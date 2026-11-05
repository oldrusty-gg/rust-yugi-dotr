//! BMODULE b290 (card 618), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xE390
//! bytes) [F]: a single unscheduled function in
//! a 0x460 frame (D kept at sp + 0xD0, s0 = the OT, the return at sp +
//! 0xC0). Every update the camera turn with roll (sp + 0x2B0); the
//! target's point (0x1F65D0 kind 1) taken once at D+0x30. From update 0
//! for 0x131 updates four textured quads (table 0x28AD70, 12 x f) 52 up
//! and 95 x the side away from the target's point at alphas 80 / 255 f
//! sin x (0.8 + 0.2 (age & 1)) (f = clamp(age) clamp(0x130 - age) / 1024;
//! textures D+0x10 / D+0x14, ALPHA 0 / 1). On updates 0x59..0xEF a beam
//! record an update (64 round robin, +0x50, 0x50): a random table
//! direction from the point, its length and an aim matrix spun by rand;
//! each update for 64 updates a sprite (10, ALPHA 1, texture D+0x18)
//! along it at 40 (4 t - 4 t t) up, t = (age / 64)^2, leaving one of 8
//! trails (+0x1450 + 0x300 i, 0x60) of 4 scattered sprites (6.5 x size,
//! texture D+0x1C) and on age 0x3F a ring (32 round robin, +0xD790,
//! 0x60) with a random turn: per update for 32 updates a 16-segment
//! two-band mesh (texture D+0x20, 0x100BA0) of the 17 points D+0xD450
//! (10 / 35 round, their mean), scrolled in s by the age, scaled 0.1 +
//! 0.9 tan falloff, and a flat sprite (20, texture D+0x18) at its centre.
//! The stage light 0x80 - 80 clamp(0x170 - n) clamp(n) / 1024. Returns 1
//! on 0x8E, 6 on 0x99..0x14F, 2 on 0x150, 8 once the count reaches 0x171.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70 / 0x205210's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x460;
/// The quad / mesh descriptor (prim 0x34, 12 registers).
const QUAD: u32 = 0x370;
/// The sprite descriptor.
const DESC: u32 = 0x400;
/// The beam records (64 x 0x50, +0 active), the trails (64 x 8 x 0x60),
/// the mesh points (17 x 0x30), the ring counter and rings (32 x 0x60).
const BEAMS: u32 = D + 0x50;
const TRAILS: u32 = D + 0x1450;
const POINTS: u32 = D + 0xD450;
const RING_N: u32 = D + 0xD780;
const RINGS: u32 = D + 0xD790;
/// The quads' corner table (4 quads x 4 vec4) in the executable.
const QUAD_TABLE: u32 = 0x0028_AD70;
/// rand's saved s0 (the OT).
const S0: u32 = OT;

/// The registers 0x20C0B0 saves (f20..f24, s1, s3) as the record leaves
/// them [F; the runner's values before the first write are not known: 0,
/// H].
#[derive(Default)]
struct Regs {
    f: [f32; 5],
    s1: u64,
    s3: u64,
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xC0, 0);
    fx.mem.write_u32(sp + 0xD0, D);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp, param) };
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

fn words(fx: &mut Fx, to: u32, from: u32, n: u32) {
    for c in 0..n {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn vec4(fx: &mut Fx, p: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        fx.mem.setf(p + 4 * k as u32, *x);
    }
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

/// rand as rand % 256 << `sh` >> 8 (an angle).
fn rand_angle(fx: &mut Fx, ra: u32, sh: u32) -> i32 {
    let r = fx.rand_from(S0, ra) >> 23;
    (rem2(r, 256) << sh) >> 8
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0xE390);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..5u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x30);
    fx.mem.write_u8(D + 0x40, 0);
    for k in 0..0x40u32 {
        fx.mem.write_u8(BEAMS + 0x50 * k, 0);
        fx.mem.write_u32(BEAMS + 0x50 * k + 8, 0);
    }
    for i in 0..0x40u32 {
        for k in 0..8u32 {
            fx.mem.write_u8(TRAILS + 0x300 * i + 0x60 * k, 0);
        }
    }
    for k in 0..0x11u32 {
        let e = POINTS + 0x30 * k;
        let a = (k << 12) as i32;
        let s = fx.tsin(a);
        fx.mem.setf(e, ee::mul(10.0, s));
        let c = fx.tcos(a);
        fx.mem.setf(e + 4, ee::mul(10.0, c));
        fx.mem.write_u32(e + 8, 0);
        fx.mem.write_u32(e + 0xC, 0x3F80_0000);
        let s = fx.tsin(a);
        fx.mem.setf(e + 0x10, ee::mul(35.0, s));
        let c = fx.tcos(a);
        fx.mem.setf(e + 0x14, ee::mul(35.0, c));
        fx.mem.write_u32(e + 0x18, 0);
        fx.mem.write_u32(e + 0x1C, 0x3F80_0000);
        fx.vu_add4(e + 0x20, e, e + 0x10);
        fx.vu_scale4(e + 0x20, e + 0x20, 0.5);
    }
    fx.mem.write_u32(RING_N, 0);
    for k in 0..0x20u32 {
        fx.mem.write_u8(RINGS + 0x60 * k, 0);
    }
    fx.mem.write_u32(sp + 0xD0, D + 4);
    0
}

/// The view-projection (x identity) at sp + 0x230.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x230);
    fx.identity(sp + 0x270);
    crate::blib::exe::mul_mem(fx, sp + 0x230, sp + 0x270);
}

/// (0, 52, 95 x the side, 1) at `p`.
fn lift(fx: &mut Fx, p: u32) {
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    vec4(fx, p, [0.0, 52.0, ee::mul(95.0, side), 1.0]);
}

/// The quad / mesh descriptor (prim 0x34: RGBAQ / ST / XYZF2 x 4).
fn quad_desc(fx: &mut Fx, sp: u32) {
    let d = sp + QUAD;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
}

/// The sprite descriptor reset (prim 0x26 | 0x800, 5 registers).
fn sprite_desc(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// The quad STs (0, 0), (1, 0), (0, 1), (1, 1).
fn quad_st(fx: &mut Fx, sp: u32) {
    for (o, v) in [(0x388, 0), (0x38C, 0), (0x3A0, 0x3F80_0000), (0x3A4, 0), (0x3B8, 0), (0x3BC, 0x3F80_0000), (0x3D0, 0x3F80_0000), (0x3D4, 0x3F80_0000)] {
        fx.mem.write_u32(sp + o, v);
    }
}

/// The quad colour (c, a, Q 1) at sp + 0x380 copied to the other three.
fn quad_colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    let p = sp + 0x380;
    fx.mem.write_u8(p, c[0]);
    fx.mem.write_u8(p + 1, c[1]);
    fx.mem.write_u8(p + 2, c[2]);
    fx.mem.write_u8(p + 3, a);
    fx.mem.write_u32(p + 4, 0x3F80_0000);
    for o in [0x398, 0x3B0, 0x3C8] {
        words(fx, sp + o, p, 2);
    }
}

/// The sprite colour (c, a, Q 1) at sp + 0x410.
fn sprite_colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    let p = sp + 0x410;
    fx.mem.write_u8(p, c[0]);
    fx.mem.write_u8(p + 1, c[1]);
    fx.mem.write_u8(p + 2, c[2]);
    fx.mem.write_u8(p + 3, a);
    fx.mem.write_u32(p + 4, 0x3F80_0000);
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The sprite's UVs (sp + 0x418 / 0x41A, 0x428 / 0x42A).
fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x418, u0);
    uv(fx, sp + 0x41A, v0);
    uv(fx, sp + 0x428, u1);
    uv(fx, sp + 0x42A, v1);
}

/// The corners (-s, -s, 0, 1) at sp + 0x2F0 and (s, s, 0, 1) at sp +
/// 0x300, turned by the camera when `turn`.
fn corners(fx: &mut Fx, sp: u32, s: f32, turn: bool) {
    let n = ee::neg(s);
    vec4(fx, sp + 0x2F0, [n, n, 0.0, 1.0]);
    vec4(fx, sp + 0x300, [s, s, 0.0, 1.0]);
    if turn {
        fx.vu_apply_to(sp + 0x2F0, sp + 0x2B0, sp + 0x2F0);
        fx.vu_apply_to(sp + 0x300, sp + 0x2B0, sp + 0x300);
    }
}

/// The sprite at sp + 0x350 (+ the corners) projected (0x204F70) and
/// drawn at the mean w.
fn sprite_draw(fx: &mut Fx, sp: u32, ra: u32) {
    fx.vu_add4(sp + 0x330, sp + 0x2F0, sp + 0x350);
    fx.vu_add4(sp + 0x340, sp + 0x300, sp + 0x350);
    sprite_submit(fx, sp, sp + 0x330, ra);
}

fn sprite_submit(fx: &mut Fx, sp: u32, v: u32, ra: u32) {
    if crate::fxlib2::project(fx, v, PROJ_OUT, sp + 0x230, sp + DESC, sp + 0x45C) {
        let w = fx.mem.f(sp + 0x45C);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// The four quads [F 0x1E003B8].
fn quads(fx: &mut Fx, sp: u32, rg: &mut Regs) {
    let d = sp + QUAD;
    quad_desc(fx, sp);
    fx.desc_clamp(d, 5);
    fx.camera_vp_to(sp + 0x230);
    fx.identity(sp + 0x270);
    lift(fx, sp + 0x350);
    fx.translate_rows(sp + 0x270, sp + 0x270, sp + 0x350);
    crate::blib::exe::mul_mem(fx, sp + 0x230, sp + 0x270);
    let age = fx.mem.i32(D + 0x44);
    let a = ee::from_i32(Fx::clamp_i(age, 0, 0x20));
    let age = fx.mem.i32(D + 0x44);
    let b = ee::from_i32(Fx::clamp_i(0x130 - age, 0, 0x20));
    let f2 = ee::div(ee::mul(a, b), 1024.0);
    let f20 = ee::mul(12.0, f2);
    let f21 = ee::mul(80.0, f2);
    let f22 = ee::mul(ee::mul(255.0, f2), f2);
    rg.f[..3].copy_from_slice(&[f20, f21, f22]);
    for q in 0..4u32 {
        let t = QUAD_TABLE + 0x40 * q;
        for k in 0..4u32 {
            fx.vu_scale4(sp + 0x2F0 + 0x10 * k, t + 0x10 * k, f20);
        }
        let rh = fx.mem.read_u32(D + 0x10);
        fx.desc_texture(d, rh, 1);
        let a = fx.alpha_preset(0);
        fx.desc_alpha(d, a);
        quad_st(fx, sp);
        let age = fx.mem.i32(D + 0x44);
        let mut s6 = (rem2(age, 0x20) << 15) >> 5;
        let sn = fx.tsin(s6);
        let f3 = ee::mul(f21, sn);
        let age = fx.mem.i32(D + 0x44);
        let k = ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(rem2(age, 2))));
        let a = fix(fx, ee::mul(k, f3), 0x01E0_0604);
        quad_colour(fx, sp, [0x80; 3], a as u8);
        let m = fx.read_m4(sp + 0x230);
        if fx.project_desc(sp + 0x2F0, PROJ_OUT, Some(&m), d, sp + 0x45C) {
            let w = fx.mem.f(sp + 0x45C);
            let key = fix(fx, w, 0x01E0_06A0);
            fx.submit(d, OT, key as i32);
        }
        quad_st(fx, sp);
        s6 = (s6 + 0x4000) & 0xFFFF;
        let sn = fx.tsin(s6);
        let f0 = fabsf(fx, sn);
        let f3 = ee::mul(f22, f0);
        let age = fx.mem.i32(D + 0x44);
        let k = ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(rem2(age, 2))));
        let a = fix(fx, ee::mul(k, f3), 0x01E0_0764);
        quad_colour(fx, sp, [0xFF, 0xFF, 0x80], a as u8);
        let rh = fx.mem.read_u32(D + 0x14);
        fx.desc_texture(d, rh, 1);
        let a = fx.alpha_preset(1);
        fx.desc_alpha(d, a);
        let m = fx.read_m4(sp + 0x230);
        if fx.project_desc(sp + 0x2F0, PROJ_OUT, Some(&m), d, sp + 0x45C) {
            let w = fx.mem.f(sp + 0x45C);
            let key = fix(fx, w, 0x01E0_0814);
            fx.submit(d, OT, key as i32);
        }
    }
    rg.s1 = u64::from(QUAD_TABLE + 0x100);
    rg.s3 = u64::from(sp + 0x3B0);
    let age = fx.mem.i32(D + 0x44) + 1;
    fx.mem.set32(D + 0x44, age);
    if age >= 0x131 {
        fx.mem.write_u8(D + 0x40, 0);
    }
}

/// A new beam record [F 0x1E00884].
fn spawn(fx: &mut Fx, sp: u32, n: i32, rg: &mut Regs) {
    let r = BEAMS + 0x50 * rem2(n, 0x40) as u32;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    lift(fx, sp + 0x360);
    let v = sp + 0x350;
    fx.random_table_vec(v, 1.0);
    fx.vu_add4(v, v, D + 0x30);
    fx.vu_sub4(v, v, sp + 0x360);
    let x = Fx::powf2(fx.mem.f(v));
    let f20 = ee::add(x, Fx::powf2(fx.mem.f(v + 4)));
    let z = Fx::powf2(fx.mem.f(v + 8));
    let l = fx.sqrtf(ee::add(z, f20));
    fx.mem.setf(r + 0xC, l);
    fx.point_along(r + 0x10, v);
    let a = rand_angle(fx, 0x01E0_0980, 16);
    fx.rotate_axis(r + 0x10, a, 2);
    rg.f[0] = f20;
    rg.s1 = u64::from(r - 0x50 + 4);
    rg.s3 = 1;
}

/// The beams' sprites and their trails / rings [F 0x1E009C0].
fn beams(fx: &mut Fx, sp: u32, rg: &mut Regs) {
    let d = sp + DESC;
    sprite_desc(fx, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    corners(fx, sp, 5.0, true);
    lift(fx, sp + 0x360);
    for i in 0..0x40u32 {
        let r = BEAMS + 0x50 * i;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        sprite_colour(fx, sp, [0xFF, 0x28, 0x14], 0xFF);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let f21 = Fx::powf2(ee::div(age, 64.0));
        let f1 = ee::add(ee::mul(4.0, f21), ee::mul(ee::mul(-4.0, f21), f21));
        let old = fx.mem.i32(r + 8);
        fx.mem.set32(r + 8, old + 1);
        if old + 1 >= 8 {
            fx.mem.write_u32(r + 8, 0);
        }
        let f20 = ee::mul(40.0, f1);
        rg.f[0] = f20;
        rg.f[1] = f21;
        let v = sp + 0x350;
        let l = fx.mem.f(r + 0xC);
        vec4(fx, v, [0.0, f20, ee::mul(f21, l), 1.0]);
        fx.vu_apply_to(v, r + 0x10, v);
        fx.vu_add4(v, v, sp + 0x360);
        sprite_draw(fx, sp, 0x01E0_0D0C);
        let t = TRAILS + 0x300 * i + 0x60 * old as u32;
        fx.mem.write_u8(t, 1);
        fx.mem.write_u32(t + 4, 0);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let k = ee::add(f32::from_bits(0x3E99_999A), ee::div(ee::mul(f32::from_bits(0x3F33_3333), age), 64.0));
        fx.mem.setf(t + 0xC, k);
        words(fx, t + 0x10, v, 4);
        for j in 0..4u32 {
            let x = fx.rand_from(S0, 0x01E0_0DB8) >> 23;
            fx.mem.set8(t + 8 + j, rem2(x, 8));
            let s = ee::mul(f32::from_bits(0x3E4C_CCCD), fx.mem.f(t + 0xC));
            fx.random_table_vec(t + 0x20 + 0x10 * j, s);
        }
        rg.s1 = u64::from(t + 4);
        rg.s3 = 4;
        if fx.mem.i32(r + 4) == 0x3F {
            let c = fx.mem.read_u32(RING_N);
            let b = RINGS + 0x60 * c;
            fx.mem.write_u8(b, 1);
            fx.mem.write_u32(b + 4, 0);
            words(fx, b + 0x10, v, 4);
            let m = b + 0x20;
            rg.s1 = u64::from(m);
            fx.identity(m);
            let a = rand_angle(fx, 0x01E0_0E9C, 15);
            fx.rotate_axis(m, a, 0);
            let a = rand_angle(fx, 0x01E0_0ED0, 15);
            fx.rotate_axis(m, a, 1);
            let a = rand_angle(fx, 0x01E0_0F04, 16);
            fx.rotate_axis(m, a, 2);
            let c = fx.mem.i32(RING_N) + 1;
            fx.mem.set32(RING_N, c);
            if c >= 0x20 {
                fx.mem.write_u32(RING_N, 0);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x41 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The trails [F 0x1E010C8].
fn trails(fx: &mut Fx, sp: u32, rg: &mut Regs) {
    let d = sp + DESC;
    sprite_desc(fx, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    for i in 0..0x40u32 {
        for k in 0..8u32 {
            let t = TRAILS + 0x300 * i + 0x60 * k;
            if fx.mem.read_u8(t) == 0 {
                continue;
            }
            let age = fx.mem.i32(t + 4);
            let f2 = ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(age)), 8.0));
            let s = ee::mul(ee::mul(6.5, f2), fx.mem.f(t + 0xC));
            corners(fx, sp, s, true);
            let age = fx.mem.i32(t + 4);
            let a = ((8 - age) * 0xFF) >> 3;
            sprite_colour(fx, sp, [0xFF, 0x3C, 0x14], a as u8);
            let age = fx.mem.i32(t + 4);
            let tf = fx.tan_falloff_p(age, 8, 1);
            let f20 = ee::add(f32::from_bits(0x3ECC_CCCD), ee::mul(f32::from_bits(0x3F19_999A), tf));
            rg.f[0] = f20;
            for j in 0..4u32 {
                let b = u32::from(fx.mem.read_u8(t + 8 + j));
                let (c, r) = (b & 3, b >> 2);
                uvs(fx, sp, (c << 9) & 0x3FFF, (r << 9) & 0x3FFF, ((32 * c + 32) << 4) & 0x3FFF, ((32 * r + 32) << 4) & 0x3FFF);
                fx.vu_scale4(sp + 0x350, t + 0x20 + 0x10 * j, f20);
                fx.vu_add4(sp + 0x350, sp + 0x350, t + 0x10);
                sprite_draw(fx, sp, 0x01E0_1318);
            }
            rg.s1 = 4;
            rg.s3 = u64::from(t + 0x40);
            let age = fx.mem.i32(t + 4) + 1;
            fx.mem.set32(t + 4, age);
            if age >= 9 {
                fx.mem.write_u8(t, 0);
            }
        }
    }
}

/// The rings: the mesh and the centre sprite [F 0x1E013C8].
fn rings(fx: &mut Fx, sp: u32, rg: &mut Regs) {
    let q = sp + QUAD;
    quad_desc(fx, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(q, a);
    let rh = fx.mem.read_u32(D + 0x20);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 0);
    let d = sp + DESC;
    sprite_desc(fx, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    corners(fx, sp, 20.0, false);
    for n in 0..0x20u32 {
        let b = RINGS + 0x60 * n;
        if fx.mem.read_u8(b) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(b + 4));
        let f0 = Fx::ramp_above(age, 32.0, 16.0);
        let p = sp + 0x380;
        fx.mem.write_u8(p, 0x40);
        fx.mem.write_u8(p + 1, 0x10);
        fx.mem.write_u8(p + 2, 8);
        fx.mem.write_u8(p + 3, 0);
        fx.mem.write_u32(p + 4, 0x3F80_0000);
        words(fx, sp + 0x398, p, 2);
        let a = fix(fx, ee::mul(128.0, f0), 0x01E0_15E4);
        let p = sp + 0x3B0;
        fx.mem.write_u8(p, 0xFF);
        fx.mem.write_u8(p + 1, 0x3C);
        fx.mem.write_u8(p + 2, 0x14);
        fx.mem.write_u8(p + 3, a as u8);
        fx.mem.write_u32(p + 4, 0x3F80_0000);
        words(fx, sp + 0x3C8, p, 2);
        fx.camera_vp_to(sp + 0x230);
        let s2 = u64::from(b - 0xD790);
        fx.trs_into_saving(sp + 0x270, 0, 0, b + 0x10, rg.f, [u64::from(OT), rg.s1, s2, rg.s3], 0x01E0_1640);
        crate::blib::exe::mul_mem(fx, sp + 0x270, b + 0x20);
        let age = fx.mem.i32(b + 4);
        let tf = fx.tan_falloff_p(age, 0x20, 1);
        let s = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), tf));
        vec4(fx, sp + 0x350, [s, s, 0.0, 1.0]);
        fx.scale_rows(sp + 0x270, sp + 0x350);
        crate::blib::exe::mul_mem(fx, sp + 0x230, sp + 0x270);
        let age = fx.mem.i32(b + 4);
        let f20 = ee::div(ee::mul(-1.0, ee::from_i32(rem2(age, 0x20))), 32.0);
        let f21 = ee::add(0.5, f20);
        let f22 = ee::add(1.0, f20);
        for j in 0..0x10u32 {
            let e0 = POINTS + 0x30 * j;
            let e1 = e0 + 0x30;
            let f23 = ee::div(ee::mul(6.0, ee::from_i32(j as i32)), 16.0);
            let f24 = ee::div(ee::mul(6.0, ee::from_i32(j as i32 + 1)), 16.0);
            for (o, v) in [(0x388, f23), (0x38C, f20), (0x3A0, f24), (0x3A4, f20), (0x3B8, f23), (0x3BC, f21), (0x3D0, f24), (0x3D4, f21)] {
                fx.mem.setf(sp + o, v);
            }
            if fx.project_n(&[e0, e1, e0 + 0x20, e1 + 0x20], sp + 0x230, q, sp + 0x45C, 0x01E0_17E8) == 1 {
                let w = fx.mem.f(sp + 0x45C);
                let key = fix(fx, w, 0x01E0_17FC);
                fx.submit(q, OT, key as i32);
            }
            for (o, v) in [(0x388, f23), (0x38C, f22), (0x3A0, f24), (0x3A4, f22), (0x3B8, f23), (0x3BC, f21), (0x3D0, f24), (0x3D4, f21)] {
                fx.mem.setf(sp + o, v);
            }
            if fx.project_n(&[e0 + 0x10, e1 + 0x10, e0 + 0x20, e1 + 0x20], sp + 0x230, q, sp + 0x45C, 0x01E0_1870) == 1 {
                let w = fx.mem.f(sp + 0x45C);
                let key = fix(fx, w, 0x01E0_1884);
                fx.submit(q, OT, key as i32);
            }
            rg.f = [f20, f21, f22, f23, f24];
        }
        rg.s1 = u64::from(D + 0x300);
        rg.s3 = u64::from(D + 0x300);
        let age = ee::from_i32(fx.mem.i32(b + 4));
        let f0 = Fx::ramp_above(age, 32.0, 16.0);
        let a = fix(fx, ee::mul(255.0, f0), 0x01E0_18DC);
        sprite_colour(fx, sp, [0xFF, 0x56, 0x28], a as u8);
        fx.camera_vp_to(sp + 0x230);
        fx.translate_rows(sp + 0x270, sp + 0x2B0, b + 0x10);
        let age = fx.mem.i32(b + 4);
        let tf = fx.tan_falloff_p(age, 0x20, 1);
        let s = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), tf));
        vec4(fx, sp + 0x350, [s, s, s, 1.0]);
        fx.scale_rows(sp + 0x270, sp + 0x350);
        crate::blib::exe::mul_mem(fx, sp + 0x230, sp + 0x270);
        sprite_submit(fx, sp, sp + 0x2F0, 0x01E0_19BC);
        let age = fx.mem.i32(b + 4) + 1;
        fx.mem.set32(b + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(b, 0);
        }
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let mut rg = Regs { s1: param as i64 as u64, ..Regs::default() };
    fx.camera_turn_roll(sp + 0x2B0);
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + 0x40, 1);
        fx.mem.write_u32(D + 0x44, 0);
    }
    if fx.mem.read_u8(D + 0x40) == 1 {
        quads(fx, sp, &mut rg);
    }
    let n = fx.mem.i32(D);
    if (0x59..0xF0).contains(&n) {
        spawn(fx, sp, n, &mut rg);
    }
    beams(fx, sp, &mut rg);
    trails(fx, sp, &mut rg);
    rings(fx, sp, &mut rg);
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(0x170 - n, 0, 0x20);
    let n = fx.mem.i32(D);
    let b = Fx::clamp_i(n, 0, 0x20);
    let m = fx.mult(a, (b.wrapping_neg() << 2).wrapping_sub(b) << 4);
    fx.set_stage_light(((m >> 10) + 0x80) & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x8E {
        ret = 1;
    } else {
        if (0x99..0x150).contains(&n) {
            ret = 6;
        }
        if n == 0x150 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x171 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}
