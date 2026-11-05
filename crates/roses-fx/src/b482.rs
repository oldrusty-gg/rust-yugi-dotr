//! BMODULE b482 (card 252), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E034B0, data area 0x1ED0000) [F] (helper A11, 31
//! Oct 2026): b180's program shape (400 tasks of 0x70 bytes at D+0x2A0,
//! alive +0x58, step +0x44 called through `jalr`; spawned by 0x1E03C60 =
//! `spawn::task400`) drawn with the second library: quads (add5d81dd3fe at
//! 0x1E01230, `billboard::quad`), rings of quads (7369422df7f8 at
//! 0x1E00920, `billboard::ring_quads`; UVs by uv_rect 0x1E00080), sparks
//! (21cb40d76bec at 0x1E014F0, `quads4::sparks`) and arcs (e6dce326551d at
//! 0x1E00ED0, `billboard::arc_strips`), plus b180's unit flash (0x1E03DF0,
//! d1cf0abc65df) with its own flash step (pink, in by 6 for 170 updates).
//! Update 1 the stage light task, 47 a camera light (slots 4 / 5 from
//! D+0xB1A0 at node 0x49), 50 the flash; 30..47 and 150..165 grey rings at
//! nodes 0x30 / 0x42; 58..170 a spark a update and on 160 twenty white
//! quads at the attacker's point; 175..215 pink quads and rings round it;
//! 210..225 rings, quads, sparks every other update, an arc on 215; an
//! arc ring on 156 at the point. Returns 1 on update 202, 4 on 270, 2 on
//! 245, 6 on 213..239, 8 once the 280-update timer +0xB210 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, spawn::task400, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const QUAD_A: u32 = 0x01E0_2180;
const RING_A: u32 = 0x01E0_23B0;
const RING_B: u32 = 0x01E0_25D0;
const QUAD_B: u32 = 0x01E0_2780;
const SPARK: u32 = 0x01E0_2A40;
const ARC_A: u32 = 0x01E0_2C20;
const ARC_B: u32 = 0x01E0_2D10;
const CAMERA: u32 = 0x01E0_2E40;
const LIGHT: u32 = 0x01E0_30E0;
/// The flash tasks, run by the unit flash 0x1E03DF0.
const FLASH: u32 = 0x01E0_31F0;
/// The return address of the entry's `jalr` into a step (0x1E037C0 + 8).
const TASK_RA: u64 = 0x01E0_37C8;
const POS: u32 = D + 0x250;
const CAM: u32 = D + 0xB1A0;
const AIM: u32 = D + 0xB1D0;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const LAYER: u32 = D + 0xB224;
const FLASH_DESC: u32 = D + 0xB228;
const NODES: u32 = D + 0xB244;
const KIND: u32 = D + 0xB2B0;
const FACING: u32 = D + 0xB2B4;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;
const SINES: u32 = D + 0xB4BC;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E034B0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x118);
    for (k, node) in [0x49, 0x5C, 0x30, 0x42, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x298 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_36A8);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_36D8, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_372C);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

/// The last XYZ2 of the last primitive drawn since `before` (0 when none).
fn last_xyz2(fx: &mut Fx, before: usize) -> u32 {
    match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => 0,
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s0: the flash tasks counted here (the steps' saved s0).
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        // The step's prologue: the entry's s0 = the flashes so far, s1 = 0.
        let frame = match f {
            0 => continue,
            FLASH => {
                flashes += 1;
                continue;
            }
            RING_A => 0x30,
            CAMERA => 0x50,
            QUAD_A | RING_B | QUAD_B | SPARK | ARC_A | ARC_B | LIGHT => 0x20,
            _ => panic!("b482: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(flashes))]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(flashes))]);
        }
        match f {
            QUAD_A => quad_a(fx, t),
            RING_A => ring(fx, t, false),
            RING_B => ring(fx, t, true),
            QUAD_B => quad_b(fx, t),
            SPARK => spark(fx, t),
            ARC_A => arc(fx, t, false),
            ARC_B => arc(fx, t, true),
            CAMERA => camera(fx, t, s),
            _ => light(fx, t),
        }
        fx.leave(frame);
    }
    // a3 as the draws' last library call left it, for the unit flash's
    // first 0x207BB0 (b180's lesson): 0 after a quad, a ring or sparks, the
    // last XYZ2 after an arc.
    let mut a3 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == QUAD_A || f == QUAD_B {
            let local = if f == QUAD_A { 0x60 } else { 0x90 };
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            crate::b615::cell(fx, sp + local, 0x14, sub);
            let o = fx.mem.i16(t + 0x52);
            fx.mem.set16(D + 0xBE, o);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
            a3 = 0;
        }
        for (g, local, n, site) in [(RING_A, 0x70u32, 5, 0x01E0_3908u32), (RING_B, 0x80, 8, 0x01E0_39A8)] {
            if fx.mem.read_u32(t + 0x44) != g {
                continue;
            }
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            crate::b615::cell(fx, sp + local, 0x14, sub);
            fx.mem.set16(D + 0xB8, n);
            let v = fx.rand_from(flashes, site);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == SPARK {
            let sub = fx.mem.i16(t + 0x56);
            crate::b615::cell(fx, sp + 0xA0, 0x97, sub);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            fx.mem.set16(D + 0xB8, 0xC);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        for (g, pos) in [(ARC_A, t + 0x10), (ARC_B, AIM)] {
            if fx.mem.read_u32(t + 0x44) != g {
                continue;
            }
            fx.mem.set16(D + 0xB8, 0x24);
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, 0x21, pos);
            a3 = last_xyz2(fx, before);
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
        0xCA => 1,
        0x10E => 4,
        0xF5 => 2,
        0xD5..=0xEF => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E018F0(D, 0): the update counter and the spawns (frame 0x30: the
/// entry's s1 / s0 = 0; then s1 = D, s0 a loop count). The spawns' saves:
/// the runner's s4 (0 [H]), the entry's s3 = the parameter, s2 = D, the
/// emitter's s1 = D and s0.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3788), (0x10, 0), (0, 0)]);
    let p = param as i64 as u64;
    let go = |fx: &mut Fx, f: u32, ra: u32, s0: u64| {
        task400(fx, D, f, 0, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, p), (0x20, u64::from(D)), (0x10, u64::from(D)), (0, s0)]);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    // The emitter's s0: 0 until a loop runs, then the loop's count.
    let mut s0 = 0u64;
    if c(fx) == 1 {
        go(fx, LIGHT, 0x01E0_1944, s0);
    }
    if c(fx) == 0x32 {
        go(fx, FLASH, 0x01E0_1970, s0);
    }
    if c(fx) == 0x2F {
        go(fx, CAMERA, 0x01E0_199C, s0);
    }
    let v = c(fx);
    if v < 0xAE {
        let h = fx.self_handle();
        unit_position_saving(fx, h, POS, s0, 0x01E0_19C8);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, AIM, s0, Some(u64::from(D)), 0x01E0_19EC);
        let x = fx.mem.read_u32(POS);
        fx.mem.write_u32(AIM, x);
    } else if v < 0xD7 {
        let z = fx.mem.f(AIM + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 4.0) } else { ee::add(z, 4.0) };
        fx.mem.setf(AIM + 8, z);
    }
    let aim_to_pos = |fx: &mut Fx| {
        for k in 0..4 {
            let w = fx.mem.read_u32(AIM + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    };
    // x / y / z += rand % 10 - 5 (`m` 10, `lo` -5; y may differ).
    let jitter = |fx: &mut Fx, s0: u32, sites: [u32; 3], y: (i32, i32)| {
        for (k, site) in sites.into_iter().enumerate() {
            let (m, lo) = if k == 1 { y } else { (0xA, -5) };
            let v = rmod(fx, s0, site, m);
            let o = POS + 4 * k as u32;
            let q = ee::add(fx.mem.f(o), ee::from_i32(v + lo));
            fx.mem.setf(o, q);
        }
    };
    if (0xAF..0xD8).contains(&c(fx)) {
        for k in 0..4u32 {
            aim_to_pos(fx);
            jitter(fx, k, [0x01E0_1ACC, 0x01E0_1B00, 0x01E0_1B34], (0xA, -5));
            go(fx, QUAD_A, 0x01E0_1B70, u64::from(k));
        }
        for k in 0..2u32 {
            aim_to_pos(fx);
            jitter(fx, k, [0x01E0_1BC8, 0x01E0_1BFC, 0x01E0_1C30], (0xA, -5));
            go(fx, RING_A, 0x01E0_1C6C, u64::from(k));
        }
        s0 = 2;
    }
    if (0xD2..0xE2).contains(&c(fx)) {
        for k in 0..2u64 {
            aim_to_pos(fx);
            go(fx, RING_B, 0x01E0_1CF8, k);
        }
        fx.mem.set16(KIND, 0);
        for k in 0..5u64 {
            aim_to_pos(fx);
            go(fx, QUAD_B, 0x01E0_1D6C, k);
        }
        s0 = 5;
        if c(fx) % 2 == 1 {
            go(fx, SPARK, 0x01E0_1DCC, s0);
        }
        if c(fx) == 0xD7 {
            go(fx, ARC_A, 0x01E0_1DF8, s0);
        }
    }
    if c(fx) == 0xA0 {
        for k in 0..0x14u64 {
            aim_to_pos(fx);
            fx.mem.set16(KIND, 1);
            go(fx, QUAD_B, 0x01E0_1E64, k);
        }
        s0 = 0x14;
    }
    if (0x3A..0xAB).contains(&c(fx)) {
        aim_to_pos(fx);
        go(fx, SPARK, 0x01E0_1EF0, 0);
        s0 = 1;
    }
    let v = c(fx);
    if (0x1E..0x30).contains(&v) || (0x96..0xA6).contains(&v) {
        for (slot, node_ra, sites, ra) in [
            (NODES + 4, 0x01E0_1F78u32, [0x01E0_1F80u32, 0x01E0_1FB4, 0x01E0_1FE8], 0x01E0_2020u32),
            (NODES + 6, 0x01E0_206C, [0x01E0_2074, 0x01E0_20A8, 0x01E0_20DC], 0x01E0_2114),
        ] {
            for k in 0..3u32 {
                let h = fx.self_handle();
                let s = fx.mem.i16(slot);
                node_position_saving(fx, h, s, POS, u64::from(k), Some(u64::from(D)), node_ra);
                jitter(fx, k, sites, (0xF, 0));
                go(fx, RING_A, ra, u64::from(k));
            }
        }
        s0 = 3;
    }
    if c(fx) == 0x9C {
        go(fx, ARC_B, 0x01E0_2164, s0);
    }
    fx.leave(0x30);
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

/// The quads' random setup from the drift on: drift +0x34 / +0x30 / +0x38
/// (-50..49 each, from `sites`), divided by `by` (none for `None`), angle
/// +0x5C random, the camera's angles into +0x20, +4 / +2 / +0 random
/// (-1024..1023), orientation +0x52 (rand >> 4) % 4.
fn quad_setup(fx: &mut Fx, t: u32, sites: [u32; 9], by: Option<f32>) {
    for (o, site) in [(0x34u32, sites[0]), (0x30, sites[1]), (0x38, sites[2])] {
        let v = rmod(fx, t, site, 0x64);
        fx.mem.setf(t + o, ee::from_i32(v - 0x32));
    }
    if let Some(by) = by {
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), by);
            fx.mem.setf(t + o, q);
        }
    }
    let v = rmod(fx, t, sites[3], 0x168);
    fx.mem.set16(t + 0x5C, v);
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, sites[4]);
    for (o, site) in [(4u32, sites[5]), (2, sites[6]), (0, sites[7])] {
        let v = fx.rand_from(t, site);
        fx.mem.set16(t + o, v % 0x800 - 0x400);
    }
    let v = fx.rand_from(t, sites[8]);
    fx.mem.set16(t + 0x52, (v >> 4) % 4);
}

/// The quads' fade: in by 0x40 under 0x80 for 8 updates, then out by 4.
fn quad_fade(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < 8 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 6, 4);
    }
}

/// 0x1E02180 (D, task): a pink quad of size 80 at its point, cell row
/// (rand >> 4) % 3 + 2, its drift (-50..49) / 60 set but unused, turned by
/// the camera's angles each update.
fn quad_a(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x50);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_21B0);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        quad_setup(fx, t, [0x01E0_21E8, 0x01E0_2214, 0x01E0_2240, 0x01E0_2298, 0x01E0_22B4, 0x01E0_22BC, 0x01E0_22DC, 0x01E0_22FC, 0x01E0_231C], Some(60.0));
    }
    quad_fade(fx, t);
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2390);
    age(fx, t);
}

/// 0x1E02780 (D, task): a white-pink quad of size 100 drifting by
/// (-50..49) / 25 (kind 0) or / 45 (kind 1) a update.
fn quad_b(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x64);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_27B0);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xC0, 0xC0);
        fx.mem.set16(t + 0x58, 2);
        let by = match fx.mem.i16(t + 0x50) {
            0 => Some(25.0),
            1 => Some(45.0),
            _ => None,
        };
        quad_setup(fx, t, [0x01E0_27E8, 0x01E0_2814, 0x01E0_2840, 0x01E0_28F4, 0x01E0_2910, 0x01E0_2918, 0x01E0_2938, 0x01E0_2958, 0x01E0_2978], by);
    }
    quad_fade(fx, t);
    for k in 0..3 {
        let p = ee::add(fx.mem.f(t + 0x10 + 4 * k), fx.mem.f(t + 0x30 + 4 * k));
        fx.mem.setf(t + 0x10 + 4 * k, p);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2A18);
    age(fx, t);
}

/// 0x1E023B0 (`late` false, frame 0x30) / 0x1E025D0 (`late`, frame 0x20)
/// (D, task): a grey ring of quads: size 8..15, cell row 0..1 (before
/// update 175, else 2..3; always 2..3 when late), radius 10..49 (from 175:
/// 40..79) - growing by +0xE (16..35) when late, a random angle, row
/// 0..34, life 3..5; in by 0x40 under 0xC0, out by 14.
fn ring(fx: &mut Fx, t: u32, late: bool) {
    if fx.mem.i16(t + 0x58) == 1 {
        let s: [u32; 9] = if late {
            [0x01E0_25F4, 0x01E0_2620, 0x01E0_2620, 0x01E0_2658, 0x01E0_2658, 0x01E0_2678, 0x01E0_2694, 0x01E0_26B4, 0x01E0_26D0]
        } else {
            [0x01E0_23DC, 0x01E0_2420, 0x01E0_2444, 0x01E0_2494, 0x01E0_24BC, 0x01E0_24E0, 0x01E0_24FC, 0x01E0_251C, 0x01E0_2538]
        };
        let v = fx.rand_from(t, s[0]);
        fx.mem.setf(t + 0x48, ee::from_i32(v % 8 + 8));
        let early = !late && fx.mem.i16(COUNTER) < 0xAF;
        let v = fx.rand_from(t, if early { s[1] } else { s[2] });
        fx.mem.set16(t + 0x56, if early { v % 2 } else { v % 2 + 2 });
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let early = late || fx.mem.i16(COUNTER) < 0xAF;
        let v = rmod(fx, t, if early { s[3] } else { s[4] }, 0x28);
        fx.mem.set16(t + 0x50, if early { v + 0xA } else { v + 0x28 });
        let v = rmod(fx, t, s[5], 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, s[6], 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, s[7], 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, s[8], 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x10, 0xE);
    }
    if late && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02A40 (D, task): sparks: radius 120 (inner 62) growing by
/// (200..279) / 10 a update with the inner at 0.2 of it; kind 0 pink on
/// black, 1 violet on grey; random angles +0x20..+0x28 (% 0xFFFF); alpha
/// 0x80 for 8 updates, then down by 5.
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42F0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(120.0, 58.0));
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xC0, 0x80, 0x80);
            l2::rgb(fx, t + 0x64, 0, 0, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xA0, 0x80, 0xFF);
            l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        }
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 0x1E);
        let v = rmod(fx, t, 0x01E0_2AF8, 0x50);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0xC8));
        let q = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, q);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, site) in [(0x20u32, 0x01E0_2B40u32), (0x24, 0x01E0_2B5C), (0x28, 0x01E0_2B78)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, 7, 5);
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3E4C_CCCD), r));
    age(fx, t);
}

/// 0x1E02C20 (`at_aim` false: radius 300, white outer, 14 updates) /
/// 0x1E02D10 (true: radius 53 growing by 3 below 200, grey outer, 54
/// updates) (D, task): an arc ring, inner pink (0xFF, 0x80, 0x80), angles
/// 10 / 10; outer alpha in by 0x30 under 0xC0, then out by 15; turned by
/// the camera's angles (the first + 0x4000).
fn arc(fx: &mut Fx, t: u32, at_aim: bool) {
    if fx.mem.i16(t + 0x58) == 1 {
        let (r, grey) = if at_aim { (0x4254_0000u32, 0xC0) } else { (0x4396_0000, 0xFF) };
        fx.mem.write_u32(t + 0x4C, r);
        l2::rgb(fx, t + 0x64, grey, grey, grey);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let life = if at_aim { 0x36 } else { 0xE };
    if fx.mem.i16(t + 0x54) < life {
        fade_in(fx, t, 0x67, 0xC0, 0x30);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF);
    }
    let ra = if at_aim { 0x01E0_2DD0 } else { 0x01E0_2CE0 };
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, ra);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if at_aim && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.f(t + 0x4C);
        if r < 200.0 {
            fx.mem.setf(t + 0x4C, ee::add(r, 3.0));
        }
    }
    age(fx, t);
}

/// 0x1E02E40 (D, task): camera slots 4 / 5 (0x1F7E20) from D+0xB1A0 (128,
/// 0, 0, 0; node 0x49's point; 0, 123, w): for 220 updates w up by 0.03
/// below 1, the second / third values up by 1 below 20 / 10; then w down
/// by 0.3 (the task ends at 0.1, or with the timer at 1); slots cleared
/// when it ends.
fn camera(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0), (8, 0), (0xC, 0), (0x20, 0), (0x24, 0x42F6_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        for k in 0..3 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(CAM + 0x10 + 4 * k, w);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.set16(t + 0x50, 0);
    }
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_2F1C);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
        fx.mem.write_u32(CAM + 0x10 + 4 * k, w);
    }
    if fx.mem.i16(t + 0x54) < 0xDC {
        let w = fx.mem.f(CAM + 0x28);
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, f32::from_bits(0x3CF5_C28F)));
        }
        for (o, cap) in [(4u32, 20.0f32), (8, 10.0)] {
            let v = fx.mem.f(CAM + o);
            if v < cap {
                fx.mem.setf(CAM + o, ee::add(v, 1.0));
            }
        }
    } else {
        let w = fx.mem.f(CAM + 0x28);
        if w <= f32::from_bits(0x3DCC_CCCD) {
            fx.mem.set16(t + 0x58, 0);
        } else {
            fx.mem.setf(CAM + 0x28, ee::sub(w, f32::from_bits(0x3E99_999A)));
        }
    }
    let (p, ra) = if fx.mem.i16(t + 0x58) == 0 { (0, 0x01E0_3088) } else { (CAM, 0x01E0_309C) };
    crate::blib::exe::camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
}

/// 0x1E030E0 (D, task): the stage light (0x1F7F10) down by 10 to 0x30 for
/// 151 updates, back up by 10 to 0x80 once the timer is under 30; 0x80 and
/// the task's end when the timer is at 3 (b180's 0x1E021D0).
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

/// 0x1E031F0(D, t): b180's flash step (the target over the window, UVs
/// inset by +8 / 10 and +0xA / 10 over 512, into the descriptor at
/// +0xB228, no ordering table) in pink (0xFF, 0x80, 0x80): alpha up by 6
/// below 0x80 for 170 updates, then down by 7. `regs` the unit flash's s0
/// / s1 (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_3EFC), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f20 / f21: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
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
    if fx.mem.i16(t + 0x54) < 0xAA {
        fade_in(fx, t, 0x63, 0x80, 6);
    } else {
        fade_out(fx, t, 0x63, 9, 7);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E03DF0(D) (d1cf0abc65df, b180's 0x1E02DF0): in a packet layer
/// (0x2078A0) the attacker drawn into the target (0x1F8270 filter 0; ALPHA
/// 0x48), TEXFLUSH before and after; every flash task stepped (0x1E031F0);
/// the layer linked into the ordering table at key 0 when any task was
/// alive.
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
    // s1 counts every live task (the `bne`'s delay slot).
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
