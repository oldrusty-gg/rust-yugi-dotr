//! BMODULE b184 (card 175), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04E40, data area 0x1ED0000) [F] (helper C4, 31
//! Oct 2026): b628's program with three rings: on update 53 two rings of
//! the quad trail (+0x1E10) pushed at the attacker's nodes 0xC / 0x39; a
//! glow at node 0x23 (disc +0x380, quad burst +0x19B0) from update 15,
//! flaring on update 49; from update 49 a ribbon trail (+0x70) toward the
//! target, from 50 the fan (+0x20D0) and the sprite fountain (+0x8E0);
//! from update 65 three rings (+0x2690 + 0x400 k, transforms +0x2540 +
//! 0x70 k) at the target, each swelling to 2 and restarting turned at
//! random, with a disc / quad burst growing to 1.5; the frame quad from
//! update 49; the stage light down until update 252. Returns 1 on update
//! 54, 6 on 64..251, 2 on 252, 8 after 270.
//!
//! Library as b628's at its own addresses: the frame quad (0x1E00120),
//! `sprites` (0x1E00240), `disc` (0x1E01B30), `ring_setup` / `ring_draw`
//! (0x1E02090 / 0x1E02350), `quadtrail::push` / `draw` (0x1E02840 /
//! 0x1E02A40), `quads_setup` / `quads_draw` (0x1E03450 / 0x1E035B0), the
//! ribbon (0x1E03BC0 / 0x1E03D10), the fan (0x1E046D0 / 0x1E04830).
use crate::b628::{fountain_spawn, halves, trail_fade, trail_group};
use crate::blib::quadtrail;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad / fan corner templates (rodata, 0x1E03450's / 0x1E046D0's
/// lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_5E40;
const FAN_TEMPLATE: u32 = 0x01E0_5E80;
/// `quadtrail::draw` (its template is record-relative to it).
const TRAIL_DRAW: u32 = 0x01E0_2A40;

/// The entry 0x1E04E40.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn disc_record(fx: &mut Fx, at: u32, radius: u32) {
    fx.mem.write_u32(at, 0x8_0000);
    fx.mem.write_u8(at + 6, 0xE0);
    fx.mem.write_u8(at + 5, 0x30);
    fx.mem.write_u8(at + 4, 0x20);
    fx.mem.write_u8(at + 7, 0);
    fx.mem.write_u32(at + 8, radius);
    fx.mem.set16(at + 0xC, 0x24);
    fx.mem.set16(at + 0xE, 0);
}

fn quad_group(fx: &mut Fx, g: u32, flags: u32, tex: u32, size: f32) {
    fx.mem.write_u32(g + 4, flags);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    fx.mem.write_u8(g + 0xA, 0xE0);
    fx.mem.write_u8(g + 9, 0x30);
    fx.mem.write_u8(g + 8, 0x20);
    fx.mem.write_u8(g + 0xB, 0x20);
    fx.quads_setup(g, tex, size, QUAD_TEMPLATE);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x32D0);
    for x in [0u32, 0x2A0, 0x870, 0x310, 0x390, 0x1940, 0x2060, 0x2540, 0x25B0, 0x2620] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x32B4u32, 0x23, 1), (0x32B8, 0x24, 0), (0x32BC, 0x39, 0), (0x32C0, 0xC, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x3290, 0x10E);
    fx.mem.set32(D + 0x3294, 1);
    fx.mem.set32(D + 0x3298, param);
    fx.mem.set32(D + 0x32A0, 0x10);
    fx.mem.set32(D + 0x329C, 0x80);
    disc_record(fx, D + 0x380, 0x40A0_0000);
    disc_record(fx, D + 0x860, 0x41B4_0000);
    fx.texture5(sp + 0xB0, 0);
    // The ribbon (+0x70), as b628's.
    let g = D + 0x70;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 8, 0x8003);
    for k in [6u32, 5, 4, 7] {
        fx.mem.write_u8(g + k, 0x80);
    }
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x10), (0x16, 0x20), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, 0x4220_0000);
    halves(fx, g, &[(0x22, 8), (0x24, 0), (0x26, 0x48), (0x28, 0x20), (0x2A, 0), (0x2C, 1)]);
    // The fan (+0x20D0), as b628's.
    let g = D + 0x20D0;
    fx.mem.write_u32(g + 4, 1);
    halves(fx, g, &[(0x10, 0x40), (0x12, 0), (0x14, 0x40), (0x16, 0x40)]);
    for k in [0xAu32, 9, 8] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 0xB, 0x10);
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x3F80_0000);
    fx.mem.write_u32(g + 0x28, 0x4100_0000);
    fx.mem.write_u32(g + 0x2C, 0);
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.fan_setup(g, tex, 3.0, FAN_TEMPLATE);
    // The fountain (+0x8E0): 256 sprites, size 0.8, radius 4.
    let g = D + 0x8E0;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0xC, 0x3F4C_CCCD);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0x20), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0x20), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x940);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4080_0000);
    halves(fx, g, &[(0x5A, 0x100), (0x5C, 1), (0x5E, 0)]);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x93A) {
        fountain_spawn(fx, D + 16 * k as u32, 0x01E0_5154, [0x01E0_515C, 0x01E0_5178, 0x01E0_51A0, 0x01E0_51F8]);
        k += 1;
    }
    fx.texture4(sp + 0xB0, 0xF);
    let tex = fx.mem.read_u32(sp + 0xB0);
    quad_group(fx, D + 0x400, 0x8_0001, tex, 3.0);
    let tex = fx.mem.read_u32(sp + 0xB0);
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
    // Three rings (+0x2690 + 0x400 k): radii 21.5 / 22.5, cell (142, 0,
    // 32, 128), colour (0x20, 0x30, 0xE0, 0x80).
    for k in 0..3u32 {
        let g = D + 0x2690 + 0x400 * k;
        fx.mem.write_u32(g + 4, 0x8_0400);
        halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
        for (o, x) in [(0xAu32, 0xE0u8), (9, 0x30), (8, 0x20), (0xB, 0x80)] {
            fx.mem.write_u8(g + o, x);
        }
        let tex = fx.mem.read_u32(sp + 0xB0);
        fx.ring_setup(g, tex, f32::from_bits(0x41AC_0000), f32::from_bits(0x41B4_0000), 0.0, 0.0);
    }
    let tex = fx.mem.read_u32(sp + 0xB0);
    trail_group(fx, D + 0x1E10, tex);
    for x in [0x390u32, 0x1940] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, 0x3DCC_CCCD);
        }
    }
    for (x, v) in [(0x2540u32, 0u32), (0x25B0, 0x3F00_0000), (0x2620, 0x3F80_0000)] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, v);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x3294);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    if n(fx) == 0x35 {
        node(fx, 0x32C0, sp + 0x80);
        quadtrail::push(fx, D + 0x1E10, sp + 0x80);
        node(fx, 0x32BC, sp + 0x80);
        quadtrail::push(fx, D + 0x1E10, sp + 0x80);
    }
    node(fx, 0x32B4, sp + 0x90);
    node(fx, 0x32B8, sp + 0xA0);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x880);
    for k in 0..4u32 {
        let w = fx.mem.read_u32(D + 0x880 + 4 * k);
        fx.mem.write_u32(D + 0x1950 + 4 * k, w);
    }
    fx.midpoint(sp + 0x90, sp + 0xA0, D + 0x1960);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x32A4);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x3B0);
    for k in 0..3u32 {
        let t = fx.target_handle();
        fx.unit_position(t, D + 0x2560 + 0x70 * k);
    }
    node(fx, 0x32B4, D + 0x890);
    node(fx, 0x32B4, D + 0x330);
    if n(fx) < 0x32 {
        node(fx, 0x32B4, D + 0x2C0);
    }
    if n(fx) >= 0x31 {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x2C0);
        let m = fx.mem.i32(D + 0x32A0);
        fx.approach_by(D + 0x2C0, D + 0x32A4, m);
        let m = fx.mem.i32(D + 0x32A0);
        if m != 0 {
            fx.mem.set32(D + 0x32A0, m - 1);
        }
    }
    if n(fx) == 0x31 {
        let t = fx.target_handle();
        fx.unit_position(t, sp + 0x90);
        node(fx, 0x32B4, sp + 0xA0);
        fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x2070);
        for k in 0..4u32 {
            let w = fx.mem.read_u32(sp + 0xA0 + 4 * k);
            fx.mem.write_u32(D + 0x2080 + 4 * k, w);
        }
    }
    // s0 = D, s1 = the OT, s2 = sp + 0xA0, s3 = D + 0x150 (left by the
    // target loop).
    let regs = [Some(D), Some(OT), Some(sp + 0xA0), Some(D + 0x150)];
    fx.xform_world_saving(D, 0x01E0_571C, regs);
    fx.xform_billboard_saving(D + 0x2A0, 0x01E0_5724, regs);
    fx.xform_world_saving(D + 0x870, 0x01E0_572C, regs);
    fx.xform_billboard_saving(D + 0x310, 0x01E0_5734, regs);
    fx.xform_billboard_saving(D + 0x390, 0x01E0_573C, regs);
    fx.xform_world_saving(D + 0x1940, 0x01E0_5744, regs);
    fx.xform_world_saving(D + 0x2060, 0x01E0_574C, regs);
    for k in 0..3u32 {
        fx.xform_world_saving(D + 0x2540 + 0x70 * k, 0x01E0_575C, [Some(D), Some(OT), Some(D + 0x70 * k), Some(k)]);
    }
    if n(fx) >= 0x31 {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0xFC {
        fx.step_down(D + 0x329C, 0x20, 4);
    } else {
        fx.step_up(D + 0x329C, 0x80, 4);
    }
    let light = fx.mem.u8(D + 0x329C);
    fx.set_stage_light(light);
    let tenth = f32::from_bits(0x3DCC_CCCD);
    let step20 = f32::from_bits(0x3D4C_CCCD);
    if n(fx) >= 0x41 {
        for k in 0..3u32 {
            let g = D + 0x2690 + 0x400 * k;
            let x = D + 0x2540 + 0x70 * k;
            fx.ring_draw(g, x);
            fx.step_down(g + 0xB, 0, 8);
            if fx.approach(x, 2.0, tenth) == 0 {
                for o in [8u32, 4, 0] {
                    fx.mem.setf(x + o, tenth);
                }
                for (o, ra) in [(0x10u32, 0x01E0_5844), (0x14, 0x01E0_5860), (0x18, 0x01E0_587C)] {
                    let r = fx.rand_from(D, ra);
                    fx.mem.set32(x + o, r & 0xFFFF);
                }
                if n(fx) < 0xFC {
                    fx.mem.write_u8(g + 0xB, 0x80);
                }
            }
        }
        fx.approach(D + 0x390, 1.5, tenth);
        fx.disc(D + 0x860, D + 0x390);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_58F0)]);
        fx.quads_draw(D + 0x400, D + 0x390);
        if n(fx) >= 0xFC {
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
    let m = n(fx);
    if m >= 0x31 {
        if m == 0x31 {
            fx.mem.write_u8(D + 0x387, 0xFF);
        }
        fx.disc(D + 0x380, D + 0x310);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_59BC)]);
        fx.step_down(D + 0x387, 0, 1);
    }
    if n(fx) >= 0xF {
        fx.disc(D + 0x380, D + 0x310);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_59EC)]);
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
    if n(fx) >= 0x32 {
        fx.sprites(D + 0x8E0, D + 0x870);
        fx.step_down(D + 0x8EB, 0, 8);
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
                    fountain_spawn(fx, p, 0x01E0_5B6C, [0x01E0_5B74, 0x01E0_5B90, 0x01E0_5BB8, 0x01E0_5C10]);
                }
            }
            k += 1;
        }
    }
    if n(fx) >= 0x32 {
        fx.fan_draw(D + 0x20D0, D + 0x2060);
        if n(fx) % 2 == 0 {
            fx.step_down(D + 0x20DB, 0, 1);
        }
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
    }
    quadtrail::draw(fx, TRAIL_DRAW, OT, D + 0x1E10, D);
    trail_fade(fx, D + 0x1E10);
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x3294, m);
    let left = fx.mem.i32(D + 0x3290) - 1;
    fx.mem.set32(D + 0x3290, left);
    if m == 0xFC {
        2
    } else if m == 0x37 {
        1
    } else if (0x41..0xFC).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
