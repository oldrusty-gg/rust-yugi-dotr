//! Scrolling textured quads of the effect library: four world corners projected, the texture's S / T scrolled by
//! the elapsed count and divided by each corner's w (perspective STQ).
use super::exe::{camera_vp_into, countdown, cvt_u32};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx};

/// 61d5e1542057 [b706 (card 75) 0x1E05130, 320 words] [F]: `quads(at,
/// recs, ot, n) -> 1` steps and draws `n` records (0x50 bytes; a 0x1D0
/// frame, f20..f22 saved, n / ot at sp + 0xB4 / 0xB8). Per record:
/// countdown 0x101AF0; the texture (5, +8); the camera (0x20E2B0, sp +
/// 0xC0); the corners +0x20 + 12 k projected (0x204D80, the result not
/// checked) into sp + 0x110 + 8 k with the w's at sp + 0x130 + 4 k. With
/// e = length - count: s0 = +0x14 e, t = +0x18 e, each above 1 reduced to
/// its fraction (0x1A2B80); u from s0 + 4 +0xC (corners 0 / 2) to s0 + 5
/// +0xC (1 / 3), v from 1 + t (0 / 1) to t (2 / 3), all over the corner's
/// w with Q = 1 / w (a prim 0x34 strip at sp + 0x140, ALPHA 0x48); colour
/// +0x1E / +0x1D / +0x1C, alpha +0x1F (x count / 8 over the last 8 unless
/// +4 0x40); TEX0 / TEX1 (0x207E50, filter 1). The key: 0 with Z 0xFFFFFE
/// (+4 0x80), else the corners' mean w through 0x1A2C70 twice (back to a
/// float in between).
pub fn quads(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1D0);
    fx.mem.set32(sp + 0xB4, n);
    fx.mem.write_u32(sp + 0xB8, ot);
    let mut p = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            p += 0x50;
        }
        one(fx, at, sp, p);
        i += 1;
    }
    fx.leave(0x1D0);
    1
}

fn one(fx: &mut Fx, at: u32, sp: u32, p: u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    let id = fx.mem.u8(p + 8);
    let tex = fx.texture(5, id);
    camera_vp_into(fx, sp + 0xC0);
    let m = fx.read_m4(sp + 0xC0);
    for k in 0..4u32 {
        for c in 0..3u32 {
            let w = fx.mem.read_u32(p + 0x20 + 12 * k + 4 * c);
            fx.mem.write_u32(sp + 0x100 + 4 * c, w);
        }
        fx.project_at(sp + 0x100, &m, sp + 0x110 + 8 * k, sp + 0x130 + 4 * k);
    }
    let e = fx.mem.u8(p + 2) - fx.mem.u8(p);
    let mut f22 = ee::mul(fx.mem.f(p + 0x14), ee::from_i32(e));
    let f20 = fx.mem.f(p + 0xC);
    if !ee_le(f22, 1.0) {
        fx.fix_residue(f22, at + 0x12C);
        let t = ee::fixsfsi(f22);
        f22 = ee::sub(f22, ee::from_i32(t));
    }
    let mut f21 = ee::mul(fx.mem.f(p + 0x18), ee::from_i32(e));
    if !ee_le(f21, 1.0) {
        fx.fix_residue(f21, at + 0x174);
        let t = ee::fixsfsi(f21);
        f21 = ee::sub(f21, ee::from_i32(t));
    }
    let f7 = ee::add(f22, ee::mul(f20, 4.0));
    let d = sp + 0x140;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    let f9 = ee::add(f22, ee::mul(f20, 5.0));
    let w = |fx: &mut Fx, k: u32| fx.mem.f(sp + 0x130 + 4 * k);
    let (w0, w1, w2, w3) = (w(fx, 0), w(fx, 1), w(fx, 2), w(fx, 3));
    let t1 = ee::add(1.0, f21);
    for (o, v) in [
        (0x158u32, ee::div(f7, w0)),
        (0x154, ee::div(1.0, w0)),
        (0x16C, ee::div(1.0, w1)),
        (0x184, ee::div(1.0, w2)),
        (0x19C, ee::div(1.0, w3)),
        (0x15C, ee::div(t1, w0)),
        (0x174, ee::div(t1, w1)),
        (0x170, ee::div(f9, w1)),
        (0x188, ee::div(f7, w2)),
        (0x18C, ee::div(f21, w2)),
        (0x1A0, ee::div(f9, w3)),
        (0x1A4, ee::div(f21, w3)),
    ] {
        fx.mem.setf(sp + o, v);
    }
    for (from, to) in [(0x110u32, 0x160u32), (0x118, 0x178), (0x120, 0x190), (0x128, 0x1A8)] {
        let v = fx.mem.read_u64(sp + from);
        fx.mem.write_u64(sp + to, v);
    }
    fx.desc_alpha(d, 0x48);
    let c = fx.mem.u8(p);
    let alpha = if fx.mem.u8(p + 4) & 0x40 == 0 && c < 8 {
        let a = fx.mem.u8(p + 0x1F) >> 3;
        fx.mult(a, c)
    } else {
        fx.mem.u8(p + 0x1F)
    };
    for (src, k) in [(0x1Eu32, 0u32), (0x1D, 1), (0x1C, 2)] {
        let v = fx.mem.read_u8(p + src);
        for o in [0x198u32, 0x180, 0x168, 0x150] {
            fx.mem.write_u8(sp + o + k, v);
        }
    }
    for o in [0x19Bu32, 0x183, 0x16B, 0x153] {
        fx.mem.set8(sp + o, alpha);
    }
    for o in [0x1AFu32, 0x197, 0x17F, 0x167] {
        fx.mem.write_u8(sp + o, 0);
    }
    fx.desc_texture(d, tex, 1);
    let key_w = if fx.mem.u8(p + 4) & 0x80 != 0 {
        let mut z = 0xFF_FFFE;
        for o in [0x1A8u32, 0x190, 0x178, 0x160] {
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
        }
        0.0
    } else {
        let s = ee::add(w0, w1);
        let s = ee::add(w2, s);
        let s = ee::add(w3, s);
        let v = ee::div(s, 4.0);
        fx.fix_residue(v, at + 0x454);
        cvt_u32(ee::fixunssfsi(v))
    };
    fx.fix_residue(key_w, at + 0x490);
    let key = ee::fixunssfsi(key_w) as i32;
    let ot = fx.mem.read_u32(sp + 0xB8);
    fx.submit(d, ot, key);
}
