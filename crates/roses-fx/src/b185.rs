//! BMODULE b185 (card 268), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03A80, data area 0x1ED0000) [F] (helper C, 30 Oct
//! 2026): b263's family: a glow at the attacker's node 0x4E (disc +0x380,
//! wobbling quad burst +0x19B0) from update 50, flaring on update 120; from
//! update 120 a ribbon trail (+0x70) toward the target (1 / 16 of the way
//! per update, ...), a fan of quads (+0x1E80) turned toward the target and
//! a rising sprite fountain (+0x8E0); at the target from update 136 a disc
//! / quad burst growing to 1.5; the frame quad from update 123; the stage
//! light down until update 242. Returns 1 on update 126, 6 on 136..241, 2
//! on 242, 8 after 260.
//!
//! Library: `sprites` (0x1E00240), `disc` (0x1E01B30), `quads_setup` /
//! `quads_draw` (0x1E02090 / 0x1E021F0) [`crate::fxlib`], the ribbon
//! (0x1E02800 / 0x1E02950, [`crate::ribbon`]), the fan (0x1E03310 /
//! 0x1E03470, [`crate::fan`]), the frame quad (0x1E00120).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad / fan corner templates (rodata, 0x1E02090's / 0x1E03310's
/// lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_4700;
const FAN_TEMPLATE: u32 = 0x01E0_4740;

/// The entry 0x1E03A80.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn disc_record(fx: &mut Fx, at: u32, radius: u32) {
    fx.mem.write_u32(at, 0x8_0000);
    fx.mem.write_u8(at + 6, 0xA0);
    fx.mem.write_u8(at + 5, 0xD0);
    fx.mem.write_u8(at + 4, 0x70);
    fx.mem.write_u8(at + 7, 0);
    fx.mem.write_u32(at + 8, radius);
    fx.mem.set16(at + 0xC, 0x24);
    fx.mem.set16(at + 0xE, 0);
}

/// A fountain sprite (`p` = D + 16 k, fields at +0x940) set off (as
/// b263's, radius from +0x92C).
fn fountain_spawn(fx: &mut Fx, p: u32, ra: u32, rands: [u32; 4]) {
    let r = fx.mem.f(D + 0x92C);
    let f = ee::mul(10.0, r);
    fx.fix_residue(f, ra);
    let m = ee::fixsfsi(f);
    let a = fx.rand_from(D, rands[0]) & 0xFFFF;
    let x = fx.rand_from(D, rands[1]);
    let (_, h) = fx.div(x, m);
    let tenth = f32::from_bits(0x3DCC_CCCD);
    let radius = ee::mul(tenth, ee::from_i32(h));
    let x = fx.rand_from(D, rands[2]);
    let (_, h) = fx.div(x, 0x28);
    fx.mem.setf(p + 0x944, ee::mul(tenth, ee::neg(ee::from_i32(h - 0x14))));
    let c = fx.tcos(a);
    fx.mem.setf(p + 0x948, ee::mul(radius, c));
    let s = fx.tsin(a);
    fx.mem.setf(p + 0x940, ee::mul(radius, s));
    let x = fx.rand_from(D, rands[3]);
    let (_, h) = fx.div(x, 0xC);
    fx.mem.set16(p + 0x94E, -h);
    fx.mem.write_u8(p + 0x94C, 0x80);
}

fn quad_group(fx: &mut Fx, g: u32, flags: u32, tex: u32, size: f32) {
    fx.mem.write_u32(g + 4, flags);
    for (k, x) in [(0x10u32, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u8(g + 0xA, 0xA0);
    fx.mem.write_u8(g + 9, 0xD0);
    fx.mem.write_u8(g + 8, 0x70);
    fx.mem.write_u8(g + 0xB, 0x20);
    fx.quads_setup(g, tex, size, QUAD_TEMPLATE);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2330);
    for x in [0u32, 0x2A0, 0x870, 0x310, 0x390, 0x1940, 0x1E10] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2314u32, 0x4E, 1), (0x2318, 0x4D, 0), (0x231C, 0x48, 0), (0x2320, 0x3C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x22F0, 0x104);
    fx.mem.set32(D + 0x22F4, 1);
    fx.mem.set32(D + 0x22F8, param);
    fx.mem.set32(D + 0x2300, 0x10);
    fx.mem.set32(D + 0x22FC, 0x80);
    disc_record(fx, D + 0x380, 0x40A0_0000);
    disc_record(fx, D + 0x860, 0x41F0_0000);
    fx.texture5(sp + 0xA0, 0);
    // The ribbon (+0x70): 32 slots, life 64, width 40, cell (0, 0, 16,
    // 32); points at +0xA0.
    let g = D + 0x70;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 8, 0x8003);
    for k in [6u32, 5, 4, 7] {
        fx.mem.write_u8(g + k, 0x80);
    }
    for (k, x) in [(0x10u32, 0), (0x12, 0), (0x14, 0x10), (0x16, 0x20), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u32(g + 0xC, 0x4220_0000);
    for (k, x) in [(0x22u32, 8), (0x24, 0), (0x26, 0x40), (0x28, 0x20), (0x2A, 0), (0x2C, 1)] {
        fx.mem.set16(g + k, x);
    }
    // The fan (+0x1E80): cell (64, 0, 64, 64), half widths 0.5 / 1,
    // height 8, size 3.
    let g = D + 0x1E80;
    fx.mem.write_u32(g + 4, 1);
    for (k, x) in [(0x10u32, 0x40), (0x12, 0), (0x14, 0x40), (0x16, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    for k in [0xAu32, 9, 8] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 0xB, 0x10);
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x3F80_0000);
    fx.mem.write_u32(g + 0x28, 0x4100_0000);
    fx.mem.write_u32(g + 0x2C, 0);
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.fan_setup(g, tex, 3.0, FAN_TEMPLATE);
    // The fountain (+0x8E0): 256 sprites, size 0.5, radius 10.
    let g = D + 0x8E0;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0xC, 0x3F00_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    for (k, x) in [(0x30u32, 0), (0x32, 0x20), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0x20), (0x3C, 0x80), (0x3E, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u32(g + 0x40, D + 0x940);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4120_0000);
    fx.mem.set16(g + 0x5A, 0x100);
    fx.mem.set16(g + 0x5C, 1);
    fx.mem.set16(g + 0x5E, 0);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x93A) {
        fountain_spawn(fx, D + 16 * k as u32, 0x01E0_3D68, [0x01E0_3D70, 0x01E0_3D8C, 0x01E0_3DB4, 0x01E0_3E0C]);
        k += 1;
    }
    fx.texture4(sp + 0xA0, 0xF);
    let tex = fx.mem.read_u32(sp + 0xA0);
    quad_group(fx, D + 0x400, 0x8_0001, tex, 4.0);
    let tex = fx.mem.read_u32(sp + 0xA0);
    quad_group(fx, D + 0x19B0, 1, tex, 0.5);
    for k in 0..64u32 {
        let q = D + 16 * k;
        let r = fx.rand();
        fx.mem.set32(q + 0x1A10, (r & 0x3FFF) - 0x2000);
        let r = fx.rand();
        fx.mem.set32(q + 0x1A14, r & 0xFFFF);
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x6000);
        fx.mem.set32(q + 0x1A18, h - 0x3000);
    }
    for x in [0x390u32, 0x1940] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, 0x3DCC_CCCD);
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
    node(fx, 0x2314, sp + 0x80);
    node(fx, 0x2318, sp + 0x90);
    fx.look_angles(sp + 0x80, sp + 0x90, D + 0x1950);
    fx.midpoint(sp + 0x80, sp + 0x90, D + 0x1960);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x2304);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x3B0);
    node(fx, 0x2314, D + 0x890);
    fx.look_angles(D + 0x2304, sp + 0x80, D + 0x880);
    node(fx, 0x2314, D + 0x330);
    if fx.mem.i32(D + 0x22F4) < 0x79 {
        node(fx, 0x2314, D + 0x2C0);
    }
    if fx.mem.i32(D + 0x22F4) >= 0x78 {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x2C0);
        let n = fx.mem.i32(D + 0x2300);
        fx.approach_by(D + 0x2C0, D + 0x2304, n);
        let n = fx.mem.i32(D + 0x2300);
        if n != 0 {
            fx.mem.set32(D + 0x2300, n - 1);
        }
    }
    if fx.mem.i32(D + 0x22F4) == 0x78 {
        let t = fx.target_handle();
        fx.unit_position(t, sp + 0x80);
        node(fx, 0x2314, sp + 0x90);
        fx.look_angles(sp + 0x80, sp + 0x90, D + 0x1E20);
        for k in 0..4u32 {
            let w = fx.mem.read_u32(sp + 0x90 + 4 * k);
            fx.mem.write_u32(D + 0x1E30 + 4 * k, w);
        }
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x90.
    let regs = [Some(D), Some(OT), Some(sp + 0x90), None];
    fx.xform_world_saving(D, 0x01E0_4144, regs);
    fx.xform_billboard_saving(D + 0x2A0, 0x01E0_414C, regs);
    fx.xform_world_saving(D + 0x870, 0x01E0_4154, regs);
    fx.xform_billboard_saving(D + 0x310, 0x01E0_415C, regs);
    fx.xform_billboard_saving(D + 0x390, 0x01E0_4164, regs);
    fx.xform_world_saving(D + 0x1940, 0x01E0_416C, regs);
    fx.xform_world_saving(D + 0x1E10, 0x01E0_4174, regs);
    if fx.mem.i32(D + 0x22F4) >= 0x7B {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if fx.mem.i32(D + 0x22F4) < 0xF2 {
        fx.step_down(D + 0x22FC, 0x20, 4);
    } else {
        fx.step_up(D + 0x22FC, 0x80, 4);
    }
    let light = fx.mem.u8(D + 0x22FC);
    fx.set_stage_light(light);
    let step20 = f32::from_bits(0x3D4C_CCCD);
    if fx.mem.i32(D + 0x22F4) >= 0x88 {
        fx.approach(D + 0x390, 1.5, step20);
        fx.disc(D + 0x860, D + 0x390);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_420C)]);
        fx.quads_draw(D + 0x400, D + 0x390);
        if fx.mem.i32(D + 0x22F4) >= 0xF2 {
            fx.step_down(D + 0x867, 0, 0x10);
            fx.step_down(D + 0x40B, 0, 2);
        } else {
            fx.step_up(D + 0x867, 0xFF, 8);
        }
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0x460 + 16 * k + 4 * c;
                let x = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, x);
            }
        }
    }
    let n = fx.mem.i32(D + 0x22F4);
    if n >= 0x78 {
        if n == 0x78 {
            fx.mem.write_u8(D + 0x387, 0xFF);
            fx.mem.write_u8(D + 0x19BB, 0x20);
            for k in [8u32, 4, 0] {
                fx.mem.write_u32(D + 0x1940 + k, 0x4000_0000);
            }
        }
        fx.disc(D + 0x380, D + 0x310);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_42EC)]);
        fx.step_down(D + 0x387, 0, 2);
        fx.quads_draw(D + 0x19B0, D + 0x1940);
        fx.step_down(D + 0x19BB, 0, 1);
    }
    let n = fx.mem.i32(D + 0x22F4);
    if (0x32..0x78).contains(&n) {
        fx.disc(D + 0x380, D + 0x310);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_4348)]);
        fx.quads_draw(D + 0x19B0, D + 0x1940);
        if fx.approach(D + 0x1940, 1.0, step20) == 0 {
            fx.step_down(D + 0x19BB, 0, 1);
            fx.step_down(D + 0x387, 0, 4);
        } else {
            fx.step_up(D + 0x387, 0xFF, 8);
        }
        for k in 0..64u32 {
            let a = D + 0x1A14 + 16 * k;
            let x = fx.mem.i32(a);
            fx.mem.set32(a, if k % 2 == 0 { x.wrapping_add(0x80) } else { x.wrapping_sub(0x80) });
        }
    }
    if fx.mem.i32(D + 0x22F4) >= 0x79 {
        fx.sprites(D + 0x8E0, D + 0x870);
        fx.step_down(D + 0x8EB, 0, 4);
        let grow = f32::from_bits(0x3F8C_CCCD);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x93A) {
            let p = D + 16 * k as u32;
            if fx.mem.i16(p + 0x94E) >= 0 {
                let y = ee::add(fx.mem.f(p + 0x944), 2.0);
                fx.mem.setf(p + 0x944, y);
                let x = ee::mul(fx.mem.f(p + 0x940), grow);
                fx.mem.setf(p + 0x940, x);
                let z = ee::mul(fx.mem.f(p + 0x948), grow);
                fx.mem.setf(p + 0x948, z);
                fx.step_down(p + 0x94C, 0, 8);
                if !ee_le(fx.mem.f(p + 0x944), 20.0) {
                    fountain_spawn(fx, p, 0x01E0_44CC, [0x01E0_44D4, 0x01E0_44F0, 0x01E0_4518, 0x01E0_4570]);
                }
            }
            k += 1;
        }
    }
    if fx.mem.i32(D + 0x22F4) >= 0x79 {
        fx.fan_draw(D + 0x1E80, D + 0x1E10);
        if fx.mem.i32(D + 0x22F4) % 2 == 0 {
            fx.step_down(D + 0x1E8B, 0, 1);
        }
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
    }
    let n = fx.mem.i32(D + 0x22F4) + 1;
    fx.mem.set32(D + 0x22F4, n);
    let left = fx.mem.i32(D + 0x22F0) - 1;
    fx.mem.set32(D + 0x22F0, left);
    if n == 0xF2 {
        2
    } else if n == 0x7E {
        1
    } else if (0x88..0xF2).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
