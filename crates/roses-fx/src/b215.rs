//! BMODULE b215 (card 89), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02DF0, data area 0x1ED0000, 0xE9F0 bytes) [F]
//!: a pool of 130 tasks of 0x70 bytes at D+0x2A0
//! (alive +0x58, step +0x44 through `jalr`; spawn 0x1E038D0 =
//! `spawn::task70` on 6fd147a7dc0d's layout: colour D+0x9FF4, the point
//! D+0x250, the index D+0xE66E into +0x50). The tasks: two grids of 36
//! spinning petals about the attacker's node 1 (0x1E01500 stepping the
//! angle tables D+0xB460 / 0xC0E0 / 0xCD60 + 0x50 k and the lives
//! D+0xD9E0, drawn by 0x1E03A60), a 20 x 28 grid of quads about the
//! target's node 1 lifted where they fall inside the growing radius
//! D+0x9FC0 (0x1E01A70, drawn by 0x1E03FB0 on the cells D+0xA048 + 0x64 j
//! and the jitters D+0x3BC0 + 0x280 j), rising billboards (0x1E01EA0 /
//! 0x1E01FF0, `billboard::quad` on `uv::grid_rect` cells), a flat arc
//! fan (0x1E02140, `billboard::arc_strips`), the stage light (0x1E022A0),
//! a window sprite (0x1E02390), and in a packet layer (0x1E044C0) a frame
//! lens of 24 segments (0x1E02530 + 0x1E02650) and the attacker drawn
//! into a full-window sprite (0x1F8270, 0x1E02B30). Returns 1 / 4 / 2 / 6
//! / 8 on 0x8A / 0x104 / 0xFF / 0x95..0xF9 / timer out.
use crate::blib::{billboard, spawn as bspawn, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 130;
const PETALS: u32 = 0x01E0_1500;
const GRID: u32 = 0x01E0_1A70;
const RISE: u32 = 0x01E0_1EA0;
const RISE_B: u32 = 0x01E0_1FF0;
const FAN: u32 = 0x01E0_2140;
const LIGHT: u32 = 0x01E0_22A0;
const WINDOW: u32 = 0x01E0_2390;
const LENS: u32 = 0x01E0_2530;
const SHADOW: u32 = 0x01E0_2B30;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_34B0;
const POOL: bspawn::Pool70 = bspawn::Pool70 { base: 0x2A0, n: 130, colour: 0x9FF4, pos: 0x250, slot: 0xE66E };
const POS: u32 = D + 0x250;
const TEX4: u32 = D + 0x29C;
const JIT: u32 = D + 0x3BC0;
const RADIUS: u32 = D + 0x9FC0;
const GROW: u32 = D + 0x9FC4;
const TEX5: u32 = D + 0x9FD0;
const TIMER: u32 = D + 0x9FF0;
const OT_AT: u32 = D + 0x9FFC;
const LAYER: u32 = D + 0xA004;
const SPRITE_PTR: u32 = D + 0xA008;
const QUAD_PTR: u32 = D + 0xA00C;
const SELF_NODE: u32 = D + 0xA028;
const TARGET_NODE: u32 = D + 0xA038;
const CELLS40: u32 = D + 0xA048;
const CELLS_A: u32 = D + 0xB3D0;
const CELLS_B: u32 = D + 0xB418;
const SLOT: u32 = D + 0xE66E;
const FACING: u32 = D + 0xE672;
const COUNTER: u32 = D + 0xE676;
const BYTES: u32 = D + 0xE678;
const BYTES_B: u32 = D + 0xE778;
const SINES: u32 = D + 0xE878;
/// The scratchpad, where the originals project.
const SPR: u32 = 0x7000_0000;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn rand_mod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02DF0 (frame 0xC0: s0 = D, s1 = the lens / shadow tasks
/// counted, s2 = the param, s3 / s2 the step loop's index / slot, s4 the
/// drawn count).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xE9F0);
    l2::sine_table_saving(fx, SINES, u64::from(D), 0, 0x01E0_2E50);
    l2::sine_table_saving(fx, D + 0xD8, u64::from(D), 0, 0x01E0_2E58);
    fx.mem.set16(D + 0xE674, param);
    fx.mem.set32(TIMER, 0x10E);
    let s = [Some(u64::from(D)), Some(0), Some(param as i64 as u64)];
    for (k, (n, clear, ra)) in [(1, 1, 0x01E0_2E88u32), (0x34, 0, 0x01E0_2EA8), (0x35, 0, 0x01E0_2EC8), (0x30, 0, 0x01E0_2EE8), (0x2F, 0, 0x01E0_2F08), (0x2C, 0, 0x01E0_2F28)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = crate::b076::request_node_saving(fx, h, n, clear, s, ra);
        fx.mem.set16(SELF_NODE + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = crate::b076::request_node_saving(fx, t, 1, 1, s, 0x01E0_2F48);
    fx.mem.set16(TARGET_NODE, r);
    let tex = fx.texture(4, 0xF);
    fx.mem.write_u32(TEX4, tex);
    for k in 0..2u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX5 + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(v, 0x01E0_2FE8);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rand_mod(fx, D, 0x01E0_3018, 0x1E);
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 100..0xB4u32 {
        let r = fx.rand_from(D, 0x01E0_3074);
        fx.mem.write_u8(BYTES + k, (r & 3) as u8);
    }
    for k in 0..0x100u32 {
        let v = rand_mod(fx, D, 0x01E0_30CC, 0x28);
        fx.mem.write_u8(BYTES_B + k, (v - 0x14) as u8);
    }
    for k in 0..0x24u32 {
        let r = fx.rand_from(D, 0x01E0_3120);
        fx.mem.set16(CELLS_A + 2 * k, r & 7);
    }
    for k in 0..0x24u32 {
        let r = fx.rand_from(D, 0x01E0_3178);
        fx.mem.set16(CELLS_B + 2 * k, r & 7);
    }
    for j in 0..0x28u32 {
        for k in 0..0x28u32 {
            let a = CELLS40 + 0x64 * j + 2 * k;
            let v = rand_mod(fx, D, 0x01E0_31DC, 0x50);
            fx.mem.set16(a, v);
            let v = fx.mem.i16(a);
            let hi = ((i64::from(v) * 0x6666_6667) >> 32) as i32;
            fx.mult(0x6666_6667, v);
            fx.mem.set16(a, (hi >> 2) + ((v as u32) >> 31) as i32);
        }
    }
    for j in 0..2u32 {
        for k in 0..0x28u32 {
            let a = JIT + 0x280 * j + 0x10 * k;
            for (o, ra) in [(0u32, 0x01E0_3294u32), (4, 0x01E0_32C0), (8, 0x01E0_32EC)] {
                let v = rand_mod(fx, D, ra, 0x1E);
                fx.mem.setf(a + o, ee::from_i32(v - 0xF));
            }
            for o in [0u32, 4, 8] {
                let v = ee::div(fx.mem.f(a + o), 10.0);
                fx.mem.setf(a + o, v);
            }
        }
    }
    for j in 0..2u32 {
        let s0 = D + 0x50 * j;
        for k in 0..0x24u32 {
            let a = s0 + 2 * k;
            let v = rand_mod(fx, s0, 0x01E0_33A0, 0x168);
            fx.mem.set16(a + 0xB460, v);
            let v = rand_mod(fx, s0, 0x01E0_33BC, 0x168);
            fx.mem.set16(a + 0xC0E0, v);
            let v = rand_mod(fx, s0, 0x01E0_33D8, 0x168);
            fx.mem.set16(a + 0xCD60, v);
            fx.mem.set16(a + 0xD9E0, 0xA);
        }
    }
    0
}

/// The atlas cell (0x106960(r, 0x14, +0x56, 0)) into D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, frame);
    copy4(fx, D + 0x94, r);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    let mut counted = 0i32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 {
            continue;
        }
        if f == LENS || f == SHADOW {
            counted = (counted + 1) as i16 as i32;
            continue;
        }
        let e = [u64::from(D), counted as i64 as u64, u64::from(slot), u64::from(k), 0];
        run_task(fx, f, slot + 0x2A0, e);
    }
    let mut drawn = 0i32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        drawn = (drawn + 1) as i16 as i32;
        draw(fx, sp, slot + 0x2A0, counted);
    }
    if counted != 0 {
        layer(fx, counted, drawn);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x8A => 1,
        0x104 => 4,
        0xFF => 2,
        0x95..=0xF9 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw cases for task `t`.
fn draw(fx: &mut Fx, sp: u32, t: u32, counted: i32) {
    let f = fx.mem.read_u32(t + 0x44);
    if f == PETALS {
        let w = fx.mem.read_u32(TEX5);
        fx.mem.write_u32(D + 0x88, w);
        cell(fx, t, sp + 0x70);
        fx.mem.set16(D + 0xB8, 1);
        fx.mem.set16(D + 0xBE, 0);
        let h = fx.self_handle();
        let n = fx.mem.i16(SELF_NODE);
        crate::b076::node_position_saving(fx, h, n, sp + 0x60, u64::from(D), Some(counted as i64 as u64), 0x01E0_3590);
        let y = ee::sub(fx.mem.f(sp + 0x64), 2.5);
        fx.mem.setf(sp + 0x64, y);
        petals(fx, t, 0x20, sp + 0x60);
    }
    if fx.mem.read_u32(t + 0x44) == GRID {
        let w = fx.mem.read_u32(TEX5);
        fx.mem.write_u32(D + 0x88, w);
        cell(fx, t, sp + 0x80);
        fx.mem.set16(D + 0xB8, 1);
        fx.mem.set16(D + 0xBE, 0);
        let h = fx.target_handle();
        let n = fx.mem.i16(TARGET_NODE);
        crate::b076::node_position_saving(fx, h, n, sp + 0x60, u64::from(D), Some(counted as i64 as u64), 0x01E0_363C);
        fx.mem.write_u32(sp + 0x64, 0x4204_0000);
        grid(fx, t, 0x20, sp + 0x60);
    }
    for (step, r) in [(RISE, sp + 0x90), (RISE_B, sp + 0xA0)] {
        if fx.mem.read_u32(t + 0x44) == step {
            let w = fx.mem.read_u32(TEX5 + 4);
            fx.mem.write_u32(D + 0x88, w);
            let frame = fx.mem.i16(t + 0x56);
            uv::grid_rect(fx, r, frame);
            copy4(fx, D + 0x94, r);
            fx.mem.set16(D + 0xBE, 0);
            for c in [0u32, 4, 8] {
                let w = fx.mem.read_u32(t + 0x10 + c);
                fx.mem.write_u32(sp + 0x60 + c, w);
            }
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
        }
    }
    if fx.mem.read_u32(t + 0x44) == FAN {
        fx.mem.set16(D + 0xB8, 0x24);
        let h = fx.target_handle();
        let n = fx.mem.i16(TARGET_NODE);
        crate::b076::node_position_saving(fx, h, n, sp + 0x60, u64::from(D), Some(counted as i64 as u64), 0x01E0_37B0);
        fx.mem.write_u32(sp + 0x64, 0x4204_0000);
        billboard::arc_strips(fx, D, t, 0x20, sp + 0x60);
        fx.mem.set16(D + 0xB8, 0);
    }
}

fn run_task(fx: &mut Fx, f: u32, t: u32, e: [u64; 5]) {
    match f {
        PETALS => petals_step(fx, t, e),
        GRID => grid_step(fx, t, e),
        RISE => rise(fx, t, e, 4, 0x01E0_1ED0, 0x01E0_1FCC),
        RISE_B => rise(fx, t, e, 0xA, 0x01E0_2020, 0x01E0_211C),
        FAN => fan(fx, t, e),
        LIGHT => light(fx, t, e),
        WINDOW => window(fx, t, e),
        _ => panic!("b215: unknown task step {f:#x}"),
    }
}

/// 0x1E038D0(D, step, 0) from the emitter (its s0..s3 = `s`, s4 the
/// entry's 0).
fn spawn(fx: &mut Fx, f: u32, s: [u64; 4], ra: u32) {
    bspawn::task70(fx, D, POOL, f, 0, &[(0, s[0]), (0x10, s[1]), (0x20, s[2]), (0x30, s[3]), (0x40, 0), (0x50, u64::from(ra))]);
}

/// 0x1E01090(D, 0) (frame 0x50: the entry's s0 = D, s1 = 0, s2 = the
/// param, s3 the runner's (0 [H]), ra).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x40, 0x01E0_3460), (0x30, 0), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    let mut s: [u64; 4] = [u64::from(D), u64::from(D), param as i64 as u64, 0];
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    let c = counter(fx);
    if fx.div(c, 3).1 == 1 && c < 0xEB {
        spawn(fx, LENS, s, 0x01E0_110C);
    }
    if counter(fx) == 1 {
        fx.mem.set16(SLOT, 0);
        spawn(fx, PETALS, s, 0x01E0_1144);
        fx.mem.set16(SLOT, 1);
        spawn(fx, PETALS, s, 0x01E0_1168);
        spawn(fx, WINDOW, s, 0x01E0_117C);
        spawn(fx, SHADOW, s, 0x01E0_1190);
    }
    if counter(fx) == 0x8D {
        fx.mem.set16(SLOT, 0);
        spawn(fx, GRID, s, 0x01E0_11C8);
        spawn(fx, FAN, s, 0x01E0_11DC);
    }
    if counter(fx) == 1 {
        spawn(fx, LIGHT, s, 0x01E0_1208);
    }
    if counter(fx) >= 0x8D {
        let g = fx.mem.f(GROW);
        if ee_lt(g, 50.0) {
            fx.mem.setf(GROW, ee::add(g, 1.5));
        }
        let r = fx.mem.f(RADIUS);
        if ee_lt(r, 80.0) {
            fx.mem.setf(RADIUS, ee::add(r, 0.75));
        }
    }
    if counter(fx) < 0x8E {
        for k in 0..2u64 {
            s[0] = k;
            let h = fx.self_handle();
            let n = fx.mem.i16(SELF_NODE);
            crate::b076::node_position_saving(fx, h, n, POS, k, Some(u64::from(D)), 0x01E0_12E0);
            fx.mem.write_u32(POS + 4, 0x41F0_0000);
            let r = rand_mod(fx, k as u32, 0x01E0_12EC, 0x3C);
            let v = ee::add(fx.mem.f(POS), ee::from_i32(r - 0x1E));
            fx.mem.setf(POS, v);
            let r = rand_mod(fx, k as u32, 0x01E0_1320, 0x3C);
            let v = ee::add(fx.mem.f(POS + 8), ee::from_i32(r - 0x1E));
            fx.mem.setf(POS + 8, v);
            let r = rand_mod(fx, k as u32, 0x01E0_1354, 0x14);
            let v = ee::add(fx.mem.f(POS + 4), ee::from_i32(r - 0xA));
            fx.mem.setf(POS + 4, v);
            spawn(fx, RISE, s, 0x01E0_1390);
        }
        s[0] = 2;
    }
    let c = counter(fx);
    if c >= 0x8D && c < 0x100 {
        for k in 0..2u64 {
            s[0] = k;
            let h = fx.target_handle();
            let n = fx.mem.i16(TARGET_NODE);
            crate::b076::node_position_saving(fx, h, n, POS, k, Some(u64::from(D)), 0x01E0_1404);
            fx.mem.write_u32(POS + 4, 0x41F0_0000);
            let r = fx.mem.f(RADIUS);
            fx.fix_residue(r, 0x01E0_141C);
            let mut m = ee::fixsfsi(r) as i16 as i32;
            if m < 2 {
                m = 2;
            }
            let half = m >> 1;
            s[2] = m as i64 as u64;
            s[3] = half as i64 as u64;
            let r = rand_mod(fx, k as u32, 0x01E0_144C, m);
            let v = ee::add(fx.mem.f(POS), ee::from_i32(r - half));
            fx.mem.setf(POS, v);
            let r = rand_mod(fx, k as u32, 0x01E0_1478, m);
            let v = ee::add(fx.mem.f(POS + 8), ee::from_i32(r - half));
            fx.mem.setf(POS + 8, v);
            spawn(fx, RISE_B, s, 0x01E0_14B0);
        }
    }
    fx.leave(0x50);
}

/// A step's prologue: `frame` bytes, the entry's registers `e` (s0..s4)
/// at `slots` (offset, which), ra at `ra_at`.
fn enter_step(fx: &mut Fx, frame: u32, e: [u64; 5], slots: &[(u32, usize)], ra_at: u32, ra: u64) -> u32 {
    let sp = fx.enter(frame);
    for &(o, k) in slots {
        let v = e.get(k).copied().unwrap_or(0);
        fx.saved(sp + o, &[(0, v)]);
    }
    fx.saved(sp, &[(ra_at, ra)]);
    sp
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x58) == 1
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn odd_age(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54).wrapping_rem(2) == 1
}

/// The alpha byte `a` up by `up` while below `cap`.
fn fade_in(fx: &mut Fx, t: u32, a: u32, cap: u8, up: u8) {
    let v = fx.mem.read_u8(t + a);
    if v < cap {
        fx.mem.write_u8(t + a, v.wrapping_add(up));
    }
}

/// The alpha byte `a` down by `down` while at least `floor`, else the
/// task ends (`end`) or nothing.
fn fade_out(fx: &mut Fx, t: u32, a: u32, floor: u8, down: u8, end: bool) {
    let v = fx.mem.read_u8(t + a);
    if v >= floor {
        fx.mem.write_u8(t + a, v.wrapping_sub(down));
    } else if end {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The i16 at `a` as the original adds to it.
fn add16(fx: &mut Fx, a: u32, d: i32) {
    let v = fx.mem.i16(a).wrapping_add(d);
    fx.mem.set16(a, v);
}

/// 0x1E01500 (frame 0x30): the petal grid +0x50 (0 / 1, turned +-0x2000
/// about z): radius 20, cell rand % 7, life rand % 3 + 4, spin steps
/// +0x5A / +0x5C = rand % 8 + 4; both alphas in by 0xC below 0x80 to age
/// 0x90, then out by 9; white / light grey by age parity; the first +0x52
/// petals unwinding (their life down by 1, angles back by the steps, the
/// third by 10, to 0), the rest winding on (mod 360); +0x52 up by one a
/// update from age 0x1F to 36, then the whole turning +0x200 from age
/// 0x51; ended on update 0x8D.
fn petals_step(fx: &mut Fx, t: u32, e: [u64; 5]) {
    enter_step(fx, 0x30, e, &[(0, 0), (0x10, 1)], 0x20, TASK_RA);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        let r = rand_mod(fx, t, 0x01E0_1530, 7);
        fx.mem.set16(t + 0x56, r);
        fx.mem.set16(t + 0x58, 2);
        let r = rand_mod(fx, t, 0x01E0_1554, 3);
        fx.mem.set16(t + 8, r + 4);
        fx.mem.write_u32(t + 0x34, 0x3F99_999A);
        let r = fx.rand_from(t, 0x01E0_1580);
        fx.mem.set16(t + 0x5C, (r & 7) + 4);
        let r = fx.rand_from(t, 0x01E0_15A0);
        fx.mem.set16(t + 0x5A, (r & 7) + 4);
        fx.mem.set16(t + 0x52, 0);
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.set32(t + 0x28, 0x2000);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.set32(t + 0x28, -0x2000);
        }
    }
    if fx.mem.i16(t + 0x54) < 0x91 {
        fade_in(fx, t, 0x63, 0x80, 0xC);
        fade_in(fx, t, 0x67, 0x80, 0xC);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
        fade_out(fx, t, 0x67, 0xB, 9, false);
    }
    let g = if odd_age(fx, t) { 0xC0 } else { 0xFF };
    l2::rgb(fx, t + 0x60, g, g, g);
    l2::rgb(fx, t + 0x64, g, g, g);
    let row = |fx: &mut Fx, k: i32| (D as i32 + 0x50 * fx.mem.i16(t + 0x50) + 2 * k) as u32;
    let mut k = 0i32;
    while k < fx.mem.i16(t + 0x52) {
        let b = row(fx, k);
        let v = fx.mem.i16(b + 0xD9E0);
        if v > 0 {
            fx.mem.set16(b + 0xD9E0, v - 1);
        }
        for (o, st) in [(0xB460u32, 0x5Au32), (0xC0E0, 0x5C)] {
            let b = row(fx, k);
            let s = fx.mem.i16(t + st);
            let v = fx.mem.i16(b + o);
            fx.mem.set16(b + o, if s < v { v - s } else { 0 });
        }
        let b = row(fx, k);
        let v = fx.mem.i16(b + 0xCD60);
        fx.mem.set16(b + 0xCD60, if v >= 0xB { v - 0xA } else { 0 });
        k = (k + 1) as i16 as i32;
    }
    let mut k = fx.mem.i16(t + 0x52);
    while k < 0x24 {
        let b = row(fx, k);
        let s = fx.mem.i16(t + 0x5A);
        add16(fx, b + 0xB460, s);
        let b = row(fx, k);
        let s = fx.mem.i16(t + 0x5C);
        add16(fx, b + 0xC0E0, s);
        let b = row(fx, k);
        add16(fx, b + 0xCD60, 0xA);
        for o in [0xB460u32, 0xC0E0, 0xCD60] {
            let b = row(fx, k);
            let v = fx.mem.i16(b + o);
            if v >= 0x168 {
                let r = fx.div(v, 0x168).1;
                fx.mem.set16(b + o, r);
            }
        }
        k = (k + 1) as i16 as i32;
    }
    if fx.mem.i16(t + 0x54) >= 0x51 && fx.mem.i16(t + 0x52) == 0x24 {
        let v = fx.mem.i32(t + 0x24).wrapping_add(0x200);
        fx.mem.set32(t + 0x24, v);
    }
    if fx.mem.i16(t + 0x54) >= 0x1F {
        let v = fx.mem.i16(t + 0x52);
        if v < 0x24 {
            fx.mem.set16(t + 0x52, v + 1);
        }
    }
    if counter(fx) == 0x8D {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1A7520 sqrtf of `x` called from `ra` (its `sd ra` at sp - 0x10).
fn sqrtf_from(fx: &mut Fx, x: f32, ra: u32) -> f32 {
    let a = fx.sp.wrapping_sub(0x10);
    fx.mem.write_u64(a, u64::from(ra));
    fx.sqrtf(x)
}

/// 0x1E01A70 (frame 0xB0): the quad grid about the target's node 1: size
/// 4.2, its 20 x 28 cells laid out 2 x 4.2 apart (y 0, angles 0); alphas
/// 0x80 for 20 updates, then out by 9 once the timer is below 20; the
/// cells inside the radius D+0x9FC0 of the node rise by 0.8 a update and
/// turn by -5 / +5; grey; turned +0x4000 about y.
fn grid_step(fx: &mut Fx, t: u32, e: [u64; 5]) {
    let sp = enter_step(fx, 0xB0, e, &[(0, 0), (0x10, 1), (0x20, 2), (0x30, 3), (0x40, 4), (0x50, 5), (0x60, 6), (0x70, 7)], 0x80, TASK_RA);
    let h = fx.target_handle();
    let n = fx.mem.i16(TARGET_NODE);
    crate::b076::node_position_saving(fx, h, n, sp + 0x90, e[0], Some(e[1]), 0x01E0_1ABC);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x4086_6666);
        let r = rand_mod(fx, D, 0x01E0_1ADC, 7);
        fx.mem.set16(t + 0x56, r);
        fx.mem.set16(t + 0x58, 2);
        let r = rand_mod(fx, D, 0x01E0_1B00, 3);
        fx.mem.set16(t + 8, r + 4);
        fx.mem.write_u32(t + 0x34, 0x3F99_999A);
        for j in 0..0x14u32 {
            let fj = ee::from_i32(j as i32);
            for k in 0..0x1Cu32 {
                let a1 = JIT + 0x280 * j + 0x10 * k;
                let a2 = D + 0x50 * j + 2 * k;
                let fk = ee::from_i32(k as i32);
                let r2 = ee::mul(2.0, fx.mem.f(t + 0x48));
                let x = ee::sub(ee::mul(r2, fj), ee::div(ee::mul(r2, 20.0), 2.0));
                fx.mem.setf(a1, x);
                let r2 = ee::mul(2.0, fx.mem.f(t + 0x48));
                let z = ee::sub(ee::mul(r2, fk), ee::div(ee::mul(r2, 28.0), 2.0));
                fx.mem.setf(a1 + 8, z);
                fx.mem.write_u32(a1 + 4, 0);
                for o in [0xB460u32, 0xC0E0, 0xCD60] {
                    fx.mem.set16(a2 + o, 0);
                }
            }
        }
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        fx.mem.write_u8(t + 0x63, 0x80);
        fx.mem.write_u8(t + 0x67, 0x80);
    } else if fx.mem.i32(TIMER) < 0x14 {
        fade_out(fx, t, 0x63, 0xB, 9, true);
        fade_out(fx, t, 0x67, 0xB, 9, false);
    }
    let (a, b) = (fx.mem.f(RADIUS), fx.mem.f(D + 0x9FC8));
    sqrtf_from(fx, ee::add(ee::mul(a, a), ee::mul(b, b)), 0x01E0_1CB4);
    for j in 0..0x14u32 {
        for k in 0..0x1Cu32 {
            let s0 = JIT + 0x280 * j + 0x10 * k;
            let s1 = D + 0x50 * j + 2 * k;
            let x = fx.mem.f(sp + 0x90);
            let dx = ee::sub(x, ee::add(x, fx.mem.f(s0)));
            fx.mem.setf(sp + 0xA0, dx);
            let z = fx.mem.f(sp + 0x98);
            let dz = ee::sub(z, ee::add(z, fx.mem.f(s0 + 8)));
            fx.mem.setf(sp + 0xA8, dz);
            let (dx, dz) = (fx.mem.f(sp + 0xA0), fx.mem.f(sp + 0xA8));
            let dist = sqrtf_from(fx, ee::add(ee::mul(dx, dx), ee::mul(dz, dz)), 0x01E0_1D18);
            if ee_le(fx.mem.f(RADIUS), dist) {
                continue;
            }
            let y = ee::add(fx.mem.f(s0 + 4), f32::from_bits(0x3F4C_CCCD));
            fx.mem.setf(s0 + 4, y);
            add16(fx, s1 + 0xB460, -5);
            add16(fx, s1 + 0xC0E0, 5);
        }
    }
    l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
    l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
    fx.mem.write_u32(t + 0x24, 0x4000);
    age(fx, t);
    fx.leave(0xB0);
}

/// 0x1E01EA0 / 0x1E01FF0 (frame 0x20): a white billboard of size `n`
/// (cell rand % 8) rising by 2.5 a update, facing the camera; both alphas
/// in by 0x60 below 0xC0 for 10 updates, then +0x63 out by 7 twice.
fn rise(fx: &mut Fx, t: u32, e: [u64; 5], n: i32, rand_ra: u32, cam_ra: u32) {
    enter_step(fx, 0x20, e, &[(0, 0)], 0x10, TASK_RA);
    if first(fx, t) {
        fx.mem.set16(t + 8, n);
        fx.mem.set16(t + 0xA, n);
        fx.mem.write_u32(t + 0x48, 0);
        let r = fx.rand_from(t, rand_ra);
        fx.mem.set16(t + 0x56, r & 7);
        fx.mem.write_u32(t + 0x34, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x5C, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
        fade_in(fx, t, 0x67, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 9, 7, true);
        fade_out(fx, t, 0x63, 9, 7, false);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let y = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
        fx.mem.setf(t + 0x14, y);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), e[1], cam_ra);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02140 (frame 0x30): the flat arc fan (36 strips 10 degrees wide,
/// outer height 6 x the radius D+0x9FC0), grey, the inner colour blue
/// tinted by age parity; +0x67 in by 0x40 below 0x80 for 4 updates, out
/// by 0xB from update 0xF6.
fn fan(fx: &mut Fx, t: u32, e: [u64; 5]) {
    enter_step(fx, 0x30, e, &[(0, 0), (0x10, 1)], 0x20, TASK_RA);
    if first(fx, t) {
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x67, 0x80, 0x40);
    } else if counter(fx) >= 0xF6 {
        fade_out(fx, t, 0x67, 0xD, 0xB, true);
    }
    let b = if odd_age(fx, t) { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x60, 0x80, 0x80, b);
    let r = ee::mul(6.0, fx.mem.f(RADIUS));
    fx.mem.setf(t + 0x4C, r);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E022A0 (frame 0x20): the stage light, 0x80 down by 10 to 0x40 for
/// 0xFA updates, then up by 10 to 0x80; on timer 3 back to 0x80 and over.
fn light(fx: &mut Fx, t: u32, e: [u64; 5]) {
    enter_step(fx, 0x20, e, &[(0, 0)], 0x10, TASK_RA);
    if first(fx, t) {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let v = fx.mem.i16(t + 0x50);
    if fx.mem.i16(t + 0x54) < 0xFA {
        fx.mem.set16(t + 0x50, if v < 0x41 { 0x40 } else { v - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if v < 0x80 { v + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    age(fx, t);
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
    fx.leave(0x20);
}

/// The 0x6FF8 / 0x8FF8 corner XYZ2 at `at` (Z 0xFFFFFF, the top byte 0).
fn corner(fx: &mut Fx, at: u32, v: u16) {
    fx.mem.write_u16(at, v);
    fx.mem.write_u16(at + 2, v);
    let x = fx.mem.read_u64(at);
    fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
    fx.mem.write_u8(at + 7, 0);
}

/// 0x1E02390 (frame 0x80): a full-window sprite (ALPHA 0x44) of the frame
/// (0x207EF0), tinted (0x78, 0x78, 0xA0), in by 10 below 0x50 for 0xF4
/// updates, then out by 9.
fn window(fx: &mut Fx, t: u32, e: [u64; 5]) {
    let sp = enter_step(fx, 0x80, e, &[(0, 0)], 0x10, TASK_RA);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u8(sp + 0x38, 0x78);
    fx.mem.write_u8(sp + 0x39, 0x78);
    fx.mem.write_u8(sp + 0x3A, 0xA0);
    let a = fx.mem.read_u8(t + 0x63);
    fx.mem.write_u8(sp + 0x3B, a);
    fx.mem.write_u32(sp + 0x3C, 0x3F80_0000);
    corner(fx, sp + 0x48, 0x6FF8);
    corner(fx, sp + 0x58, 0x8FF8);
    fx.mem.write_u32(sp + 0x40, 0);
    fx.mem.write_u32(sp + 0x44, 0);
    fx.mem.write_u32(sp + 0x50, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x54, 0x3F80_0000);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, 0x80_0000_0044);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(t + 0x54) < 0xF4 {
        fade_in(fx, t, 0x63, 0x50, 0xA);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    age(fx, t);
    fx.leave(0x80);
}

/// 0x1E02530(D, t) from the layer (frame 0x30): a lens ring at the
/// target's node 1 (y 33), grey inside / light grey outside, the outer
/// alpha 0xFF out by 9 from age 6; radius D+0x9FC0, height D+0x9FC4 at
/// its spawn, turned rand % 360.
fn lens_step(fx: &mut Fx, t: u32, saves: &[(u32, u64)]) {
    let sp = fx.enter(0x30);
    fx.saved(sp, saves);
    if first(fx, t) {
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.write_u8(t + 0x67, 0xFF);
        l2::rgb(fx, t + 0x60, 0x38, 0x38, 0x38);
        l2::rgb(fx, t + 0x64, 0xCF, 0xCF, 0xCF);
        fx.mem.set16(t + 0x58, 2);
        let h = fx.target_handle();
        let n = fx.mem.i16(TARGET_NODE);
        crate::b076::node_position_saving(fx, h, n, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_25AC);
        fx.mem.write_u32(t + 0x14, 0x4204_0000);
        let w = fx.mem.read_u32(GROW);
        fx.mem.write_u32(t + 0x34, w);
        let r = rand_mod(fx, t, 0x01E0_25C8, 0x168);
        fx.mem.set16(t + 0x5A, r);
    }
    let w = fx.mem.read_u32(RADIUS);
    fx.mem.write_u32(t + 0x48, w);
    if fx.mem.i16(t + 0x54) >= 6 {
        fade_out(fx, t, 0x67, 0xB, 9, true);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// One ST coordinate from a projected XYZ value: (base - v) / -16.
fn st(v: u16, base: i32) -> f32 {
    ee::div(ee::from_i32(base - i32::from(v)), -16.0)
}

/// 0x1E02650(D, t) (frame 0x130): the lens as 24 quads 15 degrees wide
/// (radius +0x48, from the angle +0x5A; the inner edge at height +0x34)
/// at the task's point, unturned; ST the vertices' own screen texels in
/// the frame copy (the inner edge's y by the task's i16 +0xA + 0x700,
/// clamped at 0; the outer's clamped at 1, the last u over 1024),
/// unlinked. Returns a3 as its last call leaves it.
fn lens_draw(fx: &mut Fx, t: u32, saves: &[(u32, u64)]) -> u32 {
    let sp = fx.enter(0x130);
    fx.saved(sp, saves);
    for (k, (c, at)) in [(0x60u32, 0x10u32), (0x60, 0x28), (0x64, 0x40), (0x64, 0x58)].into_iter().enumerate() {
        let tmp = sp + 0x110 + 8 * k as u32;
        l2::rgbaq(fx, tmp, t + c);
        let (a, b) = (fx.mem.read_u32(tmp), fx.mem.read_u32(tmp + 4));
        let q = fx.mem.read_u32(QUAD_PTR);
        fx.mem.write_u32(q + at, a);
        fx.mem.write_u32(q + at + 4, b);
    }
    for c in [0u32, 4, 8] {
        let w = fx.mem.read_u32(t + 0x10 + c);
        fx.mem.write_u32(sp + 0xC0 + c, w);
    }
    l2::xform_vp(fx, sp + 0xD0, 0, 0, sp + 0xC0);
    let f20 = fx.mem.f(t + 0x48);
    let h = fx.mem.read_u32(t + 0x34);
    fx.mem.write_u32(sp + 0x84, h);
    fx.mem.write_u32(sp + 0x94, h);
    fx.mem.write_u32(sp + 0xA4, 0);
    fx.mem.write_u32(sp + 0xB4, 0);
    let mut a3 = 0u32;
    let mut s1 = 0i32;
    for _ in 0..0x18 {
        let a = (fx.mem.i16(t + 0x5A) + s1) as i16 as i32;
        let c = l2::sin_deg(fx, SINES, a, 1);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let (x, z) = (ee::mul(c, f20), ee::mul(s, f20));
        fx.mem.setf(sp + 0xA0, x);
        fx.mem.setf(sp + 0xA8, z);
        fx.mem.setf(sp + 0x80, x);
        fx.mem.setf(sp + 0x88, z);
        let a = (s1 + fx.mem.i16(t + 0x5A) + 0xF) as i16 as i32;
        let c = l2::sin_deg(fx, SINES, a, 1);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let (x, z) = (ee::mul(c, f20), ee::mul(s, f20));
        s1 = (s1 + 0xF) as i16 as i32;
        fx.mem.setf(sp + 0xB0, x);
        fx.mem.setf(sp + 0xB8, z);
        fx.mem.setf(sp + 0x90, x);
        fx.mem.setf(sp + 0x98, z);
        let q = fx.mem.read_u32(QUAD_PTR);
        if !l2::project(fx, sp + 0x80, SPR, sp + 0xD0, q, sp + 0x70) {
            a3 = q;
            continue;
        }
        let inner = ((fx.mem.i16(t + 0xA) + 0x700) << 4) - 8;
        for (xyz, stq) in [(0x20u32, 0x18u32), (0x38, 0x30)] {
            let q = fx.mem.read_u32(QUAD_PTR);
            let (x, y) = (fx.mem.read_u16(q + xyz), fx.mem.read_u16(q + xyz + 2));
            let u = st(x, 0x6FF8);
            let mut v = st(y, inner);
            if ee_lt(v, 0.0) {
                v = 0.0;
            }
            fx.mem.setf(q + stq, ee::div(u, 512.0));
            fx.mem.setf(q + stq + 4, ee::div(v, 512.0));
        }
        for (xyz, stq, du) in [(0x50u32, 0x48u32, 512.0f32), (0x68, 0x60, 1024.0)] {
            let q = fx.mem.read_u32(QUAD_PTR);
            let (x, y) = (fx.mem.read_u16(q + xyz), fx.mem.read_u16(q + xyz + 2));
            let u = st(x, 0x6FF8);
            let mut v = st(y, 0x6FF8);
            if !ee_le(v, 1.0) {
                v = 1.0;
            }
            fx.mem.setf(q + stq, ee::div(u, du));
            fx.mem.setf(q + stq + 4, ee::div(v, 512.0));
        }
        let q = fx.mem.read_u32(QUAD_PTR);
        fx.submit(q, 0, 0);
        // [run] 0x206F30 leaves a3 = the descriptor too.
        a3 = q;
    }
    fx.leave(0x130);
    a3
}

/// 0x1E02B30(D, t) from the layer (frame 0x40): the attacker's sprite (a
/// full window, ST from the task's i16 +8 / +0xA in tenths), white, its
/// alpha in by 8 below 0x80 for 0xC8 updates, then out by 9; unlinked.
fn shadow(fx: &mut Fx, t: u32, saves: &[(u32, u64)]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, saves);
    let f20 = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let f21 = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if first(fx, t) {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(SPRITE_PTR);
    for k in 0..4 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    corner(fx, d + 0x20, 0x6FF8);
    corner(fx, d + 0x30, 0x8FF8);
    let f0 = ee::sub(512.0, ee::mul(3.0, f21));
    let f1 = ee::div(f20, 512.0);
    let f2 = ee::div(f21, 512.0);
    let f0 = ee::div(f0, 512.0);
    fx.mem.setf(d + 0x18, f1);
    let f1 = ee::div(ee::sub(512.0, f20), 512.0);
    fx.mem.setf(d + 0x1C, f2);
    fx.mem.setf(d + 0x28, f1);
    fx.mem.setf(d + 0x2C, f0);
    fx.submit(d, 0, 0);
    if fx.mem.i16(t + 0x54) < 0xC8 {
        let v = fx.mem.read_u8(t + 0x63);
        if v < 0x80 {
            fx.mem.write_u8(t + 0x63, v + 8);
        } else {
            fx.mem.set16(t + 0x50, 1);
        }
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E044C0(D) (frame 0x130: the sprite descriptor at sp + 0x48, the
/// quad at sp + 0xA0): a packet layer; TEXFLUSH, ALPHA 0x48, the frame
/// copied to 0x2095A0's TBP for the quad's TEX0 / TEX1 (0x207F50, point
/// sampled), TEXFLUSH; the lens tasks stepped and drawn; the sprite
/// re-headed, TEXFLUSH, ALPHA 0x48, the attacker drawn into its TEX0 /
/// TEX1 (0x1F8270 filter 0), TEXFLUSH; the shadow tasks stepped; the
/// layer at key 0.
fn layer(fx: &mut Fx, counted: i32, drawn: i32) {
    let sp = fx.enter(0x130);
    fx.saved(sp, &[(0x30, 0x01E0_3804), (0x20, u64::from(D + 0x70 * TASKS)), (0x10, counted as i64 as u64), (0, u64::from(D))]);
    let (d, q) = (sp + 0x48, sp + 0xA0);
    fx.mem.write_u32(SPRITE_PTR, d);
    fx.mem.write_u32(QUAD_PTR, q);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u16(q + 2, 0x34);
    fx.mem.write_u16(q, 0xC);
    fx.mem.write_u64(q + 8, 0x4214_2142_1421);
    fx.mem.write_u32(q + 4, 0);
    fx.texflush([0xC, 0x34, d, 0x4_2421]);
    let old = u32::from(fx.mem.read_u16(q + 2));
    fx.desc_alpha(q, 0x80_0000_0048);
    let dest = fx.copy_dest([q, 0x48, old | 0x200, old]);
    let regs = fx.frame_texture_packet(dest, 0, q + 0x78, q + 0x80);
    fx.texflush(regs);
    let mut a3 = regs[3];
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 || fx.mem.read_u32(t + 0x2E4) != LENS {
            continue;
        }
        // The layer's s0 = k, s1 = the slot, s2 = D; s3 / s4 the entry's
        // (130, the drawn count).
        let s = |ra: u32| [(0u32, u64::from(k)), (0x10, u64::from(t)), (0x20, u64::from(ra))];
        lens_step(fx, t + 0x2A0, &s(0x01E0_4638));
        let sv = [(0x10u32, u64::from(k)), (0x20, u64::from(t)), (0x30, u64::from(D)), (0x40, u64::from(TASKS)), (0x50, drawn as i64 as u64), (0x60, 0x01E0_4644)];
        a3 = lens_draw(fx, t + 0x2A0, &sv);
    }
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x88, sp + 0x90);
    fx.texflush(regs);
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 || fx.mem.read_u32(t + 0x2E4) != SHADOW {
            continue;
        }
        shadow(fx, t + 0x2A0, &[(0x10, u64::from(t)), (0x20, u64::from(k)), (0x30, 0x01E0_4724)]);
    }
    let layer = fx.mem.read_u32(LAYER);
    fx.layer_end(layer, OT, 0);
    fx.leave(0x130);
}

/// 0x109600(out, w, h): a quad in the xz plane: (-w, 0, h), (w, 0, h),
/// (-w, 0, -h), (w, 0, -h) (the w components left) [F] (b582's).
fn quad_xz(fx: &mut Fx, out: u32, w: f32, h: f32) {
    let (nw, nh) = (ee::neg(w), ee::neg(h));
    for (k, (x, z)) in [(nw, h), (w, h), (nw, nh), (w, nh)].into_iter().enumerate() {
        let a = out + 0x10 * k as u32;
        fx.mem.setf(a, x);
        fx.mem.set32(a + 4, 0);
        fx.mem.setf(a + 8, z);
    }
}

/// The strip3 header shared by the two grid draws (frame-local `d`):
/// colours +0x60 / +0x64, UVs from D+0x94, texture D+0x88, blend mode >> 4.
fn strip_desc(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 1);
    l2::colour_strip3(fx, d, t + 0x64, 2);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    fx.desc_alpha(d, crate::c061::blend_alpha(mode >> 4));
}

/// The projected strip `d` keyed by its mean w at `w` on D+0x9FFC.
fn submit_at_w(fx: &mut Fx, d: u32, w: u32, ra: u32) {
    let v = fx.mem.f(w);
    fx.fix_residue(v, ra);
    let key = ee::fixunssfsi(v) as i32;
    let ot = fx.mem.read_u32(OT_AT);
    fx.submit(d, ot, key);
}

/// 0x1E03A60(D, t, mode, pos) (frame 0x220): the task's 36 petals 10
/// degrees apart, each a strip3 piece 5 degrees wide of radius +0x48 (y
/// +-+0x34) on its own cell (0x14, D+0xB3D0 [k]), turned by the row's
/// angles (+0x50: D+0xB460 / 0xC0E0 [k] about x then y) about its centre
/// and pushed out by the life (D+0xD9E0 [k]) x the jitter D+0x3BC0 + 0x280
/// row + 0x10 k; through TRS(angles +0x20, pos) x the view-projection;
/// keyed by its mean w.
fn petals(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x220);
    copy4(fx, sp + 0xC0, pos);
    let d = sp + 0x190;
    strip_desc(fx, t, d, mode);
    for c in [0u32, 4, 8] {
        let w = fx.mem.read_u32(sp + 0xC0 + c);
        fx.mem.write_u32(sp + 0x110 + c, w);
    }
    l2::xform_vp(fx, sp + 0x120, 0, t + 0x20, sp + 0x110);
    let f21 = fx.mem.f(t + 0x48);
    let mut s3 = 0i32;
    for k in 0..0x24u32 {
        let c = fx.mem.i16(CELLS_A + 2 * k);
        l2::rect(fx, sp + 0x180, 0x14, c);
        copy4(fx, D + 0x94, sp + 0x180);
        let order = fx.mem.i16(D + 0xBE);
        l2::uv_strip3(fx, d, D + 0x94, order);
        let h = fx.mem.f(t + 0x34);
        fx.mem.setf(sp + 0xD4, h);
        fx.mem.setf(sp + 0xE4, h);
        fx.mem.setf(sp + 0xF4, ee::neg(h));
        fx.mem.setf(sp + 0x104, ee::neg(h));
        let b = (D as i32 + 0x50 * fx.mem.i16(t + 0x50) + 2 * k as i32) as u32;
        let f20 = ee::from_i32(fx.mem.i16(b + 0xD9E0));
        for (o, at) in [(0xB460u32, 0x170u32), (0xC0E0, 0x174), (0xCD60, 0x178)] {
            let b = (D as i32 + 0x50 * fx.mem.i16(t + 0x50) + 2 * k as i32) as u32;
            let v = fx.mem.i16(b + o);
            fx.mem.set32(sp + at, v);
        }
        let f22 = l2::sin_deg(fx, SINES, s3, 1);
        let f23 = l2::sin_deg(fx, SINES, s3, 0);
        let a5 = (s3 + 5) as i16 as i32;
        let f24 = l2::sin_deg(fx, SINES, a5, 1);
        let f25 = l2::sin_deg(fx, SINES, a5, 0);
        let (x, z) = (ee::mul(f22, f21), ee::mul(f23, f21));
        fx.mem.setf(sp + 0xF0, x);
        fx.mem.setf(sp + 0xF8, z);
        fx.mem.setf(sp + 0xD0, x);
        fx.mem.setf(sp + 0xD8, z);
        let a10 = (s3 + 0xA) as i16 as i32;
        let c = l2::sin_deg(fx, SINES, a10, 1);
        let s = l2::sin_deg(fx, SINES, a10, 0);
        let (x, z) = (ee::mul(c, f21), ee::mul(s, f21));
        fx.mem.setf(sp + 0x100, x);
        fx.mem.setf(sp + 0x108, z);
        fx.mem.setf(sp + 0xE0, x);
        fx.mem.setf(sp + 0xE8, z);
        let (cx, cz) = (ee::mul(f24, f21), ee::mul(f25, f21));
        for i in 0..4u32 {
            let p = sp + 0xD0 + 0x10 * i;
            let v = ee::sub(fx.mem.f(p), cx);
            fx.mem.setf(p, v);
            let v = ee::sub(fx.mem.f(p + 8), cz);
            fx.mem.setf(p + 8, v);
        }
        l2::rotate4(fx, sp + 0xD0, sp + 0x170, 1, SINES);
        l2::rotate4(fx, sp + 0xD0, sp + 0x170, 0, SINES);
        for i in 0..4u32 {
            let v = sp + 0xD0 + 0x10 * i;
            let j = (D as i32 + 0x10 * k as i32 + 0x280 * fx.mem.i16(t + 0x50)) as u32 + 0x3BC0;
            let x = ee::add(fx.mem.f(v), ee::add(cx, ee::mul(fx.mem.f(j), f20)));
            fx.mem.setf(v, x);
            let j = (D as i32 + 0x10 * k as i32 + 0x280 * fx.mem.i16(t + 0x50)) as u32 + 0x3BC0;
            let z = ee::add(fx.mem.f(v + 8), ee::add(cz, ee::mul(fx.mem.f(j + 8), f20)));
            fx.mem.setf(v + 8, z);
            let j = (D as i32 + 0x10 * k as i32 + 0x280 * fx.mem.i16(t + 0x50)) as u32 + 0x3BC0;
            let y = ee::add(fx.mem.f(v + 4), ee::add(cz, ee::mul(fx.mem.f(j + 4), f20)));
            fx.mem.setf(v + 4, y);
        }
        if l2::project(fx, sp + 0xD0, SPR, sp + 0x120, d, sp + 0x160) {
            submit_at_w(fx, d, sp + 0x160, 0x01E0_3F04);
        }
        s3 = (s3 + 0xA) as i16 as i32;
    }
    fx.leave(0x220);
}

/// 0x1E03FB0(D, t, mode, pos) (frame 0x2A0): the 20 x 28 grid of quads
/// of size +0x48 (xz), each on its cell (0x14, D+0xA048 + 0x64 j + 2 k),
/// turned by D+0xB460 / 0xC0E0 + 0x50 j [k] (about x then y) and moved by
/// the jitter D+0x3BC0 + 0x280 j + 0x10 k; through TRS(angles +0x20, pos)
/// x the view-projection; keyed by its mean w. The vertex colours: row 0
/// clear / +0x60 alternating, row 1 +0x60, row 19 +0x60 / clear.
fn grid(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x2A0);
    copy4(fx, sp + 0xD0, pos);
    let clear = sp + 0x29C;
    for k in 0..4 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(clear + k, b);
    }
    fx.mem.write_u8(clear + 3, 0);
    let d = sp + 0x1A8;
    strip_desc(fx, t, d, mode);
    for c in [0u32, 4, 8] {
        let w = fx.mem.read_u32(sp + 0xD0 + c);
        fx.mem.write_u32(sp + 0x120 + c, w);
    }
    l2::xform_vp(fx, sp + 0x130, 0, t + 0x20, sp + 0x120);
    for o in [0xE4u32, 0xF4, 0x104, 0x114] {
        fx.mem.write_u32(sp + o, 0);
    }
    let c60 = t + 0x60;
    for j in 0..0x14u32 {
        fx.mem.set16(sp + 0xA0, j as i32);
        fx.mem.write_u32(sp + 0xB0, D + 0x64 * j);
        fx.mem.write_u32(sp + 0xC0, D + 0x50 * j);
        let colours = match j {
            0 => Some(([clear, c60, clear, c60], 0x238u32)),
            1 => Some(([c60, c60, c60, c60], 0x258)),
            0x13 => Some(([c60, clear, c60, clear], 0x278)),
            _ => None,
        };
        if let Some((cs, tmp)) = colours {
            for (i, (c, at)) in cs.into_iter().zip([0x10u32, 0x28, 0x40, 0x58]).enumerate() {
                let o = sp + tmp + 8 * i as u32;
                l2::rgbaq(fx, o, c);
                let (a, b) = (fx.mem.read_u32(o), fx.mem.read_u32(o + 4));
                fx.mem.write_u32(d + at, a);
                fx.mem.write_u32(d + at + 4, b);
            }
        }
        for k in 0..0x1Cu32 {
            let (s2, s3, s4) = (D + 0x64 * j + 2 * k, D + 0x50 * j + 2 * k, D + 0x280 * j + 0x10 * k);
            let c = fx.mem.i16(s2 + 0xA048);
            l2::rect(fx, sp + 0x190, 0x14, c);
            copy4(fx, D + 0x94, sp + 0x190);
            let order = fx.mem.i16(D + 0xBE);
            l2::uv_strip3(fx, d, D + 0x94, order);
            let w = fx.mem.f(t + 0x48);
            quad_xz(fx, sp + 0xE0, w, w);
            let a = fx.mem.i16(s3 + 0xB460);
            fx.mem.set32(sp + 0x180, a);
            let b = fx.mem.i16(s3 + 0xC0E0);
            fx.mem.set32(sp + 0x184, b);
            l2::rotate4(fx, sp + 0xE0, sp + 0x180, 1, SINES);
            l2::rotate4(fx, sp + 0xE0, sp + 0x180, 0, SINES);
            let jx = fx.mem.f(s4 + 0x3BC0);
            l2::add_column(fx, sp + 0xE0, 0, jx);
            let jz = fx.mem.f(s4 + 0x3BC8);
            l2::add_column(fx, sp + 0xE0, 2, jz);
            let jy = fx.mem.f(s4 + 0x3BC4);
            l2::add_column(fx, sp + 0xE0, 1, jy);
            if l2::project(fx, sp + 0xE0, SPR, sp + 0x130, d, sp + 0x170) {
                submit_at_w(fx, d, sp + 0x170, 0x01E0_43F4);
            }
        }
    }
    fx.mem.set16(sp + 0xA0, 0x14);
    fx.leave(0x2A0);
}
