//! BMODULE b420 (card 496), hand-ported from the record (linked at
//! 0x1E00000; one function, the entry 0x1E00080 in a 0x7A0 frame with D
//! kept at sp + 0xBC, the OT in s0, the return at sp + 0xC0; data area
//! a0 = 0x1ED0000, 0xD540 bytes) [F]. b511's
//! family (its locals 0x40..0x100 higher, no unit pass, no dome): the init
//! lays out two flowers of 8 rings x 16 shards (D+0x35C0, 30 long / 12.5
//! wide, cells D+0x55C0; D+0x6790, 240 long / 6 wide tilted by the side,
//! cells D+0x8790). Every update the camera turn with roll (sp + 0x4A0),
//! the attacker's point D+0x30, seven nodes 0x1C / 0x23 / 0x22 / 9 / 0xA /
//! 0xB / 0xC (D+0x70..) with one aim (D+0xE0: node 0x23 from 0x1C) and the
//! target's screen point D+0x40. A lens at the target on 0xEA / 0xFB /
//! 0x10C / 0x11D for 0x21 updates (D+0xD530, b424's fan); on 0x31..0x84 a
//! flare each update at the target (12 x 0x460 at D+0x130, 0xD updates: 32
//! sparks in two colours, a 45-wide halo, 32 rays of six sprites); on n %
//! 4 == 1 of 0x2D..0x84 a flower at node 0x23 (D+0x5640, 3 x 0x60), a
//! spark set there (D+0x5770, 16 x 0xA0) and a falling quad from node 0x22
//! (D+0x6180, 32 x 0x30, b488's quad); on 0xBA / 0xC5 / 0xCD / 0xD5 a big
//! flower and 32 sparks at node 9 / 0xA / 0xB / 0xC (D+0x8810, 4 x 0x20;
//! D+0x88A0, 4 x 0x240); with the lens a burst at the target (D+0x91B0, 4
//! x 0x10E0, 0x21 updates: 64 puffs, 64 sparks in two colours, a 120-wide
//! halo and 128 rays of six sprites). The stage light 0x80 - 80
//! clamp(0x15D - n) clamp(n) / 1024; returns 1 on 0x26 / 0xE0, 6 on
//! 0x31..0x84 / 0xEB..0x13D, 7 on 0x84, 2 on 0x13D, 8 on 0x15E.
use crate::b182::{alpha, head, rgba, uv};
use crate::b440::{copy8, copy_z, fix, rem2, words};
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::LENS_CLAMP;

const D: u32 = crate::bmodule::DATA_AREA;
const FRAME: u32 = 0x7A0;
const PROJ_OUT: u32 = 0x002D_3600;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
/// The frame's locals.
const VP: u32 = 0x420;
const MAT: u32 = 0x460;
const TURN: u32 = 0x4A0;
const C0: u32 = 0x4E0;
const C1: u32 = 0x4F0;
const V0: u32 = 0x520;
const V1: u32 = 0x530;
const ROT: u32 = 0x560;
const PT: u32 = 0x5A0;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const DESC: u32 = 0x5B8;
const COL: u32 = DESC + 0x10;
const UV0: u32 = DESC + 0x18;
const Z0: u32 = DESC + 0x20;
const UV1: u32 = DESC + 0x28;
const Z1: u32 = DESC + 0x30;
/// The falling quad's descriptor (prim 0x24, 9 registers).
const STRIP: u32 = 0x610;
/// The lens' triangle descriptor.
const TRI: u32 = 0x688;
/// The flowers' quad descriptor (prim 0x34, 12 registers).
const QUAD: u32 = 0x700;
const SCREEN: u32 = 0x790;
const W: u32 = 0x79C;
/// The rays' step along their direction.
const RAY_STEP: u32 = 0x3BE5_6042;

const FLARES: u32 = 0x130;
const FLOWERS: u32 = 0x5640;
const SPARKS: u32 = 0x5770;
const QUADS: u32 = 0x6180;
const BIG_FLOWERS: u32 = 0x8810;
const BIG_SPARKS: u32 = 0x88A0;
const BURSTS: u32 = 0x91B0;

const LENS_RAS: [u32; 12] = [
    0x01E0_0B54, 0x01E0_0B64, 0x01E0_0BA8, 0x01E0_0BB4, 0x01E0_0BF8, 0x01E0_0C04, 0x01E0_0CCC, 0x01E0_0CDC, 0x01E0_0D48, 0x01E0_0D54, 0x01E0_0DC0,
    0x01E0_0DCC,
];

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E00080 of b420 (card 496).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xBC, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(OT, ra) >> 23
}

fn vec4(fx: &mut Fx, p: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        fx.mem.setf(p + 4 * k as u32, *x);
    }
}

/// A flower of 8 rings x 16 shards at D+`at` (cells D+`cells`), half-width
/// 2.5. `second`: the second flower, whose rands save other s0s (the
/// angle, then one shard past `q`) than the first (j).
struct Ring {
    at: u32,
    cells: u32,
    ra: [u32; 2],
    second: bool,
}

/// Ring i: `f20` long, `f21` wide, `z` the tilt.
fn ring(fx: &mut Fx, sp: u32, v: &Ring, i: i32, f20: f32, z: f32, f21: f32) {
    let (m, vv) = (sp + ROT, sp + PT);
    let s6 = (i << 13) >> 3;
    for j in 0..0x10i32 {
        let a = (((j << 16) >> 4).wrapping_add(s6)) & 0xFFFF;
        let sn = fx.tsin(a);
        fx.mem.setf(vv, ee::mul(f21, sn));
        let cs = fx.tcos(a);
        fx.mem.setf(vv + 4, ee::mul(f21, cs));
        fx.mem.setf(vv + 8, z);
        fx.mem.write_u32(vv + 0xC, 0x3F80_0000);
        fx.point_along(m, vv);
        let shard = 0x400 * i as u32 + 0x40 * j as u32;
        let q = D + shard + v.at;
        let r = fx.rand_from(if v.second { a as u32 } else { j as u32 }, v.ra[0]) >> 23;
        fx.rotate_axis(m, (rem2(r, 256) << 16) >> 8, 2);
        vec4(fx, q, [-2.5, 0.0, 0.0, 1.0]);
        vec4(fx, q + 0x10, [2.5, 0.0, 0.0, 1.0]);
        vec4(fx, q + 0x20, [-2.5, 0.0, f20, 1.0]);
        vec4(fx, q + 0x30, [2.5, 0.0, f20, 1.0]);
        for k in 0..4u32 {
            fx.vu_apply_to(q + 0x10 * k, m, q + 0x10 * k);
        }
        let s0 = if v.second { D + shard + 0x40 } else { j as u32 };
        let r = fx.rand_from(s0, v.ra[1]) >> 23;
        fx.mem.set8(D + 0x10 * i as u32 + j as u32 + v.cells, rem2(r, 4) + 4);
    }
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0xD540);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..4u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    for (k, &node) in [0x1C, 0x23, 0x22, 9, 0xA, 0xB, 0xC].iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set32(D + 0x50 + 4 * k as u32, r);
    }
    // The record's clears of flags and counters repeat the bzero.
    let first = Ring { at: 0x35C0, cells: 0x55C0, ra: [0x01E0_0350, 0x01E0_03F8], second: false };
    for i in 0..8i32 {
        let f20 = ee::div(ee::mul(-30.0, ee::from_i32(i + 1)), 8.0);
        let z = ee::div(ee::mul(-30.0, ee::from_i32(i)), 7.0);
        let f21 = ee::div(ee::mul(12.5, ee::from_i32(7 - i)), 7.0);
        ring(fx, sp, &first, i, f20, z, f21);
    }
    let second = Ring { at: 0x6790, cells: 0x8790, ra: [0x01E0_05EC, 0x01E0_0698], second: true };
    for i in 0..8i32 {
        let f20 = ee::div(ee::mul(-240.0, ee::from_i32(i + 1)), 8.0);
        let z = ee::div(ee::mul(-240.0, ee::from_i32(i)), 7.0);
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let z = ee::mul(-1.0, ee::mul(z, side));
        let f21 = ee::div(ee::mul(6.0, ee::from_i32(7 - i)), 7.0);
        ring(fx, sp, &second, i, f20, z, f21);
    }
    0
}

fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MAT);
    mul_mem(fx, sp + VP, sp + MAT);
}

/// The screen point of D+0x20 into D+0x40 (x / y clamped to 0..512).
fn screen_point(fx: &mut Fx, sp: u32) {
    let (p, o) = (D + 0x20, D + 0x40);
    let m = fx.read_m4(sp + VP);
    fx.project_at(p, &m, sp + SCREEN, sp + W);
    let x = (i32::from(fx.mem.read_u16(sp + SCREEN)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + SCREEN + 2)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + SCREEN) << 8) >> 40;
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

fn pulse(n: i32) -> bool {
    matches!(n, 0xEA | 0xFB | 0x10C | 0x11D)
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    for k in 0..7u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x50 + 4 * k);
        fx.node_position(h, s, D + 0x70 + 0x10 * k);
    }
    fx.vu_sub4(sp + PT, D + 0x80, D + 0x70);
    fx.point_along(D + 0xE0, sp + PT);
    camera(fx, sp);
    screen_point(fx, sp);

    lens(fx, sp);
    let n = fx.mem.i32(D);
    if (0x31..0x84).contains(&n) {
        flare_spawn(fx);
    }
    for k in 0..0xCu32 {
        flare(fx, sp, D + 0x460 * k + FLARES);
    }
    let n = fx.mem.i32(D);
    if rem2(n, 4) == 1 && (0x2D..0x84).contains(&n) {
        flower_spawn(fx, sp);
    }
    quad_head(fx, sp);
    flowers(fx, sp);
    sparks(fx, sp);
    quads(fx, sp);
    let idx = match fx.mem.i32(D) {
        0xD5 => 3,
        0xCD => 2,
        0xC5 => 1,
        0xBA => 0,
        _ => -1i32,
    };
    fx.mem.set32(sp + 0x3E0, idx);
    if idx >= 0 {
        big_spawn(fx, idx as u32);
    }
    quad_head(fx, sp);
    big_flowers(fx, sp);
    big_sparks(fx, sp);
    if pulse(fx.mem.i32(D)) {
        burst_spawn(fx);
    }
    for k in 0..4u32 {
        burst(fx, sp, D + 0x10E0 * k + BURSTS);
    }

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x15D - n, 0, 0x20);
    let n = fx.mem.i32(D);
    let c = Fx::clamp_i(n, 0, 0x20);
    let l = (fx.mult(s0, c.wrapping_mul(-80)) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x26 || n == 0xE0 {
        ret = 1;
    } else {
        if (0x31..0x84).contains(&n) || (0xEB..0x13D).contains(&n) {
            ret = 6;
        }
        if n == 0x84 {
            ret = 7;
        }
        if n == 0x13D {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x15E {
        ret = 8;
    }
    fx.mem.set32(sp + 0xC0, ret);
    ret
}

/// The lens at the target's screen point (D+0xD530 for 0x21 updates) [F
/// 0x1E00944..0x1E00EBC].
fn lens(fx: &mut Fx, sp: u32) {
    if pulse(fx.mem.i32(D)) {
        fx.mem.write_u8(D + 0xD530, 1);
        fx.mem.write_u32(D + 0xD534, 0);
    }
    if fx.mem.read_u8(D + 0xD530) != 1 {
        return;
    }
    let t = sp + TRI;
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0x800);
    alpha(fx, t, 0);
    fx.desc_clamp(t, LENS_CLAMP);
    let a = crate::b182::clamp_regs(fx, t, LENS_CLAMP);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xEC, layer);
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x6E8, sp + 0x6F0);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0xD534);
    let al = ((0x20 - age) << 6) >> 5;
    rgba(fx, t + 0x10, [0x80; 3], al as u8);
    copy8(fx, t + 0x28, t + 0x10);
    copy8(fx, t + 0x40, t + 0x10);
    for j in 0..8i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f(0x3E19_999A), ee::from_i32(8 - j)), 8.0));
        crate::b424::fan(fx, t, D + 0x40, sp + V0, s, 0, &LENS_RAS);
    }
    let layer = fx.mem.read_u32(sp + 0xEC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0xD534) + 1;
    fx.mem.set32(D + 0xD534, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + 0xD530, 0);
    }
}

/// A flare (D+0x130 + 0x460 k, age from 1) [F 0x1E00EDC..0x1E010B8]: the
/// point +0x50 about the target, 32 cells +8 (halfwords), 32 spark vectors
/// 0.5 +0x60, 32 ray vectors 1.75 +0x260.
fn flare_spawn(fx: &mut Fx) {
    let k = fx.mem.i32(D + 0x120) as u32;
    let r = D + 0x460 * k + FLARES;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 1);
    fx.random_table_vec(r + 0x50, 1.0);
    fx.vu_add4(r + 0x50, r + 0x50, D + 0x20);
    for j in 0..0x20u32 {
        let b = rnd(fx, 0x01E0_0FAC);
        fx.mem.write_u16(r + 8 + 2 * j, rem2(b, 8) as u16);
        fx.random_table_vec(r + 0x60 + 0x10 * j, 0.5);
    }
    for j in 0..0x20u32 {
        fx.random_table_vec(r + 0x260 + 0x10 * j, 1.75);
    }
    let k = fx.mem.i32(D + 0x120) + 1;
    fx.mem.set32(D + 0x120, k);
    if k >= 0xC {
        fx.mem.write_u32(D + 0x120, 0);
    }
}

/// The sprite descriptor with texture D+`tex`, clamp 5 and (with `preset`)
/// its ALPHA.
fn sprite_desc(fx: &mut Fx, sp: u32, tex: u32, preset: Option<i32>) {
    let d = sp + DESC;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    if let Some(p) = preset {
        alpha(fx, d, p);
    }
}

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1), turned with `turn`.
fn corners(fx: &mut Fx, sp: u32, s: f32, turn: bool) {
    let n = ee::neg(s);
    vec4(fx, sp + C0, [n, n, 0.0, 1.0]);
    vec4(fx, sp + C1, [s, s, 0.0, 1.0]);
    if turn {
        fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
        fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
    }
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + UV0, u0);
    uv(fx, sp + UV0 + 2, v0);
    uv(fx, sp + UV1, u1);
    uv(fx, sp + UV1 + 2, v1);
}

/// The 4 x 8 cell `b` of the texture into the sprite's UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    uvs(fx, sp, ((c << 9) & 0x3FFF) as u32, ((r << 9) & 0x3FFF) as u32, ((((c << 5) + 0x20) << 4) & 0x3FFF) as u32, ((((r << 5) + 0x20) << 4) & 0x3FFF) as u32);
}

/// The sprite's vertices about sp + PT.
fn verts(fx: &mut Fx, sp: u32) {
    fx.vu_add4(sp + V0, sp + C0, sp + PT);
    fx.vu_add4(sp + V1, sp + C1, sp + PT);
}

/// The sprite at sp + PT (= dir x f + at).
fn place(fx: &mut Fx, sp: u32, dir: u32, f: f32, at: u32) {
    fx.vu_scale4(sp + PT, dir, f);
    fx.vu_add4(sp + PT, sp + PT, at);
    verts(fx, sp);
}

/// The primitive at `v` through `desc` submitted at its mean w.
fn draw_from(fx: &mut Fx, sp: u32, v: u32, desc: u32, ra: u32) {
    if crate::fxlib2::project(fx, v, PROJ_OUT, sp + VP, desc, sp + W) {
        let w = fx.mem.f(sp + W);
        let key = fix(fx, w, ra);
        fx.submit(desc, OT, key as i32);
    }
}

fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    draw_from(fx, sp, sp + V0, sp + DESC, ra);
}

/// The sprite with vertices at `v` projected at far Z.
fn far_sprite(fx: &mut Fx, sp: u32, v: u32) {
    let d = sp + DESC;
    if crate::fxlib2::project(fx, v, PROJ_OUT, sp + VP, d, sp + W) {
        let x = fx.mem.read_u64(sp + Z0);
        fx.mem.write_u64(sp + Z0, (x & KEEP) | FAR_Z);
        copy_z(fx, sp + Z1, sp + Z0);
        fx.submit(d, OT, 0);
    }
}

/// The sprite at the point `at` projected at far Z.
fn draw_far(fx: &mut Fx, sp: u32, at: u32) {
    words(fx, sp + PT, at);
    verts(fx, sp);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    far_sprite(fx, sp, sp + V0);
}

/// (x x 0x2AAAAAAB >> 32 >> 1) + sign: x / 12.
fn div12(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    (fx.hilo().1 >> 1).wrapping_add(((x as u32) >> 31) as i32)
}

/// A flare (0xD updates) [F 0x1E010F0..0x1E01980].
fn flare(fx: &mut Fx, sp: u32, r: u32) {
    if fx.mem.read_u8(r) == 0 {
        return;
    }
    let pt = r + 0x50;
    // Sparks in two colours.
    sprite_desc(fx, sp, 0x10, None);
    camera(fx, sp);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let s = ee::mul(6.0, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), age), 12.0)));
    corners(fx, sp, s, true);
    let left = 0xC - fx.mem.i32(r + 4);
    let a = div12(fx, (left << 8) - left);
    for j in 0..0x20u32 {
        let age = fx.mem.i32(r + 4);
        let mut f22 = fx.tan_falloff_p(age, 0xC, 1);
        if j % 2 == 0 {
            f22 = ee::mul(f22, f(0x3F4C_CCCD));
            alpha(fx, sp + DESC, 2);
            rgba(fx, sp + COL, [0xFF; 3], 0);
        } else {
            alpha(fx, sp + DESC, 1);
            rgba(fx, sp + COL, [0xFF, 0x60, 0x1E], a as u8);
        }
        place(fx, sp, r + 0x60 + 0x10 * j, f22, pt);
        let c = i32::from(fx.mem.read_u16(r + 8 + 2 * j));
        cell_uvs(fx, sp, c);
        draw(fx, sp, 0x01E0_1448);
    }
    // The halo, scaled by the spread at the point.
    sprite_desc(fx, sp, 0x14, Some(1));
    fx.camera_vp_to(sp + VP);
    fx.translate_rows(sp + MAT, sp + TURN, pt);
    let age = fx.mem.i32(r + 4);
    let tf = fx.tan_falloff_p(age, 0xC, 1);
    let s = ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), tf));
    vec4(fx, sp + PT, [s, s, s, 1.0]);
    fx.scale_rows(sp + MAT, sp + PT);
    mul_mem(fx, sp + VP, sp + MAT);
    let left = ee::from_i32(0xC - fx.mem.i32(r + 4));
    let a = fix(fx, ee::mul(255.0, ee::div(left, 12.0)), 0x01E0_15A0);
    rgba(fx, sp + COL, [0x40, 0xFF, 0x1C], a as u8);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    corners(fx, sp, 45.0, false);
    draw_from(fx, sp, sp + C0, sp + DESC, 0x01E0_16AC);
    // The rays.
    sprite_desc(fx, sp, 0x14, Some(1));
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    camera(fx, sp);
    corners(fx, sp, 0.75, true);
    let age = fx.mem.i32(r + 4);
    let f22 = fx.tan_falloff_p(age, 0xC, 1);
    for m in 0..0x20u32 {
        for s6 in 0..6i32 {
            rgba(fx, sp + COL, [0xFF, 0x5A, 0x30], 0);
            let f12 = ee::add(f22, ee::mul(f(RAY_STEP), ee::from_i32(s6)));
            place(fx, sp, r + 0x260 + 0x10 * m, f12, pt);
            draw(fx, sp, 0x01E0_1904);
        }
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0xD {
        fx.mem.write_u8(r, 0);
    }
}

/// A flower (D+0x5640 + 0x60 k), a spark set (D+0x5770 + 0xA0 m) at node 1
/// and a falling quad (D+0x6180 + 0x30 c) from node 2 [F
/// 0x1E019E8..0x1E01CDC].
fn flower_spawn(fx: &mut Fx, sp: u32) {
    let k = fx.mem.i32(D + 0x35B0) as u32;
    let fb = D + 0x60 * k + FLOWERS;
    fx.mem.write_u8(fb, 1);
    fx.mem.write_u32(fb + 4, 0);
    let m = fx.mem.i32(D + 0x5760) as u32;
    let tb = D + 0xA0 * m + SPARKS;
    fx.mem.write_u8(tb, 1);
    fx.mem.write_u32(tb + 4, 0);
    let r = rnd(fx, 0x01E0_1A40);
    fx.mem.set32(fb + 8, (rem2(r, 256) << 16) >> 8);
    words(fx, fb + 0x10, D + 0x80);
    for c in 0..8u32 {
        copy8(fx, fb + 0x20 + 8 * c, D + 0xE0 + 8 * c);
    }
    words(fx, tb + 0x10, D + 0x80);
    for j in 0..8u32 {
        let r = rnd(fx, 0x01E0_1B18);
        fx.mem.set8(tb + 8 + j, rem2(r, 8));
        fx.random_table_vec(tb + 0x20 + 0x10 * j, 0.5);
    }
    let k = fx.mem.i32(D + 0x35B0) + 1;
    fx.mem.set32(D + 0x35B0, k);
    if k >= 3 {
        fx.mem.write_u32(D + 0x35B0, 0);
    }
    let m = fx.mem.i32(D + 0x5760) + 1;
    fx.mem.set32(D + 0x5760, m);
    if m >= 0x10 {
        fx.mem.write_u32(D + 0x5760, 0);
    }
    let c = fx.mem.i32(D + 0x6170) as u32;
    let g = D + 0x30 * c + QUADS;
    fx.mem.write_u8(g, 1);
    fx.mem.write_u32(g + 4, 0);
    words(fx, g + 0x10, D + 0x90);
    let r = rnd(fx, 0x01E0_1C18);
    fx.mem.set32(g + 8, (rem2(r, 256) << 16) >> 8);
    fx.random_table_vec(sp + PT, 0.5);
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    fx.mem.setf(g + 0x20, ee::mul(30.0, side));
    fx.mem.write_u32(g + 0x24, 0x41A0_0000);
    fx.mem.setf(g + 0x28, ee::mul(20.0, side));
    fx.mem.write_u32(g + 0x2C, 0x3F80_0000);
    fx.vu_add4(g + 0x20, g + 0x20, sp + PT);
    let c = fx.mem.i32(D + 0x6170) + 1;
    fx.mem.set32(D + 0x6170, c);
    if c >= 0x20 {
        fx.mem.write_u32(D + 0x6170, 0);
    }
}

fn quad_head(fx: &mut Fx, sp: u32) {
    let q = sp + QUAD;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(q, rh, 1);
    alpha(fx, q, 1);
    fx.desc_clamp(q, 0);
}

/// The flower colour and its three copies.
fn quad_colour(fx: &mut Fx, sp: u32, a: u8) {
    let c = sp + QUAD + 0x10;
    rgba(fx, c, [0x54, 0xFF, 0x30], a);
    for o in [0x18u32, 0x30, 0x48] {
        copy8(fx, c + o, c);
    }
}

/// The 8 x 16 shards of `at` (cells `cells`) through the matrix at sp +
/// VP, keyed by their mean w.
fn shards(fx: &mut Fx, sp: u32, at: u32, cells: u32, ra: u32) {
    let q = sp + QUAD;
    let mm = fx.read_m4(sp + VP);
    for i in 0..8u32 {
        for j in 0..0x10u32 {
            let cb = i32::from(fx.mem.read_u8(D + 0x10 * i + j + cells));
            let c = rem2(cb, 4) << 5;
            let r = (cb >> 2) << 5;
            let f4 = ee::from_i32(c);
            let f2 = ee::from_i32(r);
            let f5 = ee::div(ee::add(32.0, f4), 128.0);
            let f4 = ee::div(f4, 128.0);
            let f3 = ee::div(f2, 64.0);
            let f0 = ee::div(ee::add(16.0, f2), 64.0);
            for (o, x) in [(0x18u32, f4), (0x1C, f0), (0x30, f5), (0x34, f0), (0x48, f4), (0x4C, f3), (0x60, f5), (0x64, f3)] {
                fx.mem.setf(q + o, x);
            }
            let petal = D + 0x400 * i + 0x40 * j + at;
            if fx.project_desc(petal, PROJ_OUT, Some(&mm), q, sp + W) {
                let w = fx.mem.f(sp + W);
                let key = fix(fx, w, ra);
                fx.submit(q, OT, key as i32);
            }
        }
    }
}

/// The small flowers [F 0x1E01D60..0x1E022B8].
fn flowers(fx: &mut Fx, sp: u32) {
    for k in 0..3u32 {
        let fb = D + 0x60 * k + FLOWERS;
        if fx.mem.read_u8(fb) == 0 {
            continue;
        }
        let left = 3 - fx.mem.i32(fb + 4);
        let x = fx.mult(left, 0x56);
        fx.mult(0x5555_5556, x);
        let a = fx.hilo().1.wrapping_add(((x as u32) >> 31) as i32);
        quad_colour(fx, sp, a as u8);
        let age = ee::from_i32(fx.mem.i32(fb + 4));
        let s = ee::add(0.5, ee::div(ee::mul(0.5, age), 3.0));
        vec4(fx, sp + PT, [s, s, s, 1.0]);
        fx.camera_vp_to(sp + VP);
        fx.translate_rows(sp + MAT, fb + 0x20, fb + 0x10);
        let ang = fx.mem.i32(fb + 8);
        fx.rotate_axis(sp + MAT, ang, 2);
        fx.scale_rows(sp + MAT, sp + PT);
        mul_mem(fx, sp + VP, sp + MAT);
        shards(fx, sp, 0x35C0, 0x55C0, 0x01E0_1F84);
        sprite_desc(fx, sp, 0x14, Some(1));
        camera(fx, sp);
        corners(fx, sp, 45.0, true);
        let age = fx.mem.i32(fb + 4);
        fx.tan_falloff_p(age, 3, 1);
        let a = fix(fx, 0.0, 0x01E0_20E4);
        rgba(fx, sp + COL, [0xFF, 0x56, 0x14], a as u8);
        draw_far(fx, sp, fb + 0x10);
        let age = fx.mem.i32(fb + 4) + 1;
        fx.mem.set32(fb + 4, age);
        if age >= 4 {
            fx.mem.write_u8(fb, 0);
        }
    }
}

/// The spark sets [F 0x1E022C0..0x1E02600].
fn sparks(fx: &mut Fx, sp: u32) {
    sprite_desc(fx, sp, 0x10, Some(0));
    camera(fx, sp);
    for t in 0..0x10u32 {
        let tb = D + 0xA0 * t + SPARKS;
        if fx.mem.read_u8(tb) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(tb + 4));
        let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 16.0)));
        corners(fx, sp, s, true);
        rgba(fx, sp + COL, [0x80; 3], 0);
        let age = fx.mem.i32(tb + 4);
        let f20 = fx.tan_falloff_p(age, 0x10, 1);
        for j in 0..8u32 {
            place(fx, sp, tb + 0x20 + 0x10 * j, f20, tb + 0x10);
            let c = i32::from(fx.mem.read_u8(tb + 8 + j));
            cell_uvs(fx, sp, c);
            draw(fx, sp, 0x01E0_2560);
        }
        let age = fx.mem.i32(tb + 4) + 1;
        fx.mem.set32(tb + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(tb, 0);
        }
    }
}

/// The falling quads (b488's quad 0.35 wide, turned with the camera by the
/// start + age x 0xD903 about z, at the point moved age / 32 of the drift
/// with y + 10 (0.5 (-0.0109 age^2)); grey at alpha 0, the 0x100FF0 ramp
/// computed and dropped), 0x21 updates [F 0x1E02600..0x1E028D0].
fn quads(fx: &mut Fx, sp: u32) {
    let d = sp + STRIP;
    head(fx, d, 0x24, 9, 0x4_2424_2421, 0);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    alpha(fx, d, 0);
    camera(fx, sp);
    let (n, p) = (ee::neg(f(0x3EB3_3333)), f(0x3EB3_3333));
    for k in 0..0x20u32 {
        let g = D + 0x30 * k + QUADS;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let a = fix(fx, 0.0, 0x01E0_26C8);
        rgba(fx, d + 0x10, [0x80; 3], a as u8);
        for (o, (s, t)) in [(0, 0), (0x3F80_0000u32, 0), (0, 0x3F80_0000), (0x3F80_0000, 0x3F80_0000)].into_iter().enumerate() {
            fx.mem.write_u32(d + 0x18 + 0x10 * o as u32, s);
            fx.mem.write_u32(d + 0x1C + 0x10 * o as u32, t);
        }
        let c = sp + C0;
        vec4(fx, c, [n, n, 0.0, 1.0]);
        vec4(fx, c + 0x10, [p, n, 0.0, 1.0]);
        vec4(fx, c + 0x20, [n, p, 0.0, 1.0]);
        vec4(fx, c + 0x30, [p, p, 0.0, 1.0]);
        let m = sp + ROT;
        for o in 0..0x10u32 {
            let w = fx.mem.read_u32(sp + TURN + 4 * o);
            fx.mem.write_u32(m + 4 * o, w);
        }
        let age = fx.mem.i32(g + 4);
        let ang = fx.mult(age, 0xD903).wrapping_add(fx.mem.i32(g + 8));
        fx.rotate_axis(m, ang, 2);
        let age = ee::from_i32(fx.mem.i32(g + 4));
        fx.vu_scale4(sp + PT, g + 0x20, ee::div(age, 32.0));
        fx.vu_add4(sp + PT, sp + PT, g + 0x10);
        let age = ee::from_i32(fx.mem.i32(g + 4));
        let p2 = Fx::powf2(age);
        let dy = ee::mul(10.0, ee::mul(0.5, ee::mul(f(0xBC32_864D), p2)));
        let y = ee::add(fx.mem.f(sp + PT + 4), dy);
        fx.mem.setf(sp + PT + 4, y);
        for o in 0..4u32 {
            fx.vu_apply_to(c + 0x10 * o, m, c + 0x10 * o);
            fx.vu_add4(sp + V0 + 0x10 * o, c + 0x10 * o, sp + PT);
        }
        draw_from(fx, sp, sp + V0, d, 0x01E0_288C);
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// A big flower (D+0x8810 + 0x20 c) and 32 sparks (D+0x88A0 + 0x240 m) at
/// node `idx` + 3 [F 0x1E0294C..0x1E02B70].
fn big_spawn(fx: &mut Fx, idx: u32) {
    let c = fx.mem.i32(D + 0x6780) as u32;
    let fb = D + 0x20 * c + BIG_FLOWERS;
    fx.mem.write_u8(fb, 1);
    fx.mem.write_u32(fb + 4, 0);
    let m = fx.mem.i32(D + 0x8890) as u32;
    let tb = D + 0x240 * m + BIG_SPARKS;
    fx.mem.write_u8(tb, 1);
    fx.mem.write_u32(tb + 4, 0);
    let r = rnd(fx, 0x01E0_29D0);
    fx.mem.set32(fb + 8, (rem2(r, 256) << 16) >> 8);
    let node = D + 0x70 + 0x10 * (idx + 3);
    words(fx, fb + 0x10, node);
    words(fx, tb + 0x30, node);
    for j in 0..0x20u32 {
        let r = rnd(fx, 0x01E0_2A78);
        fx.mem.set8(tb + 8 + j, rem2(r, 8));
        fx.random_table_vec(tb + 0x40 + 0x10 * j, 1.0);
    }
    let c = fx.mem.i32(D + 0x6780) + 1;
    fx.mem.set32(D + 0x6780, c);
    if c >= 4 {
        fx.mem.write_u32(D + 0x6780, 0);
    }
    let m = fx.mem.i32(D + 0x8890) + 1;
    fx.mem.set32(D + 0x8890, m);
    if m >= 4 {
        fx.mem.write_u32(D + 0x8890, 0);
    }
}

/// The big flowers [F 0x1E02BF8..0x1E031A8].
fn big_flowers(fx: &mut Fx, sp: u32) {
    for k in 0..4u32 {
        let fb = D + 0x20 * k + BIG_FLOWERS;
        if fx.mem.read_u8(fb) == 0 {
            continue;
        }
        let left = 7 - fx.mem.i32(fb + 4);
        let x = fx.mult(left, 0x56);
        fx.mult(0x9249_2493u32 as i32, x);
        let a = (fx.hilo().1.wrapping_add(x) >> 2).wrapping_add(((x as u32) >> 31) as i32);
        quad_colour(fx, sp, a as u8);
        let age = ee::from_i32(fx.mem.i32(fb + 4));
        let s = ee::add(0.5, ee::div(ee::mul(0.5, age), 7.0));
        vec4(fx, sp + PT, [s, s, s, 1.0]);
        fx.camera_vp_to(sp + VP);
        fx.identity(sp + MAT);
        fx.translate_rows(sp + MAT, sp + MAT, fb + 0x10);
        fx.rotate_axis(sp + MAT, 0, 0);
        let ang = fx.mem.i32(fb + 8);
        fx.rotate_axis(sp + MAT, ang, 2);
        fx.scale_rows(sp + MAT, sp + PT);
        mul_mem(fx, sp + VP, sp + MAT);
        shards(fx, sp, 0x6790, 0x8790, 0x01E0_2E5C);
        sprite_desc(fx, sp, 0x14, Some(1));
        camera(fx, sp);
        corners(fx, sp, 50.0, true);
        let age = fx.mem.i32(fb + 4);
        fx.tan_falloff_p(age, 7, 1);
        let a = fix(fx, 0.0, 0x01E0_2FCC);
        rgba(fx, sp + COL, [0xFF, 0x64, 0x1E], a as u8);
        draw_far(fx, sp, fb + 0x10);
        let age = fx.mem.i32(fb + 4) + 1;
        fx.mem.set32(fb + 4, age);
        if age >= 8 {
            fx.mem.write_u8(fb, 0);
        }
    }
}

/// The big flowers' sparks [F 0x1E031D0..0x1E03560].
fn big_sparks(fx: &mut Fx, sp: u32) {
    sprite_desc(fx, sp, 0x10, Some(0));
    camera(fx, sp);
    for k in 0..4u32 {
        let tb = D + 0x240 * k + BIG_SPARKS;
        if fx.mem.read_u8(tb) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(tb + 4));
        let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0)));
        corners(fx, sp, s, true);
        rgba(fx, sp + COL, [0x80; 3], 0);
        let age = fx.mem.i32(tb + 4);
        let f20 = fx.tan_falloff_p(age, 0x20, 1);
        for j in 0..0x20u32 {
            place(fx, sp, tb + 0x40 + 0x10 * j, f20, tb + 0x30);
            let c = i32::from(fx.mem.read_u8(tb + 8 + j));
            cell_uvs(fx, sp, c);
            draw(fx, sp, 0x01E0_349C);
        }
        let age = fx.mem.i32(tb + 4) + 1;
        fx.mem.set32(tb + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(tb, 0);
        }
    }
}

/// A burst (D+0x91B0 + 0x10E0 c, age from 1) at the target [F
/// 0x1E035B0..0x1E038BC]: cells +8 (64 halfwords), point +0x90, 64 spark
/// vectors 1.5 +0xA0, 128 ray vectors 1.5 +0x4A0, 64 puff cells +0xCA0 and
/// vectors 1.5 +0xCE0.
fn burst_spawn(fx: &mut Fx) {
    let c = fx.mem.i32(D + 0x91A0) as u32;
    let r = D + 0x10E0 * c + BURSTS;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 1);
    fx.random_table_vec(r + 0x90, 0.5);
    fx.vu_add4(r + 0x90, r + 0x90, D + 0x20);
    for j in 0..0x40u32 {
        let b = rnd(fx, 0x01E0_36B4);
        fx.mem.write_u16(r + 8 + 2 * j, rem2(b, 8) as u16);
        fx.random_table_vec(r + 0xA0 + 0x10 * j, 1.5);
    }
    for j in 0..0x80u32 {
        fx.random_table_vec(r + 0x4A0 + 0x10 * j, 1.5);
    }
    for j in 0..0x40u32 {
        let b = rnd(fx, 0x01E0_37CC);
        let (_, h) = fx.div(b, 3);
        fx.mem.set8(r + 0xCA0 + j, h + 5);
        fx.random_table_vec(r + 0xCE0 + 0x10 * j, 1.5);
    }
    let c = fx.mem.i32(D + 0x91A0) + 1;
    fx.mem.set32(D + 0x91A0, c);
    if c >= 4 {
        fx.mem.write_u32(D + 0x91A0, 0);
    }
}

/// A burst (0x21 updates) [F 0x1E038D8..0x1E04638].
fn burst(fx: &mut Fx, sp: u32, r: u32) {
    if fx.mem.read_u8(r) == 0 {
        return;
    }
    let pt = r + 0x90;
    // Puffs.
    sprite_desc(fx, sp, 0x1C, Some(1));
    camera(fx, sp);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let s = ee::mul(8.0, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), age), 32.0)));
    corners(fx, sp, s, true);
    let left = 0x20 - fx.mem.i32(r + 4);
    let a = ((left << 8) - left) >> 5;
    rgba(fx, sp + COL, [0x80, 0xFF, 0x30], a as u8);
    let age = fx.mem.i32(r + 4);
    let f22 = fx.tan_falloff_p(age, 0x20, 1);
    for j in 0..0x40u32 {
        place(fx, sp, r + 0xCE0 + 0x10 * j, f22, pt);
        let c = i32::from(fx.mem.read_u8(r + 0xCA0 + j));
        cell_uvs(fx, sp, c);
        far_sprite(fx, sp, sp + V0);
    }
    // Sparks in two colours.
    sprite_desc(fx, sp, 0x10, None);
    camera(fx, sp);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let s = ee::mul(8.0, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), age), 32.0)));
    corners(fx, sp, s, true);
    let left = 0x20 - fx.mem.i32(r + 4);
    let a1 = ((left << 8) - left) >> 5;
    fx.mem.write_u8(sp + 0x3F0, a1 as u8);
    let a2 = (left << 7) >> 5;
    fx.mem.write_u8(sp + 0x400, a2 as u8);
    for j in 0..0x40u32 {
        let age = fx.mem.i32(r + 4);
        let mut f22 = fx.tan_falloff_p(age, 0x20, 1);
        if j % 2 == 0 {
            f22 = ee::mul(f22, f(0x3F4C_CCCD));
            alpha(fx, sp + DESC, 2);
            rgba(fx, sp + COL, [0x80, 0xFF, 0xFF], a2 as u8);
        } else {
            alpha(fx, sp + DESC, 1);
            rgba(fx, sp + COL, [0xFF, 0x5A, 0x1E], a1 as u8);
        }
        place(fx, sp, r + 0xA0 + 0x10 * j, f22, pt);
        let c = i32::from(fx.mem.read_u16(r + 8 + 2 * j));
        cell_uvs(fx, sp, c);
        far_sprite(fx, sp, sp + V0);
    }
    // The halo, scaled by the spread at the point.
    sprite_desc(fx, sp, 0x14, Some(1));
    fx.camera_vp_to(sp + VP);
    fx.translate_rows(sp + MAT, sp + TURN, pt);
    let age = fx.mem.i32(r + 4);
    let tf = fx.tan_falloff_p(age, 0x20, 1);
    let s = ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), tf));
    vec4(fx, sp + PT, [s, s, s, 1.0]);
    fx.scale_rows(sp + MAT, sp + PT);
    mul_mem(fx, sp + VP, sp + MAT);
    let left = ee::from_i32(0x20 - fx.mem.i32(r + 4));
    let a = fix(fx, ee::mul(200.0, ee::div(left, 32.0)), 0x01E0_41CC);
    rgba(fx, sp + COL, [0x5E, 0xFF, 0x3A], a as u8);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    corners(fx, sp, 120.0, false);
    far_sprite(fx, sp, sp + C0);
    // The rays.
    sprite_desc(fx, sp, 0x14, Some(1));
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    camera(fx, sp);
    corners(fx, sp, 0.75, true);
    let age = fx.mem.i32(r + 4);
    let f22 = fx.tan_falloff_p(age, 0x20, 1);
    for m in 0..0x80u32 {
        for s6 in 0..6i32 {
            rgba(fx, sp + COL, [0xFF, 0xF0, 0x80], 0);
            let f12 = ee::add(f22, ee::mul(f(RAY_STEP), ee::from_i32(s6)));
            place(fx, sp, r + 0x4A0 + 0x10 * m, f12, pt);
            draw(fx, sp, 0x01E0_459C);
        }
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(r, 0);
    }
}
