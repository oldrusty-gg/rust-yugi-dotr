//! BMODULE b717 (card 472), hand-ported from the record (linked at
//! 0x1E00000; the one function 0x1E00080 in a 0x740 frame with D in s1 and
//! the OT in s0, data area a0 = 0x1ED0000, 0x65B0 bytes) [F] (helper C35,
//! 4 Nov 2026). b488's family (card 658, [`crate::b488`]) re-cut: the camera
//! sp + 0x3C0 (model + 0x40, turn + 0x80), corners sp + 0x480.., vertices
//! sp + 0x4C0.., the placed point sp + 0x540, the sprite descriptor sp +
//! 0x558, the quad sp + 0x5B0, the fan's triangle descriptor sp + 0x628, the
//! petals' quad descriptor sp + 0x6A0, the mean w sp + 0x73C. The init takes
//! the target's position (D+0x20), nodes 0x28 / 0x2F / 0x19 (D+0x70..) and
//! builds a flower of 8 rings x 16 shards (D+0xC0, 4 wide, 180 (i + 1) / 8
//! long, turned by 0x100E40 along (5 (7 - i) / 7 at the shard's angle, -180
//! i / 7 side) and a random z angle; cells D+0x20C0) and four thin quads
//! (D+0x5B90). Every update the camera turn with roll, the units' points
//! D+0x30 / 0x40, the three nodes (D+0x80..) and the window points of the
//! attacker's point (D+0x50) and the target's position (D+0x60). On 0xF4 / 0x11C / 0x14E a lens at the target for 0x31 updates
//! (D+0x5B20, b424's fan) and a burst at the target (D+0x2880 x 3: puffs,
//! sparks, halo, rays); on 0x5D / 0x8E / 0xC1 a flower head at node 0 / 1 / 2
//! (D+0x2140 x 3, 0xB updates; the head itself follows the node at its own
//! record index, its sprite stays) with 32 sparks (D+0x21B0 x 3, 0x41
//! updates); on 0x15..0x45 a flash record (D+0x5B28 + 8 (n % 12), 0xD
//! updates) drawing the attacker (0x1F8270) through the fan at its window
//! point; on 2 four quads and a sprite at node 2 (D+0x5C90, 0x11 updates); on
//! 0x14F..0x1B1 a rise of four sprites over the target's point (D+0x5CB0 x
//! 24, 0x19 updates). The stage light 0x80 - 80 clamp(0x19E - n, 0, 32)
//! clamp(n, 0, 32) / 1024; returns 1 on 0xEA, 6 on 0xF5..0x17D, 3 on 0x17E,
//! 8 once the count reaches 0x1CB.
use crate::b182::{alpha, clamp_regs, head, rgba};
use crate::b440::{copy8, copy_z, fix, rem2, vec4, words};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::LENS_CLAMP;

const D: u32 = crate::bmodule::DATA_AREA;
const FRAME: u32 = 0x740;
const PROJ_OUT: u32 = 0x002D_3600;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
const ONE: u32 = 0x3F80_0000;

const SHARDS: u32 = 0xC0;
const CELLS: u32 = 0x20C0;
const FLOWER: u32 = 0x2140;
const FLOWER_IDX: u32 = 0xB0;
const SPARK_IDX: u32 = 0x21A0;
const SPARKS: u32 = 0x21B0;
const BURST_IDX: u32 = 0x2870;
const BURST: u32 = 0x2880;
const LENS: u32 = 0x5B20;
const FLASH: u32 = 0x5B28;
const QUADS: u32 = 0x5B90;
const QUAD: u32 = 0x5C90;
const RISE: u32 = 0x5CB0;

const LENS_RA: [u32; 12] = [
    0x01E0_09F0, 0x01E0_09FC, 0x01E0_0A40, 0x01E0_0A4C, 0x01E0_0A90, 0x01E0_0A9C, 0x01E0_0B50, 0x01E0_0B5C, 0x01E0_0BC8, 0x01E0_0BD4, 0x01E0_0C40,
    0x01E0_0C4C,
];
const FLASH_RA: [u32; 12] = [
    0x01E0_2B38, 0x01E0_2B44, 0x01E0_2B8C, 0x01E0_2B98, 0x01E0_2BE0, 0x01E0_2BEC, 0x01E0_2CA4, 0x01E0_2CB0, 0x01E0_2D20, 0x01E0_2D2C, 0x01E0_2DA0,
    0x01E0_2DAC,
];

/// The record's working slots.
struct Fr {
    sp: u32,
    vp: u32,
    model: u32,
    turn: u32,
    c0: u32,
    c1: u32,
    v0: u32,
    pl: u32,
    d: u32,
    g: u32,
    t: u32,
    q: u32,
    mean: u32,
}

impl Fr {
    fn new(sp: u32) -> Fr {
        let vp = sp + 0x3C0;
        Fr {
            sp,
            vp,
            model: vp + 0x40,
            turn: vp + 0x80,
            c0: sp + 0x480,
            c1: sp + 0x490,
            v0: sp + 0x4C0,
            pl: sp + 0x540,
            d: sp + 0x558,
            g: sp + 0x5B0,
            t: sp + 0x628,
            q: sp + 0x6A0,
            mean: sp + 0x73C,
        }
    }
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
    let f = Fr::new(sp);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, &f) };
    fx.leave(FRAME);
    ret
}

/// rand >> 23 from `ra` with the caller's s0.
fn rnd(fx: &mut Fx, s0: u32, ra: u32) -> i32 {
    fx.rand_from(s0, ra) >> 23
}

/// (x x 0x2AAAAAAB >> 32 >> 3) + sign: x / 48.
fn div48(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    (fx.hilo().1 >> 3).wrapping_add(((x as u32) >> 31) as i32)
}

// ------------------------------------------------------------ init

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0x65B0);
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
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    for (k, (node, clear)) in [(0x28, 1), (0x2F, 0), (0x19, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0x70 + 4 * k as u32, r);
    }
    flower_init(fx, sp);
    quads_init(fx, sp);
    // The record's flag loops 0x1E00388.. clear what the bzero cleared.
    0
}

/// The flower: ring i's shards (-2..2 wide, 180 (i + 1) / 8 long) aimed
/// (0x100E40) along (5 (7 - i) / 7 at the shard's angle, -180 i / 7 side),
/// turned by a random z angle [F b717 0x1E00188..0x1E00380].
fn flower_init(fx: &mut Fx, sp: u32) {
    let (m, vv) = (sp + 0x500, sp + 0x540);
    for i in 0..8i32 {
        let f20 = ee::div(ee::mul(180.0, ee::from_i32(i + 1)), 8.0);
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let f2 = ee::mul(ee::div(ee::mul(180.0, ee::from_i32(i)), 7.0), side);
        let f22 = ee::mul(-1.0, f2);
        let f21 = ee::div(ee::mul(5.0, ee::from_i32(7 - i)), 7.0);
        let s7 = (0x2000 * i) >> 3;
        for j in 0..0x10i32 {
            let a = (((j << 16) >> 4).wrapping_add(s7)) & 0xFFFF;
            let sn = fx.tsin(a);
            fx.mem.setf(vv, ee::mul(f21, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(vv + 4, ee::mul(f21, cs));
            fx.mem.setf(vv + 8, f22);
            fx.mem.write_u32(vv + 0xC, ONE);
            fx.point_along(m, vv);
            let r = rnd(fx, j as u32, 0x01E0_026C);
            fx.rotate_axis(m, (rem2(r, 256) << 16) >> 8, 2);
            let q = D + 0x400 * i as u32 + 0x40 * j as u32 + SHARDS;
            let (n, p, z) = (0xC000_0000u32, 0x4000_0000u32, f20.to_bits());
            vec4(fx, q, [n, 0, 0, ONE]);
            vec4(fx, q + 0x10, [p, 0, 0, ONE]);
            vec4(fx, q + 0x20, [n, 0, z, ONE]);
            vec4(fx, q + 0x30, [p, 0, z, ONE]);
            for k in 0..4u32 {
                fx.vu_apply_to(q + 0x10 * k, m, q + 0x10 * k);
            }
            let r = rnd(fx, j as u32, 0x01E0_0318);
            fx.mem.set8(D + 0x10 * i as u32 + j as u32 + CELLS, rem2(r, 4) + 4);
        }
    }
}

/// The four thin quads (0.4 wide, 10 (1 - 0.4 (k % 2)) long), each turned
/// a quarter more about z [F b717 0x1E00424..0x1E00528].
fn quads_init(fx: &mut Fx, sp: u32) {
    let (m, c) = (sp + 0x500, sp + 0x480);
    let w = 0x3ECC_CCCDu32;
    let nw = ee::neg(f32::from_bits(w)).to_bits();
    for k in 0..4i32 {
        let f3 = ee::from_i32(k % 2);
        let f1 = ee::mul(10.0, ee::sub(1.0, ee::mul(f32::from_bits(w), f3)));
        let (p, n) = (f1.to_bits(), ee::neg(f1).to_bits());
        vec4(fx, c, [nw, n, 0, ONE]);
        vec4(fx, c + 0x10, [w, n, 0, ONE]);
        vec4(fx, c + 0x20, [nw, p, 0, ONE]);
        vec4(fx, c + 0x30, [w, p, 0, ONE]);
        fx.identity(m);
        fx.rotate_axis(m, (0x8000 * k) >> 2, 2);
        for j in 0..4u32 {
            fx.vu_apply_to(D + QUADS + 0x40 * k as u32 + 0x10 * j, m, c + 0x10 * j);
        }
    }
}

// ------------------------------------------------------------ shared parts

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, f: &Fr, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, f.d + 0x18, u0);
    uv(fx, f.d + 0x1A, v0);
    uv(fx, f.d + 0x28, u1);
    uv(fx, f.d + 0x2A, v1);
}

/// The 4 x 8 cell `b` of the texture into the sprite's two UVs.
fn cell_uvs(fx: &mut Fx, f: &Fr, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    let m = |x: i32| (x & 0x3FFF) as u32;
    uvs(fx, f, m(c << 9), m(r << 9), m(((c << 5) + 0x20) << 4), m(((r << 5) + 0x20) << 4));
}

fn camera(fx: &mut Fx, f: &Fr) {
    fx.camera_vp_to(f.vp);
    fx.identity(f.model);
    let id = fx.read_m4(f.model);
    fx.mul_into(Some(f.vp), &id);
}

/// The sprite descriptor rewritten (prim 0x26 | 0x800), its texture D +
/// `tex`, clamp 5 and (with `preset`) its ALPHA.
fn desc_reset(fx: &mut Fx, f: &Fr, tex: u32, preset: Option<i32>) {
    head(fx, f.d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(f.d, rh, 1);
    fx.desc_clamp(f.d, 5);
    if let Some(p) = preset {
        alpha(fx, f.d, p);
    }
}

/// A descriptor with its ALPHA first, then the texture D + `tex`, clamp 5.
fn desc_alpha_first(fx: &mut Fx, d: u32, prim: u16, n: u16, regs: u64, preset: i32, tex: u32) {
    head(fx, d, prim, n, regs, 0x800);
    alpha(fx, d, preset);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
}

/// The corners (-s, -s, 0, 1) and (s, s, 0, 1) (turned by the camera turn
/// with `turn`).
fn corners(fx: &mut Fx, f: &Fr, s: f32, turn: bool) {
    let n = ee::neg(s);
    vec4(fx, f.c0, [n.to_bits(), n.to_bits(), 0, ONE]);
    vec4(fx, f.c1, [s.to_bits(), s.to_bits(), 0, ONE]);
    if turn {
        fx.vu_apply_to(f.c0, f.turn, f.c0);
        fx.vu_apply_to(f.c1, f.turn, f.c1);
    }
}

fn colour(fx: &mut Fx, f: &Fr, c: [u8; 3], a: u8) {
    rgba(fx, f.d + 0x10, c, a);
}

/// The sprite's vertices around the placed point.
fn around(fx: &mut Fx, f: &Fr) {
    fx.vu_add4(f.v0, f.c0, f.pl);
    fx.vu_add4(f.v0 + 0x10, f.c1, f.pl);
}

/// The placed point (= dir x s + at) and the sprite's vertices around it.
fn place(fx: &mut Fx, f: &Fr, dir: u32, s: f32, at: u32) {
    fx.vu_scale4(f.pl, dir, s);
    fx.vu_add4(f.pl, f.pl, at);
    around(fx, f);
}

fn project(fx: &mut Fx, f: &Fr, input: u32, d: u32) -> bool {
    crate::fxlib2::project(fx, input, PROJ_OUT, f.vp, d, f.mean)
}

/// Projected from `input` and submitted at the mean w (0x1A2C70 from `ra`).
fn draw(fx: &mut Fx, f: &Fr, d: u32, input: u32, ra: u32) {
    if project(fx, f, input, d) {
        let w = fx.mem.f(f.mean);
        let key = fix(fx, w, ra);
        fx.submit(d, OT, key as i32);
    }
}

fn far(fx: &mut Fx, f: &Fr) {
    let x = fx.mem.read_u64(f.d + 0x20);
    fx.mem.write_u64(f.d + 0x20, (x & KEEP) | FAR_Z);
    copy_z(fx, f.d + 0x30, f.d + 0x20);
}

/// Projected from `input` and submitted at far Z (key 0).
fn draw_far(fx: &mut Fx, f: &Fr, input: u32) {
    if project(fx, f, input, f.d) {
        far(fx, f);
        fx.submit(f.d, OT, 0);
    }
}

/// The window point of `p` (0x204D80 through the camera into sp + 0x730)
/// into `o`: x / y ((x16 >> 4) - 0x700) clamped to 0..512, Z, 1.0.
fn screen_point(fx: &mut Fx, f: &Fr, p: u32, o: u32) {
    camera(fx, f);
    let m = fx.read_m4(f.vp);
    let sp = f.sp;
    fx.project_at(p, &m, sp + 0x730, sp + 0x73C);
    let x = (i32::from(fx.mem.read_u16(sp + 0x730)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x732)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x730) << 8) >> 40;
    fx.mem.setf(o + 8, ee::from_i32(z as i32));
    fx.mem.write_u32(o + 0xC, ONE);
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

// ------------------------------------------------------------ step

fn step(fx: &mut Fx, f: &Fr) -> i32 {
    let sp = f.sp;
    fx.camera_turn_roll(f.turn);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x40);
    for k in 0..3u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x70 + 4 * k);
        fx.node_position(h, s, D + 0x80 + 0x10 * k);
    }
    screen_point(fx, f, D + 0x30, D + 0x50);
    screen_point(fx, f, D + 0x20, D + 0x60);
    lens(fx, f);
    let node = match fx.mem.i32(D) {
        0xC1 => Some(2),
        0x8E => Some(1),
        0x5D => Some(0),
        _ => None,
    };
    if let Some(a) = node {
        flower_spawn(fx, a);
    }
    flowers(fx, f);
    flower_sparks(fx, f);
    if matches!(fx.mem.i32(D), 0x14E | 0x11C | 0xF4) {
        burst_spawn(fx);
    }
    bursts(fx, f);
    let n = fx.mem.i32(D);
    if (0x15..0x46).contains(&n) {
        let k = fx.div(n, 0xC).1 as u32;
        fx.mem.write_u8(D + FLASH + 8 * k, 1);
        fx.mem.write_u32(D + FLASH + 8 * k + 4, 0);
    }
    flashes(fx, f);
    if fx.mem.i32(D) == 2 {
        words(fx, f.pl, D + 0xA0);
        fx.mem.write_u8(D + QUAD, 1);
        fx.mem.write_u32(D + QUAD + 4, 0);
        fx.mem.write_u32(D + QUAD + 8, ONE);
        words(fx, D + QUAD + 0x10, f.pl);
    }
    quads(fx, f);
    let n = fx.mem.i32(D);
    if (0x14F..0x1B2).contains(&n) {
        rise_spawn(fx, n);
    }
    rises(fx, f);
    let n = fx.mem.i32(D);
    let c1 = Fx::clamp_i(0x19E - n, 0, 0x20);
    let n = fx.mem.i32(D);
    let c2 = Fx::clamp_i(n, 0, 0x20);
    let l = fx.mult(c1, c2.wrapping_mul(-80));
    fx.set_stage_light(((l >> 10) + 0x80) & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xB0);
    if n == 0xEA {
        ret = 1;
    } else {
        if (0xF5..0x17E).contains(&n) {
            ret = 6;
        }
        if n == 0x17E {
            ret = 3;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x1CB {
        ret = 8;
    }
    fx.mem.set32(sp + 0xB0, ret);
    ret
}

/// The lens at the target's window point (D+0x5B20 for 0x31 updates) [F
/// b717 0x1E00808..0x1E00D18].
fn lens(fx: &mut Fx, f: &Fr) {
    if matches!(fx.mem.i32(D), 0x14E | 0x11C | 0xF4) {
        fx.mem.write_u8(D + LENS, 1);
        fx.mem.write_u32(D + LENS + 4, 0);
    }
    if fx.mem.read_u8(D + LENS) != 1 {
        return;
    }
    let (sp, t) = (f.sp, f.t);
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0x800);
    alpha(fx, t, 0);
    fx.desc_clamp(t, LENS_CLAMP);
    let a = clamp_regs(fx, t, LENS_CLAMP);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xEC, layer);
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x688, sp + 0x690);
    fx.texflush(regs);
    let age = fx.mem.i32(D + LENS + 4);
    let al = div48(fx, (0x30 - age) << 6);
    rgba(fx, t + 0x10, [0x80; 3], al as u8);
    copy8(fx, t + 0x28, t + 0x10);
    copy8(fx, t + 0x40, t + 0x10);
    for j in 0..8i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f32::from_bits(0x3E19_999A), ee::from_i32(8 - j)), 8.0));
        crate::b424::fan(fx, t, D + 0x60, sp + 0x4C0, s, 0, &LENS_RA);
    }
    let layer = fx.mem.read_u32(sp + 0xEC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + LENS + 4) + 1;
    fx.mem.set32(D + LENS + 4, age);
    if age >= 0x31 {
        fx.mem.write_u8(D + LENS, 0);
    }
}

/// The flower record (D+0x2140 + 0x20 k: a random z angle and node `a`'s
/// point) and its sparks (D+0x21B0 + 0x240 m: 32 cells and 0.8 vectors) [F
/// b717 0x1E00D70..0x1E00EF8].
fn flower_spawn(fx: &mut Fx, a: u32) {
    let at = D + 0x80 + 0x10 * a;
    let k = fx.mem.i32(D + FLOWER_IDX) as u32;
    let fb = D + 0x20 * k + FLOWER;
    fx.mem.write_u8(fb, 1);
    fx.mem.write_u32(fb + 4, 0);
    let m = fx.mem.i32(D + SPARK_IDX) as u32;
    let tb = D + 0x240 * m + SPARKS;
    fx.mem.write_u8(tb, 1);
    fx.mem.write_u32(tb + 4, 0);
    let r = rnd(fx, OT, 0x01E0_0DC4);
    fx.mem.set32(fb + 8, (rem2(r, 256) << 16) >> 8);
    words(fx, fb + 0x10, at);
    words(fx, tb + 0x30, at);
    for j in 0..0x20u32 {
        let r = rnd(fx, OT, 0x01E0_0E40);
        fx.mem.set8(tb + 8 + j, rem2(r, 8));
        fx.random_table_vec(tb + 0x40 + 0x10 * j, f32::from_bits(0x3F4C_CCCD));
    }
    let k = fx.mem.i32(D + FLOWER_IDX) + 1;
    fx.mem.set32(D + FLOWER_IDX, if k < 3 { k } else { 0 });
    let m = fx.mem.i32(D + SPARK_IDX) + 1;
    fx.mem.set32(D + SPARK_IDX, if m < 3 { m } else { 0 });
}

/// The flowers [F b717 0x1E00EFC..0x1E01500]: heads of 8 x 16 shards at the
/// node of their record's index (alpha 64 (10 - age) / 10, (3C, 64, 80),
/// 0.5 + age / 20 wide), then a 100-wide far sprite (D7, 64, 3C) at alpha 0
/// (the 0x100D50 falloff computed and dropped) submitted twice at the
/// stored point, 0xB updates.
fn flowers(fx: &mut Fx, f: &Fr) {
    let q = f.q;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(q, rh, 1);
    alpha(fx, q, 1);
    fx.desc_clamp(q, 0);
    for k in 0..3u32 {
        let fb = D + 0x20 * k + FLOWER;
        if fx.mem.read_u8(fb) != 0 {
            flower(fx, f, fb, D + 0x80 + 0x10 * k);
        }
    }
}

fn flower(fx: &mut Fx, f: &Fr, fb: u32, node: u32) {
    let q = f.q;
    let x = (10 - fx.mem.i32(fb + 4)) << 6;
    fx.mult(0x6666_6667, x);
    let a = (fx.hilo().1 >> 2).wrapping_add(((x as u32) >> 31) as i32);
    rgba(fx, q + 0x10, [0x3C, 0x64, 0x80], a as u8);
    for o in [0x28u32, 0x40, 0x58] {
        copy8(fx, q + o, q + 0x10);
    }
    let age = ee::from_i32(fx.mem.i32(fb + 4));
    let s = ee::add(0.5, ee::div(ee::mul(0.5, age), 10.0));
    vec4(fx, f.pl, [s.to_bits(), s.to_bits(), s.to_bits(), ONE]);
    fx.camera_vp_to(f.vp);
    fx.identity(f.model);
    fx.translate_rows(f.model, f.model, node);
    fx.rotate_axis(f.model, 0, 0);
    let ang = fx.mem.i32(fb + 8);
    fx.rotate_axis(f.model, ang, 2);
    fx.scale_rows(f.model, f.pl);
    let mm = fx.read_m4(f.model);
    fx.mul_into(Some(f.vp), &mm);
    shards(fx, f);
    desc_reset(fx, f, 0x14, Some(1));
    camera(fx, f);
    corners(fx, f, 100.0, true);
    let age = fx.mem.i32(fb + 4);
    fx.tan_falloff_p(age, 0xA, 1);
    let a = fix(fx, 0.0, 0x01E0_1308);
    colour(fx, f, [0xD7, 0x64, 0x3C], a as u8);
    words(fx, f.pl, fb + 0x10);
    around(fx, f);
    uvs(fx, f, 0x600, 0, 0x800, 0x200);
    if project(fx, f, f.v0, f.d) {
        far(fx, f);
        fx.submit(f.d, OT, 0);
        fx.submit(f.d, OT, 0);
    }
    let age = fx.mem.i32(fb + 4) + 1;
    fx.mem.set32(fb + 4, age);
    if age >= 0xB {
        fx.mem.write_u8(fb, 0);
    }
}

/// The 8 x 16 shards through the camera matrix (0x205210), keyed by their
/// mean w.
fn shards(fx: &mut Fx, f: &Fr) {
    let q = f.q;
    let mm = fx.read_m4(f.vp);
    for i in 0..8u32 {
        for j in 0..0x10u32 {
            let cb = i32::from(fx.mem.read_u8(D + 0x10 * i + j + CELLS));
            let c = rem2(cb, 4) << 5;
            let r = (cb >> 2) << 5;
            let f4 = ee::from_i32(c);
            let f2 = ee::from_i32(r);
            let f5 = ee::div(ee::add(32.0, f4), 128.0);
            let f4 = ee::div(f4, 128.0);
            let f3 = ee::div(f2, 64.0);
            let f0 = ee::div(ee::add(16.0, f2), 64.0);
            for (o, x) in [(0x18u32, f4), (0x1C, f0), (0x30, f5), (0x34, f0), (0x48, f4), (0x4C, f3), (0x60, f5), (0x64, f3)] {
                fx.mem.setf(q + o, x);
            }
            let shard = D + 0x400 * i + 0x40 * j + SHARDS;
            if fx.project_desc(shard, PROJ_OUT, Some(&mm), q, f.mean) {
                let w = fx.mem.f(f.mean);
                let key = fix(fx, w, 0x01E0_11BC);
                fx.submit(q, OT, key as i32);
            }
        }
    }
}

/// The flowers' sparks: 10 x (0.5 + age / 128) wide, white at alpha 0, 0x41
/// updates [F b717 0x1E01504..0x1E0184C].
fn flower_sparks(fx: &mut Fx, f: &Fr) {
    desc_reset(fx, f, 0x10, Some(0));
    camera(fx, f);
    for k in 0..3u32 {
        let tb = D + 0x240 * k + SPARKS;
        if fx.mem.read_u8(tb) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(tb + 4));
        let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 64.0)));
        corners(fx, f, s, true);
        colour(fx, f, [0xFF; 3], 0);
        let age = fx.mem.i32(tb + 4);
        let f20 = fx.tan_falloff_p(age, 0x40, 1);
        for j in 0..0x20u32 {
            place(fx, f, tb + 0x40 + 0x10 * j, f20, tb + 0x30);
            let c = i32::from(fx.mem.read_u8(tb + 8 + j));
            cell_uvs(fx, f, c);
            draw(fx, f, f.d, f.v0, 0x01E0_17A8);
        }
        let age = fx.mem.i32(tb + 4) + 1;
        fx.mem.set32(tb + 4, age);
        if age >= 0x41 {
            fx.mem.write_u8(tb, 0);
        }
    }
}

/// The burst record (D+0x2880 + 0x10E0 b) at the target's position [F b717
/// 0x1E01890..0x1E01B00]: age 1, point +0x90, 64 spark cells (+8, halfwords)
/// and 2.5 vectors (+0xA0), 128 ray vectors 3.0 (+0x4A0), 64 puff cells
/// (+0xCA0, rand % 3 + 5) and 2.5 vectors (+0xCE0).
fn burst_spawn(fx: &mut Fx) {
    let b = fx.mem.i32(D + BURST_IDX) as u32;
    let r = D + 0x10E0 * b + BURST;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 1);
    vec4(fx, r + 0x90, [0, 0, 0, ONE]);
    fx.vu_add4(r + 0x90, r + 0x90, D + 0x20);
    for j in 0..0x40u32 {
        let c = rnd(fx, OT, 0x01E0_1994);
        fx.mem.write_u16(r + 8 + 2 * j, rem2(c, 8) as u16);
        fx.random_table_vec(r + 0xA0 + 0x10 * j, 2.5);
    }
    for m in 0..0x80u32 {
        fx.random_table_vec(r + 0x4A0 + 0x10 * m, 3.0);
    }
    for j in 0..0x40u32 {
        let c = rnd(fx, OT, 0x01E0_1A6C);
        let (_, h) = fx.div(c, 3);
        fx.mem.set8(r + 0xCA0 + j, h + 5);
        fx.random_table_vec(r + 0xCE0 + 0x10 * j, 2.5);
    }
    let b = fx.mem.i32(D + BURST_IDX) + 1;
    fx.mem.set32(D + BURST_IDX, if b < 3 { b } else { 0 });
}

/// `size` x (0.8 + 0.2 age / 48).
fn burst_size(fx: &mut Fx, r: u32, size: f32) -> f32 {
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let s = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), age), 48.0);
    ee::mul(size, ee::add(f32::from_bits(0x3F4C_CCCD), s))
}

/// The bursts [F b717 0x1E01B08..0x1E0285C].
fn bursts(fx: &mut Fx, f: &Fr) {
    for b in 0..3u32 {
        let r = D + 0x10E0 * b + BURST;
        if fx.mem.read_u8(r) != 0 {
            burst(fx, f, r);
        }
    }
}

fn burst(fx: &mut Fx, f: &Fr, r: u32) {
    let pt = r + 0x90;
    // Puffs.
    desc_reset(fx, f, 0x18, Some(1));
    camera(fx, f);
    let s = burst_size(fx, r, 15.0);
    corners(fx, f, s, true);
    let age = fx.mem.i32(r + 4);
    let a = div48(fx, (0x30 - age) * 200);
    colour(fx, f, [0x32, 0x80, 0xFF], a as u8);
    let age = fx.mem.i32(r + 4);
    let f22 = fx.tan_falloff_p(age, 0x30, 1);
    for j in 0..0x40u32 {
        place(fx, f, r + 0xCE0 + 0x10 * j, f22, pt);
        let c = i32::from(fx.mem.read_u8(r + 0xCA0 + j));
        cell_uvs(fx, f, c);
        draw_far(fx, f, f.v0);
    }
    // Sparks in two colours.
    desc_reset(fx, f, 0x10, None);
    camera(fx, f);
    let s = burst_size(fx, r, 13.0);
    corners(fx, f, s, true);
    let age = fx.mem.i32(r + 4);
    let a255 = div48(fx, (0x30 - age) * 255);
    let a128 = div48(fx, (0x30 - age) << 7);
    for j in 0..0x40u32 {
        let age = fx.mem.i32(r + 4);
        let mut f22 = fx.tan_falloff_p(age, 0x30, 1);
        if j % 2 == 0 {
            f22 = ee::mul(f22, f32::from_bits(0x3F4C_CCCD));
            alpha(fx, f.d, 2);
            colour(fx, f, [0xFF, 0xFF, 0x80], a128 as u8);
        } else {
            alpha(fx, f.d, 1);
            colour(fx, f, [0x1E, 0x60, 0xFF], a255 as u8);
        }
        place(fx, f, r + 0xA0 + 0x10 * j, f22, pt);
        let c = i32::from(fx.mem.read_u16(r + 8 + 2 * j));
        cell_uvs(fx, f, c);
        draw_far(fx, f, f.v0);
    }
    // The halo: the camera at the point scaled by 0.2 + 0.8 falloff.
    desc_reset(fx, f, 0x14, Some(1));
    fx.camera_vp_to(f.vp);
    fx.translate_rows(f.model, f.turn, pt);
    let age = fx.mem.i32(r + 4);
    let tf = fx.tan_falloff_p(age, 0x30, 1);
    let s = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf));
    vec4(fx, f.pl, [s.to_bits(), s.to_bits(), s.to_bits(), ONE]);
    fx.scale_rows(f.model, f.pl);
    let mm = fx.read_m4(f.model);
    fx.mul_into(Some(f.vp), &mm);
    let age = fx.mem.i32(r + 4);
    let a = fix(fx, ee::mul(200.0, ee::div(ee::from_i32(0x30 - age), 48.0)), 0x01E0_23FC);
    colour(fx, f, [0x3A, 0x5E, 0xFF], a as u8);
    uvs(fx, f, 0x600, 0, 0x800, 0x200);
    corners(fx, f, 130.0, false);
    draw_far(fx, f, f.c0);
    // The rays: 128 of six sprites, (80, F0, FF) at alpha 0.
    desc_reset(fx, f, 0x14, Some(1));
    uvs(fx, f, 0x600, 0, 0x800, 0x200);
    camera(fx, f);
    corners(fx, f, 1.0, true);
    let age = fx.mem.i32(r + 4);
    let f22 = fx.tan_falloff_p(age, 0x30, 1);
    for m in 0..0x80u32 {
        let dir = r + 0x4A0 + 0x10 * m;
        for s4 in 0..6i32 {
            colour(fx, f, [0x80, 0xF0, 0xFF], 0);
            let f12 = ee::add(f22, ee::mul(f32::from_bits(0x3BE5_6042), ee::from_i32(s4)));
            place(fx, f, dir, f12, pt);
            draw(fx, f, f.d, f.v0, 0x01E0_27C0);
        }
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x31 {
        fx.mem.write_u8(r, 0);
    }
}

/// The flash records (D+0x5B28, 12 x 8): the first live one opens a layer
/// with the attacker drawn (0x1F8270); each draws b424's fan at the
/// attacker's window point, (94, C8, FF) at alpha f 128 sin((age + 1) / 13
/// half turn) to (0, 0, FF) at 0, f = clamp(n - 0x14, 0, 16) clamp(0x46 - n,
/// 0, 16) / 256, 1 + 0.4 age / 12 - 0.2 + 1 / 60 wide, 0xD updates [F b717
/// 0x1E028A8..0x1E02EA8].
fn flashes(fx: &mut Fx, f: &Fr) {
    let (sp, t) = (f.sp, f.t);
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0x800);
    alpha(fx, t, 1);
    fx.desc_clamp(t, LENS_CLAMP);
    let n = fx.mem.i32(D);
    let c1 = Fx::clamp_i(n - 0x14, 0, 0x10);
    let n = fx.mem.i32(D);
    let c2 = Fx::clamp_i(0x46 - n, 0, 0x10);
    let f21 = ee::div(ee::mul(ee::from_i32(c1), ee::from_i32(c2)), 256.0);
    let f20 = ee::mul(f21, 0.0);
    let mut open = false;
    for k in 0..0xCu32 {
        let r = D + FLASH + 8 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        if !open {
            open = true;
            let l = fx.layer_begin();
            fx.mem.write_u32(sp + 0xEC, l);
            // 0x101050 leaves a0 = its answer, a1 = 0, a2 = 0x10; a3 = t + 2
            // from 0x208400.
            fx.texflush([c2 as u32, 0, 0x10, t + 2]);
            let h = fx.mem.i32(D + 4);
            let (_, regs) = fx.unit_draw(h, 1, sp + 0x688, sp + 0x690);
            fx.texflush(regs);
        }
        let sn = half_turn(fx, r);
        let a = fix(fx, ee::mul(f21, ee::mul(128.0, sn)), 0x01E0_2A30);
        rgba(fx, t + 0x10, [0x94, 0xC8, 0xFF], a as u8);
        half_turn(fx, r);
        let a = fix(fx, f20, 0x01E0_2A94);
        rgba(fx, t + 0x28, [0, 0, 0xFF], a as u8);
        copy8(fx, t + 0x40, t + 0x28);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let g = ee::div(ee::mul(f32::from_bits(0x3ECC_CCCD), age), 12.0);
        let g = ee::add(1.0, ee::add(f32::from_bits(0xBE4C_CCCD), g));
        let f22 = ee::add(f32::from_bits(0x3C88_8889), g);
        crate::b424::fan(fx, t, D + 0x50, sp + 0x4C0, f22, 0, &FLASH_RA);
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0xD {
            fx.mem.write_u8(r, 0);
        }
    }
    if open {
        let l = fx.mem.read_u32(sp + 0xEC);
        fx.layer_end(l, OT, 0);
    }
}

/// sin(((age + 1) << 15) / 13).
fn half_turn(fx: &mut Fx, r: u32) -> f32 {
    let x = (fx.mem.i32(r + 4) + 1) << 15;
    fx.mult(0x4EC4_EC4F, x);
    let q = (fx.hilo().1 >> 2).wrapping_add(((x as u32) >> 31) as i32);
    fx.tsin(q)
}

/// The quad record (D+0x5C90: the four thin quads scaled by +8 (1.0), far,
/// (86, 68, FF) at 255 (1 - falloff), then a 3-wide far sprite at 255 ramp,
/// at the third node's live point, 0x11 updates) [F b717
/// 0x1E02F34..0x1E03568].
fn quads(fx: &mut Fx, f: &Fr) {
    let g = f.g;
    desc_alpha_first(fx, g, 0x24, 9, 0x4_2424_2421, 1, 0x14);
    for (o, u, v) in [(0x18u32, 0u32, 0x200u32), (0x28, 0x200, 0x200), (0x38, 0, 0x400), (0x48, 0x200, 0x400)] {
        uv(fx, g + o, u);
        uv(fx, g + o + 2, v);
    }
    desc_alpha_first(fx, f.d, 0x26, 5, 0x4_2421, 1, 0x14);
    uvs(fx, f, 0x600, 0, 0x800, 0x200);
    camera(fx, f);
    let r = D + QUAD;
    if fx.mem.read_u8(r) == 0 {
        return;
    }
    let age = fx.mem.i32(r + 4);
    let tf = fx.tan_falloff_p(age, 0x10, 1);
    let a = fix(fx, ee::mul(255.0, ee::sub(1.0, tf)), 0x01E0_318C);
    rgba(fx, g + 0x10, [0x86, 0x68, 0xFF], a as u8);
    let f20 = fx.mem.f(r + 8);
    let (c, v) = (f.c0, f.v0);
    for m in 0..4u32 {
        for k in 0..4u32 {
            let (s, o) = (c + 0x10 * k, v + 0x10 * k);
            fx.vu_scale4(s, D + QUADS + 0x40 * m + 0x10 * k, f20);
            fx.vu_apply_to(s, f.turn, s);
            fx.vu_add4(o, s, D + 0xA0);
        }
        if project(fx, f, v, g) {
            crate::lens::far_z(fx, g + 0x20, &[g + 0x30, g + 0x40, g + 0x50]);
            fx.submit(g, OT, 0);
        }
    }
    let s = ee::mul(3.0, fx.mem.f(r + 8));
    corners(fx, f, s, true);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let ramp = Fx::ramp_above(age, 16.0, 10.0);
    let a = fix(fx, ee::mul(255.0, ramp), 0x01E0_33E4);
    colour(fx, f, [0x86, 0x68, 0xFF], a as u8);
    words(fx, f.pl, D + 0xA0);
    around(fx, f);
    draw_far(fx, f, f.v0);
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x11 {
        fx.mem.write_u8(r, 0);
    }
}

/// A rise record (D+0x5CB0 + 0x60 (n % 24)) at the target's point: four
/// cells and 1.5 vectors [F b717 0x1E0358C..0x1E0365C].
fn rise_spawn(fx: &mut Fx, n: i32) {
    let k = fx.div(n, 0x18).1 as u32;
    let r = D + 0x60 * k + RISE;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    words(fx, r + 0x10, D + 0x40);
    for j in 0..4u32 {
        let c = rnd(fx, OT, 0x01E0_35F8);
        fx.mem.set8(r + 8 + j, rem2(c, 8));
        fx.random_table_vec(r + 0x20 + 0x10 * j, 1.5);
    }
}

/// The rises: four sprites (0C, 08, 08) at alpha 0 (the sine computed and
/// dropped), 15 (0.5 + age / 48) wide, at the point plus (0.4 + 0.6 age /
/// 24) x vector, y - 15 + 40 age / 24, key 0, 0x19 updates [F b717
/// 0x1E03660..0x1E03A34].
fn rises(fx: &mut Fx, f: &Fr) {
    desc_alpha_first(fx, f.d, 0x26, 5, 0x4_2421, 0, 0x10);
    camera(fx, f);
    for k in 0..0x18u32 {
        let r = D + 0x60 * k + RISE;
        if fx.mem.read_u8(r) != 0 {
            rise(fx, f, r);
        }
    }
}

fn rise(fx: &mut Fx, f: &Fr, r: u32) {
    let x = fx.mem.i32(r + 4) << 15;
    fx.mult(0x2AAA_AAAB, x);
    let q = (fx.hilo().1 >> 2).wrapping_add(((x as u32) >> 31) as i32);
    fx.tsin(q);
    let a = fix(fx, 0.0, 0x01E0_3748);
    colour(fx, f, [0x0C, 0x08, 0x08], a as u8);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let s = ee::mul(15.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 24.0)));
    corners(fx, f, s, true);
    let f4 = ee::from_i32(fx.mem.i32(r + 4));
    let f20 = ee::add(f32::from_bits(0x3ECC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F19_999A), f4), 24.0));
    let f21 = ee::add(-15.0, ee::div(ee::mul(40.0, f4), 24.0));
    for j in 0..4u32 {
        fx.vu_scale4(f.pl, r + 0x20 + 0x10 * j, f20);
        fx.vu_add4(f.pl, f.pl, r + 0x10);
        let y = ee::add(fx.mem.f(f.pl + 4), f21);
        fx.mem.setf(f.pl + 4, y);
        around(fx, f);
        let c = i32::from(fx.mem.read_u8(r + 8 + j));
        cell_uvs(fx, f, c);
        if project(fx, f, f.v0, f.d) {
            fx.submit(f.d, OT, 0);
        }
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x19 {
        fx.mem.write_u8(r, 0);
    }
}
