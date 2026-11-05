//! BMODULE b391 (card 488), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02E10, data area a0 = 0x1ED0000) [F] (helper B9, 31
//! Oct 2026). The stage dims (0x1F7F10, +0x2688) until update 161; 256
//! sparks (+0x1640) rising round the attacker (+0x15D0, turned to look at
//! the target until update 111) and respawning above 250; from update 50
//! a frame quad (0x80, 0x80, 0x80, 0x60) until 240, a disc (+0xE0), a quad
//! burst (+0x100, its 64 angles turning by 0x40 a step), the attacker
//! zooming in four sprites (+0xF0, `zoom::sprites`) and until 161 four
//! rings (+0x5D0 + 0x400 k, radii 6 (k + 1) / 6 (k + 1) - 4) in a
//! billboard (+0x560) growing to 10 by 1 from 97; from 240 a full-window
//! tint (+0x2670) fading. Returns 1 on update 151, 6 on 161..239, 2 on 240,
//! 8 after 260.
//!
//! Library (record addresses): `colours4` 0x1E00080, `tint` 0x1E00120,
//! `zoom::sprites` 0x1E00280, `frame_quad_rgba` 0x1E00950, `disc`
//! 0x1E00A70, `scatter` 0x1E00FD0, `trail::sparks` 0x1E01190, `ring_setup`
//! / `ring_draw` 0x1E01EF0 / 0x1E021B0, `quads_setup` / `quads_draw`
//! 0x1E026A0 / 0x1E02800.
use crate::b262::tint;
use crate::blib::{spawn, trail, zoom};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad corner template (rodata, 0x1E026A0's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_3680;

/// The entry 0x1E02E10 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn bytes(fx: &mut Fx, at: &[(u32, u8)]) {
    for &(o, v) in at {
        fx.mem.write_u8(D + o, v);
    }
}

fn halves(fx: &mut Fx, at: &[(u32, i32)]) {
    for &(o, v) in at {
        fx.mem.set16(D + o, v);
    }
}

/// Spark `k` of the list *`list` placed on a ring: angle rand & 0xFFFF,
/// radius 0.1 (rand % 400), height 0.1 (rand % `m`); x = r sin, z = r cos.
pub(crate) fn spark_place(fx: &mut Fx, list: u32, k: u32, m: i32, ras: [u32; 3]) {
    let a = fx.rand_from(D, ras[0]) & 0xFFFF;
    let r = fx.rand_from(D, ras[1]);
    let (_, h) = fx.div(r, 0x190);
    let tenth = f32::from_bits(0x3DCC_CCCD);
    let rad = ee::mul(tenth, ee::from_i32(h));
    let r = fx.rand_from(D, ras[2]);
    let (_, y) = fx.div(r, m);
    let p = fx.mem.read_u32(list) + 0x10 * k;
    fx.mem.setf(p + 4, ee::mul(tenth, ee::from_i32(y)));
    let c = fx.tcos(a);
    let p = fx.mem.read_u32(list) + 0x10 * k;
    fx.mem.setf(p + 8, ee::mul(rad, c));
    let s = fx.tsin(a);
    let p = fx.mem.read_u32(list) + 0x10 * k;
    fx.mem.setf(p, ee::mul(rad, s));
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2690);
    for x in [0u32, 0x70, 0x15D0] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2680u32, 0x12, 1), (0x2684, 1, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2674, 0x104);
    fx.mem.set32(D + 0x2678, 1);
    fx.mem.set32(D + 0x267C, param);
    fx.mem.set32(D + 0x2688, 0x80);
    bytes(fx, &[(0x2672, 0x70), (0x2671, 0xB0), (0x2670, 0xE0), (0x2673, 0x80)]);
    // The disc (+0xE0): radius 60.
    fx.mem.write_u32(D + 0xE0, 0);
    bytes(fx, &[(0xE6, 0x70), (0xE5, 0xB0), (0xE4, 0xE0), (0xE7, 0)]);
    fx.mem.write_u32(D + 0xE8, 0x4270_0000);
    halves(fx, &[(0xEC, 0x24), (0xEE, 0)]);
    // The zoom (+0xF0) of the attacker.
    fx.mem.write_u32(D + 0xF0, 0);
    bytes(fx, &[(0xF6, 0x60), (0xF5, 0x60), (0xF4, 0x80), (0xF7, 0)]);
    fx.mem.write_u32(D + 0xF8, 0x4220_0000);
    halves(fx, &[(0xFC, 6)]);
    let h = fx.self_handle();
    halves(fx, &[(0xFE, h)]);
    // The quad burst (+0x100): size 2.4.
    fx.texture4(sp + 0xA0, 0xF);
    fx.mem.write_u32(D + 0x104, 0x4_0001);
    halves(fx, &[(0x110, 0x8E), (0x112, 0), (0x114, 0x22), (0x116, 0x40)]);
    bytes(fx, &[(0x10A, 0xD0), (0x109, 0xA0), (0x108, 0x70), (0x10B, 0)]);
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.quads_setup(D + 0x100, tex, f32::from_bits(0x4019_999A), QUAD_TEMPLATE);
    // Four rings (+0x5D0 + 0x400 k).
    for k in 0..4u32 {
        let g = 0x5D0 + 0x400 * k;
        fx.mem.write_u32(D + g + 4, 0x2000_0000);
        halves(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x80)]);
        bytes(fx, &[(g + 0xA, 0x70), (g + 9, 0xB0), (g + 8, 0xE0), (g + 0xB, 0)]);
        let outer = ee::mul(6.0, ee::from_i32(k as i32 + 1));
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(D + g, tex, outer, ee::sub(outer, 4.0), 0.0, 0.0);
    }
    // The sparks (+0x1640): 256 at +0x1670.
    fx.texture5(sp + 0xA0, 0);
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x1640, tex);
    fx.mem.write_u32(D + 0x1644, 0x6000_0403);
    bytes(fx, &[(0x164A, 0x80), (0x1649, 0x80), (0x1648, 0x80), (0x164B, 0x80)]);
    fx.mem.write_u32(D + 0x1660, 0x3ECC_CCCD);
    fx.mem.write_u32(D + 0x1664, 0x41B0_0000);
    halves(fx, &[(0x1650, 0), (0x1652, 0), (0x1654, 0x20), (0x1656, 0x20), (0x1658, 0), (0x165A, 0), (0x165C, 0x80), (0x165E, 0x40)]);
    fx.mem.write_u32(D + 0x164C, D + 0x1670);
    fx.mem.write_u32(D + 0x1668, 0x41F0_0000);
    halves(fx, &[(0x166C, 0x100)]);
    bytes(fx, &[(0x166E, 8), (0x166F, 0)]);
    spawn::scatter(fx, D + 0x1640, 0xC, 3.0);
    let mut k = 0u32;
    while (k as i32) < fx.mem.i16(D + 0x166C) {
        spark_place(fx, D + 0x164C, k, 0x9C4, [0x01E0_30E4, 0x01E0_3100, 0x01E0_312C]);
        k += 1;
    }
    fx.mem.write_u32(D + 0x74, 0x3FC0_0000);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2678);
    let look = n(fx) < 0x6F;
    if look {
        let h = fx.self_handle();
        fx.unit_point(1, h, D + 0x15F0);
        let t = fx.target_handle();
        fx.unit_position(t, sp + 0x80);
        let h = fx.self_handle();
        fx.unit_point(1, h, sp + 0x90);
        fx.look_angles(sp + 0x80, sp + 0x90, D + 0x15E0);
    }
    for out in [0x90u32, 0x580] {
        let h = fx.self_handle();
        fx.unit_point(1, h, D + out);
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x90 (the look branch) or the
    // parameter (-1, 64 bits), s3 the runner's 0.
    let s2 = if look { u64::from(sp + 0x90) } else { u64::MAX };
    let regs = [Some(D), Some(OT), None, Some(0)];
    for (x, ra, bill) in [(0u32, 0x01E0_3258, false), (0x70, 0x01E0_3260, true), (0x560, 0x01E0_3268, true), (0x15D0, 0x01E0_3270, false)] {
        if bill {
            fx.xform_billboard_saving(D + x, ra, regs);
            fx.saved(sp - 0xF0, &[(0x40, s2)]);
        } else {
            fx.xform_world_saving(D + x, ra, regs);
            fx.saved(sp - 0xD0, &[(0x40, s2)]);
        }
    }
    if (0x32..0xF0).contains(&n(fx)) {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0xA1 {
        fx.step_down(D + 0x2688, 0x20, 4);
    } else {
        fx.step_up(D + 0x2688, 0x80, 4);
    }
    let l = fx.mem.read_u8(D + 0x2688);
    fx.set_stage_light(i32::from(l));
    if n(fx) >= 0xF0 {
        tint(fx, D + 0x2670);
        fx.step_down(D + 0x2673, 0, 8);
    }
    if n(fx) == 0xA1 {
        let f = fx.mem.read_u32(D + 0x1644) & !0x400;
        fx.mem.write_u32(D + 0x1644, f);
    }
    trail::sparks(fx, OT, D + 0x1640, D + 0x15D0);
    if n(fx) >= 0xF1 {
        fx.step_down(D + 0x164B, 0, 8);
    }
    let mut k = 0u32;
    while (k as i32) < fx.mem.i16(D + 0x166C) {
        let p = D + 0x10 * k;
        if fx.mem.i16(p + 0x167E) >= 0 {
            let y = ee::add(fx.mem.f(p + 0x1674), 12.0);
            fx.mem.setf(p + 0x1674, y);
            if !ee_le(y, 250.0) {
                spark_place(fx, D + 0x164C, k, 0x3C, [0x01E0_33A4, 0x01E0_33C0, 0x01E0_33EC]);
                fx.mem.set16(p + 0x167E, 0);
            }
        }
        k += 1;
    }
    if n(fx) >= 0x32 {
        fx.disc(D + 0xE0, D + 0x70);
        fx.step_up(D + 0xE7, 0xFF, 0x10);
        if n(fx) < 0xA1 {
            for k in 0..4u32 {
                fx.ring_draw(D + 0x5D0 + 0x400 * k, D + 0x560);
                fx.step_up(D + 0x5DB + 0x400 * k, 0x40, 4);
            }
        }
        fx.quads_draw(D + 0x100, D + 0x70);
        fx.step_up(D + 0x10B, 0x10, 1);
        for k in 0..0x40u32 {
            for o in [0x160u32, 0x164, 0x168] {
                let a = D + o + 0x10 * k;
                let v = fx.mem.i32(a).wrapping_add(0x40);
                fx.mem.set32(a, v);
            }
        }
        zoom::sprites(fx, 0x01E0_0280, D + 0xF0, D + 0x90);
        fx.step_up(D + 0xF7, 0x40, 4);
        if (0x61..0xA1).contains(&n(fx)) {
            fx.approach(D + 0x560, 10.0, 1.0);
            fx.approach(D + 0x70, 2.0, f32::from_bits(0x3DCC_CCCD));
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2678, m);
    let left = fx.mem.i32(D + 0x2674) - 1;
    fx.mem.set32(D + 0x2674, left);
    if m == 0xF0 {
        2
    } else if m == 0x97 {
        1
    } else if (0xA1..0xF0).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
