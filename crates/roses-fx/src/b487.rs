//! BMODULE b487 (card 657), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E043F0, data area 0x1ED0000) [F] (helper C3, 31
//! Oct 2026): b851's family ([`crate::b851`]). Until update 169 a rising
//! sprite fountain (+0x2660, 128 sprites thrown out again above 10) at
//! the attacker's node 0x15; on updates 94..148 a turned plate (+0x1750)
//! with a disc (+0x25E0) fading in; from update 108 the tube (+0x1180)
//! round the node, drawn until update 206; from update 123 two flickering
//! discs (+0x1800 / +0x1810) on the target, three rings (+0x1970 + 0x400
//! k) swelling and respawned turned, and a spark burst (+0xE0) thrown out
//! again as each dies. Every colour flickers at random every update.
//! Returns 1 on update 113, 6 on 123..223, 2 on 224, 8 after 250.
//!
//! Library: `spawn::scatter` (0x1E00120), `trail::sparks` (0x1E002E0),
//! `plate::plates` (0x1E017F0) [`crate::blib`]; `ring_setup` / `ring_draw`
//! (0x1E01040 / 0x1E01300), `disc` (0x1E01D80), `sprites` (0x1E022E0)
//! [`crate::fxlib`]; the tube (0x1E03BD0 / 0x1E03CD0, [`crate::tube`]).
use crate::b851::{halves, ring_place, rings_step};
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PLATES: u32 = 0x01E0_17F0;

/// The entry 0x1E043F0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn disc_record(fx: &mut Fx, at: u32, alpha: u8, radius: u32) {
    fx.mem.write_u32(at, 0x8_0000);
    for k in [6u32, 5, 4] {
        fx.mem.write_u8(at + k, 0x80);
    }
    fx.mem.write_u8(at + 7, alpha);
    fx.mem.write_u32(at + 8, radius);
    fx.mem.set16(at + 0xC, 0x24);
    fx.mem.set16(at + 0xE, 0);
}

/// A fountain sprite (`p`) placed: on a ring of radius 2, height
/// -(rand % 30) / 10.
fn fountain_place(fx: &mut Fx, p: u32) {
    ring_place(fx, p, 0x28);
    let r = fx.rand();
    let (_, h) = fx.div(r, 0x1E);
    fx.mem.setf(p + 4, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::neg(ee::from_i32(h))));
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2EE0);
    for x in [0u32, 0x16E0, 0x70, 0x1820, 0x1890, 0x1900, 0x2570, 0x1790, 0x25F0, 0x1110] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2ECCu32, 0x1D, 1), (0x2ED0, 0x15, 0), (0x2ED4, 0x12, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2EC0, 0xFA);
    fx.mem.set32(D + 0x2EC4, 1);
    fx.mem.set32(D + 0x2EC8, param);
    disc_record(fx, D + 0x1800, 0xFF, 0x41F0_0000);
    disc_record(fx, D + 0x1810, 0xFF, 0x4120_0000);
    disc_record(fx, D + 0x25E0, 0, 0x4120_0000);
    fx.texture5(sp + 0xA0, 0);
    // The tube (+0x1180): 32 points, radius 2, rings at +0x12E0.
    let g = D + 0x1180;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    for (k, x) in [(6u32, 0x40u8), (5, 0x40), (4, 0x80), (7, 0x80)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 8, 0x8000);
    fx.mem.write_u32(g + 0xC, 0x4000_0000);
    halves(fx, g, &[(0x140, 0x20), (0x142, 0)]);
    fx.mem.write_u32(g + 0x144, 0x40A0_0000);
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x80), (0x16, 0x80), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x80)]);
    fx.mem.write_u32(g + 0x148, D + 0x12E0);
    halves(fx, g, &[(0x14C, 0x30), (0x14E, 0)]);
    fx.mem.write_u32(g + 0x150, 0x3E4C_CCCD);
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.tube_setup(g, tex);
    fx.texture4(sp + 0xA0, 0xF);
    // The spark burst (+0xE0): 256 sparks at +0x110, width 0.3, length 3.
    let g = D + 0xE0;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 3);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0x20, 0x3E99_999A);
    fx.mem.write_u32(g + 0x24, 0x4040_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x60), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x60), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, D + 0x110);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    // The plate (+0x1750): half sizes (1, 16) and (8, 1).
    let g = D + 0x1750;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x8_0000);
    for k in [0xAu32, 9, 8] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 0xB, 0);
    for (k, v) in [(0xCu32, 0x3F80_0000u32), (0x10, 0x4180_0000), (0x14, 0x3F80_0000), (0x18, 0x4100_0000)] {
        fx.mem.write_u32(g + k, v);
    }
    halves(fx, g, &[(0x20, 0xB0), (0x22, 0x40), (0x24, 0x20), (0x26, 0x20)]);
    for k in [0x30u32, 0x34, 0x38] {
        fx.mem.write_u32(g + k, 0);
    }
    for k in 0..3u32 {
        let g = D + 0x1970 + 0x400 * k;
        fx.mem.write_u32(g + 4, 0);
        halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
        for (o, x) in [(0xAu32, 0xD0u8), (9, 0x70), (8, 0xA0), (0xB, 0x80)] {
            fx.mem.write_u8(g + o, x);
        }
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(g, tex, 11.5, 12.5, 0.0, 0.0);
    }
    // The fountain (+0x2660): 128 sprites at +0x26C0, size 1.
    let g = D + 0x2660;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0001);
    for k in [0xAu32, 9, 8] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 0xB, 0);
    fx.mem.write_u32(g + 0xC, 0x3F80_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0xE4), (0x34, 0xA), (0x36, 0xA), (0x38, 0), (0x3A, 0xE4), (0x3C, 0x100), (0x3E, 0x100)]);
    fx.mem.write_u32(g + 0x40, D + 0x26C0);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4000_0000);
    halves(fx, g, &[(0x5A, 0x80), (0x5C, 2), (0x5E, 0)]);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x26BA) {
        let p = D + 0x26C0 + 16 * k as u32;
        let r = fx.rand();
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, h.wrapping_neg());
        fx.mem.write_u8(p + 0xC, 0);
        fx.mem.write_u8(p + 0xD, 0);
        fountain_place(fx, p);
        k += 1;
    }
    for (x, s) in [(0x1820u32, 0u32), (0x1890, 0x3F00_0000), (0x1900, 0x3F80_0000)] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, s);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n_at = D + 0x2EC4;
    let n = |fx: &mut Fx| fx.mem.i32(n_at);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    node(fx, 0x2ED0, sp + 0x80);
    node(fx, 0x2ED4, sp + 0x90);
    node(fx, 0x2ED0, D + 0x2610);
    fx.look_angles(sp + 0x80, sp + 0x90, D + 0x2600);
    node(fx, 0x2ED0, D + 0x1130);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x90);
    fx.look_angles(sp + 0x90, sp + 0x80, D + 0x1120);
    node(fx, 0x2ED0, D + 0x2590);
    for out in [0x17B0u32, 0x90, 0x1840, 0x18B0, 0x1920] {
        let t = fx.target_handle();
        fx.unit_point(1, t, D + out);
    }
    // s0 = D, s1 = the OT, s2 = D + 0x150, s3 = 3 (after the unit loop),
    // then the loop's.
    let regs = [Some(D), Some(OT), Some(D + 0x150), Some(3)];
    fx.xform_world_saving(D, 0x01E0_4A54, regs);
    fx.xform_billboard_saving(D + 0x16E0, 0x01E0_4A5C, regs);
    fx.xform_world_saving(D + 0x70, 0x01E0_4A64, regs);
    for k in 0..3u32 {
        fx.xform_world_saving(D + 0x1820 + 0x70 * k, 0x01E0_4A74, [Some(D), Some(OT), Some(D + 0x70 * k), Some(k)]);
    }
    fx.xform_billboard_saving(D + 0x2570, 0x01E0_4A90, regs);
    fx.xform_billboard_saving(D + 0x1790, 0x01E0_4A98, regs);
    fx.xform_world_saving(D + 0x25F0, 0x01E0_4AA0, regs);
    fx.xform_world_saving(D + 0x1110, 0x01E0_4AA8, regs);
    for (b, ra) in [(2u32, 0x01E0_4AB0), (1, 0x01E0_4AF0), (0, 0x01E0_4B30)] {
        let r = fx.rand_from(D, ra);
        let x = (r % 128) as u8;
        for g in [0x2178u32, 0x1D78, 0x1978, 0x1184, 0x1758, 0x2668, 0xE8, 0x25E4, 0x1814, 0x1804] {
            fx.mem.write_u8(D + g + b, x);
        }
    }
    if n(fx) < 0xAA {
        fx.sprites(D + 0x2660, D + 0x25F0);
        fx.step_up(D + 0x266B, 0x80, 8);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x26BA) {
            let p = D + 0x26C0 + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) < 0 {
                continue;
            }
            let y = ee::add(fx.mem.f(p + 4), 1.0);
            fx.mem.setf(p + 4, y);
            fx.step_down(p + 0xC, 0, 8);
            if ee_lt(fx.mem.f(p + 4), 10.0) {
                let s = f32::from_bits(0x3F99_999A);
                for c in [8u32, 0] {
                    let v = ee::mul(fx.mem.f(p + c), s);
                    fx.mem.setf(p + c, v);
                }
            }
            if ee_lt(fx.mem.f(p + 4), 10.0) {
                continue;
            }
            fountain_place(fx, p);
            fx.mem.write_u8(p + 0xC, 0x80);
        }
    }
    if n(fx) >= 0x7B {
        for (g, ra) in [(0x1800u32, 0x01E0_4D30u64), (0x1810, 0x01E0_4D40)] {
            fx.disc(D + g, D + 0x1790);
            fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, ra)]);
        }
        let m = n(fx);
        if m >= 0xE0 {
            fx.step_down(D + 0x1807, 0, 0x10);
            fx.step_down(D + 0x1817, 0, 0x10);
        } else {
            fx.mem.write_u8(D + 0x1807, if m % 2 == 0 { 0xFF } else { 0x80 });
        }
    }
    if (0x5E..0x95).contains(&n(fx)) {
        blib::plate::plates(fx, PLATES, OT, D + 0x1750, D + 0x2570);
        fx.disc(D + 0x25E0, D + 0x2570);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_4DE8)]);
        fx.step_up(D + 0x175B, 0x80, 0x10);
        fx.step_up(D + 0x25E7, 0x80, 0x10);
    }
    if n(fx) >= 0x7B {
        rings_step(fx, D + 0x1970, D + 0x1820, n_at, 0xE0);
        blib::trail::sparks(fx, OT, D + 0xE0, D + 0x70);
        if n(fx) >= 0xE1 {
            fx.step_down(D + 0xEB, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x10C) {
            let p = D + 0x110 + 16 * k as u32;
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
            fx.random_dir(sp + 0x70, 1.0);
            for c in [1u32, 2, 0] {
                let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
                fx.mem.write_u32(p + 4 * c, w);
            }
            // rand's saved s0 (D) is the tube's stale TEX1 below.
            let r = fx.rand_from(D, 0x01E0_4FB4);
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0xE, h.wrapping_neg());
        }
    }
    if n(fx) >= 0x6C {
        if n(fx) < 0xCF {
            fx.tube_draw(OT, D + 0x1180, D + 0x1110);
        }
        let s = fx.mem.i16(D + 0x12CE) + 1;
        fx.mem.set16(D + 0x12CE, i32::from(s));
    }
    let m = n(fx) + 1;
    fx.mem.set32(n_at, m);
    let left = fx.mem.i32(D + 0x2EC0) - 1;
    fx.mem.set32(D + 0x2EC0, left);
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
