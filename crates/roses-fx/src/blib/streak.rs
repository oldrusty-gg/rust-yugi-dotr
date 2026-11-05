//! The ribbon of the effect library: textured segments from a record's
//! origin along a wobbling or table-driven path.
use super::exe::{aim_angles, atlas_cell, camera_vp_into, countdown, cvt_u32, libm_cosf, mul_mem, node_source, rot_trans_into, tex_rect_strip};
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx};

/// 255.0, 17.0, 360.0, 180.0 and pi as the code loads them.
const PI: u32 = 0x4049_0FDB;

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v) as i32;
    fx.mem.set16(a, x);
}

/// 303bacc0e521 [73 B records; b098 0x1E00FC0, 740 words] [F]:
/// `ribbon(g, ot, n) -> 1` steps and draws `n` ribbon records (0x68
/// bytes from `g`; locals of a 0x260 frame at their stack addresses).
/// Texture: flag 4 an own-slot texture (0x202530(5, +0xA)) with blend
/// +0xB and atlas cell +0xC of layout 0 (0x1020B0), else the record's
/// sprite spec (record-relative, `lui` / `addiu` at +0x48 / +0x4C);
/// without flag 2 the spec's u1 / v1 lose 16. Per record: countdown
/// (0x101AF0), origin +0x28 += +0x38, angles +0x48 += +0x58 (i32), the
/// origin from a node (0x101BA0), with flag 8 the angles aimed at the
/// self unit's node +0xE (0x1F65B0, 0x1F6830, 0x101940); the matrix
/// camera x (angles, origin) (0x20E2B0, 0x20C0B0, 0x20B370); colours
/// +0x20..+0x27 (alphas faded over the first 8 counts unless flag 0x40);
/// then +8 segments, each from the projected origin to the projected
/// point along the path (flag 1: deltas of the table at *(+0x10) six bytes
/// a step, scaled by +0x14 / 255; else a spiral stepping 17 degrees with
/// radius table x +0x18 / 255 and length +0x14 / 2 (1 + table / 255)),
/// scaled by the length left (+2 - max(count, +0xD)), widened by
/// 0x1018E0(+0x1C) across its major axis; flag 0x80 at the far Z with key
/// 0, else the nearer depth.
pub fn ribbon(fx: &mut Fx, at: u32, g: u32, ot: u32, n: i32) -> i32 {
    let src = rel(fx, at, 0x48, 0x4C);
    ribbon_from(fx, src, g, ot, n)
}

/// The ribbon with its sprite spec at `src`.
pub fn ribbon_from(fx: &mut Fx, src: u32, g: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x260);
    fx.mem.write_u32(sp + 0xC8, ot);
    fx.mem.set32(sp + 0xC4, n);
    copy18(fx, src, sp + 0x190);
    let mut g = g;
    let mut i = 0;
    fx.mem.set32(sp + 0xB0, 0);
    while fx.mem.i32(sp + 0xB0) < n {
        if i != 0 {
            g += 0x68;
        }
        one(fx, sp, g);
        i += 1;
        fx.mem.set32(sp + 0xB0, i);
    }
    fx.leave(0x260);
    1
}

/// lq / ld, sq / sd of a 0x18-byte record.
fn copy18(fx: &mut Fx, src: u32, dst: u32) {
    for k in (0..16).step_by(8) {
        let v = fx.mem.read_u64((src & !15) + k);
        fx.mem.write_u64((dst & !15) + k, v);
    }
    let v = fx.mem.read_u64(src + 0x10);
    fx.mem.write_u64(dst + 0x10, v);
}

fn one(fx: &mut Fx, sp: u32, g: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let tex = if fx.mem.u8(g + 4) & 4 != 0 {
        let id = fx.mem.u8(g + 0xA);
        let t = fx.texture(5, id);
        let b = fx.mem.read_u8(g + 0xB);
        fx.mem.write_u8(sp + 0x1B5, b);
        let cell = fx.mem.u8(g + 0xC);
        atlas_cell(fx, sp + 0x1B0, cell, 0);
        t
    } else {
        let (kind, id) = (fx.mem.i32(sp + 0x190), fx.mem.u8(sp + 0x194));
        let t = fx.texture(kind, id);
        copy18(fx, sp + 0x190, sp + 0x1B0);
        t
    };
    if fx.mem.u8(g + 4) & 2 == 0 {
        for o in [0x1BCu32, 0x1C4] {
            let v = fx.mem.i32(sp + o).wrapping_sub(0x10);
            fx.mem.set32(sp + o, v);
        }
    }
    let mut step = fx.mem.u8(g + 9);
    let mut turn = 0.0f32;
    let (count, hold) = (fx.mem.u8(g), fx.mem.u8(g + 0xD));
    let left = if count < hold { fx.mem.u8(g + 2) - hold } else { fx.mem.u8(g + 2) - count };
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(g + 0x28 + 4 * k), fx.mem.f(g + 0x38 + 4 * k));
        fx.mem.setf(g + 0x28 + 4 * k, v);
    }
    for k in 0..3u32 {
        let v = fx.mem.i32(g + 0x48 + 4 * k).wrapping_add(fx.mem.i32(g + 0x58 + 4 * k));
        fx.mem.set32(g + 0x48 + 4 * k, v);
    }
    node_source(fx, g, g + 0x28);
    if fx.mem.u8(g + 4) & 8 != 0 {
        let h = fx.self_handle();
        let slot = fx.mem.u8(g + 0xE);
        fx.node_position(h, slot, sp + 0x180);
        for k in 0..3u32 {
            let v = ee::sub(fx.mem.f(sp + 0x180 + 4 * k), fx.mem.f(g + 0x28 + 4 * k));
            fx.mem.setf(sp + 0x170 + 4 * k, v);
        }
        aim_angles(fx, sp + 0x170, g + 0x48);
    }
    camera_vp_into(fx, sp + 0x110);
    rot_trans_into(fx, sp + 0xD0, g + 0x48, g + 0x28);
    mul_mem(fx, sp + 0x110, sp + 0xD0);
    let d = sp + 0x1C8;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    for (src, dsts) in [(0x22u32, [0x1F0u32, 0x1D8]), (0x26, [0x220, 0x208]), (0x21, [0x1F1, 0x1D9]), (0x25, [0x221, 0x209]), (0x20, [0x1F2, 0x1DA]), (0x24, [0x222, 0x20A])] {
        let v = fx.mem.read_u8(g + src);
        for o in dsts {
            fx.mem.write_u8(sp + o, v);
        }
    }
    for o in [0x224u32, 0x20C, 0x1F4, 0x1DC] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let c = fx.mem.u8(g);
    if c < 8 && fx.mem.u8(g + 4) & 0x40 == 0 {
        let a = fx.mem.u8(g + 0x23) >> 3;
        let v = fx.mult(a, c);
        for o in [0x20Bu32, 0x1F3, 0x1DB] {
            fx.mem.set8(sp + o, v);
        }
        let v = (fx.mem.u8(g + 0x27) >> 3).wrapping_mul(fx.mem.u8(g));
        fx.mem.set8(sp + 0x223, v);
    } else {
        let v = fx.mem.read_u8(g + 0x23);
        for o in [0x20Bu32, 0x1F3, 0x1DB] {
            fx.mem.write_u8(sp + o, v);
        }
        let v = fx.mem.read_u8(g + 0x27);
        fx.mem.write_u8(sp + 0x223, v);
    }
    tex_rect_strip(fx, d, sp + 0x1B0, tex);
    for o in [0x168u32, 0x164, 0x160] {
        fx.mem.write_u32(sp + o, 0);
    }
    let m = fx.read_m4(sp + 0x110);
    if !fx.project_at(sp + 0x160, &m, sp + 0x158, sp + 0x25C) {
        return;
    }
    let xyz = fx.mem.read_u64(sp + 0x158);
    if !Fx::on_screen(xyz) {
        return;
    }
    let mut s = 0;
    while s < fx.mem.u8(g + 8) {
        let (px, py, pz);
        if fx.mem.u8(g + 4) & 1 != 0 {
            let t = fx.mem.read_u32(g + 0x10).wrapping_add((step & 0xFF) as u32);
            let k = fx.mem.f(g + 0x14);
            let d = |fx: &mut Fx, a: u32, b: u32| fx.mem.u8(t + a) - fx.mem.u8(t + b);
            let d01 = d(fx, 0, 1);
            px = ee::div(ee::mul(k, ee::from_i32(d01)), 255.0);
            let d32 = d(fx, 2, 3);
            py = ee::div(ee::mul(k, ee::from_i32(d32)), 255.0);
            let d45 = d(fx, 4, 5);
            pz = ee::div(ee::mul(k, ee::from_i32(d45)), 255.0);
            step = (step + 6) & 0xFF;
        } else {
            turn = ee::add(turn, 17.0);
            if !ee_le(turn, 360.0) {
                turn = ee::sub(turn, 360.0);
            }
            let t = fx.mem.read_u32(g + 0x10).wrapping_add((step & 0xFF) as u32);
            let r = cvt_u32(fx.mem.read_u8(t) as u32);
            let rad = ee::div(ee::mul(fx.mem.f(g + 0x18), r), 255.0);
            let ang = ee::div(ee::mul(f32::from_bits(PI), turn), 180.0);
            px = ee::mul(rad, libm_cosf(fx, ang));
            let sn = fx.libm_sinf(ang);
            pz = ee::mul(rad, sn);
            let half = ee::div(fx.mem.f(g + 0x14), 2.0);
            let t = fx.mem.read_u32(g + 0x10).wrapping_add(((step + 1) & 0xFF) as u32);
            let u = cvt_u32(fx.mem.read_u8(t) as u32);
            py = ee::add(half, ee::mul(half, ee::div(u, 255.0)));
            step = (step + 2) & 0xFF;
        }
        let l = ee::from_i32(left);
        fx.mem.setf(sp + 0x160, ee::mul(px, l));
        fx.mem.setf(sp + 0x164, ee::mul(py, l));
        fx.mem.setf(sp + 0x168, ee::mul(pz, l));
        let m = fx.read_m4(sp + 0x110);
        if fx.project_at(sp + 0x160, &m, sp + 0x150, sp + 0x258) && Fx::on_screen(fx.mem.read_u64(sp + 0x150)) {
            segment(fx, sp, g);
        }
        s += 1;
    }
}

fn segment(fx: &mut Fx, sp: u32, g: u32) {
    let (size, w) = (fx.mem.f(g + 0x1C), fx.mem.f(sp + 0x258));
    let half = fx.view_scale(size, w);
    let depth = if fx.mem.u8(g + 4) & 0x80 != 0 {
        let mut z = 0xFF_FFFE;
        for o in [0x230u32, 0x218, 0x200, 0x1E8] {
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
        }
        0.0
    } else {
        let (w1, w0) = (fx.mem.f(sp + 0x258), fx.mem.f(sp + 0x25C));
        let depth = if ee_lt(w1, w0) { w1 } else { w0 };
        let mut z = fx.xyz_z(sp + 0x150);
        for o in [0x200u32, 0x1E8] {
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
        }
        let mut z = fx.xyz_z(sp + 0x158);
        for o in [0x230u32, 0x218] {
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
        }
        depth
    };
    for o in [0x237u32, 0x21F, 0x207, 0x1EF] {
        fx.mem.write_u8(sp + o, 0);
    }
    let (y0, y1) = (fx.mem.u16(sp + 0x15A), fx.mem.u16(sp + 0x152));
    let dy = (y0 - y1).abs();
    let (x0, x1) = (fx.mem.u16(sp + 0x158), fx.mem.u16(sp + 0x150));
    let dx = (x0 - x1).abs();
    let f = |v: i32| ee::from_i32(v);
    if dy < dx {
        fx.mem.set16(sp + 0x1EA, y1);
        fix16(fx, sp + 0x202, ee::sub(f(y1), half));
        fix16(fx, sp + 0x21A, ee::add(f(y1), half));
        fx.mem.set16(sp + 0x232, y0);
        if x1 < x0 {
            fix16(fx, sp + 0x1E8, ee::sub(f(x1), half));
        } else {
            fix16(fx, sp + 0x1E8, ee::add(f(x1), half));
        }
        fx.mem.set16(sp + 0x218, x1);
        fx.mem.set16(sp + 0x200, x1);
        fx.mem.set16(sp + 0x230, x0);
    } else {
        fx.mem.set16(sp + 0x1E8, x1);
        fix16(fx, sp + 0x200, ee::sub(f(x1), half));
        fix16(fx, sp + 0x218, ee::add(f(x1), half));
        fx.mem.set16(sp + 0x230, x0);
        if y1 < y0 {
            fix16(fx, sp + 0x1EA, ee::sub(f(y1), half));
        } else {
            fix16(fx, sp + 0x1EA, ee::add(f(y1), half));
        }
        fx.mem.set16(sp + 0x21A, y1);
        fx.mem.set16(sp + 0x202, y1);
        fx.mem.set16(sp + 0x232, y0);
    }
    let key = ee::fixunssfsi(depth) as i32;
    let ot = fx.mem.read_u32(sp + 0xC8);
    fx.submit(sp + 0x1C8, ot, key);
}
