//! BMODULE b732 (card 52), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E048E0, data area 0x1ED0000) [F]: b764's (card 458) family on a pool of 280 tasks of 0x70 bytes at
//! D+0x2A0 (alive +0x58, step +0x44 through `jalr`; spawn 0x1E05490 =
//! b764's 0x1E04D30 on 0x118 tasks with the colour D+0xE6B4, the angle
//! D+0xE702 into +0x5A and the index D+0xE70E into +0x50 / +0x52), no unit
//! flash. Drawn with quad rings (0x1E009B0, `billboard::ring_quads` on the
//! bytes D+0xE71A / D+0xE81A / D+0xE91A), sprite quads (0x1E00F60,
//! `quads4::sprite_quads`), camera-facing quads (0x1E012D0,
//! `billboard::quad`), arcs (0x1E01590, `billboard::arc_strips`), the
//! full-window tint (0x1E018F0, `quads4::tint4`) and trails as the
//! blended tube 0x1E01AC0 (`tubetrail::tube_with(SINGLE_BLEND)`) along
//! 20-point histories D+0x7D50 + 0x140 k of relative heads D+0xE150 + 0x10
//! k, drawn at the task's own point. The entry keeps D in s0 and counts the
//! drawn tasks in s1 (the steps save s0 = D, s1 = 0, s2 = the slot, s3 =
//! the slot index). Globals past D+0xE000: two drifting points D+0xE650 /
//! D+0xE660, the node offset D+0xE670, textures D+0xE690.., timer D+0xE6B0
//! = 325, OT D+0xE6BC, tint w D+0xE6CC, nodes D+0xE6E0.. (the attacker's
//! 0x33 / 0x34 / 0x42 / 0xB / 0x11, the target's 1 at D+0xE6EE), the angle
//! D+0xE702, the trail counter D+0xE706, the index D+0xE70E, the arc count
//! D+0xE712, the facing D+0xE714, the counter D+0xE718, bytes D+0xE71A
//! (rand % 40 - 20) / D+0xE81A / D+0xE91A (100..0xB3 cleared), sines
//! D+0xEA1C. The emitter 0x1E02610 (see [`emit`]). Returns 1 / 4 / 2 / 6 /
//! 8 on 0x82 / 0x12C / 0x11D / 0xFD..0x117 / timer out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::b590::project_residue;
use crate::b764::{age, copy4, fade_in, rand_mod};
use crate::blib::{billboard, quads4, spawn as bspawn, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0x118;
const ARCS: u32 = 0x01E0_2FC0;
const RING: u32 = 0x01E0_3290;
const FAN: u32 = 0x01E0_3430;
const TINT: u32 = 0x01E0_3550;
const MOTE: u32 = 0x01E0_3650;
const FAN_B: u32 = 0x01E0_38B0;
const GLOW: u32 = 0x01E0_39B0;
const SPRITE: u32 = 0x01E0_3BC0;
const TRAIL: u32 = 0x01E0_3CD0;
const RING_B: u32 = 0x01E0_4320;
const LIGHT: u32 = 0x01E0_45A0;
const OBJECT: u32 = 0x01E0_4660;
const WINDOW: u32 = 0x01E0_46E0;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_4C68;
const HEADS: u32 = D + 0xE150;
const HIST: u32 = D + 0x7D50;
const PT_A: u32 = D + 0xE650;
const PT_B: u32 = D + 0xE660;
const OFFSET: u32 = D + 0xE670;
const TEX: u32 = D + 0xE690;
const TIMER: u32 = D + 0xE6B0;
const COLOUR: u32 = D + 0xE6B4;
const OT_AT: u32 = D + 0xE6BC;
const TINT_W: u32 = D + 0xE6CC;
const NODES: u32 = D + 0xE6E0;
const ANGLE: u32 = D + 0xE702;
const TRAIL_K: u32 = D + 0xE706;
const INDEX: u32 = D + 0xE70E;
const ARC_K: u32 = D + 0xE712;
const FACING: u32 = D + 0xE714;
const COUNTER: u32 = D + 0xE718;
const BYTES: u32 = D + 0xE71A;
const BYTES_B: u32 = D + 0xE81A;
const BYTES_C: u32 = D + 0xE91A;
const SINES: u32 = D + 0xEA1C;

fn node(k: u32) -> i32 {
    // The node slots are read as i16 at D+0xE6E0 + 2k.
    k as i32
}

fn node_slot(fx: &mut Fx, k: u32) -> i32 {
    fx.mem.i16(NODES + 2 * node(k) as u32)
}

fn head_of(k: i32) -> u32 {
    (HEADS as i32 + 0x10 * k) as u32
}

fn hist_of(k: i32) -> u32 {
    (HIST as i32 + 0x140 * k) as u32
}

/// The entry 0x1E048E0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xEB90);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xE716, param);
    fx.mem.set32(TIMER, 0x145);
    for (k, n) in [0x33, 0x34, 0x42, 0xB, 0x11].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xE6EE, r);
    let tex = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, tex);
    let tex = fx.texture(4, 7);
    fx.mem.write_u32(D + 0x27C, tex);
    for k in 0..4u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(v, 0x01E0_4AC4);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    // (table, ra, m, add)
    for (table, ra, m, add) in [(BYTES, 0x01E0_4AF4u32, 0x28, -0x14), (BYTES_B, 0x01E0_4B44, 0x1E, -0xF), (BYTES_C, 0x01E0_4B94, 0x1E, -0xF)] {
        for k in 0..0x100u32 {
            let r = fx.rand_from(D, ra);
            let (_, h) = fx.div(r, m);
            fx.mem.write_u8(table + k, (h + add) as u8);
        }
    }
    for k in 100..0xB4u32 {
        fx.mem.write_u8(BYTES_C + k, 0);
    }
    fx.mem.set16(ARC_K, 1);
    0
}

/// The atlas cell `id` frame `sub` (0x106960 into `r`) at D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    copy4(fx, D + 0x94, r);
}

fn arcs(fx: &mut Fx, t: u32, n: i32, mode: i32) {
    fx.mem.set16(D + 0xB8, n);
    billboard::arc_strips(fx, D, t, mode, t + 0x10);
    fx.mem.set16(D + 0xB8, 0);
}

/// A quad ring of `n` (D+0xB8, layout 0): texture `tex`, cell 0x14 frame
/// +0x56 into `r`, on the bytes `table` (by +0x52 0 / 1).
fn ring(fx: &mut Fx, t: u32, r: u32, n: i32, tables: [u32; 2]) {
    let f = fx.mem.i16(t + 0x56);
    cell(fx, r, 0x14, f);
    fx.mem.set16(D + 0xB8, n);
    fx.mem.set16(D + 0xB0, 0);
    for (k, table) in tables.into_iter().enumerate() {
        if fx.mem.i16(t + 0x52) == k as i32 {
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, table);
        }
    }
    fx.mem.set16(D + 0xB8, 0);
    fx.mem.set16(D + 0xB0, 0);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 {
            continue;
        }
        run_task(fx, f, slot + 0x2A0, [u64::from(slot), u64::from(k)]);
    }
    let mut drawn: i32 = 0;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        drawn = (drawn + 1) as i16 as i32;
        let f = fx.mem.read_u32(t + 0x44);
        if f == ARCS {
            arcs(fx, t, 0x12, 0x21);
        }
        if f == RING {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            ring(fx, t, sp + 0x70, 2, [BYTES, BYTES_B]);
        }
        if f == FAN {
            arcs(fx, t, 0x24, 0x21);
        }
        if f == FAN_B {
            arcs(fx, t, 0x24, 0x21);
        }
        if f == TINT {
            for (k, ra) in [(0, 0x01E0_4E4Cu32), (1, 0x01E0_4EF4)] {
                if fx.mem.i16(t + 0x52) != k {
                    continue;
                }
                let h = if k == 0 { fx.self_handle() } else { fx.target_handle() };
                unit_position_saving(fx, h, sp + 0x60, u64::from(D), ra);
                let z = fx.mem.f(sp + 0x68);
                let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 60.0) } else { ee::sub(z, 60.0) };
                fx.mem.setf(sp + 0x68, z);
                let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
                let mut x = fx.mem.read_u64(sp + 0xE8);
                if let Some(w) = fx.project(p, None, &mut x) {
                    fx.mem.write_u64(sp + 0xE8, x);
                    fx.mem.setf(TINT_W, w);
                }
                project_residue(fx, sp, sp + 0x60);
                let w = fx.mem.f(TINT_W);
                quads4::tint4(fx, t, 0x21, w);
            }
        }
        if f == MOTE {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x14, c);
            let c = fx.mem.i16(t + 0x56);
            if c < 2 {
                fx.mem.set16(D + 0x94, (c << 6) + 1);
                fx.mem.set16(D + 0x96, 1);
            } else {
                fx.mem.set16(D + 0x94, ((c - 2) << 6) + 1);
                fx.mem.set16(D + 0x96, 0x40);
            }
            fx.mem.set16(D + 0x98, 0x3F);
            fx.mem.set16(D + 0x9A, 0x3F);
            let r = fx.rand_from(D, 0x01E0_5028);
            fx.mem.set16(D + 0xBE, r.wrapping_rem(8));
            for c in [0u32, 4, 8] {
                let w = fx.mem.read_u32(t + 0x10 + c);
                fx.mem.write_u32(sp + 0x60 + c, w);
            }
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
        }
        if f == GLOW {
            let w = fx.mem.read_u32(TEX + 0xC);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x90, 0x14, c);
            fx.mem.set16(D + 0xBE, 0);
            if fx.mem.i16(t + 0x50) == 1 {
                let r = fx.rand_from(D, 0x01E0_50E8);
                fx.mem.set16(D + 0xBE, (r >> 4).wrapping_rem(8));
            }
            for c in [0u32, 4, 8] {
                let w = fx.mem.read_u32(t + 0x10 + c);
                fx.mem.write_u32(sp + 0x60 + c, w);
            }
            if fx.mem.i16(t + 0x50) == 0 {
                billboard::quad(fx, uv::uv_rect, D, t, 0x10, sp + 0x60);
            }
            if fx.mem.i16(t + 0x50) == 1 {
                billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
            }
        }
        if f == SPRITE {
            let h = fx.self_handle();
            let s = node_slot(fx, 2);
            node_position_saving(fx, h, s, sp + 0x60, u64::from(D), Some(drawn as i64 as u64), 0x01E0_519C);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, sp + 0xA0, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.write_u32(sp + 0x6C, 0);
            quads4::sprite_quads(fx, D, t, 0, sp + 0x60);
            let h = fx.self_handle();
            let s = node_slot(fx, 2);
            node_position_saving(fx, h, s, sp + 0x60, u64::from(D), Some(drawn as i64 as u64), 0x01E0_5218);
            quads4::sprite_quads(fx, D, t, 0, sp + 0x60);
        }
        if f == TRAIL {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x3F)] {
                fx.mem.set16(D + o, v);
            }
            for c in [0u32, 4, 8] {
                let w = fx.mem.read_u32(t + 0x10 + c);
                fx.mem.write_u32(D + 0x20 + c, w);
            }
            fx.mem.set16(D + 0xBE, 1);
            let k = fx.mem.i16(t + 0x50);
            tubetrail::tube_with(fx, &tubetrail::SINGLE_BLEND, D, t, hist_of(k), 0x21);
        }
        if f == RING_B {
            if fx.mem.i16(t + 0x52) == 0 {
                let w = fx.mem.read_u32(TEX + 0xC);
                fx.mem.write_u32(D + 0x88, w);
            }
            if fx.mem.i16(t + 0x52) == 1 {
                let w = fx.mem.read_u32(TEX);
                fx.mem.write_u32(D + 0x88, w);
            }
            ring(fx, t, sp + 0xB0, 0xA, [BYTES_C, BYTES_B]);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x82 => 1,
        0x12C => 4,
        0x11D => 2,
        0xFD..=0x117 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// `regs`: the entry's s2 / s3 (the slot, its index) as a step saves or
/// passes them on.
fn run_task(fx: &mut Fx, f: u32, t: u32, regs: [u64; 2]) {
    match f {
        ARCS => arcs_step(fx, t, regs),
        RING => ring_step(fx, t),
        FAN => fan(fx, t),
        TINT => tint(fx, t),
        MOTE => mote(fx, t, regs),
        FAN_B => fan_b(fx, t),
        GLOW => glow(fx, t),
        SPRITE => sprite(fx, t),
        TRAIL => trail(fx, t),
        RING_B => ring_b(fx, t),
        LIGHT => light(fx, t),
        OBJECT => object(fx, t),
        WINDOW => window(fx, t),
        _ => panic!("b732: unknown task step {f:#x}"),
    }
}

/// 0x1E02610(D, 0) (frame 0x90; s1 = D, s0 the loop counter, s2 / s3 sp +
/// 0x64 / + 0x68): the update counter D+0xE718 and the spawns. Each update
/// the offset D+0xE670 is (the attacker's node 0x34 - node 0x33) turned 90
/// degrees about x, and (to 0x17F) D+0x250 node 0x34 + the offset; 0x3C
/// three motes 0x1E03650 at node 0x42 (angles 0 / 120 / 240); 0x50 an arc
/// fan 0x1E03430, two sprites 0x1E03BC0 (angles 90 / 0), a tint 0x1E03550
/// and the stage light 0x1E045A0; 0x5F..0xC8 a ring 0x1E03290 and a glow
/// 0x1E039B0 at node 0x42 a update; 0x80 the window 0x1E046E0; odd
/// 0x8E..0xBE up to 16 arc rows 0x1E02FC0; 0xD0 / 0xDF a ring 0x1E04320 at
/// node 0xB, 0xD6 / 0xF6 one at node 0x11; 0xD2 the object op 0x1E04660;
/// 0xF6..0x109 five trails 0x1E03CD0 a update at node 0x34; 0xFA two points
/// at node 0x34 (5 higher), an arc fan 0x1E038B0, a target tint and five
/// rings 0x1E04320 (index 1); 0xFA..0x11D a glow at each point a update,
/// the points moving 3 along z (by the facing) and 2.3 apart along x.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x90);
    let d = u64::from(D);
    fx.saved(sp, &[(0x40, 0x01E0_4C38), (0x20, param as i64 as u64), (0x10, 0), (0, d)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let h = fx.self_handle();
    let s = node_slot(fx, 0);
    node_position_saving(fx, h, s, sp + 0x50, d, Some(d), 0x01E0_2664);
    let mut s0 = u64::from(sp + 0x60);
    let h = fx.self_handle();
    let s = node_slot(fx, 1);
    node_position_saving(fx, h, s, sp + 0x60, s0, Some(d), 0x01E0_2688);
    for c in [0u32, 4, 8] {
        let v = ee::sub(fx.mem.f(sp + 0x60 + c), fx.mem.f(sp + 0x50 + c));
        fx.mem.setf(sp + 0x60 + c, v);
    }
    fx.mem.write_u32(sp + 0x80, 0x5A);
    l2::rotate_deg(fx, sp + 0x60, sp + 0x80, 1, SINES);
    for c in [0u32, 4, 8] {
        let w = fx.mem.read_u32(sp + 0x60 + c);
        fx.mem.write_u32(OFFSET + c, w);
    }
    let r = |s0: u64| [s0, d, u64::from(sp + 0x64), u64::from(sp + 0x68), 0];
    let n = fx.mem.i16(COUNTER);
    if n > 0 && n < 0x180 {
        let h = fx.self_handle();
        let s = node_slot(fx, 1);
        node_position_saving(fx, h, s, D + 0x250, 0, Some(d), 0x01E0_2758);
        for c in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(D + 0x250 + c), fx.mem.f(OFFSET + c));
            fx.mem.setf(D + 0x250 + c, v);
        }
        s0 = 1;
    }
    if fx.mem.i16(COUNTER) == 0xD2 {
        spawn(fx, OBJECT, r(s0), 0x01E0_27EC);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x8E..0xBF).contains(&n) && n.wrapping_rem(2) == 1 {
        let k = fx.mem.i16(ARC_K);
        if k < 0x11 {
            fx.mem.set16(INDEX, k);
            spawn(fx, ARCS, r(s0), 0x01E0_2868);
            let k = fx.mem.i16(ARC_K) + 1;
            fx.mem.set16(ARC_K, k);
        }
    }
    if fx.mem.i16(COUNTER) == 0x50 {
        fx.mem.set16(INDEX, 0);
        spawn(fx, FAN, r(s0), 0x01E0_28BC);
        fx.mem.set16(ANGLE, 0x5A);
        spawn(fx, SPRITE, r(s0), 0x01E0_28E0);
        fx.mem.set16(ANGLE, 0);
        spawn(fx, SPRITE, r(s0), 0x01E0_2900);
        spawn(fx, TINT, r(s0), 0x01E0_2914);
    }
    if fx.mem.i16(COUNTER) == 0x3C {
        let h = fx.self_handle();
        let s = node_slot(fx, 2);
        node_position_saving(fx, h, s, D + 0x250, s0, Some(d), 0x01E0_294C);
        for (a, ra) in [(0, 0x01E0_296Cu32), (0x78, 0x01E0_2990), (0xF0, 0x01E0_29B4)] {
            fx.mem.set16(ANGLE, a);
            spawn(fx, MOTE, r(s0), ra);
        }
    }
    let n = fx.mem.i16(COUNTER);
    if (0x5F..0xC9).contains(&n) {
        let h = fx.self_handle();
        let s = node_slot(fx, 2);
        node_position_saving(fx, h, s, D + 0x250, s0, Some(d), 0x01E0_29F8);
        fx.mem.set16(INDEX, 1);
        spawn(fx, RING, r(s0), 0x01E0_2A1C);
        spawn(fx, GLOW, r(s0), 0x01E0_2A30);
    }
    if fx.mem.i16(COUNTER) == 0x80 {
        spawn(fx, WINDOW, r(s0), 0x01E0_2A5C);
    }
    let n = fx.mem.i16(COUNTER);
    if (0xF6..0x10A).contains(&n) {
        let h = fx.self_handle();
        let s = node_slot(fx, 1);
        node_position_saving(fx, h, s, D + 0x250, s0, Some(d), 0x01E0_2AA0);
        for k in 0..5u64 {
            let i = fx.mem.i16(TRAIL_K);
            fx.mem.set16(INDEX, i);
            spawn(fx, TRAIL, r(k), 0x01E0_2AD4);
            let i = fx.mem.i16(TRAIL_K) + 1;
            fx.mem.set16(TRAIL_K, i);
            if fx.mem.i16(TRAIL_K) >= 0x50 {
                fx.mem.set16(TRAIL_K, 0);
            }
        }
        s0 = 5;
    }
    let n = fx.mem.i16(COUNTER);
    if (0xFA..0x11E).contains(&n) {
        if n == 0xFA {
            for (p, ra) in [(PT_A, 0x01E0_2B94u32), (PT_B, 0x01E0_2BF8)] {
                let h = fx.self_handle();
                let s = node_slot(fx, 1);
                node_position_saving(fx, h, s, p, s0, Some(d), ra);
                let y = ee::add(fx.mem.f(p + 4), 5.0);
                fx.mem.setf(p + 4, y);
            }
            fx.mem.set16(INDEX, 0);
            spawn(fx, FAN_B, r(s0), 0x01E0_2C58);
            fx.mem.set16(INDEX, 1);
            spawn(fx, TINT, r(s0), 0x01E0_2C7C);
            fx.mem.set16(INDEX, 1);
            let h = fx.self_handle();
            let s = node_slot(fx, 1);
            node_position_saving(fx, h, s, D + 0x250, s0, Some(d), 0x01E0_2CA8);
            for ra in [0x01E0_2CBCu32, 0x01E0_2CD0, 0x01E0_2CE4, 0x01E0_2CF8, 0x01E0_2D0C] {
                spawn(fx, RING_B, r(s0), ra);
            }
        }
        fx.mem.set16(INDEX, 0);
        for (p, ra) in [(PT_A, 0x01E0_2D54u32), (PT_B, 0x01E0_2D90)] {
            copy4(fx, D + 0x250, p);
            spawn(fx, GLOW, r(s0), ra);
        }
        for p in [PT_A, PT_B] {
            let z = fx.mem.f(p + 8);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 3.0) } else { ee::add(z, 3.0) };
            fx.mem.setf(p + 8, z);
        }
        let step = f32::from_bits(0x4013_3333);
        let x = ee::sub(fx.mem.f(PT_A), step);
        fx.mem.setf(PT_A, x);
        let x = ee::add(fx.mem.f(PT_B), step);
        fx.mem.setf(PT_B, x);
    }
    // (counters, node slot, node ra, spawn ra)
    for (at, k, nra, ra) in [([0xD0, 0xDF], 3u32, 0x01E0_2EECu32, 0x01E0_2F0Cu32), ([0xD6, 0xF6], 4, 0x01E0_2F50, 0x01E0_2F70)] {
        let n = fx.mem.i16(COUNTER);
        if !at.contains(&n) {
            continue;
        }
        let h = fx.self_handle();
        let s = node_slot(fx, k);
        node_position_saving(fx, h, s, D + 0x250, s0, Some(d), nra);
        fx.mem.set16(INDEX, 0);
        spawn(fx, RING_B, r(s0), ra);
    }
    if fx.mem.i16(COUNTER) == 0x50 {
        spawn(fx, LIGHT, r(s0), 0x01E0_2F9C);
    }
    fx.leave(0x90);
}

/// 0x1E05490(D, step, 0): b764's spawn on 280 tasks and this record's
/// globals; `regs` the caller's s0..s4.
fn spawn(fx: &mut Fx, f: u32, regs: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, regs[4]), (0x30, regs[3]), (0x20, regs[2]), (0x10, regs[1]), (0, regs[0])]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, gr, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(gr), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            // 0x107390's `sq ra` upper half (0) (b232's).
            fx.mem.write_u64(sp - 8, 0);
            for c in [0u32, 4, 8] {
                let w = fx.mem.read_u32(D + 0x250 + c);
                fx.mem.write_u32(t + 0x10 + c, w);
            }
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

/// A step's frame (`frame` bytes) with the entry's s0 = D, s1 = 0, s2 =
/// the slot (the first `n`) saved and ra at `ra_at`.
fn enter_step(fx: &mut Fx, frame: u32, n: u32, s2: u64, ra_at: u32) -> u32 {
    let sp = fx.enter(frame);
    let vals = [u64::from(D), 0, s2];
    let base = if frame == 0x70 { 0x10 } else { 0 };
    for k in 0..n {
        fx.saved(sp + base + 0x10 * k, &[(0, vals[k as usize])]);
    }
    fx.saved(sp, &[(ra_at, TASK_RA)]);
    sp
}

fn fade_down(fx: &mut Fx, t: u32, a: u32, down: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if v >= down.0 {
        fx.mem.write_u8(t + a, v.wrapping_sub(down.1));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn odd(v: i32) -> bool {
    v.wrapping_rem(2) == 1
}

/// 0x1E02FC0 (frame 0x70: f20, s0..s2): an arc row +0x50 (k) of 18
/// 10-degree arcs (inner radius 0, outer 25, dark grey over blue-grey),
/// k fifteenths of the way from the attacker's node 0x33 to node 0x34; its
/// outer alpha in by 0x18 below 0xC0 until update 0x122, then out by 0xC;
/// the outer colour (0x20, 0x20, 0x80) / (0x30, 0x30, 0xA0) on odd / even
/// updates; on every third age (1, 4, ..) a ring 0x1E03290 at its point
/// (until update 0x122); facing the camera turned 0x4000 about x.
fn arcs_step(fx: &mut Fx, t: u32, regs: [u64; 2]) {
    let sp = enter_step(fx, 0x70, 3, regs[0], 0x40);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    let d = u64::from(D);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0x10, 0x10, 0x10);
        l2::rgb(fx, t + 0x64, 0x60, 0x60, 0x80);
        fx.mem.set16(t + 6, 0x8C);
        fx.mem.write_u32(t + 0x4C, 0x41C8_0000);
        fx.mem.set16(t + 0x5A, 0x14);
        fx.mem.set16(t + 0x5C, 0x14);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(COUNTER) < 0x122 {
        fade_in(fx, t, 0x67, (0xC0, 0x18));
    } else {
        fade_down(fx, t, 0x67, (0xE, 0xC));
    }
    if odd(fx.mem.i16(COUNTER)) {
        l2::rgb(fx, t + 0x64, 0x20, 0x20, 0x80);
    } else {
        l2::rgb(fx, t + 0x64, 0x30, 0x30, 0xA0);
    }
    let f20 = ee::from_i32(fx.mem.i16(t + 0x50));
    let h = fx.self_handle();
    let s = node_slot(fx, 0);
    node_position_saving(fx, h, s, t + 0x10, u64::from(t), Some(d), 0x01E0_3118);
    let h = fx.self_handle();
    let s = node_slot(fx, 0);
    node_position_saving(fx, h, s, sp + 0x50, u64::from(t), Some(d), 0x01E0_3138);
    let h = fx.self_handle();
    let s = node_slot(fx, 1);
    node_position_saving(fx, h, s, sp + 0x60, u64::from(t), Some(d), 0x01E0_315C);
    for c in [0u32, 4, 8] {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x60 + c), fx.mem.f(sp + 0x50 + c)), 15.0);
        let v = ee::add(fx.mem.f(t + 0x10 + c), ee::mul(f20, v));
        fx.mem.setf(t + 0x10 + c, v);
    }
    let a = fx.mem.i16(t + 0x54);
    let (_, m) = fx.div(a, 3);
    if m == 1 {
        copy4(fx, D + 0x250, t + 0x10);
        fx.mem.set16(INDEX, 0);
        if fx.mem.i16(COUNTER) < 0x122 {
            spawn(fx, RING, [u64::from(t), d, u64::from(sp + 0x60), regs[1], 0], 0x01E0_3240);
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), d, 0x01E0_324C);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x70);
}

/// 0x1E03290: a quad ring (size 2, cell 3, pink-white; radius rand % 8 + 4
/// growing by 1, rows rand % 8 + 1, angle rand % 360), 0xC0 for rand % 3 +
/// 5 updates, then out by 0xB; light blue / grey by age.
fn ring_step(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0, 0x10);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4000_0000);
        fx.mem.set16(t + 0x56, 3);
        l2::rgb(fx, t + 0x60, 0xFF, 0x60, 0x60);
        let r = fx.rand_from(t, 0x01E0_32D8);
        fx.mem.set16(t + 0x50, r.wrapping_rem(8) + 4);
        let v = rand_mod(fx, t, 0x01E0_32F8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0xE, 1);
        let r = fx.rand_from(t, 0x01E0_331C);
        fx.mem.set16(t + 8, r.wrapping_rem(8) + 1);
        let v = rand_mod(fx, t, 0x01E0_333C, 3);
        fx.mem.set16(t + 6, v + 5);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade_down(fx, t, 0x63, (0xD, 0xB));
    }
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x60, 0xCF, 0xCF, 0xFF);
    } else {
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x80);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03430: an arc fan riding the attacker's node 0x42 (inner radius 0,
/// outer 20, grey over blue-grey, 10-degree arcs), its outer alpha in by
/// 0x60 below 0xC0 for 10 updates, then out by 0xC; facing the camera
/// turned 0x4000 about x.
fn fan(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2, 0, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x50, 0x50, 0x80);
        fx.mem.set16(t + 6, 0xA);
        fx.mem.write_u32(t + 0x4C, 0x41A0_0000);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x67, (0xC0, 0x60));
    } else {
        fade_down(fx, t, 0x67, (0xE, 0xC));
    }
    let h = fx.self_handle();
    let s = node_slot(fx, 2);
    node_position_saving(fx, h, s, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_3514);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_351C);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03550: b764's tint in (0x60, 0x60, 0x80) (both alphas in by 0x2A
/// below 0x80 for 12 updates, then out by 0xC).
fn tint(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0, 0x10);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x80);
        l2::rgb(fx, t + 0x64, 0x60, 0x60, 0x80);
        fx.mem.set16(t + 6, 0xC);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, (0x80, 0x2A));
        fade_in(fx, t, 0x67, (0x80, 0x2A));
    } else {
        fade_down(fx, t, 0x63, (0xE, 0xC));
        let v = fx.mem.read_u8(t + 0x67);
        fx.mem.write_u8(t + 0x67, if v >= 0xE { v - 0xC } else { 0 });
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03650: a mote (8 x 8, cell rand % 4 cycling, +0x34 0.5) at the
/// attacker's node 0x42 pushed +0x48 (14, slowing by 0.7) along its angle
/// +0x5A (turning 20 a update) in x / y, each update a ring 0x1E03290 at
/// its point; alpha in by 0x30 below 0xC0 for 20 updates, then out by 9;
/// white / grey by age; facing the camera.
fn mote(fx: &mut Fx, t: u32, regs: [u64; 2]) {
    enter_step(fx, 0x30, 2, 0, 0x20);
    let d = u64::from(D);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 8);
        fx.mem.set16(t + 0xA, 8);
        fx.mem.write_u32(t + 0x48, 0x4160_0000);
        let r = fx.rand_from(t, 0x01E0_368C);
        fx.mem.set16(t + 0x56, r.wrapping_rem(4));
        fx.mem.write_u32(t + 0x34, 0x3F00_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        fade_in(fx, t, 0x63, (0xC0, 0x30));
    } else {
        fade_down(fx, t, 0x63, (0xB, 9));
    }
    let c = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, c);
    if fx.mem.i16(t + 0x56) >= 4 {
        fx.mem.set16(t + 0x56, 0);
    }
    let g = if odd(fx.mem.i16(t + 0x54)) { 0xFF } else { 0x80 };
    l2::rgb(fx, t + 0x60, g, g, g);
    let h = fx.self_handle();
    let s = node_slot(fx, 2);
    node_position_saving(fx, h, s, t + 0x10, u64::from(t), Some(d), 0x01E0_37A8);
    for (o, cos) in [(0u32, 1), (4, 0)] {
        let a = fx.mem.i16(t + 0x5A);
        let v = l2::sin_deg(fx, SINES, a, cos);
        let m = ee::mul(fx.mem.f(t + 0x48), v);
        let x = ee::add(fx.mem.f(t + 0x10 + o), m);
        fx.mem.setf(t + 0x10 + o, x);
    }
    let s = fx.mem.f(t + 0x48);
    if !crate::exe::ee_le(s, 0.0) {
        fx.mem.setf(t + 0x48, ee::sub(s, f32::from_bits(0x3F33_3333)));
    }
    copy4(fx, D + 0x250, t + 0x10);
    fx.mem.set16(INDEX, 0);
    spawn(fx, RING, [u64::from(t), d, regs[0], regs[1], 0], 0x01E0_386C);
    let a = fx.mem.i16(t + 0x5A) + 0x14;
    fx.mem.set16(t + 0x5A, a);
    camera_angles_saving(fx, t + 0x20, u64::from(t), d, 0x01E0_3880);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E038B0: an arc fan at its spawn point (inner radius 0, outer 200,
/// grey over light blue, 10-degree arcs), its outer alpha in by 0x60 below
/// 0xC0 for 12 updates, then out by 0xC; facing the camera turned 0x4000.
fn fan_b(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0, 0x10);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0xC0);
        fx.mem.set16(t + 6, 0xC);
        fx.mem.write_u32(t + 0x4C, 0x4348_0000);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x67, (0xC0, 0x60));
    } else {
        fade_down(fx, t, 0x67, (0xE, 0xC));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_397C);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E039B0: a blue glow at its spawn point (+0x50 0: 0x8C x 0x8C, in by
/// 0x30 below 0xC0 for 20 updates, rising 0.1 and growing 3 in height a
/// update; 1: 10 x 10, in by 0x20 below 0x80 for 10, rising 0.1), cell rand
/// % 4, +0x34 0.5, then out by 0xD; facing the camera.
fn glow(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0, 0x10);
    if fx.mem.i16(t + 0x58) == 1 {
        // (kind, size, hold, rand ra)
        for (k, size, hold, ra) in [(0, 0x8C, 0x14, 0x01E0_39F0u32), (1, 0xA, 0xA, 0x01E0_3A54)] {
            if fx.mem.i16(t + 0x50) != k {
                continue;
            }
            fx.mem.set16(t + 8, size);
            fx.mem.set16(t + 0xA, size);
            fx.mem.write_u32(t + 0x48, 0x4160_0000);
            let r = fx.rand_from(t, ra);
            fx.mem.set16(t + 0x56, r.wrapping_rem(4));
            fx.mem.write_u32(t + 0x34, 0x3F00_0000);
            fx.mem.set16(t + 6, hold);
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if fx.mem.i16(t + 0x50) == 0 {
            fade_in(fx, t, 0x63, (0xC0, 0x30));
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fade_in(fx, t, 0x63, (0x80, 0x20));
        }
    } else {
        fade_down(fx, t, 0x63, (0xF, 0xD));
    }
    let rise = f32::from_bits(0x3DCC_CCCD);
    if fx.mem.i16(t + 0x50) == 0 && fx.mem.i16(t + 0x54) != 0 {
        let y = ee::add(fx.mem.f(t + 0x14), rise);
        fx.mem.setf(t + 0x14, y);
        let h = fx.mem.i16(t + 0xA) + 3;
        fx.mem.set16(t + 0xA, h);
    }
    if fx.mem.i16(t + 0x50) == 1 && fx.mem.i16(t + 0x54) != 0 {
        let y = ee::add(fx.mem.f(t + 0x14), rise);
        fx.mem.setf(t + 0x14, y);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_3BA0);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03BC0: a red-rimmed sprite pair (6 wide, 2.5 x 50 growing 4 in
/// height, grey over red) at the attacker's node 0x42 turning 6 a update,
/// alpha in by 0x40 below 0x80 for 4 updates, then out by 9.
fn sprite(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0, 0x10);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x10, 0x10);
        fx.mem.write_u32(t + 0x30, 0x4020_0000);
        fx.mem.write_u32(t + 0x34, 0x4248_0000);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, (0x80, 0x40));
    } else {
        fade_down(fx, t, 0x63, (0xB, 9));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let h = ee::add(fx.mem.f(t + 0x34), 4.0);
        fx.mem.setf(t + 0x34, h);
    }
    age(fx, t);
    let a = fx.mem.i16(t + 0x5A) + 6;
    fx.mem.set16(t + 0x5A, a);
    fx.leave(0x20);
}

/// 0x1E03CD0: b764's trail B on this record's heads / histories (+0x52 =
/// +0x50 % 6; the first jitter rand % 60 - 30 over 10; headings (rand >> 4)
/// % 360, +0x5A drawn twice; half width rand % 100 + 20 tenths, step rand %
/// 55 + 10 tenths; per update jitter rand % 60 - 30 over 10).
fn trail(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2, 0, 0x20);
    let headk = |fx: &mut Fx| head_of(fx.mem.i16(t + 0x50));
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 6, 1);
        fx.mem.write_u8(t + 0x63, 0);
        let k = fx.mem.i16(t + 0x50);
        let (_, m) = fx.div(k, 6);
        fx.mem.set16(t + 0x52, m);
        for o in [0u32, 4, 8] {
            let h = headk(fx);
            fx.mem.write_u32(h + o, 0);
        }
        for (o, ra) in [(0x30u32, 0x01E0_3D64u32), (0x34, 0x01E0_3D90), (0x38, 0x01E0_3DBC)] {
            let r = rand_mod(fx, t, ra, 0x3C);
            fx.mem.setf(t + o, ee::from_i32(r - 0x1E));
        }
        for o in [0u32, 4, 8] {
            let j = ee::div(fx.mem.f(t + 0x30 + o), 10.0);
            let h = headk(fx);
            let v = ee::add(fx.mem.f(h + o), j);
            fx.mem.setf(h + o, v);
        }
        let r = fx.rand_from(t, 0x01E0_3E78);
        let (_, a) = fx.div(r >> 4, 0x168);
        fx.mem.set16(t + 0x5A, a);
        let k = fx.mem.i16(t + 0x50);
        let (tr, h) = (hist_of(k), head_of(k));
        for j in 0..0x14u32 {
            for o in [0u32, 4, 8] {
                let w = fx.mem.read_u32(h + o);
                fx.mem.write_u32(tr + 0x10 * j + o, w);
            }
        }
        let r = rand_mod(fx, t, 0x01E0_3F04, 0x64);
        fx.mem.setf(t + 0x48, ee::from_i32(r + 0x14));
        let v = ee::div(fx.mem.f(t + 0x48), 10.0);
        fx.mem.setf(t + 0x48, v);
        let r = rand_mod(fx, t, 0x01E0_3F44, 0x37);
        fx.mem.setf(t + 0x4C, ee::from_i32(r + 0xA));
        let v = ee::div(fx.mem.f(t + 0x4C), 10.0);
        fx.mem.setf(t + 0x4C, v);
        for (o, ra) in [(0x5Au32, 0x01E0_3F84u32), (0x5C, 0x01E0_3FA4)] {
            let r = fx.rand_from(t, ra);
            let (_, a) = fx.div(r >> 4, 0x168);
            fx.mem.set16(t + o, a);
        }
        let r = rand_mod(fx, t, 0x01E0_3FC4, 0xF);
        fx.mem.set16(t + 8, r + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let n = fx.mem.i16(t + 6);
    if fx.mem.i16(t + 0x54) < 7 {
        if n < 0xF {
            fx.mem.set16(t + 6, n + 1);
        }
    } else if n > 0 {
        fx.mem.set16(t + 6, n - 1);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 8) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade_down(fx, t, 0x63, (0x1F, 0x1D));
    }
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) != 0 { 0x80 } else { 0xFF };
    l2::rgb(fx, t + 0x60, g, g, g);
    for (o, ra) in [(0x30u32, 0x01E0_40C0u32), (0x34, 0x01E0_40EC), (0x38, 0x01E0_4118)] {
        let r = rand_mod(fx, t, ra, 0x3C);
        fx.mem.setf(t + o, ee::from_i32(r - 0x1E));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        // (column, heading, cos)
        for (o, a, cos) in [(8u32, 0x5Au32, 0), (0, 0x5A, 1), (4, 0x5C, 0)] {
            let a = fx.mem.i16(t + a);
            let s = l2::sin_deg(fx, SINES, a, cos);
            let m = ee::mul(fx.mem.f(t + 0x4C), s);
            let h = headk(fx);
            let v = ee::add(fx.mem.f(h + o), m);
            fx.mem.setf(h + o, v);
        }
        for o in [0u32, 4, 8] {
            let j = ee::div(fx.mem.f(t + 0x30 + o), 10.0);
            let h = headk(fx);
            let v = ee::add(fx.mem.f(h + o), j);
            fx.mem.setf(h + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 7 {
        let k = fx.mem.i16(t + 0x50);
        bspawn::history(fx, hist_of(k) + 0x130, 0x14, head_of(k));
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E04320: a white quad ring (+0x50 0: size 50, rows rand % 8 + 1,
/// radius rand % 8 + 4 growing 20, 0xC0 for rand % 3 + 5 updates; 1: size
/// 10, growing 10, for rand % 15 + 30), cell 3, angle rand % 360, then out
/// by 0xB; index 1 white / grey by age.
fn ring_b(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0, 0x10);
    if fx.mem.i16(t + 0x58) == 1 {
        // (kind, size, growth, hold m, hold add, rand ras)
        let kinds = [
            (0, 0x4248_0000u32, 0x14, 3, 5, [0x01E0_4374u32, 0x01E0_4394, 0x01E0_43B8, 0x01E0_43D8]),
            (1, 0x4120_0000, 0xA, 0xF, 0x1E, [0x01E0_4430, 0x01E0_4450, 0x01E0_4474, 0x01E0_4494]),
        ];
        for (k, size, grow, hm, ha, ra) in kinds {
            if fx.mem.i16(t + 0x50) != k {
                continue;
            }
            fx.mem.write_u32(t + 0x48, size);
            fx.mem.set16(t + 0x56, 3);
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
            let r = fx.rand_from(t, ra[0]);
            fx.mem.set16(t + 0x50, r.wrapping_rem(8) + 4);
            let v = rand_mod(fx, t, ra[1], 0x168);
            fx.mem.set16(t + 0x5A, v);
            fx.mem.set16(t + 0xE, grow);
            let r = fx.rand_from(t, ra[2]);
            fx.mem.set16(t + 8, r.wrapping_rem(8) + 1);
            let v = rand_mod(fx, t, ra[3], hm);
            fx.mem.set16(t + 6, v + ha);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade_down(fx, t, 0x63, (0xD, 0xB));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
    if fx.mem.i16(t + 0x52) == 1 {
        let g = if odd(fx.mem.i16(t + 0x54)) { 0xFF } else { 0x80 };
        l2::rgb(fx, t + 0x60, g, g, g);
    }
    fx.leave(0x20);
}

/// 0x1E045A0: the stage light +0x50 from 0x80 down by 10 to 0x50, then
/// (timer below 0x15) back up by 10 to 0x80, the task ending there.
fn light(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x10);
    fx.saved(sp, &[(0, TASK_RA)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let v = fx.mem.i16(t + 0x50);
    if fx.mem.i32(TIMER) >= 0x15 {
        fx.mem.set16(t + 0x50, if v < 0x51 { 0x50 } else { v - 0xA });
    } else {
        if v < 0x80 {
            fx.mem.set16(t + 0x50, v + 0xA);
        } else {
            fx.mem.set16(t + 0x50, 0x80);
            fx.mem.set16(t + 0x58, 0);
        }
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    let l = fx.mem.read_u8(t + 0x50);
    fx.set_stage_light(i32::from(l));
    fx.leave(0x10);
}

/// 0x1E04660 (frame 0x30: s0, ra +0x10): the object op 0x1F8380(2, 0, the
/// record's words 0x1E05680 / 0x1E05688 at sp + 0x28 / + 0x2C, 0) each
/// update; the task ends from age 0x2D.
fn object(fx: &mut Fx, t: u32) {
    let sp = enter_step(fx, 0x30, 1, 0, 0x10);
    for (to, from) in [(0x28u32, 0x01E0_5680u32), (0x2C, 0x01E0_5688)] {
        let w = fx.mem.read_u32(from);
        fx.mem.write_u32(sp + to, w);
    }
    fx.boundary(0x001F_8380, "object op 0x1F8380", [2, 0, sp + 0x28, sp + 0x2C]);
    age(fx, t);
    if fx.mem.i16(t + 0x54) >= 0x2D {
        fx.mem.set16(t + 0x58, 0);
    }
    fx.leave(0x30);
}

/// 0x1E046E0 (frame 0x90): b020's window sprite (colour (0x90, 0x78,
/// 0x78), ALPHA 0x44, key 0), in by 5 below 0x50 while the timer is at
/// least 0x1A, then out by 7.
fn window(fx: &mut Fx, t: u32) {
    let sp = enter_step(fx, 0x90, 2, 0, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
        fx.mem.set16(t + 0x58, 2);
        l2::rgb(fx, t + 0x60, 0x90, 0x78, 0x78);
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
    for o in [0x50u32, 0x54] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(s, 512.0));
    }
    for o in [0x60u32, 0x64] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(ee::sub(512.0, s), 512.0));
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i32(TIMER) >= 0x1A {
        if a < 0x50 {
            fx.mem.write_u8(t + 0x63, a + 5);
        }
    } else if a >= 9 {
        fx.mem.write_u8(t + 0x63, a - 7);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
    fx.leave(0x90);
}
