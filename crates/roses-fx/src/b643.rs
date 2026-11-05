//! BMODULE b643 (card 623), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000 kept in
//! s4, 0x4B40 bytes) [F]: a single unscheduled
//! function in a 0x2F0 frame on b150's helpers, b261's (card 207) sibling;
//! every sprite a textured window sprite (descriptor sp + 0x290: 5
//! registers 0x42421, prim 0x26 | 0x800, ALPHA preset 1, clamp 5) through
//! 0x204F70 / 0x206F30. Every update the camera turn with roll (0x2055E0
//! into sp + 0x1F0), the attacker's nodes 0x1D / 0x1E (D+0x20 / D+0x30)
//! and their midpoint D+0x40, nodes 0x17 / 0x18 (D+0x60 / D+0x70) and a
//! point 1.4 of the way along them D+0x80 (last update's in D+0x90). On
//! 0x92..0x12B one of 32 ring records (+0xC0, 0xE0) at the midpoint, of 8
//! random rays (0.4) at the target (D+0xA0) with sizes 0.2 + 0.8 rand /
//! 256, living 32 updates: pushed out by 2.5 age / 32 and raised by (-50 +
//! 100 clamp(n - 0x91, 0, 155) / 155) age / 32, half size 12 (0.5 + 0.5
//! age / 32), alpha 180 x ratio_below(age, 3) x ramp_above(age, 32, 21) x
//! clamp(0x12C - n) / 32, colour (FF, 38, 19). On 0xB5..0x149 one of 16
//! burst records (+0x1CC0, 0x230) of 32 random directions (1.3) round the
//! target's point D+0xB0 as b261's (alpha 96 sin(age << 11) x the window
//! fade, colour (FF, 39, 19), keyed by w). On 0x92..0x12B one of 24 puff
//! records (+0x3FC0, 0x60) of four random puffs (0.5) at the midpoint
//! (texture D+0x14, half size 4 (0.2 + 0.8 (24 - age) / 24), alpha 255
//! cos((age << 14) / 24), colour (FF, 46, 17)). On 1..0x98 one of 8 trail
//! records (+0x48C0, 0x50) of four points from last update's point to
//! this one, 9 updates (half size 2, alpha 64 cos(age << 11) x the window
//! fade (n, 0x99 - n), colour (FF, 40, 18), far Z, key 0); while the
//! window (n, 0x12C - n) is open a head sprite (half size 4, alpha 255 x
//! the window x (0.8 + 0.2 (n & 1))) at the point. Returns 1 on update
//! 0xAA, 6 on 0xB5..0x148, 2 on 0x149, 8 once the count reaches 0x14B.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x2F0;
const DESC: u32 = 0x290;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
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

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x4B40);
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
    for (node, clear, at) in [(0x1D, 1, 0x18u32), (0x1E, 0, 0x1C), (0x17, 0, 0x50), (0x18, 0, 0x54)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0xA0);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0xB0);
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0xC0 + 0xE0 * k, 0);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x1CC0 + 0x230 * k, 0);
    }
    for k in 0..0x18u32 {
        fx.mem.write_u8(D + 0x3FC0 + 0x60 * k, 0);
    }
    fx.mem.write_u32(D + 0x80, 0);
    fx.mem.write_u32(D + 0x84, 0);
    fx.mem.write_u32(D + 0x88, 0);
    fx.mem.write_u32(D + 0x8C, 0x3F80_0000);
    for k in 0..8u32 {
        fx.mem.write_u8(D + 0x48C0 + 0x50 * k, 0);
    }
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x2A8, u0);
    uv(fx, sp + 0x2AA, v0);
    uv(fx, sp + 0x2B8, u1);
    uv(fx, sp + 0x2BA, v1);
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

/// The view-projection (x identity) at sp + 0x170.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x170);
    fx.identity(sp + 0x1B0);
    let id = fx.read_m4(sp + 0x1B0);
    fx.mul_into(Some(sp + 0x170), &id);
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

/// ALPHA preset 1, the texture, clamp 5 (texture first when `tex_first`).
fn desc_setup(fx: &mut Fx, sp: u32, rh_at: u32, tex_first: bool) {
    let d = sp + DESC;
    if tex_first {
        let rh = fx.mem.read_u32(D + rh_at);
        fx.desc_texture(d, rh, 1);
        let a = fx.alpha_preset(1);
        fx.desc_alpha(d, a);
    } else {
        let a = fx.alpha_preset(1);
        fx.desc_alpha(d, a);
        let rh = fx.mem.read_u32(D + rh_at);
        fx.desc_texture(d, rh, 1);
    }
    fx.desc_clamp(d, 5);
    camera(fx, sp);
}

/// The corners (-s, -s, 0, 1) at sp + 0x230 and (s, s, 0, 1) at sp +
/// 0x240 turned by the camera (sp + 0x1F0).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x230, n);
    fx.mem.setf(sp + 0x234, n);
    fx.mem.write_u32(sp + 0x238, 0);
    fx.mem.write_u32(sp + 0x23C, 0x3F80_0000);
    fx.mem.setf(sp + 0x240, s);
    fx.mem.setf(sp + 0x244, s);
    fx.mem.write_u32(sp + 0x248, 0);
    fx.mem.write_u32(sp + 0x24C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x230, sp + 0x1F0, sp + 0x230);
    fx.vu_apply_to(sp + 0x240, sp + 0x1F0, sp + 0x240);
}

/// The colour (c, a, Q = 1.0) at sp + 0x2A0.
fn colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(sp + 0x2A0, c[0]);
    fx.mem.write_u8(sp + 0x2A1, c[1]);
    fx.mem.write_u8(sp + 0x2A2, c[2]);
    fx.mem.write_u8(sp + 0x2A3, a);
    fx.mem.write_u32(sp + 0x2A4, 0x3F80_0000);
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// The vertices at sp + 0x250 / 0x260 = the corners + sp + 0x270.
fn place(fx: &mut Fx, sp: u32) {
    fx.vu_add4(sp + 0x250, sp + 0x230, sp + 0x270);
    fx.vu_add4(sp + 0x260, sp + 0x240, sp + 0x270);
}

fn project(fx: &mut Fx, sp: u32) -> bool {
    crate::fxlib2::project(fx, sp + 0x250, PROJ_OUT, sp + 0x170, sp + DESC, sp + 0x2EC)
}

/// Projected and submitted at the mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    if project(fx, sp) {
        let w = fx.mem.f(sp + 0x2EC);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// Projected and submitted at the far Z with key 0.
fn draw_far(fx: &mut Fx, sp: u32) {
    if project(fx, sp) {
        let v = fx.mem.read_u64(sp + 0x2B0);
        fx.mem.write_u64(sp + 0x2B0, (v & KEEP) | FAR_Z);
        let z = ((fx.mem.read_u64(sp + 0x2B0) << 8) >> 40) << 32;
        let w = fx.mem.read_u64(sp + 0x2C0);
        fx.mem.write_u64(sp + 0x2C0, (w & KEEP) | z);
        fx.submit(sp + DESC, OT, 0);
    }
}

/// clamp(a, 0, 32) x clamp(b, 0, 32) / 1024.
fn window(a: i32, b: i32) -> f32 {
    let f = ee::from_i32(Fx::clamp_i(a, 0, 0x20));
    let g = ee::from_i32(Fx::clamp_i(b, 0, 0x20));
    ee::div(ee::mul(f, g), 1024.0)
}

/// The ring records [F 0x1E0035C / 0x1E004E0].
fn rings(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x92..0x12C).contains(&n) {
        let base = D + 0xE0 * rem2(n, 0x20) as u32;
        let r = base + 0xC0;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        let n = fx.mem.i32(D);
        let c = ee::from_i32(Fx::clamp_i(n - 0x91, 0, 0x9B));
        let f = ee::add(-50.0, ee::div(ee::mul(100.0, c), 155.0));
        fx.mem.setf(r + 0x48, f);
        words(fx, r + 0x50, D + 0x40);
        let z = ee::add(fx.mem.f(r + 0x58), 0.0);
        fx.mem.setf(r + 0x58, z);
        for j in 0..8u32 {
            let s0 = base + 4 * j;
            let c = fx.rand_from(s0, 0x01E0_0424) >> 23;
            fx.mem.set32(r + 8 + 4 * j, rem2(c, 8));
            let q = fx.rand_from(s0, 0x01E0_0444) >> 23;
            let f = ee::from_i32(rem2(q, 256));
            let f = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), f), 256.0));
            fx.mem.setf(r + 0x28 + 4 * j, f);
            let p = r + 0x60 + 0x10 * j;
            fx.random_table_vec(p, f32::from_bits(0x3ECC_CCCD));
            fx.vu_add4(p, p, D + 0xA0);
            fx.vu_sub4(p, p, r + 0x50);
        }
    }
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u32(sp + 0x110, d + 2);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(sp + 0x120, d + 8);
    fx.mem.write_u32(sp + 0x130, d + 4);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    desc_setup(fx, sp, 0x10, false);
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(0x12C - n, 0, 0x20)), 32.0);
    fx.mem.write_u32(sp + 0x160, 0);
    fx.mem.write_u32(sp + 0xD0, D);
    for k in 0..0x20u32 {
        let base = D + 0xE0 * k;
        let r = base + 0xC0;
        if fx.mem.read_u8(r) != 0 {
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f0 = Fx::ratio_below(age, 3.0);
            let f21 = ee::mul(180.0, f0);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f0 = Fx::ramp_above(age, 32.0, 21.0);
            let f0 = ee::mul(f21, f0);
            let a = fix(fx, ee::mul(f20, f0), 0x01E0_0618);
            colour(fx, sp, [0xFF, 0x38, 0x19], a as u8);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let s = ee::div(ee::mul(0.5, age), 32.0);
            let s = ee::mul(12.0, ee::add(0.5, s));
            corners(fx, sp, s);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f22 = ee::div(ee::mul(2.5, age), 32.0);
            let f21 = ee::div(ee::mul(fx.mem.f(r + 0x48), age), 32.0);
            fx.mem.write_u32(sp + 0xC0, r + 0x50);
            for j in 0..8u32 {
                let b = fx.mem.i32(r + 8 + 4 * j);
                cell_uvs(fx, sp, b);
                let sz = fx.mem.f(r + 0x28 + 4 * j);
                fx.vu_scale4(sp + 0x250, sp + 0x230, sz);
                let sz = fx.mem.f(r + 0x28 + 4 * j);
                fx.vu_scale4(sp + 0x260, sp + 0x240, sz);
                fx.vu_scale4(sp + 0x270, r + 0x60 + 0x10 * j, f22);
                let y = ee::add(fx.mem.f(sp + 0x274), f21);
                fx.mem.setf(sp + 0x274, y);
                fx.vu_add4(sp + 0x270, sp + 0x270, r + 0x50);
                fx.vu_add4(sp + 0x250, sp + 0x250, sp + 0x270);
                fx.vu_add4(sp + 0x260, sp + 0x260, sp + 0x270);
                draw(fx, sp, 0x01E0_083C);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x21 {
                fx.mem.write_u8(r, 0);
            }
        }
        fx.mem.write_u32(sp + 0xD0, base + 0xE0);
        fx.mem.write_u32(sp + 0x160, k + 1);
    }
}

/// The burst records [F 0x1E008B8 / 0x1E00960].
fn bursts(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0xB5..0x14A).contains(&n) {
        let q = D + 0x1CC0 + 0x230 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(q, 1);
        fx.mem.write_u32(q + 4, 0);
        for j in 0..0x20u32 {
            let r = fx.rand_from(sp + 0x2AA, 0x01E0_091C) >> 23;
            fx.mem.set8(q + 8 + j, rem2(r, 8));
            fx.random_table_vec(q + 0x30 + 0x10 * j, f32::from_bits(0x3FA6_6666));
        }
    }
    desc_reset(fx, sp);
    desc_setup(fx, sp, 0x10, true);
    let n = fx.mem.i32(D);
    let f20 = window(n - 0xB4, 0x14A - n);
    fx.mem.write_u32(sp + 0x150, 0);
    fx.mem.write_u32(sp + 0xE0, D);
    for i in 0..0x10u32 {
        let q = D + 0x1CC0 + 0x230 * i;
        if fx.mem.read_u8(q) != 0 {
            let age = fx.mem.i32(q + 4);
            let sn = fx.tsin((age << 15) >> 4);
            let a = fix(fx, ee::mul(f20, ee::mul(96.0, sn)), 0x01E0_0A80);
            colour(fx, sp, [0xFF, 0x39, 0x19], a as u8);
            let age = fx.mem.i32(q + 4);
            let s = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(0x10 - age)), 16.0);
            let s = ee::mul(15.0, ee::add(f32::from_bits(0x3E4C_CCCD), s));
            corners(fx, sp, s);
            let age = fx.mem.i32(q + 4);
            let f4 = ee::div(ee::from_i32(age), 16.0);
            let f1 = ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(0x10 - age)), 16.0);
            let f21 = ee::add(f32::from_bits(0x3F66_6666), f1);
            let f22 = ee::add(-25.0, ee::mul(80.0, f4));
            for j in 0..0x20u32 {
                let b = fx.mem.read_u8(q + 8 + j);
                cell_uvs(fx, sp, i32::from(b));
                let p = q + 0x30 + 0x10 * j;
                let x = ee::mul(fx.mem.f(p), f21);
                fx.mem.setf(sp + 0x270, x);
                let y = ee::add(f22, fx.mem.f(p + 4));
                fx.mem.setf(sp + 0x274, y);
                let z = ee::mul(fx.mem.f(p + 8), f21);
                fx.mem.setf(sp + 0x278, z);
                fx.vu_add4(sp + 0x270, sp + 0x270, D + 0xB0);
                place(fx, sp);
                draw(fx, sp, 0x01E0_0CDC);
            }
            let age = fx.mem.i32(q + 4) + 1;
            fx.mem.set32(q + 4, age);
            if age >= 0x11 {
                fx.mem.write_u8(q, 0);
            }
        }
        fx.mem.write_u32(sp + 0xE0, D + 0x230 * (i + 1));
        fx.mem.write_u32(sp + 0x150, i + 1);
    }
}

/// The puff records [F 0x1E00D58 / 0x1E00E10].
fn puffs(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x92..0x12C).contains(&n) {
        let (_, k) = fx.div(n, 0x18);
        let p = D + 0x3FC0 + 0x60 * k as u32;
        fx.mem.write_u8(p, 1);
        fx.mem.write_u32(p + 4, 0);
        words(fx, p + 0x10, D + 0x40);
        for j in 0..4u32 {
            let r = fx.rand_from(sp + 0x2AA, 0x01E0_0DD0) >> 23;
            fx.mem.write_u16(p + 8 + 2 * j, rem2(r, 4) as u16);
            fx.random_table_vec(p + 0x20 + 0x10 * j, 0.5);
        }
    }
    desc_reset(fx, sp);
    desc_setup(fx, sp, 0x14, false);
    fx.mem.write_u32(sp + 0x140, 0);
    fx.mem.write_u32(sp + 0x100, D);
    for i in 0..0x18u32 {
        let p = D + 0x3FC0 + 0x60 * i;
        if fx.mem.read_u8(p) != 0 {
            let age = fx.mem.i32(p + 4);
            let s = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(0x18 - age)), 24.0);
            let s = ee::mul(4.0, ee::add(f32::from_bits(0x3E4C_CCCD), s));
            corners(fx, sp, s);
            let x = fx.mem.i32(p + 4) << 14;
            fx.mult(0x2AAA_AAAB, x);
            let hi = fx.hilo().1;
            let ang = (hi >> 2).wrapping_add(((x as u32) >> 31) as i32);
            let cs = fx.tcos(ang);
            let a = fix(fx, ee::mul(255.0, cs), 0x01E0_0FA0);
            colour(fx, sp, [0xFF, 0x46, 0x17], a as u8);
            let age = fx.mem.i32(p + 4);
            let f20 = fx.tan_falloff_p(age, 0x18, 1);
            fx.mem.write_u32(sp + 0xF0, p + 0x10);
            for j in 0..4u32 {
                fx.vu_scale4(sp + 0x270, p + 0x20 + 0x10 * j, f20);
                fx.vu_add4(sp + 0x270, sp + 0x270, p + 0x10);
                place(fx, sp);
                let b = fx.mem.read_u16(p + 8 + 2 * j);
                cell_uvs(fx, sp, i32::from(b));
                draw(fx, sp, 0x01E0_10F8);
            }
            let age = fx.mem.i32(p + 4) + 1;
            fx.mem.set32(p + 4, age);
            if age >= 0x19 {
                fx.mem.write_u8(p, 0);
            }
        }
        fx.mem.write_u32(sp + 0x100, D + 0x60 * (i + 1));
        fx.mem.write_u32(sp + 0x140, i + 1);
    }
}

/// The square corners (-s, -s) / (s, s) turned by the camera.
fn square(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x230, n);
    fx.mem.setf(sp + 0x234, n);
    fx.mem.write_u32(sp + 0x238, 0);
    fx.mem.write_u32(sp + 0x23C, 0x3F80_0000);
    fx.mem.setf(sp + 0x240, s);
    fx.mem.setf(sp + 0x244, s);
    fx.mem.write_u32(sp + 0x248, 0);
    fx.mem.write_u32(sp + 0x24C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x230, sp + 0x1F0, sp + 0x230);
    fx.vu_apply_to(sp + 0x240, sp + 0x1F0, sp + 0x240);
}

/// The trail records and the head [F 0x1E01170 / 0x1E01248].
fn trails(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x99 {
        let k = rem2(n, 8) as u32;
        let t = D + 0x48C0 + 0x50 * k;
        fx.mem.write_u8(t, 1);
        fx.mem.write_u32(t + 4, 0);
        fx.vu_sub4(sp + 0x270, D + 0x80, D + 0x90);
        for s5 in 0..4i32 {
            let f = ee::div(ee::from_i32(s5 + 1), 4.0);
            fx.vu_scale4(sp + 0x280, sp + 0x270, f);
            fx.vu_add4(sp + 0x280, sp + 0x280, D + 0x90);
            words(fx, t + 0x10 + 0x10 * s5 as u32, sp + 0x280);
        }
    }
    desc_reset(fx, sp);
    desc_setup(fx, sp, 0x14, true);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    let n = fx.mem.i32(D);
    let f21 = window(n, 0x99 - n);
    for i in 0..8u32 {
        let t = D + 0x48C0 + 0x50 * i;
        if fx.mem.read_u8(t) == 0 {
            continue;
        }
        let age = fx.mem.i32(t + 4);
        let cs = fx.tcos((age << 14) >> 3);
        let a = fix(fx, ee::mul(f21, ee::mul(64.0, cs)), 0x01E0_13C4);
        colour(fx, sp, [0xFF, 0x40, 0x18], a as u8);
        square(fx, sp, 2.0);
        for s6 in 0..4u32 {
            words(fx, sp + 0x270, t + 0x10 + 0x10 * s6);
            place(fx, sp);
            draw_far(fx, sp);
        }
        let age = fx.mem.i32(t + 4) + 1;
        fx.mem.set32(t + 4, age);
        if age >= 9 {
            fx.mem.write_u8(t, 0);
        }
    }
    let n = fx.mem.i32(D);
    let f3 = window(n, 0x12C - n);
    if crate::exe::ee_le(f3, 0.0) {
        return;
    }
    let parity = ee::from_i32(rem2(fx.mem.i32(D), 2));
    let f1 = ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), parity));
    let a = fix(fx, ee::mul(ee::mul(255.0, f3), f1), 0x01E0_1624);
    colour(fx, sp, [0xFF, 0x40, 0x18], a as u8);
    square(fx, sp, 4.0);
    words(fx, sp + 0x270, D + 0x80);
    place(fx, sp);
    draw_far(fx, sp);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x1F0);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x18 + 4 * k));
        fx.node_position(h, slot, D + 0x20 + 0x10 * k);
    }
    fx.vu_add4(D + 0x40, D + 0x20, D + 0x30);
    fx.vu_scale4(D + 0x40, D + 0x40, 0.5);
    words(fx, D + 0x90, D + 0x80);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x50 + 4 * k));
        fx.node_position(h, slot, D + 0x60 + 0x10 * k);
    }
    fx.vu_sub4(sp + 0x270, D + 0x70, D + 0x60);
    fx.vu_scale4(sp + 0x270, sp + 0x270, f32::from_bits(0x3FB3_3333));
    fx.vu_add4(D + 0x80, sp + 0x270, D + 0x60);
    if fx.mem.i32(D) == 0 {
        words(fx, D + 0x90, D + 0x80);
    }
    rings(fx, sp);
    bursts(fx, sp);
    puffs(fx, sp);
    trails(fx, sp);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0xAA {
        ret = 1;
    } else if (0xB5..0x149).contains(&n) {
        ret = 6;
    } else if n == 0x149 {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x14B {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}
