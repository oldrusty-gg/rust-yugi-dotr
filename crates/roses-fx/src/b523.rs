//! BMODULE b523 (card 593), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E046E0, data area 0x1ED0000; compiled without
//! delay-slot scheduling) [F]: b642's family,
//! re-coloured every update (green rand % 0x70, blue rand % 0xA0 for seven
//! groups). Until update 169 a sprite cloud (+0x2400) rising round the
//! attacker's node 0x18 and respawned on a ring; from update 107 a ribbon
//! trail (+0x70) from node 0x1F toward the target (1 / 16 of the way,
//! ...); on updates 93..148 a turned plate (+0x13B0) and a disc (+0x2380)
//! at node 0x18; from update 123 a pulsing disc (+0x1460) on the target,
//! three rings (+0x1710 + 0x400 k) swelling there and restarting turned at
//! random, and a spark burst (+0x310) thrown out again in random
//! directions. The particle cloud (+0x1470) is set up but never drawn.
//! Returns 1 on update 113, 6 on 123..223, 2 on 224, 8 after 260.
//!
//! Library: `spawn::scatter` (0x1E00120), `trail::sparks` (0x1E002E0)
//! [`crate::blib`]; `ring_setup` / `ring_draw` (0x1E01040 / 0x1E01300),
//! `disc` (0x1E01D80), `sprites` (0x1E022E0) [`crate::fxlib`];
//! `plate::plates` (0x1E017F0); the ribbon (0x1E03BD0 / 0x1E03D20).
use crate::b146::cloud_spawn;
use crate::b436::ring_group;
use crate::b642::{disc_record, halves};
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PLATES: u32 = 0x01E0_17F0;
const TENTH: f32 = f32::from_bits(0x3DCC_CCCD);

/// The entry 0x1E046E0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xD0);
    ret
}

/// A rising sprite (`p`) set on a ring: angle rand, radius (rand % 40 -
/// 20) / 10, height -(rand % 30) / 10 (`ras`: three rands' return
/// addresses, or plain rand).
fn sprite_place(fx: &mut Fx, p: u32, ras: Option<[u32; 3]>) {
    let rand = |fx: &mut Fx, k: usize| match ras {
        Some(r) => fx.rand_from(D, r[k]),
        None => fx.rand(),
    };
    let a = rand(fx, 0) & 0xFFFF;
    let r = rand(fx, 1);
    let (_, h) = fx.div(r, 0x28);
    let radius = ee::mul(TENTH, ee::from_i32(h - 0x14));
    let c = fx.tcos(a);
    fx.mem.setf(p + 8, ee::mul(radius, c));
    let s = fx.tsin(a);
    fx.mem.setf(p, ee::mul(radius, s));
    let r = rand(fx, 2);
    let (_, h) = fx.div(r, 0x1E);
    fx.mem.setf(p + 4, ee::mul(TENTH, ee::neg(ee::from_i32(h))));
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2C90);
    for x in [0u32, 0x1340, 0x2A0, 0x15C0, 0x1630, 0x16A0, 0x2310, 0x13F0, 0x2390] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2C80u32, 0x1F, 1), (0x2C84, 0x18, 0), (0x2C88, 0x20, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2C74, 0x104);
    fx.mem.set32(D + 0x2C78, 1);
    fx.mem.set32(D + 0x2C7C, param);
    fx.mem.set32(D + 0x2C70, 0x10);
    disc_record(fx, D + 0x1460, [0x80, 0x80, 0x80, 0xFF], 0x41F0_0000);
    disc_record(fx, D + 0x2380, [0x80, 0x80, 0x80, 0], 0x4080_0000);
    fx.texture4(sp + 0xC0, 0xF);
    // The ribbon (+0x70): 32 slots, life 100, width 20.
    let g = D + 0x70;
    let t = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 8, 0x8003);
    for (k, x) in [(6u32, 0xD0u8), (5, 0x70), (4, 0xA0), (7, 0x80)] {
        fx.mem.write_u8(g + k, x);
    }
    halves(fx, g, &[(0x10, 0xC8), (0x12, 0x20), (0x14, 7), (0x16, 0x20), (0x18, 0xC8), (0x1A, 0x20), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    halves(fx, g, &[(0x22, 8), (0x24, 0), (0x26, 0x64), (0x28, 0x20), (0x2A, 0), (0x2C, 1)]);
    // The spark burst (+0x310): 256 sparks at +0x340 (random directions).
    let g = D + 0x310;
    let t = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 3);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x4040_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x60), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x60), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, D + 0x340);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    // The plate (+0x13B0): half sizes (0.5, 8) / (4, 0.5), rectangle
    // (176, 64, 32, 32).
    let g = D + 0x13B0;
    let t = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x8_0000);
    for k in [0xAu32, 9, 8] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 0xB, 0);
    for (k, v) in [(0xCu32, 0x3F00_0000u32), (0x10, 0x4100_0000), (0x14, 0x3F00_0000), (0x18, 0x4080_0000)] {
        fx.mem.write_u32(g + k, v);
    }
    halves(fx, g, &[(0x20, 0xB0), (0x22, 0x40), (0x24, 0x20), (0x26, 0x20)]);
    for k in [0x30u32, 0x34, 0x38] {
        fx.mem.write_u32(g + k, 0);
    }
    // The particle cloud (+0x1470).
    let g = D + 0x1470;
    let t = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x85);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0xF0), (0x2A, 0), (0x24, 0x10), (0x26, 0x10), (0x2C, 0x100), (0x2E, 0x100)]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 1), (0x13C, 1), (0x140, 8), (0x136, 0xA), (0x138, 0xA)]);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, x);
    }
    cloud_spawn(fx, g);
    // Three rings (+0x1710 + 0x400 k): radii 11.5 / 12.5.
    for k in 0..3u32 {
        let g = D + 0x1710 + 0x400 * k;
        ring_group(fx, g, 0, 0x80, [0xA0, 0x70, 0xD0, 0x80]);
        let t = fx.mem.read_u32(sp + 0xC0);
        fx.ring_setup(g, t, f32::from_bits(0x4138_0000), f32::from_bits(0x4148_0000), 0.0, 0.0);
    }
    // The sprite cloud (+0x2400): 128 sprites at +0x2460, size 1.
    let g = D + 0x2400;
    let t = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x7000_0001);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0xC, 0x3F80_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0xE4), (0x34, 0xA), (0x36, 0xA), (0x38, 0), (0x3A, 0xE4), (0x3C, 0x100), (0x3E, 0x100)]);
    fx.mem.write_u32(g + 0x40, D + 0x2460);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4000_0000);
    halves(fx, g, &[(0x5A, 0x80), (0x5C, 2), (0x5E, 0)]);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x245A) {
        let p = D + 0x2460 + 16 * k as u32;
        let r = fx.rand();
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, h.wrapping_neg());
        fx.mem.write_u8(p + 0xC, 0);
        fx.mem.write_u8(p + 0xD, 0);
        sprite_place(fx, p, None);
        k += 1;
    }
    for (x, v) in [(0x15C0u32, 0u32), (0x1630, 0x3F00_0000), (0x16A0, 0x3F80_0000)] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, v);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2C78);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    node(fx, 0x2C84, sp + 0xA0);
    node(fx, 0x2C88, sp + 0xB0);
    node(fx, 0x2C84, D + 0x23B0);
    fx.look_angles(sp + 0xA0, sp + 0xB0, D + 0x23A0);
    node(fx, 0x2C84, D + 0x2330);
    for out in [D + 0x2C60, D + 0x1410, D + 0x2C0, D + 0x15E0, D + 0x1650, D + 0x16C0] {
        let t = fx.target_handle();
        fx.unit_position(t, out);
    }
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0xA0);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0xB0);
    fx.look_angles(sp + 0xA0, sp + 0xB0, D + 0x2B0);
    if n(fx) < 0x6C {
        node(fx, 0x2C80, D + 0x1360);
    }
    if n(fx) >= 0x6B {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x1360);
        let m = fx.mem.i32(D + 0x2C70);
        fx.approach_by(D + 0x1360, D + 0x2C60, m);
        let m = fx.mem.i32(D + 0x2C70);
        if m != 0 {
            fx.mem.set32(D + 0x2C70, m - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 = the loop count (3 after the target loop),
    // s3 = sp + 0xB0.
    let regs = [Some(D), Some(OT), Some(3), Some(sp + 0xB0)];
    fx.xform_world_saving(D, 0x01E0_4FBC, regs);
    fx.xform_billboard_saving(D + 0x1340, 0x01E0_4FC8, regs);
    fx.xform_world_saving(D + 0x2A0, 0x01E0_4FD4, regs);
    for k in 0..3u32 {
        fx.xform_world_saving(D + 0x15C0 + 0x70 * k, 0x01E0_4FFC, [Some(D), Some(OT), Some(k), Some(sp + 0xB0)]);
    }
    fx.xform_billboard_saving(D + 0x2310, 0x01E0_5018, regs);
    fx.xform_billboard_saving(D + 0x13F0, 0x01E0_5024, regs);
    fx.xform_world_saving(D + 0x2390, 0x01E0_5030, regs);
    for (c, m, ra) in [(1u32, 0x70, 0x01E0_5038), (0, 0xA0, 0x01E0_5070)] {
        let r = fx.rand_from(D, ra);
        let (_, h) = fx.div(r, m);
        for g in [0x1478u32, 0x13B8, 0x2408, 0x318, 0x74, 0x2384, 0x1464] {
            fx.mem.write_u8(D + g + c, h as u8);
        }
    }
    if n(fx) < 0xAA {
        fx.sprites(D + 0x2400, D + 0x2390);
        fx.step_up(D + 0x240B, 0x80, 8);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x245A) {
            let p = D + 0x2460 + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) < 0 {
                continue;
            }
            let y = ee::add(fx.mem.f(p + 4), 1.0);
            fx.mem.setf(p + 4, y);
            fx.step_down(p + 0xC, 0, 8);
            if ee_lt(fx.mem.f(p + 4), 10.0) {
                let s = f32::from_bits(0x3F99_999A);
                for o in [8u32, 0] {
                    let v = ee::mul(fx.mem.f(p + o), s);
                    fx.mem.setf(p + o, v);
                }
            }
            if ee_lt(fx.mem.f(p + 4), 10.0) {
                continue;
            }
            sprite_place(fx, p, Some([0x01E0_5198, 0x01E0_51B8, 0x01E0_5228]));
            fx.mem.write_u8(p + 0xC, 0x80);
        }
    }
    if n(fx) >= 0x7B {
        fx.disc(D + 0x1460, D + 0x13F0);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_52AC)]);
        let m = n(fx);
        if m >= 0xE0 {
            fx.step_down(D + 0x1467, 0, 0x10);
        } else {
            fx.mem.write_u8(D + 0x1467, if m % 2 == 0 { 0xFF } else { 0x80 });
        }
    }
    let m = n(fx);
    if (0x5D..0x95).contains(&m) {
        blib::plate::plates(fx, PLATES, OT, D + 0x13B0, D + 0x2310);
        fx.disc(D + 0x2380, D + 0x2310);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_5354)]);
        fx.step_up(D + 0x13BB, 0x80, 0x10);
        fx.step_up(D + 0x2387, 0x80, 0x10);
    }
    if n(fx) >= 0x7B {
        for k in 0..3u32 {
            let g = D + 0x1710 + 0x400 * k;
            let x = D + 0x15C0 + 0x70 * k;
            fx.ring_draw(g, x);
            fx.step_down(g + 0xB, 0, 8);
            if fx.approach(x, 2.0, TENTH) == 0 {
                for o in [8u32, 4, 0] {
                    fx.mem.setf(x + o, TENTH);
                }
                for (o, ra) in [(0x10u32, 0x01E0_5424), (0x14, 0x01E0_544C), (0x18, 0x01E0_5474)] {
                    let r = fx.rand_from(D, ra);
                    fx.mem.set32(x + o, r & 0xFFFF);
                }
                if n(fx) < 0xE0 {
                    fx.mem.write_u8(g + 0xB, 0x80);
                }
            }
        }
        blib::trail::sparks(fx, OT, D + 0x310, D + 0x2A0);
        if n(fx) >= 0xE1 {
            fx.step_down(D + 0x31B, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x33C) {
            let p = D + 0x340 + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) < 0 {
                continue;
            }
            let s = f32::from_bits(0x3FB3_3333);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 4 * c), s);
                fx.mem.setf(p + 4 * c, x);
            }
            if fx.mem.i16(p + 0xE) < 9 {
                continue;
            }
            fx.random_dir(sp + 0x90, 1.0);
            for (o, s) in [(4u32, 0x94u32), (8, 0x98), (0, 0x90)] {
                let w = fx.mem.read_u32(sp + s);
                fx.mem.write_u32(p + o, w);
            }
            let r = fx.rand_from(D, 0x01E0_55A0);
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0xE, h.wrapping_neg());
        }
    }
    if n(fx) >= 0x6C {
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2C78, m);
    let left = fx.mem.i32(D + 0x2C74) - 1;
    fx.mem.set32(D + 0x2C74, left);
    if m == 0xE0 {
        2
    } else if m == 0x71 {
        1
    } else if (0x7B..0xE0).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
