//! BMODULE b644 (card 524), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04730, data area 0x1ED0000) [F] (helper C5, 31
//! Oct 2026): b556's library, a wider scene. The stage light dims to 0x20
//! until update 280 and the other light (0x1F7EF0) until 68. At the
//! attacker's nodes 0x3B / 0x3C / 0x3D a disc and plate pair each
//! (+0x3FE0.. / +0x3F20..) fade in until 100 and out until 125 (then
//! held, the discs at size 5) until 181. A point 28 times node 0x3E's
//! offset from node 0x3D (+0x4010, height 10) gathers glowing lines
//! (`blib::glow`) from the three nodes to node 0x3E (+0xB0..) and from
//! node 0x3E to it (+0x70) from update 138, fading from 256; from 166 a
//! sprite fountain (+0x2D70) rises round it, a spark burst (+0x390) is
//! thrown out again on a flat ring and two rings of blades (+0xC30 /
//! +0x1CD0, `blib::blade`) turn at the attacker. Returns 1 on update 156,
//! 6 on 166..279, 2 on 280, 8 after 300.
//!
//! Library: as b556 (the same addresses).
use crate::b107::target_radius;
use crate::b556::{blades, blades_fade, colour, sprite_place};
use crate::b628::halves;
use crate::blib::{self, blade, glow, plate};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PLATES: u32 = 0x01E0_2BE0;
/// The light byte 0x1F7EF0 sets.
const LIGHT2: u32 = 0x002D_F5B0;

/// The entry 0x1E04730.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xD0);
    ret
}

/// A glowing line: `pieces` pieces of width `w`, cell (16, 0, 16, 32).
fn line(fx: &mut Fx, g: u32, tex: u32, w: u32, pieces: i32, drawn: i32) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x400_0000);
    fx.mem.write_u32(g + 0xC, w);
    halves(fx, g, &[(0x10, 0x10), (0x12, 0), (0x14, 0x10), (0x16, 0x20)]);
    colour(fx, g, [0x80; 4]);
    halves(fx, g, &[(0x28, pieces), (0x2C, drawn), (0x2A, 0x28)]);
    fx.mem.write_u32(g + 0x20, 0);
    fx.mem.write_u32(g + 0x24, 0);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4050);
    for x in [0u32, 0x2B0, 0x320, 0xBC0, 0x3DD0, 0x3E40, 0x3EB0] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x402Cu32, 0x3B, 1), (0x4030, 0x3C, 0), (0x4034, 0x3D, 0), (0x4038, 0x3E, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x4020, 0x12C);
    fx.mem.set32(D + 0x4024, 1);
    fx.mem.set32(D + 0x4028, param);
    let r = target_radius(fx);
    fx.mem.setf(D + 0x4044, r);
    fx.mem.set32(D + 0x403C, 0x80);
    fx.mem.set32(D + 0x4040, 0x80);
    fx.texture5(sp + 0xC0, 0);
    let tex = |fx: &mut Fx| fx.mem.read_u32(sp + 0xC0);
    let t = tex(fx);
    line(fx, D + 0x70, t, 0x4220_0000, 0x1C, -3);
    for k in 0..3u32 {
        let t = tex(fx);
        line(fx, D + 0xB0 + 0x40 * k, t, 0x41A0_0000, 4, 0);
    }
    // The spark burst (+0x390): 128 sparks at +0x3C0, width 0.5, length 5.
    let g = D + 0x390;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x2000_0003);
    colour(fx, g, [0x80; 4]);
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x40A0_0000);
    halves(fx, g, &[(0x10, 0x20), (0x12, 0x20), (0x14, 0x20), (0x16, 0x20), (0x18, 0x20), (0x1A, 0x20), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, D + 0x3C0);
    fx.mem.write_u32(g + 0x28, 0x40A0_0000);
    fx.mem.set16(g + 0x2C, 0x80);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 4.0);
    // Plates (+0x3F20 + 0x40 k) and discs (+0x3FE0 + 0x10 k).
    for k in 0..3u32 {
        let g = D + 0x3F20 + 0x40 * k;
        let t = tex(fx);
        fx.mem.write_u32(g, t);
        fx.mem.write_u32(g + 4, 0x8_0000);
        colour(fx, g, [0x80, 0x80, 0x80, 0]);
        for (o, x) in [(0xCu32, 0x3E80_0000u32), (0x10, 0x4040_0000), (0x14, 0x3E80_0000), (0x18, 0x3F80_0000)] {
            fx.mem.write_u32(g + o, x);
        }
        halves(fx, g, &[(0x20, 0x20), (0x22, 0x20), (0x24, 0x20), (0x26, 0x20)]);
        for o in [0x30u32, 0x34, 0x38] {
            fx.mem.write_u32(g + o, 0);
        }
        let g = D + 0x3FE0 + 0x10 * k;
        fx.mem.write_u32(g, 0x8_0000);
        for (o, x) in [(6u32, 0xD0u8), (5, 0x70), (4, 0xA0), (7, 0)] {
            fx.mem.write_u8(g + o, x);
        }
        fx.mem.write_u32(g + 8, 0x4000_0000);
        halves(fx, g, &[(0xC, 0x24), (0xE, 0)]);
    }
    fx.texture4(sp + 0xC0, 0xF);
    // The sprite fountain (+0x2D70): 256 sprites at +0x2DD0, size 30.
    let g = D + 0x2D70;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x4800_0003);
    colour(fx, g, [0x80, 0x80, 0x80, 0x10]);
    fx.mem.write_u32(g + 0xC, 0x41F0_0000);
    for o in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + o, 0);
    }
    halves(fx, g, &[(0x30, 0x80), (0x32, 0xC0), (0x34, 0x20), (0x36, 0x20), (0x38, 0x80), (0x3A, 0xC0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x2DD0);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4120_0000);
    fx.mem.write_u32(g + 0x50, 0x40A0_0000);
    halves(fx, g, &[(0x5A, 0x100), (0x5C, 8), (0x5E, 1)]);
    fx.sprites_spawn(g, 30.0);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = D + 0x2DD0 + 16 * k as u32;
        let r = fx.rand_from(D + 16 * k as u32, 0x01E0_4B2C);
        fx.mem.set16(p + 0xE, -(r % 16));
        let a = fx.mem.read_u8(g + 0xB);
        fx.mem.write_u8(p + 0xC, a);
        let r = fx.rand_from(D + 16 * k as u32, 0x01E0_4B54);
        fx.mem.write_u8(p + 0xD, (r % 8) as u8);
        k += 1;
    }
    fx.texture5(sp + 0xC0, 1);
    let t = tex(fx);
    blades(fx, D + 0xC30, t, 0x4120_0000, 0, [0xA0, 0x70, 0xD0, 0x80], 3);
    let t = tex(fx);
    blades(fx, D + 0x1CD0, t, 0, 0x4140_0000, [0xA0, 0x70, 0xD0, 0x80], 3);
    0
}

fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x4024);
    for (k, at) in [0x402Cu32, 0x4030, 0x4034].into_iter().enumerate() {
        node(fx, at, sp + 0xA0 + 0x10 * k as u32);
    }
    for k in 0..3u32 {
        for c in 0..4u32 {
            let w = fx.mem.read_u32(sp + 0xA0 + 0x10 * k + 4 * c);
            fx.mem.write_u32(D + 0x3DF0 + 0x70 * k + 4 * c, w);
        }
    }
    node(fx, 0x4038, sp + 0xD0);
    node(fx, 0x4038, D + 0x4010);
    // 0x103660: +0x4010 -= node 0x3D; x, z times 28, plus node 0x3E;
    // height 10.
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(D + 0x4010 + 4 * c), fx.mem.f(sp + 0xC0 + 4 * c));
        fx.mem.setf(D + 0x4010 + 4 * c, v);
    }
    let s = f32::from_bits(0x41E0_0000);
    for c in [0u32, 8] {
        let v = ee::mul(fx.mem.f(D + 0x4010 + c), s);
        fx.mem.setf(D + 0x4010 + c, v);
    }
    for c in [0u32, 8] {
        let v = ee::add(fx.mem.f(D + 0x4010 + c), fx.mem.f(sp + 0xD0 + c));
        fx.mem.setf(D + 0x4010 + c, v);
    }
    fx.mem.write_u32(D + 0x4014, 0x4120_0000);
    for c in 0..4u32 {
        let w = fx.mem.read_u32(D + 0x4010 + 4 * c);
        fx.mem.write_u32(D + 0x340 + 4 * c, w);
    }
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x2D0);
    let h = fx.self_handle();
    fx.unit_position(h, D + 0xBE0);
    fx.mem.write_u32(D + 0xBE4, 0);
    if ee_lt(fx.mem.f(D + 0xBE8), 0.0) {
        fx.mem.write_u32(D + 0xBD4, 0x8000);
    }
    // s0 = sp + 0xD0, s1 = sp + 0xA0, s2 = D, s3 = the OT; the billboards
    // in a loop (s4 its pointer, s5 its count).
    for (x, ra) in [(0u32, 0x01E0_4E58u32), (0x2B0, 0x01E0_4E60), (0x320, 0x01E0_4E68), (0xBC0, 0x01E0_4E70)] {
        fx.xform_world_saving(D + x, ra, [Some(sp + 0xD0), Some(sp + 0xA0), Some(D), Some(OT)]);
    }
    for k in 0..3u32 {
        fx.xform_billboard_saving(D + 0x3DD0 + 0x70 * k, 0x01E0_4E80, [Some(sp + 0xD0), Some(sp + 0xA0), Some(D), Some(OT)]);
    }
    if n(fx) >= 0x10F {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0x118 {
        fx.step_down(D + 0x403C, 0x20, 4);
    } else {
        fx.step_up(D + 0x403C, 0x80, 4);
    }
    let l = i32::from(fx.mem.read_u8(D + 0x403C));
    fx.set_stage_light(l);
    if n(fx) < 0x44 {
        fx.step_down(D + 0x4040, 0x20, 4);
    } else {
        fx.step_up(D + 0x4040, 0x80, 4);
    }
    let l = fx.mem.read_u8(D + 0x4040);
    fx.mem.write_u8(LIGHT2, l);
    if (0x28..0xB5).contains(&n(fx)) {
        for k in 0..3u32 {
            let (disc, pl, x) = (D + 0x3FE0 + 0x10 * k, D + 0x3F20 + 0x40 * k, D + 0x3DD0 + 0x70 * k);
            if n(fx) == 0x7D {
                fx.mem.write_u32(disc + 8, 0x40A0_0000);
            }
            fx.disc(disc, x);
            plate::plates(fx, PLATES, OT, pl, x);
            let m = n(fx);
            if m < 0x64 {
                fx.step_up(disc + 7, 0x80, 8);
                fx.step_up(pl + 0xB, 0x80, 8);
            } else if m < 0x7D {
                fx.step_down(disc + 7, 0, 8);
                fx.step_down(pl + 0xB, 0, 8);
            } else {
                fx.mem.write_u8(disc + 7, 0x80);
                fx.mem.write_u8(pl + 0xB, 0x80);
            }
            if n(fx) < 0x7D {
                let a = fx.mem.i32(pl + 0x38).wrapping_add(0x200);
                fx.mem.set32(pl + 0x38, a);
            } else {
                fx.mem.write_u32(pl + 0x38, 0);
            }
        }
    }
    if n(fx) >= 0x8A {
        for k in 0..3u32 {
            glow::line(fx, OT, D + 0xB0 + 0x40 * k, D, sp + 0xA0 + 0x10 * k, sp + 0xD0);
        }
        glow::line(fx, OT, D + 0x70, D, sp + 0xD0, D + 0x4010);
        if n(fx) >= 0x100 {
            fx.step_down(D + 0x7B, 0, 8);
            for k in 0..3u32 {
                fx.step_down(D + 0xBB + 0x40 * k, 0, 8);
            }
        }
    }
    if n(fx) >= 0xA6 {
        fx.sprites(D + 0x2D70, D);
        if n(fx) >= 0x100 {
            fx.step_down(D + 0x2D7B, 0, 1);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x2DCA) {
            let p = D + 0x2DD0 + 16 * k as u32;
            let s0 = D + 16 * k as u32;
            let kk = k;
            k += 1;
            if fx.mem.i16(p + 0xE) == 0 {
                sprite_place(fx, s0, p, D + 0x340, D + 0x2DBC, 0x01E0_5168);
            }
            if fx.mem.i16(p + 0xE) < 0 {
                continue;
            }
            let up = [1.0f32, 2.0, 3.0][(kk % 3) as usize];
            let y = ee::add(fx.mem.f(p + 4), up);
            fx.mem.setf(p + 4, y);
            if fx.step_down(p + 0xC, 0, 1) != 0 {
                continue;
            }
            fx.mem.set16(p + 0xE, 0);
            fx.mem.write_u8(p + 0xC, 0x10);
            sprite_place(fx, s0, p, D + 0x340, D + 0x2DBC, 0x01E0_5270);
        }
        blib::trail::sparks(fx, OT, D + 0x390, D + 0x320);
        if n(fx) >= 0x100 {
            fx.step_down(D + 0x39B, 0, 0x10);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x3BC) {
            let p = D + 0x3C0 + 16 * k as u32;
            let s0 = D + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) < 0 {
                continue;
            }
            let s = f32::from_bits(0x3FA6_6666);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 4 * c), s);
                fx.mem.setf(p + 4 * c, x);
            }
            if ee_le(fx.mem.f(p + 4), 50.0) {
                continue;
            }
            let m = ee::fixsfsi(ee::mul(10.0, fx.mem.f(D + 0x3B8)));
            let a = fx.rand_from(s0, 0x01E0_5394) & 0xFFFF;
            let r = fx.rand_from(s0, 0x01E0_53B0);
            let (_, h) = fx.div(r, m);
            fx.mem.write_u32(p + 4, 0x4080_0000);
            let rad = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
            let c = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(rad, c));
            let s = fx.tsin(a);
            fx.mem.setf(p, ee::mul(rad, s));
            let r = fx.rand_from(s0, 0x01E0_5400);
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0xE, h.wrapping_neg());
        }
        blade::draw(fx, OT, D + 0xC30, D + 0xBC0);
        if n(fx) >= 0x10F {
            if n(fx) == 0x10F {
                fx.mem.write_u32(D + 0xCBC, 0x4140_0000);
                let mut k = 0;
                while k < fx.mem.i16(D + 0xCCA) {
                    let p = fx.mem.read_u32(D + 0xCB0) + 16 * k as u32;
                    fx.mem.set16(p + 0xE, 0);
                    k += 1;
                }
            }
            blades_fade(fx, D + 0xC30);
        }
        blade::draw(fx, OT, D + 0x1CD0, D + 0xBC0);
        blades_fade(fx, D + 0x1CD0);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x4024, m);
    let left = fx.mem.i32(D + 0x4020) - 1;
    fx.mem.set32(D + 0x4020, left);
    if m == 0x118 {
        2
    } else if m == 0x9C {
        1
    } else if (0xA6..0x118).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
