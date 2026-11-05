//! BMODULE b173 (card 98), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04C60, data area a0 = 0x1ED0000, 0xB570 bytes)
//! [F]: s2 = D, s7 = the OT (0x206E40), s0 the
//! parameter (then the spawn's offset / a pointer). The entry first copies
//! the record's 17-word pulse table (0x1E05EC0) to sp + 0x90. Update 2 the
//! frame fade (+0xB440, `window::frame_fade`, updates 2..250) and the
//! attacker's streaks (+0xB408, `radiate::streaks`, 224 counts, their far
//! alpha pulsed by the table from update 16); 90 a long camera shake
//! (+0x40). On every third update 70..200 a bolt (+0x8E10, 0x54 apart,
//! drawn by `arch::arches`) along 21 points (+0x97E8, 240 bytes apart,
//! 0x102F20) from node 0xC to the target jittered; each bolt walks its
//! points and on its 19th step bursts at its end: three sheets (+0x73D0),
//! a flash (+0x6AE8), a ribbon (+0x81E0) and 25 bouncing sprites (+0xA0).
//! Returns 6 on updates 90..222, 2 on 87, 1 on 80, 8 on 256 (no drawing
//! then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `ribbon` 0x1E00FC0, `fade` 0x1E01B50, `arches` 0x1E02090,
//! `sheets` 0x1E02980, `bounce` 0x1E02EF0, `streaks` 0x1E03960,
//! `frame_fade` 0x1E04780 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{aim_angles, first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PULSE: u32 = 0x01E0_5EC0;
const STREAKS: u32 = D + 0xB408;
const FADE: u32 = D + 0xB440;
const BOLTS: u32 = D + 0x8E10;
const POINTS: u32 = D + 0x97E8;
const BYTES: u32 = D + 0xB470;
const PI: u32 = 0x4049_0FDB;
const TWO_PI: u32 = 0x40C9_0FDB;

/// The entry 0x1E04C60 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x170);
    for k in (0..0x40u32).step_by(8) {
        let v = fx.mem.read_u64(PULSE + k);
        fx.mem.write_u64(sp + 0x90 + k, v);
    }
    let v = fx.mem.read_u32(PULSE + 0x40);
    fx.mem.write_u32(sp + 0xD0, v);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x170);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0xB570).
    fx.bzero(D, 0xB570);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..4 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0xC, 1), (0x10, 0xA, 0), (0x14, 0xB, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The entry's s0 = the parameter, s1 the runner's 0, s2 = D.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4DC8, [Some(param as u32), Some(0), Some(D)]);
    0
}

/// 0x101940 `aim_angles(v, out)` leaving its second atan2f's 0x1A5020
/// frame: ra 0x1019B4 (`sd`, 64 bits) at the caller's sp - 0x38 [F; run:
/// the next arch's second Q word reads its upper half].
fn aim_saving(fx: &mut Fx, v: u32, out: u32) {
    aim_angles(fx, v, out);
    let sp = fx.sp;
    fx.mem.write_u64(sp - 0x38, 0x0010_19B4);
}

/// rand from return address `ra` with the entry's s0 (64 bits) saved in
/// its frame.
fn rand64(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand()
}

/// ((rand >> 4) % m) x 2 - 2 `half` over 100 (the bolts' jitter).
fn spread(fx: &mut Fx, s0: u64, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand64(fx, s0, ra) >> 4;
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(200 * r - half), 100.0)
}

/// 0x102F20(p) [F]: p+0x28 = `n` + 1 points (12 bytes) of an arc from
/// p+0x10 to p+0x1C: the chord aimed by atan2 (x: of the horizontal
/// length and dy, y: of dx and dz, 65536 / 2 pi, 0x1A2B80) as a rotation
/// (0x20BDA0); point s = R x (p+4 x cos b, len / n x s, p+4 x sin b) +
/// from, b = p+8 + s p+0xC degrees (libm cosf / sinf), each through
/// 0x20C540 (its w written over the next point's x).
pub(crate) fn arc_points(fx: &mut Fx, p: u32) {
    let sp = fx.enter(0xD0);
    let mut out = fx.mem.read_u32(p + 0x28);
    for c in 0..3u32 {
        let v = fx.mem.read_u32(p + 0x10 + 4 * c);
        fx.mem.write_u32(out + 4 * c, v);
    }
    out += 0xC;
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(p + 0x1C + 4 * c), fx.mem.f(p + 0x10 + 4 * c));
        fx.mem.setf(sp + 0xA0 + 4 * c, v);
    }
    let len = blib::beam::length3(fx, sp + 0xA0);
    let n = fx.mem.i32(p);
    let (dx, dz) = (fx.mem.f(sp + 0xA0), fx.mem.f(sp + 0xA8));
    // mula.s / madd.s: the products rounded, then the sum.
    let h2 = ee::add(ee::mul(dx, dx), ee::mul(dz, dz));
    let fnn = ee::from_i32(n);
    let f21 = ee::div(180.0, fnn);
    let f20 = ee::div(len, fnn);
    let h = fx.sqrtf(h2);
    let dy = fx.mem.f(sp + 0xA4);
    let a = fx.atan2f(h, dy);
    let f12 = ee::mul(65536.0, ee::div(a, f32::from_bits(TWO_PI)));
    fx.fix_residue(f12, 0x0010_3024);
    fx.mem.write_u32(sp + 0xC0, ee::fixsfsi(f12) as u32 & 0xFFFF);
    let (dx, dz) = (fx.mem.f(sp + 0xA0), fx.mem.f(sp + 0xA8));
    let a = fx.atan2f(dx, dz);
    let f12 = ee::mul(65536.0, ee::div(a, f32::from_bits(TWO_PI)));
    fx.fix_residue(f12, 0x0010_3060);
    fx.mem.write_u32(sp + 0xC4, ee::fixsfsi(f12) as u32 & 0xFFFF);
    fx.mem.write_u32(sp + 0xC8, 0);
    let rot = [fx.mem.i32(sp + 0xC0), fx.mem.i32(sp + 0xC4), 0];
    let m = fx.rotation_matrix(rot);
    fx.write_m4(sp + 0x60, &m);
    let mut s1 = 1i32;
    while s1 < fx.mem.i32(p) + 1 {
        let fs = ee::from_i32(s1);
        let a = ee::div(ee::mul(f32::from_bits(PI), ee::mul(f21, fs)), 180.0);
        let sn = fx.libm_sinf(a);
        let f22 = ee::mul(fx.mem.f(p + 4), sn);
        let b = ee::add(fx.mem.f(p + 8), ee::mul(fs, fx.mem.f(p + 0xC)));
        let f23 = ee::div(ee::mul(f32::from_bits(PI), b), 180.0);
        let c = libm_cosf(fx, f23);
        fx.mem.setf(sp + 0xB0, ee::mul(f22, c));
        let s = fx.libm_sinf(f23);
        fx.mem.setf(sp + 0xB8, ee::mul(f22, s));
        fx.mem.setf(sp + 0xB4, ee::mul(f20, fs));
        let m = fx.read_m4(sp + 0x60);
        let v = fx.read_v4(sp + 0xB0);
        let r = fx.transform(Some(&m), v);
        fx.write_v4(out, r);
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(out + 4 * c), fx.mem.f(p + 0x10 + 4 * c));
            fx.mem.setf(out + 4 * c, v);
        }
        s1 += 1;
        out += 0xC;
    }
    fx.leave(0xD0);
}

/// Every third update 70..200 [F 0x1E050FC]: a bolt from node 0xC to the
/// target. Returns the entry's s0 after it.
fn bolt(fx: &mut Fx, sp: u32, s0: u64) -> u64 {
    let i = first_free(fx, BOLTS, 0x54, 0x1E);
    if i < 0 {
        return s0;
    }
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, s, sp + 0xF0, s0, Some(0), 0x01E0_5134);
    fx.mem.set32(sp + 0x120, 0x14);
    let f = ee::add(20.0, spread(fx, s0, 0x01E0_5144, 0xF, 0x5DC));
    fx.mem.setf(sp + 0x124, f);
    let f = spread(fx, s0, 0x01E0_51AC, 0xB4, 0x4650);
    fx.mem.setf(sp + 0x128, f);
    let s1 = 240 * i as u32;
    fx.mem.write_u32(sp + 0x148, POINTS + s1);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0xF0 + 4 * c);
        fx.mem.write_u32(sp + 0x130 + 4 * c, w);
    }
    for (c, ra, m, half) in [(0u32, 0x01E0_5238u32, 0xF, 0x5DC), (1, 0x01E0_5298, 0xF, 0x5DC), (2, 0x01E0_52F8, 0xA, 0x3E8)] {
        let f = ee::add(fx.mem.f(sp + 0xE0 + 4 * c), spread(fx, s0, ra, m, half));
        fx.mem.setf(sp + 0x13C + 4 * c, f);
    }
    arc_points(fx, sp + 0x120);
    let s0 = 0x54 * i as u32;
    let r = BOLTS + s0;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    for (o, v) in [(4u32, 0xC0u8), (5, 0), (7, 6), (0xE, 0x20), (0xD, 0x80), (0xC, 0xC0), (0xF, 0x20), (8, 0xF0), (9, 0xF0), (0xA, 0xF0), (0xB, 0x60)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, 0x4100_0000);
    fx.mem.write_u32(r + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(r + 0x18, 0);
    fx.mem.write_u32(r + 0x1C, 0);
    let v = rand64(fx, u64::from(s0), 0x01E0_5474) >> 4;
    fx.mem.set32(r + 0x20, v & 0x3FF);
    let p = POINTS + s1;
    for c in 0..3u32 {
        let w = fx.mem.read_u32(p + 4 * c);
        fx.mem.write_u32(r + 0x24 + 4 * c, w);
    }
    for o in [0x34u32, 0x38, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(p + 0xC + 4 * c), fx.mem.f(r + 0x24 + 4 * c));
        fx.mem.setf(sp + 0x150 + 4 * c, v);
    }
    aim_saving(fx, sp + 0x150, r + 0x44);
    u64::from(s0)
}

/// A bolt's end [F 0x1E05678]: sheets, a flash, a ribbon and bouncing
/// sprites at its last point (sp + 0xF0).
fn burst(fx: &mut Fx, sp: u32, s3: u32) {
    let r = BOLTS + 0x54 * s3;
    let p = POINTS + 240 * s3;
    for c in 0..3u32 {
        let w = fx.mem.read_u32(p + 0xE4 + 4 * c);
        fx.mem.write_u32(sp + 0xF0 + 4 * c, w);
    }
    for s5 in 0..3i32 {
        let i = first_free(fx, D + 0x73D0, 0x3C, 0x3C);
        if i < 0 {
            continue;
        }
        let q = D + 0x73D0 + 0x3C * i as u32;
        set_counter4(fx, q, 0xE, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0x80), (8, 2), (0xA, 0), (9, 6), (0xB, 2), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        copy_v4(fx, sp + 0xF0, q + 0x2C);
        let dy = ee::add(-1.5, ee::mul(1.5, ee::from_i32(s5)));
        let y = ee::add(fx.mem.f(q + 0x30), dy);
        fx.mem.setf(q + 0x30, y);
        fx.mem.write_u32(q + 0x10, 0x3FC0_0000);
        fx.mem.write_u32(q + 0x14, 0x3F80_0000);
        fx.mem.write_u32(q + 0x18, if s5 == 1 { 0x40A0_0000 } else { 0x4080_0000 });
        let t = fx.mem.read_u32(r + 0x1C);
        fx.mem.write_u32(q + 0x24, t);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let f = ee::mul(ee::from_i32(fx.mem.i32(q + 0x24)), -1.0);
            fx.fix_residue(f, 0x01E0_5814);
            fx.mem.set32(q + 0x24, ee::fixsfsi(f));
        }
    }
    let i = first_free(fx, D + 0x6AE8, 0x4C, 0x1E);
    if i >= 0 {
        let a = D + 0x6AE8 + 0x4C * i as u32;
        set_counter4(fx, a, 0x12, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0x80), (7, 9)] {
            fx.mem.write_u8(a + o, v);
        }
        fx.mem.write_u32(a + 0x10, 0x4348_0000);
        fx.mem.write_u32(a + 0x14, 0x41C8_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xA, 0x80), (9, 0x20), (8, 0x60), (0xF, 0xC0)] {
            fx.mem.write_u8(a + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(a + o, 0);
        }
        copy_v4(fx, sp + 0xF0, a + 0x1C);
        fx.mem.write_u32(a + 0x18, 0x4170_0000);
    }
    let i = first_free(fx, D + 0x81E0, 0x68, 0x1E);
    if i >= 0 {
        let q = D + 0x81E0 + 0x68 * i as u32;
        set_counter4(fx, q, 0x10, 0, 0xFF);
        fx.mem.write_u8(q + 4, 5);
        fx.mem.write_u32(q + 0x14, 0x40A0_0000);
        fx.mem.write_u32(q + 0x1C, 0x4220_0000);
        for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0x28)] {
            fx.mem.write_u8(q + o, v);
        }
        let x = fx.rand_from((sp + 0xF4) as u32, 0x01E0_59E4);
        fx.mem.set8(q + 9, x >> 4);
        fx.mem.write_u32(q + 0x10, BYTES);
        fx.mem.write_u32(q + 0x18, 0x3F80_0000);
        for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        copy_v4(fx, sp + 0xF0, q + 0x28);
        fx.mem.write_u32(q + 0x58, 0x40);
        fx.mem.write_u32(q + 0x5C, 0x80);
        fx.mem.write_u32(q + 0x60, 0x100);
    }
    let s0 = sp + 0xF4;
    for _ in 0..0x19 {
        let i = first_free(fx, D + 0xA0, 0x44, 0x190);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(s0, 0x01E0_5B0C) >> 4) & 7;
        let q = D + 0xA0 + 0x44 * i as u32;
        set_counter4(fx, q, 0x20, delay, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 0), (0xB, 2), (0xA, 0), (9, 3), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        let f = ee::add(90.0, spread(fx, u64::from(s0), 0x01E0_5B74, 0x1E, 0xBB8));
        fx.mem.setf(q + 0x10, f);
        fx.mem.write_u32(q + 0x14, 0xBECC_CCCD);
        fx.mem.write_u32(q + 0x18, 0xBD75_C28F);
        fx.mem.write_u32(q + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xF0 + 4 * c);
            fx.mem.write_u32(q + 0x20 + 4 * c, w);
        }
        let jit = |fx: &mut Fx, ra: u32| {
            let v = fx.rand_from(s0, ra) >> 4;
            let (_, m) = fx.div(v, 0xD2);
            ee::div(ee::from_i32(m - 0x69), 100.0)
        };
        let f = jit(fx, 0x01E0_5C14);
        fx.mem.setf(q + 0x2C, f);
        let f = jit(fx, 0x01E0_5C60);
        fx.mem.setf(q + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), f));
        let f = jit(fx, 0x01E0_5CBC);
        fx.mem.setf(q + 0x34, f);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    let mut s0 = i64::from(param) as u64;
    unit_position_saving(fx, t, sp + 0xE0, s0, 0x01E0_4DF0);
    match fx.mem.u16(D) {
        0x100 => return 8,
        0x5A => {
            set_counter4(fx, D + 0x40, 0x98, 0, 0xFF);
            fx.mem.write_u8(D + 0x45, 0);
            fx.mem.write_u8(D + 0x44, 4);
            fx.mem.write_u32(D + 0x54, 0x3FE6_6666);
            fx.mem.write_u32(D + 0x58, 0x3F80_0000);
            fx.mem.write_u32(D + 0x5C, 0x3FA6_6666);
            fx.mem.write_u32(D + 0x70, 0);
            fx.mem.write_u32(D + 0x74, 0x42A0_0000);
            fx.mem.write_u32(D + 0x90, 0x4080_0000);
            copy_v4(fx, sp + 0xE0, D + 0x60);
        }
        2 => {
            fx.mem.set16(FADE, 2);
            fx.mem.set16(FADE + 2, 0xFA);
            fx.mem.write_u8(FADE + 5, 6);
            fx.mem.write_u8(FADE + 6, 1);
            fx.mem.write_u32(FADE + 8, 0x4180_0000);
            for (o, v) in [(0x14u32, -0xCi32), (0xC, -0xC), (0x18, 0xC), (0x10, 0xC), (0x20, -0xC), (0x1C, -0xC), (0x28, 0xC), (0x24, 0xC)] {
                fx.mem.set32(FADE + o, v);
            }
            for o in [0x2Eu32, 0x2D, 0x2C] {
                fx.mem.write_u8(FADE + o, 0x70);
            }
            let s = STREAKS;
            set_counter4(fx, s, 0xE0, 0, 0xFF);
            for (o, v) in [(4u32, 0x80u8), (9, 1), (0xA, 0xFF), (8, 0), (0xB, 1)] {
                fx.mem.write_u8(s + o, v);
            }
            let h = fx.mem.read_u32(D + 4);
            fx.mem.write_u32(s + 0xC, h);
            for (o, v) in [(0x16u32, 0x80u8), (0x15, 0x80), (0x14, 0x80), (0x17, 0x20), (0x12, 0x80), (0x11, 0x80), (0x10, 0x80), (0x13, 0x20), (5, 1)] {
                fx.mem.write_u8(s + o, v);
            }
            let b = fx.mem.read_u8(D + 0x10);
            fx.mem.write_u8(s + 6, b);
        }
        _ => {}
    }
    if fx.mem.u8(STREAKS) > 0 {
        let k = ((fx.mem.u16(D) - 0x10) & 0xF) as u32;
        let v = fx.mem.read_u8(sp + 0x90 + 4 * k);
        fx.mem.write_u8(STREAKS + 0x13, v.wrapping_mul(2));
    }
    let n = fx.mem.u16(D);
    if (0x46..0xC9).contains(&n) {
        let (_, m) = fx.div(n, 3);
        if m == 0 {
            s0 = bolt(fx, sp, s0);
        }
    }
    for s3 in 0..0x1Eu32 {
        let r = BOLTS + 0x54 * s3;
        let c = fx.mem.u8(r);
        if c <= 0 {
            continue;
        }
        let k = fx.mem.u8(r + 2) - c;
        let p = POINTS + 240 * s3 + 12 * k as u32;
        for c in 0..3u32 {
            let w = fx.mem.read_u32(p + 4 * c);
            fx.mem.write_u32(r + 0x24 + 4 * c, w);
        }
        if k != 0x13 {
            for c in 0..3u32 {
                let v = ee::sub(fx.mem.f(p + 0xC + 4 * c), fx.mem.f(r + 0x24 + 4 * c));
                fx.mem.setf(sp + 0x160 + 4 * c, v);
            }
            aim_saving(fx, sp + 0x160, r + 0x44);
            continue;
        }
        fx.mem.write_u8(r, 0);
        burst(fx, sp, s3);
        s0 = u64::from(sp + 0xF4);
    }
    shake_saving(fx, D + 0x40, s0, 0x01E0_5D38, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::fade::fade(fx, D + 0x6AE0, OT);
    // fade's 0xF0 frame saves s6 (0x54 x 29, the bolt loop's last) at
    // +0x70 and ra at +0x80: the streaks' first TEXFLUSH reads them as
    // vertex 1 / 2's stale XYZ [run].
    fx.saved(sp - 0xF0, &[(0x70, 0x984), (0x80, 0x01E0_5D48)]);
    blib::streak::ribbon(fx, 0x01E0_0FC0, D + 0x81E0, OT, 1);
    blib::star::flash(fx, 0x01E0_0410, D + 0x6AE8, OT, 0x1E);
    blib::arch::arches(fx, 0x01E0_2090, BOLTS, OT, 0x1E);
    blib::sheet::sheets(fx, 0x01E0_2980, D + 0x73D0, OT, 0x3C);
    let (_, bounce_a3) = blib::sprite::bounce_a3(fx, D + 0xA0, OT, 0x190);
    // a3 = what streaks left (frame_fade's 0x207EF0 reports it), with the
    // streaks done what bounce left [run]; [H] neither counting: sheets'
    // (not modelled: 0).
    let (_, a3) = blib::radiate::streaks_a3(fx, 0x01E0_3960, STREAKS, OT, 1);
    let n = fx.mem.u16(D);
    blib::window::frame_fade(fx, 0x01E0_4780, FADE, OT, n, a3.or(bounce_a3).unwrap_or(0));
    let n = fx.mem.u16(D);
    if (0x5A..0xDF).contains(&n) {
        6
    } else if n == 0x57 {
        2
    } else if n == 0x50 {
        1
    } else {
        0
    }
}
