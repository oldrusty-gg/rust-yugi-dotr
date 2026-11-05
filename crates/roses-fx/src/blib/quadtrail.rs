//! A trail of camera-facing quads laid in rings:
//! e505011844d5 pushes a ring of points (rows shifted down, a new ring
//! round a point, turned to face a direction) and ea3f31e9e9f0 draws every
//! point as an animated, growing quad facing the camera; with the
//! executable helpers they need (0x2055D0 / 0x1FC2C0, 0x103C40, 0x20C8B0,
//! 0x20BEC0, 0x1A74C0 asinf).
//!
//! A group `g`: +0 the texture, +4..+6 colour (B, G, R), +7 a new point's
//! alpha, +8 flags (1: frames by column; 0xA..0xC blend; 0xF grow with
//! age; 0x10 drift by +0x20; 0x13 far Z; 0x14..0x17 vertex k projected
//! during a draw; 0x1F draw every slot), +0xC the points (16 bytes: x, y,
//! z, +0xC alpha, +0xD frame, +0xE age), +0x14 the cell (w, h, u, v; +0x1C
//! / +0x1E the sheet's w / h), +0x20 the drift / ring direction (vec4),
//! +0x30 the half size, +0x34 the growth, +0x38 the ring radius, +0x46
//! slots (rows), +0x48 rows in use, +0x4A points a ring, +0x4C frames, +0x4E
//! ticks a frame.
use super::beam::length3;
use super::rel;
use super::trail::normalize3;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx, CAMERA_TABLE};
use crate::fxlib::{ALPHA_42, ALPHA_44, ALPHA_88};

/// RGBAQ then UV / XYZF2 x 4 (one colour).
const REGS_TEX4_FLAT: u64 = 0x4_2424_2421;
const PI: f32 = f32::from_bits(0x4049_0FDB);

/// 0x2055D0(out) = 0x1FC2C0(0, out) [F]: with a camera object (0x2DF5C0
/// [0]) the rotation (-(32768 obj+8 / pi), 0x4000 - 32768 obj+4 / pi, 0)
/// (0x1A2B80; 0x20BDA0) into `out`, 1; else 0 and nothing written.
pub fn camera_turn(fx: &mut Fx, out: u32) -> i32 {
    if out == 0 {
        return 0;
    }
    let obj = fx.mem.read_u32(CAMERA_TABLE);
    if obj == 0 {
        return 0;
    }
    let sp = fx.enter(0x40);
    let a = |fx: &mut Fx, off: u32| ee::fixsfsi(ee::div(ee::mul(32768.0, fx.mem.f(obj + off)), PI));
    let rx = a(fx, 8).wrapping_neg();
    let ry = 0x4000i32.wrapping_sub(a(fx, 4));
    fx.mem.set32(sp + 0x30, rx);
    fx.mem.set32(sp + 0x34, ry);
    fx.mem.write_u32(sp + 0x3C, 0);
    fx.mem.write_u32(sp + 0x38, 0);
    let m = fx.rotation_matrix([rx, ry, 0]);
    fx.write_m4(out, &m);
    fx.leave(0x40);
    1
}

/// 0x20C8B0(out, a, b) [F]: out = a x b (mula / msub), out.w = a.w.
pub fn cross3(fx: &mut Fx, out: u32, a: u32, b: u32) {
    for (k, (i, j)) in [(1u32, 2u32), (2, 0), (0, 1)].into_iter().enumerate() {
        let acc = ee::mul(fx.mem.f(a + 4 * i), fx.mem.f(b + 4 * j));
        let v = ee::sub(acc, ee::mul(fx.mem.f(a + 4 * j), fx.mem.f(b + 4 * i)));
        fx.mem.setf(out + 4 * k as u32, v);
    }
    let w = fx.mem.read_u32(a + 0xC);
    fx.mem.write_u32(out + 0xC, w);
}

/// 0x20BEC0(out, axis, angle) [F]: the rotation by `angle` (table trig)
/// about the (normalized, 0x20C7E0) `axis` (copied to sp + 0x40).
pub fn axis_matrix(fx: &mut Fx, out: u32, axis: u32, angle: i32) {
    let sp = fx.enter(0x50);
    let v = fx.read_v4(axis);
    fx.write_v4(sp + 0x40, v);
    let c = fx.tcos(angle);
    let k = ee::sub(1.0, c);
    let s = fx.tsin(angle);
    normalize3(fx, sp + 0x40);
    let (x, y, z) = (fx.mem.f(sp + 0x40), fx.mem.f(sp + 0x44), fx.mem.f(sp + 0x48));
    let diag = |a: f32| ee::add(c, ee::mul(k, ee::mul(a, a)));
    let madd = |p: f32, q: f32, r: f32| ee::add(ee::mul(k, ee::mul(p, q)), ee::mul(r, s));
    let msub = |p: f32, q: f32, r: f32| ee::sub(ee::mul(k, ee::mul(p, q)), ee::mul(r, s));
    for (off, v) in [
        (0u32, diag(x)),
        (0x10, msub(x, y, z)),
        (0x20, madd(x, z, y)),
        (0x04, madd(x, y, z)),
        (0x14, diag(y)),
        (0x24, msub(y, z, x)),
        (0x08, msub(x, z, y)),
        (0x18, madd(y, z, x)),
        (0x28, diag(z)),
    ] {
        fx.mem.setf(out + off, v);
    }
    for off in [0x30u32, 0x34, 0x38, 0xC, 0x1C, 0x2C] {
        fx.mem.write_u32(out + off, 0);
    }
    fx.mem.write_u32(out + 0x3C, 0x3F80_0000);
    fx.leave(0x50);
}

/// The polynomials of asinf (t: p, q).
fn asin_pq(t: f32) -> (f32, f32) {
    let c = |b: u32| f32::from_bits(b);
    let mut p = ee::mul(t, c(0x3811_EF08));
    let mut q = ee::mul(t, c(0x3D9D_C62E));
    p = ee::add(p, c(0x3A4F_7F04));
    q = ee::add(q, c(0xBF30_3361));
    p = ee::mul(t, p);
    q = ee::mul(t, q);
    p = ee::add(p, c(0xBD24_1146));
    q = ee::add(q, c(0x4001_572D));
    p = ee::mul(t, p);
    q = ee::mul(t, q);
    p = ee::add(p, c(0x3E4E_0AA8));
    q = ee::add(q, c(0xC019_D139));
    p = ee::mul(t, p);
    q = ee::mul(t, q);
    p = ee::add(p, c(0xBEA6_B090));
    let q = ee::add(q, 1.0);
    p = ee::mul(t, p);
    p = ee::add(p, c(0x3E2A_AAAB));
    (ee::mul(t, p), q)
}

/// 0x1A74C0 -> 0x1A4C68 asinf [F, the executable's float asin]: |x| = 1
/// x pio2_hi + x pio2_lo; |x| > 1 (x - x) / (x - x); |x| < 0.5 x + x p / q
/// (tiny: x when huge + x > 1); else from t = (1 - |x|) / 2 and s =
/// sqrtf(t) (0x1A5FC0), with the low half of s split off below 0.975.
pub fn asinf(fx: &mut Fx, x: f32) -> f32 {
    const PIO2_HI: f32 = f32::from_bits(0x3FC9_0FDA);
    const PIO2_LO: f32 = f32::from_bits(0x33A2_2168);
    const PIO4_HI: f32 = f32::from_bits(0x3F49_0FDB);
    let hx = x.to_bits() as i32;
    let ix = hx & 0x7FFF_FFFF;
    if ix == 0x3F80_0000 {
        return ee::add(ee::mul(x, PIO2_HI), ee::mul(x, PIO2_LO));
    }
    if ix > 0x3F80_0000 {
        let d = ee::sub(x, x);
        return ee::div(d, d);
    }
    if ix <= 0x3EFF_FFFF {
        if ix > 0x31FF_FFFF {
            let t = ee::mul(x, x);
            let (p, q) = asin_pq(t);
            let r = ee::div(p, q);
            return ee::add(x, ee::mul(x, r));
        }
        if crate::exe::ee_lt(1.0, ee::add(x, f32::from_bits(0x7149_F2CA))) {
            return x;
        }
    }
    let w = ee::abs(x);
    let t = ee::mul(ee::sub(1.0, w), 0.5);
    let (p, q) = asin_pq(t);
    let s = fx.sqrtf(t);
    let v = if ix > 0x3F79_9999 {
        let r = ee::div(p, q);
        let a = ee::add(s, ee::mul(s, r));
        let a = ee::sub(ee::add(a, a), PIO2_LO);
        ee::sub(PIO2_HI, a)
    } else {
        let w = f32::from_bits(s.to_bits() & 0xFFFF_F000);
        let c = ee::sub(t, ee::mul(w, w));
        let sw = ee::add(s, w);
        let r = ee::div(p, q);
        let s2 = ee::add(s, s);
        let w2 = ee::add(w, w);
        let c = ee::div(c, sw);
        let qq = ee::sub(PIO4_HI, w2);
        let sr = ee::mul(s2, r);
        let c2 = ee::add(c, c);
        let pp = ee::sub(sr, ee::sub(PIO2_LO, c2));
        ee::sub(PIO4_HI, ee::sub(pp, qq))
    };
    if hx > 0 {
        v
    } else {
        ee::neg(v)
    }
}

/// 0x103C40(v, out) [F]: the rotation turning (0, 1, 0) onto the
/// direction `v` (both normalized, 0x20C7E0): axis = up x v (0x20C8B0),
/// angle = asinf(|axis|) (0x20C850, 0x1A74C0) in 1 / 65536 turns (0x1A2B80),
/// 0x20BEC0 into `out`.
pub fn aim_matrix(fx: &mut Fx, v: u32, out: u32) {
    let sp = fx.enter(0x50);
    let d = fx.read_v4(v);
    fx.write_v4(sp + 0x20, d);
    normalize3(fx, sp + 0x20);
    fx.mem.write_u32(sp + 0x38, 0);
    fx.mem.write_u32(sp + 0x30, 0);
    fx.mem.write_u32(sp + 0x34, 0x3F80_0000);
    normalize3(fx, sp + 0x30);
    cross3(fx, sp + 0x40, sp + 0x30, sp + 0x20);
    let len = length3(fx, sp + 0x40);
    let a = asinf(fx, len);
    let turn = ee::div(a, f32::from_bits(0x40C9_0FDB));
    let angle = ee::fixsfsi(ee::mul(65536.0, turn));
    axis_matrix(fx, out, sp + 0x40, angle);
    fx.leave(0x50);
}

/// e505011844d5 [11 B records; b628 0x1E02090, 128 words] [F]: `push(g,
/// pos) -> 0`: one more row in use (up to the slots), the rows shifted down
/// one (row r = row r - 1, from the last), then row 0 a ring of +0x4A
/// points of radius +0x38 round `pos`, turned to face +0x20 (0x103C40 into
/// sp + 0x60; each point (r sin, 0, r cos) through it, 0x20C540, w the
/// stale sp + 0xAC), alpha +7, frame and age 0.
pub fn push(fx: &mut Fx, g: u32, pos: u32) -> i32 {
    let sp = fx.enter(0xC0);
    let used = fx.mem.i16(g + 0x48);
    if fx.mem.i16(g + 0x46) - 1 >= used {
        fx.mem.set16(g + 0x48, i32::from(used) + 1);
    }
    let mut r = i32::from(fx.mem.i16(g + 0x48));
    while r > 0 {
        let n = i32::from(fx.mem.i16(g + 0x4A));
        let base = fx.mult(r, n);
        let mut k = 0;
        while k < i32::from(fx.mem.i16(g + 0x4A)) {
            let n = i32::from(fx.mem.i16(g + 0x4A));
            let pts = fx.mem.read_u32(g + 0xC);
            let src = pts.wrapping_add(((k + base - n) << 4) as u32);
            let dst = pts.wrapping_add(((base + k) << 4) as u32);
            for c in 0..4u32 {
                let w = fx.mem.read_u32(src + 4 * c);
                fx.mem.write_u32(dst + 4 * c, w);
            }
            k += 1;
        }
        r -= 1;
    }
    aim_matrix(fx, g + 0x20, sp + 0x60);
    let m = fx.read_m4(sp + 0x60);
    let mut k = 0;
    while k < i32::from(fx.mem.i16(g + 0x4A)) {
        fx.mem.write_u32(sp + 0xA4, 0);
        let n = i32::from(fx.mem.i16(g + 0x4A));
        let (step, _) = fx.div(0x1_0000, n);
        let a = fx.mult(step, k);
        let c = fx.tcos(a);
        let r = fx.mem.f(g + 0x38);
        fx.mem.setf(sp + 0xA8, ee::mul(r, c));
        let n = i32::from(fx.mem.i16(g + 0x4A));
        let (step, _) = fx.div(0x1_0000, n);
        let a = fx.mult(step, k);
        let s = fx.tsin(a);
        let r = fx.mem.f(g + 0x38);
        fx.mem.setf(sp + 0xA0, ee::mul(r, s));
        let v = fx.read_v4(sp + 0xA0);
        let o = fx.transform(Some(&m), v);
        fx.write_v4(sp + 0xB0, o);
        let e = fx.mem.read_u32(g + 0xC).wrapping_add((16 * k) as u32);
        for c in 0..3u32 {
            let x = ee::add(fx.mem.f(pos + 4 * c), fx.mem.f(sp + 0xB0 + 4 * c));
            let e = fx.mem.read_u32(g + 0xC).wrapping_add((16 * k) as u32);
            fx.mem.setf(e + 4 * c, x);
        }
        fx.mem.write_u8(e + 0xD, 0);
        fx.mem.set16(e + 0xE, 0);
        let a = fx.mem.read_u8(g + 7);
        fx.mem.write_u8(e + 0xC, a);
        k += 1;
    }
    fx.leave(0xC0);
    0
}

/// ea3f31e9e9f0 [11 B records; b628 0x1E02290, 644 words] [F]: `draw(ot,
/// g, x) -> 0`: every point in use (rows in use x points; flag 0x1F: every
/// slot) as a quad: the record's corner template (record-relative `lui` /
/// `addiu` +0x28 / +0x30, 4 vec4 at sp + 0x160, x / y = -+ the half size
/// +0x30) turned to face the camera (0x2055D0 into sp + 0x110; 0x20C540
/// into sp + 0x1A0), scaled by +0x34 x age (flag 0xF), at the point
/// (drifting by +0x20 with flag 0x10, 0x103620; age + 1), projected through
/// `x` + 0x30 (0x204D80; the fourth w lands on the second's slot, the key's
/// fourth w is the stale sp + 0x15C). Descriptor at sp + 0x1F8 (9 values:
/// RGBAQ then UV / XYZF2 x 4; blend 0xC / 0xB / 0xA, else 0x48; colour +6
/// / +5 / +4, alpha the point's): the cell of the point's frame (by column
/// with flag 1) as UVs and REGION_CLAMP, the texture (filter 1); flag 0x13
/// far Z key 0, else the mean-w key (0x1A2C70). The frame steps every +0x4E
/// ages, wrapping after +0x4C.
pub fn draw(fx: &mut Fx, at: u32, ot: u32, g: u32, x: u32) -> i32 {
    let template = rel(fx, at, 0x28, 0x30);
    let sp = fx.enter(0x270);
    for k in 0..4u32 {
        let v = fx.read_v4(template + 16 * k);
        fx.write_v4(sp + 0x160 + 16 * k, v);
    }
    for (k, (sx, sy)) in [(true, false), (false, false), (true, true), (false, true)].into_iter().enumerate() {
        let a = sp + 0x160 + 16 * k as u32;
        let h = fx.mem.f(g + 0x30);
        fx.mem.setf(a, if sx { ee::neg(h) } else { h });
        let h = fx.mem.f(g + 0x30);
        fx.mem.setf(a + 4, if sy { ee::neg(h) } else { h });
    }
    let rows = if bit(fx.mem.read_u32(g + 8), 0x1F) { fx.mem.i16(g + 0x46) } else { fx.mem.i16(g + 0x48) };
    let n = i32::from(fx.mem.i16(g + 0x4A));
    let count = fx.mult(i32::from(rows), n);
    fx.mem.set32(sp + 0xB0, count);
    camera_turn(fx, sp + 0x110);
    let cam = fx.read_m4(sp + 0x110);
    for k in 0..4u32 {
        let v = fx.read_v4(sp + 0x160 + 16 * k);
        let o = fx.transform(Some(&cam), v);
        fx.write_v4(sp + 0x1A0 + 16 * k, o);
    }
    let d = sp + 0x1F8;
    fx.desc_init(d, 9, 0x24, REGS_TEX4_FLAT);
    fx.mem.write_u32(sp + 0x100, d + 2);
    fx.alpha_for(d, g + 8, &[(0xC, ALPHA_44), (0xB, ALPHA_88), (0xA, ALPHA_42)]);
    for (o, s) in [(0u32, 6u32), (1, 5), (2, 4)] {
        let c = fx.mem.read_u8(g + s);
        fx.mem.write_u8(sp + 0x208 + o, c);
    }
    fx.mem.write_u32(sp + 0x20C, 0x3F80_0000);
    let xyz = [sp + 0x218, sp + 0x228, sp + 0x238, sp + 0x248];
    let wslot = [sp + 0x150, sp + 0x154, sp + 0x158, sp + 0x154];
    let mut i = 0;
    while i < fx.mem.i32(sp + 0xB0) {
        let off = (16 * i) as u32;
        let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
        let frame = i32::from(fx.mem.read_u8(e + 0xD));
        let (cw, ch) = (i32::from(fx.mem.i16(g + 0x14)), i32::from(fx.mem.i16(g + 0x16)));
        let (u0, v0) = (i32::from(fx.mem.i16(g + 0x18)), i32::from(fx.mem.i16(g + 0x1A)));
        let (u, v) = if bit(fx.mem.read_u32(g + 8), 1) {
            let sheet = i32::from(fx.mem.i16(g + 0x1C));
            let (cols, _) = fx.div(sheet, cw);
            let (_, col) = fx.div(frame, cols);
            let a = fx.mult(cw, col);
            let (row, _) = fx.div(frame, cols);
            let b = fx.mult(ch, row);
            (u0 + a, v0 + b)
        } else {
            let sheet = i32::from(fx.mem.i16(g + 0x1E));
            let (rws, _) = fx.div(sheet, ch);
            let (col, _) = fx.div(frame, rws);
            let a = fx.mult(cw, col);
            let (_, row) = fx.div(frame, rws);
            let b = fx.mult(ch, row);
            (u0 + a, v0 + b)
        };
        let (u, v) = (i32::from(u as i16), i32::from(v as i16));
        // The frame steps every +0x4E ages.
        let age = i32::from(fx.mem.i16(e + 0xE));
        let ticks = i32::from(fx.mem.i16(g + 0x4E));
        let (_, rem) = fx.div(age, ticks);
        if rem == 0 {
            let f = fx.mem.read_u8(e + 0xD).wrapping_add(1);
            fx.mem.write_u8(e + 0xD, f);
            let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
            if i32::from(fx.mem.i16(g + 0x4C)) - 1 < i32::from(fx.mem.read_u8(e + 0xD)) {
                fx.mem.write_u8(e + 0xD, 0);
            }
        }
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        let (cw, ch) = (i32::from(fx.mem.i16(g + 0x14)), i32::from(fx.mem.i16(g + 0x16)));
        for (k, (du, dv)) in [(false, false), (true, false), (false, true), (true, true)].into_iter().enumerate() {
            let a = sp + 0x210 + 0x10 * k as u32;
            fx.set_uv(a, if du { u + cw - 1 } else { u });
            fx.set_uv(a + 2, if dv { v + ch - 1 } else { v });
        }
        let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
        let a = fx.mem.read_u8(e + 0xC);
        fx.mem.write_u8(sp + 0x20B, a);
        let scale = if bit(fx.mem.read_u32(g + 8), 0xF) {
            let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
            ee::mul(fx.mem.f(g + 0x34), ee::from_i32(i32::from(fx.mem.i16(e + 0xE))))
        } else {
            1.0
        };
        if bit(fx.mem.read_u32(g + 8), 0x10) {
            let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
            fx.add3(e, g + 0x20);
        }
        let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
        let age = fx.mem.i16(e + 0xE).wrapping_add(1);
        fx.mem.set16(e + 0xE, i32::from(age));
        let m = fx.read_m4(x + 0x30);
        for k in 0..4u32 {
            let c = sp + 0x1A0 + 16 * k;
            let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
            for j in 0..3u32 {
                let v = ee::add(ee::mul(fx.mem.f(c + 4 * j), scale), fx.mem.f(e + 4 * j));
                fx.mem.setf(sp + 0x1E0 + 4 * j, v);
            }
            if fx.project_at(sp + 0x1E0, &m, xyz[k as usize], wslot[k as usize]) {
                fx.flag(g + 8, 0x14 + k);
            }
        }
        let clamp = ((u as i64) << 4) as u64 | 0xA | (((u + cw - 1) as i64 as u64) << 14) | ((v as i64 as u64) << 24) | (((v + ch - 1) as i64 as u64) << 34);
        fx.desc_clamp(d, clamp);
        let rh2 = fx.mem.read_u32(g);
        fx.desc_texture(d, rh2, 1);
        if fx.all_projected(g + 8) {
            if bit(fx.mem.read_u32(g + 8), 0x13) {
                fx.far_z(&xyz);
                fx.submit(d, ot, 0);
            } else {
                let key = fx.mean_key4([sp + 0x150, sp + 0x154, sp + 0x158, sp + 0x15C]);
                fx.submit(d, ot, key);
            }
        }
        fx.clear_projected(g + 8);
        i += 1;
    }
    fx.leave(0x270);
    0
}
