//! BMODULE b431 (card 578), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xC450
//! bytes) [F], written cold from
//! the project notes (s0 = D, s1 = the ordering table, frame 0x550).
//!
//! Init: three textures, nodes 0x5D (kind 1) / 0x60 (kind 0), the target's
//! position D+0x70; a 41 x 41 sheet D+0x80 (rows 0x290, 450 i / 40 - 225)
//! with phases D+0x6990 (rows 0xA4) and four 17 x 2 wall strips D+0x83E0
//! (stride 0x220, y 0 / 100). Every update the unit flag (0x1F69C0(1)),
//! the camera turn with roll (sp + 0x230), the nodes into D+0x30 / D+0x40,
//! their midpoint D+0x50, the attacker's point D+0x60. On 0x7E for 0xF6
//! updates the pool: walls (gouraud, (FF, EB, 80)..) and the sheet (height
//! 2 sin(phase + 0x14D age), scrolling, drawn flat then textured). On
//! 0x11A..0x153 smoke records (16 x 0x170 at D+0x8C70) at the midpoint; on
//! 0x6A for 0x92 updates a glow at the point; on 0x6B..0xFA mote records (32
//! x 0xA0 at D+0xA380) at the point; on 0x137 a final burst at the target
//! (D+0xB780, 128 strips, a glow, 64 sprites); on 0x107 the frame sweep
//! (D+0x8C60). The stage light 0x80 - 96 clamp(n - 0x3C) clamp(0x188 - n)
//! >> 10. Returns 1 on 0x12D, 6 on 0x138..0x156, 2 on 0x157, 8 from 0x188.
use crate::b182::{alpha, head, rgba};
use crate::blib::exe::{mul_mem, translation_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, far_z};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x550;
const TURN: u32 = 0x230;
const VP: u32 = 0x1B0;
const MS: u32 = 0x1F0;
const C0: u32 = 0x270;
const C1: u32 = 0x280;
const Q0: u32 = 0x290;
const Q1: u32 = 0x2A0;
const AIM: u32 = 0x2D0;
const T: u32 = 0x310;
/// The sheet's descriptor (prim 0x24, 9 registers).
const SHEET_D: u32 = 0x3A8;
/// Its flat copy (prim 4, 5 registers).
const FLAT_D: u32 = 0x420;
/// The walls' descriptor (prim 0x14, 8 registers).
const WALL_D: u32 = 0x460;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const SPRITE_D: u32 = 0x350;
/// The strip descriptor (prim 0x34, 12 registers).
const STRIP_D: u32 = 0x4B8;
const W: u32 = 0x54C;

const SHEET: u32 = 0x80;
const PHASES: u32 = 0x6990;
const POOL: u32 = 0x83D4;
const WALLS: u32 = 0x83E0;
const SWEEP: u32 = 0x8C60;
const SMOKE: u32 = 0x8C70;
const GLOW: u32 = 0xA370;
const MOTES: u32 = 0xA380;
const BURST: u32 = 0xB780;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
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
fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

fn fixs(fx: &mut Fx, f: f32, ra: u32) -> i32 {
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}

fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 23
}

fn uv(fx: &mut Fx, a: u32, x: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (x & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    let d = sp + SPRITE_D;
    uv(fx, d + 0x18, u0);
    uv(fx, d + 0x1A, v0);
    uv(fx, d + 0x28, u1);
    uv(fx, d + 0x2A, v1);
}

/// The 32 x 32 atlas cell of byte `b` (4 across) into the sprite's UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: u32) {
    let (c, r) = (b % 4, b >> 2);
    uvs(fx, sp, c << 9, r << 9, (32 * c + 32) << 4, (32 * r + 32) << 4);
}

/// The sprite descriptor: texture `tex` (filter 1), ALPHA preset 1, CLAMP 5.
fn sprite_head(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + SPRITE_D;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    alpha(fx, d, 1);
    fx.desc_clamp(d, 5);
}

/// The strip descriptor: texture D+0x18 (filter 1), ALPHA preset 1, CLAMP 5.
fn strip_head(fx: &mut Fx, sp: u32) {
    let d = sp + STRIP_D;
    head(fx, d, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
}

/// The camera's view-projection (x identity).
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MS);
    mul_mem(fx, sp + VP, sp + MS);
}

/// The camera's view-projection x the identity translated by `t`.
fn camera_moved(fx: &mut Fx, sp: u32, t: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MS);
    fx.translate_rows(sp + MS, sp + MS, t);
    mul_mem(fx, sp + VP, sp + MS);
}

/// The corners (a, a, 0, 1) / (-a, -a, 0, 1) turned by the camera.
fn corners(fx: &mut Fx, sp: u32, a: f32) {
    vec4(fx, sp + C0, [a, a, 0.0, 1.0]);
    vec4(fx, sp + C1, [ee::neg(a), ee::neg(a), 0.0, 1.0]);
    fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
    fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
}

/// The sprite at `q` projected and drawn at the far Z.
fn sprite_far(fx: &mut Fx, sp: u32, q: u32) {
    let d = sp + SPRITE_D;
    if crate::fxlib2::project(fx, q, PROJ_OUT, sp + VP, d, sp + W) {
        far_z(fx, d + 0x20, &[d + 0x30]);
        fx.submit(d, OT, 0);
    }
}

/// sp + Q0 / Q1 = the corners + sp + T, drawn at the far Z.
fn sprite_at_t(fx: &mut Fx, sp: u32) {
    fx.vu_add4(sp + Q0, sp + C0, sp + T);
    fx.vu_add4(sp + Q1, sp + C1, sp + T);
    sprite_far(fx, sp, sp + Q0);
}

/// The strip's sides (0, h, 0, 1) / (0, -h, 0, 1) aimed along `v` (turned
/// by `a` about z) at `v`, stretched by `v` x `f`, its cell `b`'s STs.
fn strip_quad(fx: &mut Fx, sp: u32, v: u32, a: i32, h: f32, f: f32, b: u32) {
    fx.point_along(sp + AIM, v);
    fx.rotate_axis(sp + AIM, a, 2);
    vec4(fx, sp + C0, [0.0, h, 0.0, 1.0]);
    vec4(fx, sp + C1, [0.0, ee::neg(h), 0.0, 1.0]);
    fx.vu_apply_to(sp + C0, sp + AIM, sp + C0);
    fx.vu_apply_to(sp + C1, sp + AIM, sp + C1);
    fx.vu_add4(sp + Q0, sp + C0, v);
    fx.vu_add4(sp + Q1, sp + C1, v);
    fx.vu_scale4(sp + T, v, f);
    fx.vu_add4(sp + 0x2B0, sp + Q0, sp + T);
    fx.vu_add4(sp + 0x2C0, sp + Q1, sp + T);
    let c = (b % 4) << 5;
    let r = (b >> 2) << 5;
    let f5 = ee::from_i32(c as i32);
    let f2 = ee::from_i32(r as i32);
    let s0 = ee::div(f5, 128.0);
    let s1 = ee::div(ee::add(32.0, f5), 128.0);
    let t0 = ee::div(f2, 64.0);
    let t1 = ee::div(ee::add(32.0, f2), 64.0);
    let d = sp + STRIP_D;
    for (o, x) in [(0x18u32, s0), (0x1C, t0), (0x30, s1), (0x34, t0), (0x48, s0), (0x4C, t1), (0x60, s1), (0x64, t1)] {
        fx.mem.setf(d + o, x);
    }
}

fn project_strip(fx: &mut Fx, sp: u32) -> bool {
    let m = fx.read_m4(sp + VP);
    fx.project_desc(sp + Q0, PROJ_OUT, Some(&m), sp + STRIP_D, sp + W)
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xC450);
    let h = fx.self_handle();
    fx.mem.write_u32(D, 0);
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..3u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x5D, 1);
    fx.mem.set32(D + 0x1C, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x60, 0);
    fx.mem.set32(D + 0x20, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x70);
    for i in 0..0x29u32 {
        let x = ee::div(ee::mul(450.0, ee::from_i32(i as i32)), 40.0);
        for j in 0..0x29u32 {
            let z = ee::div(ee::mul(450.0, ee::from_i32(j as i32)), 40.0);
            let q = D + SHEET + 0x290 * i + 0x10 * j;
            vec4(fx, q, [ee::sub(x, 225.0), 0.0, ee::sub(z, 225.0), 1.0]);
            let r = rnd(fx, 0x01E0_01F8);
            fx.mem.set32(D + PHASES + 0xA4 * i + 4 * j, ((r % 256) << 16) >> 8);
        }
    }
    for i in 0..0x11u32 {
        let f = ee::sub(ee::div(ee::mul(450.0, ee::from_i32(i as i32)), 16.0), 225.0);
        let q = D + WALLS + 0x20 * i;
        vec4(fx, q, [f, 0.0, -225.0, 1.0]);
        vec4(fx, q + 0x220, [f, 0.0, 225.0, 1.0]);
        vec4(fx, q + 0x440, [-225.0, 0.0, f, 1.0]);
        vec4(fx, q + 0x660, [225.0, 0.0, f, 1.0]);
        for k in 0..4u32 {
            let p = q + 0x220 * k;
            words(fx, p + 0x10, p, 4);
            fx.mem.setf(p + 0x14, 100.0);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    crate::b369::unit_flag(fx, 1, u64::from(D), 0x01E0_0430);
    fx.camera_turn_roll(sp + TURN);
    for k in 0..2u32 {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x1C + 4 * k));
        fx.node_position(h, s, D + 0x30 + 0x10 * k);
    }
    fx.vu_add4(sp + T, D + 0x30, D + 0x40);
    fx.vu_scale4(D + 0x50, sp + T, 0.5);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x60);
    pool(fx, sp);
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(0x188 - n, 0, 0x20);
    let b = Fx::clamp_i(n - 0x3C, 0, 0x20);
    let l = (a.wrapping_mul((b - 4 * b) << 5) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);
    smoke(fx, sp);
    glow(fx, sp);
    motes(fx, sp);
    burst(fx, sp);
    sweep(fx, sp);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x12D {
        ret = 1;
    } else if (0x138..0x157).contains(&n) {
        ret = 6;
    } else if n == 0x157 {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x189 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// The walls and the sheet [F 0x1E0049C..0x1E00C38].
fn pool(fx: &mut Fx, sp: u32) {
    let (on, ag) = (D + POOL, D + POOL + 4);
    if fx.mem.i32(D) == 0x7E {
        fx.mem.write_u8(on, 1);
        fx.mem.write_u32(ag, 0);
    }
    if fx.mem.read_u8(on) != 1 {
        return;
    }
    let sd = sp + SHEET_D;
    head(fx, sd, 0x24, 9, 0x4_2424_2421, 0);
    alpha(fx, sd, 0);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(sd, rh, 1);
    fx.desc_clamp(sd, 0);
    let fd = sp + FLAT_D;
    head(fx, fd, 4, 5, 0x4_4441, 0);
    alpha(fx, fd, 1);
    let wd = sp + WALL_D;
    head(fx, wd, 0x14, 8, 0x4141_4141, 0);
    alpha(fx, wd, 1);
    let age = fx.mem.i32(ag);
    let f = ee::from_i32(age);
    let f20 = ee::mul(Fx::ratio_below(f, 128.0), Fx::ramp_above(f, 245.0, 213.0));
    let a = ee::from_i32(Fx::clamp_i(age, 0, 0x18));
    let b = ee::from_i32(Fx::clamp_i(0x7D - age, 0, 0x18));
    let f3 = ee::div(ee::mul(a, b), 576.0);
    let f2 = ee::from_i32(age % 2);
    let f1 = ee::mul(f32::from_bits(0x3DCC_CCCD), f2);
    let f21 = ee::mul(ee::add(f32::from_bits(0x3F66_6666), f1), f3);
    let al = fixu(fx, ee::mul(128.0, f21), 0x01E0_06C4);
    rgba(fx, fd + 0x10, [0xFF, 0xEB, 0x80], al as u8);
    let al = fixu(fx, ee::mul(255.0, f21), 0x01E0_06F8);
    rgba(fx, wd + 0x10, [0xFF, 0xE8, 0xD8], al as u8);
    words(fx, wd + 0x20, wd + 0x10, 2);
    rgba(fx, wd + 0x30, [0, 0, 0], 0);
    words(fx, wd + 0x40, wd + 0x30, 2);
    vec4(fx, sp + T, [0.0, 55.0, 0.0, 1.0]);
    camera_moved(fx, sp, sp + T);
    for s in 0..4u32 {
        let g = D + WALLS + 0x220 * s;
        for k in 0..0x10u32 {
            let (p, q) = (g + 0x20 * k, g + 0x20 * (k + 1));
            let vs = [p, q, p + 0x10, q + 0x10];
            if fx.project_n(&vs, sp + VP, wd, sp + W, 0x01E0_07FC) == 1 {
                let w = fx.mem.f(sp + W);
                let key = fixu(fx, w, 0x01E0_0810);
                fx.submit(wd, OT, key as i32);
            }
        }
    }
    let f20 = ee::mul(70.0, f20);
    vec4(fx, sp + T, [0.0, 59.0, 0.0, 1.0]);
    camera_moved(fx, sp, sp + T);
    let al = fixu(fx, f20, 0x01E0_08E8);
    rgba(fx, sd + 0x10, [0x8C, 0x8C, 0x8C], al as u8);
    for i in 0..0x29u32 {
        for j in 0..0x29u32 {
            let ph = fx.mem.i32(D + PHASES + 0xA4 * i + 4 * j);
            let age = fx.mem.i32(ag);
            let s = fx.tsin(ph.wrapping_add(age.wrapping_mul(0x14D)) & 0xFFFF);
            fx.mem.setf(D + SHEET + 0x290 * i + 0x10 * j + 4, ee::mul(2.0, s));
        }
    }
    let age = fx.mem.i32(ag);
    let a = ((age % 128) << 16) >> 7;
    let s2 = a.wrapping_neg() & 0xFFFF;
    let f21 = ee::mul(f32::from_bits(0x3DCC_CCCD), fx.tsin(s2));
    let f22 = ee::mul(f32::from_bits(0x3DCC_CCCD), fx.tcos(s2));
    for i in 0..0x28u32 {
        let v = (i % 2) as i32;
        let f23 = ee::add(ee::div(ee::from_i32(v), 2.0), f22);
        let f24 = ee::add(ee::div(ee::from_i32(v + 1), 2.0), f22);
        let (r0, r1) = (D + SHEET + 0x290 * i, D + SHEET + 0x290 * (i + 1));
        for j in 0..0x28u32 {
            let c = (j % 2) as i32;
            let u0 = ee::add(ee::div(ee::from_i32(c), 2.0), f21);
            let u1 = ee::add(ee::div(ee::from_i32(c + 1), 2.0), f21);
            for (o, x) in [(0x18u32, u0), (0x1C, f23), (0x28, u1), (0x2C, f23), (0x38, u0), (0x3C, f24), (0x48, u1), (0x4C, f24)] {
                fx.mem.setf(sd + o, x);
            }
            let vs = [r0 + 0x10 * j, r0 + 0x10 * (j + 1), r1 + 0x10 * j, r1 + 0x10 * (j + 1)];
            if fx.project_n(&vs, sp + VP, sd, sp + W, 0x01E0_0B2C) == 1 {
                for k in 0..4u32 {
                    let x = fx.mem.read_u64(sd + 0x20 + 0x10 * k);
                    fx.mem.write_u64(fd + 0x18 + 8 * k, x);
                }
                let w = fx.mem.f(sp + W);
                let key = fixu(fx, w, 0x01E0_0B80);
                fx.submit(fd, OT, key as i32);
                let w = fx.mem.f(sp + W);
                let key = fixu(fx, w, 0x01E0_0B98);
                fx.submit(sd, OT, key as i32);
            }
        }
    }
    let age = fx.mem.i32(ag) + 1;
    fx.mem.set32(ag, age);
    if age >= 0xF6 {
        fx.mem.write_u8(on, 0);
    }
}

/// The smoke [F 0x1E00C8C..0x1E011E8].
fn smoke(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x11A..0x154).contains(&n) {
        let g = D + SMOKE + 0x170 * (n % 16) as u32;
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        words(fx, g + 0x60, D + 0x50, 4);
        for k in 0..0x10u32 {
            let b = rnd(fx, 0x01E0_0D2C) % 8;
            fx.mem.set8(g + 8 + k, b);
            let s = if fx.mem.read_u8(g + 8 + k) < 5 { 0.5 } else { 1.0 };
            fx.mem.setf(g + 0x18 + 4 * k, s);
            fx.random_table_vec(g + 0x70 + 0x10 * k, 1.0);
        }
    }
    sprite_head(fx, sp, 0x14);
    camera(fx, sp);
    let d = sp + SPRITE_D;
    for r in 0..0x10u32 {
        let g = D + SMOKE + 0x170 * r;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 4);
        let c = fx.tcos((age << 14) >> 4);
        let al = fixu(fx, ee::mul(100.0, c), 0x01E0_0EAC);
        rgba(fx, d + 0x10, [0x78, 0x78, 0xA0], al as u8);
        let f = ee::from_i32(fx.mem.i32(g + 4));
        let f20 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), f), 16.0));
        for k in 0..0x10u32 {
            let f = ee::from_i32(fx.mem.i32(g + 4));
            let f3 = ee::div(ee::mul(f32::from_bits(0x3F66_6666), f), 16.0);
            let f1 = ee::mul(12.5, ee::add(f32::from_bits(0x3DCC_CCCD), f3));
            let s = fx.mem.f(g + 0x18 + 4 * k);
            corners(fx, sp, ee::mul(f1, s));
            let b = u32::from(fx.mem.read_u8(g + 8 + k));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + T, g + 0x70 + 0x10 * k, f20);
            fx.vu_add4(sp + T, sp + T, g + 0x60);
            sprite_at_t(fx, sp);
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// The glow [F 0x1E011EC..0x1E01564].
fn glow(fx: &mut Fx, sp: u32) {
    let (on, ag) = (D + GLOW, D + GLOW + 4);
    if fx.mem.i32(D) == 0x6A {
        fx.mem.write_u8(on, 1);
        fx.mem.write_u32(ag, 0);
    }
    if fx.mem.read_u8(on) != 1 {
        return;
    }
    sprite_head(fx, sp, 0x18);
    camera(fx, sp);
    let age = fx.mem.i32(ag);
    let f = ee::from_i32(age);
    let f3 = ee::mul(Fx::ratio_below(f, 16.0), Fx::ramp_above(f, 145.0, 129.0));
    let f2 = ee::from_i32(age % 2);
    let f1 = ee::mul(f32::from_bits(0x3E4C_CCCD), f2);
    let f1 = ee::mul(ee::add(f32::from_bits(0x3F4C_CCCD), f1), f3);
    let al = fixu(fx, ee::mul(255.0, f1), 0x01E0_1364);
    rgba(fx, sp + SPRITE_D + 0x10, [0x62, 0x10, 0xFF], al as u8);
    corners(fx, sp, 30.0);
    uvs(fx, sp, 0, 0x200, 0x200, 0x400);
    words(fx, sp + T, D + 0x60, 4);
    sprite_at_t(fx, sp);
    let age = fx.mem.i32(ag) + 1;
    fx.mem.set32(ag, age);
    if age >= 0x92 {
        fx.mem.write_u8(on, 0);
    }
}

/// The motes [F 0x1E01568..0x1E01A60].
fn motes(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x6B..0xFB).contains(&n) {
        let g = D + MOTES + 0xA0 * (n % 32) as u32;
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        words(fx, g + 0x10, D + 0x60, 4);
        for k in 0..8u32 {
            let b = rnd(fx, 0x01E0_15FC) % 8;
            fx.mem.set8(g + 8 + k, b);
            fx.random_table_vec(g + 0x20 + 0x10 * k, f32::from_bits(0x3E4C_CCCD));
        }
    }
    strip_head(fx, sp);
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n - 0x6A, 0, 0x10));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(0xFB - n, 0, 0x10));
    let f20 = ee::div(ee::mul(a, b), 256.0);
    let d = sp + STRIP_D;
    for r in 0..0x20u32 {
        let g = D + MOTES + 0xA0 * r;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 4);
        let x = ee::from_i32(((0x20 - age) << 7) >> 5);
        let al = fixu(fx, ee::mul(f20, x), 0x01E0_1768);
        rgba(fx, d + 0x10, [0x9B, 0x74, 0xC4], al as u8);
        for o in [0x28u32, 0x40, 0x58] {
            words(fx, d + o, d + 0x10, 2);
        }
        fx.camera_vp_to(sp + VP);
        translation_into(fx, sp + MS, g + 0x10);
        mul_mem(fx, sp + VP, sp + MS);
        let age = fx.mem.i32(g + 4);
        let f21 = ee::mul(-20.0, fx.tan_falloff_p(age, 0x20, 1));
        let base = ((r << 16) as i32) >> 5;
        for k in 0..8u32 {
            let b = u32::from(fx.mem.read_u8(g + 8 + k));
            strip_quad(fx, sp, g + 0x20 + 0x10 * k, base + (((k << 16) as i32) >> 3), 2.0, f21, b);
            if project_strip(fx, sp) {
                let w = fx.mem.f(sp + W);
                let key = fixu(fx, w, 0x01E0_19C8);
                fx.submit(d, OT, key as i32);
            }
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// The final burst [F 0x1E01A64..0x1E02634].
fn burst(fx: &mut Fx, sp: u32) {
    let (on, ag) = (D + BURST, D + BURST + 4);
    if fx.mem.i32(D) == 0x137 {
        fx.mem.write_u8(on, 1);
        fx.mem.write_u32(ag, 0);
        for k in 0..0x80u32 {
            let b = rnd(fx, 0x01E0_1AA0) % 8;
            fx.mem.set8(D + 0xB788 + k, b);
            fx.random_table_vec(D + 0xB810 + 0x10 * k, 5.0);
        }
        for k in 0..0x40u32 {
            let b = rnd(fx, 0x01E0_1AFC) % 4 + 4;
            fx.mem.set8(D + 0xC010 + k, b);
            fx.random_table_vec(D + 0xC050 + 0x10 * k, 2.5);
        }
    }
    if fx.mem.read_u8(on) != 1 {
        return;
    }
    strip_head(fx, sp);
    let d = sp + STRIP_D;
    let f = ee::from_i32(fx.mem.i32(ag));
    let al = fixu(fx, ee::mul(128.0, Fx::ramp_above(f, 32.0, 16.0)), 0x01E0_1C04);
    rgba(fx, d + 0x10, [0xC8, 0xAA, 0x96], al as u8);
    words(fx, d + 0x28, d + 0x10, 2);
    rgba(fx, d + 0x40, [0xC8, 0xAA, 0x96], 0);
    words(fx, d + 0x58, d + 0x40, 2);
    fx.camera_vp_to(sp + VP);
    translation_into(fx, sp + MS, D + 0x70);
    let age = fx.mem.i32(ag);
    let f0 = fx.tan_falloff_p(age, 0x20, 1);
    let f0 = ee::add(0.5, ee::mul(0.5, f0));
    vec4(fx, sp + T, [f0, f0, f0, 1.0]);
    fx.scale_rows(sp + MS, sp + T);
    mul_mem(fx, sp + VP, sp + MS);
    for k in 0..0x80u32 {
        let b = u32::from(fx.mem.read_u8(D + 0xB788 + k));
        strip_quad(fx, sp, D + 0xB810 + 0x10 * k, ((k << 16) as i32) >> 7, 4.0, -2.0, b);
        if project_strip(fx, sp) {
            far_z(fx, d + 0x20, &[d + 0x38, d + 0x50, d + 0x68]);
            fx.submit(d, OT, 0);
        }
    }
    let sd = sp + SPRITE_D;
    sprite_head(fx, sp, 0x18);
    let f = ee::from_i32(fx.mem.i32(ag));
    let al = fixu(fx, ee::mul(128.0, Fx::ramp_above(f, 32.0, 16.0)), 0x01E0_2028);
    rgba(fx, sd + 0x10, [0x92, 0xC8, 0xFF], al as u8);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    camera_moved(fx, sp, D + 0x70);
    let age = fx.mem.i32(ag);
    let f0 = fx.tan_falloff_p(age, 0x20, 1);
    let f1 = ee::add(0.5, ee::mul(0.5, f0));
    corners(fx, sp, ee::mul(120.0, f1));
    sprite_far(fx, sp, sp + C0);
    sprite_head(fx, sp, 0x14);
    let f = ee::from_i32(fx.mem.i32(ag));
    let al = fixu(fx, ee::mul(164.0, Fx::ramp_above(f, 32.0, 16.0)), 0x01E0_22E4);
    rgba(fx, sd + 0x10, [0x78, 0x78, 0xA0], al as u8);
    let f = ee::from_i32(fx.mem.i32(ag));
    let f2 = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), f), 32.0);
    corners(fx, sp, ee::mul(15.0, ee::add(f32::from_bits(0x3E4C_CCCD), f2)));
    camera_moved(fx, sp, D + 0x70);
    let age = fx.mem.i32(ag);
    let t = fx.tan_falloff_p(age, 0x20, 1);
    let f20 = (f64::from_bits(0x3FB9_9999_A000_0000) + f64::from_bits(0x3FEC_CCCC_CCCC_CCCD) * f64::from(t)) as f32;
    for k in 0..0x40u32 {
        let b = u32::from(fx.mem.read_u8(D + 0xC010 + k));
        cell_uvs(fx, sp, b);
        fx.vu_scale4(sp + T, D + 0xC050 + 0x10 * k, f20);
        sprite_at_t(fx, sp);
    }
    let age = fx.mem.i32(ag) + 1;
    fx.mem.set32(ag, age);
    if age >= 0x21 {
        fx.mem.write_u8(on, 0);
    }
}

/// The frame sweep [F 0x1E02638..0x1E0280C].
fn sweep(fx: &mut Fx, sp: u32) {
    let (on, ag) = (D + SWEEP, D + SWEEP + 4);
    if fx.mem.i32(D) == 0x107 {
        fx.mem.write_u8(on, 1);
        fx.mem.write_u32(ag, 0);
    }
    if fx.mem.read_u8(on) != 1 {
        return;
    }
    let d = sp + 0x320;
    head(fx, d, 6, 3, 0x441, 0);
    alpha(fx, d, 0);
    let f = ee::from_i32(fx.mem.i32(ag));
    let al = fixu(fx, ee::mul(64.0, Fx::ramp_above(f, 108.0, 93.0)), 0x01E0_26E4);
    rgba(fx, d + 0x10, [0x1C, 0x30, 0x60], al as u8);
    let f = ee::from_i32(fx.mem.i32(ag));
    let r = ee::sub(1.0, Fx::ratio_below(f, 15.0));
    let y = fixs(fx, ee::mul(512.0, r), 0x01E0_2744);
    far_xy(fx, d + 0x18, 0x7000, ((y + 0x700) << 4) as u16);
    far_xy(fx, d + 0x20, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
    let age = fx.mem.i32(ag) + 1;
    fx.mem.set32(ag, age);
    if age >= 0x6D {
        fx.mem.write_u8(on, 0);
    }
}
