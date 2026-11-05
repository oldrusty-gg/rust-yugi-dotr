//! Panels of the effect library: one textured
//! quad per 0x3C-byte record in the record's own frame (angles +0x1C,
//! origin +0x2C), narrowing / growing over its count.
use super::exe::{alpha_mode, atlas_cell, camera_vp_into, countdown, mul_mem, node_source, rot_trans_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

/// 22209fd991ad [6 B records; b029 0x1E04830, 324 words] [F]:
/// `panels(recs, ot, n) -> 1` steps and draws `n` records (0x3C bytes; a
/// 0x230 frame, the descriptor at sp + 0x198 (RGBAQ / ST / XYZ2 x 4), an
/// atlas spec at sp + 0x180). Per record: countdown 0x101AF0; texture (5,
/// +8); with e = p[2] - p[0] the half width a = +0x10 - e (+0x10 / p[2])
/// and half height b = +0x14 + e +0x18; the corners (-a, -b), (a, -b),
/// (-a, b), (a, b) at z 0; the origin +0x2C from a node (0x101BA0); the
/// matrix camera x (angles +0x1C, origin); colour +0xE / +0xD / +0xC,
/// alpha +0xF (x count / 8 over the first 8 counts unless flag 0x40); all
/// four corners projected (else nothing); atlas cell +0xA of layout +9,
/// blend +0xB; STQ 0..1 by 1 / w, REGION_CLAMP the cell; ALPHA, TEX0 /
/// TEX1 (filter 1); keyed by the mean w (flag 0x80: depth 0xFFFFFE, key
/// 0). `at`: its address (the soft-float call's return is at + 0x4B0).
pub fn panels(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x230);
    fx.mem.set32(sp + 0xA4, n);
    fx.mem.write_u32(sp + 0xA8, ot);
    let mut p = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            p += 0x3C;
        }
        one(fx, at, sp, p, ot);
        i += 1;
    }
    fx.leave(0x230);
    1
}

fn one(fx: &mut Fx, at: u32, sp: u32, p: u32, ot: u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    let id = fx.mem.u8(p + 8);
    let tex = fx.texture(5, id);
    let (total, c) = (fx.mem.u8(p + 2), fx.mem.u8(p));
    let w0 = fx.mem.f(p + 0x10);
    let per = ee::div(w0, ee::from_i32(total));
    let e = ee::from_i32(total - c);
    let a = ee::sub(w0, ee::mul(e, per));
    let b = ee::add(fx.mem.f(p + 0x14), ee::mul(fx.mem.f(p + 0x18), e));
    for o in [0x168u32, 0x158, 0x148, 0x138] {
        fx.mem.write_u32(sp + o, 0);
    }
    for (o, v) in [(0x150u32, -a), (0x130, -a), (0x160, a), (0x140, a), (0x144, -b), (0x134, -b), (0x164, b), (0x154, b)] {
        fx.mem.setf(sp + o, v);
    }
    node_source(fx, p, p + 0x2C);
    camera_vp_into(fx, sp + 0xB0);
    rot_trans_into(fx, sp + 0xF0, p + 0x1C, p + 0x2C);
    mul_mem(fx, sp + 0xB0, sp + 0xF0);
    let d = sp + 0x198;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    for (s, k) in [(0xEu32, 0u32), (0xD, 1), (0xC, 2)] {
        let v = fx.mem.read_u8(p + s);
        for o in [0x1F0u32, 0x1D8, 0x1C0, 0x1A8] {
            fx.mem.write_u8(sp + o + k, v);
        }
    }
    let c = fx.mem.u8(p);
    let alpha = if fx.mem.u8(p + 4) & 0x40 == 0 && c < 8 {
        let k = fx.mem.u8(p + 0xF) >> 3;
        fx.mult(k, c) as u8
    } else {
        fx.mem.read_u8(p + 0xF)
    };
    for o in [0x1F3u32, 0x1DB, 0x1C3, 0x1AB] {
        fx.mem.write_u8(sp + o, alpha);
    }
    let m = fx.read_m4(sp + 0xB0);
    for (v, xyz, w) in [(0x130u32, 0x1B8u32, 0x170u32), (0x140, 0x1D0, 0x174), (0x150, 0x1E8, 0x178), (0x160, 0x200, 0x17C)] {
        if !fx.project_at(sp + v, &m, sp + xyz, sp + w) {
            return;
        }
    }
    let b = fx.mem.read_u8(p + 0xB);
    fx.mem.write_u8(sp + 0x185, b);
    let (cell, layout) = (fx.mem.u8(p + 0xA), fx.mem.u8(p + 9));
    // [H] a layout outside 0..9 leaves the spec as it was (the original:
    // stale registers).
    atlas_cell(fx, sp + 0x180, cell, layout);
    let w: [f32; 4] = std::array::from_fn(|k| fx.mem.f(sp + 0x170 + 4 * k as u32));
    let q: [f32; 4] = std::array::from_fn(|k| ee::div(1.0, w[k]));
    let (u0, u1, v0, v1) = (fx.mem.i32(sp + 0x188), fx.mem.i32(sp + 0x18C), fx.mem.i32(sp + 0x190), fx.mem.i32(sp + 0x194));
    let clamp = ((i64::from(u0) << 4) as u64 | 0xA) | ((i64::from(u1.wrapping_sub(1)) << 14) as u64) | ((i64::from(v0) << 24) as u64) | ((i64::from(v1.wrapping_sub(1)) << 34) as u64);
    fx.mem.setf(sp + 0x1AC, q[0]);
    fx.mem.setf(sp + 0x1C4, q[1]);
    fx.mem.setf(sp + 0x1DC, q[2]);
    fx.mem.setf(sp + 0x1F4, q[3]);
    for o in [0x1B0u32, 0x1B4, 0x1CC, 0x1E0] {
        fx.mem.write_u32(sp + o, 0);
    }
    fx.mem.setf(sp + 0x1C8, q[1]);
    fx.mem.setf(sp + 0x1E4, q[2]);
    fx.mem.setf(sp + 0x1F8, q[3]);
    fx.mem.setf(sp + 0x1FC, q[3]);
    fx.desc_clamp(d, clamp);
    alpha_mode(fx, sp + 0x180, sp + 0x22C);
    let s = |fx: &mut Fx, k: u32| u64::from(fx.mem.read_u8(sp + 0x22C + k));
    let al = s(fx, 0) | (s(fx, 1) << 2) | (s(fx, 2) << 4) | (s(fx, 3) << 6);
    fx.desc_alpha(d, al);
    fx.desc_texture(d, tex, 1);
    let depth = if fx.mem.u8(p + 4) & 0x80 != 0 {
        let mut z = 0xFF_FFFE;
        for o in [0x200u32, 0x1E8, 0x1D0, 0x1B8] {
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
        }
        0.0
    } else {
        ee::div(ee::add(w[3], ee::add(w[2], ee::add(w[0], w[1]))), 4.0)
    };
    fx.fix_residue(depth, at + 0x4B0);
    let key = ee::fixunssfsi(depth) as i32;
    fx.submit(d, ot, key);
}
