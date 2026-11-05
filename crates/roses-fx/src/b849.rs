//! BMODULE b849 (card 678), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E033B0, data area 0x1ED0000) [F]: a pool of 300 tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step
//! +0x44 through `jalr`, the stub 0x1E033A0 skipped; spawn 0x1E03DA0(D,
//! step, 0) with the colour D+0xF0F4, the angle D+0xF1DE into +0x5A and the
//! index D+0xF206 into +0x50 / +0x52), no unit flash. Drawn with arcs
//! (0x1E00920, `billboard::arc_strips`), raised turned quads (0x1E01570,
//! `quads4::quad_turned_raised`), a self tint (0x1E013A0, `quads4::tint4`)
//! and the record's own band piece 0x1E03F50 (one strip3 quad 10 degrees
//! wide from +0x5A - 200, cell `uv::grid_rect` of D+0xF148 + 2 (+0x50 %
//! 36)). The petals 0x1E02280 ride five points D+0xEF10 + 0x10 k (the
//! attacker's node 0xA5, z +-10 by the facing) by +0x50 / 36, their radii
//! shrinking by phases. Globals: nodes D+0xF128.. (attacker 0xA5 (clear) /
//! 0xAC / 0xA9 / 0xB0 / 0xB1 / 0xB2 .. 0xB4 into one slot), target 1 at
//! D+0xF138, textures (5, 0..2) D+0xF0D0.. and (4, 0xF) D+0x29C, timer
//! D+0xF0F0 = 499, colour D+0xF0F4, OT D+0xF0FC, tint w D+0xF114, cells
//! D+0xF148, petal ring D+0xF1DA, angle D+0xF1DE, arc angle D+0xF1E4 / its
//! index D+0xF1E6, index D+0xF206, facing D+0xF20A, param D+0xF20C, counter
//! D+0xF20E, bytes D+0xF210 / D+0xF310 / D+0xF410, sines D+0xF510. The steps
//! 0x1E02B90 / 0x1E02F20 are never spawned [F]: not ported, nor their draw
//! cases. Returns 1 / 4 / 2 / 6 / 8 on 0x136 / 0x1F0 / 0x1E5 / 0x141..0x1DF
//! / timer out.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0x12C;
const ARC: u32 = 0x01E0_20C0;
const PETAL: u32 = 0x01E0_2280;
const ARC_R: u32 = 0x01E0_29D0;
const QUAD: u32 = 0x01E0_2CD0;
const ARC_B: u32 = 0x01E0_2F30;
const FLASH: u32 = 0x01E0_30B0;
const WINDOW: u32 = 0x01E0_31B0;
const STUB: u32 = 0x01E0_33A0;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_38C8;
const POS: u32 = D + 0x250;
const POINTS: u32 = D + 0xEF10;
const TEX: u32 = D + 0xF0D0;
const TIMER: u32 = D + 0xF0F0;
const COLOUR: u32 = D + 0xF0F4;
const OT_AT: u32 = D + 0xF0FC;
const TINT_W: u32 = D + 0xF114;
const NODES: u32 = D + 0xF128;
const TARGET_NODE: u32 = D + 0xF138;
const CELLS: u32 = D + 0xF148;
const RING_K: u32 = D + 0xF1DA;
const ANGLE: u32 = D + 0xF1DE;
const ARC_A: u32 = D + 0xF1E4;
const ARC_K: u32 = D + 0xF1E6;
const INDEX: u32 = D + 0xF206;
const FACING: u32 = D + 0xF20A;
const COUNTER: u32 = D + 0xF20E;
const BYTES: u32 = D + 0xF210;
const BYTES_B: u32 = D + 0xF310;
const BYTES_C: u32 = D + 0xF410;
const SINES: u32 = D + 0xF510;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn facing(fx: &mut Fx) -> bool {
    fx.mem.i16(FACING) != 0
}

fn node(fx: &mut Fx, k: u32) -> i32 {
    fx.mem.i16(NODES + 2 * k)
}

/// The entry 0x1E033B0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x150);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xF680);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xF20C, param);
    fx.mem.set32(TIMER, 0x1F3);
    for (k, n) in [(0u32, 0xA5), (1, 0xAC), (2, 0xA9), (3, 0xB0), (4, 0xB1), (5, 0xB2), (5, 0xB3), (5, 0xB4)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(n == 0xA5));
        fx.mem.set16(NODES + 2 * k, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    let tex = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, tex);
    for k in 0..3u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(v, 0x01E0_35E4);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    let fill = |fx: &mut Fx, at: u32, ks: std::ops::Range<u32>, ra: u32, m: i32, add: i32| {
        for k in ks {
            let r = fx.rand_from(D, ra);
            let v = fx.div(r >> 4, m).1;
            fx.mem.write_u8(at + k, (v + add) as u8);
        }
    };
    fill(fx, BYTES, 0..0x100, 0x01E0_3614, 0x1E, -0xF);
    for k in 100..0xB4u32 {
        fx.mem.write_u8(BYTES + k, 0);
    }
    fill(fx, BYTES_B, 0..0x100, 0x01E0_36AC, 0x1E, -0xF);
    fill(fx, BYTES_B, 100..0xB4, 0x01E0_370C, 0x14, 0xA);
    fill(fx, BYTES_C, 0..0x100, 0x01E0_3764, 0x1E, -0xF);
    for k in 0..0x24u32 {
        let r = fx.rand_from(D, 0x01E0_37C0);
        fx.mem.set16(CELLS + 2 * k, r.wrapping_rem(8));
    }
    for k in 0..8u32 {
        fx.mem.set16(D + 0xF1EE + 2 * k, 0);
    }
    fx.mem.set16(ARC_A, 0x5A);
    0
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 || f == STUB {
            continue;
        }
        run_task(fx, f, slot + 0x2A0, [u64::from(D), 0, u64::from(slot), u64::from(k), 0]);
    }
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        draw(fx, sp, slot + 0x2A0);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x136 => 1,
        0x1F0 => 4,
        0x1E5 => 2,
        0x141..=0x1DF => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw case for task `t`.
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    if f == ARC || f == ARC_R || f == ARC_B {
        fx.mem.set16(D + 0xB8, 0x18);
        billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == PETAL {
        let w = fx.mem.read_u32(TEX);
        fx.mem.write_u32(D + 0x88, w);
        let fr = fx.mem.i16(t + 0x56);
        l2::rect(fx, sp + 0x70, 0x14, fr);
        copy4(fx, D + 0x94, sp + 0x70);
        fx.mem.set16(D + 0xB8, 1);
        fx.mem.set16(D + 0xBE, 0);
        let v = fx.mem.i16(D + 0x9A) + 1;
        fx.mem.set16(D + 0x9A, v);
        petal(fx, t, 0x20, t + 0x10);
    }
    if f == QUAD {
        let fr = fx.mem.i16(t + 0x56);
        l2::rect(fx, sp + 0x90, 0x14, fr);
        copy4(fx, D + 0x94, sp + 0x90);
        let w = fx.mem.read_u32(TEX + 8);
        fx.mem.write_u32(D + 0x88, w);
        let c = fx.mem.i16(t + 0x56);
        if c < 6 {
            fx.mem.set16(D + 0x94, c << 5);
            fx.mem.set16(D + 0x96, 0);
        } else {
            fx.mem.set16(D + 0x94, (c - 6) << 5);
            fx.mem.set16(D + 0x96, 0x20);
        }
        fx.mem.set16(D + 0x98, 0x1F);
        fx.mem.set16(D + 0x9A, 0x1F);
        fx.mem.set16(D + 0xBE, 0);
        copy3(fx, sp + 0x60, t + 0x10);
        quads4::quad_turned_raised(fx, uv::uv_rect, D, t, 0x21, sp + 0x60);
    }
    if f == FLASH {
        let h = fx.self_handle();
        fx.unit_position(h, sp + 0x60);
        // 0x1F71A0's frame below the entry's (its saved ra, s0 = D).
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_3C30), (0, u64::from(D))]);
        let z = fx.mem.f(sp + 0x68);
        let z = if facing(fx) { ee::sub(z, 2.0) } else { ee::add(z, 2.0) };
        fx.mem.setf(sp + 0x68, z);
        let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
        let mut x = fx.mem.read_u64(sp + 0xD8);
        if let Some(w) = fx.project(p, None, &mut x) {
            fx.mem.write_u64(sp + 0xD8, x);
            fx.mem.setf(TINT_W, w);
        }
        crate::b590::project_residue(fx, sp, sp + 0x60);
        let w = fx.mem.f(TINT_W);
        quads4::tint4(fx, t, 0x21, w);
    }
}

/// `regs`: the run loop's s0 (D), s1 (0), s2 (the slot), s3 (its index),
/// s4 (the runner's, 0 [H]) at the `jalr`.
fn run_task(fx: &mut Fx, f: u32, t: u32, regs: [u64; 5]) {
    match f {
        ARC => arc(fx, t, regs, false),
        PETAL => petal_step(fx, t, regs),
        ARC_R => arc(fx, t, regs, true),
        QUAD => quad(fx, t, regs),
        ARC_B => arc_b(fx, t, regs),
        FLASH => flash(fx, t, regs),
        WINDOW => window(fx, t, regs),
        _ => panic!("b849: unknown task step {f:#x}"),
    }
}

/// A step's prologue: `frame` bytes, the caller's s0.. (`n` of `regs`) at
/// +`base` + 0x10 k, ra at `ra_at`.
fn enter_step(fx: &mut Fx, frame: u32, regs: [u64; 5], n: usize, base: u32, ra_at: u32) -> u32 {
    let sp = fx.enter(frame);
    for k in 0..n {
        fx.saved(sp + base + 0x10 * k as u32, &[(0, regs[k])]);
    }
    fx.saved(sp, &[(ra_at, TASK_RA)]);
    sp
}

/// 0x1E03DA0(D, step, 0) with the caller's s0..s4 = `regs`.
fn spawn(fx: &mut Fx, f: u32, regs: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, regs[4]), (0x30, regs[3]), (0x20, regs[2]), (0x10, regs[1]), (0, regs[0])]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            // 0x107390's `sq ra` upper half (0) (b232's).
            fx.mem.write_u64(sp - 8, 0);
            copy3(fx, t + 0x10, POS);
            let a = fx.mem.i16(ANGLE);
            fx.mem.set16(t + 0x5A, a);
            let i = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x50, i);
            let i = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x52, i);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E018B0(D, 0) (frame 0x40: the entry's s0 = D, s1 = 0, s2 = param,
/// ra).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let (mut s1, mut s2) = (0u64, param as i64 as u64);
    fx.saved(sp, &[(0x30, 0x01E0_3888), (0x20, s2), (0x10, s1), (0, u64::from(D))]);
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    if counter(fx) == 1 {
        for (k, ra) in [0x01E0_1920u32, 0x01E0_1944, 0x01E0_1968, 0x01E0_198C, 0x01E0_19B0].into_iter().enumerate() {
            let h = fx.self_handle();
            let s = node(fx, 0);
            crate::b076::node_position_saving(fx, h, s, POINTS + 0x10 * k as u32, u64::from(D), Some(s1), ra);
        }
        for k in 0..5u32 {
            let z = fx.mem.f(POINTS + 0x10 * k + 8);
            let z = if facing(fx) { ee::add(z, 10.0) } else { ee::sub(z, 10.0) };
            fx.mem.setf(POINTS + 0x10 * k + 8, z);
        }
    }
    if counter(fx) == 0xDC {
        for k in 0..8u32 {
            s1 = u64::from(k);
            let h = fx.self_handle();
            let s = node(fx, 0);
            crate::b076::node_position_saving(fx, h, s, D + 0xF030 + 0x10 * k, u64::from(D), Some(s1), 0x01E0_1B40);
        }
        s1 = 8;
        s2 = u64::from(D + 0x80);
    }
    let regs = |s1: u64, s2: u64| [u64::from(D), s1, s2, 0, 0];
    if counter(fx) == 0x132 {
        spawn(fx, WINDOW, regs(s1, s2), 0x01E0_1B94);
    }
    let c = counter(fx);
    if (0x23..0x1A5).contains(&c) && fx.mem.i16(RING_K) < 0x24 {
        for (da, di, ra) in [(0, 0, 0x01E0_1C14u32), (0x5A, 0x24, 0x01E0_1C6C), (0xB4, 0x48, 0x01E0_1CC4), (0x10E, 0x6C, 0x01E0_1D1C), (0, 0x90, 0x01E0_1D70)] {
            let j = fx.mem.i16(RING_K);
            fx.mem.set16(ANGLE, 10 * j + da);
            let j = fx.mem.i16(RING_K);
            fx.mem.set16(INDEX, j + di);
            spawn(fx, PETAL, regs(s1, s2), ra);
        }
        let j = fx.mem.i16(RING_K) + 1;
        fx.mem.set16(RING_K, j);
    }
    let c = counter(fx);
    if (0x7F..0x1A5).contains(&c) && c.wrapping_rem(2) == 1 && fx.mem.i16(ARC_A) < 0x1E0 {
        let h = fx.self_handle();
        let s = node(fx, 0);
        crate::b076::node_position_saving(fx, h, s, POS, u64::from(D), Some(s1), 0x01E0_1E08);
        let a = fx.mem.i16(ARC_A);
        let i = if facing(fx) { a } else { 0xB4 - a };
        fx.mem.set16(INDEX, i);
        let a = fx.mem.i16(ARC_A);
        fx.mem.set16(ANGLE, a);
        spawn(fx, ARC_B, regs(s1, s2), 0x01E0_1E84);
        let a = fx.mem.i16(ARC_A);
        fx.mem.set16(ANGLE, a);
        let k = fx.mem.i16(ARC_K);
        fx.mem.set16(INDEX, k);
        spawn(fx, QUAD, regs(s1, s2), 0x01E0_1EC8);
        let a = fx.mem.i16(ARC_A) + 0x1E;
        fx.mem.set16(ARC_A, a);
        if fx.mem.i16(ARC_A) >= 0x1E0 {
            spawn(fx, FLASH, regs(s1, s2), 0x01E0_1F10);
        }
        let k = fx.mem.i16(ARC_K) + 1;
        fx.mem.set16(ARC_K, k);
    }
    if counter(fx) == 0x7F {
        for (f, ras) in [(ARC, [0x01E0_1F64u32, 0x01E0_1F88, 0x01E0_1FAC, 0x01E0_1FD0, 0x01E0_1FF4]), (ARC_R, [0x01E0_2014, 0x01E0_2038, 0x01E0_205C, 0x01E0_2080, 0x01E0_20A4])] {
            for (i, ra) in ras.into_iter().enumerate() {
                fx.mem.set16(INDEX, i as i32);
                spawn(fx, f, regs(s1, s2), ra);
            }
        }
    }
    fx.leave(0x40);
}

/// The alpha byte `a` down by `down.1` while at least `down.0`, else the
/// task ends.
fn fade_out(fx: &mut Fx, t: u32, a: u32, down: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if v >= down.0 {
        fx.mem.write_u8(t + a, v.wrapping_sub(down.1));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The alpha byte `a` up by `up.1` while below `up.0`.
fn fade_in(fx: &mut Fx, t: u32, a: u32, up: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if v < up.0 {
        fx.mem.write_u8(t + a, v.wrapping_add(up.1));
    }
}

/// The alpha byte `a` down by `down.1` while at least `down.0`, else 0.
fn fade_to0(fx: &mut Fx, t: u32, a: u32, down: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    fx.mem.write_u8(t + a, if v >= down.0 { v - down.1 } else { 0 });
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn young(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6)
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x58) == 1
}

fn odd(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54).wrapping_rem(2) == 1
}

/// The camera turn: 0x205580 into +0x20, then +0x4000.
fn face(fx: &mut Fx, t: u32, ra: u32) {
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), ra);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
}

/// 0x1E020C0 / 0x1E029D0 (frame 0x60): a camera-facing arc (radius 30
/// grey over olive, or 15 dark red) at the attacker's node 0xA5, pushed
/// +0x50 fifths of the way towards node 0xAC (`red`: 0xA9) in x / y; in by
/// 0x30 below 0xC0 for 200 updates, then out by 0xB.
fn arc(fx: &mut Fx, t: u32, regs: [u64; 5], red: bool) {
    let sp = enter_step(fx, 0x60, regs, 2, 0x10, 0x30);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.write_u32(t + 0x4C, if red { 0x4170_0000 } else { 0x41F0_0000 });
        fx.mem.set16(t + 6, 0xC8);
        if red {
            l2::rgb(fx, t + 0x64, 0x80, 0x10, 0x10);
            l2::rgb(fx, t + 0x60, 0x80, 0x10, 0x10);
        } else {
            l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x40);
        }
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x67, (0xC0, 0x30));
    } else {
        fade_out(fx, t, 0x67, (0xD, 0xB));
    }
    let f20 = ee::from_i32(fx.mem.i16(t + 0x50));
    let ras: [u32; 4] = if red { [0x01E0_2AB0, 0x01E0_2AD8, 0x01E0_2AF8, 0x01E0_2B1C] } else { [0x01E0_21A0, 0x01E0_21C8, 0x01E0_21E8, 0x01E0_220C] };
    face(fx, t, ras[0]);
    let (s0, s1) = (u64::from(t), u64::from(D));
    let h = fx.self_handle();
    let s = node(fx, 0);
    crate::b076::node_position_saving(fx, h, s, t + 0x10, s0, Some(s1), ras[1]);
    let h = fx.self_handle();
    let s = node(fx, 0);
    crate::b076::node_position_saving(fx, h, s, sp + 0x40, s0, Some(s1), ras[2]);
    let h = fx.self_handle();
    let s = node(fx, if red { 2 } else { 1 });
    crate::b076::node_position_saving(fx, h, s, sp + 0x50, s0, Some(u64::from(sp + 0x50)), ras[3]);
    for c in [0u32, 4] {
        let d = ee::div(ee::sub(fx.mem.f(sp + 0x50 + c), fx.mem.f(sp + 0x40 + c)), 5.0);
        let v = ee::add(fx.mem.f(t + 0x10 + c), ee::mul(f20, d));
        fx.mem.setf(t + 0x10 + c, v);
    }
    age(fx, t);
    fx.leave(0x60);
}

/// 0x1E02280 (frame 0x30): a petal (radii 60 / 60, phase +0x52 = 0 / 2 /
/// 4 / 6 / 8 by the ring +0x50 / 36) riding its ring's point D+0xEF10 +
/// 0x10 (+0x50 / 36) for +6 = (rand >> 4) % 100 + 0x132 updates then
/// drifting 2 a update along z, spinning +-0x180 about z from 0x80, its
/// outer then inner radius shrinking by 5 a phase; white / grey by age
/// parity; in by 0xC below 0xC0 for 30 updates, out by 0xB from 0x1DC.
fn petal_step(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x30, regs, 2, 0, 0x20);
    let band = |fx: &mut Fx| {
        let k = fx.mem.i16(t + 0x50);
        [0x24, 0x48, 0x6C, 0x90, 0xB4].iter().position(|&b| k < b)
    };
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x4270_0000);
        fx.mem.write_u32(t + 0x4C, 0x4270_0000);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.set16(t + 0x58, 2);
        if let Some(b) = band(fx) {
            fx.mem.set16(t + 0x52, 2 * b as i32);
        }
        fx.mem.set32(t + 0x20, 0x4000);
        let r = fx.rand_from(t, 0x01E0_2338);
        let v = fx.div(r, 3).1;
        fx.mem.set16(t + 8, v + 4);
        let r = fx.rand_from(t, 0x01E0_2358);
        let v = fx.div(r >> 4, 0x64).1;
        fx.mem.set16(t + 6, v);
    }
    if fx.mem.i16(t + 0x54) < 0x1E {
        fade_in(fx, t, 0x63, (0xC0, 0xC));
        fade_in(fx, t, 0x67, (0xC0, 0xC));
    } else if counter(fx) >= 0x1DC {
        fade_out(fx, t, 0x63, (0xD, 0xB));
        let v = fx.mem.read_u8(t + 0x67);
        if v >= 0xD {
            fx.mem.write_u8(t + 0x67, v - 0xB);
        }
    }
    let g = if odd(fx, t) { 0xFF } else { 0x80 };
    l2::rgb(fx, t + 0x60, g, g, g);
    l2::rgb(fx, t + 0x64, g, g, g);
    if counter(fx) != 0 {
        let p = fx.mem.i16(t + 0x52);
        if (0..10).contains(&p) {
            // Even phases shrink the outer radius to 15 / 25 / 35 / 45 / 55
            // and move on; odd ones shrink the inner one to 5 / 15 / .. 45.
            let n = p / 2;
            if p % 2 == 0 {
                let r = fx.mem.f(t + 0x48);
                if crate::exe::ee_le(r, ee::from_i32(15 + 10 * n)) {
                    fx.mem.set16(t + 0x52, p + 1);
                } else {
                    fx.mem.setf(t + 0x48, ee::sub(r, 5.0));
                }
            } else {
                let r = fx.mem.f(t + 0x4C);
                if !crate::exe::ee_le(r, ee::from_i32(5 + 10 * n)) {
                    fx.mem.setf(t + 0x4C, ee::sub(r, 5.0));
                }
            }
        }
    }
    let c = counter(fx);
    let until = fx.mem.i16(t + 6) + 0x132;
    if c >= 0x80 && c < until {
        if let Some(b) = band(fx) {
            let d = if b % 2 == 0 { -0x180 } else { 0x180 };
            let v = fx.mem.i32(t + 0x28).wrapping_add(d);
            fx.mem.set32(t + 0x28, v);
        }
    }
    let c = counter(fx);
    let until = fx.mem.i16(t + 6) + 0x132;
    if c < until {
        if let Some(b) = band(fx) {
            copy4(fx, t + 0x10, POINTS + 0x10 * b as u32);
        }
    } else {
        let z = fx.mem.f(t + 0x18);
        let z = if facing(fx) { ee::sub(z, 2.0) } else { ee::add(z, 2.0) };
        fx.mem.setf(t + 0x18, z);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02CD0 (frame 0x30): a grey raised quad (+8 / +0xA 15, cell +0x50,
/// height 43.5, turned +0x5A - 0x38 about x and 0 / 180 about y by the
/// facing) riding the attacker's node 0xA5, sinking 0.25 and drifting 0.5
/// along z a update; in by 0x2A below 0x80 for 3 updates, out by 9 from
/// 0x15F.
fn quad(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x30, regs, 2, 0, 0x20);
    if first(fx, t) {
        fx.mem.set16(t + 8, 0xF);
        fx.mem.set16(t + 0xA, 0xF);
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x56, v);
        fx.mem.write_u32(t + 0x34, 0x422E_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let a = fx.mem.i16(t + 0x5A);
        fx.mem.set16(t + 0x5C, a - 0x38);
        let b = if facing(fx) { 0xB4 } else { 0 };
        fx.mem.set16(t + 0x5A, b);
    }
    if fx.mem.i16(t + 0x54) < 3 {
        fade_in(fx, t, 0x63, (0x80, 0x2A));
        fade_in(fx, t, 0x67, (0x80, 0x2A));
    } else if counter(fx) >= 0x15F {
        fade_out(fx, t, 0x63, (0xB, 9));
        fade_to0(fx, t, 0x67, (0xB, 9));
    }
    let g = if odd(fx, t) { 0x80 } else { 0x70 };
    l2::rgb(fx, t + 0x60, g, g, g);
    fx.mem.write_u32(t + 0x20, 0);
    let h = fx.self_handle();
    let s = node(fx, 0);
    crate::b076::node_position_saving(fx, h, s, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_2E94);
    let z = fx.mem.f(t + 0x18);
    let z = if facing(fx) { ee::add(z, 0.5) } else { ee::sub(z, 0.5) };
    fx.mem.setf(t + 0x18, z);
    let y = ee::sub(fx.mem.f(t + 0x14), 0.25);
    fx.mem.setf(t + 0x14, y);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02F30 (frame 0x30): a camera-facing arc (radius 30, white over
/// olive) at the attacker's node 0xA5 pushed 6 along the angle +0x50; in
/// by 0x30 below 0xC0 for 5 updates, then out by 0xB.
fn arc_b(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x30, regs, 2, 0, 0x20);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.write_u32(t + 0x4C, 0x41F0_0000);
        fx.mem.set16(t + 6, 5);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x40);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x67, (0xC0, 0x30));
    } else {
        fade_out(fx, t, 0x67, (0xD, 0xB));
    }
    face(fx, t, 0x01E0_3004);
    let h = fx.self_handle();
    let s = node(fx, 0);
    crate::b076::node_position_saving(fx, h, s, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_302C);
    for (o, cos) in [(0u32, 1), (4, 0)] {
        let a = fx.mem.i16(t + 0x50);
        let s = l2::sin_deg(fx, SINES, a, cos);
        let v = ee::add(fx.mem.f(t + 0x10 + o), ee::mul(6.0, s));
        fx.mem.setf(t + 0x10 + o, v);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E030B0: the self tint, grey, in by 0x2A below 0x80 for 12 updates,
/// then out by 0xC.
fn flash(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x20, regs, 1, 0, 0x10);
    if first(fx, t) {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 6, 0xC);
        fx.mem.set16(t + 0x58, 2);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, (0x80, 0x2A));
        fade_in(fx, t, 0x67, (0x80, 0x2A));
    } else {
        fade_out(fx, t, 0x63, (0xE, 0xC));
        fade_to0(fx, t, 0x67, (0xE, 0xC));
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E031B0 (frame 0x90): a violet-grey window sprite ((0x78, 0x78,
/// 0xA0), ALPHA 0x44, key 0), in by 5 below 0x50 until update 0x1DB, then
/// out by 7.
fn window(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    let sp = enter_step(fx, 0x90, regs, 2, 0, 0x20);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for (k, c) in [0x78u8, 0x78, 0xA0].into_iter().enumerate() {
        fx.mem.write_u8(sp + 0x48 + k as u32, c);
    }
    let a = fx.mem.read_u8(t + 0x63);
    fx.mem.write_u8(sp + 0x4B, a);
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for o in [0x50u32, 0x54] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(s, 512.0));
    }
    for o in [0x60u32, 0x64] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(ee::sub(512.0, s), 512.0));
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, 0x80_0000_0044);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if counter(fx) < 0x1DB {
        fade_in(fx, t, 0x63, (0x50, 5));
    } else {
        fade_out(fx, t, 0x63, (9, 7));
    }
    age(fx, t);
    fx.leave(0x90);
}

/// 0x1E03F50(D, t, mode, pos) (frame 0x1D0): one textured strip3 quad of a
/// flat band (inner +0x4C, outer +0x48) 10 degrees wide from +0x5A - 200,
/// cell `uv::grid_rect` of D+0xF148 + 2 (+0x50 % 36), turned +0x20 at
/// `pos`, keyed by mean w.
fn petal(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1D0);
    copy4(fx, sp + 0x80, pos);
    let d = sp + 0x140;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 1);
    l2::colour_strip3(fx, d, t + 0x64, 2);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    fx.desc_alpha(d, crate::c061::blend_alpha(mode >> 4));
    copy3(fx, sp + 0xD0, sp + 0x80);
    l2::xform_vp(fx, sp + 0xE0, 0, t + 0x20, sp + 0xD0);
    for o in [0x94u32, 0xA4, 0xB4, 0xC4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let (f20, f21) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let k = fx.mem.i16(t + 0x50);
    let i = fx.div(k, 0x24).1;
    let fr = fx.mem.i16((CELLS as i32 + 2 * i) as u32);
    uv::grid_rect(fx, sp + 0x130, fr);
    copy4(fx, D + 0x94, sp + 0x130);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    for (a0, outer, inner) in [(0i32, 0xB0u32, 0x90u32), (0xA, 0xC0, 0xA0)] {
        let a = (fx.mem.i16(t + 0x5A) + a0 - 0xC8) as i16 as i32;
        let c = l2::sin_deg(fx, SINES, a, 1);
        let s = l2::sin_deg(fx, SINES, a, 0);
        fx.mem.setf(sp + outer, ee::mul(c, f20));
        fx.mem.setf(sp + outer + 8, ee::mul(s, f20));
        fx.mem.setf(sp + inner, ee::mul(c, f21));
        fx.mem.setf(sp + inner + 8, ee::mul(s, f21));
    }
    if l2::project(fx, sp + 0x90, 0x7000_0000, sp + 0xE0, d, sp + 0x120) {
        let key = ee::fixunssfsi(fx.mem.f(sp + 0x120)) as i32;
        let ot = fx.mem.read_u32(OT_AT);
        fx.submit(d, ot, key);
    }
    fx.leave(0x1D0);
}
