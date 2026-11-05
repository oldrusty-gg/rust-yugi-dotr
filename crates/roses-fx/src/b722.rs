//! BMODULE b722 (card 505), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0x68F0
//! bytes) [F], written cold from the project notes
//! (s0 = D, s1 = the ordering table, frame 0x6A0).
//!
//! Init: textures (4, 0xF), (5, 0..2), nodes 0x23 (kind 1) / 0x24 / 0x46 /
//! 0x4C, the target's position D+0xC0, a colour ramp of 11 (D+0x108, (AA,
//! FF, 46, 80) to (32, 80, 1E, 0)), a cone of 8 x 16 strips D+0x4320 (0x40
//! each, cells D+0x6320), a diamond table D+0x6888.
//! Every update the camera turn with roll (sp + 0x370), nodes into D+0x30 /
//! D+0x40 / D+0xA0 / D+0xB0, the aim D+0x50, the target's screen point
//! D+0xD0, the point D+0xE0 flying from node 0x24 to the target on
//! 0x17F..0x191, its scale D+0xF0 growing from 0x191. On 0x191 the fan lens
//! (75 updates) about the screen point; on 0x12A a glow at D+0xE0 (0xA8
//! updates); on 0x12B..0x1D0 ring records (16 x 0x3D0 at D+0x140: 12 turned
//! rings of 10 colour steps); on 0x17F the hoop D+0x3E50 (16 ribbons along
//! the flight, 0x41 updates); on 0xAA the cone D+0x63A0 (0xFB updates) with
//! a glow; on 0xB2 / 0xCC sparks at node 0x46 / 0x4C (2 x 0x240 at
//! D+0x6400); on 0x38 / 0x4A / 0x82 three green diamonds on the window until
//! 0x91, which puts them out with a window flash D+0x68E8.
//! The stage light 0x80 - 80 clamp(n) clamp(0x1FB - n) >> 10. Returns 1 on
//! 0x187, 6 on 0x192..0x1D0, 2 on 0x1D1, 8 from 0x1FB.
use crate::b182::{alpha, head, rgba};
use crate::blib::exe::{mul_mem, translation_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, far_z, FAR_Z, KEEP, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The fan's nine screen points (exe rodata, 0x10 apart).
const FAN: u32 = 0x0028_ABE0;
const FRAME: u32 = 0x6A0;
const VP: u32 = 0x2F0;
const MS: u32 = 0x330;
const TURN: u32 = 0x370;
const C0: u32 = 0x3B0;
const C1: u32 = 0x3C0;
const Q: u32 = 0x3F0;
const T: u32 = 0x470;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const SD: u32 = 0x638;
/// The strip descriptor (prim 0x34, 12 registers).
const ST: u32 = 0x5A8;
/// The diamonds' descriptor (prim 0x24, 9 registers).
const QD: u32 = 0x4B8;
const W: u32 = 0x69C;
const RINGS: u32 = 0x140;
const HOOP: u32 = 0x3E50;
const CONE_V: u32 = 0x4320;
const CONE_B: u32 = 0x6320;
const CONE: u32 = 0x63A0;
const SPARKS: u32 = 0x6400;
const LENS: u32 = 0x6880;
const DIAMONDS: u32 = 0x6888;
const FLASHES: u32 = 0x68D8;
const WINDOW: u32 = 0x68E8;
const R_FAN: [u32; 12] = [
    0x01E0_09B8, 0x01E0_09C4, 0x01E0_0A08, 0x01E0_0A14, 0x01E0_0A58, 0x01E0_0A64, 0x01E0_0B18, 0x01E0_0B24, 0x01E0_0B90, 0x01E0_0B9C, 0x01E0_0C08,
    0x01E0_0C14,
];

fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

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

fn fix_at(fx: &mut Fx, a: u32, ra: u32) -> i32 {
    let v = fx.mem.f(a);
    fixs(fx, v, ra)
}

fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 23
}

/// rand >> 23 as a phase: ((r % 256) << 16) >> 8.
fn phase(fx: &mut Fx, ra: u32) -> i32 {
    let r = rnd(fx, ra);
    ((r % 256) << 16) >> 8
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

/// The sprite descriptor: texture `tex` (filter 1), CLAMP 5, ALPHA `preset`.
fn sprite_head(fx: &mut Fx, sp: u32, tex: u32, preset: i32) {
    let d = sp + SD;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    if preset == 0 {
        alpha(fx, d, 0);
    }
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    if preset != 0 {
        alpha(fx, d, preset);
    }
}

/// The strip descriptor: ALPHA preset 1, texture `tex` (filter 1), `clamp`.
fn strip_head(fx: &mut Fx, sp: u32, tex: u32, clamp: u64) {
    let d = sp + ST;
    head(fx, d, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, clamp);
}

const RIBBON_CLAMP: u64 = (0x1FC << 32) | 0x0123_40BA;

/// The strip's four STs.
fn strip_st(fx: &mut Fx, sp: u32, s0: f32, t0: f32, s1: f32, t1: f32) {
    let d = sp + ST;
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

/// The vertices at `q` through 0x204F70 to the descriptor `d`, drawn keyed
/// by the mean w.
fn draw_w(fx: &mut Fx, sp: u32, q: u32, d: u32, ra: u32) {
    if crate::fxlib2::project(fx, q, PROJ_OUT, sp + VP, d, sp + W) {
        let w = fx.mem.f(sp + W);
        let key = fixu(fx, w, ra);
        fx.submit(d, OT, key as i32);
    }
}

/// The sprite at `q` at the far Z.
fn sprite_far(fx: &mut Fx, sp: u32, q: u32) {
    let d = sp + SD;
    if crate::fxlib2::project(fx, q, PROJ_OUT, sp + VP, d, sp + W) {
        far_z(fx, d + 0x20, &[d + 0x30]);
        fx.submit(d, OT, 0);
    }
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0x68F0);
    let h = fx.self_handle();
    fx.mem.write_u32(D, 0);
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    let r = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x10, r);
    for k in 0..3u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x14 + 4 * k, r);
    }
    for (slot, node, kind) in [(0x20u32, 0x23, 1), (0x24, 0x24, 0), (0x90, 0x46, 0), (0x94, 0x4C, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, kind);
        fx.mem.set32(D + slot, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0xC0);
    for k in 0..0xBu32 {
        let f20 = ee::div(ee::from_i32(k as i32), 10.0);
        let f21 = ee::sub(1.0, f20);
        let c = D + 0x108 + 4 * k;
        for (o, a, b, ra) in [(0u32, 170.0, 50.0, 0x01E0_0200u32), (1, 255.0, 128.0, 0x01E0_0220), (2, 70.0, 30.0, 0x01E0_0240)] {
            let v = ee::vmadd(ee::vmul(a, f21), b, f20);
            let x = fixu(fx, v, ra);
            fx.mem.write_u8(c + o, x as u8);
        }
        let x = fixu(fx, ee::mul(128.0, f21), 0x01E0_0254);
        fx.mem.write_u8(c + 3, x as u8);
    }
    for fp in 0..8i32 {
        let f20 = ee::div(ee::mul(-20.0, ee::from_i32(fp + 1)), 8.0);
        let f22 = ee::div(ee::mul(-20.0, ee::from_i32(fp)), 7.0);
        let f21 = ee::div(ee::mul(14.0, ee::from_i32(7 - fp)), 7.0);
        let s7 = (fp * 0x2000) >> 3;
        for s1 in 0..0x10i32 {
            let a = (((s1 << 16) >> 4) + s7) & 0xFFFF;
            let x = ee::mul(f21, fx.tsin(a));
            fx.mem.setf(sp + T, x);
            let y = ee::mul(f21, fx.tcos(a));
            fx.mem.setf(sp + T + 4, y);
            fx.mem.setf(sp + T + 8, f22);
            fx.mem.write_u32(sp + T + 0xC, 0x3F80_0000);
            fx.point_along(sp + 0x430, sp + T);
            let v = phase(fx, 0x01E0_0394);
            fx.rotate_axis(sp + 0x430, v, 2);
            let g = D + CONE_V + 0x400 * fp as u32 + 0x40 * s1 as u32;
            vec4(fx, g, [-2.5, 0.0, 0.0, 1.0]);
            vec4(fx, g + 0x10, [2.5, 0.0, 0.0, 1.0]);
            vec4(fx, g + 0x20, [-2.5, 0.0, f20, 1.0]);
            vec4(fx, g + 0x30, [2.5, 0.0, f20, 1.0]);
            for k in 0..4u32 {
                fx.vu_apply_to(g + 0x10 * k, sp + 0x430, g + 0x10 * k);
            }
            let r = rnd(fx, 0x01E0_0440);
            fx.mem.set8(D + CONE_B + 0x10 * fp as u32 + s1 as u32, r % 4 + 4);
        }
    }
    let g = D + DIAMONDS;
    for (o, x, y) in [(0u32, -0x53, -0xB6), (0x10, 0x53, -0xB6), (0x20, 0x53, 0xB6), (0x30, -0x53, 0xB6), (0x40, -0x53, -0xB6)] {
        fx.mem.set32(g + o, x);
        fx.mem.set32(g + o + 4, y);
    }
    for (o, x, y) in [(8u32, -0x13, -0x76), (0x18, 0x13, -0x76), (0x28, 0x13, 0x76), (0x38, -0x13, 0x76), (0x48, -0x13, -0x76)] {
        fx.mem.set32(g + o, x);
        fx.mem.set32(g + o + 4, y);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    for k in 0..2u32 {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x20 + 4 * k));
        fx.node_position(h, s, D + 0x30 + 0x10 * k);
    }
    fx.vu_sub4(sp + T, D + 0x40, D + 0x30);
    fx.point_along(D + 0x50, sp + T);
    for k in 0..2u32 {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x90 + 4 * k));
        fx.node_position(h, s, D + 0xA0 + 0x10 * k);
    }
    camera(fx, sp);
    screen_point(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(n - 0x17F, 0, 0x12)), 18.0);
    fx.vu_sub4(sp + T, D + 0xC0, D + 0x40);
    fx.vu_scale4(sp + T, sp + T, f20);
    fx.vu_add4(D + 0xE0, sp + T, D + 0x40);
    let b = ee::from_i32(Fx::clamp_i(n - 0x191, 0, 0x40));
    let s = ee::add(1.0, ee::div(ee::mul(4.5, b), 64.0));
    vec4(fx, D + 0xF0, [s, s, s, 1.0]);
    lens(fx, sp);
    let f21 = glow(fx, sp);
    rings(fx, sp, f21);
    hoop(fx, sp);
    cone(fx, sp);
    sparks(fx, sp);
    diamonds(fx, sp);
    window(fx, sp);
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(0x1FB - n, 0, 0x20);
    let b = Fx::clamp_i(n, 0, 0x20);
    let l = (a.wrapping_mul((-b * 4 - b) << 4) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);
    let mut ret = 0;
    if n == 0x187 {
        ret = 1;
    } else if (0x192..0x1D1).contains(&n) {
        ret = 6;
    } else if n == 0x1D1 {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x1FC {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// The target's screen point D+0xD0: x / y clamped to 0..512, Z, 1.0.
fn screen_point(fx: &mut Fx, sp: u32) {
    let m = fx.read_m4(sp + VP);
    let o = D + 0xD0;
    fx.project_at(D + 0xC0, &m, sp + 0x690, sp + W);
    let x = (i32::from(fx.mem.read_u16(sp + 0x690)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x692)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x690) << 8) >> 40;
    fx.mem.setf(o + 8, ee::from_i32(z as i32));
    fx.mem.write_u32(o + 0xC, 0x3F80_0000);
    for k in [0u32, 4] {
        if crate::exe::ee_lt(fx.mem.f(o + k), 0.0) {
            fx.mem.write_u32(o + k, 0);
        }
    }
    for k in [0u32, 4] {
        if !crate::exe::ee_le(fx.mem.f(o + k), 512.0) {
            fx.mem.setf(o + k, 512.0);
        }
    }
}

/// An XYZ2 at `p`: x / y halfwords of pixel (x, y), the far Z and a zero
/// top byte.
fn xyz_far(fx: &mut Fx, p: u32, x: i32, y: i32) {
    fx.mem.write_u16(p, ((x + 0x700) << 4) as u16);
    fx.mem.write_u16(p + 2, ((y + 0x700) << 4) as u16);
    let v = fx.mem.read_u64(p);
    fx.mem.write_u64(p, (v & KEEP) | FAR_Z);
    fx.mem.write_u8(p + 7, 0);
}

/// The lens (75 updates): b088's: the frame copied in a packet layer, 8
/// rings of the fan about the screen point [F 0x1E007A0..0x1E00CE0].
fn lens(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x191 {
        fx.mem.write_u8(D + LENS, 1);
        fx.mem.write_u32(D + LENS + 4, 0);
    }
    if fx.mem.read_u8(D + LENS) != 1 {
        return;
    }
    let d = sp + 0x530;
    head(fx, d, 0x33, 9, 0x4_2142_1421, 0x800);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let a = [u32::from(fx.mem.read_u16(d + 2)), LENS_CLAMP as u32, d + 0x10, d + 2];
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xDC, layer);
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x590, sp + 0x598);
    fx.texflush(regs);
    let age = fx.mem.i32(D + LENS + 4);
    let al = ((0x4A - age) * 0x50 / 0x4A) as u8;
    rgba(fx, sp + 0x540, [0xFF; 3], al);
    words(fx, sp + 0x558, sp + 0x540, 2);
    words(fx, sp + 0x570, sp + 0x540, 2);
    let c = D + 0xD0;
    let (q0, q1) = (sp + 0x3F0, sp + 0x400);
    for i in 0..8i32 {
        let f20 = ee::add(1.0, ee::div(ee::mul(0.25, ee::from_i32(8 - i)), 8.0));
        for k in 0..8u32 {
            let s0 = FAN + 0x10 * k;
            let x = fix_at(fx, c, R_FAN[0]);
            let y = fix_at(fx, c + 4, R_FAN[1]);
            uv(fx, sp + 0x548, ((x << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x54A, ((y << 4) & 0x3FFF) as u32);
            let x = fix_at(fx, s0, R_FAN[2]);
            let y = fix_at(fx, s0 + 4, R_FAN[3]);
            uv(fx, sp + 0x560, ((x << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x562, ((y << 4) & 0x3FFF) as u32);
            let x = fix_at(fx, s0 + 0x10, R_FAN[4]);
            let y = fix_at(fx, s0 + 0x14, R_FAN[5]);
            uv(fx, sp + 0x578, ((x << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x57A, ((y << 4) & 0x3FFF) as u32);
            fx.vu_sub4(q0, s0, c);
            fx.vu_sub4(q1, s0 + 0x10, c);
            fx.vu_scale4(q0, q0, f20);
            fx.vu_scale4(q1, q1, f20);
            fx.vu_add4(q0, q0, c);
            fx.vu_add4(q1, q1, c);
            let x = fix_at(fx, c, R_FAN[6]);
            let y = fix_at(fx, c + 4, R_FAN[7]);
            xyz_far(fx, sp + 0x550, x, y);
            let x = fix_at(fx, q0, R_FAN[8]);
            let y = fix_at(fx, q0 + 4, R_FAN[9]);
            xyz_far(fx, sp + 0x568, x, y);
            let x = fix_at(fx, q1, R_FAN[10]);
            let y = fix_at(fx, q1 + 4, R_FAN[11]);
            xyz_far(fx, sp + 0x580, x, y);
            fx.submit(d, 0, 0);
        }
    }
    let layer = fx.mem.read_u32(sp + 0xDC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + LENS + 4) + 1;
    fx.mem.set32(D + LENS + 4, age);
    if age >= 0x4B {
        fx.mem.write_u8(D + LENS, 0);
    }
}

/// The glow [F 0x1E00CE4..0x1E00FD8]; returns the fade f21 the rings use.
fn glow(fx: &mut Fx, sp: u32) -> f32 {
    if fx.mem.i32(D) == 0x12A {
        fx.mem.write_u8(D + 0x100, 1);
        fx.mem.write_u32(D + 0x104, 0);
    }
    let age = fx.mem.i32(D + 0x104);
    let x = ee::from_i32(age);
    let f21 = ee::mul(Fx::ratio_below(x, 16.0), Fx::ramp_above(x, 167.0, 151.0));
    if fx.mem.read_u8(D + 0x100) != 1 {
        return f21;
    }
    sprite_head(fx, sp, 0x14, 1);
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MS);
    fx.translate_rows(sp + MS, sp + MS, D + 0xE0);
    fx.scale_rows(sp + MS, D + 0xF0);
    mul_mem(fx, sp + VP, sp + MS);
    let v = ee::from_i32(age % 2);
    let f1 = ee::add(f(0x3F33_3333), ee::mul(f(0x3E99_999A), v));
    let al = fixu(fx, ee::mul(ee::mul(255.0, f1), f21), 0x01E0_0E34);
    rgba(fx, sp + SD + 0x10, [0xAA, 0xFF, 0x46], al as u8);
    corners(fx, sp, 30.0);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    sprite_far(fx, sp, sp + C0);
    let age = fx.mem.i32(D + 0x104) + 1;
    fx.mem.set32(D + 0x104, age);
    if age >= 0xA8 {
        fx.mem.write_u8(D + 0x100, 0);
    }
    f21
}

/// A random 0.5 / 0.3-spread scale: (lo + (a r) / 256) k.
fn spread(r: i32, a: f32, lo: f32, k: f32) -> f32 {
    ee::mul(k, ee::add(lo, ee::div(ee::mul(a, ee::from_i32(r)), 256.0)))
}

/// The ring records [F 0x1E00FDC..0x1E01740].
fn rings(fx: &mut Fx, sp: u32, f21: f32) {
    let n = fx.mem.i32(D);
    let (a3, a7) = (f(0x3E99_999A), f(0x3F33_3333));
    if (0x12B..0x1D1).contains(&n) {
        let g = D + RINGS + 0x3D0 * (n % 16) as u32;
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        for k in 0..0xCu32 {
            let r = rnd(fx, 0x01E0_1050) % 256;
            let life = fixs(fx, spread(r, a3, a7, 16.0), 0x01E0_10B4);
            fx.mem.set32(g + 8 + 4 * k, life);
            let r = rnd(fx, 0x01E0_10BC) % 256 + 1;
            fx.mem.setf(g + 0x38 + 4 * k, spread(r, 0.5, 0.5, 2.5));
            let r = rnd(fx, 0x01E0_1118) % 256 + 1;
            fx.mem.setf(g + 0x68 + 4 * k, spread(r, a3, a7, 35.0));
            let r = rnd(fx, 0x01E0_1184) % 256 + 1;
            fx.mem.setf(g + 0x98 + 4 * k, spread(r, 0.5, 0.5, 8.5));
            fx.random_table_vec(sp + T, 1.0);
            fx.point_along(g + 0xD0 + 0x40 * k, sp + T);
            fx.rotate_axis(g + 0xD0 + 0x40 * k, ((k << 16) as i32) / 12, 2);
        }
    }
    strip_head(fx, sp, 0x10, RIBBON_CLAMP);
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
            translation_into(fx, sp + MS, D + 0xE0);
            mul_mem(fx, sp + MS, g + 0xD0 + 0x40 * k);
            fx.scale_rows(sp + MS, D + 0xF0);
            mul_mem(fx, sp + VP, sp + MS);
            let tf = fx.tan_falloff_p(age, life, 1);
            let f23 = ee::sub(1.0, tf);
            let f22 = ee::add(1.0, ee::sub(1.0, f23));
            for s2 in 0..0xAi32 {
                let c = D + 0x108 + 4 * s2 as u32;
                let x = ee::from_i32(i32::from(fx.mem.read_u8(c + 3)));
                let a0 = fixu(fx, ee::mul(f21, ee::mul(x, f23)), 0x01E0_13D8);
                let x = ee::from_i32(i32::from(fx.mem.read_u8(c + 7)));
                let a1 = fixu(fx, ee::mul(f21, ee::mul(x, f23)), 0x01E0_141C);
                for (o, s, al) in [(0x10u32, 0u32, a0), (0x40, 4, a1)] {
                    let rgb = [fx.mem.read_u8(c + s), fx.mem.read_u8(c + s + 1), fx.mem.read_u8(c + s + 2)];
                    rgba(fx, d + o, rgb, al as u8);
                }
                words(fx, d + 0x28, d + 0x10, 2);
                words(fx, d + 0x58, d + 0x40, 2);
                let t = s2 % 4;
                let t0 = ee::div(ee::mul(0.5, ee::from_i32(t)), 4.0);
                let t1 = ee::div(ee::mul(0.5, ee::from_i32(t + 1)), 4.0);
                strip_st(fx, sp, f(0x3D20_0000), t0, f(0x3E58_0000), t1);
                let f4 = fx.mem.f(g + 0x38 + 4 * k);
                let f3 = fx.mem.f(g + 0x68 + 4 * k);
                let f26 = ee::mul(f23, ee::div(ee::mul(f4, ee::from_i32(10 - s2)), 10.0));
                let f24 = ee::div(ee::mul(f3, ee::from_i32(s2 + 1)), 10.0);
                let f25 = ee::div(ee::mul(f3, ee::from_i32(s2)), 10.0);
                let f20 = ee::mul(f23, ee::div(ee::mul(f4, ee::from_i32(9 - s2)), 10.0));
                let s = fx.tsin(((s2 << 16) / 10) & 0xFFFF);
                let f27 = ee::mul(f22, ee::mul(fx.mem.f(g + 0x98 + 4 * k), s));
                let s = fx.tsin((((s2 + 1) << 16) / 10) & 0xFFFF);
                let f1 = ee::mul(f22, ee::mul(fx.mem.f(g + 0x98 + 4 * k), s));
                vec4(fx, sp + C0, [f27, ee::add(ee::neg(f26), f27), f25, 1.0]);
                vec4(fx, sp + C1, [f27, ee::add(f26, f27), f25, 1.0]);
                vec4(fx, sp + 0x3D0, [f1, ee::add(ee::neg(f20), f1), f24, 1.0]);
                vec4(fx, sp + 0x3E0, [f1, ee::add(f20, f1), f24, 1.0]);
                draw_w(fx, sp, sp + C0, d, 0x01E0_167C);
            }
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// The hoop [F 0x1E01740..0x1E01D00].
fn hoop(fx: &mut Fx, sp: u32) {
    let g = D + HOOP;
    if fx.mem.i32(D) == 0x17F {
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        words(fx, g + 0x10, D + 0x40, 4);
        fx.vu_sub4(g + 0x20, D + 0xC0, g + 0x10);
        let x = Fx::powf2(fx.mem.f(g + 0x20));
        let y = Fx::powf2(fx.mem.f(g + 0x24));
        let f20 = ee::add(x, y);
        let z = Fx::powf2(fx.mem.f(g + 0x28));
        fx.mem.write_u64(fx.sp - 0x10, 0x01E0_17FC);
        let l = fx.sqrtf(ee::add(z, f20));
        fx.mem.setf(g + 0x30, l);
        for k in 0..0x10u32 {
            let r = rnd(fx, 0x01E0_1814) % 256;
            fx.mem.setf(g + 0x34 + 4 * k, spread(r, 0.5, 0.5, 1.5));
            let r = rnd(fx, 0x01E0_186C) % 256;
            fx.mem.setf(g + 0x74 + 4 * k, spread(r, 0.5, 0.5, 8.0));
            fx.point_along(g + 0xC0 + 0x40 * k, g + 0x20);
            let v = phase(fx, 0x01E0_18D4);
            fx.rotate_axis(g + 0xC0 + 0x40 * k, v, 2);
        }
    }
    strip_head(fx, sp, 0x10, RIBBON_CLAMP);
    if fx.mem.read_u8(g) == 0 {
        return;
    }
    let d = sp + ST;
    let x = ee::from_i32(fx.mem.i32(g + 4));
    let s7 = fixs(fx, ee::mul(16.0, Fx::ratio_below(x, 18.0)), 0x01E0_19E8).min(0x10);
    let age = fx.mem.i32(g + 4);
    let f21 = ee::sub(1.0, fx.tan_falloff_p(age, 0x40, 1));
    let al = fixu(fx, ee::mul(128.0, f21), 0x01E0_1A30);
    rgba(fx, d + 0x10, [0xAA, 0xFF, 0x46], al as u8);
    strip_one_colour(fx, sp);
    let f20 = ee::add(1.0, ee::mul(1.5, ee::sub(1.0, f21)));
    let mut s4 = 0i32;
    for fp in 0..0x10u32 {
        fx.camera_vp_to(sp + VP);
        fx.translate_rows(sp + MS, g + 0xC0 + 0x40 * fp, g + 0x10);
        mul_mem(fx, sp + VP, sp + MS);
        for s6 in 0..s7 {
            let t = s6 % 3;
            let t0 = ee::div(ee::mul(0.5, ee::from_i32(t)), 3.0);
            let t1 = ee::div(ee::mul(0.5, ee::from_i32(t + 1)), 3.0);
            strip_st(fx, sp, f(0x3D20_0000), t0, f(0x3E58_0000), t1);
            let f4 = fx.mem.f(g + 0x30);
            let f23 = ee::div(ee::mul(f4, ee::from_i32(s6)), 16.0);
            let f22 = ee::div(ee::mul(f4, ee::from_i32(s6 + 1)), 16.0);
            let f24 = ee::mul(fx.mem.f(g + 0x34 + 4 * fp), f21);
            let c = fx.tcos((((s6 << 16) >> 4) + s4) & 0xFFFF);
            let f25 = ee::mul(f20, ee::mul(fx.mem.f(g + 0x74 + 4 * fp), c));
            let c = fx.tcos(((((s6 + 1) << 16) >> 4) + s4) & 0xFFFF);
            let f1 = ee::mul(f20, ee::mul(fx.mem.f(g + 0x74 + 4 * fp), c));
            let f3 = ee::neg(f24);
            vec4(fx, sp + Q, [0.0, ee::add(f3, f25), f23, 1.0]);
            vec4(fx, sp + 0x400, [0.0, ee::add(f24, f25), f23, 1.0]);
            vec4(fx, sp + 0x410, [0.0, ee::add(f3, f1), f22, 1.0]);
            vec4(fx, sp + 0x420, [0.0, ee::add(f24, f1), f22, 1.0]);
            draw_w(fx, sp, sp + Q, d, 0x01E0_1C78);
        }
        s4 += 0x1A0A;
    }
    let age = fx.mem.i32(g + 4) + 1;
    fx.mem.set32(g + 4, age);
    if age >= 0x41 {
        fx.mem.write_u8(g, 0);
    }
}

/// The cone and its glow [F 0x1E01D04..0x1E0249C].
fn cone(fx: &mut Fx, sp: u32) {
    let g = D + CONE;
    if fx.mem.i32(D) == 0xAA {
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        let v = phase(fx, 0x01E0_1D48);
        fx.mem.set32(g + 8, v);
        words(fx, g + 0x10, D + 0x40, 4);
        words(fx, g + 0x20, D + 0x50, 16);
    }
    strip_head(fx, sp, 0x14, 0);
    if fx.mem.read_u8(g) == 0 {
        return;
    }
    let d = sp + ST;
    let age = fx.mem.i32(g + 4);
    let x = ee::from_i32(age);
    let f21 = ee::mul(48.0, Fx::ratio_below(x, 16.0));
    let p = ee::mul(f21, Fx::ramp_above(x, 250.0, 234.0));
    let v = ee::from_i32(age % 2);
    let h = ee::mul(0.5, v);
    let al = ((0.5 + f64::from(h)) * f64::from(p)) as i32;
    rgba(fx, d + 0x10, [0xAA, 0xFF, 0x46], al as u8);
    strip_one_colour(fx, sp);
    vec4(fx, sp + T, [1.0; 4]);
    fx.camera_vp_to(sp + VP);
    fx.translate_rows(sp + MS, D + 0x50, D + 0x40);
    let a = fx.mem.i32(g + 8).wrapping_add(age.wrapping_mul(0x457));
    fx.rotate_axis(sp + MS, a, 2);
    fx.scale_rows(sp + MS, sp + T);
    mul_mem(fx, sp + VP, sp + MS);
    let m = fx.read_m4(sp + VP);
    for s6 in 0..8u32 {
        for s5 in 0..0x10u32 {
            let b = u32::from(fx.mem.read_u8(D + CONE_B + 0x10 * s6 + s5));
            let f4 = ee::from_i32(((b % 4) << 5) as i32);
            let f2 = ee::from_i32(((b >> 2) << 5) as i32);
            let s0 = ee::div(f4, 128.0);
            let s1 = ee::div(ee::add(32.0, f4), 128.0);
            let t1 = ee::div(f2, 64.0);
            let t0 = ee::div(ee::add(16.0, f2), 64.0);
            strip_st(fx, sp, s0, t0, s1, t1);
            if fx.project_desc(D + CONE_V + 0x400 * s6 + 0x40 * s5, PROJ_OUT, Some(&m), d, sp + W) {
                let w = fx.mem.f(sp + W);
                let key = fixu(fx, w, 0x01E0_20FC);
                fx.submit(d, OT, key as i32);
            }
        }
    }
    sprite_head(fx, sp, 0x14, 1);
    camera(fx, sp);
    corners(fx, sp, 16.5);
    let age = fx.mem.i32(g + 4);
    let x = ee::from_i32(age);
    let f3 = ee::mul(ee::mul(128.0, Fx::ratio_below(x, 16.0)), Fx::ramp_above(x, 250.0, 234.0));
    let v = ee::from_i32(age % 2);
    let f0 = ee::add(f(0x3F19_999A), ee::mul(f(0x3ECC_CCCD), v));
    let al = fixu(fx, ee::mul(f0, f3), 0x01E0_22BC);
    rgba(fx, sp + SD + 0x10, [0xAA, 0xFF, 0x46], al as u8);
    words(fx, sp + T, D + 0x40, 4);
    fx.vu_add4(sp + Q, sp + C0, sp + T);
    fx.vu_add4(sp + 0x400, sp + C1, sp + T);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    sprite_far(fx, sp, sp + Q);
    let age = fx.mem.i32(g + 4) + 1;
    fx.mem.set32(g + 4, age);
    if age >= 0xFB {
        fx.mem.write_u8(g, 0);
    }
}

/// The sparks [F 0x1E0249C..0x1E02928].
fn sparks(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n == 0xB2 || n == 0xCC {
        let a = u32::from(n == 0xB2);
        let g = D + SPARKS + 0x240 * a;
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        words(fx, g + 0x30, D + 0xA0 + 0x10 * a, 4);
        for k in 0..0x20u32 {
            let b = fx.rand_from(D, 0x01E0_2538) >> 23;
            fx.mem.set8(g + 8 + k, b % 8);
            let v = g + 0x40 + 0x10 * k;
            fx.random_table_vec(v, f(0x3F4C_CCCD));
            let y = fx.mem.f(v + 4);
            fx.mem.write_f32(fx.sp - 0x10, y);
            let y = f32::from_bits(y.to_bits() & 0x7FFF_FFFF);
            fx.mem.setf(v + 4, ee::mul(y, f(0x3F4C_CCCD)));
        }
    }
    sprite_head(fx, sp, 0x18, 0);
    camera(fx, sp);
    for r in 0..2u32 {
        let g = D + SPARKS + 0x240 * r;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 4);
        let c = fx.tcos((age << 14) >> 5);
        let al = fixu(fx, ee::mul(64.0, c), 0x01E0_2680);
        rgba(fx, sp + SD + 0x10, [0xEE; 3], al as u8);
        let x = ee::from_i32(age);
        corners(fx, sp, ee::mul(7.0, ee::add(0.5, ee::div(ee::mul(0.5, x), 32.0))));
        let f20 = fx.tan_falloff_p(age, 0x20, 1);
        for k in 0..0x20u32 {
            fx.vu_scale4(sp + T, g + 0x40 + 0x10 * k, f20);
            fx.vu_add4(sp + T, sp + T, g + 0x30);
            let y = ee::add(fx.mem.f(sp + T + 4), -5.0);
            fx.mem.setf(sp + T + 4, y);
            fx.vu_add4(sp + Q, sp + C0, sp + T);
            fx.vu_add4(sp + 0x400, sp + C1, sp + T);
            let b = u32::from(fx.mem.read_u8(g + 8 + k));
            let (c, r) = (b % 4, b >> 2);
            uvs(fx, sp, c << 9, r << 9, (32 * c + 32) << 4, (32 * r + 32) << 4);
            let d = sp + SD;
            if crate::fxlib2::project(fx, sp + Q, PROJ_OUT, sp + VP, d, sp + W) {
                fx.submit(d, OT, 0);
            }
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// The quad descriptor's head at sp + QD; texture D+`tex`, `or` 0x800 or 0.
fn quad_head(fx: &mut Fx, sp: u32, tex: u32, or: u16) {
    let d = sp + QD;
    head(fx, d, 0x24, 9, 0x4_2424_2421, or);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    alpha(fx, d, 1);
}

/// The diamonds [F 0x1E02928..0x1E02D1C].
fn diamonds(fx: &mut Fx, sp: u32) {
    let k = match fx.mem.i32(D) {
        0x82 => Some(2u32),
        0x4A => Some(1),
        0x38 => Some(0),
        _ => None,
    };
    if let Some(k) = k {
        fx.mem.write_u8(D + FLASHES + k, 1);
        fx.mem.write_u32(D + FLASHES + 4 + 4 * k, 0);
    }
    quad_head(fx, sp, 0x14, 0x800);
    let d = sp + QD;
    for (o, u, v) in [(0x18u32, 0x600u32, 0x100u32), (0x28, 0x800, 0x100), (0x38, 0x600, 0x100), (0x48, 0x800, 0x100)] {
        uv(fx, d + o, u);
        uv(fx, d + o + 2, v);
    }
    let mut s6 = 0x70u32;
    for k in 0..3u32 {
        if fx.mem.read_u8(D + FLASHES + k) != 0 {
            let at = D + FLASHES + 4 + 4 * k;
            let age = fx.mem.i32(at);
            let x = ee::from_i32(age);
            let f2 = ee::mul(255.0, Fx::ratio_below(x, 8.0));
            let v = ee::from_i32(age % 2);
            let f0 = ee::add(0.5, ee::mul(0.5, v));
            let al = fixu(fx, ee::mul(f0, f2), 0x01E0_2B74);
            rgba(fx, d + 0x10, [0x60, 0xFF, 0x60], al as u8);
            for s5 in 0..4u32 {
                let g = D + DIAMONDS + 0x10 * s5;
                for (c, o) in [(0u32, 0x20u32), (8, 0x30), (0x10, 0x40), (0x18, 0x50)] {
                    let x = (fx.mem.read_u16(g + c) as u32).wrapping_add(s6).wrapping_add(0x700) << 4;
                    let y = (fx.mem.read_u16(g + c + 4) as u32).wrapping_add(0x800) << 4;
                    far_xy(fx, d + o, x as u16, y as u16);
                }
                fx.submit(d, OT, 0);
            }
            let age = fx.mem.i32(at) + 1;
            fx.mem.set32(at, age);
            if age >= 0x3201 {
                fx.mem.write_u8(D + FLASHES + k, 0);
            }
        }
        s6 += 0x90;
    }
}

/// The window flash [F 0x1E02D20..0x1E03008].
fn window(fx: &mut Fx, sp: u32) {
    let (on, ag) = (D + WINDOW, D + WINDOW + 4);
    if fx.mem.i32(D) == 0x91 {
        fx.mem.write_u8(on, 1);
        fx.mem.write_u32(ag, 0);
        for k in 0..3 {
            fx.mem.write_u8(D + FLASHES + k, 0);
        }
    }
    if fx.mem.read_u8(on) != 1 {
        return;
    }
    quad_head(fx, sp, 0x1C, 0);
    let d = sp + QD;
    for (o, s, t) in [(0x18u32, 0.0f32, 0.0f32), (0x28, 1.0, 0.0), (0x38, 0.0, 1.0), (0x48, 1.0, 1.0)] {
        fx.mem.setf(d + o, s);
        fx.mem.setf(d + o + 4, t);
    }
    let a = 0x20 - fx.mem.i32(ag);
    rgba(fx, d + 0x10, [0x80; 3], ((a * 0xFF) >> 5) as u8);
    for (o, x, y) in [(0x20u32, 0x5A80u16, 0x7200u16), (0x30, 0x5A80, 0x8E00), (0x40, 0xA580, 0x7200), (0x50, 0xA580, 0x8E00)] {
        far_xy(fx, d + o, x, y);
    }
    fx.submit(d, OT, 0);
    let e = sp + 0x488;
    head(fx, e, 6, 3, 0x441, 0);
    alpha(fx, e, 2);
    let a = 0x20 - fx.mem.i32(ag);
    rgba(fx, e + 0x10, [0x80; 3], ((a * 0x94) >> 5) as u8);
    far_xy(fx, e + 0x18, 0x7000, 0x7000);
    far_xy(fx, e + 0x20, 0x9000, 0x9000);
    fx.submit(e, OT, 0);
    let age = fx.mem.i32(ag) + 1;
    fx.mem.set32(ag, age);
    if age >= 0x21 {
        fx.mem.write_u8(on, 0);
    }
}
