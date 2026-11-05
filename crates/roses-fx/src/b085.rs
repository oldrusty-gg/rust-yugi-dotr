//! BMODULE b085 (card 301), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E044D0, data area a0 = 0x1ED0000) [F] (helper B5, 31
//! Oct 2026): b076's "a0 = D" family (s0 = D, s1 = the OT). Node bursts
//! (+0xA4, `burst::node_bursts`, specs 0x1E04EC0 / 0x1E04ED0, motes at
//! +0xE8 / +0x628) from nodes +0x14 / +0x1C, from update 18 nodes +0xC /
//! +0x10; on updates 60 and 69 (a strike from above, then from below) the
//! fade (+0x1240), a ribbon (+0xE90), four band rings (+0xF60,
//! `band::bands`), 150 bouncing sprites (+0x1248), a wide flash (+0xDF8)
//! and a short camera shake, all at the target 6 above / below; the stage
//! light pulsed by 0x1031C0 (+0xA0). Returns 1 on update 50, 6 on 60..97,
//! 2 on 98, 8 on 120 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `flash` 0x1E015F0, `ribbon`
//! 0x1E021A0, `bands` 0x1E02D30, `bounce` 0x1E03520, `fade` 0x1E03F90
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x61F8;
const SPRITES: u32 = D + 0x1248;

/// The entry 0x1E044D0 (a0 = D, a1 = param; s0 = D, s1 = the OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x6300);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(4, 2);
    for k in 0..4 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x4A, 1), (0x10, 0x55, 0), (0x14, 0x36, 0), (0x18, 0x33, 0), (0x1C, 0x41, 0), (0x20, 0x3E, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): specs 0x1E04EC0 / 0x1E04ED0, motes at
    // +0xE8 / +0x628, nodes +0x14 / +0x1C, updates 45..55.
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xCC, b);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_4EC0);
    fx.mem.write_u32(D + 0xB0, 0x01E0_4ED0);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x18);
    fx.mem.write_u32(D + 0xA8, D + 0x628);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(D + 0xD7, b);
    fx.mem.set16(D + 0xD8, 0x2D);
    fx.mem.set16(D + 0xDA, 0x37);
    fx.mem.write_u32(D + 0xDC, 0x435C_0000);
    fx.mem.write_u32(D + 0xE0, 0x435C_0000);
    fx.mem.write_u32(D + 0xE4, 0x4100_0000);
    fx.mem.set16(D + 0xA0, 0x36);
    fx.mem.set16(D + 0xA2, 0x72);
    // The entry's s0 = D, s1 = the OT, s2 = 4 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_46D4, [Some(D), Some(OT), Some(4)]);
    0
}

/// ((rand >> 4) % 180 - 90) / 100 from return address `ra`.
fn spread(fx: &mut Fx, ra: u32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let (_, m) = fx.div(x, 0xB4);
    ee::div(ee::from_i32(m - 0x5A), 100.0)
}

/// Updates 60 / 69 [F 0x1E04750] (`k` 0 / 1): the fade, the strike point
/// sp + 0x60 = the target 6 above / below, a ribbon, four band rings
/// (turning one way / the other, reversed for a mirrored attacker), 150
/// bouncing sprites, a wide flash and the shake.
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0x1240, 8, 0, 0xFF);
    fx.mem.write_u8(D + 0x1244, 1);
    copy_v4(fx, sp + 0x70, sp + 0x60);
    let k = u32::from(fx.mem.u16(D) != 0x3C);
    let y = fx.mem.f(sp + 0x64);
    let y = if k == 0 { ee::add(y, 6.0) } else { ee::sub(y, 6.0) };
    fx.mem.setf(sp + 0x64, y);
    let r = D + 0xE90 + 0x68 * k;
    set_counter4(fx, r, 0x18, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4040_0000);
    fx.mem.write_u32(r + 0x1C, 0x4270_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 2), (0xB, 2), (0xC, 5), (8, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(D, 0x01E0_483C);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, sp + 0x60, r + 0x28);
    fx.mem.set32(r + 0x58, 0x40);
    fx.mem.set32(r + 0x5C, 0x80);
    fx.mem.set32(r + 0x60, 0x100);
    for j in 0..4u32 {
        let r = D + 0xF60 + 0x5C * (j + 4 * k);
        set_counter4(fx, r, 0x1C, j as i32, 0xFF);
        for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 1), (8, 1), (9, 2), (0xA, 6), (0xB, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x3F80_0000);
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0x4000_0000);
        fx.mem.write_u32(r + 0x1C, 0x3F80_0000);
        fx.mem.write_u32(r + 0x20, 0x3E80_0000);
        for (o, v) in [(0x26u32, 0xA0u8), (0x24, 0x60), (0x25, 0x60), (0x2A, 0xA0), (0x28, 0x60), (0x29, 0x60), (0x27, 0x10), (0x2B, 0x30)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x60, r + 0x2C);
        fx.mem.set32(r + 0x3C, 0x2800);
        fx.mem.set32(r + 0x40, if k == 1 { -0x5000 } else { 0x5000 });
        if fx.mem.i32(D + 0x30) != 0x8000 {
            let v = fx.mem.i32(r + 0x40).wrapping_neg();
            fx.mem.set32(r + 0x40, v);
        }
    }
    for _ in 0..0x96 {
        let i = first_free(fx, SPRITES, 0x44, 0x12C);
        if i < 0 {
            continue;
        }
        let x = fx.rand_from(D, 0x01E0_4A40) >> 4;
        let b = SPRITES + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, x & 0xF, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 3), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(b + o, v);
        }
        let x = fx.rand_from(D, 0x01E0_4A90) >> 4;
        fx.mem.set8(b + 9, x & 7);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(b + o, 0x80);
        }
        fx.mem.write_u32(b + 0x10, 0x4316_0000);
        fx.mem.write_u32(b + 0x14, 0xC000_0000);
        fx.mem.write_u32(b + 0x18, 0xBD23_D70A);
        fx.mem.write_u32(b + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        let v = spread(fx, 0x01E0_4AF4);
        fx.mem.setf(b + 0x2C, v);
        let v = spread(fx, 0x01E0_4B40);
        fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3ECC_CCCD), v));
        let v = spread(fx, 0x01E0_4B9C);
        fx.mem.setf(b + 0x34, v);
    }
    let r = D + 0xDF8 + 0x4C * k;
    set_counter4(fx, r, 0x18, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x60), (9, 0x20), (8, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, 0x43FA_0000);
    fx.mem.write_u32(r + 0x14, 0);
    copy_v4(fx, sp + 0x60, r + 0x1C);
    // The shake's count is the 0x20 left in a1 by the flash's +9 store
    // [F 0x1E04C58].
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FB3_3333);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4270_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x70, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x70, u64::from(D), 0x01E0_46FC);
    match fx.mem.u16(D) {
        0x78 => return 8,
        0x45 | 0x3C => strike(fx, sp),
        0x12 => {
            let b = fx.mem.read_u8(D + 0xC);
            fx.mem.write_u8(D + 0xCC, b);
            let b = fx.mem.read_u8(D + 0x10);
            fx.mem.write_u8(D + 0xCD, b);
        }
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_4D30, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4D40, [Some(u64::from(D)), Some(u64::from(OT))]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x18);
    blib::fade::fade(fx, D + 0x1240, OT);
    blib::star::flash(fx, 0x01E0_15F0, D + 0xDF8, OT, 2);
    blib::streak::ribbon(fx, 0x01E0_21A0, D + 0xE90, OT, 2);
    blib::band::bands(fx, 0x01E0_2D30, D + 0xF60, OT, 8);
    blib::sprite::bounce(fx, SPRITES, OT, 0x12C);
    let n = fx.mem.u16(D);
    if (0x3C..0x62).contains(&n) {
        6
    } else if n == 0x62 {
        2
    } else if n == 0x32 {
        1
    } else {
        0
    }
}
