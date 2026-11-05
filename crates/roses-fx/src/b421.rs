//! BMODULE b421 (card 497), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03420, data area a0 = 0x1ED0000) [F] (helper B9, 31
//! Oct 2026). Sixteen jagged strands (+0x70, 0x30 apart) between two
//! distinct random nodes of the attacker's 0x33..0x38 (ends +0x14F0 /
//! +0x1500, 0x20 apart, relative to node 0x12 +0x13A0) re-aimed whenever
//! their counter runs out, and a point-set strip (+0x1490 / +0x13F0) at
//! the target from update 41 (transform +0x1380 growing to 1 by 0.5, then
//! fading); from 115 a disc (+0x3E0) and until 134 a ring (+0x460) and a
//! cloud of 60 squares (+0x8D0) spreading by 1.12 (transforms +0x370 /
//! +0x3F0 / +0x860 at the target growing to 4 by 0.3); from 135 a quad
//! burst (+0xF20) and a second cloud (+0xBC0). Returns 1 on update 105, 2
//! on 135, 6 on 115..134, 8 after 170.
//!
//! Library (record addresses): `ring_setup` / `ring_draw` 0x1E00120 /
//! 0x1E003E0, `pointquad::squares` / `setup` / `draw` 0x1E008D0 /
//! 0x1E00EB0 / 0x1E011F0, `disc` 0x1E01DD0, `quads_setup` / `quads_draw`
//! 0x1E02330 / 0x1E02490, `strand::draw` 0x1E02AA0.
use crate::b028::square_points;
use crate::b079::sub3;
use crate::blib::{pointquad, strand};
use crate::bus::Bus;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The launch's printf format (record rodata).
const FORMAT: u32 = 0x01E0_4080;
/// The quad corner template (rodata, 0x1E02330's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_4000;

/// The entry 0x1E03420 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xA0);
    ret
}

/// A cloud of 60 squares `g`: half size 3, flags 0x2000_0001, cell `cell`.
fn squares_init(fx: &mut Fx, g: u32, tex: u32, cell: [u16; 4]) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x2000_0001);
    fx.mem.write_u32(g + 0xC, 0x4040_0000);
    square_points(fx, g);
    for (k, v) in cell.into_iter().enumerate() {
        fx.mem.write_u16(g + 0x10 + 2 * k as u32, v);
    }
    for (o, v) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, 0xFF)] {
        fx.mem.write_u8(g + o, v);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.boundary(0x0019_A410, "printf", [FORMAT, 0x1720, 0, 0]);
    fx.bzero(D, 0x1720);
    for x in [0u32, 0x370, 0xEB0, 0x3F0] {
        fx.xform_init(D + x);
    }
    for o in [0x3F8u32, 0x3F4, 0x3F0] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    fx.xform_init(D + 0x860);
    for o in [0x868u32, 0x864, 0x860] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    fx.xform_init(D + 0x1380);
    for (at, node, clear) in [(0x16FCu32, 0x12, 1), (0x1700, 1, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    for k in 0..6u32 {
        let h = fx.self_handle();
        let r = fx.request_node(h, 0x33 + k as i32, 0);
        fx.mem.set32(D + 0x1704 + 4 * k, r);
    }
    fx.mem.set32(D + 0x16F0, 0xAA);
    fx.mem.set32(D + 0x16F4, 1);
    fx.mem.set32(D + 0x16F8, param);
    fx.mem.write_u8(D + 0x171C, 0x80);
    // The disc (+0x3E0): radius 20, 0x24 triangles.
    let g = D + 0x3E0;
    fx.mem.write_u32(g, 0x8_0000);
    for (o, v) in [(6u32, 0x70u8), (5, 0xB0), (4, 0xE0), (7, 0xFF)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 8, 0x41A0_0000);
    fx.mem.write_u16(g + 0xC, 0x24);
    fx.mem.write_u16(g + 0xE, 0);
    // Sixteen strands (+0x70, 0x30 apart), texture 5 / 0.
    fx.texture5(sp + 0x90, 0);
    for k in 0..16u32 {
        let g = D + 0x70 + 0x30 * k;
        let t = fx.mem.read_u32(sp + 0x90);
        fx.mem.write_u32(g, t);
        fx.mem.write_u32(g + 4, 0);
        for (o, v) in [(0x10u32, 0x80u16), (0x12, 0x40), (0x14, 0x20), (0x16, 0x40)] {
            fx.mem.write_u16(g + o, v);
        }
        for o in [0xAu32, 9, 8] {
            fx.mem.write_u8(g + o, 0x80);
        }
        let r = fx.rand();
        fx.mem.set8(g + 0xB, r % 0x80);
        fx.mem.write_u16(g + 0xC, 0x10);
        fx.mem.set16(g + 0x28, -(k as i32 + 1));
        fx.mem.write_u16(g + 0xE, 5);
        fx.mem.write_u32(g + 0x20, 0);
        fx.mem.write_u32(g + 0x24, 0);
    }
    // The ring (+0x460): radii 10 / 14.
    let g = D + 0x460;
    fx.mem.write_u32(g + 4, 0x2000_0000);
    for (o, v) in [(0x10u32, 0xC0u16), (0x12, 0x40), (0x14, 0x20), (0x16, 0x40)] {
        fx.mem.write_u16(g + o, v);
    }
    for o in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + o, 0x80);
    }
    let t = fx.mem.read_u32(sp + 0x90);
    fx.ring_setup(g, t, 10.0, 14.0, 0.0, 0.0);
    // The quad burst (+0xF20): size 1.
    let g = D + 0xF20;
    fx.mem.write_u32(g + 4, 1);
    for (o, v) in [(0x10u32, 0x9Eu16), (0x12, 0), (0x14, 0x40), (0x16, 0x40)] {
        fx.mem.write_u16(g + o, v);
    }
    for o in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + o, 0x80);
    }
    let t = fx.mem.read_u32(sp + 0x90);
    fx.quads_setup(g, t, 1.0, QUAD_TEMPLATE);
    // Two clouds of 60 squares (+0x8D0, +0xBC0).
    let t = fx.mem.read_u32(sp + 0x90);
    squares_init(fx, D + 0x8D0, t, [0xE0, 0x40, 0x20, 0x20]);
    let t = fx.mem.read_u32(sp + 0x90);
    squares_init(fx, D + 0xBC0, t, [0xE0, 0, 0x20, 0x20]);
    // The point set (+0x13F0) and its look (+0x1490), texture 4 / 0xF.
    fx.texture4(sp + 0x90, 0xF);
    fx.mem.write_u32(D + 0x14C8, 0x8_0001);
    let g = D + 0x1490;
    for (o, v) in [(0u32, 0xB0u16), (2, 0x20), (4, 0x20), (6, 0x20), (8, 0xB0), (0xA, 0x20), (0xC, 0x100), (0xE, 0x100)] {
        fx.mem.write_u16(g + o, v);
    }
    fx.mem.write_u32(g + 0x30, 0x3F80_0000);
    for (o, v) in [(0x36u32, 0x70u8), (0x35, 0xB0), (0x34, 0xE0), (0x37, 0x40)] {
        fx.mem.write_u8(g + o, v);
    }
    for (o, v) in [(0x48u32, 0u16), (0x4A, 0), (0x4C, 0), (0x4E, 1), (0x50, 0), (0x52, 0x3E7)] {
        fx.mem.write_u16(g + o, v);
    }
    fx.mem.write_u32(D + 0x13F8, 1);
    let t = fx.mem.read_u32(sp + 0x90);
    pointquad::setup(fx, 0x01E0_0EB0, D + 0x1490, D + 0x13F0, t, 8.0);
    for o in [0x1388u32, 0x1384, 0x1380] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    0
}

/// rand % 6.
fn rand6(fx: &mut Fx) -> i32 {
    let r = fx.rand();
    fx.div(r, 6).1
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x16F4);
    for out in [0x390u32, 0xED0, 0x410, 0x880] {
        let t = fx.target_handle();
        fx.unit_position(t, D + out);
    }
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x1704);
    fx.node_position(h, s, D + 0x13A0);
    let (mut s2, mut s3) = (u64::MAX, 0u64);
    for k in 0..16u32 {
        if n(fx) < 0x29 || fx.mem.i16(D + 0x98 + 0x30 * k) != 0 {
            continue;
        }
        let a = rand6(fx);
        let r = fx.rand();
        let mut b = fx.div(r - 5, 6).1;
        while a == b {
            b = rand6(fx);
        }
        (s2, s3) = (a as u64, b as u64);
        let e = D + 0x14F0 + 0x20 * k;
        for (i, out) in [(a, e), (b, e + 0x10)] {
            let h = fx.self_handle();
            let s = fx.mem.i32(D + 0x1704 + 4 * i as u32);
            fx.node_position(h, s, out);
        }
        sub3(fx, e, D + 0x13A0);
        sub3(fx, e + 0x10, D + 0x13A0);
    }
    // The transforms' frames (the last one's TRS matrix is the strands'
    // missing-texture TEX0 / TEX1 [run]): s0 = D, s1 = the OT, s2 / s3 the
    // last re-aimed strand's nodes, else the parameter (-1) and the
    // runner's 0.
    let regs = [Some(D), Some(OT), None, None];
    for (x, ra, bill) in [(0u32, 0x01E0_3C0C, false), (0x370, 0x01E0_3C14, true), (0xEB0, 0x01E0_3C1C, false), (0x3F0, 0x01E0_3C24, false), (0x860, 0x01E0_3C2C, true), (0x1380, 0x01E0_3C34, false)] {
        if bill {
            fx.xform_billboard_saving(D + x, ra, regs);
            fx.saved(sp - 0xF0, &[(0x40, s2), (0x50, s3)]);
        } else {
            fx.xform_world_saving(D + x, ra, regs);
            fx.saved(sp - 0xD0, &[(0x40, s2), (0x50, s3)]);
        }
    }
    if n(fx) >= 0x29 {
        for k in 0..16u32 {
            let e = D + 0x14F0 + 0x20 * k;
            strand::draw(fx, 0x01E0_2AA0, OT, D + 0x70 + 0x30 * k, D + 0x1380, e, e + 0x10);
        }
        pointquad::draw(fx, OT, D + 0x1490, D + 0x13F0, D + 0x1380);
        if !ee_lt(fx.mem.f(D + 0x1380), 1.0) {
            fx.step_down(D + 0x14C7, 0, 4);
        }
        fx.approach(D + 0x1380, 1.0, 0.5);
    }
    if n(fx) == 0x87 {
        fx.mem.write_u8(D + 0x3E7, 0xFF);
    }
    let grow = f32::from_bits(0x3E99_999A);
    if n(fx) >= 0x73 {
        fx.disc(D + 0x3E0, D + 0x370);
        fx.step_down(D + 0x3E7, 0, 8);
        if n(fx) < 0x87 {
            fx.ring_draw(D + 0x460, D + 0x3F0);
            fx.step_down(D + 0x46B, 0, 8);
            for o in [0x88u32, 0x84, 0x80] {
                fx.mem.write_u32(sp + o, 0x3F8F_5C29);
            }
            pointquad::squares(fx, OT, D + 0x8D0, D + 0x860, sp + 0x80);
            fx.step_down(D + 0x8DB, 0, 8);
            fx.approach(D + 0x3F0, 4.0, grow);
        }
    }
    if n(fx) >= 0x87 {
        fx.quads_draw(D + 0xF20, D + 0xEB0);
        fx.step_down(D + 0xF2B, 0, 8);
        for o in [0x88u32, 0x84, 0x80] {
            fx.mem.write_u32(sp + o, 0x3F8F_5C29);
        }
        pointquad::squares(fx, OT, D + 0xBC0, D + 0x860, sp + 0x80);
        fx.step_down(D + 0xBCB, 0, 8);
        fx.approach(D + 0x370, 4.0, grow);
        fx.approach(D + 0xEB0, 4.0, grow);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x16F4, m);
    let left = fx.mem.i32(D + 0x16F0) - 1;
    fx.mem.set32(D + 0x16F0, left);
    if m == 0x87 {
        2
    } else if m == 0x69 {
        1
    } else if (0x73..0x87).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
