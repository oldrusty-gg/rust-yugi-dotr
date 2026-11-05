//! BMODULE b461 (card 536), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E034D0, data area a0 = 0x1ED0000) [F] (helper B6, 31
//! Oct 2026): the "a0 = D" family (s6 = D, s7 = the OT; s0 / s1 the
//! parameter, then loop counters). From update 2 a long camera shake on
//! node 0x2A and, until update 139, sparks (two short bolts, +0x2024) and
//! a small flash (+0x1904) jittered about node 1, until 49 a spray
//! (+0x3924, `spray::sprays`) from it; on update 50 six jagged bolts
//! (0x102A50) from 80 above node 1 down to it, side by side; on 170 a wide
//! flash high over the target; on 190 the fade (+0x5724), the shake at the
//! target and six jagged bolts down onto it; on even updates 170..327 two
//! ribbons (+0xA4) over the target; on 190..326 four flashes and ribbons
//! scattered about it. The stage light pulsed by 0x1031C0 (+0xA0).
//! Returns 1 on update 180, 6 on 190..326, 2 on 327, 8 on 350 (and at once
//! for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `ribbon` 0x1E00FC0, `bolts` 0x1E01B50, `fade` 0x1E026A0,
//! `sprays` 0x1E02BE0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x572C;
const FLASHES: u32 = D + 0x1904;
const RIBBONS: u32 = D + 0xA4;
const BOLTS: u32 = D + 0x2024;

/// The entry 0x1E034D0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1E0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x1E0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x5830);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x2A, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x158);
    // The entry's s0 = 2 (the texture loop's), s1 the runner's 0.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_35FC, [Some(2), Some(0), None]);
    0
}

/// rand() with the caller's s0 = `s0` (64 bits) from return address `ra`.
fn rand64(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand()
}

/// ((rand >> 4) % m - h) / 100.
fn jitter(fx: &mut Fx, s0: u64, ra: u32, m: i32, h: i32) -> f32 {
    let x = rand64(fx, s0, ra) >> 4;
    let (_, r) = fx.div(x, m);
    ee::div(ee::from_i32(r - h), 100.0)
}

/// The flash bytes the record's flashes share: +5 = 0, +4 = `b4`, +7 = 9,
/// colour 0xD0, +0xA = 0, +9 = `b9`, +8 = 0x80, width `w`, +0x14 = `h`.
fn flash_at(fx: &mut Fx, r: u32, b4: u8, b9: u8, w: u32, h: u32, at: u32) {
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, b4);
    fx.mem.write_u8(r + 7, 9);
    for (o, v) in [(0xFu32, 0xD0u8), (0xC, 0xD0), (0xD, 0xD0), (0xE, 0xD0), (0xA, 0), (9, b9), (8, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0);
    fx.mem.write_u32(r + 0x10, w);
    fx.mem.write_u32(r + 0x14, h);
    copy_v4(fx, at, r + 0x1C);
}

/// The bolt bytes after its points: width `w`, sizes 1 / `size`, +0xB 1,
/// +0xD 4, +0xC 0, white, alpha 0x30.
fn bolt_look(fx: &mut Fx, b: u32, w: u8, size: u32) {
    fx.mem.write_u8(b + 8, w);
    fx.mem.write_u32(b + 0x10, 0x3F80_0000);
    fx.mem.write_u32(b + 0x14, size);
    for (o, v) in [(0xBu32, 1u8), (0xD, 4), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x30)] {
        fx.mem.write_u8(b + o, v);
    }
}

/// Bolt pass `k` of six [F 0x1E03750 / 0x1E03C20]: the end points sp +
/// 0xC0 / + 0xD0 moved by `side` (cases 0..3 along x / z) or `both`
/// (cases 4 / 5 on both axes, + / -), a jagged path of five segments
/// (0x102A50, spec sp + 0xE0, points sp + 0x110) and its segments as
/// bolts (count 0x8C, delay k & 1). Returns the segment loop's counter.
fn bolt_pass(fx: &mut Fx, sp: u32, k: u32, side: f32, both: f32) -> u64 {
    let (a, b) = (sp + 0xC0, sp + 0xD0);
    let shift = |fx: &mut Fx, o: u32, d: f32| {
        for p in [a + o, b + o] {
            let v = ee::add(fx.mem.f(p), d);
            fx.mem.setf(p, v);
        }
    };
    match k {
        0 => shift(fx, 0, ee::neg(side)),
        1 => shift(fx, 0, side),
        2 => shift(fx, 8, ee::neg(side)),
        3 => shift(fx, 8, side),
        4 => {
            for o in [0u32, 8] {
                for p in [a + o, b + o] {
                    let v = ee::add(fx.mem.f(p), both);
                    fx.mem.setf(p, v);
                }
            }
        }
        _ => {
            for o in [0u32, 8] {
                for p in [a + o, b + o] {
                    let v = ee::sub(fx.mem.f(p), both);
                    fx.mem.setf(p, v);
                }
            }
        }
    }
    let p = sp + 0xE0;
    fx.mem.set32(p, 5);
    fx.mem.write_u32(p + 4, 0x3ECC_CCCD);
    fx.mem.write_u32(p + 0x20, sp + 0x110);
    for (o, src) in [(8u32, a), (0xC, a + 4), (0x10, a + 8), (0x14, b), (0x18, b + 4), (0x1C, b + 8)] {
        let v = fx.mem.read_u32(src);
        fx.mem.write_u32(p + o, v);
    }
    blib::bolt::jag(fx, p);
    let mut s = 0u32;
    while s < 5 {
        let i = first_free(fx, BOLTS, 0x40, 0x64);
        if i < 0 {
            break;
        }
        let bo = BOLTS + ((i as u32) << 6);
        set_counter4(fx, bo, 0x8C, (k & 1) as i32, 0xFF);
        fx.mem.write_u8(bo + 5, 0);
        fx.mem.write_u8(bo + 4, 0);
        for c in 0..6u32 {
            let v = fx.mem.read_u32(sp + 0x110 + 12 * s + 4 * c);
            fx.mem.write_u32(bo + 0x1C + 4 * c, v);
        }
        bolt_look(fx, bo, 6, 0x3E99_999A);
        s += 1;
    }
    u64::from(s)
}

/// Update 50 [F 0x1E036D0]: six bolts from 80 above node +0xC down to 15
/// below it.
fn bolts_rise(fx: &mut Fx, sp: u32, s0: u64) -> u64 {
    let mut s0 = s0;
    for k in 0..6u32 {
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0xC);
        node_position_saving(fx, h, n, sp + 0xC0, s0, Some(u64::from(k)), 0x01E0_36F0);
        copy_v4(fx, sp + 0xC0, sp + 0xD0);
        let v = ee::add(fx.mem.f(sp + 0xC4), 80.0);
        fx.mem.setf(sp + 0xC4, v);
        let v = ee::sub(fx.mem.f(sp + 0xD4), 15.0);
        fx.mem.setf(sp + 0xD4, v);
        s0 = bolt_pass(fx, sp, k, 8.0, 6.0);
    }
    s0
}

/// Update 190 [F 0x1E03AF8]: the fade, the shake at the target and six
/// bolts from 80 above it down to 60 below.
fn bolts_fall(fx: &mut Fx, sp: u32, s0: u64) -> u64 {
    set_counter4(fx, D + 0x5724, 0x18, 0, 0xFF);
    fx.mem.write_u8(D + 0x5728, 1);
    let s = D + 0x40;
    set_counter4(fx, s, 0x8C, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4320_0000);
    copy_v4(fx, sp + 0xA0, D + 0x60);
    fx.mem.write_u32(s + 0x50, 0x4100_0000);
    fx.mem.write_u32(D + 0x68, 0);
    let mut s0 = s0;
    for k in 0..6u32 {
        copy_v4(fx, sp + 0xA0, sp + 0xD0);
        copy_v4(fx, sp + 0xD0, sp + 0xC0);
        let v = ee::add(fx.mem.f(sp + 0xC4), 80.0);
        fx.mem.setf(sp + 0xC4, v);
        let v = ee::sub(fx.mem.f(sp + 0xD4), 60.0);
        fx.mem.setf(sp + 0xD4, v);
        s0 = bolt_pass(fx, sp, k, 18.0, 10.0);
    }
    s0
}

/// Updates 2..139 [F 0x1E03F0C]: two short bolts and a small flash at a
/// point jittered +-8 about node +0xC (sp + 0xD0).
fn sparks(fx: &mut Fx, sp: u32, s0: u64, s1: u64) -> u64 {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0xC0, s0, Some(s1), 0x01E0_3F20);
    for (c, ra) in [(0u32, 0x01E0_3F28u32), (4, 0x01E0_3F78), (8, 0x01E0_3FC8)] {
        let j = jitter(fx, s0, ra, 0x640, 0x320);
        let v = ee::add(fx.mem.f(sp + 0xC0 + c), j);
        fx.mem.setf(sp + 0xD0 + c, v);
    }
    let mut s = 0u32;
    while s < 2 {
        let i = first_free(fx, BOLTS, 0x40, 0x64);
        if i < 0 {
            break;
        }
        let b = BOLTS + ((i as u32) << 6);
        set_counter4(fx, b, 0xE, (s & 0xFF) as i32, 0xFF);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 4, 0);
        for c in 0..3u32 {
            let v = fx.mem.read_u32(sp + 0xD0 + 4 * c);
            fx.mem.write_u32(b + 0x28 + 4 * c, v);
            fx.mem.write_u32(b + 0x1C + 4 * c, v);
        }
        for (c, ra) in [(0u32, 0x01E0_40A0u32), (4, 0x01E0_4100), (8, 0x01E0_4160)] {
            let x = rand64(fx, u64::from(s), ra) >> 4;
            let (_, h) = fx.div(x, 6);
            let v = ee::add(fx.mem.f(b + 0x28 + c), ee::div(ee::from_i32(h * 200 - 0x258), 100.0));
            fx.mem.setf(b + 0x28 + c, v);
        }
        bolt_look(fx, b, 5, 0x3ECC_CCCD);
        s += 1;
    }
    let i = first_free(fx, FLASHES, 0x4C, 0x18);
    if i >= 0 {
        let r = FLASHES + 0x4C * i as u32;
        set_counter4(fx, r, 0xC, 0, 0xFF);
        flash_at(fx, r, 5, 0, 0x4348_0000, 0, sp + 0xD0);
    }
    u64::from(s)
}

/// Updates 2..49 [F 0x1E042EC]: a spray from node +0xC (sp + 0xC0).
fn spray(fx: &mut Fx, sp: u32, s0: u64, s1: u64) {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0xC0, s0, Some(s1), 0x01E0_4300);
    let i = first_free(fx, D + 0x3924, 0x60, 0x50);
    if i < 0 {
        return;
    }
    let r = D + 0x3924 + 0x60 * i as u32;
    set_counter4(fx, r, 0x20, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 2);
    let x = rand64(fx, s0, 0x01E0_435C);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u8(r + 0xA, 0);
    let x = rand64(fx, s0, 0x01E0_4374);
    fx.mem.set8(r + 0xB, (x >> 4) & 7);
    fx.mem.write_u8(r + 0xC, 0);
    fx.mem.write_u8(r + 0xD, 2);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x24, 0);
    for (o, v) in [(0x28u32, 0x80u8), (0x29, 0x80), (0x2A, 0x80), (0x2B, 0x60)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(r + 0x3C, x);
    let y = ee::sub(fx.mem.f(sp + 0xC4), 15.0);
    fx.mem.setf(r + 0x40, y);
    let z = fx.mem.read_u32(sp + 0xC8);
    fx.mem.write_u32(r + 0x44, z);
    fx.mem.write_u8(r + 8, 6);
    fx.mem.write_u32(r + 0x1C, 0x41A0_0000);
    let x = (rand64(fx, s0, 0x01E0_43F4) >> 4).wrapping_rem(2);
    let v = ee::add(10.0, ee::div(ee::from_i32(x * 200 - 0xC8), 100.0));
    fx.mem.setf(r + 0x14, v);
    fx.mem.write_u32(r + 0x20, 0x41C8_0000);
    let v = ee::add(0.5, jitter(fx, s0, 0x01E0_446C, 0x50, 0x28));
    fx.mem.setf(r + 0x4C, v);
}

/// The ribbon bytes from +5 on (+0xA 0, +0xB 2, +0xC 3, +8 `b8`, +9 rand,
/// the random bytes, +0x18 1, white).
fn ribbon_bytes(fx: &mut Fx, r: u32, b8: u8, s0: u64, ra: u32) {
    for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3), (8, b8)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = rand64(fx, s0, ra);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
}

/// Even updates 170..327 [F 0x1E044E4]: two ribbons 80 over the target.
fn ribbons(fx: &mut Fx, sp: u32, s0: u64) {
    for _ in 0..2 {
        let i = first_free(fx, RIBBONS, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let r = RIBBONS + 0x68 * i as u32;
        set_counter4(fx, r, 0x10, 0, 0xFF);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u32(r + 0x14, 0x4170_0000);
        fx.mem.write_u32(r + 0x1C, 0x42C8_0000);
        ribbon_bytes(fx, r, 0x18, s0, 0x01E0_4580);
        fx.mem.write_u8(r + 0x23, 0x10);
        fx.mem.write_u8(r + 0x27, 0x40);
        for (o, ra) in [(0x58u32, 0x01E0_45D0u32), (0x5C, 0x01E0_45E8), (0x60, 0x01E0_4600)] {
            let x = rand64(fx, s0, ra) >> 4;
            fx.mem.set32(r + o, x & 0x7F);
        }
        copy_v4(fx, sp + 0xA0, r + 0x28);
        let y = ee::add(fx.mem.f(r + 0x2C), 80.0);
        fx.mem.setf(r + 0x2C, y);
    }
}

/// Updates 190..326 [F 0x1E04684]: four points +-14 about the target (sp +
/// 0xB0), each with a flash and a ribbon.
fn scatter(fx: &mut Fx, sp: u32) {
    for k in 0..4u32 {
        let s0 = u64::from(k);
        for (c, ra) in [(0u32, 0x01E0_4698u32), (4, 0x01E0_46E4), (8, 0x01E0_4730)] {
            let j = jitter(fx, s0, ra, 0xAF0, 0x578);
            let v = ee::add(fx.mem.f(sp + 0xA0 + c), j);
            fx.mem.setf(sp + 0xB0 + c, v);
        }
        let i = first_free(fx, FLASHES, 0x4C, 0x18);
        if i >= 0 {
            let r = FLASHES + 0x4C * i as u32;
            set_counter4(fx, r, 0xC, 0, 0xFF);
            flash_at(fx, r, 1, 0, 0x4396_0000, 0, sp + 0xB0);
        }
        let i = first_free(fx, RIBBONS, 0x68, 0x3C);
        if i >= 0 {
            let r = RIBBONS + 0x68 * i as u32;
            fx.mem.write_u8(r + 4, 5);
            set_counter4(fx, r, 8, 0, 0xFF);
            fx.mem.write_u32(r + 0x14, 0x40A0_0000);
            fx.mem.write_u32(r + 0x1C, 0x4220_0000);
            fx.mem.write_u8(r + 0x27, 0x80);
            fx.mem.write_u8(r + 0x23, 0);
            ribbon_bytes(fx, r, 0x28, s0, 0x01E0_48CC);
            copy_v4(fx, sp + 0xB0, r + 0x28);
        }
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut s0 = i64::from(param) as u64;
    // The interpreter runner's s1 is 0 at the entry [run, helper B3].
    let mut s1 = 0u64;
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xA0, s0, 0x01E0_3624);
    match fx.mem.u16(D) {
        0x15E => return 8,
        0xBE => {
            s0 = bolts_fall(fx, sp, s0);
            s1 = 6;
        }
        0xAA => {
            let i = first_free(fx, FLASHES, 0x4C, 0x18);
            if i >= 0 {
                let r = FLASHES + 0x4C * i as u32;
                set_counter4(fx, r, 0x20, 0, 0xFF);
                flash_at(fx, r, 3, 0x40, 0x43FA_0000, 0x4334_0000, sp + 0xA0);
                let y = ee::add(fx.mem.f(r + 0x20), 80.0);
                fx.mem.setf(r + 0x20, y);
            }
        }
        0x32 => {
            s0 = bolts_rise(fx, sp, s0);
            s1 = 6;
        }
        2 => {
            let s = D + 0x40;
            set_counter4(fx, s, 0xFA, 0, 0xFF);
            fx.mem.write_u8(s + 5, 1);
            let b = fx.mem.read_u8(D + 0x10);
            fx.mem.write_u8(s + 6, b);
            fx.mem.write_u8(s + 4, 8);
            fx.mem.write_u32(s + 0x14, 0x3F80_0000);
            fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
            fx.mem.write_u32(s + 0x1C, 0x3FE6_6666);
            fx.mem.write_u32(s + 0x30, 0);
            fx.mem.write_u32(s + 0x34, 0x4220_0000);
            fx.mem.write_u32(s + 0x50, 0x4080_0000);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0x8C).contains(&n) {
        s0 = sparks(fx, sp, s0, s1);
    }
    let n = fx.mem.u16(D);
    if (2..0x32).contains(&n) {
        spray(fx, sp, s0, s1);
    }
    let n = fx.mem.u16(D);
    if (0xAA..0x148).contains(&n) && n & 1 == 0 {
        ribbons(fx, sp, s0);
        s1 = 2;
    }
    let n = fx.mem.u16(D);
    if (0xBE..0x147).contains(&n) {
        scatter(fx, sp);
        s0 = 4;
    }
    shake_saving(fx, D + 0x40, s0, 0x01E0_494C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_495C, [Some(s0), Some(s1)]);
    blib::fade::fade(fx, D + 0x5724, OT);
    blib::star::flash(fx, 0x01E0_0410, FLASHES, OT, 0x18);
    blib::streak::ribbon(fx, 0x01E0_0FC0, RIBBONS, OT, 0x3C);
    blib::bolt::bolts(fx, 0x01E0_1B50, BOLTS, OT, 0x64);
    blib::spray::sprays(fx, 0x01E0_2BE0, D + 0x3924, OT, 0x50);
    let n = fx.mem.u16(D);
    if (0xBE..0x147).contains(&n) {
        6
    } else if n == 0x147 {
        2
    } else if n == 0xB4 {
        1
    } else {
        0
    }
}
