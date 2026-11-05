//! Smears of the effect library: a trail of
//! textured screen-space squares between two of the attacker's nodes,
//! shown while the line between them faces the camera.
use super::exe::{camera_vp_into, countdown, first_free, node_source, set_counter, tex_rect_strip_a3};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, CAMERA_TABLE};

/// The record's spec pair (0x30 bytes of rodata) that `smears` loads with
/// `lui` / `addiu` at +0x38 / +0x3C.
fn spec_source(fx: &mut Fx, at: u32, o: u32) -> u32 {
    let hi = fx.mem.read_u32(at + o) & 0xFFFF;
    let lo = fx.mem.read_u32(at + o + 4) & 0xFFFF;
    (hi << 16).wrapping_add(lo as u16 as i16 as i32 as u32)
}

/// 0x1A2C70 of `v` (its residue left, return address `ra`) into the u16
/// at `a`.
fn fix16(fx: &mut Fx, a: u32, v: f32, ra: u32) {
    fx.fix_residue(v, ra);
    fx.mem.write_u16(a, ee::fixunssfsi(v) as u16);
}

/// 0x1A2C70 of `v` (residue left) into the byte at each of `at`.
fn fix8(fx: &mut Fx, at: &[u32], v: f32, ra: u32) {
    fx.fix_residue(v, ra);
    let b = ee::fixunssfsi(v) as u8;
    for &a in at {
        fx.mem.write_u8(a, b);
    }
}

/// 47f21f86ab26 [3 B records; b715 0x1E02E60, 672 words] [F]:
/// `smears(recs, ot, n) -> 1` steps and draws `n` records (0x48 bytes; a
/// 0x450 frame, the record's spec pair copied to sp + 0x350, the
/// descriptor at sp + 0x3B8: RGBAQ / ST / XYZ2 x 4, Q 1.0). The point
/// pool is the first record's +8 (0x10-byte points: count, screen x, y,
/// z). Per record: countdown 0x101AF0; the head = the attacker's node +0x21
/// (0x101BA0, +5 = 1) + offset +0x38; the tail node +0x22; the line's
/// direction (head - tail) and the camera's +0x10 vector (0x1FBF20(0)),
/// both normalized (0x20C7E0): with their dot (0x20C910) above +0x28 the
/// history (+0xC, i16 x +0x1F) is freed and nothing drawn. Else the
/// camera (0x20E2B0 into sp + 0xD0); the history shifted (each live
/// point's count set to the record's, the oldest freed); a new point
/// (first free of +0x1E, 0x101B40) at the head projected (0x204D80: screen
/// x / y / z, w into +0x30; history -1 when off); the live points more
/// than 32 pixels apart sampled into sp + 0x110 (x, y, 0) and resampled
/// (0x102550) when three or more (else the first alone, two steps). Texture
/// +0x20 (0x202530(5)); colour +0x36.. with alpha +0x37 (x count / 8 over
/// the last 8) fading along the steps; z 0xFFFFFF; half width h = +0x2C /
/// (+0x30 / 256). The first sample a square (x +- h, y +- h, spec A);
/// each next one two quads from the previous sample (spec B): a vertical
/// band (y +- h at both) and a horizontal one (x +- h at both), key 0.
pub fn smears(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    smears_by(fx, at, recs, ot, n, false, &SMEARS).0
}

/// A smears compile: the spec pair's `lui` offset and the return
/// addresses (from the function) of its 19 0x1A2C70 calls in the
/// unsquared path: the first step's five, the next steps' seven + five,
/// the two that keep the sample.
pub struct Layout {
    spec: u32,
    ra: [u32; 19],
    /// The frame, how much lower the locals sit (the same caller-relative
    /// addresses) and whether n / the counter have their sp + 0xCC / 0xB0
    /// slots.
    frame: u32,
    shift: u32,
    slots: bool,
}

const SMEARS: Layout = Layout {
    frame: 0x450,
    shift: 0,
    slots: true,
    spec: 0x38,
    ra: [0x73C, 0x758, 0x770, 0x784, 0x7A4, 0x7F4, 0x814, 0x85C, 0x898, 0x8AC, 0x8C0, 0x8E4, 0x93C, 0x978, 0x98C, 0x99C, 0x9C4, 0x9F4, 0xA00],
};

/// 8a6ed04e164a [2 B records; b508 0x1E05250, 740 words] [F word-compare,
/// helper D9]: [`smears`] compiled unscheduled (f22 saved at sp + 8, the
/// unsigned conversions inline): the spec pair at +0x48 / +0x4C, its
/// 0x1A2C70 calls at other return addresses; otherwise the same steps.
pub fn smears_c(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    smears_by(fx, at, recs, ot, n, false, &SMEARS_C).0
}

/// [`smears_c`] and the a3 it leaves (as [`smears_a3`]).
pub fn smears_c_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    smears_by(fx, at, recs, ot, n, false, &SMEARS_C)
}

const SMEARS_C: Layout = Layout {
    frame: 0x450,
    shift: 0,
    slots: true,
    spec: 0x48,
    ra: [0x7B4, 0x7D0, 0x7F4, 0x808, 0x82C, 0x888, 0x8AC, 0x900, 0x94C, 0x970, 0x984, 0x9B4, 0xA20, 0xA6C, 0xA84, 0xA98, 0xAC4, 0xAFC, 0xB10],
};

/// 239ad2781e06 [3 B records; card 241's 0x1E05CB0, 768 words] (helper
/// A19, 1 Nov 2026) [F]: [`smears_c`] recompiled in a 0x420 frame: the
/// locals 0x30 lower (the same caller-relative addresses), n / the OT /
/// the counter in registers (the old sp + 0xCC / 0xB0 slots would be this
/// frame's saves: not written), the history slots re-read per use; the
/// spec pair at +0x40 / +0x44, its 0x1A2C70 calls at other returns.
pub fn smears_d(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    smears_by(fx, at, recs, ot, n, false, &SMEARS_D).0
}

const SMEARS_D: Layout = Layout {
    frame: 0x420,
    shift: 0x30,
    slots: false,
    spec: 0x40,
    ra: [0x834, 0x854, 0x874, 0x894, 0x8B8, 0x914, 0x93C, 0x98C, 0x9D8, 0x9F4, 0xA10, 0xA34, 0xA98, 0xAE4, 0xB00, 0xB1C, 0xB40, 0xB74, 0xB8C],
};

/// [`smears`] and the a3 it leaves: the last spec setter's
/// (0x101EE0, [`tex_rect_strip_a3`]; the submits keep it), `None` when no
/// record drew (the caller's a3 kept) [run b715: frame_fade_b's 0x207EF0].
pub fn smears_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    smears_by(fx, at, recs, ot, n, false, &SMEARS)
}

/// e21227568d6c [b444 0x1E03C60, 536 words] [F word-compare, helper C11]:
/// [`smears`] (register-renamed) with the steps capped at +0x23 and every
/// sample drawn as the square alone, its alpha fade x (steps - k).
pub fn smears_a(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    smears_by(fx, at, recs, ot, n, true, &SMEARS).0
}

fn smears_by(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, square: bool, l: &Layout) -> (i32, Option<u32>) {
    let mut a3 = None;
    let sp = fx.enter(l.frame);
    let lsp = sp - l.shift;
    let pool = fx.mem.read_u32(recs + 8);
    let src = spec_source(fx, at, l.spec);
    for k in (0..0x30u32).step_by(8) {
        let v = fx.mem.read_u64(src + k);
        fx.mem.write_u64(lsp + 0x350 + k, v);
    }
    let mut g = recs;
    if l.slots {
        fx.mem.set32(sp + 0xCC, n);
        fx.mem.write_u32(sp + 0xB0, 0);
        while fx.mem.i32(sp + 0xB0) < n {
            if fx.mem.i32(sp + 0xB0) != 0 {
                g += 0x48;
            }
            one(fx, at, sp, g, pool, ot, square, l, &mut a3);
            let i = fx.mem.i32(sp + 0xB0) + 1;
            fx.mem.set32(sp + 0xB0, i);
        }
    } else {
        let mut i = 0;
        while i < n {
            if i != 0 {
                g += 0x48;
            }
            one(fx, at, lsp, g, pool, ot, square, l, &mut a3);
            i += 1;
        }
    }
    fx.leave(l.frame);
    (1, a3)
}

#[allow(clippy::too_many_arguments)]
fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, pool: u32, ot: u32, square: bool, l: &Layout, a3: &mut Option<u32>) {
    if countdown(fx, g) == 0 {
        return;
    }
    fx.mem.write_u8(g + 5, 1);
    let s = fx.mem.read_u8(g + 0x21);
    fx.mem.write_u8(g + 6, s);
    node_source(fx, g, sp + 0x340);
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(sp + 0x340 + 4 * k), fx.mem.f(g + 0x38 + 4 * k));
        fx.mem.setf(sp + 0x340 + 4 * k, v);
    }
    // 0x1FBF20(0): the camera object's +0x10 (0 without one).
    let cam = fx.mem.read_u32(CAMERA_TABLE);
    let e = if cam != 0 { cam.wrapping_add(0x10) } else { 0 };
    for k in 0..4u32 {
        let v = fx.mem.read_u32(e.wrapping_add(4 * k));
        fx.mem.write_u32(sp + 0x3A0 + 4 * k, v);
    }
    let s = fx.mem.read_u8(g + 0x22);
    fx.mem.write_u8(g + 6, s);
    node_source(fx, g, sp + 0x380);
    for k in 0..3u32 {
        let v = ee::sub(fx.mem.f(sp + 0x340 + 4 * k), fx.mem.f(sp + 0x380 + 4 * k));
        fx.mem.setf(sp + 0x390 + 4 * k, v);
    }
    super::trail::normalize3(fx, sp + 0x390);
    super::trail::normalize3(fx, sp + 0x3A0);
    let a = |fx: &mut Fx, k: u32| fx.mem.f(sp + 0x390 + 4 * k);
    let b = |fx: &mut Fx, k: u32| fx.mem.f(sp + 0x3A0 + 4 * k);
    let (y, x, z) = (ee::mul(a(fx, 1), b(fx, 1)), ee::mul(a(fx, 0), b(fx, 0)), ee::mul(a(fx, 2), b(fx, 2)));
    let dot = ee::add(ee::add(x, y), z);
    let len = fx.mem.u8(g + 0x1F) as u32;
    if !ee_le(dot, fx.mem.f(g + 0x28)) {
        for k in 0..len {
            let h = fx.mem.i16(g + 0xC + 2 * k);
            if h < 0 {
                continue;
            }
            fx.mem.write_u8(pool + 16 * h as u32, 0);
            fx.mem.set16(g + 0xC + 2 * k, -1);
        }
        return;
    }
    camera_vp_into(fx, sp + 0xD0);
    let mut k = len;
    while k > 0 {
        let prev = fx.mem.i16(g + 0xA + 2 * k);
        if prev >= 0 {
            let c = fx.mem.read_u8(g);
            fx.mem.write_u8(pool + 16 * prev as u32, c);
        }
        let v = fx.mem.read_u16(g + 0xA + 2 * k);
        fx.mem.write_u16(g + 0xC + 2 * k, v);
        k -= 1;
    }
    let last = g + 0xC + 2 * len;
    if fx.mem.i16(last) >= 0 {
        let old = fx.mem.i16(last);
        fx.mem.write_u8(pool + 16 * old as u32, 0);
        fx.mem.set16(last, -1);
    }
    if fx.mem.read_u8(g) == 0 {
        return;
    }
    let max = fx.mem.u8(g + 0x1E);
    let i = first_free(fx, pool, 0x10, max);
    if i < 0 {
        fx.mem.set16(g + 0xC, -1);
    } else {
        fx.mem.set16(g + 0xC, i);
        let p = pool + 16 * fx.mem.i16(g + 0xC) as u32;
        let m = fx.read_m4(sp + 0xD0);
        if fx.project_at(sp + 0x340, &m, sp + 0x3B0, sp + 0x44C) {
            let c = fx.mem.u8(g);
            set_counter(fx, p, c, 0);
            let x = ee::from_i32(fx.mem.u16(sp + 0x3B0));
            fx.mem.setf(p + 4, x);
            let y = ee::from_i32(fx.mem.u16(sp + 0x3B2));
            fx.mem.setf(p + 8, y);
            let z = fx.xyz_z(sp + 0x3B0) as i32;
            fx.mem.setf(p + 0xC, ee::from_i32(z));
            let w = fx.mem.read_u32(sp + 0x44C);
            fx.mem.write_u32(g + 0x30, w);
        } else {
            fx.mem.set16(g + 0xC, -1);
        }
    }
    fx.mem.write_u32(sp + 0x110, 0);
    for k in 0..len {
        let h = fx.mem.i16(g + 0xC + 2 * k);
        if h < 0 {
            continue;
        }
        let p = pool + 16 * h as u32;
        let m = fx.mem.read_u32(sp + 0x110);
        let (x, y) = (fx.mem.f(p + 4), fx.mem.f(p + 8));
        fx.mem.setf(sp + 0x114 + 4 * m, x);
        fx.mem.setf(sp + 0x134 + 4 * m, y);
        fx.mem.write_u32(sp + 0x154 + 4 * m, 0);
        if m == 0 {
            fx.mem.write_u32(sp + 0x110, 1);
            continue;
        }
        let dx = ee::sub(fx.mem.f(sp + 0x110 + 4 * m), fx.mem.f(p + 4));
        let dy = ee::sub(fx.mem.f(sp + 0x130 + 4 * m), fx.mem.f(p + 8));
        let d = fx.sqrtf(ee::add(ee::mul(dx, dx), ee::mul(dy, dy)));
        if !ee_le(d, 32.0) {
            fx.mem.write_u32(sp + 0x110, m + 1);
        }
    }
    let steps = if fx.mem.i32(sp + 0x110) < 3 {
        for (from, to) in [(0x114u32, 0x174u32), (0x134, 0x178), (0x154, 0x17C)] {
            let v = fx.mem.read_u32(sp + from);
            fx.mem.write_u32(sp + to, v);
        }
        2
    } else {
        super::arc::resample(fx, sp + 0x110);
        (fx.mem.i32(sp + 0x110) - 1) << 2
    };
    let cap = fx.mem.u8(g + 0x23);
    let steps = if square && cap < steps { cap } else { steps };
    let id = fx.mem.u8(g + 0x20);
    let tex = fx.texture(5, id);
    let d = sp + 0x3B8;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    for (src, k) in [(0x36u32, 0u32), (0x35, 1), (0x34, 2)] {
        let v = fx.mem.read_u8(g + src);
        for o in [0x410u32, 0x3F8, 0x3E0, 0x3C8] {
            fx.mem.write_u8(sp + o + k, v);
        }
    }
    for o in [0x414u32, 0x3FC, 0x3E4, 0x3CC] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let c = fx.mem.u8(g);
    let a = ee::from_i32(fx.mem.u8(g + 0x37));
    let alpha = if c < 8 { ee::div(ee::mul(ee::from_i32(c), a), 8.0) } else { a };
    for o in [0x420u32, 0x3F0, 0x408, 0x3D8] {
        fx.set_xyz_z(sp + o, 0xFF_FFFF);
    }
    let fade = ee::div(alpha, ee::from_i32(steps));
    let h = ee::div(fx.mem.f(g + 0x2C), ee::div(fx.mem.f(g + 0x30), 256.0));
    let r = |o: u32| at + o;
    let q = |i: usize| at + l.ra[i];
    let (v0x, v0y, v1x, v1y, v2x, v2y, v3x, v3y) = (sp + 0x3D8, sp + 0x3DA, sp + 0x3F0, sp + 0x3F2, sp + 0x408, sp + 0x40A, sp + 0x420, sp + 0x422);
    let alphas = [sp + 0x413, sp + 0x3FB, sp + 0x3E3, sp + 0x3CB];
    for s0 in 0..steps - 1 {
        let s5 = sp + 0x110 + 16 * s0 as u32;
        if square {
            *a3 = Some(tex_rect_strip_a3(fx, d, sp + 0x350, tex));
            let x = fx.mem.f(s5 + 0x64);
            fix16(fx, v2x, ee::sub(x, h), r(0x744));
            let v = fx.mem.read_u16(v2x);
            fx.mem.write_u16(v0x, v);
            let x = fx.mem.f(s5 + 0x64);
            fix16(fx, v3x, ee::add(x, h), r(0x760));
            let v = fx.mem.read_u16(v3x);
            fx.mem.write_u16(v1x, v);
            let y = fx.mem.f(s5 + 0x68);
            fix16(fx, v1y, ee::sub(y, h), r(0x774));
            let v = fx.mem.read_u16(v1y);
            fx.mem.write_u16(v0y, v);
            let y = fx.mem.f(s5 + 0x68);
            fix16(fx, v3y, ee::add(y, h), r(0x788));
            let v = fx.mem.read_u16(v3y);
            fx.mem.write_u16(v2y, v);
            fix8(fx, &alphas, ee::mul(fade, ee::from_i32(steps - s0)), r(0x7A8));
            fx.submit(d, ot, 0);
            let x = fx.mem.f(s5 + 0x64);
            fix16(fx, sp + 0x3B0, x, r(0x7D0));
            let y = fx.mem.f(s5 + 0x68);
            fix16(fx, sp + 0x3B2, y, r(0x7DC));
            continue;
        }
        if s0 == 0 {
            *a3 = Some(tex_rect_strip_a3(fx, d, sp + 0x350, tex));
            let x = fx.mem.f(s5 + 0x64);
            fix16(fx, v2x, ee::sub(x, h), q(0));
            let v = fx.mem.read_u16(v2x);
            fx.mem.write_u16(v0x, v);
            let x = fx.mem.f(s5 + 0x64);
            fix16(fx, v3x, ee::add(x, h), q(1));
            let v = fx.mem.read_u16(v3x);
            fx.mem.write_u16(v1x, v);
            let y = fx.mem.f(s5 + 0x68);
            fix16(fx, v1y, ee::sub(y, h), q(2));
            let v = fx.mem.read_u16(v1y);
            fx.mem.write_u16(v0y, v);
            let y = fx.mem.f(s5 + 0x68);
            fix16(fx, v3y, ee::add(y, h), q(3));
            let v = fx.mem.read_u16(v3y);
            fx.mem.write_u16(v2y, v);
            fix8(fx, &alphas, ee::mul(fade, ee::from_i32(steps)), q(4));
            fx.submit(d, ot, 0);
        } else {
            *a3 = Some(tex_rect_strip_a3(fx, d, sp + 0x368, tex));
            let left = steps - s0;
            fix8(fx, &[sp + 0x3FB, sp + 0x3CB], ee::mul(fade, ee::from_i32(left)), q(5));
            fix8(fx, &[sp + 0x413, sp + 0x3E3], ee::mul(fade, ee::from_i32(left - 1)), q(6));
            let py = ee::from_i32(fx.mem.u16(sp + 0x3B2));
            fix16(fx, v0y, ee::sub(py, h), q(7));
            fix16(fx, v2y, ee::add(py, h), q(8));
            let y = fx.mem.f(s5 + 0x68);
            fix16(fx, v1y, ee::sub(y, h), q(9));
            let y = fx.mem.f(s5 + 0x68);
            fix16(fx, v3y, ee::add(y, h), q(10));
            let px = fx.mem.read_u16(sp + 0x3B0);
            fx.mem.write_u16(v2x, px);
            fx.mem.write_u16(v0x, px);
            let x = fx.mem.f(s5 + 0x64);
            fix16(fx, v3x, x, q(11));
            let v = fx.mem.read_u16(v3x);
            fx.mem.write_u16(v1x, v);
            fx.submit(d, ot, 0);
            let px = ee::from_i32(fx.mem.u16(sp + 0x3B0));
            fix16(fx, v0x, ee::sub(px, h), q(12));
            fix16(fx, v2x, ee::add(px, h), q(13));
            let x = fx.mem.f(s5 + 0x64);
            fix16(fx, v1x, ee::sub(x, h), q(14));
            let x = fx.mem.f(s5 + 0x64);
            fix16(fx, v3x, ee::add(x, h), q(15));
            let py = fx.mem.read_u16(sp + 0x3B2);
            fx.mem.write_u16(v2y, py);
            fx.mem.write_u16(v0y, py);
            let y = fx.mem.f(s5 + 0x68);
            fix16(fx, v3y, y, q(16));
            let v = fx.mem.read_u16(v3y);
            fx.mem.write_u16(v1y, v);
            fx.submit(d, ot, 0);
        }
        let x = fx.mem.f(s5 + 0x64);
        fix16(fx, sp + 0x3B0, x, q(17));
        let y = fx.mem.f(s5 + 0x68);
        fix16(fx, sp + 0x3B2, y, q(18));
    }
}
