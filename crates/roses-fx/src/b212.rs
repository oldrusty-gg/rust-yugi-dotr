//! BMODULE b212 (card 100), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0x6F20
//! bytes) [F]: a single unscheduled function in a
//! 0x510 frame (s1 = D, s0 the OT, the return at sp + 0xC0), the b014 /
//! b563 family. Every update the camera turn with roll (sp + 0x2D0). On
//! 0x3C node 0x4D (D+0x20) and for 16 updates a burst there: a 60-wide
//! sprite, a 32-quad flat far-Z disc (the rings D+0x50, 6.5 / 10.8 / 11
//! round), 96 textured quads (D+0x680, random turned blades) and 128
//! sprites (D+0x1F00 directions, D+0x1E80 cells). Every twelfth update
//! below 0x4E the object op 0x1F83B0. On 0x4B for 0x83 updates a column
//! at the target's point (D+0x5610, 5 up): a 21 x 17 grid of rings
//! (D+0x2810) and, in its own packet layer, the frame copied as a texture
//! onto a turned copy (D+0x3E60, screen-position UVs, front faces only).
//! 0x4C..0xCC sprite rings (8 x 0x1A0, D+0x5620) and motes (32 x 0x60,
//! D+0x6320). Returns 1 on 0x41, 6 on 0x4C..0xCB, 2 on 0xCC, 8 once the
//! count reaches 0xCE.
use crate::blib::exe::{mul_mem, translation_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70 / 0x205210's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x510;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const DESC: u32 = 0x3C0;
/// The flat quad descriptor (prim 0x14, 8 registers).
const FLAT: u32 = 0x418;
/// The strip descriptor (prim 0x34, 12 registers).
const STRIP: u32 = 0x470;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR: u64 = 0xFF_FFF8;
const LENS_CLAMP: u64 = 0x800_0080_000A;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xC0, 0);
    for (o, r) in [(0x508u32, 0x01E0_2680u32), (0x50C, 0x01E0_2688)] {
        let w = fx.mem.read_u32(r);
        fx.mem.write_u32(sp + o, w);
    }
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

/// 0x1A2C70 / 0x1A2B80 with their residue frames.
fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}
fn fixs(fx: &mut Fx, f: f32, ra: u32) -> i32 {
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The 32 x 32 atlas cell of byte `b` (4 across) into the sprite's UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: u8) {
    let b = i32::from(b);
    let (c, r) = (b & 3, b >> 2);
    uv(fx, sp + 0x3D8, ((c << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x3DA, ((r << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x3E8, ((((c << 5) + 0x20) << 4) & 0x3FFF) as u32);
    uv(fx, sp + 0x3EA, ((((r << 5) + 0x20) << 4) & 0x3FFF) as u32);
}

fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

/// The sprite descriptor (prim 0x26 | 0x800, 5 registers) textured by
/// `tex` (ALPHA preset 1, REGION_CLAMP `clamp`).
fn sprite_desc(fx: &mut Fx, sp: u32, tex: u32, clamp: u64) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, clamp);
}

fn strip_head(fx: &mut Fx, sp: u32) {
    let d = sp + STRIP;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
}

/// The strip descriptor textured by `tex` (ALPHA preset 1, REGION_CLAMP 0).
fn strip_desc(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + STRIP;
    strip_head(fx, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
}

/// The camera's view-projection (x identity) at sp + 0x250.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x250);
    fx.identity(sp + 0x290);
    mul_mem(fx, sp + 0x250, sp + 0x290);
}

/// The view-projection at the point `p` (identity + translation).
fn camera_at(fx: &mut Fx, sp: u32, p: u32) {
    fx.camera_vp_to(sp + 0x250);
    fx.identity(sp + 0x290);
    fx.translate_rows(sp + 0x290, sp + 0x290, p);
    mul_mem(fx, sp + 0x250, sp + 0x290);
}

/// The corners at sp + 0x310 / 0x320 turned by the camera.
fn corners(fx: &mut Fx, sp: u32, a: f32, b: f32) {
    vec4(fx, sp + 0x310, [a, a, 0.0, 1.0]);
    vec4(fx, sp + 0x320, [b, b, 0.0, 1.0]);
    fx.vu_apply_to(sp + 0x310, sp + 0x2D0, sp + 0x310);
    fx.vu_apply_to(sp + 0x320, sp + 0x2D0, sp + 0x320);
}

/// The sprite at sp + 0x3B0 (+ the corners) projected; true when drawn.
fn sprite_at(fx: &mut Fx, sp: u32) -> bool {
    fx.vu_add4(sp + 0x350, sp + 0x310, sp + 0x3B0);
    fx.vu_add4(sp + 0x360, sp + 0x320, sp + 0x3B0);
    crate::fxlib2::project(fx, sp + 0x350, PROJ_OUT, sp + 0x250, sp + DESC, sp + 0x504)
}

fn sprite_draw(fx: &mut Fx, sp: u32, ra: u32) {
    if sprite_at(fx, sp) {
        let w = fx.mem.f(sp + 0x504);
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

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0x6F20);
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
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x4D, 1);
    fx.mem.set32(D + 0x1C, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x30);
    fx.mem.write_u8(D + 0x40, 0);
    for k in 0..0x21u32 {
        let p = D + 0x50 + 0x30 * k;
        let a = ((k << 16) as i32) >> 5;
        let s = fx.tsin(a);
        let c = fx.tcos(a);
        for (o, r) in [(0u32, 6.5f32), (0x10, f32::from_bits(0x412C_CCCD)), (0x20, 11.0)] {
            fx.mem.setf(p + o, ee::mul(r, s));
            fx.mem.setf(p + o + 4, ee::mul(r, c));
            fx.mem.write_u32(p + o + 8, 0);
            fx.mem.write_u32(p + o + 0xC, 0x3F80_0000);
        }
    }
    for k in 0..0x60u32 {
        let s0 = D + 0x40 * k;
        let f = ee::div(ee::mul(f32::from_bits(0x3F66_6666), ee::from_i32(k as i32 + 1)), 96.0);
        let h = ee::add(f32::from_bits(0x3DCC_CCCD), f);
        let n = ee::neg(h);
        let z = ee::mul(15.0, h);
        vec4(fx, sp + 0x310, [0.0, n, 0.0, 1.0]);
        vec4(fx, sp + 0x320, [0.0, h, 0.0, 1.0]);
        vec4(fx, sp + 0x330, [0.0, n, z, 1.0]);
        vec4(fx, sp + 0x340, [0.0, h, z, 1.0]);
        fx.random_table_vec(sp + 0x3B0, 1.0);
        fx.point_along(sp + 0x370, sp + 0x3B0);
        let r = fx.rand_from(s0, 0x01E0_02E4) >> 23;
        fx.rotate_axis(sp + 0x370, (rem2(r, 256) << 16) >> 8, 2);
        for j in 0..4u32 {
            fx.vu_apply_to(s0 + 0x680 + 0x10 * j, sp + 0x370, sp + 0x310 + 0x10 * j);
        }
    }
    for k in 0..0x80u32 {
        let s0 = D + 0x10 * k;
        let r = fx.rand_from(s0, 0x01E0_0364) >> 23;
        fx.mem.set8(D + 0x1E80 + k, rem2(r, 4));
        fx.random_table_vec(s0 + 0x1F00, f32::from_bits(0x3ECC_CCCD));
    }
    fx.mem.write_u8(D + 0x5600, 0);
    fx.mem.write_u32(D + 0x5604, 0);
    for k in 0..8u32 {
        fx.mem.write_u8(D + 0x5620 + 0x1A0 * k, 0);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x6320 + 0x60 * k, 0);
    }
    0
}

/// 0x101430(a, b, c) [F]: 1 when the triangle of the XYZ2 x / y halfwords
/// at `a` / `b` / `c` faces the camera: its normal (0x2057B0 over three
/// points: the sum of the edge cross products (0x20C8B0) / 3, normalized
/// by 0x20C7E0) has z < 0.
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

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x2D0);
    if fx.mem.i32(D) == 0x3C {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x1C));
        fx.node_position(h, s, D + 0x20);
    }
    if fx.mem.i32(D) == 0x3C {
        fx.mem.write_u8(D + 0x40, 1);
        fx.mem.write_u32(D + 0x44, 0);
    }
    if fx.mem.read_u8(D + 0x40) == 1 {
        burst(fx, sp);
    }
    let n = fx.mem.i32(D);
    let (_, r) = fx.div(n, 12);
    if r == 0 && n > 0 && n < 0x4E {
        let h = fx.mem.read_u32(D + 4);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h, 0x20, 1, sp + 0x508]);
    }
    if fx.mem.i32(D) == 0x4B {
        fx.mem.write_u8(D + 0x5600, 1);
        fx.mem.write_u32(D + 0x5604, 0);
        let a = 0xE38;
        fx.tsin(a);
        fx.mem.write_u32(D + 0x5610, 0);
        fx.tcos(a);
        fx.mem.write_u32(D + 0x5614, 0);
        fx.mem.write_u32(D + 0x5618, 0);
        fx.mem.write_u32(D + 0x561C, 0x3F80_0000);
        fx.vu_add4(D + 0x5610, D + 0x5610, D + 0x30);
        fx.mem.setf(D + 0x5614, 5.0);
    }
    let age = fx.mem.i32(D + 0x5604);
    let f22 = ee::div(ee::from_i32(Fx::clamp_i(0x82 - age, 0, 0x20)), 32.0);
    let mut f21 = 0.0;
    if fx.mem.read_u8(D + 0x5600) == 1 {
        f21 = column(fx, sp, f22);
    }
    let n = fx.mem.i32(D);
    if (0x4C..0xCD).contains(&n) {
        let g = D + 0x1A0 * rem2(n, 8) as u32;
        fx.mem.write_u8(g + 0x5620, 1);
        fx.mem.write_u32(g + 0x5624, 0);
        let r = fx.rand_from(OT, 0x01E0_1CF4) >> 23;
        let s7 = (rem2(r, 256) << 16) >> 8;
        for k in 0..0x18u32 {
            let b = fx.rand_from(OT, 0x01E0_1D2C) >> 23;
            fx.mem.set8(g + 0x5628 + k, rem2(b, 8));
            let s4 = (k << 16) as i32;
            fx.mult(0x2AAA_AAAB, s4);
            let a = (s7 + s4 / 24) & 0xFFFF;
            let p = g + 0x5640 + 0x10 * k;
            let s = fx.tsin(a);
            fx.mem.setf(p, ee::mul(46.0, s));
            fx.mem.write_u32(p + 4, 0);
            let c = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(46.0, c));
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    sprite_desc(fx, sp, 0x18, 0);
    camera(fx, sp);
    for i in 0..8u32 {
        let g = D + 0x1A0 * i;
        if fx.mem.read_u8(g + 0x5620) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x5624);
        let tf = fx.tan_falloff_p(age, 8, 1);
        let s = ee::mul(13.0, ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf)));
        corners(fx, sp, s, ee::neg(s));
        let age = fx.mem.i32(g + 0x5624);
        let c = fx.tcos((age << 14) >> 3);
        let a = fixu(fx, ee::mul(f22, ee::mul(100.0, c)), 0x01E0_1F34);
        rgba(fx, sp + 0x3D0, [0x80, 0x80, 0x80], a as u8);
        for k in 0..0x18u32 {
            let b = fx.mem.read_u8(g + 0x5628 + k);
            cell_uvs(fx, sp, b);
            let p = g + 0x5640 + 0x10 * k;
            fx.vu_add4(sp + 0x3B0, p, D + 0x30);
            fx.mem.setf(sp + 0x3B4, 5.0);
            sprite_draw(fx, sp, 0x01E0_206C);
            if k % 2 == 0 {
                fx.vu_scale4(sp + 0x3B0, p, 0.5);
                fx.vu_add4(sp + 0x3B0, sp + 0x3B0, D + 0x30);
                fx.mem.setf(sp + 0x3B4, f21);
                sprite_draw(fx, sp, 0x01E0_2118);
            }
        }
        let age = fx.mem.i32(g + 0x5624) + 1;
        fx.mem.set32(g + 0x5624, age);
        if age >= 9 {
            fx.mem.write_u8(g + 0x5620, 0);
        }
    }
    let n = fx.mem.i32(D);
    if (0x4C..0xCD).contains(&n) {
        let g = D + 0x60 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(g + 0x6320, 1);
        fx.mem.write_u32(g + 0x6324, 0);
        for k in 0..4u32 {
            let b = fx.rand_from(OT, 0x01E0_21DC) >> 23;
            fx.mem.set8(g + 0x6328 + k, rem2(b, 8));
            let r = fx.rand_from(OT, 0x01E0_21FC) >> 23;
            fx.mem.set32(g + 0x632C + 4 * k, (rem2(r, 256) << 16) >> 8);
            fx.random_table_vec(g + 0x6340 + 0x10 * k, f32::from_bits(0x3F66_6666));
        }
    }
    sprite_desc(fx, sp, 0x18, 0);
    camera(fx, sp);
    for i in 0..0x20u32 {
        let g = D + 0x60 * i;
        if fx.mem.read_u8(g + 0x6320) == 0 {
            continue;
        }
        corners(fx, sp, 4.0, -4.0);
        let age = fx.mem.i32(g + 0x6324);
        let s = fx.tsin((age << 15) >> 5);
        let a = fixu(fx, ee::mul(f22, ee::mul(164.0, s)), 0x01E0_2374);
        rgba(fx, sp + 0x3D0, [0x80, 0x80, 0x80], a as u8);
        let age = ee::from_i32(fx.mem.i32(g + 0x6324));
        let f21 = ee::add(5.0, ee::mul(100.0, ee::div(age, 32.0)));
        for k in 0..4u32 {
            let age = fx.mem.i32(g + 0x6324);
            let a = (fx.mem.i32(g + 0x632C + 4 * k).wrapping_add(age.wrapping_mul(0x309))) & 0xFFFF;
            let s = fx.tsin(a);
            let b = fx.mem.read_u8(g + 0x6328 + k);
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + 0x3B0, g + 0x6340 + 0x10 * k, s);
            let x = ee::add(fx.mem.f(sp + 0x3B0), fx.mem.f(D + 0x30));
            fx.mem.setf(sp + 0x3B0, x);
            let y = ee::add(fx.mem.f(sp + 0x3B4), f21);
            fx.mem.setf(sp + 0x3B4, y);
            let z = ee::add(fx.mem.f(sp + 0x3B8), fx.mem.f(D + 0x38));
            fx.mem.setf(sp + 0x3B8, z);
            sprite_draw(fx, sp, 0x01E0_2518);
        }
        let age = fx.mem.i32(g + 0x6324) + 1;
        fx.mem.set32(g + 0x6324, age);
        if age >= 0x21 {
            fx.mem.write_u8(g + 0x6320, 0);
        }
    }
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x41 {
        ret = 1;
    } else {
        if (0x4C..0xCC).contains(&n) {
            ret = 6;
        }
        if n == 0xCC {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xCE {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// The burst at node 0x4D (16 updates) [F 0x1E00450..0x1E00F04].
fn burst(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    sprite_desc(fx, sp, 0x10, 5);
    uv(fx, sp + 0x3D8, 0x400);
    uv(fx, sp + 0x3DA, 0);
    uv(fx, sp + 0x3E8, 0x600);
    uv(fx, sp + 0x3EA, 0x200);
    vec4(fx, sp + 0x310, [-30.0, -30.0, 0.0, 1.0]);
    vec4(fx, sp + 0x320, [30.0, 30.0, 0.0, 1.0]);
    let age = fx.mem.i32(D + 0x44);
    let f20 = ee::div(ee::from_i32(0x10 - age), 16.0);
    let f22 = ee::mul(255.0, f20);
    let a = fixu(fx, f22, 0x01E0_05C4);
    rgba(fx, sp + 0x3D0, [0x64, 0x78, 0xFF], a as u8);
    fx.camera_vp_to(sp + 0x250);
    fx.translate_rows(sp + 0x290, sp + 0x2D0, D + 0x20);
    mul_mem(fx, sp + 0x250, sp + 0x290);
    if crate::fxlib2::project(fx, sp + 0x310, PROJ_OUT, sp + 0x250, d, sp + 0x504) {
        far_z(fx, sp + 0x3E0, &[sp + 0x3F0]);
        fx.submit(d, OT, 0);
    }
    // The flat disc.
    let q = sp + FLAT;
    fx.mem.write_u16(q + 2, 0x14);
    fx.mem.write_u16(q, 8);
    fx.mem.write_u64(q + 8, 0x4141_4141);
    fx.mem.write_u32(q + 4, 0);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(q, p);
    let a = fixu(fx, f22, 0x01E0_0700);
    rgba(fx, sp + 0x428, [0x0A, 0x50, 0xFF], a as u8);
    words(fx, sp + 0x438, sp + 0x428, 2);
    rgba(fx, sp + 0x448, [0x18, 0x20, 0xFF], 0);
    words(fx, sp + 0x458, sp + 0x448, 2);
    fx.camera_vp_to(sp + 0x250);
    fx.translate_rows(sp + 0x290, sp + 0x2D0, D + 0x20);
    let age = fx.mem.i32(D + 0x44);
    let tf = fx.tan_falloff_p(age, 0x10, 1);
    let s = ee::add(f32::from_bits(0x3E99_999A), ee::mul(f32::from_bits(0x3F33_3333), tf));
    vec4(fx, sp + 0x3B0, [s, s, 0.0, 1.0]);
    fx.scale_rows(sp + 0x290, sp + 0x3B0);
    mul_mem(fx, sp + 0x250, sp + 0x290);
    for k in 0..0x20u32 {
        let s2 = D + 0x30 * k;
        let s4 = s2 + 0x30;
        for (o, ra) in [(0x50u32, 0x01E0_082Cu32), (0x70, 0x01E0_0938)] {
            if fx.project_n(&[s2 + 0x60, s4 + 0x60, s2 + o, s4 + o], sp + 0x250, q, sp + 0x504, ra) == 1 {
                far_z(fx, sp + 0x430, &[sp + 0x440, sp + 0x450, sp + 0x460]);
                fx.submit(q, OT, 0);
            }
        }
    }
    // The blades.
    strip_desc(fx, sp, 0x10);
    let a = fixu(fx, ee::mul(96.0, f20), 0x01E0_0AA0);
    rgba(fx, sp + 0x480, [0xFF, 0xD8, 0xC4], a as u8);
    words(fx, sp + 0x498, sp + 0x480, 2);
    rgba(fx, sp + 0x4B0, [0x80, 0x64, 0x5A], 0);
    words(fx, sp + 0x4C8, sp + 0x4B0, 2);
    camera_at(fx, sp, D + 0x20);
    for k in 0..0x60u32 {
        let s2 = D + 0x40 * k;
        for (o, v) in [(0x488u32, 0.5f32), (0x48C, 0.25), (0x4A0, 0.75), (0x4A4, 0.25), (0x4B8, 0.5), (0x4BC, 0.25), (0x4D0, 0.75), (0x4D4, 0.25)] {
            fx.mem.setf(sp + o, v);
        }
        if fx.project_n(&[s2 + 0x680, s2 + 0x690, s2 + 0x6A0, s2 + 0x6B0], sp + 0x250, sp + STRIP, sp + 0x504, 0x01E0_0B94) == 1 {
            let w = fx.mem.f(sp + 0x504);
            let key = fixu(fx, w, 0x01E0_0BA8);
            fx.submit(sp + STRIP, OT, key as i32);
        }
    }
    // The sprites.
    sprite_desc(fx, sp, 0x10, 5);
    let s = f32::from_bits(0x3ECC_CCCD);
    corners(fx, sp, ee::neg(s), s);
    let a = fixu(fx, ee::mul(128.0, f20), 0x01E0_0CBC);
    rgba(fx, sp + 0x3D0, [0xFF, 0xC8, 0xBB], a as u8);
    camera_at(fx, sp, D + 0x20);
    let age = fx.mem.i32(D + 0x44);
    let tf = fx.tan_falloff_p(age, 0x10, 1);
    let f20 = ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), tf));
    for k in 0..0x80u32 {
        let b = fx.mem.read_u8(D + 0x1E80 + k);
        cell_uvs(fx, sp, b);
        fx.vu_scale4(sp + 0x3B0, D + 0x1F00 + 0x10 * k, f20);
        if sprite_at(fx, sp) {
            far_z(fx, sp + 0x3E0, &[sp + 0x3F0]);
            fx.submit(d, OT, 0);
        }
    }
    let age = fx.mem.i32(D + 0x44) + 1;
    fx.mem.set32(D + 0x44, age);
    if age >= 0x11 {
        fx.mem.write_u8(D + 0x40, 0);
    }
}

/// The colours of row `k` of the 20 strip rows: (0x80, g, b) with alpha
/// `a` but 0 on the outer edge (the first row's top, the last's bottom).
fn row_colours(fx: &mut Fx, sp: u32, k: u32, g: u8, b: u8, a: u8) {
    let (top, bottom) = match k {
        0 => (0, a),
        0x13 => (a, 0),
        _ => (a, a),
    };
    rgba(fx, sp + 0x480, [0x80, g, b], top);
    words(fx, sp + 0x498, sp + 0x480, 2);
    rgba(fx, sp + 0x4B0, [0x80, g, b], bottom);
    words(fx, sp + 0x4C8, sp + 0x4B0, 2);
}

/// The column at the target (0x83 updates) [F 0x1E01020..0x1E01C98];
/// returns f21 (its height).
fn column(fx: &mut Fx, sp: u32, f22: f32) -> f32 {
    let age = fx.mem.i32(D + 0x5604);
    for k in 0..0x15u32 {
        let s2 = (k << 16) as i32;
        fx.mult(0x6666_6667, s2);
        let a = (s2 / 20).wrapping_add(age.wrapping_mul(1820)) & 0xFFFF;
        let p = D + 0x54B0 + 0x10 * k;
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(0.0, s));
        fx.mem.write_u32(p + 4, 0);
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(0.0, c));
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
    }
    let age = fx.mem.i32(D + 0x5604);
    let c = Fx::clamp_i(age, 0, 0x10);
    let tf = fx.tan_falloff_p(c, 0x10, 1);
    let f21 = ee::mul(100.0, tf);
    strip_desc(fx, sp, 0x14);
    let a = fixu(fx, ee::mul(96.0, f22), 0x01E0_1188);
    fx.mem.write_u8(sp + 0x240, a as u8);
    for k in 0..0x11u32 {
        let p = D + 0x2700 + 0x10 * k;
        let a = ((k << 16) as i32) >> 4;
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(45.0, s));
        fx.mem.write_u32(p + 4, 0);
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(45.0, c));
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
    }
    fx.camera_vp_to(sp + 0x250);
    translation_into(fx, sp + 0x290, D + 0x5610);
    mul_mem(fx, sp + 0x250, sp + 0x290);
    grid(fx, f21, D + 0x2810, None);
    let age = fx.mem.i32(D + 0x5604);
    let (_, q) = fx.div(age, 0xE);
    let f20 = ee::sub(0.0, ee::div(ee::from_i32(q), 14.0));
    let a = fx.mem.read_u8(sp + 0x240);
    let mut last = None;
    for k in 0..0x14u32 {
        let s4 = D + 0x110 * k;
        let s5 = s4 + 0x110;
        row_colours(fx, sp, k, 0xB2, 0xC8, a);
        let f23 = ee::add(ee::div(ee::mul(2.0, ee::from_i32(k as i32)), 20.0), f20);
        let f24 = ee::add(ee::div(ee::mul(2.0, ee::from_i32(k as i32 + 1)), 20.0), f20);
        for j in 0..0x10u32 {
            let s0 = ee::div(ee::mul(2.0, ee::from_i32(j as i32)), 16.0);
            let s1 = ee::div(ee::mul(2.0, ee::from_i32(j as i32 + 1)), 16.0);
            for (o, v) in [(0x488u32, s0), (0x48C, f23), (0x4A0, s1), (0x4A4, f23), (0x4B8, s0), (0x4BC, f24), (0x4D0, s1), (0x4D4, f24)] {
                fx.mem.setf(sp + o, v);
            }
            let (a0, a1) = (s4 + 0x10 * j + 0x2810, s4 + 0x10 * (j + 1) + 0x2810);
            let (a2, a3) = (s5 + 0x10 * j + 0x2810, s5 + 0x10 * (j + 1) + 0x2810);
            last = None;
            if fx.project_n(&[a0, a1, a2, a3], sp + 0x250, sp + STRIP, sp + 0x504, 0x01E0_1580) == 1 {
                let w = fx.mem.f(sp + 0x504);
                let key = fixu(fx, w, 0x01E0_1594);
                fx.submit(sp + STRIP, OT, key as i32);
                last = Some(key);
            }
        }
    }
    // The lens: the frame copied as a texture onto a turned copy.
    strip_head(fx, sp);
    let d = sp + STRIP;
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xDC, layer);
    let a = lens_regs(sp, last);
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x4E8, sp + 0x4F0);
    fx.texflush(regs);
    let p = fx.alpha_preset(0);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, LENS_CLAMP);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fixu(fx, ee::mul(128.0, f22), 0x01E0_16BC);
    fx.mem.write_u8(sp + 0x230, a as u8);
    for k in 0..0x11u32 {
        let p = D + 0x2700 + 0x10 * k;
        let a = ((k << 16) as i32) >> 4;
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(44.0, s));
        fx.mem.write_u32(p + 4, 0);
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(44.0, c));
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
    }
    fx.camera_vp_to(sp + 0x250);
    translation_into(fx, sp + 0x290, D + 0x5610);
    mul_mem(fx, sp + 0x250, sp + 0x290);
    grid(fx, f21, D + 0x2810, Some(sp));
    for k in 0..0x14u32 {
        let s7 = D + 0x110 * k;
        let fp = s7 + 0x110;
        row_colours(fx, sp, k, 0x8A, 0xAA, a as u8);
        for j in 0..0x10u32 {
            let (s3, s5) = (s7 + 0x10 * j, s7 + 0x10 * (j + 1));
            let (s2, s4) = (fp + 0x10 * j, fp + 0x10 * (j + 1));
            if fx.project_n(&[s3 + 0x3E60, s5 + 0x3E60, s2 + 0x3E60, s4 + 0x3E60], sp + 0x250, d, sp + 0x504, 0x01E0_1B48) != 1 {
                continue;
            }
            for (u, x) in [(0x488u32, 0x490u32), (0x4A0, 0x4A8), (0x4B8, 0x4C0), (0x4D0, 0x4D8)] {
                fx.uv_from_xy(sp + u, sp + x, 0, 0);
            }
            if fx.project_n(&[s3 + 0x2810, s5 + 0x2810, s2 + 0x2810, s4 + 0x2810], sp + 0x250, d, sp + 0x504, 0x01E0_1BC4) == 1 && front(fx, sp + 0x490, sp + 0x4A8, sp + 0x4C0) {
                fx.submit(d, 0, 0);
            }
        }
    }
    let layer = fx.mem.read_u32(sp + 0xDC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x5604) + 1;
    fx.mem.set32(D + 0x5604, age);
    if age >= 0x83 {
        fx.mem.write_u8(D + 0x5600, 0);
    }
    f21
}

/// a0..a3 at the lens' first TEXFLUSH: as the strip loop's last submit
/// leaves them (the descriptor, the OT, the key, the descriptor still in
/// a3 from 0x100BA0's 0x205210) [run]; after a rejected last strip
/// 0x205210's own [H: the descriptor in a3, the rest not read].
fn lens_regs(sp: u32, last: Option<u32>) -> [u32; 4] {
    match last {
        Some(key) => [sp + STRIP, OT, key, sp + STRIP],
        None => [sp + STRIP, OT, 0, sp + STRIP],
    }
}

/// The 21 x 17 grid of rings D+0x2810 + 0x110 r + 0x10 k: the ring
/// D+0x2700 scaled 0.5 + 0.5 (1 - tan falloff(r, 20)) at height f21 r / 20
/// round the column's centre D+0x54B0 + 0x10 r; with `turn` (the lens)
/// also turned about y by 910 sin(..) into D+0x3E60 + .., wobbling.
fn grid(fx: &mut Fx, f21: f32, _base: u32, turn: Option<u32>) {
    let age = fx.mem.i32(D + 0x5604);
    for r in 0..0x15u32 {
        let f20 = ee::div(ee::mul(f21, ee::from_i32(r as i32)), 20.0);
        let tf = fx.tan_falloff_p(r as i32, 0x14, 1);
        let f23 = ee::add(0.5, ee::mul(0.5, ee::sub(1.0, tf)));
        let centre = D + 0x54B0 + 0x10 * r;
        if let Some(sp) = turn {
            let m = sp + 0x370;
            fx.identity(m);
            let (_, q) = fx.div(age, 0x60);
            let a1 = q << 16;
            fx.mult(0x2AAA_AAAB, a1);
            let fp = (r << 16) as i32;
            fx.mult(0x6666_6667, fp);
            let a = ((fp / 20).wrapping_add(a1 / 96)) & 0xFFFF;
            let s = fx.tsin(a);
            let v = fixs(fx, ee::mul(910.0, s), 0x01E0_1854);
            fx.rotate_axis(m, v, 1);
        }
        for k in 0..0x11u32 {
            let p = D + 0x2810 + 0x110 * r + 0x10 * k;
            fx.vu_scale4(p, D + 0x2700 + 0x10 * k, f23);
            fx.mem.setf(p + 4, f20);
            fx.vu_add4(p, p, centre);
            if let Some(sp) = turn {
                let t = p + 0x1650;
                fx.vu_apply_to(t, sp + 0x370, p);
                let (_, q) = fx.div(age, 0x30);
                let v = q << 16;
                fx.mult(0x2AAA_AAAB, v);
                let a = ((((k << 16) as i32) >> 4).wrapping_add(v / 48)) & 0xFFFF;
                let s = fx.tsin(a);
                let w = ee::mul(ee::mul(f32::from_bits(0x3E19_999A), ee::div(f21, 20.0)), s);
                fx.mem.setf(t + 4, ee::add(f20, w));
                fx.vu_add4(t, t, centre);
            }
        }
    }
}
