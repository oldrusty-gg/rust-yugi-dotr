//! BMODULE b518 (card 548), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xD130
//! bytes) [F]: b030's family (b030.rs, its fan /
//! lens / spark / mote / ring / flat / second-lens code at this record's
//! values, kept private so b030.rs stays untouched) in a 0xA30 frame (s1 =
//! D, s0 = the ordering table; the scratch 0x280 above b030's, return slot
//! sp + 0xC0, layer sp + 0xEC), the header 0x50 up, four nodes: 0x42
//! (clear) / 0x45 at D+0x30 / 0x40 (slots D+0x1C / 0x20), 0x3F / 0x40 at
//! D+0x60 / 0x70 (slots D+0x50 / 0x54). In place of b030's flecks / embers,
//! on 1..0x10 two bursts at node 1 (8 x 0x240 at D+0x120, 32 sprites, and
//! 8 x 0xA0 at D+0x7160, 8 sprites) closing on the node over 8 updates,
//! and on 1..0xD9 two rises at nodes 2 / 3 (28 x 0x2D0 at D+0x7670, two
//! blocks of 20 sprites, and 12 x 0x100 at D+0xC530, two blocks of 6, far
//! Z and key 0). Lens on 0x19 (16 updates), second lens on 0x87 (0x73
//! updates). Returns 1 / 6 / 2 / 8 on 0x3E / 0x49..0xD9 / 0xDA / 0x11A.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, far_z, KEEP, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// The header (b030's D+0x30.. and pools), 0x50 up.
const H: u32 = D + 0x50;
const FRAME: u32 = 0xA30;
/// The scratch area's shift from b030.rs: `sp` below is the frame + FS.
const FS: u32 = 0x280;
const RET_SLOT: u32 = 0xC0;
const LAYER_SLOT: u32 = 0xEC;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The fan's nine screen points (exe rodata, 0x10 apart).
const FAN: u32 = 0x0028_ABE0;
/// (node, clear, slot, position).
const NODES: [(i32, i32, u32, u32); 4] = [
    (0x42, 1, D + 0x1C, D + 0x30),
    (0x45, 0, D + 0x20, D + 0x40),
    (0x3F, 0, D + 0x50, D + 0x60),
    (0x40, 0, D + 0x54, D + 0x70),
];
/// Pools, relative to H.
const SPARKS: u32 = 0x12D0;
const MOTES: u32 = 0x42D0;
const RING_PTS: u32 = 0x63D0;
const RINGS: u32 = 0x6700;
const LENS1: u32 = 0x7100;
const LENS2: u32 = 0x7610;
/// The update the fans / sparks / rises close on.
const CLOSE: i32 = 0xDA;
/// The fans' 0x1A2B80 return addresses, then the second lens fan's.
const R_FAN1: [u32; 12] = [
    0x1E0_0D38, 0x1E0_0D44, 0x1E0_0D94, 0x1E0_0DA0, 0x1E0_0DF0, 0x1E0_0DFC, 0x1E0_0EBC, 0x1E0_0EC8, 0x1E0_0F34,
    0x1E0_0F40, 0x1E0_0FAC, 0x1E0_0FB8,
];
const R_FAN2: [u32; 12] = [
    0x1E0_2C20, 0x1E0_2C2C, 0x1E0_2C7C, 0x1E0_2C88, 0x1E0_2CD8, 0x1E0_2CE4, 0x1E0_2DA4, 0x1E0_2DB0, 0x1E0_2E20,
    0x1E0_2E2C, 0x1E0_2E9C, 0x1E0_2EA8,
];

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// One of the two bursts.
struct Burst {
    /// D+.., the stride, the sprites a record.
    base: u32,
    stride: u32,
    n: u32,
    /// The record's point / directions.
    pos: u32,
    dirs: u32,
    /// The cell mod.
    cells: i32,
    /// The texture word D+.., the alpha preset, the sine's amplitude.
    tex: u32,
    alpha: i32,
    amp: f32,
    /// The size: k (base + slope (8 - age) / 8).
    k: Option<f32>,
    size: (f32, f32),
    colour: [u8; 3],
    r_rand: u32,
    r_fixu: [u32; 2],
}

const FLECKS: Burst = Burst {
    base: 0x120,
    stride: 0x240,
    n: 0x20,
    pos: 0x30,
    dirs: 0x40,
    cells: 4,
    tex: 0x10,
    alpha: 1,
    amp: 255.0,
    k: None,
    size: (f(0x3E4C_CCCD), f(0x3F4C_CCCD)),
    colour: [0xFF, 0x3C, 0x1E],
    r_rand: 0x1E0_1158,
    r_fixu: [0x1E0_12CC, 0x1E0_14E8],
};
const EMBERS: Burst = Burst {
    base: 0x7160,
    stride: 0xA0,
    n: 8,
    pos: 0x10,
    dirs: 0x20,
    cells: 8,
    tex: 0x14,
    alpha: 0,
    amp: 128.0,
    k: Some(10.0),
    size: (0.5, 0.5),
    colour: [0x64, 0x64, 0x64],
    r_rand: 0x1E0_3010,
    r_fixu: [0x1E0_3184, 0x1E0_339C],
};

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + RET_SLOT, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp + FS) };
    fx.leave(FRAME);
    ret
}

/// The byte clears after the bzero change nothing and are left out.
fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xD130);
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
    for (node, clear, slot, _) in NODES {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(slot, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, H + 0x30);
    let h = fx.mem.i32(D + 4);
    let r = crate::blib::afterimage::unit_radius(fx, h);
    fx.mem.setf(H + 0x70, ee::mul(1.5, r));
    // The ring points: 17 x (30 / 100 at k / 16 of a turn, the middle).
    for k in 0..0x11u32 {
        let p = H + RING_PTS + 0x30 * k;
        let a = (k << 12) as i32;
        let s = fx.tsin(a);
        let c = fx.tcos(a);
        fx.mem.setf(p, ee::mul(30.0, s));
        fx.mem.setf(p + 4, ee::mul(30.0, c));
        fx.mem.write_u32(p + 8, 0);
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        fx.mem.setf(p + 0x10, ee::mul(100.0, s));
        fx.mem.setf(p + 0x14, ee::mul(100.0, c));
        fx.mem.write_u32(p + 0x18, 0);
        fx.mem.write_u32(p + 0x1C, 0x3F80_0000);
        fx.vu_add4(p + 0x20, p, p + 0x10);
        fx.vu_scale4(p + 0x20, p + 0x20, 0.5);
    }
    0
}

/// A pool's sprite size k (base + slope x age / d).
#[derive(Clone, Copy)]
struct Lin(f32, f32);

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

/// The sprite cell `b` (4 x n cells of 32) into UV0 / UV1 at sp + 0x658 /
/// 0x65A / 0x668 / 0x66A.
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    uv(fx, sp + 0x658, ((c << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x65A, ((r << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x668, (((32 * c + 32) << 4) & 0x3FFF) as u32);
    uv(fx, sp + 0x66A, (((32 * r + 32) << 4) & 0x3FFF) as u32);
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

/// The view-projection (x identity) at sp + 0x4F0.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x4F0);
    fx.identity(sp + 0x530);
    let id = fx.read_m4(sp + 0x530);
    fx.mul_into(Some(sp + 0x4F0), &id);
}

/// The sprite descriptor at sp + 0x640 (5 registers 0x42421, prim 0x26).
fn sprite_head(fx: &mut Fx, sp: u32) {
    let d = sp + 0x640;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// The sprite's corners (-s, -s, 0, 1) at sp + 0x5B0 and (s, s, 0, 1) at
/// sp + 0x5C0 turned by the camera (sp + 0x570).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x5B0, n);
    fx.mem.setf(sp + 0x5B4, n);
    fx.mem.write_u32(sp + 0x5B8, 0);
    fx.mem.write_u32(sp + 0x5BC, 0x3F80_0000);
    fx.mem.setf(sp + 0x5C0, s);
    fx.mem.setf(sp + 0x5C4, s);
    fx.mem.write_u32(sp + 0x5C8, 0);
    fx.mem.write_u32(sp + 0x5CC, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x5B0, sp + 0x570, sp + 0x5B0);
    fx.vu_apply_to(sp + 0x5C0, sp + 0x570, sp + 0x5C0);
}

/// The sprite at sp + 0x5D0 / 0x5E0 projected (0x204F70) and submitted at
/// the mean w.
fn draw(fx: &mut Fx, sp: u32, at: u32, ra: u32) {
    let d = sp + 0x640;
    if crate::fxlib2::project(fx, at, PROJ_OUT, sp + 0x4F0, d, sp + 0x7AC) {
        let w = fx.mem.f(sp + 0x7AC);
        let key = fixu(fx, w, ra) as i32;
        fx.submit(d, OT, key);
    }
}

/// The corners about sp + 0x5F0 (sp + 0x5D0 / 0x5E0), drawn.
fn draw_at(fx: &mut Fx, sp: u32, ra: u32) {
    fx.vu_add4(sp + 0x5D0, sp + 0x5B0, sp + 0x5F0);
    fx.vu_add4(sp + 0x5E0, sp + 0x5C0, sp + 0x5F0);
    draw(fx, sp, sp + 0x5D0, ra);
}

/// clamp(a, 0, m) x clamp(b, 0, m) / d.
fn window(a: i32, b: i32, m: i32, d: f32) -> f32 {
    ee::div(ee::mul(ee::from_i32(Fx::clamp_i(a, 0, m)), ee::from_i32(Fx::clamp_i(b, 0, m))), d)
}

/// The screen point of `p` (0x204D80 through sp + 0x4F0 into sp + 0x7A0):
/// x / y into `out` / `out` + 4 ((x16 >> 4) - 0x700); the Z (bits 32..55).
fn screen(fx: &mut Fx, sp: u32, p: u32, out: Option<u32>) -> u64 {
    let m = fx.read_m4(sp + 0x4F0);
    fx.project_at(p, &m, sp + 0x7A0, sp + 0x7AC);
    if let Some(o) = out {
        let x = (i32::from(fx.mem.read_u16(sp + 0x7A0)) >> 4) - 0x700;
        fx.mem.setf(o, ee::from_i32(x));
        let y = (i32::from(fx.mem.read_u16(sp + 0x7A2)) >> 4) - 0x700;
        fx.mem.setf(o + 4, ee::from_i32(y));
    }
    (fx.mem.read_u64(sp + 0x7A0) << 8) >> 40
}

/// The screen point at `o`: Z (0x17BC10) into +8, 1.0 into +0xC, x / y
/// clamped to 0..512.
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

/// An XYZ2 at `p`: x / y halfwords of pixel (x, y), Z `z` and a zero top
/// byte.
fn xyz_z(fx: &mut Fx, p: u32, x: i32, y: i32, z: u64) {
    fx.mem.write_u16(p, ((x + 0x700) << 4) as u16);
    fx.mem.write_u16(p + 2, ((y + 0x700) << 4) as u16);
    let v = fx.mem.read_u64(p);
    fx.mem.write_u64(p, (v & KEEP) | (z << 32));
    fx.mem.write_u8(p + 7, 0);
}

/// One ring of the eight-triangle fan (descriptor sp + 0x698) about the
/// screen point `c`: texels the fan points, positions scaled by `f`, at
/// Z `z` (0xFFFFF8 far); `ras` the 0x1A2B80 return addresses.
fn fan(fx: &mut Fx, sp: u32, c: u32, f: f32, z: u64, ot: u32, ras: &[u32; 12]) {
    for k in 0..8u32 {
        let s1 = FAN + 0x10 * k;
        let x = fix_at(fx, c, ras[0]);
        let y = fix_at(fx, c + 4, ras[1]);
        uv(fx, sp + 0x6B0, ((x << 4) & 0x3FFF) as u32);
        uv(fx, sp + 0x6B2, ((y << 4) & 0x3FFF) as u32);
        let x = fix_at(fx, s1, ras[2]);
        let y = fix_at(fx, s1 + 4, ras[3]);
        uv(fx, sp + 0x6C8, ((x << 4) & 0x3FFF) as u32);
        uv(fx, sp + 0x6CA, ((y << 4) & 0x3FFF) as u32);
        let x = fix_at(fx, s1 + 0x10, ras[4]);
        let y = fix_at(fx, s1 + 0x14, ras[5]);
        uv(fx, sp + 0x6E0, ((x << 4) & 0x3FFF) as u32);
        uv(fx, sp + 0x6E2, ((y << 4) & 0x3FFF) as u32);
        fx.vu_sub4(sp + 0x5D0, s1, c);
        fx.vu_sub4(sp + 0x5E0, s1 + 0x10, c);
        fx.vu_scale4(sp + 0x5D0, sp + 0x5D0, f);
        fx.vu_scale4(sp + 0x5E0, sp + 0x5E0, f);
        fx.vu_add4(sp + 0x5D0, sp + 0x5D0, c);
        fx.vu_add4(sp + 0x5E0, sp + 0x5E0, c);
        let x = fix_at(fx, c, ras[6]);
        let y = fix_at(fx, c + 4, ras[7]);
        xyz_z(fx, sp + 0x6B8, x, y, z);
        let x = fix_at(fx, sp + 0x5D0, ras[8]);
        let y = fix_at(fx, sp + 0x5D4, ras[9]);
        xyz_z(fx, sp + 0x6D0, x, y, z);
        let x = fix_at(fx, sp + 0x5E0, ras[10]);
        let y = fix_at(fx, sp + 0x5E4, ras[11]);
        xyz_z(fx, sp + 0x6E8, x, y, z);
        fx.submit(sp + 0x698, ot, 0);
    }
}

/// The fan's triangle descriptor at sp + 0x698 (prim 0x33 | 0x800, 9
/// registers), alpha preset `a`, the lens CLAMP.
fn fan_desc(fx: &mut Fx, sp: u32, a: i32) {
    let d = sp + 0x698;
    fx.mem.write_u16(d + 2, 0x33);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let p = fx.alpha_preset(a);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, LENS_CLAMP);
}


/// `l`.0 + `l`.1 x `x` / `d`.
fn lin(l: Lin, x: f32, d: f32) -> f32 {
    ee::add(l.0, ee::div(ee::mul(l.1, x), d))
}

fn in_window(n: i32, w: (i32, i32)) -> bool {
    n >= w.0 && n < w.1
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x570);
    for (_, _, slot, pos) in NODES {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(slot));
        fx.node_position(h, s, pos);
    }
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, H + 0x50);
    camera(fx, sp);
    screen_point(fx, sp, H + 0x50, H + 0x60);
    // The point 1.5 radii in front of the attacker: its Z.
    fx.mem.write_u32(sp + 0x5F0, 0);
    fx.mem.write_u32(sp + 0x5F4, 0);
    let r = fx.mem.f(H + 0x70);
    fx.mem.setf(sp + 0x5F8, ee::neg(r));
    fx.mem.write_u32(sp + 0x5FC, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x5F0, sp + 0x570, sp + 0x5F0);
    fx.vu_add4(sp + 0x5F0, sp + 0x5F0, H + 0x50);
    let z = screen(fx, sp, sp + 0x5F0, None);
    fx.mem.write_u32(H + 0x74, z as u32);
    camera(fx, sp);
    screen_point(fx, sp, H + 0x30, H + 0x40);

    if fx.mem.i32(D) == 0x19 {
        fx.mem.write_u8(H + LENS1, 1);
        fx.mem.write_u32(H + LENS1 + 4, 0);
    }
    // The tint (sp + 0x610: prim 6, 3 registers) and the frame sprite
    // (sp + 0x640).
    let t = sp + 0x610;
    fx.mem.write_u16(t + 2, 6);
    fx.mem.write_u16(t, 3);
    fx.mem.write_u64(t + 8, 0x441);
    fx.mem.write_u32(t + 4, 0);
    let p = fx.alpha_preset(3);
    fx.desc_alpha(t, p);
    let d = sp + 0x640;
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
    lens1(fx, sp, a);

    let n = fx.mem.i32(D);
    if in_window(n, (1, CLOSE)) {
        let r = H + 0x78 + 8 * fx.div(n, 10).1 as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
    }
    fans(fx, sp);

    burst_spawn(fx, &FLECKS, f(0x3F33_3333));
    burst_draw(fx, sp, &FLECKS);

    let n = fx.mem.i32(D);
    if in_window(n, (0x2D, CLOSE)) {
        let r = H + 0x100 * fx.div(n, 0x30).1 as u32;
        fx.mem.write_u8(r + SPARKS, 1);
        fx.mem.write_u32(r + SPARKS + 4, 0);
        for c in 0..4u32 {
            let w = fx.mem.read_u32(D + 0x30 + 4 * c);
            fx.mem.write_u32(r + SPARKS + 0x70 + 4 * c, w);
        }
        for j in 0..8u32 {
            let b = fx.rand_from(OT, 0x1E0_1640) >> 23;
            fx.mem.set32(r + SPARKS + 8 + 4 * j, rem2(b, 8));
            let b = fx.rand_from(OT, 0x1E0_1660) >> 23;
            let x = ee::div(ee::mul(0.5, ee::from_i32(rem2(b, 0x100))), 256.0);
            // 0x1A2C30 / 0x1A3118 / 0x1A38F8: (float) (0.5 + (double) x).
            let s = (0.5f64 + f64::from(x)) as f32;
            let dir = r + SPARKS + 0x80 + 0x10 * j;
            fx.random_table_vec(dir, 1.0);
            fx.mem.setf(r + SPARKS + 0x28 + 4 * j, s);
            let a = Fx::powf2(fx.mem.f(dir));
            let b = Fx::powf2(fx.mem.f(dir + 4));
            let l = fx.sqrtf(ee::add(a, b));
            let q = fx.sqrtf(ee::mul(2.0, Fx::powf2(16.0)));
            let k = ee::mul(f(0x3E4C_CCCD), ee::sub(q, l));
            let q = fx.sqrtf(ee::mul(2.0, Fx::powf2(16.0)));
            fx.mem.setf(r + SPARKS + 0x48 + 4 * j, ee::add(f(0x3F4C_CCCD), ee::div(k, q)));
        }
    }
    sparks(fx, sp);

    let n = fx.mem.i32(D);
    if in_window(n, (0x2D, CLOSE)) {
        let r = H + MOTES + 0xB0 * fx.div(n, 0x30).1 as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for c in 0..4u32 {
            let w = fx.mem.read_u32(D + 0x30 + 4 * c);
            fx.mem.write_u32(r + 0x40 + 4 * c, w);
        }
        for j in 0..6u32 {
            let b = fx.rand_from(OT, 0x1E0_1D40) >> 23;
            fx.mem.set32(r + 8 + 4 * j, rem2(b, 4));
            let b = fx.rand_from(OT, 0x1E0_1D60) >> 23;
            fx.mem.set32(r + 0x20 + 4 * j, (rem2(b, 0x100) << 16) >> 8);
            fx.random_table_vec(r + 0x50 + 0x10 * j, 1.0);
        }
    }
    motes(fx, sp);

    let n = fx.mem.i32(D);
    if in_window(n, (0x49, 0xBA)) {
        let r = H + RINGS + 0x50 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        let m = r + 0x10;
        fx.identity(m);
        for (axis, ra) in [(1, 0x1E0_2320), (0, 0x1E0_2354), (2, 0x1E0_2388)] {
            let b = fx.rand_from(OT, ra) >> 23;
            fx.rotate_axis(m, (rem2(b, 0x100) << 16) >> 8, axis);
        }
    }
    rings(fx, sp);
    flat(fx, sp);
    lens2(fx, sp);

    burst_spawn(fx, &EMBERS, f(0x3F33_3333));
    burst_draw(fx, sp, &EMBERS);

    rise_spawn(fx, &RISE_A);
    rise_draw(fx, sp, &RISE_A);
    rise_spawn(fx, &RISE_B);
    rise_draw(fx, sp, &RISE_B);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x11A - n, 0, 0x20);
    let k = Fx::clamp_i(n, 0, 0x20);
    let l = ((k.wrapping_mul(-3) << 5).wrapping_mul(s0) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x3E {
        ret = 1;
    } else if in_window(n, (0x49, CLOSE)) {
        ret = 6;
    }
    if n == CLOSE {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x11B {
        ret = 8;
    }
    fx.mem.write_u32(sp - FS + RET_SLOT, ret as u32);
    ret
}

/// On 1..0x10 record n % 8: the point of node 1, the cells, `dir_len`
/// directions about the point.
fn burst_spawn(fx: &mut Fx, b: &Burst, dir_len: f32) {
    let n = fx.mem.i32(D);
    if !in_window(n, (1, 0x11)) {
        return;
    }
    let r = D + b.base + b.stride * rem2(n, 8) as u32;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    for c in 0..4u32 {
        let w = fx.mem.read_u32(D + 0x40 + 4 * c);
        fx.mem.write_u32(r + b.pos + 4 * c, w);
    }
    for j in 0..b.n {
        let c = fx.rand_from(OT, b.r_rand) >> 23;
        fx.mem.set8(r + 8 + j, rem2(c, b.cells));
        let dir = r + b.dirs + 0x10 * j;
        fx.random_table_vec(dir, dir_len);
        fx.vu_add4(dir, dir, r + b.pos);
    }
}

/// The records closing on node 1 ((direction - node) (8 - age) / 8 about
/// it), alpha amp sin((age << 14) >> 3), 9 updates [F 0x1E011F8..0x1E015A0,
/// 0x1E030B0..0x1E03450].
fn burst_draw(fx: &mut Fx, sp: u32, b: &Burst) {
    sprite_head(fx, sp);
    let d = sp + 0x640;
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    for k in 0..8u32 {
        let r = D + b.base + b.stride * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let rh = fx.mem.read_u32(D + b.tex);
        fx.desc_texture(d, rh, 1);
        let p = fx.alpha_preset(b.alpha);
        fx.desc_alpha(d, p);
        let age = fx.mem.i32(r + 4);
        let s = fx.tsin((age << 14) >> 3);
        let al = fixu(fx, ee::mul(1.0, ee::mul(b.amp, s)), b.r_fixu[0]) as u8;
        let age = fx.mem.i32(r + 4);
        let x = ee::div(ee::mul(b.size.1, ee::from_i32(8 - age)), 8.0);
        let s = ee::add(b.size.0, x);
        corners(fx, sp, b.k.map_or(s, |k| ee::mul(k, s)));
        let age = fx.mem.i32(r + 4);
        let f20 = ee::div(ee::from_i32(8 - age), 8.0);
        rgba(fx, sp + 0x650, b.colour, al);
        for j in 0..b.n {
            let c = i32::from(fx.mem.read_u8(r + 8 + j));
            cell_uvs(fx, sp, c);
            fx.vu_sub4(sp + 0x5F0, r + b.dirs + 0x10 * j, D + 0x40);
            fx.vu_scale4(sp + 0x5F0, sp + 0x5F0, f20);
            fx.vu_add4(sp + 0x5F0, sp + 0x5F0, D + 0x40);
            draw_at(fx, sp, b.r_fixu[1]);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 9 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// One of the two rises: `life` records of two blocks at the nodes 2 / 3.
struct Rise {
    /// D+.., the stride, the records' life (also their number), the
    /// sprites a block.
    base: u32,
    stride: u32,
    life: i32,
    n: u32,
    /// The record's points / directions.
    pos: u32,
    dirs: u32,
    cells: i32,
    dir_len: f32,
    tex: u32,
    r_rand: u32,
    r_alpha: u32,
    /// The key's 0x1A2C70 return address; `None`: far Z at key 0.
    r_key: Option<u32>,
}

const RISE_A: Rise = Rise {
    base: 0x7670,
    stride: 0x2D0,
    life: 0x1C,
    n: 0x14,
    pos: 0x30,
    dirs: 0x50,
    cells: 4,
    dir_len: f(0x3F19_999A),
    tex: 0x10,
    r_rand: 0x1E0_34F0,
    r_alpha: 0x1E0_36C8,
    r_key: Some(0x1E0_393C),
};
const RISE_B: Rise = Rise {
    base: 0xC530,
    stride: 0x100,
    life: 0xC,
    n: 6,
    pos: 0x20,
    dirs: 0x40,
    cells: 8,
    dir_len: 0.5,
    tex: 0x14,
    r_rand: 0x1E0_3AA8,
    r_alpha: 0x1E0_3C98,
    r_key: None,
};

/// On 1..0xD9 record n % life: per block its node's point, cells and
/// directions (not offset by the point).
fn rise_spawn(fx: &mut Fx, e: &Rise) {
    let n = fx.mem.i32(D);
    if !in_window(n, (1, CLOSE)) {
        return;
    }
    let r = D + e.base + e.stride * fx.div(n, e.life).1 as u32;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    for b in 0..2u32 {
        for c in 0..4u32 {
            let w = fx.mem.read_u32(D + 0x60 + 0x10 * b + 4 * c);
            fx.mem.write_u32(r + e.pos + 0x10 * b + 4 * c, w);
        }
        for j in 0..e.n {
            let c = fx.rand_from(OT, e.r_rand) >> 23;
            fx.mem.set8(r + 8 + e.n * b + j, rem2(c, e.cells));
            fx.random_table_vec(r + e.dirs + 0x10 * (e.n * b + j), e.dir_len);
        }
    }
}

/// [F 0x1E03590..0x1E03A00, 0x1E03B50..0x1E04060]
fn rise_draw(fx: &mut Fx, sp: u32, e: &Rise) {
    sprite_head(fx, sp);
    let d = sp + 0x640;
    fx.desc_clamp(d, 5);
    let rh = fx.mem.read_u32(D + e.tex);
    fx.desc_texture(d, rh, 1);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = window(n, CLOSE - n, 0x20, 1024.0);
    for k in 0..e.life as u32 {
        let r = D + e.base + e.stride * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let s = fx.tsin((age << 15).wrapping_div(e.life));
        let al = fixu(fx, ee::mul(f20, ee::mul(128.0, s)), e.r_alpha) as u8;
        let age = fx.mem.i32(r + 4);
        let left = ee::from_i32(e.life - age);
        let lf = ee::from_i32(e.life);
        let size = if e.r_key.is_none() {
            ee::mul(5.0, ee::add(f(0x3E99_999A), ee::div(ee::mul(f(0x3F33_3333), left), lf)))
        } else {
            ee::mul(f(0x3F4C_CCCD), ee::add(0.5, ee::div(ee::mul(0.5, left), lf)))
        };
        corners(fx, sp, size);
        let age_i = fx.mem.i32(r + 4);
        let age = ee::from_i32(age_i);
        let (spread, rise) = if e.r_key.is_none() {
            (ee::add(0.5, ee::div(ee::mul(0.5, age), lf)), ee::div(ee::mul(6.0, age), lf))
        } else {
            let t = fx.tan_falloff_p(age_i, e.life, 1);
            (ee::add(f(0x3F4C_CCCD), ee::mul(f(0x3E4C_CCCD), t)), ee::div(ee::mul(8.0, age), lf))
        };
        for b in 0..2u32 {
            rgba(fx, sp + 0x650, [0xFF, 0x3C, 0x1E], al);
            for j in 0..e.n {
                let m = e.n * b + j;
                let c = i32::from(fx.mem.read_u8(r + 8 + m));
                cell_uvs(fx, sp, c);
                fx.vu_scale4(sp + 0x5F0, r + e.dirs + 0x10 * m, spread);
                fx.vu_add4(sp + 0x5F0, sp + 0x5F0, r + e.pos + 0x10 * b);
                let y = fx.mem.f(sp + 0x5F4);
                fx.mem.setf(sp + 0x5F4, ee::add(y, rise));
                fx.vu_add4(sp + 0x5D0, sp + 0x5B0, sp + 0x5F0);
                fx.vu_add4(sp + 0x5E0, sp + 0x5C0, sp + 0x5F0);
                match e.r_key {
                    Some(ra) => draw(fx, sp, sp + 0x5D0, ra),
                    None => {
                        if crate::fxlib2::project(fx, sp + 0x5D0, PROJ_OUT, sp + 0x4F0, d, sp + 0x7AC) {
                            far_z(fx, sp + 0x660, &[sp + 0x670]);
                            fx.submit(d, OT, 0);
                        }
                    }
                }
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= e.life + 1 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The lens (48 / 32 updates) [F 0x1E00740..0x1E00A00].
fn lens1(fx: &mut Fx, sp: u32, a: [u32; 4]) {
    let l = H + LENS1;
    if fx.mem.read_u8(l) == 0 {
        return;
    }
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp - FS + LAYER_SLOT, layer);
    let regs = crate::lens::frame_copy(fx, a, 0, sp + 0x680, sp + 0x688);
    fx.texflush(regs);
    let age = fx.mem.i32(l + 4);
    let t = ee::div(ee::from_i32(16 - age), ee::from_i32(16));
    let al = fixu(fx, ee::mul(128.0, t), 0x1E0_082C) as u8;
    rgba(fx, sp + 0x620, [0xFF, 0x5A, 0x1C], al);
    far_xy(fx, sp + 0x628, 0x7000, 0x7000);
    far_xy(fx, sp + 0x630, 0x9000, 0x9000);
    fx.submit(sp + 0x610, 0, 0);
    rgba(fx, sp + 0x650, [0x80, 0x80, 0x80], 0x80u8.wrapping_sub(al));
    uv(fx, sp + 0x658, 0);
    uv(fx, sp + 0x65A, 0);
    uv(fx, sp + 0x668, 0x2000);
    uv(fx, sp + 0x66A, 0x2000);
    far_xy(fx, sp + 0x660, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x670, 0x8FF8, 0x8FF8);
    fx.submit(sp + 0x640, 0, 0);
    let age = fx.mem.i32(l + 4) + 1;
    fx.mem.set32(l + 4, age);
    if age >= 16 + 1 {
        fx.mem.write_u8(l, 0);
    }
    let layer = fx.mem.read_u32(sp - FS + LAYER_SLOT);
    fx.layer_end(layer, OT, 0);
}

/// The ten fans (D+0x78, 11 updates) [F 0x1E00A44..0x1E010A4].
fn fans(fx: &mut Fx, sp: u32) {
    fan_desc(fx, sp, 1);
    let n = fx.mem.i32(D);
    let a0 = Fx::clamp_i(n - (0), 0, 0x40);
    let b0 = Fx::clamp_i(CLOSE - n, 0, 0x40);
    let f21 = ee::div(ee::mul(ee::from_i32(a0), ee::from_i32(b0)), 4096.0);
    // a0..a3 as the second 0x101050 leaves them.
    let a = [b0 as u32, 0, 0x40, sp + 0x69A];
    let mut open = false;
    for k in 0..10u32 {
        let r = H + 0x78 + 8 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        if !open {
            open = true;
            let layer = fx.layer_begin();
            fx.mem.write_u32(sp - FS + LAYER_SLOT, layer);
            fx.texflush(a);
            let h = fx.mem.i32(D + 4);
            let (_, regs) = fx.unit_draw(h, 1, sp + 0x6F8, sp + 0x700);
            fx.texflush(regs);
        }
        let x = (fx.mem.i32(r + 4) + 1) << 15;
        let s = fx.tsin(x.wrapping_div(11));
        let al = fixu(fx, ee::mul(f21, ee::mul(255.0, s)), 0x1E0_0C20) as u8;
        rgba(fx, sp + 0x6A8, [0xFF, 0xBC, 0x30], al);
        let x = (fx.mem.i32(r + 4) + 1) << 15;
        let s = fx.tsin(x.wrapping_div(11));
        let al = fixu(fx, ee::mul(f21, ee::mul(128.0, s)), 0x1E0_0C94) as u8;
        rgba(fx, sp + 0x6C0, [0xFF, 0x64, 0x00], al);
        copy8(fx, sp + 0x6D8, sp + 0x6C0);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let t = ee::div(ee::mul(f(0x3E4C_CCCD), age), 10.0);
        let t = ee::add(1.0, ee::add(f(0xBDCC_CCCD), t));
        let f22 = ee::add(f(0x3C23_D70A), t);
        let z = u64::from(fx.mem.read_u32(H + 0x74) & 0xFF_FFFF);
        fan(fx, sp, H + 0x60, f22, z, 0, &R_FAN1);
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0xB {
            fx.mem.write_u8(r, 0);
        }
    }
    if open {
        let layer = fx.mem.read_u32(sp - FS + LAYER_SLOT);
        fx.layer_end(layer, OT, 0);
    }
}

/// The 48 x 8 (12) sparks (D+0x14D0) [F 0x1E01840..0x1E01CD8].
fn sparks(fx: &mut Fx, sp: u32) {
    sprite_head(fx, sp);
    let d = sp + 0x640;
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f21 = ee::div(ee::from_i32(Fx::clamp_i(CLOSE - n, 0, 0x20)), 32.0);
    for k in 0..0x30u32 {
        let r = H + 0x100 * k;
        if fx.mem.read_u8(r + SPARKS) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + SPARKS + 4));
        corners(fx, sp, ee::mul(18.0, lin(Lin(f(0x3E99_999A), f(0x3F33_3333)), age, 48.0)));
        let age = ee::from_i32(fx.mem.i32(r + SPARKS + 4));
        let pull = ee::div(ee::mul(f(0x3FD9_999A), age), 48.0);
        let spread = lin(Lin(f(0x3DCC_CCCD), f(0x3F66_6666)), age, 48.0);
        {
            let at = r + SPARKS + 0x70;
            fx.vu_sub4(sp + 0x600, H + 0x30, at);
            fx.vu_scale4(sp + 0x600, sp + 0x600, pull);
            fx.vu_add4(sp + 0x600, sp + 0x600, at);
            for j in 0..8 {
                let m = j;
                let age = ee::from_i32(fx.mem.i32(r + SPARKS + 4));
                let f22 = ee::mul(128.0, Fx::ratio_below(age, 1.0));
                let age = ee::from_i32(fx.mem.i32(r + SPARKS + 4));
                let f0 = Fx::ramp_above(age, 48.0, 32.0);
                let w = fx.mem.f(r + SPARKS + 0x48 + 4 * m);
                let al = fixu(fx, ee::mul(w, ee::mul(f21, ee::mul(f22, f0))), 0x1E0_1A88) as u8;
                rgba(fx, sp + 0x650, [0x80, 0x30, 0x1C], al);
                let b = fx.mem.i32(r + SPARKS + 8 + 4 * m);
                cell_uvs(fx, sp, b);
                let s = fx.mem.f(r + SPARKS + 0x28 + 4 * m);
                fx.vu_scale4(sp + 0x5D0, sp + 0x5B0, s);
                let s = fx.mem.f(r + SPARKS + 0x28 + 4 * m);
                fx.vu_scale4(sp + 0x5E0, sp + 0x5C0, s);
                fx.vu_scale4(sp + 0x5F0, r + SPARKS + 0x80 + 0x10 * m, spread);
                fx.vu_add4(sp + 0x5F0, sp + 0x5F0, sp + 0x600);
                fx.vu_add4(sp + 0x5D0, sp + 0x5D0, sp + 0x5F0);
                fx.vu_add4(sp + 0x5E0, sp + 0x5E0, sp + 0x5F0);
                draw(fx, sp, sp + 0x5D0, 0x1E0_1BD8);
            }
        }
        let age = fx.mem.i32(r + SPARKS + 4) + 1;
        fx.mem.set32(r + SPARKS + 4, age);
        if age >= 0x31 {
            fx.mem.write_u8(r + SPARKS, 0);
        }
    }
}

/// The 48 x 6 motes (0xB0) [F 0x1E01E40..0x1E022B8].
fn motes(fx: &mut Fx, sp: u32) {
    sprite_head(fx, sp);
    let d = sp + 0x640;
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f21 = ee::div(ee::from_i32(Fx::clamp_i(CLOSE - n, 0, 0x20)), 32.0);
    let (ang, pos, dirs, stride) = (0x20, 0x40, 0x50, 0xB0);
    for k in 0..0x30u32 {
        let r = H + MOTES + stride * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + 4));
        corners(fx, sp, ee::mul(12.0, lin(Lin(f(0x3E99_999A), f(0x3F33_3333)), age, 48.0)));
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let pull = ee::div(ee::mul(f(0x3FD9_999A), age), 48.0);
        {
            let at = r + pos;
            fx.vu_sub4(sp + 0x600, H + 0x30, at);
            fx.vu_scale4(sp + 0x600, sp + 0x600, pull);
            fx.vu_add4(sp + 0x600, sp + 0x600, at);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f22 = ee::mul(100.0, Fx::ratio_below(age, 1.0));
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f0 = Fx::ramp_above(age, 48.0, 32.0);
            let al = fixu(fx, ee::mul(f21, ee::mul(f22, f0)), 0x1E0_2038) as u8;
            rgba(fx, sp + 0x650, [0xFF, 0x40, 0x14], al);
            for j in 0..6u32 {
                let m = j;
                let b = fx.mem.i32(r + 8 + 4 * m);
                let age_i = fx.mem.i32(r + 4);
                let age = ee::from_i32(age_i);
                let s = lin(Lin(f(0x3DCC_CCCD), f(0x3F66_6666)), age, 48.0);
                let a = fx.mem.i32(r + ang + 4 * m).wrapping_add((age_i << 15).wrapping_div(48)) & 0xFFFF;
                let spread = ee::mul(s, fx.tsin(a));
                cell_uvs(fx, sp, b);
                fx.vu_scale4(sp + 0x5F0, r + dirs + 0x10 * m, spread);
                fx.vu_add4(sp + 0x5F0, sp + 0x5F0, sp + 0x600);
                draw_at(fx, sp, 0x1E0_21FC);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x31 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The 32 rings (0x50) [F 0x1E023B4..0x1E02838].
fn rings(fx: &mut Fx, sp: u32) {
    let d = sp + 0x710;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    let pts = H + RING_PTS;
    for k in 0..0x20u32 {
        let r = H + RINGS + 0x50 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let s = fx.tsin((age << 15) >> 5);
        rgba(fx, sp + 0x720, [0xE4, 0x26, 0x1A], 0);
        copy8(fx, sp + 0x738, sp + 0x720);
        let al = fixu(fx, ee::mul(100.0, s), 0x1E0_248C) as u8;
        rgba(fx, sp + 0x750, [0xFF, 0xAE, 0x5A], al);
        copy8(fx, sp + 0x768, sp + 0x750);
        fx.camera_vp_to(sp + 0x4F0);
        let t = [fx.mem.f(H + 0x30), fx.mem.f(H + 0x34), fx.mem.f(H + 0x38)];
        let m = fx.trs(None, None, Some(t));
        fx.write_m4(sp + 0x530, &m);
        let rm = fx.read_m4(r + 0x10);
        fx.mul_into(Some(sp + 0x530), &rm);
        let s1 = 2 * rem2(k as i32, 2) - 1;
        let age = fx.mem.i32(r + 4);
        fx.rotate_axis(sp + 0x530, s1.wrapping_mul(55 * age), 0);
        let age = fx.mem.i32(r + 4);
        fx.rotate_axis(sp + 0x530, s1.wrapping_mul(77 * age), 1);
        let age = fx.mem.i32(r + 4);
        fx.rotate_axis(sp + 0x530, s1.wrapping_mul(99 * age), 2);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let s = lin(Lin(f(0x3ECC_CCCD), f(0x3F19_999A)), age, 32.0);
        fx.mem.setf(sp + 0x5F0, s);
        fx.mem.setf(sp + 0x5F4, s);
        fx.mem.write_u32(sp + 0x5F8, 0);
        fx.mem.write_u32(sp + 0x5FC, 0x3F80_0000);
        fx.scale_rows(sp + 0x530, sp + 0x5F0);
        let m = fx.read_m4(sp + 0x530);
        fx.mul_into(Some(sp + 0x4F0), &m);
        let age = fx.mem.i32(r + 4);
        let f20 = ee::div(ee::mul(-1.0, ee::from_i32(rem2(age, 16))), 16.0);
        let f21 = ee::add(0.5, f20);
        let f22 = ee::add(1.0, f20);
        for s7 in 0..0x10u32 {
            let (s1, fp) = (pts + 0x30 * s7, pts + 0x30 * (s7 + 1));
            let f23 = ee::div(ee::mul(8.0, ee::from_i32(s7 as i32)), 16.0);
            let f24 = ee::div(ee::mul(8.0, ee::from_i32(s7 as i32 + 1)), 16.0);
            for (o, x) in [(0x728u32, f23), (0x72C, f20), (0x740, f24), (0x744, f20), (0x758, f23), (0x75C, f21), (0x770, f24), (0x774, f21)] {
                fx.mem.setf(sp + o, x);
            }
            let vs = [s1, fp, s1 + 0x20, fp + 0x20];
            if fx.project_n(&vs, sp + 0x4F0, d, sp + 0x7AC, 0x1E0_271C) == 1 {
                let w = fx.mem.f(sp + 0x7AC);
                let key = fixu(fx, w, 0x1E0_2730) as i32;
                fx.submit(d, OT, key);
            }
            for (o, x) in [(0x728u32, f23), (0x72C, f22), (0x740, f24), (0x744, f22), (0x758, f23), (0x75C, f21), (0x770, f24), (0x774, f21)] {
                fx.mem.setf(sp + o, x);
            }
            let vs = [s1 + 0x10, fp + 0x10, s1 + 0x20, fp + 0x20];
            if fx.project_n(&vs, sp + 0x4F0, d, sp + 0x7AC, 0x1E0_27A4) == 1 {
                let w = fx.mem.f(sp + 0x7AC);
                let key = fixu(fx, w, 0x1E0_27B8) as i32;
                fx.submit(d, OT, key);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The flat sprite at the target [F 0x1E02838..0x1E02AA0].
fn flat(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let f20 = window(n - 0x48, CLOSE - n, 0x20, 1024.0);
    if crate::exe::ee_le(f20, 0.0) {
        return;
    }
    sprite_head(fx, sp);
    let d = sp + 0x640;
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    uv(fx, sp + 0x658, 0x400);
    uv(fx, sp + 0x65A, 0);
    uv(fx, sp + 0x668, 0x600);
    uv(fx, sp + 0x66A, 0x200);
    fx.mem.setf(sp + 0x5B0, ee::neg(60.0));
    fx.mem.setf(sp + 0x5B4, ee::neg(60.0));
    fx.mem.write_u32(sp + 0x5B8, 0);
    fx.mem.write_u32(sp + 0x5BC, 0x3F80_0000);
    fx.mem.setf(sp + 0x5C0, 60.0);
    fx.mem.setf(sp + 0x5C4, 60.0);
    fx.mem.write_u32(sp + 0x5C8, 0);
    fx.mem.write_u32(sp + 0x5CC, 0x3F80_0000);
    let n = fx.mem.i32(D);
    let f1 = ee::add(f(0x3F66_6666), ee::mul(f(0x3DCC_CCCD), ee::from_i32(rem2(n, 2))));
    let al = fixu(fx, ee::mul(ee::mul(255.0, f20), f1), 0x1E0_29FC) as u8;
    rgba(fx, sp + 0x650, [0xFF, 0xA4, 0x64], al);
    fx.camera_vp_to(sp + 0x4F0);
    fx.translate_rows(sp + 0x530, sp + 0x570, H + 0x30);
    let m = fx.read_m4(sp + 0x530);
    fx.mul_into(Some(sp + 0x4F0), &m);
    draw(fx, sp, sp + 0x5B0, 0x1E0_2A74);
}

/// The second lens fan (no layer) [F 0x1E02AA0..0x1E02FF4].
fn lens2(fx: &mut Fx, sp: u32) {
    let l = H + LENS2;
    if fx.mem.i32(D) == 0x87 {
        fx.mem.write_u8(l, 1);
        fx.mem.write_u32(l + 4, 0);
    }
    if fx.mem.read_u8(l) != 1 {
        return;
    }
    fan_desc(fx, sp, 0);
    let d = sp + 0x698;
    fx.frame_desc(d, 1, [d + 0x10, d + 2]);
    let age = ee::from_i32(fx.mem.i32(l + 4));
    let f20 = Fx::ratio_below(age, 32.0);
    let age = ee::from_i32(fx.mem.i32(l + 4));
    let f0 = Fx::ramp_above(age, 115.0, 83.0);
    let al = fixu(fx, ee::mul(64.0, ee::mul(f20, f0)), 0x1E0_2B84) as u8;
    rgba(fx, sp + 0x6A8, [0xBC, 0x6C, 0x44], al);
    copy8(fx, sp + 0x6C0, sp + 0x6A8);
    copy8(fx, sp + 0x6D8, sp + 0x6A8);
    let s = ee::add(1.0, ee::mul(f(0x3D4C_CCCD), 1.0));
    fan(fx, sp, H + 0x40, s, 0xFF_FFF8, OT, &R_FAN2);
    let age = fx.mem.i32(l + 4) + 1;
    fx.mem.set32(l + 4, age);
    if age >= 0x74 {
        fx.mem.write_u8(l, 0);
    }
}

