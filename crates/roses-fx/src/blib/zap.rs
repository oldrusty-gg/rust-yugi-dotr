//! A jittered spline line between two points, grown segment by segment,
//! of the effect library.
use super::arc::resample;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};

/// 8ffeb0d8bbc1 [b740 0x1E01A10 / b741 0x1E02930, 328 words] [F]:
/// `zap(ot, g, x, a, b) -> 0` (b in t0). A path of 8 points (at sp +
/// 0xC0, 0x102550's layout) from `a` to `b`, the inner six at k / 7 of the
/// way moved by (rand % 2s - s) / 10 per axis (s = +0x2A), resampled to 28
/// points (0x102550) and projected through `x` + 0x30 (0x204D80; XYZ at sp
/// + 0x4B0 + 8 k, w at sp + 0x590 + 4 k; bit k of a mask when on screen,
/// 0x101600). Then each of the 27 segments up to +0x2C (all with flag
/// 0x1A) whose ends are both visible drawn as a gouraud line (descriptor
/// sp + 0x608: 4 values RGBAQ / XYZF2 x 2, flags 0x11 | 0x400, ALPHA 0x48,
/// colour +8 (bytes 2, 1, 0, 3) Q 1): flag 0x13 far Z key 0, else key the
/// mean w (0x1A2C70). +0x2C counts up to +0x28 (at most 0x1C), then 0.
pub fn zap(fx: &mut Fx, ot: u32, g: u32, x: u32, a: u32, b: u32) -> i32 {
    let sp = fx.enter(0x640);
    if fx.mem.i16(g + 0x28) >= 0x1D {
        fx.mem.set16(g + 0x28, 0x1C);
    }
    let d = sp + 0x608;
    fx.mem.write_u16(d + 2, 0x11);
    fx.mem.write_u16(d, 4);
    fx.mem.write_u64(d + 8, 0x4141);
    fx.mem.write_u32(d + 4, 0);
    fx.desc_alpha(d, crate::fxlib::ALPHA_ADD);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x400);
    for (o, s) in [(0u32, 0xAu32), (1, 9), (2, 8), (3, 0xB)] {
        let c = fx.mem.read_u8(g + s);
        fx.mem.write_u8(sp + 0x628 + o, c);
        fx.mem.write_u8(sp + 0x618 + o, c);
    }
    fx.mem.write_u32(sp + 0x62C, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x61C, 0x3F80_0000);
    let p = sp + 0xC0;
    let delta: [f32; 3] = std::array::from_fn(|c| ee::sub(fx.mem.f(b + 4 * c as u32), fx.mem.f(a + 4 * c as u32)));
    for c in 0..3u32 {
        let v = fx.mem.read_u32(a + 4 * c);
        fx.mem.write_u32(p + 4 + 0x20 * c, v);
    }
    let tenth = f32::from_bits(0x3DCC_CCCD);
    for k in 1..7u32 {
        let t = ee::div(ee::from_i32(k as i32), 7.0);
        for c in 0..3u32 {
            let s = i32::from(fx.mem.i16(g + 0x2A));
            let r = fx.rand();
            let (_, h) = fx.div(r, 2 * s);
            let v = ee::add(fx.mem.f(a + 4 * c), ee::mul(delta[c as usize], t));
            let v = ee::add(v, ee::mul(tenth, ee::from_i32(h.wrapping_sub(s))));
            fx.mem.setf(p + 4 + 0x20 * c + 4 * k, v);
        }
    }
    for c in 0..3u32 {
        let v = fx.mem.read_u32(b + 4 * c);
        fx.mem.write_u32(p + 0x20 + 0x20 * c, v);
    }
    fx.mem.set32(p, 8);
    resample(fx, p);
    let m = fx.read_m4(x + 0x30);
    let mut seen = 0u32;
    for k in 0..0x1Cu32 {
        let v = sp + 0x2F0 + 16 * k;
        for c in 0..3u32 {
            let w = fx.mem.read_u32(p + 0x64 + 16 * k + 4 * c);
            fx.mem.write_u32(v + 4 * c, w);
        }
        let xyz = sp + 0x4B0 + 8 * k;
        if fx.project_at(v, &m, xyz, sp + 0x590 + 4 * k) && Fx::on_screen(fx.mem.read_u64(xyz)) {
            seen |= 1 << k;
        }
    }
    let (x0, x1) = (sp + 0x620, sp + 0x630);
    for k in 0..0x1Bu32 {
        if !bit(fx.mem.read_u32(g + 4), 0x1A) && i32::from(fx.mem.i16(g + 0x2C)) < k as i32 {
            continue;
        }
        let xyz = sp + 0x4B0 + 8 * k;
        for (dst, src) in [(x0, xyz), (x1, xyz + 8), (x0 + 2, xyz + 2), (x1 + 2, xyz + 0xA)] {
            let h = fx.mem.read_u16(src);
            fx.mem.write_u16(dst, h);
        }
        if !bit(seen, k) || !bit(seen, k + 1) {
            continue;
        }
        if bit(fx.mem.read_u32(g + 4), 0x13) {
            fx.far_z(&[x0, x1]);
            fx.submit(d, ot, 0);
        } else {
            let z = fx.xyz_z(xyz);
            fx.set_xyz_z(x0, z);
            let z = fx.xyz_z(xyz + 8);
            fx.set_xyz_z(x1, z);
            let w = ee::add(fx.mem.f(sp + 0x590 + 4 * k), fx.mem.f(sp + 0x594 + 4 * k));
            let key = ee::fixunssfsi(ee::div(w, 2.0)) as i32;
            fx.submit(d, ot, key);
        }
    }
    let n = fx.mem.i16(g + 0x2C) + 1;
    fx.mem.set16(g + 0x2C, i32::from(n));
    if fx.mem.i16(g + 0x28) < fx.mem.i16(g + 0x2C) {
        fx.mem.set16(g + 0x2C, 0);
    }
    fx.leave(0x640);
    0
}
