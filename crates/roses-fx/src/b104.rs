//! BMODULE b104 (card 439), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03ED0, data area a0 = 0x1ED0000) [F] (helper B6, 31
//! Oct 2026): the "a0 = D" family (s1 = D, s7 = the OT; s0 the parameter,
//! then pointers and counts). Node bursts (+0xA0, motes +0xE4) every
//! update; on updates 114..123 eleven trails (+0x1B40, the record's own
//! 0x1E03510 `trails`, started on 114) swing in a half circle about the
//! attacker's node 0x26, the circle turned from node 0x24 towards it
//! (0x101A00) with a radius from node 0x22's distance (0x20C850); on 124
//! two fans of 18 dashes (+0x1000, `dash::dashes`) from node 0x26, a
//! camera shake and a flash there and four bead beams (+0x1E58). Returns
//! 1 on update 114, 2 on 121, 8 on 204 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `beads` 0x1E015F0, `flash`
//! 0x1E023C0, `dashes` 0x1E02F70 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving};
use crate::blib::{self, exe::{camera_vp_into, countdown, first_free, libm_cosf, node_source, set_counter, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x2558;
const TRAILS: u32 = D + 0x1B40;
const DASHES: u32 = D + 0x1000;
const PI: u32 = 0x4049_0FDB;

/// The entry 0x1E03ED0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x2660);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (at, node, clear) in [(0xCu32, 0xC, 1), (0x10, 0x37, 0), (0x14, 0x26, 0), (0x18, 0x22, 0), (0x1C, 0x24, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA0): specs 0x1E04AA0 / 0x1E04AC0, motes +0xE4 /
    // +0x7E4.
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(D + 0xC8, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xC9, b);
    fx.mem.write_u32(D + 0xA8, 0x01E0_4AA0);
    fx.mem.write_u32(D + 0xAC, 0x01E0_4AC0);
    fx.mem.write_u32(D + 0xA0, D + 0xE4);
    fx.mem.write_u8(D + 0xD0, 0x20);
    fx.mem.write_u32(D + 0xA4, D + 0x7E4);
    fx.mem.write_u8(D + 0xD1, 0x32);
    fx.mem.write_u8(D + 0xD2, 0);
    fx.mem.write_u16(D + 0xD4, 0);
    fx.mem.write_u32(D + 0xD8, 0x42F0_0000);
    fx.mem.write_u32(D + 0xDC, 0x42F0_0000);
    fx.mem.write_u32(D + 0xE0, 0x40A0_0000);
    // The entry's s0 = the parameter, s1 = D, s2 = 1 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_409C, [Some(param as u32), Some(D), Some(1)]);
    0
}

/// rand() with the caller's s0 = `s0` (64 bits) from return address `ra`.
fn rand64(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand()
}

/// Nodes +0x14 / +0x18 / +0x1C into sp + 0xF0 / + 0xE0 / + 0x100, the
/// rotation from node +0x1C towards +0x14 (0x101A00 into sp + 0x120) and
/// the distance from node +0x18 (0x20C850 of sp + 0x110) [F 0x1E040D8 /
/// 0x1E0464C]; `aim_s0` the entry's s0 that 0x101A00 saves.
fn frame(fx: &mut Fx, sp: u32, s0: u64, aim_s0: u32, ras: [u32; 4]) -> f32 {
    for (k, (slot, out)) in [(0x14u32, 0xF0u32), (0x18, 0xE0), (0x1C, 0x100)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + slot);
        node_position_saving(fx, h, n, sp + out, s0, Some(u64::from(D)), ras[k]);
    }
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(sp + 0xF0 + 4 * c), fx.mem.f(sp + 0x100 + 4 * c));
        fx.mem.setf(sp + 0x110 + 4 * c, v);
    }
    blib::spray::aim_rotation(fx, sp + 0x110, sp + 0x120, [u64::from(aim_s0), u64::from(D)], ras[3]);
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(sp + 0xF0 + 4 * c), fx.mem.f(sp + 0xE0 + 4 * c));
        fx.mem.setf(sp + 0x110 + 4 * c, v);
    }
    blib::beam::length3(fx, sp + 0x110)
}

/// 0x20C540(out, sp + 0x120, v).
fn turn(fx: &mut Fx, sp: u32, out: u32, v: u32) {
    let m = fx.read_m4(sp + 0x120);
    let x = fx.read_v4(v);
    let r = fx.transform(Some(&m), x);
    fx.write_v4(out, r);
}

/// Update 124 [F 0x1E040D8]: two fans of 18 dashes from node +0x14, the
/// shake, a flash there and four bead beams. Returns the entry's s0
/// after.
fn strike(fx: &mut Fx, sp: u32, s0: u64) -> u64 {
    let len = frame(fx, sp, s0, sp + 0x114, [0x01E0_40EC, 0x01E0_4100, 0x01E0_4114, 0x01E0_4164]);
    let inner = ee::sub(len, f32::from_bits(0x3ECC_CCCD));
    let outer = ee::add(5.0, inner);
    for f in 0..2u32 {
        for k in 0..0x12u32 {
            let a = ee::div(ee::mul(f32::from_bits(PI), ee::mul(10.0, ee::from_i32(k as i32))), 180.0);
            let s = fx.libm_sinf(a);
            fx.mem.setf(sp + 0xA4, ee::mul(outer, s));
            let c = libm_cosf(fx, a);
            fx.mem.setf(sp + 0xA0, ee::mul(outer, c));
            fx.mem.write_u32(sp + 0xA8, if f != 0 { 0xC000_0000 } else { 0x4000_0000 });
            let s = fx.libm_sinf(a);
            fx.mem.setf(sp + 0xB4, ee::mul(inner, s));
            let c = libm_cosf(fx, a);
            fx.mem.setf(sp + 0xB0, ee::mul(inner, c));
            fx.mem.write_u32(sp + 0xB8, 0);
            turn(fx, sp, sp + 0xC0, sp + 0xA0);
            turn(fx, sp, sp + 0xD0, sp + 0xB0);
            let r = DASHES + 0x50 * (k + 0x12 * f);
            set_counter4(fx, r, 0x10, 0, 0xFF);
            fx.mem.write_u8(r + 4, 1);
            fx.mem.write_u8(r + 5, 0);
            fx.mem.write_u32(r + 8, 0x42A0_0000);
            for (o, v) in [(0xEu32, 0x80u8), (0xD, 0x40), (0xC, 0x20), (0xF, 0xC0)] {
                fx.mem.write_u8(r + o, v);
            }
            for c in 0..3u32 {
                let v = ee::add(fx.mem.f(sp + 0xD0 + 4 * c), fx.mem.f(sp + 0xF0 + 4 * c));
                fx.mem.setf(r + 0x10 + 4 * c, v);
            }
            let d = f32::from_bits(0x4060_0000);
            let v = ee::div(ee::sub(fx.mem.f(sp + 0xC0), fx.mem.f(sp + 0xD0)), d);
            fx.mem.setf(r + 0x20, v);
            for c in [4u32, 8] {
                let v = ee::div(ee::add(fx.mem.f(sp + 0xC0 + c), fx.mem.f(sp + 0xD0 + c)), d);
                fx.mem.setf(r + 0x20 + c, v);
            }
        }
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x10, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x4080_0000);
    copy_v4(fx, sp + 0xF0, D + 0x60);
    let r = D + 0xFB4;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x43FA_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xAu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (8, 0x20), (9, 0x20), (0xF, 0x60)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, sp + 0xF0, r + 0x1C);
    let mut s0 = u64::from(sp + 0xD0);
    for k in 0..4i32 {
        let count = (((rand64(fx, s0, 0x01E0_44E8) >> 4) & 7) + 0x14) & 0xFF;
        s0 = count as u64;
        let delay = (rand64(fx, s0, 0x01E0_4500) >> 4) & 3;
        let b = D + 0x1E58 + 0x58 * k as u32;
        set_counter4(fx, b, count, delay, 0xFF);
        fx.mem.setf(b + 0x1C, ee::add(8.0, ee::from_i32(3 * k)));
        let v = ee::sub(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(k)));
        fx.mem.setf(b + 0x14, v);
        for (o, v) in [(0xAu32, 0u8), (8, 0x1E), (0x26, 0xC0), (0x25, 0x30), (0x24, 0x10), (0x27, 0xF0), (4, 1), (5, 0)] {
            fx.mem.write_u8(b + o, v);
        }
        let x = rand64(fx, s0, 0x01E0_45D4);
        fx.mem.set8(b + 9, x >> 4);
        fx.mem.write_u32(b + 0x10, BYTES);
        copy_v4(fx, sp + 0xF0, b + 0x28);
    }
    s0
}

/// Updates 114..123 [F 0x1E0464C]: the eleven trail heads on a half
/// circle about node +0x14 (started on 114).
fn swing(fx: &mut Fx, sp: u32, s0: u64) {
    let len = frame(fx, sp, s0, sp + 0xF4, [0x01E0_4660, 0x01E0_4674, 0x01E0_4688, 0x01E0_46D8]);
    let r = ee::add(1.0, len);
    let mut deg = -20.0f32;
    for k in 0..0xBu32 {
        deg = ee::add(deg, 20.0);
        let a = ee::div(ee::mul(f32::from_bits(PI), deg), 180.0);
        let s = fx.libm_sinf(a);
        fx.mem.setf(sp + 0xA4, ee::mul(r, s));
        let c = libm_cosf(fx, a);
        fx.mem.setf(sp + 0xA0, ee::mul(r, c));
        fx.mem.write_u32(sp + 0xA8, 0);
        turn(fx, sp, sp + 0xC0, sp + 0xA0);
        let t = TRAILS + 0x48 * k;
        if fx.mem.u16(D) == 0x72 {
            set_counter4(fx, t, 0xE, 0, 0xFF);
            fx.mem.write_u8(t + 4, 1);
            fx.mem.write_u8(t + 5, 0);
            fx.mem.write_u32(t + 8, D + 0x1FB8);
            fx.mem.write_u8(t + 0x1E, 0x5A);
            fx.mem.write_u8(t + 0x1F, 8);
            fx.mem.write_u32(t + 0x20, 0x4100_0000);
            for (o, v) in [(0x26u32, 0x80u8), (0x25, 0x40), (0x24, 0x20), (0x27, 0xC0)] {
                fx.mem.write_u8(t + o, v);
            }
            let mut j = 0u32;
            while j < u32::from(fx.mem.read_u8(t + 0x1F)) + 1 {
                fx.mem.set16(t + 0xC + 2 * j, -1);
                j += 1;
            }
        }
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0xC0 + 4 * c), fx.mem.f(sp + 0xF0 + 4 * c));
            fx.mem.setf(t + 0x28 + 4 * c, v);
        }
    }
}

/// 0x1A2C70 of `v` (its residue left) into the u16 at `a`.
fn fix16(fx: &mut Fx, a: u32, v: f32, ra: u32) {
    fx.fix_residue(v, ra);
    fx.mem.write_u16(a, ee::fixunssfsi(v) as u16);
}

/// The record's 0x1E03510 [F] (fdb725dc1b7e, b104 only): `trails(recs,
/// ot, n) -> 1` steps and draws `n` trail records (0x48 bytes; a 0x3B0
/// frame, the descriptor at sp + 0x350: RGBAQ / XYZ2 x 4, ALPHA 0x48).
/// The point pool is the first record's +8 (0x10-byte points: count, xyz).
/// Per record: countdown 0x101AF0; the camera (0x20E2B0 into sp + 0xD0);
/// unless flag 1 the head +0x28 += +0x38 and from a node with +5
/// (0x101BA0); the history (+0xC, i16 x (+0x1F + 1)) shifted, each live
/// point's count set to the record's, the oldest freed; a new point (first
/// free of +0x1E, 0x101B40) at the head; the live points resampled
/// (0x102550 on sp + 0x110) or the record is skipped; colour +0x26.. with
/// alpha +0x27 (x count / 8 over the last 8) fading along the 4 (k - 1)
/// samples; each segment drawn as four screen-space quads (above / below,
/// left / right of the line, half width +0x20 / (w / 256)), key 0.
fn trails(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x3B0);
    let pool = fx.mem.read_u32(recs + 8);
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x48;
        }
        fx.mem.set32(sp + 0xB0, i);
        trail(fx, sp, g, pool, ot);
        i += 1;
    }
    fx.mem.set32(sp + 0xB0, i);
    fx.leave(0x3B0);
    1
}

fn trail(fx: &mut Fx, sp: u32, g: u32, pool: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    camera_vp_into(fx, sp + 0xD0);
    if fx.mem.u8(g + 4) & 1 == 0 {
        for o in [0x28u32, 0x2C, 0x30] {
            let v = ee::add(fx.mem.f(g + o), fx.mem.f(g + o + 0x10));
            fx.mem.setf(g + o, v);
        }
        node_source(fx, g, g + 0x28);
    }
    let len = fx.mem.u8(g + 0x1F) as u32;
    let mut k = len;
    while k > 0 {
        let prev = fx.mem.i16(g + 0xA + 2 * k);
        if prev >= 0 {
            let c = fx.mem.read_u8(g);
            fx.mem.write_u8(pool + 16 * prev as u32, c);
        }
        let v = fx.mem.read_u16(g + 0xA + 2 * k);
        fx.mem.write_u16(g + 0xC + 2 * k, v);
        k -= 1;
    }
    let last = g + 0xC + 2 * len;
    let old = fx.mem.i16(last);
    if old >= 0 {
        fx.mem.write_u8(pool + 16 * old as u32, 0);
        fx.mem.set16(last, -1);
    }
    if fx.mem.read_u8(g) == 0 {
        return;
    }
    let max = fx.mem.u8(g + 0x1E);
    let i = first_free(fx, pool, 0x10, max);
    if i >= 0 {
        fx.mem.set16(g + 0xC, i);
        let p = pool + 16 * fx.mem.i16(g + 0xC) as u32;
        let c = fx.mem.u8(g);
        set_counter(fx, p, c, 0);
        for c in 0..3u32 {
            let v = fx.mem.read_u32(g + 0x28 + 4 * c);
            fx.mem.write_u32(p + 4 + 4 * c, v);
        }
    } else {
        fx.mem.set16(g + 0xC, -1);
    }
    fx.mem.write_u32(sp + 0x110, 0);
    for k in 0..len {
        let h = fx.mem.i16(g + 0xC + 2 * k);
        if h < 0 {
            continue;
        }
        let p = pool + 16 * h as u32;
        let m = fx.mem.read_u32(sp + 0x110);
        for (c, o) in [(4u32, 0x114u32), (8, 0x134), (0xC, 0x154)] {
            let v = fx.mem.read_u32(p + c);
            fx.mem.write_u32(sp + o + 4 * m, v);
        }
        fx.mem.write_u32(sp + 0x110, m + 1);
    }
    if blib::arc::resample(fx, sp + 0x110) == 0 {
        return;
    }
    let steps = (fx.mem.i32(sp + 0x110) - 1) << 2;
    let d = sp + 0x350;
    fx.desc_init(d, 8, 0x14, 0x4141_4141);
    for (src, k) in [(0x26u32, 0u32), (0x25, 1), (0x24, 2)] {
        let v = fx.mem.read_u8(g + src);
        for o in [0x390u32, 0x380, 0x370, 0x360] {
            fx.mem.write_u8(sp + o + k, v);
        }
    }
    for o in [0x394u32, 0x384, 0x374, 0x364] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    for o in [0x39Fu32, 0x38F, 0x37F, 0x36F] {
        fx.mem.write_u8(sp + o, 0);
    }
    fx.desc_alpha(d, 0x48);
    fx.mem.write_u8(sp + 0x373, 0);
    fx.mem.write_u8(sp + 0x363, 0);
    let c = fx.mem.u8(g);
    let a = ee::from_i32(fx.mem.u8(g + 0x27));
    let alpha = if c < 8 { ee::div(ee::mul(ee::from_i32(c), a), 8.0) } else { a };
    let fade = ee::div(alpha, ee::from_i32(steps));
    for s in 0..steps - 1 {
        let m = fx.read_m4(sp + 0xD0);
        let p = sp + 0x174 + 16 * s as u32;
        if !fx.project_at(p, &m, sp + 0x340, sp + 0x3A8) || !Fx::on_screen(fx.mem.read_u64(sp + 0x340)) {
            continue;
        }
        let m = fx.read_m4(sp + 0xD0);
        if !fx.project_at(p + 16, &m, sp + 0x348, sp + 0x3AC) || !Fx::on_screen(fx.mem.read_u64(sp + 0x348)) {
            continue;
        }
        let w = fx.mem.f(g + 0x20);
        let h0 = ee::div(w, ee::div(fx.mem.f(sp + 0x3A8), 256.0));
        let h1 = ee::div(w, ee::div(fx.mem.f(sp + 0x3AC), 256.0));
        let left = steps - s;
        let v = ee::mul(fade, ee::from_i32(left));
        fx.fix_residue(v, 0x01E0_3A00);
        fx.mem.write_u8(sp + 0x383, ee::fixunssfsi(v) as u8);
        let v = ee::mul(fade, ee::from_i32(left - 1));
        fx.fix_residue(v, 0x01E0_3A24);
        fx.mem.write_u8(sp + 0x393, ee::fixunssfsi(v) as u8);
        if s == 0 {
            fx.mem.write_u8(sp + 0x383, 0);
        }
        for (dst, src) in [(0x388u32, 0x340u32), (0x368, 0x388), (0x398, 0x348), (0x378, 0x398)] {
            let z = fx.xyz_z(sp + src);
            fx.set_xyz_z(sp + dst, z);
        }
        let (x0, y0, x1, y1) = (fx.mem.u16(sp + 0x340), fx.mem.u16(sp + 0x342), fx.mem.u16(sp + 0x348), fx.mem.u16(sp + 0x34A));
        fx.mem.set16(sp + 0x38A, y0);
        fx.mem.set16(sp + 0x39A, y1);
        let (fx0, fy0, fx1, fy1) = (ee::from_i32(x0), ee::from_i32(y0), ee::from_i32(x1), ee::from_i32(y1));
        fix16(fx, sp + 0x36A, ee::sub(fy0, h0), 0x01E0_3B68);
        fix16(fx, sp + 0x37A, ee::sub(fy1, h1), 0x01E0_3BB4);
        for (o, v) in [(0x388u32, x0), (0x368, x0), (0x398, x1), (0x378, x1)] {
            fx.mem.set16(sp + o, v);
        }
        fx.submit(d, ot, 0);
        fix16(fx, sp + 0x36A, ee::add(fy0, h0), 0x01E0_3C40);
        fix16(fx, sp + 0x37A, ee::add(fy1, h1), 0x01E0_3C94);
        fx.submit(d, ot, 0);
        fix16(fx, sp + 0x368, ee::sub(fx0, h0), 0x01E0_3CF8);
        fix16(fx, sp + 0x378, ee::sub(fx1, h1), 0x01E0_3D48);
        for (o, v) in [(0x388u32, x0), (0x398, x1), (0x38A, y0), (0x36A, y0), (0x39A, y1), (0x37A, y1)] {
            fx.mem.set16(sp + o, v);
        }
        fx.submit(d, ot, 0);
        fix16(fx, sp + 0x368, ee::add(fx0, h0), 0x01E0_3DE8);
        fix16(fx, sp + 0x378, ee::add(fx1, h1), 0x01E0_3E38);
        fx.submit(d, ot, 0);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut s0 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0xCC => return 8,
        0x7C => s0 = strike(fx, sp, s0),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x72..0x7C).contains(&n) {
        swing(fx, sp, s0);
        s0 = u64::from(sp + 0xF4);
    }
    shake_saving(fx, D + 0x40, s0, 0x01E0_48E4, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA0, n);
    blib::mote::motes(fx, D + 0xE4, OT, 0x20);
    blib::beam::beads(fx, 0x01E0_15F0, D + 0x1E58, OT, 4);
    blib::star::flash(fx, 0x01E0_23C0, D + 0xFB4, OT, 1);
    trails(fx, TRAILS, OT, 0xB);
    blib::dash::dashes(fx, 0x01E0_2F70, DASHES, OT, 0x24);
    match fx.mem.u16(D) {
        0x79 => 2,
        0x72 => 1,
        _ => 0,
    }
}
