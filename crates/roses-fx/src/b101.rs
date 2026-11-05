//! BMODULE b101 (card 305), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03100, data area 0x1ED0000) [F] (helper C6, 31
//! Oct 2026): the stage dims (0x1F7F10) and the other light (0x1F7EF0)
//! until update 0x44 / 0x118; on updates 0x28..0x8F two glows (discs
//! +0xB60 / +0xB70 and plates +0xAE0 / +0xB20) at the attacker's nodes 3 /
//! 4; from update 0x4E two glow lines (+0x70, +0xB0) from those nodes to
//! the target; from update 0x5A sparks rising at the attacker (+0x1D0,
//! respawning above 50), three rings swelling at the target (+0xCD0 +
//! 0x400 k) and the target's afterimage in wavy strips (+0x1940,
//! `blib::afterimage`) with the unit tinted (0x1F75F0). Returns 1 on update
//! 0x50, 6 on 0x5A..0x117, 2 on 0x118, 8 after 0x12C.
//!
//! Library at its own addresses: `colours4` (0x1E00080), `strips`
//! (0x1E00120), `scatter` (0x1E00790), `trail::sparks` (0x1E00950),
//! `plates` (0x1E016B0), `disc` (0x1E01C40), `glow::line` (0x1E021A0),
//! `ring_setup` / `ring_draw` (0x1E02950 / 0x1E02C10).
use crate::blib::{afterimage, glow, plate, spawn, trail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PLATES: u32 = 0x01E0_16B0;
/// The light byte 0x1F7EF0 sets.
const LIGHT2: u32 = 0x002D_F5B0;

/// The entry 0x1E03100.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

/// 0x1F75F0(handle, c) = 0x1F7600(handle, -1, c) [F]: for a unit object
/// (side 0..2, slot 0..10, 0x1E25E0) its tint obj+0x12C..0x12F = the 4
/// bytes at `c` (0: 0x80 each) (0x1F38F0, mode -1 leaves obj+0x1F8). 1
/// when written.
pub(crate) fn unit_tint(fx: &mut Fx, handle: i32, c: u32) -> i32 {
    let (side, slot) = (handle >> 8, handle & 0xFF);
    if !(0..3).contains(&side) || slot >= 0xB {
        return 0;
    }
    let i = fx.object_index(side, slot);
    if !(0..0x54).contains(&i) {
        return 0;
    }
    let o = fx.mem.read_u32(crate::world::OBJECT_TABLE + 4 * i as u32);
    if o == 0 {
        return 0;
    }
    for k in 0..4u32 {
        let v = if c != 0 { fx.mem.read_u8(c + k) } else { 0x80 };
        fx.mem.write_u8(o + 0x12C + k, v);
    }
    1
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

fn copy_v4(fx: &mut Fx, to: u32, from: u32) {
    let v = fx.read_v4(from);
    fx.write_v4(to, v);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1990);
    for x in [0u32, 0xF0, 0x160, 0xA00, 0xA70, 0xB80, 0xBF0, 0xC60, 0x18D0] {
        fx.xform_init(D + x);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 3, 1);
    fx.mem.set32(D + 0x1974, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 4, 0);
    fx.mem.set32(D + 0x1978, r);
    fx.mem.set32(D + 0x1968, 0x12C);
    fx.mem.set32(D + 0x196C, 1);
    fx.mem.set32(D + 0x1970, param);
    let rad = crate::b107::target_radius(fx);
    fx.mem.setf(D + 0x1988, rad);
    fx.mem.set32(D + 0x1980, 0x80);
    fx.mem.set32(D + 0x1984, 0x80);
    // The afterimage (+0x1940).
    fx.mem.write_u32(D + 0x1940, 0x4_1000);
    bytes(fx, &[(0x1946, 0x80), (0x1945, 0x40), (0x1944, 0x40), (0x1947, 0x80)]);
    fx.mem.write_u32(D + 0x1950, 0);
    fx.mem.write_u32(D + 0x194C, 0x1000);
    halves(fx, &[(0x1948, 0)]);
    fx.mem.write_u32(D + 0x1954, 0x41A0_0000);
    let t = fx.target_handle();
    halves(fx, &[(0x194A, t)]);
    bytes(fx, &[(0x197E, 0x80), (0x197D, 0x80), (0x197C, 0x80), (0x197F, 0x80)]);
    fx.texture5(sp + 0xB0, 0);
    // The glow lines (+0x70 + 0x40 k).
    for k in 0..2u32 {
        let g = 0x70 + 0x40 * k;
        let tex = fx.mem.read_u32(sp + 0xB0);
        fx.mem.write_u32(D + g, tex);
        fx.mem.write_u32(D + g + 4, 0x0400_0000);
        fx.mem.write_u32(D + g + 0xC, 0x41A0_0000);
        halves(fx, &[(g + 0x10, 0), (g + 0x12, 0), (g + 0x14, 0x10), (g + 0x16, 0x20)]);
        bytes(fx, &[(g + 0xA, 0x80), (g + 9, 0x80), (g + 8, 0x80), (g + 0xB, 0x80)]);
        halves(fx, &[(g + 0x28, 0x14), (g + 0x2C, 0), (g + 0x2A, 0x28)]);
        fx.mem.write_u32(D + g + 0x20, 0);
        fx.mem.write_u32(D + g + 0x24, 0);
    }
    // The sparks (+0x1D0).
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(D + 0x1D0, tex);
    fx.mem.write_u32(D + 0x1D4, 0x2000_0003);
    bytes(fx, &[(0x1DA, 0x80), (0x1D9, 0x80), (0x1D8, 0x80), (0x1DB, 0x80)]);
    fx.mem.write_u32(D + 0x1F0, 0x3F00_0000);
    fx.mem.write_u32(D + 0x1F4, 0x40A0_0000);
    halves(fx, &[(0x1E0, 0x20), (0x1E2, 0x20), (0x1E4, 0x20), (0x1E6, 0x20), (0x1E8, 0x20), (0x1EA, 0x20), (0x1EC, 0x80), (0x1EE, 0x40)]);
    fx.mem.write_u32(D + 0x1DC, D + 0x200);
    fx.mem.write_u32(D + 0x1F8, 0x40A0_0000);
    halves(fx, &[(0x1FC, 0x80)]);
    bytes(fx, &[(0x1FE, 1), (0x1FF, 0)]);
    spawn::scatter(fx, D + 0x1D0, 0xC, 4.0);
    // The plates (+0xAE0 + 0x40 k) and discs (+0xB60 + 0x10 k).
    for k in 0..2u32 {
        let (p, c) = (0xAE0 + 0x40 * k, 0xB60 + 0x10 * k);
        let tex = fx.mem.read_u32(sp + 0xB0);
        fx.mem.write_u32(D + p, tex);
        fx.mem.write_u32(D + p + 4, 0x8_0000);
        bytes(fx, &[(p + 0xA, 0x80), (p + 9, 0x80), (p + 8, 0x80), (p + 0xB, 0)]);
        for (o, v) in [(0xCu32, 0x3E80_0000u32), (0x10, 0x4040_0000), (0x14, 0x3E80_0000), (0x18, 0x3F80_0000)] {
            fx.mem.write_u32(D + p + o, v);
        }
        halves(fx, &[(p + 0x20, 0x20), (p + 0x22, 0x20), (p + 0x24, 0x20), (p + 0x26, 0x20)]);
        for o in [0x30u32, 0x34, 0x38] {
            fx.mem.write_u32(D + p + o, 0);
        }
        fx.mem.write_u32(D + c, 0x8_0000);
        bytes(fx, &[(c + 6, 0xE0), (c + 5, 0x50), (c + 4, 0x40), (c + 7, 0)]);
        fx.mem.write_u32(D + c + 8, 0x4000_0000);
        halves(fx, &[(c + 0xC, 0x24), (c + 0xE, 0)]);
    }
    fx.texture4(sp + 0xB0, 0xF);
    // Three rings (+0xCD0 + 0x400 k).
    for k in 0..3u32 {
        let g = 0xCD0 + 0x400 * k;
        fx.mem.write_u32(D + g + 4, 0);
        halves(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x80)]);
        bytes(fx, &[(g + 0xA, 0xE0), (g + 9, 0x50), (g + 8, 0x40), (g + 0xB, 0x80)]);
        let tex = fx.mem.read_u32(sp + 0xB0);
        fx.ring_setup(D + g, tex, f32::from_bits(0x413B_3333), f32::from_bits(0x4144_CCCD), 0.0, 0.0);
    }
    for (x, v) in [(0xB80u32, 0u32), (0xBF0, 0x3F00_0000), (0xC60, 0x3F80_0000)] {
        for o in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + o, v);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x196C);
    // s0 = sp + 0x90 from the node copies on (rand saves it).
    let s0 = sp + 0x90;
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x1974);
    fx.node_position(h, s, sp + 0x90);
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x1978);
    fx.node_position(h, s, sp + 0xA0);
    copy_v4(fx, D + 0xA20, sp + 0x90);
    copy_v4(fx, D + 0xA90, sp + 0xA0);
    let h = fx.self_handle();
    fx.unit_position(h, D + 0x110);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x1958);
    fx.look_angles(D + 0x1958, D + 0x110, D + 0x170);
    copy_v4(fx, D + 0x180, D + 0x1958);
    for k in 0..3u32 {
        let t = fx.target_handle();
        fx.unit_position(t, D + 0xBA0 + 0x70 * k);
        copy_v4(fx, D + 0xB90 + 0x70 * k, D + 0x170);
    }
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x18F0);
    // s0 = sp + 0x90, s1 = D, s2 = the OT, s3 = the loops' D + 0x70 k.
    let regs = |s3: u32| [Some(sp + 0x90), Some(D), Some(OT), Some(s3)];
    for (k, x) in [0u32, 0xF0, 0x160].into_iter().enumerate() {
        fx.xform_world_saving(D + x, 0x01E0_3618 + 8 * k as u32, regs(D + 0x150));
    }
    for k in 0..2u32 {
        fx.xform_billboard_saving(D + 0xA00 + 0x70 * k, 0x01E0_3638, regs(D + 0x70 * k));
    }
    for k in 0..3u32 {
        fx.xform_world_saving(D + 0xB80 + 0x70 * k, 0x01E0_3658, regs(D + 0x70 * k));
    }
    fx.xform_world_saving(D + 0x18D0, 0x01E0_3678, regs(D + 0x150));
    if n(fx) < 0x118 {
        fx.step_down(D + 0x1980, 0x20, 4);
    } else {
        fx.step_up(D + 0x1980, 0x80, 4);
    }
    let l = fx.mem.read_u8(D + 0x1980);
    fx.set_stage_light(i32::from(l));
    if n(fx) < 0x44 {
        fx.step_down(D + 0x1984, 0x20, 4);
    } else {
        fx.step_up(D + 0x1984, 0x80, 4);
    }
    let l = fx.mem.read_u8(D + 0x1984);
    fx.mem.write_u8(LIGHT2, l);
    let m = n(fx);
    if (0x28..0x90).contains(&m) {
        for k in 0..2u32 {
            let x = D + 0xA00 + 0x70 * k;
            fx.disc(D + 0xB60 + 0x10 * k, x);
            plate::plates(fx, PLATES, OT, D + 0xAE0 + 0x40 * k, x);
            if n(fx) >= 0x80 {
                fx.step_down(D + 0xB67 + 0x10 * k, 0, 8);
                fx.step_down(D + 0xAEB + 0x40 * k, 0, 8);
            } else {
                fx.step_up(D + 0xB67 + 0x10 * k, 0x80, 8);
                fx.step_up(D + 0xAEB + 0x40 * k, 0x80, 8);
            }
            let a = D + 0xB18 + 0x40 * k;
            let v = fx.mem.i32(a).wrapping_add(0x200);
            fx.mem.set32(a, v);
        }
    }
    if n(fx) >= 0x5A {
        trail::sparks(fx, OT, D + 0x1D0, D + 0x160);
        if n(fx) >= 0x118 {
            fx.step_down(D + 0x1DB, 0, 8);
        }
        let grow = f32::from_bits(0x3FA6_6666);
        let mut k = 0i32;
        while k < fx.mem.i16(D + 0x1FC) {
            let p = D + 0x10 * k as u32;
            if fx.mem.i16(p + 0x20E) >= 0 {
                for o in [0x200u32, 0x204, 0x208] {
                    let v = ee::mul(fx.mem.f(p + o), grow);
                    fx.mem.setf(p + o, v);
                }
                if !ee_le(fx.mem.f(p + 0x204), 50.0) {
                    let spread = ee::fixsfsi(ee::mul(10.0, fx.mem.f(D + 0x1F8)));
                    let a = fx.rand_from(s0, 0x01E0_3894) & 0xFFFF;
                    let r = fx.rand_from(s0, 0x01E0_38B0);
                    let (_, rem) = fx.div(r, spread);
                    fx.mem.write_u32(p + 0x204, 0x4080_0000);
                    let rad = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(rem));
                    let c = fx.tcos(a);
                    fx.mem.setf(p + 0x208, ee::mul(rad, c));
                    let s = fx.tsin(a);
                    fx.mem.setf(p + 0x200, ee::mul(rad, s));
                    let r = fx.rand_from(s0, 0x01E0_3900);
                    let (_, rem) = fx.div(r, 0xC);
                    fx.mem.set16(p + 0x20E, rem.wrapping_neg());
                }
            }
            k += 1;
        }
        let tenth = f32::from_bits(0x3DCC_CCCD);
        for k in 0..3u32 {
            let (g, x) = (D + 0xCD0 + 0x400 * k, D + 0xB80 + 0x70 * k);
            fx.ring_draw(g, x);
            fx.step_down(g + 0xB, 0, 8);
            if fx.approach(x, 2.0, tenth) == 0 {
                for o in [8u32, 4, 0] {
                    fx.mem.setf(x + o, tenth);
                }
                for (o, ra) in [(0x10u32, 0x01E0_39A0), (0x14, 0x01E0_39BC), (0x18, 0x01E0_39D8)] {
                    let r = fx.rand_from(s0, ra);
                    fx.mem.set32(x + o, r & 0xFFFF);
                }
                if n(fx) < 0x118 {
                    fx.mem.write_u8(g + 0xB, 0x80);
                }
            }
        }
    }
    if n(fx) >= 0x4E {
        for k in 0..2u32 {
            glow::line(fx, OT, D + 0x70 + 0x40 * k, D, sp + 0x90 + 0x10 * k, D + 0x1958);
        }
        if n(fx) >= 0x100 {
            for k in 0..3u32 {
                fx.step_down(D + 0x7B + 0x40 * k, 0, 8);
            }
        }
    }
    if n(fx) >= 0x5A {
        let c = (fx.rand_from(s0, 0x01E0_3AB8) & 0x3F) + 0x20;
        fx.mem.write_u8(D + 0x1944, c as u8);
        fx.mem.write_u8(D + 0x1945, c as u8);
        let a = fx.mem.read_u8(D + 0x1947);
        fx.mem.write_u8(D + 0x197F, 0x80u8.wrapping_sub(a));
        afterimage::strips(fx, D + 0x1940, D + 0x18F0);
        let h = fx.mem.i16(D + 0x194A);
        unit_tint(fx, h, D + 0x197C);
        if n(fx) >= 0x119 {
            fx.step_down(D + 0x1947, 0, 8);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x196C, m);
    let left = fx.mem.i32(D + 0x1968) - 1;
    fx.mem.set32(D + 0x1968, left);
    if m == 0x118 {
        2
    } else if m == 0x50 {
        1
    } else if (0x5A..0x118).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
