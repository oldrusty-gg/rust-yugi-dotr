//! BMODULE b454 (card 635), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000 kept at sp
//! + 0xBC, 0x7300 bytes) [F]: b507 compiled in a
//! 0x480 frame (its locals 0x30 higher) with doubled flowers and sparks
//! and a big burst. The init lays out a flower of 8 rings x 16 petals
//! (D+0x3540, 0x40 a petal: a quad 4 wide and -300 side (ring + 1) / 8
//! long, turned by a random angle about z, its far end pushed out by (24
//! (7 - ring) / 7 sin, cos), each with a random cell 4..7 at D+0x5540).
//! Every update the camera turn with roll (0x2055E0 into sp + 0x2C0), the
//! attacker's point (0x1F65D0 kind 1, D+0x30), nodes 6 / 7 (D+0x50 /
//! D+0x60) and 8 / 9 (D+0x80 / D+0x90). On 0x65..0xF7 one of 12 burst
//! records (+0xB0, 0x460) at a random point (3.0) round the target: 32
//! sparks (1.0, 12 updates, half size 12 (0.8 + 0.2 age / 12); white /
//! (FF, 1E, 60)), a 180-wide halo (FF, 40, 1C) and 32 rays of six 3-wide
//! sprites (FF, 5A, 30) along random directions (3.5). Every fourth update
//! on 0x64..0xF7 a flower (one of 3 records +0x55C0, 0x30) at each of
//! nodes 6 / 7, turned by its own random angle and scaled by 0.5 + 0.5
//! age / 3 for 4 updates (prim 0x34 quads, colour (FF, 30, 54)) with
//! 90-wide far-Z sprites (FF, 56, 14), and one of 16 spark records
//! (+0x5660, 0x140) of 8 random sparks (0.5) at each of nodes 8 / 9, 16
//! updates (grey 9E). On 0x40 the big burst (+0x6A60): 128 sparks (1.0)
//! from the attacker's point raised 17 and pushed -10 side, 48 updates,
//! half size 6 (0.5 + 0.5 age / 48), sinking 30 age / 48, 2 x 2 cells of
//! texture 2. The stage light 0x80 - 64 clamp(n) clamp(0x124 - n) / 1024
//! (0x1F7F10). Returns 1 on update 0x5A, 6 on 0x65..0xF7, 2 on 0xF8, 8
//! once the count reaches 0x125.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x480;
const DESC: u32 = 0x390;
/// The flower's quad descriptor (prim 0x34, 12 registers).
const QUAD: u32 = 0x3E8;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
/// The entry's s0 (0x206E40's return), rand's saved s0 in the step.
const S0: u32 = OT;
const BIG: u32 = D + 0x6A60;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xBC, D);
    fx.mem.write_u32(sp + 0xC0, 0);
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

/// (x x 0x2AAAAAAB >> 32 >> `sh`) + sign: x / 12 (sh 1) or x / 48 (3).
fn div6x(fx: &mut Fx, x: i32, sh: u32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    let hi = fx.hilo().1;
    (hi >> sh).wrapping_add(((x as u32) >> 31) as i32)
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0x7300);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..3u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    for (at, node, clear) in [(0x40u32, 6, 1), (0x44, 7, 0), (0x70, 8, 0), (0x74, 9, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.write_u32(D + 0xA0, 0);
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + 0xB0 + 0x460 * k, 0);
    }
    fx.mem.write_u32(D + 0x3530, 0);
    fx.mem.write_u32(sp + 0xE0, D);
    fx.mem.write_u32(sp + 0xF0, D);
    let (m, v) = (sp + 0x340, sp + 0x380);
    for s7 in 0..8i32 {
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let f20 = ee::div(ee::mul(ee::mul(-300.0, side), ee::from_i32(s7 + 1)), 8.0);
        let f21 = ee::div(ee::mul(24.0, ee::from_i32(7 - s7)), 7.0);
        let s6 = (s7 << 13) >> 3;
        let ring = D + 0x400 * s7 as u32;
        for s0 in 0..0x10i32 {
            let a = (((s0 << 16) >> 4).wrapping_add(s6)) & 0xFFFF;
            let sn = fx.tsin(a);
            fx.mem.setf(v, ee::mul(f21, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(v + 4, ee::mul(f21, cs));
            fx.mem.write_u32(v + 8, 0);
            fx.mem.write_u32(v + 0xC, 0x3F80_0000);
            fx.identity(m);
            let r = fx.rand_from(s0 as u32, 0x01E0_02D8) >> 23;
            fx.rotate_axis(m, (rem2(r, 256) << 16) >> 8, 2);
            let q = ring + 0x40 * s0 as u32 + 0x3540;
            let (n2, p2, one) = (0xC000_0000u32, 0x4000_0000u32, 0x3F80_0000u32);
            vec4(fx, q, [n2, 0, 0, one]);
            vec4(fx, q + 0x10, [p2, 0, 0, one]);
            vec4(fx, q + 0x20, [n2, 0, f20.to_bits(), one]);
            vec4(fx, q + 0x30, [p2, 0, f20.to_bits(), one]);
            for k in 0..4u32 {
                fx.vu_apply_to(q + 0x10 * k, m, q + 0x10 * k);
            }
            fx.vu_add4(q + 0x20, q + 0x20, v);
            fx.vu_add4(q + 0x30, q + 0x30, v);
            let r = fx.rand_from(s0 as u32, 0x01E0_03A0) >> 23;
            fx.mem.set8(D + 0x10 * s7 as u32 + s0 as u32 + 0x5540, rem2(r, 4) + 4);
        }
        fx.mem.write_u32(sp + 0xE0, D + 0x400 * (s7 + 1) as u32);
        fx.mem.write_u32(sp + 0xF0, D + 0x10 * (s7 + 1) as u32);
    }
    for k in 0..3u32 {
        fx.mem.write_u8(D + 0x55C0 + 0x30 * k, 0);
    }
    fx.mem.write_u32(D + 0x5650, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x5660 + 0x140 * k, 0);
    }
    fx.mem.write_u8(BIG, 0);
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x3A8, u0);
    uv(fx, sp + 0x3AA, v0);
    uv(fx, sp + 0x3B8, u1);
    uv(fx, sp + 0x3BA, v1);
}

/// The 4 x 8 cell `b` of the texture into the sprite's two UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: u32) {
    let b = b as i32;
    let (c, r) = (rem2(b, 4) as u32, (b >> 2) as u32);
    uvs(fx, sp, (c << 9) & 0x3FFF, (r << 9) & 0x3FFF, (((c << 5) + 0x20) << 4) & 0x3FFF, (((r << 5) + 0x20) << 4) & 0x3FFF);
}

/// The 2 x 2 cell `b` (64 texels) into the sprite's two UVs.
fn cell2_uvs(fx: &mut Fx, sp: u32, b: u32) {
    let b = b as i32;
    let (c, r) = (rem2(b, 2) as u32, (b >> 1) as u32);
    uvs(fx, sp, (c << 10) & 0x3FFF, (r << 10) & 0x3FFF, (((c << 6) + 0x40) << 4) & 0x3FFF, (((r << 6) + 0x40) << 4) & 0x3FFF);
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x240);
    fx.identity(sp + 0x280);
    let id = fx.read_m4(sp + 0x280);
    fx.mul_into(Some(sp + 0x240), &id);
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

/// The corners (-s, -s, 0, 1) at sp + 0x300 and (s, s, 0, 1) at sp +
/// 0x310 (turned by the camera sp + 0x2C0 with `turn`).
fn corners(fx: &mut Fx, sp: u32, s: f32, turn: bool) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x300, n);
    fx.mem.setf(sp + 0x304, n);
    fx.mem.write_u32(sp + 0x308, 0);
    fx.mem.write_u32(sp + 0x30C, 0x3F80_0000);
    fx.mem.setf(sp + 0x310, s);
    fx.mem.setf(sp + 0x314, s);
    fx.mem.write_u32(sp + 0x318, 0);
    fx.mem.write_u32(sp + 0x31C, 0x3F80_0000);
    if turn {
        fx.vu_apply_to(sp + 0x300, sp + 0x2C0, sp + 0x300);
        fx.vu_apply_to(sp + 0x310, sp + 0x2C0, sp + 0x310);
    }
}

fn colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(sp + 0x3A0, c[0]);
    fx.mem.write_u8(sp + 0x3A1, c[1]);
    fx.mem.write_u8(sp + 0x3A2, c[2]);
    fx.mem.write_u8(sp + 0x3A3, a);
    fx.mem.write_u32(sp + 0x3A4, 0x3F80_0000);
}

/// The sprite at sp + 0x380 (= dir x f + at); its vertices sp + 0x320 /
/// 0x330.
fn place(fx: &mut Fx, sp: u32, dir: u32, f: f32, at: u32) {
    fx.vu_scale4(sp + 0x380, dir, f);
    fx.vu_add4(sp + 0x380, sp + 0x380, at);
    fx.vu_add4(sp + 0x320, sp + 0x300, sp + 0x380);
    fx.vu_add4(sp + 0x330, sp + 0x310, sp + 0x380);
}

/// Projected from `input` and submitted at the mean w.
fn draw(fx: &mut Fx, sp: u32, input: u32, ra: u32) {
    if crate::fxlib2::project(fx, input, PROJ_OUT, sp + 0x240, sp + DESC, sp + 0x47C) {
        let w = fx.mem.f(sp + 0x47C);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// A burst record started [F 0x1E00508].
fn burst_spawn(fx: &mut Fx) {
    let b = fx.mem.i32(D + 0xA0) as u32;
    let r = D + 0x460 * b + 0xB0;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 1);
    fx.random_table_vec(r + 0x50, 3.0);
    fx.vu_add4(r + 0x50, r + 0x50, D + 0x20);
    for j in 0..0x20u32 {
        let c = fx.rand_from(S0, 0x01E0_05D8) >> 23;
        fx.mem.write_u16(r + 8 + 2 * j, rem2(c, 8) as u16);
        fx.random_table_vec(r + 0x60 + 0x10 * j, 1.0);
    }
    for j in 0..0x20u32 {
        fx.random_table_vec(r + 0x260 + 0x10 * j, 3.5);
    }
    let b = fx.mem.i32(D + 0xA0) + 1;
    fx.mem.set32(D + 0xA0, b);
    if b >= 0xC {
        fx.mem.write_u32(D + 0xA0, 0);
    }
}

/// The bursts [F 0x1E006E4].
fn bursts(fx: &mut Fx, sp: u32) {
    fx.mem.write_u32(sp + 0x220, 0);
    fx.mem.write_u32(sp + 0x110, D);
    for bi in 0..0xCu32 {
        let base = D + 0x460 * bi;
        let r = base + 0xB0;
        if fx.mem.read_u8(r) != 0 {
            let d = sp + DESC;
            fx.mem.write_u16(d + 2, 0x26);
            fx.mem.write_u32(sp + 0x190, d + 2);
            fx.mem.write_u16(d, 5);
            fx.mem.write_u64(d + 8, 0x4_2421);
            fx.mem.write_u32(d + 4, 0);
            fx.mem.write_u32(sp + 0x1A0, d + 8);
            fx.mem.write_u32(sp + 0x1B0, d + 4);
            let f = fx.mem.read_u16(d + 2);
            fx.mem.write_u16(d + 2, f | 0x800);
            let rh = fx.mem.read_u32(D + 0x10);
            fx.desc_texture(d, rh, 1);
            fx.desc_clamp(d, 5);
            camera(fx, sp);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let s = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), age), 12.0);
            let s = ee::mul(12.0, ee::add(f32::from_bits(0x3F4C_CCCD), s));
            corners(fx, sp, s, true);
            let left = 0xC - fx.mem.i32(r + 4);
            let fp = div6x(fx, (left << 8) - left, 1) & 0xFF;
            let s7 = div6x(fx, left << 7, 1) & 0xFF;
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
                    colour(fx, sp, [0xFF, 0x1E, 0x60], fp as u8);
                }
                place(fx, sp, r + 0x60 + 0x10 * j, f22, pos);
                let c = fx.mem.read_u16(r + 8 + 2 * j);
                cell_uvs(fx, sp, u32::from(c));
                draw(fx, sp, sp + 0x320, 0x01E0_0A80);
            }
            // The halo, scaled by the spread at the burst's point.
            desc_reset(fx, sp, 0x14, Some(1));
            fx.camera_vp_to(sp + 0x240);
            fx.translate_rows(sp + 0x280, sp + 0x2C0, pos);
            let age = fx.mem.i32(r + 4);
            let f0 = fx.tan_falloff_p(age, 0xC, 1);
            let f = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
            fx.mem.setf(sp + 0x380, f);
            fx.mem.setf(sp + 0x384, f);
            fx.mem.setf(sp + 0x388, f);
            fx.mem.write_u32(sp + 0x38C, 0x3F80_0000);
            fx.scale_rows(sp + 0x280, sp + 0x380);
            let mm = fx.read_m4(sp + 0x280);
            fx.mul_into(Some(sp + 0x240), &mm);
            colour(fx, sp, [0xFF, 0x40, 0x1C], 0);
            uvs(fx, sp, 0x600, 0, 0x800, 0x200);
            corners(fx, sp, 90.0, false);
            draw(fx, sp, sp + 0x300, 0x01E0_0CA4);
            // The rays.
            desc_reset(fx, sp, 0x14, Some(1));
            uvs(fx, sp, 0x600, 0, 0x800, 0x200);
            camera(fx, sp);
            corners(fx, sp, 1.5, true);
            let age = fx.mem.i32(r + 4);
            let f22 = fx.tan_falloff_p(age, 0xC, 1);
            fx.mem.write_u32(sp + 0x230, 0);
            fx.mem.write_u32(sp + 0x100, base);
            for m in 0..0x20u32 {
                let dir = r + 0x260 + 0x10 * m;
                for s6 in 0..6i32 {
                    colour(fx, sp, [0xFF, 0x5A, 0x30], 0);
                    let f12 = ee::add(f22, ee::mul(f32::from_bits(0x3C23_D70A), ee::from_i32(s6)));
                    place(fx, sp, dir, f12, pos);
                    draw(fx, sp, sp + 0x320, 0x01E0_0EFC);
                }
                fx.mem.write_u32(sp + 0x100, base + 0x10 * (m + 1));
                fx.mem.write_u32(sp + 0x230, m + 1);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0xD {
                fx.mem.write_u8(r, 0);
            }
        }
        fx.mem.write_u32(sp + 0x110, base + 0x460);
        fx.mem.write_u32(sp + 0x220, bi + 1);
    }
}

/// A flower record and a spark record started [F 0x1E00FDC].
fn flower_spawn(fx: &mut Fx, sp: u32) {
    let k = fx.mem.i32(D + 0x3530) as u32;
    let fb = D + 0x30 * k;
    fx.mem.write_u8(fb + 0x55C0, 1);
    fx.mem.write_u32(fb + 0x55C4, 0);
    let m = fx.mem.i32(D + 0x5650) as u32;
    let tb = D + 0x140 * m;
    fx.mem.write_u8(tb + 0x5660, 1);
    fx.mem.write_u32(tb + 0x5664, 0);
    for s6 in 0..2u32 {
        let r = fx.rand_from(S0, 0x01E0_1038) >> 23;
        fx.mem.set32(fb + 0x55C8 + 4 * s6, (rem2(r, 256) << 16) >> 8);
        words(fx, fb + 0x55D0 + 0x10 * s6, D + 0x50 + 0x10 * s6);
    }
    fx.mem.write_u32(sp + 0x210, 0);
    fx.mem.write_u32(sp + 0x120, tb);
    for t in 0..2u32 {
        words(fx, tb + 0x5680 + 0x10 * t, D + 0x80 + 0x10 * t);
        for s6 in 0..8u32 {
            let r = fx.rand_from(S0, 0x01E0_10E0) >> 23;
            fx.mem.set8(tb + 0x5668 + 8 * t + s6, rem2(r, 8));
            fx.random_table_vec(tb + 0x56A0 + 0x80 * t + 0x10 * s6, 0.5);
        }
        fx.mem.write_u32(sp + 0x120, tb + 0x80 * (t + 1));
        fx.mem.write_u32(sp + 0x210, t + 1);
    }
    let k = fx.mem.i32(D + 0x3530) + 1;
    fx.mem.set32(D + 0x3530, k);
    if k >= 3 {
        fx.mem.write_u32(D + 0x3530, 0);
    }
    let m = fx.mem.i32(D + 0x5650) + 1;
    fx.mem.set32(D + 0x5650, m);
    if m >= 0x10 {
        fx.mem.write_u32(D + 0x5650, 0);
    }
}

/// The flowers [F 0x1E011A8].
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
    fx.mem.write_u32(sp + 0x200, 0);
    for fi in 0..3u32 {
        let fb = D + 0x30 * fi;
        fx.mem.write_u32(sp + 0x150, fb);
        if fx.mem.read_u8(fb + 0x55C0) != 0 {
            fx.mem.write_u32(sp + 0xD0, 0);
            let left = 3 - fx.mem.i32(fb + 0x55C4);
            let x = fx.mult(left, 0x56);
            fx.mult(0x5555_5556, x);
            let hi = fx.hilo().1;
            let a = hi.wrapping_add(((x as u32) >> 31) as i32);
            fx.mem.write_u8(sp + 0x3F8, 0xFF);
            fx.mem.write_u8(sp + 0x3F9, 0x30);
            fx.mem.write_u8(sp + 0x3FA, 0x54);
            fx.mem.write_u8(sp + 0x3FB, a as u8);
            fx.mem.write_u32(sp + 0x3FC, 0x3F80_0000);
            copy8(fx, sp + 0x410, sp + 0x3F8);
            copy8(fx, sp + 0x428, sp + 0x3F8);
            copy8(fx, sp + 0x440, sp + 0x3F8);
            fx.mem.write_u32(sp + 0x130, fb);
            fx.mem.write_u32(sp + 0x140, fb);
            let age = ee::from_i32(fx.mem.i32(fb + 0x55C4));
            let f = ee::add(0.5, ee::div(ee::mul(0.5, age), 3.0));
            fx.mem.setf(sp + 0x380, f);
            fx.mem.setf(sp + 0x384, f);
            fx.mem.setf(sp + 0x388, f);
            fx.mem.write_u32(sp + 0x38C, 0x3F80_0000);
            for t in 0..2u32 {
                fx.camera_vp_to(sp + 0x240);
                fx.identity(sp + 0x280);
                fx.translate_rows(sp + 0x280, sp + 0x280, fb + 0x55D0 + 0x10 * t);
                fx.rotate_axis(sp + 0x280, 0, 0);
                let ang = fx.mem.i32(fb + 0x55C8 + 4 * t);
                fx.rotate_axis(sp + 0x280, ang, 2);
                fx.scale_rows(sp + 0x280, sp + 0x380);
                let mm = fx.read_m4(sp + 0x280);
                fx.mul_into(Some(sp + 0x240), &mm);
                let mm = fx.read_m4(sp + 0x240);
                for s6 in 0..8u32 {
                    for s5 in 0..0x10u32 {
                        let cb = i32::from(fx.mem.read_u8(D + 0x10 * s6 + s5 + 0x5540));
                        let c = rem2(cb, 4) << 5;
                        let r = (cb >> 2) << 5;
                        let f4 = ee::from_i32(c);
                        let f2 = ee::from_i32(r);
                        let f5 = ee::div(ee::add(32.0, f4), 128.0);
                        let f4 = ee::div(f4, 128.0);
                        let f3 = ee::div(f2, 64.0);
                        let f0 = ee::div(ee::add(16.0, f2), 64.0);
                        fx.mem.setf(sp + 0x400, f4);
                        fx.mem.setf(sp + 0x404, f0);
                        fx.mem.setf(sp + 0x418, f5);
                        fx.mem.setf(sp + 0x41C, f0);
                        fx.mem.setf(sp + 0x430, f4);
                        fx.mem.setf(sp + 0x434, f3);
                        fx.mem.setf(sp + 0x448, f5);
                        fx.mem.setf(sp + 0x44C, f3);
                        let petal = D + 0x400 * s6 + 0x40 * s5 + 0x3540;
                        if fx.project_desc(petal, PROJ_OUT, Some(&mm), q, sp + 0x47C) {
                            let w = fx.mem.f(sp + 0x47C);
                            let key = fix(fx, w, 0x01E0_1458);
                            fx.submit(q, OT, key as i32);
                        }
                    }
                }
                fx.mem.write_u32(sp + 0x130, fb + 0x10 * (t + 1));
                fx.mem.write_u32(sp + 0x140, fb + 4 * (t + 1));
                fx.mem.write_u32(sp + 0xD0, t + 1);
            }
            // The far-Z sprites at the nodes.
            let d = sp + DESC;
            desc_reset(fx, sp, 0x14, Some(1));
            camera(fx, sp);
            corners(fx, sp, 45.0, true);
            let age = fx.mem.i32(fb + 0x55C4);
            fx.tan_falloff_p(age, 3, 1);
            let a = fix(fx, 0.0, 0x01E0_15AC);
            colour(fx, sp, [0xFF, 0x56, 0x14], a as u8);
            for s2 in 0..2u32 {
                words(fx, sp + 0x380, fb + 0x55D0 + 0x10 * s2);
                fx.vu_add4(sp + 0x320, sp + 0x300, sp + 0x380);
                fx.vu_add4(sp + 0x330, sp + 0x310, sp + 0x380);
                uvs(fx, sp, 0x600, 0, 0x800, 0x200);
                if crate::fxlib2::project(fx, sp + 0x320, PROJ_OUT, sp + 0x240, d, sp + 0x47C) {
                    let v = fx.mem.read_u64(sp + 0x3B0);
                    fx.mem.write_u64(sp + 0x3B0, (v & KEEP) | FAR_Z);
                    let z = ((fx.mem.read_u64(sp + 0x3B0) << 8) >> 40) << 32;
                    let w = fx.mem.read_u64(sp + 0x3C0);
                    fx.mem.write_u64(sp + 0x3C0, (w & KEEP) | z);
                    fx.submit(d, OT, 0);
                }
            }
            let age = fx.mem.i32(fb + 0x55C4) + 1;
            fx.mem.set32(fb + 0x55C4, age);
            if age >= 4 {
                fx.mem.write_u8(fb + 0x55C0, 0);
            }
        }
        fx.mem.write_u32(sp + 0x150, fb + 0x30);
        fx.mem.write_u32(sp + 0x200, fi + 1);
    }
}

/// The spark records [F 0x1E01790].
fn sparks(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u32(sp + 0x1C0, d + 2);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(sp + 0x1D0, d + 8);
    fx.mem.write_u32(sp + 0x1E0, d + 4);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x1F0, 0);
    for ti in 0..0x10u32 {
        let tb = D + 0x140 * ti;
        fx.mem.write_u32(sp + 0x180, tb);
        if fx.mem.read_u8(tb + 0x5660) != 0 {
            let age = ee::from_i32(fx.mem.i32(tb + 0x5664));
            let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 16.0)));
            corners(fx, sp, s, true);
            colour(fx, sp, [0x9E, 0x9E, 0x9E], 0);
            let age = fx.mem.i32(tb + 0x5664);
            let f20 = fx.tan_falloff_p(age, 0x10, 1);
            fx.mem.write_u32(sp + 0x160, tb);
            fx.mem.write_u32(sp + 0x170, tb);
            for t in 0..2u32 {
                for s6 in 0..8u32 {
                    place(fx, sp, tb + 0x56A0 + 0x80 * t + 0x10 * s6, f20, tb + 0x5680 + 0x10 * t);
                    let c = fx.mem.read_u8(tb + 0x5668 + 8 * t + s6);
                    cell_uvs(fx, sp, u32::from(c));
                    draw(fx, sp, sp + 0x320, 0x01E0_1A30);
                }
                fx.mem.write_u32(sp + 0x160, tb + 0x80 * (t + 1));
                fx.mem.write_u32(sp + 0x170, tb + 8 * (t + 1));
            }
            let age = fx.mem.i32(tb + 0x5664) + 1;
            fx.mem.set32(tb + 0x5664, age);
            if age >= 0x11 {
                fx.mem.write_u8(tb + 0x5660, 0);
            }
        }
        fx.mem.write_u32(sp + 0x180, tb + 0x140);
        fx.mem.write_u32(sp + 0x1F0, ti + 1);
    }
}

/// Update 0x40 [F 0x1E01AE4]: the big burst from the attacker's point.
fn big_spawn(fx: &mut Fx) {
    fx.mem.write_u8(BIG, 1);
    fx.mem.write_u32(BIG + 4, 0);
    words(fx, D + 0x6AF0, D + 0x30);
    let y = ee::add(fx.mem.f(D + 0x6AF4), 17.0);
    fx.mem.setf(D + 0x6AF4, y);
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    let z = ee::add(fx.mem.f(D + 0x6AF8), ee::mul(-10.0, side));
    fx.mem.setf(D + 0x6AF8, z);
    for s2 in 0..0x80u32 {
        let r = fx.rand_from(S0, 0x01E0_1B5C) >> 23;
        fx.mem.set8(D + 0x6A68 + s2, rem2(r, 4));
        fx.random_table_vec(D + 0x6B00 + 0x10 * s2, 1.0);
    }
}

/// The big burst [F 0x1E01BB4].
fn big(fx: &mut Fx, sp: u32) {
    desc_reset(fx, sp, 0x18, Some(0));
    camera(fx, sp);
    let age = ee::from_i32(fx.mem.i32(BIG + 4));
    let s = ee::mul(6.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 48.0)));
    corners(fx, sp, s, true);
    let x = (0x30 - fx.mem.i32(BIG + 4)) << 7;
    let a = div6x(fx, x, 3);
    colour(fx, sp, [0xFF, 0xFF, 0xFF], a as u8);
    let age = fx.mem.i32(BIG + 4);
    let f20 = fx.tan_falloff_p(age, 0x30, 1);
    let agef = ee::from_i32(fx.mem.i32(BIG + 4));
    let f21 = ee::div(ee::mul(-30.0, agef), 48.0);
    for s2 in 0..0x80u32 {
        fx.vu_scale4(sp + 0x380, D + 0x6B00 + 0x10 * s2, f20);
        fx.vu_add4(sp + 0x380, sp + 0x380, D + 0x6AF0);
        let y = ee::add(fx.mem.f(sp + 0x384), f21);
        fx.mem.setf(sp + 0x384, y);
        fx.vu_add4(sp + 0x320, sp + 0x300, sp + 0x380);
        fx.vu_add4(sp + 0x330, sp + 0x310, sp + 0x380);
        let c = fx.mem.read_u8(D + 0x6A68 + s2);
        cell2_uvs(fx, sp, u32::from(c));
        draw(fx, sp, sp + 0x320, 0x01E0_1E80);
    }
    let age = fx.mem.i32(BIG + 4) + 1;
    fx.mem.set32(BIG + 4, age);
    if age >= 0x31 {
        fx.mem.write_u8(BIG, 0);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x2C0);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x40 + 4 * k));
        fx.node_position(h, slot, D + 0x50 + 0x10 * k);
    }
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x70 + 4 * k));
        fx.node_position(h, slot, D + 0x80 + 0x10 * k);
    }
    let n = fx.mem.i32(D);
    if (0x65..0xF8).contains(&n) {
        burst_spawn(fx);
    }
    bursts(fx, sp);
    let n = fx.mem.i32(D);
    if rem2(n, 4) == 0 && (0x64..0xF8).contains(&n) {
        flower_spawn(fx, sp);
    }
    flowers(fx, sp);
    sparks(fx, sp);
    if fx.mem.i32(D) == 0x40 {
        big_spawn(fx);
    }
    if fx.mem.read_u8(BIG) == 1 {
        big(fx, sp);
    }
    let n = fx.mem.i32(D);
    let c1 = Fx::clamp_i(0x124 - n, 0, 0x20);
    let c2 = Fx::clamp_i(n, 0, 0x20);
    let l = fx.mult(c1, c2.wrapping_neg() << 6);
    fx.set_stage_light(((l >> 10) + 0x80) & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x5A {
        ret = 1;
    } else if (0x65..0xF8).contains(&n) {
        ret = 6;
    } else if n == 0xF8 {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x125 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}
