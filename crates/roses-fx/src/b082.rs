//! BMODULE b082 (card 298), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02860, data area 0x1ED0000) [F]: b547's family (a pool of 150 tasks of 0x70 bytes at D+0x2A0,
//! alive +0x58, step +0x44 called through `jalr`) on a D+0x4Cxx layout with
//! its own emitter and steps, drawn with flat facing rings turned about z
//! (2f5a28a746a2 at 0x1E00D80, `quads4::ring_facing_flat_z`) and a
//! twice-submitted turned quad (fc6290cb1a17 at 0x1E03130,
//! `billboard::quad_b_turned2`), UVs by uv_rect 0x1E00080. Each update a
//! point on the attacker's node 1 jittered by +-10: below update 171 a
//! ring starts there (0x1E02400); on updates 50..100 two columns a update
//! (0x1E01A50: a quad drifting from the point along a random direction by
//! a growing step, its history row D+0x4470 + 16 slot falling and moving to
//! the facing from age 64; at age 100 three 0x1E01EC0 and three 0x1E02250
//! rings at its end). The steps 0x1E02050 / 0x1E025B0 (and their draws)
//! are never spawned and not ported. Returns 1 on update 130, 4 on 220, 2
//! on 215, 6 on 141..209, 8 once the 250-update timer +0x4C10 has run out.
use crate::b615::{cell, ring, rmod, spawn_at};
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const COLUMN: u32 = 0x01E0_1A50;
const RING: u32 = 0x01E0_1EC0;
const RING2: u32 = 0x01E0_2250;
const RING3: u32 = 0x01E0_2400;
/// The return address of the entry's `jalr` into a step (0x1E02B00 + 8).
const TASK_RA: u64 = 0x01E0_2B08;
/// The column's position history (16 bytes a spawn slot).
const HISTORY: u32 = D + 0x4470;
const TEX: u32 = D + 0x4BF0;
const TIMER: u32 = D + 0x4C10;
const COLOUR: u32 = D + 0x4C14;
const NODES: u32 = D + 0x4C3C;
const TARGET_NODE: u32 = D + 0x4C4A;
const SLOT: u32 = D + 0x4C6A;
const NEXT: u32 = D + 0x4C6C;
const FACING: u32 = D + 0x4C6E;
const COUNTER: u32 = D + 0x4C72;
const BYTES: u32 = D + 0x4C74;
const BYTES2: u32 = D + 0x4D74;
const SINES: u32 = D + 0xD8;

/// The entry 0x1E02860.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0x4C1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4FE0);
    l2::sine_table(fx, D + 0x4E74);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4C70, param);
    fx.mem.set32(TIMER, 0xFA);
    for (k, node) in [1, 0x1C, 0xD].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x268, t);
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
    // The entry's s0 is D here (rand's frame).
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_29EC);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_2A3C);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_2A8C);
        let (_, h) = fx.div(v, 0x28);
        fx.mem.set8(BYTES2 + k, h - 0x14);
    }
    0
}

/// D+0x94..+0x9A: the cell (sub << 6) + 1, 1, 0x40 x 0x52.
fn strip_cell(fx: &mut Fx, sub: i32) {
    fx.mem.set16(D + 0x94, (sub << 6) + 1);
    fx.mem.set16(D + 0x96, 1);
    fx.mem.set16(D + 0x98, 0x40);
    fx.mem.set16(D + 0x9A, 0x52);
}

/// The facing ring of a ring task with texture `tex` and `rows` rows.
fn ring_draw(fx: &mut Fx, t: u32, tex: u32, rows: i32) {
    let w = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, w);
    fx.mem.set16(D + 0xB8, rows);
    let o = fx.mem.i16(t + 0xA);
    fx.mem.set16(D + 0xC0, o);
    for k in 0..3 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(D + 0x20 + 4 * k, w);
    }
    quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, BYTES);
    fx.mem.set16(D + 0xB8, 0);
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
            COLUMN => column(fx, t),
            RING => ring(fx, t, RING, TASK_RA, 0x41A0_0000),
            RING2 => ring_v(fx, t, RING2, TASK_RA, 0x428C_0000, 0x80, (0x14, 0xA), (0xF, 9), (0xA, 8)),
            RING3 => ring_v(fx, t, RING3, TASK_RA, 0x4302_0000, 0xC0, (0xA, 5), (0xF, 9), (0xB, 9)),
            _ => panic!("b082: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == COLUMN {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            let o = fx.mem.f(t + 0x3C);
            fx.fix_residue(o, 0x01E0_2B70);
            fx.mem.set16(D + 0xBE, ee::fixsfsi(o));
            let sub = fx.mem.i16(t + 0x56);
            strip_cell(fx, sub);
            let n = ee::from_i32(fx.mem.i16(t + 0x52));
            let h = HISTORY + 16 * fx.mem.i16(t + 0x50) as u32;
            let x = ee::add(fx.mem.f(h), ee::mul(fx.mem.f(t + 0x30), n));
            fx.mem.setf(sp + 0x60, x);
            let y = ee::add(fx.mem.f(h + 4), ee::mul(fx.mem.f(t + 0x34), n));
            let a = fx.mem.i16(t + 0x5C);
            let s = l2::sin_deg(fx, SINES, a, 1);
            fx.mem.setf(sp + 0x64, ee::add(y, ee::mul(4.0, s)));
            let h = HISTORY + 16 * fx.mem.i16(t + 0x50) as u32;
            let z = ee::add(fx.mem.f(h + 8), ee::mul(fx.mem.f(t + 0x38), n));
            fx.mem.setf(sp + 0x68, z);
            billboard::quad_b_turned2(fx, uv::uv_rect, D, t, 0x10, sp + 0x60);
        }
        if fx.mem.read_u32(t + 0x44) == RING {
            let sub = fx.mem.i16(t + 0x56);
            strip_cell(fx, sub);
            ring_draw(fx, t, TEX + 4, 7);
        }
        if fx.mem.read_u32(t + 0x44) == RING2 {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x14, sub);
            ring_draw(fx, t, TEX + 8, 7);
        }
        if fx.mem.read_u32(t + 0x44) == RING3 {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x90, 0x14, sub);
            ring_draw(fx, t, TEX + 8, 5);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x82 => 1,
        0xDC => 4,
        0xD7 => 2,
        0x8D..=0xD1 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E01880(D, 0): the update counter, the jittered point and the
/// emitter (frame 0x40: ra, the entry's s2 = the parameter, s1 = 0, s0 =
/// D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_2AD8), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    fx.node_position(h, slot, D + 0x250);
    for (o, ra) in [(0x250u32, 0x01E0_18C0u32), (0x254, 0x01E0_18F4), (0x258, 0x01E0_1928)] {
        let v = fx.rand_from(D, ra);
        let (_, m) = fx.div(v, 0x14);
        let c = ee::add(fx.mem.f(D + o), ee::from_i32(m - 0xA));
        fx.mem.setf(D + o, c);
    }
    if fx.mem.i16(COUNTER) < 0xAB {
        spawn_at(fx, RING3, COLOUR, SLOT, &[(0x50, 0x01E0_197C), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, 0), (0, u64::from(D))]);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x32..0x65).contains(&c) {
        for k in 0..2u64 {
            let s = fx.mem.i16(NEXT);
            fx.mem.set16(SLOT, s);
            spawn_at(fx, COLUMN, COLOUR, SLOT, &[(0x50, 0x01E0_19E4), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, 2), (0, k)]);
            let s = fx.mem.i16(NEXT) + 1;
            fx.mem.set16(NEXT, s);
            if fx.mem.i16(NEXT) >= 0x78 {
                fx.mem.set16(NEXT, 0);
            }
        }
    }
    fx.leave(0x40);
}

/// 0x1E01A50 (D, task): a column (frame 0x40: ra, the entry's s1 = 0, s0
/// = D, f21 / f20): quad size 10..59 x +15, orientation +0xE 0..7, cell row
/// 0..2, white, its start kept in the history row of its slot, a direction
/// (rand % 50 - 25) / 15 and a random angle +0x5A; in by 0x2A under 0x80
/// for 95 updates, then out by 14 twice an update; its step +0x52 grows
/// for 20 updates; +0x5C by 4. At age 100 three RING and three RING2 tasks
/// start at its end (the history + the direction x +0x52, y + 4 sin
/// +0x5C). From age 64 +0x5A turns by 4 and the history row moves 5 to the
/// facing and falls by 0.5.
fn column(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, TASK_RA), (0x20, 0), (0x10, u64::from(D))]);
    let f = COLUMN;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x2C, 0x32);
        fx.mem.set16(t + 8, v + 0xA);
        let w = fx.mem.i16(t + 8) + 0xF;
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, f + 0x60);
        fx.mem.set16(t + 0xE, v % 8);
        let v = rmod(fx, t, f + 0x74, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        let h = HISTORY + 16 * fx.mem.i16(t + 0x50) as u32;
        for k in 0..3 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(h + 4 * k, w);
        }
        for (o, site) in [(0x30u32, 0xECu32), (0x34, 0x118), (0x38, 0x144)] {
            let v = rmod(fx, t, f + site, 0x32);
            fx.mem.setf(t + o, ee::from_i32(v - 0x19));
        }
        for o in [0x30u32, 0x34, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 15.0);
            fx.mem.setf(t + o, q);
        }
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, f + 0x1A4, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x5F {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x10 {
            fx.mem.set16(t + 0x58, 0);
        } else {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        }
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        }
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        let n = fx.mem.i16(t + 0x52) + 1;
        fx.mem.set16(t + 0x52, n);
    }
    let a = fx.mem.i16(t + 0x5C) + 4;
    fx.mem.set16(t + 0x5C, a);
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), f + 0x274);
    if fx.mem.i16(t + 0x54) == 0x64 {
        let n = ee::from_i32(fx.mem.i16(t + 0x52));
        let h = HISTORY + 16 * fx.mem.i16(t + 0x50) as u32;
        let x = ee::add(fx.mem.f(h), ee::mul(fx.mem.f(t + 0x30), n));
        fx.mem.setf(D + 0x250, x);
        let h = HISTORY + 16 * fx.mem.i16(t + 0x50) as u32;
        let y = ee::add(fx.mem.f(h + 4), ee::mul(fx.mem.f(t + 0x34), n));
        let a = fx.mem.i16(t + 0x5C);
        let s = l2::sin_deg(fx, SINES, a, 1);
        fx.mem.setf(D + 0x254, ee::add(y, ee::mul(4.0, s)));
        let h = HISTORY + 16 * fx.mem.i16(t + 0x50) as u32;
        let z = ee::add(fx.mem.f(h + 8), ee::mul(fx.mem.f(t + 0x38), n));
        fx.mem.setf(D + 0x258, z);
        let k = (t - D - 0x2A0) / 0x70;
        // The spawn saves the entry's s3 = k / s2 = the task's base and the column's s1 = D / s0 = t.
        let saves = |ra: u32| [(0x50, u64::from(ra)), (0x40, 0), (0x30, u64::from(k)), (0x20, u64::from(t - 0x2A0)), (0x10, u64::from(D)), (0, u64::from(t))];
        for ra in [0x01E0_1D74u32, 0x01E0_1D88, 0x01E0_1D9C] {
            spawn_at(fx, RING, COLOUR, SLOT, &saves(ra));
        }
        for ra in [0x01E0_1DB0u32, 0x01E0_1DC4, 0x01E0_1DD8] {
            spawn_at(fx, RING2, COLOUR, SLOT, &saves(ra));
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if fx.mem.i16(t + 0x54) >= 0x40 {
        let a = fx.mem.i16(t + 0x5A) + 4;
        fx.mem.set16(t + 0x5A, a);
        let a = fx.mem.i16(t + 0x5A);
        if a >= 0x168 {
            let (_, m) = fx.div(a, 0x168);
            fx.mem.set16(t + 0x5A, m);
        }
        let h = HISTORY + 16 * fx.mem.i16(t + 0x50) as u32;
        let z = fx.mem.f(h + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(h + 8, z);
        let y = ee::sub(fx.mem.f(h + 4), 0.5);
        fx.mem.setf(h + 4, y);
    }
    fx.leave(0x40);
}

/// 0x1E02250 / 0x1E02400 (D, task) at `f` (also b206's 0x1E019D0): b615's
/// ring (frame 0x20: `ra`,
/// the entry's s0 = D) with its own size, grey, radius rand % m + a,
/// growth rand % m + a, fade (below `fade.0` gone, else less `fade.1`),
/// and the size growing by 2 a update.
#[allow(clippy::too_many_arguments)]
pub(crate) fn ring_v(fx: &mut Fx, t: u32, f: u32, ra: u64, size: u32, grey: i32, radius: (i32, i32), grow: (i32, i32), fade: (u8, u8)) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, ra), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, size);
        let v = rmod(fx, t, f + 0x20, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, grey, grey, grey);
        let v = rmod(fx, t, f + 0x4C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, f + 0x70);
        fx.mem.set16(t + 0xA, v % 8);
        let w = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, w);
        let v = rmod(fx, t, f + 0x8C, radius.0);
        fx.mem.set16(t + 0x50, v + radius.1);
        let v = rmod(fx, t, f + 0xAC, grow.0);
        fx.mem.set16(t + 0xE, v + grow.1);
        let v = rmod(fx, t, f + 0xCC, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xE8, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= fade.0 {
            fx.mem.write_u8(t + 0x63, a - fade.1);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let s = ee::add(fx.mem.f(t + 0x48), 2.0);
    fx.mem.setf(t + 0x48, s);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}
