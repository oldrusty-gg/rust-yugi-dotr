//! BMODULE b138 (card 112), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03C50, data area 0x1ED0000) [F] (helper C, 30 Oct
//! 2026): b263's family: a falling sprite shower (+0x8E0, until update 60)
//! and a disc (+0x380, updates 23..209) at the attacker's node 0x47, a
//! wobbling quad burst (+0x2A10) there; from update 70 a ribbon trail
//! (+0x70) toward the target (1 / 26 of the way per update, then 1 / 25
//! ...); at the target from update 86 three widening rings (+0x2FC0 +
//! 0x400 k, respawned at random angles) and a disc / quad burst growing to
//! 2, and from update 65 a rising sprite fountain (+0x1940); the frame
//! quad (white, alpha 0x60) from update 75; the stage light down to 0x20
//! until update 205. Returns 1 on update 76, 6 on 86..204, 2 on 205, 8
//! after 222.
//!
//! Library: `sprites_spawn` (0x1E00240), `sprites` (0x1E003D0),
//! `ring_setup` / `ring_draw` (0x1E01CC0 / 0x1E01F80), `disc` (0x1E02470),
//! `quads_setup` / `quads_draw` (0x1E029D0 / 0x1E02B30) [`crate::fxlib`],
//! the ribbon (0x1E03140 / 0x1E03290, [`crate::ribbon`]), the frame quad
//! (0x1E00120).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// b138 / b211's differences (b211: `crate::b211`).
pub struct Variant {
    /// The quad corner template (rodata, 0x1E029D0's lui / addiu).
    pub template: u32,
    pub nodes: [i32; 4],
    pub length: i32,
    /// The ribbon's approach divisor at the start.
    pub approach: i32,
    /// The discs', quads' and rings' colour (B, G, R bytes +0, +1, +2).
    pub bgr: [u8; 3],
    /// The target disc's (+0x860) flags; the ribbon's life; the shower's
    /// alpha; the target quad burst's (+0x400) flags and size; the rings'
    /// flags and radii.
    pub disc2_flags: u32,
    pub life: i32,
    pub shower_alpha: u8,
    pub quad_flags: u32,
    pub quad_size: f32,
    pub ring_flags: u32,
    pub ring_radii: (u32, u32),
}

pub const B138: Variant = Variant {
    template: 0x01E0_4B80,
    nodes: [0x47, 0x5A, 0x21, 0x30],
    length: 0xDE,
    approach: 0x1A,
    bgr: [0xD0, 0xA0, 0x70],
    disc2_flags: 0x8_0000,
    life: 0x5A,
    shower_alpha: 0x80,
    quad_flags: 0x8_0001,
    quad_size: 4.0,
    ring_flags: 0x8_0400,
    ring_radii: (0x41AC_0000, 0x41B4_0000),
};

/// The entry 0x1E03C50.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B138)
}

pub fn run(fx: &mut Fx, param: i32, v: &Variant) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param, v) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn disc_record(fx: &mut Fx, at: u32, flags: u32, bgr: [u8; 3], radius: u32) {
    fx.mem.write_u32(at, flags);
    fx.mem.write_u8(at + 6, bgr[2]);
    fx.mem.write_u8(at + 5, bgr[1]);
    fx.mem.write_u8(at + 4, bgr[0]);
    fx.mem.write_u8(at + 7, 0);
    fx.mem.write_u32(at + 8, radius);
    fx.mem.set16(at + 0xC, 0x24);
    fx.mem.set16(at + 0xE, 0);
}

/// A sprite group at `g` of 256 sprites at `parts`, size 1, radius `r`.
fn sprite_group(fx: &mut Fx, g: u32, tex: u32, parts: u32, r: u32) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0xC, 0x3F80_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    for (k, x) in [(0x30u32, 0x20), (0x32, 0x20), (0x34, 0x20), (0x36, 0x20), (0x38, 0x20), (0x3A, 0x20), (0x3C, 0x80), (0x3E, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u32(g + 0x40, parts);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, r);
    fx.mem.set16(g + 0x5A, 0x100);
    fx.mem.set16(g + 0x5C, 1);
    fx.mem.set16(g + 0x5E, 0);
}

/// A fountain sprite (`p` = D + 16 k, fields at +0x19A0) set off (as
/// b263's, radius from +0x198C).
pub(crate) fn fountain_spawn(fx: &mut Fx, p: u32, ra: u32) {
    let r = fx.mem.f(D + 0x198C);
    let f = ee::mul(10.0, r);
    fx.fix_residue(f, ra);
    let m = ee::fixsfsi(f);
    let a = fx.rand() & 0xFFFF;
    let x = fx.rand();
    let (_, h) = fx.div(x, m);
    let tenth = f32::from_bits(0x3DCC_CCCD);
    let radius = ee::mul(tenth, ee::from_i32(h));
    let x = fx.rand();
    let (_, h) = fx.div(x, 0x28);
    fx.mem.setf(p + 0x19A4, ee::mul(tenth, ee::neg(ee::from_i32(h - 0x14))));
    let c = fx.tcos(a);
    fx.mem.setf(p + 0x19A8, ee::mul(radius, c));
    let s = fx.tsin(a);
    fx.mem.setf(p + 0x19A0, ee::mul(radius, s));
    let x = fx.rand();
    let (_, h) = fx.div(x, 0xC);
    fx.mem.set16(p + 0x19AE, -h);
    fx.mem.write_u8(p + 0x19AC, 0x80);
}

fn quad_group(fx: &mut Fx, g: u32, flags: u32, tex: u32, size: f32, v: &Variant) {
    fx.mem.write_u32(g + 4, flags);
    for (k, x) in [(0x10u32, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u8(g + 0xA, v.bgr[2]);
    fx.mem.write_u8(g + 9, v.bgr[1]);
    fx.mem.write_u8(g + 8, v.bgr[0]);
    fx.mem.write_u8(g + 0xB, 0x20);
    fx.quads_setup(g, tex, size, v.template);
}

pub(crate) fn init(fx: &mut Fx, sp: u32, param: i32, v: &Variant) -> i32 {
    fx.bzero(D, 0x3C00);
    for x in [0u32, 0x2A0, 0x870, 0x310, 0x390, 0x29A0] {
        fx.xform_init(D + x);
    }
    for k in 0..3u32 {
        fx.xform_init(D + 0x2E70 + 0x70 * k);
    }
    for (at, node, clear) in [(0x3BE8u32, v.nodes[0], 1), (0x3BEC, v.nodes[1], 0), (0x3BF0, v.nodes[2], 0), (0x3BF4, v.nodes[3], 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x3BC0, v.length);
    fx.mem.set32(D + 0x3BC4, 1);
    fx.mem.set32(D + 0x3BC8, param);
    fx.mem.set32(D + 0x3BCC, 0x80);
    fx.mem.set32(D + 0x3BD0, v.approach);
    disc_record(fx, D + 0x380, 0x8_0000, v.bgr, 0x40A0_0000);
    disc_record(fx, D + 0x860, v.disc2_flags, v.bgr, 0x4190_0000);
    fx.texture5(sp + 0xA0, 0);
    // The ribbon (+0x70): 32 slots, life 90, width 100, cell (16, 0, 16,
    // 32); points at +0xA0.
    let g = D + 0x70;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 8, 0x8003);
    for k in [6u32, 5, 4, 7] {
        fx.mem.write_u8(g + k, 0x80);
    }
    for (k, x) in [(0x10u32, 0x10), (0x12, 0), (0x14, 0x10), (0x16, 0x20), (0x18, 0x10), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u32(g + 0xC, 0x42C8_0000);
    for (k, x) in [(0x22u32, 8), (0x24, 0), (0x26, v.life), (0x28, 0x20), (0x2A, 0), (0x2C, 1)] {
        fx.mem.set16(g + k, x);
    }
    // The shower (+0x8E0): 256 sprites spawned round the node, 10 higher.
    let tex = fx.mem.read_u32(sp + 0xA0);
    sprite_group(fx, D + 0x8E0, tex, D + 0x940, 0x4170_0000);
    fx.mem.write_u8(D + 0x8EB, v.shower_alpha);
    fx.sprites_spawn(D + 0x8E0, 1.0);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x93A) {
        let p = D + 16 * k as u32;
        let r = fx.rand();
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0x94E, -h);
        fx.mem.write_u8(p + 0x94C, 0x30);
        fx.mem.write_u8(p + 0x94D, 0);
        let y = ee::add(fx.mem.f(p + 0x944), 10.0);
        fx.mem.setf(p + 0x944, y);
        k += 1;
    }
    // The fountain (+0x1940): radius 2.
    let tex = fx.mem.read_u32(sp + 0xA0);
    sprite_group(fx, D + 0x1940, tex, D + 0x19A0, 0x4000_0000);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x199A) {
        fountain_spawn(fx, D + 16 * k as u32, 0x01E0_3FF0);
        k += 1;
    }
    fx.texture4(sp + 0xA0, 0xF);
    let tex = fx.mem.read_u32(sp + 0xA0);
    quad_group(fx, D + 0x400, v.quad_flags, tex, v.quad_size, v);
    let tex = fx.mem.read_u32(sp + 0xA0);
    quad_group(fx, D + 0x2A10, 1, tex, 0.5, v);
    for k in 0..64u32 {
        let q = D + 16 * k;
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x3000);
        fx.mem.set32(q + 0x2A70, h - 0x1800);
        let r = fx.rand();
        fx.mem.set32(q + 0x2A74, r & 0xFFFF);
        fx.mem.write_u32(q + 0x2A78, 0);
    }
    // Three rings (+0x2FC0 + 0x400 k): radii 21.5 / 22.5.
    for s in 0..3u32 {
        let g = D + 0x400 * s + 0x2FC0;
        fx.mem.write_u32(g + 4, v.ring_flags);
        for (k, x) in [(0x10u32, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x80)] {
            fx.mem.set16(g + k, x);
        }
        fx.mem.write_u8(g + 0xA, v.bgr[2]);
        fx.mem.write_u8(g + 9, v.bgr[1]);
        fx.mem.write_u8(g + 8, v.bgr[0]);
        fx.mem.write_u8(g + 0xB, 0x80);
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(g, tex, f32::from_bits(v.ring_radii.0), f32::from_bits(v.ring_radii.1), 0.0, 0.0);
    }
    for (x, val) in [(0x390u32, 0x3DCC_CCCDu32), (0x29A0, 0x3DCC_CCCD), (0x2E70, 0), (0x2EE0, 0x3F00_0000), (0x2F50, 0x3F80_0000)] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, val);
        }
    }
    0
}

/// The step's first part, shared with b211: nodes, look angles, unit
/// positions, the ribbon's push and the transforms (`ra`: the return
/// addresses of the 7 transform calls, the last one in a loop).
pub(crate) fn step_head(fx: &mut Fx, sp: u32, ra: [u32; 7]) {
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    node(fx, 0x3BE8, sp + 0x80);
    node(fx, 0x3BEC, sp + 0x90);
    node(fx, 0x3BEC, D + 0x29C0);
    fx.look_angles(sp + 0x80, sp + 0x90, D + 0x880);
    for k in 0..4u32 {
        let w = fx.mem.read_u32(D + 0x880 + 4 * k);
        fx.mem.write_u32(D + 0x29B0 + 4 * k, w);
    }
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x3BD4);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x3B0);
    for k in 0..3u32 {
        let t = fx.target_handle();
        fx.unit_position(t, D + 0x2E90 + 0x70 * k);
    }
    node(fx, 0x3BE8, D + 0x890);
    node(fx, 0x3BE8, D + 0x330);
    if fx.mem.i32(D + 0x3BC4) < 0x47 {
        node(fx, 0x3BE8, D + 0x2C0);
    }
    if fx.mem.i32(D + 0x3BC4) >= 0x46 {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x2C0);
        let n = fx.mem.i32(D + 0x3BD0);
        fx.approach_by(D + 0x2C0, D + 0x3BD4, n);
        let n = fx.mem.i32(D + 0x3BD0);
        if n != 0 {
            fx.mem.set32(D + 0x3BD0, n - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 / s3 as the unit-position loop left them
    // (D + 0x150, 3); then the loop's D + 0x70 k and k.
    let regs = [Some(D), Some(OT), Some(D + 0x150), Some(3)];
    fx.xform_world_saving(D, ra[0], regs);
    fx.xform_billboard_saving(D + 0x2A0, ra[1], regs);
    fx.xform_world_saving(D + 0x870, ra[2], regs);
    fx.xform_billboard_saving(D + 0x310, ra[3], regs);
    fx.xform_billboard_saving(D + 0x390, ra[4], regs);
    fx.xform_world_saving(D + 0x29A0, ra[5], regs);
    for k in 0..3u32 {
        fx.xform_world_saving(D + 0x2E70 + 0x70 * k, ra[6], [Some(D), Some(OT), Some(D + 0x70 * k), Some(k)]);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    step_head(fx, sp, [0x01E0_4434, 0x01E0_443C, 0x01E0_4444, 0x01E0_444C, 0x01E0_4454, 0x01E0_445C, 0x01E0_446C]);
    if fx.mem.i32(D + 0x3BC4) >= 0x4B {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if fx.mem.i32(D + 0x3BC4) < 0xCD {
        fx.step_down(D + 0x3BCC, 0x20, 4);
    } else {
        fx.step_up(D + 0x3BCC, 0x80, 4);
    }
    let light = fx.mem.u8(D + 0x3BCC);
    fx.set_stage_light(light);
    if fx.mem.i32(D + 0x3BC4) >= 0x56 {
        for s in 0..3u32 {
            let (x, g) = (D + 0x2E70 + 0x70 * s, D + 0x2FC0 + 0x400 * s);
            fx.ring_draw(g, x);
            fx.step_down(g + 0xB, 0, 8);
            if fx.approach(x, 2.0, f32::from_bits(0x3DCC_CCCD)) == 0 {
                for k in [8u32, 4, 0] {
                    fx.mem.write_u32(x + k, 0x3DCC_CCCD);
                }
                for k in [0x10u32, 0x14, 0x18] {
                    let r = fx.rand();
                    fx.mem.set32(x + k, r & 0xFFFF);
                }
                if fx.mem.i32(D + 0x3BC4) < 0xCD {
                    fx.mem.write_u8(g + 0xB, 0x80);
                }
            }
        }
        if fx.mem.i32(D + 0x3BC4) >= 0x66 {
            fx.approach(D + 0x390, 2.0, f32::from_bits(0x3D4C_CCCD));
        }
        fx.disc(D + 0x860, D + 0x390);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_4610)]);
        fx.quads_draw(D + 0x400, D + 0x390);
        if fx.mem.i32(D + 0x3BC4) >= 0xBE {
            fx.step_down(D + 0x867, 0, 0x10);
            fx.step_down(D + 0x40B, 0, 2);
        } else {
            fx.step_up(D + 0x867, 0xFF, 8);
        }
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0x460 + 16 * k + 4 * c;
                let x = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, x);
            }
        }
    }
    let n = fx.mem.i32(D + 0x3BC4);
    if (0x17..0xD2).contains(&n) {
        fx.disc(D + 0x380, D + 0x310);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_46D4)]);
        fx.step_up(D + 0x387, 0xFF, 8);
    }
    fx.quads_draw(D + 0x2A10, D + 0x29A0);
    fx.approach(D + 0x29A0, 1.0, f32::from_bits(0x3D4C_CCCD));
    for k in 0..64u32 {
        let a = D + 0x2A74 + 16 * k;
        let x = fx.mem.i32(a);
        fx.mem.set32(a, if k % 2 == 0 { x.wrapping_add(0x40) } else { x.wrapping_sub(0x40) });
    }
    if fx.mem.i32(D + 0x3BC4) < 0x3D {
        fx.sprites(D + 0x8E0, D + 0x870);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x93A) {
            let p = D + 16 * k as u32;
            if fx.mem.i16(p + 0x94E) >= 0 {
                let y = ee::sub(fx.mem.f(p + 0x944), 1.0);
                fx.mem.setf(p + 0x944, y);
                fx.step_up(p + 0x94C, 0x80, 8);
                if !ee_le(fx.mem.f(p + 0x944), 0.0) {
                    let f = f32::from_bits(0x3F4C_CCCD);
                    let z = ee::mul(fx.mem.f(p + 0x948), f);
                    fx.mem.setf(p + 0x948, z);
                    let x = ee::mul(fx.mem.f(p + 0x940), f);
                    fx.mem.setf(p + 0x940, x);
                }
                if ee_lt(fx.mem.f(p + 0x944), 0.0) {
                    let a = fx.rand() & 0xFFFF;
                    let c = fx.tcos(a);
                    let z = ee::mul(fx.mem.f(D + 0x92C), c);
                    fx.mem.setf(p + 0x948, z);
                    let s = fx.tsin(a);
                    let x = ee::mul(fx.mem.f(D + 0x92C), s);
                    fx.mem.setf(p + 0x940, x);
                    let r = fx.rand();
                    let (_, h) = fx.div(r, 0xA);
                    fx.mem.setf(p + 0x944, ee::add(10.0, ee::from_i32(h)));
                    fx.mem.write_u8(p + 0x94C, 0);
                }
            }
            k += 1;
        }
    }
    if fx.mem.i32(D + 0x3BC4) >= 0x41 {
        fx.sprites(D + 0x1940, D + 0x870);
        let grow = f32::from_bits(0x3F99_999A);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x199A) {
            let p = D + 16 * k as u32;
            if fx.mem.i16(p + 0x19AE) >= 0 {
                let y = ee::add(fx.mem.f(p + 0x19A4), 2.0);
                fx.mem.setf(p + 0x19A4, y);
                let x = ee::mul(fx.mem.f(p + 0x19A0), grow);
                fx.mem.setf(p + 0x19A0, x);
                let z = ee::mul(fx.mem.f(p + 0x19A8), grow);
                fx.mem.setf(p + 0x19A8, z);
                fx.step_down(p + 0x19AC, 0, 8);
                if !ee_le(fx.mem.f(p + 0x19A4), 20.0) {
                    fountain_spawn(fx, p, 0x01E0_4964);
                }
            }
            k += 1;
        }
    }
    if fx.mem.i32(D + 0x3BC4) >= 0x47 {
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
    }
    let n = fx.mem.i32(D + 0x3BC4) + 1;
    fx.mem.set32(D + 0x3BC4, n);
    let left = fx.mem.i32(D + 0x3BC0) - 1;
    fx.mem.set32(D + 0x3BC0, left);
    if n == 0xCD {
        2
    } else if n == 0x4C {
        1
    } else if (0x56..0xCD).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
