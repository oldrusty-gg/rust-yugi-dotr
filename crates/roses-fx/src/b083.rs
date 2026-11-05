//! BMODULE b083 (card 299), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05540, data area a0 = 0x1ED0000) [F] (helper C14,
//! 31 Oct 2026): b042's program shape (s3 = D, s0 = the OT, s1 the
//! parameter then loop counters, f20 kept). An arc (+0xA0, `arc::arcs`)
//! between the attacker's nodes +0x10 / +0x14 on 75; a bead-trail emitter
//! (+0x1E80, beads +0x300) at node +0x18 on 85; the strike at the target
//! on 86: the attacker hidden (0x1F7940, 0), a shake (+0x40), a flash
//! (+0x231C) and thirteen bars (+0x1F0C, `cross::bars`) fanned 15 degrees
//! apart from a random angle; on 180 a one-update fade and nine
//! screen-space blades (+0x2370, this record's 0x1E03F70: textured
//! crosses spinning about a screen point, the first centred) fanned 22.5
//! degrees apart; on 184 a fade (16, shape 2) and two frame-copy windows
//! (+0x25B0, 0x1E04DC0: the frame copied to TBP 0x2000 in a packet layer
//! and pasted back as a quad whose texels slide by +0x48 / +0x4C x 12 an
//! update); on 200 the attacker shown again (0x1F7940, 1) and a fade (16,
//! shape 4); on 216 a fade (32, shape 1); a full-window tint (0x2079D0,
//! grey 0x80 alpha 0x40, ALPHA preset 0) on 32..66. Returns 1 on 77, 2 on
//! 200, 8 on 260 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `beads`
//! 0x1E00410, `arcs` 0x1E011E0, `bars` 0x1E02460, `flash` 0x1E02E70,
//! `bead_trails` 0x1E03A20, `fade` 0x1E04880 [`crate::blib`]; the two
//! record-local functions are [`blades`] and [`windows`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{countdown, set_counter4, tex_rect_strip}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The blades' texture spec (24 bytes of record rodata: texture (4, 0xF),
/// blend 2, UV rectangle 0xD0..0xF0 x 0x20..0x40).
const BLADE_SPEC: u32 = 0x01E0_6230;
const PI: f32 = f32::from_bits(0x4049_0FDB);
/// The far Z merged into the vertices (bits 32..55 of XYZ2).
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;

/// The entry 0x1E05540 (a0 = D, a1 = param; s3 = D, s0 = the OT, s1 =
/// param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x2760);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    // `for s0 = 0; s0 < 0` (the own-texture loop) never runs.
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x1F, 0), (0x14, 0x1D, 0), (0x18, 0x20, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The entry's s0 = 0 (the loop's), s1 = param, s2 the runner's 0.
    rand_bytes(fx, D + 0x2660, 0x100, 0x01E0_5678, [Some(0), Some(param as u32), None]);
    0
}

/// rand() >> 4 % m - m / 2 as a float / 100 (the entry's s0 = the OT).
fn rand_angle(fx: &mut Fx, m: i32, ra: u32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let v = fx.div(x, m).1 - m / 2;
    ee::div(ee::from_i32(v), 100.0)
}

/// Update 75 [F 0x1E0570C]: the arc (+0xA0) from node +0x10 to +0x14,
/// points at +0x100.
fn arc_start(fx: &mut Fx) {
    let g = D + 0xA0;
    set_counter4(fx, g, 0x16, 0, 0xFF);
    fx.mem.write_u8(g + 4, 0xB);
    fx.mem.write_u32(g + 8, D + 0x100);
    fx.mem.write_u8(g + 0x30, 0x20);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F99_999A);
    fx.mem.write_u32(g + 0x3C, 0x4080_0000);
    fx.mem.write_u32(g + 0x40, 0x4220_0000);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0x80), (0x4A, 0x80), (0x48, 0x10), (0x49, 0x10), (0x4E, 0x80), (0x4C, 0x10), (0x4D, 0x10), (0x4F, 0xA0)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut k = 0u32;
    while k < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0xC + 2 * k, -1);
        fx.mem.set16(g + 0x1E + 2 * k, -1);
        k += 1;
    }
}

/// Update 85 [F 0x1E057EC]: the bead-trail emitter (+0x1E80) at node
/// +0x18, beads +0x300, random bytes +0x2660.
fn emitter_start(fx: &mut Fx, param: i32) {
    let e = D + 0x1E80;
    set_counter4(fx, e, 5, 0, 0xFF);
    fx.mem.write_u8(e + 5, 1);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(e + 6, b);
    fx.mem.write_u32(e + 8, D + 0x300);
    for (o, v) in [(0x10u32, 0x50u8), (0x12, 0x1E), (0x11, 0x18), (0x13, 0)] {
        fx.mem.write_u8(e + o, v);
    }
    fx.mem.write_u32(e + 0x18, 0x3F99_999A);
    fx.mem.write_u32(e + 0x1C, 0x3DCC_CCCD);
    fx.mem.write_u32(e + 0x20, 0x41A0_0000);
    fx.mem.write_u32(e + 0xC, D + 0x2660);
    for (o, v) in [(0x2Au32, 0x80u8), (0x28, 0x10), (0x29, 0x10), (0x2B, 0xA0)] {
        fx.mem.write_u8(e + o, v);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x18);
    node_position_saving(fx, h, n, e + 0x3C, u64::from(OT), Some(i64::from(param) as u64), 0x01E0_5890);
    copy_v4(fx, e + 0x3C, e + 0x4C);
    fx.mem.write_u8(e + 0x15, 1);
    for o in [0x74u32, 0x70, 0x6C, 0x84, 0x80, 0x7C] {
        fx.mem.write_u32(e + o, 0);
    }
}

/// Update 86 [F 0x1E058D8]: the attacker hidden, the shake (+0x40), the
/// flash (+0x231C) and thirteen bars (+0x1F0C) at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
    let t = fx.mem.i32(D + 8);
    fx.unit_show(t, 0, 0x01E0_58E8);
    let s = D + 0x40;
    set_counter4(fx, s, 0x14, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
    let r = D + 0x231C;
    set_counter4(fx, r, 0x12, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x43AF_0000);
    for (o, v) in [(0xAu32, 0x80u8), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (8, 0x10), (9, 0x10), (0xF, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, at, r + 0x1C);
    let f20 = rand_angle(fx, 0x9C40, 0x01E0_59E0);
    for k in 0..0xDi32 {
        let r = D + 0x1F0C + 0x50 * k as u32;
        fx.mem.write_u8(r + 5, 0);
        copy_v4(fx, at, r + 0x20);
        if k == 0 {
            set_counter4(fx, r, 0x12, 0, 0xFF);
            fx.mem.write_u8(r + 4, 0x81);
            fx.mem.write_u32(r + 0x10, 0x452F_0000);
            fx.mem.write_u32(r + 0x14, 0x41F0_0000);
            for (o, v) in [(0xDu32, 0x40u8), (0xE, 0x40), (0xC, 0x80), (9, 0x10), (0xA, 0x10), (8, 0x40), (0xF, 0x60)] {
                fx.mem.write_u8(r + o, v);
            }
            fx.mem.write_u32(r + 0x1C, 0);
            fx.mem.write_u32(r + 0x18, 0);
        } else {
            set_counter4(fx, r, 0x10, 0, 0xFF);
            fx.mem.write_u8(r + 4, 0x80);
            let a = rand_angle(fx, 0x9C40, 0x01E0_5B08);
            fx.mem.setf(r + 0x10, ee::add(f32::from_bits(0x4416_0000), a));
            fx.mem.write_u32(r + 0x14, 0x41A0_0000);
            for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xA, 0x40), (9, 0x10), (8, 0x10), (0xF, 0x40)] {
                fx.mem.write_u8(r + o, v);
            }
            fx.mem.setf(r + 0x18, ee::add(f20, ee::mul(15.0, ee::from_i32(k))));
            fx.mem.write_u32(r + 0x1C, 0x4000_0000);
        }
    }
}

/// Update 180 [F 0x1E05BCC]: a one-update fade (shape 4) and the nine
/// blades (+0x2370, 0x40 bytes each) at a random angle, 22.5 degrees
/// apart, the first centred and still.
fn blades_start(fx: &mut Fx) {
    set_counter4(fx, D + 0x2368, 1, 0, 0xFF);
    fx.mem.write_u8(D + 0x236C, 4);
    let f20 = rand_angle(fx, 0x9C40, 0x01E0_5BF4);
    for k in 0..9i32 {
        let r = D + 0x2370 + ((k as u32) << 6);
        set_counter4(fx, r, 0x12, 0, 0xFF);
        fx.mem.write_u32(r + 0x24, 0x4700_0000);
        fx.mem.write_u32(r + 0x20, 0x4700_0000);
        for (o, v) in [(0xAu32, 0x40u8), (9, 0x10), (8, 0x10), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0xC0)] {
            fx.mem.write_u8(r + o, v);
        }
        if k == 0 {
            fx.mem.write_u8(r + 4, 1);
            fx.mem.write_u32(r + 0x10, 0x45C8_0000);
            fx.mem.write_u32(r + 0x14, 0x4300_0000);
            fx.mem.write_u32(r + 0x18, 0x431A_0000);
            fx.mem.write_u32(r + 0x1C, 0);
        } else {
            let a = rand_angle(fx, 0x2710, 0x01E0_5CD8);
            let a = ee::mul(16.0, a);
            fx.mem.setf(r + 0x10, ee::add(f32::from_bits(0x44C8_0000), a));
            fx.mem.write_u32(r + 0x14, 0x4280_0000);
            fx.mem.setf(r + 0x18, ee::add(f20, ee::mul(f32::from_bits(0x41B4_0000), ee::from_i32(k))));
            fx.mem.write_u32(r + 0x1C, 0x3F80_0000);
        }
    }
}

/// Update 184 [F 0x1E05D84]: the two windows (+0x25B0, 0x58 bytes each:
/// the screen quad +8.., its texels +0x28.. sliding by +0x48 / +0x4C x
/// +0x50 an update) and the fade (16, shape 2).
fn windows_start(fx: &mut Fx) {
    for k in 0..2u32 {
        let r = D + 0x25B0 + 0x58 * k;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        fx.mem.write_u32(r + 0x30, 0);
        fx.mem.write_u32(r + 0x28, 0);
        fx.mem.write_u32(r + 0x34, 0x200);
        fx.mem.write_u32(r + 0x2C, 0x200);
        fx.mem.write_u16(r + 0x18, 0x7000);
        fx.mem.write_u16(r + 8, 0x7000);
        fx.mem.write_u16(r + 0x20, 0x9000);
        fx.mem.write_u16(r + 0x10, 0x9000);
        fx.mem.write_u32(r + 0x50, 0x4140_0000);
        if k != 0 {
            fx.mem.write_u32(r + 0x3C, 0);
            fx.mem.write_u32(r + 0x38, 0);
            fx.mem.write_u32(r + 0x40, 0x180);
            fx.mem.write_u32(r + 0x44, 0x80);
            fx.mem.write_u16(r + 0x12, 0x7000);
            fx.mem.write_u16(r + 0xA, 0x7000);
            fx.mem.write_u16(r + 0x1A, 0x8800);
            fx.mem.write_u16(r + 0x22, 0x7800);
            fx.mem.write_u32(r + 0x48, 0xC100_0000);
            fx.mem.write_u32(r + 0x4C, 0x4033_3333);
        } else {
            fx.mem.write_u32(r + 0x38, 0x180);
            fx.mem.write_u32(r + 0x3C, 0x80);
            fx.mem.write_u32(r + 0x44, 0x200);
            fx.mem.write_u32(r + 0x40, 0x200);
            fx.mem.write_u16(r + 0xA, 0x8800);
            fx.mem.write_u16(r + 0x12, 0x7800);
            fx.mem.write_u16(r + 0x22, 0x9000);
            fx.mem.write_u16(r + 0x1A, 0x9000);
            fx.mem.write_u32(r + 0x48, 0x4100_0000);
            fx.mem.write_u32(r + 0x4C, 0xC033_3333);
        }
    }
    set_counter4(fx, D + 0x2368, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0x236C, 2);
}

/// The soft-float conversion of a u16 (`lhu` then `cvt.s.w`: never the
/// unsigned path).
fn cvt_u16(fx: &mut Fx, a: u32) -> f32 {
    ee::from_i32(fx.mem.u16(a))
}

/// A 0x1A2C70 conversion stored as a u16 at `at`, its frame residue left.
fn fix_to(fx: &mut Fx, v: f32, at: u32, ra: u32) {
    fx.fix_residue(v, ra);
    let x = ee::fixunssfsi(v);
    fx.mem.write_u16(at, x as u16);
}

/// The far Z `z` merged into the XYZ2 qword at `a` (bits 32..55; the rest
/// kept).
fn merge_z(fx: &mut Fx, a: u32, z: u64) {
    let v = fx.mem.read_u64(a);
    fx.mem.write_u64(a, (v & KEEP) | (z << 32));
}

/// 0x1E03F70 (b80482df79a8, this record's own) [F]: `blades(recs, ot, n)
/// -> 1`: a 0x180 frame; the texture (4, 0xF) of the spec copied to sp +
/// 0xD0 (`BLADE_SPEC`); for each live record (0x40 bytes, 0x101AF0): the
/// screen point +0x20 / +0x24 moved by +0x30 / +0x34 (and +0x28 by
/// +0x38), the angle +0x18 turned by +0x1C; a strip of 4 vertices (sp +
/// 0xF0: prim 0x34, RGBAQ / UV / XYZ2, the UVs and blend from the spec
/// (0x101EE0), vertex 0 black, 1 / 2 the colour +8.., 3 the colour +0xC..,
/// alpha +0xF or (+0xF >> 3) x count while the count is below 8 without
/// flag 0x40, Z 0xFFFFFE); its length +0x10 scaled by (length - count) / 8
/// below 8 (without flags 0x40 / 1) and by count / 8 below 8 (without flag
/// 0x40); for k = 0, 1 the direction (cos, sin) of +0x18 + 180 k degrees
/// (libm cosf / sinf): a bar of width +0x14 from the point along it,
/// submitted twice (the two halves of the width), laid out by the steeper
/// axis.
fn blades(fx: &mut Fx, recs: u32, ot: u32, n: i32, s: [u64; 2]) -> i32 {
    let sp = fx.enter(0x180);
    // The entry's registers: ra, fp / s7..s4 the runner's 0, s3 = D, s2 /
    // s1 the entry's, s0 = the OT.
    fx.saved(sp, &[(0xA0, 0x01E0_5FBC), (0x90, 0), (0x80, 0), (0x70, 0), (0x60, 0), (0x50, 0), (0x40, u64::from(D)), (0x30, s[1]), (0x20, s[0]), (0x10, u64::from(ot))]);
    for k in 0..6u32 {
        let w = fx.mem.read_u32(BLADE_SPEC + 4 * k);
        fx.mem.write_u32(sp + 0xD0 + 4 * k, w);
    }
    fx.mem.set32(sp + 0xCC, n);
    let kind = fx.mem.i32(sp + 0xD0);
    let id = fx.mem.u8(sp + 0xD4);
    let rh2 = fx.texture(kind, id);
    fx.mem.write_u32(sp + 0xC0, rh2);
    let d = sp + 0xF0;
    let mut i = 0;
    fx.mem.set32(sp + 0xB0, i);
    while i < n {
        let r = recs + 0x40 * i as u32;
        i += 1;
        fx.mem.set32(sp + 0xB0, i);
        if countdown(fx, r) == 0 {
            continue;
        }
        for (a, v) in [(0x20u32, 0x30u32), (0x24, 0x34), (0x28, 0x38), (0x18, 0x1C)] {
            let x = ee::add(fx.mem.f(r + a), fx.mem.f(r + v));
            fx.mem.setf(r + a, x);
        }
        fx.mem.write_u16(d + 2, 0x34);
        fx.mem.write_u16(d, 0xC);
        fx.mem.write_u64(d + 8, 0x4214_2142_1421);
        fx.mem.write_u32(d + 4, 0);
        for o in 0..3u32 {
            fx.mem.write_u8(sp + 0x100 + o, 0);
        }
        for (o, src) in [(0u32, 0xAu32), (1, 9), (2, 8)] {
            let v = fx.mem.read_u8(r + src);
            fx.mem.write_u8(sp + 0x130 + o, v);
            fx.mem.write_u8(sp + 0x118 + o, v);
        }
        for (o, src) in [(0u32, 0xEu32), (1, 0xD), (2, 0xC)] {
            let v = fx.mem.read_u8(r + src);
            fx.mem.write_u8(sp + 0x148 + o, v);
        }
        for o in [0x14Cu32, 0x134, 0x11C, 0x104] {
            fx.mem.write_u32(sp + o, 0x3F80_0000);
        }
        tex_rect_strip(fx, d, sp + 0xD0, rh2);
        let a = fx.mem.u8(r + 0xF);
        let count = fx.mem.u8(r);
        let flags = fx.mem.u8(r + 4);
        let alpha = if count < 8 && flags & 0x40 == 0 { ((a >> 3).wrapping_mul(count)) as u8 } else { a as u8 };
        for o in [0x14Bu32, 0x133, 0x11B] {
            fx.mem.write_u8(sp + o, alpha);
        }
        fx.mem.write_u8(sp + 0x103, 0);
        let x = fx.mem.f(r + 0x20);
        fix_to(fx, x, sp + 0xE8, 0x01E0_4150);
        let y = fx.mem.f(r + 0x24);
        fix_to(fx, y, sp + 0xEA, 0x01E0_4160);
        for o in [0xE8u32, 0x158, 0x140, 0x128, 0x110] {
            merge_z(fx, sp + o, 0xFF_FFFE);
        }
        for o in [0x15Fu32, 0x147, 0x12F, 0x117] {
            fx.mem.write_u8(sp + o, 1);
        }
        let size = fx.mem.f(r + 0x10);
        let mut f22 = size;
        if flags & 0x40 == 0 {
            if flags & 1 == 0 {
                let dd = (fx.mem.u8(r + 2) - count) & 0xFF;
                if dd < 8 {
                    f22 = ee::mul(size, ee::mul(0.125, ee::from_i32(dd)));
                }
            }
            if count < 8 {
                f22 = ee::mul(size, ee::mul(0.125, ee::from_i32(count)));
            }
        }
        let width = fx.mem.f(r + 0x14);
        for k in 0..2i32 {
            let f21 = ee::mul(180.0, ee::from_i32(k));
            let a = ee::add(fx.mem.f(r + 0x18), f21);
            let rad = ee::div(ee::mul(PI, a), 180.0);
            let c = blib::exe::libm_cosf(fx, rad);
            let f20 = ee::mul(f22, c);
            let a = ee::add(fx.mem.f(r + 0x18), f21);
            let rad = ee::div(ee::mul(PI, a), 180.0);
            let s = fx.libm_sinf(rad);
            let f21 = ee::mul(f22, s);
            if ee_le(f20.abs(), f21.abs()) {
                // The steeper direction: the bar along y.
                let y = cvt_u16(fx, sp + 0xEA);
                let yy = fx.mem.u16(sp + 0xEA);
                fix_to(fx, ee::add(y, f21), sp + 0x142, 0x01E0_4674);
                let v = fx.mem.read_u16(sp + 0x142);
                fx.mem.write_u16(sp + 0x112, v);
                fx.mem.set16(sp + 0x15A, yy);
                fx.mem.set16(sp + 0x12A, yy);
                let x = cvt_u16(fx, sp + 0xE8);
                let xx = fx.mem.u16(sp + 0xE8);
                let f21 = ee::add(x, f20);
                fix_to(fx, ee::sub(f21, width), sp + 0x110, 0x01E0_46D4);
                fix_to(fx, ee::sub(x, width), sp + 0x128, 0x01E0_4720);
                fix_to(fx, f21, sp + 0x140, 0x01E0_4730);
                fx.mem.set16(sp + 0x158, xx);
                fx.submit(d, ot, 0);
                let x = cvt_u16(fx, sp + 0xE8);
                fix_to(fx, ee::add(width, ee::add(x, f20)), sp + 0x110, 0x01E0_479C);
                fix_to(fx, ee::add(x, width), sp + 0x128, 0x01E0_47E8);
                fx.submit(d, ot, 0);
            } else {
                let x = cvt_u16(fx, sp + 0xE8);
                let xx = fx.mem.u16(sp + 0xE8);
                fix_to(fx, ee::add(x, f20), sp + 0x140, 0x01E0_4488);
                let v = fx.mem.read_u16(sp + 0x140);
                fx.mem.write_u16(sp + 0x110, v);
                fx.mem.set16(sp + 0x158, xx);
                fx.mem.set16(sp + 0x128, xx);
                let y = cvt_u16(fx, sp + 0xEA);
                let yy = fx.mem.u16(sp + 0xEA);
                let f20 = ee::add(y, f21);
                fix_to(fx, ee::sub(f20, width), sp + 0x112, 0x01E0_44E8);
                fix_to(fx, ee::sub(y, width), sp + 0x12A, 0x01E0_4538);
                fix_to(fx, f20, sp + 0x142, 0x01E0_454C);
                fx.mem.set16(sp + 0x15A, yy);
                fx.submit(d, ot, 0);
                let y = cvt_u16(fx, sp + 0xEA);
                fix_to(fx, ee::add(width, ee::add(y, f21)), sp + 0x112, 0x01E0_45B8);
                fix_to(fx, ee::add(y, width), sp + 0x12A, 0x01E0_4608);
                fx.submit(d, ot, 0);
            }
        }
    }
    fx.leave(0x180);
    1
}

/// 0x1E04DC0 (7af984a9d86c, this record's own) [F]: `windows(recs, ot, n)
/// -> 1`: a packet layer (0x2078A0); for each live record (0x58 bytes,
/// 0x101AF0) with age = +2 - +0: the first time a TEXFLUSH, the frame
/// copied to 0x2095A0's TBP for the quad's TEX0 / TEX1 (0x207F50, point
/// sampled, at sp + 0xF8 / + 0x100: the TEX1's untouched bits are the
/// stale stack word) and another TEXFLUSH; with +4 & 3 a full-window
/// untextured sprite (0x7000..0x9000, grey 0x80, or black with +4 & 1,
/// Z 0xFFFFFF, prim 4, unlinked); the quad (sp + 0x80: prim 0x34 | 0x800,
/// grey 0x80, Z 0xFFFFFF) with the four texels +0x28.. / +0x38.. (x 16, as
/// 14-bit UVs) at the four corners +8.. + age x (+0x48, +0x4C) x +0x50,
/// unlinked; the layer closed into `ot` at key 0 (0x207910) when a copy
/// was made. `a3`: the entry's a3 at the call (the first TEXFLUSH's).
fn windows(fx: &mut Fx, recs: u32, ot: u32, n: i32, s: [u64; 2], a3: u32) -> i32 {
    let sp = fx.enter(0x150);
    fx.saved(sp, &[(0x70, 0x01E0_5FE0), (0x60, 0), (0x50, 0), (0x40, u64::from(D)), (0x30, s[1]), (0x20, s[0]), (0x10, u64::from(ot))]);
    let layer = fx.layer_begin();
    let mut copied = false;
    let d = sp + 0x80;
    for i in 0..n {
        let r = recs + 0x58 * i as u32;
        if countdown(fx, r) == 0 {
            continue;
        }
        let age = fx.mem.u8(r + 2) - fx.mem.u8(r);
        let f3 = fx.mem.f(r + 0x50);
        let f2 = ee::mul(fx.mem.f(r + 0x48), f3);
        let f1 = ee::from_i32(age);
        let f20 = ee::mul(f1, f2);
        let f0 = ee::mul(fx.mem.f(r + 0x4C), f3);
        let f21 = ee::mul(f1, f0);
        if !copied {
            fx.mem.write_u16(d + 2, 0x34);
            fx.mem.write_u16(d, 0xC);
            fx.mem.write_u64(d + 8, 0x4214_2142_1421);
            fx.mem.write_u32(d + 4, 0);
            for v in [0x90u32, 0xA8, 0xC0, 0xD8] {
                for k in 0..4u32 {
                    fx.mem.write_u8(sp + v + k, 0x80);
                }
                fx.mem.write_u32(sp + v + 4, 0x3F80_0000);
            }
            // a0 / a1 = the stale qwords at sp + 0xA0 / + 0xB8 before the
            // far Z is merged in (fade's saved s3 = D and the upper half of
            // its s4 = 0), a2 = n.
            let a0 = fx.mem.read_u32(sp + 0xA0);
            let a1 = fx.mem.read_u32(sp + 0xB8);
            for o in [0xE8u32, 0xD0, 0xB8, 0xA0] {
                merge_z(fx, sp + o, 0xFF_FFFF);
            }
            for o in [0xEFu32, 0xD7, 0xBF, 0xA7] {
                fx.mem.write_u8(sp + o, 0);
            }
            let p0 = fx.mem.read_u32(crate::world::PACKET_POINTER);
            fx.texflush([a0, a1, n as u32, a3]);
            // a0..a3 as 0x207BB0 leaves them: the new packet pointer twice,
            // the old one, 0x8000.
            let p1 = fx.mem.read_u32(crate::world::PACKET_POINTER);
            let dest = fx.copy_dest([p1, p1, p0, 0x8000]);
            let regs = fx.frame_texture_packet(dest, 0, sp + 0xF8, sp + 0x100);
            fx.texflush(regs);
            copied = true;
        }
        let flags = fx.mem.u8(r + 4);
        if flags & 3 != 0 {
            let e = sp + 0x110;
            fx.mem.write_u16(e + 2, 4);
            fx.mem.write_u16(e, 5);
            fx.mem.write_u64(e + 8, 0x4_4441);
            fx.mem.write_u32(e + 4, 0);
            fx.mem.write_u8(sp + 0x123, 0x80);
            let c = if flags & 1 != 0 { 0 } else { 0x80 };
            for o in [0x122u32, 0x121, 0x120] {
                fx.mem.write_u8(sp + o, c);
            }
            fx.mem.write_u32(sp + 0x124, 0x3F80_0000);
            for (o, v) in [(0x138u32, 0x7000u16), (0x128, 0x7000), (0x140, 0x9000), (0x130, 0x9000), (0x132, 0x7000), (0x12A, 0x7000), (0x142, 0x9000), (0x13A, 0x9000)] {
                fx.mem.write_u16(sp + o, v);
            }
            for o in [0x140u32, 0x138, 0x130, 0x128] {
                merge_z(fx, sp + o, 0xFF_FFFF);
            }
            fx.submit(e, 0, 0);
        }
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        for (o, src) in [(0x98u32, 0x28u32), (0x9A, 0x38), (0xB0, 0x2C), (0xB2, 0x3C), (0xC8, 0x30), (0xCA, 0x40), (0xE0, 0x34), (0xE2, 0x44)] {
            let v = (fx.mem.i32(r + src) << 4) & 0x3FFF;
            let old = fx.mem.read_u16(sp + o);
            fx.mem.write_u16(sp + o, (old & 0xC000) | v as u16);
        }
        for (o, src, ra) in [(0xA0u32, 8u32, 0x01E0_52A4u32), (0xB8, 0x10, 0x01E0_533C), (0xD0, 0x18, 0x01E0_53D4), (0xE8, 0x20, 0x01E0_546C)] {
            let x = cvt_u16(fx, r + src);
            fix_to(fx, ee::add(x, f20), sp + o, ra);
            let y = cvt_u16(fx, r + src + 2);
            fix_to(fx, ee::add(y, f21), sp + o + 2, ra + 0x4C);
        }
        fx.submit(d, 0, 0);
    }
    if copied {
        fx.layer_end(layer, ot, 0);
    }
    fx.leave(0x150);
    1
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let ot = u64::from(OT);
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, ot, 0x01E0_56A0);
    // The entry's s1 (param, then the last loop's count) and s2 (the last
    // loop's record offset; the runner's 0 before).
    let mut s1 = i64::from(param) as u64;
    let mut s2 = 0u64;
    match fx.mem.u16(D) {
        0x104 => return 8,
        0xD8 => {
            set_counter4(fx, D + 0x2368, 0x20, 0, 0xFF);
            fx.mem.write_u8(D + 0x236C, 1);
        }
        0xC8 => {
            set_counter4(fx, D + 0x2368, 0x10, 0, 0xFF);
            fx.mem.write_u8(D + 0x236C, 4);
            let t = fx.mem.i32(D + 8);
            fx.unit_show(t, 1, 0x01E0_5EFC);
        }
        0xB8 => {
            windows_start(fx);
            s1 = 2;
            s2 = 0x58;
        }
        0xB4 => {
            blades_start(fx);
            s1 = 9;
            s2 = 8 << 6;
        }
        0x56 => {
            strike(fx, sp);
            s1 = 0xD;
            s2 = 0x50 * 0xC;
        }
        0x55 => emitter_start(fx, param),
        0x4B => {
            arc_start(fx);
            s1 = 9;
        }
        _ => {}
    }
    shake_saving(fx, D + 0x40, ot, 0x01E0_5F44, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::arc::arcs(fx, 0x01E0_11E0, D + 0xA0, OT, 1);
    blib::emit::bead_trails(fx, 0x01E0_3A20, D + 0x1E80, 1);
    blib::beam::beads(fx, 0x01E0_0410, D + 0x300, OT, 0x50);
    blib::cross::bars(fx, 0x01E0_2460, D + 0x1F0C, OT, 0xD);
    blib::star::flash(fx, 0x01E0_2E70, D + 0x231C, OT, 1);
    blades(fx, D + 0x2370, OT, 9, [s1, s2]);
    let drew = blib::fade::fade(fx, D + 0x2368, OT) != 0;
    // fade's 0xF0 frame: ra, s6 = the record, s5 = the OT, s4 = 0, s3 =
    // D, s2 / s1 the entry's, s0 = the OT (its s1 slot is the windows'
    // TEX1's stale bits, its s3 / s4 slots the first TEXFLUSH's a0 / a1).
    let f = fx.sp - 0xF0;
    fx.saved(f, &[(0x80, 0x01E0_5FCC), (0x70, u64::from(D + 0x2368)), (0x60, ot), (0x50, 0), (0x40, u64::from(D)), (0x30, s2), (0x20, s1), (0x10, ot)]);
    // The entry's a3 after fade: 0x207E20's leftover, fade's descriptor
    // flags 0x14 (no wipe in this record) [H: the a3 when fade is not
    // due (never while the windows live here) is not modelled].
    let a3 = if drew { 0x14 } else { 0x14 };
    windows(fx, D + 0x25B0, OT, 2, [s1, s2], a3);
    let n = fx.mem.u16(D);
    if (0x20..0x43).contains(&n) {
        for o in [0x7Cu32, 0x7D, 0x7E] {
            fx.mem.write_u8(sp + o, 0x80);
        }
        fx.mem.write_u8(sp + 0x7F, 0x40);
        let a = fx.alpha_preset(0);
        fx.frame_pass(OT, 0, 1, a, sp + 0x7C, 0x01E0_603C);
    }
    match fx.mem.u16(D) {
        0xC8 => 2,
        0x4D => 1,
        _ => 0,
    }
}
