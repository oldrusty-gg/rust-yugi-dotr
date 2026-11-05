//! BMODULE b458 (card 540), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E052C0, data area a0 = 0x1ED0000, 0xB6C0 bytes)
//! [F]: b268's family (s3 = D, s0 = the
//! parameter, fp = the OT, a 0x130 frame, the target sp + 0xB0). Update 2
//! three long flashes (+0x212C) on nodes 6 / 8 / 0xA and a long shake;
//! 0x28 six points round node 1 (60 degrees apart, radius 25 +- 2.5, 5
//! below node 0x13) into +0xB104, each the end of a five-point arc stroke
//! (+0x7A04, its points +0x8404 + 0x90 k, 0x102F20) from node 6 / 8 / 0xA,
//! twenty rising sprites (+0x414C) and a band (+0x766C) there, then the
//! fade; 0x5A a long shake at the target; 0x64 the target untinted with its
//! jitter (+0xB194, `radiate::jitter`); 0x102 tinted back. Every fourth
//! update 4..0xBC three ribbons (+0xAC) on the nodes; 0x2C..0xFC six
//! ribbons at the six points; 0x5C..0xFC at a point about the target a
//! band, a ribbon, ten sprites, an eleven-point arc stroke from the first
//! point, three bolts (+0x284C) and a flash. Returns 6 on 0x5A..0xFE, 2
//! on 0xFF, 1 on 0x50, 8 on 0x10E (no drawing then) and at once for -2 /
//! -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `fade`
//! 0x1E00410, `flash` 0x1E00950, `ribbon` 0x1E01500, `bolts` 0x1E02090,
//! `strokes_b` 0x1E02BE0, `bounce` 0x1E03830, `jitter` 0x1E042A0, `bands`
//! 0x1E04AD0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b173::arc_points;
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const FADE: u32 = D + 0xA4;
const RIBBONS: u32 = D + 0xAC;
const FLASH: u32 = D + 0x212C;
const BOLTS: u32 = D + 0x284C;
const SPRITES: u32 = D + 0x414C;
const BANDS: u32 = D + 0x766C;
const STROKES: u32 = D + 0x7A04;
const POINTS: u32 = D + 0x8404;
const RING: u32 = D + 0xB104;
const JITTER: u32 = D + 0xB194;
const BYTES: u32 = D + 0xB5C0;

/// The runner's s7 at the entry [H: 0, as its s1]: the target point's
/// sprites read the ring point s7 (the 0x28 loop's index) unchanged.
const RUNNER_S7: u32 = 0;

/// The entry 0x1E052C0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x130);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x130);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0xB6C0);
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
    for (at, node, clear) in [(0xCu32, 6, 1), (0x10, 8, 0), (0x14, 0xA, 0), (0x18, 1, 0), (0x1C, 0x13, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x108);
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5440, [Some(param as u32), None, None]);
    0
}

/// The entry's s0 / s1 (what rand / 0x1F6830 / the library calls save;
/// s1 the runner's 0 until a block sets it [run]).
struct Regs {
    s0: u64,
    s1: u64,
}

fn rand4(fx: &mut Fx, r: &Regs, ra: u32) -> i32 {
    fx.rand_from(r.s0 as u32, ra) >> 4
}

/// (rand >> 4) % m.
fn rand_rem(fx: &mut Fx, r: &Regs, ra: u32, m: i32) -> i32 {
    let v = rand4(fx, r, ra);
    fx.div(v, m).1
}

/// ((rand >> 4) % m - half) / 100.
fn jit(fx: &mut Fx, r: &Regs, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand_rem(fx, r, ra, m);
    ee::div(ee::from_i32(v - half), 100.0)
}

/// (200 x ((rand >> 4) % m) - `sub`) / 100.
fn jit200(fx: &mut Fx, r: &Regs, ra: u32, m: i32, sub: i32) -> f32 {
    let v = rand_rem(fx, r, ra, m);
    ee::div(ee::from_i32(v.wrapping_mul(200) - sub), 100.0)
}

fn node_pos(fx: &mut Fx, r: &Regs, s1: Option<u64>, slot: i32, out: u32, ra: u32) {
    let h = fx.mem.i32(D + 4);
    node_position_saving(fx, h, slot, out, r.s0, s1, ra);
}

fn bytes(fx: &mut Fx, p: u32, at: &[(u32, u8)]) {
    for &(o, v) in at {
        fx.mem.write_u8(p + o, v);
    }
}

fn copy_n(fx: &mut Fx, from: u32, to: u32, n: u32) {
    for k in 0..n {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// Update 2 [F 0x1E054BC]: three long flashes on nodes 6 / 8 / 0xA and a
/// long shake on node 6.
fn opening(fx: &mut Fx, r: &mut Regs) {
    for s5 in 0..3u32 {
        let f = FLASH + 0x4C * s5;
        r.s0 = u64::from(0x4C * s5);
        set_counter4(fx, f, 0xA0, 0, 0xFF);
        fx.mem.write_u8(f + 5, 1);
        let v = fx.mem.read_u8(D + 0xC + 4 * s5);
        fx.mem.write_u8(f + 6, v);
        bytes(fx, f, &[(4, 5), (7, 9), (0xA, 0x40), (9, 0x20), (8, 0x80), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0)]);
        fx.mem.write_u32(f + 0x10, 0x4348_0000);
        fx.mem.write_u32(f + 0x14, 0);
    }
    let s = SHAKE;
    set_counter4(fx, s, 0xFA, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, v);
    fx.mem.write_u8(s + 4, 4);
    fx.mem.write_u32(s + 0x14, 0x3F99_999A);
    fx.mem.write_u32(s + 0x18, 0x3F99_999A);
    fx.mem.write_u32(s + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x4000_0000);
}

/// An arc stroke (+0x7A04 record `i`, unchecked) of the arc 0x102F20 built
/// at sp + 0x100 (n points, radius, 270 degrees) from sp + 0xE0-ish `from`
/// to `to`, counted `count` with kind +4 `k4`, +0xD / +0xE, colour `c`.
#[allow(clippy::too_many_arguments)]
fn arc_stroke(fx: &mut Fx, sp: u32, i: i32, n: i32, radius: f32, from: [u32; 3], to: [u32; 3], st: (i32, u8, u32, [u8; 2], u8)) -> u32 {
    fx.mem.set32(sp + 0x100, n);
    fx.mem.setf(sp + 0x104, radius);
    fx.mem.write_u32(sp + 0x108, 0x4387_0000);
    let pts = POINTS.wrapping_add((0x90i32.wrapping_mul(i)) as u32);
    fx.mem.write_u32(sp + 0x128, pts);
    for (k, a) in from.iter().enumerate() {
        let w = fx.mem.read_u32(*a);
        fx.mem.write_u32(sp + 0x110 + 4 * k as u32, w);
    }
    for (k, a) in to.iter().enumerate() {
        let w = fx.mem.read_u32(*a);
        fx.mem.write_u32(sp + 0x11C + 4 * k as u32, w);
    }
    arc_points(fx, sp + 0x100);
    let (count, k4, w, k, c) = st;
    let g = STROKES.wrapping_add((i << 5) as u32);
    set_counter4(fx, g, count, 0, 0xFF);
    fx.mem.write_u8(g + 4, k4);
    let v = fx.mem.read_u8(sp + 0x100);
    fx.mem.write_u8(g + 8, v);
    fx.mem.write_u32(g + 0x14, w);
    bytes(fx, g, &[(0xA, 0), (9, 1), (0xB, 2), (0xC, 7), (0xD, k[0]), (0xE, k[1]), (0x18, c), (0x19, c), (0x1A, c), (0x1B, 0x20)]);
    fx.mem.write_u32(g + 0x1C, pts);
    pts
}

/// A band (+0x766C) counted `count` / `delay` at the 4 words `at` (only 3
/// when `three`).
fn band(fx: &mut Fx, count: i32, delay: i32, at: [u32; 4], three: bool) -> Option<u32> {
    let i = first_free(fx, BANDS, 0x5C, 0xA);
    if i < 0 {
        return None;
    }
    let b = BANDS + 0x5C * i as u32;
    set_counter4(fx, b, count, delay, 0xFF);
    bytes(fx, b, &[(5, 0), (7, 0x24), (0xC, 2), (8, 0), (9, 2), (0xA, 0), (0xB, 5)]);
    fx.mem.write_u32(b + 0x10, 0x4040_0000);
    fx.mem.write_u32(b + 0x14, 0x4020_0000);
    bytes(fx, b, &[(0x28, 0x80), (0x29, 0x80), (0x2A, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x2B, 0x80), (0x27, 0x80)]);
    fx.mem.write_u32(b + 0x18, 0x3F33_3333);
    fx.mem.write_u32(b + 0x1C, 0x3F19_999A);
    for (k, a) in at.iter().enumerate().take(if three { 3 } else { 4 }) {
        let w = fx.mem.read_u32(*a);
        fx.mem.write_u32(b + 0x2C + 4 * k as u32, w);
    }
    Some(0x5C * i as u32)
}

/// Update 0x28 [F 0x1E055D4]: the six ring points with their arc strokes,
/// sprites and bands, then the fade.
fn ring(fx: &mut Fx, sp: u32, r: &mut Regs) {
    let s = fx.mem.i32(D + 0x18);
    node_pos(fx, r, Some(r.s1), s, sp + 0xC0, 0x01E0_55E8);
    let s = fx.mem.i32(D + 0x1C);
    node_pos(fx, r, Some(r.s1), s, sp + 0xD0, 0x01E0_55FC);
    let mut s1: Option<u64> = Some(r.s1);
    for s7 in 0..6u32 {
        let i = first_free(fx, STROKES, 0x20, 0x50);
        let slot = fx.mem.i32(D + 0xC + 4 * (s7 % 3));
        node_pos(fx, r, s1, slot, sp + 0xE0, 0x01E0_5650);
        let f20 = ee::mul(60.0, ee::from_i32(s7 as i32));
        let f21 = ee::add(25.0, jit200(fx, r, 0x01E0_5670, 5, 0x1F4));
        let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), f20), 180.0);
        let p = RING + 12 * s7;
        let cs = libm_cosf(fx, a);
        let x = ee::add(fx.mem.f(sp + 0xC0), ee::mul(f21, cs));
        fx.mem.setf(sp + 0xF0, x);
        fx.mem.setf(p, x);
        s1 = Some(u64::from(p));
        let sn = fx.libm_sinf(a);
        let z = ee::add(fx.mem.f(sp + 0xC8), ee::mul(f21, sn));
        fx.mem.setf(sp + 0xF8, z);
        fx.mem.setf(p + 8, z);
        let y = ee::sub(fx.mem.f(sp + 0xD4), 5.0);
        fx.mem.setf(sp + 0xF4, y);
        fx.mem.setf(p + 4, y);
        r.s0 = u64::from(p + 4);
        fx.mem.set32(sp + 0x100, 5);
        let radius = ee::add(18.0, jit200(fx, r, 0x01E0_579C, 5, 0x1F4));
        arc_stroke(fx, sp, i, 5, radius, [sp + 0xE0, sp + 0xE4, sp + 0xE8], [sp + 0xF0, sp + 0xF4, sp + 0xF8], (0xFF, 2, 0x3DCC_CCCD, [2, 1], 0x80));
        for _ in 0..0x14 {
            let j = first_free(fx, SPRITES, 0x44, 0xC8);
            if j < 0 {
                continue;
            }
            let count = ((rand4(fx, r, 0x01E0_5918) & 0xF) + 0x20) & 0xFF;
            let delay = ((rand4(fx, r, 0x01E0_5930) & 0xF) + 4) & 0xFF;
            let q = SPRITES + 0x44 * j as u32;
            set_counter4(fx, q, count, delay, 0xFF);
            bytes(fx, q, &[(4, 0x42), (8, 0), (0xB, 2), (0xA, 0)]);
            let v = (rand4(fx, r, 0x01E0_5988) & 3) + 2;
            fx.mem.set8(q + 9, v);
            bytes(fx, q, &[(0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)]);
            fx.mem.write_u32(q + 0x10, 0x4220_0000);
            fx.mem.write_u32(q + 0x14, 0xBE4C_CCCD);
            fx.mem.write_u32(q + 0x18, 0xBD75_C28F);
            // +0x1C takes the point's y (odd original kept).
            for (o, a) in [(0x1Cu32, p + 4), (0x20, p), (0x24, p + 4), (0x28, p + 8)] {
                let w = fx.mem.read_u32(a);
                fx.mem.write_u32(q + o, w);
            }
            let v = jit(fx, r, 0x01E0_59F8, 0x5A, 0x2D);
            fx.mem.setf(q + 0x2C, v);
            let v = ee::add(f32::from_bits(0x3F99_999A), jit(fx, r, 0x01E0_5A44, 0x64, 0x32));
            fx.mem.setf(q + 0x30, v);
            let v = jit(fx, r, 0x01E0_5AA0, 0x5A, 0x2D);
            fx.mem.setf(q + 0x34, v);
        }
        band(fx, 0x18, 4, [p, p + 4, p + 8, 0], true);
    }
    r.s1 = s1.unwrap_or(0);
    set_counter4(fx, FADE, 8, 4, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
}

/// Update 0x5A [F 0x1E05C14]: a long shake at the target.
fn shake_target(fx: &mut Fx, sp: u32) {
    let s = SHAKE;
    set_counter4(fx, s, 0xF5, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_n(fx, sp + 0xB0, s + 0x20, 4);
}

/// Update 0x64 [F 0x1E05C94]: the target untinted and its jitter.
fn jitter_on(fx: &mut Fx, sp: u32, r: &Regs) {
    for k in 0..4 {
        fx.mem.write_u8(sp + 0x12C + k, 0);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_tint_mode(t, -1, sp + 0x12C, [r.s0, r.s1], 0x01E0_5CB4);
    let j = JITTER;
    set_counter4(fx, j, 0x9E, 0, 0);
    bytes(fx, j, &[(0xC, 1), (0xD, 0xCF), (0xE, 0x10), (0x12, 1), (0x10, 3), (0x11, 0)]);
    let v = fx.mem.read_u32(D + 8);
    fx.mem.write_u32(j + 0x14, v);
    bytes(fx, j, &[(0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x80)]);
}

/// A ribbon (+0xAC) counted `count`: +4 `k4`, +8 `k8`, sizes, flags.
fn ribbon(fx: &mut Fx, r: &mut Regs, count: i32, k: (u8, u8), size: [u32; 3], ra: u32) -> Option<u32> {
    let i = first_free(fx, RIBBONS, 0x68, 0x50);
    if i < 0 {
        return None;
    }
    let g = RIBBONS + 0x68 * i as u32;
    r.s0 = u64::from(0x68 * i as u32);
    set_counter4(fx, g, count, 0, 0xFF);
    fx.mem.write_u8(g + 4, k.0);
    fx.mem.write_u32(g + 0x14, size[0]);
    if size[1] != 0 {
        fx.mem.write_u32(g + 0x18, size[1]);
    }
    fx.mem.write_u32(g + 0x1C, size[2]);
    fx.mem.write_u8(g + 8, k.1);
    let _ = ra;
    Some(g)
}

/// Every fourth update 4..0xBC [F 0x1E05DD4]: three ribbons on nodes 6 /
/// 8 / 0xA.
fn node_ribbons(fx: &mut Fx, r: &mut Regs) {
    for s5 in 0..3u32 {
        let Some(g) = ribbon(fx, r, 0x10, (5, 0xC), [0x3FE6_6666, 0, 0x41A0_0000], 0) else { continue };
        fx.mem.write_u8(g + 5, 1);
        let v = fx.mem.read_u8(D + 0xC + 4 * s5);
        fx.mem.write_u8(g + 6, v);
        bytes(fx, g, &[(0xA, 0), (0xB, 2), (0xC, 3)]);
        let x = rand4(fx, r, 0x01E0_5E8C);
        fx.mem.set8(g + 9, x);
        fx.mem.write_u32(g + 0x10, BYTES);
        fx.mem.write_u32(g + 0x18, 0x3F80_0000);
        bytes(fx, g, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x30)]);
        for (o, ra) in [(0x58u32, 0x01E0_5EDCu32), (0x5C, 0x01E0_5EF4), (0x60, 0x01E0_5F0C)] {
            let v = rand4(fx, r, ra) & 0x7F;
            fx.mem.set32(g + o, v);
        }
    }
}

/// The still ribbons' common fields (count `count` set by the caller).
fn still_ribbon(fx: &mut Fx, r: &Regs, g: u32, ra: u32) {
    fx.mem.write_u8(g + 5, 0);
    bytes(fx, g, &[(0xA, 0), (0xB, 2), (0xC, 3)]);
    let x = rand4(fx, r, ra);
    fx.mem.set8(g + 9, x);
    fx.mem.write_u32(g + 0x10, BYTES);
    bytes(fx, g, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x40)]);
    for o in [0x50u32, 0x4C, 0x48, 0x60, 0x5C, 0x58] {
        fx.mem.write_u32(g + o, 0);
    }
}

/// Every fourth update 0x2C..0xFC [F 0x1E05F60]: a still ribbon at each
/// ring point.
fn ring_ribbons(fx: &mut Fx, r: &mut Regs) {
    for s5 in 0..6u32 {
        let Some(g) = ribbon(fx, r, 0xC, (4, 0x1C), [0x3F99_999A, 0x4000_0000, 0x4220_0000], 0) else { continue };
        still_ribbon(fx, r, g, 0x01E0_600C);
        copy_n(fx, RING + 12 * s5, g + 0x28, 3);
    }
}

/// Every fourth update 0x5C..0xFC [F 0x1E060E8]: at a point about the
/// target a band, a ribbon, ten sprites, an arc stroke from the first ring
/// point, three bolts and a flash.
fn burst(fx: &mut Fx, sp: u32, r: &mut Regs) {
    r.s0 = u64::from(sp + 0xD0);
    copy_n(fx, sp + 0xB0, sp + 0xD0, 4);
    copy_n(fx, sp + 0xD0, sp + 0xC0, 4);
    let v = ee::add(fx.mem.f(sp + 0xC0), jit(fx, r, 0x01E0_613C, 0xDAC, 0x6D6));
    fx.mem.setf(sp + 0xC0, v);
    let y = fx.mem.read_u32(RING + 4);
    fx.mem.write_u32(sp + 0xC4, y);
    let (ra, side) = if fx.mem.read_u32(D + 0x30) == 0x8000 { (0x01E0_61ACu32, 40.0f32) } else { (0x01E0_6210, -40.0) };
    let j = ee::add(side, jit(fx, r, ra, 0xDAC, 0x6D6));
    let v = ee::add(fx.mem.f(sp + 0xC8), j);
    fx.mem.setf(sp + 0xC8, v);
    for (o, ra, m, half) in [(0xD0u32, 0x01E0_626Cu32, 0xBB8, 0x5DC), (0xD4, 0x01E0_62B8, 0x7D0, 0x3E8), (0xD8, 0x01E0_6308, 0x5DC, 0x2EE)] {
        let j = jit(fx, r, ra, m, half);
        let v = ee::add(fx.mem.f(sp + o), j);
        fx.mem.setf(sp + o, v);
    }
    if let Some(o) = band(fx, 0xE, 0, [sp + 0xC0, sp + 0xC4, sp + 0xC8, sp + 0xCC], false) {
        r.s1 = u64::from(o);
    }
    let i = first_free(fx, RIBBONS, 0x68, 0x50);
    if i >= 0 {
        let g = RIBBONS + 0x68 * i as u32;
        r.s1 = u64::from(0x68 * i as u32);
        set_counter4(fx, g, 0xE, 0, 0xFF);
        fx.mem.write_u8(g + 4, 4);
        fx.mem.write_u32(g + 0x14, 0x3F80_0000);
        fx.mem.write_u32(g + 0x18, 0x3F80_0000);
        fx.mem.write_u32(g + 0x1C, 0x4220_0000);
        fx.mem.write_u8(g + 8, 0x1C);
        still_ribbon(fx, r, g, 0x01E0_64D4);
        copy_n(fx, sp + 0xC0, g + 0x28, 4);
    }
    let ring_pt = RING + 12 * RUNNER_S7;
    for _ in 0..0xA {
        let j = first_free(fx, SPRITES, 0x44, 0xC8);
        if j < 0 {
            continue;
        }
        let count = ((rand4(fx, r, 0x01E0_6588) & 7) + 0x20) & 0xFF;
        r.s1 = count as u64;
        let delay = rand4(fx, r, 0x01E0_65A0) & 7;
        let q = SPRITES + 0x44 * j as u32;
        set_counter4(fx, q, count, delay, 0xFF);
        bytes(fx, q, &[(4, 0x42), (8, 0), (0xB, 2), (0xA, 0)]);
        let v = (rand4(fx, r, 0x01E0_65F0) & 3) + 2;
        fx.mem.set8(q + 9, v);
        bytes(fx, q, &[(0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)]);
        fx.mem.write_u32(q + 0x10, 0x420C_0000);
        fx.mem.write_u32(q + 0x14, 0xBE99_999A);
        fx.mem.write_u32(q + 0x18, 0xBD75_C28F);
        for (o, a) in [(0x1Cu32, ring_pt + 4), (0x20, sp + 0xC0), (0x24, sp + 0xC4), (0x28, sp + 0xC8)] {
            let w = fx.mem.read_u32(a);
            fx.mem.write_u32(q + o, w);
        }
        let v = jit(fx, r, 0x01E0_6678, 0x3C, 0x1E);
        fx.mem.setf(q + 0x2C, v);
        let v = ee::add(f32::from_bits(0x3F4C_CCCD), jit(fx, r, 0x01E0_66C4, 0x64, 0x32));
        fx.mem.setf(q + 0x30, v);
        let v = jit(fx, r, 0x01E0_6720, 0x3C, 0x1E);
        fx.mem.setf(q + 0x34, v);
    }
    let i = first_free(fx, STROKES, 0x20, 0x50);
    if i >= 0 {
        fx.mem.set32(sp + 0x100, 0xB);
        let radius = ee::add(30.0, jit200(fx, r, 0x01E0_67A4, 0xA, 0x3E8));
        r.s1 = u64::from(POINTS.wrapping_add((0x90i32.wrapping_mul(i)) as u32));
        arc_stroke(fx, sp, i, 0xB, radius, [sp + 0xC0, sp + 0xC4, sp + 0xC8], [sp + 0xD0, sp + 0xD4, sp + 0xD8], (0x10, 0x80, 0x3E4C_CCCD, [3, 2], 0x60));
    }
    let mut s5 = 0;
    while s5 < 3 {
        let i = first_free(fx, BOLTS, 0x40, 0x64);
        if i < 0 {
            break;
        }
        let b = BOLTS + ((i as u32) << 6);
        r.s1 = u64::from((i as u32) << 6);
        set_counter4(fx, b, 0xE, s5 & 0xFF, 0xFF);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 4, 0x84);
        for k in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xD0 + 4 * k);
            fx.mem.write_u32(b + 0x28 + 4 * k, w);
            fx.mem.write_u32(b + 0x1C + 4 * k, w);
        }
        for (k, ra) in [0x01E0_698Cu32, 0x01E0_69EC, 0x01E0_6A4C].iter().enumerate() {
            let j = jit200(fx, r, *ra, 0x12, 0x708);
            let a = b + 0x28 + 4 * k as u32;
            let v = ee::add(fx.mem.f(a), j);
            fx.mem.setf(a, v);
        }
        fx.mem.write_u8(b + 8, 7);
        fx.mem.write_u32(b + 0x10, 0x3F80_0000);
        fx.mem.write_u32(b + 0x14, 0x3ECC_CCCD);
        bytes(fx, b, &[(0xB, 1), (0xD, 4), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)]);
        s5 += 1;
    }
    let i = first_free(fx, FLASH, 0x4C, 0x18);
    if i >= 0 {
        let f = FLASH + 0x4C * i as u32;
        r.s1 = u64::from(0x4C * i as u32);
        set_counter4(fx, f, 0x10, 0, 0xFF);
        bytes(fx, f, &[(5, 0), (4, 0x80), (7, 9), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xF, 0xC0), (0xA, 0x80), (9, 0x80), (8, 0x10)]);
        fx.mem.write_u32(f + 0x10, 0x437A_0000);
        fx.mem.write_u32(f + 0x14, 0x41A0_0000);
        fx.mem.write_u32(f + 0x18, 0x4170_0000);
        copy_n(fx, sp + 0xD0, f + 0x1C, 4);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut r = Regs { s0: i64::from(param) as u64, s1: 0 };
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xB0, r.s0, 0x01E0_5468);
    match fx.mem.u16(D) {
        0x10E => return 8,
        0x102 => {
            for k in 0..4 {
                fx.mem.write_u8(sp + 0x12C + k, 0x80);
            }
            let t = fx.mem.i32(D + 8);
            fx.unit_tint_mode(t, -1, sp + 0x12C, [r.s0, r.s1], 0x01E0_5D98);
        }
        0x64 => jitter_on(fx, sp, &r),
        0x5A => shake_target(fx, sp),
        0x28 => ring(fx, sp, &mut r),
        2 => opening(fx, &mut r),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0xC0).contains(&n) && n & 3 == 0 {
        node_ribbons(fx, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0x29..0xFF).contains(&n) && n & 3 == 0 {
        ring_ribbons(fx, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0x5A..0xFF).contains(&n) && n & 3 == 0 {
        burst(fx, sp, &mut r);
    }
    shake_saving(fx, SHAKE, r.s0, 0x01E0_6BD8, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6BE8, [Some(r.s0), Some(r.s1)]);
    blib::fade::fade(fx, FADE, OT);
    blib::star::flash(fx, 0x01E0_0950, FLASH, OT, 0x18);
    blib::streak::ribbon(fx, 0x01E0_1500, RIBBONS, OT, 0x50);
    let (_, bolt_a3) = blib::bolt::bolts_a3(fx, 0x01E0_2090, BOLTS, OT, 0x64);
    let (_, stroke_a3) = blib::dash::strokes_b_a3(fx, 0x01E0_2BE0, STROKES, OT, 0x50);
    let (_, sprite_a3) = blib::sprite::bounce_a3(fx, SPRITES, OT, 0xC8);
    let (_, band_a3) = blib::band::bands_a3(fx, 0x01E0_4AD0, BANDS, OT, 0xA);
    // [H] a3 = what the bands / sprites / strokes / bolts left, else not
    // modelled (0).
    let a3 = band_a3.or(sprite_a3).or(stroke_a3).or(bolt_a3).unwrap_or(0);
    blib::radiate::jitter(fx, 0x01E0_42A0, JITTER, OT, 1, r.s0 as u32, a3);
    let n = fx.mem.u16(D);
    if (0x5A..0xFF).contains(&n) {
        6
    } else if n == 0xFF {
        2
    } else if n == 0x50 {
        1
    } else {
        0
    }
}
