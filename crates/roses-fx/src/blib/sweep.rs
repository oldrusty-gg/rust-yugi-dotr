//! Swept arcs of the effect library: a quarter
//! circle profile turned about y, drawn as textured quads from an atlas
//! cell.
use super::beam::length3;
use super::exe::{aim_angles, atlas_cell, camera_vp_into, countdown, mul_mem, node_source, rot_trans_into};
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx};

const PI: u32 = 0x4049_0FDB;
/// 65536 / 360 (0x43360A3D): degrees to the 16-bit angle.
const TURN: u32 = 0x4336_0A3D;

/// 77495e9106f8 [2 B records; b712 (card 14) 0x1E01590, 684 words]
/// [F]: `sweeps(at, recs, ot, n) -> 1` steps and draws `n`
/// records (0x4C bytes; a 0x400 frame, f20..f28 saved; the descriptor at
/// sp + 0x360: RGBAQ / UV / XYZ2 x 4, prim 0x34). Per record: countdown
/// 0x101AF0; the texture (5, +8) with +4 2, else the function's 0x18-byte
/// spec table (record-relative `lui` / `addiu` at +0xC4 / +0xC8, kind
/// word +0 / id byte +4 at +0xE0 / +0xE4) entry +8; +0x24 += +0x28, +0x1C
/// += +0x20; the start 90 - +0x24 (+360 below 0). Columns +0xB, rows +0xC
/// (each at most 8). With +4 1 the ends: nodes +0xD / +0xE of +5 (0x101BA0
/// into sp + 0x340 / 0x350), their difference aimed (0x101940 into +0x3C),
/// times +0x10 and its length (0x20C850) the radius +0x2C, the first the
/// position +0x30. The camera (0x20E2B0, sp + 0x130) x the aim at the
/// position (0x20C0B0 / 0x20B370). The profile at sp + 0x190: per column c
/// the angle start + c +0x14 (90 when in (90, 180)), r (cosf, sinf) of it
/// (libm), z 0; the first column at 90 ends it. Alpha count x 8 below 16,
/// else 0x80; colour 0x80, Q 1; +4 4 steps the cell +9 (& 7); the cell +9
/// of layout +0xF (0x1020B0 into sp + 0x170, blend +0xA) gives the UV
/// steps 16 (v1 - v0) / +0xB and 16 (u1 - u0) / +0xC from 16 v0 / 16 u0.
/// Rows r turn the profile about y by +0x1C + r +0x18 degrees (0x1A2B80,
/// 0x20BDA0), each point turned (0x20C540) and projected (0x204D80 into
/// sp + 0x380 / 0x3FC); a quad to the last row / column when all four
/// projected (row buffers sp + 0x218 XYZ, 0x26C flags, 0x29C w), UVs by
/// 0x1A2C70, TEX0 / TEX1 (filter 1), keyed by the mean w (0x1A2C70).
pub fn sweeps(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x400);
    fx.mem.set32(sp + 0xEC, n);
    let mut g = recs;
    fx.mem.set32(sp + 0xD0, 0);
    while fx.mem.i32(sp + 0xD0) < fx.mem.i32(sp + 0xEC) {
        if fx.mem.i32(sp + 0xD0) != 0 {
            g += 0x4C;
        }
        sweep(fx, at, sp, g, ot);
        let i = fx.mem.i32(sp + 0xD0) + 1;
        fx.mem.set32(sp + 0xD0, i);
    }
    fx.leave(0x400);
    1
}

fn sweep(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let t = if fx.mem.u8(g + 4) & 2 != 0 {
        let id = fx.mem.u8(g + 8);
        fx.texture(5, id)
    } else {
        let e = 0x18 * fx.mem.u8(g + 8) as u32;
        let kind = rel(fx, at, 0xC4, 0xC8).wrapping_add(e);
        let id = rel(fx, at, 0xE0, 0xE4).wrapping_add(e);
        let id = fx.mem.u8(id);
        let kind = fx.mem.i32(kind);
        fx.texture(kind, id)
    };
    let v = ee::add(fx.mem.f(g + 0x24), fx.mem.f(g + 0x28));
    fx.mem.setf(g + 0x24, v);
    let v = ee::add(fx.mem.f(g + 0x1C), fx.mem.f(g + 0x20));
    fx.mem.setf(g + 0x1C, v);
    let mut start = ee::sub(90.0, fx.mem.f(g + 0x24));
    if ee_lt(start, 0.0) {
        start = ee::add(start, 360.0);
    }
    if fx.mem.u8(g + 0xB) >= 9 || fx.mem.u8(g + 0xC) >= 9 {
        return;
    }
    let mut cols = fx.mem.u8(g + 0xB);
    let rows = fx.mem.u8(g + 0xC);
    if fx.mem.u8(g + 4) & 1 != 0 {
        fx.mem.write_u8(g + 5, 1);
        let v = fx.mem.read_u8(g + 0xD);
        fx.mem.write_u8(g + 6, v);
        node_source(fx, g, sp + 0x340);
        let v = fx.mem.read_u8(g + 0xE);
        fx.mem.write_u8(g + 6, v);
        node_source(fx, g, sp + 0x350);
        for c in 0..3u32 {
            let v = ee::sub(fx.mem.f(sp + 0x350 + 4 * c), fx.mem.f(sp + 0x340 + 4 * c));
            fx.mem.setf(sp + 0x330 + 4 * c, v);
        }
        aim_angles(fx, sp + 0x330, g + 0x3C);
        for c in 0..3u32 {
            let v = ee::mul(fx.mem.f(sp + 0x330 + 4 * c), fx.mem.f(g + 0x10));
            fx.mem.setf(sp + 0x330 + 4 * c, v);
        }
        let r = length3(fx, sp + 0x330);
        fx.mem.setf(g + 0x2C, r);
        for c in 0..3u32 {
            let v = fx.mem.read_u32(sp + 0x340 + 4 * c);
            fx.mem.write_u32(g + 0x30 + 4 * c, v);
        }
    }
    for c in 0..3u32 {
        let v = fx.mem.read_u32(g + 0x30 + 4 * c);
        fx.mem.write_u32(sp + 0x330 + 4 * c, v);
    }
    camera_vp_into(fx, sp + 0x130);
    rot_trans_into(fx, sp + 0xF0, g + 0x3C, sp + 0x330);
    mul_mem(fx, sp + 0x130, sp + 0xF0);
    let mut c = 0;
    while c < cols + 1 {
        let mut a = ee::add(start, ee::mul(fx.mem.f(g + 0x14), ee::from_i32(c)));
        if !ee_le(a, 90.0) && ee_lt(a, 180.0) {
            a = 90.0;
        }
        let rad = ee::div(ee::mul(f32::from_bits(PI), a), 180.0);
        let x = super::exe::libm_cosf(fx, rad);
        let p = sp + 0x190 + 16 * c as u32;
        let v = ee::mul(fx.mem.f(g + 0x2C), x);
        fx.mem.setf(p, v);
        let rad = ee::div(ee::mul(f32::from_bits(PI), a), 180.0);
        let y = fx.libm_sinf(rad);
        let v = ee::mul(fx.mem.f(g + 0x2C), y);
        fx.mem.setf(p + 4, v);
        fx.mem.write_u32(p + 8, 0);
        if a == 90.0 {
            cols = c;
            break;
        }
        c += 1;
    }
    let d = sp + 0x360;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    fx.desc_alpha(d, 0x48);
    let count = fx.mem.u8(g);
    let alpha = if count < 0x10 { count << 3 } else { 0x80 };
    for o in [0x3BBu32, 0x3A3, 0x38B, 0x373] {
        fx.mem.set8(sp + o, alpha);
    }
    for o in [0x3BAu32, 0x3A2, 0x38A, 0x372, 0x3B9, 0x3A1, 0x389, 0x371, 0x3B8, 0x3A0, 0x388, 0x370] {
        fx.mem.write_u8(sp + o, 0x80);
    }
    for o in [0x3BCu32, 0x3A4, 0x38C, 0x374] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    if fx.mem.u8(g + 4) & 4 != 0 {
        let v = (fx.mem.u8(g + 9) + 1) & 7;
        fx.mem.set8(g + 9, v);
    }
    let v = fx.mem.read_u8(g + 0xA);
    fx.mem.write_u8(sp + 0x175, v);
    let (cell, layout) = (fx.mem.u8(g + 9), fx.mem.u8(g + 0xF));
    atlas_cell(fx, sp + 0x170, cell, layout);
    let du = ee::from_i32(fx.mem.i32(sp + 0x184).wrapping_sub(fx.mem.i32(sp + 0x180)));
    let du = ee::mul(16.0, ee::div(du, ee::from_i32(fx.mem.u8(g + 0xB))));
    let dv = ee::from_i32(fx.mem.i32(sp + 0x17C).wrapping_sub(fx.mem.i32(sp + 0x178)));
    let dv = ee::mul(16.0, ee::div(dv, ee::from_i32(fx.mem.u8(g + 0xC))));
    let v0 = ee::mul(16.0, ee::from_i32(fx.mem.i32(sp + 0x178)));
    let u0 = ee::mul(16.0, ee::from_i32(fx.mem.i32(sp + 0x180)));
    fx.mem.write_u32(sp + 0x318, 0);
    fx.mem.write_u32(sp + 0x310, 0);
    let (mut s2, mut f20) = (0u32, 0.0f32);
    for r in 0..rows + 1 {
        let deg = ee::add(fx.mem.f(g + 0x1C), ee::mul(ee::from_i32(r), fx.mem.f(g + 0x18)));
        let v = ee::mul(f32::from_bits(TURN), deg);
        fx.fix_residue(v, at + 0x660);
        fx.mem.set32(sp + 0x314, ee::fixsfsi(v));
        let rot = [fx.mem.i32(sp + 0x310), fx.mem.i32(sp + 0x314), fx.mem.i32(sp + 0x318)];
        let m = fx.rotation_matrix(rot);
        fx.write_m4(sp + 0x2D0, &m);
        let mut c = 0;
        while c < cols + 1 {
            let v = fx.read_v4(sp + 0x190 + 16 * c as u32);
            let o = fx.transform(Some(&m), v);
            fx.write_v4(sp + 0x320, o);
            let cam = fx.read_m4(sp + 0x130);
            let s3 = u32::from(fx.project_at(sp + 0x320, &cam, sp + 0x380, sp + 0x3FC));
            if r != 0 && c != 0 && s3 != 0 && s2 != 0 && fx.mem.read_u32(sp + 0x270 + 4 * c as u32) != 0 && fx.mem.read_u32(sp + 0x26C + 4 * c as u32) != 0 {
                quad(fx, at, sp, t, ot, [u0, du, v0, dv], r, c, f20);
            }
            if c != 0 {
                let v = fx.mem.read_u64(sp + 0x3F0);
                fx.mem.write_u64(sp + 0x218 + 8 * c as u32, v);
                fx.mem.setf(sp + 0x29C + 4 * c as u32, f20);
                fx.mem.write_u32(sp + 0x26C + 4 * c as u32, s2);
            }
            let v = fx.mem.read_u64(sp + 0x380);
            fx.mem.write_u64(sp + 0x3F0, v);
            f20 = fx.mem.f(sp + 0x3FC);
            s2 = s3;
            c += 1;
        }
        let v = fx.mem.read_u64(sp + 0x380);
        fx.mem.write_u64(sp + 0x218 + 8 * c as u32, v);
        let w = fx.mem.read_u32(sp + 0x3FC);
        fx.mem.write_u32(sp + 0x29C + 4 * c as u32, w);
        fx.mem.write_u32(sp + 0x26C + 4 * c as u32, s2);
    }
}

/// One quad: this point, the last column's (sp + 0x3F0), the last row's
/// column c / c - 1 (sp + 0x220 / 0x218 + 8 c).
#[allow(clippy::too_many_arguments)]
fn quad(fx: &mut Fx, at: u32, sp: u32, t: u32, ot: u32, uv: [f32; 4], r: i32, c: i32, f20: f32) {
    let [u0, du, v0, dv] = uv;
    let f25 = ee::add(u0, ee::mul(ee::from_i32(c - 1), du));
    let f26 = ee::add(u0, ee::mul(ee::from_i32(c), du));
    let f27 = ee::add(v0, ee::mul(ee::from_i32(r - 1), dv));
    let f28 = ee::add(v0, ee::mul(ee::from_i32(r), dv));
    let d = sp + 0x360;
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    for (o, v, ra) in [(0x378u32, f28, 0x76Cu32), (0x37A, f26, 0x794), (0x390, f27, 0x7BC), (0x392, f26, 0x7E4), (0x3A8, f28, 0x80C), (0x3AA, f25, 0x834), (0x3C0, f27, 0x85C), (0x3C2, f25, 0x884)] {
        fx.fix_residue(v, at + ra);
        let k = ee::fixunssfsi(v);
        let h = fx.mem.read_u16(sp + o);
        fx.mem.write_u16(sp + o, (h & 0xC000) | (k & 0x3FFF) as u16);
    }
    fx.desc_texture(d, t, 1);
    for o in [0x3CFu32, 0x3B7, 0x39F, 0x387] {
        fx.mem.write_u8(sp + o, 0);
    }
    let v = fx.mem.read_u64(sp + 0x220 + 8 * c as u32);
    fx.mem.write_u64(sp + 0x398, v);
    let v = fx.mem.read_u64(sp + 0x3F0);
    fx.mem.write_u64(sp + 0x3B0, v);
    let v = fx.mem.read_u64(sp + 0x218 + 8 * c as u32);
    fx.mem.write_u64(sp + 0x3C8, v);
    let w = ee::add(fx.mem.f(sp + 0x3FC), f20);
    let w = ee::add(w, fx.mem.f(sp + 0x2A0 + 4 * c as u32));
    let w = ee::add(fx.mem.f(sp + 0x29C + 4 * c as u32), w);
    let w = ee::div(w, 4.0);
    fx.fix_residue(w, at + 0x950);
    let key = ee::fixunssfsi(w) as i32;
    fx.submit(d, ot, key);
}
