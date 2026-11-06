//! Arcs of the effect library: two trails of
//! points left by two skeleton nodes, resampled by a cubic spline
//! (0x102550 / 0x102390) and drawn as a strip between them and as glow
//! segments along a blend of the two.
use super::exe::{camera_vp_into, countdown, first_free, node_source, set_counter, tex_rect_strip, tex_rect_strip_a3};
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx};

/// 0x102390(n, t, y, m) [F]: the second-derivative weights `m` (n f32)
/// of the values `y` at the knots `t` (a 0x40 leaf frame: steps h at sp,
/// slopes at sp + 0x24, overwritten by the pivots): m[0] = m[n-1] = 0,
/// a forward sweep m[k+1] = (d[k+1] - d[k]) - h[k] / D m[k], D' = 2 (t[k+2]
/// - t[k]) - h[k] / D h[k], then m[i] = (m[i] - h[i] m[i+1]) / D[i-1] back
/// from n-2. The original's operation order and EE rounding are kept.
pub fn spline_weights(fx: &mut Fx, n: i32, t: u32, y: u32, m: u32) {
    let n = n as u32;
    fx.mem.write_u32(m.wrapping_add((n << 2).wrapping_sub(4)), 0);
    let sp = fx.enter(0x40);
    fx.mem.write_u32(m, 0);
    let (h, d) = (sp, sp + 0x24);
    let mut i = 0u32;
    if 0 < n as i32 - 1 {
        loop {
            let v = ee::sub(fx.mem.f(t + 4 * i + 4), fx.mem.f(t + 4 * i));
            fx.mem.setf(h + 4 * i, v);
            let dy = ee::sub(fx.mem.f(y + 4 * i + 4), fx.mem.f(y + 4 * i));
            let hv = fx.mem.f(h + 4 * i);
            fx.mem.setf(d + 4 * i, ee::div(dy, hv));
            i += 1;
            if (i as i32) >= n as i32 - 1 {
                break;
            }
        }
    }
    let acc = ee::sub(fx.mem.f(d + 4), fx.mem.f(d));
    let v = ee::sub(acc, ee::mul(fx.mem.f(h), fx.mem.f(m)));
    fx.mem.setf(m + 4, v);
    let v = ee::mul(2.0, ee::sub(fx.mem.f(t + 8), fx.mem.f(t)));
    fx.mem.setf(d, v);
    let a2 = n as i32 - 2;
    if a2 >= 2 {
        let mut j = 0u32;
        loop {
            let r = ee::div(fx.mem.f(h + 4 + 4 * j), fx.mem.f(d + 4 * j));
            let acc = ee::sub(fx.mem.f(d + 4 * j + 8), fx.mem.f(d + 4 * j + 4));
            let v = ee::sub(acc, ee::mul(r, fx.mem.f(m + 4 + 4 * j)));
            fx.mem.setf(m + 8 + 4 * j, v);
            let acc = ee::mul(2.0, ee::sub(fx.mem.f(t + 4 + 4 * j + 8), fx.mem.f(t + 4 + 4 * j)));
            let v = ee::sub(acc, ee::mul(r, fx.mem.f(h + 4 + 4 * j)));
            fx.mem.setf(d + 4 * j + 4, v);
            j += 1;
            if (j as i32) + 1 >= a2 {
                break;
            }
        }
    }
    let k = n.wrapping_sub(2);
    let f1 = ee::mul(fx.mem.f(sp + 4 * k), fx.mem.f(m + 4 * (n - 1)));
    let v = ee::sub(fx.mem.f(m + 4 * k), f1);
    fx.mem.setf(m + 4 * k, v);
    let mut i = a2;
    while i > 0 {
        let iu = i as u32;
        let f2 = ee::mul(fx.mem.f(h + 4 * iu), fx.mem.f(m + 4 * iu + 4));
        let f1 = ee::sub(fx.mem.f(m + 4 * iu), f2);
        let v = ee::div(f1, fx.mem.f(sp + 0x20 + 4 * iu));
        fx.mem.setf(m + 4 * iu, v);
        i -= 1;
    }
    fx.leave(0x40);
}

/// 0x102550(p) [F]: a path of p[0] = n points (3..8; else 0) with x at p
/// + 4, y at + 0x24, z at + 0x44 (f32 x 8 each) resampled at 4 (n - 1)
/// even steps of its chord length (a 0xF0 frame: knots at sp + 0x70, the
/// cumulative lengths 0x1A7520 sqrtf divided by the total; weights of x,
/// y, z by 0x102390 at sp + 0x90 / 0xB0 / 0xD0): sample s (u = s / (4 (n
/// - 1)), the knot interval by a binary search with 0x1A2B80) as the
/// cubic of its interval into p + 0x64 + 0x10 s (x, y, z). Returns 1.
pub fn resample(fx: &mut Fx, p: u32) -> i32 {
    let sp = fx.enter(0xF0);
    let n = fx.mem.i32(p);
    if n < 3 || n >= 9 {
        fx.leave(0xF0);
        return 0;
    }
    fx.mem.write_u32(sp + 0x70, 0);
    let mut i = 1i32;
    while i < fx.mem.i32(p) {
        let s = p + 4 * i as u32;
        let dx = ee::sub(fx.mem.f(s + 4), fx.mem.f(s));
        let dy = ee::sub(fx.mem.f(s + 0x24), fx.mem.f(s + 0x20));
        let dz = ee::sub(fx.mem.f(s + 0x44), fx.mem.f(s + 0x40));
        let acc = ee::add(ee::mul(dx, dx), ee::mul(dy, dy));
        let l = fx.sqrtf(ee::add(acc, ee::mul(dz, dz)));
        let a = sp + 0x70 + 4 * i as u32;
        let v = ee::add(fx.mem.f(a - 4), l);
        fx.mem.setf(a, v);
        i += 1;
    }
    let mut i = 1i32;
    while i < fx.mem.i32(p) {
        let n = fx.mem.i32(p);
        let a = sp + 0x70 + 4 * i as u32;
        let x = fx.mem.f(a);
        let total = fx.mem.f((sp + 0x6C).wrapping_add((n as u32) << 2));
        fx.mem.setf(a, ee::div(x, total));
        i += 1;
    }
    for (vals, w) in [(4u32, 0x90u32), (0x24, 0xB0), (0x44, 0xD0)] {
        let n = fx.mem.i32(p);
        spline_weights(fx, n, sp + 0x70, p + vals, sp + w);
    }
    let n = fx.mem.i32(p);
    let step = ee::div(1.0, ee::mul(4.0, ee::from_i32(n - 1)));
    let mut u = 0.0f32;
    let mut s = 0i32;
    let mut out = p;
    while s < (fx.mem.i32(p) - 1) << 2 {
        for (vals, w, o) in [(4u32, 0x90u32, 0x64u32), (0x24, 0xB0, 0x68), (0x44, 0xD0, 0x6C)] {
            let (mut lo, mut hi) = (0i32, fx.mem.i32(p) - 1);
            while lo < hi {
                let mid = ee::fixsfsi(ee::div(ee::from_i32(lo + hi), 2.0));
                let tm = fx.mem.f((sp + 0x70).wrapping_add((mid as u32) << 2));
                if ee_lt(tm, u) {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            if lo > 0 {
                lo -= 1;
            }
            let k = (lo as u32) << 2;
            let y0 = fx.mem.f((p + vals).wrapping_add(k));
            let y1 = fx.mem.f((p + vals + 4).wrapping_add(k));
            let t0 = fx.mem.f((sp + 0x70).wrapping_add(k));
            let t1 = fx.mem.f((sp + 0x74).wrapping_add(k));
            let m0 = fx.mem.f((sp + w).wrapping_add(k));
            let m1 = fx.mem.f((sp + w + 4).wrapping_add(k));
            let dy = ee::sub(y1, y0);
            let h = ee::sub(t1, t0);
            let x = ee::sub(u, t0);
            let slope = ee::div(dy, h);
            let acc = ee::sub(slope, ee::mul(h, ee::add(ee::mul(2.0, m0), m1)));
            let c = ee::div(ee::mul(x, ee::sub(m1, m0)), h);
            let q = ee::add(ee::mul(3.0, m0), c);
            let v = ee::add(y0, ee::mul(x, ee::add(acc, ee::mul(x, q))));
            fx.mem.setf(out + o, v);
        }
        s += 1;
        u = ee::add(u, step);
        out += 0x10;
    }
    fx.leave(0xF0);
    1
}

/// Copy `n` bytes as the original's lq / ld pairs (16-byte then 8-byte
/// aligned source and destination).
fn copy_q(fx: &mut Fx, src: u32, dst: u32, quads: u32, tail: bool) {
    for q in 0..quads {
        let (s, d) = ((src + 0x10 * q) & !15, (dst + 0x10 * q) & !15);
        for k in (0..16).step_by(8) {
            let v = fx.mem.read_u64(s + k);
            fx.mem.write_u64(d + k, v);
        }
    }
    if tail {
        let v = fx.mem.read_u64((src + 0x10 * quads) & !7);
        fx.mem.write_u64((dst + 0x10 * quads) & !7, v);
    }
}

fn copy_words(fx: &mut Fx, src: u32, dst: u32, n: u32) {
    for k in 0..n {
        let v = fx.mem.read_u32(src + 4 * k);
        fx.mem.write_u32(dst + 4 * k, v);
    }
}

/// bac069293c10 [43 B records; b123 0x1E01530, 1184 words] [F]:
/// `arcs(recs, ot, n) -> 1` steps and draws `n` records (0x60 bytes; a
/// 0x790 frame, the descriptor at sp + 0x700; three 0x18-byte strip specs
/// (record-relative `lui` / `addiu` at +0x48 / +0x4C) at sp + 0x650, a
/// glow spec (+0x7C / +0x80) at sp + 0x6A0 and a core spec (+0x98 /
/// +0x9C) at sp + 0x6C0). The point pool (16-byte entries: count, delay,
/// position) is the first record's +8. Per record: countdown 0x101AF0;
/// the camera's view-projection; the self unit's nodes +0x32 / +0x33
/// (0x101BA0); per track (+0xC / +0x1E, i16 x 9) the entries shift by one
/// (each still live takes the count), the one past +0x31 is freed; a free
/// pool entry (0x101C30, +0x30 entries) starts at each node; the two
/// tracks' points become two paths resampled by 0x102550 (equal lengths
/// only). Per sample step: a blend +0x38 of the two paths and along it
/// +0x3C; flag 1: a strip of four vertices (spec +0x34, its v1 from +0x36,
/// colours +0x44.. / +0x48.., alpha +0x47 faded over the length and the
/// first 8 counts); flags 2 / 4: 1 / 3 glow segments (+0x50 / +0x54
/// offsets for the outer two; flag 8: the core spec and a line to the +0x3C
/// point), width +0x40 x 256 / w, colour +0x4C.., alpha +0x4F, each drawn
/// twice (across y, then across x).
///
/// fb70e0bee134 [7 B records; b063 0x1E01730, 1296 words] is the same
/// routine with a 0x750 frame (same record-relative pairs; proven equal
/// at function level, only the dead frame differs).
pub fn arcs(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    arcs_a3(fx, at, recs, ot, n).0
}

/// [`arcs`] and the a3 it leaves (helper B10, for a later boundary call
/// that reports it) [F]: the strips' spec address (the first `lui` /
/// `addiu`), then per step of a drawing record sp + 0x5C8, a projection's
/// depth pointer (sp + 0x640..0x64C) when it fails or falls outside the
/// window, after a drawn strip tex_rect_strip's (kind 0) or the low word
/// of sp + 0x788 (kinds 1 / 2), after a drawn line the second end's Z.
/// [H] a record that stops before its steps (unequal sample counts, a
/// failed resample, under two samples) leaves what its node / pool /
/// texture calls left: not modelled (the previous value is kept).
pub fn arcs_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, u32) {
    let strips = rel(fx, at, 0x48, 0x4C);
    let glow = rel(fx, at, 0x7C, 0x80);
    let core = rel(fx, at, 0x98, 0x9C);
    let mut a3 = strips;
    let r = body(fx, [strips, glow, core], recs, ot, n, &mut a3);
    (r, a3)
}

/// fae628ffdd27 [11 B records; b103 0x1E03710, 1096 words] [F]: the same
/// routine compiled with a 0x7D0 frame whose locals sit at [`arcs`]'
/// addresses (its sp + 0x40 = this one's sp) and the three specs' `lui` /
/// `addiu` at +0x3C / +0x40, +0x60 / +0x64, +0x80 / +0x88.
pub fn arcs_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    arcs_b_a3(fx, at, recs, ot, n).0
}

/// [`arcs_b`] and the a3 it leaves [F]: this
/// compile starts with the low word of the strips spec's second quadword
/// (`lq a3, 0x10(t2)` of its copy; 0x3F in b040), then as [`arcs_a3`]. A
/// counted record (count set, no delay) that returns before a step leaves
/// its k loops' a3 = 0, not the spec word; both users pass n = 1, so the
/// first record is the one that counts.
pub fn arcs_b_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, u32) {
    let strips = rel(fx, at, 0x3C, 0x40);
    let glow = rel(fx, at, 0x60, 0x64);
    let core = rel(fx, at, 0x80, 0x88);
    let spec = fx.mem.read_u32(strips + 0x10);
    let counted = fx.mem.read_u8(recs) != 0 && fx.mem.read_u8(recs + 1) == 0;
    let mut a3 = spec;
    let r = body(fx, [strips, glow, core], recs, ot, n, &mut a3);
    (r, if counted && a3 == spec { 0 } else { a3 })
}

fn body(fx: &mut Fx, [strips, glow, core]: [u32; 3], recs: u32, ot: u32, n: i32, a3: &mut u32) -> i32 {
    let sp = fx.enter(0x790);
    fx.mem.set32(sp + 0x10C, n);
    let pool = fx.mem.read_u32(recs + 8);
    copy_q(fx, strips, sp + 0x650, 4, true);
    copy_q(fx, glow, sp + 0x6A0, 1, true);
    copy_q(fx, core, sp + 0x6C0, 1, true);
    let mut p = recs;
    fx.mem.set32(sp + 0xB0, 0);
    while fx.mem.i32(sp + 0xB0) < fx.mem.i32(sp + 0x10C) {
        if fx.mem.i32(sp + 0xB0) != 0 {
            p += 0x60;
        }
        one(fx, sp, p, pool, ot, a3);
        let v = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, v);
    }
    fx.leave(0x790);
    1
}

fn entry(pool: u32, idx: i32) -> u32 {
    pool.wrapping_add((idx as u32) << 4)
}

fn one(fx: &mut Fx, sp: u32, p: u32, pool: u32, ot: u32, a3: &mut u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    camera_vp_into(fx, sp + 0x110);
    fx.mem.write_u8(p + 5, 1);
    let v = fx.mem.read_u8(p + 0x32);
    fx.mem.write_u8(p + 6, v);
    node_source(fx, p, sp + 0x5A0);
    let v = fx.mem.read_u8(p + 0x33);
    fx.mem.write_u8(p + 6, v);
    node_source(fx, p, sp + 0x5B0);
    for t in 0..2u32 {
        let e = p + 0x12 * t + 0xC;
        let mut k = fx.mem.u8(p + 0x31);
        while k > 0 {
            let a = e + 2 * (k as u32 - 1);
            let prev = fx.mem.i16(a);
            if prev >= 0 {
                let c = fx.mem.read_u8(p);
                fx.mem.write_u8(entry(pool, prev), c);
            }
            let v = fx.mem.i16(a);
            fx.mem.set16(a + 2, v);
            k -= 1;
        }
        let last = e + 2 * fx.mem.u8(p + 0x31) as u32;
        let x = fx.mem.i16(last);
        if x >= 0 {
            fx.mem.write_u8(entry(pool, x), 0);
            let last = e + 2 * fx.mem.u8(p + 0x31) as u32;
            fx.mem.set16(last, -1);
        }
    }
    if fx.mem.read_u8(p) == 0 {
        return;
    }
    for (slot, node) in [(0xCu32, 0x5A0u32), (0x1E, 0x5B0)] {
        let cap = fx.mem.u8(p + 0x30);
        let v = first_free(fx, pool, 0x10, cap);
        if v >= 0 {
            fx.mem.set16(p + slot, v);
            let idx = fx.mem.i16(p + slot);
            let s4 = entry(pool, idx);
            let c = fx.mem.u8(p);
            set_counter(fx, s4, c, 0);
            copy_words(fx, sp + node, s4 + 4, 3);
        } else {
            fx.mem.set16(p + slot, -1);
        }
    }
    for t in 0..2u32 {
        let l = sp + 0x150 + 0x224 * t;
        fx.mem.write_u32(l, 0);
        let mut k = 0i32;
        while k < fx.mem.u8(p + 0x31) {
            let idx = fx.mem.i16(p + 0x12 * t + 0xC + 2 * k as u32);
            if idx >= 0 {
                let s4 = entry(pool, idx);
                for (src, dst) in [(4u32, 4u32), (8, 0x24), (0xC, 0x44)] {
                    let v = fx.mem.read_u32(s4 + src);
                    let c = fx.mem.read_u32(l);
                    fx.mem.write_u32(l.wrapping_add(c << 2) + dst, v);
                }
                let c = fx.mem.read_u32(l);
                fx.mem.write_u32(l, c.wrapping_add(1));
            }
            k += 1;
        }
    }
    let l1 = sp + 0x374;
    if fx.mem.read_u32(sp + 0x150) != fx.mem.read_u32(l1) {
        return;
    }
    if resample(fx, sp + 0x150) == 0 || resample(fx, l1) == 0 {
        return;
    }
    let s4 = (fx.mem.i32(sp + 0x150) - 1) << 2;
    let spec = sp + 0x650 + 0x18 * fx.mem.u8(p + 0x34) as u32;
    for (s, out) in [(spec, 0xC0u32), (sp + 0x6A0, 0xD0), (sp + 0x6C0, 0xE0)] {
        let (kind, id) = (fx.mem.i32(s), fx.mem.u8(s + 4));
        let t = fx.texture(kind, id);
        fx.mem.write_u32(sp + out, t);
    }
    fx.desc_init(sp + 0x700, 0xC, 0x34, 0x4214_2142_1421);
    let mut s3 = 0;
    while s3 < s4 - 1 {
        step(fx, sp, p, ot, s3, s4, a3);
        s3 += 1;
    }
}

fn step(fx: &mut Fx, sp: u32, p: u32, ot: u32, s3: i32, s4: i32, a3: &mut u32) {
    *a3 = sp + 0x5C8;
    let a = sp + 0x1B4 + 0x10 * s3 as u32;
    let b = sp + 0x3D8 + 0x10 * s3 as u32;
    for (o, dst) in [(0u32, 0x5C0u32), (4, 0x5C4), (8, 0x5C8), (0x10, 0x5D0), (0x14, 0x5D4), (0x18, 0x5D8)] {
        let bv = fx.mem.f(b + o);
        let d = ee::sub(fx.mem.f(a + o), bv);
        let v = ee::add(bv, ee::mul(fx.mem.f(p + 0x38), d));
        fx.mem.setf(sp + dst, v);
    }
    copy_words(fx, b, sp + 0x5E0, 4);
    copy_words(fx, b + 0x10, sp + 0x5F0, 4);
    for k in 0..3u32 {
        let av = fx.mem.f(sp + 0x5C0 + 4 * k);
        let d = ee::sub(fx.mem.f(sp + 0x5D0 + 4 * k), av);
        let v = ee::add(av, ee::mul(fx.mem.f(p + 0x3C), d));
        fx.mem.setf(sp + 0x600 + 4 * k, v);
    }
    if fx.mem.u8(p + 4) & 1 != 0 {
        strip(fx, sp, p, ot, s3, s4, a3);
    }
    let f = fx.mem.u8(p + 4);
    let mut lines = 0;
    if f & 2 != 0 {
        lines = 1;
    }
    if f & 4 != 0 {
        lines = 3;
    }
    fx.mem.set32(sp + 0xF0, lines);
    let mut s2 = 0;
    while s2 < fx.mem.i32(sp + 0xF0) {
        line(fx, sp, p, ot, s2, s3, s4, a3);
        s2 += 1;
    }
}

/// The vertex colours: `src` bytes of the record into R, G, B of the
/// four vertices (`dst` the R bytes), Q = 1 and the XYZF2 top bytes 0.
fn colours(fx: &mut Fx, sp: u32, p: u32, src: [(u32, &[u32]); 3]) {
    for (s, dsts) in src {
        let v = fx.mem.read_u8(p + s);
        for &d in dsts {
            fx.mem.write_u8(sp + d, v);
        }
    }
    for o in [0x75Cu32, 0x744, 0x72C, 0x714] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    for o in [0x76Fu32, 0x757, 0x73F, 0x727] {
        fx.mem.write_u8(sp + o, 0);
    }
}

/// alpha x count / 8 over the first 8 counts, divided by the samples.
fn fade(fx: &mut Fx, p: u32, alpha: u32, s4: i32) -> f32 {
    let c = fx.mem.u8(p);
    let mut f1 = ee::from_i32(fx.mem.u8(p + alpha));
    if c < 8 {
        f1 = ee::div(ee::mul(ee::from_i32(c), f1), 8.0);
    }
    ee::div(f1, ee::from_i32(s4))
}

fn strip(fx: &mut Fx, sp: u32, p: u32, ot: u32, s3: i32, s4: i32, a3: &mut u32) {
    let m = fx.read_m4(sp + 0x110);
    for (v, xyz, w) in [(0x5C0u32, 0x720u32, 0x640u32), (0x5D0, 0x738, 0x644), (0x5E0, 0x750, 0x648), (0x5F0, 0x768, 0x64C)] {
        *a3 = sp + w;
        if !fx.project_at(sp + v, &m, sp + xyz, sp + w) || !Fx::on_screen(fx.mem.read_u64(sp + xyz)) {
            return;
        }
    }
    colours(fx, sp, p, [(0x46, &[0x728, 0x710]), (0x4A, &[0x758, 0x740]), (0x45, &[0x729, 0x711])]);
    for (s, dsts) in [(0x49u32, [0x759u32, 0x741]), (0x44, [0x72A, 0x712]), (0x48, [0x75A, 0x742])] {
        let v = fx.mem.read_u8(p + s);
        for d in dsts {
            fx.mem.write_u8(sp + d, v);
        }
    }
    let kind = fx.mem.u8(p + 0x34);
    let spec = sp + 0x650 + 0x18 * kind as u32;
    copy_words(fx, spec, sp + 0x6E0, 6);
    if fx.mem.u8(p + 0x34) != 0 {
        let v1 = fx.mem.i16(p + 0x36);
        if v1 != 0 {
            fx.mem.set32(sp + 0x6F4, v1);
        }
    }
    let tex = fx.mem.read_u32(sp + 0xC0);
    *a3 = tex_rect_strip_a3(fx, sp + 0x700, sp + 0x6E0, tex);
    let tweak = match fx.mem.u8(p + 0x34) {
        2 => Some((3u64, 0x210u16)),
        1 => Some((7, 0x200)),
        _ => None,
    };
    if let Some((shift, h)) = tweak {
        let a = sp + 0x788;
        let b = fx.mem.read_u8(a);
        fx.mem.write_u8(a, (b & 0xF3) | 0xC);
        let v = fx.mem.read_u64(a);
        fx.mem.write_u64(a, (v & 0xFFFF_FFFC_00FF_FFFF) | (shift << 24));
        *a3 = (v & 0xFFFF_FFFF) as u32;
        let x = fx.mem.read_u16(sp + 0x78C);
        fx.mem.write_u16(sp + 0x78C, (x & 0xF003) | h);
    }
    let f22 = fade(fx, p, 0x47, s4);
    fx.mem.write_u8(sp + 0x75B, 0);
    fx.mem.write_u8(sp + 0x743, 0);
    let s2 = s4 - s3;
    let v = ee::fixunssfsi(ee::mul(f22, ee::from_i32(s2)));
    fx.mem.write_u8(sp + 0x713, v as u8);
    let v = ee::fixunssfsi(ee::mul(f22, ee::from_i32(s2 - 1)));
    fx.mem.write_u8(sp + 0x72B, v as u8);
    fx.submit(sp + 0x700, ot, 0);
}

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v);
    fx.mem.write_u16(a, x as u16);
}

fn line(fx: &mut Fx, sp: u32, p: u32, ot: u32, s2: i32, s3: i32, s4: i32, a3: &mut u32) {
    copy_q(fx, sp + 0x5C0, sp + 0x610, 1, false);
    if fx.mem.u8(p + 4) & 8 != 0 {
        copy_words(fx, sp + 0x600, sp + 0x620, 4);
    } else {
        copy_words(fx, sp + 0x5D0, sp + 0x620, 4);
    }
    if s2 == 2 || s2 == 1 {
        for (o, s) in [(0x610u32, 0x50u32), (0x614, 0x54), (0x620, 0x50), (0x624, 0x54)] {
            let (x, d) = (fx.mem.f(sp + o), fx.mem.f(p + s));
            let v = if s2 == 2 { ee::sub(x, d) } else { ee::add(x, d) };
            fx.mem.setf(sp + o, v);
        }
    }
    let m = fx.read_m4(sp + 0x110);
    for (v, xyz, w) in [(0x610u32, 0x630u32, 0x640u32), (0x620, 0x638, 0x644)] {
        *a3 = sp + w;
        if !fx.project_at(sp + v, &m, sp + xyz, sp + w) || !Fx::on_screen(fx.mem.read_u64(sp + xyz)) {
            return;
        }
    }
    let f1 = ee::div(fx.mem.f(sp + 0x640), 256.0);
    let mut f21 = ee::div(fx.mem.f(p + 0x40), f1);
    let f20 = f21;
    if fx.mem.u8(p + 4) & 8 != 0 {
        f21 = 0.0;
        let tex = fx.mem.read_u32(sp + 0xE0);
        tex_rect_strip(fx, sp + 0x700, sp + 0x6C0, tex);
    } else {
        let tex = fx.mem.read_u32(sp + 0xD0);
        tex_rect_strip(fx, sp + 0x700, sp + 0x6A0, tex);
    }
    colours(fx, sp, p, [(0x4E, &[0x758, 0x728, 0x740, 0x710]), (0x4D, &[0x759, 0x729, 0x741, 0x711]), (0x4C, &[0x75A, 0x72A, 0x742, 0x712])]);
    let f22 = fade(fx, p, 0x4F, s4);
    let s6 = s4 - s3;
    let v = ee::fixunssfsi(ee::mul(f22, ee::from_i32(s6))) as u8;
    fx.mem.write_u8(sp + 0x72B, v);
    fx.mem.write_u8(sp + 0x713, v);
    let v = ee::fixunssfsi(ee::mul(f22, ee::from_i32(s6 - 1))) as u8;
    fx.mem.write_u8(sp + 0x75B, v);
    fx.mem.write_u8(sp + 0x743, v);
    if s3 == 0 {
        fx.mem.write_u8(sp + 0x72B, 0);
        fx.mem.write_u8(sp + 0x713, 0);
    }
    for (from, to) in [(0x630u32, 0x738u32), (0x738, 0x720), (0x638, 0x768), (0x768, 0x750)] {
        let z = fx.xyz_z(sp + from);
        fx.set_xyz_z(sp + to, z);
        *a3 = z as u32;
    }
    let f = |v: i32| ee::from_i32(v);
    // Across y.
    let y0 = f(fx.mem.u16(sp + 0x632));
    fix16(fx, sp + 0x722, ee::add(y0, f20));
    fix16(fx, sp + 0x73A, ee::sub(y0, f20));
    let y1 = f(fx.mem.u16(sp + 0x63A));
    fix16(fx, sp + 0x752, ee::add(y1, f21));
    fix16(fx, sp + 0x76A, ee::sub(y1, f21));
    let x0 = fx.mem.read_u16(sp + 0x630);
    fx.mem.write_u16(sp + 0x738, x0);
    fx.mem.write_u16(sp + 0x720, x0);
    let x1 = fx.mem.read_u16(sp + 0x638);
    fx.mem.write_u16(sp + 0x768, x1);
    fx.mem.write_u16(sp + 0x750, x1);
    fx.submit(sp + 0x700, ot, 0);
    // Across x.
    let x0 = f(fx.mem.u16(sp + 0x630));
    fix16(fx, sp + 0x720, ee::sub(x0, f20));
    fix16(fx, sp + 0x738, ee::add(x0, f20));
    let x1 = f(fx.mem.u16(sp + 0x638));
    fix16(fx, sp + 0x750, ee::sub(x1, f21));
    fix16(fx, sp + 0x768, ee::add(x1, f21));
    let y0 = fx.mem.read_u16(sp + 0x632);
    fx.mem.write_u16(sp + 0x73A, y0);
    fx.mem.write_u16(sp + 0x722, y0);
    let y1 = fx.mem.read_u16(sp + 0x63A);
    fx.mem.write_u16(sp + 0x76A, y1);
    fx.mem.write_u16(sp + 0x752, y1);
    fx.submit(sp + 0x700, ot, 0);
}
