//! BMODULE b470 (card 345), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000 kept in
//! s1, 0xBE10 bytes) [F]: a 0.55 relative of b295
//! (card 27, b295.rs) in a 0x3D0 frame (descriptor sp + 0x2D0, its stack
//! 0x10 lower), written fresh from the project notes on b295's helpers.
//! The init lays out 33 points of three circles (radii 8 / 8.7 / 9,
//! D+0x8690) and a 33-point circle of radius 12 (D+0xB960). Every update
//! the camera turn with roll (sp + 0x240), the attacker's nodes 0xC / 0xF
//! (D+0x20 / D+0x30), their midpoint D+0x40 and node 0x67 (D+0x60); the
//! target's position (0x1F71A0) at D+0x70 from the init. On 0x4D..0xEB one
//! of 48 fleck records (D+0x80, 0x260) of 24 flecks round the target; on
//! 0x4D..0xCB one of 16 mote records (D+0x7280, 0x140) of 16 motes; on 0xC
//! the flash (D+0x8680) at node 0x67; on 1..0x1D one of 32 spark records
//! (D+0x9550, 0x120) of 16 sparks drawn in to the midpoint; on 0 the glow
//! (D+0xB950, 0xED updates: a far sprite and a 32-triangle fan at the
//! midpoint); on 0x6F..0xEB one of 24 swirl records (D+0xBB70, 0x1C) of 16
//! far sprites on a circle round the target. The stage light 0x80 - 80
//! clamp(n) clamp(0x124 - n) / 1024. Returns 1 on 0x64, 6 on 0x6F..0xEB, 3
//! on 0xEC, 8 once the count reaches 0x125.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x3D0;
const DESC: u32 = 0x2D0;
/// The flash's gouraud band descriptor.
const BAND: u32 = 0x370;
/// The glow's fan descriptor.
const FAN: u32 = 0x328;
const CAM: u32 = 0x1C0;
const IDM: u32 = 0x200;
const TURN: u32 = 0x240;
const C0: u32 = 0x280;
const C1: u32 = 0x290;
const V0: u32 = 0x2A0;
const V1: u32 = 0x2B0;
const P: u32 = 0x2C0;
const W: u32 = 0x3CC;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
const FLECKS: u32 = D + 0x80;
const MOTES: u32 = D + 0x7280;
const FLASH: u32 = D + 0x8680;
const CIRCLES: u32 = D + 0x8690;
const FLASH_CELLS: u32 = D + 0x8CC0;
const FLASH_AT: u32 = D + 0x8D40;
const FLASH_DIRS: u32 = D + 0x8D50;
const SPARKS: u32 = D + 0x9550;
const GLOW: u32 = D + 0xB950;
const RING: u32 = D + 0xB960;
const SWIRLS: u32 = D + 0xBB70;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

/// The compiler's signed remainder by a power of two.
fn rem2(x: i32, m: i32) -> i32 {
    let r = x & (m - 1);
    if x < 0 && r != 0 {
        r - m
    } else {
        r
    }
}

fn words(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn words2(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..2u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0xBE10);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..2u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    for (node, clear, at) in [(0xC, 1, 0x18u32), (0xF, 0, 0x1C), (0x67, 0, 0x50)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x70);
    for k in 0..0x30u32 {
        fx.mem.write_u8(FLECKS + 0x260 * k, 0);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(MOTES + 0x140 * k, 0);
    }
    fx.mem.write_u8(FLASH, 0);
    for k in 0..0x21u32 {
        let a = ((k << 16) as i32) >> 5;
        let s = fx.tsin(a);
        fx.mem.setf(sp + 0x2C0, s);
        let c = fx.tcos(a);
        fx.mem.setf(sp + 0x2C4, c);
        fx.mem.write_u32(sp + 0x2C8, 0);
        fx.mem.write_u32(sp + 0x2CC, 0x3F80_0000);
        let e = CIRCLES + 0x30 * k;
        for (j, r) in [8.0f32, f32::from_bits(0x410B_3333), 9.0].into_iter().enumerate() {
            fx.vu_scale4(e + 0x10 * j as u32, sp + 0x2C0, r);
        }
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(SPARKS + 0x120 * k, 0);
    }
    fx.mem.write_u8(GLOW, 0);
    for k in 0..0x21u32 {
        let a = ((k << 16) as i32) >> 5;
        let s = fx.tsin(a);
        let e = RING + 0x10 * k;
        fx.mem.setf(e, ee::mul(12.0, s));
        let c = fx.tcos(a);
        fx.mem.setf(e + 4, ee::mul(12.0, c));
        fx.mem.write_u32(e + 8, 0);
        fx.mem.write_u32(e + 0xC, 0x3F80_0000);
    }
    for k in 0..0x18u32 {
        fx.mem.write_u8(SWIRLS + 0x1C * k, 0);
    }
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x2E8, u0);
    uv(fx, sp + 0x2EA, v0);
    uv(fx, sp + 0x2F8, u1);
    uv(fx, sp + 0x2FA, v1);
}

/// The 4 x 8 cell `b` of the texture into the sprite's two UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    let u0 = ((c << 9) & 0x3FFF) as u32;
    let u1 = ((((c << 5) + 0x20) << 4) & 0x3FFF) as u32;
    let v0 = ((r << 9) & 0x3FFF) as u32;
    let v1 = ((((r << 5) + 0x20) << 4) & 0x3FFF) as u32;
    uvs(fx, sp, u0, v0, u1, v1);
}

/// The view-projection (x identity) at sp + 0x1C0.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + CAM);
    fx.identity(sp + IDM);
    let id = fx.read_m4(sp + IDM);
    fx.mul_into(Some(sp + CAM), &id);
}

/// The view-projection x (the camera turn moved to `at`) at sp + 0x1C0.
fn camera_at(fx: &mut Fx, sp: u32, at: u32) {
    fx.camera_vp_to(sp + CAM);
    fx.translate_rows(sp + IDM, sp + TURN, at);
    let m = fx.read_m4(sp + IDM);
    fx.mul_into(Some(sp + CAM), &m);
}

/// The descriptor rewritten through its pointers, | 0x800.
fn desc_reset(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// ALPHA preset `preset`, the texture, clamp 5 (texture first when
/// `tex_first`).
fn desc_setup(fx: &mut Fx, rh_at: u32, preset: i32, tex_first: bool, sp: u32) {
    let d = sp + DESC;
    if tex_first {
        let rh = fx.mem.read_u32(D + rh_at);
        fx.desc_texture(d, rh, 1);
        let a = fx.alpha_preset(preset);
        fx.desc_alpha(d, a);
    } else {
        let a = fx.alpha_preset(preset);
        fx.desc_alpha(d, a);
        let rh = fx.mem.read_u32(D + rh_at);
        fx.desc_texture(d, rh, 1);
    }
    fx.desc_clamp(d, 5);
}

/// The corners (-s, -s, 0, 1) at sp + 0x280 and (s, s, 0, 1) at sp +
/// 0x290, turned by the camera (sp + 0x240) when `turn`.
fn corners(fx: &mut Fx, sp: u32, s: f32, turn: bool) {
    let n = ee::neg(s);
    fx.mem.setf(sp + C0, n);
    fx.mem.setf(sp + C0 + 4, n);
    fx.mem.write_u32(sp + C0 + 8, 0);
    fx.mem.write_u32(sp + C0 + 0xC, 0x3F80_0000);
    fx.mem.setf(sp + C1, s);
    fx.mem.setf(sp + C1 + 4, s);
    fx.mem.write_u32(sp + C1 + 8, 0);
    fx.mem.write_u32(sp + C1 + 0xC, 0x3F80_0000);
    if turn {
        fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
        fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
    }
}

/// The colour (c, a, Q = 1.0) at `at`.
fn colour_at(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

/// The sprite colour at sp + 0x2E0.
fn colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    colour_at(fx, sp + 0x2E0, c, a);
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// The vertices at sp + 0x2A0 / 0x2B0 = the corners + sp + 0x2C0.
fn place(fx: &mut Fx, sp: u32) {
    fx.vu_add4(sp + V0, sp + C0, sp + P);
    fx.vu_add4(sp + V1, sp + C1, sp + P);
}

fn project_from(fx: &mut Fx, sp: u32, input: u32) -> bool {
    crate::fxlib2::project(fx, input, PROJ_OUT, sp + CAM, sp + DESC, sp + W)
}

/// Projected and submitted at the mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    if project_from(fx, sp, sp + V0) {
        let w = fx.mem.f(sp + W);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// The far Z on the XYZ values at `xyz[0]` (set) and the rest (copied).
fn far_z(fx: &mut Fx, xyz: &[u32]) {
    let v = fx.mem.read_u64(xyz[0]);
    fx.mem.write_u64(xyz[0], (v & KEEP) | FAR_Z);
    for &a in &xyz[1..] {
        let z = ((fx.mem.read_u64(xyz[0]) << 8) >> 40) << 32;
        let w = fx.mem.read_u64(a);
        fx.mem.write_u64(a, (w & KEEP) | z);
    }
}

/// Projected from `input` and submitted at the far Z with key 0.
fn draw_far(fx: &mut Fx, sp: u32, input: u32) {
    if project_from(fx, sp, input) {
        far_z(fx, &[sp + 0x2F0, sp + 0x300]);
        fx.submit(sp + DESC, OT, 0);
    }
}

/// The fleck records [F 0x1E00398..0x1E008B0] (b295's flecks).
fn flecks(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x4D..0xEC).contains(&n) {
        let (_, k) = fx.div(n, 0x30);
        let base = D + 0x260 * k as u32;
        let r = base + 0x80;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0xD0, D + 0x40);
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let z = ee::add(fx.mem.f(r + 0xD8), ee::mul(8.0, side));
        fx.mem.setf(r + 0xD8, z);
        for j in 0..0x18u32 {
            let c = fx.rand_from(OT, 0x01E0_043C) >> 23;
            fx.mem.set32(r + 8 + 4 * j, rem2(c, 8));
            let q = fx.rand_from(OT, 0x01E0_045C) >> 23;
            let f = ee::from_i32(rem2(q, 256));
            let f = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), f), 256.0));
            fx.mem.setf(r + 0x68 + 4 * j, f);
            let p = r + 0xE0 + 0x10 * j;
            fx.random_table_vec(p, f32::from_bits(0x3F33_3333));
            fx.vu_add4(p, p, D + 0x70);
            fx.vu_sub4(p, p, r + 0xD0);
        }
    }
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u32(sp + 0x120, d + 2);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(sp + 0x130, d + 8);
    fx.mem.write_u32(sp + 0x140, d + 4);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    desc_setup(fx, 0x10, 1, false, sp);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(0xEC - n, 0, 0x20)), 32.0);
    fx.mem.write_u32(sp + 0x1A0, 0);
    fx.mem.write_u32(sp + 0xD0, D);
    for k in 0..0x30u32 {
        let base = D + 0x260 * k;
        let r = base + 0x80;
        if fx.mem.read_u8(r) != 0 {
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f21 = ee::mul(108.0, Fx::ratio_below(age, 4.0));
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f0 = ee::mul(f21, Fx::ramp_above(age, 48.0, 32.0));
            let a = fix(fx, ee::mul(f20, f0), 0x01E0_0630);
            colour(fx, sp, [0xFF, 0x7C, 0x56], a as u8);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let s = ee::div(ee::mul(0.5, age), 48.0);
            let s = ee::mul(12.0, ee::add(0.5, s));
            corners(fx, sp, s, true);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f21 = ee::div(ee::mul(1.5, age), 48.0);
            fx.mem.write_u32(sp + 0xC0, r + 0xD0);
            for j in 0..0x18u32 {
                let b = fx.mem.i32(r + 8 + 4 * j);
                cell_uvs(fx, sp, b);
                let sz = fx.mem.f(r + 0x68 + 4 * j);
                fx.vu_scale4(sp + V0, sp + C0, sz);
                let sz = fx.mem.f(r + 0x68 + 4 * j);
                fx.vu_scale4(sp + V1, sp + C1, sz);
                fx.vu_scale4(sp + P, r + 0xE0 + 0x10 * j, f21);
                fx.vu_add4(sp + P, sp + P, r + 0xD0);
                fx.vu_add4(sp + V0, sp + V0, sp + P);
                fx.vu_add4(sp + V1, sp + V1, sp + P);
                draw(fx, sp, 0x01E0_0838);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x31 {
                fx.mem.write_u8(r, 0);
            }
        }
        fx.mem.write_u32(sp + 0xD0, base + 0x260);
        fx.mem.write_u32(sp + 0x1A0, k + 1);
    }
}

/// The mote records [F 0x1E008B0..0x1E00CB0].
fn motes(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x4D..0xCC).contains(&n) {
        let r = MOTES + 0x140 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x30, D + 0x40);
        for j in 0..0x10u32 {
            let c = fx.rand_from(OT, 0x01E0_092C) >> 23;
            fx.mem.write_u16(r + 8 + 2 * j, rem2(c, 4) as u16);
            fx.random_table_vec(r + 0x40 + 0x10 * j, f32::from_bits(0x3F33_3333));
        }
    }
    desc_reset(fx, sp);
    desc_setup(fx, 0x14, 1, false, sp);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x190, 0);
    fx.mem.write_u32(sp + 0xF0, D);
    for k in 0..0x10u32 {
        let r = MOTES + 0x140 * k;
        if fx.mem.read_u8(r) != 0 {
            let age = fx.mem.i32(r + 4);
            let s = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(0x10 - age)), 16.0);
            let s = ee::mul(4.0, ee::add(f32::from_bits(0x3E4C_CCCD), s));
            corners(fx, sp, s, true);
            let age = fx.mem.i32(r + 4);
            let sn = fx.tsin((age << 15) >> 4);
            let a = fix(fx, ee::mul(255.0, sn), 0x01E0_0AE0);
            colour(fx, sp, [0xFF, 0x7C, 0x56], a as u8);
            let age = fx.mem.i32(r + 4);
            let f20 = fx.tan_falloff_p(age, 0x10, 1);
            fx.mem.write_u32(sp + 0xE0, r + 0x30);
            for j in 0..0x10u32 {
                fx.vu_scale4(sp + P, r + 0x40 + 0x10 * j, f20);
                fx.vu_add4(sp + P, sp + P, r + 0x30);
                place(fx, sp);
                let b = fx.mem.read_u16(r + 8 + 2 * j);
                cell_uvs(fx, sp, i32::from(b));
                draw(fx, sp, 0x01E0_0C38);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x11 {
                fx.mem.write_u8(r, 0);
            }
        }
        fx.mem.write_u32(sp + 0xF0, D + 0x140 * (k + 1));
        fx.mem.write_u32(sp + 0x190, k + 1);
    }
}

/// The flash record [F 0x1E00CB0..0x1E016B8] (b295's flash).
fn flash(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0xC {
        fx.mem.write_u8(FLASH, 1);
        fx.mem.write_u32(FLASH + 4, 0);
        words(fx, FLASH_AT, D + 0x60);
        for j in 0..0x80u32 {
            let r = fx.rand_from(OT, 0x01E0_0D14) >> 23;
            fx.mem.set8(FLASH_CELLS + j, rem2(r, 4));
            fx.random_table_vec(FLASH_DIRS + 0x10 * j, f32::from_bits(0x3F4C_CCCD));
        }
    }
    if fx.mem.read_u8(FLASH) != 1 {
        return;
    }
    desc_reset(fx, sp);
    desc_setup(fx, 0x14, 1, true, sp);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    camera_at(fx, sp, FLASH_AT);
    let age = fx.mem.i32(FLASH + 4);
    let f0 = fx.tan_falloff_p(age, 0x20, 1);
    let a = fix(fx, ee::mul(255.0, ee::sub(1.0, f0)), 0x01E0_0EA0);
    colour(fx, sp, [0x9B, 0x37, 0xFF], a as u8);
    corners(fx, sp, 8.0, false);
    draw_far(fx, sp, sp + C0);
    // The ring band: a 4-vertex gouraud strip (8 registers RGBAQ / XYZF2).
    let d2 = sp + BAND;
    fx.mem.write_u16(d2 + 2, 0x14);
    fx.mem.write_u16(d2, 8);
    fx.mem.write_u64(d2 + 8, 0x4141_4141);
    fx.mem.write_u32(d2 + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d2, a);
    fx.camera_vp_to(sp + CAM);
    fx.translate_rows(sp + IDM, sp + TURN, FLASH_AT);
    let age = fx.mem.i32(FLASH + 4);
    let t = fx.tan_falloff_p(age, 0x20, 2);
    let f20 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), t));
    fx.mem.setf(sp + P, f20);
    fx.mem.setf(sp + P + 4, f20);
    fx.mem.write_u32(sp + P + 8, 0);
    fx.mem.write_u32(sp + P + 0xC, 0x3F80_0000);
    fx.scale_rows(sp + IDM, sp + P);
    let m = fx.read_m4(sp + IDM);
    fx.mul_into(Some(sp + CAM), &m);
    let age = fx.mem.i32(FLASH + 4);
    colour_at(fx, sp + 0x380, [0x60, 0x20, 0xFF], (((0x20 - age) * 255) >> 5) as u8);
    words2(fx, sp + 0x390, sp + 0x380);
    colour_at(fx, sp + 0x3A0, [0, 0x15, 0x80], 0);
    words2(fx, sp + 0x3B0, sp + 0x3A0);
    for k in 0..0x20u32 {
        let (a, b) = (CIRCLES + 0x30 * k, CIRCLES + 0x30 * (k + 1));
        fx.mem.write_u32(sp + 0x150, D + 0x30 * (k + 1));
        fx.mem.write_u32(sp + 0x160, b + 0x10);
        for (outer, ra) in [(0u32, 0x01E0_116Cu32), (0x20, 0x01E0_1288)] {
            let vs = [a + 0x10, b + 0x10, a + outer, b + outer];
            if fx.project_n(&vs, sp + CAM, d2, sp + W, ra) == 1 {
                far_z(fx, &[sp + 0x388, sp + 0x398, sp + 0x3A8, sp + 0x3B8]);
                fx.submit(d2, OT, 0);
            }
        }
    }
    desc_reset(fx, sp);
    desc_setup(fx, 0x14, 1, true, sp);
    camera(fx, sp);
    let age = fx.mem.i32(FLASH + 4);
    colour(fx, sp, [0x21, 0x42, 0xFF], (((0x20 - age) * 255) >> 5) as u8);
    corners(fx, sp, f32::from_bits(0x3F4C_CCCD), true);
    for j in 0..0x80u32 {
        let b = fx.mem.read_u8(FLASH_CELLS + j);
        cell_uvs(fx, sp, i32::from(b));
        fx.vu_scale4(sp + P, FLASH_DIRS + 0x10 * j, f20);
        fx.vu_add4(sp + P, sp + P, FLASH_AT);
        place(fx, sp);
        draw_far(fx, sp, sp + V0);
    }
    let age = fx.mem.i32(FLASH + 4) + 1;
    fx.mem.set32(FLASH + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(FLASH, 0);
    }
}

/// The spark records [F 0x1E016B8..0x1E01B08]: 16 sparks drawn in to the
/// midpoint.
fn sparks(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x1E {
        let r = SPARKS + 0x120 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for j in 0..0x10u32 {
            let c = fx.rand_from(OT, 0x01E0_1720) >> 23;
            fx.mem.set8(r + 8 + j, rem2(c, 4));
            let p = r + 0x20 + 0x10 * j;
            fx.random_table_vec(p, 1.0);
            fx.vu_add4(p, p, D + 0x40);
        }
    }
    desc_reset(fx, sp);
    desc_setup(fx, 0x14, 1, false, sp);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x180, 0);
    fx.mem.write_u32(sp + 0x100, D);
    for k in 0..0x20u32 {
        let r = SPARKS + 0x120 * k;
        if fx.mem.read_u8(r) != 0 {
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let s = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), age), 32.0);
            let s = ee::mul(2.0, ee::add(f32::from_bits(0x3E4C_CCCD), s));
            corners(fx, sp, s, true);
            let age = fx.mem.i32(r + 4);
            let sn = fx.tsin((age << 14) >> 5);
            let a = fix(fx, ee::mul(255.0, sn), 0x01E0_18FC);
            colour(fx, sp, [0xFF, 0x38, 0x26], a as u8);
            let age = fx.mem.i32(r + 4);
            let f20 = fx.tan_falloff_p(0x20 - age, 0x20, 1);
            for j in 0..0x10u32 {
                fx.vu_sub4(sp + P, r + 0x20 + 0x10 * j, D + 0x40);
                fx.vu_scale4(sp + P, sp + P, f20);
                fx.vu_add4(sp + P, sp + P, D + 0x40);
                place(fx, sp);
                let b = fx.mem.read_u8(r + 8 + j);
                cell_uvs(fx, sp, i32::from(b));
                draw(fx, sp, 0x01E0_1A6C);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x21 {
                fx.mem.write_u8(r, 0);
            }
        }
        fx.mem.write_u32(sp + 0x100, D + 0x120 * (k + 1));
        fx.mem.write_u32(sp + 0x180, k + 1);
    }
}

/// The glow [F 0x1E01B08..0x1E01FD8]: from update 0 for 0xED updates a
/// far sprite and a 32-triangle fan (prim 0x13, 0x100B00) at the midpoint.
fn glow(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(GLOW, 1);
        fx.mem.write_u32(GLOW + 4, 0);
    }
    if fx.mem.read_u8(GLOW) != 1 {
        return;
    }
    desc_reset(fx, sp);
    desc_setup(fx, 0x14, 1, true, sp);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    camera_at(fx, sp, D + 0x40);
    let age = fx.mem.i32(GLOW + 4);
    let f20 = ee::from_i32(Fx::clamp_i(0xEC - age, 0, 0x40));
    let age = fx.mem.i32(GLOW + 4);
    let f0 = ee::from_i32(Fx::clamp_i(age, 0, 0x40));
    let f3 = ee::div(ee::mul(f20, f0), 4096.0);
    let age = fx.mem.i32(GLOW + 4);
    let f2 = ee::from_i32(rem2(age, 2));
    let f20 = ee::mul(ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), f2)), f3);
    let a = fix(fx, ee::mul(255.0, f20), 0x01E0_1CF8);
    colour(fx, sp, [0xFF, 0xA8, 0x94], a as u8);
    corners(fx, sp, 12.0, false);
    draw_far(fx, sp, sp + C0);
    let d3 = sp + FAN;
    fx.mem.write_u16(d3 + 2, 0x13);
    fx.mem.write_u16(d3, 6);
    fx.mem.write_u64(d3 + 8, 0x41_4141);
    fx.mem.write_u32(d3 + 4, 0);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d3, p);
    let a = fix(fx, ee::mul(128.0, f20), 0x01E0_1E34);
    colour_at(fx, sp + 0x338, [0xFF, 0x0A, 0x0A], a as u8);
    colour_at(fx, sp + 0x348, [0xFF, 0, 0], 0);
    words2(fx, sp + 0x358, sp + 0x348);
    fx.mem.write_u32(sp + P, 0);
    fx.mem.write_u32(sp + P + 4, 0);
    fx.mem.write_u32(sp + P + 8, 0);
    fx.mem.write_u32(sp + P + 0xC, 0x3F80_0000);
    for k in 0..0x20u32 {
        let vs = [sp + P, RING + 0x10 * k, RING + 0x10 * (k + 1)];
        if fx.project_n(&vs, sp + CAM, d3, sp + W, 0x01E0_1ECC) == 1 {
            far_z(fx, &[sp + 0x340, sp + 0x350, sp + 0x360]);
            fx.submit(d3, OT, 0);
        }
    }
    let age = fx.mem.i32(GLOW + 4) + 1;
    fx.mem.set32(GLOW + 4, age);
    if age >= 0xED {
        fx.mem.write_u8(GLOW, 0);
    }
}

/// The swirl records [F 0x1E01FD8..0x1E02578]: 16 far sprites on a circle
/// round the target, rising with age.
fn swirls(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x6F..0xEC).contains(&n) {
        let (_, k) = fx.div(n, 0x18);
        let r = SWIRLS + 0x1C * k as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        let b = fx.rand_from(OT, 0x01E0_2034) >> 23;
        fx.mem.set32(r + 0x18, (rem2(b, 0x100) << 16) >> 8);
        for j in 0..0x10u32 {
            let c = fx.rand_from(OT, 0x01E0_2070) >> 23;
            fx.mem.set8(r + 8 + j, rem2(c, 8));
        }
    }
    desc_reset(fx, sp);
    desc_setup(fx, 0x10, 1, false, sp);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(0xEC - n, 0, 0x20)), 32.0);
    fx.mem.write_u32(sp + 0x170, 0);
    fx.mem.write_u32(sp + 0x110, D);
    for k in 0..0x18u32 {
        let r = SWIRLS + 0x1C * k;
        if fx.mem.read_u8(r) != 0 {
            let age = fx.mem.i32(r + 4);
            let s = ee::div(ee::mul(0.5, ee::from_i32(0x18 - age)), 24.0);
            let s = ee::mul(15.0, ee::add(0.5, s));
            corners(fx, sp, s, true);
            let x = fx.mem.i32(r + 4) << 14;
            fx.mult(0x2AAA_AAAB, x);
            let hi = fx.hilo().1;
            let ang = (hi >> 2).wrapping_add(((x as u32) >> 31) as i32);
            let cs = fx.tcos(ang);
            let a = fix(fx, ee::mul(f20, ee::mul(128.0, cs)), 0x01E0_2274);
            colour(fx, sp, [0xFF, 0x5E, 0x36], a as u8);
            let age = fx.mem.i32(r + 4);
            let t = fx.tan_falloff_p(age, 0x18, 1);
            let f2 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), t));
            let f22 = ee::div(ee::from_i32(fx.mem.i32(r + 4)), 24.0);
            let f21 = ee::mul(60.0, f2);
            for j in 0..0x10u32 {
                let a = (fx.mem.i32(r + 0x18).wrapping_add(((j << 16) as i32) >> 4)) & 0xFFFF;
                fx.mem.set32(sp + 0x1B0, a);
                let sn = fx.tsin(a);
                fx.mem.setf(sp + P, ee::mul(f21, sn));
                let cs = fx.tcos(a);
                fx.mem.setf(sp + P + 4, ee::mul(f21, cs));
                let side = ee::from_i32(fx.mem.i32(D + 0xC));
                fx.mem.setf(sp + P + 8, ee::mul(ee::mul(-80.0, side), f22));
                fx.mem.write_u32(sp + P + 0xC, 0x3F80_0000);
                fx.vu_add4(sp + P, sp + P, D + 0x70);
                place(fx, sp);
                let b = fx.mem.read_u8(r + 8 + j);
                cell_uvs(fx, sp, i32::from(b));
                draw_far(fx, sp, sp + V0);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x19 {
                fx.mem.write_u8(r, 0);
            }
        }
        fx.mem.write_u32(sp + 0x110, D + 0x1C * (k + 1));
        fx.mem.write_u32(sp + 0x170, k + 1);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x18 + 4 * k));
        fx.node_position(h, slot, D + 0x20 + 0x10 * k);
    }
    fx.vu_add4(D + 0x40, D + 0x20, D + 0x30);
    fx.vu_scale4(D + 0x40, D + 0x40, 0.5);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x50));
    fx.node_position(h, slot, D + 0x60);
    flecks(fx, sp);
    motes(fx, sp);
    flash(fx, sp);
    sparks(fx, sp);
    glow(fx, sp);
    swirls(fx, sp);
    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x124 - n, 0, 0x20);
    let k = Fx::clamp_i(n, 0, 0x20);
    let lt = (s0.wrapping_mul(k.wrapping_mul(-80)) >> 10) + 0x80;
    fx.set_stage_light(lt & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x64 {
        ret = 1;
    } else {
        if (0x6F..0xEC).contains(&n) {
            ret = 6;
        }
        if n == 0xEC {
            ret = 3;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x125 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}
