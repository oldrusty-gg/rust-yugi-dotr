//! BMODULE b400 (card 343), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E023A0, data area 0x1ED0000) [F]: a pool of 150 tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step
//! +0x44 called through `jalr`; spawned by b615's 0x1E02910 body,
//! `b615::spawn_at`) drawn with the second library: turned quads of a
//! float size (0a017ff6bc9f at 0x1E00ED0, `quads4::quad_turned_f`), rings
//! of quads (7369422df7f8 at 0x1E00920, `billboard::ring_quads`), an arc
//! (e6dce326551d at 0x1E011A0, `billboard::arc_strips`) and a full-window
//! gouraud tint (561a6131a635 at 0x1E01500, `quads4::tint4`), UVs by
//! uv_rect 0x1E00080. The attacker's node 0x36 (until update 154, then
//! moving to the target's node 1 by 1/25 of the way a update) carries a
//! growing pair of dark quads and grey rings from update 18 to 176; on
//! update 176 there: an arc, a tint (keyed by a point 60 off the attacker)
//! and twenty rings. Returns 1 on update
//! 168, 4 on 220, 2 on 195, 6 on 179..189, 8 once the 225-update timer
//! +0x44B0 has run out.
use crate::b494::camera_angles_saving;
use crate::b615::{cell, rmod, spawn_at};
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const QUAD: u32 = 0x01E0_1970;
const RING: u32 = 0x01E0_1BA0;
const TINT: u32 = 0x01E0_1D30;
const ARC: u32 = 0x01E0_1E30;
const QUADS: u32 = 0x01E0_1F80;
const QUADS_B: u32 = 0x01E0_2190;
/// A step that only returns 0 (counted by the entry, not called).
const IDLE: u32 = 0x01E0_2390;
/// The return address of the entry's `jalr` into a step (0x1E02688 + 8).
const TASK_RA: u64 = 0x01E0_2690;
/// The carried point (vec4) and its step towards the target.
const POINT: u32 = D + 0x4470;
const TEX: u32 = D + 0x4490;
const TIMER: u32 = D + 0x44B0;
const COLOUR: u32 = D + 0x44B4;
/// The quads' size limit (grows by 2 to 100).
const LIMIT: u32 = D + 0x44D4;
const TINT_W: u32 = D + 0x44D8;
const NODES: u32 = D + 0x44EC;
const SLOT: u32 = D + 0x451A;
const FACING: u32 = D + 0x451E;
const COUNTER: u32 = D + 0x4522;
const BYTES: u32 = D + 0x4524;
const BYTES2: u32 = D + 0x4624;

/// The entry 0x1E023A0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x130);
    fx.mem.write_u32(D + 0x44BC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x130);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4890);
    l2::sine_table(fx, D + 0x4724);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4520, param);
    fx.mem.set32(TIMER, 0xE1);
    for (k, node) in [0x36, 0x34, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x44FC, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3 {
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
        let v = fx.rand_from(D, 0x01E0_2564);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_25B4);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_2604);
        let (_, h) = fx.div(v, 0x28);
        fx.mem.set8(BYTES2 + k, h - 0x14);
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
            QUAD => quad(fx, t),
            RING => ring(fx, t),
            TINT => tint(fx, t),
            ARC => arc(fx, t),
            QUADS | QUADS_B => quads(fx, t, f),
            _ => panic!("b400: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == QUAD {
            let i = fx.mem.i16(t + 0x50) as u32;
            let w = fx.mem.read_u32(TEX.wrapping_add(4 * i));
            fx.mem.write_u32(D + 0x88, w);
            whole(fx);
            let o = fx.mem.i16(t + 0x56);
            fx.mem.set16(D + 0xBE, o);
            quads4::quad_turned_f(fx, uv::uv_rect, D, t, 0x21, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == RING {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x14, sub);
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand_from(D, 0x01E0_27A8);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == TINT {
            let h = fx.self_handle();
            fx.unit_position(h, sp + 0x60);
            // 0x1F71A0's frame below the entry's (its saved ra, s0 = D).
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_2808), (0, u64::from(D))]);
            let z = fx.mem.f(sp + 0x68);
            let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 60.0) } else { ee::sub(z, 60.0) };
            fx.mem.setf(sp + 0x68, z);
            let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
            let mut x = fx.mem.read_u64(sp + 0xB8);
            if let Some(w) = fx.project(p, None, &mut x) {
                fx.mem.write_u64(sp + 0xB8, x);
                fx.mem.setf(TINT_W, w);
            }
            project_residue(fx, sp, sp + 0x60);
            let w = fx.mem.f(TINT_W);
            quads4::tint4(fx, t, 0x21, w);
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, POINT);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == QUADS {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x14, sub);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if fx.mem.read_u32(t + 0x44) == QUADS_B {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            whole(fx);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 1);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xA8 => 1,
        0xDC => 4,
        0xC3 => 2,
        0xB3..=0xBD => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The whole texture: UV (0, 0) size (127, 127).
fn whole(fx: &mut Fx) {
    for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x7F)] {
        fx.mem.set16(D + o, v);
    }
}

/// What 0x204D80 with no matrix leaves below its caller's `sp`: the
/// camera's view-projection at sp - 0x50 and its local vec at sp - 0x10
/// (as [`crate::b590`]'s).
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

/// 0x1E016D0(D, 0): the update counter, the carried point and the
/// emitters (frame 0x50: ra, the entry's s1 = 0 / s0 = D; its own s0 = D,
/// s1 the loop counter).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_2650), (0x10, 0), (0, u64::from(D))]);
    // The spawn's prologue saves the entry's s3 = the parameter, s2 = 0
    // and the emitter's s1 / s0 = D.
    let p = param as i64 as u64;
    let spawn = |fx: &mut Fx, f: u32, s1: u64, ra: u32| {
        spawn_at(fx, f, COLOUR, SLOT, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, p), (0x20, 0), (0x10, s1), (0, u64::from(D))]);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    if c < 0x9B {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        fx.node_position(h, slot, POINT);
    } else {
        if c == 0x9B {
            let h = fx.target_handle();
            let slot = fx.mem.i16(D + 0x44FC);
            fx.node_position(h, slot, sp + 0x30);
            let dz = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(POINT + 8)), 25.0);
            fx.mem.setf(D + 0x4488, dz);
            let dy = ee::div(ee::sub(fx.mem.f(sp + 0x34), fx.mem.f(POINT + 4)), 25.0);
            fx.mem.setf(D + 0x4484, dy);
        }
        if fx.mem.i16(COUNTER) < 0xB0 {
            let z = ee::add(fx.mem.f(POINT + 8), fx.mem.f(D + 0x4488));
            fx.mem.setf(POINT + 8, z);
            let y = ee::add(fx.mem.f(POINT + 4), fx.mem.f(D + 0x4484));
            fx.mem.setf(POINT + 4, y);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x12..0xB1).contains(&c) {
        fx.mem.set16(SLOT, 0);
        spawn(fx, QUAD, 0, 0x01E0_17D8);
        fx.mem.set16(SLOT, 1);
        spawn(fx, QUAD, 0, 0x01E0_17F4);
        for k in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        spawn(fx, RING, 0, 0x01E0_1828);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x14..0xB1).contains(&c) {
        let l = fx.mem.f(LIMIT);
        if l < 100.0 {
            fx.mem.setf(LIMIT, ee::add(l, 2.0));
        }
    }
    if fx.mem.i16(COUNTER) == 0xB0 {
        for k in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        fx.mem.set16(SLOT, 0);
        for i in 0..0xAu64 {
            spawn(fx, QUADS_B, i, 0x01E0_18C8);
        }
        for i in 0..0xAu64 {
            spawn(fx, QUADS, i, 0x01E0_1908);
        }
        spawn(fx, TINT, 0xA, 0x01E0_1940);
        spawn(fx, ARC, 0xA, 0x01E0_1954);
    }
    fx.leave(0x50);
}

/// The step frame (0x20: ra, the entry's s0 = D).
fn frame20(fx: &mut Fx) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
}

/// 0x1E01970 (D, task): a dark quad carried with the point for 15
/// updates (frame 0x30): size +0x30 / +0x34 growing by 3 (5 / 2.5 from
/// update 140) up to the limit D+0x44D4, cell row (rand >> 4) % 8, its
/// texture by the spawn's slot +0x50; in by 10 under 0x80 for 15 updates,
/// out by 8; turned by the camera's angles; aging twice as fast from
/// update 178.
fn quad(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        let v = fx.rand_from(t, 0x01E0_19A0);
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0x10, 0x10, 0x10);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.set16(t + 0x52, 0x32);
        fx.mem.set16(t + 0x5A, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xF {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0xA);
        }
    } else if a >= 0xA {
        fx.mem.write_u8(t + 0x63, a - 8);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let early = fx.mem.i16(COUNTER) < 0x8C;
        let (dw, dh) = if early { (3.0f32, 3.0f32) } else { (5.0, 2.5) };
        for (o, d) in [(0x30u32, dw), (0x34, dh)] {
            let lim = fx.mem.f(LIMIT);
            let v = fx.mem.f(t + o);
            if !(lim <= v) {
                fx.mem.setf(t + o, ee::add(v, d));
            }
        }
    }
    if fx.mem.i16(t + 0x54) < 0xF {
        for k in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * k);
            fx.mem.write_u32(t + 0x10 + 4 * k, w);
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1B50);
    let step = if fx.mem.i16(COUNTER) < 0xB2 { 1 } else { 2 };
    let n = fx.mem.i16(t + 0x54) + step;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E01BA0 (D, task): a grey ring of quads: size 10..19, cell row 0..2,
/// radius 10..49, a random angle, +0xE 16..35, row 0..34, life 3..5; in by
/// 0x40 under 0xC0, out by 14.
fn ring(fx: &mut Fx, t: u32) {
    frame20(fx);
    let f = RING;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x1C, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = rmod(fx, t, f + 0x48, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x74, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
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
            fx.mem.write_u8(t + 0x63, a + 0x40);
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

/// 0x1E01D30 (D, task): the tint's colours: grey over red (0x80, 0x40,
/// 0x40), life 12; both alphas in by 0x2A under 0x80, then out by 12 (the
/// task ends when the first is below 14, the second is cleared).
fn tint(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x40, 0x40);
        fx.mem.set16(t + 6, 0xC);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xE {
            fx.mem.write_u8(t + 0x63, a - 0xC);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x67);
        fx.mem.write_u8(t + 0x67, if a >= 0xE { a - 0xC } else { 0 });
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01E30 (D, task): the arc: outer radius 300, outer white fading in
/// by 0x60 under 0xC0 for 12 updates, out by 11; inner (0xFF, 0x80, 0x80)
/// on odd ages, (0xC0, 0x80, 0x80) on even; angles 10 / 10; turned by the
/// camera's angles (its first + 0x4000).
fn arc(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4396_0000);
        fx.mem.write_u32(t + 0x4C, 0x4396_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 0xC {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a + 0x60);
        }
    } else if a >= 0xD {
        fx.mem.write_u8(t + 0x67, a - 0xB);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
    } else {
        l2::rgb(fx, t + 0x60, 0xC0, 0x80, 0x80);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1F48);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01F80 (`QUADS`: white, size 15..24, radius 7..26, life 10..14,
/// alpha 0xF0) / 0x1E02190 (`QUADS_B`: red (0xC0, 0x50, 0x50), size
/// 10..19, radius 200..399, life 5..9, alpha 0x60) (D, task): a ring of
/// quads, cell row 0..3, random angles +0x20..+0x28, +0x52 counting from
/// 1, the radius growing by +0xE 10..29 (`QUADS` from age 1), row 1..8;
/// out by 11 after its life.
fn quads(fx: &mut Fx, t: u32, f: u32) {
    frame20(fx);
    let b = f == QUADS_B;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x1C, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + if b { 0xA } else { 0xF }));
        let v = fx.rand_from(t, f + 0x50);
        fx.mem.set16(t + 0x56, v % 4);
        if b {
            l2::rgb(fx, t + 0x60, 0xC0, 0x50, 0x50);
        } else {
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        }
        for (o, site) in [(0x20u32, 0x7Cu32), (0x24, 0x98), (0x28, 0xB4)] {
            let v = rmod(fx, t, f + site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x52, 1);
        let v = if b { rmod(fx, t, f + 0xD8, 0xC8) + 0xC8 } else { rmod(fx, t, f + 0xD8, 0x14) + 7 };
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, t, f + 0xF8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0x114, 0x14);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = fx.rand_from(t, f + 0x13C);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, f + 0x154, 5);
        fx.mem.set16(t + 6, v + if b { 5 } else { 0xA });
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, if b { 0x60 } else { 0xF0 });
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xD {
            fx.mem.write_u8(t + 0x63, a - 0xB);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let c = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, c);
    if b || fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}
