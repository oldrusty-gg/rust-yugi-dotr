//! BMODULE b150 (card 168), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xD560
//! bytes) [F]: a single unscheduled function
//! with its own particles, drawn as textured sprites (descriptor at sp +
//! 0x2D0: RGBAQ, UV / XYZ2 x 2, prim 0x26) through 0x204F70 / 0x206F30.
//! A 0x360 frame; the update count is the word D+0 (counted at the end).
//! Every update the camera turn with roll (0x2055E0 into sp + 0x240), the
//! attacker's nodes 0x1B / 0x1C (D+0x40 / D+0x50) and eight points from
//! the first towards twice the second (D+0x60..). Updates 1..209: one of
//! 16 groups (+0xE0, 0x2B0) of 8 x 4 random flecks (0x101080, 0.1) at
//! those points, each 16 updates rising 15 and spreading (reddish, alpha
//! 128 cos x the in / out ramp); 134..180 one of 8 spawners (+0x2BE0,
//! 0xC) sweeping 16-fleck bursts (+0x2C40, 0x130, 8 x 16) in front of the
//! attacker, each rising 50; 197 a full-window pink tint (+0xC440, 64
//! updates, 1 - tan falloff) and 256 random rays (+0xC458 cells, +0xC560
//! vectors x 5) of eight sprites each from the target, growing by the
//! tan falloff. Returns 1 on update 123, 6 on 134..204, 2 on 205, 8 once
//! the count reaches 262.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The UV table 0x28AE70 (four (u0, u1, v0, v1) halfword cells).
const UV_TABLE: u32 = 0x0028_AE70;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const TINT: u32 = D + 0xC440;
const RAYS: u32 = D + 0xC450;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x360);
    fx.mem.write_u32(sp + 0xBC, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(0x360);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xD560);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    let r = fx.texture(5, 0);
    fx.mem.write_u32(D + 0x10, r);
    let r = fx.texture(5, 1);
    fx.mem.write_u32(D + 0x14, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x1B, 1);
    fx.mem.set32(D + 0x30, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x1C, 0);
    fx.mem.set32(D + 0x34, r);
    for k in 0..16u32 {
        fx.mem.write_u8(D + 0xE0 + 0x2B0 * k, 0);
    }
    for k in 0..8u32 {
        fx.mem.write_u8(D + 0x2BE0 + 0xC * k, 0);
        fx.mem.write_u8(D + 0x2BE8 + 0xC * k, 0);
    }
    for j in 0..8u32 {
        for i in 0..16u32 {
            fx.mem.write_u8(D + 0x2C40 + 0x1300 * j + 0x130 * i, 0);
        }
    }
    fx.mem.write_u8(TINT, 0);
    fx.mem.write_u8(RAYS, 0);
    0
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

/// The sprite descriptor at sp + 0x2D0 (5 registers 0x42421, prim 0x26 |
/// 0x800) with the texture `rh2`, ALPHA preset 1, CLAMP 5, and the
/// camera's view-projection (x identity) at sp + 0x1C0.
fn sprite_desc(fx: &mut Fx, sp: u32, rh2: u32) {
    let d = sp + 0x2D0;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    fx.desc_texture(d, rh2, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
    fx.camera_vp_to(sp + 0x1C0);
    fx.identity(sp + 0x200);
    let id = fx.read_m4(sp + 0x200);
    fx.mul_into(Some(sp + 0x1C0), &id);
}

/// UV0 / UV1 of the sprite: (h & 0xC000) | (v & 0x3FFF) at sp + 0x2E8 /
/// 0x2EA / 0x2F8 / 0x2FA.
fn set_uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    for (a, v) in [(0x2E8u32, u0), (0x2EA, v0), (0x2F8, u1), (0x2FA, v1)] {
        let h = fx.mem.read_u16(sp + a);
        fx.mem.write_u16(sp + a, (h & 0xC000) | (v & 0x3FFF) as u16);
    }
}

/// UV cell `i` of the table 0x28AE70.
fn table_uvs(fx: &mut Fx, sp: u32, i: u8) {
    let t = UV_TABLE + 8 * u32::from(i);
    let (u0, v0, u1, v1) = (fx.mem.read_u16(t), fx.mem.read_u16(t + 4), fx.mem.read_u16(t + 2), fx.mem.read_u16(t + 6));
    set_uvs(fx, sp, u32::from(u0), u32::from(v0), u32::from(u1), u32::from(v1));
}

fn colour(fx: &mut Fx, sp: u32, rgb: [u8; 3], a: u32) {
    fx.mem.write_u8(sp + 0x2E0, rgb[0]);
    fx.mem.write_u8(sp + 0x2E1, rgb[1]);
    fx.mem.write_u8(sp + 0x2E2, rgb[2]);
    fx.mem.write_u8(sp + 0x2E3, a as u8);
    fx.mem.write_u32(sp + 0x2E4, 0x3F80_0000);
}

/// The corner offsets (-s, -s, 0, 1) / (s, s, 0, 1) at sp + 0x280 / 0x290
/// turned by the camera (sp + 0x240).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x280, n);
    fx.mem.setf(sp + 0x284, n);
    fx.mem.write_u32(sp + 0x288, 0);
    fx.mem.write_u32(sp + 0x28C, 0x3F80_0000);
    fx.mem.setf(sp + 0x290, s);
    fx.mem.setf(sp + 0x294, s);
    fx.mem.write_u32(sp + 0x298, 0);
    fx.mem.write_u32(sp + 0x29C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x280, sp + 0x240, sp + 0x280);
    fx.vu_apply_to(sp + 0x290, sp + 0x240, sp + 0x290);
}

/// The sprite about the centre sp + 0x2C0: corners sp + 0x2A0 / 0x2B0,
/// projected (0x204F70) and submitted at the mean w (0x1A2C70 from `ra`).
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    fx.vu_add4(sp + 0x2A0, sp + 0x280, sp + 0x2C0);
    fx.vu_add4(sp + 0x2B0, sp + 0x290, sp + 0x2C0);
    let d = sp + 0x2D0;
    if crate::fxlib2::project(fx, sp + 0x2A0, PROJ_OUT, sp + 0x1C0, d, sp + 0x35C) {
        let w = fx.mem.f(sp + 0x35C);
        fx.fix_residue(w, ra);
        let key = ee::fixunssfsi(w) as i32;
        fx.submit(d, OT, key);
    }
}

/// (16 - age) / 2 / 16 + 0.5, times `k`.
fn size(age: i32, k: f32) -> f32 {
    let f = ee::mul(0.5, ee::from_i32(16 - age));
    ee::mul(k, ee::add(0.5, ee::div(f, 16.0)))
}

fn alpha(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// Updates 1..209 [F 0x1E002F4]: group n % 16 at the eight points.
fn spawn_group(fx: &mut Fx, n: i32) {
    let g = D + 0x2B0 * rem2(n, 16) as u32;
    fx.mem.write_u8(g + 0xE0, 1);
    fx.mem.write_u32(g + 0xE4, 0);
    for s7 in 0..8u32 {
        for c in 0..4u32 {
            let w = fx.mem.read_u32(D + 0x60 + 0x10 * s7 + 4 * c);
            fx.mem.write_u32(g + 0x110 + 0x10 * s7 + 4 * c, w);
        }
        for s6 in 0..4u32 {
            let s0 = g + 4 * s7 + s6;
            let r = fx.rand_from(s0, 0x01E0_0378) >> 23;
            fx.mem.set8(s0 + 0xE8, rem2(r, 4));
            fx.random_table_vec(g + 0x40 * s7 + 0x10 * s6 + 0x190, f32::from_bits(0x3DCC_CCCD));
        }
    }
}

/// The groups' flecks [F 0x1E004DC].
fn draw_groups(fx: &mut Fx, sp: u32, f20: f32) {
    for k in 0..16u32 {
        let g = D + 0x2B0 * k;
        if fx.mem.read_u8(g + 0xE0) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0xE4);
        let c = fx.table_trig((age << 14) >> 4, true);
        let a = alpha(fx, ee::mul(f20, ee::mul(128.0, c)), 0x01E0_0518);
        colour(fx, sp, [0x80, 0x18, 0x14], a);
        let s = size(age, 2.5);
        corners(fx, sp, s);
        let af = ee::from_i32(age);
        let f3 = ee::div(af, 16.0);
        let f2 = ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), af), 16.0);
        let f21 = ee::add(f32::from_bits(0x3F66_6666), f2);
        let f22 = ee::add(-1.0, ee::mul(15.0, f3));
        for j in 0..8u32 {
            for s4 in 0..4u32 {
                let i = fx.mem.read_u8(g + 0xE8 + 4 * j + s4);
                table_uvs(fx, sp, i);
                let o = g + 0x190 + 0x40 * j + 0x10 * s4;
                let x = ee::mul(fx.mem.f(o), f21);
                fx.mem.setf(sp + 0x2C0, x);
                let y = ee::add(f22, fx.mem.f(o + 4));
                fx.mem.setf(sp + 0x2C4, y);
                let z = ee::mul(fx.mem.f(o + 8), f21);
                fx.mem.setf(sp + 0x2C8, z);
                fx.vu_add4(sp + 0x2C0, sp + 0x2C0, g + 0x110 + 0x10 * j);
                draw(fx, sp, 0x01E0_0798);
            }
        }
        let age = fx.mem.i32(g + 0xE4) + 1;
        fx.mem.set32(g + 0xE4, age);
        if age >= 0x11 {
            fx.mem.write_u8(g + 0xE0, 0);
        }
    }
}

/// The spawners (+0x2BE0, 8 x 0xC) [F 0x1E008A4]: each update a burst of
/// 16 flecks in slot +8 of its block (D + 0x1300 s + 0x2C40, 0x130).
fn spawners(fx: &mut Fx, sp: u32) {
    for s in 0..8u32 {
        let fp = D + 0xC * s;
        if fx.mem.read_u8(fp + 0x2BE0) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(fp + 0x2BE4));
        let f2 = ee::div(ee::mul(-95.0, age), 8.0);
        fx.mem.write_u32(sp + 0x2C0, 0);
        fx.mem.write_u32(sp + 0x2C4, 0x41A0_0000);
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        fx.mem.setf(sp + 0x2C8, ee::mul(ee::add(-45.0, f2), side));
        fx.mem.write_u32(sp + 0x2CC, 0x3F80_0000);
        let idx = u32::from(fx.mem.read_u8(fp + 0x2BE8));
        let r = D + 0x1300 * s + 0x130 * idx;
        fx.mem.write_u8(r + 0x2C40, 1);
        fx.mem.write_u32(r + 0x2C44, 0);
        for c in 0..4u32 {
            let w = fx.mem.read_u32(sp + 0x2C0 + 4 * c);
            fx.mem.write_u32(r + 0x2C60 + 4 * c, w);
        }
        for s6 in 0..16u32 {
            let b = fx.rand_from(sp + 0x2F8, 0x01E0_0980) >> 23;
            fx.mem.set8(r + s6 + 0x2C48, rem2(b, 4));
            let v = r + 0x10 * s6 + 0x2C70;
            fx.random_table_vec(v, 1.0);
            let y = fx.mem.f(v + 4);
            // 0x1A7230 fabsf: the argument at its sp.
            fx.mem.write_f32(fx.sp - 0x10, y);
            fx.mem.write_u32(v + 4, y.to_bits() & 0x7FFF_FFFF);
        }
        let i = fx.mem.read_u8(fp + 0x2BE8).wrapping_add(1);
        fx.mem.write_u8(fp + 0x2BE8, i);
        if i >= 0x10 {
            fx.mem.write_u8(fp + 0x2BE8, 0);
        }
        let a = fx.mem.i32(fp + 0x2BE4) + 1;
        fx.mem.set32(fp + 0x2BE4, a);
        if a >= 9 {
            fx.mem.write_u8(fp + 0x2BE0, 0);
        }
    }
}

/// The bursts' flecks [F 0x1E00AD4].
fn draw_bursts(fx: &mut Fx, sp: u32) {
    for b in 0..8u32 {
        for i in 0..16u32 {
            let fp = D + 0x1300 * b + 0x130 * i;
            if fx.mem.read_u8(fp + 0x2C40) == 0 {
                continue;
            }
            let age = fx.mem.i32(fp + 0x2C44);
            let sn = fx.table_trig((age << 15) >> 4, false);
            let a = alpha(fx, ee::mul(128.0, sn), 0x01E0_0B0C);
            colour(fx, sp, [0x80, 0x1E, 0x1A], a);
            let s = size(age, 7.0);
            corners(fx, sp, s);
            let af = ee::from_i32(age);
            let f3 = ee::div(af, 16.0);
            let f2 = ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), af), 16.0);
            let f20 = ee::mul(50.0, f3);
            let f21 = ee::add(f32::from_bits(0x3F66_6666), f2);
            for s6 in 0..16u32 {
                let i = fx.mem.read_u8(fp + 0x2C48 + s6);
                table_uvs(fx, sp, i);
                let o = fp + 0x2C70 + 0x10 * s6;
                let x = ee::mul(fx.mem.f(o), f21);
                fx.mem.setf(sp + 0x2C0, x);
                let y = ee::add(f20, fx.mem.f(o + 4));
                fx.mem.setf(sp + 0x2C4, y);
                let z = ee::mul(fx.mem.f(o + 8), f21);
                fx.mem.setf(sp + 0x2C8, z);
                fx.vu_add4(sp + 0x2C0, sp + 0x2C0, fp + 0x2C60);
                draw(fx, sp, 0x01E0_0D58);
            }
            let age = fx.mem.i32(fp + 0x2C44) + 1;
            fx.mem.set32(fp + 0x2C44, age);
            if age >= 0x11 {
                fx.mem.write_u8(fp + 0x2C40, 0);
            }
        }
    }
}

/// The full-window tint [F 0x1E00E9C] (descriptor sp + 0x328: RGBAQ and
/// two XYZ2 corners, prim 6), alpha 200 (1 - (1 - tan falloff)).
fn tint(fx: &mut Fx, sp: u32) {
    let d = sp + 0x328;
    fx.mem.write_u16(d + 2, 6);
    fx.mem.write_u16(d, 3);
    fx.mem.write_u64(d + 8, 0x441);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let k = fx.mem.i32(TINT + 4);
    let f = fx.tan_falloff_p(k, 0x40, 1);
    let a = alpha(fx, ee::mul(200.0, ee::sub(1.0, f)), 0x01E0_0F08);
    fx.mem.write_u8(sp + 0x338, 0xFF);
    fx.mem.write_u8(sp + 0x339, 0xA4);
    fx.mem.write_u8(sp + 0x33A, 0xA4);
    fx.mem.write_u8(sp + 0x33B, a as u8);
    fx.mem.write_u32(sp + 0x33C, 0x3F80_0000);
    for (o, c) in [(0x340u32, 0x7000u16), (0x348, 0x9000)] {
        fx.mem.write_u16(sp + o, c);
        fx.mem.write_u16(sp + o + 2, c);
        let v = fx.mem.read_u64(sp + o);
        let v = (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFF8_0000_0000;
        fx.mem.write_u64(sp + o, v);
        fx.mem.write_u8(sp + o + 7, 0);
    }
    fx.submit(d, OT, 0);
    let k = fx.mem.i32(TINT + 4) + 1;
    fx.mem.set32(TINT + 4, k);
    if k >= 0x41 {
        fx.mem.write_u8(TINT, 0);
    }
}

/// The 256 rays of eight sprites from the target [F 0x1E010AC].
fn rays(fx: &mut Fx, sp: u32) {
    let rh2 = fx.mem.read_u32(D + 0x14);
    sprite_desc(fx, sp, rh2);
    corners(fx, sp, 2.5);
    let k = fx.mem.i32(RAYS + 4);
    let f21 = fx.tan_falloff_p(k, 0x40, 1);
    let x = ee::from_i32(fx.mem.i32(RAYS + 4));
    let f20 = ee::mul(255.0, Fx::ramp_above(x, 64.0, 32.0));
    for s6 in 0..0x100u32 {
        let b = fx.mem.read_u8(RAYS + 8 + s6);
        let (c, r) = (u32::from(b & 3), u32::from(b >> 2));
        set_uvs(fx, sp, (c << 9) & 0x3FFF, (r << 9) & 0x3FFF, ((32 * c + 32) << 4) & 0x3FFF, ((32 * r + 32) << 4) & 0x3FFF);
        let v = D + 0xC560 + 0x10 * s6;
        for s5 in 0..8i32 {
            let a = alpha(fx, ee::div(ee::mul(f20, ee::from_i32(s5 + 1)), 8.0), 0x01E0_12CC);
            colour(fx, sp, [0xFF, 0x40, 0x30], a);
            let s = ee::add(f21, ee::mul(f32::from_bits(0x3BE5_6042), ee::from_i32(s5)));
            fx.vu_scale4(sp + 0x2C0, v, s);
            fx.vu_add4(sp + 0x2C0, sp + 0x2C0, D + 0x20);
            draw(fx, sp, 0x01E0_137C);
        }
    }
    let k = fx.mem.i32(RAYS + 4) + 1;
    fx.mem.set32(RAYS + 4, k);
    if k >= 0x41 {
        fx.mem.write_u8(RAYS, 0);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x240);
    for s2 in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x30 + 4 * s2));
        fx.node_position(h, slot, D + 0x40 + 0x10 * s2);
    }
    fx.vu_sub4(sp + 0x2C0, D + 0x50, D + 0x40);
    for s1 in 0..8i32 {
        let o = D + 0x60 + 0x10 * s1 as u32;
        let f = ee::div(ee::mul(2.0, ee::from_i32(s1)), 7.0);
        fx.vu_scale4(o, sp + 0x2C0, f);
        fx.vu_add4(o, o, D + 0x40);
    }
    let n = fx.mem.i32(D);
    if n > 0 && n < 0xD2 {
        spawn_group(fx, n);
    }
    let rh2 = fx.mem.read_u32(D + 0x10);
    sprite_desc(fx, sp, rh2);
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n, 0, 0x40));
    let b = ee::from_i32(Fx::clamp_i(0xD2 - n, 0, 0x40));
    let f20 = ee::div(ee::mul(a, b), 4096.0);
    draw_groups(fx, sp, f20);
    let n = fx.mem.i32(D);
    if (0x86..0xB5).contains(&n) {
        let e = D + 0xC * rem2(n, 8) as u32;
        fx.mem.write_u8(e + 0x2BE0, 1);
        fx.mem.write_u32(e + 0x2BE4, 0);
    }
    spawners(fx, sp);
    sprite_desc(fx, sp, rh2);
    draw_bursts(fx, sp);
    if fx.mem.i32(D) == 0xC5 {
        fx.mem.write_u8(TINT, 1);
        fx.mem.write_u32(TINT + 4, 0);
        for k in 0..8u32 {
            fx.mem.write_u8(D + 0x2BE0 + 0xC * k, 0);
        }
        for j in 0..8u32 {
            for i in 0..16u32 {
                fx.mem.write_u8(D + 0x2C40 + 0x1300 * j + 0x130 * i, 0);
            }
        }
    }
    if fx.mem.read_u8(TINT) == 1 {
        tint(fx, sp);
    }
    if fx.mem.i32(D) == 0xC5 {
        fx.mem.write_u8(RAYS, 1);
        fx.mem.write_u32(RAYS + 4, 0);
        for s5 in 0..0x100u32 {
            let r = fx.rand_from(sp + 0x2F8, 0x01E0_103C) >> 23;
            fx.mem.set8(RAYS + 8 + s5, rem2(r, 8));
            fx.random_table_vec(D + 0xC560 + 0x10 * s5, 5.0);
        }
    }
    if fx.mem.read_u8(RAYS) == 1 {
        rays(fx, sp);
    }
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x7B {
        ret = 1;
    } else if (0x86..0xCD).contains(&n) {
        ret = 6;
    } else if n == 0xCD {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x106 {
        ret = 8;
    }
    ret
}
