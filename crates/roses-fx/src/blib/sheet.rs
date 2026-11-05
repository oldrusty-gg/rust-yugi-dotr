//! A shrinking textured sheet of the effect library.
use super::exe::{alpha_mode, atlas_cell, camera_vp_into, countdown, mul_mem, node_source, rot_trans_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

/// RGBAQ / ST / XYZF2 x 4 (a textured strip).
const REGS_TEX4: u64 = 0x4214_2142_1421;

/// 127e33ae418a [17 B records; b098 0x1E02980, 348 words] [F]:
/// `sheets(g, ot, n) -> 1` steps and draws `n` sheet records (0x3C bytes
/// from `g`; a 0x220 frame, the descriptor at sp + 0x188). Per record:
/// countdown (0x101AF0); the own-slot texture +8 (0x202530(5)); half
/// width +0x10 shrunk to 0 over the count (+0x10 - elapsed x +0x10 /
/// count +2), half height +0x14 + elapsed x +0x18; the corners (-w, -h),
/// (w, -h), (-w, h), (w, h) in the plane z = 0 (sp + 0x120..); the
/// position +0x2C from a node with +5 (0x101BA0); through the camera x
/// (angles +0x1C, position +0x2C) (0x20E2B0, 0x20C0B0, 0x20B370 into sp +
/// 0xA0); colour +0xE, +0xD, +0xC, alpha +0xF (x count / 8 over the last
/// 8 counts unless flag 0x40); all four projected (0x204D80, w at sp +
/// 0x160..) or the record is skipped; atlas cell +0xA of layout +9 with
/// blend +0xB (0x1020B0 into the spec sp + 0x170), STQ 0..1 over the
/// whole cell, REGION_CLAMP from the cell (0x208400), ALPHA from the
/// blend's selectors (0x101C80 into sp + 0x21C, stale for an unknown
/// blend; 0x207E20), texture (0x207E50 filter 1); flag 0x80 at the far Z
/// (0xFFFFFE) with key 0, else keyed by the mean w (0x1A2C70).
pub fn sheets(fx: &mut Fx, at: u32, g: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x220);
    let mut g = g;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x3C;
        }
        one(fx, at, sp, g, ot, 0x500);
        i += 1;
    }
    fx.leave(0x220);
    1
}

/// f694ab0da4da [3 B records; card 602's 0x1E054E0, 372 words] (helper
/// A19, 31 Oct 2026) [F]: [`sheets`] recompiled in a 0x1E0 frame (the
/// locals 0x40 lower = the same caller-relative addresses; n / the
/// counter / the texture in registers), its 0x1A2C70 returning to +0x578.
pub fn sheets_c(fx: &mut Fx, at: u32, g: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1E0);
    let mut g = g;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x3C;
        }
        one(fx, at, sp - 0x40, g, ot, 0x578);
        i += 1;
    }
    fx.leave(0x1E0);
    1
}

fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, ra: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 8);
    let tex = fx.texture(5, id);
    let count = fx.mem.u8(g + 2);
    let elapsed = count - fx.mem.u8(g);
    let size = fx.mem.f(g + 0x10);
    let step = ee::div(size, ee::from_i32(count));
    let e = ee::from_i32(elapsed);
    let w = ee::sub(size, ee::mul(e, step));
    let h = ee::add(fx.mem.f(g + 0x14), ee::mul(fx.mem.f(g + 0x18), e));
    for o in [0x158u32, 0x148, 0x138, 0x128] {
        fx.mem.write_u32(sp + o, 0);
    }
    for (o, v) in [(0x140u32, ee::neg(w)), (0x120, ee::neg(w)), (0x150, w), (0x130, w), (0x134, ee::neg(h)), (0x124, ee::neg(h)), (0x154, h), (0x144, h)] {
        fx.mem.setf(sp + o, v);
    }
    node_source(fx, g, g + 0x2C);
    camera_vp_into(fx, sp + 0xA0);
    rot_trans_into(fx, sp + 0xE0, g + 0x1C, g + 0x2C);
    mul_mem(fx, sp + 0xA0, sp + 0xE0);
    let d = sp + 0x188;
    fx.desc_init(d, 0xC, 0x34, REGS_TEX4);
    let verts = [0x198u32, 0x1B0, 0x1C8, 0x1E0];
    for (src, k) in [(0xEu32, 0u32), (0xD, 1), (0xC, 2)] {
        let v = fx.mem.read_u8(g + src);
        for o in [0x1E0u32, 0x1C8, 0x1B0, 0x198] {
            fx.mem.write_u8(sp + o + k, v);
        }
    }
    let c = fx.mem.u8(g);
    let alpha = if fx.mem.u8(g + 4) & 0x40 == 0 && c < 8 {
        let k = fx.mem.u8(g + 0xF) >> 3;
        fx.mult(k, c)
    } else {
        fx.mem.u8(g + 0xF)
    };
    for o in [0x1E3u32, 0x1CB, 0x1B3, 0x19B] {
        fx.mem.set8(sp + o, alpha);
    }
    let corners = [(0x120u32, 0x1A8u32, 0x160u32), (0x130, 0x1C0, 0x164), (0x140, 0x1D8, 0x168), (0x150, 0x1F0, 0x16C)];
    for (v, xyz, wv) in corners {
        let m = fx.read_m4(sp + 0xA0);
        if !fx.project_at(sp + v, &m, sp + xyz, sp + wv) {
            return;
        }
    }
    let b = fx.mem.read_u8(g + 0xB);
    fx.mem.write_u8(sp + 0x175, b);
    let (cell, layout) = (fx.mem.u8(g + 0xA), fx.mem.u8(g + 9));
    atlas_cell(fx, sp + 0x170, cell, layout);
    let q: [f32; 4] = std::array::from_fn(|k| ee::div(1.0, fx.mem.f(sp + 0x160 + 4 * k as u32)));
    for (k, v) in verts.iter().enumerate() {
        fx.mem.setf(sp + v + 4, q[k]);
    }
    fx.mem.write_u32(sp + 0x1A0, 0);
    fx.mem.write_u32(sp + 0x1A4, 0);
    fx.mem.setf(sp + 0x1B8, q[1]);
    fx.mem.write_u32(sp + 0x1BC, 0);
    fx.mem.write_u32(sp + 0x1D0, 0);
    fx.mem.setf(sp + 0x1D4, q[2]);
    fx.mem.setf(sp + 0x1E8, q[3]);
    fx.mem.setf(sp + 0x1EC, q[3]);
    let (u0, u1, v0, v1) = (fx.mem.i32(sp + 0x178), fx.mem.i32(sp + 0x17C), fx.mem.i32(sp + 0x180), fx.mem.i32(sp + 0x184));
    let clamp = ((i64::from(u0) << 4) as u64 | 0xA) | ((i64::from(u1.wrapping_sub(1)) << 14) as u64) | ((i64::from(v0) << 24) as u64) | ((i64::from(v1.wrapping_sub(1)) << 34) as u64);
    fx.desc_clamp(d, clamp);
    alpha_mode(fx, sp + 0x170, sp + 0x21C);
    let s = |fx: &mut Fx, k: u32| u64::from(fx.mem.read_u8(sp + 0x21C + k));
    let a = s(fx, 0) | (s(fx, 1) << 2) | (s(fx, 2) << 4) | (s(fx, 3) << 6);
    fx.desc_alpha(d, a);
    fx.desc_texture(d, tex, 1);
    let depth = if fx.mem.u8(g + 4) & 0x80 != 0 {
        let mut z = 0xFF_FFFE;
        for o in [0x1F0u32, 0x1D8, 0x1C0, 0x1A8] {
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
        }
        0.0
    } else {
        let (w0, w1, w2, w3) = (fx.mem.f(sp + 0x160), fx.mem.f(sp + 0x164), fx.mem.f(sp + 0x168), fx.mem.f(sp + 0x16C));
        ee::div(ee::add(w3, ee::add(w2, ee::add(w0, w1))), 4.0)
    };
    let key = ee::fixunssfsi(depth) as i32;
    fx.fix_residue(depth, at + ra);
    fx.submit(d, ot, key);
}

/// a05609c6c0fb [2 B records; b758 (card 86) 0x1E03760, 464 words]
/// [F]: `tiles(recs, ot, n) -> 1` steps and draws `n` flat
/// square tiles (0x44 bytes; a 0x2D0 frame, the descriptor at sp + 0x238:
/// RGBAQ / ST / XYZF2 x 4). Per record: countdown (0x101AF0); the texture
/// (5, +8); unless +4 bit 4, i32 angles +0x18.. += +0x28.. and the half
/// size +0xC += +0x10; the position +0x38 (from a node with +5, 0x101BA0)
/// at sp + 0x150; the camera (0x20E2B0) x the rotation / translation
/// (0x20C0B0, 0x20B370) at sp + 0x110. A 3 x 3 grid (x, 0, z) for x, z in
/// -s, 0, s (a rodata jump table, z rows) projected (0x204D80: XYZ sp +
/// 0x170 + 8 k, w sp + 0x1C0 + 4 k, the result sp + 0x210 + 4 k); colour
/// +0x16 / +0x15 / +0x14, alpha +0x17 (x count / 8 over the last 8); the
/// atlas cell +0xA of layout +9 with blend +0xB (0x1020B0 into the spec
/// sp + 0x1F0). Four quads (0, 1, 3, 4), (2, 1, 5, 4), (6, 7, 3, 4), (8,
/// 7, 5, 4) when all four corners projected: ST (0, 0) (q, 0) (0, q) (q,
/// q), REGION_CLAMP from the cell, ALPHA from the blend's selectors
/// (0x101C80 into sp + 0x2CC, stale for an unknown blend), the texture
/// (filter 1); +4 bit 0x80 at the far Z (0xFFFFFE) with key 0, else keyed
/// by the mean w (0x1A2C70).
pub fn tiles(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x2D0);
    fx.mem.set32(sp + 0xC4, n);
    fx.mem.write_u32(sp + 0xC8, ot);
    let mut g = recs;
    fx.mem.set32(sp + 0xB0, 0);
    while fx.mem.i32(sp + 0xB0) < fx.mem.i32(sp + 0xC4) {
        if fx.mem.i32(sp + 0xB0) != 0 {
            g += 0x44;
        }
        tile(fx, at, sp, g, ot);
        let v = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, v);
    }
    fx.leave(0x2D0);
    1
}

fn tile(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 8);
    let tex = fx.texture(5, id);
    fx.mem.write_u32(sp + 0xC0, tex);
    if fx.mem.u8(g + 4) & 4 == 0 {
        for k in 0..3u32 {
            let v = fx.mem.i32(g + 0x18 + 4 * k).wrapping_add(fx.mem.i32(g + 0x28 + 4 * k));
            fx.mem.set32(g + 0x18 + 4 * k, v);
        }
        let s = ee::add(fx.mem.f(g + 0xC), fx.mem.f(g + 0x10));
        fx.mem.setf(g + 0xC, s);
    }
    for k in 0..3u32 {
        let v = fx.mem.read_u32(g + 0x38 + 4 * k);
        fx.mem.write_u32(sp + 0x150 + 4 * k, v);
    }
    if fx.mem.read_u8(g + 5) != 0 {
        node_source(fx, g, sp + 0x150);
    }
    camera_vp_into(fx, sp + 0x110);
    rot_trans_into(fx, sp + 0xD0, g + 0x18, sp + 0x150);
    mul_mem(fx, sp + 0x110, sp + 0xD0);
    fx.mem.write_u32(sp + 0x164, 0);
    for k in 0..9u32 {
        let s = fx.mem.f(g + 0xC);
        let (x, z) = ((k % 3) as i32 - 1, (k / 3) as i32 - 1);
        let pick = |c: i32| match c {
            -1 => Some(ee::neg(s)),
            0 => None,
            _ => Some(s),
        };
        match pick(x) {
            Some(v) => fx.mem.setf(sp + 0x160, v),
            None => fx.mem.write_u32(sp + 0x160, 0),
        }
        match pick(z) {
            Some(v) => fx.mem.setf(sp + 0x168, v),
            None => fx.mem.write_u32(sp + 0x168, 0),
        }
        let m = fx.read_m4(sp + 0x110);
        let ok = fx.project_at(sp + 0x160, &m, sp + 0x170 + 8 * k, sp + 0x1C0 + 4 * k);
        fx.mem.set32(sp + 0x210 + 4 * k, i32::from(ok));
    }
    let d = sp + 0x238;
    fx.desc_init(d, 0xC, 0x34, REGS_TEX4);
    for (src, k) in [(0x16u32, 0u32), (0x15, 1), (0x14, 2)] {
        let v = fx.mem.read_u8(g + src);
        for o in [0x290u32, 0x278, 0x260, 0x248] {
            fx.mem.write_u8(sp + o + k, v);
        }
    }
    for o in [0x2A7u32, 0x28F, 0x277, 0x25F] {
        fx.mem.write_u8(sp + o, 0);
    }
    let b = fx.mem.read_u8(g + 0xB);
    fx.mem.write_u8(sp + 0x1F5, b);
    let (cell, layout) = (fx.mem.u8(g + 0xA), fx.mem.u8(g + 9));
    atlas_cell(fx, sp + 0x1F0, cell, layout);
    let c = fx.mem.u8(g);
    let alpha = if c < 8 {
        let k = fx.mem.u8(g + 0x17) >> 3;
        fx.mult(k, c)
    } else {
        fx.mem.u8(g + 0x17)
    };
    for o in [0x293u32, 0x27B, 0x263, 0x24B] {
        fx.mem.set8(sp + o, alpha);
    }
    for (a, b, c, e) in [(0u32, 1u32, 3u32, 4u32), (2, 1, 5, 4), (6, 7, 3, 4), (8, 7, 5, 4)] {
        if [a, b, c, e].iter().any(|&k| fx.mem.i32(sp + 0x210 + 4 * k) == 0) {
            continue;
        }
        for (k, o) in [(a, 0x258u32), (b, 0x270), (c, 0x288), (e, 0x2A0)] {
            let v = fx.mem.read_u64(sp + 0x170 + 8 * k);
            fx.mem.write_u64(sp + o, v);
        }
        let w = |fx: &mut Fx, k: u32| fx.mem.f(sp + 0x1C0 + 4 * k);
        let depth = if fx.mem.u8(g + 4) & 0x80 != 0 {
            let mut z = 0xFF_FFFE;
            for o in [0x2A0u32, 0x288, 0x270, 0x258] {
                fx.set_xyz_z(sp + o, z);
                z = fx.xyz_z(sp + o);
            }
            0.0
        } else {
            let (wa, wb, wc, we) = (w(fx, a), w(fx, b), w(fx, c), w(fx, e));
            ee::div(ee::add(we, ee::add(wc, ee::add(wa, wb))), 4.0)
        };
        let (qa, qb, qc, qe) = (ee::div(1.0, w(fx, a)), ee::div(1.0, w(fx, b)), ee::div(1.0, w(fx, c)), ee::div(1.0, w(fx, e)));
        fx.mem.setf(sp + 0x24C, qa);
        fx.mem.setf(sp + 0x264, qb);
        fx.mem.setf(sp + 0x27C, qc);
        fx.mem.setf(sp + 0x294, qe);
        fx.mem.write_u32(sp + 0x250, 0);
        fx.mem.write_u32(sp + 0x254, 0);
        fx.mem.setf(sp + 0x268, qb);
        fx.mem.write_u32(sp + 0x26C, 0);
        fx.mem.write_u32(sp + 0x280, 0);
        fx.mem.setf(sp + 0x284, qc);
        fx.mem.setf(sp + 0x298, qe);
        fx.mem.setf(sp + 0x29C, qe);
        let (u0, u1, v0, v1) = (fx.mem.i32(sp + 0x1F8), fx.mem.i32(sp + 0x1FC), fx.mem.i32(sp + 0x200), fx.mem.i32(sp + 0x204));
        let clamp = ((i64::from(u0) << 4) as u64 | 0xA) | ((i64::from(u1.wrapping_sub(1)) << 14) as u64) | ((i64::from(v0) << 24) as u64) | ((i64::from(v1.wrapping_sub(1)) << 34) as u64);
        fx.desc_clamp(d, clamp);
        alpha_mode(fx, sp + 0x1F0, sp + 0x2CC);
        let s = |fx: &mut Fx, k: u32| u64::from(fx.mem.read_u8(sp + 0x2CC + k));
        let al = s(fx, 0) | (s(fx, 1) << 2) | (s(fx, 2) << 4) | (s(fx, 3) << 6);
        fx.desc_alpha(d, al);
        let tex = fx.mem.read_u32(sp + 0xC0);
        fx.desc_texture(d, tex, 1);
        let key = ee::fixunssfsi(depth) as i32;
        fx.fix_residue(depth, at + 0x6C4);
        let ot = fx.mem.read_u32(sp + 0xC8);
        fx.submit(d, ot, key);
    }
    let _ = ot;
}
