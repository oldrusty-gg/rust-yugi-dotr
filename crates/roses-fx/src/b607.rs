//! BMODULE b607 (card 389), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E031D0, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): a wobbling quad burst (+0xE0) at the attacker's node 3 and
//! the frame quad (colour +0x1CC0, fading after 223) all along; a sprite
//! column (+0x1650, 34 sprites rising 5 a step to 160, turned by
//! +0x18D0) aimed from node 3 at the target, lit from update 190; a
//! pulse of five rings of sprites (+0x1B60) at the target, lit from
//! update 222; from update 222 a spark burst (+0x5B0,
//! `blib::trail::sparks`) at the target thrown out again as each spark
//! dies; the stage light down to 0x20 until update 365. Returns 1 on
//! update 196, 6 on 222..363, 2 on 364, 8 after 379.
//!
//! Library: the frame quad (0x1E00120, [`crate::framegs`]), `sprites`
//! (0x1E00250), `spawn::scatter` (0x1E01B40), `trail::sparks`
//! (0x1E01D00) [`crate::blib`], `quads_setup` / `quads_draw` (0x1E02A60
//! / 0x1E02BC0) [`crate::fxlib`].
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad corner template (rodata, 0x1E02A60's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_3C80;
const TENTH: u32 = 0x3DCC_CCCD;

/// The entry 0x1E031D0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x90);
    ret
}

fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

/// A sprite group at `g` (cell (0, 0, 64, 64), white, alpha 0).
fn sprite_group(fx: &mut Fx, g: u32, tex: u32, flags: u32, size: u32) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, flags);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0xC, size);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0), (0x34, 0x40), (0x36, 0x40), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)]);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1CE0);
    for x in [0u32, 0x70, 0x15E0, 0x1AF0, 0x540] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x1CD0u32, 3, 1), (0x1CD4, 1, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x1CC4, 0x17C);
    fx.mem.set32(D + 0x1CC8, 1);
    fx.mem.set32(D + 0x1CCC, param);
    for k in [0x1CD8u32, 0x1CC2, 0x1CC1, 0x1CC0] {
        fx.mem.write_u8(D + k, 0x80);
    }
    fx.mem.write_u8(D + 0x1CC3, 0x60);
    fx.texture5(sp + 0x80, 0);
    // The column (+0x1650): 34 sprites at +0x16B0, angles +0x18D0.
    let g = D + 0x1650;
    let tex = fx.mem.read_u32(sp + 0x80);
    sprite_group(fx, g, tex, 0xFC00_0003, 0x4040_0000);
    fx.mem.write_u32(g + 0x40, D + 0x16B0);
    fx.mem.write_u32(g + 0x44, D + 0x18D0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x50, TENTH);
    fx.mem.write_u32(g + 0x4C, 0x4170_0000);
    fx.mem.write_u32(g + 0x54, 0x41C8_0000);
    halves(fx, g, &[(0x58, 0x400), (0x5A, 0x22), (0x5C, 1), (0x5E, 0)]);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x16AA) {
        let p = D + 16 * k as u32;
        fx.mem.set16(p + 0x16BE, -k);
        fx.mem.write_u8(p + 0x16BC, 0x30);
        fx.mem.write_u8(p + 0x16BD, 0);
        for c in [8u32, 4, 0] {
            fx.mem.write_u32(p + 0x16B0 + c, 0);
        }
        fx.mem.set32(p + 0x18D0, -0x2000);
        k += 1;
    }
    // The pulse (+0x1B60): 5 sprites at +0x1BC0, angles +0x1C40.
    let g = D + 0x1B60;
    let tex = fx.mem.read_u32(sp + 0x80);
    sprite_group(fx, g, tex, 0xF800_0003, 0x40C0_0000);
    fx.mem.write_u32(g + 0x40, D + 0x1BC0);
    fx.mem.write_u32(g + 0x44, D + 0x1C40);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x50, 0x3F00_0000);
    fx.mem.write_u32(g + 0x4C, 0x3F80_0000);
    halves(fx, g, &[(0x5A, 5), (0x5C, 1), (0x5E, 0)]);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x1BBA) {
        let p = D + 16 * k as u32;
        fx.mem.set16(p + 0x1BCE, -6 * k);
        fx.mem.write_u8(p + 0x1BCC, 0);
        fx.mem.write_u8(p + 0x1BCD, 0);
        for c in [8u32, 4, 0] {
            fx.mem.write_u32(p + 0x1BC0 + c, 0);
        }
        k += 1;
    }
    fx.texture4(sp + 0x80, 0xF);
    // The quad burst (+0xE0), size 0.8.
    let g = D + 0xE0;
    fx.mem.write_u32(g + 4, 1);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x60), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.quads_setup(g, tex, f32::from_bits(0x3F4C_CCCD), QUAD_TEMPLATE);
    for k in 0..64u32 {
        let q = D + 16 * k;
        let r = fx.rand_from(D, 0x01E0_34F0);
        fx.mem.set32(q + 0x140, r % 0x1000 + 0x3800);
        let r = fx.rand_from(D, 0x01E0_3510);
        fx.mem.set32(q + 0x144, r & 0xFFFF);
        let r = fx.rand_from(D, 0x01E0_352C);
        fx.mem.set32(q + 0x148, r % 0x400 - 0x200);
    }
    // The sparks (+0x5B0): 256 at +0x5E0, width 0.2, length 5.
    let g = D + 0x5B0;
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 3);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x60), (0xB, 0x80)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0x20, 0x3E4C_CCCD);
    fx.mem.write_u32(g + 0x24, 0x40A0_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x60), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x60), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, D + 0x5E0);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x1CC8);
    let node = |fx: &mut Fx, slot: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + slot);
        fx.node_position(h, s, out);
    };
    let target = |fx: &mut Fx, out: u32| {
        let t = fx.target_handle();
        fx.unit_position(t, out);
    };
    target(fx, D + 0x560);
    node(fx, 0x1CD0, D + 0x90);
    node(fx, 0x1CD0, sp + 0x60);
    node(fx, 0x1CD4, sp + 0x70);
    fx.look_angles(sp + 0x60, sp + 0x70, D + 0x80);
    node(fx, 0x1CD0, sp + 0x60);
    target(fx, sp + 0x70);
    node(fx, 0x1CD0, D + 0x1600);
    fx.look_angles(sp + 0x70, sp + 0x60, D + 0x15F0);
    target(fx, D + 0x1B10);
    for c in 0..4u32 {
        let w = fx.mem.read_u32(D + 0x15F0 + 4 * c);
        fx.mem.write_u32(D + 0x1B00 + 4 * c, w);
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x70, s3 the caller's.
    let regs = [Some(D), Some(OT), Some(sp + 0x70), None];
    fx.xform_world_saving(D, 0x01E0_36FC, regs);
    fx.xform_world_saving(D + 0x70, 0x01E0_3704, regs);
    fx.xform_world_saving(D + 0x15E0, 0x01E0_370C, regs);
    fx.xform_world_saving(D + 0x1AF0, 0x01E0_3714, regs);
    fx.xform_world_saving(D + 0x540, 0x01E0_371C, regs);
    if n(fx) >= 0xDF {
        fx.step_down(D + 0x1CC3, 0, 6);
    }
    fx.frame_quad(D + 0x1CC0);
    if n(fx) >= 0x16D {
        fx.step_up(D + 0x1CD8, 0x80, 6);
    } else {
        fx.step_down(D + 0x1CD8, 0x20, 6);
    }
    let light = fx.mem.u8(D + 0x1CD8);
    fx.set_stage_light(light);
    if n(fx) == 0xBE {
        fx.mem.write_u8(D + 0xEB, 0x20);
    }
    fx.quads_draw(D + 0xE0, D + 0x70);
    if (0x2D..0xBE).contains(&n(fx)) {
        fx.step_down(D + 0xEB, 0, 2);
    } else {
        fx.step_up(D + 0xEB, 0x20, 2);
    }
    for k in 0..64u32 {
        let a = D + 0x144 + 16 * k;
        let x = fx.mem.i32(a);
        fx.mem.set32(a, if k % 2 == 0 { x.wrapping_add(0x80) } else { x.wrapping_sub(0x80) });
    }
    if n(fx) == 0xBE {
        fx.mem.write_u8(D + 0x165B, 0x80);
    }
    fx.sprites(D + 0x1650, D + 0x15E0);
    if n(fx) >= 0x16C {
        fx.step_down(D + 0x165B, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x16AA) {
        let p = D + 16 * k as u32;
        k += 1;
        let life = fx.mem.i16(p + 0x16BE);
        if life < 0 {
            continue;
        }
        fx.mem.set32(p + 0x18D0, life.wrapping_mul(0x180).wrapping_sub(0x2000));
        let y = fx.mem.f(p + 0x16B4);
        if ee_le(y, 160.0) {
            fx.mem.setf(p + 0x16B4, ee::add(y, 5.0));
        }
        if ee_le(fx.mem.f(p + 0x16B4), 160.0) {
            continue;
        }
        for c in [4u32, 0, 8] {
            fx.mem.write_u32(p + 0x16B0 + c, 0);
        }
        fx.mem.set16(p + 0x16BE, 0);
        fx.mem.set32(p + 0x18D0, -0x2000);
        let alpha = if n(fx) < 0xBE {
            0
        } else if (k - 1) % 2 == 0 {
            0x80
        } else {
            0x40
        };
        fx.mem.write_u8(p + 0x16BC, alpha);
    }
    if n(fx) == 0xDE {
        fx.mem.write_u8(D + 0x1B6B, 0x80);
    }
    fx.sprites(D + 0x1B60, D + 0x1AF0);
    if n(fx) >= 0x16C {
        fx.step_down(D + 0x1B6B, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x1BBA) {
        let p = D + 16 * k as u32;
        let even = k % 2 == 0;
        k += 1;
        let life = fx.mem.i16(p + 0x1BCE);
        if life < 0 {
            continue;
        }
        if life >= 0x1F {
            fx.mem.set16(p + 0x1BCE, 0);
            fx.mem.write_u32(p + 0x1BC4, 0);
            if n(fx) >= 0xDE {
                fx.mem.write_u8(p + 0x1BCC, if even { 0x80 } else { 0x40 });
            }
            continue;
        }
        if life >= 0x11 {
            fx.step_down(p + 0x1BCC, 0, if even { 8 } else { 4 });
        }
        let y = ee::add(fx.mem.f(p + 0x1BC4), 1.0);
        fx.mem.setf(p + 0x1BC4, y);
    }
    if n(fx) >= 0xDE {
        blib::trail::sparks(fx, OT, D + 0x5B0, D + 0x540);
        if n(fx) >= 0x16C {
            fx.step_down(D + 0x5BB, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x5DC) {
            let p = D + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0x5EE) < 0 {
                continue;
            }
            let s = f32::from_bits(0x3FA6_6666);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 0x5E0 + 4 * c), s);
                fx.mem.setf(p + 0x5E0 + 4 * c, x);
            }
            if fx.mem.i16(p + 0x5EE) < 9 {
                continue;
            }
            let r = fx.rand_from(D, 0x01E0_3B54);
            let (_, h) = fx.div(r, 0xA);
            let len = ee::add(1.0, ee::mul(f32::from_bits(TENTH), ee::from_i32(h)));
            fx.random_dir(sp + 0x50, len);
            for c in [1u32, 2, 0] {
                let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
                fx.mem.write_u32(p + 0x5E0 + 4 * c, w);
            }
            let r = fx.rand_from(D, 0x01E0_3BAC);
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0x5EE, h.wrapping_neg());
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x1CC8, m);
    let left = fx.mem.i32(D + 0x1CC4) - 1;
    fx.mem.set32(D + 0x1CC4, left);
    if m == 0x16C {
        2
    } else if m == 0xC4 {
        1
    } else if (0xDE..0x16C).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
