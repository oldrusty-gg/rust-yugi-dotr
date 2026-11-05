//! BMODULE b146 (card 167), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04210, data area 0x1ED0000) [F] (helper C3, 31
//! Oct 2026): b642's family. A quad burst (+0x2760) between the
//! attacker's nodes 3..6 growing to 1 on updates 71..129, then fading from
//! update 170; two turned plates (+0x15E0 / +0x1690) with a disc each at
//! nodes 3 / 4, faded in on 74..89 and out on 90..105, drawn until update
//! 224; from update 170 two ribbon trails (+0x70 / +0xA0) from those nodes
//! to the target (1 / 24 of the way per update, then 1 / 23 ...) and the
//! frame quad; a flickering disc (+0x1740) on the target; from update 194
//! three rings (+0x19F0 + 0x400 k) swelling and respawned turned, a spark
//! burst (+0x540) thrown out again as each dies, and a particle cloud
//! (+0x1750). Returns 1 on update 184, 6 on 194..291, 2 on 292, 8 after
//! 310.
//!
//! Library: the frame quad (0x1E00120, [`crate::framegs`]),
//! `spawn::scatter` (0x1E00240), `trail::sparks` (0x1E00400)
//! [`crate::blib`]; `ring_setup` / `ring_draw` (0x1E01160 / 0x1E01420),
//! `disc` (0x1E01EA0), `particle_quads` (0x1E02400), `quads_setup` /
//! `quads_draw` (0x1E02F90 / 0x1E030F0) [`crate::fxlib`];
//! `plate::plates` (0x1E01910); the ribbon (0x1E03700 / 0x1E03850).
use crate::b642::{disc_record, halves, plate_group, ribbon_group};
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PLATES: u32 = 0x01E0_1910;
/// The quad corner template (rodata 0x1E05100, 0x1E02F90's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_5100;

/// The entry 0x1E04210.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xA0);
    ret
}

/// A particle cloud's particles (`g`: count +0x130, particles +0x30 +
/// 16 k) placed at +0x10 / +0x14 +- 10 (rand % 20 - 10), z +0x18, frame
/// -(+0x13A) k, tick 0; +0x13E = +0x140 - 1 [F].
pub(crate) fn cloud_spawn(fx: &mut Fx, g: u32) {
    let mut k = 0;
    while k < fx.mem.i16(g + 0x130) {
        let p = g + 0x30 + 16 * k as u32;
        for c in 0..2u32 {
            let r = fx.rand();
            let (_, h) = fx.div(r, 0x14);
            let v = ee::add(fx.mem.f(g + 0x10 + 4 * c), ee::from_i32(h - 10));
            fx.mem.setf(p + 4 * c, v);
        }
        let z = fx.mem.read_u32(g + 0x18);
        fx.mem.write_u32(p + 8, z);
        let a = -i32::from(fx.mem.i16(g + 0x13A));
        let f = fx.mult(a, k as i32);
        fx.mem.set16(p + 0xE, f);
        fx.mem.set16(p + 0xC, 0);
        let t = fx.mem.i16(g + 0x140) - 1;
        fx.mem.set16(g + 0x13E, t as i32);
        k += 1;
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2BF0);
    for x in [0u32, 0x1570, 0x1620, 0x4D0, 0x18A0, 0x1910, 0x1980, 0x25F0, 0x2670, 0x16D0, 0x26F0] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2BE0u32, 3, 1), (0x2BE4, 4, 0), (0x2BE8, 5, 0), (0x2BEC, 6, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2BD4, 0x136);
    fx.mem.set32(D + 0x2BD8, 1);
    fx.mem.set32(D + 0x2BDC, param);
    fx.mem.set32(D + 0x2BD0, 0x18);
    disc_record(fx, D + 0x1740, [0x30, 0x70, 0xA0, 0xFF], 0x41A0_0000);
    disc_record(fx, D + 0x2660, [0x30, 0x70, 0xA0, 0], 0x4000_0000);
    disc_record(fx, D + 0x26E0, [0x30, 0x70, 0xA0, 0], 0x4000_0000);
    fx.texture5(sp + 0x90, 0);
    for g in [0x70u32, 0xA0] {
        let tex = fx.mem.read_u32(sp + 0x90);
        ribbon_group(fx, D + g, tex, 0, 0x4220_0000, 0x48);
    }
    // The spark burst (+0x540): 256 sparks at +0x570, width 0.5, length 3.
    let g = D + 0x540;
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 3);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x4040_0000);
    halves(fx, g, &[(0x10, 0), (0x12, 0x20), (0x14, 0x20), (0x16, 0x20), (0x18, 0), (0x1A, 0x20), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, D + 0x570);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 2);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    for g in [0x15E0u32, 0x1690] {
        let tex = fx.mem.read_u32(sp + 0x90);
        plate_group(fx, D + g, tex, 0);
    }
    fx.texture4(sp + 0x90, 0xF);
    // The particle cloud (+0x1750): 16 particles, half size 20.
    let g = D + 0x1750;
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x485);
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
    // Three rings (+0x19F0 + 0x400 k).
    for k in 0..3u32 {
        let g = D + 0x19F0 + 0x400 * k;
        fx.mem.write_u32(g + 4, 0);
        halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
        for (o, x) in [(0xAu32, 0xA0u8), (9, 0x70), (8, 0x30), (0xB, 0x80)] {
            fx.mem.write_u8(g + o, x);
        }
        let tex = fx.mem.read_u32(sp + 0x90);
        fx.ring_setup(g, tex, 11.5, 12.5, 0.0, 0.0);
    }
    // The quad burst (+0x2760), 64 quads, size 0.5.
    let g = D + 0x2760;
    fx.mem.write_u32(g + 4, 1);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    for (o, x) in [(0xAu32, 0xA0u8), (9, 0x70), (8, 0x30), (0xB, 0x20)] {
        fx.mem.write_u8(g + o, x);
    }
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.quads_setup(g, tex, 0.5, QUAD_TEMPLATE);
    for k in 0..64u32 {
        let q = D + 16 * k;
        let r = fx.rand();
        fx.mem.set32(q + 0x27C0, (r & 0x1FFF) - 0x1000);
        let r = fx.rand();
        fx.mem.set32(q + 0x27C4, r & 0xFFFF);
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x6000);
        fx.mem.set32(q + 0x27C8, h - 0x3000);
    }
    for k in [0x26F8u32, 0x26F4, 0x26F0] {
        fx.mem.write_u32(D + k, 0x3DCC_CCCD);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2BD8);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    node(fx, 0x2BE0, D + 0x2610);
    node(fx, 0x2BE4, D + 0x2690);
    fx.midpoint(D + 0x2610, D + 0x2690, sp + 0x60);
    node(fx, 0x2BE8, sp + 0x70);
    node(fx, 0x2BEC, sp + 0x80);
    fx.midpoint(sp + 0x70, sp + 0x80, D + 0x2710);
    fx.look_angles(sp + 0x60, D + 0x2710, D + 0x2700);
    for out in [0x2BC0u32, 0x16F0, 0x4F0, 0x18C0, 0x1930, 0x19A0] {
        let t = fx.target_handle();
        fx.unit_position(t, D + out);
    }
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x70);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x80);
    fx.look_angles(sp + 0x70, sp + 0x80, D + 0x4E0);
    if n(fx) < 0xAB {
        node(fx, 0x2BE0, D + 0x1590);
        node(fx, 0x2BE4, D + 0x1640);
    }
    if n(fx) >= 0xAA {
        fx.ribbon_push(D + 0x70, D + 0xD0, D + 0x1590);
        fx.ribbon_push(D + 0xA0, D + 0x2D0, D + 0x1640);
        for p in [0x1590u32, 0x1640] {
            let m = fx.mem.i32(D + 0x2BD0);
            fx.approach_by(D + p, D + 0x2BC0, m);
        }
        let m = fx.mem.i32(D + 0x2BD0);
        if m != 0 {
            fx.mem.set32(D + 0x2BD0, m - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 = D + 0x150 (after the unit loop), s3 = sp +
    // 0x80, then the loop's.
    let regs = [Some(D), Some(OT), Some(D + 0x150), Some(sp + 0x80)];
    fx.xform_world_saving(D, 0x01E0_4A4C, regs);
    fx.xform_billboard_saving(D + 0x1570, 0x01E0_4A54, regs);
    fx.xform_billboard_saving(D + 0x1620, 0x01E0_4A5C, regs);
    fx.xform_world_saving(D + 0x4D0, 0x01E0_4A64, regs);
    for k in 0..3u32 {
        fx.xform_world_saving(D + 0x18A0 + 0x70 * k, 0x01E0_4A74, [Some(D), Some(OT), Some(D + 0x70 * k), Some(k)]);
    }
    let regs = [Some(D), Some(OT), Some(D + 0x150), Some(3)];
    fx.xform_billboard_saving(D + 0x25F0, 0x01E0_4A90, regs);
    fx.xform_billboard_saving(D + 0x2670, 0x01E0_4A98, regs);
    fx.xform_billboard_saving(D + 0x16D0, 0x01E0_4AA0, regs);
    fx.xform_world_saving(D + 0x26F0, 0x01E0_4AA8, regs);
    if n(fx) >= 0xAA {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    fx.disc(D + 0x1740, D + 0x16D0);
    fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_4AD0)]);
    let m = n(fx);
    if m >= 0x124 {
        fx.step_down(D + 0x1747, 0, 0x10);
    } else {
        fx.mem.write_u8(D + 0x1747, if m % 2 == 0 { 0x80 } else { 0x40 });
    }
    fx.quads_draw(D + 0x2760, D + 0x26F0);
    let m = n(fx);
    if (0x47..0x82).contains(&m) {
        if fx.approach(D + 0x26F0, 1.0, f32::from_bits(0x3D4C_CCCD)) == 0 {
            fx.step_down(D + 0x276B, 0, 1);
        }
    } else if m >= 0xAA {
        if m == 0xAA {
            fx.mem.write_u8(D + 0x276B, 0x20);
        }
        fx.step_down(D + 0x276B, 0, 1);
    }
    let m = n(fx);
    if m < 0xE1 {
        if m == 0xAA {
            fx.mem.write_u8(D + 0x169B, 0x20);
            fx.mem.write_u8(D + 0x15EB, 0x20);
            fx.mem.write_u8(D + 0x26E7, 0xFF);
            fx.mem.write_u8(D + 0x2667, 0xFF);
        }
        blib::plate::plates(fx, PLATES, OT, D + 0x15E0, D + 0x25F0);
        blib::plate::plates(fx, PLATES, OT, D + 0x1690, D + 0x2670);
        fx.disc(D + 0x2660, D + 0x25F0);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_4C28)]);
        fx.disc(D + 0x26E0, D + 0x2670);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_4C38)]);
        let m = n(fx);
        if (0x4A..0x5A).contains(&m) {
            for k in [0x15EBu32, 0x169B, 0x2667, 0x26E7] {
                fx.step_up(D + k, 0x80, 8);
            }
        } else if (0x5A..0x6A).contains(&m) {
            for k in [0x15EBu32, 0x169B, 0x2667, 0x26E7] {
                fx.step_down(D + k, 0, 8);
            }
        }
    }
    if n(fx) < 0xAA {
        for k in [0x1618u32, 0x16C8] {
            let v = fx.mem.i32(D + k).wrapping_add(0x200);
            fx.mem.set32(D + k, v);
        }
    } else {
        fx.mem.write_u32(D + 0x16C8, 0);
        fx.mem.write_u32(D + 0x1618, 0);
    }
    if n(fx) >= 0xC2 {
        for k in 0..3u32 {
            let (g, x) = (D + 0x19F0 + 0x400 * k, D + 0x18A0 + 0x70 * k);
            fx.ring_draw(g, x);
            fx.step_down(g + 0xB, 0, 8);
            if fx.approach(x, 2.0, f32::from_bits(0x3DCC_CCCD)) != 0 {
                continue;
            }
            for o in [8u32, 4, 0] {
                fx.mem.write_u32(x + o, 0x3DCC_CCCD);
            }
            for o in [0x10u32, 0x14, 0x18] {
                let r = fx.rand();
                fx.mem.set32(x + o, r & 0xFFFF);
            }
            if n(fx) < 0x124 {
                fx.mem.write_u8(g + 0xB, 0x80);
            }
        }
        blib::trail::sparks(fx, OT, D + 0x540, D + 0x4D0);
        if n(fx) >= 0x125 {
            fx.step_down(D + 0x54B, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x56C) {
            let p = D + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0x57E) < 0 {
                continue;
            }
            let s = f32::from_bits(0x3FB3_3333);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 0x570 + 4 * c), s);
                fx.mem.setf(p + 0x570 + 4 * c, x);
            }
            if fx.mem.i16(p + 0x57E) < 0xD {
                continue;
            }
            fx.random_dir(sp + 0x60, 1.0);
            for c in [1u32, 2, 0] {
                let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
                fx.mem.write_u32(p + 0x570 + 4 * c, w);
            }
            let r = fx.rand();
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0x57E, h.wrapping_neg());
            let a = fx.mem.read_u8(D + 0x54B);
            fx.mem.write_u8(p + 0x57C, a);
        }
        let big = n(fx) >= 0x124;
        fx.mem.write_u32(sp + 0x68, 0x3F81_47AE);
        fx.mem.write_u32(sp + 0x64, if big { 0x3F99_999A } else { 0x3F81_47AE });
        fx.mem.write_u32(sp + 0x60, 0x3F81_47AE);
        fx.particle_quads(D + 0x16D0, D + 0x1750, sp + 0x60);
        if n(fx) >= 0x124 {
            fx.step_down(D + 0x175B, 0, 2);
        }
    }
    if n(fx) >= 0xAB {
        fx.ribbon_draw(D + 0x70, D + 0xD0, D);
        fx.ribbon_draw(D + 0xA0, D + 0x2D0, D);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2BD8, m);
    let left = fx.mem.i32(D + 0x2BD4) - 1;
    fx.mem.set32(D + 0x2BD4, left);
    if m == 0x124 {
        2
    } else if m == 0xB8 {
        1
    } else if (0xC2..0x124).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
