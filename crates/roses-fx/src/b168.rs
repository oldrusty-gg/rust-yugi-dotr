//! BMODULE b168 (card 315), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03310, data area 0x1ED0000) [F] (helper C, 30 Oct
//! 2026): b263's layout with other timings: a quad burst (+0x8E0) at the
//! attacker's node 0x1D growing to 0.3 on updates 30..100 with its quads
//! wobbling; from update 90 a ribbon trail (+0x70) toward the target (1 /
//! 20 of the way per update, then 1 / 19 ...) and a disc (+0x380) until
//! update 209; at the target from update 106 a disc and quad burst growing
//! to 2, and from update 90 a rising sprite fountain (+0xD40, full size
//! from update 101); the frame quad (white, alpha 0x60) from update 101;
//! the stage light down to 0x20 until update 204. Returns 1 on update 96,
//! 6 on 106..203, 2 on 204, 8 after 220.
//!
//! Library: as b263 ([`crate::b263`]): `sprites` (0x1E00240), `disc`
//! (0x1E01B30), `quads_setup` / `quads_draw` (0x1E02090 / 0x1E021F0), the
//! ribbon (0x1E02800 / 0x1E02950), the frame quad (0x1E00120).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad corner template (rodata 0x1E03D80, 0x1E02090's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_3D80;

/// The entry 0x1E03310.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn disc_record(fx: &mut Fx, at: u32, alpha: u8, radius: u32) {
    fx.mem.write_u32(at, 0x8_0000);
    fx.mem.write_u8(at + 6, 0xD0);
    fx.mem.write_u8(at + 5, 0x70);
    fx.mem.write_u8(at + 4, 0xA0);
    fx.mem.write_u8(at + 7, alpha);
    fx.mem.write_u32(at + 8, radius);
    fx.mem.set16(at + 0xC, 0x24);
    fx.mem.set16(at + 0xE, 0);
}

/// A fountain sprite (`p` = D + 16 k, fields at +0xDA0) set off (as
/// b263's, radius from +0xD8C).
fn fountain_spawn(fx: &mut Fx, p: u32, ra: u32) {
    let r = fx.mem.f(D + 0xD8C);
    let f = ee::mul(10.0, r);
    fx.fix_residue(f, ra);
    let m = ee::fixsfsi(f);
    let a = fx.rand() & 0xFFFF;
    let x = fx.rand();
    let (_, h) = fx.div(x, m);
    let tenth = f32::from_bits(0x3DCC_CCCD);
    let radius = ee::mul(tenth, ee::from_i32(h));
    let x = fx.rand();
    let (_, h) = fx.div(x, 0x28);
    fx.mem.setf(p + 0xDA4, ee::mul(tenth, ee::neg(ee::from_i32(h - 0x14))));
    let c = fx.tcos(a);
    fx.mem.setf(p + 0xDA8, ee::mul(radius, c));
    let s = fx.tsin(a);
    fx.mem.setf(p + 0xDA0, ee::mul(radius, s));
    let x = fx.rand();
    let (_, h) = fx.div(x, 0xC);
    fx.mem.set16(p + 0xDAE, -h);
    fx.mem.write_u8(p + 0xDAC, 0x80);
}

fn quad_group(fx: &mut Fx, g: u32, flags: u32, tex: u32, size: f32) {
    fx.mem.write_u32(g + 4, flags);
    for (k, x) in [(0x10u32, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u8(g + 0xA, 0xD0);
    fx.mem.write_u8(g + 9, 0x70);
    fx.mem.write_u8(g + 8, 0xA0);
    fx.mem.write_u8(g + 0xB, 0x20);
    fx.quads_setup(g, tex, size, QUAD_TEMPLATE);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1DD0);
    for x in [0u32, 0x2A0, 0x870, 0x310, 0x390] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x1DC4u32, 0x1E, 1), (0x1DC8, 0x1D, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x1DA0, 0xDC);
    fx.mem.set32(D + 0x1DA4, 1);
    fx.mem.set32(D + 0x1DA8, param);
    fx.mem.set32(D + 0x1DB0, 0x14);
    fx.mem.set32(D + 0x1DAC, 0x80);
    disc_record(fx, D + 0x380, 0xFF, 0x4000_0000);
    disc_record(fx, D + 0x860, 0, 0x41F0_0000);
    fx.texture5(sp + 0xA0, 0);
    // The ribbon (+0x70): 32 slots, life 80, width 40, cell (16, 0, 16,
    // 32); points at +0xA0.
    let g = D + 0x70;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 8, 0x8003);
    for k in [6u32, 5, 4, 7] {
        fx.mem.write_u8(g + k, 0x80);
    }
    for (k, x) in [(0x10u32, 0x10), (0x12, 0), (0x14, 0x10), (0x16, 0x20), (0x18, 0x10), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u32(g + 0xC, 0x4220_0000);
    for (k, x) in [(0x22u32, 8), (0x24, 0), (0x26, 0x50), (0x28, 0x20), (0x2A, 0), (0x2C, 1)] {
        fx.mem.set16(g + k, x);
    }
    // The fountain (+0xD40): 256 sprites, size 0.3, radius 5.
    let g = D + 0xD40;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0xC, 0x3E99_999A);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    for (k, x) in [(0x30u32, 0x20), (0x32, 0x20), (0x34, 0x20), (0x36, 0x20), (0x38, 0x20), (0x3A, 0x20), (0x3C, 0x80), (0x3E, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u32(g + 0x40, D + 0xDA0);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x40A0_0000);
    fx.mem.set16(g + 0x5A, 0x100);
    fx.mem.set16(g + 0x5C, 1);
    fx.mem.set16(g + 0x5E, 0);
    let mut k = 0;
    while k < fx.mem.i16(D + 0xD9A) {
        fountain_spawn(fx, D + 16 * k as u32, 0x01E0_3560);
        k += 1;
    }
    fx.texture4(sp + 0xA0, 0xF);
    let tex = fx.mem.read_u32(sp + 0xA0);
    quad_group(fx, D + 0x400, 0x8_0001, tex, 4.0);
    let tex = fx.mem.read_u32(sp + 0xA0);
    quad_group(fx, D + 0x8E0, 1, tex, f32::from_bits(0x3F33_3333));
    for k in 0..64u32 {
        let q = D + 16 * k;
        fx.mem.write_u32(q + 0x940, 0x4000);
        let r = fx.rand();
        fx.mem.set32(q + 0x944, r & 0xFFFF);
        fx.mem.write_u32(q + 0x948, 0);
    }
    for (x, v) in [(0x390u32, 0x3DCC_CCCD), (0x870, 0x3C23_D70A)] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, v);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    node(fx, 0x1DC4, sp + 0x80);
    node(fx, 0x1DC8, sp + 0x90);
    fx.look_angles(sp + 0x80, sp + 0x90, D + 0x880);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x1DB4);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x3B0);
    node(fx, 0x1DC8, D + 0x890);
    node(fx, 0x1DC8, D + 0x330);
    if fx.mem.i32(D + 0x1DA4) < 0x5B {
        node(fx, 0x1DC8, D + 0x2C0);
    }
    if fx.mem.i32(D + 0x1DA4) >= 0x5A {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x2C0);
        let n = fx.mem.i32(D + 0x1DB0);
        fx.approach_by(D + 0x2C0, D + 0x1DB4, n);
        let n = fx.mem.i32(D + 0x1DB0);
        if n != 0 {
            fx.mem.set32(D + 0x1DB0, n - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x90.
    let regs = [Some(D), Some(OT), Some(sp + 0x90), None];
    fx.xform_world_saving(D, 0x01E0_3888, regs);
    fx.xform_billboard_saving(D + 0x2A0, 0x01E0_3890, regs);
    fx.xform_world_saving(D + 0x870, 0x01E0_3898, regs);
    fx.xform_billboard_saving(D + 0x310, 0x01E0_38A0, regs);
    fx.xform_billboard_saving(D + 0x390, 0x01E0_38A8, regs);
    if fx.mem.i32(D + 0x1DA4) >= 0x65 {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if fx.mem.i32(D + 0x1DA4) < 0xCC {
        fx.step_down(D + 0x1DAC, 0x20, 4);
    } else {
        fx.step_up(D + 0x1DAC, 0x80, 4);
    }
    let light = fx.mem.u8(D + 0x1DAC);
    fx.set_stage_light(light);
    let n = fx.mem.i32(D + 0x1DA4);
    if (0x1E..0x65).contains(&n) {
        if fx.approach(D + 0x870, f32::from_bits(0x3E99_999A), f32::from_bits(0x3CA3_D70A)) == 0 {
            fx.step_down(D + 0x8EB, 0, 1);
        }
        fx.quads_draw(D + 0x8E0, D + 0x870);
        for k in 0..64u32 {
            let a = D + 0x944 + 16 * k;
            let v = fx.mem.i32(a);
            fx.mem.set32(a, if k % 2 == 0 { v.wrapping_add(0x40) } else { v.wrapping_sub(0x40) });
        }
    }
    if fx.mem.i32(D + 0x1DA4) >= 0x6A {
        fx.approach(D + 0x390, 2.0, f32::from_bits(0x3D4C_CCCD));
        fx.disc(D + 0x860, D + 0x390);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_39F4)]);
        fx.quads_draw(D + 0x400, D + 0x390);
        if fx.mem.i32(D + 0x1DA4) >= 0xBE {
            fx.step_down(D + 0x867, 0, 0x10);
            fx.step_down(D + 0x40B, 0, 2);
        } else {
            fx.step_up(D + 0x867, 0xFF, 8);
        }
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0x460 + 16 * k + 4 * c;
                let v = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, v);
            }
        }
    }
    let n = fx.mem.i32(D + 0x1DA4);
    if (0x5A..0xD2).contains(&n) {
        fx.disc(D + 0x380, D + 0x310);
        // The disc's saved fp / ra (frame 0x140) under the sprite group's
        // TEX0 / TEX1 slots.
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_3AB4)]);
    }
    let n = fx.mem.i32(D + 0x1DA4);
    if n >= 0x5A {
        if n >= 0x65 {
            fx.mem.write_u32(D + 0xD4C, 0x3F80_0000);
            for k in [8u32, 4, 0] {
                fx.mem.write_u32(D + 0x870 + k, 0x3F80_0000);
            }
        }
        fx.sprites(D + 0xD40, D + 0x870);
        let grow = f32::from_bits(0x3F8C_CCCD);
        let mut k = 0;
        while k < fx.mem.i16(D + 0xD9A) {
            let p = D + 16 * k as u32;
            if fx.mem.i16(p + 0xDAE) >= 0 {
                let y = ee::add(fx.mem.f(p + 0xDA4), 2.0);
                fx.mem.setf(p + 0xDA4, y);
                let x = ee::mul(fx.mem.f(p + 0xDA0), grow);
                fx.mem.setf(p + 0xDA0, x);
                let z = ee::mul(fx.mem.f(p + 0xDA8), grow);
                fx.mem.setf(p + 0xDA8, z);
                fx.step_down(p + 0xDAC, 0, 8);
                if !ee_le(fx.mem.f(p + 0xDA4), 20.0) {
                    fountain_spawn(fx, p, 0x01E0_3B88);
                }
            }
            k += 1;
        }
    }
    if fx.mem.i32(D + 0x1DA4) >= 0x5B {
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
    }
    let n = fx.mem.i32(D + 0x1DA4) + 1;
    fx.mem.set32(D + 0x1DA4, n);
    let left = fx.mem.i32(D + 0x1DA0) - 1;
    fx.mem.set32(D + 0x1DA0, left);
    if n == 0xCC {
        2
    } else if n == 0x60 {
        1
    } else if (0x6A..0xCC).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
