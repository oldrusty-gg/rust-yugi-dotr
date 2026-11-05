//! Halos of the effect library: a gouraud disc
//! of triangles around a record's projected origin and, unless flag 1, a
//! textured band along its rim drawn twice (across y, then across x).
use super::exe::{camera_vp_into, countdown, libm_cosf, mul_mem, node_source, tex_rect, translation_into};
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

const PI: u32 = 0x4049_0FDB;
const ASPECT: u32 = 0x3FAA_3D71;

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v);
    fx.mem.write_u16(a, x as u16);
}

/// 91abaa11813e [26 B records; b012 0x1E00440, 852 words] [F]:
/// `halo(recs, ot, n) -> 1` steps and draws `n` records (0x4C bytes; a
/// 0x220 frame; the triangle descriptor at sp + 0x158, the band's at sp +
/// 0x1A0; the band's sprite spec (record-relative `lui` / `addiu` at +0x50
/// / +0x54) at sp + 0x130, its texture asked once). Per record: countdown
/// 0x101AF0; origin +0x1C += +0x2C, radius +0x10 += +0x14; the point
/// +0x3C (vec4) projected through camera x translation(origin from a node,
/// 0x101BA0); radius and band width +0x18 by 0x1018E0; key w (flag 0x80:
/// depth 0xFFFFFE, key 0); colours +0xC.. centre / +8.. rim, alpha +0xF
/// (band half) faded over the first and last 8 counts unless flag 0x40,
/// on odd counts flag 2 dims it by 16 (above 16), flag 4 halves it; ALPHA
/// 0x42 (flag 8) or 0x48 (0x207E20). Then 4 x min(+7, 9) + 0 .. steps of
/// 90 / that degrees (libm cosf / sinf, y x 1.33): a triangle centre, last
/// rim point, this one; the band quad between them widened by the width.
pub fn halo(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let spec = rel(fx, at, 0x50, 0x54);
    let sp = fx.enter(0x220);
    for k in (0..16u32).step_by(8) {
        let v = fx.mem.read_u64((spec & !15) + k);
        fx.mem.write_u64(sp + 0x130 + k, v);
    }
    let v = fx.mem.read_u64((spec + 0x10) & !7);
    fx.mem.write_u64(sp + 0x140, v);
    let (kind, id) = (fx.mem.i32(sp + 0x130), fx.mem.u8(sp + 0x134));
    let tex = fx.texture(kind, id);
    let mut p = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            p += 0x4C;
        }
        one(fx, sp, p, ot, tex);
        i += 1;
    }
    fx.leave(0x220);
    1
}

fn one(fx: &mut Fx, sp: u32, p: u32, ot: u32, tex: u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(p + 0x1C + 4 * k), fx.mem.f(p + 0x2C + 4 * k));
        fx.mem.setf(p + 0x1C + 4 * k, v);
    }
    let v = ee::add(fx.mem.f(p + 0x10), fx.mem.f(p + 0x14));
    fx.mem.setf(p + 0x10, v);
    for k in 0..4u32 {
        let v = fx.mem.read_u32(p + 0x3C + 4 * k);
        fx.mem.write_u32(sp + 0x120 + 4 * k, v);
    }
    let s3 = if fx.mem.u8(p + 7) < 10 { fx.mem.u8(p + 7) } else { 9 };
    let step = ee::div(90.0, ee::from_i32(s3));
    node_source(fx, p, p + 0x1C);
    camera_vp_into(fx, sp + 0xE0);
    translation_into(fx, sp + 0xA0, p + 0x1C);
    mul_mem(fx, sp + 0xE0, sp + 0xA0);
    let m = fx.read_m4(sp + 0xE0);
    if !fx.project_at(sp + 0x120, &m, sp + 0x150, sp + 0x21C) || !Fx::on_screen(fx.mem.read_u64(sp + 0x150)) {
        return;
    }
    let w = fx.mem.f(sp + 0x21C);
    let size = fx.mem.f(p + 0x10);
    let r = fx.view_scale(size, w);
    let size = fx.mem.f(p + 0x18);
    let width = fx.view_scale(size, w);
    let depth = if fx.mem.u8(p + 4) & 0x80 != 0 {
        fx.set_xyz_z(sp + 0x150, 0xFF_FFFE);
        0.0
    } else {
        fx.mem.f(sp + 0x21C)
    };
    let a = sp + 0x158;
    fx.desc_init(a, 6, 0x13, 0x41_4141);
    for (s, d) in [(0xEu32, &[0x168u32][..]), (0xA, &[0x188, 0x178]), (0xD, &[0x169]), (9, &[0x189, 0x179]), (0xC, &[0x16A]), (8, &[0x18A, 0x17A])] {
        let v = fx.mem.read_u8(p + s);
        for &o in d {
            fx.mem.write_u8(sp + o, v);
        }
    }
    for o in [0x18Cu32, 0x17C, 0x16C] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let alpha = if fx.mem.u8(p + 4) & 8 != 0 { 0x42 } else { 0x48 };
    fx.desc_alpha(a, alpha);
    let b = sp + 0x1A0;
    fx.desc_init(b, 9, 0x24, 0x4_2424_2421);
    for (s, o) in [(0xAu32, 0x1B0u32), (9, 0x1B1), (8, 0x1B2)] {
        let v = fx.mem.read_u8(p + s);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0x1B4, 0x3F80_0000);
    tex_rect(fx, b, sp + 0x130, tex);
    let al = fx.mem.read_u8(p + 0xF);
    fx.mem.write_u8(sp + 0x16B, al);
    fx.mem.write_u8(sp + 0x1B3, al >> 1);
    fx.mem.write_u8(sp + 0x18B, 0);
    fx.mem.write_u8(sp + 0x17B, 0);
    if fx.mem.u8(p + 4) & 0x40 == 0 {
        let c = fx.mem.u8(p);
        let k = if c < 8 { Some(c) } else { Some((fx.mem.u8(p + 2) - c) & 0xFF).filter(|&g| g < 8) };
        if let Some(k) = k {
            let a = fx.mem.u8(p + 0xF) >> 3;
            let v = fx.mult(a, k);
            fx.mem.set8(sp + 0x16B, v);
            // mult1: LO1, not LO.
            let v = (fx.mem.u8(p + 0xF) >> 4).wrapping_mul(k);
            fx.mem.set8(sp + 0x1B3, v);
        }
    }
    if fx.mem.u8(p) & 1 != 0 {
        if fx.mem.u8(p + 4) & 2 != 0 && fx.mem.u8(sp + 0x16B) >= 0x11 {
            let v = fx.mem.u8(sp + 0x16B) - 0x10;
            fx.mem.set8(sp + 0x16B, v);
        }
        if fx.mem.u8(p + 4) & 4 != 0 && fx.mem.u8(sp + 0x16B) > 0 {
            let v = fx.mem.u8(sp + 0x16B) >> 1;
            fx.mem.set8(sp + 0x16B, v);
        }
    }
    let mut cx = ee::mul(r, libm_cosf(fx, 0.0));
    let sn = fx.libm_sinf(0.0);
    let mut cy = ee::mul(f32::from_bits(ASPECT), ee::mul(r, sn));
    for (from, to) in [(0x150u32, 0x190u32), (0x190, 0x180), (0x180, 0x170), (0x170, 0x1F0), (0x1F0, 0x1E0), (0x1E0, 0x1D0), (0x1D0, 0x1C0)] {
        let z = fx.xyz_z(sp + from);
        fx.set_xyz_z(sp + to, z);
    }
    for o in [0x197u32, 0x187, 0x177, 0x1F7, 0x1E7, 0x1D7, 0x1C7] {
        fx.mem.write_u8(sp + o, 1);
    }
    let key = ee::fixunssfsi(depth) as i32;
    let f = |v: i32| ee::from_i32(v);
    let mut s2 = 1;
    while s2 < 4 * s3 + 1 {
        let (px, py) = (cx, cy);
        let ang = ee::div(ee::mul(f32::from_bits(PI), ee::mul(step, f(s2))), 180.0);
        cx = ee::mul(r, libm_cosf(fx, ang));
        let sn = fx.libm_sinf(ang);
        cy = ee::mul(f32::from_bits(ASPECT), ee::mul(r, sn));
        let (x, y) = (fx.mem.read_u16(sp + 0x150), fx.mem.read_u16(sp + 0x152));
        fx.mem.write_u16(sp + 0x170, x);
        fx.mem.write_u16(sp + 0x172, y);
        let (x, y) = (f(i32::from(x)), f(i32::from(y)));
        fix16(fx, sp + 0x180, ee::add(x, px));
        fix16(fx, sp + 0x182, ee::add(y, py));
        fix16(fx, sp + 0x190, ee::add(x, cx));
        fix16(fx, sp + 0x192, ee::add(y, cy));
        fx.submit(a, ot, key);
        if fx.mem.u8(p + 4) & 1 == 0 {
            band(fx, sp, ot, key, [px, py, cx, cy], width);
        }
        s2 += 1;
    }
}

fn band(fx: &mut Fx, sp: u32, ot: u32, key: i32, [px, py, cx, cy]: [f32; 4], w: f32) {
    let f = |v: i32| ee::from_i32(v);
    let x = f(fx.mem.u16(sp + 0x150));
    let y = f(fx.mem.u16(sp + 0x152));
    let b = sp + 0x1A0;
    let v = ee::add(x, px);
    fix16(fx, sp + 0x1E0, v);
    fix16(fx, sp + 0x1C0, v);
    let v = ee::add(x, cx);
    fix16(fx, sp + 0x1F0, v);
    fix16(fx, sp + 0x1D0, v);
    let (y0, y1) = (ee::add(y, py), ee::add(y, cy));
    fix16(fx, sp + 0x1C2, ee::sub(y0, w));
    fix16(fx, sp + 0x1E2, ee::add(w, y0));
    fix16(fx, sp + 0x1D2, ee::sub(y1, w));
    fix16(fx, sp + 0x1F2, ee::add(w, y1));
    fx.submit(b, ot, key);
    fix16(fx, sp + 0x1E2, y0);
    fix16(fx, sp + 0x1C2, y0);
    fix16(fx, sp + 0x1F2, y1);
    fix16(fx, sp + 0x1D2, y1);
    let (x0, x1) = (ee::add(x, px), ee::add(x, cx));
    fix16(fx, sp + 0x1C0, ee::add(w, x0));
    fix16(fx, sp + 0x1E0, ee::sub(x0, w));
    fix16(fx, sp + 0x1D0, ee::add(w, x1));
    fix16(fx, sp + 0x1F0, ee::sub(x1, w));
    fx.submit(b, ot, key);
}
