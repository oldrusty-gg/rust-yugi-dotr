//! BMODULE b542 (card 470), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04240, data area 0x1ED0000) [F] (helper C6, 31
//! Oct 2026): b186's program shape: the stage dims until update 0x7F; two
//! glows (plates +0xE0 / +0x1A0, discs +0x120 / +0x1E0) at the attacker's
//! nodes 3 / 4 until update 0x18; from 0x48 a fountain of 128 sprites
//! (+0x2E0) at node 0x1F; from 0x40 a disc (+0x260) there, sparks
//! (+0xBB0) and the target's afterimage in wavy strips (+0x29A0) with the
//! unit tinted; from 0x44 three rings swelling at the target (+0x1D30 +
//! 0x400 k). Returns 1 on update 0x36, 6 on 0x40..0x7E, 2 on 0x7F, 8
//! after 0x91.
//!
//! Library at its own addresses: `colours4` (0x1E00080), `strips`
//! (0x1E00120), `scatter` (0x1E00790), `trail::sparks` (0x1E00950), `disc`
//! (0x1E016B0), `plates` (0x1E01C10), `ring_setup` / `ring_draw`
//! (0x1E021A0 / 0x1E02460), `sprites` (0x1E02950).
use crate::b101::unit_tint;
use crate::b186::{bytes, fountain_init, fountain_step, halves, rings_step, sparks_step};
use crate::blib::{afterimage, plate, spawn, trail};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PLATES: u32 = 0x01E0_1C10;

/// The entry 0x1E04240.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x29F0);
    for x in [0u32, 0x1F0, 0xB40, 0x1BE0, 0x1C50, 0x1CC0, 0x2930, 0x270, 0x70, 0x130] {
        fx.xform_init(D + x);
    }
    for (at, n, clear) in [(0x29D4u32, 3, 1), (0x29D8, 4, 0), (0x29DC, 0x1F, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x29C8, 0x91);
    fx.mem.set32(D + 0x29CC, 1);
    fx.mem.set32(D + 0x29D0, param);
    let rad = crate::b107::target_radius(fx);
    fx.mem.setf(D + 0x29E8, rad);
    fx.mem.set32(D + 0x29E4, 0x80);
    // The afterimage (+0x29A0).
    fx.mem.write_u32(D + 0x29A0, 0x4_1000);
    bytes(fx, &[(0x29A6, 0x80), (0x29A5, 0x50), (0x29A4, 0x80), (0x29A7, 0x80)]);
    fx.mem.write_u32(D + 0x29B0, 0);
    fx.mem.write_u32(D + 0x29AC, 0x2000);
    halves(fx, &[(0x29A8, 0)]);
    fx.mem.write_u32(D + 0x29B4, 0x4120_0000);
    let t = fx.target_handle();
    halves(fx, &[(0x29AA, t)]);
    bytes(fx, &[(0x29E2, 0x80), (0x29E1, 0x80), (0x29E0, 0x80), (0x29E3, 0x80)]);
    // Three discs (+0x260, +0x120, +0x1E0).
    for (c, r) in [(0x260u32, 0x4170_0000u32), (0x120, 0x4000_0000), (0x1E0, 0x4000_0000)] {
        fx.mem.write_u32(D + c, 0x8_0000);
        bytes(fx, &[(c + 6, 0x80), (c + 5, 0x50), (c + 4, 0x80), (c + 7, 0xFF)]);
        fx.mem.write_u32(D + c + 8, r);
        halves(fx, &[(c + 0xC, 0x24), (c + 0xE, 0)]);
    }
    fx.texture4(sp + 0xA0, 0xF);
    // Two plates (+0xE0, +0x1A0).
    for p in [0xE0u32, 0x1A0] {
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.mem.write_u32(D + p, tex);
        fx.mem.write_u32(D + p + 4, 0x8_0000);
        bytes(fx, &[(p + 0xA, 0x80), (p + 9, 0x50), (p + 8, 0x80), (p + 0xB, 0x80)]);
        for (o, v) in [(0xCu32, 0x3F00_0000u32), (0x10, 0x40C0_0000), (0x14, 0x3F00_0000), (0x18, 0x4000_0000)] {
            fx.mem.write_u32(D + p + o, v);
        }
        halves(fx, &[(p + 0x20, 0xB0), (p + 0x22, 0x40), (p + 0x24, 0x20), (p + 0x26, 0x20)]);
        for o in [0x30u32, 0x34, 0x38] {
            fx.mem.write_u32(D + p + o, 0);
        }
    }
    // Three rings (+0x1D30 + 0x400 k).
    for k in 0..3u32 {
        let g = 0x1D30 + 0x400 * k;
        fx.mem.write_u32(D + g + 4, 0);
        halves(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x80)]);
        bytes(fx, &[(g + 0xA, 0x80), (g + 9, 0x50), (g + 8, 0x80), (g + 0xB, 0x80)]);
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(D + g, tex, f32::from_bits(0x413C_CCCD), f32::from_bits(0x4143_3333), 0.0, 0.0);
    }
    fx.texture5(sp + 0xA0, 0);
    // The sparks (+0xBB0).
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0xBB0, tex);
    fx.mem.write_u32(D + 0xBB4, 3);
    bytes(fx, &[(0xBBA, 0x80), (0xBB9, 0x80), (0xBB8, 0x80), (0xBBB, 0)]);
    fx.mem.write_u32(D + 0xBD0, 0x3F33_3333);
    fx.mem.write_u32(D + 0xBD4, 0x4080_0000);
    halves(fx, &[(0xBC0, 0x20), (0xBC2, 0x20), (0xBC4, 0x20), (0xBC6, 0x20), (0xBC8, 0x20), (0xBCA, 0x20), (0xBCC, 0x80), (0xBCE, 0x40)]);
    fx.mem.write_u32(D + 0xBBC, D + 0xBE0);
    fx.mem.write_u32(D + 0xBD8, 0x4120_0000);
    halves(fx, &[(0xBDC, 0x100)]);
    bytes(fx, &[(0xBDE, 1), (0xBDF, 0)]);
    spawn::scatter(fx, D + 0xBB0, 0xC, 3.0);
    fx.texture5(sp + 0xA0, 1);
    // The sprite fountain (+0x2E0).
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x2E0, tex);
    fx.mem.write_u32(D + 0x2E4, 0x7000_0003);
    bytes(fx, &[(0x2EA, 0x80), (0x2E9, 0x50), (0x2E8, 0x80), (0x2EB, 0x80)]);
    fx.mem.write_u32(D + 0x2EC, 0x4000_0000);
    fx.mem.write_u32(D + 0x300, 0);
    fx.mem.write_u32(D + 0x304, 0x400);
    fx.mem.write_u32(D + 0x308, 0);
    halves(fx, &[(0x310, 0), (0x312, 0), (0x314, 0x20), (0x316, 0x20), (0x318, 0), (0x31A, 0), (0x31C, 0x80), (0x31E, 0x40)]);
    fx.mem.write_u32(D + 0x320, D + 0x340);
    fx.mem.write_u32(D + 0x324, 0);
    fx.mem.write_u32(D + 0x328, 0);
    fx.mem.write_u32(D + 0x32C, 0x4000_0000);
    halves(fx, &[(0x33A, 0x80), (0x33C, 8), (0x33E, 0x64)]);
    fountain_init(fx, 0x340, 0x33A, [0x01E0_46B0, 0x01E0_46CC, 0x01E0_46E8, 0x01E0_4738]);
    for (x, v) in [(0x1BE0u32, 0u32), (0x1C50, 0x3F00_0000), (0x1CC0, 0x3F80_0000), (0x270, 0x4000_0000)] {
        for o in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + o, v);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x29CC);
    node(fx, 0x29D4, D + 0x90);
    node(fx, 0x29D8, D + 0x150);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x290);
    if n(fx) < 0x40 {
        node(fx, 0x29DC, D + 0xB60);
        node(fx, 0x29DC, D + 0x210);
    }
    for k in 0..3u32 {
        let t = fx.target_handle();
        fx.unit_position(t, D + 0x1C00 + 0x70 * k);
        let h = fx.self_handle();
        fx.unit_point(1, h, sp + 0x80);
        let t = fx.target_handle();
        fx.unit_point(1, t, sp + 0x90);
        fx.look_angles(sp + 0x80, sp + 0x90, D + 0x1BF0 + 0x70 * k);
    }
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x2950);
    // s0 = D, s1 = the OT, s2 = D + 0x70 k (0x150 after the loops), s3 =
    // sp + 0x90 then the transform loop's counter.
    let regs = |s2: u32, s3: u32| [Some(D), Some(OT), Some(s2), Some(s3)];
    fx.xform_world_saving(D, 0x01E0_48D8, regs(D + 0x150, sp + 0x90));
    fx.xform_billboard_saving(D + 0x1F0, 0x01E0_48E0, regs(D + 0x150, sp + 0x90));
    fx.xform_world_saving(D + 0xB40, 0x01E0_48E8, regs(D + 0x150, sp + 0x90));
    for k in 0..3u32 {
        fx.xform_world_saving(D + 0x1BE0 + 0x70 * k, 0x01E0_48F8, regs(D + 0x70 * k, k));
    }
    fx.xform_world_saving(D + 0x2930, 0x01E0_4914, regs(D + 0x150, 3));
    fx.xform_world_saving(D + 0x270, 0x01E0_491C, regs(D + 0x150, 3));
    fx.xform_billboard_saving(D + 0x70, 0x01E0_4924, regs(D + 0x150, 3));
    fx.xform_billboard_saving(D + 0x130, 0x01E0_492C, regs(D + 0x150, 3));
    if n(fx) < 0x7F {
        fx.step_down(D + 0x29E4, 0x20, 2);
    } else {
        fx.step_up(D + 0x29E4, 0x80, 4);
    }
    let l = fx.mem.read_u8(D + 0x29E4);
    fx.set_stage_light(i32::from(l));
    if n(fx) < 0x18 {
        plate::plates(fx, PLATES, OT, D + 0xE0, D + 0x70);
        fx.step_down(D + 0xEB, 0, 8);
        plate::plates(fx, PLATES, OT, D + 0x1A0, D + 0x130);
        fx.step_down(D + 0x1AB, 0, 8);
        fx.disc(D + 0x120, D + 0x70);
        fx.step_down(D + 0x127, 0, 0x10);
        fx.disc(D + 0x1E0, D + 0x130);
        fx.step_down(D + 0x1E7, 0, 0x10);
        for a in [0x118u32, 0x1D8] {
            let v = fx.mem.i32(D + a).wrapping_add(0x400);
            fx.mem.set32(D + a, v);
        }
    }
    if n(fx) >= 0x48 {
        fx.sprites(D + 0x2E0, D + 0x270);
        if n(fx) >= 0x7F {
            fx.step_down(D + 0x2EB, 0, 8);
        }
        fountain_step(fx, 0x340, 0x33A, [0x01E0_4B50, 0x01E0_4B6C, 0x01E0_4BBC, 0x01E0_4BFC]);
    }
    if n(fx) >= 0x40 {
        fx.disc(D + 0x260, D + 0x1F0);
        fx.step_down(D + 0x267, 0, 0x10);
    }
    if n(fx) == 0x40 {
        fx.mem.write_u8(D + 0xBBB, 0x80);
    }
    trail::sparks(fx, OT, D + 0xBB0, D + 0xB40);
    fx.step_down(D + 0xBBB, 0, 8);
    sparks_step(fx, sp, 0xBE0, 0xBDC, 0xD, 0x01E0_4D20);
    if n(fx) >= 0x44 {
        let m = n(fx);
        rings_step(fx, 0x1D30, 0x1BE0, m, 0x7F, [0x01E0_4DD8, 0x01E0_4DF4, 0x01E0_4E10]);
    }
    if n(fx) >= 0x40 {
        let a = fx.mem.read_u8(D + 0x29A7);
        fx.mem.write_u8(D + 0x29E3, 0x80u8.wrapping_sub(a));
        afterimage::strips(fx, D + 0x29A0, D + 0x2950);
        let h = fx.mem.i16(D + 0x29AA);
        unit_tint(fx, h, D + 0x29E0);
        if n(fx) >= 0x80 {
            fx.step_down(D + 0x29A7, 0, 8);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x29CC, m);
    let left = fx.mem.i32(D + 0x29C8) - 1;
    fx.mem.set32(D + 0x29C8, left);
    if m == 0x7F {
        2
    } else if m == 0x36 {
        1
    } else if (0x40..0x7F).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
