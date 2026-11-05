//! BMODULE b453 (card 634), hand-ported from the record (linked at
//! 0x1E00000, the entry 0x1E00080 in a 0x6F0 frame with D in s7 and the
//! OT in s0, data area a0 = 0x1ED0000, 0xA7C0 bytes; one attacker post
//! callback 0x1E02F00 with its static D at 0x1E03200) [F] (helper A32,
//! 3 Nov 2026). Each update: the attacker's / target's screen points
//! (D+0x50 / D+0x60, b424's clamped 0x204D80 + 0x17BC10 points); a lens at
//! the target (D+0x1B70 from 0xB5 for 0x4C: b424's eight-triangle fan of
//! the frame 8 times, 1 + 0.15 (8 - j) / 8); a frame-textured window on
//! 0x20..0x10A; 16 flame records at the target on 0xB5..0xF9 (D+0xE0,
//! 0xD0: 12 + 12 sprites and 12 turned quads along 2.0 table vectors); six
//! smoke records at the two hand nodes on 0x18 + 25 k (D+0xDF0, 0x240);
//! 28 ember records (D+0x1B80, 0x440) and 20 spark records (D+0x9280,
//! 0x110) at the three nodes 0x1B / 0x1C / 0x20 on 0x21..0x109; from 0x20
//! for 0xEB updates the attacker drawn into its target with b768's
//! callback (frame sprite + (C8, 76, 00) veil) and laid out as five fans
//! about its screen point; the stage light (96 / 1024); returns 1 / 6 / 2
//! / 8 on 0xAA / 0xB5..0xF9 / 0xFA / 0x12B.
use crate::b182::{alpha, clamp_regs, head, rgba, uv};
use crate::b440::{copy8, fix, rem2};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x6F0;
const CB: u32 = 0x01E0_2F00;
const D_STATIC: u32 = 0x01E0_3200;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E00080 of b453 (card 634).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

/// rand() >> 23 from `ra` (the caller's s0 = the OT).
fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(OT, ra) >> 23
}

fn vec4(fx: &mut Fx, p: u32, x: f32, y: f32, z: f32) {
    fx.mem.setf(p, x);
    fx.mem.setf(p + 4, y);
    fx.mem.setf(p + 8, z);
    fx.mem.write_u32(p + 0xC, 0x3F80_0000);
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xA7C0);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let side = fx.unit_side(h);
    fx.mem.set32(D + 0xC, side);
    for id in 0..4u32 {
        let r = fx.texture(5, id as i32);
        fx.mem.write_u32(D + 0x10 + 4 * id, r);
    }
    let h = fx.mem.i32(D + 4);
    fx.unit_position(h, D + 0x20);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x30);
    let y = ee::add(fx.mem.f(D + 0x34), -20.0);
    fx.mem.setf(D + 0x34, y);
    for (at, node, kind) in [(0x70u32, 0x10, 1), (0x74, 9, 0), (0xA0, 0x1B, 0), (0xA4, 0x1C, 0), (0xA8, 0x20, 0)] {
        let h = fx.mem.i32(D + 4);
        let s = fx.request_node(h, node, kind);
        fx.mem.set32(D + at, s);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0xE0 + 0xD0 * k, 0);
    }
    fx.mem.write_u8(D + 0xDE0, 0);
    for k in 0..6u32 {
        fx.mem.write_u8(D + 0xDF0 + 0x240 * k, 0);
    }
    fx.mem.write_u8(D + 0x1B70, 0);
    for k in 0..0x1Cu32 {
        fx.mem.write_u8(D + 0x1B80 + 0x440 * k, 0);
    }
    for k in 0..0x14u32 {
        fx.mem.write_u8(D + 0x9280 + 0x110 * k, 0);
    }
    0
}

fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x3F0);
    fx.identity(sp + 0x430);
    crate::blib::exe::mul_mem(fx, sp + 0x3F0, sp + 0x430);
}

/// The screen point of `p` (0x204D80 through sp + 0x3F0 into sp + 0x6E0)
/// into `o`: x / y ((x16 >> 4) - 0x700), Z (0x17BC10), 1.0, x / y clamped
/// to 0..512 (b424's).
fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    let m = fx.read_m4(sp + 0x3F0);
    fx.project_at(p, &m, sp + 0x6E0, sp + 0x6EC);
    let x = (i32::from(fx.mem.read_u16(sp + 0x6E0)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x6E2)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x6E0) << 8) >> 40;
    fx.mem.setf(o + 8, z as u32 as f32);
    fx.mem.write_u32(o + 0xC, 0x3F80_0000);
    for k in [0u32, 4] {
        if crate::exe::ee_lt(fx.mem.f(o + k), 0.0) {
            fx.mem.write_u32(o + k, 0);
        }
    }
    for k in [0u32, 4] {
        if !crate::exe::ee_le(fx.mem.f(o + k), 512.0) {
            fx.mem.write_u32(o + k, 0x4400_0000);
        }
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x470);
    for k in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x70 + 4 * k);
        fx.node_position(h, s, D + 0x80 + 0x10 * k);
    }
    for k in 0..3u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0xA0 + 4 * k);
        fx.node_position(h, s, D + 0xB0 + 0x10 * k);
    }
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x40);
    camera(fx, sp);
    screen_point(fx, sp, D + 0x40, D + 0x50);
    camera(fx, sp);
    screen_point(fx, sp, D + 0x30, D + 0x60);

    lens(fx, sp);
    window(fx, sp);
    flames(fx, sp);
    smoke(fx, sp);
    embers(fx, sp);
    sparks(fx, sp);
    attacker_pass(fx, sp);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x12A - n, 0, 0x20);
    let c = Fx::clamp_i(n, 0, 0x20);
    let l = (fx.mult(s0, c.wrapping_neg().wrapping_mul(96)) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xB0);
    if n == 0xAA {
        ret = 1;
    } else {
        if n >= 0xB5 && n < 0xFA {
            ret = 6;
        }
        if n == 0xFA {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x12B {
        ret = 8;
    }
    fx.mem.set32(sp + 0xB0, ret);
    ret
}

/// The fan triangle descriptor at sp + 0x598 (prim 0x33 | 0x800), ALPHA
/// preset 0, the lens CLAMP; a0..a3 as 0x208400 leaves them.
fn fan_desc(fx: &mut Fx, sp: u32) -> [u32; 4] {
    let t = sp + 0x598;
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0x800);
    alpha(fx, t, 0);
    fx.desc_clamp(t, LENS_CLAMP);
    clamp_regs(fx, t, LENS_CLAMP)
}

const LENS_RAS: [u32; 12] = [
    0x01E0_06D8, 0x01E0_06E4, 0x01E0_0728, 0x01E0_0734, 0x01E0_0778, 0x01E0_0784, 0x01E0_0838, 0x01E0_0844, 0x01E0_08B0, 0x01E0_08BC, 0x01E0_0928,
    0x01E0_0934,
];
const PASS_RAS: [u32; 12] = [
    0x01E0_2AD0, 0x01E0_2ADC, 0x01E0_2B20, 0x01E0_2B2C, 0x01E0_2B70, 0x01E0_2B7C, 0x01E0_2C30, 0x01E0_2C3C, 0x01E0_2CA8, 0x01E0_2CB4, 0x01E0_2D24,
    0x01E0_2D30,
];

/// The lens at the target's screen point (D+0x1B70 from 0xB5 for 0x4C
/// updates) [F 0x1E00518..0x1E00A00].
fn lens(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0xB5 {
        fx.mem.write_u8(D + 0x1B70, 1);
        fx.mem.write_u32(D + 0x1B74, 0);
    }
    if fx.mem.read_u8(D + 0x1B70) != 1 {
        return;
    }
    let a = fan_desc(fx, sp);
    let t = sp + 0x598;
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x5F8, sp + 0x600);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0x1B74);
    let x = (0x4C - age).wrapping_mul(56);
    fx.mult(0x6BCA_1AF3, x);
    let al = (fx.hilo().1 >> 5).wrapping_add(((x as u32) >> 31) as i32);
    rgba(fx, sp + 0x5A8, [0x80; 3], al as u8);
    copy8(fx, sp + 0x5C0, sp + 0x5A8);
    copy8(fx, sp + 0x5D8, sp + 0x5A8);
    for j in 0..8i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f(0x3E19_999A), ee::from_i32(8 - j)), 8.0));
        crate::b424::fan(fx, t, D + 0x60, sp + 0x4F0, s, 0, &LENS_RAS);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x1B74) + 1;
    fx.mem.set32(D + 0x1B74, age);
    if age >= 0x4D {
        fx.mem.write_u8(D + 0x1B70, 0);
    }
}

/// The frame-textured window while clamp(n - 0x20, 0, 0x10) clamp(0x10A -
/// n, 0, 0x10) / 256 > 0 [F 0x1E00A04..0x1E00BD8].
fn window(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n - 0x20, 0, 0x10));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(0x10A - n, 0, 0x10));
    let f20 = ee::div(ee::mul(a, b), 256.0);
    if crate::exe::ee_le(f20, 0.0) {
        return;
    }
    let d = sp + 0x688;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    alpha(fx, d, 0);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.desc_clamp(d, 5);
    uv(fx, sp + 0x6A0, 0x50);
    uv(fx, sp + 0x6A2, 0x50);
    uv(fx, sp + 0x6B0, 0x1FB0);
    uv(fx, sp + 0x6B2, 0x1FB0);
    let a = fix(fx, ee::mul(64.0, f20), 0x01E0_0B34);
    rgba(fx, sp + 0x698, [0xA8, 0x8A, 0x80], a as u8);
    far_xy(fx, sp + 0x6A8, 0x7000, 0x7000);
    far_xy(fx, sp + 0x6B8, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
}

/// The sprite descriptor at sp + 0x688 in the order the record builds it
/// (`order`: texture / clamp / alpha as 't' / 'c' / 'a').
fn sprite_desc(fx: &mut Fx, d: u32, tex: u32, preset: i32, order: &[u8; 3]) {
    for &o in order {
        match o {
            b't' => {
                let rh = fx.mem.read_u32(D + tex);
                fx.desc_texture(d, rh, 1);
            }
            b'c' => fx.desc_clamp(d, 5),
            _ => alpha(fx, d, preset),
        }
    }
}

fn sprite_head(fx: &mut Fx, d: u32) {
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
}

/// Corners (-s, -s) / (s, s) at sp + 0x4B0 / 0x4C0 turned by the camera
/// turn.
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    vec4(fx, sp + 0x4B0, ee::neg(s), ee::neg(s), 0.0);
    vec4(fx, sp + 0x4C0, s, s, 0.0);
    fx.vu_apply_to(sp + 0x4B0, sp + 0x470, sp + 0x4B0);
    fx.vu_apply_to(sp + 0x4C0, sp + 0x470, sp + 0x4C0);
}

/// Cell `c` of a 4 x 2 atlas of 32 into the sprite's UVs (sp + 0x6A0..).
fn atlas(fx: &mut Fx, sp: u32, c: i32) {
    let u = rem2(c, 4);
    let w = c >> 2;
    uv(fx, sp + 0x6A0, ((u << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x6A2, ((w << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x6B0, ((((u << 5) + 0x20) << 4) & 0x3FFF) as u32);
    uv(fx, sp + 0x6B2, ((((w << 5) + 0x20) << 4) & 0x3FFF) as u32);
}

/// The sprite about sp + 0x570 (corners sp + 0x4F0 / 0x500), unkeyed.
fn sprite_at(fx: &mut Fx, sp: u32) {
    fx.vu_add4(sp + 0x4F0, sp + 0x4B0, sp + 0x570);
    fx.vu_add4(sp + 0x500, sp + 0x4C0, sp + 0x570);
    let d = sp + 0x688;
    if crate::fxlib2::project(fx, sp + 0x4F0, PROJ_OUT, sp + 0x3F0, d, sp + 0x6EC) {
        fx.submit(d, OT, 0);
    }
}

/// The flames (D+0xE0, 16 x 0xD0): on 0xB5..0xF9 a record of 12 2.0
/// table vectors at the target; drawn as 12 rising sprites (texture
/// D+0x14), 12 falling ones (D+0x18) and 12 turned shrinking quads
/// (D+0x1C) [F 0x1E00BDC..0x1E01800].
fn flames(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= 0xB5 && n < 0xFA {
        let r = D + 0xD0 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0xE0, 1);
        fx.mem.write_u32(r + 0xE4, 0);
        for k in 0..0xCu32 {
            fx.random_table_vec(r + 0xF0 + 0x10 * k, 2.0);
        }
    }
    let d = sp + 0x688;
    sprite_head(fx, d);
    let q = sp + 0x610;
    head(fx, q, 0x24, 9, 0x4_2424_2421, 0x800);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 5);
    alpha(fx, q, 0);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(0xFA - n, 0, 0x10)), 16.0);
    for k in 0..0x10u32 {
        let r = D + 0xD0 * k;
        if fx.mem.read_u8(r + 0xE0) == 0 {
            continue;
        }
        // Rising sprites.
        sprite_desc(fx, d, 0x14, 1, b"tca");
        let agef = ee::from_i32(fx.mem.i32(r + 0xE4));
        let s = ee::mul(15.0, ee::add(0.5, ee::div(ee::mul(0.5, agef), 16.0)));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 0xE4);
        let c = fx.tcos((age << 14) >> 4);
        let a = fix(fx, ee::mul(f20, ee::mul(180.0, c)), 0x01E0_0EC4);
        rgba(fx, sp + 0x698, [0xFF, 0x6E, 0x2D], a as u8);
        let age = fx.mem.i32(r + 0xE4);
        let t = fx.tan_falloff_p(age, 0x10, 1);
        let f23 = ee::add(f(0x3ECC_CCCD), ee::mul(f(0x3F19_999A), t));
        let agef = ee::from_i32(fx.mem.i32(r + 0xE4));
        let f22 = ee::div(ee::mul(90.0, agef), 16.0);
        for s1 in 0..0xCi32 {
            atlas(fx, sp, s1);
            fx.vu_scale4(sp + 0x570, r + 0xF0 + 0x10 * s1 as u32, f23);
            fx.vu_add4(sp + 0x570, sp + 0x570, D + 0x30);
            let y = ee::add(fx.mem.f(sp + 0x574), f22);
            fx.mem.setf(sp + 0x574, y);
            sprite_at(fx, sp);
        }
        // Falling sprites.
        sprite_desc(fx, d, 0x18, 1, b"tca");
        let agef = ee::from_i32(fx.mem.i32(r + 0xE4));
        let s = ee::mul(15.0, ee::add(0.5, ee::div(ee::mul(0.5, agef), 16.0)));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 0xE4);
        let c = fx.tcos((age << 14) >> 4);
        let a = fix(fx, ee::mul(f20, ee::mul(180.0, c)), 0x01E0_1180);
        rgba(fx, sp + 0x698, [0xFF, 0x6E, 0x0C], a as u8);
        let age = fx.mem.i32(r + 0xE4);
        let t = fx.tan_falloff_p(age, 0x10, 1);
        let g = ee::div(ee::mul(2.0, ee::add(f(0x3ECC_CCCD), ee::mul(f(0x3F19_999A), t))), 2.0);
        let f23 = ee::mul(-1.0, g);
        let agef = ee::from_i32(fx.mem.i32(r + 0xE4));
        let f22 = ee::div(ee::mul(90.0, agef), 16.0);
        for s2 in 0..0xCi32 {
            let u = rem2(s2, 4);
            uv(fx, sp + 0x6A0, ((u << 9) & 0x3FFF) as u32);
            uv(fx, sp + 0x6A2, 0x200);
            uv(fx, sp + 0x6B0, ((((u << 5) + 0x20) << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x6B2, 0x400);
            fx.vu_scale4(sp + 0x570, r + 0xF0 + 0x10 * s2 as u32, f23);
            fx.vu_add4(sp + 0x570, sp + 0x570, D + 0x30);
            let y = ee::add(fx.mem.f(sp + 0x574), f22);
            fx.mem.setf(sp + 0x574, y);
            sprite_at(fx, sp);
        }
        // Turned quads.
        let f21 = -3.5f32;
        for (o, x, y) in [(0x4B0u32, f21, f21), (0x4C0, 3.5, f21), (0x4D0, f21, 3.5), (0x4E0, 3.5, 3.5)] {
            vec4(fx, sp + o, x, y, 0.0);
        }
        let agef = ee::from_i32(fx.mem.i32(r + 0xE4));
        let ra = Fx::ramp_above(agef, 16.0, 10.0);
        let a = fix(fx, ee::mul(128.0, ra), 0x01E0_13EC);
        rgba(fx, sp + 0x620, [0xFF, 0x80, 0x80], a as u8);
        let age = fx.mem.i32(r + 0xE4);
        let t = fx.tan_falloff_p(age, 0x10, 1);
        let g = ee::div(ee::mul(2.5, ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), t))), 2.0);
        let f23 = ee::mul(-1.0, g);
        let agef = ee::from_i32(fx.mem.i32(r + 0xE4));
        let p = fx.powf(agef, 2.0);
        let f22 = ee::sub(-10.0, ee::mul(5.0, ee::mul(0.5, ee::mul(f(0x3C32_864D), p))));
        let base = (k << 16) as i32 >> 4;
        for s6 in 0..0xCi32 {
            let u = rem2(s6, 4);
            let u0 = ((u << 10) & 0x3FFF) as u32;
            let u1 = ((((u << 6) + 0x40) << 4) & 0x3FFF) as u32;
            for (o, v) in [(0x628u32, u0), (0x62A, 0), (0x638, u1), (0x63A, 0), (0x648, u0), (0x64A, 0x400), (0x658, u1), (0x65A, 0x400)] {
                uv(fx, sp + o, v);
            }
            fx.vu_scale4(sp + 0x570, r + 0xF0 + 0x10 * s6 as u32, f23);
            // fabsf's frame: the argument at its sp.
            let y = fx.mem.f(sp + 0x574);
            fx.mem.setf(sp - 0x10, y);
            fx.mem.setf(sp + 0x574, f32::from_bits(y.to_bits() & 0x7FFF_FFFF));
            fx.vu_add4(sp + 0x570, sp + 0x570, D + 0x30);
            let y = ee::add(fx.mem.f(sp + 0x574), f22);
            fx.mem.setf(sp + 0x574, y);
            for w in 0..0x10u32 {
                let x = fx.mem.read_u32(sp + 0x470 + 4 * w);
                fx.mem.write_u32(sp + 0x530 + 4 * w, x);
            }
            let x = s6 << 16;
            fx.mult(0x2AAA_AAAB, x);
            let a = (fx.hilo().1 >> 1).wrapping_add(((x as u32) >> 31) as i32);
            fx.rotate_axis(sp + 0x530, a.wrapping_add(base), 2);
            let s = ee::add(f(0x3E4C_CCCD), ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(s6)), 11.0));
            fx.mem.setf(sp + 0x580, s);
            fx.mem.setf(sp + 0x584, s);
            fx.mem.write_u32(sp + 0x588, 0);
            fx.mem.write_u32(sp + 0x58C, 0x3F80_0000);
            fx.scale_rows(sp + 0x530, sp + 0x580);
            for v in 0..4u32 {
                fx.vu_apply_to(sp + 0x4F0 + 0x10 * v, sp + 0x530, sp + 0x4B0 + 0x10 * v);
                fx.vu_add4(sp + 0x4F0 + 0x10 * v, sp + 0x4F0 + 0x10 * v, sp + 0x570);
            }
            if crate::fxlib2::project(fx, sp + 0x4F0, PROJ_OUT, sp + 0x3F0, q, sp + 0x6EC) {
                fx.submit(q, OT, 0);
            }
        }
        let age = fx.mem.i32(r + 0xE4) + 1;
        fx.mem.set32(r + 0xE4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r + 0xE0, 0);
        }
    }
}

/// The smoke (D+0xDF0, 6 x 0x240): on 0x18 + 25 k (k = the cursor D+0xDE8
/// below 6) 32 grey sprites from hand node k % 2 along 1.0 table vectors
/// (y |y| 0.8) [F 0x1E01804..0x1E01CFC].
fn smoke(fx: &mut Fx, sp: u32) {
    let c = fx.mem.i32(D + 0xDE8);
    let n = fx.mem.i32(D);
    if n == c.wrapping_mul(25) + 0x18 && c < 6 {
        let r = D + 0x240 * c as u32;
        fx.mem.write_u8(r + 0xDF0, 1);
        fx.mem.write_u32(r + 0xDF4, 0);
        let src = D + 0x10 * rem2(c, 2) as u32 + 0x80;
        for w in 0..4u32 {
            let x = fx.mem.read_u32(src + 4 * w);
            fx.mem.write_u32(r + 0xE20 + 4 * w, x);
        }
        for s2 in 0..0x20u32 {
            let b = rem2(rnd(fx, 0x01E0_18C0), 8);
            fx.mem.write_u8(r + 0xDF8 + s2, b as u8);
            let p = r + 0xE30 + 0x10 * s2;
            fx.random_table_vec(p, 1.0);
            let y = fx.mem.f(p + 4);
            fx.mem.setf(sp - 0x10, y);
            fx.mem.setf(p + 4, f32::from_bits(y.to_bits() & 0x7FFF_FFFF));
            let y = ee::mul(fx.mem.f(p + 4), f(0x3F4C_CCCD));
            fx.mem.setf(p + 4, y);
        }
        fx.mem.set32(D + 0xDE8, c + 1);
    }
    let d = sp + 0x688;
    sprite_head(fx, d);
    sprite_desc(fx, d, 0x14, 0, b"atc");
    camera(fx, sp);
    for k in 0..6u32 {
        let r = D + 0x240 * k;
        if fx.mem.read_u8(r + 0xDF0) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0xDF4);
        let cs = fx.tcos((age << 14) >> 5);
        let a = fix(fx, ee::mul(128.0, cs), 0x01E0_1A2C);
        rgba(fx, sp + 0x698, [0x58; 3], a as u8);
        let agef = ee::from_i32(fx.mem.i32(r + 0xDF4));
        let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, agef), 32.0)));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 0xDF4);
        let t = fx.tan_falloff_p(age, 0x20, 1);
        let f21 = ee::add(f(0x3E99_999A), ee::mul(f(0x3F33_3333), t));
        for s2 in 0..0x20u32 {
            fx.vu_scale4(sp + 0x570, r + 0xE30 + 0x10 * s2, f21);
            fx.vu_add4(sp + 0x570, sp + 0x570, r + 0xE20);
            let y = ee::add(fx.mem.f(sp + 0x574), -10.0);
            fx.mem.setf(sp + 0x574, y);
            fx.vu_add4(sp + 0x4F0, sp + 0x4B0, sp + 0x570);
            fx.vu_add4(sp + 0x500, sp + 0x4C0, sp + 0x570);
            let b = i32::from(fx.mem.read_u8(r + 0xDF8 + s2));
            atlas(fx, sp, b);
            if crate::fxlib2::project(fx, sp + 0x4F0, PROJ_OUT, sp + 0x3F0, d, sp + 0x6EC) {
                fx.submit(d, OT, 0);
            }
        }
        let age = fx.mem.i32(r + 0xDF4) + 1;
        fx.mem.set32(r + 0xDF4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r + 0xDF0, 0);
        }
    }
}

/// clamp(n - 0x20, 0, 0x20) clamp(0x10A - n, 0, 0x20) / 1024.
fn level(fx: &mut Fx) -> f32 {
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n - 0x20, 0, 0x20));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(0x10A - n, 0, 0x20));
    ee::div(ee::mul(a, b), 1024.0)
}

/// Emitter records at the three nodes (D+0xB0..): `base` the pool, `n`
/// records of `stride`, `per` sprites per node, the cell mask `cells`,
/// vectors of `len`, the spawn return address.
struct Pool {
    base: u32,
    count: i32,
    stride: u32,
    per: u32,
    cells: i32,
    len: f32,
    /// Offsets of the node points / cells / vectors in a record.
    pts: u32,
    bytes: u32,
    vecs: u32,
    ra: u32,
}

const EMBERS: Pool = Pool { base: 0x1B80, count: 0x1C, stride: 0x440, per: 0x14, cells: 4, len: f(0x3F19_999A), pts: 0x50, bytes: 8, vecs: 0x80, ra: 0x01E0_1D98 };
const SPARKS: Pool = Pool { base: 0x9280, count: 0x14, stride: 0x110, per: 4, cells: 8, len: 0.5, pts: 0x20, bytes: 8, vecs: 0x50, ra: 0x01E0_2328 };

fn spawn(fx: &mut Fx, p: &Pool) {
    let n = fx.mem.i32(D);
    if !(n >= 0x21 && n < 0x10A) {
        return;
    }
    let (_, k) = fx.div(n, p.count);
    let r = D + p.stride * k as u32 + p.base;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    for j in 0..3u32 {
        for w in 0..4u32 {
            let x = fx.mem.read_u32(D + 0xB0 + 0x10 * j + 4 * w);
            fx.mem.write_u32(r + p.pts + 0x10 * j + 4 * w, x);
        }
        for s2 in 0..p.per {
            let c = rem2(rnd(fx, p.ra), p.cells);
            fx.mem.write_u8(r + p.bytes + p.per * j + s2, c as u8);
            fx.random_table_vec(r + p.vecs + 0x10 * p.per * j + 0x10 * s2, p.len);
        }
    }
}

/// The embers (D+0x1B80, 28 x 0x440): 3 x 20 (FF, 32, 14) sprites rising
/// 8 age / 28 from the nodes along 0.6 table vectors [F 0x1E01D00..0x1E02274].
fn embers(fx: &mut Fx, sp: u32) {
    spawn(fx, &EMBERS);
    let d = sp + 0x688;
    sprite_head(fx, d);
    sprite_desc(fx, d, 0x10, 1, b"cta");
    camera(fx, sp);
    let f20 = level(fx);
    for k in 0..0x1Cu32 {
        let r = D + 0x1B80 + 0x440 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let x = fx.mem.i32(r + 4) << 14;
        fx.mult(0x9249_2493u32 as i32, x);
        let q = (fx.hilo().1.wrapping_add(x) >> 4).wrapping_add(((x as u32) >> 31) as i32);
        let cs = fx.tcos(q);
        let a = fix(fx, ee::mul(f20, ee::mul(128.0, cs)), 0x01E0_1F5C);
        fx.mem.write_u8(sp + 0x3E0, a as u8);
        let age = fx.mem.i32(r + 4);
        let s = ee::mul(f(0x3F4C_CCCD), ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(0x1C - age)), 28.0)));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, 0x1C, 1);
        let f21 = ee::add(f(0x3F4C_CCCD), ee::mul(f(0x3E4C_CCCD), t));
        let agef = ee::from_i32(fx.mem.i32(r + 4));
        let f22 = ee::div(ee::mul(8.0, agef), 28.0);
        draw_nodes(fx, sp, r, &EMBERS, f21, f22, 0x3E0);
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x1D {
            fx.mem.write_u8(r, 0);
        }
    }
}

fn draw_nodes(fx: &mut Fx, sp: u32, r: u32, p: &Pool, f21: f32, f22: f32, a_at: u32) {
    for j in 0..3u32 {
        let a = fx.mem.read_u8(sp + a_at);
        rgba(fx, sp + 0x698, [0xFF, 0x32, 0x14], a);
        for s2 in 0..p.per {
            let c = i32::from(fx.mem.read_u8(r + p.bytes + p.per * j + s2));
            atlas(fx, sp, c);
            fx.vu_scale4(sp + 0x570, r + p.vecs + 0x10 * p.per * j + 0x10 * s2, f21);
            fx.vu_add4(sp + 0x570, sp + 0x570, r + p.pts + 0x10 * j);
            let y = ee::add(fx.mem.f(sp + 0x574), f22);
            fx.mem.setf(sp + 0x574, y);
            sprite_at(fx, sp);
        }
    }
}

/// The sparks (D+0x9280, 20 x 0x110): 3 x 4 sprites from the nodes along
/// 0.5 table vectors [F 0x1E02278..0x1E0284C].
fn sparks(fx: &mut Fx, sp: u32) {
    spawn(fx, &SPARKS);
    let d = sp + 0x688;
    sprite_head(fx, d);
    sprite_desc(fx, d, 0x14, 1, b"cta");
    camera(fx, sp);
    let f20 = level(fx);
    for k in 0..0x14u32 {
        let r = D + 0x9280 + 0x110 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let x = fx.mem.i32(r + 4) << 14;
        fx.mult(0x6666_6667, x);
        let q = (fx.hilo().1 >> 3).wrapping_add(((x as u32) >> 31) as i32);
        let cs = fx.tcos(q);
        let a = fix(fx, ee::mul(f20, ee::mul(128.0, cs)), 0x01E0_24FC);
        fx.mem.write_u8(sp + 0x3D0, a as u8);
        let age = fx.mem.i32(r + 4);
        let s = ee::mul(5.0, ee::add(f(0x3E99_999A), ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(0x14 - age)), 20.0)));
        corners(fx, sp, s);
        let agef = ee::from_i32(fx.mem.i32(r + 4));
        let f21 = ee::add(f(0x3E4C_CCCD), ee::div(ee::mul(f(0x3F4C_CCCD), agef), 20.0));
        let f22 = ee::div(ee::mul(6.0, agef), 20.0);
        draw_nodes(fx, sp, r, &SPARKS, f21, f22, 0x3D0);
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x15 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The attacker drawn into its target with the callback and laid out as
/// five fans about its screen point (D+0xDE0 from 0x20 for 0xEB updates)
/// [F 0x1E02850..0x1E02DF0].
fn attacker_pass(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x20 {
        fx.mem.write_u8(D + 0xDE0, 1);
        fx.mem.write_u32(D + 0xDE4, 0);
    }
    if fx.mem.read_u8(D + 0xDE0) != 1 {
        return;
    }
    let a = fan_desc(fx, sp);
    let t = sp + 0x598;
    let layer = fx.layer_begin();
    fx.texflush(a);
    fx.mem.write_u32(D_STATIC, D);
    let (_, regs) = fx.unit_draw_post(1, sp + 0x5F8, sp + 0x600, CB, &mut |fx, p| callback(fx, p));
    fx.texflush(regs);
    let age = ee::from_i32(fx.mem.i32(D + 0xDE4));
    let r1 = Fx::ratio_below(age, 48.0);
    let age = ee::from_i32(fx.mem.i32(D + 0xDE4));
    let f20 = ee::mul(r1, Fx::ramp_above(age, 234.0, 186.0));
    let age = ee::from_i32(fx.mem.i32(D + 0xDE4));
    let r2 = Fx::ratio_below(age, 192.0);
    let age = ee::from_i32(fx.mem.i32(D + 0xDE4));
    let f21 = ee::mul(r2, Fx::ramp_above(age, 234.0, 186.0));
    for j in 0..5i32 {
        let f22 = ee::sub(ee::add(1.0, ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(j)), 4.0)), f(0x3DCC_CCCD));
        let x = (j + 1) << 15;
        fx.mult(0x2AAA_AAAB, x);
        let s1 = fx.hilo().1.wrapping_add(((x as u32) >> 31) as i32);
        let sn = fx.tsin(s1);
        let a = fix(fx, ee::mul(f20, ee::mul(128.0, sn)), 0x01E0_2A4C) & 0xFF;
        let sn = fx.tsin(s1);
        let b = fix(fx, ee::mul(f21, ee::mul(128.0, sn)), 0x01E0_2A70);
        rgba(fx, sp + 0x5A8, [0x80; 3], a as u8);
        rgba(fx, sp + 0x5C0, [0x80; 3], b as u8);
        copy8(fx, sp + 0x5D8, sp + 0x5C0);
        crate::b424::fan(fx, t, D + 0x50, sp + 0x4F0, f22, 0, &PASS_RAS);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0xDE4) + 1;
    fx.mem.set32(D + 0xDE4, age);
    if age >= 0xEB {
        fx.mem.write_u8(D + 0xDE0, 0);
    }
}

/// The post callback 0x1E02F00(p) [F]: b768's (FRAME_1 masked, a TEXFLUSH,
/// the frame drawn (0x207FC0 -1, filter 1) as the texture of a full-window
/// sprite (FF, FF, FF, 80) at ALPHA preset 0 and the lens CLAMP (TEX1
/// merged into 0x209800's saved ra, the slot at sp + 0xB0), then a (C8, 76,
/// 00, FF) veil at preset 3, both buffered; FRAME_1 unmasked).
fn callback(fx: &mut Fx, p: u32) -> u32 {
    let sp = fx.enter(0xC0);
    let s0 = crate::b440::frame_mask_packet(fx, p, 0xFF00_0000, [0, 0]);
    let d = sp + 0x68;
    head(fx, d, 0x26, 5, 0x4_2421, 0);
    let (a1, a2, a3) = mask_regs(fx);
    fx.boundary(0x0020_EA20, "packet writer 0x20EA20", [s0, a1, a2, a3]);
    let p1 = fx.texflush_packet(s0);
    fx.mem.write_u64(sp + 0xB0, 0x001F_1044);
    fx.mem.write_u64(sp + 0xB8, 0);
    let p2 = fx.frame_texture(p1, -1, 1, sp + 0xA8, sp + 0xB0);
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
    rgba(fx, sp + 0x48, [0xC8, 0x76, 0x00], 0xFF);
    far_xy(fx, sp + 0x50, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x58, 0x8FF8, 0x8FF8);
    let v = fx.submit_at(s0, t, 0, 0);
    let r = crate::b440::frame_mask_packet(fx, v, 0, [0, 0]);
    fx.leave(0xC0);
    r
}

/// a1..a3 as 0x2096C0 leaves them (b768's).
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
