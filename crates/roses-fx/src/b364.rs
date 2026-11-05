//! BMODULE b364 (card 338), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E041D0, data area 0x1ED0000) [F] (helper C5, 31
//! Oct 2026). At the attacker's node 7: a full-window tint until update
//! 340 while the stage light dims to 0x20; a particle cloud (+0xEB0)
//! until 105; from 45 a tube (+0x2D60, `tube`) round the target aimed at
//! node 7, flickering, shrinking from 122 and fading from 184; from 105 a
//! sphere of the target's radius round it (+0x14D0, `blib::sphere`),
//! flickering and growing to 1 from 181, the target shrinking to 0 from
//! 221 (its scale = the sphere's; restored from the data area's first
//! vec on 340); from 231 a flickering point-set strip (+0x1070 / look
//! +0x1110) grows at the target and climbs from 272 toward node 7 (30
//! steps, swaying by 5 sin), then a cloud of squares (+0x1170) spreads
//! there; a point-set strip (+0x540 / look +0x5E0), rings (+0x640 /
//! +0xA40, from 262) and a quad burst (+0xE0, from 195) at node 7 fade
//! after 340. Returns 1 on update 176, 6 on 189..235, 2 on 340, 8 after
//! 356.
//!
//! Library: `colours4` (0x1E00080), the frame quad (0x1E00120),
//! `pointquad::setup` / `draw` / `squares` (0x1E00240 / 0x1E00580 /
//! 0x1E01160), `ring_setup` / `ring_draw` (0x1E01740 / 0x1E01A00),
//! `particle_quads` (0x1E01EF0), `sphere::setup` / `draw` (0x1E02A80 /
//! 0x1E02B70), `quads_setup` / `quads_draw` (0x1E03240 / 0x1E033A0),
//! `tube_setup` / `tube_draw` (0x1E039B0 / 0x1E03AB0).
use crate::b028::{cloud16_spawn, square_points};
use crate::b107::{approach_down, target_radius};
use crate::b436::{fstep_down, ring_group};
use crate::b628::halves;
use crate::blib::{pointquad, sphere};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SETUP: u32 = 0x01E0_0240;
/// The quad corner template (rodata, 0x1E03240's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_5280;
const TENTH: f32 = f32::from_bits(0x3DCC_CCCD);

/// The entry 0x1E041D0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x70);
    ret
}

/// A point set's look `g`: cell (176, 32, 32, 32) of a 256 x 256 sheet
/// from (176, 32), looping frame 0.
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
    fx.bzero(D, 0x3300);
    for x in [0u32, 0x70, 0x1460, 0xE40, 0x1000, 0x2CF0] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x32E0u32, 6, 1), (0x32E4, 5, 0), (0x32E8, 7, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x32D4, 0x164);
    fx.mem.set32(D + 0x32D8, 1);
    fx.mem.set32(D + 0x32DC, param);
    fx.mem.set32(D + 0x32D0, 0);
    fx.mem.set32(D + 0x32F8, 0x1E);
    fx.mem.set32(D + 0x32FC, 0);
    let r = target_radius(fx);
    fx.mem.setf(D + 0x32F0, r);
    fx.mem.set32(D + 0x32EC, 0x80);
    for (k, x) in [(0x32F4u32, 0u8), (0x32F5, 0), (0x32F7, 0x80), (0x32F6, 0x80)] {
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
    // The quad burst (+0xE0): size 1.5.
    let g = D + 0xE0;
    fx.mem.write_u32(g + 4, 0x8_0001);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    for (k, x) in [(0xAu32, 0xD0u8), (9, 0xA0), (8, 0x70), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, x);
    }
    let t = tex(fx);
    fx.quads_setup(g, t, 1.5, QUAD_TEMPLATE);
    // Two rings (+0x640: radii 12 / 11.2; +0xA40: 12 / 12.8).
    for (g, a, outer) in [(0x640u32, 0x20u8, 0x4133_3333u32), (0xA40, 0x30, 0x414C_CCCD)] {
        ring_group(fx, D + g, 0x2008_0000, 0x40, [0x70, 0xA0, 0xD0, a]);
        let t = tex(fx);
        fx.ring_setup(D + g, t, 12.0, f32::from_bits(outer), 0.0, 0.0);
    }
    // The point sets: +0x540 (look +0x5E0, size 16), +0x1070 (look
    // +0x1110, size 50).
    look(fx, D + 0x5E0, 0x4_0001, [0x70, 0xA0, 0xD0, 0x60]);
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
    fx.mem.write_u32(g + 4, 0x8_1083);
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
    fx.texture5(sp + 0x60, 1);
    // The sphere (+0x14D0) of the target's radius.
    let g = D + 0x14D0;
    fx.mem.write_u32(g + 4, 3);
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x80), (0x16, 0x80)]);
    for (k, x) in [(0xAu32, 0xB0u8), (9, 0xA0), (8, 0x70), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    let r = fx.mem.f(D + 0x32F0);
    let t = tex(fx);
    sphere::setup(fx, g, t, r);
    // The tube (+0x2D60): 48 rings at +0x2EC0, radius 5, height 5.
    let g = D + 0x2D60;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    for (k, x) in [(6u32, 0x40u8), (5, 0x40), (4, 0x80), (7, 0xA0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 8, 0x8000);
    fx.mem.write_u32(g + 0xC, 0x40A0_0000);
    halves(fx, g, &[(0x140, 0x20), (0x142, 0)]);
    fx.mem.write_u32(g + 0x144, 0x40A0_0000);
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x80), (0x16, 0x80), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x80)]);
    fx.mem.write_u32(g + 0x148, D + 0x2EC0);
    halves(fx, g, &[(0x14C, 0x30), (0x14E, 0)]);
    fx.mem.write_u32(g + 0x150, 0x4000_0000);
    let t = tex(fx);
    fx.tube_setup(g, t);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x14C) {
        let n = fx.mem.i16(g + 0x14C);
        let a = i32::from(fx.mem.read_u8(g + 7));
        let p = fx.mem.read_u32(g + 0x148) + 16 * k as u32;
        let (q, _) = fx.div(a, n);
        let m = fx.mult(k, q);
        fx.mem.write_u8(p + 0xC, a.wrapping_sub(m) as u8);
        k += 1;
    }
    let t = fx.target_handle();
    fx.unit_point(2, t, D + 0x20);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x90);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x32C0);
    for x in [0x70u32, 0x1000, 0x1460] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, 0x3DCC_CCCD);
        }
    }
    0
}

fn node(fx: &mut Fx, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x32E8);
    fx.node_position(h, s, out);
}

/// Three random bytes & 0x7F (rand from the entry, s0 = D) into the
/// colour at `c` (R, G, B: c + 2, c + 1, c).
fn flicker(fx: &mut Fx, c: u32, ras: [u32; 3]) {
    for (k, ra) in ras.into_iter().enumerate() {
        let v = (fx.rand_from(D, ra) % 0x80) as u8;
        fx.mem.write_u8(c + 2 - k as u32, v);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x32D8);
    node(fx, D + 0xE60);
    node(fx, D + 0x32C0);
    node(fx, D + 0x90);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x1480);
    if (0xEC..0x106).contains(&n(fx)) {
        let t = fx.target_handle();
        fx.unit_point(1, t, D + 0x1020);
    }
    node(fx, D + 0x2D10);
    fx.look_angles(D + 0x1480, D + 0x2D10, D + 0x2D00);
    // s0 = D, s1 = the OT, s2 the param (-1, 64 bits), s3 the caller's.
    let regs = [Some(D), Some(OT), None, None];
    for (x, ra, bb) in [(0u32, 0x01E0_4A54u32, false), (0x70, 0x01E0_4A5C, true), (0xE40, 0x01E0_4A64, true), (0x1460, 0x01E0_4A6C, false), (0x1000, 0x01E0_4A74, true), (0x2CF0, 0x01E0_4A7C, false)] {
        if bb {
            fx.xform_billboard_saving(D + x, ra, regs);
            fx.saved(sp - 0xF0, &[(0x40, u64::MAX)]);
        } else {
            fx.xform_world_saving(D + x, ra, regs);
            fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
        }
    }
    if n(fx) < 0x154 {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0x154 {
        fx.step_down(D + 0x32EC, 0x20, 4);
    } else {
        fx.step_up(D + 0x32EC, 0x80, 4);
    }
    let light = i32::from(fx.mem.read_u8(D + 0x32EC));
    fx.set_stage_light(light);
    if n(fx) < 0x6A {
        for k in [0x48u32, 0x44, 0x40] {
            fx.mem.write_u32(sp + k, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0xE40, D + 0xEB0, sp + 0x40);
        if n(fx) >= 0x5A {
            fx.step_down(D + 0xEBB, 0, 0xA);
        }
    }
    if n(fx) >= 0x69 {
        sphere::draw(fx, OT, D + 0x14D0, D + 0x1460);
        if n(fx) >= 0xB5 {
            flicker(fx, D + 0x14D8, [0x01E0_4B68, 0x01E0_4B84, 0x01E0_4BA0]);
            if n(fx) < 0xDC {
                fx.approach(D + 0x1460, 1.0, TENTH);
            }
            fx.step_up(D + 0x14DB, 0x80, 8);
            let a = fx.mem.i32(D + 0x1474).wrapping_add(0x800);
            fx.mem.set32(D + 0x1474, a);
        }
        if (0xDD..0x154).contains(&n(fx)) {
            let t = fx.target_handle();
            fx.unit_set_scale(t, D + 0x1460);
            approach_down(fx, D + 0x1460, 0.0, f32::from_bits(0x3D4C_CCCD));
        }
        if !bit(fx.mem.read_u32(D + 0x32D0), 0x1F) {
            if n(fx) >= 0xE7 {
                flicker(fx, D + 0x1144, [0x01E0_4C74, 0x01E0_4C90, 0x01E0_4CAC]);
                pointquad::draw(fx, OT, D + 0x1110, D + 0x1070, D + 0x1000);
                if n(fx) < 0x110 {
                    fx.approach(D + 0x1000, 1.0, TENTH);
                }
            }
            if n(fx) >= 0x110 {
                let left = fx.mem.i32(D + 0x32F8);
                fx.approach_by(D + 0x1020, D + 0x32C0, left);
                approach_down(fx, D + 0x1000, f32::from_bits(0x3E4C_CCCD), f32::from_bits(0x3CF5_C28F));
                let left = fx.mem.i32(D + 0x32F8);
                if left != 0 {
                    fx.mem.set32(D + 0x32F8, left - 1);
                    let a = fx.mem.i32(D + 0x32FC).wrapping_add(0x444);
                    fx.mem.set32(D + 0x32FC, a);
                    let s = fx.tsin(a);
                    let y = ee::add(fx.mem.f(D + 0x1024), ee::mul(5.0, s));
                    fx.mem.setf(D + 0x1024, y);
                } else {
                    for k in [8u32, 4, 0] {
                        fx.mem.write_u32(D + 0x1000 + k, 0x3F80_0000);
                    }
                    let f = fx.mem.read_u32(D + 0x32D0) | 0x8000_0000;
                    fx.mem.write_u32(D + 0x32D0, f);
                }
            }
        } else {
            flicker(fx, D + 0x1178, [0x01E0_4DB4, 0x01E0_4DD0, 0x01E0_4DEC]);
            for k in [0x58u32, 0x54, 0x50] {
                fx.mem.write_u32(sp + k, 0x3F82_8F5C);
            }
            pointquad::squares(fx, OT, D + 0x1170, D + 0x1000, sp + 0x50);
            fx.step_down(D + 0x117B, 0, 8);
        }
        if n(fx) < 0x154 {
            pointquad::draw(fx, OT, D + 0x5E0, D + 0x540, D + 0x70);
            if n(fx) >= 0x106 {
                fx.ring_draw(D + 0x640, D + 0x70);
                fx.ring_draw(D + 0xA40, D + 0x70);
            }
        }
        if n(fx) >= 0xC3 {
            fx.quads_draw(D + 0xE0, D + 0x70);
        }
        if n(fx) >= 0x155 {
            for (k, s) in [(0xEBu32, 2), (0x64B, 2), (0xA4B, 3), (0x617, 6)] {
                fx.step_down(D + k, 0, s);
            }
        } else {
            fx.approach(D + 0x70, 1.0, TENTH);
        }
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0x140 + 16 * k + 4 * c;
                let x = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, x);
            }
        }
        for (k, ra) in [(2u32, 0x01E0_4F68u32), (1, 0x01E0_4F90), (0, 0x01E0_4FB8)] {
            let v = (fx.rand_from(D, ra) % 0x80) as u8;
            for c in [0xE8u32, 0xA48, 0x648, 0x614] {
                fx.mem.write_u8(D + c + k, v);
            }
        }
    }
    if n(fx) >= 0x2D {
        let r = fx.rand_from(D, 0x01E0_4FF4);
        let (_, h) = fx.div(r, 0x60);
        fx.mem.write_u8(D + 0x2D64, (h + 0x60) as u8);
        fx.tube_draw(OT, D + 0x2D60, D + 0x2CF0);
        if n(fx) >= 0x7A {
            fstep_down(fx, D + 0x2EB0, 0.0, f32::from_bits(0x3D4C_CCCD));
        }
        if n(fx) >= 0xB8 {
            fx.step_down(D + 0x2D67, 0, 0xA);
        }
        let s = fx.mem.i16(D + 0x2EAE) + 1;
        fx.mem.set16(D + 0x2EAE, s);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x32D8, m);
    let left = fx.mem.i32(D + 0x32D4) - 1;
    fx.mem.set32(D + 0x32D4, left);
    if m == 0x154 {
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
