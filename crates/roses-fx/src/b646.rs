//! BMODULE b646 (card 609), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02670, data area 0x1ED0000) [F]: a pool of 400 tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step
//! +0x44 called through `jalr`; spawned by 8154ee20301e at 0x1E02D70,
//! `spawn::task400_b4`) drawn with the second library: single quads
//! (add5d81dd3fe at 0x1E01230, `billboard::quad`), arcs (e6dce326551d at
//! 0x1E00ED0 and 3f36e61a971c at 0x1E016C0, `billboard::arc_strips` /
//! `quads4::arc_strips_52`), rings of quads (7369422df7f8 at 0x1E00920) and
//! full-window tints (561a6131a635 at 0x1E014F0, `quads4::tint4`), UVs by
//! uv_rect 0x1E00080. White motes orbit the attacker's node 0x36 on
//! updates 36, 77, 100 and node 0x3F on 59, 101 (13 each); an arc at node
//! 0x15 on 146; from 146 to 280 a blue swirl at that node with two grey
//! rings a update at the target, and on odd updates from 166 a red / blue
//! tint (alternating) keyed by a point 50 in front of the target. Returns
//! 1 on update 160, 4 on 340, 2 on 310, 6 on 171..304, 8 once the
//! 350-update timer +0xB210 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::b615::{cell, rmod};
use crate::blib::{billboard, quads4, spawn::task400_b4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const MOTE: u32 = 0x01E0_1D80;
const ARC: u32 = 0x01E0_1F50;
const SWIRL: u32 = 0x01E0_2110;
const RING: u32 = 0x01E0_2340;
const TINT: u32 = 0x01E0_24D0;
/// A step that only returns 0 (skipped by the entry).
const IDLE: u32 = 0x01E0_2660;
/// The return address of the entry's `jalr` into a step (0x1E02978 + 8).
const TASK_RA: u64 = 0x01E0_2980;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const TINT_W: u32 = D + 0xB234;
const NODES: u32 = D + 0xB248;
const KIND: u32 = D + 0xB2B4;
const FACING: u32 = D + 0xB2B8;
const COUNTER: u32 = D + 0xB2BC;
const BYTES: u32 = D + 0xB2BE;
const SINES: u32 = D + 0xB4C0;

/// The entry 0x1E02670.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x130);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x130);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2BA, param);
    fx.mem.set32(TIMER, 0x15E);
    for (k, node) in [0x36, 0x3F, 0x15, 0x16, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB258, r);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x298 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(k, 0x01E0_2894);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_28E4);
        fx.mem.set16(D + 0xB268 + 2 * k, v % 8);
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
        match f {
            0 | IDLE => {}
            MOTE => mote(fx, t),
            ARC => arc(fx, t),
            SWIRL => swirl(fx, t),
            RING => ring(fx, t),
            TINT => tint(fx, t),
            _ => panic!("b646: unknown task step {f:#x}"),
        }
    }
    // The entry's s0: the live tasks counted in the draw loop.
    let mut s0 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        s0 += 1;
        if fx.mem.read_u32(t + 0x44) == MOTE {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x14, sub);
            fx.mem.set16(D + 0xBE, 0);
            billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == SWIRL {
            fx.mem.set16(D + 0xB8, 6);
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 4);
            // 0x1F6830's frame (the entry's s0 / s1) is read later as a
            // missing texture's TEX1 [run].
            node_position_saving(fx, h, slot, t + 0x10, u64::from(s0), Some(u64::from(D)), 0x01E0_2AB0);
            quads4::arc_strips_52(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == RING {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x14, sub);
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand_from(s0, 0x01E0_2B30);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == TINT {
            let h = fx.target_handle();
            fx.unit_position(h, sp + 0x60);
            // 0x1F71A0's frame below the entry's (its saved ra, s0).
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_2B90), (0, u64::from(s0))]);
            let z = fx.mem.f(sp + 0x68);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 50.0) } else { ee::add(z, 50.0) };
            fx.mem.setf(sp + 0x68, z);
            let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
            let mut x = fx.mem.read_u64(sp + 0xB8);
            if let Some(w) = fx.project(p, None, &mut x) {
                fx.mem.write_u64(sp + 0xB8, x);
                fx.mem.setf(TINT_W, w);
            }
            project_residue(fx, sp, sp + 0x60);
            for kind in 0..3 {
                if fx.mem.i16(t + 0x50) == kind {
                    let w = fx.mem.f(TINT_W);
                    quads4::tint4(fx, t, 0x21, w);
                }
            }
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xA0 => 1,
        0x154 => 4,
        0x136 => 2,
        0xAB..=0x130 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// What 0x204D80 with no matrix leaves below its caller's `sp` (as
/// [`crate::b590`]'s).
fn project_residue(fx: &mut Fx, sp: u32, v: u32) {
    fx.camera_vp_to(sp - 0x50);
    let m = fx.camera_vp();
    let p = [fx.mem.f(v), fx.mem.f(v + 4), fx.mem.f(v + 8), 1.0];
    let o = Fx::vu_apply(&m, p);
    let q = ee::div(1.0, o[3]);
    for k in 0..3u32 {
        fx.mem.setf(sp - 0x10 + 4 * k, ee::mul(o[k as usize], q));
    }
    fx.mem.setf(sp - 4, o[3]);
}

/// 0x1E01A20(D, 0): the update counter and the emitters (frame 0x30: ra,
/// the entry's s1 = D / s0 = 0; its own s1 = D, s0 the loop counter).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_2940), (0x10, u64::from(D)), (0, 0)]);
    let p = param as i64 as u64;
    let spawn = |fx: &mut Fx, f: u32, s0: u64, ra: u32| {
        task400_b4(fx, D, f, 0, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, u64::from(D)), (0, s0)]);
    };
    let node = |fx: &mut Fx, k: u32, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, slot, D + 0x250, 0, Some(u64::from(D)), ra);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    for (k, on, at, ra) in [(0u32, &[0x24, 0x4D, 0x64][..], 0x01E0_1AA0u32, 0x01E0_1ABCu32), (1, &[0x3B, 0x65][..], 0x01E0_1B24, 0x01E0_1B40)] {
        if on.contains(&c) {
            node(fx, k, at);
            for i in 0..0xDu64 {
                spawn(fx, MOTE, i, ra);
            }
        }
    }
    if c == 0x92 {
        node(fx, 2, 0x01E0_1BA0);
        spawn(fx, ARC, 0, 0x01E0_1BB4);
    }
    let c = fx.mem.i16(COUNTER);
    if (0xA6..0x119).contains(&c) {
        fx.mem.set16(KIND, c % 2);
        if fx.mem.i16(COUNTER) % 2 == 1 {
            spawn(fx, TINT, 0, 0x01E0_1C38);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x92..0x119).contains(&c) {
        spawn(fx, SWIRL, 0, 0x01E0_1C70);
        for i in 0..2u64 {
            let h = fx.target_handle();
            unit_position_saving(fx, h, D + 0x250, i, 0x01E0_1C8C);
            for (o, site) in [(0x250u32, 0x01E0_1C8Cu32), (0x254, 0x01E0_1CC0), (0x258, 0x01E0_1CF4)] {
                // rand's frame: s0 = the loop counter.
                let v = fx.rand_from(i as u32, site + 8);
                let r = fx.div(v, 0x14).1;
                let v = ee::add(fx.mem.f(D + o), ee::from_i32(r - 0xA));
                fx.mem.setf(D + o, v);
            }
            spawn(fx, RING, i, 0x01E0_1D38);
        }
    }
    fx.leave(0x30);
}

/// 0x1E01D80 (D, task): a white mote orbiting its node (frame 0x30: the
/// entry's s1 = D, s0 = 0): size 90, cell row (rand >> 4) % 3 + 2, angle
/// (rand >> 4) % 360, pushed out by 5.6, then by 0.8 a update; in by 0x40
/// under 0x80 for 10 updates, out by 9; turned by the camera's angles.
fn mote(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    let swing = |fx: &mut Fx, k: f32| {
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let z = ee::add(fx.mem.f(t + 0x18), ee::mul(k, s));
        fx.mem.setf(t + 0x18, z);
        let a = fx.mem.i16(t + 0x5A);
        let c = l2::sin_deg(fx, SINES, a, 1);
        let x = ee::add(fx.mem.f(t + 0x10), ee::mul(k, c));
        fx.mem.setf(t + 0x10, x);
    };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x5A);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_1DB8);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_1DF0);
        let (_, h) = fx.div(v >> 4, 0x168);
        fx.mem.set16(t + 0x5A, h);
        swing(fx, f32::from_bits(0x40B3_3333));
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xA {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        swing(fx, f32::from_bits(0x3F4C_CCCD));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1F28);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E01F50 (D, task): the arc (frame 0x50): outer radius 30 growing by
/// 10 below 100, inner blue (0x40, 0x40, 0xFF), outer white fading in by
/// 0x60 under 0xC0 for 20 updates, out by 9; at node 0x16 pushed twice as
/// far from node 0x15; angles 10 / 10; turned by the camera's angles (its
/// first + 0x4000).
fn arc(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x41F0_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0xFF);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 0x14 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a.wrapping_add(0x60));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x67, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES + 6);
    node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_202C);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES + 4);
    node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_204C);
    for k in 0..3u32 {
        let p = fx.mem.f(t + 0x10 + 4 * k);
        let v = ee::add(p, ee::mul(2.0, ee::sub(p, fx.mem.f(sp + 0x30 + 4 * k))));
        fx.mem.setf(t + 0x10 + 4 * k, v);
    }
    let r = fx.mem.f(t + 0x4C);
    if r < 100.0 {
        fx.mem.setf(t + 0x4C, ee::add(r, 10.0));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_20DC);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x50);
}

/// 0x1E02110 (D, task): a blue swirl arc (frame 0x30): radii 11, inner
/// blue, outer white, start +0x52 random, step 60, span 6, facing away
/// (+0x20 0x4000 / 0xC000); its height +0x34 -(15..44) sinking by 5 (by 5
/// x +0x38 5..14 from update 115) while the outer radius widens by 0.5 (by
/// 0.5 x +0x30 15..44); in by 0x60 under 0xC0 for 4 updates, out by 9.
fn swirl(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    let f = SWIRL;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4130_0000);
        fx.mem.write_u32(t + 0x48, 0x4130_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0, 0, 0xFF);
        let v = rmod(fx, t, f + 0x58, 0x168);
        fx.mem.set16(t + 0x52, v);
        fx.mem.set16(t + 0x5A, 0x3C);
        fx.mem.set16(t + 0x5C, 6);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, f + 0x8C, 0x1E);
        fx.mem.setf(t + 0x34, ee::from_i32(-(v + 0xF)));
        let v = rmod(fx, t, f + 0xBC, 0x1E);
        fx.mem.setf(t + 0x30, ee::from_i32(v + 0xF));
        let v = rmod(fx, t, f + 0xE8, 0xA);
        fx.mem.setf(t + 0x38, ee::from_i32(v + 5));
        let a = if fx.mem.i16(FACING) != 0 { 0x4000 } else { 0xC000 };
        fx.mem.set32(t + 0x20, a);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 4 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a.wrapping_add(0x60));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x67, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(COUNTER) < 0x73 {
        let h = ee::sub(fx.mem.f(t + 0x34), 5.0);
        fx.mem.setf(t + 0x34, h);
        let r = ee::add(fx.mem.f(t + 0x4C), 0.5);
        fx.mem.setf(t + 0x4C, r);
    } else {
        let h = ee::sub(fx.mem.f(t + 0x34), ee::mul(5.0, fx.mem.f(t + 0x38)));
        fx.mem.setf(t + 0x34, h);
        let r = ee::add(fx.mem.f(t + 0x4C), ee::mul(0.5, fx.mem.f(t + 0x30)));
        fx.mem.setf(t + 0x4C, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E02340 (D, task): a grey ring of quads: size 6..20, cell row 0..2,
/// radius 50..249, a random angle, +0xE 16..35, row 0..34, life 3..5; in by
/// 0x40 under 0xC0, out by 14.
fn ring(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    let f = RING;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x1C, 0xF);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 6));
        let v = rmod(fx, t, f + 0x48, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x74, 0xC8);
        fx.mem.set16(t + 0x50, v + 0x32);
        let v = rmod(fx, t, f + 0x94, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB0, 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, f + 0xD0, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xEC, 3);
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
    fx.leave(0x20);
}

/// 0x1E024D0 (D, task): a tint's colours by its kind +0x50 (0 red /
/// orange, 1 blue / violet, life 8; 2 grey, 22); both alphas in by 0x40
/// under 0x80 while young, then out by 4 (the task ends when the first is
/// below 6, the second is cleared).
fn tint(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        let kinds: [([i32; 3], [i32; 3], i32); 3] = [([0xF0, 0, 0x30], [0xF0, 0x30, 0], 8), ([0, 0x30, 0xF0], [0x30, 0, 0xF0], 8), ([0x58, 0x58, 0x78], [0x58, 0x58, 0x78], 0x16)];
        for (kind, (a, b, life)) in kinds.into_iter().enumerate() {
            if fx.mem.i16(t + 0x50) == kind as i32 {
                l2::rgb(fx, t + 0x60, a[0], a[1], a[2]);
                l2::rgb(fx, t + 0x64, b[0], b[1], b[2]);
                fx.mem.set16(t + 6, life);
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a.wrapping_add(0x40));
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 6 {
            fx.mem.write_u8(t + 0x63, a - 4);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x67);
        fx.mem.write_u8(t + 0x67, if a >= 6 { a - 4 } else { 0 });
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}
