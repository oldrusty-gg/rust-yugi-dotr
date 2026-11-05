//! BMODULE b021 (card 294), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04320, data area 0x1ED0000, 0x3660 bytes) [F]
//!: an unscheduled monolith of the b036 / b113
//! family (s0 = D, s1 the OT, s3 the parameter at launch). Four target
//! points (D+0x990 + 0x70 k, 0x1F65D0, swung 170 x (sin, cos) of 90 k
//! degrees, 30 up) with sixteen strands each from the self unit's nodes
//! (D+0x70 + 0x30 k, `strand::draw`); two rings of sixteen strands round
//! nodes 0x19 / 0x26 (D+0x370 / 0x670, from 0xFF); a quad cloud (D+0x1F10)
//! and point quads (D+0x2410) at the self unit on 0xCE..0xEA, camera
//! light (D+0x1EE0) on 0x97..0xF9; from 0x11A four columns of three
//! five-piece strips (D+0xB30, the record's 0x1E03DE0 / 0x1E03F30) at the
//! points with facing quads (D+0x1130, `facing::quad`) on their tips,
//! particle groups (D+0x1930), square clouds (D+0x2630), facing quads
//! (D+0x1730) and the strands re-aimed at the points; the full-window
//! tint (D+0x3640, b262's) every fifth update 0x11C..0x18D and from
//! 0x18E; the stage light (D+0x3658) 0x80 down by 4 to 0x20, up by 4
//! from 0x1A4; the self unit's added colour (D+0x3654, 0x1F7790) below
//! 0x11A. Returns 1 on 0x115, 6 on 0x11B..0x1A3, 3 on 0x1A4, 8 when the
//! timer D+0x3610 (0x1E0) runs out.
//!
//! Library (record addresses): `colours4` 0x1E00080, `b262::tint`
//! 0x1E00270, `facing::quad` 0x1E003D0, `pointquad` setup / draw /
//! squares 0x1E00C60 / 0x1E00FA0 / 0x1E02710, `particle_quads` 0x1E01B80,
//! `quads_setup` / `quads_draw` 0x1E02CF0 / 0x1E02E50, `strand::draw`
//! 0x1E03460. Record-local (shared by cards 362 / 677, not yet library
//! rows): the column setup 0x1E03DE0 (1c6424378088), the column draw
//! 0x1E03F30 (4d7f719bbd8a) and its colours 0x1E00120 (3272eb9440cc).
use crate::b079::sub3;
use crate::b142::fstep_up;
use crate::b436::fstep_down;
use crate::blib::{exe::camera_slots45, facing, pointquad, strand};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const TIMER: u32 = D + 0x3610;
const N: u32 = D + 0x3614;
const SPACE: u32 = D + 0x1E70;
const SELF_PT: u32 = D + 0x1E90;
const STRANDS: u32 = D + 0x70;
/// The quad cloud's corner template (0x1E02CF0's `lui` / `addiu`).
const QUAD_TEMPLATE: u32 = 0x01E0_6000;
/// The node table 0x1E05D00 copies (30 bytes).
const NODE_TABLE: u32 = 0x01E0_6040;
const PI_2: u32 = 0x3FC9_0FDB;
const PI_8: u32 = 0x3EC9_0FDB;

/// The entry 0x1E04320.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xE0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xE0);
    ret
}

fn halves(fx: &mut Fx, at: u32, v: &[(u32, i32)]) {
    for &(k, x) in v {
        fx.mem.set16(at + k, x);
    }
}

fn bytes(fx: &mut Fx, at: u32, v: &[(u32, u8)]) {
    for &(k, x) in v {
        fx.mem.write_u8(at + k, x);
    }
}

fn set3(fx: &mut Fx, at: u32, w: u32) {
    for k in [8u32, 4, 0] {
        fx.mem.write_u32(at + k, w);
    }
}

/// rand() % m - sub as a float, rand's frame saving the entry's s0 = D.
fn rand_f(fx: &mut Fx, s0: u32, ra: u32, m: i32, sub: i32) -> f32 {
    let r = fx.rand_from(s0, ra);
    let (_, h) = fx.div(r, m);
    ee::from_i32(h - sub)
}

fn node(fx: &mut Fx, slot: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + slot);
    fx.node_position(h, s, out);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3660);
    fx.xform_init(D);
    fx.xform_init(SPACE);
    for k in 0..4u32 {
        fx.xform_init(D + 0x2470 + 0x70 * k);
        set3(fx, D + 0x2470 + 0x70 * k, 0x3DCC_CCCD);
        fx.xform_init(D + 0x970 + 0x70 * k);
        fx.mem.write_u32(D + 0x3644 + 4 * k, 0x432A_0000);
    }
    for (at, n, clear) in [(0x362Cu32, 0x1D, 1), (0x3630, 0x19, 0), (0x3634, 0x26, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(TIMER, 0x1E0);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x3618, param);
    fx.mem.set32(D + 0x361C, 0);
    fx.mem.write_u8(D + 0x3658, 0x80);
    let h = fx.self_handle();
    let c = fx.unit_nodes(h);
    fx.mem.set32(D + 0x3620, c - 1);
    bytes(fx, D + 0x3640, &[(2, 0x80), (1, 0x80), (0, 0x80), (3, 0x80)]);
    bytes(fx, D + 0x3654, &[(2, 0x30), (1, 0x60), (0, 0x80), (3, 0)]);
    // The camera light (+0x1EE0).
    for (o, w) in [(0x1EE4u32, 0x3F00_0000u32), (0x1EE8, 0x3F40_0000), (0x1EEC, 0x3F80_0000), (0x1EE0, 0x3F80_0000), (0x1F00, 0), (0x1F04, 0x4348_0000), (0x1F08, 0)] {
        fx.mem.write_u32(D + o, w);
    }
    fx.texture4(sp + 0xDC, 0xF);
    // The quad cloud (+0x1F10).
    let g = D + 0x1F10;
    fx.mem.write_u32(g + 4, 0x0008_0001);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    bytes(fx, g, &[(0xA, 0x80), (9, 0x80), (8, 0x80), (0xB, 0x20)]);
    let tex = fx.mem.read_u32(sp + 0xDC);
    fx.quads_setup(g, tex, f32::from_bits(0x3FCC_CCCD), QUAD_TEMPLATE);
    // The point quads (+0x2410, points +0x2370).
    let g = D + 0x2410;
    fx.mem.write_u32(g + 0x38, 0x0008_0001);
    halves(fx, g, &[(0, 0x80), (2, 0), (4, 0x20), (6, 0x20), (8, 0xB0), (0xA, 0x20), (0xC, 0x100), (0xE, 0x100)]);
    fx.mem.write_u32(g + 0x30, 0x3F80_0000);
    bytes(fx, g, &[(0x36, 0x80), (0x35, 0x80), (0x34, 0x80), (0x37, 0x60)]);
    halves(fx, g, &[(0x48, 0), (0x4A, 0), (0x4C, 0), (0x4E, 1), (0x50, 0), (0x52, 0x3E7)]);
    fx.mem.set32(D + 0x2378, 1);
    let tex = fx.mem.read_u32(sp + 0xDC);
    pointquad::setup(fx, 0x01E0_0C60, g, D + 0x2370, tex, 20.0);
    // Four square clouds (+0x2630 + 0x2F0 k) of 60 points.
    for k in 0..4u32 {
        let g = D + 0x2630 + 0x2F0 * k;
        let tex = fx.mem.read_u32(sp + 0xDC);
        fx.mem.write_u32(g, tex);
        fx.mem.set32(g + 4, 1);
        halves(fx, g, &[(0x10, 0xB0), (0x12, 0x40), (0x14, 0x20), (0x16, 0x20)]);
        fx.mem.write_u32(g + 0xC, 0x3F00_0000);
        for j in 0..0x3Cu32 {
            let p = g + 0x20 + 12 * j;
            if bit(fx.mem.read_u32(g + 4), 0x1D) {
                let v = rand_f(fx, D, 0x01E0_4684, 100, 50);
                fx.mem.setf(p, v);
                let v = rand_f(fx, D, 0x01E0_46C8, 50, 0);
                fx.mem.setf(p + 8, v);
                let v = rand_f(fx, D, 0x01E0_46FC, 100, 50);
                fx.mem.setf(p + 4, v);
            } else {
                let v = rand_f(fx, D, 0x01E0_473C, 50, 25);
                fx.mem.setf(p, v);
                let v = rand_f(fx, D, 0x01E0_4780, 25, 0);
                fx.mem.setf(p + 4, v);
                let v = rand_f(fx, D, 0x01E0_47B4, 50, 25);
                fx.mem.setf(p + 8, v);
            }
        }
        bytes(fx, g, &[(0xA, 0x40), (9, 0x40), (8, 0x80), (0xB, 0x80)]);
    }
    // Four particle groups (+0x1930 + 0x150 k) of 16.
    for k in 0..4u32 {
        let g = D + 0x1930 + 0x150 * k;
        let tex = fx.mem.read_u32(sp + 0xDC);
        fx.mem.write_u32(g, tex);
        fx.mem.set32(g + 4, 7);
        fx.mem.write_u32(g + 0x10, 0);
        fx.mem.write_u32(g + 0x14, 0x4120_0000);
        fx.mem.write_u32(g + 0x18, 0);
        halves(fx, g, &[(0x20, 0x80), (0x22, 0xC0), (0x24, 0x20), (0x26, 0x20), (0x28, 0x80), (0x2A, 0xC0), (0x2C, 0x80), (0x2E, 0x100)]);
        fx.mem.write_u32(g + 0xC, 0x41F0_0000);
        halves(fx, g, &[(0x130, 8), (0x13A, 1), (0x13C, 1), (0x140, 8), (0x136, 0x1E), (0x138, -0x1E)]);
        bytes(fx, g, &[(0xA, 0x80), (9, 0x80), (8, 0x80), (0xB, 0x20)]);
        for j in 0..0x10u32 {
            let p = g + 0x30 + 0x10 * j;
            let v = rand_f(fx, D, 0x01E0_48FC, 0x50, 0x28);
            fx.mem.setf(p, v);
            let v = rand_f(fx, D, 0x01E0_4938, 0x50, 0);
            fx.mem.setf(p + 4, v);
            let f = -fx.mem.i16(g + 0x13A);
            let v = fx.mult(f, k as i32);
            fx.mem.set16(p + 0xE, v);
            fx.mem.set16(p + 0xC, 0);
            let v = fx.mem.i16(g + 0x140) - 1;
            fx.mem.set16(g + 0x13E, v);
        }
    }
    fx.texture5(sp + 0xDC, 0);
    // Sixteen strands (+0x70 + 0x30 k): odd ones far Z.
    for k in 0..0x10u32 {
        let g = STRANDS + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0xDC);
        fx.mem.write_u32(g, tex);
        fx.mem.write_u32(g + 4, if k & 1 != 0 { 0x0008_0000 } else { 0 });
        halves(fx, g, &[(0x10, 0x80), (0x12, 0x40), (0x14, 0x20), (0x16, 0x40)]);
        bytes(fx, g, &[(0xA, 0x80), (9, 0x80), (8, 0x80), (0xB, 0x20)]);
        halves(fx, g, &[(0xC, 0x10), (0x28, -(k as i32 + 1)), (0xE, 0x14)]);
        fx.mem.write_u32(g + 0x20, 0);
        fx.mem.write_u32(g + 0x24, 0);
    }
    // Two rings of sixteen strands (+0x370 / +0x670 + 0x30 k).
    for base in [0x370u32, 0x670] {
        for k in 0..0x10u32 {
            let g = D + base + 0x30 * k;
            let tex = fx.mem.read_u32(sp + 0xDC);
            fx.mem.write_u32(g, tex);
            fx.mem.write_u32(g + 4, 0x0C00_0000);
            let (_, h) = fx.div(k as i32, 3);
            halves(fx, g, &[(0x10, h << 5), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
            bytes(fx, g, &[(0xA, 0x80), (9, 0x80), (8, 0x80), (0xB, 0x80)]);
            halves(fx, g, &[(0xC, 6), (0x28, 0), (0xE, 0x1E)]);
            fx.mem.write_u32(g + 0x20, 0);
            fx.mem.write_u32(g + 0x24, 0);
            fx.mem.write_u32(g + 0x2C, 0x4348_0000);
        }
    }
    // Four columns of three strips (+0xB30 + 0x180 k + 0x80 j), their tip
    // quads (+0x1130 + 0x120 k + 0x60 j, looks +0x15B0 + 0x60 k + 0x20 j).
    for k in 0..4u32 {
        for j in 0..3u32 {
            let c = D + 0xB30 + 0x180 * k + 0x80 * j;
            halves(fx, c, &[(0xC, (j + 5 * k + 5) as i32), (0x10, 0), (0x12, 0), (0x14, 0x20), (0x16, 0x80), (0x18, 0), (0x1A, 0)]);
            fx.mem.write_u32(c + 0x2C, 0x41A0_0000);
            bytes(fx, c, &[(6, 0x80), (5, 0x80), (4, 0x80), (7, 0x80)]);
            let tex = fx.mem.read_u32(sp + 0xDC);
            column_setup(fx, c, tex, f32::from_bits(0x43D7_0000));
            let q = D + 0x1130 + 0x120 * k + 0x60 * j;
            fx.mem.set32(q + 0x38, 1);
            fx.mem.write_u32(q + 0x30, 0x3F80_0000);
            bytes(fx, q, &[(0x36, 0x80), (0x35, 0x80), (0x34, 0x80), (0x37, 0x80)]);
            halves(fx, q, &[(0, 0xE0), (2, 0), (4, 0x20), (6, 0x20), (8, 0xE0), (0xA, 0), (0xC, 0x100), (0xE, 0x80)]);
            halves(fx, q, &[(0x48, 0), (0x4A, 0), (0x4E, 2), (0x52, 0x3E7), (0x50, 0), (0x4C, 1)]);
            let l = D + 0x15B0 + 0x60 * k + 0x20 * j;
            let tex = fx.mem.read_u32(sp + 0xDC);
            set3(fx, l + 0x10, 0);
            fx.mem.write_u32(l + 8, 0x4170_0000);
            fx.mem.write_u32(l, tex);
            bytes(fx, l, &[(0xE, 0x80), (0xD, 0x80), (0xC, 0x80), (0xF, 0x80)]);
        }
    }
    // Four facing quads (+0x1730 + 0x60 k) and their looks (+0x18B0 + 0x20 k).
    for k in 0..4u32 {
        let q = D + 0x1730 + 0x60 * k;
        fx.mem.set32(q + 0x38, 1);
        fx.mem.write_u32(q + 0x30, 0x3F80_0000);
        bytes(fx, q, &[(0x36, 0x80), (0x35, 0x80), (0x34, 0x80), (0x37, 0x80)]);
        halves(fx, q, &[(0, 0xA0), (2, 0), (4, 0x40), (6, 0x40), (8, 0xA0), (0xA, 0), (0xC, 0x100), (0xE, 0x80)]);
        halves(fx, q, &[(0x48, 0), (0x4A, 0), (0x4E, 1), (0x52, 0x3E7), (0x50, 0), (0x4C, 0)]);
        let l = D + 0x18B0 + 0x20 * k;
        let tex = fx.mem.read_u32(sp + 0xDC);
        set3(fx, l + 0x10, 0);
        fx.mem.write_u32(l + 8, 0x41A0_0000);
        fx.mem.write_u32(l, tex);
        bytes(fx, l, &[(0xE, 0x80), (0xD, 0x80), (0xC, 0x80), (0xF, 0x80)]);
    }
    0
}

fn n(fx: &mut Fx) -> i32 {
    fx.mem.i32(N)
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    // The four target points.
    for k in 0..4u32 {
        let p = D + 0x990 + 0x70 * k;
        let t = fx.target_handle();
        fx.unit_point(1, t, p);
        let a = ee::mul(f32::from_bits(PI_2), ee::from_i32(k as i32));
        let c = fx.cosf(a);
        let r = D + 0x3644 + 4 * k;
        let v = ee::add(fx.mem.f(p + 8), ee::mul(fx.mem.f(r), c));
        fx.mem.setf(p + 8, v);
        let s = fx.sinf(a);
        let v = ee::add(fx.mem.f(p), ee::mul(fx.mem.f(r), s));
        fx.mem.setf(p, v);
        fx.mem.write_u32(p + 4, 0x41F0_0000);
        for o in [0u32, 4, 8, 0xC] {
            let w = fx.mem.read_u32(p + o);
            fx.mem.write_u32(D + 0x2490 + 0x70 * k + o, w);
        }
    }
    node(fx, 0x3630, D + 0x33F0);
    node(fx, 0x3634, D + 0x3400);
    for k in 0..0x10u32 {
        let a = ee::mul(f32::from_bits(PI_8), ee::from_i32(k as i32));
        let (r0, r1) = (D + 0x3410 + 0x10 * k, D + 0x3510 + 0x10 * k);
        let v = ee::add(30.0, fx.mem.f(D + 0x33F4));
        fx.mem.setf(r0 + 4, v);
        let c = fx.cosf(a);
        let v = ee::add(fx.mem.f(D + 0x33F0), ee::mul(20.0, c));
        fx.mem.setf(r0, v);
        let s = fx.sinf(a);
        let v = ee::add(fx.mem.f(D + 0x33F8), ee::mul(20.0, s));
        fx.mem.setf(r0 + 8, v);
        let v = ee::add(30.0, fx.mem.f(D + 0x3404));
        fx.mem.setf(r1 + 4, v);
        let c = fx.cosf(a);
        let v = ee::add(fx.mem.f(D + 0x3400), ee::mul(20.0, c));
        fx.mem.setf(r1, v);
        let s = fx.sinf(a);
        let v = ee::add(fx.mem.f(D + 0x3408), ee::mul(20.0, s));
        fx.mem.setf(r1 + 8, v);
    }
    // The strands' ends.
    for k in 0..0x10u32 {
        let (a, b) = (D + 0x31F0 + 0x20 * k, D + 0x3200 + 0x20 * k);
        if n(fx) < 0x11B {
            if fx.mem.i16(STRANDS + 0x30 * k + 0x28) == 0 {
                node(fx, 0x3638, a);
                node(fx, 0x363C, b);
                sub3(fx, a, SELF_PT);
                sub3(fx, b, SELF_PT);
            }
        } else {
            fx.mem.write_u32(a + 4, 0x4120_0000);
            let ang = ee::mul(f32::from_bits(PI_2), ee::from_i32((k & 3) as i32));
            let p = D + 0x990 + 0x70 * (k >> 2);
            let c = fx.cosf(ang);
            let v = ee::add(fx.mem.f(p), ee::mul(100.0, c));
            fx.mem.setf(a, v);
            let s = fx.sinf(ang);
            let v = ee::add(fx.mem.f(p + 8), ee::mul(100.0, s));
            fx.mem.setf(a + 8, v);
        }
    }
    let h = fx.self_handle();
    fx.unit_point(1, h, SELF_PT);
    // 0x1038D0 / 0x1039A0 with their frames (the entry's s0 = D, s1 = the
    // OT, s2 / s3 the loop's k / D + 0x70 k): the last billboard's TRS
    // matrix is the columns' missing-texture TEX0 / TEX1 [run].
    fx.xform_world_saving(D, 0x01E0_51FC, [Some(D), Some(OT), None, None]);
    fx.xform_world_saving(SPACE, 0x01E0_5208, [Some(D), Some(OT), None, None]);
    for k in 0..4u32 {
        let s = [Some(D), Some(OT), Some(k), Some(D + 0x70 * k)];
        fx.xform_billboard_saving(D + 0x2470 + 0x70 * k, 0x01E0_5230, s);
        fx.xform_billboard_saving(D + 0x970 + 0x70 * k, 0x01E0_523C, s);
    }
    if n(fx) < 0x1A4 {
        fx.step_down(D + 0x3658, 0x20, 4);
    } else {
        fx.step_up(D + 0x3658, 0x80, 4);
    }
    let l = fx.mem.read_u8(D + 0x3658);
    fx.set_stage_light(i32::from(l));
    let m = n(fx);
    if (0x11C..0x18E).contains(&m) {
        let (_, r) = fx.div(m, 5);
        if r == 0 {
            crate::b262::tint(fx, D + 0x3640);
        }
    }
    if n(fx) == 0x18E {
        fx.mem.write_u8(D + 0x3643, 0xFF);
    }
    if n(fx) >= 0x18E {
        crate::b262::tint(fx, D + 0x3640);
        if n(fx) >= 0x196 {
            fx.step_down(D + 0x3643, 0, 4);
        }
    }
    if n(fx) == 0x11B {
        for k in 0..0x10u32 {
            let g = STRANDS + 0x30 * k;
            let f = fx.mem.read_u32(g + 4) & 0xFFF7_FFFF;
            fx.mem.write_u32(g + 4, f);
            let f = fx.mem.read_u32(g + 4) | 0x0400_0000;
            fx.mem.write_u32(g + 4, f);
            fx.mem.write_u8(g + 0xB, 0x80);
            fx.mem.set16(g + 0xE, 100);
            fx.mem.set16(g + 0x28, 0);
        }
    }
    if n(fx) < 0x11A {
        let h = fx.self_handle();
        // 0x1F77A0's frame (the entry's s0 = D / s1 = the OT, ra): the
        // strands' missing-texture TEX0 / TEX1 [run].
        fx.unit_add_colour_from(h, D + 0x3654, [D, OT], 0x01E0_53E0);
        let a = if n(fx) & 1 != 0 { 0 } else { 0x20 };
        fx.mem.write_u8(D + 0x3657, a);
        for k in 0..0x10u32 {
            strand::draw(fx, 0x01E0_3460, OT, STRANDS + 0x30 * k, SPACE, D + 0x31F0 + 0x20 * k, D + 0x3200 + 0x20 * k);
        }
        for k in 0..8u32 {
            if n(fx) >= 0xFF {
                strand::draw(fx, 0x01E0_3460, OT, D + 0x370 + 0x30 * k, D, D + 0x33F0, D + 0x3410 + 0x10 * k);
                strand::draw(fx, 0x01E0_3460, OT, D + 0x670 + 0x30 * k, D, D + 0x3400, D + 0x3510 + 0x10 * k);
            }
        }
    } else {
        for k in 0..4u32 {
            let x = D + 0x970 + 0x70 * k;
            if fx.mem.i16(D + 0xB3C + 0x180 * k) == 0 {
                let q = D + 0x1730 + 0x60 * k;
                facing::quad(fx, OT, q, D + 0x18B0, x);
                fx.step_down(q + 0x37, 0, 8);
                fstep_up(fx, q + 0x30, 5.0, f32::from_bits(0x3E4C_CCCD));
                for o in [0xC8u32, 0xC4, 0xC0] {
                    fx.mem.write_u32(sp + o, 0);
                }
                let g = D + 0x1930 + 0x150 * k;
                fx.particle_quads(x, g, sp + 0xC0);
                fx.step_down(g + 0xB, 0, 2);
                for o in [0xC8u32, 0xC4, 0xC0] {
                    fx.mem.write_u32(sp + o, 0x3F80_0000);
                }
                let s = D + 0x2470 + 0x70 * k;
                let g = D + 0x2630 + 0x2F0 * k;
                pointquad::squares(fx, OT, g, s, sp + 0xC0);
                fx.approach(s, 4.0, f32::from_bits(0x3E99_999A));
                fx.step_down(g + 0xB, 0, 8);
            } else {
                fx.mem.write_u8(D + 0x1767 + 0x60 * k, 0x80);
                fx.mem.write_u32(D + 0x1760 + 0x60 * k, 0x3F80_0000);
                fx.mem.write_u8(D + 0x193B + 0x150 * k, 0x20);
                fx.mem.write_u8(D + 0x263B + 0x2F0 * k, 0x80);
                set3(fx, D + 0x2470 + 0x70 * k, 0x3DCC_CCCD);
            }
        }
        for k in 0..0x10u32 {
            let g = STRANDS + 0x30 * k;
            if fx.mem.i16(D + 0xB3C + 0x180 * (k >> 2)) == 0 {
                strand::draw(fx, 0x01E0_3460, OT, g, D, D + 0x990 + 0x70 * (k >> 2), D + 0x31F0 + 0x20 * k);
                fx.step_down(g + 0xB, 0, 8);
                if fx.mem.i16(g + 0x28) == fx.mem.i16(g + 0xC) {
                    let f = fx.mem.read_u32(g + 4) | 0x0400_0000;
                    fx.mem.write_u32(g + 4, f);
                }
            } else {
                fx.mem.write_u8(g + 0xB, 0x80);
                let f = fx.mem.read_u32(g + 4) & 0xFBFF_FFFF;
                fx.mem.write_u32(g + 4, f);
            }
        }
    }
    let m = n(fx);
    if (0x97..0xFA).contains(&m) {
        if m < 0xE6 {
            fstep_up(fx, D + 0x1F08, 1.0, f32::from_bits(0x3D4C_CCCD));
        } else {
            fstep_down(fx, D + 0x1F08, 0.0, f32::from_bits(0x3E4C_CCCD));
        }
        for o in [0u32, 4, 8] {
            let w = fx.mem.read_u32(SELF_PT + o);
            fx.mem.write_u32(D + 0x1EF0 + o, w);
        }
        camera_slots45(fx, D + 0x1EE0);
    }
    let m = n(fx);
    if (0xCE..0xEB).contains(&m) {
        if fx.approach(SPACE, f32::from_bits(0x3F4C_CCCD), f32::from_bits(0x3DCC_CCCD)) == 0 {
            fx.step_down(D + 0x2447, 0, 6);
            fx.step_down(D + 0x1F1B, 0, 2);
        }
        pointquad::draw(fx, OT, D + 0x2410, D + 0x2370, SPACE);
        fx.quads_draw(D + 0x1F10, SPACE);
        for k in 0..0x40u32 {
            for c in 0..3u32 {
                let a = D + 0x1F70 + 0x10 * k + 4 * c;
                let w = fx.mem.read_u32(a).wrapping_add(0x80);
                fx.mem.write_u32(a, w);
            }
        }
    }
    let m = n(fx);
    if (0x11A..0x18F).contains(&m) {
        for k in 0..4u32 {
            for j in 0..3u32 {
                let c = D + 0xB30 + 0x180 * k + 0x80 * j;
                let a = fx.mem.u8(c + 7);
                if a < 0x60 {
                    let v = fx.mem.i16(c + 0x18) + fx.mem.i16(c + 0x14);
                    fx.mem.set16(c + 0x10, v);
                }
                let x = D + 0x970 + 0x70 * k;
                column_draw(fx, OT, c, x);
                let cnt = fx.mem.i16(c + 0xC);
                if cnt < 6 {
                    let p = (c + 0x30).wrapping_add(0x10u32.wrapping_mul((4 - cnt) as u32));
                    let l = D + 0x15B0 + 0x60 * k + 0x20 * j;
                    let v = ee::add(10.0, fx.mem.f(p));
                    fx.mem.setf(l + 0x10, v);
                    let w = fx.mem.read_u32(p + 8);
                    fx.mem.write_u32(l + 0x18, w);
                    let w = fx.mem.read_u32(p + 4);
                    fx.mem.write_u32(l + 0x14, w);
                    let q = D + 0x1130 + 0x120 * k + 0x60 * j;
                    let s = if cnt == 0 { 0x3FC0_0000 } else { 0x3F80_0000 };
                    fx.mem.write_u32(q + 0x30, s);
                    facing::quad(fx, OT, q, l, x);
                    let a = fx.mem.read_u8(c + 7);
                    fx.mem.write_u8(q + 0x37, a);
                }
                let cnt = fx.mem.i16(c + 0xC);
                if cnt != 0 {
                    fx.mem.set16(c + 0xC, cnt - 1);
                } else if fx.step_down(c + 7, 0, 8) == 0 {
                    fx.mem.set16(c + 0xC, 5);
                    fx.mem.write_u8(c + 7, 0x80);
                    if j == 2 {
                        fstep_down(fx, D + 0x3644 + 4 * k, 20.0, 30.0);
                    }
                }
            }
        }
    }
    if n(fx) < 0x11B {
        renodes(fx);
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(TIMER) - 1;
    fx.mem.set32(TIMER, left);
    if m == 0x1A4 {
        3
    } else if m == 0x115 {
        1
    } else if (0x11B..0x1A4).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        let h = fx.self_handle();
        fx.unit_add_colour(h, 0);
        camera_slots45(fx, 0);
        8
    }
}

/// 0x1E05D00(D, ot) [F] (e5dea8902836, this record's own): for each strand
/// whose piece counter +0x28 is 0, two random successive nodes of the
/// table 0x1E06040 (rand % 30, the next or 0) re-requested into D+0x3638
/// / 0x363C (with 0x1D (cleared) / 0x19 / 0x26 into D+0x362C..0x3634).
fn renodes(fx: &mut Fx) {
    let sp = fx.enter(0x50);
    for k in 0..0x1Eu32 {
        let b = fx.mem.read_u8(NODE_TABLE + k);
        fx.mem.write_u8(sp + 0x30 + k, b);
    }
    for k in 0..0x10u32 {
        if fx.mem.i16(STRANDS + 0x30 * k + 0x28) != 0 {
            continue;
        }
        let r = fx.rand_from(k, 0x01E0_5D70);
        let (_, h) = fx.div(r, 0x1E);
        fx.mem.set32(D + 0x3624, h);
        let nx = fx.mem.i32(D + 0x3624) + 1;
        fx.mem.set32(D + 0x3628, if nx < 0x1E { nx } else { 0 });
        for (at, nd, clear) in [(0x362Cu32, 0x1D, 1), (0x3630, 0x19, 0), (0x3634, 0x26, 0)] {
            let h = fx.self_handle();
            let r = fx.request_node(h, nd, clear);
            fx.mem.set32(D + at, r);
        }
        for (at, from) in [(0x3638u32, 0x3624u32), (0x363C, 0x3628)] {
            let h = fx.self_handle();
            let i = fx.mem.read_u32(D + from);
            let nd = fx.mem.u8((sp + 0x30).wrapping_add(i));
            let r = fx.request_node(h, nd, 0);
            fx.mem.set32(D + at, r);
        }
    }
    fx.leave(0x50);
}

/// 0x1E03DE0(g, rh2, _, len) [F] (1c6424378088, this record's own; also
/// cards 362 / 677): a column of five points (+0x30 + 16 k) hanging from
/// the base (0, len, 0) at +0x20, each len / 5 lower, x / z scattered by
/// rand % 30 - 15 (the last by rand % 10 - 5); texture +0 = `rh2`.
fn column_setup(fx: &mut Fx, g: u32, rh2: u32, len: f32) {
    let sp = fx.enter(0x40);
    let _ = sp;
    let step = ee::div(len, 5.0);
    fx.mem.write_u32(g, rh2);
    fx.mem.write_u32(g + 0x28, 0);
    fx.mem.write_u32(g + 0x20, 0);
    fx.mem.setf(g + 0x24, len);
    for k in 0..5u32 {
        let p = g + 0x10 * k;
        let (m, h, ras) = if k == 4 { (10, 5, [0x01E0_3E34u32, 0x01E0_3E60]) } else { (30, 15, [0x01E0_3E94, 0x01E0_3EC0]) };
        let v = rand_f(fx, k, ras[0], m, h);
        fx.mem.setf(p + 0x30, v);
        let v = rand_f(fx, k, ras[1], m, h);
        fx.mem.setf(p + 0x38, v);
        let v = ee::sub(len, ee::mul(step, ee::from_i32(k as i32 + 1)));
        fx.mem.setf(p + 0x34, v);
    }
    fx.leave(0x40);
}

/// 0x1E00120(desc, c, k) [F] (3272eb9440cc): a strip's four colours from
/// the B, G, R, A bytes at `c`, alpha less 16 (5 - k) (vertices 0 / 2) or
/// 16 (4 - k) (1 / 3), not below 0; Q 1; the vertices' top XYZ bytes 0.
fn column_colours(fx: &mut Fx, d: u32, c: u32, k: i32) {
    for (o, sub) in [(0x10u32, 5), (0x28, 4), (0x40, 5), (0x58, 4)] {
        for (s, t) in [(2u32, 0u32), (1, 1), (0, 2)] {
            let b = fx.mem.read_u8(c + s);
            fx.mem.write_u8(d + o + t, b);
        }
        let v = (sub - k) << 4;
        let a = fx.mem.u8(c + 3);
        fx.mem.set8(d + o + 3, if a < v { 0 } else { a - v });
        fx.mem.write_u32(d + o + 4, 0x3F80_0000);
    }
    for o in [0x27u32, 0x3F, 0x57, 0x6F] {
        fx.mem.write_u8(d + o, 0);
    }
}

/// 0x1E03F30(ot, g, x) [F] (4d7f719bbd8a, this record's own; also cards
/// 362 / 677): the column's pieces 0..4 - +0xC as textured strips (ALPHA
/// 0x48 fixed 0x80), each from the point before (the base for piece 0)
/// to its point, +0x2C wide in x; colours fading by piece (0x1E00120),
/// UVs the rectangle +0x10 in order 1, REGION_CLAMP to it, texture +0
/// (filter 1); drawn when all four corners project, keyed by their mean
/// w. A 0x150 frame.
pub(crate) fn column_draw(fx: &mut Fx, ot: u32, g: u32, x: u32) {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(sp + 0xA8, ot);
    let d = sp + 0xC0;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    fx.desc_alpha(d, 0x80_0000_0048);
    let m = fx.read_m4(x + 0x30);
    let mut k = 0i32;
    while k < 5 - fx.mem.i16(g + 0xC) {
        let s2 = g + 0x10 * k as u32;
        column_colours(fx, d, g + 4, k);
        fx.uv_rect(d, g + 0x10, 1);
        let mut bits = 0u32;
        let a = if k == 0 { g + 0x20 } else { g + 0x10 * (k as u32 - 1) + 0x30 };
        let b = s2 + 0x30;
        if fx.project_at(a, &m, sp + 0xE0, sp + 0xB0) {
            bits |= 0x10_0000;
        }
        if fx.project_at(b, &m, sp + 0xF8, sp + 0xB4) {
            bits |= 0x20_0000;
        }
        let w = fx.mem.f(g + 0x2C);
        let v = ee::add(fx.mem.f(a), w);
        fx.mem.setf(a, v);
        let w = fx.mem.f(g + 0x2C);
        let v = ee::add(fx.mem.f(b), w);
        fx.mem.setf(b, v);
        if fx.project_at(a, &m, sp + 0x110, sp + 0xB8) {
            bits |= 0x40_0000;
        }
        if fx.project_at(b, &m, sp + 0x128, sp + 0xBC) {
            bits |= 0x80_0000;
        }
        let w = fx.mem.f(g + 0x2C);
        let v = ee::sub(fx.mem.f(a), w);
        fx.mem.setf(a, v);
        let w = fx.mem.f(g + 0x2C);
        let v = ee::sub(fx.mem.f(b), w);
        fx.mem.setf(b, v);
        let (u0, v0, uw, vh) = (i64::from(fx.mem.i16(g + 0x10)), i64::from(fx.mem.i16(g + 0x12)), i64::from(fx.mem.i16(g + 0x14)), i64::from(fx.mem.i16(g + 0x16)));
        let clamp = ((u0 << 4) as u64 | 0xA) | (((u0 + uw - 1) << 14) as u64) | ((v0 << 24) as u64) | (((v0 + vh - 1) << 34) as u64);
        fx.desc_clamp(d, clamp);
        let rh2 = fx.mem.read_u32(g);
        fx.desc_texture(d, rh2, 1);
        if bits & 0xF0_0000 == 0xF0_0000 {
            let s = ee::add(fx.mem.f(sp + 0xB0), fx.mem.f(sp + 0xB4));
            let s = ee::add(fx.mem.f(sp + 0xB8), s);
            let s = ee::add(fx.mem.f(sp + 0xBC), s);
            let key = ee::fixunssfsi(ee::div(s, 4.0)) as i32;
            fx.submit(d, ot, key);
        }
        k += 1;
    }
    fx.leave(0x150);
}
