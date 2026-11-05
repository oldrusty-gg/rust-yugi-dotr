//! A jagged textured strand between two points,
//! hand-ported from b147 (card 311) [F]: 9f0de6fe0b60 cuts the segment
//! into up to 16 pieces, jitters the inner points and draws each piece as
//! one screen-space quad widened across its steeper axis (`draw`).
//!
//! A strand group `g`: +0 the texture, +4 flags (0x13 far Z, 0x1A every
//! piece, 0x1B the width as it is), +8 the colour (B, G, R, A), +0xC the
//! piece count (capped at 16), +0xE the jitter, +0x10 / +0x12 / +0x14 /
//! +0x16 the cell (u, v, w, h), +0x28 the pieces drawn so far, +0x2C the
//! width.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, Fx};

/// RGBAQ / UV / XYZ2 x 4 (a textured strip).
const REGS_TEX4: u64 = 0x4214_2142_1421;

/// A screen coordinate (u16 12.4) moved by `d` (0x1A2C70 of `v` - `d`,
/// or of `d` + `v`).
fn moved(v: u16, d: f32, sub: bool) -> u16 {
    let f = ee::from_i32(i32::from(v));
    ee::fixunssfsi(if sub { ee::sub(f, d) } else { ee::add(d, f) }) as u16
}

/// 9f0de6fe0b60 [11 B records; b147 0x1E01930, 608 words] [F]:
/// `draw(at, ot, g, x, a, b in t0) -> 0` (descriptor at sp + 0x1B0: a
/// textured strip, blend 0x48, the group's colour): a projected through
/// `x` + 0x30 (0x204D80: XYZ sp + 0xC0 + 8 k, w sp + 0x160 + 4 k), the
/// points a + (b - a) k / n (k = 1..n - 1; each axis + 0.1 (rand % 2j -
/// j) with jitter j, rand frames saving the on-screen mask s0), then b;
/// bit k of the mask when point k is on screen (0x101600). Piece c (c <=
/// +0x28 unless 0x1A) gets a random UV orientation (rand % 4 -> 0x104540
/// order 1 / 2 / 5 / 7) and, when both ends are on screen, the quad of
/// half width w (+0x2C with 0x1B, else sqrt(dx^2 + dy^2) / 3 of the
/// screen deltas): across y when |dx| > |dy|, else across x; REGION_CLAMP
/// to the cell, filter 1; far Z (0x13, key 0) or each end's Z keyed by
/// the ends' mean w (0x1A2C70). Then +0x28 + 1, back to 0 past the count.
/// A 0x240 frame.
pub fn draw(fx: &mut Fx, at: u32, ot: u32, g: u32, x: u32, a: u32, b: u32) -> i32 {
    let sp = fx.enter(0x240);
    fx.mem.write_u32(sp + 0xB8, ot);
    let mut mask = 0u32;
    if fx.mem.i16(g + 0xC) >= 0x11 {
        fx.mem.set16(g + 0xC, 0x10);
    }
    let d = sp + 0x1B0;
    fx.desc_init(d, 0xC, 0x34, REGS_TEX4);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.colours4(d, g + 8);
    let delta: [f32; 3] = std::array::from_fn(|c| ee::sub(fx.mem.f(b + 4 * c as u32), fx.mem.f(a + 4 * c as u32)));
    let (xyz, ws, p) = (sp + 0xC0, sp + 0x160, sp + 0x150);
    let m = fx.read_m4(x + 0x30);
    if fx.project_at(a, &m, xyz, ws) && Fx::on_screen(fx.mem.read_u64(xyz)) {
        mask |= 1;
    }
    let mut k = 1i32;
    while k < fx.mem.i16(g + 0xC) {
        let n = fx.mem.i16(g + 0xC);
        let t = ee::div(ee::from_i32(k), ee::from_i32(n));
        if fx.mem.i16(g + 0xE) != 0 {
            for (c, off) in [(0u32, 0x150u32), (1, 0x198), (2, 0x1E0)] {
                let j = fx.mem.i16(g + 0xE);
                let r = fx.rand_from(mask, at + off);
                let (_, h) = fx.div(r, 2 * j);
                let v = ee::add(fx.mem.f(a + 4 * c), ee::mul(delta[c as usize], t));
                let v = ee::add(v, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h - j)));
                fx.mem.setf(p + 4 * c, v);
            }
        } else {
            for c in 0..3u32 {
                let v = ee::add(fx.mem.f(a + 4 * c), ee::mul(delta[c as usize], t));
                fx.mem.setf(p + 4 * c, v);
            }
        }
        let m = fx.read_m4(x + 0x30);
        let at_xyz = xyz + 8 * k as u32;
        if fx.project_at(p, &m, at_xyz, ws + 4 * k as u32) && Fx::on_screen(fx.mem.read_u64(at_xyz)) {
            mask |= 1 << (k & 31);
        }
        k += 1;
    }
    let n = fx.mem.i16(g + 0xC);
    let m = fx.read_m4(x + 0x30);
    if fx.project_at(b, &m, xyz + 8 * n as u32, ws + 4 * n as u32) {
        let n = fx.mem.i16(g + 0xC);
        if Fx::on_screen(fx.mem.read_u64(xyz + 8 * n as u32)) {
            mask |= 1 << (n & 31);
        }
    }
    let (v0, v1, v2, v3) = (sp + 0x1D0, sp + 0x1E8, sp + 0x200, sp + 0x218);
    let mut c = 0i32;
    while c < fx.mem.i16(g + 0xC) {
        let (p0, p1) = (xyz + 8 * c as u32, xyz + 8 * (c + 1) as u32);
        let (w0, w1) = (ws + 4 * c as u32, ws + 4 * (c + 1) as u32);
        if !bit(fx.mem.read_u32(g + 4), 0x1A) && fx.mem.i16(g + 0x28) < c {
            c += 1;
            continue;
        }
        let r = fx.rand_from(mask, at + 0x358);
        let order = match r.wrapping_rem(4) {
            0 => 1,
            1 => 2,
            2 => 5,
            _ => 7,
        };
        fx.uv_rect(d, g + 0x10, order);
        let (x0, y0, x1, y1) = (fx.mem.read_u16(p0), fx.mem.read_u16(p0 + 2), fx.mem.read_u16(p1), fx.mem.read_u16(p1 + 2));
        let dx = ee::abs(ee::from_i32(i32::from(x0) - i32::from(x1)));
        let dy = ee::abs(ee::from_i32(i32::from(y0) - i32::from(y1)));
        // mula.s / madd.s: the products rounded, then the sum.
        let len = fx.sqrtf(ee::add(ee::mul(dx, dx), ee::mul(dy, dy)));
        let w = if bit(fx.mem.read_u32(g + 4), 0x1B) { fx.mem.f(g + 0x2C) } else { ee::div(len, 3.0) };
        if ee_le(dx, dy) {
            fx.mem.write_u16(v0, moved(x0, w, true));
            fx.mem.write_u16(v1, moved(x1, w, true));
            fx.mem.write_u16(v2, moved(x0, w, false));
            fx.mem.write_u16(v3, moved(x1, w, false));
            for (at, v) in [(v2 + 2, y0), (v0 + 2, y0), (v3 + 2, y1), (v1 + 2, y1)] {
                fx.mem.write_u16(at, v);
            }
        } else {
            for (at, v) in [(v2, x0), (v0, x0), (v3, x1), (v1, x1)] {
                fx.mem.write_u16(at, v);
            }
            fx.mem.write_u16(v0 + 2, moved(y0, w, true));
            fx.mem.write_u16(v1 + 2, moved(y1, w, true));
            fx.mem.write_u16(v2 + 2, moved(y0, w, false));
            fx.mem.write_u16(v3 + 2, moved(y1, w, false));
        }
        let h = |fx: &mut Fx, o: u32| i64::from(fx.mem.i16(g + o));
        let clamp = (h(fx, 0x10) << 4 | 0xA) | ((h(fx, 0x10) + h(fx, 0x14) - 1) << 14) | (h(fx, 0x12) << 24) | ((h(fx, 0x12) + h(fx, 0x16) - 1) << 34);
        fx.desc_clamp(d, clamp as u64);
        let rh2 = fx.mem.read_u32(g);
        fx.desc_texture(d, rh2, 1);
        if bit(mask, c as u32 & 31) && bit(mask, (c as u32 + 1) & 31) {
            if bit(fx.mem.read_u32(g + 4), 0x13) {
                fx.far_z(&[v0, v1, v2, v3]);
                fx.submit(d, ot, 0);
            } else {
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
        }
        c += 1;
    }
    let s = fx.mem.i16(g + 0x28) + 1;
    fx.mem.set16(g + 0x28, s);
    if fx.mem.i16(g + 0xC) < fx.mem.i16(g + 0x28) {
        fx.mem.set16(g + 0x28, 0);
    }
    fx.leave(0x240);
    0
}
