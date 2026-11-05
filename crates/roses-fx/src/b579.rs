//! BMODULE b579 (card 523), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04D00, data area a0 = 0x1ED0000) [F] (helper F10,
//! 3 Nov 2026): b387's family (card 487, `crate::b387`; s0 = D, s1 = the
//! OT) with two more strike stages and ring bands. On update 20 four bars
//! (+0xB3C4 + 0x50 k at node +0xC's point: the first 1000 long, the others
//! 400 at 45 k degrees), a flash (+0xAC) and a camera shake there (+0x40);
//! on 49 sixteen bands (+0xA284, 0x5C records, delays 0..15) stepped from
//! node +0x10's point to the target; on 65 the fade (+0xA4), one more band,
//! four ribbons (+0xA2C), two flashes at the target (600 / 200 wide) and
//! the long shake there; on 65..192 every update four comets (+0x228C,
//! `comet::comets_c`, velocities +0x37CC, z away from the attacker), ten
//! bouncing sprites (+0x4A8C) and a ribbon (random bytes +0xB504); the
//! self unit's flag (0x1F69C0) on 65..212; the stage light (+0xA0).
//! Returns 1 on update 55, 6 on 65..209, 2 on 210, 8 on 250 (and at once
//! for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `comets_c`
//! 0x1E00410, `flash` 0x1E01410, `ribbon` 0x1E01FC0, `bounce` 0x1E02B50,
//! `fade` 0x1E035C0, `bands` 0x1E03B00, `bars` 0x1E042F0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BANDS: u32 = D + 0xA284;
const COMETS: u32 = D + 0x228C;
const VEL: u32 = D + 0x37CC;
const BOUNCE: u32 = D + 0x4A8C;
const RIBBON: u32 = D + 0xA2C;
const BARS: u32 = D + 0xB3C4;
const BYTES: u32 = D + 0xB504;

/// The entry 0x1E04D00 (a0 = D, a1 = param; s0 = D, s1 = the OT, s2 =
/// param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xB610);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..2 {
        fx.texture(5, k);
    }
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x26, 1);
    fx.mem.set32(D + 0xC, r);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xF4);
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4E18, [Some(D), Some(OT), Some(2)]);
    0
}

/// Update 20 [F 0x1E04E7C]: four bars, a flash and the shake at node
/// +0xC's point (sp + 0x90).
fn bars_start(fx: &mut Fx, sp: u32) {
    let at = sp + 0x90;
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, slot, at, u64::from(D), Some(u64::from(OT)), 0x01E0_4E90);
    for k in 0..4i32 {
        let r = BARS + 0x50 * k as u32;
        set_counter4(fx, r, 0x16, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x80);
        if k == 0 {
            fx.mem.write_u32(r + 0x10, 0x447A_0000);
            fx.mem.write_u32(r + 0x18, 0);
            fx.mem.write_u32(r + 0x14, 0x41A0_0000);
        } else {
            fx.mem.write_u32(r + 0x10, 0x43C8_0000);
            fx.mem.setf(r + 0x18, ee::mul(45.0, ee::from_i32(k)));
            fx.mem.write_u32(r + 0x14, 0x4170_0000);
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0xC0), (9, 0x80), (8, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x40C0_0000);
        copy_v4(fx, at, r + 0x20);
    }
    let r = D + 0xAC;
    set_counter4(fx, r, 0x16, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x4334_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x40), (9, 0x10), (8, 0x80), (0xF, 0xD0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, at, r + 0x1C);
    let s = D + 0x40;
    set_counter4(fx, s, 0x16, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, s + 0x20);
}

/// The band fields both band stages share (counter, delay, kind bytes,
/// radii 0.3 / 0.1, white, spin 0x4000).
fn band_common(fx: &mut Fx, r: u32, count: i32, delay: i32, len: u8) {
    set_counter4(fx, r, count, delay, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 7, len);
    fx.mem.write_u8(r + 0xC, 2);
    for (o, v) in [(8u32, 0u8), (9, 2), (0xA, 0), (0xB, 4)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, 0x3E99_999A);
    fx.mem.write_u32(r + 0x14, 0x3DCC_CCCD);
    for o in [0x24u32, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B] {
        fx.mem.write_u8(r + o, 0x80);
    }
    fx.mem.write_u32(r + 0x3C, 0x4000);
}

/// Update 49 [F 0x1E05160]: sixteen bands from node +0x10's point (sp +
/// 0x90) to the target (sp + 0x80) in sixteenths.
fn bands_start(fx: &mut Fx, sp: u32) {
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, slot, sp + 0x90, u64::from(D), Some(u64::from(OT)), 0x01E0_5174);
    copy_v4(fx, sp + 0x90, sp + 0xA0);
    let step: [f32; 3] = core::array::from_fn(|c| {
        let c = 4 * c as u32;
        ee::div(ee::sub(fx.mem.f(sp + 0x80 + c), fx.mem.f(sp + 0x90 + c)), 16.0)
    });
    for k in 0..0x10i32 {
        let r = BANDS + 0x5C * k as u32;
        band_common(fx, r, 0x1C, k & 0xFF, 0x18);
        fx.mem.write_u8(r + 4, 0x80);
        fx.mem.write_u32(r + 0x44, 0);
        fx.mem.write_u32(r + 0x40, 0);
        fx.mem.write_u32(r + 0x18, 0x3E99_999A);
        fx.mem.setf(r + 0x1C, ee::sub(f32::from_bits(0x3E99_999A), f32::from_bits(0x3DCC_CCCD)));
        let kf = ee::from_i32(k);
        for c in 0..3u32 {
            let p = ee::add(fx.mem.f(sp + 0x90 + 4 * c), ee::mul(step[c as usize], kf));
            fx.mem.setf(r + 0x2C + 4 * c, p);
        }
    }
}

/// Update 65 [F 0x1E053E4]: the fade, a band and four ribbons at the
/// target (sp + 0x80), two flashes and the long shake there.
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x80;
    set_counter4(fx, D + 0xA4, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0xA8, 1);
    let i = first_free(fx, BANDS, 0x5C, 0x30);
    if i >= 0 {
        let r = BANDS + 0x5C * i as u32;
        band_common(fx, r, 0x1E, 0, 0x24);
        fx.mem.write_u32(r + 0x18, 0x3FCC_CCCD);
        fx.mem.write_u32(r + 0x1C, 0x3FB3_3333);
        copy_v4(fx, at, r + 0x2C);
    }
    // The loop counter is also the slot index (s3) [F 0x1E055BC..0x1E056DC].
    let mut s = 0i32;
    while s < 4 {
        s = first_free(fx, RIBBON, 0x68, 0x3C);
        if s >= 0 {
            let r = RIBBON + 0x68 * s as u32;
            fx.mem.write_u8(r + 4, 5);
            set_counter4(fx, r, 0x20, s & 0xFF, 0xFF);
            let b = fx.mem.read_u8(r + 4) | 2;
            fx.mem.write_u8(r + 4, b);
            fx.mem.write_u32(r + 0x14, 0x4000_0000);
            fx.mem.write_u32(r + 0x1C, 0x4270_0000);
            for (o, v) in [(0x23u32, 0x80u8), (0x27, 0x80), (5, 0), (0xA, 0), (0xB, 2), (0xC, 5), (8, 0x3C)] {
                fx.mem.write_u8(r + o, v);
            }
            let x = fx.rand_from(D, 0x01E0_566C);
            fx.mem.set8(r + 9, x >> 4);
            fx.mem.write_u32(r + 0x10, BYTES);
            fx.mem.write_u32(r + 0x18, 0x3F80_0000);
            for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
                fx.mem.write_u8(r + o, 0x80);
            }
            copy_v4(fx, at, r + 0x28);
        }
        s += 1;
    }
    for k in 0..2u32 {
        let r = D + 0xAC + 0x4C * k;
        set_counter4(fx, r, 0x90, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 7, 9);
        if k != 0 {
            fx.mem.write_u8(r + 4, 3);
            fx.mem.write_u32(r + 0x10, 0x4348_0000);
        } else {
            fx.mem.write_u8(r + 4, 0x85);
            fx.mem.write_u32(r + 0x10, 0x4416_0000);
        }
        for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x80), (8, 0x10)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0);
        copy_v4(fx, at, r + 0x1C);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x90, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, s + 0x20);
}

/// ((rand >> 4) % m - off) / 100 (rand's s0 = D).
fn spread(fx: &mut Fx, m: i32, off: i32, ra: u32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h - off), 100.0)
}

/// |z|, then negated unless the attacker faces 0x8000.
fn away(fx: &mut Fx, p: u32) {
    if fx.mem.f(p) < 0.0 {
        let z = ee::mul(fx.mem.f(p), -1.0);
        fx.mem.setf(p, z);
    }
    if fx.mem.read_u32(D + 0x30) != 0x8000 {
        let z = ee::mul(fx.mem.f(p), -1.0);
        fx.mem.setf(p, z);
    }
}

/// 65..192 [F 0x1E058AC..0x1E05EB4]: four comets, ten bouncing sprites and
/// a ribbon at the target (sp + 0x80).
fn throw(fx: &mut Fx, sp: u32) {
    let at = sp + 0x80;
    for _ in 0..4 {
        let i = first_free(fx, COMETS, 0x44, 0x50);
        if i < 0 {
            continue;
        }
        let r = COMETS + 0x44 * i as u32;
        set_counter4(fx, r, 0x12, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 4);
        fx.mem.write_u32(r + 8, D + 0x3B8C);
        fx.mem.write_u32(r + 0x28, 0xF0);
        for (o, v) in [(0x1Eu32, 4u8), (0x1F, 0), (0x21, 2), (0x22, 5), (0x23, 5), (0x36, 0xB0), (0x35, 0xE0), (0x34, 0x80), (0x37, 0xC0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x2C, 0x41F0_0000);
        fx.mem.write_u32(r + 0x30, 0x41F0_0000);
        let mut a = 0u32;
        while a < u32::from(fx.mem.read_u8(r + 0x1E)) + 1 {
            fx.mem.set16(r + 0xC + 2 * a, -1);
            a += 1;
        }
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x18));
        node_position_saving(fx, h, slot, sp + 0x90, u64::from(D), Some(u64::from(OT)), 0x01E0_59B8);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(r + 0x38 + 4 * c, w);
        }
        let v = VEL + 12 * i as u32;
        let x = spread(fx, 300, 150, 0x01E0_59DC);
        fx.mem.setf(v, x);
        let y = spread(fx, 200, 100, 0x01E0_5A30);
        fx.mem.setf(v + 4, ee::add(f32::from_bits(0x3ECC_CCCD), y));
        let z = spread(fx, 400, 200, 0x01E0_5A8C);
        fx.mem.setf(v + 8, z);
        away(fx, v + 8);
    }
    for _ in 0..10 {
        let i = first_free(fx, BOUNCE, 0x44, 0xFA);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(D, 0x01E0_5B70) >> 4) & 7;
        let b = BOUNCE + 0x44 * i as u32;
        set_counter4(fx, b, 0x18, delay, 0xFF);
        for (o, x) in [(4u32, 0x40u8), (8, 0), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(b + o, x);
        }
        let c = (fx.rand_from(D, 0x01E0_5BC0) >> 4) & 3;
        fx.mem.set8(b + 9, c);
        for (o, x) in [(0xEu32, 0x80u8), (0xD, 0xC0), (0xC, 0xC0), (0xF, 0xC0)] {
            fx.mem.write_u8(b + o, x);
        }
        fx.mem.write_u32(b + 0x10, 0x4270_0000);
        fx.mem.write_u32(b + 0x14, 0xBF00_0000);
        fx.mem.write_u32(b + 0x18, 0xBDA3_D70A);
        fx.mem.write_u32(b + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        let x = spread(fx, 200, 100, 0x01E0_5C28);
        fx.mem.setf(b + 0x2C, x);
        let y = spread(fx, 100, 50, 0x01E0_5C74);
        fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3ECC_CCCD), y));
        let z = spread(fx, 200, 100, 0x01E0_5CD0);
        fx.mem.setf(b + 0x34, z);
        away(fx, b + 0x34);
    }
    let i = first_free(fx, RIBBON, 0x68, 0x3C);
    if i < 0 {
        return;
    }
    let r = RIBBON + 0x68 * i as u32;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4000_0000);
    fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
    for (o, v) in [(8u32, 8u8), (5, 0), (0xA, 0), (0xB, 2), (0xC, 6)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(D, 0x01E0_5E10);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x30), (0x27, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = (fx.rand_from(D, 0x01E0_5E60) >> 4) & 0x3FF;
    fx.mem.set32(r + 0x60, x);
    if fx.mem.read_u32(D + 0x30) != 0x8000 {
        let x = fx.mem.i32(r + 0x60).wrapping_neg();
        fx.mem.set32(r + 0x60, x);
    }
    copy_v4(fx, at, r + 0x28);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x80, u64::from(D), 0x01E0_4E40);
    match fx.mem.u16(D) {
        0xFA => return 8,
        0x41 => strike(fx, sp),
        0x31 => bands_start(fx, sp),
        0x14 => bars_start(fx, sp),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x41..0xD5).contains(&n) {
        crate::b369::unit_flag(fx, 1, u64::from(D), 0x01E0_5890);
    }
    let n = fx.mem.u16(D);
    if (0x41..0xC1).contains(&n) {
        throw(fx, sp);
    }
    for i in 0..0x50u32 {
        let r = COMETS + 0x44 * i;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let v = VEL + 12 * i;
        let y = ee::sub(fx.mem.f(v + 4), f32::from_bits(0x3DB8_51EC));
        fx.mem.setf(v + 4, y);
        for c in 0..3u32 {
            let p = ee::add(fx.mem.f(r + 0x38 + 4 * c), fx.mem.f(v + 4 * c));
            fx.mem.setf(r + 0x38 + 4 * c, p);
        }
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_5F60, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5F70, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0xA4, OT);
    // fade's frame (0xF0) below the entry's, as b387 leaves it [F9 run]:
    // its saved s4's upper half and s5 / s6 (the runner's 0); ra 0x1E05F80.
    let f = sp - 0xF0;
    fx.mem.write_u64(f + 0x58, 0);
    fx.saved(f, &[(0x60, 0), (0x70, 0), (0x80, 0x01E0_5F80)]);
    blib::band::bands(fx, 0x01E0_3B00, BANDS, OT, 0x30);
    blib::comet::comets_c(fx, 0x01E0_0410, COMETS, OT, 0x50);
    blib::star::flash(fx, 0x01E0_1410, D + 0xAC, OT, 5);
    blib::streak::ribbon(fx, 0x01E0_1FC0, RIBBON, OT, 0x3C);
    blib::sprite::bounce(fx, BOUNCE, OT, 0xFA);
    blib::cross::bars(fx, 0x01E0_42F0, BARS, OT, 4);
    let n = fx.mem.u16(D);
    if (0x41..0xD2).contains(&n) {
        6
    } else if n == 0xD2 {
        2
    } else if n == 0x37 {
        1
    } else {
        0
    }
}
