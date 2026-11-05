//! BMODULE b768 (card 622), hand-ported from the record (linked at
//! 0x1E00000, the entry 0x1E00080 in a 0x540 frame with D kept at sp +
//! 0xBC, data area a0 = 0x1ED0000, 0x4980 bytes; one attacker post
//! callback 0x1E01D80 with its static D slot at 0x1E02080) [F] (helper
//! A31, 3 Nov 2026). Eight shell records (D+0x70, 0x220: spawned on 0xBC +
//! 18 k at 1.25 (k + 2) / 8 of the way from the attacker's position D+0x20
//! to the target's point D+0x40, 32 spinning grey quads about it) each
//! dropping 16 burst records (D+0x1170 + 0x700 k, 0x70: six random
//! vectors) once 0x30 old: two sprite fans (FF, 50, 2D) / (FF, 50, 0C) and
//! six falling, spinning, growing (FF, 80, 80) quads; a frame-textured
//! window (BC, 8A, 80); the attacker drawn into its target with the
//! callback (the frame as a full-window sprite, then a (C8, 80, 00) veil)
//! and pulled through three widening eight-triangle fans about its screen
//! point (D+0x4970, from 0x66 for 0xA9 updates); the stage light 0x80 - 96
//! clamp(n) clamp(0x1FD - n) / 1024; returns 1 / 6 / 2 / 8 on 0x118 /
//! 0x123..0x1BC / 0x1BD / 0x1FE.
use crate::b182::{alpha, head, rgba, uv};
use crate::b440::{copy8, fix, rem2};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const CB: u32 = 0x01E0_1D80;
const D_STATIC: u32 = 0x01E0_2080;
const FRAME: u32 = 0x540;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xBC, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x4980);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let side = fx.unit_side(h);
    fx.mem.set32(D + 0xC, side);
    for (k, id) in [(0x10u32, 0), (0x14, 1), (0x18, 2), (0x1C, 3)] {
        let r = fx.texture(5, id);
        fx.mem.write_u32(D + k, r);
    }
    let h = fx.mem.i32(D + 4);
    fx.unit_position(h, D + 0x20);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x40);
    fx.mem.write_u32(D + 0x60, 0);
    for k in 0..8u32 {
        fx.mem.write_u8(D + 0x70 + 0x220 * k, 0);
    }
    for k in 0..8u32 {
        for j in 0..0x10u32 {
            fx.mem.write_u8(D + 0x700 * k + 0x70 * j + 0x1170, 0);
        }
    }
    fx.mem.write_u8(D + 0x4970, 0);
    0
}

fn vec4(fx: &mut Fx, p: u32, x: f32, y: f32) {
    fx.mem.setf(p, x);
    fx.mem.setf(p + 4, y);
    fx.mem.write_u32(p + 8, 0);
    fx.mem.write_u32(p + 0xC, 0x3F80_0000);
}

/// The camera's view-projection at sp + 0x240 times the identity at sp +
/// 0x280.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x240);
    fx.identity(sp + 0x280);
    crate::blib::exe::mul_mem(fx, sp + 0x240, sp + 0x280);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x2C0);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    camera(fx, sp);
    // The attacker's screen point D+0x50 (x / y clamped to 0..512, Z by
    // 0x17BC10).
    let m = fx.read_m4(sp + 0x240);
    fx.project_at(D + 0x30, &m, sp + 0x530, sp + 0x53C);
    let x = (i32::from(fx.mem.read_u16(sp + 0x530)) >> 4) - 0x700;
    fx.mem.setf(D + 0x50, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x532)) >> 4) - 0x700;
    fx.mem.setf(D + 0x54, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x530) << 8) >> 40;
    fx.mem.setf(D + 0x58, z as u32 as f32);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    for k in [0x50u32, 0x54] {
        if crate::exe::ee_lt(fx.mem.f(D + k), 0.0) {
            fx.mem.write_u32(D + k, 0);
        }
    }
    for k in [0x50u32, 0x54] {
        if !crate::exe::ee_le(fx.mem.f(D + k), 512.0) {
            fx.mem.write_u32(D + k, 0x4400_0000);
        }
    }

    shells(fx, sp);
    bursts(fx, sp);
    window(fx, sp);
    attacker_pass(fx, sp);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x1FD - n, 0, 0x20);
    let n = fx.mem.i32(D);
    let c = Fx::clamp_i(n, 0, 0x20);
    let v = (c - (c << 2)) << 5;
    let l = (fx.mult(s0, v) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xC0);
    if n == 0x118 {
        ret = 1;
    } else {
        if (0x123..0x1BD).contains(&n) {
            ret = 6;
        }
        if n == 0x1BD {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x1FE {
        ret = 8;
    }
    fx.mem.set32(sp + 0xC0, ret);
    ret
}

/// The quad descriptor at sp + 0x460 (prim 0x24 | 0x800, ST quads):
/// texture D+0x1C, CLAMP 5, ALPHA preset 0 (in that order).
fn quad_desc(fx: &mut Fx, sp: u32) {
    let q = sp + 0x460;
    head(fx, q, 0x24, 9, 0x4_2424_2421, 0x800);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 5);
    alpha(fx, q, 0);
}

/// The ST of cell `c` of a 4-wide strip (rows `v0` / `v1`) at the quad
/// descriptor's four vertices.
fn strip(fx: &mut Fx, sp: u32, c: i32, v0: u32, v1: u32) {
    let c = rem2(c, 4);
    let s0 = ((c << 10) & 0x3FFF) as u32;
    let s1 = ((((c << 6) + 0x40) << 4) & 0x3FFF) as u32;
    for (o, s, t) in [(0x478u32, s0, v0), (0x488, s1, v0), (0x498, s0, v1), (0x4A8, s1, v1)] {
        uv(fx, sp + o, s);
        uv(fx, sp + o + 2, t);
    }
}

/// The four corners at sp + 0x300 turned by `m` and moved to `at` into sp +
/// 0x340, projected into the descriptor `d`; drawn at its mean-w key.
fn quad(fx: &mut Fx, sp: u32, m: u32, at: u32, d: u32, ra: u32) {
    for k in 0..4u32 {
        let o = sp + 0x340 + 0x10 * k;
        fx.vu_apply_to(o, m, sp + 0x300 + 0x10 * k);
        fx.vu_add4(o, o, at);
    }
    if crate::fxlib2::project(fx, sp + 0x340, PROJ_OUT, sp + 0x240, d, sp + 0x53C) {
        let w = fx.mem.f(sp + 0x53C);
        let key = fix(fx, w, ra);
        fx.submit(d, OT, key as i32);
    }
}

/// Square corners (-s, -s) (s, -s) (-s, s) (s, s) at sp + 0x300.
fn square(fx: &mut Fx, sp: u32, s: f32) {
    let ns = ee::neg(s);
    vec4(fx, sp + 0x300, ns, ns);
    vec4(fx, sp + 0x310, s, ns);
    vec4(fx, sp + 0x320, ns, s);
    vec4(fx, sp + 0x330, s, s);
}

/// Copy of the camera turn (sp + 0x2C0) into sp + 0x380.
fn turn_copy(fx: &mut Fx, sp: u32) {
    for w in 0..0x10u32 {
        let x = fx.mem.read_u32(sp + 0x2C0 + 4 * w);
        fx.mem.write_u32(sp + 0x380 + 4 * w, x);
    }
}

/// The shells (D+0x70, 8 x 0x220) [F 0x1E0033C..0x1E0091C].
fn shells(fx: &mut Fx, sp: u32) {
    let k = fx.mem.i32(D + 0x60);
    if k < 8 && fx.mem.i32(D) == ((k << 3) + k) * 2 + 0xBC {
        let r = D + 0x220 * k as u32;
        fx.mem.write_u8(r + 0x70, 1);
        fx.mem.write_u32(r + 0x74, 0);
        let f20 = ee::div(ee::mul(f(0x3FA0_0000), ee::from_i32(k + 2)), 8.0);
        let c = sp + 0x3C0;
        fx.vu_sub4(c, D + 0x40, D + 0x20);
        fx.vu_scale4(c, c, f20);
        fx.vu_add4(c, c, D + 0x20);
        fx.mem.write_u32(c + 4, 0x4120_0000);
        for w in 0..4u32 {
            let x = fx.mem.read_u32(c + 4 * w);
            fx.mem.write_u32(r + 0x80 + 4 * w, x);
        }
        for s1 in 0..0x20u32 {
            fx.random_table_vec(sp + 0x3D0, 2.0);
            let y = ee::mul(fx.mem.f(sp + 0x3D4), f(0x3DCC_CCCD));
            fx.mem.setf(sp + 0x3D4, y);
            fx.vu_add4(r + 0x90 + 0x10 * s1, sp + 0x3D0, sp + 0x3C0);
        }
        fx.mem.set32(D + 0x60, k + 1);
    }
    quad_desc(fx, sp);
    camera(fx, sp);
    square(fx, sp, 10.0);
    for k in 0..8u32 {
        let r = D + 0x220 * k;
        if fx.mem.read_u8(r + 0x70) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + 0x74));
        let f20 = ee::mul(128.0, Fx::ratio_below(age, 32.0));
        let n = ee::from_i32(fx.mem.i32(D));
        let f0 = Fx::ramp_above(n, 477.0, 445.0);
        let a = fix(fx, ee::mul(f20, f0), 0x01E0_0654);
        rgba(fx, sp + 0x470, [0x40; 3], a as u8);
        for s5 in 0..0x20i32 {
            strip(fx, sp, s5, 0, 0x400);
            turn_copy(fx, sp);
            fx.rotate_axis(sp + 0x380, (s5 << 16) >> 5, 2);
            quad(fx, sp, sp + 0x380, r + 0x90 + 0x10 * s5 as u32, sp + 0x460, 0x01E0_0824);
        }
        let n = fx.mem.i32(D);
        if fx.mem.i32(r + 0x74) >= 0x31 && n < 0x1BD {
            let b = D + 0x700 * k + 0x70 * rem2(n, 0x10) as u32;
            fx.mem.write_u8(b + 0x1170, 1);
            fx.mem.write_u32(b + 0x1174, 0);
            for s1 in 0..6u32 {
                fx.random_table_vec(b + 0x10 * s1 + 0x1180, 1.0);
            }
        }
        let age = fx.mem.i32(r + 0x74) + 1;
        fx.mem.set32(r + 0x74, age);
        if fx.mem.i32(D) >= 0x1DE {
            fx.mem.write_u8(r + 0x70, 0);
        }
    }
}

/// The sprite descriptor at sp + 0x4D8 with texture D + `tex`, CLAMP 5,
/// ALPHA preset 1.
fn sprite_tex(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + 0x4D8;
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    alpha(fx, d, 1);
}

/// A sprite's corners at sp + 0x300 / 0x310: (-s, -s) and (s, s) turned by
/// the camera turn.
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    vec4(fx, sp + 0x300, ee::neg(s), ee::neg(s));
    vec4(fx, sp + 0x310, s, s);
    fx.vu_apply_to(sp + 0x300, sp + 0x2C0, sp + 0x300);
    fx.vu_apply_to(sp + 0x310, sp + 0x2C0, sp + 0x310);
}

/// The sprite at sp + 0x3C0 (corners at sp + 0x340 / 0x350), projected
/// into sp + 0x4D8.
fn sprite(fx: &mut Fx, sp: u32, ra: u32) {
    fx.vu_add4(sp + 0x340, sp + 0x300, sp + 0x3C0);
    fx.vu_add4(sp + 0x350, sp + 0x310, sp + 0x3C0);
    if crate::fxlib2::project(fx, sp + 0x340, PROJ_OUT, sp + 0x240, sp + 0x4D8, sp + 0x53C) {
        let w = fx.mem.f(sp + 0x53C);
        let key = fix(fx, w, ra);
        fx.submit(sp + 0x4D8, OT, key as i32);
    }
}

/// The burst records (D+0x1170 + 0x700 k, 16 x 0x70 a shell)
/// [F 0x1E00920..0x1E014D8].
fn bursts(fx: &mut Fx, sp: u32) {
    head(fx, sp + 0x4D8, 0x26, 5, 0x4_2421, 0x800);
    quad_desc(fx, sp);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(0x1BD - n, 0, 0x10)), 16.0);
    for k in 0..8u32 {
        let centre = D + 0x220 * k + 0x80;
        for j in 0..0x10u32 {
            let b = D + 0x700 * k + 0x70 * j;
            if fx.mem.read_u8(b + 0x1170) == 0 {
                continue;
            }
            // Fan A: (FF, 50, 2D) sprites on texture D+0x14 flung out.
            sprite_tex(fx, sp, 0x14);
            let size = |fx: &mut Fx| {
                let age = ee::from_i32(fx.mem.i32(b + 0x1174));
                ee::mul(12.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 16.0)))
            };
            let s = size(fx);
            corners(fx, sp, s);
            let age = fx.mem.i32(b + 0x1174);
            let c = fx.tcos((age << 14) >> 4);
            let a = fix(fx, ee::mul(f20, ee::mul(164.0, c)), 0x01E0_0B90);
            rgba(fx, sp + 0x4E8, [0xFF, 0x50, 0x2D], a as u8);
            let age = fx.mem.i32(b + 0x1174);
            let t = fx.tan_falloff_p(age, 0x10, 1);
            let f23 = ee::add(f(0x3ECC_CCCD), ee::mul(f(0x3F19_999A), t));
            let age = ee::from_i32(fx.mem.i32(b + 0x1174));
            let f22 = ee::div(ee::mul(70.0, age), 16.0);
            for s2 in 0..6i32 {
                let u = rem2(s2, 4);
                let w = s2 >> 2;
                uv(fx, sp + 0x4F0, ((u << 9) & 0x3FFF) as u32);
                uv(fx, sp + 0x4F2, ((w << 9) & 0x3FFF) as u32);
                uv(fx, sp + 0x500, ((((u << 5) + 0x20) << 4) & 0x3FFF) as u32);
                uv(fx, sp + 0x502, ((((w << 5) + 0x20) << 4) & 0x3FFF) as u32);
                fx.vu_scale4(sp + 0x3C0, b + 0x10 * s2 as u32 + 0x1180, f23);
                fx.vu_add4(sp + 0x3C0, sp + 0x3C0, centre);
                let y = ee::add(fx.mem.f(sp + 0x3C4), f22);
                fx.mem.setf(sp + 0x3C4, y);
                sprite(fx, sp, 0x01E0_0D4C);
            }
            // Fan B: (FF, 50, 0C) sprites on texture D+0x18 flung down.
            sprite_tex(fx, sp, 0x18);
            let s = size(fx);
            corners(fx, sp, s);
            let age = fx.mem.i32(b + 0x1174);
            let c = fx.tcos((age << 14) >> 4);
            let a = fix(fx, ee::mul(f20, ee::mul(128.0, c)), 0x01E0_0E70);
            rgba(fx, sp + 0x4E8, [0xFF, 0x50, 0x0C], a as u8);
            let age = fx.mem.i32(b + 0x1174);
            let t = fx.tan_falloff_p(age, 0x10, 1);
            let f22 = ee::mul(-1.0, ee::add(f(0x3ECC_CCCD), ee::mul(f(0x3F19_999A), t)));
            let age = ee::from_i32(fx.mem.i32(b + 0x1174));
            let f23 = ee::div(ee::mul(70.0, age), 16.0);
            for s1 in 0..6i32 {
                let u = rem2(s1, 4);
                uv(fx, sp + 0x4F0, ((u << 9) & 0x3FFF) as u32);
                uv(fx, sp + 0x4F2, 0x200);
                uv(fx, sp + 0x500, ((((u << 5) + 0x20) << 4) & 0x3FFF) as u32);
                uv(fx, sp + 0x502, 0x400);
                fx.vu_scale4(sp + 0x3C0, b + 0x10 * s1 as u32 + 0x1180, f22);
                fx.vu_add4(sp + 0x3C0, sp + 0x3C0, centre);
                let y = ee::add(fx.mem.f(sp + 0x3C4), f23);
                fx.mem.setf(sp + 0x3C4, y);
                sprite(fx, sp, 0x01E0_1014);
            }
            // Six falling (FF, 80, 80) quads, spun and growing.
            square(fx, sp, 3.0);
            let age = ee::from_i32(fx.mem.i32(b + 0x1174));
            let f0 = Fx::ramp_above(age, 16.0, 10.0);
            let a = fix(fx, ee::mul(128.0, f0), 0x01E0_10EC);
            rgba(fx, sp + 0x470, [0xFF, 0x80, 0x80], a as u8);
            let age = fx.mem.i32(b + 0x1174);
            let t = fx.tan_falloff_p(age, 0x10, 1);
            let f2 = ee::mul(2.0, ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), t)));
            let f22 = ee::mul(-1.0, f2);
            let age = ee::from_i32(fx.mem.i32(b + 0x1174));
            let p = fx.powf(age, 2.0);
            let f23 = ee::mul(5.0, ee::mul(0.5, ee::mul(f(0xBC32_864D), p)));
            let spin = ((j as i32) << 16) >> 4;
            for s6 in 0..6i32 {
                strip(fx, sp, s6, 0, 0x400);
                let v = b + 0x10 * s6 as u32 + 0x1180;
                fx.vu_scale4(sp + 0x3C0, v, f22);
                // fabsf's frame: the argument at its sp.
                let y = fx.mem.f(sp + 0x3C4);
                fx.mem.setf(sp - 0x10, y);
                fx.mem.write_u32(sp + 0x3C4, y.to_bits() & 0x7FFF_FFFF);
                fx.vu_add4(sp + 0x3C0, sp + 0x3C0, centre);
                let y = ee::add(fx.mem.f(sp + 0x3C4), f23);
                fx.mem.setf(sp + 0x3C4, y);
                let s3 = s6 << 16;
                fx.mult(0x2AAA_AAAB, s3);
                let q = fx.hilo().1.wrapping_add(((s3 as u32) >> 31) as i32);
                turn_copy(fx, sp);
                fx.rotate_axis(sp + 0x380, q.wrapping_add(spin), 2);
                let s = ee::add(f(0x3E4C_CCCD), ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(s6)), 5.0));
                fx.mem.setf(sp + 0x3D0, s);
                fx.mem.setf(sp + 0x3D4, s);
                fx.mem.write_u32(sp + 0x3D8, 0);
                fx.mem.write_u32(sp + 0x3DC, 0x3F80_0000);
                fx.scale_rows(sp + 0x380, sp + 0x3D0);
                quad(fx, sp, sp + 0x380, sp + 0x3C0, sp + 0x460, 0x01E0_140C);
            }
            let age = fx.mem.i32(b + 0x1174) + 1;
            fx.mem.set32(b + 0x1174, age);
            if age >= 0x11 {
                fx.mem.write_u8(b + 0x1170, 0);
            }
        }
    }
}

/// The frame-textured window sprite while clamp(n - 0xD6, 0, 0x10)
/// clamp(0x1CD - n, 0, 0x10) / 256 > 0 [F 0x1E014DC..0x1E016D4].
fn window(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n - 0xD6, 0, 0x10));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(0x1CD - n, 0, 0x10));
    let f20 = ee::div(ee::mul(a, b), 256.0);
    if crate::exe::ee_le(f20, 0.0) {
        return;
    }
    let d = sp + 0x4D8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    alpha(fx, d, 0);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.desc_clamp(d, 5);
    uv(fx, sp + 0x4F0, 0x50);
    uv(fx, sp + 0x4F2, 0x50);
    uv(fx, sp + 0x500, 0x1FB0);
    uv(fx, sp + 0x502, 0x1FB0);
    let a = fix(fx, ee::mul(64.0, f20), 0x01E0_1630);
    rgba(fx, sp + 0x4E8, [0xBC, 0x8A, 0x80], a as u8);
    far_xy(fx, sp + 0x4F8, 0x7000, 0x7000);
    far_xy(fx, sp + 0x508, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
}

/// The attacker drawn into its target with the callback and pulled
/// through three widening fans (D+0x4970, from 0x66 for 0xA9 updates)
/// [F 0x1E016D8..0x1E01C58].
fn attacker_pass(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x66 {
        fx.mem.write_u8(D + 0x4970, 1);
        fx.mem.write_u32(D + 0x4974, 0);
    }
    if fx.mem.read_u8(D + 0x4970) != 1 {
        return;
    }
    let t = sp + 0x3E8;
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0x800);
    alpha(fx, t, 0);
    fx.desc_clamp(t, LENS_CLAMP);
    let a = crate::b182::clamp_regs(fx, t, LENS_CLAMP);
    let layer = fx.layer_begin();
    fx.texflush(a);
    fx.mem.write_u32(D_STATIC, D);
    let (_, regs) = fx.unit_draw_post(1, sp + 0x448, sp + 0x450, CB, &mut |fx, p| callback(fx, p));
    fx.texflush(regs);
    let age = ee::from_i32(fx.mem.i32(D + 0x4974));
    let f20 = Fx::ratio_below(age, 48.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x4974));
    let f21 = Fx::ratio_below(age, 192.0);
    const RAS: [u32; 12] = [
        0x01E0_191C, 0x01E0_192C, 0x01E0_1970, 0x01E0_197C, 0x01E0_19C0, 0x01E0_19CC, 0x01E0_1A94, 0x01E0_1AA4, 0x01E0_1B10, 0x01E0_1B1C, 0x01E0_1B80,
        0x01E0_1B8C,
    ];
    for r in 0..3i32 {
        let f1 = ee::add(1.0, ee::div(ee::mul(f(0x3DCC_CCCD), ee::from_i32(r)), 2.0));
        let f22 = ee::sub(f1, f(0x3D4C_CCCD));
        let s0 = ((r + 1) << 15) >> 2;
        let sn = fx.tsin(s0);
        let a = fix(fx, ee::mul(f20, ee::mul(128.0, sn)), 0x01E0_1894) & 0xFF;
        let sn = fx.tsin(s0);
        let b = fix(fx, ee::mul(f21, ee::mul(128.0, sn)), 0x01E0_18B8);
        rgba(fx, sp + 0x3F8, [0x80; 3], a as u8);
        rgba(fx, sp + 0x410, [0x80; 3], b as u8);
        copy8(fx, sp + 0x428, sp + 0x410);
        crate::b424::fan(fx, t, D + 0x50, sp + 0x340, f22, 0, &RAS);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x4974) + 1;
    fx.mem.set32(D + 0x4974, age);
    if age >= 0xA9 {
        fx.mem.write_u8(D + 0x4970, 0);
    }
}

/// The post callback 0x1E01D80(p) [F]: run by 0x1F0FD0 after the
/// attacker's render into its target. FRAME_1 masked (0x2096C0
/// 0xFF000000), a TEXFLUSH, the frame drawn (0x207FC0 -1, filter 1) as the
/// texture of a full-window sprite (FF, FF, FF, 80) at ALPHA preset 0 and
/// the lens CLAMP (TEX1 merged into 0x209800's saved ra, the slot at sp +
/// 0xB0), then a (C8, 80, 00, FF) veil at preset 3, both
/// buffered; FRAME_1 unmasked.
fn callback(fx: &mut Fx, p: u32) -> u32 {
    let sp = fx.enter(0xC0);
    let s0 = crate::b440::frame_mask_packet(fx, p, 0xFF00_0000, [0, 0]);
    let d = sp + 0x68;
    head(fx, d, 0x26, 5, 0x4_2421, 0);
    let (a1, a2, a3) = mask_regs(fx);
    fx.boundary(0x0020_EA20, "packet writer 0x20EA20", [s0, a1, a2, a3]);
    let p1 = fx.texflush_packet(s0);
    // TEX1's slot holds 0x209800's saved ra (0x1F0FD0's call at 0x1F103C).
    fx.mem.write_u64(sp + 0xB0, 0x001F_1044);
    fx.mem.write_u64(sp + 0xB8, 0);
    let p2 = fx.frame_texture(p1, -1, 1, sp + 0xA8, sp + 0xB0);
    // 0x207FC0 without a copy leaves a1..a3 = 0, 0x1F, 0 [run].
    fx.boundary(0x0020_EA20, "packet writer 0x20EA20", [p2, 0, 0x1F, 0]);
    let s1 = fx.texflush_packet(p2);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
    rgba(fx, sp + 0x78, [0xFF; 3], 0x80);
    uv(fx, sp + 0x80, 0);
    uv(fx, sp + 0x82, 0);
    uv(fx, sp + 0x90, 0x2000);
    uv(fx, sp + 0x92, 0x2000);
    far_xy(fx, sp + 0x88, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x98, 0x8FF8, 0x8FF8);
    let s0 = fx.submit_at(s1, d, 0, 0);
    let t = sp + 0x38;
    head(fx, t, 6, 3, 0x441, 0);
    alpha(fx, t, 3);
    rgba(fx, sp + 0x48, [0xC8, 0x80, 0x00], 0xFF);
    far_xy(fx, sp + 0x50, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x58, 0x8FF8, 0x8FF8);
    let v = fx.submit_at(s0, t, 0, 0);
    let r = crate::b440::frame_mask_packet(fx, v, 0, [0, 0]);
    fx.leave(0xC0);
    r
}

/// a1..a3 as 0x2096C0 leaves them: FBP | FBW << 16, FBW << 16 and the
/// display record 0x2E93A0 (with the display flags 0x2E93C0 bits 0 and 1:
/// the FBP from 0x2E93AC, the FBW from 0x29322C, a3 = 40 x the index).
fn mask_regs(fx: &mut Fx) -> (u32, u32, u32) {
    let a3c = 0x002E_93A0u32;
    let t0 = if fx.mem.read_u32(0x002E_963C) & 1 != 0 { 0x002E_93D0 + 0x150 } else { 0x002E_93D0 + 0x60 };
    let flags = fx.mem.read_u8(a3c + 0x20);
    if flags & 1 != 0 && flags & 2 != 0 {
        let k = fx.mem.read_u32(a3c);
        let fbw = ((fx.mem.read_u32(0x0029_322C + 0x28 * k) as i32 >> 6) << 16) as u32;
        let fbp = fx.mem.read_u32(a3c + 0xC);
        (fbp | fbw, fbw, k.wrapping_mul(40))
    } else {
        let fbw = (u32::from(fx.mem.read_u8(t0 + 2)) & 0x3F) << 16;
        let fbp = u32::from(fx.mem.read_u16(t0)) & 0x1FF;
        (fbp | fbw, fbw, a3c)
    }
}
