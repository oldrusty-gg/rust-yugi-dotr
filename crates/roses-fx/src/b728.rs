//! BMODULE b728 (card 362), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02C90, data area 0x1ED0000, 0x2CC0 bytes) [F]
//!: b021's (card 294, `b021.rs`) monolith
//! without its quad cloud, point quads, camera light and second ring,
//! on a shorter timeline (s0 = D, s1 the OT, s3 the parameter at
//! launch). Four target points (D+0x690 + 0x70 k, 0x1F65D0, swung 85 x
//! the table cosine / sine (0x20ABF0 / 0x20ACC0) of 0x2000 + 0x4000 k, 30
//! up) with sixteen strands each from the self unit's nodes (D+0x70 +
//! 0x30 k, `strand::draw`, from 0x24); a ring of sixteen strands round
//! node 0x1C (D+0x370, 80 wide, 100 up, from 0x5A, fading from 0x80);
//! from 0x80 four columns of three five-piece strips (D+0x830, b021's
//! column draw) at the points with facing quads (D+0xE30, `facing::quad`)
//! on their tips, particle groups (D+0x1630), square clouds (D+0x1DA0),
//! facing quads (D+0x1430) and the strands re-aimed at the points; the
//! full-window tint (D+0x2C98, b262's) every fifth update 0x86..0xC7 and
//! from 0xC8; the stage light (D+0x2CB0) 0x80 down by 4 to 0x20, up by 4
//! from 0xC9; the self unit's added colour (D+0x2CAC, 0x1F7790) below
//! 0x80. Returns 1 on 0x80, 6 on 0x85..0xC8, 3 on 0xC9, 8 when the timer
//! D+0x2C70 (0xDC) runs out.
//!
//! Library (record addresses): `colours4` 0x1E00080, `b262::tint`
//! 0x1E00270, `facing::quad` 0x1E003D0, `particle_quads` 0x1E00C60,
//! `pointquad::squares` 0x1E017F0, `strand::draw` 0x1E01DD0; b021's
//! column setup / draw at 0x1E02750 / 0x1E028A0 (the setup copied here
//! for its rand return addresses), the node picks 0x1E04270 (record-local:
//! a 50-byte table, the second node the one before when the first is
//! last).
use crate::b021::column_draw;
use crate::b079::sub3;
use crate::b142::fstep_up;
use crate::b436::fstep_down;
use crate::blib::{facing, pointquad, strand};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const TIMER: u32 = D + 0x2C70;
const N: u32 = D + 0x2C74;
const SPACE: u32 = D + 0x1B70;
const SELF_PT: u32 = D + 0x1B90;
const STRANDS: u32 = D + 0x70;
const RING: u32 = D + 0x370;
/// The 50-byte node table of 0x1E04270.
const NODE_TABLE: u32 = 0x01E0_4400;
const PI_2: u32 = 0x3FC9_0FDB;
const PI_8: u32 = 0x3EC9_0FDB;
const STRAND_AT: u32 = 0x01E0_1DD0;

/// The entry 0x1E02C90.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xE0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xE0);
    ret
}

pub(crate) fn halves(fx: &mut Fx, at: u32, v: &[(u32, i32)]) {
    for &(k, x) in v {
        fx.mem.set16(at + k, x);
    }
}

pub(crate) fn bytes(fx: &mut Fx, at: u32, v: &[(u32, u8)]) {
    for &(k, x) in v {
        fx.mem.write_u8(at + k, x);
    }
}

pub(crate) fn set3(fx: &mut Fx, at: u32, w: u32) {
    for k in [8u32, 4, 0] {
        fx.mem.write_u32(at + k, w);
    }
}

/// rand() % m - sub as a float, rand's frame saving the caller's s0.
pub(crate) fn rand_f(fx: &mut Fx, s0: u32, ra: u32, m: i32, sub: i32) -> f32 {
    let r = fx.rand_from(s0, ra);
    let (_, h) = fx.div(r, m);
    ee::from_i32(h - sub)
}

pub(crate) fn node(fx: &mut Fx, slot: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + slot);
    fx.node_position(h, s, out);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2CC0);
    fx.xform_init(D);
    fx.xform_init(SPACE);
    for k in 0..4u32 {
        fx.xform_init(D + 0x1BE0 + 0x70 * k);
        set3(fx, D + 0x1BE0 + 0x70 * k, 0x3DCC_CCCD);
        fx.xform_init(D + 0x670 + 0x70 * k);
        fx.mem.write_u32(D + 0x2C9C + 4 * k, 0x42AA_0000);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x1C, 1);
    fx.mem.set32(D + 0x2C8C, r);
    fx.mem.set32(TIMER, 0xDC);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x2C78, param);
    fx.mem.set32(D + 0x2C7C, 0);
    fx.mem.write_u8(D + 0x2CB0, 0x80);
    let h = fx.self_handle();
    let c = fx.unit_nodes(h);
    fx.mem.set32(D + 0x2C80, c - 1);
    bytes(fx, D + 0x2C98, &[(2, 0x80), (1, 0x80), (0, 0x80), (3, 0x80)]);
    bytes(fx, D + 0x2CAC, &[(2, 0x30), (1, 0x60), (0, 0x80), (3, 0)]);
    fx.texture4(sp + 0xDC, 0xF);
    // Four square clouds (+0x1DA0 + 0x2F0 k) of 60 points.
    for k in 0..4u32 {
        let g = D + 0x1DA0 + 0x2F0 * k;
        let tex = fx.mem.read_u32(sp + 0xDC);
        fx.mem.write_u32(g, tex);
        fx.mem.set32(g + 4, 1);
        halves(fx, g, &[(0x10, 0xB0), (0x12, 0x40), (0x14, 0x20), (0x16, 0x20)]);
        fx.mem.write_u32(g + 0xC, 0x3F00_0000);
        for j in 0..0x3Cu32 {
            let p = g + 0x20 + 12 * j;
            if bit(fx.mem.read_u32(g + 4), 0x1D) {
                let v = rand_f(fx, D, 0x01E0_2E94, 100, 50);
                fx.mem.setf(p, v);
                let v = rand_f(fx, D, 0x01E0_2ED8, 50, 0);
                fx.mem.setf(p + 8, v);
                let v = rand_f(fx, D, 0x01E0_2F0C, 100, 50);
                fx.mem.setf(p + 4, v);
            } else {
                let v = rand_f(fx, D, 0x01E0_2F4C, 50, 25);
                fx.mem.setf(p, v);
                let v = rand_f(fx, D, 0x01E0_2F90, 25, 0);
                fx.mem.setf(p + 4, v);
                let v = rand_f(fx, D, 0x01E0_2FC4, 50, 25);
                fx.mem.setf(p + 8, v);
            }
        }
        bytes(fx, g, &[(0xA, 0x40), (9, 0x40), (8, 0x80), (0xB, 0x80)]);
    }
    // Four particle groups (+0x1630 + 0x150 k) of 16.
    for k in 0..4u32 {
        let g = D + 0x1630 + 0x150 * k;
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
            let v = rand_f(fx, D, 0x01E0_310C, 0x50, 0x28);
            fx.mem.setf(p, v);
            let v = rand_f(fx, D, 0x01E0_3148, 0x50, 0);
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
        halves(fx, g, &[(0xC, 0x10), (0x28, -(k as i32 + 1)), (0xE, 0x32)]);
        fx.mem.write_u32(g + 0x20, 0);
        fx.mem.write_u32(g + 0x24, 0);
    }
    // The ring of sixteen strands (+0x370 + 0x30 k).
    for k in 0..0x10u32 {
        let g = RING + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0xDC);
        fx.mem.write_u32(g, tex);
        fx.mem.write_u32(g + 4, 0x0C00_0000);
        let (_, h) = fx.div(k as i32, 3);
        halves(fx, g, &[(0x10, h << 5), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
        bytes(fx, g, &[(0xA, 0x80), (9, 0x80), (8, 0x80), (0xB, 0x80)]);
        halves(fx, g, &[(0xC, 6), (0x28, 0), (0xE, 0x50)]);
        fx.mem.write_u32(g + 0x20, 0);
        fx.mem.write_u32(g + 0x24, 0);
        fx.mem.write_u32(g + 0x2C, 0x43C8_0000);
    }
    // Four columns of three strips (+0x830 + 0x180 k + 0x80 j), their tip
    // quads (+0xE30 + 0x120 k + 0x60 j, looks +0x12B0 + 0x60 k + 0x20 j).
    for k in 0..4u32 {
        for j in 0..3u32 {
            let c = D + 0x830 + 0x180 * k + 0x80 * j;
            halves(fx, c, &[(0xC, (j + 5 * k + 5) as i32), (0x10, 0), (0x12, 0), (0x14, 0x20), (0x16, 0x80), (0x18, 0), (0x1A, 0)]);
            fx.mem.write_u32(c + 0x2C, 0x41A0_0000);
            bytes(fx, c, &[(6, 0x80), (5, 0x80), (4, 0x80), (7, 0x80)]);
            let tex = fx.mem.read_u32(sp + 0xDC);
            column_setup(fx, c, tex, f32::from_bits(0x43D7_0000), 0x01E0_2750);
            let q = D + 0xE30 + 0x120 * k + 0x60 * j;
            fx.mem.set32(q + 0x38, 1);
            fx.mem.write_u32(q + 0x30, 0x3F80_0000);
            bytes(fx, q, &[(0x36, 0x80), (0x35, 0x80), (0x34, 0x80), (0x37, 0x80)]);
            halves(fx, q, &[(0, 0xE0), (2, 0), (4, 0x20), (6, 0x20), (8, 0xE0), (0xA, 0), (0xC, 0x100), (0xE, 0x80)]);
            halves(fx, q, &[(0x48, 0), (0x4A, 0), (0x4E, 2), (0x52, 0x3E7), (0x50, 0), (0x4C, 1)]);
            let l = D + 0x12B0 + 0x60 * k + 0x20 * j;
            let tex = fx.mem.read_u32(sp + 0xDC);
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
        let p = D + 0x690 + 0x70 * k;
        let t = fx.target_handle();
        fx.unit_point(1, t, p);
        let a = ((k as i32) << 14) + 0x2000;
        let c = fx.tcos(a);
        let r = D + 0x2C9C + 4 * k;
        let v = ee::add(fx.mem.f(p + 8), ee::mul(fx.mem.f(r), c));
        fx.mem.setf(p + 8, v);
        let s = fx.tsin(a);
        let v = ee::add(fx.mem.f(p), ee::mul(fx.mem.f(r), s));
        fx.mem.setf(p, v);
        fx.mem.write_u32(p + 4, 0x41F0_0000);
        for o in [0u32, 4, 8, 0xC] {
            let w = fx.mem.read_u32(p + o);
            fx.mem.write_u32(D + 0x1C00 + 0x70 * k + o, w);
        }
    }
    node(fx, 0x2C8C, D + 0x2B60);
    for k in 0..0x10u32 {
        let r0 = D + 0x2B70 + 0x10 * k;
        let v = ee::add(100.0, fx.mem.f(D + 0x2B64));
        fx.mem.setf(r0 + 4, v);
        let a = ee::mul(f32::from_bits(PI_8), ee::from_i32(k as i32));
        let c = fx.cosf(a);
        let v = ee::add(fx.mem.f(D + 0x2B60), ee::mul(80.0, c));
        fx.mem.setf(r0, v);
        let s = fx.sinf(a);
        let v = ee::add(fx.mem.f(D + 0x2B68), ee::mul(80.0, s));
        fx.mem.setf(r0 + 8, v);
    }
    // The strands' ends.
    for k in 0..0x10u32 {
        let (a, b) = (D + 0x2960 + 0x20 * k, D + 0x2970 + 0x20 * k);
        if n(fx) < 0x85 {
            if fx.mem.i16(STRANDS + 0x30 * k + 0x28) == 0 {
                node(fx, 0x2C90, a);
                node(fx, 0x2C94, b);
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
    // 0x1038D0 / 0x1039A0 with their frames (the entry's s0 = D, s1 = the
    // OT, s2 / s3 the loop's k / D + 0x70 k), as b021's.
    fx.xform_world_saving(D, 0x01E0_38CC, [Some(D), Some(OT), None, None]);
    fx.xform_world_saving(SPACE, 0x01E0_38D8, [Some(D), Some(OT), None, None]);
    for k in 0..4u32 {
        let s = [Some(D), Some(OT), Some(k), Some(D + 0x70 * k)];
        fx.xform_billboard_saving(D + 0x1BE0 + 0x70 * k, 0x01E0_3900, s);
        fx.xform_billboard_saving(D + 0x670 + 0x70 * k, 0x01E0_390C, s);
    }
    if n(fx) < 0xC9 {
        fx.step_down(D + 0x2CB0, 0x20, 4);
    } else {
        fx.step_up(D + 0x2CB0, 0x80, 4);
    }
    let l = fx.mem.read_u8(D + 0x2CB0);
    fx.set_stage_light(i32::from(l));
    let m = n(fx);
    if (0x86..0xC8).contains(&m) {
        let (_, r) = fx.div(m, 5);
        if r == 0 {
            crate::b262::tint(fx, D + 0x2C98);
        }
    }
    if n(fx) == 0xC8 {
        fx.mem.write_u8(D + 0x2C9B, 0xFF);
    }
    if n(fx) >= 0xC8 {
        crate::b262::tint(fx, D + 0x2C98);
        if n(fx) >= 0xD0 {
            fx.step_down(D + 0x2C9B, 0, 4);
        }
    }
    if n(fx) == 0x80 {
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
    if n(fx) >= 0x5A {
        for k in 0..0x10u32 {
            let g = RING + 0x30 * k;
            strand::draw(fx, STRAND_AT, OT, g, D, D + 0x2B60, D + 0x2B70 + 0x10 * k);
            if n(fx) >= 0x80 {
                fx.step_down(g + 0xB, 0, 8);
            }
        }
    }
    if n(fx) < 0x80 {
        let h = fx.self_handle();
        fx.unit_add_colour_from(h, D + 0x2CAC, [D, OT], 0x01E0_3B40);
        let a = if n(fx) & 1 != 0 { 0 } else { 0x20 };
        fx.mem.write_u8(D + 0x2CAF, a);
        if n(fx) >= 0x24 {
            for k in 0..0x10u32 {
                strand::draw(fx, STRAND_AT, OT, STRANDS + 0x30 * k, SPACE, D + 0x2960 + 0x20 * k, D + 0x2970 + 0x20 * k);
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
                for o in [0xC8u32, 0xC4, 0xC0] {
                    fx.mem.write_u32(sp + o, 0);
                }
                let g = D + 0x1630 + 0x150 * k;
                fx.particle_quads(x, g, sp + 0xC0);
                fx.step_down(g + 0xB, 0, 2);
                for o in [0xC8u32, 0xC4, 0xC0] {
                    fx.mem.write_u32(sp + o, 0x3F80_0000);
                }
                let s = D + 0x1BE0 + 0x70 * k;
                let g = D + 0x1DA0 + 0x2F0 * k;
                pointquad::squares(fx, OT, g, s, sp + 0xC0);
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
                strand::draw(fx, STRAND_AT, OT, g, D, D + 0x690 + 0x70 * (k >> 2), D + 0x2960 + 0x20 * k);
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
    if (0x80..0xC9).contains(&m) {
        for k in 0..4u32 {
            for j in 0..3u32 {
                let c = D + 0x830 + 0x180 * k + 0x80 * j;
                let a = fx.mem.u8(c + 7);
                if a < 0x60 {
                    let v = fx.mem.i16(c + 0x18) + fx.mem.i16(c + 0x14);
                    fx.mem.set16(c + 0x10, v);
                }
                let x = D + 0x670 + 0x70 * k;
                fx.mem.write_u32(sp + 0xB0, x);
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
                } else if fx.step_down(c + 7, 0, 8) == 0 {
                    fx.mem.set16(c + 0xC, 5);
                    fx.mem.write_u8(c + 7, 0x80);
                    if j == 2 {
                        fstep_down(fx, D + 0x2C9C + 4 * k, 20.0, 15.0);
                    }
                }
            }
        }
    }
    if n(fx) < 0x80 {
        renodes(fx);
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(TIMER) - 1;
    fx.mem.set32(TIMER, left);
    if m == 0xC9 {
        3
    } else if m == 0x80 {
        1
    } else if (0x85..0xC9).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        let h = fx.self_handle();
        fx.unit_add_colour(h, 0);
        8
    }
}

/// 0x1E04270(D, ot) [F] (1c2e70b84c36, this record's own; b021's
/// 0x1E05D00 with a 50-byte table): for each strand whose piece counter
/// +0x28 is 0, a random node index (rand % 50) and the next (or, for the
/// last, the one before) re-requested into D+0x2C90 / 0x2C94 (with 0x1D
/// (cleared) into D+0x2C8C).
fn renodes(fx: &mut Fx) {
    let sp = fx.enter(0x70);
    for k in 0..0x32u32 {
        let b = fx.mem.read_u8(NODE_TABLE + k);
        fx.mem.write_u8(sp + 0x30 + k, b);
    }
    for k in 0..0x10u32 {
        if fx.mem.i16(STRANDS + 0x30 * k + 0x28) != 0 {
            continue;
        }
        let r = fx.rand_from(k, 0x01E0_42E0);
        let (_, h) = fx.div(r, 0x32);
        fx.mem.set32(D + 0x2C84, h);
        let i = fx.mem.i32(D + 0x2C84);
        fx.mem.set32(D + 0x2C88, if i + 1 < 0x32 { i + 1 } else { i - 1 });
        let h = fx.self_handle();
        let r = fx.request_node(h, 0x1D, 1);
        fx.mem.set32(D + 0x2C8C, r);
        for (at, from) in [(0x2C90u32, 0x2C84u32), (0x2C94, 0x2C88)] {
            let h = fx.self_handle();
            let i = fx.mem.read_u32(D + from);
            let nd = fx.mem.u8((sp + 0x30).wrapping_add(i));
            let r = fx.request_node(h, nd, 0);
            fx.mem.set32(D + at, r);
        }
    }
    fx.leave(0x70);
}

/// 0x1E02750(g, rh2, _, len) [F] (1c6424378088, b021's column setup at
/// this record's address: its rand return addresses; `at` the function's
/// address, 0x1E02750 here, 0x1E02BF0 in b848).
pub(crate) fn column_setup(fx: &mut Fx, g: u32, rh2: u32, len: f32, at: u32) {
    let sp = fx.enter(0x40);
    let _ = sp;
    let step = ee::div(len, 5.0);
    fx.mem.write_u32(g, rh2);
    fx.mem.write_u32(g + 0x28, 0);
    fx.mem.write_u32(g + 0x20, 0);
    fx.mem.setf(g + 0x24, len);
    for k in 0..5u32 {
        let p = g + 0x10 * k;
        let (m, h, ras) = if k == 4 { (10, 5, [at + 0x54, at + 0x80]) } else { (30, 15, [at + 0xB4, at + 0xE0]) };
        let v = rand_f(fx, k, ras[0], m, h);
        fx.mem.setf(p + 0x30, v);
        let v = rand_f(fx, k, ras[1], m, h);
        fx.mem.setf(p + 0x38, v);
        let v = ee::sub(len, ee::mul(step, ee::from_i32(k as i32 + 1)));
        fx.mem.setf(p + 0x34, v);
    }
    fx.leave(0x40);
}
