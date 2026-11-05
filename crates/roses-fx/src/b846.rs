//! BMODULE b846 (card 675), hand-ported from the record (linked at
//! 0x1E00000; the one function 0x1E00080 in a 0x630 frame with D spilled
//! to sp + 0xBC and the OT in s0, data area a0 = 0x1ED0000, 0xC9C0 bytes)
//! [F]. Written cold from the raw listing
//! (the project notes); nothing ported is above 0.36 by near.py. Slots:
//! the camera sp + 0x310 (model + 0x40, turn + 0x80), corners sp + 0x3D0 /
//! 0x3E0, vertices sp + 0x410.., a matrix sp + 0x450, points sp + 0x490 /
//! 0x4A0, the tint descriptor sp + 0x4B0, the sprite sp + 0x4E0, the lines
//! sp + 0x538, the quad sp + 0x590, the screen point sp + 0x620, the mean w
//! sp + 0x62C; the return slot sp + 0xC0. Init: textures (5, 0 / 1), the
//! target's position D+0x20 and point D+0x40, nodes 0xA9 (clear) / 0xAB /
//! 0xAD / 0xAF / 0x8B / 0x8E (D+0x60..) and 0x99 (D+0x1E0), 8 x 16 petal
//! quads (D+0x210, cells D+0x2210), rings (D+0x2D00 3 / 6, D+0xA300 6.5 /
//! 10.8 / 11), 96 ray quads D+0xA930, 128 mote cells / vectors D+0xC130 /
//! D+0xC1B0. Every update the unit flag, the camera turn, the self point
//! D+0x30 and its screen point D+0x50, the six nodes D+0x80.. and four aims
//! from D+0xC0 (D+0xE0..), node D+0x1F0; a red frame tint (D+0xC9B0) on
//! 0x87 and every 8 in 0xE7..0x10D; the rosettes (D+0x2290) from 0x53; puff
//! records (D+0x2400) on 0x54..0x118; 32 spinning fans (D+0x3120), a halo
//! (D+0x39D0) from 0x96; ray records (D+0x39E0) on 0x96..0x10D; 24 flying
//! sparks (D+0x3FF0) from 0x87, each leaving trails (D+0x42F0); the burst
//! at node D+0x1F0 (D+0xA2F0) from 0x3A; the stage light. Returns 1 on
//! 0x7D, 2 on 0x87, 4 on 0x119, 8 once the count reaches 0x16A.
use crate::b182::{alpha, head, rgba};
use crate::b440::{copy8, copy_z, fix, rem2, vec4, words};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::lens::{far_xy, FAR_Z, KEEP, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const ONE: u32 = 0x3F80_0000;
const SPR_REGS: u64 = 0x4_2421;
const QUAD_REGS: u64 = 0x4214_2142_1421;

const VP: u32 = 0x310;
const MODEL: u32 = 0x350;
const TURN: u32 = 0x390;
const C0: u32 = 0x3D0;
const C1: u32 = 0x3E0;
const V0: u32 = 0x410;
const V1: u32 = 0x420;
const M: u32 = 0x450;
const P: u32 = 0x490;
const TINT: u32 = 0x4B0;
const SPR: u32 = 0x4E0;
const LINE: u32 = 0x538;
const QUAD: u32 = 0x590;
const SCREEN: u32 = 0x620;
const MEAN: u32 = 0x62C;

const PETALS: u32 = 0x210;
const CELLS: u32 = 0x2210;
const ROSE: u32 = 0x2290;
const PUFFS: u32 = 0x2400;
const RING: u32 = 0x2D00;
const FANS: u32 = 0x3120;
const HALO: u32 = 0x39D0;
const RAYS: u32 = 0x39E0;
const SPARKS: u32 = 0x3FF0;
const TRAILS: u32 = 0x42F0;
const BURST: u32 = 0xA2F0;
const BRING: u32 = 0xA300;
const BRAYS: u32 = 0xA930;
const BCELLS: u32 = 0xC130;
const BVECS: u32 = 0xC1B0;
const FLASH: u32 = 0xC9B0;

fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x630);
    fx.mem.write_u32(sp + 0xBC, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp) };
    fx.leave(0x630);
    ret
}

/// rand >> 23 from `ra` (saved s0 = `s0`).
fn rnd(fx: &mut Fx, s0: u32, ra: u32) -> i32 {
    fx.rand_from(s0, ra) >> 23
}

/// ((r % 256) << 16) >> 8 of a rand from `ra`.
fn rand_angle(fx: &mut Fx, s0: u32, ra: u32) -> i32 {
    let r = rnd(fx, s0, ra);
    (rem2(r, 256) << 16) >> 8
}

// ------------------------------------------------------------ init

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0xC9C0);
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
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x40);
    for (at, node, clear) in [(0x60, 0xA9, 1), (0x64, 0xAB, 0), (0x68, 0xAD, 0), (0x6C, 0xAF, 0), (0x70, 0x8B, 0), (0x74, 0x8E, 0), (0x1E0, 0x99, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.write_u32(D + 0x200, 0);
    petals_init(fx, sp);
    for k in 0..0x21i32 {
        let a = (k << 16) >> 5;
        let sn = fx.tsin(a);
        let cs = fx.tcos(a);
        let p = D + RING + 0x20 * k as u32;
        for (o, r) in [(0u32, 3.0), (0x10, 6.0)] {
            fx.mem.setf(p + o, ee::mul(r, sn));
            fx.mem.setf(p + o + 4, ee::mul(r, cs));
            fx.mem.write_u32(p + o + 8, 0);
            fx.mem.write_u32(p + o + 0xC, ONE);
        }
    }
    for k in 0..0x21i32 {
        let a = (k << 16) >> 5;
        let sn = fx.tsin(a);
        let cs = fx.tcos(a);
        let p = D + BRING + 0x30 * k as u32;
        for (o, r) in [(0u32, 6.5), (0x10, f(0x412C_CCCD)), (0x20, 11.0)] {
            fx.mem.setf(p + o, ee::mul(r, sn));
            fx.mem.setf(p + o + 4, ee::mul(r, cs));
            fx.mem.write_u32(p + o + 8, 0);
            fx.mem.write_u32(p + o + 0xC, ONE);
        }
    }
    for k in 0..0x60u32 {
        let t = ee::div(ee::mul(f(0x3F66_6666), ee::from_i32(k as i32 + 1)), 96.0);
        let s = ee::add(f(0x3DCC_CCCD), t);
        let l = ee::mul(15.0, s);
        let (n, s, l) = (ee::neg(s).to_bits(), s.to_bits(), l.to_bits());
        vec4(fx, sp + C0, [0, n, 0, ONE]);
        vec4(fx, sp + C1, [0, s, 0, ONE]);
        vec4(fx, sp + 0x3F0, [0, n, l, ONE]);
        vec4(fx, sp + 0x400, [0, s, l, ONE]);
        fx.random_table_vec(sp + P, 1.0);
        fx.point_along(sp + M, sp + P);
        let a = rand_angle(fx, D + 0x40 * k, 0x01E0_075C);
        fx.rotate_axis(sp + M, a, 2);
        for c in 0..4u32 {
            fx.vu_apply_to(D + BRAYS + 0x40 * k + 0x10 * c, sp + M, sp + C0 + 0x10 * c);
        }
    }
    for k in 0..0x80u32 {
        let r = rnd(fx, D + 0x10 * k, 0x01E0_07E0);
        fx.mem.set8(D + BCELLS + k, rem2(r, 4));
        fx.random_table_vec(D + BVECS + 0x10 * k, f(0x3ECC_CCCD));
    }
    0
}

/// The 8 x 16 petal quads [F 0x1E00238..0x1E00420]: ring i of width 1.7,
/// from -6.7 i / 7 to -6.7 (i + 1) / 8 along z, aimed at (4.8 (7 - i) / 7
/// sin, cos, -6.7 i / 7) and turned about z at random; cells 4..7.
fn petals_init(fx: &mut Fx, sp: u32) {
    let c = f(0xC0D6_6666);
    for i in 0..8i32 {
        let f20 = ee::div(ee::mul(c, ee::from_i32(i + 1)), 8.0);
        let f22 = ee::div(ee::mul(c, ee::from_i32(i)), 7.0);
        let f21 = ee::div(ee::mul(f(0x4099_999A), ee::from_i32(7 - i)), 7.0);
        for j in 0..0x10i32 {
            let a = (((j << 16) >> 4) + i * 0x400) & 0xFFFF;
            let sn = fx.tsin(a);
            fx.mem.setf(sp + P, ee::mul(f21, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(sp + P + 4, ee::mul(f21, cs));
            fx.mem.setf(sp + P + 8, f22);
            fx.mem.write_u32(sp + P + 0xC, ONE);
            fx.point_along(sp + M, sp + P);
            let a = rand_angle(fx, j as u32, 0x01E0_030C);
            fx.rotate_axis(sp + M, a, 2);
            let q = D + PETALS + 0x400 * i as u32 + 0x40 * j as u32;
            let (n, p, z) = (0xBF59_999Au32, 0x3F59_999Au32, f20.to_bits());
            vec4(fx, q, [n, 0, 0, ONE]);
            vec4(fx, q + 0x10, [p, 0, 0, ONE]);
            vec4(fx, q + 0x20, [n, 0, z, ONE]);
            vec4(fx, q + 0x30, [p, 0, z, ONE]);
            for c in 0..4u32 {
                fx.vu_apply_to(q + 0x10 * c, sp + M, q + 0x10 * c);
            }
            let r = rnd(fx, j as u32, 0x01E0_03C0);
            fx.mem.set8(D + CELLS + 0x10 * i as u32 + j as u32, rem2(r, 4) + 4);
        }
    }
}

// ------------------------------------------------------------ shared parts

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The sprite's two UVs.
fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + SPR + 0x18, u0);
    uv(fx, sp + SPR + 0x1A, v0);
    uv(fx, sp + SPR + 0x28, u1);
    uv(fx, sp + SPR + 0x2A, v1);
}

/// The 4 x 8 cell `b` of the texture into the sprite's two UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    let m = |x: i32| (x & 0x3FFF) as u32;
    uvs(fx, sp, m(c << 9), m(r << 9), m(((c << 5) + 0x20) << 4), m(((r << 5) + 0x20) << 4));
}

/// The camera's view-projection times the identity.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MODEL);
    mul_model(fx, sp);
}

fn mul_model(fx: &mut Fx, sp: u32) {
    let m = fx.read_m4(sp + MODEL);
    fx.mul_into(Some(sp + VP), &m);
}

/// The camera times a translation by the point at `at`.
fn camera_at(fx: &mut Fx, sp: u32, at: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MODEL);
    fx.translate_rows(sp + MODEL, sp + MODEL, at);
    mul_model(fx, sp);
}

/// The sprite descriptor's head (prim 0x26 | 0x800).
fn sprite_head(fx: &mut Fx, sp: u32) {
    head(fx, sp + SPR, 0x26, 5, SPR_REGS, 0x800);
}

fn texture(fx: &mut Fx, d: u32, tex: u32) {
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
}

/// The quad descriptor's head, ALPHA preset 1 and `clamp`, its texture
/// D + `tex` before (`first`) or after the ALPHA.
fn quad_desc(fx: &mut Fx, sp: u32, tex: u32, first: bool, clamp: u64) {
    let q = sp + QUAD;
    head(fx, q, 0x34, 0xC, QUAD_REGS, 0);
    if first {
        texture(fx, q, tex);
    }
    alpha(fx, q, 1);
    if !first {
        texture(fx, q, tex);
    }
    fx.desc_clamp(q, clamp);
}

/// The corners (-s, -s, 0, 1) and (s, s, 0, 1), turned by the camera turn.
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    vec4(fx, sp + C0, [n.to_bits(), n.to_bits(), 0, ONE]);
    vec4(fx, sp + C1, [s.to_bits(), s.to_bits(), 0, ONE]);
    fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
    fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
}

/// The sprite's vertices about the point at `P`.
fn corners_at(fx: &mut Fx, sp: u32) {
    fx.vu_add4(sp + V0, sp + C0, sp + P);
    fx.vu_add4(sp + V1, sp + C1, sp + P);
}

fn project(fx: &mut Fx, sp: u32, input: u32, d: u32) -> bool {
    crate::fxlib2::project(fx, input, PROJ_OUT, sp + VP, d, sp + MEAN)
}

/// 0x205210 through the camera matrix.
fn project_q(fx: &mut Fx, sp: u32, input: u32) -> bool {
    let mm = fx.read_m4(sp + VP);
    fx.project_desc(input, PROJ_OUT, Some(&mm), sp + QUAD, sp + MEAN)
}

/// The sprite projected from `V0` and submitted at the mean w (0x1A2C70
/// from `ra`).
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    if project(fx, sp, sp + V0, sp + SPR) {
        submit_w(fx, sp, sp + SPR, ra);
    }
}

fn submit_w(fx: &mut Fx, sp: u32, d: u32, ra: u32) {
    let w = fx.mem.f(sp + MEAN);
    let key = fix(fx, w, ra);
    fx.submit(d, OT, key as i32);
}

/// The sprite projected from `input` and submitted at the far Z.
fn draw_far(fx: &mut Fx, sp: u32, input: u32) {
    let d = sp + SPR;
    if project(fx, sp, input, d) {
        far(fx, d + 0x20, &[d + 0x30]);
        fx.submit(d, OT, 0);
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

fn quad_far(fx: &mut Fx, sp: u32) {
    let q = sp + QUAD;
    far(fx, q + 0x20, &[q + 0x38, q + 0x50, q + 0x68]);
    fx.submit(q, OT, 0);
}

/// The quad's four colours: `a`, `a`, `b`, `b` (the second of each pair a
/// copy of the first).
fn quad_colours(fx: &mut Fx, sp: u32, a: ([u8; 3], u8), b: ([u8; 3], u8)) {
    let q = sp + QUAD;
    rgba(fx, q + 0x10, a.0, a.1);
    copy8(fx, q + 0x28, q + 0x10);
    rgba(fx, q + 0x40, b.0, b.1);
    copy8(fx, q + 0x58, q + 0x40);
}

/// The quad's four STs (s, t pairs).
fn quad_st(fx: &mut Fx, sp: u32, st: [f32; 8]) {
    let q = sp + QUAD;
    for (o, x) in [0x18u32, 0x1C, 0x30, 0x34, 0x48, 0x4C, 0x60, 0x64].into_iter().zip(st) {
        fx.mem.setf(q + o, x);
    }
}

/// age + 1 at `r` + 4, the record cleared at `end`.
fn tick(fx: &mut Fx, r: u32, end: i32) {
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= end {
        fx.mem.write_u8(r, 0);
    }
}

/// x / 24 (x 0x2AAAAAAB >> 34 + sign).
fn div24(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    let hi = fx.hilo().1;
    (hi >> 2).wrapping_add(((x as u32) >> 31) as i32)
}

fn age_f(fx: &mut Fx, r: u32) -> f32 {
    ee::from_i32(fx.mem.i32(r + 4))
}

// ------------------------------------------------------------ step

fn step(fx: &mut Fx, sp: u32) -> i32 {
    crate::b369::unit_flag(fx, 1, u64::from(OT), 0x01E0_0854);
    fx.camera_turn_roll(sp + TURN);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    for k in 0..6u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x60 + 4 * k);
        fx.node_position(h, s, D + 0x80 + 0x10 * k);
    }
    for k in 0..4u32 {
        fx.vu_sub4(sp + P, D + 0x80 + 0x10 * k, D + 0xC0);
        fx.point_along(D + 0xE0 + 0x40 * k, sp + P);
    }
    let h = fx.mem.i32(D + 4);
    let s = fx.mem.i32(D + 0x1E0);
    fx.node_position(h, s, D + 0x1F0);
    camera(fx, sp);
    screen_point(fx, sp, D + 0x30, D + 0x50);
    let n = fx.mem.i32(D);
    if n == 0x87 || ((0xE7..0x10E).contains(&n) && rem2(n, 8) == 0) {
        fx.mem.write_u8(D + FLASH, 1);
        fx.mem.write_u32(D + FLASH + 4, 0);
    }
    if fx.mem.read_u8(D + FLASH) == 1 {
        flash(fx, sp);
    }
    if fx.mem.i32(D) == 0x53 {
        rose_spawn(fx);
    }
    rose(fx, sp);
    let n = fx.mem.i32(D);
    if (0x54..0x119).contains(&n) {
        puff_spawn(fx);
    }
    puffs(fx, sp);
    if fx.mem.i32(D) == 0x96 {
        fans_spawn(fx, sp);
    }
    if fx.mem.read_u8(D + FANS) == 1 {
        fans(fx, sp);
    }
    if fx.mem.i32(D) == 0x96 {
        fx.mem.write_u8(D + HALO, 1);
        fx.mem.write_u32(D + HALO + 4, 0);
    }
    if fx.mem.read_u8(D + HALO) == 1 {
        halo(fx, sp);
    }
    let n = fx.mem.i32(D);
    if (0x96..0x10E).contains(&n) {
        let (_, i) = fx.div(n, 0x18);
        let r = D + RAYS + 0x40 * i as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for k in 0..3u32 {
            let c = rnd(fx, OT, 0x01E0_2424);
            fx.mem.set8(r + 8 + k, rem2(c, 8));
            fx.random_table_vec(r + 0x10 + 0x10 * k, 0.5);
        }
    }
    rays(fx, sp);
    if fx.mem.i32(D) == 0x87 {
        sparks_spawn(fx);
    }
    sparks(fx, sp);
    trails(fx, sp);
    if fx.mem.i32(D) == 0x3A {
        fx.mem.write_u8(D + BURST, 1);
        fx.mem.write_u32(D + BURST + 4, 0);
    }
    if fx.mem.read_u8(D + BURST) == 1 {
        burst(fx, sp);
    }
    let n = fx.mem.i32(D);
    let c1 = Fx::clamp_i(0x169 - n, 0, 0x20);
    let c2 = Fx::clamp_i(n, 0, 0x20);
    let l = (fx.mult(c1, c2.wrapping_mul(-96)) >> 0xA) + 0x80;
    fx.set_stage_light(l & 0xFF);
    let mut ret = fx.mem.i32(sp + 0xC0);
    match n {
        0x7D => ret = 1,
        0x87 => ret = 2,
        0x119 => ret = 4,
        _ => {}
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x16A {
        ret = 8;
    }
    fx.mem.set32(sp + 0xC0, ret);
    ret
}

/// The screen point of `p` into `o` (0x204D80 into sp + 0x620): x / y
/// ((x16 >> 4) - 0x700), Z (0x17BC10), 1.0, x / y clamped to 0..512.
fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    let m = fx.read_m4(sp + VP);
    fx.project_at(p, &m, sp + SCREEN, sp + MEAN);
    let x = (i32::from(fx.mem.read_u16(sp + SCREEN)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + SCREEN + 2)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + SCREEN) << 8) >> 40;
    fx.mem.setf(o + 8, ee::from_i32(z as i32));
    fx.mem.write_u32(o + 0xC, ONE);
    for k in [0u32, 4] {
        if ee_lt(fx.mem.f(o + k), 0.0) {
            fx.mem.write_u32(o + k, 0);
        }
    }
    for k in [0u32, 4] {
        if !ee_le(fx.mem.f(o + k), 512.0) {
            fx.mem.write_u32(o + k, 0x4400_0000);
        }
    }
}

/// The frame tint (D+0xC9B0) [F 0x1E00A98..0x1E00D7C]: in a packet layer
/// the frame copied as a texture (filter 0), a window of (80, 0, 0) at
/// clamp(16 - age, 0, 16) x 8, the frame over it at 0x80 less; 0x11
/// updates.
fn flash(fx: &mut Fx, sp: u32) {
    let layer = fx.layer_begin();
    let d = sp + SPR;
    head(fx, d, 0x26, 5, SPR_REGS, 0);
    // a0..a3 as 0x17BC10 left them this update (the float of the Z at
    // D+0x58: its exponent bits, mantissa with the hidden bit, the float,
    // the exponent) [run]; Z 0 not seen [H].
    let z = fx.mem.read_u32(D + 0x58);
    let a = [z & 0xFF80_0000, (z & 0x7F_FFFF) | 0x80_0000, z, z >> 23];
    let regs = crate::lens::frame_copy(fx, a, 0, sp + 0x520, sp + 0x528);
    fx.texflush(regs);
    let t = sp + TINT;
    head(fx, t, 6, 3, 0x441, 0);
    alpha(fx, t, 3);
    let age = fx.mem.i32(D + FLASH + 4);
    let a = (Fx::clamp_i(0x10 - age, 0, 0x10) << 7) >> 4;
    rgba(fx, t + 0x10, [0x80, 0, 0], a as u8);
    far_xy(fx, t + 0x18, 0x7000, 0x7000);
    far_xy(fx, t + 0x20, 0x9000, 0x9000);
    fx.submit(t, 0, 0);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let x = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, x | 0x800);
    rgba(fx, d + 0x10, [0x80; 3], 0x80u8.wrapping_sub(a as u8));
    uvs(fx, sp, 0, 0, 0x2000, 0x2000);
    far_xy(fx, d + 0x20, 0x6FF8, 0x6FF8);
    far_xy(fx, d + 0x30, 0x8FF8, 0x8FF8);
    fx.submit(d, 0, 0);
    fx.layer_end(layer, OT, 0);
    tick(fx, D + FLASH, 0x11);
}

/// On 0x53 the rosette record: four random angles (+8), the four node
/// points (+0x20) and aims (+0x60) [F 0x1E00D94..0x1E00E98].
fn rose_spawn(fx: &mut Fx) {
    let i = fx.mem.read_u32(D + 0x200);
    let r = D + ROSE + 0x160 * i;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    for k in 0..4u32 {
        let a = rand_angle(fx, OT, 0x01E0_0DE0);
        fx.mem.set32(r + 8 + 4 * k, a);
        words(fx, r + 0x20 + 0x10 * k, D + 0x80 + 0x10 * k);
        for c in 0..4u32 {
            words(fx, r + 0x60 + 0x40 * k + 0x10 * c, D + 0xE0 + 0x40 * k + 0x10 * c);
        }
    }
    let i = fx.mem.i32(D + 0x200) + 1;
    fx.mem.set32(D + 0x200, if i > 0 { 0 } else { i });
}

/// The rosettes at the first four nodes [F 0x1E00E9C..0x1E01660]: the 8 x
/// 16 petals (texture D+0x14) spun by 0x457 age about z, (FF, 58, 30) at
/// 48 ratio(age, 8) ramp(age, 198, 190) (0.3 + 0.7 (age % 2)) in soft
/// double, then a glowing sprite at each node; 0xC7 updates.
fn rose(fx: &mut Fx, sp: u32) {
    quad_desc(fx, sp, 0x14, true, 0);
    let r = D + ROSE;
    if fx.mem.read_u8(r) == 0 {
        return;
    }
    let age = age_f(fx, r);
    let f21 = ee::mul(48.0, Fx::ratio_below(age, 8.0));
    let age = age_f(fx, r);
    let rp = Fx::ramp_above(age, 198.0, 190.0);
    let s1 = f64::from(ee::mul(f21, rp));
    let age = fx.mem.i32(r + 4);
    let x = ee::mul(f(0x3F33_3333), ee::from_i32(rem2(age, 2)));
    let a = ((f64::from_bits(0x3FD3_3333_3333_3333) + f64::from(x)) * s1) as u32;
    let q = sp + QUAD;
    rgba(fx, q + 0x10, [0xFF, 0x58, 0x30], a as u8);
    for o in [0x28u32, 0x40, 0x58] {
        copy8(fx, q + o, q + 0x10);
    }
    vec4(fx, sp + P, [ONE; 4]);
    for k in 0..4u32 {
        fx.camera_vp_to(sp + VP);
        fx.translate_rows(sp + MODEL, D + 0xE0 + 0x40 * k, D + 0x80 + 0x10 * k);
        let age = fx.mem.i32(r + 4);
        let turn = fx.mult(age, 0x457);
        let a = fx.mem.i32(r + 8 + 4 * k).wrapping_add(turn);
        fx.rotate_axis(sp + MODEL, a, 2);
        fx.scale_rows(sp + MODEL, sp + P);
        mul_model(fx, sp);
        for i in 0..8u32 {
            for j in 0..0x10u32 {
                let c = i32::from(fx.mem.read_u8(D + CELLS + 0x10 * i + j));
                let f4 = ee::from_i32(rem2(c, 4) << 5);
                let f2 = ee::from_i32((c >> 2) << 5);
                let f5 = ee::div(ee::add(32.0, f4), 128.0);
                let f4 = ee::div(f4, 128.0);
                let f3 = ee::div(f2, 64.0);
                let f0 = ee::div(ee::add(16.0, f2), 64.0);
                quad_st(fx, sp, [f4, f0, f5, f0, f4, f3, f5, f3]);
                if project_q(fx, sp, D + PETALS + 0x400 * i + 0x40 * j) {
                    quad_far(fx, sp);
                }
            }
        }
    }
    sprite_head(fx, sp);
    let d = sp + SPR;
    texture(fx, d, 0x14);
    fx.desc_clamp(d, 5);
    alpha(fx, d, 1);
    camera(fx, sp);
    corners(fx, sp, 5.5);
    let age = age_f(fx, r);
    let f21 = ee::mul(64.0, Fx::ratio_below(age, 8.0));
    let age = age_f(fx, r);
    let f3 = ee::mul(f21, Fx::ramp_above(age, 198.0, 190.0));
    let age = fx.mem.i32(r + 4);
    let f2 = ee::from_i32(rem2(age, 2));
    let g = ee::add(f(0x3F19_999A), ee::mul(f(0x3ECC_CCCD), f2));
    let a = fix(fx, ee::mul(g, f3), 0x01E0_149C);
    rgba(fx, d + 0x10, [0xFF, 0x58, 0x1C], a as u8);
    for k in 0..4u32 {
        words(fx, sp + P, D + 0x80 + 0x10 * k);
        corners_at(fx, sp);
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        draw_far(fx, sp, sp + V0);
    }
    tick(fx, r, 0xC7);
}

/// A puff record: at the four node points (+0x10), a cell (+8) and a 0.5
/// vector (+0x50) each [F 0x1E016A4..0x1E017C4].
fn puff_spawn(fx: &mut Fx) {
    let i = fx.mem.read_u32(D + 0x23F0);
    let r = D + PUFFS + 0x90 * i;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    for k in 0..4u32 {
        words(fx, r + 0x10 + 0x10 * k, D + 0x80 + 0x10 * k);
        let c = rnd(fx, OT, 0x01E0_1720);
        fx.mem.set8(r + 8 + k, rem2(c, 8));
        fx.random_table_vec(r + 0x50 + 0x10 * k, 0.5);
    }
    let i = fx.mem.i32(D + 0x23F0) + 1;
    fx.mem.set32(D + 0x23F0, if i >= 0x10 { 0 } else { i });
}

/// The 16 puff records [F 0x1E017CC..0x1E01B2C]: 10 (0.5 + 0.5 age / 16)
/// wide (8A, 8A, 8A) at (16 - age) x 4, reach falloff(age, 16), keyed by
/// their w; 0x11 updates.
fn puffs(fx: &mut Fx, sp: u32) {
    let d = sp + SPR;
    sprite_head(fx, sp);
    texture(fx, d, 0x10);
    fx.desc_clamp(d, 5);
    alpha(fx, d, 0);
    camera(fx, sp);
    for m in 0..0x10u32 {
        let r = D + PUFFS + 0x90 * m;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = age_f(fx, r);
        let t = ee::div(ee::mul(0.5, age), 16.0);
        corners(fx, sp, ee::mul(10.0, ee::add(0.5, t)));
        let age = fx.mem.i32(r + 4);
        rgba(fx, d + 0x10, [0x8A; 3], (((0x10 - age) << 6) >> 4) as u8);
        let age = fx.mem.i32(r + 4);
        let f20 = fx.tan_falloff_p(age, 0x10, 1);
        for k in 0..4u32 {
            fx.vu_scale4(sp + P, r + 0x50 + 0x10 * k, f20);
            fx.vu_add4(sp + P, sp + P, r + 0x10 + 0x10 * k);
            corners_at(fx, sp);
            let c = i32::from(fx.mem.read_u8(r + 8 + k));
            cell_uvs(fx, sp, c);
            draw(fx, sp, 0x01E0_1A88);
        }
        tick(fx, r, 0x11);
    }
}

/// On 0x96 the 32 fans: cells +8, phases +0x28, random matrices +0xB0 [F
/// 0x1E01B44..0x1E01C90].
fn fans_spawn(fx: &mut Fx, sp: u32) {
    let r = D + FANS;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    for k in 0..0x20u32 {
        let c = rnd(fx, OT, 0x01E0_1B68);
        fx.mem.set8(r + 8 + k, rem2(c, 8));
        let p = rnd(fx, OT, 0x01E0_1B90);
        fx.identity(sp + M);
        fx.mem.set32(r + 0x28 + 4 * k, rem2(p, 0x20));
        for (ra, axis) in [(0x01E0_1BBC, 2), (0x01E0_1BF0, 1), (0x01E0_1C24, 0)] {
            let a = rand_angle(fx, OT, ra);
            fx.rotate_axis(sp + M, a, axis);
        }
        for c in 0..4u32 {
            words(fx, r + 0xB0 + 0x40 * k + 0x10 * c, sp + M + 0x10 * c);
        }
    }
}

/// The 32 fans at node D+0xD0 [F 0x1E01CAC..0x1E020A8]: each 32 quads of
/// the ring D+0x2D00, turned by age (33, 44, 0x22B) on its matrix, scaled
/// 0.8 + 0.2 (k + 1) / 32, (78, 4B, FF) at 128 ratio(age, 16) ramp(age,
/// 120, 104) to black, ST strip scrolled by age + phase; 0x79 updates.
fn fans(fx: &mut Fx, sp: u32) {
    let r = D + FANS;
    quad_desc(fx, sp, 0x10, true, 0);
    let age = age_f(fx, r);
    let f20 = Fx::ratio_below(age, 16.0);
    let age = age_f(fx, r);
    let f1 = ee::mul(f20, Fx::ramp_above(age, 120.0, 104.0));
    let a = fix(fx, ee::mul(128.0, f1), 0x01E0_1D64);
    quad_colours(fx, sp, ([0x78, 0x4B, 0xFF], a as u8), ([0; 3], 0));
    for k in 0..0x20u32 {
        fx.camera_vp_to(sp + VP);
        fx.identity(sp + MODEL);
        fx.translate_rows(sp + MODEL, sp + MODEL, D + 0xD0);
        for c in 0..4u32 {
            words(fx, sp + M + 0x10 * c, r + 0xB0 + 0x40 * k + 0x10 * c);
        }
        let age = fx.mem.i32(r + 4);
        fx.rotate_axis(sp + M, (age << 5) + age, 2);
        let age = fx.mem.i32(r + 4);
        fx.rotate_axis(sp + M, 44 * age, 1);
        let age = fx.mem.i32(r + 4);
        let a = fx.mult(age, 0x22B);
        fx.rotate_axis(sp + M, a, 0);
        let m = fx.read_m4(sp + M);
        fx.mul_into(Some(sp + MODEL), &m);
        let t = ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(k as i32 + 1)), 32.0);
        let s = ee::add(f(0x3F4C_CCCD), t).to_bits();
        vec4(fx, sp + P, [s, s, s, ONE]);
        fx.scale_rows(sp + MODEL, sp + P);
        mul_model(fx, sp);
        let age = fx.mem.i32(r + 4);
        let v = age.wrapping_add(fx.mem.i32(r + 0x28 + 4 * k));
        let f2 = ee::div(ee::mul(32.0, ee::from_i32(rem2(v, 0x20))), 32.0);
        let c = i32::from(fx.mem.read_u8(r + 8 + k));
        let f3 = ee::add(ee::from_i32(rem2(c, 4) << 5), f2);
        let f21 = ee::div(f3, 128.0);
        let row = ee::from_i32((c >> 2) << 5);
        let f22 = ee::div(ee::add(32.0, row), 64.0);
        let f20 = ee::div(row, 64.0);
        for s in 0..0x20u32 {
            quad_st(fx, sp, [f21, f20, f21, f20, f21, f22, f21, f22]);
            let (a, b) = (D + RING + 0x20 * s, D + RING + 0x20 * (s + 1));
            if fx.project_n(&[a, b, a + 0x10, b + 0x10], sp + VP, sp + QUAD, sp + MEAN, 0x01E0_2020) == 1 {
                submit_w(fx, sp, sp + QUAD, 0x01E0_2034);
            }
        }
    }
    tick(fx, r, 0x79);
}

/// The halo at node D+0xD0 [F 0x1E020E4..0x1E023C8]: an 8-wide sprite (78,
/// 4B, FF) at 128 (0.8 + 0.2 (age % 2)) ratio(age, 16) ramp(age, 120,
/// 104), far Z; 0x79 updates.
fn halo(fx: &mut Fx, sp: u32) {
    let (r, d) = (D + HALO, sp + SPR);
    sprite_head(fx, sp);
    texture(fx, d, 0x14);
    alpha(fx, d, 1);
    fx.desc_clamp(d, 5);
    camera_at(fx, sp, D + 0xD0);
    let age = age_f(fx, r);
    let f0 = Fx::ratio_below(age, 16.0);
    let age = fx.mem.i32(r + 4);
    let f4 = ee::from_i32(rem2(age, 2));
    let g = ee::add(f(0x3F4C_CCCD), ee::mul(f(0x3E4C_CCCD), f4));
    let f20 = ee::mul(g, f0);
    let age = age_f(fx, r);
    let f1 = ee::mul(f20, Fx::ramp_above(age, 120.0, 104.0));
    let a = fix(fx, ee::mul(128.0, f1), 0x01E0_2218);
    rgba(fx, d + 0x10, [0x78, 0x4B, 0xFF], a as u8);
    let (p, n) = (0x4100_0000u32, 0xC100_0000u32);
    vec4(fx, sp + C0, [p, p, 0, ONE]);
    vec4(fx, sp + C1, [n, n, 0, ONE]);
    fx.vu_apply_to(sp + V0, sp + TURN, sp + C0);
    fx.vu_apply_to(sp + V1, sp + TURN, sp + C1);
    uvs(fx, sp, 0, 0x200, 0x200, 0x400);
    draw_far(fx, sp, sp + V0);
    tick(fx, r, 0x79);
}

/// The 24 ray records at node D+0xD0 [F 0x1E02468..0x1E02994]: 3 rays a
/// record, each a quad a unit wide along its vector, (78, 4B, FF) at
/// clamp(n - 0x95, 0, 16) clamp(0x10E - n, 0, 16) / 256 x (24 - age) 80 /
/// 24; 0x19 updates.
fn rays(fx: &mut Fx, sp: u32) {
    let q = sp + QUAD;
    head(fx, q, 0x34, 0xC, QUAD_REGS, 0);
    alpha(fx, q, 1);
    texture(fx, q, 0x14);
    fx.desc_clamp(q, 5);
    let n = fx.mem.i32(D);
    let c1 = ee::from_i32(Fx::clamp_i(n - 0x95, 0, 0x10));
    let n = fx.mem.i32(D);
    let c2 = ee::from_i32(Fx::clamp_i(0x10E - n, 0, 0x10));
    let f20 = ee::div(ee::mul(c1, c2), 256.0);
    for m in 0..0x18u32 {
        let r = D + RAYS + 0x40 * m;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let v = div24(fx, (0x18 - age) * 80);
        let a = fix(fx, ee::mul(f20, ee::from_i32(v)), 0x01E0_25A4);
        rgba(fx, q + 0x10, [0x78, 0x4B, 0xFF], a as u8);
        for o in [0x28u32, 0x40, 0x58] {
            copy8(fx, q + o, q + 0x10);
        }
        fx.camera_vp_to(sp + VP);
        words(fx, sp + P, D + 0xD0);
        let t = [fx.mem.f(sp + P), fx.mem.f(sp + P + 4), fx.mem.f(sp + P + 8)];
        let mm = fx.trs(None, None, Some(t));
        fx.write_m4(sp + MODEL, &mm);
        mul_model(fx, sp);
        let age = fx.mem.i32(r + 4);
        let fl = fx.tan_falloff_p(age, 0x18, 1);
        let f21 = ee::mul(f(0xC060_0000), ee::add(0.5, ee::mul(0.5, fl)));
        let spin = div24(fx, (m as i32) << 16);
        for k in 0..3u32 {
            let v = r + 0x10 + 0x10 * k;
            fx.point_along(sp + M, v);
            let s2 = (k as i32) << 16;
            fx.mult(0x5555_5556, s2);
            let third = fx.hilo().1.wrapping_add(((s2 as u32) >> 31) as i32);
            fx.rotate_axis(sp + M, spin.wrapping_add(third), 2);
            vec4(fx, sp + C0, [0, 0x3F00_0000, 0, ONE]);
            vec4(fx, sp + C1, [0, 0xBF00_0000, 0, ONE]);
            fx.vu_apply_to(sp + C0, sp + M, sp + C0);
            fx.vu_apply_to(sp + C1, sp + M, sp + C1);
            fx.vu_add4(sp + V0, sp + C0, v);
            fx.vu_add4(sp + V1, sp + C1, v);
            fx.vu_scale4(sp + P, v, f21);
            fx.vu_add4(sp + 0x430, sp + V0, sp + P);
            fx.vu_add4(sp + 0x440, sp + V1, sp + P);
            let c = i32::from(fx.mem.read_u8(r + 8 + k));
            let f5 = ee::from_i32(rem2(c, 4) << 5);
            let f2 = ee::from_i32((c >> 2) << 5);
            let f6 = ee::div(f5, 128.0);
            let f4 = ee::div(ee::add(32.0, f5), 128.0);
            let f0 = ee::div(ee::add(32.0, f2), 64.0);
            let f2 = ee::div(f2, 64.0);
            quad_st(fx, sp, [f6, f2, f4, f2, f6, f0, f4, f0]);
            if project_q(fx, sp, sp + V0) {
                quad_far(fx, sp);
            }
        }
        tick(fx, r, 0x19);
    }
}

/// On 0x87 the 24 sparks (0x20 each: cell +8, step +9, trail index +0xA,
/// 1.0 vector +0x10) [F 0x1E029AC..0x1E02A28].
fn sparks_spawn(fx: &mut Fx) {
    for k in 0..0x18u32 {
        let r = D + SPARKS + 0x20 * k;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        let v = rnd(fx, OT, 0x01E0_29C4);
        let (_, c) = fx.div(v, 3);
        fx.mem.set8(r + 8, c + 1);
        let v = rnd(fx, OT, 0x01E0_29E8);
        fx.mem.set8(r + 9, (rem2(v, 2) << 1) - 1);
        fx.random_table_vec(r + 0x10, 1.0);
    }
}

/// The 24 sparks [F 0x1E02A30..0x1E02E50]: flying from the target's point
/// D+0x40 to node D+0xD0 over 15 updates bowed by sin(age 0x8000 / 15)
/// along their vector, 3 wide (78, 4B, FF) at 255 ratio(age, 7), each
/// update a trail record; 0x10 updates.
fn sparks(fx: &mut Fx, sp: u32) {
    let d = sp + SPR;
    sprite_head(fx, sp);
    texture(fx, d, 0x14);
    alpha(fx, d, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    for k in 0..0x18u32 {
        let r = D + SPARKS + 0x20 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = age_f(fx, r);
        let f22 = Fx::ratio_below(age, 7.0);
        let a = fix(fx, ee::mul(255.0, f22), 0x01E0_2B0C);
        rgba(fx, d + 0x10, [0x78, 0x4B, 0xFF], a as u8);
        let c = i32::from(fx.mem.read_u8(r + 8));
        let (col, row) = (rem2(c, 4) << 5, (c >> 2) << 5);
        let step = fx.mem.read_u8(r + 9);
        fx.mem.write_u8(r + 8, (c as u8).wrapping_add(step));
        let c = fx.mem.read_u8(r + 8);
        if c == 0 || c == 4 {
            let s = fx.mem.read_u8(r + 9);
            fx.mem.write_u8(r + 9, s.wrapping_neg());
        }
        let m = |x: i32| (x & 0x3FFF) as u32;
        uvs(fx, sp, m(col << 4), m(row << 4), m((col + 0x20) << 4), m((row + 0x20) << 4));
        corners(fx, sp, 3.0);
        let f21 = ee::div(age_f(fx, r), 15.0);
        fx.vu_sub4(sp + P, D + 0xD0, D + 0x40);
        fx.vu_scale4(sp + P, sp + P, f21);
        fx.vu_add4(sp + P, sp + P, D + 0x40);
        let x = fx.mem.i32(r + 4) << 15;
        fx.mult(0x8888_8889u32 as i32, x);
        let hi = fx.hilo().1;
        let a = (hi.wrapping_add(x) >> 3).wrapping_add(((x as u32) >> 31) as i32);
        let s = fx.tsin(a);
        fx.vu_scale4(sp + 0x4A0, r + 0x10, s);
        fx.vu_add4(sp + P, sp + P, sp + 0x4A0);
        corners_at(fx, sp);
        draw(fx, sp, 0x01E0_2D58);
        let i = u32::from(fx.mem.read_u8(r + 0xA));
        let t = D + TRAILS + 0x400 * k + 0x40 * i;
        fx.mem.write_u8(t, 1);
        fx.mem.write_u32(t + 4, 0);
        fx.mem.setf(t + 0xC, f22);
        words(fx, t + 0x10, sp + P);
        for s in 0..2u32 {
            let c = rnd(fx, OT, 0x01E0_2DB8);
            fx.mem.set8(t + 8 + s, rem2(c, 8));
            fx.random_table_vec(t + 0x20 + 0x10 * s, f(0x3F19_999A));
        }
        let i = fx.mem.read_u8(r + 0xA).wrapping_add(1);
        fx.mem.write_u8(r + 0xA, if i < 0x10 { i } else { 0 });
        tick(fx, r, 0x10);
    }
}

/// The 24 x 16 trail records [F 0x1E02E54..0x1E031E8]: two sprites a
/// record, 3.9 (0.2 + 0.8 (16 - age) / 16) wide (78, 4B, FF) at (the
/// record's alpha) 64 cos(age 0x400), reach 0.2 + 0.8 age / 16, rising 18
/// age / 16; 0x11 updates.
fn trails(fx: &mut Fx, sp: u32) {
    let d = sp + SPR;
    sprite_head(fx, sp);
    texture(fx, d, 0x10);
    alpha(fx, d, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    for i in 0..0x18u32 {
        for j in 0..0x10u32 {
            let t = D + TRAILS + 0x400 * i + 0x40 * j;
            if fx.mem.read_u8(t) == 0 {
                continue;
            }
            let age = fx.mem.i32(t + 4);
            let c = fx.tcos((age << 14) >> 4);
            let w = fx.mem.f(t + 0xC);
            let a = fix(fx, ee::mul(w, ee::mul(64.0, c)), 0x01E0_2F2C);
            rgba(fx, d + 0x10, [0x78, 0x4B, 0xFF], a as u8);
            let age = fx.mem.i32(t + 4);
            let x = ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(0x10 - age)), 16.0);
            corners(fx, sp, ee::mul(f(0x4079_999A), ee::add(f(0x3E4C_CCCD), x)));
            let age = age_f(fx, t);
            let f3 = ee::div(age, 16.0);
            let f2 = ee::div(ee::mul(f(0x3F4C_CCCD), age), 16.0);
            let f20 = ee::mul(18.0, f3);
            let f21 = ee::add(f(0x3E4C_CCCD), f2);
            for s in 0..2u32 {
                let c = i32::from(fx.mem.read_u8(t + 8 + s));
                cell_uvs(fx, sp, c);
                let v = t + 0x20 + 0x10 * s;
                let x = ee::mul(fx.mem.f(v), f21);
                fx.mem.setf(sp + P, x);
                let y = ee::mul(fx.mem.f(v + 4), f21);
                fx.mem.setf(sp + P + 4, ee::add(f20, y));
                let z = ee::mul(fx.mem.f(v + 8), f21);
                fx.mem.setf(sp + P + 8, z);
                fx.vu_add4(sp + P, sp + P, t + 0x10);
                corners_at(fx, sp);
                draw(fx, sp, 0x01E0_3180);
            }
            tick(fx, t, 0x11);
        }
    }
}

/// The burst at node D+0x1F0 [F 0x1E03244..0x1E03D90]: a 30-wide sprite
/// (FF, 20, 20) at 140 (16 - age) / 16, a ring of 32 x 2 line quads
/// (FF, 0, 0) scaled 0.3 + 0.7 falloff(age, 16), 96 ray quads (78, 78,
/// FF) and 128 motes (C5, C8, FF) 0.8 wide at 255 (16 - age) / 16; 0x11
/// updates.
fn burst(fx: &mut Fx, sp: u32) {
    let (r, d) = (D + BURST, sp + SPR);
    sprite_head(fx, sp);
    alpha(fx, d, 0);
    texture(fx, d, 0x14);
    fx.desc_clamp(d, 5);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    let (p, n) = (0x41F0_0000u32, 0xC1F0_0000u32);
    vec4(fx, sp + C0, [n, n, 0, ONE]);
    vec4(fx, sp + C1, [p, p, 0, ONE]);
    let age = fx.mem.i32(r + 4);
    let f20 = ee::div(ee::from_i32(0x10 - age), 16.0);
    let a = fix(fx, ee::mul(140.0, f20), 0x01E0_33C0);
    rgba(fx, d + 0x10, [0xFF, 0x20, 0x20], a as u8);
    fx.camera_vp_to(sp + VP);
    fx.translate_rows(sp + MODEL, sp + TURN, D + 0x1F0);
    mul_model(fx, sp);
    draw_far(fx, sp, sp + C0);
    let l = sp + LINE;
    head(fx, l, 0x14, 8, 0x4141_4141, 0);
    alpha(fx, l, 0);
    let f20 = ee::mul(255.0, f20);
    let a = fix(fx, f20, 0x01E0_3518);
    rgba(fx, l + 0x10, [0xFF, 0, 0], a as u8);
    copy8(fx, l + 0x20, l + 0x10);
    rgba(fx, l + 0x30, [0; 3], 0);
    copy8(fx, l + 0x40, l + 0x30);
    fx.camera_vp_to(sp + VP);
    fx.translate_rows(sp + MODEL, sp + TURN, D + 0x1F0);
    let age = fx.mem.i32(r + 4);
    let tf = fx.tan_falloff_p(age, 0x10, 1);
    let s = ee::add(f(0x3E99_999A), ee::mul(f(0x3F33_3333), tf)).to_bits();
    vec4(fx, sp + P, [s, s, 0, ONE]);
    fx.scale_rows(sp + MODEL, sp + P);
    mul_model(fx, sp);
    for k in 0..0x20u32 {
        let (a, b) = (D + BRING + 0x30 * k, D + BRING + 0x30 * (k + 1));
        for (o, ra) in [(0u32, 0x01E0_3660), (0x20, 0x01E0_3764)] {
            if fx.project_n(&[a + 0x10, b + 0x10, a + o, b + o], sp + VP, l, sp + MEAN, ra) == 1 {
                far(fx, l + 0x18, &[l + 0x28, l + 0x38, l + 0x48]);
                fx.submit(l, OT, 0);
            }
        }
    }
    quad_desc(fx, sp, 0x14, false, 0);
    let a = fix(fx, f20, 0x01E0_38C8);
    quad_colours(fx, sp, ([0x78, 0x78, 0xFF], a as u8), ([0x62, 0x64, 0x80], 0));
    camera_at(fx, sp, D + 0x1F0);
    for k in 0..0x60u32 {
        quad_st(fx, sp, [0.5, 0.25, 0.75, 0.25, 0.5, 0.25, 0.75, 0.25]);
        let v = D + BRAYS + 0x40 * k;
        if fx.project_n(&[v, v + 0x10, v + 0x20, v + 0x30], sp + VP, sp + QUAD, sp + MEAN, 0x01E0_39CC) == 1 {
            submit_w(fx, sp, sp + QUAD, 0x01E0_39E0);
        }
    }
    sprite_head(fx, sp);
    alpha(fx, d, 1);
    texture(fx, d, 0x14);
    fx.desc_clamp(d, 5);
    corners(fx, sp, f(0x3ECC_CCCD));
    let a = fix(fx, f20, 0x01E0_3AF0);
    rgba(fx, d + 0x10, [0xC5, 0xC8, 0xFF], a as u8);
    camera_at(fx, sp, D + 0x1F0);
    let age = fx.mem.i32(r + 4);
    let tf = fx.tan_falloff_p(age, 0x10, 1);
    let f20 = ee::add(f(0x3F4C_CCCD), ee::mul(f(0x3E4C_CCCD), tf));
    for k in 0..0x80u32 {
        let c = i32::from(fx.mem.read_u8(D + BCELLS + k));
        cell_uvs(fx, sp, c);
        fx.vu_scale4(sp + P, D + BVECS + 0x10 * k, f20);
        corners_at(fx, sp);
        draw_far(fx, sp, sp + V0);
    }
    tick(fx, r, 0x11);
}
