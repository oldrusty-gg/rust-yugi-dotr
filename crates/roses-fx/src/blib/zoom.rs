//! A unit's zoom afterimage, hand-ported from
//! b067 (card 208) [F]: 91008151b0ec draws the unit into the render
//! target (0x1F8270, as `afterimage::strips`) and then layers of four
//! sprites from the unit's screen position to the window's quarters, each
//! layer's UVs closing in on the unit (`sprites`).
//!
//! A group `r`: +0 flags (0xA..0xC the blend, 0x13 far Z), +4 the colour
//! (B, G, R, A), +8 the zoom scale, +0xC the layer count, +0xE the unit's
//! handle.
use super::afterimage::{depth_key, depth_w, unit_radius};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};

/// Z (bits 32..55) of the XYZ2 values at `xyz` set to `z`.
fn set_z(fx: &mut Fx, xyz: &[u32], z: u64) {
    for &a in xyz {
        let v = fx.mem.read_u64(a);
        fx.mem.write_u64(a, (v & 0xFF00_0000_FFFF_FFFF) | ((z & 0xFF_FFFF) << 32));
    }
}

/// i16 of a 32-bit value (dsll32 / dsra32 by 16).
fn h16(v: i32) -> i32 {
    v as i16 as i32
}

/// 91008151b0ec [8 B records; b067 0x1E00120, 436 words] [F]:
/// `sprites(at, r, v) -> 0` (-1 when `v`, the unit's position, does not
/// project: the layer is left open). In a packet layer (0x2078A0): `v`
/// projected through a fresh transform (0x103890 / 0x1038D0 at sp +
/// 0x1A0; XYZ sp + 0x270, w sp + 0x27C); its screen position less 0x700
/// clamped to 0..0x200 is (sx, sy); a sprite descriptor at sp + 0x218
/// (ALPHA 0x44 / 0x88 / 0x42 / 0x48 by flags 0xC / 0xB / 0xA, the colour
/// +4); TEXFLUSH, the unit of +0xE drawn into the target (0x1F8270 filter
/// 0), TEXFLUSH, flags | 0x800; then +0xC layers of the four sprites from
/// the unit's position (UV (sx, sy)) to the corners (0x700 / 0x900, 0x700
/// / 0x900), the far UVs the layer's (ax, ay) / (0x200 - bx, 0x200 - by),
/// which grow each layer by the scale +8 times sx / 512, sy / 512, 1 -
/// sx / 512, 1 - sy / 512 (0x1A2B80); Z far (0x13, key 0) or the unit's
/// depth moved its radius toward the camera (0x205650, keyed by
/// 0x205730's w); drawn with no ordering table. Then the layer linked
/// into the ordering table at key 0. A 0x280 frame.
pub fn sprites(fx: &mut Fx, at: u32, r: u32, v: u32) -> i32 {
    let sp = fx.enter(0x280);
    fx.mem.write_u32(sp + 0x19C, v);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xBC, layer);
    fx.xform_init(sp + 0x1A0);
    fx.xform_world(sp + 0x1A0);
    let p = fx.mem.read_u32(sp + 0x19C);
    let m = fx.read_m4(sp + 0x1D0);
    if !fx.project_at(p, &m, sp + 0x270, sp + 0x27C) {
        fx.leave(0x280);
        return -1;
    }
    let mut sx = h16((i32::from(fx.mem.read_u16(sp + 0x270)) >> 4) - 0x700);
    let mut sy = h16((i32::from(fx.mem.read_u16(sp + 0x272)) >> 4) - 0x700);
    if sx < 0 {
        sx = 0;
    }
    if sy < 0 {
        sy = 0;
    }
    if sx > 0x200 {
        sx = 0x200;
    }
    if sy > 0x200 {
        sy = 0x200;
    }
    let scale = fx.mem.f(r + 8);
    let fx0 = ee::div(ee::from_i32(sx), 512.0);
    let fx1 = ee::sub(1.0, fx0);
    let fy0 = ee::div(ee::from_i32(sy), 512.0);
    let fy1 = ee::sub(1.0, fy0);
    let (fx0, fx1, fy0, fy1) = (ee::mul(fx0, scale), ee::mul(fx1, scale), ee::mul(fy0, scale), ee::mul(fy1, scale));
    let d = sp + 0x218;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let flags = fx.mem.read_u32(r);
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
    for (o, k) in [(0x228u32, 6u32), (0x229, 5), (0x22A, 4), (0x22B, 7)] {
        let b = fx.mem.read_u8(r + k);
        fx.mem.write_u8(sp + o, b);
    }
    fx.mem.write_u32(sp + 0x22C, 0x3F80_0000);
    // a0..a3 left by 0x207E20: the descriptor, the blend, the flags | 0x200,
    // the flags.
    fx.texflush([d, alpha as u32, 0x226, 0x26]);
    let h = fx.mem.i16(r + 0xE);
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x258, sp + 0x260);
    fx.texflush(regs);
    let (u0, v0) = (sx << 4, sy << 4);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let (xy0, uv1, xy1) = (sp + 0x238, sp + 0x240, sp + 0x248);
    let mut acc = [0i32; 4];
    let mut i = 0i32;
    while i < fx.mem.i16(r + 0xC) {
        let mut ybase = 0x700u16;
        let (au, av) = (acc[0] << 4, acc[1] << 4);
        let (bu, bv) = ((0x200 - acc[2]) << 4, (0x200 - acc[3]) << 4);
        for j in 0..2 {
            let mut x = 0x700u16;
            let ey = (i32::from(ybase) << 4) - 8;
            for k in 0..2 {
                let px = fx.mem.read_u16(sp + 0x270);
                fx.mem.write_u16(xy0, px);
                let py = fx.mem.read_u16(sp + 0x272);
                fx.mem.write_u16(xy0 + 2, py);
                fx.mem.write_u8(xy0 + 7, 0);
                fx.mem.write_u16(xy1, ((i32::from(x) << 4) - 8) as u16);
                fx.mem.write_u16(xy1 + 2, ey as u16);
                fx.mem.write_u8(xy1 + 7, 0);
                let u1 = if k == 0 { au } else { bu };
                let v1 = if j == 0 { av } else { bv };
                for (a, uv) in [(sp + 0x230, u0), (sp + 0x232, v0), (uv1, u1), (uv1 + 2, v1)] {
                    let w = fx.mem.read_u16(a);
                    fx.mem.write_u16(a, (w & 0xC000) | (uv & 0x3FFF) as u16);
                }
                if bit(fx.mem.read_u32(r), 0x13) {
                    set_z(fx, &[xy1, xy0], 0xFF_FFFF);
                    fx.submit(d, 0, 0);
                } else {
                    let h = fx.mem.i16(r + 0xE);
                    let rad = unit_radius(fx, h);
                    let p = fx.mem.read_u32(sp + 0x19C);
                    let z = depth_key(fx, p, ee::neg(rad));
                    set_z(fx, &[xy1, xy0], u64::from(z));
                    let h = fx.mem.i16(r + 0xE);
                    let rad = unit_radius(fx, h);
                    let p = fx.mem.read_u32(sp + 0x19C);
                    let w = depth_w(fx, p, ee::neg(rad));
                    fx.submit(d, 0, ee::fixunssfsi(w) as i32);
                }
                x = x.wrapping_add(0x200);
            }
            ybase = ybase.wrapping_add(0x200);
        }
        for (k, (f, off)) in [(fx0, 0x600u32), (fy0, 0x614), (fx1, 0x628), (fy1, 0x63C)].into_iter().enumerate() {
            fx.fix_residue(f, at + off);
            acc[k] = acc[k].wrapping_add(ee::fixsfsi(f));
        }
        i += 1;
    }
    for (k, o) in [0x120u32, 0x130, 0x140, 0x150].into_iter().enumerate() {
        fx.mem.set32(sp + o, acc[k]);
    }
    fx.mem.set32(sp + 0xD0, i);
    fx.layer_end(layer, crate::exe::OT, 0);
    fx.leave(0x280);
    0
}

/// The UV halfword at `a` set to `v` (the top two bits kept).
fn set_uv(fx: &mut Fx, a: u32, v: i32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v as u16 & 0x3FFF));
}

/// 7e5add6e0c53 [b001 0x1E00280 (card 35), b087 0x1E02220 (card 303);
/// 548 words] [F]: `quarters(r, v) -> 0` (-1 when `v`, the
/// unit's position, does not project: the layer is left open). The
/// group `r` as `sprites`' (+0 flags, +4 colour, +0xC layer count, +0xE
/// the unit's handle; no scale). In a packet layer (0x2078A0): `v`
/// projected through a fresh transform (0x103890 / 0x1038D0 at sp + 0xF0;
/// XYZ sp + 0x1C0, w sp + 0x1DC); `sprites`' descriptor at sp + 0x168,
/// TEXFLUSH, the unit drawn into the target (0x1F8270 filter 0), TEXFLUSH;
/// then +0xC layers of four sprites splitting the window at the unit's
/// screen point (sx, sy): x spans [0x700, sx] / [sx, 2 sx - 0x700] when sx
/// < 0x800, else [2 sx - 0x900, sx] / [sx, 0x900]; y the other way round
/// (the 0x900 side when sy < 0x800). Each sprite's UVs are its window
/// rectangle less 0x700 (the left / top one cleared when past the right /
/// bottom one, the right / bottom ones capped at 0x100 when over 0x100),
/// the side away from the unit pulled in by 8 a layer. Z far (0x13) or
/// the unit's depth moved its radius toward the camera, as `sprites`'.
/// Then the layer linked into the ordering table at key 0. A 0x1E0 frame.
pub fn quarters(fx: &mut Fx, r: u32, v: u32) -> i32 {
    let sp = fx.enter(0x1E0);
    fx.mem.write_u32(sp + 0xE0, r);
    fx.mem.write_u32(sp + 0xDC, v);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xAC, layer);
    fx.xform_init(sp + 0xF0);
    fx.xform_world(sp + 0xF0);
    let m = fx.read_m4(sp + 0x120);
    if !fx.project_at(v, &m, sp + 0x1C0, sp + 0x1DC) {
        fx.leave(0x1E0);
        return -1;
    }
    let d = sp + 0x168;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u32(sp + 0xC0, d + 2);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let flags = fx.mem.read_u32(r);
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
    for (o, k) in [(0x178u32, 6u32), (0x179, 5), (0x17A, 4), (0x17B, 7)] {
        let b = fx.mem.read_u8(r + k);
        fx.mem.write_u8(sp + o, b);
    }
    fx.mem.write_u32(sp + 0x17C, 0x3F80_0000);
    fx.texflush([d, alpha as u32, 0x226, 0x26]);
    let h = fx.mem.i16(r + 0xE);
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x1A8, sp + 0x1B0);
    fx.texflush(regs);
    let (xy0, xy1) = (sp + 0x188, sp + 0x198);
    let (u0, v0, u1, v1) = (sp + 0x1C8, sp + 0x1CA, sp + 0x1D0, sp + 0x1D2);
    let mut inset = 0i32;
    let mut i = 0i32;
    fx.mem.set32(sp + 0xB0, 0);
    while i < fx.mem.i16(r + 0xC) {
        for j in 0..2i32 {
            for k in 0..2i32 {
                let sx = i32::from(fx.mem.read_u16(sp + 0x1C0)) >> 4;
                let (x0, x1) = if sx < 0x800 {
                    let a = sx - 0x700;
                    let p = fx.mult(a, k);
                    (p + 0x700, a.wrapping_mul(k + 1) + 0x700)
                } else {
                    let a = 0x900 - sx;
                    let q = a.wrapping_mul(k);
                    let p = fx.mult(a, k - 1);
                    (sx + p, sx + q)
                };
                fx.mem.write_u16(xy0, (x0 << 4) as u16);
                fx.mem.write_u16(xy1, (x1 << 4) as u16);
                let sy = i32::from(fx.mem.read_u16(sp + 0x1C2)) >> 4;
                let (y0, y1) = if sy < 0x800 {
                    let a = 0x900 - sy;
                    let q = a.wrapping_mul(j);
                    let p = fx.mult(a, j - 1);
                    (sy + p, sy + q)
                } else {
                    let a = sy - 0x700;
                    let p = fx.mult(a, j);
                    (p + 0x700, a.wrapping_mul(j + 1) + 0x700)
                };
                fx.mem.write_u16(xy0 + 2, (y0 << 4) as u16);
                fx.mem.write_u16(xy1 + 2, (y1 << 4) as u16);
                fx.mem.write_u8(xy0 + 7, 0);
                fx.mem.write_u8(xy1 + 7, 0);
                let fl = fx.mem.read_u32(sp + 0xC0);
                let f = fx.mem.read_u16(fl);
                fx.mem.write_u16(fl, f | 0x800);
                for (a, src) in [(u0, xy0), (v0, xy0 + 2), (u1, xy1), (v1, xy1 + 2)] {
                    let s = i32::from(fx.mem.read_u16(src)) >> 4;
                    set_uv(fx, a, s - 0x700);
                }
                let px = |fx: &mut Fx, a: u32| (i32::from(fx.mem.read_u16(a)) & 0x3FFF) >> 4;
                if px(fx, u1) < px(fx, u0) {
                    set_uv(fx, u0, 0);
                }
                if px(fx, v1) < px(fx, v0) {
                    set_uv(fx, v0, 0);
                }
                for a in [u1, v1] {
                    if px(fx, a) >= 0x101 {
                        set_uv(fx, a, 0x1000);
                    }
                }
                let du0 = if k == 0 { inset } else { 0 };
                let dv0 = if j == 0 { inset } else { 0 };
                let du1 = if k == 1 { inset.wrapping_neg() } else { 0 };
                let dv1 = if j == 1 { inset.wrapping_neg() } else { 0 };
                for (dst, src, dd) in [(sp + 0x180, u0, du0), (sp + 0x182, v0, dv0), (sp + 0x190, u1, du1), (sp + 0x192, v1, dv1)] {
                    let s = i32::from(fx.mem.read_u16(src)) & 0x3FFF;
                    set_uv(fx, dst, (s + dd) << 4);
                }
                if bit(fx.mem.read_u32(r), 0x13) {
                    set_z(fx, &[xy1, xy0], 0xFF_FFFF);
                    fx.submit(d, 0, 0);
                } else {
                    let h = fx.mem.i16(r + 0xE);
                    let rad = unit_radius(fx, h);
                    let z = depth_key(fx, v, ee::neg(rad));
                    set_z(fx, &[xy1, xy0], u64::from(z));
                    let h = fx.mem.i16(r + 0xE);
                    let rad = unit_radius(fx, h);
                    let w = depth_w(fx, v, ee::neg(rad));
                    fx.submit(d, 0, ee::fixunssfsi(w) as i32);
                }
            }
        }
        i += 1;
        inset += 8;
        fx.mem.set32(sp + 0xB0, i);
    }
    fx.layer_end(layer, crate::exe::OT, 0);
    fx.leave(0x1E0);
    0
}
