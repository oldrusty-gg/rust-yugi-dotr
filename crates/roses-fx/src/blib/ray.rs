//! Rays of the effect library: textured
//! screen-space quads from a centre out to points read from a byte table.
use super::exe::{camera_vp_into, countdown, libm_cosf, mul_mem, node_source, rot_trans_into, tex_rect_strip};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx};

const PI: u32 = 0x4049_0FDB;

/// a6f75bab69e1 [10 B records; b106 0x1E023C0, b649 0x1E01D90, 732 words]
/// [F]: `rays(at, recs, ot, n) -> 1` steps and draws `n` records (0x74
/// bytes; a 0x2A0 frame, the descriptor at sp + 0x210: RGBAQ / ST / XYZ2 x
/// 4, Q 1.0). Two sprite specs (0x18 each) from the record's rodata (the
/// `lui` / `addiu` at +0x4C / +0x50) into sp + 0x1E0 / + 0x1F8, their
/// textures (0x202530) into sp + 0xEC / + 0xF0. Per record: countdown
/// 0x101AF0; the byte table +0xC read from cursor +9 (the first byte + 20
/// the angle step); the camera (0x20E2B0 into sp + 0x140). Flag 2: the end
/// +0x34 must project and lie on screen (else the record is skipped), the
/// turn +0x54 about it (0x20C0B0), colours +0x1E.. (alpha 0) / +0x22..
/// (alpha +0x23, x count / 16 over the last 16) on the two pairs, spec B.
/// Else: position +0x24 += +0x44, angles +0x54 += +0x64 (i32), the
/// position from a node with +5 (0x101BA0), the turn about it, colour
/// +0x1E.. on three vertices and +0x22.. on the last, spec A. The centre
/// +0x24 must project and lie on screen; through the turn (0x20B370). Then
/// +8 rays: flag 1 a table vector ((b0 - b1, b2 - b3, b4 - b5) x +0x10 /
/// 255), else at angle a += step (wrapped past 360) a radius +0x14 b0 /
/// 255 (libm cosf / sinf) and height +0x10 / 2 (1 + b1 / 255); scaled by
/// the elapsed count, projected (skipped off screen); width +0x18 / (w /
/// 256) as a quad along the steeper screen axis (flag 2: from the ray's
/// point to the end, else from the centre); far w the nearer; key that w
/// (0x1A2C70).
pub fn rays(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    rays_with(fx, at, recs, ot, n, &RAYS)
}

/// Where a compilation of [`rays`] keeps its frame, table and `jal
/// 0x1A2C70`s.
pub struct RayLayout {
    /// The frame size (the locals are caller-relative the same in both).
    pub frame: u32,
    /// The spec table's `lui` / `addiu` offsets in the function.
    pub table: (u32, u32),
    /// The `jal 0x1A2C70` offsets: flag 2 flat / steep, else flat (centre,
    /// end) / steep (centre, end), then the key's.
    pub fixes: [u32; 7],
    /// ot / n / the textures / the record counter / the elapsed count kept
    /// in their stack slots (sp + 0xF8 / 0xF4 / 0xEC / 0xF0 / 0xC0 / 0xD0;
    /// false: in registers, the slots never written).
    pub slots: bool,
}

/// a6f75bab69e1's layout (b106 0x1E023C0).
pub const RAYS: RayLayout = RayLayout { frame: 0x2A0, table: (0x4C, 0x50), fixes: [0x778, 0x7CC, 0x97C, 0x9C4, 0xA38, 0xA84, 0xAC4], slots: true };

/// 16fe8691a0ac [1 B record; b131 (card 110) 0x1E024B0, 676 words] (helper
/// B16) [F]: [`rays`] compiled with a 0x290 frame (every local 0x10 lower,
/// so at the same caller-relative address: the descriptor at sp + 0x200,
/// the specs at + 0x1D0 / + 0x1E8, the camera at + 0x130, i / e at + 0xB0
/// / + 0xC0), the table at +0x34 / +0x3C, the same conversions and store
/// order, its own 0x1A2C70 offsets.
pub const RAYS_B: RayLayout = RayLayout { frame: 0x290, table: (0x34, 0x3C), fixes: [0x708, 0x74C, 0x8E0, 0x918, 0x978, 0x9B4, 0x9F0], slots: true };

/// fbc4ffd89c21 [2 B records; card 107's 0x1E025D0, 816 words] (helper
/// A18) [F]: [`rays`] compiled with a 0x250 frame (every local 0x50 lower,
/// the same caller-relative addresses), ot / n / the textures / the
/// counters in registers (no slots), the angle and the table bytes
/// recomputed per use (the same operations in the same order), the table
/// at +0x48 / +0x4C, its own 0x1A2C70 offsets.
pub const RAYS_C: RayLayout = RayLayout { frame: 0x250, table: (0x48, 0x4C), fixes: [0x850, 0x8AC, 0xAC4, 0xB10, 0xB8C, 0xBD8, 0xC24], slots: false };

/// The fbc4ffd89c21 compilation of [`rays`] ([`RAYS_C`]).
pub fn rays_c(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    rays_with(fx, at, recs, ot, n, &RAYS_C)
}

/// The 16fe8691a0ac compilation of [`rays`] ([`RAYS_B`]).
pub fn rays_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    rays_with(fx, at, recs, ot, n, &RAYS_B)
}

/// [`rays`] compiled with layout `l`; `sp` below is the 0x2A0-frame base
/// (the locals' caller-relative addresses).
pub fn rays_with(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, l: &RayLayout) -> i32 {
    let sp = fx.enter(l.frame) + l.frame - 0x2A0;
    if l.slots {
        fx.mem.set32(sp + 0xF8, ot as i32);
        fx.mem.set32(sp + 0xF4, n);
    }
    let src = super::rel(fx, at, l.table.0, l.table.1);
    for k in 0..0xCu32 {
        let w = fx.mem.read_u32(src + 4 * k);
        fx.mem.write_u32(sp + 0x1E0 + 4 * k, w);
    }
    let (kind, id) = (fx.mem.i32(sp + 0x1E0), fx.mem.u8(sp + 0x1E4));
    let ta = fx.texture(kind, id);
    if l.slots {
        fx.mem.write_u32(sp + 0xEC, ta);
    }
    let (kind, id) = (fx.mem.i32(sp + 0x1F8), fx.mem.u8(sp + 0x1FC));
    let tb_ = fx.texture(kind, id);
    if l.slots {
        fx.mem.write_u32(sp + 0xF0, tb_);
    }
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x74;
        }
        if l.slots {
            fx.mem.set32(sp + 0xC0, i);
        }
        one(fx, at, sp, g, ot, l, [ta, tb_]);
        i += 1;
    }
    if l.slots {
        fx.mem.set32(sp + 0xC0, i);
    }
    fx.leave(l.frame);
    1
}

/// u8 at `tbl + (k & 0xFF)` as a float.
fn tb(fx: &mut Fx, tbl: u32, k: u32) -> f32 {
    ee::from_i32(fx.mem.u8(tbl.wrapping_add(k & 0xFF)))
}

/// The alpha +0x23 (x count / 16 over the last 16).
fn alpha(fx: &mut Fx, g: u32) -> i32 {
    let c = fx.mem.u8(g);
    if c < 0x10 {
        let a = fx.mem.u8(g + 0x23) >> 4;
        fx.mult(a, c)
    } else {
        fx.mem.u8(g + 0x23)
    }
}

fn colour(fx: &mut Fx, g: u32, src: u32, dsts: &[u32]) {
    for k in 0..3u32 {
        let v = fx.mem.read_u8(g + src - k);
        for &d in dsts {
            fx.mem.write_u8(d + k, v);
        }
    }
}

fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, l: &RayLayout, tex: [u32; 2]) {
    if countdown(fx, g) == 0 {
        return;
    }
    let mut cur = fx.mem.u8(g + 9) as u32;
    let tbl = fx.mem.read_u32(g + 0xC);
    let step = ee::add(20.0, tb(fx, tbl, cur));
    cur = (cur + 1) & 0xFF;
    let mut a = 0.0f32;
    let e = fx.mem.u8(g + 2) - fx.mem.u8(g);
    if l.slots {
        fx.mem.set32(sp + 0xD0, e);
    }
    let d = sp + 0x210;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    camera_vp_into(fx, sp + 0x140);
    let flag2 = fx.mem.u8(g + 4) & 2 != 0;
    if flag2 {
        let m = fx.read_m4(sp + 0x140);
        if !fx.project_at(g + 0x34, &m, sp + 0x1A8, sp + 0x18C) || !Fx::on_screen(fx.mem.read_u64(sp + 0x1A8)) {
            return;
        }
        rot_trans_into(fx, sp + 0x100, g + 0x54, g + 0x34);
        colour(fx, g, 0x1E, &[sp + 0x238, sp + 0x220]);
        colour(fx, g, 0x22, &[sp + 0x268, sp + 0x250]);
        let v = alpha(fx, g);
        fx.mem.set8(sp + 0x26B, v);
        fx.mem.set8(sp + 0x253, v);
        fx.mem.write_u8(sp + 0x23B, 0);
        fx.mem.write_u8(sp + 0x223, 0);
        let t = if l.slots { fx.mem.read_u32(sp + 0xF0) } else { tex[1] };
        tex_rect_strip(fx, d, sp + 0x1F8, t);
    } else {
        for o in [0x24u32, 0x28, 0x2C] {
            let v = ee::add(fx.mem.f(g + o), fx.mem.f(g + o + 0x20));
            fx.mem.setf(g + o, v);
        }
        for o in [0x54u32, 0x58, 0x5C] {
            let v = fx.mem.i32(g + o).wrapping_add(fx.mem.i32(g + o + 0x10));
            fx.mem.set32(g + o, v);
        }
        node_source(fx, g, g + 0x24);
        rot_trans_into(fx, sp + 0x100, g + 0x54, g + 0x24);
        colour(fx, g, 0x1E, &[sp + 0x250, sp + 0x238, sp + 0x220]);
        colour(fx, g, 0x22, &[sp + 0x268]);
        let v = alpha(fx, g);
        fx.mem.set8(sp + 0x26B, v);
        fx.mem.write_u8(sp + 0x253, 0);
        fx.mem.write_u8(sp + 0x23B, 0);
        fx.mem.write_u8(sp + 0x223, 0);
        let t = if l.slots { fx.mem.read_u32(sp + 0xEC) } else { tex[0] };
        tex_rect_strip(fx, d, sp + 0x1E0, t);
    }
    let m = fx.read_m4(sp + 0x140);
    if !fx.project_at(g + 0x24, &m, sp + 0x1A0, sp + 0x188) || !Fx::on_screen(fx.mem.read_u64(sp + 0x1A0)) {
        return;
    }
    mul_mem(fx, sp + 0x140, sp + 0x100);
    for o in [0x26Cu32, 0x254, 0x23C, 0x224] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let mut k = 0;
    while k < fx.mem.u8(g + 8) {
        let (x, y, z);
        if fx.mem.u8(g + 4) & 1 != 0 {
            let tbl = fx.mem.read_u32(g + 0xC);
            let p = tbl.wrapping_add(cur & 0xFF);
            let s = fx.mem.f(g + 0x10);
            let mut v = [0.0f32; 3];
            for (j, vv) in v.iter_mut().enumerate() {
                let j = 2 * j as u32;
                let dd = fx.mem.u8(p + j) - fx.mem.u8(p + j + 1);
                *vv = ee::div(ee::mul(s, ee::from_i32(dd)), 255.0);
            }
            (x, y, z) = (v[0], v[1], v[2]);
            cur = (cur + 6) & 0xFF;
        } else {
            a = ee::add(a, step);
            if !ee_le(a, 360.0) {
                a = ee::sub(a, 360.0);
            }
            let tbl = fx.mem.read_u32(g + 0xC);
            let r = ee::div(ee::mul(fx.mem.f(g + 0x14), tb(fx, tbl, cur)), 255.0);
            let rad = ee::div(ee::mul(f32::from_bits(PI), a), 180.0);
            let c = libm_cosf(fx, rad);
            x = ee::mul(r, c);
            let s = fx.libm_sinf(rad);
            z = ee::mul(r, s);
            let h = ee::div(fx.mem.f(g + 0x10), 2.0);
            let tbl = fx.mem.read_u32(g + 0xC);
            let b = tb(fx, tbl, cur + 1);
            y = ee::add(h, ee::div(ee::mul(h, b), 255.0));
            cur = (cur + 2) & 0xFF;
        }
        let f = ee::from_i32(if l.slots { fx.mem.i32(sp + 0xD0) } else { e });
        fx.mem.setf(sp + 0x1B0, ee::mul(x, f));
        fx.mem.setf(sp + 0x1B4, ee::mul(y, f));
        fx.mem.setf(sp + 0x1B8, ee::mul(z, f));
        let m = fx.read_m4(sp + 0x140);
        if fx.project_at(sp + 0x1B0, &m, sp + 0x190, sp + 0x180) && Fx::on_screen(fx.mem.read_u64(sp + 0x190)) {
            ray(fx, at, sp, g, ot, flag2, l);
        }
        k += 1;
    }
}

/// 0x1A2C70 of `v` from the `jal` at `at + off` (its frame left), as u16.
fn fix16(fx: &mut Fx, at: u32, off: u32, v: f32) -> i32 {
    fx.fix_residue(v, at + off + 8);
    ee::fixunssfsi(v) as u16 as i32
}

fn ray(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, flag2: bool, l: &RayLayout) {
    let f = &l.fixes;
    let width = fx.mem.f(g + 0x18);
    let mut far = fx.mem.f(sp + 0x180);
    let w = ee::div(width, ee::div(far, 256.0));
    let wc = fx.mem.f(sp + 0x188);
    if !ee_lt(far, wc) {
        far = wc;
    }
    let (x0, y0) = (fx.mem.u16(sp + 0x190), fx.mem.u16(sp + 0x192));
    let (x1, y1) = (fx.mem.u16(sp + 0x1A0), fx.mem.u16(sp + 0x1A2));
    let flat = (y1 - y0).abs() < (x1 - x0).abs();
    if flag2 {
        let z = fx.xyz_z(sp + 0x190);
        fx.set_xyz_z(sp + 0x248, z);
        if flat {
            let v = fix16(fx, at, f[0], ee::add(ee::from_i32(y0), w));
            fx.mem.set16(sp + 0x24A, v);
            fx.mem.set16(sp + 0x248, x0);
        } else {
            let v = fix16(fx, at, f[1], ee::add(ee::from_i32(x0), w));
            fx.mem.set16(sp + 0x248, v);
            fx.mem.set16(sp + 0x24A, y0);
        }
        for (dst, src) in [(0x230u32, 0x190u32), (0x260, 0x1A0), (0x278, 0x1A8)] {
            let v = fx.mem.read_u64(sp + src);
            fx.mem.write_u64(sp + dst, v);
        }
    } else {
        let w1 = ee::div(width, ee::div(wc, 256.0));
        for (dst, src) in [(0x248u32, 0x190u32), (0x230, 0x248), (0x278, 0x1A0), (0x260, 0x278)] {
            let z = fx.xyz_z(sp + src);
            fx.set_xyz_z(sp + dst, z);
        }
        if flat {
            let v = fix16(fx, at, f[2], ee::sub(ee::from_i32(y0), w));
            fx.mem.set16(sp + 0x232, v);
            let v = fix16(fx, at, f[3], ee::sub(ee::from_i32(y1), w1));
            fx.mem.set16(sp + 0x24A, v);
            for (o, v) in [(0x262u32, y0), (0x27A, y1), (0x260, x0), (0x230, x0), (0x278, x1), (0x248, x1)] {
                fx.mem.set16(sp + o, v);
            }
        } else {
            let v = fix16(fx, at, f[4], ee::sub(ee::from_i32(x0), w));
            fx.mem.set16(sp + 0x230, v);
            let v = fix16(fx, at, f[5], ee::sub(ee::from_i32(x1), w1));
            fx.mem.set16(sp + 0x248, v);
            for (o, v) in [(0x260u32, x0), (0x278, x1), (0x262, y0), (0x232, y0), (0x27A, y1), (0x24A, y1)] {
                fx.mem.set16(sp + o, v);
            }
        }
    }
    for o in [0x27Fu32, 0x267, 0x24F, 0x237] {
        fx.mem.write_u8(sp + o, 0);
    }
    fx.fix_residue(far, at + f[6] + 8);
    let key = ee::fixunssfsi(far) as i32;
    fx.submit(sp + 0x210, ot, key);
}

/// 62024681779c [4 B records; b042 0x1E05CC0, 356 words] [F]:
/// `arc_rays(at, recs, n) -> 1` spawns ray bursts along arcs (`n` records
/// of 0x2C; a 0x550 frame). Per record: countdown 0x101AF0; the arc +8
/// (none: skipped) has its two point trails (heads +0xC / +0x1E, +0x31
/// + 1... of its pool +8) gathered into sp + 0xC0 / + 0x2E4 and resampled
/// (0x102550; skipped unless both have the same count and resample). For
/// each of +0x17 samples k: the point at t = arc +0x38 between the trails'
/// samples k (sp + 0x520) and k + 1 (sp + 0x530); a ray record (0x74) from
/// the pool +0xC (first free of +0x14, 0x101C30; none: next sample) with
/// count +0x15 (less k with flag 1), its direction (flag 2: along the
/// trail, else away from trail 1's sample) normalized past length 1
/// (0x20C850 / 0x20C7E0), its length (x +0x18 unless 0) at +0x10, +0x1C
/// into +0x14, 2 into +4, +0x16 rays, width +0x20, a random table cursor
/// (rand with s0 = the record), the table +0x10, colours +0x24.. /
/// +0x28.., both points and the aim angles (0x101940) into +0x54.
pub fn arc_rays(fx: &mut Fx, at: u32, recs: u32, n: i32) -> i32 {
    arc_rays_with(fx, at, recs, n, 0x550, false)
}

/// db6d31a2770e [2 B records; card 107's 0x1E03290, 504 words] (helper
/// A18) [F]: [`arc_rays`] compiled with a 0x500 frame (every local 0x50
/// lower, the same caller-relative addresses), n in s0 and the record
/// counter in s2 (no slots), so rand's saved s0 is n; its rand returns to
/// +0x618.
pub fn arc_rays_c(fx: &mut Fx, at: u32, recs: u32, n: i32) -> i32 {
    arc_rays_with(fx, at, recs, n, 0x500, true)
}

/// `c`: the 0x500-frame compilation ([`arc_rays_c`]); `sp` below is the
/// 0x550-frame base (the locals' caller-relative addresses).
fn arc_rays_with(fx: &mut Fx, at: u32, recs: u32, n: i32, frame: u32, c: bool) -> i32 {
    let sp = fx.enter(frame) + frame - 0x550;
    if !c {
        fx.mem.set32(sp + 0xBC, n);
    }
    let rand = if c { (Some(n as u32), at + 0x618) } else { (None, at + 0x434) };
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x2C;
        }
        if !c {
            fx.mem.set32(sp + 0xA0, i);
        }
        arc_one(fx, rand, sp, g);
        i += 1;
    }
    if !c {
        fx.mem.set32(sp + 0xA0, i);
    }
    fx.leave(frame);
    1
}

/// `rand`: rand's saved s0 (`None`: the record) and return address.
fn arc_one(fx: &mut Fx, rand: (Option<u32>, u32), sp: u32, g: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let arc = fx.mem.read_u32(g + 8);
    if arc == 0 {
        return;
    }
    let pool = fx.mem.read_u32(arc + 8);
    for t in 0..2u32 {
        let buf = sp + 0xC0 + 0x224 * t;
        fx.mem.write_u32(buf, 0);
        let mut k = 0u32;
        while k < u32::from(fx.mem.read_u8(arc + 0x31)) {
            let h = fx.mem.i16(arc + 0x12 * t + 0xC + 2 * k);
            k += 1;
            if h < 0 {
                continue;
            }
            let p = pool.wrapping_add((h << 4) as u32);
            for (c, o) in [(4u32, 4u32), (8, 0x24), (0xC, 0x44)] {
                let m = fx.mem.read_u32(buf);
                let v = fx.mem.read_u32(p + c);
                fx.mem.write_u32(buf + o + 4 * m, v);
            }
            let m = fx.mem.read_u32(buf);
            fx.mem.write_u32(buf, m + 1);
        }
    }
    if fx.mem.read_u32(sp + 0xC0) != fx.mem.read_u32(sp + 0x2E4) {
        return;
    }
    if super::arc::resample(fx, sp + 0xC0) == 0 || super::arc::resample(fx, sp + 0x2E4) == 0 {
        return;
    }
    let mut k = 0;
    while k < fx.mem.u8(g + 0x17) {
        let q = 16 * k as u32;
        let t = fx.mem.f(arc + 0x38);
        for (dst, a, b) in [(0x520u32, 0x348u32, 0x124u32), (0x524, 0x34C, 0x128), (0x528, 0x350, 0x12C), (0x530, 0x358, 0x134), (0x534, 0x35C, 0x138), (0x538, 0x360, 0x13C)] {
            let a = fx.mem.f(sp + a + q);
            let b = fx.mem.f(sp + b + q);
            fx.mem.setf(sp + dst, ee::add(a, ee::mul(t, ee::sub(b, a))));
        }
        for c in 0..4u32 {
            let v = fx.mem.read_u32(sp + 0x348 + q + 4 * c);
            fx.mem.write_u32(sp + 0x540 + 4 * c, v);
        }
        let (rays, max) = (fx.mem.read_u32(g + 0xC), fx.mem.u8(g + 0x14));
        let i = super::exe::first_free(fx, rays, 0x74, max);
        if i >= 0 {
            arc_ray(fx, rand, sp, g, (i * 0x74) as u32, k);
        }
        k += 1;
    }
}

fn arc_ray(fx: &mut Fx, rand: (Option<u32>, u32), sp: u32, g: u32, off: u32, k: i32) {
    let r = fx.mem.read_u32(g + 0xC).wrapping_add(off);
    let less = if fx.mem.u8(g + 4) & 1 != 0 { k } else { 0 };
    let count = (fx.mem.u8(g + 0x15) - less) & 0xFF;
    super::exe::set_counter4(fx, r, count, 0, 0xFF);
    let (from, to) = if fx.mem.u8(g + 4) & 2 != 0 { (0x520u32, 0x530u32) } else { (0x540, 0x520) };
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(sp + to + 4 * c), fx.mem.f(sp + from + 4 * c));
        fx.mem.setf(sp + 0x510 + 4 * c, v);
    }
    let len = super::beam::length3(fx, sp + 0x510);
    if !ee_le(len, 1.0) {
        super::trail::normalize3(fx, sp + 0x510);
    }
    let len = super::beam::length3(fx, sp + 0x510);
    let r = fx.mem.read_u32(g + 0xC).wrapping_add(off);
    fx.mem.setf(r + 0x10, len);
    let s = fx.mem.f(g + 0x18);
    // c.eq.s with 0: false for NaN / infinities either way.
    if s != 0.0 {
        let v = ee::mul(fx.mem.f(r + 0x10), s);
        fx.mem.setf(r + 0x10, v);
    }
    let v = fx.mem.read_u32(g + 0x1C);
    fx.mem.write_u32(r + 0x14, v);
    fx.mem.write_u8(r + 4, 2);
    fx.mem.write_u8(r + 5, 0);
    let v = fx.mem.read_u8(g + 0x16);
    fx.mem.write_u8(r + 8, v);
    let v = fx.mem.read_u32(g + 0x20);
    fx.mem.write_u32(r + 0x18, v);
    let x = fx.rand_from(rand.0.unwrap_or(g), rand.1);
    fx.mem.set8(r + 9, x >> 4);
    let v = fx.mem.read_u32(g + 0x10);
    fx.mem.write_u32(r + 0xC, v);
    for c in 0..8u32 {
        let v = fx.mem.read_u8(g + 0x24 + c);
        fx.mem.write_u8(r + 0x1C + c, v);
    }
    for c in 0..4u32 {
        let v = fx.mem.read_u32(sp + 0x520 + 4 * c);
        fx.mem.write_u32(r + 0x24 + 4 * c, v);
        let v = fx.mem.read_u32(sp + 0x530 + 4 * c);
        fx.mem.write_u32(r + 0x34 + 4 * c, v);
    }
    super::exe::aim_angles(fx, sp + 0x510, r + 0x54);
}
