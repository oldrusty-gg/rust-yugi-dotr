//! BMODULE b387 (card 487), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04510, data area a0 = 0x1ED0000) [F] (helper F9,
//! 3 Nov 2026): b468's library family (s0 = D, s1 = the OT). On update 12
//! four bars (+0xA284 + 0x50 k on node 4, the first 1000 long at 45
//! degrees, the others 400 at 45 (k + 1)), a flash (+0xAC) and a camera
//! shake there (+0x40); on 45 the fade (+0xA4), two flashes at the target
//! (+0xAC / +0xF8: 800 / 200 wide) and a long shake on the target's
//! position; on 45..104 every update four comets (+0x228C, 0x50 records,
//! the point pool +0x3B8C, `comet::comets_c`) thrown from the target with
//! random velocities (+0x37CC, 12 bytes each; falling 0.09 an update), ten
//! bouncing sprites (+0x4A8C, 250 records) and a ribbon (+0xA2C, 60
//! records, the random bytes +0xA3C4); the stage light by 0x1031C0
//! (+0xA0). Returns 1 on update 35, 6 on 45..101, 2 on 102, 8 on 160 (and
//! at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `comets_c`
//! 0x1E00410, `flash` 0x1E01410, `ribbon` 0x1E01FC0, `bounce` 0x1E02B50,
//! `fade` 0x1E035C0, `bars` 0x1E03B00 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const COMETS: u32 = D + 0x228C;
const VEL: u32 = D + 0x37CC;
const BOUNCE: u32 = D + 0x4A8C;
const RIBBON: u32 = D + 0xA2C;
const BARS: u32 = D + 0xA284;
const BYTES: u32 = D + 0xA3C4;

/// The entry 0x1E04510 (a0 = D, a1 = param; s0 = D, s1 = the OT, s2 =
/// param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0xA4D0).
    fx.bzero(D, 0xA4D0);
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
    let r = fx.request_node(h, 4, 1);
    fx.mem.set32(D + 0xC, r);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x9A);
    // The entry's s0 = D, s1 = the OT, s2 = 2 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4618, [Some(D), Some(OT), Some(2)]);
    0
}

/// Update 12 [F 0x1E04670]: four bars, a flash and the shake on node +0xC.
fn bars_start(fx: &mut Fx) {
    for k in 0..4i32 {
        let r = BARS + 0x50 * k as u32;
        set_counter4(fx, r, 0x12, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 4, 0x80);
        if k == 0 {
            fx.mem.write_u32(r + 0x10, 0x447A_0000);
            fx.mem.write_u32(r + 0x18, 0x4234_0000);
            fx.mem.write_u32(r + 0x14, 0x41A0_0000);
        } else {
            fx.mem.write_u32(r + 0x10, 0x43C8_0000);
            fx.mem.setf(r + 0x18, ee::add(45.0, ee::mul(45.0, ee::from_i32(k))));
            fx.mem.write_u32(r + 0x14, 0x4170_0000);
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0xC0), (9, 0x80), (8, 0x30)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x40C0_0000);
    }
    let r = D + 0xAC;
    set_counter4(fx, r, 0x12, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x4334_0000);
    fx.mem.write_u32(r + 0x14, 0x4120_0000);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x40), (9, 0x10), (8, 0x80), (0xF, 0xD0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x12, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, b);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x4100_0000);
}

/// Update 45 [F 0x1E048FC]: the fade, two flashes at the target (sp +
/// 0x60) and the long shake there.
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0xA4, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0xA8, 1);
    for k in 0..2u32 {
        let r = D + 0xAC + 0x4C * k;
        set_counter4(fx, r, 0x4E, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 7, 9);
        if k != 0 {
            fx.mem.write_u8(r + 4, 3);
            fx.mem.write_u32(r + 0x10, 0x4348_0000);
        } else {
            fx.mem.write_u8(r + 4, 0x83);
            fx.mem.write_u32(r + 0x10, 0x4448_0000);
        }
        for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x80), (8, 0x10)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0);
        copy_v4(fx, sp + 0x60, r + 0x1C);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x4C, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x60, s + 0x20);
}

/// ((rand >> 4) % m - off) / 100 (rand's s0 = D).
fn spread(fx: &mut Fx, m: i32, off: i32, ra: u32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h - off), 100.0)
}

/// 45..104 [F 0x1E04ABC..0x1E05064]: four comets, ten bouncing sprites
/// and a ribbon at the target (sp + 0x60).
fn throw(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
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
        node_position_saving(fx, h, slot, sp + 0x70, u64::from(D), Some(u64::from(OT)), 0x01E0_4BC8);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(r + 0x38 + 4 * c, w);
        }
        let v = VEL + 12 * i as u32;
        let x = spread(fx, 300, 150, 0x01E0_4BEC);
        fx.mem.setf(v, x);
        let y = spread(fx, 200, 100, 0x01E0_4C40);
        fx.mem.setf(v + 4, ee::add(f32::from_bits(0x3ECC_CCCD), y));
        let z = spread(fx, 400, 200, 0x01E0_4C9C);
        fx.mem.setf(v + 8, z);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let z = ee::mul(fx.mem.f(v + 8), -1.0);
            fx.mem.setf(v + 8, z);
        }
    }
    for _ in 0..10 {
        let i = first_free(fx, BOUNCE, 0x44, 0xFA);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(D, 0x01E0_4D50) >> 4) & 7;
        let b = BOUNCE + 0x44 * i as u32;
        set_counter4(fx, b, 0x18, delay, 0xFF);
        for (o, x) in [(4u32, 0x40u8), (8, 0), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(b + o, x);
        }
        let c = (fx.rand_from(D, 0x01E0_4DA0) >> 4) & 3;
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
        let x = spread(fx, 200, 100, 0x01E0_4E08);
        fx.mem.setf(b + 0x2C, x);
        let y = spread(fx, 100, 50, 0x01E0_4E54);
        fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3ECC_CCCD), y));
        let z = spread(fx, 200, 100, 0x01E0_4EB0);
        fx.mem.setf(b + 0x34, z);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let z = ee::mul(fx.mem.f(b + 0x34), -1.0);
            fx.mem.setf(b + 0x34, z);
        }
    }
    let i = first_free(fx, RIBBON, 0x68, 0x3C);
    if i < 0 {
        return;
    }
    let r = RIBBON + 0x68 * i as u32;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4020_0000);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    for (o, v) in [(8u32, 8u8), (5, 0), (0xA, 0), (0xB, 2), (0xC, 6)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(D, 0x01E0_4FC0);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x10), (0x27, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = (fx.rand_from(D, 0x01E0_5010) >> 4) & 0x3FF;
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
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_4640);
    match fx.mem.u16(D) {
        0xA0 => return 8,
        0x2D => strike(fx, sp),
        0xC => bars_start(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x2D..0x69).contains(&n) {
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
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_5110, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5120, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0xA4, OT);
    // fade's frame (0xF0) stays below the entry's: its saved s4's upper
    // half (0) and s5 / s6 (the runner's 0) are what the comets' tail
    // descriptor finds as TEX0 / TEX1 for a missing texture [run]; ra
    // 0x1E05130.
    let f = sp - 0xF0;
    fx.mem.write_u64(f + 0x58, 0);
    fx.saved(f, &[(0x60, 0), (0x70, 0), (0x80, 0x01E0_5130)]);
    blib::comet::comets_c(fx, 0x01E0_0410, COMETS, OT, 0x50);
    blib::star::flash(fx, 0x01E0_1410, D + 0xAC, OT, 5);
    blib::streak::ribbon(fx, 0x01E0_1FC0, RIBBON, OT, 0x3C);
    blib::sprite::bounce(fx, BOUNCE, OT, 0xFA);
    blib::cross::bars(fx, 0x01E0_3B00, BARS, OT, 4);
    let n = fx.mem.u16(D);
    if (0x2D..0x66).contains(&n) {
        6
    } else if n == 0x66 {
        2
    } else if n == 0x23 {
        1
    } else {
        0
    }
}
