//! BMODULE b465 (card 281), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02310, data area 0x1ED0000, 0xB630 bytes) [F]
//!: b606's program (b631's pool of 400 tasks of
//! 0x70 bytes at D+0x2A0, alive +0x58, step +0x44 through `jalr`; spawned
//! by `spawn::task400` at 0x1E029F0) with b098's unit flash (0x1E02E90:
//! the attacker drawn into the target, the flash tasks 0x1E02040 stepped)
//! and the cross quads 086d46d618c2 (0x1E02B80,
//! [`crate::blib::billboard::quad_cross`], new). Update 1 a white flash (in by
//! 9 for 70 updates, out by 15, gone by update 111) and the stage dimmed
//! (0x1E01F30); 1..24 two grey rings a update (b400's ring, 0x1E01560) at
//! the attacker's node 2, 7..16 to either side, 5 up; 27..50 a cross of
//! orange quads a update (0x1E01B10) at each of nodes 0x2B / 0x52 (0..9
//! to the side), sliding 5 along z, which at age 30 throws two orange
//! star bursts (0x1E01880, ten quads round y, rising 15 a update) and a
//! larger grey ring (0x1E016F0). Returns 1 on update 46, 4 on 125, 2 on
//! 90, 6 on 57..84, 8 once the 130-update timer D+0xB210 has run out.
use crate::b076::node_position_saving;
use crate::blib::{billboard, spawn::task400, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const RING: u32 = 0x01E0_1560;
const RING_L: u32 = 0x01E0_16F0;
const STAR: u32 = 0x01E0_1880;
const CROSS: u32 = 0x01E0_1B10;
const ARC: u32 = 0x01E0_1E10;
const LIGHT: u32 = 0x01E0_1F30;
/// The flash tasks, run by the unit flash 0x1E02E90.
const FLASH: u32 = 0x01E0_2040;
/// The return address of the entry's `jalr` into a step (0x1E02620 + 8).
const TASK_RA: u64 = 0x01E0_2628;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const LAYER: u32 = D + 0xB224;
const FLASH_DESC: u32 = D + 0xB228;
const NODES: u32 = D + 0xB244;
const FACING: u32 = D + 0xB2B4;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02310 (s0 = the flash count, s1 = 0 / the live count,
/// s2 = D).
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
    l2::sine_table(fx, D + 0xB4BC);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x82);
    for (k, node) in [0x2B, 0x52, 2, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_2508);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_2538, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_258C);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s0: the flash tasks counted here (the steps' saved s0).
    let mut flashes = 0u32;
    // a3 as the last call left it, for the unit flash's first 0x207BB0.
    let mut a3 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        let frame = match f {
            0 => continue,
            FLASH => {
                flashes += 1;
                continue;
            }
            CROSS => 0x30,
            _ => 0x20,
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(flashes))]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(flashes))]);
        }
        match f {
            RING => ring(fx, t, RING, (6, 6), (100, 0x32), (0x14, 0x10), &mut a3),
            RING_L => ring(fx, t, RING_L, (0xF, 0xA), (100, 0x32), (0x32, 0x2E), &mut a3),
            STAR => star(fx, t, &mut a3),
            CROSS => cross(fx, t, flashes, &mut a3),
            ARC => arc(fx, t, &mut a3),
            LIGHT => light(fx, t, &mut a3),
            _ => panic!("b465: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        let _ = live;
        for (f, local, site) in [(RING, 0x60u32, 0x01E0_26E4u32), (RING_L, 0x70, 0x01E0_2788)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            tex(fx, TEX);
            cell(fx, t, sp + local, 0x14);
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand_from(flashes, site + 8);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        for (f, local, kind) in [(STAR, 0x80u32, 1), (CROSS, 0x90, 0)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            tex(fx, D + 0x29C);
            cell(fx, t, sp + local, 0x9B);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.set16(D + 0xB0, kind);
            billboard::quad_cross(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
            a3 = 0;
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    if flashes != 0 {
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x2E => 1,
        0x7D => 4,
        0x5A => 2,
        0x39..=0x54 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// The atlas cell `id` frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

fn node(fx: &mut Fx, k: u32, s1: u64, ra: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES + 2 * k);
    node_position_saving(fx, h, slot, POS, u64::from(D), Some(s1), ra);
}

/// 0x1E01230(D, 0): the update counter and the spawns (frame 0x20: ra,
/// the entry's s0 = 0). The spawns' saves: the emitter's s0 = D, the
/// entry's s1 = 0 / s2 = D / s3 = the parameter / s4 = 0.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x20);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x10, 0x01E0_25E8), (0, 0)]);
    let go = |fx: &mut Fx, f: u32, ra: u32| {
        task400(fx, D, f, 0, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, p), (0x20, u64::from(D)), (0x10, 0), (0, u64::from(D))]);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    if (0x1B..0x33).contains(&c) {
        for (k, ra_node, ra_rand, ra_spawn, toward) in [(0u32, 0x01E0_12A0u32, [0x01E0_12BCu32, 0x01E0_12F4], 0x01E0_1334u32, true), (1, 0x01E0_1354, [0x01E0_13A8, 0x01E0_1370], 0x01E0_13E8, false)] {
            node(fx, k, 0, ra_node);
            // facing: node 0x2B pushed +x, node 0x52 -x (the other way
            // round when not).
            let plus = (fx.mem.i16(FACING) != 0) == toward;
            let v = rmod(fx, D, if plus { ra_rand[0] } else { ra_rand[1] }, 0xA);
            let x = fx.mem.f(POS);
            let d = ee::from_i32(v);
            fx.mem.setf(POS, if plus { ee::add(x, d) } else { ee::sub(x, d) });
            go(fx, CROSS, ra_spawn);
        }
    }
    if fx.mem.i16(COUNTER) < 0x19 {
        for (plus, ra_node, ra_rand, ra_spawn) in [(true, 0x01E0_1420u32, 0x01E0_1428u32, 0x01E0_1478u32), (false, 0x01E0_1498, 0x01E0_14A0, 0x01E0_14F0)] {
            node(fx, 2, 0, ra_node);
            let v = rmod(fx, D, ra_rand, 0xA);
            let x = fx.mem.f(POS);
            let d = ee::from_i32(v + 7);
            fx.mem.setf(POS, if plus { ee::add(x, d) } else { ee::sub(x, d) });
            let y = ee::add(fx.mem.f(POS + 4), 5.0);
            fx.mem.setf(POS + 4, y);
            go(fx, RING, ra_spawn);
        }
    }
    if fx.mem.i16(COUNTER) == 1 {
        go(fx, LIGHT, 0x01E0_151C);
    }
    if fx.mem.i16(COUNTER) == 1 {
        go(fx, FLASH, 0x01E0_1548);
    }
    fx.leave(0x20);
}

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

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01560 / 0x1E016F0 (`f`) (D, task): b400's grey ring of quads
/// (0x1E01BA0) with its size rand % s.0 + s.1, radius rand % r.0 + r.1
/// and growth rand % g.0 + g.1: cell row 0..2, a random angle, row 0..34,
/// life 3..5; in by 0x40 under 0xC0, out by 14.
fn ring(fx: &mut Fx, t: u32, f: u32, s: (i32, i32), r: (i32, i32), g: (i32, i32), a3: &mut u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x1C + 8, s.0);
        fx.mem.setf(t + 0x48, ee::from_i32(v + s.1));
        let v = rmod(fx, t, f + 0x48 + 8, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        *a3 = 0x80;
        let v = rmod(fx, t, f + 0x74 + 8, r.0);
        fx.mem.set16(t + 0x50, v + r.1);
        let v = rmod(fx, t, f + 0x94 + 8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB0 + 8, g.0);
        fx.mem.set16(t + 0xE, v + g.1);
        let v = rmod(fx, t, f + 0xD0 + 8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xEC + 8, 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0x10 {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
}

/// The velocity +0x30 / +0x34 / +0x38 (rand % 100 - 50) / 20 and the
/// spins +4 / +2 / +0 (rand % 0x800 - 0x400) of a star / cross's first
/// step (the rands from `at`).
fn fly(fx: &mut Fx, t: u32, at: [u32; 3]) {
    for (o, ra) in [(0x34u32, at[0]), (0x30, at[1]), (0x38, at[2])] {
        let v = rmod(fx, t, ra, 0x64);
        fx.mem.setf(t + o, ee::from_i32(v - 0x32));
    }
    for o in [0x34u32, 0x30, 0x38] {
        let q = ee::div(fx.mem.f(t + o), 20.0);
        fx.mem.setf(t + o, q);
    }
}

fn spins(fx: &mut Fx, t: u32, at: [u32; 3]) {
    for (o, ra) in [(4u32, at[0]), (2, at[1]), (0, at[2])] {
        let v = fx.rand_from(t, ra);
        fx.mem.set16(t + o, v % 0x800 - 0x400);
    }
}

/// 0x1E01880 (D, task): an orange (0xFF, 0x80, 0x40) star burst: width 3,
/// height 150 growing by 10 and rising 15 a update (+0xC through
/// 0x1A2B80), cell 2, kind +0x52 0..3, random angles +0x20..+0x28, life
/// 10; in by 0x40 under 0x80, out by 9.
fn star(fx: &mut Fx, t: u32, a3: &mut u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 3);
        fx.mem.set16(t + 0xA, 0x96);
        fx.mem.set16(t + 0xC, 0);
        fx.mem.set16(t + 0x56, 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x40);
        fx.mem.set16(t + 0x58, 2);
        fly(fx, t, [0x01E0_18D8, 0x01E0_1904, 0x01E0_1930]);
        spins(fx, t, [0x01E0_1988, 0x01E0_19A8, 0x01E0_19C8]);
        let v = fx.rand_from(t, 0x01E0_19E8);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
        for (o, ra) in [(0x20u32, 0x01E0_1A08u32), (0x24, 0x01E0_1A24), (0x28, 0x01E0_1A40)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 6, 0xA);
        *a3 = 0x40;
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let h = fx.mem.i16(t + 0xA) + 0xA;
        fx.mem.set16(t + 0xA, h);
        let y = ee::add(ee::from_i32(fx.mem.i16(t + 0xC)), 15.0);
        fx.fix_residue(y, 0x01E0_1AE8);
        fx.mem.set16(t + 0xC, ee::fixsfsi(y));
    }
    age(fx, t);
}

/// 0x1E01B10 (D, task; frame 0x30, s1 = D): an orange cross: width 4,
/// height 200, cell 2, a random angle +0x5A, kind +0x52 0..3, turned
/// 0x4000 (0xC000 when not facing), life 22; in by 0x40 under 0x80, out
/// by 9; sliding 5 along -z (facing) / +z; at age 30 two stars and a large
/// ring spawned at its point.
fn cross(fx: &mut Fx, t: u32, flashes: u32, a3: &mut u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 4);
        fx.mem.set16(t + 0xA, 0xC8);
        fx.mem.set16(t + 0xC, 0);
        fx.mem.set16(t + 0x56, 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x40);
        fx.mem.set16(t + 0x58, 2);
        fly(fx, t, [0x01E0_1B70, 0x01E0_1B9C, 0x01E0_1BC8]);
        let v = rmod(fx, t, 0x01E0_1C20, 0x168);
        fx.mem.set16(t + 0x5A, v);
        spins(fx, t, [0x01E0_1C3C, 0x01E0_1C5C, 0x01E0_1C7C]);
        let v = fx.rand_from(t, 0x01E0_1C9C);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
        let turn = if fx.mem.i16(FACING) != 0 { 0x4000 } else { 0xC000 };
        fx.mem.write_u32(t + 0x20, turn);
        fx.mem.set16(t + 6, 0x16);
        *a3 = 0x40;
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    age(fx, t);
    let z = fx.mem.f(t + 0x18);
    let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
    fx.mem.setf(t + 0x18, z);
    if fx.mem.i16(t + 0x54) == 0x1E {
        for k in 0..4 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        // task400's saves: the step's s0 = the task / s1 = D, the entry's
        // s2 = D / s3 = the pool pointer / s4 = the index (left [H]).
        let saves = |ra: u32| [(0x50u32, u64::from(ra)), (0x10, u64::from(D)), (0, u64::from(t))];
        task400(fx, D, STAR, 0, &saves(0x01E0_1DD0));
        task400(fx, D, STAR, 0, &saves(0x01E0_1DE4));
        task400(fx, D, RING_L, 0, &saves(0x01E0_1DF8));
        *a3 = 0;
        let _ = flashes;
    }
}

/// 0x1E01E10 (D, task): b606's arc ring (never spawned here [F]).
fn arc(_fx: &mut Fx, t: u32, a3: &mut u32) {
    let _ = (t, a3);
    panic!("b465: the arc 0x1E01E10 is never spawned");
}

/// 0x1E01F30 (D, task): b065's stage light (0x1F7F10) down by 10 to 0x30
/// for 51 updates, back up by 10 to 0x80 once the timer is under 20; 0x80
/// and the task's end when the timer is at 3.
fn light(fx: &mut Fx, t: u32, a3: &mut u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x33 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x14 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
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
    *a3 = 0;
}

/// 0x1E02040(D, t): b098's flash step in white: alpha up by 9 below 0x80
/// for 70 updates, then down by 15; the task ends from update 111.
/// `regs` the unit flash's s0 / s1 (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_2F9C), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f20 / f21: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, xy) in [(0x20u32, 0x6FF8u16), (0x30, 0x8FF8)] {
        fx.mem.write_u16(d + at, xy);
        fx.mem.write_u16(d + at + 2, xy);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    for (o, v) in [(0x18u32, ee::div(w, 512.0)), (0x1C, ee::div(h, 512.0)), (0x28, ee::div(ee::sub(512.0, w), 512.0)), (0x2C, ee::div(ee::sub(512.0, h), 512.0))] {
        fx.mem.setf(d + o, v);
    }
    fx.submit(d, 0, 0);
    if fx.mem.i16(t + 0x54) < 0x46 {
        fade_in(fx, t, 0x63, 0x80, 9);
    } else {
        fade_out(fx, t, 0x63, 0x11, 0xF);
    }
    if fx.mem.i16(COUNTER) >= 0x6F {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E02E90(D) (b098's 0x1E02610 word for word but the step): in a packet
/// layer (0x2078A0) the attacker drawn into the target (0x1F8270 filter
/// 0; ALPHA 0x48), TEXFLUSH before and after; every flash task stepped
/// (0x1E02040); the layer linked into the ordering table at key 0 when
/// any task was alive.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xB0);
    let d = sp + 0x58;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x98, sp + 0xA0);
    fx.texflush(regs);
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        if fx.mem.read_u32(t + 0x44) == FLASH {
            flash(fx, t, [k, live]);
        }
    }
    if live != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}
