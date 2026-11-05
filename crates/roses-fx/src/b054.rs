//! BMODULE b054 (card 400), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080 in a 0x640 frame (s0 = D, s1 = the
//! ordering table), data area a0 = 0x1ED0000, 0xA870 bytes) [F] (helper
//! D33, 3 Nov 2026). b624 (card 605, b291.rs's V624) at 0.928 masked: its
//! code with b291's frame offsets from 0x230 at + 0xD0 (`at`, mean-w slot
//! 0x638), node 0x3C, every colour (C8, 0, 0), the attacker's point
//! (0x1F65D0 kind 1, D+0x40) taken before the aim (node - target), the target's (kind 1,
//! D+0x50) in the init, a new timeline (tube 0x56 for 0x7C updates,
//! spirals 0x57..0xD1, bursts / flecks 0x57..0xD1, puffs 0x67..0xD1, the
//! glow on 0, blades 1..0x22, fades to 0xD2), the tube's radius soft double
//! on (20 - i) / 20, bursts / puffs dropping by 12 / 10 (0.5 (-0.0109
//! powf(age, 2))), the glow / blades at the attacker's point, and in place
//! of b624's motes a 64-sprite shower (D+0xA410, from 0x24 for 0x21
//! updates); then the outcome boundary 0x1F8100 on the last pool's a0..a3
//! and at 1 from 0x93 0x1F77A0(D+8, 2, (0, 40, 60, 100 clamp(n - 0x92, 0,
//! 0x40) >> 6)). Returns 1 on 0x4C, 6 on 0x57..0xD0, 2 on 0xD1, 8 from
//! 0xD3.
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::LENS_CLAMP;

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x640;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers), b291's 0x480.
const DESC: u32 = 0x480;
/// The strip descriptor (prim 0x34, 12 registers), b291's 0x4D8.
const STRIP: u32 = 0x4D8;
/// b291's frame offsets from 0x230 sit 0xD0 higher here.
const LO: u32 = 0xD0;
/// The mean-w slot (b291's 0x56C).
const W: u32 = 0x638;
/// The fade's colour bytes.
const FADE: u32 = 0x63C;
/// Every colour of the record.
const RED: [u8; 3] = [0xC8, 0, 0];
/// The fields from D+0x40 of b291 sit 0x10 higher.
const DZ: u32 = D + 0x10;

/// b291's frame offset `x` as this record's.
fn at(x: u32) -> u32 {
    match x {
        0x230..=0x56B => x + LO,
        0x56C => W,
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

/// 0x1A7230 fabsf (its argument left at its sp).
fn fabsf(fx: &mut Fx, x: f32) -> f32 {
    fx.mem.write_f32(fx.sp - 0x10, x);
    f32::from_bits(x.to_bits() & 0x7FFF_FFFF)
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

/// The sprite descriptor head's words (prim 0x26 | 0x800, 5 registers).
fn head_words(fx: &mut Fx, sp: u32) -> u32 {
    let d = sp + at(DESC);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    d
}

/// The sprite descriptor head; texture `tex` (filter 1), ALPHA preset
/// `preset`, CLAMP 5.
fn head(fx: &mut Fx, sp: u32, tex: u32, preset: i32) {
    let d = head_words(fx, sp);
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

/// The camera's view-projection x the aim D+0x60 at the target D+0x50.
fn aimed(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + at(0x2D0));
    fx.translate_rows(sp + at(0x310), DZ + 0x50, DZ + 0x40);
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
/// by its mean w; returns a0..a3 as the last callee leaves them.
fn sprite(fx: &mut Fx, sp: u32, ra: u32) -> [u32; 4] {
    fx.vu_add4(sp + at(0x3D0), sp + at(0x390), sp + at(0x450));
    fx.vu_add4(sp + at(0x3E0), sp + at(0x3A0), sp + at(0x450));
    draw(fx, sp, ra)
}

fn draw(fx: &mut Fx, sp: u32, ra: u32) -> [u32; 4] {
    let d = sp + at(DESC);
    if crate::fxlib2::project(fx, sp + at(0x3D0), PROJ_OUT, sp + at(0x2D0), d, sp + W) {
        let w = fx.mem.f(sp + W);
        let key = fixu(fx, w, ra);
        fx.submit(d, OT, key as i32);
        [d, OT, key, 0]
    } else {
        [sp + at(0x3D0), PROJ_OUT, sp + at(0x2D0), d]
    }
}

fn clamp32(v: i32) -> f32 {
    ee::from_i32(Fx::clamp_i(v, 0, 0x20))
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xA870);
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
    let r = fx.request_node(h, 0x3C, 1);
    fx.mem.set32(D + 0x20, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x50);
    fx.mem.write_u8(DZ + 0x2FA0, 0);
    fx.mem.write_u32(DZ + 0x2FA4, 0);
    for k in 0..0x30u32 {
        fx.random_table_vec(DZ + 0x5AB0 + 0x10 * k, 1.5);
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
    for k in 0..8u32 {
        fx.mem.write_u8(DZ + 0xA080 + 0x70 * k, 0);
    }
    fx.mem.write_u8(D + 0xA410, 0);
    let t = fx.mem.i32(D + 8);
    fx.unit_add_mode(t, 2, 0, [u64::from(D), u64::from(OT)], 0x01E0_0254);
    0
}

/// (float)((double)(0.1 + 0.9 i / 20) x (0.8 + (double)(0.2 sin a))) (the
/// soft-double chain 0x1A2C30 / 0x1A3118 / 0x1A31D8; `k` the extra
/// (double) factor of the spirals).
fn radius(fx: &mut Fx, i: i32, row: i32, age: i32, k: Option<f32>) -> f32 {
    let (_, r) = fx.div(row, 12);
    let a = ((r << 16) / 12).wrapping_sub(0x8AE * age);
    let s = fx.tsin(a & 0xFFFF);
    let d1 = 0.8f64 + f64::from(ee::mul(f32::from_bits(0x3E4C_CCCD), s));
    let f2 = ee::mul(f32::from_bits(0x3F66_6666), ee::from_i32(i));
    let f12 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::div(f2, 20.0));
    let p = f64::from(f12) * d1;
    match k {
        Some(k) => (f64::from(k) * p) as f32,
        None => p as f32,
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + at(0x350));
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x40);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x20));
    fx.node_position(h, slot, D + 0x30);
    fx.vu_sub4(sp + at(0x450), D + 0x30, DZ + 0x40);
    fx.point_along(DZ + 0x50, sp + at(0x450));
    fx.rotate_axis(DZ + 0x50, 0x4000, 0);
    let x2 = Fx::powf2(fx.mem.f(sp + at(0x450)));
    let y2 = Fx::powf2(fx.mem.f(sp + at(0x454)));
    let f20 = ee::add(x2, y2);
    let z2 = Fx::powf2(fx.mem.f(sp + at(0x458)));
    let d = fx.sqrtf(ee::add(z2, f20));
    fx.mem.setf(DZ + 0x90, d);
    if fx.mem.i32(D) == 0x56 {
        fx.mem.write_u8(DZ + 0x2FA0, 1);
        fx.mem.write_u32(DZ + 0x2FA4, 0);
    }
    let age = fx.mem.i32(DZ + 0x2FA4);
    let f20 = ee::div(clamp32(0x7C - age), 32.0);
    if fx.mem.read_u8(DZ + 0x2FA0) == 1 {
        tube(fx, sp, f20);
    }
    let n = fx.mem.i32(D);
    if (0x57..0xD2).contains(&n) {
        spiral_spawn(fx, sp, n);
    }
    spirals(fx, sp);
    let n = fx.mem.i32(D);
    if (0x57..0xD2).contains(&n) {
        let (_, r) = fx.div(n, 0x12);
        let g = DZ + 0x340 * r as u32;
        fx.mem.write_u8(g + 0x5DB0, 1);
        fx.mem.write_u32(g + 0x5DB4, 0);
        for j in 0..0x30u32 {
            let b = fx.rand_from(D, 0x01E0_1880) >> 23;
            fx.mem.set8(g + 0x5DB8 + j, b % 8);
            fx.random_table_vec(sp + at(0x450), 0.5);
            fx.vu_add4(g + 0x5DF0 + 0x10 * j, sp + at(0x450), DZ + 0x5AB0 + 0x10 * j);
        }
    }
    bursts(fx, sp);
    let n = fx.mem.i32(D);
    if (0x57..0xD2).contains(&n) {
        let (_, r) = fx.div(n, 0xC);
        let g = DZ + 0x50 * r as u32;
        fx.mem.write_u8(g + 0x9830, 1);
        fx.mem.write_u32(g + 0x9834, 0);
        for j in 0..4u32 {
            let b = fx.rand_from(D, 0x01E0_1CFC) >> 23;
            fx.mem.set8(g + 0x9838 + j, b % 8);
            fx.random_table_vec(g + 0x9840 + 0x10 * j, f32::from_bits(0x3FA6_6666));
        }
    }
    flecks(fx, sp);
    let n = fx.mem.i32(D);
    if (0x67..0xD2).contains(&n) {
        let (_, r) = fx.div(n, 0xC);
        let g = DZ + 0x60 * r as u32;
        fx.mem.write_u8(g + 0x9BF0, 1);
        fx.mem.write_u32(g + 0x9BF4, 0);
        words(fx, g + 0x9C00, D + 0x30, 4);
        for j in 0..4u32 {
            let b = fx.rand_from(D, 0x01E0_217C) >> 23;
            fx.mem.set8(g + 0x9BF8 + j, b % 8);
            fx.random_table_vec(g + 0x9C10 + 0x10 * j, 0.5);
        }
    }
    puffs(fx, sp);
    let args = blades(fx, sp);
    let args = motes(fx, sp, args);
    // The fade of a destroyed target from 0x93 [F 0x1E033F8..0x1E03468].
    if fx.outcome(args) == 1 && fx.mem.i32(D) >= 0x93 {
        let n = fx.mem.i32(D);
        let c = Fx::clamp_i(n - 0x92, 0, 0x40);
        fx.mem.write_u8(sp + FADE + 2, 0x60);
        fx.mem.write_u8(sp + FADE + 1, 0x40);
        fx.mem.write_u8(sp + FADE, 0);
        fx.mem.write_u8(sp + FADE + 3, ((c * 100) >> 6) as u8);
        let t = fx.mem.i32(D + 8);
        fx.unit_add_mode(t, 2, sp + FADE, [u64::from(D), u64::from(OT)], 0x01E0_346C);
    }
    finish(fx, sp)
}

fn finish(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x4C {
        ret = 1;
    } else {
        if (0x57..0xD1).contains(&n) {
            ret = 6;
        }
        if n == 0xD1 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xD3 {
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
fn row_colours(fx: &mut Fx, sp: u32, j: u32, a: u8) {
    let (top, bottom) = match j {
        0 => (0, a),
        0x13 => (a, 0),
        _ => (a, a),
    };
    rgba(fx, sp + at(0x4E8), RED, top);
    words(fx, sp + at(0x500), sp + at(0x4E8), 2);
    rgba(fx, sp + at(0x518), RED, bottom);
    words(fx, sp + at(0x530), sp + at(0x518), 2);
}

/// The tube [F 0x1E00314..0x1E010BC].
fn tube(fx: &mut Fx, sp: u32, f20: f32) {
    let age = fx.mem.i32(DZ + 0x2FA4);
    // 21 centre points (all 0 x sin / cos).
    for s2 in 0..0x15u32 {
        let a = (1820 * age).wrapping_add(((s2 << 16) / 20) as i32) & 0xFFFF;
        let s = fx.tsin(a);
        let p = DZ + 0x2E50 + 0x10 * s2;
        fx.mem.setf(p, ee::mul(0.0, s));
        fx.mem.write_u32(p + 4, 0);
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(0.0, c));
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
    }
    let age = fx.mem.i32(DZ + 0x2FA4);
    let f3 = ee::from_i32(Fx::clamp_i(age, 0, 0x10));
    let f21 = ee::div(ee::mul(fx.mem.f(DZ + 0x90), f3), 16.0);
    let f22 = ee::add(0.5, ee::div(ee::mul(0.5, f3), 16.0));
    let d = sp + at(STRIP);
    strip_head(fx, sp);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    let a_in = fixu(fx, ee::mul(64.0, f20), 0x01E0_04E8) as u8;
    fx.mem.write_u8(sp + 0xD0, a_in);
    ring(fx, ee::mul(4.5, f22));
    // The textured grid.
    aimed(fx, sp);
    grid(fx, f21, None);
    let age = fx.mem.i32(DZ + 0x2FA4);
    let (_, r) = fx.div(age, 10);
    let f20b = ee::sub(0.0, ee::div(ee::from_i32(r), 10.0));
    let mut regs = [0u32; 4];
    for j in 0..0x14u32 {
        row_colours(fx, sp, j, a_in);
        let f24 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(j as i32)), 20.0), f20b);
        let f25 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(j as i32 + 1)), 20.0), f20b);
        let (s4, s5) = (DZ + 0x110 * j, DZ + 0x110 * (j + 1));
        for s6 in 0..0x10u32 {
            let s0 = ee::div(ee::from_i32(s6 as i32), 16.0);
            let s1 = ee::div(ee::from_i32(s6 as i32 + 1), 16.0);
            for (o, x) in [(0x4F0u32, s0), (0x4F4, f24), (0x508, s1), (0x50C, f24), (0x520, s0), (0x524, f25), (0x538, s1), (0x53C, f25)] {
                fx.mem.setf(sp + at(o), x);
            }
            let vs = [s4 + 0x10 * s6 + 0x1B0, s4 + 0x10 * (s6 + 1) + 0x1B0, s5 + 0x10 * s6 + 0x1B0, s5 + 0x10 * (s6 + 1) + 0x1B0];
            if fx.project_n(&vs, sp + at(0x2D0), d, sp + W, 0x01E0_0954) == 1 {
                let w = fx.mem.f(sp + W);
                let key = fixu(fx, w, 0x01E0_0968);
                fx.submit(d, OT, key as i32);
                regs = [d, OT, key, d];
            } else {
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
    let a_in = fixu(fx, ee::mul(128.0, f20), 0x01E0_0A80) as u8;
    fx.mem.write_u8(sp + at(0x2C0), a_in);
    ring(fx, ee::mul(4.0, f22));
    aimed(fx, sp);
    grid(fx, f21, Some(sp));
    for j in 0..0x14u32 {
        row_colours(fx, sp, j, a_in);
        let (s7, fp) = (DZ + 0x110 * j, DZ + 0x110 * (j + 1));
        for s6 in 0..0x10u32 {
            let (s3, s5, s2, s4) = (s7 + 0x10 * s6, s7 + 0x10 * (s6 + 1), fp + 0x10 * s6, fp + 0x10 * (s6 + 1));
            if fx.project_n(&[s3 + 0x1800, s5 + 0x1800, s2 + 0x1800, s4 + 0x1800], sp + at(0x2D0), d, sp + W, 0x01E0_0F78) != 1 {
                continue;
            }
            for k in 0..4u32 {
                fx.uv_from_xy(sp + at(0x4F0) + 0x18 * k, sp + at(0x4F8) + 0x18 * k, 0, 0);
            }
            if fx.project_n(&[s3 + 0x1B0, s5 + 0x1B0, s2 + 0x1B0, s4 + 0x1B0], sp + at(0x2D0), d, sp + W, 0x01E0_0FF4) != 1 {
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
    if age >= 0x7D {
        fx.mem.write_u8(DZ + 0x2FA0, 0);
    }
}

/// The 21 x 17 grid D+0x1C0 (rows 0x110 apart) along the aim, radius the
/// soft double of (20 - i), row y f21 i / 20; with `lens` (the frame) also
/// the wobbling copy D+0x1810 turned about y.
fn grid(fx: &mut Fx, f21: f32, lens: Option<u32>) {
    let age = fx.mem.i32(DZ + 0x2FA4);
    for s6 in 0..0x15u32 {
        let i = s6 as i32;
        let f20 = ee::div(ee::mul(f21, ee::from_i32(i)), 20.0);
        let f24 = radius(fx, 20 - i, i, age, None);
        let mut s5 = 0;
        if let Some(sp) = lens {
            fx.identity(sp + at(0x410));
            let (_, r) = fx.div(age, 0x2E);
            let a0 = (r << 16) / 46;
            s5 = (i % 8) << 13;
            let s = fx.tsin(s5.wrapping_add(a0) & 0xFFFF);
            let a = fixs(fx, ee::mul(8192.0, s), 0x01E0_0CAC);
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
                let f1 = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::div(f21, 20.0));
                fx.mem.setf(q + 4, ee::add(f20, ee::mul(f1, s)));
            }
        }
    }
}

/// 0x101430(a, b, c) [F] (b212's): 1 when the triangle of the XYZ2 x / y
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

/// The spirals' spawn [F 0x1E010DC..0x1E01438].
fn spiral_spawn(fx: &mut Fx, sp: u32, n: i32) {
    let off = 0x560 * (n % 8) as u32;
    let g = DZ + off;
    fx.mem.write_u8(g + 0x2FB0, 1);
    fx.mem.write_u32(g + 0x2FB4, 0);
    fx.vu_sub4(sp + at(0x450), D + 0x30, DZ + 0x40);
    let age = fx.mem.i32(DZ + 0x2FA4);
    let f = ee::div(ee::from_i32(Fx::clamp_i(age, 0, 0x10)), 16.0);
    fx.vu_scale4(sp + at(0x450), sp + at(0x450), f);
    let age = fx.mem.i32(DZ + 0x2FA4);
    let f1 = ee::from_i32(Fx::clamp_i(age, 0, 0x10));
    let f21 = ee::mul(4.5, ee::add(0.5, ee::div(ee::mul(0.5, f1), 16.0)));
    for s7 in 0..0x14u32 {
        let age = fx.mem.i32(DZ + 0x2FA4);
        let f20 = radius(fx, s7 as i32, s7 as i32, age, Some(f21));
        for s5 in 0..4u32 {
            let b = fx.rand_from(D, 0x01E0_12D8) >> 23;
            fx.mem.set8(g + 0x2FB8 + 4 * s7 + s5, b % 8);
            let r = fx.rand_from(D, 0x01E0_12F8) >> 23;
            let f2 = ee::div(ee::mul(f32::from_bits(0x3D4C_CCCD), ee::from_i32(r % 256)), 256.0);
            let f0 = ee::div(ee::from_i32(20 - s7 as i32), 20.0);
            fx.vu_scale4(sp + at(0x460), sp + at(0x450), ee::add(f0, f2));
            fx.vu_add4(sp + at(0x460), sp + at(0x460), DZ + 0x40);
            let r = fx.rand_from(D, 0x01E0_1380) >> 23;
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

/// The spirals [F 0x1E01440..0x1E01820].
fn spirals(fx: &mut Fx, sp: u32) {
    head(fx, sp, 0x18, 0);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let a = clamp32(n - 0x56);
    let n = fx.mem.i32(D);
    let b = clamp32(0xD2 - n);
    let f20 = ee::div(ee::mul(a, b), 1024.0);
    for k in 0..8u32 {
        let g = DZ + 0x560 * k;
        if fx.mem.read_u8(g + 0x2FB0) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x2FB4);
        let s = fx.tsin((age << 15) >> 3);
        let al = fixu(fx, ee::mul(f20, ee::mul(50.0, s)), 0x01E0_1574);
        rgba(fx, sp + at(0x490), RED, al as u8);
        for j in 0..0x14u32 {
            let age = fx.mem.i32(g + 0x2FB4);
            let tf = fx.tan_falloff_p(age, 8, 1);
            let f3 = ee::mul(4.0, ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf)));
            let s = ee::mul(ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(j as i32)), 20.0)), f3);
            corners(fx, sp, s, ee::neg(s));
            for s6 in 0..4u32 {
                let b = u32::from(fx.mem.read_u8(g + 0x2FB8 + 4 * j + s6));
                cell_uvs(fx, sp, b);
                words(fx, sp + at(0x450), g + 0x3010 + 0x40 * j + 0x10 * s6, 4);
                sprite(fx, sp, 0x01E0_1770);
            }
        }
        let age = fx.mem.i32(g + 0x2FB4) + 1;
        fx.mem.set32(g + 0x2FB4, age);
        if age >= 9 {
            fx.mem.write_u8(g + 0x2FB0, 0);
        }
    }
}

/// 12 (0.5 (-0.0109 powf(age, 2))) / 10 (...): the drop of the bursts /
/// puffs.
fn drop(k: f32, age: f32) -> f32 {
    let p = Fx::powf2(age);
    let f2 = ee::mul(f32::from_bits(0xBC32_864D), p);
    ee::mul(k, ee::mul(0.5, f2))
}

/// The y of sp + at(0x450) plus `dy`.
fn lower(fx: &mut Fx, sp: u32, dy: f32) {
    let y = fx.mem.f(sp + at(0x454));
    fx.mem.setf(sp + at(0x454), ee::add(y, dy));
}

/// The bursts at the target [F 0x1E018E0..0x1E01C94].
fn bursts(fx: &mut Fx, sp: u32) {
    head(fx, sp, 0x18, 0);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(clamp32(0xD2 - n), 32.0);
    for k in 0..0x12u32 {
        let g = DZ + 0x340 * k;
        if fx.mem.read_u8(g + 0x5DB0) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(g + 0x5DB4));
        let r = Fx::ramp_above(age, 18.0, 12.0);
        let al = fixu(fx, ee::mul(f20, ee::mul(50.0, r)), 0x01E0_19EC);
        rgba(fx, sp + at(0x490), RED, al as u8);
        let age = fx.mem.i32(g + 0x5DB4);
        let tf = fx.tan_falloff_p(age, 0x12, 1);
        let s = ee::mul(5.0, ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf)));
        corners(fx, sp, s, ee::neg(s));
        let age = ee::from_i32(fx.mem.i32(g + 0x5DB4));
        let f22 = ee::add(f32::from_bits(0x3D4C_CCCD), ee::div(age, 18.0));
        let f21 = drop(12.0, age);
        for j in 0..0x30u32 {
            let b = u32::from(fx.mem.read_u8(g + 0x5DB8 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + at(0x450), g + 0x5DF0 + 0x10 * j, f22);
            fx.vu_add4(sp + at(0x450), sp + at(0x450), DZ + 0x40);
            lower(fx, sp, f21);
            sprite(fx, sp, 0x01E0_1C20);
        }
        let age = fx.mem.i32(g + 0x5DB4) + 1;
        fx.mem.set32(g + 0x5DB4, age);
        if age >= 0x13 {
            fx.mem.write_u8(g + 0x5DB0, 0);
        }
    }
}

/// The flecks at the target [F 0x1E01D50..0x1E020F0].
fn flecks(fx: &mut Fx, sp: u32) {
    head(fx, sp, 0x1C, 0);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(clamp32(0xD2 - n), 32.0);
    for k in 0..0xCu32 {
        let g = DZ + 0x50 * k;
        if fx.mem.read_u8(g + 0x9830) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x9834);
        let c = fx.tsin((age << 15) / 12);
        let al = fixu(fx, ee::mul(f20, ee::mul(50.0, c)), 0x01E0_1E8C);
        rgba(fx, sp + at(0x490), RED, al as u8);
        corners(fx, sp, 5.0, -5.0);
        let age = ee::from_i32(fx.mem.i32(g + 0x9834));
        let f22 = ee::add(f32::from_bits(0x3E99_999A), ee::div(ee::mul(f32::from_bits(0x3F33_3333), age), 12.0));
        for j in 0..4u32 {
            let b = u32::from(fx.mem.read_u8(g + 0x9838 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + at(0x450), g + 0x9840 + 0x10 * j, f22);
            fx.vu_add4(sp + at(0x450), sp + at(0x450), DZ + 0x40);
            sprite(fx, sp, 0x01E0_2054);
        }
        let age = fx.mem.i32(g + 0x9834) + 1;
        fx.mem.set32(g + 0x9834, age);
        if age >= 0xD {
            fx.mem.write_u8(g + 0x9830, 0);
        }
    }
}

/// The puffs at the node [F 0x1E021C8..0x1E025EC].
fn puffs(fx: &mut Fx, sp: u32) {
    head(fx, sp, 0x18, 0);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(clamp32(0xD2 - n), 32.0);
    for k in 0..0xCu32 {
        let g = DZ + 0x60 * k;
        if fx.mem.read_u8(g + 0x9BF0) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x9BF4);
        let c = fx.tcos((age << 14) / 12);
        let al = fixu(fx, ee::mul(f20, ee::mul(96.0, c)), 0x01E0_22F8);
        rgba(fx, sp + at(0x490), RED, al as u8);
        let age = ee::from_i32(fx.mem.i32(g + 0x9BF4));
        let s = ee::mul(5.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 12.0)));
        corners(fx, sp, s, ee::neg(s));
        let age = ee::from_i32(fx.mem.i32(g + 0x9BF4));
        let f21 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F66_6666), age), 12.0));
        let f22 = drop(10.0, age);
        for j in 0..4u32 {
            let b = u32::from(fx.mem.read_u8(g + 0x9BF8 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + at(0x450), g + 0x9C10 + 0x10 * j, f21);
            fx.vu_add4(sp + at(0x450), sp + at(0x450), g + 0x9C00);
            lower(fx, sp, f22);
            sprite(fx, sp, 0x01E0_2554);
        }
        let age = fx.mem.i32(g + 0x9BF4) + 1;
        fx.mem.set32(g + 0x9BF4, age);
        if age >= 0xD {
            fx.mem.write_u8(g + 0x9BF0, 0);
        }
    }
}

/// The glow and six-sprite blades at the attacker's point [F
/// 0x1E025F0..0x1E02F34]: the glow from update 0 for 0x24 updates (D+0xA080,
/// 20 wide, cell 4, (FF, 37, 55), alpha 255 ratio_below(age, 16)
/// ramp_above(age, 35, 19) (0.8 + 0.2 (age % 2)), far Z, key 0); on
/// 1..0x22 one of eight blade records (D+0xA090 + 0x70 (n % 8), 8
/// updates): 6 cells + 6 vectors 0.5, drawn as strips (1 x 2 turned along
/// the vector and by 0x2000 k + 0x10000 j / 6 about z, stretched -3.5 tan
/// falloff along it, (FF, 80, B4), alpha (8 - age) 255 / 8 clamp(n, 0, 16)
/// clamp(0x23 - n, 0, 16) / 256, far Z, key 0). Returns a0..a3 as the
/// blades' last callee leaves them.
fn blades(fx: &mut Fx, sp: u32) -> [u32; 4] {
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
        let f0 = Fx::ramp_above(age, 35.0, 19.0);
        let f3 = ee::mul(f20, f0);
        let r = fx.mem.i32(DZ + 0xA074) % 2;
        let f0 = ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(r)));
        let al = fixu(fx, ee::mul(255.0, ee::mul(f0, f3)), 0x01E0_2764);
        rgba(fx, sp + at(0x490), [0xFF, 0x37, 0x55], al as u8);
        corners(fx, sp, 20.0, -20.0);
        cell_uvs(fx, sp, 4);
        words(fx, sp + at(0x450), D + 0x40, 4);
        fx.vu_add4(sp + at(0x3D0), sp + at(0x390), sp + at(0x450));
        fx.vu_add4(sp + at(0x3E0), sp + at(0x3A0), sp + at(0x450));
        if crate::fxlib2::project(fx, sp + at(0x3D0), PROJ_OUT, sp + at(0x2D0), sp + at(DESC), sp + W) {
            crate::lens::far_z(fx, sp + at(0x4A0), &[sp + at(0x4B0)]);
            fx.submit(sp + at(DESC), OT, 0);
        }
        let age = fx.mem.i32(DZ + 0xA074) + 1;
        fx.mem.set32(DZ + 0xA074, age);
        if age >= 0x24 {
            fx.mem.write_u8(DZ + 0xA070, 0);
        }
    }
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x23 {
        let g = DZ + 0x70 * (n % 8) as u32;
        fx.mem.write_u8(g + 0xA080, 1);
        fx.mem.write_u32(g + 0xA084, 0);
        for j in 0..6u32 {
            let b = fx.rand_from(D, 0x01E0_29B0) >> 23;
            fx.mem.set8(g + 0xA088 + j, b % 8);
            fx.random_table_vec(g + 0xA090 + 0x10 * j, 0.5);
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
    let ca = Fx::clamp_i(n, 0, 0x10);
    let a = ee::from_i32(ca);
    let n = fx.mem.i32(D);
    let cb = Fx::clamp_i(0x23 - n, 0, 0x10);
    let b = ee::from_i32(cb);
    let f20 = ee::div(ee::mul(a, b), 256.0);
    // 0x101050 leaves a0 = its answer, a1 / a2 its bounds, a3 as
    // 0x208400 left it (the descriptor + 2).
    let mut last = [cb as u32, 0, 0x10, d + 2];
    for k in 0..8u32 {
        let g = DZ + 0x70 * k;
        if fx.mem.read_u8(g + 0xA080) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0xA084);
        let x = (8 - age).wrapping_mul(255) >> 3;
        let al = fixu(fx, ee::mul(f20, ee::from_i32(x)), 0x01E0_2B08);
        rgba(fx, sp + at(0x4E8), [0xFF, 0x80, 0xB4], al as u8);
        words(fx, sp + at(0x500), sp + at(0x4E8), 2);
        words(fx, sp + at(0x518), sp + at(0x4E8), 2);
        words(fx, sp + at(0x530), sp + at(0x4E8), 2);
        fx.camera_vp_to(sp + at(0x2D0));
        words(fx, sp + at(0x450), D + 0x40, 4);
        let t = std::array::from_fn(|c| fx.mem.f(sp + at(0x450) + 4 * c as u32));
        let m = fx.trs(None, None, Some(t));
        fx.write_m4(sp + at(0x310), &m);
        mul_mem(fx, sp + at(0x2D0), sp + at(0x310));
        let age = fx.mem.i32(g + 0xA084);
        let f21 = ee::mul(-3.5, fx.tan_falloff_p(age, 8, 1));
        for j in 0..6u32 {
            let p = g + 0xA090 + 0x10 * j;
            fx.point_along(sp + at(0x410), p);
            let a = (((k << 16) as i32) >> 3).wrapping_add(((j << 16) as i32) / 6);
            fx.rotate_axis(sp + at(0x410), a, 2);
            vec4(fx, sp + at(0x390), [0.0, 1.0, 0.0, 1.0]);
            vec4(fx, sp + at(0x3A0), [0.0, -1.0, 0.0, 1.0]);
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
            if fx.project_desc(sp + at(0x3D0), PROJ_OUT, Some(&mm), d, sp + W) {
                crate::lens::far_z(fx, sp + at(0x4F8), &[sp + at(0x510), sp + at(0x528), sp + at(0x540)]);
                fx.submit(d, OT, 0);
                // a3 = the first vertex's XYZ2 low word << 8 (the far Z
                // merge's leftover); 0x206F30 keeps it.
                let a3 = fx.mem.read_u32(sp + at(0x4F8)) << 8;
                last = [d, OT, 0, a3];
            } else {
                last = [sp + at(0x3D0), PROJ_OUT, sp + at(0x2D0), d];
            }
        }
        let age = fx.mem.i32(g + 0xA084) + 1;
        fx.mem.set32(g + 0xA084, age);
        if age >= 9 {
            fx.mem.write_u8(g + 0xA080, 0);
        }
    }
    last
}

/// The shower [F 0x1E02F38..0x1E033F4]: on 0x24 (D+0xA410, age +4) a
/// centre 5 below the attacker's point (D+0xA460) and 64 sprites (cells
/// D+0xA418, unit vectors with y = 0.5 |y| at D+0xA470), drawn 0x21
/// updates from the centre out by falloff(age, 32) (texture D+0x1C, CLAMP
/// before the texture, preset 0), (C8, C8, C8, 128 cos(age << 9)), half
/// size 5 (0.5 + 0.5 age / 32), keyed by mean w. Returns a0..a3 as the
/// last callee before 0x1F8100 leaves them (`last` the blades').
fn motes(fx: &mut Fx, sp: u32, mut last: [u32; 4]) -> [u32; 4] {
    let m = D + 0xA410;
    if fx.mem.i32(D) == 0x24 {
        fx.mem.write_u8(m, 1);
        fx.mem.write_u32(m + 4, 0);
        words(fx, D + 0xA460, D + 0x40, 4);
        let y = fx.mem.f(D + 0xA464);
        fx.mem.setf(D + 0xA464, ee::add(y, -5.0));
        for k in 0..0x40u32 {
            let b = fx.rand_from(D, 0x01E0_2FE0) >> 23;
            fx.mem.set8(m + 8 + k, b % 8);
            let q = D + 0xA470 + 0x10 * k;
            fx.random_table_vec(q, 1.0);
            let y = fx.mem.f(q + 4);
            let y = fabsf(fx, y);
            fx.mem.setf(q + 4, ee::mul(0.5, y));
        }
    }
    if fx.mem.read_u8(m) != 1 {
        return last;
    }
    let d = head_words(fx, sp);
    fx.desc_clamp(d, 5);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    camera(fx, sp);
    let age = fx.mem.i32(m + 4);
    let c = fx.tcos((age << 14) >> 5);
    let al = fixu(fx, ee::mul(128.0, c), 0x01E0_3158);
    rgba(fx, sp + at(0x490), [0xC8; 3], al as u8);
    let age = ee::from_i32(fx.mem.i32(m + 4));
    let s = ee::mul(5.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0)));
    corners(fx, sp, ee::neg(s), s);
    let age = fx.mem.i32(m + 4);
    let f20 = fx.tan_falloff_p(age, 0x20, 1);
    for k in 0..0x40u32 {
        let b = u32::from(fx.mem.read_u8(m + 8 + k));
        cell_uvs(fx, sp, b);
        fx.vu_scale4(sp + at(0x450), D + 0xA470 + 0x10 * k, f20);
        fx.vu_add4(sp + at(0x450), sp + at(0x450), D + 0xA460);
        last = sprite(fx, sp, 0x01E0_3354);
    }
    let age = fx.mem.i32(m + 4) + 1;
    fx.mem.set32(m + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(m, 0);
    }
    last
}
