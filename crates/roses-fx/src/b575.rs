//! BMODULE b575 (card 424), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080..0x1E02E9C, data area a0 =
//! 0x1ED0000, 0xFCD0 bytes, D kept in s0, OT in s1) [F] (helper C34, 4 Nov
//! 2026; from b249.rs's shell, card 206). A 0x690 frame: the camera turn
//! at sp + 0x380, the view-projection at sp + 0x300 (a scratch matrix at sp
//! + 0x340), the sprite descriptor at sp + 0x4E0, the quad descriptor at sp
//! + 0x538, the mean w at sp + 0x68C, the result at sp + 0xB0.
//!
//! Init: textures (5, 0..2) D+0x10..0x18, the attacker's nodes 0x14 (clear
//! 1) / 0x1D / 0x26 / 0x2F, the target's point D+0x70 and its radius min(r,
//! 20) D+0x80; 24 x 0x190 beam records D+0x90; a 17 x 17 grid D+0xA620
//! about the target (b249's rows over 16), 16 x 16 x 2 third points
//! D+0xB830, 16 x 16 angle triples D+0xD830, 16 x 16 x 0x80 ST cells
//! D+0x2620; 32 x 0xA0 spark records D+0xE440 (8 cells), 64 cell numbers
//! D+0xF848.
//!
//! Step: the four nodes D+0x30.. every update; a glow at each node from
//! update 0 (0xC9 updates); on 1..0xA7 a beam record (n % 24; 24 aim
//! vectors of 1.2 from the direction table), drawn 25 updates as 4 x 6
//! quads from each node along its vectors (0x100E40 / 0x100E00 aim); on
//! 0x2D the ribbons (0x92 updates: 16 segments from each node down to the
//! target, widened in screen space by a wobble); on 0x9E the shell (b249's,
//! 16 rows, 0x97 updates); on 0x4E..0xBD a spark record (8 sparks, 32
//! updates) below the target; on 0x115 64 sparks at the target (32
//! updates); the target hidden on 0xB4 and shown on 0x114 (0x1F7940).
//! Returns 1 on 0x94, 6 in 0x9F..0xBD, 2 on 0xBE, 8 once the count reaches
//! 0x136.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::LENS_CLAMP;

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70 / 0x205210's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// 0x101080's direction table (4 x 32 vec3).
const DIRS: u32 = 0x0028_AE90;
/// The sprite cell UV table (u0, u1, v0, v1 halfwords).
const CELL_UV: u32 = 0x0028_AE70;
const FRAME: u32 = 0x690;

// Stack layout.
const VP: u32 = 0x300;
const MS: u32 = 0x340;
const TURN: u32 = 0x380;
const C0: u32 = 0x3C0;
const C1: u32 = 0x3D0;
const Q0: u32 = 0x3E0;
const Q1: u32 = 0x3F0;
const Q2: u32 = 0x400;
const Q3: u32 = 0x410;
const AIM: u32 = 0x420;
const P: u32 = 0x460;
const P2: u32 = 0x470;
const NRM: u32 = 0x480;
const ROT: u32 = 0x4D0;
const SD: u32 = 0x4E0;
const QD: u32 = 0x538;
const FD: u32 = 0x5C8;
const TD: u32 = 0x640;
const W: u32 = 0x68C;

// D layout.
const BEAMS: u32 = 0x90;
const RIBBON: u32 = 0x2610;
const CELLS: u32 = 0x2620;
const GRID: u32 = 0xA620;
const THIRDS: u32 = 0xB830;
const ANGLES: u32 = 0xD830;
const SHELL: u32 = 0xE430;
const SPARKS: u32 = 0xE440;
const BURST: u32 = 0xF840;
const BURST_CELL: u32 = 0xF848;
const BURST_DIR: u32 = 0xF8D0;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

fn rem2(x: i32, m: i32) -> i32 {
    let r = x & (m - 1);
    if x < 0 && r != 0 {
        r - m
    } else {
        r
    }
}

/// rand() >> 23 (the caller's s0 is D throughout).
fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 23
}

/// 0x1A2B80 / 0x1A2C70 with their residue.
fn fixs(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}
fn fixu(fx: &mut Fx, v: f32, ra: u32) -> u32 {
    fx.fix_residue(v, ra);
    ee::fixunssfsi(v)
}

fn vec4(fx: &mut Fx, at: u32, x: f32, y: f32, z: f32) {
    fx.mem.setf(at, x);
    fx.mem.setf(at + 4, y);
    fx.mem.setf(at + 8, z);
    fx.mem.write_u32(at + 0xC, 0x3F80_0000);
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

fn copy8(fx: &mut Fx, to: u32, from: u32) {
    let (x, y) = (fx.mem.read_u32(from), fx.mem.read_u32(from + 4));
    fx.mem.write_u32(to, x);
    fx.mem.write_u32(to + 4, y);
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The sprite UVs of cell `c` from the table 0x28AE70.
fn table_uvs(fx: &mut Fx, sp: u32, c: u32) {
    let e = CELL_UV + 8 * c;
    let d = sp + SD;
    for (to, off) in [(0x18u32, 0u32), (0x1A, 4), (0x28, 2), (0x2A, 6)] {
        let v = u32::from(fx.mem.read_u16(e + off));
        uv(fx, d + to, v);
    }
}

/// The sprite descriptor (5 registers 0x42421, prim 0x26 | 0x800).
fn sprite_head(fx: &mut Fx, sp: u32) {
    let d = sp + SD;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
}

/// The quad descriptor (12 registers, prim 0x34).
fn quad_head(fx: &mut Fx, sp: u32) {
    let d = sp + QD;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
}

fn alpha(fx: &mut Fx, d: u32, a: i32) {
    let p = fx.alpha_preset(a);
    fx.desc_alpha(d, p);
}

fn tex(fx: &mut Fx, d: u32, t: u32) {
    let rh = fx.mem.read_u32(D + t);
    fx.desc_texture(d, rh, 1);
}

fn vp_times(fx: &mut Fx, sp: u32) {
    let m = fx.read_m4(sp + MS);
    fx.mul_into(Some(sp + VP), &m);
}

/// sp + 0x300 = the view-projection x translate(`at`).
fn vp_at(fx: &mut Fx, sp: u32, at: u32) {
    fx.camera_vp_to(sp + VP);
    let tr = std::array::from_fn(|j| fx.mem.f(at + 4 * j as u32));
    let m = fx.trs(None, None, Some(tr));
    fx.write_m4(sp + MS, &m);
    vp_times(fx, sp);
}

/// The corners (-k, -k, 0, 1) / (k, k, 0, 1) turned by the camera.
fn corners(fx: &mut Fx, sp: u32, k: f32) {
    let n = ee::neg(k);
    vec4(fx, sp + C0, n, n, 0.0);
    vec4(fx, sp + C1, k, k, 0.0);
    fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
    fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
}

/// 0x204F70 of the sprite from sp + 0x3E0, keyed by the mean w (`ra`).
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    let d = sp + SD;
    if crate::fxlib2::project(fx, sp + Q0, PROJ_OUT, sp + VP, d, sp + W) {
        let w = fx.mem.f(sp + W);
        let key = fixu(fx, w, ra) as i32;
        fx.submit(d, OT, key);
    }
}

/// 0x205210 of the quad descriptor from sp + 0x3E0 (four vertices).
fn project_quad(fx: &mut Fx, sp: u32) -> bool {
    let m = fx.read_m4(sp + VP);
    fx.project_desc(sp + Q0, PROJ_OUT, Some(&m), sp + QD, sp + W)
}

/// The quad's ST floats (0, 0) / (1, 0) / (0, 1) / (1, 1).
fn quad_st(fx: &mut Fx, sp: u32) {
    let q = sp + QD;
    for (o, s, t) in [(0x18u32, 0u32, 0u32), (0x30, 0x3F80_0000, 0), (0x48, 0, 0x3F80_0000), (0x60, 0x3F80_0000, 0x3F80_0000)] {
        fx.mem.write_u32(q + o, s);
        fx.mem.write_u32(q + o + 4, t);
    }
}

fn dir(fx: &mut Fx, row: i32, s: i32, c: u32) -> f32 {
    fx.mem.f(DIRS + 0x180 * row as u32 + 12 * s as u32 + 4 * c)
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0xFCD0);
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
    for (k, (node, clear)) in [(0x14, 1), (0x1D, 0), (0x26, 0), (0x2F, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0x1C + 4 * k as u32, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x70);
    let t = fx.mem.i32(D + 8);
    let r = crate::blib::afterimage::unit_radius(fx, t);
    fx.mem.setf(D + 0x80, r);
    if !crate::exe::ee_le(r, 20.0) {
        fx.mem.setf(D + 0x80, 20.0);
    }
    fx.mem.write_u8(D + 0x84, 0);
    for k in 0..0x18u32 {
        fx.mem.write_u8(D + BEAMS + 0x190 * k, 0);
    }
    fx.mem.write_u8(D + RIBBON, 0);
    // The grid: 17 rows of 17 points (point 16 = point 0).
    for row in 0..0x11i32 {
        let s4 = (row << 15) >> 4;
        for pt in 0..0x11i32 {
            let sn = fx.tsin(s4 & 0xFFFF);
            let mut f20 = ee::mul(fx.mem.f(D + 0x80), sn);
            let b = rem2(rnd(fx, 0x01E0_0214), 8);
            f20 = ee::add(f20, ee::div(ee::mul(ee::mul(0.5, f20), ee::from_i32(b)), 8.0));
            let f3 = ee::mul(5.0, fx.mem.f(D + 0x80));
            let mut f21 = ee::add(ee::div(ee::neg(f3), 2.0), ee::div(ee::mul(f3, ee::from_i32(row)), 16.0));
            if row != 0 && row != 16 {
                let b = rem2(rnd(fx, 0x01E0_02B8), 8);
                let k = ee::div(ee::div(ee::mul(5.0, fx.mem.f(D + 0x80)), 16.0), 2.0);
                f21 = ee::add(f21, ee::div(ee::mul(k, ee::sub(4.0, ee::from_i32(b))), 4.0));
            }
            let b = rem2(rnd(fx, 0x01E0_0328), 8);
            let a = ((((pt << 16) >> 4) + ((b << 11) >> 3)) & 0xFFFF) as i32;
            let p = D + GRID + 0x110 * row as u32 + 0x10 * pt as u32;
            let sn = fx.tsin(a);
            fx.mem.setf(p, ee::mul(f20, sn));
            fx.mem.setf(p + 4, f21);
            let cs = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(f20, cs));
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
        let p = D + GRID + 0x110 * row as u32;
        copy4(fx, p + 0x100, p);
    }
    // The third points: two a cell.
    let g = |r: u32, c: u32| D + GRID + 0x110 * r + 0x10 * c;
    for row in 0..0x10u32 {
        fx.mem.write_u32(sp + 0x2D0, row);
        fx.mem.write_u32(sp + 0xC0, D + 0x200 * row);
        for c in 0..0x10u32 {
            let out = D + THIRDS + 0x200 * row + 0x20 * c;
            crate::blib::exe::third_point(fx, out, g(row, c), g(row + 1, c), g(row + 1, c + 1));
            crate::blib::exe::third_point(fx, out + 0x10, g(row, c), g(row, c + 1), g(row + 1, c + 1));
        }
        fx.mem.write_u32(sp + 0xC0, D + 0x200 * (row + 1));
        fx.mem.write_u32(sp + 0x2D0, row + 1);
    }
    // The angle triples.
    for row in 0..0x10u32 {
        for c in 0..0x10u32 {
            let a = D + ANGLES + 0xC0 * row + 0xC * c;
            let mut ra = [0x01E0_04E8u32, 0x01E0_0510, 0x01E0_0548, 0x01E0_057C, 0x01E0_05B4, 0x01E0_05E4].into_iter();
            for (j, m) in [0x14Di32, 0xDE, 0x6F].into_iter().enumerate() {
                let b = rem2(rnd(fx, ra.next().unwrap()), 0x100);
                let s1 = b.wrapping_mul(m) >> 8;
                let sg = (rem2(rnd(fx, ra.next().unwrap()), 2) - 1) << 1;
                fx.mem.set32(a + 4 * j as u32, s1.wrapping_mul(sg) & 0xFFFF);
            }
        }
    }
    fx.mem.write_u8(D + SHELL, 0);
    // The ST cells: six points about a random centre, two thirds.
    for row in 0..0x10u32 {
        for c in 0..0x10u32 {
            let cell = D + CELLS + 0x800 * row + 0x80 * c;
            let lin = |b: i32| ee::div(ee::add(64.0, ee::div(ee::mul(384.0, ee::from_i32(b)), 256.0)), 512.0);
            let b = rem2(rnd(fx, 0x01E0_0660), 0x100);
            let f20 = lin(b);
            let b = rem2(rnd(fx, 0x01E0_06C8), 0x100);
            let f21 = lin(b);
            for j in 0..6u32 {
                let jit = |b: i32| ee::mul(0.25, ee::add(-0.5, ee::div(ee::from_i32(b), 256.0)));
                let b = rem2(rnd(fx, 0x01E0_0730), 0x100);
                let p = cell + 0x10 * j;
                fx.mem.setf(p, ee::add(f20, jit(b)));
                let b = rem2(rnd(fx, 0x01E0_0788), 0x100);
                fx.mem.setf(p + 4, ee::add(f21, jit(b)));
                fx.mem.write_u32(p + 8, 0);
                fx.mem.write_u32(p + 0xC, 0x3F80_0000);
            }
            crate::blib::exe::third_point(fx, cell + 0x60, cell, cell + 0x10, cell + 0x20);
            crate::blib::exe::third_point(fx, cell + 0x70, cell + 0x30, cell + 0x40, cell + 0x50);
        }
    }
    for i in 0..0x20u32 {
        let rec = D + SPARKS + 0xA0 * i;
        fx.mem.write_u8(rec, 0);
        for j in 0..8u32 {
            let b = rem2(rnd(fx, 0x01E0_0868), 4);
            fx.mem.write_u16(rec + 8 + 2 * j, b as u16);
        }
    }
    for k in 0..0x40u32 {
        let b = rem2(rnd(fx, 0x01E0_08BC), 4);
        fx.mem.write_u16(D + BURST_CELL + 2 * k, b as u16);
    }
    fx.mem.write_u8(D + BURST, 0);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    for k in 0..4u32 {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x1C + 4 * k));
        fx.node_position(h, s, D + 0x30 + 0x10 * k);
    }
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + 0x84, 1);
        fx.mem.write_u32(D + 0x88, 0);
    }
    let d = sp + SD;
    fx.mem.write_u32(sp + 0x1C0, d + 2);
    sprite_head(fx, sp);
    fx.mem.write_u32(sp + 0x270, d + 8);
    fx.mem.write_u32(sp + 0x280, d + 4);
    alpha(fx, d, 1);
    tex(fx, d, 0x10);
    fx.desc_clamp(d, 5);
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MS);
    vp_times(fx, sp);
    if fx.mem.read_u8(D + 0x84) == 1 {
        glow(fx, sp);
    }

    let n = fx.mem.i32(D);
    if n > 0 && n < 0xA8 {
        spawn_beam(fx, n);
    }
    beams(fx, sp);

    if fx.mem.i32(D) == 0x2D {
        fx.mem.write_u8(D + RIBBON, 1);
        fx.mem.write_u32(D + RIBBON + 4, 0);
    }
    if fx.mem.read_u8(D + RIBBON) == 1 {
        ribbons(fx, sp);
    }

    if fx.mem.i32(D) == 0x9E {
        fx.mem.write_u8(D + SHELL, 1);
        fx.mem.write_u32(D + SHELL + 4, 0);
    }
    if fx.mem.read_u8(D + SHELL) == 1 {
        shell(fx, sp);
    }

    let n = fx.mem.i32(D);
    if (0x4E..0xBE).contains(&n) {
        let rec = D + SPARKS + 0xA0 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(rec, 1);
        fx.mem.write_u32(rec + 4, 0);
        for j in 0..8i32 {
            let s3 = rem2(fx.mem.i32(D) + j, 0x20);
            let o = rec + 0x20 + 0x10 * j as u32;
            let row = rem2(rnd(fx, 0x01E0_233C), 4);
            let r80 = fx.mem.f(D + 0x80);
            let v = ee::mul(ee::mul(f(0x3DCC_CCCD), r80), dir(fx, row, s3, 0));
            fx.mem.setf(o, v);
            let f20 = fx.mem.f(D + 0x80);
            let row = rem2(rnd(fx, 0x01E0_23B0), 4);
            let acc = ee::mul(ee::mul(f(0x3DCC_CCCD), f20), dir(fx, row, s3, 1));
            fx.mem.setf(o + 4, ee::add(acc, ee::mul(3.0, f20)));
            let row = rem2(rnd(fx, 0x01E0_2420), 4);
            let r80 = fx.mem.f(D + 0x80);
            let v = ee::mul(ee::mul(f(0x3DCC_CCCD), r80), dir(fx, row, s3, 2));
            fx.mem.setf(o + 8, v);
            fx.mem.write_u32(o + 0xC, 0x3F80_0000);
        }
    }
    sparks(fx, sp);

    if fx.mem.i32(D) == 0x115 {
        fx.mem.write_u8(D + BURST, 1);
        fx.mem.write_u32(D + BURST + 4, 0);
        for k in 0..0x40u32 {
            let s3 = rem2(rnd(fx, 0x01E0_28C8), 0x20);
            let o = D + BURST_DIR + 0x10 * k;
            for (c, ra) in [0x01E0_28E8u32, 0x01E0_2958, 0x01E0_29BC].into_iter().enumerate() {
                let row = rem2(rnd(fx, ra), 4);
                let r80 = fx.mem.f(D + 0x80);
                let v = ee::mul(ee::mul(f(0x3E4C_CCCD), r80), dir(fx, row, s3, c as u32));
                fx.mem.setf(o + 4 * c as u32, v);
            }
            fx.mem.write_u32(o + 0xC, 0x3F80_0000);
        }
    }
    burst(fx, sp);

    if fx.mem.i32(D) == 0xB4 {
        let t = fx.mem.i32(D + 8);
        fx.unit_show(t, 0, 0x01E0_2DC8);
    }
    if fx.mem.i32(D) == 0x114 {
        let t = fx.mem.i32(D + 8);
        fx.unit_show(t, 1, 0x01E0_2DE4);
    }

    let n = fx.mem.i32(D);
    if n == 0x94 {
        fx.mem.write_u32(sp + 0xB0, 1);
    } else {
        if (0x9F..0xBE).contains(&n) {
            fx.mem.write_u32(sp + 0xB0, 6);
        }
        if n == 0xBE {
            fx.mem.write_u32(sp + 0xB0, 2);
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x136 {
        fx.mem.write_u32(sp + 0xB0, 8);
    }
    fx.mem.i32(sp + 0xB0)
}

/// The glow at the four nodes (0xC9 updates) [F 0x1E009E8..0x1E00BF4].
fn glow(fx: &mut Fx, sp: u32) {
    let d = sp + SD;
    let age = fx.mem.i32(D + 0x88);
    let v = (0xC8 - age).min(0x20);
    let a = ((v << 8) - v) >> 5;
    rgba(fx, d + 0x10, [0xA4, 0x80, 0xFF], a as u8);
    uv(fx, d + 0x18, 0);
    uv(fx, d + 0x1A, 0);
    uv(fx, d + 0x28, 0x800);
    uv(fx, d + 0x2A, 0x800);
    let s6 = fx.mem.i32(D + 0x88).min(0x20);
    for k in 0..4i32 {
        let v = rem2(fx.mem.i32(D + 0x88) + k, 2);
        let m = ee::sub(1.0, ee::mul(f(0x3E99_999A), ee::from_i32(v)));
        let r = ee::div(ee::mul(ee::from_i32(s6), ee::mul(10.0, m)), 32.0);
        corners(fx, sp, r);
        let at = D + 0x30 + 0x10 * k as u32;
        fx.vu_add4(sp + Q0, sp + C0, at);
        fx.vu_add4(sp + Q1, sp + C1, at);
        draw(fx, sp, 0x01E0_0BB8);
    }
    let age = fx.mem.i32(D + 0x88) + 1;
    fx.mem.set32(D + 0x88, age);
    if fx.mem.i32(D + 0x88) >= 0xC9 {
        fx.mem.write_u8(D + 0x84, 0);
    }
}

/// A beam record (n % 24): flag 1 (2 from 0x2D), 4 x 6 vectors of 1.2 from
/// the direction table (entry (n + i + j) % 32 of a random row per axis).
fn spawn_beam(fx: &mut Fx, n: i32) {
    let (_, r) = fx.div(n, 0x18);
    let rec = D + 0x190 * r as u32;
    fx.mem.write_u8(rec + 0x90, if n < 0x2D { 1 } else { 2 });
    fx.mem.write_u32(rec + 0x94, 0);
    for i in 0..4i32 {
        for j in 0..6i32 {
            let s4 = rem2(fx.mem.i32(D) + i + j, 0x20);
            let o = rec + 0xA0 + 0x60 * i as u32 + 0x10 * j as u32;
            for (c, ra) in [0x01E0_0CC0u32, 0x01E0_0D20, 0x01E0_0D74].into_iter().enumerate() {
                let row = rem2(rnd(fx, ra), 4);
                let v = ee::mul(f(0x3F99_999A), dir(fx, row, s4, c as u32));
                fx.mem.setf(o + 4 * c as u32, v);
            }
            fx.mem.write_u32(o + 0xC, 0x3F80_0000);
        }
    }
}

/// The beams (25 updates): from each node 6 quads along the record's
/// vectors, each 3 wide (turned about its axis by (i << 16) / 24 + (j <<
/// 16) / 6) and -2 x the falloff long [F 0x1E00DF4..0x1E0119C].
fn beams(fx: &mut Fx, sp: u32) {
    let q = sp + QD;
    quad_head(fx, sp);
    fx.mem.write_u32(sp + 0x1D0, q + 2);
    fx.mem.write_u32(sp + 0x1E0, q + 8);
    fx.mem.write_u32(sp + 0x1F0, q + 4);
    alpha(fx, q, 1);
    tex(fx, q, 0x10);
    fx.desc_clamp(q, 5);
    for i in 0..0x18u32 {
        let rec = D + 0x190 * i;
        fx.mem.write_u32(sp + 0x2C0, i);
        fx.mem.write_u32(sp + 0xE0, rec);
        if fx.mem.read_u8(rec + 0x90) != 0 {
            let age = fx.mem.i32(rec + 0x94);
            let t = fx.tan_falloff_p(age, 0x18, 1);
            let mut a = fixu(fx, ee::mul(255.0, ee::sub(1.0, t)), 0x01E0_0EC0) & 0xFF;
            if fx.mem.read_u8(rec + 0x90) == 2 {
                a >>= 1;
            }
            rgba(fx, q + 0x10, [0xA4, 0x80, 0xFF], a as u8);
            for o in [0x28u32, 0x40, 0x58] {
                copy8(fx, q + o, q + 0x10);
            }
            let f20 = ee::mul(-2.0, t);
            let s5 = ((i << 16) / 24) as i32;
            for k in 0..4u32 {
                fx.mem.write_u32(sp + 0xD0, D + 0x10 * k);
                vp_at(fx, sp, D + 0x30 + 0x10 * k);
                for j in 0..6u32 {
                    let v = rec + 0xA0 + 0x60 * k + 0x10 * j;
                    fx.point_along(sp + AIM, v);
                    fx.rotate_axis(sp + AIM, s5 + ((j << 16) / 6) as i32, 2);
                    vec4(fx, sp + C0, 0.0, 1.5, 0.0);
                    vec4(fx, sp + C1, 0.0, ee::mul(-1.0, 1.5), 0.0);
                    fx.vu_apply_to(sp + C0, sp + AIM, sp + C0);
                    fx.vu_apply_to(sp + C1, sp + AIM, sp + C1);
                    fx.vu_add4(sp + Q0, sp + C0, v);
                    fx.vu_add4(sp + Q1, sp + C1, v);
                    fx.vu_scale4(sp + P, v, f20);
                    fx.vu_add4(sp + Q2, sp + Q0, sp + P);
                    fx.vu_add4(sp + Q3, sp + Q1, sp + P);
                    quad_st(fx, sp);
                    if project_quad(fx, sp) {
                        let w = fx.mem.f(sp + W);
                        let key = fixu(fx, w, 0x01E0_10F8) as i32;
                        fx.submit(q, OT, key);
                    }
                }
            }
            fx.mem.write_u32(sp + 0xD0, D + 0x40);
            let age = fx.mem.i32(rec + 0x94) + 1;
            fx.mem.set32(rec + 0x94, age);
            if fx.mem.i32(rec + 0x94) >= 0x19 {
                fx.mem.write_u8(rec + 0x90, 0);
            }
        }
        fx.mem.write_u32(sp + 0xF0, (i + 1) << 16);
    }
    fx.mem.write_u32(sp + 0x2C0, 0x18);
    fx.mem.write_u32(sp + 0xE0, D + 0x190 * 0x18);
}

/// The XYZ2 halfword at `a` as a float.
fn xy(fx: &mut Fx, a: u32) -> i32 {
    i32::from(fx.mem.read_u16(a))
}

/// The ribbons (0x92 updates): from each node 16 segments down to the
/// target (lowered by 2.5 r), each projected as a quad, then its screen
/// edges rebuilt about the segment's ends at half widths wobbling with
/// sin((k % 8) << 13 - 0x176F n + 0x83F node) [F 0x1E011BC..0x1E01A8C].
fn ribbons(fx: &mut Fx, sp: u32) {
    let q = sp + QD;
    quad_head(fx, sp);
    alpha(fx, q, 1);
    tex(fx, q, 0x14);
    fx.desc_clamp(q, 5);
    let age = fx.mem.i32(D + RIBBON + 4);
    let v = (0x91 - age).min(0x20);
    let a = ((v << 8) - v) >> 5;
    rgba(fx, q + 0x10, [0xA4, 0x80, 0xFF], a as u8);
    for o in [0x28u32, 0x40, 0x58] {
        copy8(fx, q + o, q + 0x10);
    }
    let a0 = fx.mem.i32(D + RIBBON + 4).min(0x20);
    let f20 = ee::div(ee::from_i32(a0), 32.0);
    let mut s3 = 0i32;
    for j in 0..4u32 {
        fx.mem.write_u32(sp + 0x2B0, j);
        fx.mem.write_u32(sp + 0x100, D + 0x10 * j);
        let v = rem2(fx.mem.i32(D + RIBBON + 4) + j as i32, 2);
        let k = ee::mul(f20, ee::mul(3.0, ee::sub(1.0, ee::mul(f(0x3DCC_CCCD), ee::from_i32(v)))));
        corners(fx, sp, k);
        copy4(fx, sp + P, D + 0x70);
        let y = fx.mem.f(sp + P + 4);
        let drop = ee::div(ee::mul(5.0, fx.mem.f(D + 0x80)), 2.0);
        fx.mem.setf(sp + P + 4, ee::sub(y, drop));
        let node = D + 0x30 + 0x10 * j;
        fx.vu_sub4(sp + P, sp + P, node);
        vp_at(fx, sp, node);
        let mut prev = false;
        for s5 in 0..0x10i32 {
            fx.vu_scale4(sp + P2, sp + P, ee::div(ee::mul(f20, ee::from_i32(s5)), 16.0));
            fx.vu_add4(sp + Q0, sp + C0, sp + P2);
            fx.vu_add4(sp + Q1, sp + C1, sp + P2);
            fx.vu_scale4(sp + P2, sp + P, ee::div(ee::mul(f20, ee::from_i32(s5 + 1)), 16.0));
            fx.vu_add4(sp + Q2, sp + C0, sp + P2);
            fx.vu_add4(sp + Q3, sp + C1, sp + P2);
            quad_st(fx, sp);
            if !project_quad(fx, sp) {
                prev = false;
                continue;
            }
            let (x0, y0) = (xy(fx, q + 0x20), xy(fx, q + 0x22));
            let (x1, y1) = (xy(fx, q + 0x38), xy(fx, q + 0x3A));
            let (x2, y2) = (xy(fx, q + 0x50), xy(fx, q + 0x52));
            let (x3, y3) = (xy(fx, q + 0x68), xy(fx, q + 0x6A));
            let xa = ee::div(ee::from_i32(x0 + x1), 2.0);
            let ya = ee::div(ee::from_i32(y0 + y1), 2.0);
            vec4(fx, sp + P2, xa, ya, 0.0);
            let xb = ee::div(ee::from_i32(x2 + x3), 2.0);
            let yb = ee::div(ee::from_i32(y2 + y3), 2.0);
            vec4(fx, sp + 0x480, xb, yb, 0.0);
            let at = fx.atan2f(ee::sub(ya, yb), ee::sub(xa, xb));
            let s6 = fixs(fx, ee::mul(65536.0, ee::div(at, f(0x40C9_0FDB))), 0x01E0_162C);
            let n = fx.mem.i32(D);
            let wob = |k: i32| ((((rem2(k, 8) << 16) >> 3) + n.wrapping_mul(-0x176F) + s3) & 0xFFFF) as i32;
            let px = Fx::powf2(ee::sub(xa, ee::from_i32(x0)));
            let py = Fx::powf2(ee::sub(ya, ee::from_i32(y0)));
            let mut f21 = fx.sqrtf(ee::add(px, py));
            let sn = fx.tsin(wob(s5));
            f21 = ee::add(ee::mul(ee::mul(f(0x3DCC_CCCD), f21), sn), ee::mul(f(0x3F66_6666), f21));
            let px = Fx::powf2(ee::sub(xb, ee::from_i32(x2)));
            let py = Fx::powf2(ee::sub(yb, ee::from_i32(y2)));
            let mut f22 = fx.sqrtf(ee::add(px, py));
            let sn = fx.tsin(wob(s5 + 1));
            f22 = ee::add(ee::mul(ee::mul(f(0x3DCC_CCCD), f22), sn), ee::mul(f(0x3F66_6666), f22));
            if !prev {
                let (sn, cs) = (fx.tsin(s6), fx.tcos(s6));
                fx.mem.setf(sp + 0x490, ee::sub(xa, ee::mul(f21, sn)));
                fx.mem.setf(sp + 0x494, ee::add(ya, ee::mul(f21, cs)));
                fx.mem.write_u64(sp + 0x498, 0);
                fx.mem.setf(sp + 0x4A0, ee::add(xa, ee::mul(f21, sn)));
                fx.mem.setf(sp + 0x4A4, ee::sub(ya, ee::mul(f21, cs)));
                fx.mem.write_u64(sp + 0x4A8, 0);
            } else {
                copy4(fx, sp + 0x490, sp + 0x4B0);
                copy4(fx, sp + 0x4A0, sp + 0x4C0);
            }
            let (sn, cs) = (fx.tsin(s6), fx.tcos(s6));
            fx.mem.setf(sp + 0x4B0, ee::sub(xb, ee::mul(f22, sn)));
            fx.mem.setf(sp + 0x4B4, ee::add(yb, ee::mul(f22, cs)));
            fx.mem.write_u64(sp + 0x4B8, 0);
            fx.mem.setf(sp + 0x4C0, ee::add(xb, ee::mul(f22, sn)));
            fx.mem.setf(sp + 0x4C4, ee::sub(yb, ee::mul(f22, cs)));
            fx.mem.write_u64(sp + 0x4C8, 0);
            for (from, to, ra) in [
                (0x490u32, 0x20u32, 0x01E0_1998u32),
                (0x494, 0x22, 0x01E0_19A8),
                (0x4A0, 0x38, 0x01E0_19B8),
                (0x4A4, 0x3A, 0x01E0_19C8),
                (0x4B0, 0x50, 0x01E0_19D8),
                (0x4B4, 0x52, 0x01E0_19E8),
                (0x4C0, 0x68, 0x01E0_19F8),
                (0x4C4, 0x6A, 0x01E0_1A08),
            ] {
                let v = fx.mem.f(sp + from);
                let h = fixu(fx, v, ra);
                fx.mem.write_u16(q + to, h as u16);
            }
            let w = fx.mem.f(sp + W);
            let key = fixu(fx, w, 0x01E0_1A18) as i32;
            fx.submit(q, OT, key);
            prev = true;
        }
        s3 += 0x83F;
    }
    let age = fx.mem.i32(D + RIBBON + 4) + 1;
    fx.mem.set32(D + RIBBON + 4, age);
    if fx.mem.i32(D + RIBBON + 4) >= 0x92 {
        fx.mem.write_u8(D + RIBBON, 0);
    }
}

/// The shell (150 updates): b249's, 16 rows of 16 cells [F
/// 0x1E01AD4..0x1E022B4].
fn shell(fx: &mut Fx, sp: u32) {
    let fd = sp + FD;
    fx.mem.write_u16(fd + 2, 0x33);
    fx.mem.write_u16(fd, 9);
    fx.mem.write_u64(fd + 8, 0x4_2142_1421);
    fx.mem.write_u32(fd + 4, 0);
    alpha(fx, fd, 0);
    fx.desc_clamp(fd, LENS_CLAMP);
    fx.frame_desc(fd, 1, [fd + 0x10, fd + 2]);
    let td = sp + TD;
    fx.mem.write_u16(td + 2, 0x13);
    fx.mem.write_u16(td, 6);
    fx.mem.write_u64(td + 8, 0x41_4141);
    fx.mem.write_u32(td + 4, 0);
    alpha(fx, td, 1);
    let age = fx.mem.i32(D + SHELL + 4);
    let a0 = age.min(0x20);
    let mut rows = (a0 << 4) >> 5;
    if rows >= 0x11 {
        rows = 0x10;
    }
    fx.mem.set32(sp + 0x2F0, rows);
    let v = (0x96 - age).min(0x20);
    let k = 0x20 - v;
    fx.mem.set32(sp + 0x200, k);
    let t = fx.tan_falloff_p(k, 0x20, 1);
    let f20 = ee::mul(ee::mul(3.0, fx.mem.f(D + 0x80)), t);
    let r = Fx::ramp_above(ee::from_i32(k), 32.0, 16.0);
    let al1 = fixu(fx, ee::mul(120.0, r), 0x01E0_1C30) as u8;
    rgba(fx, fd + 0x10, [0x80, 0x80, 0x80], 0);
    rgba(fx, fd + 0x28, [0x80, 0x80, 0x80], al1);
    copy8(fx, fd + 0x40, fd + 0x28);
    let r = Fx::ramp_above(ee::from_i32(k), 32.0, 16.0);
    let al2 = fixu(fx, ee::mul(56.0, r), 0x01E0_1CA8) as u8;
    rgba(fx, td + 0x10, [0, 0, 0], 0);
    rgba(fx, td + 0x20, [0xFF, 0xFF, 0xFF], al2);
    copy8(fx, td + 0x30, td + 0x20);
    let g = |r: u32, c: u32| D + GRID + 0x110 * r + 0x10 * c;
    for row in 0..rows.max(0) as u32 {
        fx.mem.write_u32(sp + 0x2A0, row);
        for c in 0..0x10u32 {
            fx.mem.write_u32(sp + 0x2E0, c);
            let ang = D + ANGLES + 0xC0 * row + 0xC * c;
            for j in 0..3u32 {
                let a = fx.mem.i32(ang + 4 * j);
                fx.mem.set32(sp + ROT + 4 * j, k.wrapping_mul(a) & 0xFFFF);
            }
            fx.mem.write_u32(sp + ROT + 0xC, 1);
            let cell = D + CELLS + 0x800 * row + 0x80 * c;
            for h in 0..2u32 {
                let t = D + THIRDS + 0x200 * row + 0x20 * c + 0x10 * h;
                fx.vu_sub4(sp + Q0, t, t);
                fx.vu_sub4(sp + Q1, g(row, c), t);
                let gr = if h == 0 { g(row + 1, c) } else { g(row, c + 1) };
                fx.vu_sub4(sp + Q2, gr, t);
                fx.vu_sub4(sp + Q3, g(row + 1, c + 1), t);
                normal(fx, 3, sp + Q1, sp + NRM);
                let sgn = if h == 0 { -1.0 } else { 1.0 };
                fx.vu_scale4(sp + P2, sp + NRM, ee::mul(f20, sgn));
                fx.vu_add4(sp + P2, sp + P2, D + 0x70);
                fx.vu_add4(sp + P2, sp + P2, t);
                fx.camera_vp_to(sp + VP);
                let rot = std::array::from_fn(|j| fx.mem.i32(sp + ROT + 4 * j as u32));
                let tr = std::array::from_fn(|j| fx.mem.f(sp + P2 + 4 * j as u32));
                let m = fx.trs(None, Some(rot), Some(tr));
                fx.write_m4(sp + MS, &m);
                vp_times(fx, sp);
                let s5 = cell + 0x30 * h;
                let mid = cell + 0x60 + 0x10 * h;
                let tris: [([u32; 3], u32, u32, [u32; 3]); 3] = [
                    ([Q0, Q1, Q2], s5, s5 + 0x10, [0x01E0_1F88, 0x01E0_1FCC, 0x01E0_1FE4]),
                    ([Q0, Q2, Q3], s5 + 0x10, s5 + 0x20, [0x01E0_2074, 0x01E0_20B8, 0x01E0_20D0]),
                    ([Q0, Q3, Q1], s5 + 0x20, s5, [0x01E0_2154, 0x01E0_2198, 0x01E0_21B0]),
                ];
                for (vs, st1, st2, ras) in tris {
                    for (o, from) in [(fd + 0x18, mid), (fd + 0x30, st1), (fd + 0x48, st2)] {
                        copy8(fx, o, from);
                    }
                    let vs = [sp + vs[0], sp + vs[1], sp + vs[2]];
                    if fx.project_n(&vs, sp + VP, fd, sp + W, ras[0]) == 1 {
                        for (o, from) in [(td + 0x18, fd + 0x20), (td + 0x28, fd + 0x38), (td + 0x38, fd + 0x50)] {
                            let v = fx.mem.read_u64(from);
                            fx.mem.write_u64(o, v);
                        }
                        let w = fx.mem.f(sp + W);
                        let key = fixu(fx, w, ras[1]) as i32;
                        fx.submit(td, OT, key);
                        let w = fx.mem.f(sp + W);
                        let key = fixu(fx, w, ras[2]) as i32;
                        fx.submit(fd, OT, key);
                    }
                }
            }
        }
    }
    let age = fx.mem.i32(D + SHELL + 4) + 1;
    fx.mem.set32(D + SHELL + 4, age);
    if fx.mem.i32(D + SHELL + 4) >= 0x97 {
        fx.mem.write_u8(D + SHELL, 0);
    }
}

/// The spark records below the target (32 updates, 8 sparks each) [F
/// 0x1E024A0..0x1E02888].
fn sparks(fx: &mut Fx, sp: u32) {
    let d = sp + SD;
    sprite_head(fx, sp);
    alpha(fx, d, 1);
    tex(fx, d, 0x18);
    fx.desc_clamp(d, 5);
    copy4(fx, sp + P, D + 0x70);
    let y = fx.mem.f(sp + P + 4);
    let drop = ee::div(ee::mul(5.0, fx.mem.f(D + 0x80)), 2.0);
    fx.mem.setf(sp + P + 4, ee::sub(y, drop));
    vp_at(fx, sp, sp + P);
    for i in 0..0x20u32 {
        fx.mem.write_u32(sp + 0x290, i);
        let rec = D + SPARKS + 0xA0 * i;
        if fx.mem.read_u8(rec) == 0 {
            continue;
        }
        let q = ee::mul(0.5, ee::mul(0.5, fx.mem.f(D + 0x80)));
        let age = ee::from_i32(fx.mem.i32(rec + 4));
        corners(fx, sp, ee::add(q, ee::div(ee::mul(q, age), 32.0)));
        let age = ee::from_i32(fx.mem.i32(rec + 4));
        let r = Fx::ramp_above(age, 32.0, f(0x412A_AAAB));
        let al = fixu(fx, ee::mul(128.0, r), 0x01E0_2678) as u8;
        rgba(fx, d + 0x10, [0x80, 0x80, 0x80], al);
        let f20 = ee::div(ee::from_i32(fx.mem.i32(rec + 4)), 32.0);
        for j in 0..8u32 {
            fx.vu_scale4(sp + P, rec + 0x20 + 0x10 * j, f20);
            fx.vu_add4(sp + Q0, sp + C0, sp + P);
            fx.vu_add4(sp + Q1, sp + C1, sp + P);
            let c = u32::from(fx.mem.read_u16(rec + 8 + 2 * j));
            table_uvs(fx, sp, c);
            draw(fx, sp, 0x01E0_2808);
        }
        let age = fx.mem.i32(rec + 4) + 1;
        fx.mem.set32(rec + 4, age);
        if fx.mem.i32(rec + 4) >= 0x21 {
            fx.mem.write_u8(rec, 0);
        }
    }
    fx.mem.write_u32(sp + 0x290, 0x20);
}

/// The 64 sparks at the target (32 updates) [F 0x1E02A38..0x1E02DA8].
fn burst(fx: &mut Fx, sp: u32) {
    let d = sp + SD;
    sprite_head(fx, sp);
    alpha(fx, d, 1);
    tex(fx, d, 0x18);
    fx.desc_clamp(d, 5);
    vp_at(fx, sp, D + 0x70);
    if fx.mem.read_u8(D + BURST) != 1 {
        return;
    }
    let half = ee::mul(0.5, fx.mem.f(D + 0x80));
    let age = ee::from_i32(fx.mem.i32(D + BURST + 4));
    corners(fx, sp, ee::add(half, ee::div(ee::mul(half, age), 32.0)));
    let age = ee::from_i32(fx.mem.i32(D + BURST + 4));
    let r = Fx::ramp_above(age, 32.0, 16.0);
    let al = fixu(fx, ee::mul(128.0, r), 0x01E0_2BA4) as u8;
    rgba(fx, d + 0x10, [0x80, 0x80, 0x80], al);
    let age = ee::from_i32(fx.mem.i32(D + BURST + 4));
    let f20 = ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0));
    for k in 0..0x40u32 {
        fx.vu_scale4(sp + P, D + BURST_DIR + 0x10 * k, f20);
        fx.vu_add4(sp + Q0, sp + C0, sp + P);
        fx.vu_add4(sp + Q1, sp + C1, sp + P);
        let c = u32::from(fx.mem.read_u16(D + BURST_CELL + 2 * k));
        table_uvs(fx, sp, c);
        draw(fx, sp, 0x01E0_2D44);
    }
    let age = fx.mem.i32(D + BURST + 4) + 1;
    fx.mem.set32(D + BURST + 4, age);
    if fx.mem.i32(D + BURST + 4) >= 0x21 {
        fx.mem.write_u8(D + BURST, 0);
    }
}

/// 0x2057B0(n, pts, out) [F] (as b249.rs): out = the normalized mean of
/// the cross products of consecutive edges of the polygon `pts`.
fn normal(fx: &mut Fx, n: i32, pts: u32, out: u32) -> i32 {
    for k in 0..4u32 {
        fx.mem.write_u32(out + 4 * k, 0);
    }
    if n < 3 {
        return 0;
    }
    let sp = fx.enter(0x80);
    for i in 0..n {
        let (_, r0) = fx.div(i, n);
        let (_, r1) = fx.div(i + 1, n);
        let (_, r2) = fx.div(i + 2, n);
        let (p0, p1, p2) = (pts + 16 * r0 as u32, pts + 16 * r1 as u32, pts + 16 * r2 as u32);
        let w = fx.read_v4(0x0029_3070);
        fx.write_v4(sp + 0x50, w);
        for k in 0..3u32 {
            let x = ee::sub(fx.mem.f(p1 + 4 * k), fx.mem.f(p0 + 4 * k));
            fx.mem.setf(sp + 0x50 + 4 * k, x);
        }
        let w = fx.read_v4(0x0029_3080);
        fx.write_v4(sp + 0x60, w);
        for k in 0..3u32 {
            let x = ee::sub(fx.mem.f(p2 + 4 * k), fx.mem.f(p1 + 4 * k));
            fx.mem.setf(sp + 0x60 + 4 * k, x);
        }
        crate::blib::quadtrail::cross3(fx, sp + 0x70, sp + 0x50, sp + 0x60);
        for k in 0..3u32 {
            let x = ee::add(fx.mem.f(out + 4 * k), fx.mem.f(sp + 0x70 + 4 * k));
            fx.mem.setf(out + 4 * k, x);
        }
    }
    let nf = ee::from_i32(n);
    for k in 0..3u32 {
        let x = ee::div(fx.mem.f(out + 4 * k), nf);
        fx.mem.setf(out + 4 * k, x);
    }
    crate::blib::trail::normalize3(fx, out);
    fx.leave(0x80);
    1
}
