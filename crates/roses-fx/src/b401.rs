//! BMODULE b401 (card 344), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02FD0, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): b634's family with five ribbons fired in volleys. At the
//! attacker's nodes 3 / 0x16 / 0x29 / 0x3C / 0x4F five discs (+0x2E50 +
//! 0x10 k) and quad bursts (+0x30D0 + 0x460 k) flash at each volley
//! (updates 10, 30, 53, 73, 96, 112, 133, 155, 175, 194: 0x1E03B90 resets
//! the ribbons, the start update +0x46F4 and the alphas) until update 170;
//! the five ribbons (+0x70 + 0x30 k, points at *(+0xB60 + 4 k)) run from
//! the nodes toward the target (1 / 16 of the way per update ...); at the
//! target from update 26 a disc (+0x1EC0), three widening rings (+0x2020
//! + 0x400 k) and a spark burst (+0xBF0, `blib::trail::sparks`), relit at
//! each volley's arrival; the red / green / blue bytes re-rolled every
//! update. Returns 1 on update 16, 6 on 26..215, 2 on 216, 8 after 234.
//!
//! Library: `colours4` (0x1E00080), `spawn::scatter` (0x1E00120),
//! `trail::sparks` (0x1E002E0) [`crate::blib`], `ring_setup` /
//! `ring_draw` (0x1E01040 / 0x1E01300), `disc` (0x1E017F0), `quads_setup`
//! / `quads_draw` (0x1E02860 / 0x1E029C0) [`crate::fxlib`], the ribbon
//! (0x1E01D50 / 0x1E01EA0, [`crate::ribbon`]).
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad corner template (rodata, 0x1E02860's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_3C00;
const TENTH: u32 = 0x3DCC_CCCD;
/// The updates a volley starts (0x1E03530..) / arrives (0x1E037E8..).
const VOLLEYS: [i32; 10] = [0xA, 0x1E, 0x35, 0x49, 0x60, 0x70, 0x85, 0x9B, 0xAF, 0xC2];
const ARRIVALS: [i32; 10] = [0x1A, 0x2E, 0x45, 0x59, 0x70, 0x80, 0x95, 0xAB, 0xBF, 0xD2];

/// The entry 0x1E02FD0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

fn bytes(fx: &mut Fx, at: u32, xs: &[(u32, u8)]) {
    for &(k, x) in xs {
        fx.mem.write_u8(at + k, x);
    }
}

fn disc_record(fx: &mut Fx, at: u32, alpha: u8, radius: u32) {
    fx.mem.write_u32(at, 0x8_0000);
    bytes(fx, at, &[(6, 0xC0), (5, 0x70), (4, 0xC0), (7, alpha)]);
    fx.mem.write_u32(at + 8, radius);
    fx.mem.set16(at + 0xC, 0x24);
    fx.mem.set16(at + 0xE, 0);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4700);
    fx.xform_init(D);
    for k in 0..5u32 {
        fx.xform_init(D + 0x1C20 + 0x70 * k);
        fx.xform_init(D + 0x2C20 + 0x70 * k);
        fx.xform_init(D + 0x2EA0 + 0x70 * k);
    }
    fx.xform_init(D + 0xB80);
    for k in 0..3u32 {
        fx.xform_init(D + 0x1ED0 + 0x70 * k);
    }
    fx.xform_init(D + 0x1E50);
    for (at, node, clear) in [(0x46CCu32, 3, 1), (0x46D0, 0x16, 0), (0x46D4, 0x29, 0), (0x46D8, 0x3C, 0), (0x46DC, 0x4F, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x46C0, 0xEB);
    fx.mem.set32(D + 0x46C4, 1);
    fx.mem.set32(D + 0x46C8, param);
    for k in 0..5u32 {
        fx.mem.set32(D + 0x46E0 + 4 * k, 0x10);
    }
    fx.mem.set32(D + 0x46F4, 0xA);
    disc_record(fx, D + 0x1EC0, 0xFF, 0x41F0_0000);
    for k in 0..5u32 {
        disc_record(fx, D + 0x2E50 + 0x10 * k, 0, 0x4120_0000);
    }
    fx.texture5(sp + 0xA0, 0);
    // The ribbons (+0x70 + 0x30 k): cell (16, 0, 16, 32), width 70.
    for k in 0..5u32 {
        let g = D + 0x70 + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.mem.write_u32(g, tex);
        fx.mem.write_u32(g + 8, 0x8003);
        bytes(fx, g, &[(6, 0x80), (5, 0x80), (4, 0x80), (7, 0x80)]);
        halves(fx, g, &[(0x10, 0x10), (0x12, 0), (0x14, 0x10), (0x16, 0x20), (0x18, 0x10), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)]);
        fx.mem.write_u32(g + 0xC, 0x428C_0000);
        halves(fx, g, &[(0x22, 0x20), (0x24, 0), (0x26, 1), (0x28, 0x10), (0x2A, 0), (0x2C, 1)]);
    }
    for k in 0..5u32 {
        fx.mem.write_u32(D + 0xB60 + 4 * k, D + 0x160 + 0x200 * k);
    }
    fx.texture4(sp + 0xA0, 0xF);
    let tex = fx.mem.read_u32(sp + 0xA0);
    // The sparks (+0xBF0): 256 at +0xC20, width 0.2, length 5.
    let g = D + 0xBF0;
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 3);
    bytes(fx, g, &[(0xA, 0xC0), (9, 0x70), (8, 0xC0), (0xB, 0x80)]);
    fx.mem.write_u32(g + 0x20, 0x3E4C_CCCD);
    fx.mem.write_u32(g + 0x24, 0x40A0_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x60), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x60), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, D + 0xC20);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    // The rings (+0x2020 + 0x400 k): radii 11.7 / 12.3.
    for k in 0..3u32 {
        let g = D + 0x2020 + 0x400 * k;
        fx.mem.write_u32(g + 4, 0);
        halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
        bytes(fx, g, &[(0xA, 0xC0), (9, 0x70), (8, 0xC0), (0xB, 0x80)]);
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(g, tex, f32::from_bits(0x413B_3333), f32::from_bits(0x4144_CCCD), 0.0, 0.0);
    }
    // The quad bursts (+0x30D0 + 0x460 k), size 1.
    for k in 0..5u32 {
        let g = D + 0x30D0 + 0x460 * k;
        fx.mem.write_u32(g + 4, 1);
        halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
        bytes(fx, g, &[(0xA, 0xC0), (9, 0x70), (8, 0xC0), (0xB, 0)]);
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.quads_setup(g, tex, 1.0, QUAD_TEMPLATE);
    }
    for (x, s) in [(0x1ED0u32, 0u32), (0x1F40, 0x3F00_0000), (0x1FB0, 0x3F80_0000)] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, s);
        }
    }
    0
}

/// 0x1E03B90(D): a volley: the start update = now, the ribbons' approach
/// divisors 16, their points (+0x94 / +0x9A) cleared, the quad bursts'
/// alpha 0x20 and the discs' 0xFF.
fn volley(fx: &mut Fx) {
    let n = fx.mem.read_u32(D + 0x46C4);
    fx.mem.write_u32(D + 0x46F4, n);
    for k in 0..5u32 {
        fx.mem.write_u32(D + 0x46E0 + 4 * k, 0x10);
        fx.mem.set16(D + 0x9A + 0x30 * k, 0);
        fx.mem.set16(D + 0x94 + 0x30 * k, 0);
        fx.mem.write_u8(D + 0x30DB + 0x460 * k, 0x20);
        fx.mem.write_u8(D + 0x2E57 + 0x10 * k, 0xFF);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x46C4);
    let start = |fx: &mut Fx| fx.mem.i32(D + 0x46F4);
    let node = |fx: &mut Fx, k: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x46CC + 4 * k);
        fx.node_position(h, s, out);
    };
    let target = |fx: &mut Fx, out: u32| {
        let t = fx.target_handle();
        fx.unit_position(t, out);
    };
    for k in 0..5u32 {
        node(fx, k, D + 0x2C40 + 0x70 * k);
        node(fx, k, D + 0x2EC0 + 0x70 * k);
    }
    target(fx, D + 0x46B0);
    target(fx, D + 0x1E70);
    target(fx, D + 0xBA0);
    for k in 0..3u32 {
        target(fx, D + 0x1EF0 + 0x70 * k);
    }
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x80);
    target(fx, sp + 0x90);
    fx.look_angles(sp + 0x80, sp + 0x90, D + 0xB90);
    if VOLLEYS.contains(&n(fx)) {
        volley(fx);
    }
    for k in 0..5u32 {
        let p = D + 0x1C40 + 0x70 * k;
        if n(fx) <= start(fx) {
            node(fx, k, p);
        }
        if n(fx) >= start(fx) {
            let pts = fx.mem.read_u32(D + 0xB60 + 4 * k);
            fx.ribbon_push(D + 0x70 + 0x30 * k, pts, p);
            let c = fx.mem.i32(D + 0x46E0 + 4 * k);
            fx.approach_by(p, D + 0x46B0, c);
            let c = fx.mem.i32(D + 0x46E0 + 4 * k);
            if c != 0 {
                fx.mem.set32(D + 0x46E0 + 4 * k, c - 1);
            }
        }
    }
    // s0 = D, s1 = the OT, s2 / s3 the loops'.
    fx.xform_world_saving(D, 0x01E0_3658, [Some(D), Some(OT), Some(D + 0x230), Some(D + 0x14)]);
    for k in 0..5u32 {
        let regs = [Some(D), Some(OT), Some(D + 0x70 * k), Some(k)];
        fx.xform_billboard_saving(D + 0x1C20 + 0x70 * k, 0x01E0_3668, regs);
        fx.xform_billboard_saving(D + 0x2C20 + 0x70 * k, 0x01E0_3670, regs);
        fx.xform_world_saving(D + 0x2EA0 + 0x70 * k, 0x01E0_3678, regs);
    }
    fx.xform_world_saving(D + 0xB80, 0x01E0_3690, [Some(D), Some(OT), Some(D + 0x230), Some(5)]);
    for k in 0..3u32 {
        let regs = [Some(D), Some(OT), Some(D + 0x70 * k), Some(k)];
        fx.xform_world_saving(D + 0x1ED0 + 0x70 * k, 0x01E0_36A0, regs);
    }
    fx.xform_billboard_saving(D + 0x1E50, 0x01E0_36BC, [Some(D), Some(OT), Some(D + 0x150), Some(3)]);
    for (base, at) in [(0x80, 2u32), (0x30, 1), (0x80, 0)] {
        let r = fx.rand();
        let v = r % 64 + base;
        fx.mem.set8(D + 0xBF8 + at, v);
        fx.mem.set8(D + 0x1EC4 + at, v);
    }
    if n(fx) < 0xAA {
        for k in 0..5u32 {
            let disc = D + 0x2E50 + 0x10 * k;
            fx.disc(disc, D + 0x2C20 + 0x70 * k);
            fx.step_down(disc + 7, 0, 0x10);
            let g = D + 0x30D0 + 0x460 * k;
            fx.quads_draw(g, D + 0x2EA0 + 0x70 * k);
            fx.step_down(g + 0xB, 0, 2);
            for q in 0..64u32 {
                for c in 0..3u32 {
                    let a = g + 0x60 + 16 * q + 4 * c;
                    let v = fx.mem.i32(a).wrapping_add(0x80);
                    fx.mem.set32(a, v);
                }
            }
        }
    }
    if ARRIVALS.contains(&n(fx)) {
        fx.mem.write_u8(D + 0x1EC7, 0xFF);
        fx.mem.write_u8(D + 0xBFB, 0x80);
        for k in 0..3u32 {
            fx.mem.write_u8(D + 0x202B + 0x400 * k, 0x80);
        }
    }
    if n(fx) >= 0x1A {
        fx.disc(D + 0x1EC0, D + 0x1E50);
        fx.step_down(D + 0x1EC7, 0, 0x10);
        for k in 0..3u32 {
            let (g, x) = (D + 0x2020 + 0x400 * k, D + 0x1ED0 + 0x70 * k);
            fx.ring_draw(g, x);
            fx.step_down(g + 0xB, 0, 8);
            if fx.approach(x, 2.0, f32::from_bits(TENTH)) != 0 {
                continue;
            }
            for c in [8u32, 4, 0] {
                fx.mem.write_u32(x + c, TENTH);
            }
            for c in 0..3u32 {
                let r = fx.rand();
                fx.mem.set32(x + 0x10 + 4 * c, r & 0xFFFF);
            }
        }
        blib::trail::sparks(fx, OT, D + 0xBF0, D + 0xB80);
        fx.step_down(D + 0xBFB, 0, 8);
        let mut k = 0;
        while k < fx.mem.i16(D + 0xC1C) {
            let p = D + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xC2E) < 0 {
                continue;
            }
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 0xC20 + 4 * c), 1.5);
                fx.mem.setf(p + 0xC20 + 4 * c, x);
            }
            if fx.mem.i16(p + 0xC2E) < 9 {
                continue;
            }
            let r = fx.rand();
            let (_, h) = fx.div(r, 0xA);
            let len = ee::add(1.0, ee::mul(f32::from_bits(TENTH), ee::from_i32(h)));
            fx.random_dir(sp + 0x70, len);
            for c in [1u32, 2, 0] {
                let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
                fx.mem.write_u32(p + 0xC20 + 4 * c, w);
            }
            let r = fx.rand();
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0xC2E, h.wrapping_neg());
        }
    }
    if start(fx) < n(fx) {
        for k in 0..5u32 {
            let pts = fx.mem.read_u32(D + 0xB60 + 4 * k);
            fx.ribbon_draw(D + 0x70 + 0x30 * k, pts, D);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x46C4, m);
    let left = fx.mem.i32(D + 0x46C0) - 1;
    fx.mem.set32(D + 0x46C0, left);
    if m == 0xD8 {
        2
    } else if m == 0x10 {
        1
    } else if (0x1A..0xD8).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
