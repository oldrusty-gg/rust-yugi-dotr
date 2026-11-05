//! More second-library sprite routines compiled into the BMODULE records
//!: c061's 0x1E00560 / 0x1E01100 with the data
//! area `D` as the first argument [F word-compare], and the records' own
//! ring variants
//! of [`billboard::ring_facing`](crate::blib::billboard::ring_facing).
//! Arguments as the original's (a0 = `D`, a1 = the task, a2 = the mode,
//! a3 = a position or table); locals stay at the original's stack
//! addresses below `fx.sp`.
use crate::blib::billboard::Uv;
use crate::bus::Bus;
use crate::c061::{blend_alpha, desc_uv4, rgbaq_at, scale_tenth, to_degrees_neg};
use crate::ee;
use crate::exe::Fx;
use crate::fxlib2 as l2;

/// The scratchpad, where the originals project.
const SPR: u32 = 0x7000_0000;

/// 792ca1ad9378 [33 B records; b448 0x1E00E90, 220 words]:
/// `sprite_quads(D, task, mode, pos)`: four textured quads (strip3, FST)
/// of the task's size (f32 +0x30 / +0x34 as integers, 0x109640's four
/// layouts) turned by its angle +0x5A about z, billboarded at `pos` (scale
/// 0.05), coloured by (mode >> 4) 0: +0x64 x3 / +0x60, 1: +0x64 x2 /
/// +0x60 x2 (else the header's colours stay); UVs from D+0x94 in D+0xBE's
/// orientation, texture D+0x88; each visible quad at its far Z, keyed by
/// pos.w on D+0x80 (c061's 0x1E00560 with D an argument) [F].
pub fn sprite_quads(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x210);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x70 + 4 * k, w);
    }
    let d = sp + 0x140;
    l2::desc_strip3(fx, d);
    match mode >> 4 {
        0 => {
            rgbaq_at(fx, sp + 0x1D0, t + 0x64, d + 0x10);
            rgbaq_at(fx, sp + 0x1D8, t + 0x64, d + 0x28);
            rgbaq_at(fx, sp + 0x1E0, t + 0x64, d + 0x40);
            rgbaq_at(fx, sp + 0x1E8, t + 0x60, d + 0x58);
        }
        1 => {
            rgbaq_at(fx, sp + 0x1F0, t + 0x64, d + 0x10);
            rgbaq_at(fx, sp + 0x1F8, t + 0x64, d + 0x28);
            rgbaq_at(fx, sp + 0x200, t + 0x60, d + 0x40);
            rgbaq_at(fx, sp + 0x208, t + 0x60, d + 0x58);
        }
        _ => {}
    }
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xD0 + 4 * k, 0x3D4C_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x130);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x70 + 4 * k);
        fx.mem.write_u32(sp + 0xC0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xF0, sp + 0xD0, sp + 0x130, sp + 0xC0);
    let a = fx.mem.i16(t + 0x5A);
    let depth = fx.mem.f(sp + 0x7C);
    fx.mem.set32(sp + 0x138, a);
    fx.mem.set32(sp + 0x134, 0);
    fx.mem.set32(sp + 0x130, 0);
    for k in 0..4 {
        let w = ee::fixsfsi(fx.mem.f(t + 0x30));
        let h = ee::fixsfsi(fx.mem.f(t + 0x34));
        l2::quad_ints(fx, sp + 0x80, w, h, k);
        l2::rotate4(fx, sp + 0x80, sp + 0x130, 2, d0 + 0xD8);
        if l2::project(fx, sp + 0x80, SPR, sp + 0xF0, d, sp + 0xE0) {
            l2::far_z(fx, &[d + 0x68, d + 0x50, d + 0x38, d + 0x20]);
            let key = ee::fixunssfsi(depth) as i32;
            let ot = fx.mem.read_u32(d0 + 0x80);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x210);
}

/// 07c172c43e56 [12 B records; b538 0x1E004D0, 352 words]:
/// `ring_facing_i(D, task, depth, table)`: [`billboard::ring_facing`]'s
/// ring for an older task layout: D+0xB8 textured quads (9 values, FST,
/// blend 1) of size i16 +0x38 facing the camera (negated degrees) around
/// the i16 position +0 / +2 / +4 (scale 0.1), each pushed out by the bytes
/// `table` [k + i16 +8 (+0, +0x64, +0xB4)] / 20 times the radius i16 +0x30
/// + k / 2 (no turn about y), coloured +0x28; UVs from D+0x94 in the
/// orientation D+0xC0 + 2k per quad, texture D+0x88; depth 0 the mean w,
/// 1 the far Z at key 0, 2 key -1, else nothing drawn. Its 0x206E40 (the
/// ordering table, unused) has no effect [F].
///
/// [`billboard::ring_facing`]: crate::blib::billboard::ring_facing
pub fn ring_facing_i(fx: &mut Fx, uv: Uv, d0: u32, t: u32, depth: i32, table: u32) {
    ring_i(fx, uv, d0, t, depth, table, (0x38, 0x38), blend_alpha(1), true);
}

/// cb18430f13b0 [2 B records; b068 0x1E00920, 356 words]:
/// [`ring_facing_i`] with quads of width i16 +0xC and height i16 +0xA [F].
pub fn ring_facing_wh(fx: &mut Fx, uv: Uv, d0: u32, t: u32, depth: i32, table: u32) {
    ring_i(fx, uv, d0, t, depth, table, (0xC, 0xA), blend_alpha(1), true);
}

/// 1b8a350ccf03 [3 B records; b537 0x1E00EA0, 324 words]:
/// `ring_facing_m(D, task, mode, table)`: [`ring_facing_i`] taking a mode
/// (blend (mode >> 4) as i16, depth mode & 15) and one UV orientation
/// D+0xC0 for every quad; a compiled twin (0x1E0 frame, its locals at
/// ring_facing_i's absolute addresses) [F word-compare].
pub fn ring_facing_m(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    ring_i(fx, uv, d0, t, mode & 0xF, table, (0x38, 0x38), blend_alpha((mode >> 4) as i16 as i32), false);
}

/// The body of [`ring_facing_i`] / [`ring_facing_wh`] / [`ring_facing_m`]:
/// `size` the task's i16 width / height offsets, `per_quad` the UV
/// orientation D+0xC0 + 2k per quad (else D+0xC0 for all).
#[allow(clippy::too_many_arguments)]
fn ring_i(fx: &mut Fx, uv: Uv, d0: u32, t: u32, depth: i32, table: u32, size: (u32, u32), blend: u64, per_quad: bool) {
    let sp = fx.enter(0x1F0);
    let d = sp + 0x158;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend);
    rgbaq_at(fx, sp + 0x1E8, t + 0x28, d + 0x10);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xE0 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x140);
    to_degrees_neg(fx, sp + 0x144);
    to_degrees_neg(fx, sp + 0x140);
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xD0 + 4 * k, v);
    }
    l2::xform_vp(fx, sp + 0x100, sp + 0xE0, 0, sp + 0xD0);
    let tab = d0 + 0xD8;
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let order = fx.mem.i16(d0 + 0xC0 + if per_quad { 2 * k as u32 } else { 0 });
        uv(fx, d, d0 + 0x94, order);
        let (w, h) = (ee::from_i32(fx.mem.i16(t + size.0)), ee::from_i32(fx.mem.i16(t + size.1)));
        l2::quad_xy(fx, sp + 0x90, w, h);
        l2::rotate4(fx, sp + 0x90, sp + 0x140, 1, tab);
        l2::rotate4(fx, sp + 0x90, sp + 0x140, 0, tab);
        let row = table.wrapping_add(k as u32);
        let half = k >> 1;
        for (axis, off) in [(0u32, 0u32), (4, 0x64), (8, 0xB4)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::div(ee::from_i32(fx.mem.i8(row.wrapping_add(i as u32).wrapping_add(off))), 20.0);
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + half);
            let p = ee::mul(b, r);
            for c in [0x90u32, 0xA0, 0xB0, 0xC0] {
                let v = ee::add(fx.mem.f(sp + c + axis), p);
                fx.mem.setf(sp + c + axis, v);
            }
        }
        if (0..3).contains(&depth) && l2::project(fx, sp + 0x90, SPR, sp + 0x100, d, sp + 0xF0) {
            let ot = fx.mem.read_u32(d0 + 0x80);
            match depth {
                0 => {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0xF0)) as i32;
                    fx.submit(d, ot, key);
                }
                1 => {
                    l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                    fx.submit(d, ot, 0);
                }
                _ => fx.submit(d, ot, -1),
            }
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x1F0);
}

/// 21cb40d76bec [37 B records; b044 0x1E01520, 256 words]:
/// `sparks(D, task, mode, pos)`: c061's 0x1E01100 with D an argument (and
/// a 0x220 frame, its locals 0x10 higher): D+0xB8 sparks (strip3, FST,
/// coloured +0x60 inner / +0x64 outer) on a circle at `pos` (scale 0.1)
/// turned by +0x20: inner radius +0x48, outer +0x4C, spark k from angle
/// (k + 1) x +0x5A to that + +0x5C degrees (i16 steps); UVs from D+0x94 in
/// D+0xBE's orientation, texture D+0x88; blend mode >> 4, depth mode & 15
/// (0 the mean w, 1 the far Z at key 0, else nothing drawn) [F].
pub fn sparks(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x220);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    let span = fx.mem.i16(t + 0x5C);
    let d = sp + 0x170;
    l2::colour_strip4(fx, d, t + 0x60, 2);
    l2::colour_strip4(fx, d, t + 0x64, 1);
    let blend = (mode >> 4) as i16 as i32;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 0);
    fx.desc_alpha(d, blend_alpha(blend));
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    rgbaq_at(fx, sp + 0x200, t + 0x60, d + 0x10);
    rgbaq_at(fx, sp + 0x208, t + 0x64, d + 0x40);
    rgbaq_at(fx, sp + 0x210, t + 0x60, d + 0x28);
    rgbaq_at(fx, sp + 0x218, t + 0x64, d + 0x58);
    scale_tenth(fx, sp + 0x110);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0xB0 + 4 * k);
        fx.mem.write_u32(sp + 0x100 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x110, t + 0x20, sp + 0x100);
    for o in [0xC4u32, 0xD4, 0xE4, 0xF4] {
        fx.mem.set32(sp + o, 0);
    }
    let (inner, outer) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let depth = mode & 0xF;
    let tab = d0 + 0xD8;
    let mut a = 0i32;
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let start = (fx.mem.i16(t + 0x5A) + a) as i16 as i32;
        let end = (a + fx.mem.i16(t + 0x5A) + span) as i16 as i32;
        for (angle, at) in [(start, 0xC0u32), (end, 0xD0)] {
            for (r, o) in [(inner, 0u32), (outer, 0x20)] {
                let c = l2::sin_deg(fx, tab, angle, 1);
                fx.mem.setf(sp + at + o, ee::mul(r, c));
                let s = l2::sin_deg(fx, tab, angle, 0);
                fx.mem.setf(sp + at + o + 8, ee::mul(r, s));
            }
        }
        if depth <= 1 && l2::project(fx, sp + 0xC0, SPR, sp + 0x120, d, sp + 0x160) {
            let ot = fx.mem.read_u32(d0 + 0x80);
            if depth == 0 {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
                fx.submit(d, ot, key);
            } else {
                l2::far_z_desc(fx, d, 1);
                fx.submit(d, ot, 0);
            }
        }
        k += 1;
        a = (a + fx.mem.i16(t + 0x5A)) as i16 as i32;
    }
    fx.leave(0x220);
}

/// e18ba1c23957 [5 B records; b718 0x1E00DE0, 224 words]:
/// `ring_into(D, task, desc, table)`: D+0xB8 (read once) camera-facing
/// textured quads (9 values, FST, blend 2, vertex alphas 0) built in the
/// caller's descriptor `desc`, of size i16 +0x38 turned by the camera's
/// negated yaw only, around the i16 position +0 / +2 / +4 plus the i16
/// +8 / +0xA / +0xC / 10 (scale 0.1), each pushed out per axis by the bytes
/// `table` [k + i16 +8 (+0, +0x64, +0xB4)] / 20 times the radius i16 +0x30
/// + k / 2; coloured +0x28, UVs from D+0x94 (orientation 0), texture
/// D+0x88; keyed by the mean w. Its 0x206E40 has no effect; 0x207E50 is
/// called with the UV setter's a0 (the descriptor) [F].
pub fn ring_into(fx: &mut Fx, uv: Uv, d0: u32, t: u32, d: u32, table: u32) {
    let sp = fx.enter(0x160);
    let count = fx.mem.i16(d0 + 0xB8);
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha(2));
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, sp + 0x158, t + 0x28, d + 0x10);
    uv(fx, d, d0 + 0x94, 0);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xE0 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x140);
    to_degrees_neg(fx, sp + 0x144);
    for k in 0..3 {
        let off = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + 2 * k)), off);
        fx.mem.setf(sp + 0xD0 + 4 * k, v);
    }
    l2::xform_vp(fx, sp + 0x100, sp + 0xE0, 0, sp + 0xD0);
    let tab = d0 + 0xD8;
    let mut k = 0i32;
    while k < count {
        let size = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x90, size, size);
        l2::rotate4(fx, sp + 0x90, sp + 0x140, 0, tab);
        let row = table.wrapping_add(k as u32);
        let half = k >> 1;
        for (axis, off) in [(0, 0u32), (1, 0x64), (2, 0xB4)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::div(ee::from_i32(fx.mem.i8(row.wrapping_add(i as u32).wrapping_add(off))), 20.0);
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + half);
            l2::add_column(fx, sp + 0x90, axis, ee::mul(b, r));
        }
        if l2::project(fx, sp + 0x90, SPR, sp + 0x100, d, sp + 0xF0) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0xF0)) as i32;
            let ot = fx.mem.read_u32(d0 + 0x80);
            fx.submit(d, ot, key);
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x160);
}

/// 2d29e1f9b4be [5 B records; b552 0x1E00920, 240 words]:
/// `ring_at(D, task, table, pos)`: D+0xB8 (0: 20) camera-facing textured
/// quads (9 values, FST, blend 2, vertex alphas 0) of size i16 +0x38
/// (turned by the camera's negated yaw and pitch) at `pos` (scale 0.1),
/// each pushed out by the bytes `table` [k + i16 +8 (+0, +0x64, +0xB4)] /
/// 20 times the radius i16 +0x30 + k / 2, that offset turned about z by
/// i16 +0x3A degrees; coloured +0x28, UVs from D+0x94 in D+0xBE's
/// orientation, texture D+0x88 (0x207E50 with the UV setter's a0, the
/// descriptor); keyed by the mean w. Its 0x206E40 has no effect [F].
pub fn ring_at(fx: &mut Fx, uv: Uv, d0: u32, t: u32, table: u32, pos: u32) {
    let sp = fx.enter(0x220);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x90 + 4 * k, w);
    }
    let d = sp + 0x188;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha(2));
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, sp + 0x218, t + 0x28, d + 0x10);
    let order = fx.mem.i16(d0 + 0xBE);
    uv(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    let count = match fx.mem.i16(d0 + 0xB8) {
        0 => 0x14,
        n => n,
    };
    for k in 0..3 {
        fx.mem.write_u32(sp + 0x100 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x160);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x90 + 4 * k);
        fx.mem.write_u32(sp + 0xF0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x100, 0, sp + 0xF0);
    to_degrees_neg(fx, sp + 0x164);
    to_degrees_neg(fx, sp + 0x160);
    let a = fx.mem.i16(t + 0x3A);
    fx.mem.set32(sp + 0x178, a);
    let tab = d0 + 0xD8;
    let mut k = 0i32;
    while k < count {
        let size = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0xA0, size, size);
        l2::rotate4(fx, sp + 0xA0, sp + 0x160, 0, tab);
        l2::rotate4(fx, sp + 0xA0, sp + 0x160, 1, tab);
        let row = table.wrapping_add(k as u32);
        let half = k >> 1;
        for (o, off) in [(0u32, 0u32), (4, 0x64), (8, 0xB4)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::div(ee::from_i32(fx.mem.i8(row.wrapping_add(i as u32).wrapping_add(off))), 20.0);
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + half);
            fx.mem.setf(sp + 0xE0 + o, ee::mul(b, r));
        }
        l2::rotate_deg(fx, sp + 0xE0, sp + 0x170, 2, tab);
        for (axis, o) in [(0, 0u32), (1, 4), (2, 8)] {
            let f = fx.mem.f(sp + 0xE0 + o);
            l2::add_column(fx, sp + 0xA0, axis, f);
        }
        if l2::project(fx, sp + 0xA0, SPR, sp + 0x120, d, sp + 0x110) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x110)) as i32;
            let ot = fx.mem.read_u32(d0 + 0x80);
            fx.submit(d, ot, key);
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x220);
}

/// b0ec19f6c988 [5 B records; b041 0x1E00CF0, 184 words]:
/// `quad_turned(D, task, mode, pos)`: [`billboard::quad`] turned about z
/// by i16 +0x5A degrees: one textured quad (9 values, FST) of the task's
/// size (i16 +8 / +0xA) turned by its angles +0x20, at `pos` (scale 0.1),
/// coloured +0x60, UVs from D+0x94 in D+0xBE's orientation, texture
/// D+0x88 (0x207E50 with the UV setter's a0, the descriptor); blend
/// (mode >> 4) & 15, depth mode & 15 (0 the mean w, 1 the far Z at key 0,
/// else key -1) [F].
///
/// [`billboard::quad`]: crate::blib::billboard::quad
pub fn quad_turned(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    quad_turned_by(fx, uv, d0, t, mode, pos, false, false);
}

/// 32e1959038be [4 B records; b363 0x1E01D30, 208 words]:
/// [`quad_turned`] with the corners raised by i16 +0xA + float +0x34 in y
/// and turned about z by +0x5C, then about y by +0x5A (angles (+0x5C,
/// +0x5A, +0x5C) at sp + 0x110, two 0x1099E0 calls, axes 2 / 0) [F
/// word-compare: the only differences].
pub fn quad_turned_raised(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    quad_turned_by(fx, uv, d0, t, mode, pos, false, true);
}

/// 0a017ff6bc9f [2 B records; b400 0x1E00ED0, 180 words]:
/// [`quad_turned`] of the float size +0x30 / +0x34 (not i16 +8 / +0xA);
/// its UV setter's `jal` at +0x100 too [F word-compare].
pub fn quad_turned_f(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    quad_turned_by(fx, uv, d0, t, mode, pos, true, false);
}

#[allow(clippy::too_many_arguments)]
fn quad_turned_by(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32, float_size: bool, raised: bool) {
    let sp = fx.enter(0x1C0);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x50 + 4 * k, w);
    }
    let d = sp + 0x128;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) & 0xF));
    rgbaq_at(fx, sp + 0x1B8, t + 0x60, d + 0x10);
    let order = fx.mem.i16(d0 + 0xBE);
    uv(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x50 + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
        fx.mem.write_u32(sp + 0xA0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, t + 0x20, sp + 0xA0);
    let (w, h) = if float_size { (fx.mem.f(t + 0x30), fx.mem.f(t + 0x34)) } else { (ee::from_i32(fx.mem.i16(t + 8)), ee::from_i32(fx.mem.i16(t + 0xA))) };
    l2::quad_xy(fx, sp + 0x60, w, h);
    if raised {
        for k in 0..4u32 {
            let lift = ee::add(ee::from_i32(fx.mem.i16(t + 0xA)), fx.mem.f(t + 0x34));
            let y = sp + 0x64 + 0x10 * k;
            let v = ee::add(fx.mem.f(y), lift);
            fx.mem.setf(y, v);
        }
        for (o, a) in [(0x110u32, 0x5Cu32), (0x114, 0x5A), (0x118, 0x5C)] {
            let v = fx.mem.i16(t + a);
            fx.mem.set32(sp + o, v);
        }
        l2::rotate4(fx, sp + 0x60, sp + 0x110, 2, d0 + 0xD8);
        l2::rotate4(fx, sp + 0x60, sp + 0x110, 0, d0 + 0xD8);
    } else {
        let a = fx.mem.i16(t + 0x5A);
        fx.mem.set32(sp + 0x118, a);
        l2::rotate4(fx, sp + 0x60, sp + 0x110, 2, d0 + 0xD8);
    }
    if l2::project(fx, sp + 0x60, SPR, sp + 0xD0, d, sp + 0xC0) {
        let ot = fx.mem.read_u32(d0 + 0x80);
        match mode & 0xF {
            0 => {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0xC0)) as i32;
                fx.submit(d, ot, key);
            }
            1 => {
                l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                fx.submit(d, ot, 0);
            }
            _ => fx.submit(d, ot, -1),
        }
    }
    fx.leave(0x1C0);
}

/// 9dae9a5cb92b [6 B records; b426 0x1E00D70, 364 words]:
/// [`billboard::ring_quads`] with layout 0's offset turned about z (not y)
/// by +0x5A degrees [F word-compare: the angle's stack slot and 0x109230's
/// axis differ].
///
/// [`billboard::ring_quads`]: crate::blib::billboard::ring_quads
pub fn ring_quads_z(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    crate::blib::billboard::ring_quads_about(fx, uv, d0, t, mode, table, 2);
}

/// 383a1b86ead2 [15 B records; b503 0x1E00CE0, 344 words]:
/// `ring_at_m(D, task, mode, table)`: [`ring_at`] with a mode, at D+0x20:
/// D+0xB8 camera-facing textured quads (9 values, FST) of size i16 +0x38
/// (turned by the camera's negated pitch and yaw, axis 1 then 0) at
/// D+0x20 (scale 0.1), each pushed out by the bytes `table` [k + i16 +8
/// (+0, +0x64, +0xB4)] / 20 times the radius i16 +0x30 + k / 2, the xy of
/// that offset turned about z by i16 +0x3A degrees (0x109230, before the z
/// is computed); coloured +0x28, UVs from D+0x94 in D+0xC0's orientation
/// (the record's setter, every quad), texture D+0x88; blend (mode >> 4)
/// as i16, depth mode & 15 (0 the mean w, 1 the far Z at key 0, 2 key -1,
/// else nothing drawn). Its 0x206E40 has no effect [F].
pub fn ring_at_m(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    ring_at_m_from(fx, uv, d0, t, mode, table, false, false);
}

/// 737e393f7a6a [4 B records; b572 0x1E00920, 352 words]:
/// [`ring_at_m`] centred on the task's i16 position +0 / +2 / +4 (as
/// floats) instead of D+0x20; its UV setter's `jal` at +0x1F4 [F
/// word-compare: only the three centre loads differ].
pub fn ring_at_mt(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    ring_at_m_from(fx, uv, d0, t, mode, table, true, false);
}

/// 404333042047 [3 B records; b116 (card 274) 0x1E00C80, 344 words]
///: [`ring_at_m`] with its UV setter called once, before the
/// quad loop (even with no quads; its `jal` at +0x1C4) [F fdiff: the
/// loop's entry block sits after the setter's call instead of before it,
/// the only difference].
pub fn ring_at_m_uv1(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    ring_at_m_from(fx, uv, d0, t, mode, table, false, true);
}

/// [`ring_at_m`]'s body; `at_task`: centred on the task ([`ring_at_mt`]);
/// `uv_once`: the UV setter called once before the loop ([`ring_at_m_uv1`]).
#[allow(clippy::too_many_arguments)]
fn ring_at_m_from(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32, at_task: bool, uv_once: bool) {
    let sp = fx.enter(0x220);
    let d = sp + 0x188;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    rgbaq_at(fx, sp + 0x218, t + 0x28, d + 0x10);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    scale_tenth(fx, sp + 0x100);
    l2::camera_angles_to(fx, sp + 0x160);
    to_degrees_neg(fx, sp + 0x164);
    to_degrees_neg(fx, sp + 0x160);
    let a = fx.mem.i16(t + 0x3A);
    fx.mem.set32(sp + 0x178, a);
    for k in 0..3 {
        if at_task {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0xF0 + 4 * k, v);
        } else {
            let w = fx.mem.read_u32(d0 + 0x20 + 4 * k);
            fx.mem.write_u32(sp + 0xF0 + 4 * k, w);
        }
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x100, 0, sp + 0xF0);
    let depth = mode & 0xF;
    let tab = d0 + 0xD8;
    if uv_once {
        let order = fx.mem.i16(d0 + 0xC0);
        uv(fx, d, d0 + 0x94, order);
    }
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        if !uv_once {
            let order = fx.mem.i16(d0 + 0xC0);
            uv(fx, d, d0 + 0x94, order);
        }
        let size = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0xA0, size, size);
        l2::rotate4(fx, sp + 0xA0, sp + 0x160, 1, tab);
        l2::rotate4(fx, sp + 0xA0, sp + 0x160, 0, tab);
        let row = table.wrapping_add(k as u32);
        let half = k >> 1;
        let push = |fx: &mut Fx, off: u32| {
            let i = fx.mem.i16(t + 8);
            let b = ee::div(ee::from_i32(fx.mem.i8(row.wrapping_add(i as u32).wrapping_add(off))), 20.0);
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + half);
            ee::mul(b, r)
        };
        let x = push(fx, 0);
        fx.mem.setf(sp + 0xE0, x);
        let y = push(fx, 0x64);
        fx.mem.setf(sp + 0xE4, y);
        l2::rotate_deg(fx, sp + 0xE0, sp + 0x170, 2, tab);
        for (axis, o) in [(0, 0u32), (1, 4)] {
            let f = fx.mem.f(sp + 0xE0 + o);
            l2::add_column(fx, sp + 0xA0, axis, f);
        }
        let z = push(fx, 0xB4);
        fx.mem.setf(sp + 0xE8, z);
        l2::add_column(fx, sp + 0xA0, 2, z);
        if depth <= 2 && l2::project(fx, sp + 0xA0, SPR, sp + 0x120, d, sp + 0x110) {
            let ot = fx.mem.read_u32(d0 + 0x80);
            match depth {
                0 => {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x110)) as i32;
                    fx.submit(d, ot, key);
                }
                1 => {
                    l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                    fx.submit(d, ot, 0);
                }
                _ => fx.submit(d, ot, -1),
            }
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x220);
}

/// 9c13e08cfd2f [27 B records; b533 0x1E01110, 340 words]:
/// [`billboard::ring_facing`] without the quads' turn about z by +0x5C;
/// its UV setter's `jal` at +0x1C4 [F word-compare: the +0x5C store and
/// the axis-2 0x1099E0 call are missing].
///
/// [`billboard::ring_facing`]: crate::blib::billboard::ring_facing
pub fn ring_facing_flat(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    crate::blib::billboard::ring_facing_turned(fx, uv, d0, t, mode, table, false, 0);
}

/// 2f5a28a746a2 [19 B records; b051 0x1E009B0, 340 words]:
/// [`ring_facing_flat`] with the push offsets turned by +0x5A about z (the
/// angle stored at sp + 0x178, 0x109230 axis 2; sp + 0x174 left as it
/// was) [F word-compare: only that store and the axis differ]; UV setter
/// `jal` at +0x1C4.
pub fn ring_facing_flat_z(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    crate::blib::billboard::ring_facing_turned(fx, uv, d0, t, mode, table, false, 2);
}

/// 561a6131a635 [32 B records; b533 0x1E00F40, 116 words]:
/// `tint4(D, task, mode, w)`: a full-window gouraud quad (0x109BC0 /
/// 0x109DD0) coloured +0x60 (top) / +0x64 (bottom) (0x108270 kinds 1, 2);
/// blend (mode >> 4) as i16; depth mode & 15: 0 the far Z at key 0, else
/// key `w` (0x1A2C70). A 0xF0 frame, the descriptor at sp + 0x60 [F].
pub fn tint4(fx: &mut Fx, t: u32, mode: i32, w: f32) {
    let sp = fx.enter(0xF0);
    let d = sp + 0x60;
    l2::colour_strip4(fx, d, t + 0x60, 1);
    l2::colour_strip4(fx, d, t + 0x64, 2);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    if mode & 0xF == 0 {
        l2::far_z_desc(fx, d, 1);
        fx.submit(d, crate::exe::OT, 0);
    } else {
        let key = ee::fixunssfsi(w) as i32;
        fx.submit(d, crate::exe::OT, key);
    }
    fx.leave(0xF0);
}

/// 3a2db45c5a9a [11 B records; b751 0x1E00F40, 260 words]:
/// `arc_strips_tex(D, task, mode, pos)`: [`billboard::arc_strips`] as
/// textured strips (a strip3, 0x109B50 / 0x108070, UVs from D+0x94 in
/// D+0xBE's orientation by 0x108DE0, texture D+0x88; RGBAQ +0x60 inner /
/// +0x64 outer), the inner radius +0x48 at height +0x34 and the outer +0x4C
/// at height 0 (swapped); a 0x220 frame [F word-compare].
///
/// [`billboard::arc_strips`]: crate::blib::billboard::arc_strips
pub fn arc_strips_tex(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x220);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    let span = fx.mem.i16(t + 0x5C);
    let d = sp + 0x170;
    l2::colour_strip4(fx, d, t + 0x60, 2);
    l2::colour_strip4(fx, d, t + 0x64, 1);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 0);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for (local, c, to) in [(0x200u32, 0x60u32, 0x10u32), (0x208, 0x64, 0x40), (0x210, 0x60, 0x28), (0x218, 0x64, 0x58)] {
        rgbaq_at(fx, sp + local, t + c, d + to);
    }
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0xB0 + 4 * k);
        fx.mem.write_u32(sp + 0x110 + 4 * k, 0x3DCC_CCCD);
        fx.mem.write_u32(sp + 0x100 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x110, t + 0x20, sp + 0x100);
    let y = fx.mem.f(t + 0x34);
    fx.mem.setf(sp + 0xC4, y);
    let y = fx.mem.f(t + 0x34);
    fx.mem.setf(sp + 0xD4, y);
    fx.mem.set32(sp + 0xE4, 0);
    fx.mem.set32(sp + 0xF4, 0);
    let (inner, outer) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let depth = mode & 0xF;
    let tab = d0 + 0xD8;
    let mut a = 0i32;
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let s4 = a as i16 as i32;
        let s = (fx.mem.i16(t + 0x5A) + s4) as i16 as i32;
        let e = (s4 + (fx.mem.i16(t + 0x5A) + span)) as i16 as i32;
        for (deg, r, x, z) in [(s, inner, 0xC0u32, 0xC8u32), (s, outer, 0xE0, 0xE8), (e, inner, 0xD0, 0xD8), (e, outer, 0xF0, 0xF8)] {
            let c = l2::sin_deg(fx, tab, deg, 1);
            fx.mem.setf(sp + x, ee::mul(r, c));
            let sn = l2::sin_deg(fx, tab, deg, 0);
            fx.mem.setf(sp + z, ee::mul(r, sn));
        }
        if depth <= 1 && l2::project(fx, sp + 0xC0, SPR, sp + 0x120, d, sp + 0x160) {
            let ot = fx.mem.read_u32(d0 + 0x80);
            if depth == 0 {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
                fx.submit(d, ot, key);
            } else {
                l2::far_z_desc(fx, d, 1);
                fx.submit(d, ot, 0);
            }
        }
        k += 1;
        a = (a + fx.mem.i16(t + 0x5A)) as i16 as i32;
    }
    fx.leave(0x220);
}

/// 3fb2be4df9d4 [18 B records; b615 0x1E00920, 280 words]:
/// `ring_band(D, task, mode, pos)`: a closed ring of 36 untextured gouraud
/// strips (10 degrees each from +0x5A) at `pos` turned by the task's
/// angles +0x20 (scale 0.1), inner radius +0x48, outer i16 +6, all at
/// height 0; colours +0x60 (inner) / +0x64 (outer) as
/// [`billboard::arc_strips`]; blend (mode >> 4) as i16, depth mode & 15 (0
/// the mean w, 1 the far Z at key 0, else key -1). Each strip after the
/// first reuses the previous one's end points (the same values) [F].
///
/// [`billboard::arc_strips`]: crate::blib::billboard::arc_strips
pub fn ring_band(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    ring_band_by(fx, d0, t, mode, pos, BandTask { colours: (0x60, 0x64), angles: 0x20, inner: Radius::F32(0x48), outer: Radius::I16(6), angle: 0x5A, height: None, step: 0xA, keyless: true });
}

/// 2f3eecc0b281 [13 B records; b491 0x1E00920, 276 words]:
/// [`ring_band`] with the outer radius the float +0x4C (in place of i16
/// +6) [F word-compare: only that load differs].
pub fn ring_band_f(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    ring_band_by(fx, d0, t, mode, pos, BandTask { colours: (0x60, 0x64), angles: 0x20, inner: Radius::F32(0x48), outer: Radius::F32(0x4C), angle: 0x5A, height: None, step: 0xA, keyless: true });
}

/// 853ed233d9de [7 B records; b219 (card 69) 0x1E00920, 280 words] (helper
/// A14): [`ring_band_f`] with the inner points at the task's height +0x34
/// (in place of 0; the outer ones stay at 0) [F word-compare: only those
/// two stores differ].
pub fn ring_band_fy(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    ring_band_by(fx, d0, t, mode, pos, BandTask { colours: (0x60, 0x64), angles: 0x20, inner: Radius::F32(0x48), outer: Radius::F32(0x4C), angle: 0x5A, height: Some(0x34), step: 0xA, keyless: true });
}

/// d058fcddf114 [b708 (card 352) 0x1E02420, 280 words]:
/// [`ring_band_fy`] with 18 strips of 20 degrees and no key -1 path (depth
/// modes above 1 project and submit nothing); only the first strip's start
/// is turned by +0x5A, every end point is at 20 (k + 1) degrees [F
/// word-compare; run].
pub fn ring_band_fy20(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    ring_band_by(fx, d0, t, mode, pos, BandTask { colours: (0x60, 0x64), angles: 0x20, inner: Radius::F32(0x48), outer: Radius::F32(0x4C), angle: 0x5A, height: Some(0x34), step: 0x14, keyless: false });
}

/// ad71810de1d8 [2 B records; b258 0x1E010A0, 284 words]:
/// [`ring_band`] for the 0x40-byte tasks: colours +0x28 (inner) / +0x2C
/// (outer), turned by the angles +0x10, inner radius i16 +0x38, strips from
/// i16 +0x3A [F].
pub fn ring_band_t(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    ring_band_by(fx, d0, t, mode, pos, BandTask { colours: (0x28, 0x2C), angles: 0x10, inner: Radius::I16(0x38), outer: Radius::I16(6), angle: 0x3A, height: None, step: 0xA, keyless: true });
}

/// 865250076f2e [cards 7 / 25 0x1E00DE0, 284 words]:
/// [`ring_band_t`] with 18 strips of 20 degrees [F fdiff: the four step
/// immediates 0xA -> 0x14 and the loop bound 0x24 -> 0x12 are the only
/// differences].
pub fn ring_band_t20(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    ring_band_by(fx, d0, t, mode, pos, BandTask { colours: (0x28, 0x2C), angles: 0x10, inner: Radius::I16(0x38), outer: Radius::I16(6), angle: 0x3A, height: None, step: 0x14, keyless: true });
}

/// Where a ring band's task keeps its inner radius.
#[derive(Clone, Copy)]
enum Radius {
    F32(u32),
    I16(u32),
}

/// A ring band's task layout.
#[derive(Clone, Copy)]
struct BandTask {
    colours: (u32, u32),
    angles: u32,
    inner: Radius,
    outer: Radius,
    angle: u32,
    /// The inner points' height (+4 of sp + 0xC0 / 0xD0) from this task
    /// offset (else 0).
    height: Option<u32>,
    /// Degrees a strip (360 / step strips).
    step: i32,
    /// Depth modes above 1 submit at key -1 and the end points are turned
    /// by the start angle (else: nothing is projected, and only the first
    /// start point is turned).
    keyless: bool,
}

fn ring_band_by(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32, o: BandTask) {
    let sp = fx.enter(0x200);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    let d = sp + 0x170;
    l2::colour_strip4(fx, d, t + o.colours.0, 2);
    l2::colour_strip4(fx, d, t + o.colours.1, 1);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0xB0 + 4 * k);
        fx.mem.write_u32(sp + 0x110 + 4 * k, 0x3DCC_CCCD);
        fx.mem.write_u32(sp + 0x100 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x110, t + o.angles, sp + 0x100);
    for o in [0xC4u32, 0xD4, 0xE4, 0xF4] {
        fx.mem.set32(sp + o, 0);
    }
    if let Some(h) = o.height {
        let y = fx.mem.read_u32(t + h);
        fx.mem.write_u32(sp + 0xC4, y);
        fx.mem.write_u32(sp + 0xD4, y);
    }
    let radius = |fx: &mut Fx, r: Radius| match r {
        Radius::F32(at) => fx.mem.f(t + at),
        Radius::I16(at) => ee::from_i32(fx.mem.i16(t + at)),
    };
    let inner = radius(fx, o.inner);
    let outer = radius(fx, o.outer);
    let depth = mode & 0xF;
    let tab = d0 + 0xD8;
    let mut a = 0i32;
    for k in 0..360 / o.step {
        if k == 0 {
            let s = (fx.mem.i16(t + o.angle) + a) as i16 as i32;
            for (r, x, z) in [(inner, 0xC0u32, 0xC8u32), (outer, 0xE0, 0xE8)] {
                let c = l2::sin_deg(fx, tab, s, 1);
                fx.mem.setf(sp + x, ee::mul(r, c));
                let sn = l2::sin_deg(fx, tab, s, 0);
                fx.mem.setf(sp + z, ee::mul(r, sn));
            }
        } else {
            // The previous strip's end points (vec4s) become the start.
            for (to, from) in [(0xC0u32, 0xD0u32), (0xE0, 0xF0)] {
                for c in 0..4 {
                    let w = fx.mem.read_u32(sp + from + 4 * c);
                    fx.mem.write_u32(sp + to + 4 * c, w);
                }
            }
        }
        // ring_band_fy20's end points leave out the start angle.
        let from = if o.keyless { fx.mem.i16(t + o.angle) } else { 0 };
        let e = (a + (from + o.step)) as i16 as i32;
        for (r, x, z) in [(inner, 0xD0u32, 0xD8u32), (outer, 0xF0, 0xF8)] {
            let c = l2::sin_deg(fx, tab, e, 1);
            fx.mem.setf(sp + x, ee::mul(r, c));
            let sn = l2::sin_deg(fx, tab, e, 0);
            fx.mem.setf(sp + z, ee::mul(r, sn));
        }
        if (o.keyless || depth <= 1) && l2::project(fx, sp + 0xC0, SPR, sp + 0x120, d, sp + 0x160) {
            let ot = fx.mem.read_u32(d0 + 0x80);
            match depth {
                0 => {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
                    fx.submit(d, ot, key);
                }
                1 => {
                    l2::far_z_desc(fx, d, 1);
                    fx.submit(d, ot, 0);
                }
                _ => fx.submit(d, ot, -1),
            }
        }
        a = (a + o.step) as i16 as i32;
    }
    fx.leave(0x200);
}

/// 3f36e61a971c [3 B records; b646 0x1E016C0, 216 words]:
/// [`billboard::arc_strips`] starting at the task's angle +0x52 (the step
/// still +0x5A) [F word-compare].
///
/// [`billboard::arc_strips`]: crate::blib::billboard::arc_strips
pub fn arc_strips_52(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    crate::blib::billboard::arc_strips_from(fx, d0, t, mode, pos, 0x52);
}

/// 7f395ce1b4cc [5 B records; b573 0x1E01120, 296 words]:
/// `band18(D, task, depth, pos)`: a closed ring of 18 untextured gouraud
/// strips (20 degrees each from i16 +0x3A) at `pos`, facing the camera
/// (0x205580's angles, x + 0x4000; scale 0.1), inner radius i16 +0x38,
/// outer i16 +6, height 0; colours +0x28 (inner) / +0x2C (outer) through
/// 0x108270, additive. `depth` is the whole a2: 0 keys by the mean w, 1
/// the far Z at key 0, else key -1 (each strip after the first reuses
/// the previous one's end points) [F].
pub fn band18(fx: &mut Fx, d0: u32, t: u32, depth: i32, pos: u32) {
    band18_by(fx, d0, t, depth, pos, true);
}

/// 05dcbcde46f4 [4 B records; b582 0x1E01400, 260 words]:
/// [`band18`] turned by the task's angles +0x10 (scale 0.1) instead of
/// facing the camera; its locals 0x10 higher [F].
pub fn band18_turned(fx: &mut Fx, d0: u32, t: u32, depth: i32, pos: u32) {
    band18_by(fx, d0, t, depth, pos, false);
}

fn band18_by(fx: &mut Fx, d0: u32, t: u32, depth: i32, pos: u32, facing: bool) {
    let sp = fx.enter(0x200);
    // Locals: the position, the vertices (inner start, inner end, outer
    // start, outer end), the scale, the translation, the matrix, the mean w.
    let o = if facing { 0 } else { 0x10 };
    let (at, v, scale, trans, out, mean) = (sp + 0xA0 + o, sp + 0xB0 + o, sp + 0x100 + o, sp + 0xF0 + o, sp + 0x110 + o, sp + 0x150 + o);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(at + 4 * k, w);
    }
    let d = sp + 0x170;
    l2::colour_strip4(fx, d, t + 0x28, 2);
    l2::colour_strip4(fx, d, t + 0x2C, 1);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    for k in 0..3 {
        fx.mem.write_u32(scale + 4 * k, 0x3DCC_CCCD);
    }
    let rot = if facing {
        l2::camera_angles_to(fx, sp + 0x160);
        let x = fx.mem.i32(sp + 0x160).wrapping_add(0x4000);
        fx.mem.set32(sp + 0x160, x);
        sp + 0x160
    } else {
        t + 0x10
    };
    for k in 0..3 {
        let w = fx.mem.read_u32(at + 4 * k);
        fx.mem.write_u32(trans + 4 * k, w);
    }
    l2::xform_vp(fx, out, scale, rot, trans);
    for k in 0..4 {
        fx.mem.set32(v + 0x10 * k + 4, 0);
    }
    let tab = d0 + 0xD8;
    let mut a = 0i32;
    for k in 0..0x12 {
        // (deg, radius offset, vertex)
        let mut pts: Vec<(i32, u32, u32)> = Vec::with_capacity(4);
        if k == 0 {
            let s = (fx.mem.i16(t + 0x3A) + a) as i16 as i32;
            pts.push((s, 0x38, v));
            pts.push((s, 6, v + 0x20));
        } else {
            for (to, from) in [(v, v + 0x10), (v + 0x20, v + 0x30)] {
                for c in 0..4 {
                    let w = fx.mem.read_u32(from + 4 * c);
                    fx.mem.write_u32(to + 4 * c, w);
                }
            }
        }
        let e = (a + (fx.mem.i16(t + 0x3A) + 0x14)) as i16 as i32;
        pts.push((e, 0x38, v + 0x10));
        pts.push((e, 6, v + 0x30));
        for (deg, r, x) in pts {
            let c = l2::sin_deg(fx, tab, deg, 1);
            let rf = ee::from_i32(fx.mem.i16(t + r));
            fx.mem.setf(x, ee::mul(rf, c));
            let sn = l2::sin_deg(fx, tab, deg, 0);
            let rf = ee::from_i32(fx.mem.i16(t + r));
            fx.mem.setf(x + 8, ee::mul(rf, sn));
        }
        if l2::project(fx, v, SPR, out, d, mean) {
            let ot = fx.mem.read_u32(d0 + 0x80);
            match depth {
                0 => {
                    let key = ee::fixunssfsi(fx.mem.f(mean)) as i32;
                    fx.submit(d, ot, key);
                }
                1 => {
                    l2::far_z_desc(fx, d, 1);
                    fx.submit(d, ot, 0);
                }
                _ => fx.submit(d, ot, -1),
            }
        }
        a = (a + 0x14) as i16 as i32;
    }
    fx.leave(0x200);
}

/// a6d0a94b7c36 [4 B records; b573 0x1E00920, 168 words]:
/// `quarters_far(D, task, desc, pos)`: a textured quad of half extents
/// (i16 +8, i16 +0xA) drawn as its four quarters (0x109640 modes 0..3)
/// into the caller's strip3 `desc`, turned about z by i16 +0x3A, at `pos`
/// facing the camera (0x205580's angles; scale 0.05), each quarter at the
/// far Z, key 0. Colours: vertices 0..2 +0x2C, vertex 3 +0x28; UVs D+0x94
/// in orientation i16 D+0xBE, texture D+0x88 [F].
pub fn quarters_far(fx: &mut Fx, d0: u32, t: u32, d: u32, pos: u32) {
    quarters_far_by(fx, d0, t, d, Some(pos));
}

/// 09eeeb9282c1 [3 B records; b073 0x1E03BD0, 168 words]:
/// [`quarters_far`] at the data area's point D+0x20 + 0x10 i16 +0x30
/// (xyz) instead of `pos`; frame 0x130 (locals 0x10 lower) [F].
pub fn quarters_far_at(fx: &mut Fx, d0: u32, t: u32, d: u32) {
    quarters_far_by(fx, d0, t, d, None);
}

fn quarters_far_by(fx: &mut Fx, d0: u32, t: u32, d: u32, pos: Option<u32>) {
    let frame = if pos.is_some() { 0x140 } else { 0x130 };
    let sp = fx.enter(frame);
    let o = if pos.is_some() { 0x10 } else { 0 };
    let (quad, mean, scale, out, rot, trans) = (sp + 0x50 + o, sp + 0xB0 + o, sp + 0xA0 + o, sp + 0xC0 + o, sp + 0x100 + o, sp + 0x90 + o);
    if let Some(pos) = pos {
        for k in 0..4 {
            let w = fx.mem.read_u32(pos + 4 * k);
            fx.mem.write_u32(sp + 0x50 + 4 * k, w);
        }
    }
    l2::desc_strip3(fx, d);
    for (local, c, to) in [(0x110u32, 0x2Cu32, 0x10u32), (0x118, 0x2C, 0x28), (0x120, 0x2C, 0x40), (0x128, 0x28, 0x58)] {
        rgbaq_at(fx, sp + local + o, t + c, d + to);
    }
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(scale + 4 * k, 0x3D4C_CCCD);
    }
    l2::camera_angles_to(fx, rot);
    let src = match pos {
        Some(_) => sp + 0x50,
        None => (d0 as i32 + 0x20 + 0x10 * fx.mem.i16(t + 0x30)) as u32,
    };
    for k in 0..3 {
        let w = fx.mem.read_u32(src + 4 * k);
        fx.mem.write_u32(trans + 4 * k, w);
    }
    l2::xform_vp(fx, out, scale, rot, trans);
    let z = fx.mem.i16(t + 0x3A);
    fx.mem.set32(rot + 8, z);
    fx.mem.set32(rot + 4, 0);
    fx.mem.set32(rot, 0);
    for q in 0..4 {
        let (a, b) = (fx.mem.i16(t + 8), fx.mem.i16(t + 0xA));
        l2::quad_ints(fx, quad, a, b, q);
        l2::rotate4(fx, quad, rot, 2, d0 + 0xD8);
        if l2::project(fx, quad, SPR, out, d, mean) {
            l2::far_z_desc(fx, d, 0);
            let ot = fx.mem.read_u32(d0 + 0x80);
            fx.submit(d, ot, 0);
        }
    }
    fx.leave(frame);
}

/// 0cb01f69a41a [25 B records; b073 0x1E00F10, 164 words]:
/// `fan18(D, task, desc)`: a disc of 18 untextured gouraud strips into the
/// caller's `desc` (20 degrees each from i16 +0x3A, the centre twice, rim
/// radius i16 +0x38) at the task's i16 position +0 / +2 / +4 plus i16 +8 /
/// +0xA / +0xC over 10, facing the camera (x + 0x4000; scale 0.1); colours
/// +0x28 (centre) / +0x2C (rim); each keyed by its mean w [F].
pub fn fan18(fx: &mut Fx, d0: u32, t: u32, d: u32) {
    fan18_by(fx, d0, t, d, false, false);
}

/// 095f4ce60e44 [3 B records; b073 0x1E03970, 148 words]:
/// [`fan18`] at the data area's point D+0x20 + 0x10 i16 +0x30, each strip
/// at the far Z (still keyed by the mean w) [F].
pub fn fan18_far(fx: &mut Fx, d0: u32, t: u32, d: u32) {
    fan18_by(fx, d0, t, d, true, false);
}

/// 70e89a6b0666 [8 B records; b850 0x1E00150, 164 words]:
/// [`fan18`] with each strip at the far Z (0x109C60 kind 1) and key 0
/// [F fdiff vs 0cb01f69a41a: only the key differs].
pub fn fan18_far0(fx: &mut Fx, d0: u32, t: u32, d: u32) {
    fan18_by(fx, d0, t, d, false, true);
}

/// 1f87ac8741ac [2 B records; b361 0x1E02470, 128 words]:
/// [`fan18`] for the 0x70-byte tasks: colours +0x60 (centre) / +0x64
/// (rim), at the float position +0x10, from the i16 angle +0x5A, rim
/// radius the float +0x48 [F word-compare: only the task fields differ].
pub fn fan18_t(fx: &mut Fx, d0: u32, t: u32, d: u32) {
    let sp = fx.enter(0x140);
    l2::colour_strip4(fx, d, t + 0x60, 1);
    l2::colour_strip4(fx, d, t + 0x64, 2);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xD0 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x130);
    let x = fx.mem.i32(sp + 0x130).wrapping_add(0x4000);
    fx.mem.set32(sp + 0x130, x);
    for k in 0..3 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(sp + 0xC0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xD0, sp + 0x130, sp + 0xC0);
    for o in [0x84u32, 0x94, 0xA4, 0xB4, 0x80, 0x88, 0x90, 0x98] {
        fx.mem.set32(sp + o, 0);
    }
    let tab = d0 + 0xD8;
    let mut a = 0i32;
    for _ in 0..0x12 {
        let s = (fx.mem.i16(t + 0x5A) + a) as i16 as i32;
        let e = (a + (fx.mem.i16(t + 0x5A) + 0x14)) as i16 as i32;
        for (deg, v) in [(s, 0xA0u32), (e, 0xB0)] {
            let c = l2::sin_deg(fx, tab, deg, 1);
            let r = fx.mem.f(t + 0x48);
            fx.mem.setf(sp + v, ee::mul(r, c));
            let sn = l2::sin_deg(fx, tab, deg, 0);
            let r = fx.mem.f(t + 0x48);
            fx.mem.setf(sp + v + 8, ee::mul(r, sn));
        }
        if l2::project(fx, sp + 0x80, SPR, sp + 0xE0, d, sp + 0x120) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x120)) as i32;
            let ot = fx.mem.read_u32(d0 + 0x80);
            fx.submit(d, ot, key);
        }
        a = (a + 0x14) as i16 as i32;
    }
    fx.leave(0x140);
}

fn fan18_by(fx: &mut Fx, d0: u32, t: u32, d: u32, far: bool, key0: bool) {
    let sp = fx.enter(0x140);
    l2::colour_strip4(fx, d, t + 0x28, 1);
    l2::colour_strip4(fx, d, t + 0x2C, 2);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xD0 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x130);
    let x = fx.mem.i32(sp + 0x130).wrapping_add(0x4000);
    fx.mem.set32(sp + 0x130, x);
    if far {
        let p = (d0 as i32 + 0x20 + 0x10 * fx.mem.i16(t + 0x30)) as u32;
        for k in 0..3 {
            let w = fx.mem.read_u32(p + 4 * k);
            fx.mem.write_u32(sp + 0xC0 + 4 * k, w);
        }
    } else {
        for k in 0..3u32 {
            let off = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
            let v = ee::add(ee::from_i32(fx.mem.i16(t + 2 * k)), off);
            fx.mem.setf(sp + 0xC0 + 4 * k, v);
        }
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xD0, sp + 0x130, sp + 0xC0);
    for o in [0x84u32, 0x94, 0xA4, 0xB4, 0x80, 0x88, 0x90, 0x98] {
        fx.mem.set32(sp + o, 0);
    }
    let tab = d0 + 0xD8;
    let mut a = 0i32;
    for _ in 0..0x12 {
        let s = (fx.mem.i16(t + 0x3A) + a) as i16 as i32;
        let e = (a + (fx.mem.i16(t + 0x3A) + 0x14)) as i16 as i32;
        for (deg, v) in [(s, 0xA0u32), (e, 0xB0)] {
            let c = l2::sin_deg(fx, tab, deg, 1);
            let r = ee::from_i32(fx.mem.i16(t + 0x38));
            fx.mem.setf(sp + v, ee::mul(r, c));
            let sn = l2::sin_deg(fx, tab, deg, 0);
            let r = ee::from_i32(fx.mem.i16(t + 0x38));
            fx.mem.setf(sp + v + 8, ee::mul(r, sn));
        }
        if l2::project(fx, sp + 0x80, SPR, sp + 0xE0, d, sp + 0x120) {
            if far || key0 {
                l2::far_z_desc(fx, d, 1);
            }
            let key = if key0 { 0 } else { ee::fixunssfsi(fx.mem.f(sp + 0x120)) as i32 };
            let ot = fx.mem.read_u32(d0 + 0x80);
            fx.submit(d, ot, key);
        }
        a = (a + 0x14) as i16 as i32;
    }
    fx.leave(0x140);
}

/// 58c265bfc632 [13 B records; b064 0x1E00080, 120 words]:
/// `sprite_spec(ot, task, spec)`: one camera-facing textured strip3 quad
/// (half size i16 +0x38; scale none) at the task's i16 position plus i16
/// +8 / +0xA / +0xC over 10 (integer division), coloured +0x28, UVs of
/// sprite i16 spec+4 cell i16 +0x34, texture spec+0; spec+0xC 0: through
/// 0x109870 (keyed by the mean w), else at the far Z keyed by the mean w
/// (0x1A2C70) [F].
pub fn sprite_spec(fx: &mut Fx, ot: u32, t: u32, spec: u32) {
    let sp = fx.enter(0x1A0);
    let d = sp + 0x110;
    let id = fx.mem.i16(spec + 4);
    let sub = fx.mem.i16(t + 0x34);
    l2::rect(fx, sp + 0xA0, id, sub);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    l2::uv_strip3(fx, d, sp + 0xA0, 0);
    let tex = fx.mem.read_u32(spec);
    fx.desc_texture(d, tex, 1);
    l2::camera_angles_to(fx, sp + 0x100);
    for k in 0..3u32 {
        let off = fx.mem.i16(t + 8 + 2 * k);
        fx.mult(0x6666_6667, off);
        let hi = fx.hilo().1;
        let q = (hi >> 2).wrapping_add(((off as u32) >> 31) as i32);
        let v = fx.mem.i16(t + 2 * k).wrapping_add(q);
        fx.mem.setf(sp + 0x90 + 4 * k, ee::from_i32(v));
    }
    l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0x90);
    let s = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x50, s, s);
    if fx.mem.read_u32(spec + 0xC) == 0 {
        l2::project_submit(fx, sp + 0x50, d, sp + 0xC0, ot);
    } else if l2::project(fx, sp + 0x50, SPR, sp + 0xC0, d, sp + 0xB0) {
        l2::far_z_desc(fx, d, 0);
        let key = ee::fixunssfsi(fx.mem.f(sp + 0xB0)) as i32;
        fx.submit(d, ot, key);
    }
    fx.leave(0x1A0);
}

/// 5705ae2c0496 [4 B records; b064 0x1E00C10, 200 words]:
/// `band18_tex(D, task, desc)`: a ring of 18 textured strip3 quads into
/// the caller's `desc` (20 degrees each from i16 +0x3A; inner radius i16
/// +0x38 (vertices 2, 3), outer that + i16 +6 (vertices 0, 1)) at the
/// task's i16 position turned by its angles +0x10 (scale 1); colours +0x28
/// (strip3 `which` 1) / +0x2C (2), UVs D+0x94 in orientation i16 D+0xBE,
/// texture D+0x88; each visible quad submitted i16 D+0xB8 + 1 times at
/// its mean w's key on D+0x80 [F].
pub fn band18_tex(fx: &mut Fx, d0: u32, t: u32, d: u32) {
    band_tex_by(fx, d0, t, d, &BAND18);
}

/// 31a0a740716d [7 B records; b850 0x1E003E0, 192 words]:
/// [`band18_tex`] as 9 slivers of 2 degrees every 40, the outer radius
/// i16 +0x38 + 20 and one colour +0x28 for all four vertices (strip3
/// `which` 0) [F fdiff vs 5705ae2c0496: only these differ].
pub fn band9_tex(fx: &mut Fx, d0: u32, t: u32, d: u32) {
    band_tex_by(fx, d0, t, d, &BAND9);
}

/// A [`band18_tex`] compile: `n` quads `width` degrees wide every `every`,
/// the outer radius +0x38 + (`plus`, else i16 +6), colours +0x28 / +0x2C
/// (`two`) or +0x28 for all.
struct BandTex {
    n: i32,
    every: i32,
    width: i32,
    plus: Option<i32>,
    two: bool,
}

const BAND18: BandTex = BandTex { n: 0x12, every: 0x14, width: 0x14, plus: None, two: true };
const BAND9: BandTex = BandTex { n: 9, every: 0x28, width: 2, plus: Some(0x14), two: false };

fn band_tex_by(fx: &mut Fx, d0: u32, t: u32, d: u32, l: &BandTex) {
    let sp = fx.enter(0x130);
    l2::desc_strip3(fx, d);
    if l.two {
        l2::colour_strip3(fx, d, t + 0x28, 1);
        l2::colour_strip3(fx, d, t + 0x2C, 2);
    } else {
        l2::colour_strip3(fx, d, t + 0x28, 0);
    }
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xC0 + 4 * k, v);
        fx.mem.write_u32(sp + 0xD0 + 4 * k, 0x3F80_0000);
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xD0, t + 0x10, sp + 0xC0);
    for o in [0x84u32, 0x94, 0xA4, 0xB4] {
        fx.mem.set32(sp + o, 0);
    }
    let tab = d0 + 0xD8;
    let mut a = 0i32;
    let wide_by = |fx: &mut Fx| match l.plus {
        Some(p) => p,
        None => fx.mem.i16(t + 6),
    };
    for _ in 0..l.n {
        let s = (fx.mem.i16(t + 0x3A) + a) as i16 as i32;
        let e = (a + (fx.mem.i16(t + 0x3A) + l.width)) as i16 as i32;
        for (deg, inner, outer) in [(s, 0xA0u32, 0x80u32), (e, 0xB0, 0x90)] {
            for (v, wide) in [(inner, false), (outer, true)] {
                let c = l2::sin_deg(fx, tab, deg, 1);
                let r = fx.mem.i16(t + 0x38) + if wide { wide_by(fx) } else { 0 };
                fx.mem.setf(sp + v, ee::mul(ee::from_i32(r), c));
                let sn = l2::sin_deg(fx, tab, deg, 0);
                let r = fx.mem.i16(t + 0x38) + if wide { wide_by(fx) } else { 0 };
                fx.mem.setf(sp + v + 8, ee::mul(ee::from_i32(r), sn));
            }
        }
        if l2::project(fx, sp + 0x80, SPR, sp + 0xE0, d, sp + 0x120) {
            let mut k = 0;
            while k < fx.mem.i16(d0 + 0xB8) + 1 {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x120)) as i32;
                let ot = fx.mem.read_u32(d0 + 0x80);
                fx.submit(d, ot, key);
                k += 1;
            }
        }
        a = (a + l.every) as i16 as i32;
    }
    fx.leave(0x130);
}

/// 38821b785abf [3 B records; b501 0x1E00E30, 200 words]:
/// `sprite_turned(D, task, mode, pos)`: one textured sprite (9 values,
/// FST) of half size i16 +0x38 at `pos` + i16 +8 / +0xA / +0xC over 10,
/// facing the camera (0x205580's angles; scale 0.1), turned about z by the
/// task's angles +0x10 (0x1099E0 axis 2); coloured +0x28, UVs D+0x94 in
/// orientation i16 D+0xBE (the record's setter `uv`), texture D+0x88;
/// ALPHA by mode >> 4 (0: 0x48, 1: 0x44, else 0x42); depth mode & 15 (0
/// the mean w, 1 the far Z at key 0, else not drawn) on D+0x80 [F].
pub fn sprite_turned(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    sprite_turned_by(fx, uv, d0, t, mode, pos, true);
}

/// d5ba2f6c51e8 [1 B record; b555 0x1E00EA0, 180 words]:
/// [`sprite_turned`] at `pos` itself (no i16 +8 / +0xA / +0xC offset; the
/// rest word-equal) [F word-compare].
pub fn sprite_turned_at(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    sprite_turned_by(fx, uv, d0, t, mode, pos, false);
}

fn sprite_turned_by(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32, offset: bool) {
    let sp = fx.enter(0x1C0);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x50 + 4 * k, w);
    }
    let d = sp + 0x128;
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, (4u64 << 32) | 0x2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let alpha = match mode >> 4 {
        0 => 0x80_0000_0048u64,
        1 => 0x80_0000_0044,
        _ => 0x80_0000_0042,
    };
    fx.desc_alpha(d, alpha);
    rgbaq_at(fx, sp + 0x1B8, t + 0x28, d + 0x10);
    let order = fx.mem.i16(d0 + 0xBE);
    uv(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x110);
    for k in 0..3u32 {
        if offset {
            let off = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
            let v = ee::add(fx.mem.f(sp + 0x50 + 4 * k), off);
            fx.mem.setf(sp + 0xA0 + 4 * k, v);
        } else {
            let w = fx.mem.read_u32(sp + 0x50 + 4 * k);
            fx.mem.write_u32(sp + 0xA0 + 4 * k, w);
        }
    }
    l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, sp + 0x110, sp + 0xA0);
    let s = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x60, s, s);
    l2::rotate4(fx, sp + 0x60, t + 0x10, 2, d0 + 0xD8);
    if l2::project(fx, sp + 0x60, SPR, sp + 0xD0, d, sp + 0xC0) {
        let ot = fx.mem.read_u32(d0 + 0x80);
        match mode & 0xF {
            0 => {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0xC0)) as i32;
                fx.submit(d, ot, key);
            }
            1 => {
                l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                fx.submit(d, ot, 0);
            }
            _ => {}
        }
    }
    fx.leave(0x1C0);
}

/// 3af0da313c90 [2 B records; card 320's 0x1E01D70, 328 words] (helper
/// A18) [F]: `ring_band_stq(D, task, desc, mode)`: a closed ring of 36
/// textured strips (10 degrees each from +0x5A) into the caller's
/// descriptor `desc` (a strip3 with Q: RGBAQ / ST / XYZ2 x 4, flags 0x34),
/// turned by the task's angles +0x20 at D+0x20 (scale 1); the outer points
/// (radius +0x48 + +0x4C) at height +0x3C, the inner (+0x48) at 0; colours
/// +0x60 (vertices 0, 1) / +0x64 (2, 3); blend (mode >> 4) as i16; texture
/// D+0x88 with a REGION_REPEAT / REGION_CLAMP of the cell D+0x94 (U mask
/// 0xF fixed x, V y .. y + h - 1); ST / w from S 0 / 0.25 and T y / 256 /
/// (y + h) / 256, Q 1 / w; each strip projected vertex by vertex (0x204D80,
/// the strip skipped when one fails) and submitted i16 D+0xB8 + 1 times at
/// the mean w into D+0x80. `at`: the function's record address (the key
/// conversion returns to +0x488).
pub fn ring_band_stq(fx: &mut Fx, at: u32, d0: u32, t: u32, d: u32, mode: i32) {
    use ee::{add, div, mul};
    let sp = fx.enter(0x170);
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    l2::colour_strip3(fx, d, t + 0x60, 1);
    l2::colour_strip3(fx, d, t + 0x64, 2);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        let w = fx.mem.read_u32(d0 + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0x100 + 4 * k, w);
        fx.mem.write_u32(sp + 0x110 + 4 * k, 0x3F80_0000);
    }
    let y0 = fx.mem.i16(d0 + 0x96);
    let f20 = ee::from_i32(y0);
    let f21 = ee::from_i32(y0 + fx.mem.i16(d0 + 0x9A));
    l2::xform_vp(fx, sp + 0x120, sp + 0x110, t + 0x20, sp + 0x100);
    let h = fx.mem.read_u32(t + 0x3C);
    fx.mem.write_u32(sp + 0xC4, h);
    fx.mem.write_u32(sp + 0xD4, h);
    fx.mem.set32(sp + 0xE4, 0);
    fx.mem.set32(sp + 0xF4, 0);
    let tab = d0 + 0xD8;
    let mut a = 0i32;
    let mut k = 0;
    while k < 0x24 {
        fx.mem.set32(sp + 0xB0, k);
        for (off, x, z) in [(0i32, 0xE0u32, 0xC0u32), (0xA, 0xF0, 0xD0)] {
            let deg = (a + (fx.mem.i16(t + 0x5A) + off)) as i16 as i32;
            let r = fx.mem.f(t + 0x48);
            let c = l2::sin_deg(fx, tab, deg, 1);
            fx.mem.setf(sp + x, mul(r, c));
            let s = l2::sin_deg(fx, tab, deg, 0);
            let r = fx.mem.f(t + 0x48);
            fx.mem.setf(sp + x + 8, mul(r, s));
            let c = l2::sin_deg(fx, tab, deg, 1);
            let r = add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
            fx.mem.setf(sp + z, mul(r, c));
            let s = l2::sin_deg(fx, tab, deg, 0);
            let r = add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
            fx.mem.setf(sp + z + 8, mul(r, s));
        }
        let m = fx.read_m4(sp + 0x120);
        let mut ok = true;
        for (v, xyz, w) in [(0xC0u32, 0x20u32, 0x160u32), (0xD0, 0x38, 0x164), (0xE0, 0x50, 0x168), (0xF0, 0x68, 0x16C)] {
            if !fx.project_at(sp + v, &m, d + xyz, sp + w) {
                ok = false;
                break;
            }
        }
        if ok {
            for o in [0x6Fu32, 0x57, 0x3F, 0x27] {
                fx.mem.write_u8(d + o, 0);
            }
            let w: [f32; 4] = std::array::from_fn(|i| fx.mem.f(sp + 0x160 + 4 * i as u32));
            for (i, q) in [0x14u32, 0x2C, 0x44, 0x5C].into_iter().enumerate() {
                fx.mem.setf(d + q, div(1.0, w[i]));
            }
            let x = i64::from(fx.mem.i16(d0 + 0x94));
            let y = fx.mem.i16(d0 + 0x96);
            let y1 = i64::from(y + fx.mem.i16(d0 + 0x9A) - 1);
            let clamp = ((x << 14) as u64) | 0xFB | ((i64::from(y) << 24) as u64) | ((y1 << 34) as u64);
            fx.desc_clamp(d, clamp);
            let quarter = div(64.0, 256.0);
            let (t0, t1) = (div(f20, 256.0), div(f21, 256.0));
            for (i, s, tv) in [(0u32, div(0.0, 256.0), t0), (1, quarter, t0), (2, div(0.0, 256.0), t1), (3, quarter, t1)] {
                let o = 0x18 + 0x18 * i;
                let wv = fx.mem.f(sp + 0x160 + 4 * i);
                fx.mem.setf(d + o, div(s, wv));
                fx.mem.setf(d + o + 4, div(tv, wv));
            }
            let mut n = 0;
            while n < fx.mem.i16(d0 + 0xB8) + 1 {
                let w: [f32; 4] = std::array::from_fn(|i| fx.mem.f(sp + 0x160 + 4 * i as u32));
                let v = div(add(w[3], add(w[2], add(w[0], w[1]))), 4.0);
                let key = ee::fixunssfsi(v) as i32;
                fx.fix_residue(v, at + 0x488);
                let ot = fx.mem.read_u32(d0 + 0x80);
                fx.submit(d, ot, key);
                n += 1;
            }
        }
        a = (a + 0xA) as i16 as i32;
        k = fx.mem.i32(sp + 0xB0) + 1;
    }
    fx.mem.set32(sp + 0xB0, k);
    fx.leave(0x170);
}

/// 2e99d7179898 [6 B records; card 312's 0x1E02790, 336 words] (helper
/// B23) [F]: `ring_band_task(D, task, desc)`: [`ring_band_stq`]'s ring of
/// 36 textured strips for a 0x40-byte task: turned by the task's angles
/// +0x10 at its i16 point +0 (scale 1), from the angle i16 +0x3A, the
/// inner points at radius i16 +0x38, the outer at +0x38 + +6, all at
/// height 0; colours +0x28 (vertices 0, 1) / +0x2C (2, 3), blend 2; the
/// texture, clamp, ST / Q and the D+0xB8 + 1 submissions at the mean w as
/// there (the key conversion returns to +0x4B0).
pub fn ring_band_task(fx: &mut Fx, at: u32, d0: u32, t: u32, d: u32) {
    use ee::{add, div, mul};
    let sp = fx.enter(0x170);
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    fx.desc_alpha(d, blend_alpha(2));
    l2::colour_strip3(fx, d, t + 0x28, 1);
    l2::colour_strip3(fx, d, t + 0x2C, 2);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0x100 + 4 * k, v);
        fx.mem.write_u32(sp + 0x110 + 4 * k, 0x3F80_0000);
    }
    let y0 = fx.mem.i16(d0 + 0x96);
    let f20 = ee::from_i32(y0);
    let f21 = ee::from_i32(y0 + fx.mem.i16(d0 + 0x9A));
    l2::xform_vp(fx, sp + 0x120, sp + 0x110, t + 0x10, sp + 0x100);
    for o in [0xC4u32, 0xD4, 0xE4, 0xF4, 0xB0] {
        fx.mem.set32(sp + o, 0);
    }
    let tab = d0 + 0xD8;
    let mut a = 0i32;
    let mut k = 0;
    while k < 0x24 {
        for (off, x, z) in [(0i32, 0xE0u32, 0xC0u32), (0xA, 0xF0, 0xD0)] {
            let deg = (a + (fx.mem.i16(t + 0x3A) + off)) as i16 as i32;
            let c = l2::sin_deg(fx, tab, deg, 1);
            let r = ee::from_i32(fx.mem.i16(t + 0x38));
            fx.mem.setf(sp + x, mul(r, c));
            let s = l2::sin_deg(fx, tab, deg, 0);
            let r = ee::from_i32(fx.mem.i16(t + 0x38));
            fx.mem.setf(sp + x + 8, mul(r, s));
            let c = l2::sin_deg(fx, tab, deg, 1);
            let r = ee::from_i32(fx.mem.i16(t + 0x38) + fx.mem.i16(t + 6));
            fx.mem.setf(sp + z, mul(r, c));
            let s = l2::sin_deg(fx, tab, deg, 0);
            let r = ee::from_i32(fx.mem.i16(t + 0x38) + fx.mem.i16(t + 6));
            fx.mem.setf(sp + z + 8, mul(r, s));
        }
        let m = fx.read_m4(sp + 0x120);
        let mut ok = true;
        for (v, xyz, w) in [(0xC0u32, 0x20u32, 0x160u32), (0xD0, 0x38, 0x164), (0xE0, 0x50, 0x168), (0xF0, 0x68, 0x16C)] {
            if !fx.project_at(sp + v, &m, d + xyz, sp + w) {
                ok = false;
                break;
            }
        }
        if ok {
            for o in [0x6Fu32, 0x57, 0x3F, 0x27] {
                fx.mem.write_u8(d + o, 0);
            }
            let w: [f32; 4] = std::array::from_fn(|i| fx.mem.f(sp + 0x160 + 4 * i as u32));
            for (i, q) in [0x14u32, 0x2C, 0x44, 0x5C].into_iter().enumerate() {
                fx.mem.setf(d + q, div(1.0, w[i]));
            }
            let x = i64::from(fx.mem.i16(d0 + 0x94));
            let y = fx.mem.i16(d0 + 0x96);
            let y1 = i64::from(y + fx.mem.i16(d0 + 0x9A) - 1);
            let clamp = ((x << 14) as u64) | 0xFB | ((i64::from(y) << 24) as u64) | ((y1 << 34) as u64);
            fx.desc_clamp(d, clamp);
            let quarter = div(64.0, 256.0);
            let (t0, t1) = (div(f20, 256.0), div(f21, 256.0));
            let w: [f32; 4] = std::array::from_fn(|i| fx.mem.f(sp + 0x160 + 4 * i as u32));
            fx.mem.setf(d + 0x18, div(0.0, w[0]));
            fx.mem.setf(d + 0x1C, div(t0, w[0]));
            fx.mem.setf(d + 0x30, div(quarter, w[1]));
            fx.mem.setf(d + 0x34, div(t0, w[1]));
            fx.mem.setf(d + 0x48, div(div(0.0, 256.0), w[2]));
            fx.mem.setf(d + 0x4C, div(t1, w[2]));
            fx.mem.setf(d + 0x60, div(quarter, w[3]));
            fx.mem.setf(d + 0x64, div(t1, w[3]));
            let mut n = 0;
            while n < fx.mem.i16(d0 + 0xB8) + 1 {
                let w: [f32; 4] = std::array::from_fn(|i| fx.mem.f(sp + 0x160 + 4 * i as u32));
                let v = div(add(w[3], add(w[2], add(w[0], w[1]))), 4.0);
                let key = ee::fixunssfsi(v) as i32;
                fx.fix_residue(v, at + 0x4B0);
                let ot = fx.mem.read_u32(d0 + 0x80);
                fx.submit(d, ot, key);
                n += 1;
            }
        }
        a = (a + 0xA) as i16 as i32;
        k = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, k);
    }
    fx.leave(0x170);
}

/// 2c18e688d09b [10 B records; b124 (card 275) 0x1E00920, 284 words]
///: `ring_spun(D, task, mode, table)`: D+0xB8 (0: 20)
/// textured quads (9 values, FST, blend 2, vertex alphas 0) of size i16
/// +0x38, each pushed out along x / y / z by the bytes `table` [k + i16
/// +8 (+0, +0x64, +0xB4)] / 20 times the radius i16 +0x30 + k / 2, then
/// turned by the task's angles +0x10 (axes 0, 1, 2), through D+0x20
/// (scale 0.1) turned by the camera's angles; coloured +0x28, UVs from
/// D+0x94 in D+0xBE's orientation, texture D+0x88; depth `mode` (0 the
/// mean w, 1 the far Z at key 0, 2 key -1, else nothing drawn) [F].
pub fn ring_spun(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    ring_spun_by(fx, uv, d0, t, mode, table, false);
}

/// 9802da476db4 [2 B records; card 312's 0x1E01540, 328 words] (helper
/// B23) [F]: [`ring_spun`] with the blend from D+0xA4 as
/// [`ring_spun_axis`] picks it (the setter's `jal` at +0x17C).
pub fn ring_spun_blend(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    ring_spun_by(fx, uv, d0, t, mode, table, true);
}

/// D+0xA4's blend (0.0: 2, 1.0: 1, 2.0: 0, else left).
fn blend_a4(fx: &mut Fx, d0: u32, d: u32) {
    let b = fx.mem.f(d0 + 0xA4);
    if b == 0.0 {
        fx.desc_alpha(d, blend_alpha(2));
    } else if b == 1.0 {
        fx.desc_alpha(d, blend_alpha(1));
    } else if b == 2.0 {
        fx.desc_alpha(d, blend_alpha(0));
    }
}

fn ring_spun_by(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32, a4: bool) {
    let sp = fx.enter(0x200);
    let d = sp + 0x168;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha(2));
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    if a4 {
        blend_a4(fx, d0, d);
    }
    rgbaq_at(fx, sp + 0x1F8, t + 0x28, d + 0x10);
    let order = fx.mem.i16(d0 + 0xBE);
    uv(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    let count = match fx.mem.i16(d0 + 0xB8) {
        0 => 0x14,
        n => n,
    };
    scale_tenth(fx, sp + 0xF0);
    l2::camera_angles_to(fx, sp + 0x150);
    for k in 0..3 {
        let w = fx.mem.read_u32(d0 + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0xE0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x110, sp + 0xF0, sp + 0x150, sp + 0xE0);
    let tab = d0 + 0xD8;
    let mut k = 0i32;
    while k < count {
        let size = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0xA0, size, size);
        let row = table.wrapping_add(k as u32);
        let half = k >> 1;
        for (axis, off) in [(0, 0u32), (1, 0x64), (2, 0xB4)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::div(ee::from_i32(fx.mem.i8(row.wrapping_add(i as u32).wrapping_add(off))), 20.0);
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + half);
            l2::add_column(fx, sp + 0xA0, axis, ee::mul(b, r));
        }
        for axis in 0..3 {
            l2::rotate4(fx, sp + 0xA0, t + 0x10, axis, tab);
        }
        if (0..=2).contains(&mode) && l2::project(fx, sp + 0xA0, SPR, sp + 0x110, d, sp + 0x100) {
            let ot = fx.mem.read_u32(d0 + 0x80);
            match mode {
                0 => {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x100)) as i32;
                    fx.submit(d, ot, key);
                }
                1 => {
                    l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                    fx.submit(d, ot, 0);
                }
                _ => fx.submit(d, ot, -1),
            }
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x200);
}

/// 65539c83a2fd [2 B records; card 312's 0x1E01FC0, 348 words] (helper
/// B23) [F]: `ring_spun_axis(D, task, mode, table)`: [`ring_spun`]'s ring
/// with the blend from D+0xA4 (0.0: 2, 1.0: 1, 2.0: 0, else 2), each quad
/// turned about x only (axis 0, before the push) and its last two
/// vertices pinned at (0, 0, -size / 2) / (0, 0, size / 2); D+0x20 (scale
/// 0.1) not turned (the camera's angles read, y / 180 negated, but
/// unused); depth modes as there.
pub fn ring_spun_axis(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    ring_spun_axis_by(fx, uv, d0, t, mode, table, true);
}

/// 1520266faf3a [4 B records; card 230's 0x1E00FC0, 344 words] (helper
/// B23) [F]: [`ring_spun_axis`] with only the y / z of the last two
/// vertices pinned (their pushed x kept).
pub fn ring_spun_axis_z(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    ring_spun_axis_by(fx, uv, d0, t, mode, table, false);
}

fn ring_spun_axis_by(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32, pin_x: bool) {
    let sp = fx.enter(0x200);
    let d = sp + 0x168;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha(2));
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    blend_a4(fx, d0, d);
    rgbaq_at(fx, sp + 0x1F8, t + 0x28, d + 0x10);
    let order = fx.mem.i16(d0 + 0xBE);
    uv(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    let count = match fx.mem.i16(d0 + 0xB8) {
        0 => 0x14,
        n => n,
    };
    scale_tenth(fx, sp + 0xF0);
    l2::camera_angles_to(fx, sp + 0x150);
    let y = fx.mem.i32(sp + 0x154);
    fx.mult(0xB60B_60B7_u32 as i32, y);
    fx.mem.set32(sp + 0x154, -(y / 180));
    for k in 0..3 {
        let w = fx.mem.read_u32(d0 + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0xE0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x110, sp + 0xF0, 0, sp + 0xE0);
    let tab = d0 + 0xD8;
    let mut k = 0i32;
    while k < count {
        let size = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0xA0, size, size);
        l2::rotate4(fx, sp + 0xA0, t + 0x10, 0, tab);
        let row = table.wrapping_add(k as u32);
        let half = k >> 1;
        for (axis, off) in [(0, 0u32), (1, 0x64), (2, 0xB4)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::div(ee::from_i32(fx.mem.i8(row.wrapping_add(i as u32).wrapping_add(off))), 20.0);
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + half);
            l2::add_column(fx, sp + 0xA0, axis, ee::mul(b, r));
        }
        let h = fx.mem.i16(t + 0x38) >> 1;
        fx.mem.setf(sp + 0xC8, ee::from_i32(-h));
        fx.mem.set32(sp + 0xC4, 0);
        if pin_x {
            fx.mem.set32(sp + 0xC0, 0);
        }
        fx.mem.setf(sp + 0xD8, ee::from_i32(h));
        fx.mem.set32(sp + 0xD4, 0);
        if pin_x {
            fx.mem.set32(sp + 0xD0, 0);
        }
        if (0..=2).contains(&mode) && l2::project(fx, sp + 0xA0, SPR, sp + 0x110, d, sp + 0x100) {
            let ot = fx.mem.read_u32(d0 + 0x80);
            match mode {
                0 => {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x100)) as i32;
                    fx.submit(d, ot, key);
                }
                1 => {
                    l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                    fx.submit(d, ot, 0);
                }
                _ => fx.submit(d, ot, -1),
            }
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x200);
}

/// 22e62fdcabc6 [2 B records: cards 412 / 431; b477 (card 431) 0x1E01210,
/// 252 words]: `ring_ticks(D, task, side)`: nine textured
/// strip3 ticks (2 degrees wide every 40 from i16 +0x3A, between radius
/// i16 +0x38 (vertices 2, 3) and that + 20 (0, 1), y 0; D+0xD8's sines)
/// turned about x, y, z by the task's angles +0x10 (0x1099E0), at its i16
/// point (scale 1, no turn); coloured +0x28 (all four), UVs D+0x94 in
/// orientation i16 D+0xBE, texture D+0x88; a tick is drawn only on the
/// side `side` gives (0: its first x below 0, else above 0), submitted
/// i16 D+0xB8 + 1 times at its mean w's key on D+0x80 [F].
pub fn ring_ticks(fx: &mut Fx, d0: u32, t: u32, side: i32) {
    let sp = fx.enter(0x1D0);
    let d = sp + 0x140;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xD0 + 4 * k, v);
        fx.mem.write_u32(sp + 0xE0 + 4 * k, 0x3F80_0000);
    }
    l2::xform_vp(fx, sp + 0xF0, sp + 0xE0, 0, sp + 0xD0);
    for o in [0x94u32, 0xA4, 0xB4, 0xC4] {
        fx.mem.set32(sp + o, 0);
    }
    let tab = d0 + 0xD8;
    let mut a = 0i32;
    for _ in 0..9 {
        let s = (fx.mem.i16(t + 0x3A) + a) as i16 as i32;
        let e = (a + (fx.mem.i16(t + 0x3A) + 2)) as i16 as i32;
        for (deg, inner, outer) in [(s, 0xB0u32, 0x90u32), (e, 0xC0, 0xA0)] {
            for (v, wide) in [(inner, 0), (outer, 0x14)] {
                let c = l2::sin_deg(fx, tab, deg, 1);
                let r = ee::from_i32(fx.mem.i16(t + 0x38) + wide);
                fx.mem.setf(sp + v, ee::mul(r, c));
                let sn = l2::sin_deg(fx, tab, deg, 0);
                let r = ee::from_i32(fx.mem.i16(t + 0x38) + wide);
                fx.mem.setf(sp + v + 8, ee::mul(r, sn));
            }
        }
        for axis in 0..3 {
            l2::rotate4(fx, sp + 0x90, t + 0x10, axis, tab);
        }
        let x = fx.mem.f(sp + 0x90);
        let shown = if side == 0 { x < 0.0 } else { !(x <= 0.0) };
        if shown && l2::project(fx, sp + 0x90, SPR, sp + 0xF0, d, sp + 0x130) {
            let mut k = 0;
            while k < fx.mem.i16(d0 + 0xB8) + 1 {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x130)) as i32;
                let ot = fx.mem.read_u32(d0 + 0x80);
                fx.submit(d, ot, key);
                k += 1;
            }
        }
        a = (a + 0x28) as i16 as i32;
    }
    fx.leave(0x1D0);
}
