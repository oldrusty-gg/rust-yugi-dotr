//! BMODULE b094 (card 304), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E028C0, data area 0x1ED0000) [F]: a pool of 150 tasks of 0x70 bytes at D+0x2B0 (alive +0x58, step
//! +0x44 through `jalr`; spawn 0x1E02FC0(D, step, 0) with the colour
//! D+0x44C4 and the index D+0x455C into +0x50; 0x1E03100 also copies the
//! turn D+0x260 into +0x20), no unit flash. Drawn with flat arcs
//! (0x1E00920, `billboard::arc_strips_flat`), raised turned quads
//! (0x1E01230, `quads4::quad_turned_raised`), quad rings (0x1E00C80,
//! `billboard::ring_quads` on the bytes D+0x4566, cell orientation rand % 4
//! in the draw loop), the record's own textured band 0x1E03260 (up to 18
//! strip3 quads 20 degrees apart, as many as D+0x4556 / 20, each cell from
//! the table D+0x4510) and gouraud fan 0x1E03570 (D+0xB8 quads +0x5C
//! degrees wide, depth mode 0 keyed / 1 far Z). Globals: the swinging
//! point D+0x4480 (the attacker's node 2 at y 26 plus D+0x44D4 x sin
//! D+0x4552, drifting towards the target), its base D+0x4490, textures (5,
//! 0..2) D+0x44A0.., timer D+0x44C0 = 254, colour D+0x44C4, OT D+0x44CC,
//! the swing D+0x44D4 (30, down by 1 from 0xAA, 0 at 0xC8), nodes D+0x44F0..
//! (attacker 1 (clear) / 2 / 0xD), target 1 at D+0x4500, cells D+0x4510..,
//! angle D+0x4552, band limit D+0x4556 (up by 20 to 360 from 0x42),
//! facing D+0x4560, param D+0x4562, counter D+0x4564, bytes D+0x4566 /
//! D+0x4666, sines D+0x4768. Returns 1 / 3 / 6 / 8 on 0xC8 / 0xF0 /
//! 0xD3..0xEA / timer out.
use crate::b494::camera_angles_saving;
use crate::blib::{afterimage, billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0x96;
const BAND: u32 = 0x01E0_1900;
const QUAD: u32 = 0x01E0_1AF0;
const ARC: u32 = 0x01E0_2000;
const RING: u32 = 0x01E0_20E0;
const FAN: u32 = 0x01E0_2290;
const ARC_P: u32 = 0x01E0_2480;
const ARC_B: u32 = 0x01E0_25F0;
const ARC_C: u32 = 0x01E0_26F0;
const LIGHT: u32 = 0x01E0_27F0;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_2B60;
const POS: u32 = D + 0x250;
const TURN: u32 = D + 0x260;
const POINT: u32 = D + 0x4480;
const BASE: u32 = D + 0x4490;
const TEX: u32 = D + 0x44A0;
const TIMER: u32 = D + 0x44C0;
const COLOUR: u32 = D + 0x44C4;
const OT_AT: u32 = D + 0x44CC;
const SWING: u32 = D + 0x44D4;
const NODES: u32 = D + 0x44F0;
const TARGET_NODE: u32 = D + 0x4500;
const CELLS: u32 = D + 0x4510;
const ANGLE: u32 = D + 0x4552;
const LIMIT: u32 = D + 0x4556;
const INDEX: u32 = D + 0x455C;
const FACING: u32 = D + 0x4560;
const COUNTER: u32 = D + 0x4564;
const BYTES: u32 = D + 0x4566;
const BYTES_B: u32 = D + 0x4666;
const SINES: u32 = D + 0x4768;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn node(fx: &mut Fx, k: u32) -> i32 {
    fx.mem.i16(NODES + 2 * k)
}

/// The entry 0x1E028C0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x48E0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4562, param);
    fx.mem.set32(TIMER, 0xFE);
    for (k, n) in [1, 2, 0xD].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for k in 0..3u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    fx.fix_residue(v, 0x01E0_29E0);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(k, 0x01E0_2A08);
        let v = fx.div(r, 0x1E).1;
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 100..0xB4u32 {
        fx.mem.write_u8(BYTES + k, 0);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(k, 0x01E0_2A8C);
        let v = fx.div(r, 0x28).1;
        fx.mem.write_u8(BYTES_B + k, (v - 0x14) as u8);
    }
    fx.mem.write_u32(SWING, 0x41F0_0000);
    for k in 0..0x1Eu32 {
        let r = fx.rand_from(k, 0x01E0_2ADC);
        fx.mem.set16(CELLS + 2 * k, r.wrapping_rem(8));
    }
    0
}

/// The atlas cell `id` (0x106960 into `r`, frame `frame`) at D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, frame: i32) {
    l2::rect(fx, r, id, frame);
    copy4(fx, D + 0x94, r);
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

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x308) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2F4);
        if f == 0 {
            continue;
        }
        run_task(fx, f, slot + 0x2B0, [0, u64::from(D), u64::from(slot), u64::from(k), 0]);
    }
    let mut drawn = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x308) == 0 {
            continue;
        }
        drawn = (drawn as i16).wrapping_add(1) as i32 as u32;
        draw(fx, sp, slot + 0x2B0, drawn);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0xC8 => 1,
        0xF0 => 3,
        0xD3..=0xEA => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw case for task `t` (`drawn` its s0).
fn draw(fx: &mut Fx, sp: u32, t: u32, drawn: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    if f == BAND {
        tex(fx, TEX);
        let fr = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x60, 0x14, fr);
        fx.mem.set16(D + 0xB8, 1);
        fx.mem.set16(D + 0xBE, 0);
        band(fx, t, 0x20, POINT);
    }
    if f == QUAD {
        tex(fx, TEX + 4);
        for (k, v) in [1, 1, 0xFF, 0x3F].into_iter().enumerate() {
            fx.mem.set16(D + 0x94 + 2 * k as u32, v);
        }
        fx.mem.set16(D + 0xBE, 5);
        copy3(fx, sp + 0x50, t + 0x10);
        quads4::quad_turned_raised(fx, uv::uv_rect, D, t, 0x11, sp + 0x50);
    }
    if f == ARC {
        flat(fx, t, 0x20, t + 0x10);
    }
    if f == RING {
        tex(fx, TEX + 8);
        let fr = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x70, 0x14, fr);
        fx.mem.set16(D + 0xB8, 5);
        let r = fx.rand_from(drawn, 0x01E0_2D20);
        fx.mem.set16(D + 0xBE, r.wrapping_rem(4));
        fx.mem.set16(D + 0xB0, 0);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == FAN {
        fx.mem.set16(D + 0xB8, 0x18);
        let mode = if counter(fx) < 0x96 { 0x21 } else { 0x20 };
        if fx.mem.i16(t + 0x50) == 0 {
            fan(fx, t, mode, POINT);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fan(fx, t, mode, t + 0x10);
        }
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == ARC_P {
        fx.mem.set16(D + 0xB8, 0x18);
        if fx.mem.i16(t + 0x50) == 0 {
            billboard::arc_strips_flat(fx, D, t, 0x20, POINT);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            billboard::arc_strips_flat(fx, D, t, 0x20, t + 0x10);
        }
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == ARC_B || f == ARC_C {
        flat(fx, t, 0x20, t + 0x10);
    }
}

fn flat(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    fx.mem.set16(D + 0xB8, 0x18);
    billboard::arc_strips_flat(fx, D, t, mode, pos);
    fx.mem.set16(D + 0xB8, 0);
}

/// `regs`: the run loop's s0 (0), s1 (D), s2 (the slot), s3 (its index),
/// s4 (the runner's, 0 [H]) at the `jalr`.
fn run_task(fx: &mut Fx, f: u32, t: u32, regs: [u64; 5]) {
    match f {
        BAND => band_step(fx, t, regs),
        QUAD => quad(fx, t, regs),
        ARC => arc(fx, t, regs),
        RING => ring(fx, t, regs),
        FAN => fan_step(fx, t, regs, true),
        ARC_P => fan_step(fx, t, regs, false),
        ARC_B => arc_grow(fx, t, regs, 2, (0x10, 0xE), 25.0, 20.0),
        ARC_C => arc_grow(fx, t, regs, 5, (0xD, 0xB), 15.0, 50.0),
        LIGHT => light(fx, t, regs),
        _ => panic!("b094: unknown task step {f:#x}"),
    }
}

/// A step's prologue: `frame` bytes, the caller's s0.. (`n` of `regs`) at
/// +0x10 k, ra at `ra_at`.
fn enter_step(fx: &mut Fx, frame: u32, regs: [u64; 5], n: usize, ra_at: u32) -> u32 {
    let sp = fx.enter(frame);
    for k in 0..n {
        fx.saved(sp + 0x10 * k as u32, &[(0, regs[k])]);
    }
    fx.saved(sp, &[(ra_at, TASK_RA)]);
    sp
}

/// 0x1E02FC0 (or 0x1E03100 when `turned`)(D, step, 0) with the caller's
/// s0..s4 = `regs`.
fn spawn(fx: &mut Fx, f: u32, turned: bool, regs: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, regs[4]), (0x30, regs[3]), (0x20, regs[2]), (0x10, regs[1]), (0, regs[0])]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x308) == 0) {
        Some(k) => {
            let t = D + 0x2B0 + 0x70 * k;
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
            if turned {
                copy4(fx, t + 0x20, TURN);
            }
            copy3(fx, t + 0x10, POS);
            let i = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x50, i);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand % m (HI).
fn rand_mod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// 0x1E01570(D, 0) (frame 0x50: the entry's s0 = 0, s1 = D, ra).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_2B30), (0x10, u64::from(D)), (0, 0)]);
    let regs = |s0: u64| [s0, u64::from(D), param as i64 as u64, 0, 0];
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    let c = counter(fx);
    if (0x64..0xC9).contains(&c) {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x30);
        let h = fx.self_handle();
        fx.unit_position(h, sp + 0x40);
        let h = fx.target_handle();
        let s = fx.mem.i16(TARGET_NODE);
        crate::b076::node_position_saving(fx, h, s, sp + 0x30, 0, Some(u64::from(D)), 0x01E0_15E8);
        let d = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48)), 100.0);
        let v = ee::add(fx.mem.f(BASE + 8), d);
        fx.mem.setf(BASE + 8, v);
        let a = fx.mem.i16(ANGLE) + 8;
        fx.mem.set16(ANGLE, a);
        let a = fx.mem.i16(ANGLE);
        if a >= 0x168 {
            let r = fx.div(a, 0x168).1;
            fx.mem.set16(ANGLE, r);
        }
        if counter(fx) >= 0xAA {
            let s = fx.mem.f(SWING);
            if !crate::exe::ee_le(s, 0.0) {
                fx.mem.setf(SWING, ee::sub(s, 1.0));
            }
        }
        if counter(fx) == 0xC8 {
            fx.mem.write_u32(SWING, 0);
        }
    }
    copy4(fx, POINT, BASE);
    let a = fx.mem.i16(ANGLE);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let v = ee::add(fx.mem.f(POINT), ee::mul(fx.mem.f(SWING), s));
    fx.mem.setf(POINT, v);
    if counter(fx) == 1 {
        let h = fx.self_handle();
        let s = node(fx, 0);
        crate::b076::node_position_saving(fx, h, s, POS, 0, Some(u64::from(D)), 0x01E0_1700);
        let h = fx.self_handle();
        let s = node(fx, 1);
        crate::b076::node_position_saving(fx, h, s, BASE, 0, Some(u64::from(D)), 0x01E0_1718);
        fx.mem.write_u32(BASE + 4, 0x41D0_0000);
        copy4(fx, POINT, BASE);
        spawn(fx, BAND, false, regs(0), 0x01E0_1754);
        fx.mem.set16(INDEX, 0);
        spawn(fx, FAN, false, regs(0), 0x01E0_176C);
        spawn(fx, ARC_P, false, regs(0), 0x01E0_1780);
        copy4(fx, POS, POINT);
        fx.mem.set16(INDEX, 1);
        spawn(fx, FAN, false, regs(0), 0x01E0_17BC);
        spawn(fx, ARC_P, false, regs(0), 0x01E0_17D0);
        spawn(fx, LIGHT, false, regs(0), 0x01E0_17E4);
    }
    let c = counter(fx);
    if (0xCB..0xD6).contains(&c) {
        copy4(fx, POS, POINT);
        for (o, ra) in [(0u32, 0x01E0_182Cu32), (8, 0x01E0_1860)] {
            let r = rand_mod(fx, 0, ra, 0x28);
            let v = ee::add(fx.mem.f(POS + o), ee::from_i32(r - 0x14));
            fx.mem.setf(POS + o, v);
        }
        spawn(fx, QUAD, false, regs(0), 0x01E0_189C);
    }
    if counter(fx) >= 0x42 {
        let v = fx.mem.i16(LIMIT);
        if v < 0x168 {
            fx.mem.set16(LIMIT, v + 0x14);
        }
    }
    fx.leave(0x50);
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

/// 0x1E01900 (frame 0x30): the band at the point (radii 20 / 13, cell
/// rand % 7, row rand % 3 + 4), in by 0xC below 0x80 for 20 updates, out
/// by 6 below timer 30; grey by age parity; faced to the attacker's side
/// to 0x96, then turning 0x100 a update.
fn band_step(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x30, regs, 2, 0x20);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(20.0, 7.0));
        let r = rand_mod(fx, t, 0x01E0_1948, 7);
        fx.mem.set16(t + 0x56, r);
        fx.mem.set16(t + 0x58, 2);
        let r = rand_mod(fx, t, 0x01E0_196C, 3);
        fx.mem.set16(t + 8, r + 4);
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        fade_in(fx, t, 0x63, (0x80, 0xC));
        fade_in(fx, t, 0x67, (0x80, 0xC));
    } else if fx.mem.i32(TIMER) < 0x1E {
        fade_out(fx, t, 0x63, (8, 6));
        let v = fx.mem.read_u8(t + 0x67);
        if v >= 8 {
            fx.mem.write_u8(t + 0x67, v - 6);
        }
    }
    let g = if odd(fx, t) { 0x80 } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    l2::rgb(fx, t + 0x64, g, g, g);
    if counter(fx) < 0x96 {
        let v = if fx.mem.i16(FACING) != 0 { 0 } else { 0x8000 };
        fx.mem.set32(t + 0x24, v);
    } else {
        let v = fx.mem.i32(t + 0x24).wrapping_add(0x100);
        fx.mem.set32(t + 0x24, v);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E01AF0 (frame 0x90): a white raised quad dropping from 1000 by 80,
/// turned randomly; age 10 a flat arc 0x1E025F0 at it raised by the drop /
/// 10 turned with it (0x1E03100 with D+0x260 / D+0x264 = -180 x the
/// turns), age 12 an arc 0x1E02000 at the target (jittered), age 16 an arc
/// 0x1E026F0 and two rings 0x1E020E0 (jittered) at it; in by 0x2A below
/// 0x80 for 3 updates, out by 9 twice from 0xF1 once landed.
fn quad(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    let sp = enter_step(fx, 0x90, regs, 5, 0x50);
    let (mut s0, mut s1) = (regs[0], regs[1]);
    if first(fx, t) {
        fx.mem.set16(t + 8, 0x14);
        fx.mem.set16(t + 0xA, 0x98);
        fx.mem.write_u32(t + 0x34, 0x447A_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let r = rand_mod(fx, t, 0x01E0_1B70, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rand_mod(fx, t, 0x01E0_1B8C, 0x1E);
        fx.mem.set16(t + 0x5C, r - 0xF);
    }
    if fx.mem.i16(t + 0x54) < 3 {
        fade_in(fx, t, 0x63, (0x80, 0x2A));
        fade_in(fx, t, 0x67, (0x80, 0x2A));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let d = fx.mem.f(t + 0x34);
        if !crate::exe::ee_le(d, 0.0) {
            fx.mem.setf(t + 0x34, ee::sub(d, 80.0));
        } else if counter(fx) >= 0xF1 {
            fade_out(fx, t, 0x63, (0xB, 9));
            let v = fx.mem.read_u8(t + 0x63);
            if v >= 0xB {
                fx.mem.write_u8(t + 0x63, v - 9);
            }
        }
    }
    let sregs = |s0: u64| [s0, regs[1], regs[2], u64::from(t), u64::from(D)];
    if fx.mem.i16(t + 0x54) == 0x10 {
        copy4(fx, POS, t + 0x10);
        spawn(fx, ARC_C, false, sregs(s0), 0x01E0_1CB4);
        for k in 0..2u64 {
            s0 = k;
            copy4(fx, POS, t + 0x10);
            for (o, ra) in [(0u32, 0x01E0_1CE0u32), (8, 0x01E0_1D14)] {
                let r = rand_mod(fx, k as u32, ra, 0x14);
                let v = ee::add(fx.mem.f(POS + o), ee::from_i32(r - 0xA));
                fx.mem.setf(POS + o, v);
            }
            spawn(fx, RING, false, sregs(s0), 0x01E0_1D50);
        }
        s0 = 2;
    }
    if fx.mem.i16(t + 0x54) == 0xC {
        s0 = 0;
        let h = fx.target_handle();
        let s = fx.mem.i16(TARGET_NODE);
        crate::b076::node_position_saving(fx, h, s, POS, s0, Some(s1), 0x01E0_1DA8);
        for (o, ra, m, half) in [(0u32, 0x01E0_1DB0u32, 0x1E, 0xF), (8, 0x01E0_1DE4, 0x14, 0xA), (4, 0x01E0_1E18, 0x14, 0xA)] {
            let r = rand_mod(fx, 0, ra, m);
            let v = ee::add(fx.mem.f(POS + o), ee::from_i32(r - half));
            fx.mem.setf(POS + o, v);
        }
        spawn(fx, ARC, false, sregs(s0), 0x01E0_1E54);
        s0 = 1;
    }
    if fx.mem.i16(t + 0x54) == 0xA {
        copy4(fx, POS, t + 0x10);
        s1 = u64::from(sp + 0x64);
        s0 = u64::from(sp + 0x74);
        let d = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(sp + 0x64, d);
        fx.mem.write_u32(sp + 0x60, 0);
        fx.mem.write_u32(sp + 0x68, 0);
        let (a, b) = (fx.mem.i16(t + 0x5C), fx.mem.i16(t + 0x5A));
        fx.mem.set32(sp + 0x70, a);
        fx.mem.set32(sp + 0x74, b);
        fx.mem.set32(sp + 0x78, a);
        camera_angles_saving(fx, sp + 0x80, s0, s1, 0x01E0_1EE8);
        let yaw = fx.mem.i32(sp + 0x84);
        fx.mult(0xB60B_60B7_u32 as i32, yaw);
        let v = fx.mem.i32(sp + 0x74).wrapping_sub(yaw / 180);
        fx.mem.set32(sp + 0x74, v);
        l2::rotate_deg(fx, sp + 0x60, sp + 0x70, 2, SINES);
        l2::rotate_deg(fx, sp + 0x60, sp + 0x70, 0, SINES);
        for c in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(POS + c), fx.mem.f(sp + 0x60 + c));
            fx.mem.setf(POS + c, v);
        }
        let a = fx.mem.i16(t + 0x5A);
        fx.mem.set32(TURN + 4, a.wrapping_mul(-180));
        let b = fx.mem.i16(t + 0x5C);
        fx.mem.set32(TURN, b.wrapping_mul(-180));
        spawn(fx, ARC_B, true, [s0, s1, u64::from(sp + 0x68), u64::from(t), u64::from(D)], 0x01E0_1FBC);
    }
    camera_angles_saving(fx, t + 0x20, s0, s1, 0x01E0_1FC8);
    fx.mem.write_u32(t + 0x20, 0);
    age(fx, t);
    fx.leave(0x90);
}

/// 0x1E02000: a red-white camera-facing arc of radius 80, 0x80 for 2
/// updates, then out by 0xE.
fn arc(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x20, regs, 1, 0x10);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x42A0_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 2 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, (0x10, 0xE));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), regs[1], 0x01E0_20AC);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E020E0: a white quad ring (size rand % 15 + 5, cell rand % 8, radius
/// rand % 30 + 5 growing by rand % 15 + 6, row rand % 35), in by 0x40
/// below 0xC0 for rand % 9 + 3 updates, then out by 0x10.
fn ring(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x20, regs, 1, 0x10);
    if first(fx, t) {
        let r = rand_mod(fx, t, 0x01E0_2104, 0xF);
        fx.mem.setf(t + 0x48, ee::from_i32(r + 5));
        let r = fx.rand_from(t, 0x01E0_2130);
        fx.mem.set16(t + 0x56, r.wrapping_rem(8));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let r = rand_mod(fx, t, 0x01E0_2164, 0x1E);
        fx.mem.set16(t + 0x50, r + 5);
        let r = rand_mod(fx, t, 0x01E0_2184, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rand_mod(fx, t, 0x01E0_21A0, 0xF);
        fx.mem.set16(t + 0xE, r + 6);
        let r = rand_mod(fx, t, 0x01E0_21C0, 0x23);
        fx.mem.set16(t + 8, r);
        let r = rand_mod(fx, t, 0x01E0_21DC, 9);
        fx.mem.set16(t + 6, r + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, (0xC0, 0x40));
    } else {
        fade_out(fx, t, 0x63, (0x12, 0x10));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02290 (`fan`) / 0x1E02480 (frame 0x30): kind +0x50 0 at the point,
/// 1 at the attacker's node 1 (y 26) from 0x65; radius 10 x the target's
/// radius, at most 220 and at least 80 (`fan`) or at most 200; in by 0xC
/// below 0x80 for 10 updates, out by 9 below timer 30; the fan red by age
/// parity, its +0x34 1300.
fn fan_step(fx: &mut Fx, t: u32, regs: [u64; 5], is_fan: bool) {
    enter_step(fx, 0x30, regs, 2, 0x20);
    if first(fx, t) {
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        if is_fan {
            fx.mem.write_u32(t + 0x34, 0x44A2_8000);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, (0x80, 0xC));
    } else if fx.mem.i32(TIMER) < 0x1E {
        fade_out(fx, t, 0x63, (0xB, 9));
    }
    if is_fan {
        if odd(fx, t) {
            l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        } else {
            l2::rgb(fx, t + 0x60, 0x80, 0x40, 0x40);
        }
    }
    if fx.mem.i16(t + 0x50) == 1 && counter(fx) >= 0x65 {
        let h = fx.self_handle();
        let s = node(fx, 0);
        let ra = if is_fan { 0x01E0_23E8 } else { 0x01E0_2578 };
        crate::b076::node_position_saving(fx, h, s, t + 0x10, u64::from(t), Some(u64::from(D)), ra);
        fx.mem.write_u32(t + 0x14, 0x41D0_0000);
    }
    fx.mem.write_u32(t + 0x48, 0x4334_0000);
    fx.mem.write_u32(t + 0x4C, 0);
    let h = fx.target_handle();
    let r = ee::mul(10.0, afterimage::unit_radius(fx, h));
    fx.mem.setf(t + 0x48, r);
    let top = if is_fan { 220.0 } else { 200.0 };
    if !crate::exe::ee_le(r, top) {
        fx.mem.setf(t + 0x48, top);
    }
    if is_fan && crate::exe::ee_lt(fx.mem.f(t + 0x48), 80.0) {
        fx.mem.write_u32(t + 0x48, 0x42A0_0000);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E025F0 / 0x1E026F0: a violet-white flat arc (inner +0x48 from 20
/// growing by `grow`, outer `gap` inside it), 0x80 for `hold` updates, then
/// out.
fn arc_grow(fx: &mut Fx, t: u32, regs: [u64; 5], hold: i32, down: (u8, u8), grow: f32, gap: f32) {
    enter_step(fx, 0x20, regs, 1, 0x10);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(20.0, 58.0));
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < hold {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, down);
    }
    let r = ee::add(fx.mem.f(t + 0x48), grow);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::sub(r, gap));
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E027F0: the stage light, 0x80 down by 10 to 0x50 while the timer is
/// above 0x1E, then up by 10 to 0x80; on timer 3 back to 0x80 and over.
fn light(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x20, regs, 1, 0x10);
    if first(fx, t) {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let v = fx.mem.i16(t + 0x50);
    if fx.mem.i32(TIMER) >= 0x1F {
        fx.mem.set16(t + 0x50, if v < 0x51 { 0x50 } else { v - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if v < 0x80 { v + 0xA } else { 0x80 });
    }
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
    fx.leave(0x20);
}

/// 0x1E03260(D, t, mode, pos) (frame 0x1E0): up to 18 textured strip3
/// quads of a flat band (inner +0x4C, outer +0x48) 20 degrees each from
/// +0x5A - 200, as long as the angle stays below D+0x4556; cell k from
/// D+0x4510 + 2k; turned +0x20 at `pos`, keyed by mean w.
fn band(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1E0);
    copy4(fx, sp + 0x90, pos);
    let d = sp + 0x150;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 1);
    l2::colour_strip3(fx, d, t + 0x64, 2);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    fx.desc_alpha(d, crate::c061::blend_alpha(mode >> 4));
    copy3(fx, sp + 0xE0, sp + 0x90);
    l2::xform_vp(fx, sp + 0xF0, 0, t + 0x20, sp + 0xE0);
    for o in [0xA4u32, 0xB4, 0xC4, 0xD4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let (f20, f21) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let mut s1 = 0i32;
    for k in 0..0x12u32 {
        if s1 >= fx.mem.i16(LIMIT) {
            continue;
        }
        let fr = fx.mem.i16(CELLS + 2 * k);
        cell(fx, sp + 0x140, 0x14, fr);
        let order = fx.mem.i16(D + 0xBE);
        l2::uv_strip3(fx, d, D + 0x94, order);
        for (a0, outer, inner) in [(0i32, 0xC0u32, 0xA0u32), (0x14, 0xD0, 0xB0)] {
            let a = (fx.mem.i16(t + 0x5A) + a0 + s1 - 0xC8) as i16 as i32;
            let c = l2::sin_deg(fx, SINES, a, 1);
            let s = l2::sin_deg(fx, SINES, a, 0);
            fx.mem.setf(sp + outer, ee::mul(c, f20));
            fx.mem.setf(sp + outer + 8, ee::mul(s, f20));
            fx.mem.setf(sp + inner, ee::mul(c, f21));
            fx.mem.setf(sp + inner + 8, ee::mul(s, f21));
        }
        if l2::project(fx, sp + 0xA0, 0x7000_0000, sp + 0xF0, d, sp + 0x130) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x130)) as i32;
            let ot = fx.mem.read_u32(OT_AT);
            fx.submit(d, ot, key);
        }
        s1 = (s1 + 0x14) as i16 as i32;
    }
    fx.leave(0x1E0);
}

/// 0x1E03570(D, t, mode, pos) (frame 0x200): D+0xB8 gouraud quads of a fan
/// (outer +0x48, inner +0x4C, height +0x34 at the inner edge) each +0x5C
/// degrees wide from +0x5A, stepping +0x5A, scaled 0.1 and shifted 10 x
/// the point's x, turned +0x20 at (0, y, z); depth mode 0 keyed by mean
/// w, 1 the far Z at key 0.
fn fan(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x200);
    copy4(fx, sp + 0xB0, pos);
    let wide = fx.mem.i16(t + 0x5C);
    let d = sp + 0x170;
    l2::colour_strip4(fx, d, t + 0x60, 2);
    l2::colour_strip4(fx, d, t + 0x64, 1);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    fx.desc_alpha(d, crate::c061::blend_alpha((mode >> 4) as i16 as i32));
    fx.mem.write_u32(sp + 0x100, 0);
    for o in [0x110u32, 0x114, 0x118] {
        fx.mem.write_u32(sp + o, 0x3DCC_CCCD);
    }
    let (y, z) = (fx.mem.read_u32(sp + 0xB4), fx.mem.read_u32(sp + 0xB8));
    fx.mem.write_u32(sp + 0x104, y);
    fx.mem.write_u32(sp + 0x108, z);
    l2::xform_vp(fx, sp + 0x120, sp + 0x110, t + 0x20, sp + 0x100);
    fx.mem.write_u32(sp + 0xC4, 0);
    fx.mem.write_u32(sp + 0xD4, 0);
    let h = fx.mem.read_u32(t + 0x34);
    fx.mem.write_u32(sp + 0xE4, h);
    fx.mem.write_u32(sp + 0xF4, h);
    let depth = mode & 0xF;
    let shift = ee::mul(10.0, fx.mem.f(sp + 0xB0));
    let (f20, f21) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let mut s2 = 0i32;
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0xB8) {
        for (a0, outer, inner) in [(0i32, 0xC0u32, 0xE0u32), (wide, 0xD0, 0xF0)] {
            let a = (fx.mem.i16(t + 0x5A) + a0 + s2) as i16 as i32;
            let c = l2::sin_deg(fx, D + 0xD8, a, 1);
            fx.mem.setf(sp + outer, ee::mul(f20, c));
            let s = l2::sin_deg(fx, D + 0xD8, a, 0);
            fx.mem.setf(sp + outer + 8, ee::mul(f20, s));
            let c = l2::sin_deg(fx, D + 0xD8, a, 1);
            fx.mem.setf(sp + inner, ee::mul(f21, c));
            let s = l2::sin_deg(fx, D + 0xD8, a, 0);
            fx.mem.setf(sp + inner + 8, ee::mul(f21, s));
        }
        for o in [0xC0u32, 0xD0] {
            let v = ee::add(fx.mem.f(sp + o), shift);
            fx.mem.setf(sp + o, v);
        }
        if depth == 0 {
            if l2::project(fx, sp + 0xC0, 0x7000_0000, sp + 0x120, d, sp + 0x160) {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
                let ot = fx.mem.read_u32(D + 0x80);
                fx.submit(d, ot, key);
            }
        } else if depth == 1 && l2::project(fx, sp + 0xC0, 0x7000_0000, sp + 0x120, d, sp + 0x160) {
            l2::far_z_desc(fx, d, 1);
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, 0);
        }
        s2 = (s2 + fx.mem.i16(t + 0x5A)) as i16 as i32;
        k += 1;
    }
    fx.leave(0x200);
}
