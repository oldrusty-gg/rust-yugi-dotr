//! BMODULE b632 (card 391), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03BA0, data area 0x1ED0000, 0x5FA0 bytes) [F]
//!: b426's family (card 3, `b426.rs`) - a pool of
//! 200 tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step +0x44 called
//! through `jalr`; the flash tasks 0x1E038F0 counted, not called, and
//! stepped by the unit flash 0x1E046D0 after the draws), spawned by
//! 0x1E04590 (b426's 0x1E04630 with the colour D+0x5BB4 and the kind
//! D+0x5C26), drawn with `quads4::ring_facing_flat` (0x1E00920),
//! `quads4::ring_quads_z` (0x1E00E70), `billboard::arc_strips_flat`
//! (0x1E01420), `billboard::arc_quads` (0x1E01780) and `billboard::quad`
//! (0x1E01BD0), UVs by uv_rect 0x1E00080.
//!
//! The emitter 0x1E01E90 counts D+0x5C30: on 0x73 / 0x82 / 0x91 / 0xA0 /
//! 0xAF / 0xBE an orb at the attacker's node 3 (D+0x5A50 + 16 n) with an
//! orb step 0x1E02750, a white quad 0x1E03770 and three grey rings
//! 0x1E023C0; on its first update each orb aims at the target's node 1
//! (jittered; y / z only: z computed twice, x never - b426's quirk) and
//! moves a 60th of the way a update while younger than 60; 0x3A..0x9E a
//! red band 0x1E032E0 a update, below 0x64 also a closing ring 0x1E03100
//! and an arc 0x1E02F70 at node 3; on 0x41 the unit flash 0x1E038F0. The
//! orb (a red band radius 40 growing to 100) spawns a spark 0x1E03450 and
//! a ring 0x1E035F0 a update before 61, wide bands 0x1E02E00 on 60..64 and
//! on 60 ten rings 0x1E02230 and four arcs 0x1E02AE0 (a quarter turn
//! apart by D+0x5C2A). Returns 1 on 0xA1, 4 on 0x116, 2 on 0x10C, 6 on
//! 0xAC..0x106, 8 once the 0x122-update timer D+0x5BB0 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
const RING_A: u32 = 0x01E0_2230;
const RING_B: u32 = 0x01E0_23C0;
const BURST: u32 = 0x01E0_2550;
const ORB: u32 = 0x01E0_2750;
const ARC: u32 = 0x01E0_2AE0;
const WIDE: u32 = 0x01E0_2E00;
const ARC_B: u32 = 0x01E0_2F70;
const CLOSE: u32 = 0x01E0_3100;
const BAND: u32 = 0x01E0_32E0;
const SPARK: u32 = 0x01E0_3450;
const RING_C: u32 = 0x01E0_35F0;
const WHITE: u32 = 0x01E0_3770;
const FLASH: u32 = 0x01E0_38F0;
/// The return address of the entry's `jalr` into a step (0x1E03E88 + 8).
const TASK_RA: u64 = 0x01E0_3E90;
const POS: u32 = D + 0x250;
/// The orbs' points (16 bytes each) and their steps (+0xA0).
const ORBS: u32 = D + 0x5A50;
const TEX: u32 = D + 0x5B90;
const TIMER: u32 = D + 0x5BB0;
const COLOUR: u32 = D + 0x5BB4;
const LAYER: u32 = D + 0x5BC4;
const FLASH_DESC: u32 = D + 0x5BC8;
const NODES: u32 = D + 0x5BE4;
const TARGET_NODE: u32 = D + 0x5BF4;
/// The orbs' ages (i16 each).
const AGES: u32 = D + 0x5C04;
const KIND: u32 = D + 0x5C26;
const ORB_N: u32 = D + 0x5C28;
const ARC_N: u32 = D + 0x5C2A;
const FACING: u32 = D + 0x5C2C;
const COUNTER: u32 = D + 0x5C30;
const BYTES: u32 = D + 0x5C32;
const BYTES_B: u32 = D + 0x5D32;
const SINES: u32 = D + 0x5E34;

fn rmod(fx: &mut Fx, s0: u64, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0 as u32, ra);
    fx.div(v, m).1
}

/// The entry 0x1E03BA0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x100);
    fx.mem.write_u32(D + 0x5BBC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x100);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x5FA0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x5C2E, param);
    fx.mem.set32(TIMER, 0x122);
    for (k, node) in [3, 0x34, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xF0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xF4)), 180.0);
    fx.fix_residue(y, 0x01E0_3D3C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = D.
    let d = u64::from(D);
    for k in 0..0x100u32 {
        let v = rmod(fx, d, 0x01E0_3D64, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0xB4..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_3DB4);
        fx.mem.set8(BYTES + k, v % 8 - 4);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, d, 0x01E0_3E04, 0x28);
        fx.mem.set8(BYTES_B + k, v - 0x14);
    }
    0
}

fn orb_point(n: i32) -> u32 {
    (ORBS as i32 + 16 * n) as u32
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// a3 as a draw left it: the last primitive's last XYZ2 word (a draw
/// with depth mode 1), else `old`.
fn last_xyz2(fx: &mut Fx, before: usize, old: u32) -> u32 {
    match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => old,
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let mut a3 = emit(fx, param);
    // The entry's s1: the flash tasks counted here.
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        if f == FLASH {
            flashes += 1;
            continue;
        }
        // The steps' prologues: the entry's s0 = D, s1 = the flash count,
        // s2 = 0, s3 = the slot, s4 = its index.
        let frame = match f {
            ORB | ARC => 0x40,
            CLOSE => 0x30,
            RING_A | RING_B | BURST | WIDE | ARC_B | BAND | SPARK | RING_C | WHITE => 0x20,
            _ => panic!("b632: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        let (d, n) = (u64::from(D), u64::from(flashes));
        match frame {
            0x20 => fx.saved(s, &[(0x10, TASK_RA), (0, d)]),
            0x30 => fx.saved(s, &[(0x20, TASK_RA), (0x10, n), (0, d)]),
            _ => fx.saved(s, &[(0x30, TASK_RA), (0x20, 0), (0x10, n), (0, d)]),
        }
        match f {
            RING_A => ring(fx, t, 0, 0x4170_0000, (0x14, 5), (5, 3), 0, true),
            RING_B => ring(fx, t, 0x190, 0x4120_0000, (0x32, 0xA), (5, 3), 0, true),
            RING_C => ring(fx, t, 0x13C0, 0x4120_0000, (0x50, 0xA), (3, 2), 5, false),
            BURST => burst(fx, t),
            ORB => orb(fx, t, slot, k),
            ARC => arc(fx, t),
            WIDE => wide(fx, t, flashes, false),
            BAND => wide(fx, t, flashes, true),
            ARC_B => arc_b(fx, t),
            CLOSE => close(fx, t),
            SPARK => spark(fx, t, flashes),
            _ => white(fx, t, flashes),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let g = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        for (step, local) in [(RING_A, 0x70u32), (RING_B, 0x80)] {
            if g(fx) == step {
                cell(fx, t, sp + local, 0x14);
                tex(fx, TEX);
                fx.mem.set16(D + 0xB8, 0xA);
                let r = fx.mem.i16(t + 0xA);
                fx.mem.set16(D + 0xC0, r);
                for o in [0u32, 4, 8] {
                    let w = fx.mem.read_u32(t + 0x10 + o);
                    fx.mem.write_u32(D + 0x20 + o, w);
                }
                quads4::ring_facing_flat(fx, uv::uv_rect, D, t, 0x20, BYTES);
                a3 = 0;
                fx.mem.set16(D + 0xB8, 0);
            }
        }
        if g(fx) == BURST {
            tex(fx, D + 0x29C);
            cell(fx, t, sp + 0x90, 0x9B);
            rings(fx, t, 0xA, 1);
            a3 = 0;
        }
        let strips = |fx: &mut Fx, n: i32, a3: u32| {
            fx.mem.set16(D + 0xB8, n);
            let before = fx.out.prims.len();
            billboard::arc_strips_flat(fx, D, t, 0x21, sp + 0x60);
            fx.mem.set16(D + 0xB8, 0);
            last_xyz2(fx, before, a3)
        };
        if g(fx) == ORB {
            let i = fx.mem.i16(t + 0x50);
            copy4(fx, sp + 0x60, orb_point(i));
            a3 = strips(fx, 0x18, a3);
        }
        if g(fx) == ARC {
            copy4(fx, sp + 0x60, t + 0x10);
            a3 = strips(fx, 0x24, a3);
        }
        if g(fx) == WIDE {
            copy4(fx, sp + 0x60, t + 0x10);
            a3 = strips(fx, 0x18, a3);
        }
        if g(fx) == ARC_B {
            tex(fx, D + 0x29C);
            cell(fx, t, sp + 0xA0, 0x9B);
            fx.mem.set16(D + 0xB8, 9);
            // sp + 0x60 as an earlier draw left it; arc_quads leaves a3 = 0
            // (b491's rule) [run].
            billboard::arc_quads(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if g(fx) == CLOSE {
            tex(fx, TEX);
            cell(fx, t, sp + 0xB0, 0x14);
            rings(fx, t, 5, 1);
            a3 = 0;
        }
        if g(fx) == BAND {
            fx.mem.set16(D + 0xB8, 0x18);
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES);
            node_position_saving(fx, h, slot, sp + 0x60, u64::from(D), Some(u64::from(flashes)), 0x01E0_42D0);
            let before = fx.out.prims.len();
            billboard::arc_strips_flat(fx, D, t, 0x21, sp + 0x60);
            fx.mem.set16(D + 0xB8, 0);
            a3 = last_xyz2(fx, before, 1);
        }
        if g(fx) == SPARK {
            tex(fx, TEX + 4);
            cell(fx, t, sp + 0xC0, 0x14);
            fx.mem.set16(D + 0xBE, 5);
            for o in [0u32, 4, 8] {
                let w = fx.mem.read_u32(t + 0x10 + o);
                fx.mem.write_u32(sp + 0x60 + o, w);
            }
            billboard::quad(fx, uv::uv_rect, D, t, 0x11, sp + 0x60);
            a3 = 0;
        }
        if g(fx) == RING_C {
            cell(fx, t, sp + 0xD0, 0x14);
            tex(fx, TEX);
            rings(fx, t, 6, 0);
            a3 = 0;
        }
        if g(fx) == WHITE {
            tex(fx, TEX + 8);
            cell(fx, t, sp + 0xE0, 0x14);
            let r = fx.mem.i16(t + 0x56);
            for (o, v) in [(0x94u32, r << 6), (0x96, 0), (0x98, 0x3F), (0x9A, 0x3F), (0xBE, 5)] {
                fx.mem.set16(D + o, v);
            }
            let i = fx.mem.i16(t + 0x50);
            copy4(fx, sp + 0x60, orb_point(i));
            billboard::quad(fx, uv::uv_rect, D, t, 0x21, sp + 0x60);
            a3 = 0;
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
        0xA1 => 1,
        0x116 => 4,
        0x10C => 2,
        0xAC..=0x106 => 6,
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
    copy4(fx, D + 0x94, r);
}

/// `rows` rows (D+0xB8) of `ring_quads_z` in layout `layout` (D+0xB0) on
/// the bytes D+0x5D32, both cleared after.
fn rings(fx: &mut Fx, t: u32, rows: i32, layout: i32) {
    fx.mem.set16(D + 0xB8, rows);
    fx.mem.set16(D + 0xB0, layout);
    quads4::ring_quads_z(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
    fx.mem.set16(D + 0xB8, 0);
    fx.mem.set16(D + 0xB0, 0);
}

/// 0x1E04590(D, step, 0): the first free task cleared and set off at
/// D+0x250 (i16 +0, floats +0x10..+0x18) with the colour D+0x5BB4
/// (twice), kind D+0x5C26 (+0x50); `saves` the caller's s0..s4, `ra` the
/// return address.
fn spawn(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        let t = D + 0x2A0 + 0x70 * k;
        fx.bzero(t, 0x70);
        fx.mem.set16(t + 0x58, 1);
        for o in [0x60u32, 0x64] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + o + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        for k in 0..3 {
            let w = fx.mem.read_u32(POS + 4 * k);
            fx.mem.write_u32(t + 0x10 + 4 * k, w);
        }
        let s = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x50, s);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

/// 0x1E01E90(D, 0): the update counter, the orbs and the spawns (frame
/// 0x70: ra, the entry's s3 (the parameter), s2 / s1 (0), s0 = D).
/// Returns a3 as it leaves it: the second orb loop's counter (the orb
/// count), else 0 after a spawn [H].
fn emit(fx: &mut Fx, param: i32) -> u32 {
    let mut a3;
    let sp = fx.enter(0x70);
    let d = u64::from(D);
    fx.saved(sp, &[(0x40, 0x01E0_3E50), (0x30, param as i64 as u64), (0x20, 0), (0x10, 0), (0, d)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let mut i = 0i32;
    while i < fx.mem.i16(ORB_N) {
        let at = (AGES as i32 + 2 * i) as u32;
        let p = orb_point(i);
        if fx.mem.i16(at) == 0 {
            let h = fx.target_handle();
            let slot = fx.mem.i16(TARGET_NODE);
            let s1 = (D as i32 + 2 * i) as u32;
            node_position_saving(fx, h, slot, sp + 0x50, i as u32 as u64, Some(u64::from(s1)), 0x01E0_1EEC);
            let v = rmod(fx, i as u32 as u64, 0x01E0_1EF4, 0x14);
            let x = ee::add(fx.mem.f(sp + 0x50), ee::from_i32(v - 0xA));
            fx.mem.setf(sp + 0x50, x);
            let v = rmod(fx, i as u32 as u64, 0x01E0_1F28, 0x14);
            let x = ee::add(fx.mem.f(sp + 0x54), ee::from_i32(v - 0xA));
            fx.mem.setf(sp + 0x54, x);
            for o in [8u32, 4, 8] {
                let v = ee::div(ee::sub(fx.mem.f(sp + 0x50 + o), fx.mem.f(p + o)), 60.0);
                fx.mem.setf(p + 0xA0 + o, v);
            }
            let a = fx.mem.i16(at) + 1;
            fx.mem.set16(at, a);
        }
        i = (i + 1) as i16 as i32;
    }
    let n_orbs = i64::from(i.max(0));
    let mut i = 0i32;
    while i < fx.mem.i16(ORB_N) {
        let at = (AGES as i32 + 2 * i) as u32;
        let p = orb_point(i);
        let a = fx.mem.i16(at);
        if a != 0 && a < 0x3C {
            for o in [8u32, 4, 0] {
                let v = ee::add(fx.mem.f(p + o), fx.mem.f(p + 0xA0 + o));
                fx.mem.setf(p + o, v);
            }
            let a = fx.mem.i16(at) + 1;
            fx.mem.set16(at, a);
        }
        i = (i + 1) as i16 as i32;
    }
    // The second loop's counter is a3.
    a3 = i as u32;
    let (s0, s1, s2) = (n_orbs as u64, d + 2 * n_orbs as u64, d + 16 * n_orbs as u64);
    let saves = [s0, s1, s2, d, 0];
    let c = fx.mem.i16(COUNTER);
    if [0x73, 0x82, 0x91, 0xA0, 0xAF, 0xBE].contains(&c) {
        let m = fx.mem.i16(ORB_N);
        fx.mem.set16(KIND, m);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        let k = fx.mem.i16(KIND);
        node_position_saving(fx, h, slot, orb_point(k), s0, Some(s1), 0x01E0_20E0);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, POS, s0, Some(s1), 0x01E0_20F8);
        spawn(fx, ORB, saves, 0x01E0_210C);
        spawn(fx, WHITE, saves, 0x01E0_2120);
        for ra in [0x01E0_2134, 0x01E0_2148, 0x01E0_215C] {
            spawn(fx, RING_B, saves, ra);
        }
        a3 = 0;
        let m = fx.mem.i16(ORB_N) + 1;
        fx.mem.set16(ORB_N, m);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x3A..0x9F).contains(&c) {
        spawn(fx, BAND, saves, 0x01E0_2198);
        a3 = 0;
        if fx.mem.i16(COUNTER) < 0x64 {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES);
            node_position_saving(fx, h, slot, POS, s0, Some(s1), 0x01E0_21C0);
            spawn(fx, CLOSE, saves, 0x01E0_21D4);
            spawn(fx, ARC_B, saves, 0x01E0_21E8);
        }
    }
    if fx.mem.i16(COUNTER) == 0x41 {
        spawn(fx, FLASH, saves, 0x01E0_220C);
        a3 = 0;
    }
    fx.leave(0x70);
    a3
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn odd_age(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54).wrapping_rem(2) == 1
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends
/// (`end`; otherwise the byte is left).
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8, end: bool) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else if end {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// Both alphas in by 0x40 under 0xC0 for `n` updates, then +0x63 out by
/// 14 twice (the task ending on the first).
fn pulse(fx: &mut Fx, t: u32, n: i32) {
    if fx.mem.i16(t + 0x54) < n {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
        fade_in(fx, t, 0x67, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x10, 0xE, true);
        fade_out(fx, t, 0x63, 0x10, 0xE, false);
    }
}

/// +0x20 / +0x24 / +0x28 = rand % 0xFFFF each.
fn turn(fx: &mut Fx, t: u32, ras: [u32; 3]) {
    for (o, ra) in [0x20u32, 0x24, 0x28].into_iter().zip(ras) {
        let v = rmod(fx, u64::from(t), ra, 0xFFFF);
        fx.mem.set32(t + o, v);
    }
}

/// 0x1E02230 / 0x1E023C0 / 0x1E035F0 (D, task; frame 0x20; `o` the rand
/// return addresses' offset from 0x1E02230's): a ring of grey quads (size
/// `size`, cell row 0..6, angle 0..359 (up by `spin` a update), +0xA 0..7,
/// +0xC the kind, radius rand % m50 + a50 (`grow`ing by +0xE 16..45 from
/// the second update), row 0..34, at 0x80 for rand % m6 + a6 updates,
/// then out by 8).
#[allow(clippy::too_many_arguments)]
fn ring(fx: &mut Fx, t: u32, o: u32, size: u32, (m50, a50): (i32, i32), (m6, a6): (i32, i32), spin: i32, grow: bool) {
    let s = u64::from(t);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, size);
        let v = rmod(fx, s, 0x01E0_2258 + o, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, s, 0x01E0_2284 + o, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_22A0 + o);
        fx.mem.set16(t + 0xA, v % 8);
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, k);
        let v = rmod(fx, s, 0x01E0_22C4 + o, m50);
        fx.mem.set16(t + 0x50, v + a50);
        let v = rmod(fx, s, 0x01E0_22E4 + o, 0x1E);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, s, 0x01E0_2304 + o, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, s, 0x01E0_2320 + o, m6);
        fx.mem.set16(t + 6, v + a6);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, 0xA, 8, true);
    }
    if spin != 0 {
        let a = fx.mem.i16(t + 0x5A) + spin;
        fx.mem.set16(t + 0x5A, a);
    }
    if grow && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02550 (D, task; frame 0x20): a ring of quads (size 2..6, (0x80,
/// 0x30, 0x30), cell row 0..3, turned at random, radius 10..39 growing by
/// 1..3, row +0x52 from 1 stepping, +8 1..8, at 0x60 for 2..4 updates then
/// out by 11).
fn burst(fx: &mut Fx, t: u32) {
    let s = u64::from(t);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, s, 0x01E0_2574, 5);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 2));
        let v = fx.rand_from(t, 0x01E0_25A0);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x30, 0x30);
        turn(fx, t, [0x01E0_25D4, 0x01E0_25F0, 0x01E0_260C]);
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, s, 0x01E0_2630, 0x1E);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, s, 0x01E0_2650, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, s, 0x01E0_266C, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, 0x01E0_268C);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, s, 0x01E0_26AC, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0xD, 0xB, true);
    }
    let r = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, r);
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
}

/// 0x1E02750 (D, task; frame 0x40): the orb - a band (grey 0xCF over red
/// (0xFF / 0xC0, 8, 8) by its age's parity) of radius 40 growing by 10 to
/// 100, in by 0x40 under 0xC0 for 60 updates, out by 14; before 61 a spark
/// 0x1E03450 and a ring 0x1E035F0 a update at its point, on 60..64 a wide
/// band 0x1E02E00, on 60 ten rings 0x1E02230 and four arcs 0x1E02AE0
/// (kinds 0..3; D+0x5C2A up one); turned by the camera and 0x4000 more.
fn orb(fx: &mut Fx, t: u32, slot: u32, k: u32) {
    let d = u64::from(D);
    // The spawns' saves: s0 (the entry's D, or the loop counter), s1 = the
    // task, s2 = D, the entry's s3 / s4 (the slot, its index).
    let sv = |s0: u64| [s0, u64::from(t), d, u64::from(slot), u64::from(k)];
    let mut s0 = d;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4220_0000);
        l2::rgb(fx, t + 0x60, 0xCF, 0xCF, 0xCF);
        l2::rgb(fx, t + 0x64, 0x80, 8, 8);
        let v = rmod(fx, d, 0x01E0_27B0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, d, 0x01E0_27CC, 0xA);
        fx.mem.set16(t + 0xE, v + 1);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
    }
    let g = if odd_age(fx, t) { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x64, g, 8, 8);
    if fx.mem.i16(t + 0x54) < 0x3C {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x10, 0xE, true);
    }
    let at_point = |fx: &mut Fx| {
        let i = fx.mem.i16(t + 0x50);
        copy4(fx, POS, orb_point(i));
    };
    if fx.mem.i16(t + 0x54) == 0x3C {
        at_point(fx);
        for i in 0..0xAu64 {
            spawn(fx, RING_A, sv(i), 0x01E0_28F8);
        }
        s0 = 0xA;
        for (kind, ra) in [(0, 0x01E0_2934u32), (1, 0x01E0_2950), (2, 0x01E0_296C), (3, 0x01E0_2988)] {
            fx.mem.set16(KIND, kind);
            spawn(fx, ARC, sv(s0), ra);
        }
        let n = fx.mem.i16(ARC_N) + 1;
        fx.mem.set16(ARC_N, n);
    }
    let a = fx.mem.i16(t + 0x54);
    if (0x3C..0x41).contains(&a) {
        at_point(fx);
        spawn(fx, WIDE, sv(s0), 0x01E0_29F0);
    }
    if fx.mem.i16(t + 0x54) < 0x3D {
        at_point(fx);
        spawn(fx, SPARK, sv(s0), 0x01E0_2A40);
        spawn(fx, RING_C, sv(s0), 0x01E0_2A54);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.f(t + 0x48);
        if r < 100.0 {
            fx.mem.setf(t + 0x48, ee::add(r, 10.0));
        }
    }
    camera_angles_saving(fx, t + 0x20, s0, u64::from(t), 0x01E0_2AA0);
    let r = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, r);
    age(fx, t);
}

/// 0x1E02AE0 (D, task; frame 0x40): an arc (white over (0xFF / 0xC0, 0x80
/// / 0x70, same) by its age's parity) of radius 20 widening by 35 (the
/// inner 80 behind), turned about y into its kind's quarter (+0x24 =
/// 0x4000 kind + 0..0x1FFF) and about x by a quarter (D+0x5C2A % 4) +
/// 0..0x4FFF, in by 0x60 under 0xC0 for 5 updates, out by 15.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
        // s0 = the quarter (else the entry's D) [F 0x1E02B3C..0x1E02B98].
        let q = fx.mem.i16(ARC_N).wrapping_rem(4);
        let s0: u64 = match q {
            0 => 0,
            1 => 0x4000,
            2 => 0x8000,
            3 => 0xC000,
            _ => u64::from(D),
        };
        let kind = fx.mem.i16(t + 0x50);
        let ras = [(0x01E0_2BB0u32, 0x01E0_2BCCu32), (0x01E0_2C00, 0x01E0_2C20), (0x01E0_2C54, 0x01E0_2C78), (0x01E0_2CAC, 0x01E0_2CD0)];
        if (0..4).contains(&kind) {
            let (ra_y, ra_x) = ras[kind as usize];
            let v = fx.rand_from(s0 as u32, ra_y);
            fx.mem.set32(t + 0x24, v % 0x2000 + 0x4000 * kind);
            let v = rmod(fx, s0, ra_x, 0x5000);
            fx.mem.write_u32(t + 0x20, (s0 as u32).wrapping_add(v as u32));
        }
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 5 {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0x11, 0xF, true);
    }
    if odd_age(fx, t) {
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
    } else {
        l2::rgb(fx, t + 0x64, 0xC0, 0x70, 0x70);
    }
    let r = ee::add(fx.mem.f(t + 0x48), 35.0);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::sub(r, 80.0));
    age(fx, t);
}

/// 0x1E02E00 (`!band`) / 0x1E032E0 (`band`) (D, task; frame 0x20): a wide
/// arc (radius 200..499 / 15..64, grey over (0x80 / 0x60, 0x40 / 0x30,
/// same) by its age's parity), in by 0x30 under 0x60 for 2 updates, out by
/// 11; turned by the camera and 0x4000 more.
fn wide(fx: &mut Fx, t: u32, flashes: u32, band: bool) {
    let o = if band { 0x4E0 } else { 0 };
    if fx.mem.i16(t + 0x58) == 1 {
        let (m, a) = if band { (0x32, 15.0) } else { (0x12C, 200.0) };
        let v = rmod(fx, u64::from(t), 0x01E0_2E24 + o, m);
        fx.mem.setf(t + 0x48, ee::add(a, ee::from_i32(v)));
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 2 {
        fade_in(fx, t, 0x63, 0x60, 0x30);
    } else {
        fade_out(fx, t, 0x63, 0xD, 0xB, true);
    }
    if odd_age(fx, t) {
        l2::rgb(fx, t + 0x64, 0x80, 0x40, 0x40);
    } else {
        l2::rgb(fx, t + 0x64, 0x60, 0x30, 0x30);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_2F3C + o);
    let r = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, r);
    age(fx, t);
}

/// 0x1E02F70 (D, task; frame 0x20): an arc of quads ((0xCF, 0x80, 0x80)
/// over (0x80, 8, 8)) of radius 10..29 shrinking by 1 (the outer 6
/// beyond), turned at random, from angle 45 spanning 5, in by 0x40 under
/// 0xC0 for 8 updates, out by 9.
fn arc_b(fx: &mut Fx, t: u32) {
    let s = u64::from(t);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, s, 0x01E0_2F94, 0x14);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        l2::rgb(fx, t + 0x60, 0xCF, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 8, 8);
        let v = rmod(fx, s, 0x01E0_2FE4, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        turn(fx, t, [0x01E0_300C, 0x01E0_3028, 0x01E0_3044]);
        fx.mem.set16(t + 0x5A, 0x2D);
        fx.mem.set16(t + 0x5C, 5);
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    let r = ee::sub(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::add(6.0, r));
    age(fx, t);
}

/// 0x1E03100 (D, task; frame 0x30): a grey ring of quads at the
/// attacker's node 3 (size 3, cell row 4, turned at random, radius
/// 300..649 closing by 10..29 a update (no floor), row +0x52 from 1
/// stepping, +8 1..8), at 0x60 for 2..4 updates, out by 11.
fn close(fx: &mut Fx, t: u32) {
    let s = u64::from(t);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4040_0000);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        turn(fx, t, [0x01E0_3150, 0x01E0_316C, 0x01E0_3188]);
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, s, 0x01E0_31AC, 0x15E);
        fx.mem.set16(t + 0x50, v + 0x12C);
        let v = rmod(fx, s, 0x01E0_31CC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, s, 0x01E0_31E8, 0x14);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = fx.rand_from(t, 0x01E0_3208);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, s, 0x01E0_3228, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0xD, 0xB, true);
    }
    let r = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, r);
    let r = fx.mem.i16(t + 0x50) - fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    node_position_saving(fx, h, slot, t + 0x10, s, Some(u64::from(D)), 0x01E0_32C8);
}

/// 0x1E03450 (D, task; frame 0x20): b426's spark in (0xFF, 0x80, 0x80): a
/// quad 150 x 150 shrinking by 15 to 20, cell row 0..7, in by 0x40 for 3
/// updates, out by 14; turned by the camera.
fn spark(fx: &mut Fx, t: u32, flashes: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x96);
        fx.mem.set16(t + 0xA, 0x96);
        let v = fx.rand_from(t, 0x01E0_347C);
        fx.mem.set16(t + 0x56, v % 8);
        fx.mem.write_u32(t + 0x34, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, u64::from(t), 0x01E0_34D0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, u64::from(t), 0x01E0_34EC, 0xB4);
        fx.mem.set16(t + 0x5C, v - 0x5A);
    }
    pulse(fx, t, 3);
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s >= 0x15 {
                fx.mem.set16(t + o, s - 0xF);
            }
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_35CC);
    age(fx, t);
}

/// 0x1E03770 (D, task; frame 0x20): b426's grey quad in white: 25 x 25
/// growing by 10 to 84, its cell row toggling 0 / 1; in for 60 updates;
/// turned by the camera.
fn white(fx: &mut Fx, t: u32, flashes: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x19);
        fx.mem.set16(t + 0xA, 0x19);
        let v = fx.rand_from(t, 0x01E0_379C);
        fx.mem.set16(t + 0x56, v % 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    pulse(fx, t, 0x3C);
    let r = fx.mem.i16(t + 0x56);
    fx.mem.set16(t + 0x56, i32::from(r == 0));
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s < 0x4B {
                fx.mem.set16(t + o, s + 0xA);
            }
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_38C8);
    age(fx, t);
}

/// 0x1E038F0(D, t): b016's flash in red (0x80 / 0x60, 0x10, 0x10 by the
/// age's parity): the target over the window (UVs from +8 / 10 and +0xA /
/// 10 over 512) into the descriptor at D+0x5BC8, submitted once with no
/// ordering table; alpha up by 12 below 0x80 for 150 updates, then down by
/// 15. `k` the unit flash's s1 (the slot index).
fn flash(fx: &mut Fx, t: u32, k: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x30, 0x01E0_479C), (0x20, u64::from(k)), (0x10, u64::from(D))]);
    // swc1 f20 / f21: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x10, 0x10);
        fx.mem.set16(t + 0x58, 2);
    }
    let hd = fx.self_handle();
    let s = fx.mem.i16(NODES);
    node_position_saving(fx, hd, s, sp + 0x40, u64::from(t), Some(u64::from(D)), 0x01E0_3990);
    let hd = fx.self_handle();
    unit_position_saving(fx, hd, sp + 0x40, u64::from(t), 0x01E0_39A4);
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
    let bottom = ee::div(ee::sub(512.0, ee::mul(3.0, h)), 512.0);
    for (o, v) in [(0x18u32, ee::div(w, 512.0)), (0x1C, ee::div(h, 512.0)), (0x28, ee::div(ee::sub(512.0, w), 512.0)), (0x2C, bottom)] {
        fx.mem.setf(d + o, v);
    }
    fx.submit(d, 0, 0);
    let g = if odd_age(fx, t) { 0x80 } else { 0x60 };
    l2::rgb(fx, t + 0x60, g, 0x10, 0x10);
    if fx.mem.i16(t + 0x54) < 0x96 {
        fade_in(fx, t, 0x63, 0x80, 0xC);
    } else {
        fade_out(fx, t, 0x63, 0x11, 0xF, true);
    }
    age(fx, t);
    fx.leave(0x50);
}

/// 0x1E046D0(D) (b016's 0x1E03840): in a packet layer (0x2078A0) the
/// attacker drawn into the target (0x1F8270 filter 0; ALPHA 0x48),
/// TEXFLUSH before and after; every flash task stepped (0x1E038F0); the
/// layer linked into the ordering table at key 0.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xA0);
    let d = sp + 0x48;
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
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x88, sp + 0x90);
    fx.texflush(regs);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) != 0 && fx.mem.read_u32(t + 0x44) == FLASH {
            flash(fx, t, k);
        }
    }
    let layer = fx.mem.read_u32(LAYER);
    fx.layer_end(layer, OT, 0);
    fx.leave(0xA0);
}
