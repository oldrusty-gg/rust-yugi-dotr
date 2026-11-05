//! BMODULE b014 (card 151), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0x9360
//! bytes) [F]: a single unscheduled function in
//! a 0x5D0 frame (D kept at sp + 0xBC, s3 the OT, the return at sp +
//! 0xC0), the b030 / b290 family. Every update the camera turn with roll
//! (sp + 0x370), nodes 0x25 / 0x26 (D+0x50 / 0x60) and eight points along
//! them (D+0x70 + 0x10 k, k / 7 of the way); the target's screen point
//! D+0x30 (0x204D80, x / y clamped to 0..512, its Z into +8). On updates
//! 1..0x9F a spark burst (16 round robin, D+0xF0 + 0x2B0 i: four sprites
//! scattered 0.6 from each point, drifting up 15, coloured by a cosine,
//! 16 updates); on 0x76 a 50-wide flash at the middle point (D+0x2BF0, 20
//! updates, far Z); 0x77..0x89 strips (8 round robin, D+0x2C00 + 0xE0 i:
//! twelve 4-wide strips thrown 2.5 from the middle point, 8 updates); on
//! 0x87..0x92 rings (20 round robin, D+0x3310 + 0x4D0 i: two layers of 32
//! sprites each from the points, 32 updates); from 0x92 for 32 updates a
//! lens (D+0x9350): the frame copied as a texture (0x207F50) into eight
//! rings of an eight-triangle fan (0x28ABE0) about the target's screen
//! point, in its own packet layer. The stage light 0x80 - 96 clamp(0xE8
//! - n) clamp(n) / 1024. Returns 1 on 0x88, 6 on 0x93..0xB1, 3 on 0xB2, 8
//! once the count reaches 0xE9.
use crate::blib::exe::{mul_mem, translation_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70 / 0x205210's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x5D0;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const DESC: u32 = 0x460;
/// The strip descriptor (prim 0x34, 12 registers).
const STRIP: u32 = 0x530;
/// The fan's triangle descriptor (prim 0x33 | 0x800, 9 registers).
const TRI: u32 = 0x4B8;
const FAN: u32 = 0x0028_ABE0;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR: u64 = 0xFF_FFF8;
const LENS_CLAMP: u64 = 0x800_0080_000A;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xBC, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
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

/// 0x1A2C70 (unsigned) / 0x1A2B80 (signed) with their residue frames.
fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}
fn fixs(fx: &mut Fx, f: f32, ra: u32) -> i32 {
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}
fn fix_at(fx: &mut Fx, a: u32, ra: u32) -> i32 {
    let f = fx.mem.f(a);
    fixs(fx, f, ra)
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The sprite's UVs (sp + 0x478 / 0x47A, 0x488 / 0x48A).
fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x478, u0);
    uv(fx, sp + 0x47A, v0);
    uv(fx, sp + 0x488, u1);
    uv(fx, sp + 0x48A, v1);
}

/// The 32 x 32 atlas cell of byte `b` (4 across).
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    uvs(fx, sp, ((c << 9) & 0x3FFF) as u32, ((r << 9) & 0x3FFF) as u32, (((32 * c + 32) << 4) & 0x3FFF) as u32, (((32 * r + 32) << 4) & 0x3FFF) as u32);
}

fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

/// The sprite descriptor head (prim 0x26 | 0x800, 5 registers).
fn head(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// Texture `tex` (filter 1), ALPHA preset 1, REGION_CLAMP 5.
fn textured(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + DESC;
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
}

/// The camera's view-projection (x identity) at sp + 0x2F0.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x2F0);
    fx.identity(sp + 0x330);
    mul_mem(fx, sp + 0x2F0, sp + 0x330);
}

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1) at sp + 0x3B0 / 0x3C0 turned
/// by the camera.
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    vec4(fx, sp + 0x3B0, [n, n, 0.0, 1.0]);
    vec4(fx, sp + 0x3C0, [s, s, 0.0, 1.0]);
    fx.vu_apply_to(sp + 0x3B0, sp + 0x370, sp + 0x3B0);
    fx.vu_apply_to(sp + 0x3C0, sp + 0x370, sp + 0x3C0);
}

/// The sprite at sp + 0x450 (+ the corners) projected (0x204F70); true
/// when it projects.
fn sprite_at(fx: &mut Fx, sp: u32) -> bool {
    fx.vu_add4(sp + 0x3D0, sp + 0x3B0, sp + 0x450);
    fx.vu_add4(sp + 0x3E0, sp + 0x3C0, sp + 0x450);
    crate::fxlib2::project(fx, sp + 0x3D0, PROJ_OUT, sp + 0x2F0, sp + DESC, sp + 0x5CC)
}

fn sprite_draw(fx: &mut Fx, sp: u32, ra: u32) {
    if sprite_at(fx, sp) {
        let w = fx.mem.f(sp + 0x5CC);
        let key = fixu(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

fn far_z(fx: &mut Fx, first: u32, others: &[u32]) {
    let v = fx.mem.read_u64(first);
    fx.mem.write_u64(first, (v & KEEP) | (FAR << 32));
    let z = ((fx.mem.read_u64(first) << 8) >> 40) << 32;
    for &o in others {
        let w = fx.mem.read_u64(o);
        fx.mem.write_u64(o, (w & KEEP) | z);
    }
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x9360);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
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
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    for (k, (node, clear)) in [(0x25, 1), (0x26, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0x40 + 4 * k as u32, r);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x370);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x40 + 4 * k));
        fx.node_position(h, slot, D + 0x50 + 0x10 * k);
    }
    fx.vu_sub4(sp + 0x450, D + 0x60, D + 0x50);
    for k in 0..8u32 {
        let p = D + 0x70 + 0x10 * k;
        let f = ee::div(ee::from_i32(k as i32), 7.0);
        fx.vu_scale4(p, sp + 0x450, f);
        fx.vu_add4(p, p, D + 0x50);
    }
    camera(fx, sp);
    // The target's screen point.
    let m = fx.read_m4(sp + 0x2F0);
    fx.project_at(D + 0x20, &m, sp + 0x5C0, sp + 0x5CC);
    let x = (i32::from(fx.mem.read_u16(sp + 0x5C0)) >> 4) - 0x700;
    fx.mem.setf(D + 0x30, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x5C2)) >> 4) - 0x700;
    fx.mem.setf(D + 0x34, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x5C0) << 8) >> 40;
    fx.mem.setf(D + 0x38, ee::from_i32(z as i32));
    fx.mem.write_u32(D + 0x3C, 0x3F80_0000);
    for k in [0u32, 4] {
        if crate::exe::ee_lt(fx.mem.f(D + 0x30 + k), 0.0) {
            fx.mem.write_u32(D + 0x30 + k, 0);
        }
    }
    for k in [0u32, 4] {
        if !crate::exe::ee_le(fx.mem.f(D + 0x30 + k), 512.0) {
            fx.mem.write_u32(D + 0x30 + k, 0x4400_0000);
        }
    }
    let n = fx.mem.i32(D);
    if n > 0 && n < 0xA0 {
        let g = D + 0x2B0 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(g + 0xF0, 1);
        fx.mem.write_u32(g + 0xF4, 0);
        for k in 0..8u32 {
            words(fx, g + 0x120 + 0x10 * k, D + 0x70 + 0x10 * k, 4);
            for j in 0..4u32 {
                let r = fx.rand_from(j, 0x01E0_0480) >> 23;
                fx.mem.set8(g + 0xF8 + 4 * k + j, rem2(r, 8));
                fx.random_table_vec(g + 0x1A0 + 0x40 * k + 0x10 * j, f32::from_bits(0x3F19_999A));
            }
        }
    }
    sparks(fx, sp);
    if fx.mem.i32(D) == 0x76 {
        fx.mem.write_u8(D + 0x2BF0, 1);
        fx.mem.write_u32(D + 0x2BF4, 0);
    }
    if fx.mem.read_u8(D + 0x2BF0) == 1 {
        flash(fx, sp);
    }
    let n = fx.mem.i32(D);
    if (0x77..0x8A).contains(&n) {
        let r = D + 0xE0 * rem2(n, 8) as u32;
        fx.mem.write_u8(r + 0x2C00, 1);
        fx.mem.write_u32(r + 0x2C04, 0);
        for k in 0..0xCu32 {
            let b = fx.rand_from(sp + 0x488, 0x01E0_0CEC) >> 23;
            fx.mem.set8(r + 0x2C08 + k, rem2(b, 8));
            fx.random_table_vec(r + 0x2C20 + 0x10 * k, 2.5);
        }
    }
    strips(fx, sp);
    let n = fx.mem.i32(D);
    if (0x87..0x93).contains(&n) {
        let c = fx.mem.read_u32(D + 0x3300);
        let g = D + 0x4D0 * c;
        fx.mem.write_u8(g + 0x3310, 1);
        fx.mem.write_u32(g + 0x3314, 0);
        for k in 0..8u32 {
            words(fx, g + 0x3340 + 0x10 * k, D + 0x70 + 0x10 * k, 4);
            for j in 0..4u32 {
                let b = fx.rand_from(sp + 0x488, 0x01E0_12D0) >> 23;
                let (_, h) = fx.div(b, 3);
                fx.mem.set8(g + 0x3318 + 4 * k + j, h + 5);
                fx.random_table_vec(g + 0x33C0 + 0x40 * k + 0x10 * j, f32::from_bits(0x3FA6_6666));
            }
            for j in 0..4u32 {
                let b = fx.rand_from(sp + 0x488, 0x01E0_1328) >> 23;
                fx.mem.set8(g + 0x35C0 + 4 * k + j, rem2(b, 8));
                fx.random_table_vec(g + 0x35E0 + 0x40 * k + 0x10 * j, 1.0);
            }
        }
        let c = fx.mem.i32(D + 0x3300) + 1;
        fx.mem.set32(D + 0x3300, c);
        if c >= 0x14 {
            fx.mem.write_u32(D + 0x3300, 0);
        }
    }
    rings(fx, sp);
    if fx.mem.i32(D) == 0x92 {
        fx.mem.write_u8(D + 0x9350, 1);
        fx.mem.write_u32(D + 0x9354, 0);
    }
    if fx.mem.read_u8(D + 0x9350) == 1 {
        lens(fx, sp);
    }
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(0xE8 - n, 0, 0x20);
    let n = fx.mem.i32(D);
    let b = Fx::clamp_i(n, 0, 0x20);
    let m = fx.mult(a, (b - (b << 2)) << 5);
    fx.set_stage_light(((m >> 10) + 0x80) & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x88 {
        ret = 1;
    } else {
        if (0x93..0xB2).contains(&n) {
            ret = 6;
        }
        if n == 0xB2 {
            ret = 3;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xE9 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// The spark bursts [F 0x1E004E0].
fn sparks(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    textured(fx, sp, 0x10);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n, 0, 0x20));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(0xA0 - n, 0, 0x20));
    let f20 = ee::div(ee::mul(a, b), 1024.0);
    for i in 0..0x10u32 {
        let g = D + 0x2B0 * i;
        if fx.mem.read_u8(g + 0xF0) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0xF4);
        let c = fx.tcos((age << 14) >> 4);
        let al = fixu(fx, ee::mul(ee::mul(55.0, c), f20), 0x01E0_0624) & 0xFF;
        let gr = fixs(fx, ee::mul(75.0, c), 0x01E0_0638);
        let bl = fixs(fx, ee::mul(33.0, c), 0x01E0_064C);
        rgba(fx, sp + 0x470, [0xFF, gr as u8, bl as u8], al as u8);
        let age = fx.mem.i32(g + 0xF4);
        let h = ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(0x10 - age)), 16.0));
        corners(fx, sp, ee::mul(3.5, h));
        let age = ee::from_i32(fx.mem.i32(g + 0xF4));
        let f3 = ee::div(age, 16.0);
        let f2 = ee::div(ee::mul(f32::from_bits(0x3F66_6666), age), 16.0);
        let f22 = ee::mul(15.0, f3);
        let f21 = ee::add(f32::from_bits(0x3DCC_CCCD), f2);
        for k in 0..8u32 {
            let pt = g + 0x120 + 0x10 * k;
            for j in 0..4u32 {
                let b = i32::from(fx.mem.read_u8(g + 0xF8 + 4 * k + j));
                cell_uvs(fx, sp, b);
                let v = g + 0x1A0 + 0x40 * k + 0x10 * j;
                let x = ee::mul(fx.mem.f(v), f21);
                fx.mem.setf(sp + 0x450, x);
                let y = ee::add(f22, ee::mul(fx.mem.f(v + 4), f21));
                fx.mem.setf(sp + 0x454, y);
                let z = ee::mul(fx.mem.f(v + 8), f21);
                fx.mem.setf(sp + 0x458, z);
                fx.vu_add4(sp + 0x450, sp + 0x450, pt);
                sprite_draw(fx, sp, 0x01E0_0898);
            }
        }
        let age = fx.mem.i32(g + 0xF4) + 1;
        fx.mem.set32(g + 0xF4, age);
        if age >= 0x11 {
            fx.mem.write_u8(g + 0xF0, 0);
        }
    }
}

/// The flash at the middle point [F 0x1E00988].
fn flash(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    textured(fx, sp, 0x14);
    camera(fx, sp);
    let age = ee::from_i32(fx.mem.i32(D + 0x2BF4));
    let f20 = Fx::ratio_below(age, 4.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x2BF4));
    let f3 = ee::mul(f20, Fx::ramp_above(age, 20.0, 16.0));
    let v = ee::from_i32(rem2(fx.mem.i32(D + 0x2BF4), 2));
    let f1 = ee::mul(ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), v)), f3);
    let al = fixu(fx, ee::mul(164.0, f1), 0x01E0_0AB4);
    rgba(fx, sp + 0x470, [0xFF, 0xE6, 0xC8], al as u8);
    vec4(fx, sp + 0x3B0, [50.0, 50.0, 0.0, 1.0]);
    vec4(fx, sp + 0x3C0, [-50.0, -50.0, 0.0, 1.0]);
    fx.vu_apply_to(sp + 0x3B0, sp + 0x370, sp + 0x3B0);
    fx.vu_apply_to(sp + 0x3C0, sp + 0x370, sp + 0x3C0);
    uvs(fx, sp, 0, 0x200, 0x200, 0x400);
    words(fx, sp + 0x450, D + 0xB0, 4);
    if sprite_at(fx, sp) {
        far_z(fx, sp + 0x480, &[sp + 0x490]);
        fx.submit(sp + DESC, OT, 0);
    }
    let age = fx.mem.i32(D + 0x2BF4) + 1;
    fx.mem.set32(D + 0x2BF4, age);
    if age >= 0x15 {
        fx.mem.write_u8(D + 0x2BF0, 0);
    }
}

/// The strips [F 0x1E00D30].
fn strips(fx: &mut Fx, sp: u32) {
    let d = sp + STRIP;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n - 0x76, 0, 4));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(0x8A - n, 0, 4));
    let f20 = ee::div(ee::mul(a, b), 16.0);
    for k in 0..8u32 {
        let r = D + 0xE0 * k;
        if fx.mem.read_u8(r + 0x2C00) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0x2C04);
        let al = fixu(fx, ee::mul(f20, ee::from_i32((((8 - age) * 5) << 4) >> 3)), 0x01E0_0E40);
        let p = sp + 0x540;
        rgba(fx, p, [0xFF, 0xE6, 0xC8], al as u8);
        for o in [0x558u32, 0x570, 0x588] {
            words(fx, sp + o, p, 2);
        }
        fx.camera_vp_to(sp + 0x2F0);
        words(fx, sp + 0x450, D + 0xB0, 4);
        translation_into(fx, sp + 0x330, sp + 0x450);
        mul_mem(fx, sp + 0x2F0, sp + 0x330);
        let age = fx.mem.i32(r + 0x2C04);
        let tf = fx.tan_falloff_p(age, 8, 1);
        let f21 = ee::mul(-1.5, tf);
        for j in 0..0xCu32 {
            let v = r + 0x2C20 + 0x10 * j;
            fx.point_along(sp + 0x410, v);
            let s5 = (0x10000 * j) as i32;
            fx.mult(0x2AAA_AAAB, s5);
            let a = (0x2000 * k as i32).wrapping_add(s5 / 12);
            fx.rotate_axis(sp + 0x410, a, 2);
            vec4(fx, sp + 0x3B0, [0.0, 2.0, 0.0, 1.0]);
            vec4(fx, sp + 0x3C0, [0.0, -2.0, 0.0, 1.0]);
            fx.vu_apply_to(sp + 0x3B0, sp + 0x410, sp + 0x3B0);
            fx.vu_apply_to(sp + 0x3C0, sp + 0x410, sp + 0x3C0);
            fx.vu_add4(sp + 0x3D0, sp + 0x3B0, v);
            fx.vu_add4(sp + 0x3E0, sp + 0x3C0, v);
            fx.vu_scale4(sp + 0x450, v, f21);
            fx.vu_add4(sp + 0x3F0, sp + 0x3D0, sp + 0x450);
            fx.vu_add4(sp + 0x400, sp + 0x3E0, sp + 0x450);
            let b = i32::from(fx.mem.read_u8(r + 0x2C08 + j));
            let f5 = ee::from_i32(rem2(b, 4) << 5);
            let f2 = ee::from_i32((b >> 2) << 5);
            let s0 = ee::div(f5, 128.0);
            let s1 = ee::div(ee::add(32.0, f5), 128.0);
            let t0 = ee::div(f2, 64.0);
            let t1 = ee::div(ee::add(32.0, f2), 64.0);
            for (o, x) in [(0x548u32, s0), (0x54C, t0), (0x560, s1), (0x564, t0), (0x578, s0), (0x57C, t1), (0x590, s1), (0x594, t1)] {
                fx.mem.setf(sp + o, x);
            }
            let m = fx.read_m4(sp + 0x2F0);
            if fx.project_desc(sp + 0x3D0, PROJ_OUT, Some(&m), d, sp + 0x5CC) {
                far_z(fx, sp + 0x550, &[sp + 0x568, sp + 0x580, sp + 0x598]);
                fx.submit(d, OT, 0);
            }
        }
        let age = fx.mem.i32(r + 0x2C04) + 1;
        fx.mem.set32(r + 0x2C04, age);
        if age >= 9 {
            fx.mem.write_u8(r + 0x2C00, 0);
        }
    }
}

/// The rings [F 0x1E013E0].
fn rings(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    camera(fx, sp);
    for i in 0..0x14u32 {
        let g = D + 0x4D0 * i;
        if fx.mem.read_u8(g + 0x3310) == 0 {
            continue;
        }
        for (tex, amp, col, size, ra, key_ra, bytes, dirs) in [
            (0x18u32, 80.0f32, [0xC8u8, 0x80, 0x3C], f32::from_bits(0x4108_0000), 0x01E0_14B4u32, 0x01E0_1728u32, 0x3318u32, 0x33C0u32),
            (0x10, 30.0, [0xE6, 0x42, 0x19], f32::from_bits(0x4168_0000), 0x01E0_17E4, 0x01E0_1A58, 0x35C0, 0x35E0),
        ] {
            textured(fx, sp, tex);
            let age = fx.mem.i32(g + 0x3314);
            let c = fx.tcos((age << 14) >> 5);
            let al = fixu(fx, ee::mul(amp, c), ra);
            rgba(fx, sp + 0x470, col, al as u8);
            let age = ee::from_i32(fx.mem.i32(g + 0x3314));
            let h = ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0));
            corners(fx, sp, ee::mul(size, h));
            let age = fx.mem.i32(g + 0x3314);
            let tf = fx.tan_falloff_p(age, 0x20, 1);
            let f20 = ee::add(f32::from_bits(0x3E99_999A), ee::mul(f32::from_bits(0x3F33_3333), tf));
            let age = ee::from_i32(fx.mem.i32(g + 0x3314));
            let f21 = ee::div(ee::mul(10.0, age), 32.0);
            for k in 0..8u32 {
                let pt = g + 0x3340 + 0x10 * k;
                for j in 0..4u32 {
                    let b = i32::from(fx.mem.read_u8(g + bytes + 4 * k + j));
                    cell_uvs(fx, sp, b);
                    fx.vu_scale4(sp + 0x450, g + dirs + 0x40 * k + 0x10 * j, f20);
                    let y = ee::add(fx.mem.f(sp + 0x454), f21);
                    fx.mem.setf(sp + 0x454, y);
                    fx.vu_add4(sp + 0x450, sp + 0x450, pt);
                    sprite_draw(fx, sp, key_ra);
                }
            }
        }
        let age = fx.mem.i32(g + 0x3314) + 1;
        fx.mem.set32(g + 0x3314, age);
        if age >= 0x21 {
            fx.mem.write_u8(g + 0x3310, 0);
        }
    }
}

/// An XYZ2 at `p`: x / y halfwords of pixel (x, y), the far Z, a zero top
/// byte.
fn xyz_far(fx: &mut Fx, p: u32, x: i32, y: i32) {
    fx.mem.write_u16(p, ((x + 0x700) << 4) as u16);
    fx.mem.write_u16(p + 2, ((y + 0x700) << 4) as u16);
    let v = fx.mem.read_u64(p);
    fx.mem.write_u64(p, (v & KEEP) | (FAR << 32));
    fx.mem.write_u8(p + 7, 0);
}

/// The lens [F 0x1E01B60..0x1E0204C].
fn lens(fx: &mut Fx, sp: u32) {
    let d = sp + TRI;
    fx.mem.write_u16(d + 2, 0x33);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let p = fx.alpha_preset(0);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, LENS_CLAMP);
    // a0..a3 as 0x208400 leaves them.
    let a = [u32::from(fx.mem.read_u16(d + 2)), LENS_CLAMP as u32, d + 0x10, d + 2];
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xDC, layer);
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x518, sp + 0x520);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0x9354);
    let al = ((0x20 - age) << 6) >> 5;
    let p = sp + 0x4C8;
    rgba(fx, p, [0xFF, 0xFF, 0xFF], al as u8);
    for o in [0x4E0u32, 0x4F8] {
        words(fx, sp + o, p, 2);
    }
    let c = D + 0x30;
    for i in 0..8u32 {
        let f20 = ee::add(1.0, ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(8 - i as i32)), 8.0));
        for k in 0..8u32 {
            let s0 = FAN + 0x10 * k;
            let x = fix_at(fx, c, 0x01E0_1CE4);
            let y = fix_at(fx, c + 4, 0x01E0_1CF4);
            uv(fx, sp + 0x4D0, ((x << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x4D2, ((y << 4) & 0x3FFF) as u32);
            let x = fix_at(fx, s0, 0x01E0_1D38);
            let y = fix_at(fx, s0 + 4, 0x01E0_1D44);
            uv(fx, sp + 0x4E8, ((x << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x4EA, ((y << 4) & 0x3FFF) as u32);
            let x = fix_at(fx, s0 + 0x10, 0x01E0_1D88);
            let y = fix_at(fx, s0 + 0x14, 0x01E0_1D94);
            uv(fx, sp + 0x500, ((x << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x502, ((y << 4) & 0x3FFF) as u32);
            fx.vu_sub4(sp + 0x3D0, s0, c);
            fx.vu_sub4(sp + 0x3E0, s0 + 0x10, c);
            fx.vu_scale4(sp + 0x3D0, sp + 0x3D0, f20);
            fx.vu_scale4(sp + 0x3E0, sp + 0x3E0, f20);
            fx.vu_add4(sp + 0x3D0, sp + 0x3D0, c);
            fx.vu_add4(sp + 0x3E0, sp + 0x3E0, c);
            let x = fix_at(fx, c, 0x01E0_1E5C);
            let y = fix_at(fx, c + 4, 0x01E0_1E6C);
            xyz_far(fx, sp + 0x4D8, x, y);
            let x = fix_at(fx, sp + 0x3D0, 0x01E0_1ED8);
            let y = fix_at(fx, sp + 0x3D4, 0x01E0_1EE4);
            xyz_far(fx, sp + 0x4F0, x, y);
            let x = fix_at(fx, sp + 0x3E0, 0x01E0_1F50);
            let y = fix_at(fx, sp + 0x3E4, 0x01E0_1F5C);
            xyz_far(fx, sp + 0x508, x, y);
            fx.submit(d, 0, 0);
        }
    }
    let layer = fx.mem.read_u32(sp + 0xDC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x9354) + 1;
    fx.mem.set32(D + 0x9354, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + 0x9350, 0);
    }
}
