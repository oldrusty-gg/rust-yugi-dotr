//! BMODULE b439 (card 466), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03C60, data area a0 = 0x1ED0000, 0xA600 bytes)
//! [F]: s0 =
//! D, s1 = the OT (0x206E40), s2 the parameter then offsets. The entry
//! first copies the record's 17-word pulse table (0x1E05340) to sp + 0x60.
//! Update 19 the attacker's streaks (+0x1470, `radiate::streaks_b`, node
//! 1; restarted on 255), their far alpha pulsed by the table; 138 at node
//! 0xB 120 + 200 bouncing sprites (+0x2028, 0x44 apart); on updates
//! 65..110 / 226..252 four sprites a update from node 0x1A, on 139..200 /
//! 300..350 ten from node 0x10; 257 at the target the fade (+0xA4F8), six
//! spikes (+0xA4, 0x68 apart), 200 sprites, a flash (+0x1424), bands
//! (+0x14A8) at nodes 8 / 0xE and a shake (+0x40); on updates 257..280 a
//! spike a update. The stage light pulsed by 0x1031C0 (+0xA0). Returns 6
//! on updates 257..285, 2 on 254, 1 on 247, 8 on 399 (no drawing then)
//! and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080 (0538257fe1fa),
//! `spikes` 0x1E003A0, `flash_b` 0x1E00E80, `streaks_b` 0x1E018F0,
//! `fade` 0x1E026F0, `drops` 0x1E02BC0, `bands_b` 0x1E03520
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PULSE: u32 = 0x01E0_5340;
const STREAKS: u32 = D + 0x1470;
const SPRITES: u32 = D + 0x2028;
const SPIKES: u32 = D + 0xA4;
const BYTES: u32 = D + 0xA500;

/// The entry 0x1E03C60 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    for k in (0..0x40u32).step_by(8) {
        let v = fx.mem.read_u64(PULSE + k);
        fx.mem.write_u64(sp + 0x60 + k, v);
    }
    let v = fx.mem.read_u32(PULSE + 0x40);
    fx.mem.write_u32(sp + 0xA0, v);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0xA600).
    fx.bzero(D, 0xA600);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..5 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 8, 1), (0x10, 0xE, 0), (0x14, 0xB, 0), (0x18, 1, 0), (0x1C, 0x10, 0), (0x20, 0x1A, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x189);
    // The entry's s0 = D, s1 = 5 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_3DD0, [Some(D), Some(5), Some(param as u32)]);
    0
}

fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 4
}

/// ((rand >> 4) % m x `k` - `off`) / 100.
fn spread(fx: &mut Fx, ra: u32, m: i32, k: i32, off: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(k * r - off), 100.0)
}

/// rand >> 4 & 1 with the signed-remainder fix-up.
fn odd(fx: &mut Fx, ra: u32) -> i32 {
    let v = rand4(fx, ra);
    let mut b = v & 1;
    if v < 0 && b != 0 {
        b -= 2;
    }
    b
}

fn grey(fx: &mut Fx, r: u32) {
    for o in [0xFu32, 0xC, 0xD, 0xE] {
        fx.mem.write_u8(r + o, 0x80);
    }
}

fn warm(fx: &mut Fx, r: u32) {
    for (o, v) in [(0xEu32, 0x80u8), (0xD, 0xA0), (0xC, 0xC0), (0xF, 0x40)] {
        fx.mem.write_u8(r + o, v);
    }
}

/// A free sprite counted `count` after `delay` with flags rand & 3 << 4
/// (`ra`), or `None` with the pool full.
fn sprite(fx: &mut Fx, count: i32, delay: Option<(u32, i32)>, ra: u32) -> Option<u32> {
    let i = first_free(fx, SPRITES, 0x44, 0x1F4);
    if i < 0 {
        return None;
    }
    let d = match delay {
        Some((dra, mask)) => rand4(fx, dra) & mask,
        None => 0,
    };
    let r = SPRITES + 0x44 * i as u32;
    set_counter4(fx, r, count, d, 0xFF);
    let v = rand4(fx, ra);
    fx.mem.set8(r + 4, (v & 3) << 4);
    Some(r)
}

/// r + 0x20.. = `from` + jitter (rand % m - half) / 100 per axis (`ras`).
fn place(fx: &mut Fx, r: u32, from: u32, ras: [u32; 3], m: i32, half: i32) {
    for (c, ra) in ras.into_iter().enumerate() {
        let f = ee::add(fx.mem.f(from + 4 * c as u32), spread(fx, ra, m, 1, half));
        fx.mem.setf(r + 0x20 + 4 * c as u32, f);
    }
}

/// r + 0xB = 2, + 0xA = 0, + 9 = rand & 7, + 0x1C = + 0x18 = 0.
fn tail(fx: &mut Fx, r: u32, ra: u32) {
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xA, 0);
    let v = rand4(fx, ra);
    fx.mem.set8(r + 9, v & 7);
    fx.mem.write_u32(r + 0x1C, 0);
    fx.mem.write_u32(r + 0x18, 0);
}

/// The two-kind sprite colours / speeds [F 0x1E04140 / 0x1E04A20 /
/// 0x1E04D94]: `fast` -> kind 2 grey at 180 + jitter, else kind 1 warm at
/// `slow` + jitter, its vertical speed `rise` + (odd x 2 - 2) / 100.
fn kinds(fx: &mut Fx, r: u32, fast: bool, ras: [u32; 3], slow: (f32, i32, i32), rise: Option<f32>) {
    if fast {
        fx.mem.write_u8(r + 8, 2);
        grey(fx, r);
        let f = ee::add(180.0, spread(fx, ras[0], 0x28, 200, 0xFA0));
        fx.mem.setf(r + 0x10, f);
        fx.mem.write_u32(r + 0x14, 0);
    } else {
        fx.mem.write_u8(r + 8, 1);
        warm(fx, r);
        let (base, m, off) = slow;
        let f = ee::add(base, spread(fx, ras[1], m, 200, off));
        fx.mem.setf(r + 0x10, f);
        if let Some(b) = rise {
            let v = odd(fx, ras[2]);
            let f = ee::add(b, ee::div(ee::from_i32(200 * v - 200), 100.0));
            fx.mem.setf(r + 0x14, f);
        }
    }
}

/// Update 138 [F 0x1E03ED0]: from node 0xB (sp + 0xC0) 120 sprites, then
/// 200.
fn burst(fx: &mut Fx, sp: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14));
    node_position_saving(fx, h, s, sp + 0xC0, u64::from(D), Some(u64::from(OT)), 0x01E0_3EE0);
    for _ in 0..0x78 {
        let Some(r) = sprite(fx, 0x24, Some((0x01E0_3F08, 7)), 0x01E0_3F38) else { continue };
        fx.mem.write_u8(r + 8, 2);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xA, 0);
        let v = rand4(fx, 0x01E0_3F5C);
        fx.mem.set8(r + 9, (v + 4) & 7);
        grey(fx, r);
        let f = ee::add(150.0, spread(fx, 0x01E0_3F84, 0x1E, 200, 0xBB8));
        fx.mem.setf(r + 0x10, f);
        fx.mem.write_u32(r + 0x14, 0x40A0_0000);
        fx.mem.write_u32(r + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xC0 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let f = spread(fx, 0x01E0_4010, 0x96, 1, 0x4B);
        fx.mem.setf(r + 0x2C, f);
        let f = ee::add(1.0, spread(fx, 0x01E0_404C, 0x96, 1, 0x4B));
        fx.mem.setf(r + 0x30, f);
        let f = spread(fx, 0x01E0_4090, 0x96, 1, 0x4B);
        fx.mem.setf(r + 0x34, f);
    }
    let mut s3 = 0i32;
    while s3 < 0xC8 {
        let Some(r) = sprite(fx, 0x18, Some((0x01E0_40F8, 7)), 0x01E0_4128) else {
            s3 += 1;
            continue;
        };
        let k = s3 & 7;
        kinds(fx, r, k == 0 || k == 1, [0x01E0_4178, 0x01E0_41F8, 0x01E0_4250], (40.0, 0x28, 0xFA0), Some(3.0));
        tail(fx, r, 0x01E0_42B8);
        let f = ee::add(fx.mem.f(sp + 0xC0), spread(fx, 0x01E0_42D0, 0xBB8, 1, 0x5DC));
        fx.mem.setf(r + 0x20, f);
        let f = ee::add(fx.mem.f(sp + 0xC4), spread(fx, 0x01E0_4310, 0xBB8, 1, 0x5DC));
        fx.mem.setf(r + 0x24, f);
        let z = fx.mem.read_u32(sp + 0xC8);
        fx.mem.write_u32(r + 0x28, z);
        let f = ee::add(f32::from_bits(0x3F66_6666), spread(fx, 0x01E0_4354, 0x64, 1, 0x32));
        fx.mem.setf(r + 0x30, f);
        let f = spread(fx, 0x01E0_439C, 0x3C, 1, 0x1E);
        fx.mem.setf(r + 0x2C, f);
        s3 += 1;
    }
}

/// Update 257 [F 0x1E043EC]: the strike at the target (sp + 0xB0).
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0xA4F8, 0xE, 0, 0xFF);
    fx.mem.write_u8(D + 0xA4FC, 1);
    // Six spikes (+0xA4, 0x68 apart), delayed 3 apart.
    for s4 in 0..6u32 {
        let r = SPIKES + 0x68 * s4;
        fx.mem.write_u8(r + 4, 4);
        set_counter4(fx, r, 0x1C, (3 * s4) as i32 & 0xFF, 0xFF);
        let b = fx.mem.read_u8(r + 4);
        fx.mem.write_u8(r + 4, b | 2);
        for (o, v) in [(0x27u32, 0x40u8), (0x23, 0x80), (5, 0), (0xA, 4), (0xB, 2), (0xC, 4), (8, 0x32)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = rand4(fx, 0x01E0_4478);
        fx.mem.set8(r + 9, x);
        let f3 = ee::from_i32(s4 as i32);
        fx.mem.write_u32(r + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0xB0, r + 0x28);
        fx.mem.setf(r + 0x1C, ee::add(30.0, ee::mul(20.0, f3)));
        fx.mem.setf(r + 0x14, ee::sub(3.0, ee::mul(0.5, f3)));
        let k = f32::from_bits(0x3ECC_CCCD);
        fx.mem.setf(r + 0x18, ee::add(k, ee::mul(k, f3)));
        let t = if fx.mem.read_u32(D + 0x30) == 0x8000 { 0x4000 } else { -0x4000 };
        fx.mem.set32(r + 0x48, t);
    }
    // 200 sprites at the target.
    for _ in 0..0xC8 {
        let Some(r) = sprite(fx, 0x24, Some((0x01E0_458C, 0xF)), 0x01E0_45BC) else { continue };
        fx.mem.write_u8(r + 8, 4);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xA, 0);
        let v = rand4(fx, 0x01E0_45E4);
        fx.mem.set8(r + 9, (v + 4) & 7);
        grey(fx, r);
        let f = ee::add(100.0, spread(fx, 0x01E0_460C, 0x14, 200, 0x7D0));
        fx.mem.setf(r + 0x10, f);
        fx.mem.write_u32(r + 0x14, 0x4040_0000);
        fx.mem.write_u32(r + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xB0 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let f = spread(fx, 0x01E0_468C, 0xDC, 1, 0x6E);
        fx.mem.setf(r + 0x2C, f);
        let f = ee::add(1.0, spread(fx, 0x01E0_46C8, 0x96, 1, 0x4B));
        fx.mem.setf(r + 0x30, f);
        let f = spread(fx, 0x01E0_470C, 0xDC, 1, 0x6E);
        fx.mem.setf(r + 0x34, f);
    }
    // The flash (+0x1424).
    let a = D + 0x1424;
    set_counter4(fx, a, 0x24, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (7, 9), (4, 0x81)] {
        fx.mem.write_u8(a + o, v);
    }
    fx.mem.write_u32(a + 0x10, 0x4448_0000);
    fx.mem.write_u32(a + 0x18, 0);
    fx.mem.write_u32(a + 0x14, 0);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x10), (8, 0x80)] {
        fx.mem.write_u8(a + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(a + o, 0);
    }
    copy_v4(fx, sp + 0xB0, a + 0x1C);
    // Bands (+0x14A8, 0x5C apart) at nodes 8 / 0xE (sp + 0xC0).
    for s4 in 0..2u32 {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC + 4 * s4));
        node_position_saving(fx, h, s, sp + 0xC0, u64::from(D), Some(u64::from(OT)), 0x01E0_47FC);
        let i = first_free(fx, D + 0x14A8, 0x5C, 0x20);
        if i < 0 {
            continue;
        }
        let b = D + 0x14A8 + 0x5C * i as u32;
        set_counter4(fx, b, 0x2A, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (7, 0x24), (0xC, 2), (8, 4), (9, 2), (0xA, 0), (0xB, 4)] {
            fx.mem.write_u8(b + o, v);
        }
        fx.mem.write_u32(b + 0x10, 0x4000_0000);
        fx.mem.write_u32(b + 0x14, 0x3F00_0000);
        for o in [0x2Bu32, 0x27, 0x28, 0x29, 0x2A, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(b + o, 0x80);
        }
        fx.mem.set32(b + 0x3C, 0x4000);
        fx.mem.write_u32(b + 0x18, 0x3F99_999A);
        let v = ee::sub(fx.mem.f(b + 0x18), f32::from_bits(0x3E4C_CCCD));
        fx.mem.setf(b + 0x1C, v);
        copy_v4(fx, sp + 0xC0, b + 0x2C);
    }
    // The camera shake (+0x40) at the target.
    set_counter4(fx, D + 0x40, 0x20, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x4000_0000);
    fx.mem.write_u32(D + 0x58, 0x4000_0000);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    copy_v4(fx, sp + 0xB0, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xB0, u64::from(D), 0x01E0_3DF0);
    match fx.mem.u16(D) {
        0x18F => return 8,
        0x101 => strike(fx, sp),
        0x8A => burst(fx, sp),
        0xFF => set_counter4(fx, STREAKS, 0xFF, 0, 0xFF),
        0x13 => {
            let s = STREAKS;
            set_counter4(fx, s, 0xFF, 0, 0xFF);
            for (o, v) in [(4u32, 0x80u8), (9, 2), (0xA, 0xFF), (8, 0x10), (0xB, 8)] {
                fx.mem.write_u8(s + o, v);
            }
            let h = fx.mem.read_u32(D + 4);
            fx.mem.write_u32(s + 0xC, h);
            for (o, v) in [(0x16u32, 0x20u8), (0x15, 0x20), (0x14, 0x80), (0x17, 0x10), (0x12, 0x40), (0x11, 0x40), (0x10, 0xF0), (0x13, 0), (5, 1)] {
                fx.mem.write_u8(s + o, v);
            }
            let b = fx.mem.read_u8(D + 0x18);
            fx.mem.write_u8(s + 6, b);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x8B..0xC9).contains(&n) || (0x12C..0x15F).contains(&n) {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x1C));
        node_position_saving(fx, h, s, sp + 0xC0, u64::from(D), Some(u64::from(OT)), 0x01E0_49C0);
        let mut s3 = 0i32;
        while s3 < 0xA {
            let Some(r) = sprite(fx, 0x1C, None, 0x01E0_4A08) else {
                s3 += 1;
                continue;
            };
            let k = s3 & 7;
            kinds(fx, r, k == 0 || k == 1, [0x01E0_4A58, 0x01E0_4AD8, 0x01E0_4B30], (30.0, 0x14, 0x7D0), Some(4.0));
            tail(fx, r, 0x01E0_4B98);
            place(fx, r, sp + 0xC0, [0x01E0_4BB0, 0x01E0_4BF0, 0x01E0_4C30], 0x2BC, 0x15E);
            let f = ee::add(f32::from_bits(0x3E99_999A), spread(fx, 0x01E0_4C70, 0x32, 1, 0x19));
            fx.mem.setf(r + 0x30, f);
            let f = spread(fx, 0x01E0_4CB8, 0x3C, 1, 0x1E);
            fx.mem.setf(r + 0x2C, f);
            s3 += 1;
        }
    }
    let n = fx.mem.u16(D);
    if (0x41..0x6F).contains(&n) || (0xE2..0xFD).contains(&n) {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x20));
        node_position_saving(fx, h, s, sp + 0xC0, u64::from(D), Some(u64::from(OT)), 0x01E0_4D3C);
        let mut s3 = 0i32;
        while s3 < 4 {
            let Some(r) = sprite(fx, 0x1C, None, 0x01E0_4D84) else {
                s3 += 1;
                continue;
            };
            kinds(fx, r, s3 & 1 != 0, [0x01E0_4DC0, 0x01E0_4E44, 0x01E0_4E9C], (30.0, 0x14, 0x7D0), Some(4.0));
            tail(fx, r, 0x01E0_4F08);
            place(fx, r, sp + 0xC0, [0x01E0_4F20, 0x01E0_4F60, 0x01E0_4FA0], 0x1F4, 0xFA);
            let f = ee::add(0.5, spread(fx, 0x01E0_4FE0, 0x1E, 1, 0xF));
            fx.mem.setf(r + 0x30, f);
            let f = spread(fx, 0x01E0_5024, 0x14, 1, 0xA);
            fx.mem.setf(r + 0x2C, f);
            s3 += 1;
        }
    }
    let n = fx.mem.u16(D);
    if (0x101..0x119).contains(&n) {
        let i = first_free(fx, SPIKES, 0x68, 0x30);
        if i >= 0 {
            let r = SPIKES + 0x68 * i as u32;
            set_counter4(fx, r, 0x10, 0, 0xFF);
            fx.mem.write_u8(r + 4, 5);
            fx.mem.write_u32(r + 0x14, 0x4090_0000);
            fx.mem.write_u32(r + 0x1C, 0x42A0_0000);
            fx.mem.write_u8(r + 8, 0x1E);
            fx.mem.write_u8(r + 5, 0);
            copy_v4(fx, sp + 0xB0, r + 0x28);
            for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 3)] {
                fx.mem.write_u8(r + o, v);
            }
            let x = rand4(fx, 0x01E0_512C);
            fx.mem.set8(r + 9, x);
            fx.mem.write_u32(r + 0x10, BYTES);
            fx.mem.write_u32(r + 0x18, 0x3F80_0000);
            for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x60)] {
                fx.mem.write_u8(r + o, v);
            }
            for (o, ra) in [(0x58u32, 0x01E0_517Cu32), (0x5C, 0x01E0_5190), (0x60, 0x01E0_51A4)] {
                let v = rand4(fx, ra);
                fx.mem.set32(r + o, v & 0x7F);
            }
        }
    }
    if fx.mem.u8(STREAKS) > 0 {
        let k = ((fx.mem.u16(D) - 0x10) & 0xF) as u32;
        let v = fx.mem.read_u8(sp + 0x60 + 4 * k);
        fx.mem.write_u8(STREAKS + 0x13, v.wrapping_mul(5));
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_51F0, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_51FC, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0xA4F8, OT);
    // fade's 0xF0 frame saves the runner's s6 (0) at +0x70 and ra at
    // +0x80 (the streaks' first TEXFLUSH reads the first as stale XYZ).
    fx.saved(sp - 0xF0, &[(0x70, 0), (0x80, 0x01E0_520C)]);
    blib::radiate::streaks_b(fx, 0x01E0_18F0, STREAKS, OT, 1);
    blib::spike::spikes(fx, 0x01E0_03A0, SPIKES, OT, 0x30);
    blib::star::flash_b(fx, 0x01E0_0E80, D + 0x1424, OT, 1);
    blib::band::bands_b(fx, 0x01E0_3520, D + 0x14A8, OT, 0x20);
    blib::drop::drops(fx, SPRITES, OT, 0x1F4);
    let n = fx.mem.u16(D);
    if (0x101..0x11E).contains(&n) {
        6
    } else if n == 0xFE {
        2
    } else if n == 0xF7 {
        1
    } else {
        0
    }
}
