//! BMODULE b545 (card 471), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01620, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): b489's family. A wide white spark column (+0xE0, radius up
//! to 20, alternate sparks rising 10 / 7 a step) round the attacker's
//! node 0xF; from update 160 a disc (+0x42C0), the frame quad, a second
//! spark burst (+0x1180, 512 sparks) thrown out again as each dies, and
//! a third spark fountain (+0x3220) on the target respawned on a ring
//! when above 30; the stage light down to 0x20; on updates 182..265 the
//! target's added colour (0x1F7790) flickering at random. Returns 1 on
//! update 171, 6 on 181..265, 2 on 266, 8 after 285.
//!
//! Library: the frame quad (0x1E00080, [`crate::framegs`]),
//! `spawn::scatter` (0x1E001A0), `trail::sparks` (0x1E00360)
//! [`crate::blib`], `disc` (0x1E010C0) [`crate::fxlib`].
use crate::b057::spark_place_r;
use crate::b447::sparks_group;
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const TENTH: u32 = 0x3DCC_CCCD;

/// The entry 0x1E01620.
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
    fx.bzero(D, 0x4300);
    for x in [0u32, 0x4250, 0x70, 0x1110, 0x31B0] {
        fx.xform_init(D + x);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0xF, 1);
    fx.mem.set32(D + 0x42F0, r);
    fx.mem.set32(D + 0x42E0, 0x11E);
    fx.mem.set32(D + 0x42E4, 1);
    fx.mem.set32(D + 0x42E8, param);
    for k in [0x42F4u32, 0x42EF, 0x42EC, 0x42ED, 0x42EE] {
        fx.mem.write_u8(D + k, 0x80);
    }
    // The disc (+0x42C0), radius 30.
    let g = D + 0x42C0;
    fx.mem.write_u32(g, 0x8_0000);
    for (k, x) in [(6u32, 0x40u8), (5, 0x40), (4, 0x80), (7, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 8, 0x41F0_0000);
    fx.mem.set16(g + 0xC, 0x24);
    fx.mem.set16(g + 0xE, 0);
    fx.texture5(sp + 0xB0, 0);
    let tex = fx.mem.read_u32(sp + 0xB0);
    sparks_group(fx, D + 0xE0, tex, 0x6000_0003, 0, 0x3F00_0000, 0x41B0_0000);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x10C) {
        spark_place_r(fx, k as u32, 0xC8, 0x7D0, [0x01E0_17BC, 0x01E0_17D8, 0x01E0_1804]);
        let e = fx.mem.read_u32(D + 0xEC).wrapping_add(16 * k as u32);
        fx.mem.write_u8(e + 0xC, 0);
        k += 1;
    }
    // The second burst (+0x1180): 512 sparks at +0x11B0, width 0.4.
    let g = D + 0x1180;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 3);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0x20, 0x3ECC_CCCD);
    fx.mem.write_u32(g + 0x24, 0x4120_0000);
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x20), (0x16, 0x20), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, D + 0x11B0);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x200);
    fx.mem.write_u8(g + 0x2E, 8);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    // The fountain (+0x3220): 256 sparks at +0x3250, width 0.5, length 3.
    let g = D + 0x3220;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x2000_0003);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, 0xFF)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x4040_0000);
    halves(fx, g, &[(0x10, 0), (0x12, 0x20), (0x14, 0x20), (0x16, 0x20), (0x18, 0), (0x1A, 0x20), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, D + 0x3250);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x42E4);
    let node = |fx: &mut Fx, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x42F0);
        fx.node_position(h, s, out);
    };
    let target = |fx: &mut Fx, out: u32| {
        let t = fx.target_handle();
        fx.unit_position(t, out);
    };
    target(fx, D + 0x42D0);
    node(fx, D + 0x90);
    target(fx, D + 0x31D0);
    target(fx, sp + 0x90);
    node(fx, sp + 0xA0);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x80);
    fx.look_angles(sp + 0xA0, sp + 0x90, D + 0x31C0);
    node(fx, D + 0x1130);
    node(fx, D + 0x4270);
    // s0 = D, s1 = the OT, s2 = sp + 0xA0, s3 the caller's.
    let regs = [Some(D), Some(OT), Some(sp + 0xA0), None];
    fx.xform_world_saving(D, 0x01E0_1A8C, regs);
    fx.xform_billboard_saving(D + 0x4250, 0x01E0_1A94, regs);
    fx.xform_world_saving(D + 0x70, 0x01E0_1A9C, regs);
    fx.xform_world_saving(D + 0x1110, 0x01E0_1AA4, regs);
    fx.xform_world_saving(D + 0x31B0, 0x01E0_1AAC, regs);
    if n(fx) >= 0x10A {
        fx.step_up(D + 0x42F4, 0x80, 4);
    } else {
        fx.step_down(D + 0x42F4, 0x20, 4);
    }
    let light = fx.mem.u8(D + 0x42F4);
    fx.set_stage_light(light);
    if n(fx) >= 0xA0 {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if (0xB6..0x10A).contains(&n(fx)) {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, D + 0x42EC, [D, OT], 0x01E0_1B38);
        for (at, ra) in [(0x42EEu32, 0x01E0_1B40), (0x42ED, 0x01E0_1B5C), (0x42EC, 0x01E0_1B78)] {
            let r = fx.rand_from(D, ra);
            fx.mem.set8(D + at, r % 128);
        }
    }
    if n(fx) == 0x10A {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, 0, [D, OT], 0x01E0_1BB4);
    }
    if n(fx) >= 0xA0 {
        fx.disc(D + 0x42C0, D + 0x4250);
        // The disc's fp / ra saves, under later descriptors' TEX slots.
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_1BD4)]);
        if n(fx) >= 0x10A {
            fx.step_down(D + 0x42C7, 0, 0x10);
        } else {
            fx.step_up(D + 0x42C7, 0xFF, 0x10);
        }
    }
    blib::trail::sparks(fx, OT, D + 0xE0, D + 0x70);
    let m = n(fx);
    if m >= 0x10B {
        fx.step_down(D + 0xEB, 0, 8);
    } else if (0xA0..0x10A).contains(&m) {
        fx.step_up(D + 0xEB, 0x80, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x10C) {
        let p = D + 16 * k as u32;
        if fx.mem.i16(p + 0x11E) >= 0 {
            let rise = if k % 2 == 0 { 10.0 } else { 7.0 };
            let y = ee::add(fx.mem.f(p + 0x114), rise);
            fx.mem.setf(p + 0x114, y);
            fx.step_up(p + 0x11C, 0x80, 0x10);
            if !ee_le(fx.mem.f(p + 0x114), 200.0) {
                spark_place_r(fx, k as u32, 0xC8, 0x1E, [0x01E0_1D10, 0x01E0_1D2C, 0x01E0_1D58]);
                fx.mem.write_u8(p + 0x11C, 0);
                fx.mem.set16(p + 0x11E, 0);
            }
        }
        k += 1;
    }
    blib::trail::sparks(fx, OT, D + 0x1180, D + 0x1110);
    let m = n(fx);
    if m >= 0x10B {
        fx.step_down(D + 0x118B, 0, 8);
    } else if (0xA0..0x10A).contains(&m) {
        fx.step_up(D + 0x118B, 0x80, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x11AC) {
        let p = D + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x11BE) < 0 {
            continue;
        }
        if n(fx) >= 0xA0 {
            fx.step_up(p + 0x11BC, 0x80, 0x10);
        }
        let s = f32::from_bits(0x3FA6_6666);
        for c in 0..3u32 {
            let x = ee::mul(fx.mem.f(p + 0x11B0 + 4 * c), s);
            fx.mem.setf(p + 0x11B0 + 4 * c, x);
        }
        if fx.mem.i16(p + 0x11BE) < 0x11 {
            continue;
        }
        fx.random_dir(sp + 0x80, 1.0);
        for c in [1u32, 2, 0] {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * c);
            fx.mem.write_u32(p + 0x11B0 + 4 * c, w);
        }
        let r = fx.rand_from(D, 0x01E0_1EF0);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0x11BE, h.wrapping_neg());
        fx.mem.write_u8(p + 0x11BC, 0);
    }
    blib::trail::sparks(fx, OT, D + 0x3220, D + 0x31B0);
    if n(fx) >= 0x10B {
        fx.step_down(D + 0x322B, 0, 0x10);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x324C) {
        let p = D + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x325E) < 0 {
            continue;
        }
        let s = f32::from_bits(0x3FA6_6666);
        for c in 0..3u32 {
            let x = ee::mul(fx.mem.f(p + 0x3250 + 4 * c), s);
            fx.mem.setf(p + 0x3250 + 4 * c, x);
        }
        if ee_le(fx.mem.f(p + 0x3254), 30.0) {
            continue;
        }
        // Respawned on a flat ring (as `spawn::scatter` with flag 0x1D).
        let f = ee::mul(10.0, fx.mem.f(D + 0x3248));
        fx.fix_residue(f, 0x01E0_1FD4);
        let w = ee::fixsfsi(f);
        let a = fx.rand_from(D, 0x01E0_1FDC) & 0xFFFF;
        let r = fx.rand_from(D, 0x01E0_1FF8);
        let (_, h) = fx.div(r, w);
        fx.mem.write_u32(p + 0x3254, 0x4080_0000);
        let radius = ee::mul(f32::from_bits(TENTH), ee::from_i32(h));
        let c = fx.tcos(a);
        fx.mem.setf(p + 0x3258, ee::mul(radius, c));
        let s = fx.tsin(a);
        fx.mem.setf(p + 0x3250, ee::mul(radius, s));
        let r = fx.rand_from(D, 0x01E0_2048);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0x325E, h.wrapping_neg());
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x42E4, m);
    let left = fx.mem.i32(D + 0x42E0) - 1;
    fx.mem.set32(D + 0x42E0, left);
    if m == 0x10A {
        2
    } else if m == 0xAB {
        1
    } else if (0xB5..0x10A).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
