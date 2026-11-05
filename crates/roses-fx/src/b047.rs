//! BMODULE b047 (card 296), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01980, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): b600's family with three tubes. Until update 100 the
//! attacker's object op 0x1F83B0 (a boundary call); from update 58 the
//! frame quad and three tubes (+0x1C0 + 0x160 k, [`crate::tube`]: 32
//! points, radius 1 growing 0.1 per ring, then 0.5 from update 100) from
//! the attacker's nodes 8 / 9 / 10 aimed at nodes 3 / 4 / 5 (pulled 25
//! down from update 100), their red bytes flickering; at the target (its
//! animated position) a spark burst (+0x1250, `blib::trail::sparks`) lit
//! from update 82; the stage light down to 0x20 until update 170. Returns
//! 1 on update 72, 6 on 82..169, 2 on 170, 8 after 189.
//!
//! Library: `colours4` (0x1E00080), the frame quad (0x1E00120,
//! [`crate::framegs`]), `spawn::scatter` (0x1E00240), `trail::sparks`
//! (0x1E00400) [`crate::blib`], the tube (0x1E01160 / 0x1E01260).
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E01980.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x22B0);
    fx.xform_init(D);
    for k in 0..3u32 {
        fx.xform_init(D + 0x70 + 0x70 * k);
    }
    fx.xform_init(D + 0x11E0);
    for (at, node, clear) in [(0x228Cu32, 8, 1), (0x2290, 9, 0), (0x2294, 0xA, 0), (0x2298, 3, 0), (0x229C, 4, 0), (0x22A0, 5, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2280, 0xBE);
    fx.mem.set32(D + 0x2284, 1);
    fx.mem.set32(D + 0x2288, param);
    fx.mem.set32(D + 0x22A4, 0x80);
    fx.texture5(sp + 0xB0, 0);
    for k in 0..3u32 {
        fx.mem.write_u32(D + 0x308 + 0x160 * k, D + 0x5E0 + 0x400 * k);
    }
    // The tubes (+0x1C0 + 0x160 k): 32 points, radius 1 (+0.1 per ring),
    // 20 records, rings 5 apart, cell (0, 0, 128, 128).
    for k in 0..3u32 {
        let g = D + 0x1C0 + 0x160 * k;
        let tex = fx.mem.read_u32(sp + 0xB0);
        fx.mem.write_u32(g, tex);
        for o in [6u32, 5, 4, 7] {
            fx.mem.write_u8(g + o, 0x80);
        }
        fx.mem.write_u32(g + 8, 0x8000);
        fx.mem.write_u32(g + 0xC, 0x3F80_0000);
        halves(fx, g, &[(0x140, 0x20), (0x142, 0)]);
        fx.mem.write_u32(g + 0x144, 0x40A0_0000);
        halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x80), (0x16, 0x80), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x80)]);
        halves(fx, g, &[(0x14C, 0x14), (0x14E, 0)]);
        fx.mem.write_u32(g + 0x150, 0x3DCC_CCCD);
        let tex = fx.mem.read_u32(sp + 0xB0);
        fx.tube_setup(g, tex);
    }
    fx.texture4(sp + 0xB0, 0xF);
    // The sparks (+0x1250): 256 at +0x1280, width 0.3, length 11.
    let g = D + 0x1250;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 3);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x70), (8, 0x40), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0x20, 0x3E99_999A);
    fx.mem.write_u32(g + 0x24, 0x4130_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x60), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x60), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, D + 0x1280);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2284);
    let node = |fx: &mut Fx, slot: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + slot);
        fx.node_position(h, s, out);
    };
    for k in 0..3u32 {
        let x = D + 0x70 * k;
        node(fx, 0x228C + 4 * k, sp + 0x80);
        node(fx, 0x2298 + 4 * k, sp + 0x90);
        fx.look_angles(sp + 0x80, sp + 0x90, x + 0x80);
        node(fx, 0x228C + 4 * k, x + 0x90);
        if n(fx) >= 0x64 {
            fx.mem.write_u32(x + 0x98, 0);
            let y = ee::sub(fx.mem.f(x + 0x94), 25.0);
            fx.mem.setf(x + 0x94, y);
            fx.mem.write_u32(D + 0x310 + 0x160 * k, 0x3F00_0000);
        }
    }
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x1200);
    // s0 = D, s1 = the OT, s2 / s3 the loops'.
    fx.xform_world_saving(D, 0x01E0_1CF8, [Some(D), Some(OT), Some(D + 0xC), Some(D + 0x150)]);
    for k in 0..3u32 {
        fx.xform_world_saving(D + 0x70 + 0x70 * k, 0x01E0_1D08, [Some(D), Some(OT), Some(D + 0x70 * k), Some(k)]);
    }
    fx.xform_world_saving(D + 0x11E0, 0x01E0_1D24, [Some(D), Some(OT), Some(D + 0x150), Some(3)]);
    if n(fx) < 0xAA {
        fx.step_down(D + 0x22A4, 0x20, 4);
    } else {
        fx.step_up(D + 0x22A4, 0x80, 4);
    }
    let light = fx.mem.u8(D + 0x22A4);
    fx.set_stage_light(light);
    if n(fx) >= 0x3A {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0x64 {
        let h = fx.self_handle();
        for k in [0xBCu32, 0xBD, 0xBE, 0xBB, 0xB8, 0xB9, 0xBA] {
            fx.mem.write_u8(sp + k, 0x80);
        }
        fx.mem.write_u8(sp + 0xBF, 0);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h as u32, 3, 0, sp + 0xB8]);
    }
    if n(fx) >= 0x3A {
        for k in 0..3u32 {
            let g = D + 0x1C0 + 0x160 * k;
            let r = fx.rand_from(D, 0x01E0_1DFC);
            fx.mem.set8(g + 4, r % 64 + 0x40);
            fx.tube_draw(OT, g, D + 0x70 + 0x70 * k);
            if n(fx) >= 0xAB {
                fx.step_down(g + 7, 0, 8);
            }
            let c = fx.mem.i16(g + 0x14E) + 1;
            fx.mem.set16(g + 0x14E, c);
        }
    }
    if n(fx) == 0x52 {
        fx.mem.write_u8(D + 0x125B, 0x80);
    }
    blib::trail::sparks(fx, OT, D + 0x1250, D + 0x11E0);
    if n(fx) >= 0xAB {
        fx.step_down(D + 0x125B, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x127C) {
        let p = D + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x128E) < 0 {
            continue;
        }
        let s = f32::from_bits(0x3FA6_6666);
        for c in 0..3u32 {
            let x = ee::mul(fx.mem.f(p + 0x1280 + 4 * c), s);
            fx.mem.setf(p + 0x1280 + 4 * c, x);
        }
        if fx.mem.i16(p + 0x128E) < 0x11 {
            continue;
        }
        fx.random_dir(sp + 0xA0, 1.0);
        for c in [1u32, 2, 0] {
            let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
            fx.mem.write_u32(p + 0x1280 + 4 * c, w);
        }
        let r = fx.rand_from(D, 0x01E0_1F38);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0x128E, h.wrapping_neg());
        let a = if n(fx) >= 0x52 { 0x80 } else { 0 };
        fx.mem.write_u8(p + 0x128C, a);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2284, m);
    let left = fx.mem.i32(D + 0x2280) - 1;
    fx.mem.set32(D + 0x2280, left);
    if m == 0xAA {
        2
    } else if m == 0x48 {
        1
    } else if (0x52..0xAA).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
