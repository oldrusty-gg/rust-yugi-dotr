//! BMODULE b719 (card 77), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02E10, data area a0 = 0x1ED0000) [F] (helper B9, 31
//! Oct 2026): b391's program with a second zoom (+0x5D0, the target at
//! +0x580 in billboard +0x560) and the colours switched at update 190.
//! The stage dims (+0x2708) until update 291; 256 sparks (+0x16C0, the
//! TEXEFF cell sheet) rising round the attacker (+0x1650) and respawning
//! above 250, their bit 0x400 cleared on 140 / 240 and set on 190; a frame
//! quad (0x80, 0x80, 0x80, 0x60) every update; on 140..189 and from 240 a
//! full-window tint (+0x26F0, alpha 0x30, 0xA0 on 290, then fading); from
//! update 50 a disc (+0xE0), four rings (+0x650 + 0x400 k) in billboard
//! +0x5E0, a quad burst (+0x100) and the two zooms. Returns 1 on update
//! 130, 6 on 140..289, 2 on 290, 8 after 310.
//!
//! Library (record addresses): as b391's, `quads_setup`'s template at
//! 0x1E03780.
use crate::b262::tint;
use crate::b391::spark_place;
use crate::blib::{spawn, trail, zoom};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad corner template (rodata, 0x1E026A0's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_3780;

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

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2710);
    for x in [0u32, 0x70, 0x1650] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2700u32, 0x12, 1), (0x2704, 1, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x26F4, 0x136);
    fx.mem.set32(D + 0x26F8, 1);
    fx.mem.set32(D + 0x26FC, param);
    fx.mem.set32(D + 0x2708, 0x80);
    bytes(fx, &[(0x26F2, 0x80), (0x26F1, 0x80), (0x26F0, 0x20), (0x26F3, 0x80)]);
    // The disc (+0xE0): radius 60.
    fx.mem.write_u32(D + 0xE0, 0);
    bytes(fx, &[(0xE6, 0xA0), (0xE5, 0xD0), (0xE4, 0x70), (0xE7, 0)]);
    fx.mem.write_u32(D + 0xE8, 0x4270_0000);
    halves(fx, &[(0xEC, 0x24), (0xEE, 0)]);
    // The zooms of the attacker (+0xF0) and the target (+0x5D0).
    for (g, h) in [(0xF0u32, true), (0x5D0, false)] {
        fx.mem.write_u32(D + g, 0);
        bytes(fx, &[(g + 6, 0x80), (g + 5, 0x80), (g + 4, 0x60), (g + 7, 0)]);
        fx.mem.write_u32(D + g + 8, 0x4220_0000);
        halves(fx, &[(g + 0xC, 6)]);
        let u = if h { fx.self_handle() } else { fx.target_handle() };
        halves(fx, &[(g + 0xE, u)]);
    }
    // The quad burst (+0x100): size 2.4.
    fx.texture4(sp + 0xA0, 0xF);
    fx.mem.write_u32(D + 0x104, 0x4_0001);
    halves(fx, &[(0x110, 0x8E), (0x112, 0), (0x114, 0x22), (0x116, 0x40)]);
    bytes(fx, &[(0x10A, 0xA0), (0x109, 0xD0), (0x108, 0x70), (0x10B, 0)]);
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.quads_setup(D + 0x100, tex, f32::from_bits(0x4019_999A), QUAD_TEMPLATE);
    // Four rings (+0x650 + 0x400 k).
    for k in 0..4u32 {
        let g = 0x650 + 0x400 * k;
        fx.mem.write_u32(D + g + 4, 0x2000_0000);
        halves(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x80)]);
        bytes(fx, &[(g + 0xA, 0xA0), (g + 9, 0xD0), (g + 8, 0x70), (g + 0xB, 0)]);
        let outer = ee::mul(6.0, ee::from_i32(k as i32 + 1));
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(D + g, tex, outer, ee::sub(outer, 4.0), 0.0, 0.0);
    }
    // The sparks (+0x16C0): 256 at +0x16F0, cells of the same sheet.
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x16C0, tex);
    fx.mem.write_u32(D + 0x16C4, 0x6000_0403);
    bytes(fx, &[(0x16CA, 0xA0), (0x16C9, 0xD0), (0x16C8, 0x70), (0x16CB, 0x80)]);
    fx.mem.write_u32(D + 0x16E0, 0x3ECC_CCCD);
    fx.mem.write_u32(D + 0x16E4, 0x41B0_0000);
    halves(fx, &[(0x16D0, 0xB0), (0x16D2, 0x40), (0x16D4, 0x20), (0x16D6, 0x20), (0x16D8, 0xB0), (0x16DA, 0x40), (0x16DC, 0x100), (0x16DE, 0x100)]);
    fx.mem.write_u32(D + 0x16CC, D + 0x16F0);
    fx.mem.write_u32(D + 0x16E8, 0x41F0_0000);
    halves(fx, &[(0x16EC, 0x100)]);
    bytes(fx, &[(0x16EE, 1), (0x16EF, 0)]);
    spawn::scatter(fx, D + 0x16C0, 0xC, 3.0);
    let mut k = 0u32;
    while (k as i32) < fx.mem.i16(D + 0x16EC) {
        spark_place(fx, D + 0x16CC, k, 0x9C4, [0x01E0_3120, 0x01E0_313C, 0x01E0_3168]);
        k += 1;
    }
    fx.mem.write_u32(D + 0x74, 0x3FC0_0000);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x26F8);
    let look = n(fx) < 0x6F;
    if look {
        let h = fx.self_handle();
        fx.unit_point(1, h, D + 0x1670);
        let t = fx.target_handle();
        fx.unit_position(t, sp + 0x80);
        let h = fx.self_handle();
        fx.unit_point(1, h, sp + 0x90);
        fx.look_angles(sp + 0x80, sp + 0x90, D + 0x1660);
    }
    for out in [0x90u32, 0x600] {
        let h = fx.self_handle();
        fx.unit_point(1, h, D + out);
    }
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x580);
    // s0 = D, s1 = the OT, s2 = sp + 0x90 (the look branch) or the
    // parameter (-1, 64 bits), s3 the runner's 0.
    let s2 = if look { u64::from(sp + 0x90) } else { u64::MAX };
    let regs = [Some(D), Some(OT), None, Some(0)];
    for (x, ra, bill) in [(0u32, 0x01E0_32B0, false), (0x70, 0x01E0_32B8, true), (0x5E0, 0x01E0_32C0, true), (0x1650, 0x01E0_32C8, false), (0x560, 0x01E0_32D0, true)] {
        if bill {
            fx.xform_billboard_saving(D + x, ra, regs);
            fx.saved(sp - 0xF0, &[(0x40, s2)]);
        } else {
            fx.xform_world_saving(D + x, ra, regs);
            fx.saved(sp - 0xD0, &[(0x40, s2)]);
        }
    }
    fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    if n(fx) >= 0x123 {
        fx.step_up(D + 0x2708, 0x80, 4);
    } else {
        fx.step_down(D + 0x2708, 0x20, 4);
    }
    let l = fx.mem.read_u8(D + 0x2708);
    fx.set_stage_light(i32::from(l));
    let m = n(fx);
    if (0x8C..0xBE).contains(&m) || m >= 0xF0 {
        if m == 0x122 {
            fx.mem.write_u8(D + 0x26F3, 0xA0);
        } else if m < 0x122 {
            fx.mem.write_u8(D + 0x26F3, 0x30);
        }
        tint(fx, D + 0x26F0);
        if n(fx) >= 0x122 {
            fx.step_down(D + 0x26F3, 0, 0xA);
        }
    }
    if n(fx) == 0xBE {
        bytes(fx, &[(0x10A, 0x10), (0xE6, 0x10), (0x16CA, 0x10), (0x109, 0x10), (0xE5, 0x10), (0x16C9, 0x10)]);
        bytes(fx, &[(0x108, 0x80), (0xE4, 0x80), (0x16C8, 0x80)]);
        bytes(fx, &[(0x5D6, 0x30), (0xF6, 0x30), (0x5D5, 0x30), (0xF5, 0x30), (0x5D4, 0x80), (0xF4, 0x80)]);
        bytes(fx, &[(0x26F2, 0x20), (0x26F1, 0x20), (0x26F0, 0x80)]);
    }
    let m = n(fx);
    if m == 0x8C || m == 0xF0 {
        let f = fx.mem.read_u32(D + 0x16C4) & !0x400;
        fx.mem.write_u32(D + 0x16C4, f);
    } else if m == 0xBE {
        let f = fx.mem.read_u32(D + 0x16C4) | 0x400;
        fx.mem.write_u32(D + 0x16C4, f);
    }
    trail::sparks(fx, OT, D + 0x16C0, D + 0x1650);
    if n(fx) >= 0x123 {
        fx.step_down(D + 0x16CB, 0, 8);
    }
    let mut k = 0u32;
    while (k as i32) < fx.mem.i16(D + 0x16EC) {
        let p = D + 0x10 * k;
        if fx.mem.i16(p + 0x16FE) >= 0 {
            let y = ee::add(fx.mem.f(p + 0x16F4), 12.0);
            fx.mem.setf(p + 0x16F4, y);
            if !ee_le(y, 250.0) {
                spark_place(fx, D + 0x16CC, k, 0x3C, [0x01E0_34CC, 0x01E0_34E8, 0x01E0_3514]);
                fx.mem.set16(p + 0x16FE, 0);
            }
        }
        k += 1;
    }
    if n(fx) >= 0x32 {
        fx.disc(D + 0xE0, D + 0x70);
        fx.step_up(D + 0xE7, 0xFF, 0x10);
        for k in 0..4u32 {
            fx.ring_draw(D + 0x650 + 0x400 * k, D + 0x5E0);
            fx.step_up(D + 0x65B + 0x400 * k, 0x40, 4);
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
        zoom::sprites(fx, 0x01E0_0280, D + 0x5D0, D + 0x580);
        if n(fx) >= 0x122 {
            fx.step_down(D + 0x5D7, 0, 4);
        } else {
            fx.step_up(D + 0x5D7, 0x40, 4);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x26F8, m);
    let left = fx.mem.i32(D + 0x26F4) - 1;
    fx.mem.set32(D + 0x26F4, left);
    if m == 0x122 {
        2
    } else if m == 0x82 {
        1
    } else if (0x8C..0x122).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
