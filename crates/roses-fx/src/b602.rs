//! BMODULE b602 (card 33), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xA9E0
//! bytes) [F]: a single delay-slot-scheduled
//! function (beql / bnel) in a 0x510 frame (D kept at sp + 0xE0, s0 = the
//! OT), b630's compile. Init: the handles, the attacker's side, textures
//! (5, 0..2) D+0x10..0x18, the attacker's nodes 0x15 (clear) / 0x17 / 0x13
//! at D+0x1C / 0x20 / 0x60, the target's point D+0x80, a 33-point ring
//! table (D+0x5320 + 0x30 k: radii 4 / 4.7 / 5). Every update the camera
//! turn with roll (sp + 0x2D0), the two nodes' positions D+0x30 / 0x40,
//! their midpoint D+0x50 and node 0x13's D+0x70. On 0x49..0xE4 one of 48
//! streams (D+0x90 + 0xF0 k, 48 updates) starts at the midpoint (4 x side
//! higher z) with eight random directions to the target, drawn as eight
//! growing sprites (alternately alpha preset 0 white-yellow / 1 dim) that
//! drop a trail entry (D+0x61E0 + 0x100 k + 0x20 j, 24 updates: two
//! sprites, one turned) every fourth update; on 0x65..0x12B one of 24
//! bursts (D+0x2D90 + 0x130 k, 25 updates: 16 sprites rising about the
//! target, trail entries D+0x91E0.. of 17 updates); on 0x49..0xE4 one of
//! 24 puffs (D+0x4A10 + 0x60 k, four sprites spreading from the midpoint);
//! on 0x3C a 33-update flash at node 0x13 (a sprite, a band of 32 x 2
//! quads on the ring table, 128 sparkles). Returns 1 on 0x5A, 6 on
//! 0x65..0x12A, 2 on 0x12B, 8 once the count reaches 0x12D.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{FAR_Z, KEEP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x510);
    fx.mem.write_u32(sp + 0xE0, D);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp) };
    fx.leave(0x510);
    ret
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0xA9E0);
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
    for (at, node, clear) in [(0x1Cu32, 0x15, 1), (0x20, 0x17, 0), (0x60, 0x13, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x80);
    for k in 0..0x30u32 {
        fx.mem.write_u8(D + 0x90 + 0xF0 * k, 0);
        fx.mem.write_u32(D + 0x170 + 0xF0 * k, 0);
    }
    for k in 0..0x18u32 {
        fx.mem.write_u8(D + 0x2D90 + 0x130 * k, 0);
        fx.mem.write_u32(D + 0x2EB0 + 0x130 * k, 0);
    }
    for k in 0..0x18u32 {
        fx.mem.write_u8(D + 0x4A10 + 0x60 * k, 0);
    }
    fx.mem.write_u8(D + 0x5310, 0);
    for k in 0..0x21u32 {
        let a = ((k << 16) as i32) >> 5;
        let s = fx.table_trig(a, false);
        fx.mem.setf(sp + 0x3D0, s);
        let c = fx.table_trig(a, true);
        fx.mem.setf(sp + 0x3D4, c);
        fx.mem.write_u32(sp + 0x3D8, 0);
        fx.mem.write_u32(sp + 0x3DC, 0x3F80_0000);
        let o = D + 0x30 * k;
        fx.vu_scale4(o + 0x5320, sp + 0x3D0, 4.0);
        fx.vu_scale4(o + 0x5330, sp + 0x3D0, f32::from_bits(0x4096_6666));
        fx.vu_scale4(o + 0x5340, sp + 0x3D0, 5.0);
    }
    for g in 0..0x30u32 {
        for j in 0..8u32 {
            fx.mem.write_u8(D + 0x61E0 + 0x100 * g + 0x20 * j, 0);
        }
    }
    for g in 0..0x18u32 {
        for j in 0..8u32 {
            fx.mem.write_u8(D + 0x91E0 + 0x100 * g + 0x20 * j, 0);
        }
        fx.mem.write_u32(sp + 0xE0, D + 0x100 * (g + 1));
    }
    0
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

fn uv(fx: &mut Fx, a: u32, v: i32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The cell UVs (c, r) into sp + 0x3F8 / 0x3FA / 0x408 / 0x40A (32-texel
/// cells of a 128 x 128 sheet).
fn cell_uv(fx: &mut Fx, sp: u32, c: i32, r: i32) {
    uv(fx, sp + 0x3F8, c << 9);
    uv(fx, sp + 0x3FA, r << 9);
    uv(fx, sp + 0x408, ((c << 5) + 0x20) << 4);
    uv(fx, sp + 0x40A, ((r << 5) + 0x20) << 4);
}

fn copy_n(fx: &mut Fx, to: u32, from: u32, words: u32) {
    for c in 0..words {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

/// The far Z into the XYZ2 at `first`, then its Z into the others.
fn far_z(fx: &mut Fx, first: u32, others: &[u32]) {
    let v = fx.mem.read_u64(first);
    fx.mem.write_u64(first, (v & KEEP) | FAR_Z);
    let z = ((fx.mem.read_u64(first) << 8) >> 40) << 32;
    for &o in others {
        let w = fx.mem.read_u64(o);
        fx.mem.write_u64(o, (w & KEEP) | z);
    }
}

/// 0x1A2C70 (fixunssfsi) with its frame.
fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

fn v4(fx: &mut Fx, a: u32, x: f32, y: f32) {
    fx.mem.setf(a, x);
    fx.mem.setf(a + 4, y);
    fx.mem.write_u32(a + 8, 0);
    fx.mem.write_u32(a + 0xC, 0x3F80_0000);
}

fn colour(fx: &mut Fx, a: u32, c: [u8; 4]) {
    for (k, b) in c.into_iter().enumerate() {
        fx.mem.write_u8(a + k as u32, b);
    }
    fx.mem.write_u32(a + 4, 0x3F80_0000);
}

/// The sprite descriptor sp + 0x3E0 (5 registers 0x42421, prim 0x26 |
/// 0x800) through its field pointers sp + 0x150 / 0x160 / 0x170.
fn sprite_desc(fx: &mut Fx, sp: u32) -> u32 {
    let d = sp + 0x3E0;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    d
}

/// The quad descriptor sp + 0x438 (9 registers 0x4_2424_2421, prim 0x24 |
/// 0x800, alpha preset 1, texture D+0x18, clamp 5, UVs 0..0x200).
fn quad_desc(fx: &mut Fx, sp: u32) -> u32 {
    let d = sp + 0x438;
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    for (o, v) in [(0x450u32, 0), (0x452, 0), (0x460, 0x200), (0x462, 0), (0x470, 0), (0x472, 0x200), (0x480, 0x200), (0x482, 0x200)] {
        uv(fx, sp + o, v);
    }
    d
}

/// The trails' sprite descriptor (alpha preset 1, texture D+0x14, clamp
/// 5, UVs 0x600 / 0 .. 0x800 / 0x200) and the plain view-projection.
fn trail_sprite_desc(fx: &mut Fx, sp: u32) {
    let d = sprite_desc(fx, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    flash_uv(fx, sp);
    plain_vp(fx, sp);
}

fn flash_uv(fx: &mut Fx, sp: u32) {
    uv(fx, sp + 0x3F8, 0x600);
    uv(fx, sp + 0x3FA, 0);
    uv(fx, sp + 0x408, 0x800);
    uv(fx, sp + 0x40A, 0x200);
}

/// sp + 0x250 = the view-projection times the identity at sp + 0x290.
fn plain_vp(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x250);
    fx.identity(sp + 0x290);
    let m = fx.read_m4(sp + 0x290);
    fx.mul_into(Some(sp + 0x250), &m);
}

fn project(fx: &mut Fx, sp: u32, d: u32) -> bool {
    crate::fxlib2::project(fx, sp + 0x350, PROJ_OUT, sp + 0x250, d, sp + 0x50C)
}

fn submit_w(fx: &mut Fx, sp: u32, d: u32, ra: u32) {
    let w = fx.mem.f(sp + 0x50C);
    let key = fixu(fx, w, ra);
    fx.submit(d, OT, key as i32);
}

/// The window ramp clamp(n - 0x64, 0, 32) clamp(0x12C - n, 0, 32) / 1024.
fn window_ramp(fx: &mut Fx) -> f32 {
    let a = Fx::clamp_i(fx.mem.i32(D) - 0x64, 0, 0x20);
    let b = Fx::clamp_i(0x12C - fx.mem.i32(D), 0, 0x20);
    ee::div(ee::mul(ee::from_i32(a), ee::from_i32(b)), 1024.0)
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x2D0);
    for k in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0x1C + 4 * k);
        fx.node_position(h, n, D + 0x30 + 0x10 * k);
    }
    fx.vu_add4(D + 0x50, D + 0x30, D + 0x40);
    fx.vu_scale4(D + 0x50, D + 0x50, 0.5);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x60);
    fx.node_position(h, n, D + 0x70);
    streams(fx, sp);
    bursts(fx, sp);
    puffs(fx, sp);
    flash(fx, sp);
    trails(fx, sp);
    burst_trails(fx, sp);
    let n = fx.mem.i32(D);
    if n == 0x5A {
        fx.mem.write_u32(sp + 0xB0, 1);
    } else {
        if (0x65..0x12B).contains(&n) {
            fx.mem.write_u32(sp + 0xB0, 6);
        }
        if n == 0x12B {
            fx.mem.write_u32(sp + 0xB0, 2);
        }
    }
    let n = fx.mem.i32(D) + 1;
    fx.mem.set32(D, n);
    if n >= 0x12D {
        fx.mem.write_u32(sp + 0xB0, 8);
    }
    fx.mem.i32(sp + 0xB0)
}

/// [F 0x1E003BC..0x1E00A80] the streams D+0x90 + 0xF0 k.
fn streams(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x49..0xE5).contains(&n) {
        let (_, e) = fx.div(n, 0x30);
        let rec = D + 0xF0 * e as u32;
        fx.mem.write_u8(rec + 0x90, 1);
        fx.mem.write_u32(rec + 0x94, 0);
        copy_n(fx, rec + 0xE0, D + 0x50, 4);
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let z = ee::add(fx.mem.f(rec + 0xE8), ee::mul(4.0, side));
        fx.mem.setf(rec + 0xE8, z);
        for j in 0..8u32 {
            let r = fx.rand_from(D, 0x01E0_0460) >> 23;
            fx.mem.set32(rec + 0x98 + 4 * j, rem2(r, 8));
            let r = fx.rand_from(D, 0x01E0_0480) >> 23;
            let f = ee::add(f32::from_bits(0x3F4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(rem2(r, 256))), 256.0));
            fx.mem.setf(rec + 0xB8 + 4 * j, f);
            let v = rec + 0xF0 + 0x10 * j;
            fx.random_table_vec(v, 1.0);
            fx.vu_add4(v, v, D + 0x80);
            fx.vu_sub4(v, v, rec + 0xE0);
        }
    }
    let d = sprite_desc(fx, sp);
    fx.mem.write_u32(sp + 0x150, d + 2);
    fx.mem.write_u32(sp + 0x160, d + 8);
    fx.mem.write_u32(sp + 0x170, d + 4);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    plain_vp(fx, sp);
    let c = Fx::clamp_i(0xE5 - fx.mem.i32(D), 0, 0x20);
    let f20 = ee::div(ee::from_i32(c), 32.0);
    fx.mem.write_u32(sp + 0x100, D);
    fx.mem.write_u32(sp + 0x240, 0);
    for k in 0..0x30u32 {
        let rec = D + 0xF0 * k;
        if fx.mem.read_u8(rec + 0x90) != 0 {
            stream(fx, sp, rec, f20);
        }
        fx.mem.write_u32(sp + 0x100, D + 0x100 * (k + 1));
        fx.mem.write_u32(sp + 0x240, k + 1);
    }
}

fn stream(fx: &mut Fx, sp: u32, rec: u32, f20: f32) {
    let d = sp + 0x3E0;
    let (s2, s5) = (sp + 0x320, sp + 0x360);
    let a = ee::from_i32(fx.mem.i32(rec + 0x94));
    let f21 = ee::mul(80.0, Fx::ratio_below(a, 2.0));
    let a = ee::from_i32(fx.mem.i32(rec + 0x94));
    let f0 = ee::mul(f21, Fx::ramp_above(a, 48.0, 32.0));
    let x = fixu(fx, ee::mul(f20, f0), 0x01E0_064C);
    fx.mem.write_u8(sp + 0xC0, x as u8);
    let a = ee::from_i32(fx.mem.i32(rec + 0x94));
    let f21 = ee::mul(32.0, Fx::ratio_below(a, 2.0));
    let a = ee::from_i32(fx.mem.i32(rec + 0x94));
    let f0 = ee::mul(f21, Fx::ramp_above(a, 48.0, 32.0));
    let x = fixu(fx, ee::mul(f20, f0), 0x01E0_0690);
    fx.mem.write_u8(sp + 0xD0, x as u8);
    let a = ee::from_i32(fx.mem.i32(rec + 0x94));
    let s = ee::mul(12.0, ee::add(0.5, ee::div(ee::mul(0.5, a), 48.0)));
    v4(fx, sp + 0x310, ee::neg(s), ee::neg(s));
    v4(fx, s2, s, s);
    fx.vu_apply_to(sp + 0x310, sp + 0x2D0, sp + 0x310);
    fx.vu_apply_to(s2, sp + 0x2D0, s2);
    let a = ee::from_i32(fx.mem.i32(rec + 0x94));
    let f21 = ee::div(ee::mul(1.5, a), 48.0);
    fx.mem.write_u32(sp + 0xF0, rec + 0xE0);
    for j in 0..8u32 {
        if j & 1 == 0 {
            let a = fx.alpha_preset(0);
            fx.desc_alpha(d, a);
            let al = fx.mem.read_u8(sp + 0xD0);
            colour(fx, sp + 0x3F0, [0xFF, 0xFF, 0xC8, al]);
        } else {
            let a = fx.alpha_preset(1);
            fx.desc_alpha(d, a);
            let al = fx.mem.read_u8(sp + 0xC0);
            colour(fx, sp + 0x3F0, [0x80, 0x80, 0x40, al]);
        }
        let cell = fx.mem.i32(rec + 0x98 + 4 * j);
        cell_uv(fx, sp, rem2(cell, 4), cell >> 2);
        let f = fx.mem.f(rec + 0xB8 + 4 * j);
        fx.vu_scale4(sp + 0x350, sp + 0x310, f);
        let f = fx.mem.f(rec + 0xB8 + 4 * j);
        fx.vu_scale4(s5, s2, f);
        fx.vu_scale4(sp + 0x3D0, rec + 0xF0 + 0x10 * j, f21);
        fx.vu_add4(sp + 0x3D0, sp + 0x3D0, rec + 0xE0);
        fx.vu_add4(sp + 0x350, sp + 0x350, sp + 0x3D0);
        fx.vu_add4(s5, s5, sp + 0x3D0);
        if project(fx, sp, d) {
            submit_w(fx, sp, d, 0x01E0_0910);
        }
    }
    let age = fx.mem.i32(rec + 0x94);
    if rem2(age, 4) == 0 {
        let c = fx.mem.i32(rec + 0x170);
        fx.mem.set32(rec + 0x170, c + 1);
        if c + 1 >= 8 {
            fx.mem.write_u32(rec + 0x170, 0);
        }
        let base = fx.mem.read_u32(sp + 0x100);
        let e = base.wrapping_add((c << 5) as u32);
        fx.mem.write_u8(e + 0x61E0, 1);
        fx.mem.write_u32(e + 0x61E4, 0);
        let r = fx.rand_from(D, 0x01E0_0994) >> 23;
        let f = ee::add(f32::from_bits(0x3E99_999A), ee::div(ee::mul(f32::from_bits(0x3F33_3333), ee::from_i32(rem2(r, 256))), 256.0));
        fx.mem.setf(e + 0x61E8, f);
        let r = fx.rand_from(D, 0x01E0_09F0) >> 23;
        fx.mem.set32(e + 0x61EC, (rem2(r, 8) << 16) >> 3);
        fx.random_table_vec(e + 0x61F0, 0.5);
        fx.vu_add4(e + 0x61F0, e + 0x61F0, sp + 0x3D0);
    }
    let age = fx.mem.i32(rec + 0x94) + 1;
    fx.mem.set32(rec + 0x94, age);
    if age >= 0x31 {
        fx.mem.write_u8(rec + 0x90, 0);
    }
}

/// [F 0x1E00A80..0x1E01070] the bursts D+0x2D90 + 0x130 k at the target.
fn bursts(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x65..0x12C).contains(&n) {
        let (_, e) = fx.div(n, 0x18);
        let rec = D + 0x130 * e as u32;
        fx.mem.write_u8(rec + 0x2D90, 1);
        fx.mem.write_u32(rec + 0x2D94, 0);
        for j in 0..0x10u32 {
            let r = fx.rand_from(D, 0x01E0_0AE8) >> 23;
            fx.mem.set8(rec + 0x2D98 + j, rem2(r, 8));
            fx.random_table_vec(rec + 0x2DB0 + 0x10 * j, f32::from_bits(0x3FA6_6666));
        }
    }
    let d = sprite_desc(fx, sp);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
    plain_vp(fx, sp);
    let f20 = window_ramp(fx);
    fx.mem.write_u32(sp + 0x230, 0);
    fx.mem.write_u32(sp + 0x110, D);
    for k in 0..0x18u32 {
        let rec = D + 0x130 * k;
        if fx.mem.read_u8(rec + 0x2D90) != 0 {
            burst(fx, sp, rec, f20);
        }
        fx.mem.write_u32(sp + 0x110, D + 0x100 * (k + 1));
        fx.mem.write_u32(sp + 0x230, k + 1);
    }
}

fn burst(fx: &mut Fx, sp: u32, rec: u32, f20: f32) {
    let d = sp + 0x3E0;
    let (s2, s5) = (sp + 0x320, sp + 0x360);
    let v = fx.mem.i32(rec + 0x2D94) << 15;
    fx.mult(0x2AAA_AAAB, v);
    let q = (fx.hilo().1 >> 2) + ((v as u32) >> 31) as i32;
    let s = fx.table_trig(q, false);
    let a = fixu(fx, ee::mul(f20, ee::mul(64.0, s)), 0x01E0_0C7C);
    colour(fx, sp + 0x3F0, [0x80, 0x80, 0x5A, a as u8]);
    let age = fx.mem.i32(rec + 0x2D94);
    let sz = ee::mul(15.0, ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(0x18 - age)), 24.0)));
    v4(fx, sp + 0x310, ee::neg(sz), ee::neg(sz));
    v4(fx, s2, sz, sz);
    fx.vu_apply_to(sp + 0x310, sp + 0x2D0, sp + 0x310);
    fx.vu_apply_to(s2, sp + 0x2D0, s2);
    let age = fx.mem.i32(rec + 0x2D94);
    let f4 = ee::div(ee::from_i32(age), 24.0);
    let f1 = ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(0x18 - age)), 24.0);
    let f21 = ee::add(f32::from_bits(0x3F66_6666), f1);
    let f22 = ee::add(-25.0, ee::mul(80.0, f4));
    for j in 0..0x10u32 {
        let b = i32::from(fx.mem.read_u8(rec + 0x2D98 + j));
        cell_uv(fx, sp, b & 3, b >> 2);
        let v = rec + 0x2DB0 + 0x10 * j;
        let x = ee::mul(fx.mem.f(v), f21);
        fx.mem.setf(sp + 0x3D0, x);
        let y = ee::add(f22, fx.mem.f(v + 4));
        fx.mem.setf(sp + 0x3D4, y);
        let z = ee::mul(fx.mem.f(v + 8), f21);
        fx.mem.setf(sp + 0x3D8, z);
        fx.vu_add4(sp + 0x3D0, sp + 0x3D0, D + 0x80);
        fx.vu_add4(sp + 0x350, sp + 0x310, sp + 0x3D0);
        fx.vu_add4(s5, s2, sp + 0x3D0);
        if project(fx, sp, d) {
            submit_w(fx, sp, d, 0x01E0_0ED8);
        }
    }
    let age = fx.mem.i32(rec + 0x2D94);
    if rem2(age, 4) == 0 {
        let c = fx.mem.i32(rec + 0x2EB0);
        fx.mem.set32(rec + 0x2EB0, c + 1);
        if c + 1 >= 8 {
            fx.mem.write_u32(rec + 0x2EB0, 0);
        }
        let base = fx.mem.read_u32(sp + 0x110);
        let e = base.wrapping_add((c << 5) as u32);
        fx.mem.write_u8(e + 0x91E0, 1);
        fx.mem.write_u32(e + 0x91E4, 0);
        let r = fx.rand_from(D, 0x01E0_0F6C) >> 23;
        let f = ee::add(f32::from_bits(0x3DCC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F66_6666), ee::from_i32(rem2(r, 256))), 256.0));
        fx.mem.setf(e + 0x91E8, f);
        let r = fx.rand_from(D, 0x01E0_0FD0) >> 23;
        fx.mem.set32(e + 0x91EC, (rem2(r, 8) << 16) >> 3);
        fx.random_table_vec(e + 0x91F0, f32::from_bits(0x3DCC_CCCD));
        fx.vu_add4(e + 0x91F0, e + 0x91F0, sp + 0x3D0);
    }
    let age = fx.mem.i32(rec + 0x2D94) + 1;
    fx.mem.set32(rec + 0x2D94, age);
    if age >= 0x19 {
        fx.mem.write_u8(rec + 0x2D90, 0);
    }
}

/// [F 0x1E01070..0x1E014AC] the puffs D+0x4A10 + 0x60 k from the
/// midpoint.
fn puffs(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x49..0xE5).contains(&n) {
        let (_, e) = fx.div(n, 0x18);
        let rec = D + 0x60 * e as u32;
        fx.mem.write_u8(rec + 0x4A10, 1);
        fx.mem.write_u32(rec + 0x4A14, 0);
        copy_n(fx, rec + 0x4A20, D + 0x50, 4);
        for j in 0..4u32 {
            let r = fx.rand_from(D, 0x01E0_10F0) >> 23;
            fx.mem.set16(rec + 0x4A18 + 2 * j, rem2(r, 4));
            fx.random_table_vec(rec + 0x4A30 + 0x10 * j, 0.5);
        }
    }
    let d = sprite_desc(fx, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    plain_vp(fx, sp);
    let c = Fx::clamp_i(0xE5 - fx.mem.i32(D), 0, 0x20);
    let f20 = ee::div(ee::from_i32(c), 32.0);
    fx.mem.write_u32(sp + 0x220, 0);
    let (s2, s5) = (sp + 0x320, sp + 0x360);
    for k in 0..0x18u32 {
        let rec = D + 0x60 * k;
        if fx.mem.read_u8(rec + 0x4A10) != 0 {
            let age = fx.mem.i32(rec + 0x4A14);
            let sz = ee::mul(4.0, ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(0x18 - age)), 24.0)));
            v4(fx, sp + 0x310, ee::neg(sz), ee::neg(sz));
            v4(fx, s2, sz, sz);
            fx.vu_apply_to(sp + 0x310, sp + 0x2D0, sp + 0x310);
            fx.vu_apply_to(s2, sp + 0x2D0, s2);
            let v = fx.mem.i32(rec + 0x4A14) << 14;
            fx.mult(0x2AAA_AAAB, v);
            let q = (fx.hilo().1 >> 2) + ((v as u32) >> 31) as i32;
            let c = fx.table_trig(q, true);
            let a = fixu(fx, ee::mul(f20, ee::mul(128.0, c)), 0x01E0_12F0);
            colour(fx, sp + 0x3F0, [0x80, 0x80, 0x40, a as u8]);
            let age = fx.mem.i32(rec + 0x4A14);
            let f21 = fx.tan_falloff_p(age, 0x18, 1);
            fx.mem.write_u32(sp + 0x120, rec + 0x4A20);
            for j in 0..4u32 {
                fx.vu_scale4(sp + 0x3D0, rec + 0x4A30 + 0x10 * j, f21);
                fx.vu_add4(sp + 0x3D0, sp + 0x3D0, rec + 0x4A20);
                fx.vu_add4(sp + 0x350, sp + 0x310, sp + 0x3D0);
                fx.vu_add4(s5, s2, sp + 0x3D0);
                let b = i32::from(fx.mem.read_u16(rec + 0x4A18 + 2 * j));
                cell_uv(fx, sp, b & 3, b >> 2);
                if project(fx, sp, d) {
                    submit_w(fx, sp, d, 0x01E0_1448);
                }
            }
            let age = fx.mem.i32(rec + 0x4A14) + 1;
            fx.mem.set32(rec + 0x4A14, age);
            if age >= 0x19 {
                fx.mem.write_u8(rec + 0x4A10, 0);
            }
        }
        fx.mem.write_u32(sp + 0x220, k + 1);
    }
}

/// [F 0x1E014AC..0x1E01E38] update 0x3C: the flash D+0x5310 at node
/// 0x13's position D+0x59D0 (33 updates): a sprite, the band, 128
/// sparkles.
fn flash(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x3C {
        fx.mem.write_u8(D + 0x5310, 1);
        fx.mem.write_u32(D + 0x5314, 0);
        copy_n(fx, D + 0x59D0, D + 0x70, 4);
        for k in 0..0x80u32 {
            let r = fx.rand_from(D, 0x01E0_1500) >> 23;
            fx.mem.set8(D + 0x5950 + k, rem2(r, 4));
            fx.random_table_vec(D + 0x59E0 + 0x10 * k, f32::from_bits(0x3E99_999A));
        }
    }
    if fx.mem.read_u8(D + 0x5310) != 1 {
        return;
    }
    let (s2, s5) = (sp + 0x320, sp + 0x360);
    let d = sprite_desc(fx, sp);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
    flash_uv(fx, sp);
    fx.camera_vp_to(sp + 0x250);
    fx.translate_rows(sp + 0x290, sp + 0x2D0, D + 0x59D0);
    let m = fx.read_m4(sp + 0x290);
    fx.mul_into(Some(sp + 0x250), &m);
    let age = fx.mem.i32(D + 0x5314);
    let f0 = fx.tan_falloff_p(age, 0x20, 1);
    let a = fixu(fx, ee::mul(255.0, ee::sub(1.0, f0)), 0x01E0_168C);
    colour(fx, sp + 0x3F0, [0xFF, 0xC8, 0xB4, a as u8]);
    v4(fx, sp + 0x310, -6.0, -6.0);
    v4(fx, s2, 6.0, 6.0);
    if crate::fxlib2::project(fx, sp + 0x310, PROJ_OUT, sp + 0x250, d, sp + 0x50C) {
        far_z(fx, sp + 0x400, &[sp + 0x410]);
        fx.submit(d, OT, 0);
    }
    // The band: sp + 0x4B0 (8 registers 0x41414141, prim 0x14).
    let b = sp + 0x4B0;
    fx.mem.write_u16(b + 2, 0x14);
    fx.mem.write_u16(b, 8);
    fx.mem.write_u64(b + 8, 0x4141_4141);
    fx.mem.write_u32(b + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(b, a);
    fx.camera_vp_to(sp + 0x250);
    fx.translate_rows(sp + 0x290, sp + 0x2D0, D + 0x59D0);
    let age = fx.mem.i32(D + 0x5314);
    let f20 = fx.tan_falloff_p(age, 0x20, 2);
    fx.mem.setf(sp + 0x3D0, f20);
    fx.mem.setf(sp + 0x3D4, f20);
    fx.mem.write_u32(sp + 0x3D8, 0);
    fx.mem.write_u32(sp + 0x3DC, 0x3F80_0000);
    fx.scale_rows(sp + 0x290, sp + 0x3D0);
    let m = fx.read_m4(sp + 0x290);
    fx.mul_into(Some(sp + 0x250), &m);
    let age = fx.mem.i32(D + 0x5314);
    let al = ((0x20 - age).wrapping_mul(255)) >> 5;
    colour(fx, sp + 0x4C0, [0xFF, 0xC8, 0xB4, al as u8]);
    copy_n(fx, sp + 0x4D0, sp + 0x4C0, 2);
    colour(fx, sp + 0x4E0, [0, 0, 0x80, 0]);
    copy_n(fx, sp + 0x4F0, sp + 0x4E0, 2);
    for k in 0..0x20u32 {
        let (s1, fp) = (D + 0x30 * k, D + 0x30 * (k + 1));
        fx.mem.write_u32(sp + 0x180, fp + 0x5330);
        for (inner, outer, ra) in [(0x5320u32, 0x5330u32, 0x01E0_1914u32), (0x5340, 0x5330, 0x01E0_1A20)] {
            let r = fx.project_n(&[s1 + outer, fp + outer, s1 + inner, fp + inner], sp + 0x250, b, sp + 0x50C, ra);
            if r == 1 {
                far_z(fx, sp + 0x4C8, &[sp + 0x4D8, sp + 0x4E8, sp + 0x4F8]);
                fx.submit(b, OT, 0);
            }
        }
    }
    let d = sprite_desc(fx, sp);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
    plain_vp(fx, sp);
    let age = fx.mem.i32(D + 0x5314);
    let al = ((0x20 - age).wrapping_mul(255)) >> 5;
    colour(fx, sp + 0x3F0, [0xFF, 0xC8, 0xB4, al as u8]);
    v4(fx, sp + 0x310, -0.5, -0.5);
    v4(fx, s2, 0.5, 0.5);
    fx.vu_apply_to(sp + 0x310, sp + 0x2D0, sp + 0x310);
    fx.vu_apply_to(s2, sp + 0x2D0, s2);
    for k in 0..0x80u32 {
        let b = i32::from(fx.mem.read_u8(D + 0x5950 + k));
        cell_uv(fx, sp, b & 3, b >> 2);
        fx.vu_scale4(sp + 0x3D0, D + 0x59E0 + 0x10 * k, f20);
        fx.vu_add4(sp + 0x3D0, sp + 0x3D0, D + 0x59D0);
        fx.vu_add4(sp + 0x350, sp + 0x310, sp + 0x3D0);
        fx.vu_add4(s5, s2, sp + 0x3D0);
        if project(fx, sp, d) {
            far_z(fx, sp + 0x400, &[sp + 0x410]);
            fx.submit(d, OT, 0);
        }
    }
    let age = fx.mem.i32(D + 0x5314) + 1;
    fx.mem.set32(D + 0x5314, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + 0x5310, 0);
    }
}

/// The four turned corners (-s, -s), (s, -s), (-s, s), (s, s) at sp +
/// 0x310 / 0x320 / 0x330 / 0x340 through the camera turn rotated by
/// `angle` about z (sp + 0x390).
fn turned_quad(fx: &mut Fx, sp: u32, s: f32, angle: i32) {
    let n = ee::neg(s);
    v4(fx, sp + 0x310, n, n);
    v4(fx, sp + 0x320, s, n);
    v4(fx, sp + 0x330, n, s);
    v4(fx, sp + 0x340, s, s);
    copy_n(fx, sp + 0x390, sp + 0x2D0, 16);
    fx.rotate_axis(sp + 0x390, angle, 2);
    for o in [0x310u32, 0x320, 0x330, 0x340] {
        fx.vu_apply_to(sp + o, sp + 0x390, sp + o);
    }
}

/// alpha = (0.8 + 0.2 (age % 2)) x f20 x 255 sin(a).
fn trail_alpha(fx: &mut Fx, a: i32, age: i32, f20: f32, ra: u32) -> u8 {
    let s = fx.table_trig(a, false);
    let f3 = ee::mul(f20, ee::mul(255.0, s));
    let f0 = ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(rem2(age, 2))));
    fixu(fx, ee::mul(f0, f3), ra) as u8
}

/// A trail entry's two sprites: the turned quad (sp + 0x438, `big` x the
/// size) and the plain one (sp + 0x3E0, `small` x the size).
fn trail_entry(fx: &mut Fx, sp: u32, e: u32, f20: f32, sizes: [f32; 2], a: i32, ras: [u32; 4]) {
    let (s2, s5) = (sp + 0x320, sp + 0x360);
    let s = ee::mul(sizes[0], fx.mem.f(e + 8));
    let ang = fx.mem.i32(e + 0xC);
    turned_quad(fx, sp, s, ang);
    let age = fx.mem.i32(e + 4);
    let al = trail_alpha(fx, a, age, f20, ras[0]);
    colour(fx, sp + 0x448, [0xFF, 0xBE, 0x96, al]);
    copy_n(fx, sp + 0x3D0, e + 0x10, 4);
    fx.vu_add4(sp + 0x350, sp + 0x310, sp + 0x3D0);
    fx.vu_add4(s5, s2, sp + 0x3D0);
    fx.vu_add4(sp + 0x370, sp + 0x330, sp + 0x3D0);
    fx.vu_add4(sp + 0x380, sp + 0x340, sp + 0x3D0);
    if project(fx, sp, sp + 0x438) {
        submit_w(fx, sp, sp + 0x438, ras[1]);
    }
    let s = ee::mul(sizes[1], fx.mem.f(e + 8));
    v4(fx, sp + 0x310, ee::neg(s), ee::neg(s));
    v4(fx, s2, s, s);
    fx.vu_apply_to(sp + 0x310, sp + 0x2D0, sp + 0x310);
    fx.vu_apply_to(s2, sp + 0x2D0, s2);
    let age = fx.mem.i32(e + 4);
    let al = trail_alpha(fx, a, age, f20, ras[2]);
    colour(fx, sp + 0x3F0, [0xFF, 0xBE, 0x96, al]);
    copy_n(fx, sp + 0x3D0, e + 0x10, 4);
    fx.vu_add4(sp + 0x350, sp + 0x310, sp + 0x3D0);
    fx.vu_add4(s5, s2, sp + 0x3D0);
    if project(fx, sp, sp + 0x3E0) {
        submit_w(fx, sp, sp + 0x3E0, ras[3]);
    }
}

/// [F 0x1E01E38..0x1E02520] the streams' trail entries D+0x61E0 + 0x100 g
/// + 0x20 j (24 updates).
fn trails(fx: &mut Fx, sp: u32) {
    quad_desc(fx, sp);
    fx.mem.write_u32(sp + 0x190, sp + 0x43A);
    fx.mem.write_u32(sp + 0x1A0, sp + 0x440);
    fx.mem.write_u32(sp + 0x1B0, sp + 0x43C);
    for (k, o) in [(0x1C0u32, 0x452u32), (0x1D0, 0x462), (0x1E0, 0x472), (0x1F0, 0x482)] {
        fx.mem.write_u32(sp + k, sp + o);
    }
    trail_sprite_desc(fx, sp);
    let c = Fx::clamp_i(0xE5 - fx.mem.i32(D), 0, 0x20);
    let f20 = ee::div(ee::from_i32(c), 32.0);
    fx.mem.write_u32(sp + 0x210, 0);
    fx.mem.write_u32(sp + 0x130, D);
    for g in 0..0x30u32 {
        for j in 0..8u32 {
            let e = D + 0x100 * g + 0x20 * j + 0x61E0;
            if fx.mem.read_u8(e) == 0 {
                continue;
            }
            let v = fx.mem.i32(e + 4) << 15;
            fx.mult(0x2AAA_AAAB, v);
            let a = (fx.hilo().1 >> 2) + ((v as u32) >> 31) as i32;
            trail_entry(fx, sp, e, f20, [2.0, 1.0], a, [0x01E0_2268, 0x01E0_231C, 0x01E0_2424, 0x01E0_24B8]);
            let age = fx.mem.i32(e + 4) + 1;
            fx.mem.set32(e + 4, age);
            if age >= 0x19 {
                fx.mem.write_u8(e, 0);
            }
        }
        fx.mem.write_u32(sp + 0x130, D + 0x100 * (g + 1));
        fx.mem.write_u32(sp + 0x210, g + 1);
    }
}

/// [F 0x1E02520..0x1E02C48] the bursts' trail entries D+0x91E0 + 0x100 g
/// + 0x20 j (17 updates).
fn burst_trails(fx: &mut Fx, sp: u32) {
    quad_desc(fx, sp);
    trail_sprite_desc(fx, sp);
    let f20 = window_ramp(fx);
    fx.mem.write_u32(sp + 0x200, 0);
    fx.mem.write_u32(sp + 0x140, D);
    for g in 0..0x18u32 {
        for j in 0..8u32 {
            let e = D + 0x100 * g + 0x20 * j + 0x91E0;
            if fx.mem.read_u8(e) == 0 {
                continue;
            }
            let a = (fx.mem.i32(e + 4) << 15) >> 4;
            trail_entry(fx, sp, e, f20, [4.0, 3.0], a, [0x01E0_2960, 0x01E0_2A1C, 0x01E0_2B20, 0x01E0_2BBC]);
            let age = fx.mem.i32(e + 4) + 1;
            fx.mem.set32(e + 4, age);
            if age >= 0x11 {
                fx.mem.write_u8(e, 0);
            }
        }
        fx.mem.write_u32(sp + 0x140, D + 0x100 * (g + 1));
        fx.mem.write_u32(sp + 0x200, g + 1);
    }
}
