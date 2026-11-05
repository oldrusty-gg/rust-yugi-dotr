//! BMODULE b472 (card 619), hand-ported from the record (linked at
//! 0x1E00000; one 2,784-word function 0x1E00080 in a 0x6B0 frame with D
//! spilled to sp + 0xF0 and the OT in s0, data area a0 = 0x1ED0000, 0xB1E0
//! bytes) [F]. Written fresh from the raw listing
//! (the project notes); its beams / trails / rings are b290's (card
//! 618) and b528's. Slots: the camera sp + 0x400 (model + 0x40, turn +
//! 0x80), corners sp + 0x4C0 / 0x4D0, vertices sp + 0x500 / 0x510, the
//! placed point sp + 0x520, the lifted point sp + 0x530, the fan descriptor
//! sp + 0x540, the quad descriptor sp + 0x5B8, the sprite descriptor sp +
//! 0x648, the mean w sp + 0x6AC. Init: nodes 0x11 (clear) / 0x21, the
//! target's position D+0xA0, 1.5 x the unit radius D+0x70, a ring of 17
//! (25, 55, mean) points D+0x3AE0. Every update two node points (D+0x30)
//! pulled up to (0, 80, 70 side) by clamp(n - 0x82, 0, 40) / 40 (D+0xB0;
//! scale 1 + that D+0xD0), the unit's point D+0x50 and its screen point
//! D+0x60 (Z of a point 1.5 r nearer D+0x74). On 0xB5..0x115 a beam record
//! an update (32 x 0x50 at D+0xE0) from the lifted point to the target
//! over 0x21 updates, 40 (4 t - 4 t t) up, dropping trail entries (32 x 4
//! x 0x60 at D+0xAE0) and on age 0x1F a ring (32 x 0x60 at D+0x3E20). On
//! 0x15..0x135 a cloud record of each kind (28 x 0x2D0 at D+0x4A20, 20 x
//! 0x140 at D+0x98E0). From 1 lens records (5 x 8 at D+0x78: the attacker
//! in a packet layer, a fan of the frame about the screen point). The
//! stage dims. Returns 1 on 0xCA, 6 on 0xD5..0x155, 2 on 0x156, 8 once the
//! count reaches 0x177.
use crate::b182::{alpha, head, rgba};
use crate::b440::{copy8, fix, fixs, rem2, words};
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::fxlib2::project;
use crate::lens::LENS_CLAMP;

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const ONE: u32 = 0x3F80_0000;
const FAN: u32 = 0x0028_ABE0;
const QUAD_REGS: u64 = 0x4214_2142_1421;

const VP: u32 = 0x400;
const MODEL: u32 = 0x440;
const TURN: u32 = 0x480;
const C0: u32 = 0x4C0;
const C1: u32 = 0x4D0;
const V0: u32 = 0x500;
const V1: u32 = 0x510;
const PL: u32 = 0x520;
const P2: u32 = 0x530;
const T: u32 = 0x540;
const Q: u32 = 0x5B8;
const S: u32 = 0x648;
const MEAN: u32 = 0x6AC;

const LENS: u32 = D + 0x78;
const BEAMS: u32 = D + 0xE0;
const TRAILS: u32 = D + 0xAE0;
const RING: u32 = D + 0x3AE0;
const RING_IDX: u32 = D + 0x3E10;
const RINGS: u32 = D + 0x3E20;
const CLOUD_A: u32 = D + 0x4A20;
const CLOUD_B: u32 = D + 0x98E0;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x6B0);
    fx.mem.write_u32(sp + 0xF0, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(0x6B0);
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

fn init(fx: &mut Fx) -> i32 {
    // The flags and counters the record clears after this are all inside it.
    fx.bzero(D, 0xB1E0);
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
    for (at, node, clear) in [(0x24u32, 0x11, 1), (0x28, 0x21, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0xA0);
    let h = fx.mem.i32(D + 4);
    let r = crate::blib::afterimage::unit_radius(fx, h);
    fx.mem.setf(D + 0x70, ee::mul(1.5, r));
    for k in 0..0x11u32 {
        let a = ((k << 16) as i32) >> 4;
        let p = RING + 0x30 * k;
        let s = ee::mul(25.0, sin16(fx, a));
        let c = ee::mul(25.0, cos16(fx, a));
        vec3(fx, p, s, c, 0.0);
        let s = ee::mul(55.0, sin16(fx, a));
        let c = ee::mul(55.0, cos16(fx, a));
        vec3(fx, p + 0x10, s, c, 0.0);
        fx.vu_add4(p + 0x20, p, p + 0x10);
        fx.vu_scale4(p + 0x20, p + 0x20, 0.5);
    }
    0
}

/// The sprite descriptor head (prim 0x26 | 0x800).
fn sprite_head(fx: &mut Fx, sp: u32) {
    head(fx, sp + S, 0x26, 5, 0x4_2421, 0x800);
}

/// The sprite descriptor: ALPHA preset 1, texture D + `tex`, clamp 5.
fn sprite_desc(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + S;
    sprite_head(fx, sp);
    alpha(fx, d, 1);
    let t = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, t, 1);
    fx.desc_clamp(d, 5);
}

/// The camera's view-projection x the identity.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MODEL);
    mul_mem(fx, sp + VP, sp + MODEL);
}

fn uv(fx: &mut Fx, at: u32, v: i32) {
    let h = fx.mem.read_u16(at);
    fx.mem.write_u16(at, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The sprite's UV rectangle.
fn uvs(fx: &mut Fx, sp: u32, u0: i32, v0: i32, u1: i32, v1: i32) {
    for (at, v) in [(0x660u32, u0), (0x662, v0), (0x670, u1), (0x672, v1)] {
        uv(fx, sp + at, v);
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

/// The two vertices at the placed point, projected and submitted at their
/// mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    fx.vu_add4(sp + V0, sp + C0, sp + PL);
    fx.vu_add4(sp + V1, sp + C1, sp + PL);
    if project(fx, sp + V0, PROJ_OUT, sp + VP, sp + S, sp + MEAN) {
        let w = fx.mem.f(sp + MEAN);
        let key = fix(fx, w, ra);
        fx.submit(sp + S, OT, key as i32);
    }
}

/// The quad's four ST pairs.
fn sts(fx: &mut Fx, sp: u32, s0: f32, s1: f32, t0: f32, t1: f32) {
    for (at, s, t) in [(0x5D0u32, s0, t0), (0x5E8, s1, t0), (0x600, s0, t1), (0x618, s1, t1)] {
        fx.mem.setf(sp + at, s);
        fx.mem.setf(sp + at + 4, t);
    }
}

/// (0, 80, 70 side, 1) at `p`.
fn lift(fx: &mut Fx, p: u32) {
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    vec3(fx, p, 0.0, 80.0, ee::mul(70.0, side));
}

/// The unit's screen point D+0x60 and the Z of a point nearer it.
fn screen(fx: &mut Fx, sp: u32) {
    let (o, pl) = (D + 0x60, sp + PL);
    let m = fx.read_m4(sp + VP);
    fx.project_at(D + 0x50, &m, sp + 0x6A0, sp + MEAN);
    let x = (i32::from(fx.mem.read_u16(sp + 0x6A0)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x6A2)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x6A0) << 8) >> 40;
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
    let n = ee::neg(fx.mem.f(D + 0x70));
    vec3(fx, pl, 0.0, 0.0, n);
    fx.vu_apply_to(pl, sp + TURN, pl);
    fx.vu_add4(pl, pl, D + 0x50);
    let m = fx.read_m4(sp + VP);
    fx.project_at(pl, &m, sp + 0x6A0, sp + MEAN);
    let z = (fx.mem.read_u64(sp + 0x6A0) << 8) >> 40;
    fx.mem.set32(D + 0x74, z as i32);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    let pl = sp + PL;
    for k in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x24 + 4 * k);
        fx.node_position(h, s, D + 0x30 + 0x10 * k);
    }
    let c = Fx::clamp_i(fx.mem.i32(D) - 0x82, 0, 0x28);
    let f20 = ee::div(ee::from_i32(c), 40.0);
    fx.mem.setf(D + 0xD0, ee::add(1.0, f20));
    for k in 0..2u32 {
        lift(fx, pl);
        fx.vu_sub4(pl, pl, D + 0x30 + 0x10 * k);
        fx.vu_scale4(pl, pl, f20);
        fx.vu_add4(D + 0xB0 + 0x10 * k, pl, D + 0x30 + 0x10 * k);
    }
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x50);
    camera(fx, sp);
    screen(fx, sp);
    let c = fx.mem.i32(D);
    if (0xB5..0x116).contains(&c) {
        spawn_beam(fx, sp, c);
    }
    sprite_desc(fx, sp, 0x18);
    camera(fx, sp);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    corners(fx, sp, 5.0, true);
    lift(fx, sp + P2);
    for k in 0..0x20u32 {
        if fx.mem.read_u8(BEAMS + 0x50 * k) != 0 {
            beam(fx, sp, k);
        }
    }
    sprite_desc(fx, sp, 0x1C);
    camera(fx, sp);
    for k in 0..0x80u32 {
        let t = TRAILS + 0x60 * k;
        if fx.mem.read_u8(t) != 0 {
            trail(fx, sp, t);
        }
    }
    let q = sp + Q;
    head(fx, q, 0x34, 0xC, QUAD_REGS, 0);
    alpha(fx, q, 1);
    let t = fx.mem.read_u32(D + 0x20);
    fx.desc_texture(q, t, 1);
    fx.desc_clamp(q, 0);
    sprite_desc(fx, sp, 0x18);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    corners(fx, sp, 20.0, false);
    for k in 0..0x20u32 {
        let r = RINGS + 0x60 * k;
        if fx.mem.read_u8(r) != 0 {
            ring(fx, sp, r);
        }
    }
    let c = fx.mem.i32(D);
    if (0x15..0x136).contains(&c) {
        spawn_cloud_a(fx, c);
    }
    clouds_a(fx, sp);
    let c = fx.mem.i32(D);
    if (0x15..0x136).contains(&c) {
        spawn_cloud_b(fx, c);
    }
    clouds_b(fx, sp);
    let c = fx.mem.i32(D);
    if c > 0 && c < 0x136 {
        let (_, m) = fx.div(c, 5);
        let l = LENS + 8 * m as u32;
        fx.mem.write_u8(l, 1);
        fx.mem.write_u32(l + 4, 0);
    }
    lenses(fx, sp);
    let c = fx.mem.i32(D);
    let a = Fx::clamp_i(0x176 - c, 0, 0x20);
    let b = Fx::clamp_i(c, 0, 0x20);
    let l = (a.wrapping_mul((b - (b << 2)) << 5) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);
    let c = fx.mem.i32(D);
    if c == 0xCA {
        fx.mem.write_u32(sp + 0xC0, 1);
    } else {
        if (0xD5..0x156).contains(&c) {
            fx.mem.write_u32(sp + 0xC0, 6);
        }
        if c == 0x156 {
            fx.mem.write_u32(sp + 0xC0, 2);
        }
    }
    let n = fx.mem.i32(D) + 1;
    fx.mem.set32(D, n);
    if fx.mem.i32(D) >= 0x177 {
        fx.mem.write_u32(sp + 0xC0, 8);
    }
    fx.mem.i32(sp + 0xC0)
}

/// [F 0x1E006B4] Beam c % 32: its length and aim from the lifted point to
/// the target + a random vector, spun by rand.
fn spawn_beam(fx: &mut Fx, sp: u32, c: i32) {
    let v = sp + PL;
    let r = BEAMS + 0x50 * rem2(c, 0x20) as u32;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    lift(fx, sp + P2);
    fx.random_table_vec(v, 1.0);
    fx.vu_add4(v, v, D + 0xA0);
    fx.vu_sub4(v, v, sp + P2);
    let x = Fx::powf2(fx.mem.f(v));
    let f20 = ee::add(x, Fx::powf2(fx.mem.f(v + 4)));
    let z = Fx::powf2(fx.mem.f(v + 8));
    let l = fx.sqrtf(ee::add(z, f20));
    fx.mem.setf(r + 0xC, l);
    fx.point_along(r + 0x10, v);
    let n = rnd(fx, 0x01E0_07B8);
    fx.rotate_axis(r + 0x10, (rem2(n, 256) << 16) >> 8, 2);
}

/// [F 0x1E009B0] A beam's sprite, the trail entry it leaves and on age
/// 0x1F a ring with a random turn.
fn beam(fx: &mut Fx, sp: u32, k: u32) {
    let pl = sp + PL;
    let r = BEAMS + 0x50 * k;
    rgba(fx, sp + S + 0x10, [0xFF, 0x50, 0x23], 0xFF);
    let f21 = ee::div(ee::from_i32(fx.mem.i32(r + 4)), 32.0);
    let f1 = ee::add(ee::mul(4.0, f21), ee::mul(ee::mul(-4.0, f21), f21));
    let ia = fx.mem.i32(r + 8) as u32;
    let v = ia as i32 + 1;
    fx.mem.set32(r + 8, if v < 4 { v } else { 0 });
    let f20 = ee::mul(40.0, f1);
    let l = fx.mem.f(r + 0xC);
    vec3(fx, pl, 0.0, f20, ee::mul(f21, l));
    fx.vu_apply_to(pl, r + 0x10, pl);
    fx.vu_add4(pl, pl, sp + P2);
    draw(fx, sp, 0x01E0_0B28);
    let t = TRAILS + 0x180 * k + 0x60 * ia;
    fx.mem.write_u8(t, 1);
    fx.mem.write_u32(t + 4, 0);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let f1 = ee::div(ee::mul(f32::from_bits(0x3F33_3333), age), 32.0);
    fx.mem.setf(t + 0xC, ee::add(f32::from_bits(0x3E99_999A), f1));
    words(fx, t + 0x10, pl);
    for j in 0..4u32 {
        let n = rnd(fx, 0x01E0_0BD8);
        fx.mem.set8(t + 8 + j, rem2(n, 8));
        let w = fx.mem.f(t + 0xC);
        fx.random_table_vec(t + 0x20 + 0x10 * j, ee::mul(f32::from_bits(0x3E99_999A), w));
    }
    if fx.mem.i32(r + 4) == 0x1F {
        let n = fx.mem.read_u32(RING_IDX);
        let w = RINGS + 0x60 * n;
        fx.mem.write_u8(w, 1);
        fx.mem.write_u32(w + 4, 0);
        words(fx, w + 0x10, pl);
        let m = w + 0x20;
        fx.identity(m);
        for (axis, sh, ra) in [(0, 15, 0x01E0_0C98u32), (1, 15, 0x01E0_0CCC), (2, 16, 0x01E0_0D00)] {
            let v = rnd(fx, ra);
            fx.rotate_axis(m, (rem2(v, 256) << sh) >> 8, axis);
        }
        let v = fx.mem.i32(RING_IDX) + 1;
        fx.mem.set32(RING_IDX, if v < 0x20 { v } else { 0 });
    }
    let n = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, n);
    if n >= 0x21 {
        fx.mem.write_u8(r, 0);
    }
}

/// [F 0x1E00EDC] A trail entry: 4 sprites (FF, 46, 1E) at 128 (4 - age) /
/// 4, 6 (0.5 + (4 - age) / 8) wide, out by 0.2 + 0.8 falloff, 5 updates.
fn trail(fx: &mut Fx, sp: u32, t: u32) {
    let pl = sp + PL;
    let age = fx.mem.i32(t + 4);
    let f1 = ee::mul(0.5, ee::from_i32(4 - age));
    let s = ee::mul(6.0, ee::add(0.5, ee::div(f1, 4.0)));
    corners(fx, sp, s, true);
    let age = fx.mem.i32(t + 4);
    rgba(fx, sp + S + 0x10, [0xFF, 0x46, 0x1E], (((4 - age) << 7) >> 2) as u8);
    let f0 = fx.tan_falloff_p(age, 4, 1);
    let f20 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
    for k in 0..4u32 {
        let b = i32::from(fx.mem.read_u8(t + 8 + k));
        cell(fx, sp, b);
        fx.vu_scale4(pl, t + 0x20 + 0x10 * k, f20);
        fx.vu_add4(pl, pl, t + 0x10);
        draw(fx, sp, 0x01E0_1100);
    }
    let n = fx.mem.i32(t + 4) + 1;
    fx.mem.set32(t + 4, n);
    if n >= 5 {
        fx.mem.write_u8(t, 0);
    }
}

/// [F 0x1E01358] A ring: the 16-segment mesh at its matrix, scaled 0.1 +
/// 0.9 falloff, scrolled by age % 32, keyed by w; then a flat sprite at its
/// centre; 0x21 updates.
fn ring(fx: &mut Fx, sp: u32, r: u32) {
    let (vp, m, q, pl) = (sp + VP, sp + MODEL, sp + Q, sp + PL);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let f0 = Fx::ramp_above(age, 32.0, 16.0);
    rgba(fx, q + 0x10, [0x60, 0x26, 0x1C], 0);
    copy8(fx, q + 0x28, q + 0x10);
    let a = fix(fx, ee::mul(100.0, f0), 0x01E0_13C4);
    rgba(fx, q + 0x40, [0xFF, 0x62, 0x26], a as u8);
    copy8(fx, q + 0x58, q + 0x40);
    fx.camera_vp_to(vp);
    let t = [fx.mem.f(r + 0x10), fx.mem.f(r + 0x14), fx.mem.f(r + 0x18)];
    let tm = fx.trs(None, None, Some(t));
    fx.write_m4(m, &tm);
    mul_mem(fx, m, r + 0x20);
    let age = fx.mem.i32(r + 4);
    let f0 = fx.tan_falloff_p(age, 0x20, 1);
    let f = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), f0));
    vec3(fx, pl, f, f, 0.0);
    fx.scale_rows(m, pl);
    mul_mem(fx, vp, m);
    let v = rem2(fx.mem.i32(r + 4), 32);
    let f20 = ee::div(ee::mul(-1.0, ee::from_i32(v)), 32.0);
    let f21 = ee::add(0.5, f20);
    let f22 = ee::add(1.0, f20);
    for k in 0..0x10u32 {
        let (a, b) = (RING + 0x30 * k, RING + 0x30 * (k + 1));
        let f23 = ee::div(ee::mul(6.0, ee::from_i32(k as i32)), 16.0);
        let f24 = ee::div(ee::mul(6.0, ee::from_i32(k as i32 + 1)), 16.0);
        sts(fx, sp, f23, f24, f20, f21);
        if fx.project_n(&[a, b, a + 0x20, b + 0x20], vp, q, sp + MEAN, 0x01E0_15B0) == 1 {
            let w = fx.mem.f(sp + MEAN);
            let key = fix(fx, w, 0x01E0_15C4);
            fx.submit(q, OT, key as i32);
        }
        sts(fx, sp, f23, f24, f22, f21);
        if fx.project_n(&[a + 0x10, b + 0x10, a + 0x20, b + 0x20], vp, q, sp + MEAN, 0x01E0_1638) == 1 {
            let w = fx.mem.f(sp + MEAN);
            let key = fix(fx, w, 0x01E0_164C);
            fx.submit(q, OT, key as i32);
        }
    }
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let f0 = Fx::ramp_above(age, 32.0, 16.0);
    let a = fix(fx, ee::mul(255.0, f0), 0x01E0_16A0);
    rgba(fx, sp + S + 0x10, [0xFF, 0x44, 0x23], a as u8);
    fx.camera_vp_to(vp);
    fx.translate_rows(m, sp + TURN, r + 0x10);
    let age = fx.mem.i32(r + 4);
    let f0 = fx.tan_falloff_p(age, 0x20, 1);
    let f = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), f0));
    vec3(fx, pl, f, f, f);
    fx.scale_rows(m, pl);
    mul_mem(fx, vp, m);
    if project(fx, sp + C0, PROJ_OUT, vp, sp + S, sp + MEAN) {
        let w = fx.mem.f(sp + MEAN);
        let key = fix(fx, w, 0x01E0_177C);
        fx.submit(sp + S, OT, key as i32);
    }
    let n = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, n);
    if n >= 0x21 {
        fx.mem.write_u8(r, 0);
    }
}

/// [F 0x1E0182C] Cloud record c % 28: 2 x 20 sparks at the pulled nodes,
/// vectors of 0.28 x the scale.
fn spawn_cloud_a(fx: &mut Fx, c: i32) {
    let (_, m) = fx.div(c, 0x1C);
    let r = CLOUD_A + 0x2D0 * m as u32;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    for i in 0..2u32 {
        words(fx, r + 0x30 + 0x10 * i, D + 0xB0 + 0x10 * i);
        for j in 0..0x14u32 {
            let n = rnd(fx, 0x01E0_18B8);
            fx.mem.set8(r + 8 + 0x14 * i + j, rem2(n, 4));
            let w = fx.mem.f(D + 0xD0);
            fx.random_table_vec(r + 0x50 + 0x140 * i + 0x10 * j, ee::mul(f32::from_bits(0x3E8F_5C29), w));
        }
    }
}

/// [F 0x1E01E08] Cloud record c % 20: 2 x 8 sparks, vectors of 0.15 x the
/// scale.
fn spawn_cloud_b(fx: &mut Fx, c: i32) {
    let (_, m) = fx.div(c, 0x14);
    let r = CLOUD_B + 0x140 * m as u32;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    for i in 0..2u32 {
        words(fx, r + 0x20 + 0x10 * i, D + 0xB0 + 0x10 * i);
        for j in 0..8u32 {
            let n = rnd(fx, 0x01E0_1EA0);
            fx.mem.set8(r + 8 + 8 * i + j, rem2(n, 8));
            let w = fx.mem.f(D + 0xD0);
            fx.random_table_vec(r + 0x40 + 0x80 * i + 0x10 * j, ee::mul(f32::from_bits(0x3E19_999A), w));
        }
    }
}

/// clamp(n - 0x14, 0, 32) clamp(0x136 - n, 0, 32) / 1024.
fn cloud_f(fx: &mut Fx) -> f32 {
    let c1 = Fx::clamp_i(fx.mem.i32(D) - 0x14, 0, 0x20);
    let c2 = Fx::clamp_i(0x136 - fx.mem.i32(D), 0, 0x20);
    ee::div(ee::mul(ee::from_i32(c1), ee::from_i32(c2)), 1024.0)
}

/// The cloud sprite descriptor head and its clamp.
fn cloud_head(fx: &mut Fx, sp: u32) {
    sprite_head(fx, sp);
    fx.desc_clamp(sp + S, 5);
}

/// [F 0x1E01948] The first clouds.
fn clouds_a(fx: &mut Fx, sp: u32) {
    let (d, pl) = (sp + S, sp + PL);
    cloud_head(fx, sp);
    let t = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, t, 1);
    alpha(fx, d, 1);
    camera(fx, sp);
    let f20 = cloud_f(fx);
    for k in 0..0x1Cu32 {
        let r = CLOUD_A + 0x2D0 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let x = fx.mem.i32(r + 4) << 15;
        fx.mult(0x9249_2493_u32 as i32, x);
        let q = (fx.hilo().1.wrapping_add(x) >> 4).wrapping_add(((x as u32) >> 31) as i32);
        let f0 = sin16(fx, q);
        let a = fix(fx, ee::mul(f20, ee::mul(128.0, f0)), 0x01E0_1A94);
        let age = fx.mem.i32(r + 4);
        let sc = fx.mem.f(D + 0xD0);
        let f2 = ee::mul(0.5, ee::from_i32(0x1C - age));
        let s = ee::mul(ee::mul(0.25, ee::add(0.5, ee::div(f2, 28.0))), sc);
        corners(fx, sp, s, true);
        let age = fx.mem.i32(r + 4);
        let f0 = fx.tan_falloff_p(age, 0x1C, 1);
        let f21 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let f22 = ee::mul(ee::add(-0.5, ee::div(ee::mul(15.0, age), 28.0)), fx.mem.f(D + 0xD0));
        for i in 0..2u32 {
            rgba(fx, sp + S + 0x10, [0xFF, 0x64, 0x2D], a as u8);
            for j in 0..0x14u32 {
                let b = i32::from(fx.mem.read_u8(r + 8 + 0x14 * i + j));
                cell(fx, sp, b);
                fx.vu_scale4(pl, r + 0x50 + 0x140 * i + 0x10 * j, f21);
                fx.vu_add4(pl, pl, r + 0x30 + 0x10 * i);
                let y = ee::add(fx.mem.f(pl + 4), f22);
                fx.mem.setf(pl + 4, y);
                draw(fx, sp, 0x01E0_1D30);
            }
        }
        let n = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, n);
        if n >= 0x1D {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// [F 0x1E01F40] The second clouds.
fn clouds_b(fx: &mut Fx, sp: u32) {
    let (d, pl) = (sp + S, sp + PL);
    cloud_head(fx, sp);
    camera(fx, sp);
    let f20 = cloud_f(fx);
    for k in 0..0x14u32 {
        let r = CLOUD_B + 0x140 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let t = fx.mem.read_u32(D + 0x1C);
        fx.desc_texture(d, t, 1);
        alpha(fx, d, 1);
        let x = fx.mem.i32(r + 4) << 15;
        fx.mult(0x6666_6667, x);
        let q = (fx.hilo().1 >> 3).wrapping_add(((x as u32) >> 31) as i32);
        let f0 = sin16(fx, q);
        let a = fix(fx, ee::mul(f20, ee::mul(128.0, f0)), 0x01E0_209C);
        let age = fx.mem.i32(r + 4);
        let sc = fx.mem.f(D + 0xD0);
        let f2 = ee::div(ee::mul(f32::from_bits(0x3F33_3333), ee::from_i32(0x14 - age)), 20.0);
        let s = ee::mul(ee::mul(3.0, ee::add(f32::from_bits(0x3E99_999A), f2)), sc);
        corners(fx, sp, s, true);
        let age = fx.mem.i32(r + 4);
        let sc = fx.mem.f(D + 0xD0);
        let f3 = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(0x14 - age)), 20.0);
        let f21 = ee::add(f32::from_bits(0x3F4C_CCCD), f3);
        let f22 = ee::mul(ee::add(-4.5, ee::div(ee::mul(17.0, ee::from_i32(age)), 20.0)), sc);
        for i in 0..2u32 {
            rgba(fx, sp + S + 0x10, [0xFF, 0x3C, 0x1E], a as u8);
            for j in 0..8u32 {
                let b = i32::from(fx.mem.read_u8(r + 8 + 8 * i + j));
                cell(fx, sp, b);
                fx.vu_scale4(pl, r + 0x40 + 0x80 * i + 0x10 * j, f21);
                fx.vu_add4(pl, pl, r + 0x20 + 0x10 * i);
                let y = ee::add(fx.mem.f(pl + 4), f22);
                fx.mem.setf(pl + 4, y);
                draw(fx, sp, 0x01E0_236C);
            }
        }
        let n = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, n);
        if n >= 0x15 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The XYZ2 at `p`: pixel (x, y) at the Z of D+0x74, a zero top byte.
fn xyz(fx: &mut Fx, p: u32, x: i32, y: i32) {
    fx.mem.write_u16(p, ((x + 0x700) << 4) as u16);
    fx.mem.write_u16(p + 2, ((y + 0x700) << 4) as u16);
    let z = (fx.mem.read_u32(D + 0x74) as u64 & 0xFF_FFFF) << 32;
    let v = fx.mem.read_u64(p);
    fx.mem.write_u64(p, (v & KEEP) | z);
    fx.mem.write_u8(p + 7, 0);
}

fn fix_at(fx: &mut Fx, p: u32, ra: u32) -> i32 {
    let f = fx.mem.f(p);
    fixs(fx, f, ra)
}

/// [F 0x1E02730] b424's fan about the screen point at the Z of D+0x74.
fn fan(fx: &mut Fx, sp: u32, f22: f32) {
    let (d, c, v0, v1) = (sp + T, D + 0x60, sp + V0, sp + V1);
    for k in 0..8u32 {
        let s1 = FAN + 0x10 * k;
        let x = fix_at(fx, c, 0x01E0_273C);
        let y = fix_at(fx, c + 4, 0x01E0_274C);
        uv(fx, d + 0x18, x << 4);
        uv(fx, d + 0x1A, y << 4);
        let x = fix_at(fx, s1, 0x01E0_2790);
        let y = fix_at(fx, s1 + 4, 0x01E0_279C);
        uv(fx, d + 0x30, x << 4);
        uv(fx, d + 0x32, y << 4);
        let x = fix_at(fx, s1 + 0x10, 0x01E0_27E0);
        let y = fix_at(fx, s1 + 0x14, 0x01E0_27EC);
        uv(fx, d + 0x48, x << 4);
        uv(fx, d + 0x4A, y << 4);
        fx.vu_sub4(v0, s1, c);
        fx.vu_sub4(v1, s1 + 0x10, c);
        fx.vu_scale4(v0, v0, f22);
        fx.vu_scale4(v1, v1, f22);
        fx.vu_add4(v0, v0, c);
        fx.vu_add4(v1, v1, c);
        let x = fix_at(fx, c, 0x01E0_28B4);
        let y = fix_at(fx, c + 4, 0x01E0_28C4);
        xyz(fx, d + 0x20, x, y);
        let x = fix_at(fx, v0, 0x01E0_2930);
        let y = fix_at(fx, v0 + 4, 0x01E0_293C);
        xyz(fx, d + 0x38, x, y);
        let x = fix_at(fx, v1, 0x01E0_29A0);
        let y = fix_at(fx, v1 + 4, 0x01E0_29AC);
        xyz(fx, d + 0x50, x, y);
        fx.submit(d, 0, 0);
    }
}

/// [F 0x1E02448] The lens records: a packet layer with the attacker drawn
/// and for each a fan, (A8, FF, 64) at f 255 sin((age + 1) / 6 half turn)
/// at its centre, (20, FF, 8) at 0 at its rim, 6 updates.
fn lenses(fx: &mut Fx, sp: u32) {
    let d = sp + T;
    head(fx, d, 0x33, 9, 0x4_2142_1421, 0x800);
    alpha(fx, d, 1);
    fx.desc_clamp(d, LENS_CLAMP);
    let c1 = Fx::clamp_i(fx.mem.i32(D), 0, 0x20);
    let c2 = Fx::clamp_i(0x136 - fx.mem.i32(D), 0, 0x20);
    let f21 = ee::div(ee::mul(ee::from_i32(c1), ee::from_i32(c2)), 1024.0);
    let f20 = ee::mul(f21, 0.0);
    fx.mem.write_u8(sp + 0xD0, 0);
    for k in 0..5u32 {
        let l = LENS + 8 * k;
        if fx.mem.read_u8(l) == 0 {
            continue;
        }
        if fx.mem.read_u8(sp + 0xD0) == 0 {
            fx.mem.write_u8(sp + 0xD0, 1);
            let layer = fx.layer_begin();
            fx.mem.write_u32(sp + 0xEC, layer);
            fx.texflush([c2 as u32, 0, 0x20, d + 2]);
            let h = fx.mem.i32(D + 4);
            let (_, regs) = fx.unit_draw(h, 1, sp + 0x5A0, sp + 0x5A8);
            fx.texflush(regs);
        }
        let x = (fx.mem.i32(l + 4) + 1) << 15;
        fx.mult(0x2AAA_AAAB, x);
        let q = fx.hilo().1.wrapping_add(((x as u32) >> 31) as i32);
        let f0 = sin16(fx, q);
        let a = fix(fx, ee::mul(f21, ee::mul(255.0, f0)), 0x01E0_262C);
        rgba(fx, d + 0x10, [0xA8, 0xFF, 0x64], a as u8);
        let a = fix(fx, f20, 0x01E0_268C);
        rgba(fx, d + 0x28, [0x20, 0xFF, 8], a as u8);
        copy8(fx, d + 0x40, d + 0x28);
        let age = ee::from_i32(fx.mem.i32(l + 4));
        let f2 = ee::div(ee::mul(f32::from_bits(0x3E99_999A), age), 5.0);
        let f1 = ee::add(1.0, ee::add(f32::from_bits(0xBE19_999A), f2));
        let f22 = ee::add(f32::from_bits(0x3CF5_C290), f1);
        fan(fx, sp, f22);
        let n = fx.mem.i32(l + 4) + 1;
        fx.mem.set32(l + 4, n);
        if n >= 6 {
            fx.mem.write_u8(l, 0);
        }
    }
    if fx.mem.read_u8(sp + 0xD0) == 1 {
        let layer = fx.mem.read_u32(sp + 0xEC);
        fx.layer_end(layer, OT, 0);
    }
}
