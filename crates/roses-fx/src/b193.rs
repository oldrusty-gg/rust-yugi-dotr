//! BMODULE b193 (card 320), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03220, data area 0x1ED0000) [F] (helper A18, 31
//! Oct 2026): b747 / b491's task pool (150 x 0x70 at +0x2A0, alive +0x58,
//! step +0x44 through `jalr`; spawned by 0x1E03A90 with the colour
//! +0xBC94 and the index +0xBCEA) on the second library (`uv_rect`,
//! `ring_band_fy`, `ring_facing_flat_z`, `ring_quads` and the textured
//! STQ ring 3af0da313c90 at 0x1E01D70, `quads4::ring_band_stq`). The
//! emitter 0x1E02290 counts +0xBCF2: on update 25 at the attacker's nodes
//! 0x1B and 0x25 a dust ring (0x1E02780, `ring_facing_flat_z`, 6 counts,
//! radius 10..59 growing 16..45) and a green mote ring (0x1E02910,
//! `ring_quads` of 10 from the bytes +0xBDF4, its node index running on
//! every update); on 39..229 at the attacker's first node (8 behind or in
//! front by the facing +0xBCEE) a grey STQ ring (0x1E02B10, radius 3 + 4
//! widening, at the angle +0xBCE0 turning 4 a update) and, below 76, a
//! green sway band (0x1E02530, `ring_band_fy`, radius 35 growing 35.5);
//! on 77..229 every third update (n % 3 = 1) a dust ring at the target's
//! first node (0x1E02E50, 5 counts, 130 growing 2). The steps 0x1E02D40
//! (`ring_band`) and 0x1E03000 (53814623ff1c, a recompiled `tube`) are
//! never spawned: their draw cases are not ported. Returns 1 on update
//! 70, 3 on 255, 6 on 81..244, 8 once the 266-update timer +0xBC90 has
//! run out.
use crate::b076::node_position_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const BAND: u32 = 0x01E0_2530;
const DUST: u32 = 0x01E0_2780;
const MOTE: u32 = 0x01E0_2910;
const STQ: u32 = 0x01E0_2B10;
const DUST_T: u32 = 0x01E0_2E50;
const STQ_AT: u32 = 0x01E0_1D70;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3528;
/// The return address of the entry's call into the emitter.
const EMIT_RA: u64 = 0x01E0_34F8;
const SPAWN_AT: u32 = D + 0x250;
const TEX_A: u32 = D + 0x29C;
const TEX: u32 = D + 0xBC70;
const TIMER: u32 = D + 0xBC90;
const COLOUR: u32 = D + 0xBC94;
const NODE_A: u32 = D + 0xBCBC;
/// The node slots by index (+0xBCBE, +0xBCC0, ...).
const NODES: u32 = D + 0xBCBE;
const NODE_B: u32 = D + 0xBCC0;
const TNODE: u32 = D + 0xBCCA;
const ANGLE: u32 = D + 0xBCE0;
const INDEX: u32 = D + 0xBCEA;
const FACING: u32 = D + 0xBCEE;
const COUNTER: u32 = D + 0xBCF2;
const BYTES: u32 = D + 0xBCF4;
const BYTES2: u32 = D + 0xBDF4;
const SINES: u32 = D + 0xD8;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// rand % 2^k as C's signed remainder (rand is never negative).
fn rpow2(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    if v >= 0 {
        v & (m - 1)
    } else {
        let r = v & (m - 1);
        if r == 0 {
            0
        } else {
            r - m
        }
    }
}

/// The entry 0x1E03220 (a0 = D, a1 = param; s0 = D, s1 = 0, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(D + 0xBC9C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x150);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xC060);
    l2::sine_table(fx, D + 0xBEF4);
    l2::sine_table(fx, SINES);
    fx.mem.set16(D + 0xBCF0, param);
    fx.mem.set32(TIMER, 0x10A);
    for (node, clear, at) in [(1, 1, NODE_A), (0x1B, 0, NODES), (0x25, 0, NODE_B)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TNODE, r);
    for k in 0..6u32 {
        let t = fx.texture(4, 0xA + k as i32);
        fx.mem.write_u32(D + 0x288 + 4 * k, t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_33C0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_33F0, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rpow2(fx, D, 0x01E0_344C, 4);
        fx.mem.set8(BYTES + k, v);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_34A4, 0x28);
        fx.mem.set8(BYTES2 + k, v - 0x14);
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
        match fx.mem.read_u32(t + 0x44) {
            0 => {}
            BAND => band(fx, t),
            DUST => dust(fx, t),
            MOTE => mote(fx, t),
            STQ => stq(fx, t),
            DUST_T => dust_t(fx, t),
            f => panic!("b193: unknown task step {f:#x}"),
        }
    }
    // The entry's s1: the live tasks counted so far.
    let mut alive = 0i32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        alive = (alive + 1) as i16 as i32;
        let s1 = Some(alive as i64 as u64);
        match fx.mem.read_u32(t + 0x44) {
            BAND => {
                for c in 0..4 {
                    let w = fx.mem.read_u32(t + 0x10 + 4 * c);
                    fx.mem.write_u32(sp + 0x50 + 4 * c, w);
                }
                quads4::ring_band_fy(fx, D, t, 0x20, sp + 0x50);
            }
            DUST => {
                let sub = fx.mem.i16(t + 0x56);
                cell(fx, sp + 0x60, 0x14, sub);
                let w = fx.mem.read_u32(TEX);
                fx.mem.write_u32(D + 0x88, w);
                fx.mem.set16(D + 0xB8, 6);
                let a = fx.mem.i16(t + 0xA);
                fx.mem.set16(D + 0xC0, a);
                let h = fx.self_handle();
                let i = fx.mem.i16(t + 0x52);
                let slot = fx.mem.i16((NODES as i32 + 2 * i) as u32);
                node_position_saving(fx, h, slot, D + 0x20, u64::from(D), s1, 0x01E0_364C);
                quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
            MOTE => {
                let w = fx.mem.read_u32(TEX_A);
                fx.mem.write_u32(D + 0x88, w);
                let sub = fx.mem.i16(t + 0x56);
                cell(fx, sp + 0x70, 0x9B, sub);
                fx.mem.set16(D + 0xB8, 0xA);
                fx.mem.set16(D + 0xB0, 1);
                let h = fx.self_handle();
                let i = fx.mem.i16(t + 0x52);
                let slot = fx.mem.i16((NODES as i32 + 2 * i) as u32);
                node_position_saving(fx, h, slot, t + 0x10, u64::from(D), s1, 0x01E0_36F4);
                billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES2);
                fx.mem.set16(D + 0xB8, 0);
                fx.mem.set16(D + 0xB0, 0);
            }
            DUST_T => {
                let sub = fx.mem.i16(t + 0x56);
                cell(fx, sp + 0x80, 0x14, sub);
                let w = fx.mem.read_u32(TEX + 4);
                fx.mem.write_u32(D + 0x88, w);
                fx.mem.set16(D + 0xB8, 5);
                let a = fx.mem.i16(t + 0xA);
                fx.mem.set16(D + 0xC0, a);
                for c in 0..3 {
                    let w = fx.mem.read_u32(t + 0x10 + 4 * c);
                    fx.mem.write_u32(D + 0x20 + 4 * c, w);
                }
                quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
            STQ => {
                let w = fx.mem.read_u32(TEX + 8);
                fx.mem.write_u32(D + 0x88, w);
                let sub = fx.mem.i16(t + 0x56);
                cell(fx, sp + 0x90, 0x8C, sub);
                fx.mem.set16(D + 0xB8, 0);
                for c in 0..3 {
                    let w = fx.mem.read_u32(t + 0x10 + 4 * c);
                    fx.mem.write_u32(D + 0x20 + 4 * c, w);
                }
                let deg = fx.mem.i16(t + 0x5C);
                let s = l2::sin_deg(fx, SINES, deg, 0);
                let v = ee::add(fx.mem.f(D + 0x20), ee::mul(fx.mem.f(t + 0x30), s));
                fx.mem.setf(D + 0x20, v);
                let deg = fx.mem.i16(t + 0x5C);
                let c = l2::sin_deg(fx, SINES, deg, 1);
                let v = ee::add(fx.mem.f(D + 0x24), ee::mul(fx.mem.f(t + 0x30), c));
                fx.mem.setf(D + 0x24, v);
                quads4::ring_band_stq(fx, STQ_AT, D, t, sp + 0xC0, 0x20);
                fx.mem.set16(D + 0xB8, 0);
            }
            _ => {}
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left.wrapping_sub(1));
    }
    match fx.mem.i16(COUNTER) {
        0x46 => 1,
        0xFF => 3,
        0x51..=0xF4 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` frame `sub` (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E03A90(D, step, 0): the first free task cleared and set off at
/// +0x250 with the colour +0xBC94 twice and the index +0xBCEA (+0x50,
/// +0x52). Frame 0x60 (ra, s4 .. s0: the emitter's s0 = D, the entry's s1
/// = 0, s2 = the parameter, the runner's s3 / s4 (0 [H, as b747])).
fn spawn(fx: &mut Fx, f: u32, p: u64, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, 0), (0, u64::from(D))]);
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
            l2::to_i16x4(fx, t, SPAWN_AT, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(SPAWN_AT + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x50, a);
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x52, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E02290(D, 0): the update counter, the spawns and the angle's turn.
/// Frame 0x20 (ra, the entry's s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, EMIT_RA), (0, u64::from(D))]);
    // The parameter (the entry's s2) for the spawns' saves.
    let p = param as i64 as u64;
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let s1 = Some(0u64);
    if fx.mem.i16(COUNTER) == 0x19 {
        fx.mem.set16(INDEX, 0);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, SPAWN_AT, u64::from(D), s1, 0x01E0_22FC);
        spawn(fx, DUST, p, 0x01E0_2310);
        spawn(fx, MOTE, p, 0x01E0_2324);
        fx.mem.set16(INDEX, 1);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE_B);
        node_position_saving(fx, h, slot, SPAWN_AT, u64::from(D), s1, 0x01E0_2350);
        spawn(fx, DUST, p, 0x01E0_2364);
        spawn(fx, MOTE, p, 0x01E0_2378);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x27..0xE6).contains(&c) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE_A);
        node_position_saving(fx, h, slot, SPAWN_AT, u64::from(D), s1, 0x01E0_23BC);
        let z = fx.mem.f(SPAWN_AT + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 8.0) } else { ee::add(z, 8.0) };
        fx.mem.setf(SPAWN_AT + 8, z);
        if fx.mem.i16(COUNTER) < 0x4C {
            spawn(fx, BAND, p, 0x01E0_2430);
        }
        spawn(fx, STQ, p, 0x01E0_2444);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x4D..0xE6).contains(&c) {
        let h = fx.target_handle();
        let slot = fx.mem.i16(TNODE);
        node_position_saving(fx, h, slot, SPAWN_AT, u64::from(D), s1, 0x01E0_2488);
        let c = fx.mem.i16(COUNTER);
        if fx.div(c, 3).1 == 1 {
            spawn(fx, DUST_T, p, 0x01E0_24C8);
        }
    }
    let a = fx.mem.i16(ANGLE) + 4;
    fx.mem.set16(ANGLE, a);
    let a = fx.mem.i16(ANGLE);
    if a >= 0x169 {
        let (_, h) = fx.div(a, 0x168);
        fx.mem.set16(ANGLE, h);
    }
    fx.leave(0x20);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// A 0x20-frame step's prologue (ra, the entry's s0 = D).
fn frame20(fx: &mut Fx) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
}

/// A 0x30-frame step's prologue (ra, the entry's s1 = 0, s0 = D).
fn frame30(fx: &mut Fx) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
}

/// The alpha byte +0x63 while the age is below the life +6 (`hold`), then
/// out by `down.1` while at least `down.0`, else the task ends.
fn hold_fade(fx: &mut Fx, t: u32, hold: u8, down: (u8, u8)) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, hold);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= down.0 {
            fx.mem.write_u8(t + 0x63, a - down.1);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
}

/// 0x1E02530 (D, task; frame 0x30): a green sway band (`ring_band_fy`,
/// the inner colour flickering (0x10, 0x80, 0x10) / (0x10, 0x60, 0x10),
/// outer grey 0x80, alpha +0x67 in by 0x40 under 0x80 for 2 updates, then
/// out by 0x13) from radius 35 growing 35.5 (the inner +0x38 less),
/// turned 0x4000, swaying to +-20 in +0x34 by 5 (by the facing), +0x30 up
/// to 10 by 0.2, +0x38 up to 30 by 13, the angle +0x5C turning 9.
fn band(fx: &mut Fx, t: u32) {
    frame30(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x420C_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 2 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x67, a + 0x40);
        }
    } else if a >= 0x15 {
        fx.mem.write_u8(t + 0x67, a - 0x13);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 {
        l2::rgb(fx, t + 0x60, 0x10, 0x80, 0x10);
    } else {
        l2::rgb(fx, t + 0x60, 0x10, 0x60, 0x10);
    }
    fx.mem.write_u32(t + 0x20, 0x4000);
    let x = fx.mem.f(t + 0x34);
    if fx.mem.i16(FACING) != 0 {
        if x < 20.0 {
            fx.mem.setf(t + 0x34, ee::add(x, 5.0));
        }
    } else if !(x <= -20.0) {
        fx.mem.setf(t + 0x34, ee::sub(x, 5.0));
    }
    let v = fx.mem.f(t + 0x30);
    if v < 10.0 {
        fx.mem.setf(t + 0x30, ee::add(v, f32::from_bits(0x3E4C_CCCD)));
    }
    let v = fx.mem.f(t + 0x38);
    if v < 30.0 {
        fx.mem.setf(t + 0x38, ee::add(v, 13.0));
    }
    let r = ee::add(fx.mem.f(t + 0x48), 35.5);
    fx.mem.setf(t + 0x48, r);
    let r = ee::sub(r, fx.mem.f(t + 0x38));
    fx.mem.setf(t + 0x4C, r);
    let a = fx.mem.i16(t + 0x5C) + 9;
    fx.mem.set16(t + 0x5C, a);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02780 / 0x1E02E50 (D, task): a dust ring (`ring_facing_flat_z`):
/// cell rand % 7, grey 0x80, random angle, turn rand % 8 (+0xA), radius
/// `radius` (+0x50 / +0xE / +8 / +6 from `r`: (radius base, m), (growth
/// base, m), row m, (life base, m)); alpha 0x80 during the life, then out
/// by `down.1`; `grow`: +0x48 grows 2 every update.
fn dust_by(fx: &mut Fx, t: u32, f: u32, size: u32, r: [(i32, i32); 4], down: (u8, u8), grow: bool) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, size);
        let v = rmod(fx, t, f + 0x28, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x54, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rpow2(fx, t, f + 0x70, 8);
        fx.mem.set16(t + 0xA, v);
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        let v = rmod(fx, t, f + 0x94, r[0].1);
        fx.mem.set16(t + 0x50, v + r[0].0);
        let v = rmod(fx, t, f + 0xB4, r[1].1);
        fx.mem.set16(t + 0xE, v + r[1].0);
        let v = rmod(fx, t, f + 0xD4, r[2].1);
        fx.mem.set16(t + 8, v + r[2].0);
        let v = rmod(fx, t, f + 0xF0, r[3].1);
        fx.mem.set16(t + 6, v + r[3].0);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_fade(fx, t, 0x80, down);
    if grow {
        let v = ee::add(fx.mem.f(t + 0x48), 2.0);
        fx.mem.setf(t + 0x48, v);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    age(fx, t);
    fx.leave(0x20);
}

fn dust(fx: &mut Fx, t: u32) {
    dust_by(fx, t, DUST, 0x4220_0000, [(0xA, 0x32), (0x10, 0x1E), (0, 0x23), (3, 5)], (0xA, 8), false);
}

fn dust_t(fx: &mut Fx, t: u32) {
    dust_by(fx, t, DUST_T, 0x4302_0000, [(5, 0xA), (0xD, 0xF), (0, 0x23), (3, 5)], (0xB, 9), true);
}

/// 0x1E02910 (D, task): a green mote ring (`ring_quads` of 10 from the
/// bytes +0xBDF4): size 10..19, cell rand % 4, (0x10, 0x80, 0) random
/// angles, radius 150..299 growing 1..3, row 1..8, life 2..4; alpha 0x60
/// during the life, then out by 11; the node index +0x52 from 1 up by one
/// every update.
fn mote(fx: &mut Fx, t: u32) {
    frame20(fx);
    let f = MOTE;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = rpow2(fx, t, f + 0x50, 4);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x10, 0x80, 0);
        for (o, site) in [(0x20u32, f + 0x84), (0x24, f + 0xA0), (0x28, f + 0xBC)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, f + 0xE0, 0x96);
        fx.mem.set16(t + 0x50, v + 0x96);
        let v = rmod(fx, t, f + 0x100, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0x11C, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = rpow2(fx, t, f + 0x13C, 8);
        fx.mem.set16(t + 8, v + 1);
        let v = rmod(fx, t, f + 0x15C, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_fade(fx, t, 0x60, (0xD, 0xB));
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, v);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02B10 (D, task; frame 0x30): a grey (0x50) STQ ring
/// (`ring_band_stq`): radius 3 + 4 (the outer +0x4C up to 20 by 0.8, the
/// inner +0x48 by 0.8), turned 0x4000, row 3..4, at the angle +0xBCE0
/// turning 9; alpha 0x80 for 25 updates, then out by 14; +0x3C up to 3 by
/// 1, +0x30 up to 15 by 1.5, z 6 behind / in front by the facing.
fn stq(fx: &mut Fx, t: u32) {
    frame30(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4040_0000);
        fx.mem.write_u32(t + 0x4C, 0x4080_0000);
        fx.mem.write_u32(t + 0x20, 0x4000);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.set16(t + 0x58, 2);
        let v = rpow2(fx, t, STQ + 0x4C, 2);
        fx.mem.set16(t + 8, v + 3);
        let a = fx.mem.i16(ANGLE);
        fx.mem.set16(t + 0x5C, a);
        l2::rgb(fx, t + 0x60, 0x50, 0x50, 0x50);
        l2::rgb(fx, t + 0x64, 0x50, 0x50, 0x50);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x19 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, 0x80);
        }
    } else if a >= 0x10 {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.f(t + 0x3C);
        if v < 3.0 {
            fx.mem.setf(t + 0x3C, ee::add(v, 1.0));
        }
        let v = fx.mem.f(t + 0x30);
        if v < 15.0 {
            fx.mem.setf(t + 0x30, ee::add(v, 1.5));
        }
        let v = fx.mem.f(t + 0x4C);
        if v < 20.0 {
            fx.mem.setf(t + 0x4C, ee::add(v, f32::from_bits(0x3F4C_CCCD)));
        }
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 6.0) } else { ee::add(z, 6.0) };
        fx.mem.setf(t + 0x18, z);
        let v = ee::add(fx.mem.f(t + 0x48), f32::from_bits(0x3F4C_CCCD));
        fx.mem.setf(t + 0x48, v);
    }
    let a = fx.mem.i16(t + 0x5C) + 9;
    fx.mem.set16(t + 0x5C, a);
    age(fx, t);
    fx.leave(0x30);
}
