//! BMODULE b761 (card 365), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06A70, data area a0 = 0x1ED0000) [F] (helper B8, 31
//! Oct 2026): s0 = D, s1 = the OT, s2..s4 the parameter / loop counters.
//! On update 2 a long flash (+0x424) and a slow shake at the attacker's
//! node 0x2E and a rod (+0x5EC, `rod::rods`) from node 0x2D to 0x2E,
//! stretched on 164; while 2 <= n < 291 one (two from 165) small ribbons
//! (+0x624) a update between the nodes; on 155 two flashes, ribbon 0 and a
//! shake; on 184 an arc (+0xA4); on 283 the fade (+0x1E84), 30 sprays
//! (+0x1E8C) falling at the target, four bands (+0x29DC) there and four
//! scrolling tubes (+0x65BC, `tubes`, the record's own 0x1E06010) up from
//! it; on 285 six ribbons widening about the target, a wide flash (+0x470)
//! and a camera shake there. The stage light pulsed by 0x1031C0 (+0xA0),
//! the object ops of 0x1E07D50 (0x1F83B0). Returns 1 on update 273, 2 on
//! 280, 6 on 283..311, 8 on 344 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `rods`
//! 0x1E00410, `arcs` 0x1E015C0, `flash` 0x1E02840, `ribbon` 0x1E033F0,
//! `sprays` 0x1E03F80, `fade` 0x1E04870, `bands` 0x1E04DB0, `bounce`
//! 0x1E055A0 (its pool +0x306C never filled) [`crate::blib`].
use crate::b035::{light_ramp, object_ops};
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{aim_angles, camera_vp_into, countdown, first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x671C;
/// The object-op table (rodata).
const OPS: u32 = 0x01E0_7D50;
const PI: u32 = 0x4049_0FDB;

/// The entry 0x1E06A70 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x6820);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..3 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x2D, 1), (0x10, 0x2E, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x152);
    // The entry's s0 = D, s1 = the OT, s2 = 3 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_6B8C, [Some(D), Some(OT), Some(3)]);
    0
}

fn flash_fields(fx: &mut Fx, r: u32, colour: u8) {
    for (o, v) in [(0xCu32, colour), (0xD, colour), (0xE, colour), (0xA, 0x20), (9, 0x30), (8, 0x80), (0xF, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
}

/// Update 2 [F 0x1E06C14]: a long flash at node +0x10, the slow shake and
/// the rod (+0x5EC) from node +0xC to +0x10.
fn start(fx: &mut Fx) {
    let i = first_free(fx, D + 0x424, 0x4C, 6);
    if i >= 0 {
        let r = D + 0x424 + 0x4C * i as u32;
        set_counter4(fx, r, 0xB6, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0x10);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 4, 0x82);
        fx.mem.write_u8(r + 7, 9);
        fx.mem.write_u32(r + 0x10, 0x4316_0000);
        fx.mem.write_u32(r + 0x14, 0);
        flash_fields(fx, r, 0xC0);
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(r + o, 0);
        }
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x99, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(s + 6, b);
    fx.mem.write_u8(s + 4, 0);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3F99_999A);
    fx.mem.write_u32(s + 0x1C, 0x3F99_999A);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x4000_0000);
    let r = D + 0x5EC;
    set_counter4(fx, r, 0xFF, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 8, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(r + 9, b);
    fx.mem.write_u8(r + 4, 2);
    fx.mem.write_u32(r + 0x10, 0x3F00_0000);
    fx.mem.write_u32(r + 0x14, 0x3DCC_CCCD);
    fx.mem.write_u32(r + 0xC, 0x4248_0000);
    for (o, v) in [(0x1Bu32, 0xC0u8), (0x18, 0xC0), (0x19, 0xC0), (0x1A, 0xC0), (0x1E, 0), (0x1D, 0), (0x1C, 0x80), (0x1F, 0)] {
        fx.mem.write_u8(r + o, v);
    }
}

/// Update 155 [F 0x1E06DA8]: two flashes at node +0x10, ribbon 0 (+0x624)
/// and a shake there.
fn flashes(fx: &mut Fx) {
    for k in 0..2 {
        let i = first_free(fx, D + 0x424, 0x4C, 6);
        if i < 0 {
            continue;
        }
        let r = D + 0x424 + 0x4C * i as u32;
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0x10);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 7, 9);
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(r + o, 0);
        }
        if k != 0 {
            set_counter4(fx, r, 0x18, 0, 0xFF);
            fx.mem.write_u8(r + 4, 0x80);
            fx.mem.write_u32(r + 0x10, 0x4248_0000);
            fx.mem.write_u32(r + 0x14, 0x4248_0000);
            fx.mem.write_u32(r + 0x18, 0x41A0_0000);
            flash_fields(fx, r, 0xF0);
        } else {
            set_counter4(fx, r, 0x80, 0, 0xFF);
            fx.mem.write_u8(r + 4, 0x83);
            fx.mem.write_u32(r + 0x10, 0x4396_0000);
            fx.mem.write_u32(r + 0x14, 0);
            fx.mem.write_u32(r + 0x18, 0);
            flash_fields(fx, r, 0xC0);
        }
    }
    let r = D + 0x624;
    fx.mem.write_u8(r + 4, 5);
    set_counter4(fx, r, 0x18, 0, 0xFF);
    fx.mem.write_u8(r + 0x27, 0x40);
    fx.mem.write_u8(r + 0x23, 0);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(r + 6, b);
    for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 4), (8, 0x64)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(D, 0x01E0_6F5C);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
    fx.mem.write_u32(r + 0x14, 0x4020_0000);
    fx.mem.write_u32(D + 0x67C, 0x40);
    fx.mem.write_u32(D + 0x680, 0x80);
    fx.mem.write_u32(D + 0x684, 0x100);
    let s = D + 0x40;
    set_counter4(fx, s, 0x80, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(s + 6, b);
    fx.mem.write_u8(s + 4, 4);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
}

/// Update 184 [F 0x1E0703C]: the arc (+0xA4, points +0x104).
fn arc_start(fx: &mut Fx) {
    let g = D + 0xA4;
    set_counter4(fx, g, 0x6B, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x104);
    fx.mem.write_u8(g + 0x30, 0x32);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x4080_0000);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0xC0), (0x4A, 0), (0x49, 0), (0x48, 0x80)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0xC + 2 * a, -1);
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        a += 1;
    }
}

/// Update 283 [F 0x1E070F4]: the fade, 30 sprays above the target (sp +
/// 0x60), four bands and four tubes there (points +0x658C).
fn fall(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0x1E84, 0x12, 0, 0xFF);
    fx.mem.write_u8(D + 0x1E88, 1);
    for k in 0..0x1Ei32 {
        let i = first_free(fx, D + 0x1E8C, 0x60, 0x28);
        if i < 0 {
            continue;
        }
        let r = D + 0x1E8C + 0x60 * i as u32;
        set_counter4(fx, r, 0x28, (k >> 1) & 0xFF, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 2);
        fx.mem.write_u8(r + 8, 0x12);
        let x = fx.rand_from(D, 0x01E0_718C);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u8(r + 0xA, 0);
        let x = fx.rand_from(D, 0x01E0_71A4);
        fx.mem.set8(r + 0xB, (x >> 4) & 7);
        fx.mem.write_u8(r + 0xC, 0);
        fx.mem.write_u8(r + 0xD, 2);
        fx.mem.write_u32(r + 0x10, BYTES);
        let x = fx.rand_from(D, 0x01E0_71D0) >> 4;
        let (_, h) = fx.div(x, 5);
        let v = ee::add(18.0, ee::div(ee::from_i32(h * 200 - 0x1F4), 100.0));
        fx.mem.setf(r + 0x14, v);
        fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
        fx.mem.write_u32(r + 0x20, 0x4270_0000);
        fx.mem.write_u32(r + 0x24, 0);
        for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let v = ee::sub(2.0, ee::mul(f32::from_bits(0x3D75_C28F), ee::from_i32(k)));
        fx.mem.setf(r + 0x4C, v);
        let x = fx.mem.read_u32(sp + 0x60);
        fx.mem.write_u32(r + 0x3C, x);
        let y = ee::sub(fx.mem.f(sp + 0x64), 25.0);
        fx.mem.setf(r + 0x40, y);
        let z = fx.mem.read_u32(sp + 0x68);
        fx.mem.write_u32(r + 0x44, z);
    }
    for k in 0..4u32 {
        let r = D + 0x2D8C + 0x5C * k;
        set_counter4(fx, r, 0x22, k as i32, 0xFF);
        for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 1), (8, 1), (9, 2), (0xA, 6), (0xB, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x3F80_0000);
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0x3FC0_0000);
        fx.mem.write_u32(r + 0x1C, 0x3F19_999A);
        fx.mem.write_u32(r + 0x20, 0x3F00_0000);
        for (o, v) in [(0x25u32, 0x80u8), (0x26, 0x80), (0x24, 0x90), (0x29, 0x80), (0x2A, 0x80), (0x28, 0x90), (0x27, 0x10), (0x2B, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x60, r + 0x2C);
        let y = ee::sub(fx.mem.f(r + 0x30), 20.0);
        fx.mem.setf(r + 0x30, y);
    }
    for c in 0..3u32 {
        let v = fx.mem.read_u32(sp + 0x60 + 4 * c);
        fx.mem.write_u32(D + 0x6598 + 4 * c, v);
    }
    for c in 0..3u32 {
        let v = fx.mem.read_u32(D + 0x6598 + 4 * c);
        fx.mem.write_u32(D + 0x658C + 4 * c, v);
    }
    let y = ee::sub(fx.mem.f(D + 0x6590), 20.0);
    fx.mem.setf(D + 0x6590, y);
    fx.mem.setf(D + 0x659C, ee::add(80.0, y));
    for k in 0..4u32 {
        let r = D + 0x65BC + 0x2C * k;
        set_counter4(fx, r, 0x22, k as i32, 0xFF);
        for (o, v) in [(7u32, 0x12u8), (8, 1), (0xB, 0x40), (9, 2), (0xA, 2)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0xC, D + 0x658C);
        let (s, t, span) = if k & 1 != 0 { (0x3C23_D70Au32, 0x3CA3_D70Au32, 0x3F80_0000u32) } else { (0xBC23_D70A, 0x3CA3_D70A, 0x4000_0000) };
        fx.mem.write_u32(r + 0x18, s);
        fx.mem.write_u32(r + 0x1C, t);
        fx.mem.write_u32(r + 0x10, span);
        fx.mem.write_u32(r + 0x20, 0x3F80_0000);
        for o in [0x28u32, 0x29, 0x2A, 0x2B] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u32(r + 0x24, 0x3F99_999A);
    }
}

/// Update 285 [F 0x1E0754C]: six ribbons, the flash +0x470 and the shake
/// at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    for k in 0..6i32 {
        let i = first_free(fx, D + 0x624, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let r = D + 0x624 + 0x68 * i as u32;
        fx.mem.write_u8(r + 4, 4);
        set_counter4(fx, r, 0x28, 0, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(0x10);
        fx.mem.write_u8(r, c);
        for (o, v) in [(0x27u32, 0x60u8), (0x23, 0x28), (5, 0), (0xA, 0), (0xB, 2), (0xC, 4), (8, 0x50)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(D, 0x01E0_75F8);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0x60, r + 0x28);
        let y = ee::sub(fx.mem.f(r + 0x2C), 20.0);
        fx.mem.setf(r + 0x2C, y);
        for o in [0x50u32, 0x4C, 0x48] {
            fx.mem.write_u32(r + o, 0);
        }
        let w = ee::add(20.0, ee::mul(5.0, ee::from_i32(k + 1)));
        fx.mem.setf(r + 0x1C, w);
        let f2 = ee::from_i32(k);
        fx.mem.setf(r + 0x14, ee::sub(f32::from_bits(0x400C_CCCD), ee::mul(f32::from_bits(0x3E99_999A), f2)));
        let q = f32::from_bits(0x3ECC_CCCD);
        fx.mem.setf(r + 0x18, ee::add(q, ee::mul(q, f2)));
    }
    let r = D + 0x470;
    set_counter4(fx, r, 0x28, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    for (o, v) in [(0xFu32, 0xD0u8), (0xC, 0xD0), (0xD, 0xD0), (0xE, 0xD0), (0xA, 0x60), (9, 0x10), (8, 0x40)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0);
    fx.mem.write_u32(r + 0x10, 0x44C8_0000);
    copy_v4(fx, sp + 0x60, r + 0x1C);
    let y = ee::sub(fx.mem.f(r + 0x20), 20.0);
    fx.mem.setf(r + 0x20, y);
    let s = D + 0x40;
    set_counter4(fx, s, 0x28, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    for o in [0x14u32, 0x18, 0x1C] {
        fx.mem.write_u32(s + o, 0x4000_0000);
    }
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x60, D + 0x60);
}

/// Updates 2..290 [F 0x1E0781C]: one ribbon (two from 165) from node +0x10
/// towards +0xC. A full pool (0x101C30 -1) writes the record before the
/// pool (D + 0x5BC) as the original does.
fn trickle(fx: &mut Fx, n: i32) {
    let count = if n < 0xA5 { 1 } else { 2 };
    for k in 0..count {
        let i = first_free(fx, D + 0x624, 0x68, 0x3C);
        let r = (D + 0x624).wrapping_add((0x68 * i) as u32);
        if i >= 0 {
            let (a, b, c) = if count == 1 {
                (0x3F33_3333u32, 0x3F19_999Au32, 0x4120_0000u32)
            } else if k != 0 {
                (0x3FB3_3333, 0x3F33_3333, 0x41A0_0000)
            } else {
                (0x3F80_0000, 0x3F99_999A, 0x41A0_0000)
            };
            fx.mem.write_u32(r + 0x14, a);
            fx.mem.write_u32(r + 0x18, b);
            fx.mem.write_u32(r + 0x1C, c);
        }
        set_counter4(fx, r, 0xC, 0, 0xFF);
        fx.mem.write_u8(r + 8, 0x20);
        fx.mem.write_u8(r + 4, 0xC);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0x10);
        fx.mem.write_u8(r + 6, b);
        let b = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(r + 0xE, b);
        for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 3)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(D, 0x01E0_79C0);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x23, 0x20);
        fx.mem.write_u8(r + 0x27, 0x80);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_6BB4);
    match fx.mem.u16(D) {
        0x158 => return 8,
        0x11D => strike(fx, sp),
        0x11B => fall(fx, sp),
        0xB8 => arc_start(fx),
        0xA4 => {
            set_counter4(fx, D + 0x5EC, 0x7F, 0, 0xFF);
            fx.mem.write_u32(D + 0x5FC, 0x4080_0000);
            fx.mem.write_u32(D + 0x5F8, 0x42C8_0000);
        }
        0x9B => flashes(fx),
        2 => start(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0x123).contains(&n) {
        trickle(fx, n);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_7A1C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_7A2C, [Some(u64::from(D)), Some(u64::from(OT))]);
    let n = fx.mem.u16(D);
    object_ops(fx, OPS, n);
    // The rod's extensions take the caller's f22: the entry never sets it
    // (the runner's 0) [run].
    blib::rod::rods(fx, 0x01E0_0410, D + 0x5EC, OT, 1, 0.0);
    blib::arc::arcs(fx, 0x01E0_15C0, D + 0xA4, OT, 1);
    blib::star::flash(fx, 0x01E0_2840, D + 0x424, OT, 6);
    blib::streak::ribbon(fx, 0x01E0_33F0, D + 0x624, OT, 0x3C);
    blib::fade::fade(fx, D + 0x1E84, OT);
    blib::band::bands(fx, 0x01E0_4DB0, D + 0x2D8C, OT, 8);
    blib::spray::sprays(fx, 0x01E0_3F80, D + 0x1E8C, OT, 0x28);
    blib::sprite::bounce(fx, D + 0x306C, OT, 0xC8);
    tubes(fx, D + 0x65BC, OT, 8);
    let n = fx.mem.u16(D);
    if (0x11B..0x138).contains(&n) {
        6
    } else if n == 0x118 {
        2
    } else if n == 0x111 {
        1
    } else {
        0
    }
}

/// 0x1A2B80 of `v` from return address `ra` (its frame left).
fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

/// 0x1E06010 (ae37ae16e23f, this record's own) [F]: `tubes(recs, ot, n)
/// -> 1` steps and draws `n` records (0x2C bytes; a 0x480 frame, the
/// descriptor at sp + 0x3F0: RGBAQ / ST / XYZF2 x 4). Per record:
/// countdown 0x101AF0; the texture (5, +9); radius +0x20 += +0x24; a ring
/// of k = min(+7, 36) points (libm cosf / sinf, plane by +0xB & 0x18: 0x10
/// the yz plane, 8 xy, else xz) closed by a copy of the first; S spans
/// +0x10 over the ring, scrolled by the fractions of +0x18 / +0x1C x the
/// elapsed count (0x1A2B80). +8 rows (with flag 0x20 at most the elapsed
/// count), row r between points r / r + 1 of the list +0xC (12 bytes
/// each; their difference's aim 0x101940 / length 0x20C850 computed and
/// dropped); each ring step a quad of the two rings' points (all four
/// must project, 0x204D80); flag 1: a front-facing quad (screen cross >
/// 0) ends the row. STQ / w, REGION_CLAMP 0 (0x208400), ALPHA 0x48, colour
/// +0x28.. with alpha +0x2B (x count / 8 over the last 8 unless +4 bit
/// 0x40; 0 on odd counts with +0xB flag 2; the far ring's 0 with +0xB
/// 0x40), the texture (0x207E50, filter 1); keyed by the mean w (0 with +4
/// bit 0x80) through 0x1A2C70 twice.
pub(crate) fn tubes(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x480);
    fx.mem.set32(sp + 0x118, ot as i32);
    fx.mem.set32(sp + 0x114, n);
    let mut g = recs;
    let mut i = 0;
    fx.mem.set32(sp + 0xB0, i);
    while i < n {
        if i != 0 {
            g += 0x2C;
        }
        tube(fx, sp, g, ot);
        i += 1;
        fx.mem.set32(sp + 0xB0, i);
    }
    fx.leave(0x480);
    1
}

fn tube(fx: &mut Fx, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 9);
    let t = fx.texture(5, id);
    fx.mem.write_u32(sp + 0xC0, t);
    let r = ee::add(fx.mem.f(g + 0x20), fx.mem.f(g + 0x24));
    fx.mem.setf(g + 0x20, r);
    let mut k = fx.mem.u8(g + 7);
    if k >= 0x25 {
        k = 0x24;
    }
    fx.mem.set32(sp + 0xF0, k);
    let step = ee::div(360.0, ee::from_i32(k));
    for j in 0..k {
        let plane = fx.mem.u8(g + 0xB) & 0x18;
        let a = ee::div(ee::mul(f32::from_bits(PI), ee::mul(step, ee::from_i32(j))), 180.0);
        let p = sp + 0x160 + 16 * j as u32;
        let c = libm_cosf(fx, a);
        let c = ee::mul(fx.mem.f(g + 0x20), c);
        let (co, so, zo) = match plane {
            0x10 => (8u32, 4u32, 0u32),
            8 => (0, 4, 8),
            _ => (0, 8, 4),
        };
        fx.mem.setf(p + co, c);
        let s = fx.libm_sinf(a);
        let s = ee::mul(fx.mem.f(g + 0x20), s);
        fx.mem.setf(p + so, s);
        fx.mem.write_u32(p + zo, 0);
    }
    copy_v4(fx, sp + 0x160, sp + 0x160 + 16 * k as u32);
    let su = ee::div(fx.mem.f(g + 0x10), ee::from_i32(k));
    let e = fx.mem.u8(g + 2) - fx.mem.u8(g);
    let mut s0 = ee::mul(fx.mem.f(g + 0x18), ee::from_i32(e));
    if !ee_le(s0, 1.0) {
        let w = fix(fx, s0, 0x01E0_635C);
        s0 = ee::sub(s0, ee::from_i32(w));
    }
    let mut t0 = ee::mul(fx.mem.f(g + 0x1C), ee::from_i32(e));
    if !ee_le(t0, 1.0) {
        let w = fix(fx, t0, 0x01E0_63AC);
        t0 = ee::sub(t0, ee::from_i32(w));
    }
    let d = sp + 0x3F0;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    let mut rows = fx.mem.u8(g + 8);
    fx.mem.set32(sp + 0x100, rows);
    if fx.mem.u8(g + 0xB) & 0x20 != 0 {
        let e = fx.mem.u8(g + 2) - fx.mem.u8(g);
        if e < rows {
            rows = e;
            fx.mem.set32(sp + 0x100, rows);
        }
    }
    let mut row = 0;
    fx.mem.set32(sp + 0xD0, row);
    while row < fx.mem.i32(sp + 0x100) {
        let q = 12 * row as u32;
        for c in 0..3u32 {
            let pts = fx.mem.read_u32(g + 0xC).wrapping_add(q);
            let v = ee::sub(fx.mem.f(pts + 0xC + 4 * c), fx.mem.f(pts + 4 * c));
            fx.mem.setf(sp + 0x3C0 + 4 * c, v);
        }
        aim_angles(fx, sp + 0x3C0, sp + 0x3E0);
        blib::beam::length3(fx, sp + 0x3C0);
        camera_vp_into(fx, sp + 0x120);
        let mut j = 0;
        while j < fx.mem.i32(sp + 0xF0) {
            if !quad(fx, sp, g, ot, q, j, su, s0, t0) {
                break;
            }
            j += 1;
        }
        row += 1;
        fx.mem.set32(sp + 0xD0, row);
    }
}

/// Ring step `j` of the row at point offset `q`: false when a front-facing
/// quad ends the row.
#[allow(clippy::too_many_arguments)]
fn quad(fx: &mut Fx, sp: u32, g: u32, ot: u32, q: u32, j: i32, su: f32, s0: f32, t0: f32) -> bool {
    let ring = sp + 0x160 + 16 * j as u32;
    let mut ok = [false; 4];
    for (v, (po, ro, xyz, w)) in [(0u32, 0u32, 0x410u32, 0x3D0u32), (0, 0x10, 0x428, 0x3D4), (0xC, 0, 0x440, 0x3D8), (0xC, 0x10, 0x458, 0x3DC)].into_iter().enumerate() {
        for c in 0..3u32 {
            let pts = fx.mem.read_u32(g + 0xC).wrapping_add(q);
            let a = ee::add(fx.mem.f(pts + po + 4 * c), fx.mem.f(ring + ro + 4 * c));
            fx.mem.setf(sp + 0x3B0 + 4 * c, a);
        }
        let m = fx.read_m4(sp + 0x120);
        ok[v] = fx.project_at(sp + 0x3B0, &m, sp + xyz, sp + w);
        if v == 0 {
            fx.mem.set32(sp + 0xE8, i32::from(ok[0]));
        } else if v == 1 {
            fx.mem.set32(sp + 0xEC, i32::from(ok[1]));
        }
    }
    if ok.contains(&false) {
        return true;
    }
    if fx.mem.u8(g + 0xB) & 1 != 0 {
        let (cx, cy) = (fx.mem.u16(sp + 0x440), fx.mem.u16(sp + 0x442));
        let (dx, dy) = (fx.mem.u16(sp + 0x410) - cx, fx.mem.u16(sp + 0x45A) - cy);
        let a = fx.mult(dx, dy);
        let b = (fx.mem.u16(sp + 0x458) - cx).wrapping_mul(fx.mem.u16(sp + 0x412) - cy);
        if !ee_le(ee::from_i32(a.wrapping_sub(b)), 0.0) {
            return false;
        }
    }
    let (w0, w1, w2, w3) = (fx.mem.f(sp + 0x3D0), fx.mem.f(sp + 0x3D4), fx.mem.f(sp + 0x3D8), fx.mem.f(sp + 0x3DC));
    fx.mem.setf(sp + 0x404, ee::div(1.0, w0));
    fx.mem.setf(sp + 0x41C, ee::div(1.0, w1));
    fx.mem.setf(sp + 0x434, ee::div(1.0, w2));
    fx.mem.setf(sp + 0x44C, ee::div(1.0, w3));
    let sa = ee::add(s0, ee::mul(su, ee::from_i32(j)));
    fx.mem.setf(sp + 0x408, ee::div(sa, w0));
    let ta = ee::add(1.0, t0);
    fx.mem.setf(sp + 0x40C, ee::div(ta, w0));
    let sb = ee::add(s0, ee::mul(su, ee::from_i32(j + 1)));
    fx.mem.setf(sp + 0x420, ee::div(sb, w1));
    fx.mem.setf(sp + 0x424, ee::div(ta, w1));
    fx.mem.setf(sp + 0x438, ee::div(sa, w2));
    fx.mem.setf(sp + 0x43C, ee::div(t0, w2));
    fx.mem.setf(sp + 0x450, ee::div(sb, w3));
    fx.mem.setf(sp + 0x454, ee::div(t0, w3));
    let d = sp + 0x3F0;
    fx.desc_clamp(d, 0);
    fx.desc_alpha(d, 0x48);
    let count = fx.mem.u8(g);
    let alpha = if fx.mem.u8(g + 4) & 0x40 == 0 && count < 8 {
        let a = fx.mem.u8(g + 0x2B) >> 3;
        fx.mult(a, count) & 0xFF
    } else {
        fx.mem.u8(g + 0x2B)
    };
    let alpha = if fx.mem.u8(g + 0xB) & 2 != 0 && count & 1 != 0 { 0 } else { alpha };
    for o in [0x44Bu32, 0x433, 0x41B, 0x403] {
        fx.mem.set8(sp + o, alpha);
    }
    if fx.mem.u8(g + 0xB) & 0x40 != 0 {
        fx.mem.write_u8(sp + 0x44B, 0);
        fx.mem.write_u8(sp + 0x433, 0);
    }
    for c in 0..3u32 {
        let v = fx.mem.read_u8(g + 0x2A - c);
        for o in [0x448u32, 0x430, 0x418, 0x400] {
            fx.mem.write_u8(sp + o + c, v);
        }
    }
    for o in [0x45Fu32, 0x447, 0x42F, 0x417] {
        fx.mem.write_u8(sp + o, 0);
    }
    let t = fx.mem.read_u32(sp + 0xC0);
    fx.desc_texture(d, t, 1);
    let f = if fx.mem.u8(g + 4) & 0x80 != 0 {
        0.0
    } else {
        let w = ee::div(ee::add(w3, ee::add(w2, ee::add(w0, w1))), 4.0);
        fx.fix_residue(w, 0x01E0_6968);
        let k = ee::fixunssfsi(w);
        if (k as i32) < 0 {
            let h = ee::from_i32(((k >> 1) | (k & 1)) as i32);
            ee::add(h, h)
        } else {
            ee::from_i32(k as i32)
        }
    };
    fx.fix_residue(f, 0x01E0_69A8);
    let key = ee::fixunssfsi(f) as i32;
    fx.submit(d, ot, key);
    true
}
