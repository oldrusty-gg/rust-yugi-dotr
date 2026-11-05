//! The unit's afterimage as 64 wavy strips,
//! hand-ported from b101 (card 305) [F]: 6aca4aa26462 `strips` (412
//! words, 10 B records: b186 b435 b542 b101 + 6) draws the unit into the
//! offscreen target (0x1F8270, between two TEXFLUSHes 0x207BB0) and pastes
//! the target back over the window in 64 horizontal strips shifted by a
//! sine, in a packet layer. With its executable helpers 0x1F6A70
//! (`unit_radius`), 0x205650 (`depth_key`) and 0x205730 (`depth_w`).
//!
//! An afterimage record `r`: +0 flags (0xA..0xC blend, 0x12 the unit's
//! depth, 0x13 far Z), +4 the colour (B, G, R, A), +0xA the unit's handle
//! (i16), +0xC the phase step per strip, +0x10 the phase (advanced by +0xC
//! each draw), +0x14 the amplitude (pixels).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, Fx, OT};

/// 0x1F6A70(handle) [F]: the radius obj+0x1E4 (0x1F2150) of the unit of
/// `handle` (side 0..2, slot 0..10, 0x1E25E0); 0 without one, -1 for an
/// index outside the table or an empty entry. A 0x10 frame.
pub fn unit_radius(fx: &mut Fx, handle: i32) -> f32 {
    let (side, slot) = (handle >> 8, handle & 0xFF);
    if !(0..3).contains(&side) || slot >= 0xB {
        return 0.0;
    }
    let i = fx.object_index(side, slot);
    if i < 0 {
        return 0.0;
    }
    if i >= 0x54 {
        return -1.0;
    }
    let o = fx.mem.read_u32(crate::world::OBJECT_TABLE + 4 * i as u32);
    if o == 0 {
        return -1.0;
    }
    fx.mem.f(o + 0x1E4)
}

/// 0x205650 / 0x205730's common part [F]: v.w = 1, v through the camera's
/// view (0x20E310 copy at sp + 0x40), z - `d`, through the projection
/// (0x20E370 copy at sp + 0x80) into sp + 0x30 (0x20C540 twice). A 0xC0
/// frame; returns its stack pointer.
fn view_offset(fx: &mut Fx, v: u32, d: f32) -> u32 {
    let sp = fx.enter(0xC0);
    let idx = fx.mem.read_u32(0x002E_D6C0);
    let rec = crate::world::CAMERA_RECORD.wrapping_add(idx.wrapping_mul(0x1A0));
    for k in 0..16u32 {
        let w = fx.mem.read_u32(rec + 0x40 + 4 * k);
        fx.mem.write_u32(sp + 0x40 + 4 * k, w);
    }
    for k in 0..16u32 {
        let w = fx.mem.read_u32(rec + 0x80 + 4 * k);
        fx.mem.write_u32(sp + 0x80 + 4 * k, w);
    }
    fx.mem.write_u32(v + 0xC, 0x3F80_0000);
    let view = fx.read_m4(sp + 0x40);
    let p = fx.read_v4(v);
    let q = fx.transform(Some(&view), p);
    fx.write_v4(sp + 0x30, q);
    let z = ee::sub(fx.mem.f(sp + 0x38), d);
    fx.mem.setf(sp + 0x38, z);
    let proj = fx.read_m4(sp + 0x80);
    let p = fx.read_v4(sp + 0x30);
    let q = fx.transform(Some(&proj), p);
    fx.write_v4(sp + 0x30, q);
    sp
}

/// 0x205650(v, d) [F]: the Z of `v` moved `d` toward the camera (view
/// space z - d), projected: 0xFFFFF6 for w <= 0, 0 for z <= 0, else
/// 0x1A2C70(z / w).
pub fn depth_key(fx: &mut Fx, v: u32, d: f32) -> u32 {
    let sp = view_offset(fx, v, d);
    let w = fx.mem.f(sp + 0x3C);
    let z = fx.mem.f(sp + 0x38);
    let r = if ee_le(w, 0.0) {
        0xFF_FFF6
    } else if ee_le(z, 0.0) {
        0
    } else {
        ee::fixunssfsi(ee::div(z, w))
    };
    fx.leave(0xC0);
    r
}

/// 0x205730(v, d) [F]: the projected w of `v` moved `d` toward the camera.
pub fn depth_w(fx: &mut Fx, v: u32, d: f32) -> f32 {
    let sp = view_offset(fx, v, d);
    let w = fx.mem.f(sp + 0x3C);
    fx.leave(0xC0);
    w
}

/// Z (bits 32..55) of the XYZ2 values at `xyz` set to `z`.
fn set_z(fx: &mut Fx, xyz: &[u32], z: u64) {
    for &a in xyz {
        let v = fx.mem.read_u64(a);
        fx.mem.write_u64(a, (v & 0xFF00_0000_FFFF_FFFF) | ((z & 0xFF_FFFF) << 32));
    }
}

/// 6aca4aa26462 [b101 0x1E00120] [F]: `strips(r, v) -> 0` (-1 when `v`,
/// the unit's position, does not project: the layer is left open). In a
/// packet layer (0x2078A0): `v` projected through a fresh transform's
/// view-projection (0x103890 / 0x1038D0 at sp + 0x100; XYZ at sp + 0x200,
/// w sp + 0x20C); a strip descriptor at sp + 0x170 (flags 0x34 | 0x800,
/// ALPHA 0x44 / 0x88 / 0x42 / 0x48 by flags 0xC / 0xB / 0xA, the colour
/// +4); TEXFLUSH, the unit of +0xA drawn into the target (0x1F8270 filter
/// 1, its TEX0 / TEX1 into the descriptor), TEXFLUSH; then 64 strips k
/// (rows 8k..8k + 8 of the target, 512 wide, at window y 0x700 + 8k),
/// shifted by +0x14 sin(+0x10 + +0xC k) (0x20ACC0, 0x1A2B80) at the top
/// and k + 1 at the bottom; Z far (0x13), the unit's (0x12) or its depth
/// moved its radius toward the camera (0x205650, keyed by 0x205730's w);
/// drawn with no ordering table. Then +0x10 += +0xC and the layer linked
/// into the ordering table at the last key. A 0x210 frame.
pub fn strips(fx: &mut Fx, r: u32, v: u32) -> i32 {
    let sp = fx.enter(0x210);
    fx.mem.write_u32(sp + 0xFC, v);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xAC, layer);
    fx.xform_init(sp + 0x100);
    fx.xform_world(sp + 0x100);
    let p = fx.mem.read_u32(sp + 0xFC);
    let m = fx.read_m4(sp + 0x130);
    if !fx.project_at(p, &m, sp + 0x200, sp + 0x20C) {
        fx.leave(0x210);
        return -1;
    }
    let d = sp + 0x170;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let flags = fx.mem.read_u32(r);
    let alpha = if bit(flags, 0xC) {
        0x44
    } else if bit(flags, 0xB) {
        0x88
    } else if bit(flags, 0xA) {
        0x42
    } else {
        0x48
    };
    fx.desc_alpha(d, 0x80_0000_0000 | alpha);
    fx.colours4(d, r + 4);
    // a0..a3 left by 0x207E20 / colours4: the descriptor, the colour, 1.0,
    // the flags before 0x200.
    fx.texflush([d, r + 4, 0x3F80_0000, 0x34]);
    let h = fx.mem.i16(r + 0xA);
    let (_, regs) = fx.unit_draw(h, 1, sp + 0x1E8, sp + 0x1F0);
    fx.texflush(regs);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let (v0, v1, v2, v3) = (sp + 0x190, sp + 0x1A8, sp + 0x1C0, sp + 0x1D8);
    let mut y = 0x700u32;
    let mut row = -1i32;
    let mut key = 0u32;
    for k in 0..0x40i32 {
        let step = fx.mem.i32(r + 0xC);
        let a = fx.mult(step, k);
        let a = fx.mem.i32(r + 0x10).wrapping_add(a);
        let s = fx.tsin(a);
        let top = ee::fixsfsi(ee::mul(fx.mem.f(r + 0x14), s));
        let step = fx.mem.i32(r + 0xC);
        let a = fx.mult(step, k + 1);
        let a = fx.mem.i32(r + 0x10).wrapping_add(a);
        let s = fx.tsin(a);
        let bottom = ee::fixsfsi(ee::mul(fx.mem.f(r + 0x14), s));
        let xy = |x: i32| ((x << 4) - 8) as u16;
        let yb = ((k + 1) << 3) + 0x700;
        fx.mem.write_u16(v0, xy(top + 0x700));
        fx.mem.write_u16(v0 + 2, xy(y as i32));
        fx.mem.write_u8(v0 + 7, 0);
        fx.mem.write_u16(v1, xy(top + 0x900));
        fx.mem.write_u16(v1 + 2, xy(y as i32));
        fx.mem.write_u8(v1 + 7, 0);
        fx.mem.write_u16(v2, xy(bottom + 0x700));
        fx.mem.write_u16(v2 + 2, xy(yb));
        fx.mem.write_u8(v2 + 7, 0);
        fx.mem.write_u16(v3, xy(bottom + 0x900));
        fx.mem.write_u16(v3 + 2, xy(yb));
        fx.mem.write_u8(v3 + 7, 0);
        let vt = ((row << 4) & 0x3FFF) as u16;
        let vb = (((((k + 1) << 3) - 1) << 4) & 0x3FFF) as u16;
        for (a, uv) in [(sp + 0x188, 0u16), (sp + 0x18A, vt), (sp + 0x1A0, 0x1FF0), (sp + 0x1A2, vt), (sp + 0x1B8, 0), (sp + 0x1BA, vb), (sp + 0x1D0, 0x1FF0), (sp + 0x1D2, vb)] {
            let h = fx.mem.read_u16(a);
            fx.mem.write_u16(a, (h & 0xC000) | uv);
        }
        let flags = fx.mem.read_u32(r);
        if bit(flags, 0x13) {
            key = 0;
            set_z(fx, &[v3, v2, v1, v0], 0xFF_FFFF);
        } else if bit(flags, 0x12) {
            let z = (fx.mem.read_u64(sp + 0x200) >> 32) & 0xFF_FFFF;
            key = 0xFF_FFFF;
            set_z(fx, &[v3, v2, v1, v0], z);
        } else {
            let h = fx.mem.i16(r + 0xA);
            let rad = unit_radius(fx, h);
            let p = fx.mem.read_u32(sp + 0xFC);
            let z = depth_key(fx, p, ee::neg(rad));
            set_z(fx, &[v3, v2, v1, v0], u64::from(z));
            let h = fx.mem.i16(r + 0xA);
            let rad = unit_radius(fx, h);
            let p = fx.mem.read_u32(sp + 0xFC);
            let w = depth_w(fx, p, ee::neg(rad));
            key = ee::fixunssfsi(w);
        }
        fx.submit(d, 0, key as i32);
        y = (y + 8) & 0xFFFF;
        row += 8;
    }
    let ph = fx.mem.i32(r + 0x10).wrapping_add(fx.mem.i32(r + 0xC));
    fx.mem.set32(r + 0x10, ph);
    let layer = fx.mem.read_u32(sp + 0xAC);
    fx.layer_end(layer, OT, key as i32);
    fx.leave(0x210);
    0
}
