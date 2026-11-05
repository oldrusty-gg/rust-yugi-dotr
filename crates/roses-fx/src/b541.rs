//! BMODULE b541 (card 469), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080..0x1E02618, data area a0 = 0x1ED0000,
//! 0x7B80 bytes, D kept in s0, OT in s1) [F] (helper C34, 4 Nov 2026;
//! written fresh on b575.rs's helpers). A 0x3C0 frame: the camera turn at
//! sp + 0x2D0, the view-projection at sp + 0x250 (identity at sp + 0x290),
//! the sprite descriptor at sp + 0x360, the mean w at sp + 0x3BC.
//!
//! Init: textures (5, 0 / 1) D+0x10 / 0x14, nodes 9 (clear) / 7 at D+0x50 /
//! D+0xC60, four UV cells D+0x30, 64 (cell, brightness) pairs D+0xCB8 and
//! 32 x 24 at D+0x4190 + 0x1D0 i; the target's point D+0x20; 32 table
//! vectors of 2 D+0x1AE0, 24 of 3 D+0x3FF0 (x / z by the side).
//!
//! Step, each group drawn every update on its own sprite setup: on
//! 0x1F..0x4F a puff record (32 x 0x60 at D+0x60: 4 vectors at +-20, -10,
//! -30 side, 32 updates (0x30, 0x60, 0xA0)); on 0x74 a burst at node 7
//! (D+0xC70: 64 sparks in seven colours, 64 updates, tan falloff); on
//! 0x74..0xB8 a coloured pair (32 x 0x40 at D+0x1150, ALPHA 0, 32 updates)
//! and a four-spark flash (4 x 0x60 at D+0x1950, 4 updates); on
//! 0xBA..0xC8 a ring closing on the target (16 x 0x230 at D+0x1CE0, 32
//! points of D+0x1AE0 x the falloff of D+0x1AD0, ALPHA 0, 16 updates);
//! on 0xBA..0xD0 a shell (32 x 0x1D0 at D+0x4170, 24 points x D+0x3FE0 /
//! 24, 32 updates); on 0xB9 a full-screen frame flash (D+0x7B70: the frame
//! as a far-Z sprite, 48 updates). Returns 1 on 0xAF, 2 on 0xB9, 8 once
//! the count reaches 0xEA.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::far_xy;

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
/// 0x101080's direction table (4 x 32 vec3).
const DIRS: u32 = 0x0028_AE90;
const FRAME: u32 = 0x3C0;

const VP: u32 = 0x250;
const MS: u32 = 0x290;
const TURN: u32 = 0x2D0;
const C0: u32 = 0x310;
const C1: u32 = 0x320;
const Q0: u32 = 0x330;
const Q1: u32 = 0x340;
const P: u32 = 0x350;
const SD: u32 = 0x360;
const W: u32 = 0x3BC;

const BRIGHT: [[u8; 3]; 7] = [[0xFF, 0, 0], [0, 0xFF, 0xFF], [0xFF, 0xFF, 0], [0xFF, 0, 0xFF], [0, 0xFF, 0], [0, 0x80, 0xFF], [0, 0, 0xFF]];
const PALE: [[u8; 3]; 7] = [[0x80, 0x20, 0x20], [0x20, 0x80, 0x80], [0x80, 0x80, 0x20], [0x80, 0x20, 0x80], [0x20, 0x80, 0], [0x20, 0x40, 0x80], [0x20, 0x20, 0x80]];
const DIM: [[u8; 3]; 7] = [[0x80, 0, 0], [0, 0x80, 0x80], [0x80, 0x80, 0], [0x80, 0, 0x80], [0, 0x80, 0], [0, 0x40, 0x80], [0, 0, 0x80]];

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

fn rem2(x: i32, m: i32) -> i32 {
    let r = x & (m - 1);
    if x < 0 && r != 0 {
        r - m
    } else {
        r
    }
}

/// rand() >> 23 (the caller's s0 is D throughout).
fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 23
}

fn fixu(fx: &mut Fx, v: f32, ra: u32) -> u32 {
    fx.fix_residue(v, ra);
    ee::fixunssfsi(v)
}

fn vec4(fx: &mut Fx, at: u32, x: f32, y: f32, z: f32) {
    fx.mem.setf(at, x);
    fx.mem.setf(at + 4, y);
    fx.mem.setf(at + 8, z);
    fx.mem.write_u32(at + 0xC, 0x3F80_0000);
}

fn rgba(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    let at = sp + SD + 0x10;
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The sprite UVs (u0, v0, u1, v1).
fn uvs(fx: &mut Fx, sp: u32, q: [u32; 4]) {
    let d = sp + SD;
    for (o, v) in [0x18u32, 0x1A, 0x28, 0x2A].into_iter().zip(q) {
        uv(fx, d + o, v);
    }
}

/// The sprite UVs of cell `c` of D+0x30 (u0, u1, v0, v1 halfwords).
fn cell_uvs(fx: &mut Fx, sp: u32, c: u32) {
    let e = D + 0x30 + 8 * c;
    let q = [0u32, 4, 2, 6].map(|o| u32::from(fx.mem.read_u16(e + o)));
    uvs(fx, sp, q);
}

/// The sprite descriptor (5 registers 0x42421, prim 0x26 | 0x800).
fn sprite_head(fx: &mut Fx, sp: u32) {
    let d = sp + SD;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
}

fn alpha(fx: &mut Fx, d: u32, a: i32) {
    let p = fx.alpha_preset(a);
    fx.desc_alpha(d, p);
}

/// The sprite head, ALPHA `a`, texture D + `t`, clamp 5 and the bare
/// view-projection.
fn setup(fx: &mut Fx, sp: u32, a: &[i32], t: u32) {
    let d = sp + SD;
    sprite_head(fx, sp);
    for &a in a {
        alpha(fx, d, a);
    }
    let rh = fx.mem.read_u32(D + t);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MS);
    let m = fx.read_m4(sp + MS);
    fx.mul_into(Some(sp + VP), &m);
}

/// The corners (-k, -k, 0, 1) / (k, k, 0, 1) turned by the camera.
fn corners(fx: &mut Fx, sp: u32, k: f32) {
    let n = ee::neg(k);
    vec4(fx, sp + C0, n, n, 0.0);
    vec4(fx, sp + C1, k, k, 0.0);
    fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
    fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
}

/// sp + 0x350 = `v` x `k` + `at`, the sprite corners about it.
fn along(fx: &mut Fx, sp: u32, v: u32, k: f32, at: u32) {
    fx.vu_scale4(sp + P, v, k);
    fx.vu_add4(sp + P, sp + P, at);
    fx.vu_add4(sp + Q0, sp + C0, sp + P);
    fx.vu_add4(sp + Q1, sp + C1, sp + P);
}

/// 0x204F70 of the sprite from sp + 0x330, keyed by the mean w (`ra`).
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    let d = sp + SD;
    if crate::fxlib2::project(fx, sp + Q0, PROJ_OUT, sp + VP, d, sp + W) {
        let w = fx.mem.f(sp + W);
        let key = fixu(fx, w, ra) as i32;
        fx.submit(d, OT, key);
    }
}

fn dir(fx: &mut Fx, row: i32, s: i32, c: u32) -> f32 {
    fx.mem.f(DIRS + 0x180 * row as u32 + 12 * s as u32 + 4 * c)
}

/// The age at `at` up by one, the flag at `flag` cleared from `end`.
fn age_up(fx: &mut Fx, at: u32, flag: u32, end: i32) {
    let a = fx.mem.i32(at) + 1;
    fx.mem.set32(at, a);
    if a >= end {
        fx.mem.write_u8(flag, 0);
    }
}

/// `n` (cell, brightness) byte pairs at `at`.
fn pairs(fx: &mut Fx, at: u32, n: u32, ra: [u32; 2]) {
    for k in 0..n {
        let b = rem2(rnd(fx, ra[0]), 0x80);
        fx.mem.write_u8(at + 2 * k, b as u8);
        let b = rem2(rnd(fx, ra[1]), 0x20) + 0x60;
        fx.mem.write_u8(at + 2 * k + 1, b as u8);
    }
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x7B80);
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
    let r = fx.request_node(h, 9, 1);
    fx.mem.set32(D + 0x50, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 7, 0);
    fx.mem.set32(D + 0xC60, r);
    for k in 0..4u32 {
        let (u, v) = ((k & 1) << 6, (k >> 1) << 6);
        for (o, x) in [u, u + 0x40, v, v + 0x40].into_iter().enumerate() {
            fx.mem.write_u16(D + 0x30 + 8 * k + 2 * o as u32, (x << 4) as u16);
        }
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x60 + 0x60 * k, 0);
    }
    fx.mem.write_u8(D + 0xC70, 0);
    pairs(fx, D + 0xCB8, 0x40, [0x01E0_01F4, 0x01E0_0214]);
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x1150 + 0x40 * k, 0);
    }
    for k in 0..4u32 {
        fx.mem.write_u8(D + 0x1950 + 0x60 * k, 0);
    }
    fx.mem.write_u32(D + 0x1AD0, 0);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    for k in 0..0x20i32 {
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let (x, y, z) = (dir(fx, 0, k, 0), dir(fx, 2, k, 1), dir(fx, 1, k, 2));
        vec4(fx, D + 0x1AE0 + 0x10 * k as u32, ee::mul(side, ee::mul(2.0, x)), ee::mul(2.0, y), ee::mul(ee::mul(2.0, z), side));
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x1CE0 + 0x230 * k, 0);
    }
    for i in 0..0x20u32 {
        pairs(fx, D + 0x4190 + 0x1D0 * i, 0x18, [0x01E0_0368, 0x01E0_0388]);
    }
    fx.mem.write_u32(D + 0x3FE0, 0);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    for k in 0..0x18i32 {
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let (x, y, z) = (dir(fx, 0, k, 0), dir(fx, 3, k, 1), dir(fx, 2, k, 2));
        vec4(fx, D + 0x3FF0 + 0x10 * k as u32, ee::mul(side, ee::mul(3.0, x)), ee::mul(3.0, y), ee::mul(ee::mul(3.0, z), side));
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x4170 + 0x1D0 * k, 0);
    }
    fx.mem.write_u8(D + 0x7B70, 0);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    let n = fx.mem.i32(D);
    if (0x1F..0x50).contains(&n) {
        spawn_puff(fx, n);
    }
    let d = sp + SD;
    fx.mem.write_u32(sp + 0x1A0, d + 2);
    fx.mem.write_u32(sp + 0x1B0, d + 8);
    fx.mem.write_u32(sp + 0x1C0, d + 4);
    setup(fx, sp, &[1], 0x10);
    puffs(fx, sp);

    if fx.mem.i32(D) == 0x74 {
        spawn_burst(fx);
    }
    setup(fx, sp, &[1], 0x14);
    if fx.mem.read_u8(D + 0xC70) == 1 {
        burst(fx, sp);
    }

    let n = fx.mem.i32(D);
    if (0x74..0xB9).contains(&n) {
        let b = D + 0x1150 + 0x40 * rem2(n, 0x20) as u32;
        spawn_at(fx, b, 2, None, [0x01E0_0FE0, 0x01E0_102C, 0x01E0_106C]);
    }
    setup(fx, sp, &[0], 0x10);
    pairs_draw(fx, sp);

    let n = fx.mem.i32(D);
    if (0x74..0xB9).contains(&n) {
        let b = D + 0x1950 + 0x60 * rem2(n, 4) as u32;
        spawn_at(fx, b, 4, Some(f(0x3DCC_CCCD)), [0x01E0_15D4, 0x01E0_1634, 0x01E0_1688]);
    }
    setup(fx, sp, &[1], 0x10);
    flashes(fx, sp);

    let n = fx.mem.i32(D);
    if (0xBA..0xC9).contains(&n) {
        spawn_ring(fx, n);
    }
    setup(fx, sp, &[0], 0x10);
    rings(fx, sp);

    let n = fx.mem.i32(D);
    if (0xBA..0xD1).contains(&n) {
        spawn_shell(fx, n);
    }
    setup(fx, sp, &[0, 1], 0x14);
    shells(fx, sp);

    if fx.mem.i32(D) == 0xB9 {
        fx.mem.write_u8(D + 0x7B70, 1);
        fx.mem.write_u32(D + 0x7B74, 0);
    }
    if fx.mem.read_u8(D + 0x7B70) == 1 {
        screen(fx, sp);
    }

    let n = fx.mem.i32(D);
    if n == 0xAF {
        fx.mem.write_u32(sp + 0xB0, 1);
    } else if n == 0xB9 {
        fx.mem.write_u32(sp + 0xB0, 2);
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xEA {
        fx.mem.write_u32(sp + 0xB0, 8);
    }
    fx.mem.i32(sp + 0xB0)
}

/// A puff record at the first node: 2 x 2 table vectors shifted by (-+20,
/// -10, -30 side) [F 0x1E004BC..0x1E006AC].
fn spawn_puff(fx: &mut Fx, n: i32) {
    let r = D + 0x60 * rem2(n, 0x20) as u32;
    fx.mem.write_u8(r + 0x60, 1);
    fx.mem.write_u32(r + 0x64, 0);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x50));
    fx.node_position(h, s, r + 0x70);
    for a in 0..2i32 {
        let f20 = ee::mul(20.0, ee::from_i32(2 * a - 1));
        for b in 0..2i32 {
            let k = (2 * a + b) as u32;
            let e = fx.mem.i32(D) + a + b;
            fx.mem.write_u8(r + 0x68 + k, rem2(e, 4) as u8);
            let s4 = rem2(e, 0x20);
            let o = r + 0x80 + 0x10 * k;
            let row = rem2(rnd(fx, 0x01E0_057C), 4);
            let x = ee::add(f20, dir(fx, row, s4, 0));
            fx.mem.setf(o, x);
            let row = rem2(rnd(fx, 0x01E0_05CC), 4);
            let y = ee::add(-10.0, dir(fx, row, s4, 1));
            fx.mem.setf(o + 4, y);
            let row = rem2(rnd(fx, 0x01E0_061C), 4);
            let side = ee::from_i32(fx.mem.i32(D + 0xC));
            let z = ee::add(ee::mul(-30.0, side), dir(fx, row, s4, 2));
            fx.mem.setf(o + 8, z);
            fx.mem.write_u32(o + 0xC, 0x3F80_0000);
        }
    }
}

/// The puffs (32 updates) [F 0x1E0075C..0x1E00A24].
fn puffs(fx: &mut Fx, sp: u32) {
    for i in 0..0x20u32 {
        let r = D + 0x60 * i;
        if fx.mem.read_u8(r + 0x60) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0x64);
        corners(fx, sp, ee::add(3.0, ee::div(ee::mul(3.0, ee::from_i32(age)), 32.0)));
        let a = (((0x20 - age) * 3) << 4) >> 5;
        rgba(fx, sp, [0x30, 0x60, 0xA0], a as u8);
        let f20 = ee::div(ee::from_i32(age), 32.0);
        for k in 0..4u32 {
            along(fx, sp, r + 0x80 + 0x10 * k, f20, r + 0x70);
            let c = u32::from(fx.mem.read_u8(r + 0x68 + k));
            cell_uvs(fx, sp, c);
            draw(fx, sp, 0x01E0_0974);
        }
        age_up(fx, r + 0x64, r + 0x60, 0x21);
    }
}

/// The burst at node 7 [F 0x1E00A38..0x1E00BA0].
fn spawn_burst(fx: &mut Fx) {
    fx.mem.write_u8(D + 0xC70, 1);
    fx.mem.write_u32(D + 0xC74, 0);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC60));
    fx.node_position(h, s, D + 0xD40);
    for k in 0..0x40i32 {
        fx.mem.write_u8(D + 0xC78 + k as u32, rem2(k, 4) as u8);
        let s4 = rem2(k, 0x20);
        let o = D + 0xD50 + 0x10 * k as u32;
        for (c, ra) in [0x01E0_0A94u32, 0x01E0_0AF0, 0x01E0_0B40].into_iter().enumerate() {
            let row = rem2(rnd(fx, ra), 4);
            let v = ee::mul(1.5, dir(fx, row, s4, c as u32));
            fx.mem.setf(o + 4 * c as u32, v);
        }
        fx.mem.write_u32(o + 0xC, 0x3F80_0000);
    }
}

/// 64 sparks in seven colours (64 updates) [F 0x1E00C48..0x1E00F30].
fn burst(fx: &mut Fx, sp: u32) {
    let age = fx.mem.i32(D + 0xC74);
    corners(fx, sp, ee::add(4.0, ee::div(ee::mul(4.0, ee::from_i32(age)), 64.0)));
    let f20 = fx.tan_falloff_p(age, 0x40, 1);
    for k in 0..0x40u32 {
        let b = i32::from(fx.mem.read_u8(D + 0xCB9 + 2 * k));
        let a = ((0x40 - age) * b) >> 6;
        rgba(fx, sp, BRIGHT[(k % 7) as usize], a as u8);
        along(fx, sp, D + 0xD50 + 0x10 * k, f20, D + 0xD40);
        uvs(fx, sp, [0, 0, 0x200, 0x200]);
        draw(fx, sp, 0x01E0_0EEC);
    }
    age_up(fx, D + 0xC74, D + 0xC70, 0x41);
}

/// A record `b` at the first node: `n` table vectors (x `k`) at +0x20,
/// cells (+8) by the count; flag +0, age +4, point +0x10.
fn spawn_at(fx: &mut Fx, b: u32, n: i32, k: Option<f32>, ras: [u32; 3]) {
    fx.mem.write_u8(b, 1);
    fx.mem.write_u32(b + 4, 0);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x50));
    fx.node_position(h, s, b + 0x10);
    for j in 0..n {
        let e = fx.mem.i32(D) + j;
        fx.mem.write_u8(b + 8 + j as u32, rem2(e, 4) as u8);
        let s3 = rem2(e, 0x20);
        let o = b + 0x20 + 0x10 * j as u32;
        for (c, ra) in ras.into_iter().enumerate() {
            let row = rem2(rnd(fx, ra), 4);
            let v = dir(fx, row, s3, c as u32);
            fx.mem.setf(o + 4 * c as u32, k.map_or(v, |k| ee::mul(k, v)));
        }
        fx.mem.write_u32(o + 0xC, 0x3F80_0000);
    }
}

/// The coloured pairs (32 updates) [F 0x1E01168..0x1E0151C].
fn pairs_draw(fx: &mut Fx, sp: u32) {
    for i in 0..0x20u32 {
        let b = D + 0x1150 + 0x40 * i;
        if fx.mem.read_u8(b) == 0 {
            continue;
        }
        let age = fx.mem.i32(b + 4);
        let k = ee::add(f(0x4066_6667), ee::div(ee::mul(f(0x4106_6666), ee::from_i32(age)), 32.0));
        corners(fx, sp, k);
        let a = ((0x20 - age) << 7) >> 5;
        rgba(fx, sp, PALE[(i % 7) as usize], a as u8);
        let f20 = ee::div(ee::from_i32(age), 32.0);
        for j in 0..2u32 {
            along(fx, sp, b + 0x20 + 0x10 * j, f20, b + 0x10);
            let c = u32::from(fx.mem.read_u8(b + 8 + j));
            cell_uvs(fx, sp, c);
            draw(fx, sp, 0x01E0_14A4);
        }
        age_up(fx, b + 4, b, 0x21);
    }
}

/// The four-spark flashes (4 updates) [F 0x1E01790..0x1E019FC].
fn flashes(fx: &mut Fx, sp: u32) {
    for i in 0..4u32 {
        let b = D + 0x1950 + 0x60 * i;
        if fx.mem.read_u8(b) == 0 {
            continue;
        }
        let age = fx.mem.i32(b + 4);
        corners(fx, sp, ee::add(3.0, ee::div(ee::mul(3.0, ee::from_i32(age)), 4.0)));
        let a = ((4 - age) << 7) >> 2;
        rgba(fx, sp, [0x40, 0x40, 0], a as u8);
        let f20 = ee::div(ee::from_i32(age), 4.0);
        for j in 0..4u32 {
            along(fx, sp, b + 0x20 + 0x10 * j, f20, b + 0x10);
            let c = u32::from(fx.mem.read_u8(b + 8 + j));
            cell_uvs(fx, sp, c);
            draw(fx, sp, 0x01E0_1984);
        }
        age_up(fx, b + 4, b, 5);
    }
}

/// `n` points at `out` = the vectors at `from` x `k` + the target's point,
/// cells by the count at `cells`.
fn closing(fx: &mut Fx, out: u32, from: u32, n: u32, k: f32, cells: u32) {
    for j in 0..n {
        let o = out + 0x10 * j;
        fx.vu_scale4(o, from + 0x10 * j, k);
        fx.vu_add4(o, o, D + 0x20);
    }
    for j in 0..n {
        let c = rem2(fx.mem.i32(D) + j as i32, 4);
        fx.mem.write_u8(cells + j, c as u8);
    }
}

/// A ring record [F 0x1E01A1C..0x1E01AF0].
fn spawn_ring(fx: &mut Fx, n: i32) {
    let b = D + 0x1CE0 + 0x230 * rem2(n, 0x10) as u32;
    fx.mem.write_u8(b, 1);
    fx.mem.write_u32(b + 4, 0);
    let a = fx.mem.i32(D + 0x1AD0);
    let f20 = fx.tan_falloff_p(a, 0x10, 1);
    closing(fx, b + 0x30, D + 0x1AE0, 0x20, f20, b + 8);
    fx.mem.set32(D + 0x1AD0, a + 1);
}

/// The rings (16 updates) [F 0x1E01B8C..0x1E01EE4].
fn rings(fx: &mut Fx, sp: u32) {
    for i in 0..0x10u32 {
        let b = D + 0x1CE0 + 0x230 * i;
        if fx.mem.read_u8(b) == 0 {
            continue;
        }
        let age = fx.mem.i32(b + 4);
        let k = ee::add(2.0, ee::div(ee::mul(8.0, ee::from_i32(age)), 16.0));
        corners(fx, sp, k);
        let v = 0x10 - age;
        let a = ((v << 8) - v) >> 4;
        rgba(fx, sp, DIM[(i % 7) as usize], a as u8);
        for j in 0..0x20u32 {
            let p = b + 0x30 + 0x10 * j;
            fx.vu_add4(sp + Q0, sp + C0, p);
            fx.vu_add4(sp + Q1, sp + C1, p);
            let c = u32::from(fx.mem.read_u8(b + 8 + j));
            cell_uvs(fx, sp, c);
            draw(fx, sp, 0x01E0_1E6C);
        }
        age_up(fx, b + 4, b, 0x11);
    }
}

/// A shell record [F 0x1E01F04..0x1E01FD8].
fn spawn_shell(fx: &mut Fx, n: i32) {
    let b = D + 0x4170 + 0x1D0 * rem2(n, 0x20) as u32;
    fx.mem.write_u8(b, 1);
    fx.mem.write_u32(b + 4, 0);
    let a = fx.mem.i32(D + 0x3FE0);
    let f20 = ee::div(ee::from_i32(a), 24.0);
    closing(fx, b + 0x50, D + 0x3FF0, 0x18, f20, b + 8);
    fx.mem.set32(D + 0x3FE0, a + 1);
}

/// The shells (32 updates) [F 0x1E02088..0x1E023AC].
fn shells(fx: &mut Fx, sp: u32) {
    for i in 0..0x20u32 {
        let b = D + 0x4170 + 0x1D0 * i;
        if fx.mem.read_u8(b) == 0 {
            continue;
        }
        let age = fx.mem.i32(b + 4);
        let k = ee::add(f(0x4019_999A), ee::div(ee::mul(f(0x4119_999A), ee::from_i32(age)), 32.0));
        corners(fx, sp, k);
        for j in 0..0x18u32 {
            let w = i32::from(fx.mem.read_u8(b + 0x21 + 2 * j));
            let a = ((0x20 - age) * w) >> 5;
            rgba(fx, sp, DIM[(i % 7) as usize], a as u8);
            let p = b + 0x50 + 0x10 * j;
            fx.vu_add4(sp + Q0, sp + C0, p);
            fx.vu_add4(sp + Q1, sp + C1, p);
            uvs(fx, sp, [0, 0, 0x200, 0x200]);
            draw(fx, sp, 0x01E0_2334);
        }
        age_up(fx, b + 4, b, 0x21);
    }
}

/// The frame as a far-Z sprite, (0xFF, 0xC8, 0xC8) fading over 48 updates
/// [F 0x1E023DC..0x1E02584].
fn screen(fx: &mut Fx, sp: u32) {
    let d = sp + SD;
    sprite_head(fx, sp);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    alpha(fx, d, 0);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.desc_clamp(d, 5);
    uvs(fx, sp, [0x220, 0x220, 0x1DE0, 0x1DE0]);
    let age = fx.mem.i32(D + 0x7B74);
    let a = ((0x30 - age) << 6) / 48;
    rgba(fx, sp, [0xFF, 0xC8, 0xC8], a as u8);
    far_xy(fx, d + 0x20, 0x7000, 0x7000);
    far_xy(fx, d + 0x30, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
    age_up(fx, D + 0x7B74, D + 0x7B70, 0x31);
}
