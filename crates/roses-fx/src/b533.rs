//! BMODULE b533 (card 420), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02D90, data area 0x1ED0000) [F]: a pool of 400 tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step
//! +0x44 called through `jalr`; spawned by a6cf8ad55488 at 0x1E034D0,
//! `spawn::task400`) drawn with the second library: single quads
//! (add5d81dd3fe at 0x1E00C80, `billboard::quad`), arcs (e6dce326551d at
//! 0x1E00920, `billboard::arc_strips`), flat facing rings (9c13e08cfd2f at
//! 0x1E01110, `quads4::ring_facing_flat`) and a full-window gouraud tint
//! (561a6131a635 at 0x1E00F40, `quads4::tint4`), UVs by uv_rect 0x1E00080.
//! On update 1 a frame flash (0x207EF0) that brightens on 40..49; until
//! update 100 red and dark sparks at the attacker's nodes 0x14 / 0x3F /
//! 0x26 / 0x51; on update 82 an arc, five rings and five sparks at each
//! and a red tint; from 34 to 90 white sparks orbit nodes 0x6A / 0x75 (ten
//! each on updates 34 and 90). Returns 1 on update 72, 4 on 145, 2 on 125,
//! 6 on 83..119, 8 once the 150-update timer +0xB210 has run out.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, spawn::task400, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const ARC: u32 = 0x01E0_1D00;
const SPARK: u32 = 0x01E0_1DF0;
const DARK: u32 = 0x01E0_2090;
const TINT: u32 = 0x01E0_22F0;
const RING: u32 = 0x01E0_23C0;
const PINK: u32 = 0x01E0_2630;
const ORBIT: u32 = 0x01E0_2890;
const FLASH: u32 = 0x01E0_2AD0;
/// The return address of the entry's `jalr` into a step (0x1E03088 + 8).
const TASK_RA: u64 = 0x01E0_3090;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const NODES: u32 = D + 0xB244;
const ORBIT_KIND: u32 = D + 0xB2B0;
const FACING: u32 = D + 0xB2B4;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;
const SINES: u32 = D + 0xB4BC;

/// The entry 0x1E02D90.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x96);
    for (k, node) in [0x14, 0x3F, 0x26, 0x51, 0x6A, 0x75].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x298 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(k, 0x01E0_2FB0);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_3004);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 => {}
            ARC => arc(fx, t),
            SPARK | DARK | PINK => spark(fx, t, f),
            TINT => tint(fx, t),
            RING => ring(fx, t),
            ORBIT => orbit(fx, t),
            FLASH => flash(fx, t),
            _ => panic!("b533: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == SPARK {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x50, 0x9B, sub);
            let o = fx.mem.i16(t + 0x52);
            fx.mem.set16(D + 0xBE, o);
            let sub = fx.mem.i16(t + 0x56);
            let (u, v) = if sub < 2 { (sub << 6, 0) } else { ((sub - 2) << 6, 0x40) };
            fx.mem.set16(D + 0x94, u);
            fx.mem.set16(D + 0x96, v);
            fx.mem.set16(D + 0x98, 0x40);
            fx.mem.set16(D + 0x9A, 0x40);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == DARK {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x60, 0x14, sub);
            let o = fx.mem.i16(t + 0x52);
            fx.mem.set16(D + 0xBE, o);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == TINT {
            quads4::tint4(fx, t, 0x21, 0.0);
        }
        if fx.mem.read_u32(t + 0x44) == RING {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x9B, sub);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 6);
            let o = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, o);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            quads4::ring_facing_flat(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == PINK {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x14, sub);
            let o = fx.mem.i16(t + 0x52);
            fx.mem.set16(D + 0xBE, o);
            billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == ORBIT {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x90, 0x14, sub);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x48 => 1,
        0x91 => 4,
        0x7D => 2,
        0x53..=0x77 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// rand % m (HI) from a step's `jal` at `site` (rand's frame: s0 = `t`).
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// 0x1E01660(D, 0): the update counter and the emitters (frame 0x30: ra,
/// the entry's s1 = D / s0 = 0; its own s0 = D, s1 the loop counters).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3060), (0x10, u64::from(D)), (0, 0)]);
    let p = param as i64 as u64;
    // task400's prologue saves (the emitter's s0 = D, s1, the entry's s2
    // = the parameter, s3 / s4 = 0).
    let spawn = |fx: &mut Fx, f: u32, s1: u64, ra: u32| {
        task400(fx, D, f, 0, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, s1), (0, u64::from(D))]);
    };
    let node = |fx: &mut Fx, k: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        fx.node_position(h, slot, D + 0x250);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, FLASH, u64::from(D), 0x01E0_16B4);
    }
    if fx.mem.i16(COUNTER) == 0x52 {
        // An arc, five rings and five pink sparks at each of the four
        // nodes, then the tint.
        let sites: [[u32; 3]; 4] = [[0x01E0_1700, 0x01E0_171C, 0x01E0_1758], [0x01E0_17B0, 0x01E0_17CC, 0x01E0_1808], [0x01E0_1860, 0x01E0_187C, 0x01E0_18B8], [0x01E0_1910, 0x01E0_192C, 0x01E0_1968]];
        for (k, s) in sites.iter().enumerate() {
            node(fx, k as u32);
            // The emitter's s1: D, then the last loop's count.
            spawn(fx, ARC, if k == 0 { u64::from(D) } else { 5 }, s[0]);
            for i in 0..5u64 {
                spawn(fx, RING, i, s[1]);
            }
            for i in 0..5u64 {
                spawn(fx, PINK, i, s[2]);
            }
        }
        spawn(fx, TINT, 5, 0x01E0_19A0);
    }
    if fx.mem.i16(COUNTER) < 0x65 {
        let s1 = if fx.mem.i16(COUNTER) == 0x52 { 5 } else { u64::from(D) };
        for (k, ra) in [0x01E0_19EC, 0x01E0_1A20, 0x01E0_1A54, 0x01E0_1A88].into_iter().enumerate() {
            node(fx, k as u32);
            spawn(fx, SPARK, s1, ra);
        }
        for (k, ra) in [0x01E0_1ABC, 0x01E0_1AF0, 0x01E0_1B24, 0x01E0_1B58].into_iter().enumerate() {
            node(fx, k as u32);
            spawn(fx, DARK, s1, ra);
        }
    }
    let c = fx.mem.i16(COUNTER);
    let s1 = if c == 0x52 { 5 } else { u64::from(D) };
    if (0x22..0x5B).contains(&c) {
        fx.mem.set16(ORBIT_KIND, 0);
        node(fx, 4);
        spawn(fx, ORBIT, s1, 0x01E0_1BB8);
        node(fx, 5);
        spawn(fx, ORBIT, s1, 0x01E0_1BEC);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0x22 || c == 0x5A {
        fx.mem.set16(ORBIT_KIND, 1);
        node(fx, 4);
        for i in 0..10u64 {
            spawn(fx, ORBIT, i, 0x01E0_1C58);
        }
        node(fx, 5);
        for i in 0..10u64 {
            spawn(fx, ORBIT, i, 0x01E0_1CB8);
        }
    }
    fx.leave(0x30);
}

/// The step frame (0x20: ra, the entry's s0 = 0).
fn frame20(fx: &mut Fx) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
}

/// Alpha +0x63: in by `up` under `cap` for `young` updates, then out by
/// `down` (the task ends below `floor`).
fn fade63(fx: &mut Fx, t: u32, young: i32, cap: u8, up: u8, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < young {
        if a < cap {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(up));
        }
    } else if a >= floor {
        fx.mem.write_u8(t + 0x63, a - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E01D00 (D, task): the arc: outer radius +0x4C 70, inner red
/// (0xFF, 0x40, 0x40), outer white fading in by 0x60 under 0xC0 for 11
/// updates, then out by 9; angles 10 / 10; turned by the camera's angles
/// (0x205580 into +0x20, its first + 0x4000).
fn arc(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x428C_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 0xB {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a.wrapping_add(0x60));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x67, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1DC0);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01DF0 (red, size 30) / 0x1E02090 (dark, 50) / 0x1E02630 (pink,
/// 200) (D, task) at `f`: a spark quad: cell row (rand >> 4) % 3 (+ 2 for
/// the last two), drift +0x30..+0x38 in (-50..49) / 60 (0x1E01DF0 moves
/// by it), angle +0x5C random, turned by the camera's angles each update
/// (0x205580 into +0x20; the offsets +0..+4 (+-1024) added first are lost),
/// orientation +0x52 (rand >> 4) % 4; in by 0x40 under 0x80 for 10 (8)
/// updates, out by 9 (4).
fn spark(fx: &mut Fx, t: u32, f: u32) {
    frame20(fx);
    let red = f == SPARK;
    // The `jal`s from +0x5C on sit one word later in the last two.
    let at = |o: u32| if red || o < 0x30 { f + o } else { f + o + 4 };
    if fx.mem.i16(t + 0x58) == 1 {
        let (size, extra, rgb) = match f {
            SPARK => (0x1E, 0, [0xFF, 0x40, 0x40]),
            DARK => (0x32, 2, [0x10, 0x10, 0x10]),
            _ => (0xC8, 2, [0xFF, 0x80, 0x80]),
        };
        fx.mem.set16(t + 8, size);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, at(0x28) + 8);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h + extra);
        l2::rgb(fx, t + 0x60, rgb[0], rgb[1], rgb[2]);
        fx.mem.set16(t + 0x58, 2);
        for (o, site) in [(0x34u32, 0x5Cu32), (0x30, 0x88), (0x38, 0xB4)] {
            let v = rmod(fx, t, at(site), 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, at(0x10C), 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), at(0x130));
        for (o, site) in [(4u32, 0x130u32), (2, 0x150), (0, 0x170)] {
            let v = fx.rand_from(t, at(site) + 8);
            fx.mem.set16(t + o, (v & 0x7FF) - 0x400);
        }
        let v = fx.rand_from(t, at(0x190) + 8);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
    }
    if red {
        fade63(fx, t, 0xA, 0x80, 0x40, 0xB, 9);
        if fx.mem.i16(t + 0x54) != 0 {
            for k in 0..3 {
                let p = ee::add(fx.mem.f(t + 0x10 + 4 * k), fx.mem.f(t + 0x30 + 4 * k));
                fx.mem.setf(t + 0x10 + 4 * k, p);
            }
        }
    } else {
        fade63(fx, t, 8, 0x80, 0x40, 6, 4);
    }
    for (o, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + o).wrapping_add(fx.mem.i16(t + s) as u32);
        fx.mem.write_u32(t + o, v);
    }
    let late = if red { f + 0x274 } else { f + 0x23C };
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), late);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E022F0 (D, task): the red tint's alphas: both up to 0xF0 at once
/// (+0x50 = 1 once the first is there), then down by 14 (the task ends
/// when the first is below 16); colours (0x9F, 0x5F, 0x5F). No age.
fn tint(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x50) == 0 {
        if fx.mem.read_u8(t + 0x63) < 0xF0 {
            fx.mem.write_u8(t + 0x63, 0xF0);
        } else {
            fx.mem.set16(t + 0x50, 1);
        }
        if fx.mem.read_u8(t + 0x67) < 0xF0 {
            fx.mem.write_u8(t + 0x67, 0xF0);
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x67);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x67, a - 0xE);
        }
    }
    l2::rgb(fx, t + 0x60, 0x9F, 0x5F, 0x5F);
    l2::rgb(fx, t + 0x64, 0x9F, 0x5F, 0x5F);
    fx.leave(0x20);
}

/// 0x1E023C0 (D, task): a ring: size +0x48 5..14, cell row random 0..3,
/// grey, angles +0x5A / +0x5C random, orientation +0xA 0..7, +0xC the
/// spawn's +0x50, radius +0x50 (rand >> 4) % 25 + 5 growing by +0xE
/// 11..40, +0x52 300..749, row 0..34, life 5..14; in by 0x30 under 0xC0,
/// out by 8; flickering between (0xFF, 0x10, 0x80) on odd ages and (0xC0,
/// 0x80, 0x80).
fn ring(fx: &mut Fx, t: u32) {
    frame20(fx);
    let f = RING;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x1C, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 5));
        let v = fx.rand_from(t, f + 0x50);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x7C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0x98, 0x168);
        fx.mem.set16(t + 0x5C, v);
        let v = fx.rand_from(t, f + 0xBC);
        fx.mem.set16(t + 0xA, v % 8);
        let w = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, w);
        let v = fx.rand_from(t, f + 0xE0);
        let (_, h) = fx.div(v >> 4, 0x19);
        fx.mem.set16(t + 0x50, h + 5);
        let v = rmod(fx, t, f + 0xFC, 0x1C2);
        fx.mem.set16(t + 0x52, v + 0x12C);
        let v = rmod(fx, t, f + 0x11C, 0x1E);
        fx.mem.set16(t + 0xE, v + 0xB);
        let v = rmod(fx, t, f + 0x13C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0x158, 0xA);
        fx.mem.set16(t + 6, v + 5);
        fx.mem.set16(t + 0x58, 2);
    }
    let life = fx.mem.i16(t + 6);
    fade63(fx, t, life, 0xC0, 0x30, 0xA, 8);
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x60, 0xFF, 0x10, 0x80);
    } else {
        l2::rgb(fx, t + 0x60, 0xC0, 0x80, 0x80);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E02890 (D, task): a white spark orbiting its node (frame 0x30): size
/// 70 (kind D+0xB2B0 1: 100), cell row (rand >> 4) % 3 + 2, angle +0x5A
/// (rand >> 4) % 360, pushed out on the table's sines by 4.6 (8.6), then by
/// 0.8 a update; in by 0x40 under 0x80 for 10 updates, out by 9; turned by
/// the camera's angles.
fn orbit(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    let f = ORBIT;
    let swing = |fx: &mut Fx, k: f32| {
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let k = if k == 0.0 { fx.mem.f(t + 0x48) } else { k };
        let z = ee::add(fx.mem.f(t + 0x18), ee::mul(k, s));
        fx.mem.setf(t + 0x18, z);
        let a = fx.mem.i16(t + 0x5A);
        let c = l2::sin_deg(fx, SINES, a, 1);
        let k2 = if k == 0.0 { fx.mem.f(t + 0x48) } else { k };
        let x = ee::add(fx.mem.f(t + 0x10), ee::mul(k2, c));
        fx.mem.setf(t + 0x10, x);
    };
    if fx.mem.i16(t + 0x58) == 1 {
        for (kind, size) in [(0, 0x46), (1, 0x64)] {
            if fx.mem.i16(ORBIT_KIND) == kind {
                fx.mem.set16(t + 8, size);
                let w = fx.mem.i16(t + 8);
                fx.mem.set16(t + 0xA, w);
            }
        }
        let v = fx.rand_from(t, f + 0x78);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, f + 0xB0);
        let (_, h) = fx.div(v >> 4, 0x168);
        fx.mem.set16(t + 0x5A, h);
        for (kind, r) in [(0, 0x4093_3333u32), (1, 0x4109_999A)] {
            if fx.mem.i16(ORBIT_KIND) == kind {
                fx.mem.write_u32(t + 0x48, r);
            }
        }
        swing(fx, 0.0);
    }
    fade63(fx, t, 0xA, 0x80, 0x40, 0xB, 9);
    if fx.mem.i16(t + 0x54) != 0 {
        swing(fx, f32::from_bits(0x3F4C_CCCD));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2AAC);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E02AD0 (D, task): the frame flash (frame 0x90): a window sprite
/// (0x6FF8 .. 0x8FF8) sampling the frame (0x207EF0) at S 0..1, T +0x48 /
/// 512 .. (512 - +0x48) / 512, grey (+0x60), ALPHA 0x44, key 0; on
/// updates 43..49 red + 1, blue + 2, alpha + 2 a update, +0x48 + 0.3 on
/// 40..49; in by 0x14 under 0x50 while the timer is at 26 or more, then
/// out by 6 (the task ends below 8).
fn flash(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let c = fx.mem.i16(COUNTER);
    if (0x2B..0x32).contains(&c) {
        let r = fx.mem.read_u8(t + 0x60).wrapping_add(1);
        fx.mem.write_u8(t + 0x60, r);
        let b = fx.mem.read_u8(t + 0x62).wrapping_add(2);
        fx.mem.write_u8(t + 0x62, b);
    }
    if (0x28..0x32).contains(&c) {
        let s = ee::add(fx.mem.f(t + 0x48), f32::from_bits(0x3E99_999A));
        fx.mem.setf(t + 0x48, s);
    }
    if (0x2B..0x32).contains(&c) {
        let a = fx.mem.read_u8(t + 0x63).wrapping_add(2);
        fx.mem.write_u8(t + 0x63, a);
    }
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x48 + k, v);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    fx.mem.write_u32(sp + 0x50, 0);
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(s, 512.0));
    fx.mem.write_u32(sp + 0x60, 0x3F80_0000);
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(512.0, s), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i32(TIMER) >= 0x1A {
        if a < 0x50 {
            fx.mem.write_u8(t + 0x63, a + 0x14);
        }
    } else if a >= 8 {
        fx.mem.write_u8(t + 0x63, a - 6);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x90);
}
