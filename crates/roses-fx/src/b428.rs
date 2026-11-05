//! BMODULE b428 (card 382), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01F80, data area 0x1ED0000, 0xB630 bytes) [F]
//!: b641's task pool (400 tasks of 0x70 bytes at
//! D+0x2A0, alive +0x58, step +0x44 through `jalr`, 0x1E01F70 the idle
//! step; globals at D+0xB2xx) with the emitter inlined in the entry,
//! spawned by `spawn::task400` (a2350b28c0ee at 0x1E026F0) and drawn with
//! the second library: rings of quads (7369422df7f8 at 0x1E00920), arcs
//! (e6dce326551d at 0x1E00ED0) and quads (add5d81dd3fe at 0x1E01230).
//! 0x97..0x15F node 0x20's point (D+0xB1D0) lifted 3.3 a update, a
//! flickering green arc ring and a grey ring a update there; 0x109..0x1C1
//! three grey orbs a update from it, drifting up and across, each
//! bursting into three sparks at age 80. Returns 1 on update 0x122, 4 on
//! 0x1D1, 2 on 0x1C7, 6 on 0x12D..0x1C1, 8 once the 0x1DA-update timer
//! D+0xB210 has run out.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, spawn::task400, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const ORB: u32 = 0x01E0_14F0;
const RING: u32 = 0x01E0_18E0;
const SPARK: u32 = 0x01E0_1A70;
const ARC: u32 = 0x01E0_1DE0;
const IDLE: u32 = 0x01E0_1F70;
/// The return address of the entry's `jalr` into a step (0x1E023E0 + 8).
const TASK_RA: u64 = 0x01E0_23E8;
const POS: u32 = D + 0x250;
/// Node 0x20's point, lifted 3.3 a update.
const AIM: u32 = D + 0xB1D0;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const NODES: u32 = D + 0xB244;
const FACING: u32 = D + 0xB2B4;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;
const SINES: u32 = D + 0xB4BC;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E01F80 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x1DA);
    for (k, node) in [0x20, 0x34, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    for k in 0xE..0x10u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x298 + 4 * (k - 0xE), t);
    }
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    fx.fix_residue(y, 0x01E0_2170);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_21A0, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_21F4);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

fn aim_to_pos(fx: &mut Fx) {
    for k in 0..4 {
        let w = fx.mem.read_u32(AIM + 4 * k);
        fx.mem.write_u32(POS + 4 * k, w);
    }
}

/// `spawn::task400` from the entry: its saves the entry's s0 = 0, s1 = D,
/// s2 = param, s3 (`s3`), the runner's s4 (0 [H]) and `ra`.
fn spawn(fx: &mut Fx, f: u32, param: i32, s3: u64, ra: u32) {
    task400(fx, D, f, 0, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, s3), (0x20, param as i64 as u64), (0x10, u64::from(D)), (0, 0)]);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if (0x97..0x160).contains(&c(fx)) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        fx.node_position(h, slot, AIM);
        let y = ee::add(fx.mem.f(AIM + 4), f32::from_bits(0x4053_3333));
        fx.mem.setf(AIM + 4, y);
        // The arc at the spawn point as it was; the ring at the new one.
        spawn(fx, ARC, param, 0, 0x01E0_22E4);
        aim_to_pos(fx);
        spawn(fx, RING, param, 0, 0x01E0_2320);
    }
    if (0x109..0x1C2).contains(&c(fx)) {
        aim_to_pos(fx);
        for ra in [0x01E0_2380, 0x01E0_2394, 0x01E0_23A8] {
            spawn(fx, ORB, param, 0, ra);
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 || f == IDLE {
            continue;
        }
        // The step's prologue: ra, the entry's s1 = D, s0 = 0.
        let frame = if f == ORB || f == SPARK { 0x30 } else { 0x20 };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        }
        match f {
            ORB => orb(fx, t, k),
            RING => ring(fx, t),
            SPARK => spark(fx, t),
            ARC => arc(fx, t),
            _ => panic!("b428: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    // The entry's s0: the live tasks counted in the draw loop.
    let mut s0 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        s0 += 1;
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        for (g, local) in [(ORB, 0x50u32), (SPARK, 0x70)] {
            if f(fx) == g {
                tex(fx, TEX + 4);
                let c = fx.mem.i16(t + 0x56);
                cell(fx, sp + local, 0x14, c);
                fx.mem.set16(D + 0x9A, 0x3F);
                fx.mem.set16(D + 0xBE, 0);
                billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
            }
            if g == ORB && f(fx) == RING {
                tex(fx, TEX);
                let c = fx.mem.i16(t + 0x56);
                cell(fx, sp + 0x60, 0x14, c);
                fx.mem.set16(D + 0xB8, 4);
                let v = fx.rand_from(s0, 0x01E0_2510);
                fx.mem.set16(D + 0xBE, v % 4);
                fx.mem.set16(D + 0xB0, 0);
                billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
        }
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, AIM);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x122 => 1,
        0x1D1 => 4,
        0x1C7 => 2,
        0x12D..=0x1C1 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// Alpha in by `up` under `cap` while the age is under `until`, else out
/// by `down` (the task ends under `floor`).
fn fade(fx: &mut Fx, t: u32, at: u32, until: i32, cap: u8, up: u8, floor: u8, down: u8) {
    if fx.mem.i16(t + 0x54) < until {
        fade_in(fx, t, at, cap, up);
    } else {
        fade_out(fx, t, at, floor, down);
    }
}

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

/// The orbs' and sparks' first update: size +8 3..5 (+0xA 4 more), cell
/// row (rand >> 4) % 3, grey 0x80; the drift +0x34 / +0x30 (/ +0x38 with
/// `z`) rand % 100 - 50 over `by`; +0x5C random, the camera's angles, the
/// spins +4 / +2 / +0 (rand % 0x800 - 0x400) and the angles +0x20..+0x28
/// random. `ras`: the rands' and 0x205580's return addresses in order.
fn launch(fx: &mut Fx, t: u32, ras: &[u32], z: bool, by: f32) {
    let mut r = ras.iter().copied();
    let v = rmod(fx, t, r.next().unwrap(), 3);
    fx.mem.set16(t + 8, v + 3);
    let s = fx.mem.i16(t + 8) + 4;
    fx.mem.set16(t + 0xA, s);
    let v = fx.rand_from(t, r.next().unwrap());
    let (_, h) = fx.div(v >> 4, 3);
    fx.mem.set16(t + 0x56, h);
    l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
    fx.mem.set16(t + 0x58, 2);
    let axes: &[u32] = if z { &[0x34, 0x30, 0x38] } else { &[0x34, 0x30] };
    for &o in axes {
        let v = rmod(fx, t, r.next().unwrap(), 0x64);
        fx.mem.setf(t + o, ee::from_i32(v - 0x32));
    }
    for &o in axes {
        let q = ee::div(fx.mem.f(t + o), by);
        fx.mem.setf(t + o, q);
    }
    let v = rmod(fx, t, r.next().unwrap(), 0x168);
    fx.mem.set16(t + 0x5C, v);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), r.next().unwrap());
    for o in [4u32, 2, 0] {
        let v = fx.rand_from(t, r.next().unwrap());
        fx.mem.set16(t + o, v % 0x800 - 0x400);
    }
    for o in [0x20u32, 0x24, 0x28] {
        let v = rmod(fx, t, r.next().unwrap(), 0xFFFF);
        fx.mem.set32(t + o, v);
    }
}

/// Every third update (age % 3 == 1) +8 up by 1 under 11 and +0xA by 1
/// while under +8 + 4.
fn swell(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) % 3 == 1 {
        let s = fx.mem.i16(t + 8);
        if s < 0xB {
            fx.mem.set16(t + 8, s + 1);
        }
        let h = fx.mem.i16(t + 0xA);
        if h < fx.mem.i16(t + 8) + 4 {
            fx.mem.set16(t + 0xA, h + 1);
        }
    }
}

/// The angles +0x28 / +0x24 / +0x20 turned by the spins +4 / +2 / +0, the
/// age up, and held at 90 from update 0x1BE.
fn spin_age(fx: &mut Fx, t: u32) {
    for (o, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + o).wrapping_add(fx.mem.i16(t + s) as u32);
        fx.mem.write_u32(t + o, v);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if fx.mem.i16(COUNTER) >= 0x1BE {
        fx.mem.set16(t + 0x54, 0x5A);
    }
}

/// 0x1E014F0 (D, task): a grey orb drifting (-50..49) / 400 in x / y,
/// 1.7 a update with the facing in z and 0.09 down; in by 0x40 under 0x80
/// for 90 updates, out by 9; at age 80 three sparks at its point. `k` the
/// entry's s3 (the task's index) for the sparks' spawn saves.
fn orb(fx: &mut Fx, t: u32, k: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let ras = [
            0x01E0_151C, 0x01E0_1548, 0x01E0_157C, 0x01E0_15A8, 0x01E0_15F4, 0x01E0_1610, 0x01E0_1618, 0x01E0_1638, 0x01E0_1658, 0x01E0_1678, 0x01E0_1694,
            0x01E0_16B0,
        ];
        launch(fx, t, &ras, false, 400.0);
    }
    fade(fx, t, 0x63, 0x5A, 0x80, 0x40, 0xB, 9);
    swell(fx, t);
    if fx.mem.i16(t + 0x54) != 0 {
        let by = f32::from_bits(0x3FD9_999A);
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, by) } else { ee::add(z, by) };
        fx.mem.setf(t + 0x18, z);
        let dx = fx.mem.f(t + 0x30);
        add(fx, t + 0x10, dx);
        let dy = ee::sub(fx.mem.f(t + 0x34), f32::from_bits(0x3DB8_51EC));
        add(fx, t + 0x14, dy);
    }
    if fx.mem.i16(t + 0x54) == 0x50 {
        for o in 0..4 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * o);
            fx.mem.write_u32(POS + 4 * o, w);
        }
        for ra in [0x01E0_1844u32, 0x01E0_1858, 0x01E0_186C] {
            let saves = [(0x50, u64::from(ra)), (0x40, 0), (0x30, u64::from(k)), (0x20, u64::from(t - 0x2A0)), (0x10, u64::from(D)), (0, u64::from(t))];
            task400(fx, D, SPARK, 0, &saves);
        }
    }
    spin_age(fx, t);
}

/// 0x1E018E0 (D, task): b703's grey ring (0x1E02520) with size 2..4:
/// radius 10..49 fixed, life 3..5; in by 0x40 under 0xC0, out by 0xE.
fn ring(fx: &mut Fx, t: u32) {
    let f = RING;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 3);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 2));
        let v = rmod(fx, t, f + 0x50, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x7C, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, f + 0x9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB8, 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, f + 0xD8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xF4, 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0x10 {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01A70 (D, task): a grey spark flying (-50..49) / 100 a update in
/// x / y / z; in by 0x40 under 0x80 for 6 updates, out by 11.
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let ras = [
            0x01E0_1A9C, 0x01E0_1AC8, 0x01E0_1AFC, 0x01E0_1B28, 0x01E0_1B54, 0x01E0_1BAC, 0x01E0_1BC8, 0x01E0_1BD0, 0x01E0_1BF0, 0x01E0_1C10, 0x01E0_1C30,
            0x01E0_1C4C, 0x01E0_1C68,
        ];
        launch(fx, t, &ras, true, 100.0);
    }
    fade(fx, t, 0x63, 6, 0x80, 0x40, 0xD, 0xB);
    swell(fx, t);
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [0u32, 4, 8] {
            let d = fx.mem.f(t + 0x30 + o);
            add(fx, t + 0x10 + o, d);
        }
    }
    spin_age(fx, t);
}

/// 0x1E01DE0 (D, task): an arc ring of radius 3..27 growing by 0.4, outer
/// dark grey 0x20, inner flickering green ((0x20, 0x40, 0x20) on odd ages,
/// (0x10, 0x30, 0x10) on even); outer alpha in by 0x30 under 0xC0 for 4
/// updates, out by 15; turned by the camera's angles (the first +
/// 0x4000).
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_1E04, 0x19);
        fx.mem.setf(t + 0x4C, ee::add(3.0, ee::from_i32(v)));
        l2::rgb(fx, t + 0x64, 0x20, 0x20, 0x20);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0x67, 4, 0xC0, 0x30, 0x11, 0xF);
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x60, 0x20, 0x40, 0x20);
    } else {
        l2::rgb(fx, t + 0x60, 0x10, 0x30, 0x10);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1F1C);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x4C, f32::from_bits(0x3ECC_CCCD));
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}
