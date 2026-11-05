//! Curve evaluation of the effect library: cubic weights of seven bases
//! and the point they blend.
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

fn k(bits: u32) -> f32 {
    f32::from_bits(bits)
}
/// 1/6, 2/3, 11/12, 7/12 as the code builds them (lui / ori).
const SIXTH: u32 = 0x3E2A_AAAB;
const TWO_THIRDS: u32 = 0x3F2A_AAAB;
const ELEVEN_TWELFTHS: u32 = 0x3F6A_AAAB;
const SEVEN_TWELFTHS: u32 = 0x3F15_5555;

/// The four weights of basis `basis` at `t` (f0..f3 of 44769f26f5eb, in
/// the original's operation order [F b001 0x1E008D0..0x1E00C5C]); `None`
/// for a basis outside 0..6 (the original then blends with whatever f0..f3
/// held).
pub fn weights(t: f32, basis: i32) -> Option<[f32; 4]> {
    use ee::{add, div, mul, neg, sub};
    Some(match basis {
        // Lagrange through -1, 0, 1, 2.
        0 => {
            let f6 = sub(t, 1.0);
            let f2 = sub(t, 2.0);
            let f7 = add(1.0, t);
            let w0 = div(mul(neg(t), mul(f6, f2)), 6.0);
            let w1 = div(mul(f2, mul(f7, f6)), 2.0);
            let f4 = mul(t, f2);
            let w3 = div(mul(f7, mul(t, f6)), 6.0);
            let w2 = div(mul(neg(f7), f4), 2.0);
            [w0, w1, w2, w3]
        }
        // Uniform cubic B-spline.
        1 => {
            let f2 = sub(1.0, t);
            let w0 = div(mul(f2, mul(f2, f2)), 6.0);
            let f8 = mul(t, t);
            let f6 = mul(t, f8);
            let w3 = div(f6, 6.0);
            let f5 = sub(div(f6, 2.0), f8);
            let w1 = add(k(TWO_THIRDS), f5);
            let f6b = div(neg(f6), 2.0);
            let f5b = div(f8, 2.0);
            let f4 = div(t, 2.0);
            let f4 = add(f4, add(f6b, f5b));
            let w2 = add(k(SIXTH), f4);
            [w0, w1, w2, w3]
        }
        2 => {
            let f7 = sub(1.0, t);
            let f8 = mul(t, t);
            let f5 = mul(4.5, f8);
            let f2 = mul(1.5, f8);
            let f8 = mul(t, f8);
            let acc = sub(mul(1.75, f8), f5);
            let w1 = add(acc, mul(3.0, t));
            let f4 = mul(k(ELEVEN_TWELFTHS), f8);
            let w3 = mul(k(SIXTH), f8);
            let w2 = add(neg(f4), f2);
            let w0 = mul(f7, mul(f7, f7));
            [w0, w1, w2, w3]
        }
        3 => {
            let f8 = mul(t, t);
            let f2 = div(t, 2.0);
            let f1 = sub(1.0, t);
            let f0 = mul(f1, mul(f1, f1));
            let f5 = mul(1.25, f8);
            let f9 = mul(t, f8);
            let f3 = mul(0.5, f8);
            let f4 = neg(mul(0.5, f9));
            let f3 = add(f4, f3);
            let f2 = add(f2, f3);
            let w0 = mul(0.25, f0);
            let w3 = mul(k(SIXTH), f9);
            let w2 = add(k(SIXTH), f2);
            let acc = sub(mul(k(SEVEN_TWELFTHS), f9), f5);
            let f1 = add(acc, mul(0.25, t));
            let w1 = add(k(SEVEN_TWELFTHS), f1);
            [w0, w1, w2, w3]
        }
        4 => {
            let f7 = sub(1.0, t);
            let f8 = mul(f7, f7);
            let f9 = mul(f7, f8);
            let f2 = mul(1.25, f8);
            let f3 = mul(-0.5, f9);
            let f0 = mul(0.5, f8);
            let acc = add(f3, f0);
            let f0 = add(acc, mul(0.5, f7));
            let f3 = mul(k(SEVEN_TWELFTHS), f9);
            let acc = sub(f3, f2);
            let f3 = mul(t, t);
            let w1 = add(k(SIXTH), f0);
            let f3 = mul(t, f3);
            let f2 = add(acc, mul(0.25, f7));
            let w0 = mul(k(SIXTH), f9);
            let w3 = mul(0.25, f3);
            let w2 = add(k(SEVEN_TWELFTHS), f2);
            [w0, w1, w2, w3]
        }
        5 => {
            let f7 = sub(1.0, t);
            let f8 = mul(f7, f7);
            let f9 = mul(f7, f8);
            let f4 = mul(1.75, f9);
            let f3 = mul(4.5, f8);
            let acc = mul(f32::from_bits(0xBF6A_AAAB), f9);
            let w1 = add(acc, mul(1.5, f8));
            let acc = sub(f4, f3);
            let f3 = mul(t, t);
            let w0 = mul(k(SIXTH), f9);
            let w3 = mul(t, f3);
            let w2 = add(acc, mul(3.0, f7));
            [w0, w1, w2, w3]
        }
        // Bernstein (Bezier).
        6 => {
            let f3 = sub(1.0, t);
            let f4 = mul(f3, f3);
            let f1 = mul(3.0, t);
            let w0 = mul(f3, f4);
            let w1 = mul(f1, f4);
            let f4 = mul(t, t);
            let f2 = mul(3.0, f4);
            let w2 = mul(f3, f2);
            let w3 = mul(t, f4);
            [w0, w1, w2, w3]
        }
        _ => return None,
    })
}

/// 44769f26f5eb [41 B records; b001 0x1E008D0, 272 words, a leaf]:
/// `spline(out, pts, t, basis)`: the point at `t` of the curve through the
/// four vec4 control points at `pts` (stride 0x10) with the weights of
/// `basis` ([`weights`]): x, y, z = ((p0 w0 + p1 w1) + p2 w2 ... as the
/// original's accumulator chain sums them, kept in a 16-byte stack local;
/// w is the local's fourth word, never written (stale) [F]. A basis outside
/// 0..6 blends with `stale_weights` (the caller's f0..f3 in the original).
pub fn spline(fx: &mut Fx, out: u32, pts: u32, t: f32, basis: i32, stale_weights: [f32; 4]) {
    use ee::{add, mul};
    let w = weights(t, basis).unwrap_or(stale_weights);
    let l = fx.enter(0x10);
    for c in 0..3u32 {
        let p: [f32; 4] = std::array::from_fn(|i| fx.mem.f(pts + 0x10 * i as u32 + 4 * c));
        let f5 = mul(p[2], w[2]);
        let f6 = add(mul(p[0], w[0]), mul(p[1], w[1]));
        let acc = add(f5, f6);
        let v = add(acc, mul(p[3], w[3]));
        fx.mem.setf(l + 4 * c, v);
    }
    let v: [f32; 4] = std::array::from_fn(|i| fx.mem.f(l + 4 * i as u32));
    fx.leave(0x10);
    for (i, x) in v.into_iter().enumerate() {
        fx.mem.setf(out + 4 * i as u32, x);
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 94867f3df00e [3 B records; rec 92 0x1E01E20, b501 0x1E016B0, 520
/// words; helper A9]: `tube(D, task, pts, mode)`: a textured strip3 tube
/// along the vec4 points `pts` (i16 +6 of them), each segment k in
/// 0 .. n - 1 cut in three by [`spline`] basis 0 through pts[k - 1 .. k + 3]
/// (the first segment through p0, p0, p1, p2) at t = 0.33, 0.66 and 1
/// (0x3EA8F5C3 added, clamped to 1 from 0.9); per piece two quads
/// (vertical, then horizontal with half the z offset) of half width
/// size i16 +0x38 / 10 narrowing by size / 40 n a piece, alpha falling by
/// +0x2B / 3n, colours +0x28 / +0x2C, UVs D+0x94 in orientation D+0xBE,
/// texture D+0x88, transformed by the task's angles +0x10 about D+0x20;
/// mode 0 keyed by the mean w, 1 far Z at key 0, 2 key -1, else nothing
/// drawn. 0x205580's angles at sp + 0x1B0 get the yaw turned to -yaw / 180
/// (the 0xB60B60B7 magic division), then go unused [F].
pub fn tube(fx: &mut Fx, at: u32, d0: u32, t: u32, pts: u32, mode: i32) {
    tube_by(fx, at, d0, t, pts, mode, &TUBE);
}

/// a28d42cd7fc2 [card 25 0x1E033A0, 504 words]: [`tube`]
/// compiled flat: no task turn (no 0x1097D0 / 0x205580; projected through
/// the camera's view-projection, 0x204F70 with m 0, the mean w at sp +
/// 0x190), the alpha falling by +0x2B / 2n and the width by size / 80 n a
/// piece, in a 0x280 frame (the descriptor at sp + 0x1C8, the spline ends
/// at sp + 0x1A0 / 0x1B0) [F fdiff vs 94867f3df00e].
pub fn tube_flat(fx: &mut Fx, d0: u32, t: u32, pts: u32, mode: i32) {
    tube_by(fx, 0, d0, t, pts, mode, &FLAT);
}

/// A compile of [`tube`]'s frame and divisors.
struct TubeFrame {
    frame: u32,
    desc: u32,
    /// The four RGBAQ locals (8 apart).
    rgba: u32,
    colour: u32,
    /// The spline end (later segments, the first).
    ends: (u32, u32),
    mean_w: u32,
    /// Turned by the task's angles (0x1097D0 into sp + 0x1C0, 0x205580).
    turned: bool,
    /// The alpha's divisor per n.
    alpha_n: i32,
    /// The width's divisor per n (size / 10 / (width n) a piece).
    width_n: f32,
}

const TUBE: TubeFrame = TubeFrame { frame: 0x2E0, desc: 0x228, rgba: 0x2B8, colour: 0x2DC, ends: (0x200, 0x210), mean_w: 0x1A0, turned: true, alpha_n: 3, width_n: 4.0 };
const FLAT: TubeFrame = TubeFrame { frame: 0x280, desc: 0x1C8, rgba: 0x258, colour: 0x27C, ends: (0x1A0, 0x1B0), mean_w: 0x190, turned: false, alpha_n: 2, width_n: 8.0 };

fn tube_by(fx: &mut Fx, at: u32, d0: u32, t: u32, pts: u32, mode: i32, l: &TubeFrame) {
    use crate::c061::rgbaq_at;
    use crate::fxlib2 as l2;
    use ee::{add, div, from_i32, mul, sub};
    let sp = fx.enter(l.frame);
    fx.mem.write_u32(sp + 0xD0, pts);
    let ot = crate::exe::OT;
    let mut f21 = div(from_i32(fx.mem.i16(t + 0x38)), 10.0);
    let n = fx.mem.i16(t + 6);
    let d = sp + l.desc;
    l2::desc_strip3(fx, d);
    let f22 = f21;
    for (k, (c, to)) in [(0x28u32, 0x10u32), (0x2C, 0x28), (0x28, 0x40), (0x2C, 0x58)].into_iter().enumerate() {
        rgbaq_at(fx, sp + l.rgba + 8 * k as u32, t + c, d + to);
    }
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    let colour = sp + l.colour;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(colour + k, v);
    }
    let pa = sp + 0x100;
    copy4(fx, pa, pts);
    copy4(fx, pa + 0x10, pts);
    copy4(fx, pa + 0x20, pts + 0x10);
    copy4(fx, pa + 0x30, pts + 0x20);
    if l.turned {
        for k in 0..3 {
            let w = fx.mem.read_u32(d0 + 0x20 + 4 * k);
            fx.mem.write_u32(sp + 0x190 + 4 * k, w);
        }
        l2::xform_vp(fx, sp + 0x1C0, 0, t + 0x10, sp + 0x190);
        crate::b494::camera_angles_saving(fx, sp + 0x1B0, u64::from(d0), n as i64 as u64, at + 0x228);
        let yaw = fx.mem.i32(sp + 0x1B4);
        fx.mult(yaw, 0xB60B_60B7u32 as i32);
        fx.mem.set32(sp + 0x1B4, (yaw / 180).wrapping_neg());
    }
    let m = if l.turned { sp + 0x1C0 } else { 0 };
    let f24 = mul(l.width_n, from_i32(n));
    let fp = l.alpha_n * n;
    fx.mem.set32(sp + 0xC0, n - 1);
    let third = f32::from_bits(0x3EA8_F5C3);
    let near_end = f32::from_bits(0x3F66_6666);
    let mut s0 = 0;
    while s0 < n - 1 {
        let s5 = pts.wrapping_add((0x10 * (s0 - 1)) as u32);
        let mut f20 = 0.0f32;
        for s2 in 0..3 {
            let (out, from) = if s0 == 0 { (l.ends.1, pa) } else { (l.ends.0, s5) };
            if s0 == 0 && s2 == 0 {
                spline(fx, sp + 0xE0, pa, f20, 0, [0.0; 4]);
            } else {
                copy4(fx, sp + 0xE0, sp + 0x140);
            }
            f20 = add(f20, third);
            if !(f20 < near_end) {
                f20 = 1.0;
            }
            spline(fx, sp + out, from, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0xF0, sp + out);
            copy4(fx, sp + 0x140, sp + 0xF0);
            l2::colour_strip3(fx, d, colour, 1);
            let al = i32::from(fx.mem.read_u8(t + 0x2B));
            let q = fx.div(al, fp).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(colour + 3)) - q;
            fx.mem.write_u8(colour + 3, if v > 0 { v as u8 } else { 0 });
            l2::colour_strip3(fx, d, colour, 2);
            let f23 = f21;
            f21 = sub(f21, div(f22, f24));
            let pv: [u32; 3] = std::array::from_fn(|c| fx.mem.read_u32(sp + 0xE0 + 4 * c as u32));
            let qv: [u32; 3] = std::array::from_fn(|c| fx.mem.read_u32(sp + 0xF0 + 4 * c as u32));
            let v = sp + 0x150;
            for s1 in 0..2 {
                for (o, src) in [(0u32, pv), (0x10, pv), (0x20, qv), (0x30, qv)] {
                    for c in 0..3u32 {
                        fx.mem.write_u32(v + o + 4 * c, src[c as usize]);
                    }
                }
                let bump = |fx: &mut Fx, at: u32, by: f32, up: bool| {
                    let x = fx.mem.f(v + at);
                    fx.mem.setf(v + at, if up { add(x, by) } else { sub(x, by) });
                };
                if s1 == 0 {
                    bump(fx, 0x14, f23, true);
                    bump(fx, 0x34, f21, true);
                    bump(fx, 0x4, f23, false);
                    bump(fx, 0x24, f21, false);
                } else {
                    let h2 = div(f23, 2.0);
                    bump(fx, 0x10, f23, true);
                    let h1 = div(f21, 2.0);
                    bump(fx, 0x30, f21, true);
                    bump(fx, 0x18, h2, true);
                    bump(fx, 0x38, h1, true);
                    bump(fx, 0x0, f23, false);
                    bump(fx, 0x20, f21, false);
                    bump(fx, 0x8, h2, false);
                    bump(fx, 0x28, h1, false);
                }
                if (0..=2).contains(&mode) && l2::project(fx, v, 0x7000_0000, m, d, sp + l.mean_w) {
                    match mode {
                        0 => {
                            let key = ee::fixunssfsi(fx.mem.f(sp + l.mean_w)) as i32;
                            fx.submit(d, ot, key);
                        }
                        1 => {
                            l2::far_z_desc(fx, d, 0);
                            fx.submit(d, ot, 0);
                        }
                        _ => fx.submit(d, ot, -1),
                    }
                }
            }
        }
        s0 += 1;
    }
    fx.leave(l.frame);
}

/// c88b3058a080 [5 B records: 111 / 180 / 216 / 225 / 376; card 376
/// 0x1E01260, 368 words]: `ribbon_tex(D, task, a, b)`: a
/// textured strip3 ribbon between the vec4 point histories `a` and `b`
/// (i16 task +6 = n points): per segment s in 0 .. n - 2 four strips (t
/// = 0, 0.25, 0.5, 0.75; vertices a(t), a(t + 1/4), b(t), b(t + 1/4) by
/// [`spline`] basis 0) through the camera (matrix 0) at key 0. The first
/// segment runs on stack copies a0 a1 a2 a3 / b0 b0 b1 b2 (sp + 0x190 /
/// 0x1D0), the later ones through a + 0x10 s / b + 0x10 (s - 1) (the
/// original's own asymmetry). Colours +0x28 / +0x2C: the near pair as
/// they stand, the far pair with the alphas down by +0x2B / (n - 1) and
/// +0x2F / (n - 1) (per segment, wrapping as bytes); UVs D+0x94 in
/// orientation D+0xBE (0x108DE0), texture D+0x88 (filter 1). A 0x330
/// frame (descriptor sp + 0x258, mean w sp + 0xC0) [F].
pub fn ribbon_tex(fx: &mut Fx, d0: u32, t: u32, a: u32, b: u32) {
    use crate::fxlib2 as l2;
    let sp = fx.enter(0x330);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x258;
    l2::desc_strip3(fx, d);
    for (local, c, to) in [(0x2E8u32, 0x28u32, 0x10u32), (0x2F0, 0x2C, 0x28), (0x2F8, 0x28, 0x40), (0x300, 0x2C, 0x58)] {
        rgbaq_to(fx, sp + local, t + c, d + to);
    }
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let rh2 = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, rh2, 1);
    let (ca, cb) = (sp + 0x328, sp + 0x32C);
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(ca + k, v);
        let v = fx.mem.read_u8(t + 0x2C + k);
        fx.mem.write_u8(cb + k, v);
    }
    let div = n - 1;
    let (pa, pb) = (sp + 0x190, sp + 0x1D0);
    for (k, o) in [0u32, 0x10, 0x20, 0x30].into_iter().enumerate() {
        copy4(fx, pa + 0x10 * k as u32, a + o);
    }
    for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
        copy4(fx, pb + 0x10 * k as u32, b + o);
    }
    fx.mem.set32(sp + 0xB0, n - 2);
    let mut s0 = 0i32;
    while s0 < fx.mem.i32(sp + 0xB0) {
        rgbaq_to(fx, sp + 0x308, ca, d + 0x10);
        rgbaq_to(fx, sp + 0x310, cb, d + 0x28);
        let al = i32::from(fx.mem.read_u8(t + 0x2B));
        let q = fx.div(al, div).0;
        let v = fx.mem.read_u8(ca + 3).wrapping_sub(q as u8);
        fx.mem.write_u8(ca + 3, v);
        // div1: LO1, not the LO / HI the other divisions leave.
        let al = i32::from(fx.mem.read_u8(t + 0x2F));
        let q = if div != 0 { al.wrapping_div(div) } else { 0 };
        let v = fx.mem.read_u8(cb + 3).wrapping_sub(q as u8);
        fx.mem.write_u8(cb + 3, v);
        rgbaq_to(fx, sp + 0x318, ca, d + 0x40);
        rgbaq_to(fx, sp + 0x320, cb, d + 0x58);
        let (qa, qb, ends) = if s0 != 0 {
            (a.wrapping_add((0x10 * s0) as u32), b.wrapping_add((0x10 * (s0 - 1)) as u32), (0x210u32, 0x220u32))
        } else {
            (pa, pb, (0x230, 0x240))
        };
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            spline(fx, sp + 0x110, qa, f20, 0, [0.0; 4]);
            spline(fx, sp + 0x150, qb, f20, 0, [0.0; 4]);
            f20 = ee::add(f20, 0.25);
            spline(fx, sp + ends.0, qa, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x120, sp + ends.0);
            spline(fx, sp + ends.1, qb, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x160, sp + ends.1);
            for (to, from) in [(0xD0u32, 0x110u32), (0xE0, 0x120), (0xF0, 0x150), (0x100, 0x160)] {
                for k in 0..3 {
                    let w = fx.mem.read_u32(sp + from + 4 * k);
                    fx.mem.write_u32(sp + to + 4 * k, w);
                }
            }
            if l2::project(fx, sp + 0xD0, 0x7000_0000, 0, d, sp + 0xC0) {
                fx.submit(d, crate::exe::OT, 0);
            }
        }
        s0 += 1;
    }
    fx.leave(0x330);
}

/// 0x108020(local, c) and the 8 bytes it builds copied to `to`.
fn rgbaq_to(fx: &mut Fx, local: u32, c: u32, to: u32) {
    crate::fxlib2::rgbaq(fx, local, c);
    for k in 0..2 {
        let w = fx.mem.read_u32(local + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 30cc3a899ca1 [7 B records: 56 / 57 / 180 / 216 / 225 / 353 / 596; card
/// 180 0x1E009E0, 284 words]: `ribbon_g(D, task, a, b)`: an
/// untextured gouraud ribbon (0x109BC0 blend 0 over 0x109DD0's window
/// strip) between the vec4 point histories `a` and `b` (i16 task +6 = n
/// points): per segment s in 0 .. n - 1 four strips (t = 0, 0.25, 0.5,
/// 0.75; vertices a(t), a(t + 1/4), b(t), b(t + 1/4) by [`spline`] basis
/// 0) through the camera (matrix 0) at key 0, the first segment on stack
/// copies a0 a0 a1 a2 / b0 b0 b1 b2, the later ones through a / b + 0x10
/// (s - 1). Colour +0x28: the near pair (0x108270 which 1) as it stands,
/// the far pair (which 2) with the alpha down by +0x2B / n once a segment
/// (wrapping as a byte). A 0x2C0 frame (descriptor sp + 0x228, mean w sp
/// + 0xD0) [F].
pub fn ribbon_g(fx: &mut Fx, t: u32, a: u32, b: u32) {
    use crate::fxlib2 as l2;
    let sp = fx.enter(0x2C0);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x228;
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    fx.mem.set32(sp + 0xC0, n);
    let c = sp + 0x2BC;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(c + k, v);
    }
    let (pa, pb) = (sp + 0x160, sp + 0x1A0);
    for (p, src) in [(pa, a), (pb, b)] {
        for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
            copy4(fx, p + 0x10 * k as u32, src + o);
        }
    }
    fx.mem.set32(sp + 0xB0, n - 1);
    let mut s0 = 0i32;
    while s0 < fx.mem.i32(sp + 0xB0) {
        l2::colour_strip4(fx, d, c, 1);
        let al = i32::from(fx.mem.read_u8(t + 0x2B));
        let m = fx.mem.i32(sp + 0xC0);
        let q = fx.div(al, m).0;
        let v = fx.mem.read_u8(c + 3).wrapping_sub(q as u8);
        fx.mem.write_u8(c + 3, v);
        l2::colour_strip4(fx, d, c, 2);
        let off = (0x10 * (s0 - 1)) as u32;
        let (qa, qb, ends) = if s0 != 0 { (a.wrapping_add(off), b.wrapping_add(off), (0x1E0u32, 0x1F0u32)) } else { (pa, pb, (0x200, 0x210)) };
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            spline(fx, sp + 0x120, qa, f20, 0, [0.0; 4]);
            spline(fx, sp + 0x140, qb, f20, 0, [0.0; 4]);
            f20 = ee::add(f20, 0.25);
            spline(fx, sp + ends.0, qa, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x130, sp + ends.0);
            spline(fx, sp + ends.1, qb, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x150, sp + ends.1);
            for (to, from) in [(0xE0u32, 0x120u32), (0xF0, 0x130), (0x100, 0x140), (0x110, 0x150)] {
                for k in 0..3 {
                    let w = fx.mem.read_u32(sp + from + 4 * k);
                    fx.mem.write_u32(sp + to + 4 * k, w);
                }
            }
            if l2::project(fx, sp + 0xE0, 0x7000_0000, 0, d, sp + 0xD0) {
                fx.submit(d, crate::exe::OT, 0);
            }
        }
        s0 += 1;
    }
    fx.leave(0x2C0);
}

/// d4890e2ea1cf [6 B records: 56 / 57 / 128 / 353 / 430 / 513; card 56
/// 0x1E01320, 272 words]: `ribbon_cross(D, task, a)`: an
/// untextured gouraud ribbon (0x109DD0, 0x108270 which 0 from +0x28,
/// 0x109BC0 blend 0, 0x109DD0 again) along the vec4 history `a` (i16
/// task +6 = n points) drawn as four flat strips per curve piece, each
/// pair c, c, d, d (the points at t and t + 1/4, basis 0) with the first
/// and third vertices pushed by w = +0x38 / 10 along +z, -z, +x, -x;
/// per segment s in 0 .. n - 1 four pieces (the first segment on stack
/// copies a0 a0 a1 a2, later ones a + 0x10 (s - 1)); before each piece
/// the near pair (which 1) takes colour +0x28 as it stands and the far
/// pair (which 2) the alpha less +0x2B / 3n (C quotient, LO / HI) clamped
/// at 0. Keyed by the mean w (0x1A2C70) into 0x206E40's table. A 0x230
/// frame (descriptor sp + 0x198, colour sp + 0x22C, mean w sp + 0x160)
/// [F].
pub fn ribbon_cross(fx: &mut Fx, t: u32, a: u32) {
    use crate::fxlib2 as l2;
    let sp = fx.enter(0x230);
    let w = ee::div(fx.mem.i16(t + 0x38) as f32, 10.0);
    let d = sp + 0x198;
    l2::fullscreen4(fx, d);
    l2::colour_strip4(fx, d, t + 0x28, 0);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    let c = sp + 0x22C;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(c + k, v);
    }
    let n = fx.mem.i16(t + 6);
    let n3 = 3 * n;
    let pa = sp + 0xE0;
    for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
        copy4(fx, pa + 0x10 * k as u32, a + o);
    }
    fx.mem.set32(sp + 0xB0, n - 1);
    let mut s0 = 0i32;
    while s0 < fx.mem.i32(sp + 0xB0) {
        let (q, end) = if s0 != 0 { (a.wrapping_add((0x10 * (s0 - 1)) as u32), 0x170u32) } else { (pa, 0x180) };
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            spline(fx, sp + 0xC0, q, f20, 0, [0.0; 4]);
            f20 = ee::add(f20, 0.25);
            spline(fx, sp + end, q, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0xD0, sp + end);
            l2::colour_strip4(fx, d, c, 1);
            let al = i32::from(fx.mem.read_u8(t + 0x2B));
            let dq = fx.div(al, n3).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(c + 3)) - dq;
            fx.mem.write_u8(c + 3, if v > 0 { v as u8 } else { 0 });
            l2::colour_strip4(fx, d, c, 2);
            for s1 in 0..4 {
                for (to, from) in [(0x120u32, 0xC0u32), (0x130, 0xC0), (0x140, 0xD0), (0x150, 0xD0)] {
                    for k in 0..3 {
                        let v = fx.mem.read_u32(sp + from + 4 * k);
                        fx.mem.write_u32(sp + to + 4 * k, v);
                    }
                }
                let (o, sign) = match s1 {
                    0 => (8u32, 1.0f32),
                    1 => (8, -1.0),
                    2 => (0, 1.0),
                    _ => (0, -1.0),
                };
                for v in [0x120u32, 0x140] {
                    let x = fx.mem.f(sp + v + o);
                    let y = if sign > 0.0 { ee::add(x, w) } else { ee::sub(x, w) };
                    fx.mem.setf(sp + v + o, y);
                }
                if l2::project(fx, sp + 0x120, 0x7000_0000, 0, d, sp + 0x160) {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
                    fx.submit(d, crate::exe::OT, key);
                }
            }
        }
        s0 += 1;
    }
    fx.leave(0x230);
}

/// ac8c0a0c18e0 [2 B records: 180 / 216; card 180 0x1E010E0, 328 words]
///: `ribbon_tex1(D, task, a, b)`: [`ribbon_g`]'s ribbon on a
/// textured strip3 (0x109B50, all four RGBAQs +0x28 through 0x108020,
/// UVs D+0x94 in D+0xBE's order (0x108DE0), texture D+0x88 filter 1): per
/// segment the near pair (0x108070 which 1) as the colour stands, the far
/// pair (which 2) with the alpha down by +0x2B / n (wrapping), then four
/// strips a(t), a(t + 1/4), b(t), b(t + 1/4) at key 0; the first segment
/// on stack copies a0 a0 a1 a2 / b0 b0 b1 b2, later ones a / b + 0x10 (s -
/// 1). A 0x2E0 frame (descriptor sp + 0x228, colour sp + 0x2DC, mean w sp
/// + 0xD0) [F].
pub fn ribbon_tex1(fx: &mut Fx, d0: u32, t: u32, a: u32, b: u32) {
    use crate::fxlib2 as l2;
    let sp = fx.enter(0x2E0);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x228;
    l2::desc_strip3(fx, d);
    for (local, to) in [(0x2B8u32, 0x10u32), (0x2C0, 0x28), (0x2C8, 0x40), (0x2D0, 0x58)] {
        rgbaq_to(fx, sp + local, t + 0x28, d + to);
    }
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let rh2 = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, rh2, 1);
    fx.mem.set32(sp + 0xC0, n);
    let c = sp + 0x2DC;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(c + k, v);
    }
    let (pa, pb) = (sp + 0x160, sp + 0x1A0);
    for (p, src) in [(pa, a), (pb, b)] {
        for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
            copy4(fx, p + 0x10 * k as u32, src + o);
        }
    }
    fx.mem.set32(sp + 0xB0, n - 1);
    let mut s0 = 0i32;
    while s0 < fx.mem.i32(sp + 0xB0) {
        l2::colour_strip3(fx, d, c, 1);
        let al = i32::from(fx.mem.read_u8(t + 0x2B));
        let m = fx.mem.i32(sp + 0xC0);
        let q = fx.div(al, m).0;
        let v = fx.mem.read_u8(c + 3).wrapping_sub(q as u8);
        fx.mem.write_u8(c + 3, v);
        l2::colour_strip3(fx, d, c, 2);
        let off = (0x10 * (s0 - 1)) as u32;
        let (qa, qb, ends) = if s0 != 0 { (a.wrapping_add(off), b.wrapping_add(off), (0x1E0u32, 0x1F0u32)) } else { (pa, pb, (0x200, 0x210)) };
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            spline(fx, sp + 0x120, qa, f20, 0, [0.0; 4]);
            spline(fx, sp + 0x140, qb, f20, 0, [0.0; 4]);
            f20 = ee::add(f20, 0.25);
            spline(fx, sp + ends.0, qa, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x130, sp + ends.0);
            spline(fx, sp + ends.1, qb, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x150, sp + ends.1);
            for (to, from) in [(0xE0u32, 0x120u32), (0xF0, 0x130), (0x100, 0x140), (0x110, 0x150)] {
                for k in 0..3 {
                    let w = fx.mem.read_u32(sp + from + 4 * k);
                    fx.mem.write_u32(sp + to + 4 * k, w);
                }
            }
            if l2::project(fx, sp + 0xE0, 0x7000_0000, 0, d, sp + 0xD0) {
                fx.submit(d, crate::exe::OT, 0);
            }
        }
        s0 += 1;
    }
    fx.leave(0x2E0);
}
