//! BMODULE b848 (card 677), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03130, data area 0x1ED0000, 0x2D40 bytes) [F]
//!: b728's (card 362,
//! `b728.rs`) layout on a longer timeline with a camera pass instead of
//! the attacker tint (s0 = the OT, s1 = D, s4 the parameter at launch).
//! Four target points (D+0x690 + 0x70 k, 0x1F65D0, swung 20 x cosf / sinf
//! of pi / 2 k, 30 up) with sixteen strands from the self unit's nodes
//! (D+0x70 + 0x30 k) up to 0x119; a ring of sixteen strands round node
//! 0x36 (D+0x370, 20 wide, 30 up) from 0xB5 to 0x119; from 0x11A b728's
//! four columns (D+0x830), tip quads, particle groups, square clouds and
//! facing quads; the full-window tint (D+0x2D28) every fifth update
//! 0x10F..0x135 and from 0x136 (its alpha up by 16 to 0x145, down by 8);
//! the stage light (D+0x2D3C) 0x80 down by 4 to 0x20, up from 0x140; every
//! update the target drawn into its render target under a camera of its
//! own (0x1F82B0 with [`pre`] / [`post`]) and composited as a quad on the
//! transform D+0x2960 between nodes 0x14 / 0x15 (0x1E01DD0, [`unit_pass`]).
//! Returns 1 on 0x115, 6 on 0x11B..0x13F, 3 on 0x140, 8 when the timer
//! D+0x2CF8 (0x154) runs out (the added colour and camera slots cleared).
//!
//! Library (record addresses): `colours4` 0x1E00080, `b262::tint`
//! 0x1E00270, `facing::quad` 0x1E003D0, `particle_quads` 0x1E00C60,
//! `pointquad::squares` 0x1E017F0, `strand::draw` 0x1E02270, b728's
//! column setup 0x1E02BF0, b021's column draw 0x1E02D40; record-local:
//! the unit pass 0x1E01DD0, the node picks 0x1E04280 ([`renodes`]).
use crate::b021::column_draw;
use crate::b079::sub3;
use crate::b142::fstep_up;
use crate::b728::{bytes, column_setup, halves, node, rand_f, set3};
use crate::blib::{facing, pointquad, strand};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const TIMER: u32 = D + 0x2CF8;
const N: u32 = D + 0x2CFC;
const SPACE: u32 = D + 0x1B70;
const SELF_PT: u32 = D + 0x1B90;
const STRANDS: u32 = D + 0x70;
const RING: u32 = D + 0x370;
const CAM: u32 = D + 0x2960;
const PASS: u32 = D + 0x29D0;
const PI_2: u32 = 0x3FC9_0FDB;
const PI_8: u32 = 0x3EC9_0FDB;
const STRAND_AT: u32 = 0x01E0_2270;
/// The unit pass's four corners (0x1E04400, 4 quads).
const QUAD: u32 = 0x01E0_4400;

/// The entry 0x1E03130.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x130);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x130);
    ret
}

fn n(fx: &mut Fx) -> i32 {
    fx.mem.i32(N)
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2D40);
    fx.xform_init(D);
    fx.xform_init(SPACE);
    for k in 0..4u32 {
        fx.xform_init(D + 0x1BE0 + 0x70 * k);
        set3(fx, D + 0x1BE0 + 0x70 * k, 0x3DCC_CCCD);
        fx.xform_init(D + 0x670 + 0x70 * k);
        fx.mem.write_u32(D + 0x2D2C + 4 * k, 0x41A0_0000);
    }
    fx.xform_init(CAM);
    for (at, nd, clear) in [(0x2D14u32, 0x36, 1), (0x2D18, 0x15, 0), (0x2D1C, 0x14, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, nd, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(TIMER, 0x154);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x2D00, param);
    fx.mem.set32(D + 0x2D04, 0);
    fx.mem.write_u8(D + 0x2D3C, 0x80);
    let h = fx.self_handle();
    let c = fx.unit_nodes(h);
    fx.mem.set32(D + 0x2D08, c - 1);
    bytes(fx, D + 0x2D28, &[(2, 0x80), (1, 0x80), (0, 0x80), (3, 0x80)]);
    fx.mem.write_u32(PASS, 0);
    fx.mem.write_u32(PASS + 4, 0x1000);
    bytes(fx, PASS, &[(0xA, 0x10), (9, 0x10), (8, 0x80), (0xB, 0x80)]);
    fx.mem.write_u32(PASS + 0xC, 0x4180_0000);
    fx.mem.write_u32(PASS + 0x10, 0);
    let t = fx.target_handle();
    fx.mem.set32(PASS + 0x14, t);
    fx.texture4(sp + 0x12C, 0xF);
    // Four square clouds (+0x1DA0 + 0x2F0 k) of 60 points (rand's s0 the
    // point's index).
    for k in 0..4u32 {
        let g = D + 0x1DA0 + 0x2F0 * k;
        let tex = fx.mem.read_u32(sp + 0x12C);
        fx.mem.write_u32(g, tex);
        fx.mem.set32(g + 4, 1);
        halves(fx, g, &[(0x10, 0xB0), (0x12, 0x40), (0x14, 0x20), (0x16, 0x20)]);
        fx.mem.write_u32(g + 0xC, 0x3F00_0000);
        for j in 0..0x3Cu32 {
            let p = g + 0x20 + 12 * j;
            if bit(fx.mem.read_u32(g + 4), 0x1D) {
                let v = rand_f(fx, j, 0x01E0_331C, 100, 50);
                fx.mem.setf(p, v);
                let v = rand_f(fx, j, 0x01E0_3348, 50, 0);
                fx.mem.setf(p + 8, v);
                let v = rand_f(fx, j, 0x01E0_3370, 100, 50);
                fx.mem.setf(p + 4, v);
            } else {
                let v = rand_f(fx, j, 0x01E0_33A4, 50, 25);
                fx.mem.setf(p, v);
                let v = rand_f(fx, j, 0x01E0_33D0, 25, 0);
                fx.mem.setf(p + 4, v);
                let v = rand_f(fx, j, 0x01E0_33F8, 50, 25);
                fx.mem.setf(p + 8, v);
            }
        }
        bytes(fx, g, &[(0xA, 0x40), (9, 0x40), (8, 0x80), (0xB, 0x80)]);
    }
    // Four particle groups (+0x1630 + 0x150 k) of 16.
    for k in 0..4u32 {
        let g = D + 0x1630 + 0x150 * k;
        let tex = fx.mem.read_u32(sp + 0x12C);
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
            let v = rand_f(fx, j, 0x01E0_3500, 0x50, 0x28);
            fx.mem.setf(p, v);
            let v = rand_f(fx, j, 0x01E0_352C, 0x50, 0);
            fx.mem.setf(p + 4, v);
            let f = -fx.mem.i16(g + 0x13A);
            let v = fx.mult(f, k as i32);
            fx.mem.set16(p + 0xE, v);
            fx.mem.set16(p + 0xC, 0);
            let v = fx.mem.i16(g + 0x140) - 1;
            fx.mem.set16(g + 0x13E, v);
        }
    }
    fx.texture5(sp + 0x12C, 0);
    // Sixteen strands (+0x70 + 0x30 k): odd ones far Z.
    for k in 0..0x10u32 {
        let g = STRANDS + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0x12C);
        fx.mem.write_u32(g, tex);
        fx.mem.write_u32(g + 4, if k & 1 != 0 { 0x0008_0000 } else { 0 });
        halves(fx, g, &[(0x10, 0x80), (0x12, 0x40), (0x14, 0x20), (0x16, 0x40)]);
        bytes(fx, g, &[(0xA, 0x80), (9, 0x80), (8, 0x80), (0xB, 0x20)]);
        halves(fx, g, &[(0xC, 0x10), (0x28, -(k as i32 + 1)), (0xE, 0x14)]);
        fx.mem.write_u32(g + 0x20, 0);
        fx.mem.write_u32(g + 0x24, 0);
    }
    // The ring of sixteen strands (+0x370 + 0x30 k).
    for k in 0..0x10u32 {
        let g = RING + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0x12C);
        fx.mem.write_u32(g, tex);
        fx.mem.write_u32(g + 4, 0x0400_0000);
        halves(fx, g, &[(0x10, 0x20), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
        bytes(fx, g, &[(0xA, 0x80), (9, 0x80), (8, 0x80), (0xB, 0x80)]);
        halves(fx, g, &[(0xC, 6), (0x28, 0), (0xE, 0x1E)]);
        fx.mem.write_u32(g + 0x20, 0);
        fx.mem.write_u32(g + 0x24, 0);
    }
    // Four columns of three strips (+0x830 + 0x180 k + 0x80 j), their tip
    // quads (+0xE30 + 0x120 k + 0x60 j, looks +0x12B0 + 0x60 k + 0x20 j).
    for k in 0..4u32 {
        for j in 0..3u32 {
            let c = D + 0x830 + 0x180 * k + 0x80 * j;
            halves(fx, c, &[(0xC, (j + 5 * k + 5) as i32), (0x10, 0), (0x12, 0), (0x14, 0x20), (0x16, 0x80), (0x18, 0), (0x1A, 0)]);
            fx.mem.write_u32(c + 0x2C, 0x41A0_0000);
            bytes(fx, c, &[(6, 0x80), (5, 0x80), (4, 0x80), (7, 0x80)]);
            let tex = fx.mem.read_u32(sp + 0x12C);
            column_setup(fx, c, tex, f32::from_bits(0x43D7_0000), 0x01E0_2BF0);
            let q = D + 0xE30 + 0x120 * k + 0x60 * j;
            fx.mem.set32(q + 0x38, 1);
            fx.mem.write_u32(q + 0x30, 0x3F80_0000);
            bytes(fx, q, &[(0x36, 0x80), (0x35, 0x80), (0x34, 0x80), (0x37, 0x80)]);
            halves(fx, q, &[(0, 0xE0), (2, 0), (4, 0x20), (6, 0x20), (8, 0xE0), (0xA, 0), (0xC, 0x100), (0xE, 0x80)]);
            halves(fx, q, &[(0x48, 0), (0x4A, 0), (0x4E, 2), (0x52, 0x3E7), (0x50, 0), (0x4C, 1)]);
            let l = D + 0x12B0 + 0x60 * k + 0x20 * j;
            let tex = fx.mem.read_u32(sp + 0x12C);
            set3(fx, l + 0x10, 0);
            fx.mem.write_u32(l + 8, 0x4170_0000);
            fx.mem.write_u32(l, tex);
            bytes(fx, l, &[(0xE, 0x80), (0xD, 0x80), (0xC, 0x80), (0xF, 0x80)]);
        }
    }
    // Four facing quads (+0x1430 + 0x60 k) and their looks (+0x15B0 + 0x20 k).
    for k in 0..4u32 {
        let q = D + 0x1430 + 0x60 * k;
        fx.mem.set32(q + 0x38, 1);
        fx.mem.write_u32(q + 0x30, 0x3F80_0000);
        bytes(fx, q, &[(0x36, 0x80), (0x35, 0x80), (0x34, 0x80), (0x37, 0x80)]);
        halves(fx, q, &[(0, 0xA0), (2, 0), (4, 0x40), (6, 0x40), (8, 0xA0), (0xA, 0), (0xC, 0x100), (0xE, 0x80)]);
        halves(fx, q, &[(0x48, 0), (0x4A, 0), (0x4E, 1), (0x52, 0x3E7), (0x50, 0), (0x4C, 0)]);
        let l = D + 0x15B0 + 0x20 * k;
        let tex = fx.mem.read_u32(sp + 0x12C);
        set3(fx, l + 0x10, 0);
        fx.mem.write_u32(l + 8, 0x41A0_0000);
        fx.mem.write_u32(l, tex);
        bytes(fx, l, &[(0xE, 0x80), (0xD, 0x80), (0xC, 0x80), (0xF, 0x80)]);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    // The four target points.
    for k in 0..4u32 {
        let p = D + 0x690 + 0x70 * k;
        let t = fx.target_handle();
        fx.unit_point(1, t, p);
        let a = ee::mul(f32::from_bits(PI_2), ee::from_i32(k as i32));
        let c = fx.cosf(a);
        let r = D + 0x2D2C + 4 * k;
        let v = ee::add(fx.mem.f(p + 8), ee::mul(fx.mem.f(r), c));
        fx.mem.setf(p + 8, v);
        let s = fx.sinf(a);
        let v = ee::add(fx.mem.f(p), ee::mul(fx.mem.f(r), s));
        fx.mem.setf(p, v);
        fx.mem.write_u32(p + 4, 0x41F0_0000);
        for o in [0u32, 4, 8, 0xC] {
            let w = fx.mem.read_u32(p + o);
            fx.mem.write_u32(D + 0x1C00 + 0x70 * k + o, w);
        }
    }
    // The ring's ends round node 0x36.
    node(fx, 0x2D14, D + 0x2BE8);
    for k in 0..0x10u32 {
        let r0 = D + 0x2BF8 + 0x10 * k;
        let v = ee::add(30.0, fx.mem.f(D + 0x2BEC));
        fx.mem.setf(r0 + 4, v);
        let a = ee::mul(f32::from_bits(PI_8), ee::from_i32(k as i32));
        let c = fx.cosf(a);
        let v = ee::add(fx.mem.f(D + 0x2BE8), ee::mul(20.0, c));
        fx.mem.setf(r0, v);
        let s = fx.sinf(a);
        let v = ee::add(fx.mem.f(D + 0x2BF0), ee::mul(20.0, s));
        fx.mem.setf(r0 + 8, v);
    }
    // The strands' ends.
    for k in 0..0x10u32 {
        let (a, b) = (D + 0x29E8 + 0x20 * k, D + 0x29F8 + 0x20 * k);
        if n(fx) < 0x11B {
            if fx.mem.i16(STRANDS + 0x30 * k + 0x28) == 0 {
                node(fx, 0x2D20, a);
                node(fx, 0x2D24, b);
                sub3(fx, a, SELF_PT);
                sub3(fx, b, SELF_PT);
            }
        } else {
            fx.mem.write_u32(a + 4, 0x4120_0000);
            let ang = ee::mul(f32::from_bits(PI_2), ee::from_i32((k & 3) as i32));
            let p = D + 0x690 + 0x70 * (k >> 2);
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
    // The camera pass's transform: at node 0x14, looking from it to 0x15.
    node(fx, 0x2D1C, CAM + 0x20);
    node(fx, 0x2D18, sp + 0x100);
    node(fx, 0x2D1C, sp + 0x110);
    fx.look_angles(sp + 0x100, sp + 0x110, CAM + 0x10);
    // 0x1038D0 / 0x1039A0 with their frames (s0 = the OT, s1 = D, s2 / s3
    // the strand loop's D + 0x300 / D + 0x200, then the loop's D + 0x70 k
    // / k).
    let s = [Some(OT), Some(D), Some(D + 0x300), Some(D + 0x200)];
    fx.xform_world_saving(D, 0x01E0_3BAC, s);
    fx.xform_world_saving(SPACE, 0x01E0_3BB4, s);
    for k in 0..4u32 {
        let s = [Some(OT), Some(D), Some(D + 0x70 * k), Some(k)];
        fx.xform_billboard_saving(D + 0x1BE0 + 0x70 * k, 0x01E0_3BC4, s);
        fx.xform_billboard_saving(D + 0x670 + 0x70 * k, 0x01E0_3BCC, s);
    }
    fx.xform_world_saving(CAM, 0x01E0_3BE4, [Some(OT), Some(D), Some(D + 0x1C0), Some(4)]);
    if n(fx) < 0x140 {
        fx.step_down(D + 0x2D3C, 0x20, 4);
    } else {
        fx.step_up(D + 0x2D3C, 0x80, 4);
    }
    let l = fx.mem.read_u8(D + 0x2D3C);
    fx.set_stage_light(i32::from(l));
    let m = n(fx);
    if (0x10F..0x136).contains(&m) {
        let (_, r) = fx.div(m, 5);
        if r == 0 {
            crate::b262::tint(fx, D + 0x2D28);
        }
    }
    if n(fx) >= 0x136 {
        crate::b262::tint(fx, D + 0x2D28);
        if n(fx) < 0x146 {
            fx.step_up(D + 0x2D2B, 0xFF, 0x10);
        }
        fx.step_down(D + 0x2D2B, 0, 8);
    }
    if n(fx) == 0x60 {
        bytes(fx, PASS, &[(0xA, 0x80), (9, 0x10), (8, 0x10)]);
    }
    unit_pass(fx, PASS, CAM);
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
        for k in 0..0x10u32 {
            strand::draw(fx, STRAND_AT, OT, STRANDS + 0x30 * k, SPACE, D + 0x29E8 + 0x20 * k, D + 0x29F8 + 0x20 * k);
            if n(fx) >= 0xB5 {
                strand::draw(fx, STRAND_AT, OT, RING + 0x30 * k, D, D + 0x2BE8, D + 0x2BF8 + 0x10 * k);
            }
        }
    } else {
        for k in 0..4u32 {
            let x = D + 0x670 + 0x70 * k;
            if fx.mem.i16(D + 0x83C + 0x180 * k) == 0 {
                let q = D + 0x1430 + 0x60 * k;
                facing::quad(fx, OT, q, D + 0x15B0, x);
                fx.step_down(q + 0x37, 0, 8);
                fstep_up(fx, q + 0x30, 5.0, f32::from_bits(0x3E4C_CCCD));
                for o in [0x108u32, 0x104, 0x100] {
                    fx.mem.write_u32(sp + o, 0);
                }
                let g = D + 0x1630 + 0x150 * k;
                fx.particle_quads(x, g, sp + 0x100);
                fx.step_down(g + 0xB, 0, 2);
                for o in [0x108u32, 0x104, 0x100] {
                    fx.mem.write_u32(sp + o, 0x3F80_0000);
                }
                let s = D + 0x1BE0 + 0x70 * k;
                let g = D + 0x1DA0 + 0x2F0 * k;
                pointquad::squares(fx, OT, g, s, sp + 0x100);
                fx.approach(s, 4.0, f32::from_bits(0x3E99_999A));
                fx.step_down(g + 0xB, 0, 8);
            } else {
                fx.mem.write_u8(D + 0x1467 + 0x60 * k, 0x80);
                fx.mem.write_u32(D + 0x1460 + 0x60 * k, 0x3F80_0000);
                fx.mem.write_u8(D + 0x163B + 0x150 * k, 0x20);
                fx.mem.write_u8(D + 0x1DAB + 0x2F0 * k, 0x80);
                set3(fx, D + 0x1BE0 + 0x70 * k, 0x3DCC_CCCD);
            }
        }
        for k in 0..0x10u32 {
            let g = STRANDS + 0x30 * k;
            if fx.mem.i16(D + 0x83C + 0x180 * (k >> 2)) == 0 {
                strand::draw(fx, STRAND_AT, OT, g, D, D + 0x690 + 0x70 * (k >> 2), D + 0x29E8 + 0x20 * k);
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
    if (0x11A..0x137).contains(&m) {
        for k in 0..4u32 {
            for j in 0..3u32 {
                let c = D + 0x830 + 0x180 * k + 0x80 * j;
                let a = fx.mem.u8(c + 7);
                if a < 0x60 {
                    let v = fx.mem.i16(c + 0x18) + fx.mem.i16(c + 0x14);
                    fx.mem.set16(c + 0x10, v);
                }
                let x = D + 0x670 + 0x70 * k;
                column_draw(fx, OT, c, x);
                let cnt = fx.mem.i16(c + 0xC);
                if cnt < 6 {
                    let p = (c + 0x30).wrapping_add(0x10u32.wrapping_mul((4 - cnt) as u32));
                    let l = D + 0x12B0 + 0x60 * k + 0x20 * j;
                    let v = ee::add(10.0, fx.mem.f(p));
                    fx.mem.setf(l + 0x10, v);
                    let w = fx.mem.read_u32(p + 8);
                    fx.mem.write_u32(l + 0x18, w);
                    let w = fx.mem.read_u32(p + 4);
                    fx.mem.write_u32(l + 0x14, w);
                    let q = D + 0xE30 + 0x120 * k + 0x60 * j;
                    let s = if cnt == 0 { 0x3FC0_0000 } else { 0x3F80_0000 };
                    fx.mem.write_u32(q + 0x30, s);
                    facing::quad(fx, OT, q, l, x);
                    let a = fx.mem.read_u8(c + 7);
                    fx.mem.write_u8(q + 0x37, a);
                }
                let cnt = fx.mem.i16(c + 0xC);
                if cnt != 0 {
                    fx.mem.set16(c + 0xC, cnt - 1);
                } else {
                    fx.step_down(c + 7, 0, 8);
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
    if m == 0x140 {
        3
    } else if m == 0x115 {
        1
    } else if (0x11B..0x140).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        let h = fx.self_handle();
        fx.unit_add_colour(h, 0);
        crate::blib::exe::camera_slots45(fx, 0);
        8
    }
}

/// 0x1E04280(D, ot) [F] (this record's own; b728's 0x1E04270 with the
/// node count instead of a table): for each strand whose piece counter
/// +0x28 is 0, a node index 1 + rand % (nodes - 1) into D+0x2D0C and the
/// one two on (or two back, when that is not below the count) into
/// D+0x2D10; the nodes 0x36 (cleared) / 0x15 / 0x14 and those two
/// re-requested into D+0x2D14 .. 0x2D24.
fn renodes(fx: &mut Fx) {
    fx.enter(0x40);
    for k in 0..0x10u32 {
        if fx.mem.i16(STRANDS + 0x30 * k + 0x28) != 0 {
            continue;
        }
        let r = fx.rand_from(k, 0x01E0_42B4);
        let m = fx.mem.i32(D + 0x2D08);
        let (_, h) = fx.div(r, m);
        fx.mem.set32(D + 0x2D0C, h + 1);
        let i = fx.mem.i32(D + 0x2D0C);
        let c = fx.mem.i32(D + 0x2D08);
        fx.mem.set32(D + 0x2D10, if i + 2 < c { i + 2 } else { i - 2 });
        for (at, nd, clear) in [(0x2D14u32, 0x36, 1), (0x2D18, 0x15, 0), (0x2D1C, 0x14, 0)] {
            let h = fx.self_handle();
            let r = fx.request_node(h, nd, clear);
            fx.mem.set32(D + at, r);
        }
        for (at, from) in [(0x2D20u32, 0x2D0Cu32), (0x2D24, 0x2D10)] {
            let h = fx.self_handle();
            let nd = fx.mem.i32(D + from);
            let r = fx.request_node(h, nd, 0);
            fx.mem.set32(D + at, r);
        }
    }
    fx.leave(0x40);
}

/// 0x1E01DD0(ot, rec, x) [F] (frame 0x170): in a packet layer the target
/// (rec + 0x14) drawn into its render target under [`pre`]'s camera (the
/// distance word 0x1E04510 = the handle, -1 for 0) and composited as a
/// textured quad (descriptor sp + 0x98, prim 0x34) of half width 0.32 x
/// rec + 0xC, half depth 0.24 x it, at height rec + 0x10 on the transform
/// `x` (0x205210 through x + 0x30), with an untextured strip (sp + 0x128)
/// in rec's colour on the same corners; the quad's blend by rec + 4's bits
/// 0xC / 0xB / 0xA / none, its colour (0x80, 0x80, 0x80, rec + 0xB).
pub fn unit_pass(fx: &mut Fx, rec: u32, x: u32) {
    let sp = fx.enter(0x170);
    let layer = fx.layer_begin();
    for k in 0..0x10u32 {
        let w = fx.mem.read_u32(QUAD + 4 * k);
        fx.mem.write_u32(sp + 0x40 + 4 * k, w);
    }
    let (a, b) = (f32::from_bits(0x3EA3_D70A), f32::from_bits(0x3E75_C28F));
    for (k, (sx, sz)) in [(-a, -b), (a, -b), (-a, b), (a, b)].into_iter().enumerate() {
        let v = sp + 0x40 + 0x10 * k as u32;
        let c = fx.mem.f(rec + 0xC);
        fx.mem.setf(v, ee::mul(sx, c));
        let y = fx.mem.read_u32(rec + 0x10);
        fx.mem.write_u32(v + 4, y);
        let c = fx.mem.f(rec + 0xC);
        fx.mem.setf(v + 8, ee::mul(sz, c));
    }
    let h = fx.mem.i32(rec + 0x14);
    fx.mem.set32(SAVED + 0x90, if h != 0 { h } else { -1 });
    let (d, e) = (sp + 0x98, sp + 0x128);
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u16(e + 2, 4);
    fx.mem.write_u16(e, 5);
    fx.mem.write_u64(e + 8, 0x4_4441);
    fx.mem.write_u32(e + 4, 0);
    let f = fx.mem.read_u32(rec + 4);
    let blend = if bit(f, 0xC) {
        0x44
    } else if bit(f, 0xB) {
        0x88
    } else if bit(f, 0xA) {
        0x42
    } else {
        0x48
    };
    fx.desc_alpha(d, 0x80_0000_0000 | blend);
    let old = u32::from(fx.mem.read_u16(e + 2));
    fx.desc_alpha(e, 0x80_0000_0044);
    // a0..a3 as 0x207E20 leaves them: the descriptor, the ALPHA value, the
    // flags | 0x200 and the flags.
    fx.texflush([e, 0x44, old | 0x200, old]);
    // a1..a3 handed from 0x1EF8F0 (the object index, 0, 0) to the pre
    // callback, then from it to the post.
    let (side, slot) = (h >> 8, h & 0xFF);
    let i = if (0..3).contains(&side) && slot < 0xB { fx.object_index(side, slot) as u32 } else { 0 };
    let regs = std::cell::Cell::new([i, 0, 0]);
    let mut pcb = |fx: &mut Fx, p: u32| {
        let (v, r) = pre(fx, p, regs.get()[0]);
        regs.set(r);
        v
    };
    let mut qcb = |fx: &mut Fx, p: u32| {
        let (v, r) = post(fx, p, regs.get());
        regs.set(r);
        v
    };
    let (_, r) = fx.unit_target_pre_post(h, 1, sp + 0x110, sp + 0x118, (0x01E0_2180, &mut pcb), Some((0x01E0_2230, &mut qcb)));
    fx.texflush(r);
    fx.desc_clamp(d, 0x0000_0800_0080_000A);
    for k in 0..3 {
        fx.mem.write_u8(sp + 0x16C + k, 0x80);
    }
    let al = fx.mem.read_u8(rec + 0xB);
    fx.mem.write_u8(sp + 0x16F, al);
    fx.colours4(d, sp + 0x16C);
    for (o, w) in [(0xB0u32, 0u32), (0xB4, 0), (0xC8, 0x3F80_0000), (0xCC, 0), (0xE0, 0), (0xE4, 0x3F80_0000), (0xF8, 0x3F80_0000), (0xFC, 0x3F80_0000)] {
        fx.mem.write_u32(sp + o, w);
    }
    let m = fx.read_m4(x + 0x30);
    if fx.project_desc(sp + 0x40, 0x7000_0000, Some(&m), d, sp + 0x80) {
        for (o, from) in [(0u32, 0xAu32), (1, 9), (2, 8), (3, 0xB)] {
            let c = fx.mem.read_u8(rec + from);
            fx.mem.write_u8(e + 0x10 + o, c);
        }
        for (to, from) in [(0x140u32, 0xB8u32), (0x148, 0xD0), (0x150, 0xE8), (0x158, 0x100)] {
            let v = fx.mem.read_u64(sp + from);
            fx.mem.write_u64(sp + to, v);
        }
        fx.submit(e, 0, 0);
        fx.submit(d, 0, 0);
    }
    fx.layer_end(layer, OT, 0);
    fx.leave(0x170);
}

/// The camera input the pre callback saves (9 quads) and the word after
/// it (0x1E04510, an int: the camera's distance back along -z).
const SAVED: u32 = 0x01E0_4480;

/// 0x1E02180(p) [F]: the pre-model callback, [`camera_pre`] on the
/// input 0x1E04480 and the distance 0x1E04510 (frame 0xB0, local + 0x20).
pub(crate) fn pre(fx: &mut Fx, p: u32, a1: u32) -> (u32, [u32; 3]) {
    camera_pre(fx, p, a1, SAVED, SAVED + 0x90, 0xB0, 0x20)
}

/// 0x1E02230(p) [F]: the post callback, [`camera_post`] on 0x1E04480.
pub(crate) fn post(fx: &mut Fx, p: u32, regs: [u32; 3]) -> (u32, [u32; 3]) {
    camera_post(fx, p, regs, SAVED)
}

/// A pre-model unit callback that draws the unit with a camera of its
/// own [F] (b848's 0x1E02180; card 500's 0x1E04F10 is the same code with
/// the input at D + 0xAE40, the distance at D + 0xC, frame 0xC0, local
/// + 0x30): the current camera input saved to `saved` (0x20D660), copied
/// to the frame's `local` with the position (+0x40) = (0, 50, 0, 1) and
/// the direction (+0x50) = (0, 0, -1.0 x (float) [dist], 1), set
/// (0x20DAD0) and its VU1 state written into the packet (0x20E3D0);
/// returns the pointer after and a1..a3 as it leaves them. The three exe
/// calls are boundary calls of the runner, reported with a0..a3: a1 at
/// entry is 0x1EF8F0's (the object index `a1`), a2 / a3 0.
pub fn camera_pre(fx: &mut Fx, p: u32, a1: u32, saved: u32, dist: u32, frame: u32, local: u32) -> (u32, [u32; 3]) {
    let sp = fx.enter(frame);
    let l = sp + local;
    fx.boundary(0x0020_D660, "camera copy 0x20D660", [saved, a1, 0, 0]);
    fx.camera_save(saved);
    let index = fx.mem.read_u32(crate::world::CAMERA_STACK_INDEX);
    for k in 0..9u32 {
        for w in 0..4u32 {
            let x = fx.mem.read_u32((saved & !15) + 0x10 * k + 4 * w);
            fx.mem.write_u32((l & !15) + 0x10 * k + 4 * w, x);
        }
    }
    fx.mem.write_u32(l + 0x40, 0);
    fx.mem.write_u32(l + 0x44, 0x4248_0000);
    fx.mem.write_u32(l + 0x48, 0);
    fx.mem.write_u32(l + 0x4C, 0x3F80_0000);
    fx.mem.write_u32(l + 0x50, 0);
    fx.mem.write_u32(l + 0x54, 0);
    let d = ee::from_i32(fx.mem.i32(dist));
    fx.mem.setf(l + 0x58, ee::mul(-1.0, d));
    fx.mem.write_u32(l + 0x5C, 0x3F80_0000);
    fx.boundary(0x0020_DAD0, "camera set 0x20DAD0", [l, saved + 0x90, index, 0]);
    let r = fx.camera_set_regs(l);
    fx.boundary(0x0020_E3D0, "0x20E3D0", [p, r[0], r[1], r[2]]);
    let v = fx.camera_packet_regs(p);
    fx.leave(frame);
    v
}

/// The post callback paired with [`camera_pre`] [F] (b848's 0x1E02230,
/// card 500's 0x1E04FD0; frame 0x20): the saved input `saved` set back
/// (0x20DAD0) and written into the packet (0x20E3D0); returns the pointer
/// after and a1..a3 as it leaves them (`regs` = a1..a3 at entry, as the
/// pre callback left them).
pub fn camera_post(fx: &mut Fx, p: u32, regs: [u32; 3], saved: u32) -> (u32, [u32; 3]) {
    fx.enter(0x20);
    fx.boundary(0x0020_DAD0, "camera set 0x20DAD0", [saved, regs[0], regs[1], regs[2]]);
    let r = fx.camera_set_regs(saved);
    fx.boundary(0x0020_E3D0, "0x20E3D0", [p, r[0], r[1], r[2]]);
    let v = fx.camera_packet_regs(p);
    fx.leave(0x20);
    v
}

/// The callback at `addr` run on the packet `p` with a1..a3 = `regs` at
/// entry (a pre takes a1 only), for the fnparity adapter of 0x1F82B0:
/// b848's 0x1E02180 / 0x1E02230 and card 500's 0x1E04F10 / 0x1E04FD0
/// (D = [0x1E05100]); the pointer after and a1..a3 after.
pub fn callback(fx: &mut Fx, addr: u32, p: u32, regs: [u32; 3]) -> (u32, [u32; 3]) {
    let d500 = || 0x01E0_5100u32;
    match addr {
        0x01E0_2180 => pre(fx, p, regs[0]),
        0x01E0_2230 => post(fx, p, regs),
        0x01E0_4F10 => {
            let d = fx.mem.read_u32(d500());
            camera_pre(fx, p, regs[0], d + 0xAE40, d + 0xC, 0xC0, 0x30)
        }
        0x01E0_4FD0 => {
            let d = fx.mem.read_u32(d500());
            camera_post(fx, p, regs, d + 0xAE40)
        }
        _ => panic!("b848: no callback at {addr:#x}"),
    }
}
