//! BMODULE b565 (card 448), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xE590
//! bytes) [F]: b246.rs's b399 (card 455) path
//! copied by script (the project notes, every changed constant asserted
//! from the fdiff the project notes) - the D layout 0x10 on (a third
//! texture (5, 2) at D+0x18, the self unit's point (0x1F65D0 kind 1) at
//! D+0x20 every update), the frame 0x430 (its locals moved by
//! spmap.py's alignment), timings 0x98 later (wall 0xD0, sparks 0xD1..
//! 0x1FB, bursts 0x1DA..0x263, white flash 0x224, third flash 0xCD), greys
//! (0xC8 grids / layer, 0x8A sparks / bursts) - plus two blocks written
//! fresh: 32 mote records (D+0xD380, 0x90) spawned on 1..0x91 (8 cells,
//! random 3.0 vectors from the point), drawn 0x21 updates as 8 sprites
//! (texture D+0x18, CLAMP 5, keyed by their w) drawn in to the point by
//! tan falloff, alpha (0.7 + 0.3 (age % 2)) x 255 sin through soft double;
//! and the unit's added colour (0xFF, 0x46, 0x1C, 128 f (0.1 + 0.9 sin))
//! (0x1F77A0 mode 1) while f = clamp(n, 0, 0xA4) / 164 > 0 and n < 0xCD,
//! else cleared. Returns 1 on 0x1CF, 6 on 0x1DA..0x263, 2 on 0x264, 8 at
//! 0x285.
use crate::blib::exe::{camera_vp_into, mul_mem};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The wall's profile (x, y, z, w rows) in the executable.
const CURVE: u32 = 0x0028_AAD0;
const FLOOR: u32 = D + 0x30;
const FLOOR_PH: u32 = D + 0x4440;
const FLOOR_ON: u32 = D + 0x5544;
const FLOOR_T: u32 = D + 0x5548;
const WALL: u32 = D + 0x5550;
const WALL_PH: u32 = D + 0x7860;
const WALL_ON: u32 = D + 0x8124;
const WALL_T: u32 = D + 0x8128;
const SPARKS: u32 = D + 0x8130;
const FLASH2: u32 = D + 0xC630;
const BURSTS: u32 = D + 0xC638;
const FLASH: u32 = D + 0xD378;
/// The third flash (on, t).
const FLASH3: u32 = D + 0xE580;
/// 32 spark records of 0x90 (on, age, 8 cell bytes, 8 points +0x10).
const MOTES: u32 = D + 0xD380;
/// The unit's point (0x1F65D0 kind 1), every update.
const POINT: u32 = D + 0x20;

/// b246.rs's variant fields, kept for the copied parts (this record's values only).
struct V {
    size: u32,
    white_at: i32,
    grid_rgb: [u8; 3],
    wall_at: i32,
    /// The wall's sweep length (as f32 and the falloff's n), its ramp
    /// start and the z sweep.
    wall_len: (f32, i32),
    wall_ramp: f32,
    wall_dz: f32,
    sparks: (i32, i32),
    spark_rgb: [u8; 3],
    bursts: (i32, i32),
    burst_rgb: [u8; 3],
    layer_rgb: [u8; 3],
    /// The third window flash (start, colour, its 0x1A2C70's return).
    flash3: Option<(i32, [u8; 3], u32)>,
    ret1: i32,
    end: i32,
    /// (from, delta): the record's addresses from b246's (`from` and up).
    shift: &'static [(u32, i32)],
    /// b246's return addresses moved one by one to this record's.
    remap: &'static [(u32, u32)],
    /// The frame, and the parts' locals moved up by `frame - 0x410`.
    frame: u32,
    /// b446's order: white flash, bursts, layer flash, floor, wall, sparks.
    bursts_first: bool,
}

const B565: V = V { size: 0xE590, white_at: 0x224, grid_rgb: [0xC8, 0xC8, 0xC8], wall_at: 0xD0, wall_len: (300.0, 0x12C), wall_ramp: 268.0, wall_dz: -400.0, sparks: (0xD1, 0x1FC), spark_rgb: [0x8A, 0x8A, 0x8A], bursts: (0x1DA, 0x264), burst_rgb: [0x8A, 0x8A, 0x8A], layer_rgb: [0xC8, 0xC8, 0xC8], flash3: Some((0xCD, [0xFF, 0xE8, 0xC8], 0x01E0_2384)), ret1: 0x1CF, end: 0x285, shift: &[], remap: RA, frame: 0x430, bursts_first: false };

/// b246's return addresses moved to this record's [F].
const RA: &[(u32, u32)] = &[(0x01E0_04CC, 0x01E0_051C), (0x01E0_0720, 0x01E0_0770), (0x01E0_0734, 0x01E0_0784), (0x01E0_095C, 0x01E0_09A4), (0x01E0_0970, 0x01E0_09B8), (0x01E0_0B84, 0x01E0_0BC4), (0x01E0_0E04, 0x01E0_0E3C), (0x01E0_0E18, 0x01E0_0E50), (0x01E0_0F38, 0x01E0_0F70), (0x01E0_10EC, 0x01E0_1124), (0x01E0_144C, 0x01E0_147C), (0x01E0_1478, 0x01E0_14A8), (0x01E0_14DC, 0x01E0_150C), (0x01E0_152C, 0x01E0_155C), (0x01E0_153C, 0x01E0_156C), (0x01E0_158C, 0x01E0_15BC), (0x01E0_16A8, 0x01E0_16D8), (0x01E0_1784, 0x01E0_17AC), (0x01E0_18D0, 0x01E0_18F8), (0x01E0_18FC, 0x01E0_1924), (0x01E0_1B20, 0x01E0_1B48)];

impl V {
    /// The record's return address for b246's `a`.
    fn ra(&self, a: u32) -> u32 {
        if let Some(&(_, r)) = self.remap.iter().find(|&&(f, _)| f == a) {
            return r;
        }
        let d = self.shift.iter().rev().find(|&&(f, _)| a >= f).map_or(0, |&(_, d)| d);
        a.wrapping_add(d as u32)
    }
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    program_by(fx, param, &B565)
}

fn program_by(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let sp = fx.enter(v.frame);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx, v) } else { step(fx, sp, v) };
    fx.leave(v.frame);
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

/// (rand >> 23) % 256 << 8 (a 16-bit phase), rand's frame s0 = D.
fn phase(fx: &mut Fx, ra: u32) -> i32 {
    let r = fx.rand_from(D, ra) >> 23;
    (rem2(r, 256) << 16) >> 8
}

/// 0x1A2C70 with its residue frame.
fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// 0x1A2B80 with its residue frame.
fn fixs(fx: &mut Fx, f: f32, ra: u32) -> i32 {
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}

fn sin16(fx: &mut Fx, a: i32) -> f32 {
    fx.table_trig(a, false)
}

fn cos16(fx: &mut Fx, a: i32) -> f32 {
    fx.table_trig(a, true)
}

fn init(fx: &mut Fx, v: &V) -> i32 {
    fx.bzero(D, v.size);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    let r = fx.texture(5, 0);
    fx.mem.write_u32(D + 0x10, r);
    let r = fx.texture(5, 1);
    fx.mem.write_u32(D + 0x14, r);
    let r = fx.texture(5, 2);
    fx.mem.write_u32(D + 0x18, r);
    for i in 0..33u32 {
        let f20 = ee::div(ee::mul(600.0, ee::from_i32(i as i32)), 32.0);
        for j in 0..33u32 {
            let f2 = ee::mul(600.0, ee::from_i32(j as i32));
            let z = ee::sub(ee::div(f2, 32.0), 300.0);
            let x = ee::sub(f20, 300.0);
            let p = FLOOR + 0x210 * i + 0x10 * j;
            fx.mem.setf(p, x);
            fx.mem.write_u32(p + 4, 0);
            fx.mem.setf(p + 8, z);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
            let v = phase(fx, 0x01E0_01C8);
            fx.mem.set32(FLOOR_PH + 0x84 * i + 4 * j, v);
        }
    }
    fx.mem.write_u8(FLOOR_ON, 0);
    for i in 0..33u32 {
        let f20 = ee::div(ee::mul(600.0, ee::from_i32(i as i32)), 32.0);
        for j in 0..17u32 {
            let p = WALL + 0x110 * i + 0x10 * j;
            let c = CURVE + 0x10 * j;
            fx.mem.setf(p, ee::sub(f20, 300.0));
            let y = ee::mul(160.0, fx.mem.read_f32(c + 4));
            fx.mem.setf(p + 4, y);
            let side = ee::from_i32(fx.mem.i32(D + 0xC));
            let z = ee::mul(side, ee::mul(-150.0, fx.mem.read_f32(c + 8)));
            fx.mem.setf(p + 8, z);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
            let v = phase(fx, 0x01E0_02B4);
            fx.mem.set32(WALL_PH + 0x44 * i + 4 * j, v);
        }
    }
    stop_parts(fx);
    fx.mem.write_u8(FLASH, 0);
    for k in 0..32u32 {
        fx.mem.write_u8(MOTES + 0x90 * k, 0);
    }
    fx.mem.write_u8(FLASH3, 0);
    0
}

/// The wall, the spark groups, the window flash and the bursts stopped.
fn stop_parts(fx: &mut Fx) {
    fx.mem.write_u8(WALL_ON, 0);
    for k in 0..16u32 {
        fx.mem.write_u8(SPARKS + 0x450 * k, 0);
    }
    fx.mem.write_u8(FLASH2, 0);
    for k in 0..8u32 {
        fx.mem.write_u8(BURSTS + 0x1A8 * k, 0);
    }
}

/// An XYZ2 at `at`: x / y halfwords, z 0xFFFFF8, the top byte 0.
fn xyz2(fx: &mut Fx, at: u32, x: u16, y: u16) {
    fx.mem.write_u16(at, x);
    fx.mem.write_u16(at + 2, y);
    let q = fx.mem.read_u64(at);
    fx.mem.write_u64(at, (q & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFF8u64 << 32));
    fx.mem.write_u8(at + 7, 0);
}

/// A halfword UV field: the top two bits kept, `v` & 0x3FFF.
fn uv(fx: &mut Fx, at: u32, v: i32) {
    let h = fx.mem.read_u16(at);
    fx.mem.write_u16(at, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn colour(fx: &mut Fx, at: u32, rgb: [u8; 3], a: u32) {
    for (k, &c) in rgb.iter().enumerate() {
        fx.mem.write_u8(at + k as u32, c);
    }
    fx.mem.write_u8(at + 3, a as u8);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

/// The grids' descriptor at sp + 0x3B0 (9 registers, prim 0x24, ALPHA
/// preset 0, texture D+0x10, CLAMP 0).
fn grid_desc(fx: &mut Fx, sp: u32) -> u32 {
    let d = sp + 0x3B0;
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, (4u64 << 32) | 0x2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    let t = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, t, 1);
    fx.desc_clamp(d, 0);
    d
}

/// The sprites' descriptor at sp + 0x358 (5 registers 0x42421, prim 0x26,
/// `or` into the flags), through the pointers the record keeps at sp +
/// 0x190 / 0x1A0 / 0x1B0.
fn sprite_desc(fx: &mut Fx, sp: u32, or: u16) -> u32 {
    let d = sp + 0x358;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | or);
    d
}

/// The grid matrix: the camera's view-projection x the identity moved by
/// the vector at sp + 0x310.
fn grid_matrix(fx: &mut Fx, sp: u32) {
    camera_vp_into(fx, sp + 0x210);
    fx.identity(sp + 0x250);
    fx.translate_rows(sp + 0x250, sp + 0x250, sp + 0x310);
    mul_mem(fx, sp + 0x210, sp + 0x250);
}

/// One grid quad (rows `a` / `b`, the next column 0x10 on) with ST (s0,
/// t0) .. (s1, t1) projected by 0x100BA0 and submitted at its w.
#[allow(clippy::too_many_arguments)]
fn quad(fx: &mut Fx, sp: u32, a: u32, b: u32, st: [f32; 4], ras: [u32; 2]) {
    let [s0, s1, t0, t1] = st;
    fx.mem.setf(sp + 0x3C8, s0);
    fx.mem.setf(sp + 0x3CC, t0);
    fx.mem.setf(sp + 0x3D8, s1);
    fx.mem.setf(sp + 0x3DC, t0);
    fx.mem.setf(sp + 0x3E8, s0);
    fx.mem.setf(sp + 0x3EC, t1);
    fx.mem.setf(sp + 0x3F8, s1);
    fx.mem.setf(sp + 0x3FC, t1);
    let vs = [a, a + 0x10, b, b + 0x10];
    if fx.project_n(&vs, sp + 0x210, sp + 0x3B0, sp + 0x428, ras[0]) == 1 {
        let w = fx.mem.f(sp + 0x428);
        let key = fixu(fx, w, ras[1]) as i32;
        fx.submit(sp + 0x3B0, OT, key);
    }
}

/// (k % 2) / 2 and (k % 2 + 1) / 2.
fn halves(k: i32) -> (f32, f32) {
    let v = rem2(k, 2);
    (ee::div(ee::from_i32(v), 2.0), ee::div(ee::from_i32(v + 1), 2.0))
}

fn step(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.camera_turn_roll(sp + 0x290);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, POINT);
    if fx.mem.i32(D) == v.white_at {
        fx.mem.write_u8(FLASH, 1);
        fx.mem.set32(FLASH + 4, 0);
    }
    if fx.mem.read_u8(FLASH) == 1 {
        white_flash(fx, sp, v);
    }
    if v.bursts_first {
        bursts_layer(fx, sp, v);
    }
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(FLOOR_ON, 1);
        fx.mem.set32(FLOOR_T, 0);
    }
    if fx.mem.read_u8(FLOOR_ON) == 1 {
        floor(fx, sp, v);
    }
    if fx.mem.i32(D) == v.wall_at {
        fx.mem.write_u8(WALL_ON, 1);
        fx.mem.set32(WALL_T, 0);
    }
    if fx.mem.read_u8(WALL_ON) == 1 {
        wall(fx, sp, v);
    }
    let c = fx.mem.i32(D);
    if (v.sparks.0..v.sparks.1).contains(&c) {
        spawn_sparks(fx, c, v);
    }
    sparks(fx, sp, v);
    if !v.bursts_first {
        bursts_layer(fx, sp, v);
    }
    let c = fx.mem.i32(D);
    if c > 0 && c < 0x92 {
        spawn_motes(fx, c);
    }
    motes(fx, sp);
    tint(fx, sp);
    if let Some((at, rgb, ra)) = v.flash3 {
        if fx.mem.i32(D) == at {
            fx.mem.write_u8(FLASH3, 1);
            fx.mem.set32(FLASH3 + 4, 0);
        }
        if fx.mem.read_u8(FLASH3) == 1 {
            third_flash(fx, sp, rgb, ra);
        }
    }
    let c = fx.mem.i32(D);
    if c == v.ret1 {
        fx.mem.write_u32(sp + 0xC0, 1);
    } else {
        if (v.bursts.0..v.bursts.1).contains(&c) {
            fx.mem.write_u32(sp + 0xC0, 6);
        }
        if c == v.bursts.1 {
            fx.mem.write_u32(sp + 0xC0, 2);
        }
    }
    let n = fx.mem.i32(D) + 1;
    fx.mem.set32(D, n);
    if fx.mem.i32(D) >= v.end {
        fx.mem.write_u32(sp + 0xC0, 8);
    }
    fx.mem.i32(sp + 0xC0)
}

/// The bursts and the layered window flash.
fn bursts_layer(fx: &mut Fx, sp: u32, v: &V) {
    let c = fx.mem.i32(D);
    if (v.bursts.0..v.bursts.1).contains(&c) {
        spawn_burst(fx, c, v);
    }
    bursts(fx, sp, v);
    if fx.mem.i32(D) == v.bursts.0 - 1 {
        fx.mem.write_u8(FLASH2, 1);
        fx.mem.set32(FLASH2 + 4, 0);
    }
    if fx.mem.read_u8(FLASH2) == 1 {
        layer_flash(fx, sp, v);
    }
}

/// [F 0x1E003B4] The white window flash: 128 x (1 - tan falloff of t -
/// 0x40) x min(t / 64, 1) for 0x61 updates; at 0x40 the other parts end.
fn white_flash(fx: &mut Fx, sp: u32, v: &V) {
    if fx.mem.i32(FLASH + 4) == 0x40 {
        fx.mem.write_u8(FLOOR_ON, 0);
        stop_parts(fx);
    }
    let d = sp + 0x328;
    fx.mem.write_u16(d + 2, 6);
    fx.mem.write_u16(d, 3);
    fx.mem.write_u64(d + 8, 0x441);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let t = fx.mem.i32(FLASH + 4);
    let c = Fx::clamp_i(t - 0x40, 0, 0x20);
    let f0 = fx.tan_falloff_p(c, 0x20, 1);
    let f20 = ee::sub(1.0, f0);
    let t = fx.mem.i32(FLASH + 4);
    let f0 = Fx::ratio_below(ee::from_i32(t), 64.0);
    let a = fixu(fx, ee::mul(128.0, ee::mul(f0, f20)), v.ra(0x01E0_04CC));
    colour(fx, sp + 0x338, [0xFF, 0xFF, 0xFF], a);
    xyz2(fx, sp + 0x340, 0x7000, 0x7000);
    xyz2(fx, sp + 0x348, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
    let t = fx.mem.i32(FLASH + 4) + 1;
    fx.mem.set32(FLASH + 4, t);
    if fx.mem.i32(FLASH + 4) >= 0x61 {
        fx.mem.write_u8(FLASH, 0);
    }
}

/// [F 0x1E005D4] The floor: two passes (lifted 10 and 0) of 32 x 32
/// quads, alpha 60 / 128 x min(t / 64, 1) x the ramp out from 12736.
fn floor(fx: &mut Fx, sp: u32, v: &V) {
    grid_desc(fx, sp);
    let t = ee::from_i32(fx.mem.i32(FLOOR_T));
    let f20 = Fx::ratio_below(t, 64.0);
    let t = ee::from_i32(fx.mem.i32(FLOOR_T));
    let f0 = Fx::ramp_above(t, 12800.0, 12736.0);
    let f1 = ee::mul(f20, f0);
    fx.mem.write_u32(sp + 0xD0, 0);
    fx.mem.write_u32(sp + 0xE0, 0);
    fx.mem.write_u32(sp + 0xF0, 0);
    let f22 = ee::mul(60.0, f1);
    let f23 = ee::mul(128.0, f1);
    loop {
        let pass = fx.mem.i32(sp + 0xD0);
        let f2 = ee::mul(5.0, ee::from_i32(pass));
        fx.mem.write_u32(sp + 0x310, 0);
        fx.mem.setf(sp + 0x314, ee::sub(10.0, ee::mul(2.0, f2)));
        fx.mem.write_u32(sp + 0x318, 0);
        fx.mem.write_u32(sp + 0x31C, 0x3F80_0000);
        grid_matrix(fx, sp);
        let a = if pass != 0 { fixu(fx, f23, v.ra(0x01E0_0734)) } else { fixu(fx, f22, v.ra(0x01E0_0720)) };
        colour(fx, sp + 0x3C0, v.grid_rgb, a & 0xFF);
        let s4 = fx.mem.i32(sp + 0xE0) >> 1;
        for i in 0..33u32 {
            for j in 0..33u32 {
                let t = fx.mem.i32(FLOOR_T);
                let ph = fx.mem.i32(FLOOR_PH + 0x84 * i + 4 * j);
                let a = ph.wrapping_add(s4).wrapping_add(0x14D_i32.wrapping_mul(t)) & 0xFFFF;
                let f0 = sin16(fx, a);
                fx.mem.setf(FLOOR + 0x210 * i + 0x10 * j + 4, ee::mul(5.0, f0));
            }
        }
        let w = fx.mem.i32(FLOOR_T).wrapping_add(fx.mem.i32(sp + 0xF0));
        let a0 = (rem2(w, 128) << 16) >> 7;
        let sgn = (rem2(pass, 2) << 1) - 1;
        let s2 = sgn.wrapping_mul(a0) & 0xFFFF;
        let f24 = ee::mul(0.1, sin16(fx, s2));
        let f25 = ee::mul(0.1, cos16(fx, s2));
        for r in 0..32i32 {
            let (a, b) = halves(r);
            let (f20, f21) = (ee::add(a, f25), ee::add(b, f25));
            let row = FLOOR + 0x210 * r as u32;
            for c in 0..32i32 {
                let (a, b) = halves(c);
                let (s0, s1) = (ee::add(a, f24), ee::add(b, f24));
                let p = row + 0x10 * c as u32;
                quad(fx, sp, p, p + 0x210, [s0, s1, f20, f21], [v.ra(0x01E0_095C), v.ra(0x01E0_0970)]);
            }
        }
        let v = fx.mem.i32(sp + 0xE0).wrapping_add(0x3_3333);
        fx.mem.set32(sp + 0xE0, v);
        let v = fx.mem.i32(sp + 0xF0) + 0x20;
        fx.mem.set32(sp + 0xF0, v);
        let v = fx.mem.i32(sp + 0xD0) + 1;
        fx.mem.set32(sp + 0xD0, v);
        if fx.mem.i32(sp + 0xD0) >= 2 {
            break;
        }
    }
    let t = fx.mem.i32(FLOOR_T) + 1;
    fx.mem.set32(FLOOR_T, t);
    if fx.mem.i32(FLOOR_T) >= 0x3201 {
        fx.mem.write_u8(FLOOR_ON, 0);
    }
}

/// [F 0x1E00A44] The wall: swept from z 300 to -100 (by the side) over
/// 250 updates, its height 160 (curve + 0.04 sin) x (1 - tan falloff
/// cubed), alpha 90 x min(t / 32, 1) x the ramp out from 218.
fn wall(fx: &mut Fx, sp: u32, v: &V) {
    grid_desc(fx, sp);
    let t = ee::from_i32(fx.mem.i32(WALL_T));
    let f20 = Fx::ratio_below(t, 32.0);
    let t = ee::from_i32(fx.mem.i32(WALL_T));
    let f0 = Fx::ramp_above(t, v.wall_len.0, v.wall_ramp);
    let f20 = ee::mul(f20, f0);
    let f3 = ee::div(ee::from_i32(fx.mem.i32(WALL_T)), v.wall_len.0);
    fx.mem.write_u32(sp + 0x310, 0);
    fx.mem.write_u32(sp + 0x314, 0xC0A0_0000);
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    let z = ee::mul(ee::add(300.0, ee::mul(v.wall_dz, f3)), side);
    fx.mem.setf(sp + 0x318, z);
    fx.mem.write_u32(sp + 0x31C, 0x3F80_0000);
    grid_matrix(fx, sp);
    let a = fixu(fx, ee::mul(90.0, f20), v.ra(0x01E0_0B84));
    colour(fx, sp + 0x3C0, v.grid_rgb, a);
    let t = fx.mem.i32(WALL_T);
    let f20 = fx.tan_falloff_p(t, v.wall_len.1, 3);
    for i in 0..33u32 {
        for j in 0..17u32 {
            let ph = fx.mem.i32(WALL_PH + 0x44 * i + 4 * j);
            let t = fx.mem.i32(WALL_T);
            let a = ph.wrapping_add(0x14D_i32.wrapping_mul(t)) & 0xFFFF;
            let f0 = ee::mul(f32::from_bits(0x3D23_D70A), sin16(fx, a));
            let f1 = ee::add(f0, fx.mem.read_f32(CURVE + 0x10 * j + 4));
            let y = ee::mul(f20, ee::mul(160.0, f1));
            fx.mem.setf(WALL + 0x110 * i + 0x10 * j + 4, y);
        }
    }
    let t = fx.mem.i32(WALL_T);
    let s2 = ((rem2(t, 128) << 16) >> 7) & 0xFFFF;
    let t = fx.mem.i32(WALL_T);
    let (_, m) = fx.div(t, 0x18);
    let f20 = ee::div(ee::from_i32(m), 24.0);
    let f20 = ee::add(f20, ee::mul(0.1, sin16(fx, s2)));
    let f21 = ee::mul(f32::from_bits(0x3E4C_CCCD), cos16(fx, s2));
    for r in 0..32i32 {
        let (a, b) = halves(r);
        let (f22, f23) = (ee::add(a, f21), ee::add(b, f21));
        let row = WALL + 0x110 * r as u32;
        for c in 0..16i32 {
            let (a, b) = halves(c);
            let (s0, s1) = (ee::add(a, f20), ee::add(b, f20));
            let p = row + 0x10 * c as u32;
            quad(fx, sp, p, p + 0x110, [s0, s1, f22, f23], [v.ra(0x01E0_0E04), v.ra(0x01E0_0E18)]);
        }
    }
    let t = fx.mem.i32(WALL_T) + 1;
    fx.mem.set32(WALL_T, t);
    if fx.mem.i32(WALL_T) > v.wall_len.1 {
        fx.mem.write_u8(WALL_ON, 0);
    }
}

/// [F 0x1E00EA8] Spark group c % 16: the wall's column 10 (33 points),
/// speeds (rand >> 23) % 3 + 5 and random unit vectors (0x101080).
fn spawn_sparks(fx: &mut Fx, c: i32, v: &V) {
    let g = SPARKS + 0x450 * rem2(c, 16) as u32;
    fx.mem.write_u8(g, 1);
    fx.mem.set32(g + 4, 0);
    for k in 0..33u32 {
        for w in 0..4 {
            let v = fx.mem.read_u32(WALL + 0xA0 + 0x110 * k + 4 * w);
            fx.mem.write_u32(g + 0x30 + 0x10 * k + 4 * w, v);
        }
        let r = fx.rand_from(D, v.ra(0x01E0_0F38)) >> 23;
        let (_, m) = fx.div(r, 3);
        fx.mem.set8(g + 8 + k, m + 5);
        fx.random_table_vec(g + 0x240 + 0x10 * k, 1.0);
    }
}

/// [F 0x1E00F98] The spark groups: 33 camera-facing sprites (cells by the
/// speed byte) a group, size 17 (0.5 + age / 32), out along their vectors
/// by 0.2 + 0.8 tan falloff, alpha 100 sin x the in / out ramp, key 0.
fn sparks(fx: &mut Fx, sp: u32, v: &V) {
    let d = sprite_desc(fx, sp, 0x800);
    fx.mem.write_u32(sp + 0x190, d + 2);
    fx.mem.write_u32(sp + 0x1A0, d + 8);
    fx.mem.write_u32(sp + 0x1B0, d + 4);
    let t = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, t, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 0);
    grid_matrix(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::from_i32(Fx::clamp_i(n - (v.sparks.0 - 1), 0, 0x20));
    let n = fx.mem.i32(D);
    let f0 = ee::from_i32(Fx::clamp_i(v.sparks.1 - n, 0, 0x20));
    let f20 = ee::div(ee::mul(f20, f0), 1024.0);
    fx.mem.write_u32(sp + 0x1E0, 0);
    for k in 0..16u32 {
        let g = SPARKS + 0x450 * k;
        fx.mem.write_u32(sp + 0x130, D + 0x450 * k);
        if fx.mem.read_u8(g) != 0 {
            spark_group(fx, sp, g, f20, v);
        }
        fx.mem.write_u32(sp + 0x1E0, k + 1);
    }
    fx.mem.write_u32(sp + 0x130, D + 0x450 * 16);
}

fn spark_group(fx: &mut Fx, sp: u32, g: u32, f20: f32, v: &V) {
    let d = sp + 0x358;
    let age = fx.mem.i32(g + 4);
    let f0 = sin16(fx, (age << 15) >> 4);
    let a = fixu(fx, ee::mul(f20, ee::mul(100.0, f0)), v.ra(0x01E0_10EC));
    colour(fx, sp + 0x368, v.spark_rgb, a);
    let age = fx.mem.i32(g + 4);
    let f1 = ee::mul(0.5, ee::from_i32(age));
    let s = ee::mul(17.0, ee::add(0.5, ee::div(f1, 16.0)));
    corners(fx, sp, s);
    let age = fx.mem.i32(g + 4);
    let f0 = fx.tan_falloff_p(age, 0x10, 1);
    let f21 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
    for j in 0..33u32 {
        fx.mem.write_u32(sp + 0x1F0, j);
        fx.mem.write_u32(sp + 0x110, g - 0x8130 + j);
        fx.mem.write_u32(sp + 0x120, g - 0x8130 + 0x10 * j);
        let pv = g + 0x30 + 0x10 * j;
        fx.mem.write_u32(sp + 0x100, pv);
        let b = i32::from(fx.mem.read_u8(g + 8 + j));
        let u = rem2(b, 4) << 5;
        let v = (b >> 2) << 5;
        uv(fx, sp + 0x370, u << 4);
        uv(fx, sp + 0x372, v << 4);
        uv(fx, sp + 0x380, (u + 0x20) << 4);
        uv(fx, sp + 0x382, (v + 0x20) << 4);
        fx.vu_scale4(sp + 0x310, g + 0x240 + 0x10 * j, f21);
        fx.vu_add4(sp + 0x310, sp + 0x310, pv);
        fx.vu_add4(sp + 0x2F0, sp + 0x2D0, sp + 0x310);
        fx.vu_add4(sp + 0x300, sp + 0x2E0, sp + 0x310);
        if crate::fxlib2::project(fx, sp + 0x2F0, PROJ_OUT, sp + 0x210, d, sp + 0x428) {
            fx.submit(d, OT, 0);
        }
    }
    fx.mem.write_u32(sp + 0x110, g - 0x8130 + 33);
    fx.mem.write_u32(sp + 0x120, g - 0x8130 + 0x210);
    fx.mem.write_u32(sp + 0x1F0, 33);
    let n = fx.mem.i32(g + 4) + 1;
    fx.mem.set32(g + 4, n);
    if fx.mem.i32(g + 4) >= 0x11 {
        fx.mem.write_u8(g, 0);
    }
}

/// The sprite corners (s, s, 0, 1) / (-s, -s, 0, 1) at sp + 0x2D0 /
/// 0x2C0 turned by the camera (sp + 0x290).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    fx.mem.setf(sp + 0x2D0, s);
    fx.mem.setf(sp + 0x2D4, s);
    fx.mem.write_u32(sp + 0x2D8, 0);
    fx.mem.write_u32(sp + 0x2DC, 0x3F80_0000);
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x2E0, n);
    fx.mem.setf(sp + 0x2E4, n);
    fx.mem.write_u32(sp + 0x2E8, 0);
    fx.mem.write_u32(sp + 0x2EC, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x2D0, sp + 0x290, sp + 0x2D0);
    fx.vu_apply_to(sp + 0x2E0, sp + 0x290, sp + 0x2E0);
}

/// [F 0x1E013FC] Burst c % 8: 32 sprites with cells (rand >> 23) % 8,
/// sizes 0.2 + 0.8 (rand >> 23) % 256 / 256 and screen offsets -456 +
/// 912 (rand >> 23) % 256 / 256 in x and y.
fn spawn_burst(fx: &mut Fx, c: i32, v: &V) {
    let b = BURSTS + 0x1A8 * rem2(c, 8) as u32;
    fx.mem.write_u8(b, 1);
    fx.mem.set32(b + 4, 0);
    for j in 0..32u32 {
        let r = fx.rand_from(D, v.ra(0x01E0_144C)) >> 23;
        fx.mem.set8(b + 8 + j, rem2(r, 8));
        let r = fx.rand_from(D, v.ra(0x01E0_1478)) >> 23;
        let f2 = ee::from_i32(rem2(r, 256));
        let f = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), f2), 256.0));
        fx.mem.setf(b + 0x28 + 4 * j, f);
        for (o, ras) in [(0xA8u32, [0x01E0_14DCu32, 0x01E0_152C]), (0xAC, [0x01E0_153C, 0x01E0_158C])] {
            let r = fx.rand_from(D, v.ra(ras[0])) >> 23;
            let f2 = ee::from_i32(rem2(r, 256));
            let f12 = ee::add(-456.0, ee::div(ee::mul(912.0, f2), 256.0));
            let x = fixs(fx, f12, v.ra(ras[1]));
            fx.mem.set32(b + o + 8 * j, x);
        }
    }
}

/// [F 0x1E015B8] The bursts: 32 screen sprites a burst at (256, 256) +
/// the offsets x (0.2 + 0.8 age / 8), half size 200 (0.5 + age / 16) x
/// the size, alpha 96 sin x min((count - 0x14A) / 8, 1), key 0.
fn bursts(fx: &mut Fx, sp: u32, v: &V) {
    let d = sprite_desc(fx, sp, 0x800);
    let t = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, t, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 0);
    let n = fx.mem.i32(D);
    let f21 = ee::div(ee::from_i32(Fx::clamp_i(n - (v.bursts.0 - 1), 0, 8)), 8.0);
    fx.mem.write_u32(sp + 0x1D0, 0);
    for k in 0..8u32 {
        let b = BURSTS + 0x1A8 * k;
        fx.mem.write_u32(sp + 0x170, D + 0x1A8 * k);
        if fx.mem.read_u8(b) != 0 {
            burst(fx, sp, b, f21, v);
        }
        fx.mem.write_u32(sp + 0x1D0, k + 1);
    }
    fx.mem.write_u32(sp + 0x170, D + 0x1A8 * 8);
}

fn burst(fx: &mut Fx, sp: u32, b: u32, f21: f32, v: &V) {
    let d = sp + 0x358;
    let age = fx.mem.i32(b + 4);
    let f0 = sin16(fx, (age << 15) >> 3);
    let a = fixu(fx, ee::mul(f21, ee::mul(96.0, f0)), v.ra(0x01E0_16A8));
    colour(fx, sp + 0x368, v.burst_rgb, a);
    let age = fx.mem.i32(b + 4);
    let f20 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(age)), 8.0));
    let base = b - 0xC638;
    for j in 0..32u32 {
        fx.mem.write_u32(sp + 0x150, base + 4 * j);
        fx.mem.write_u32(sp + 0x160, base + j);
        let age = fx.mem.i32(b + 4);
        let f2 = ee::mul(0.5, ee::from_i32(age));
        let f1 = ee::mul(200.0, ee::add(0.5, ee::div(f2, 8.0)));
        let size = fx.mem.f(b + 0x28 + 4 * j);
        let s2 = fixs(fx, ee::mul(f1, size), v.ra(0x01E0_1784));
        let c = i32::from(fx.mem.read_u8(b + 8 + j));
        let u = rem2(c, 4) << 5;
        let w = (c >> 2) << 5;
        uv(fx, sp + 0x370, u << 4);
        uv(fx, sp + 0x380, (u + 0x20) << 4);
        if c < 4 {
            uv(fx, sp + 0x372, (w + 1) << 4);
            uv(fx, sp + 0x382, (w + 0x20) << 4);
        } else {
            uv(fx, sp + 0x372, (w - 1) << 4);
            uv(fx, sp + 0x382, (w + 0x21) << 4);
        }
        let ox = ee::from_i32(fx.mem.i32(b + 0xA8 + 8 * j));
        let x = fixs(fx, ee::add(256.0, ee::mul(ox, f20)), v.ra(0x01E0_18D0));
        fx.mem.set32(sp + 0x20C, x);
        let oy = ee::from_i32(fx.mem.i32(b + 0xAC + 8 * j));
        let y = fixs(fx, ee::add(256.0, ee::mul(oy, f20)), v.ra(0x01E0_18FC));
        let x = fx.mem.i32(sp + 0x20C);
        let h = |v: i32| (v.wrapping_add(0x700) << 4) as u16;
        xyz2(fx, sp + 0x378, h(x - s2), h(y - s2));
        xyz2(fx, sp + 0x388, h(x + s2), h(y + s2));
        fx.submit(d, OT, 0);
    }
    fx.mem.write_u32(sp + 0x150, base + 0x80);
    fx.mem.write_u32(sp + 0x160, base + 0x20);
    let n = fx.mem.i32(b + 4) + 1;
    fx.mem.set32(b + 4, n);
    if fx.mem.i32(b + 4) >= 9 {
        fx.mem.write_u8(b, 0);
    }
}

/// [F 0x1E01A9C] The window flash in four layers (ST scaled by (t % k) /
/// k for k = 16, 26, 36, 46, mirrored by the layer), alpha 64 x min(t /
/// 8, 1), for 0x3201 updates.
fn layer_flash(fx: &mut Fx, sp: u32, v: &V) {
    let d = sprite_desc(fx, sp, 0);
    let t = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, t, 1);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 0);
    let t = ee::from_i32(fx.mem.i32(FLASH2 + 4));
    let f0 = Fx::ratio_below(t, 8.0);
    let a = fixu(fx, ee::mul(64.0, f0), v.ra(0x01E0_1B20));
    colour(fx, sp + 0x368, v.layer_rgb, a);
    xyz2(fx, sp + 0x378, 0x7000, 0x7000);
    xyz2(fx, sp + 0x388, 0x9000, 0x9000);
    let mut s2 = 0x10;
    for s3 in 0..4i32 {
        let f0 = ee::from_i32(s2);
        let t = fx.mem.i32(FLASH2 + 4);
        let (_, m) = fx.div(t, s2);
        let f1 = ee::div(ee::from_i32(m), f0);
        let su = if rem2(s3, 2) == 0 { 1.0 } else { -1.0 };
        let sv = if s3 >> 1 == 0 { 1.0 } else { -1.0 };
        let f0 = ee::mul(su, f1);
        let f2 = ee::mul(sv, f1);
        fx.mem.setf(sp + 0x370, f0);
        fx.mem.setf(sp + 0x374, f2);
        fx.mem.setf(sp + 0x380, ee::add(1.0, f0));
        fx.mem.setf(sp + 0x384, ee::add(1.0, f2));
        fx.submit(d, OT, 0);
        s2 += 0xA;
    }
    let t = fx.mem.i32(FLASH2 + 4) + 1;
    fx.mem.set32(FLASH2 + 4, t);
    if fx.mem.i32(FLASH2 + 4) >= 0x3201 {
        fx.mem.write_u8(FLASH2, 0);
    }
}

/// [F 0x1E022DC] The third window flash (b399's):
/// 128 x (1 - tan falloff of t over 0x20) in `rgb` for 0x21 updates, key 0.
fn third_flash(fx: &mut Fx, sp: u32, rgb: [u8; 3], ra: u32) {
    let d = sp + 0x328;
    fx.mem.write_u16(d + 2, 6);
    fx.mem.write_u16(d, 3);
    fx.mem.write_u64(d + 8, 0x441);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let t = fx.mem.i32(FLASH3 + 4);
    let f0 = fx.tan_falloff_p(t, 0x20, 1);
    let a = fixu(fx, ee::mul(128.0, ee::sub(1.0, f0)), ra);
    colour(fx, sp + 0x338, rgb, a);
    xyz2(fx, sp + 0x340, 0x7000, 0x7000);
    xyz2(fx, sp + 0x348, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
    let t = fx.mem.i32(FLASH3 + 4) + 1;
    fx.mem.set32(FLASH3 + 4, t);
    if fx.mem.i32(FLASH3 + 4) >= 0x21 {
        fx.mem.write_u8(FLASH3, 0);
    }
}

/// [F 0x1E01D10] Mote record c % 32 on 1..0x91: 8 cells (rand >> 23) % 4
/// and 8 points, the unit's point + random 3.0 vectors (0x101080).
fn spawn_motes(fx: &mut Fx, c: i32) {
    let m = MOTES + 0x90 * rem2(c, 32) as u32;
    fx.mem.write_u8(m, 1);
    fx.mem.set32(m + 4, 0);
    for k in 0..8u32 {
        let r = fx.rand_from(D, 0x01E0_1D78) >> 23;
        fx.mem.set8(m + 8 + k, rem2(r, 4));
        let p = m + 0x10 + 0x10 * k;
        fx.random_table_vec(p, 3.0);
        fx.vu_add4(p, p, POINT);
    }
}

/// [F 0x1E01DD8] The motes: 8 camera-facing sprites a record (texture
/// D+0x18, CLAMP 5), half size 10 (0.2 + 0.8 age / 32), drawn in to the
/// point by the tan falloff of 0x20 - age, alpha (0.7 + 0.3 (age % 2)) x
/// 255 sin(age 0x200) (soft double 0x1A2C30 / 0x1A3118 / 0x1A31D8 /
/// 0x1A3950), keyed by their w; 0x21 updates. The sprite pointers at sp +
/// 0x190 / 0x1A0 / 0x1B0 are the sparks'; the record pointer sp + 0x180,
/// the count sp + 0x1C0.
fn motes(fx: &mut Fx, sp: u32) {
    let d = sprite_desc(fx, sp, 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let t = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, t, 1);
    fx.desc_clamp(d, 5);
    camera_vp_into(fx, sp + 0x210);
    fx.identity(sp + 0x250);
    mul_mem(fx, sp + 0x210, sp + 0x250);
    fx.mem.write_u32(sp + 0x1C0, 0);
    fx.mem.write_u32(sp + 0x180, D);
    for k in 0..32u32 {
        let p = fx.mem.read_u32(sp + 0x180);
        if fx.mem.read_u8(p + 0xD380) != 0 {
            mote(fx, sp, p);
        }
        fx.mem.write_u32(sp + 0x180, p + 0x90);
        fx.mem.write_u32(sp + 0x1C0, k + 1);
    }
}

fn mote(fx: &mut Fx, sp: u32, p: u32) {
    let d = sp + 0x358;
    let age = fx.mem.i32(p + 0xD384);
    let f2 = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(age)), 32.0);
    let s = ee::mul(10.0, ee::add(f32::from_bits(0x3E4C_CCCD), f2));
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x2D0, n);
    fx.mem.setf(sp + 0x2D4, n);
    fx.mem.write_u32(sp + 0x2D8, 0);
    fx.mem.write_u32(sp + 0x2DC, 0x3F80_0000);
    fx.mem.setf(sp + 0x2E0, s);
    fx.mem.setf(sp + 0x2E4, s);
    fx.mem.write_u32(sp + 0x2E8, 0);
    fx.mem.write_u32(sp + 0x2EC, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x2D0, sp + 0x290, sp + 0x2D0);
    fx.vu_apply_to(sp + 0x2E0, sp + 0x290, sp + 0x2E0);
    let age = fx.mem.i32(p + 0xD384);
    let f0 = sin16(fx, (age << 14) >> 5);
    let s2 = f64::from(ee::mul(255.0, f0));
    let age = fx.mem.i32(p + 0xD384);
    let f = ee::mul(f32::from_bits(0x3E99_999A), ee::from_i32(rem2(age, 2)));
    let a = ((0.7f64 + f64::from(f)) * s2) as u32;
    colour(fx, sp + 0x368, [0x44, 0x62, 0xFF], a);
    let age = fx.mem.i32(p + 0xD384);
    let f20 = fx.tan_falloff_p(0x20 - age, 0x20, 1);
    for j in 0..8u32 {
        let v = p + 0xD390 + 0x10 * j;
        fx.vu_sub4(sp + 0x310, v, POINT);
        fx.vu_scale4(sp + 0x310, sp + 0x310, f20);
        fx.vu_add4(sp + 0x310, sp + 0x310, POINT);
        fx.vu_add4(sp + 0x2F0, sp + 0x2D0, sp + 0x310);
        fx.vu_add4(sp + 0x300, sp + 0x2E0, sp + 0x310);
        let b = i32::from(fx.mem.read_u8(p + 0xD388 + j));
        let u = rem2(b, 4) << 5;
        let w = (b >> 2) << 5;
        uv(fx, sp + 0x370, u << 4);
        uv(fx, sp + 0x372, w << 4);
        uv(fx, sp + 0x380, (u + 0x20) << 4);
        uv(fx, sp + 0x382, (w + 0x20) << 4);
        if crate::fxlib2::project(fx, sp + 0x2F0, PROJ_OUT, sp + 0x210, d, sp + 0x428) {
            let w = fx.mem.f(sp + 0x428);
            let key = fixu(fx, w, 0x01E0_2144) as i32;
            fx.submit(d, OT, key);
        }
    }
    let n = fx.mem.i32(p + 0xD384) + 1;
    fx.mem.set32(p + 0xD384, n);
    if fx.mem.i32(p + 0xD384) >= 0x21 {
        fx.mem.write_u8(p + 0xD380, 0);
    }
}

/// [F 0x1E021E0] The self unit's added colour (0x1F77A0 mode 1): (0xFF,
/// 0x46, 0x1C, 128 f (0.1 + 0.9 sin((n % 32) 0x400))), f = clamp(n, 0,
/// 0xA4) / 164, while f > 0 and n < 0xCD; else cleared.
fn tint(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(n, 0, 0xA4)), 164.0);
    if f20 > 0.0 && fx.mem.i32(D) < 0xCD {
        let n = fx.mem.i32(D);
        let f0 = sin16(fx, (rem2(n, 32) << 15) >> 5);
        let f2 = ee::mul(f32::from_bits(0x3F66_6666), f0);
        let f = ee::mul(ee::mul(128.0, f20), ee::add(f32::from_bits(0x3DCC_CCCD), f2));
        let a = fixu(fx, f, 0x01E0_2294);
        fx.mem.write_u8(sp + 0x42E, 0x1C);
        fx.mem.write_u8(sp + 0x42D, 0x46);
        fx.mem.write_u8(sp + 0x42C, 0xFF);
        fx.mem.write_u8(sp + 0x42F, a as u8);
        let h = fx.mem.i32(D + 4);
        fx.unit_add_mode(h, 1, sp + 0x42C, [u64::from(D), u64::from(OT)], 0x01E0_22C0);
    } else {
        let h = fx.mem.i32(D + 4);
        fx.unit_add_mode(h, 1, 0, [u64::from(D), u64::from(OT)], 0x01E0_22D8);
    }
}
