//! BMODULE b647 (card 525), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xC700
//! bytes) [F], written cold from the project notes
//! (b722's family: D spilled to sp + 0xD0, s0 the ordering table, frame
//! 0x4F0).
//!
//! Init: textures (4, 0xF), (5, 0), (5, 1), nodes 0xA (kind 1) / 0x1D, the
//! target's position D+0x50, a 17-point ring pair D+0x60 (radii 4 / 12), a
//! colour ramp of 9 (D+0x288), a circle of 64 D+0xC3E0.
//! Every update the camera turn with roll (sp + 0x280), the nodes into D+0x30
//! / D+0x40, their midpoint sp + 0x350 and distance. From 0 for 0x191
//! updates a gouraud tube about each node; on 1..0x18F ring records (16 x
//! 0x3D0 at D+0x2B0, b722's, 12 rings of 8 steps, 6 about each node); on
//! 0x51..0x18F spokes between the nodes (16 x 0x48 at D+0x3FB0); on
//! 0xC9..0x158 every tenth update a beam (24 x 0x280 at D+0x4440) from a node
//! to a random point near the target, which on its fourth update leaves a
//! flash (12 x 0x20 at D+0x8050), 64 sparks (16 x 0x420 at D+0x81E0) and a
//! disc (16 x 0x20 at D+0xC500). Returns 1 on 0xC2, 6 on 0xCD..0x158, 2 on
//! 0x159, 8 from 0x190.
use crate::b182::{alpha, head, rgba};
use crate::blib::exe::{mul_mem, translation_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// Four tables of 32 random 12-byte vectors (exe rodata, 0x180 apart).
const VECTORS: u32 = 0x0028_AE90;
const FRAME: u32 = 0x4F0;
const VP: u32 = 0x200;
const MS: u32 = 0x240;
const TURN: u32 = 0x280;
const C0: u32 = 0x2C0;
const C1: u32 = 0x2D0;
const Q: u32 = 0x300;
const T: u32 = 0x340;
const MID: u32 = 0x350;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const SD: u32 = 0x490;
/// The strip descriptor (prim 0x34, 12 registers).
const ST: u32 = 0x400;
const W: u32 = 0x4EC;
const RINGS: u32 = 0x2B0;
const SPOKES: u32 = 0x3FB0;
const BEAMS: u32 = 0x4440;
const FLASHES: u32 = 0x8050;
const SPARKS: u32 = 0x81E0;
const CIRCLE: u32 = 0xC3E0;
const DISCS: u32 = 0xC500;
const STRIP_CLAMP: u64 = (0x1FC << 32) | 0x0123_40BA;

fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xD0, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
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

/// 0x1A2C70 with its residue frame.
fn fixu(fx: &mut Fx, v: f32, ra: u32) -> u32 {
    fx.fix_residue(v, ra);
    ee::fixunssfsi(v)
}

/// 0x1A2B80 with its residue frame.
fn fixs(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(OT, ra) >> 23
}

/// rand >> 23 as a phase: ((r % 256) << 16) >> 8.
fn phase(fx: &mut Fx, ra: u32) -> i32 {
    let r = rnd(fx, ra);
    ((r % 256) << 16) >> 8
}

/// A component `c` of a random table vector: the table from rand % 4 (from
/// `ra.0`), the vector from rand % 32 (from `ra.1`).
fn table_part(fx: &mut Fx, c: u32, ra: (u32, u32)) -> f32 {
    let r = rnd(fx, ra.0) % 4;
    let base = (VECTORS as i32 + 0x180 * r) as u32;
    let i = rnd(fx, ra.1) % 32;
    fx.mem.f((base as i32 + 12 * i) as u32 + 4 * c)
}

fn uv(fx: &mut Fx, a: u32, x: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (x & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    let d = sp + SD;
    uv(fx, d + 0x18, u0);
    uv(fx, d + 0x1A, v0);
    uv(fx, d + 0x28, u1);
    uv(fx, d + 0x2A, v1);
}

fn sprite_head(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + SD;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
}

fn strip_head(fx: &mut Fx, sp: u32) {
    let d = sp + ST;
    head(fx, d, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, STRIP_CLAMP);
}

/// The strip's fixed S columns with the T rows t0 / t1.
fn strip_st(fx: &mut Fx, sp: u32, t0: f32, t1: f32) {
    let d = sp + ST;
    let (s0, s1) = (f(0x3D20_0000), f(0x3E58_0000));
    for (o, x) in [(0x18u32, s0), (0x1C, t0), (0x30, s1), (0x34, t0), (0x48, s0), (0x4C, t1), (0x60, s1), (0x64, t1)] {
        fx.mem.setf(d + o, x);
    }
}

/// The strip's colour 0 in all four corners.
fn strip_one_colour(fx: &mut Fx, sp: u32) {
    let d = sp + ST;
    for o in [0x28u32, 0x40, 0x58] {
        words(fx, d + o, d + 0x10, 2);
    }
}

/// The camera's view-projection (x identity).
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MS);
    mul_mem(fx, sp + VP, sp + MS);
}

/// The corners (-a, -a, 0, 1) / (a, a, 0, 1) turned by the camera.
fn corners(fx: &mut Fx, sp: u32, a: f32) {
    vec4(fx, sp + C0, [ee::neg(a), ee::neg(a), 0.0, 1.0]);
    vec4(fx, sp + C1, [a, a, 0.0, 1.0]);
    fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
    fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
}

/// The vertices at `q` through 0x204F70 to `d`, drawn keyed by the mean w.
fn draw_w(fx: &mut Fx, sp: u32, q: u32, d: u32, ra: u32) {
    if crate::fxlib2::project(fx, q, PROJ_OUT, sp + VP, d, sp + W) {
        let w = fx.mem.f(sp + W);
        let key = fixu(fx, w, ra);
        fx.submit(d, OT, key as i32);
    }
}

/// The sprite at sp + T + the corners, keyed by the mean w.
fn sprite_at(fx: &mut Fx, sp: u32, ra: u32) {
    fx.vu_add4(sp + Q, sp + C0, sp + T);
    fx.vu_add4(sp + 0x310, sp + C1, sp + T);
    draw_w(fx, sp, sp + Q, sp + SD, ra);
}

/// (lo + (a r) / 256) k.
fn spread(r: i32, a: f32, lo: f32, k: f32) -> f32 {
    ee::mul(k, ee::add(lo, ee::div(ee::mul(a, ee::from_i32(r)), 256.0)))
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xC700);
    let h = fx.self_handle();
    fx.mem.write_u32(D, 0);
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for (k, (g, i)) in [(4, 0xF), (5, 0), (5, 1)].into_iter().enumerate() {
        let r = fx.texture(g, i);
        fx.mem.write_u32(D + 0x10 + 4 * k as u32, r);
    }
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0xA, 1);
    fx.mem.set32(D + 0x1C, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x1D, 0);
    fx.mem.set32(D + 0x20, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x50);
    for k in 0..0x11u32 {
        let a = ((k << 16) as i32) >> 4;
        let g = D + 0x60 + 0x20 * k;
        let (s, c) = (fx.tsin(a), fx.tcos(a));
        vec4(fx, g, [ee::mul(4.0, s), ee::mul(4.0, c), 0.0, 1.0]);
        vec4(fx, g + 0x10, [ee::mul(12.0, s), ee::mul(12.0, c), 0.0, 1.0]);
        vec4(fx, D + CIRCLE + 0x10 * k, [ee::mul(64.0, s), ee::mul(64.0, c), 0.0, 1.0]);
    }
    for k in 0..9u32 {
        let f20 = ee::div(ee::from_i32(k as i32), 8.0);
        let f21 = ee::sub(1.0, f20);
        let c = D + 0x288 + 4 * k;
        let x = fixu(fx, ee::mul(96.0, f21), 0x01E0_0290);
        fx.mem.write_u8(c, x as u8);
        let x = fixu(fx, ee::mul(128.0, f21), 0x01E0_02A4);
        fx.mem.write_u8(c + 1, x as u8);
        let f21 = ee::mul(255.0, f21);
        let x = fixu(fx, ee::add(f21, ee::mul(128.0, f20)), 0x01E0_02D0);
        fx.mem.write_u8(c + 2, x as u8);
        let x = fixu(fx, f21, 0x01E0_02DC);
        fx.mem.write_u8(c + 3, x as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    for k in 0..2u32 {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x1C + 4 * k));
        fx.node_position(h, s, D + 0x30 + 0x10 * k);
    }
    fx.vu_add4(sp + MID, D + 0x30, D + 0x40);
    fx.vu_scale4(sp + MID, sp + MID, 0.5);
    let x = Fx::powf2(ee::sub(fx.mem.f(D + 0x30), fx.mem.f(D + 0x40)));
    let y = Fx::powf2(ee::sub(fx.mem.f(D + 0x34), fx.mem.f(D + 0x44)));
    let z = Fx::powf2(ee::sub(fx.mem.f(D + 0x38), fx.mem.f(D + 0x48)));
    fx.mem.write_u64(fx.sp - 0x10, 0x01E0_055C);
    let f23 = fx.sqrtf(ee::add(z, ee::add(x, y)));
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + 0x280, 1);
        fx.mem.write_u32(D + 0x284, 0);
    }
    let f22 = tube(fx, sp);
    rings(fx, sp, f22);
    spokes(fx, sp, f23);
    beam_spawn(fx);
    beams(fx, sp);
    flashes(fx, sp);
    sparks(fx, sp);
    discs(fx, sp);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0xC2 {
        ret = 1;
    } else if (0xCD..0x159).contains(&n) {
        ret = 6;
    } else if n == 0x159 {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x191 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// The tube [F 0x1E0057C..0x1E007C0]; returns its fade f22.
fn tube(fx: &mut Fx, sp: u32) -> f32 {
    let d = sp + 0x3A8;
    head(fx, d, 0x14, 8, 0x4141_4141, 0);
    alpha(fx, d, 1);
    let age = fx.mem.i32(D + 0x284);
    let x = ee::from_i32(age);
    let f22 = ee::mul(Fx::ratio_below(x, 32.0), Fx::ramp_above(x, 400.0, 368.0));
    if fx.mem.read_u8(D + 0x280) != 1 {
        return f22;
    }
    let al = fixu(fx, ee::mul(200.0, f22), 0x01E0_0614);
    rgba(fx, d + 0x10, [0xA4, 0x80, 0xA4], al as u8);
    rgba(fx, d + 0x30, [0, 0, 0xFF], 0);
    words(fx, d + 0x20, d + 0x10, 2);
    words(fx, d + 0x40, d + 0x30, 2);
    for s4 in 0..2i32 {
        fx.camera_vp_to(sp + VP);
        fx.translate_rows(sp + MS, sp + TURN, D + 0x30 + 0x10 * s4 as u32);
        let age = fx.mem.i32(D + 0x284);
        let v = ee::from_i32((age + s4) % 2);
        let s = ee::add(f(0x3F66_6666), ee::mul(f(0x3DCC_CCCD), v));
        vec4(fx, sp + T, [s, s, s, 1.0]);
        fx.scale_rows(sp + MS, sp + T);
        mul_mem(fx, sp + VP, sp + MS);
        for k in 0..0x10u32 {
            let (p, q) = (D + 0x60 + 0x20 * k, D + 0x60 + 0x20 * (k + 1));
            if fx.project_n(&[p, q, p + 0x10, q + 0x10], sp + VP, d, sp + W, 0x01E0_0750) == 1 {
                let w = fx.mem.f(sp + W);
                let key = fixu(fx, w, 0x01E0_0764);
                fx.submit(d, OT, key as i32);
            }
        }
    }
    let age = fx.mem.i32(D + 0x284) + 1;
    fx.mem.set32(D + 0x284, age);
    if age >= 0x191 {
        fx.mem.write_u8(D + 0x280, 0);
    }
    f22
}

/// The ring records [F 0x1E007C4..0x1E00FD8].
fn rings(fx: &mut Fx, sp: u32, f22: f32) {
    let n = fx.mem.i32(D);
    let (a3, a7) = (f(0x3E99_999A), f(0x3F33_3333));
    if n > 0 && n < 0x190 {
        let g = D + RINGS + 0x3D0 * (n % 16) as u32;
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        for k in 0..0xCu32 {
            let r = rnd(fx, 0x01E0_0840) % 256;
            let life = fixs(fx, spread(r, a3, a7, 16.0), 0x01E0_08A4);
            fx.mem.set32(g + 8 + 4 * k, life);
            let r = rnd(fx, 0x01E0_08AC) % 256 + 1;
            fx.mem.setf(g + 0x38 + 4 * k, spread(r, 0.5, 0.5, 2.0));
            let r = rnd(fx, 0x01E0_0908) % 256 + 1;
            fx.mem.setf(g + 0x68 + 4 * k, spread(r, a3, a7, 25.0));
            let r = rnd(fx, 0x01E0_0974) % 256 + 1;
            fx.mem.setf(g + 0x98 + 4 * k, spread(r, 0.5, 0.5, 4.5));
            for (c, ra) in [(0u32, (0x01E0_09D0, 0x01E0_0A04)), (1, (0x01E0_0A38, 0x01E0_0A6C)), (2, (0x01E0_0AA0, 0x01E0_0AD4))] {
                let x = table_part(fx, c, ra);
                fx.mem.setf(sp + T + 4 * c, x);
            }
            fx.mem.write_u32(sp + T + 0xC, 0x3F80_0000);
            fx.point_along(g + 0xD0 + 0x40 * k, sp + T);
            fx.rotate_axis(g + 0xD0 + 0x40 * k, (((k % 6) << 16) as i32) / 6, 2);
        }
    }
    strip_head(fx, sp);
    let d = sp + ST;
    for r in 0..0x10u32 {
        let g = D + RINGS + 0x3D0 * r;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        for k in 0..0xCu32 {
            let age = fx.mem.i32(g + 4);
            let life = fx.mem.i32(g + 8 + 4 * k);
            if life < age {
                continue;
            }
            fx.camera_vp_to(sp + VP);
            translation_into(fx, sp + MS, D + 0x30 + 0x10 * (k / 6));
            mul_mem(fx, sp + MS, g + 0xD0 + 0x40 * k);
            mul_mem(fx, sp + VP, sp + MS);
            let tf = fx.tan_falloff_p(age, life, 1);
            let f25 = ee::sub(1.0, tf);
            let f24 = ee::add(1.0, ee::sub(1.0, f25));
            for s1 in 0..8i32 {
                let c = D + 0x288 + 4 * s1 as u32;
                let x = ee::from_i32(i32::from(fx.mem.read_u8(c + 3)));
                let a0 = fixu(fx, ee::mul(f22, ee::mul(x, f25)), 0x01E0_0CF8);
                let x = ee::from_i32(i32::from(fx.mem.read_u8(c + 7)));
                let a1 = fixu(fx, ee::mul(f22, ee::mul(x, f25)), 0x01E0_0D3C);
                for (o, s, al) in [(0x10u32, 0u32, a0), (0x40, 4, a1)] {
                    let rgb = [fx.mem.read_u8(c + s), fx.mem.read_u8(c + s + 1), fx.mem.read_u8(c + s + 2)];
                    rgba(fx, d + o, rgb, al as u8);
                }
                words(fx, d + 0x28, d + 0x10, 2);
                words(fx, d + 0x58, d + 0x40, 2);
                let t = s1 % 4;
                let t0 = ee::div(ee::mul(0.5, ee::from_i32(t)), 4.0);
                let t1 = ee::div(ee::mul(0.5, ee::from_i32(t + 1)), 4.0);
                strip_st(fx, sp, t0, t1);
                let f3 = fx.mem.f(g + 0x68 + 4 * k);
                let f26 = ee::mul(fx.mem.f(g + 0x38 + 4 * k), f25);
                let f21 = ee::div(ee::mul(f3, ee::from_i32(s1 + 1)), 8.0);
                let f20 = ee::div(ee::mul(f3, ee::from_i32(s1)), 8.0);
                let s = fx.tsin((s1 << 16) >> 3);
                let f27 = ee::mul(f24, ee::mul(fx.mem.f(g + 0x98 + 4 * k), s));
                let s = fx.tsin(((s1 + 1) << 16) >> 3);
                let f1 = ee::mul(f24, ee::mul(fx.mem.f(g + 0x98 + 4 * k), s));
                let f3 = ee::neg(f26);
                vec4(fx, sp + C0, [f27, ee::add(f3, f27), f20, 1.0]);
                vec4(fx, sp + C1, [f27, ee::add(f26, f27), f20, 1.0]);
                vec4(fx, sp + 0x2E0, [f1, ee::add(f3, f1), f21, 1.0]);
                vec4(fx, sp + 0x2F0, [f1, ee::add(f26, f1), f21, 1.0]);
                draw_w(fx, sp, sp + C0, d, 0x01E0_0F20);
            }
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// The spokes [F 0x1E00FDC..0x1E01590].
fn spokes(fx: &mut Fx, sp: u32, f23: f32) {
    let n = fx.mem.i32(D);
    let (a3, a7) = (f(0x3E99_999A), f(0x3F33_3333));
    if (0x51..0x190).contains(&n) {
        let g = D + SPOKES + 0x48 * (n % 16) as u32;
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 1);
        for k in 0..4u32 {
            let r = rnd(fx, 0x01E0_103C) % 256;
            let life = fixs(fx, spread(r, a3, a7, 16.0), 0x01E0_10A0);
            fx.mem.set32(g + 8 + 4 * k, life);
            let r = rnd(fx, 0x01E0_10A8) % 256;
            fx.mem.setf(g + 0x18 + 4 * k, spread(r, 0.5, 0.5, 3.0));
            let r = rnd(fx, 0x01E0_1100) % 256;
            fx.mem.setf(g + 0x28 + 4 * k, spread(r, 0.5, 0.5, 3.5));
            let v = phase(fx, 0x01E0_1158);
            fx.mem.set32(g + 0x38 + 4 * k, v & 0xFFFF);
        }
    }
    strip_head(fx, sp);
    let d = sp + ST;
    let n = (fx.mem.i32(D) - 0x50).max(0);
    let x = ee::from_i32(n);
    let f22 = ee::mul(Fx::ratio_below(x, 32.0), Fx::ramp_above(x, 320.0, 288.0));
    for r in 0..0x10u32 {
        let g = D + SPOKES + 0x48 * r;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        for k in 0..4u32 {
            let age = fx.mem.i32(g + 4);
            let life = fx.mem.i32(g + 8 + 4 * k);
            if life < age {
                continue;
            }
            let base = (r * 0x15B3 + k * 0x1A0A) as i32;
            fx.camera_vp_to(sp + VP);
            translation_into(fx, sp + MS, sp + MID);
            let a = fx.mem.i32(g + 0x38 + 4 * k);
            fx.rotate_axis(sp + MS, a, 0);
            mul_mem(fx, sp + VP, sp + MS);
            let tf = fx.tan_falloff_p(age, life, 1);
            let f21 = ee::sub(1.0, tf);
            let al = fixu(fx, ee::mul(ee::mul(255.0, f21), f22), 0x01E0_130C);
            rgba(fx, d + 0x10, [0xA4, 0x64, 0xFF], al as u8);
            let f20 = ee::add(1.0, ee::sub(1.0, f21));
            strip_one_colour(fx, sp);
            for s6 in 0..8i32 {
                let t = s6 % 4;
                let t0 = ee::div(ee::mul(0.5, ee::from_i32(t)), 4.0);
                let t1 = ee::div(ee::mul(0.5, ee::from_i32(t + 1)), 4.0);
                strip_st(fx, sp, t0, t1);
                let h = ee::div(f23, 2.0);
                let f26 = ee::mul(fx.mem.f(g + 0x18 + 4 * k), f21);
                let f25 = ee::sub(ee::div(ee::mul(f23, ee::from_i32(s6 + 1)), 8.0), h);
                let f24 = ee::sub(ee::div(ee::mul(f23, ee::from_i32(s6)), 8.0), h);
                let c = fx.tcos((((s6 << 16) >> 3) + base) & 0xFFFF);
                let f27 = ee::mul(f20, ee::mul(fx.mem.f(g + 0x28 + 4 * k), c));
                let c = fx.tcos(((((s6 + 1) << 16) >> 3) + base) & 0xFFFF);
                let f1 = ee::mul(f20, ee::mul(fx.mem.f(g + 0x28 + 4 * k), c));
                let f3 = ee::neg(f26);
                vec4(fx, sp + C0, [f24, ee::add(f3, f27), 0.0, 1.0]);
                vec4(fx, sp + C1, [f24, ee::add(f26, f27), 0.0, 1.0]);
                vec4(fx, sp + 0x2E0, [f25, ee::add(f3, f1), 0.0, 1.0]);
                vec4(fx, sp + 0x2F0, [f25, ee::add(f26, f1), 0.0, 1.0]);
                draw_w(fx, sp, sp + C0, d, 0x01E0_151C);
            }
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// A beam's spawn [F 0x1E01594..0x1E01B08].
fn beam_spawn(fx: &mut Fx) {
    let n = fx.mem.i32(D);
    if !(0xC9..0x159).contains(&n) {
        return;
    }
    if fx.mem.i32(D + 0x4430) % 10 == 0 {
        let i = fx.mem.i32(D + 0x4434);
        let g = D + BEAMS + 0x280 * i as u32;
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        words(fx, g + 0x10, D + 0x30 + 0x10 * ((i + 1) % 2) as u32, 4);
        for (c, ra) in [(0u32, (0x01E0_1680, 0x01E0_16B4)), (1, (0x01E0_1704, 0x01E0_1738)), (2, (0x01E0_1788, 0x01E0_17BC))] {
            let x = table_part(fx, c, ra);
            fx.mem.setf(g + 0x20 + 4 * c, x);
        }
        fx.mem.write_u32(g + 0x2C, 0x3F80_0000);
        fx.vu_add4(g + 0x20, g + 0x20, D + 0x50);
        fx.vu_sub4(g + 0x20, g + 0x20, g + 0x10);
        let x = Fx::powf2(fx.mem.f(g + 0x20));
        let y = Fx::powf2(fx.mem.f(g + 0x24));
        let z = Fx::powf2(fx.mem.f(g + 0x28));
        fx.mem.write_u64(fx.sp - 0x10, 0x01E0_1918);
        let l = fx.sqrtf(ee::add(z, ee::add(x, y)));
        fx.mem.setf(g + 0x30, l);
        for k in 0..8u32 {
            let r = rnd(fx, 0x01E0_194C) % 256;
            fx.mem.setf(g + 0x34 + 4 * k, spread(r, 0.5, 0.5, 8.0));
            let r = rnd(fx, 0x01E0_19C4) % 256;
            fx.mem.setf(g + 0x54 + 4 * k, spread(r, 0.5, 0.5, 6.0));
            fx.point_along(g + 0x80 + 0x40 * k, g + 0x20);
            let v = phase(fx, 0x01E0_1A6C);
            fx.rotate_axis(g + 0x80 + 0x40 * k, v, 2);
        }
        let i = i + 1;
        fx.mem.set32(D + 0x4434, if i < 0x18 { i } else { 0 });
    }
    let c = fx.mem.i32(D + 0x4430) + 1;
    fx.mem.set32(D + 0x4430, c);
}

/// A slot of a round robin: the record index at `at`, bumped.
fn next(fx: &mut Fx, at: u32, n: i32) -> u32 {
    let i = fx.mem.i32(at);
    fx.mem.set32(at, if i + 1 < n { i + 1 } else { 0 });
    i as u32
}

/// The beams [F 0x1E01B0C..0x1E023CC].
fn beams(fx: &mut Fx, sp: u32) {
    sprite_head(fx, sp, 0x14);
    strip_head(fx, sp);
    let (sd, d) = (sp + SD, sp + ST);
    for b in 0..0x18u32 {
        let g = D + BEAMS + 0x280 * b;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        camera(fx, sp);
        let age = fx.mem.i32(g + 4);
        let x = ee::from_i32(age);
        let al = fixu(fx, ee::mul(255.0, Fx::ramp_above(x, 24.0, 16.0)), 0x01E0_1C84);
        rgba(fx, sd + 0x10, [0xA4, 0x80, 0xFF], al as u8);
        uvs(fx, sp, 0, 0, 0x800, 0x800);
        corners(fx, sp, ee::mul(25.0, if age < 4 { 0.5 } else { 1.0 }));
        let f20 = Fx::ratio_below(x, 4.0);
        fx.vu_scale4(sp + T, g + 0x20, f20);
        fx.vu_add4(sp + T, sp + T, g + 0x10);
        if age == 4 {
            let i = next(fx, D + 0x8040, 0xC);
            let r = D + FLASHES + 0x20 * i;
            fx.mem.write_u8(r, 1);
            fx.mem.write_u32(r + 4, 0);
            let v = rnd(fx, 0x01E0_1E14) % 8;
            fx.mem.set32(r + 8, v);
            words(fx, r + 0x10, sp + T, 4);
            let i = next(fx, D + 0x81D0, 0x10);
            let r = D + SPARKS + 0x420 * i;
            fx.mem.write_u8(r, 1);
            fx.mem.write_u32(r + 4, 0);
            words(fx, r + 0x10, sp + T, 4);
            for k in 0..0x40u32 {
                fx.random_table_vec(r + 0x20 + 0x10 * k, 2.0);
            }
            let i = next(fx, D + 0xC4F0, 0x10);
            let r = D + DISCS + 0x20 * i;
            fx.mem.write_u8(r, 1);
            fx.mem.write_u32(r + 4, 0);
            words(fx, r + 0x10, sp + T, 4);
        }
        sprite_at(fx, sp, 0x01E0_2090);
        let s1 = fixs(fx, ee::mul(8.0, f20), 0x01E0_20B0).min(8);
        let age = fx.mem.i32(g + 4);
        let f21 = ee::sub(1.0, fx.tan_falloff_p(age, 0x18, 1));
        let al = fixu(fx, ee::mul(255.0, f21), 0x01E0_20F8);
        rgba(fx, d + 0x10, [0x60, 0x40, 0xFF], al as u8);
        strip_one_colour(fx, sp);
        let f20 = ee::add(1.0, ee::mul(1.5, ee::sub(1.0, f21)));
        let mut s4 = (b * 0x15B3) as i32;
        for fp in 0..8u32 {
            fx.camera_vp_to(sp + VP);
            fx.translate_rows(sp + MS, g + 0x80 + 0x40 * fp, g + 0x10);
            mul_mem(fx, sp + VP, sp + MS);
            for s6 in 0..s1 {
                let t = s6 % 3;
                let t0 = ee::div(ee::mul(0.5, ee::from_i32(t)), 3.0);
                let t1 = ee::div(ee::mul(0.5, ee::from_i32(t + 1)), 3.0);
                strip_st(fx, sp, t0, t1);
                let f4 = fx.mem.f(g + 0x30);
                let f22 = ee::div(ee::mul(f4, ee::from_i32(s6)), 8.0);
                let f23 = ee::div(ee::mul(f4, ee::from_i32(s6 + 1)), 8.0);
                let f24 = ee::mul(fx.mem.f(g + 0x34 + 4 * fp), f21);
                let c = fx.tcos((((s6 << 16) >> 3) + s4) & 0xFFFF);
                let f25 = ee::mul(f20, ee::mul(fx.mem.f(g + 0x54 + 4 * fp), c));
                let c = fx.tcos(((((s6 + 1) << 16) >> 3) + s4) & 0xFFFF);
                let f1 = ee::mul(f20, ee::mul(fx.mem.f(g + 0x54 + 4 * fp), c));
                let f3 = ee::neg(f24);
                vec4(fx, sp + Q, [0.0, ee::add(f3, f25), f22, 1.0]);
                vec4(fx, sp + 0x310, [0.0, ee::add(f24, f25), f22, 1.0]);
                vec4(fx, sp + 0x320, [0.0, ee::add(f3, f1), f23, 1.0]);
                vec4(fx, sp + 0x330, [0.0, ee::add(f24, f1), f23, 1.0]);
                draw_w(fx, sp, sp + Q, d, 0x01E0_2340);
            }
            s4 += 0x1A0A;
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x19 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// The flashes [F 0x1E023D0..0x1E026F4].
fn flashes(fx: &mut Fx, sp: u32) {
    sprite_head(fx, sp, 0x18);
    camera(fx, sp);
    let d = sp + SD;
    for k in 0..0xCu32 {
        let g = D + FLASHES + 0x20 * k;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 4);
        rgba(fx, d + 0x10, [0xFF, 0x20, 0x40], (((0xC - age) * 0xFF) / 12) as u8);
        let b = fx.mem.i32(g + 8);
        let (c, r) = ((b % 4) as u32, (b >> 2) as u32);
        uvs(fx, sp, c << 9, r << 9, (c + 1) << 9, (r + 1) << 9);
        let tf = fx.tan_falloff_p(age, 0xC, 2);
        corners(fx, sp, ee::mul(24.0, tf));
        words(fx, sp + T, g + 0x10, 4);
        sprite_at(fx, sp, 0x01E0_2694);
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0xD {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// The sparks [F 0x1E026F8..0x1E029DC].
fn sparks(fx: &mut Fx, sp: u32) {
    sprite_head(fx, sp, 0x14);
    camera(fx, sp);
    uvs(fx, sp, 0, 0, 0x800, 0x800);
    let d = sp + SD;
    for k in 0..0x10u32 {
        let g = D + SPARKS + 0x420 * k;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 4);
        rgba(fx, d + 0x10, [0xFF, 0x80, 0xA4], (((0x10 - age) * 0xFF) >> 4) as u8);
        corners(fx, sp, 2.5);
        let tf = fx.tan_falloff_p(age, 0x10, 1);
        let f21 = ee::add(f(0x3F33_3333), ee::mul(f(0x3E99_999A), tf));
        for j in 0..0x40u32 {
            fx.vu_scale4(sp + T, g + 0x20 + 0x10 * j, f21);
            fx.vu_add4(sp + T, sp + T, g + 0x10);
            sprite_at(fx, sp, 0x01E0_296C);
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// The discs [F 0x1E029E0..0x1E02C24].
fn discs(fx: &mut Fx, sp: u32) {
    let d = sp + 0x360;
    head(fx, d, 0x13, 6, 0x41_4141, 0);
    alpha(fx, d, 1);
    for k in 0..0x10u32 {
        let g = D + DISCS + 0x20 * k;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        fx.camera_vp_to(sp + VP);
        fx.translate_rows(sp + MS, sp + TURN, g + 0x10);
        let age = fx.mem.i32(g + 4);
        let tf = fx.tan_falloff_p(age, 0x10, 1);
        let s = ee::add(f(0x3F4C_CCCD), ee::mul(f(0x3E4C_CCCD), tf));
        vec4(fx, sp + T, [s, s, s, 1.0]);
        fx.scale_rows(sp + MS, sp + T);
        mul_mem(fx, sp + VP, sp + MS);
        let age = fx.mem.i32(g + 4);
        let x = ee::div(ee::from_i32(0x10 - age), 16.0);
        let al = fixu(fx, ee::mul(255.0, x), 0x01E0_2B00);
        rgba(fx, d + 0x10, [0xA4, 0x80, 0xFF], al as u8);
        rgba(fx, d + 0x20, [0x20, 0x20, 0xFF], 0);
        words(fx, d + 0x30, d + 0x20, 2);
        vec4(fx, sp + T, [0.0, 0.0, 0.0, 1.0]);
        for j in 0..0x10u32 {
            let p = D + CIRCLE + 0x10 * j;
            if fx.project_n(&[sp + T, p, p + 0x10], sp + VP, d, sp + W, 0x01E0_2BA4) == 1 {
                let w = fx.mem.f(sp + W);
                let key = fixu(fx, w, 0x01E0_2BB8);
                fx.submit(d, OT, key as i32);
            }
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(g, 0);
        }
    }
}
