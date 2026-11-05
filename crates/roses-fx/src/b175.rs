//! BMODULE b175 (card 616), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0x4D90
//! bytes) [F]: a single unscheduled function in
//! a 0x470 frame on b150's helpers (textured window sprites, descriptor sp
//! + 0x3C8: 5 registers 0x42421, prim 0x26 | 0x800, clamp 5). The init
//! lays out a circle of 33 points of radius 42 (D+0x4B80). Every update
//! the camera turn with roll (0x2055E0 into sp + 0x310) and the attacker's
//! nodes 0x10 / 0x17 (D+0x40 / D+0x50). From update 1 one of 32 spark
//! records (+0x60, 0x50) of two random sparks (0.5) at the two nodes, 32
//! updates (half size 8 (0.2 + 0.8 age / 32), spread 0.1 + 0.9 age / 32,
//! alpha 255 cos((age << 14) >> 5) x the window clamp(n) / 32, colour (FF,
//! 40, 18), the whole texture). On updates 33 + 34 k (k < 8) one of two
//! wave records (+0xA70, 0x50) of two sprites from the nodes towards
//! random points round the target (1.0), 25 updates (half size 6, colour
//! (FF, 40, 1C, FF), cells ping-ponging 0..4 a step an update); every
//! update each live wave leaves one of 12 burst records (+0xB10, 0xF00 a
//! wave, 0x140) of 2 x 8 random sprites (0.5) at its sprites, drawn 12
//! updates (texture D+0x18, half size 6 (0.8 + 0.2 age / 12), spread 0.3
//! + 0.7 tan falloff, alpha 128 (12 - age) / 12, colour (80, 28, 12), UVs
//! from the table 0x28AE70); at age 24 a wave leaves one of two ring
//! records (+0x2920, 0x1130) of 2 x 128 random sprites (1.2), 40 updates
//! (half size 6 (0.8 + 0.2 age / 40), spread (0.3 + 0.7 tan falloff) or
//! 0.8 of it for every third sprite (grey, ALPHA preset 2; else (80, 22,
//! 08)), alpha 128 x ramp_above(age, 40, 20)) and under each a gouraud disc
//! (prim 0x13, 0x100B00) of the circle scaled by the spread (0x20C290),
//! centre (FF, A4, 38), rim (FF, 26, 06, 0), key 0xFFFFFF. Returns 1 on
//! update 0x2F, 6 on 0x3A..0x14D, 2 on 0x14E, 8 once the count reaches
//! 0x150.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x470;
const DESC: u32 = 0x3C8;
/// The gouraud disc's descriptor (prim 0x13, 6 registers).
const DISC: u32 = 0x420;
/// The UV table 0x28AE70 (four (u0, u1, v0, v1) halfword cells).
const UV_TABLE: u32 = 0x0028_AE70;
/// The entry's s0 (0x206E40's return), rand's saved s0.
const S0: u32 = OT;

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

fn copy8(fx: &mut Fx, to: u32, from: u32) {
    let a = fx.mem.read_u32(from);
    let b = fx.mem.read_u32(from + 4);
    fx.mem.write_u32(to, a);
    fx.mem.write_u32(to + 4, b);
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0x4D90);
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
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x10, 1);
    fx.mem.set32(D + 0x30, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x17, 0);
    fx.mem.set32(D + 0x34, r);
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x60 + 0x50 * k, 0);
    }
    fx.mem.write_u32(D + 0xA60, 0);
    fx.mem.write_u32(D + 0xA64, 0);
    for k in 0..2u32 {
        fx.mem.write_u8(D + 0xA70 + 0x50 * k, 0);
        fx.mem.write_u8(D + 0xA7C + 0x50 * k, 0);
    }
    for g in 0..2u32 {
        for k in 0..0xCu32 {
            fx.mem.write_u8(D + 0xB10 + 0xF00 * g + 0x140 * k, 0);
        }
    }
    fx.mem.write_u32(D + 0x2910, 0);
    for k in 0..2u32 {
        fx.mem.write_u8(D + 0x2920 + 0x1130 * k, 0);
    }
    for k in 0..0x21u32 {
        let a = (k << 16) as i32 >> 5;
        let p = D + 0x4B80 + 0x10 * k;
        let sn = fx.tsin(a);
        fx.mem.setf(p, ee::mul(42.0, sn));
        let cs = fx.tcos(a);
        fx.mem.setf(p + 4, ee::mul(42.0, cs));
        fx.mem.write_u32(p + 8, 0);
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        // The circle loop steps the stack copy of a0.
        fx.mem.write_u32(sp + 0xC0, D + 0x10 * (k + 1));
    }
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The sprite's two UVs (sp + 0x3E0 / 0x3E2 / 0x3F0 / 0x3F2).
fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x3E0, u0);
    uv(fx, sp + 0x3E2, v0);
    uv(fx, sp + 0x3F0, u1);
    uv(fx, sp + 0x3F2, v1);
}

/// UV cell `i` of the table 0x28AE70.
fn table_uvs(fx: &mut Fx, sp: u32, i: u8) {
    let t = UV_TABLE + 8 * u32::from(i);
    let (u0, u1, v0, v1) = (fx.mem.read_u16(t), fx.mem.read_u16(t + 2), fx.mem.read_u16(t + 4), fx.mem.read_u16(t + 6));
    uvs(fx, sp, u32::from(u0), u32::from(v0), u32::from(u1), u32::from(v1));
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// The view-projection (x identity) at sp + 0x290.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x290);
    fx.identity(sp + 0x2D0);
    let id = fx.read_m4(sp + 0x2D0);
    fx.mul_into(Some(sp + 0x290), &id);
}

/// The descriptor rewritten through its pointers (sp + 0x1D0 / 0x1E0 /
/// 0x1F0), | 0x800, its texture and (with `alpha`) ALPHA preset 1, clamp 5.
fn desc_reset(fx: &mut Fx, sp: u32, rh_at: u32, alpha: bool) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + rh_at);
    fx.desc_texture(d, rh, 1);
    if alpha {
        let a = fx.alpha_preset(1);
        fx.desc_alpha(d, a);
    }
    fx.desc_clamp(d, 5);
}

/// The corners (-s, -s, 0, 1) at sp + 0x350 and (s, s, 0, 1) at sp +
/// 0x360 turned by the camera (sp + 0x310).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x350, n);
    fx.mem.setf(sp + 0x354, n);
    fx.mem.write_u32(sp + 0x358, 0);
    fx.mem.write_u32(sp + 0x35C, 0x3F80_0000);
    fx.mem.setf(sp + 0x360, s);
    fx.mem.setf(sp + 0x364, s);
    fx.mem.write_u32(sp + 0x368, 0);
    fx.mem.write_u32(sp + 0x36C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x350, sp + 0x310, sp + 0x350);
    fx.vu_apply_to(sp + 0x360, sp + 0x310, sp + 0x360);
}

/// The colour (c, a, Q = 1.0) at sp + 0x3D8.
fn colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(sp + 0x3D8, c[0]);
    fx.mem.write_u8(sp + 0x3D9, c[1]);
    fx.mem.write_u8(sp + 0x3DA, c[2]);
    fx.mem.write_u8(sp + 0x3DB, a);
    fx.mem.write_u32(sp + 0x3DC, 0x3F80_0000);
}

/// The vertices at sp + 0x370 / 0x380 = the corners + sp + 0x390, drawn
/// at the mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    fx.vu_add4(sp + 0x370, sp + 0x350, sp + 0x390);
    fx.vu_add4(sp + 0x380, sp + 0x360, sp + 0x390);
    if crate::fxlib2::project(fx, sp + 0x370, PROJ_OUT, sp + 0x290, sp + DESC, sp + 0x46C) {
        let w = fx.mem.f(sp + 0x46C);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// The spark records [F 0x1E0030C / 0x1E003B4].
fn sparks(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x1_F400 {
        let f = D + 0x60 + 0x50 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(f, 1);
        fx.mem.write_u32(f + 4, 0);
        for j in 0..2u32 {
            words(fx, f + 0x10 + 0x10 * j, D + 0x40 + 0x10 * j);
            fx.random_table_vec(f + 0x30 + 0x10 * j, 0.5);
        }
    }
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u32(sp + 0x1D0, d + 2);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(sp + 0x1E0, d + 8);
    fx.mem.write_u32(sp + 0x1F0, d + 4);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
    uvs(fx, sp, 0, 0, 0x800, 0x800);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n, 0, 0x20));
    let b = ee::from_i32(Fx::clamp_i(0x1_F400 - n, 0, 0x20));
    let f20 = ee::div(ee::mul(a, b), 1024.0);
    for i in 0..0x20u32 {
        let f = D + 0x60 + 0x50 * i;
        if fx.mem.read_u8(f) == 0 {
            continue;
        }
        let age = fx.mem.i32(f + 4);
        let cs = fx.tcos((age << 14) >> 5);
        let a = fix(fx, ee::mul(f20, ee::mul(255.0, cs)), 0x01E0_0534);
        colour(fx, sp, [0xFF, 0x40, 0x18], a as u8);
        let age = ee::from_i32(fx.mem.i32(f + 4));
        let s = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), age), 32.0);
        let s = ee::mul(8.0, ee::add(f32::from_bits(0x3E4C_CCCD), s));
        corners(fx, sp, s);
        let age = ee::from_i32(fx.mem.i32(f + 4));
        let f21 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F66_6666), age), 32.0));
        for j in 0..2u32 {
            let p = f + 0x30 + 0x10 * j;
            let x = ee::mul(fx.mem.f(p), f21);
            fx.mem.setf(sp + 0x390, x);
            let y = ee::mul(fx.mem.f(p + 4), f21);
            fx.mem.setf(sp + 0x394, y);
            let z = ee::mul(fx.mem.f(p + 8), f21);
            fx.mem.setf(sp + 0x398, z);
            fx.vu_add4(sp + 0x390, sp + 0x390, f + 0x10 + 0x10 * j);
            draw(fx, sp, 0x01E0_06B4);
        }
        let age = fx.mem.i32(f + 4) + 1;
        fx.mem.set32(f + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(f, 0);
        }
    }
}

/// A wave record started [F 0x1E00708].
fn wave_spawn(fx: &mut Fx) {
    let n = fx.mem.i32(D);
    let w = fx.mem.i32(D + 0xA60);
    if n != w * 34 + 0x21 || w >= 8 {
        return;
    }
    let k = fx.mem.i32(D + 0xA64) as u32;
    let base = D + 0x50 * k;
    fx.mem.write_u8(base + 0xA70, 1);
    fx.mem.write_u32(base + 0xA74, 0);
    for j in 0..2u32 {
        let r = fx.rand_from(S0, 0x01E0_0774) >> 23;
        fx.mem.set8(base + 0xA78 + j, rem2(r, 4));
        fx.mem.write_u8(base + 0xA7A + j, 1);
        let at = base + 0x10 * j;
        words(fx, at + 0xA80, D + 0x40 + 0x10 * j);
        fx.random_table_vec(at + 0xAA0, 1.0);
        fx.vu_add4(at + 0xAA0, at + 0xAA0, D + 0x20);
        fx.vu_sub4(at + 0xAA0, at + 0xAA0, at + 0xA80);
    }
    let w = fx.mem.i32(D + 0xA60) + 1;
    fx.mem.set32(D + 0xA60, w);
    let k = fx.mem.i32(D + 0xA64) + 1;
    fx.mem.set32(D + 0xA64, k);
    if k >= 2 {
        fx.mem.write_u32(D + 0xA64, 0);
    }
}

/// The waves [F 0x1E00840]: two sprites each, leaving bursts and (at age
/// 24) a ring.
fn waves(fx: &mut Fx, sp: u32) {
    desc_reset(fx, sp, 0x14, true);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x230, 0);
    fx.mem.write_u32(sp + 0x120, D);
    fx.mem.write_u32(sp + 0x130, D);
    for wi in 0..2u32 {
        let base = D + 0x50 * wi;
        let grp = D + 0xF00 * wi;
        if fx.mem.read_u8(base + 0xA70) != 0 {
            colour(fx, sp, [0xFF, 0x40, 0x1C], 0xFF);
            corners(fx, sp, 6.0);
            fx.mem.write_u32(sp + 0x200, sp + 0x3A0);
            for j in 0..2u32 {
                let cb = fx.mem.read_u8(base + 0xA78 + j);
                let (c, r) = (u32::from(cb & 3), u32::from(cb >> 2));
                let step = fx.mem.read_u8(base + 0xA7A + j);
                let nb = cb.wrapping_add(step);
                fx.mem.write_u8(base + 0xA78 + j, nb);
                if nb == 0 || nb == 4 {
                    let s = fx.mem.read_u8(base + 0xA7A + j);
                    fx.mem.write_u8(base + 0xA7A + j, s.wrapping_neg());
                }
                uvs(fx, sp, (c << 9) & 0x3FFF, (r << 9) & 0x3FFF, (((c << 5) + 0x20) << 4) & 0x3FFF, (((r << 5) + 0x20) << 4) & 0x3FFF);
                let at = base + 0x10 * j;
                let f = ee::div(ee::from_i32(fx.mem.i32(base + 0xA74)), 24.0);
                fx.vu_scale4(sp + 0x390, at + 0xAA0, f);
                fx.vu_add4(sp + 0x390, sp + 0x390, at + 0xA80);
                draw(fx, sp, 0x01E0_0AE4);
                words(fx, sp + 0x3A0 + 0x10 * j, sp + 0x390);
            }
            // A burst record at the sprites.
            fx.mem.write_u32(sp + 0x270, 0);
            let b = u32::from(fx.mem.read_u8(base + 0xA7C));
            fx.mem.write_u32(sp + 0xD0, sp + 0x3A0);
            let bb = grp + 0x140 * b;
            fx.mem.write_u8(bb + 0xB10, 1);
            fx.mem.write_u32(bb + 0xB14, 0);
            fx.mem.write_u32(sp + 0xE0, bb);
            fx.mem.write_u32(sp + 0xF0, bb);
            for m in 0..2u32 {
                words(fx, bb + 0xB30 + 0x10 * m, sp + 0x3A0 + 0x10 * m);
                for s3 in 0..8u32 {
                    let r = fx.rand_from(S0, 0x01E0_0BA8) >> 23;
                    fx.mem.set8(bb + 8 * m + 0xB18 + s3, rem2(r, 4));
                    fx.random_table_vec(bb + 0x80 * m + 0xB50 + 0x10 * s3, 0.5);
                }
                fx.mem.write_u32(sp + 0xD0, sp + 0x3B0 + 0x10 * m);
                fx.mem.write_u32(sp + 0xE0, bb + 8 * (m + 1));
                fx.mem.write_u32(sp + 0xF0, bb + 0x80 * (m + 1));
                fx.mem.write_u32(sp + 0x270, m + 1);
            }
            let b = fx.mem.read_u8(base + 0xA7C).wrapping_add(1);
            fx.mem.write_u8(base + 0xA7C, b);
            if b >= 0xC {
                fx.mem.write_u8(base + 0xA7C, 0);
            }
            if fx.mem.i32(base + 0xA74) == 0x18 {
                ring_spawn(fx, sp);
            }
            let age = fx.mem.i32(base + 0xA74) + 1;
            fx.mem.set32(base + 0xA74, age);
            if age >= 0x19 {
                fx.mem.write_u8(base + 0xA70, 0);
            }
        }
        fx.mem.write_u32(sp + 0x120, base + 0x50);
        fx.mem.write_u32(sp + 0x130, grp + 0xF00);
        fx.mem.write_u32(sp + 0x230, wi + 1);
    }
}

/// A ring record at the wave's sprites [F 0x1E00C6C].
fn ring_spawn(fx: &mut Fx, sp: u32) {
    fx.mem.write_u32(sp + 0x260, 0);
    let k = fx.mem.i32(D + 0x2910);
    let off = fx.mult(k, 0x1130) as u32;
    let rb = D + off;
    fx.mem.write_u8(rb + 0x2920, 1);
    fx.mem.write_u32(rb + 0x2924, 0);
    fx.mem.write_u32(sp + 0x100, rb);
    fx.mem.write_u32(sp + 0x110, rb);
    for m in 0..2u32 {
        let from = fx.mem.read_u32(sp + 0x200);
        words(fx, rb + 0x2A30 + 0x10 * m, from);
        for s2 in 0..0x80u32 {
            let r = fx.rand_from(S0, 0x01E0_0CD8) >> 23;
            fx.mem.set8(rb + 0x80 * m + 0x2928 + s2, rem2(r, 4));
            fx.random_table_vec(rb + 0x800 * m + 0x2A50 + 0x10 * s2, f32::from_bits(0x3F99_999A));
        }
        fx.mem.write_u32(sp + 0x200, from + 0x10);
        fx.mem.write_u32(sp + 0x100, rb + 0x80 * (m + 1));
        fx.mem.write_u32(sp + 0x110, rb + 0x800 * (m + 1));
        fx.mem.write_u32(sp + 0x260, m + 1);
    }
    let k = fx.mem.i32(D + 0x2910) + 1;
    fx.mem.set32(D + 0x2910, k);
    if k >= 2 {
        fx.mem.write_u32(D + 0x2910, 0);
    }
}

/// The bursts [F 0x1E00DEC]: 2 groups of 12 records of 2 x 8 sprites.
fn bursts(fx: &mut Fx, sp: u32) {
    desc_reset(fx, sp, 0x18, true);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x220, 0);
    for g in 0..2u32 {
        let grp = D + 0xF00 * g;
        fx.mem.write_u32(sp + 0x180, grp);
        fx.mem.write_u32(sp + 0x250, 0);
        for b in 0..0xCu32 {
            let bb = grp + 0x140 * b;
            fx.mem.write_u32(sp + 0x170, bb);
            if fx.mem.read_u8(bb + 0xB10) != 0 {
                let x = (0xC - fx.mem.i32(bb + 0xB14)) << 7;
                fx.mult(0x2AAA_AAAB, x);
                let hi = fx.hilo().1;
                let a = (hi >> 1).wrapping_add(((x as u32) >> 31) as i32);
                colour(fx, sp, [0x80, 0x28, 0x12], a as u8);
                let age = ee::from_i32(fx.mem.i32(bb + 0xB14));
                let s = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), age), 12.0);
                let s = ee::mul(6.0, ee::add(f32::from_bits(0x3F4C_CCCD), s));
                corners(fx, sp, s);
                let age = fx.mem.i32(bb + 0xB14);
                let f0 = fx.tan_falloff_p(age, 0xC, 1);
                let f20 = ee::add(f32::from_bits(0x3E99_999A), ee::mul(f32::from_bits(0x3F33_3333), f0));
                fx.mem.write_u32(sp + 0x280, 0);
                for m in 0..2u32 {
                    fx.mem.write_u32(sp + 0x140, bb + 8 * m);
                    fx.mem.write_u32(sp + 0x150, bb + 0x80 * m);
                    fx.mem.write_u32(sp + 0x160, bb + 0x10 * m);
                    let pos = bb + 0x10 * m + 0xB30;
                    for s1 in 0..8u32 {
                        let c = fx.mem.read_u8(bb + 8 * m + 0xB18 + s1);
                        table_uvs(fx, sp, c);
                        fx.vu_scale4(sp + 0x390, bb + 0x80 * m + 0xB50 + 0x10 * s1, f20);
                        fx.vu_add4(sp + 0x390, sp + 0x390, pos);
                        draw(fx, sp, 0x01E0_111C);
                    }
                    fx.mem.write_u32(sp + 0x140, bb + 8 * (m + 1));
                    fx.mem.write_u32(sp + 0x150, bb + 0x80 * (m + 1));
                    fx.mem.write_u32(sp + 0x160, bb + 0x10 * (m + 1));
                    fx.mem.write_u32(sp + 0x280, m + 1);
                }
                let age = fx.mem.i32(bb + 0xB14) + 1;
                fx.mem.set32(bb + 0xB14, age);
                if age >= 0xD {
                    fx.mem.write_u8(bb + 0xB10, 0);
                }
            }
            fx.mem.write_u32(sp + 0x170, bb + 0x140);
            fx.mem.write_u32(sp + 0x250, b + 1);
        }
        fx.mem.write_u32(sp + 0x180, grp + 0xF00);
        fx.mem.write_u32(sp + 0x220, g + 1);
    }
}

/// The rings [F 0x1E01200]: 2 records of 2 x 128 sprites, each half over a
/// gouraud disc.
fn rings(fx: &mut Fx, sp: u32) {
    desc_reset(fx, sp, 0x18, false);
    let d2 = sp + DISC;
    fx.mem.write_u16(d2 + 2, 0x13);
    fx.mem.write_u16(d2, 6);
    fx.mem.write_u64(d2 + 8, 0x41_4141);
    fx.mem.write_u32(d2 + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d2, a);
    let d = sp + DESC;
    fx.mem.write_u32(sp + 0x210, 0);
    for ri in 0..2u32 {
        let rb = D + 0x1130 * ri;
        fx.mem.write_u32(sp + 0x1C0, rb);
        if fx.mem.read_u8(rb + 0x2920) != 0 {
            let age = ee::from_i32(fx.mem.i32(rb + 0x2924));
            let s = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), age), 40.0);
            let s = ee::mul(6.0, ee::add(f32::from_bits(0x3F4C_CCCD), s));
            corners(fx, sp, s);
            let age = fx.mem.i32(rb + 0x2924);
            let f0 = fx.tan_falloff_p(age, 0x28, 1);
            let f22 = ee::add(f32::from_bits(0x3E99_999A), ee::mul(f32::from_bits(0x3F33_3333), f0));
            let age = ee::from_i32(fx.mem.i32(rb + 0x2924));
            let f0 = Fx::ramp_above(age, 40.0, 20.0);
            fx.mem.write_u32(sp + 0x240, 0);
            fx.mem.write_u32(sp + 0x190, rb);
            fx.mem.write_u32(sp + 0x1A0, rb);
            fx.mem.write_u32(sp + 0x1B0, rb);
            let f20 = ee::mul(f32::from_bits(0x3F4C_CCCD), f22);
            let f21 = ee::mul(128.0, f0);
            for m in 0..2u32 {
                camera(fx, sp);
                let pos = rb + 0x10 * m + 0x2A30;
                for s2 in 0..0x80u32 {
                    let (_, third) = fx.div(s2 as i32, 3);
                    let f23 = if third == 0 {
                        let a = fx.alpha_preset(2);
                        fx.desc_alpha(d, a);
                        let a = fix(fx, f21, 0x01E0_1444);
                        colour(fx, sp, [0x80, 0x80, 0x80], a as u8);
                        f20
                    } else {
                        let a = fx.alpha_preset(1);
                        fx.desc_alpha(d, a);
                        let a = fix(fx, f21, 0x01E0_1484);
                        colour(fx, sp, [0x80, 0x22, 0x08], a as u8);
                        f22
                    };
                    let c = fx.mem.read_u8(rb + 0x80 * m + 0x2928 + s2);
                    table_uvs(fx, sp, c);
                    fx.vu_scale4(sp + 0x390, rb + 0x800 * m + 0x2A50 + 0x10 * s2, f23);
                    fx.vu_add4(sp + 0x390, sp + 0x390, pos);
                    draw(fx, sp, 0x01E0_15CC);
                }
                // The disc under it: the circle scaled by the spread at the
                // position, turned by the camera.
                fx.camera_vp_to(sp + 0x290);
                fx.translate_rows(sp + 0x2D0, sp + 0x310, pos);
                fx.mem.setf(sp + 0x390, f22);
                fx.mem.setf(sp + 0x394, f22);
                fx.mem.setf(sp + 0x398, f22);
                fx.mem.write_u32(sp + 0x39C, 0x3F80_0000);
                fx.scale_rows(sp + 0x2D0, sp + 0x390);
                let mm = fx.read_m4(sp + 0x2D0);
                fx.mul_into(Some(sp + 0x290), &mm);
                let a = fix(fx, f21, 0x01E0_1650);
                fx.mem.write_u8(sp + 0x430, 0xFF);
                fx.mem.write_u8(sp + 0x431, 0xA4);
                fx.mem.write_u8(sp + 0x432, 0x38);
                fx.mem.write_u8(sp + 0x433, a as u8);
                fx.mem.write_u32(sp + 0x434, 0x3F80_0000);
                fx.mem.write_u8(sp + 0x440, 0xFF);
                fx.mem.write_u8(sp + 0x441, 0x26);
                fx.mem.write_u8(sp + 0x442, 0x06);
                fx.mem.write_u8(sp + 0x443, 0);
                fx.mem.write_u32(sp + 0x444, 0x3F80_0000);
                copy8(fx, sp + 0x450, sp + 0x440);
                fx.mem.write_u32(sp + 0x390, 0);
                fx.mem.write_u32(sp + 0x394, 0);
                fx.mem.write_u32(sp + 0x398, 0);
                fx.mem.write_u32(sp + 0x39C, 0x3F80_0000);
                for s2 in 0..0x20u32 {
                    let (v1, v2) = (D + 0x4B80 + 0x10 * s2, D + 0x4B80 + 0x10 * (s2 + 1));
                    let r = fx.project_n(&[sp + 0x390, v1, v2], sp + 0x290, d2, sp + 0x46C, 0x01E0_16F4);
                    if r == 1 {
                        fx.submit(d2, OT, 0xFF_FFFF);
                    }
                }
                fx.mem.write_u32(sp + 0x190, rb + 0x80 * (m + 1));
                fx.mem.write_u32(sp + 0x1A0, rb + 0x800 * (m + 1));
                fx.mem.write_u32(sp + 0x1B0, rb + 0x10 * (m + 1));
                fx.mem.write_u32(sp + 0x240, m + 1);
            }
            let age = fx.mem.i32(rb + 0x2924) + 1;
            fx.mem.set32(rb + 0x2924, age);
            if age >= 0x29 {
                fx.mem.write_u8(rb + 0x2920, 0);
            }
        }
        fx.mem.write_u32(sp + 0x1C0, rb + 0x1130);
        fx.mem.write_u32(sp + 0x210, ri + 1);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x310);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x30 + 4 * k));
        fx.node_position(h, slot, D + 0x40 + 0x10 * k);
    }
    sparks(fx, sp);
    wave_spawn(fx);
    waves(fx, sp);
    bursts(fx, sp);
    rings(fx, sp);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x2F {
        ret = 1;
    } else if (0x3A..0x14E).contains(&n) {
        ret = 6;
    } else if n == 0x14E {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x150 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}
