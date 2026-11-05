//! Sprite clouds round a task, hand-ported from
//! the records' code [F]: camera-facing textured quads scattered by a
//! table of random bytes, as [`quads4::ring_into`](super::quads4::ring_into)
//! with other stack layouts and per-quad UV orientations.
use crate::bus::Bus;
use crate::c061::{blend_alpha, desc_uv4, rgbaq_at, scale_tenth};
use crate::ee;
use crate::exe::Fx;
use crate::fxlib2 as l2;

/// The camera's negated yaw in degrees at `a` (the 0xB60B60B7 `mult`
/// through LO / HI, as the original's).
fn yaw_degrees_neg(fx: &mut Fx, a: u32) {
    let v = fx.mem.i32(a);
    fx.mult(0xB60B_60B7u32 as i32, v);
    let hi = fx.hilo().1;
    let q = (hi.wrapping_add(v) >> 7).wrapping_add(((v as u32) >> 31) as i32);
    fx.mem.set32(a, q.wrapping_neg());
}

/// 03b2505be7b0 [2 B records: 1 / 225; card 225 0x1E01F20, 260 words]:
/// `cloud(D, task, desc, table)`: D+0xB8 textured quads (9 values, FST,
/// blend 2, vertex alphas 0) built in `desc`, coloured +0x28, texture
/// D+0x88 (0x207E50 filter 1), each with its UVs from D+0x94 in the
/// orientation i16 D+0xC0 + 2 k (uv_rect4), of size i16 +0x38 turned by
/// the camera's negated yaw (D+0xD8 the turn's table), around the i16
/// position +0 / +2 / +4 (scale 0.1), pushed out per axis by the bytes
/// `table` [k + i16 +8 (+0, +0x64, +0xB4)] / 20 times the radius i16
/// +0x30 + k / 2; keyed by the mean w on D+0x80. A 0x150 frame (RGBAQ sp
/// + 0x148, angles sp + 0x130, matrix sp + 0xF0, mean w sp + 0xE0, scale
/// sp + 0xD0, position sp + 0xC0, corners sp + 0x80).
pub fn cloud(fx: &mut Fx, d0: u32, t: u32, d: u32, table: u32) {
    let sp = fx.enter(0x150);
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha(2));
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, sp + 0x148, t + 0x28, d + 0x10);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    scale_tenth(fx, sp + 0xD0);
    l2::camera_angles_to(fx, sp + 0x130);
    l2::camera_angles_to(fx, sp + 0x130);
    yaw_degrees_neg(fx, sp + 0x134);
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xC0 + 4 * k, v);
    }
    l2::xform_vp(fx, sp + 0xF0, sp + 0xD0, 0, sp + 0xC0);
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let o = fx.mem.i16(d0 + 0xC0 + 2 * k as u32);
        super::uv::uv_rect4(fx, d, d0 + 0x94, o);
        let size = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x80, size, size);
        l2::rotate4(fx, sp + 0x80, sp + 0x130, 0, d0 + 0xD8);
        let row = table.wrapping_add(k as u32);
        let half = k >> 1;
        for (axis, off) in [(0, 0u32), (1, 0x64), (2, 0xB4)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::div(ee::from_i32(fx.mem.i8(row.wrapping_add(i as u32).wrapping_add(off))), 20.0);
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + half);
            l2::add_column(fx, sp + 0x80, axis, ee::mul(b, r));
        }
        if l2::project(fx, sp + 0x80, 0x7000_0000, sp + 0xF0, d, sp + 0xE0) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0xE0)) as i32;
            let ot = fx.mem.read_u32(d0 + 0x80);
            fx.submit(d, ot, key);
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x150);
}
