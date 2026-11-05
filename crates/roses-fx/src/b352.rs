//! BMODULE b352 (card 477), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xCF40
//! bytes) [F]: b358.rs (card 122) at this
//! record's frame (0x610: the upper locals 0x20 lower, the 0x2B0..0x310
//! counters 0x10 lower) with its own constants: nodes 0x32 (kind 1) / 0x30
//! (kind 0), burst base vectors 4.0; every update node 0 moved by (0, -1,
//! -2 side) and node 1 by -1.5 in y; the flash from 0x37 ((50, FF, 82),
//! band (43, FF, 64) / (18, 78, 17)); the tube from 0x4B for 0x13C updates
//! with fixed centres (0 x sin / cos: signed zeros), soft-double radii
//! ((float) ((double) (0.2 + 0.8 i / 20) x (0.8 + (double) (0.2 sin)))),
//! colour (BC, 8A, D0) at ALPHA preset 1, the lens ring 8.5 turned by fix
//! (1820 sin) about y with the centres added after the wave, lens rows
//! (8A, 80, 94); spirals 0x4C..0x185 without the sideways swing; bursts /
//! flecks 0x8C..0x185, puffs 0x4C..0x185 (1.0 vectors, 10 wide), all
//! (BC, 80, C8) at preset 1 (bursts 60 alpha, 10 wide); rises 1..0x36 at
//! node 0 (alpha 100, no lift, keyed by their mean w).
//! Returns 1 on 0x81, 6 on 0x8C..0x184, 2 on 0x185, 8 from 0x186.
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_z, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x610;
const VP: u32 = 0x310;
const MS: u32 = 0x350;
const TURN: u32 = 0x390;
const C0: u32 = 0x3D0;
const C1: u32 = 0x3E0;
const Q0: u32 = 0x410;
const Q1: u32 = 0x420;
const ROT: u32 = 0x450;
const T: u32 = 0x490;
const T2: u32 = 0x4A0;
const T3: u32 = 0x4B0;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const DESC: u32 = 0x4C8;
/// The flash band descriptor (prim 0x14, 8 registers).
const BAND: u32 = 0x520;
/// The strip descriptor (prim 0x34, 12 registers).
const STRIP: u32 = 0x578;
const W: u32 = 0x60C;

const DISC: u32 = 0xC0;
const QUADS: u32 = 0x6F0;
const MOTE_CELL: u32 = 0x1EF0;
const MOTES: u32 = 0x1F70;
const RING: u32 = 0x2770;
const GRID: u32 = 0x2880;
const LENS: u32 = 0x3ED0;
const CENTRES: u32 = 0x5520;
const TUBE: u32 = 0x5670;
const SPIRALS: u32 = 0x5680;
const BASE: u32 = 0x8180;
const BURSTS: u32 = 0x8480;
const FLECKS: u32 = 0xBF00;
const PUFFS: u32 = 0xC2C0;
const RISES: u32 = 0xC740;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

fn vec4(fx: &mut Fx, p: u32, x: [f32; 4]) {
    for (k, c) in x.iter().enumerate() {
        fx.mem.setf(p + 4 * k as u32, *c);
    }
}

fn words(fx: &mut Fx, to: u32, from: u32, n: u32) {
    for c in 0..n {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

/// 0x1A2C70 (unsigned) / 0x1A2B80 (signed) with their residue frames.
fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}
fn fixs(fx: &mut Fx, f: f32, ra: u32) -> i32 {
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}

fn rnd(fx: &mut Fx, s0: u32, ra: u32) -> i32 {
    fx.rand_from(s0, ra) >> 23
}

fn uv(fx: &mut Fx, a: u32, x: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (x & 0x3FFF) as u16);
}

/// The 32 x 32 atlas cell of byte `b` (4 across) into the sprite's UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: u32) {
    let (c, r) = (b & 3, b >> 2);
    uv(fx, sp + DESC + 0x18, c << 9);
    uv(fx, sp + DESC + 0x1A, r << 9);
    uv(fx, sp + DESC + 0x28, (32 * c + 32) << 4);
    uv(fx, sp + DESC + 0x2A, (32 * r + 32) << 4);
}

fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

/// The sprite descriptor head; texture `tex` (filter 1) then ALPHA preset
/// `preset` (`alpha_first`: the other way round), CLAMP 5.
fn head(fx: &mut Fx, sp: u32, tex: u32, preset: i32, alpha_first: bool) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + tex);
    if alpha_first {
        let a = fx.alpha_preset(preset);
        fx.desc_alpha(d, a);
        fx.desc_texture(d, rh, 1);
    } else {
        fx.desc_texture(d, rh, 1);
        let a = fx.alpha_preset(preset);
        fx.desc_alpha(d, a);
    }
    fx.desc_clamp(d, 5);
}

/// The strip descriptor head (prim 0x34, 12 registers).
fn strip_head(fx: &mut Fx, sp: u32) {
    let d = sp + STRIP;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
}

/// The camera's view-projection (x identity).
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MS);
    mul_mem(fx, sp + VP, sp + MS);
}

/// The camera's view-projection x the identity moved to `at`.
fn camera_at(fx: &mut Fx, sp: u32, at: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MS);
    fx.translate_rows(sp + MS, sp + MS, at);
    mul_mem(fx, sp + VP, sp + MS);
}

/// The camera's view-projection x the turn `m` moved to `at`.
fn turned_at(fx: &mut Fx, sp: u32, m: u32, at: u32) {
    fx.camera_vp_to(sp + VP);
    fx.translate_rows(sp + MS, m, at);
    mul_mem(fx, sp + VP, sp + MS);
}

/// The corners (a, a, 0, 1) / (b, b, 0, 1) turned by the camera.
fn corners(fx: &mut Fx, sp: u32, a: f32, b: f32) {
    vec4(fx, sp + C0, [a, a, 0.0, 1.0]);
    vec4(fx, sp + C1, [b, b, 0.0, 1.0]);
    fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
    fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
}

/// The sprite at sp + T + the corners projected and drawn keyed by its
/// mean w (`ra`) or at far Z / key 0 (`ra` None).
fn sprite(fx: &mut Fx, sp: u32, ra: Option<u32>) {
    fx.vu_add4(sp + Q0, sp + C0, sp + T);
    fx.vu_add4(sp + Q1, sp + C1, sp + T);
    draw(fx, sp, ra);
}

fn draw(fx: &mut Fx, sp: u32, ra: Option<u32>) {
    if crate::fxlib2::project(fx, sp + Q0, PROJ_OUT, sp + VP, sp + DESC, sp + W) {
        match ra {
            Some(ra) => {
                let w = fx.mem.f(sp + W);
                let key = fixu(fx, w, ra);
                fx.submit(sp + DESC, OT, key as i32);
            }
            None => {
                far_z(fx, sp + DESC + 0x20, &[sp + DESC + 0x30]);
                fx.submit(sp + DESC, OT, 0);
            }
        }
    }
}

fn clamp32(v: i32) -> f32 {
    ee::from_i32(Fx::clamp_i(v, 0, 0x20))
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0xCF40);
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
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x32, 1);
    fx.mem.set32(D + 0x20, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x30, 0);
    fx.mem.set32(D + 0x24, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x60);
    fx.mem.write_u8(D + 0xB4, 0);
    // The disc: 33 points, radii 6.5 / 10.8 / 11.
    for k in 0..0x21u32 {
        let a = ((k << 16) as i32) >> 5;
        let s = fx.tsin(a);
        let c = fx.tcos(a);
        let p = D + DISC + 0x30 * k;
        for (o, r) in [(0u32, 6.5f32), (0x10, f32::from_bits(0x412C_CCCD)), (0x20, 11.0)] {
            fx.mem.setf(p + o, ee::mul(r, s));
            fx.mem.setf(p + o + 4, ee::mul(r, c));
            fx.mem.write_u32(p + o + 8, 0);
            fx.mem.write_u32(p + o + 0xC, 0x3F80_0000);
        }
    }
    // The 96 quads.
    for k in 0..0x60u32 {
        let f2 = ee::div(ee::mul(f32::from_bits(0x3F66_6666), ee::from_i32(k as i32 + 1)), 96.0);
        let f2 = ee::add(f32::from_bits(0x3DCC_CCCD), f2);
        let f1 = ee::neg(f2);
        let f0 = ee::mul(15.0, f2);
        vec4(fx, sp + C0, [0.0, f1, 0.0, 1.0]);
        vec4(fx, sp + C1, [0.0, f2, 0.0, 1.0]);
        vec4(fx, sp + 0x3F0, [0.0, f1, f0, 1.0]);
        vec4(fx, sp + 0x400, [0.0, f2, f0, 1.0]);
        fx.random_table_vec(sp + T, 1.0);
        fx.point_along(sp + ROT, sp + T);
        let b = rnd(fx, D + 0x40 * k, 0x01E0_02E0) % 256;
        fx.rotate_axis(sp + ROT, (b << 16) >> 8, 2);
        for c in 0..4u32 {
            fx.vu_apply_to(D + QUADS + 0x40 * k + 0x10 * c, sp + ROT, sp + C0 + 0x10 * c);
        }
    }
    // The motes.
    for k in 0..0x80u32 {
        let b = rnd(fx, D + 0x10 * k, 0x01E0_035C) % 4;
        fx.mem.set8(D + MOTE_CELL + k, b);
        fx.random_table_vec(D + MOTES + 0x10 * k, f32::from_bits(0x3ECC_CCCD));
    }
    fx.mem.write_u8(D + TUBE, 0);
    fx.mem.write_u32(D + TUBE + 4, 0);
    for k in 0..0x30u32 {
        fx.random_table_vec(D + BASE + 0x10 * k, 4.0);
    }
    for k in 0..0x12u32 {
        fx.mem.write_u8(D + BURSTS + 0x340 * k, 0);
    }
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + FLECKS + 0x50 * k, 0);
    }
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + PUFFS + 0x60 * k, 0);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + RISES + 0x40 * k, 0);
    }
    0
}

/// (double) (0.2 + 0.8 i / 20) x (0.8 + (double) (0.2 sin(((i % 12) << 16) /
/// 12 - 0x8AE age))) through 0x1A2C30 / 0x1A3118 / 0x1A31D8 (left double).
fn radius(fx: &mut Fx, i: i32, age: i32) -> f64 {
    let (_, r) = fx.div(i, 12);
    let a = ((r << 16) / 12).wrapping_sub(0x8AE * age);
    let s = fx.tsin(a & 0xFFFF);
    let x = 0.8f64 + f64::from(ee::mul(f32::from_bits(0x3E4C_CCCD), s));
    let f1 = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(i)), 20.0);
    f64::from(ee::add(f32::from_bits(0x3E4C_CCCD), f1)) * x
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x20 + 4 * k));
        fx.node_position(h, slot, D + 0x30 + 0x10 * k);
    }
    let y = fx.mem.f(D + 0x34);
    fx.mem.setf(D + 0x34, ee::add(y, -1.0));
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    let z = fx.mem.f(D + 0x38);
    fx.mem.setf(D + 0x38, ee::add(z, ee::mul(-2.0, side)));
    let y = fx.mem.f(D + 0x44);
    fx.mem.setf(D + 0x44, ee::add(y, -1.5));
    if fx.mem.i32(D) == 0x37 {
        words(fx, D + 0x50, D + 0x30, 4);
    }
    fx.vu_sub4(sp + T, D + 0x60, D + 0x40);
    fx.point_along(D + 0x70, sp + T);
    fx.rotate_axis(D + 0x70, 0x4000, 0);
    let x2 = Fx::powf2(fx.mem.f(sp + T));
    let y2 = Fx::powf2(fx.mem.f(sp + T + 4));
    let f20 = ee::add(x2, y2);
    let z2 = Fx::powf2(fx.mem.f(sp + T + 8));
    let d = fx.sqrtf(ee::add(z2, f20));
    fx.mem.setf(D + 0xB0, d);
    if fx.mem.i32(D) == 0x37 {
        fx.mem.write_u8(D + 0xB4, 1);
        fx.mem.write_u32(D + 0xB8, 0);
    }
    if fx.mem.read_u8(D + 0xB4) == 1 {
        flash(fx, sp);
    }
    if fx.mem.i32(D) == 0x4B {
        fx.mem.write_u8(D + TUBE, 1);
        fx.mem.write_u32(D + TUBE + 4, 0);
    }
    let age = fx.mem.i32(D + TUBE + 4);
    let f20 = ee::div(clamp32(0x13B - age), 32.0);
    if fx.mem.read_u8(D + TUBE) == 1 {
        tube(fx, sp, f20);
    }
    let n = fx.mem.i32(D);
    if (0x4C..0x186).contains(&n) {
        spiral_spawn(fx, sp, n);
    }
    spirals(fx, sp);
    let n = fx.mem.i32(D);
    if (0x8C..0x186).contains(&n) {
        let (_, r) = fx.div(n, 0x12);
        let g = D + 0x340 * r as u32;
        fx.mem.write_u8(g + BURSTS, 1);
        fx.mem.write_u32(g + BURSTS + 4, 0);
        for j in 0..0x30u32 {
            let b = rnd(fx, OT, 0x01E0_26E0);
            fx.mem.set8(g + BURSTS + 8 + j, b % 8);
            fx.random_table_vec(sp + T, f32::from_bits(0x3E99_999A));
            fx.vu_add4(g + BURSTS + 0x40 + 0x10 * j, sp + T, D + BASE + 0x10 * j);
        }
    }
    bursts(fx, sp);
    let n = fx.mem.i32(D);
    if (0x8C..0x186).contains(&n) {
        let (_, r) = fx.div(n, 0xC);
        let g = D + 0x50 * r as u32;
        fx.mem.write_u8(g + FLECKS, 1);
        fx.mem.write_u32(g + FLECKS + 4, 0);
        for j in 0..4u32 {
            let b = rnd(fx, OT, 0x01E0_2B7C);
            fx.mem.set8(g + FLECKS + 8 + j, b % 8);
            fx.random_table_vec(g + FLECKS + 0x10 + 0x10 * j, 2.0);
        }
    }
    flecks(fx, sp);
    let n = fx.mem.i32(D);
    if (0x4C..0x186).contains(&n) {
        let (_, r) = fx.div(n, 0xC);
        let g = D + 0x60 * r as u32;
        fx.mem.write_u8(g + PUFFS, 1);
        fx.mem.write_u32(g + PUFFS + 4, 0);
        words(fx, g + PUFFS + 0x10, D + 0x40, 4);
        for j in 0..4u32 {
            let b = rnd(fx, OT, 0x01E0_2FFC);
            fx.mem.set8(g + PUFFS + 8 + j, b % 8);
            fx.random_table_vec(g + PUFFS + 0x20 + 0x10 * j, 1.0);
        }
    }
    puffs(fx, sp);
    let n = fx.mem.i32(D);
    if (1..0x37).contains(&n) {
        let g = D + 0x40 * (n % 32) as u32;
        fx.mem.write_u8(g + RISES, 1);
        fx.mem.write_u32(g + RISES + 4, 0);
        words(fx, g + RISES + 0x10, D + 0x30, 4);
        for j in 0..2u32 {
            let b = rnd(fx, OT, 0x01E0_34C0);
            fx.mem.set8(g + RISES + 8 + j, b % 8);
            fx.random_table_vec(g + RISES + 0x20 + 0x10 * j, f32::from_bits(0x3E99_999A));
        }
    }
    rises(fx, sp);
    finish(fx, sp)
}

fn finish(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x81 {
        ret = 1;
    } else {
        if (0x8C..0x185).contains(&n) {
            ret = 6;
        }
        if n == 0x185 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x187 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// The flash at node 0 (D+0xB4, 16 updates) [F 0x1E00584..0x1E010E4].
fn flash(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    uv(fx, d + 0x18, 0x400);
    uv(fx, d + 0x1A, 0);
    uv(fx, d + 0x28, 0x600);
    uv(fx, d + 0x2A, 0x200);
    vec4(fx, sp + C0, [-30.0, -30.0, 0.0, 1.0]);
    vec4(fx, sp + C1, [30.0, 30.0, 0.0, 1.0]);
    let age = fx.mem.i32(D + 0xB8);
    let f20 = ee::mul(255.0, ee::div(ee::from_i32(0x10 - age), 16.0));
    let al = fixu(fx, f20, 0x01E0_0730) as u8;
    rgba(fx, d + 0x10, [0x50, 0xFF, 0x82], al);
    turned_at(fx, sp, sp + TURN, D + 0x50);
    if crate::fxlib2::project(fx, sp + C0, PROJ_OUT, sp + VP, d, sp + W) {
        far_z(fx, d + 0x20, &[d + 0x30]);
        fx.submit(d, OT, 0);
    }

    // The disc band.
    let b = sp + BAND;
    fx.mem.write_u16(b + 2, 0x14);
    fx.mem.write_u16(b, 8);
    fx.mem.write_u64(b + 8, 0x4141_4141);
    fx.mem.write_u32(b + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(b, a);
    let al = fixu(fx, f20, 0x01E0_086C) as u8;
    rgba(fx, b + 0x10, [0x43, 0xFF, 0x64], al);
    words(fx, b + 0x20, b + 0x10, 2);
    rgba(fx, b + 0x30, [0x18, 0x78, 0x17], 0);
    words(fx, b + 0x40, b + 0x30, 2);
    fx.camera_vp_to(sp + VP);
    fx.translate_rows(sp + MS, sp + TURN, D + 0x50);
    let age = fx.mem.i32(D + 0xB8);
    let t = fx.tan_falloff_p(age, 0x10, 1);
    let s = ee::add(f32::from_bits(0x3E99_999A), ee::mul(f32::from_bits(0x3F33_3333), t));
    vec4(fx, sp + T, [s, s, 0.0, 1.0]);
    fx.scale_rows(sp + MS, sp + T);
    mul_mem(fx, sp + VP, sp + MS);
    for k in 0..0x20u32 {
        let p = D + DISC + 0x30 * k;
        let q = p + 0x30;
        for (vs, ra) in [([p + 0x10, q + 0x10, p, q], 0x01E0_099Cu32), ([p + 0x10, q + 0x10, p + 0x20, q + 0x20], 0x01E0_0AA8)] {
            if fx.project_n(&vs, sp + VP, b, sp + W, ra) == 1 {
                far_z(fx, b + 0x18, &[b + 0x28, b + 0x38, b + 0x48]);
                fx.submit(b, OT, 0);
            }
        }
    }

    // The 96 quads.
    let q = sp + STRIP;
    strip_head(fx, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(q, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 0);
    let al = fixu(fx, f20, 0x01E0_0C08) as u8;
    rgba(fx, q + 0x10, [0xFF, 0xF0, 0x96], al);
    words(fx, q + 0x28, q + 0x10, 2);
    rgba(fx, q + 0x40, [0x80, 0, 0], 0);
    words(fx, q + 0x58, q + 0x40, 2);
    camera_at(fx, sp, D + 0x50);
    for k in 0..0x60u32 {
        for (o, x) in [(0x18u32, 0.5f32), (0x1C, 0.25), (0x30, 0.75), (0x34, 0.25), (0x48, 0.5), (0x4C, 0.25), (0x60, 0.75), (0x64, 0.25)] {
            fx.mem.setf(q + o, x);
        }
        let g = D + QUADS + 0x40 * k;
        if fx.project_n(&[g, g + 0x10, g + 0x20, g + 0x30], sp + VP, q, sp + W, 0x01E0_0D00) == 1 {
            far_z(fx, q + 0x20, &[q + 0x38, q + 0x50, q + 0x68]);
            fx.submit(q, OT, 0);
        }
    }

    // The motes.
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    let k = f32::from_bits(0x3ECC_CCCD);
    corners(fx, sp, ee::neg(k), k);
    let al = fixu(fx, f20, 0x01E0_0ED4) as u8;
    rgba(fx, d + 0x10, [0xFF, 0xB4, 0x64], al);
    camera_at(fx, sp, D + 0x50);
    let age = fx.mem.i32(D + 0xB8);
    let t = fx.tan_falloff_p(age, 0x10, 1);
    let f20 = ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), t));
    for k in 0..0x80u32 {
        let b = u32::from(fx.mem.read_u8(D + MOTE_CELL + k));
        cell_uvs(fx, sp, b);
        fx.vu_scale4(sp + T, D + MOTES + 0x10 * k, f20);
        sprite(fx, sp, None);
    }
    let age = fx.mem.i32(D + 0xB8) + 1;
    fx.mem.set32(D + 0xB8, age);
    if age >= 0x11 {
        fx.mem.write_u8(D + 0xB4, 0);
    }
}

/// The ring of 17 points (D+0x2770) at radius `r`.
fn ring(fx: &mut Fx, r: f32) {
    for k in 0..0x11u32 {
        let a = (k << 12) as i32;
        let s = fx.tsin(a);
        fx.mem.setf(D + RING + 0x10 * k, ee::mul(r, s));
        fx.mem.write_u32(D + RING + 4 + 0x10 * k, 0);
        let c = fx.tcos(a);
        fx.mem.setf(D + RING + 8 + 0x10 * k, ee::mul(r, c));
        fx.mem.write_u32(D + RING + 0xC + 0x10 * k, 0x3F80_0000);
    }
}

/// The strip's four colours for row `j` (alpha `a` inside, 0 at the ends).
fn row_colours(fx: &mut Fx, sp: u32, j: u32, a: u8, c: [u8; 3]) {
    let (top, bottom) = match j {
        0 => (0, a),
        0x13 => (a, 0),
        _ => (a, a),
    };
    let d = sp + STRIP;
    rgba(fx, d + 0x10, c, top);
    words(fx, d + 0x28, d + 0x10, 2);
    rgba(fx, d + 0x40, c, bottom);
    words(fx, d + 0x58, d + 0x40, 2);
}

/// The tube [F 0x1E0114C..0x1E01E3C].
fn tube(fx: &mut Fx, sp: u32, f20: f32) {
    // 21 centre points (0 sin, 0, 0 cos, 1) of ((s6 << 16) / 20 + 1820 age).
    for s6 in 0..0x15u32 {
        let age = fx.mem.i32(D + TUBE + 4);
        let a = (((s6 << 16) as i32) / 20).wrapping_add(age.wrapping_mul(0x71C)) & 0xFFFF;
        let p = D + CENTRES + 0x10 * s6;
        let sn = fx.tsin(a);
        fx.mem.setf(p, ee::mul(0.0, sn));
        fx.mem.write_u32(p + 4, 0);
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(0.0, c));
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
    }
    let age = fx.mem.i32(D + TUBE + 4);
    let f3 = ee::from_i32(Fx::clamp_i(age, 0, 0x40));
    let f21 = ee::div(ee::mul(fx.mem.f(D + 0xB0), f3), 64.0);
    let f22 = ee::add(0.5, ee::div(ee::mul(0.5, f3), 64.0));
    let d = sp + STRIP;
    strip_head(fx, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    let f23 = ee::mul(128.0, f20);
    let a_in = fixu(fx, f23, 0x01E0_1318) as u8;
    fx.mem.write_u8(sp + 0x300, a_in);
    ring(fx, ee::mul(9.0, f22));
    fx.mem.write_u32(sp + 0x2C0, 0);
    // The textured grid.
    turned_at(fx, sp, D + 0x70, D + 0x40);
    grid(fx, f21, None);
    let age = fx.mem.i32(D + TUBE + 4);
    let (_, r) = fx.div(age, 10);
    let f20 = ee::sub(0.0, ee::div(ee::from_i32(r), 10.0));
    let mut regs = [0u32; 4];
    for j in 0..0x14u32 {
        row_colours(fx, sp, j, a_in, [0xBC, 0x8A, 0xD0]);
        let f24 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(j as i32)), 20.0), f20);
        let f25 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(j as i32 + 1)), 20.0), f20);
        let (s4, s5) = (D + 0x110 * j, D + 0x110 * (j + 1));
        for s6 in 0..0x10u32 {
            let s0 = ee::div(ee::from_i32(s6 as i32), 16.0);
            let s1 = ee::div(ee::from_i32(s6 as i32 + 1), 16.0);
            for (o, x) in [(0x18u32, s0), (0x1C, f24), (0x30, s1), (0x34, f24), (0x48, s0), (0x4C, f25), (0x60, s1), (0x64, f25)] {
                fx.mem.setf(d + o, x);
            }
            let vs = [s4 + 0x10 * s6 + GRID, s4 + 0x10 * (s6 + 1) + GRID, s5 + 0x10 * s6 + GRID, s5 + 0x10 * (s6 + 1) + GRID];
            if fx.project_n(&vs, sp + VP, d, sp + W, 0x01E0_179C) == 1 {
                let w = fx.mem.f(sp + W);
                let key = fixu(fx, w, 0x01E0_17B0);
                fx.submit(d, OT, key as i32);
                // 0x206F30 leaves a0..a3 = (desc, ot, key, desc) [run].
                regs = [d, OT, key, d];
            } else {
                // 0x100BA0 rejected: 0x205210's arguments [H].
                regs = [fx.sp - 0x40, PROJ_OUT, sp + VP, d];
            }
        }
    }
    fx.mem.write_u32(sp + 0x2C0, 1);
    // The lens grid.
    strip_head(fx, sp);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xDC, layer);
    let regs = crate::lens::frame_copy(fx, regs, 1, sp + 0x5F0, sp + 0x5F8);
    fx.texflush(regs);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, LENS_CLAMP);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a_in = fixu(fx, f23, 0x01E0_18C0) as u8;
    fx.mem.write_u8(sp + 0x2F0, a_in);
    ring(fx, ee::mul(8.5, f22));
    turned_at(fx, sp, D + 0x70, D + 0x40);
    grid(fx, f21, Some(sp));
    for j in 0..0x14u32 {
        row_colours(fx, sp, j, a_in, [0x8A, 0x80, 0x94]);
        let (s7, fp) = (D + 0x110 * j, D + 0x110 * (j + 1));
        for s6 in 0..0x10u32 {
            let (s3, s5, s2, s4) = (s7 + 0x10 * s6, s7 + 0x10 * (s6 + 1), fp + 0x10 * s6, fp + 0x10 * (s6 + 1));
            if fx.project_n(&[s3 + LENS, s5 + LENS, s2 + LENS, s4 + LENS], sp + VP, d, sp + W, 0x01E0_1DE0) != 1 {
                continue;
            }
            for k in 0..4u32 {
                fx.uv_from_xy(d + 0x18 + 0x18 * k, d + 0x20 + 0x18 * k, 0, 0);
            }
            if fx.project_n(&[s3 + GRID, s5 + GRID, s2 + GRID, s4 + GRID], sp + VP, d, sp + W, 0x01E0_1E5C) != 1 {
                continue;
            }
            if front(fx, d + 0x20, d + 0x38, d + 0x50) {
                fx.submit(d, 0, 0);
            }
        }
    }
    let layer = fx.mem.read_u32(sp + 0xDC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + TUBE + 4) + 1;
    fx.mem.set32(D + TUBE + 4, age);
    if age >= 0x13C {
        fx.mem.write_u8(D + TUBE, 0);
    }
}

/// The 21 x 17 grid D+0x2880 (rows 0x110 apart) along the aim, row y f21 i
/// / 20, about the swinging centres; with `lens` (the frame) also the copy
/// D+0x3ED0 (turned by 0 about y) with its y wave.
fn grid(fx: &mut Fx, f21: f32, lens: Option<u32>) {
    let age = fx.mem.i32(D + TUBE + 4);
    for s6 in 0..0x15u32 {
        let i = s6 as i32;
        let f20 = ee::div(ee::mul(f21, ee::from_i32(i)), 20.0);
        let f24 = radius(fx, i, age) as f32;
        let mut s5 = 0;
        if let Some(sp) = lens {
            fx.identity(sp + ROT);
            let (_, r) = fx.div(age, 0x2E);
            let a0 = (r << 16) / 46;
            s5 = (i % 8) << 13;
            let sn = fx.tsin(s5.wrapping_add(a0) & 0xFFFF);
            let a = fixs(fx, ee::mul(1820.0, sn), 0x01E0_1AF0);
            fx.rotate_axis(sp + ROT, a, 1);
        }
        let row = D + 0x110 * s6;
        let centre = D + CENTRES + 0x10 * s6;
        for s2 in 0..0x11u32 {
            let p = row + 0x10 * s2 + GRID;
            fx.vu_scale4(p, D + RING + 0x10 * s2, f24);
            fx.mem.setf(p + 4, f20);
            fx.vu_add4(p, p, centre);
            if let Some(sp) = lens {
                let q = row + 0x10 * s2 + LENS;
                fx.vu_apply_to(q, sp + ROT, p);
                let (_, r) = fx.div(age, 0x30);
                let b = (r << 16) / 48;
                let a = ((s2 as i32 % 16) << 12).wrapping_add(s5.wrapping_add(b));
                let s = fx.tsin(a & 0xFFFF);
                let f1 = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::div(f21, 20.0));
                fx.mem.setf(q + 4, ee::add(f20, ee::mul(f1, s)));
                fx.vu_add4(q, q, centre);
            }
        }
    }
}

/// The spirals' spawn [F 0x1E01F44..0x1E02284].
fn spiral_spawn(fx: &mut Fx, sp: u32, n: i32) {
    let off = 0x560 * (n % 8) as u32;
    let g = D + off;
    fx.mem.write_u8(g + SPIRALS, 1);
    fx.mem.write_u32(g + SPIRALS + 4, 0);
    fx.vu_sub4(sp + T, D + 0x60, D + 0x40);
    let age = fx.mem.i32(D + TUBE + 4);
    let f = ee::div(ee::from_i32(Fx::clamp_i(age, 0, 0x40)), 64.0);
    fx.vu_scale4(sp + T, sp + T, f);
    let age = fx.mem.i32(D + TUBE + 4);
    let f1 = ee::from_i32(Fx::clamp_i(age, 0, 0x40));
    let f21 = ee::mul(9.0, ee::add(0.5, ee::div(ee::mul(0.5, f1), 64.0)));
    for s7 in 0..0x14u32 {
        let age = fx.mem.i32(D + TUBE + 4);
        let f20 = (f64::from(f21) * radius(fx, s7 as i32, age)) as f32;
        for s5 in 0..4u32 {
            let b = rnd(fx, OT, 0x01E0_2130);
            fx.mem.set8(g + SPIRALS + 8 + 4 * s7 + s5, b % 8);
            let r = rnd(fx, OT, 0x01E0_2150);
            let f2 = ee::div(ee::mul(f32::from_bits(0x3D4C_CCCD), ee::from_i32(r % 256)), 256.0);
            let f0 = ee::div(ee::from_i32(s7 as i32), 20.0);
            fx.vu_scale4(sp + T2, sp + T, ee::add(f0, f2));
            fx.vu_add4(sp + T2, sp + T2, D + 0x40);
            let r = rnd(fx, OT, 0x01E0_21D8);
            let a = (((0x10000 * s5) >> 2) as i32).wrapping_add((r % 256) << 8) & 0xFFFF;
            let s = fx.tsin(a);
            fx.mem.setf(sp + T3, ee::mul(f20, s));
            fx.mem.write_u32(sp + T3 + 4, 0);
            let c = fx.tcos(a);
            fx.mem.setf(sp + T3 + 8, ee::mul(f20, c));
            fx.mem.write_u32(sp + T3 + 0xC, 0x3F80_0000);
            let q = g + 0x56E0 + 0x40 * s7 + 0x10 * s5;
            fx.vu_apply_to(q, D + 0x70, sp + T3);
            fx.vu_add4(q, q, sp + T2);
        }
    }
}

/// The spirals [F 0x1E021C8..0x1E025AC].
fn spirals(fx: &mut Fx, sp: u32) {
    head(fx, sp, 0x18, 1, false);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let a = clamp32(n - 0x4B);
    let n = fx.mem.i32(D);
    let b = clamp32(0x186 - n);
    let f20 = ee::div(ee::mul(a, b), 1024.0);
    for k in 0..8u32 {
        let g = D + 0x560 * k;
        if fx.mem.read_u8(g + SPIRALS) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + SPIRALS + 4);
        let s = fx.tsin((age << 15) >> 3);
        let al = fixu(fx, ee::mul(f20, ee::mul(50.0, s)), 0x01E0_23BC);
        rgba(fx, sp + DESC + 0x10, [0xBC, 0x80, 0xC8], al as u8);
        for j in 0..0x14u32 {
            let age = fx.mem.i32(g + SPIRALS + 4);
            let tf = fx.tan_falloff_p(age, 8, 1);
            let f3 = ee::mul(8.0, ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf)));
            let s = ee::mul(ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(j as i32)), 20.0)), f3);
            corners(fx, sp, s, ee::neg(s));
            for s6 in 0..4u32 {
                let b = u32::from(fx.mem.read_u8(g + SPIRALS + 8 + 4 * j + s6));
                cell_uvs(fx, sp, b);
                words(fx, sp + T, g + 0x56E0 + 0x40 * j + 0x10 * s6, 4);
                sprite(fx, sp, Some(0x01E0_25C0));
            }
        }
        let age = fx.mem.i32(g + SPIRALS + 4) + 1;
        fx.mem.set32(g + SPIRALS + 4, age);
        if age >= 9 {
            fx.mem.write_u8(g + SPIRALS, 0);
        }
    }
}

/// The bursts at the target [F 0x1E02698..0x1E02A58].
fn bursts(fx: &mut Fx, sp: u32) {
    head(fx, sp, 0x18, 1, false);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(clamp32(0x186 - n), 32.0);
    for k in 0..0x12u32 {
        let g = D + 0x340 * k + BURSTS;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(g + 4));
        let r = Fx::ramp_above(age, 18.0, 12.0);
        let al = fixu(fx, ee::mul(f20, ee::mul(60.0, r)), 0x01E0_2874);
        rgba(fx, sp + DESC + 0x10, [0xBC, 0x80, 0xC8], al as u8);
        let age = fx.mem.i32(g + 4);
        let tf = fx.tan_falloff_p(age, 0x12, 1);
        let s = ee::mul(10.0, ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf)));
        corners(fx, sp, s, ee::neg(s));
        let age = ee::from_i32(fx.mem.i32(g + 4));
        let f21 = ee::add(f32::from_bits(0x3D4C_CCCD), ee::div(age, 18.0));
        for j in 0..0x30u32 {
            let b = u32::from(fx.mem.read_u8(g + 8 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + T, g + 0x40 + 0x10 * j, f21);
            fx.vu_add4(sp + T, sp + T, D + 0x60);
            sprite(fx, sp, Some(0x01E0_2A7C));
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x13 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// The flecks at the target [F 0x1E02B08..0x1E02EAC].
fn flecks(fx: &mut Fx, sp: u32) {
    head(fx, sp, 0x1C, 1, false);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(clamp32(0x186 - n), 32.0);
    for k in 0..0xCu32 {
        let g = D + 0x50 * k + FLECKS;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 4);
        let c = fx.tsin((age << 15) / 12);
        let al = fixu(fx, ee::mul(f20, ee::mul(60.0, c)), 0x01E0_2D04);
        rgba(fx, sp + DESC + 0x10, [0xBC, 0x80, 0xC8], al as u8);
        corners(fx, sp, 18.0, -18.0);
        let age = ee::from_i32(fx.mem.i32(g + 4));
        let f22 = ee::add(f32::from_bits(0x3E99_999A), ee::div(ee::mul(f32::from_bits(0x3F33_3333), age), 12.0));
        for j in 0..4u32 {
            let b = u32::from(fx.mem.read_u8(g + 8 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + T, g + 0x10 + 0x10 * j, f22);
            fx.vu_add4(sp + T, sp + T, D + 0x60);
            sprite(fx, sp, Some(0x01E0_2ED4));
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0xD {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// The puffs at node 1 [F 0x1E02F88..0x1E03374].
fn puffs(fx: &mut Fx, sp: u32) {
    head(fx, sp, 0x18, 1, false);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(clamp32(0x186 - n), 32.0);
    for k in 0..0xCu32 {
        let g = D + 0x60 * k + PUFFS;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 4);
        let c = fx.tcos((age << 14) / 12);
        let al = fixu(fx, ee::mul(f20, ee::mul(96.0, c)), 0x01E0_3178);
        rgba(fx, sp + DESC + 0x10, [0xBC, 0x80, 0xC8], al as u8);
        let age = ee::from_i32(fx.mem.i32(g + 4));
        let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 12.0)));
        corners(fx, sp, s, ee::neg(s));
        let age = ee::from_i32(fx.mem.i32(g + 4));
        let f21 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F66_6666), age), 12.0));
        for j in 0..4u32 {
            let b = u32::from(fx.mem.read_u8(g + 8 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + T, g + 0x20 + 0x10 * j, f21);
            fx.vu_add4(sp + T, sp + T, g + 0x10);
            sprite(fx, sp, Some(0x01E0_339C));
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0xD {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// The rises at node 0 (D+0xC740, 32 updates) [F 0x1E03510..0x1E038A8]:
/// two sprites each, 3 (0.3 + 0.7 tan falloff) wide, colour (BC, 80, C8),
/// alpha 100 cos(age / 32 of a quarter), at the node + a 0.3 vector x (0.1 +
/// 0.9 age / 32), keyed by their mean w.
fn rises(fx: &mut Fx, sp: u32) {
    head(fx, sp, 0x18, 1, false);
    camera(fx, sp);
    for k in 0..0x20u32 {
        let g = D + 0x40 * k + RISES;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 4);
        let c = fx.tcos((age << 14) >> 5);
        let al = fixu(fx, ee::mul(100.0, c), 0x01E0_35E8);
        rgba(fx, sp + DESC + 0x10, [0xBC, 0x80, 0xC8], al as u8);
        let age = fx.mem.i32(g + 4);
        let tf = fx.tan_falloff_p(age, 0x20, 1);
        let s = ee::mul(3.0, ee::add(f32::from_bits(0x3E99_999A), ee::mul(f32::from_bits(0x3F33_3333), tf)));
        corners(fx, sp, s, ee::neg(s));
        let f4 = ee::from_i32(fx.mem.i32(g + 4));
        let f20 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F66_6666), f4), 32.0));
        for j in 0..2u32 {
            let b = u32::from(fx.mem.read_u8(g + 8 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + T, g + 0x20 + 0x10 * j, f20);
            fx.vu_add4(sp + T, sp + T, g + 0x10);
            sprite(fx, sp, Some(0x01E0_3814));
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// 0x101430(a, b, c) [F] (b212's, copied from b291.rs): 1 when the triangle of the XYZ2 x / y
/// halfwords at `a` / `b` / `c` faces the camera (normal z < 0).
fn front(fx: &mut Fx, a: u32, b: u32, c: u32) -> bool {
    let sp = fx.enter(0x60);
    for (k, p) in [a, b, c].into_iter().enumerate() {
        let q = sp + 0x30 + 0x10 * k as u32;
        let x = ee::from_i32(i32::from(fx.mem.read_u16(p)));
        fx.mem.setf(q, x);
        let y = ee::from_i32(i32::from(fx.mem.read_u16(p + 2)));
        fx.mem.setf(q + 4, y);
        fx.mem.write_u32(q + 8, 0);
        fx.mem.write_u32(q + 0xC, 0x3F80_0000);
    }
    let n = sp + 0x20;
    let pts = sp + 0x30;
    let fp = fx.enter(0x80);
    for k in 0..4 {
        fx.mem.write_u32(n + 4 * k, 0);
    }
    for i in 0..3u32 {
        let p0 = pts + 0x10 * (i % 3);
        let p1 = pts + 0x10 * ((i + 1) % 3);
        let p2 = pts + 0x10 * ((i + 2) % 3);
        let (e1, e2, x) = (fp + 0x50, fp + 0x60, fp + 0x70);
        words(fx, e1, 0x0029_3070, 4);
        for k in 0..3u32 {
            let v = ee::sub(fx.mem.f(p1 + 4 * k), fx.mem.f(p0 + 4 * k));
            fx.mem.setf(e1 + 4 * k, v);
        }
        words(fx, e2, 0x0029_3080, 4);
        for k in 0..3u32 {
            let v = ee::sub(fx.mem.f(p2 + 4 * k), fx.mem.f(p1 + 4 * k));
            fx.mem.setf(e2 + 4 * k, v);
        }
        crate::blib::quadtrail::cross3(fx, x, e1, e2);
        for k in 0..3u32 {
            let v = ee::add(fx.mem.f(n + 4 * k), fx.mem.f(x + 4 * k));
            fx.mem.setf(n + 4 * k, v);
        }
    }
    for k in 0..3u32 {
        let v = ee::div(fx.mem.f(n + 4 * k), 3.0);
        fx.mem.setf(n + 4 * k, v);
    }
    crate::blib::trail::normalize3(fx, n);
    fx.leave(0x80);
    let r = crate::exe::ee_lt(fx.mem.f(n + 8), 0.0);
    fx.leave(0x60);
    r
}
