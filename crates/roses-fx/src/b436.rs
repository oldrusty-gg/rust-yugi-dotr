//! BMODULE b436 (card 464), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05520, data area 0x1ED0000) [F] (helper C4, 31
//! Oct 2026): b642's family. From update 50 a sprite cloud (+0x1B90)
//! falling round the attacker's node 0x15 and respawned above it; from
//! update 80 a ribbon trail (+0x70) from node 0x15 toward the target (1 /
//! 36 of the way, ...) in a flickering colour, the frame quad, a ring
//! (+0x23F0) and a pulsing disc (+0x14D0) at the node; on updates 97..181
//! a turned plate (+0x1410) and a disc (+0x1B10) there; from update 114 a
//! ring (+0x16A0) swelling at the target, a spark fountain (+0x370)
//! respawned on a ring above 30 and a particle cloud (+0x14E0); from
//! update 130 a flickering disc (+0x1450) on the target. Returns 1 on
//! update 94, 6 on 104..235, 2 on 236, 8 after 260.
//!
//! Library: the frame quad (0x1E00120, [`crate::framegs`]),
//! `spawn::scatter` (0x1E00240), `trail::sparks` (0x1E00400)
//! [`crate::blib`]; `ring_setup` / `ring_draw` (0x1E01160 / 0x1E01420),
//! `disc` (0x1E01EA0), `sprites_spawn` (0x1E02400), `sprites`
//! (0x1E02590), `particle_quads` (0x1E03E80) [`crate::fxlib`];
//! `plate::plates` (0x1E01910); the ribbon (0x1E04A10 / 0x1E04B60).
use crate::b146::cloud_spawn;
use crate::b642::{disc_record, halves, ribbon_group};
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PLATES: u32 = 0x01E0_1910;

/// The entry 0x1E05520.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

/// 0x103540(p, floor, step) [F]: *p = floor (0) when below floor + step,
/// else *p -= step (1).
pub(crate) fn fstep_down(fx: &mut Fx, p: u32, floor: f32, step: f32) -> i32 {
    let v = fx.mem.f(p);
    if ee_lt(v, ee::add(floor, step)) {
        fx.mem.setf(p, floor);
        0
    } else {
        fx.mem.setf(p, ee::sub(v, step));
        1
    }
}

/// The spark fountain's sparks (`g`: count +0x2C, sparks from +0x30) grown
/// 1.1 a step and, above 30, respawned on a flat ring (as `spawn::scatter`
/// with flag 0x1D; radius modulus 10 x +0x28), life -(rand % 12): b642's
/// loop with the record's return addresses (`ras`: 0x1A2B80, three rands).
pub(crate) fn sparks_step(fx: &mut Fx, g: u32, ras: [u32; 4]) {
    let mut k = 0;
    while k < fx.mem.i16(g + 0x2C) {
        let p = g + 0x30 + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        let s = f32::from_bits(0x3F8C_CCCD);
        for c in 0..3u32 {
            let x = ee::mul(fx.mem.f(p + 4 * c), s);
            fx.mem.setf(p + 4 * c, x);
        }
        if ee_le(fx.mem.f(p + 4), 30.0) {
            continue;
        }
        let f = ee::mul(10.0, fx.mem.f(g + 0x28));
        fx.fix_residue(f, ras[0]);
        let w = ee::fixsfsi(f);
        let a = fx.rand_from(D, ras[1]) & 0xFFFF;
        let r = fx.rand_from(D, ras[2]);
        let (_, h) = fx.div(r, w);
        fx.mem.write_u32(p + 4, 0x4080_0000);
        let radius = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(radius, c));
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(radius, s));
        let r = fx.rand_from(D, ras[3]);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, h.wrapping_neg());
    }
}

/// A falling sprite cloud (`g`: count +0x5A, radius +0x4C, sprites +0x60
/// + 16 k) set off: life -(rand % 12), alpha 0x80, frame 0, 10 higher.
pub(crate) fn sprite_cloud_init(fx: &mut Fx, g: u32) {
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = g + 0x60 + 16 * k as u32;
        let r = fx.rand();
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, h.wrapping_neg());
        fx.mem.write_u8(p + 0xC, 0x80);
        fx.mem.write_u8(p + 0xD, 0);
        let y = ee::add(fx.mem.f(p + 4), 10.0);
        fx.mem.setf(p + 4, y);
        k += 1;
    }
}

/// The sprite cloud stepped: each live sprite 1 lower, drawn in by 0.8
/// while above 0, and below 0 respawned on the ring (angle rand, radius
/// +0x4C) at height 10 + rand % 10 (`ras`: the two rands' return
/// addresses).
pub(crate) fn sprite_cloud_step(fx: &mut Fx, g: u32, ras: [u32; 2]) {
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = g + 0x60 + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        let y = ee::sub(fx.mem.f(p + 4), 1.0);
        fx.mem.setf(p + 4, y);
        if !ee_le(y, 0.0) {
            let s = f32::from_bits(0x3F4C_CCCD);
            for o in [8u32, 0] {
                let v = ee::mul(fx.mem.f(p + o), s);
                fx.mem.setf(p + o, v);
            }
        }
        if !ee_lt(fx.mem.f(p + 4), 0.0) {
            continue;
        }
        let a = fx.rand_from(D, ras[0]) & 0xFFFF;
        let c = fx.tcos(a);
        let v = ee::mul(fx.mem.f(g + 0x4C), c);
        fx.mem.setf(p + 8, v);
        let s = fx.tsin(a);
        let v = ee::mul(fx.mem.f(g + 0x4C), s);
        fx.mem.setf(p, v);
        let r = fx.rand_from(D, ras[1]);
        let (_, h) = fx.div(r, 0xA);
        fx.mem.setf(p + 4, ee::add(10.0, ee::from_i32(h)));
    }
}

pub(crate) fn ring_group(fx: &mut Fx, g: u32, flags: u32, v: i32, bgra: [u8; 4]) {
    fx.mem.write_u32(g + 4, flags);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, v)]);
    for (k, x) in [(0xAu32, bgra[2]), (9, bgra[1]), (8, bgra[0]), (0xB, bgra[3])] {
        fx.mem.write_u8(g + k, x);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2820);
    for x in [0u32, 0x13A0, 0x300, 0x1630, 0x1AA0, 0x1460, 0x1B20] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2814u32, 0x16, 1), (0x2818, 0x15, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2804, 0x104);
    fx.mem.set32(D + 0x2808, 1);
    fx.mem.set32(D + 0x280C, param);
    fx.mem.set32(D + 0x2800, 0x24);
    fx.mem.set32(D + 0x2810, 0x80);
    disc_record(fx, D + 0x14D0, [0x80, 0x60, 0x60, 0xFF], 0x41A0_0000);
    disc_record(fx, D + 0x1450, [0x80, 0x60, 0x60, 0xFF], 0x40A0_0000);
    disc_record(fx, D + 0x1B10, [0x80, 0x80, 0x80, 0], 0x4000_0000);
    fx.texture5(sp + 0xB0, 0);
    let tex = fx.mem.read_u32(sp + 0xB0);
    ribbon_group(fx, D + 0x70, tex, 0, 0x42F0_0000, 0x6E);
    fx.mem.set16(D + 0x98, 0x26);
    // The spark fountain (+0x370): 256 sparks at +0x3A0, width 0.5,
    // length 3.
    let g = D + 0x370;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x2000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x4040_0000);
    halves(fx, g, &[(0x10, 0), (0x12, 0x20), (0x14, 0x20), (0x16, 0x20), (0x18, 0), (0x1A, 0x20), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, D + 0x3A0);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    // The plate (+0x1410): half sizes (0.5, 4) / (3, 0.5), rectangle (30,
    // 0, 32, 32).
    let g = D + 0x1410;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x8_0000);
    for k in [0xAu32, 9, 8] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 0xB, 0);
    for (k, v) in [(0xCu32, 0x3F00_0000u32), (0x10, 0x4080_0000), (0x14, 0x3F00_0000), (0x18, 0x4040_0000)] {
        fx.mem.write_u32(g + k, v);
    }
    halves(fx, g, &[(0x20, 0x1E), (0x22, 0), (0x24, 0x20), (0x26, 0x20)]);
    for k in [0x30u32, 0x34, 0x38] {
        fx.mem.write_u32(g + k, 0);
    }
    fx.texture4(sp + 0xB0, 0xF);
    // The particle cloud (+0x14E0): 16 particles, half size 20.
    let g = D + 0x14E0;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x85);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0xF0), (0x2A, 0), (0x24, 0x10), (0x26, 0x10), (0x2C, 0x100), (0x2E, 0x100)]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 1), (0x13C, 1), (0x140, 8), (0x136, 0xA), (0x138, 0xA)]);
    for (k, x) in [(0xAu32, 0x40u8), (9, 0x60), (8, 0x80), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, x);
    }
    cloud_spawn(fx, g);
    // The rings (+0x16A0: radii 11.2 / 12.8; +0x23F0: 2 / 6, width 8).
    ring_group(fx, D + 0x16A0, 0, 0x80, [0x80, 0x60, 0x60, 0x80]);
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.ring_setup(D + 0x16A0, tex, f32::from_bits(0x4133_3333), f32::from_bits(0x414C_CCCD), 0.0, 0.0);
    ring_group(fx, D + 0x23F0, 0x8000, 0x40, [0x80, 0x40, 0x40, 0x80]);
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.ring_setup(D + 0x23F0, tex, 2.0, 6.0, 0.0, 8.0);
    // The sprite cloud (+0x1B90): 128 sprites at +0x1BF0, size 1, radius
    // 12.
    let g = D + 0x1B90;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0001);
    for (k, x) in [(0xAu32, 0x60u8), (9, 0x60), (8, 0x80), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0xC, 0x3F80_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0xE4), (0x34, 0xA), (0x36, 0xA), (0x38, 0), (0x3A, 0xE4), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x1BF0);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4140_0000);
    halves(fx, g, &[(0x5A, 0x80), (0x5C, 2), (0x5E, 0)]);
    fx.sprites_spawn(g, 1.0);
    sprite_cloud_init(fx, g);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2808);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    node(fx, 0x2814, sp + 0x90);
    node(fx, 0x2818, sp + 0xA0);
    node(fx, 0x2818, D + 0x1B40);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x1B30);
    node(fx, 0x2818, D + 0x1AC0);
    for out in [D + 0x27F0, D + 0x1480, D + 0x320, D + 0x1650] {
        let t = fx.target_handle();
        fx.unit_position(t, out);
    }
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x90);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0xA0);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x310);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x1640);
    if n(fx) < 0x51 {
        node(fx, 0x2818, D + 0x13C0);
    }
    if n(fx) >= 0x50 {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x13C0);
        let m = fx.mem.i32(D + 0x2800);
        fx.approach_by(D + 0x13C0, D + 0x27F0, m);
        let m = fx.mem.i32(D + 0x2800);
        if m != 0 {
            fx.mem.set32(D + 0x2800, m - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 = sp + 0xA0.
    let regs = [Some(D), Some(OT), Some(sp + 0xA0), None];
    fx.xform_world_saving(D, 0x01E0_5C84, regs);
    fx.xform_billboard_saving(D + 0x13A0, 0x01E0_5C8C, regs);
    fx.xform_world_saving(D + 0x300, 0x01E0_5C94, regs);
    fx.xform_world_saving(D + 0x1630, 0x01E0_5C9C, regs);
    fx.xform_billboard_saving(D + 0x1AA0, 0x01E0_5CA4, regs);
    fx.xform_billboard_saving(D + 0x1460, 0x01E0_5CAC, regs);
    fx.xform_world_saving(D + 0x1B20, 0x01E0_5CB4, regs);
    if n(fx) >= 0x50 {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0xEC {
        fx.step_down(D + 0x2810, 0x20, 4);
    } else {
        fx.step_up(D + 0x2810, 0x80, 4);
    }
    let light = fx.mem.u8(D + 0x2810);
    fx.set_stage_light(light);
    if n(fx) >= 0x50 {
        fx.ring_draw(D + 0x23F0, D + 0x1B20);
        if n(fx) >= 0x64 {
            fx.step_down(D + 0x23FB, 0, 6);
        }
    }
    if n(fx) >= 0x32 {
        fx.sprites(D + 0x1B90, D + 0x1B20);
        fx.step_up(D + 0x1B9B, 0x80, 8);
        sprite_cloud_step(fx, D + 0x1B90, [0x01E0_5E0C, 0x01E0_5E50]);
    }
    if n(fx) >= 0x82 {
        fx.disc(D + 0x1450, D + 0x13A0);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_5EB0)]);
    }
    let m = n(fx);
    if m >= 0xEC {
        fx.step_down(D + 0x1457, 0, 0x10);
    } else {
        fx.mem.write_u32(D + 0x1458, if m % 2 == 0 { 0x40A0_0000 } else { 0x4040_0000 });
    }
    if n(fx) >= 0x50 {
        fx.disc(D + 0x14D0, D + 0x1AA0);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_5F2C)]);
        let m = n(fx);
        if m >= 0xEC {
            fx.step_down(D + 0x14D7, 0, 0x10);
        } else {
            fx.mem.write_u8(D + 0x14D7, if m % 2 == 0 { 0xFF } else { 0x80 });
        }
    }
    let m = n(fx);
    if (0x61..0xB6).contains(&m) {
        blib::plate::plates(fx, PLATES, OT, D + 0x1410, D + 0x1AA0);
        fx.disc(D + 0x1B10, D + 0x1AA0);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_5FC4)]);
        fx.step_up(D + 0x141B, 0x80, 8);
        fx.step_up(D + 0x1B17, 0x80, 8);
    }
    if n(fx) >= 0x72 {
        fx.ring_draw(D + 0x16A0, D + 0x1630);
        fx.step_down(D + 0x16AB, 0, 8);
        if fx.approach(D + 0x1630, 2.0, f32::from_bits(0x3E4C_CCCD)) == 0 && n(fx) < 0xEC {
            for k in [0x1638u32, 0x1634, 0x1630] {
                fx.mem.write_u32(D + k, 0x3DCC_CCCD);
            }
            fx.mem.write_u8(D + 0x16AB, 0x80);
        }
        blib::trail::sparks(fx, OT, D + 0x370, D + 0x300);
        if n(fx) >= 0xED {
            fx.step_down(D + 0x37B, 0, 8);
        }
        sparks_step(fx, D + 0x370, [0x01E0_6110, 0x01E0_6118, 0x01E0_6134, 0x01E0_6184]);
        for k in [0x88u32, 0x84, 0x80] {
            fx.mem.write_u32(sp + k, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0x1460, D + 0x14E0, sp + 0x80);
        if n(fx) >= 0xEC {
            fx.step_down(D + 0x14EB, 0, 2);
        }
    }
    if n(fx) >= 0x51 {
        for (k, ra) in [(0x76u32, 0x01E0_6218), (0x75, 0x01E0_6238), (0x74, 0x01E0_6258)] {
            let r = fx.rand_from(D, ra);
            fx.mem.write_u8(D + k, ((r & 0x3F) + 0x40) as u8);
        }
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
        if n(fx) >= 0xED {
            fstep_down(fx, D + 0x7C, 0.0, 4.0);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2808, m);
    let left = fx.mem.i32(D + 0x2804) - 1;
    fx.mem.set32(D + 0x2804, left);
    if m == 0xEC {
        2
    } else if m == 0x5E {
        1
    } else if (0x68..0xEC).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
