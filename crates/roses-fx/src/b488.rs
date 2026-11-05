//! BMODULE b488 (card 658), hand-ported from the record (linked at
//! 0x1E00000; the one function 0x1E00080 in a 0x660 frame with D in s1 and
//! the OT in s0, data area a0 = 0x1ED0000, 0x3740 bytes) [F] (helper A40,
//! 4 Nov 2026). b739's family (card 502, b739.rs) re-cut: the camera sp +
//! 0x2E0 (model + 0x40, turn + 0x80), corners sp + 0x3A0.., vertices sp +
//! 0x3E0.., the placed point sp + 0x460, the sprite descriptor sp + 0x478,
//! the fan's triangle descriptor sp + 0x4D0, the petals' quad descriptor sp
//! + 0x548, the textured quad sp + 0x5D8, the mean w sp + 0x65C. The init
//! takes the target's position (D+0x30), nodes 0x32 / 0x39 (D+0x60..) and
//! builds a flower of 8 rings x 16 petals (D+0x1190, 5 (7 - i) / 7 apart,
//! -100 side (i + 1) / 8 long, each turned by a random z angle; cells
//! D+0x3190); no disc, no tint. Every update the camera turn with roll (sp
//! + 0x360), the attacker's point D+0x20, the two nodes (D+0x70..) and the
//! window points of the attacker (D+0x40) and the target (D+0x50). On 0x4F
//! a lens at the target for 0x31 updates (D+0x3700, b424's fan) and one
//! burst at the target (D+0xA0: 64 puffs, 64 sparks in two colours, a
//! 130-wide halo and 128 rays of six sprites, 0x31 updates); on 0x38 one
//! flower head at the second node (D+0x3210, 0xB updates) with 64 sparks
//! (D+0x3240, 0x21 updates); on 1..0x27 a red flash record (D+0x36A0 + 8 (n
//! % 12), 0xD updates) each drawing the attacker (0x1F8270) through b424's
//! fan at its window point; on 0x45 a falling textured quad from the first
//! node (D+0x3710, 0x41 updates). The stage light 0x80 - 80 clamp(0x9F -
//! n, 0, 32) clamp(n, 0, 32) / 1024; returns 1 on 0x45, 6 on 0x50..0x7E, 2
//! on 0x7F, 8 once the count reaches 0xA0.
use crate::b182::{alpha, clamp_regs, head, rgba};
use crate::b440::{copy8, copy_z, fix, rem2, vec4, words};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::LENS_CLAMP;

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
const ONE: u32 = 0x3F80_0000;

const BURST_IDX: u32 = 0x90;
const BURST: u32 = 0xA0;
const FLOWER_IDX: u32 = 0x1180;
const PETALS: u32 = 0x1190;
const CELLS: u32 = 0x3190;
const FLOWER: u32 = 0x3210;
const SPARK_IDX: u32 = 0x3230;
const SPARKS: u32 = 0x3240;
const FLASH: u32 = 0x36A0;
const LENS: u32 = 0x3700;
const QUAD: u32 = 0x3710;

const LENS_RA: [u32; 12] = [
    0x01E0_0868, 0x01E0_0874, 0x01E0_08B8, 0x01E0_08C4, 0x01E0_0908, 0x01E0_0914, 0x01E0_09C8, 0x01E0_09D4, 0x01E0_0A40, 0x01E0_0A4C, 0x01E0_0AB8,
    0x01E0_0AC4,
];
const FLASH_RA: [u32; 12] = [
    0x01E0_28D8, 0x01E0_28E4, 0x01E0_2928, 0x01E0_2934, 0x01E0_2978, 0x01E0_2984, 0x01E0_2A38, 0x01E0_2A44, 0x01E0_2AB0, 0x01E0_2ABC, 0x01E0_2B2C,
    0x01E0_2B38,
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
    t: u32,
    q: u32,
    g: u32,
    mean: u32,
}

impl Fr {
    fn new(sp: u32) -> Fr {
        let vp = sp + 0x2E0;
        Fr {
            sp,
            vp,
            model: vp + 0x40,
            turn: vp + 0x80,
            c0: sp + 0x3A0,
            c1: sp + 0x3B0,
            v0: sp + 0x3E0,
            pl: sp + 0x460,
            d: sp + 0x478,
            t: sp + 0x4D0,
            q: sp + 0x548,
            g: sp + 0x5D8,
            mean: sp + 0x65C,
        }
    }
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x660);
    fx.mem.write_u32(sp + 0xB0, 0);
    let f = Fr::new(sp);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, &f) };
    fx.leave(0x660);
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
    fx.bzero(D, 0x3740);
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
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x30);
    for (k, (node, clear)) in [(0x32, 1), (0x39, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0x60 + 4 * k as u32, r);
    }
    fx.mem.write_u32(D + BURST_IDX, 0);
    fx.mem.write_u8(D + BURST, 0);
    fx.mem.write_u32(D + FLOWER_IDX, 0);
    flower_init(fx, sp);
    fx.mem.write_u8(D + FLOWER, 0);
    fx.mem.write_u32(D + SPARK_IDX, 0);
    fx.mem.write_u8(D + SPARKS, 0);
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + FLASH + 8 * k, 0);
    }
    fx.mem.write_u8(D + LENS, 0);
    fx.mem.write_u8(D + QUAD, 0);
    0
}

/// The flower: ring i's petals (-1..1 wide, -100 side (i + 1) / 8 long)
/// turned by a random z angle and moved 5 (7 - i) / 7 out along the
/// petal's angle [F b488 0x1E0019C..0x1E003A4]; the ring pointers spilled
/// to sp + 0x100 / 0x110 / 0x120.
fn flower_init(fx: &mut Fx, sp: u32) {
    let (m, vv) = (sp + 0x420, sp + 0x460);
    for i in 0..8i32 {
        fx.mem.write_u32(sp + 0x100, D + 0x400 * i as u32);
        fx.mem.write_u32(sp + 0x110, D + 0x10 * i as u32);
        fx.mem.write_u32(sp + 0x120, 0x2000 * i as u32);
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let f20 = ee::div(ee::mul(ee::mul(-100.0, side), ee::from_i32(i + 1)), 8.0);
        let f21 = ee::div(ee::mul(5.0, ee::from_i32(7 - i)), 7.0);
        let s7 = (0x2000 * i) >> 3;
        for j in 0..0x10i32 {
            let a = (((j << 16) >> 4).wrapping_add(s7)) & 0xFFFF;
            let sn = fx.tsin(a);
            fx.mem.setf(vv, ee::mul(f21, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(vv + 4, ee::mul(f21, cs));
            fx.mem.write_u32(vv + 8, 0);
            fx.mem.write_u32(vv + 0xC, ONE);
            fx.identity(m);
            let r = rnd(fx, j as u32, 0x01E0_0270);
            fx.rotate_axis(m, (rem2(r, 256) << 16) >> 8, 2);
            let q = D + 0x400 * i as u32 + 0x40 * j as u32 + PETALS;
            let (n, p, z) = (0xBF80_0000u32, ONE, f20.to_bits());
            vec4(fx, q, [n, 0, 0, ONE]);
            vec4(fx, q + 0x10, [p, 0, 0, ONE]);
            vec4(fx, q + 0x20, [n, 0, z, ONE]);
            vec4(fx, q + 0x30, [p, 0, z, ONE]);
            for k in 0..4u32 {
                fx.vu_apply_to(q + 0x10 * k, m, q + 0x10 * k);
            }
            fx.vu_add4(q + 0x20, q + 0x20, vv);
            fx.vu_add4(q + 0x30, q + 0x30, vv);
            let r = rnd(fx, j as u32, 0x01E0_0338);
            fx.mem.set8(D + 0x10 * i as u32 + j as u32 + CELLS, rem2(r, 4) + 4);
        }
    }
    fx.mem.write_u32(sp + 0x100, D + 0x2000);
    fx.mem.write_u32(sp + 0x110, D + 0x80);
    fx.mem.write_u32(sp + 0x120, 0x10000);
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

/// The placed point (= dir x s + at) and the sprite's vertices around it.
fn place(fx: &mut Fx, f: &Fr, dir: u32, s: f32, at: u32) {
    fx.vu_scale4(f.pl, dir, s);
    fx.vu_add4(f.pl, f.pl, at);
    fx.vu_add4(f.v0, f.c0, f.pl);
    fx.vu_add4(f.v0 + 0x10, f.c1, f.pl);
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

/// Projected from `input` and submitted at far Z (key 0).
fn draw_far(fx: &mut Fx, f: &Fr, input: u32) {
    if project(fx, f, input, f.d) {
        let x = fx.mem.read_u64(f.d + 0x20);
        fx.mem.write_u64(f.d + 0x20, (x & KEEP) | FAR_Z);
        copy_z(fx, f.d + 0x30, f.d + 0x20);
        fx.submit(f.d, OT, 0);
    }
}

/// The window point of `p` (0x204D80 through the camera into sp + 0x650)
/// into `o`: x / y ((x16 >> 4) - 0x700) clamped to 0..512, Z, 1.0.
fn screen_point(fx: &mut Fx, f: &Fr, p: u32, o: u32) {
    camera(fx, f);
    let m = fx.read_m4(f.vp);
    let sp = f.sp;
    fx.project_at(p, &m, sp + 0x650, sp + 0x65C);
    let x = (i32::from(fx.mem.read_u16(sp + 0x650)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x652)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x650) << 8) >> 40;
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
    fx.unit_point(1, h, D + 0x20);
    for k in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x60 + 4 * k);
        fx.node_position(h, s, D + 0x70 + 0x10 * k);
    }
    screen_point(fx, f, D + 0x20, D + 0x40);
    screen_point(fx, f, D + 0x30, D + 0x50);
    lens(fx, f);
    if fx.mem.i32(D) == 0x4F {
        burst_spawn(fx);
    }
    bursts(fx, f);
    if fx.mem.i32(D) == 0x38 {
        flower_spawn(fx, sp);
    }
    flowers(fx, f);
    flower_sparks(fx, f);
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x28 {
        let k = fx.div(n, 0xC).1 as u32;
        fx.mem.write_u8(D + FLASH + 8 * k, 1);
        fx.mem.write_u32(D + FLASH + 8 * k + 4, 0);
    }
    flashes(fx, f);
    if fx.mem.i32(D) == 0x45 {
        quad_spawn(fx);
    }
    quads(fx, f);
    let n = fx.mem.i32(D);
    let c1 = Fx::clamp_i(0x9F - n, 0, 0x20);
    let n = fx.mem.i32(D);
    let c2 = Fx::clamp_i(n, 0, 0x20);
    let l = fx.mult(c1, c2.wrapping_mul(-80));
    fx.set_stage_light(((l >> 10) + 0x80) & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xB0);
    if n == 0x45 {
        ret = 1;
    } else {
        if (0x50..0x7F).contains(&n) {
            ret = 6;
        }
        if n == 0x7F {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xA0 {
        ret = 8;
    }
    fx.mem.set32(sp + 0xB0, ret);
    ret
}

/// The lens at the target's window point (D+0x3700 for 0x31 updates) [F
/// b488 0x1E006AC..0x1E00B94].
fn lens(fx: &mut Fx, f: &Fr) {
    if fx.mem.i32(D) == 0x4F {
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
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x530, sp + 0x538);
    fx.texflush(regs);
    let age = fx.mem.i32(D + LENS + 4);
    let al = div48(fx, (0x30 - age) << 6);
    rgba(fx, t + 0x10, [0x80; 3], al as u8);
    copy8(fx, t + 0x28, t + 0x10);
    copy8(fx, t + 0x40, t + 0x10);
    for j in 0..8i32 {
        fx.mem.write_u32(sp + 0x2B0, j as u32);
        let s = ee::add(1.0, ee::div(ee::mul(f32::from_bits(0x3E19_999A), ee::from_i32(8 - j)), 8.0));
        crate::b424::fan(fx, t, D + 0x50, sp + 0x3E0, s, 0, &LENS_RA);
    }
    fx.mem.write_u32(sp + 0x2B0, 8);
    let layer = fx.mem.read_u32(sp + 0xEC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + LENS + 4) + 1;
    fx.mem.set32(D + LENS + 4, age);
    if age >= 0x31 {
        fx.mem.write_u8(D + LENS, 0);
    }
}

/// The burst record (D+0xA0) at the target [F b488 0x1E00BA4..0x1E00E1C]:
/// point +0x130, 64 spark cells (+0xA8, halfwords) and 1.8 vectors
/// (+0x140), 128 ray vectors 2.5 (+0x540), 64 puff cells (+0xD40, rand % 3
/// + 5) and 1.8 vectors (+0xD80).
fn burst_spawn(fx: &mut Fx) {
    let v18 = f32::from_bits(0x3FE6_6666);
    fx.mem.write_u32(D + BURST_IDX, 0);
    let idx = fx.mem.i32(D + BURST_IDX);
    let r = D + 0x10E0 * idx as u32;
    fx.mem.write_u8(r + BURST, 1);
    fx.mem.write_u32(r + 0xA4, 1);
    vec4(fx, r + 0x130, [0, 0, 0, ONE]);
    fx.vu_add4(r + 0x130, r + 0x130, D + 0x30);
    for j in 0..0x40u32 {
        let c = rnd(fx, OT, 0x01E0_0CB8);
        fx.mem.write_u16(r + 0xA8 + 2 * j, rem2(c, 8) as u16);
        fx.random_table_vec(r + 0x140 + 0x10 * j, v18);
    }
    for m in 0..0x80u32 {
        fx.random_table_vec(r + 0x540 + 0x10 * m, 2.5);
    }
    for j in 0..0x40u32 {
        let c = rnd(fx, OT, 0x01E0_0D98);
        let (_, h) = fx.div(c, 3);
        fx.mem.set8(r + 0xD40 + j, h + 5);
        fx.random_table_vec(r + 0xD80 + 0x10 * j, v18);
    }
}

/// `size` x (0.8 + 0.2 age / 48).
fn burst_size(fx: &mut Fx, r: u32, size: f32) -> f32 {
    let age = ee::from_i32(fx.mem.i32(r + 0xA4));
    let s = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), age), 48.0);
    ee::mul(size, ee::add(f32::from_bits(0x3F4C_CCCD), s))
}

/// The burst [F b488 0x1E00E20..0x1E01B48].
fn bursts(fx: &mut Fx, f: &Fr) {
    let sp = f.sp;
    fx.mem.write_u32(sp + 0x2A0, 0);
    let r = D;
    fx.mem.write_u32(sp + 0x160, r);
    if fx.mem.read_u8(r + BURST) != 0 {
        burst(fx, f, r);
    }
    fx.mem.write_u32(sp + 0x160, r + 0x10E0);
    fx.mem.write_u32(sp + 0x2A0, 1);
}

fn burst(fx: &mut Fx, f: &Fr, r: u32) {
    let sp = f.sp;
    let pt = r + 0x130;
    fx.mem.write_u32(sp + 0x220, f.d + 2);
    fx.mem.write_u32(sp + 0x230, f.d + 8);
    fx.mem.write_u32(sp + 0x240, f.d + 4);
    // Puffs.
    desc_reset(fx, f, 0x18, Some(1));
    camera(fx, f);
    let s = burst_size(fx, r, 12.0);
    corners(fx, f, s, true);
    let age = fx.mem.i32(r + 0xA4);
    let a = div48(fx, (0x30 - age) * 200);
    colour(fx, f, [0x6C, 0xFF, 0x38], a as u8);
    let age = fx.mem.i32(r + 0xA4);
    let f22 = fx.tan_falloff_p(age, 0x30, 1);
    fx.mem.write_u32(sp + 0x130, pt);
    for j in 0..0x40u32 {
        place(fx, f, r + 0xD80 + 0x10 * j, f22, pt);
        let c = i32::from(fx.mem.read_u8(r + 0xD40 + j));
        cell_uvs(fx, f, c);
        draw_far(fx, f, f.v0);
    }
    // Sparks in two colours.
    desc_reset(fx, f, 0x10, None);
    camera(fx, f);
    let s = burst_size(fx, r, 10.0);
    corners(fx, f, s, true);
    let age = fx.mem.i32(r + 0xA4);
    let a255 = div48(fx, (0x30 - age) * 255);
    fx.mem.write_u8(sp + 0x2D0, a255 as u8);
    fx.mem.write_u32(sp + 0x140, pt);
    let a128 = div48(fx, (0x30 - age) << 7);
    fx.mem.write_u8(sp + 0xD0, a128 as u8);
    for j in 0..0x40u32 {
        let age = fx.mem.i32(r + 0xA4);
        let mut f22 = fx.tan_falloff_p(age, 0x30, 1);
        if j % 2 == 0 {
            f22 = ee::mul(f22, f32::from_bits(0x3F4C_CCCD));
            alpha(fx, f.d, 2);
            colour(fx, f, [0x80, 0xFF, 0xFF], a128 as u8);
        } else {
            alpha(fx, f.d, 1);
            colour(fx, f, [0x60, 0xFF, 0x1E], a255 as u8);
        }
        place(fx, f, r + 0x140 + 0x10 * j, f22, pt);
        let c = i32::from(fx.mem.read_u16(r + 0xA8 + 2 * j));
        cell_uvs(fx, f, c);
        draw_far(fx, f, f.v0);
    }
    // The halo: the camera at the point scaled by 0.2 + 0.8 falloff.
    desc_reset(fx, f, 0x14, Some(1));
    fx.camera_vp_to(f.vp);
    fx.translate_rows(f.model, f.turn, pt);
    let age = fx.mem.i32(r + 0xA4);
    let tf = fx.tan_falloff_p(age, 0x30, 1);
    let s = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf));
    vec4(fx, f.pl, [s.to_bits(), s.to_bits(), s.to_bits(), ONE]);
    fx.scale_rows(f.model, f.pl);
    let mm = fx.read_m4(f.model);
    fx.mul_into(Some(f.vp), &mm);
    colour(fx, f, [0xFF, 0x5E, 0x3A], 0);
    uvs(fx, f, 0x600, 0, 0x800, 0x200);
    corners(fx, f, 130.0, false);
    draw_far(fx, f, f.c0);
    // The rays: 128 of six sprites, (FF, F0, 80) at alpha 0.
    desc_reset(fx, f, 0x14, Some(1));
    uvs(fx, f, 0x600, 0, 0x800, 0x200);
    camera(fx, f);
    corners(fx, f, 1.0, true);
    let age = fx.mem.i32(r + 0xA4);
    let f22 = fx.tan_falloff_p(age, 0x30, 1);
    for m in 0..0x80u32 {
        fx.mem.write_u32(sp + 0x2C0, m);
        fx.mem.write_u32(sp + 0x150, r + 0x10 * m);
        let dir = r + 0x540 + 0x10 * m;
        for s6 in 0..6i32 {
            colour(fx, f, [0xFF, 0xF0, 0x80], 0);
            let f12 = ee::add(f22, ee::mul(f32::from_bits(0x3C23_D70A), ee::from_i32(s6)));
            place(fx, f, dir, f12, pt);
            draw(fx, f, f.d, f.v0, 0x01E0_1AB0);
        }
    }
    fx.mem.write_u32(sp + 0x2C0, 0x80);
    fx.mem.write_u32(sp + 0x150, r + 0x800);
    let age = fx.mem.i32(r + 0xA4) + 1;
    fx.mem.set32(r + 0xA4, age);
    if age >= 0x31 {
        fx.mem.write_u8(r + BURST, 0);
    }
}

/// On 0x38 the flower head (D+0x3210 + 0x20 k: a random z angle at the
/// second node) and its sparks (D+0x3240 + 0x460 m: 64 cells and 0.5
/// vectors) [F b488 0x1E01B5C..0x1E01CF0].
fn flower_spawn(fx: &mut Fx, sp: u32) {
    let k = fx.mem.i32(D + FLOWER_IDX) as u32;
    let fb = D + 0x20 * k + FLOWER;
    fx.mem.write_u32(sp + 0x290, 0);
    fx.mem.write_u8(fb, 1);
    fx.mem.write_u32(fb + 4, 0);
    fx.mem.write_u32(sp + 0x170, fb - FLOWER);
    let m = fx.mem.i32(D + SPARK_IDX) as u32;
    let tb = D + 0x460 * m + SPARKS;
    fx.mem.write_u32(sp + 0x250, tb - SPARKS);
    fx.mem.write_u8(tb, 1);
    fx.mem.write_u32(tb + 4, 0);
    fx.mem.write_u32(sp + 0x180, tb - SPARKS);
    let r = rnd(fx, OT, 0x01E0_1BB8);
    fx.mem.set32(fb + 8, (rem2(r, 256) << 16) >> 8);
    words(fx, fb + 0x10, D + 0x80);
    words(fx, tb + 0x50, D + 0x80);
    for j in 0..0x40u32 {
        let r = rnd(fx, OT, 0x01E0_1C38);
        fx.mem.set8(tb + 8 + j, rem2(r, 8));
        fx.random_table_vec(tb + 0x60 + 0x10 * j, 0.5);
    }
    fx.mem.write_u32(sp + 0x170, fb - FLOWER + 4);
    fx.mem.write_u32(sp + 0x250, tb - SPARKS + 0x40);
    fx.mem.write_u32(sp + 0x180, tb - SPARKS + 0x400);
    fx.mem.write_u32(sp + 0x290, 1);
    let k = fx.mem.i32(D + FLOWER_IDX) + 1;
    fx.mem.set32(D + FLOWER_IDX, k);
    if k > 0 {
        fx.mem.write_u32(D + FLOWER_IDX, 0);
    }
    let m = fx.mem.i32(D + SPARK_IDX) + 1;
    fx.mem.set32(D + SPARK_IDX, m);
    if m > 0 {
        fx.mem.write_u32(D + SPARK_IDX, 0);
    }
}

/// The flower [F b488 0x1E01CF4..0x1E022D8]: one head of 8 x 16 petals
/// (alpha 0x56 (10 - age) / 10, (40, FF, 30), 0.5 + age / 20 wide), then
/// a 45-wide far sprite (4C, FF, 14) at alpha 255 (1 - falloff) at the
/// node, 0xB updates.
fn flowers(fx: &mut Fx, f: &Fr) {
    let (sp, q) = (f.sp, f.q);
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(q, rh, 1);
    alpha(fx, q, 1);
    fx.desc_clamp(q, 0);
    fx.mem.write_u32(sp + 0x280, 0);
    let fb = D + FLOWER;
    fx.mem.write_u32(sp + 0x1C0, D);
    if fx.mem.read_u8(fb) != 0 {
        flower(fx, f, fb);
    }
    fx.mem.write_u32(sp + 0x1C0, D + 0x20);
    fx.mem.write_u32(sp + 0x280, 1);
}

fn flower(fx: &mut Fx, f: &Fr, fb: u32) {
    let (sp, q) = (f.sp, f.q);
    fx.mem.write_u32(sp + 0xC0, 0);
    let left = 10 - fx.mem.i32(fb + 4);
    let x = fx.mult(left, 0x56);
    fx.mult(0x6666_6667, x);
    let a = (fx.hilo().1 >> 2).wrapping_add(((x as u32) >> 31) as i32);
    rgba(fx, q + 0x10, [0x40, 0xFF, 0x30], a as u8);
    for o in [0x28u32, 0x40, 0x58] {
        copy8(fx, q + o, q + 0x10);
    }
    fx.mem.write_u32(sp + 0x1A0, fb - FLOWER);
    fx.mem.write_u32(sp + 0x1B0, fb - FLOWER);
    let age = ee::from_i32(fx.mem.i32(fb + 4));
    let s = ee::add(0.5, ee::div(ee::mul(0.5, age), 10.0));
    vec4(fx, f.pl, [s.to_bits(), s.to_bits(), s.to_bits(), ONE]);
    fx.camera_vp_to(f.vp);
    fx.identity(f.model);
    fx.translate_rows(f.model, f.model, fb + 0x10);
    fx.rotate_axis(f.model, 0, 0);
    let ang = fx.mem.i32(fb + 8);
    fx.rotate_axis(f.model, ang, 2);
    fx.scale_rows(f.model, f.pl);
    let mm = fx.read_m4(f.model);
    fx.mul_into(Some(f.vp), &mm);
    petals(fx, f);
    fx.mem.write_u32(sp + 0x1A0, fb - FLOWER + 0x10);
    fx.mem.write_u32(sp + 0x1B0, fb - FLOWER + 4);
    fx.mem.write_u32(sp + 0xC0, 1);
    desc_reset(fx, f, 0x14, Some(1));
    camera(fx, f);
    corners(fx, f, 45.0, true);
    let age = fx.mem.i32(fb + 4);
    let tf = fx.tan_falloff_p(age, 0xA, 1);
    let a = fix(fx, ee::mul(255.0, ee::sub(1.0, tf)), 0x01E0_2100);
    colour(fx, f, [0x4C, 0xFF, 0x14], a as u8);
    words(fx, f.pl, fb + 0x10);
    fx.vu_add4(f.v0, f.c0, f.pl);
    fx.vu_add4(f.v0 + 0x10, f.c1, f.pl);
    uvs(fx, f, 0x600, 0, 0x800, 0x200);
    draw_far(fx, f, f.v0);
    let age = fx.mem.i32(fb + 4) + 1;
    fx.mem.set32(fb + 4, age);
    if age >= 0xB {
        fx.mem.write_u8(fb, 0);
    }
}

/// The 8 x 16 petals through the camera matrix (0x205210), keyed by their
/// mean w.
fn petals(fx: &mut Fx, f: &Fr) {
    let (sp, q) = (f.sp, f.q);
    let mm = fx.read_m4(f.vp);
    for i in 0..8u32 {
        fx.mem.write_u32(sp + 0x190, D + 0x10 * i);
        for j in 0..0x10u32 {
            let cb = i32::from(fx.mem.read_u8(D + 0x10 * i + j + CELLS));
            let c = rem2(cb, 4) << 5;
            let r = (cb >> 2) << 5;
            let f4 = ee::from_i32(c);
            let f2 = ee::from_i32(r);
            let f3 = ee::div(ee::add(32.0, f4), 128.0);
            let f4 = ee::div(f4, 128.0);
            let f5 = ee::div(f2, 64.0);
            let f0 = ee::div(ee::add(16.0, f2), 64.0);
            for (o, x) in [(0x18u32, f4), (0x1C, f0), (0x30, f3), (0x34, f0), (0x48, f4), (0x4C, f5), (0x60, f3), (0x64, f5)] {
                fx.mem.setf(q + o, x);
            }
            let petal = D + 0x400 * i + 0x40 * j + PETALS;
            if fx.project_desc(petal, PROJ_OUT, Some(&mm), q, f.mean) {
                let w = fx.mem.f(f.mean);
                let key = fix(fx, w, 0x01E0_1F98);
                fx.submit(q, OT, key as i32);
            }
        }
    }
    fx.mem.write_u32(sp + 0x190, D + 0x80);
}

/// The flower's 64 sparks: 5 x (0.5 + age / 64) wide, grey BC, 0x21
/// updates [F b488 0x1E022DC..0x1E0260C].
fn flower_sparks(fx: &mut Fx, f: &Fr) {
    let sp = f.sp;
    desc_reset(fx, f, 0x10, Some(0));
    camera(fx, f);
    fx.mem.write_u32(sp + 0x270, 0);
    fx.mem.write_u32(sp + 0x200, D);
    let tb = D + SPARKS;
    if fx.mem.read_u8(tb) != 0 {
        let age = ee::from_i32(fx.mem.i32(tb + 4));
        let s = ee::mul(5.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0)));
        corners(fx, f, s, true);
        colour(fx, f, [0xBC; 3], 0);
        let age = fx.mem.i32(tb + 4);
        let f20 = fx.tan_falloff_p(age, 0x20, 1);
        fx.mem.write_u32(sp + 0x1D0, D);
        fx.mem.write_u32(sp + 0x1E0, D);
        fx.mem.write_u32(sp + 0x1F0, D);
        for j in 0..0x40u32 {
            place(fx, f, tb + 0x60 + 0x10 * j, f20, tb + 0x50);
            let c = i32::from(fx.mem.read_u8(tb + 8 + j));
            cell_uvs(fx, f, c);
            draw(fx, f, f.d, f.v0, 0x01E0_2568);
        }
        fx.mem.write_u32(sp + 0x1D0, D + 0x400);
        fx.mem.write_u32(sp + 0x1E0, D + 0x40);
        fx.mem.write_u32(sp + 0x1F0, D + 0x10);
        let age = fx.mem.i32(tb + 4) + 1;
        fx.mem.set32(tb + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(tb, 0);
        }
    }
    fx.mem.write_u32(sp + 0x200, D + 0x460);
    fx.mem.write_u32(sp + 0x270, 1);
}

/// The red flash records (D+0x36A0, 12 x 8): the first live one opens a
/// layer with the attacker drawn (0x1F8270); each draws b424's fan at the
/// attacker's window point, (FF, 0, 0) at alpha f 64 sin((age + 1) / 13
/// half turn), f = clamp(n, 0, 16) clamp(0x28 - n, 0, 16) / 256, 1 + 0.4
/// age / 12 - 0.2 + 1 / 60 wide, 0xD updates [F b488 0x1E0264C..0x1E02C2C].
fn flashes(fx: &mut Fx, f: &Fr) {
    let (sp, t) = (f.sp, f.t);
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0x800);
    alpha(fx, t, 1);
    fx.desc_clamp(t, LENS_CLAMP);
    let n = fx.mem.i32(D);
    let c1 = Fx::clamp_i(n, 0, 0x10);
    let n = fx.mem.i32(D);
    let c2 = Fx::clamp_i(0x28 - n, 0, 0x10);
    let f20 = ee::div(ee::mul(ee::from_i32(c1), ee::from_i32(c2)), 256.0);
    fx.mem.write_u8(sp + 0xF0, 0);
    let mut open = false;
    for k in 0..0xCu32 {
        fx.mem.write_u32(sp + 0x260, k);
        let r = D + FLASH + 8 * k;
        fx.mem.write_u32(sp + 0x210, r - FLASH);
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        if !open {
            open = true;
            fx.mem.write_u8(sp + 0xF0, 1);
            let l = fx.layer_begin();
            fx.mem.write_u32(sp + 0xEC, l);
            // 0x101050 leaves a0 = its answer, a1 = 0, a2 = 0x10; a3 = t + 2
            // from 0x208400.
            fx.texflush([c2 as u32, 0, 0x10, t + 2]);
            let h = fx.mem.i32(D + 4);
            let (_, regs) = fx.unit_draw(h, 1, sp + 0x530, sp + 0x538);
            fx.texflush(regs);
        }
        for (o, ra) in [(0x10u32, 0x01E0_27C8), (0x28, 0x01E0_2834)] {
            let x = (fx.mem.i32(r + 4) + 1) << 15;
            fx.mult(0x4EC4_EC4F, x);
            let q = (fx.hilo().1 >> 2).wrapping_add(((x as u32) >> 31) as i32);
            let sn = fx.tsin(q);
            let a = fix(fx, ee::mul(f20, ee::mul(64.0, sn)), ra);
            rgba(fx, t + o, [0xFF, 0, 0], a as u8);
        }
        copy8(fx, t + 0x40, t + 0x28);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let g = ee::div(ee::mul(f32::from_bits(0x3ECC_CCCD), age), 12.0);
        let g = ee::add(1.0, ee::add(f32::from_bits(0xBE4C_CCCD), g));
        let f21 = ee::add(f32::from_bits(0x3C88_8889), g);
        crate::b424::fan(fx, t, D + 0x40, sp + 0x3E0, f21, 0, &FLASH_RA);
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0xD {
            fx.mem.write_u8(r, 0);
        }
    }
    fx.mem.write_u32(sp + 0x260, 0xC);
    fx.mem.write_u32(sp + 0x210, 0x60);
    if open {
        let l = fx.mem.read_u32(sp + 0xEC);
        fx.layer_end(l, OT, 0);
    }
}

/// On 0x45 the quad record (D+0x3710): from the first node, drifting
/// (10 side, 150, 20 side) over 64 updates [F b488 0x1E02C2C..0x1E02CA8].
fn quad_spawn(fx: &mut Fx) {
    let r = D + QUAD;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    words(fx, r + 0x10, D + 0x70);
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    fx.mem.setf(r + 0x20, ee::mul(10.0, side));
    fx.mem.write_u32(r + 0x24, 0x4316_0000);
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    fx.mem.setf(r + 0x28, ee::mul(20.0, side));
    fx.mem.write_u32(r + 0x2C, ONE);
}

/// The textured quad (sp + 0x5D8: prim 0x24, ST floats, texture D+0x1C) 3
/// wide, turned with the camera by age x 0xD903 about z, at the point
/// moved age / 64 of the drift with y + 10 (0.5 (-0.0109 age^2)); grey at
/// alpha 0 (the 0x100FF0 ramp computed and dropped), 0x41 updates [F b488
/// 0x1E02CAC..0x1E02F5C].
fn quads(fx: &mut Fx, f: &Fr) {
    let (sp, g) = (f.sp, f.g);
    head(fx, g, 0x24, 9, 0x4_2424_2421, 0);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(g, rh, 1);
    fx.desc_clamp(g, 5);
    alpha(fx, g, 0);
    camera(fx, f);
    let r = D + QUAD;
    if fx.mem.read_u8(r) == 0 {
        return;
    }
    let a = fix(fx, 0.0, 0x01E0_2D68);
    rgba(fx, g + 0x10, [0x80; 3], a as u8);
    let (z, o) = (0u32, ONE);
    for (k, (s, t)) in [(z, z), (o, z), (z, o), (o, o)].into_iter().enumerate() {
        fx.mem.write_u32(g + 0x18 + 0x10 * k as u32, s);
        fx.mem.write_u32(g + 0x1C + 0x10 * k as u32, t);
    }
    let (n, p) = (0xBFC0_0000u32, 0x3FC0_0000u32);
    let c = f.c0;
    vec4(fx, c, [n, n, 0, ONE]);
    vec4(fx, c + 0x10, [p, n, 0, ONE]);
    vec4(fx, c + 0x20, [n, p, 0, ONE]);
    vec4(fx, c + 0x30, [p, p, 0, ONE]);
    let m = sp + 0x420;
    for k in 0..0x10u32 {
        let w = fx.mem.read_u32(f.turn + 4 * k);
        fx.mem.write_u32(m + 4 * k, w);
    }
    let age = fx.mem.i32(r + 4);
    let ang = fx.mult(age, 0xD903);
    fx.rotate_axis(m, ang, 2);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    fx.vu_scale4(f.pl, r + 0x20, ee::div(age, 64.0));
    fx.vu_add4(f.pl, f.pl, r + 0x10);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let p2 = Fx::powf2(age);
    let dy = ee::mul(10.0, ee::mul(0.5, ee::mul(f32::from_bits(0xBC32_864D), p2)));
    let y = ee::add(fx.mem.f(f.pl + 4), dy);
    fx.mem.setf(f.pl + 4, y);
    for k in 0..4u32 {
        fx.vu_apply_to(c + 0x10 * k, m, c + 0x10 * k);
        fx.vu_add4(f.v0 + 0x10 * k, c + 0x10 * k, f.pl);
    }
    draw(fx, f, g, f.v0, 0x01E0_2F1C);
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x41 {
        fx.mem.write_u8(r, 0);
    }
}
