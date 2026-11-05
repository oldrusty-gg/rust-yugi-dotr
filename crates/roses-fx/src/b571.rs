//! BMODULE b571 (card 199), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0x3680
//! bytes) [F]: a single function in a 0x380
//! frame (s0 = D, s1 = the OT, the return at sp + 0xB0). Every update the
//! camera turn with roll (sp + 0x150) and the attacker's nodes 0x36 / 0x38
//! (D+0x50 / D+0x60); the target's position at D+0x30. On 1..0x59 a burst
//! (16 round robin, +0x70, 0x90) of 8 points 2 x a table direction from
//! node 0x36, each drawn 16 times pulled back to the node ((16 - min(age
//! + 0.07 k, 16)) / 16; sprite 0.6, texture D+0x10, (FF, 60, 56), alpha
//! 255 age / 16) for 16 updates. On 0x45..0xC7 a spinning square (32
//! round robin, +0x980, 0x20; quad 40 x 40, texture D+0x14, scaled 0.4 +
//! 0.6 age / 32, alpha 200 (1 - tan falloff) x the square count / 64) at
//! node 0x36 for 32 updates. Every tenth update below 0x5A a ring (16
//! round robin, +0x11B0, 8; the 33-point band D+0xD90 (20 / 50 round),
//! texture D+0x18, shrinking (16 - age) / 16) for 16 updates. From update
//! 0 for 0x6B updates a flat fan (the 33 points D+0x1230, radius 10)
//! pulsing 1 - 0.1 (age & 1) at the node; on 0xBB at the target 64 fans
//! of 3 x 4 quads (D+0x1450, set at init from table directions; texture
//! D+0x1C), two crossed quad sets (table 0x28AC70, 50 x (0.4 + 0.6 age /
//! 48), texture D+0x20) and a flat fan (D+0x3460, radius 50), each for 48
//! updates. Returns 1 on 0xB1, 2 on 0xBB, 8 once the count reaches 0xEC.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70 / 0x205210's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x380;
/// The sprite descriptor (prim 0x26).
const SPRITE: u32 = 0x248;
/// The quad descriptor (prim 0x34, 12 registers).
const QUAD: u32 = 0x2A0;
/// The fan descriptor (prim 0x13, 6 registers).
const FAN: u32 = 0x330;
/// The direction table of 0x101080 (4 rows x 32 vec3).
const DIR_TABLE: u32 = 0x0028_AE90;
/// The crossed quads' corner table (4 quads x 4 vec4).
const QUAD_TABLE: u32 = 0x0028_AC70;

/// The registers 0x20C0B0 saves that the record leaves (f20, s2, s3)
/// [F; the runner's f20 before the first write not known: 0, H].
#[derive(Default)]
struct Regs {
    f20: f32,
    s2: u64,
    s3: u64,
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

/// The compiler's signed remainder by a power of two.
fn rem2(x: i32, m: i32) -> i32 {
    let r = x & (m - 1);
    if x < 0 && r != 0 {
        r - m
    } else {
        r
    }
}

fn words(fx: &mut Fx, to: u32, from: u32, n: u32) {
    for c in 0..n {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn vec4(fx: &mut Fx, p: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        fx.mem.setf(p + 4 * k as u32, *x);
    }
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// A table row (rand % 4) and its entry `col` (rand % 32 x 12).
fn table_row(fx: &mut Fx, ra: u32) -> u32 {
    let r = fx.rand_from(D, ra) >> 23;
    DIR_TABLE.wrapping_add((rem2(r, 4) * 0x180) as u32)
}

/// (0x10000 x k) >> 5 for k = 0..32: a circle of 33 points (`a` / `b`
/// the radii of two rings, or one) from `p`, `stride` apart.
fn circle(fx: &mut Fx, p: u32, stride: u32, radii: &[f32]) {
    for k in 0..0x21u32 {
        let a = ((k << 16) as i32) >> 5;
        let e = p + stride * k;
        for (j, r) in radii.iter().enumerate() {
            let q = e + 0x10 * j as u32;
            let s = fx.tsin(a);
            fx.mem.setf(q, ee::mul(*r, s));
            let c = fx.tcos(a);
            fx.mem.setf(q + 4, ee::mul(*r, c));
            fx.mem.write_u32(q + 8, 0);
            fx.mem.write_u32(q + 0xC, 0x3F80_0000);
        }
    }
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0x3680);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for (k, (kind, id)) in [(5, 0), (5, 1), (4, 0xF), (5, 2), (5, 3)].into_iter().enumerate() {
        let r = fx.texture(kind, id);
        fx.mem.write_u32(D + 0x10 + 4 * k as u32, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x30);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x36, 1);
    fx.mem.set32(D + 0x40, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x38, 0);
    fx.mem.set32(D + 0x44, r);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x70 + 0x90 * k, 0);
    }
    fx.mem.write_u32(D + 0x970, 0);
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x980 + 0x20 * k, 0);
    }
    fx.mem.write_u32(D + 0xD80, 0);
    circle(fx, D + 0xD90, 0x20, &[20.0, 50.0]);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x11B0 + 8 * k, 0);
    }
    circle(fx, D + 0x1230, 0x10, &[10.0]);
    fx.mem.write_u8(D + 0x1440, 0);
    for s3 in 0..0x40i32 {
        let ras = [[0x01E0_0308, 0x01E0_033C], [0x01E0_0370, 0x01E0_03A4], [0x01E0_03D8, 0x01E0_040C]];
        for (c, ra) in ras.into_iter().enumerate() {
            let row = table_row(fx, ra[0]);
            let r = fx.rand_from(D, ra[1]) >> 23;
            let f = fx.mem.f(row.wrapping_add((rem2(r, 32) * 12) as u32) + 4 * c as u32);
            fx.mem.setf(sp + 0x230 + 4 * c as u32, f);
        }
        fx.mem.write_u32(sp + 0x23C, 0x3F80_0000);
        fx.point_along(sp + 0x1F0, sp + 0x230);
        fx.rotate_axis(sp + 0x1F0, (rem2(s3, 0x10) << 16) >> 4, 2);
        let f1 = ee::div(ee::mul(0.5, ee::from_i32(s3)), 64.0);
        let f20 = ee::mul(80.0, ee::add(0.5, f1));
        for s4 in 0..4i32 {
            let z = ee::div(ee::mul(f20, ee::from_i32(s4)), 3.0);
            vec4(fx, sp + 0x190, [0.0, -5.0, z, 1.0]);
            vec4(fx, sp + 0x1A0, [0.0, 5.0, z, 1.0]);
            let p = D + 0x80 * s3 as u32 + 0x20 * s4 as u32;
            fx.vu_apply_to(p + 0x1450, sp + 0x1F0, sp + 0x190);
            fx.vu_apply_to(p + 0x1460, sp + 0x1F0, sp + 0x1A0);
        }
    }
    fx.mem.write_u8(D + 0x3450, 0);
    fx.mem.write_u8(D + 0x3458, 0);
    circle(fx, D + 0x3460, 0x10, &[50.0]);
    fx.mem.write_u8(D + 0x3670, 0);
    0
}

/// The sprite colour / the quad colours (c, a, Q 1) at `p`.
fn colour(fx: &mut Fx, p: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(p, c[0]);
    fx.mem.write_u8(p + 1, c[1]);
    fx.mem.write_u8(p + 2, c[2]);
    fx.mem.write_u8(p + 3, a);
    fx.mem.write_u32(p + 4, 0x3F80_0000);
}

/// The quad descriptor (re)set.
fn quad_desc(fx: &mut Fx, sp: u32, rh_at: u32, clamp: u64) {
    let d = sp + QUAD;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + rh_at);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, clamp);
}

/// The quad STs (0, 0), (1, 0), (0, 1), (1, 1).
fn quad_st(fx: &mut Fx, sp: u32) {
    for (o, v) in [(0x2B8, 0), (0x2BC, 0), (0x2D0, 0x3F80_0000), (0x2D4, 0), (0x2E8, 0), (0x2EC, 0x3F80_0000), (0x300, 0x3F80_0000), (0x304, 0x3F80_0000)] {
        fx.mem.write_u32(sp + o, v);
    }
}

fn quad_submit(fx: &mut Fx, sp: u32, n: u32, vs: &[u32], ras: [u32; 2]) {
    let d = sp + QUAD;
    let ok = if vs.len() == 1 {
        let m = fx.read_m4(sp + 0xD0);
        fx.project_desc(vs[0], PROJ_OUT, Some(&m), d, sp + 0x37C)
    } else {
        fx.project_n(vs, sp + 0xD0, d, sp + 0x37C, ras[0]) == 1
    };
    let _ = n;
    if ok {
        let w = fx.mem.f(sp + 0x37C);
        let key = fix(fx, w, ras[1]);
        fx.submit(d, OT, key as i32);
    }
}

/// The fan descriptor (prim 0x13: RGBAQ / XYZF2 x 3, ALPHA 1).
fn fan_desc(fx: &mut Fx, sp: u32) {
    let d = sp + FAN;
    fx.mem.write_u16(d + 2, 0x13);
    fx.mem.write_u16(d, 6);
    fx.mem.write_u64(d + 8, 0x41_4141);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
}

/// The fan colours: the centre (c, a), the rim (c, 0).
fn fan_colours(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    colour(fx, sp + 0x340, c, a);
    colour(fx, sp + 0x350, c, 0);
    words(fx, sp + 0x360, sp + 0x350, 2);
}

/// The fan of 32 triangles from the origin (sp + 0x230) to the circle at
/// `p`, at the far Z, key 0.
fn fan(fx: &mut Fx, sp: u32, p: u32, ra: u32) {
    vec4(fx, sp + 0x230, [0.0, 0.0, 0.0, 1.0]);
    for j in 0..0x20u32 {
        if fx.project_n(&[sp + 0x230, p + 0x10 * j, p + 0x10 * (j + 1)], sp + 0xD0, sp + FAN, sp + 0x37C, ra) == 1 {
            let v = fx.mem.read_u64(sp + 0x348);
            fx.mem.write_u64(sp + 0x348, (v & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFF8u64 << 32));
            let z = (fx.mem.read_u64(sp + 0x348) >> 32) & 0xFF_FFFF;
            for a in [sp + 0x358, sp + 0x368] {
                let v = fx.mem.read_u64(a);
                fx.mem.write_u64(a, (v & 0xFF00_0000_FFFF_FFFF) | (z << 32));
            }
            fx.submit(sp + FAN, OT, 0);
        }
    }
}

/// The bursts [F 0x1E00798].
fn bursts(fx: &mut Fx, sp: u32, rg: &mut Regs) {
    let d = sp + SPRITE;
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
    for (a, v) in [(sp + 0x260, 0u32), (sp + 0x262, 0), (sp + 0x270, 0x800), (sp + 0x272, 0x800)] {
        let h = fx.mem.read_u16(a);
        fx.mem.write_u16(a, (h & 0xC000) | v as u16);
    }
    fx.camera_vp_to(sp + 0xD0);
    fx.identity(sp + 0x110);
    crate::blib::exe::mul_mem(fx, sp + 0xD0, sp + 0x110);
    let s = f32::from_bits(0x3F19_999A);
    vec4(fx, sp + 0x190, [ee::neg(s), ee::neg(s), 0.0, 1.0]);
    vec4(fx, sp + 0x1A0, [s, s, 0.0, 1.0]);
    fx.vu_apply_to(sp + 0x190, sp + 0x150, sp + 0x190);
    fx.vu_apply_to(sp + 0x1A0, sp + 0x150, sp + 0x1A0);
    for i in 0..0x10u32 {
        let r = D + 0x70 + 0x90 * i;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        colour(fx, sp + 0x258, [0xFF, 0x60, 0x56], ((age * 0xFF) >> 4) as u8);
        for k in 0..0x10i32 {
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let mut t = ee::add(age, ee::mul(f32::from_bits(0x3D8F_5C29), ee::from_i32(k)));
            if !crate::exe::ee_le(t, 16.0) {
                t = 16.0;
            }
            let f20 = ee::div(ee::sub(16.0, t), 16.0);
            rg.f20 = f20;
            for j in 0..8u32 {
                let p = sp + 0x230;
                fx.vu_sub4(p, r + 0x10 + 0x10 * j, D + 0x50);
                fx.vu_scale4(p, p, f20);
                fx.vu_add4(p, p, D + 0x50);
                fx.vu_add4(sp + 0x1D0, sp + 0x190, p);
                fx.vu_add4(sp + 0x1E0, sp + 0x1A0, p);
                if crate::fxlib2::project(fx, sp + 0x1D0, PROJ_OUT, sp + 0xD0, d, sp + 0x37C) {
                    let w = fx.mem.f(sp + 0x37C);
                    let key = fix(fx, w, 0x01E0_0A08);
                    fx.submit(d, OT, key as i32);
                }
            }
        }
        rg.s2 = 0x10;
        rg.s3 = u64::from(r - 0x70 + 0x80);
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The spinning squares [F 0x1E00BC0].
fn squares(fx: &mut Fx, sp: u32, rg: &mut Regs) {
    let (n, p) = (-20.0f32, 20.0f32);
    vec4(fx, sp + 0x190, [n, 0.0, n, 1.0]);
    vec4(fx, sp + 0x1A0, [p, 0.0, n, 1.0]);
    vec4(fx, sp + 0x1B0, [n, 0.0, p, 1.0]);
    vec4(fx, sp + 0x1C0, [p, 0.0, p, 1.0]);
    for i in 0..0x20u32 {
        let b = D + 0x980 + 0x20 * i;
        if fx.mem.read_u8(b) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(b + 4));
        let s = ee::add(f32::from_bits(0x3ECC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F19_999A), age), 32.0));
        vec4(fx, sp + 0x230, [s, 1.0, s, 1.0]);
        fx.camera_vp_to(sp + 0xD0);
        let f = [rg.f20, 0.0, 0.0, 0.0, 0.0];
        fx.trs_into_saving(sp + 0x110, 0, 0, b + 0x10, f, [u64::from(D), u64::from(OT), u64::from(b - 0x980), u64::from(i)], 0x01E0_0CA4);
        let a = fx.mem.i32(b + 0xC);
        fx.rotate_axis(sp + 0x110, a, 2);
        let a = fx.mem.i32(b + 8);
        fx.rotate_axis(sp + 0x110, a, 1);
        fx.scale_rows(sp + 0x110, sp + 0x230);
        crate::blib::exe::mul_mem(fx, sp + 0xD0, sp + 0x110);
        let age = fx.mem.i32(b + 4);
        let tf = fx.tan_falloff_p(age, 0x20, 1);
        let f2 = ee::mul(200.0, ee::sub(1.0, tf));
        let c = ee::from_i32(fx.mem.i32(D + 0x970));
        let a = fix(fx, ee::div(ee::mul(c, f2), 64.0), 0x01E0_0D2C);
        colour(fx, sp + 0x2B0, [0xFF, 0x74, 0x64], a as u8);
        for o in [0x2C8, 0x2E0, 0x2F8] {
            words(fx, sp + o, sp + 0x2B0, 2);
        }
        quad_st(fx, sp);
        quad_submit(fx, sp, 0, &[sp + 0x190], [0, 0x01E0_0DE0]);
        let age = fx.mem.i32(b + 4) + 1;
        fx.mem.set32(b + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(b, 0);
        }
    }
    rg.s2 = u64::from(D + 0x400);
    rg.s3 = 0x20;
}

/// The shrinking rings [F 0x1E00E98].
fn rings(fx: &mut Fx, sp: u32, rg: &mut Regs) {
    quad_desc(fx, sp, 0x18, 0x0000_01FC_012B_C8FA);
    for i in 0..0x10u32 {
        let c = D + 0x11B0 + 8 * i;
        if fx.mem.read_u8(c) == 0 {
            continue;
        }
        let age = fx.mem.i32(c + 4);
        let s = ee::div(ee::from_i32(0x10 - age), 16.0);
        vec4(fx, sp + 0x230, [s, s, 1.0, 1.0]);
        fx.camera_vp_to(sp + 0xD0);
        fx.translate_rows(sp + 0x110, sp + 0x150, D + 0x50);
        fx.scale_rows(sp + 0x110, sp + 0x230);
        crate::blib::exe::mul_mem(fx, sp + 0xD0, sp + 0x110);
        let age = fx.mem.i32(c + 4);
        colour(fx, sp + 0x2B0, [0xFF, 0x68, 0x60], ((age * 0xFF) >> 4) as u8);
        words(fx, sp + 0x2C8, sp + 0x2B0, 2);
        colour(fx, sp + 0x2E0, [0xFF, 0x68, 0x60], 0);
        words(fx, sp + 0x2F8, sp + 0x2E0, 2);
        for j in 0..0x20u32 {
            for (o, v) in [(0x2B8, 0x3F0E_0000), (0x2BC, 0), (0x2D0, 0x3F30_0000), (0x2D4, 0), (0x2E8, 0x3F0E_0000), (0x2EC, 0x3F00_0000), (0x300, 0x3F30_0000), (0x304, 0x3F00_0000)] {
                fx.mem.write_u32(sp + o, v);
            }
            let (e0, e1) = (D + 0xD90 + 0x20 * j, D + 0xD90 + 0x20 * (j + 1));
            quad_submit(fx, sp, j, &[e0, e1, e0 + 0x10, e1 + 0x10], [0x01E0_1060, 0x01E0_1074]);
        }
        rg.s2 = u64::from(D + 0x400);
        let age = fx.mem.i32(c + 4) + 1;
        fx.mem.set32(c + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(c, 0);
        }
    }
    rg.s3 = u64::from(D + 0x80);
}

/// The node's pulsing fan [F 0x1E010F0].
fn node_fan(fx: &mut Fx, sp: u32, rg: &mut Regs) {
    fan_desc(fx, sp);
    fx.camera_vp_to(sp + 0xD0);
    fx.translate_rows(sp + 0x110, sp + 0x150, D + 0x50);
    let age = fx.mem.i32(D + 0x1444);
    let s = ee::sub(1.0, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(rem2(age, 2))));
    vec4(fx, sp + 0x230, [s, s, s, 1.0]);
    fx.scale_rows(sp + 0x110, sp + 0x230);
    crate::blib::exe::mul_mem(fx, sp + 0xD0, sp + 0x110);
    let age = ee::from_i32(fx.mem.i32(D + 0x1444));
    let f20 = ee::mul(128.0, Fx::ratio_below(age, 32.0));
    let age = ee::from_i32(fx.mem.i32(D + 0x1444));
    let f0 = Fx::ramp_above(age, 106.0, 74.0);
    let a = fix(fx, ee::mul(f20, f0), 0x01E0_11F4);
    rg.f20 = f20;
    fan_colours(fx, sp, [0xFF, 0x40, 0x40], a as u8);
    fan(fx, sp, D + 0x1230, 0x01E0_1298);
    rg.s2 = u64::from(D + 0x200);
    rg.s3 = u64::from(sp + 0x348);
    let age = fx.mem.i32(D + 0x1444) + 1;
    fx.mem.set32(D + 0x1444, age);
    if age >= 0x6B {
        fx.mem.write_u8(D + 0x1440, 0);
    }
}

/// x / 3 as compiled (the multiply by 0x55555556).
fn div3(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x5555_5556, x);
    let hi = fx.hilo().1;
    hi.wrapping_add(((x as u32) >> 31) as i32)
}

/// The 64 fans of quads at the target [F 0x1E0139C].
fn target_fans(fx: &mut Fx, sp: u32, rg: &mut Regs) {
    quad_desc(fx, sp, 0x1C, 5);
    let age = ee::from_i32(fx.mem.i32(D + 0x3454));
    let s = ee::add(f32::from_bits(0x3F19_999A), ee::div(ee::mul(f32::from_bits(0x3ECC_CCCD), age), 48.0));
    vec4(fx, sp + 0x230, [s, s, s, 1.0]);
    fx.camera_vp_to(sp + 0xD0);
    let f = [rg.f20, 0.0, 0.0, 0.0, 0.0];
    fx.trs_into_saving(sp + 0x110, sp + 0x230, 0, D + 0x30, f, [u64::from(D), u64::from(OT), rg.s2, rg.s3], 0x01E0_1468);
    crate::blib::exe::mul_mem(fx, sp + 0xD0, sp + 0x110);
    let age = fx.mem.i32(D + 0x3454);
    let tf = fx.tan_falloff_p(age, 0x30, 1);
    let f20 = ee::sub(1.0, tf);
    rg.f20 = f20;
    for s4 in 0..0x40u32 {
        let s3 = D + 0x80 * s4;
        for s5 in 0..3i32 {
            let p = s3 + 0x20 * s5 as u32;
            let q = s3 + 0x20 * (s5 + 1) as u32;
            let v = div3(fx, (3 - s5) * 0xFF);
            let a = fix(fx, ee::mul(f20, ee::from_i32(v)), 0x01E0_14E4);
            colour(fx, sp + 0x2B0, [0xFF, 0x64, 0x5A], a as u8);
            words(fx, sp + 0x2C8, sp + 0x2B0, 2);
            let v = div3(fx, (3 - (s5 + 1)) * 0xFF);
            let a = fix(fx, ee::mul(f20, ee::from_i32(v)), 0x01E0_1560);
            colour(fx, sp + 0x2E0, [0xFF, 0x64, 0x5A], a as u8);
            words(fx, sp + 0x2F8, sp + 0x2E0, 2);
            quad_st(fx, sp);
            quad_submit(fx, sp, 0, &[p + 0x1450, p + 0x1460, q + 0x1450, q + 0x1460], [0x01E0_15E8, 0x01E0_15FC]);
        }
    }
    rg.s2 = u64::from(D + 0x2060);
    rg.s3 = u64::from(D + 0x2000);
    let age = fx.mem.i32(D + 0x3454) + 1;
    fx.mem.set32(D + 0x3454, age);
    if age >= 0x31 {
        fx.mem.write_u8(D + 0x3450, 0);
    }
}

/// The two crossed quad sets at the target [F 0x1E0167C].
fn crossed(fx: &mut Fx, sp: u32, rg: &mut Regs) {
    quad_desc(fx, sp, 0x20, 5);
    let age = ee::from_i32(fx.mem.i32(D + 0x345C));
    let f2 = ee::div(ee::mul(f32::from_bits(0x3F19_999A), age), 48.0);
    let s = ee::mul(50.0, ee::add(f32::from_bits(0x3ECC_CCCD), f2));
    vec4(fx, sp + 0x230, [s, s, 1.0, 1.0]);
    let age = fx.mem.i32(D + 0x345C);
    let tf = fx.tan_falloff_p(age, 0x30, 1);
    let a = fix(fx, ee::mul(128.0, ee::sub(1.0, tf)), 0x01E0_1768);
    colour(fx, sp + 0x2B0, [0xFF, 0x80, 0x80], a as u8);
    for o in [0x2C8, 0x2E0, 0x2F8] {
        words(fx, sp + o, sp + 0x2B0, 2);
    }
    let mut s3 = -1i32;
    for _ in 0..2 {
        fx.camera_vp_to(sp + 0xD0);
        let f = [rg.f20, 0.0, 0.0, 0.0, 0.0];
        fx.trs_into_saving(sp + 0x110, 0, 0, D + 0x30, f, [u64::from(D), u64::from(OT), rg.s2, s3 as i64 as u64], 0x01E0_17EC);
        let a1 = (s3 * 15) << 17;
        fx.mult(0xB60B_60B7_u32 as i32, a1);
        let hi = fx.hilo().1;
        let ang = (hi.wrapping_add(a1) >> 8).wrapping_add(((a1 as u32) >> 31) as i32);
        fx.rotate_axis(sp + 0x110, ang, 2);
        fx.rotate_axis(sp + 0x110, ang + 0x4000, 0);
        fx.scale_rows(sp + 0x110, sp + 0x230);
        crate::blib::exe::mul_mem(fx, sp + 0xD0, sp + 0x110);
        for k in 0..4u32 {
            quad_st(fx, sp);
            quad_submit(fx, sp, k, &[QUAD_TABLE + 0x40 * k], [0, 0x01E0_18B4]);
        }
        rg.s2 = u64::from(QUAD_TABLE + 0x100);
        s3 += 2;
    }
    let age = fx.mem.i32(D + 0x345C) + 1;
    fx.mem.set32(D + 0x345C, age);
    if age >= 0x31 {
        fx.mem.write_u8(D + 0x3458, 0);
    }
}

/// The target's fan [F 0x1E01934].
fn target_fan(fx: &mut Fx, sp: u32) {
    fan_desc(fx, sp);
    fx.camera_vp_to(sp + 0xD0);
    fx.translate_rows(sp + 0x110, sp + 0x150, D + 0x30);
    crate::blib::exe::mul_mem(fx, sp + 0xD0, sp + 0x110);
    let age = fx.mem.i32(D + 0x3674);
    let tf = fx.tan_falloff_p(age, 0x30, 1);
    let a = fix(fx, ee::mul(255.0, ee::sub(1.0, tf)), 0x01E0_19BC);
    fan_colours(fx, sp, [0xFF, 0x60, 0x50], a as u8);
    fan(fx, sp, D + 0x3460, 0x01E0_1A58);
    let age = fx.mem.i32(D + 0x3674) + 1;
    fx.mem.set32(D + 0x3674, age);
    if age >= 0x31 {
        fx.mem.write_u8(D + 0x3670, 0);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let mut rg = Regs::default();
    fx.camera_turn_roll(sp + 0x150);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x40 + 4 * k));
        fx.node_position(h, slot, D + 0x50 + 0x10 * k);
    }
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x5A {
        let r = D + 0x70 + 0x90 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for k in 0..8u32 {
            let e = r + 0x10 + 0x10 * k;
            let x = fx.rand_from(D, 0x01E0_0640) >> 23;
            let col = (rem2(rem2(x, 0x100), 0x20) * 12) as u32;
            for (c, ra) in [0x01E0_0678u32, 0x01E0_06D4, 0x01E0_0724].into_iter().enumerate() {
                let row = table_row(fx, ra);
                let f = fx.mem.f(row.wrapping_add(col) + 4 * c as u32);
                fx.mem.setf(e + 4 * c as u32, ee::mul(2.0, f));
            }
            fx.mem.write_u32(e + 0xC, 0x3F80_0000);
            fx.vu_add4(e, e, D + 0x50);
        }
    }
    bursts(fx, sp, &mut rg);
    let n = fx.mem.i32(D);
    if (0x45..0xC8).contains(&n) {
        let b = D + 0x980 + 0x20 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(b, 1);
        fx.mem.write_u32(b + 4, 0);
        let r = fx.rand_from(D, 0x01E0_0AB0) >> 23;
        fx.mem.set32(b + 8, (rem2(r, 0x100) << 16) >> 8);
        let r = fx.rand_from(D, 0x01E0_0ADC) >> 23;
        let m = fx.mult(rem2(r, 0x100), 0xB60);
        fx.mem.set32(b + 0xC, (m >> 8) - 0x5B0);
        words(fx, b + 0x10, D + 0x50, 4);
        let c = fx.mem.i32(D + 0x970) + 1;
        fx.mem.set32(D + 0x970, c);
        if c >= 0x41 {
            fx.mem.write_u32(D + 0x970, 0x40);
        }
    }
    quad_desc(fx, sp, 0x14, 5);
    squares(fx, sp, &mut rg);
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x5A {
        let (_, m) = fx.div(n, 10);
        if m == 0 {
            let c = fx.mem.read_u32(D + 0xD80);
            fx.mem.write_u8(D + 0x11B0 + 8 * c, 1);
            let c = fx.mem.read_u32(D + 0xD80);
            fx.mem.write_u32(D + 0x11B4 + 8 * c, 0);
            let c = fx.mem.i32(D + 0xD80) + 1;
            fx.mem.set32(D + 0xD80, c);
            if c >= 0x10 {
                fx.mem.write_u32(D + 0xD80, 0);
            }
        }
    }
    rings(fx, sp, &mut rg);
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + 0x1440, 1);
        fx.mem.write_u32(D + 0x1444, 0);
    }
    if fx.mem.read_u8(D + 0x1440) == 1 {
        node_fan(fx, sp, &mut rg);
    }
    if fx.mem.i32(D) == 0xBB {
        fx.mem.write_u8(D + 0x3450, 1);
        fx.mem.write_u32(D + 0x3454, 0);
    }
    if fx.mem.read_u8(D + 0x3450) == 1 {
        target_fans(fx, sp, &mut rg);
    }
    if fx.mem.i32(D) == 0xBB {
        fx.mem.write_u8(D + 0x3458, 1);
        fx.mem.write_u32(D + 0x345C, 0);
    }
    if fx.mem.read_u8(D + 0x3458) == 1 {
        crossed(fx, sp, &mut rg);
    }
    if fx.mem.i32(D) == 0xBB {
        fx.mem.write_u8(D + 0x3670, 1);
        fx.mem.write_u32(D + 0x3674, 0);
    }
    if fx.mem.read_u8(D + 0x3670) == 1 {
        target_fan(fx, sp);
    }
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0xB1 {
        ret = 1;
    } else if n == 0xBB {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xEC {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}
