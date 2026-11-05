//! BMODULE b440 (card 532), hand-ported from the record (linked at
//! 0x1E00000; the entry 0x1E00080 in a 0x490 frame, data area a0 =
//! 0x1ED0000 kept at sp + 0xF0, 0x9920 bytes; the dome callback 0x1E01ED0)
//! [F]: b507's single function (card 521) with
//! other sizes, a third node and a refracting dome drawn into the
//! attacker's target. The init lays out a flower of 8 rings x 16 petals
//! (D+0x2960, 180 long, 6 wide at the base) with cells at D+0x4960, and a
//! flat 32 x 17 disc of radius 35 (32 - i) / 32 at D+0x5520 (0x110 a ring).
//! Every update the camera turn with roll (sp + 0x2C0), the attacker's
//! point (D+0x30), nodes 0x10 / 5 / 0x23 (D+0x50..) and the aim D+0x80
//! along node 0x10 from the midpoint of the other two. On 0x3B..0x108 one
//! of 12 burst records (+0xD0, 0x360; 32 sparks of 1.2 in (1E, 60, FF) /
//! (60, 1E, FF), a 180-wide halo (40, 1C, FF) and 16 rays of twelve
//! 3-wide sprites (5A, 30, FF)); on n % 5 == 4 a flower (+0x49E0) at node
//! 0x10 along the aim with a 160-wide far-Z sprite and 8 white sparks
//! (+0x4B10). From update 0x1E for 0x110 updates (D+0x5510 / 0x5514) the
//! attacker darkened (0x1F7600 mode 1, (0, 0, 0, 0x80)) and, in a packet
//! layer, drawn into its target (0x1F8280 filter 1) with the dome callback
//! (the frame sampled at a breathing 32 x 17 dome's screen points drawn on
//! the flat disc at far Z, alpha 255 x ramp) and composited back by a
//! full-window grey sprite. The stage light 0x80 - 64 clamp(n) clamp(0x135
//! - n) / 1024. Returns 1 on 0x30, 6 on 0x3B..0x108, 2 on 0x109, 8 once
//! the count reaches 0x136.
//!
//! b296 (card 484, [`program_b296`]) is the same program on [`V`]: four
//! nodes 9 / 0xA / 0x1C / 0x1D with two aims (node 0xA from 9, 0x1D from
//! 0x1C), each flower / spark spawn a pair (at nodes 0xA / 0x1D), other
//! colours, sizes and timings, and the dome pass from update 0 for 0x31
//! updates at alpha 255 x ramp(48 / 24) alone.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The stack layout of b440's frame (the other records' frames are larger;
/// only the boundary-visible slots are taken from [`V`]).
const DESC: u32 = 0x3A0;
/// The flower's quad descriptor (prim 0x34, 12 registers).
const QUAD: u32 = 0x3F8;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
/// The entry's s0 (0x206E40's return), rand's saved s0 in the step.
const S0: u32 = OT;

/// One record of the family.
pub struct V {
    frame: u32,
    bzero: u32,
    /// (node, clear) requested into D+0x40.. (positions D+0x50..).
    nodes: &'static [(i32, i32)],
    /// Two aims from node pairs (b296) rather than one from the midpoint.
    pair_aims: bool,
    /// Aims (0x40 each) at D+aim.
    aim: u32,
    burst_idx: u32,
    /// The burst records (0x360 each): flag +0, age +4, cells +8, point
    /// +0x50, spark vectors +0x60, ray vectors +0x260.
    burst: u32,
    burst_len: f32,
    burst_from: i32,
    burst_to: i32,
    spark_rgb: [[u8; 3]; 2],
    /// Halo colour; with `Some(ra)` its alpha 228 (12 - age) / 12.
    halo_rgb: [u8; 3],
    halo_alpha: Option<u32>,
    ray_rgb: [u8; 3],
    petal_len: f32,
    petal_width: f32,
    flower_idx: u32,
    petals: u32,
    cells: u32,
    /// Flower records: count, size, base (flag +0, age +4, angles +8,
    /// points +0x10, matrices +0x10 + 0x10 sub).
    flowers: u32,
    flower_size: u32,
    flower: u32,
    /// Flowers / spark sets a spawn makes, and the node of the first and
    /// the step to the next.
    sub: u32,
    sub_node: u32,
    sub_step: u32,
    /// Spawn on n % m == r within from..to.
    flower_mod: (i32, i32),
    flower_from: i32,
    flower_to: i32,
    /// A flower's life (its alpha 0x56 x (life - age) / 3 or / 2).
    flower_life: i32,
    flower_rgb: [u8; 3],
    /// The flower turned at the live node (else the stored point).
    flower_live: bool,
    far_rgb: [u8; 3],
    spark_idx: u32,
    /// Spark records (0xA0 x sub each): flag +0, age +4, cells +8, points
    /// +0x10 sub, vectors +0x20 sub (0x80 a set).
    sparks: u32,
    spark_len: f32,
    spark_size: f32,
    disc: u32,
    dome: u32,
    pass: u32,
    pass_on: i32,
    pass_len: i32,
    init_tint: Option<u32>,
    /// The dome's alpha ramp: (24, 247..271 with the |sin|^3 pulse) for
    /// b440, (48 / 24 alone) for b296.
    pulse: bool,
    light: i32,
    ret1: i32,
    to: i32,
    end: i32,
    tint_at: u32,
    d_static: u32,
    callback: u32,
    /// Return addresses: init rands (2), burst rand, spark draw, halo
    /// alpha, halo draw, ray draw, flower rand, spark rand, petal draw,
    /// far alpha, spark-record draw, tint (3: step, pass, off), the pass
    /// key, the dome's alpha / angle / four projections.
    ra: [u32; 22],
}

const V440: V = V {
    frame: 0x490,
    bzero: 0x9920,
    nodes: &[(0x10, 1), (5, 0), (0x23, 0)],
    pair_aims: false,
    aim: 0x80,
    burst_idx: 0xC0,
    burst: 0xD0,
    burst_len: 2.0,
    burst_from: 0x3B,
    burst_to: 0x109,
    spark_rgb: [[0x1E, 0x60, 0xFF], [0x60, 0x1E, 0xFF]],
    halo_rgb: [0x40, 0x1C, 0xFF],
    halo_alpha: None,
    ray_rgb: [0x5A, 0x30, 0xFF],
    petal_len: -180.0,
    petal_width: 6.0,
    flower_idx: 0x2950,
    petals: 0x2960,
    cells: 0x4960,
    flowers: 3,
    flower_size: 0x60,
    flower: 0x49E0,
    sub: 1,
    sub_node: 0,
    sub_step: 0,
    flower_mod: (5, 4),
    flower_from: 0x3B,
    flower_to: 0x109,
    flower_life: 3,
    flower_rgb: [0x54, 0x30, 0xFF],
    flower_live: false,
    far_rgb: [0x56, 0x14, 0xFF],
    spark_idx: 0x4B00,
    sparks: 0x4B10,
    spark_len: 0.5,
    spark_size: 10.0,
    disc: 0x5520,
    dome: 0x7720,
    pass: 0x5510,
    pass_on: 0x1E,
    pass_len: 0x110,
    init_tint: None,
    pulse: true,
    light: 0x135,
    ret1: 0x30,
    to: 0x109,
    end: 0x136,
    tint_at: 0x48C,
    d_static: 0x01E0_2800,
    callback: 0x01E0_1ED0,
    ra: [
        0x01E0_02A4, 0x01E0_0350, 0x01E0_0640, 0x01E0_0AC8, 0, 0x01E0_0CE4, 0x01E0_0F44, 0x01E0_1080, 0x01E0_1158, 0x01E0_14AC, 0x01E0_160C, 0x01E0_1A88, 0x01E0_1B60, 0x01E0_1B98, 0x01E0_1DB8, 0x01E0_1D6C,
        0x01E0_2164, 0x01E0_2314, 0x01E0_2408, 0x01E0_246C, 0x01E0_25AC, 0x01E0_2628,
    ],
};

const V296: V = V {
    frame: 0x4A0,
    bzero: 0xA3B0,
    nodes: &[(9, 1), (0xA, 0), (0x1C, 0), (0x1D, 0)],
    pair_aims: true,
    aim: 0x90,
    burst_idx: 0x110,
    burst: 0x120,
    burst_len: 2.5,
    burst_from: 0x44,
    burst_to: 0xBE,
    spark_rgb: [[0x32, 0x40, 0x80], [0x40, 0x80, 0x32]],
    halo_rgb: [0x40, 0xB4, 0xFF],
    halo_alpha: Some(0x01E0_0C70),
    ray_rgb: [0x64, 0x40, 0x80],
    petal_len: -400.0,
    petal_width: 4.0,
    flower_idx: 0x29A0,
    petals: 0x29B0,
    cells: 0x49B0,
    flowers: 2,
    flower_size: 0xB0,
    flower: 0x4A30,
    sub: 2,
    sub_node: 1,
    sub_step: 2,
    flower_mod: (4, 0),
    flower_from: 0x40,
    flower_to: 0xBE,
    flower_life: 2,
    flower_rgb: [0x40, 0x64, 0x80],
    flower_live: true,
    far_rgb: [0xC8, 0x40, 0xFF],
    spark_idx: 0x4B90,
    sparks: 0x4BA0,
    spark_len: 0.7,
    spark_size: 12.0,
    disc: 0x5FB0,
    dome: 0x81B0,
    pass: 0x5FA0,
    pass_on: 0,
    pass_len: 0x31,
    init_tint: Some(0x01E0_04F0),
    pulse: false,
    light: 0xEA,
    ret1: 0x39,
    to: 0xBE,
    end: 0xEB,
    tint_at: 0x49C,
    d_static: 0x01E0_2900,
    callback: 0x01E0_1FC0,
    ra: [
        0x01E0_02C4, 0x01E0_0370, 0x01E0_0690, 0x01E0_0B18, 0x01E0_0C70, 0x01E0_0D7C, 0x01E0_0FD4, 0x01E0_1124, 0x01E0_1228, 0x01E0_158C, 0x01E0_16F0, 0x01E0_1B78, 0x01E0_1C4C, 0x01E0_1C84, 0x01E0_1EA4, 0x01E0_1E58,
        0x01E0_221C, 0x01E0_23CC, 0x01E0_24E8, 0x01E0_254C, 0x01E0_269C, 0x01E0_2718,
    ],
};

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &V440)
}

/// b296 (card 484).
pub fn program_b296(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &V296)
}

fn run(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let sp = fx.enter(v.frame);
    let ret = if param >= 0 { init(fx, sp, v) } else { step(fx, sp, v) };
    fx.leave(v.frame);
    ret
}

/// The compiler's signed remainder by a power of two.
pub(crate) fn rem2(x: i32, m: i32) -> i32 {
    let r = x & (m - 1);
    if x < 0 && r != 0 {
        r - m
    } else {
        r
    }
}

pub(crate) fn words(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

pub(crate) fn copy8(fx: &mut Fx, to: u32, from: u32) {
    let a = fx.mem.read_u32(from);
    let b = fx.mem.read_u32(from + 4);
    fx.mem.write_u32(to, a);
    fx.mem.write_u32(to + 4, b);
}

pub(crate) fn vec4(fx: &mut Fx, p: u32, v: [u32; 4]) {
    for (c, w) in v.iter().enumerate() {
        fx.mem.write_u32(p + 4 * c as u32, *w);
    }
}

/// (x x 0x2AAAAAAB >> 32 >> 1) + sign: x / 12.
pub(crate) fn div12(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    let hi = fx.hilo().1;
    (hi >> 1).wrapping_add(((x as u32) >> 31) as i32)
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
pub(crate) fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// 0x1A2B80 with its residue frame.
pub(crate) fn fixs(fx: &mut Fx, f: f32, ra: u32) -> i32 {
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}

/// The tint 0x1F7600(attacker, mode, c) from `ra`.
pub(crate) fn tint(fx: &mut Fx, mode: i32, c: u32, ra: u32) {
    let h = fx.mem.i32(D + 4);
    fx.unit_tint_mode(h, mode, c, [u64::from(S0), 0], ra);
}

fn init(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.bzero(D, v.bzero);
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
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    for (k, &(node, clear)) in v.nodes.iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0x40 + 4 * k as u32, r);
    }
    fx.mem.write_u32(D + v.burst_idx, 0);
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + v.burst + 0x360 * k, 0);
    }
    fx.mem.write_u32(D + v.flower_idx, 0);
    let (m, vv) = (sp + 0x340, sp + 0x380);
    for s7 in 0..8i32 {
        let f20 = ee::div(ee::mul(v.petal_len, ee::from_i32(s7 + 1)), 8.0);
        let f22 = ee::div(ee::mul(v.petal_len, ee::from_i32(s7)), 7.0);
        let f21 = ee::div(ee::mul(v.petal_width, ee::from_i32(7 - s7)), 7.0);
        let s6 = (s7 << 13) >> 3;
        let ring = D + 0x400 * s7 as u32;
        for s0 in 0..0x10i32 {
            let a = (((s0 << 16) >> 4).wrapping_add(s6)) & 0xFFFF;
            let sn = fx.tsin(a);
            fx.mem.setf(vv, ee::mul(f21, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(vv + 4, ee::mul(f21, cs));
            fx.mem.setf(vv + 8, f22);
            fx.mem.write_u32(vv + 0xC, 0x3F80_0000);
            fx.point_along(m, vv);
            let r = fx.rand_from(s0 as u32, v.ra[0]) >> 23;
            fx.rotate_axis(m, (rem2(r, 256) << 16) >> 8, 2);
            let q = ring + 0x40 * s0 as u32 + v.petals;
            let (n5, p5, one) = (0xC0A0_0000u32, 0x40A0_0000u32, 0x3F80_0000u32);
            vec4(fx, q, [n5, 0, 0, one]);
            vec4(fx, q + 0x10, [p5, 0, 0, one]);
            vec4(fx, q + 0x20, [n5, 0, f20.to_bits(), one]);
            vec4(fx, q + 0x30, [p5, 0, f20.to_bits(), one]);
            for k in 0..4u32 {
                fx.vu_apply_to(q + 0x10 * k, m, q + 0x10 * k);
            }
            let r = fx.rand_from(s0 as u32, v.ra[1]) >> 23;
            fx.mem.set8(D + 0x10 * s7 as u32 + s0 as u32 + v.cells, rem2(r, 4) + 4);
        }
    }
    for k in 0..v.flowers {
        fx.mem.write_u8(D + v.flower + v.flower_size * k, 0);
    }
    fx.mem.write_u32(D + v.spark_idx, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + v.sparks + 0xA0 * v.sub * k, 0);
    }
    fx.mem.write_u8(D + v.pass, 0);
    // The flat disc: 32 rings of 17 points (0x110 a ring).
    for i in 0..0x20i32 {
        let f20 = ee::div(ee::mul(35.0, ee::from_i32(0x20 - i)), 32.0);
        let row = D + 0x110 * i as u32 + v.disc;
        for j in 0..0x11i32 {
            let a = (j << 16) >> 4;
            let sn = fx.tsin(a);
            fx.mem.setf(row + 0x10 * j as u32, ee::mul(f20, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(row + 4 + 0x10 * j as u32, ee::mul(f20, cs));
            fx.mem.write_u32(row + 8 + 0x10 * j as u32, 0);
            fx.mem.write_u32(row + 0xC + 0x10 * j as u32, 0x3F80_0000);
        }
    }
    if let Some(ra) = v.init_tint {
        let c = sp + v.tint_at;
        for (k, b) in [0u8, 0, 0, 0x80].into_iter().enumerate() {
            fx.mem.write_u8(c + k as u32, b);
        }
        tint(fx, 1, c, ra);
    }
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x3B8, u0);
    uv(fx, sp + 0x3BA, v0);
    uv(fx, sp + 0x3C8, u1);
    uv(fx, sp + 0x3CA, v1);
}

/// The 4 x 8 cell `b` of the texture into the sprite's two UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: u32) {
    let (c, r) = (b & 3, b >> 2);
    uvs(fx, sp, (c << 9) & 0x3FFF, (r << 9) & 0x3FFF, (((c << 5) + 0x20) << 4) & 0x3FFF, (((r << 5) + 0x20) << 4) & 0x3FFF);
}

fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x240);
    fx.identity(sp + 0x280);
    let id = fx.read_m4(sp + 0x280);
    fx.mul_into(Some(sp + 0x240), &id);
}

/// The sprite descriptor rewritten (prim 0x26 | 0x800), its texture,
/// clamp 5 and (with `preset`) its ALPHA.
fn desc_reset(fx: &mut Fx, sp: u32, rh_at: u32, preset: Option<i32>) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + rh_at);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    if let Some(p) = preset {
        let a = fx.alpha_preset(p);
        fx.desc_alpha(d, a);
    }
}

/// The corners (-s, -s, 0, 1) at sp + 0x300 and (s, s, 0, 1) at sp +
/// 0x310 (turned by the camera sp + 0x2C0 with `turn`).
fn corners(fx: &mut Fx, sp: u32, s: f32, turn: bool) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x300, n);
    fx.mem.setf(sp + 0x304, n);
    fx.mem.write_u32(sp + 0x308, 0);
    fx.mem.write_u32(sp + 0x30C, 0x3F80_0000);
    fx.mem.setf(sp + 0x310, s);
    fx.mem.setf(sp + 0x314, s);
    fx.mem.write_u32(sp + 0x318, 0);
    fx.mem.write_u32(sp + 0x31C, 0x3F80_0000);
    if turn {
        fx.vu_apply_to(sp + 0x300, sp + 0x2C0, sp + 0x300);
        fx.vu_apply_to(sp + 0x310, sp + 0x2C0, sp + 0x310);
    }
}

fn colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(sp + 0x3B0, c[0]);
    fx.mem.write_u8(sp + 0x3B1, c[1]);
    fx.mem.write_u8(sp + 0x3B2, c[2]);
    fx.mem.write_u8(sp + 0x3B3, a);
    fx.mem.write_u32(sp + 0x3B4, 0x3F80_0000);
}

/// The sprite at sp + 0x380 (= dir x f + at); its vertices sp + 0x320 /
/// 0x330.
fn place(fx: &mut Fx, sp: u32, dir: u32, f: f32, at: u32) {
    fx.vu_scale4(sp + 0x380, dir, f);
    fx.vu_add4(sp + 0x380, sp + 0x380, at);
    fx.vu_add4(sp + 0x320, sp + 0x300, sp + 0x380);
    fx.vu_add4(sp + 0x330, sp + 0x310, sp + 0x380);
}

/// Projected from `input` and submitted at the mean w.
fn draw(fx: &mut Fx, sp: u32, input: u32, ra: u32) {
    if crate::fxlib2::project(fx, input, PROJ_OUT, sp + 0x240, sp + DESC, sp + 0x488) {
        let w = fx.mem.f(sp + 0x488);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// A burst record started [F b440 0x1E00570].
fn burst_spawn(fx: &mut Fx, v: &V) {
    let b = fx.mem.i32(D + v.burst_idx) as u32;
    let r = D + 0x360 * b + v.burst;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 1);
    fx.random_table_vec(r + 0x50, v.burst_len);
    fx.vu_add4(r + 0x50, r + 0x50, D + 0x20);
    for j in 0..0x20u32 {
        let c = fx.rand_from(S0, v.ra[2]) >> 23;
        fx.mem.write_u16(r + 8 + 2 * j, rem2(c, 8) as u16);
        fx.random_table_vec(r + 0x60 + 0x10 * j, f32::from_bits(0x3F99_999A));
    }
    for j in 0..0x10u32 {
        fx.random_table_vec(r + 0x260 + 0x10 * j, 2.0);
    }
    let b = fx.mem.i32(D + v.burst_idx) + 1;
    fx.mem.set32(D + v.burst_idx, b);
    if b >= 0xC {
        fx.mem.write_u32(D + v.burst_idx, 0);
    }
}

/// The bursts [F b440 0x1E00750].
fn bursts(fx: &mut Fx, sp: u32, v: &V) {
    for bi in 0..0xCu32 {
        let r = D + 0x360 * bi + v.burst;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let d = sp + DESC;
        desc_reset(fx, sp, 0x10, None);
        camera(fx, sp);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let s = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), age), 12.0);
        let s = ee::mul(12.0, ee::add(f32::from_bits(0x3F4C_CCCD), s));
        corners(fx, sp, s, true);
        let left = 0xC - fx.mem.i32(r + 4);
        let s1 = div12(fx, left << 7) & 0xFF;
        let pos = r + 0x50;
        for j in 0..0x20u32 {
            let age = fx.mem.i32(r + 4);
            let f22 = fx.tan_falloff_p(age, 0xC, 1);
            let a = fx.alpha_preset(1);
            fx.desc_alpha(d, a);
            colour(fx, sp, v.spark_rgb[(j & 1) as usize], s1 as u8);
            place(fx, sp, r + 0x60 + 0x10 * j, f22, pos);
            let c = fx.mem.read_u16(r + 8 + 2 * j);
            cell_uvs(fx, sp, u32::from(c));
            draw(fx, sp, sp + 0x320, v.ra[3]);
        }
        // The halo, scaled by the spread at the burst's point.
        desc_reset(fx, sp, 0x14, Some(1));
        fx.camera_vp_to(sp + 0x240);
        fx.translate_rows(sp + 0x280, sp + 0x2C0, pos);
        let age = fx.mem.i32(r + 4);
        let f0 = fx.tan_falloff_p(age, 0xC, 1);
        let f = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
        fx.mem.setf(sp + 0x380, f);
        fx.mem.setf(sp + 0x384, f);
        fx.mem.setf(sp + 0x388, f);
        fx.mem.write_u32(sp + 0x38C, 0x3F80_0000);
        fx.scale_rows(sp + 0x280, sp + 0x380);
        let mm = fx.read_m4(sp + 0x280);
        fx.mul_into(Some(sp + 0x240), &mm);
        let ha = match v.halo_alpha {
            Some(ra) => {
                let left = ee::from_i32(0xC - fx.mem.i32(r + 4));
                fix(fx, ee::mul(228.0, ee::div(left, 12.0)), ra) as u8
            }
            None => 0,
        };
        colour(fx, sp, v.halo_rgb, ha);
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        corners(fx, sp, 90.0, false);
        draw(fx, sp, sp + 0x300, v.ra[5]);
        // The rays.
        desc_reset(fx, sp, 0x14, Some(1));
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        camera(fx, sp);
        corners(fx, sp, 1.5, true);
        let age = fx.mem.i32(r + 4);
        let f22 = fx.tan_falloff_p(age, 0xC, 1);
        for m in 0..0x10u32 {
            let dir = r + 0x260 + 0x10 * m;
            for s6 in 0..0xCi32 {
                colour(fx, sp, v.ray_rgb, 0);
                let f12 = ee::add(f22, ee::mul(f32::from_bits(0x3C03_126F), ee::from_i32(s6)));
                place(fx, sp, dir, f12, pos);
                draw(fx, sp, sp + 0x320, v.ra[6]);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0xD {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The node of flower / spark set `s` (D+0x50 + 0x10 node).
fn sub_point(v: &V, s: u32) -> u32 {
    D + 0x50 + 0x10 * (v.sub_node + v.sub_step * s)
}

/// A flower record and a spark record started [F b440 0x1E01024].
fn flower_spawn(fx: &mut Fx, v: &V) {
    let k = fx.mem.i32(D + v.flower_idx) as u32;
    let fb = D + v.flower_size * k + v.flower;
    fx.mem.write_u8(fb, 1);
    fx.mem.write_u32(fb + 4, 0);
    let m = fx.mem.i32(D + v.spark_idx) as u32;
    let tb = D + 0xA0 * v.sub * m + v.sparks;
    fx.mem.write_u8(tb, 1);
    fx.mem.write_u32(tb + 4, 0);
    for s in 0..v.sub {
        let r = fx.rand_from(S0, v.ra[7]) >> 23;
        fx.mem.set32(fb + 8 + 4 * s, (rem2(r, 256) << 16) >> 8);
        words(fx, fb + 0x10 + 0x10 * s, sub_point(v, s));
        let mat = fb + 0x10 + 0x10 * v.sub + 0x40 * s;
        for c in 0..8u32 {
            copy8(fx, mat + 8 * c, D + v.aim + 0x40 * s + 8 * c);
        }
    }
    for s in 0..v.sub {
        words(fx, tb + 0x10 * v.sub + 0x10 * s, sub_point(v, s));
        for s6 in 0..8u32 {
            let r = fx.rand_from(S0, v.ra[8]) >> 23;
            fx.mem.set8(tb + 8 + 8 * s + s6, rem2(r, 8));
            fx.random_table_vec(tb + 0x20 * v.sub + 0x80 * s + 0x10 * s6, v.spark_len);
        }
    }
    let k = fx.mem.i32(D + v.flower_idx) + 1;
    fx.mem.set32(D + v.flower_idx, k);
    if k >= v.flowers as i32 {
        fx.mem.write_u32(D + v.flower_idx, 0);
    }
    let m = fx.mem.i32(D + v.spark_idx) + 1;
    fx.mem.set32(D + v.spark_idx, m);
    if m >= 0x10 {
        fx.mem.write_u32(D + v.spark_idx, 0);
    }
}

/// The flowers [F b440 0x1E0120C].
fn flowers(fx: &mut Fx, sp: u32, v: &V) {
    let q = sp + QUAD;
    fx.mem.write_u16(q + 2, 0x34);
    fx.mem.write_u16(q, 0xC);
    fx.mem.write_u64(q + 8, 0x4214_2142_1421);
    fx.mem.write_u32(q + 4, 0);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(q, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(q, a);
    fx.desc_clamp(q, 0);
    let life = v.flower_life;
    for fi in 0..v.flowers {
        let fb = D + v.flower_size * fi + v.flower;
        if fx.mem.read_u8(fb) == 0 {
            continue;
        }
        let left = life - fx.mem.i32(fb + 4);
        let a = if life == 3 {
            let x = fx.mult(left, 0x56);
            fx.mult(0x5555_5556, x);
            let hi = fx.hilo().1;
            hi.wrapping_add(((x as u32) >> 31) as i32)
        } else {
            fx.mult(left, 0x56) >> 1
        };
        let c = v.flower_rgb;
        fx.mem.write_u8(sp + 0x408, c[0]);
        fx.mem.write_u8(sp + 0x409, c[1]);
        fx.mem.write_u8(sp + 0x40A, c[2]);
        fx.mem.write_u8(sp + 0x40B, a as u8);
        fx.mem.write_u32(sp + 0x40C, 0x3F80_0000);
        copy8(fx, sp + 0x420, sp + 0x408);
        copy8(fx, sp + 0x438, sp + 0x408);
        copy8(fx, sp + 0x450, sp + 0x408);
        let age = ee::from_i32(fx.mem.i32(fb + 4));
        let f = ee::add(0.5, ee::div(ee::mul(0.5, age), ee::from_i32(life)));
        fx.mem.setf(sp + 0x380, f);
        fx.mem.setf(sp + 0x384, f);
        fx.mem.setf(sp + 0x388, f);
        fx.mem.write_u32(sp + 0x38C, 0x3F80_0000);
        for s in 0..v.sub {
            fx.camera_vp_to(sp + 0x240);
            let at = if v.flower_live { sub_point(v, s) } else { fb + 0x10 + 0x10 * s };
            fx.translate_rows(sp + 0x280, fb + 0x10 + 0x10 * v.sub + 0x40 * s, at);
            let ang = fx.mem.i32(fb + 8 + 4 * s);
            fx.rotate_axis(sp + 0x280, ang, 2);
            fx.scale_rows(sp + 0x280, sp + 0x380);
            let mm = fx.read_m4(sp + 0x280);
            fx.mul_into(Some(sp + 0x240), &mm);
            let mm = fx.read_m4(sp + 0x240);
            for s6 in 0..8u32 {
                for s5 in 0..0x10u32 {
                    let cb = i32::from(fx.mem.read_u8(D + 0x10 * s6 + s5 + v.cells));
                    let c = rem2(cb, 4) << 5;
                    let r = (cb >> 2) << 5;
                    let f4 = ee::from_i32(c);
                    let f2 = ee::from_i32(r);
                    let f5 = ee::div(ee::add(32.0, f4), 128.0);
                    let f4 = ee::div(f4, 128.0);
                    let f3 = ee::div(f2, 64.0);
                    let f0 = ee::div(ee::add(16.0, f2), 64.0);
                    fx.mem.setf(sp + 0x410, f4);
                    fx.mem.setf(sp + 0x414, f0);
                    fx.mem.setf(sp + 0x428, f5);
                    fx.mem.setf(sp + 0x42C, f0);
                    fx.mem.setf(sp + 0x440, f4);
                    fx.mem.setf(sp + 0x444, f3);
                    fx.mem.setf(sp + 0x458, f5);
                    fx.mem.setf(sp + 0x45C, f3);
                    let petal = D + 0x400 * s6 + 0x40 * s5 + v.petals;
                    if fx.project_desc(petal, PROJ_OUT, Some(&mm), q, sp + 0x488) {
                        let w = fx.mem.f(sp + 0x488);
                        let key = fix(fx, w, v.ra[9]);
                        fx.submit(q, OT, key as i32);
                    }
                }
            }
        }
        // The far-Z sprites at the points.
        let d = sp + DESC;
        desc_reset(fx, sp, 0x14, Some(1));
        camera(fx, sp);
        corners(fx, sp, 80.0, true);
        let age = fx.mem.i32(fb + 4);
        fx.tan_falloff_p(age, life, 1);
        let a = fix(fx, 0.0, v.ra[10]);
        colour(fx, sp, v.far_rgb, a as u8);
        for s in 0..v.sub {
            words(fx, sp + 0x380, fb + 0x10 + 0x10 * s);
            fx.vu_add4(sp + 0x320, sp + 0x300, sp + 0x380);
            fx.vu_add4(sp + 0x330, sp + 0x310, sp + 0x380);
            uvs(fx, sp, 0x600, 0, 0x800, 0x200);
            if crate::fxlib2::project(fx, sp + 0x320, PROJ_OUT, sp + 0x240, d, sp + 0x488) {
                let x = fx.mem.read_u64(sp + 0x3C0);
                fx.mem.write_u64(sp + 0x3C0, (x & KEEP) | FAR_Z);
                copy_z(fx, sp + 0x3D0, sp + 0x3C0);
                fx.submit(d, OT, 0);
            }
        }
        let age = fx.mem.i32(fb + 4) + 1;
        fx.mem.set32(fb + 4, age);
        if age >= life + 1 {
            fx.mem.write_u8(fb, 0);
        }
    }
}

/// The spark records [F b440 0x1E017E8].
fn sparks(fx: &mut Fx, sp: u32, v: &V) {
    desc_reset(fx, sp, 0x10, Some(0));
    camera(fx, sp);
    for ti in 0..0x10u32 {
        let tb = D + 0xA0 * v.sub * ti + v.sparks;
        if fx.mem.read_u8(tb) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(tb + 4));
        let s = ee::mul(v.spark_size, ee::add(0.5, ee::div(ee::mul(0.5, age), 16.0)));
        corners(fx, sp, s, true);
        colour(fx, sp, [0xFF, 0xFF, 0xFF], 0);
        let age = fx.mem.i32(tb + 4);
        let f20 = fx.tan_falloff_p(age, 0x10, 1);
        for s in 0..v.sub {
            for s6 in 0..8u32 {
                place(fx, sp, tb + 0x20 * v.sub + 0x80 * s + 0x10 * s6, f20, tb + 0x10 * v.sub + 0x10 * s);
                let c = fx.mem.read_u8(tb + 8 + 8 * s + s6);
                cell_uvs(fx, sp, u32::from(c));
                draw(fx, sp, sp + 0x320, v.ra[11]);
            }
        }
        let age = fx.mem.i32(tb + 4) + 1;
        fx.mem.set32(tb + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(tb, 0);
        }
    }
}

/// The darkened attacker drawn into its target with the dome, composited
/// by a full-window sprite [F b440 0x1E01B6C..0x1E01DB4].
fn unit_pass(fx: &mut Fx, sp: u32, v: &V) {
    let c = sp + v.tint_at;
    for (k, b) in [0u8, 0, 0, 0x80].into_iter().enumerate() {
        fx.mem.write_u8(c + k as u32, b);
    }
    tint(fx, 1, c, v.ra[13]);
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let layer = fx.layer_begin();
    // 0x1F7600 leaves a0 = a1 = 0, a2 = the colour, a3 = the object.
    let h = fx.mem.i32(D + 4);
    let i = fx.object_index(h >> 8, h & 0xFF);
    let o = fx.object_at(i);
    fx.texflush([0, 0, c, o]);
    fx.mem.write_u32(v.d_static, D);
    // TEX0 / TEX1 of the target straight into the sprite's slots.
    let (t0, t1) = (sp + v.tint_at - 0xAC, sp + v.tint_at - 0xA4);
    let cfg = dome_of(v);
    let (_, regs) = fx.unit_draw_post(1, t0, t1, v.callback, &mut |fx, p| dome(fx, p, &cfg));
    let x = fx.mem.read_u64(t0);
    fx.mem.write_u64(d + 0x40, x);
    let x = fx.mem.read_u64(t1);
    fx.mem.write_u64(d + 0x48, x);
    fx.texflush(regs);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 0x0000_0800_0080_000A);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    for k in 0..4 {
        fx.mem.write_u8(sp + 0x3B0 + k, 0x80);
    }
    fx.mem.write_u32(sp + 0x3B4, 0x3F80_0000);
    uvs(fx, sp, 0, 0, 0x2000, 0x2000);
    for (at, xy) in [(sp + 0x3C0, 0x6FF8u16), (sp + 0x3D0, 0x8FF8)] {
        fx.mem.write_u16(at, xy);
        fx.mem.write_u16(at + 2, xy);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & KEEP) | FAR_Z);
        fx.mem.write_u8(at + 7, 0);
    }
    fx.submit(d, 0, 0);
    let w = crate::blib::afterimage::depth_w(fx, D + 0x30, 0.0);
    fx.mem.setf(sp + 0x488, w);
    let key = fix(fx, w, v.ra[15]);
    fx.layer_end(layer, OT, key as i32);
}

fn step(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.camera_turn_roll(sp + 0x2C0);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    for k in 0..v.nodes.len() as u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x40 + 4 * k));
        fx.node_position(h, slot, D + 0x50 + 0x10 * k);
    }
    if v.pair_aims {
        for s in 0..2u32 {
            fx.vu_sub4(sp + 0x380, D + 0x60 + 0x20 * s, D + 0x50 + 0x20 * s);
            fx.point_along(D + v.aim + 0x40 * s, sp + 0x380);
        }
    } else {
        fx.vu_add4(sp + 0x390, D + 0x60, D + 0x70);
        fx.vu_scale4(sp + 0x390, sp + 0x390, 0.5);
        fx.vu_sub4(sp + 0x380, D + 0x50, sp + 0x390);
        fx.point_along(D + v.aim, sp + 0x380);
    }
    let n = fx.mem.i32(D);
    if (v.burst_from..v.burst_to).contains(&n) {
        burst_spawn(fx, v);
    }
    bursts(fx, sp, v);
    let n = fx.mem.i32(D);
    if n % v.flower_mod.0 == v.flower_mod.1 && (v.flower_from..v.flower_to).contains(&n) {
        flower_spawn(fx, v);
    }
    flowers(fx, sp, v);
    sparks(fx, sp, v);
    if fx.mem.i32(D) == v.pass_on {
        fx.mem.write_u8(D + v.pass, 1);
        fx.mem.write_u32(D + v.pass + 4, 0);
    }
    tint(fx, 0, 0, v.ra[12]);
    if fx.mem.read_u8(D + v.pass) == 1 {
        unit_pass(fx, sp, v);
        let t = fx.mem.i32(D + v.pass + 4) + 1;
        fx.mem.set32(D + v.pass + 4, t);
        if t >= v.pass_len {
            fx.mem.write_u8(D + v.pass, 0);
            tint(fx, 0, 0, v.ra[14]);
        }
    }
    let n = fx.mem.i32(D);
    let c1 = Fx::clamp_i(v.light - n, 0, 0x20);
    let c2 = Fx::clamp_i(n, 0, 0x20);
    let l = fx.mult(c1, c2.wrapping_neg() << 6);
    fx.set_stage_light(((l >> 10) + 0x80) & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == v.ret1 {
        ret = 1;
    } else {
        if (v.burst_from..v.to).contains(&n) {
            ret = 6;
        }
        if n == v.to {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= v.end {
        ret = 8;
    }
    ret
}

// ------------------------------------------------------------ the dome

/// The dome's pulse: b440's 0.1 + 0.9 |sin|^3, b296's none (powf still
/// called, its result dropped), b389's 0.1 + 0.9 |sin| (no powf), b766's
/// none (|sin| taken and dropped, no powf).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Pulse {
    Cubed,
    Dropped,
    Abs,
    AbsDropped,
}

/// One record's dome callback: the static holding D, the camera point
/// (D + cam), the pass timer, the dome / disc rows, the ramp (ratio
/// below `ratio` when set, times the ramp above `ramp`), the pulse, the
/// alpha scale, the dome radius and wobble, and the return addresses
/// (alpha, angle, four projections).
pub(crate) struct Dome {
    pub d_static: u32,
    pub cam: u32,
    pub timer: u32,
    pub dome: u32,
    pub disc: u32,
    pub ratio: Option<f32>,
    pub ramp: (f32, f32),
    pub pulse: Pulse,
    pub alpha: f32,
    pub radius: f32,
    pub wobble: f32,
    pub ra: [u32; 6],
}

fn dome_of(v: &V) -> Dome {
    let mut ra = [0; 6];
    ra.copy_from_slice(&v.ra[16..22]);
    Dome {
        d_static: v.d_static,
        cam: 0x30,
        timer: v.pass + 4,
        dome: v.dome,
        disc: v.disc,
        ratio: if v.pulse { Some(24.0) } else { None },
        ramp: if v.pulse { (271.0, 247.0) } else { (48.0, 24.0) },
        pulse: if v.pulse { Pulse::Cubed } else { Pulse::Dropped },
        alpha: 255.0,
        radius: 35.0,
        wobble: 5461.0,
        ra,
    }
}

/// 0x2096C0(p, mask) [F]: FRAME_1 (0x4C) as a register packet at `p`
/// (DMA cnt 2, VIF NOP / DIRECT 2, A+D GIF tag): the frame of the parity
/// (0x2E93D0 + 0x60 / + 0x150 by 0x2E963C), or with the display flags
/// 0x2E93C0 bits 0 and 1 the FBP from 0x2E93A0 / 0x29322C; FBMSK = `mask`.
/// The pointer after it (p + 0x30).
pub(crate) fn frame_mask_packet(fx: &mut Fx, p: u32, mask: u32, a23: [u32; 2]) -> u32 {
    fx.boundary(0x0020_96C0, "packet writer 0x2096C0", [p, mask, a23[0], a23[1]]);
    let disp = 0x002E_93D0u32;
    let a3 = 0x002E_93A0u32;
    let t0 = if fx.mem.read_u32(0x002E_963C) & 1 != 0 { disp + 0x150 } else { disp + 0x60 };
    fx.mem.write_u32(p, 0x1000_0002);
    fx.mem.write_u32(p + 4, 0);
    fx.mem.write_u32(p + 8, 0x1100_0000);
    fx.mem.write_u32(p + 0xC, 0x5100_0002);
    fx.mem.write_u64(p + 0x10, 0x1000_0000_0000_8001);
    fx.mem.write_u64(p + 0x18, 0xE);
    let flags = fx.mem.read_u8(a3 + 0x20);
    let hi = u64::from(mask) << 32;
    let v = if flags & 1 != 0 && flags & 2 != 0 {
        let k = fx.mem.read_u32(a3);
        let fbw = (fx.mem.read_u32(0x0029_322C + 0x28 * k) as i32 >> 6) as i64 as u64;
        let fbp = u64::from(fx.mem.read_u32(a3 + 0xC));
        let psm = u64::from(fx.mem.read_u32(a3 + 0x14));
        hi | (psm << 24) | fbp | (fbw << 16)
    } else {
        let fbp = u64::from(fx.mem.read_u16(t0) & 0x1FF);
        let fbw = u64::from(fx.mem.read_u8(t0 + 2) & 0x3F);
        let psm = u64::from(fx.mem.read_u8(t0 + 3) & 0x3F);
        hi | (psm << 24) | fbp | (fbw << 16)
    };
    fx.mem.write_u64(p + 0x20, v);
    fx.mem.write_u64(p + 0x28, 0x4C);
    p + 0x30
}

/// The z field of the XYZ2 at `from` into the one at `to`.
pub(crate) fn copy_z(fx: &mut Fx, to: u32, from: u32) {
    let z = ((fx.mem.read_u64(from) << 8) >> 40) << 32;
    let x = fx.mem.read_u64(to);
    fx.mem.write_u64(to, (x & KEEP) | z);
}

/// The dome callback (b440 0x1E01ED0 / b296 0x1E01FC0)(p) [F]: run by
/// 0x1F0FD0 after the attacker's render into its target. FRAME_1 masked
/// (0x2096C0 0xFF000000), the camera at the attacker's point (sp + 0x130),
/// a TEXFLUSH, the frame drawn (0x207FC0 -1, filter 1) as the texture of
/// a triangle descriptor (sp + 0x208, prim 0x33) and a quad one (sp +
/// 0x280, prim 0x34), both ALPHA preset 0, CLAMP 0x800_0080_000A, colour
/// (FF, FF, FF, 255 x ramp), then 31 rows of 16 quads and one fan of 16
/// triangles: UVs from the breathing dome (rebuilt here) projected, drawn
/// on the flat disc at far Z, buffered into the packet; FRAME_1 unmasked.
pub(crate) fn dome(fx: &mut Fx, p: u32, v: &Dome) -> u32 {
    let sp = fx.enter(0x320);
    let d = fx.mem.read_u32(v.d_static);
    fx.mem.write_u32(sp + 0xB0, d);
    let s0 = frame_mask_packet(fx, p, 0xFF00_0000, [0, 0]);
    fx.camera_turn_roll(sp + 0x1B0);
    fx.camera_vp_to(sp + 0x130);
    words(fx, sp + 0x1F0, d + v.cam);
    fx.translate_rows(sp + 0x170, sp + 0x1B0, sp + 0x1F0);
    let m = fx.read_m4(sp + 0x170);
    fx.mul_into(Some(sp + 0x130), &m);
    let (ta, qb) = (sp + 0x208, sp + 0x280);
    fx.mem.write_u16(ta + 2, 0x33);
    fx.mem.write_u16(ta, 9);
    fx.mem.write_u64(ta + 8, 0x4_2142_1421);
    fx.mem.write_u32(ta + 4, 0);
    fx.mem.write_u16(qb + 2, 0x34);
    fx.mem.write_u16(qb, 0xC);
    fx.mem.write_u64(qb + 8, 0x4214_2142_1421);
    fx.mem.write_u32(qb + 4, 0);
    // 0x188420 leaves a3 = the turn's first word.
    let a3 = fx.mem.read_u32(sp + 0x1B0);
    fx.boundary(0x0020_EA20, "packet writer 0x20EA20", [s0, 0x2142_1421, ta, a3]);
    let p1 = fx.texflush_packet(s0);
    let p2 = fx.frame_texture(p1, -1, 1, sp + 0x268, sp + 0x270);
    let t = fx.mem.read_u64(sp + 0x268);
    fx.mem.write_u64(sp + 0x2F8, t);
    let t = fx.mem.read_u64(sp + 0x270);
    fx.mem.write_u64(sp + 0x300, t);
    let a1 = fx.mem.read_u32(sp + 0x268);
    fx.boundary(0x0020_EA20, "packet writer 0x20EA20", [p2, a1, sp + 0x2F8, 0]);
    let mut s5 = fx.texflush_packet(p2);
    for dd in [ta, qb] {
        let a = fx.alpha_preset(0);
        fx.desc_alpha(dd, a);
        fx.desc_clamp(dd, 0x0000_0800_0080_000A);
        let f = fx.mem.read_u16(dd + 2);
        fx.mem.write_u16(dd + 2, f | 0x800);
    }
    let t = fx.mem.i32(d + v.timer);
    let f0 = Fx::ramp_above(ee::from_i32(t), v.ramp.0, v.ramp.1);
    let f20 = match v.ratio {
        Some(b) => ee::mul(Fx::ratio_below(ee::from_i32(t), b), f0),
        None => f0,
    };
    let a = ((rem2(t, 0x40) << 16) >> 6) & 0xFFFF;
    let s = fx.tsin(a);
    fx.mem.write_f32(fx.sp - 0x10, s);
    let s = f32::from_bits(s.to_bits() & 0x7FFF_FFFF);
    let s = if matches!(v.pulse, Pulse::Abs | Pulse::AbsDropped) { s } else { fx.powf(s, 3.0) };
    let f1 = if matches!(v.pulse, Pulse::Dropped | Pulse::AbsDropped) {
        ee::mul(f20, 1.0)
    } else {
        let f1 = ee::mul(f32::from_bits(0x3F66_6666), s);
        let f0 = ee::add(f32::from_bits(0x3DCC_CCCD), f1);
        ee::mul(f20, f0)
    };
    for k in 0..3 {
        fx.mem.write_u8(sp + 0x218 + k, 0xFF);
    }
    let alpha = fix(fx, ee::mul(v.alpha, f1), v.ra[0]);
    fx.mem.write_u8(sp + 0x21B, alpha as u8);
    fx.mem.write_u32(sp + 0x21C, 0x3F80_0000);
    for to in [0x230, 0x248, 0x310, 0x290, 0x2A8, 0x2C0, 0x2D8] {
        copy8(fx, sp + to, sp + 0x218);
    }
    // The dome rebuilt: 32 rings of 17 points.
    let fp = fx.mult(t, 0xD3);
    let t111 = t.wrapping_mul(0x6F);
    for i in 0..0x20i32 {
        let f21 = ee::div(ee::mul(v.radius, ee::from_i32(0x20 - i)), 32.0);
        let f20 = ee::div(ee::from_i32(i + 1), 32.0);
        let s7 = (i << 17) >> 5;
        let s4 = (((i << 14) >> 5).wrapping_add(t111)) & 0xFFFF;
        let row = d + 0x110 * i as u32 + v.dome;
        for j in 0..0x11i32 {
            let a = (s7 + ((j << 17) >> 4)).wrapping_add(fp) & 0xFFFF;
            let c = fx.tcos(a);
            let f22 = ee::add(1.0, ee::mul(f20, ee::mul(f32::from_bits(0x3E99_999A), c)));
            let c2 = fx.tcos(s4);
            let f12 = ee::add(ee::from_i32((j << 16) >> 4), ee::mul(f20, ee::mul(v.wobble, c2)));
            let s6 = fixs(fx, f12, v.ra[1]) & 0xFFFF;
            let sn = fx.tsin(s6);
            let f22 = ee::mul(f21, f22);
            fx.mem.setf(row + 0x10 * j as u32, ee::mul(f22, sn));
            let cs = fx.tcos(s6);
            fx.mem.setf(row + 4 + 0x10 * j as u32, ee::mul(f22, cs));
            fx.mem.write_u32(row + 8 + 0x10 * j as u32, 0);
            fx.mem.write_u32(row + 0xC + 0x10 * j as u32, 0x3F80_0000);
        }
    }
    vec4(fx, sp + 0x1F0, [0, 0, 0, 0x3F80_0000]);
    let mm = sp + 0x130;
    let (dm, dc) = (v.dome, v.disc);
    for k in 0..0x20u32 {
        let fpr = d + 0x110 * k;
        if k == 0x1F {
            for s0 in 0..0x10u32 {
                let (s1, s3) = (fpr + 0x10 * s0, fpr + 0x10 * (s0 + 1));
                if fx.project_n(&[sp + 0x1F0, s1 + dm, s3 + dm], mm, ta, sp + 0x31C, v.ra[2]) != 1 {
                    continue;
                }
                for (u, x) in [(0x220, 0x228), (0x238, 0x240), (0x250, 0x258)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&[sp + 0x1F0, s1 + dc, s3 + dc], mm, ta, sp + 0x31C, v.ra[3]) != 1 {
                    continue;
                }
                let x = fx.mem.read_u64(sp + 0x228);
                fx.mem.write_u64(sp + 0x228, (x & KEEP) | FAR_Z);
                copy_z(fx, sp + 0x240, sp + 0x228);
                copy_z(fx, sp + 0x258, sp + 0x228);
                s5 = fx.submit_at(s5, ta, 0, 0);
            }
        } else {
            let nrow = d + 0x110 * (k + 1);
            for s6 in 0..0x10u32 {
                let (s1, s4) = (fpr + 0x10 * s6, fpr + 0x10 * (s6 + 1));
                let (s0, s3) = (nrow + 0x10 * s6, nrow + 0x10 * (s6 + 1));
                if fx.project_n(&[s1 + dm, s4 + dm, s0 + dm, s3 + dm], mm, qb, sp + 0x31C, v.ra[4]) != 1 {
                    continue;
                }
                for (u, x) in [(0x298, 0x2A0), (0x2B0, 0x2B8), (0x2C8, 0x2D0), (0x2E0, 0x2E8)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&[s1 + dc, s4 + dc, s0 + dc, s3 + dc], mm, qb, sp + 0x31C, v.ra[5]) != 1 {
                    continue;
                }
                let x = fx.mem.read_u64(sp + 0x2A0);
                fx.mem.write_u64(sp + 0x2A0, (x & KEEP) | FAR_Z);
                for to in [0x2B8, 0x2D0, 0x2E8] {
                    copy_z(fx, sp + to, sp + 0x2A0);
                }
                s5 = fx.submit_at(s5, qb, 0, 0);
            }
        }
    }
    let r = frame_mask_packet(fx, s5, 0, [0, 0]);
    fx.leave(0x320);
    r
}
