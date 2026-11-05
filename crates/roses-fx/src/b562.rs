//! BMODULE b562 (card 355), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000 kept in
//! s2, 0x24A0 bytes) [F]: a single unscheduled
//! function in a 0x3B0 frame on b150's helpers (s0 = the OT, the return at
//! sp + 0xB0): window sprites (descriptor sp + 0x2C0: 5 registers 0x42421,
//! prim 0x26 | 0x800, clamp 5) and textured bands (sp + 0x318: 12
//! registers 0x4214_2142_1421, prim 0x34, 0x100BA0). The init lays out
//! three rings (17 points of radius 30 at D+0x880 with y 1 / -1 copies,
//! 9 of radius 20 at D+0x11B0 with y 7.5 / -7.5, 9 half circles of 15
//! (z offset 15) at D+0x2370 with z + 15 copies) and two ST tables
//! (D+0xBB0 / D+0x1360: cell columns of a 4 x 4 texture, rows 0.5 / 0.625
//! / 0.75). Every update the camera turn with roll (sp + 0x210), the
//! attacker's node 2 (D+0x40, last update's D+0x50); the target's position
//! D+0x20. On 0xB..0x86 one of 32 swirl records (+0x70, 0x40) of 16
//! sprites turning round the node (radius 50 n' / 64 out by age / 32,
//! sizes 10 n' / 64 (0.2 + 0.8 age / 32), n' the record count to 64, UVs
//! from the table 0x28AE70, alpha 64 sin((age << 16) >> 5), grey). On
//! every fourth update of 0x29..0x86 one of 16 tube records (+0xFB0, 0x20)
//! at the node 15 up: 16 band pieces of the radius-30 ring (scale 0.2 +
//! 0.8 age / 64, raised by -24 tan falloff, spun by +8 += 0x1555; alphas
//! 64 sin((age << 15) >> 6) sin(piece angle), 64 updates; texture 4 / 15,
//! clamp 0x2FC_813FC81A). On 0x92..0x99 one of 16 fan records (+0x1560,
//! 0x60) of four 9-point bands (radius 20) turned to the node's motion
//! (atan2f) at the node pushed by table directions and 4 k x the side, 16
//! updates (scale 0.5 + 0.5 age / 16, alphas 64 cos((age << 14) >> 4)
//! sin(piece)). On update 0x9B a burst (D+0x1B60) of 128 table directions
//! (2.0) at the target, drawn 8 times along each (sprite 1, (60, 80, FF),
//! alpha 255 (1 - tan falloff) sin, 24 updates) and a dome (D+0x2490) of 4
//! x 8 turned copies of the half circles at the target (scale (1, 0.2 +
//! 0.8 tan falloff), alpha (k + 1) 255 (1 - tan falloff) / 4, (40, 40,
//! FF), 48 updates). Returns 1 on update 0x91, 2 on 0x9B, 8 once the count
//! reaches 0xCC.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, M4, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The UV table the swirls read (4 halfwords an entry).
const UV_TABLE: u32 = 0x0028_AE70;
/// 0x101080's direction table (4 x 32 vec3).
const DIRS: u32 = 0x0028_AE90;
const FRAME: u32 = 0x3B0;
const DESC: u32 = 0x2C0;
const BAND: u32 = 0x318;
const BAND_REGS: u64 = 0x4214_2142_1421;
const BAND_CLAMP: u64 = 0x2FC_813F_C81A;
const SWIRLS: u32 = D + 0x70;
const RING30: u32 = D + 0x880;
const ST_A: u32 = D + 0xBB0;
const TUBES: u32 = D + 0xFB0;
const RING20: u32 = D + 0x11B0;
const ST_B: u32 = D + 0x1360;
const FANS: u32 = D + 0x1560;
const BURST: u32 = D + 0x1B60;
const HALF15: u32 = D + 0x2370;
const DOME: u32 = D + 0x2490;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
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

fn words(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn words2(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..2u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

/// A ring point (r sin, 0, r cos, 1) at `p + 0x10` with copies at `p` /
/// `p + 0x20` whose y is `y` / `-y` (`a` the 16-bit angle).
fn ring_point(fx: &mut Fx, p: u32, a: i32, r: f32, y: u32) {
    let s = fx.tsin(a);
    fx.mem.setf(p + 0x10, ee::mul(r, s));
    fx.mem.write_u32(p + 0x14, 0);
    let c = fx.tcos(a);
    fx.mem.setf(p + 0x18, ee::mul(r, c));
    fx.mem.write_u32(p + 0x1C, 0x3F80_0000);
    words(fx, p, p + 0x10);
    fx.mem.write_u32(p + 4, y);
    words(fx, p + 0x20, p + 0x10);
    fx.mem.write_u32(p + 0x24, y ^ 0x8000_0000);
}

/// The ST table entry at `p`: column `t` of 4 over rows 0.5 / 0.625 /
/// 0.75 (two pieces of four ST pairs).
fn st_entry(fx: &mut Fx, p: u32, t: i32) {
    let c = rem2(t, 4);
    let s0 = ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(c)), 4.0));
    fx.mem.setf(p, s0);
    fx.mem.write_u32(p + 4, 0x3F00_0000);
    let s1 = ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(c + 1)), 4.0));
    fx.mem.setf(p + 8, s1);
    let order: [(u32, Option<u32>); 13] = [
        (0xC, Some(4)),
        (0x10, Some(0)),
        (0x14, None),
        (0x18, Some(8)),
        (0x1C, Some(0x14)),
        (0x20, Some(0)),
        (0x24, None),
        (0x28, Some(8)),
        (0x2C, Some(0x24)),
        (0x30, Some(0x20)),
        (0x34, Some(0x14)),
        (0x38, Some(0x28)),
        (0x3C, Some(0x34)),
    ];
    for (o, from) in order {
        match from {
            Some(f) => {
                let w = fx.mem.read_u32(p + f);
                fx.mem.write_u32(p + o, w);
            }
            None => fx.mem.write_u32(p + o, if o == 0x14 { 0x3F20_0000 } else { 0x3F40_0000 }),
        }
    }
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x24A0);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for (k, (kind, id)) in [(5, 0), (4, 0xF), (5, 1)].into_iter().enumerate() {
        let r = fx.texture(kind, id);
        fx.mem.write_u32(D + 0x10 + 4 * k as u32, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 2, 1);
    fx.mem.set32(D + 0x30, r);
    fx.mem.write_u32(D + 0x60, 0);
    for k in 0..0x20u32 {
        fx.mem.write_u8(SWIRLS + 0x40 * k, 0);
    }
    fx.mem.write_u32(D + 0x870, 0);
    for k in 0..0x11u32 {
        ring_point(fx, RING30 + 0x30 * k, ((k << 16) as i32) >> 4, 30.0, 0x3F80_0000);
    }
    for t in 0..0x10 {
        st_entry(fx, ST_A + 0x40 * t as u32, t);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(TUBES + 0x20 * k, 0);
    }
    for k in 0..9u32 {
        ring_point(fx, RING20 + 0x30 * k, (0xAAAA * k as i32) >> 3, 20.0, 0x40F0_0000);
    }
    for t in 0..8 {
        st_entry(fx, ST_B + 0x40 * t as u32, t);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(FANS + 0x60 * k, 0);
    }
    fx.mem.write_u8(BURST, 0);
    fx.mem.write_u8(DOME, 0);
    for k in 0..9u32 {
        let p = HALF15 + 0x20 * k;
        let a = ((0x8000 * k) as i32) >> 3;
        let c = fx.tcos(a);
        fx.mem.write_u32(p, 0);
        fx.mem.setf(p + 4, ee::add(15.0, ee::mul(15.0, c)));
        let s = fx.tsin(a);
        fx.mem.setf(p + 8, ee::mul(15.0, s));
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        words(fx, p + 0x10, p);
        let z = ee::add(fx.mem.f(p + 0x18), 15.0);
        fx.mem.setf(p + 0x18, z);
    }
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x2D8, u0);
    uv(fx, sp + 0x2DA, v0);
    uv(fx, sp + 0x2E8, u1);
    uv(fx, sp + 0x2EA, v1);
}

/// The table's UVs of entry `i` (u0 h0, u1 h2, v0 h4, v1 h6).
fn table_uvs(fx: &mut Fx, sp: u32, i: u8) {
    let t = UV_TABLE + 8 * u32::from(i);
    let (u0, u1, v0, v1) = (fx.mem.read_u16(t), fx.mem.read_u16(t + 2), fx.mem.read_u16(t + 4), fx.mem.read_u16(t + 6));
    uvs(fx, sp, u32::from(u0), u32::from(v0), u32::from(u1), u32::from(v1));
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// 0x20C0B0(out, scale, rot, trans) through pointers (0: none).
fn trs_to(fx: &mut Fx, out: u32, scale: u32, rot: u32, trans: u32) {
    let s = (scale != 0).then(|| std::array::from_fn(|k| fx.mem.f(scale + 4 * k as u32)));
    let r = (rot != 0).then(|| std::array::from_fn(|k| fx.mem.i32(rot + 4 * k as u32)));
    let t = (trans != 0).then(|| std::array::from_fn(|k| fx.mem.f(trans + 4 * k as u32)));
    let m: M4 = fx.trs(s, r, t);
    fx.write_m4(out, &m);
}

/// sp + 0x190 = the view-projection x sp + 0x1D0 (the matrix there).
fn combine(fx: &mut Fx, sp: u32) {
    let m = fx.read_m4(sp + 0x1D0);
    fx.mul_into(Some(sp + 0x190), &m);
}

/// The corners (-s, -s, 0, 1) at sp + 0x250 and (s, s, 0, 1) at sp +
/// 0x260 turned by the camera (sp + 0x210).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x250, n);
    fx.mem.setf(sp + 0x254, n);
    fx.mem.write_u32(sp + 0x258, 0);
    fx.mem.write_u32(sp + 0x25C, 0x3F80_0000);
    fx.mem.setf(sp + 0x260, s);
    fx.mem.setf(sp + 0x264, s);
    fx.mem.write_u32(sp + 0x268, 0);
    fx.mem.write_u32(sp + 0x26C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x250, sp + 0x210, sp + 0x250);
    fx.vu_apply_to(sp + 0x260, sp + 0x210, sp + 0x260);
}

/// The sprite's colour (c, a, Q = 1.0) at sp + 0x2D0.
fn colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(sp + 0x2D0, c[0]);
    fx.mem.write_u8(sp + 0x2D1, c[1]);
    fx.mem.write_u8(sp + 0x2D2, c[2]);
    fx.mem.write_u8(sp + 0x2D3, a);
    fx.mem.write_u32(sp + 0x2D4, 0x3F80_0000);
}

/// The sprite at sp + 0x270 / 0x280 = the corners + sp + 0x290, projected
/// and submitted at the mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    fx.vu_add4(sp + 0x270, sp + 0x250, sp + 0x290);
    fx.vu_add4(sp + 0x280, sp + 0x260, sp + 0x290);
    if crate::fxlib2::project(fx, sp + 0x270, PROJ_OUT, sp + 0x190, sp + DESC, sp + 0x3AC) {
        let w = fx.mem.f(sp + 0x3AC);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// The sprite descriptor rewritten (through its pointers), | 0x800, ALPHA
/// preset `preset`, the texture D+`tex`, clamp 5.
fn sprite_desc(fx: &mut Fx, sp: u32, preset: i32, tex: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(preset);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
}

/// The band descriptor rewritten, ALPHA preset `preset`, the texture
/// D+`tex`, clamp `clamp`.
fn band_desc(fx: &mut Fx, sp: u32, preset: i32, tex: u32, clamp: u64) {
    let d = sp + BAND;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, BAND_REGS);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(preset);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, clamp);
}

/// The band's four colours: grey with alphas 0, 0, `a2`, `a3`.
fn band_colours(fx: &mut Fx, sp: u32, a2: u8, a3: u8) {
    for (at, a) in [(0x328u32, 0u8)] {
        fx.mem.write_u8(sp + at, 0x80);
        fx.mem.write_u8(sp + at + 1, 0x80);
        fx.mem.write_u8(sp + at + 2, 0x80);
        fx.mem.write_u8(sp + at + 3, a);
        fx.mem.write_u32(sp + at + 4, 0x3F80_0000);
    }
    words2(fx, sp + 0x340, sp + 0x328);
    for (at, a) in [(0x358u32, a2), (0x370, a3)] {
        fx.mem.write_u8(sp + at, 0x80);
        fx.mem.write_u8(sp + at + 1, 0x80);
        fx.mem.write_u8(sp + at + 2, 0x80);
        fx.mem.write_u8(sp + at + 3, a);
        fx.mem.write_u32(sp + at + 4, 0x3F80_0000);
    }
}

/// The band's four ST pairs from the eight floats at `st`.
fn band_st(fx: &mut Fx, sp: u32, st: u32) {
    for (k, at) in [0x330u32, 0x334, 0x348, 0x34C, 0x360, 0x364, 0x378, 0x37C].into_iter().enumerate() {
        let w = fx.mem.read_u32(st + 4 * k as u32);
        fx.mem.write_u32(sp + at, w);
    }
}

/// A band piece of the four vertices, submitted at the mean w.
fn band(fx: &mut Fx, sp: u32, vs: [u32; 4], ras: [u32; 2]) {
    if fx.project_n(&vs, sp + 0x190, sp + BAND, sp + 0x3AC, ras[0]) == 1 {
        let w = fx.mem.f(sp + 0x3AC);
        let key = fix(fx, w, ras[1]);
        fx.submit(sp + BAND, OT, key as i32);
    }
}

/// rand >> 23, its signed remainder by 256, << 16 >> 8.
fn rand_turn(fx: &mut Fx, ra: u32) -> i32 {
    let r = fx.rand_from(OT, ra) >> 23;
    (rem2(r, 256) << 16) >> 8
}

/// The swirl records [F 0x1E005D8 / 0x1E00720].
fn swirls(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0xB..0x87).contains(&n) {
        let k = rem2(n, 0x20);
        let r = SWIRLS + 0x40 * k as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for j in 0..0x10 {
            let n = fx.mem.i32(D);
            fx.mem.set8(r + 8 + j as u32, rem2(k + n + j, 4));
        }
        let v = rand_turn(fx, 0x01E0_065C);
        fx.mem.set32(r + 0x18, v);
        let c = ee::from_i32(fx.mem.i32(D + 0x60));
        fx.mem.setf(r + 0x1C, ee::div(ee::mul(10.0, c), 64.0));
        let c = ee::from_i32(fx.mem.i32(D + 0x60));
        fx.mem.setf(r + 0x20, ee::div(ee::mul(50.0, c), 64.0));
        words(fx, r + 0x30, D + 0x40);
        let y = ee::add(fx.mem.f(r + 0x34), -5.0);
        fx.mem.setf(r + 0x34, y);
    }
    let c = fx.mem.i32(D + 0x60) + 1;
    fx.mem.set32(D + 0x60, c);
    if fx.mem.i32(D + 0x60) >= 0x41 {
        fx.mem.set32(D + 0x60, 0x40);
    }
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u32(sp + 0xE0, d + 2);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(sp + 0x120, d + 8);
    fx.mem.write_u32(sp + 0x130, d + 4);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    fx.camera_vp_to(sp + 0x190);
    fx.identity(sp + 0x1D0);
    combine(fx, sp);
    fx.mem.write_u32(sp + 0x140, sp + 0x2DA);
    fx.mem.write_u32(sp + 0x150, sp + 0x2EA);
    for k in 0..0x20u32 {
        let r = SWIRLS + 0x40 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let big = fx.mem.f(r + 0x1C);
        let f2 = ee::mul(ee::mul(f32::from_bits(0x3F4C_CCCD), big), age);
        let s = ee::add(ee::mul(f32::from_bits(0x3E4C_CCCD), big), ee::div(f2, 32.0));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 4);
        let sn = fx.tsin((age << 16) >> 5);
        let a = fix(fx, ee::mul(64.0, sn), 0x01E0_088C);
        colour(fx, sp, [0x80, 0x80, 0x80], a as u8);
        let f20 = ee::div(ee::from_i32(fx.mem.i32(r + 4)), 32.0);
        for j in 0..0x10i32 {
            let age = fx.mem.i32(r + 4);
            let spin = fx.mult(age, 0x5555) >> 5;
            let a = (fx.mem.i32(r + 0x18) + ((j << 16) >> 4) + spin) & 0xFFFF;
            let sn = fx.tsin(a);
            let rr = fx.mem.f(r + 0x20);
            fx.mem.setf(sp + 0x290, ee::mul(rr, sn));
            fx.mem.write_u32(sp + 0x294, 0);
            let cs = fx.tcos(a);
            let rr = fx.mem.f(r + 0x20);
            fx.mem.setf(sp + 0x298, ee::mul(rr, cs));
            fx.mem.write_u32(sp + 0x29C, 0x3F80_0000);
            fx.vu_scale4(sp + 0x290, sp + 0x290, f20);
            fx.vu_add4(sp + 0x290, sp + 0x290, r + 0x30);
            let b = fx.mem.read_u8(r + 8 + j as u32);
            table_uvs(fx, sp, b);
            draw(fx, sp, 0x01E0_0A6C);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The tube records [F 0x1E00AC8 / 0x1E00B98].
fn tubes(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if rem2(n, 4) == 0 && (0x29..0x87).contains(&n) {
        let r = TUBES + 0x20 * fx.mem.read_u32(D + 0x870);
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        let v = rand_turn(fx, 0x01E0_0B1C);
        fx.mem.set32(r + 8, v);
        words(fx, r + 0x10, D + 0x40);
        let y = ee::add(fx.mem.f(r + 0x14), 15.0);
        fx.mem.setf(r + 0x14, y);
        let c = fx.mem.i32(D + 0x870) + 1;
        fx.mem.set32(D + 0x870, c);
        if fx.mem.i32(D + 0x870) >= 0x10 {
            fx.mem.set32(D + 0x870, 0);
        }
    }
    let d = sp + BAND;
    fx.mem.write_u32(sp + 0xF0, d + 2);
    fx.mem.write_u32(sp + 0x100, d + 8);
    fx.mem.write_u32(sp + 0x110, d + 4);
    band_desc(fx, sp, 0, 0x14, BAND_CLAMP);
    fx.mem.write_u32(sp + 0x170, 0);
    for k in 0..0x10u32 {
        let r = TUBES + 0x20 * k;
        if fx.mem.read_u8(r) != 0 {
            let age = fx.mem.i32(r + 4);
            let f0 = fx.tan_falloff_p(age, 0x40, 1);
            fx.mem.write_u32(sp + 0x290, 0);
            fx.mem.setf(sp + 0x294, ee::mul(-24.0, f0));
            fx.mem.write_u32(sp + 0x298, 0);
            fx.mem.write_u32(sp + 0x29C, 0x3F80_0000);
            fx.vu_add4(sp + 0x290, sp + 0x290, r + 0x10);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let s = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), age), 64.0));
            fx.mem.setf(sp + 0x2A0, s);
            fx.mem.write_u32(sp + 0x2A4, 0x3F80_0000);
            fx.mem.setf(sp + 0x2A8, s);
            fx.mem.write_u32(sp + 0x2AC, 0x3F80_0000);
            let t = fx.mem.i32(r + 8).wrapping_add(0x1555);
            fx.mem.set32(r + 8, t);
            let t = fx.mem.read_u16(r + 8);
            fx.mem.write_u32(r + 8, u32::from(t));
            fx.mem.write_u32(sp + 0x2B0, 0);
            fx.mem.write_u32(sp + 0x2B4, u32::from(t));
            fx.mem.write_u32(sp + 0x2B8, 0);
            fx.mem.write_u32(sp + 0x2BC, 1);
            fx.camera_vp_to(sp + 0x190);
            trs_to(fx, sp + 0x1D0, sp + 0x2A0, sp + 0x2B0, sp + 0x290);
            combine(fx, sp);
            for s6 in 0..0x10u32 {
                let age = fx.mem.i32(r + 4);
                let f20 = ee::mul(64.0, fx.tsin((age << 15) >> 6));
                let sn = fx.tsin(((0x8000 * s6) as i32) >> 4);
                let a2 = fix(fx, ee::mul(f20, sn), 0x01E0_0D5C);
                let age = fx.mem.i32(r + 4);
                let f20 = ee::mul(64.0, fx.tsin((age << 15) >> 6));
                let sn = fx.tsin((((s6 + 1) << 15) as i32) >> 4);
                let a3 = fix(fx, ee::mul(f20, sn), 0x01E0_0DDC);
                band_colours(fx, sp, a2 as u8, a3 as u8);
                let (a, b) = (RING30 + 0x30 * s6, RING30 + 0x30 * (s6 + 1));
                band_st(fx, sp, ST_A + 0x40 * s6);
                band(fx, sp, [a, b, a + 0x10, b + 0x10], [0x01E0_0E90, 0x01E0_0EA4]);
                band_st(fx, sp, ST_A + 0x40 * s6 + 0x20);
                band(fx, sp, [a + 0x20, b + 0x20, a + 0x10, b + 0x10], [0x01E0_0F38, 0x01E0_0F4C]);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x41 {
                fx.mem.write_u8(r, 0);
            }
        }
        fx.mem.write_u32(sp + 0x170, k + 1);
    }
}

/// The fan records [F 0x1E00FBC / 0x1E011E8].
fn fans(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x92..0x9A).contains(&n) {
        let r = FANS + 0x60 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        let dz = ee::sub(fx.mem.f(D + 0x48), fx.mem.f(D + 0x58));
        let dy = ee::sub(fx.mem.f(D + 0x44), fx.mem.f(D + 0x54));
        let a = fx.atan2f(dz, dy);
        let f = ee::mul(65536.0, ee::div(a, f32::from_bits(0x40C9_0FDB)));
        fx.fix_residue(f, 0x01E0_104C);
        fx.mem.set32(r + 8, ee::fixsfsi(f));
        for j in 0..4u32 {
            let v = rand_turn(fx, 0x01E0_1068);
            fx.mem.set32(r + 0xC + 4 * j, ((0x10000 * j) as i32 >> 2) + v);
            let t = fx.mem.read_u16(r + 0xC + 4 * j);
            fx.mem.write_u32(r + 0xC + 4 * j, u32::from(t));
            let p = r + 0x20 + 0x10 * j;
            words(fx, p, D + 0x40);
            let s3 = rem2(fx.mem.i32(D), 0x20) as u32;
            let q = rem2(fx.rand_from(OT, 0x01E0_10DC) >> 23, 4) as u32;
            let e = DIRS + 0x180 * q + 0xC * s3;
            let x = ee::add(fx.mem.f(p), ee::mul(f32::from_bits(0x3DCC_CCCD), fx.mem.f(e)));
            fx.mem.setf(p, x);
            let q = rem2(fx.rand_from(OT, 0x01E0_1144) >> 23, 4) as u32;
            let e = DIRS + 0x180 * q + 0xC * s3;
            let y = ee::add(fx.mem.f(p + 4), ee::mul(f32::from_bits(0x3DCC_CCCD), fx.mem.f(e + 4)));
            fx.mem.setf(p + 4, y);
            let f2 = ee::mul(4.0, ee::from_i32(j as i32));
            let side = ee::from_i32(fx.mem.i32(D + 0xC));
            let z = ee::add(fx.mem.f(p + 8), ee::mul(f2, side));
            fx.mem.setf(p + 8, z);
        }
    }
    band_desc(fx, sp, 1, 0x14, BAND_CLAMP);
    fx.mem.write_u32(sp + 0x160, 0);
    for k in 0..0x10u32 {
        let r = FANS + 0x60 * k;
        if fx.mem.read_u8(r) != 0 {
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let s = ee::add(0.5, ee::div(ee::mul(0.5, age), 16.0));
            fx.mem.write_u32(sp + 0x180, 0);
            fx.mem.write_u32(sp + 0xC0, D + 0x60 * k);
            fx.mem.write_u32(sp + 0xD0, D + 0x60 * k);
            fx.mem.setf(sp + 0x290, s);
            fx.mem.setf(sp + 0x294, s);
            fx.mem.setf(sp + 0x298, s);
            fx.mem.write_u32(sp + 0x29C, 0x3F80_0000);
            for j in 0..4u32 {
                fx.camera_vp_to(sp + 0x190);
                trs_to(fx, sp + 0x1D0, 0, 0, r + 0x20 + 0x10 * j);
                let a = fx.mem.i32(r + 8);
                fx.rotate_axis(sp + 0x1D0, a, 0);
                let a = fx.mem.i32(r + 0xC + 4 * j);
                fx.rotate_axis(sp + 0x1D0, a, 1);
                fx.scale_rows(sp + 0x1D0, sp + 0x290);
                combine(fx, sp);
                for s1 in 0..8u32 {
                    let age = fx.mem.i32(r + 4);
                    let f20 = ee::mul(64.0, fx.tcos((age << 14) >> 4));
                    let sn = fx.tsin(((0x8000 * s1) as i32) >> 3);
                    let a2 = fix(fx, ee::mul(f20, sn), 0x01E0_1364);
                    let age = fx.mem.i32(r + 4);
                    let f20 = ee::mul(64.0, fx.tcos((age << 14) >> 4));
                    let sn = fx.tsin((((s1 + 1) << 15) as i32) >> 3);
                    let a3 = fix(fx, ee::mul(f20, sn), 0x01E0_13E4);
                    band_colours(fx, sp, a2 as u8, a3 as u8);
                    let (a, b) = (RING20 + 0x30 * s1, RING20 + 0x30 * (s1 + 1));
                    band_st(fx, sp, ST_B + 0x40 * s1);
                    band(fx, sp, [a, b, a + 0x10, b + 0x10], [0x01E0_1498, 0x01E0_14AC]);
                    band_st(fx, sp, ST_B + 0x40 * s1 + 0x20);
                    band(fx, sp, [a + 0x20, b + 0x20, a + 0x10, b + 0x10], [0x01E0_1540, 0x01E0_1554]);
                }
                fx.mem.write_u32(sp + 0xC0, D + 0x60 * k + 0x10 * (j + 1));
                fx.mem.write_u32(sp + 0xD0, D + 0x60 * k + 4 * (j + 1));
                fx.mem.write_u32(sp + 0x180, j + 1);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x11 {
                fx.mem.write_u8(r, 0);
            }
        }
        fx.mem.write_u32(sp + 0x160, k + 1);
    }
}

/// The burst at the target [F 0x1E015F4 / 0x1E01748].
fn burst(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x9B {
        fx.mem.write_u8(BURST, 1);
        fx.mem.write_u32(BURST + 4, 0);
        for s4 in 0..0x80u32 {
            let s3 = rem2(s4 as i32, 0x20) as u32;
            let p = BURST + 0x10 + 0x10 * s4;
            for (c, ra) in [(0u32, 0x01E0_1634u32), (4, 0x01E0_1690), (8, 0x01E0_16E0)] {
                let q = rem2(fx.rand_from(OT, ra) >> 23, 4) as u32;
                let e = DIRS + 0x180 * q + 0xC * s3;
                let v = fx.mem.f(e + c);
                fx.mem.setf(p + c, ee::mul(2.0, v));
            }
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    sprite_desc(fx, sp, 1, 0x18);
    fx.camera_vp_to(sp + 0x190);
    trs_to(fx, sp + 0x1D0, 0, 0, D + 0x20);
    combine(fx, sp);
    if fx.mem.read_u8(BURST) != 1 {
        return;
    }
    corners(fx, sp, 1.0);
    uvs(fx, sp, 0, 0, 0x400, 0x400);
    colour(fx, sp, [0x60, 0x80, 0xFF], 0);
    let age = fx.mem.i32(BURST + 4);
    let f0 = fx.tan_falloff_p(age, 0x18, 1);
    let f20 = ee::mul(255.0, ee::sub(1.0, f0));
    for s5 in 0..0x80u32 {
        let p = BURST + 0x10 + 0x10 * s5;
        for s6 in 0..8i32 {
            let sn = fx.tsin(((s6 + 1) << 14) >> 3);
            let a = fix(fx, ee::mul(f20, sn), 0x01E0_1930);
            fx.mem.write_u8(sp + 0x2D3, a as u8);
            let age = ee::from_i32(fx.mem.i32(BURST + 4));
            let f = ee::div(ee::add(age, ee::mul(f32::from_bits(0x3ECC_CCCD), ee::from_i32(s6))), 24.0);
            fx.vu_scale4(sp + 0x290, p, f);
            draw(fx, sp, 0x01E0_19D0);
        }
    }
    let age = fx.mem.i32(BURST + 4) + 1;
    fx.mem.set32(BURST + 4, age);
    if age >= 0x19 {
        fx.mem.write_u8(BURST, 0);
    }
}

/// The dome at the target [F 0x1E01A20 / 0x1E01A3C].
fn dome(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x9B {
        fx.mem.write_u8(DOME, 1);
        fx.mem.write_u32(DOME + 4, 0);
    }
    band_desc(fx, sp, 1, 0x10, 5);
    if fx.mem.read_u8(DOME) != 1 {
        return;
    }
    let age = fx.mem.i32(DOME + 4);
    let f0 = fx.tan_falloff_p(age, 0x30, 1);
    let s = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
    fx.mem.write_u32(sp + 0x290, 0x3F80_0000);
    fx.mem.setf(sp + 0x294, s);
    fx.mem.setf(sp + 0x298, s);
    fx.mem.write_u32(sp + 0x29C, 0x3F80_0000);
    for s7 in 0..4i32 {
        let age = fx.mem.i32(DOME + 4);
        let f0 = fx.tan_falloff_p(age, 0x30, 1);
        let f = ee::mul(ee::from_i32(s7 + 1), ee::mul(255.0, ee::sub(1.0, f0)));
        let a = fix(fx, ee::div(f, 4.0), 0x01E0_1B5C);
        fx.mem.write_u8(sp + 0x328, 0x40);
        fx.mem.write_u8(sp + 0x329, 0x40);
        fx.mem.write_u8(sp + 0x32A, 0xFF);
        fx.mem.write_u8(sp + 0x32B, a as u8);
        fx.mem.write_u32(sp + 0x32C, 0x3F80_0000);
        for at in [0x340u32, 0x358, 0x370] {
            words2(fx, sp + at, sp + 0x328);
        }
        let s4 = (0xE38 * s7) >> 2;
        for s5 in 0..8i32 {
            fx.camera_vp_to(sp + 0x190);
            trs_to(fx, sp + 0x1D0, 0, 0, D + 0x20);
            let side = fx.mem.i32(D + 0xC);
            fx.rotate_axis(sp + 0x1D0, side << 13, 2);
            let n = fx.mem.i32(D);
            let a = ((0x10000 * s5) >> 3) + fx.mult(n, 0x16C) + s4;
            fx.rotate_axis(sp + 0x1D0, a, 0);
            fx.scale_rows(sp + 0x1D0, sp + 0x290);
            combine(fx, sp);
            for s6 in 0..8i32 {
                fx.mem.write_u32(sp + 0x330, 0);
                fx.mem.setf(sp + 0x334, ee::div(ee::mul(0.5, ee::from_i32(s6)), 8.0));
                fx.mem.write_u32(sp + 0x348, 0x3F00_0000);
                let w = fx.mem.read_u32(sp + 0x334);
                fx.mem.write_u32(sp + 0x34C, w);
                let w = fx.mem.read_u32(sp + 0x330);
                fx.mem.write_u32(sp + 0x360, w);
                fx.mem.setf(sp + 0x364, ee::div(ee::mul(0.5, ee::from_i32(s6 + 1)), 8.0));
                let w = fx.mem.read_u32(sp + 0x348);
                fx.mem.write_u32(sp + 0x378, w);
                let w = fx.mem.read_u32(sp + 0x364);
                fx.mem.write_u32(sp + 0x37C, w);
                let (a, b) = (HALF15 + 0x20 * s6 as u32, HALF15 + 0x20 * (s6 + 1) as u32);
                band(fx, sp, [a, a + 0x10, b, b + 0x10], [0x01E0_1CE4, 0x01E0_1CF8]);
            }
        }
    }
    let age = fx.mem.i32(DOME + 4) + 1;
    fx.mem.set32(DOME + 4, age);
    if age >= 0x31 {
        fx.mem.write_u8(DOME, 0);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x210);
    words(fx, D + 0x50, D + 0x40);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x30));
    fx.node_position(h, slot, D + 0x40);
    if fx.mem.i32(D) == 0 {
        words(fx, D + 0x50, D + 0x40);
    }
    swirls(fx, sp);
    tubes(fx, sp);
    fans(fx, sp);
    burst(fx, sp);
    dome(fx, sp);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x91 {
        ret = 1;
    } else if n == 0x9B {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xCC {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}
