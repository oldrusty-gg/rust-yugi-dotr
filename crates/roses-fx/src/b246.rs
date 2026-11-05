//! BMODULE b246 (card 461), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xD370
//! bytes) [F]: a single unscheduled function in
//! b150's style (frame 0x410, s0 = D, s1 = the OT; the update count the
//! word D+0, counted at the end). Every update the camera turn with roll
//! (0x2055E0 into sp + 0x270). From update 0 a 33 x 33 floor grid (D+0x20,
//! 600 wide; textured gouraud quads through 0x100BA0, (0x80, 0xBE, 0xE4))
//! drawn twice, rippled 5 sin(phase + 0x14D t) in y, its ST scrolled by
//! 0.1 sin / cos; from 0x6E a 33 x 17 wall grid (D+0x5540, the curve
//! 0x28AAD0 x 160 / x -150 by the side) swept in from z 300 to -100 over
//! 250 updates with 1 - tan falloff cubed (0x100D50 p = 3, powf); 0x6F..
//! 0x167 one of 16 spark groups (+0x8120, 0x450) of 33 sprites up the
//! wall's column 10, 17 updates each; 0x14B..0x233 one of 8 bursts
//! (+0xC628, 0x1A8) of 32 screen sprites, 9 updates each; from 0x14A a
//! four-layer window flash (+0xC620) and from 500 a white window flash
//! (+0xD368, 0x61 updates; at 0x40 every other part is stopped). Returns 1
//! on 0x140, 6 on 0x14B..0x233, 2 on 0x234, 8 once the count reaches
//! 0x255. The siblings b399 (card 455) / b072 (card 476)
//! are this function with other timings / colours and a third window
//! flash ([`V`]); b446 (card 583) runs the same parts in another order
//! (bursts, layer flash, floor, wall, sparks) in a 0x450 frame.
use crate::blib::exe::{camera_vp_into, mul_mem};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The wall's profile (x, y, z, w rows) in the executable.
const CURVE: u32 = 0x0028_AAD0;
const FLOOR: u32 = D + 0x20;
const FLOOR_PH: u32 = D + 0x4430;
const FLOOR_ON: u32 = D + 0x5534;
const FLOOR_T: u32 = D + 0x5538;
const WALL: u32 = D + 0x5540;
const WALL_PH: u32 = D + 0x7850;
const WALL_ON: u32 = D + 0x8114;
const WALL_T: u32 = D + 0x8118;
const SPARKS: u32 = D + 0x8120;
const FLASH2: u32 = D + 0xC620;
const BURSTS: u32 = D + 0xC628;
const FLASH: u32 = D + 0xD368;
/// The siblings' third flash (on, t).
const FLASH3: u32 = D + 0xD370;

/// What differs between b246 and its siblings b399 / b072 [F fdiff].
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
    /// b246's return addresses moved one by one (b446's reordered parts).
    remap: &'static [(u32, u32)],
    /// The frame, and the parts' locals moved up by `frame - 0x410`.
    frame: u32,
    /// b446's order: white flash, bursts, layer flash, floor, wall, sparks.
    bursts_first: bool,
}

const B246: V = V { size: 0xD370, white_at: 500, grid_rgb: [0x80, 0xBE, 0xE4], wall_at: 0x6E, wall_len: (250.0, 0xFA), wall_ramp: 218.0, wall_dz: -400.0, sparks: (0x6F, 0x168), spark_rgb: [0x80, 0x94, 0xFF], bursts: (0x14B, 0x234), burst_rgb: [0x80, 0xB2, 0xFF], layer_rgb: [0x80, 0x94, 0xC8], flash3: None, ret1: 0x140, end: 0x255, shift: &[], remap: &[], frame: 0x410, bursts_first: false };
const B399: V = V { size: 0xD380, white_at: 0x18C, grid_rgb: [0xA0, 0xA0, 0xA0], wall_at: 0x38, wall_len: (300.0, 0x12C), wall_ramp: 268.0, wall_dz: -400.0, sparks: (0x39, 0x164), spark_rgb: [0x80, 0x80, 0x8A], bursts: (0x142, 0x1CC), burst_rgb: [0x80, 0x80, 0x8A], layer_rgb: [0x80, 0x80, 0x8A], flash3: Some((0x38, [0xFF, 0xE8, 0xC8], 0x01E0_1D8C)), ret1: 0x137, end: 0x1ED, shift: &[(0x01E0_0400, 0x10), (0x01E0_0900, 8), (0x01E0_0B00, 0), (0x01E0_0D00, -8), (0x01E0_1200, -0x10)], remap: &[], frame: 0x410, bursts_first: false };
const B072: V = V { size: 0xD380, white_at: 0x12C, grid_rgb: [0x80, 0x8C, 0xC8], wall_at: 0x58, wall_len: (185.0, 0xB9), wall_ramp: 153.0, wall_dz: -450.0, sparks: (0x59, 0x111), spark_rgb: [0x6C, 0x78, 0xC8], bursts: (0xE7, 0x16C), burst_rgb: [0x58, 0x64, 0xB4], layer_rgb: [0x58, 0x64, 0xB4], flash3: Some((0x58, [0xFF, 0xC8, 0xFF], 0x01E0_1D9C)), ret1: 0xDC, end: 0x18D, shift: &[], remap: &[], frame: 0x410, bursts_first: false };
const B446: V = V { size: 0xD380, white_at: 0x1CC, grid_rgb: [0x80, 0xDC, 0xC8], wall_at: 0x25, wall_len: (383.0, 0x17F), wall_ramp: 351.0, wall_dz: -400.0, sparks: (0x26, 0x1A4), spark_rgb: [0xBC, 0xC8, 0xC8], bursts: (0x182, 0x20C), burst_rgb: [0x80, 0xB4, 0xB4], layer_rgb: [0x80, 0xB4, 0xB4], flash3: Some((0x3A, [0xFF, 0xC8, 0xFF], 0x01E0_1E1C)), ret1: 0x177, end: 0x22D, shift: &[], remap: &[(0x01E0_0720, 0x01E0_103C), (0x01E0_0734, 0x01E0_1050), (0x01E0_095C, 0x01E0_129C), (0x01E0_0970, 0x01E0_12B0), (0x01E0_0B84, 0x01E0_14DC), (0x01E0_0E04, 0x01E0_177C), (0x01E0_0E18, 0x01E0_1790), (0x01E0_0F38, 0x01E0_18C8), (0x01E0_10EC, 0x01E0_1A7C), (0x01E0_144C, 0x01E0_0618), (0x01E0_1478, 0x01E0_0640), (0x01E0_14DC, 0x01E0_06A4), (0x01E0_152C, 0x01E0_06F4), (0x01E0_153C, 0x01E0_0704), (0x01E0_158C, 0x01E0_0754), (0x01E0_16A8, 0x01E0_0878), (0x01E0_1784, 0x01E0_0948), (0x01E0_18D0, 0x01E0_0AB0), (0x01E0_18FC, 0x01E0_0ADC), (0x01E0_1B20, 0x01E0_0CF0)], frame: 0x450, bursts_first: true };

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
    program_by(fx, param, &B246)
}

/// b399 (card 455): its entry 0x1E00080.
pub fn program_b399(fx: &mut Fx, param: i32) -> i32 {
    program_by(fx, param, &B399)
}

/// b446 (card 583): its entry 0x1E00080.
pub fn program_b446(fx: &mut Fx, param: i32) -> i32 {
    program_by(fx, param, &B446)
}

/// b072 (card 476): its entry 0x1E00080.
pub fn program_b072(fx: &mut Fx, param: i32) -> i32 {
    program_by(fx, param, &B072)
}

fn program_by(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let base = fx.enter(v.frame);
    // The parts' locals at b246's offsets from here [H for b446's low
    // pointer / counter slots, never read in these runs].
    let sp = base + (v.frame - 0x410);
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
            let v = phase(fx, 0x01E0_01B8);
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
            let v = phase(fx, 0x01E0_02A4);
            fx.mem.set32(WALL_PH + 0x44 * i + 4 * j, v);
        }
    }
    stop_parts(fx);
    fx.mem.write_u8(FLASH, 0);
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

/// The grids' descriptor at sp + 0x390 (9 registers, prim 0x24, ALPHA
/// preset 0, texture D+0x10, CLAMP 0).
fn grid_desc(fx: &mut Fx, sp: u32) -> u32 {
    let d = sp + 0x390;
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

/// The sprites' descriptor at sp + 0x338 (5 registers 0x42421, prim 0x26,
/// `or` into the flags), through the pointers the record keeps at sp +
/// 0x180 / 0x190 / 0x1A0.
fn sprite_desc(fx: &mut Fx, sp: u32, or: u16) -> u32 {
    let d = sp + 0x338;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | or);
    d
}

/// The grid matrix: the camera's view-projection x the identity moved by
/// the vector at sp + 0x2F0.
fn grid_matrix(fx: &mut Fx, sp: u32) {
    camera_vp_into(fx, sp + 0x1F0);
    fx.identity(sp + 0x230);
    fx.translate_rows(sp + 0x230, sp + 0x230, sp + 0x2F0);
    mul_mem(fx, sp + 0x1F0, sp + 0x230);
}

/// One grid quad (rows `a` / `b`, the next column 0x10 on) with ST (s0,
/// t0) .. (s1, t1) projected by 0x100BA0 and submitted at its w.
#[allow(clippy::too_many_arguments)]
fn quad(fx: &mut Fx, sp: u32, a: u32, b: u32, st: [f32; 4], ras: [u32; 2]) {
    let [s0, s1, t0, t1] = st;
    fx.mem.setf(sp + 0x3A8, s0);
    fx.mem.setf(sp + 0x3AC, t0);
    fx.mem.setf(sp + 0x3B8, s1);
    fx.mem.setf(sp + 0x3BC, t0);
    fx.mem.setf(sp + 0x3C8, s0);
    fx.mem.setf(sp + 0x3CC, t1);
    fx.mem.setf(sp + 0x3D8, s1);
    fx.mem.setf(sp + 0x3DC, t1);
    let vs = [a, a + 0x10, b, b + 0x10];
    if fx.project_n(&vs, sp + 0x1F0, sp + 0x390, sp + 0x40C, ras[0]) == 1 {
        let w = fx.mem.f(sp + 0x40C);
        let key = fixu(fx, w, ras[1]) as i32;
        fx.submit(sp + 0x390, OT, key);
    }
}

/// (k % 2) / 2 and (k % 2 + 1) / 2.
fn halves(k: i32) -> (f32, f32) {
    let v = rem2(k, 2);
    (ee::div(ee::from_i32(v), 2.0), ee::div(ee::from_i32(v + 1), 2.0))
}

fn step(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.camera_turn_roll(sp + 0x270);
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
    let d = sp + 0x308;
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
    colour(fx, sp + 0x318, [0xFF, 0xFF, 0xFF], a);
    xyz2(fx, sp + 0x320, 0x7000, 0x7000);
    xyz2(fx, sp + 0x328, 0x9000, 0x9000);
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
        fx.mem.write_u32(sp + 0x2F0, 0);
        fx.mem.setf(sp + 0x2F4, ee::sub(10.0, ee::mul(2.0, f2)));
        fx.mem.write_u32(sp + 0x2F8, 0);
        fx.mem.write_u32(sp + 0x2FC, 0x3F80_0000);
        grid_matrix(fx, sp);
        let a = if pass != 0 { fixu(fx, f23, v.ra(0x01E0_0734)) } else { fixu(fx, f22, v.ra(0x01E0_0720)) };
        colour(fx, sp + 0x3A0, v.grid_rgb, a & 0xFF);
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
    fx.mem.write_u32(sp + 0x2F0, 0);
    fx.mem.write_u32(sp + 0x2F4, 0xC0A0_0000);
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    let z = ee::mul(ee::add(300.0, ee::mul(v.wall_dz, f3)), side);
    fx.mem.setf(sp + 0x2F8, z);
    fx.mem.write_u32(sp + 0x2FC, 0x3F80_0000);
    grid_matrix(fx, sp);
    let a = fixu(fx, ee::mul(90.0, f20), v.ra(0x01E0_0B84));
    colour(fx, sp + 0x3A0, v.grid_rgb, a);
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
    fx.mem.write_u32(sp + 0x180, d + 2);
    fx.mem.write_u32(sp + 0x190, d + 8);
    fx.mem.write_u32(sp + 0x1A0, d + 4);
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
    fx.mem.write_u32(sp + 0x1C0, 0);
    for k in 0..16u32 {
        let g = SPARKS + 0x450 * k;
        fx.mem.write_u32(sp + 0x130, D + 0x450 * k);
        if fx.mem.read_u8(g) != 0 {
            spark_group(fx, sp, g, f20, v);
        }
        fx.mem.write_u32(sp + 0x1C0, k + 1);
    }
    fx.mem.write_u32(sp + 0x130, D + 0x450 * 16);
}

fn spark_group(fx: &mut Fx, sp: u32, g: u32, f20: f32, v: &V) {
    let d = sp + 0x338;
    let age = fx.mem.i32(g + 4);
    let f0 = sin16(fx, (age << 15) >> 4);
    let a = fixu(fx, ee::mul(f20, ee::mul(100.0, f0)), v.ra(0x01E0_10EC));
    colour(fx, sp + 0x348, v.spark_rgb, a);
    let age = fx.mem.i32(g + 4);
    let f1 = ee::mul(0.5, ee::from_i32(age));
    let s = ee::mul(17.0, ee::add(0.5, ee::div(f1, 16.0)));
    corners(fx, sp, s);
    let age = fx.mem.i32(g + 4);
    let f0 = fx.tan_falloff_p(age, 0x10, 1);
    let f21 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
    for j in 0..33u32 {
        fx.mem.write_u32(sp + 0x1D0, j);
        fx.mem.write_u32(sp + 0x110, g - 0x8120 + j);
        fx.mem.write_u32(sp + 0x120, g - 0x8120 + 0x10 * j);
        let pv = g + 0x30 + 0x10 * j;
        fx.mem.write_u32(sp + 0x100, pv);
        let b = i32::from(fx.mem.read_u8(g + 8 + j));
        let u = rem2(b, 4) << 5;
        let v = (b >> 2) << 5;
        uv(fx, sp + 0x350, u << 4);
        uv(fx, sp + 0x352, v << 4);
        uv(fx, sp + 0x360, (u + 0x20) << 4);
        uv(fx, sp + 0x362, (v + 0x20) << 4);
        fx.vu_scale4(sp + 0x2F0, g + 0x240 + 0x10 * j, f21);
        fx.vu_add4(sp + 0x2F0, sp + 0x2F0, pv);
        fx.vu_add4(sp + 0x2D0, sp + 0x2B0, sp + 0x2F0);
        fx.vu_add4(sp + 0x2E0, sp + 0x2C0, sp + 0x2F0);
        if crate::fxlib2::project(fx, sp + 0x2D0, PROJ_OUT, sp + 0x1F0, d, sp + 0x40C) {
            fx.submit(d, OT, 0);
        }
    }
    fx.mem.write_u32(sp + 0x110, g - 0x8120 + 33);
    fx.mem.write_u32(sp + 0x120, g - 0x8120 + 0x210);
    fx.mem.write_u32(sp + 0x1D0, 33);
    let n = fx.mem.i32(g + 4) + 1;
    fx.mem.set32(g + 4, n);
    if fx.mem.i32(g + 4) >= 0x11 {
        fx.mem.write_u8(g, 0);
    }
}

/// The sprite corners (s, s, 0, 1) / (-s, -s, 0, 1) at sp + 0x2B0 /
/// 0x2C0 turned by the camera (sp + 0x270).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    fx.mem.setf(sp + 0x2B0, s);
    fx.mem.setf(sp + 0x2B4, s);
    fx.mem.write_u32(sp + 0x2B8, 0);
    fx.mem.write_u32(sp + 0x2BC, 0x3F80_0000);
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x2C0, n);
    fx.mem.setf(sp + 0x2C4, n);
    fx.mem.write_u32(sp + 0x2C8, 0);
    fx.mem.write_u32(sp + 0x2CC, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x2B0, sp + 0x270, sp + 0x2B0);
    fx.vu_apply_to(sp + 0x2C0, sp + 0x270, sp + 0x2C0);
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
    fx.mem.write_u32(sp + 0x1B0, 0);
    for k in 0..8u32 {
        let b = BURSTS + 0x1A8 * k;
        fx.mem.write_u32(sp + 0x170, D + 0x1A8 * k);
        if fx.mem.read_u8(b) != 0 {
            burst(fx, sp, b, f21, v);
        }
        fx.mem.write_u32(sp + 0x1B0, k + 1);
    }
    fx.mem.write_u32(sp + 0x170, D + 0x1A8 * 8);
}

fn burst(fx: &mut Fx, sp: u32, b: u32, f21: f32, v: &V) {
    let d = sp + 0x338;
    let age = fx.mem.i32(b + 4);
    let f0 = sin16(fx, (age << 15) >> 3);
    let a = fixu(fx, ee::mul(f21, ee::mul(96.0, f0)), v.ra(0x01E0_16A8));
    colour(fx, sp + 0x348, v.burst_rgb, a);
    let age = fx.mem.i32(b + 4);
    let f20 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(age)), 8.0));
    let base = b - 0xC628;
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
        uv(fx, sp + 0x350, u << 4);
        uv(fx, sp + 0x360, (u + 0x20) << 4);
        if c < 4 {
            uv(fx, sp + 0x352, (w + 1) << 4);
            uv(fx, sp + 0x362, (w + 0x20) << 4);
        } else {
            uv(fx, sp + 0x352, (w - 1) << 4);
            uv(fx, sp + 0x362, (w + 0x21) << 4);
        }
        let ox = ee::from_i32(fx.mem.i32(b + 0xA8 + 8 * j));
        let x = fixs(fx, ee::add(256.0, ee::mul(ox, f20)), v.ra(0x01E0_18D0));
        fx.mem.set32(sp + 0x1EC, x);
        let oy = ee::from_i32(fx.mem.i32(b + 0xAC + 8 * j));
        let y = fixs(fx, ee::add(256.0, ee::mul(oy, f20)), v.ra(0x01E0_18FC));
        let x = fx.mem.i32(sp + 0x1EC);
        let h = |v: i32| (v.wrapping_add(0x700) << 4) as u16;
        xyz2(fx, sp + 0x358, h(x - s2), h(y - s2));
        xyz2(fx, sp + 0x368, h(x + s2), h(y + s2));
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
    colour(fx, sp + 0x348, v.layer_rgb, a);
    xyz2(fx, sp + 0x358, 0x7000, 0x7000);
    xyz2(fx, sp + 0x368, 0x9000, 0x9000);
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
        fx.mem.setf(sp + 0x350, f0);
        fx.mem.setf(sp + 0x354, f2);
        fx.mem.setf(sp + 0x360, ee::add(1.0, f0));
        fx.mem.setf(sp + 0x364, ee::add(1.0, f2));
        fx.submit(d, OT, 0);
        s2 += 0xA;
    }
    let t = fx.mem.i32(FLASH2 + 4) + 1;
    fx.mem.set32(FLASH2 + 4, t);
    if fx.mem.i32(FLASH2 + 4) >= 0x3201 {
        fx.mem.write_u8(FLASH2, 0);
    }
}

/// [F b399 0x1E01CE0 / b072 0x1E01CF0] The siblings' third window flash:
/// 128 x (1 - tan falloff of t over 0x20) in `rgb` for 0x21 updates, key 0.
fn third_flash(fx: &mut Fx, sp: u32, rgb: [u8; 3], ra: u32) {
    let d = sp + 0x308;
    fx.mem.write_u16(d + 2, 6);
    fx.mem.write_u16(d, 3);
    fx.mem.write_u64(d + 8, 0x441);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let t = fx.mem.i32(FLASH3 + 4);
    let f0 = fx.tan_falloff_p(t, 0x20, 1);
    let a = fixu(fx, ee::mul(128.0, ee::sub(1.0, f0)), ra);
    colour(fx, sp + 0x318, rgb, a);
    xyz2(fx, sp + 0x320, 0x7000, 0x7000);
    xyz2(fx, sp + 0x328, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
    let t = fx.mem.i32(FLASH3 + 4) + 1;
    fx.mem.set32(FLASH3 + 4, t);
    if fx.mem.i32(FLASH3 + 4) >= 0x21 {
        fx.mem.write_u8(FLASH3, 0);
    }
}
