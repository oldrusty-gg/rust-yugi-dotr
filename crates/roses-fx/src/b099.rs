//! BMODULE b099 (card 161), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05180, data area a0 = 0x1ED0000, 0xA560 bytes)
//! [F]: b629's family with s0 = D, s2 = the OT
//! (0x206E40), s1 the parameter then counters / offsets. Update 2 the
//! attacker's node 0x16 read (+0xA44C, 5 lower); 30 a flash (+0x9DDC,
//! 0x4C apart) held on the attacker, a ribbon (+0xAC, 0x68 apart) and a
//! long shake (+0x40); 40 a second held flash (+0x9E28) whose flags +4 go
//! to 3 on 102 and gain 0x80 on 143; 176 eighty sprites (+0x190C, 0x44
//! apart, `sprite::bounce`) round the target; 177 the frame fade (+0xA104,
//! `window::frame_fade`, 177..217), the attacker's streaks (+0xA134,
//! `radiate::streaks`), eight bands (+0xA16C, `band::bands`), a ribbon and
//! 300 sprites at the target; on 102..181 two sprites a update from each
//! of nodes 0x16 / 0xC; on 40..176 twelve sprites a update thrown from
//! node 0x37 at random turns (0x20C0B0 / 0x20C540); on even 65..176 a
//! ribbon; on every eighth 80..169 the attacker's tint (object op
//! 0x1F83B0); the smears (+0x9E74) are stepped but never set; the stage
//! light pulsed by 0x1031C0 (+0xA0). Returns 6 on 177..205, 2 on 206, 1 on
//! 167, 8 on 320 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `fade`
//! 0x1E00410, `flash` 0x1E00950, `ribbon` 0x1E01500, `bounce` 0x1E02090,
//! `smears_c` 0x1E02B00, `bands` 0x1E03690, `streaks` 0x1E03E80,
//! `frame_fade` 0x1E04CA0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0xA460;
const NODE_Y: u32 = D + 0xA450;
const TINT: u32 = D + 0xA45C;
const SPRITES: u32 = D + 0x190C;
const FLASH: u32 = D + 0x9DDC;
const FLASH1: u32 = D + 0x9E28;
const SMEARS: u32 = D + 0x9E74;
const RIBBONS: u32 = D + 0xAC;
const FADE: u32 = D + 0xA104;
const STREAKS: u32 = D + 0xA134;
const BANDS: u32 = D + 0xA16C;

/// The entry 0x1E05180 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x110);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x110);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0xA560).
    fx.bzero(D, 0xA560);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x37, 0), (0x14, 0x16, 0), (0x18, 0xC, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x13A);
    // The entry's s0 = D, s1 = the parameter, s2 = the OT.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_52D8, [Some(D), Some(param as u32), Some(OT)]);
    fx.mem.write_u32(TINT, 0);
    0
}

/// rand >> 4 from return address `ra` (the entry's s0 = D saved).
fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 4
}

/// ((rand >> 4) % m - `half`) / 100.
fn jit(fx: &mut Fx, ra: u32, m: i32, half: i32) -> f32 {
    let v = rnd(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

/// ((rand >> 4) % m x 200 - `off`) / 100.
fn spread(fx: &mut Fx, ra: u32, m: i32, off: i32) -> f32 {
    let v = rnd(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(200 * r - off), 100.0)
}

/// (C `%` of 2^k) x 200 - `off`, over 100.
fn spread_pow2(fx: &mut Fx, ra: u32, mask: i32, off: i32) -> f32 {
    let v = rnd(fx, ra);
    let mut m = v & mask;
    if v < 0 && m != 0 {
        m -= mask + 1;
    }
    ee::div(ee::from_i32(200 * m - off), 100.0)
}

/// The ribbon tail [F]: +0xB, the random +9, the bytes and the colours.
fn ribbon_tail(fx: &mut Fx, r: u32, ra: u32) {
    fx.mem.write_u8(r + 0xB, 2);
    let x = rnd(fx, ra);
    fx.mem.set8(r + 9, x);
    fx.mem.write_u32(r + 0x10, BYTES);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
}

/// Update 30 [F 0x1E053C0]: the held flash, a ribbon and the shake.
fn first(fx: &mut Fx) {
    set_counter4(fx, FLASH, 0x10, 0, 0xFF);
    fx.mem.write_u8(FLASH + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(FLASH + 6, b);
    for (o, v) in [(4u32, 0x80u8), (7, 9), (0xA, 0x80), (9, 0x40), (8, 0), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0)] {
        fx.mem.write_u8(FLASH + o, v);
    }
    fx.mem.write_u32(FLASH + 0x10, 0x41A0_0000);
    fx.mem.write_u32(FLASH + 0x14, 0x4170_0000);
    fx.mem.write_u32(FLASH + 0x18, 0x4120_0000);
    let r = RIBBONS;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(r + 6, b);
    for (o, v) in [(4u32, 5u8), (0xA, 0), (0xC, 5)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0x3FA6_6666);
    fx.mem.write_u32(r + 0x1C, 0x4170_0000);
    fx.mem.write_u8(r + 0x23, 0);
    fx.mem.write_u8(r + 0x27, 0x60);
    fx.mem.write_u8(r + 8, 0x28);
    fx.mem.write_u32(r + 0x58, 0);
    fx.mem.write_u32(r + 0x5C, 0);
    fx.mem.write_u32(r + 0x60, 0x300);
    ribbon_tail(fx, r, 0x01E0_5528);
    set_counter4(fx, D + 0x40, 0x93, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0x46, b);
    fx.mem.write_u8(D + 0x44, 0);
    fx.mem.write_u32(D + 0x54, 0x4000_0000);
    fx.mem.write_u32(D + 0x58, 0x3FCC_CCCD);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x4080_0000);
}

/// Update 40 [F 0x1E055BC]: the second held flash.
fn second(fx: &mut Fx) {
    let a = FLASH1;
    set_counter4(fx, a, 0x95, 0, 0xFF);
    fx.mem.write_u8(a + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(a + 6, b);
    fx.mem.write_u8(a + 4, 0x83);
    fx.mem.write_u8(a + 7, 9);
    fx.mem.write_u32(a + 0x10, 0x42F0_0000);
    fx.mem.write_u32(a + 0x14, 0);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x40), (8, 0x20)] {
        fx.mem.write_u8(a + o, v);
    }
}

/// Update 176 [F 0x1E056F8]: eighty sprites round the target (sp + 0x70).
/// Returns the entry's s1 after it.
fn ring(fx: &mut Fx, sp: u32) -> u64 {
    for _ in 0..0x50 {
        let i = first_free(fx, SPRITES, 0x44, 0x1F4);
        if i < 0 {
            continue;
        }
        let delay = rnd(fx, 0x01E0_572C) & 0xF;
        let r = SPRITES + 0x44 * i as u32;
        set_counter4(fx, r, 0x20, delay, 0xFF);
        let v = rnd(fx, 0x01E0_5760);
        fx.mem.set8(r + 4, (v & 3) << 4);
        for (o, v) in [(8u32, 1u8), (0xE, 0x80), (0xD, 0xA0), (0xC, 0xC0)] {
            fx.mem.write_u8(r + o, v);
        }
        let v = rnd(fx, 0x01E0_579C);
        fx.mem.set8(r + 0xF, (v & 0x3F) + 0x20);
        let f = ee::add(120.0, spread(fx, 0x01E0_57B8, 0x14, 0x7D0));
        fx.mem.setf(r + 0x10, f);
        let f = ee::add(12.0, spread_pow2(fx, 0x01E0_5824, 3, 0x190));
        fx.mem.setf(r + 0x14, f);
        fx.mem.write_u8(r + 0xB, 1);
        fx.mem.write_u8(r + 0xA, 5);
        let v = rnd(fx, 0x01E0_58A4);
        fx.mem.set8(r + 9, v & 3);
        fx.mem.write_u32(r + 0x18, 0);
        fx.mem.write_u32(r + 0x1C, 0);
        for (c, ra) in [(0u32, 0x01E0_58C4u32), (1, 0x01E0_5910), (2, 0x01E0_595C)] {
            let f = jit(fx, ra, 0x708, 0x384);
            fx.mem.setf(sp + 0x80 + 4 * c, f);
        }
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0x70 + 4 * c), fx.mem.f(sp + 0x80 + 4 * c));
            fx.mem.setf(r + 0x20 + 4 * c, v);
        }
        for c in 0..3u32 {
            let v = ee::div(fx.mem.f(sp + 0x80 + 4 * c), 45.0);
            fx.mem.setf(r + 0x2C + 4 * c, v);
        }
    }
    0x50
}

/// Update 177 [F 0x1E05A1C]: the frame fade, the streaks, eight bands, a
/// ribbon and 300 sprites at the target (sp + 0x70). Returns the entry's
/// s1 after it.
fn strike(fx: &mut Fx, sp: u32, mut s1: u64) -> u64 {
    fx.mem.set16(FADE, 0xB1);
    let e = fx.mem.u16(FADE) + 0x28;
    fx.mem.set16(FADE + 2, e);
    fx.mem.write_u8(FADE + 5, 6);
    fx.mem.write_u8(FADE + 6, 2);
    fx.mem.write_u32(FADE + 8, 0x4180_0000);
    for (o, v) in [(0x14u32, -8i32), (0xC, -8), (0x18, 8), (0x10, 8), (0x20, -8), (0x1C, -8), (0x28, 8), (0x24, 8)] {
        fx.mem.set32(FADE + o, v);
    }
    fx.mem.write_u8(FADE + 0x2E, 0x90);
    fx.mem.write_u8(FADE + 0x2D, 0x60);
    fx.mem.write_u8(FADE + 0x2C, 0x40);
    let s = STREAKS;
    set_counter4(fx, s, 0x28, 0, 0xFF);
    for (o, v) in [(4u32, 0x80u8), (9, 1), (0xA, 0xFF), (8, 0), (0xB, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(s + 0xC, h);
    for (o, v) in [(0x14u32, 0x80u8), (0x15, 0x80), (0x16, 0x80), (0x17, 0x20), (0x10, 0x80), (0x11, 0x80), (0x12, 0x80), (0x13, 0x20), (5, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, b);
    for s3 in 0..8i32 {
        let i = first_free(fx, BANDS, 0x5C, 8);
        if i < 0 {
            continue;
        }
        s1 = u64::from(0x5C * i as u32);
        let r = BANDS + 0x5C * i as u32;
        set_counter4(fx, r, 0x1C, s3 & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 1), (8, 2), (9, 2), (0xA, 6), (0xB, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x3F00_0000);
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0x3F99_999A);
        fx.mem.write_u32(r + 0x1C, 0x3F4C_CCCD);
        fx.mem.write_u32(r + 0x20, 0x3E80_0000);
        for (o, v) in [(0x28u32, 0x80u8), (0x29, 0x80), (0x2A, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0x10), (0x2B, 0x30)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x70, r + 0x2C);
        fx.mem.set32(r + 0x3C, 0x2800);
        fx.mem.set32(r + 0x40, if s3 < 4 { 0x2000 } else { -0x2000 });
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let v = fx.mem.i32(r + 0x40).wrapping_neg();
            fx.mem.set32(r + 0x40, v);
        }
    }
    let r = RIBBONS;
    set_counter4(fx, r, 0x1E, 0, 0xFF);
    for (o, v) in [(4u32, 5u8), (0xA, 0), (0xC, 5)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    fx.mem.write_u8(r + 0x23, 0x20);
    fx.mem.write_u8(r + 0x27, 0x60);
    fx.mem.write_u8(r + 8, 0xA0);
    fx.mem.write_u32(r + 0x58, 0x40);
    fx.mem.write_u32(r + 0x5C, 0x80);
    fx.mem.write_u32(r + 0x60, 0x100);
    fx.mem.write_u8(r + 5, 0);
    ribbon_tail(fx, r, 0x01E0_5EC4);
    copy_v4(fx, sp + 0x70, r + 0x28);
    for _ in 0..0x12C {
        let i = first_free(fx, SPRITES, 0x44, 0x1F4);
        if i < 0 {
            continue;
        }
        let count = ((rnd(fx, 0x01E0_5F4C) & 0xF) + 0x20) & 0xFF;
        s1 = count as u64;
        let delay = rnd(fx, 0x01E0_5F64) & 0xF;
        let r = SPRITES + 0x44 * i as u32;
        set_counter4(fx, r, count, delay, 0xFF);
        for (o, v) in [(4u32, 0x42u8), (8, 0), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        let v = (rnd(fx, 0x01E0_5FB4) & 3) + 2;
        fx.mem.set8(r + 9, v);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u32(r + 0x10, 0x4220_0000);
        fx.mem.write_u32(r + 0x14, 0xBE4C_CCCD);
        fx.mem.write_u32(r + 0x18, 0xBD75_C28F);
        let y = fx.mem.read_u32(NODE_Y);
        fx.mem.write_u32(r + 0x1C, y);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let f = jit(fx, 0x01E0_602C, 0x78, 0x3C);
        fx.mem.setf(r + 0x2C, f);
        let f = ee::add(f32::from_bits(0x3F4C_CCCD), jit(fx, 0x01E0_6078, 0x64, 0x32));
        fx.mem.setf(r + 0x30, f);
        let f = jit(fx, 0x01E0_60D4, 0x78, 0x3C);
        fx.mem.setf(r + 0x34, f);
    }
    s1
}

/// Updates 102..181 [F 0x1E0615C]: two sprites from each of nodes 0x16 /
/// 0xC. Returns the entry's s1 after.
fn sparks(fx: &mut Fx, sp: u32, mut s1: u64) -> u64 {
    for s3 in 0..2u32 {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14 + 4 * s3));
        node_position_saving(fx, h, s, sp + 0x80, u64::from(D), Some(s1), 0x01E0_6184);
        for _ in 0..2 {
            let i = first_free(fx, SPRITES, 0x44, 0x1F4);
            if i < 0 {
                continue;
            }
            let r = SPRITES + 0x44 * i as u32;
            set_counter4(fx, r, 0x20, 0, 0xFF);
            let v = rnd(fx, 0x01E0_61E0);
            fx.mem.set8(r + 4, (v & 3) << 4);
            for (o, v) in [(8u32, 1u8), (0xE, 0x80), (0xD, 0xA0), (0xC, 0xC0), (0xF, 0x60)] {
                fx.mem.write_u8(r + o, v);
            }
            let f = ee::add(120.0, spread(fx, 0x01E0_6224, 0x14, 0x7D0));
            fx.mem.setf(r + 0x10, f);
            let f = ee::add(5.0, spread_pow2(fx, 0x01E0_6290, 1, 0xC8));
            fx.mem.setf(r + 0x14, f);
            fx.mem.write_u8(r + 0xB, 1);
            fx.mem.write_u8(r + 0xA, 5);
            let v = rnd(fx, 0x01E0_6310);
            fx.mem.set8(r + 9, v & 3);
            fx.mem.write_u32(r + 0x18, 0);
            fx.mem.write_u32(r + 0x1C, 0);
            for (c, ra) in [(0u32, 0x01E0_6330u32), (1, 0x01E0_6380), (2, 0x01E0_63D0)] {
                let f = ee::add(fx.mem.f(sp + 0x80 + 4 * c), jit(fx, ra, 0x190, 0xC8));
                fx.mem.setf(r + 0x20 + 4 * c, f);
            }
            let f = ee::add(f32::from_bits(0x3E4C_CCCD), jit(fx, 0x01E0_6420, 0x1E, 0xF));
            fx.mem.setf(r + 0x30, f);
            let f = jit(fx, 0x01E0_647C, 0x3C, 0x1E);
            fx.mem.setf(r + 0x2C, f);
            let f = ee::add(0.5, jit(fx, 0x01E0_64C8, 0x5A, 0x2D));
            fx.mem.setf(r + 0x34, f);
            if fx.mem.read_u32(D + 0x30) != 0x8000 {
                let v = ee::mul(fx.mem.f(r + 0x34), -1.0);
                fx.mem.setf(r + 0x34, v);
            }
        }
        s1 = 2;
    }
    s1
}

/// Updates 40..176 [F 0x1E06584]: twelve sprites thrown from node 0x37
/// at random turns. Returns the entry's s1 after.
fn throw(fx: &mut Fx, sp: u32, mut s1: u64) -> u64 {
    for s3 in 0..0xCu32 {
        let i = first_free(fx, SPRITES, 0x44, 0x1F4);
        if i < 0 {
            continue;
        }
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
        node_position_saving(fx, h, s, sp + 0x80, u64::from(D), Some(s1), 0x01E0_65C4);
        let f20 = ee::add(16.0, spread(fx, 0x01E0_65CC, 6, 0x258));
        fx.fix_residue(f20, 0x01E0_6634);
        let count = ee::fixsfsi(f20) & 0xFF;
        s1 = u64::from(0x44 * i as u32);
        let r = SPRITES + 0x44 * i as u32;
        set_counter4(fx, r, count, 0, 0xFF);
        fx.mem.write_u8(r + 4, 0x40);
        fx.mem.write_u8(r + 8, 0);
        fx.mem.write_u8(r + 0xB, 2);
        let v = (rnd(fx, 0x01E0_667C) + 2) & 3;
        fx.mem.set8(r + 9, v);
        fx.mem.write_u8(r + 0xA, 0);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u32(r + 0x10, 0x41A0_0000);
        for o in [0x14u32, 0x1C, 0x18] {
            fx.mem.write_u32(r + o, 0);
        }
        for (c, ra) in [(0u32, 0x01E0_66C4u32), (1, 0x01E0_66D8), (2, 0x01E0_66EC)] {
            let v = rnd(fx, ra) & 0xFFFF;
            fx.mem.set32(sp + 0xF0 + 4 * c, v);
        }
        fx.trs_into_saving(sp + 0xA0, 0, sp + 0xF0, 0, [f20, 0.0, 0.0, 0.0, 0.0], [u64::from(D), s1, u64::from(OT), u64::from(s3)], 0x01E0_6710);
        fx.mem.write_u32(sp + 0xE8, 0);
        fx.mem.write_u32(sp + 0xE0, 0);
        let a = rnd(fx, 0x01E0_6720) & 0xFF;
        let b = rnd(fx, 0x01E0_6730) & 0xFF;
        fx.mem.setf(sp + 0xE4, ee::div(ee::from_i32(a - b), 50.0));
        let m = fx.read_m4(sp + 0xA0);
        let x = fx.read_v4(sp + 0xE0);
        let v = fx.transform(Some(&m), x);
        fx.write_v4(sp + 0x90, v);
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0x80 + 4 * c), fx.mem.f(sp + 0x90 + 4 * c));
            fx.mem.setf(r + 0x20 + 4 * c, v);
        }
        for c in 0..3u32 {
            let v = ee::neg(ee::div(fx.mem.f(sp + 0x90 + 4 * c), f20));
            fx.mem.setf(r + 0x2C + 4 * c, v);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(r + o, 0);
        }
    }
    s1
}

/// Even updates 65..176 [F 0x1E06830]: a ribbon at the attacker. Returns
/// the entry's s1 after.
fn trail(fx: &mut Fx, s1: u64) -> u64 {
    let i = first_free(fx, RIBBONS, 0x68, 0x3C);
    if i < 0 {
        return s1;
    }
    let r = RIBBONS + 0x68 * i as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    if fx.mem.u16(D) < 0xA1 {
        fx.mem.write_u32(r + 0x14, 0x3F4C_CCCD);
        fx.mem.write_u32(r + 0x1C, 0x4120_0000);
        fx.mem.write_u8(r + 8, 8);
    } else {
        fx.mem.write_u32(r + 0x14, 0x4040_0000);
        fx.mem.write_u32(r + 0x1C, 0x41C8_0000);
        fx.mem.write_u8(r + 8, 0x14);
    }
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(r + 6, b);
    fx.mem.write_u8(r + 0xA, 0);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 3);
    let x = rnd(fx, 0x01E0_6908);
    fx.mem.set8(r + 9, x);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    fx.mem.write_u8(r + 0x23, 0x10);
    fx.mem.write_u8(r + 0x27, 0x40);
    for (o, ra) in [(0x58u32, 0x01E0_695Cu32), (0x5C, 0x01E0_6974), (0x60, 0x01E0_698C)] {
        let v = rnd(fx, ra) & 0x7F;
        fx.mem.set32(r + o, v);
    }
    u64::from(0x68 * i as u32)
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut s1 = i64::from(param) as u64;
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x70, u64::from(D), 0x01E0_530C);
    match fx.mem.u16(D) {
        0x140 => return 8,
        0xB1 => s1 = strike(fx, sp, s1),
        0xB0 => s1 = ring(fx, sp),
        0x8F => {
            let v = fx.mem.read_u8(FLASH1 + 4) | 0x80;
            fx.mem.write_u8(FLASH1 + 4, v);
            fx.mem.write_u32(TINT, 0);
        }
        0x66 => {
            fx.mem.write_u8(FLASH1 + 4, 3);
            fx.mem.write_u32(TINT, 1);
        }
        0x28 => second(fx),
        0x1E => first(fx),
        2 => {
            let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14));
            node_position_saving(fx, h, s, D + 0xA44C, u64::from(D), Some(s1), 0x01E0_5390);
            let y = ee::sub(fx.mem.f(NODE_Y), 5.0);
            fx.mem.setf(NODE_Y, y);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x66..0xB6).contains(&n) {
        s1 = sparks(fx, sp, s1);
    }
    let n = fx.mem.u16(D);
    if (0x28..0xB1).contains(&n) {
        s1 = throw(fx, sp, s1);
    }
    let n = fx.mem.u16(D);
    if (0x41..0xB1).contains(&n) && n & 1 == 0 {
        s1 = trail(fx, s1);
    }
    let n = fx.mem.u16(D);
    if (0x50..0xAA).contains(&n) && n & 7 == 0 {
        for o in [0x108u32, 0x109, 0x10A, 0x10C, 0x10D, 0x10E] {
            fx.mem.write_u8(sp + o, 0x80);
        }
        fx.mem.write_u8(sp + 0x10B, 0x70);
        fx.mem.write_u8(sp + 0x10F, 0x10);
        let h = fx.mem.read_u32(D + 4);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h, 0xF, 0, sp + 0x108]);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_6A24, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6A34, [Some(u64::from(D)), Some(s1)]);
    blib::fade::fade(fx, D + 0xA4, OT);
    // fade's 0xF0 frame saves the runner's s6 (0) at +0x70 and ra at
    // +0x80: the streaks' first TEXFLUSH a0 / a1 (vertex 1's / 2's stale
    // XYZ low words) [run].
    fx.saved(sp - 0xF0, &[(0x70, 0), (0x80, 0x01E0_6A44)]);
    let (_, smear_a3) = blib::smear::smears_c_a3(fx, 0x01E0_2B00, SMEARS, OT, 2);
    let (_, flash_a3) = blib::star::flash_a3(fx, 0x01E0_0950, FLASH, OT, 2);
    let (_, a3) = blib::radiate::streaks_a3(fx, 0x01E0_3E80, STREAKS, OT, 1);
    let n = fx.mem.u16(D);
    blib::window::frame_fade(fx, 0x01E0_4CA0, FADE, OT, n, a3.or(flash_a3).or(smear_a3).unwrap_or(0));
    blib::streak::ribbon(fx, 0x01E0_1500, RIBBONS, OT, 0x3C);
    blib::sprite::bounce(fx, SPRITES, OT, 0x1F4);
    blib::band::bands(fx, 0x01E0_3690, BANDS, OT, 8);
    let n = fx.mem.u16(D);
    if (0xB1..0xCE).contains(&n) {
        6
    } else if n == 0xCE {
        2
    } else if n == 0xA7 {
        1
    } else {
        0
    }
}
