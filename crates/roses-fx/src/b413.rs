//! BMODULE b413 (card 518), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03E30, data area 0x1ED0000, 0xB640 bytes) [F]
//!: b482's family (400 tasks of 0x70 bytes at
//! D+0x2A0, alive +0x58, step +0x44 called through `jalr`; a record-own
//! spawn 0x1E04730 with +0x50 / +0x52 from D+0xB2C4 and +0x5A from
//! D+0xB2B6) drawn with the second library (uv_rect, ring_quads,
//! arc_strips, quad, sprite_quads, sparks, quad_cross) and a lens layer
//! (0x1E04DE0, b388's 0x1E01CA0 with ALPHA 0x44) of flash discs (step
//! 0x1E03610, draw 0x1E037E0: 36 ten-degree quads of radius +0x48 in xy,
//! textured with the frame copy at their own screen places).
//! Update 1 the stage light; below 0x76 a red quad pair a update at nodes
//! 0x54 / 0x55; every third update below 0x53 two lens discs at node
//! 0x54 (+ 2 x kind) rising; on 0x16 / 0x1E rings and stars at nodes 0x20
//! / 0x2D; 0x6E..0x78 rings, sparks, quads and stars at nodes 0x20 / 0x2D
//! (taken on 0x6E), on 0x6E also four flares and an arc there. Returns 1
//! on update 0x64, 4 on 0x9B, 2 on 0x87, 6 on 0x6F..0x81, 8 once the
//! 0xA0-update timer +0xB220 has run out.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const FLARE: u32 = 0x01E0_23D0;
const RING: u32 = 0x01E0_2590;
const STAR_A: u32 = 0x01E0_2740;
const STAR_B: u32 = 0x01E0_29E0;
const PAIR: u32 = 0x01E0_2C70;
const QUAD: u32 = 0x01E0_2F40;
const SPARK: u32 = 0x01E0_3200;
const ARC: u32 = 0x01E0_33E0;
const LIGHT: u32 = 0x01E0_3500;
/// The lens discs, run by the lens 0x1E04DE0.
const DISC: u32 = 0x01E0_3610;
/// The return address of the entry's `jalr` into a step (0x1E04140 + 8).
const TASK_RA: u64 = 0x01E0_4148;
const POS: u32 = D + 0x250;
const AIM: u32 = D + 0xB1D0;
const TEX: u32 = D + 0xB200;
const TIMER: u32 = D + 0xB220;
const LAYER: u32 = D + 0xB234;
const SPRITE_PTR: u32 = D + 0xB238;
const QUAD_PTR: u32 = D + 0xB23C;
const NODES: u32 = D + 0xB258;
const ANGLE: u32 = D + 0xB2B6;
const KIND: u32 = D + 0xB2C4;
const FACING: u32 = D + 0xB2C8;
const COUNTER: u32 = D + 0xB2CC;
const BYTES: u32 = D + 0xB2CE;
const SINES: u32 = D + 0xB4D0;
const SPR: u32 = 0x7000_0000;

pub(crate) fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E03E30.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    fx.mem.write_u32(D + 0xB22C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB640);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2CA, param);
    fx.mem.set32(TIMER, 0xA0);
    for (k, node) in [0x54, 0x55, 0x20, 0x2D, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB268, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_4028);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_4058, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_40AC);
        fx.mem.set16(D + 0xB27A + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    // The entry's s0: the discs counted here (the steps' saved s0); s1 0.
    let mut discs = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        let frame = match f {
            0 => continue,
            DISC => {
                discs += 1;
                continue;
            }
            PAIR => 0x30,
            FLARE | RING | STAR_A | STAR_B | QUAD | SPARK | ARC | LIGHT => 0x20,
            _ => panic!("b413: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(discs))]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(discs))]);
        }
        match f {
            FLARE => flare(fx, t),
            RING => ring(fx, t),
            STAR_A => star(fx, t, false),
            STAR_B => star(fx, t, true),
            PAIR => pair(fx, t),
            QUAD => quad(fx, t),
            SPARK => spark(fx, t),
            ARC => arc(fx, t),
            _ => light(fx, t),
        }
        fx.leave(frame);
    }
    // The draws: s1 counts the live tasks.
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == FLARE {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            crate::b615::cell(fx, sp + 0x60, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
        }
        if f(fx) == RING {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            crate::b615::cell(fx, sp + 0x70, 0x14, sub);
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand_from(discs, 0x01E0_4278);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (g, local) in [(STAR_A, 0x80u32), (STAR_B, 0x90)] {
            if f(fx) != g {
                continue;
            }
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            crate::b615::cell(fx, sp + local, 0x9B, sub);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.set16(D + 0xB0, 1);
            billboard::quad_cross(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if f(fx) == PAIR {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xBE, 0);
            let sub = fx.mem.i16(t + 0x56);
            fx.mem.set16(D + 0x94, sub << 5);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x20);
            fx.mem.set16(D + 0x9A, 0x40);
            for (n, ra) in [(0u32, 0x01E0_4404u32), (2, 0x01E0_4484)] {
                let h = fx.self_handle();
                let slot = fx.mem.i16(NODES + n);
                node_position_saving(fx, h, slot, t + 0x10, u64::from(discs), Some(u64::from(live)), ra);
                let z = fx.mem.f(t + 0x18);
                let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 3.0) } else { ee::add(z, 3.0) };
                fx.mem.setf(t + 0x18, z);
                quad9(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
            }
        }
        if f(fx) == QUAD {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            crate::b615::cell(fx, sp + 0xA0, 0x14, sub);
            let o = fx.mem.i16(t + 0x52);
            fx.mem.set16(D + 0xBE, o);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if f(fx) == SPARK {
            let sub = fx.mem.i16(t + 0x56);
            crate::b615::cell(fx, sp + 0xB0, 0x97, sub);
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
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    if discs != 0 {
        lens(fx, discs, live);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x64 => 1,
        0x9B => 4,
        0x87 => 2,
        0x6F..=0x81 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E04730(D, step, 0): the first free task cleared and set off at
/// D+0x250 (i16 +0, floats +0x10) with the colour D+0xB224 (+0x60,
/// +0x64), +0x50 = +0x52 = D+0xB2C4, +0x5A = D+0xB2B6; 1 when one was
/// free.
pub(crate) fn spawn(fx: &mut Fx, step: u32) -> i32 {
    spawn_with(fx, step, 0xB224, 0xB2C4, 0xB2B6)
}

/// [`spawn`] on another layout: the colour D + `colour`, +0x50 / +0x52
/// from D + `kind`, +0x5A from D + `angle`.
pub(crate) fn spawn_with(fx: &mut Fx, step: u32, colour: u32, kind: u32, angle: u32) -> i32 {
    fx.enter(0x60);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            let c = D + colour;
            for o in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(c), fx.mem.read_u8(c + 1), fx.mem.read_u8(c + 2));
                l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(c + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(POS + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let s = fx.mem.i16(D + kind);
            fx.mem.set16(t + 0x50, s);
            let s = fx.mem.i16(D + kind);
            fx.mem.set16(t + 0x52, s);
            let a = fx.mem.i16(D + angle);
            fx.mem.set16(t + 0x5A, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

pub(crate) fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 0x1E01C60(D, 0): the update counter and the spawns (frame 0x40).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_4108), (0x20, u64::from(D)), (0x10, 0), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let many = |fx: &mut Fx, f: u32, n: u32| {
        for _ in 0..n {
            spawn(fx, f);
        }
    };
    if c(fx) < 0x76 {
        spawn(fx, PAIR);
    }
    let v = c(fx);
    if v % 3 == 1 && v < 0x53 {
        fx.mem.set16(KIND, 0);
        spawn(fx, DISC);
        fx.mem.set16(KIND, 1);
        spawn(fx, DISC);
    }
    if c(fx) == 1 {
        spawn(fx, LIGHT);
    }
    for (n, at, ra) in [(4u32, AIM, 0x01E0_1D9Cu32), (6, AIM + 0x10, 0x01E0_1DD8)] {
        if c(fx) == 0x6E {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + n);
            node_position_saving(fx, h, slot, at, 0, Some(0), ra);
        }
    }
    if (0x6E..0x79).contains(&c(fx)) {
        for at in [AIM, AIM + 0x10] {
            copy4(fx, POS, at);
            many(fx, RING, 5);
            many(fx, SPARK, 2);
            many(fx, QUAD, 2);
            fx.mem.set16(KIND, 0);
            many(fx, STAR_A, 2);
        }
        if c(fx) == 0x6E {
            for s1 in 0..2u32 {
                copy4(fx, POS, AIM + 0x10 * s1);
                for (a, k) in [(0, 0), (0x5A, 0), (0x2D, 1), (0x87, 1)] {
                    fx.mem.set16(ANGLE, a);
                    fx.mem.set16(KIND, k);
                    spawn(fx, FLARE);
                }
                spawn(fx, ARC);
            }
        }
    }
    let v = c(fx);
    if v == 0x16 || v == 0x1E {
        for (n, ra, s0) in [(4u32, 0x01E0_21ECu32, 0u64), (6, 0x01E0_22DC, 0xF)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + n);
            node_position_saving(fx, h, slot, POS, s0, Some(0), ra);
            let z = fx.mem.f(POS + 8);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
            fx.mem.setf(POS + 8, z);
            many(fx, RING, 0xA);
            fx.mem.set16(KIND, 0);
            many(fx, STAR_B, 0xF);
        }
    }
    fx.leave(0x40);
}

/// Byte `at` up by `up` while under `cap`.
pub(crate) fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends.
pub(crate) fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

pub(crate) fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// The drift +0x34 / +0x30 / +0x38 (-50..49 each, from `sites`) divided
/// by `by` (none for `None`).
pub(crate) fn drift(fx: &mut Fx, t: u32, sites: [u32; 3], by: Option<f32>) {
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
}

/// +4 / +2 / +0 random (-1024..1023), orientation +0x52 (rand >> 4) % 4.
pub(crate) fn spins(fx: &mut Fx, t: u32, sites: [u32; 4]) {
    for (o, site) in [(4u32, sites[0]), (2, sites[1]), (0, sites[2])] {
        let v = fx.rand_from(t, site);
        fx.mem.set16(t + o, v % 0x800 - 0x400);
    }
    let v = fx.rand_from(t, sites[3]);
    fx.mem.set16(t + 0x52, (v >> 4) % 4);
}

/// Random angles +0x20 / +0x24 / +0x28 (% 0xFFFF).
pub(crate) fn angles(fx: &mut Fx, t: u32, sites: [u32; 3]) {
    for (o, site) in [(0x20u32, sites[0]), (0x24, sites[1]), (0x28, sites[2])] {
        let v = rmod(fx, t, site, 0xFFFF);
        fx.mem.set32(t + o, v);
    }
}

/// 0x1E023D0 (D, task): a flare (sprite_quads): size +0x30 x +0x34
/// (kind 0: 40.5 x 470 growing by 30; 1: 40.5 x 350 by 10; 2: 8 x 400),
/// yellow; in by 0x40 under 0x80 for 4 updates, then out by 9; +0x52 up
/// to 25, turning by 6 a update.
fn flare(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x80);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0x40);
        for (k, w, h) in [(0, 0x4222_0000u32, 0x43EB_0000u32), (1, 0x4222_0000, 0x43AF_0000), (2, 0x4100_0000, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == k {
                fx.mem.write_u32(t + 0x30, w);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x52, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    for (k, by) in [(0, 30.0f32), (1, 10.0)] {
        if fx.mem.i16(t + 0x50) == k && fx.mem.i16(t + 0x54) != 0 {
            let h = ee::add(fx.mem.f(t + 0x34), by);
            fx.mem.setf(t + 0x34, h);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x52);
        if v < 0x19 {
            fx.mem.set16(t + 0x52, v + 1);
        }
    }
    age(fx, t);
    let a = fx.mem.i16(t + 0x5A) + 6;
    fx.mem.set16(t + 0x5A, a);
}

/// 0x1E02590 (D, task): a grey ring of quads: size 6..20, cell row 0..2,
/// radius 10..49 growing by +0xE (16..35), a random angle, row 0..34, life
/// 7..11; in by 0x40 under 0xC0, out by 14.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_25B4, 0xF);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 6));
        let v = rmod(fx, t, 0x01E0_25E0, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_260C, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_262C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2648, 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, 0x01E0_2668, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_2684, 5);
        fx.mem.set16(t + 6, v + 7);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x10, 0xE);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02740 (`b` false) / 0x1E029E0 (true) (D, task): a yellow star
/// (quad_cross, D+0xB0 = 1) 2 wide, 40 (10) tall, raised 0..4; drift
/// (-50..49) / 20 set but unused; random spins and angles; life 6: in by
/// 0x40 under 0x80, out by 9; growing by 6 tall and 3.0 raised (by 2 / 3).
fn star(fx: &mut Fx, t: u32, b: bool) {
    let o = if b { 0x2A0 } else { 0 };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 2);
        fx.mem.set16(t + 0xA, if b { 0xA } else { 0x28 });
        let v = rmod(fx, t, 0x01E0_2770 + o, 5);
        fx.mem.set16(t + 0xC, v);
        fx.mem.set16(t + 0x56, 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x40);
        fx.mem.set16(t + 0x58, 2);
        drift(fx, t, [0x01E0_27A8 + o, 0x01E0_27D4 + o, 0x01E0_2800 + o], Some(20.0));
        spins(fx, t, [0x01E0_2858 + o, 0x01E0_2878 + o, 0x01E0_2898 + o, 0x01E0_28B8 + o]);
        angles(fx, t, [0x01E0_28D8 + o, 0x01E0_28F4 + o, 0x01E0_2910 + o]);
        fx.mem.set16(t + 6, 6);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        if b {
            let h = fx.mem.i16(t + 0xA) + 2;
            fx.mem.set16(t + 0xA, h);
            let z = fx.mem.i16(t + 0xC) + 3;
            fx.mem.set16(t + 0xC, z);
        } else {
            let h = fx.mem.i16(t + 0xA) + 6;
            fx.mem.set16(t + 0xA, h);
            let z = ee::add(ee::from_i32(fx.mem.i16(t + 0xC)), 3.0);
            fx.fix_residue(z, 0x01E0_29B8);
            fx.mem.set16(t + 0xC, ee::fixsfsi(z));
        }
    }
    age(fx, t);
}

/// 0x1E02C70 (D, task): a red quad pair (0x1E048E0 at nodes 0x54 / 0x55)
/// 18 x 13, cell (rand >> 4) % 3; drift (-50..49) / 100 unused; in by
/// 0x40 under 0x80 for 10 updates, then out by 9; growing tall by 5
/// (below update 0x5A), 10 (below 0x79), else 60 and 3 wide; standing
/// across the facing (+0x20 = +-0x4000).
fn pair(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x12);
        fx.mem.set16(t + 0xA, 0xD);
        let v = fx.rand_from(t, 0x01E0_2CA8);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x58, 2);
        drift(fx, t, [0x01E0_2CDC, 0x01E0_2D08, 0x01E0_2D34], Some(100.0));
        let v = rmod(fx, t, 0x01E0_2D8C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2DA8);
        spins(fx, t, [0x01E0_2DB0, 0x01E0_2DD0, 0x01E0_2DF0, 0x01E0_2E10]);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let n = fx.mem.i16(COUNTER);
        let h = fx.mem.i16(t + 0xA);
        if n < 0x5A {
            fx.mem.set16(t + 0xA, h + 5);
        } else if n < 0x79 {
            fx.mem.set16(t + 0xA, h + 0xA);
        } else {
            fx.mem.set16(t + 0xA, h + 0x3C);
            let w = fx.mem.i16(t + 8) + 3;
            fx.mem.set16(t + 8, w);
        }
    }
    fx.mem.write_u32(t + 0x24, 0);
    fx.mem.write_u32(t + 0x28, 0);
    let a: i32 = if fx.mem.i16(FACING) != 0 { 0x4000 } else { -0x4000 };
    fx.mem.set32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02F40 (D, task): a dark red quad of size 100 drifting by
/// (-50..49) / 25 (kind 0) or / 45 (kind 1) a update, turned by the
/// camera's angles (b482's 0x1E02780 in other colours).
fn quad(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x64);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2F70);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h + 2);
        l2::rgb(fx, t + 0x60, 0x60, 0x30, 0x30);
        fx.mem.set16(t + 0x58, 2);
        let by = match fx.mem.i16(t + 0x50) {
            0 => Some(25.0),
            1 => Some(45.0),
            _ => None,
        };
        drift(fx, t, [0x01E0_2FA8, 0x01E0_2FD4, 0x01E0_3000], by);
        let v = rmod(fx, t, 0x01E0_30B4, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_30D0);
        spins(fx, t, [0x01E0_30D8, 0x01E0_30F8, 0x01E0_3118, 0x01E0_3138]);
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 6, 4);
    }
    for k in 0..3 {
        let p = ee::add(fx.mem.f(t + 0x10 + 4 * k), fx.mem.f(t + 0x30 + 4 * k));
        fx.mem.setf(t + 0x10 + 4 * k, p);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_31D8);
    age(fx, t);
}

/// 0x1E03200 (D, task): sparks (b482's 0x1E02A40 in dark colours): radius
/// 120 growing by (200..279) / 10 a update, the inner at 0.2 of it.
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42F0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(120.0, 58.0));
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0x50, 0x30, 0x30);
            l2::rgb(fx, t + 0x64, 8, 8, 8);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0x30, 0x20, 0x40);
            l2::rgb(fx, t + 0x64, 0x18, 0x18, 0x18);
        }
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 0x1E);
        let v = rmod(fx, t, 0x01E0_32B8, 0x50);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0xC8));
        let q = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, q);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        angles(fx, t, [0x01E0_3300, 0x01E0_331C, 0x01E0_3338]);
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

/// 0x1E033E0 (D, task): an arc ring of radius 50 growing by 0.4, inner
/// yellow, outer teal; outer alpha in by 0x30 under 0xC0 for 10 updates,
/// then out by 15; turned by the camera's angles (the first + 0x4000).
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4248_0000);
        l2::rgb(fx, t + 0x64, 0x20, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x67, 0xC0, 0x30);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_34A0);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x4C), f32::from_bits(0x3ECC_CCCD));
        fx.mem.setf(t + 0x4C, r);
    }
    age(fx, t);
}

/// 0x1E03500 (D, task): the stage light down by 10 to 0x50 for 131
/// updates, back up by 10 to 0x80 once the timer is under 20; 0x80 and the
/// task's end when the timer is at 3.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x83 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x51 { 0x50 } else { l - 0xA });
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
}

/// 0x1E03610 (D, task): a lens disc at node 0x54 + 2 x kind (+0x50),
/// pink, radius 2.0, moving along z by 0.9 a update with the facing (the
/// first z push +-7 into +0x34, the drawn height), a random start angle,
/// +0xA up by 6; from update 6 both alphas down by 29 (the task ends
/// below 31).
fn disc(fx: &mut Fx, t: u32, s: [u64; 2]) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_4F58), (0x10, s[1]), (0, s[0])]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.write_u8(t + 0x67, 0x80);
        l2::rgb(fx, t + 0x60, 0xC0, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0xC0, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_3688);
        let z: u32 = if fx.mem.i16(FACING) != 0 { 0x40E0_0000 } else { 0xC0E0_0000 };
        fx.mem.write_u32(t + 0x34, z);
        let v = rmod(fx, t, 0x01E0_36B8, 0x168);
        fx.mem.set16(t + 0x5A, v);
    }
    let h = fx.self_handle();
    let k = fx.mem.i16(t + 0x50) as u32;
    let slot = fx.mem.i16(NODES.wrapping_add(k.wrapping_mul(2)));
    node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_36FC);
    let step = f32::from_bits(0x3F66_6666);
    let z = fx.mem.f(t + 0x18);
    let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, step) } else { ee::add(z, step) };
    fx.mem.setf(t + 0x18, z);
    fx.mem.write_u32(t + 0x48, 0x4000_0000);
    let y = fx.mem.i16(t + 0xA) + 6;
    fx.mem.set16(t + 0xA, y);
    if fx.mem.i16(t + 0x54) >= 6 {
        fade_out(fx, t, 0x63, 0x1F, 0x1D);
        fade_out(fx, t, 0x67, 0x1F, 0x1D);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// One ST coordinate from a projected XYZ halfword: (base - v) / -16
/// clamped to 0..512, over 512.
pub(crate) fn st(v: u16, base: i32) -> f32 {
    let mut f = ee::div(ee::from_i32(base - i32::from(v)), -16.0);
    if ee_lt(f, 0.0) {
        f = 0.0;
    }
    if !ee_le(f, 512.0) {
        f = 512.0;
    }
    ee::div(f, 512.0)
}

/// 0x1E037E0(D, t): the disc as 36 quads (the lens descriptor at
/// D+0xB23C, colours +0x60 at height +0x34 / +0x64 at 0) of radius +0x48
/// in xy, ten degrees each from angle +0x5A, through the task's place
/// +0x10 x the camera (no rotation); ST = the vertices' own screen texels
/// in the frame copy (the first two's x by the task's i16 +0xA + 0x700),
/// unlinked.
fn disc_draw(fx: &mut Fx, t: u32, saves: &[(u32, u64)]) {
    let sp = fx.enter(0x130);
    fx.saved(sp + 0x10, saves);
    for (k, (c, at)) in [(0x60u32, 0x10u32), (0x60, 0x28), (0x64, 0x40), (0x64, 0x58)].into_iter().enumerate() {
        let tmp = sp + 0x110 + 8 * k as u32;
        l2::rgbaq(fx, tmp, t + c);
        let (a, b) = (fx.mem.read_u32(tmp), fx.mem.read_u32(tmp + 4));
        let q = fx.mem.read_u32(QUAD_PTR);
        fx.mem.write_u32(q + at, a);
        fx.mem.write_u32(q + at + 4, b);
    }
    for k in 0..3 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(sp + 0xC0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xD0, 0, 0, sp + 0xC0);
    let r = fx.mem.f(t + 0x48);
    let h = fx.mem.read_u32(t + 0x34);
    fx.mem.write_u32(sp + 0x88, h);
    fx.mem.write_u32(sp + 0x98, h);
    fx.mem.write_u32(sp + 0xA8, 0);
    fx.mem.write_u32(sp + 0xB8, 0);
    let mut s1 = 0i32;
    for _ in 0..0x24 {
        let s4 = (fx.mem.i16(t + 0x5A) + s1) as i16 as i32;
        let c = l2::sin_deg(fx, SINES, s4, 1);
        let s = l2::sin_deg(fx, SINES, s4, 0);
        let (x, y) = (ee::mul(c, r), ee::mul(s, r));
        fx.mem.setf(sp + 0xA0, x);
        fx.mem.setf(sp + 0xA4, y);
        fx.mem.setf(sp + 0x80, x);
        fx.mem.setf(sp + 0x84, y);
        let s4 = (s1 + (fx.mem.i16(t + 0x5A) + 0xA)) as i16 as i32;
        let c = l2::sin_deg(fx, SINES, s4, 1);
        let s = l2::sin_deg(fx, SINES, s4, 0);
        let (x, y) = (ee::mul(c, r), ee::mul(s, r));
        s1 = (s1 + 0xA) as i16 as i32;
        fx.mem.setf(sp + 0xB0, x);
        fx.mem.setf(sp + 0xB4, y);
        fx.mem.setf(sp + 0x90, x);
        fx.mem.setf(sp + 0x94, y);
        let q = fx.mem.read_u32(QUAD_PTR);
        if !l2::project(fx, sp + 0x80, SPR, sp + 0xD0, q, sp + 0x70) {
            continue;
        }
        let x_first = ((fx.mem.i16(t + 0xA) + 0x700) << 4) - 8;
        for (xyz, stq, xb) in [(0x20u32, 0x18u32, x_first), (0x38, 0x30, x_first), (0x50, 0x48, 0x6FF8), (0x68, 0x60, 0x6FF8)] {
            let q = fx.mem.read_u32(QUAD_PTR);
            let (x, y) = (fx.mem.read_u16(q + xyz), fx.mem.read_u16(q + xyz + 2));
            let (u, v) = (st(x, xb), st(y, 0x6FF8));
            fx.mem.setf(q + stq, u);
            let q = fx.mem.read_u32(QUAD_PTR);
            fx.mem.setf(q + stq + 4, v);
        }
        let q = fx.mem.read_u32(QUAD_PTR);
        fx.submit(q, 0, 0);
    }
    fx.leave(0x130);
}

/// 0x1E04DE0(D): the lens layer (frame 0x130: the sprite descriptor at
/// sp + 0x48, the quad at sp + 0xA0): TEXFLUSH, ALPHA 0x44, the frame
/// copied to 0x2095A0's TBP for the quad's TEX0 / TEX1 (0x207F50, point
/// sampled), TEXFLUSH; the disc tasks stepped and drawn; the layer at key
/// 0 (b388's 0x1E01CA0).
fn lens(fx: &mut Fx, discs: u32, live: u32) {
    let sp = fx.enter(0x130);
    fx.saved(sp, &[(0x30, 0x01E0_4664), (0x20, u64::from(D)), (0x10, u64::from(live)), (0, u64::from(discs))]);
    let (d, q) = (sp + 0x48, sp + 0xA0);
    fx.mem.write_u32(SPRITE_PTR, d);
    fx.mem.write_u32(QUAD_PTR, q);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u16(q + 2, 0x34);
    fx.mem.write_u16(q, 0xC);
    fx.mem.write_u64(q + 8, 0x4214_2142_1421);
    fx.mem.write_u32(q + 4, 0);
    fx.texflush([0xC, 0x34, d, 0x4_2421]);
    let old = u32::from(fx.mem.read_u16(q + 2));
    fx.desc_alpha(q, 0x80_0000_0044);
    let dest = fx.copy_dest([q, 0x44, old | 0x200, old]);
    let regs = fx.frame_texture_packet(dest, 0, q + 0x78, q + 0x80);
    fx.texflush(regs);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 || fx.mem.read_u32(t + 0x44) != DISC {
            continue;
        }
        let s = [u64::from(D), u64::from(k)];
        disc(fx, t, s);
        disc_draw(fx, t, &[(0x50, 0x01E0_4F64), (0x40, 0), (0x30, 0), (0x20, u64::from(D + 0x70 * k)), (0x10, u64::from(k)), (0, u64::from(D))]);
    }
    let layer = fx.mem.read_u32(LAYER);
    fx.layer_end(layer, OT, 0);
    fx.leave(0x130);
}

/// 0x1E048E0 (2318189970ee, cards 518 / 492) `quad9(D, task, mode,
/// pos)`: one textured quad (9 values, FST) of the task's size i16 +8 /
/// +0xA turned about x by i16 +0x5A degrees (0x1099E0 axis 0 with D+0xD8's
/// sines) with its lower corners' y cleared, at `pos` turned by the task's
/// angles +0x20 (scale 0.1); coloured +0x60, UVs from D+0x94 in D+0xBE's
/// orientation, texture D+0x88; blend (mode >> 4) & 15; keyed by its mean
/// w into D+0x80 (b422's 0x1E027C0 without the z push).
pub(crate) fn quad9(fx: &mut Fx, uv: billboard::Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    use crate::c061::{blend_alpha, desc_uv4, rgbaq_at};
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
    if l2::project(fx, sp + 0x50, SPR, sp + 0xC0, d, sp + 0xB0) {
        let key = ee::fixunssfsi(fx.mem.f(sp + 0xB0)) as i32;
        let ot = fx.mem.read_u32(d0 + 0x80);
        fx.submit(d, ot, key);
    }
    fx.leave(0x1B0);
}
