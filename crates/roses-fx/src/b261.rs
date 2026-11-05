//! BMODULE b261 (card 207), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xB560
//! bytes) [F]: a single unscheduled function in
//! a 0x350 frame on b150's helpers, every sprite a textured window sprite
//! (descriptor sp + 0x2F0: 5 registers 0x42421, prim 0x26 | 0x800, clamp
//! 5) through 0x204F70 / 0x206F30. Every update the camera turn with roll
//! (0x2055E0 into sp + 0x210), the attacker's nodes 0x3B / 0x3C (D+0x20 /
//! D+0x30) and their midpoint D+0x40; the target's point (0x1F65D0 kind
//! 1) D+0x50 is read once at the start. On updates 0x66..0x87 one of 32
//! fleck records (+0x60, 0x60) of four random flecks (0x101080, 0.4, y
//! halved) at the midpoint, each 32 updates spreading by the tan falloff
//! (half size 5 (0.2 + 0.8 age / 32), alpha 3 (32 - age), colour (8F, 84,
//! 80), ALPHA preset 0). On 0x81..0x153 one of 40 ring records (+0xC60,
//! 0x320) at node 0x3C lowered 1, of 32 random rays (1.5) with sizes 0.2
//! + 0.8 rand / 256, living 40 updates: each ray's sprite turned about z
//! by age x 0x22B (alternate records the other way) and pushed out by age
//! / 40 (z - 200 x side), half size 15 (0.4 + 0.6 age / 40), alpha 100 x
//! ratio_below(age, 4) x ramp_above(age, 40, 26) x the window fade
//! (clamp(n - 0x80) clamp(0x154 - n) / 1024), colour (FF, 38, 28). On
//! 0xA1..0x191 one of 16 burst records (+0x8960, 0x230) of 32 random
//! directions (1.3) round the target's point, 16 updates (spread 0.9 + 0.1
//! (16 - age) / 16, rising -25 + 80 age / 16, half size 15 (0.2 + 0.8 (16
//! - age) / 16), alpha 134 sin(age << 11) x the window fade (0xA0 /
//! 0x192), colour (FF, 39, 27)), drawn at the far Z with key 0. On
//! 0x81..0x153 one of 24 puff records (+0xAC60, 0x60) of four random
//! puffs (0.5) at the midpoint, 24 updates by the tan falloff (texture
//! D+0x14, half size 8 (0.2 + 0.8 (24 - age) / 24), alpha 128 sin((age <<
//! 15) / 24), colour (8F, 28, 1C)). Returns 1 on update 0x96, 6 on
//! 0xA1..0x190, 2 on 0x191, 8 once the count reaches 0x193.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The entry's s0 (0x206E40's return), rand's saved s0.
const S0: u32 = OT;
const FRAME: u32 = 0x350;
/// The sprite descriptor and its vertex fields.
const DESC: u32 = 0x2F0;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xC0, D);
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

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0xB560);
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
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x3B, 1);
    fx.mem.set32(D + 0x18, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x3C, 0);
    fx.mem.set32(D + 0x1C, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x50);
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x60 + 0x60 * k, 0);
    }
    for k in 0..0x28u32 {
        fx.mem.write_u8(D + 0xC60 + 0x320 * k, 0);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x8960 + 0x230 * k, 0);
    }
    for k in 0..0x18u32 {
        fx.mem.write_u8(D + 0xAC60 + 0x60 * k, 0);
    }
    // The last clear loop steps the stack copy of a0.
    fx.mem.write_u32(sp + 0xC0, D + 0x60 * 0x18);
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The 4 x 8 cell `b` of the texture into the sprite's two UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    let u0 = ((c << 9) & 0x3FFF) as u32;
    let u1 = ((((c << 5) + 0x20) << 4) & 0x3FFF) as u32;
    let v0 = ((r << 9) & 0x3FFF) as u32;
    let v1 = ((((r << 5) + 0x20) << 4) & 0x3FFF) as u32;
    uv(fx, sp + 0x308, u0);
    uv(fx, sp + 0x30A, v0);
    uv(fx, sp + 0x318, u1);
    uv(fx, sp + 0x31A, v1);
}

/// The view-projection (x identity) at sp + 0x190.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x190);
    fx.identity(sp + 0x1D0);
    let id = fx.read_m4(sp + 0x1D0);
    fx.mul_into(Some(sp + 0x190), &id);
}

/// The descriptor rewritten through its pointers (sp + 0x120 / 0x130 /
/// 0x140), | 0x800.
fn desc_reset(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// The corners (-s, -s, 0, 1) at sp + 0x250 and (s, s, 0, 1) at sp +
/// 0x260 turned by the camera (sp + 0x210).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x250, n);
    fx.mem.setf(sp + 0x254, n);
    fx.mem.write_u32(sp + 0x258, 0);
    fx.mem.write_u32(sp + 0x25C, 0x3F80_0000);
    fx.mem.setf(sp + 0x260, s);
    fx.mem.setf(sp + 0x264, s);
    fx.mem.write_u32(sp + 0x268, 0);
    fx.mem.write_u32(sp + 0x26C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x250, sp + 0x210, sp + 0x250);
    fx.vu_apply_to(sp + 0x260, sp + 0x210, sp + 0x260);
}

/// The colour (c, a, Q = 1.0) at sp + 0x300.
fn colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(sp + 0x300, c[0]);
    fx.mem.write_u8(sp + 0x301, c[1]);
    fx.mem.write_u8(sp + 0x302, c[2]);
    fx.mem.write_u8(sp + 0x303, a);
    fx.mem.write_u32(sp + 0x304, 0x3F80_0000);
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// The two vertices at sp + 0x270 / 0x280 = the corners + sp + 0x2D0.
fn place(fx: &mut Fx, sp: u32) {
    fx.vu_add4(sp + 0x270, sp + 0x250, sp + 0x2D0);
    fx.vu_add4(sp + 0x280, sp + 0x260, sp + 0x2D0);
}

/// Projected (0x204F70 into 0x2D3600 through sp + 0x190) and submitted
/// at the mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    if crate::fxlib2::project(fx, sp + 0x270, PROJ_OUT, sp + 0x190, sp + DESC, sp + 0x34C) {
        let w = fx.mem.f(sp + 0x34C);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// The far Z (0xFFFFF8) into the XYZ2 at `first`, then its Z into
/// `other` (the low word and the top byte kept).
fn far_z(fx: &mut Fx, first: u32, other: u32) {
    const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
    let v = fx.mem.read_u64(first);
    fx.mem.write_u64(first, (v & KEEP) | 0x00FF_FFF8_0000_0000);
    let z = ((fx.mem.read_u64(first) << 8) >> 40) << 32;
    let w = fx.mem.read_u64(other);
    fx.mem.write_u64(other, (w & KEEP) | z);
}

/// clamp(a, 0, 32) x clamp(b, 0, 32) / 1024.
fn window(a: i32, b: i32) -> f32 {
    let f = ee::from_i32(Fx::clamp_i(a, 0, 0x20));
    let g = ee::from_i32(Fx::clamp_i(b, 0, 0x20));
    ee::div(ee::mul(f, g), 1024.0)
}

/// The fleck records [F 0x1E002A8 / 0x1E00400].
fn flecks(fx: &mut Fx, sp: u32, n: i32) {
    if (0x66..0x88).contains(&n) {
        let f = D + 0x60 + 0x60 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(f, 1);
        fx.mem.write_u32(f + 4, 0);
        words(fx, f + 0x10, D + 0x40);
        for j in 0..4u32 {
            let r = fx.rand_from(S0, 0x01E0_030C) >> 23;
            fx.mem.write_u16(f + 8 + 2 * j, rem2(r, 8) as u16);
            let p = f + 0x20 + 0x10 * j;
            fx.random_table_vec(p, f32::from_bits(0x3ECC_CCCD));
            let y = ee::mul(fx.mem.f(p + 4), 0.5);
            fx.mem.setf(p + 4, y);
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
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x180, 0);
    for i in 0..0x20u32 {
        let f = D + 0x60 + 0x60 * i;
        if fx.mem.read_u8(f) != 0 {
            let age = fx.mem.i32(f + 4);
            let s = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(age)), 32.0);
            let s = ee::mul(5.0, ee::add(f32::from_bits(0x3E4C_CCCD), s));
            corners(fx, sp, s);
            let age = fx.mem.i32(f + 4);
            colour(fx, sp, [0x8F, 0x84, 0x80], ((0x20 - age) * 3) as u8);
            let age = fx.mem.i32(f + 4);
            let f20 = fx.tan_falloff_p(age, 0x20, 1);
            fx.mem.write_u32(sp + 0xD0, f + 0x10);
            for j in 0..4u32 {
                fx.vu_scale4(sp + 0x2D0, f + 0x20 + 0x10 * j, f20);
                fx.vu_add4(sp + 0x2D0, sp + 0x2D0, f + 0x10);
                place(fx, sp);
                let b = fx.mem.read_u16(f + 8 + 2 * j);
                cell_uvs(fx, sp, i32::from(b));
                draw(fx, sp, 0x01E0_0620);
            }
            let age = fx.mem.i32(f + 4) + 1;
            fx.mem.set32(f + 4, age);
            if age >= 0x21 {
                fx.mem.write_u8(f, 0);
            }
        }
        fx.mem.write_u32(sp + 0x180, i + 1);
    }
}

/// The ring records [F 0x1E006A4 / 0x1E007B8].
fn rings(fx: &mut Fx, sp: u32, n: i32) {
    if (0x81..0x154).contains(&n) {
        let (_, k) = fx.div(n, 0x28);
        let r = D + 0xC60 + 0x320 * k as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x110, D + 0x30);
        let y = ee::add(fx.mem.f(r + 0x114), -1.0);
        fx.mem.setf(r + 0x114, y);
        for j in 0..0x20u32 {
            let c = fx.rand_from(S0, 0x01E0_071C) >> 23;
            fx.mem.set32(r + 8 + 4 * j, rem2(c, 8));
            let q = fx.rand_from(S0, 0x01E0_073C) >> 23;
            let f = ee::from_i32(rem2(q, 256));
            let f = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), f), 256.0));
            fx.mem.setf(r + 0x88 + 4 * j, f);
            fx.random_table_vec(r + 0x120 + 0x10 * j, 1.5);
        }
    }
    desc_reset(fx, sp);
    let d = sp + DESC;
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = window(n - 0x80, 0x154 - n);
    fx.mem.write_u32(sp + 0x170, 0);
    for k in 0..0x28u32 {
        let r = D + 0xC60 + 0x320 * k;
        if fx.mem.read_u8(r) != 0 {
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f0 = Fx::ratio_below(age, 4.0);
            let f21 = ee::mul(100.0, f0);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f0 = Fx::ramp_above(age, 40.0, 26.0);
            let f0 = ee::mul(f21, f0);
            let a = fix(fx, ee::mul(f20, f0), 0x01E0_0900);
            colour(fx, sp, [0xFF, 0x38, 0x28], a as u8);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let s = ee::div(ee::mul(f32::from_bits(0x3F19_999A), age), 40.0);
            let s = ee::mul(15.0, ee::add(f32::from_bits(0x3ECC_CCCD), s));
            corners(fx, sp, s);
            let f21 = ee::div(ee::from_i32(fx.mem.i32(r + 4)), 40.0);
            let turn = fx.mult(rem2(k as i32, 2) * 2 - 1, 0x22B);
            fx.mem.set32(sp + 0xE0, turn);
            fx.mem.write_u32(sp + 0xF0, r + 0x110);
            for j in 0..0x20u32 {
                let b = fx.mem.i32(r + 8 + 4 * j);
                cell_uvs(fx, sp, b);
                let sz = fx.mem.f(r + 0x88 + 4 * j);
                fx.vu_scale4(sp + 0x270, sp + 0x250, sz);
                let sz = fx.mem.f(r + 0x88 + 4 * j);
                fx.vu_scale4(sp + 0x280, sp + 0x260, sz);
                fx.identity(sp + 0x290);
                let age = fx.mem.i32(r + 4);
                let turn = fx.mem.i32(sp + 0xE0);
                let a = fx.mult(age, turn);
                fx.rotate_axis(sp + 0x290, a, 2);
                fx.vu_apply_to(sp + 0x2E0, sp + 0x290, r + 0x120 + 0x10 * j);
                let x = ee::mul(fx.mem.f(sp + 0x2E0), f21);
                fx.mem.setf(sp + 0x2D0, x);
                let side = ee::from_i32(fx.mem.i32(D + 0xC));
                let y = ee::mul(fx.mem.f(sp + 0x2E4), f21);
                let z = ee::add(fx.mem.f(sp + 0x2E8), ee::mul(-200.0, side));
                fx.mem.setf(sp + 0x2D8, ee::mul(f21, z));
                fx.mem.setf(sp + 0x2D4, y);
                fx.vu_add4(sp + 0x2D0, sp + 0x2D0, r + 0x110);
                fx.vu_add4(sp + 0x270, sp + 0x270, sp + 0x2D0);
                fx.vu_add4(sp + 0x280, sp + 0x280, sp + 0x2D0);
                draw(fx, sp, 0x01E0_0B98);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x29 {
                fx.mem.write_u8(r, 0);
            }
        }
        fx.mem.write_u32(sp + 0x170, k + 1);
    }
}

/// The burst records [F 0x1E00C1C / 0x1E00CC8], at the far Z.
fn bursts(fx: &mut Fx, sp: u32, n: i32) {
    if (0xA1..0x192).contains(&n) {
        let q = D + 0x8960 + 0x230 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(q, 1);
        fx.mem.write_u32(q + 4, 0);
        for j in 0..0x20u32 {
            let r = fx.rand_from(S0, 0x01E0_0C78) >> 23;
            fx.mem.set8(q + 8 + j, rem2(r, 8));
            fx.random_table_vec(q + 0x30 + 0x10 * j, f32::from_bits(0x3FA6_6666));
        }
    }
    desc_reset(fx, sp);
    let d = sp + DESC;
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = window(n - 0xA0, 0x192 - n);
    fx.mem.write_u32(sp + 0x160, 0);
    fx.mem.write_u32(sp + 0x100, D);
    for i in 0..0x10u32 {
        let q = D + 0x8960 + 0x230 * i;
        if fx.mem.read_u8(q) != 0 {
            let age = fx.mem.i32(q + 4);
            let sn = fx.tsin((age << 15) >> 4);
            let a = fix(fx, ee::mul(f20, ee::mul(134.0, sn)), 0x01E0_0E0C);
            colour(fx, sp, [0xFF, 0x39, 0x27], a as u8);
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
                fx.mem.setf(sp + 0x2D0, x);
                let y = ee::add(f22, fx.mem.f(p + 4));
                fx.mem.setf(sp + 0x2D4, y);
                let z = ee::mul(fx.mem.f(p + 8), f21);
                fx.mem.setf(sp + 0x2D8, z);
                fx.vu_add4(sp + 0x2D0, sp + 0x2D0, D + 0x50);
                place(fx, sp);
                if crate::fxlib2::project(fx, sp + 0x270, PROJ_OUT, sp + 0x190, d, sp + 0x34C) {
                    far_z(fx, sp + 0x310, sp + 0x320);
                    fx.submit(d, OT, 0);
                }
            }
            let age = fx.mem.i32(q + 4) + 1;
            fx.mem.set32(q + 4, age);
            if age >= 0x11 {
                fx.mem.write_u8(q, 0);
            }
        }
        fx.mem.write_u32(sp + 0x100, D + 0x230 * (i + 1));
        fx.mem.write_u32(sp + 0x160, i + 1);
    }
}

/// The puff records [F 0x1E011B8 / 0x1E01278].
fn puffs(fx: &mut Fx, sp: u32, n: i32) {
    if (0x81..0x154).contains(&n) {
        let (_, k) = fx.div(n, 0x18);
        let p = D + 0xAC60 + 0x60 * k as u32;
        fx.mem.write_u8(p, 1);
        fx.mem.write_u32(p + 4, 0);
        words(fx, p + 0x10, D + 0x40);
        for j in 0..4u32 {
            let r = fx.rand_from(S0, 0x01E0_122C) >> 23;
            fx.mem.write_u16(p + 8 + 2 * j, rem2(r, 4) as u16);
            fx.random_table_vec(p + 0x20 + 0x10 * j, 0.5);
        }
    }
    desc_reset(fx, sp);
    let d = sp + DESC;
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x150, 0);
    for i in 0..0x18u32 {
        let p = D + 0xAC60 + 0x60 * i;
        if fx.mem.read_u8(p) != 0 {
            let age = fx.mem.i32(p + 4);
            let s = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(0x18 - age)), 24.0);
            let s = ee::mul(8.0, ee::add(f32::from_bits(0x3E4C_CCCD), s));
            corners(fx, sp, s);
            let x = fx.mem.i32(p + 4) << 15;
            fx.mult(0x2AAA_AAAB, x);
            let hi = fx.hilo().1;
            let a = (hi >> 2).wrapping_add(((x as u32) >> 31) as i32);
            let sn = fx.tsin(a);
            let a = fix(fx, ee::mul(128.0, sn), 0x01E0_1418);
            colour(fx, sp, [0x8F, 0x28, 0x1C], a as u8);
            let age = fx.mem.i32(p + 4);
            let f20 = fx.tan_falloff_p(age, 0x18, 1);
            fx.mem.write_u32(sp + 0x110, p + 0x10);
            for j in 0..4u32 {
                fx.vu_scale4(sp + 0x2D0, p + 0x20 + 0x10 * j, f20);
                fx.vu_add4(sp + 0x2D0, sp + 0x2D0, p + 0x10);
                place(fx, sp);
                let b = fx.mem.read_u16(p + 8 + 2 * j);
                cell_uvs(fx, sp, i32::from(b));
                draw(fx, sp, 0x01E0_157C);
            }
            let age = fx.mem.i32(p + 4) + 1;
            fx.mem.set32(p + 4, age);
            if age >= 0x19 {
                fx.mem.write_u8(p, 0);
            }
        }
        fx.mem.write_u32(sp + 0x150, i + 1);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x210);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x18 + 4 * k));
        fx.node_position(h, slot, D + 0x20 + 0x10 * k);
    }
    fx.vu_add4(D + 0x40, D + 0x20, D + 0x30);
    fx.vu_scale4(D + 0x40, D + 0x40, 0.5);
    let n = fx.mem.i32(D);
    flecks(fx, sp, n);
    let n = fx.mem.i32(D);
    rings(fx, sp, n);
    let n = fx.mem.i32(D);
    bursts(fx, sp, n);
    let n = fx.mem.i32(D);
    puffs(fx, sp, n);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x96 {
        ret = 1;
    } else if (0xA1..0x191).contains(&n) {
        ret = 6;
    } else if n == 0x191 {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x193 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}
