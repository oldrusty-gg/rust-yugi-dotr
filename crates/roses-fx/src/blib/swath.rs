//! A glowing swath between two point trails, drawn in a packet layer
//!, hand-ported from b467 (card 282) [F]:
//! a31ec1233f93 `push` (84 words, 13 B records) and 4e99f772a60f `draw`
//! (512 words, 13 B records; users b067 b025 b360 b467 b147 b074 b637
//! b036 b464 b635 b298 + 2).
//!
//! A swath record `r`: +0 the texture (RH2), +4 flags (bit 0 integer UVs,
//! 0x14..0x17 corner k projected), +8 the colour (B, G, R, A; +0xB the
//! alpha the strip fades from), +0xC the most points less 3 (i16), +0xE
//! the points less 3 (i16), +0x10 + 0x10 k / +0x90 + 0x10 k (k < 8) the two
//! trails (vec4, newest first), +0x110 the cell (u, v, w, h as i16 with
//! integer UVs, else s, t, w, h as f32), +0x120 / +0x124 the resampled
//! trails (4 (n + 2) vec4 each).
use crate::blib::arc;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};

fn copy_words(fx: &mut Fx, to: u32, from: u32, n: u32) {
    for k in 0..n {
        let v = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, v);
    }
}

/// a31ec1233f93 [b467 0x1E009F0] [F]: `push(r, a, b) -> 0`. With no
/// points yet (+0xE = 0): point 2 = point 0, point 0 = `a` (xyz) and point
/// 1 their midpoint (0x1036E0), the same for the second trail with `b`.
/// Else both trails shift up one (points 7..1 = 6..0, vec4) and point 0 =
/// `a` / `b` (xyz, w kept). A 0x30 frame.
pub fn push(fx: &mut Fx, r: u32, a: u32, b: u32) -> i32 {
    fx.enter(0x30);
    if fx.mem.i16(r + 0xE) == 0 {
        copy_words(fx, r + 0x30, r + 0x10, 3);
        copy_words(fx, r + 0x10, a, 3);
        fx.midpoint(r + 0x10, r + 0x30, r + 0x20);
        copy_words(fx, r + 0xB0, r + 0x90, 3);
        copy_words(fx, r + 0x90, b, 3);
        fx.midpoint(r + 0x90, r + 0xB0, r + 0xA0);
    } else {
        for k in (0..7u32).rev() {
            let v = r + 0x10 * k;
            copy_words(fx, v + 0x20, v + 0x10, 4);
            copy_words(fx, v + 0xA0, v + 0x90, 4);
            if k == 0 {
                copy_words(fx, v + 0x10, a, 3);
                copy_words(fx, v + 0x90, b, 3);
            }
        }
    }
    fx.leave(0x30);
    0
}

/// One trail resampled: its n = +0xE + 3 points into 0x102550's path at
/// `p` (x / y / z rows), resampled, the `samples` samples into *(r + out).
fn resample_into(fx: &mut Fx, r: u32, p: u32, src: u32, out: u32, samples: i32) {
    let mut k = 0i32;
    while k < fx.mem.i16(r + 0xE) as i32 + 3 {
        let (s, d) = (src + 0x10 * k as u32, p + 4 * k as u32);
        for (o, row) in [(0u32, 4u32), (4, 0x24), (8, 0x44)] {
            let v = fx.mem.read_u32(s + o);
            fx.mem.write_u32(d + row, v);
        }
        k += 1;
    }
    arc::resample(fx, p);
    let mut k = 0i32;
    while k < samples {
        for o in [0u32, 4, 8] {
            let to = fx.mem.read_u32(r + out).wrapping_add(0x10 * k as u32 + o);
            let v = fx.mem.read_u32(p + 0x64 + 0x10 * k as u32 + o);
            fx.mem.write_u32(to, v);
        }
        k += 1;
    }
}

/// 4e99f772a60f [b467 0x1E00B40] [F]: `draw(r, x) -> 0` in a packet layer
/// (0x2078A0 .. 0x207910, closed into the ordering table 0x206E40 at key
/// 0). Both trails resampled (0x102550 path at sp + 0xB0) into +0x120 /
/// +0x124; a strip descriptor at sp + 0x310 (flags 0x34, ALPHA 0x48, FIX
/// 0x80, the colour by `colours4`, the cell's corners as integer UVs with
/// flag 0x800 or as ST); then for each of the s = 4 (n + 2) - 1 segments
/// k, with step = +0xB / 4 (n + 2) (`div`) and a = step x k (u8): two
/// quads, (mid k, mid k + 1, A k, A k + 1) and the same with B, mid the
/// trails' midpoint (0x1036E0); alphas (k = 0 ? 0 : +0xB - a), (k = s - 1
/// ? 0 : +0xB - a - step), 0, 0; corners through x + 0x30 (0x204D80, w at
/// sp + 0x300..0x30C, corner bits 0x14..0x17), drawn with the texture
/// (filter 0) keyed by the mean w (0x1A2C70) and no ordering table when
/// all four project; corner bits cleared. Then +0xE + 1 while +0xE + 3 <
/// +0xC. A 0x3A0 frame.
pub fn draw(fx: &mut Fx, r: u32, x: u32) -> i32 {
    let sp = fx.enter(0x3A0);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xAC, layer);
    let n = fx.mem.i16(r + 0xE) as i32;
    let segments = (n + 2) << 2;
    let alpha = i32::from(fx.mem.read_u8(r + 0xB));
    let step = fx.div(alpha, segments).0;
    fx.mem.write_u32(sp + 0xB0, (n + 3) as u32);
    resample_into(fx, r, sp + 0xB0, r + 0x10, 0x120, segments);
    resample_into(fx, r, sp + 0xB0, r + 0x90, 0x124, segments);
    let d = sp + 0x310;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.colours4(d, r + 8);
    let flags = fx.mem.read_u32(r + 4);
    if bit(flags, 0) {
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        let h = |fx: &mut Fx, o: u32| fx.mem.i16(r + o) as i32;
        let (u, v) = (h(fx, 0x110), h(fx, 0x112));
        fx.set_uv(sp + 0x328, u);
        fx.set_uv(sp + 0x32A, v);
        let (u, v) = (h(fx, 0x110), h(fx, 0x112) + h(fx, 0x116) - 1);
        fx.set_uv(sp + 0x340, u);
        fx.set_uv(sp + 0x342, v);
        let (u, v) = (h(fx, 0x110) + h(fx, 0x114) - 1, h(fx, 0x112));
        fx.set_uv(sp + 0x358, u);
        fx.set_uv(sp + 0x35A, v);
        let (u, v) = (h(fx, 0x110) + h(fx, 0x114) - 1, h(fx, 0x112) + h(fx, 0x116) - 1);
        fx.set_uv(sp + 0x370, u);
        fx.set_uv(sp + 0x372, v);
    } else {
        let f = |fx: &mut Fx, o: u32| fx.mem.f(r + o);
        let (s, t) = (f(fx, 0x110), f(fx, 0x114));
        fx.mem.setf(sp + 0x328, s);
        fx.mem.setf(sp + 0x32C, t);
        fx.mem.setf(sp + 0x340, s);
        let v = ee::add(f(fx, 0x114), f(fx, 0x11C));
        fx.mem.setf(sp + 0x344, v);
        let v = ee::add(f(fx, 0x110), f(fx, 0x118));
        fx.mem.setf(sp + 0x358, v);
        let v = f(fx, 0x114);
        fx.mem.setf(sp + 0x35C, v);
        let v = ee::add(f(fx, 0x110), f(fx, 0x118));
        fx.mem.setf(sp + 0x370, v);
        let v = ee::add(f(fx, 0x114), f(fx, 0x11C));
        fx.mem.setf(sp + 0x374, v);
    }
    if segments - 1 > 0 {
        fx.mult(0, step);
        let mut a = 0u32;
        let mut k = 0i32;
        while k < segments - 1 {
            let alpha = u32::from(fx.mem.read_u8(r + 0xB));
            fx.mem.write_u8(sp + 0x323, if k == 0 { 0 } else { alpha.wrapping_sub(a) as u8 });
            let alpha = u32::from(fx.mem.read_u8(r + 0xB));
            fx.mem.write_u8(sp + 0x33B, if k == segments - 2 { 0 } else { alpha.wrapping_sub((step as u32).wrapping_add(a)) as u8 });
            fx.mem.write_u8(sp + 0x36B, 0);
            fx.mem.write_u8(sp + 0x353, 0);
            let (s2, s3) = (0x10 * k as u32, 0x10 * (k + 1) as u32);
            let (pa, pb) = (fx.mem.read_u32(r + 0x120), fx.mem.read_u32(r + 0x124));
            fx.midpoint(pa.wrapping_add(s2), pb.wrapping_add(s2), sp + 0x2E0);
            let (pa, pb) = (fx.mem.read_u32(r + 0x120), fx.mem.read_u32(r + 0x124));
            fx.midpoint(pa.wrapping_add(s3), pb.wrapping_add(s3), sp + 0x2F0);
            for side in [0x120u32, 0x124] {
                let m = fx.read_m4(x + 0x30);
                if fx.project_at(sp + 0x2E0, &m, sp + 0x330, sp + 0x300) {
                    fx.flag(r + 4, 0x14);
                }
                let m = fx.read_m4(x + 0x30);
                if fx.project_at(sp + 0x2F0, &m, sp + 0x348, sp + 0x304) {
                    fx.flag(r + 4, 0x15);
                }
                let p = fx.mem.read_u32(r + side).wrapping_add(s2);
                let m = fx.read_m4(x + 0x30);
                if fx.project_at(p, &m, sp + 0x360, sp + 0x308) {
                    fx.flag(r + 4, 0x16);
                }
                let p = fx.mem.read_u32(r + side).wrapping_add(s3);
                let m = fx.read_m4(x + 0x30);
                if fx.project_at(p, &m, sp + 0x378, sp + 0x30C) {
                    fx.flag(r + 4, 0x17);
                }
                let flags = fx.mem.read_u32(r + 4);
                if (0x14..0x18).all(|b| bit(flags, b)) {
                    let rh2 = fx.mem.read_u32(r);
                    fx.desc_texture(d, rh2, 0);
                    let s = ee::add(fx.mem.f(sp + 0x300), fx.mem.f(sp + 0x304));
                    let s = ee::add(fx.mem.f(sp + 0x308), s);
                    let s = ee::add(fx.mem.f(sp + 0x30C), s);
                    let key = ee::fixunssfsi(ee::div(s, 4.0)) as i32;
                    fx.submit(d, 0, key);
                }
                let f = fx.mem.read_u32(r + 4) & 0xFF0F_FFFF;
                fx.mem.write_u32(r + 4, f);
            }
            a = a.wrapping_add(step as u32) & 0xFF;
            k += 1;
        }
    }
    let layer = fx.mem.read_u32(sp + 0xAC);
    fx.layer_end(layer, crate::exe::OT, 0);
    let n = fx.mem.i16(r + 0xE);
    if (n as i32 + 3) < fx.mem.i16(r + 0xC) as i32 {
        fx.mem.set16(r + 0xE, n + 1);
    }
    fx.leave(0x3A0);
    0
}
