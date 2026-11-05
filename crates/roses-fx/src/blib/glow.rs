//! A glowing line between two points,
//! hand-ported from b221 (card 322) [F]: 8c35601184f9 cuts the segment
//! into up to 32 pieces and draws each as four screen-space quads
//! widened up, down, right and left by the view-scaled width (`line`).
//!
//! A line group `g`: +0 the texture, +4 flags (0x14 / 0x15 corner on
//! screen, 0x1A hold when grown), +8 the colour (B, G, R, A), +0xC the
//! width, +0x10 / +0x12 / +0x14 / +0x16 the cell (u, v, w, h), +0x28 the
//! piece count (capped at 32), +0x2C the pieces drawn so far.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};

/// RGBAQ / UV / XYZ2 x 4 (a textured strip).
const REGS_TEX4: u64 = 0x4214_2142_1421;

/// A screen coordinate (u16 12.4) moved by `d` (0x1A2C70 of `v` + `d`,
/// or of `v` - `d` when `sub`).
fn moved(v: u16, d: f32, sub: bool) -> u16 {
    let f = ee::from_i32(i32::from(v));
    ee::fixunssfsi(if sub { ee::sub(f, d) } else { ee::add(d, f) }) as u16
}

/// 8c35601184f9 [5 B records; b221 0x1E03560, 492 words] [F]:
/// `line(ot, g, x, a, b in t0) -> 0` (descriptor at sp + 0x490: a
/// textured strip, the cell's UVs (0x104540 order 0) and REGION_CLAMP,
/// filter 1, blend 0x48, the group's colour): the points a + (b - a) k /
/// n (k < n; b itself for k = n; sp + 0xE0 + 16 k) projected through `x`
/// + 0x30 (0x204D80: XYZ at sp + 0x2F0 + 8 k, w at sp + 0x400 + 4 k);
/// piece c (c <= +0x2C, both ends projected) widened by the view scale
/// (0x1018E0) of the width at each end: four quads (y + / -, x + / -;
/// the far corners moved, the near ones at the points), drawn when both
/// moved corners are on screen (0x101600), each corner's Z its point's,
/// keyed by the ends' mean w. Then +0x2C + 1, wrapping to 0 (or held,
/// flag 0x1A) at the count. A 0x520 frame.
pub fn line(fx: &mut Fx, ot: u32, g: u32, x: u32, a: u32, b: u32) -> i32 {
    let sp = fx.enter(0x520);
    fx.mem.write_u32(sp + 0xD8, ot);
    fx.mem.write_u32(sp + 0xD0, x);
    if fx.mem.i16(g + 0x28) >= 0x21 {
        fx.mem.set16(g + 0x28, 0x20);
    }
    fx.mem.set32(sp + 0xB0, 0);
    let d = sp + 0x490;
    fx.desc_init(d, 0xC, 0x34, REGS_TEX4);
    let h = |fx: &mut Fx, o: u32| i64::from(fx.mem.i16(g + o));
    let clamp = (h(fx, 0x10) << 4 | 0xA) | ((h(fx, 0x10) + h(fx, 0x14) - 1) << 14) | (h(fx, 0x12) << 24) | ((h(fx, 0x12) + h(fx, 0x16) - 1) << 34);
    fx.desc_clamp(d, clamp as u64);
    let rh2 = fx.mem.read_u32(g);
    fx.desc_texture(d, rh2, 1);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.colours4(d, g + 8);
    let delta: [f32; 3] = std::array::from_fn(|c| ee::sub(fx.mem.f(b + 4 * c as u32), fx.mem.f(a + 4 * c as u32)));
    let (pts, xyz, ws) = (sp + 0xE0, sp + 0x2F0, sp + 0x400);
    let mut k = 0i32;
    while k < fx.mem.i16(g + 0x28) + 1 {
        let n = fx.mem.i16(g + 0x28);
        let p = pts + 16 * k as u32;
        if k == n {
            for c in 0..4u32 {
                let v = fx.mem.read_u32(b + 4 * c);
                fx.mem.write_u32(p + 4 * c, v);
            }
        } else {
            let t = ee::div(ee::from_i32(k), ee::from_i32(n));
            for c in 0..3u32 {
                let v = ee::add(fx.mem.f(a + 4 * c), ee::mul(delta[c as usize], t));
                fx.mem.setf(p + 4 * c, v);
            }
        }
        let m = fx.read_m4(x + 0x30);
        if fx.project_at(p, &m, xyz + 8 * k as u32, ws + 4 * k as u32) {
            let f = fx.mem.read_u32(sp + 0xB0) | (1 << (k & 31));
            fx.mem.write_u32(sp + 0xB0, f);
        }
        k += 1;
    }
    let (v0, v1, v2, v3) = (sp + 0x4B0, sp + 0x4C8, sp + 0x4E0, sp + 0x4F8);
    fx.mem.set32(sp + 0xC0, 0);
    while fx.mem.i32(sp + 0xC0) < fx.mem.i16(g + 0x28) {
        let c = fx.mem.i32(sp + 0xC0);
        if fx.mem.i16(g + 0x2C) >= c {
            fx.uv_rect(d, g + 0x10, 0);
            let mask = fx.mem.read_u32(sp + 0xB0);
            if bit(mask, c as u32 & 31) && bit(mask, (c as u32 + 1) & 31) {
                let (w0, w1) = (ws + 4 * c as u32, ws + 4 * (c + 1) as u32);
                let (width, depth) = (fx.mem.f(g + 0xC), fx.mem.f(w0));
                let s0 = fx.view_scale(width, depth);
                let (width, depth) = (fx.mem.f(g + 0xC), fx.mem.f(w1));
                let s1 = fx.view_scale(width, depth);
                let (p0, p1) = (xyz + 8 * c as u32, xyz + 8 * (c + 1) as u32);
                for q in 0..4 {
                    let (x0, y0, x1, y1) = (fx.mem.read_u16(p0), fx.mem.read_u16(p0 + 2), fx.mem.read_u16(p1), fx.mem.read_u16(p1 + 2));
                    if q < 2 {
                        for (at, v) in [(v2, x0), (v0, x0), (v3, x1), (v1, x1)] {
                            fx.mem.write_u16(at, v);
                        }
                        let sub = q == 1;
                        fx.mem.write_u16(v0 + 2, moved(y0, s0, sub));
                        fx.mem.write_u16(v1 + 2, moved(y1, s1, sub));
                        fx.mem.write_u16(v2 + 2, y0);
                        fx.mem.write_u16(v3 + 2, y1);
                    } else {
                        for (at, v) in [(v2 + 2, y0), (v0 + 2, y0), (v3 + 2, y1), (v1 + 2, y1)] {
                            fx.mem.write_u16(at, v);
                        }
                        let sub = q == 3;
                        fx.mem.write_u16(v0, moved(x0, s0, sub));
                        fx.mem.write_u16(v1, moved(x1, s1, sub));
                        fx.mem.write_u16(v2, x0);
                        fx.mem.write_u16(v3, x1);
                    }
                    let c0 = fx.mem.read_u64(v0);
                    if Fx::on_screen(c0) {
                        fx.flag(g + 4, 0x14);
                    }
                    let c1 = fx.mem.read_u64(v1);
                    if Fx::on_screen(c1) {
                        fx.flag(g + 4, 0x15);
                    }
                    let flags = fx.mem.read_u32(g + 4);
                    if bit(flags, 0x14) && bit(flags, 0x15) {
                        let z0 = fx.xyz_z(p0);
                        fx.set_xyz_z(v2, z0);
                        let z = fx.xyz_z(v2);
                        fx.set_xyz_z(v0, z);
                        let z1 = fx.xyz_z(p1);
                        fx.set_xyz_z(v3, z1);
                        let z = fx.xyz_z(v3);
                        fx.set_xyz_z(v1, z);
                        let key = ee::fixunssfsi(ee::div(ee::add(fx.mem.f(w0), fx.mem.f(w1)), 2.0)) as i32;
                        fx.submit(d, ot, key);
                    }
                    let f = fx.mem.read_u32(g + 4) & 0xFFCF_FFFF;
                    fx.mem.write_u32(g + 4, f);
                }
            }
        }
        fx.mem.set32(sp + 0xC0, c + 1);
    }
    let s = fx.mem.i16(g + 0x2C) + 1;
    fx.mem.set16(g + 0x2C, s);
    if fx.mem.i16(g + 0x2C) >= fx.mem.i16(g + 0x28) {
        if bit(fx.mem.read_u32(g + 4), 0x1A) {
            let n = fx.mem.i16(g + 0x28);
            fx.mem.set16(g + 0x2C, n);
        } else {
            fx.mem.set16(g + 0x2C, 0);
        }
    }
    fx.leave(0x520);
    0
}
