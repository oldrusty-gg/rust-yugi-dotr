//! BMODULE b389 (card 508), hand-ported from the record (linked at
//! 0x1E00000; the entry 0x1E00080 in a 0x470 frame, data area a0 =
//! 0x1ED0000 kept at sp + 0xF0, 0xAEB0 bytes; the dome callback 0x1E02150
//! with its static D at 0x1E02A80) [F]: the third
//! member of b507's family (cards 532 / 484 in b440.rs) with its parts in
//! another order and other shapes. The init lays out a flat 32 x 17 disc
//! of radius 30 (32 - i) / 32 at D+0x3580, then a flower of 8 rings x 16
//! petals (D+0x7990, 360 long, 3 wide, the rings turned by the unit's
//! side) with cells at D+0x9990. Every update the camera turn with roll
//! (sp + 0x2B0), the attacker's point (D+0x20) and eight nodes 3 / 8 /
//! 0x20 / 6 / 0xA / 0xB / 0xF / 0xE (D+0x60..). On 0x9F..0x12B one of 12
//! burst records (D+0xF0, 0x460) at the target's point (D+0x30: 32 sparks,
//! even ones white at alpha preset 2 and 0.8 out, odd ones (60, FF, 3C)
//! at preset 1, both at far Z; a far-Z halo (FF, 40, 1C) and 32 rays of
//! six sprites (FF, 5A, 30)); from update 0 for 0x12D updates (D+0x3570 /
//! 0x3574) the attacker darkened and drawn into its target with the dome
//! (b440's callback at the attacker's point, ramp 16 / 300..284 with a
//! 0.1 + 0.9 |sin| pulse at alpha 128, radius 30, wobble 1820); on n % 3
//! == 0 within 0x2D..0x87 a flower pair (D+0x9A10, 0x30 each) at two of
//! the nodes (one of 0..3 and one of 4..7) with a spark pair (D+0x9AB0,
//! 0x140, (58, 58, 58)). The stage light 0x80 - 64 clamp(n) clamp(0x158 -
//! n) / 1024. Returns 1 on 0x94, 6 on 0x9F..0x12B, 2 on 0x12C, 8 once the
//! count reaches 0x159.
//!
//! b766 (card 510, [`program_b766`]; entry frame 0x5A0, D at sp + 0x100,
//! 0xBB80 bytes, dome 0x1E02B30 / static 0x1E03400) is b296's member
//! (card 484) on these parts: the target's position (D+0x20) and the
//! attacker's point (D+0x30) with its window pixel / Z (D+0x40..), nodes
//! 0x1F (D+0x60) and 0xA / 0x25 (D+0x80 / 0x90); bursts D+0xB0 on
//! 0x85..0x120 at the target (keyed, the odd sparks (7E, 55, FF), the halo
//! (4A, 1C, FF) at 255 (12 - age) / 12, rays (FF, 62, 20)); single flowers
//! (D+0x55C0, 0x60, the identity as their matrix) with 16-spark records
//! (D+0x56F0, 0x130) at node 0x1F on n % 4 == 0 within 0x81..0x120; the
//! pass from update 0 for 0x31 updates (dome at the attacker's point, ramp
//! 48 / 24, no pulse, radius 45); then motes (24 x 0x60 at D+0xAE00, four
//! each, 0x123..0x16D at node 0x1F, rising) and two fountains (2 x 0x240
//! at D+0xB700, 32 sparks thrown up, on 0x1B at node 0x25 / 0x43 at node
//! 0xA). Light clamp 0x40 (80 / 4096), returns 1 / 6 / 2 / 4 / 8 on 0x7A /
//! 0x85..0x120 / 0x121 / 0x16E / 0x1C7.
use crate::b440::{copy8, copy_z, div12, dome, fix, rem2, tint, vec4, words, Dome, Pulse};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
const S0: u32 = OT;

/// A record's working slots on its stack: the camera matrix (with the
/// model matrix + 0x40, the turn + 0x80, the corners + 0xC0 / 0xD0, the
/// vertices + 0xE0 / 0xF0 and the placed point + 0x140), the sprite
/// descriptor (colour + 0x10, UVs + 0x18 / + 0x28, XYZ + 0x20 / + 0x30,
/// TEX0 / TEX1 + 0x40 / 0x48; the quad one + 0x58) and the mean w (the
/// tint colour + 4).
struct Fr {
    vp: u32,
    model: u32,
    turn: u32,
    c0: u32,
    c1: u32,
    v0: u32,
    v1: u32,
    pl: u32,
    d: u32,
    q: u32,
    mean: u32,
}

impl Fr {
    fn new(sp: u32, vp: u32, d: u32, mean: u32) -> Fr {
        let vp = sp + vp;
        let d = sp + d;
        Fr { vp, model: vp + 0x40, turn: vp + 0x80, c0: vp + 0xC0, c1: vp + 0xD0, v0: vp + 0xE0, v1: vp + 0xF0, pl: vp + 0x140, d, q: d + 0x58, mean: sp + mean }
    }
}

const DOME: Dome = Dome {
    d_static: 0x01E0_2A80,
    cam: 0x20,
    timer: 0x3574,
    dome: 0x5780,
    disc: 0x3580,
    ratio: Some(16.0),
    ramp: (300.0, 284.0),
    pulse: Pulse::Abs,
    alpha: 128.0,
    radius: 30.0,
    wobble: 1820.0,
    ra: [0x01E0_23D0, 0x01E0_257C, 0x01E0_2670, 0x01E0_26D4, 0x01E0_2814, 0x01E0_2890],
};

const DOME766: Dome = Dome {
    d_static: 0x01E0_3400,
    cam: 0x30,
    timer: 0x69F4,
    dome: 0xCC00,
    disc: 0x6A00,
    ratio: None,
    ramp: (48.0, 24.0),
    pulse: Pulse::AbsDropped,
    alpha: 255.0,
    radius: 45.0,
    wobble: 5461.0,
    ra: [0x01E0_2D7C, 0x01E0_2F2C, 0x01E0_3048, 0x01E0_30AC, 0x01E0_31FC, 0x01E0_3278],
};

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x470);
    let f = Fr::new(sp, 0x230, 0x380, 0x468);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, &f) };
    fx.leave(0x470);
    ret
}

/// b766 (card 510).
pub fn program_b766(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x5A0);
    let f = Fr::new(sp, 0x350, 0x4A8, 0x598);
    let ret = if param >= 0 { init766(fx, sp) } else { step766(fx, sp, &f) };
    fx.leave(0x5A0);
    ret
}

// ------------------------------------------------------------ shared parts

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, f: &Fr, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, f.d + 0x18, u0);
    uv(fx, f.d + 0x1A, v0);
    uv(fx, f.d + 0x28, u1);
    uv(fx, f.d + 0x2A, v1);
}

/// The 4 x 8 cell `b` of the texture into the sprite's two UVs.
fn cell_uvs(fx: &mut Fx, f: &Fr, b: u32) {
    let (c, r) = (b & 3, b >> 2);
    uvs(fx, f, (c << 9) & 0x3FFF, (r << 9) & 0x3FFF, (((c << 5) + 0x20) << 4) & 0x3FFF, (((r << 5) + 0x20) << 4) & 0x3FFF);
}

fn camera(fx: &mut Fx, f: &Fr) {
    fx.camera_vp_to(f.vp);
    fx.identity(f.model);
    let id = fx.read_m4(f.model);
    fx.mul_into(Some(f.vp), &id);
}

/// The sprite descriptor rewritten (prim 0x26 | 0x800).
fn desc_head(fx: &mut Fx, f: &Fr) {
    let d = f.d;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let x = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, x | 0x800);
}

/// The sprite descriptor rewritten, its texture, clamp 5 and (with
/// `preset`) its ALPHA.
fn desc_reset(fx: &mut Fx, f: &Fr, rh_at: u32, preset: Option<i32>) {
    desc_head(fx, f);
    let rh = fx.mem.read_u32(D + rh_at);
    fx.desc_texture(f.d, rh, 1);
    fx.desc_clamp(f.d, 5);
    if let Some(p) = preset {
        let a = fx.alpha_preset(p);
        fx.desc_alpha(f.d, a);
    }
}

/// The corners (-s, -s, 0, 1) and (s, s, 0, 1) (turned by the camera turn
/// with `turn`).
fn corners(fx: &mut Fx, f: &Fr, s: f32, turn: bool) {
    let n = ee::neg(s);
    fx.mem.setf(f.c0, n);
    fx.mem.setf(f.c0 + 4, n);
    fx.mem.write_u32(f.c0 + 8, 0);
    fx.mem.write_u32(f.c0 + 0xC, 0x3F80_0000);
    fx.mem.setf(f.c1, s);
    fx.mem.setf(f.c1 + 4, s);
    fx.mem.write_u32(f.c1 + 8, 0);
    fx.mem.write_u32(f.c1 + 0xC, 0x3F80_0000);
    if turn {
        fx.vu_apply_to(f.c0, f.turn, f.c0);
        fx.vu_apply_to(f.c1, f.turn, f.c1);
    }
}

fn colour(fx: &mut Fx, f: &Fr, c: [u8; 3], a: u8) {
    fx.mem.write_u8(f.d + 0x10, c[0]);
    fx.mem.write_u8(f.d + 0x11, c[1]);
    fx.mem.write_u8(f.d + 0x12, c[2]);
    fx.mem.write_u8(f.d + 0x13, a);
    fx.mem.write_u32(f.d + 0x14, 0x3F80_0000);
}

/// The placed point (= dir x s + at, its y + `dy`) and the sprite's
/// vertices around it.
fn place_dy(fx: &mut Fx, f: &Fr, dir: u32, s: f32, at: u32, dy: Option<f32>) {
    fx.vu_scale4(f.pl, dir, s);
    fx.vu_add4(f.pl, f.pl, at);
    if let Some(dy) = dy {
        let y = ee::add(fx.mem.f(f.pl + 4), dy);
        fx.mem.setf(f.pl + 4, y);
    }
    fx.vu_add4(f.v0, f.c0, f.pl);
    fx.vu_add4(f.v1, f.c1, f.pl);
}

fn place(fx: &mut Fx, f: &Fr, dir: u32, s: f32, at: u32) {
    place_dy(fx, f, dir, s, at, None);
}

fn project(fx: &mut Fx, f: &Fr, input: u32) -> bool {
    crate::fxlib2::project(fx, input, PROJ_OUT, f.vp, f.d, f.mean)
}

/// Projected from `input` and submitted at the mean w.
fn draw(fx: &mut Fx, f: &Fr, input: u32, ra: u32) {
    if project(fx, f, input) {
        let w = fx.mem.f(f.mean);
        let key = fix(fx, w, ra);
        fx.submit(f.d, OT, key as i32);
    }
}

/// Projected from `input` and submitted at key 0.
fn draw0(fx: &mut Fx, f: &Fr, input: u32) {
    if project(fx, f, input) {
        fx.submit(f.d, OT, 0);
    }
}

/// Projected from `input` and submitted at far Z (key 0).
fn draw_far(fx: &mut Fx, f: &Fr, input: u32) {
    if project(fx, f, input) {
        let x = fx.mem.read_u64(f.d + 0x20);
        fx.mem.write_u64(f.d + 0x20, (x & KEEP) | FAR_Z);
        copy_z(fx, f.d + 0x30, f.d + 0x20);
        fx.submit(f.d, OT, 0);
    }
}

/// A flower's petals: 8 rings x 16 at `petals` with cells at `cells`,
/// the quad's colour set, through the camera matrix.
fn petals(fx: &mut Fx, f: &Fr, petals: u32, cells: u32, ra: u32) {
    let q = f.q;
    let mm = fx.read_m4(f.vp);
    for s6 in 0..8u32 {
        for s5 in 0..0x10u32 {
            let cb = i32::from(fx.mem.read_u8(D + 0x10 * s6 + s5 + cells));
            let c = rem2(cb, 4) << 5;
            let r = (cb >> 2) << 5;
            let f4 = ee::from_i32(c);
            let f2 = ee::from_i32(r);
            let f5 = ee::div(ee::add(32.0, f4), 128.0);
            let f4 = ee::div(f4, 128.0);
            let f3 = ee::div(f2, 64.0);
            let f0 = ee::div(ee::add(16.0, f2), 64.0);
            fx.mem.setf(q + 0x18, f4);
            fx.mem.setf(q + 0x1C, f0);
            fx.mem.setf(q + 0x30, f5);
            fx.mem.setf(q + 0x34, f0);
            fx.mem.setf(q + 0x48, f4);
            fx.mem.setf(q + 0x4C, f3);
            fx.mem.setf(q + 0x60, f5);
            fx.mem.setf(q + 0x64, f3);
            let petal = D + 0x400 * s6 + 0x40 * s5 + petals;
            if fx.project_desc(petal, PROJ_OUT, Some(&mm), q, f.mean) {
                let w = fx.mem.f(f.mean);
                let key = fix(fx, w, ra);
                fx.submit(q, OT, key as i32);
            }
        }
    }
}

/// The flower quad descriptor (prim 0x34, 12 registers).
fn quad_head(fx: &mut Fx, f: &Fr) {
    let q = f.q;
    fx.mem.write_u16(q + 2, 0x34);
    fx.mem.write_u16(q, 0xC);
    fx.mem.write_u64(q + 8, 0x4214_2142_1421);
    fx.mem.write_u32(q + 4, 0);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(q, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(q, a);
    fx.desc_clamp(q, 0);
}

/// The flower's colour (alpha 0x56 (3 - age) / 3) into the quad's four
/// vertices and its scale 0.5 + age / 6 into the placed point.
fn flower_colour(fx: &mut Fx, f: &Fr, fb: u32, c: [u8; 3]) {
    let left = 3 - fx.mem.i32(fb + 4);
    let x = fx.mult(left, 0x56);
    fx.mult(0x5555_5556, x);
    let hi = fx.hilo().1;
    let a = hi.wrapping_add(((x as u32) >> 31) as i32);
    let q = f.q;
    fx.mem.write_u8(q + 0x10, c[0]);
    fx.mem.write_u8(q + 0x11, c[1]);
    fx.mem.write_u8(q + 0x12, c[2]);
    fx.mem.write_u8(q + 0x13, a as u8);
    fx.mem.write_u32(q + 0x14, 0x3F80_0000);
    copy8(fx, q + 0x28, q + 0x10);
    copy8(fx, q + 0x40, q + 0x10);
    copy8(fx, q + 0x58, q + 0x10);
    let age = ee::from_i32(fx.mem.i32(fb + 4));
    let s = ee::add(0.5, ee::div(ee::mul(0.5, age), 3.0));
    fx.mem.setf(f.pl, s);
    fx.mem.setf(f.pl + 4, s);
    fx.mem.setf(f.pl + 8, s);
    fx.mem.write_u32(f.pl + 0xC, 0x3F80_0000);
}

/// A flower's far-Z sprites at its `n` points (+0x10).
fn flower_far(fx: &mut Fx, f: &Fr, fb: u32, n: u32, c: [u8; 3], ra: u32) {
    desc_reset(fx, f, 0x14, Some(1));
    camera(fx, f);
    corners(fx, f, 45.0, true);
    let age = fx.mem.i32(fb + 4);
    fx.tan_falloff_p(age, 3, 1);
    let a = fix(fx, 0.0, ra);
    colour(fx, f, c, a as u8);
    for s in 0..n {
        words(fx, f.pl, fb + 0x10 + 0x10 * s);
        fx.vu_add4(f.v0, f.c0, f.pl);
        fx.vu_add4(f.v1, f.c1, f.pl);
        uvs(fx, f, 0x600, 0, 0x800, 0x200);
        draw_far(fx, f, f.v0);
    }
    let age = fx.mem.i32(fb + 4) + 1;
    fx.mem.set32(fb + 4, age);
    if age >= 4 {
        fx.mem.write_u8(fb, 0);
    }
}

/// The burst's sprite size 12 / 6 x (0.8 + 0.2 age / 12) at age `r` + 4.
fn burst_corners(fx: &mut Fx, f: &Fr, r: u32, size: f32) {
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let s = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), age), 12.0);
    let s = ee::mul(size, ee::add(f32::from_bits(0x3F4C_CCCD), s));
    corners(fx, f, s, true);
}

/// The burst's halo matrix: the camera at the point scaled by 0.2 + 0.8
/// falloff.
fn halo_matrix(fx: &mut Fx, f: &Fr, r: u32) {
    desc_reset(fx, f, 0x14, Some(1));
    fx.camera_vp_to(f.vp);
    fx.translate_rows(f.model, f.turn, r + 0x50);
    let age = fx.mem.i32(r + 4);
    let f0 = fx.tan_falloff_p(age, 0xC, 1);
    let s = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
    fx.mem.setf(f.pl, s);
    fx.mem.setf(f.pl + 4, s);
    fx.mem.setf(f.pl + 8, s);
    fx.mem.write_u32(f.pl + 0xC, 0x3F80_0000);
    fx.scale_rows(f.model, f.pl);
    let mm = fx.read_m4(f.model);
    fx.mul_into(Some(f.vp), &mm);
}

/// 32 rays of six sprites from the burst's point, `step` apart.
fn rays(fx: &mut Fx, f: &Fr, r: u32, size: f32, c: [u8; 3], step: f32, ra: u32) {
    desc_reset(fx, f, 0x14, Some(1));
    uvs(fx, f, 0x600, 0, 0x800, 0x200);
    camera(fx, f);
    corners(fx, f, size, true);
    let age = fx.mem.i32(r + 4);
    let f22 = fx.tan_falloff_p(age, 0xC, 1);
    for m in 0..0x20u32 {
        let dir = r + 0x260 + 0x10 * m;
        for s6 in 0..6i32 {
            colour(fx, f, c, 0);
            let f12 = ee::add(f22, ee::mul(step, ee::from_i32(s6)));
            place(fx, f, dir, f12, r + 0x50);
            draw(fx, f, f.v0, ra);
        }
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0xD {
        fx.mem.write_u8(r, 0);
    }
}

/// A burst record (0x460) started at D+at: point `p` out, 32 sparks `s`
/// out, 32 rays `l` out.
fn burst_spawn(fx: &mut Fx, idx: u32, base: u32, at: u32, len: [f32; 3], ra: u32) {
    let b = fx.mem.i32(D + idx) as u32;
    let r = D + 0x460 * b + base;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 1);
    fx.random_table_vec(r + 0x50, len[0]);
    fx.vu_add4(r + 0x50, r + 0x50, D + at);
    for j in 0..0x20u32 {
        let c = fx.rand_from(S0, ra) >> 23;
        fx.mem.write_u16(r + 8 + 2 * j, rem2(c, 8) as u16);
        fx.random_table_vec(r + 0x60 + 0x10 * j, len[1]);
    }
    for j in 0..0x20u32 {
        fx.random_table_vec(r + 0x260 + 0x10 * j, len[2]);
    }
    let b = fx.mem.i32(D + idx) + 1;
    fx.mem.set32(D + idx, b);
    if b >= 0xC {
        fx.mem.write_u32(D + idx, 0);
    }
}

/// Spark records: `n` sets of `per` sparks (cells +8 + 8 s, points +p +
/// 0x10 s, vectors +v + 0x80 s), `size` x (0.5 + age / 32), grey `g`.
#[allow(clippy::too_many_arguments)]
fn spark_records(fx: &mut Fx, f: &Fr, base: u32, rec: u32, n: u32, per: u32, p: u32, v: u32, size: f32, g: u8, ra: u32) {
    desc_reset(fx, f, 0x10, Some(0));
    camera(fx, f);
    for ti in 0..0x10u32 {
        let tb = D + rec * ti + base;
        if fx.mem.read_u8(tb) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(tb + 4));
        let s = ee::mul(size, ee::add(0.5, ee::div(ee::mul(0.5, age), 16.0)));
        corners(fx, f, s, true);
        colour(fx, f, [g, g, g], 0);
        let age = fx.mem.i32(tb + 4);
        let f20 = fx.tan_falloff_p(age, 0x10, 1);
        for s in 0..n {
            for s6 in 0..per {
                place(fx, f, tb + v + 0x80 * s + 0x10 * s6, f20, tb + p + 0x10 * s);
                let c = fx.mem.read_u8(tb + 8 + 8 * s + s6);
                cell_uvs(fx, f, u32::from(c));
                draw(fx, f, f.v0, ra);
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
/// by a full-window sprite; the key from the depth of D + `at`.
fn unit_pass(fx: &mut Fx, f: &Fr, dm: &Dome, cb: u32, at: u32, ra: [u32; 2]) {
    let c = f.mean + 4;
    for (k, b) in [0u8, 0, 0, 0x80].into_iter().enumerate() {
        fx.mem.write_u8(c + k as u32, b);
    }
    tint(fx, 1, c, ra[0]);
    let d = f.d;
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
    fx.mem.write_u32(dm.d_static, D);
    // TEX0 / TEX1 of the target straight into the sprite's slots.
    let (_, regs) = fx.unit_draw_post(1, d + 0x40, d + 0x48, cb, &mut |fx, p| dome(fx, p, dm));
    fx.texflush(regs);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 0x0000_0800_0080_000A);
    let x = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, x | 0x800);
    for k in 0..4 {
        fx.mem.write_u8(d + 0x10 + k, 0x80);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    uvs(fx, f, 0, 0, 0x2000, 0x2000);
    for (at, xy) in [(d + 0x20, 0x6FF8u16), (d + 0x30, 0x8FF8)] {
        fx.mem.write_u16(at, xy);
        fx.mem.write_u16(at + 2, xy);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & KEEP) | FAR_Z);
        fx.mem.write_u8(at + 7, 0);
    }
    fx.submit(d, 0, 0);
    let w = crate::blib::afterimage::depth_w(fx, D + at, 0.0);
    fx.mem.setf(f.mean, w);
    let key = fix(fx, w, ra[1]);
    fx.layer_end(layer, OT, key as i32);
}

/// A flat 32 x 17 disc of radius `r` (32 - i) / 32 at D + `at`.
fn disc(fx: &mut Fx, at: u32, r: f32) {
    for i in 0..0x20i32 {
        let f20 = ee::div(ee::mul(r, ee::from_i32(0x20 - i)), 32.0);
        let row = D + 0x110 * i as u32 + at;
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
}

/// The flower of 8 rings x 16 petals (`len` long, `w` half wide at the
/// base, the rings `width` x (7 - ring) / 7 out and turned by the unit's
/// side) at D + `at`, cells at D + `cells`; rand's s0 the angle (`s0a`)
/// or the petal index.
#[allow(clippy::too_many_arguments)]
fn flower_init(fx: &mut Fx, m: u32, vv: u32, at: u32, cells: u32, len: f32, w: u32, width: f32, s0a: bool, ra: [u32; 2]) {
    for s7 in 0..8i32 {
        let f20 = ee::div(ee::mul(len, ee::from_i32(s7 + 1)), 8.0);
        let f1 = ee::div(ee::mul(ee::neg(len), ee::from_i32(s7)), 7.0);
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let f22 = ee::mul(f1, side);
        let f21 = ee::div(ee::mul(width, ee::from_i32(8 - (s7 + 1))), 7.0);
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
            let seed = if s0a { a as u32 } else { s0 as u32 };
            let r = fx.rand_from(seed, ra[0]) >> 23;
            fx.rotate_axis(m, (rem2(r, 256) << 16) >> 8, 2);
            let q = ring + 0x40 * s0 as u32 + at;
            let (n, p, one) = (w | 0x8000_0000, w, 0x3F80_0000u32);
            vec4(fx, q, [n, 0, 0, one]);
            vec4(fx, q + 0x10, [p, 0, 0, one]);
            vec4(fx, q + 0x20, [n, 0, f20.to_bits(), one]);
            vec4(fx, q + 0x30, [p, 0, f20.to_bits(), one]);
            for k in 0..4u32 {
                fx.vu_apply_to(q + 0x10 * k, m, q + 0x10 * k);
            }
            let r = fx.rand_from(seed, ra[1]) >> 23;
            fx.mem.set8(D + 0x10 * s7 as u32 + s0 as u32 + cells, rem2(r, 4) + 4);
        }
    }
}

/// The base of init: the counter, handles, side and textures.
fn init_head(fx: &mut Fx, size: u32) {
    fx.bzero(D, size);
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
}

fn request(fx: &mut Fx, node: i32, clear: i32, at: u32) {
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, node, clear);
    fx.mem.set32(at, r);
}

fn node(fx: &mut Fx, slot: u32, out: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(slot));
    fx.node_position(h, s, out);
}

/// The stage light 0x80 + (clamp(len - n) x -(k clamp(n)) >> sh) and the
/// counter step.
fn light(fx: &mut Fx, len: i32, hi: i32, k: i32, sh: u32) {
    let n = fx.mem.i32(D);
    let c1 = Fx::clamp_i(len - n, 0, hi);
    let c2 = Fx::clamp_i(n, 0, hi);
    let l = fx.mult(c1, c2.wrapping_mul(k).wrapping_neg());
    fx.set_stage_light(((l >> sh) + 0x80) & 0xFF);
}

// ------------------------------------------------------------ b389

const BURST_IDX: u32 = 0xE0;
const BURST: u32 = 0xF0;
const PASS: u32 = 0x3570;
const FLOWER_IDX: u32 = 0x7980;
const PETALS: u32 = 0x7990;
const CELLS: u32 = 0x9990;
const FLOWER: u32 = 0x9A10;
const SPARK_IDX: u32 = 0x9AA0;
const SPARKS: u32 = 0x9AB0;
const NODES: [i32; 8] = [3, 8, 0x20, 6, 0xA, 0xB, 0xF, 0xE];

fn init(fx: &mut Fx, sp: u32) -> i32 {
    init_head(fx, 0xAEB0);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x30);
    for (k, &n) in NODES.iter().enumerate() {
        request(fx, n, i32::from(k == 0), D + 0x40 + 4 * k as u32);
    }
    fx.mem.write_u32(D + BURST_IDX, 0);
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + BURST + 0x460 * k, 0);
    }
    fx.mem.write_u8(D + PASS, 0);
    disc(fx, 0x3580, 30.0);
    fx.mem.write_u32(D + FLOWER_IDX, 0);
    flower_init(fx, sp + 0x330, sp + 0x370, PETALS, CELLS, 360.0, 0x4040_0000, 3.0, true, [0x01E0_03F4, 0x01E0_04A0]);
    for k in 0..3u32 {
        fx.mem.write_u8(D + FLOWER + 0x30 * k, 0);
    }
    fx.mem.write_u32(D + SPARK_IDX, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + SPARKS + 0x140 * k, 0);
    }
    0
}

/// The bursts [F b389 0x1E007BC].
fn bursts(fx: &mut Fx, f: &Fr) {
    for bi in 0..0xCu32 {
        let r = D + 0x460 * bi + BURST;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let d = f.d;
        desc_reset(fx, f, 0x10, None);
        camera(fx, f);
        burst_corners(fx, f, r, 12.0);
        let left = 0xC - fx.mem.i32(r + 4);
        let odd_a = div12(fx, left.wrapping_mul(255)) & 0xFF;
        let even_a = div12(fx, left << 7) & 0xFF;
        for j in 0..0x20u32 {
            let age = fx.mem.i32(r + 4);
            let mut f22 = fx.tan_falloff_p(age, 0xC, 1);
            if j & 1 == 0 {
                f22 = ee::mul(f22, f32::from_bits(0x3F4C_CCCD));
                let a = fx.alpha_preset(2);
                fx.desc_alpha(d, a);
                colour(fx, f, [0xFF, 0xFF, 0xFF], even_a as u8);
            } else {
                let a = fx.alpha_preset(1);
                fx.desc_alpha(d, a);
                colour(fx, f, [0x60, 0xFF, 0x3C], odd_a as u8);
            }
            place(fx, f, r + 0x60 + 0x10 * j, f22, r + 0x50);
            let c = fx.mem.read_u16(r + 8 + 2 * j);
            cell_uvs(fx, f, u32::from(c));
            draw_far(fx, f, f.v0);
        }
        halo_matrix(fx, f, r);
        colour(fx, f, [0xFF, 0x40, 0x1C], 0);
        uvs(fx, f, 0x600, 0, 0x800, 0x200);
        corners(fx, f, 90.0, false);
        draw_far(fx, f, f.c0);
        rays(fx, f, r, 1.5, [0xFF, 0x5A, 0x30], f32::from_bits(0x3C23_D70A), 0x01E0_10AC);
    }
}

/// A flower pair and a spark pair started [F b389 0x1E01408].
fn flower_spawn(fx: &mut Fx) {
    let k = fx.mem.i32(D + FLOWER_IDX) as u32;
    let fb = D + 0x30 * k + FLOWER;
    fx.mem.write_u8(fb, 1);
    fx.mem.write_u32(fb + 4, 0);
    let m = fx.mem.i32(D + SPARK_IDX) as u32;
    let tb = D + 0x140 * m + SPARKS;
    fx.mem.write_u8(tb, 1);
    fx.mem.write_u32(tb + 4, 0);
    for s in 0..2u32 {
        let r = fx.rand_from(S0, 0x01E0_1494) >> 23;
        let node = 4 * s + rem2(r, 4) as u32;
        let r = fx.rand_from(S0, 0x01E0_14B8) >> 23;
        fx.mem.set32(fb + 8 + 4 * s, (rem2(r, 256) << 16) >> 8);
        let at = D + 0x60 + 0x10 * node;
        words(fx, fb + 0x10 + 0x10 * s, at);
        words(fx, tb + 0x20 + 0x10 * s, at);
        for s6 in 0..8u32 {
            let r = fx.rand_from(S0, 0x01E0_1560) >> 23;
            fx.mem.set8(tb + 8 + 8 * s + s6, rem2(r, 8));
            fx.random_table_vec(tb + 0x40 + 0x80 * s + 0x10 * s6, 1.0);
        }
    }
    let k = fx.mem.i32(D + FLOWER_IDX) + 1;
    fx.mem.set32(D + FLOWER_IDX, k);
    if k >= 3 {
        fx.mem.write_u32(D + FLOWER_IDX, 0);
    }
    let m = fx.mem.i32(D + SPARK_IDX) + 1;
    fx.mem.set32(D + SPARK_IDX, m);
    if m >= 0x10 {
        fx.mem.write_u32(D + SPARK_IDX, 0);
    }
}

/// The flowers [F b389 0x1E01668].
fn flowers(fx: &mut Fx, f: &Fr) {
    quad_head(fx, f);
    for fi in 0..3u32 {
        let fb = D + 0x30 * fi + FLOWER;
        if fx.mem.read_u8(fb) == 0 {
            continue;
        }
        flower_colour(fx, f, fb, [0x40, 0xFF, 0x30]);
        for s in 0..2u32 {
            fx.camera_vp_to(f.vp);
            fx.identity(f.model);
            fx.translate_rows(f.model, f.model, fb + 0x10 + 0x10 * s);
            let side = fx.mem.i32(D + 0xC);
            fx.rotate_axis(f.model, side.wrapping_mul(0x18E3), 0);
            let ang = fx.mem.i32(fb + 8 + 4 * s);
            fx.rotate_axis(f.model, ang, 2);
            fx.scale_rows(f.model, f.pl);
            let mm = fx.read_m4(f.model);
            fx.mul_into(Some(f.vp), &mm);
            petals(fx, f, PETALS, CELLS, 0x01E0_1954);
        }
        flower_far(fx, f, fb, 2, [0xFF, 0x4C, 0x14], 0x01E0_1AB4);
    }
}

fn step(fx: &mut Fx, f: &Fr) -> i32 {
    fx.camera_turn_roll(f.turn);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x20);
    for k in 0..NODES.len() as u32 {
        node(fx, D + 0x40 + 4 * k, D + 0x60 + 0x10 * k);
    }
    let n = fx.mem.i32(D);
    if (0x9F..0x12C).contains(&n) {
        burst_spawn(fx, BURST_IDX, BURST, 0x30, [3.0, 1.0, 3.5], 0x01E0_06B0);
    }
    bursts(fx, f);
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + PASS, 1);
        fx.mem.write_u32(D + PASS + 4, 0);
    }
    tint(fx, 0, 0, 0x01E0_1184);
    if fx.mem.read_u8(D + PASS) == 1 {
        unit_pass(fx, f, &DOME, 0x01E0_2150, 0x20, [0x01E0_11BC, 0x01E0_1380]);
        let t = fx.mem.i32(D + PASS + 4) + 1;
        fx.mem.set32(D + PASS + 4, t);
        if t >= 0x12D {
            fx.mem.write_u8(D + PASS, 0);
            tint(fx, 0, 0, 0x01E0_13CC);
        }
    }
    let n = fx.mem.i32(D);
    if n % 3 == 0 && (0x2D..0x88).contains(&n) {
        flower_spawn(fx);
    }
    flowers(fx, f);
    spark_records(fx, f, SPARKS, 0x140, 2, 8, 0x20, 0x40, 10.0, 0x58, 0x01E0_1F74);
    light(fx, 0x158, 0x20, 64, 10);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x94 {
        ret = 1;
    } else {
        if (0x9F..0x12C).contains(&n) {
            ret = 6;
        }
        if n == 0x12C {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x159 {
        ret = 8;
    }
    ret
}

// ------------------------------------------------------------ b766

const B7_IDX: u32 = 0xA0;
const B7: u32 = 0xB0;
const F7_IDX: u32 = 0x3530;
const P7: u32 = 0x3540;
const C7: u32 = 0x5540;
const F7: u32 = 0x55C0;
const S7_IDX: u32 = 0x56E0;
const S7: u32 = 0x56F0;
const PASS7: u32 = 0x69F0;
const MOTES: u32 = 0xAE00;
const FOUNT: u32 = 0xB700;

fn init766(fx: &mut Fx, sp: u32) -> i32 {
    init_head(fx, 0xBB80);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    request(fx, 0x1F, 1, D + 0x50);
    request(fx, 0xA, 0, D + 0x70);
    request(fx, 0x25, 0, D + 0x74);
    fx.mem.write_u32(D + B7_IDX, 0);
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + B7 + 0x460 * k, 0);
    }
    fx.mem.write_u32(D + F7_IDX, 0);
    flower_init(fx, sp + 0x450, sp + 0x490, P7, C7, 140.0, 0x3F80_0000, 2.0, false, [0x01E0_02C4, 0x01E0_0368]);
    for k in 0..3u32 {
        fx.mem.write_u8(D + F7 + 0x60 * k, 0);
    }
    fx.mem.write_u32(D + S7_IDX, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + S7 + 0x130 * k, 0);
    }
    fx.mem.write_u8(D + PASS7, 0);
    disc(fx, 0x6A00, 45.0);
    let c = sp + 0x59C;
    for (k, b) in [0u8, 0, 0, 0x80].into_iter().enumerate() {
        fx.mem.write_u8(c + k as u32, b);
    }
    tint(fx, 1, c, 0x01E0_04E8);
    for k in 0..0x18u32 {
        fx.mem.write_u8(D + MOTES + 0x60 * k, 0);
    }
    for k in 0..2u32 {
        fx.mem.write_u8(D + FOUNT + 0x240 * k, 0);
    }
    0
}

/// The attacker's window pixel (D+0x40 / 0x44, clamped to 0..512) and Z
/// (D+0x48) [F b766 0x1E005BC..0x1E006D8].
fn screen_point(fx: &mut Fx, f: &Fr, sp: u32) {
    camera(fx, f);
    let m = fx.read_m4(f.vp);
    fx.project_at(D + 0x30, &m, sp + 0x590, sp + 0x598);
    let x = (i32::from(fx.mem.read_u16(sp + 0x590)) >> 4) - 0x700;
    fx.mem.setf(D + 0x40, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x592)) >> 4) - 0x700;
    fx.mem.setf(D + 0x44, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x590) << 8) >> 40;
    fx.mem.setf(D + 0x48, ee::from_i32(z as i32));
    fx.mem.write_u32(D + 0x4C, 0x3F80_0000);
    for o in [0x40u32, 0x44] {
        if crate::exe::ee_lt(fx.mem.f(D + o), 0.0) {
            fx.mem.write_u32(D + o, 0);
        }
    }
    for o in [0x40u32, 0x44] {
        if !crate::exe::ee_le(fx.mem.f(D + o), 512.0) {
            fx.mem.write_u32(D + o, 0x4400_0000);
        }
    }
}

/// The bursts [F b766 0x1E008D4].
fn bursts766(fx: &mut Fx, f: &Fr) {
    for bi in 0..0xCu32 {
        let r = D + 0x460 * bi + B7;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let d = f.d;
        desc_reset(fx, f, 0x10, None);
        camera(fx, f);
        burst_corners(fx, f, r, 6.0);
        let left = 0xC - fx.mem.i32(r + 4);
        let odd_a = div12(fx, left.wrapping_mul(255)) & 0xFF;
        for j in 0..0x20u32 {
            let age = fx.mem.i32(r + 4);
            let mut f22 = fx.tan_falloff_p(age, 0xC, 1);
            if j & 1 == 0 {
                f22 = ee::mul(f22, f32::from_bits(0x3F4C_CCCD));
                let a = fx.alpha_preset(2);
                fx.desc_alpha(d, a);
                colour(fx, f, [0xFF, 0xFF, 0xFF], 0);
            } else {
                let a = fx.alpha_preset(1);
                fx.desc_alpha(d, a);
                colour(fx, f, [0x7E, 0x55, 0xFF], odd_a as u8);
            }
            place(fx, f, r + 0x60 + 0x10 * j, f22, r + 0x50);
            let c = fx.mem.read_u16(r + 8 + 2 * j);
            cell_uvs(fx, f, u32::from(c));
            draw(fx, f, f.v0, 0x01E0_0C60);
        }
        halo_matrix(fx, f, r);
        let left = ee::from_i32(0xC - fx.mem.i32(r + 4));
        let a = fix(fx, ee::mul(255.0, ee::div(left, 12.0)), 0x01E0_0DB8);
        colour(fx, f, [0x4A, 0x1C, 0xFF], a as u8);
        uvs(fx, f, 0x600, 0, 0x800, 0x200);
        corners(fx, f, 45.0, false);
        draw(fx, f, f.c0, 0x01E0_0EBC);
        rays(fx, f, r, 0.75, [0xFF, 0x62, 0x20], f32::from_bits(0x3BE5_6042), 0x01E0_1110);
    }
}

/// A flower and a spark record at node 0x1F [F b766 0x1E011E8].
fn flower_spawn766(fx: &mut Fx, sp: u32) {
    let k = fx.mem.i32(D + F7_IDX) as u32;
    let fb = D + 0x60 * k + F7;
    fx.mem.write_u8(fb, 1);
    fx.mem.write_u32(fb + 4, 0);
    let m = fx.mem.i32(D + S7_IDX) as u32;
    let tb = D + 0x130 * m + S7;
    fx.mem.write_u8(tb, 1);
    fx.identity(sp + 0x450);
    fx.mem.write_u32(tb + 4, 0);
    let r = fx.rand_from(S0, 0x01E0_1264) >> 23;
    fx.mem.set32(fb + 8, (rem2(r, 256) << 16) >> 8);
    words(fx, fb + 0x10, D + 0x60);
    for c in 0..8u32 {
        copy8(fx, fb + 0x20 + 8 * c, sp + 0x450 + 8 * c);
    }
    words(fx, tb + 0x20, D + 0x60);
    for s6 in 0..0x10u32 {
        let r = fx.rand_from(S0, 0x01E0_1338) >> 23;
        fx.mem.set8(tb + 8 + s6, rem2(r, 8));
        fx.random_table_vec(tb + 0x30 + 0x10 * s6, 1.0);
    }
    let k = fx.mem.i32(D + F7_IDX) + 1;
    fx.mem.set32(D + F7_IDX, k);
    if k >= 3 {
        fx.mem.write_u32(D + F7_IDX, 0);
    }
    let m = fx.mem.i32(D + S7_IDX) + 1;
    fx.mem.set32(D + S7_IDX, m);
    if m >= 0x10 {
        fx.mem.write_u32(D + S7_IDX, 0);
    }
}

/// The flowers [F b766 0x1E013F0].
fn flowers766(fx: &mut Fx, f: &Fr) {
    quad_head(fx, f);
    for fi in 0..3u32 {
        let fb = D + 0x60 * fi + F7;
        if fx.mem.read_u8(fb) == 0 {
            continue;
        }
        flower_colour(fx, f, fb, [0x7C, 0x30, 0xFF]);
        fx.camera_vp_to(f.vp);
        fx.translate_rows(f.model, fb + 0x20, fb + 0x10);
        let ang = fx.mem.i32(fb + 8);
        fx.rotate_axis(f.model, ang, 2);
        fx.scale_rows(f.model, f.pl);
        let mm = fx.read_m4(f.model);
        fx.mul_into(Some(f.vp), &mm);
        petals(fx, f, P7, C7, 0x01E0_1684);
        flower_far(fx, f, fb, 1, [0xFF, 0x7E, 0x14], 0x01E0_17EC);
    }
}

/// Four motes at node 0x1F into record n % 24 [F b766 0x1E01F9C].
fn motes_spawn(fx: &mut Fx, n: i32) {
    let e = D + 0x60 * (n % 0x18) as u32 + MOTES;
    fx.mem.write_u8(e, 1);
    fx.mem.write_u32(e + 4, 0);
    words(fx, e + 0x10, D + 0x60);
    for s6 in 0..4u32 {
        let r = fx.rand_from(S0, 0x01E0_2028) >> 23;
        fx.mem.set8(e + 8 + s6, rem2(r, 8));
        fx.random_table_vec(e + 0x20 + 0x10 * s6, f32::from_bits(0x3E99_999A));
    }
}

/// The motes: 5 x (0.5 + age / 48) wide, (0.2 + 0.8 age / 24) out and
/// 20 age / 24 up, 25 updates [F b766 0x1E020A0].
fn motes(fx: &mut Fx, f: &Fr) {
    desc_head(fx, f);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(f.d, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(f.d, rh, 1);
    fx.desc_clamp(f.d, 5);
    camera(fx, f);
    for k in 0..0x18u32 {
        let e = D + 0x60 * k + MOTES;
        if fx.mem.read_u8(e) == 0 {
            continue;
        }
        let x = fx.mem.i32(e + 4) << 15;
        fx.mult(0x2AAA_AAAB, x);
        let hi = fx.hilo().1;
        fx.tsin((hi >> 2).wrapping_add(((x as u32) >> 31) as i32));
        let a = fix(fx, 0.0, 0x01E0_21A0);
        colour(fx, f, [0xEE, 0xEE, 0xEE], a as u8);
        let age = ee::from_i32(fx.mem.i32(e + 4));
        let s = ee::mul(5.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 24.0)));
        corners(fx, f, s, true);
        let age = ee::from_i32(fx.mem.i32(e + 4));
        let f2 = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), age), 24.0);
        let up = ee::div(ee::mul(20.0, age), 24.0);
        let out = ee::add(f32::from_bits(0x3E4C_CCCD), f2);
        for s6 in 0..4u32 {
            place_dy(fx, f, e + 0x20 + 0x10 * s6, out, e + 0x10, Some(up));
            let c = fx.mem.read_u8(e + 8 + s6);
            cell_uvs(fx, f, u32::from(c));
            draw0(fx, f, f.v0);
        }
        let age = fx.mem.i32(e + 4) + 1;
        fx.mem.set32(e + 4, age);
        if age >= 0x19 {
            fx.mem.write_u8(e, 0);
        }
    }
}

/// A fountain (32 sparks thrown up, 0.8 |y|) at node 0xA / 0x25 [F b766
/// 0x1E024B0].
fn fountain_spawn(fx: &mut Fx, k: u32) {
    let g = D + 0x240 * k + FOUNT;
    fx.mem.write_u8(g, 1);
    fx.mem.write_u32(g + 4, 0);
    words(fx, g + 0x30, D + 0x80 + 0x10 * k);
    for s6 in 0..0x20u32 {
        let r = fx.rand_from(S0, 0x01E0_2570) >> 23;
        fx.mem.set8(g + 8 + s6, rem2(r, 8));
        let v = g + 0x40 + 0x10 * s6;
        fx.random_table_vec(v, f32::from_bits(0x3F4C_CCCD));
        let y = fx.mem.f(v + 4);
        fx.mem.write_f32(fx.sp - 0x10, y);
        let y = f32::from_bits(y.to_bits() & 0x7FFF_FFFF);
        fx.mem.setf(v + 4, y);
        let y = ee::mul(fx.mem.f(v + 4), f32::from_bits(0x3F4C_CCCD));
        fx.mem.setf(v + 4, y);
    }
}

/// The fountains: 7 x (0.5 + age / 64) wide, alpha 64 cos, 5 down, 33
/// updates [F b766 0x1E02638].
fn fountains(fx: &mut Fx, f: &Fr) {
    desc_head(fx, f);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(f.d, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(f.d, rh, 1);
    fx.desc_clamp(f.d, 5);
    camera(fx, f);
    for k in 0..2u32 {
        let g = D + 0x240 * k + FOUNT;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 4);
        let c = fx.tcos((age << 14) >> 5);
        let a = fix(fx, ee::mul(64.0, c), 0x01E0_271C);
        colour(fx, f, [0xEE, 0xEE, 0xEE], a as u8);
        let age = ee::from_i32(fx.mem.i32(g + 4));
        let s = ee::mul(7.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0)));
        corners(fx, f, s, true);
        let age = fx.mem.i32(g + 4);
        let out = fx.tan_falloff_p(age, 0x20, 1);
        for s6 in 0..0x20u32 {
            place_dy(fx, f, g + 0x40 + 0x10 * s6, out, g + 0x30, Some(-5.0));
            let c = fx.mem.read_u8(g + 8 + s6);
            cell_uvs(fx, f, u32::from(c));
            draw0(fx, f, f.v0);
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(g, 0);
        }
    }
}

fn step766(fx: &mut Fx, sp: u32, f: &Fr) -> i32 {
    fx.camera_turn_roll(f.turn);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    node(fx, D + 0x50, D + 0x60);
    for k in 0..2u32 {
        node(fx, D + 0x70 + 4 * k, D + 0x80 + 0x10 * k);
    }
    screen_point(fx, f, sp);
    let n = fx.mem.i32(D);
    if (0x85..0x121).contains(&n) {
        burst_spawn(fx, B7_IDX, B7, 0x20, [1.0, 0.5, 1.75], 0x01E0_07C8);
    }
    bursts766(fx, f);
    let n = fx.mem.i32(D);
    if rem2(n, 4) == 0 && (0x81..0x121).contains(&n) {
        flower_spawn766(fx, sp);
    }
    flowers766(fx, f);
    spark_records(fx, f, S7, 0x130, 1, 0x10, 0x20, 0x30, 12.0, 0xEE, 0x01E0_1C58);
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + PASS7, 1);
        fx.mem.write_u32(D + PASS7 + 4, 0);
    }
    tint(fx, 0, 0, 0x01E0_1D2C);
    if fx.mem.read_u8(D + PASS7) == 1 {
        unit_pass(fx, f, &DOME766, 0x01E0_2B30, 0x30, [0x01E0_1D64, 0x01E0_1F30]);
        let t = fx.mem.i32(D + PASS7 + 4) + 1;
        fx.mem.set32(D + PASS7 + 4, t);
        if t >= 0x31 {
            fx.mem.write_u8(D + PASS7, 0);
            tint(fx, 0, 0, 0x01E0_1F7C);
        }
    }
    let n = fx.mem.i32(D);
    if (0x123..0x16E).contains(&n) {
        motes_spawn(fx, n);
    }
    motes(fx, f);
    let n = fx.mem.i32(D);
    if n == 0x1B || n == 0x43 {
        fountain_spawn(fx, u32::from(n == 0x1B));
    }
    fountains(fx, f);
    light(fx, 0x1C6, 0x40, 80, 12);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x7A {
        ret = 1;
    } else {
        if (0x85..0x121).contains(&n) {
            ret = 6;
        }
        if n == 0x121 {
            ret = 2;
        }
        if n == 0x16E {
            ret = 4;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x1C7 {
        ret = 8;
    }
    ret
}
