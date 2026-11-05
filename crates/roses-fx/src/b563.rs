//! BMODULE b563 (card 130), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080 and its spawner 0x1E02180, data area
//! a0 = 0x1ED0000, 0x91E0 bytes) [F]: a single
//! unscheduled function in a 0x3E0 frame (D kept at sp + 0x140, s0 the
//! OT, the return at sp + 0xC0), the b014 / b290 family. Every update the
//! camera turn with roll (sp + 0x1D0) and the nodes 0xD / 0x1E / 0x31 /
//! 0x39 (D+0x30..), 0xF / 0x11 / 0x12 (D+0x80..), 0x27 (D+0xC0); the
//! target's point D+0xD0 (30 lowered). Bursts (4 x 4 records D+0x100 +
//! 0x980 j + 0x260 c, 32 sprites thrown 1.2 along random table directions
//! from node j, atlas UVs from 0x28AE70) spawned on 16 k + 0x2D / 0x2F /
//! 0x37 / 0x36 (below 0x9E / 0x90 / 0x98 / 0x97) and on 0xF3; dust (32 x
//! 0x100 at D+0x2700, 1..0x10D, twelve sprites from nodes 0xF / 0x11 /
//! 0x12); on 0x9B for 0x6E updates a band of eight 33-point rings
//! (D+0x4700) about the target turned by the side, with sin alphas
//! (powf(sin, 1)); 0x9C..0xE8 sparks (32 x 0x90, D+0x7890) round the
//! target; every tenth update there 32-strip rings (D+0x8EC0, the 33 x 2
//! points D+0x8AA0); on 0x6C a 32-triangle far-Z fan at node 0x27 (D+0x8FC0
//! points) for 0x82 updates. Returns 1 on 0x91, 6 on 0x9C..0xFC, 2 on 0xFD,
//! 8 once the count reaches 0x12F.
use crate::blib::exe::{mul_mem, translation_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70 / 0x205210's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x3E0;
/// The sprite descriptor (prim 0x26, 5 registers).
const DESC: u32 = 0x2A8;
/// The strip descriptor (prim 0x34, 12 registers).
const STRIP: u32 = 0x300;
/// The fan's triangle descriptor (prim 0x13, 6 registers).
const TRI: u32 = 0x390;
/// The direction table 0x28AE90 (4 x 32 vec3) and the UV table 0x28AE70.
const DIRS: u32 = 0x0028_AE90;
const UVS: u32 = 0x0028_AE70;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR: u64 = 0xFF_FFF8;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0x140, D);
    fx.mem.write_u32(sp + 0xC0, 0);
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

/// 0x1A2C70 with its residue frame.
fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// 0x1A7230 fabsf (its argument left at its sp).
fn fabsf(fx: &mut Fx, x: f32) -> f32 {
    fx.mem.write_f32(fx.sp - 0x10, x);
    f32::from_bits(x.to_bits() & 0x7FFF_FFFF)
}

/// Component `c` of a random direction of the table: rand % 4 x 384 +
/// rand % 32 x 12 (rand's frames saving `s0`).
fn dir(fx: &mut Fx, s0: u32, ra: [u32; 2], c: u32) -> f32 {
    let r = fx.rand_from(s0, ra[0]) >> 23;
    let i = rem2(r, 4);
    let r = fx.rand_from(s0, ra[1]) >> 23;
    let j = rem2(r, 32);
    fx.mem.f(DIRS.wrapping_add((i * 384 + j * 12) as u32) + 4 * c)
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

/// The sprite descriptor head (prim 0x26, 5 registers), textured by
/// `tex` (ALPHA preset 1, REGION_CLAMP 5).
fn sprite_desc(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
}

/// The strip descriptor (prim 0x34, 12 registers) textured by `tex`.
fn strip_desc(fx: &mut Fx, sp: u32, tex: u32, clamp: u64) {
    let d = sp + STRIP;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, clamp);
}

/// The camera's view-projection (x identity) at sp + 0x150.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x150);
    fx.identity(sp + 0x190);
    mul_mem(fx, sp + 0x150, sp + 0x190);
}

/// The view-projection at the target's point D+0xD0.
fn camera_at_target(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x150);
    translation_into(fx, sp + 0x190, D + 0xD0);
}

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1) at sp + 0x210 / 0x220 turned
/// by the camera.
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    vec4(fx, sp + 0x210, [n, n, 0.0, 1.0]);
    vec4(fx, sp + 0x220, [s, s, 0.0, 1.0]);
    fx.vu_apply_to(sp + 0x210, sp + 0x1D0, sp + 0x210);
    fx.vu_apply_to(sp + 0x220, sp + 0x1D0, sp + 0x220);
}

/// The sprite at sp + 0x290 (+ the corners) projected (0x204F70) and
/// drawn at the mean w.
fn sprite_draw(fx: &mut Fx, sp: u32, ra: u32) {
    fx.vu_add4(sp + 0x230, sp + 0x210, sp + 0x290);
    fx.vu_add4(sp + 0x240, sp + 0x220, sp + 0x290);
    if crate::fxlib2::project(fx, sp + 0x230, PROJ_OUT, sp + 0x150, sp + DESC, sp + 0x3DC) {
        let w = fx.mem.f(sp + 0x3DC);
        let key = fixu(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// A projected strip / triangle submitted at its mean w.
fn strip_submit(fx: &mut Fx, sp: u32, vs: &[u32], ra: u32, key_ra: u32) {
    if fx.project_n(vs, sp + 0x150, sp + STRIP, sp + 0x3DC, ra) == 1 {
        let w = fx.mem.f(sp + 0x3DC);
        let key = fixu(fx, w, key_ra);
        fx.submit(sp + STRIP, OT, key as i32);
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
    fx.bzero(D, 0x91E0);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for (k, (kind, id)) in [(5, 0), (5, 1), (5, 2), (4, 0xF)].into_iter().enumerate() {
        let r = fx.texture(kind, id);
        fx.mem.write_u32(D + 0x10 + 4 * k as u32, r);
    }
    for (slot, node, clear) in [(0x20u32, 0xD, 1), (0x24, 0x1E, 0), (0x28, 0x31, 0), (0x2C, 0x39, 0), (0x70, 0xF, 0), (0x74, 0x11, 0), (0x78, 0x12, 0), (0xB0, 0x27, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + slot, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0xD0);
    let y = ee::add(fx.mem.f(D + 0xD4), -30.0);
    fx.mem.setf(D + 0xD4, y);
    for j in 0..4u32 {
        fx.mem.write_u32(D + 0xE0 + 4 * j, 0);
        fx.mem.write_u32(D + 0xF0 + 4 * j, 0);
        for c in 0..4u32 {
            let r = D + 0x980 * j + 0x260 * c;
            fx.mem.write_u8(r + 0x100, 0);
            for k in 0..0x20u32 {
                let b = fx.rand_from(j, 0x01E0_02B0) >> 23;
                fx.mem.write_u16(r + 0x108 + 2 * k, rem2(b, 4) as u16);
            }
        }
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x2700 + 0x100 * k, 0);
    }
    // Eight rings of 33 x 3 points (D+0x4700 + 0x630 r).
    let m = sp + 0x250;
    for r in 0..8u32 {
        fx.identity(m);
        let a = ((r << 16) >> 3) as i32;
        fx.rotate_axis(m, if r < 4 { a } else { a + 0x1000 }, 1);
        for k in 0..0x21u32 {
            let p = D + 0x4700 + 0x630 * r + 0x30 * k;
            let x = ee::add(-25.0, ee::div(ee::mul(50.0, ee::from_i32(k as i32)), 32.0));
            fx.mem.setf(p, x);
            fx.mem.write_u32(p + 4, 0);
            let s = fx.tsin(((k << 16) >> 5) as i32);
            fx.mem.setf(p + 8, ee::mul(5.0, s));
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
            words(fx, p + 0x10, p, 4);
            let (_, q) = fx.div(k as i32, 6);
            let v = q << 16;
            fx.mult(0x2AAA_AAAB, v);
            let s = fx.tsin(v / 6);
            let s = fabsf(fx, s);
            let f20 = ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), s));
            let s = fx.tsin(((0x5555 * k) as i32) >> 5);
            fx.mem.setf(p + 0x14, ee::mul(ee::mul(100.0, s), f20));
            words(fx, p + 0x20, p + 0x10, 4);
            let y = ee::div(fx.mem.f(p + 0x24), 2.0);
            fx.mem.setf(p + 0x24, y);
            for o in [0u32, 0x10, 0x20] {
                fx.vu_apply_to(p + o, m, p + o);
            }
        }
    }
    fx.mem.write_u8(D + 0x7880, 0);
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x7890 + 0x90 * k, 0);
    }
    // 33 x 2 ring points (D+0x8AA0 + 0x20 k).
    fx.mem.write_u32(D + 0x8A90, 0);
    for k in 0..0x21u32 {
        let p = D + 0x8AA0 + 0x20 * k;
        let a = ((((k << 16) as i32) >> 5) - 0x4000) & 0xFFFF;
        let c = fx.tcos(a);
        fx.mem.setf(p, ee::mul(20.0, c));
        fx.mem.write_u32(p + 4, 0);
        let s = fx.tsin(a);
        fx.mem.setf(p + 8, ee::mul(20.0, s));
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        let b = ((0x8000 * k) as i32) >> 5;
        let s = fx.tsin(b);
        let f20 = ee::add(20.0, ee::mul(50.0, s));
        let c = fx.tcos(a);
        fx.mem.setf(p + 0x10, ee::mul(f20, c));
        let s = fx.tsin(b);
        fx.mem.setf(p + 0x14, ee::mul(30.0, s));
        let s = fx.tsin(a);
        fx.mem.setf(p + 0x18, ee::mul(f20, s));
        fx.mem.write_u32(p + 0x1C, 0x3F80_0000);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x8EC0 + 8 * k, 0);
    }
    // The fan's 33 points (D+0x8FC0 + 0x10 k).
    for k in 0..0x21u32 {
        let p = D + 0x8FC0 + 0x10 * k;
        let a = ((k << 16) as i32) >> 5;
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(20.0, s));
        let c = fx.tcos(a);
        fx.mem.setf(p + 4, ee::mul(20.0, c));
        fx.mem.write_u32(p + 8, 0);
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
    }
    fx.mem.write_u8(D + 0x91D0, 0);
    0
}

/// 0x1E02180(D, j) [F]: node j's next burst record (D+0x100 + 0x980 j +
/// 0x260 c, c = D+0xF0[j] round robin of 4): on, age 1, 32 directions
/// 1.2 x (x, |y|, z) of the table, from the node D+0x30 + 0x10 j;
/// D+0xE0[j] + 1.
fn spawn(fx: &mut Fx, j: u32) {
    fx.enter(0xB0);
    let c = fx.mem.read_u32(D + 0xF0 + 4 * j);
    let r = D + 0x980 * j + 0x260 * c;
    fx.mem.write_u8(r + 0x100, 1);
    fx.mem.write_u32(r + 0x104, 1);
    let k12 = f32::from_bits(0x3F99_999A);
    for i in 0..0x20u32 {
        let v = r + 0x160 + 0x10 * i;
        let x = dir(fx, D, [0x01E0_222C, 0x01E0_2260], 0);
        fx.mem.setf(v, ee::mul(k12, x));
        let y = dir(fx, D, [0x01E0_22C4, 0x01E0_22F8], 1);
        let y = fabsf(fx, y);
        fx.mem.setf(v + 4, ee::mul(k12, y));
        let z = dir(fx, D, [0x01E0_2364, 0x01E0_2398], 2);
        fx.mem.setf(v + 8, ee::mul(k12, z));
        fx.mem.write_u32(v + 0xC, 0x3F80_0000);
    }
    words(fx, r + 0x150, D + 0x30 + 0x10 * j, 4);
    let n = fx.mem.i32(D + 0xE0 + 4 * j) + 1;
    fx.mem.set32(D + 0xE0 + 4 * j, n);
    let c = fx.mem.i32(D + 0xF0 + 4 * j) + 1;
    fx.mem.set32(D + 0xF0 + 4 * j, c);
    if c >= 4 {
        fx.mem.write_u32(D + 0xF0 + 4 * j, 0);
    }
    fx.leave(0xB0);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x1D0);
    for k in 0..4u32 {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x20 + 4 * k));
        fx.node_position(h, s, D + 0x30 + 0x10 * k);
    }
    for k in 0..3u32 {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x70 + 4 * k));
        fx.node_position(h, s, D + 0x80 + 0x10 * k);
    }
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xB0));
    fx.node_position(h, s, D + 0xC0);
    for (j, (off, below)) in [(0x2D, 0x9E), (0x2F, 0x90), (0x37, 0x98), (0x36, 0x97)].into_iter().enumerate() {
        let n = fx.mem.i32(D);
        let c = fx.mem.i32(D + 0xE0 + 4 * j as u32);
        if n == (c << 4) + off && n < below {
            spawn(fx, j as u32);
        }
    }
    if fx.mem.i32(D) == 0xF3 {
        spawn(fx, 1);
        spawn(fx, 2);
    }
    bursts(fx, sp);
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x10E {
        let g = D + 0x100 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(g + 0x2700, 1);
        fx.mem.write_u32(g + 0x2704, 0);
        for p in 0..3u32 {
            words(fx, g + 0x2710 + 0x10 * p, D + 0x80 + 0x10 * p, 4);
            for s in 0..4u32 {
                let v = g + 0x2740 + 0x40 * p + 0x10 * s;
                let x = dir(fx, OT, [0x01E0_0C98, 0x01E0_0CCC], 0);
                fx.mem.setf(v, ee::mul(1.5, x));
                let y = dir(fx, OT, [0x01E0_0D10, 0x01E0_0D44], 1);
                let y = fabsf(fx, y);
                fx.mem.setf(v + 4, ee::mul(-2.0, y));
                let z = dir(fx, OT, [0x01E0_0D8C, 0x01E0_0DC0], 2);
                fx.mem.setf(v + 8, ee::mul(1.5, z));
                fx.mem.write_u32(v + 0xC, 0x3F80_0000);
            }
        }
    }
    dust(fx, sp);
    if fx.mem.i32(D) == 0x9B {
        fx.mem.write_u8(D + 0x7880, 1);
        fx.mem.write_u32(D + 0x7884, 0);
    }
    if fx.mem.read_u8(D + 0x7880) == 1 {
        band(fx, sp);
    }
    let n = fx.mem.i32(D);
    if (0x9C..0xE9).contains(&n) {
        let g = D + 0x90 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(g + 0x7890, 1);
        fx.mem.write_u32(g + 0x7894, 0);
        let m = sp + 0x250;
        fx.identity(m);
        let side = fx.mem.i32(D + 0xC);
        fx.rotate_axis(m, side.wrapping_mul(-0xAAA), 0);
        vec4(fx, sp + 0x290, [0.0, 100.0, 0.0, 1.0]);
        fx.vu_apply_to(sp + 0x290, m, sp + 0x290);
        for s in 0..8u32 {
            let v = g + 0x78A0 + 0x10 * s;
            let x = dir(fx, OT, [0x01E0_1590, 0x01E0_15C4], 0);
            fx.mem.setf(v, ee::mul(2.0, x));
            let y = dir(fx, OT, [0x01E0_1608, 0x01E0_163C], 1);
            fx.mem.setf(v + 4, ee::mul(2.0, y));
            let z = dir(fx, OT, [0x01E0_1680, 0x01E0_16B4], 2);
            fx.mem.setf(v + 8, ee::mul(2.0, z));
            fx.mem.write_u32(v + 0xC, 0x3F80_0000);
            fx.vu_add4(v, v, sp + 0x290);
        }
    }
    sparks(fx, sp);
    let n = fx.mem.i32(D);
    let (_, r) = fx.div(n, 10);
    if r == 0 && (0x9C..0xE9).contains(&n) {
        let c = fx.mem.read_u32(D + 0x8A90);
        fx.mem.write_u8(D + 0x8EC0 + 8 * c, 1);
        let c = fx.mem.read_u32(D + 0x8A90);
        fx.mem.write_u32(D + 0x8EC4 + 8 * c, 0);
        let c = fx.mem.i32(D + 0x8A90) + 1;
        fx.mem.set32(D + 0x8A90, c);
        if c >= 0x20 {
            fx.mem.write_u32(D + 0x8A90, 0);
        }
    }
    rings(fx, sp);
    if fx.mem.i32(D) == 0x6C {
        fx.mem.write_u8(D + 0x91D0, 1);
        fx.mem.write_u32(D + 0x91D4, 0);
    }
    if fx.mem.read_u8(D + 0x91D0) == 1 {
        fan(fx, sp);
    }
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x91 {
        ret = 1;
    } else {
        if (0x9C..0xFD).contains(&n) {
            ret = 6;
        }
        if n == 0xFD {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x12F {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// The bursts [F 0x1E008AC].
fn bursts(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
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
    camera(fx, sp);
    for j in 0..4u32 {
        for c in 0..4u32 {
            let r = D + 0x980 * j + 0x260 * c;
            if fx.mem.read_u8(r + 0x100) == 0 {
                continue;
            }
            let age = ee::from_i32(fx.mem.i32(r + 0x104));
            let s = ee::add(3.5, ee::div(ee::mul(3.5, age), 32.0));
            corners(fx, sp, s);
            let age = ee::from_i32(fx.mem.i32(r + 0x104));
            let f = Fx::ramp_above(age, 32.0, 16.0);
            let a = fixu(fx, ee::mul(64.0, f), 0x01E0_0A10);
            rgba(fx, sp + 0x2B8, [0x60, 0x60, 0x20], a as u8);
            let age = fx.mem.i32(r + 0x104);
            let f20 = fx.tan_falloff_p(age, 0x20, 1);
            for k in 0..0x20u32 {
                fx.vu_scale4(sp + 0x290, r + 0x160 + 0x10 * k, f20);
                fx.vu_add4(sp + 0x290, sp + 0x290, r + 0x150);
                let e = UVS + 8 * u32::from(fx.mem.read_u16(r + 0x108 + 2 * k));
                for (o, t) in [(0x2C0u32, 0u32), (0x2C2, 4), (0x2D0, 2), (0x2D2, 6)] {
                    let v = u32::from(fx.mem.read_u16(e + t));
                    uv(fx, sp + o, v);
                }
                sprite_draw(fx, sp, 0x01E0_0B84);
            }
            let age = fx.mem.i32(r + 0x104) + 1;
            fx.mem.set32(r + 0x104, age);
            if age >= 0x21 {
                fx.mem.write_u8(r + 0x100, 0);
            }
        }
    }
}

/// The whole-texture sprite UVs (the words 0, 0, 1.0, 1.0 at sp + 0x2C0 /
/// 0x2C4 / 0x2D0 / 0x2D4).
fn whole_uvs(fx: &mut Fx, sp: u32) {
    fx.mem.write_u32(sp + 0x2C0, 0);
    fx.mem.write_u32(sp + 0x2C4, 0);
    fx.mem.write_u32(sp + 0x2D0, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x2D4, 0x3F80_0000);
}

/// The dust [F 0x1E00E30].
fn dust(fx: &mut Fx, sp: u32) {
    sprite_desc(fx, sp, 0x14);
    whole_uvs(fx, sp);
    camera(fx, sp);
    for i in 0..0x20u32 {
        let g = D + 0x100 * i;
        if fx.mem.read_u8(g + 0x2700) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(g + 0x2704));
        let s = ee::add(4.0, ee::div(ee::mul(4.0, age), 32.0));
        corners(fx, sp, s);
        let age = ee::from_i32(fx.mem.i32(g + 0x2704));
        let f = Fx::ramp_above(age, 32.0, 0.0);
        let a = fixu(fx, ee::mul(255.0, f), 0x01E0_0F80);
        rgba(fx, sp + 0x2B8, [0xC4, 0xC4, 0x40], a as u8);
        let age = ee::from_i32(fx.mem.i32(g + 0x2704));
        let f20 = ee::div(age, 32.0);
        for p in 0..3u32 {
            for s in 0..4u32 {
                fx.vu_scale4(sp + 0x290, g + 0x2740 + 0x40 * p + 0x10 * s, f20);
                fx.vu_add4(sp + 0x290, sp + 0x290, g + 0x2710 + 0x10 * p);
                sprite_draw(fx, sp, 0x01E0_1040);
            }
        }
        let age = fx.mem.i32(g + 0x2704) + 1;
        fx.mem.set32(g + 0x2704, age);
        if age >= 0x21 {
            fx.mem.write_u8(g + 0x2700, 0);
        }
    }
}

/// The band of rings about the target [F 0x1E010EC].
fn band(fx: &mut Fx, sp: u32) {
    strip_desc(fx, sp, 0x18, 0);
    camera_at_target(fx, sp);
    let side = fx.mem.i32(D + 0xC);
    fx.rotate_axis(sp + 0x190, side.wrapping_mul(-0xAAA), 0);
    let age = ee::from_i32(fx.mem.i32(D + 0x7884));
    let f20 = Fx::ratio_below(age, 12.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x7884));
    let f0 = Fx::ratio_below(age, 12.0);
    let h = ee::add(0.5, ee::mul(0.5, f0));
    vec4(fx, sp + 0x290, [h, f20, h, 1.0]);
    fx.scale_rows(sp + 0x190, sp + 0x290);
    mul_mem(fx, sp + 0x150, sp + 0x190);
    let age = ee::from_i32(fx.mem.i32(D + 0x7884));
    let f20 = ee::mul(128.0, Fx::ramp_above(age, 110.0, 12.0));
    for r in 0..8u32 {
        let ring = D + 0x4700 + 0x630 * r;
        for k in 0..0x20u32 {
            let s = fx.tsin(((k << 15) as i32) >> 5);
            let p = fx.powf(s, 1.0);
            let a0 = fixu(fx, ee::mul(f20, p), 0x01E0_1258);
            rgba(fx, sp + 0x310, [0xFF, 0x80, 0x40], a0 as u8);
            let s = fx.tsin((((k + 1) << 15) as i32) >> 5);
            let p = fx.powf(s, 1.0);
            let a1 = fixu(fx, ee::mul(f20, p), 0x01E0_12A8);
            rgba(fx, sp + 0x328, [0xFF, 0x80, 0x40], a1 as u8);
            words(fx, sp + 0x340, sp + 0x310, 2);
            fx.mem.write_u8(sp + 0x343, 0);
            words(fx, sp + 0x358, sp + 0x328, 2);
            fx.mem.write_u8(sp + 0x35B, 0);
            let f21 = ee::div(ee::mul(5.0, ee::from_i32(k as i32)), 32.0);
            let n = fx.mem.i32(D);
            let dv = r as i32 + 8;
            let f24 = ee::div(ee::mul(5.0, ee::from_i32(k as i32 + 1)), 32.0);
            let (_, q) = fx.div(n, dv);
            let f22 = ee::div(ee::from_i32(q), ee::from_i32(dv));
            let f23 = ee::add(1.0, f22);
            let e0 = ring + 0x30 * k;
            let e1 = e0 + 0x30;
            for (o, v) in [(0x318u32, f21), (0x31C, f22), (0x330, f24), (0x334, f22), (0x348, f21), (0x34C, f23), (0x360, f24), (0x364, f23)] {
                fx.mem.setf(sp + o, v);
            }
            strip_submit(fx, sp, &[e0 + 0x20, e1 + 0x20, e0, e1], 0x01E0_13D8, 0x01E0_13EC);
            for (o, v) in [(0x318u32, f21), (0x31C, f23), (0x330, f24), (0x334, f23), (0x348, f21), (0x34C, f22), (0x360, f24), (0x364, f22)] {
                fx.mem.setf(sp + o, v);
            }
            strip_submit(fx, sp, &[e0 + 0x20, e1 + 0x20, e0 + 0x10, e1 + 0x10], 0x01E0_1460, 0x01E0_1474);
        }
    }
    let age = fx.mem.i32(D + 0x7884) + 1;
    fx.mem.set32(D + 0x7884, age);
    if age >= 0x6F {
        fx.mem.write_u8(D + 0x7880, 0);
    }
}

/// The sparks round the target [F 0x1E01720].
fn sparks(fx: &mut Fx, sp: u32) {
    sprite_desc(fx, sp, 0x14);
    whole_uvs(fx, sp);
    camera_at_target(fx, sp);
    mul_mem(fx, sp + 0x150, sp + 0x190);
    for i in 0..0x20u32 {
        let g = D + 0x90 * i;
        if fx.mem.read_u8(g + 0x7890) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(g + 0x7894));
        let s = ee::add(7.5, ee::div(ee::mul(7.5, age), 32.0));
        corners(fx, sp, s);
        let age = fx.mem.i32(g + 0x7894);
        let sn = fx.tsin((age << 15) >> 5);
        let a = fixu(fx, ee::mul(255.0, sn), 0x01E0_1878);
        rgba(fx, sp + 0x2B8, [0xC4, 0xC4, 0x40], a as u8);
        let age = fx.mem.i32(g + 0x7894);
        let f20 = fx.tan_falloff_p(age, 0x20, 1);
        for k in 0..8u32 {
            fx.vu_scale4(sp + 0x290, g + 0x78A0 + 0x10 * k, f20);
            sprite_draw(fx, sp, 0x01E0_1918);
        }
        let age = fx.mem.i32(g + 0x7894) + 1;
        fx.mem.set32(g + 0x7894, age);
        if age >= 0x21 {
            fx.mem.write_u8(g + 0x7890, 0);
        }
    }
}

/// The strip rings [F 0x1E01A44].
fn rings(fx: &mut Fx, sp: u32) {
    strip_desc(fx, sp, 0x1C, 0x1FC_012B_C8FA);
    for i in 0..0x20u32 {
        let g = D + 8 * i;
        if fx.mem.read_u8(g + 0x8EC0) == 0 {
            continue;
        }
        camera_at_target(fx, sp);
        let side = fx.mem.i32(D + 0xC);
        fx.rotate_axis(sp + 0x190, ((side + 1) >> 1) << 15, 1);
        let age = fx.mem.i32(g + 0x8EC4);
        let f0 = fx.tan_falloff_p(age, 0x20, 1);
        let h = ee::add(0.5, ee::mul(0.5, f0));
        vec4(fx, sp + 0x290, [h, f0, h, 1.0]);
        fx.scale_rows(sp + 0x190, sp + 0x290);
        mul_mem(fx, sp + 0x150, sp + 0x190);
        rgba(fx, sp + 0x310, [0xFF, 0x80, 0x40], 0);
        words(fx, sp + 0x328, sp + 0x310, 2);
        let k13 = f32::from_bits(0x3E08_0000);
        let base = f32::from_bits(0x3F0E_0000);
        for k in 0..0x20u32 {
            let s = fx.tsin(((k << 15) as i32) >> 5);
            let f2 = ee::mul(255.0, s);
            let age = fx.mem.i32(g + 0x8EC4);
            let a = fixu(fx, ee::div(ee::mul(ee::from_i32(0x20 - age), f2), 32.0), 0x01E0_1C04);
            rgba(fx, sp + 0x340, [0xFF, 0x80, 0x40], a as u8);
            words(fx, sp + 0x358, sp + 0x340, 2);
            let v = rem2(k as i32, 2);
            let f2 = ee::div(ee::mul(k13, ee::from_i32(v)), 2.0);
            let f0 = ee::div(ee::mul(k13, ee::from_i32(v + 1)), 2.0);
            let s0 = ee::add(base, f2);
            let s1 = ee::add(base, f0);
            for (o, x) in [(0x318u32, s0), (0x31C, 0.5), (0x330, s1), (0x334, 0.5), (0x348, s0), (0x34C, 0.0), (0x360, s1), (0x364, 0.0)] {
                fx.mem.setf(sp + o, x);
            }
            let e0 = D + 0x8AA0 + 0x20 * k;
            let e1 = e0 + 0x20;
            strip_submit(fx, sp, &[e0, e1, e0 + 0x10, e1 + 0x10], 0x01E0_1D00, 0x01E0_1D14);
        }
        let age = fx.mem.i32(g + 0x8EC4) + 1;
        fx.mem.set32(g + 0x8EC4, age);
        if age >= 0x21 {
            fx.mem.write_u8(g + 0x8EC0, 0);
        }
    }
}

/// The far-Z fan at node 0x27 [F 0x1E01DE0].
fn fan(fx: &mut Fx, sp: u32) {
    let d = sp + TRI;
    fx.mem.write_u16(d + 2, 0x13);
    fx.mem.write_u16(d, 6);
    fx.mem.write_u64(d + 8, 0x41_4141);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.camera_vp_to(sp + 0x150);
    fx.translate_rows(sp + 0x190, sp + 0x1D0, D + 0xC0);
    let age = fx.mem.i32(D + 0x91D4);
    let v = ee::from_i32(rem2(age, 2));
    let s = ee::sub(1.0, ee::mul(f32::from_bits(0x3DCC_CCCD), v));
    vec4(fx, sp + 0x290, [s, s, s, 1.0]);
    fx.scale_rows(sp + 0x190, sp + 0x290);
    mul_mem(fx, sp + 0x150, sp + 0x190);
    let age = ee::from_i32(fx.mem.i32(D + 0x91D4));
    let f20 = ee::mul(96.0, Fx::ratio_below(age, 32.0));
    let age = ee::from_i32(fx.mem.i32(D + 0x91D4));
    let f0 = Fx::ramp_above(age, 130.0, 98.0);
    let a = fixu(fx, ee::mul(f20, f0), 0x01E0_1F0C);
    rgba(fx, sp + 0x3A0, [0xFF, 0xFF, 0x80], a as u8);
    rgba(fx, sp + 0x3B0, [0xFF, 0xFF, 0x80], 0);
    words(fx, sp + 0x3C0, sp + 0x3B0, 2);
    vec4(fx, sp + 0x290, [0.0, 0.0, 0.0, 1.0]);
    for k in 0..0x20u32 {
        let p = D + 0x8FC0 + 0x10 * k;
        if fx.project_n(&[sp + 0x290, p, p + 0x10], sp + 0x150, d, sp + 0x3DC, 0x01E0_1FB0) == 1 {
            far_z(fx, sp + 0x3A8, &[sp + 0x3B8, sp + 0x3C8]);
            fx.submit(d, OT, 0);
        }
    }
    let age = fx.mem.i32(D + 0x91D4) + 1;
    fx.mem.set32(D + 0x91D4, age);
    if age >= 0x83 {
        fx.mem.write_u8(D + 0x91D0, 0);
    }
}
