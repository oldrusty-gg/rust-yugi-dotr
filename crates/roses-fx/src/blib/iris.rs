//! Camera-facing ring sets of the effect library: rings of 36 ten-degree steps about a projected point, drawn as
//! triangle fans or quad strips by a per-ring kind byte, and a
//! whole-screen tint, only while the record's turned up-vector faces the
//! camera.
use super::exe::{camera_vp_into, countdown, libm_cosf, mul_mem, node_source, rot_trans_into};
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, CAMERA_TABLE};

const PI: u32 = 0x4049_0FDB;
const ASPECT: u32 = 0x3FAA_3D71;
const FACING: u32 = 0xBF33_3333;
const RANGE: u32 = 0x3E99_999A;

/// 0x1A2C70 of the u16 window coordinate at `a` plus `d` from the `jal`
/// at `at + off` (its frame left), as u16.
fn fix16(fx: &mut Fx, at: u32, off: u32, a: u32, d: f32) -> i32 {
    let v = ee::add(ee::from_i32(fx.mem.u16(a)), d);
    fx.fix_residue(v, at + off + 8);
    ee::fixunssfsi(v) as u16 as i32
}

/// 0f18e261b7c3 [2 B records; b285 (card 483) 0x1E03B90, b351 (card 187)
/// 0x1E05B10, 952 words] [F]: `rings(at, recs, ot, n) -> 1` steps and
/// draws `n` records (0x6C bytes; a 0x2E0 frame: a 6-register fan
/// descriptor at sp + 0x238 (prim 0x13, RGBAQ / XYZ2 x 3) and an
/// 8-register strip descriptor at sp + 0x280 (prim 0x14, x 4), both ALPHA
/// 0x48 (0x207E20), colours 0x80, Q 1). The camera object's +0x10 vector
/// (0x1FBF20) is copied to sp + 0x220 and normalized (0x20C7E0). Per
/// record: countdown 0x101AF0; angles +0x4C.. += +0x5C.. (i32); the
/// position +0x3C from a node with +5 (0x101BA0); it must project
/// (0x204D80 through the camera 0x20E2B0) and lie on screen (0x101600);
/// the up vector (0, 1, 0, stale w) turned by the angles (0x20BDA0,
/// 0x20C540) must face the camera: its dot (0x20C910) with the camera
/// vector at most -0.7, else the record draws nothing. The matrix: the
/// camera x the turn at the position (0x20C0B0, 0x20B370). Alphas +0x11
/// (the rings) / +0x12 (the tint), x count / 8 over the last 8; the rings'
/// scaled by (-dot - 0.7) / 0.3 (0x1A2B80). Then +0x10 rings j at height
/// +0x34 + +0x38 j (projected and on screen, else skipped) of kind +8+j
/// and radius 0x1018E0(+0x14+4j, w) r: 36 steps of ten degrees (libm cosf
/// / sinf, y x 1.33) at radii r, r + r / 4, r - r / 4; kinds 0..2 fans
/// (centre, previous, current) at the radius r with the alpha on all three
/// / the centre only (the others a quarter) / the outer two; kinds 3..5
/// strips between the previous and current steps of r + r / 4 and r
/// (alpha on the inner edge (3, 5) or the outer (4)), kind 5 also a strip
/// between r - r / 4 and r (vertices 2 / 3 kept); z from the projection
/// (flag 0x80: 0xFFFFFE, key 0), byte 7 of every vertex 1, key the depth
/// (0x1A2C70). Last a whole-screen strip (0x7000..0x9000, z 0xFFFFFF,
/// white, alpha +0x12 x (-dot - 0.7) / 0.3, key 0).
pub fn rings(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x2E0);
    fx.mem.set32(sp + 0x13C, n);
    // 0x1FBF20(0): the camera object's +0x10 (0 without one).
    let cam = fx.mem.read_u32(CAMERA_TABLE);
    let e = if cam != 0 { cam.wrapping_add(0x10) } else { 0 };
    for k in 0..4u32 {
        let v = fx.mem.read_u32(e.wrapping_add(4 * k));
        fx.mem.write_u32(sp + 0x220 + 4 * k, v);
    }
    fx.mem.set32(sp + 0xF0, 0);
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x6C;
        }
        one(fx, at, sp, g, ot);
        i += 1;
        fx.mem.set32(sp + 0xF0, i);
    }
    fx.leave(0x2E0);
    1
}

fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    for o in [0x4Cu32, 0x50, 0x54] {
        let v = fx.mem.i32(g + o).wrapping_add(fx.mem.i32(g + o + 0x10));
        fx.mem.set32(g + o, v);
    }
    node_source(fx, g, g + 0x3C);
    camera_vp_into(fx, sp + 0x180);
    let m = fx.read_m4(sp + 0x180);
    if !fx.project_at(g + 0x3C, &m, sp + 0x230, sp + 0x2DC) || !Fx::on_screen(fx.mem.read_u64(sp + 0x230)) {
        return;
    }
    fx.mem.write_u32(sp + 0x208, 0);
    fx.mem.write_u32(sp + 0x200, 0);
    fx.mem.write_u32(sp + 0x204, 0x3F80_0000);
    let rot = [fx.mem.i32(g + 0x4C), fx.mem.i32(g + 0x50), fx.mem.i32(g + 0x54)];
    let r = fx.rotation_matrix(rot);
    fx.write_m4(sp + 0x1C0, &r);
    let v = std::array::from_fn(|k| fx.mem.f(sp + 0x200 + 4 * k as u32));
    let t = fx.transform(Some(&r), v);
    for (k, c) in t.into_iter().enumerate() {
        fx.mem.setf(sp + 0x210 + 4 * k as u32, c);
    }
    super::trail::normalize3(fx, sp + 0x220);
    let a = |fx: &mut Fx, k: u32| fx.mem.f(sp + 0x210 + 4 * k);
    let b = |fx: &mut Fx, k: u32| fx.mem.f(sp + 0x220 + 4 * k);
    let (y, x, z) = (ee::mul(a(fx, 1), b(fx, 1)), ee::mul(a(fx, 0), b(fx, 0)), ee::mul(a(fx, 2), b(fx, 2)));
    let dot = ee::add(ee::add(x, y), z);
    if !ee_le(dot, f32::from_bits(FACING)) {
        return;
    }
    let fan = sp + 0x238;
    fx.desc_init(fan, 6, 0x13, 0x41_4141_4141);
    for v in [0x248u32, 0x258, 0x268] {
        for k in 0..3u32 {
            fx.mem.write_u8(sp + v + k, 0x80);
        }
        fx.mem.write_u32(sp + v + 4, 0x3F80_0000);
    }
    fx.desc_alpha(fan, 0x48);
    let strip = sp + 0x280;
    fx.desc_init(strip, 8, 0x14, 0x4141_4141_4141_4141);
    for v in [0x290u32, 0x2A0, 0x2B0, 0x2C0] {
        for k in 0..3u32 {
            fx.mem.write_u8(sp + v + k, 0x80);
        }
        fx.mem.write_u32(sp + v + 4, 0x3F80_0000);
    }
    fx.desc_alpha(strip, 0x48);
    camera_vp_into(fx, sp + 0x180);
    rot_trans_into(fx, sp + 0x140, g + 0x4C, g + 0x3C);
    mul_mem(fx, sp + 0x180, sp + 0x140);
    let c = fx.mem.u8(g);
    let (alpha_a, alpha_b) = if c < 8 {
        let a = fx.mem.u8(g + 0x11) >> 3;
        let a = fx.mult(a, c);
        (a, (fx.mem.u8(g + 0x12) >> 3).wrapping_mul(c))
    } else {
        (fx.mem.u8(g + 0x11), fx.mem.u8(g + 0x12))
    };
    fx.mem.set32(sp + 0x110, alpha_b);
    fx.mem.set32(sp + 0x100, 0);
    fx.mem.write_u32(sp + 0x120, g);
    let facing = ee::sub(ee::neg(dot), f32::from_bits(0x3F33_3333));
    fx.mem.setf(sp + 0xE8, facing);
    fx.mem.setf(sp + 0xE4, ee::mul(ee::from_i32(alpha_a), facing));
    let m = fx.read_m4(sp + 0x180);
    let table = rel(fx, at, 0x40C, 0x410);
    let mut j = 0;
    while j < fx.mem.u8(g + 0x10) {
        ring(fx, at, sp, g, ot, &m, table, j);
        j += 1;
        fx.mem.write_u32(sp + 0x120, g + 4 * j as u32);
        fx.mem.set32(sp + 0x100, j);
    }
    // The whole-screen tint.
    let f1 = ee::from_i32(fx.mem.i32(sp + 0x110));
    for v in [0x2C0u32, 0x2B0, 0x2A0, 0x290] {
        for k in 0..3u32 {
            fx.mem.write_u8(sp + v + k, 0xFF);
        }
        fx.mem.write_u32(sp + v + 4, 0x3F80_0000);
    }
    for v in [0x2C8u32, 0x2B8, 0x2A8, 0x298] {
        fx.set_xyz_z(sp + v, 0xFF_FFFF);
    }
    let v = ee::div(ee::mul(f1, fx.mem.f(sp + 0xE8)), f32::from_bits(RANGE));
    fx.fix_residue(v, at + 0xDFC + 8);
    let alpha = ee::fixsfsi(v);
    for o in [0x2C3u32, 0x2B3, 0x2A3, 0x293] {
        fx.mem.set8(sp + o, alpha);
    }
    for (o, v) in [(0x2B8u32, 0x7000), (0x298, 0x7000), (0x2C8, 0x9000), (0x2A8, 0x9000), (0x2AA, 0x7000), (0x29A, 0x7000), (0x2CA, 0x9000), (0x2BA, 0x9000)] {
        fx.mem.set16(sp + o, v);
    }
    fx.submit(strip, ot, 0);
}

/// Ring `j` of the record.
#[allow(clippy::too_many_arguments)]
fn ring(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, m: &crate::exe::M4, table: u32, j: i32) {
    let y = ee::add(fx.mem.f(g + 0x34), ee::mul(fx.mem.f(g + 0x38), ee::from_i32(j)));
    fx.mem.setf(sp + 0x204, y);
    if !fx.project_at(sp + 0x200, m, sp + 0x230, sp + 0x2DC) || !Fx::on_screen(fx.mem.read_u64(sp + 0x230)) {
        return;
    }
    let kind = fx.mem.u8(g + 8 + j as u32);
    let size = fx.mem.f(g + 0x14 + 4 * j as u32);
    let w = fx.mem.f(sp + 0x2DC);
    let r = fx.view_scale(size, w);
    let v = ee::div(fx.mem.f(sp + 0xE4), f32::from_bits(RANGE));
    fx.fix_residue(v, at + 0x3EC + 8);
    let alpha = ee::fixsfsi(v);
    if kind < 6 {
        let target = fx.mem.read_u32(table + 4 * kind as u32).wrapping_sub(at);
        match target {
            0x428 => {
                for o in [0x26Bu32, 0x25B, 0x24B] {
                    fx.mem.set8(sp + o, alpha);
                }
            }
            0x438 => {
                fx.mem.set8(sp + 0x24B, alpha);
                fx.mem.set8(sp + 0x26B, alpha >> 2);
                fx.mem.set8(sp + 0x25B, alpha >> 2);
            }
            0x44C => {
                fx.mem.set8(sp + 0x24B, 0);
                fx.mem.set8(sp + 0x26B, alpha);
                fx.mem.set8(sp + 0x25B, alpha);
            }
            0x45C => {
                fx.mem.set8(sp + 0x2A3, 0);
                fx.mem.set8(sp + 0x293, 0);
                fx.mem.set8(sp + 0x2C3, alpha);
                fx.mem.set8(sp + 0x2B3, alpha);
            }
            0x470 => {
                fx.mem.set8(sp + 0x2A3, alpha);
                fx.mem.set8(sp + 0x293, alpha);
                fx.mem.set8(sp + 0x2C3, 0);
                fx.mem.set8(sp + 0x2B3, 0);
            }
            _ => {}
        }
    }
    let depth = if fx.mem.u8(g + 4) & 0x80 != 0 {
        fx.set_xyz_z(sp + 0x230, 0xFF_FFFE);
        0.0
    } else {
        fx.mem.f(sp + 0x2DC)
    };
    let z = fx.xyz_z(sp + 0x230);
    for v in [0x270u32, 0x260, 0x250, 0x2C8, 0x2B8, 0x2A8, 0x298] {
        fx.set_xyz_z(sp + v, z);
    }
    for o in [0x2CFu32, 0x277, 0x2BF, 0x267, 0x2AF, 0x257, 0x29F] {
        fx.mem.write_u8(sp + o, 1);
    }
    let aspect = f32::from_bits(ASPECT);
    let c0 = libm_cosf(fx, 0.0);
    let s0 = fx.libm_sinf(0.0);
    let (mut x1, mut y1) = (ee::mul(r, c0), ee::mul(aspect, ee::mul(r, s0)));
    let quarter = ee::div(r, 4.0);
    let outer = ee::add(r, quarter);
    let (mut x2, mut y2) = (ee::mul(outer, c0), ee::mul(aspect, ee::mul(outer, s0)));
    let inner = ee::sub(r, quarter);
    let (mut x3, mut y3) = (ee::mul(inner, c0), ee::mul(aspect, ee::mul(inner, s0)));
    let (x0, y0) = (sp + 0x230, sp + 0x232);
    for s1 in 1..0x25 {
        let kind = fx.mem.u8(g + 8 + j as u32);
        let ang = ee::div(ee::mul(f32::from_bits(PI), ee::mul(10.0, ee::from_i32(s1))), 180.0);
        if kind < 3 {
            fx.mem.setf(sp + 0xEC, x1);
            let py = y1;
            let c = libm_cosf(fx, ang);
            x1 = ee::mul(r, c);
            let s = fx.libm_sinf(ang);
            y1 = ee::mul(aspect, ee::mul(r, s));
            let x = fx.mem.u16(x0);
            fx.mem.set16(sp + 0x250, x);
            let y = fx.mem.u16(y0);
            fx.mem.set16(sp + 0x252, y);
            let px = fx.mem.f(sp + 0xEC);
            let v = fix16(fx, at, 0xB90, x0, px);
            fx.mem.set16(sp + 0x260, v);
            let v = fix16(fx, at, 0xBCC, y0, py);
            fx.mem.set16(sp + 0x262, v);
            let v = fix16(fx, at, 0xC04, x0, x1);
            fx.mem.set16(sp + 0x270, v);
            let v = fix16(fx, at, 0xC40, y0, y1);
            fx.mem.set16(sp + 0x272, v);
            fx.fix_residue(depth, at + 0xC4C + 8);
            fx.submit(sp + 0x238, ot, ee::fixunssfsi(depth) as i32);
        } else {
            fx.mem.setf(sp + 0xD8, x3);
            fx.mem.setf(sp + 0xD0, x1);
            fx.mem.setf(sp + 0xD4, x2);
            fx.mem.setf(sp + 0xDC, y1);
            fx.mem.setf(sp + 0xE0, y2);
            let py3 = y3;
            let c = libm_cosf(fx, ang);
            let s = fx.libm_sinf(ang);
            x1 = ee::mul(r, c);
            y1 = ee::mul(aspect, ee::mul(r, s));
            x2 = ee::mul(outer, c);
            y2 = ee::mul(aspect, ee::mul(outer, s));
            x3 = ee::mul(inner, c);
            y3 = ee::mul(aspect, ee::mul(inner, s));
            let px2 = fx.mem.f(sp + 0xD4);
            let v = fix16(fx, at, 0x7E8, x0, px2);
            fx.mem.set16(sp + 0x298, v);
            let py2 = fx.mem.f(sp + 0xE0);
            let v = fix16(fx, at, 0x830, y0, py2);
            fx.mem.set16(sp + 0x29A, v);
            let v = fix16(fx, at, 0x86C, x0, x2);
            fx.mem.set16(sp + 0x2A8, v);
            let v = fix16(fx, at, 0x8A4, y0, y2);
            fx.mem.set16(sp + 0x2AA, v);
            let px1 = fx.mem.f(sp + 0xD0);
            let v = fix16(fx, at, 0x8E4, x0, px1);
            fx.mem.set16(sp + 0x2B8, v);
            let py1 = fx.mem.f(sp + 0xDC);
            let v = fix16(fx, at, 0x924, y0, py1);
            fx.mem.set16(sp + 0x2BA, v);
            let v = fix16(fx, at, 0x95C, x0, x1);
            fx.mem.set16(sp + 0x2C8, v);
            let v = fix16(fx, at, 0x994, y0, y1);
            fx.mem.set16(sp + 0x2CA, v);
            fx.fix_residue(depth, at + 0x9A0 + 8);
            fx.submit(sp + 0x280, ot, ee::fixunssfsi(depth) as i32);
            if kind == 5 {
                let px3 = fx.mem.f(sp + 0xD8);
                let v = fix16(fx, at, 0xA00, x0, px3);
                fx.mem.set16(sp + 0x298, v);
                let v = fix16(fx, at, 0xA44, y0, py3);
                fx.mem.set16(sp + 0x29A, v);
                let v = fix16(fx, at, 0xA80, x0, x3);
                fx.mem.set16(sp + 0x2A8, v);
                let v = fix16(fx, at, 0xAB8, y0, y3);
                fx.mem.set16(sp + 0x2AA, v);
                fx.fix_residue(depth, at + 0xAC8 + 8);
                fx.submit(sp + 0x280, ot, ee::fixunssfsi(depth) as i32);
            }
        }
    }
}

/// 0x1A2C70 of `v` from the call returning to `at + ra` (its frame left).
fn fixu(fx: &mut Fx, at: u32, ra: u32, v: f32) -> i32 {
    fx.fix_residue(v, at + ra);
    ee::fixunssfsi(v) as i32
}

/// The u16 window coordinate at `a` plus `d`, converted (0x1A2C70
/// returning to `at + ra`) into the u16 at `to`.
fn put16(fx: &mut Fx, at: u32, ra: u32, to: u32, a: u32, d: f32) {
    let v = ee::add(ee::from_i32(fx.mem.u16(a)), d);
    let x = fixu(fx, at, ra, v);
    fx.mem.set16(to, x as u16 as i32);
}

/// b25d21b671cb [1 B record; card 319's 0x1E03020, 1200 words] (helper
/// A19, 31 Oct 2026) [F]: `rings_b(at, recs, ot, n) -> 1`, [`rings`]
/// recompiled for 0x58-byte records in a 0x2B0 frame (n / the counters in
/// registers, no slots) with the rings stacked along an aim: per record
/// countdown 0x101AF0; the position +0x38 from a node with +5 (0x101BA0);
/// it must project (0x204D80 through the camera 0x20E2B0 at sp + 0x130,
/// into sp + 0x200 / w sp + 0x2AC) and lie on screen (0x101600); the aim
/// angles (0x101940, sp + 0x1F0) of +0x48 - +0x38 (sp + 0x1D0) turn the
/// up vector (0, 1, 0, stale w) at sp + 0x1B0 (0x20BDA0 into sp + 0x170,
/// 0x20C540 into sp + 0x1C0), whose dot (0x20C910) with the normalized
/// camera vector (0x1FBF20 copied to sp + 0x1E0 once, normalized every
/// record by 0x20C7E0) is kept at sp + 0xDC and must be at most -0.7. A
/// fan (sp + 0x208: 6 registers, prim 0x13) and a strip (sp + 0x250: 8,
/// 0x14), colours 0x80, Q 1, ALPHA 0x48 (0x207E20); the matrix: the camera
/// x the aim turn at +0x48 (0x20C0B0 into sp + 0xF0, 0x20B370). Alphas
/// +0x11 (s5) / +0x12 (s7), x count / 8 over the last 8. Ring j (0..+0x10)
/// at (0, +0x34 (j - 4), 0) (sp + 0x1B0 reused) projected and on screen
/// (else skipped), kind +8+j, radius 0x1018E0(+0x14+4j, w) and alpha
/// fixsfsi(s5 (-dot - 0.7) / 0.3) set by the kind's jump-table case (the
/// table by the `lui` / `addiu` at +0x454 / +0x458) as in [`rings`]; 36
/// steps of 90 / 9 degrees: kinds 0..2 fans, 3.. strips between r + r / 4
/// and r (kind 5 also r - r / 4 and r). Last a whole-screen strip with
/// alpha fixsfsi(s7 (-dot - 0.7) / 0.3) as in [`rings`].
pub fn rings_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x2B0);
    let cam = fx.mem.read_u32(CAMERA_TABLE);
    let e = if cam != 0 { cam.wrapping_add(0x10) } else { 0 };
    for k in 0..4u32 {
        let v = fx.mem.read_u32(e.wrapping_add(4 * k));
        fx.mem.write_u32(sp + 0x1E0 + 4 * k, v);
    }
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x58;
        }
        one_b(fx, at, sp, g, ot);
        i += 1;
    }
    fx.leave(0x2B0);
    1
}

fn one_b(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    node_source(fx, g, g + 0x38);
    camera_vp_into(fx, sp + 0x130);
    let m = fx.read_m4(sp + 0x130);
    if !fx.project_at(g + 0x38, &m, sp + 0x200, sp + 0x2AC) || !Fx::on_screen(fx.mem.read_u64(sp + 0x200)) {
        return;
    }
    for k in 0..3u32 {
        let v = ee::sub(fx.mem.f(g + 0x48 + 4 * k), fx.mem.f(g + 0x38 + 4 * k));
        fx.mem.setf(sp + 0x1D0 + 4 * k, v);
    }
    super::exe::aim_angles(fx, sp + 0x1D0, sp + 0x1F0);
    fx.mem.write_u32(sp + 0x1B8, 0);
    fx.mem.write_u32(sp + 0x1B0, 0);
    fx.mem.write_u32(sp + 0x1B4, 0x3F80_0000);
    let rot = [fx.mem.i32(sp + 0x1F0), fx.mem.i32(sp + 0x1F4), fx.mem.i32(sp + 0x1F8)];
    let r = fx.rotation_matrix(rot);
    fx.write_m4(sp + 0x170, &r);
    let v = std::array::from_fn(|k| fx.mem.f(sp + 0x1B0 + 4 * k as u32));
    let t = fx.transform(Some(&r), v);
    for (k, c) in t.into_iter().enumerate() {
        fx.mem.setf(sp + 0x1C0 + 4 * k as u32, c);
    }
    super::trail::normalize3(fx, sp + 0x1E0);
    let a = |fx: &mut Fx, k: u32| fx.mem.f(sp + 0x1C0 + 4 * k);
    let b = |fx: &mut Fx, k: u32| fx.mem.f(sp + 0x1E0 + 4 * k);
    let (y, x, z) = (ee::mul(a(fx, 1), b(fx, 1)), ee::mul(a(fx, 0), b(fx, 0)), ee::mul(a(fx, 2), b(fx, 2)));
    let dot = ee::add(ee::add(x, y), z);
    fx.mem.setf(sp + 0xDC, dot);
    if !ee_le(dot, f32::from_bits(FACING)) {
        return;
    }
    let f23 = ee::div(90.0, ee::from_i32(9));
    let fan = sp + 0x208;
    fx.desc_init(fan, 6, 0x13, 0x41_4141);
    for v in [0x238u32, 0x228, 0x218] {
        for k in 0..3u32 {
            fx.mem.write_u8(sp + v + k, 0x80);
        }
    }
    for v in [0x23Cu32, 0x22C, 0x21C] {
        fx.mem.write_u32(sp + v, 0x3F80_0000);
    }
    fx.desc_alpha(fan, 0x48);
    let strip = sp + 0x250;
    fx.desc_init(strip, 8, 0x14, 0x4141_4141);
    for v in [0x290u32, 0x280, 0x270, 0x260] {
        for k in 0..3u32 {
            fx.mem.write_u8(sp + v + k, 0x80);
        }
    }
    for v in [0x294u32, 0x284, 0x274, 0x264] {
        fx.mem.write_u32(sp + v, 0x3F80_0000);
    }
    fx.desc_alpha(strip, 0x48);
    camera_vp_into(fx, sp + 0x130);
    rot_trans_into(fx, sp + 0xF0, sp + 0x1F0, g + 0x48);
    mul_mem(fx, sp + 0x130, sp + 0xF0);
    let c = fx.mem.u8(g);
    let (s5, s7) = if c < 8 {
        let a = fx.mem.u8(g + 0x11) >> 3;
        let a = fx.mult(a, c);
        (a, (fx.mem.u8(g + 0x12) >> 3).wrapping_mul(c))
    } else {
        (fx.mem.u8(g + 0x11), fx.mem.u8(g + 0x12))
    };
    let m = fx.read_m4(sp + 0x130);
    let table = rel(fx, at, 0x454, 0x458);
    let mut j = 0;
    while j < fx.mem.u8(g + 0x10) {
        ring_b(fx, at, sp, g, ot, &m, table, j, s5, f23);
        j += 1;
    }
    // The whole-screen tint.
    for v in [0x290u32, 0x280, 0x270, 0x260] {
        for k in 0..3u32 {
            fx.mem.write_u8(sp + v + k, 0xFF);
        }
    }
    for v in [0x294u32, 0x284, 0x274, 0x264] {
        fx.mem.write_u32(sp + v, 0x3F80_0000);
    }
    for v in [0x298u32, 0x288, 0x278, 0x268] {
        fx.set_xyz_z(sp + v, 0xFF_FFFF);
    }
    let facing = ee::sub(ee::neg(fx.mem.f(sp + 0xDC)), f32::from_bits(0x3F33_3333));
    let v = ee::div(ee::mul(ee::from_i32(s7), facing), f32::from_bits(RANGE));
    fx.fix_residue(v, at + 0x11E4);
    let alpha = ee::fixsfsi(v);
    for o in [0x293u32, 0x283, 0x273, 0x263] {
        fx.mem.set8(sp + o, alpha);
    }
    for (o, v) in [(0x288u32, 0x7000), (0x268, 0x7000), (0x298, 0x9000), (0x278, 0x9000), (0x27A, 0x7000), (0x26A, 0x7000), (0x29A, 0x9000), (0x28A, 0x9000)] {
        fx.mem.set16(sp + o, v);
    }
    fx.submit(strip, ot, 0);
}

/// Ring `j` of a [`rings_b`] record.
#[allow(clippy::too_many_arguments)]
fn ring_b(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, m: &crate::exe::M4, table: u32, j: i32, s5: i32, f23: f32) {
    let base = ee::neg(ee::mul(4.0, fx.mem.f(g + 0x34)));
    let y = ee::add(base, ee::mul(fx.mem.f(g + 0x34), ee::from_i32(j)));
    fx.mem.setf(sp + 0x1B4, y);
    if !fx.project_at(sp + 0x1B0, m, sp + 0x200, sp + 0x2AC) || !Fx::on_screen(fx.mem.read_u64(sp + 0x200)) {
        return;
    }
    let size = fx.mem.f(g + 0x14 + 4 * j as u32);
    let w = fx.mem.f(sp + 0x2AC);
    let r = fx.view_scale(size, w);
    let facing = ee::sub(ee::neg(fx.mem.f(sp + 0xDC)), f32::from_bits(0x3F33_3333));
    let v = ee::div(ee::mul(ee::from_i32(s5), facing), f32::from_bits(RANGE));
    fx.fix_residue(v, at + 0x440);
    let alpha = ee::fixsfsi(v);
    let kind = fx.mem.u8(g + 8 + j as u32);
    if (kind as u32) < 6 {
        let target = fx.mem.read_u32(table + 4 * kind as u32).wrapping_sub(at);
        match target {
            0x470 => {
                for o in [0x23Bu32, 0x22B, 0x21B] {
                    fx.mem.set8(sp + o, alpha);
                }
            }
            0x484 => {
                fx.mem.set8(sp + 0x21B, alpha);
                fx.mem.set8(sp + 0x23B, alpha >> 2);
                fx.mem.set8(sp + 0x22B, alpha >> 2);
            }
            0x49C => {
                fx.mem.set8(sp + 0x21B, 0);
                fx.mem.set8(sp + 0x23B, alpha);
                fx.mem.set8(sp + 0x22B, alpha);
            }
            0x4B0 => {
                fx.mem.set8(sp + 0x273, 0);
                fx.mem.set8(sp + 0x263, 0);
                fx.mem.set8(sp + 0x293, alpha);
                fx.mem.set8(sp + 0x283, alpha);
            }
            0x4C8 => {
                fx.mem.set8(sp + 0x273, alpha);
                fx.mem.set8(sp + 0x263, alpha);
                fx.mem.set8(sp + 0x293, 0);
                fx.mem.set8(sp + 0x283, 0);
            }
            _ => {}
        }
    }
    let f22 = if fx.mem.u8(g + 4) & 0x80 != 0 {
        fx.set_xyz_z(sp + 0x200, 0xFF_FFFE);
        0.0
    } else {
        fx.mem.f(sp + 0x2AC)
    };
    let z = fx.xyz_z(sp + 0x200);
    for v in [0x240u32, 0x230, 0x220, 0x298, 0x288, 0x278, 0x268] {
        fx.set_xyz_z(sp + v, z);
    }
    for o in [0x247u32, 0x237, 0x227, 0x29F, 0x28F, 0x27F, 0x26F] {
        fx.mem.write_u8(sp + o, 1);
    }
    let aspect = f32::from_bits(ASPECT);
    let c = libm_cosf(fx, 0.0);
    let mut f24 = ee::mul(r, c);
    let s = fx.libm_sinf(0.0);
    let mut f25 = ee::mul(aspect, ee::mul(r, s));
    let f21 = ee::div(r, 4.0);
    let c = libm_cosf(fx, 0.0);
    let mut f26 = ee::mul(ee::add(r, f21), c);
    let s = fx.libm_sinf(0.0);
    let mut f27 = ee::mul(aspect, ee::mul(ee::add(r, f21), s));
    let c = libm_cosf(fx, 0.0);
    let mut f28 = ee::mul(ee::sub(r, f21), c);
    let s = fx.libm_sinf(0.0);
    let mut f29 = ee::mul(aspect, ee::mul(ee::sub(r, f21), s));
    let (x0, y0) = (sp + 0x200, sp + 0x202);
    let ang = |s2: i32| ee::div(ee::mul(f32::from_bits(PI), ee::mul(f23, ee::from_i32(s2))), 180.0);
    for s2 in 1..(9 * 4 + 1) {
        let kind = fx.mem.u8(g + 8 + j as u32);
        if kind >= 3 {
            let (f30, f31) = (f24, f25);
            fx.mem.setf(sp + 0xE0, f26);
            fx.mem.setf(sp + 0xE4, f28);
            fx.mem.setf(sp + 0xE8, f27);
            fx.mem.setf(sp + 0xEC, f29);
            let c = libm_cosf(fx, ang(s2));
            f24 = ee::mul(r, c);
            let s = fx.libm_sinf(ang(s2));
            f25 = ee::mul(aspect, ee::mul(r, s));
            let c = libm_cosf(fx, ang(s2));
            f26 = ee::mul(ee::add(r, f21), c);
            let s = fx.libm_sinf(ang(s2));
            f27 = ee::mul(aspect, ee::mul(ee::add(r, f21), s));
            let c = libm_cosf(fx, ang(s2));
            f28 = ee::mul(ee::sub(r, f21), c);
            let s = fx.libm_sinf(ang(s2));
            f29 = ee::mul(aspect, ee::mul(ee::sub(r, f21), s));
            let d = fx.mem.f(sp + 0xE0);
            put16(fx, at, 0xA5C, sp + 0x268, x0, d);
            let d = fx.mem.f(sp + 0xE8);
            put16(fx, at, 0xAAC, sp + 0x26A, y0, d);
            put16(fx, at, 0xAF8, sp + 0x278, x0, f26);
            put16(fx, at, 0xB44, sp + 0x27A, y0, f27);
            put16(fx, at, 0xB90, sp + 0x288, x0, f30);
            put16(fx, at, 0xBDC, sp + 0x28A, y0, f31);
            put16(fx, at, 0xC28, sp + 0x298, x0, f24);
            put16(fx, at, 0xC74, sp + 0x29A, y0, f25);
            let key = fixu(fx, at, 0xC84, f22);
            fx.submit(sp + 0x250, ot, key);
            if fx.mem.u8(g + 8 + j as u32) == 5 {
                let d = fx.mem.f(sp + 0xE4);
                put16(fx, at, 0xCF8, sp + 0x268, x0, d);
                let d = fx.mem.f(sp + 0xEC);
                put16(fx, at, 0xD48, sp + 0x26A, y0, d);
                put16(fx, at, 0xD94, sp + 0x278, x0, f28);
                put16(fx, at, 0xDE0, sp + 0x27A, y0, f29);
                let key = fixu(fx, at, 0xDF0, f22);
                fx.submit(sp + 0x250, ot, key);
            }
        } else {
            let (f30, f31) = (f24, f25);
            let c = libm_cosf(fx, ang(s2));
            f24 = ee::mul(r, c);
            let s = fx.libm_sinf(ang(s2));
            f25 = ee::mul(aspect, ee::mul(r, s));
            let x = fx.mem.u16(x0);
            fx.mem.set16(sp + 0x220, x);
            let y = fx.mem.u16(y0);
            fx.mem.set16(sp + 0x222, y);
            put16(fx, at, 0xF18, sp + 0x230, x0, f30);
            put16(fx, at, 0xF64, sp + 0x232, y0, f31);
            put16(fx, at, 0xFB0, sp + 0x240, x0, f24);
            put16(fx, at, 0xFFC, sp + 0x242, y0, f25);
            let key = fixu(fx, at, 0x100C, f22);
            fx.submit(sp + 0x208, ot, key);
        }
    }
}
