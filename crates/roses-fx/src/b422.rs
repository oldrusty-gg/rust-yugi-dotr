//! BMODULE b422 (card 520), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02020, data area 0x1ED0000) [F] (helper A11, 31
//! Oct 2026): b631's pool shape (400 tasks of 0x70 bytes at D+0x2A0, alive
//! +0x58, step +0x44 called through `jalr`, 0x1E02010 an idle step;
//! spawned by 0x1E02630 = `spawn::task400`) drawn with the second library:
//! white quads (add5d81dd3fe at 0x1E01230, `billboard::quad`), sparks
//! (the record's 0x1E027C0, [`spark`]), an arc (e6dce326551d at
//! 0x1E00ED0, `billboard::arc_strips`) and rings of quads (7369422df7f8 at
//! 0x1E00920, `billboard::ring_quads`; its texture D+0xB1F8 is never set),
//! UVs by uv_rect 0x1E00080. Update 1 the stage light task; 40..149 two
//! white quads a update at the attacker's node 1 (5 lower) drifting away;
//! 135 an orange arc ring and twenty grey rings at node 1 (8 lower), and to
//! 145 twenty sparks a update there. Returns 1 on update 125, 4 on 190, 2
//! on 165, 6 on 136..159, 8 once the 200-update timer +0xB210 has run out.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, spawn::task400, uv};
use crate::bus::Bus;
use crate::c061::{blend_alpha, desc_uv4, rgbaq_at};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const QUAD: u32 = 0x01E0_1790;
const SPARK: u32 = 0x01E0_1950;
const ARC: u32 = 0x01E0_1C60;
const RING: u32 = 0x01E0_1D50;
const LIGHT: u32 = 0x01E0_1F00;
const IDLE: u32 = 0x01E0_2010;
/// The return address of the entry's `jalr` into a step (0x1E02328 + 8).
const TASK_RA: u64 = 0x01E0_2330;
const POS: u32 = D + 0x250;
const AIM: u32 = D + 0xB1D0;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const NODES: u32 = D + 0xB244;
const FACING: u32 = D + 0xB2B4;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;
const SINES: u32 = D + 0xB4BC;
/// The scratchpad, where the originals project.
const SPR: u32 = 0x7000_0000;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02020.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0xC8);
    for (k, node) in [1, 0x34, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
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
    fx.unit_rotation(h, sp + 0x80);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    fx.fix_residue(y, 0x01E0_2210);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_2240, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_2294);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
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
        // The step's prologue: the entry's s0 = 0, s1 = D.
        let frame = match f {
            0 | IDLE => continue,
            QUAD | SPARK => 0x30,
            ARC | RING | LIGHT => 0x20,
            _ => panic!("b422: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        }
        match f {
            QUAD => quad(fx, t),
            SPARK => spark_step(fx, t),
            ARC => arc(fx, t),
            RING => ring(fx, t),
            _ => light(fx, t),
        }
        fx.leave(frame);
    }
    // The entry's s0: the live tasks counted in the draw loop.
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        let sub = |fx: &mut Fx| fx.mem.i16(t + 0x56);
        if fx.mem.read_u32(t + 0x44) == QUAD {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let s = sub(fx);
            crate::b615::cell(fx, sp + 0x50, 0x14, s);
            billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == SPARK {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            let s = sub(fx);
            crate::b615::cell(fx, sp + 0x60, 0x14, s);
            fx.mem.set16(D + 0xBE, 0);
            spark(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == RING {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            let s = sub(fx);
            crate::b615::cell(fx, sp + 0x70, 0x14, s);
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand_from(live, 0x01E0_2500);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x7D => 1,
        0xBE => 4,
        0xA5 => 2,
        0x88..=0x9F => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E014F0(D, 0): the update counter and the spawns (frame 0x30: the
/// entry's s1 = D, s0 = 0; then s1 = D, s0 a loop count). The spawns'
/// saves: the runner's s4 / s3 (0 [H]), the entry's s2 = the parameter,
/// the emitter's s1 = D and s0.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_22F0), (0x10, u64::from(D)), (0, 0)]);
    let p = param as i64 as u64;
    let go = |fx: &mut Fx, f: u32, ra: u32, s0: u64| {
        task400(fx, D, f, 0, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, u64::from(D)), (0, s0)]);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let node = |fx: &mut Fx, out: u32, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, out, 0, Some(u64::from(D)), ra);
    };
    let aim_to_pos = |fx: &mut Fx| {
        for k in 0..4 {
            let w = fx.mem.read_u32(AIM + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        let y = ee::sub(fx.mem.f(POS + 4), 8.0);
        fx.mem.setf(POS + 4, y);
    };
    if c(fx) == 1 {
        go(fx, LIGHT, 0x01E0_1544, 0);
    }
    if (0x28..0x96).contains(&c(fx)) {
        node(fx, POS, 0x01E0_1588);
        let y = ee::sub(fx.mem.f(POS + 4), 5.0);
        fx.mem.setf(POS + 4, y);
        go(fx, QUAD, 0x01E0_15B0, 0);
        go(fx, QUAD, 0x01E0_15C4, 0);
    }
    if c(fx) == 0x87 {
        node(fx, AIM, 0x01E0_1600);
    }
    if (0x87..0x92).contains(&c(fx)) {
        for k in 0..0x14u32 {
            aim_to_pos(fx);
            let v = rmod(fx, k, 0x01E0_1670, 0x14);
            let x = ee::add(fx.mem.f(POS), ee::from_i32(v - 0xA));
            fx.mem.setf(POS, x);
            go(fx, SPARK, 0x01E0_16AC, u64::from(k));
        }
    }
    if c(fx) == 0x87 {
        aim_to_pos(fx);
        go(fx, ARC, 0x01E0_1738, 0x14);
        for k in 0..0x14u64 {
            go(fx, RING, 0x01E0_1754, k);
        }
    }
    fx.leave(0x30);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// Byte `at` up by `up` while under `cap` before `life`, then down by
/// `down` while at least `floor` (else the task ends).
fn fade(fx: &mut Fx, t: u32, at: u32, life: i32, cap: u8, up: u8, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + at);
    if fx.mem.i16(t + 0x54) < life {
        if a < cap {
            fx.mem.write_u8(t + at, a.wrapping_add(up));
        }
    } else if a >= floor {
        fx.mem.write_u8(t + at, a - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E01790 (D, task): a white quad of size 150 (cell row (rand >> 4) %
/// 3 + 2), drifting 0.6 a update along a random angle +0x5A ((rand >> 4)
/// % 360) and 2 toward the facing; in by 0x60 under 0xC0 for 6 updates,
/// out by 0x13; turned by the camera's angles.
fn quad(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x96);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_17C8);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_1800);
        let (_, h) = fx.div(v >> 4, 0x168);
        fx.mem.set16(t + 0x5A, h);
    }
    fade(fx, t, 0x63, 6, 0xC0, 0x60, 0x15, 0x13);
    if fx.mem.i16(t + 0x54) != 0 {
        let by = f32::from_bits(0x3F19_999A);
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let z = ee::add(fx.mem.f(t + 0x18), ee::mul(by, s));
        fx.mem.setf(t + 0x18, z);
        let a = fx.mem.i16(t + 0x5A);
        let c = l2::sin_deg(fx, SINES, a, 1);
        let x = ee::add(fx.mem.f(t + 0x10), ee::mul(by, c));
        fx.mem.setf(t + 0x10, x);
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 2.0) } else { ee::sub(z, 2.0) };
        fx.mem.setf(t + 0x18, z);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1928);
    age(fx, t);
}

/// 0x1E01950 (D, task): a spark of size 10 x (10 growing by 40), cell row
/// (rand >> 4) % 3, drift (-50..49) / 20 set but unused, turned about x by
/// a random +0x20 (toward the facing: -(0..0x5FFF), else 0..0x5FFF; +0x24
/// set at random then cleared); its z offset +0xC += the y drift (as an
/// integer); in by 0x40 under 0x80 for 10 updates, out by 9.
fn spark_step(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xA);
        fx.mem.set16(t + 0xA, 0xA);
        fx.mem.set16(t + 0xC, 0);
        let v = fx.rand_from(t, 0x01E0_1988);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        for (o, site) in [(0x34u32, 0x01E0_19BCu32), (0x30, 0x01E0_19E8), (0x38, 0x01E0_1A14)] {
            let v = rmod(fx, t, site, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 20.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_1A6C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1A88);
        for (o, site) in [(4u32, 0x01E0_1A90u32), (2, 0x01E0_1AB0), (0, 0x01E0_1AD0)] {
            let v = fx.rand_from(t, site);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
        let v = fx.rand_from(t, 0x01E0_1AF0);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
        let (s1, s2, neg) = if fx.mem.i16(FACING) != 0 { (0x01E0_1B28, 0x01E0_1B48, true) } else { (0x01E0_1B70, 0x01E0_1B8C, false) };
        let v = rmod(fx, t, s1, 0x6000);
        fx.mem.set32(t + 0x20, if neg { v.wrapping_neg() } else { v });
        let v = rmod(fx, t, s2, 0xC000);
        fx.mem.set32(t + 0x24, v - 0x6000);
    }
    fade(fx, t, 0x63, 0xA, 0x80, 0x40, 0xB, 9);
    if fx.mem.i16(t + 0x54) != 0 {
        let h = fx.mem.i16(t + 0xA) + 0x28;
        fx.mem.set16(t + 0xA, h);
    }
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 0xC)), fx.mem.f(t + 0x38));
    fx.fix_residue(z, 0x01E0_1C28);
    fx.mem.set16(t + 0xC, ee::fixsfsi(z));
    fx.mem.write_u32(t + 0x24, 0);
    fx.mem.write_u32(t + 0x28, 0);
    age(fx, t);
}

/// 0x1E01C60 (D, task): the orange arc ring (radius 220, angles 10 / 10):
/// its outer alpha in by 0x60 under 0xC0 for 14 updates, out by 9; turned
/// by the camera's angles (the first + 0x4000).
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x435C_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x40);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x40);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0x67, 0xE, 0xC0, 0x60, 0xB, 9);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1D20);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E01D50 (D, task): a grey ring of quads: size 20..59, cell row 0..2,
/// radius 10..49 growing by 10..29, a random angle, row 0..34, life 7..14;
/// in by 0x40 under 0xC0, out by 14.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_1D74, 0x28);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x14));
        let v = rmod(fx, t, 0x01E0_1DA0, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_1DCC, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_1DEC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_1E08, 0x14);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = rmod(fx, t, 0x01E0_1E28, 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, 0x01E0_1E44);
        fx.mem.set16(t + 6, v % 8 + 7);
        fx.mem.set16(t + 0x58, 2);
    }
    let life = fx.mem.i16(t + 6);
    fade(fx, t, 0x63, life, 0xC0, 0x40, 0x10, 0xE);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E01F00 (D, task): the stage light (0x1F7F10) down by 10 to 0x30 for
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

/// 0x1E027C0 (928129938979, this record only) `spark(D, task, mode,
/// pos)`: one textured quad (9 values, FST) of the task's size i16 +8 /
/// +0xA turned about x by i16 +0x5A degrees (0x1099E0 axis 0 with D+0xD8's
/// sines) with its lower corners' y cleared, pushed by i16 +0xC in z, at
/// `pos` turned by the task's angles +0x20 (scale 0.1); coloured +0x60,
/// UVs from D+0x94 in D+0xBE's orientation, texture D+0x88; blend (mode >>
/// 4) & 15; keyed by its mean w into D+0x80 [F].
pub(crate) fn spark(fx: &mut Fx, uv: billboard::Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1B0);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x40 + 4 * k, w);
    }
    let d = sp + 0x118;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) & 0xF));
    rgbaq_at(fx, sp + 0x1A8, t + 0x60, d + 0x10);
    let order = fx.mem.i16(d0 + 0xBE);
    uv(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xA0 + 4 * k, 0x3DCC_CCCD);
        let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
        fx.mem.write_u32(sp + 0x90 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xC0, sp + 0xA0, t + 0x20, sp + 0x90);
    let (w, h) = (ee::from_i32(fx.mem.i16(t + 8)), ee::from_i32(fx.mem.i16(t + 0xA)));
    l2::quad_xy(fx, sp + 0x50, w, h);
    fx.mem.write_u32(sp + 0x100, 0);
    let a = fx.mem.i16(t + 0x5A);
    fx.mem.set32(sp + 0x104, a);
    fx.mem.write_u32(sp + 0x108, 0);
    l2::rotate4(fx, sp + 0x50, sp + 0x100, 0, d0 + 0xD8);
    fx.mem.write_u32(sp + 0x74, 0);
    fx.mem.write_u32(sp + 0x84, 0);
    let z = ee::from_i32(fx.mem.i16(t + 0xC));
    l2::add_column(fx, sp + 0x50, 2, z);
    if l2::project(fx, sp + 0x50, SPR, sp + 0xC0, d, sp + 0xB0) {
        let key = ee::fixunssfsi(fx.mem.f(sp + 0xB0)) as i32;
        let ot = fx.mem.read_u32(d0 + 0x80);
        fx.submit(d, ot, key);
    }
    fx.leave(0x1B0);
}
