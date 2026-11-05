//! BMODULE b433 (card 192), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E039E0, data area 0x1ED0000) [F] (helper A23, 3
//! Nov 2026): b611's task pool grown to 250 (0x70 bytes at +0x2A0, alive
//! +0x58, step +0x44 through `jalr`; 0x1E03710 is skipped and never
//! spawned; spawned by 0x1E04510 with the colour +0xE884 and the slot
//! +0xEBBA into +0x50) on the second library (`uv_rect`, `ring_quads`,
//! `quad`, `arc_strips`, `quad_standing`, `tubetrail::tube` and
//! `quad_b_twice`). The emitter 0x1E01850 counts +0xEBC2: on update 30 a
//! full-window frame tint (0x1E03520); on update 1 the stage light
//! (0x1E03420, down to 0x30 until update 220); on the odd updates
//! 0x72..0x87 at the attacker's node 0x1E (35 up) an orbiting orb
//! (0x1E02350, `quad_b_twice` round its centre +0xE0B0 + 0x10 k, k the
//! slot +0xEBBC cycling 0..119) and its tube (0x1E02D80, a 15-point
//! history +0x7030 + 0xF0 k drawn by `tube`); the attacker's node 0x30 is
//! followed to +0xE830 and slid to the target over updates 0x99..0xA1; on
//! update 0x72 four sparks (0x1E02AC0), on 0xA2 five / seven spark rings
//! (0x1E027F0 / 0x1E02620) and two arcs (0x1E030B0), on the even updates
//! 0xA2..0xCD a ring (0x1E02170) and two spark rings, on 0xA2 and
//! 0x95..0xA2 the coloured arcs (0x1E032E0), every fourth update
//! 0xA2..0xD2 and 0x95..0xA2 a standing post (0x1E031A0), on 0x95..0xA2
//! dots (0x1E029A0 / 0x1E02C40). Returns 1 on update 0x98, 3 on 0xE6, 6
//! on 0xA3..0xE0, 8 once the 241-update timer +0xE880 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 250;
/// The step functions a task holds (+0x44), as the record's addresses.
const RING: u32 = 0x01E0_2170;
const ORB: u32 = 0x01E0_2350;
const SPARKS: u32 = 0x01E0_2620;
const SPARKS_B: u32 = 0x01E0_27F0;
const DOT: u32 = 0x01E0_29A0;
const SPARKS_C: u32 = 0x01E0_2AC0;
const DOT_B: u32 = 0x01E0_2C40;
const TUBE: u32 = 0x01E0_2D80;
const ARC: u32 = 0x01E0_30B0;
const POST: u32 = 0x01E0_31A0;
const ARC_B: u32 = 0x01E0_32E0;
const LIGHT: u32 = 0x01E0_3420;
const FRAME: u32 = 0x01E0_3520;
const SKIPPED: u32 = 0x01E0_3710;
/// The return address of the entry's `jalr` into a step (0x1E03D28 + 8).
const TASK_RA: u64 = 0x01E0_3D30;
const SINES: u32 = D + 0xEDC4;
const CENTRES: u32 = D + 0xE0B0;
const FOLLOW: u32 = D + 0xE830;
const SLOPE_X: u32 = D + 0xE850;
const SLOPE_Z: u32 = D + 0xE858;
const TEX_A: u32 = D + 0xE860;
const TEX_B: u32 = D + 0xE864;
const TIMER: u32 = D + 0xE880;
const COLOUR: u32 = D + 0xE884;
const RADII: u32 = D + 0xE8A4;
const NODE_A: u32 = D + 0xEA9C;
const NODE_B: u32 = D + 0xEA9E;
const ANGLES: u32 = D + 0xEABC;
const SLOT: u32 = D + 0xEBBA;
const NEXT: u32 = D + 0xEBBC;
const COUNTER: u32 = D + 0xEBC2;
const BYTES: u32 = D + 0xEBC4;
const BYTES_B: u32 = D + 0xECC4;

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// rand (the caller's s0 = `s0`) from `ra`, mod `m` (truncated).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// (rand >> 4) mod `m`.
fn rmod4(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v >> 4, m).1
}

/// The alpha byte `a` up by `up.1` while below `up.0` while `rising`, else
/// down by `down.1` while at least `down.0`, else the task ends.
fn fade(fx: &mut Fx, t: u32, a: u32, rising: bool, up: (u8, u8), down: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if rising {
        if v < up.0 {
            fx.mem.write_u8(t + a, v.wrapping_add(up.1));
        }
    } else if v >= down.0 {
        fx.mem.write_u8(t + a, v.wrapping_sub(down.1));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The entry 0x1E039E0 (a0 = D, a1 = param; s0 = D, s1 = 0, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x110);
    fx.mem.write_u32(D + 0xE88C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x110);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xEF30);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xEBC0, param);
    fx.mem.set32(TIMER, 0xF1);
    for (at, n, clear) in [(NODE_A, 1, 1), (NODE_B, 0x30, 0), (D + 0xEAA0, 0x34, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(at, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xEAAA, r);
    for k in 0xAu32..0x10 {
        let tex = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x260 + 4 * k, tex);
    }
    let tex = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, tex);
    for k in 0..2u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX_A + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x100);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0x104)), 180.0);
    fx.fix_residue(v, 0x01E0_3B84);
    fx.mem.set16(D + 0xEBBE, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3BB4, 0x1E);
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(BYTES + k, 0);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3C44, 0x28);
        fx.mem.write_u8(BYTES_B + k, (v - 0x14) as u8);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_3C9C, 0x14);
        fx.mem.write_u8(BYTES_B + k, (v + 5) as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 || f == SKIPPED {
            continue;
        }
        // The steps' prologues: the entry's s0 = D, s1 = 0, s2 = the slot,
        // s3 = k.
        let frame = match f {
            ORB | POST => 0x30,
            TUBE => 0x60,
            FRAME => 0x80,
            RING | SPARKS | SPARKS_B | DOT | SPARKS_C | DOT_B | ARC | ARC_B | LIGHT => 0x20,
            _ => panic!("b433: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match frame {
            0x30 => fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]),
            0x60 => fx.saved(s, &[(0x40, TASK_RA), (0x30, u64::from(k)), (0x20, u64::from(slot)), (0x10, 0), (0, u64::from(D))]),
            _ => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]),
        }
        match f {
            RING => ring(fx, t),
            ORB => orb(fx, t),
            SPARKS => sparks(fx, t),
            SPARKS_B => sparks_b(fx, t),
            DOT => dot(fx, t),
            SPARKS_C => sparks_c(fx, t),
            DOT_B => dot_b(fx, t),
            TUBE => tube_step(fx, s, t),
            ARC => arc(fx, t),
            POST => post(fx, t),
            ARC_B => arc_b(fx, t),
            LIGHT => light(fx, t),
            _ => frame_quad(fx, s, t),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        draw(fx, sp, t);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x98 => 1,
        0xE6 => 3,
        0xA3..=0xE0 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The draw's head: texture `tex` into D+0x88, the atlas cell 0x14 frame
/// +0x56 (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, tex: u32, r: u32) {
    let w = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, w);
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// An orb's point (the slot +0x50's centre plus its radius turned by its
/// angle) into `out`.
fn orb_point(fx: &mut Fx, t: u32, out: u32) {
    let i = fx.mem.i16(t + 0x50) as u32;
    let a = fx.mem.i16(ANGLES + 2 * i);
    let s = l2::sin_deg(fx, SINES, a, 1);
    let v = ee::add(fx.mem.f(CENTRES + 0x10 * i), ee::mul(fx.mem.f(RADII + 4 * i), s));
    fx.mem.setf(out, v);
    let i = fx.mem.i16(t + 0x50) as u32;
    let y = fx.mem.f(CENTRES + 0x10 * i + 4);
    fx.mem.setf(out + 4, y);
    let i = fx.mem.i16(t + 0x50) as u32;
    let a = fx.mem.i16(ANGLES + 2 * i);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let v = ee::add(fx.mem.f(CENTRES + 0x10 * i + 8), ee::mul(fx.mem.f(RADII + 4 * i), s));
    fx.mem.setf(out + 8, v);
}

fn copy3(fx: &mut Fx, from: u32, to: u32) {
    for o in 0..3 {
        let w = fx.mem.read_u32(from + 4 * o);
        fx.mem.write_u32(to + 4 * o, w);
    }
}

/// The entry's draw of one live task by its step.
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    let rings = |fx: &mut Fx, tex: u32, r: u32, n: i32, layout: i32, table: u32| {
        cell(fx, t, tex, r);
        fx.mem.set16(D + 0xB8, n);
        fx.mem.set16(D + 0xB0, layout);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, table);
        fx.mem.set16(D + 0xB8, 0);
        fx.mem.set16(D + 0xB0, 0);
    };
    let quad = |fx: &mut Fx, tex: u32, r: u32| {
        cell(fx, t, tex, r);
        fx.mem.set16(D + 0xBE, 5);
        copy3(fx, t + 0x10, sp + 0x60);
        billboard::quad(fx, uv::uv_rect, D, t, 0x10, sp + 0x60);
    };
    if f == RING {
        cell(fx, t, TEX_B, sp + 0x70);
        fx.mem.set16(D + 0xB8, 5);
        let r = fx.rand_from(D, 0x01E0_3DDC);
        fx.mem.set16(D + 0xBE, r.wrapping_rem(4));
        fx.mem.set16(D + 0xB0, 0);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x10, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == ORB {
        cell(fx, t, TEX_A, sp + 0x80);
        fx.mem.set16(D + 0xBE, 0);
        orb_point(fx, t, sp + 0x60);
        billboard::quad_b_twice(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
    }
    if f == SPARKS {
        rings(fx, TEX_A, sp + 0x90, 0xA, 1, BYTES_B);
    }
    if f == SPARKS_B {
        rings(fx, TEX_A, sp + 0xA0, 0xA, 0, BYTES);
    }
    if f == DOT {
        quad(fx, TEX_B, sp + 0xB0);
    }
    if f == SPARKS_C {
        rings(fx, TEX_B, sp + 0xC0, 0xA, 0, BYTES);
    }
    if f == DOT_B {
        quad(fx, TEX_B, sp + 0xD0);
    }
    if f == TUBE {
        cell(fx, t, TEX_A, sp + 0xE0);
        let v = fx.mem.i16(D + 0x96) + 0xF;
        fx.mem.set16(D + 0x96, v);
        fx.mem.set16(D + 0x98, 0x1F);
        fx.mem.set16(D + 0x9A, 1);
        let h = fx.self_handle();
        unit_position_saving(fx, h, sp + 0x60, u64::from(D), 0x01E0_42A8);
        for o in [0x20u32, 0x24, 0x28] {
            fx.mem.write_u32(D + o, 0);
        }
        fx.mem.set16(D + 0xBE, 2);
        let i = fx.mem.i16(t + 0x50) as u32;
        tubetrail::tube(fx, D, t, D + 0x7030 + 0xF0 * i, 0);
    }
    if f == ARC {
        fx.mem.set16(D + 0xB8, 0x24);
        billboard::arc_strips(fx, D, t, 0x21, FOLLOW);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == POST {
        cell(fx, t, TEX_A, sp + 0xF0);
        let v = fx.mem.i16(D + 0x9A) >> 1;
        fx.mem.set16(D + 0x9A, v);
        fx.mem.set16(D + 0xBE, 0);
        copy3(fx, t + 0x10, sp + 0x60);
        billboard::quad_standing(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
    }
    if f == ARC_B {
        fx.mem.set16(D + 0xB8, 0x24);
        if fx.mem.i16(t + 0x50) == 0 {
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        }
        fx.mem.set16(D + 0xB8, 0);
    }
}

/// The emitter's registers at a call: s0, s1 (s2 = D; s3 / s4 the
/// runner's, 0 [H]).
#[derive(Clone, Copy)]
struct Regs(u32, u32);

/// 0x1E04510(D, step, 0): the first free of the 250 tasks cleared and set
/// off at D+0x250 (as i16 and its xyz at +0x10) with the colour +0xE884
/// (+0x60, +0x64), +0x50 = +0xEBBA; 1 when one was free.
fn spawn(fx: &mut Fx, step: u32, r: Regs, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, u64::from(r.1)), (0, u64::from(r.0))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for o in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            copy3(fx, D + 0x250, t + 0x10);
            let s = fx.mem.i16(SLOT);
            fx.mem.set16(t + 0x50, s);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// +0xEBBC up by one, back to 0 at 120.
fn next_slot(fx: &mut Fx) {
    let n = fx.mem.i16(NEXT) + 1;
    fx.mem.set16(NEXT, n);
    if fx.mem.i16(NEXT) >= 0x78 {
        fx.mem.set16(NEXT, 0);
    }
}

fn odd(n: i32) -> bool {
    n % 2 == 1
}

/// 0x1E01850(D, 0) (frame 0x50: ra, the entry's s2 = param, s1 = 0, s0 =
/// D): the update counter +0xEBC2 and the emitters.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x30, 0x01E0_3CF0), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    let mut r = Regs(D, 0);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 0x1E {
        fx.mem.set16(SLOT, 5);
        spawn(fx, FRAME, r, 0x01E0_18B8);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x72..0x88).contains(&c) && odd(c) {
        let n = if c == 0x9A { 0x1E } else { 1 };
        r = Regs(0, n as u32);
        for k in 0..n {
            r.0 = k as u32;
            let h = fx.self_handle();
            let s = fx.mem.i16(NODE_A);
            node_position_saving(fx, h, s, D + 0x250, u64::from(r.0), Some(u64::from(r.1)), 0x01E0_193C);
            fx.mem.write_u32(D + 0x254, 0x420C_0000);
            let v = fx.mem.i16(NEXT);
            fx.mem.set16(SLOT, v);
            spawn(fx, ORB, r, 0x01E0_1970);
            spawn(fx, TUBE, r, 0x01E0_1984);
            next_slot(fx);
        }
        r.0 = n as u32;
    }
    let h = fx.self_handle();
    let s = fx.mem.i16(NODE_A);
    node_position_saving(fx, h, s, D + 0x250, u64::from(r.0), Some(u64::from(r.1)), 0x01E0_1A10);
    for (o, ra, m, b) in [(0u32, 0x01E0_1A18u32, 0x50, -0x28), (8, 0x01E0_1A4C, 0x50, -0x28), (4, 0x01E0_1A80, 0x96, 0x64)] {
        let v = rmod(fx, r.0, ra, m) + b;
        let p = ee::add(fx.mem.f(D + 0x250 + o), ee::from_i32(v));
        fx.mem.setf(D + 0x250 + o, p);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x34..0x9A).contains(&c) && odd(c) {
        // The loop's spawn is compiled out: the slot only runs on.
        for _ in 0..2 {
            let v = fx.mem.i16(NEXT);
            fx.mem.set16(SLOT, v);
            next_slot(fx);
        }
    }
    if fx.mem.i16(COUNTER) == 0x3B {
        fx.mem.set16(SLOT, 1);
        fx.mem.set16(SLOT, 2);
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, LIGHT, r, 0x01E0_1C0C);
    }
    let c = fx.mem.i16(COUNTER);
    if c < 0x99 {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE_B);
        node_position_saving(fx, h, s, FOLLOW, u64::from(r.0), Some(u64::from(r.1)), 0x01E0_1C48);
    } else {
        if c == 0x99 {
            let h = fx.target_handle();
            unit_position_saving(fx, h, sp + 0x40, u64::from(r.0), 0x01E0_1C70);
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x40), fx.mem.f(FOLLOW)), 8.0);
            fx.mem.setf(SLOPE_X, v);
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(FOLLOW + 8)), 8.0);
            fx.mem.setf(SLOPE_Z, v);
        }
        if fx.mem.i16(COUNTER) < 0xA2 {
            let v = ee::add(fx.mem.f(FOLLOW + 8), fx.mem.f(SLOPE_Z));
            fx.mem.setf(FOLLOW + 8, v);
            let v = ee::add(fx.mem.f(FOLLOW), fx.mem.f(SLOPE_X));
            fx.mem.setf(FOLLOW, v);
        }
    }
    for o in 0..4 {
        let w = fx.mem.read_u32(FOLLOW + 4 * o);
        fx.mem.write_u32(D + 0x250 + 4 * o, w);
    }
    if fx.mem.i16(COUNTER) == 0xA2 {
        let mut ra = 0x01E0_1D7Cu32;
        for f in [SPARKS_B; 5].into_iter().chain([SPARKS; 7]).chain([ARC; 2]) {
            spawn(fx, f, r, ra);
            ra += 0x14;
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0xA2..0xCE).contains(&c) && !odd(c) {
        spawn(fx, RING, r, 0x01E0_1ED8);
        spawn(fx, SPARKS, r, 0x01E0_1EEC);
        spawn(fx, SPARKS_B, r, 0x01E0_1F00);
    }
    if fx.mem.i16(COUNTER) == 0xA2 {
        fx.mem.set16(SLOT, 1);
        spawn(fx, ARC_B, r, 0x01E0_1F3C);
        spawn(fx, ARC_B, r, 0x01E0_1F50);
    }
    let c = fx.mem.i16(COUNTER);
    if (0xA2..0xD3).contains(&c) && c % 4 == 1 {
        spawn(fx, POST, r, 0x01E0_1FA8);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x95..0xA3).contains(&c) {
        spawn(fx, DOT, r, 0x01E0_1FE0);
        fx.mem.set16(SLOT, 0);
        spawn(fx, DOT_B, r, 0x01E0_2000);
        fx.mem.set16(SLOT, 1);
        spawn(fx, DOT_B, r, 0x01E0_2024);
        fx.mem.write_u32(D + 0x254, 0x420C_0000);
        for (o, ra) in [(0u32, 0x01E0_2030u32), (8, 0x01E0_2064)] {
            let v = rmod(fx, r.0, ra, 0xA) - 5;
            let p = ee::add(fx.mem.f(D + 0x250 + o), ee::from_i32(v));
            fx.mem.setf(D + 0x250 + o, p);
        }
        spawn(fx, POST, r, 0x01E0_20A0);
        fx.mem.set16(SLOT, 0);
        spawn(fx, ARC_B, r, 0x01E0_20C0);
    }
    for o in 0..4 {
        let w = fx.mem.read_u32(FOLLOW + 4 * o);
        fx.mem.write_u32(D + 0x250 + 4 * o, w);
    }
    if fx.mem.i16(COUNTER) == 0x72 {
        fx.mem.write_u32(D + 0x254, 0x420C_0000);
        let mut ra = 0x01E0_211Cu32;
        for _ in 0..4 {
            spawn(fx, SPARKS_C, r, ra);
            ra += 0x14;
        }
    }
    fx.leave(0x50);
}

/// 0x1E02170: a ring (ring_quads of 5 from the bytes +0xEBC4, cell frame
/// rand % 8) of size 100..129 growing by 6 to age 140, radius 100..149
/// growing by 26..45 to age 90, pink (0xFF, 0xA0, 0xA0); in by 0x2A below
/// 0x80 for 7..20 updates, then out by 9.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2194, 0x1E);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x64));
        let v = fx.rand_from(t, 0x01E0_21C0);
        fx.mem.set16(t + 0x56, v.wrapping_rem(8));
        l2::rgb(fx, t + 0x60, 0xFF, 0xA0, 0xA0);
        let v = rmod(fx, t, 0x01E0_21F4, 0x32);
        fx.mem.set16(t + 0x50, v + 0x64);
        let v = rmod(fx, t, 0x01E0_2214, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2230, 0x14);
        fx.mem.set16(t + 0xE, v + 0x1A);
        let v = rmod(fx, t, 0x01E0_2250, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_226C, 0xE);
        fx.mem.set16(t + 6, v + 7);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, 0x63, rising, (0x80, 0x2A), (0xB, 9));
    let n = fx.mem.i16(t + 0x54);
    if n < 0x5A && n != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    if fx.mem.i16(t + 0x54) < 0x8C {
        let v = ee::add(fx.mem.f(t + 0x48), 6.0);
        fx.mem.setf(t + 0x48, v);
    }
    age(fx, t);
}

/// 0x1E02350 (frame 0x30: s1 = D): an orb at its slot's centre (+0x10..
/// into +0xE0B0 + 0x10 k), the slot's angle (rand >> 4) % 360 turning by
/// -18 a update and radius ((rand >> 4) % 320 + 50) / 10; size 15, grey,
/// flickering white; in by 0x2A for 48 updates, then out by 14; it ends
/// from update 171.
fn orb(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xF);
        fx.mem.set16(t + 0xA, 0xF);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        for o in 0..3 {
            let i = fx.mem.i16(t + 0x50) as u32;
            let w = fx.mem.read_u32(t + 0x10 + 4 * o);
            fx.mem.write_u32(CENTRES + 0x10 * i + 4 * o, w);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod4(fx, t, 0x01E0_2414, 0x168);
        let i = fx.mem.i16(t + 0x50) as u32;
        fx.mem.set16(ANGLES + 2 * i, v);
        let v = rmod4(fx, t, 0x01E0_2440, 0x140);
        let i = fx.mem.i16(t + 0x50) as u32;
        fx.mem.setf(RADII + 4 * i, ee::from_i32(v + 0x32));
        let i = fx.mem.i16(t + 0x50) as u32;
        let r = ee::div(fx.mem.f(RADII + 4 * i), 10.0);
        fx.mem.setf(RADII + 4 * i, r);
        let v = rmod4(fx, t, 0x01E0_24A8, 0xE);
        fx.mem.set16(t + 0x5C, v + 4);
    }
    fade2(fx, t, 0x30);
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    } else {
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_25A4);
    let v = fx.mem.i16(t + 0x5A) + 5;
    fx.mem.set16(t + 0x5A, v);
    let i = fx.mem.i16(t + 0x50) as u32;
    let a = fx.mem.i16(ANGLES + 2 * i) - 0x12;
    fx.mem.set16(ANGLES + 2 * i, a);
    age(fx, t);
    if fx.mem.i16(COUNTER) >= 0xAB {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// Both alphas (+0x63 / +0x67) in by 0x2A below 0x80 for `n` updates,
/// then out by 14 (the task ending once +0x63 is below 16).
fn fade2(fx: &mut Fx, t: u32, n: i32) {
    if fx.mem.i16(t + 0x54) < n {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
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
}

/// The spark rings' common update: alpha 0x80 for +6 updates, then out by
/// 11; +0x52 counts, the radius +0x50 grows by +0xE.
fn spark_tail(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xD {
            fx.mem.write_u8(t + 0x63, a - 0xB);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, v);
    age(fx, t);
}

/// 0x1E02620: a spark ring (size 10..19, cell rand % 4, red-white), turned
/// by +0x24 rand % 0xFFFF, radius 100..199 growing 10..24, row 1..8, held
/// 13..22 updates.
fn sparks(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2644, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = fx.rand_from(t, 0x01E0_2670);
        fx.mem.set16(t + 0x56, v.wrapping_rem(4));
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_26A4, 0xFFFF);
        fx.mem.set32(t + 0x24, v);
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_26C8, 0x64);
        fx.mem.set16(t + 0x50, v + 0x64);
        let v = rmod(fx, t, 0x01E0_26E8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2704, 0xF);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = fx.rand_from(t, 0x01E0_2724);
        fx.mem.set16(t + 8, v.wrapping_rem(8) + 1);
        let v = rmod(fx, t, 0x01E0_2744, 0xA);
        fx.mem.set16(t + 6, v + 0xD);
        fx.mem.set16(t + 0x58, 2);
    }
    spark_tail(fx, t);
}

/// 0x1E027F0: [`sparks`] untilted, radius 10..29, its angle (rand >> 4) %
/// 360.
fn sparks_b(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2814, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = fx.rand_from(t, 0x01E0_2840);
        fx.mem.set16(t + 0x56, v.wrapping_rem(4));
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_2878, 0x14);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod4(fx, t, 0x01E0_2898, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_28B8, 0xF);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = fx.rand_from(t, 0x01E0_28D8);
        fx.mem.set16(t + 8, v.wrapping_rem(8) + 1);
        let v = rmod(fx, t, 0x01E0_28F8, 0xA);
        fx.mem.set16(t + 6, v + 0xD);
        fx.mem.set16(t + 0x58, 2);
    }
    spark_tail(fx, t);
}

/// 0x1E02AC0: [`sparks_b`] of size 70, cell rand % 8, held 6..10 updates.
fn sparks_c(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x428C_0000);
        let v = fx.rand_from(t, 0x01E0_2AE8);
        fx.mem.set16(t + 0x56, v.wrapping_rem(8));
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_2B20, 0x14);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod4(fx, t, 0x01E0_2B40, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2B60, 0xF);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = fx.rand_from(t, 0x01E0_2B80);
        fx.mem.set16(t + 8, v.wrapping_rem(8) + 1);
        let v = rmod(fx, t, 0x01E0_2BA0, 5);
        fx.mem.set16(t + 6, v + 6);
        fx.mem.set16(t + 0x58, 2);
    }
    spark_tail(fx, t);
}

/// The dots' first update: size 70, cell rand % 6 + 2, +0x34 0.5, pink
/// (0xFF, 0xA0, 0xAF).
fn dot_head(fx: &mut Fx, t: u32, ra: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x46);
        fx.mem.set16(t + 0xA, 0x46);
        let v = rmod(fx, t, ra, 6);
        fx.mem.set16(t + 0x56, v + 2);
        fx.mem.write_u32(t + 0x34, 0x3F00_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xA0, 0xAF);
        fx.mem.set16(t + 0x58, 2);
    }
}

/// 0x1E029A0: a dot growing to 90, in by 0x60 for 20 updates, then out by
/// 9; facing the camera.
fn dot(fx: &mut Fx, t: u32) {
    dot_head(fx, t, 0x01E0_29CC);
    let rising = fx.mem.i16(t + 0x54) < 0x14;
    fade(fx, t, 0x63, rising, (0xC0, 0x60), (0xB, 9));
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s < 0x5A {
                fx.mem.set16(t + o, s + 1);
            }
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2A98);
    age(fx, t);
}

/// 0x1E02C40: a dot in for 3 updates, growing by 1, drifting 0.5 along x
/// (+ for slot 0, - else).
fn dot_b(fx: &mut Fx, t: u32) {
    dot_head(fx, t, 0x01E0_2C6C);
    let rising = fx.mem.i16(t + 0x54) < 3;
    fade(fx, t, 0x63, rising, (0xC0, 0x60), (0xB, 9));
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o) + 1;
            fx.mem.set16(t + o, s);
        }
    }
    let x = fx.mem.f(t + 0x10);
    let x = if fx.mem.i16(t + 0x50) == 0 { ee::add(x, 0.5) } else { ee::sub(x, 0.5) };
    fx.mem.setf(t + 0x10, x);
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2D5C);
    age(fx, t);
}

/// 0x1E02D80 (frame 0x60, the point at sp + 0x50): the orb's tube: its
/// slot's orb point pushed into the 15-point history +0x7030 + 0xF0 k
/// (filled with it on the first update) until age 117, white, 2 points
/// growing to 8, size 7, cell 4; both alphas as the orb's; turning by 5
/// until age 83; it ends from update 171.
fn tube_step(fx: &mut Fx, sp: u32, t: u32) {
    let p = sp + 0x50;
    orb_point(fx, t, p);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        let i = fx.mem.i16(t + 0x50) as u32;
        let trail = D + 0x7030 + 0xF0 * i;
        for j in 0..0xFu32 {
            copy3(fx, p, trail + 0x10 * j);
        }
        fx.mem.set16(t + 6, 2);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0x56, 4);
        fx.mem.write_u32(t + 0x48, 0x40E0_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    fade2(fx, t, 0x30);
    if fx.mem.i16(t + 0x54) < 0x53 {
        let v = fx.mem.i16(t + 0x5A) + 5;
        fx.mem.set16(t + 0x5A, v);
    }
    if fx.mem.i16(t + 0x54) < 0x75 {
        let i = fx.mem.i16(t + 0x50) as u32;
        crate::blib::spawn::history(fx, D + 0x7110 + 0xF0 * i, 0xF, p);
    }
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    } else {
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
    }
    if fx.mem.i16(t + 0x54) != 0 && fx.mem.i16(t + 6) < 8 {
        let v = fx.mem.i16(t + 6) + 1;
        fx.mem.set16(t + 6, v);
    }
    age(fx, t);
    if fx.mem.i16(COUNTER) >= 0xAB {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E030B0: an arc ring (arc_strips at +0xE830, 36 strips) of outer
/// radius 200, pink (0xFF, 0xC0, 0xC0), the outer alpha in by 0x20 for 12
/// updates, then out by 11; facing the camera, spun by 0x4000 a update.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4348_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xC0, 0xC0);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 0xC;
    fade(fx, t, 0x67, rising, (0x80, 0x20), (0xD, 0xB));
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_3170);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E031A0 (frame 0x30: s1 = D): a standing post (35 x 70, cell 4,
/// pink), its height swaying by 20 sin(+0x5A) (+0x5A up by 15 to 180), in
/// by 0x60 for 10 updates, then out by 15; facing the camera.
fn post(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x23);
        fx.mem.set16(t + 0xA, 0x46);
        fx.mem.set16(t + 0x56, 4);
        fx.mem.write_u32(t + 0x34, 0x3F00_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xA0, 0xAF);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < 0xA;
    fade(fx, t, 0x63, rising, (0xC0, 0x60), (0x11, 0xF));
    if fx.mem.i16(t + 0x54) != 0 {
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + 0xA)), ee::mul(20.0, s));
        fx.fix_residue(v, 0x01E0_3290);
        fx.mem.set16(t + 0xA, ee::fixsfsi(v));
    }
    let a = fx.mem.i16(t + 0x5A);
    if a < 0xB4 {
        fx.mem.set16(t + 0x5A, a + 0xF);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_32B8);
    age(fx, t);
}

/// 0x1E032E0: an arc ring at its own point: slot 0 grey / red (0xC0,
/// 0x40, 0x40) of radius 150 held 20 updates, slot 1 white / (0xC0, 0x70,
/// 0x70) of radius 100 held 62; the outer alpha in by 0x50 below 0xA0,
/// then out by 12; facing the camera.
fn arc_b(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
            l2::rgb(fx, t + 0x64, 0xC0, 0x40, 0x40);
            fx.mem.set16(t + 6, 0x14);
            fx.mem.write_u32(t + 0x4C, 0x4316_0000);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
            l2::rgb(fx, t + 0x64, 0xC0, 0x70, 0x70);
            fx.mem.set16(t + 6, 0x3E);
            fx.mem.write_u32(t + 0x4C, 0x42C8_0000);
        }
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, 0x67, rising, (0xA0, 0x50), (0xE, 0xC));
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_3400);
    age(fx, t);
}

/// 0x1E03420: the stage light +0x50 from 0x80 down by 10 to 0x30 until
/// update 220, then back up by 10 to 0x80; at the timer's 3 the light 0x80
/// and the task ends.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let l = fx.mem.i16(t + 0x50);
    if fx.mem.i16(COUNTER) < 0xDC {
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    age(fx, t);
    if fx.mem.read_u32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
}

/// 0x1E03520 (frame 0x80, the descriptor at sp + 0x28): a full-window
/// frame quad (colour 0xA0 / 0x78 / 0x78, the task's alpha, ALPHA 0x44),
/// its UVs from +0x50 / 512 to 1 (+0x50 from the spawn's 5 up by 1 to
/// 30); in by 12 below 0x60 for 65 updates, then out by 12.
fn frame_quad(fx: &mut Fx, sp: u32, t: u32) {
    let d = sp + 0x28;
    let u = ee::from_i32(fx.mem.i16(t + 0x50));
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.mem.read_u8(t + 0x63);
    for (k, v) in [0xA0u8, 0x78, 0x78, a].into_iter().enumerate() {
        fx.mem.write_u8(d + 0x10 + k as u32, v);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, xy) in [(0x20u32, 0x6FF8u16), (0x30, 0x8FF8)] {
        fx.mem.write_u16(d + at, xy);
        fx.mem.write_u16(d + at + 2, xy);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    let lo = ee::div(u, 512.0);
    fx.mem.setf(d + 0x18, lo);
    fx.mem.setf(d + 0x1C, lo);
    fx.mem.write_u32(d + 0x28, 0x3F80_0000);
    fx.mem.write_u32(d + 0x2C, 0x3F80_0000);
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    // After 0x207E20: a2 = the flags | 0x200, a3 = the flags (as b362).
    fx.frame_desc(d, 1, [0x226, 0x26]);
    // 0x207FC0's `sq ra` upper half (0) under 0x207EF0 (as b232).
    fx.mem.write_u64(sp - 0x28, 0);
    fx.submit(d, OT, 0);
    let rising = fx.mem.i16(t + 0x54) < 0x41;
    fade(fx, t, 0x63, rising, (0x60, 0xC), (0xE, 0xC));
    age(fx, t);
    let v = fx.mem.i16(t + 0x50);
    if v < 0x1E {
        fx.mem.set16(t + 0x50, v + 1);
    }
}
