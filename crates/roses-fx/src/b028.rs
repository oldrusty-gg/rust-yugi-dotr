//! BMODULE b028 (card 213), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03CE0, data area 0x1ED0000; compiled without
//! delay-slot scheduling) [F]. Six rings of the
//! quad trail (+0x70, `blib::quadtrail`) pushed at the attacker's nodes
//! 0x39 (updates 39, 53, 80) and 0x30 (45, 60, 79); from update 78 at
//! node 0x27: a cloud of 60 squares (+0x2250, `pointquad::squares`)
//! spreading by 1.03 a step, two rings (+0x1890 / +0x1C90), a point-set
//! strip (+0x1790 / look +0x1830, `pointquad::draw`) and a quad burst
//! (+0x1330) growing by 0.4 to 1 then by 0.005 while everything fades,
//! with a particle cloud (+0x2090). Returns 1 on update 67, 2 on 74, 8
//! after 144 (never 6).
//!
//! Library: `pointquad::setup` / `draw` / `squares` (0x1E00120 /
//! 0x1E00460 / 0x1E01040), `ring_setup` / `ring_draw` (0x1E01620 /
//! 0x1E018E0), `particle_quads` (0x1E01DD0), `quadtrail::push` / `draw`
//! (0x1E02960 / 0x1E02B60), `quads_setup` / `quads_draw` (0x1E03570 /
//! 0x1E036D0).
use crate::b436::ring_group;
use crate::b628::{halves, trail_fade, trail_group};
use crate::blib::{pointquad, quadtrail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SETUP: u32 = 0x01E0_0120;
/// The quad corner template (rodata, 0x1E03570's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_4840;
const TRAIL_DRAW: u32 = 0x01E0_2B60;

/// The entry 0x1E03CE0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x60);
    ret
}

/// The squares' 60 points (`g` + 0x20 + 12 k): (rand % 50 - 25, rand %
/// 25, rand % 50 - 25), or with flag 0x1D (rand % 100 - 50, rand % 100 -
/// 50, rand % 50) [F].
pub(crate) fn square_points(fx: &mut Fx, g: u32) {
    for k in 0..60u32 {
        let p = g + 12 * k;
        let wide = bit(fx.mem.read_u32(g + 4), 0x1D);
        let (m, mid, order) = if wide { (100, 50, [0x20u32, 0x28, 0x24]) } else { (50, 25, [0x20, 0x24, 0x28]) };
        for (i, o) in order.into_iter().enumerate() {
            let r = fx.rand();
            let (_, h) = fx.div(r, if i == 1 { m / 2 } else { m });
            let v = if i == 1 { h } else { h - mid };
            fx.mem.setf(p + o, ee::from_i32(v));
        }
    }
}

/// A point set's look `g` (b028 +0x1830, b142 +0x570): cell (128, 0, 32,
/// 32) of a 256 x 256 sheet from (176, 32), colour (0x70, 0xA0, 0xD0,
/// 0x60), looping frame 0 [F].
pub(crate) fn look_init(fx: &mut Fx, g: u32) {
    fx.mem.write_u32(g + 0x38, 0x8_0001);
    halves(fx, g, &[(0, 0x80), (2, 0), (4, 0x20), (6, 0x20), (8, 0xB0), (0xA, 0x20), (0xC, 0x100), (0xE, 0x100)]);
    fx.mem.write_u32(g + 0x30, 0x3F80_0000);
    for (k, x) in [(0x36u32, 0xD0u8), (0x35, 0xA0), (0x34, 0x70), (0x37, 0x60)] {
        fx.mem.write_u8(g + k, x);
    }
    halves(fx, g, &[(0x48, 0), (0x4A, 0), (0x4C, 0), (0x4E, 1), (0x50, 0), (0x52, 0x3E7)]);
}

/// A particle cloud `g` of 16 particles (`particle_quads`; half size 6,
/// 8 frames): x +- 5 (rand % 10 - 5), y (rand & 7) - 2 about +0x10 /
/// +0x14, frame -3 k [F].
pub(crate) fn cloud16(fx: &mut Fx, g: u32, tex: u32, bgra: [u8; 4]) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x81);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0xF0), (0x2A, 0), (0x24, 0x10), (0x26, 0x10), (0x2C, 0x100), (0x2E, 0x100)]);
    fx.mem.write_u32(g + 0xC, 0x40C0_0000);
    halves(fx, g, &[(0x130, 8), (0x13A, 3), (0x13C, 1), (0x140, 8), (0x136, 0xA), (0x138, 0xA)]);
    for (k, x) in [(0xAu32, bgra[2]), (9, bgra[1]), (8, bgra[0]), (0xB, bgra[3])] {
        fx.mem.write_u8(g + k, x);
    }
    cloud16_spawn(fx, g);
}

/// 16 particles of the cloud `g` placed (the loop of [`cloud16`]).
pub(crate) fn cloud16_spawn(fx: &mut Fx, g: u32) {
    for k in 0..16u32 {
        let p = g + 0x30 + 16 * k;
        let r = fx.rand();
        let (_, h) = fx.div(r, 0xA);
        let v = ee::add(fx.mem.f(g + 0x10), ee::from_i32(h - 5));
        fx.mem.setf(p, v);
        let r = fx.rand();
        let v = ee::add(fx.mem.f(g + 0x14), ee::from_i32((r & 7) - 2));
        fx.mem.setf(p + 4, v);
        let z = fx.mem.read_u32(g + 0x18);
        fx.mem.write_u32(p + 8, z);
        let a = -i32::from(fx.mem.i16(g + 0x13A));
        let f = fx.mult(a, k as i32);
        fx.mem.set16(p + 0xE, f);
        fx.mem.set16(p + 0xC, 0);
        let t = fx.mem.i16(g + 0x140) - 1;
        fx.mem.set16(g + 0x13E, t as i32);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x28F0);
    for x in [0u32, 0x12C0, 0x21E0] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x255Cu32, 0x27, 1), (0x2560, 0x39, 0), (0x2564, 0x30, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2550, 0x91);
    fx.mem.set32(D + 0x2554, 1);
    fx.mem.set32(D + 0x2558, param);
    for k in [0x2540u32, 0x2544, 0x2548] {
        fx.mem.write_u32(D + k, 0x3F83_D70A);
    }
    fx.texture4(sp + 0x50, 0xF);
    // The squares (+0x2250): 60 points, half size 0.6.
    let g = D + 0x2250;
    let tex = fx.mem.read_u32(sp + 0x50);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 0xC, 0x3F19_999A);
    square_points(fx, g);
    fx.mem.write_u32(g + 4, 1);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x40), (0x14, 0x20), (0x16, 0x20)]);
    for (k, x) in [(0xAu32, 0xD0u8), (9, 0xA0), (8, 0x70), (0xB, 0x80)] {
        fx.mem.write_u8(g + k, x);
    }
    // The quad burst (+0x1330): size 1.6.
    let g = D + 0x1330;
    fx.mem.write_u32(g + 4, 0x8_0001);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    for (k, x) in [(0xAu32, 0xD0u8), (9, 0xA0), (8, 0x70), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, x);
    }
    let tex = fx.mem.read_u32(sp + 0x50);
    fx.quads_setup(g, tex, f32::from_bits(0x3FCC_CCCD), QUAD_TEMPLATE);
    // Two rings (+0x1890: radii 12 / 11.2; +0x1C90: 12 / 12.8).
    ring_group(fx, D + 0x1890, 0x2008_0000, 0x40, [0x70, 0xA0, 0xD0, 0x20]);
    let tex = fx.mem.read_u32(sp + 0x50);
    fx.ring_setup(D + 0x1890, tex, 12.0, f32::from_bits(0x4133_3333), 0.0, 0.0);
    ring_group(fx, D + 0x1C90, 0x2008_0000, 0x40, [0x70, 0xA0, 0xD0, 0x30]);
    let tex = fx.mem.read_u32(sp + 0x50);
    fx.ring_setup(D + 0x1C90, tex, 12.0, f32::from_bits(0x414C_CCCD), 0.0, 0.0);
    // The point set's look (+0x1830): cell (128, 0, 32, 32) of a 256 x 256
    // sheet from (176, 32), looping frame 0.
    look_init(fx, D + 0x1830);
    fx.mem.write_u32(D + 0x1798, 1);
    let tex = fx.mem.read_u32(sp + 0x50);
    pointquad::setup(fx, SETUP, D + 0x1830, D + 0x1790, tex, 20.0);
    fx.texture4(sp + 0x50, 0xF);
    // The particle cloud (+0x2090): 8 particles, half size 6.
    let tex = fx.mem.read_u32(sp + 0x50);
    cloud16(fx, D + 0x2090, tex, [0x40, 0x20, 0x80, 0x60]);
    let tex = fx.mem.read_u32(sp + 0x50);
    trail_group(fx, D + 0x70, tex);
    fx.mem.set16(D + 0xB6, 0x24);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2554);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    if n(fx) < 0x4F {
        node(fx, 0x255C, D + 0x12E0);
        node(fx, 0x255C, D + 0x2200);
    }
    if [0x2D, 0x3C, 0x4F].contains(&n(fx)) {
        node(fx, 0x2564, sp + 0x40);
        quadtrail::push(fx, D + 0x70, sp + 0x40);
    }
    if [0x27, 0x35, 0x50].contains(&n(fx)) {
        node(fx, 0x2560, sp + 0x40);
        quadtrail::push(fx, D + 0x70, sp + 0x40);
    }
    // s0 = D, s1 = the OT, s2 the param (-1, 64 bits), s3 the caller's.
    let regs = [Some(D), Some(OT), None, None];
    fx.xform_world_saving(D, 0x01E0_43F0, regs);
    fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
    for (x, ra) in [(0x12C0u32, 0x01E0_43F8), (0x21E0, 0x01E0_4400)] {
        fx.xform_billboard_saving(D + x, ra, regs);
        fx.saved(sp - 0xF0, &[(0x40, u64::MAX)]);
    }
    if n(fx) >= 0x4E {
        pointquad::squares(fx, OT, D + 0x2250, D + 0x21E0, D + 0x2540);
        fx.step_down(D + 0x225B, 0, 0xA);
        fx.ring_draw(D + 0x1890, D + 0x12C0);
        fx.ring_draw(D + 0x1C90, D + 0x12C0);
        pointquad::draw(fx, OT, D + 0x1830, D + 0x1790, D + 0x12C0);
        fx.quads_draw(D + 0x1330, D + 0x12C0);
        if ee_lt(fx.mem.f(D + 0x21E0), 1.0) {
            let v = ee::add(fx.mem.f(D + 0x21E8), 0.5);
            for k in [0x21E8u32, 0x21E4, 0x21E0] {
                fx.mem.setf(D + k, v);
            }
        }
        let grow = ee_le(fx.mem.f(D + 0x12C0), 1.0);
        let step = f32::from_bits(if grow { 0x3ECC_CCCD } else { 0x3BA3_D70A });
        for k in [0x12C0u32, 0x12C4, 0x12C8] {
            let v = ee::add(fx.mem.f(D + k), step);
            fx.mem.setf(D + k, v);
        }
        if !grow {
            for (k, s) in [(0x133Bu32, 6), (0x189B, 6), (0x1C9B, 6), (0x1867, 0xC)] {
                fx.step_down(D + k, 0, s);
            }
            for k in [0x48u32, 0x44, 0x40] {
                fx.mem.write_u32(sp + k, 0x3F82_8F5C);
            }
            fx.particle_quads(D + 0x12C0, D + 0x2090, sp + 0x40);
            fx.step_down(D + 0x209B, 0, 4);
        }
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0x1390 + 16 * k + 4 * c;
                let x = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, x);
            }
        }
    }
    quadtrail::draw(fx, TRAIL_DRAW, OT, D + 0x70, D);
    trail_fade(fx, D + 0x70);
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2554, m);
    let left = fx.mem.i32(D + 0x2550) - 1;
    fx.mem.set32(D + 0x2550, left);
    if m == 0x4B {
        2
    } else if m == 0x44 {
        1
    } else if left > 0 {
        0
    } else {
        8
    }
}
