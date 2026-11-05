//! BMODULE b556 (card 522), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04730, data area 0x1ED0000) [F] (helper C5, 31
//! Oct 2026). The stage light dims to 0x20 until update 280 and the
//! other light (0x1F7EF0) until 68. At the attacker's nodes 0x14 / 0x13
//! a disc and plate pair each (+0x3D70 / +0x3CF0, +0x3D80 / +0x3D30)
//! fade in and out on 40..91. From 120 at the attacker a fountain of
//! sprites (+0x2BB0) rising round a circle the size of the attacker
//! (re-spawned as they fade), a spark burst (+0x1D0) thrown out again on
//! a flat ring and a ring of blades (+0xA70, `blib::blade`) at the
//! attacker (turned by +0xA14 when it faces back) and a second (+0x1B10)
//! widening by 12 an age, fading its old blades (the first too from
//! 271). From 108
//! two glowing lines (+0x70 / +0xB0, `blib::glow`) run from the nodes to
//! the second ring's newest blade (+0x3D90), fading from 256. Returns 1
//! on update 110, 6 on 120..279, 2 on 280, 8 after 300.
//!
//! Library: `colours4` (0x1E00080), the frame quad (0x1E00120),
//! `spawn::scatter` (0x1E00240), `trail::sparks` (0x1E00400),
//! `sprites_spawn` / `sprites` (0x1E01160 / 0x1E012F0), `plate::plates`
//! (0x1E02BE0), `disc` (0x1E03170), `glow::line` (0x1E036D0),
//! `blade::setup` / `draw` (0x1E03E80 / 0x1E04020).
use crate::b107::target_radius;
use crate::b628::halves;
use crate::blib::{self, blade, glow, plate};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PLATES: u32 = 0x01E0_2BE0;
const BLADE_SETUP: u32 = 0x01E0_3E80;
/// The light byte 0x1F7EF0 sets.
const LIGHT2: u32 = 0x002D_F5B0;

/// The entry 0x1E04730.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

pub(crate) fn colour(fx: &mut Fx, g: u32, bgra: [u8; 4]) {
    for (k, x) in [(0xAu32, bgra[2]), (9, bgra[1]), (8, bgra[0]), (0xB, bgra[3])] {
        fx.mem.write_u8(g + k, x);
    }
}

/// A blade ring of b556 / b644: 256 blades, cell (0, 0, 32, 64), radius
/// 200, width 30, arc 0x2800, drawn + `more` a step.
pub(crate) fn blades(fx: &mut Fx, g: u32, tex: u32, height: u32, grow: u32, bgra: [u8; 4], more: i32) {
    fx.mem.write_u32(g + 4, 1);
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x20), (0x16, 0x40)]);
    colour(fx, g, bgra);
    for (k, x) in [(0x80u32, g + 0xA0), (0x84, height), (0x88, 0x41F0_0000), (0x8C, grow), (0x90, 0), (0x94, 0x2800), (0xC, 0x4348_0000)] {
        fx.mem.write_u32(g + k, x);
    }
    halves(fx, g, &[(0x98, 0), (0x9A, 0x100), (0x9C, 0), (0x9E, more)]);
    blade::setup(fx, BLADE_SETUP, g, tex);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3DD0);
    for x in [0u32, 0xF0, 0x160, 0xA00, 0x3C10, 0x3C80] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x3DACu32, 0x14, 1), (0x3DB0, 0x13, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x3DA0, 0x12C);
    fx.mem.set32(D + 0x3DA4, 1);
    fx.mem.set32(D + 0x3DA8, param);
    let r = target_radius(fx);
    fx.mem.setf(D + 0x3DBC, r);
    fx.mem.set32(D + 0x3DB4, 0x80);
    fx.mem.set32(D + 0x3DB8, 0x80);
    fx.texture5(sp + 0xB0, 0);
    let tex = |fx: &mut Fx| fx.mem.read_u32(sp + 0xB0);
    // Two glowing lines (+0x70 / +0xB0): 40 pieces, width 20.
    for k in 0..2u32 {
        let g = D + 0x70 + 0x40 * k;
        let t = tex(fx);
        fx.mem.write_u32(g, t);
        fx.mem.write_u32(g + 4, 0x400_0000);
        fx.mem.write_u32(g + 0xC, 0x41A0_0000);
        halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x10), (0x16, 0x20)]);
        colour(fx, g, [0x80; 4]);
        halves(fx, g, &[(0x28, 0xE), (0x2C, 0), (0x2A, 0x28)]);
        fx.mem.write_u32(g + 0x20, 0);
        fx.mem.write_u32(g + 0x24, 0);
    }
    // The spark burst (+0x1D0): 128 sparks at +0x200, width 0.5, length 5.
    let g = D + 0x1D0;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x2000_0003);
    colour(fx, g, [0x80; 4]);
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x40A0_0000);
    halves(fx, g, &[(0x10, 0x20), (0x12, 0x20), (0x14, 0x20), (0x16, 0x20), (0x18, 0x20), (0x1A, 0x20), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, D + 0x200);
    fx.mem.write_u32(g + 0x28, 0x40A0_0000);
    fx.mem.set16(g + 0x2C, 0x80);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 4.0);
    // Plates (+0x3CF0 / +0x3D30) and discs (+0x3D70 / +0x3D80).
    for k in 0..2u32 {
        let g = D + 0x3CF0 + 0x40 * k;
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
        let g = D + 0x3D70 + 0x10 * k;
        fx.mem.write_u32(g, 0x8_0000);
        for (o, x) in [(6u32, 0xE0u8), (5, 0x50), (4, 0x40), (7, 0)] {
            fx.mem.write_u8(g + o, x);
        }
        fx.mem.write_u32(g + 8, 0x4000_0000);
        halves(fx, g, &[(0xC, 0x24), (0xE, 0)]);
    }
    fx.texture4(sp + 0xB0, 0xF);
    // The sprite fountain (+0x2BB0): 256 sprites at +0x2C10, size 30.
    let g = D + 0x2BB0;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x4800_0003);
    colour(fx, g, [0x80, 0x80, 0x80, 0x10]);
    fx.mem.write_u32(g + 0xC, 0x41F0_0000);
    for o in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + o, 0);
    }
    halves(fx, g, &[(0x30, 0x80), (0x32, 0xC0), (0x34, 0x20), (0x36, 0x20), (0x38, 0x80), (0x3A, 0xC0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x2C10);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4120_0000);
    fx.mem.write_u32(g + 0x50, 0x40A0_0000);
    halves(fx, g, &[(0x5A, 0x100), (0x5C, 8), (0x5E, 1)]);
    fx.sprites_spawn(g, 30.0);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = D + 0x2C10 + 16 * k as u32;
        let r = fx.rand_from(D + 16 * k as u32, 0x01E0_4AA8);
        fx.mem.set16(p + 0xE, -(r % 16));
        let a = fx.mem.read_u8(g + 0xB);
        fx.mem.write_u8(p + 0xC, a);
        let r = fx.rand_from(D + 16 * k as u32, 0x01E0_4AD0);
        fx.mem.write_u8(p + 0xD, (r % 8) as u8);
        k += 1;
    }
    fx.texture5(sp + 0xB0, 1);
    let t = tex(fx);
    blades(fx, D + 0xA70, t, 0x4120_0000, 0, [0x40, 0x50, 0xE0, 0x80], 2);
    let t = tex(fx);
    blades(fx, D + 0x1B10, t, 0, 0x4140_0000, [0x40, 0x50, 0xE0, 0x80], 2);
    0
}

/// A fountain sprite `p` placed on the circle of radius `r` (address)
/// round the point `c` at a random angle (rand with s0 = `s0`).
pub(crate) fn sprite_place(fx: &mut Fx, s0: u32, p: u32, c: u32, r: u32, ra: u32) {
    let a = fx.rand_from(s0, ra) & 0xFFFF;
    let cs = fx.tcos(a);
    let v = ee::add(fx.mem.f(c + 8), ee::mul(fx.mem.f(r), cs));
    fx.mem.setf(p + 8, v);
    let s = fx.tsin(a);
    let v = ee::add(fx.mem.f(c), ee::mul(fx.mem.f(r), s));
    fx.mem.setf(p, v);
    let y = fx.mem.read_u32(c + 4);
    fx.mem.write_u32(p + 4, y);
}

/// Old blades (age 11 or more) of the ring `g` fade by 8.
pub(crate) fn blades_fade(fx: &mut Fx, g: u32) {
    let mut k = 0;
    while k < fx.mem.i16(g + 0x9A) {
        let p = fx.mem.read_u32(g + 0x80) + 16 * k as u32;
        if fx.mem.i16(p + 0xE) >= 0xB {
            fx.step_down(p + 0xC, 0, 8);
        }
        k += 1;
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x3DA4);
    for (at, out) in [(0x3DACu32, sp + 0x90), (0x3DB0, sp + 0xA0)] {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    }
    for k in 0..2u32 {
        for c in 0..4u32 {
            let w = fx.mem.read_u32(sp + 0x90 + 0x10 * k + 4 * c);
            fx.mem.write_u32(D + 0x3C30 + 0x70 * k + 4 * c, w);
        }
    }
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x110);
    let h = fx.self_handle();
    fx.unit_position(h, D + 0xA20);
    fx.mem.write_u32(D + 0xA24, 0);
    if ee_lt(fx.mem.f(D + 0xA28), 0.0) {
        fx.mem.write_u32(D + 0xA14, 0x8000);
    }
    // s0 = sp + 0x90, s1 = D, s2 = the OT, s3 the caller's (the loop's
    // pointer for the billboards); the last TRS matrix is the glow lines'
    // stale TEX0 / TEX1 without a texture [run].
    for (x, ra) in [(0u32, 0x01E0_4D08u32), (0xF0, 0x01E0_4D10), (0x160, 0x01E0_4D18), (0xA00, 0x01E0_4D20)] {
        fx.xform_world_saving(D + x, ra, [Some(sp + 0x90), Some(D), Some(OT), None]);
    }
    for k in 0..2u32 {
        fx.xform_billboard_saving(D + 0x3C10 + 0x70 * k, 0x01E0_4D30, [Some(sp + 0x90), Some(D), Some(OT), Some(D + 0x70 * k)]);
    }
    if n(fx) >= 0x10F {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0x118 {
        fx.step_down(D + 0x3DB4, 0x20, 4);
    } else {
        fx.step_up(D + 0x3DB4, 0x80, 4);
    }
    let l = i32::from(fx.mem.read_u8(D + 0x3DB4));
    fx.set_stage_light(l);
    if n(fx) < 0x44 {
        fx.step_down(D + 0x3DB8, 0x20, 4);
    } else {
        fx.step_up(D + 0x3DB8, 0x80, 4);
    }
    let l = fx.mem.read_u8(D + 0x3DB8);
    fx.mem.write_u8(LIGHT2, l);
    if (0x28..0x5C).contains(&n(fx)) {
        for k in 0..2u32 {
            let (disc, pl, x) = (D + 0x3D70 + 0x10 * k, D + 0x3CF0 + 0x40 * k, D + 0x3C10 + 0x70 * k);
            fx.disc(disc, x);
            plate::plates(fx, PLATES, OT, pl, x);
            if n(fx) >= 0x4C {
                fx.step_down(disc + 7, 0, 8);
                fx.step_down(pl + 0xB, 0, 8);
            } else {
                fx.step_up(disc + 7, 0x80, 8);
                fx.step_up(pl + 0xB, 0x80, 8);
            }
            let a = fx.mem.i32(pl + 0x38).wrapping_add(0x200);
            fx.mem.set32(pl + 0x38, a);
        }
    }
    if n(fx) >= 0x78 {
        fx.sprites(D + 0x2BB0, D);
        if n(fx) >= 0xEC {
            fx.step_down(D + 0x2BBB, 0, 1);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x2C0A) {
            let p = D + 0x2C10 + 16 * k as u32;
            let kk = k;
            k += 1;
            if fx.mem.i16(p + 0xE) == 0 {
                sprite_place(fx, sp + 0x90, p, D + 0x180, D + 0x2BFC, 0x01E0_4F10);
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
            sprite_place(fx, sp + 0x90, p, D + 0x180, D + 0x2BFC, 0x01E0_5018);
        }
        blib::trail::sparks(fx, OT, D + 0x1D0, D + 0x160);
        if n(fx) >= 0xEC {
            fx.step_down(D + 0x1DB, 0, 0x10);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x1FC) {
            let p = D + 0x200 + 16 * k as u32;
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
            let m = ee::fixsfsi(ee::mul(10.0, fx.mem.f(D + 0x1F8)));
            let a = fx.rand_from(sp + 0x90, 0x01E0_513C) & 0xFFFF;
            let r = fx.rand_from(sp + 0x90, 0x01E0_5158);
            let (_, h) = fx.div(r, m);
            fx.mem.write_u32(p + 4, 0x4080_0000);
            let rad = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
            let c = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(rad, c));
            let s = fx.tsin(a);
            fx.mem.setf(p, ee::mul(rad, s));
            let r = fx.rand_from(sp + 0x90, 0x01E0_51A8);
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0xE, h.wrapping_neg());
        }
        blade::draw(fx, OT, D + 0xA70, D + 0xA00);
        if n(fx) >= 0x10F {
            if n(fx) == 0x10F {
                fx.mem.write_u32(D + 0xAFC, 0x4140_0000);
                let mut k = 0;
                while k < fx.mem.i16(D + 0xB0A) {
                    let p = fx.mem.read_u32(D + 0xAF0) + 16 * k as u32;
                    fx.mem.set16(p + 0xE, 0);
                    k += 1;
                }
            }
            blades_fade(fx, D + 0xA70);
        }
        blade::draw(fx, OT, D + 0x1B10, D + 0xA00);
        blades_fade(fx, D + 0x1B10);
    }
    // The second ring's newest blade (+0x3D90 = +0x180).
    let arc = fx.mem.i32(D + 0x1BA4);
    let count = fx.mem.i16(D + 0x1BAA);
    let start = 0x8000i32.wrapping_sub(arc).wrapping_neg() >> 1;
    let (q, _) = fx.div(arc, count);
    let step = q.wrapping_neg();
    let drawn = fx.mem.i16(D + 0x1BAC) - 1;
    let m = fx.mult(step, drawn);
    let a = fx.mem.i32(D + 0xA14).wrapping_add(start).wrapping_add(m);
    let s = fx.tsin(a);
    let v = ee::mul(fx.mem.f(D + 0x1B1C), s);
    fx.mem.setf(D + 0x3D98, v);
    let drawn = fx.mem.i16(D + 0x1BAC) - 1;
    let m = fx.mult(step, drawn);
    let a = fx.mem.i32(D + 0xA14).wrapping_add(start).wrapping_add(m);
    let c = fx.tcos(a);
    let v = ee::mul(fx.mem.f(D + 0x1B1C), c);
    fx.mem.setf(D + 0x3D90, v);
    fx.mem.write_u32(D + 0x3D94, 0);
    for (o, b) in [(0x3D98u32, 0xA28u32), (0x3D90, 0xA20), (0x3D94, 0xA24)] {
        let v = ee::add(fx.mem.f(D + o), fx.mem.f(D + b));
        fx.mem.setf(D + o, v);
    }
    for c in 0..4u32 {
        let w = fx.mem.read_u32(D + 0x3D90 + 4 * c);
        fx.mem.write_u32(D + 0x180 + 4 * c, w);
    }
    if n(fx) >= 0x6C {
        for k in 0..2u32 {
            glow::line(fx, OT, D + 0x70 + 0x40 * k, D, sp + 0x90 + 0x10 * k, D + 0x3D90);
        }
        if n(fx) >= 0x100 {
            // Three steps of 0x40 from +0x7B: the lines' alphas and the
            // byte +0xFB of the xform at +0xF0 [F].
            for k in 0..3u32 {
                fx.step_down(D + 0x7B + 0x40 * k, 0, 8);
            }
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x3DA4, m);
    let left = fx.mem.i32(D + 0x3DA0) - 1;
    fx.mem.set32(D + 0x3DA0, left);
    if m == 0x118 {
        2
    } else if m == 0x6E {
        1
    } else if (0x78..0x118).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
