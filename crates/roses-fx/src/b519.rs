//! BMODULE b519 (card 592), hand-ported from the record (linked at
//! 0x1E00000; one 2,464-word function 0x1E00080 in a 0x410 frame, s1 = D,
//! s0 = the OT, the return at sp + 0xB0, data area a0 = 0x1ED0000, 0xBD30
//! bytes) [F]. Written cold from the raw listing
//! (the project notes). Slots: the camera sp + 0x1C0 (model + 0x40,
//! turn + 0x80), corners sp + 0x280 / 0x290, vertices sp + 0x2A0 / 0x2B0,
//! the aim matrix sp + 0x2C0, the point sp + 0x300, the sprite descriptor
//! sp + 0x310, the triangle sp + 0x368, the quad sp + 0x3B0, the mean w sp
//! + 0x40C. Init: nodes 0x3A (clear) / 0x3F / 0x3C, the target's position
//! D+0x70, a tube of 33 x (11.5, 11.8, 12) points D+0x8690, a ring of 33
//! points radius 12 D+0xB960. Every update the node points D+0x20 / 0x30,
//! their mean (1 up, 2 side back) D+0x40, the third D+0x60, the aim from
//! the mean to the target. On 0x47..0xDE a stream (48 x 0x260 at D+0x80:
//! 24 sprites from the mean to the target), on 0x47..0xBE a puff (16 x
//! 0x140 at D+0x7280), on 0xD..0x1D a gather (32 x 0x120 at D+0x9550), on
//! 0xC a burst (D+0x8680: glow, tube and 128 sparks at the third node),
//! from 8 a halo (D+0xB950, 0xD8 updates), on 0x68..0xDE a spiral at the
//! target (16 x 0x1C at D+0xBB70). The stage dims. Returns 1 on 0x5D, 6 on
//! 0x68..0xDE, 2 on 0xDF, 8 once the count reaches 0x110.
use crate::b182::{alpha, head, rgba};
use crate::b440::{copy8, copy_z, fix, rem2, words};
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2::project;

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
const ONE: u32 = 0x3F80_0000;

const VP: u32 = 0x1C0;
const MODEL: u32 = 0x200;
const TURN: u32 = 0x240;
const C0: u32 = 0x280;
const C1: u32 = 0x290;
const V0: u32 = 0x2A0;
const V1: u32 = 0x2B0;
const AIM: u32 = 0x2C0;
const PL: u32 = 0x300;
const S: u32 = 0x310;
const TRI: u32 = 0x368;
const QD: u32 = 0x3B0;
const MEAN: u32 = 0x40C;

const STREAMS: u32 = D + 0x80;
const PUFFS: u32 = D + 0x7280;
const BURST: u32 = D + 0x8680;
const TUBE: u32 = D + 0x8690;
const BURST_CELLS: u32 = D + 0x8CC0;
const BURST_AT: u32 = D + 0x8D40;
const BURST_VECS: u32 = D + 0x8D50;
const GATHERS: u32 = D + 0x9550;
const HALO: u32 = D + 0xB950;
const CIRCLE: u32 = D + 0xB960;
const SPIRALS: u32 = D + 0xBB70;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x410);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp) };
    fx.leave(0x410);
    ret
}

/// rand >> 23 from `ra` (s0 = the OT).
fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(OT, ra) >> 23
}

fn sin16(fx: &mut Fx, a: i32) -> f32 {
    fx.table_trig(a, false)
}

fn cos16(fx: &mut Fx, a: i32) -> f32 {
    fx.table_trig(a, true)
}

/// (x, y, z, 1) at `p`.
fn vec3(fx: &mut Fx, p: u32, x: f32, y: f32, z: f32) {
    fx.mem.setf(p, x);
    fx.mem.setf(p + 4, y);
    fx.mem.setf(p + 8, z);
    fx.mem.write_u32(p + 0xC, ONE);
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    // The flags the record clears after this are all inside it.
    fx.bzero(D, 0xBD30);
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
    for (at, node, clear) in [(0x18u32, 0x3A, 1), (0x1C, 0x3F, 0), (0x50, 0x3C, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x70);
    let pl = sp + PL;
    for k in 0..0x21u32 {
        let a = ((k << 16) as i32) >> 5;
        let s = sin16(fx, a);
        let c = cos16(fx, a);
        vec3(fx, pl, s, c, 0.0);
        let p = TUBE + 0x30 * k;
        fx.vu_scale4(p, pl, 11.5);
        fx.vu_scale4(p + 0x10, pl, f32::from_bits(0x413C_CCCD));
        fx.vu_scale4(p + 0x20, pl, 12.0);
    }
    for k in 0..0x21u32 {
        let a = ((k << 16) as i32) >> 5;
        let s = ee::mul(12.0, sin16(fx, a));
        let c = ee::mul(12.0, cos16(fx, a));
        vec3(fx, CIRCLE + 0x10 * k, s, c, 0.0);
    }
    0
}

/// The sprite descriptor: ALPHA preset 1, texture D + `tex`, clamp 5.
fn sprite_desc(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + S;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 1);
    let t = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, t, 1);
    fx.desc_clamp(d, 5);
}

/// The same with the texture before the ALPHA (the burst's and halo's).
fn sprite_desc_t(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + S;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let t = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, t, 1);
    alpha(fx, d, 1);
    fx.desc_clamp(d, 5);
}

/// The camera's view-projection x the identity.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MODEL);
    mul_mem(fx, sp + VP, sp + MODEL);
}

/// The camera's view-projection x the turn moved to `at`.
fn camera_at(fx: &mut Fx, sp: u32, at: u32) {
    fx.camera_vp_to(sp + VP);
    fx.translate_rows(sp + MODEL, sp + TURN, at);
}

/// The sprite's UV rectangle.
fn uvs(fx: &mut Fx, sp: u32, u0: i32, v0: i32, u1: i32, v1: i32) {
    for (at, v) in [(0x328u32, u0), (0x32A, v0), (0x338, u1), (0x33A, v1)] {
        let h = fx.mem.read_u16(sp + at);
        fx.mem.write_u16(sp + at, (h & 0xC000) | (v & 0x3FFF) as u16);
    }
}

/// The 32 x 32 cell `b` of a 4-wide sheet.
fn cell(fx: &mut Fx, sp: u32, b: i32) {
    let u = rem2(b, 4) << 5;
    let v = (b >> 2) << 5;
    uvs(fx, sp, u << 4, v << 4, (u + 0x20) << 4, (v + 0x20) << 4);
}

/// Corners (-s, -s) / (s, s), turned to the camera when `turned`.
fn corners(fx: &mut Fx, sp: u32, s: f32, turned: bool) {
    let n = ee::neg(s);
    vec3(fx, sp + C0, n, n, 0.0);
    vec3(fx, sp + C1, s, s, 0.0);
    if turned {
        fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
        fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
    }
}

/// The two vertices at the point.
fn vertices(fx: &mut Fx, sp: u32) {
    fx.vu_add4(sp + V0, sp + C0, sp + PL);
    fx.vu_add4(sp + V1, sp + C1, sp + PL);
}

/// The vertices projected; the sprite submitted at their mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    if project(fx, sp + V0, PROJ_OUT, sp + VP, sp + S, sp + MEAN) {
        let w = fx.mem.f(sp + MEAN);
        let key = fix(fx, w, ra);
        fx.submit(sp + S, OT, key as i32);
    }
}

/// The far Z on the XYZ2 at `first`, copied to `rest`.
fn far(fx: &mut Fx, first: u32, rest: &[u32]) {
    let x = fx.mem.read_u64(first);
    fx.mem.write_u64(first, (x & KEEP) | FAR_Z);
    for &r in rest {
        copy_z(fx, r, first);
    }
}

/// The sprite's two XYZ2 at far Z, submitted at key 0.
fn draw_far(fx: &mut Fx, sp: u32) {
    far(fx, sp + 0x330, &[sp + 0x340]);
    fx.submit(sp + S, OT, 0);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    let pl = sp + PL;
    for k in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x18 + 4 * k);
        fx.node_position(h, s, D + 0x20 + 0x10 * k);
    }
    fx.vu_add4(D + 0x40, D + 0x20, D + 0x30);
    fx.vu_scale4(D + 0x40, D + 0x40, 0.5);
    let y = ee::add(fx.mem.f(D + 0x44), 1.0);
    fx.mem.setf(D + 0x44, y);
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    let z = ee::add(fx.mem.f(D + 0x48), ee::mul(-2.0, side));
    fx.mem.setf(D + 0x48, z);
    let h = fx.mem.i32(D + 4);
    let s = fx.mem.i32(D + 0x50);
    fx.node_position(h, s, D + 0x60);
    fx.vu_sub4(pl, D + 0x70, D + 0x40);
    fx.point_along(sp + AIM, pl);
    let c = fx.mem.i32(D);
    if (0x47..0xDF).contains(&c) {
        spawn_stream(fx, c);
    }
    sprite_desc(fx, sp, 0x10);
    camera(fx, sp);
    let n = Fx::clamp_i(0xDF - fx.mem.i32(D), 0, 0x20);
    let f20 = ee::div(ee::from_i32(n), 32.0);
    for k in 0..0x30u32 {
        let r = STREAMS + 0x260 * k;
        if fx.mem.read_u8(r) != 0 {
            stream(fx, sp, r, f20);
        }
    }
    let c = fx.mem.i32(D);
    if (0x47..0xBF).contains(&c) {
        let r = PUFFS + 0x140 * rem2(c, 0x10) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x30, D + 0x40);
        for j in 0..0x10u32 {
            let v = rnd(fx, 0x01E0_097C);
            fx.mem.write_u16(r + 8 + 2 * j, rem2(v, 4) as u16);
            fx.random_table_vec(r + 0x40 + 0x10 * j, f32::from_bits(0x3F33_3333));
        }
    }
    sprite_desc(fx, sp, 0x14);
    camera(fx, sp);
    for k in 0..0x10u32 {
        let r = PUFFS + 0x140 * k;
        if fx.mem.read_u8(r) != 0 {
            puff(fx, sp, r);
        }
    }
    if fx.mem.i32(D) == 0xC {
        fx.mem.write_u8(BURST, 1);
        fx.mem.write_u32(BURST + 4, 0);
        words(fx, BURST_AT, D + 0x60);
        for j in 0..0x80u32 {
            let v = rnd(fx, 0x01E0_0D64);
            fx.mem.set8(BURST_CELLS + j, rem2(v, 4));
            fx.random_table_vec(BURST_VECS + 0x10 * j, f32::from_bits(0x3F4C_CCCD));
        }
    }
    if fx.mem.read_u8(BURST) == 1 {
        burst(fx, sp);
    }
    let c = fx.mem.i32(D);
    if (0xD..0x1E).contains(&c) {
        let r = GATHERS + 0x120 * rem2(c, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for j in 0..0x10u32 {
            let v = rnd(fx, 0x01E0_179C);
            fx.mem.set8(r + 8 + j, rem2(v, 4));
            let p = r + 0x20 + 0x10 * j;
            fx.random_table_vec(p, 1.0);
            fx.vu_add4(p, p, D + 0x40);
        }
    }
    sprite_desc(fx, sp, 0x14);
    camera(fx, sp);
    for k in 0..0x20u32 {
        let r = GATHERS + 0x120 * k;
        if fx.mem.read_u8(r) != 0 {
            gather(fx, sp, r);
        }
    }
    if fx.mem.i32(D) == 8 {
        fx.mem.write_u8(HALO, 1);
        fx.mem.write_u32(HALO + 4, 0);
    }
    if fx.mem.read_u8(HALO) == 1 {
        halo(fx, sp);
    }
    let c = fx.mem.i32(D);
    if (0x68..0xDF).contains(&c) {
        let r = SPIRALS + 0x1C * rem2(c, 0x10) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        let v = rnd(fx, 0x01E0_2118);
        fx.mem.set32(r + 0x18, (rem2(v, 256) << 16) >> 8);
        for j in 0..0x10u32 {
            let v = rnd(fx, 0x01E0_2154);
            fx.mem.set8(r + 8 + j, rem2(v, 8));
        }
    }
    sprite_desc(fx, sp, 0x10);
    camera(fx, sp);
    let n = Fx::clamp_i(0xDF - fx.mem.i32(D), 0, 0x20);
    let f20 = ee::div(ee::from_i32(n), 32.0);
    for k in 0..0x10u32 {
        let r = SPIRALS + 0x1C * k;
        if fx.mem.read_u8(r) != 0 {
            spiral(fx, sp, r, f20);
        }
    }
    let c = fx.mem.i32(D);
    let a = Fx::clamp_i(0x10F - c, 0, 0x20);
    let b = Fx::clamp_i(c, 0, 0x20);
    let l = (a.wrapping_mul(((-b << 2) - b) << 4) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);
    let c = fx.mem.i32(D);
    if c == 0x5D {
        fx.mem.write_u32(sp + 0xB0, 1);
    } else {
        if (0x68..0xDF).contains(&c) {
            fx.mem.write_u32(sp + 0xB0, 6);
        }
        if c == 0xDF {
            fx.mem.write_u32(sp + 0xB0, 2);
        }
    }
    let n = fx.mem.i32(D) + 1;
    fx.mem.set32(D, n);
    if fx.mem.i32(D) >= 0x110 {
        fx.mem.write_u32(sp + 0xB0, 8);
    }
    fx.mem.i32(sp + 0xB0)
}

/// [F 0x1E00404] Stream c % 48 at the mean (8 side back): 24 cells, sizes
/// 0.2 + 0.8 (rand % 256) / 256 and vectors (0.7 random + the target - the
/// start).
fn spawn_stream(fx: &mut Fx, c: i32) {
    let (_, m) = fx.div(c, 0x30);
    let r = STREAMS + 0x260 * m as u32;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    words(fx, r + 0xD0, D + 0x40);
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    let z = ee::add(fx.mem.f(r + 0xD8), ee::mul(8.0, side));
    fx.mem.setf(r + 0xD8, z);
    for j in 0..0x18u32 {
        let v = rnd(fx, 0x01E0_048C);
        fx.mem.set32(r + 8 + 4 * j, rem2(v, 8));
        let v = rnd(fx, 0x01E0_04AC);
        let f2 = ee::from_i32(rem2(v, 256));
        let f = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), f2), 256.0));
        fx.mem.setf(r + 0x68 + 4 * j, f);
        let p = r + 0xE0 + 0x10 * j;
        fx.random_table_vec(p, f32::from_bits(0x3F33_3333));
        fx.vu_add4(p, p, D + 0x70);
        fx.vu_sub4(p, p, r + 0xD0);
    }
}

/// [F 0x1E00630] A stream's 24 sprites, (FF, 54, 38), 12 (0.5 + age / 96)
/// wide x their sizes, along their vectors by 1.5 age / 48; 0x31 updates.
fn stream(fx: &mut Fx, sp: u32, r: u32, f20: f32) {
    let pl = sp + PL;
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let f21 = ee::mul(128.0, Fx::ratio_below(age, 4.0));
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let f0 = Fx::ramp_above(age, 48.0, 32.0);
    let a = fix(fx, ee::mul(f20, ee::mul(f21, f0)), 0x01E0_0680);
    rgba(fx, sp + S + 0x10, [0xFF, 0x54, 0x38], a as u8);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let f1 = ee::mul(0.5, age);
    let s = ee::mul(12.0, ee::add(0.5, ee::div(f1, 48.0)));
    corners(fx, sp, s, true);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let f21 = ee::div(ee::mul(1.5, age), 48.0);
    for j in 0..0x18u32 {
        let b = fx.mem.i32(r + 8 + 4 * j);
        cell(fx, sp, b);
        let g = fx.mem.f(r + 0x68 + 4 * j);
        fx.vu_scale4(sp + V0, sp + C0, g);
        let g = fx.mem.f(r + 0x68 + 4 * j);
        fx.vu_scale4(sp + V1, sp + C1, g);
        fx.vu_scale4(pl, r + 0xE0 + 0x10 * j, f21);
        fx.vu_add4(pl, pl, r + 0xD0);
        fx.vu_add4(sp + V0, sp + V0, pl);
        fx.vu_add4(sp + V1, sp + V1, pl);
        draw(fx, sp, 0x01E0_0888);
    }
    let n = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, n);
    if n >= 0x31 {
        fx.mem.write_u8(r, 0);
    }
}

/// [F 0x1E00A68] A puff: 16 sprites (FF, 68, 4C) at 255 sin, 4 (0.2 + 0.8
/// (16 - age) / 16) wide, out by the falloff; 0x11 updates.
fn puff(fx: &mut Fx, sp: u32, r: u32) {
    let pl = sp + PL;
    let age = fx.mem.i32(r + 4);
    let f2 = ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(0x10 - age));
    let s = ee::mul(4.0, ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(f2, 16.0)));
    corners(fx, sp, s, true);
    let age = fx.mem.i32(r + 4);
    let f0 = sin16(fx, (age << 15) >> 4);
    let a = fix(fx, ee::mul(255.0, f0), 0x01E0_0B30);
    rgba(fx, sp + S + 0x10, [0xFF, 0x68, 0x4C], a as u8);
    let age = fx.mem.i32(r + 4);
    let f20 = fx.tan_falloff_p(age, 0x10, 1);
    for j in 0..0x10u32 {
        fx.vu_scale4(pl, r + 0x40 + 0x10 * j, f20);
        fx.vu_add4(pl, pl, r + 0x30);
        vertices(fx, sp);
        let b = i32::from(fx.mem.read_u16(r + 8 + 2 * j));
        cell(fx, sp, b);
        draw(fx, sp, 0x01E0_0C88);
    }
    let n = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, n);
    if n >= 0x11 {
        fx.mem.write_u8(r, 0);
    }
}

/// [F 0x1E00DD0] The burst at the third node: a glow, the tube in two
/// bands scaled 0.5 + 0.5 falloff^2, 128 sparks; 0x21 updates.
fn burst(fx: &mut Fx, sp: u32) {
    let (vp, m, q, pl) = (sp + VP, sp + MODEL, sp + QD, sp + PL);
    sprite_desc_t(fx, sp, 0x14);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    camera_at(fx, sp, BURST_AT);
    mul_mem(fx, vp, m);
    let age = fx.mem.i32(BURST + 4);
    let f0 = fx.tan_falloff_p(age, 0x20, 1);
    let a = fix(fx, ee::mul(255.0, ee::sub(1.0, f0)), 0x01E0_0EF0);
    rgba(fx, sp + S + 0x10, [0x37, 0x50, 0xFF], a as u8);
    corners(fx, sp, 8.0, false);
    if project(fx, sp + C0, PROJ_OUT, vp, sp + S, sp + MEAN) {
        draw_far(fx, sp);
    }
    head(fx, q, 0x14, 8, 0x4141_4141, 0);
    alpha(fx, q, 1);
    camera_at(fx, sp, BURST_AT);
    let age = fx.mem.i32(BURST + 4);
    let f0 = fx.tan_falloff_p(age, 0x20, 2);
    let f = ee::add(0.5, ee::mul(0.5, f0));
    vec3(fx, pl, f, f, 0.0);
    fx.scale_rows(m, pl);
    mul_mem(fx, vp, m);
    let age = fx.mem.i32(BURST + 4);
    let a = ((0x20 - age) * 255) >> 5;
    rgba(fx, q + 0x10, [0x3C, 0x50, 0xFF], a as u8);
    copy8(fx, q + 0x20, q + 0x10);
    rgba(fx, q + 0x30, [0x20, 0x24, 0x80], 0);
    copy8(fx, q + 0x40, q + 0x30);
    for k in 0..0x20u32 {
        let (a, b) = (TUBE + 0x30 * k, TUBE + 0x30 * (k + 1));
        for (i, ra) in [(0u32, 0x01E0_11ACu32), (0x20, 0x01E0_12C8)] {
            if fx.project_n(&[a + 0x10, b + 0x10, a + i, b + i], vp, q, sp + MEAN, ra) == 1 {
                far(fx, q + 0x18, &[q + 0x28, q + 0x38, q + 0x48]);
                fx.submit(q, OT, 0);
            }
        }
    }
    sprite_desc_t(fx, sp, 0x14);
    camera(fx, sp);
    let age = fx.mem.i32(BURST + 4);
    let a = ((0x20 - age) * 255) >> 5;
    rgba(fx, sp + S + 0x10, [0x20, 0xC8, 0xFF], a as u8);
    corners(fx, sp, f32::from_bits(0x3F4C_CCCD), true);
    let age = fx.mem.i32(BURST + 4);
    let f0 = fx.tan_falloff_p(age, 0x20, 2);
    let f20 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), f0));
    for j in 0..0x80u32 {
        let b = i32::from(fx.mem.read_u8(BURST_CELLS + j));
        cell(fx, sp, b);
        fx.vu_scale4(pl, BURST_VECS + 0x10 * j, f20);
        fx.vu_add4(pl, pl, BURST_AT);
        vertices(fx, sp);
        if project(fx, sp + V0, PROJ_OUT, vp, sp + S, sp + MEAN) {
            draw_far(fx, sp);
        }
    }
    let n = fx.mem.i32(BURST + 4) + 1;
    fx.mem.set32(BURST + 4, n);
    if n >= 0x21 {
        fx.mem.write_u8(BURST, 0);
    }
}

/// [F 0x1E018B0] A gather: 16 sprites (FF, 38, 26) drawn in to the mean by
/// the falloff of 32 - age, 2 (0.2 + 0.8 age / 32) wide; 0x21 updates.
fn gather(fx: &mut Fx, sp: u32, r: u32) {
    let pl = sp + PL;
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let f2 = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), age), 32.0);
    let s = ee::mul(2.0, ee::add(f32::from_bits(0x3E4C_CCCD), f2));
    corners(fx, sp, s, true);
    let age = fx.mem.i32(r + 4);
    let f0 = sin16(fx, (age << 14) >> 5);
    let a = fix(fx, ee::mul(255.0, f0), 0x01E0_197C);
    rgba(fx, sp + S + 0x10, [0xFF, 0x38, 0x26], a as u8);
    let age = fx.mem.i32(r + 4);
    let f20 = fx.tan_falloff_p(0x20 - age, 0x20, 1);
    for j in 0..0x10u32 {
        fx.vu_sub4(pl, r + 0x20 + 0x10 * j, D + 0x40);
        fx.vu_scale4(pl, pl, f20);
        fx.vu_add4(pl, pl, D + 0x40);
        vertices(fx, sp);
        let b = i32::from(fx.mem.read_u8(r + 8 + j));
        cell(fx, sp, b);
        draw(fx, sp, 0x01E0_1AEC);
    }
    let n = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, n);
    if n >= 0x21 {
        fx.mem.write_u8(r, 0);
    }
}

/// [F 0x1E01BCC] The halo at the mean: a sprite (FF, A8, 94) and a 32-slice
/// disc, far Z below 0x9C, keyed by w after; 0xD8 updates.
fn halo(fx: &mut Fx, sp: u32) {
    let (vp, m, t, pl) = (sp + VP, sp + MODEL, sp + TRI, sp + PL);
    sprite_desc_t(fx, sp, 0x14);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    camera_at(fx, sp, D + 0x40);
    mul_mem(fx, vp, m);
    let age = fx.mem.i32(HALO + 4);
    let c1 = ee::from_i32(Fx::clamp_i(0xD7 - age, 0, 0x40));
    let c2 = ee::from_i32(Fx::clamp_i(age, 0, 0x40));
    let f3 = ee::div(ee::mul(c1, c2), 4096.0);
    let v = ee::from_i32(rem2(age, 2));
    let f20 = ee::mul(ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), v)), f3);
    let a = fix(fx, ee::mul(255.0, f20), 0x01E0_1D7C);
    rgba(fx, sp + S + 0x10, [0xFF, 0xA8, 0x94], a as u8);
    corners(fx, sp, 12.0, false);
    if project(fx, sp + C0, PROJ_OUT, vp, sp + S, sp + MEAN) {
        if fx.mem.i32(D) < 0x9C {
            draw_far(fx, sp);
        } else {
            let w = fx.mem.f(sp + MEAN);
            let key = fix(fx, w, 0x01E0_1E90);
            fx.submit(sp + S, OT, key as i32);
        }
    }
    head(fx, t, 0x13, 6, 0x41_4141, 0);
    alpha(fx, t, 1);
    let a = fix(fx, ee::mul(128.0, f20), 0x01E0_1EE8);
    rgba(fx, t + 0x10, [0xFF, 0xA, 0xA], a as u8);
    rgba(fx, t + 0x20, [0xFF, 0, 0], 0);
    copy8(fx, t + 0x30, t + 0x20);
    vec3(fx, pl, 0.0, 0.0, 0.0);
    for k in 0..0x20u32 {
        let (a, b) = (CIRCLE + 0x10 * k, CIRCLE + 0x10 * (k + 1));
        if fx.project_n(&[pl, a, b], vp, t, sp + MEAN, 0x01E0_1F80) != 1 {
            continue;
        }
        if fx.mem.i32(D) < 0x9C {
            far(fx, t + 0x18, &[t + 0x28, t + 0x38]);
            fx.submit(t, OT, 0);
        } else {
            let w = fx.mem.f(sp + MEAN);
            let key = fix(fx, w, 0x01E0_2058);
            fx.submit(t, OT, key as i32);
        }
    }
    let n = fx.mem.i32(HALO + 4) + 1;
    fx.mem.set32(HALO + 4, n);
    if n >= 0xD8 {
        fx.mem.write_u8(HALO, 0);
    }
}

/// [F 0x1E02268] A spiral: 16 sprites (FF, 5E, 36) on a ring of radius 60
/// (0.1 + 0.9 falloff) at 80 age / 16 along the aim, at the target; 0x11
/// updates.
fn spiral(fx: &mut Fx, sp: u32, r: u32, f20: f32) {
    let pl = sp + PL;
    let age = fx.mem.i32(r + 4);
    let f1 = ee::mul(0.5, ee::from_i32(0x10 - age));
    let s = ee::mul(23.0, ee::add(0.5, ee::div(f1, 16.0)));
    corners(fx, sp, s, true);
    let age = fx.mem.i32(r + 4);
    let f0 = cos16(fx, (age << 14) >> 4);
    let a = fix(fx, ee::mul(f20, ee::mul(200.0, f0)), 0x01E0_2334);
    rgba(fx, sp + S + 0x10, [0xFF, 0x5E, 0x36], a as u8);
    let age = fx.mem.i32(r + 4);
    let f0 = fx.tan_falloff_p(age, 0x10, 1);
    let f3 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), f0));
    let f2 = ee::div(ee::from_i32(fx.mem.i32(r + 4)), 16.0);
    let f21 = ee::mul(60.0, f3);
    let f22 = ee::mul(80.0, f2);
    for j in 0..0x10u32 {
        let a = fx.mem.i32(r + 0x18).wrapping_add(((j << 16) as i32) >> 4) & 0xFFFF;
        let x = ee::mul(f21, sin16(fx, a));
        let y = ee::mul(f21, cos16(fx, a));
        vec3(fx, pl, x, y, f22);
        fx.vu_apply_to(pl, sp + AIM, pl);
        fx.vu_add4(pl, pl, D + 0x70);
        vertices(fx, sp);
        let b = i32::from(fx.mem.read_u8(r + 8 + j));
        cell(fx, sp, b);
        draw(fx, sp, 0x01E0_2534);
    }
    let n = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, n);
    if n >= 0x11 {
        fx.mem.write_u8(r, 0);
    }
}
