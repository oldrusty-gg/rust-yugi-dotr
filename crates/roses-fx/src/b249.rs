//! BMODULE b249 (card 206), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080..0x1E02D58, data area a0 =
//! 0x1ED0000, 0xF7D0 bytes, D kept in fp) [F] (helper B33, 3 Nov 2026;
//! the init map from B32). A 0x660 frame: the camera turn at sp + 0x330,
//! the view-projection at sp + 0x2B0 (a scratch matrix at sp + 0x2F0), the
//! sprite descriptor at sp + 0x470 (UVs +0x18 / +0x28), the mean w at sp +
//! 0x65C, the result at sp + 0xB0.
//!
//! Init: textures (5, 0..2) D+0x10..0x18, the attacker's nodes 0x27
//! (clear 1) / 0x1C, the target's point D+0x50 and its radius min(r, 20)
//! D+0x60; 48 x 0xF0 puff records D+0x70; a 33-point disc D+0x2D80 (radii
//! 5 / 9.8 / 10); 4 quads D+0x3C40 (corners (+-0.4, +-6 (1 - 0.4 (k & 1)))
//! turned by k 0x2000 about z); 48 x 8 x 0x20 sub-puffs D+0x3D40; a 16 x 13
//! grid D+0xC740 about the target (rows sin(row angle) radius with
//! jitter), 15 x 12 x 2 third points D+0xD440 (blib::exe::third_point),
//! 15 x 12 angle triples D+0xEAC0, 15 x 12 x 0x80 ST cells D+0x6D40 (six
//! points and two thirds), 64 cell numbers D+0xF348.
//!
//! Step: puffs spawned in 0x5C..0xEA (8 sprites each, 48 updates, every 4th
//! update a sub-puff), a flash D+0x2D70 on 0x28 (a far-Z sprite, a disc
//! band, 128 motes; 16 updates), the sub-puffs (4 quads + a sprite, 8
//! updates), the shell D+0xF330 on 0xC8 (150 updates: rows of grid cells,
//! each two triangles through the frame texture with a coloured triangle
//! below, turned by the angle triples and pushed out along their normals),
//! 64 sparks D+0xF340 on 0x13F (32 updates); the target hidden on 0xDE and
//! shown on 0x13E (0x1F7940). Returns 1 on 0x70, 6 in 0x7B..0xE7, 2 on
//! 0xE8, 8 once the count reaches 0x160.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_z, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// 0x101080's direction table (4 x 32 vec3).
const DIRS: u32 = 0x0028_AE90;
const FRAME: u32 = 0x660;

// Stack layout.
const VP: u32 = 0x2B0;
const MS: u32 = 0x2F0;
const TURN: u32 = 0x330;
const C0: u32 = 0x370;
const C1: u32 = 0x380;
const Q0: u32 = 0x3B0;
const Q1: u32 = 0x3C0;
const Q2: u32 = 0x3D0;
const Q3: u32 = 0x3E0;
const ROT: u32 = 0x3F0;
const M2: u32 = 0x400;
const P: u32 = 0x440;
const PN: u32 = 0x450;
const NRM: u32 = 0x460;
const SD: u32 = 0x470;
const QD: u32 = 0x4C8;
const BD: u32 = 0x540;
const TD: u32 = 0x598;
const FD: u32 = 0x5E0;
const W: u32 = 0x65C;

// D layout.
const PUFFS: u32 = 0x70;
const DISC: u32 = 0x2D80;
const FLASH: u32 = 0x2D70;
const MOTE_CELL: u32 = 0x33B0;
const FLASH_AT: u32 = 0x3430;
const MOTES: u32 = 0x3440;
const QUADS: u32 = 0x3C40;
const SUBS: u32 = 0x3D40;
const CELLS: u32 = 0x6D40;
const GRID: u32 = 0xC740;
const THIRDS: u32 = 0xD440;
const ANGLES: u32 = 0xEAC0;
const SHELL: u32 = 0xF330;
const SPARKS: u32 = 0xF340;
const SPARK_CELL: u32 = 0xF348;
const SPARK_DIR: u32 = 0xF3D0;

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

fn rnd(fx: &mut Fx, s0: u32, ra: u32) -> i32 {
    fx.rand_from(s0, ra) >> 23
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

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The UV pair of a descriptor's two ST slots at `a` / `a` + 0x10.
fn uvs(fx: &mut Fx, a: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, a, u0);
    uv(fx, a + 2, v0);
    uv(fx, a + 0x10, u1);
    uv(fx, a + 0x12, v1);
}

/// The sprite cell `b` (4 x n cells of 32).
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    let m = |x: i32| (x & 0x3FFF) as u32;
    uvs(fx, sp + SD + 0x18, m(c << 9), m(r << 9), m((32 * c + 32) << 4), m((32 * r + 32) << 4));
}

/// The sprite descriptor (5 registers 0x42421, prim 0x26 | 0x800), written
/// through its frame pointers.
fn sprite_head(fx: &mut Fx, sp: u32) {
    let d = sp + SD;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
}

fn alpha(fx: &mut Fx, d: u32, a: i32) {
    let p = fx.alpha_preset(a);
    fx.desc_alpha(d, p);
}

fn tex(fx: &mut Fx, d: u32, t: u32) {
    let rh = fx.mem.read_u32(D + t);
    fx.desc_texture(d, rh, 1);
}

/// sp + 0x2B0 = the view-projection x the matrix at sp + 0x2F0.
fn vp_times(fx: &mut Fx, sp: u32) {
    let m = fx.read_m4(sp + MS);
    fx.mul_into(Some(sp + VP), &m);
}

fn vp_identity(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MS);
    vp_times(fx, sp);
}

/// The corners (-k, -k, 0, 1) / (k, k, 0, 1), turned by the camera when
/// `turn`.
fn corners(fx: &mut Fx, sp: u32, k: f32, turn: bool) {
    let n = ee::neg(k);
    vec4(fx, sp + C0, n, n, 0.0);
    vec4(fx, sp + C1, k, k, 0.0);
    if turn {
        fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
        fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
    }
}

/// The descriptor `d` projected from sp + 0x3B0; submitted at the mean w
/// (`ra`), or at far Z (the XYZ2s `xyz`) at key 0.
fn draw(fx: &mut Fx, sp: u32, d: u32, ra: Option<u32>, xyz: &[u32]) {
    if crate::fxlib2::project(fx, sp + Q0, PROJ_OUT, sp + VP, d, sp + W) {
        match ra {
            Some(ra) => {
                let w = fx.mem.f(sp + W);
                let key = fixu(fx, w, ra) as i32;
                fx.submit(d, OT, key);
            }
            None => {
                far_z(fx, xyz[0], &xyz[1..]);
                fx.submit(d, OT, 0);
            }
        }
    }
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0xF7D0);
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
    let r = fx.request_node(h, 0x27, 1);
    fx.mem.set32(D + 0x1C, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x1C, 0);
    fx.mem.set32(D + 0x30, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x50);
    let t = fx.mem.i32(D + 8);
    let r = crate::blib::afterimage::unit_radius(fx, t);
    fx.mem.setf(D + 0x60, r);
    if !crate::exe::ee_le(r, 20.0) {
        fx.mem.setf(D + 0x60, 20.0);
    }
    for k in 0..0x30u32 {
        fx.mem.write_u8(D + PUFFS + 0xF0 * k, 0);
        fx.mem.write_u32(D + PUFFS + 0xE0 + 0xF0 * k, 0);
    }
    fx.mem.write_u8(D + FLASH, 0);
    // The disc: 33 points at k / 32 of a turn, radii 5 / 9.8 / 10.
    for k in 0..0x21u32 {
        let a = ((k << 16) as i32) >> 5;
        let s = fx.tsin(a);
        fx.mem.setf(sp + P, s);
        let c = fx.tcos(a);
        fx.mem.setf(sp + P + 4, c);
        fx.mem.write_u32(sp + P + 8, 0);
        fx.mem.write_u32(sp + P + 0xC, 0x3F80_0000);
        let p = D + DISC + 0x30 * k;
        fx.vu_scale4(p, sp + P, 5.0);
        fx.vu_scale4(p + 0x10, sp + P, f(0x411C_CCCD));
        fx.vu_scale4(p + 0x20, sp + P, 10.0);
    }
    // The four quads.
    let x = ee::neg(f(0x3ECC_CCCD));
    for k in 0..4i32 {
        let y = ee::mul(6.0, ee::sub(1.0, ee::mul(f(0x3ECC_CCCD), ee::from_i32(rem2(k, 2)))));
        let ny = ee::neg(y);
        vec4(fx, sp + C0, x, ny, 0.0);
        vec4(fx, sp + C1, f(0x3ECC_CCCD), ny, 0.0);
        vec4(fx, sp + 0x390, x, y, 0.0);
        vec4(fx, sp + 0x3A0, f(0x3ECC_CCCD), y, 0.0);
        fx.identity(sp + M2);
        fx.rotate_axis(sp + M2, (k << 15) >> 2, 2);
        for j in 0..4u32 {
            fx.vu_apply_to(D + QUADS + 0x40 * k as u32 + 0x10 * j, sp + M2, sp + C0 + 0x10 * j);
        }
    }
    for i in 0..0x30u32 {
        for j in 0..8u32 {
            fx.mem.write_u8(D + SUBS + 0x100 * i + 0x20 * j, 0);
        }
    }
    // The grid: 16 rows of 13 points (point 12 = point 0).
    for row in 0..0x10i32 {
        let ang = row << 15;
        let s2 = ang / 15;
        for pt in 0..0xDi32 {
            let s0 = (pt as u32) << 16;
            let sn = fx.tsin(s2 & 0xFFFF);
            let mut f21 = ee::mul(fx.mem.f(D + 0x60), sn);
            let b = rem2(rnd(fx, s0, 0x01E0_03D4), 8);
            f21 = ee::add(f21, ee::div(ee::mul(ee::mul(0.5, f21), ee::from_i32(b)), 8.0));
            let r60 = fx.mem.f(D + 0x60);
            let f3 = ee::mul(5.0, r60);
            let mut f20 = ee::add(ee::div(ee::neg(f3), 2.0), ee::div(ee::mul(f3, ee::from_i32(row)), 15.0));
            if row != 0 && row != 15 {
                let b = rem2(rnd(fx, s0, 0x01E0_0478), 8);
                let r60 = fx.mem.f(D + 0x60);
                let k = ee::div(ee::div(ee::mul(5.0, r60), 15.0), 2.0);
                f20 = ee::add(f20, ee::div(ee::mul(k, ee::sub(4.0, ee::from_i32(b))), 4.0));
            }
            let b = rem2(rnd(fx, s0, 0x01E0_04E8), 8);
            let a = ((s0 as i32 / 12) + (b.wrapping_mul(0xAAA) >> 3)) & 0xFFFF;
            let p = D + GRID + 0xD0 * row as u32 + 0x10 * pt as u32;
            let sn = fx.tsin(a);
            fx.mem.setf(p, ee::mul(f21, sn));
            fx.mem.setf(p + 4, f20);
            let cs = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(f21, cs));
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
        let p = D + GRID + 0xD0 * row as u32;
        copy4(fx, p + 0xC0, p);
    }
    // The third points: two a cell.
    for row in 0..0xFu32 {
        fx.mem.write_u32(sp + 0x270, row);
        fx.mem.write_u32(sp + 0xC0, D + 0x180 * row);
        let g = |r: u32, c: u32| D + GRID + 0xD0 * r + 0x10 * c;
        for c in 0..0xCu32 {
            let out = D + THIRDS + 0x180 * row + 0x20 * c;
            crate::blib::exe::third_point(fx, out, g(row, c), g(row + 1, c), g(row + 1, c + 1));
            crate::blib::exe::third_point(fx, out + 0x10, g(row, c), g(row, c + 1), g(row + 1, c + 1));
        }
        fx.mem.write_u32(sp + 0xC0, D + 0x180 * (row + 1));
        fx.mem.write_u32(sp + 0x270, row + 1);
    }
    // The angle triples.
    for row in 0..0xFu32 {
        for c in 0..0xCu32 {
            let a = D + ANGLES + 0x90 * row + 0xC * c;
            let b = rem2(rnd(fx, OT, 0x01E0_06C8), 0x100);
            let s0 = b.wrapping_mul(0x14D) >> 8;
            let m = (rem2(rnd(fx, s0 as u32, 0x01E0_06F0), 2) - 1) << 1;
            fx.mem.set32(a, s0.wrapping_mul(m) & 0xFFFF);
            let b = rem2(rnd(fx, s0 as u32, 0x01E0_0728), 0x100);
            let s0 = b.wrapping_mul(0xDE) >> 8;
            let m = (rem2(rnd(fx, s0 as u32, 0x01E0_075C), 2) - 1) << 1;
            fx.mem.set32(a + 4, s0.wrapping_mul(m) & 0xFFFF);
            let b = rem2(rnd(fx, s0 as u32, 0x01E0_0794), 0x100);
            let s0 = b.wrapping_mul(0x6F) >> 8;
            let m = (rem2(rnd(fx, s0 as u32, 0x01E0_07C4), 2) - 1) << 1;
            fx.mem.set32(a + 8, s0.wrapping_mul(m) & 0xFFFF);
        }
    }
    fx.mem.write_u8(D + SHELL, 0);
    // The ST cells: six points about a random centre, two thirds.
    for row in 0..0xFu32 {
        for c in 0..0xCu32 {
            let cell = D + CELLS + 0x600 * row + 0x80 * c;
            let lin = |b: i32| ee::div(ee::add(64.0, ee::div(ee::mul(384.0, ee::from_i32(b)), 256.0)), 512.0);
            let b = rem2(rnd(fx, OT, 0x01E0_0840), 0x100);
            let f20 = lin(b);
            let b = rem2(rnd(fx, OT, 0x01E0_08A8), 0x100);
            let f21 = lin(b);
            for j in 0..6u32 {
                let jit = |b: i32| ee::mul(0.25, ee::add(-0.5, ee::div(ee::from_i32(b), 256.0)));
                let b = rem2(rnd(fx, j, 0x01E0_0910), 0x100);
                let p = cell + 0x10 * j;
                fx.mem.setf(p, ee::add(f20, jit(b)));
                let b = rem2(rnd(fx, j, 0x01E0_0968), 0x100);
                fx.mem.setf(p + 4, ee::add(f21, jit(b)));
                fx.mem.write_u32(p + 8, 0);
                fx.mem.write_u32(p + 0xC, 0x3F80_0000);
            }
            crate::blib::exe::third_point(fx, cell + 0x60, cell, cell + 0x10, cell + 0x20);
            crate::blib::exe::third_point(fx, cell + 0x70, cell + 0x30, cell + 0x40, cell + 0x50);
        }
    }
    for k in 0..0x40u32 {
        let b = rem2(rnd(fx, D + 2 * k, 0x01E0_0A30), 8);
        fx.mem.write_u16(D + SPARK_CELL + 2 * k, b as u16);
    }
    fx.mem.write_u8(D + SPARKS, 0);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x1C));
    fx.node_position(h, s, D + 0x20);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x30));
    fx.node_position(h, s, D + 0x40);

    let n = fx.mem.i32(D);
    if (0x5C..0xEB).contains(&n) {
        spawn_puff(fx, n);
    }
    puffs(fx, sp);

    if fx.mem.i32(D) == 0x28 {
        fx.mem.write_u8(D + FLASH, 1);
        fx.mem.write_u32(D + FLASH + 4, 0);
        copy4(fx, D + FLASH_AT, D + 0x40);
        for k in 0..0x80u32 {
            let b = rem2(rnd(fx, OT, 0x01E0_1120), 4);
            fx.mem.write_u8(D + MOTE_CELL + k, b as u8);
            fx.random_table_vec(D + MOTES + 0x10 * k, f(0x3E99_999A));
        }
    }
    if fx.mem.read_u8(D + FLASH) == 1 {
        flash(fx, sp);
    }
    subs(fx, sp);

    if fx.mem.i32(D) == 0xC8 {
        fx.mem.write_u8(D + SHELL, 1);
        fx.mem.write_u32(D + SHELL + 4, 0);
    }
    if fx.mem.read_u8(D + SHELL) == 1 {
        shell(fx, sp);
    }

    if fx.mem.i32(D) == 0x13F {
        fx.mem.write_u8(D + SPARKS, 1);
        fx.mem.write_u32(D + SPARKS + 4, 0);
        for k in 0..0x40u32 {
            let s2 = rem2(rnd(fx, OT, 0x01E0_27B4), 0x20);
            let o = D + SPARK_DIR + 0x10 * k;
            for (c, ra) in [0x01E0_27D4u32, 0x01E0_2844, 0x01E0_28A8].into_iter().enumerate() {
                let row = rem2(rnd(fx, OT, ra), 4);
                let p = DIRS + 0x180 * row as u32 + 12 * s2 as u32 + 4 * c as u32;
                let r60 = fx.mem.f(D + 0x60);
                let v = ee::mul(ee::mul(f(0x3E4C_CCCD), r60), fx.mem.f(p));
                fx.mem.setf(o + 4 * c as u32, v);
            }
            fx.mem.write_u32(o + 0xC, 0x3F80_0000);
        }
    }
    sparks(fx, sp);

    if fx.mem.i32(D) == 0xDE {
        let t = fx.mem.i32(D + 8);
        fx.unit_show(t, 0, 0x01E0_2C88);
    }
    if fx.mem.i32(D) == 0x13E {
        let t = fx.mem.i32(D + 8);
        fx.unit_show(t, 1, 0x01E0_2CA4);
    }

    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x70 {
        ret = 1;
    } else {
        if (0x7B..0xE8).contains(&n) {
            ret = 6;
        }
        if n == 0xE8 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x160 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}

/// A puff record (D+0x70 + 0xF0 (n % 48)) at the first node, lifted 4 x
/// the side: 8 cells, sizes 0.8 + 0.2 b / 256 and directions (a table
/// vector + the target - the point).
fn spawn_puff(fx: &mut Fx, n: i32) {
    let (_, r) = fx.div(n, 0x30);
    let rec = D + PUFFS + 0xF0 * r as u32;
    fx.mem.write_u8(rec, 1);
    fx.mem.write_u32(rec + 4, 0);
    copy4(fx, rec + 0x50, D + 0x20);
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    let y = fx.mem.f(rec + 0x58);
    fx.mem.setf(rec + 0x58, ee::add(y, ee::mul(4.0, side)));
    for j in 0..8u32 {
        let b = rem2(rnd(fx, OT, 0x01E0_0B3C), 8);
        fx.mem.set32(rec + 8 + 4 * j, b);
        let b = rem2(rnd(fx, OT, 0x01E0_0B5C), 0x100);
        fx.mem.setf(rec + 0x28 + 4 * j, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(b)), 256.0)));
        let d = rec + 0x60 + 0x10 * j;
        fx.random_table_vec(d, 1.0);
        fx.vu_add4(d, d, D + 0x50);
        fx.vu_sub4(d, d, rec + 0x50);
    }
}

/// The puffs (48 updates) and their sub-puff spawns [F 0x1E00BFC..0x1E010D0].
fn puffs(fx: &mut Fx, sp: u32) {
    let d = sp + SD;
    fx.mem.write_u32(sp + 0x1A0, d + 2);
    sprite_head(fx, sp);
    fx.mem.write_u32(sp + 0x1B0, d + 8);
    fx.mem.write_u32(sp + 0x1C0, d + 4);
    tex(fx, d, 0x10);
    fx.desc_clamp(d, 5);
    alpha(fx, d, 1);
    vp_identity(fx, sp);
    fx.mem.write_u32(sp + 0x1D0, d + 0x1A);
    fx.mem.write_u32(sp + 0x1E0, d + 0x2A);
    for i in 0..0x30u32 {
        let rec = D + PUFFS + 0xF0 * i;
        if fx.mem.read_u8(rec) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(rec + 4));
        let f20 = ee::mul(80.0, Fx::ratio_below(age, 2.0));
        let age = ee::from_i32(fx.mem.i32(rec + 4));
        let r = Fx::ramp_above(age, 48.0, 32.0);
        let al = fixu(fx, ee::mul(f20, r), 0x01E0_0CFC) as u8;
        rgba(fx, d + 0x10, [0x80, 0x80, 0x80], al);
        let age = ee::from_i32(fx.mem.i32(rec + 4));
        let k = ee::mul(12.0, ee::add(f(0x3ECC_CCCD), ee::div(ee::mul(f(0x3F19_999A), age), 48.0)));
        corners(fx, sp, k, true);
        let age = ee::from_i32(fx.mem.i32(rec + 4));
        let f20 = ee::div(ee::mul(1.5, age), 48.0);
        let a = fixs(fx, ee::mul(32768.0, f20), 0x01E0_0DDC) & 0xFFFF;
        let s = fx.tsin(a);
        let f21 = ee::mul(-10.0, s);
        for j in 0..8u32 {
            let b = fx.mem.i32(rec + 8 + 4 * j);
            cell_uvs(fx, sp, b);
            let k = fx.mem.f(rec + 0x28 + 4 * j);
            fx.vu_scale4(sp + Q0, sp + C0, k);
            let k = fx.mem.f(rec + 0x28 + 4 * j);
            fx.vu_scale4(sp + Q1, sp + C1, k);
            fx.vu_scale4(sp + P, rec + 0x60 + 0x10 * j, f20);
            let y = fx.mem.f(sp + P + 4);
            fx.mem.setf(sp + P + 4, ee::add(y, f21));
            fx.vu_add4(sp + P, sp + P, rec + 0x50);
            fx.vu_add4(sp + Q0, sp + Q0, sp + P);
            fx.vu_add4(sp + Q1, sp + Q1, sp + P);
            draw(fx, sp, d, Some(0x01E0_0F54), &[]);
        }
        if rem2(fx.mem.i32(rec + 4), 4) == 0 {
            let v1 = fx.mem.i32(rec + 0xE0);
            fx.mem.set32(rec + 0xE0, v1 + 1);
            if fx.mem.i32(rec + 0xE0) >= 8 {
                fx.mem.write_u32(rec + 0xE0, 0);
            }
            let sub = D + SUBS + 0x100 * i + (v1 << 5) as u32;
            fx.mem.write_u8(sub, 1);
            fx.mem.write_u32(sub + 4, 0);
            let b = rem2(rnd(fx, OT, 0x01E0_0FD8), 0x100);
            let f3 = ee::add(f(0x3DCC_CCCD), ee::div(ee::mul(f(0x3F66_6666), ee::from_i32(b)), 256.0));
            let age = ee::from_i32(fx.mem.i32(rec + 4));
            let k = ee::add(0.5, ee::div(ee::mul(0.5, age), 48.0));
            fx.mem.setf(sub + 8, ee::mul(k, f3));
            let b = rem2(rnd(fx, OT, 0x01E0_1058), 8);
            fx.mem.set32(sub + 0xC, (b << 16) >> 3);
            fx.random_table_vec(sub + 0x10, 0.5);
            fx.vu_add4(sub + 0x10, sub + 0x10, sp + P);
        }
        let age = fx.mem.i32(rec + 4) + 1;
        fx.mem.set32(rec + 4, age);
        if age >= 0x31 {
            fx.mem.write_u8(rec, 0);
        }
    }
}

/// The flash at node 1 (16 updates): a far-Z sprite, the disc band and
/// 128 motes [F 0x1E01178..0x1E01A20].
fn flash(fx: &mut Fx, sp: u32) {
    let d = sp + SD;
    sprite_head(fx, sp);
    tex(fx, d, 0x14);
    alpha(fx, d, 1);
    fx.desc_clamp(d, 5);
    uvs(fx, d + 0x18, 0x600, 0, 0x800, 0x200);
    fx.camera_vp_to(sp + VP);
    fx.translate_rows(sp + MS, sp + TURN, D + FLASH_AT);
    vp_times(fx, sp);
    let age = fx.mem.i32(D + FLASH + 4);
    let t = fx.tan_falloff_p(age, 0x10, 1);
    let al = fixu(fx, ee::mul(255.0, ee::sub(1.0, t)), 0x01E0_12AC) as u8;
    rgba(fx, d + 0x10, [0xB4, 0xC8, 0xFF], al);
    vec4(fx, sp + C0, -12.0, -12.0, 0.0);
    vec4(fx, sp + C1, 12.0, 12.0, 0.0);
    if crate::fxlib2::project(fx, sp + C0, PROJ_OUT, sp + VP, d, sp + W) {
        far_z(fx, d + 0x20, &[d + 0x30]);
        fx.submit(d, OT, 0);
    }

    // The disc band: 32 x 2 quads, (B4, C8, FF) inside to (0, 0, FF, 0).
    let b = sp + BD;
    fx.mem.write_u16(b + 2, 0x14);
    fx.mem.write_u16(b, 8);
    fx.mem.write_u64(b + 8, 0x4141_4141);
    fx.mem.write_u32(b + 4, 0);
    alpha(fx, b, 1);
    fx.camera_vp_to(sp + VP);
    fx.translate_rows(sp + MS, sp + TURN, D + FLASH_AT);
    let age = fx.mem.i32(D + FLASH + 4);
    let f20 = fx.tan_falloff_p(age, 0x10, 2);
    vec4(fx, sp + P, f20, f20, 0.0);
    fx.scale_rows(sp + MS, sp + P);
    vp_times(fx, sp);
    let age = fx.mem.i32(D + FLASH + 4);
    let v = 0x10 - age;
    let a = ((v << 8) - v) >> 4;
    rgba(fx, b + 0x10, [0xB4, 0xC8, 0xFF], a as u8);
    let (x, y) = (fx.mem.read_u32(b + 0x10), fx.mem.read_u32(b + 0x14));
    fx.mem.write_u32(b + 0x20, x);
    fx.mem.write_u32(b + 0x24, y);
    rgba(fx, b + 0x30, [0, 0, 0xFF], 0);
    let (x, y) = (fx.mem.read_u32(b + 0x30), fx.mem.read_u32(b + 0x34));
    fx.mem.write_u32(b + 0x40, x);
    fx.mem.write_u32(b + 0x44, y);
    for k in 0..0x20u32 {
        let p = D + DISC + 0x30 * k;
        let q = D + DISC + 0x30 * (k + 1);
        for (vs, ra) in [([p + 0x10, q + 0x10, p, q], 0x01E0_1528u32), ([p + 0x10, q + 0x10, p + 0x20, q + 0x20], 0x01E0_1620)] {
            if fx.project_n(&vs, sp + VP, b, sp + W, ra) == 1 {
                far_z(fx, b + 0x18, &[b + 0x28, b + 0x38, b + 0x48]);
                fx.submit(b, OT, 0);
            }
        }
    }

    // The motes.
    sprite_head(fx, sp);
    tex(fx, d, 0x14);
    alpha(fx, d, 1);
    fx.desc_clamp(d, 5);
    vp_identity(fx, sp);
    let age = fx.mem.i32(D + FLASH + 4);
    let v = 0x10 - age;
    let a = ((v << 8) - v) >> 4;
    rgba(fx, d + 0x10, [0x84, 0xA4, 0xFF], a as u8);
    corners(fx, sp, 0.5, true);
    for k in 0..0x80u32 {
        let b = i32::from(fx.mem.read_u8(D + MOTE_CELL + k));
        cell_uvs(fx, sp, b);
        fx.vu_scale4(sp + P, D + MOTES + 0x10 * k, f20);
        fx.vu_add4(sp + P, sp + P, D + FLASH_AT);
        fx.vu_add4(sp + Q0, sp + C0, sp + P);
        fx.vu_add4(sp + Q1, sp + C1, sp + P);
        draw(fx, sp, d, None, &[d + 0x20, d + 0x30]);
    }
    let age = fx.mem.i32(D + FLASH + 4) + 1;
    fx.mem.set32(D + FLASH + 4, age);
    if age >= 0x11 {
        fx.mem.write_u8(D + FLASH, 0);
    }
}

/// The sub-puffs (8 updates): the four quads and a sprite [F
/// 0x1E01A24..0x1E01F34].
fn subs(fx: &mut Fx, sp: u32) {
    let q = sp + QD;
    fx.mem.write_u16(q + 2, 0x24);
    fx.mem.write_u16(q, 9);
    fx.mem.write_u64(q + 8, 0x4_2424_2421);
    fx.mem.write_u32(q + 4, 0);
    let fl = fx.mem.read_u16(q + 2);
    fx.mem.write_u16(q + 2, fl | 0x800);
    alpha(fx, q, 1);
    tex(fx, q, 0x14);
    fx.desc_clamp(q, 5);
    for (a, u, v) in [(q + 0x18, 0x600, 0x200), (q + 0x28, 0x800, 0x200), (q + 0x38, 0x600, 0x400), (q + 0x48, 0x800, 0x400)] {
        uv(fx, a, u);
        uv(fx, a + 2, v);
    }
    let d = sp + SD;
    sprite_head(fx, sp);
    alpha(fx, d, 1);
    tex(fx, d, 0x14);
    fx.desc_clamp(d, 5);
    uvs(fx, d + 0x18, 0x600, 0x200, 0x800, 0x400);
    vp_identity(fx, sp);
    for i in 0..0x30u32 {
        for j in 0..8u32 {
            let s = D + SUBS + 0x100 * i + 0x20 * j;
            if fx.mem.read_u8(s) == 0 {
                continue;
            }
            let age = fx.mem.i32(s + 4);
            let c = fx.tcos((age << 14) >> 3);
            let al = fixu(fx, ee::mul(64.0, c), 0x01E0_1C7C) as u8;
            rgba(fx, q + 0x10, [0xAA, 0xEB, 0xFF], al);
            for k in 0..4u32 {
                copy4(fx, sp + M2 + 0x10 * k, sp + TURN + 0x10 * k);
            }
            let size = fx.mem.f(s + 8);
            for k in 0..4u32 {
                for c in 0..4u32 {
                    let o = sp + C0 + 0x10 * c;
                    fx.vu_scale4(o, D + QUADS + 0x40 * k + 0x10 * c, size);
                    fx.vu_apply_to(o, sp + M2, o);
                    fx.vu_add4(sp + Q0 + 0x10 * c, o, s + 0x10);
                }
                draw(fx, sp, q, Some(0x01E0_1D68), &[]);
            }
            let k = ee::mul(2.0, fx.mem.f(s + 8));
            corners(fx, sp, k, true);
            let age = fx.mem.i32(s + 4);
            let c = fx.tcos((age << 14) >> 3);
            let al = fixu(fx, ee::mul(32.0, c), 0x01E0_1E0C) as u8;
            rgba(fx, d + 0x10, [0xAA, 0xEB, 0xFF], al);
            copy4(fx, sp + P, s + 0x10);
            fx.vu_add4(sp + Q0, sp + C0, sp + P);
            fx.vu_add4(sp + Q1, sp + C1, sp + P);
            draw(fx, sp, d, Some(0x01E0_1EA4), &[]);
            let age = fx.mem.i32(s + 4) + 1;
            fx.mem.set32(s + 4, age);
            if age >= 9 {
                fx.mem.write_u8(s, 0);
            }
        }
    }
}

/// The shell (150 updates): rows of grid cells, each two triangles (the
/// frame as their texture, ST from the cells) over a coloured triangle,
/// turned by the cell's angles x k and pushed out along their normal [F
/// 0x1E01F80..0x1E02774].
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
    let a0 = if age < 0x21 { age } else { 0x20 };
    let mut rows = (a0 * 15) >> 5;
    if rows >= 0x10 {
        rows = 0xF;
    }
    fx.mem.set32(sp + 0x280, rows);
    let v = 0x96 - age;
    let v = if v < 0x21 { v } else { 0x20 };
    let k = 0x20 - v;
    fx.mem.set32(sp + 0x1F0, k);
    let t = fx.tan_falloff_p(k, 0x20, 1);
    let f20 = ee::mul(ee::mul(3.0, fx.mem.f(D + 0x60)), t);
    let r = Fx::ramp_above(ee::from_i32(k), 32.0, 16.0);
    let al1 = fixu(fx, ee::mul(128.0, r), 0x01E0_20DC) as u8;
    rgba(fx, fd + 0x10, [0x80, 0x80, 0x80], 0);
    rgba(fx, fd + 0x28, [0x80, 0x80, 0x80], al1);
    let (x, y) = (fx.mem.read_u32(fd + 0x28), fx.mem.read_u32(fd + 0x2C));
    fx.mem.write_u32(fd + 0x40, x);
    fx.mem.write_u32(fd + 0x44, y);
    let r = Fx::ramp_above(ee::from_i32(k), 32.0, 16.0);
    let al2 = fixu(fx, ee::mul(80.0, r), 0x01E0_2154) as u8;
    rgba(fx, td + 0x10, [0, 0, 0], 0);
    rgba(fx, td + 0x20, [0xD7, 0xEB, 0xFF], al2);
    let (x, y) = (fx.mem.read_u32(td + 0x20), fx.mem.read_u32(td + 0x24));
    fx.mem.write_u32(td + 0x30, x);
    fx.mem.write_u32(td + 0x34, y);
    let g = |r: u32, c: u32| D + GRID + 0xD0 * r + 0x10 * c;
    for row in 0..rows.max(0) as u32 {
        fx.mem.write_u32(sp + 0x250, row);
        for c in 0..0xCu32 {
            let ang = D + ANGLES + 0x90 * row + 0xC * c;
            for j in 0..3u32 {
                let a = fx.mem.i32(ang + 4 * j);
                fx.mem.set32(sp + ROT + 4 * j, k.wrapping_mul(a) & 0xFFFF);
            }
            fx.mem.write_u32(sp + ROT + 0xC, 1);
            let cell = D + CELLS + 0x600 * row + 0x80 * c;
            for h in 0..2u32 {
                let t = D + THIRDS + 0x180 * row + 0x20 * c + 0x10 * h;
                fx.vu_sub4(sp + Q0, t, t);
                fx.vu_sub4(sp + Q1, g(row, c), t);
                let gr = if h == 0 { g(row + 1, c) } else { g(row, c + 1) };
                fx.vu_sub4(sp + Q2, gr, t);
                fx.vu_sub4(sp + Q3, g(row + 1, c + 1), t);
                normal(fx, 3, sp + Q1, sp + NRM);
                let sgn = if h == 0 { -1.0 } else { 1.0 };
                fx.vu_scale4(sp + PN, sp + NRM, ee::mul(f20, sgn));
                fx.vu_add4(sp + PN, sp + PN, D + 0x50);
                fx.vu_add4(sp + PN, sp + PN, t);
                fx.camera_vp_to(sp + VP);
                let rot = std::array::from_fn(|j| fx.mem.i32(sp + ROT + 4 * j as u32));
                let tr = std::array::from_fn(|j| fx.mem.f(sp + PN + 4 * j as u32));
                let m = fx.trs(None, Some(rot), Some(tr));
                fx.write_m4(sp + MS, &m);
                vp_times(fx, sp);
                let s5 = cell + 0x30 * h;
                let mid = cell + 0x60 + 0x10 * h;
                let tris: [([u32; 3], u32, u32, [u32; 3]); 3] = [
                    ([Q0, Q1, Q2], s5, s5 + 0x10, [0x01E0_244C, 0x01E0_2490, 0x01E0_24A8]),
                    ([Q0, Q2, Q3], s5 + 0x10, s5 + 0x20, [0x01E0_2534, 0x01E0_2578, 0x01E0_2590]),
                    ([Q0, Q3, Q1], s5 + 0x20, s5, [0x01E0_2610, 0x01E0_2654, 0x01E0_266C]),
                ];
                for (vs, st1, st2, ras) in tris {
                    for (o, from) in [(fd + 0x18, mid), (fd + 0x30, st1), (fd + 0x48, st2)] {
                        let (x, y) = (fx.mem.read_u32(from), fx.mem.read_u32(from + 4));
                        fx.mem.write_u32(o, x);
                        fx.mem.write_u32(o + 4, y);
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
    if age >= 0x97 {
        fx.mem.write_u8(D + SHELL, 0);
    }
}

/// The sparks at the target (32 updates) [F 0x1E02928..0x1E02C68].
fn sparks(fx: &mut Fx, sp: u32) {
    let d = sp + SD;
    sprite_head(fx, sp);
    alpha(fx, d, 1);
    tex(fx, d, 0x10);
    fx.desc_clamp(d, 5);
    fx.camera_vp_to(sp + VP);
    let tr = std::array::from_fn(|j| fx.mem.f(D + 0x50 + 4 * j as u32));
    let m = fx.trs(None, None, Some(tr));
    fx.write_m4(sp + MS, &m);
    vp_times(fx, sp);
    if fx.mem.read_u8(D + SPARKS) != 1 {
        return;
    }
    let half = ee::mul(0.5, fx.mem.f(D + 0x60));
    let age = ee::from_i32(fx.mem.i32(D + SPARKS + 4));
    let k = ee::add(half, ee::div(ee::mul(half, age), 32.0));
    corners(fx, sp, k, true);
    let age = ee::from_i32(fx.mem.i32(D + SPARKS + 4));
    let r = Fx::ramp_above(age, 32.0, 16.0);
    let al = fixu(fx, ee::mul(200.0, r), 0x01E0_2A94) as u8;
    rgba(fx, d + 0x10, [0xC8, 0xC8, 0xC8], al);
    let age = ee::from_i32(fx.mem.i32(D + SPARKS + 4));
    let f20 = ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0));
    for k in 0..0x40u32 {
        fx.vu_scale4(sp + P, D + SPARK_DIR + 0x10 * k, f20);
        fx.vu_add4(sp + Q0, sp + C0, sp + P);
        fx.vu_add4(sp + Q1, sp + C1, sp + P);
        let b = i32::from(fx.mem.read_u16(D + SPARK_CELL + 2 * k));
        cell_uvs(fx, sp, b);
        draw(fx, sp, d, Some(0x01E0_2C08), &[]);
    }
    let age = fx.mem.i32(D + SPARKS + 4) + 1;
    fx.mem.set32(D + SPARKS + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + SPARKS, 0);
    }
}

/// 0x2057B0(n, pts, out) [F] (as b222.rs): out = the normalized mean of
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
