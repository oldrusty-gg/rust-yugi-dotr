//! BMODULE b293 (card 205), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02FE0, data area 0x1ED0000, 0xD030 bytes) [F]
//!: b703's layout (b590's 400-task pool of 0x70
//! bytes at D+0x2A0, alive +0x58, step +0x44 through `jalr`, 0x1E02FD0
//! the idle step; every global after the pool 0x1900 higher, four 20-point
//! node histories D+0xB1D0 / +0xB310 / +0xB450 / +0xB590) with b065's
//! steps: spawned by 0x1E03920 (03c321c0da42, `spawn::task400_b4`'s
//! layout copy: colour D+0xCB14, +0x50 from D+0xCBB0) and drawn with the
//! second library: smoke rings (7369422df7f8 at 0x1E009B0), arcs
//! (e6dce326551d at 0x1E00F60), textured arcs (3a2db45c5a9a at 0x1E01700)
//! and b703's gouraud ribbon (0x1E03AB0) between two pairs of histories.
//! Update 1 the green ribbon task (nodes 0x28 / 0x29 / 0x3C / 0x3D into
//! the histories to update 0x4A) and the stage light; 0x3D..0x48 two grey
//! rings a update at nodes 0x28 / 0x3C; 0x3D..0x6E at node 1 (15 down, 15
//! toward the facing; the point kept from 0x48) a green textured arc a
//! update (two from 0x44), rings from 0x46 / 0x48 and on 0x46 an arc; 0x42
//! a frame flash. Returns 1 on update 0x3C, 4 on 0x96, 2 on 0x91, 6 on
//! 0x47..0x8B, 8 once the 0xA5-update timer D+0xCB10 has run out.
use crate::blib::{billboard, quads4, spawn, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const RING_A: u32 = 0x01E0_1E00;
const RING_B: u32 = 0x01E0_1FB0;
const RING_C: u32 = 0x01E0_2140;
const ARC_T: u32 = 0x01E0_22F0;
const TRAILS: u32 = 0x01E0_2780;
const ARC: u32 = 0x01E0_2B10;
const LIGHT: u32 = 0x01E0_2C10;
const FLASH: u32 = 0x01E0_2D20;
const IDLE: u32 = 0x01E0_2FD0;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3428;
/// The four node histories (20 points of 16 bytes).
const HIST: [u32; 4] = [D + 0xB1D0, D + 0xB310, D + 0xB450, D + 0xB590];
/// Node 1's point kept by the emitter.
const KEPT: u32 = D + 0xCAD0;
const TEX: u32 = D + 0xCAF0;
const TIMER: u32 = D + 0xCB10;
const NODES: u32 = D + 0xCB44;
const SPAWN_K: u32 = D + 0xCBB0;
const FACING: u32 = D + 0xCBB4;
const COUNTER: u32 = D + 0xCBB8;
const BYTES: u32 = D + 0xCBBA;
const BYTES_B: u32 = D + 0xCDBA;
const TAB: u32 = D + 0xCEBC;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn node(fx: &mut Fx, k: u32) -> i32 {
    fx.mem.i16(NODES + 2 * k)
}

/// The entry 0x1E02FE0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0xCB1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xD030);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xCBB6, param);
    fx.mem.set32(TIMER, 0xA5);
    for (k, n) in [0x28, 0x29, 0x3C, 0x3D, 1, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xCB54, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_31D0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for ra in [0x01E0_3200u32, 0x01E0_3254] {
        for k in 0..0x100u32 {
            let v = rmod(fx, k, ra, 0x1E);
            fx.mem.set8(BYTES + k, v - 0xF);
        }
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, k, 0x01E0_32AC, 0x1E);
        fx.mem.set8(BYTES + k, v);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_32FC, 0x1E);
        fx.mem.set8(BYTES_B + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.set8(BYTES_B + k, 0);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_338C);
        fx.mem.set16(D + 0xCB64 + 2 * k, v % 8);
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
        if f == 0 || f == IDLE {
            continue;
        }
        // The step's prologue: ra, the entry's s1 = D, s0 = 0.
        let frame = match f {
            ARC_T | ARC => 0x30,
            TRAILS => 0x40,
            FLASH => 0x90,
            _ => 0x20,
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        }
        match f {
            RING_A => ring(fx, t, RING_A, (0x14, 0xF, 0x1A), true),
            RING_B => ring(fx, t, RING_B, (0x14, 0xA, 0x10), false),
            RING_C => ring(fx, t, RING_C, (0x32, 0xA0, 0x1A), true),
            ARC_T => arc_t(fx, t),
            TRAILS => trails(fx, t, s),
            ARC => arc(fx, t),
            LIGHT => light(fx, t),
            FLASH => flash(fx, t, s),
            _ => panic!("b293: unknown task step {f:#x}"),
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
        draw(fx, sp, t, s0, k);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x3C => 1,
        0x96 => 4,
        0x91 => 2,
        0x47..=0x8B => 6,
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

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32, k: u32) {
    let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
    // (step, texture, local, B8, rand's return, B0, mode, byte table)
    let rings = [
        (RING_A, TEX, 0x50u32, 4, 0x01E0_34D4u32, 1, 0x21, BYTES),
        (RING_B, TEX, 0x60, 4, 0x01E0_3578, 0, 0x21, BYTES),
        (RING_C, TEX + 4, 0x70, 8, 0x01E0_3618, 0, 0x11, BYTES_B),
    ];
    for (g, tx, local, n, site, b0, mode, table) in rings {
        if f(fx) == g {
            tex(fx, tx);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + local, 0x14, c);
            fx.mem.set16(D + 0xB8, n);
            let v = fx.rand_from(s0, site);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, b0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, mode, table);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    if f(fx) == ARC_T {
        let kind = |fx: &mut Fx| fx.mem.i16(t + 0x50);
        if kind(fx) == 0 {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x97, c);
            tex(fx, D + 0x29C);
        }
        if kind(fx) == 1 {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x90, 0x9B, c);
            tex(fx, D + 0x29C);
        }
        if kind(fx) == 2 {
            tex(fx, TEX);
            fx.mem.set16(D + 0xBE, 0);
            let c = fx.mem.i16(t + 0x56);
            for (o, v) in [(0x94u32, c << 5), (0x96, 0), (0x98, 0x20), (0x9A, 0x40)] {
                fx.mem.set16(D + o, v);
            }
        }
        fx.mem.set16(D + 0xB8, 0);
        for k in 0..3 {
            let v = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(D + 0x20 + 4 * k, v);
        }
        fx.mem.set16(D + 0xB8, 0x24);
        if kind(fx) == 2 {
            fx.mem.set16(D + 0xB8, 6);
        }
        fx.mem.set16(D + 0xBE, 0);
        quads4::arc_strips_tex(fx, D, t, 0x20, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f(fx) == TRAILS {
        for (pair, ra) in [((HIST[0], HIST[1]), 0x01E0_37D8u64), ((HIST[2], HIST[3]), 0x01E0_37F4)] {
            let saves = [u64::from(s0), u64::from(D), u64::from(t - 0x2A0), u64::from(k), 0, ra];
            crate::b703::ribbon(fx, t, pair.0, pair.1, saves);
        }
    }
    if f(fx) == ARC {
        fx.mem.set16(D + 0xB8, 0x24);
        billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
}

/// 0x1E03920(D, step, 0): `spawn::task70` on this record's layout, its
/// frame's saves the caller's s0..s4 and the return address `ra`.
fn spawn(fx: &mut Fx, step: u32, param: i32, ra: u32) -> i32 {
    let p = spawn::pool70_b4("03c321c0da42").expect("layout");
    let saves = [(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, param as i64 as u64), (0x10, u64::from(D)), (0, u64::from(D))];
    spawn::task70(fx, D, p, step, 0, &saves)
}

fn node_pos(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let slot = node(fx, k);
    fx.node_position(h, slot, out);
}

/// 0x1E01B10(D, 0): the update counter and the emitters (frame 0x20: ra,
/// the entry's s0 = 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_33E8), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 1 {
        spawn(fx, TRAILS, param, 0x01E0_1B60);
    }
    if c(fx) == 1 {
        spawn(fx, LIGHT, param, 0x01E0_1B8C);
    }
    if c(fx) == 0x42 {
        spawn(fx, FLASH, param, 0x01E0_1BB8);
    }
    if (0x3D..0x6F).contains(&c(fx)) {
        if c(fx) < 0x49 {
            node_pos(fx, 4, KEPT);
        }
        for k in 0..4 {
            let w = fx.mem.read_u32(KEPT + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        let y = ee::sub(fx.mem.f(POS + 4), 15.0);
        fx.mem.setf(POS + 4, y);
        let z = fx.mem.f(POS + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 15.0) } else { ee::add(z, 15.0) };
        fx.mem.setf(POS + 8, z);
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, ARC_T, param, 0x01E0_1C9C);
        fx.mem.set16(SPAWN_K, 1);
        if c(fx) >= 0x44 {
            spawn(fx, ARC_T, param, 0x01E0_1CD8);
        }
        if c(fx) >= 0x48 {
            spawn(fx, RING_A, param, 0x01E0_1D04);
        }
        if c(fx) >= 0x46 {
            spawn(fx, RING_C, param, 0x01E0_1D30);
        }
        if c(fx) == 0x46 {
            spawn(fx, ARC, param, 0x01E0_1D5C);
        }
    }
    if (0x3D..0x49).contains(&c(fx)) {
        node_pos(fx, 0, POS);
        spawn(fx, RING_B, param, 0x01E0_1DB4);
        node_pos(fx, 2, POS);
        spawn(fx, RING_B, param, 0x01E0_1DE8);
    }
    fx.leave(0x20);
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

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

/// 0x1E01E00 / 0x1E01FB0 / 0x1E02140 (`f`) (D, task): a grey ring (b703's
/// ring with the size rand % k.0 + k.1 and growth rand % 20 + k.2):
/// radius 10..49, life 3..5; in by 0x40 under 0xC0, out by 0xE; the
/// radius grows by +0xE when `grows`.
fn ring(fx: &mut Fx, t: u32, f: u32, k: (i32, i32, i32), grows: bool) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, k.0);
        fx.mem.setf(t + 0x48, ee::from_i32(v + k.1));
        let v = rmod(fx, t, f + 0x50, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x7C, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, f + 0x9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB8, 0x14);
        fx.mem.set16(t + 0xE, v + k.2);
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
    if grows && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E022F0 (D, task): b065's textured arc in green (kind 0: (0x80, 0xC0,
/// 0x80) / 8 on even updates, (0xB0, 0xFF, 0xB0) / (0x30, 0x38, 0x38) on
/// odd, 8 updates; kind 1: (0x40, 0x80, 0x40) / 0 and (0x60, 0xCF, 0x60) /
/// 0x30, 10; kind 2: (0xC0, 0x80, 0x80) / 0x10 and (0xFF, 0xA0, 0xA0) /
/// 0x30, 10), alpha 0x80 for the life then down by 5 (the task ends under
/// 7).
fn arc_t(fx: &mut Fx, t: u32) {
    type C = (i32, i32, i32);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x430C_0000);
        fx.mem.setf(t + 0x4C, ee::sub(140.0, 58.0));
        let odd = |fx: &mut Fx| fx.mem.i16(COUNTER) % 2 == 1;
        let colours: [((C, C), (C, C)); 3] = [
            (((0x80, 0xC0, 0x80), (8, 8, 8)), ((0xB0, 0xFF, 0xB0), (0x30, 0x38, 0x38))),
            (((0x40, 0x80, 0x40), (0, 0, 0)), ((0x60, 0xCF, 0x60), (0x30, 0x30, 0x30))),
            (((0xC0, 0x80, 0x80), (0x10, 0x10, 0x10)), ((0xFF, 0xA0, 0xA0), (0x30, 0x30, 0x30))),
        ];
        for kind in 0..3 {
            if fx.mem.i16(t + 0x50) == kind {
                let (even_c, odd_c) = colours[kind as usize];
                if !odd(fx) {
                    l2::rgb(fx, t + 0x64, even_c.0 .0, even_c.0 .1, even_c.0 .2);
                    l2::rgb(fx, t + 0x60, even_c.1 .0, even_c.1 .1, even_c.1 .2);
                }
                if odd(fx) {
                    l2::rgb(fx, t + 0x64, odd_c.0 .0, odd_c.0 .1, odd_c.0 .2);
                    l2::rgb(fx, t + 0x60, odd_c.1 .0, odd_c.1 .1, odd_c.1 .2);
                }
                if kind != 0 {
                    fx.mem.write_u32(t + 0x34, 0x4120_0000);
                }
                fx.mem.set16(t + 6, if kind == 0 { 8 } else { 0xA });
            }
        }
        for (kind, a, b) in [(0, 0xA, 0xA), (1, 0xA, 3), (2, 0x3C, 0x3C)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.set16(t + 0x5A, a);
                fx.mem.set16(t + 0x5C, b);
            }
        }
        let v = rmod(fx, t, 0x01E0_25D0, 0x50);
        fx.mem.setf(t + 0x38, ee::from_i32(v + 0x15E));
        let v = ee::div(fx.mem.f(t + 0x38), 10.0);
        fx.mem.setf(t + 0x38, v);
        fx.mem.write_u32(t + 0x30, 0x43AF_0000);
        let v = ee::div(fx.mem.f(t + 0x30), 10.0);
        fx.mem.setf(t + 0x30, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, 0x01E0_262C, 0xFFFF);
        fx.mem.set32(t + 0x24, v);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x67, 0x80);
    } else {
        fade_out(fx, t, 0x67, 7, 5);
    }
    for (kind, share) in [(0, 0x3ECC_CCCDu32), (1, 0x3F33_3333), (2, 0x3F33_3333)] {
        if fx.mem.i16(t + 0x50) == kind {
            let s = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x38));
            fx.mem.setf(t + 0x48, s);
            fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(share), s));
            if kind != 0 {
                let v = fx.mem.f(t + 0x38);
                add(fx, t + 0x34, v);
            }
        }
    }
    age(fx, t);
}

/// 0x1E02780 (D, task): b703's ribbon task in green (0x20, 0xC0, 0x20)
/// on nodes 0x28 / 0x29 / 0x3C / 0x3D: in by 0x3C under 0xF0 to update
/// 0x49, then out by 9 (the task ends under 0xB); to update 0x4A each
/// history takes its node's point.
fn trails(fx: &mut Fx, t: u32, sp: u32) {
    l2::rgb(fx, t + 0x60, 0x20, 0xC0, 0x20);
    let ras_fill = [0x01E0_27E8u32, 0x01E0_2854, 0x01E0_28C4, 0x01E0_2934];
    let ras_step = [0x01E0_2A18u32, 0x01E0_2A50, 0x01E0_2A88, 0x01E0_2AC0];
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        fx.mem.set16(t + 0xE, 0x1E);
        for k in 0..4u32 {
            let h = fx.self_handle();
            let slot = node(fx, k);
            crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ras_fill[k as usize]);
            for e in 0..0x13u32 {
                l2::copy3(fx, HIST[k as usize] + 0x10 * e, sp + 0x30);
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) < 0x4A {
        fade_in(fx, t, 0x63, 0xF0, 0x3C);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(COUNTER) < 0x4B {
        for k in 0..4u32 {
            let h = fx.self_handle();
            let slot = node(fx, k);
            crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ras_step[k as usize]);
            spawn::history(fx, HIST[k as usize] + 0x130, 0x14, sp + 0x30);
        }
    }
    fx.mem.set16(t + 6, 0xA);
    age(fx, t);
}

/// 0x1E02B10 (D, task): b703's arc with outer radius 200 (no growth), in
/// by 0x30 under 0xC0 to update 0x72, then out by 0xF.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4348_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xC0);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(COUNTER) < 0x73 {
        fade_in(fx, t, 0x67, 0xC0, 0x30);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2BE0);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E02C10 (D, task): b703's stage light (word for word).
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x97 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x1E {
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
}

/// 0x1E02D20 (D, task): b065's frame flash with its updates moved (0x84 /
/// 0x94 on update 0x44, the ST growth and the second fade-in from 0x42,
/// out from 0x87).
fn flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x88, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) == 0x44 {
        l2::rgb(fx, t + 0x60, 0x84, 0x94, 0x84);
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
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x50, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(512.0, s), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(512.0, s), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(COUNTER) >= 0x42 {
        let s = fx.mem.f(t + 0x48);
        if s < 4.0 {
            fx.mem.setf(t + 0x48, ee::add(s, f32::from_bits(0x3F19_999A)));
        }
    }
    if fx.mem.i16(COUNTER) < 0x87 {
        fade_in(fx, t, 0x63, 0x58, 0x16);
        if fx.mem.i16(COUNTER) >= 0x42 {
            fade_in(fx, t, 0x63, 0x60, 0xC);
        }
    } else {
        fade_out(fx, t, 0x63, 6, 4);
    }
    age(fx, t);
}
