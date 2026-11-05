//! BMODULE b506 (card 468), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01B90, data area a0 = 0x1ED0000, 0xC2A0 bytes)
//! [F]: b580's (card 288) rising-trail program
//! re-laid: 30 trail records (0x98 at D+0x2D0) on ten points D+0xAE70 at
//! the attacker's node 0x13 (D+0xAE20, kept at D+0xAF10 below update
//! 0x58), drawn turned by the facing through five scales (0.5 / 0.4 / 0.3
//! / 0.2 / 0.15, 1.5 high, z turned by 0x2000 each), twice a slice
//! (texture 4 / 15 rects 0x97 and 0x96, corner order by slice parity);
//! 350 tasks (0x70 at D+0x14A0): a frame tint (0x1E019C0) spawned on 0x46,
//! grey smoke (0x1E01240), three tints at the target's point (0x1E01540,
//! quads4::tint4) spawned on 0x57, sparks (0x1E016D0, five a update on
//! 0x46..0x56, twelve on 0x57..0x73). Returns 1 / 4 / 2 / 6 / 8 on 0x4D /
//! 0x87 / 0x64 / 0x58..0x60 / timer out (0xAA).
use crate::b076::{node_position_saving, request_node_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TRAILS: u32 = 30;
const TASKS: u32 = 350;
const TASK0: u32 = D + 0x14A0;
/// The step functions a task holds (+0x44), as the record's addresses.
const SMOKE: u32 = 0x01E0_1240;
const GLOW: u32 = 0x01E0_1540;
const SPARK: u32 = 0x01E0_16D0;
const TINT: u32 = 0x01E0_19C0;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_1E84;
const NODE13: u32 = D + 0xAE20;
const POINTS: u32 = D + 0xAE70;
const NODE13_KEPT: u32 = D + 0xAF10;
const TEX5: u32 = D + 0xBF80;
const TIMER: u32 = D + 0xBFA0;
const COLOUR: u32 = D + 0xBFA4;
const OTW: u32 = D + 0xBFAC;
const LAST: u32 = D + 0xBFB0;
const RADII: u32 = D + 0xBFB4;
const HEIGHT: u32 = D + 0xBFE0;
const TINT_W: u32 = D + 0xBFE4;
const MEAN_W: u32 = D + 0xBFE8;
const ANGLES: u32 = D + 0xBFF8;
const NODES: u32 = D + 0xC00C;
const REC_ANGLE: u32 = D + 0xC020;
const INDEX: u32 = D + 0xC026;
const FACING: u32 = D + 0xC02A;
const COUNTER: u32 = D + 0xC02C;
const BYTES: u32 = D + 0xC02E;
const SINES: u32 = D + 0xC130;

/// The entry 0x1E01B90 (a0 = D, a1 = param; frame 0x130).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x130);
    fx.mem.write_u32(OTW, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x130);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let p = param as i64 as u64;
    let d = u64::from(D);
    fx.bzero(D, 0xC2A0);
    l2::sine_table_saving(fx, SINES, d, 0, 0x01E0_1BEC);
    l2::sine_table_saving(fx, D + 0xD8, d, 0, 0x01E0_1BF4);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2C8 + 4 * (k as u32 - 0xE), t);
    }
    let t = fx.texture(5, 0);
    fx.mem.write_u32(TEX5, t);
    let s = [Some(d), Some(d + 4), Some(p)];
    for (k, (node, clear)) in [(0x13, 1), (0x81, 0), (0x24, 0), (0x50, 0), (0xE, 0), (0xF, 0), (0x23, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = request_node_saving(fx, h, node, clear, s, 0x01E0_1C60 + 0x20 * k as u32);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.fix_residue(y, 0x01E0_1D64);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    fx.mem.set16(D + 0xC028, param);
    fx.mem.set32(TIMER, 0xAA);
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_1D98);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TRAILS {
        let r = D + 0x2D0 + 0x98 * k;
        if fx.mem.i16(r + 0x88) != 0 {
            trail_step(fx, r, k);
        }
    }
    // The entry's s1: the live trails drawn.
    let mut s1 = 0u64;
    for k in 0..TRAILS {
        let r = D + 0x2D0 + 0x98 * k;
        if fx.mem.i16(r + 0x88) != 0 {
            s1 += 1;
            trail_draw(fx, r, sp + 0xA0, 0, k, s1);
            trail_draw(fx, r, sp + 0xA0, 1, k, s1);
        }
    }
    for k in 0..TASKS {
        let t = TASK0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x44) {
            0 => {}
            SMOKE => smoke_step(fx, t, s1),
            GLOW => glow_step(fx, t),
            SPARK => spark_step(fx, t, s1),
            TINT => tint_step(fx, t, s1),
            f => panic!("b506: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = TASK0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == SMOKE {
            let w = fx.mem.read_u32(D + 0x2CC);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x9B, sub);
            billboard::quad(fx, billboard::uv_rect_q, D, t, 0x20, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == GLOW {
            let h = fx.target_handle();
            fx.unit_position(h, sp + 0x60);
            // 0x1F71A0's frame below the entry's (its saved ra, s0).
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_1F40), (0, u64::from(D))]);
            let z = fx.mem.f(sp + 0x68);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 30.0) } else { ee::add(z, 30.0) };
            fx.mem.setf(sp + 0x68, z);
            let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
            let mut x = fx.mem.read_u64(sp + 0xB8);
            if let Some(w) = fx.project(p, None, &mut x) {
                fx.mem.write_u64(sp + 0xB8, x);
                fx.mem.setf(TINT_W, w);
            }
            crate::b590::project_residue(fx, sp, sp + 0x60);
            let w = fx.mem.f(TINT_W);
            quads4::tint4(fx, t, 0x20, w);
        }
        if fx.mem.read_u32(t + 0x44) == SPARK {
            let w = fx.mem.read_u32(TEX5);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x14, sub);
            billboard::quad(fx, billboard::uv_rect_q, D, t, 0x20, t + 0x10);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x4D => 1,
        0x87 => 4,
        0x64 => 2,
        0x58..=0x60 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x106960(r, id, sub) and its 16 bytes copied to D+0x94.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E00960(D, 0): the update counter and the emitter (frame 0x70: ra,
/// the runner's s4 / s3 (0), the entry's s2 = the parameter, s1 = 0, s0
/// = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x70);
    let p = param as i64 as u64;
    let d = u64::from(D);
    fx.saved(sp, &[(0x50, 0x01E0_1DD8), (0x40, 0), (0x30, 0), (0x20, p), (0x10, 0), (0, d)]);
    // The emitter's s0 / s1 at its calls.
    let (mut s0, mut s1) = (d, 0u64);
    for (k, v) in [0x80u8, 0, 0, 0].into_iter().enumerate() {
        fx.mem.write_u8(COLOUR + k as u32, v);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 0x46 {
        spawn_task(fx, TINT, 0, [d, 0, p, 0, d], 0x01E0_09F4);
    }
    if fx.mem.i16(COUNTER) == 0x57 {
        fx.mem.set16(INDEX, 0);
        spawn_task(fx, GLOW, 0, [d, 0, p, 0, d], 0x01E0_0A2C);
    }
    if fx.mem.i16(COUNTER) == 0x46 {
        for k in 0..10u32 {
            fx.mem.setf(POINTS + 4 + 0x10 * k, ee::from_i32(-0x1E + 0x11 * k as i32));
        }
        fx.mem.set32(RADII, 0);
        for k in 1..10u32 {
            fx.mem.write_u32(RADII + 4 * k, 0x4040_0000);
        }
        for k in 0..10u32 {
            let v = fx.rand_from(D + 2 * k, 0x01E0_0ABC);
            let (_, h) = fx.div(v, 0x168);
            fx.mem.set16(ANGLES + 2 * k, h);
        }
        for k in 0..3 {
            fx.mem.write_u8(COLOUR + k, 0x20);
        }
        fx.mem.set32(D + 0x250, 0);
        fx.mem.write_u32(D + 0x254, 0x41F0_0000);
        fx.mem.set32(D + 0x258, 0);
        fx.mem.set16(D + 0xC022, 1);
        fx.mem.set32(HEIGHT, 0);
        fx.mem.set16(INDEX, 1);
        fx.mem.set16(REC_ANGLE, 0);
        spawn_trail(fx, [0, 10, p], 0x01E0_0B68);
        s0 = 1;
        s1 = 10;
    }
    let h = fx.target_handle();
    let slot = fx.mem.i16(NODES + 2);
    node_position_saving(fx, h, slot, sp + 0x60, s0, Some(s1), 0x01E0_0B98);
    for k in 0..10u32 {
        let a = fx.mem.i16(ANGLES + 2 * k);
        let c = l2::sin_deg(fx, SINES, a, 1);
        let r = fx.mem.f(RADII + 4 * k);
        fx.mem.setf(POINTS + 0x10 * k, ee::mul(r, c));
        let a = fx.mem.i16(ANGLES + 2 * k);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let r = fx.mem.f(RADII + 4 * k);
        fx.mem.setf(POINTS + 8 + 0x10 * k, ee::mul(r, s));
        let a = fx.mem.i16(ANGLES + 2 * k) + 4;
        fx.mem.set16(ANGLES + 2 * k, a);
        let a = fx.mem.i16(ANGLES + 2 * k);
        if a >= 0x168 {
            let (_, h) = fx.div(a, 0x168);
            fx.mem.set16(ANGLES + 2 * k, h);
        }
    }
    let (s0, s1) = (d + 0x14, d + 0x28);
    if fx.mem.i16(COUNTER) < 0x58 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, NODE13, s0, Some(s1), 0x01E0_0D3C);
        for k in 0..4 {
            let w = fx.mem.read_u32(NODE13 + 4 * k);
            fx.mem.write_u32(NODE13_KEPT + 4 * k, w);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x46..0x74).contains(&c) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, D + 0x250, s0, Some(s1), 0x01E0_0DB0);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x46..0x74).contains(&c) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, D + 0x250, s0, Some(s1), 0x01E0_0DF4);
        let n = if fx.mem.i16(COUNTER) < 0x57 { 5u64 } else { 0xC };
        for k in 0..n {
            spawn_task(fx, SPARK, 0, [n, k, d + 0xA0, 10, d], 0x01E0_0E38);
        }
    }
    fx.leave(0x70);
}

/// 0x1E022B0(D, step, arg): a new task (the first free of 350) with the
/// colour D+0xBFA4 (both +0x60 / +0x64), at D+0x250, index D+0xC026 in
/// +0x50 / +0x52; 1, or 0 (frame 0x60: the caller's s0..s4 and `ra`).
fn spawn_task(fx: &mut Fx, step: u32, arg: u32, s: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, s[4]), (0x30, s[3]), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(TASK0 + 0x58 + 0x70 * k) == 0) else {
        fx.leave(0x60);
        return 0;
    };
    let t = TASK0 + 0x70 * k;
    fx.bzero(t, 0x70);
    fx.mem.set16(t + 0x58, 1);
    for o in [0x60u32, 0x64] {
        let (r, g, b) = (fx.mem.u8(COLOUR), fx.mem.u8(COLOUR + 1), fx.mem.u8(COLOUR + 2));
        l2::rgb(fx, t + o, r, g, b);
        let a = fx.mem.read_u8(COLOUR + 3);
        fx.mem.write_u8(t + o + 3, a);
    }
    l2::to_i16x4(fx, t, D + 0x250, 0);
    for k in 0..3 {
        let w = fx.mem.read_u32(D + 0x250 + 4 * k);
        fx.mem.write_u32(t + 0x10 + 4 * k, w);
    }
    let v = fx.mem.i16(INDEX);
    fx.mem.set16(t + 0x50, v);
    let v = fx.mem.i16(INDEX);
    fx.mem.set16(t + 0x52, v);
    fx.mem.write_u32(t + 0x40, arg);
    fx.mem.write_u32(t + 0x44, step);
    fx.leave(0x60);
    1
}

/// 0x1E02110(D, step, 0): a new trail record (the first free of 30) at
/// D+0x250 following D+0xBFB0 (angle D+0xC020, index D+0xC026, height
/// D+0xBFE0); 1, or 0 when all are in use (frame 0x40: the caller's s0..s2,
/// `ra`).
fn spawn_trail(fx: &mut Fx, s: [u64; 3], ra: u32) -> i32 {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, u64::from(ra)), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    let Some(k) = (0..TRAILS).find(|&k| fx.mem.i16(D + 0x358 + 0x98 * k) == 0) else {
        fx.leave(0x40);
        return 0;
    };
    let rec = D + 0x2D0 + 0x98 * k;
    fx.bzero(rec, 0x98);
    fx.mem.set16(rec + 0x88, 1);
    for k in 0..4 {
        let w = fx.mem.read_u32(D + 0x250 + 4 * k);
        fx.mem.write_u32(rec + 0x30 + 4 * k, w);
    }
    let v = fx.mem.i16(REC_ANGLE);
    fx.mem.set16(rec + 0x92, v);
    let prev = fx.mem.read_u32(LAST);
    fx.mem.write_u32(rec + 0x70, prev);
    let v = fx.mem.i16(INDEX);
    fx.mem.set16(rec + 0x8E, v);
    let h = fx.mem.f(HEIGHT);
    fx.mem.setf(rec + 4, h);
    fx.mem.setf(rec + 0x14, ee::sub(h, 18.0));
    let prev = fx.mem.read_u32(LAST);
    fx.mem.write_u32(rec + 0x70, prev);
    if prev != 0 {
        fx.mem.write_u32(prev + 0x74, rec);
    }
    fx.mem.set16(rec + 0x94, 1);
    fx.mem.set16(rec + 0x96, 1);
    fx.mem.write_u32(LAST, rec);
    fx.leave(0x40);
    1
}

/// 0x1E00E70(D, rec): c061's trail step on the ten points D+0xAE70 with
/// (38, 38, 48) colours, the alpha in by 10 under 0x50 below 100, else out
/// by 14; widths 10 s0 / 9 s0; the next record spawned before update 0x73
/// (frame 0x90: ra, the runner's s4, the entry's s3 = k, s2 = D + 0x98 k,
/// s1 = 0, s0 = D).
fn trail_step(fx: &mut Fx, rec: u32, k: u32) {
    let sp = fx.enter(0x90);
    let d = u64::from(D);
    fx.saved(sp, &[(0x50, 0x01E0_1DF8), (0x40, 0), (0x30, u64::from(k)), (0x20, d + 0x98 * u64::from(k)), (0x10, 0), (0, d)]);
    fx.mem.set32(sp + 0x64, 0);
    l2::rgb(fx, rec + 0x80, 0x38, 0x38, 0x48);
    l2::rgb(fx, rec + 0x84, 0x38, 0x38, 0x48);
    for k in 0..3 {
        fx.mem.set32(rec + 0x30 + 4 * k, 0);
    }
    if crate::exe::ee_lt(fx.mem.f(rec + 4), 100.0) {
        for a in [rec + 0x83, rec + 0x87] {
            let v = fx.mem.u8(a);
            if v < 0x50 {
                fx.mem.set8(a, v + 0xA);
            }
        }
    } else {
        for a in [rec + 0x83, rec + 0x87] {
            let v = fx.mem.u8(a);
            if v >= 0x11 {
                fx.mem.set8(a, v - 0xE);
            }
        }
    }
    let mut s0 = 1i32;
    while s0 < 8 {
        let p = D + 0x10 * s0 as u32;
        if crate::exe::ee_le(fx.mem.f(p + 0xAE74), fx.mem.f(rec + 4)) {
            s0 += 1;
            continue;
        }
        let x = fx.mem.read_u32(POINTS);
        fx.mem.write_u32(sp + 0x60, x);
        let z = fx.mem.read_u32(POINTS + 8);
        fx.mem.write_u32(sp + 0x68, z);
        let b = p + 0xAE70;
        let a = D + 0x10 * (s0 - 1) as u32 + 0xAE70;
        l2::segment_point(fx, sp + 0x70, sp + 0x60, rec, a, b);
        for k in 0..4 {
            let w = fx.mem.read_u32(sp + 0x70 + 4 * k);
            fx.mem.write_u32(rec + 0x40 + 4 * k, w);
        }
        l2::segment_point(fx, sp + 0x80, sp + 0x60, rec, a, b);
        for k in 0..4 {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * k);
            fx.mem.write_u32(rec + 0x50 + 4 * k, w);
        }
        let w = fx.mem.i16(rec + 0x8A);
        if 10 * s0 >= w {
            fx.mem.set16(rec + 0x8A, w + 7);
        }
        let w = fx.mem.i16(rec + 0x8C);
        if 9 * s0 >= w {
            fx.mem.set16(rec + 0x8C, w + 6);
        }
        s0 = 9;
    }
    let head = fx.mem.f(rec + 4);
    if crate::exe::ee_le(head, 80.0) {
        let head = ee::add(head, 4.0);
        fx.mem.setf(rec + 4, head);
        let tail = fx.mem.f(rec + 0x14);
        if !crate::exe::ee_le(ee::sub(head, tail), 18.0) {
            fx.mem.setf(rec + 0x14, ee::add(tail, 4.0));
        }
    } else {
        let tail = ee::add(fx.mem.f(rec + 0x14), 4.0);
        fx.mem.setf(rec + 0x14, tail);
        if !crate::exe::ee_le(tail, fx.mem.f(rec + 4)) {
            let next = fx.mem.read_u32(rec + 0x74);
            if next != 0 {
                fx.mem.set32(next + 0x70, 0);
            }
            fx.mem.set16(rec + 0x88, 0);
        }
    }
    if fx.mem.i16(rec + 0x88) == 1 && !crate::exe::ee_le(fx.mem.f(rec + 0x14), 0.0) {
        fx.mem.write_u32(D + 0xBFDC, 0x4160_0000);
        fx.mem.write_u32(LAST, rec);
        fx.mem.set16(D + 0xC022, 1);
        let tail = fx.mem.read_u32(rec + 0x14);
        fx.mem.write_u32(HEIGHT, tail);
        fx.mem.set16(INDEX, 1);
        let a = fx.mem.i16(rec + 0x92) - 0xF;
        fx.mem.set16(REC_ANGLE, a);
        if fx.mem.i16(COUNTER) < 0x73 && spawn_trail(fx, [s0 as u64, u64::from(rec), d], 0x01E0_1208) != 0 {
            fx.mem.set16(rec + 0x88, 2);
        }
    }
    fx.leave(0x90);
}

/// 0x1E02450(D, rec, desc, pass): c061's trail draw (pass 0 the strip3
/// header and colours, pass 1 the 12 slices) turned (x 0x4000 / 0xC000 by
/// the facing, z 0x2000 more a scale) through scales (0.5, 1.5) / 0.4 /
/// 0.3 / 0.2 / 0.15 at D+0xAF10, each slice drawn with texture 4 / 15
/// rects 0x97 and 0x96, corners in order s3 & 1 (frame 0x370: ra, the
/// runner's fp / s7..s4 (0), the entry's s3 = k, s2 = D + 0x98 k, s1 = the
/// live trails drawn, s0 = D).
fn trail_draw(fx: &mut Fx, rec: u32, d: u32, pass: i32, k: u32, s1: u64) {
    let sp = fx.enter(0x370);
    let dd = u64::from(D);
    let ra = if pass == 0 { 0x01E0_1E34 } else { 0x01E0_1E48 };
    fx.saved(sp, &[(0x90, ra), (0x80, 0), (0x70, 0), (0x60, 0), (0x50, 0), (0x40, 0), (0x30, u64::from(k)), (0x20, dd + 0x98 * u64::from(k)), (0x10, s1), (0, dd)]);
    let prev = fx.mem.read_u32(rec + 0x70);
    if pass == 0 {
        l2::desc_strip3(fx, d);
        rgbaq_at(fx, sp + 0x350, rec + 0x80, d + 0x10);
        rgbaq_at(fx, sp + 0x358, rec + 0x80, d + 0x28);
        rgbaq_at(fx, sp + 0x360, rec + 0x84, d + 0x40);
        rgbaq_at(fx, sp + 0x368, rec + 0x84, d + 0x58);
        fx.leave(0x370);
        return;
    }
    let rot = sp + 0x240;
    let x = if fx.mem.i16(FACING) != 0 { 0x4000 } else { 0xC000 };
    fx.mem.set32(rot, x);
    fx.mem.set32(rot + 4, 0);
    fx.mem.set32(rot + 8, 0);
    let scale = |fx: &mut Fx, xz: u32| {
        fx.mem.write_u32(sp + 0xE0, xz);
        fx.mem.write_u32(sp + 0xE4, 0x3FC0_0000);
        fx.mem.write_u32(sp + 0xE8, xz);
    };
    scale(fx, 0x3F00_0000);
    let tex = fx.mem.read_u32(D + 0x2C8);
    fx.desc_texture(d, tex, 1);
    l2::rect(fx, sp + 0xF0, 0x8C, 0);
    l2::uv_strip3(fx, d, sp + 0xF0, 0);
    l2::xform_vp(fx, sp + 0x100, sp + 0xE0, rot, NODE13_KEPT);
    for (k, xz) in [0x3ECC_CCCDu32, 0x3E99_999A, 0x3E4C_CCCD, 0x3E19_999A].into_iter().enumerate() {
        scale(fx, xz);
        let z = fx.mem.i32(rot + 8) + 0x2000;
        fx.mem.set32(rot + 8, z);
        let tmp = sp + 0x250 + 0x40 * k as u32;
        l2::xform_vp(fx, tmp, sp + 0xE0, rot, NODE13_KEPT);
        for i in 0..16 {
            let w = fx.mem.read_u32(tmp + 4 * i);
            fx.mem.write_u32(sp + 0x140 + 0x40 * k as u32 + 4 * i, w);
        }
    }
    let mut s7 = 0i32;
    for _ in 0..12 {
        let ring = |fx: &mut Fx, src: u32, width: u32, base: i32, at: [u32; 4]| {
            let a = (base + fx.mem.i16(src + 0x92)) as i16 as i32;
            let c = l2::sin_deg(fx, SINES, a, 1);
            let w = ee::from_i32(fx.mem.i16(src + width));
            fx.mem.setf(sp + at[0], ee::mul(w, c));
            let s = l2::sin_deg(fx, SINES, a, 0);
            let w = ee::from_i32(fx.mem.i16(src + width));
            fx.mem.setf(sp + at[1], ee::mul(w, s));
            let a = (base + fx.mem.i16(src + 0x92) - 0x1E) as i16 as i32;
            let c = l2::sin_deg(fx, SINES, a, 1);
            let w = ee::from_i32(fx.mem.i16(src + width));
            fx.mem.setf(sp + at[2], ee::mul(w, c));
            let s = l2::sin_deg(fx, SINES, a, 0);
            let w = ee::from_i32(fx.mem.i16(src + width));
            fx.mem.setf(sp + at[3], ee::mul(w, s));
        };
        if prev != 0 {
            ring(fx, prev, 0x8C, s7, [0xA0, 0xA8, 0xB0, 0xB8]);
            let tail = fx.mem.read_u32(prev + 0x14);
            fx.mem.write_u32(rec + 4, tail);
            l2::drift(fx, sp + 0xA0, prev + 0x40, rec + 0x40);
        } else {
            ring(fx, rec, 0x8A, s7, [0xA0, 0xA8, 0xB0, 0xB8]);
        }
        ring(fx, rec, 0x8C, s7, [0xC0, 0xC8, 0xD0, 0xD8]);
        let (h, t) = (fx.mem.read_u32(rec + 4), fx.mem.read_u32(rec + 0x14));
        fx.mem.write_u32(sp + 0xA4, h);
        fx.mem.write_u32(sp + 0xB4, h);
        fx.mem.write_u32(sp + 0xC4, t);
        fx.mem.write_u32(sp + 0xD4, t);
        for k in 0..4u32 {
            let a = sp + 0xA4 + 0x10 * k;
            if crate::exe::ee_lt(fx.mem.f(a), 0.0) {
                fx.mem.set32(a, 0);
            }
        }
        l2::add_rows(fx, sp + 0xA0, rec + 0x40);
        l2::drift(fx, sp + 0xC0, rec + 0x40, rec + 0x50);
        for s3 in 0..4u32 {
            let m = sp + 0x100 + 0x40 * s3;
            let tex = fx.mem.read_u32(TEX5);
            fx.desc_texture(d, tex, 1);
            for (o, v) in [(0xF0u32, 0), (0xF2, 0), (0xF4, 0x1F), (0xF6, 0x7F)] {
                fx.mem.set16(sp + o, v);
            }
            for (id, site) in [(0x97, 0x01E0_2B5Cu32), (0x96, 0x01E0_2BE0)] {
                let tex = fx.mem.read_u32(D + 0x2CC);
                fx.desc_texture(d, tex, 1);
                l2::rect(fx, sp + 0xF0, id, 0);
                l2::uv_strip3(fx, d, sp + 0xF0, (s3 & 1) as i32);
                if l2::project(fx, sp + 0xA0, SPR, m, d, MEAN_W) {
                    let v = fx.mem.f(MEAN_W);
                    fx.fix_residue(v, site);
                    let key = ee::fixunssfsi(v) as i32;
                    let ot = fx.mem.read_u32(OTW);
                    fx.submit(d, ot, key);
                }
            }
        }
        s7 = (s7 + 0x1E) as i16 as i32;
    }
    fx.leave(0x370);
}

/// The 8-byte RGBAQ of `c` (0x108020 through `local`) at `to`.
fn rgbaq_at(fx: &mut Fx, local: u32, c: u32, to: u32) {
    l2::rgbaq(fx, local, c);
    let (a, b) = (fx.mem.read_u32(local), fx.mem.read_u32(local + 4));
    fx.mem.write_u32(to, a);
    fx.mem.write_u32(to + 4, b);
}

/// rand from a step's `jal` at `jal` (rand's frame: s0 = `t`).
fn rnd(fx: &mut Fx, t: u32, jal: u32) -> i32 {
    fx.rand_from(t, jal + 8)
}

/// rand % m (HI) from a step's `jal` at `jal`.
fn rmod(fx: &mut Fx, t: u32, jal: u32, m: i32) -> i32 {
    let v = rnd(fx, t, jal);
    fx.div(v, m).1
}

/// rand % 0x800 - 0x400 (the record's signed `andi` remainder).
fn spin(fx: &mut Fx, t: u32, jal: u32) -> i32 {
    let v = rnd(fx, t, jal);
    v % 0x800 - 0x400
}

/// The first update shared by the smoke and the sparks (rand sites from
/// `jals`): size 20, cell row (rand >> 4) % 3, colour `grey`, velocity
/// (rand % 100 - 50) / `div` in y / x / z, +0x5C rand % 360, camera
/// angles into +0x20, spins +4 / +2 / +0.
fn first(fx: &mut Fx, t: u32, s1: u64, jals: &[u32; 9], grey: (i32, i32, i32), div: f32) {
    fx.mem.set16(t + 8, 0x14);
    let w = fx.mem.i16(t + 8);
    fx.mem.set16(t + 0xA, w);
    let v = rnd(fx, t, jals[0]);
    let (_, h) = fx.div(v >> 4, 3);
    fx.mem.set16(t + 0x56, h);
    l2::rgb(fx, t + 0x60, grey.0, grey.1, grey.2);
    fx.mem.set16(t + 0x58, 2);
    for (o, &j) in [0x34u32, 0x30, 0x38].iter().zip(&jals[1..4]) {
        let v = rmod(fx, t, j, 100);
        fx.mem.setf(t + o, ee::from_i32(v - 0x32));
    }
    for o in [0x34u32, 0x30, 0x38] {
        let v = ee::div(fx.mem.f(t + o), div);
        fx.mem.setf(t + o, v);
    }
    let v = rmod(fx, t, jals[4], 0x168);
    fx.mem.set16(t + 0x5C, v);
    camera_angles_saving(fx, t + 0x20, u64::from(t), s1, jals[5]);
    for (o, &j) in [4u32, 2, 0].iter().zip(&jals[6..9]) {
        let v = spin(fx, t, j);
        fx.mem.set16(t + o, v);
    }
}

/// The spins +4 / +2 / +0 added to +0x28 / +0x24 / +0x20, then the camera
/// angles into +0x20 again (from `jal`).
fn turn(fx: &mut Fx, t: u32, s1: u64, jal: u32) {
    for (o, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + o).wrapping_add(fx.mem.i16(t + s) as u32);
        fx.mem.write_u32(t + o, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), s1, jal + 8);
}

fn drift3(fx: &mut Fx, t: u32) {
    for k in 0..3 {
        let v = ee::add(fx.mem.f(t + 0x10 + 4 * k), fx.mem.f(t + 0x30 + 4 * k));
        fx.mem.setf(t + 0x10 + 4 * k, v);
    }
}

/// 0x1E01240 (D, task; frame 0x30: ra, the entry's s1, s0 = D): grey
/// (0x60, 0x60, 0x80) smoke of size 20 growing by 10 to 200, drifting by
/// (rand % 100 - 50) / 60 and 2 along z by the facing; in by 0x40 under
/// 0x80 for 20 updates, then out by 9.
fn smoke_step(fx: &mut Fx, t: u32, s1: u64) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, s1), (0, u64::from(D))]);
    let me = u64::from(D);
    if fx.mem.i16(t + 0x58) == 1 {
        let jals = [0x01E0_1270, 0x01E0_12A4, 0x01E0_12D0, 0x01E0_12FC, 0x01E0_1354, 0x01E0_1378, 0x01E0_1378, 0x01E0_1398, 0x01E0_13B8];
        first(fx, t, me, &jals, (0x60, 0x60, 0x80), 60.0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x14 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let w = fx.mem.i16(t + 8);
        if w < 0xC8 {
            fx.mem.set16(t + 8, w + 0xA);
        }
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        drift3(fx, t);
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 2.0) } else { ee::sub(z, 2.0) };
        fx.mem.setf(t + 0x18, z);
    }
    turn(fx, t, me, 0x01E0_1508);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E01540 (D, task; frame 0x20: ra, the entry's s0 = D): a tint by
/// kind +0x50 (0 (90, 90, A0) for 32 updates, 1 (D0, D0, 80) / (C0, C0,
/// 80) and 2 (F0, E0, FF) for 20), both alphas in by 0x40 under 0x80, then
/// out by 4.
fn glow_step(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0x90, 0x90, 0xA0);
            l2::rgb(fx, t + 0x64, 0x90, 0x90, 0xA0);
            fx.mem.set16(t + 6, 0x20);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xD0, 0xD0, 0x80);
            l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0x80);
            fx.mem.set16(t + 6, 0x14);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            l2::rgb(fx, t + 0x60, 0xF0, 0xE0, 0xFF);
            l2::rgb(fx, t + 0x64, 0xF0, 0xE0, 0xFF);
            fx.mem.set16(t + 6, 0x14);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a.wrapping_add(0x40));
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 6 {
            fx.mem.write_u8(t + 0x63, a - 4);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x67);
        if a >= 6 {
            fx.mem.write_u8(t + 0x67, a - 4);
        } else {
            fx.mem.write_u8(t + 0x67, 0);
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E016D0 (D, task; frame 0x20: ra, the entry's s0 = D): a spark of
/// size 20 drifting by (rand % 100 - 50) / 15, flickering grey 0x80 / 0xC0
/// by age parity; in by 0x40 under 0x80 for 5..14 updates, then out by 9.
fn spark_step(fx: &mut Fx, t: u32, s1: u64) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        let jals = [0x01E0_16F8, 0x01E0_172C, 0x01E0_1758, 0x01E0_1784, 0x01E0_17DC, 0x01E0_1800, 0x01E0_1800, 0x01E0_1820, 0x01E0_1840];
        first(fx, t, s1, &jals, (0x80, 0x80, 0x80), 15.0);
        let v = rmod(fx, t, 0x01E0_1860, 0xA);
        fx.mem.set16(t + 6, v + 5);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        drift3(fx, t);
    }
    let g = if fx.mem.i16(t + 0x54) % 2 == 1 { 0x80 } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    turn(fx, t, s1, 0x01E0_1994);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E019C0 (D, task; frame 0x90: ra, the entry's s1, s0 = D): a window
/// sprite (0x6FF8 .. 0x8FF8) sampling the frame (0x207EF0, filter 1) at ST
/// (0, 0) .. (1, 1), colour (0x70, 0x70, 0x80, alpha +0x63), ALPHA 0x44,
/// key 0; while the timer is 21 or more in by 20 below 0x50, then out by 9.
fn tint_step(fx: &mut Fx, t: u32, s1: u64) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, s1), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0x70, 0x70, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let c = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x48 + k, c);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    fx.mem.set32(sp + 0x50, 0);
    fx.mem.set32(sp + 0x54, 0);
    fx.mem.write_u32(sp + 0x60, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x64, 0x3F80_0000);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i32(TIMER) >= 0x15 {
        if a < 0x50 {
            fx.mem.write_u8(t + 0x63, a + 0x14);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x90);
}
