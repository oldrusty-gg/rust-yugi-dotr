//! Flat quads and lines of the effect library:
//! card 599's two locals, a growing textured quad lying in its record's
//! xz-plane and gouraud lines between two points.
use super::band::cell_size;
use super::exe::{alpha_mode, atlas_cell, camera_vp_into, countdown, frame_mask, mul_mem, node_source, rot_trans_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

/// 50ef9bca8fdd [1 B record; card 599's 0x1E055B0, 384 words] [F]:
/// `quads(at, recs, ot, n) -> 1` steps and draws `n` records (0x48 bytes;
/// a 0x250 frame, n / ot at sp + 0xA4 / 0xA8, the descriptor at sp + 0x1B0
/// (RGBAQ / ST / XYZ2 x 4), the atlas spec at sp + 0x190). Per record:
/// countdown 0x101AF0; texture (5, +8); position +0x30.. += +0x3C..; half
/// sizes +0x10 / +0x14 += +0x18 / +0x1C; with a unit +5 the position from
/// its node (0x101BA0); the corners (-a, 0, -b), (a, 0, -b), (-a, 0, b),
/// (a, 0, b); the matrix camera x (angles +0x20, the position) (0x20E2B0,
/// 0x20C0B0, 0x20B370); colour +0xE / +0xD / +0xC; atlas cell +9 (flag 1:
/// 0x1022D0(+0xA, elapsed)) of layout +0xA; alpha +0xF (x count / 8 on the
/// last 8 unless flag 0x40); all four corners projected (else nothing);
/// the layout's cell size (0x102210), REGION_CLAMP the cell, STQ over the
/// cell by 1 / w; blend +0xB (0x101C80), TEX0 / TEX1 (filter 1); keyed by
/// the mean w (flag 0x80: Z 0xFFFFFE, key 0). `at`: its address (the
/// conversion's return at + 0x5A0).
pub fn quads(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    quads_a3(fx, at, recs, ot, n).0
}

/// [`quads`] and the a3 it leaves [F card 599 0x1E055B0..0x1E05B5C]: per
/// counting record the projections' w pointers (sp + 0x180..0x18C) as far
/// as they get, then 0x207E50's (the TEX0 low word, or desc + 0x10 for a
/// missing texture), with flag 0x80 the fourth vertex's old XYZ low word
/// (`ld a3`). `None` when no record reached a projection.
pub fn quads_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let mut a3 = None;
    let sp = fx.enter(0x250);
    fx.mem.set32(sp + 0xA4, n);
    fx.mem.write_u32(sp + 0xA8, ot);
    let mut p = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            p += 0x48;
        }
        quad(fx, at, sp, p, &mut a3);
        i += 1;
    }
    fx.leave(0x250);
    (1, a3)
}

fn quad(fx: &mut Fx, at: u32, sp: u32, p: u32, a3: &mut Option<u32>) {
    if countdown(fx, p) == 0 {
        return;
    }
    let id = fx.mem.u8(p + 8);
    let tex = fx.texture(5, id);
    for o in [0x30u32, 0x34, 0x38] {
        let v = ee::add(fx.mem.f(p + o), fx.mem.f(p + o + 0xC));
        fx.mem.setf(p + o, v);
    }
    for o in [0x10u32, 0x14] {
        let v = ee::add(fx.mem.f(p + o), fx.mem.f(p + o + 8));
        fx.mem.setf(p + o, v);
    }
    if fx.mem.read_u8(p + 5) != 0 {
        node_source(fx, p, sp + 0x170);
        for k in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x170 + 4 * k);
            fx.mem.write_u32(p + 0x30 + 4 * k, w);
        }
    }
    let a = fx.mem.f(p + 0x10);
    fx.mem.setf(sp + 0x150, ee::neg(a));
    fx.mem.setf(sp + 0x130, ee::neg(a));
    let a = fx.mem.f(p + 0x10);
    fx.mem.setf(sp + 0x160, a);
    fx.mem.setf(sp + 0x140, a);
    let b = fx.mem.f(p + 0x14);
    fx.mem.setf(sp + 0x148, ee::neg(b));
    fx.mem.setf(sp + 0x138, ee::neg(b));
    let b = fx.mem.f(p + 0x14);
    fx.mem.setf(sp + 0x168, b);
    fx.mem.setf(sp + 0x158, b);
    for o in [0x164u32, 0x154, 0x144, 0x134] {
        fx.mem.write_u32(sp + o, 0);
    }
    camera_vp_into(fx, sp + 0xB0);
    for k in 0..3u32 {
        let w = fx.mem.read_u32(p + 0x30 + 4 * k);
        fx.mem.write_u32(sp + 0x170 + 4 * k, w);
    }
    rot_trans_into(fx, sp + 0xF0, p + 0x20, sp + 0x170);
    mul_mem(fx, sp + 0xB0, sp + 0xF0);
    let d = sp + 0x1B0;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    for (s, k) in [(0xEu32, 0u32), (0xD, 1), (0xC, 2)] {
        let v = fx.mem.read_u8(p + s);
        for o in [0x208u32, 0x1F0, 0x1D8, 0x1C0] {
            fx.mem.write_u8(sp + o + k, v);
        }
    }
    let mut cell = fx.mem.u8(p + 9);
    if fx.mem.u8(p + 4) & 1 != 0 {
        let elapsed = fx.mem.u8(p + 2) - fx.mem.u8(p);
        let kind = fx.mem.u8(p + 0xA);
        cell = frame_mask(fx, kind, elapsed) & 0xFF;
    }
    let layout = fx.mem.u8(p + 0xA);
    atlas_cell(fx, sp + 0x190, cell, layout);
    let al = fx.mem.read_u8(p + 0xF);
    for o in [0x20Bu32, 0x1F3, 0x1DB, 0x1C3] {
        fx.mem.write_u8(sp + o, al);
    }
    let c = fx.mem.u8(p);
    if fx.mem.u8(p + 4) & 0x40 == 0 && c < 8 {
        let k = fx.mem.u8(p + 0xF) >> 3;
        let v = fx.mult(k, c);
        for o in [0x20Bu32, 0x1F3, 0x1DB, 0x1C3] {
            fx.mem.set8(sp + o, v);
        }
    }
    let m = fx.read_m4(sp + 0xB0);
    for (v, xyz, w) in [(0x130u32, 0x1D0u32, 0x180u32), (0x140, 0x1E8, 0x184), (0x150, 0x200, 0x188), (0x160, 0x218, 0x18C)] {
        *a3 = Some(sp + w);
        if !fx.project_at(sp + v, &m, sp + xyz, sp + w) {
            return;
        }
    }
    let layout = fx.mem.u8(p + 0xA);
    cell_size(fx, layout, sp + 0x244, sp + 0x248);
    let (u0, u1, v0, v1) = (fx.mem.i32(sp + 0x198), fx.mem.i32(sp + 0x19C), fx.mem.i32(sp + 0x1A0), fx.mem.i32(sp + 0x1A4));
    let clamp = ((i64::from(u0) << 4) as u64 | 0xA) | ((i64::from(u1.wrapping_sub(1)) << 14) as u64) | ((i64::from(v0) << 24) as u64) | ((i64::from(v1.wrapping_sub(1)) << 34) as u64);
    fx.desc_clamp(d, clamp);
    let w: [f32; 4] = std::array::from_fn(|k| fx.mem.f(sp + 0x180 + 4 * k as u32));
    for (o, k) in [(0x1C4u32, 0usize), (0x1DC, 1), (0x1F4, 2), (0x20C, 3)] {
        fx.mem.setf(sp + o, ee::div(1.0, w[k]));
    }
    let (cw, ch) = (fx.mem.f(sp + 0x244), fx.mem.f(sp + 0x248));
    let s0 = ee::div(ee::from_i32(fx.mem.i32(sp + 0x198)), cw);
    let t0 = ee::div(ee::from_i32(fx.mem.i32(sp + 0x1A0)), ch);
    let s1 = ee::div(ee::from_i32(fx.mem.i32(sp + 0x19C)), cw);
    let t1 = ee::div(ee::from_i32(fx.mem.i32(sp + 0x1A4)), ch);
    for (o, s, k) in [(0x1C8u32, s0, 0usize), (0x1CC, t0, 0), (0x1E0, s1, 1), (0x1E4, t0, 1), (0x1F8, s0, 2), (0x1FC, t1, 2), (0x210, s1, 3), (0x214, t1, 3)] {
        fx.mem.setf(sp + o, ee::div(s, w[k]));
    }
    let b = fx.mem.read_u8(p + 0xB);
    fx.mem.write_u8(sp + 0x195, b);
    alpha_mode(fx, sp + 0x190, sp + 0x24C);
    let s = |fx: &mut Fx, k: u32| u64::from(fx.mem.read_u8(sp + 0x24C + k));
    let al = s(fx, 0) | (s(fx, 1) << 2) | (s(fx, 2) << 4) | (s(fx, 3) << 6);
    fx.desc_alpha(d, al);
    *a3 = Some(if fx.mem.read_u16(d + 2) & 0x20 != 0 && tex != 0 { fx.mem.read_u32(tex + 0x30) } else { d + 0x10 });
    fx.desc_texture(d, tex, 1);
    let depth = if fx.mem.u8(p + 4) & 0x80 != 0 {
        *a3 = Some(fx.mem.read_u32(sp + 0x218));
        let mut z = 0xFF_FFFE;
        for o in [0x218u32, 0x200, 0x1E8, 0x1D0] {
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
        }
        0.0
    } else {
        ee::div(ee::add(w[3], ee::add(w[2], ee::add(w[0], w[1]))), 4.0)
    };
    fx.fix_residue(depth, at + 0x5A0);
    let key = ee::fixunssfsi(depth) as i32;
    let ot = fx.mem.read_u32(sp + 0xA8);
    fx.submit(d, ot, key);
}

/// caceac36d1c1 [1 B record; card 599's 0x1E05BB0, 168 words] [F]:
/// `lines(at, recs, ot, n) -> 1` steps and draws `n` records (0x28 bytes;
/// a 0x140 frame, the descriptor at sp + 0x100: prim 0x11, two RGBAQ /
/// XYZ2 vertices). Per record: countdown 0x101AF0; the ends +0x10.. /
/// +0x1C.. (sp + 0xE0 / 0xF0); the camera (0x20E2B0, sp + 0xA0); colours
/// +0xA / +9 / +8 and +0xE / +0xD / +0xC, alphas +0xB / +0xF (x count / 8
/// on the last 8 unless flag 0x40); both ends projected (else nothing);
/// ALPHA 0x48 and flag 0x400; keyed by the mean w (flag 0x80: Z 0xFFFFFE,
/// key 0). `at`: its address (the conversion's return at + 0x248).
pub fn lines(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    lines_a3(fx, at, recs, ot, n).0
}

/// [`lines`] and the a3 it leaves [F card 599 0x1E05BB0..0x1E05E00]: per
/// counting record the projections' w pointers (sp + 0x138 / 0x13C) as
/// far as they get, then 0x207E20's (the descriptor flags before it, 0x11),
/// with flag 0x80 0 (the far Z shifted up). `None` when no record counted.
pub fn lines_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let mut a3 = None;
    let sp = fx.enter(0x140);
    let mut p = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            p += 0x28;
        }
        line(fx, at, sp, p, ot, &mut a3);
        i += 1;
    }
    fx.leave(0x140);
    (1, a3)
}

fn line(fx: &mut Fx, at: u32, sp: u32, p: u32, ot: u32, a3: &mut Option<u32>) {
    if countdown(fx, p) == 0 {
        return;
    }
    for k in 0..3u32 {
        let w = fx.mem.read_u32(p + 0x10 + 4 * k);
        fx.mem.write_u32(sp + 0xE0 + 4 * k, w);
        let w = fx.mem.read_u32(p + 0x1C + 4 * k);
        fx.mem.write_u32(sp + 0xF0 + 4 * k, w);
    }
    camera_vp_into(fx, sp + 0xA0);
    let d = sp + 0x100;
    fx.desc_init(d, 4, 0x11, 0x4141);
    for (s, o) in [(0xAu32, 0x110u32), (9, 0x111), (8, 0x112), (0xE, 0x120), (0xD, 0x121), (0xC, 0x122)] {
        let v = fx.mem.read_u8(p + s);
        fx.mem.write_u8(sp + o, v);
    }
    let c = fx.mem.u8(p);
    if fx.mem.u8(p + 4) & 0x40 == 0 && c < 8 {
        let k = fx.mem.u8(p + 0xB) >> 3;
        let v = fx.mult(k, c);
        fx.mem.set8(sp + 0x113, v);
        let k = fx.mem.u8(p + 0xF) >> 3;
        let c = fx.mem.u8(p);
        fx.mem.set8(sp + 0x123, k.wrapping_mul(c));
    } else {
        let v = fx.mem.read_u8(p + 0xB);
        fx.mem.write_u8(sp + 0x113, v);
        let v = fx.mem.read_u8(p + 0xF);
        fx.mem.write_u8(sp + 0x123, v);
    }
    let m = fx.read_m4(sp + 0xA0);
    *a3 = Some(sp + 0x138);
    if !fx.project_at(sp + 0xE0, &m, sp + 0x118, sp + 0x138) {
        return;
    }
    let m = fx.read_m4(sp + 0xA0);
    *a3 = Some(sp + 0x13C);
    if !fx.project_at(sp + 0xF0, &m, sp + 0x128, sp + 0x13C) {
        return;
    }
    *a3 = Some(u32::from(fx.mem.read_u16(d + 2)));
    fx.desc_alpha(d, 0x48);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x400);
    let depth = if fx.mem.u8(p + 4) & 0x80 != 0 {
        *a3 = Some(0);
        fx.set_xyz_z(sp + 0x128, 0xFF_FFFE);
        let z = fx.xyz_z(sp + 0x128);
        fx.set_xyz_z(sp + 0x118, z);
        0.0
    } else {
        ee::div(ee::add(fx.mem.f(sp + 0x138), fx.mem.f(sp + 0x13C)), 2.0)
    };
    fx.fix_residue(depth, at + 0x248);
    let key = ee::fixunssfsi(depth) as i32;
    fx.submit(d, ot, key);
}
