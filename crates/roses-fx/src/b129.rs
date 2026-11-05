//! BMODULE b129 (card 371), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04B80, data area 0x1ED0000; compiled without
//! delay-slot scheduling) [F]. The launch prints
//! the data size (printf 0x19A410, answered by the runner). The target's
//! added colour (0x1F7790) brightening until update 295 (cleared then,
//! 0x1F7770); from update 110 a pulsing full-window tint (+0x6FC0) and the
//! frame quad (111..294); at a point 150 above the scene centre,
//! re-coloured every update (one random byte each, & 0x7F): the point-set
//! strip (+0x5B0 / look +0x650), two rings (+0x6B0 / +0xAB0), a quad burst
//! (+0x150), a spark burst (+0x2F80) thrown out again in random directions
//! and a falling sprite cloud (+0xF20) respawned below -100. Returns 1 on
//! update 158, 6 on 168..294, 2 on 295, 8 after 312.
//!
//! Library: `b262::tint` (0x1E00120), the frame quad (0x1E00280),
//! `sprites_spawn` (0x1E003A0), `sprites` (0x1E00530), `pointquad::setup`
//! / `draw` (0x1E01E20 / 0x1E02160), `ring_setup` / `ring_draw`
//! (0x1E02D40 / 0x1E03000), `spawn::scatter` (0x1E034F0), `trail::sparks`
//! (0x1E036B0), `quads_setup` / `quads_draw` (0x1E04410 / 0x1E04570).
use crate::b262::tint;
use crate::b436::ring_group;
use crate::b628::halves;
use crate::blib::{self, pointquad};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SETUP: u32 = 0x01E0_1E20;
/// The quad corner template (rodata, 0x1E04410's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_5780;
/// The launch's printf format (record rodata).
const FORMAT: u32 = 0x01E0_5800;

/// The entry 0x1E04B80.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.boundary(0x0019_A410, "printf", [FORMAT, 0x6FE0, 0, 0]);
    fx.bzero(D, 0x6FE0);
    for x in [0u32, 0x70, 0xE0, 0xEB0] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x6FC8u32, 0x12, 1), (0x6FCC, 1, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x6FB0, 0x138);
    fx.mem.set32(D + 0x6FB4, 1);
    fx.mem.set32(D + 0x6FB8, param);
    for (k, x) in [(0x6FD0u32, 0x80u8), (0x6FC2, 0x80), (0x6FC1, 0x80), (0x6FC0, 0x40), (0x6FC3, 0), (0x6FC6, 0xFF), (0x6FC5, 0xFF), (0x6FC4, 0xFF), (0x6FC7, 0)] {
        fx.mem.write_u8(D + k, x);
    }
    fx.texture4(sp + 0x90, 0xF);
    // The quad burst (+0x150): size 2.
    let g = D + 0x150;
    fx.mem.write_u32(g + 4, 0x8_0001);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    for (k, x) in [(0xAu32, 0xD0u8), (9, 0xA0), (8, 0x70), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.quads_setup(g, tex, 2.0, QUAD_TEMPLATE);
    // Two rings (+0x6B0: radii 24 / 22.4; +0xAB0: 24 / 25.6).
    for (g, outer) in [(0x6B0u32, 0x41B3_3333u32), (0xAB0, 0x41CC_CCCD)] {
        ring_group(fx, D + g, 0x2008_0000, 0x40, [0x70, 0xA0, 0xD0, 0]);
        let tex = fx.mem.read_u32(sp + 0x90);
        fx.ring_setup(D + g, tex, 24.0, f32::from_bits(outer), 0.0, 0.0);
    }
    // The point set's look (+0x650): cell (176, 32, 32, 32) of a 256 x 256
    // sheet from (176, 32), looping frame 0.
    let g = D + 0x650;
    fx.mem.write_u32(g + 0x38, 0x8_0001);
    halves(fx, g, &[(0, 0xB0), (2, 0x20), (4, 0x20), (6, 0x20), (8, 0xB0), (0xA, 0x20), (0xC, 0x100), (0xE, 0x100)]);
    fx.mem.write_u32(g + 0x30, 0x3F80_0000);
    for (k, x) in [(0x36u32, 0xD0u8), (0x35, 0xA0), (0x34, 0x70), (0x37, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    halves(fx, g, &[(0x48, 0), (0x4A, 0), (0x4C, 0), (0x4E, 1), (0x50, 0), (0x52, 0x3E7)]);
    fx.mem.write_u32(D + 0x5B8, 1);
    let tex = fx.mem.read_u32(sp + 0x90);
    pointquad::setup(fx, SETUP, D + 0x650, D + 0x5B0, tex, 32.0);
    // The spark burst (+0x2F80): 1024 sparks at +0x2FB0 (random
    // directions), width 0.8, length 30.
    let g = D + 0x2F80;
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 3);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x70), (8, 0x40), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0x20, 0x3F4C_CCCD);
    fx.mem.write_u32(g + 0x24, 0x41F0_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x60), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x60), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, D + 0x2FB0);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x400);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 4.0);
    fx.texture5(sp + 0x90, 0);
    // The sprite cloud (+0xF20): 512 sprites at +0xF80, size 10, radius
    // 10.
    let g = D + 0xF20;
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x5000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0xC, 0x4120_0000);
    fx.mem.write_u32(g + 0x20, 0);
    fx.mem.write_u32(g + 0x24, 0x800);
    fx.mem.write_u32(g + 0x28, 0);
    halves(fx, g, &[(0x30, 0), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0xF80);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4120_0000);
    halves(fx, g, &[(0x5A, 0x200), (0x5C, 8), (0x5E, 0)]);
    fx.sprites_spawn(g, 10.0);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = g + 0x60 + 16 * k as u32;
        let r = fx.rand();
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, h.wrapping_neg());
        fx.mem.write_u8(p + 0xC, 0);
        fx.mem.write_u8(p + 0xD, 0);
        let y = ee::add(fx.mem.f(p + 4), 20.0);
        fx.mem.setf(p + 4, y);
        k += 1;
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x6FB4);
    fx.mem.write_u32(D + 0x104, 0x4316_0000);
    let v = fx.read_v4(D + 0x100);
    fx.write_v4(sp + 0x60, v);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x70);
    fx.look_angles(sp + 0x60, sp + 0x70, D + 0xEC0);
    fx.midpoint(sp + 0x60, sp + 0x70, sp + 0x80);
    fx.midpoint(sp + 0x60, sp + 0x80, D + 0xED0);
    // s0 = D, s1 = the OT, s2 = sp + 0x80, s3 the caller's.
    let regs = [Some(D), Some(OT), Some(sp + 0x80), None];
    fx.xform_world_saving(D, 0x01E0_505C, regs);
    fx.xform_billboard_saving(D + 0x70, 0x01E0_5064, regs);
    fx.xform_billboard_saving(D + 0xE0, 0x01E0_506C, regs);
    fx.xform_world_saving(D + 0xEB0, 0x01E0_5074, regs);
    if (0x6F..0x127).contains(&n(fx)) {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) >= 0x6E {
        tint(fx, D + 0x6FC0);
        let m = n(fx);
        if m >= 0x127 {
            fx.step_down(D + 0x6FC3, 0, 8);
        } else if m >= 0x100 {
            fx.step_up(D + 0x6FC0, 0x80, 4);
            fx.step_up(D + 0x6FC3, 0xFF, 8);
        } else if bit(fx.mem.read_u32(D + 0x6FBC), 0x1F) {
            if fx.step_down(D + 0x6FC3, 0x20, 4) == 0 {
                let f = fx.mem.read_u32(D + 0x6FBC) & 0x7FFF_FFFF;
                fx.mem.write_u32(D + 0x6FBC, f);
            }
        } else if fx.step_up(D + 0x6FC3, 0x40, 4) == 0 {
            let f = fx.mem.read_u32(D + 0x6FBC) | 0x8000_0000;
            fx.mem.write_u32(D + 0x6FBC, f);
        }
    }
    if n(fx) < 0x127 {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, D + 0x6FC4, [D, OT], 0x01E0_519C);
        fx.step_up(D + 0x6FC7, 0xFF, 1);
    }
    if n(fx) == 0x127 {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, 0, [D, OT], 0x01E0_51C4);
    }
    if n(fx) >= 0x6E {
        for (c, ra) in [(2u32, 0x01E0_51DC), (1, 0x01E0_520C), (0, 0x01E0_523C)] {
            let v = (fx.rand_from(D, ra) & 0x7F) as u8;
            for g in [0x158u32, 0xAB8, 0x6B8, 0x684, 0x6FC4, 0x2F88] {
                fx.mem.write_u8(D + g + c, v);
            }
        }
        pointquad::draw(fx, OT, D + 0x650, D + 0x5B0, D + 0xE0);
        fx.step_up(D + 0x687, 0x60, 6);
        fx.ring_draw(D + 0x6B0, D + 0xE0);
        fx.step_up(D + 0x6BB, 0x20, 2);
        fx.ring_draw(D + 0xAB0, D + 0xE0);
        fx.step_up(D + 0xABB, 0x20, 2);
        fx.quads_draw(D + 0x150, D + 0xE0);
        fx.step_up(D + 0x15B, 0x20, 2);
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0x1B0 + 16 * k + 4 * c;
                let x = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, x);
            }
        }
        if n(fx) == 0x6E {
            fx.mem.write_u8(D + 0x2F8B, 0x80);
        }
        blib::trail::sparks(fx, OT, D + 0x2F80, D + 0xE0);
        if n(fx) >= 0x128 {
            fx.step_down(D + 0x2F8B, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x2FAC) {
            let p = D + 0x2FB0 + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) < 0 {
                continue;
            }
            let s = f32::from_bits(0x3FB3_3333);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 4 * c), s);
                fx.mem.setf(p + 4 * c, x);
            }
            if fx.mem.i16(p + 0xE) < 0x11 {
                continue;
            }
            fx.random_dir(sp + 0x50, 40.0);
            // 0x20BDA0's saves inside 0x1043F0 (s0 = the out pointer, s1 =
            // the OT, ra 0x104460): the sprites' stale TEX0 / TEX1.
            fx.saved(sp - 0xD0, &[(0x20, u64::from(sp + 0x50)), (0x30, u64::from(OT)), (0x40, 0x0010_4460)]);
            for (o, s) in [(4u32, 0x54u32), (8, 0x58), (0, 0x50)] {
                let w = fx.mem.read_u32(sp + s);
                fx.mem.write_u32(p + o, w);
            }
            let r = fx.rand_from(D, 0x01E0_53F8);
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0xE, h.wrapping_neg());
            let a = if n(fx) < 0x6E { 0 } else { 0x80 };
            fx.mem.write_u8(p + 0xC, a);
        }
        fx.sprites(D + 0xF20, D + 0xEB0);
        if n(fx) >= 0x128 {
            fx.step_down(D + 0xF2B, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0xF7A) {
            let p = D + 0xF80 + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) < 0 {
                continue;
            }
            let y = ee::sub(fx.mem.f(p + 4), 5.0);
            fx.mem.setf(p + 4, y);
            fx.step_up(p + 0xC, 0x80, 8);
            let s = f32::from_bits(if ee_le(fx.mem.f(p + 4), 0.0) { 0x3F99_999A } else { 0x3F4C_CCCD });
            for o in [8u32, 0] {
                let v = ee::mul(fx.mem.f(p + o), s);
                fx.mem.setf(p + o, v);
            }
            if !ee_lt(fx.mem.f(p + 4), -100.0) {
                continue;
            }
            let a = fx.rand_from(D, 0x01E0_5558) & 0xFFFF;
            let c = fx.tcos(a);
            let v = ee::mul(fx.mem.f(D + 0xF6C), c);
            fx.mem.setf(p + 8, v);
            let s = fx.tsin(a);
            let v = ee::mul(fx.mem.f(D + 0xF6C), s);
            fx.mem.setf(p, v);
            let r = fx.rand_from(D, 0x01E0_559C);
            let (_, h) = fx.div(r, 0xA);
            fx.mem.setf(p + 4, ee::add(20.0, ee::from_i32(h)));
            fx.mem.write_u8(p + 0xC, 0);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x6FB4, m);
    let left = fx.mem.i32(D + 0x6FB0) - 1;
    fx.mem.set32(D + 0x6FB0, left);
    if m == 0x127 {
        2
    } else if m == 0x9E {
        1
    } else if (0xA8..0x127).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
