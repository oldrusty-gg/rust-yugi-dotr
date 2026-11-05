//! The radial flash of the effect library: a fan of triangles round a
//! projected centre, with a textured spike on every rim segment.
use super::exe::{camera_vp_into, countdown, libm_cosf, mul_mem, node_source, tex_rect_a3, translation_into};
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

/// pi (0x40490FDB) and the vertical stretch 1.33 (0x3FAA3D71).
const PI: u32 = 0x4049_0FDB;
const ASPECT: u32 = 0x3FAA_3D71;

/// 39fa9d123f4a [89 B records; b059 0x1E02020, 748 words] [F]:
/// `flash(g, ot, n) -> 1` steps and draws `n` flash records (0x4C bytes
/// each from `g`). The record's sprite spec (0x18 bytes, record-relative:
/// the `lui` / `addiu` at +0x5C / +0x60) is copied to the stack local
/// sp + 0x180 and its texture fetched (0x202530). Per record: countdown
/// (0x101AF0; not due: skipped), the centre +0x1C += velocity +0x2C, the
/// radius +0x10 += +0x14, the centre from a node (0x101BA0), projected
/// through the camera with the centre's translation (0x20E2B0, 0x20C0B0,
/// 0x20B370, 0x204D80 of the local vec +0x3C) and kept on screen
/// (0x101600); radii from +0x10 / +0x18 (0x1018E0). A fan of 4 x min(r[7],
/// 9) + 1 rim points round the centre (step 90 / that degrees, libm cos /
/// sin 0x1A7158 / 0x1A7348, y stretched 1.33): each segment a gouraud
/// triangle (centre colour +0xC..+0xE, alpha +0xF faded over the first /
/// last 8 counts unless flag 0x40, damped by flags 2 / 4 on odd counts;
/// rim colour +8..+0xA, alpha 0) and, unless flag 1, the spike quad twice
/// (sprite spec UVs, 0x101D20). Flag 0x80 draws at the far Z with key 0,
/// else at the centre's depth; flag 8 picks ALPHA 0x42 for the fan.
pub fn flash(fx: &mut Fx, at: u32, g: u32, ot: u32, n: i32) -> i32 {
    flash_a3(fx, at, g, ot, n).0
}

/// [`flash`] and the a3 it leaves (helper B10, for a later boundary call
/// that reports it) [F b629 0x1E00AF8 / 0x101D20's 0x207E20; run b629]:
/// per counting record the projection's w pointer sp + 0x26C when it
/// fails or falls outside the window, else the spike descriptor's flags
/// before 0x207E20 sets 0x200 (tex_rect's ALPHA, the last a3 writer).
/// `None` when no record counted (the caller's a3 is kept).
pub fn flash_a3(fx: &mut Fx, at: u32, g: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let src = rel(fx, at, 0x5C, 0x60);
    let mut a3 = None;
    let r = flash_body(fx, src, g, ot, n, &mut a3);
    (r, a3)
}

/// d5ca77d8029e [39 B records; b002 0x1E022B0, 668 words] [F]: [`flash`]
/// compiled with a 0x20 larger frame (the same locals at the same
/// addresses below the caller's stack) and other scheduling; its sprite
/// spec's `lui` / `addiu` at +0x48 / +0x50.
pub fn flash_b(fx: &mut Fx, at: u32, g: u32, ot: u32, n: i32) -> i32 {
    flash_b_a3(fx, at, g, ot, n).0
}

/// [`flash_b`] and the a3 it leaves, as [`flash_a3`].
pub fn flash_b_a3(fx: &mut Fx, at: u32, g: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let src = rel(fx, at, 0x48, 0x50);
    let mut a3 = None;
    let r = flash_body(fx, src, g, ot, n, &mut a3);
    (r, a3)
}


/// The flash with its sprite spec at `src`; locals as [`flash`]'s 0x270
/// frame places them.
pub fn flash_from(fx: &mut Fx, src: u32, g: u32, ot: u32, n: i32) -> i32 {
    flash_body(fx, src, g, ot, n, &mut None)
}

fn flash_body(fx: &mut Fx, src: u32, g: u32, ot: u32, n: i32, a3: &mut Option<u32>) -> i32 {
    let sp = fx.enter(0x270);
    for k in (0..16).step_by(8) {
        let v = fx.mem.read_u64((src & !15) + k);
        fx.mem.write_u64(((sp + 0x180) & !15) + k, v);
    }
    let v = fx.mem.read_u64(src + 0x10);
    fx.mem.write_u64(sp + 0x190, v);
    let (kind, id) = (fx.mem.i32(sp + 0x180), fx.mem.u8(sp + 0x184));
    let tex = fx.texture(kind, id);
    fx.mem.write_u32(sp + 0xE0, tex);
    let mut g = g;
    let mut i = 0;
    fx.mem.set32(sp + 0xD0, 0);
    while fx.mem.i32(sp + 0xD0) < n {
        if i != 0 {
            g += 0x4C;
        }
        one(fx, sp, g, ot, a3);
        i += 1;
        fx.mem.set32(sp + 0xD0, i);
    }
    fx.leave(0x270);
    1
}

fn cvt_u16(v: i32) -> f32 {
    ee::from_i32(v)
}

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v) as i32;
    fx.mem.set16(a, x);
}

fn one(fx: &mut Fx, sp: u32, g: u32, ot: u32, a3: &mut Option<u32>) {
    if countdown(fx, g) == 0 {
        return;
    }
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(g + 0x1C + 4 * k), fx.mem.f(g + 0x2C + 4 * k));
        fx.mem.setf(g + 0x1C + 4 * k, v);
    }
    let v = ee::add(fx.mem.f(g + 0x10), fx.mem.f(g + 0x14));
    fx.mem.setf(g + 0x10, v);
    let local: [u32; 4] = std::array::from_fn(|k| fx.mem.read_u32(g + 0x3C + 4 * k as u32));
    for (k, w) in local.into_iter().enumerate() {
        fx.mem.write_u32(sp + 0x170 + 4 * k as u32, w);
    }
    let mut spokes = fx.mem.u8(g + 7);
    if spokes >= 10 {
        spokes = 9;
    }
    let step = ee::div(90.0, ee::from_i32(spokes));
    node_source(fx, g, g + 0x1C);
    camera_vp_into(fx, sp + 0x130);
    translation_into(fx, sp + 0xF0, g + 0x1C);
    mul_mem(fx, sp + 0x130, sp + 0xF0);
    let m = fx.read_m4(sp + 0x130);
    *a3 = Some(sp + 0x26C);
    if !fx.project_at(sp + 0x170, &m, sp + 0x1A0, sp + 0x26C) {
        return;
    }
    let xyz = fx.mem.read_u64(sp + 0x1A0);
    if !Fx::on_screen(xyz) {
        return;
    }
    let (size, w) = (fx.mem.f(g + 0x10), fx.mem.f(sp + 0x26C));
    let r = fx.view_scale(size, w);
    let (size, w) = (fx.mem.f(g + 0x18), fx.mem.f(sp + 0x26C));
    let r2 = fx.view_scale(size, w);
    let depth = if fx.mem.u8(g + 4) & 0x80 != 0 {
        fx.set_xyz_z(sp + 0x1A0, 0xFF_FFFE);
        0.0
    } else {
        fx.mem.f(sp + 0x26C)
    };
    // The fan triangle (RGBAQ / XYZF2 x 3) at sp + 0x1A8.
    let a = sp + 0x1A8;
    fx.desc_init(a, 6, 0x13, 0x41_4141);
    let b8 = |fx: &mut Fx, o: u32| fx.mem.read_u8(g + o);
    let v = b8(fx, 0xE);
    fx.mem.write_u8(sp + 0x1B8, v);
    let v = b8(fx, 0xA);
    fx.mem.write_u8(sp + 0x1D8, v);
    fx.mem.write_u8(sp + 0x1C8, v);
    let v = b8(fx, 0xD);
    fx.mem.write_u8(sp + 0x1B9, v);
    let v = b8(fx, 9);
    fx.mem.write_u8(sp + 0x1D9, v);
    fx.mem.write_u8(sp + 0x1C9, v);
    let v = b8(fx, 0xC);
    fx.mem.write_u8(sp + 0x1BA, v);
    let v = b8(fx, 8);
    fx.mem.write_u8(sp + 0x1DA, v);
    fx.mem.write_u8(sp + 0x1CA, v);
    for o in [0x1DCu32, 0x1CC, 0x1BC] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let alpha = if fx.mem.u8(g + 4) & 8 != 0 { 0x42 } else { 0x48 };
    fx.desc_alpha(a, alpha);
    // The spike quad (RGBAQ, then UV / XYZF2 x 4) at sp + 0x1F0.
    let b = sp + 0x1F0;
    fx.desc_init(b, 9, 0x24, 0x4_2424_2421);
    let v = b8(fx, 0xA);
    fx.mem.write_u8(sp + 0x200, v);
    let v = b8(fx, 9);
    fx.mem.write_u8(sp + 0x201, v);
    let v = b8(fx, 8);
    fx.mem.write_u8(sp + 0x202, v);
    fx.mem.write_u32(sp + 0x204, 0x3F80_0000);
    let tex = fx.mem.read_u32(sp + 0xE0);
    *a3 = Some(tex_rect_a3(fx, b, sp + 0x180, tex));
    let (ca, sa) = (sp + 0x1BB, sp + 0x203);
    let v = b8(fx, 0xF);
    fx.mem.write_u8(ca, v);
    let v = fx.mem.u8(g + 0xF) >> 1;
    fx.mem.set8(sa, v);
    fx.mem.write_u8(sp + 0x1DB, 0);
    fx.mem.write_u8(sp + 0x1CB, 0);
    if fx.mem.u8(g + 4) & 0x40 == 0 {
        let count = fx.mem.u8(g);
        let k = if count < 8 { Some(count) } else { Some((fx.mem.u8(g + 2) - count) & 0xFF).filter(|&k| k < 8) };
        if let Some(k) = k {
            let a = fx.mem.u8(g + 0xF) >> 3;
            let v = fx.mult(a, k);
            fx.mem.set8(ca, v);
            let k2 = if count < 8 { fx.mem.u8(g) } else { k };
            let v = (fx.mem.u8(g + 0xF) >> 4).wrapping_mul(k2);
            fx.mem.set8(sa, v);
        }
    }
    if fx.mem.u8(g) & 1 != 0 {
        if fx.mem.u8(g + 4) & 2 != 0 {
            let v = fx.mem.u8(ca);
            if v >= 0x11 {
                fx.mem.set8(ca, v - 0x10);
            }
        }
        if fx.mem.u8(g + 4) & 4 != 0 {
            let v = fx.mem.u8(ca);
            if v > 0 {
                fx.mem.set8(ca, v >> 1);
            }
        }
    }
    let c0 = libm_cosf(fx, 0.0);
    let mut x1 = ee::mul(r, c0);
    let s0 = fx.libm_sinf(0.0);
    let mut y1 = ee::mul(f32::from_bits(ASPECT), ee::mul(r, s0));
    // Every vertex at the centre's Z (down the chain), top byte 1.
    let mut z = fx.xyz_z(sp + 0x1A0);
    for o in [0x1E0u32, 0x1D0, 0x1C0, 0x240, 0x230, 0x220, 0x210] {
        fx.set_xyz_z(sp + o, z);
        z = fx.xyz_z(sp + o);
    }
    for o in [0x1E7u32, 0x1D7, 0x1C7, 0x247, 0x237, 0x227, 0x217] {
        fx.mem.write_u8(sp + o, 1);
    }
    let mut s = 1;
    while s < 4 * spokes + 1 {
        let (x0, y0) = (x1, y1);
        let ang = ee::div(ee::mul(f32::from_bits(PI), ee::mul(step, ee::from_i32(s))), 180.0);
        x1 = ee::mul(r, libm_cosf(fx, ang));
        let sn = fx.libm_sinf(ang);
        y1 = ee::mul(f32::from_bits(ASPECT), ee::mul(r, sn));
        let cx = fx.mem.u16(sp + 0x1A0);
        fx.mem.set16(sp + 0x1C0, cx);
        let cy = fx.mem.u16(sp + 0x1A2);
        fx.mem.set16(sp + 0x1C2, cy);
        fix16(fx, sp + 0x1D0, ee::add(cvt_u16(cx), x0));
        fix16(fx, sp + 0x1D2, ee::add(cvt_u16(cy), y0));
        fix16(fx, sp + 0x1E0, ee::add(cvt_u16(cx), x1));
        fix16(fx, sp + 0x1E2, ee::add(cvt_u16(cy), y1));
        let key = ee::fixunssfsi(depth) as i32;
        fx.submit(a, ot, key);
        if fx.mem.u8(g + 4) & 1 == 0 {
            let cx = fx.mem.u16(sp + 0x1A0);
            let v = ee::fixunssfsi(ee::add(cvt_u16(cx), x0)) as i32;
            fx.mem.set16(sp + 0x230, v);
            fx.mem.set16(sp + 0x210, v);
            let v = ee::fixunssfsi(ee::add(cvt_u16(cx), x1)) as i32;
            fx.mem.set16(sp + 0x240, v);
            fx.mem.set16(sp + 0x220, v);
            let cy = fx.mem.u16(sp + 0x1A2);
            let t = ee::add(cvt_u16(cy), y0);
            fix16(fx, sp + 0x212, ee::sub(t, r2));
            fix16(fx, sp + 0x232, ee::add(r2, t));
            let t = ee::add(cvt_u16(cy), y1);
            fix16(fx, sp + 0x222, ee::sub(t, r2));
            fix16(fx, sp + 0x242, ee::add(r2, t));
            let key = ee::fixunssfsi(depth) as i32;
            fx.submit(b, ot, key);
            let cy = fx.mem.u16(sp + 0x1A2);
            let v = ee::fixunssfsi(ee::add(cvt_u16(cy), y0)) as i32;
            fx.mem.set16(sp + 0x232, v);
            fx.mem.set16(sp + 0x212, v);
            let v = ee::fixunssfsi(ee::add(cvt_u16(cy), y1)) as i32;
            fx.mem.set16(sp + 0x242, v);
            fx.mem.set16(sp + 0x222, v);
            let cx = fx.mem.u16(sp + 0x1A0);
            let t = ee::add(cvt_u16(cx), x0);
            fix16(fx, sp + 0x210, ee::add(r2, t));
            fix16(fx, sp + 0x230, ee::sub(t, r2));
            let t = ee::add(cvt_u16(cx), x1);
            fix16(fx, sp + 0x220, ee::add(r2, t));
            fix16(fx, sp + 0x240, ee::sub(t, r2));
            let key = ee::fixunssfsi(depth) as i32;
            fx.submit(b, ot, key);
        }
        s += 1;
    }
}
