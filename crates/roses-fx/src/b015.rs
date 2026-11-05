//! BMODULE b015 (card 36), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03480, data area a0 = 0x1ED0000) [F] (helper B9, 31
//! Oct 2026): b391's program with the target's afterimage. The stage dims
//! (+0x2714) until update 241; 256 sparks (+0x1640) rising round the
//! attacker (+0x15D0) and respawning above 250 (lit from update 50); from
//! update 50 a frame quad (0x80, 0x80, 0x80, 0x60) until 240, a disc
//! (+0xE0), a quad burst (+0x100), the attacker zooming (+0xF0) and until
//! 161 four rings (+0x5D0 + 0x400 k) in billboard +0x560 growing from 151;
//! from 176 the target's afterimage in wavy strips (+0x26E0, transform
//! +0x2670) with the unit tinted (0x1F75F0); from 240 a full-window tint
//! (+0x26F8) fading. Returns 1 on update 151, 6 on 161..239, 2 on 240, 8
//! after 260 (the tint's alpha back to 0x80).
//!
//! Library (record addresses): `colours4` 0x1E00080, `tint` 0x1E00120,
//! `strips` 0x1E00280, `zoom::sprites` 0x1E008F0, `frame_quad_rgba`
//! 0x1E00FC0, `disc` 0x1E010E0, `scatter` 0x1E01640, `trail::sparks`
//! 0x1E01800, `ring_setup` / `ring_draw` 0x1E02560 / 0x1E02820,
//! `quads_setup` / `quads_draw` 0x1E02D10 / 0x1E02E70.
use crate::b101::unit_tint;
use crate::b262::tint;
use crate::b391::spark_place;
use crate::blib::{afterimage, spawn, trail, zoom};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad corner template (rodata, 0x1E02D10's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_3E00;

/// The entry 0x1E03480 (a0 = D, a1 = param).
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

/// 0x1F75F0 (`unit_tint`) called with the caller's s0 / s1 = `s` from
/// return address `ra`: 0x1F7600's frame (0x30: s0 +0, s1 +0x10, ra
/// +0x20) left below the caller's stack pointer (a later missing
/// texture's TEX0 reads the saved s1) [F; run].
pub(crate) fn unit_tint_saving(fx: &mut Fx, handle: i32, c: u32, s: [u64; 2], ra: u32) -> i32 {
    let r = unit_tint(fx, handle, c);
    let f = fx.sp - 0x30;
    fx.saved(f, &[(0, s[0]), (0x10, s[1]), (0x20, u64::from(ra))]);
    r
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2720);
    for x in [0u32, 0x70, 0x15D0, 0x2670] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x270Cu32, 0x12, 1), (0x2710, 1, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2700, 0x104);
    fx.mem.set32(D + 0x2704, 1);
    fx.mem.set32(D + 0x2708, param);
    fx.mem.set32(D + 0x2714, 0x80);
    bytes(fx, &[(0x26FA, 0xE0), (0x26F9, 0xB0), (0x26F8, 0x70), (0x26FB, 0x80)]);
    // The disc (+0xE0): radius 30.
    fx.mem.write_u32(D + 0xE0, 0x4_0000);
    bytes(fx, &[(0xE6, 0xE0), (0xE5, 0xB0), (0xE4, 0x70), (0xE7, 0)]);
    fx.mem.write_u32(D + 0xE8, 0x41F0_0000);
    halves(fx, &[(0xEC, 0x24), (0xEE, 0)]);
    // The zoom (+0xF0) of the attacker.
    fx.mem.write_u32(D + 0xF0, 0);
    bytes(fx, &[(0xF6, 0x80), (0xF5, 0x80), (0xF4, 0x60), (0xF7, 0)]);
    fx.mem.write_u32(D + 0xF8, 0x4220_0000);
    halves(fx, &[(0xFC, 6)]);
    let h = fx.self_handle();
    halves(fx, &[(0xFE, h)]);
    // The afterimage (+0x26E0).
    fx.mem.write_u32(D + 0x26E0, 0x4_1000);
    bytes(fx, &[(0x26E6, 0x80), (0x26E5, 0x10), (0x26E4, 0x60), (0x26E7, 0x80)]);
    fx.mem.write_u32(D + 0x26F0, 0);
    fx.mem.write_u32(D + 0x26EC, 0x1000);
    halves(fx, &[(0x26E8, 0)]);
    fx.mem.write_u32(D + 0x26F4, 0x41A0_0000);
    let t = fx.target_handle();
    halves(fx, &[(0x26EA, t)]);
    bytes(fx, &[(0x26FE, 0x80), (0x26FD, 0x80), (0x26FC, 0x80), (0x26FF, 0x80)]);
    // The quad burst (+0x100): size 1.2.
    fx.texture4(sp + 0xA0, 0xF);
    fx.mem.write_u32(D + 0x104, 0x4_0001);
    halves(fx, &[(0x110, 0x8E), (0x112, 0), (0x114, 0x22), (0x116, 0x40)]);
    bytes(fx, &[(0x10A, 0xD0), (0x109, 0xA0), (0x108, 0x70), (0x10B, 0)]);
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.quads_setup(D + 0x100, tex, f32::from_bits(0x3F99_999A), QUAD_TEMPLATE);
    // Four rings (+0x5D0 + 0x400 k).
    for k in 0..4u32 {
        let g = 0x5D0 + 0x400 * k;
        fx.mem.write_u32(D + g + 4, 0x2000_0000);
        halves(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x80)]);
        bytes(fx, &[(g + 0xA, 0xE0), (g + 9, 0xB0), (g + 8, 0x70), (g + 0xB, 0)]);
        let outer = ee::mul(6.0, ee::from_i32(k as i32 + 1));
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(D + g, tex, outer, ee::sub(outer, 4.0), 0.0, 0.0);
    }
    // The sparks (+0x1640): 256 at +0x1670, unlit.
    fx.texture5(sp + 0xA0, 0);
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x1640, tex);
    fx.mem.write_u32(D + 0x1644, 0x6000_0403);
    bytes(fx, &[(0x164A, 0x80), (0x1649, 0x80), (0x1648, 0x80), (0x164B, 0)]);
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
        spark_place(fx, D + 0x164C, k, 0x9C4, [0x01E0_37BC, 0x01E0_37D8, 0x01E0_3804]);
        k += 1;
    }
    fx.mem.write_u32(D + 0x74, 0x3FC0_0000);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2704);
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
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x2690);
    // s0 = D, s1 = the OT, s2 = sp + 0x90 (the look branch) or the
    // parameter (-1, 64 bits), s3 the runner's 0.
    let s2 = if look { u64::from(sp + 0x90) } else { u64::MAX };
    let regs = [Some(D), Some(OT), None, Some(0)];
    for (x, ra, bill) in [(0u32, 0x01E0_3948, false), (0x70, 0x01E0_3950, true), (0x560, 0x01E0_3958, true), (0x15D0, 0x01E0_3960, false), (0x2670, 0x01E0_3968, false)] {
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
    if n(fx) >= 0xF1 {
        fx.step_up(D + 0x2714, 0x80, 8);
    } else {
        fx.step_down(D + 0x2714, 0x20, 4);
    }
    let l = fx.mem.read_u8(D + 0x2714);
    fx.set_stage_light(i32::from(l));
    if n(fx) >= 0xF0 {
        tint(fx, D + 0x26F8);
        fx.step_down(D + 0x26FB, 0, 8);
    }
    if n(fx) >= 0xB0 {
        let a = fx.mem.read_u8(D + 0x26E7);
        fx.mem.write_u8(D + 0x26FF, 0x80u8.wrapping_sub(a));
        afterimage::strips(fx, D + 0x26E0, D + 0x2690);
        let h = fx.mem.i16(D + 0x26EA);
        unit_tint_saving(fx, h, D + 0x26FC, [u64::from(D), u64::from(OT)], 0x01E0_3A34);
        if n(fx) >= 0xF1 {
            fx.step_down(D + 0x26E7, 0, 8);
        }
    }
    if n(fx) == 0x32 {
        fx.mem.write_u8(D + 0x164B, 0x80);
    }
    if n(fx) == 0xB0 {
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
                spark_place(fx, D + 0x164C, k, 0x3C, [0x01E0_3B0C, 0x01E0_3B28, 0x01E0_3B54]);
                fx.mem.set16(p + 0x167E, 0);
                if n(fx) >= 0x32 {
                    let q = fx.mem.read_u32(D + 0x164C) + 0x10 * k;
                    fx.mem.write_u8(q + 0xC, 0x80);
                }
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
        zoom::sprites(fx, 0x01E0_08F0, D + 0xF0, D + 0x90);
        fx.step_up(D + 0xF7, 0x40, 4);
        if (0x97..0xB0).contains(&n(fx)) {
            fx.approach(D + 0x560, 10.0, 1.0);
            fx.approach(D + 0x70, 2.0, f32::from_bits(0x3DCC_CCCD));
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2704, m);
    let left = fx.mem.i32(D + 0x2700) - 1;
    fx.mem.set32(D + 0x2700, left);
    if m == 0xF0 {
        2
    } else if m == 0x97 {
        1
    } else if (0xA1..0xF0).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        fx.mem.write_u8(D + 0x26FF, 0x80);
        8
    }
}
