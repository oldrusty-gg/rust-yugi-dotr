//! BMODULE b599 (card 356), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000 kept in
//! s4, 0x7870 bytes) [F]: a single unscheduled
//! function in a 0x320 frame on b150's helpers (textured window sprites,
//! descriptor sp + 0x2C0: 5 registers 0x42421, prim 0x26 | 0x800, clamp
//! 5), its random directions picked straight from 0x101080's table
//! 0x28AE90 (row rand % 4, entry rand % 32, component rand % 3, three rand
//! calls a component). Every update the camera turn with roll (0x2055E0
//! into sp + 0x230), the attacker's nodes 0x4F / 0x59 (D+0x20 / D+0x30)
//! and their midpoint D+0x40, node 0xF (D+0x60), node 0x70 into D+0x80 on
//! update 0x8C and into D+0x90 on 0xDC. On updates 0x27 / 0x31 / 0x3B /
//! 0x63 / 0x6D / 0x77 / 0x81 one of 10 dust records (+0xB0, 0x260) of 32
//! sprites at the midpoint along (0.5 x, 0.2 |y|, 0.5 z) table directions,
//! 32 updates (grey, ALPHA preset 0, half size 3 (0.5 + 0.5 age / 32),
//! spread by the tan falloff, alpha 3 (32 - age), table UVs). On
//! 0x8D..0x15D one of 32 glints (+0x1870, 0x20) at node 0xF plus 2 x a
//! table direction, 32 updates (texture D+0x10, ALPHA preset 0, half size
//! 6 (0.5 + 0.5 age / 32), alpha 64 sin(age << 10)). On 0x8D..0xA7 one of
//! 32 flare records (+0x1C70, 0x90) of 8 sprites from D+0x80 along (2 x,
//! |2 y|, 2 z), 32 updates (texture D+0x14, half size 2, colour (FF, E8,
//! E0), alpha 164 sin(age << 10), spread x / z 1 - 0.7 tan falloff, y (30
//! + y) (1 - tan falloff), the whole texture). On 0xDD..0x167 one of 32
//! ring records (+0x2E70, 0x250) of 24 sprites at D+0x90 with sizes 0.2 +
//! 0.8 rand / 256 along 3 x table directions, 32 updates (half size 18
//! (0.5 + 0.5 age / 32), pushed by 0.1 + 0.9 age / 32 (z by age / 32 x (z -
//! 150 x side) - 10 x side), alpha 128 cos(age << 9) x the fade out (1 -
//! (clamp(n, 0x148, 0x168) - 0x148) / 32), colour (80, 34, 28), table
//! UVs). The stage light 0x80 - 64 clamp(n - 0x8C) clamp(0x190 - n) / 1024
//! (0x1F7F10). Returns 6 while 230.67 < n < 0x168, 2 on 0x168, 8 once the
//! count reaches 0x191 (1 when n as a float is 220.67: never).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x320;
const DESC: u32 = 0x2C0;
/// The UV table 0x28AE70 (four (u0, u1, v0, v1) halfword cells).
const UV_TABLE: u32 = 0x0028_AE70;
/// 0x101080's direction table (4 rows of 32 vec3).
const DIR_TABLE: u32 = 0x0028_AE90;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0x110, 0);
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
    fx.bzero(D, 0x7870);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 8, s);
    for k in 0..3u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xC + 4 * k, r);
    }
    for (node, clear, at) in [(0x4F, 1, 0x18u32), (0x59, 0, 0x1C), (0xF, 0, 0x50), (0x70, 0, 0x70)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.write_u32(D + 0xA0, 0);
    for k in 0..0xAu32 {
        fx.mem.write_u8(D + 0xB0 + 0x260 * k, 0);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x1870 + 0x20 * k, 0);
    }
    for (o, w) in [(0x80u32, 0u32), (0x84, 0), (0x88, 0), (0x8C, 0x3F80_0000)] {
        fx.mem.write_u32(D + o, w);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x1C70 + 0x90 * k, 0);
    }
    for (o, w) in [(0x90u32, 0u32), (0x94, 0), (0x98, 0), (0x9C, 0x3F80_0000)] {
        fx.mem.write_u32(D + o, w);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x2E70 + 0x250 * k, 0);
    }
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x2D8, u0);
    uv(fx, sp + 0x2DA, v0);
    uv(fx, sp + 0x2E8, u1);
    uv(fx, sp + 0x2EA, v1);
}

/// UV cell `i` of the table 0x28AE70.
fn table_uvs(fx: &mut Fx, sp: u32, i: u32) {
    let t = UV_TABLE.wrapping_add(i << 3);
    let (u0, u1, v0, v1) = (fx.mem.read_u16(t), fx.mem.read_u16(t + 2), fx.mem.read_u16(t + 4), fx.mem.read_u16(t + 6));
    uvs(fx, sp, u32::from(u0), u32::from(v0), u32::from(u1), u32::from(v1));
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// 0x1A7230 fabsf (its argument left at its sp).
fn fabsf(fx: &mut Fx, x: f32) -> f32 {
    fx.mem.write_f32(fx.sp - 0x10, x);
    f32::from_bits(x.to_bits() & 0x7FFF_FFFF)
}

/// rand % 4 (a row), rand % 32 (an entry), rand % 3 (a component) of the
/// direction table: its address, the two partial addresses on the way.
fn table_pick(fx: &mut Fx, s0: u32, ra: [u32; 3]) -> (u32, u32, u32) {
    let r = fx.rand_from(s0, ra[0]) >> 23;
    let row = DIR_TABLE.wrapping_add((rem2(r, 4) * 3 * 0x80) as u32);
    let r = fx.rand_from(s0, ra[1]) >> 23;
    let entry = row.wrapping_add((rem2(r, 0x20) * 12) as u32);
    let r = fx.rand_from(s0, ra[2]) >> 23;
    let (_, q) = fx.div(r, 3);
    (row, entry, entry.wrapping_add((q * 4) as u32))
}

fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x1B0);
    fx.identity(sp + 0x1F0);
    let id = fx.read_m4(sp + 0x1F0);
    fx.mul_into(Some(sp + 0x1B0), &id);
}

/// The descriptor rewritten through its pointers, | 0x800, ALPHA preset
/// `preset`, the texture, clamp 5, the camera.
fn desc_reset(fx: &mut Fx, sp: u32, preset: i32, rh_at: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(preset);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + rh_at);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
}

/// The corners (-s, -s, 0, 1) at sp + 0x270 and (s, s, 0, 1) at sp +
/// 0x280 turned by the camera (sp + 0x230).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x270, n);
    fx.mem.setf(sp + 0x274, n);
    fx.mem.write_u32(sp + 0x278, 0);
    fx.mem.write_u32(sp + 0x27C, 0x3F80_0000);
    fx.mem.setf(sp + 0x280, s);
    fx.mem.setf(sp + 0x284, s);
    fx.mem.write_u32(sp + 0x288, 0);
    fx.mem.write_u32(sp + 0x28C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x270, sp + 0x230, sp + 0x270);
    fx.vu_apply_to(sp + 0x280, sp + 0x230, sp + 0x280);
}

fn colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(sp + 0x2D0, c[0]);
    fx.mem.write_u8(sp + 0x2D1, c[1]);
    fx.mem.write_u8(sp + 0x2D2, c[2]);
    fx.mem.write_u8(sp + 0x2D3, a);
    fx.mem.write_u32(sp + 0x2D4, 0x3F80_0000);
}

/// Projected (sp + 0x290 / 0x2A0) and submitted at the mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    if crate::fxlib2::project(fx, sp + 0x290, PROJ_OUT, sp + 0x1B0, sp + DESC, sp + 0x31C) {
        let w = fx.mem.f(sp + 0x31C);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// The vertices = the corners + sp + 0x2B0.
fn place(fx: &mut Fx, sp: u32) {
    fx.vu_add4(sp + 0x290, sp + 0x270, sp + 0x2B0);
    fx.vu_add4(sp + 0x2A0, sp + 0x280, sp + 0x2B0);
}

/// The dust records [F 0x1E002E0 / 0x1E005B8].
fn dust(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if [0x81, 0x77, 0x6D, 0x63, 0x3B, 0x31, 0x27].contains(&n) {
        let k = fx.mem.i32(D + 0xA0) as u32;
        let base = D + 0x260 * k;
        let a = base + 0xB0;
        fx.mem.write_u8(a, 1);
        fx.mem.write_u32(a + 4, 1);
        words(fx, a + 0x50, D + 0x40);
        for j in 0..0x20u32 {
            let s0 = base + 2 * j;
            let r = fx.rand_from(s0, 0x01E0_0398) >> 23;
            fx.mem.write_u16(a + 8 + 2 * j, rem2(r, 4) as u16);
            let p = a + 0x60 + 0x10 * j;
            let (_, _, t) = table_pick(fx, s0, [0x01E0_03B8, 0x01E0_03EC, 0x01E0_0418]);
            let x = ee::mul(0.5, fx.mem.f(t));
            fx.mem.setf(p, x);
            let (_, _, t) = table_pick(fx, s0, [0x01E0_0448, 0x01E0_047C, 0x01E0_04A8]);
            let y = fx.mem.f(t);
            let y = fabsf(fx, y);
            fx.mem.setf(p + 4, ee::mul(f32::from_bits(0x3E4C_CCCD), y));
            let (_, _, t) = table_pick(fx, s0, [0x01E0_04EC, 0x01E0_0520, 0x01E0_054C]);
            let z = ee::mul(0.5, fx.mem.f(t));
            fx.mem.setf(p + 8, z);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
        let k = fx.mem.i32(D + 0xA0) + 1;
        fx.mem.set32(D + 0xA0, k);
        if k >= 0xA {
            fx.mem.write_u32(D + 0xA0, 0);
        }
    }
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u32(sp + 0x150, d + 2);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(sp + 0x160, d + 8);
    fx.mem.write_u32(sp + 0x170, d + 4);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0xC);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x1A0, 0);
    fx.mem.write_u32(sp + 0x130, D);
    for k in 0..0xAu32 {
        let base = D + 0x260 * k;
        let a = base + 0xB0;
        if fx.mem.read_u8(a) != 0 {
            let age = fx.mem.i32(a + 4);
            colour(fx, sp, [0x80, 0x80, 0x80], ((0x20 - age) * 3) as u8);
            let age = ee::from_i32(fx.mem.i32(a + 4));
            let s = ee::mul(3.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0)));
            corners(fx, sp, s);
            let age = fx.mem.i32(a + 4);
            let f20 = fx.tan_falloff_p(age, 0x20, 1);
            fx.mem.write_u32(sp + 0x120, a + 0x50);
            for j in 0..0x20u32 {
                fx.vu_scale4(sp + 0x2B0, a + 0x60 + 0x10 * j, f20);
                fx.vu_add4(sp + 0x2B0, sp + 0x2B0, a + 0x50);
                place(fx, sp);
                let c = fx.mem.read_u16(a + 8 + 2 * j);
                table_uvs(fx, sp, u32::from(c));
                draw(fx, sp, 0x01E0_089C);
            }
            let age = fx.mem.i32(a + 4) + 1;
            fx.mem.set32(a + 4, age);
            if age >= 0x21 {
                fx.mem.write_u8(a, 0);
            }
        }
        fx.mem.write_u32(sp + 0x130, base + 0x260);
        fx.mem.write_u32(sp + 0x1A0, k + 1);
    }
}

/// The glints [F 0x1E00918 / 0x1E00B64].
fn glints(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let s0 = sp + 0x2DA;
    if (0x8D..0x15E).contains(&n) {
        let b = D + 0x1870 + 0x20 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(b, 1);
        fx.mem.write_u32(b + 4, 0);
        let r = fx.rand_from(s0, 0x01E0_0960) >> 23;
        fx.mem.write_u16(b + 8, rem2(r, 0x10) as u16);
        let (_, _, t) = table_pick(fx, s0, [0x01E0_0984, 0x01E0_09B8, 0x01E0_09E4]);
        let x = ee::mul(2.0, fx.mem.f(t));
        fx.mem.setf(b + 0x10, x);
        let (_, _, t) = table_pick(fx, s0, [0x01E0_0A20, 0x01E0_0A54, 0x01E0_0A80]);
        let y = ee::mul(2.0, fx.mem.f(t));
        fx.mem.setf(b + 0x14, y);
        let (_, _, t) = table_pick(fx, s0, [0x01E0_0ABC, 0x01E0_0AF0, 0x01E0_0B1C]);
        let z = ee::mul(2.0, fx.mem.f(t));
        fx.mem.setf(b + 0x18, z);
        fx.mem.write_u32(b + 0x1C, 0x3F80_0000);
        fx.vu_add4(b + 0x10, b + 0x10, D + 0x60);
    }
    desc_reset(fx, sp, 0, 0x10);
    camera(fx, sp);
    for i in 0..0x20u32 {
        let b = D + 0x1870 + 0x20 * i;
        if fx.mem.read_u8(b) == 0 {
            continue;
        }
        let age = fx.mem.i32(b + 4);
        let sn = fx.tsin((age << 15) >> 5);
        let a = fix(fx, ee::mul(64.0, sn), 0x01E0_0C28);
        colour(fx, sp, [0x80, 0x80, 0x80], a as u8);
        let age = ee::from_i32(fx.mem.i32(b + 4));
        let s = ee::mul(6.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0)));
        corners(fx, sp, s);
        fx.vu_add4(sp + 0x290, sp + 0x270, b + 0x10);
        fx.vu_add4(sp + 0x2A0, sp + 0x280, b + 0x10);
        let cb = u32::from(fx.mem.read_u16(b + 8));
        let (c, r) = (cb & 3, cb >> 2);
        uvs(fx, sp, (c << 9) & 0x3FFF, (r << 9) & 0x3FFF, (((c << 5) + 0x20) << 4) & 0x3FFF, (((r << 5) + 0x20) << 4) & 0x3FFF);
        draw(fx, sp, 0x01E0_0DB0);
        let age = fx.mem.i32(b + 4) + 1;
        fx.mem.set32(b + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(b, 0);
        }
    }
}

/// The flare records [F 0x1E00DF0 / 0x1E01018].
fn flares(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let s0 = sp + 0x2DA;
    if (0x8D..0xA8).contains(&n) {
        let c = D + 0x1C70 + 0x90 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(c, 1);
        fx.mem.write_u32(c + 4, 0);
        for j in 0..8u32 {
            let p = c + 0x10 + 0x10 * j;
            let (_, _, t) = table_pick(fx, s0, [0x01E0_0E48, 0x01E0_0E7C, 0x01E0_0EA8]);
            let x = ee::mul(2.0, fx.mem.f(t));
            fx.mem.setf(p, x);
            let (_, _, t) = table_pick(fx, s0, [0x01E0_0ED8, 0x01E0_0F0C, 0x01E0_0F38]);
            let y = ee::mul(2.0, fx.mem.f(t));
            let y = fabsf(fx, y);
            fx.mem.setf(p + 4, y);
            let (_, _, t) = table_pick(fx, s0, [0x01E0_0F6C, 0x01E0_0FA0, 0x01E0_0FCC]);
            let z = ee::mul(2.0, fx.mem.f(t));
            fx.mem.setf(p + 8, z);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    uvs(fx, sp, 0, 0, 0x800, 0x800);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x190, 0);
    for i in 0..0x20u32 {
        let c = D + 0x1C70 + 0x90 * i;
        if fx.mem.read_u8(c) != 0 {
            let age = fx.mem.i32(c + 4);
            let sn = fx.tsin((age << 15) >> 5);
            let a = fix(fx, ee::mul(164.0, sn), 0x01E0_1144);
            colour(fx, sp, [0xFF, 0xE8, 0xE0], a as u8);
            corners(fx, sp, 2.0);
            let age = fx.mem.i32(c + 4);
            let f0 = fx.tan_falloff_p(age, 0x20, 1);
            let f21 = ee::sub(1.0, f0);
            let age = fx.mem.i32(c + 4);
            let f0 = fx.tan_falloff_p(age, 0x20, 1);
            let f22 = ee::sub(1.0, ee::mul(f32::from_bits(0x3F33_3333), f0));
            for j in 0..8u32 {
                let p = c + 0x10 + 0x10 * j;
                let x = ee::mul(fx.mem.f(p), f22);
                fx.mem.setf(sp + 0x2B0, x);
                let y = ee::mul(ee::add(30.0, fx.mem.f(p + 4)), f21);
                fx.mem.setf(sp + 0x2B4, y);
                let z = ee::mul(fx.mem.f(p + 8), f22);
                fx.mem.setf(sp + 0x2B8, z);
                fx.vu_add4(sp + 0x2B0, sp + 0x2B0, D + 0x80);
                place(fx, sp);
                draw(fx, sp, 0x01E0_1290);
            }
            let age = fx.mem.i32(c + 4) + 1;
            fx.mem.set32(c + 4, age);
            if age >= 0x21 {
                fx.mem.write_u8(c, 0);
            }
        }
        fx.mem.write_u32(sp + 0x190, i + 1);
    }
}

/// A partial table address kept by `sq` (the upper 64 bits [H] 0).
fn sq(fx: &mut Fx, at: u32, v: u32) {
    fx.mem.write_u64(at, u64::from(v));
    fx.mem.write_u64(at + 8, 0);
}

/// The ring records [F 0x1E012EC / 0x1E015C8].
fn rings(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let s0 = sp + 0x2DA;
    if (0xDD..0x168).contains(&n) {
        let base = D + 0x250 * rem2(n, 0x20) as u32;
        let e = base + 0x2E70;
        fx.mem.write_u8(e, 1);
        fx.mem.write_u32(e + 4, 0);
        for j in 0..0x18u32 {
            let r = fx.rand_from(s0, 0x01E0_1350) >> 23;
            fx.mem.set32(e + 8 + 4 * j, rem2(r, 4));
            let r = fx.rand_from(s0, 0x01E0_1370) >> 23;
            let f = ee::from_i32(rem2(r, 256));
            let f = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), f), 256.0));
            fx.mem.setf(e + 0x68 + 4 * j, f);
            let p = e + 0xD0 + 0x10 * j;
            for (c, ra, slots) in [(0u32, [0x01E0_13CC, 0x01E0_1404, 0x01E0_1438], [0x100u32, 0xF0]), (4, [0x01E0_146C, 0x01E0_14A4, 0x01E0_14D8], [0xE0, 0xD0]), (8, [0x01E0_150C, 0x01E0_1544, 0x01E0_1578], [0xC0, 0xB0])] {
                let (row, entry, t) = table_pick(fx, s0, ra);
                sq(fx, sp + slots[0], row);
                sq(fx, sp + slots[1], entry);
                let v = ee::mul(3.0, fx.mem.f(t));
                fx.mem.setf(p + c, v);
            }
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    desc_reset(fx, sp, 1, 0xC);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let c = Fx::clamp_i(n, 0x148, 0x168) - 0x148;
    let f20 = ee::sub(1.0, ee::div(ee::from_i32(c), 32.0));
    fx.mem.write_u32(sp + 0x180, 0);
    fx.mem.write_u32(sp + 0x140, D);
    for i in 0..0x20u32 {
        let base = D + 0x250 * i;
        let e = base + 0x2E70;
        if fx.mem.read_u8(e) != 0 {
            let age = fx.mem.i32(e + 4);
            let cs = fx.tcos((age << 14) >> 5);
            let a = fix(fx, ee::mul(f20, ee::mul(128.0, cs)), 0x01E0_16D4);
            colour(fx, sp, [0x80, 0x34, 0x28], a as u8);
            let age = ee::from_i32(fx.mem.i32(e + 4));
            let s = ee::mul(18.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0)));
            corners(fx, sp, s);
            let age = ee::from_i32(fx.mem.i32(e + 4));
            let f21 = ee::div(age, 32.0);
            let f22 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F66_6666), age), 32.0));
            for j in 0..0x18u32 {
                let cell = fx.mem.read_u32(e + 8 + 4 * j);
                table_uvs(fx, sp, cell);
                let sz = fx.mem.f(e + 0x68 + 4 * j);
                fx.vu_scale4(sp + 0x290, sp + 0x270, sz);
                let sz = fx.mem.f(e + 0x68 + 4 * j);
                fx.vu_scale4(sp + 0x2A0, sp + 0x280, sz);
                let p = e + 0xD0 + 0x10 * j;
                let x = ee::mul(fx.mem.f(p), f22);
                fx.mem.setf(sp + 0x2B0, x);
                let side = ee::from_i32(fx.mem.i32(D + 8));
                let z = ee::add(fx.mem.f(p + 8), ee::mul(-150.0, side));
                // mula.s / madd.s: the products rounded, then the sum.
                let z = ee::add(ee::mul(f21, z), ee::mul(-10.0, side));
                fx.mem.setf(sp + 0x2B8, z);
                let y = ee::mul(fx.mem.f(p + 4), f22);
                fx.mem.setf(sp + 0x2B4, y);
                fx.vu_add4(sp + 0x2B0, sp + 0x2B0, D + 0x90);
                fx.vu_add4(sp + 0x290, sp + 0x290, sp + 0x2B0);
                fx.vu_add4(sp + 0x2A0, sp + 0x2A0, sp + 0x2B0);
                draw(fx, sp, 0x01E0_1964);
            }
            let age = fx.mem.i32(e + 4) + 1;
            fx.mem.set32(e + 4, age);
            if age >= 0x21 {
                fx.mem.write_u8(e, 0);
            }
        }
        fx.mem.write_u32(sp + 0x140, base + 0x250);
        fx.mem.write_u32(sp + 0x180, i + 1);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x230);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x18 + 4 * k));
        fx.node_position(h, slot, D + 0x20 + 0x10 * k);
    }
    fx.vu_add4(D + 0x40, D + 0x20, D + 0x30);
    fx.vu_scale4(D + 0x40, D + 0x40, 0.5);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x50));
    fx.node_position(h, slot, D + 0x60);
    if fx.mem.i32(D) == 0x8C {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x70));
        fx.node_position(h, slot, D + 0x80);
    }
    if fx.mem.i32(D) == 0xDC {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x70));
        fx.node_position(h, slot, D + 0x90);
    }
    dust(fx, sp);
    glints(fx, sp);
    flares(fx, sp);
    rings(fx, sp);
    let n = fx.mem.i32(D);
    let c1 = Fx::clamp_i(0x190 - n, 0, 0x20);
    let c2 = Fx::clamp_i(n - 0x8C, 0, 0x20);
    let l = fx.mult(c1, c2.wrapping_neg() << 6);
    fx.set_stage_light(((l >> 10) + 0x80) & 0xFF);
    let n = fx.mem.i32(D);
    let nf = ee::from_i32(n);
    let mut ret = 0;
    if nf == f32::from_bits(0x435C_AAAB) {
        ret = 1;
    } else {
        if !crate::exe::ee_le(nf, f32::from_bits(0x4366_AAAB)) && n < 0x168 {
            ret = 6;
        }
        if n == 0x168 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x191 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0x110, ret as u32);
    ret
}
