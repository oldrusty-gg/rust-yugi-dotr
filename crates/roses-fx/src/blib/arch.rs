//! Arches of the effect library: a gouraud half
//! fan of three arcs of one width and different heights.
use super::exe::{camera_vp_into, countdown, libm_cosf, mul_mem, node_source, rot_trans_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx};

const PI: u32 = 0x4049_0FDB;

/// fb4e3b6ecd2c [7 B records; b468 0x1E05970, 572 words] [F]:
/// `arches(at, recs, ot, n) -> 1` steps and draws `n` records (0x54 bytes;
/// a 0x5C0 frame, the descriptor at sp + 0x568: RGBAQ / XYZ2 x 4, ALPHA
/// 0x48 by 0x207E20). Per record: countdown 0x101AF0; position +0x24 +=
/// velocity +0x34, size +0x10 += +0x18, angle +0x1C += +0x20 (i32); with
/// k = min(+7, 9) the 2k + 1 points at 90 / k degree steps (libm sinf /
/// cosf, sqrtf) of three arcs in the plane z = 0: (b cos, b sin), (b cos,
/// h sin), (b cos, (b + 0.8 (h - b)) sin), where b / h are the size and the
/// size x ratio +0x14 (the smaller one b); turned by the angle about y
/// (0x20C0B0, 0x20C540) when it is not 0. Colour +0xA, +9, +8 / +0xE, +0xD,
/// +0xC on the two vertex pairs, alphas +0xB / +0xF faded over the last 8
/// counts unless flag 0x40 and, with flag 1, over the first 8; the
/// position from a node with +5 (0x101BA0); through the camera x (angles
/// +0x44, position +0x24). For each step two quads: arcs 1-2 and 0-2 (the
/// second keeps the first's arc-2 vertices), skipped when a vertex fails
/// to project; flag 0x80 at the far Z (0xFFFFFE) with key 0, else keyed by
/// the mean w (0x1A2C70).
pub fn arches(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    arches_by(fx, [at + 0x6C0, at + 0x840], recs, ot, n)
}

/// 4f89495ebb3d [5 B records; b351 0x1E014C0, 536 words] [F]: [`arches`]
/// compiled without scheduling in a 0x5F0 frame whose locals sit at
/// `arches`' addresses (its sp + 0x30 = this one's sp); its two soft-float
/// key calls return to +0x660 / +0x7C8.
pub fn arches_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    arches_by(fx, [at + 0x660, at + 0x7C8], recs, ot, n)
}

/// bbe4d9b03097 [1 B record; card 139's 0x1E051E0, 640 words] (helper
/// A18) [F]: [`arches`] compiled in a 0x570 frame (the shared locals 0x50
/// lower in it: the same caller-relative addresses), n in s4 and the
/// record counter in s1 (no slots), the per-point angle and 1 - x^2 / b^2
/// recomputed (the same operations in the same order, so the same
/// values); its two key calls return to +0x7C0 / +0x978.
pub fn arches_c(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    arches_in(fx, [at + 0x7C0, at + 0x978], 0x570, 0u32.wrapping_sub(0x50), recs, ot, n)
}

/// `ras`: the return addresses of the two quads' 0x1A2C70 calls.
fn arches_by(fx: &mut Fx, ras: [u32; 2], recs: u32, ot: u32, n: i32) -> i32 {
    arches_in(fx, ras, 0x5C0, 0, recs, ot, n)
}

/// A compilation with a `frame`-byte frame whose sp + `shift` (wrapping)
/// holds the locals at [`arches`]' offsets.
fn arches_in(fx: &mut Fx, ras: [u32; 2], frame: u32, shift: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(frame).wrapping_add(shift);
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x54;
        }
        one(fx, ras, sp, g, ot);
        i += 1;
    }
    fx.leave(frame);
    1
}

/// v (a vec4 at `v`) through the matrix at `m` (0x20C540 into sp + 0x530),
/// copied back.
fn turn(fx: &mut Fx, sp: u32, m: u32, v: u32) {
    let mm = fx.read_m4(m);
    let x = fx.read_v4(v);
    let r = fx.transform(Some(&mm), x);
    fx.write_v4(sp + 0x530, r);
    fx.write_v4(v, r);
}

fn one(fx: &mut Fx, ras: [u32; 2], sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    for o in [0x24u32, 0x28, 0x2C] {
        let v = ee::add(fx.mem.f(g + o), fx.mem.f(g + o + 0x10));
        fx.mem.setf(g + o, v);
    }
    let v = ee::add(fx.mem.f(g + 0x10), fx.mem.f(g + 0x18));
    fx.mem.setf(g + 0x10, v);
    let a = fx.mem.i32(g + 0x1C).wrapping_add(fx.mem.i32(g + 0x20));
    fx.mem.set32(g + 0x1C, a);
    let k = fx.mem.u8(g + 7).min(9);
    let step = ee::div(90.0, ee::from_i32(k));
    let ratio = fx.mem.f(g + 0x14);
    let (h, b) = if ee_lt(ratio, 1.0) {
        let h = fx.mem.f(g + 0x10);
        (h, ee::mul(h, ratio))
    } else {
        let b = fx.mem.f(g + 0x10);
        (ee::mul(b, ratio), b)
    };
    let mid = ee::add(b, ee::mul(f32::from_bits(0x3F4C_CCCD), ee::sub(h, b)));
    if fx.mem.i32(g + 0x1C) != 0 {
        fx.mem.write_u32(sp + 0x548, 0);
        fx.mem.write_u32(sp + 0x540, 0);
        let a = fx.mem.i32(g + 0x1C);
        fx.mem.set32(sp + 0x544, a);
        let m = fx.trs(None, Some([0, a, 0]), None);
        fx.write_m4(sp + 0x160, &m);
    }
    let mut j = 0;
    while j < 2 * k + 1 {
        let ang = ee::div(ee::mul(f32::from_bits(PI), ee::mul(step, ee::from_i32(j))), 180.0);
        let s = fx.libm_sinf(ang);
        let c = libm_cosf(fx, ang);
        let x = ee::mul(b, c);
        let p0 = sp + 0x1A0 + 16 * j as u32;
        fx.mem.setf(p0, x);
        fx.mem.setf(p0 + 4, ee::mul(b, s));
        fx.mem.write_u32(p0 + 8, 0);
        let p1 = p0 + 0x130;
        fx.mem.setf(p1, x);
        let q = ee::sub(1.0, ee::div(ee::mul(x, x), ee::mul(b, b)));
        let y = fx.sqrtf(ee::mul(ee::mul(h, h), q));
        fx.mem.setf(p1 + 4, y);
        fx.mem.write_u32(p1 + 8, 0);
        let p2 = p0 + 0x260;
        fx.mem.setf(p2, x);
        let y = fx.sqrtf(ee::mul(ee::mul(mid, mid), q));
        fx.mem.setf(p2 + 4, y);
        fx.mem.write_u32(p2 + 8, 0);
        if fx.mem.i32(g + 0x1C) != 0 {
            for p in [p0, p1, p2] {
                turn(fx, sp, sp + 0x160, p);
            }
        }
        j += 1;
    }
    let d = sp + 0x568;
    fx.desc_init(d, 8, 0x14, 0x4141_4141);
    fx.desc_alpha(d, 0x48);
    let (outer, inner) = (fx.mem.read_u8(g + 0xF), fx.mem.read_u8(g + 0xB));
    for (o, v) in [(0x5ABu32, outer), (0x59B, outer), (0x58B, inner), (0x57B, inner)] {
        fx.mem.write_u8(sp + o, v);
    }
    let c = fx.mem.u8(g);
    if fx.mem.u8(g + 4) & 0x40 == 0 && c < 8 {
        let a = fx.mem.u8(g + 0xF) >> 3;
        let v = fx.mult(a, c);
        fx.mem.set8(sp + 0x5AB, v);
        fx.mem.set8(sp + 0x59B, v);
        let v = (fx.mem.u8(g + 0xB) >> 3).wrapping_mul(fx.mem.u8(g));
        fx.mem.set8(sp + 0x58B, v);
        fx.mem.set8(sp + 0x57B, v);
    }
    if fx.mem.u8(g + 4) & 1 != 0 {
        let e = fx.mem.u8(g + 2) - fx.mem.u8(g);
        if e < 8 {
            let a = fx.mem.u8(g + 0xF) >> 3;
            let v = fx.mult(a, e + 1);
            fx.mem.set8(sp + 0x5AB, v);
            fx.mem.set8(sp + 0x59B, v);
            let e = fx.mem.u8(g + 2) - fx.mem.u8(g) + 1;
            let v = (fx.mem.u8(g + 0xB) >> 3).wrapping_mul(e);
            fx.mem.set8(sp + 0x58B, v);
            fx.mem.set8(sp + 0x57B, v);
        }
    }
    for (src, k) in [(0xAu32, 0u32), (9, 1), (8, 2)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + 0x588 + k, v);
        fx.mem.write_u8(sp + 0x578 + k, v);
        let v = fx.mem.read_u8(g + src + 4);
        fx.mem.write_u8(sp + 0x5A8 + k, v);
        fx.mem.write_u8(sp + 0x598 + k, v);
    }
    node_source(fx, g, g + 0x24);
    camera_vp_into(fx, sp + 0x120);
    rot_trans_into(fx, sp + 0xE0, g + 0x44, g + 0x24);
    mul_mem(fx, sp + 0x120, sp + 0xE0);
    let mut j = 0;
    while j < 2 * k {
        let p0 = sp + 0x1A0 + 16 * j as u32;
        let p1 = p0 + 0x10;
        let first = [(p0 + 0x130, 0x580u32, 0x550u32), (p1 + 0x130, 0x590, 0x554), (p0 + 0x260, 0x5A0, 0x558), (p1 + 0x260, 0x5B0, 0x55C)];
        if quad(fx, ras[0], sp, g, ot, &first) {
            quad(fx, ras[1], sp, g, ot, &[(p0, 0x580, 0x550), (p1, 0x590, 0x554)]);
        }
        j += 1;
    }
}

/// Project the vertices (vec, xyz, w), then the far-Z / mean-w key and the
/// submit; false when a vertex fails (the step ends).
fn quad(fx: &mut Fx, ra: u32, sp: u32, g: u32, ot: u32, verts: &[(u32, u32, u32)]) -> bool {
    for &(v, xyz, w) in verts {
        let m = fx.read_m4(sp + 0x120);
        if !fx.project_at(v, &m, sp + xyz, sp + w) {
            return false;
        }
    }
    for o in [0x5B7u32, 0x5A7, 0x597, 0x587] {
        fx.mem.write_u8(sp + o, 0);
    }
    let depth = if fx.mem.u8(g + 4) & 0x80 != 0 {
        let mut z = 0xFF_FFFE;
        for o in [0x5B0u32, 0x5A0, 0x590, 0x580] {
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
        }
        0.0
    } else {
        let (w0, w1, w2, w3) = (fx.mem.f(sp + 0x550), fx.mem.f(sp + 0x554), fx.mem.f(sp + 0x558), fx.mem.f(sp + 0x55C));
        ee::div(ee::add(w3, ee::add(w2, ee::add(w0, w1))), 4.0)
    };
    let key = ee::fixunssfsi(depth) as i32;
    fx.fix_residue(depth, ra);
    fx.submit(sp + 0x568, ot, key);
    true
}
