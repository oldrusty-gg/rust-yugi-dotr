//! BMODULE b507 (card 521), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000 kept at sp
//! + 0xF0, 0x6100 bytes) [F]: a single
//! unscheduled function in a 0x450 frame on b150's helpers. The init lays
//! out a flower of 8 rings x 16 petals (D+0x3550, 0x40 a petal: a quad 10
//! wide and 140 (ring + 1) / 8 long, turned by 0x100E40 to point along (5
//! (7 - ring) / 7 sin, cos, -140 ring / 7) and by a random angle about z,
//! each with a random cell 4..7 at D+0x5550). Every update the camera
//! turn with roll (0x2055E0 into sp + 0x290), the attacker's point
//! (0x1F65D0 kind 1, D+0x30), nodes 0x21 / 0x23 (D+0x50 / D+0x60) and the
//! aim D+0x70 (0x100E40) along them. On 0x47..0xD6 one of 12 burst records
//! (+0xC0, 0x460) at a random point (1.0) round the target: 32 sparks (0.5,
//! 12 updates, half size 6 (0.8 + 0.2 age / 12), spread by the tan falloff
//! (0.8 of it, white, ALPHA preset 2, alpha 128 (12 - age) / 12 for even
//! sparks; (1E, 60, FF), alpha 255 (12 - age) / 12 for odd), a 90-wide halo
//! (1C, 40, FF, 0) scaled by 0.2 + 0.8 falloff (0x20C290) and 32 rays of six
//! 1.5-wide sprites (30, 5A, FF, 0) along random directions (1.75). Every
//! fourth update on 0x41..0xD6 the flower (one of 3 records +0x55D0, 0x60)
//! at node 0x23 along the aim, turned by a random angle and scaled by 0.5 +
//! 0.5 age / 3 for 4 updates (prim 0x34 quads through 0x205210, cell ST,
//! colour (30, 54, FF), alpha 86 (3 - age) / 3) with a 90-wide far-Z
//! sprite (14, 56, FF, 0), and one of 16 spark records (+0x5700, 0xA0) of 8
//! random sparks (0.5) at the node, 16 updates (grey EE, ALPHA preset 0,
//! half size 10 (0.5 + 0.5 age / 16), spread by the falloff). The stage
//! light 0x80 - 80 clamp(n) clamp(0x103 - n) / 1024 (0x1F7F10). Returns 1
//! on update 0x3C, 6 on 0x47..0xD6, 2 on 0xD7, 8 once the count reaches
//! 0x104.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x450;
const DESC: u32 = 0x360;
/// The flower's quad descriptor (prim 0x34, 12 registers).
const QUAD: u32 = 0x3B8;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
/// The entry's s0 (0x206E40's return), rand's saved s0 in the step.
const S0: u32 = OT;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xF0, D);
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

fn copy8(fx: &mut Fx, to: u32, from: u32) {
    let a = fx.mem.read_u32(from);
    let b = fx.mem.read_u32(from + 4);
    fx.mem.write_u32(to, a);
    fx.mem.write_u32(to + 4, b);
}

fn vec4(fx: &mut Fx, p: u32, v: [u32; 4]) {
    for (c, w) in v.iter().enumerate() {
        fx.mem.write_u32(p + 4 * c as u32, *w);
    }
}

/// (x x 0x2AAAAAAB >> 32 >> 1) + sign: x / 12.
fn div12(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    let hi = fx.hilo().1;
    (hi >> 1).wrapping_add(((x as u32) >> 31) as i32)
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0x6100);
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
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x21, 1);
    fx.mem.set32(D + 0x40, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x23, 0);
    fx.mem.set32(D + 0x44, r);
    fx.mem.write_u32(D + 0xB0, 0);
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + 0xC0 + 0x460 * k, 0);
    }
    fx.mem.write_u32(D + 0x3540, 0);
    fx.mem.write_u32(sp + 0xD0, D);
    fx.mem.write_u32(sp + 0xE0, D);
    let (m, v) = (sp + 0x310, sp + 0x350);
    for s7 in 0..8i32 {
        let f20 = ee::div(ee::mul(-140.0, ee::from_i32(s7 + 1)), 8.0);
        let f22 = ee::div(ee::mul(-140.0, ee::from_i32(s7)), 7.0);
        let f21 = ee::div(ee::mul(5.0, ee::from_i32(7 - s7)), 7.0);
        let s6 = (s7 << 13) >> 3;
        let ring = D + 0x400 * s7 as u32;
        for s0 in 0..0x10i32 {
            let a = (((s0 << 16) >> 4).wrapping_add(s6)) & 0xFFFF;
            let sn = fx.tsin(a);
            fx.mem.setf(v, ee::mul(f21, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(v + 4, ee::mul(f21, cs));
            fx.mem.setf(v + 8, f22);
            fx.mem.write_u32(v + 0xC, 0x3F80_0000);
            fx.point_along(m, v);
            let r = fx.rand_from(s0 as u32, 0x01E0_028C) >> 23;
            fx.rotate_axis(m, (rem2(r, 256) << 16) >> 8, 2);
            let q = ring + 0x40 * s0 as u32 + 0x3550;
            let (n5, p5, one) = (0xC0A0_0000u32, 0x40A0_0000u32, 0x3F80_0000u32);
            vec4(fx, q, [n5, 0, 0, one]);
            vec4(fx, q + 0x10, [p5, 0, 0, one]);
            vec4(fx, q + 0x20, [n5, 0, f20.to_bits(), one]);
            vec4(fx, q + 0x30, [p5, 0, f20.to_bits(), one]);
            for k in 0..4u32 {
                fx.vu_apply_to(q + 0x10 * k, m, q + 0x10 * k);
            }
            let r = fx.rand_from(s0 as u32, 0x01E0_0338) >> 23;
            fx.mem.set8(D + 0x10 * s7 as u32 + s0 as u32 + 0x5550, rem2(r, 4) + 4);
        }
        fx.mem.write_u32(sp + 0xD0, D + 0x400 * (s7 + 1) as u32);
        fx.mem.write_u32(sp + 0xE0, D + 0x10 * (s7 + 1) as u32);
    }
    for k in 0..3u32 {
        fx.mem.write_u8(D + 0x55D0 + 0x60 * k, 0);
    }
    fx.mem.write_u32(D + 0x56F0, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x5700 + 0xA0 * k, 0);
    }
    // The last clear loop steps the stack copy of a0.
    fx.mem.write_u32(sp + 0xF0, D + 0xA00);
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x378, u0);
    uv(fx, sp + 0x37A, v0);
    uv(fx, sp + 0x388, u1);
    uv(fx, sp + 0x38A, v1);
}

/// The 4 x 8 cell `b` of the texture into the sprite's two UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: u32) {
    let (c, r) = (b & 3, b >> 2);
    uvs(fx, sp, (c << 9) & 0x3FFF, (r << 9) & 0x3FFF, (((c << 5) + 0x20) << 4) & 0x3FFF, (((r << 5) + 0x20) << 4) & 0x3FFF);
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x210);
    fx.identity(sp + 0x250);
    let id = fx.read_m4(sp + 0x250);
    fx.mul_into(Some(sp + 0x210), &id);
}

/// The sprite descriptor rewritten (prim 0x26 | 0x800), its texture,
/// clamp 5 and (with `preset`) its ALPHA.
fn desc_reset(fx: &mut Fx, sp: u32, rh_at: u32, preset: Option<i32>) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + rh_at);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    if let Some(p) = preset {
        let a = fx.alpha_preset(p);
        fx.desc_alpha(d, a);
    }
}

/// The corners (-s, -s, 0, 1) at sp + 0x2D0 and (s, s, 0, 1) at sp +
/// 0x2E0 (turned by the camera sp + 0x290 with `turn`).
fn corners(fx: &mut Fx, sp: u32, s: f32, turn: bool) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x2D0, n);
    fx.mem.setf(sp + 0x2D4, n);
    fx.mem.write_u32(sp + 0x2D8, 0);
    fx.mem.write_u32(sp + 0x2DC, 0x3F80_0000);
    fx.mem.setf(sp + 0x2E0, s);
    fx.mem.setf(sp + 0x2E4, s);
    fx.mem.write_u32(sp + 0x2E8, 0);
    fx.mem.write_u32(sp + 0x2EC, 0x3F80_0000);
    if turn {
        fx.vu_apply_to(sp + 0x2D0, sp + 0x290, sp + 0x2D0);
        fx.vu_apply_to(sp + 0x2E0, sp + 0x290, sp + 0x2E0);
    }
}

fn colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(sp + 0x370, c[0]);
    fx.mem.write_u8(sp + 0x371, c[1]);
    fx.mem.write_u8(sp + 0x372, c[2]);
    fx.mem.write_u8(sp + 0x373, a);
    fx.mem.write_u32(sp + 0x374, 0x3F80_0000);
}

/// The sprite at sp + 0x350 (= dir x f + at); its vertices sp + 0x2F0 /
/// 0x300.
fn place(fx: &mut Fx, sp: u32, dir: u32, f: f32, at: u32) {
    fx.vu_scale4(sp + 0x350, dir, f);
    fx.vu_add4(sp + 0x350, sp + 0x350, at);
    fx.vu_add4(sp + 0x2F0, sp + 0x2D0, sp + 0x350);
    fx.vu_add4(sp + 0x300, sp + 0x2E0, sp + 0x350);
}

/// Projected from `input` and submitted at the mean w.
fn draw(fx: &mut Fx, sp: u32, input: u32, ra: u32) {
    if crate::fxlib2::project(fx, input, PROJ_OUT, sp + 0x210, sp + DESC, sp + 0x44C) {
        let w = fx.mem.f(sp + 0x44C);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// A burst record started [F 0x1E00488].
fn burst_spawn(fx: &mut Fx) {
    let b = fx.mem.i32(D + 0xB0) as u32;
    let r = D + 0x460 * b + 0xC0;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 1);
    fx.random_table_vec(r + 0x50, 1.0);
    fx.vu_add4(r + 0x50, r + 0x50, D + 0x20);
    for j in 0..0x20u32 {
        let c = fx.rand_from(S0, 0x01E0_0558) >> 23;
        fx.mem.write_u16(r + 8 + 2 * j, rem2(c, 8) as u16);
        fx.random_table_vec(r + 0x60 + 0x10 * j, 0.5);
    }
    for j in 0..0x20u32 {
        fx.random_table_vec(r + 0x260 + 0x10 * j, 1.75);
    }
    let b = fx.mem.i32(D + 0xB0) + 1;
    fx.mem.set32(D + 0xB0, b);
    if b >= 0xC {
        fx.mem.write_u32(D + 0xB0, 0);
    }
}

/// The bursts [F 0x1E00664].
fn bursts(fx: &mut Fx, sp: u32) {
    fx.mem.write_u32(sp + 0x1F0, 0);
    fx.mem.write_u32(sp + 0x110, D);
    for bi in 0..0xCu32 {
        let base = D + 0x460 * bi;
        let r = base + 0xC0;
        if fx.mem.read_u8(r) != 0 {
            let d = sp + DESC;
            fx.mem.write_u16(d + 2, 0x26);
            fx.mem.write_u32(sp + 0x1A0, d + 2);
            fx.mem.write_u16(d, 5);
            fx.mem.write_u64(d + 8, 0x4_2421);
            fx.mem.write_u32(d + 4, 0);
            fx.mem.write_u32(sp + 0x1B0, d + 8);
            fx.mem.write_u32(sp + 0x1C0, d + 4);
            let f = fx.mem.read_u16(d + 2);
            fx.mem.write_u16(d + 2, f | 0x800);
            let rh = fx.mem.read_u32(D + 0x10);
            fx.desc_texture(d, rh, 1);
            fx.desc_clamp(d, 5);
            camera(fx, sp);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let s = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), age), 12.0);
            let s = ee::mul(6.0, ee::add(f32::from_bits(0x3F4C_CCCD), s));
            corners(fx, sp, s, true);
            let left = 0xC - fx.mem.i32(r + 4);
            let fp = div12(fx, (left << 8) - left) & 0xFF;
            let s7 = div12(fx, left << 7) & 0xFF;
            let pos = r + 0x50;
            for j in 0..0x20u32 {
                let age = fx.mem.i32(r + 4);
                let mut f22 = fx.tan_falloff_p(age, 0xC, 1);
                if rem2(j as i32, 2) == 0 {
                    f22 = ee::mul(f22, f32::from_bits(0x3F4C_CCCD));
                    let a = fx.alpha_preset(2);
                    fx.desc_alpha(d, a);
                    colour(fx, sp, [0xFF, 0xFF, 0xFF], s7 as u8);
                } else {
                    let a = fx.alpha_preset(1);
                    fx.desc_alpha(d, a);
                    colour(fx, sp, [0x1E, 0x60, 0xFF], fp as u8);
                }
                place(fx, sp, r + 0x60 + 0x10 * j, f22, pos);
                let c = fx.mem.read_u16(r + 8 + 2 * j);
                cell_uvs(fx, sp, u32::from(c));
                draw(fx, sp, sp + 0x2F0, 0x01E0_0A10);
            }
            // The halo, scaled by the spread at the burst's point.
            desc_reset(fx, sp, 0x14, Some(1));
            fx.camera_vp_to(sp + 0x210);
            fx.translate_rows(sp + 0x250, sp + 0x290, pos);
            let age = fx.mem.i32(r + 4);
            let f0 = fx.tan_falloff_p(age, 0xC, 1);
            let f = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
            fx.mem.setf(sp + 0x350, f);
            fx.mem.setf(sp + 0x354, f);
            fx.mem.setf(sp + 0x358, f);
            fx.mem.write_u32(sp + 0x35C, 0x3F80_0000);
            fx.scale_rows(sp + 0x250, sp + 0x350);
            let mm = fx.read_m4(sp + 0x250);
            fx.mul_into(Some(sp + 0x210), &mm);
            colour(fx, sp, [0x1C, 0x40, 0xFF], 0);
            uvs(fx, sp, 0x600, 0, 0x800, 0x200);
            corners(fx, sp, 45.0, false);
            draw(fx, sp, sp + 0x2D0, 0x01E0_0C34);
            // The rays.
            desc_reset(fx, sp, 0x14, Some(1));
            uvs(fx, sp, 0x600, 0, 0x800, 0x200);
            camera(fx, sp);
            corners(fx, sp, 0.75, true);
            let age = fx.mem.i32(r + 4);
            let f22 = fx.tan_falloff_p(age, 0xC, 1);
            fx.mem.write_u32(sp + 0x200, 0);
            fx.mem.write_u32(sp + 0x100, base);
            for m in 0..0x20u32 {
                let dir = r + 0x260 + 0x10 * m;
                for s6 in 0..6i32 {
                    colour(fx, sp, [0x30, 0x5A, 0xFF], 0);
                    let f12 = ee::add(f22, ee::mul(f32::from_bits(0x3BE5_6042), ee::from_i32(s6)));
                    place(fx, sp, dir, f12, pos);
                    draw(fx, sp, sp + 0x2F0, 0x01E0_0E8C);
                }
                fx.mem.write_u32(sp + 0x100, base + 0x10 * (m + 1));
                fx.mem.write_u32(sp + 0x200, m + 1);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0xD {
                fx.mem.write_u8(r, 0);
            }
        }
        fx.mem.write_u32(sp + 0x110, base + 0x460);
        fx.mem.write_u32(sp + 0x1F0, bi + 1);
    }
}

/// A flower record and a spark record started [F 0x1E00F6C].
fn flower_spawn(fx: &mut Fx, sp: u32) {
    let k = fx.mem.i32(D + 0x3540) as u32;
    let fb = D + 0x60 * k;
    fx.mem.write_u8(fb + 0x55D0, 1);
    fx.mem.write_u32(fb + 0x55D4, 0);
    let m = fx.mem.i32(D + 0x56F0) as u32;
    let tb = D + 0xA0 * m;
    fx.mem.write_u8(tb + 0x5700, 1);
    fx.mem.write_u32(tb + 0x5704, 0);
    let r = fx.rand_from(S0, 0x01E0_0FC8) >> 23;
    fx.mem.set32(fb + 0x55D8, (rem2(r, 256) << 16) >> 8);
    words(fx, fb + 0x55E0, D + 0x60);
    for c in 0..8u32 {
        copy8(fx, fb + 0x55F0 + 8 * c, D + 0x70 + 8 * c);
    }
    fx.mem.write_u32(sp + 0x120, tb);
    words(fx, tb + 0x5710, D + 0x60);
    for s6 in 0..8u32 {
        let r = fx.rand_from(S0, 0x01E0_10A0) >> 23;
        fx.mem.set8(tb + 0x5708 + s6, rem2(r, 8));
        fx.random_table_vec(tb + 0x5720 + 0x10 * s6, 0.5);
    }
    fx.mem.write_u32(sp + 0x120, tb + 0x80);
    let k = fx.mem.i32(D + 0x3540) + 1;
    fx.mem.set32(D + 0x3540, k);
    if k >= 3 {
        fx.mem.write_u32(D + 0x3540, 0);
    }
    let m = fx.mem.i32(D + 0x56F0) + 1;
    fx.mem.set32(D + 0x56F0, m);
    if m >= 0x10 {
        fx.mem.write_u32(D + 0x56F0, 0);
    }
}

/// The flowers [F 0x1E01154].
fn flowers(fx: &mut Fx, sp: u32) {
    let q = sp + QUAD;
    fx.mem.write_u16(q + 2, 0x34);
    fx.mem.write_u16(q, 0xC);
    fx.mem.write_u64(q + 8, 0x4214_2142_1421);
    fx.mem.write_u32(q + 4, 0);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(q, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(q, a);
    fx.desc_clamp(q, 0);
    fx.mem.write_u32(sp + 0x1E0, 0);
    for fi in 0..3u32 {
        let fb = D + 0x60 * fi;
        fx.mem.write_u32(sp + 0x160, fb);
        if fx.mem.read_u8(fb + 0x55D0) != 0 {
            fx.mem.write_u32(sp + 0xC0, 0);
            let left = 3 - fx.mem.i32(fb + 0x55D4);
            let x = fx.mult(left, 0x56);
            fx.mult(0x5555_5556, x);
            let hi = fx.hilo().1;
            let a = hi.wrapping_add(((x as u32) >> 31) as i32);
            fx.mem.write_u8(sp + 0x3C8, 0x30);
            fx.mem.write_u8(sp + 0x3C9, 0x54);
            fx.mem.write_u8(sp + 0x3CA, 0xFF);
            fx.mem.write_u8(sp + 0x3CB, a as u8);
            fx.mem.write_u32(sp + 0x3CC, 0x3F80_0000);
            copy8(fx, sp + 0x3E0, sp + 0x3C8);
            copy8(fx, sp + 0x3F8, sp + 0x3C8);
            copy8(fx, sp + 0x410, sp + 0x3C8);
            fx.mem.write_u32(sp + 0x130, fb);
            fx.mem.write_u32(sp + 0x140, fb);
            fx.mem.write_u32(sp + 0x150, fb);
            let age = ee::from_i32(fx.mem.i32(fb + 0x55D4));
            let f = ee::add(0.5, ee::div(ee::mul(0.5, age), 3.0));
            fx.mem.setf(sp + 0x350, f);
            fx.mem.setf(sp + 0x354, f);
            fx.mem.setf(sp + 0x358, f);
            fx.mem.write_u32(sp + 0x35C, 0x3F80_0000);
            fx.camera_vp_to(sp + 0x210);
            fx.translate_rows(sp + 0x250, fb + 0x55F0, fb + 0x55E0);
            let ang = fx.mem.i32(fb + 0x55D8);
            fx.rotate_axis(sp + 0x250, ang, 2);
            fx.scale_rows(sp + 0x250, sp + 0x350);
            let mm = fx.read_m4(sp + 0x250);
            fx.mul_into(Some(sp + 0x210), &mm);
            let mm = fx.read_m4(sp + 0x210);
            for s6 in 0..8u32 {
                for s5 in 0..0x10u32 {
                    let cb = i32::from(fx.mem.read_u8(D + 0x10 * s6 + s5 + 0x5550));
                    let c = rem2(cb, 4) << 5;
                    let r = (cb >> 2) << 5;
                    let f4 = ee::from_i32(c);
                    let f2 = ee::from_i32(r);
                    let f5 = ee::div(ee::add(32.0, f4), 128.0);
                    let f4 = ee::div(f4, 128.0);
                    let f3 = ee::div(f2, 64.0);
                    let f0 = ee::div(ee::add(16.0, f2), 64.0);
                    fx.mem.setf(sp + 0x3D0, f4);
                    fx.mem.setf(sp + 0x3D4, f0);
                    fx.mem.setf(sp + 0x3E8, f5);
                    fx.mem.setf(sp + 0x3EC, f0);
                    fx.mem.setf(sp + 0x400, f4);
                    fx.mem.setf(sp + 0x404, f3);
                    fx.mem.setf(sp + 0x418, f5);
                    fx.mem.setf(sp + 0x41C, f3);
                    let petal = D + 0x400 * s6 + 0x40 * s5 + 0x3550;
                    if fx.project_desc(petal, PROJ_OUT, Some(&mm), q, sp + 0x44C) {
                        let w = fx.mem.f(sp + 0x44C);
                        let key = fix(fx, w, 0x01E0_13E4);
                        fx.submit(q, OT, key as i32);
                    }
                }
            }
            fx.mem.write_u32(sp + 0x130, fb + 0x10);
            fx.mem.write_u32(sp + 0x140, fb + 0x40);
            fx.mem.write_u32(sp + 0x150, fb + 4);
            fx.mem.write_u32(sp + 0xC0, 1);
            // The far-Z sprite at the node.
            let d = sp + DESC;
            desc_reset(fx, sp, 0x14, Some(1));
            camera(fx, sp);
            corners(fx, sp, 45.0, true);
            let age = fx.mem.i32(fb + 0x55D4);
            fx.tan_falloff_p(age, 3, 1);
            let a = fix(fx, 0.0, 0x01E0_1544);
            colour(fx, sp, [0x14, 0x56, 0xFF], a as u8);
            words(fx, sp + 0x350, fb + 0x55E0);
            fx.vu_add4(sp + 0x2F0, sp + 0x2D0, sp + 0x350);
            fx.vu_add4(sp + 0x300, sp + 0x2E0, sp + 0x350);
            uvs(fx, sp, 0x600, 0, 0x800, 0x200);
            if crate::fxlib2::project(fx, sp + 0x2F0, PROJ_OUT, sp + 0x210, d, sp + 0x44C) {
                let v = fx.mem.read_u64(sp + 0x380);
                fx.mem.write_u64(sp + 0x380, (v & KEEP) | FAR_Z);
                let z = ((fx.mem.read_u64(sp + 0x380) << 8) >> 40) << 32;
                let w = fx.mem.read_u64(sp + 0x390);
                fx.mem.write_u64(sp + 0x390, (w & KEEP) | z);
                fx.submit(d, OT, 0);
            }
            let age = fx.mem.i32(fb + 0x55D4) + 1;
            fx.mem.set32(fb + 0x55D4, age);
            if age >= 4 {
                fx.mem.write_u8(fb + 0x55D0, 0);
            }
        }
        fx.mem.write_u32(sp + 0x160, fb + 0x60);
        fx.mem.write_u32(sp + 0x1E0, fi + 1);
    }
}

/// The spark records [F 0x1E01720].
fn sparks(fx: &mut Fx, sp: u32) {
    desc_reset(fx, sp, 0x10, Some(0));
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x1D0, 0);
    for ti in 0..0x10u32 {
        let tb = D + 0xA0 * ti;
        fx.mem.write_u32(sp + 0x190, tb);
        if fx.mem.read_u8(tb + 0x5700) != 0 {
            let age = ee::from_i32(fx.mem.i32(tb + 0x5704));
            let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 16.0)));
            corners(fx, sp, s, true);
            colour(fx, sp, [0xEE, 0xEE, 0xEE], 0);
            let age = fx.mem.i32(tb + 0x5704);
            let f20 = fx.tan_falloff_p(age, 0x10, 1);
            fx.mem.write_u32(sp + 0x170, tb);
            fx.mem.write_u32(sp + 0x180, tb);
            for s6 in 0..8u32 {
                place(fx, sp, tb + 0x5720 + 0x10 * s6, f20, tb + 0x5710);
                let c = fx.mem.read_u8(tb + 0x5708 + s6);
                cell_uvs(fx, sp, u32::from(c));
                draw(fx, sp, sp + 0x2F0, 0x01E0_19A8);
            }
            fx.mem.write_u32(sp + 0x170, tb + 0x80);
            fx.mem.write_u32(sp + 0x180, tb + 8);
            let age = fx.mem.i32(tb + 0x5704) + 1;
            fx.mem.set32(tb + 0x5704, age);
            if age >= 0x11 {
                fx.mem.write_u8(tb + 0x5700, 0);
            }
        }
        fx.mem.write_u32(sp + 0x190, tb + 0xA0);
        fx.mem.write_u32(sp + 0x1D0, ti + 1);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x290);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x40 + 4 * k));
        fx.node_position(h, slot, D + 0x50 + 0x10 * k);
    }
    fx.vu_sub4(sp + 0x350, D + 0x60, D + 0x50);
    fx.point_along(D + 0x70, sp + 0x350);
    let n = fx.mem.i32(D);
    if (0x47..0xD7).contains(&n) {
        burst_spawn(fx);
    }
    bursts(fx, sp);
    let n = fx.mem.i32(D);
    if rem2(n, 4) == 0 && (0x41..0xD7).contains(&n) {
        flower_spawn(fx, sp);
    }
    flowers(fx, sp);
    sparks(fx, sp);
    let n = fx.mem.i32(D);
    let c1 = Fx::clamp_i(0x103 - n, 0, 0x20);
    let c2 = Fx::clamp_i(n, 0, 0x20);
    let l = fx.mult(c1, ((c2.wrapping_neg() << 2) - c2) << 4);
    fx.set_stage_light(((l >> 10) + 0x80) & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x3C {
        ret = 1;
    } else if (0x47..0xD7).contains(&n) {
        ret = 6;
    } else if n == 0xD7 {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x104 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}
