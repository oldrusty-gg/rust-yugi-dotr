//! BMODULE b754 (card 136), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xA630
//! bytes) [F]: b769's (card 34) fan / spark /
//! mote / ring / flat / frame-sprite parts in a 0x470 frame (D kept in s1;
//! the camera at sp + 0x1E0, the turn at sp + 0x260, the sprite descriptor
//! at sp + 0x300, the fan's at sp + 0x358, the rings' at sp + 0x3D0).
//! Every update the camera turn with roll, the attacker's nodes 0x22
//! (clear 1) / 0x23 into D+0x30 / 0x40 (D+0x50 their difference), its
//! point into D+0x70 with its screen point (D+0x80) and the Z of the point
//! 1.5 radii in front (D+0x94). Twelve fans (D+0x98, 13 updates) in
//! 0x1F..0x197; a 16-sprite line along the nodes (D+0xF8, far Z) from
//! 0x84 for 0x114 updates; 32 x 0x160 4 x 4 sprite groups along the nodes
//! (D+0x100, 32 updates) in 0x85..0x197; 64 x 0x100 sparks (D+0x2D00) and
//! 64 x 0xB0 motes (D+0x6D00) pulled to the target (64 updates) in
//! 0x101..0x197; 32 rings (D+0x9C30, far Z) between radii 30 / 80 at the
//! target in 0x129..0x185; a flat sprite at the target and a full-window
//! frame sprite in 0x129..0x1A5. The stage light 0x80 - 96 clamp(n)
//! clamp(0x1C6 - n) / 1024. Returns 1 on 0x11E, 6 in 0x129..0x1A5, 2 on
//! 0x1A6, 8 once the count reaches 0x1C7.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, KEEP, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The fan's nine screen points (exe rodata, 0x10 apart).
const FAN: u32 = 0x0028_ABE0;
const FRAME: u32 = 0x470;

// Stack layout.
const CAM: u32 = 0x1E0;
const IDM: u32 = 0x220;
const TURN: u32 = 0x260;
const C0: u32 = 0x2A0;
const C1: u32 = 0x2B0;
const Q0: u32 = 0x2C0;
const Q1: u32 = 0x2D0;
const P: u32 = 0x2E0;
const PULL: u32 = 0x2F0;
const SD: u32 = 0x300;
const FD: u32 = 0x358;
const RD: u32 = 0x3D0;
const SCR: u32 = 0x460;
const W: u32 = 0x46C;

// D layout.
const FANS: u32 = 0x98;
const LINE: u32 = 0xF8;
const GROUPS: u32 = 0x100;
const SPARKS: u32 = 0x2D00;
const MOTES: u32 = 0x6D00;
const RING_PTS: u32 = 0x9900;
const RINGS: u32 = 0x9C30;

// Return addresses.
const R_FAN_A: [u32; 2] = [0x01E0_06D0, 0x01E0_0740];
const R_FAN: [u32; 12] = [
    0x01E0_07E8, 0x01E0_07F4, 0x01E0_0838, 0x01E0_0844, 0x01E0_0888, 0x01E0_0894, 0x01E0_0948, 0x01E0_0954, 0x01E0_09BC, 0x01E0_09C8, 0x01E0_0A34,
    0x01E0_0A40,
];
const R_LINE: u32 = 0x01E0_0D64;
const RR_GROUP: u32 = 0x01E0_0FB0;
const R_GROUP: [u32; 2] = [0x01E0_1144, 0x01E0_1340];
const RR_SPARK: [u32; 2] = [0x01E0_14A4, 0x01E0_14C4];
const R_SPARK: [u32; 2] = [0x01E0_1878, 0x01E0_19C4];
const RR_MOTE: [u32; 2] = [0x01E0_1AF0, 0x01E0_1B10];
const R_MOTE: [u32; 2] = [0x01E0_1D98, 0x01E0_1F2C];
const RR_RING: [u32; 3] = [0x01E0_2008, 0x01E0_203C, 0x01E0_2070];
const R_RING: u32 = 0x01E0_2198;
const RP_RING: [u32; 2] = [0x01E0_2458, 0x01E0_25AC];
const R_FLAT: u32 = 0x01E0_28F4;
const R_FRAME: u32 = 0x01E0_2B20;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xA630);
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
    for (i, (node, clear)) in [(0x22, 1), (0x23, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0x1C + 4 * i as u32, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x60);
    let h = fx.mem.i32(D + 4);
    let r = crate::blib::afterimage::unit_radius(fx, h);
    fx.mem.setf(D + 0x90, ee::mul(1.5, r));
    for k in 0..12u32 {
        fx.mem.write_u8(D + FANS + 8 * k, 0);
    }
    fx.mem.write_u8(D + LINE, 0);
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + GROUPS + 0x160 * k, 0);
    }
    for k in 0..0x40u32 {
        fx.mem.write_u8(D + SPARKS + 0x100 * k, 0);
    }
    for k in 0..0x40u32 {
        fx.mem.write_u8(D + MOTES + 0xB0 * k, 0);
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

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + SD + 0x18, u0);
    uv(fx, sp + SD + 0x1A, v0);
    uv(fx, sp + SD + 0x28, u1);
    uv(fx, sp + SD + 0x2A, v1);
}

/// The sprite cell `b` (4 x n cells of 32).
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    let m = |x: i32| (x & 0x3FFF) as u32;
    uvs(fx, sp, m(c << 9), m(r << 9), m((32 * c + 32) << 4), m((32 * r + 32) << 4));
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

fn vec4(fx: &mut Fx, at: u32, x: f32, y: f32, z: f32) {
    fx.mem.setf(at, x);
    fx.mem.setf(at + 4, y);
    fx.mem.setf(at + 8, z);
    fx.mem.write_u32(at + 0xC, 0x3F80_0000);
}

/// The view-projection (x identity).
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + CAM);
    fx.identity(sp + IDM);
    let id = fx.read_m4(sp + IDM);
    fx.mul_into(Some(sp + CAM), &id);
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

fn alpha(fx: &mut Fx, sp: u32, a: i32) {
    let p = fx.alpha_preset(a);
    fx.desc_alpha(sp + SD, p);
}

fn tex(fx: &mut Fx, sp: u32, t: u32) {
    let rh = fx.mem.read_u32(D + t);
    fx.desc_texture(sp + SD, rh, 1);
}

fn clamp5(fx: &mut Fx, sp: u32) {
    fx.desc_clamp(sp + SD, 5);
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

/// The sprite at sp + 0x2C0 projected (0x204F70); submitted at the mean
/// w (`ra`) or far at key 0.
fn draw(fx: &mut Fx, sp: u32, at: u32, ra: Option<u32>) {
    let d = sp + SD;
    if crate::fxlib2::project(fx, at, PROJ_OUT, sp + CAM, d, sp + W) {
        match ra {
            Some(ra) => {
                let w = fx.mem.f(sp + W);
                let key = fixu(fx, w, ra) as i32;
                fx.submit(d, OT, key);
            }
            None => {
                crate::lens::far_z(fx, sp + SD + 0x20, &[sp + SD + 0x30]);
                fx.submit(d, OT, 0);
            }
        }
    }
}

/// The corners about sp + 0x2E0, drawn.
fn draw_at(fx: &mut Fx, sp: u32, ra: Option<u32>) {
    fx.vu_add4(sp + Q0, sp + C0, sp + P);
    fx.vu_add4(sp + Q1, sp + C1, sp + P);
    draw(fx, sp, sp + Q0, ra);
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
fn screen(fx: &mut Fx, sp: u32, p: u32, out: Option<u32>) -> u64 {
    let m = fx.read_m4(sp + CAM);
    fx.project_at(p, &m, sp + SCR, sp + W);
    if let Some(o) = out {
        let x = (i32::from(fx.mem.read_u16(sp + SCR)) >> 4) - 0x700;
        fx.mem.setf(o, ee::from_i32(x));
        let y = (i32::from(fx.mem.read_u16(sp + SCR + 2)) >> 4) - 0x700;
        fx.mem.setf(o + 4, ee::from_i32(y));
    }
    (fx.mem.read_u64(sp + SCR) << 8) >> 40
}

fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    let z = screen(fx, sp, p, Some(o));
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
fn fan(fx: &mut Fx, sp: u32, c: u32, k: f32, z: u64) {
    let ras = &R_FAN;
    let d = sp + FD;
    for t in 0..8u32 {
        let s1 = FAN + 0x10 * t;
        let x = fix_at(fx, c, ras[0]);
        let y = fix_at(fx, c + 4, ras[1]);
        uv(fx, d + 0x18, ((x << 4) & 0x3FFF) as u32);
        uv(fx, d + 0x1A, ((y << 4) & 0x3FFF) as u32);
        let x = fix_at(fx, s1, ras[2]);
        let y = fix_at(fx, s1 + 4, ras[3]);
        uv(fx, d + 0x30, ((x << 4) & 0x3FFF) as u32);
        uv(fx, d + 0x32, ((y << 4) & 0x3FFF) as u32);
        let x = fix_at(fx, s1 + 0x10, ras[4]);
        let y = fix_at(fx, s1 + 0x14, ras[5]);
        uv(fx, d + 0x48, ((x << 4) & 0x3FFF) as u32);
        uv(fx, d + 0x4A, ((y << 4) & 0x3FFF) as u32);
        fx.vu_sub4(sp + Q0, s1, c);
        fx.vu_sub4(sp + Q1, s1 + 0x10, c);
        fx.vu_scale4(sp + Q0, sp + Q0, k);
        fx.vu_scale4(sp + Q1, sp + Q1, k);
        fx.vu_add4(sp + Q0, sp + Q0, c);
        fx.vu_add4(sp + Q1, sp + Q1, c);
        let x = fix_at(fx, c, ras[6]);
        let y = fix_at(fx, c + 4, ras[7]);
        xyz_z(fx, d + 0x20, x, y, z);
        let x = fix_at(fx, sp + Q0, ras[8]);
        let y = fix_at(fx, sp + Q0 + 4, ras[9]);
        xyz_z(fx, d + 0x38, x, y, z);
        let x = fix_at(fx, sp + Q1, ras[10]);
        let y = fix_at(fx, sp + Q1 + 4, ras[11]);
        xyz_z(fx, d + 0x50, x, y, z);
        fx.submit(d, 0, 0);
    }
}

fn in_window(n: i32, lo: i32, hi: i32) -> bool {
    n >= lo && n < hi
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    for i in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x1C + 4 * i));
        fx.node_position(h, slot, D + 0x30 + 0x10 * i);
    }
    fx.vu_sub4(D + 0x50, D + 0x40, D + 0x30);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x70);
    camera(fx, sp);
    screen_point(fx, sp, D + 0x70, D + 0x80);
    // The point 1.5 radii in front of the attacker: its Z.
    fx.mem.write_u32(sp + P, 0);
    fx.mem.write_u32(sp + P + 4, 0);
    let r = fx.mem.f(D + 0x90);
    fx.mem.setf(sp + P + 8, ee::neg(r));
    fx.mem.write_u32(sp + P + 0xC, 0x3F80_0000);
    fx.vu_apply_to(sp + P, sp + TURN, sp + P);
    fx.vu_add4(sp + P, sp + P, D + 0x70);
    let z = screen(fx, sp, sp + P, None);
    fx.mem.write_u32(D + 0x94, z as u32);

    let n = fx.mem.i32(D);
    if in_window(n, 0x1F, 0x198) {
        let r = D + FANS + 8 * fx.div(n, 12).1 as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
    }
    fans(fx, sp);

    if fx.mem.i32(D) == 0x84 {
        fx.mem.write_u8(D + LINE, 1);
        fx.mem.write_u32(D + LINE + 4, 0);
    }
    if fx.mem.read_u8(D + LINE) == 1 {
        line(fx, sp);
    }

    let n = fx.mem.i32(D);
    if in_window(n, 0x85, 0x198) {
        let r = D + GROUPS + 0x160 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for i in 0..4u32 {
            let k = ee::add(f(0x3E99_999A), ee::div(ee::mul(f(0x3F19_9999), ee::from_i32(i as i32)), 3.0));
            fx.vu_scale4(sp + P, D + 0x50, k);
            fx.vu_add4(sp + P, sp + P, D + 0x30);
            for c in 0..4u32 {
                let w = fx.mem.read_u32(sp + P + 4 * c);
                fx.mem.write_u32(r + 0x20 + 0x10 * i + 4 * c, w);
            }
            for j in 0..4u32 {
                let b = fx.rand_from(OT, RR_GROUP) >> 23;
                fx.mem.set8(r + 8 + 4 * i + j, rem2(b, 4));
                fx.random_table_vec(r + 0x60 + 0x40 * i + 0x10 * j, f(0x3E99_999A));
            }
        }
    }
    groups(fx, sp);

    let n = fx.mem.i32(D);
    if in_window(n, 0x101, 0x198) {
        let r = D + SPARKS + 0x100 * rem2(n, 0x40) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        spawn_pos(fx, sp, r + 0x70);
        for j in 0..8u32 {
            let b = fx.rand_from(OT, RR_SPARK[0]) >> 23;
            fx.mem.set32(r + 8 + 4 * j, rem2(b, 8));
            let b = fx.rand_from(OT, RR_SPARK[1]) >> 23;
            let x = ee::div(ee::mul(0.5, ee::from_i32(rem2(b, 0x100))), 256.0);
            // 0x1A2C30 / 0x1A3118 / 0x1A38F8: (float) (0.5 + (double) x).
            let sz = (0.5f64 + f64::from(x)) as f32;
            fx.mem.setf(r + 0x28 + 4 * j, sz);
            let dir = r + 0x80 + 0x10 * j;
            fx.random_table_vec(dir, f(0x3F8C_CCCD));
            let a = Fx::powf2(fx.mem.f(dir));
            let b = Fx::powf2(fx.mem.f(dir + 4));
            let l = fx.sqrtf(ee::add(a, b));
            let m = fx.sqrtf(ee::mul(2.0, Fx::powf2(f(0x418C_CCCD))));
            let k = ee::mul(0.5, ee::sub(m, l));
            let m = fx.sqrtf(ee::mul(2.0, Fx::powf2(f(0x418C_CCCD))));
            fx.mem.setf(r + 0x48 + 4 * j, ee::add(0.5, ee::div(k, m)));
        }
    }
    sparks(fx, sp);

    let n = fx.mem.i32(D);
    if in_window(n, 0x101, 0x198) {
        let r = D + MOTES + 0xB0 * rem2(n, 0x40) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        spawn_pos(fx, sp, r + 0x40);
        for j in 0..6u32 {
            let b = fx.rand_from(OT, RR_MOTE[0]) >> 23;
            fx.mem.set32(r + 8 + 4 * j, rem2(b, 4));
            let b = fx.rand_from(OT, RR_MOTE[1]) >> 23;
            fx.mem.set32(r + 0x20 + 4 * j, (rem2(b, 0x100) << 16) >> 8);
            fx.random_table_vec(r + 0x50 + 0x10 * j, f(0x3F4C_CCCD));
        }
    }
    motes(fx, sp);

    let n = fx.mem.i32(D);
    if in_window(n, 0x129, 0x186) {
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
    rings(fx, sp);
    flat(fx, sp);
    frame_sprite(fx, sp);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x1C6 - n, 0, 0x20);
    let k = Fx::clamp_i(n, 0, 0x20);
    let l = ((k.wrapping_mul(-3) << 5).wrapping_mul(s0) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x11E {
        ret = 1;
    } else {
        if in_window(n, 0x129, 0x1A6) {
            ret = 6;
        }
        if n == 0x1A6 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x1C7 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// A spark / mote point: D+0x50 x -0.1 + node 0 (its z + 0.0).
fn spawn_pos(fx: &mut Fx, sp: u32, at: u32) {
    fx.vu_scale4(sp + P, D + 0x50, f(0xBDCC_CCCD));
    fx.vu_add4(sp + P, sp + P, D + 0x30);
    for c in 0..4u32 {
        let w = fx.mem.read_u32(sp + P + 4 * c);
        fx.mem.write_u32(at + 4 * c, w);
    }
    let z = fx.mem.f(at + 8);
    fx.mem.setf(at + 8, ee::add(z, 0.0));
}

/// The twelve fans (13 updates) [F 0x1E00550..0x1E00B2C].
fn fans(fx: &mut Fx, sp: u32) {
    let d = sp + FD;
    fx.mem.write_u16(d + 2, 0x33);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, LENS_CLAMP);
    let n = fx.mem.i32(D);
    let a0 = Fx::clamp_i(n - 0x1E, 0, 0x40);
    let b0 = Fx::clamp_i(0x198 - n, 0, 0x40);
    let f20 = ee::div(ee::mul(ee::from_i32(a0), ee::from_i32(b0)), 4096.0);
    // a0..a3 as the second 0x101050 leaves them.
    let a = [b0 as u32, 0, 0x40, d + 2];
    fx.mem.write_u8(sp + 0xD0, 0);
    for k in 0..12u32 {
        let r = D + FANS + 8 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        if fx.mem.read_u8(sp + 0xD0) == 0 {
            fx.mem.write_u8(sp + 0xD0, 1);
            let layer = fx.layer_begin();
            fx.mem.write_u32(sp + 0xEC, layer);
            fx.texflush(a);
            let h = fx.mem.i32(D + 4);
            let (_, regs) = fx.unit_draw(h, 1, sp + 0x3B8, sp + 0x3C0);
            fx.texflush(regs);
        }
        let x = (fx.mem.i32(r + 4) + 1) << 15;
        let sn = fx.tsin(x.wrapping_div(13));
        let al = fixu(fx, ee::mul(f20, ee::mul(128.0, sn)), R_FAN_A[0]) as u8;
        rgba(fx, d + 0x10, [0xFF, 0x1C, 0x1C], al);
        let x = (fx.mem.i32(r + 4) + 1) << 15;
        let sn = fx.tsin(x.wrapping_div(13));
        let al = fixu(fx, ee::mul(f20, ee::mul(128.0, sn)), R_FAN_A[1]) as u8;
        rgba(fx, d + 0x28, [0xFF, 0x40, 0x20], al);
        copy8(fx, d + 0x40, d + 0x28);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let t = ee::div(ee::mul(f(0x3E99_999A), age), 12.0);
        let t = ee::add(1.0, ee::add(f(0xBE19_999A), t));
        let f21 = ee::add(f(0x3C4C_CCCD), t);
        let z = u64::from(fx.mem.read_u32(D + 0x94) & 0xFF_FFFF);
        fan(fx, sp, D + 0x80, f21, z);
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0xD {
            fx.mem.write_u8(r, 0);
        }
    }
    if fx.mem.read_u8(sp + 0xD0) == 1 {
        let layer = fx.mem.read_u32(sp + 0xEC);
        fx.layer_end(layer, OT, 0);
    }
}

/// The 16-sprite line along the nodes (0x114 updates, far Z) [F
/// 0x1E00B48..0x1E00EB8].
fn line(fx: &mut Fx, sp: u32) {
    sprite_head(fx, sp);
    tex(fx, sp, 0x10);
    alpha(fx, sp, 1);
    clamp5(fx, sp);
    camera(fx, sp);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    let age = fx.mem.i32(D + LINE + 4);
    let f20 = window(age, 0x114 - age, 0x20, 1024.0);
    corners(fx, sp, 9.0, true);
    for s4 in 0..0x10i32 {
        let sn = fx.tsin(((s4 + 1) << 15) / 17);
        let f3 = ee::mul(f20, ee::mul(128.0, sn));
        let age = fx.mem.i32(D + LINE + 4);
        let k = ee::add(f(0x3F4C_CCCD), ee::mul(f(0x3E4C_CCCD), ee::from_i32(rem2(age, 2))));
        let al = fixu(fx, ee::mul(k, f3), R_LINE) as u8;
        rgba(fx, sp + SD + 0x10, [0xFF, 0x44, 0x1C], al);
        let t = ee::add(f(0x3E4C_CCCD), ee::div(ee::from_i32(s4), 15.0));
        fx.vu_scale4(sp + P, D + 0x50, t);
        fx.vu_add4(sp + P, sp + P, D + 0x30);
        draw_at(fx, sp, None);
    }
    let age = fx.mem.i32(D + LINE + 4) + 1;
    fx.mem.set32(D + LINE + 4, age);
    if age >= 0x115 {
        fx.mem.write_u8(D + LINE, 0);
    }
}

/// The 32 groups (4 x 4 sprites along the nodes, 32 updates) [F
/// 0x1E01010..0x1E013F0].
fn groups(fx: &mut Fx, sp: u32) {
    sprite_head(fx, sp);
    tex(fx, sp, 0x10);
    alpha(fx, sp, 1);
    clamp5(fx, sp);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = window(n - 0x84, 0x198 - n, 0x20, 1024.0);
    for k in 0..0x20u32 {
        let r = D + GROUPS + 0x160 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let sn = fx.tsin((age << 15) >> 5);
        let al = fixu(fx, ee::mul(f20, ee::mul(255.0, sn)), R_GROUP[0]) as u8;
        rgba(fx, sp + SD + 0x10, [0xFF, 0x23, 0x0F], al);
        let age = fx.mem.i32(r + 4);
        let t = ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(0x20 - age)), 32.0);
        corners(fx, sp, ee::mul(f(0x3F4C_CCCD), ee::add(f(0x3E4C_CCCD), t)), true);
        let f21 = ee::div(ee::from_i32(fx.mem.i32(r + 4)), 32.0);
        for i in 0..4u32 {
            for j in 0..4u32 {
                let b = i32::from(fx.mem.read_u8(r + 8 + 4 * i + j));
                cell_uvs(fx, sp, b);
                fx.vu_scale4(sp + P, r + 0x60 + 0x40 * i + 0x10 * j, f21);
                // The point of group j, not i (the record's own indexing).
                fx.vu_add4(sp + P, sp + P, r + 0x20 + 0x10 * j);
                draw_at(fx, sp, Some(R_GROUP[1]));
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// sp + 0x2F0: node 0's point `pos` pulled towards the target (x / y by
/// age / 64, z by 1.7 age / 64).
fn pull(fx: &mut Fx, sp: u32, pos: u32, age: i32) {
    let a = ee::from_i32(age);
    let fz = ee::div(ee::mul(f(0x3FD9_999A), a), 64.0);
    let fxy = ee::div(a, 64.0);
    fx.vu_sub4(sp + PULL, D + 0x60, pos);
    for (o, k) in [(0u32, fxy), (4, fxy), (8, fz)] {
        let v = fx.mem.f(sp + PULL + o);
        fx.mem.setf(sp + PULL + o, ee::mul(v, k));
    }
    fx.vu_add4(sp + PULL, sp + PULL, pos);
}

/// The 64 sparks (8 sprites pulled to the target, 64 updates) [F
/// 0x1E01608..0x1E01A2C].
fn sparks(fx: &mut Fx, sp: u32) {
    sprite_head(fx, sp);
    alpha(fx, sp, 1);
    tex(fx, sp, 0x14);
    clamp5(fx, sp);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f22 = ee::div(ee::from_i32(Fx::clamp_i(0x198 - n, 0, 0x20)), 32.0);
    for k in 0..0x40u32 {
        let r = D + SPARKS + 0x100 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + 4));
        corners(fx, sp, ee::mul(11.0, lin(f(0x3F4C_CCCD), f(0x3E4C_CCCD), age, 64.0)), true);
        let age_i = fx.mem.i32(r + 4);
        let age = ee::from_i32(age_i);
        let spread = lin(f(0x3E99_999A), f(0x3F33_3333), age, 64.0);
        pull(fx, sp, r + 0x70, age_i);
        for j in 0..8u32 {
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f21 = ee::mul(80.0, Fx::ratio_below(age, 8.0));
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f0 = Fx::ramp_above(age, 64.0, 42.0);
            let w = fx.mem.f(r + 0x48 + 4 * j);
            let al = fixu(fx, ee::mul(w, ee::mul(f22, ee::mul(f21, f0))), R_SPARK[0]) as u8;
            rgba(fx, sp + SD + 0x10, [0xFF, 0x50, 0x32], al);
            let b = fx.mem.i32(r + 8 + 4 * j);
            cell_uvs(fx, sp, b);
            let sz = fx.mem.f(r + 0x28 + 4 * j);
            fx.vu_scale4(sp + Q0, sp + C0, sz);
            let sz = fx.mem.f(r + 0x28 + 4 * j);
            fx.vu_scale4(sp + Q1, sp + C1, sz);
            fx.vu_scale4(sp + P, r + 0x80 + 0x10 * j, spread);
            fx.vu_add4(sp + P, sp + P, sp + PULL);
            fx.vu_add4(sp + Q0, sp + Q0, sp + P);
            fx.vu_add4(sp + Q1, sp + Q1, sp + P);
            draw(fx, sp, sp + Q0, Some(R_SPARK[1]));
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x41 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The 64 motes (6 swaying sprites pulled to the target, 64 updates) [F
/// 0x1E01B60..0x1E01F94].
fn motes(fx: &mut Fx, sp: u32) {
    sprite_head(fx, sp);
    alpha(fx, sp, 1);
    tex(fx, sp, 0x10);
    clamp5(fx, sp);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(0x198 - n, 0, 0x20)), 32.0);
    for k in 0..0x40u32 {
        let r = D + MOTES + 0xB0 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + 4));
        corners(fx, sp, ee::mul(10.0, lin(f(0x3F4C_CCCD), f(0x3E4C_CCCD), age, 64.0)), true);
        let age_i = fx.mem.i32(r + 4);
        pull(fx, sp, r + 0x40, age_i);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let f21 = ee::mul(160.0, Fx::ratio_below(age, 8.0));
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let f0 = Fx::ramp_above(age, 64.0, 42.0);
        let al = fixu(fx, ee::mul(f20, ee::mul(f21, f0)), R_MOTE[0]) as u8;
        rgba(fx, sp + SD + 0x10, [0xFF, 0x50, 0x32], al);
        for j in 0..6u32 {
            let age_i = fx.mem.i32(r + 4);
            let age = ee::from_i32(age_i);
            let a = fx.mem.i32(r + 0x20 + 4 * j).wrapping_add((age_i << 15) >> 6) & 0xFFFF;
            let l = lin(f(0x3E99_999A), f(0x3F33_3333), age, 64.0);
            let spread = ee::mul(l, fx.tsin(a));
            let b = fx.mem.i32(r + 8 + 4 * j);
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + P, r + 0x50 + 0x10 * j, spread);
            fx.vu_add4(sp + P, sp + P, sp + PULL);
            draw_at(fx, sp, Some(R_MOTE[1]));
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x41 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The 32 rings (0x50, far Z) [F 0x1E0209C..0x1E02720].
fn rings(fx: &mut Fx, sp: u32) {
    let d = sp + RD;
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
    let xyz = [d + 0x20, d + 0x38, d + 0x50, d + 0x68];
    for k in 0..0x20u32 {
        let r = D + RINGS + 0x50 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        rgba(fx, d + 0x10, [0xFF, 0x17, 0x10], 0);
        copy8(fx, d + 0x28, d + 0x10);
        let t = ee::div(ee::from_i32(0x20 - age), 32.0);
        let al = fixu(fx, ee::mul(160.0, t), R_RING) as u8;
        rgba(fx, d + 0x40, [0xFF, 0x80, 0x45], al);
        copy8(fx, d + 0x58, d + 0x40);
        fx.camera_vp_to(sp + CAM);
        let t = [fx.mem.f(D + 0x60), fx.mem.f(D + 0x64), fx.mem.f(D + 0x68)];
        let m = fx.trs(None, None, Some(t));
        fx.write_m4(sp + IDM, &m);
        let rm = fx.read_m4(r + 0x10);
        fx.mul_into(Some(sp + IDM), &rm);
        let odd = rem2(k as i32, 2);
        let s1 = 2 * odd - 1;
        let age = fx.mem.i32(r + 4);
        fx.rotate_axis(sp + IDM, s1.wrapping_mul(55 * age), 0);
        let age = fx.mem.i32(r + 4);
        fx.rotate_axis(sp + IDM, s1.wrapping_mul(77 * age), 1);
        let age = fx.mem.i32(r + 4);
        fx.rotate_axis(sp + IDM, s1.wrapping_mul(99 * age), 2);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let sc = lin(f(0x3E4C_CCCD), f(0x3F4C_CCCD), age, 32.0);
        vec4(fx, sp + P, sc, sc, 0.0);
        fx.scale_rows(sp + IDM, sp + P);
        let m = fx.read_m4(sp + IDM);
        fx.mul_into(Some(sp + CAM), &m);
        let age = fx.mem.i32(r + 4);
        let f20 = ee::div(ee::mul(-1.0, ee::from_i32(rem2(age, 16))), 16.0);
        let f21 = ee::add(0.5, f20);
        let f22 = ee::add(1.0, f20);
        for s7 in 0..0x10u32 {
            let (a, b) = (pts + 0x30 * s7, pts + 0x30 * (s7 + 1));
            let f23 = ee::div(ee::mul(8.0, ee::from_i32(s7 as i32)), 16.0);
            let f24 = ee::div(ee::mul(8.0, ee::from_i32(s7 as i32 + 1)), 16.0);
            for (o, x) in [(0x18u32, f23), (0x1C, f20), (0x30, f24), (0x34, f20), (0x48, f23), (0x4C, f21), (0x60, f24), (0x64, f21)] {
                fx.mem.setf(d + o, x);
            }
            let vs = [a, b, a + 0x20, b + 0x20];
            if fx.project_n(&vs, sp + CAM, d, sp + W, RP_RING[0]) == 1 {
                crate::lens::far_z(fx, xyz[0], &xyz[1..]);
                fx.submit(d, OT, 0);
            }
            for (o, x) in [(0x18u32, f23), (0x1C, f22), (0x30, f24), (0x34, f22), (0x48, f23), (0x4C, f21), (0x60, f24), (0x64, f21)] {
                fx.mem.setf(d + o, x);
            }
            let vs = [a + 0x10, b + 0x10, a + 0x20, b + 0x20];
            if fx.project_n(&vs, sp + CAM, d, sp + W, RP_RING[1]) == 1 {
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

/// The flat sprite at the target (far Z) [F 0x1E02720..0x1E029DC].
fn flat(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let f20 = window(n - 0x128, 0x1A6 - n, 0x20, 1024.0);
    if crate::exe::ee_le(f20, 0.0) {
        return;
    }
    sprite_head(fx, sp);
    alpha(fx, sp, 1);
    tex(fx, sp, 0x10);
    clamp5(fx, sp);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    corners(fx, sp, 60.0, false);
    let n = fx.mem.i32(D);
    let f1 = ee::add(f(0x3F66_6666), ee::mul(f(0x3DCC_CCCD), ee::from_i32(rem2(n, 2))));
    let al = fixu(fx, ee::mul(ee::mul(255.0, f20), f1), R_FLAT) as u8;
    rgba(fx, sp + SD + 0x10, [0xFF, 0x64, 0x50], al);
    fx.camera_vp_to(sp + CAM);
    fx.translate_rows(sp + IDM, sp + TURN, D + 0x60);
    let m = fx.read_m4(sp + IDM);
    fx.mul_into(Some(sp + CAM), &m);
    draw(fx, sp, sp + C0, None);
}

/// The full-window frame sprite (0x207EF0) [F 0x1E029DC..0x1E02BC0].
fn frame_sprite(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let f20 = window(n - 0x128, 0x1A6 - n, 0x20, 1024.0);
    if crate::exe::ee_le(f20, 0.0) {
        return;
    }
    sprite_head(fx, sp);
    let d = sp + SD;
    let p = fx.alpha_preset(0);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, p);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    clamp5(fx, sp);
    uvs(fx, sp, 0x220, 0x220, 0x1DE0, 0x1DE0);
    let al = fixu(fx, ee::mul(48.0, f20), R_FRAME) as u8;
    rgba(fx, sp + SD + 0x10, [0x80, 0x80, 0x80], al);
    far_xy(fx, sp + SD + 0x20, 0x7000, 0x7000);
    far_xy(fx, sp + SD + 0x30, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
}
