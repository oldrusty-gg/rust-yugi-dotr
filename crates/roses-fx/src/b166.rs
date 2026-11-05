//! BMODULE b166 (card 627), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05400, data area 0x1ED0000) [F] (helper C4, 31
//! Oct 2026): b436's program re-coloured every update (one random blue,
//! green and red byte, & 0x7F, for all nine groups) and without the frame
//! quad or the stage light: from the start a sprite cloud (+0x1B30)
//! falling round the attacker's node 0x29, a ring (+0x2390) there and a
//! pulsing disc (+0x1470); from update 108 a ribbon trail (+0x70) from node
//! 0x2C toward the target (1 / 24 of the way, ...); on updates 97..181 a
//! turned plate (+0x13B0) and a disc (+0x1AB0); from update 114 a ring
//! (+0x1640) swelling at the target, a spark fountain (+0x310) and a
//! particle cloud (+0x1480); from update 130 a flickering disc (+0x13F0)
//! on the target. Returns 1 on update 118, 6 on 128..223, 2 on 224, 8
//! after 260.
//!
//! Library: `spawn::scatter` (0x1E00120), `trail::sparks` (0x1E002E0)
//! [`crate::blib`]; `ring_setup` / `ring_draw` (0x1E01040 / 0x1E01300),
//! `disc` (0x1E01D80), `sprites_spawn` (0x1E022E0), `sprites`
//! (0x1E02470), `particle_quads` (0x1E03D60) [`crate::fxlib`];
//! `plate::plates` (0x1E017F0); the ribbon (0x1E048F0 / 0x1E04A40).
use crate::b146::cloud_spawn;
use crate::b436::{fstep_down, ring_group, sparks_step, sprite_cloud_init, sprite_cloud_step};
use crate::b642::{disc_record, halves};
use crate::blib;
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PLATES: u32 = 0x01E0_17F0;

/// The entry 0x1E05400.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x27C0);
    for x in [0u32, 0x1340, 0x2A0, 0x15D0, 0x1A40, 0x1400, 0x1AC0] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x27B0u32, 0x2C, 1), (0x27B4, 0x29, 0), (0x27B8, 1, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x27A4, 0x104);
    fx.mem.set32(D + 0x27A8, 1);
    fx.mem.set32(D + 0x27AC, param);
    fx.mem.set32(D + 0x27A0, 0x18);
    disc_record(fx, D + 0x1470, [0x80, 0x80, 0x80, 0xFF], 0x4120_0000);
    disc_record(fx, D + 0x13F0, [0x80, 0x80, 0x80, 0xFF], 0x40A0_0000);
    disc_record(fx, D + 0x1AB0, [0x80, 0x80, 0x80, 0], 0x4000_0000);
    fx.texture4(sp + 0xB0, 0xF);
    // The ribbon (+0x70): 32 slots, life 100, width 60, cells (200, 32,
    // 7, 32) / (200, 32, 256, 256).
    let g = D + 0x70;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 8, 0x8003);
    for k in [6u32, 5, 4, 7] {
        fx.mem.write_u8(g + k, 0x80);
    }
    halves(fx, g, &[(0x10, 0xC8), (0x12, 0x20), (0x14, 7), (0x16, 0x20), (0x18, 0xC8), (0x1A, 0x20), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, 0x4270_0000);
    halves(fx, g, &[(0x22, 8), (0x24, 0), (0x26, 0x64), (0x28, 0x20), (0x2A, 0), (0x2C, 1)]);
    // The spark fountain (+0x310): 256 sparks at +0x340.
    let g = D + 0x310;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x2000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x4040_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x60), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x60), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, D + 0x340);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    // The plate (+0x13B0): half sizes (0.5, 4) / (3, 0.5), rectangle
    // (176, 64, 32, 32).
    let g = D + 0x13B0;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x8_0000);
    for k in [0xAu32, 9, 8] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 0xB, 0);
    for (k, v) in [(0xCu32, 0x3F00_0000u32), (0x10, 0x4080_0000), (0x14, 0x3F00_0000), (0x18, 0x4040_0000)] {
        fx.mem.write_u32(g + k, v);
    }
    halves(fx, g, &[(0x20, 0xB0), (0x22, 0x40), (0x24, 0x20), (0x26, 0x20)]);
    for k in [0x30u32, 0x34, 0x38] {
        fx.mem.write_u32(g + k, 0);
    }
    // The particle cloud (+0x1480): 16 particles, half size 20.
    let g = D + 0x1480;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x85);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0xF0), (0x2A, 0), (0x24, 0x10), (0x26, 0x10), (0x2C, 0x100), (0x2E, 0x100)]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 1), (0x13C, 1), (0x140, 8), (0x136, 0xA), (0x138, 0xA)]);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, x);
    }
    cloud_spawn(fx, g);
    ring_group(fx, D + 0x1640, 0, 0x80, [0x80, 0x80, 0x80, 0x80]);
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.ring_setup(D + 0x1640, tex, f32::from_bits(0x4133_3333), f32::from_bits(0x414C_CCCD), 0.0, 0.0);
    ring_group(fx, D + 0x2390, 0x8000, 0x40, [0x80, 0x80, 0x80, 0]);
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.ring_setup(D + 0x2390, tex, 2.0, 6.0, 0.0, 8.0);
    // The sprite cloud (+0x1B30): 128 sprites at +0x1B90, size 1, radius
    // 12.
    let g = D + 0x1B30;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0001);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0xC, 0x3F80_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0xE4), (0x34, 0xA), (0x36, 0xA), (0x38, 0), (0x3A, 0xE4), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x1B90);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4140_0000);
    halves(fx, g, &[(0x5A, 0x80), (0x5C, 2), (0x5E, 0)]);
    fx.sprites_spawn(g, 1.0);
    sprite_cloud_init(fx, g);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x27A8);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    node(fx, 0x27B4, sp + 0x90);
    node(fx, 0x27B8, sp + 0xA0);
    node(fx, 0x27B4, D + 0x1AE0);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x1AD0);
    node(fx, 0x27B0, D + 0x1A60);
    for out in [D + 0x2790, D + 0x1420, D + 0x2C0, D + 0x15F0] {
        let t = fx.target_handle();
        fx.unit_position(t, out);
    }
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x90);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0xA0);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x2B0);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x15E0);
    if n(fx) < 0x6D {
        node(fx, 0x27B0, D + 0x1360);
    }
    if n(fx) >= 0x6C {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x1360);
        let m = fx.mem.i32(D + 0x27A0);
        fx.approach_by(D + 0x1360, D + 0x2790, m);
        let m = fx.mem.i32(D + 0x27A0);
        if m != 0 {
            fx.mem.set32(D + 0x27A0, m - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 = sp + 0xA0.
    let regs = [Some(D), Some(OT), Some(sp + 0xA0), None];
    fx.xform_world_saving(D, 0x01E0_5B5C, regs);
    fx.xform_billboard_saving(D + 0x1340, 0x01E0_5B64, regs);
    fx.xform_world_saving(D + 0x2A0, 0x01E0_5B6C, regs);
    fx.xform_world_saving(D + 0x15D0, 0x01E0_5B74, regs);
    fx.xform_billboard_saving(D + 0x1A40, 0x01E0_5B7C, regs);
    fx.xform_billboard_saving(D + 0x1400, 0x01E0_5B84, regs);
    fx.xform_world_saving(D + 0x1AC0, 0x01E0_5B8C, regs);
    // One random blue / green / red for every group.
    for (c, ra) in [(2u32, 0x01E0_5B94), (1, 0x01E0_5BD0), (0, 0x01E0_5C0C)] {
        let v = (fx.rand_from(D, ra) & 0x7F) as u8;
        for g in [0x1648u32, 0x2398, 0x1488, 0x13B8, 0x1B38, 0x318, 0x74, 0x1AB4, 0x1474] {
            fx.mem.write_u8(D + g + c, v);
        }
    }
    fx.ring_draw(D + 0x2390, D + 0x1AC0);
    if n(fx) >= 0x50 {
        fx.step_down(D + 0x239B, 0, 6);
    } else {
        fx.step_up(D + 0x239B, 0x20, 2);
    }
    fx.sprites(D + 0x1B30, D + 0x1AC0);
    fx.step_up(D + 0x1B3B, 0x80, 8);
    sprite_cloud_step(fx, D + 0x1B30, [0x01E0_5D3C, 0x01E0_5D80]);
    if n(fx) >= 0x82 {
        fx.disc(D + 0x13F0, D + 0x1340);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_5DE0)]);
    }
    let m = n(fx);
    if m >= 0xE0 {
        fx.step_down(D + 0x13F7, 0, 0x10);
    } else {
        fx.mem.write_u32(D + 0x13F8, if m % 2 == 0 { 0x40A0_0000 } else { 0x4040_0000 });
    }
    fx.disc(D + 0x1470, D + 0x1A40);
    fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_5E4C)]);
    let m = n(fx);
    if m >= 0xE0 {
        fx.step_down(D + 0x1477, 0, 0x10);
    } else {
        fx.mem.write_u8(D + 0x1477, if m % 2 == 0 { 0x80 } else { 0x40 });
    }
    let m = n(fx);
    if (0x61..0xB6).contains(&m) {
        blib::plate::plates(fx, PLATES, OT, D + 0x13B0, D + 0x1A40);
        fx.disc(D + 0x1AB0, D + 0x1A40);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_5EE4)]);
        fx.step_up(D + 0x13BB, 0x80, 8);
        fx.step_up(D + 0x1AB7, 0x80, 8);
    }
    if n(fx) >= 0x72 {
        fx.ring_draw(D + 0x1640, D + 0x15D0);
        fx.step_down(D + 0x164B, 0, 8);
        if fx.approach(D + 0x15D0, 2.0, f32::from_bits(0x3E4C_CCCD)) == 0 && n(fx) < 0xE0 {
            for k in [0x15D8u32, 0x15D4, 0x15D0] {
                fx.mem.write_u32(D + k, 0x3DCC_CCCD);
            }
            fx.mem.write_u8(D + 0x164B, 0x80);
        }
        blib::trail::sparks(fx, OT, D + 0x310, D + 0x2A0);
        if n(fx) >= 0xE1 {
            fx.step_down(D + 0x31B, 0, 8);
        }
        sparks_step(fx, D + 0x310, [0x01E0_6030, 0x01E0_6038, 0x01E0_6054, 0x01E0_60A4]);
        for k in [0x88u32, 0x84, 0x80] {
            fx.mem.write_u32(sp + k, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0x1400, D + 0x1480, sp + 0x80);
        if n(fx) >= 0xE0 {
            fx.step_down(D + 0x148B, 0, 2);
        }
    }
    if n(fx) >= 0x6D {
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
        if n(fx) >= 0xE1 {
            fstep_down(fx, D + 0x7C, 0.0, 4.0);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x27A8, m);
    let left = fx.mem.i32(D + 0x27A4) - 1;
    fx.mem.set32(D + 0x27A4, left);
    if m == 0xE0 {
        2
    } else if m == 0x76 {
        1
    } else if (0x80..0xE0).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
