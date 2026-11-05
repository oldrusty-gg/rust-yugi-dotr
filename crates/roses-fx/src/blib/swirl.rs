//! Swirls of the effect library: spiral arms of
//! gouraud quads about a centre.
use super::exe::{camera_vp_into, countdown, libm_cosf, mul_mem, node_source, rot_trans_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx};

const PI: u32 = 0x4049_0FDB;

/// a9561e806c5e [2 B records; b091 0x1E04790, b437 0x1E04740, 636 words]
/// [F]: `arms(at, recs, ot, n) -> 1` steps and draws `n` records (0x5C
/// bytes; a 0x210 frame, the descriptor at sp + 0x198: RGBAQ / XYZ2 x 4, Q
/// 1.0, ALPHA 0x48 by 0x207E20). Per record: countdown 0x101AF0; with
/// end +0x10 <= start +0xC ("down") both -= the spin +0x18, else both +=;
/// position +0x2C += velocity +0x3C; while +9 < the count, radius +0x1C +=
/// +0x20; the position from a node with +5 (0x101BA0); the camera
/// (0x20E2B0 into sp + 0x130) x translate / rotate (+0x4C, 0x20C0B0) by
/// 0x20B370. The centre projects (0x204D80 of 0, into sp + 0x1F0 / w sp +
/// 0x20C) or the record is skipped. One arm per angle a from the start to
/// the end (step +0x14 x +8, down: from +0x10 up to +0xC): +0xA rings,
/// ring k at radius (k + 1) +0x1C / +0xA from angle a -/+ +0x14 k (down /
/// up) to that + +0x14 (libm cosf x, sinf z), each quad from the previous
/// ring's two points (the centre at first); skipped when a point fails to
/// project. Colour inner +0x24.. towards outer +0x28.. by ring (0x1A2B80;
/// with flag 1 the two colours whole, swapped for "down", the inner alpha
/// halved), alpha +0x27 less the alpha step x k / k + 1 (x count / 8 over
/// the last 8), the last ring's outer alpha 0; key the mean w (0x1A2C70).
pub fn arms(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    arms_by(fx, at, recs, ot, n, &ARMS)
}

/// 1c4c37ddd46b [3 B records; card 454's 0x1E01190, 640 words] (helper
/// A19, 1 Nov 2026) [F]: [`arms`] recompiled in a 0x1C0 frame (n / the
/// counter in registers, no slots; the down flag in s2), its locals moved
/// ([`Layout`]) and no centre copy at the old sp + 0x200.
pub fn arms_c(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    arms_by(fx, at, recs, ot, n, &ARMS_C)
}

/// An arms compile: the frame, whether n / the counter have slots, the
/// locals (camera, turn, descriptor, the projected point, the centre's
/// XYZ / w, the alpha pair, the four w), the old centre copy, and the
/// 0x1A2B80 / 0x1A2C70 return offsets (alphas k / k + 1, the three
/// colours at k (k + 1 is +0x2C), the key).
pub struct Layout {
    frame: u32,
    slots: bool,
    cam: u32,
    trs: u32,
    desc: u32,
    pt: u32,
    centre: u32,
    centre_w: u32,
    alpha: u32,
    w: u32,
    copy: Option<u32>,
    ra_alpha: [u32; 2],
    ra_colour: [u32; 3],
    ra_key: u32,
}

const ARMS: Layout = Layout {
    frame: 0x210,
    slots: true,
    cam: 0x130,
    trs: 0xF0,
    desc: 0x198,
    pt: 0x180,
    centre: 0x1F0,
    centre_w: 0x20C,
    alpha: 0x1F8,
    w: 0x170,
    copy: Some(0x200),
    ra_alpha: [0x668, 0x688],
    ra_colour: [0x7E4, 0x838, 0x88C],
    ra_key: 0x924,
};

const ARMS_C: Layout = Layout {
    frame: 0x1C0,
    slots: false,
    cam: 0xF0,
    trs: 0xB0,
    desc: 0x150,
    pt: 0x140,
    centre: 0x1A8,
    centre_w: 0x1BC,
    alpha: 0x1B0,
    w: 0x130,
    copy: None,
    ra_alpha: [0x680, 0x6B4],
    ra_colour: [0x804, 0x858, 0x8AC],
    ra_key: 0x940,
};

fn arms_by(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, l: &Layout) -> i32 {
    let sp = fx.enter(l.frame);
    if l.slots {
        fx.mem.set32(sp + 0xEC, n);
    }
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x5C;
        }
        if l.slots {
            fx.mem.set32(sp + 0xD0, i);
        }
        one(fx, at, sp, g, ot, l);
        i += 1;
    }
    if l.slots {
        fx.mem.set32(sp + 0xD0, i);
    }
    fx.leave(l.frame);
    1
}

/// 0x1A2B80 (float to integer) returning to `at + ra`, its frame left.
fn fix(fx: &mut Fx, at: u32, ra: u32, v: f32) -> i32 {
    fx.fix_residue(v, at + ra);
    ee::fixsfsi(v)
}

fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, l: &Layout) {
    if countdown(fx, g) == 0 {
        return;
    }
    let down = ee_le(fx.mem.f(g + 0x10), fx.mem.f(g + 0xC));
    for o in [0xCu32, 0x10] {
        let (a, s) = (fx.mem.f(g + o), fx.mem.f(g + 0x18));
        fx.mem.setf(g + o, if down { ee::sub(a, s) } else { ee::add(a, s) });
    }
    for o in [0x2Cu32, 0x30, 0x34] {
        let v = ee::add(fx.mem.f(g + o), fx.mem.f(g + o + 0x10));
        fx.mem.setf(g + o, v);
    }
    if fx.mem.u8(g + 9) < fx.mem.u8(g) {
        let v = ee::add(fx.mem.f(g + 0x1C), fx.mem.f(g + 0x20));
        fx.mem.setf(g + 0x1C, v);
    }
    node_source(fx, g, g + 0x2C);
    camera_vp_into(fx, sp + l.cam);
    rot_trans_into(fx, sp + l.trs, g + 0x4C, g + 0x2C);
    mul_mem(fx, sp + l.cam, sp + l.trs);
    let d = sp + l.desc;
    fx.desc_init(d, 8, 0x14, 0x4141_4141);
    for o in [0x44u32, 0x34, 0x24, 0x14] {
        fx.mem.write_u32(d + o, 0x3F80_0000);
    }
    fx.desc_alpha(d, 0x48);
    let (mut a, end) = if down { (fx.mem.f(g + 0x10), fx.mem.f(g + 0xC)) } else { (fx.mem.f(g + 0xC), fx.mem.f(g + 0x10)) };
    let width = fx.mem.f(g + 0x14);
    let step = ee::mul(width, ee::from_i32(fx.mem.u8(g + 8)));
    let rings = ee::from_i32(fx.mem.u8(g + 0xA));
    let dr = ee::div(fx.mem.f(g + 0x1C), rings);
    let fade = |fx: &mut Fx, c: u32| ee::div(ee::from_i32(fx.mem.u8(g + c) - fx.mem.u8(g + c + 4)), rings);
    let (dc0, dc1, dc2, da) = (fade(fx, 0x26), fade(fx, 0x25), fade(fx, 0x24), fade(fx, 0x27));
    for o in [8u32, 4, 0] {
        fx.mem.write_u32(sp + l.pt + o, 0);
    }
    let m = fx.read_m4(sp + l.cam);
    if !fx.project_at(sp + l.pt, &m, sp + l.centre, sp + l.centre_w) {
        return;
    }
    while ee_lt(a, end) {
        let c = fx.mem.read_u64(sp + l.centre);
        if let Some(o) = l.copy {
            fx.mem.write_u64(sp + o, c);
        }
        fx.mem.write_u64(d + 0x38, c);
        fx.mem.write_u64(d + 0x48, c);
        let w = fx.mem.read_u32(sp + l.centre_w);
        fx.mem.write_u32(sp + l.w + 0xC, w);
        fx.mem.write_u32(sp + l.w + 8, w);
        let mut k = 0i32;
        while k < fx.mem.u8(g + 0xA) {
            ring(fx, at, sp, g, ot, a, down, k, dr, [dc0, dc1, dc2, da], l);
            k += 1;
        }
        a = ee::add(a, step);
    }
}

#[allow(clippy::too_many_arguments)]
fn ring(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, a: f32, down: bool, k: i32, dr: f32, dc: [f32; 4], l: &Layout) {
    let d = sp + l.desc;
    let wv = sp + l.w;
    let v = fx.mem.read_u64(d + 0x38);
    fx.mem.write_u64(d + 0x18, v);
    let v = fx.mem.read_u32(wv + 8);
    fx.mem.write_u32(wv, v);
    let v = fx.mem.read_u64(d + 0x48);
    fx.mem.write_u64(d + 0x28, v);
    let v = fx.mem.read_u32(wv + 0xC);
    fx.mem.write_u32(wv + 4, v);
    let width = fx.mem.f(g + 0x14);
    let t = ee::mul(width, ee::from_i32(k));
    let ang = if down { ee::add(a, t) } else { ee::sub(a, t) };
    let r = ee::mul(ee::from_i32(k + 1), dr);
    let rad = ee::div(ee::mul(f32::from_bits(PI), ang), 180.0);
    let c = libm_cosf(fx, rad);
    fx.mem.setf(sp + l.pt, ee::mul(r, c));
    let s = fx.libm_sinf(rad);
    fx.mem.setf(sp + l.pt + 8, ee::mul(r, s));
    let m = fx.read_m4(sp + l.cam);
    if !fx.project_at(sp + l.pt, &m, d + 0x38, wv + 8) {
        return;
    }
    let rad = ee::div(ee::mul(f32::from_bits(PI), ee::add(ang, fx.mem.f(g + 0x14))), 180.0);
    let c = libm_cosf(fx, rad);
    fx.mem.setf(sp + l.pt, ee::mul(r, c));
    let rad = ee::div(ee::mul(f32::from_bits(PI), ee::add(ang, fx.mem.f(g + 0x14))), 180.0);
    let s = fx.libm_sinf(rad);
    fx.mem.setf(sp + l.pt + 8, ee::mul(r, s));
    let m = fx.read_m4(sp + l.cam);
    if !fx.project_at(sp + l.pt, &m, d + 0x48, wv + 0xC) {
        return;
    }
    let al = sp + l.alpha;
    let alpha = fx.mem.u8(g + 0x27);
    let t = ee::mul(dc[3], ee::from_i32(k));
    let a0 = alpha.wrapping_sub(fix(fx, at, l.ra_alpha[0], t));
    fx.mem.set32(al, a0);
    let a1 = alpha.wrapping_sub(fix(fx, at, l.ra_alpha[1], ee::add(1.0, t)));
    fx.mem.set32(al + 4, a1);
    let count = fx.mem.u8(g);
    if count < 8 {
        let a = fx.mem.i32(al) >> 3;
        let v = fx.mult(a, count);
        fx.mem.set32(al, v);
        let v = (fx.mem.i32(al + 4) >> 3).wrapping_mul(count);
        fx.mem.set32(al + 4, v);
    }
    let v = fx.mem.read_u8(al);
    fx.mem.write_u8(d + 0x13, v);
    fx.mem.write_u8(d + 0x23, v);
    let v = fx.mem.read_u8(al + 4);
    fx.mem.write_u8(d + 0x33, v);
    fx.mem.write_u8(d + 0x43, v);
    if fx.mem.u8(g + 4) & 1 != 0 {
        let half = fx.mem.i32(al) >> 1;
        // Down: the outer colour on the far pair, the inner on the near.
        let (far, near) = if down { (0x2Au32, 0x26u32) } else { (0x26, 0x2A) };
        if down {
            fx.mem.set8(d + 0x43, half);
            fx.mem.set8(d + 0x23, half);
        } else {
            fx.mem.set8(d + 0x33, half);
            fx.mem.set8(d + 0x13, half);
        }
        for j in 0..3u32 {
            let v = fx.mem.read_u8(g + far - j);
            fx.mem.write_u8(d + 0x40 + j, v);
            fx.mem.write_u8(d + 0x20 + j, v);
        }
        for j in 0..3u32 {
            let v = fx.mem.read_u8(g + near - j);
            fx.mem.write_u8(d + 0x30 + j, v);
            fx.mem.write_u8(d + 0x10 + j, v);
        }
    } else {
        for (j, (src, f)) in [(0x26u32, dc[0]), (0x25, dc[1]), (0x24, dc[2])].into_iter().enumerate() {
            let ra = l.ra_colour[j];
            let j = j as u32;
            let base = fx.mem.u8(g + src);
            let v = base.wrapping_sub(fix(fx, at, ra, ee::mul(f, ee::from_i32(k))));
            fx.mem.set8(d + 0x20 + j, v);
            fx.mem.set8(d + 0x10 + j, v);
            let base = fx.mem.u8(g + src);
            let v = base.wrapping_sub(fix(fx, at, ra + 0x2C, ee::mul(f, ee::from_i32(k + 1))));
            fx.mem.set8(d + 0x40 + j, v);
            fx.mem.set8(d + 0x30 + j, v);
        }
    }
    if k == fx.mem.u8(g + 0xA) - 1 {
        fx.mem.write_u8(d + 0x33, 0);
        fx.mem.write_u8(d + 0x43, 0);
    }
    let (w0, w1, w2, w3) = (fx.mem.f(wv + 8), fx.mem.f(wv), fx.mem.f(wv + 4), fx.mem.f(wv + 0xC));
    let depth = ee::div(ee::add(w3, ee::add(w0, ee::add(w1, w2))), 4.0);
    fx.fix_residue(depth, at + l.ra_key);
    let key = ee::fixunssfsi(depth) as i32;
    fx.submit(d, ot, key);
}
