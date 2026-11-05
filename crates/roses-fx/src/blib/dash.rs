//! Dashes of the effect library: a textured
//! screen-space strip from a record's origin along its velocity, as wide
//! as its width at each end's depth.
use super::exe::{camera_vp_into, countdown, mul_mem, node_source, rot_trans_into, tex_rect};
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx};

/// 0de7907153d1 [2 B records; b162 0x1E027A0, 360 words] [F]:
/// `dashes(at, recs, ot, n) -> 1` steps and draws `n` records (0x50 bytes;
/// a 0x210 frame, the descriptor at sp + 0x198: one colour, four UV /
/// XYZ2 pairs). The function's sprite spec (record-relative `lui` / `addiu`
/// at +0x3C / +0x40) is copied to sp + 0x170, its texture asked once (sp +
/// 0xB0). Per record: countdown 0x101AF0; the position +0x10 from a node
/// with +5 (0x101BA0); through the camera x (angles +0x30, position +0x10)
/// (0x20E2B0, 0x20C0B0, 0x20B370); colour +0xE, +0xD, +0xC, alpha +0xF (x
/// count / 8 over the last 8); UVs from the spec (0x101D20); the origin
/// and velocity +0x20 x elapsed projected (0x204D80) and on screen
/// (0x101600) or the record is skipped; half widths +8 / (w / 256) at each
/// end; the strip across y (mode +4 & 3 = 2, or with mode 0 / 3 when the
/// line is wider than tall) else across x; key 0.
pub fn dashes(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let spec = rel(fx, at, 0x3C, 0x40);
    let sp = fx.enter(0x210);
    let v = fx.mem.read_u64(spec & !15);
    fx.mem.write_u64(sp + 0x170, v);
    let v = fx.mem.read_u64((spec & !15) + 8);
    fx.mem.write_u64(sp + 0x178, v);
    let v = fx.mem.read_u64((spec + 0x10) & !7);
    fx.mem.write_u64(sp + 0x180, v);
    let (kind, id) = (fx.mem.i32(sp + 0x170), fx.mem.u8(sp + 0x174));
    let tex = fx.texture(kind, id);
    fx.mem.write_u32(sp + 0xB0, tex);
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x50;
        }
        one(fx, at, sp, g, ot);
        i += 1;
    }
    fx.leave(0x210);
    1
}

/// 0x1A2C70 of `v` (its residue left) into the u16 at `a`.
fn fix16(fx: &mut Fx, a: u32, v: f32, ra: u32) {
    fx.fix_residue(v, ra);
    fx.mem.write_u16(a, ee::fixunssfsi(v) as u16);
}

fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    node_source(fx, g, g + 0x10);
    camera_vp_into(fx, sp + 0x100);
    rot_trans_into(fx, sp + 0xC0, g + 0x30, g + 0x10);
    mul_mem(fx, sp + 0x100, sp + 0xC0);
    let d = sp + 0x198;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (src, dst) in [(0xEu32, 0x1A8u32), (0xD, 0x1A9), (0xC, 0x1AA)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + dst, v);
    }
    fx.mem.write_u32(sp + 0x1AC, 0x3F80_0000);
    let tex = fx.mem.read_u32(sp + 0xB0);
    tex_rect(fx, d, sp + 0x170, tex);
    let c = fx.mem.u8(g);
    let a = if c < 8 {
        let k = fx.mem.u8(g + 0xF) >> 3;
        fx.mult(k, c)
    } else {
        fx.mem.u8(g + 0xF)
    };
    fx.mem.set8(sp + 0x1AB, a);
    let e = ee::from_i32(fx.mem.u8(g + 2) - fx.mem.u8(g));
    for o in [0x148u32, 0x144, 0x140] {
        fx.mem.write_u32(sp + o, 0);
    }
    for k in 0..3u32 {
        let v = ee::mul(fx.mem.f(g + 0x20 + 4 * k), e);
        fx.mem.setf(sp + 0x150 + 4 * k, v);
    }
    for (v, xyz, w) in [(0x140u32, 0x160u32, 0x190u32), (0x150, 0x168, 0x194)] {
        let m = fx.read_m4(sp + 0x100);
        if !fx.project_at(sp + v, &m, sp + xyz, sp + w) || !Fx::on_screen(fx.mem.read_u64(sp + xyz)) {
            return;
        }
    }
    let width = fx.mem.f(g + 8);
    let h0 = ee::div(width, ee::div(fx.mem.f(sp + 0x190), 256.0));
    let h1 = ee::div(width, ee::div(fx.mem.f(sp + 0x194), 256.0));
    for (dst, src) in [(0x1D8u32, 0x168u32), (0x1B8, 0x1D8), (0x1E8, 0x160), (0x1C8, 0x1E8)] {
        let z = fx.xyz_z(sp + src);
        fx.set_xyz_z(sp + dst, z);
    }
    let (x0, y0, x1, y1) = (fx.mem.u16(sp + 0x160), fx.mem.u16(sp + 0x162), fx.mem.u16(sp + 0x168), fx.mem.u16(sp + 0x16A));
    let across_y = match fx.mem.u8(g + 4) & 3 {
        2 => true,
        1 => false,
        _ => (y0 - y1).abs() < (x0 - x1).abs(),
    };
    if across_y {
        fix16(fx, sp + 0x1BA, ee::sub(ee::from_i32(y1), h1), at + 0x3E8);
        fix16(fx, sp + 0x1CA, ee::sub(ee::from_i32(y0), h0), at + 0x434);
        fx.mem.set16(sp + 0x1DA, y1);
        fx.mem.set16(sp + 0x1EA, y0);
        for (o, v) in [(0x1D8u32, x1), (0x1B8, x1), (0x1E8, x0), (0x1C8, x0)] {
            fx.mem.set16(sp + o, v);
        }
    } else {
        fix16(fx, sp + 0x1B8, ee::sub(ee::from_i32(x1), h1), at + 0x4C0);
        fix16(fx, sp + 0x1C8, ee::sub(ee::from_i32(x0), h0), at + 0x50C);
        fx.mem.set16(sp + 0x1D8, x1);
        fx.mem.set16(sp + 0x1E8, x0);
        for (o, v) in [(0x1DAu32, y1), (0x1BA, y1), (0x1EA, y0), (0x1CA, y0)] {
            fx.mem.set16(sp + o, v);
        }
    }
    fx.submit(d, ot, 0);
}

/// fd85ab90a812 [3 B records; b379 (card 2) 0x1E02D20, 540 words] (helper
/// B18) [F]: `ribbons(at, recs, ot, n) -> 1` steps and draws `n` records
/// (0x20 bytes; a 0x1E0 frame, the descriptor at sp + 0x148: RGBAQ / UV /
/// XYZF2 x 4). Per record: countdown 0x101AF0; the camera (0x20E2B0 into
/// sp + 0xD0); the texture (5, +9); colour +0x1E / +0x1D / +0x1C, Q 1;
/// atlas cell +0xC of layout +0xB with blend +0xA (0x1020B0 into sp +
/// 0x130), its u0 / u1 pulled to the middle ((u1 - u0) >> 1 in, 1 out),
/// then the strip texture (0x101EE0). Segments k < +8 (with flag 2 no more
/// than the counts gone) between the vec3 points k / k + 1 of the list at
/// +0x10: both projected (0x204D80, w sp + 0x1D8 / 0x1DC) and on screen
/// (0x101600) or the segment is skipped; half widths 0x1018E0(+0x14, w)
/// at each end; alpha +0x1F (x count / 8 over the last 8 unless flag
/// 0x40; x +0x18 on odd counts with flag 1; flag 4 ramped down along the
/// segments, alpha / n x (n - k) at the near end, x (n - k - 1) at the far
/// end; flag 8 the last segment's far end 0); Z from each end (flag 0x80
/// 0xFFFFFE, key 0), keyed by the nearer w; two quads per segment, one
/// widened in y, one in x (0x1A2C70 for each coordinate).
pub fn ribbons(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1E0);
    fx.mem.set32(sp + 0xC4, n);
    fx.mem.write_u32(sp + 0xC8, ot);
    let mut p = recs;
    fx.mem.set32(sp + 0xB0, 0);
    while fx.mem.i32(sp + 0xB0) < fx.mem.i32(sp + 0xC4) {
        if fx.mem.i32(sp + 0xB0) != 0 {
            p += 0x20;
        }
        ribbon(fx, at, sp, p);
        let v = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, v);
    }
    fx.leave(0x1E0);
    1
}

/// 0x1A2C70 (the unsigned conversion) of `v` returning to `at + ra`.
fn fix(fx: &mut Fx, at: u32, ra: u32, v: f32) -> u32 {
    fx.fix_residue(v, at + ra);
    ee::fixunssfsi(v)
}

fn ribbon(fx: &mut Fx, at: u32, sp: u32, p: u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    camera_vp_into(fx, sp + 0xD0);
    let id = fx.mem.u8(p + 9);
    let tex = fx.texture(5, id);
    let d = sp + 0x148;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    for (src, k) in [(0x1Eu32, 0u32), (0x1D, 1), (0x1C, 2)] {
        let v = fx.mem.read_u8(p + src);
        for o in [0x1A0u32, 0x188, 0x170, 0x158] {
            fx.mem.write_u8(sp + o + k, v);
        }
    }
    for o in [0x1A4u32, 0x18C, 0x174, 0x15C] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let b = fx.mem.read_u8(p + 0xA);
    fx.mem.write_u8(sp + 0x135, b);
    let (cell, layout) = (fx.mem.u8(p + 0xC), fx.mem.u8(p + 0xB));
    super::exe::atlas_cell(fx, sp + 0x130, cell, layout);
    let (u0, u1) = (fx.mem.i32(sp + 0x138), fx.mem.i32(sp + 0x13C));
    let t0 = u1.wrapping_sub(u0) >> 1;
    fx.mem.set32(sp + 0x138, u0.wrapping_add(t0).wrapping_sub(1));
    let u1 = fx.mem.i32(sp + 0x13C);
    fx.mem.set32(sp + 0x13C, u1.wrapping_sub(t0).wrapping_add(1));
    super::exe::tex_rect_strip(fx, d, sp + 0x130, tex);
    let mut segs = fx.mem.u8(p + 8);
    if fx.mem.u8(p + 4) & 2 != 0 {
        let gone = fx.mem.u8(p + 2) - fx.mem.u8(p);
        if gone < segs {
            segs = gone;
        }
    }
    let (v0, v1, v2, v3) = (sp + 0x168, sp + 0x180, sp + 0x198, sp + 0x1B0);
    let mut s1 = 0;
    while s1 < segs {
        let pts = fx.mem.read_u32(p + 0x10).wrapping_add(0xC * s1 as u32);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(pts + 4 * c);
            fx.mem.write_u32(sp + 0x110 + 4 * c, w);
        }
        let m = fx.read_m4(sp + 0xD0);
        if !fx.project_at(sp + 0x110, &m, sp + 0x120, sp + 0x1D8) || !Fx::on_screen(fx.mem.read_u64(sp + 0x120)) {
            s1 += 1;
            continue;
        }
        let (size, w) = (fx.mem.f(p + 0x14), fx.mem.f(sp + 0x1D8));
        let f20 = fx.view_scale(size, w);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(pts + 0xC + 4 * c);
            fx.mem.write_u32(sp + 0x110 + 4 * c, w);
        }
        let m = fx.read_m4(sp + 0xD0);
        if !fx.project_at(sp + 0x110, &m, sp + 0x128, sp + 0x1DC) || !Fx::on_screen(fx.mem.read_u64(sp + 0x128)) {
            s1 += 1;
            continue;
        }
        let (size, w) = (fx.mem.f(p + 0x14), fx.mem.f(sp + 0x1DC));
        let f21 = fx.view_scale(size, w);
        let count = fx.mem.u8(p);
        let mut f12 = if count < 8 && fx.mem.u8(p + 4) & 0x40 == 0 {
            let k = fx.mem.u8(p + 0x1F) >> 3;
            ee::from_i32(fx.mult(k, count))
        } else {
            ee::from_i32(fx.mem.u8(p + 0x1F))
        };
        let flags = fx.mem.u8(p + 4);
        if flags & 1 != 0 && count & 1 != 0 {
            f12 = ee::mul(f12, fx.mem.f(p + 0x18));
        }
        if flags & 4 != 0 {
            let f22 = ee::div(f12, ee::from_i32(segs));
            let s4 = segs - s1;
            let a = fix(fx, at, 0x360, ee::mul(f22, ee::from_i32(s4)));
            fx.mem.write_u8(sp + 0x18B, a as u8);
            fx.mem.write_u8(sp + 0x15B, a as u8);
            let a = fix(fx, at, 0x380, ee::mul(f22, ee::from_i32(s4 - 1)));
            fx.mem.write_u8(sp + 0x1A3, a as u8);
            fx.mem.write_u8(sp + 0x173, a as u8);
        } else {
            let a = fix(fx, at, 0x394, f12);
            for o in [0x1A3u32, 0x18B, 0x173, 0x15B] {
                fx.mem.write_u8(sp + o, a as u8);
            }
        }
        if s1 == segs - 1 && fx.mem.u8(p + 4) & 8 != 0 {
            fx.mem.write_u8(sp + 0x1A3, 0);
            fx.mem.write_u8(sp + 0x173, 0);
        }
        let f22 = if fx.mem.u8(p + 4) & 0x80 != 0 {
            let mut z = 0xFF_FFFE;
            for o in [v3, v2, v1, v0] {
                fx.set_xyz_z(o, z);
                z = fx.xyz_z(o);
            }
            0.0
        } else {
            let z = fx.xyz_z(sp + 0x120);
            fx.set_xyz_z(v2, z);
            let z = fx.xyz_z(v2);
            fx.set_xyz_z(v0, z);
            let z = fx.xyz_z(sp + 0x128);
            fx.set_xyz_z(v3, z);
            let z = fx.xyz_z(v3);
            fx.set_xyz_z(v1, z);
            let (wa, wb) = (fx.mem.f(sp + 0x1D8), fx.mem.f(sp + 0x1DC));
            if ee_lt(wa, wb) {
                wa
            } else {
                wb
            }
        };
        let (ax, ay) = (ee::from_i32(fx.mem.u16(sp + 0x120)), ee::from_i32(fx.mem.u16(sp + 0x122)));
        let (bx, by) = (ee::from_i32(fx.mem.u16(sp + 0x128)), ee::from_i32(fx.mem.u16(sp + 0x12A)));
        // Widened in y.
        for (o, ra, v) in [(v0 + 2, 0x5C0u32, ee::sub(ay, f20)), (v1 + 2, 0x608, ee::sub(by, f21)), (v2 + 2, 0x644, ee::add(ay, f20)), (v3 + 2, 0x680, ee::add(by, f21))] {
            let h = fix(fx, at, ra, v);
            fx.mem.write_u16(o, h as u16);
        }
        let x = fx.mem.read_u16(sp + 0x120);
        fx.mem.write_u16(v2, x);
        fx.mem.write_u16(v0, x);
        let x = fx.mem.read_u16(sp + 0x128);
        fx.mem.write_u16(v3, x);
        fx.mem.write_u16(v1, x);
        let key = fix(fx, at, 0x6B0, f22) as i32;
        let ot = fx.mem.read_u32(sp + 0xC8);
        fx.submit(d, ot, key);
        // Widened in x.
        for (o, ra, v) in [(v0, 0x6FCu32, ee::sub(ax, f20)), (v1, 0x740, ee::sub(bx, f21)), (v2, 0x778, ee::add(ax, f20)), (v3, 0x7B4, ee::add(bx, f21))] {
            let h = fix(fx, at, ra, v);
            fx.mem.write_u16(o, h as u16);
        }
        let y = fx.mem.read_u16(sp + 0x122);
        fx.mem.write_u16(v2 + 2, y);
        fx.mem.write_u16(v0 + 2, y);
        let y = fx.mem.read_u16(sp + 0x12A);
        fx.mem.write_u16(v3 + 2, y);
        fx.mem.write_u16(v1 + 2, y);
        let key = fix(fx, at, 0x7F0, f22) as i32;
        let ot = fx.mem.read_u32(sp + 0xC8);
        fx.submit(d, ot, key);
        s1 += 1;
    }
}

/// 182310a79b24 [4 B records; b706 (card 75) 0x1E01670, 756 words]
/// [F]: `strokes(at, recs, ot, n) -> 1` steps and draws `n`
/// records (0x20 bytes; a 0x2B0 frame, f20..f22 saved; the descriptor at
/// sp + 0x210: four RGBAQ / UV / XYZ2 vertices 0x18 apart, prim 0x34). The
/// function's 0x18-byte sprite spec (record-relative `lui` / `addiu` at
/// +0x30 / +0x38) is copied to sp + 0x1F0 every call. Per record:
/// countdown 0x101AF0; the camera (0x20E2B0, sp + 0x160); colour +0x1A /
/// +0x19 / +0x18, Q 1. With +4 1 the texture (4, 0xF), flag 0x800, the
/// spec's UVs (u0 v1, u0 v0, u1 v1, u1 v0), REGION_CLAMP u0..u1 x v0..v1-1
/// (0x208400), TEX0 / TEX1 (0x207E50, filter 1), ALPHA by the spec's mode
/// (0x101C80 into sp + 0x2AC); else the texture (5, +9), the atlas cell
/// +0xA of layout +0xC (blend +0xB) by 0x1020B0 / 0x101EE0. Segments from
/// +8 - count (0 with +4 2 or a count past +8) to +8, or to +0xE x
/// (length - count) when +0xE > 0 and that is below +8 / +0xE, of the
/// point list +0x1C (12 bytes): both ends projected (0x204D80) and on
/// screen (0x101600); alpha +0x1B (+0xF 1: the first segment's start /
/// the last's end 0); the screen length sqrtf(dx^2 + dy^2) / +0xD the half
/// width w; then 2 (+4 8) or (count & 3) + 1 strokes, each w += k x w x
/// +0x14: a quad across x when dx <= dy else across y (the ends' x / y +-
/// w), Z the ends' (or 0xFFFFFE with +4 0x80, key 0), turned by (+4 4:
/// segment, else count + segment) & 3 (sp + 0x1B0 the swap), key
/// 0x1A2C70 of the nearer w.
pub fn strokes(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    strokes_a3(fx, at, recs, ot, n).0
}

/// [`strokes`] and the a3 it leaves [F card 539 0x1E02380..
/// 0x1E02EB0; run card 539]: per counting record the texture's (+4 1: the
/// descriptor flags before 0x207E20 sets 0x200; else 0x101EE0's), then
/// per segment the projections' w pointers (sp + 0x2A0 / 0x2A4) as far as
/// they get, then per stroke the low word of the fourth vertex's XYZ (`ld
/// a3`) before its new z. `None` when no record counted.
pub fn strokes_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    strokes_with(fx, at, recs, ot, n, &STROKES)
}

/// How a compile of [`strokes`] differs: its frame (the
/// descriptor and the other locals at the same caller-relative addresses:
/// `sp + frame - 0x2B0` stands for the 0x2B0 frame's sp), the spec's
/// `lui` / `addiu` offsets, the return offsets of the conversions (the
/// eight half widths, then the key's) and whether the scalars live in
/// registers (`compact`: only the count / n / start / end / point-list
/// slots of its own frame, the a3 of its own z chain and swaps).
pub struct StrokeLayout {
    pub frame: u32,
    pub spec: (u32, u32),
    pub fix: [u32; 9],
    pub compact: bool,
}

/// 182310a79b24's layout (b706 0x1E01670).
pub const STROKES: StrokeLayout = StrokeLayout { frame: 0x2B0, spec: (0x30, 0x38), fix: [0x688, 0x6CC, 0x704, 0x73C, 0x79C, 0x7E0, 0x818, 0x850, 0xB28], compact: false };

/// baf111c1d7aa [card 540 (b458) 0x1E02BE0, 788 words]: the
/// unscheduled compile of [`strokes`] in a 0x250 frame (every live local
/// 0x60 lower: the same caller-relative addresses), the spec at +0x44 /
/// +0x48, ot / steps / the segment in registers.
pub const STROKES_B: StrokeLayout = StrokeLayout { frame: 0x250, spec: (0x44, 0x48), fix: [0x670, 0x6BC, 0x704, 0x74C, 0x7B8, 0x804, 0x84C, 0x894, 0xBA0], compact: true };

/// The baf111c1d7aa compilation of [`strokes`] ([`STROKES_B`]).
pub fn strokes_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    strokes_with(fx, at, recs, ot, n, &STROKES_B).0
}

/// [`strokes_b`] and the a3 it leaves [F card 540 0x1E02D14..0x1E03778]:
/// as [`strokes_a3`] except per stroke the last `ld a3` of its z chain
/// (flag 0x80: vertex 2's low XYZ word, else vertex 4's) and, when the
/// corners turn, the last swapped pointer (vertex 3's for turn 1, vertex
/// 4's for 2 / 3).
pub fn strokes_b_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    strokes_with(fx, at, recs, ot, n, &STROKES_B)
}

/// [`strokes`] compiled with layout `l` (and its a3).
pub fn strokes_with(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, l: &StrokeLayout) -> (i32, Option<u32>) {
    let mut a3 = None;
    let spec = rel(fx, at, l.spec.0, l.spec.1);
    let real = fx.enter(l.frame);
    let sp = real.wrapping_add(l.frame).wrapping_sub(0x2B0);
    let v = fx.mem.read_u64(spec & !15);
    let w = fx.mem.read_u64((spec & !15) + 8);
    let x = fx.mem.read_u64((spec + 0x10) & !7);
    fx.mem.write_u64(sp + 0x1F0, v);
    fx.mem.write_u64(sp + 0x1F8, w);
    fx.mem.write_u64(sp + 0x200, x);
    let (count_slot, n_slot) = if l.compact { (real + 0xC0, real + 0xFC) } else { (sp + 0xB0, sp + 0x154) };
    fx.mem.set32(n_slot, n);
    if !l.compact {
        fx.mem.write_u32(sp + 0x158, ot);
    }
    fx.mem.set32(count_slot, 0);
    let mut p = recs;
    if n > 0 {
        if !l.compact {
            fx.mem.write_u32(sp + 0x110, sp + 0x22A);
            fx.mem.write_u32(sp + 0x120, sp + 0x242);
            fx.mem.write_u32(sp + 0x130, sp + 0x25A);
            fx.mem.write_u32(sp + 0x140, sp + 0x272);
        }
        while fx.mem.i32(count_slot) < fx.mem.i32(n_slot) {
            if fx.mem.i32(count_slot) != 0 {
                p += 0x20;
            }
            stroke(fx, at, (sp, real), p, ot, l, &mut a3);
            let v = fx.mem.i32(count_slot) + 1;
            fx.mem.set32(count_slot, v);
        }
    }
    fx.leave(l.frame);
    (1, a3)
}

fn stroke(fx: &mut Fx, at: u32, (sp, real): (u32, u32), p: u32, ot: u32, l: &StrokeLayout, a3: &mut Option<u32>) {
    if countdown(fx, p) == 0 {
        return;
    }
    camera_vp_into(fx, sp + 0x160);
    let d = sp + 0x210;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    for (src, k) in [(0x1Au32, 0u32), (0x19, 1), (0x18, 2)] {
        let v = fx.mem.read_u8(p + src);
        for o in [0x268u32, 0x250, 0x238, 0x220] {
            fx.mem.write_u8(sp + o + k, v);
        }
    }
    for o in [0x26Cu32, 0x254, 0x23C, 0x224] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    if fx.mem.u8(p + 4) & 1 != 0 {
        let tex = fx.texture(4, 0xF);
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        for (o, s) in [(0x228u32, 0x1F8u32), (0x22A, 0x204), (0x240, 0x1F8), (0x242, 0x200), (0x258, 0x1FC), (0x25A, 0x204), (0x270, 0x1FC), (0x272, 0x200)] {
            let v = fx.mem.i32(sp + s);
            fx.set_uv(sp + o, v);
        }
        let (u0, v1, u1, v0) = (fx.mem.i32(sp + 0x1F8), fx.mem.i32(sp + 0x204), fx.mem.i32(sp + 0x1FC), fx.mem.i32(sp + 0x200));
        let clamp = ((i64::from(u0) << 4) as u64 | 0xA) | ((i64::from(u1) << 14) as u64) | ((i64::from(v0) << 24) as u64) | ((i64::from(v1.wrapping_sub(1)) << 34) as u64);
        fx.desc_clamp(d, clamp);
        fx.desc_texture(d, tex, 1);
        super::exe::alpha_mode(fx, sp + 0x1F0, sp + 0x2AC);
        let b = |fx: &mut Fx, k: u32| u64::from(fx.mem.read_u8(sp + 0x2AC + k));
        let alpha = b(fx, 0) | (b(fx, 1) << 2) | (b(fx, 2) << 4) | (b(fx, 3) << 6);
        *a3 = Some(u32::from(fx.mem.read_u16(d + 2)));
        fx.desc_alpha(d, alpha);
    } else {
        let id = fx.mem.u8(p + 9);
        let tex = fx.texture(5, id);
        let b = fx.mem.read_u8(p + 0xB);
        fx.mem.write_u8(sp + 0x1D5, b);
        let (cell, layout) = (fx.mem.u8(p + 0xA), fx.mem.u8(p + 0xC));
        super::exe::atlas_cell(fx, sp + 0x1D0, cell, layout);
        *a3 = Some(super::exe::tex_rect_strip_a3(fx, d, sp + 0x1D0, tex));
    }
    let (start_slot, end_slot) = if l.compact { (real + 0xD0, real + 0xE0) } else { (sp + 0xC0, sp + 0xD0) };
    let count = fx.mem.u8(p);
    let start = if fx.mem.u8(p + 4) & 2 == 0 && count < fx.mem.u8(p + 8) { fx.mem.u8(p + 8) - count } else { 0 };
    fx.mem.set32(start_slot, start);
    let k = fx.mem.u8(p + 0xE);
    let mut end = fx.mem.u8(p + 8);
    if k > 0 {
        let len = fx.mem.u8(p + 8);
        let q = fx.div(len, k).0;
        let left = fx.mem.u8(p + 2) - fx.mem.u8(p);
        if left < q {
            end = fx.mult(k, left);
        }
    }
    fx.mem.set32(end_slot, end);
    let base = fx.mem.read_u32(p + 0x1C);
    if l.compact {
        fx.mem.write_u32(real + 0xB0, base);
    }
    let mut seg = start;
    if seg >= end {
        return;
    }
    let mut pts = base.wrapping_add((12 * seg) as u32);
    if !l.compact {
        fx.mem.write_u32(sp + 0x100, pts);
    }
    let v = [sp + 0x230, sp + 0x248, sp + 0x260, sp + 0x278];
    loop {
        one_segment(fx, at, sp, p, (seg, start, end), (pts, ot), v, l, a3);
        seg += 1;
        pts = pts.wrapping_add(0xC);
        if !l.compact {
            fx.mem.write_u32(sp + 0x100, pts);
        }
        if seg >= fx.mem.i32(end_slot) {
            break;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn one_segment(fx: &mut Fx, at: u32, sp: u32, p: u32, (seg, start, end): (i32, i32, i32), (pts, ot): (u32, u32), v: [u32; 4], l: &StrokeLayout, a3: &mut Option<u32>) {
    let [v0, v1, v2, v3] = v;
    let a = fx.mem.read_u8(p + 0x1B);
    for o in [0x26Bu32, 0x253, 0x23B, 0x223] {
        fx.mem.write_u8(sp + o, a);
    }
    if fx.mem.u8(p + 0xF) & 1 != 0 {
        if seg == start {
            fx.mem.write_u8(sp + 0x23B, 0);
            fx.mem.write_u8(sp + 0x223, 0);
        }
        if seg == end - 1 {
            fx.mem.write_u8(sp + 0x26B, 0);
            fx.mem.write_u8(sp + 0x253, 0);
        }
    }
    for c in 0..3u32 {
        let w = fx.mem.read_u32(pts + 4 * c);
        fx.mem.write_u32(sp + 0x1C0 + 4 * c, w);
    }
    let m = fx.read_m4(sp + 0x160);
    *a3 = Some(sp + 0x2A0);
    if !fx.project_at(sp + 0x1C0, &m, sp + 0x1A0, sp + 0x2A0) || !Fx::on_screen(fx.mem.read_u64(sp + 0x1A0)) {
        return;
    }
    for c in 0..3u32 {
        let w = fx.mem.read_u32(pts + 0xC + 4 * c);
        fx.mem.write_u32(sp + 0x1C0 + 4 * c, w);
    }
    let m = fx.read_m4(sp + 0x160);
    *a3 = Some(sp + 0x2A4);
    if !fx.project_at(sp + 0x1C0, &m, sp + 0x1A8, sp + 0x2A4) || !Fx::on_screen(fx.mem.read_u64(sp + 0x1A8)) {
        return;
    }
    let dx = (fx.mem.u16(sp + 0x1A0) - fx.mem.u16(sp + 0x1A8)).abs();
    let f20 = ee::from_i32(dx);
    let dy = (fx.mem.u16(sp + 0x1A2) - fx.mem.u16(sp + 0x1AA)).abs();
    let f21 = ee::from_i32(dy);
    // mula.s / madd.s: the products rounded, then the sum.
    let len = fx.sqrtf(ee::add(ee::mul(f20, f20), ee::mul(f21, f21)));
    let mut f22 = ee::div(len, ee::from_i32(fx.mem.u8(p + 0xD)));
    let steps = if fx.mem.u8(p + 4) & 8 != 0 { 2 } else { (fx.mem.u8(p) & 3) + 1 };
    if !l.compact {
        fx.mem.set32(sp + 0xE0, steps);
        fx.mem.set32(sp + 0xF0, seg & 3);
    }
    let fix = |fx: &mut Fx, ra: u32, v: f32| -> u16 {
        fx.fix_residue(v, at + ra);
        ee::fixunssfsi(v) as u16
    };
    for s7 in 0..steps {
        let k = ee::mul(f22, fx.mem.f(p + 0x14));
        f22 = ee::add(f22, ee::mul(ee::from_i32(s7), k));
        if !ee_le(f20, f21) {
            let y0 = ee::from_i32(fx.mem.u16(sp + 0x1A2));
            let h = fix(fx, l.fix[0], ee::sub(y0, f22));
            fx.mem.write_u16(sp + 0x232, h);
            let y1 = ee::from_i32(fx.mem.u16(sp + 0x1AA));
            let h = fix(fx, l.fix[1], ee::sub(y1, f22));
            fx.mem.write_u16(sp + 0x24A, h);
            let h = fix(fx, l.fix[2], ee::add(y0, f22));
            fx.mem.write_u16(sp + 0x262, h);
            let h = fix(fx, l.fix[3], ee::add(y1, f22));
            fx.mem.write_u16(sp + 0x27A, h);
            let x = fx.mem.read_u16(sp + 0x1A0);
            fx.mem.write_u16(v2, x);
            fx.mem.write_u16(v0, x);
            let x = fx.mem.read_u16(sp + 0x1A8);
            fx.mem.write_u16(v3, x);
            fx.mem.write_u16(v1, x);
        } else {
            let x0 = ee::from_i32(fx.mem.u16(sp + 0x1A0));
            let h = fix(fx, l.fix[4], ee::sub(x0, f22));
            fx.mem.write_u16(v0, h);
            let x1 = ee::from_i32(fx.mem.u16(sp + 0x1A8));
            let h = fix(fx, l.fix[5], ee::sub(x1, f22));
            fx.mem.write_u16(v1, h);
            let h = fix(fx, l.fix[6], ee::add(x0, f22));
            fx.mem.write_u16(v2, h);
            let h = fix(fx, l.fix[7], ee::add(x1, f22));
            fx.mem.write_u16(v3, h);
            let y = fx.mem.read_u16(sp + 0x1A2);
            fx.mem.write_u16(sp + 0x262, y);
            fx.mem.write_u16(sp + 0x232, y);
            let y = fx.mem.read_u16(sp + 0x1AA);
            fx.mem.write_u16(sp + 0x27A, y);
            fx.mem.write_u16(sp + 0x24A, y);
        }
        let far = fx.mem.u8(p + 4) & 0x80 != 0;
        *a3 = Some(fx.mem.read_u32(if l.compact && far { v1 } else { v3 }));
        let key_w = if far {
            let mut z = 0xFF_FFFE;
            for o in [v3, v2, v1, v0] {
                fx.set_xyz_z(o, z);
                z = fx.xyz_z(o);
            }
            0.0
        } else {
            let z = fx.xyz_z(sp + 0x1A0);
            fx.set_xyz_z(v1, z);
            let z = fx.xyz_z(v1);
            fx.set_xyz_z(v0, z);
            let z = fx.xyz_z(sp + 0x1A8);
            fx.set_xyz_z(v3, z);
            let z = fx.xyz_z(v3);
            fx.set_xyz_z(v2, z);
            let (wa, wb) = (fx.mem.f(sp + 0x2A0), fx.mem.f(sp + 0x2A4));
            if ee_lt(wa, wb) {
                wa
            } else {
                wb
            }
        };
        for o in [0x24Fu32, 0x237, 0x27F, 0x267] {
            fx.mem.write_u8(sp + o, 0);
        }
        let turn = if fx.mem.u8(p + 4) & 4 != 0 { seg & 3 } else { (fx.mem.u8(p) + seg) & 3 };
        let swaps: &[(u32, u32)] = match turn {
            1 => &[(v0, v3), (v1, v2)],
            2 => &[(v0, v1), (v2, v3)],
            3 => &[(v0, v2), (v1, v3)],
            _ => &[],
        };
        if l.compact && turn != 0 {
            *a3 = Some(if turn == 1 { v2 } else { v3 });
        }
        for &(a, b) in swaps {
            let t = fx.mem.read_u64(a);
            fx.mem.write_u64(sp + 0x1B0, t);
            let u = fx.mem.read_u64(b);
            fx.mem.write_u64(a, u);
            let t = fx.mem.read_u64(sp + 0x1B0);
            fx.mem.write_u64(b, t);
        }
        fx.fix_residue(key_w, at + l.fix[8]);
        let key = ee::fixunssfsi(key_w) as i32;
        let ot = if l.compact { ot } else { fx.mem.read_u32(sp + 0x158) };
        fx.submit(sp + 0x210, ot, key);
    }
}
