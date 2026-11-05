//! BMODULE b088 (card 552), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xBD40
//! bytes) [F]: a copy of b359.rs (card 337, C33's
//! b012.rs copy) with this record's layout and constants (the project notes
//! d552_337.txt, 206 hunks). Every update the camera turn with roll, the
//! attacker's nodes 0x19 (clear 1) / 0xE into D+0x20 / 0x40, its point
//! (kind 1) into D+0x50 and the target's screen point into D+0x70. A lens
//! and one blast at the target on 0x71; heads on 0x20 / 0x59 (lives 0x39 /
//! 0x18; head 1 moves towards the target, the others stay at node 0x19)
//! with a ring record (alive only for ring 1), a puff and a 16-sprite smoke
//! every update, all faded in by clamp(n - 0x20, 0, 64) / 64. Waves on even
//! updates in 1..0x78 (widths 1.0), 16 spark records in 1..0x34, a burst at
//! node 0xE on update 3 (32 updates). The stage light 0x80 - 96 clamp(n)
//! clamp(0xC1 - n) / 1024. Returns 1 on 0x67, 6 in 0x72..0xA0, 2 on 0xA1, 8
//! once the count reaches 0xC2.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::LENS_CLAMP;

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The fan's nine screen points (exe rodata, 0x10 apart).
const FAN: u32 = 0x0028_ABE0;
const FRAME: u32 = 0x740;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

type C3 = [u8; 3];

// Stack layout.
const CAM: u32 = 0x440;
const IDM: u32 = 0x480;
const TURN: u32 = 0x4C0;
const C0: u32 = 0x500;
const C1: u32 = 0x510;
const Q0: u32 = 0x540;
const Q1: u32 = 0x550;
const P: u32 = 0x560;
const SD: u32 = 0x6D8;
const W: u32 = 0x73C;

// D layout.
const HEADS: u32 = 0x90;
const PUFFS: u32 = 0x150;
const SMOKES: u32 = 0x1350;
const RING_I: u32 = 0x4C50;
const RINGS: u32 = 0x4C60;
const WAVE_I: u32 = 0x5650;
const WAVE_V0: u32 = 0x5660;
const WAVE_V1: u32 = 0x5990;
const WAVES: u32 = 0x5CC0;
const SPARKS: u32 = 0x68C0;
const BURST: u32 = 0x7BC0;
const DISC: u32 = 0x10;
const B_CELLS: u32 = 0x640;
const B_POS: u32 = 0x6C0;
const B_DIRS: u32 = 0x6D0;
const LENS: u32 = 0x8A90;
const BLAST_I: u32 = 0x8A98;
const BLASTS: u32 = 0x8AA0;

// Return addresses.
const R_FAN: [u32; 12] = [
    0x01E0_0744, 0x01E0_0754, 0x01E0_0798, 0x01E0_07A4, 0x01E0_07E8, 0x01E0_07F4, 0x01E0_08BC, 0x01E0_08CC, 0x01E0_0938, 0x01E0_0944, 0x01E0_09B0,
    0x01E0_09BC,
];
const R_HEAD: u32 = 0x01E0_10B8;
/// The heads' / puffs' / smokes' alpha fixes.
const R_HEADA: u32 = 0x01E0_0EFC;
const R_PUFFA: u32 = 0x01E0_143C;
const R_SMOKEA: u32 = 0x01E0_17FC;
const R_PUFF: u32 = 0x01E0_15A8;
const R_SMOKE: u32 = 0x01E0_1968;
const R_GLOW: u32 = 0x01E0_1BC4;
const R_RING1: u32 = 0x01E0_1F68;
const R_RING2: u32 = 0x01E0_2230;
const R_WAVE: u32 = 0x01E0_26EC;
const R_SPARKA: u32 = 0x01E0_2D50;
const R_SPARK: u32 = 0x01E0_2ED0;
const R_SPARKA2: u32 = 0x01E0_2FF4;
const R_SPARK2: u32 = 0x01E0_318C;
const R_BURST: u32 = 0x01E0_33D8;
const R_BLASTC: u32 = 0x01E0_48CC;
const R_BLASTDA: u32 = 0x01E0_4C34;
const R_BLASTD: u32 = 0x01E0_4CF0;
const RR_RING16: u32 = 0x01E0_0C28;
const RR_RING32: u32 = 0x01E0_0C80;
const RR_PUFF: u32 = 0x01E0_1120;
const RR_SMOKE: u32 = 0x01E0_11B0;
const RR_WAVE: u32 = 0x01E0_2364;
const RR_SPARK: u32 = 0x01E0_2B84;
const RR_BURST: u32 = 0x01E0_3250;
const RR_BLAST1: u32 = 0x01E0_3DA8;
const RR_BLAST3: u32 = 0x01E0_3EC4;
const RP_WAVE: [u32; 4] = [0x01E0_27A0, 0x01E0_281C, 0x01E0_2928, 0x01E0_29A4];
const RP_DISC: [u32; 2] = [0x01E0_3698, 0x01E0_37A0];

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xBC, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0xBD40);
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
    for (node, clear, at) in [(0x19, 1, 0x1C), (0xE, 0, 0x30)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x60);
    let h = fx.mem.i32(D + 4);
    let r = crate::blib::afterimage::unit_radius(fx, h);
    fx.mem.setf(D + 0x80, r);
    fx.mem.write_u32(D + 0x84, 0);
    fx.mem.write_u32(D + 0x88, 0);
    for k in 0..3u32 {
        let hd = D + HEADS + 0x40 * k;
        fx.mem.write_u8(hd, 0);
        fx.mem.write_u32(hd + 0xC, 0);
        fx.mem.write_u32(hd + 0x10, 0);
    }
    for h in 0..3u32 {
        for j in 0..0x10u32 {
            fx.mem.write_u8(puff(h, j), 0);
        }
    }
    for h in 0..3u32 {
        for j in 0..0x10u32 {
            fx.mem.write_u8(smoke(h, j), 0);
        }
    }
    fx.mem.write_u32(D + RING_I, 0);
    for k in 0..3u32 {
        fx.mem.write_u8(D + RINGS + 0x350 * k, 0);
    }
    fx.mem.write_u32(D + WAVE_I, 0);
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + WAVES + 0x60 * k, 0);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + SPARKS + 0x130 * k, 0);
    }
    let bu = D + BURST;
    fx.mem.write_u8(bu, 0);
    for k in 0..0x21u32 {
        let a = (k << 11) as i32;
        let s = fx.tsin(a);
        fx.mem.setf(sp + P, s);
        let c = fx.tcos(a);
        fx.mem.setf(sp + P + 4, c);
        fx.mem.write_u32(sp + P + 8, 0);
        fx.mem.write_u32(sp + P + 0xC, 0x3F80_0000);
        let p = bu + DISC + 0x30 * k;
        fx.vu_scale4(p, sp + P, 16.5);
        fx.vu_scale4(p + 0x10, sp + P, f(0x41D3_3333));
        fx.vu_scale4(p + 0x20, sp + P, 27.0);
    }
    fx.mem.write_u32(D + BLAST_I, 0);
    for k in 0..3u32 {
        fx.mem.write_u8(blast(k), 0);
    }
    fx.mem.write_u8(D + LENS, 0);
    0
}

fn puff(h: u32, j: u32) -> u32 {
    D + PUFFS + 0x600 * h + 0x60 * j
}

fn smoke(h: u32, j: u32) -> u32 {
    D + SMOKES + 0x1300 * h + 0x130 * j
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
    uv(fx, sp + 0x6F0, u0);
    uv(fx, sp + 0x6F2, v0);
    uv(fx, sp + 0x700, u1);
    uv(fx, sp + 0x702, v1);
}

/// The sprite cell `b` (4 x n cells of 32).
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    let m = |x: i32| (x & 0x3FFF) as u32;
    uvs(fx, sp, m(c << 9), m(r << 9), m((32 * c + 32) << 4), m((32 * r + 32) << 4));
}

fn rgba(fx: &mut Fx, at: u32, c: C3, a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn copy8(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..2u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn vec4(fx: &mut Fx, at: u32, x: f32, y: f32, z: f32) {
    fx.mem.setf(at, x);
    fx.mem.setf(at + 4, y);
    fx.mem.setf(at + 8, z);
    fx.mem.write_u32(at + 0xC, 0x3F80_0000);
}

/// The sprite descriptor at sp + 0x6D8 (5 registers 0x42421, prim 0x26 |
/// 0x800).
fn head(fx: &mut Fx, sp: u32) {
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

/// Alpha preset `a`, the texture at D + `t` (filter 1), CLAMP 5.
fn textured(fx: &mut Fx, sp: u32, a: i32, t: u32) {
    alpha(fx, sp, a);
    tex(fx, sp, t);
    clamp5(fx, sp);
}

/// The view-projection (x identity) at sp + 0x440.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + CAM);
    fx.identity(sp + IDM);
    let id = fx.read_m4(sp + IDM);
    fx.mul_into(Some(sp + CAM), &id);
}

/// The view-projection x (the camera turn moved to `p`, scaled by `s`)
/// at sp + 0x440.
fn camera_at(fx: &mut Fx, sp: u32, p: u32, s: Option<(f32, f32)>) {
    fx.camera_vp_to(sp + CAM);
    fx.translate_rows(sp + IDM, sp + TURN, p);
    if let Some((s, z)) = s {
        vec4(fx, sp + P, s, s, z);
        fx.scale_rows(sp + IDM, sp + P);
    }
    let m = fx.read_m4(sp + IDM);
    fx.mul_into(Some(sp + CAM), &m);
}

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1) at sp + 0x500 / 0x5C0,
/// turned by the camera (sp + 0x4C0) when `turn`.
fn corners(fx: &mut Fx, sp: u32, s: f32, turn: bool) {
    let n = ee::neg(s);
    vec4(fx, sp + C0, n, n, 0.0);
    vec4(fx, sp + C1, s, s, 0.0);
    if turn {
        fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
        fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
    }
}

/// k (0.5 + 0.5 x / d).
fn half(k: f32, x: i32, d: f32) -> f32 {
    ee::mul(k, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(x)), d)))
}

/// base + slope x the tan falloff.
fn spread(fx: &mut Fx, age: i32, n: i32, base: u32, slope: u32) -> f32 {
    let t = fx.tan_falloff_p(age, n, 1);
    ee::add(f(base), ee::mul(f(slope), t))
}

#[derive(Clone, Copy)]
enum Key {
    /// At the mean w (0x1A2C70 from the return address).
    Fix(u32),
    /// Far Z, key 0.
    Far,
}

/// The corners about sp + 0x560 (sp + 0x540 / 0x600) projected (0x204F70)
/// through sp + 0x440, submitted.
fn draw(fx: &mut Fx, sp: u32, key: Key) {
    fx.vu_add4(sp + Q0, sp + C0, sp + P);
    fx.vu_add4(sp + Q1, sp + C1, sp + P);
    project_sprite(fx, sp, sp + Q0, key);
}

fn project_sprite(fx: &mut Fx, sp: u32, input: u32, key: Key) {
    let d = sp + SD;
    if !crate::fxlib2::project(fx, input, PROJ_OUT, sp + CAM, d, sp + W) {
        return;
    }
    match key {
        Key::Fix(ra) => {
            let w = fx.mem.f(sp + W);
            let k = fixu(fx, w, ra) as i32;
            fx.submit(d, OT, k);
        }
        Key::Far => {
            crate::lens::far_z(fx, sp + 0x6F8, &[sp + 0x708]);
            fx.submit(d, OT, 0);
        }
    }
}

/// A sprite of the direction `dir` x `k` about `pos`, drawn.
fn sprite(fx: &mut Fx, sp: u32, dir: u32, k: f32, pos: u32, key: Key) {
    fx.vu_scale4(sp + P, dir, k);
    fx.vu_add4(sp + P, sp + P, pos);
    draw(fx, sp, key);
}

/// A quad (descriptor `d`, four vertices) through 0x100BA0, far Z at
/// key 0 into `ot` when drawn.
fn quad_far(fx: &mut Fx, sp: u32, vs: [u32; 4], d: u32, xyz: [u32; 4], ot: u32, ra: u32) -> bool {
    if fx.project_n(&vs, sp + CAM, d, sp + W, ra) != 1 {
        return false;
    }
    crate::lens::far_z(fx, xyz[0], &xyz[1..]);
    fx.submit(d, ot, 0);
    true
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    for (slot, out) in [(0x1C, 0x20), (0x30, 0x40)] {
        let (hh, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + slot));
        fx.node_position(hh, s, D + out);
    }
    let hh = fx.mem.i32(D + 4);
    fx.unit_point(1, hh, D + 0x50);
    camera(fx, sp);
    screen_point(fx, sp, D + 0x60, D + 0x70);

    if fx.mem.i32(D) == 0x71 {
        fx.mem.write_u8(D + LENS, 1);
        fx.mem.write_u32(D + LENS + 4, 0);
    }
    if fx.mem.read_u8(D + LENS) == 1 {
        lens(fx, sp);
    }

    // f21: 0x101050(n - 0x20, 0, 0x40) / 64.
    let c = Fx::clamp_i(fx.mem.i32(D) - 0x20, 0, 0x40);
    let f21 = ee::div(ee::from_i32(c), 64.0);
    let life = match fx.mem.i32(D) {
        0x20 => 0x39,
        0x59 => 0x18,
        _ => -1,
    };
    if life >= 0 {
        launch(fx, sp, life);
    }
    heads(fx, sp, f21);
    puffs(fx, sp, f21);
    smokes(fx, sp, f21);
    rings(fx, sp);

    let n = fx.mem.i32(D);
    if rem2(n, 2) == 0 && n > 0 && n < 0x79 {
        let i = fx.mem.read_u32(D + WAVE_I);
        let r = D + WAVES + 0x60 * i;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        fx.random_table_vec(r + 0x50, 1.0);
        for j in 0..0x10u32 {
            // The draw unused: every width 1.0.
            fx.rand_from(OT, RR_WAVE);
            fx.mem.write_u32(r + 8 + 4 * j, 0x3F80_0000);
        }
        let w = fx.mem.read_u32(r + 8);
        fx.mem.write_u32(r + 0x48, w);
        let i = fx.mem.i32(D + WAVE_I) + 1;
        fx.mem.set32(D + WAVE_I, if i >= 0x20 { 0 } else { i });
    }
    waves(fx, sp);

    let n = fx.mem.i32(D);
    if n > 0 && n < 0x36 {
        let r = D + SPARKS + 0x130 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for j in 0..0x10u32 {
            let b = fx.rand_from(OT, RR_SPARK) >> 23;
            fx.mem.set8(r + 8 + j, rem2(b, 4));
            let p = r + 0x30 + 0x10 * j;
            fx.random_table_vec(p, 3.0);
            fx.vu_add4(p, p, D + 0x20);
        }
    }
    sparks(fx, sp);

    let bu = D + BURST;
    if fx.mem.i32(D) == 3 {
        fx.mem.write_u8(bu, 1);
        fx.mem.write_u32(bu + 4, 0);
        copy4(fx, bu + B_POS, D + 0x40);
        for j in 0..0x80u32 {
            let b = fx.rand_from(OT, RR_BURST) >> 23;
            fx.mem.set8(bu + B_CELLS + j, rem2(b, 4));
            fx.random_table_vec(bu + B_DIRS + 0x10 * j, 0.5);
        }
    }
    if fx.mem.read_u8(bu) == 1 {
        burst(fx, sp, bu);
    }

    let idx = if fx.mem.i32(D) == 0x71 { 0 } else { -1 };
    if idx >= 0 {
        blast_spawn(fx, idx as u32);
    }
    blasts(fx, sp);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0xC1 - n, 0, 0x20);
    let k = Fx::clamp_i(n, 0, 0x20);
    let lt = (s0.wrapping_mul(k.wrapping_mul(-96)) >> 10) + 0x80;
    fx.set_stage_light(lt & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x67 {
        ret = 1;
    } else {
        if n >= 0x72 && n < 0xA1 {
            ret = 6;
        }
        if n == 0xA1 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xC2 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// The screen point of `p` (0x204D80 through sp + 0x440 into sp + 0x730)
/// at `o`: x / y ((x16 >> 4) - 0x700, clamped to 0..512), Z (0x17BC10),
/// 1.0.
fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    let m = fx.read_m4(sp + CAM);
    fx.project_at(p, &m, sp + 0x730, sp + W);
    let x = (i32::from(fx.mem.read_u16(sp + 0x730)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x732)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x730) << 8) >> 40;
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

/// An XYZ2 at `p`: x / y halfwords of pixel (x, y), the far Z and a zero
/// top byte.
fn xyz_far(fx: &mut Fx, p: u32, x: i32, y: i32) {
    fx.mem.write_u16(p, ((x + 0x700) << 4) as u16);
    fx.mem.write_u16(p + 2, ((y + 0x700) << 4) as u16);
    let v = fx.mem.read_u64(p);
    fx.mem.write_u64(p, (v & crate::lens::KEEP) | crate::lens::FAR_Z);
    fx.mem.write_u8(p + 7, 0);
}

/// The lens (48 updates): the frame copied in a packet layer, 8 rings of
/// the fan about the target's screen point [F 0x1E005E0..0x1E00AC4].
fn lens(fx: &mut Fx, sp: u32) {
    let d = sp + 0x660;
    fx.mem.write_u16(d + 2, 0x33);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
    let p = fx.alpha_preset(0);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, LENS_CLAMP);
    // a0..a3 as 0x208400 leaves them.
    let a = [u32::from(fx.mem.read_u16(d + 2)), LENS_CLAMP as u32, d + 0x10, d + 2];
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xEC, layer);
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x6C0, sp + 0x6C8);
    fx.texflush(regs);
    let age = fx.mem.i32(D + LENS + 4);
    let al = (((0x30 - age) << 6) / 48) as u8;
    rgba(fx, sp + 0x670, [0x80; 3], al);
    copy8(fx, sp + 0x688, sp + 0x670);
    copy8(fx, sp + 0x6A0, sp + 0x670);
    let c = D + 0x70;
    for i in 0..8i32 {
        let f20 = ee::add(1.0, ee::div(ee::mul(f(0x3E19_999A), ee::from_i32(8 - i)), 8.0));
        for k in 0..8u32 {
            let s0 = FAN + 0x10 * k;
            let x = fix_at(fx, c, R_FAN[0]);
            let y = fix_at(fx, c + 4, R_FAN[1]);
            uv(fx, sp + 0x678, ((x << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x67A, ((y << 4) & 0x3FFF) as u32);
            let x = fix_at(fx, s0, R_FAN[2]);
            let y = fix_at(fx, s0 + 4, R_FAN[3]);
            uv(fx, sp + 0x690, ((x << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x692, ((y << 4) & 0x3FFF) as u32);
            let x = fix_at(fx, s0 + 0x10, R_FAN[4]);
            let y = fix_at(fx, s0 + 0x14, R_FAN[5]);
            uv(fx, sp + 0x6A8, ((x << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x6AA, ((y << 4) & 0x3FFF) as u32);
            fx.vu_sub4(sp + Q0, s0, c);
            fx.vu_sub4(sp + Q1, s0 + 0x10, c);
            fx.vu_scale4(sp + Q0, sp + Q0, f20);
            fx.vu_scale4(sp + Q1, sp + Q1, f20);
            fx.vu_add4(sp + Q0, sp + Q0, c);
            fx.vu_add4(sp + Q1, sp + Q1, c);
            let x = fix_at(fx, c, R_FAN[6]);
            let y = fix_at(fx, c + 4, R_FAN[7]);
            xyz_far(fx, sp + 0x680, x, y);
            let x = fix_at(fx, sp + Q0, R_FAN[8]);
            let y = fix_at(fx, sp + Q0 + 4, R_FAN[9]);
            xyz_far(fx, sp + 0x698, x, y);
            let x = fix_at(fx, sp + Q1, R_FAN[10]);
            let y = fix_at(fx, sp + Q1 + 4, R_FAN[11]);
            xyz_far(fx, sp + 0x6B0, x, y);
            fx.submit(d, 0, 0);
        }
    }
    let layer = fx.mem.read_u32(sp + 0xEC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + LENS + 4) + 1;
    fx.mem.set32(D + LENS + 4, age);
    if age >= 0x31 {
        fx.mem.write_u8(D + LENS, 0);
    }
}

/// A head (living `life` updates) and its ring [F 0x1E00AD4..0x1E00D44].
fn launch(fx: &mut Fx, sp: u32, life: i32) {
    let idx = fx.mem.read_u32(D + 0x88);
    let hd = D + HEADS + 0x40 * idx;
    fx.mem.write_u8(hd, 1);
    fx.mem.write_u32(hd + 4, 0);
    fx.mem.set32(hd + 8, life);
    // Only ring 1 is set alive (the mult twice when it is).
    let ri = fx.mem.i32(D + RING_I);
    if ri == 1 {
        fx.mult(ri, 0x350);
        fx.mem.write_u8(D + RINGS + 0x350, 1);
    }
    fx.mult(ri, 0x350);
    let rg = D + RINGS + 0x350 * ri as u32;
    fx.mem.write_u32(rg + 4, 0);
    copy4(fx, hd + 0x20, D + 0x20);
    copy4(fx, rg + 0x10, D + 0x20);
    fx.random_table_vec(sp + P, 0.0);
    fx.vu_add4(sp + P, sp + P, D + 0x60);
    let px = fx.mem.f(hd + 0x20);
    fx.mem.setf(sp + P, px);
    fx.vu_sub4(hd + 0x30, sp + P, hd + 0x20);
    for j in 0..0x10u32 {
        let b = fx.rand_from(OT, RR_RING16) >> 23;
        fx.mem.set8(rg + 0x20 + j, rem2(b, 8));
        fx.random_table_vec(rg + 0x30 + 0x10 * j, 1.0);
    }
    for j in 0..0x20u32 {
        let b = fx.rand_from(OT, RR_RING32) >> 23;
        fx.mem.set8(rg + 0x130 + j, rem2(b, 4));
        fx.random_table_vec(rg + 0x150 + 0x10 * j, 1.0);
    }
    let c = fx.mem.i32(D + 0x84) + 1;
    fx.mem.set32(D + 0x84, c);
    let i = fx.mem.i32(D + 0x88) + 1;
    fx.mem.set32(D + 0x88, if i >= 3 { 0 } else { i });
    let i = fx.mem.i32(D + RING_I) + 1;
    fx.mem.set32(D + RING_I, if i >= 3 { 0 } else { i });
}

/// The heads (sprite 14, along their direction by age / life) and what
/// they leave [F 0x1E00D44..0x1E01238].
fn heads(fx: &mut Fx, sp: u32, f21: f32) {
    head(fx, sp);
    textured(fx, sp, 1, 0x10);
    camera(fx, sp);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    corners(fx, sp, 10.0, true);
    let f22 = ee::mul(255.0, f21);
    for k in 0..3u32 {
        let hd = D + HEADS + 0x40 * k;
        if fx.mem.read_u8(hd) == 0 {
            continue;
        }
        let al = fixu(fx, f22, R_HEADA) as u8;
        rgba(fx, sp + 0x6E8, [0x3C, 0x64, 0xFF], al);
        let (age, life) = (fx.mem.i32(hd + 4), fx.mem.i32(hd + 8));
        let f20 = ee::div(ee::from_i32(age), ee::from_i32(life));
        let a = fx.mem.i32(hd + 0xC);
        fx.mem.set32(hd + 0xC, if a + 1 >= 0x10 { 0 } else { a + 1 });
        let b = fx.mem.i32(hd + 0x10);
        fx.mem.set32(hd + 0x10, if b + 1 >= 0x10 { 0 } else { b + 1 });
        let pf = puff(k, a as u32);
        let sm = smoke(k, b as u32);
        if k == 1 {
            fx.vu_scale4(sp + P, hd + 0x30, f20);
            fx.vu_add4(sp + P, sp + P, hd + 0x20);
        } else {
            copy4(fx, sp + P, D + 0x20);
        }
        draw(fx, sp, Key::Fix(R_HEAD));
        fx.mem.write_u8(pf, 1);
        fx.mem.write_u32(pf + 4, 0);
        copy4(fx, pf + 0x10, sp + P);
        for j in 0..4u32 {
            let b = fx.rand_from(OT, RR_PUFF) >> 23;
            fx.mem.set8(pf + 8 + j, rem2(b, 8));
            fx.random_table_vec(pf + 0x20 + 0x10 * j, 0.5);
        }
        fx.mem.write_u8(sm, 1);
        fx.mem.write_u32(sm + 4, 0);
        copy4(fx, sm + 0x20, sp + P);
        for j in 0..0x10u32 {
            let b = fx.rand_from(OT, RR_SMOKE) >> 23;
            fx.mem.set8(sm + 8 + j, b % 3);
            fx.random_table_vec(sm + 0x30 + 0x10 * j, f(0x3F33_3333));
        }
        let age = fx.mem.i32(hd + 4) + 1;
        fx.mem.set32(hd + 4, age);
        if fx.mem.i32(hd + 8) < age {
            fx.mem.write_u8(hd, 0);
        }
    }
}

/// The puffs (16 updates) [F 0x1E01238..0x1E015D0].
fn puffs(fx: &mut Fx, sp: u32, f21: f32) {
    head(fx, sp);
    textured(fx, sp, 1, 0x14);
    camera(fx, sp);
    for h in 0..3u32 {
        for j in 0..0x10u32 {
            let r = puff(h, j);
            if fx.mem.read_u8(r) == 0 {
                continue;
            }
            let age = fx.mem.i32(r + 4);
            corners(fx, sp, half(7.0, 0x10 - age, 16.0), true);
            let age = fx.mem.i32(r + 4);
            let al = fixu(fx, ee::mul(f21, ee::from_i32(((0x10 - age) << 7) >> 4)), R_PUFFA) as u8;
            rgba(fx, sp + 0x6E8, [0x3C, 0x64, 0xFF], al);
            let age = fx.mem.i32(r + 4);
            let f20 = spread(fx, age, 0x10, 0x3ECC_CCCD, 0x3F19_999A);
            for i in 0..4u32 {
                let b = i32::from(fx.mem.read_u8(r + 8 + i));
                cell_uvs(fx, sp, b);
                sprite(fx, sp, r + 0x20 + 0x10 * i, f20, r + 0x10, Key::Fix(R_PUFF));
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x11 {
                fx.mem.write_u8(r, 0);
            }
        }
    }
}

/// The smokes (16 updates, 16 sprites) [F 0x1E015D0..0x1E01978].
fn smokes(fx: &mut Fx, sp: u32, f21: f32) {
    head(fx, sp);
    textured(fx, sp, 1, 0x10);
    camera(fx, sp);
    for h in 0..3u32 {
        for j in 0..0x10u32 {
            let r = smoke(h, j);
            if fx.mem.read_u8(r) == 0 {
                continue;
            }
            let age = fx.mem.i32(r + 4);
            corners(fx, sp, half(2.5, 0x10 - age, 16.0), true);
            let age = fx.mem.i32(r + 4);
            let al = fixu(fx, ee::mul(f21, ee::from_i32((0x10 - age).wrapping_mul(200) >> 4)), R_SMOKEA) as u8;
            rgba(fx, sp + 0x6E8, [0x3C, 0x64, 0xFF], al);
            let age = fx.mem.i32(r + 4);
            let f20 = spread(fx, age, 0x10, 0x3DCC_CCCD, 0x3F66_6666);
            for i in 0..0x10u32 {
                let b = i32::from(fx.mem.read_u8(r + 8 + i));
                cell_uvs(fx, sp, b);
                sprite(fx, sp, r + 0x30 + 0x10 * i, f20, r + 0x20, Key::Fix(R_SMOKE));
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x11 {
                fx.mem.write_u8(r, 0);
            }
        }
    }
}

/// The rings (32 updates): a far glow, 16 + 32 sprites [F
/// 0x1E01978..0x1E02248].
fn rings(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    camera(fx, sp);
    for k in 0..3u32 {
        let r = D + RINGS + 0x350 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        textured(fx, sp, 1, 0x10);
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        corners(fx, sp, 100.0, true);
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, 0x20, 1);
        let al = fixu(fx, ee::mul(255.0, ee::sub(1.0, t)), R_GLOW) as u8;
        rgba(fx, sp + 0x6E8, [0x3C, 0x64, 0xFF], al);
        copy4(fx, sp + P, r + 0x10);
        draw(fx, sp, Key::Far);

        textured(fx, sp, 0, 0x14);
        let age = fx.mem.i32(r + 4);
        corners(fx, sp, half(10.0, age, 32.0), true);
        let age = fx.mem.i32(r + 4);
        rgba(fx, sp + 0x6E8, [0x80, 0x87, 0xFF], (((0x20 - age) << 7) >> 5) as u8);
        let age = fx.mem.i32(r + 4);
        let f21 = fx.tan_falloff_p(age, 0x20, 1);
        for i in 0..0x10u32 {
            let b = i32::from(fx.mem.read_u8(r + 0x20 + i));
            cell_uvs(fx, sp, b);
            sprite(fx, sp, r + 0x30 + 0x10 * i, f21, r + 0x10, Key::Fix(R_RING1));
        }

        textured(fx, sp, 1, 0x10);
        let age = fx.mem.i32(r + 4);
        corners(fx, sp, half(2.0, 0x20 - age, 32.0), true);
        let age = fx.mem.i32(r + 4);
        rgba(fx, sp + 0x6E8, [0x3C, 0x64, 0xFF], ((0x20 - age).wrapping_mul(255) >> 5) as u8);
        let age = fx.mem.i32(r + 4);
        let f21 = fx.tan_falloff_p(age, 0x20, 1);
        for i in 0..0x20u32 {
            let b = i32::from(fx.mem.read_u8(r + 0x130 + i));
            cell_uvs(fx, sp, b);
            sprite(fx, sp, r + 0x150 + 0x10 * i, f21, r + 0x10, Key::Fix(R_RING2));
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The quad descriptor at `d` (12 registers, prim 0x34 | 0x800).
fn quad_head(fx: &mut Fx, d: u32) {
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
}

/// The waves (32 updates, 16 segments) in a packet layer [F
/// 0x1E02378..0x1E02AD4].
fn waves(fx: &mut Fx, sp: u32) {
    let d = sp + 0x5D0;
    quad_head(fx, d);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, LENS_CLAMP);
    let n = fx.mem.i32(D);
    let c = Fx::clamp_i(n, 0, 0x20);
    let f21 = ee::mul(228.0, ee::div(ee::from_i32(c), 32.0));
    fx.mem.write_u8(sp + 0xD0, 0);
    let (v0, v1) = (D + WAVE_V0, D + WAVE_V1);
    let xyz = [sp + 0x5F0, sp + 0x608, sp + 0x620, sp + 0x638];
    for k in 0..0x20u32 {
        let r = D + WAVES + 0x60 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        fx.camera_vp_to(sp + CAM);
        fx.vu_add4(sp + P, r + 0x50, D + 0x50);
        fx.translate_rows(sp + IDM, sp + TURN, sp + P);
        let m = fx.read_m4(sp + IDM);
        fx.mul_into(Some(sp + CAM), &m);
        let f20 = ee::div(ee::from_i32(fx.mem.i32(r + 4)), 32.0);
        for s6 in 0..0x11u32 {
            let a = ((s6 << 16) >> 4) as i32;
            let f22 = fx.tsin(a);
            let f0 = fx.tcos(a);
            let rr = r + 8 + 4 * s6;
            let o = 0x30 * s6;
            let g = |fx: &mut Fx| ee::mul(ee::mul(1.5, fx.mem.f(D + 0x80)), f20);
            let pos = |y: f32| if y < 0.0 { 0.0 } else { y };
            let r0 = g(fx);
            let r0 = pos(ee::sub(ee::sub(r0, 3.5), 3.5));
            let r1 = g(fx);
            let r1 = pos(ee::sub(r1, 3.5));
            let r2 = g(fx);
            for (m, rad) in [(0u32, r0), (0x10, r1), (0x20, r2)] {
                let (px, py) = (ee::mul(rad, f22), ee::mul(rad, f0));
                vec4(fx, v0 + o + m, px, py, 0.0);
                let w = fx.mem.f(rr);
                let qx = ee::mul(w, px);
                let w = fx.mem.f(rr);
                vec4(fx, v1 + o + m, qx, ee::mul(w, py), 0.0);
            }
        }
        if fx.mem.read_u8(sp + 0xD0) == 0 {
            fx.mem.write_u8(sp + 0xD0, 1);
            let layer = fx.layer_begin();
            fx.mem.write_u32(sp + 0xEC, layer);
            // a0 / a1 as the point loop's last luis left them (1.5, 1.0),
            // a2 the translation, a3 as 0x188420 left it (the turn's
            // first word).
            let a3 = fx.mem.read_u32(sp + TURN);
            fx.texflush([0x3FC0_0000, 0x3F80_0000, sp + P, a3]);
            let h = fx.mem.i32(D + 4);
            let (_, regs) = fx.unit_draw(h, 1, sp + 0x648, sp + 0x650);
            fx.texflush(regs);
        }
        let age = fx.mem.i32(r + 4);
        let cc = fx.tcos((age << 14) >> 5);
        let al = fixu(fx, ee::mul(f21, cc), R_WAVE) as u8;
        rgba(fx, sp + 0x5E0, [0x80, 0x8A, 0xFF], al);
        copy8(fx, sp + 0x5F8, sp + 0x5E0);
        rgba(fx, sp + 0x610, [0, 0, 0xC8], 0);
        copy8(fx, sp + 0x628, sp + 0x610);
        for s6 in 0..0x10u32 {
            let (a, b) = (0x30 * s6, 0x30 * (s6 + 1));
            for (half, ras) in [(0u32, [RP_WAVE[0], RP_WAVE[1]]), (0x10, [RP_WAVE[2], RP_WAVE[3]])] {
                let (i0, o0) = (0x10, if half == 0 { 0 } else { 0x20 });
                let vs = [v1 + a + i0, v1 + b + i0, v1 + a + o0, v1 + b + o0];
                if fx.project_n(&vs, sp + CAM, d, sp + W, ras[0]) == 1 {
                    for (uvo, xy) in [(0x5E8u32, 0x5F0u32), (0x600, 0x608), (0x618, 0x620), (0x630, 0x638)] {
                        fx.uv_from_xy(sp + uvo, sp + xy, 0, 0);
                    }
                    let vs = [v0 + a + i0, v0 + b + i0, v0 + a + o0, v0 + b + o0];
                    quad_far(fx, sp, vs, d, xyz, 0, ras[1]);
                }
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
    if fx.mem.read_u8(sp + 0xD0) == 1 {
        let layer = fx.mem.read_u32(sp + 0xEC);
        fx.layer_end(layer, OT, 0);
    }
}

/// The sparks (32 x 0x130, 32 updates): 16 points about node 0x29 drawn
/// in towards it, then 16 larger ones (cells + 4) pushed out [F
/// 0x1E02B98..0x1E031B4].
fn sparks(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    clamp5(fx, sp);
    camera(fx, sp);
    for k in 0..0x10u32 {
        let r = D + SPARKS + 0x130 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        tex(fx, sp, 0x10);
        alpha(fx, sp, 1);
        let age = fx.mem.i32(r + 4);
        let s = ee::mul(3.0, ee::add(f(0x3E4C_CCCD), ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(0x10 - age)), 16.0)));
        corners(fx, sp, s, true);
        let age = fx.mem.i32(r + 4);
        let sn = fx.tsin((age << 14) >> 4);
        let al = fixu(fx, ee::mul(128.0, sn), R_SPARKA) as u8;
        rgba(fx, sp + 0x6E8, [0x50, 0x64, 0xFF], al);
        let age = fx.mem.i32(r + 4);
        let f20 = ee::div(ee::from_i32(0x10 - age), 16.0);
        spark_pass(fx, sp, r, f20, 0, R_SPARK);

        tex(fx, sp, 0x14);
        alpha(fx, sp, 1);
        let age = fx.mem.i32(r + 4);
        let s = ee::mul(10.0, ee::add(f(0x3E4C_CCCD), ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(0x10 - age)), 16.0)));
        corners(fx, sp, s, true);
        let age = fx.mem.i32(r + 4);
        let sn = fx.tsin((age << 14) >> 4);
        let al = fixu(fx, ee::mul(64.0, sn), R_SPARKA2) as u8;
        rgba(fx, sp + 0x6E8, [0x50, 0x64, 0xFF], al);
        let age = fx.mem.i32(r + 4);
        let f20 = ee::div(ee::mul(f(0xBF33_3333), ee::from_i32(0x10 - age)), 16.0);
        spark_pass(fx, sp, r, f20, 4, R_SPARK2);

        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// 16 spark sprites at node 0x29 + (point - node) x `k`, cells + `cell`.
fn spark_pass(fx: &mut Fx, sp: u32, r: u32, k: f32, cell: i32, ra: u32) {
    for j in 0..0x10u32 {
        fx.vu_sub4(sp + P, r + 0x30 + 0x10 * j, D + 0x20);
        fx.vu_scale4(sp + P, sp + P, k);
        fx.vu_add4(sp + P, sp + P, D + 0x20);
        fx.vu_add4(sp + Q0, sp + C0, sp + P);
        fx.vu_add4(sp + Q1, sp + C1, sp + P);
        let b = i32::from(fx.mem.read_u8(r + 8 + j)) + cell;
        cell_uvs(fx, sp, b);
        project_sprite(fx, sp, sp + Q0, Key::Fix(ra));
    }
}

/// The burst (32 updates): a far glow, a 32 x 2 quad disc, 128 far
/// sprites [F 0x1E03270..0x1E03BFC].
fn burst(fx: &mut Fx, sp: u32, bu: u32) {
    head(fx, sp);
    tex(fx, sp, 0x10);
    alpha(fx, sp, 1);
    clamp5(fx, sp);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    camera_at(fx, sp, bu + B_POS, None);
    let age = fx.mem.i32(bu + 4);
    let t = fx.tan_falloff_p(age, 0x20, 1);
    let al = fixu(fx, ee::mul(255.0, ee::sub(1.0, t)), R_BURST) as u8;
    rgba(fx, sp + 0x6E8, [0x80, 0x30, 0xFF], al);
    corners(fx, sp, 48.0, false);
    project_sprite(fx, sp, sp + C0, Key::Far);

    let q = sp + 0x578;
    fx.mem.write_u16(q + 2, 0x14);
    fx.mem.write_u16(q, 8);
    fx.mem.write_u64(q + 8, 0x4141_4141);
    fx.mem.write_u32(q + 4, 0);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(q, p);
    fx.camera_vp_to(sp + CAM);
    fx.translate_rows(sp + IDM, sp + TURN, bu + B_POS);
    let age = fx.mem.i32(bu + 4);
    let f20 = spread(fx, age, 0x20, 0x3DCC_CCCD, 0x3F66_6666);
    vec4(fx, sp + P, f20, f20, 0.0);
    fx.scale_rows(sp + IDM, sp + P);
    let m = fx.read_m4(sp + IDM);
    fx.mul_into(Some(sp + CAM), &m);
    let age = fx.mem.i32(bu + 4);
    rgba(fx, sp + 0x588, [0xE4, 0x94, 0xFF], ((0x20 - age).wrapping_mul(255) >> 5) as u8);
    copy8(fx, sp + 0x598, sp + 0x588);
    rgba(fx, sp + 0x5A8, [0x1C, 0x12, 0xFF], 0);
    copy8(fx, sp + 0x5B8, sp + 0x5A8);
    let xyz = [sp + 0x590, sp + 0x5A0, sp + 0x5B0, sp + 0x5C0];
    for s5 in 0..0x20u32 {
        let (a, b) = (bu + DISC + 0x30 * s5, bu + DISC + 0x30 * (s5 + 1));
        quad_far(fx, sp, [a + 0x10, b + 0x10, a, b], q, xyz, OT, RP_DISC[0]);
        quad_far(fx, sp, [a + 0x10, b + 0x10, a + 0x20, b + 0x20], q, xyz, OT, RP_DISC[1]);
    }

    head(fx, sp);
    tex(fx, sp, 0x10);
    alpha(fx, sp, 1);
    clamp5(fx, sp);
    camera(fx, sp);
    let age = fx.mem.i32(bu + 4);
    rgba(fx, sp + 0x6E8, [0x80, 0x30, 0xFF], ((0x20 - age).wrapping_mul(255) >> 5) as u8);
    corners(fx, sp, f(0x3F4C_CCCD), true);
    for j in 0..0x80u32 {
        let b = i32::from(fx.mem.read_u8(bu + B_CELLS + j));
        cell_uvs(fx, sp, b);
        sprite(fx, sp, bu + B_DIRS + 0x10 * j, f20, bu + B_POS, Key::Far);
    }
    let age = fx.mem.i32(bu + 4) + 1;
    fx.mem.set32(bu + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(bu, 0);
    }
}

fn blast(i: u32) -> u32 {
    D + BLASTS + 0x10E0 * i
}

/// A blast at the target [F 0x1E03C4C..0x1E03FD8]: 64 sprites (halfword
/// cells, 3.7), 128 trail directions (4.75), 64 sprites (cells 5..7, 3.7).
fn blast_spawn(fx: &mut Fx, idx: u32) {
    fx.mem.write_u32(D + BLAST_I, idx);
    let r = blast(fx.mem.read_u32(D + BLAST_I));
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 1);
    fx.mem.write_u32(r + 0x90, 0);
    fx.mem.write_u32(r + 0x94, 0);
    fx.mem.write_u32(r + 0x98, 0);
    fx.mem.write_u32(r + 0x9C, 0x3F80_0000);
    fx.vu_add4(r + 0x90, r + 0x90, D + 0x60);
    for j in 0..0x40u32 {
        let b = fx.rand_from(OT, RR_BLAST1) >> 23;
        fx.mem.write_u16(r + 8 + 2 * j, rem2(b, 8) as u16);
        fx.random_table_vec(r + 0xA0 + 0x10 * j, f(0x406C_CCCD));
    }
    for j in 0..0x80u32 {
        fx.random_table_vec(r + 0x4A0 + 0x10 * j, 4.75);
    }
    for j in 0..0x40u32 {
        let b = fx.rand_from(OT, RR_BLAST3) >> 23;
        fx.mem.set8(r + 0xCA0 + j, b % 3 + 5);
        fx.random_table_vec(r + 0xCE0 + 0x10 * j, f(0x406C_CCCD));
    }
}

/// The blasts (three, 48 updates) [F 0x1E03FD8..0x1E04CD8].
fn blasts(fx: &mut Fx, sp: u32) {
    for k in 0..3u32 {
        let r = blast(k);
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let pos = r + 0x90;
        // 64 sprites, texture 2.
        head(fx, sp);
        tex(fx, sp, 0x18);
        clamp5(fx, sp);
        alpha(fx, sp, 1);
        camera(fx, sp);
        let age = fx.mem.i32(r + 4);
        corners(fx, sp, ee::mul(23.0, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(age)), 48.0))), true);
        let age = fx.mem.i32(r + 4);
        rgba(fx, sp + 0x6E8, [0x56, 0x6C, 0xFF], ((0x30 - age).wrapping_mul(100) / 48) as u8);
        let age = fx.mem.i32(r + 4);
        let f22 = fx.tan_falloff_p(age, 0x30, 1);
        for j in 0..0x40u32 {
            fx.vu_scale4(sp + P, r + 0xCE0 + 0x10 * j, f22);
            fx.vu_add4(sp + P, sp + P, pos);
            fx.vu_add4(sp + Q0, sp + C0, sp + P);
            fx.vu_add4(sp + Q1, sp + C1, sp + P);
            let b = i32::from(fx.mem.read_u8(r + 0xCA0 + j));
            cell_uvs(fx, sp, b);
            project_sprite(fx, sp, sp + Q0, Key::Far);
        }

        // 64 sprites, texture 1, alternating.
        head(fx, sp);
        tex(fx, sp, 0x14);
        clamp5(fx, sp);
        camera(fx, sp);
        let age = fx.mem.i32(r + 4);
        corners(fx, sp, ee::mul(20.0, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(age)), 48.0))), true);
        let age = fx.mem.i32(r + 4);
        let a255 = ((0x30 - age).wrapping_mul(255) / 48) as u8;
        let a128 = (((0x30 - age) << 7) / 48) as u8;
        fx.mem.write_u8(sp + 0x430, a255);
        fx.mem.write_u8(sp + 0xF0, a128);
        for j in 0..0x40u32 {
            let age = fx.mem.i32(r + 4);
            let mut f22 = fx.tan_falloff_p(age, 0x30, 1);
            if j % 2 == 0 {
                f22 = ee::mul(f22, f(0x3F4C_CCCD));
                alpha(fx, sp, 2);
                rgba(fx, sp + 0x6E8, [0xC8, 0xAA, 0x80], a128);
            } else {
                alpha(fx, sp, 1);
                rgba(fx, sp + 0x6E8, [0x3C, 0x64, 0xFF], a255);
            }
            fx.vu_scale4(sp + P, r + 0xA0 + 0x10 * j, f22);
            fx.vu_add4(sp + P, sp + P, pos);
            fx.vu_add4(sp + Q0, sp + C0, sp + P);
            fx.vu_add4(sp + Q1, sp + C1, sp + P);
            let b = i32::from(fx.mem.read_u16(r + 8 + 2 * j));
            cell_uvs(fx, sp, b);
            project_sprite(fx, sp, sp + Q0, Key::Far);
        }

        // A glow 235 at the target.
        head(fx, sp);
        tex(fx, sp, 0x10);
        clamp5(fx, sp);
        alpha(fx, sp, 1);
        fx.camera_vp_to(sp + CAM);
        fx.translate_rows(sp + IDM, sp + TURN, pos);
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, 0x30, 1);
        let s = ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), t));
        vec4(fx, sp + P, s, s, s);
        fx.scale_rows(sp + IDM, sp + P);
        let m = fx.read_m4(sp + IDM);
        fx.mul_into(Some(sp + CAM), &m);
        let age = fx.mem.i32(r + 4);
        let al = fixu(fx, ee::mul(200.0, ee::div(ee::from_i32(0x30 - age), 48.0)), R_BLASTC) as u8;
        rgba(fx, sp + 0x6E8, [0x58, 0x68, 0xFF], al);
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        corners(fx, sp, 235.0, false);
        project_sprite(fx, sp, sp + C0, Key::Far);

        // 128 x 6 trail sprites.
        head(fx, sp);
        tex(fx, sp, 0x10);
        clamp5(fx, sp);
        alpha(fx, sp, 1);
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        camera(fx, sp);
        corners(fx, sp, 1.0, true);
        let age = fx.mem.i32(r + 4);
        let f22 = ee::div(ee::from_i32(0x30 - age), 48.0);
        let age = fx.mem.i32(r + 4);
        let f23 = fx.tan_falloff_p(age, 0x30, 1);
        for g in 0..0x80u32 {
            let dir = r + 0x4A0 + 0x10 * g;
            for s5 in 0..6i32 {
                let a = ((s5 + 6) * 255) / 12;
                let al = fixu(fx, ee::mul(f22, ee::from_i32(a)), R_BLASTDA) as u8;
                rgba(fx, sp + 0x6E8, [0x80, 0xF0, 0xFF], al);
                let k = ee::add(f23, ee::mul(f(0x3BE5_6042), ee::from_i32(s5)));
                sprite(fx, sp, dir, k, pos, Key::Fix(R_BLASTD));
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x31 {
            fx.mem.write_u8(r, 0);
        }
    }
}
