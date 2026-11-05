//! One camera-plane quad at a point, hand-ported
//! from b079 (card 437) [F]: e3549c361c29 draws a point set's centre as a
//! textured quad of the look's cell, optionally turned in its plane
//! (`quad`).
//!
//! The look `g` and point set `q` are `pointquad`'s: `g` +0..+0xE the cell
//! (integers with flag 0, else floats +8 / +0xC), +0x30 the scale, +0x34
//! the colour, +0x38 flags (0: integer cells, 5: height over width,
//! 0xA..0xC blend, 0x13 far Z, 0x18 no clamp, 0x1F turned), +0x44 the
//! turn (degrees); `q` +0 the texture, +4 flags (0x14..0x17 vertex k
//! projected), +8 the half size, +0x10 the centre.
use super::pointquad::cell_frame;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, ee_lt, Fx};

/// RGBAQ / UV / XYZ2 x 4 (a textured strip).
const REGS_TEX4: u64 = 0x4214_2142_1421;
/// pi / 180 as the code builds it.
const DEGREE: u32 = 0x3C8E_FA35;

/// The cell's aspect (x, y scales): flag 5 (1, h / w); integer cells
/// (flag 0) the shorter side over the longer on that axis; float cells
/// likewise from +8 / +0xC.
fn aspect(fx: &mut Fx, g: u32) -> (f32, f32) {
    let flags = fx.mem.read_u32(g + 0x38);
    if bit(flags, 5) {
        let (w, h) = (fx.mem.i16(g + 4), fx.mem.i16(g + 6));
        return (1.0, ee::div(ee::from_i32(h), ee::from_i32(w)));
    }
    if bit(flags, 0) {
        let (w, h) = (fx.mem.i16(g + 4), fx.mem.i16(g + 6));
        if h < w {
            (1.0, ee::div(ee::from_i32(h), ee::from_i32(w)))
        } else if w < h {
            (ee::div(ee::from_i32(w), ee::from_i32(h)), 1.0)
        } else {
            (1.0, 1.0)
        }
    } else {
        let (w, h) = (fx.mem.f(g + 8), fx.mem.f(g + 0xC));
        if !ee_le(w, h) {
            (1.0, ee::div(h, w))
        } else if ee_lt(w, h) {
            (ee::div(w, h), 1.0)
        } else {
            (1.0, 1.0)
        }
    }
}

/// e3549c361c29 [13 B records; b079 0x1E00120, 548 words] [F]: `quad(ot, g,
/// q, x) -> 0` (descriptor at sp + 0xB0: 12 values, ALPHA 0x44 / 0x88 /
/// 0x42 / 0x48 by flags 0xC / 0xB / 0xA, the look's colour, the cell
/// (0x104DC0)): the four corners (x -+ a, y +- b) round the centre (a, b
/// = the aspect x half size x scale), turned by +0x44 degrees (0x20AD90 /
/// 0x20AEA0) with flag 0x1F, projected through `x` + 0x30 (0x204D80, XYZ
/// at sp + 0xD0 + 0x18 k, w at sp + 0x90 + 4 k); the texture (filter 0
/// with flag 0x18, else REGION_CLAMP to the integer cell (flag 0) and
/// filter 1); with all four projected keyed far (0x13, key 0) or by the
/// mean w; vertex bits cleared. A 0x140 frame.
pub fn quad(fx: &mut Fx, ot: u32, g: u32, q: u32, x: u32) -> i32 {
    let sp = fx.enter(0x140);
    let d = sp + 0xB0;
    fx.desc_init(d, 0xC, 0x34, REGS_TEX4);
    let flags = fx.mem.read_u32(g + 0x38);
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
    fx.colours4(d, g + 0x34);
    cell_frame(fx, d, g);
    let (ax, ay) = aspect(fx, g);
    let xyz = [sp + 0xD0, sp + 0xE8, sp + 0x100, sp + 0x118];
    let ws = [sp + 0x90, sp + 0x94, sp + 0x98, sp + 0x9C];
    let v = sp + 0xA0;
    let corner = |fx: &mut Fx, k: usize, cx: f32, cy: f32| {
        let px = ee::add(fx.mem.f(q + 0x10), cx);
        fx.mem.setf(v, px);
        let py = ee::add(fx.mem.f(q + 0x14), cy);
        fx.mem.setf(v + 4, py);
        if k == 0 {
            let z = fx.mem.read_u32(q + 0x18);
            fx.mem.write_u32(v + 8, z);
        }
        let m = fx.read_m4(x + 0x30);
        if fx.project_at(v, &m, xyz[k], ws[k]) {
            fx.flag(q + 4, 0x14 + k as u32);
        }
    };
    if bit(fx.mem.read_u32(g + 0x38), 0x1F) {
        let turn = fx.mem.i32(g + 0x44);
        let (_, deg) = fx.div(turn, 0x168);
        let a = ee::mul(f32::from_bits(DEGREE), ee::from_i32(deg));
        let c = fx.cosf(a);
        let s = fx.sinf(a);
        let size = ee::mul(fx.mem.f(q + 8), fx.mem.f(g + 0x30));
        let b = ee::mul(ay, size);
        let a = ee::mul(ax, size);
        let na = ee::neg(a);
        let (f26, f27) = (ee::mul(c, na), ee::mul(s, na));
        let f24 = ee::mul(s, b);
        let f25 = ee::mul(c, b);
        corner(fx, 0, ee::sub(f26, f24), ee::add(f27, f25));
        let f28 = ee::mul(c, a);
        let x1 = ee::sub(f28, f24);
        let f24 = ee::mul(s, a);
        corner(fx, 1, x1, ee::add(f24, f25));
        let nb = ee::neg(b);
        let (f20, f21) = (ee::mul(s, nb), ee::mul(c, nb));
        corner(fx, 2, ee::sub(f26, f20), ee::add(f27, f21));
        corner(fx, 3, ee::sub(f28, f20), ee::add(f24, f21));
    } else {
        for (k, (sx, sy)) in [(false, true), (true, true), (false, false), (true, false)].into_iter().enumerate() {
            let ox = ee::mul(ax, ee::mul(fx.mem.f(q + 8), fx.mem.f(g + 0x30)));
            let px = if sx { ee::add(fx.mem.f(q + 0x10), ox) } else { ee::sub(fx.mem.f(q + 0x10), ox) };
            fx.mem.setf(v, px);
            let oy = ee::mul(ay, ee::mul(fx.mem.f(q + 8), fx.mem.f(g + 0x30)));
            let py = if sy { ee::add(fx.mem.f(q + 0x14), oy) } else { ee::sub(fx.mem.f(q + 0x14), oy) };
            fx.mem.setf(v + 4, py);
            if k == 0 {
                let z = fx.mem.read_u32(q + 0x18);
                fx.mem.write_u32(v + 8, z);
            }
            let m = fx.read_m4(x + 0x30);
            if fx.project_at(v, &m, xyz[k], ws[k]) {
                fx.flag(q + 4, 0x14 + k as u32);
            }
        }
    }
    let flags = fx.mem.read_u32(g + 0x38);
    let rh2 = fx.mem.read_u32(q);
    if bit(flags, 0x18) {
        fx.desc_texture(d, rh2, 0);
    } else {
        if bit(flags, 0) {
            let h = |fx: &mut Fx, o: u32| i64::from(fx.mem.i16(g + o));
            let clamp = (h(fx, 0) << 4 | 0xA) | ((h(fx, 0) + h(fx, 4) - 1) << 14) | (h(fx, 2) << 24) | ((h(fx, 2) + h(fx, 6) - 1) << 34);
            fx.desc_clamp(d, clamp as u64);
        }
        fx.desc_texture(d, rh2, 1);
    }
    let qf = fx.mem.read_u32(q + 4);
    if (0x14..0x18).all(|b| bit(qf, b)) {
        if bit(fx.mem.read_u32(g + 0x38), 0x13) {
            crate::fxlib2::far_z(fx, &[xyz[3], xyz[2], xyz[1], xyz[0]]);
            fx.submit(d, ot, 0);
        } else {
            let s = ee::add(fx.mem.f(ws[0]), fx.mem.f(ws[1]));
            let s = ee::add(fx.mem.f(ws[2]), s);
            let s = ee::add(fx.mem.f(ws[3]), s);
            let key = ee::fixunssfsi(ee::div(s, 4.0)) as i32;
            fx.submit(d, ot, key);
        }
    }
    let f = fx.mem.read_u32(q + 4) & 0xFF0F_FFFF;
    fx.mem.write_u32(q + 4, f);
    fx.leave(0x140);
    0
}
