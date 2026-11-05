//! BMODULE b769 (card 34), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xC030
//! bytes) [F]: b030's lens family (b030.rs's
//! code, its scratch area 0x130 lower: the frame 0x680, sp + 0x3C0 =
//! b030's 0x4F0; the saves at sp + 0xCC / 0xD0 / 0xE0 / 0xFC / 0x350
//! stay). Every update the camera turn with roll, the attacker's nodes
//! 0x23 (clear 1) / 0x46 into D+0x30 / 0x40, its point into D+0x60 with
//! its screen point (D+0x70) and the Z of the point 1.5 radii in front
//! (D+0x84). Two lenses (on 0x6E / 0x96, 32 updates, (FF, 40, 24) / (24,
//! 40, FF)) share one packet layer; twelve fans (D+0x88, 13 updates) in
//! 0x1F..0x1F2; flecks (32 x 0xB0: two blocks of four at the two nodes,
//! blue / red), sparks (48 x 0x1F0, two blocks of eight pulled to the
//! target) and motes (48 x 0x150, two blocks of six) in their windows;
//! 32 rings (far Z, colours by parity) between radii 30 / 80 at the
//! target in 0x10F..0x1F2; a flat sprite at the target (far Z); a
//! full-window frame sprite (0x207EF0) in 0x172..0x212. The stage light
//! 0x80 - 96 clamp(n) clamp(0x232 - n) / 1024. Returns 1 on 0x104, 6 in
//! 0x10F..0x211, 2 on 0x212, 8 once the count reaches 0x233.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, KEEP, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The fan's nine screen points (exe rodata, 0x10 apart).
const FAN: u32 = 0x0028_ABE0;
const FRAME: u32 = 0x680;
/// b030's scratch offsets sit this much lower here.
const SHIFT: u32 = 0x130;

const FANS: u32 = 0x88;
const FLECKS: u32 = 0xF0;
const SPARKS: u32 = 0x16F0;
const MOTES: u32 = 0x73F0;
const RING_PTS: u32 = 0xB2F0;
const RINGS: u32 = 0xB620;
const LENS: u32 = 0xC020;

const R_LENS: [u32; 2] = [0x01E0_075C, 0x01E0_0798];
const R_FAN_A: [u32; 2] = [0x01E0_0BA8, 0x01E0_0C08];
const R_FAN: [u32; 12] = [
    0x01E0_0CB4, 0x01E0_0CC4, 0x01E0_0D14, 0x01E0_0D20, 0x01E0_0D68, 0x01E0_0D74, 0x01E0_0E40, 0x01E0_0E50, 0x01E0_0EC0, 0x01E0_0ECC, 0x01E0_0F40,
    0x01E0_0F4C,
];
const R_FLECK: [u32; 2] = [0x01E0_12AC, 0x01E0_1528];
const R_SPARK: [u32; 2] = [0x01E0_1AB8, 0x01E0_1C40];
const R_MOTE: [u32; 2] = [0x01E0_2074, 0x01E0_2260];
const R_RING: u32 = 0x01E0_2550;
const R_FLAT: u32 = 0x01E0_2CDC;
const R_FRAME: u32 = 0x01E0_2F14;
const RR_FLECK: u32 = 0x01E0_1100;
const RR_SPARK: [u32; 2] = [0x01E0_1698, 0x01E0_16B8];
const RR_MOTE: [u32; 2] = [0x01E0_1DA0, 0x01E0_1DC0];
const RR_RING: [u32; 3] = [0x01E0_2380, 0x01E0_23B4, 0x01E0_23E8];
const RP_RING: [u32; 2] = [0x01E0_2834, 0x01E0_298C];

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xCC, D);
    fx.mem.write_u32(sp + 0xD0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xC030);
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
    for (i, (node, clear)) in [(0x23, 1), (0x46, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0x1C + 4 * i as u32, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x50);
    let h = fx.mem.i32(D + 4);
    let r = crate::blib::afterimage::unit_radius(fx, h);
    fx.mem.setf(D + 0x80, ee::mul(1.5, r));
    for k in 0..12u32 {
        fx.mem.write_u8(D + FANS + 8 * k, 0);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + FLECKS + 0xB0 * k, 0);
    }
    for k in 0..0x30u32 {
        fx.mem.write_u8(D + SPARKS + 0x1F0 * k, 0);
    }
    for k in 0..0x30u32 {
        fx.mem.write_u8(D + MOTES + 0x150 * k, 0);
    }
    // The ring points: 17 x (30 / 80 at k / 16 of a turn, the middle).
    for k in 0..0x11u32 {
        let p = D + RING_PTS + 0x30 * k;
        let a = (k << 12) as i32;
        let s = fx.tsin(a);
        let c = fx.tcos(a);
        fx.mem.setf(p, ee::mul(30.0, s));
        fx.mem.setf(p + 4, ee::mul(30.0, c));
        fx.mem.write_u32(p + 8, 0);
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        fx.mem.setf(p + 0x10, ee::mul(80.0, s));
        fx.mem.setf(p + 0x14, ee::mul(80.0, c));
        fx.mem.write_u32(p + 0x18, 0);
        fx.mem.write_u32(p + 0x1C, 0x3F80_0000);
        fx.vu_add4(p + 0x20, p, p + 0x10);
        fx.vu_scale4(p + 0x20, p + 0x20, 0.5);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + RINGS + 0x50 * k, 0);
    }
    for k in 0..2u32 {
        fx.mem.write_u8(D + LENS + k, 0);
    }
    0
}

fn rem2(x: i32, m: i32) -> i32 {
    let r = x & (m - 1);
    if x < 0 && r != 0 {
        r - m
    } else {
        r
    }
}

/// 0x1A2B80 / 0x1A2C70 with their residue.
fn fixs(fx: &mut Fx, f: f32, ra: u32) -> i32 {
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}
fn fix_at(fx: &mut Fx, a: u32, ra: u32) -> i32 {
    let f = fx.mem.f(a);
    fixs(fx, f, ra)
}
fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

// From here `s` is the scratch base (sp - 0x130): every offset is b030's.

/// The sprite cell `b` (4 x n cells of 32).
fn cell_uvs(fx: &mut Fx, s: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    uv(fx, s + 0x658, ((c << 9) & 0x3FFF) as u32);
    uv(fx, s + 0x65A, ((r << 9) & 0x3FFF) as u32);
    uv(fx, s + 0x668, (((32 * c + 32) << 4) & 0x3FFF) as u32);
    uv(fx, s + 0x66A, (((32 * r + 32) << 4) & 0x3FFF) as u32);
}

fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

fn copy8(fx: &mut Fx, to: u32, from: u32) {
    let (a, b) = (fx.mem.read_u32(from), fx.mem.read_u32(from + 4));
    fx.mem.write_u32(to, a);
    fx.mem.write_u32(to + 4, b);
}

/// The view-projection (x identity).
fn camera(fx: &mut Fx, s: u32) {
    fx.camera_vp_to(s + 0x4F0);
    fx.identity(s + 0x530);
    let id = fx.read_m4(s + 0x530);
    fx.mul_into(Some(s + 0x4F0), &id);
}

/// The sprite descriptor (5 registers 0x42421, prim 0x26 | 0x800).
fn sprite_head(fx: &mut Fx, s: u32) {
    let d = s + 0x640;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
}

fn alpha(fx: &mut Fx, s: u32, a: i32) {
    let p = fx.alpha_preset(a);
    fx.desc_alpha(s + 0x640, p);
}

fn tex(fx: &mut Fx, s: u32, t: u32) {
    let rh = fx.mem.read_u32(D + t);
    fx.desc_texture(s + 0x640, rh, 1);
}

fn clamp5(fx: &mut Fx, s: u32) {
    fx.desc_clamp(s + 0x640, 5);
}

/// The corners (-k, -k, 0, 1) / (k, k, 0, 1) turned by the camera.
fn corners(fx: &mut Fx, s: u32, k: f32) {
    let n = ee::neg(k);
    fx.mem.setf(s + 0x5B0, n);
    fx.mem.setf(s + 0x5B4, n);
    fx.mem.write_u32(s + 0x5B8, 0);
    fx.mem.write_u32(s + 0x5BC, 0x3F80_0000);
    fx.mem.setf(s + 0x5C0, k);
    fx.mem.setf(s + 0x5C4, k);
    fx.mem.write_u32(s + 0x5C8, 0);
    fx.mem.write_u32(s + 0x5CC, 0x3F80_0000);
    fx.vu_apply_to(s + 0x5B0, s + 0x570, s + 0x5B0);
    fx.vu_apply_to(s + 0x5C0, s + 0x570, s + 0x5C0);
}

/// The sprite at `at` projected (0x204F70); submitted at the mean w
/// (`ra`) or far at key 0.
fn draw(fx: &mut Fx, s: u32, at: u32, ra: Option<u32>) {
    let d = s + 0x640;
    if crate::fxlib2::project(fx, at, PROJ_OUT, s + 0x4F0, d, s + 0x7AC) {
        match ra {
            Some(ra) => {
                let w = fx.mem.f(s + 0x7AC);
                let key = fixu(fx, w, ra) as i32;
                fx.submit(d, OT, key);
            }
            None => {
                crate::lens::far_z(fx, s + 0x660, &[s + 0x670]);
                fx.submit(d, OT, 0);
            }
        }
    }
}

/// The corners about s + 0x5F0, drawn.
fn draw_at(fx: &mut Fx, s: u32, ra: u32) {
    fx.vu_add4(s + 0x5D0, s + 0x5B0, s + 0x5F0);
    fx.vu_add4(s + 0x5E0, s + 0x5C0, s + 0x5F0);
    draw(fx, s, s + 0x5D0, Some(ra));
}

/// clamp(a, 0, m) x clamp(b, 0, m) / d.
fn window(a: i32, b: i32, m: i32, d: f32) -> f32 {
    ee::div(ee::mul(ee::from_i32(Fx::clamp_i(a, 0, m)), ee::from_i32(Fx::clamp_i(b, 0, m))), d)
}

/// a + b x `x` / `d`.
fn lin(a: f32, b: f32, x: f32, d: f32) -> f32 {
    ee::add(a, ee::div(ee::mul(b, x), d))
}

/// The screen point of `p` (0x204D80): x / y into `out` / `out` + 4; the
/// Z (bits 32..55).
fn screen(fx: &mut Fx, s: u32, p: u32, out: Option<u32>) -> u64 {
    let m = fx.read_m4(s + 0x4F0);
    fx.project_at(p, &m, s + 0x7A0, s + 0x7AC);
    if let Some(o) = out {
        let x = (i32::from(fx.mem.read_u16(s + 0x7A0)) >> 4) - 0x700;
        fx.mem.setf(o, ee::from_i32(x));
        let y = (i32::from(fx.mem.read_u16(s + 0x7A2)) >> 4) - 0x700;
        fx.mem.setf(o + 4, ee::from_i32(y));
    }
    (fx.mem.read_u64(s + 0x7A0) << 8) >> 40
}

fn screen_point(fx: &mut Fx, s: u32, p: u32, o: u32) {
    let z = screen(fx, s, p, Some(o));
    fx.mem.setf(o + 8, ee::from_i32(z as i32));
    fx.mem.write_u32(o + 0xC, 0x3F80_0000);
    for k in [0u32, 4] {
        if crate::exe::ee_lt(fx.mem.f(o + k), 0.0) {
            fx.mem.write_u32(o + k, 0);
        }
    }
    for k in [0u32, 4] {
        if !crate::exe::ee_le(fx.mem.f(o + k), 512.0) {
            fx.mem.write_u32(o + k, 0x4400_0000);
        }
    }
}

fn xyz_z(fx: &mut Fx, p: u32, x: i32, y: i32, z: u64) {
    fx.mem.write_u16(p, ((x + 0x700) << 4) as u16);
    fx.mem.write_u16(p + 2, ((y + 0x700) << 4) as u16);
    let v = fx.mem.read_u64(p);
    fx.mem.write_u64(p, (v & KEEP) | (z << 32));
    fx.mem.write_u8(p + 7, 0);
}

/// One ring of the eight-triangle fan about the screen point `c`.
fn fan(fx: &mut Fx, s: u32, c: u32, k: f32, z: u64) {
    let ras = &R_FAN;
    for t in 0..8u32 {
        let s1 = FAN + 0x10 * t;
        let x = fix_at(fx, c, ras[0]);
        let y = fix_at(fx, c + 4, ras[1]);
        uv(fx, s + 0x6B0, ((x << 4) & 0x3FFF) as u32);
        uv(fx, s + 0x6B2, ((y << 4) & 0x3FFF) as u32);
        let x = fix_at(fx, s1, ras[2]);
        let y = fix_at(fx, s1 + 4, ras[3]);
        uv(fx, s + 0x6C8, ((x << 4) & 0x3FFF) as u32);
        uv(fx, s + 0x6CA, ((y << 4) & 0x3FFF) as u32);
        let x = fix_at(fx, s1 + 0x10, ras[4]);
        let y = fix_at(fx, s1 + 0x14, ras[5]);
        uv(fx, s + 0x6E0, ((x << 4) & 0x3FFF) as u32);
        uv(fx, s + 0x6E2, ((y << 4) & 0x3FFF) as u32);
        fx.vu_sub4(s + 0x5D0, s1, c);
        fx.vu_sub4(s + 0x5E0, s1 + 0x10, c);
        fx.vu_scale4(s + 0x5D0, s + 0x5D0, k);
        fx.vu_scale4(s + 0x5E0, s + 0x5E0, k);
        fx.vu_add4(s + 0x5D0, s + 0x5D0, c);
        fx.vu_add4(s + 0x5E0, s + 0x5E0, c);
        let x = fix_at(fx, c, ras[6]);
        let y = fix_at(fx, c + 4, ras[7]);
        xyz_z(fx, s + 0x6B8, x, y, z);
        let x = fix_at(fx, s + 0x5D0, ras[8]);
        let y = fix_at(fx, s + 0x5D4, ras[9]);
        xyz_z(fx, s + 0x6D0, x, y, z);
        let x = fix_at(fx, s + 0x5E0, ras[10]);
        let y = fix_at(fx, s + 0x5E4, ras[11]);
        xyz_z(fx, s + 0x6E8, x, y, z);
        fx.submit(s + 0x698, 0, 0);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let s = sp - SHIFT;
    fx.camera_turn_roll(s + 0x570);
    for i in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x1C + 4 * i));
        fx.node_position(h, slot, D + 0x30 + 0x10 * i);
    }
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x60);
    camera(fx, s);
    screen_point(fx, s, D + 0x60, D + 0x70);
    // The point 1.5 radii in front of the attacker: its Z.
    fx.mem.write_u32(s + 0x5F0, 0);
    fx.mem.write_u32(s + 0x5F4, 0);
    let r = fx.mem.f(D + 0x80);
    fx.mem.setf(s + 0x5F8, ee::neg(r));
    fx.mem.write_u32(s + 0x5FC, 0x3F80_0000);
    fx.vu_apply_to(s + 0x5F0, s + 0x570, s + 0x5F0);
    fx.vu_add4(s + 0x5F0, s + 0x5F0, D + 0x60);
    let z = screen(fx, s, s + 0x5F0, None);
    fx.mem.write_u32(D + 0x84, z as u32);

    let n = fx.mem.i32(D);
    if n == 0x6E {
        fx.mem.write_u8(D + LENS, 1);
        fx.mem.write_u32(D + LENS + 4, 0);
    }
    if fx.mem.i32(D) == 0x96 {
        fx.mem.write_u8(D + LENS + 1, 1);
        fx.mem.write_u32(D + LENS + 8, 0);
    }
    lenses(fx, sp, s);

    let n = fx.mem.i32(D);
    if n >= 0x1F && n < 0x1F2 {
        let r = D + FANS + 8 * fx.div(n, 12).1 as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
    }
    fans(fx, sp, s);

    let n = fx.mem.i32(D);
    if n > 0 && n < 0x1F2 {
        let r = D + FLECKS + 0xB0 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for bk in 0..2u32 {
            for c in 0..4u32 {
                let w = fx.mem.read_u32(D + 0x30 + 0x10 * bk + 4 * c);
                fx.mem.write_u32(r + 0x10 + 0x10 * bk + 4 * c, w);
            }
            for j in 0..4u32 {
                let b = fx.rand_from(OT, RR_FLECK) >> 23;
                fx.mem.set8(r + 8 + 4 * bk + j, rem2(b, 4));
                fx.random_table_vec(r + 0x30 + 0x40 * bk + 0x10 * j, 1.0);
            }
        }
    }
    flecks(fx, s);

    let n = fx.mem.i32(D);
    if n >= 0xF4 && n < 0x1F2 {
        let r = D + SPARKS + 0x1F0 * fx.div(n, 0x30).1 as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for bk in 0..2u32 {
            for c in 0..4u32 {
                let w = fx.mem.read_u32(D + 0x30 + 0x10 * bk + 4 * c);
                fx.mem.write_u32(r + 0xD0 + 0x10 * bk + 4 * c, w);
            }
            for j in 0..8u32 {
                let i = 2 * j + bk;
                let b = fx.rand_from(OT, RR_SPARK[0]) >> 23;
                fx.mem.set32(r + 8 + 4 * i, rem2(b, 8));
                let b = fx.rand_from(OT, RR_SPARK[1]) >> 23;
                let x = ee::div(ee::mul(0.5, ee::from_i32(rem2(b, 0x100))), 256.0);
                // 0x1A2C30 / 0x1A3118 / 0x1A38F8: (float) (0.5 + (double) x).
                let sz = (0.5f64 + f64::from(x)) as f32;
                fx.mem.setf(r + 0x48 + 4 * i, sz);
                let dir = r + 0xF0 + 0x10 * i;
                fx.random_table_vec(dir, f(0x3F8C_CCCD));
                let a = Fx::powf2(fx.mem.f(dir));
                let b = Fx::powf2(fx.mem.f(dir + 4));
                let l = fx.sqrtf(ee::add(a, b));
                let m = fx.sqrtf(ee::mul(2.0, Fx::powf2(f(0x418C_CCCD))));
                let k = ee::mul(f(0x3E4C_CCCD), ee::sub(m, l));
                let m = fx.sqrtf(ee::mul(2.0, Fx::powf2(f(0x418C_CCCD))));
                fx.mem.setf(r + 0x88 + 4 * i, ee::add(f(0x3F4C_CCCD), ee::div(k, m)));
            }
        }
    }
    sparks(fx, s);

    let n = fx.mem.i32(D);
    if n >= 0xF4 && n < 0x1F2 {
        let r = D + MOTES + 0x150 * fx.div(n, 0x30).1 as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for bk in 0..2u32 {
            for c in 0..4u32 {
                let w = fx.mem.read_u32(D + 0x30 + 0x10 * bk + 4 * c);
                fx.mem.write_u32(r + 0x70 + 0x10 * bk + 4 * c, w);
            }
            for j in 0..6u32 {
                let i = 2 * j + bk;
                let b = fx.rand_from(OT, RR_MOTE[0]) >> 23;
                fx.mem.set32(r + 8 + 4 * i, rem2(b, 4));
                let b = fx.rand_from(OT, RR_MOTE[1]) >> 23;
                fx.mem.set32(r + 0x38 + 4 * i, (rem2(b, 0x100) << 16) >> 8);
                fx.random_table_vec(r + 0x90 + 0x10 * i, f(0x3F4C_CCCD));
            }
        }
    }
    motes(fx, s);

    let n = fx.mem.i32(D);
    if n >= 0x10F && n < 0x1F2 {
        let r = D + RINGS + 0x50 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        let m = r + 0x10;
        fx.identity(m);
        for (axis, ra) in [(1, RR_RING[0]), (0, RR_RING[1]), (2, RR_RING[2])] {
            let b = fx.rand_from(OT, ra) >> 23;
            fx.rotate_axis(m, (rem2(b, 0x100) << 16) >> 8, axis);
        }
    }
    rings(fx, s);
    flat(fx, s);
    frame_sprite(fx, s);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x232 - n, 0, 0x20);
    let k = Fx::clamp_i(n, 0, 0x20);
    let l = ((k.wrapping_mul(-3) << 5).wrapping_mul(s0) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x104 {
        ret = 1;
    } else {
        if n >= 0x10F && n < 0x212 {
            ret = 6;
        }
        if n == 0x212 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x233 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xD0, ret as u32);
    ret
}

/// The two lenses (32 updates) in one packet layer [F 0x1E005E4..0x1E009BC].
fn lenses(fx: &mut Fx, sp: u32, s: u32) {
    // The tint (prim 6, 3 registers) and the frame sprite.
    let t = s + 0x610;
    fx.mem.write_u16(t + 2, 6);
    fx.mem.write_u16(t, 3);
    fx.mem.write_u64(t + 8, 0x441);
    fx.mem.write_u32(t + 4, 0);
    let p = fx.alpha_preset(3);
    fx.desc_alpha(t, p);
    let d = s + 0x640;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let p = fx.alpha_preset(0);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, LENS_CLAMP);
    // a0..a3 as 0x208400 leaves them.
    let a = [u32::from(fx.mem.read_u16(d + 2)), LENS_CLAMP as u32, d + 0x10, d + 2];
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
    fx.mem.write_u8(sp + 0xE0, 0);
    let tints = [[0xFF, 0x40, 0x24], [0x24, 0x40, 0xFF]];
    for k in 0..2u32 {
        let l = D + LENS + k;
        let ag = D + LENS + 4 + 4 * k;
        if fx.mem.read_u8(l) == 0 {
            continue;
        }
        if fx.mem.read_u8(sp + 0xE0) == 0 {
            fx.mem.write_u8(sp + 0xE0, 1);
            let layer = fx.layer_begin();
            fx.mem.write_u32(sp + 0xFC, layer);
            let regs = crate::lens::frame_copy(fx, a, 0, s + 0x680, s + 0x688);
            fx.texflush(regs);
        }
        let age = fx.mem.i32(ag);
        let tt = ee::div(ee::from_i32(0x20 - age), 32.0);
        let al = fixu(fx, ee::mul(128.0, tt), R_LENS[k as usize]) as u8;
        rgba(fx, s + 0x620, tints[k as usize], al);
        far_xy(fx, s + 0x628, 0x7000, 0x7000);
        far_xy(fx, s + 0x630, 0x9000, 0x9000);
        fx.submit(s + 0x610, 0, 0);
        rgba(fx, s + 0x650, [0x80, 0x80, 0x80], 0x80u8.wrapping_sub(al));
        uv(fx, s + 0x658, 0);
        uv(fx, s + 0x65A, 0);
        uv(fx, s + 0x668, 0x2000);
        uv(fx, s + 0x66A, 0x2000);
        far_xy(fx, s + 0x660, 0x6FF8, 0x6FF8);
        far_xy(fx, s + 0x670, 0x8FF8, 0x8FF8);
        fx.submit(s + 0x640, 0, 0);
        let age = fx.mem.i32(ag) + 1;
        fx.mem.set32(ag, age);
        if age >= 0x21 {
            fx.mem.write_u8(l, 0);
        }
    }
    if fx.mem.read_u8(sp + 0xE0) == 1 {
        let layer = fx.mem.read_u32(sp + 0xFC);
        fx.layer_end(layer, OT, 0);
    }
}

/// The fan's triangle descriptor (prim 0x33 | 0x800, 9 registers), alpha
/// preset `a`, the lens CLAMP.
fn fan_desc(fx: &mut Fx, s: u32, a: i32) {
    let d = s + 0x698;
    fx.mem.write_u16(d + 2, 0x33);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
    let p = fx.alpha_preset(a);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, LENS_CLAMP);
}

/// The twelve fans (13 updates) [F 0x1E009BC..0x1E0104C].
fn fans(fx: &mut Fx, sp: u32, s: u32) {
    fan_desc(fx, s, 1);
    let n = fx.mem.i32(D);
    let a0 = Fx::clamp_i(n - 0x1E, 0, 0x40);
    let b0 = Fx::clamp_i(0x1F2 - n, 0, 0x40);
    let f21 = ee::div(ee::mul(ee::from_i32(a0), ee::from_i32(b0)), 4096.0);
    let f20 = ee::mul(f21, 0.0);
    // a0..a3 as the second 0x101050 leaves them.
    let a = [b0 as u32, 0, 0x40, s + 0x69A];
    fx.mem.write_u8(sp + 0x350, 0);
    for k in 0..12u32 {
        let r = D + FANS + 8 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        if fx.mem.read_u8(sp + 0x350) == 0 {
            fx.mem.write_u8(sp + 0x350, 1);
            let layer = fx.layer_begin();
            fx.mem.write_u32(sp + 0xFC, layer);
            fx.texflush(a);
            let h = fx.mem.i32(D + 4);
            let (_, regs) = fx.unit_draw(h, 1, s + 0x6F8, s + 0x700);
            fx.texflush(regs);
        }
        let x = (fx.mem.i32(r + 4) + 1) << 15;
        let sn = fx.tsin(x.wrapping_div(13));
        let al = fixu(fx, ee::mul(f21, ee::mul(255.0, sn)), R_FAN_A[0]) as u8;
        rgba(fx, s + 0x6A8, [0x80, 0x1C, 0x80], al);
        let x = (fx.mem.i32(r + 4) + 1) << 15;
        let _ = fx.tsin(x.wrapping_div(13));
        let al = fixu(fx, f20, R_FAN_A[1]) as u8;
        rgba(fx, s + 0x6C0, [0x60, 0x40, 0x60], al);
        copy8(fx, s + 0x6D8, s + 0x6C0);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let t = ee::div(ee::mul(f(0x3F19_999A), age), 12.0);
        let t = ee::add(1.0, ee::add(f(0xBE99_999A), t));
        let f22 = ee::add(f(0x3CCC_CCCD), t);
        let z = u64::from(fx.mem.read_u32(D + 0x84) & 0xFF_FFFF);
        fan(fx, s, D + 0x70, f22, z);
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0xD {
            fx.mem.write_u8(r, 0);
        }
    }
    if fx.mem.read_u8(sp + 0x350) == 1 {
        let layer = fx.mem.read_u32(sp + 0xFC);
        fx.layer_end(layer, OT, 0);
    }
}

/// The 32 flecks (two blocks of four at the nodes, 32 updates) [F
/// 0x1E01180..0x1E015E0].
fn flecks(fx: &mut Fx, s: u32) {
    sprite_head(fx, s);
    tex(fx, s, 0x10);
    alpha(fx, s, 1);
    clamp5(fx, s);
    camera(fx, s);
    let n = fx.mem.i32(D);
    let f20 = window(n, 0x1F2 - n, 0x20, 1024.0);
    for k in 0..0x20u32 {
        let r = D + FLECKS + 0xB0 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let sn = fx.tsin((age << 15) >> 5);
        let al = fixu(fx, ee::mul(f20, ee::mul(255.0, sn)), R_FLECK[0]) as u8;
        fx.mem.write_u8(s + 0x490, al);
        let age = fx.mem.i32(r + 4);
        let t = ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(0x20 - age)), 32.0);
        corners(fx, s, ee::mul(3.0, ee::add(f(0x3E4C_CCCD), t)));
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let spread = lin(f(0x3DCC_CCCD), f(0x3F66_6666), age, 32.0);
        for bk in 0..2u32 {
            let c = if bk == 1 { [0xFF, 0x3C, 0x18] } else { [0x18, 0x3C, 0xFF] };
            rgba(fx, s + 0x650, c, al);
            for j in 0..4u32 {
                let b = i32::from(fx.mem.read_u8(r + 8 + 4 * bk + j));
                cell_uvs(fx, s, b);
                fx.vu_scale4(s + 0x5F0, r + 0x30 + 0x40 * bk + 0x10 * j, spread);
                fx.vu_add4(s + 0x5F0, s + 0x5F0, r + 0x10 + 0x10 * bk);
                draw_at(fx, s, R_FLECK[1]);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The 48 sparks (two blocks of eight pulled to the target, 48 updates)
/// [F 0x1E01858..0x1E01CE4].
fn sparks(fx: &mut Fx, s: u32) {
    sprite_head(fx, s);
    alpha(fx, s, 1);
    tex(fx, s, 0x14);
    clamp5(fx, s);
    camera(fx, s);
    let n = fx.mem.i32(D);
    let f21 = ee::div(ee::from_i32(Fx::clamp_i(0x1F2 - n, 0, 0x20)), 32.0);
    for k in 0..0x30u32 {
        let r = D + SPARKS + 0x1F0 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + 4));
        corners(fx, s, ee::mul(13.0, lin(0.5, 0.5, age, 48.0)));
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let pull = ee::div(ee::mul(f(0x3FD9_999A), age), 48.0);
        let spread = lin(f(0x3E4C_CCCD), f(0x3F4C_CCCD), age, 48.0);
        for bk in 0..2u32 {
            let pos = r + 0xD0 + 0x10 * bk;
            fx.vu_sub4(s + 0x600, D + 0x50, pos);
            fx.vu_scale4(s + 0x600, s + 0x600, pull);
            fx.vu_add4(s + 0x600, s + 0x600, pos);
            let c = if bk == 1 { [0xC8, 0x46, 0x0A] } else { [0x0A, 0x46, 0xC8] };
            for j in 0..8u32 {
                let i = 2 * j + bk;
                let age = ee::from_i32(fx.mem.i32(r + 4));
                let f22 = ee::mul(80.0, Fx::ratio_below(age, 1.0));
                let age = ee::from_i32(fx.mem.i32(r + 4));
                let f0 = Fx::ramp_above(age, 48.0, 32.0);
                let w = fx.mem.f(r + 0x88 + 4 * i);
                let al = fixu(fx, ee::mul(w, ee::mul(f21, ee::mul(f22, f0))), R_SPARK[0]) as u8;
                rgba(fx, s + 0x650, c, al);
                let b = fx.mem.i32(r + 8 + 4 * i);
                cell_uvs(fx, s, b);
                let sz = fx.mem.f(r + 0x48 + 4 * i);
                fx.vu_scale4(s + 0x5D0, s + 0x5B0, sz);
                let sz = fx.mem.f(r + 0x48 + 4 * i);
                fx.vu_scale4(s + 0x5E0, s + 0x5C0, sz);
                fx.vu_scale4(s + 0x5F0, r + 0xF0 + 0x10 * i, spread);
                fx.vu_add4(s + 0x5F0, s + 0x5F0, s + 0x600);
                fx.vu_add4(s + 0x5D0, s + 0x5D0, s + 0x5F0);
                fx.vu_add4(s + 0x5E0, s + 0x5E0, s + 0x5F0);
                draw(fx, s, s + 0x5D0, Some(R_SPARK[1]));
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x31 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The 48 motes (two blocks of six pulled to the target, swaying, 48
/// updates) [F 0x1E01E58..0x1E02304].
fn motes(fx: &mut Fx, s: u32) {
    sprite_head(fx, s);
    alpha(fx, s, 1);
    tex(fx, s, 0x10);
    clamp5(fx, s);
    camera(fx, s);
    let n = fx.mem.i32(D);
    let f21 = ee::div(ee::from_i32(Fx::clamp_i(0x1F2 - n, 0, 0x20)), 32.0);
    for k in 0..0x30u32 {
        let r = D + MOTES + 0x150 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + 4));
        corners(fx, s, ee::mul(10.0, lin(0.5, 0.5, age, 48.0)));
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let pull = ee::div(ee::mul(f(0x3FD9_999A), age), 48.0);
        for bk in 0..2u32 {
            let pos = r + 0x70 + 0x10 * bk;
            fx.vu_sub4(s + 0x600, D + 0x50, pos);
            fx.vu_scale4(s + 0x600, s + 0x600, pull);
            fx.vu_add4(s + 0x600, s + 0x600, pos);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f22 = ee::mul(200.0, Fx::ratio_below(age, 1.0));
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f0 = Fx::ramp_above(age, 48.0, 32.0);
            let al = fixu(fx, ee::mul(f21, ee::mul(f22, f0)), R_MOTE[0]) as u8;
            let c = if bk == 1 { [0xFF, 0x3C, 0x32] } else { [0x32, 0x3C, 0xFF] };
            rgba(fx, s + 0x650, c, al);
            for j in 0..6u32 {
                let i = 2 * j + bk;
                let age_i = fx.mem.i32(r + 4);
                let age = ee::from_i32(age_i);
                let a = fx.mem.i32(r + 0x38 + 4 * i).wrapping_add((age_i << 15) / 48) & 0xFFFF;
                let l = lin(f(0x3E4C_CCCD), f(0x3F4C_CCCD), age, 48.0);
                let spread = ee::mul(l, fx.tsin(a));
                let b = fx.mem.i32(r + 8 + 4 * i);
                cell_uvs(fx, s, b);
                fx.vu_scale4(s + 0x5F0, r + 0x90 + 0x10 * i, spread);
                fx.vu_add4(s + 0x5F0, s + 0x5F0, s + 0x600);
                draw_at(fx, s, R_MOTE[1]);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x31 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The 32 rings (0x50, far Z) [F 0x1E02418..0x1E02B00].
fn rings(fx: &mut Fx, s: u32) {
    let d = s + 0x710;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    let pts = D + RING_PTS;
    let xyz = [s + 0x730, s + 0x748, s + 0x760, s + 0x778];
    for k in 0..0x20u32 {
        let r = D + RINGS + 0x50 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let sn = fx.tsin((age << 15) >> 5);
        let odd = rem2(k as i32, 2);
        let c0 = if odd == 0 { [0xFF, 0x80, 0x1C] } else { [0x1C, 0x80, 0xFF] };
        rgba(fx, s + 0x720, c0, 0);
        copy8(fx, s + 0x738, s + 0x720);
        let al = fixu(fx, ee::mul(80.0, sn), R_RING) as u8;
        let c1 = if odd == 0 { [0xC8, 0x3C, 0x20] } else { [0x20, 0x3C, 0xC8] };
        rgba(fx, s + 0x750, c1, al);
        copy8(fx, s + 0x768, s + 0x750);
        fx.camera_vp_to(s + 0x4F0);
        let t = [fx.mem.f(D + 0x50), fx.mem.f(D + 0x54), fx.mem.f(D + 0x58)];
        let m = fx.trs(None, None, Some(t));
        fx.write_m4(s + 0x530, &m);
        let rm = fx.read_m4(r + 0x10);
        fx.mul_into(Some(s + 0x530), &rm);
        let s1 = 2 * odd - 1;
        let age = fx.mem.i32(r + 4);
        fx.rotate_axis(s + 0x530, s1.wrapping_mul(55 * age), 0);
        let age = fx.mem.i32(r + 4);
        fx.rotate_axis(s + 0x530, s1.wrapping_mul(77 * age), 1);
        let age = fx.mem.i32(r + 4);
        fx.rotate_axis(s + 0x530, s1.wrapping_mul(99 * age), 2);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let sc = lin(f(0x3ECC_CCCD), f(0x3F19_999A), age, 32.0);
        fx.mem.setf(s + 0x5F0, sc);
        fx.mem.setf(s + 0x5F4, sc);
        fx.mem.write_u32(s + 0x5F8, 0);
        fx.mem.write_u32(s + 0x5FC, 0x3F80_0000);
        fx.scale_rows(s + 0x530, s + 0x5F0);
        let m = fx.read_m4(s + 0x530);
        fx.mul_into(Some(s + 0x4F0), &m);
        let age = fx.mem.i32(r + 4);
        let f20 = ee::div(ee::mul(-1.0, ee::from_i32(rem2(age, 16))), 16.0);
        let f21 = ee::add(0.5, f20);
        let f22 = ee::add(1.0, f20);
        for s7 in 0..0x10u32 {
            let (a, b) = (pts + 0x30 * s7, pts + 0x30 * (s7 + 1));
            let f23 = ee::div(ee::mul(8.0, ee::from_i32(s7 as i32)), 16.0);
            let f24 = ee::div(ee::mul(8.0, ee::from_i32(s7 as i32 + 1)), 16.0);
            for (o, x) in [(0x728u32, f23), (0x72C, f20), (0x740, f24), (0x744, f20), (0x758, f23), (0x75C, f21), (0x770, f24), (0x774, f21)] {
                fx.mem.setf(s + o, x);
            }
            let vs = [a, b, a + 0x20, b + 0x20];
            if fx.project_n(&vs, s + 0x4F0, d, s + 0x7AC, RP_RING[0]) == 1 {
                crate::lens::far_z(fx, xyz[0], &xyz[1..]);
                fx.submit(d, OT, 0);
            }
            for (o, x) in [(0x728u32, f23), (0x72C, f22), (0x740, f24), (0x744, f22), (0x758, f23), (0x75C, f21), (0x770, f24), (0x774, f21)] {
                fx.mem.setf(s + o, x);
            }
            let vs = [a + 0x10, b + 0x10, a + 0x20, b + 0x20];
            if fx.project_n(&vs, s + 0x4F0, d, s + 0x7AC, RP_RING[1]) == 1 {
                crate::lens::far_z(fx, xyz[0], &xyz[1..]);
                fx.submit(d, OT, 0);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The flat sprite at the target (far Z) [F 0x1E02B00..0x1E02DD0].
fn flat(fx: &mut Fx, s: u32) {
    let n = fx.mem.i32(D);
    let f20 = window(n - 0x10E, 0x212 - n, 0x20, 1024.0);
    if crate::exe::ee_le(f20, 0.0) {
        return;
    }
    sprite_head(fx, s);
    alpha(fx, s, 1);
    tex(fx, s, 0x10);
    clamp5(fx, s);
    uv(fx, s + 0x658, 0x400);
    uv(fx, s + 0x65A, 0);
    uv(fx, s + 0x668, 0x600);
    uv(fx, s + 0x66A, 0x200);
    fx.mem.setf(s + 0x5B0, -60.0);
    fx.mem.setf(s + 0x5B4, -60.0);
    fx.mem.write_u32(s + 0x5B8, 0);
    fx.mem.write_u32(s + 0x5BC, 0x3F80_0000);
    fx.mem.setf(s + 0x5C0, 60.0);
    fx.mem.setf(s + 0x5C4, 60.0);
    fx.mem.write_u32(s + 0x5C8, 0);
    fx.mem.write_u32(s + 0x5CC, 0x3F80_0000);
    let n = fx.mem.i32(D);
    let f1 = ee::add(f(0x3F66_6666), ee::mul(f(0x3DCC_CCCD), ee::from_i32(rem2(n, 2))));
    let al = fixu(fx, ee::mul(ee::mul(128.0, f20), f1), R_FLAT) as u8;
    rgba(fx, s + 0x650, [0xFF, 0x64, 0xFF], al);
    fx.camera_vp_to(s + 0x4F0);
    fx.translate_rows(s + 0x530, s + 0x570, D + 0x50);
    let m = fx.read_m4(s + 0x530);
    fx.mul_into(Some(s + 0x4F0), &m);
    draw(fx, s, s + 0x5B0, None);
}

/// The full-window frame sprite (0x207EF0) [F 0x1E02DD0..0x1E02FC0].
fn frame_sprite(fx: &mut Fx, s: u32) {
    let n = fx.mem.i32(D);
    let f20 = window(n - 0x172, 0x212 - n, 0x20, 1024.0);
    if crate::exe::ee_le(f20, 0.0) {
        return;
    }
    sprite_head(fx, s);
    let d = s + 0x640;
    let p = fx.alpha_preset(0);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, p);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    clamp5(fx, s);
    uv(fx, s + 0x658, 0x220);
    uv(fx, s + 0x65A, 0x220);
    uv(fx, s + 0x668, 0x1DE0);
    uv(fx, s + 0x66A, 0x1DE0);
    let al = fixu(fx, ee::mul(48.0, f20), R_FRAME) as u8;
    rgba(fx, s + 0x650, [0x80, 0x80, 0x80], al);
    far_xy(fx, s + 0x660, 0x7000, 0x7000);
    far_xy(fx, s + 0x670, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
}
