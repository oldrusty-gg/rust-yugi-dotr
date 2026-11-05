//! A ring of turned blades, hand-ported from
//! b742 (card 526) [F]: 2195e2352389 gives each blade a random turn and
//! the group its six template vertices (`setup`), 323fd061980f draws the
//! blades round a circle, each as two textured quads whose u scrolls with
//! the blade's age (`draw`).
//!
//! A group `g`: +0 the texture, +4 flags (0xA / 0xB blend, 0x12 / 0x13
//! keys, 0x1E ageless), +8 the colour (B, G, R, A), +0xC the radius, +0x10
//! / +0x12 / +0x14 / +0x16 the cell (u, v, w, h), +0x20 + 16 k the six
//! vertices, +0x80 the blades (16 bytes: turn x / y / z i32, alpha +0xC,
//! age i16 +0xE), +0x84 the height, +0x88 the width, +0x8C / +0x90 the
//! growth in y / x per age, +0x94 the arc (16-bit angle), +0x98 tick,
//! +0x9A count, +0x9C drawn count, +0x9E its growth a step.
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};

/// RGBAQ / UV / XYZ2 x 4 (a textured strip).
const REGS_TEX4: u64 = 0x4214_2142_1421;

/// 2195e2352389 [9 B records; b742 0x1E01940, 104 words] [F]:
/// `setup(g, rh2) -> 0`: the record's six-vertex template (rodata at the
/// record-relative `lui` / `addiu` +0xC / +0x14, copied to sp + 0x40) with
/// x = -/+ width / 2 and y = height at vertices 0 / 2 / 4 / 5, its xyz
/// into +0x20; texture `rh2`; each blade turned by (-rand % 0x2000, rand &
/// 0xFFFF, 0), age 0, alpha +0xB. A 0xA0 frame.
pub fn setup(fx: &mut Fx, at: u32, g: u32, rh2: u32) -> i32 {
    let sp = fx.enter(0xA0);
    let t = rel(fx, at, 0xC, 0x14);
    let l = sp + 0x40;
    for k in 0..6u32 {
        for j in (0..16).step_by(8) {
            let v = fx.mem.read_u64(t + 16 * k + j);
            fx.mem.write_u64(l + 16 * k + j, v);
        }
    }
    let half = |fx: &mut Fx, neg: bool| {
        let w = fx.mem.f(g + 0x88);
        ee::div(if neg { ee::neg(w) } else { w }, 2.0)
    };
    let v = half(fx, true);
    fx.mem.setf(sp + 0x40, v);
    let h = fx.mem.read_u32(g + 0x84);
    fx.mem.write_u32(sp + 0x44, h);
    let v = half(fx, true);
    fx.mem.setf(sp + 0x50, v);
    let h = fx.mem.read_u32(g + 0x84);
    fx.mem.write_u32(sp + 0x64, h);
    let v = half(fx, false);
    fx.mem.setf(sp + 0x80, v);
    let h = fx.mem.read_u32(g + 0x84);
    fx.mem.write_u32(sp + 0x84, h);
    let v = half(fx, false);
    fx.mem.setf(sp + 0x90, v);
    fx.mem.write_u32(g, rh2);
    for k in 0..6u32 {
        for c in 0..3u32 {
            let v = fx.mem.read_u32(l + 16 * k + 4 * c);
            fx.mem.write_u32(g + 0x20 + 16 * k + 4 * c, v);
        }
    }
    let mut k = 0;
    while k < fx.mem.i16(g + 0x9A) {
        let off = 16 * k as u32;
        let r = fx.rand_from(off, at + 0xF8);
        let x = r.wrapping_neg() % 0x2000;
        let p = fx.mem.read_u32(g + 0x80) + off;
        fx.mem.set32(p, x);
        let r = fx.rand_from(off, at + 0x120);
        let y = r & 0xFFFF;
        k += 1;
        let p = fx.mem.read_u32(g + 0x80) + off;
        fx.mem.set32(p + 4, y);
        let p = fx.mem.read_u32(g + 0x80) + off;
        fx.mem.write_u32(p + 8, 0);
        let p = fx.mem.read_u32(g + 0x80) + off;
        fx.mem.set16(p + 0xE, 0);
        let a = fx.mem.read_u8(g + 0xB);
        let p = fx.mem.read_u32(g + 0x80) + off;
        fx.mem.write_u8(p + 0xC, a);
    }
    fx.leave(0xA0);
    0
}

/// 0x20B460(a, b) [F]: `a` = `a` x `b` (every row of `a` through `b`).
pub fn mul_after(fx: &mut Fx, a: u32, b: u32) {
    let ma = fx.read_m4(a);
    let mb = fx.read_m4(b);
    let r: [[f32; 4]; 4] = std::array::from_fn(|r| Fx::vu_apply(&mb, ma[r]));
    fx.write_m4(a, &r);
}

/// 323fd061980f [9 B records; b742 0x1E01AE0, 452 words] [F]:
/// `draw(ot, g, x) -> 0` (descriptor at sp + 0x1E0: a textured strip,
/// REGION_CLAMP to the cell, ALPHA 0x88 / 0x42 / 0x48 by flags 0xB / 0xA,
/// the group's colour): the drawn count capped at the count; blade c at
/// the angle (arc - 0x8000) / 2 - c (arc / count) round the circle of
/// radius +0xC, turned by its own angles (0x20C0B0), through `x` + 0x30
/// (0x20B460); its vertices grown by the age (x of 0, 1, 4, 5 by +0x90,
/// y of 0, 2, 4 by +0x8C); two quads (vertices 0..3 and 2..5, the alpha on
/// the first's second / the second's first corner) with u from age % w
/// (+ w / 2 for the second), projected (0x204F70, w mean at sp + 0x1D0)
/// and keyed far (0x13, key 0), last (0x12) or by the mean w; the age +1
/// without flag 0x1E. Then tick +1, drawn count + growth. A 0x270 frame.
pub fn draw(fx: &mut Fx, ot: u32, g: u32, x: u32) -> i32 {
    let sp = fx.enter(0x270);
    fx.mem.write_u32(sp + 0x100, x);
    let mut count = fx.mem.i16(g + 0x9C);
    let n = fx.mem.i16(g + 0x9A);
    if n < count {
        fx.mem.set16(g + 0x9C, n);
        count = n;
    }
    fx.mem.set32(sp + 0xF0, count);
    let arc = fx.mem.i32(g + 0x94);
    let (q, _) = fx.div(arc, n);
    let d = sp + 0x1E0;
    fx.mem.set32(sp + 0xA0, q.wrapping_neg());
    let start = 0x8000i32.wrapping_sub(arc).wrapping_neg() >> 1;
    fx.mem.set32(sp + 0xB0, start);
    fx.desc_init(d, 0xC, 0x34, REGS_TEX4);
    let h = |fx: &mut Fx, o: u32| i64::from(fx.mem.i16(g + o));
    let clamp = ((h(fx, 0x14) - 1) << 4 | 0xB) | (h(fx, 0x10) << 14) | (h(fx, 0x12) << 24) | ((h(fx, 0x12) + h(fx, 0x16) - 1) << 34);
    fx.desc_clamp(d, clamp as u64);
    let rh2 = fx.mem.read_u32(g);
    fx.desc_texture(d, rh2, 0);
    let flags = fx.mem.read_u32(g + 4);
    let alpha = if bit(flags, 0xB) { 0x88 } else if bit(flags, 0xA) { 0x42 } else { 0x48 };
    fx.desc_alpha(d, 0x80_0000_0000 | alpha);
    fx.colours4(d, g + 8);
    fx.mem.set32(sp + 0xC0, 0);
    if count > 0 {
        fx.mem.write_u32(sp + 0xE0, sp + 0x1FA);
        fx.mem.set32(sp + 0xD0, 0);
        let mut off = 0u32;
        while fx.mem.i32(sp + 0xC0) < fx.mem.i32(sp + 0xF0) {
            let a = fx.mem.i32(sp + 0xB0).wrapping_add(fx.mem.i32(sp + 0xD0));
            let s = fx.tsin(a);
            let v = ee::mul(fx.mem.f(g + 0xC), s);
            fx.mem.setf(sp + 0x118, v);
            let c = fx.tcos(a);
            let v = ee::mul(fx.mem.f(g + 0xC), c);
            fx.mem.setf(sp + 0x110, v);
            fx.mem.write_u32(sp + 0x114, 0);
            let blade = |fx: &mut Fx| fx.mem.read_u32(g + 0x80) + off;
            for k in 0..3u32 {
                let p = blade(fx);
                let w = fx.mem.read_u32(p + 4 * k);
                fx.mem.write_u32(sp + 0x120 + 4 * k, w);
            }
            super::exe::rot_trans_into(fx, sp + 0x130, sp + 0x120, sp + 0x110);
            mul_after(fx, sp + 0x130, x + 0x30);
            for k in 0..6u32 {
                let (src, dst) = (g + 0x20 + 16 * k, sp + 0x170 + 16 * k);
                for c in 0..4u32 {
                    let w = fx.mem.read_u32(src + 4 * c);
                    fx.mem.write_u32(dst + 4 * c, w);
                }
                if k != 2 && k != 3 {
                    let p = blade(fx);
                    let age = ee::from_i32(i32::from(fx.mem.i16(p + 0xE)));
                    let v = ee::add(fx.mem.f(src), ee::mul(fx.mem.f(g + 0x90), age));
                    fx.mem.setf(dst, v);
                }
                if k & 1 == 0 {
                    let p = blade(fx);
                    let age = ee::from_i32(i32::from(fx.mem.i16(p + 0xE)));
                    let v = ee::add(fx.mem.f(src + 4), ee::mul(fx.mem.f(g + 0x8C), age));
                    fx.mem.setf(dst + 4, v);
                }
            }
            for q in 0..2u32 {
                let p = blade(fx);
                let a = fx.mem.read_u8(p + 0xC);
                if q != 0 {
                    fx.mem.write_u8(sp + 0x1F3, a);
                    fx.mem.write_u8(sp + 0x23B, 0);
                    fx.mem.write_u8(sp + 0x223, 0);
                    fx.mem.write_u8(sp + 0x20B, 0);
                } else {
                    fx.mem.write_u8(sp + 0x23B, 0);
                    fx.mem.write_u8(sp + 0x20B, 0);
                    fx.mem.write_u8(sp + 0x1F3, 0);
                    fx.mem.write_u8(sp + 0x223, a);
                }
                let w = i32::from(fx.mem.i16(g + 0x14));
                let p = blade(fx);
                let age = i32::from(fx.mem.i16(p + 0xE));
                let f = fx.mem.read_u16(d + 2);
                let (_, rem) = fx.div(age, w);
                fx.mem.write_u16(d + 2, f | 0x800);
                let m = fx.mult(w >> 1, q as i32);
                let u = rem.wrapping_add(m) as i16 as i32;
                fx.set_uv(sp + 0x1F8, u);
                let v = i32::from(fx.mem.i16(g + 0x12));
                fx.set_uv(sp + 0x1FA, v);
                fx.set_uv(sp + 0x210, u);
                let v = i32::from(fx.mem.i16(g + 0x12)) + i32::from(fx.mem.i16(g + 0x16)) - 1;
                fx.set_uv(sp + 0x212, v);
                let u2 = u + (i32::from(fx.mem.i16(g + 0x14)) >> 1);
                fx.set_uv(sp + 0x228, u2);
                let v = i32::from(fx.mem.i16(g + 0x12));
                fx.set_uv(sp + 0x22A, v);
                let u2 = u + (i32::from(fx.mem.i16(g + 0x14)) >> 1);
                fx.set_uv(sp + 0x240, u2);
                let v = i32::from(fx.mem.i16(g + 0x12)) + i32::from(fx.mem.i16(g + 0x16)) - 1;
                fx.set_uv(sp + 0x242, v);
                if !crate::fxlib2::project(fx, sp + 0x170 + 0x20 * q, 0x7000_0000, sp + 0x130, d, sp + 0x1D0) {
                    continue;
                }
                let flags = fx.mem.read_u32(g + 4);
                if bit(flags, 0x13) {
                    crate::fxlib2::far_z(fx, &[sp + 0x248, sp + 0x230, sp + 0x218, sp + 0x200]);
                    fx.submit(d, ot, 0);
                } else if bit(flags, 0x12) {
                    fx.submit(d, ot, 0xFF_FFFF);
                } else {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x1D0)) as i32;
                    fx.submit(d, ot, key);
                }
            }
            if !bit(fx.mem.read_u32(g + 4), 0x1E) {
                let p = blade(fx);
                let a = fx.mem.i16(p + 0xE) + 1;
                fx.mem.set16(p + 0xE, a as i32);
            }
            let a = fx.mem.i32(sp + 0xD0).wrapping_add(fx.mem.i32(sp + 0xA0));
            fx.mem.set32(sp + 0xD0, a);
            let c = fx.mem.i32(sp + 0xC0) + 1;
            fx.mem.set32(sp + 0xC0, c);
            off += 16;
        }
    }
    let t = fx.mem.i16(g + 0x98) + 1;
    fx.mem.set16(g + 0x98, t as i32);
    let c = fx.mem.i16(g + 0x9C) + fx.mem.i16(g + 0x9E);
    fx.mem.set16(g + 0x9C, c as i32);
    fx.leave(0x270);
    0
}
