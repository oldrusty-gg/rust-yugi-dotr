//! BMODULE b013 (card 210), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04510, data area 0x1ED0000; compiled without
//! delay-slot scheduling) [F]: b028's burst with
//! falling streaks. Seven rings of the quad trail (+0x70) pushed at the
//! attacker's nodes 0x13 (updates 10, 30, 50, 62) and 0xC (20, 40, 56);
//! until update 63 a point 50 above node 0x3D sets the scene's camera
//! slots 4 / 5 (0x1F7E20, +0x29C0) while, from update 41, two rings
//! (+0x1DB0 / +0x21B0) and 64 streaks (+0x25B0, `blib::rain`) fade in
//! there; from update 63 they fade out (the slots cleared) and at node
//! 0x3D the squares (+0x1A50), the point-set strip (+0x1790 / look
//! +0x1830) and a quad burst (+0x1330) grow, with a particle cloud
//! (+0x1890). Returns 1 on update 49, 3 (!) on 59, 8 after 110.
//!
//! Library: `rain::colours2` (0x1E00080), `pointquad::setup` / `draw` /
//! `squares` (0x1E001A0 / 0x1E004E0 / 0x1E010C0), `ring_setup` /
//! `ring_draw` (0x1E016A0 / 0x1E01960), `particle_quads` (0x1E01E50),
//! `quadtrail::push` / `draw` (0x1E029E0 / 0x1E02BE0), `quads_setup` /
//! `quads_draw` (0x1E035F0 / 0x1E03750), `rain::setup` / `draw`
//! (0x1E03D60 / 0x1E03FB0).
//!
//! Written by helper C4 (31 Oct 2026); finished by helper C5: the quad
//! trail keys on the stale entry sp - 0x114, the w that 0x204D80 leaves in
//! `rain::draw` (now written there), or after update 63 the 0 upper half of
//! the library prologues' `sq` (written below) [run].
use crate::b028::{cloud16, look_init, square_points};
use crate::b142::fstep_up;
use crate::b436::ring_group;
use crate::b628::{halves, trail_fade, trail_group};
use crate::blib::{self, pointquad, quadtrail, rain};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SETUP: u32 = 0x01E0_01A0;
const RAIN_SETUP: u32 = 0x01E0_3D60;
const RAIN_DRAW: u32 = 0x01E0_3FB0;
/// The quad corner template (rodata, 0x1E035F0's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_5240;
const TRAIL_DRAW: u32 = 0x01E0_2BE0;

/// The entry 0x1E04510.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2A20);
    for x in [0u32, 0x12C0, 0x19E0, 0x1D40] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x29FCu32, 0x3D, 1), (0x2A00, 0x3E, 0), (0x2A04, 0x13, 0), (0x2A08, 0xC, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x29F0, 0x6E);
    fx.mem.set32(D + 0x29F4, 1);
    fx.mem.set32(D + 0x29F8, param);
    for k in [0x2A10u32, 0x2A11, 0x2A12, 0x2A0F, 0x2A0C, 0x2A0D, 0x2A0E] {
        fx.mem.write_u8(D + k, 0x80);
    }
    fx.mem.write_u8(D + 0x2A13, 0);
    fx.texture4(sp + 0x70, 0xF);
    // The squares (+0x1A50): half size 0.5.
    let g = D + 0x1A50;
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 0xC, 0x3F00_0000);
    square_points(fx, g);
    fx.mem.write_u32(g + 4, 1);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x40), (0x14, 0x20), (0x16, 0x20)]);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    // The quad burst (+0x1330): size 3.2.
    let g = D + 0x1330;
    fx.mem.write_u32(g + 4, 0x8_0001);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    for (k, x) in [(0xAu32, 0xD0u8), (9, 0xA0), (8, 0x70), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, x);
    }
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.quads_setup(g, tex, f32::from_bits(0x404C_CCCD), QUAD_TEMPLATE);
    // Two rings (+0x1DB0: radii 0.1 / 12 at -2, width 18; +0x21B0: 0.1 / 6
    // at -1, width 9).
    for (g, outer, h0, w) in [(0x1DB0u32, 12.0f32, -2.0f32, 18.0f32), (0x21B0, 6.0, -1.0, 9.0)] {
        ring_group(fx, D + g, 0x8000, 0x40, [0xA0, 0x80, 0x60, 0]);
        let tex = fx.mem.read_u32(sp + 0x70);
        fx.ring_setup(D + g, tex, f32::from_bits(0x3DCC_CCCD), outer, h0, w);
    }
    for (k, v) in [(0x29C4u32, 0x3F00_0000u32), (0x29C8, 0x3F40_0000), (0x29CC, 0x3F80_0000), (0x29C0, 0x3F80_0000), (0x29E0, 0), (0x29E4, 0x4348_0000), (0x29E8, 0x3F80_0000)] {
        fx.mem.write_u32(D + k, v);
    }
    look_init(fx, D + 0x1830);
    fx.mem.write_u32(D + 0x1798, 1);
    let tex = fx.mem.read_u32(sp + 0x70);
    pointquad::setup(fx, SETUP, D + 0x1830, D + 0x1790, tex, 40.0);
    let tex = fx.mem.read_u32(sp + 0x70);
    cloud16(fx, D + 0x1890, tex, [0x40, 0x20, 0x70, 0x60]);
    fx.mem.write_u32(D + 0x1894, 0x8_0081);
    // The streaks (+0x25B0): 64, speed 10.
    let g = D + 0x25B0;
    fx.mem.write_u32(g, 0x8_0001);
    for (k, x) in [(6u32, 0x80u8), (5, 0x80), (4, 0xE0), (7, 0x80)] {
        fx.mem.write_u8(g + k, x);
    }
    rain::setup(fx, RAIN_SETUP, g, 0x40, 10.0);
    let tex = fx.mem.read_u32(sp + 0x70);
    trail_group(fx, D + 0x70, tex);
    fx.mem.set16(D + 0xB6, 0x24);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x29F4);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    if n(fx) < 0x40 {
        node(fx, 0x29FC, sp + 0x50);
        node(fx, 0x2A00, sp + 0x60);
        fx.look_angles(sp + 0x60, sp + 0x50, D + 0x1D50);
        for c in 0..3u32 {
            let v = fx.mem.read_u32(sp + 0x50 + 4 * c);
            fx.mem.write_u32(D + 0x29D0 + 4 * c, v);
            fx.mem.write_u32(D + 0x1D60 + 4 * c, v);
        }
        fx.mem.write_u32(D + 0x29D4, 0x4248_0000);
        node(fx, 0x29FC, D + 0x12E0);
        node(fx, 0x29FC, D + 0x1A00);
    }
    if [0x14, 0x28, 0x38].contains(&n(fx)) {
        node(fx, 0x2A08, sp + 0x50);
        quadtrail::push(fx, D + 0x70, sp + 0x50);
    }
    if [0xA, 0x1E, 0x32, 0x3E].contains(&n(fx)) {
        node(fx, 0x2A04, sp + 0x50);
        quadtrail::push(fx, D + 0x70, sp + 0x50);
    }
    // s0 = D, s1 = the OT, s2 the param (-1, 64 bits), s3 the caller's.
    let regs = [Some(D), Some(OT), None, None];
    fx.xform_world_saving(D, 0x01E0_4D28, regs);
    fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
    for (x, ra) in [(0x12C0u32, 0x01E0_4D30), (0x19E0, 0x01E0_4D38)] {
        fx.xform_billboard_saving(D + x, ra, regs);
        fx.saved(sp - 0xF0, &[(0x40, u64::MAX)]);
    }
    fx.xform_world_saving(D + 0x1D40, 0x01E0_4D40, regs);
    fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
    if n(fx) >= 0x29 {
        fx.ring_draw(D + 0x1DB0, D + 0x1D40);
        fx.ring_draw(D + 0x21B0, D + 0x1D40);
        rain::draw(fx, RAIN_DRAW, OT, D + 0x25B0, D + 0x1D40, 2.5);
        if n(fx) < 0x3F {
            fx.step_up(D + 0x1DBB, 0x40, 8);
            fx.step_up(D + 0x21BB, 0x40, 8);
            fx.step_up(D + 0x25B7, 0x80, 0x10);
            blib::exe::camera_slots45(fx, D + 0x29C0);
        } else {
            fstep_up(fx, D + 0x1D44, 3.0, 0.5);
            fx.step_down(D + 0x1DBB, 0, 4);
            fx.step_down(D + 0x21BB, 0, 4);
            fx.step_down(D + 0x25B7, 0, 0x20);
            blib::exe::camera_slots45(fx, 0);
        }
    }
    if n(fx) >= 0x3F {
        if ee_lt(fx.mem.f(D + 0x19E0), 1.0) {
            let v = ee::add(fx.mem.f(D + 0x19E8), 0.5);
            for k in [0x19E8u32, 0x19E4, 0x19E0] {
                fx.mem.setf(D + k, v);
            }
        }
        for k in [0x58u32, 0x54, 0x50] {
            fx.mem.write_u32(sp + k, 0x3F83_D70A);
        }
        pointquad::squares(fx, OT, D + 0x1A50, D + 0x19E0, sp + 0x50);
        fx.step_down(D + 0x1A5B, 0, 0xA);
        pointquad::draw(fx, OT, D + 0x1830, D + 0x1790, D + 0x12C0);
        fx.quads_draw(D + 0x1330, D + 0x12C0);
        let grow = ee_le(fx.mem.f(D + 0x12C0), 1.0);
        let step = f32::from_bits(if grow { 0x3ECC_CCCD } else { 0x3BA3_D70A });
        for k in [0x12C0u32, 0x12C4, 0x12C8] {
            let v = ee::add(fx.mem.f(D + k), step);
            fx.mem.setf(D + k, v);
        }
        if !grow {
            fx.step_down(D + 0x133B, 0, 2);
            fx.step_down(D + 0x1867, 0, 6);
            for k in [0x58u32, 0x54, 0x50] {
                fx.mem.write_u32(sp + k, 0x3F82_8F5C);
            }
            fx.particle_quads(D + 0x12C0, D + 0x1890, sp + 0x50);
            fx.step_down(D + 0x189B, 0, 4);
        }
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0x1390 + 16 * k + 4 * c;
                let x = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, x);
            }
        }
        // The prologues of quads_draw (`sq fp`) / particle_quads (`sq s3`)
        // at entry sp - 0x120: the entry's s3 / fp, 0 [run].
        fx.saved(sp - 0x120, &[(0, 0)]);
    }
    quadtrail::draw(fx, TRAIL_DRAW, OT, D + 0x70, D);
    trail_fade(fx, D + 0x70);
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x29F4, m);
    let left = fx.mem.i32(D + 0x29F0) - 1;
    fx.mem.set32(D + 0x29F0, left);
    if m == 0x3C {
        3
    } else if m == 0x32 {
        1
    } else if left > 0 {
        0
    } else {
        8
    }
}
