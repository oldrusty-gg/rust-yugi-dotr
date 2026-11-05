//! A textured sphere, hand-ported from b364
//! (card 338) [F]: e38cf4b3c49d lays out 16 rings of 32 points
//! (`setup`), 575378e537b6 draws the bands between them as textured quads
//! (`draw`).
//!
//! A sphere `g`: +0 the texture, +4 flags (0xA..0xC blend, 0x13 far Z,
//! 0x14..0x17 vertex k projected), +8 the colour (B, G, R, A), +0xC the
//! radius, +0x10 / +0x12 / +0x14 / +0x16 the cell (u, v, w, h), +0x20 +
//! 0x180 i + 12 j point j of ring i.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};

/// RGBAQ / UV / XYZ2 x 4 (a textured strip).
const REGS_TEX4: u64 = 0x4214_2142_1421;

/// e38cf4b3c49d [2 B records; b364 0x1E02A80, 60 words] [F]:
/// `setup(g, rh2, r) -> 0`: texture `rh2`, radius `r`; ring i (0..15) at
/// the 16-bit latitude 0x4000 - 0x888 i, its 32 points 0x800 apart: (r
/// cos lat sin lon, r sin lat, r cos lat cos lon). A 0x80 frame.
pub fn setup(fx: &mut Fx, g: u32, rh2: u32, r: f32) -> i32 {
    let sp = fx.enter(0x80);
    let _ = sp;
    fx.mem.write_u32(g, rh2);
    fx.mem.setf(g + 0xC, r);
    for i in 0..16u32 {
        let lat = 0x4000 - 0x888 * i as i32;
        let c = fx.tcos(lat);
        let rc = ee::mul(r, c);
        let s = fx.tsin(lat);
        let rs = ee::mul(r, s);
        for j in 0..32u32 {
            let p = g + 0x180 * i + 12 * j;
            let lon = 0x800 * j as i32;
            let c = fx.tcos(lon);
            fx.mem.setf(p + 0x28, ee::mul(rc, c));
            let s = fx.tsin(lon);
            fx.mem.setf(p + 0x20, ee::mul(rc, s));
            fx.mem.setf(p + 0x24, rs);
        }
    }
    fx.leave(0x80);
    0
}

/// b0fef4f7d29f [b118 0x1E02AC0, 60 words] [F word-compare, helper C11]:
/// [`setup`] with 8 rings of 16 points (latitude 0x4000 - 0x1249 i,
/// longitude 0x1000 j; ring i at +0x20 + 0xC0 i).
pub fn setup_b(fx: &mut Fx, g: u32, rh2: u32, r: f32) -> i32 {
    let sp = fx.enter(0x80);
    let _ = sp;
    fx.mem.write_u32(g, rh2);
    fx.mem.setf(g + 0xC, r);
    for i in 0..8u32 {
        let lat = 0x4000 - 0x1249 * i as i32;
        let c = fx.tcos(lat);
        let rc = ee::mul(r, c);
        let s = fx.tsin(lat);
        let rs = ee::mul(r, s);
        for j in 0..16u32 {
            let p = g + 0xC0 * i + 12 * j;
            let lon = 0x1000 * j as i32;
            let c = fx.tcos(lon);
            fx.mem.setf(p + 0x28, ee::mul(rc, c));
            let s = fx.tsin(lon);
            fx.mem.setf(p + 0x20, ee::mul(rc, s));
            fx.mem.setf(p + 0x24, rs);
        }
    }
    fx.leave(0x80);
    0
}

/// 015fa0447f6d [b118 0x1E02BB0, 448 words] [F, helper C11]: the
/// [`setup_b`] sphere's draw `draw_b(ot, g, x) -> 0` (descriptor at sp +
/// 0x100, a 0x190 frame): ALPHA 0x88 / 0x42 / 0x48 by flags 0xB / 0xA, the
/// colour; every quad the whole cell - with flags bit 0 integer UVs (+0x10
/// / +0x12 / +0x14 / +0x16 as u, v, w, h; the UV flag 0x800) and
/// REGION_CLAMP to it (mode 0xA), else ST floats (s +0x10, t +0x14, ds
/// +0x18, dt +0x1C); texture filter 0 (no clamp) with flags bit 0x18;
/// rings 1..7 and points j as [`draw`] (j - 1 of 0 = 15), projected
/// through `x` + 0x30 (w at sp + 0xF0..0xFC), keyed far (0x13) or by the
/// mean w with all four projected.
pub fn draw_b(fx: &mut Fx, ot: u32, g: u32, x: u32) -> i32 {
    let sp = fx.enter(0x190);
    let d = sp + 0x100;
    fx.desc_init(d, 0xC, 0x34, REGS_TEX4);
    let flags = |fx: &mut Fx| fx.mem.read_u32(g + 4);
    let alpha = if bit(flags(fx), 0xB) {
        0x88
    } else if bit(flags(fx), 0xA) {
        0x42
    } else {
        0x48
    };
    fx.desc_alpha(d, 0x80_0000_0000 | alpha);
    fx.colours4(d, g + 8);
    if bit(flags(fx), 0) {
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        let h = |fx: &mut Fx, o: u32| fx.mem.i16(g + o);
        let (u, v) = (h(fx, 0x10), h(fx, 0x12));
        let (u1, v1) = (u + h(fx, 0x14) - 1, v + h(fx, 0x16) - 1);
        for (a, val) in [(0x118u32, u), (0x11A, v), (0x130, u), (0x132, v1), (0x148, u1), (0x14A, v), (0x160, u1), (0x162, v1)] {
            fx.set_uv(sp + a, val);
        }
    } else {
        let f = |fx: &mut Fx, o: u32| fx.mem.f(g + o);
        let (s, t) = (f(fx, 0x10), f(fx, 0x14));
        let s1 = ee::add(s, f(fx, 0x18));
        let t1 = ee::add(t, f(fx, 0x1C));
        for (a, val) in [(0x118u32, s), (0x11C, t), (0x130, s), (0x134, t1), (0x148, s1), (0x14C, t), (0x160, s1), (0x164, t1)] {
            fx.mem.setf(sp + a, val);
        }
    }
    let (xyz, w) = ([sp + 0x120, sp + 0x138, sp + 0x150, sp + 0x168], [sp + 0xF0, sp + 0xF4, sp + 0xF8, sp + 0xFC]);
    let mut i = 1i32;
    fx.mem.write_u32(sp + 0xD0, g + 0xC0);
    while i < 8 {
        fx.mem.set32(sp + 0xA0, i);
        let cur = fx.mem.read_u32(sp + 0xD0);
        let prev = g + 0xC0 * (i - 1) as u32;
        fx.mem.write_u32(sp + 0xE0, prev);
        fx.mem.write_u32(sp + 0xB0, prev + 0xD4);
        fx.mem.write_u32(sp + 0xC0, cur + 0xD4);
        for j in 0..16u32 {
            let (a, b) = if j == 0 { (prev + 0xD4, cur + 0xD4) } else { (prev + 12 * (j - 1) + 0x20, cur + 12 * (j - 1) + 0x20) };
            let pts = [a, b, prev + 12 * j + 0x20, cur + 12 * j + 0x20];
            for k in 0..4 {
                let m = fx.read_m4(x + 0x30);
                if fx.project_at(pts[k], &m, xyz[k], w[k]) {
                    fx.flag(g + 4, 0x14 + k as u32);
                }
            }
            let rh2 = fx.mem.read_u32(g);
            if bit(flags(fx), 0x18) {
                fx.desc_texture(d, rh2, 0);
            } else {
                if bit(flags(fx), 0) {
                    let h = |fx: &mut Fx, o: u32| i64::from(fx.mem.i16(g + o));
                    let (u, v) = (h(fx, 0x10), h(fx, 0x12));
                    let clamp = (u << 4 | 0xA) | ((u + h(fx, 0x14) - 1) << 14) | (v << 24) | ((v + h(fx, 0x16) - 1) << 34);
                    fx.desc_clamp(d, clamp as u64);
                }
                fx.desc_texture(d, rh2, 1);
            }
            if bit(flags(fx), 0x13) {
                crate::fxlib2::far_z(fx, &[xyz[3], xyz[2], xyz[1], xyz[0]]);
                fx.submit(d, ot, 0);
            } else if bit(flags(fx), 0x14) && bit(flags(fx), 0x15) && bit(flags(fx), 0x16) && bit(flags(fx), 0x17) {
                let s = ee::add(fx.mem.f(w[0]), fx.mem.f(w[1]));
                let s = ee::add(fx.mem.f(w[2]), s);
                let s = ee::add(fx.mem.f(w[3]), s);
                let key = ee::fixunssfsi(ee::div(s, 4.0)) as i32;
                fx.submit(d, ot, key);
            }
            let f = fx.mem.read_u32(g + 4) & 0xFF0F_FFFF;
            fx.mem.write_u32(g + 4, f);
        }
        let c = fx.mem.read_u32(sp + 0xD0) + 0xC0;
        fx.mem.write_u32(sp + 0xD0, c);
        i += 1;
    }
    fx.mem.set32(sp + 0xA0, i);
    fx.leave(0x190);
    0
}

/// 575378e537b6 [2 B records; b364 0x1E02B70, 436 words] [F]:
/// `draw(ot, g, x) -> 0` (descriptor at sp + 0x180: a textured strip
/// with UVs, REGION_CLAMP to the cell, filter 1, ALPHA 0x44 / 0x88 / 0x42
/// / 0x48 by flags 0xC / 0xB / 0xA, the sphere's colour): for rings 1..15
/// and points j, the quad (ring i - 1, point j - 1), (ring i, j - 1),
/// (i - 1, j), (i, j) (j - 1 of 0 = 31) through `x` + 0x30 (0x204D80, w
/// at sp + 0x170..0x17C), u = j w / 32, v = i h / 16 of the cell size;
/// keyed far (0x13, key 0, drawn even when not projected) or, with all
/// four projected, by the mean w; vertex bits cleared. A 0x210 frame.
pub fn draw(fx: &mut Fx, ot: u32, g: u32, x: u32) -> i32 {
    let sp = fx.enter(0x210);
    let d = sp + 0x180;
    fx.desc_init(d, 0xC, 0x34, REGS_TEX4);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let h = |fx: &mut Fx, o: u32| i64::from(fx.mem.i16(g + o));
    let clamp = ((h(fx, 0x14) - 1) << 4 | 0xB) | (h(fx, 0x10) << 14) | (h(fx, 0x12) << 24) | ((h(fx, 0x12) + h(fx, 0x16) - 1) << 34);
    fx.desc_clamp(d, clamp as u64);
    let rh2 = fx.mem.read_u32(g);
    fx.desc_texture(d, rh2, 1);
    let flags = fx.mem.read_u32(g + 4);
    let alpha = if bit(flags, 0xC) {
        0x44
    } else if bit(flags, 0xB) {
        0x88
    } else if bit(flags, 0xA) {
        0x42
    } else {
        0x48
    };
    fx.desc_alpha(d, 0x80_0000_0000 | alpha);
    fx.colours4(d, g + 8);
    let du = (fx.mem.i16(g + 0x14) >> 5) as i16 as i32;
    let dv = (fx.mem.i16(g + 0x16) >> 4) as i16 as i32;
    // The stack locals as the original keeps them.
    fx.mem.set32(sp + 0x110, du);
    fx.mem.set32(sp + 0x150, dv);
    fx.mem.set32(sp + 0x100, dv);
    fx.mem.write_u32(sp + 0xF0, g + 0x180);
    for (k, a) in [(0x120u32, 0x19Au32), (0x130, 0x1B2), (0x140, 0x1CA)] {
        fx.mem.write_u32(sp + k, sp + a);
    }
    let (xyz, w) = ([sp + 0x1A0, sp + 0x1B8, sp + 0x1D0, sp + 0x1E8], [sp + 0x170, sp + 0x174, sp + 0x178, sp + 0x17C]);
    let mut i = 1i32;
    while i < 0x10 {
        fx.mem.set32(sp + 0xA0, i);
        let cur = fx.mem.read_u32(sp + 0xF0);
        let prev = g + 0x180 * (i - 1) as u32;
        let vp = fx.mult(dv, i - 1);
        fx.mem.write_u32(sp + 0x160, prev);
        fx.mem.set32(sp + 0xD0, vp << 4);
        fx.mem.write_u32(sp + 0xB0, prev + 0x194);
        fx.mem.write_u32(sp + 0xC0, cur + 0x194);
        let vc = fx.mem.i32(sp + 0x100);
        fx.mem.set32(sp + 0xE0, vc << 4);
        let mut u = 0i32;
        for j in 0..32u32 {
            let (a, b) = if j == 0 { (prev + 0x194, cur + 0x194) } else { (prev + 12 * (j - 1) + 0x20, cur + 12 * (j - 1) + 0x20) };
            let pts = [a, b, prev + 12 * j + 0x20, cur + 12 * j + 0x20];
            for k in 0..4 {
                let m = fx.read_m4(x + 0x30);
                if fx.project_at(pts[k], &m, xyz[k], w[k]) {
                    fx.flag(g + 4, 0x14 + k as u32);
                }
            }
            fx.set_uv(sp + 0x198, u);
            let un = fx.mult(du, j as i32 + 1);
            fx.set_uv(sp + 0x19A, vp);
            fx.set_uv(sp + 0x1B0, u);
            fx.set_uv(sp + 0x1B2, vc);
            fx.set_uv(sp + 0x1C8, un);
            fx.set_uv(sp + 0x1CA, vp);
            fx.set_uv(sp + 0x1E0, un);
            fx.set_uv(sp + 0x1E2, vc);
            let flags = fx.mem.read_u32(g + 4);
            if bit(flags, 0x13) {
                crate::fxlib2::far_z(fx, &[xyz[3], xyz[2], xyz[1], xyz[0]]);
                fx.submit(d, ot, 0);
            } else if bit(flags, 0x14) && bit(flags, 0x15) && bit(flags, 0x16) && bit(flags, 0x17) {
                let s = ee::add(fx.mem.f(w[0]), fx.mem.f(w[1]));
                let s = ee::add(fx.mem.f(w[2]), s);
                let s = ee::add(fx.mem.f(w[3]), s);
                let key = ee::fixunssfsi(ee::div(s, 4.0)) as i32;
                fx.submit(d, ot, key);
            }
            let f = fx.mem.read_u32(g + 4) & 0xFF0F_FFFF;
            fx.mem.write_u32(g + 4, f);
            u = u.wrapping_add(du);
        }
        let c = fx.mem.read_u32(sp + 0xF0) + 0x180;
        fx.mem.write_u32(sp + 0xF0, c);
        let v = fx.mem.i32(sp + 0x100).wrapping_add(dv);
        fx.mem.set32(sp + 0x100, v);
        i += 1;
    }
    fx.mem.set32(sp + 0xA0, i);
    fx.leave(0x210);
    0
}
