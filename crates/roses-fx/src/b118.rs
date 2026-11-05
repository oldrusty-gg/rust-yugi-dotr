//! BMODULE b118 (card 308), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03A20, data area 0x1ED0000) [F] (helper C11, 31
//! Oct 2026): b364's program (C5) without the tube, stage light and
//! second texture on a smaller layout (counters at D+0x1B00..), its
//! attacker-side groups at the midpoint of nodes 5 / 4 (0x1036E0). A particle cloud (+0xEB0) on updates
//! 54..105; from 105 a sphere of the target's radius round it (+0x14D0,
//! 8 x 16 points, `blib::sphere::setup_b` / `draw_b`), flickering and
//! brightening to 0x20 from 181, the target shrinking to 0 from 221 (its
//! scale = the sphere's; restored from the data area's first vec on 423);
//! from 231 a flickering point-set strip (+0x1070 / look +0x1110) grows
//! at the target and climbs from 272 toward the attacker (30 steps,
//! swaying by 5 sin), then a cloud of squares (+0x1170) spreads there; a
//! point-set strip (+0x540 / look +0x5E0) and rings (+0x640 / +0xA40, from
//! 262) at the attacker and a quad burst (+0xE0) fade from 335, the
//! burst blown up white from 423 while a full-window tint (D+0x1B1C)
//! fades. Returns 1 on update 176, 6 on 189..235, 2 on 423, 8 after 506.
//!
//! Library: `colours4` (0x1E00080), the tint quad (0x1E00120),
//! `pointquad::setup` / `draw` / `squares` (0x1E00280 / 0x1E005C0 /
//! 0x1E011A0), `ring_setup` / `ring_draw` (0x1E01780 / 0x1E01A40),
//! `particle_quads` (0x1E01F30), `sphere::setup_b` / `draw_b` (0x1E02AC0
//! / 0x1E02BB0), `quads_setup` / `quads_draw` (0x1E032B0 / 0x1E03410).
use crate::b028::{cloud16_spawn, square_points};
use crate::b107::{approach_down, target_radius};
use crate::b436::ring_group;
use crate::b628::halves;
use crate::blib::{pointquad, sphere};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SETUP: u32 = 0x01E0_0280;
/// The quad corner template (rodata, 0x1E032B0's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_4900;
const TENTH: f32 = f32::from_bits(0x3DCC_CCCD);
const FLAGS: u32 = D + 0x1B00;
const LEFT: u32 = D + 0x1B04;
const N: u32 = D + 0x1B08;
const CLIMB: u32 = D + 0x1B20;
const SWAY: u32 = D + 0x1B24;
const TINT: u32 = D + 0x1B1C;

/// The entry 0x1E03A20.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x70);
    ret
}

/// A point set's look `g`: cell (176, 32, 32, 32) of a 256 x 256 sheet
/// from (176, 32), looping frame 0 (b364's).
fn look(fx: &mut Fx, g: u32, flags: u32, bgra: [u8; 4]) {
    fx.mem.write_u32(g + 0x38, flags);
    halves(fx, g, &[(0, 0xB0), (2, 0x20), (4, 0x20), (6, 0x20), (8, 0xB0), (0xA, 0x20), (0xC, 0x100), (0xE, 0x100)]);
    fx.mem.write_u32(g + 0x30, 0x3F80_0000);
    for (k, x) in [(0x36u32, bgra[2]), (0x35, bgra[1]), (0x34, bgra[0]), (0x37, bgra[3])] {
        fx.mem.write_u8(g + k, x);
    }
    halves(fx, g, &[(0x48, 0), (0x4A, 0), (0x4C, 0), (0x4E, 1), (0x50, 0), (0x52, 0x3E7)]);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1B30);
    for x in [0u32, 0x70, 0x1460, 0xE40, 0x1000] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x1B10u32, 5, 1), (0x1B14, 4, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(LEFT, 0x1FA);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x1B0C, param);
    fx.mem.set32(FLAGS, 0);
    fx.mem.set32(CLIMB, 0x1E);
    fx.mem.set32(SWAY, 0);
    let r = target_radius(fx);
    fx.mem.setf(D + 0x1B18, r);
    for (k, x) in [(0x1B1Cu32, 0u8), (0x1B1D, 0), (0x1B1F, 0x80), (0x1B1E, 0x80)] {
        fx.mem.write_u8(D + k, x);
    }
    fx.texture4(sp + 0x60, 0xF);
    let tex = |fx: &mut Fx| fx.mem.read_u32(sp + 0x60);
    // The squares (+0x1170): 60 points, half size 1.
    let g = D + 0x1170;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 0xC, 0x3F80_0000);
    square_points(fx, g);
    fx.mem.write_u32(g + 4, 0x2008_0081);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x40), (0x14, 0x20), (0x16, 0x20)]);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    // The sphere (+0x14D0) of the target's radius, cell (128, 128, 128, 64).
    let g = D + 0x14D0;
    fx.mem.write_u32(g + 4, 3);
    halves(fx, g, &[(0x10, 0x80), (0x12, 0x80), (0x14, 0x80), (0x16, 0x40)]);
    for (k, x) in [(0xAu32, 0xD0u8), (9, 0xA0), (8, 0x70), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    let r = fx.mem.f(D + 0x1B18);
    let t = tex(fx);
    sphere::setup_b(fx, g, t, r);
    // The quad burst (+0xE0): size 1.
    let g = D + 0xE0;
    fx.mem.write_u32(g + 4, 0x8_0001);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    for (k, x) in [(0xAu32, 0xD0u8), (9, 0xA0), (8, 0x70), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, x);
    }
    let t = tex(fx);
    fx.quads_setup(g, t, 1.0, QUAD_TEMPLATE);
    // Two rings (+0x640: radii 12 / 11.2; +0xA40: 12 / 12.8).
    for (g, a, outer) in [(0x640u32, 0x20u8, 0x4133_3333u32), (0xA40, 0x30, 0x414C_CCCD)] {
        ring_group(fx, D + g, 0x2008_0000, 0x40, [0x70, 0xA0, 0xD0, a]);
        let t = tex(fx);
        fx.ring_setup(D + g, t, 12.0, f32::from_bits(outer), 0.0, 0.0);
    }
    // The point sets: +0x540 (look +0x5E0, size 16), +0x1070 (look
    // +0x1110, size 50).
    look(fx, D + 0x5E0, 0x8_0001, [0x70, 0xA0, 0xD0, 0x60]);
    fx.mem.write_u32(D + 0x548, 1);
    let t = tex(fx);
    pointquad::setup(fx, SETUP, D + 0x5E0, D + 0x540, t, 16.0);
    look(fx, D + 0x1110, 0x8_0001, [0x80, 0x80, 0x80, 0xFF]);
    fx.mem.write_u32(D + 0x1078, 1);
    let t = tex(fx);
    pointquad::setup(fx, SETUP, D + 0x1110, D + 0x1070, t, 50.0);
    fx.texture5(sp + 0x60, 0);
    // The particle cloud (+0xEB0): 16 particles, half size 6, 8 frames.
    let g = D + 0xEB0;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x8_0483);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0), (0x2A, 0), (0x24, 0x20), (0x26, 0x20), (0x2C, 0x80), (0x2E, 0x40)]);
    fx.mem.write_u32(g + 0xC, 0x40C0_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 3), (0x13C, 2), (0x140, 8), (0x136, 0xA), (0x138, 0xA)]);
    for (k, x) in [(0xAu32, 0x60u8), (9, 0x70), (8, 0x80), (0xB, 0xA0)] {
        fx.mem.write_u8(g + k, x);
    }
    cloud16_spawn(fx, g);
    let t = fx.target_handle();
    fx.unit_point(2, t, D + 0x20);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x90);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x1AF0);
    for x in [0x70u32, 0x1000] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, 0x3DCC_CCCD);
        }
    }
    0
}

/// Three rands & 0x7F (signed; from the entry, s0 = D) into the colour at
/// `c` (R, G, B: c + 2, c + 1, c).
fn flicker(fx: &mut Fx, c: u32, ras: [u32; 3]) {
    for (k, ra) in ras.into_iter().enumerate() {
        let v = (fx.rand_from(D, ra) % 0x80) as u8;
        fx.mem.write_u8(c + 2 - k as u32, v);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0xE60);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x1AF0);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x1480);
    if (0xEC..0x106).contains(&n(fx)) {
        let t = fx.target_handle();
        fx.unit_point(1, t, D + 0x1020);
    }
    for (at, out) in [(0x1B10u32, sp + 0x40), (0x1B14, sp + 0x50)] {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    }
    fx.midpoint(sp + 0x50, sp + 0x40, D + 0x90);
    // s0 = D, s1 = the OT, s2 the param (-1, 64 bits) (as b364).
    let regs = [Some(D), Some(OT), None, None];
    for (x, ra, bb) in [(0u32, 0x01E0_41A0u32, false), (0x70, 0x01E0_41A8, true), (0xE40, 0x01E0_41B0, true), (0x1460, 0x01E0_41B8, false), (0x1000, 0x01E0_41C0, true)] {
        if bb {
            fx.xform_billboard_saving(D + x, ra, regs);
            fx.saved(sp - 0xF0, &[(0x40, u64::MAX)]);
        } else {
            fx.xform_world_saving(D + x, ra, regs);
            fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
        }
    }
    if (0x36..0x6A).contains(&n(fx)) {
        for k in [0x48u32, 0x44, 0x40] {
            fx.mem.write_u32(sp + k, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0xE40, D + 0xEB0, sp + 0x40);
    }
    if n(fx) >= 0x69 {
        sphere::draw_b(fx, OT, D + 0x14D0, D + 0x1460);
        if n(fx) >= 0xB5 {
            flicker(fx, D + 0x14D8, [0x01E0_423C, 0x01E0_4258, 0x01E0_4274]);
            fx.step_up(D + 0x14DB, 0x20, 4);
            let a = fx.mem.i32(D + 0x1474).wrapping_add(0x800);
            fx.mem.set32(D + 0x1474, a);
        }
        if (0xDD..0x1A7).contains(&n(fx)) {
            let t = fx.target_handle();
            fx.unit_set_scale(t, D + 0x1460);
            approach_down(fx, D + 0x1460, 0.0, f32::from_bits(0x3D4C_CCCD));
        }
        if !bit(fx.mem.read_u32(FLAGS), 0x1F) {
            if n(fx) >= 0xE7 {
                flicker(fx, D + 0x1144, [0x01E0_431C, 0x01E0_4338, 0x01E0_4354]);
                pointquad::draw(fx, OT, D + 0x1110, D + 0x1070, D + 0x1000);
                if n(fx) < 0x110 {
                    fx.approach(D + 0x1000, 1.0, TENTH);
                }
            }
            if n(fx) >= 0x110 {
                let left = fx.mem.i32(CLIMB);
                fx.approach_by(D + 0x1020, D + 0x1AF0, left);
                approach_down(fx, D + 0x1000, f32::from_bits(0x3E4C_CCCD), f32::from_bits(0x3CF5_C28F));
                let left = fx.mem.i32(CLIMB);
                if left != 0 {
                    fx.mem.set32(CLIMB, left - 1);
                    let a = fx.mem.i32(SWAY).wrapping_add(0x444);
                    fx.mem.set32(SWAY, a);
                    let s = fx.tsin(a);
                    let y = ee::add(fx.mem.f(D + 0x1024), ee::mul(5.0, s));
                    fx.mem.setf(D + 0x1024, y);
                } else {
                    for k in [8u32, 4, 0] {
                        fx.mem.write_u32(D + 0x1000 + k, 0x3F80_0000);
                    }
                    let f = fx.mem.read_u32(FLAGS) | 0x8000_0000;
                    fx.mem.write_u32(FLAGS, f);
                }
            }
        } else {
            flicker(fx, D + 0x1178, [0x01E0_445C, 0x01E0_4478, 0x01E0_4494]);
            for k in [0x58u32, 0x54, 0x50] {
                fx.mem.write_u32(sp + k, 0x3F82_8F5C);
            }
            pointquad::squares(fx, OT, D + 0x1170, D + 0x1000, sp + 0x50);
            fx.step_down(D + 0x117B, 0, 8);
        }
        if n(fx) < 0x1A7 {
            pointquad::draw(fx, OT, D + 0x5E0, D + 0x540, D + 0x70);
            if n(fx) >= 0x106 {
                fx.ring_draw(D + 0x640, D + 0x70);
                fx.ring_draw(D + 0xA40, D + 0x70);
            }
        }
        fx.quads_draw(D + 0xE0, D + 0x70);
        if n(fx) < 0x14F {
            fx.approach(D + 0x70, 1.0, TENTH);
        } else if n(fx) < 0x1A7 {
            for (k, s) in [(0xEBu32, 0x10), (0x64B, 4), (0xA4B, 5), (0x617, 0xA)] {
                fx.step_down(D + k, 0, s);
            }
        }
        if n(fx) == 0x1A7 {
            for k in [8u32, 4, 0] {
                fx.mem.write_u32(D + 0x70 + k, 0x3F80_0000);
            }
            let f = fx.mem.read_u32(D + 0xE4) & 0xFFF7_FFFF;
            fx.mem.write_u32(D + 0xE4, f);
            for k in [0xE8u32, 0xE9, 0xEA] {
                fx.mem.write_u8(D + k, 0x80);
            }
            fx.mem.write_u8(D + 0xEB, 0xFF);
        }
        if n(fx) >= 0x1A7 {
            fx.approach(D + 0x70, 10.0, f32::from_bits(0x3D4C_CCCD));
            fx.step_down(D + 0xEB, 0, 0x20);
        } else {
            for k in 0..64u32 {
                for c in 0..3u32 {
                    let a = D + 0x140 + 16 * k + 4 * c;
                    let x = fx.mem.i32(a).wrapping_add(0x80);
                    fx.mem.set32(a, x);
                }
            }
            for (k, ra) in [(2u32, 0x01E0_469Cu32), (1, 0x01E0_46C4), (0, 0x01E0_46EC)] {
                let v = (fx.rand_from(D, ra) % 0x80) as u8;
                for c in [0xE8u32, 0xA48, 0x648, 0x614] {
                    fx.mem.write_u8(D + c + k, v);
                }
            }
        }
    }
    if n(fx) >= 0x1A8 {
        crate::b262::tint(fx, TINT);
        fx.step_down(D + 0x1B1F, 0, 0x10);
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0x1A7 {
        let t = fx.target_handle();
        fx.unit_set_scale(t, D);
        2
    } else if m == 0xB0 {
        1
    } else if (0xBD..0xEC).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
