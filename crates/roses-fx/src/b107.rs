//! BMODULE b107 (card 109), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E040B0, data area 0x1ED0000) [F] (helper C5, 31
//! Oct 2026). Falling streaks round the target: until update 256 two rain
//! sheets (+0x1E0 / +0x5F0, speed -30) and a slow one (+0xBF0) with a
//! particle cloud (+0xAA0) at the target, from update 37; a spark burst
//! (+0x1C40) at the target thrown out again in random directions. From
//! update 257 the scene (+0x70) grows to 1 and climbs from the target
//! (+0x90) toward the attacker's node 0xB (90 steps, swaying by 4 sin),
//! two rings (+0x1070 / +0x1470, the target's radius) and the point-set
//! strip (+0xE0 / look +0x180) turning and fading until 370 while the
//! camera slots 4 / 5 follow +0xA00 (cleared on 358); then it shrinks to
//! 0.4 and the squares (+0x18E0) spread from 370. The stage light dims
//! to 0x20 and comes back after 368. Returns 1 on update 51, 6 on
//! 61..264, 2 on 265, 8 after 400.
//!
//! Library: `rain::colours2` (0x1E00080), `pointquad::setup` / `draw` /
//! `squares` (0x1E001A0 / 0x1E004E0 / 0x1E02400), `ring_setup` /
//! `ring_draw` (0x1E010C0 / 0x1E01380), `particle_quads` (0x1E01870),
//! `spawn::scatter` (0x1E029E0), `trail::sparks` (0x1E02BA0),
//! `rain::setup` / `draw` (0x1E03900 / 0x1E03B50).
use crate::b028::square_points;
use crate::b628::halves;
use crate::blib::{self, pointquad, rain};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SETUP: u32 = 0x01E0_01A0;
const RAIN_SETUP: u32 = 0x01E0_3900;
const RAIN_DRAW: u32 = 0x01E0_3B50;
const TENTH: f32 = f32::from_bits(0x3DCC_CCCD);

/// The entry 0x1E040B0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x80);
    ret
}

/// 0x1F6A60 [F]: the radius (obj+0x1E4, 0x1F2150) of the target's unit
/// object (handle *0x2DF5A8 -> 0x1E25E0); 0 without a unit, -1 for an
/// index outside the table or an empty entry.
pub(crate) fn target_radius(fx: &mut Fx) -> f32 {
    let h = fx.target_handle();
    let (side, slot) = (h >> 8, h & 0xFF);
    if !(0..3).contains(&side) || slot >= 0xB {
        return 0.0;
    }
    let i = fx.object_index(side, slot);
    if i < 0 {
        return 0.0;
    }
    if i >= 0x54 {
        return -1.0;
    }
    let o = fx.mem.read_u32(crate::world::OBJECT_TABLE + 4 * i as u32);
    if o == 0 {
        return -1.0;
    }
    fx.mem.f(o + 0x1E4)
}

/// 0x103580(v, target, step) [F]: each component steps down by `step`
/// while at least target + step (bit k set), else is set to `target`.
pub(crate) fn approach_down(fx: &mut Fx, v: u32, target: f32, step: f32) -> i32 {
    let lim = ee::add(target, step);
    let mut bits = 0;
    for k in 0..3u32 {
        let a = v + 4 * k;
        let x = fx.mem.f(a);
        if ee_lt(x, lim) {
            fx.mem.setf(a, target);
            bits &= !(1 << k);
        } else {
            fx.mem.setf(a, ee::sub(x, step));
            bits |= 1 << k;
        }
    }
    bits
}

/// A ring of b107 (+0x1070 / +0x1470): rect (0x8E, 0, 0x20, 0x40), colour
/// (0xE0, 0x80, 0x80, 0x40).
fn ring(fx: &mut Fx, g: u32, tex: u32, r: f32, w: f32) {
    fx.mem.write_u32(g + 4, 0);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x40)]);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0xE0), (0xB, 0x40)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.ring_setup(g, tex, r, r, 30.0, w);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2CA0);
    for x in [0u32, 0x70, 0xA30, 0x1000, 0x1BD0] {
        fx.xform_init(D + x);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0xB, 1);
    fx.mem.set32(D + 0x2C90, r);
    fx.mem.set32(D + 0x2C80, 0x190);
    fx.mem.set32(D + 0x2C84, 1);
    fx.mem.set32(D + 0x2C88, param);
    fx.mem.set32(D + 0x2C8C, 0);
    fx.mem.set32(D + 0x2C94, 0x5A);
    fx.mem.set32(D + 0x2C9C, 0x80);
    fx.texture4(sp + 0x70, 0xF);
    let r = target_radius(fx);
    let tex = fx.mem.read_u32(sp + 0x70);
    ring(fx, D + 0x1070, tex, r, ee::mul(2.0, r));
    let tex = fx.mem.read_u32(sp + 0x70);
    ring(fx, D + 0x1470, tex, r, -10.0);
    // The point set's look (+0x180): cell (176, 32, 32, 32) of a 256 x 256
    // sheet from (176, 32), colour (0xE0, 0x80, 0x80, 0), looping frame 0.
    let g = D + 0x180;
    fx.mem.write_u32(g + 0x38, 0x8_0001);
    halves(fx, g, &[(0, 0xB0), (2, 0x20), (4, 0x20), (6, 0x20), (8, 0xB0), (0xA, 0x20), (0xC, 0x100), (0xE, 0x100)]);
    fx.mem.write_u32(g + 0x30, 0x3F80_0000);
    for (k, x) in [(0x36u32, 0x80u8), (0x35, 0x80), (0x34, 0xE0), (0x37, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    halves(fx, g, &[(0x48, 0), (0x4A, 0), (0x4C, 0), (0x4E, 1), (0x50, 0), (0x52, 0x3E7)]);
    fx.mem.write_u32(D + 0xE8, 1);
    let tex = fx.mem.read_u32(sp + 0x70);
    pointquad::setup(fx, SETUP, g, D + 0xE0, tex, 16.0);
    // The camera slots' source (+0xA00).
    for (k, x) in [(0xA04u32, 0x3F00_0000u32), (0xA08, 0x3F40_0000), (0xA0C, 0x3F80_0000), (0xA00, 0x3F80_0000), (0xA20, 0), (0xA24, 0x4220_0000), (0xA28, 0x41A0_0000)] {
        fx.mem.write_u32(D + k, x);
    }
    // The squares (+0x18E0): 60 points, half size 1.
    let g = D + 0x18E0;
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 0xC, 0x3F80_0000);
    square_points(fx, g);
    fx.mem.write_u32(g + 4, 0x2008_0081);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x40), (0x14, 0x20), (0x16, 0x20)]);
    for (k, x) in [(0xAu32, 0xE0u8), (9, 0x80), (8, 0x80), (0xB, 0x80)] {
        fx.mem.write_u8(g + k, x);
    }
    // The particle cloud (+0xAA0): 16 particles, half size 1, 8 frames.
    let g = D + 0xAA0;
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x8_0081);
    for (k, x) in [(0x10u32, 0), (0x14, 0x4000_0000), (0x18, 0), (0x1C, 0)] {
        fx.mem.write_u32(g + k, x);
    }
    halves(fx, g, &[(0x28, 0xF0), (0x2A, 0), (0x24, 0x10), (0x26, 0x10), (0x2C, 0x100), (0x2E, 0x100)]);
    fx.mem.write_u32(g + 0xC, 0x3F80_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 1), (0x13C, 2), (0x140, 8), (0x136, 0xA), (0x138, 0xA)]);
    for (k, x) in [(0xAu32, 0x60u8), (9, 0x70), (8, 0x80), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, x);
    }
    for k in 0..16u32 {
        let p = g + 0x30 + 16 * k;
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x28);
        let v = ee::add(fx.mem.f(g + 0x10), ee::mul(TENTH, ee::from_i32(h - 0x14)));
        fx.mem.setf(p, v);
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x14);
        let v = ee::add(fx.mem.f(g + 0x14), ee::mul(TENTH, ee::from_i32(h)));
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
    // The spark burst (+0x1C40): 256 sparks at +0x1C70, width 0.2,
    // length 10.
    let g = D + 0x1C40;
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 3);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0x20, 0x3E4C_CCCD);
    fx.mem.write_u32(g + 0x24, 0x4120_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x40), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x40), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, D + 0x1C70);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    // The rain: +0xBF0 (64 streaks, speed 10), +0x1E0 / +0x5F0 (64 each,
    // flag 0x1D, speed 100).
    let g = D + 0xBF0;
    fx.mem.write_u32(g, 0x1008_0001);
    for (k, x) in [(6u32, 0x80u8), (5, 0x80), (4, 0xE0), (7, 0x80)] {
        fx.mem.write_u8(g + k, x);
    }
    rain::setup(fx, RAIN_SETUP, g, 0x40, 10.0);
    for k in 0..2u32 {
        let g = D + 0x1E0 + 0x410 * k;
        fx.mem.write_u32(g, 0x3008_0001);
        for (k, x) in [(6u32, 0x80u8), (5, 0x80), (4, 0xE0), (7, 0x80)] {
            fx.mem.write_u8(g + k, x);
        }
        rain::setup(fx, RAIN_SETUP, g, 0x40, 100.0);
    }
    let t = fx.target_handle();
    fx.unit_point(2, t, D + 0x20);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x90);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x2C70);
    fx.mem.write_u32(D + 0x1004, 0);
    for k in [0x78u32, 0x74, 0x70] {
        fx.mem.write_u32(D + k, 0x3DCC_CCCD);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2C84);
    if n(fx) < 0x100 {
        let t = fx.target_handle();
        fx.unit_point(1, t, D + 0x90);
    }
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x1020);
    fx.mem.write_u32(D + 0x1024, 0);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x1BF0);
    for out in [0xA50u32, 0x2C70] {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x2C90);
        fx.node_position(h, s, D + out);
    }
    if !bit(fx.mem.read_u32(D + 0x2C8C), 0x1F) {
        if n(fx) >= 0x101 {
            fx.step_up(D + 0x1B7, 0x80, 0x10);
            if fx.approach(D + 0x70, 1.0, TENTH) != 0 {
                let y = ee::add(fx.mem.f(D + 0x94), 1.0);
                fx.mem.setf(D + 0x94, y);
            }
        }
        if n(fx) >= 0x10B {
            let left = fx.mem.i32(D + 0x2C94);
            fx.approach_by(D + 0x90, D + 0x2C70, left);
            let left = fx.mem.i32(D + 0x2C94);
            if left != 0 {
                fx.mem.set32(D + 0x2C94, left - 1);
                let a = fx.mem.i32(D + 0x2C98).wrapping_add(0xA00);
                fx.mem.set32(D + 0x2C98, a);
                let s = fx.tsin(a);
                let y = ee::add(fx.mem.f(D + 0x94), ee::mul(4.0, s));
                fx.mem.setf(D + 0x94, y);
            } else {
                let f = fx.mem.read_u32(D + 0x2C8C) | 0x8000_0000;
                fx.mem.write_u32(D + 0x2C8C, f);
            }
        }
    } else if approach_down(fx, D + 0x70, f32::from_bits(0x3ECC_CCCD), f32::from_bits(0x3E4C_CCCD)) == 0 {
        let f = fx.mem.read_u32(D + 0x2C8C) | 0x4000_0000;
        fx.mem.write_u32(D + 0x2C8C, f);
    }
    if bit(fx.mem.read_u32(D + 0x2C8C), 0x1E) && n(fx) < 0x171 {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x2C90);
        fx.node_position(h, s, D + 0x90);
    }
    for k in 0..3u32 {
        let v = fx.mem.read_u32(D + 0x90 + 4 * k);
        fx.mem.write_u32(D + 0xA10 + 4 * k, v);
    }
    fx.xform_world(D);
    fx.xform_billboard(D + 0x70);
    fx.xform_billboard(D + 0xA30);
    fx.xform_world(D + 0x1000);
    fx.xform_world(D + 0x1BD0);
    if n(fx) < 0x171 {
        fx.step_down(D + 0x2C9C, 0x20, 4);
    } else {
        fx.step_up(D + 0x2C9C, 0x80, 4);
    }
    let light = i32::from(fx.mem.read_u8(D + 0x2C9C));
    fx.set_stage_light(light);
    if n(fx) >= 0x172 {
        fx.approach(D + 0x70, 1.0, f32::from_bits(0x3ECC_CCCD));
        for k in [0x68u32, 0x64, 0x60] {
            fx.mem.write_u32(sp + k, 0x3F82_8F5C);
        }
        pointquad::squares(fx, OT, D + 0x18E0, D + 0x70, sp + 0x60);
        fx.step_down(D + 0x18EB, 0, 8);
    }
    let m = n(fx);
    if m == 0x166 {
        blib::exe::camera_slots45(fx, 0);
    } else if (0x101..0x166).contains(&m) {
        blib::exe::camera_slots45(fx, D + 0xA00);
    }
    if (0x101..0x173).contains(&n(fx)) {
        fx.ring_draw(D + 0x1070, D + 0x1000);
        fx.ring_draw(D + 0x1470, D + 0x1000);
        let a = ee::add(fx.mem.f(D + 0x1004), f32::from_bits(0x3E4C_CCCD));
        fx.mem.setf(D + 0x1004, a);
        fx.step_down(D + 0x107B, 0, 6);
        fx.step_down(D + 0x147B, 0, 6);
        pointquad::draw(fx, OT, D + 0x180, D + 0xE0, D + 0x70);
    }
    if (0x25..0x100).contains(&n(fx)) {
        for k in [0x68u32, 0x64, 0x60] {
            fx.mem.write_u32(sp + k, 0x3F80_0000);
        }
        fx.particle_quads(D + 0xA30, D + 0xAA0, sp + 0x60);
        rain::draw(fx, RAIN_DRAW, OT, D + 0xBF0, D + 0xA30, 1.5);
        for k in 0..2u32 {
            rain::draw(fx, RAIN_DRAW, OT, D + 0x1E0 + 0x410 * k, D + 0x70, -30.0);
        }
    }
    blib::trail::sparks(fx, OT, D + 0x1C40, D + 0x1BD0);
    if n(fx) >= 0x10A {
        fx.step_down(D + 0x1C4B, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x1C6C) {
        let p = D + 0x1C70 + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        let s = f32::from_bits(0x3FA6_6666);
        for c in 0..3u32 {
            let x = ee::mul(fx.mem.f(p + 4 * c), s);
            fx.mem.setf(p + 4 * c, x);
        }
        if fx.mem.i16(p + 0xE) < 0x11 {
            continue;
        }
        fx.random_dir(sp + 0x60, 1.0);
        for (o, s) in [(4u32, 0x64u32), (8, 0x68), (0, 0x60)] {
            let w = fx.mem.read_u32(sp + s);
            fx.mem.write_u32(p + o, w);
        }
        let r = fx.rand();
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, h.wrapping_neg());
        fx.mem.write_u8(p + 0xC, 0x80);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2C84, m);
    let left = fx.mem.i32(D + 0x2C80) - 1;
    fx.mem.set32(D + 0x2C80, left);
    if m == 0x109 {
        2
    } else if m == 0x33 {
        1
    } else if (0x3D..0x109).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
