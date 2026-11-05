//! BMODULE b642 (card 504), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03490, data area 0x1ED0000) [F] (helper C3, 31
//! Oct 2026). Two turned plates (+0x2220 / +0x22D0, spinning 0x200 a step
//! until update 50) with a disc each (+0x29C0 / +0x2A40) at the
//! attacker's nodes 0xD / 0xE, faded in on 12..27 and out on 28..43, drawn
//! until update 148; from update 78 two ribbon trails (+0x70 / +0xA0)
//! from those nodes to the target (1 / 26 of the way per update, then 1 /
//! 25 ...); a flickering disc (+0x2380) on the target; from update 114 a
//! spark fountain (+0x1180) respawned on a ring when above 30 and a
//! particle cloud (+0x2390) swelling at the target. Returns 1 on update
//! 88, 6 on 98..223, 2 on 224, 8 after 260.
//!
//! Library: `spawn::scatter` (0x1E00120), `trail::sparks` (0x1E002E0)
//! [`crate::blib`]; `ring_setup` (0x1E01040; the ring +0x2550 is set up
//! and faded but never drawn), `disc` (0x1E01890), `particle_quads`
//! (0x1E01DF0) [`crate::fxlib`]; `plate::plates` (0x1E01300); the ribbon
//! (0x1E02980 / 0x1E02AD0, [`crate::ribbon`]).
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PLATES: u32 = 0x01E0_1300;

/// The entry 0x1E03490.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

pub(crate) fn disc_record(fx: &mut Fx, at: u32, bgra: [u8; 4], radius: u32) {
    fx.mem.write_u32(at, 0x8_0000);
    for (k, x) in [(6u32, bgra[2]), (5, bgra[1]), (4, bgra[0]), (7, bgra[3])] {
        fx.mem.write_u8(at + k, x);
    }
    fx.mem.write_u32(at + 8, radius);
    fx.mem.set16(at + 0xC, 0x24);
    fx.mem.set16(at + 0xE, 0);
}

pub(crate) fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

/// A ribbon group (cell x `u`, width `width`, life `life`, 32 slots).
pub(crate) fn ribbon_group(fx: &mut Fx, g: u32, tex: u32, u: i32, width: u32, life: i32) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 8, 0x8003);
    for k in [6u32, 5, 4, 7] {
        fx.mem.write_u8(g + k, 0x80);
    }
    halves(fx, g, &[(0x10, u), (0x12, 0), (0x14, 0x10), (0x16, 0x20), (0x18, u), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40), (0x22, 8)]);
    fx.mem.write_u32(g + 0xC, width);
    halves(fx, g, &[(0x24, 0), (0x26, life), (0x28, 0x20), (0x2A, 0), (0x2C, 1)]);
}

/// A plate group (`plate::plates`; rectangle x `u`).
pub(crate) fn plate_group(fx: &mut Fx, g: u32, tex: u32, u: i32) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x8_0000);
    for k in [0xAu32, 9, 8] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 0xB, 0);
    for (k, v) in [(0xCu32, 0x3E80_0000u32), (0x10, 0x4040_0000), (0x14, 0x3E80_0000), (0x18, 0x3F80_0000)] {
        fx.mem.write_u32(g + k, v);
    }
    halves(fx, g, &[(0x20, u), (0x22, 0x20), (0x24, 0x20), (0x26, 0x20)]);
    for k in [0x30u32, 0x34, 0x38] {
        fx.mem.write_u32(g + k, 0);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2A80);
    for x in [0u32, 0x21B0, 0x2260, 0x1110, 0x24E0, 0x2950, 0x29D0, 0x2310] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2A70u32, 0xD, 1), (0x2A74, 0xE, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2A64, 0x104);
    fx.mem.set32(D + 0x2A68, 1);
    fx.mem.set32(D + 0x2A6C, param);
    fx.mem.set32(D + 0x2A60, 0x1A);
    disc_record(fx, D + 0x2380, [0x30, 0x80, 0xA0, 0xFF], 0x4120_0000);
    disc_record(fx, D + 0x29C0, [0xA0, 0x70, 0xD0, 0], 0x4000_0000);
    disc_record(fx, D + 0x2A40, [0xA0, 0x70, 0xD0, 0], 0x4000_0000);
    fx.texture5(sp + 0xB0, 0);
    for g in [0x70u32, 0xA0] {
        let tex = fx.mem.read_u32(sp + 0xB0);
        ribbon_group(fx, D + g, tex, 0x30, 0x41A0_0000, 0x64);
    }
    // The spark fountain (+0x1180): 256 sparks at +0x11B0, width 0.5,
    // length 3.
    let g = D + 0x1180;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x2000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x4040_0000);
    halves(fx, g, &[(0x10, 0x60), (0x12, 0x20), (0x14, 0x20), (0x16, 0x20), (0x18, 0x60), (0x1A, 0x20), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, D + 0x11B0);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    for g in [0x2220u32, 0x22D0] {
        let tex = fx.mem.read_u32(sp + 0xB0);
        plate_group(fx, D + g, tex, 0x60);
    }
    fx.texture4(sp + 0xB0, 0xF);
    // The particle cloud (+0x2390): 16 particles, half size 20.
    let g = D + 0x2390;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x85);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0xF0), (0x2A, 0), (0x24, 0x10), (0x26, 0x10), (0x2C, 0x100), (0x2E, 0x100)]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 1), (0x13C, 1), (0x140, 8), (0x136, 0xA), (0x138, 0xA)]);
    for (k, x) in [(0xAu32, 0xD0u8), (9, 0x70), (8, 0xA0), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, x);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x24C0) {
        let p = D + 16 * k as u32;
        for c in 0..2u32 {
            let r = fx.rand();
            let (_, h) = fx.div(r, 0x14);
            let v = ee::add(fx.mem.f(D + 0x23A0 + 4 * c), ee::from_i32(h - 10));
            fx.mem.setf(p + 0x23C0 + 4 * c, v);
        }
        let z = fx.mem.read_u32(D + 0x23A8);
        fx.mem.write_u32(p + 0x23C8, z);
        let a = -i32::from(fx.mem.i16(D + 0x24CA));
        let f = fx.mult(a, k as i32);
        fx.mem.set16(p + 0x23CE, f);
        fx.mem.set16(p + 0x23CC, 0);
        let t = fx.mem.i16(D + 0x24D0) - 1;
        fx.mem.set16(D + 0x24CE, t as i32);
        k += 1;
    }
    // The ring (+0x2550).
    let g = D + 0x2550;
    fx.mem.write_u32(g + 4, 0);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
    for (k, x) in [(0xAu32, 0xD0u8), (9, 0x70), (8, 0xA0), (0xB, 0x80)] {
        fx.mem.write_u8(g + k, x);
    }
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.ring_setup(g, tex, f32::from_bits(0x4133_3333), f32::from_bits(0x414C_CCCD), 0.0, 0.0);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2A68);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    node(fx, 0x2A70, D + 0x2970);
    node(fx, 0x2A74, D + 0x29F0);
    for out in [D + 0x2A50, D + 0x2330, D + 0x1130, D + 0x2500] {
        let t = fx.target_handle();
        fx.unit_position(t, out);
    }
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x90);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0xA0);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x1120);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x24F0);
    if n(fx) < 0x4F {
        node(fx, 0x2A70, D + 0x21D0);
        node(fx, 0x2A74, D + 0x2280);
    }
    if n(fx) >= 0x4E {
        fx.ribbon_push(D + 0x70, D + 0xD0, D + 0x21D0);
        fx.ribbon_push(D + 0xA0, D + 0x8F0, D + 0x2280);
        for p in [0x21D0u32, 0x2280] {
            let m = fx.mem.i32(D + 0x2A60);
            fx.approach_by(D + p, D + 0x2A50, m);
        }
        let m = fx.mem.i32(D + 0x2A60);
        if m != 0 {
            fx.mem.set32(D + 0x2A60, m - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 = sp + 0xA0.
    let regs = [Some(D), Some(OT), Some(sp + 0xA0), None];
    fx.xform_world_saving(D, 0x01E0_3B44, regs);
    fx.xform_billboard_saving(D + 0x21B0, 0x01E0_3B4C, regs);
    fx.xform_billboard_saving(D + 0x2260, 0x01E0_3B54, regs);
    fx.xform_world_saving(D + 0x1110, 0x01E0_3B5C, regs);
    fx.xform_world_saving(D + 0x24E0, 0x01E0_3B64, regs);
    fx.xform_billboard_saving(D + 0x2950, 0x01E0_3B6C, regs);
    fx.xform_billboard_saving(D + 0x29D0, 0x01E0_3B74, regs);
    fx.xform_billboard_saving(D + 0x2310, 0x01E0_3B7C, regs);
    fx.disc(D + 0x2380, D + 0x2310);
    // The disc's fp / ra saves (frame 0x140).
    fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_3B8C)]);
    let m = n(fx);
    if m >= 0xE0 {
        fx.step_down(D + 0x2387, 0, 0x10);
    } else {
        fx.mem.write_u8(D + 0x2387, if m % 2 == 0 { 0x80 } else { 0x40 });
    }
    let m = n(fx);
    if m < 0x95 {
        if m == 0x4C {
            for k in [0x2A47u32, 0x29C7, 0x22DB, 0x222B] {
                fx.mem.write_u8(D + k, 0x80);
            }
        }
        blib::plate::plates(fx, PLATES, OT, D + 0x2220, D + 0x2950);
        blib::plate::plates(fx, PLATES, OT, D + 0x22D0, D + 0x29D0);
        fx.disc(D + 0x29C0, D + 0x2950);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_3C48)]);
        fx.disc(D + 0x2A40, D + 0x29D0);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_3C58)]);
        let m = n(fx);
        if (0xC..0x1C).contains(&m) {
            for k in [0x222Bu32, 0x22DB, 0x29C7, 0x2A47] {
                fx.step_up(D + k, 0x80, 8);
            }
        } else if (0x1C..0x2C).contains(&m) {
            for k in [0x222Bu32, 0x22DB, 0x29C7, 0x2A47] {
                fx.step_down(D + k, 0, 8);
            }
        }
    }
    if n(fx) < 0x32 {
        for k in [0x2258u32, 0x2308] {
            let v = fx.mem.i32(D + k).wrapping_add(0x200);
            fx.mem.set32(D + k, v);
        }
    } else {
        fx.mem.write_u32(D + 0x2308, 0);
        fx.mem.write_u32(D + 0x2258, 0);
    }
    if n(fx) >= 0x72 {
        fx.step_down(D + 0x255B, 0, 8);
        if fx.approach(D + 0x24E0, 2.0, f32::from_bits(0x3E4C_CCCD)) == 0 && n(fx) < 0xE0 {
            for k in [0x24E8u32, 0x24E4, 0x24E0] {
                fx.mem.write_u32(D + k, 0x3DCC_CCCD);
            }
            fx.mem.write_u8(D + 0x255B, 0x80);
        }
        blib::trail::sparks(fx, OT, D + 0x1180, D + 0x1110);
        if n(fx) >= 0xE1 {
            fx.step_down(D + 0x118B, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x11AC) {
            let p = D + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0x11BE) < 0 {
                continue;
            }
            let s = f32::from_bits(0x3F8C_CCCD);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 0x11B0 + 4 * c), s);
                fx.mem.setf(p + 0x11B0 + 4 * c, x);
            }
            if ee_le(fx.mem.f(p + 0x11B4), 30.0) {
                continue;
            }
            // Respawned on a flat ring (as `spawn::scatter` with flag 0x1D).
            let f = ee::mul(10.0, fx.mem.f(D + 0x11A8));
            fx.fix_residue(f, 0x01E0_3E64);
            let w = ee::fixsfsi(f);
            let a = fx.rand_from(D, 0x01E0_3E6C) & 0xFFFF;
            let r = fx.rand_from(D, 0x01E0_3E88);
            let (_, h) = fx.div(r, w);
            fx.mem.write_u32(p + 0x11B4, 0x4080_0000);
            let radius = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
            let c = fx.tcos(a);
            fx.mem.setf(p + 0x11B8, ee::mul(radius, c));
            let s = fx.tsin(a);
            fx.mem.setf(p + 0x11B0, ee::mul(radius, s));
            let r = fx.rand_from(D, 0x01E0_3ED8);
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0x11BE, h.wrapping_neg());
        }
        let big = n(fx) >= 0xE0;
        fx.mem.write_u32(sp + 0x88, 0x3F81_47AE);
        fx.mem.write_u32(sp + 0x84, if big { 0x3F99_999A } else { 0x3F81_47AE });
        fx.mem.write_u32(sp + 0x80, 0x3F81_47AE);
        fx.particle_quads(D + 0x2310, D + 0x2390, sp + 0x80);
        if n(fx) >= 0xE0 {
            fx.step_down(D + 0x239B, 0, 2);
        }
    }
    if n(fx) >= 0x4F {
        fx.ribbon_draw(D + 0x70, D + 0xD0, D);
        fx.ribbon_draw(D + 0xA0, D + 0x8F0, D);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2A68, m);
    let left = fx.mem.i32(D + 0x2A64) - 1;
    fx.mem.set32(D + 0x2A64, left);
    if m == 0xE0 {
        2
    } else if m == 0x58 {
        1
    } else if (0x62..0xE0).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
