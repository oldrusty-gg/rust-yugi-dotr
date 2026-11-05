//! BMODULE b372 (card 573), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xDE70
//! bytes) [F]: a
//! private copy of b291.rs's b624 path (card 605) at this record's
//! values, with 24 three-sprite blades, a scrolling cylinder sheet, a
//! jittered frame backdrop and rising motes in place of b624's motes.
//!
//! A single unscheduled function in a 0x620 frame (s1 = D, s0 the
//! ordering table, rand's saved s0); b291's frame offsets moved by
//! [`at`] (0x2D0..0x47F + 0x30, the sprite descriptor 0x480.. + 0x38, the
//! strip 0x4D8.. + 0xB0, mean w 0x61C), b624's fields from D+0x40 0x10
//! higher. Every update the camera turn with roll (sp + 0x380), node 1
//! (D+0x30, y + 2) aimed at the target (D+0x50: the turn D+0x60, the
//! distance D+0xA0), the attacker's point D+0x40 (z + 15 side). From
//! 0x4E for 0xA3 updates the tube (radius 15 / lens 14, alpha 100 / 128,
//! soft double 0.2 + 0.8 i / 20, wobble 910, lens wave 0.05, strips (94,
//! 9E, C8) / 0x80); spirals / puffs 0x4F..0xEF, bursts / flecks
//! 0x5F..0xEF (all (94, 9E, C8)); from 0 a glow (75 / 59 ramp, (FF, E8,
//! D0), 7 wide, 0x4C updates) and on 1..0x4A the blades (24 slots, three
//! 1.5 vectors each, (D0, E8, FF) at 80 (24 - age) / 24, far Z, 0x19
//! updates); from 8 for 0x109 updates the sheet ([`sheet`]); on 9..0x10F
//! the backdrop ([`backdrop`]); on 1..0x3B the rises ([`rises`]); the
//! stage light 0x80 - 2 clamp(0x118 - n, 0, 8) clamp(n, 0, 8).
//! Returns 1 on 0x54, 6 on 0x5F..0xEE, 2 on 0xEF, 8 from 0x118.
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::LENS_CLAMP;

const D: u32 = crate::bmodule::DATA_AREA;
/// The fields from D+0x40 (b291's offsets + 0x10).
const DZ: u32 = D + 0x10;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x620;
/// The sprite descriptor (b291's 0x480).
const DESC: u32 = 0x480;
/// The strip descriptor (b291's 0x4D8).
const STRIP: u32 = 0x4D8;
/// The sheet's descriptor (prim 0x24, 9 registers; this record's own).
const SHEET_DESC: u32 = 0x510;
const SHEET: u32 = 0xA690;
const PHASES: u32 = 0xC9A0;
const SHEET_ON: u32 = 0xD264;
const RISES: u32 = 0xD270;
const STRIP_RGB: [u8; 3] = [0x94, 0x9E, 0xC8];
const END: i32 = 0xF0;

/// b291's frame offset `x` as this record's.
fn at(x: u32) -> u32 {
    match x {
        0x2C0 => 0x2E0,
        0x2D0..=0x47F => x + 0x30,
        0x480..=0x4D7 => x + 0x38,
        0x4D8..=0x56B => x + 0xB0,
        0x56C => 0x61C,
        _ => x,
    }
}

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
    uv(fx, sp + at(0x498), c << 9);
    uv(fx, sp + at(0x49A), r << 9);
    uv(fx, sp + at(0x4A8), (32 * c + 32) << 4);
    uv(fx, sp + at(0x4AA), (32 * r + 32) << 4);
}

fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

/// The sprite descriptor head (prim 0x26 | 0x800, 5 registers); texture
/// `tex` (filter 1), ALPHA `preset`, CLAMP 5.
fn head(fx: &mut Fx, sp: u32, tex: u32, preset: i32) {
    let d = sp + at(DESC);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(preset);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
}

/// The strip descriptor head (prim 0x34, 12 registers).
fn strip_head(fx: &mut Fx, sp: u32) {
    let d = sp + at(STRIP);
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
}

/// The camera's view-projection (x identity) at sp + at(0x2D0).
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + at(0x2D0));
    fx.identity(sp + at(0x310));
    mul_mem(fx, sp + at(0x2D0), sp + at(0x310));
}

/// The camera's view-projection x the aim D+0x60 at the node.
fn aimed(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + at(0x2D0));
    fx.translate_rows(sp + at(0x310), DZ + 0x50, D + 0x30);
    mul_mem(fx, sp + at(0x2D0), sp + at(0x310));
}

/// The corners (a, a, 0, 1) / (b, b, 0, 1) turned by the camera.
fn corners(fx: &mut Fx, sp: u32, a: f32, b: f32) {
    vec4(fx, sp + at(0x390), [a, a, 0.0, 1.0]);
    vec4(fx, sp + at(0x3A0), [b, b, 0.0, 1.0]);
    fx.vu_apply_to(sp + at(0x390), sp + at(0x350), sp + at(0x390));
    fx.vu_apply_to(sp + at(0x3A0), sp + at(0x350), sp + at(0x3A0));
}

/// The sprite at sp + at(0x450) + the corners projected and drawn keyed
/// by its mean w.
fn sprite(fx: &mut Fx, sp: u32, ra: u32) {
    fx.vu_add4(sp + at(0x3D0), sp + at(0x390), sp + at(0x450));
    fx.vu_add4(sp + at(0x3E0), sp + at(0x3A0), sp + at(0x450));
    if crate::fxlib2::project(fx, sp + at(0x3D0), PROJ_OUT, sp + at(0x2D0), sp + at(DESC), sp + at(0x56C)) {
        let w = fx.mem.f(sp + at(0x56C));
        let key = fixu(fx, w, ra);
        fx.submit(sp + at(DESC), OT, key as i32);
    }
}

fn clamp32(v: i32) -> f32 {
    ee::from_i32(Fx::clamp_i(v, 0, 0x20))
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xDE70);
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
    let r = fx.request_node(h, 1, 1);
    fx.mem.set32(D + 0x20, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, DZ + 0x40);
    fx.mem.write_u8(DZ + 0x2FA0, 0);
    fx.mem.write_u32(DZ + 0x2FA4, 0);
    for k in 0..0x30u32 {
        fx.random_table_vec(DZ + 0x5AB0 + 0x10 * k, 6.0);
    }
    for k in 0..0x12u32 {
        fx.mem.write_u8(DZ + 0x5DB0 + 0x340 * k, 0);
    }
    for k in 0..0xCu32 {
        fx.mem.write_u8(DZ + 0x9830 + 0x50 * k, 0);
    }
    for k in 0..0xCu32 {
        fx.mem.write_u8(DZ + 0x9BF0 + 0x60 * k, 0);
    }
    fx.mem.write_u32(DZ + 0xA074, 0);
    for k in 0..0x18u32 {
        fx.mem.write_u8(DZ + 0xA080 + 0x40 * k, 0);
    }
    // The sheet's phases: 33 rows of 17, ((rand >> 23) % 256 << 16) >> 8
    // (rand's saved s0 = the column).
    for i in 0..0x21u32 {
        for j in 0..0x11u32 {
            let r = rnd(fx, j, 0x01E0_0250);
            fx.mem.set32(D + PHASES + 0x44 * i + 4 * j, ((r % 256) << 16) >> 8);
        }
    }
    fx.mem.write_u8(D + SHEET_ON, 0);
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + RISES + 0x60 * k, 0);
    }
    0
}

/// (float)((double)(0.2 + 0.8 i / 20) x (0.8 + (double)(0.2 sin a))) (the
/// soft-double chain 0x1A2C30 / 0x1A3118 / 0x1A31D8; `k` the extra
/// (double) factor of the spirals).
fn radius(fx: &mut Fx, i: i32, age: i32, k: Option<f32>) -> f32 {
    let (_, r) = fx.div(i, 12);
    let a = ((r << 16) / 12).wrapping_sub(0x8AE_i32.wrapping_mul(age));
    let s = fx.tsin(a & 0xFFFF);
    let d1 = 0.8f64 + f64::from(ee::mul(f32::from_bits(0x3E4C_CCCD), s));
    let f2 = ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(i));
    let f12 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(f2, 20.0));
    let p = f64::from(f12) * d1;
    match k {
        Some(k) => (f64::from(k) * p) as f32,
        None => p as f32,
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + at(0x350));
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x20));
    fx.node_position(h, slot, D + 0x30);
    let y = fx.mem.f(D + 0x34);
    fx.mem.setf(D + 0x34, ee::add(y, 2.0));
    fx.vu_sub4(sp + at(0x450), DZ + 0x40, D + 0x30);
    fx.point_along(DZ + 0x50, sp + at(0x450));
    fx.rotate_axis(DZ + 0x50, 0x4000, 0);
    let x2 = Fx::powf2(fx.mem.f(sp + at(0x450)));
    let y2 = Fx::powf2(fx.mem.f(sp + at(0x454)));
    let f20 = ee::add(x2, y2);
    let z2 = Fx::powf2(fx.mem.f(sp + at(0x458)));
    let d = fx.sqrtf(ee::add(z2, f20));
    fx.mem.setf(DZ + 0x90, d);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x40);
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    let z = fx.mem.f(D + 0x48);
    fx.mem.setf(D + 0x48, ee::add(z, ee::mul(15.0, side)));
    if fx.mem.i32(D) == 0x4E {
        fx.mem.write_u8(DZ + 0x2FA0, 1);
        fx.mem.write_u32(DZ + 0x2FA4, 0);
    }
    let age = fx.mem.i32(DZ + 0x2FA4);
    let f20 = ee::div(clamp32(0xA2 - age), 32.0);
    if fx.mem.read_u8(DZ + 0x2FA0) == 1 {
        tube(fx, sp, f20);
    }
    let n = fx.mem.i32(D);
    if (0x4F..END).contains(&n) {
        spiral_spawn(fx, sp, n);
    }
    spirals(fx, sp);
    let n = fx.mem.i32(D);
    if (0x5F..END).contains(&n) {
        let (_, r) = fx.div(n, 0x12);
        let g = DZ + 0x340 * r as u32;
        fx.mem.write_u8(g + 0x5DB0, 1);
        fx.mem.write_u32(g + 0x5DB4, 0);
        for j in 0..0x30u32 {
            let b = rnd(fx, OT, 0x01E0_1920);
            fx.mem.set8(g + 0x5DB8 + j, b % 8);
            fx.random_table_vec(sp + at(0x450), 1.0);
            fx.vu_add4(g + 0x5DF0 + 0x10 * j, sp + at(0x450), DZ + 0x5AB0 + 0x10 * j);
        }
    }
    bursts(fx, sp);
    let n = fx.mem.i32(D);
    if (0x5F..END).contains(&n) {
        let (_, r) = fx.div(n, 0xC);
        let g = DZ + 0x50 * r as u32;
        fx.mem.write_u8(g + 0x9830, 1);
        fx.mem.write_u32(g + 0x9834, 0);
        for j in 0..4u32 {
            let b = rnd(fx, OT, 0x01E0_1D2C);
            fx.mem.set8(g + 0x9838 + j, b % 8);
            fx.random_table_vec(g + 0x9840 + 0x10 * j, 3.0);
        }
    }
    flecks(fx, sp);
    let n = fx.mem.i32(D);
    if (0x4F..END).contains(&n) {
        let (_, r) = fx.div(n, 0xC);
        let g = DZ + 0x60 * r as u32;
        fx.mem.write_u8(g + 0x9BF0, 1);
        fx.mem.write_u32(g + 0x9BF4, 0);
        words(fx, g + 0x9C00, D + 0x30, 4);
        for j in 0..4u32 {
            let b = rnd(fx, OT, 0x01E0_218C);
            fx.mem.set8(g + 0x9BF8 + j, b % 8);
            fx.random_table_vec(g + 0x9C10 + 0x10 * j, 1.5);
        }
    }
    puffs(fx, sp);
    blades(fx, sp);
    sheet(fx, sp);
    backdrop(fx, sp);
    rises(fx, sp);
    let a = Fx::clamp_i(0x118 - fx.mem.i32(D), 0, 8);
    let b = Fx::clamp_i(fx.mem.i32(D), 0, 8);
    let l = (a.wrapping_mul((-b) << 7) >> 6) + 0x80;
    fx.set_stage_light(l & 0xFF);
    finish(fx, sp)
}

fn finish(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x54 {
        ret = 1;
    } else {
        if (0x5F..0xEF).contains(&n) {
            ret = 6;
        }
        if n == 0xEF {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x119 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// The ring of 17 points (D+0xB0) at radius `r`.
fn ring(fx: &mut Fx, r: f32) {
    for k in 0..0x11u32 {
        let a = (k << 12) as i32;
        let s = fx.tsin(a);
        fx.mem.setf(DZ + 0xA0 + 0x10 * k, ee::mul(r, s));
        fx.mem.write_u32(DZ + 0xA4 + 0x10 * k, 0);
        let c = fx.tcos(a);
        fx.mem.setf(DZ + 0xA8 + 0x10 * k, ee::mul(r, c));
        fx.mem.write_u32(DZ + 0xAC + 0x10 * k, 0x3F80_0000);
    }
}

/// The strip's four colours for row `j` (alpha `a` inside, 0 at the ends).
fn row_colours(fx: &mut Fx, sp: u32, j: u32, a: u8, c: [u8; 3]) {
    let (top, bottom) = match j {
        0 => (0, a),
        0x13 => (a, 0),
        _ => (a, a),
    };
    rgba(fx, sp + at(0x4E8), c, top);
    words(fx, sp + at(0x500), sp + at(0x4E8), 2);
    rgba(fx, sp + at(0x518), c, bottom);
    words(fx, sp + at(0x530), sp + at(0x518), 2);
}

/// The tube [F 0x1E00408..0x1E0117C].
fn tube(fx: &mut Fx, sp: u32, f20: f32) {
    let age = fx.mem.i32(DZ + 0x2FA4);
    // 21 centre points (all 0 x sin / cos).
    for s2 in 0..0x15u32 {
        let a = 1820i32.wrapping_mul(age).wrapping_add(((s2 << 16) / 20) as i32) & 0xFFFF;
        let s = fx.tsin(a);
        let p = DZ + 0x2E50 + 0x10 * s2;
        fx.mem.setf(p, ee::mul(0.0, s));
        fx.mem.write_u32(p + 4, 0);
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(0.0, c));
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
    }
    let age = fx.mem.i32(DZ + 0x2FA4);
    let ac = 16.0;
    let f3 = ee::from_i32(Fx::clamp_i(age, 0, 0x10));
    let f21 = ee::div(ee::mul(fx.mem.f(DZ + 0x90), f3), ac);
    let f22 = ee::add(0.5, ee::div(ee::mul(0.5, f3), ac));
    let d = sp + at(STRIP);
    strip_head(fx, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    let f23 = ee::mul(128.0, f20);
    let a_in = fixu(fx, ee::mul(100.0, f20), 0x01E0_0590) as u8;
    fx.mem.write_u8(sp + 0xD0, a_in);
    ring(fx, ee::mul(15.0, f22));
    // The textured grid.
    aimed(fx, sp);
    grid(fx, f21, None);
    let age = fx.mem.i32(DZ + 0x2FA4);
    let (_, r) = fx.div(age, 10);
    let f20 = ee::sub(0.0, ee::div(ee::from_i32(r), 10.0));
    let mut regs = [0u32; 4];
    for j in 0..0x14u32 {
        row_colours(fx, sp, j, a_in, STRIP_RGB);
        let f24 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(j as i32)), 20.0), f20);
        let f25 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(j as i32 + 1)), 20.0), f20);
        let (s4, s5) = (DZ + 0x110 * j, DZ + 0x110 * (j + 1));
        for s6 in 0..0x10u32 {
            let s0 = ee::div(ee::from_i32(s6 as i32), 16.0);
            let s1 = ee::div(ee::from_i32(s6 as i32 + 1), 16.0);
            for (o, x) in [(0x4F0u32, s0), (0x4F4, f24), (0x508, s1), (0x50C, f24), (0x520, s0), (0x524, f25), (0x538, s1), (0x53C, f25)] {
                fx.mem.setf(sp + at(o), x);
            }
            let vs = [s4 + 0x10 * s6 + 0x1B0, s4 + 0x10 * (s6 + 1) + 0x1B0, s5 + 0x10 * s6 + 0x1B0, s5 + 0x10 * (s6 + 1) + 0x1B0];
            if fx.project_n(&vs, sp + at(0x2D0), d, sp + at(0x56C), 0x01E0_0A14) == 1 {
                let w = fx.mem.f(sp + at(0x56C));
                let key = fixu(fx, w, 0x01E0_0A28);
                fx.submit(d, OT, key as i32);
                // 0x206F30 leaves a0..a3 = (desc, ot, key, desc) [run].
                regs = [d, OT, key, d];
            } else {
                // 0x100BA0 rejected: 0x205210's arguments [H].
                regs = [fx.sp - 0x40, PROJ_OUT, sp + at(0x2D0), d];
            }
        }
    }
    // The lens grid.
    strip_head(fx, sp);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xEC, layer);
    let regs = crate::lens::frame_copy(fx, regs, 1, sp + at(0x550), sp + at(0x558));
    fx.texflush(regs);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, LENS_CLAMP);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a_in = fixu(fx, f23, 0x01E0_0B40) as u8;
    fx.mem.write_u8(sp + at(0x2C0), a_in);
    ring(fx, ee::mul(14.0, f22));
    aimed(fx, sp);
    grid(fx, f21, Some(sp));
    for j in 0..0x14u32 {
        row_colours(fx, sp, j, a_in, [0x80, 0x80, 0x80]);
        let (s7, fp) = (DZ + 0x110 * j, DZ + 0x110 * (j + 1));
        for s6 in 0..0x10u32 {
            let (s3, s5, s2, s4) = (s7 + 0x10 * s6, s7 + 0x10 * (s6 + 1), fp + 0x10 * s6, fp + 0x10 * (s6 + 1));
            if fx.project_n(&[s3 + 0x1800, s5 + 0x1800, s2 + 0x1800, s4 + 0x1800], sp + at(0x2D0), d, sp + at(0x56C), 0x01E0_1038) != 1 {
                continue;
            }
            for k in 0..4u32 {
                fx.uv_from_xy(sp + at(0x4F0) + 0x18 * k, sp + at(0x4F8) + 0x18 * k, 0, 0);
            }
            if fx.project_n(&[s3 + 0x1B0, s5 + 0x1B0, s2 + 0x1B0, s4 + 0x1B0], sp + at(0x2D0), d, sp + at(0x56C), 0x01E0_10B4) != 1 {
                continue;
            }
            if front(fx, sp + at(0x4F8), sp + at(0x510), sp + at(0x528)) {
                fx.submit(d, 0, 0);
            }
        }
    }
    let layer = fx.mem.read_u32(sp + 0xEC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(DZ + 0x2FA4) + 1;
    fx.mem.set32(DZ + 0x2FA4, age);
    if age >= 0xA3 {
        fx.mem.write_u8(DZ + 0x2FA0, 0);
    }
}

/// The 21 x 17 grid D+0x1C0 (rows 0x110 apart) along the aim, row y f21 i
/// / 20; with `lens` (the frame) also the wobbling copy D+0x1810 turned
/// about y by 910 sin, its y wave 0.05 f21 / 20.
fn grid(fx: &mut Fx, f21: f32, lens: Option<u32>) {
    let age = fx.mem.i32(DZ + 0x2FA4);
    for s6 in 0..0x15u32 {
        let i = s6 as i32;
        let f20 = ee::div(ee::mul(f21, ee::from_i32(i)), 20.0);
        let f24 = radius(fx, i, age, None);
        let mut s5 = 0;
        if let Some(sp) = lens {
            fx.identity(sp + at(0x410));
            let (_, r) = fx.div(age, 0x2E);
            let a0 = (r << 16) / 46;
            s5 = (i % 8) << 13;
            let s = fx.tsin(s5.wrapping_add(a0) & 0xFFFF);
            let a = fixs(fx, ee::mul(910.0, s), 0x01E0_0D70);
            fx.rotate_axis(sp + at(0x410), a, 1);
        }
        let row = DZ + 0x110 * s6;
        let centre = DZ + 0x2E50 + 0x10 * s6;
        for s2 in 0..0x11u32 {
            let p = row + 0x10 * s2 + 0x1B0;
            fx.vu_scale4(p, DZ + 0xA0 + 0x10 * s2, f24);
            fx.mem.setf(p + 4, f20);
            fx.vu_add4(p, p, centre);
            if let Some(sp) = lens {
                let q = row + 0x10 * s2 + 0x1800;
                fx.vu_apply_to(q, sp + at(0x410), p);
                let (_, r) = fx.div(age, 0x30);
                let b = (r << 16) / 48;
                let a = ((s2 as i32 % 16) << 12).wrapping_add(s5.wrapping_add(b));
                let s = fx.tsin(a & 0xFFFF);
                let f1 = ee::mul(f32::from_bits(0x3D4C_CCCD), ee::div(f21, 20.0));
                fx.mem.setf(q + 4, ee::add(f20, ee::mul(f1, s)));
            }
        }
    }
}

/// 0x101430(a, b, c) [F] (b291's): 1 when the triangle of the XYZ2 x / y
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

/// The spirals' spawn [F 0x1E011A4..0x1E014B4].
fn spiral_spawn(fx: &mut Fx, sp: u32, n: i32) {
    let off = 0x560 * (n % 8) as u32;
    let g = DZ + off;
    fx.mem.write_u8(g + 0x2FB0, 1);
    fx.mem.write_u32(g + 0x2FB4, 0);
    fx.vu_sub4(sp + at(0x450), DZ + 0x40, D + 0x30);
    let age = fx.mem.i32(DZ + 0x2FA4);
    let f = ee::div(ee::from_i32(Fx::clamp_i(age, 0, 0x10)), 16.0);
    fx.vu_scale4(sp + at(0x450), sp + at(0x450), f);
    let age = fx.mem.i32(DZ + 0x2FA4);
    let f1 = ee::from_i32(Fx::clamp_i(age, 0, 0x10));
    let f21 = ee::mul(15.0, ee::add(0.5, ee::div(ee::mul(0.5, f1), 16.0)));
    for s7 in 0..0x14u32 {
        let age = fx.mem.i32(DZ + 0x2FA4);
        let f20 = radius(fx, s7 as i32, age, Some(f21));
        for s5 in 0..4u32 {
            let b = rnd(fx, OT, 0x01E0_1388);
            fx.mem.set8(g + 0x2FB8 + 4 * s7 + s5, b % 8);
            let r = rnd(fx, OT, 0x01E0_13A8);
            let f2 = ee::div(ee::mul(f32::from_bits(0x3D4C_CCCD), ee::from_i32(r % 256)), 256.0);
            let f0 = ee::div(ee::from_i32(s7 as i32), 20.0);
            fx.vu_scale4(sp + at(0x460), sp + at(0x450), ee::add(f0, f2));
            fx.vu_add4(sp + at(0x460), sp + at(0x460), D + 0x30);
            let r = rnd(fx, OT, 0x01E0_1430);
            let a = (((0x10000 * s5) >> 2) as i32).wrapping_add((r % 256) << 8) & 0xFFFF;
            let s = fx.tsin(a);
            fx.mem.setf(sp + at(0x470), ee::mul(f20, s));
            fx.mem.write_u32(sp + at(0x474), 0);
            let c = fx.tcos(a);
            fx.mem.setf(sp + at(0x478), ee::mul(f20, c));
            fx.mem.write_u32(sp + at(0x47C), 0x3F80_0000);
            let q = g + 0x3010 + 0x40 * s7 + 0x10 * s5;
            fx.vu_apply_to(q, DZ + 0x50, sp + at(0x470));
            fx.vu_add4(q, q, sp + at(0x460));
        }
    }
}

/// The spirals [F 0x1E014E0..0x1E018C0].
fn spirals(fx: &mut Fx, sp: u32) {
    head(fx, sp, 0x18, 1);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let a = clamp32(n - 0x4E);
    let n = fx.mem.i32(D);
    let b = clamp32(END - n);
    let f20 = ee::div(ee::mul(a, b), 1024.0);
    for k in 0..8u32 {
        let g = DZ + 0x560 * k;
        if fx.mem.read_u8(g + 0x2FB0) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x2FB4);
        let s = fx.tsin((age << 15) >> 3);
        let al = fixu(fx, ee::mul(f20, ee::mul(50.0, s)), 0x01E0_1614);
        rgba(fx, sp + at(0x490), STRIP_RGB, al as u8);
        for j in 0..0x14u32 {
            let age = fx.mem.i32(g + 0x2FB4);
            let tf = fx.tan_falloff_p(age, 8, 1);
            let f3 = ee::mul(12.0, ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf)));
            let s = ee::mul(ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(j as i32)), 20.0)), f3);
            corners(fx, sp, s, ee::neg(s));
            for s6 in 0..4u32 {
                let b = u32::from(fx.mem.read_u8(g + 0x2FB8 + 4 * j + s6));
                cell_uvs(fx, sp, b);
                words(fx, sp + at(0x450), g + 0x3010 + 0x40 * j + 0x10 * s6, 4);
                sprite(fx, sp, 0x01E0_1828);
            }
        }
        let age = fx.mem.i32(g + 0x2FB4) + 1;
        fx.mem.set32(g + 0x2FB4, age);
        if age >= 9 {
            fx.mem.write_u8(g + 0x2FB0, 0);
        }
    }
}

/// The bursts at the target [F 0x1E01980..0x1E01CC8].
fn bursts(fx: &mut Fx, sp: u32) {
    head(fx, sp, 0x18, 1);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(clamp32(END - n), 32.0);
    for k in 0..0x12u32 {
        let g = DZ + 0x340 * k;
        if fx.mem.read_u8(g + 0x5DB0) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(g + 0x5DB4));
        let r = Fx::ramp_above(age, 18.0, 12.0);
        let al = fixu(fx, ee::mul(f20, ee::mul(50.0, r)), 0x01E0_1A7C);
        rgba(fx, sp + at(0x490), STRIP_RGB, al as u8);
        let age = fx.mem.i32(g + 0x5DB4);
        let tf = fx.tan_falloff_p(age, 0x12, 1);
        let s = ee::mul(25.0, ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf)));
        corners(fx, sp, s, ee::neg(s));
        let age = ee::from_i32(fx.mem.i32(g + 0x5DB4));
        let f21 = ee::add(f32::from_bits(0x3D4C_CCCD), ee::div(age, 18.0));
        for j in 0..0x30u32 {
            let b = u32::from(fx.mem.read_u8(g + 0x5DB8 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + at(0x450), g + 0x5DF0 + 0x10 * j, f21);
            fx.vu_add4(sp + at(0x450), sp + at(0x450), DZ + 0x40);
            sprite(fx, sp, 0x01E0_1C70);
        }
        let age = fx.mem.i32(g + 0x5DB4) + 1;
        fx.mem.set32(g + 0x5DB4, age);
        if age >= 0x13 {
            fx.mem.write_u8(g + 0x5DB0, 0);
        }
    }
}

/// The flecks at the target [F 0x1E01D78..0x1E02100].
fn flecks(fx: &mut Fx, sp: u32) {
    head(fx, sp, 0x1C, 1);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(clamp32(END - n), 32.0);
    for k in 0..0xCu32 {
        let g = DZ + 0x50 * k;
        if fx.mem.read_u8(g + 0x9830) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x9834);
        let c = fx.tsin((age << 15) / 12);
        let al = fixu(fx, ee::mul(f20, ee::mul(30.0, c)), 0x01E0_1EAC);
        rgba(fx, sp + at(0x490), STRIP_RGB, al as u8);
        corners(fx, sp, 22.0, -22.0);
        let age = ee::from_i32(fx.mem.i32(g + 0x9834));
        let f22 = ee::add(f32::from_bits(0x3E99_999A), ee::div(ee::mul(f32::from_bits(0x3F33_3333), age), 12.0));
        for j in 0..4u32 {
            let b = u32::from(fx.mem.read_u8(g + 0x9838 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + at(0x450), g + 0x9840 + 0x10 * j, f22);
            fx.vu_add4(sp + at(0x450), sp + at(0x450), DZ + 0x40);
            sprite(fx, sp, 0x01E0_2084);
        }
        let age = fx.mem.i32(g + 0x9834) + 1;
        fx.mem.set32(g + 0x9834, age);
        if age >= 0xD {
            fx.mem.write_u8(g + 0x9830, 0);
        }
    }
}

/// The puffs at the node [F 0x1E021D8..0x1E025AC].
fn puffs(fx: &mut Fx, sp: u32) {
    head(fx, sp, 0x18, 1);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(clamp32(END - n), 32.0);
    for k in 0..0xCu32 {
        let g = DZ + 0x60 * k;
        if fx.mem.read_u8(g + 0x9BF0) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x9BF4);
        let c = fx.tcos((age << 14) / 12);
        let al = fixu(fx, ee::mul(f20, ee::mul(96.0, c)), 0x01E0_2300);
        rgba(fx, sp + at(0x490), STRIP_RGB, al as u8);
        let age = ee::from_i32(fx.mem.i32(g + 0x9BF4));
        let s = ee::mul(15.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 12.0)));
        corners(fx, sp, s, ee::neg(s));
        let age = ee::from_i32(fx.mem.i32(g + 0x9BF4));
        let f21 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F66_6666), age), 12.0));
        for j in 0..4u32 {
            let b = u32::from(fx.mem.read_u8(g + 0x9BF8 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + at(0x450), g + 0x9C10 + 0x10 * j, f21);
            fx.vu_add4(sp + at(0x450), sp + at(0x450), g + 0x9C00);
            sprite(fx, sp, 0x01E0_2524);
        }
        let age = fx.mem.i32(g + 0x9BF4) + 1;
        fx.mem.set32(g + 0x9BF4, age);
        if age >= 0xD {
            fx.mem.write_u8(g + 0x9BF0, 0);
        }
    }
}

/// The glow (D+0xA080 from 0, 0x4C updates) and the blades (24 slots of
/// 0x40 at D+0xA090 on 1..0x4A, 0x19 updates) [F 0x1E025AC..0x1E02F14]:
/// b624's with three 1.5 vectors a blade (+0x20), 1.5 high, turned by k /
/// 24 + j / 3 of a turn, stretched -3.5 tan falloff(age, 24), (D0, E8, FF)
/// at f 80 (24 - age) / 24, far Z.
fn blades(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(DZ + 0xA070, 1);
        fx.mem.write_u32(DZ + 0xA074, 0);
    }
    if fx.mem.read_u8(DZ + 0xA070) == 1 {
        head(fx, sp, 0x10, 1);
        camera(fx, sp);
        let age = ee::from_i32(fx.mem.i32(DZ + 0xA074));
        let f20 = Fx::ratio_below(age, 16.0);
        let age = ee::from_i32(fx.mem.i32(DZ + 0xA074));
        let f0 = Fx::ramp_above(age, 75.0, 59.0);
        let f3 = ee::mul(f20, f0);
        let r = fx.mem.i32(DZ + 0xA074) % 2;
        let f0 = ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(r)));
        let al = fixu(fx, ee::mul(255.0, ee::mul(f0, f3)), 0x01E0_2720);
        rgba(fx, sp + at(0x490), [0xFF, 0xE8, 0xD0], al as u8);
        corners(fx, sp, 7.0, -7.0);
        cell_uvs(fx, sp, 4);
        words(fx, sp + at(0x450), D + 0x30, 4);
        fx.vu_add4(sp + at(0x3D0), sp + at(0x390), sp + at(0x450));
        fx.vu_add4(sp + at(0x3E0), sp + at(0x3A0), sp + at(0x450));
        if crate::fxlib2::project(fx, sp + at(0x3D0), PROJ_OUT, sp + at(0x2D0), sp + at(DESC), sp + at(0x56C)) {
            crate::lens::far_z(fx, sp + at(0x4A0), &[sp + at(0x4B0)]);
            fx.submit(sp + at(DESC), OT, 0);
        }
        let age = fx.mem.i32(DZ + 0xA074) + 1;
        fx.mem.set32(DZ + 0xA074, age);
        if age >= 0x4C {
            fx.mem.write_u8(DZ + 0xA070, 0);
        }
    }
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x4B {
        let g = DZ + 0x40 * (n % 24) as u32;
        fx.mem.write_u8(g + 0xA080, 1);
        fx.mem.write_u32(g + 0xA084, 0);
        for j in 0..3u32 {
            let b = rnd(fx, OT, 0x01E0_296C);
            fx.mem.set8(g + 0xA088 + j, b % 8);
            fx.random_table_vec(g + 0xA090 + 0x10 * j, 1.5);
        }
    }
    let d = sp + at(STRIP);
    strip_head(fx, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n, 0, 0x10));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(0x4B - n, 0, 0x10));
    let f20 = ee::div(ee::mul(a, b), 256.0);
    for k in 0..0x18u32 {
        let g = DZ + 0x40 * k;
        if fx.mem.read_u8(g + 0xA080) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0xA084);
        let x = 80i32.wrapping_mul(24 - age) / 24;
        let al = fixu(fx, ee::mul(f20, ee::from_i32(x)), 0x01E0_2AE8);
        rgba(fx, sp + at(0x4E8), [0xD0, 0xE8, 0xFF], al as u8);
        words(fx, sp + at(0x500), sp + at(0x4E8), 2);
        words(fx, sp + at(0x518), sp + at(0x4E8), 2);
        words(fx, sp + at(0x530), sp + at(0x4E8), 2);
        fx.camera_vp_to(sp + at(0x2D0));
        words(fx, sp + at(0x450), D + 0x30, 4);
        let t: [f32; 3] = std::array::from_fn(|c| fx.mem.f(D + 0x30 + 4 * c as u32));
        let m = fx.trs(None, None, Some(t));
        fx.write_m4(sp + at(0x310), &m);
        mul_mem(fx, sp + at(0x2D0), sp + at(0x310));
        let age = fx.mem.i32(g + 0xA084);
        let f21 = ee::mul(-3.5, fx.tan_falloff_p(age, 0x18, 1));
        for j in 0..3u32 {
            let p = g + 0xA090 + 0x10 * j;
            fx.point_along(sp + at(0x410), p);
            let a = ((k << 16) as i32 / 24).wrapping_add(((j << 16) as i32) / 3);
            fx.rotate_axis(sp + at(0x410), a, 2);
            vec4(fx, sp + at(0x390), [0.0, 1.5, 0.0, 1.0]);
            vec4(fx, sp + at(0x3A0), [0.0, -1.5, 0.0, 1.0]);
            fx.vu_apply_to(sp + at(0x390), sp + at(0x410), sp + at(0x390));
            fx.vu_apply_to(sp + at(0x3A0), sp + at(0x410), sp + at(0x3A0));
            fx.vu_add4(sp + at(0x3D0), sp + at(0x390), p);
            fx.vu_add4(sp + at(0x3E0), sp + at(0x3A0), p);
            fx.vu_scale4(sp + at(0x450), p, f21);
            fx.vu_add4(sp + at(0x3F0), sp + at(0x3D0), sp + at(0x450));
            fx.vu_add4(sp + at(0x400), sp + at(0x3E0), sp + at(0x450));
            let b = u32::from(fx.mem.read_u8(g + 0xA088 + j));
            let (c, r) = (ee::from_i32(32 * (b & 3) as i32), ee::from_i32(32 * (b >> 2) as i32));
            let u0 = ee::div(c, 128.0);
            let u1 = ee::div(ee::add(32.0, c), 128.0);
            let v0 = ee::div(r, 64.0);
            let v1 = ee::div(ee::add(32.0, r), 64.0);
            for (o, x) in [(0x4F0u32, u0), (0x4F4, v0), (0x508, u1), (0x50C, v0), (0x520, u0), (0x524, v1), (0x538, u1), (0x53C, v1)] {
                fx.mem.setf(sp + at(o), x);
            }
            let mm = fx.read_m4(sp + at(0x2D0));
            if fx.project_desc(sp + at(0x3D0), PROJ_OUT, Some(&mm), d, sp + at(0x56C)) {
                crate::lens::far_z(fx, sp + at(0x4F8), &[sp + at(0x510), sp + at(0x528), sp + at(0x540)]);
                fx.submit(d, OT, 0);
            }
        }
        let age = fx.mem.i32(g + 0xA084) + 1;
        fx.mem.set32(g + 0xA084, age);
        if age >= 0x19 {
            fx.mem.write_u8(g + 0xA080, 0);
        }
    }
}

/// The sheet [F 0x1E02F18..0x1E03560]: from 8 for 0x109 updates (D+0xD264
/// / age D+0xD268) a 33 x 17 cylinder D+0xA690 (rows 0x110 apart, column
/// 16 = column 0) of radius 180 + 80 p + 40 sin(phase + 0x33333 p / 2 +
/// 0x8AE age) about z, z -3000 + 6000 i / 32, drawn twice (p 0 / 1) 50
/// up as textured quads (texture 1, prim 0x24, ALPHA preset 0, CLAMP 0),
/// colour (94, 9E, FF) at 64 / 116 ratio_below(age, 16) ramp_above(age,
/// 264, 232) sin(i / 64 of a turn), ST tiles 2 x 2 scrolled by sin(+-((age
/// + 32 p) % 128) << 9) in u and 2 (age % 32) / 32 side in v, keyed by
/// mean w.
fn sheet(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 8 {
        fx.mem.write_u8(D + SHEET_ON, 1);
        fx.mem.write_u32(D + SHEET_ON + 4, 0);
    }
    if fx.mem.read_u8(D + SHEET_ON) != 1 {
        return;
    }
    let d = sp + SHEET_DESC;
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    let age = ee::from_i32(fx.mem.i32(D + SHEET_ON + 4));
    let f20 = Fx::ratio_below(age, 16.0);
    let age = ee::from_i32(fx.mem.i32(D + SHEET_ON + 4));
    let f1 = ee::mul(f20, Fx::ramp_above(age, 264.0, 232.0));
    let f20 = ee::mul(64.0, f1);
    let f21 = ee::mul(116.0, f1);
    fx.mem.write_u32(sp + 0x2F0, 0);
    fx.mem.write_u32(sp + 0x1D0, 0);
    fx.mem.write_u32(sp + 0x1E0, 0);
    for p in 0..2i32 {
        fx.mem.set32(sp + 0x2F0, p);
        fx.camera_vp_to(sp + 0x300);
        fx.identity(sp + 0x340);
        vec4(fx, sp + 0x480, [0.0, 50.0, 0.0, 1.0]);
        fx.translate_rows(sp + 0x340, sp + 0x340, sp + 0x480);
        mul_mem(fx, sp + 0x300, sp + 0x340);
        let f2 = ee::mul(40.0, ee::from_i32(p));
        let f22 = ee::add(180.0, ee::mul(2.0, f2));
        let s1b0 = fx.mem.i32(sp + 0x1D0) >> 1;
        for i in 0..0x21u32 {
            let f24 = ee::add(-3000.0, ee::div(ee::mul(6000.0, ee::from_i32(i as i32)), 32.0));
            let row = D + SHEET + 0x110 * i;
            for j in 0..0x10u32 {
                let ph = fx.mem.i32(D + PHASES + 0x44 * i + 4 * j);
                let age = fx.mem.i32(D + SHEET_ON + 4);
                let a = ph.wrapping_add(s1b0).wrapping_add(age.wrapping_mul(0x8AE));
                let s = fx.tsin(a & 0xFFFF);
                let f23 = ee::add(f22, ee::mul(40.0, s));
                let an = (j << 12) as i32;
                let q = row + 0x10 * j;
                let s = fx.tsin(an);
                fx.mem.setf(q, ee::mul(f23, s));
                let c = fx.tcos(an);
                fx.mem.setf(q + 4, ee::mul(f23, c));
                fx.mem.setf(q + 8, f24);
                fx.mem.write_u32(q + 0xC, 0x3F80_0000);
            }
            words(fx, row + 0x100, row, 4);
        }
        let t = fx.mem.i32(D + SHEET_ON + 4).wrapping_add(fx.mem.i32(sp + 0x1E0));
        let a0 = ((t % 128) << 16) >> 7;
        let sg = 2 * (p % 2) - 1;
        let s2 = sg.wrapping_mul(a0) & 0xFFFF;
        let f22 = fx.tsin(s2);
        let _ = fx.tcos(s2);
        let age = fx.mem.i32(D + SHEET_ON + 4);
        let f3 = ee::from_i32(age % 32);
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let f23 = ee::add(ee::mul(ee::div(ee::mul(2.0, f3), 32.0), side), 0.0);
        fx.mem.write_u32(sp + 0x1C0, 0);
        for i in 0..0x20u32 {
            fx.mem.set32(sp + 0x260, i as i32);
            let r = (i % 2) as i32;
            let f24 = ee::div(ee::from_i32(r), 2.0);
            let f25 = ee::div(ee::from_i32(r + 1), 2.0);
            let a = fx.mem.i32(sp + 0x1C0) >> 5;
            let s = fx.tsin(a);
            let al = if p == 0 { fixu(fx, ee::mul(f20, s), 0x01E0_3378) } else { fixu(fx, ee::mul(f21, s), 0x01E0_338C) };
            let f24 = ee::add(f24, f23);
            let f25 = ee::add(f25, f23);
            rgba(fx, d + 0x10, [0x94, 0x9E, 0xFF], al as u8);
            let (s3, s2) = (D + SHEET + 0x110 * i, D + SHEET + 0x110 * (i + 1));
            for j in 0..0x10u32 {
                let c = (j % 2) as i32;
                let u0 = ee::add(ee::div(ee::from_i32(c), 2.0), f22);
                let u1 = ee::add(ee::div(ee::from_i32(c + 1), 2.0), f22);
                for (o, x) in [(0x18u32, u0), (0x1C, f24), (0x28, u1), (0x2C, f24), (0x38, u0), (0x3C, f25), (0x48, u1), (0x4C, f25)] {
                    fx.mem.setf(d + o, x);
                }
                let vs = [s3 + 0x10 * j, s3 + 0x10 * (j + 1), s2 + 0x10 * j, s2 + 0x10 * (j + 1)];
                if fx.project_n(&vs, sp + 0x300, d, sp + 0x61C, 0x01E0_3484) == 1 {
                    let w = fx.mem.f(sp + 0x61C);
                    let key = fixu(fx, w, 0x01E0_3498);
                    fx.submit(d, OT, key as i32);
                }
            }
            let x = fx.mem.i32(sp + 0x1C0).wrapping_add(0x8000);
            fx.mem.set32(sp + 0x1C0, x);
        }
        fx.mem.write_u32(sp + 0x260, 0x20);
        let x = fx.mem.i32(sp + 0x1D0).wrapping_add(0x3_3333);
        fx.mem.set32(sp + 0x1D0, x);
        let x = fx.mem.i32(sp + 0x1E0) + 0x20;
        fx.mem.set32(sp + 0x1E0, x);
    }
    fx.mem.write_u32(sp + 0x2F0, 2);
    let age = fx.mem.i32(D + SHEET_ON + 4) + 1;
    fx.mem.set32(D + SHEET_ON + 4, age);
    if age >= 0x109 {
        fx.mem.write_u8(D + SHEET_ON, 0);
    }
}

/// The backdrop [F 0x1E03564..0x1E037B4]: with f = clamp(n - 8, 0, 8)
/// clamp(0x110 - n, 0, 8) / 64 > 0 a full-window sprite (0x7000 ..
/// 0x9000, far Z) of the frame (0x207EF0, filter 1, ALPHA preset 0, CLAMP
/// 5) at UVs (0x14 .. 0x1EC) << 4 jittered by rand % 3 - 1 each way,
/// grey 0x9E at 64 f, key 0.
fn backdrop(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n - 8, 0, 8));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(0x110 - n, 0, 8));
    let f20 = ee::div(ee::mul(a, b), 64.0);
    if crate::exe::ee_le(f20, 0.0) {
        return;
    }
    let d = sp + at(DESC);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.desc_clamp(d, 5);
    let r = rnd(fx, OT, 0x01E0_364C);
    let ju = r % 3 - 1;
    let r = rnd(fx, OT, 0x01E0_366C);
    let jv = r % 3 - 1;
    uv(fx, d + 0x18, ((ju + 0x14) << 4) as u32);
    uv(fx, d + 0x1A, ((jv + 0x14) << 4) as u32);
    uv(fx, d + 0x28, ((ju + 0x1EC) << 4) as u32);
    uv(fx, d + 0x2A, ((jv + 0x1EC) << 4) as u32);
    let al = fixu(fx, ee::mul(64.0, f20), 0x01E0_3718);
    rgba(fx, d + 0x10, [0x9E, 0x9E, 0x9E], al as u8);
    crate::lens::far_xy(fx, d + 0x20, 0x7000, 0x7000);
    crate::lens::far_xy(fx, d + 0x30, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
}

/// The rises [F 0x1E037B8..0x1E03BF4]: on 1..0x3B one of 32 records (0x60
/// at D+0xD270) at the attacker's point (+0x10) with four cells 4..7
/// (+8) and 1.0 vectors (+0x20); each drawn for 0x21 updates (texture 2)
/// as four sprites 5 (0.3 + 0.7 tf) wide at the point + the vector x tf
/// (tf = tan falloff(age, 32)), (94, 9E, C8) at 100 sin(age / 64 of a
/// turn), keyed by mean w.
fn rises(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x3C {
        let g = D + RISES + 0x60 * (n % 32) as u32;
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        words(fx, g + 0x10, D + 0x40, 4);
        for j in 0..4u32 {
            let b = rnd(fx, OT, 0x01E0_3848);
            fx.mem.set8(g + 8 + j, b % 4 + 4);
            fx.random_table_vec(g + 0x20 + 0x10 * j, 1.0);
        }
    }
    head(fx, sp, 0x18, 1);
    camera(fx, sp);
    for k in 0..0x20u32 {
        let g = D + RISES + 0x60 * k;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 4);
        let s = fx.tsin((age << 15) >> 5);
        let al = fixu(fx, ee::mul(100.0, s), 0x01E0_3968);
        rgba(fx, sp + at(0x490), STRIP_RGB, al as u8);
        let age = fx.mem.i32(g + 4);
        let tf = fx.tan_falloff_p(age, 0x20, 1);
        let s = ee::mul(5.0, ee::add(f32::from_bits(0x3E99_999A), ee::mul(f32::from_bits(0x3F33_3333), tf)));
        corners(fx, sp, s, ee::neg(s));
        let age = fx.mem.i32(g + 4);
        let f20 = fx.tan_falloff_p(age, 0x20, 1);
        for j in 0..4u32 {
            let b = u32::from(fx.mem.read_u8(g + 8 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + at(0x450), g + 0x20 + 0x10 * j, f20);
            fx.vu_add4(sp + at(0x450), sp + at(0x450), g + 0x10);
            sprite(fx, sp, 0x01E0_3B6C);
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(g, 0);
        }
    }
}
