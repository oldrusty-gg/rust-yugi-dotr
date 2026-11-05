//! BMODULE b625 (card 475), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000 kept in
//! s0, 0x4020 bytes) [F]: a single unscheduled
//! function in a 0x480 frame (s1 the OT, the return at sp + 0xC0), a 0.44
//! relative of b212 (card 100): written fresh from the project notes on
//! b212.rs's grid / lens pieces. Every update the camera turn with roll
//! (sp + 0x280), the attacker's nodes 0x50 / 0x60 (D+0x50 / D+0x60) and
//! their midpoint D+0x70; the attacker's / target's points (kind 1) from
//! the init at D+0x20 / D+0x30. On 1..0x7B one of 12 rising records
//! (D+0x80, 0x50: 4 sprites from the 0x28AE90 tables round the attacker,
//! 12 updates); on 0x15..0x5B one of 32 shard records (D+0x440, 0x90: 8
//! sprites drawn in to the midpoint, 32 updates); on 0x7C a screen flash
//! (D+0x1640, 48 updates; the rising records and the glow cleared) and
//! four columns (D+0x33C0, 0x100 updates) above the target: per column a
//! 21 x 9 grid of rings (D+0x16E0) drawn as strips and, in a packet layer,
//! the frame copied as a texture onto a turned copy (D+0x22B0); on
//! 0x105..0x15B one of 32 falling records (D+0x3410, 0x60); every eighth
//! update in 0xA3..0x103 the object op 0x1F83B0; from 0x14 a glow at the
//! midpoint (D+0x4010). The stage light 0x80 - 64 clamp(n) clamp(0x177 -
//! n) / 1024. Returns 1 on 0xFA, 6 on 0x105..0x15B, 2 on 0x15C, 8 once
//! the count reaches 0x17D.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x480;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const DESC: u32 = 0x358;
/// The screen flash descriptor (prim 6, 3 registers).
const FLASHD: u32 = 0x3B0;
/// The strip descriptor (prim 0x34, 12 registers).
const STRIP: u32 = 0x3E0;
const CAM: u32 = 0x200;
const IDM: u32 = 0x240;
const TURN: u32 = 0x280;
const C0: u32 = 0x2C0;
const C1: u32 = 0x2D0;
const V0: u32 = 0x2E0;
const V1: u32 = 0x2F0;
const P: u32 = 0x340;
const W: u32 = 0x474;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
const LENS_CLAMP: u64 = 0x800_0080_000A;
/// The point tables (4 x 32 entries of 12 bytes).
const TABLES: u32 = 0x0028_AE90;
/// The falling records' UV table (8 bytes a cell).
const UVS: u32 = 0x0028_AE70;
const RISE: u32 = D + 0x80;
const SHARDS: u32 = D + 0x440;
const FLASH: u32 = D + 0x1640;
const RING9: u32 = D + 0x1650;
const GRID: u32 = D + 0x16E0;
const TURNED: u32 = D + 0x22B0;
const RINGS: u32 = D + 0x2E80;
const COLUMNS: u32 = D + 0x33C0;
const FALLS: u32 = D + 0x3410;
const GLOW: u32 = D + 0x4010;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xC0, 0);
    for (o, r) in [(0x478u32, 0x01E0_2380u32), (0x47C, 0x01E0_2388)] {
        let w = fx.mem.read_u32(r);
        fx.mem.write_u32(sp + o, w);
    }
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

/// The sprite's UVs (u0, v0) at sp + 0x370 and (u1, v1) at sp + 0x380.
fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x370, u0);
    uv(fx, sp + 0x372, v0);
    uv(fx, sp + 0x380, u1);
    uv(fx, sp + 0x382, v1);
}

fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

fn colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    rgba(fx, sp + 0x368, c, a);
}

/// The sprite descriptor (through its pointers at sp + 0x150 / 0x160 /
/// 0x170 once `spill`), ALPHA preset 1, the texture at D + `tex`, clamp 5.
fn sprite_desc(fx: &mut Fx, sp: u32, tex: u32, spill: bool) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    if spill {
        fx.mem.write_u32(sp + 0x150, d + 2);
    }
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    if spill {
        fx.mem.write_u32(sp + 0x160, d + 8);
        fx.mem.write_u32(sp + 0x170, d + 4);
    }
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
}

/// The view-projection (x identity) at sp + 0x200.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + CAM);
    fx.identity(sp + IDM);
    let id = fx.read_m4(sp + IDM);
    fx.mul_into(Some(sp + CAM), &id);
}

/// The view-projection x the translation to `p` (0x20C0B0) at sp + 0x200.
fn camera_to(fx: &mut Fx, sp: u32, p: u32) {
    fx.camera_vp_to(sp + CAM);
    let t = [fx.mem.f(p), fx.mem.f(p + 4), fx.mem.f(p + 8)];
    let m = fx.trs(None, None, Some(t));
    fx.write_m4(sp + IDM, &m);
    let m = fx.read_m4(sp + IDM);
    fx.mul_into(Some(sp + CAM), &m);
}

/// The corners (x0, y0, 0, 1) / (x1, y1, 0, 1) at sp + 0x2C0 / 0x2D0,
/// turned by the camera.
fn corners(fx: &mut Fx, sp: u32, a: [f32; 2], b: [f32; 2]) {
    for (at, v) in [(sp + C0, a), (sp + C1, b)] {
        fx.mem.setf(at, v[0]);
        fx.mem.setf(at + 4, v[1]);
        fx.mem.write_u32(at + 8, 0);
        fx.mem.write_u32(at + 0xC, 0x3F80_0000);
    }
    fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
    fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
}

fn square(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    corners(fx, sp, [n, n], [s, s]);
}

/// The vertices sp + 0x2E0 / 0x2F0 = the corners + `p`.
fn place(fx: &mut Fx, sp: u32, p: u32) {
    fx.vu_add4(sp + V0, sp + C0, p);
    fx.vu_add4(sp + V1, sp + C1, p);
}

fn project(fx: &mut Fx, sp: u32) -> bool {
    crate::fxlib2::project(fx, sp + V0, PROJ_OUT, sp + CAM, sp + DESC, sp + W)
}

/// Projected and submitted at the mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    if project(fx, sp) {
        let w = fx.mem.f(sp + W);
        let key = fixu(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// The far Z on the XYZ values at `xyz[0]` (set) and the rest (copied).
fn far_z(fx: &mut Fx, xyz: &[u32]) {
    let v = fx.mem.read_u64(xyz[0]);
    fx.mem.write_u64(xyz[0], (v & KEEP) | FAR_Z);
    for &a in &xyz[1..] {
        let z = ((fx.mem.read_u64(xyz[0]) << 8) >> 40) << 32;
        let w = fx.mem.read_u64(a);
        fx.mem.write_u64(a, (w & KEEP) | z);
    }
}

/// One coordinate from the point tables: table rand & 3, entry rand & 31,
/// component `comp` (None: a third rand % 3, scaled by 2).
fn table_coord(fx: &mut Fx, ras: &[u32], comp: Option<u32>) -> f32 {
    let t = rem2(fx.rand_from(OT, ras[0]) >> 23, 4);
    let base = TABLES.wrapping_add((t * 0x180) as u32);
    let e = rem2(fx.rand_from(OT, ras[1]) >> 23, 0x20);
    let entry = base.wrapping_add((e * 0xC) as u32);
    match comp {
        Some(c) => fx.mem.f(entry + c),
        None => {
            let r = fx.rand_from(OT, ras[2]) >> 23;
            let (_, c) = fx.div(r, 3);
            ee::mul(2.0, fx.mem.f(entry.wrapping_add((c * 4) as u32)))
        }
    }
}

/// Three table coordinates (x 2) and w = 1 at `p`.
fn table_point(fx: &mut Fx, p: u32, ras: &[u32; 9]) {
    for k in 0..3usize {
        let v = table_coord(fx, &ras[3 * k..3 * k + 3], None);
        fx.mem.setf(p + 4 * k as u32, v);
    }
    fx.mem.write_u32(p + 0xC, 0x3F80_0000);
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x4020);
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
    for (who, at) in [(4u32, 0x20u32), (8, 0x30)] {
        let h = fx.mem.i32(D + who);
        fx.unit_point(1, h, D + at);
    }
    for (node, clear, at) in [(0x50, 1, 0x40u32), (0x60, 0, 0x44)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    for k in 0..0xCu32 {
        fx.mem.write_u8(RISE + 0x50 * k, 0);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(SHARDS + 0x90 * k, 0);
    }
    fx.mem.write_u8(FLASH, 0);
    fx.mem.write_u8(COLUMNS, 0);
    for k in 0..0x20u32 {
        fx.mem.write_u8(FALLS + 0x60 * k, 0);
    }
    fx.mem.write_u8(GLOW, 0);
    0
}

/// The rising records [F 0x1E00290..0x1E00700].
fn rising(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x7C {
        let (_, k) = fx.div(n, 0xC);
        let r = RISE + 0x50 * k as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for j in 0..4u32 {
            let p = r + 0x10 + 0x10 * j;
            let x = table_coord(fx, &[0x01E0_02E0, 0x01E0_0314], Some(0));
            fx.mem.setf(p, x);
            fx.mem.write_u32(p + 4, 0xC20C_0000);
            let z = table_coord(fx, &[0x01E0_0350, 0x01E0_0384], Some(8));
            fx.mem.setf(p + 8, z);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
            fx.vu_add4(p, p, D + 0x20);
        }
    }
    sprite_desc(fx, sp, 0x10, true);
    uvs(fx, sp, 0, 0, 0x800, 0x800);
    fx.mem.write_u32(sp + 0x180, sp + 0x372);
    fx.mem.write_u32(sp + 0x190, sp + 0x382);
    corners(fx, sp, [-1.0, -12.0], [1.0, 12.0]);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(n, 0, 0x40)), 64.0);
    for k in 0..0xCu32 {
        let r = RISE + 0x50 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let x = fx.mem.i32(r + 4) << 15;
        fx.mult(0x2AAA_AAAB, x);
        let hi = fx.hilo().1;
        let ang = (hi >> 1).wrapping_add(((x as u32) >> 31) as i32);
        let sn = fx.tsin(ang);
        let a = fixu(fx, ee::mul(f20, ee::mul(255.0, sn)), 0x01E0_05E0);
        colour(fx, sp, [0x84, 0xB4, 0xFF], a as u8);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let f21 = ee::div(ee::mul(70.0, age), 12.0);
        for j in 0..4u32 {
            words(fx, sp + P, r + 0x10 + 0x10 * j, 4);
            let y = ee::add(fx.mem.f(sp + P + 4), f21);
            fx.mem.setf(sp + P + 4, y);
            place(fx, sp, sp + P);
            draw(fx, sp, 0x01E0_06AC);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0xD {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The shard records [F 0x1E00700..0x1E00C28].
fn shards(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= 0x15 && n < 0x5C {
        let r = SHARDS + 0x90 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        const RAS: [u32; 9] = [0x01E0_0758, 0x01E0_078C, 0x01E0_07B8, 0x01E0_07E8, 0x01E0_081C, 0x01E0_0848, 0x01E0_0878, 0x01E0_08AC, 0x01E0_08D8];
        for j in 0..8u32 {
            let p = r + 0x10 + 0x10 * j;
            table_point(fx, p, &RAS);
            fx.vu_add4(p, p, D + 0x70);
        }
    }
    sprite_desc(fx, sp, 0x14, false);
    uvs(fx, sp, 0, 0, 0x800, 0x800);
    camera(fx, sp);
    for k in 0..0x20u32 {
        let r = SHARDS + 0x90 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let s = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(0x20 - age)), 32.0);
        square(fx, sp, ee::mul(6.0, ee::add(f32::from_bits(0x3E4C_CCCD), s)));
        let age = fx.mem.i32(r + 4);
        colour(fx, sp, [0x80, 0x80, 0xFF], (age.wrapping_mul(255) >> 5) as u8);
        let age = fx.mem.i32(r + 4);
        let f20 = ee::div(ee::from_i32(0x20 - age), 32.0);
        for j in 0..8u32 {
            fx.vu_sub4(sp + P, r + 0x10 + 0x10 * j, D + 0x70);
            fx.vu_scale4(sp + P, sp + P, f20);
            fx.vu_add4(sp + P, sp + P, D + 0x70);
            place(fx, sp, sp + P);
            draw(fx, sp, 0x01E0_0BD8);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// An XYZ2 at `p`: x / y halfwords of pixel (x, y), the far Z, a zero top
/// byte.
fn xyz_far(fx: &mut Fx, p: u32, x: i32, y: i32) {
    fx.mem.write_u16(p, ((x + 0x700) << 4) as u16);
    fx.mem.write_u16(p + 2, ((y + 0x700) << 4) as u16);
    let v = fx.mem.read_u64(p);
    fx.mem.write_u64(p, (v & KEEP) | FAR_Z);
    fx.mem.write_u8(p + 7, 0);
}

/// The screen flash (48 updates) [F 0x1E00C28..0x1E00DA0].
fn flash(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x7C {
        fx.mem.write_u8(FLASH, 1);
        fx.mem.write_u32(FLASH + 4, 0);
        for k in 0..0xCu32 {
            fx.mem.write_u8(RISE + 0x50 * k, 0);
        }
        fx.mem.write_u8(GLOW, 0);
    }
    if fx.mem.read_u8(FLASH) != 1 {
        return;
    }
    let d = sp + FLASHD;
    fx.mem.write_u16(d + 2, 6);
    fx.mem.write_u16(d, 3);
    fx.mem.write_u64(d + 8, 0x441);
    fx.mem.write_u32(d + 4, 0);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    let age = fx.mem.i32(FLASH + 4);
    let t = fx.tan_falloff_p(age, 0x30, 1);
    let a = fixu(fx, ee::mul(255.0, ee::sub(1.0, t)), 0x01E0_0CDC);
    rgba(fx, sp + 0x3C0, [0x80, 0x80, 0xFF], a as u8);
    xyz_far(fx, sp + 0x3C8, 0, 0);
    xyz_far(fx, sp + 0x3D0, 0x200, 0x200);
    fx.submit(d, OT, 0);
    let age = fx.mem.i32(FLASH + 4) + 1;
    fx.mem.set32(FLASH + 4, age);
    if age >= 0x31 {
        fx.mem.write_u8(FLASH, 0);
    }
}

/// The strip descriptor's head through its pointers (sp + 0x1A0 / 0x1B0 /
/// 0x1C0).
fn strip_head(fx: &mut Fx, sp: u32, spill: bool) {
    let d = sp + STRIP;
    fx.mem.write_u16(d + 2, 0x34);
    if spill {
        fx.mem.write_u32(sp + 0x1A0, d + 2);
    }
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    if spill {
        fx.mem.write_u32(sp + 0x1B0, d + 8);
    }
    fx.mem.write_u32(d + 4, 0);
    if spill {
        fx.mem.write_u32(sp + 0x1C0, d + 4);
    }
}

/// The strip vertices' colours (0x80, 0x80, 0xFF, 0x80) at sp + 0x3F0 and
/// the 9-point ring of radius `r` at D+0x1650.
fn strip_colours_ring(fx: &mut Fx, sp: u32, r: f32) {
    rgba(fx, sp + 0x3F0, [0x80, 0x80, 0xFF], 0x80);
    for o in [0x408u32, 0x420, 0x438] {
        words(fx, sp + o, sp + 0x3F0, 2);
    }
    for j in 0..9u32 {
        let a = ((j << 16) as i32) >> 3;
        let p = RING9 + 0x10 * j;
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(r, s));
        fx.mem.write_u32(p + 4, 0);
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(r, c));
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
    }
}

fn grid_at(row: u32, k: u32) -> u32 {
    0x90 * row + 0x10 * k
}

/// The four columns (0x100 updates) [F 0x1E00DA0..0x1E01928].
fn columns(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x7C {
        fx.mem.write_u8(COLUMNS, 1);
        fx.mem.write_u32(COLUMNS + 4, 0);
        for k in 0..4u32 {
            let a = ((((k << 16) as i32) >> 2) + 0xE38) & 0xFFFF;
            let p = COLUMNS + 0x10 + 0x10 * k;
            let s = fx.tsin(a);
            fx.mem.setf(p, ee::mul(24.0, s));
            fx.mem.write_u32(p + 4, 0xC2FA_0000);
            let c = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(24.0, c));
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
            fx.vu_add4(p, p, D + 0x30);
        }
    }
    if fx.mem.read_u8(COLUMNS) != 1 {
        return;
    }
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::mul(10.0, ee::from_i32(Fx::clamp_i(n - 0x104, 0, 0x10))), 16.0);
    for q in 0..4u32 {
        for k in 0..0x15u32 {
            let age = fx.mem.i32(COLUMNS + 4);
            let s2 = (k << 16) as i32;
            fx.mult(0x6666_6667, s2);
            let a = (age.wrapping_mul(1820).wrapping_add((0x3039 * q) as i32).wrapping_add(s2 / 20)) & 0xFFFF;
            let p = RINGS + 0x150 * q + 0x10 * k;
            let s = fx.tsin(a);
            fx.mem.setf(p, ee::mul(f20, s));
            fx.mem.write_u32(p + 4, 0);
            let c = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(f20, c));
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    let age = fx.mem.i32(COLUMNS + 4);
    let c = Fx::clamp_i(age, 0, 0x10);
    let t = fx.tan_falloff_p(c, 0x10, 1);
    let f24 = ee::mul(250.0, t);
    let age = fx.mem.i32(COLUMNS + 4);
    let f21 = ee::div(ee::from_i32(Fx::clamp_i(0x100 - age, 0, 0x20)), 32.0);
    let d = sp + STRIP;
    strip_head(fx, sp, true);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    strip_colours_ring(fx, sp, ee::mul(5.0, f21));
    fx.mem.write_u32(sp + 0x1E0, 0);
    fx.mem.write_u32(sp + 0xE0, D);
    fx.mem.write_u32(sp + 0xF0, D);
    let mut last = None;
    for q in 0..4u32 {
        camera_to(fx, sp, COLUMNS + 0x10 + 0x10 * q);
        for row in 0..0x15u32 {
            let f20 = ee::div(ee::mul(f24, ee::from_i32(row as i32)), 20.0);
            for k in 0..9u32 {
                let g = GRID + grid_at(row, k);
                words(fx, g, RING9 + 0x10 * k, 4);
                fx.mem.setf(g + 4, f20);
                fx.vu_add4(g, g, RINGS + 0x150 * q + 0x10 * row);
            }
        }
        let age = fx.mem.i32(COLUMNS + 4);
        let f2 = ee::div(ee::mul(2.0, ee::from_i32(rem2(age, 0x10))), 16.0);
        let f20 = ee::sub(ee::div(ee::from_i32(q as i32), 4.0), f2);
        for row in 0..0x14u32 {
            let f23 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(row as i32)), 20.0), f20);
            let f22 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(row as i32 + 1)), 20.0), f20);
            for k in 0..8u32 {
                let s0 = ee::div(ee::mul(2.0, ee::from_i32(k as i32)), 8.0);
                let s1 = ee::div(ee::mul(2.0, ee::from_i32(k as i32 + 1)), 8.0);
                for (o, v) in [(0x3F8u32, s0), (0x3FC, f23), (0x410, s1), (0x414, f23), (0x428, s0), (0x42C, f22), (0x440, s1), (0x444, f22)] {
                    fx.mem.setf(sp + o, v);
                }
                let vs = [GRID + grid_at(row, k), GRID + grid_at(row, k + 1), GRID + grid_at(row + 1, k), GRID + grid_at(row + 1, k + 1)];
                last = None;
                if fx.project_n(&vs, sp + CAM, d, sp + W, 0x01E0_1318) == 1 {
                    let w = fx.mem.f(sp + W);
                    let key = fixu(fx, w, 0x01E0_132C);
                    fx.submit(d, OT, key as i32);
                    last = Some(key);
                }
            }
        }
        fx.mem.write_u32(sp + 0xE0, D + 0x10 * (q + 1));
        fx.mem.write_u32(sp + 0xF0, D + 0x150 * (q + 1));
        fx.mem.write_u32(sp + 0x1E0, q + 1);
    }
    // The lens: the frame copied as a texture onto a turned copy.
    strip_head(fx, sp, false);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xDC, layer);
    // a0..a3 at the first TEXFLUSH: as the last strip's submit left them,
    // or after a rejected last strip 0x205210's own (its vertices in
    // 0x100BA0's frame, the output buffer, the matrix, the descriptor) [run].
    let a = match last {
        Some(key) => [d, OT, key, d],
        None => [sp - 0x40, PROJ_OUT, sp + CAM, d],
    };
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x458, sp + 0x460);
    fx.texflush(regs);
    let p = fx.alpha_preset(0);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, LENS_CLAMP);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    strip_colours_ring(fx, sp, ee::mul(7.0, f21));
    fx.mem.write_u32(sp + 0x1D0, 0);
    fx.mem.write_u32(sp + 0x120, D);
    fx.mem.write_u32(sp + 0x130, D);
    fx.mem.write_u32(sp + 0x140, 0);
    for q in 0..4u32 {
        camera_to(fx, sp, COLUMNS + 0x10 + 0x10 * q);
        fx.mem.write_u32(sp + 0x100, D);
        fx.mem.write_u32(sp + 0x110, D + 0x150 * q);
        let shift = (5 * q) as i32;
        for row in 0..0x15u32 {
            let f20 = ee::div(ee::mul(f24, ee::from_i32(row as i32)), 20.0);
            let m = sp + 0x300;
            fx.identity(m);
            let age = fx.mem.i32(COLUMNS + 4);
            let (_, r) = fx.div(age.wrapping_add(shift), 0x18);
            let a1 = r << 16;
            fx.mult(0x2AAA_AAAB, a1);
            let fp = (row << 16) as i32;
            fx.mult(0x6666_6667, fp);
            let a = ((fp / 20).wrapping_add(a1 / 24)) & 0xFFFF;
            let s = fx.tsin(a);
            let v = fixs(fx, ee::mul(910.0, s), 0x01E0_162C);
            fx.rotate_axis(m, v, 1);
            let centre = RINGS + 0x150 * q + 0x10 * row;
            for k in 0..9u32 {
                let g = GRID + grid_at(row, k);
                words(fx, g, RING9 + 0x10 * k, 4);
                fx.mem.setf(g + 4, f20);
                fx.vu_add4(g, g, centre);
                let t = TURNED + grid_at(row, k);
                fx.vu_apply_to(t, m, RING9 + 0x10 * k);
                let age = fx.mem.i32(COLUMNS + 4);
                let (_, r) = fx.div(age.wrapping_add(shift), 0x18);
                let v = r << 16;
                fx.mult(0x2AAA_AAAB, v);
                let a = ((((k << 16) as i32) >> 3).wrapping_add(v / 24)) & 0xFFFF;
                let s = fx.tsin(a);
                let w = ee::mul(ee::mul(f32::from_bits(0x3DCC_CCCD), ee::div(f24, 20.0)), s);
                fx.mem.setf(t + 4, ee::add(f20, w));
                fx.vu_add4(t, t, centre);
            }
            fx.mem.write_u32(sp + 0x100, D + 0x90 * (row + 1));
            fx.mem.write_u32(sp + 0x110, D + 0x150 * q + 0x10 * (row + 1));
        }
        fx.mem.write_u32(sp + 0x1F0, 0);
        for row in 0..0x14u32 {
            for k in 0..8u32 {
                let (s3, s5) = (grid_at(row, k), grid_at(row, k + 1));
                let (s2, s4) = (grid_at(row + 1, k), grid_at(row + 1, k + 1));
                if fx.project_n(&[TURNED + s3, TURNED + s5, TURNED + s2, TURNED + s4], sp + CAM, d, sp + W, 0x01E0_17D0) != 1 {
                    continue;
                }
                for (u, x) in [(0x3F8u32, 0x400u32), (0x410, 0x418), (0x428, 0x430), (0x440, 0x448)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&[GRID + s3, GRID + s5, GRID + s2, GRID + s4], sp + CAM, d, sp + W, 0x01E0_184C) == 1 && front(fx, sp + 0x400, sp + 0x418, sp + 0x430) {
                    fx.submit(d, 0, 0);
                }
            }
            fx.mem.write_u32(sp + 0x1F0, row + 1);
        }
        fx.mem.write_u32(sp + 0x120, D + 0x10 * (q + 1));
        fx.mem.write_u32(sp + 0x130, D + 0x150 * (q + 1));
        fx.mem.write_u32(sp + 0x140, 5 * (q + 1));
        fx.mem.write_u32(sp + 0x1D0, q + 1);
    }
    let layer = fx.mem.read_u32(sp + 0xDC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(COLUMNS + 4) + 1;
    fx.mem.set32(COLUMNS + 4, age);
    if age >= 0x101 {
        fx.mem.write_u8(COLUMNS, 0);
    }
}

/// 0x101430(a, b, c) [F] (b212's): 1 when the triangle of the XYZ2 x / y
/// halfwords at `a` / `b` / `c` faces the camera.
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

/// The falling records [F 0x1E01928..0x1E01EB0].
fn falls(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= 0x105 && n < 0x15C {
        let r = FALLS + 0x60 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 1);
        const RAS: [u32; 9] = [0x01E0_19A4, 0x01E0_19D8, 0x01E0_1A04, 0x01E0_1A34, 0x01E0_1A68, 0x01E0_1A94, 0x01E0_1AC4, 0x01E0_1AF8, 0x01E0_1B24];
        for j in 0..4u32 {
            let c = fx.rand_from(OT, 0x01E0_1984) >> 23;
            fx.mem.set32(r + 8 + 4 * j, rem2(c, 4));
            table_point(fx, r + 0x20 + 0x10 * j, &RAS);
        }
    }
    sprite_desc(fx, sp, 0x1C, false);
    camera(fx, sp);
    for k in 0..0x20u32 {
        let r = FALLS + 0x60 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        colour(fx, sp, [0x64, 0x64, 0x80], ((0x20 - age).wrapping_mul(100) >> 5) as u8);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let s = ee::div(ee::mul(0.5, age), 32.0);
        square(fx, sp, ee::mul(15.0, ee::add(0.5, s)));
        let age = fx.mem.i32(r + 4);
        let f20 = fx.tan_falloff_p(age, 0x20, 1);
        for j in 0..4u32 {
            fx.vu_scale4(sp + P, r + 0x20 + 0x10 * j, f20);
            fx.vu_add4(sp + P, sp + P, D + 0x30);
            place(fx, sp, sp + P);
            let cell = fx.mem.read_u32(r + 8 + 4 * j);
            let e = UVS.wrapping_add(cell << 3);
            let (u0, v0) = (u32::from(fx.mem.read_u16(e)), u32::from(fx.mem.read_u16(e + 4)));
            let (u1, v1) = (u32::from(fx.mem.read_u16(e + 2)), u32::from(fx.mem.read_u16(e + 6)));
            uvs(fx, sp, u0, v0, u1, v1);
            draw(fx, sp, 0x01E0_1E54);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The glow at the midpoint [F 0x1E01F04..0x1E0221C].
fn glow(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x14 {
        fx.mem.write_u8(GLOW, 1);
        fx.mem.write_u32(GLOW + 4, 0);
    }
    if fx.mem.read_u8(GLOW) != 1 {
        return;
    }
    sprite_desc(fx, sp, 0x10, false);
    uvs(fx, sp, 0, 0, 0x800, 0x800);
    camera(fx, sp);
    let age = fx.mem.i32(GLOW + 4);
    let c = Fx::clamp_i(age, 0, 0x20);
    colour(fx, sp, [0xFF, 0xE0, 0xC8], (c.wrapping_mul(255) >> 5) as u8);
    let age = fx.mem.i32(GLOW + 4);
    let f2 = ee::from_i32(rem2(age, 2));
    let s = ee::mul(10.0, ee::add(f32::from_bits(0x3F66_6666), ee::mul(f32::from_bits(0x3DCC_CCCD), f2)));
    square(fx, sp, s);
    place(fx, sp, D + 0x70);
    if project(fx, sp) {
        if fx.mem.i32(D) < 0x46 {
            far_z(fx, &[sp + 0x378, sp + 0x388]);
            fx.submit(sp + DESC, OT, 0);
        } else {
            let w = fx.mem.f(sp + W);
            let key = fixu(fx, w, 0x01E0_2200);
            fx.submit(sp + DESC, OT, key as i32);
        }
    }
    let age = fx.mem.i32(GLOW + 4) + 1;
    fx.mem.set32(GLOW + 4, age);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x40 + 4 * k));
        fx.node_position(h, slot, D + 0x50 + 0x10 * k);
    }
    fx.vu_add4(D + 0x70, D + 0x50, D + 0x60);
    fx.vu_scale4(D + 0x70, D + 0x70, 0.5);
    rising(fx, sp);
    shards(fx, sp);
    flash(fx, sp);
    columns(fx, sp);
    falls(fx, sp);
    let n = fx.mem.i32(D);
    if rem2(n, 8) == 0 && n >= 0xA3 && n < 0x104 {
        let h = fx.mem.read_u32(D + 4);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h, 0x20, 1, sp + 0x478]);
    }
    glow(fx, sp);
    let n = fx.mem.i32(D);
    let s1 = Fx::clamp_i(0x177 - n, 0, 0x20);
    let k = Fx::clamp_i(n, 0, 0x20);
    let lt = (s1.wrapping_mul(k.wrapping_neg() << 6) >> 10) + 0x80;
    fx.set_stage_light(lt & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0xFA {
        ret = 1;
    } else {
        if (0x105..0x15C).contains(&n) {
            ret = 6;
        }
        if n == 0x15C {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x17D {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}
