//! BMODULE b582 (card 597), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03830, data area 0x1ED0000) [F]. The entry takes (D, param). Two task pools: A, 180 tasks of 0x38
//! bytes at D+0x2A0 (alive +0x2C, step +0x10; spawned by 0x1E04430), and
//! B, 45 tasks of 0x40 bytes at D+0x2A00 (alive +0x36, step +0x24; spawned
//! by 0x1E04680), drawn with the second library: rings of three quads with
//! a mode (383a1b86ead2 at 0x1E00EA0, `quads4::ring_at_m`), the 18-strip
//! band turned by the task's angles (05dcbcde46f4 at 0x1E01400,
//! `quads4::band18_turned`), far-Z quarter quads (a6d0a94b7c36 at
//! 0x1E01810, `quads4::quarters_far`), full-window gouraud tints and the
//! record's own drop lines (0x1E047B0), splash sprites (0x1E04B00), sprite
//! grids (0x1E04E50 / 0x1E05150) and band ring (0x1E05460).
//!
//! On update 98 a ring of six sprite bands, a ring band, a facing glow and
//! four quarter flashes appear about the attacker's node 0xB; from 130 to
//! 305 drops fall about the target and the attacker (splashing on the
//! ground half the time), the attacker's node 0x3D throws grey smoke to the
//! target's random node, and on 130 the window is tinted with a pulsing
//! sprite grid. Returns 1 on update 120, 3 on 310, 6 on 131..299, 8 once
//! the 345-update timer D+0x35B0 has run out.
//!
//! Pool B's steps 0x1E02680, 0x1E02970, 0x1E03470 (camera slots), 0x1E035E0
//! (the stage light), 0x1E036A0 (an object op through 0x1F8380), 0x1E036F0
//! (a frame tint) and the B-side case of 0x1E032D0 are never spawned [F:
//! the emitter's and the steps' spawn `jal`s name the steps]; they are not
//! ported.
use crate::blib::{afterimage, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib::ALPHA_ADD;
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const POOL_A: u32 = D + 0x2A0;
const POOL_B: u32 = D + 0x2A00;
const POS: u32 = D + 0x250;
const TIMER: u32 = D + 0x35B0;
const COLOUR: u32 = D + 0x35B4;
const OTW: u32 = D + 0x35BC;
const COSINE: u32 = D + 0x35C8;
const MEAN: u32 = D + 0x35CC;
const ANGLE: u32 = D + 0x35EC;
const INDEX: u32 = D + 0x35F0;
const PARAM: u32 = D + 0x35F2;
const COUNTER: u32 = D + 0x35F4;
const BYTES: u32 = D + 0x35F6;
const DIR: u32 = D + 0x3560;
/// The attacker's nodes 0x3D (then 0x3A / 0x40 / 0x47) / 0xB and the target's.
const NODE_3D: u32 = D + 0x35DC;
const NODE_B: u32 = D + 0x35E4;
const TNODE: u32 = D + 0x35EA;

/// rand % m (HI) from the `jal` at `site` (rand's frame: `s0`, ra).
fn rmod(fx: &mut Fx, s0: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, site + 8);
    fx.div(v, m).1
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x109600(out, w, h): a quad in the xz plane: (-w, 0, h), (w, 0, h),
/// (-w, 0, -h), (w, 0, -h) (the w components left) [F].
fn quad_xz(fx: &mut Fx, out: u32, w: f32, h: f32) {
    let (nw, nh) = (ee::neg(w), ee::neg(h));
    for (k, (x, z)) in [(nw, h), (w, h), (nw, nh), (w, nh)].into_iter().enumerate() {
        let a = out + 0x10 * k as u32;
        fx.mem.setf(a, x);
        fx.mem.set32(a + 4, 0);
        fx.mem.setf(a + 8, z);
    }
}

/// 0x1087A0(c, _, index): the colour `index` (0 white, 1 red, 2 green, 3
/// blue, 4 yellow, 5 magenta, 6 cyan, 7 pink, 8 violet, 9 orange, else
/// white) into the three bytes at `c` [F].
fn colour_index(fx: &mut Fx, c: u32, index: i32) {
    let rgb: [u8; 3] = match index as i16 {
        1 => [0xFF, 0, 0],
        2 => [0, 0xFF, 0],
        3 => [0, 0, 0xFF],
        4 => [0xFF, 0xFF, 0],
        5 => [0xFF, 0, 0xFF],
        6 => [0, 0xFF, 0xFF],
        7 => [0xFF, 0x80, 0xFF],
        8 => [0x80, 0, 0xFF],
        9 => [0xFF, 0x80, 0],
        _ => [0xFF, 0xFF, 0xFF],
    };
    for (k, b) in rgb.into_iter().enumerate() {
        fx.mem.write_u8(c + k as u32, b);
    }
}

/// The entry 0x1E03830 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x280);
    let at = fx.camera_at30(0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param, at) };
    fx.leave(0x280);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3870);
    l2::sine_table(fx, D + 0x36F8);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(PARAM, param);
    fx.mem.set32(TIMER, 0x159);
    for (k, (node, clear)) in [(0x3D, 1), (0x3A, 0), (0x40, 0), (0x47, 0), (0xB, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(NODE_3D + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TNODE, r);
    for id in 0xAu32..0x10 {
        let t = fx.texture(4, id as i32);
        fx.mem.write_u32(D + 0x288 + 4 * (id - 0xA), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x268, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x3540 + 4 * k, t);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3994, 0xA);
        fx.mem.set8(BYTES + k, v - 5);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, k, 0x01E0_39C8, 0xF);
        fx.mem.set8(BYTES + k, v + 2);
    }
    for k in 0..8u32 {
        let v = fx.rand_from(k, 0x01E0_3A04);
        fx.mem.set16(D + 0xC0 + 2 * k, v % 4);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32, at: u32) -> i32 {
    emit(fx, param, at);
    for k in 0..0xB4u32 {
        let t = POOL_A + 0x38 * k;
        if fx.mem.i16(t + 0x2C) == 0 {
            continue;
        }
        let s = fx.mem.read_u32(t + 0x10);
        if s == 0x01E0_2100 {
            drop_step(fx, t, at);
        }
        if fx.mem.read_u32(t + 0x10) == 0x01E0_32D0 {
            splash_step(fx, t);
        }
    }
    for k in 0..0x2Du32 {
        let t = POOL_B + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let s3 = D + 0x40 * k;
        match fx.mem.read_u32(t + 0x24) {
            0x01E0_2210 => tint_step(fx, t),
            0x01E0_2300 => smoke_step(fx, t),
            0x01E0_2550 => grid_step(fx, t, at),
            0x01E0_2D30 => band_step(fx, t),
            0x01E0_2F20 => glow_step(fx, t),
            0x01E0_30B0 => fade_step(fx, t, s3, at),
            0x01E0_3360 => quarters_step(fx, t),
            _ => {}
        }
    }
    for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x3F), (0x9A, 0x3F)] {
        fx.mem.set16(D + o, v);
    }
    let w = fx.mem.read_u32(D + 0x3548);
    fx.mem.write_u32(D + 0x88, w);
    for k in 0..0xB4u32 {
        let t = POOL_A + 0x38 * k;
        if fx.mem.i16(t + 0x2C) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x10) == 0x01E0_2100 {
            drops(fx, t, sp + 0x1E8);
        }
        if fx.mem.read_u32(t + 0x10) == 0x01E0_32D0 {
            splash(fx, t, sp + 0x1E8);
        }
    }
    for k in 0..0x2Du32 {
        let s0 = D + 0x40 * k;
        let t = POOL_B + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        draw(fx, sp, t, s0, at);
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0x78 {
        1
    } else if c == 0x136 {
        3
    } else if (0x83..0x12C).contains(&c) {
        6
    } else if fx.mem.i32(TIMER) <= 0 {
        8
    } else {
        0
    }
}

/// A full-window gouraud strip (colours +0x28 top, +0x2C bottom) at key 0,
/// from the entry: 0x109BC0's frame (its saved ra `ra`, the entry's s0)
/// stays below the entry's (a later 0x106960's local reads its upper half).
fn window_tint(fx: &mut Fx, t: u32, d: u32, s0: u32, ra: u32) {
    l2::colour_strip4(fx, d, t + 0x28, 1);
    l2::colour_strip4(fx, d, t + 0x2C, 2);
    l2::desc_gouraud4(fx, d, 0);
    let f = fx.sp - 0x20;
    fx.saved(f, &[(0x10, u64::from(ra)), (0, u64::from(s0))]);
    l2::fullscreen4(fx, d);
    fx.submit(d, OT, 0);
}

/// The atlas rectangle of sprite `id` frame `sub` (into the entry's local
/// `local`) as D+0x94.
fn rect_d(fx: &mut Fx, local: u32, id: i32, sub: i32) {
    l2::rect(fx, local, id, sub);
    copy4(fx, D + 0x94, local);
}

/// The attacker's node 0xB, 2 higher, into `out` (0x1F6830 from the entry:
/// its frame with the entry's s0 / s1).
fn node_b_up(fx: &mut Fx, out: u32, s0: u32, at: u32, ra: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE_B);
    crate::b076::node_position_saving(fx, h, n, out, u64::from(s0), Some(u64::from(at)), ra);
    let y = ee::add(fx.mem.f(out + 4), 2.0);
    fx.mem.setf(out + 4, y);
}

/// The entry's draw of a live pool B task (s0 = D + 0x40 k).
fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32, at: u32) {
    match fx.mem.read_u32(t + 0x24) {
        0x01E0_2210 => window_tint(fx, t, sp + 0xC0, s0, 0x01E0_3EA0),
        0x01E0_2300 => {
            let w = fx.mem.read_u32(D + 0x3544);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            rect_d(fx, sp + 0x70, 0x14, sub);
            if fx.mem.i16(t + 0x32) >= 4 {
                fx.mem.set16(D + 0xB8, 3);
                let v = fx.rand_from(s0, 0x01E0_3F28);
                fx.mem.set16(D + 0xBE, v % 4);
                for k in 0..3u32 {
                    let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                    fx.mem.setf(D + 0x20 + 4 * k, v);
                }
                quads4::ring_at_m(fx, uv::uv_rect, D, t, 0x20, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
        }
        0x01E0_2550 => {
            let sub = fx.mem.i16(t + 0x34);
            rect_d(fx, sp + 0x80, 0x9C, sub);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xBE, 0);
            for (o, dv) in [(0x94u32, 1), (0x96, 1), (0x98, -1), (0x9A, -1)] {
                let v = fx.mem.i16(D + o) + dv;
                fx.mem.set16(D + o, v);
            }
            grid(fx, t, sp + 0x1E8, false);
            fx.mem.set16(D + 0xBE, 1);
            grid(fx, t, sp + 0x1E8, true);
        }
        0x01E0_2D30 => {
            let p = sp + 0x60;
            node_b_up(fx, p, s0, at, 0x01E0_4194);
            let x = ee::div(ee::sub(fx.mem.f(p), fx.mem.f(at)), 100.0);
            fx.mem.setf(D + 0x20, x);
            fx.mem.write_u32(D + 0x24, 0xBD4C_CCCD);
            let z = ee::div(ee::sub(fx.mem.f(p + 8), fx.mem.f(at + 8)), 100.0);
            fx.mem.setf(D + 0x28, z);
            let f = ee::from_i32(fx.mem.i16(t + 0xA));
            for o in [0x20u32, 0x24, 0x28] {
                let v = ee::mul(fx.mem.f(D + o), f);
                fx.mem.setf(D + o, v);
            }
            ring_band(fx, t, p);
        }
        0x01E0_2F20 => {
            let p = sp + 0x60;
            node_b_up(fx, p, s0, at, 0x01E0_425C);
            quads4::band18_turned(fx, D, t, 1, p);
        }
        0x01E0_30B0 => window_tint(fx, t, sp + 0x150, s0, 0x01E0_42D4),
        0x01E0_3360 => {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            rect_d(fx, sp + 0xB0, 0x9A, 1);
            let p = sp + 0x60;
            node_b_up(fx, p, s0, at, 0x01E0_4358);
            quads4::quarters_far(fx, D, t, sp + 0x1E8, p);
        }
        _ => {}
    }
}

/// A step's frame: 0x20 (ra, s0 = the entry's 0) or 0x30 (ra, s1 = the
/// camera pointer, s0 = 0).
fn frame(fx: &mut Fx, size: u32, ra: u64, at: u32) {
    let sp = fx.enter(size);
    if size == 0x20 {
        fx.saved(sp, &[(0x10, ra), (0, 0)]);
    } else {
        fx.saved(sp, &[(0x20, ra), (0x10, u64::from(at)), (0, 0)]);
    }
}

fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8, end: bool) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else if end {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.write_u8(t + at, 0);
    }
}

fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// 0x1E02100 (D, task A): a drop: its fade +0x26 up by 25 (under 50) above
/// y 20, down by 20 to 0 below; falling 30 a update; under 0 it ends, half
/// the time (before update 300) splashing (0x1E032D0) where it fell.
fn drop_step(fx: &mut Fx, t: u32, at: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3A74), (0x10, u64::from(at)), (0, 0)]);
    if crate::exe::ee_le(fx.mem.f(t + 4), 20.0) {
        let v = fx.mem.i16(t + 0x26);
        fx.mem.set16(t + 0x26, if v > 0 { v - 0x14 } else { 0 });
    } else {
        let v = fx.mem.i16(t + 0x26);
        if v < 0x32 {
            fx.mem.set16(t + 0x26, v + 0x19);
        }
    }
    let y = ee::sub(fx.mem.f(t + 4), 30.0);
    fx.mem.setf(t + 4, y);
    if crate::exe::ee_lt(y, 0.0) {
        copy3(fx, POS, t);
        let v = fx.rand_from(t, 0x01E0_21AC);
        if v % 2 == 0 && fx.mem.i16(COUNTER) < 0x12C {
            spawn_a(fx, 0x01E0_32D0);
        }
        fx.mem.set16(t + 0x2C, 0);
    }
    fx.leave(0x30);
}

/// 0x1E032D0 (D, task A), the entry's own case: the splash alpha +0x28 from
/// 0x80, down by 19 from age 61 (to 20), ending from age 91 below 20; age
/// +0x32 up by 15.
fn splash_step(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x2C) == 1 {
        fx.mem.set16(t + 0x32, 0);
        fx.mem.set16(t + 0x2C, 2);
        fx.mem.set16(t + 0x28, 0x80);
    }
    if fx.mem.i16(t + 0x32) >= 0x3D {
        let v = fx.mem.i16(t + 0x28);
        if v >= 0x15 {
            fx.mem.set16(t + 0x28, v - 0x13);
        }
    }
    if fx.mem.i16(t + 0x32) >= 0x5B && fx.mem.i16(t + 0x28) < 0x14 {
        fx.mem.set16(t + 0x2C, 0);
    }
    let a = fx.mem.i16(t + 0x32) + 0xF;
    fx.mem.set16(t + 0x32, a);
}

/// 0x1E02210 (D, task B): the window tint (blue), in by 0xC for 193
/// updates, then out by 9.
fn tint_step(fx: &mut Fx, t: u32) {
    frame(fx, 0x20, 0x01E0_3B54, 0);
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x80);
        l2::rgb(fx, t + 0x2C, 0x10, 0x10, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xC1 {
        fade_in(fx, t, 0x2B, 0x80, 0xC);
        fade_in(fx, t, 0x2F, 0x80, 0xC);
    } else {
        fade_out(fx, t, 0x2B, 0xB, 9, true);
        fade_out(fx, t, 0x2F, 0xB, 9, true);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02300 (D, task B): grey smoke: size 12..23 (half the target's
/// radius for index 0), the radius +0x30 kept in +0xA, set 10..109, angle,
/// growth 5..9, row 0..34, life 2..6; in by 0x40, out by 7; growing to 60,
/// rising 1 and its cell counting to 5 on odd ages.
fn smoke_step(fx: &mut Fx, t: u32) {
    frame(fx, 0x20, 0x01E0_3B78, 0);
    let s = 0x01E0_2300;
    if fx.mem.i16(t + 0x36) == 1 {
        if fx.mem.i16(t + 0x30) != 0 {
            let v = rmod(fx, t, s + 0x28, 0xC);
            fx.mem.set16(t + 0x38, v + 0xC);
        } else {
            let h = fx.target_handle();
            let r = afterimage::unit_radius(fx, h);
            fx.mem.set16(t + 0x38, ee::fixsfsi(ee::div(r, 2.0)));
        }
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x40);
        let r = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, r);
        let v = rmod(fx, t, s + 0xA0, 0x64);
        fx.mem.set16(t + 0x30, v + 0xA);
        let v = rmod(fx, t, s + 0xC0, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, s + 0xDC, 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rmod(fx, t, s + 0xFC, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, s + 0x118, 5);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x2B, 0x81, 0x40);
    } else {
        fade_out(fx, t, 0x2B, 9, 7, true);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x38);
        if v < 0x3C {
            fx.mem.set16(t + 0x38, v + 1);
        }
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let y = fx.mem.i16(t + 2) + 1;
        fx.mem.set16(t + 2, y);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 5 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02550 (D, task B): the sprite grids' tint (dark red, size 36): in
/// by 9 to 0x61 before update 300, then out by 9; both angles +0x3A / +0xA
/// turning 4 a update.
fn grid_step(fx: &mut Fx, t: u32, at: u32) {
    frame(fx, 0x30, 0x01E0_3B98, at);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x24);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x20, 8, 8);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(COUNTER) < 0x12C {
        fade_in(fx, t, 0x2B, 0x61, 9);
    } else {
        fade_out(fx, t, 0x2B, 0xB, 9, true);
    }
    for o in [0x3Au32, 0xA] {
        let v = fx.mem.i16(t + o) + 4;
        fx.mem.set16(t + o, v);
    }
    for o in [0x3Au32, 0xA] {
        let v = fx.mem.i16(t + o);
        if v >= 0x168 {
            let r = fx.div(v, 0x168).1;
            fx.mem.set16(t + o, r);
        }
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02D30 (D, task B): a sprite band ring (grey 0x70): random angles
/// +0x10 / +0x14 / +0x18 (then +0x10 = the camera's x + 0x4000), radius
/// +0xA 10..69, size 15..64, height +8 (0, or size - 2 for index 1),
/// angle +0x3A, +0xE 0..9; in by 0x40 for 4 updates, out by 9.
fn band_step(fx: &mut Fx, t: u32) {
    frame(fx, 0x20, 0x01E0_3BF8, 0);
    let s = 0x01E0_2D30;
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x28);
        l2::rgb(fx, t + 0x28, 0x70, 0x70, 0x70);
        l2::rgb(fx, t + 0x2C, 0x70, 0x70, 0x70);
        let v = rmod(fx, t, s + 0x4C, 0x1E);
        fx.mem.set32(t + 0x10, v + 5);
        let v = rmod(fx, t, s + 0x6C, 0xA);
        fx.mem.set32(t + 0x14, v - 5);
        let v = rmod(fx, t, s + 0x8C, 0x1E);
        fx.mem.set32(t + 0x18, v + 5);
        let v = rmod(fx, t, s + 0xAC, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.write_u8(t + 0x2F, 0);
        let v = rmod(fx, t, s + 0xDC, 0x3C);
        fx.mem.set16(t + 0xA, v + 0xA);
        let v = rmod(fx, t, s + 0xFC, 0x32);
        fx.mem.set16(t + 0x38, v + 0xF);
        if fx.mem.i16(t + 0x30) == 0 {
            fx.mem.set16(t + 8, 0);
        }
        if fx.mem.i16(t + 0x30) == 1 {
            let v = fx.mem.i16(t + 0x38) - 2;
            fx.mem.set16(t + 8, v);
        }
        let v = rmod(fx, t, s + 0x14C, 0xA);
        fx.mem.set16(t + 0xE, v);
    }
    if fx.mem.i16(t + 0x32) < 4 {
        fade_in(fx, t, 0x2B, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x2B, 0xB, 9, true);
    }
    l2::camera_angles_to(fx, t + 0x10);
    let x = fx.mem.i32(t + 0x10).wrapping_add(0x4000);
    fx.mem.set32(t + 0x10, x);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02F20 (D, task B): the facing band (size 20; index 0 white / orange,
/// 1 green, 2 blue), in by 0x60 for 3 updates, out by 0xF; +0x10 = the
/// camera's angles (x + 0x4000).
fn glow_step(fx: &mut Fx, t: u32) {
    frame(fx, 0x20, 0x01E0_3C18, 0);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x14);
        match fx.mem.i16(t + 0x30) {
            0 => {
                l2::rgb(fx, t + 0x28, 0xFF, 0xFF, 0xFF);
                l2::rgb(fx, t + 0x2C, 0xFF, 0x80, 8);
            }
            1 => {
                l2::rgb(fx, t + 0x28, 0x10, 0x80, 0x10);
                l2::rgb(fx, t + 0x2C, 0x10, 0x80, 0x10);
            }
            2 => {
                l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x80);
                l2::rgb(fx, t + 0x2C, 0x10, 0x10, 0x80);
            }
            _ => {}
        }
        let v = rmod(fx, t, 0x01E0_2FEC, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.write_u8(t + 0x2F, 8);
    }
    if fx.mem.i16(t + 0x32) < 3 {
        fade_in(fx, t, 0x2B, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x2B, 0x11, 0xF, true);
    }
    l2::camera_angles_to(fx, t + 0x10);
    let x = fx.mem.i32(t + 0x10).wrapping_add(0x4000);
    fx.mem.set32(t + 0x10, x);
    age(fx, t);
    fx.leave(0x20);
}

/// EE `adda.s` + `madd.s`: (a b + c d) + e f, each product rounded.
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let acc = ee::add(ee::mul(a[0], b[0]), ee::mul(a[1], b[1]));
    ee::add(acc, ee::mul(a[2], b[2]))
}

/// 0x1E030B0 (D, task B): the window tint (0x80, 0x60, 0x60): on its first
/// update the cosine D+0x35C8 between the camera's view (eye - at) and the
/// attacker's node 0xB (2 higher) from the camera's at; its alpha 128 x
/// that cosine for 3 updates, then out by 0xB.
fn fade_step(fx: &mut Fx, t: u32, s3: u32, at0: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, 0x01E0_3C38), (0x40, u64::from(s3)), (0x30, u64::from(D)), (0x20, u64::from(at0)), (0x10, 0)]);
    let at = fx.camera_at30(0);
    let eye = fx.camera_eye(0);
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x80, 0x60, 0x60);
        l2::rgb(fx, t + 0x2C, 0x80, 0x60, 0x60);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.write_u8(t + 0x2F, 0);
        let v = rmod(fx, at, 0x01E0_3134, 0xA);
        fx.mem.set16(t + 0xE, v);
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE_B);
        fx.node_position(h, n, DIR);
        let y = ee::add(fx.mem.f(DIR + 4), 2.0);
        fx.mem.setf(DIR + 4, y);
        for k in 0..3u32 {
            let v = ee::sub(fx.mem.f(DIR + 4 * k), fx.mem.f(at + 4 * k));
            fx.mem.setf(DIR + 4 * k, v);
        }
        let view: [f32; 3] = std::array::from_fn(|k| ee::sub(fx.mem.f(eye + 4 * k as u32), fx.mem.f(at + 4 * k as u32)));
        let p: [f32; 3] = std::array::from_fn(|k| fx.mem.f(DIR + 4 * k as u32));
        let dot = dot3(p, view);
        let l1 = fx.sqrtf(dot3(view, view));
        let p: [f32; 3] = std::array::from_fn(|k| fx.mem.f(DIR + 4 * k as u32));
        let l2v = fx.sqrtf(dot3(p, p));
        fx.mem.setf(COSINE, ee::div(dot, ee::mul(l1, l2v)));
    }
    if fx.mem.i16(t + 0x32) < 3 {
        let a = ee::fixunssfsi(ee::mul(128.0, fx.mem.f(COSINE))) as u8;
        fx.mem.write_u8(t + 0x2B, a);
        fx.mem.write_u8(t + 0x2F, a);
    } else {
        fade_out(fx, t, 0x2B, 0xD, 0xB, true);
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2F, a);
    }
    age(fx, t);
    fx.leave(0x60);
}

/// 0x1E03360 (D, task B): the quarter flash (brown 0x40 0x30 8, size 6,
/// quarters (4, 0x37)); in by 0x40 for 2 updates, out by 0xF.
fn quarters_step(fx: &mut Fx, t: u32) {
    frame(fx, 0x20, 0x01E0_3CF0, 0);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        l2::rgb(fx, t + 0x28, 0x40, 0x30, 8);
        l2::rgb(fx, t + 0x2C, 0x40, 0x30, 8);
        fx.mem.set16(t + 8, 4);
        fx.mem.set16(t + 0xA, 0x37);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        fade_in(fx, t, 0x2B, 0x80, 0x40);
        fade_in(fx, t, 0x2F, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x2B, 0x11, 0xF, true);
        fade_out(fx, t, 0x2F, 0x11, 0xF, false);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E04430(D, step, 0): a pool A task (first free of 180) at D+0x250
/// (floats +0..+8), its four streak points +0x14.. / +0x1A.. the position
/// (as i16) each moved by -75..74.
fn spawn_a(fx: &mut Fx, step: u32) {
    fx.enter(0x50);
    let mut task = None;
    for k in 0..0xB4u32 {
        let t = POOL_A + 0x38 * k;
        if fx.mem.i16(t + 0x2C) == 0 {
            fx.bzero(t, 0x38);
            fx.mem.set16(t + 0x2C, 1);
            task = Some(t);
            break;
        }
    }
    if let Some(t) = task {
        copy3(fx, t, POS);
        for (o, c) in [(0x14u32, 0u32), (0x1A, 8), (0x16, 0), (0x1C, 8), (0x18, 0), (0x1E, 8)] {
            let v = ee::fixsfsi(fx.mem.f(POS + c));
            fx.mem.set16(t + o, v);
        }
        for (k, o) in [0x14u32, 0x1A, 0x16, 0x1C, 0x18, 0x1E].into_iter().enumerate() {
            let v = fx.rand_from(t, 0x01E0_4530 + 0x30 * k as u32);
            let r = fx.div(v, 0x96).1;
            let p = fx.mem.i16(t + o) + (r - 0x4B) as i16 as i32;
            fx.mem.set16(t + o, p);
        }
        fx.mem.write_u32(t + 0x10, step);
    }
    fx.leave(0x50);
}

/// 0x1E04680(D, step, 0): a pool B task (first free of 45) with the spawn
/// colour D+0x35B4 (both colours), at D+0x250, index D+0x35F0, angle
/// D+0x35EC.
fn spawn_b(fx: &mut Fx, step: u32) {
    fx.enter(0x60);
    let mut task = None;
    for k in 0..0x2Du32 {
        let t = POOL_B + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            task = Some(t);
            break;
        }
    }
    if let Some(t) = task {
        for o in [0x28u32, 0x2C] {
            let c: [i32; 4] = std::array::from_fn(|k| i32::from(fx.mem.read_u8(COLOUR + k as u32)));
            l2::rgb(fx, t + o, c[0], c[1], c[2]);
            fx.mem.write_u8(t + o + 3, c[3] as u8);
        }
        l2::to_i16x4(fx, t, POS, 0);
        let i = fx.mem.i16(INDEX);
        fx.mem.set16(t + 0x30, i);
        let a = fx.mem.i16(ANGLE);
        fx.mem.set16(t + 0x3A, a);
        fx.mem.set32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, step);
    }
    fx.leave(0x60);
}

/// Six drops in a square of `half` about `p` (y + 150 + 0..99), rand's
/// frame s0 = the loop count.
fn drops_about(fx: &mut Fx, p: u32, n: u32, m: i32, half: f32, sites: [u32; 3]) {
    for k in 0..n {
        let v = fx.rand_from(k, sites[0]);
        let r = fx.div(v, m).1;
        let x = ee::sub(ee::add(fx.mem.f(p), ee::from_i32(r)), half);
        fx.mem.setf(POS, x);
        let v = fx.rand_from(k, sites[1]);
        let r = fx.div(v, 0x64).1;
        let y = ee::add(ee::add(150.0, fx.mem.f(p + 4)), ee::from_i32(r));
        fx.mem.setf(POS + 4, y);
        let v = fx.rand_from(k, sites[2]);
        let r = fx.div(v, m).1;
        let z = ee::sub(ee::add(fx.mem.f(p + 8), ee::from_i32(r)), half);
        fx.mem.setf(POS + 8, z);
        spawn_a(fx, 0x01E0_2100);
    }
}

/// 0x1E01AB0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x70).
fn emit(fx: &mut Fx, param: i32, at0: u32) {
    let sp = fx.enter(0x70);
    // The entry's ra, s3 = param, s2 = D, s1 = the camera pointer, s0 = 0.
    fx.saved(sp, &[(0x40, 0x01E0_3A40), (0x30, param as i64 as u64), (0x20, u64::from(D)), (0x10, u64::from(at0)), (0, 0)]);
    let at = fx.camera_at30(0);
    l2::rgb(fx, COLOUR, 0x80, 0, 0);
    fx.mem.write_u8(COLOUR + 3, 0);
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    if fx.mem.i16(COUNTER) == 0x62 {
        fx.mem.set16(INDEX, 0);
        for _ in 0..6 {
            spawn_b(fx, 0x01E0_2D30);
        }
        spawn_b(fx, 0x01E0_2F20);
        spawn_b(fx, 0x01E0_30B0);
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE_B);
        fx.node_position(h, n, sp + 0x50);
        copy3(fx, POS, sp + 0x50);
        fx.mem.set16(INDEX, 1);
        for a in [0x87, 0x2D, 0x5A, 0] {
            fx.mem.set16(ANGLE, a);
            spawn_b(fx, 0x01E0_3360);
        }
    }
    if fx.mem.i16(COUNTER) == 0x82 {
        spawn_b(fx, 0x01E0_2210);
    }
    if fx.mem.i16(COUNTER) == 0x82 {
        spawn_b(fx, 0x01E0_2550);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x82..0x132).contains(&c) {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x50);
        drops_about(fx, sp + 0x50, 7, 0xFA, 125.0, [0x01E0_1CD4, 0x01E0_1D08, 0x01E0_1D40]);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x82..0x132).contains(&c) {
        let h = fx.self_handle();
        fx.unit_position(h, sp + 0x50);
        drops_about(fx, sp + 0x50, 2, 0x96, 75.0, [0x01E0_1DD4, 0x01E0_1E08, 0x01E0_1E40]);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x82..0x132).contains(&c) {
        if fx.div(c, 0xA).1 == 1 {
            let h = fx.target_handle();
            let n = fx.unit_nodes(h);
            let v = fx.rand_from(n as u32, 0x01E0_1EEC);
            let mut r = fx.div(v, n).1;
            if r == 0 {
                r += 1;
            }
            let h = fx.target_handle();
            let q = fx.request_node(h, r, 1);
            fx.mem.set16(TNODE, q);
            let h = fx.self_handle();
            let q = fx.request_node(h, 0x3D, 1);
            fx.mem.set16(NODE_3D, q);
        } else {
            let h = fx.target_handle();
            let n = fx.mem.i16(TNODE);
            fx.node_position(h, n, sp + 0x50);
            copy3(fx, POS, sp + 0x50);
            fx.mem.set16(INDEX, 0);
            spawn_b(fx, 0x01E0_2300);
            let h = fx.self_handle();
            let n = fx.mem.i16(NODE_3D);
            fx.node_position(h, n, sp + 0x50);
            for (k, site) in [0x01E0_1FB0u32, 0x01E0_1FE4, 0x01E0_2018].into_iter().enumerate() {
                let v = fx.rand_from(sp + 0x54, site);
                let r = fx.div(v, 0x12).1;
                let p = ee::sub(ee::add(fx.mem.f(sp + 0x50 + 4 * k as u32), ee::from_i32(r)), 6.0);
                fx.mem.setf(POS + 4 * k as u32, p);
            }
            fx.mem.set16(INDEX, 1);
            spawn_b(fx, 0x01E0_2300);
        }
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE_B);
    fx.node_position(h, n, sp + 0x60);
    let y = ee::add(fx.mem.f(sp + 0x64), 2.0);
    fx.mem.setf(sp + 0x64, y);
    for k in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x60 + 4 * k), fx.mem.f(at + 4 * k)), 100.0);
        fx.mem.setf(DIR + 4 * k, v);
    }
    fx.leave(0x70);
}

/// 0x1E047B0(D, task A, desc, 0): the drop (frame 0xC0): a 2-vertex
/// additive line (0x11 flags) from each of four points (the drop, then
/// its streak points +0x12 + 2k / +0x18 + 2k added in x / z) to that point
/// lifted by i16 +0x26 (and +0x20 / +0x24, times 4 then 3, in x / z), the
/// colour the record's word 0x1E05790 (alpha 0xFF at the top, 0 at the
/// foot; D+0x35B4 set white, or orange for param != 0), keyed by the mean
/// w D+0x35CC.
fn drops(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0xC0);
    let c = fx.mem.read_u32(0x01E0_5790);
    fx.mem.write_u32(sp + 0xBC, c);
    let index = if fx.mem.i16(PARAM) == 0 { 0 } else { 9 };
    colour_index(fx, COLOUR, index);
    fx.mem.write_u16(d + 2, 0x11);
    fx.mem.write_u16(d, 4);
    fx.mem.write_u64(d + 8, 0x4141);
    fx.mem.write_u32(d + 4, 0);
    fx.desc_alpha(d, ALPHA_ADD);
    fx.mem.write_u32(d + 0x24, 0x3F80_0000);
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    fx.mem.write_u8(d + 0x1F, 0);
    fx.mem.write_u8(sp + 0xBF, 0xFF);
    fx.mem.write_u8(d + 0x2F, 0);
    crate::c061::rgbaq_at(fx, sp + 0xA8, sp + 0xBC, d + 0x10);
    fx.mem.write_u8(sp + 0xBF, 0);
    crate::c061::rgbaq_at(fx, sp + 0xB0, sp + 0xBC, d + 0x20);
    for k in 0..4u32 {
        let (a, b) = (sp + 0x60, sp + 0x70);
        copy4(fx, a, t);
        copy4(fx, b, t);
        if k == 0 {
            let v = ee::add(fx.mem.f(b + 4), ee::from_i32(fx.mem.i16(t + 0x26)));
            fx.mem.setf(b + 4, v);
            let v = ee::add(fx.mem.f(b), ee::from_i32(fx.mem.i16(t + 0x20) << 2));
            fx.mem.setf(b, v);
            let v = ee::add(fx.mem.f(b + 8), ee::from_i32(fx.mem.i16(t + 0x24) << 2));
            fx.mem.setf(b + 8, v);
        } else {
            let s1 = t + 2 * k;
            for (p, o) in [(a, 0u32), (a + 8, 6), (b, 0), (b + 8, 6)] {
                let v = ee::add(fx.mem.f(p), ee::from_i32(fx.mem.i16(s1 + 0x12 + o)));
                fx.mem.setf(p, v);
            }
            let v = ee::add(fx.mem.f(b + 4), ee::from_i32(fx.mem.i16(t + 0x26)));
            fx.mem.setf(b + 4, v);
            let v = ee::add(fx.mem.f(b), ee::from_i32(3 * fx.mem.i16(t + 0x20)));
            fx.mem.setf(b, v);
            let v = ee::add(fx.mem.f(b + 8), ee::from_i32(3 * fx.mem.i16(t + 0x24)));
            fx.mem.setf(b + 8, v);
        }
        if l2::project(fx, sp + 0x60, SPR, 0, d, MEAN) {
            let key = ee::fixunssfsi(fx.mem.f(MEAN)) as i32;
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0xC0);
}

/// 0x1E04B00(D, task A, desc, 0): the splash (frame 0x140): an additive
/// sprite (UVs D+0x94 in orientation D+0xBE, texture D+0x88) of size i16
/// +0x32 at the drop and its three streak points (x / z + i16 +0x12 + 2k /
/// +0x18 + 2k), flat on the ground (angles (0x4000, camera y, z), scale
/// 0.1); colour the record's word 0x1E05798 with alpha the low byte of
/// +0x28; keyed by the mean w.
fn splash(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x140);
    let c = fx.mem.read_u32(0x01E0_5798);
    fx.mem.write_u32(sp + 0x13C, c);
    let a = fx.mem.read_u8(t + 0x28);
    fx.mem.write_u8(sp + 0x13F, a);
    sprite_header(fx, d);
    fx.desc_alpha(d, ALPHA_ADD);
    crate::c061::rgbaq_at(fx, sp + 0x130, sp + 0x13C, d + 0x10);
    let order = fx.mem.i16(D + 0xBE);
    uv::uv_rect(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xC0 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x120);
    fx.mem.set32(sp + 0x120, 0x4000);
    for k in 0..4u32 {
        if k == 0 {
            copy3(fx, sp + 0xB0, t);
        } else {
            let s4 = t + 2 * k;
            let x = ee::add(fx.mem.f(t), ee::from_i32(fx.mem.i16(s4 + 0x12)));
            fx.mem.setf(sp + 0xB0, x);
            let y = fx.mem.read_u32(t + 4);
            fx.mem.write_u32(sp + 0xB4, y);
            let z = ee::add(fx.mem.f(t + 8), ee::from_i32(fx.mem.i16(s4 + 0x18)));
            fx.mem.setf(sp + 0xB8, z);
        }
        l2::xform_vp(fx, sp + 0xE0, sp + 0xC0, sp + 0x120, sp + 0xB0);
        let size = ee::from_i32(fx.mem.i16(t + 0x32));
        l2::quad_xy(fx, sp + 0x70, size, size);
        if l2::project(fx, sp + 0x70, SPR, sp + 0xE0, d, sp + 0xD0) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0xD0)) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x140);
}

/// A textured sprite strip's header (9 values, flags 0x824).
fn sprite_header(fx: &mut Fx, d: u32) {
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, (4u64 << 32) | 0x2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// 0x1E04E50 / 0x1E05150 (D, task B, desc, 0) (`second`): a 40 x 25 grid
/// of additive sprites of size s = i16 +0x38 lying in the xz plane at x
/// 2 s col - 25 s / 2 (+ the sway, for the first), z 2 s row - 20 s (+
/// the sway for the second), the sway 20 (30) sin +0x3A, height -1 + 10
/// sin a (-10 + 20 sin a) with a from +0xA (+ 60) by 6 a row; each keyed
/// by its mean w [F].
fn grid(fx: &mut Fx, t: u32, d: u32, second: bool) {
    let sp = fx.enter(0x120);
    sprite_header(fx, d);
    fx.desc_alpha(d, ALPHA_ADD);
    crate::c061::rgbaq_at(fx, sp + 0x118, t + 0x28, d + 0x10);
    let order = fx.mem.i16(D + 0xBE);
    uv::uv_rect(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    let s = fx.mem.i16(t + 0x38);
    let five = s.wrapping_mul(5);
    let s7 = ((five.wrapping_mul(5)) >> 1) as i16 as i32;
    let s1 = ((five << 3) >> 1) as i16 as i32;
    let a = fx.mem.i16(t + 0x3A);
    let sn = l2::sin_deg(fx, D + 0xD8, a, 0);
    let sway = ee::mul(if second { 30.0 } else { 20.0 }, sn);
    let mut fp = if second { (fx.mem.i16(t + 0xA) + 0x3C) as i16 as i32 } else { fx.mem.i16(t + 0xA) };
    fx.mem.set16(sp + 0xB0, 0);
    while fx.mem.i16(sp + 0xB0) < 0x28 {
        let row = fx.mem.i16(sp + 0xB0);
        let sn = l2::sin_deg(fx, D + 0xD8, fp, 0);
        let h = if second { ee::add(-10.0, ee::mul(20.0, sn)) } else { ee::add(-1.0, ee::mul(10.0, sn)) };
        let mut col = 0i32;
        while col < 0x19 {
            let size = ee::from_i32(fx.mem.i16(t + 0x38));
            quad_xz(fx, sp + 0xC0, size, size);
            let w = fx_twice(fx, t);
            let x = fx.mult(col, w).wrapping_sub(s7);
            let xf = if second { ee::from_i32(x) } else { ee::add(ee::from_i32(x), sway) };
            l2::add_column(fx, sp + 0xC0, 0, xf);
            let w = fx_twice(fx, t);
            let z = fx.mult(row, w).wrapping_sub(s1);
            let zf = if second { ee::add(ee::from_i32(z), sway) } else { ee::from_i32(z) };
            l2::add_column(fx, sp + 0xC0, 2, zf);
            l2::add_column(fx, sp + 0xC0, 1, h);
            if l2::project(fx, sp + 0xC0, SPR, 0, d, sp + 0x100) {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x100)) as i32;
                let ot = fx.mem.read_u32(D + 0x80);
                fx.submit(d, ot, key);
            }
            col = (col + 1) as i16 as i32;
        }
        fp = (fp + 6) as i16 as i32;
        let r = fx.mem.i16(sp + 0xB0) + 1;
        fx.mem.set16(sp + 0xB0, r);
    }
    fx.leave(0x120);
}

/// 2 x i16 +0x38.
fn fx_twice(fx: &mut Fx, t: u32) -> i32 {
    fx.mem.i16(t + 0x38) << 1
}

/// 0x1E05460(D, task B, 0, pos): the sprite band ring (frame 0x1D0): 18
/// untextured gouraud strips (20 degrees each from i16 +0x3A) between
/// radius i16 +8 and i16 +0x38 / 100 at `pos` + D+0x20, turned by the
/// task's angles +0x10 (no scale); colours +0x28 / +0x2C; keyed by the
/// mean w.
fn ring_band(fx: &mut Fx, t: u32, pos: u32) {
    let sp = fx.enter(0x1D0);
    copy4(fx, sp + 0x90, pos);
    let d = sp + 0x140;
    l2::colour_strip4(fx, d, t + 0x28, 2);
    l2::colour_strip4(fx, d, t + 0x2C, 1);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    copy3(fx, sp + 0xE0, sp + 0x90);
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(sp + 0xE0 + 4 * k), fx.mem.f(D + 0x20 + 4 * k));
        fx.mem.setf(sp + 0xE0 + 4 * k, v);
    }
    l2::xform_vp(fx, sp + 0xF0, 0, t + 0x10, sp + 0xE0);
    for o in [0xA4u32, 0xB4, 0xC4, 0xD4] {
        fx.mem.set32(sp + o, 0);
    }
    let outer = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 100.0);
    let inner = ee::from_i32(fx.mem.i16(t + 8));
    let mut a = 0i32;
    for _ in 0..0x12 {
        let s = (fx.mem.i16(t + 0x3A) + a) as i16 as i32;
        let e = (a + (fx.mem.i16(t + 0x3A) + 0x14)) as i16 as i32;
        for (deg, v0, v1) in [(s, 0xA0u32, 0xC0u32), (e, 0xB0, 0xD0)] {
            for (r, v) in [(inner, v0), (outer, v1)] {
                let c = l2::sin_deg(fx, D + 0xD8, deg, 1);
                fx.mem.setf(sp + v, ee::mul(r, c));
                let sn = l2::sin_deg(fx, D + 0xD8, deg, 0);
                fx.mem.setf(sp + v + 8, ee::mul(r, sn));
            }
        }
        if l2::project(fx, sp + 0xA0, SPR, sp + 0xF0, d, sp + 0x130) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x130)) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
        a = (a + 0x14) as i16 as i32;
    }
    fx.leave(0x1D0);
}
