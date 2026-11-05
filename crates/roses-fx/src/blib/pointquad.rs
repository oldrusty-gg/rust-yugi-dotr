//! Quads at a set of points, hand-ported from b028
//! (card 213) [F]: fd50417ef0a5 lays out a point set (`setup`),
//! af323713be0b draws it as camera-plane quads or strips round its centre
//! (`draw`, with the executable's cell animation 0x104DC0, `cell_frame`)
//! and 9006fa9b5875 draws a cloud of 60 drifting squares (`squares`).
//!
//! A point set `q`: +0 the texture, +4 flags (0x14..0x17 vertex k
//! projected), +8 the mode, +0xC the half size, +0x10 the centre (vec4),
//! +0x20 + 16 k the points. Its look `g`: +0 / +2 the cell (u, v), +4 / +6
//! its size, +8 / +0xA the sheet origin, +0xC / +0xE the sheet size (or
//! the float cell +0..+0xC without flag 0), +0x34 the colour, +0x38 flags
//! (0: integer cells, 1: by column, 0xA / 0xB blend, 0x13 far Z, 0x18 no
//! clamp), +0x48 tick, +0x4A ticks a frame, +0x4C last frame, +0x4E loop
//! length, +0x50 frame, +0x52 999 to loop (else 1 when done).
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, ee_lt, Fx};
use crate::fxlib::{ALPHA_42, ALPHA_88};

/// RGBAQ / ST / XYZF2 x 4 (a textured strip).
const REGS_TEX4: u64 = 0x4214_2142_1421;
/// The fixed blend 0x48 with FIX 0x80.
const ALPHA_48: u64 = 0x80_0000_0048;

/// fd50417ef0a5 [20 B records; b028 0x1E00120, 208 words] [F]:
/// `setup(g, q, rh2, size) -> 0`: `q` cleared at +0x10..+0x18, half size
/// `size` / 2, texture `rh2`; the cell's aspect (`g`'s integer cell size
/// +4 / +6 with flag 0, else floats +8 / +0xC: the shorter side over the
/// longer as the y scale, or the longer over the shorter as the x scale);
/// mode 3: 8 random points (rand % 30 - 15, half size, rand % 30 - 15),
/// else the record's template (8 vec4 at the record-relative `lui` /
/// `addiu` +0x180 / +0x184 for modes 5 / 6, else +0x210 / +0x214, copied
/// to sp + 0x40 / 0xC0) x the half size x the aspect. A 0x140 frame.
pub fn setup(fx: &mut Fx, at: u32, g: u32, q: u32, rh2: u32, size: f32) -> i32 {
    let half = ee::div(size, 2.0);
    let sp = fx.enter(0x140);
    for k in [0x18u32, 0x14, 0x10] {
        fx.mem.write_u32(q + k, 0);
    }
    fx.mem.setf(q + 0xC, half);
    fx.mem.write_u32(q, rh2);
    let (sx, sy) = if bit(fx.mem.read_u32(g + 0x38), 0) {
        let (a, b) = (fx.mem.i16(g + 4), fx.mem.i16(g + 6));
        if b < a {
            (1.0, ee::div(ee::from_i32(b), ee::from_i32(a)))
        } else if a < b {
            (ee::div(ee::from_i32(b), ee::from_i32(a)), 1.0)
        } else {
            (1.0, 1.0)
        }
    } else {
        let (p, r) = (fx.mem.f(g + 8), fx.mem.f(g + 0xC));
        if !ee_le(p, r) {
            (1.0, ee::div(r, p))
        } else if ee_lt(p, r) {
            (ee::div(r, p), 1.0)
        } else {
            (1.0, 1.0)
        }
    };
    let mode = fx.mem.i32(q + 8);
    if mode == 3 {
        for k in 0..8u32 {
            let p = q + 16 * k;
            let r = fx.rand_from(p, at + 0x2B0);
            let (_, h) = fx.div(r, 0x1E);
            fx.mem.setf(p + 0x20, ee::from_i32(h - 0xF));
            let c = fx.mem.read_u32(q + 0xC);
            fx.mem.write_u32(p + 0x24, c);
            let r = fx.rand_from(p, at + 0x2E4);
            let (_, h) = fx.div(r, 0x1E);
            fx.mem.setf(p + 0x28, ee::from_i32(h - 0xF));
        }
    } else {
        let (template, local) = if mode == 5 || mode == 6 { (rel(fx, at, 0x180, 0x184), sp + 0x40) } else { (rel(fx, at, 0x210, 0x214), sp + 0xC0) };
        for k in 0..8u32 {
            let v = fx.read_v4(template + 16 * k);
            fx.write_v4(local + 16 * k, v);
        }
        for k in 0..8u32 {
            let (t, p) = (local + 16 * k, q + 16 * k);
            let x = ee::mul(sx, ee::mul(fx.mem.f(q + 0xC), fx.mem.f(t)));
            fx.mem.setf(p + 0x20, x);
            let y = ee::mul(sy, ee::mul(fx.mem.f(q + 0xC), fx.mem.f(t + 4)));
            fx.mem.setf(p + 0x24, y);
            let z = ee::mul(fx.mem.f(q + 0xC), fx.mem.f(t + 8));
            fx.mem.setf(p + 0x28, z);
        }
    }
    fx.leave(0x140);
    0
}

/// The executable's 0x1A2B80 (fixsfsi) of 1 / `x`, as an i16.
fn inverse_i16(x: f32) -> i32 {
    i32::from(ee::fixsfsi(ee::div(1.0, x)) as i16)
}

/// 0x104DC0(desc, g) [F; 0x105260's sibling for the point-set look]: the
/// cell of frame +0x50 (flag 0: integer cells by column (flag 1) or by row
/// of the sheet +0xC / +0xE in steps +4 / +6 from the origin +8 / +0xA,
/// into +0 / +2, UVs (u, v), (u + w, v), (u, v + h), (u + w, v + h), flag
/// 0x800; else float cells: steps 1 / +8 and 1 / +0xC (0x1A2B80) as the
/// sheet's columns / rows, ST from +0..+0xC); then the tick +0x48 (frame
/// up past +0x4A ticks; past the last frame +0x4C looped back by +0x4E
/// when +0x52 is 999, else +0x52 = 1).
pub fn cell_frame(fx: &mut Fx, d: u32, g: u32) {
    fx.enter(0x50);
    let flags = fx.mem.read_u32(g + 0x38);
    if flags & 1 != 0 {
        if flags & 2 != 0 {
            let (w, sheet, frame, ox) = (fx.mem.i16(g + 4), fx.mem.i16(g + 0xC), fx.mem.i16(g + 0x50), fx.mem.i16(g + 8));
            let (cols, _) = fx.div(sheet, w);
            let (_, col) = fx.div(frame, cols);
            let u = fx.mult(w, col);
            fx.mem.set16(g, ox.wrapping_add(u));
            let (sheet, w, frame, h, oy) = (fx.mem.i16(g + 0xC), fx.mem.i16(g + 4), fx.mem.i16(g + 0x50), fx.mem.i16(g + 6), fx.mem.i16(g + 0xA));
            let cols = div1(sheet, w).0;
            let row = div1(frame, cols).0;
            fx.mem.set16(g + 2, oy.wrapping_add(h.wrapping_mul(row)));
        } else {
            let (sheet, h, frame, w, ox) = (fx.mem.i16(g + 0xE), fx.mem.i16(g + 6), fx.mem.i16(g + 0x50), fx.mem.i16(g + 4), fx.mem.i16(g + 8));
            let (rows, _) = fx.div(sheet, h);
            let (c, _) = fx.div(frame, rows);
            let u = fx.mult(w, c);
            fx.mem.set16(g, ox.wrapping_add(u));
            let (h, sheet, frame, oy) = (fx.mem.i16(g + 6), fx.mem.i16(g + 0xE), fx.mem.i16(g + 0x50), fx.mem.i16(g + 0xA));
            let rows = div1(sheet, h).0;
            let (_, r) = fx.div(frame, rows);
            fx.mem.set16(g + 2, oy.wrapping_add(h.wrapping_mul(r)));
        }
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        let (u, v) = (fx.mem.i16(g), fx.mem.i16(g + 2));
        fx.set_uv(d + 0x18, u);
        fx.set_uv(d + 0x1A, v);
        let w = fx.mem.i16(g + 4);
        fx.set_uv(d + 0x30, u + w);
        fx.set_uv(d + 0x32, v);
        fx.set_uv(d + 0x48, u);
        let h = fx.mem.i16(g + 6);
        fx.set_uv(d + 0x4A, v + h);
        let (u, w) = (fx.mem.i16(g), fx.mem.i16(g + 4));
        fx.set_uv(d + 0x60, u + w);
        let (v, h) = (fx.mem.i16(g + 2), fx.mem.i16(g + 6));
        fx.set_uv(d + 0x62, v + h);
    } else {
        if flags & 2 != 0 {
            let w = fx.mem.f(g + 8);
            let frame = fx.mem.i16(g + 0x50);
            let inv = ee::div(1.0, w);
            fx.fix_residue(inv, 0x0010_5050);
            let cols = inverse_i16(w);
            let (_, col) = fx.div(frame, cols);
            fx.mem.setf(g, ee::mul(w, ee::from_i32(col)));
            let w = fx.mem.f(g + 8);
            fx.fix_residue(ee::div(1.0, w), 0x0010_5094);
            let cols = inverse_i16(w);
            let frame = fx.mem.i16(g + 0x50);
            let (row, _) = fx.div(frame, cols);
            let v = ee::mul(fx.mem.f(g + 0xC), ee::from_i32(row));
            fx.mem.setf(g + 4, v);
        } else {
            let h = fx.mem.f(g + 0xC);
            fx.fix_residue(ee::div(1.0, h), 0x0010_50F0);
            let rows = inverse_i16(h);
            let frame = fx.mem.i16(g + 0x50);
            let (c, _) = fx.div(frame, rows);
            let u = ee::mul(fx.mem.f(g + 8), ee::from_i32(c));
            fx.mem.setf(g, u);
            let h = fx.mem.f(g + 0xC);
            let frame = fx.mem.i16(g + 0x50);
            fx.fix_residue(ee::div(1.0, h), 0x0010_5148);
            let rows = inverse_i16(h);
            let (_, r) = fx.div(frame, rows);
            fx.mem.setf(g + 4, ee::mul(h, ee::from_i32(r)));
        }
        let (u, v) = (fx.mem.read_u32(g), fx.mem.read_u32(g + 4));
        fx.mem.write_u32(d + 0x18, u);
        fx.mem.write_u32(d + 0x1C, v);
        let u2 = ee::add(fx.mem.f(g), fx.mem.f(g + 8));
        fx.mem.setf(d + 0x30, u2);
        fx.mem.write_u32(d + 0x34, v);
        fx.mem.write_u32(d + 0x48, u);
        let v2 = ee::add(fx.mem.f(g + 4), fx.mem.f(g + 0xC));
        fx.mem.setf(d + 0x4C, v2);
        let v = ee::add(fx.mem.f(g), fx.mem.f(g + 8));
        fx.mem.setf(d + 0x60, v);
        let v = ee::add(fx.mem.f(g + 4), fx.mem.f(g + 0xC));
        fx.mem.setf(d + 0x64, v);
    }
    let t = fx.mem.i16(g + 0x48) + 1;
    fx.mem.set16(g + 0x48, t);
    if fx.mem.i16(g + 0x4A) < fx.mem.i16(g + 0x48) {
        fx.mem.set16(g + 0x48, 0);
        let f = fx.mem.i16(g + 0x50) + 1;
        fx.mem.set16(g + 0x50, f);
        let last = fx.mem.i16(g + 0x4C);
        if last < fx.mem.i16(g + 0x50) {
            if fx.mem.i16(g + 0x52) == 0x3E7 {
                let f = last - fx.mem.i16(g + 0x4E) + 1;
                fx.mem.set16(g + 0x50, f);
            } else {
                fx.mem.set16(g + 0x52, 1);
            }
        }
    }
    fx.leave(0x50);
}

/// The LO1 pipeline's division (`div1`; the main LO / HI kept; by zero
/// (0, 0) as `fxlib`'s [H: the original keeps the old LO1 / HI1]).
fn div1(a: i32, b: i32) -> (i32, i32) {
    if b == 0 {
        (0, 0)
    } else {
        (a.wrapping_div(b), a.wrapping_rem(b))
    }
}

/// The draw's descriptor (sp + 0xC0) set up: 12 values, blend by `g`'s
/// flags 0xB / 0xA (0x88 / 0x42, else 0x48), colour +0x34, the cell
/// (0x104DC0).
fn draw_desc(fx: &mut Fx, d: u32, g: u32) {
    fx.desc_init(d, 12, 0x34, REGS_TEX4);
    let flags = fx.mem.read_u32(g + 0x38);
    let a = if bit(flags, 0xB) {
        ALPHA_88
    } else if bit(flags, 0xA) {
        ALPHA_42
    } else {
        ALPHA_48
    };
    fx.desc_alpha(d, a);
    fx.colours4(d, g + 0x34);
    cell_frame(fx, d, g);
}

/// The vertex at `v` projected through `x` + 0x30 into `xyz`, its w at
/// `w`; vertex bit `k` of `q`'s flags on success.
fn vertex(fx: &mut Fx, q: u32, x: u32, v: u32, xyz: u32, w: u32, k: u32) {
    let m = fx.read_m4(x + 0x30);
    if fx.project_at(v, &m, xyz, w) {
        fx.flag(q + 4, 0x14 + k);
    }
}

/// sp + 0xB0 = `base` + `off` (xyz), then that local projected.
fn offset_vertex(fx: &mut Fx, sp: u32, q: u32, x: u32, off: u32, xyz: u32, w: u32, k: u32) {
    for c in 0..3u32 {
        let v = ee::add(fx.mem.f(q + 0x10 + 4 * c), fx.mem.f(off + 4 * c));
        fx.mem.setf(sp + 0xB0 + 4 * c, v);
    }
    vertex(fx, q, x, sp + 0xB0, xyz, w, k);
}

/// The texture (filter 0 with flag 0x18; else REGION_CLAMP from the cell
/// with flag 0 and filter 1) and, when all four vertices projected, the
/// quad submitted (far Z key 0 with flag 0x13, else the mean-w key).
fn texture_submit(fx: &mut Fx, sp: u32, ot: u32, g: u32, q: u32) -> bool {
    let d = sp + 0xC0;
    let flags = fx.mem.read_u32(g + 0x38);
    let rh2 = fx.mem.read_u32(q);
    if bit(flags, 0x18) {
        fx.desc_texture(d, rh2, 0);
    } else {
        if bit(flags, 0) {
            let (rx, ry, rw, rh) = (fx.mem.i16(g), fx.mem.i16(g + 2), fx.mem.i16(g + 4), fx.mem.i16(g + 6));
            let clamp = ((rx as i64) << 4) as u64 | 0xA | (((rx + rw - 1) as i64 as u64) << 14) | ((ry as i64 as u64) << 24) | (((ry + rh - 1) as i64 as u64) << 34);
            fx.desc_clamp(d, clamp);
        }
        fx.desc_texture(d, rh2, 1);
    }
    if !fx.all_projected(q + 4) {
        return false;
    }
    if bit(fx.mem.read_u32(g + 0x38), 0x13) {
        fx.far_z(&[sp + 0xE0, sp + 0xF8, sp + 0x110, sp + 0x128]);
        fx.submit(d, ot, 0);
    } else {
        let key = fx.mean_key4([sp + 0xA0, sp + 0xA4, sp + 0xA8, sp + 0xAC]);
        fx.submit(d, ot, key);
    }
    true
}

/// af323713be0b [20 B records; b028 0x1E00460, 760 words] [F]: `draw(ot,
/// g, q, x) -> 0` (`draw_desc` at sp + 0xC0, vertices at sp + 0xE0 + 0x18
/// k, w at sp + 0xA0 + 4 k): mode 3: each of the 8 points a quad of half
/// size +0xC in the xy plane (vertex bits cleared only after a submit);
/// modes 1 / 5: 4 quads round the centre (points 2 k + 1, 2 or 4, 3 or 1
/// / 0 as offsets, then the centre itself); modes 2 / 6: points 2 k + 1,
/// 2 k + 2 (k = 3: the raw point 0, not offset) and the raw point 2 k with
/// the centre (the offset locals still written); then the vertex bits
/// cleared. A 0x150 frame.
pub fn draw(fx: &mut Fx, ot: u32, g: u32, q: u32, x: u32) -> i32 {
    let sp = fx.enter(0x150);
    let d = sp + 0xC0;
    draw_desc(fx, d, g);
    let xyz = [sp + 0xE0, sp + 0xF8, sp + 0x110, sp + 0x128];
    let ws = [sp + 0xA0, sp + 0xA4, sp + 0xA8, sp + 0xAC];
    let mode = fx.mem.i32(q + 8);
    if mode == 3 {
        for k in 0..8u32 {
            let p = q + 0x20 + 16 * k;
            for (i, (sx, sy)) in [(false, true), (true, true), (false, false), (true, false)].into_iter().enumerate() {
                let c = fx.mem.f(q + 0xC);
                let vx = if sx { ee::add(fx.mem.f(p), c) } else { ee::sub(fx.mem.f(p), c) };
                fx.mem.setf(sp + 0xB0, vx);
                let c = fx.mem.f(q + 0xC);
                let vy = if sy { ee::add(fx.mem.f(p + 4), c) } else { ee::sub(fx.mem.f(p + 4), c) };
                fx.mem.setf(sp + 0xB4, vy);
                if i == 0 {
                    let z = fx.mem.read_u32(p + 8);
                    fx.mem.write_u32(sp + 0xB8, z);
                }
                vertex(fx, q, x, sp + 0xB0, xyz[i], ws[i], i as u32);
            }
            if texture_submit(fx, sp, ot, g, q) {
                fx.clear_projected(q + 4);
            }
        }
    } else {
        for s in 0..4u32 {
            let p = q + 0x20 * s;
            let mode = fx.mem.i32(q + 8);
            if mode == 1 || mode == 5 {
                offset_vertex(fx, sp, q, x, p + 0x30, xyz[0], ws[0], 0);
                let o = if s < 2 { q + 0x40 } else { q + 0x80 };
                offset_vertex(fx, sp, q, x, o, xyz[1], ws[1], 1);
                let o = if s.wrapping_sub(1) < 2 { q + 0x60 } else { q + 0x20 };
                offset_vertex(fx, sp, q, x, o, xyz[2], ws[2], 2);
                vertex(fx, q, x, q + 0x10, xyz[3], ws[3], 3);
            } else if mode == 2 || mode == 6 {
                offset_vertex(fx, sp, q, x, p + 0x30, xyz[0], ws[0], 0);
                if s == 3 {
                    for c in 0..3u32 {
                        let v = ee::add(fx.mem.f(q + 0x10 + 4 * c), fx.mem.f(q + 0x20 + 4 * c));
                        fx.mem.setf(sp + 0xB0 + 4 * c, v);
                    }
                    vertex(fx, q, x, q + 0x20, xyz[1], ws[1], 1);
                } else {
                    offset_vertex(fx, sp, q, x, p + 0x40, xyz[1], ws[1], 1);
                }
                for c in 0..3u32 {
                    let v = ee::add(fx.mem.f(q + 0x10 + 4 * c), fx.mem.f(p + 0x20 + 4 * c));
                    fx.mem.setf(sp + 0xB0 + 4 * c, v);
                }
                vertex(fx, q, x, p + 0x20, xyz[2], ws[2], 2);
                vertex(fx, q, x, q + 0x10, xyz[3], ws[3], 3);
            }
            texture_submit(fx, sp, ot, g, q);
            fx.clear_projected(q + 4);
        }
    }
    fx.leave(0x150);
    0
}

/// 9006fa9b5875 [23 B records; b028 0x1E01040, 376 words] [F]:
/// `squares(ot, g, x, v) -> 0`: each of 60 points (12 bytes from +0x20)
/// moved by `v` (added with flag 0x1F, else multiplied, 0x103620 /
/// 0x1036A0) and drawn as a square of half size +0xC in the xy plane;
/// descriptor at sp + 0xC0 (blend 0x48, colour +8, the cell +0x10 as UVs
/// (x, y), (x + w - 1, y), (x, y + h - 1), (x + w - 1, y + h - 1) with
/// flag 0, else ST floats; texture +0, filter 0); far Z (flag 0x13) or
/// the mean-w key; vertex bits cleared after each. `g`: +0 texture, +4
/// flags, +8 colour, +0xC half size, +0x10 cell. A 0x150 frame.
pub fn squares(fx: &mut Fx, ot: u32, g: u32, x: u32, v: u32) -> i32 {
    let sp = fx.enter(0x150);
    let d = sp + 0xC0;
    fx.desc_init(d, 12, 0x34, REGS_TEX4);
    fx.desc_alpha(d, ALPHA_48);
    fx.colours4(d, g + 8);
    if bit(fx.mem.read_u32(g + 4), 0) {
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        let (rx, ry, rw, rh) = (fx.mem.i16(g + 0x10), fx.mem.i16(g + 0x12), fx.mem.i16(g + 0x14), fx.mem.i16(g + 0x16));
        for (k, (cu, cv)) in [(false, false), (true, false), (false, true), (true, true)].into_iter().enumerate() {
            let a = d + 0x18 + 0x18 * k as u32;
            fx.set_uv(a, if cu { rx + rw - 1 } else { rx });
            fx.set_uv(a + 2, if cv { ry + rh - 1 } else { ry });
        }
    } else {
        let (s, t) = (fx.mem.read_u32(g + 0x10), fx.mem.read_u32(g + 0x14));
        fx.mem.write_u32(sp + 0xD8, s);
        fx.mem.write_u32(sp + 0xDC, t);
        let s2 = ee::add(fx.mem.f(g + 0x10), fx.mem.f(g + 0x18));
        fx.mem.setf(sp + 0xF0, s2);
        fx.mem.write_u32(sp + 0xF4, t);
        fx.mem.write_u32(sp + 0x108, s);
        let t2 = ee::add(fx.mem.f(g + 0x14), fx.mem.f(g + 0x1C));
        fx.mem.setf(sp + 0x10C, t2);
        let v = ee::add(fx.mem.f(g + 0x10), fx.mem.f(g + 0x18));
        fx.mem.setf(sp + 0x120, v);
        let v = ee::add(fx.mem.f(g + 0x14), fx.mem.f(g + 0x1C));
        fx.mem.setf(sp + 0x124, v);
    }
    let xyz = [sp + 0xE0, sp + 0xF8, sp + 0x110, sp + 0x128];
    let ws = [sp + 0xA0, sp + 0xA4, sp + 0xA8, sp + 0xAC];
    for k in 0..60u32 {
        let p = g + 0x20 + 12 * k;
        if bit(fx.mem.read_u32(g + 4), 0x1F) {
            fx.add3(p, v);
        } else {
            fx.mul3(p, v);
        }
        for (i, (sx, sy)) in [(false, true), (true, true), (false, false), (true, false)].into_iter().enumerate() {
            let c = fx.mem.f(g + 0xC);
            let vx = if sx { ee::add(fx.mem.f(p), c) } else { ee::sub(fx.mem.f(p), c) };
            fx.mem.setf(sp + 0xB0, vx);
            let c = fx.mem.f(g + 0xC);
            let vy = if sy { ee::add(fx.mem.f(p + 4), c) } else { ee::sub(fx.mem.f(p + 4), c) };
            fx.mem.setf(sp + 0xB4, vy);
            let z = fx.mem.read_u32(p + 8);
            fx.mem.write_u32(sp + 0xB8, z);
            vertex(fx, g, x, sp + 0xB0, xyz[i], ws[i], i as u32);
        }
        let rh2 = fx.mem.read_u32(g);
        fx.desc_texture(d, rh2, 0);
        if fx.all_projected(g + 4) {
            if bit(fx.mem.read_u32(g + 4), 0x13) {
                fx.far_z(&xyz);
                fx.submit(d, ot, 0);
            } else {
                let key = fx.mean_key4(ws);
                fx.submit(d, ot, key);
            }
        }
        fx.clear_projected(g + 4);
    }
    fx.leave(0x150);
    0
}
