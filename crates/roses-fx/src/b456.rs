//! BMODULE b456 (card 637), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0x9450
//! bytes) [F]: b093.rs (card 18) at this record's
//! frame and constants through the project notes: a 0x9B0 frame (D at sp
//! + 0xBC, the result at sp + 0xC0, s3 = the ordering table), sp + 0x4F0..
//! 0x5C0 + 0xF0, P + 0x150, from 0x628 up + 0x1C0; D fields - 0x10 from
//! 0xB0 up, 4 textures D+0x10..0x1C, nodes 0xC (clear 1) / 0xE into D+0x30 /
//! 0x50, the point D+0x60, the target's position D+0x70 and screen point
//! D+0x80. Lens on 0xB3 / 0xD1 / 0xEF (32 updates). Heads on 0x57 / 0x9A /
//! 0xD9 (life 0x5C / 0x37 / 0x16), 5 wide (FF, 80, 80) at 0x80, a cell a
//! head, bowed 150 sin(age 0x8000 / life) upwards; each with its ring
//! record, whose 32 sprites are here tiles (prim 0x24, sp + 0x770) falling
//! 5 (0.5 (-0.0109 age^2)), turned by k << 11. Waves in 1..0x10F, sparks
//! (16 x 0x130, 16 updates, rising 5 age / 16) in 1..0xEF, a burst at node
//! 0xE on 0x26, three blasts at the target on the lens updates (32
//! updates; the trails are here 128 tiles falling 7 (...)). No drops. The
//! stage light 0x80 - 96 clamp(n) clamp(0x12F - n) / 1024. Returns 1 on
//! 0xA9, 6 in 0xB4..0x10E, 2 on 0x10F, 8 once the count reaches 0x130.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::LENS_CLAMP;

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The fan's nine screen points (exe rodata, 0x10 apart).
const FAN: u32 = 0x0028_ABE0;
const FRAME: u32 = 0x9B0;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

type C3 = [u8; 3];

// Stack layout.
const CAM: u32 = 0x5E0;
const IDM: u32 = 0x620;
const TURN: u32 = 0x660;
const C0: u32 = 0x6A0;
const C1: u32 = 0x6B0;
const Q0: u32 = 0x6E0;
const Q1: u32 = 0x6F0;
const P: u32 = 0x760;
const SD: u32 = 0x948;
const W: u32 = 0x9AC;

// D layout.
const HEADS: u32 = 0xA0;
const PUFFS: u32 = 0x160;
const SMOKES: u32 = 0xA60;
const RING_I: u32 = 0x2E60;
const RINGS: u32 = 0x2E70;
const WAVE_I: u32 = 0x3860;
const WAVE_V0: u32 = 0x3870;
const WAVE_V1: u32 = 0x3A20;
const WAVES: u32 = 0x3BD0;
const SPARKS: u32 = 0x3FD0;
const BURST: u32 = 0x52D0;
const DISC: u32 = 0x10;
const B_CELLS: u32 = 0x640;
const B_POS: u32 = 0x6C0;
const B_DIRS: u32 = 0x6D0;
const LENS: u32 = 0x61A0;
const BLAST_I: u32 = 0x61A8;
const BLASTS: u32 = 0x61B0;

// Return addresses.
const R_FAN: [u32; 12] = [
    0x01E0_072C, 0x01E0_073C, 0x01E0_0780, 0x01E0_078C, 0x01E0_07D0, 0x01E0_07DC, 0x01E0_08A4, 0x01E0_08B4, 0x01E0_0920, 0x01E0_092C, 0x01E0_0998,
    0x01E0_09A4,
];
const R_HEAD: u32 = 0x01E0_0FCC;
const R_PUFF: u32 = 0x01E0_14C0;
const R_SMOKE: u32 = 0x01E0_1868;
const R_GLOW: u32 = 0x01E0_1B50;
const R_RING1: u32 = 0x01E0_1EE0;
const R_RING2: u32 = 0x01E0_228C;
const R_RING2A: u32 = 0x01E0_1FFC;
const R_TRAILA: u32 = 0x01E0_4BA4;
const R_WAVE: u32 = 0x01E0_27BC;
const R_SPARK1A: u32 = 0x01E0_2E38;
const R_SPARK1: u32 = 0x01E0_3000;
const R_SPARK2A: u32 = 0x01E0_3110;
const R_SPARK2: u32 = 0x01E0_32D4;
const R_BURST: u32 = 0x01E0_351C;
const R_BLASTC: u32 = 0x01E0_4900;
const R_BLASTD: u32 = 0x01E0_4E4C;
const RR_RING16: u32 = 0x01E0_0BB8;
const RR_RING32: u32 = 0x01E0_0C10;
const RR_PUFF: u32 = 0x01E0_1038;
const RR_SMOKE: u32 = 0x01E0_10C8;
const RR_WAVE: u32 = 0x01E0_23D4;
const RR_SPARK: u32 = 0x01E0_2C7C;
const RR_BURST: u32 = 0x01E0_33A4;
const RR_BLAST1: u32 = 0x01E0_3EAC;
const RR_BLAST3: u32 = 0x01E0_3FA0;
const RP_WAVE: [u32; 4] = [0x01E0_2874, 0x01E0_28F0, 0x01E0_29F8, 0x01E0_2A74];
const RP_DISC: [u32; 2] = [0x01E0_37DC, 0x01E0_38E8];

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xBC, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0x9450);
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
    for (node, clear, at) in [(0xC, 1, 0x20), (0xE, 0, 0x40)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x70);
    let h = fx.mem.i32(D + 4);
    let r = crate::blib::afterimage::unit_radius(fx, h);
    fx.mem.setf(D + 0x90, r);
    fx.mem.write_u32(D + 0x94, 0);
    fx.mem.write_u32(D + 0x98, 0);
    for k in 0..3u32 {
        let hd = D + HEADS + 0x40 * k;
        fx.mem.write_u8(hd, 0);
        fx.mem.write_u32(hd + 0xC, 0);
        fx.mem.write_u32(hd + 0x10, 0);
    }
    for h in 0..3u32 {
        for j in 0..8u32 {
            fx.mem.write_u8(puff(h, j), 0);
        }
    }
    for h in 0..3u32 {
        for j in 0..0x20u32 {
            fx.mem.write_u8(smoke(h, j), 0);
        }
    }
    fx.mem.write_u32(D + RING_I, 0);
    for k in 0..3u32 {
        fx.mem.write_u8(D + RINGS + 0x350 * k, 0);
    }
    fx.mem.write_u32(D + WAVE_I, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + WAVES + 0x40 * k, 0);
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
        fx.mem.write_u8(D + BLASTS + 0x10E0 * k, 0);
    }
    fx.mem.write_u8(D + LENS, 0);
    0
}

fn puff(h: u32, j: u32) -> u32 {
    D + PUFFS + 0x300 * h + 0x60 * j
}

fn smoke(h: u32, j: u32) -> u32 {
    D + SMOKES + 0xC00 * h + 0x60 * j
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
    uv(fx, sp + 0x960, u0);
    uv(fx, sp + 0x962, v0);
    uv(fx, sp + 0x970, u1);
    uv(fx, sp + 0x972, v1);
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

/// The sprite descriptor at sp + 0x948 (5 registers 0x42421, prim 0x26 |
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

/// The view-projection (x identity) at sp + 0x5E0.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + CAM);
    fx.identity(sp + IDM);
    let id = fx.read_m4(sp + IDM);
    fx.mul_into(Some(sp + CAM), &id);
}

/// The view-projection x (the camera turn moved to `p`, scaled by `s`)
/// at sp + 0x5E0.
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

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1) at sp + 0x6A0 / 0x6B0,
/// turned by the camera (sp + 0x660) when `turn`.
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

/// The corners about sp + 0x760 (sp + 0x6E0 / 0x6F0) projected (0x204F70)
/// through sp + 0x5E0, submitted.
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
            crate::lens::far_z(fx, sp + 0x968, &[sp + 0x978]);
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
    for (slot, out) in [(0x20, 0x30), (0x40, 0x50)] {
        let (hh, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + slot));
        fx.node_position(hh, s, D + out);
    }
    let hh = fx.mem.i32(D + 4);
    fx.unit_point(1, hh, D + 0x60);
    camera(fx, sp);
    screen_point(fx, sp, D + 0x70, D + 0x80);

    let n = fx.mem.i32(D);
    if n == 0xB3 || n == 0xD1 || n == 0xEF {
        fx.mem.write_u8(D + LENS, 1);
        fx.mem.write_u32(D + LENS + 4, 0);
    }
    if fx.mem.read_u8(D + LENS) == 1 {
        lens(fx, sp);
    }

    let n = fx.mem.i32(D);
    match n {
        0x57 => launch(fx, sp, 0, 0x5C),
        0x9A => launch(fx, sp, 1, 0x37),
        0xD9 => launch(fx, sp, 2, 0x16),
        _ => {}
    }
    heads(fx, sp);
    puffs(fx, sp);
    smokes(fx, sp);
    rings(fx, sp);

    let n = fx.mem.i32(D);
    if n > 0 && n < 0x110 {
        let i = fx.mem.read_u32(D + WAVE_I);
        let r = D + WAVES + 0x40 * i;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        fx.random_table_vec(r + 0x30, f(0x3ECC_CCCD));
        for j in 0..8u32 {
            let b = fx.rand_from(OT, RR_WAVE) >> 23;
            let t = ee::div(ee::mul(f(0x3ECC_CCCD), ee::from_i32(rem2(b, 0x100))), 256.0);
            fx.mem.setf(r + 8 + 4 * j, ee::add(f(0x3E4C_CCCD), ee::sub(1.0, t)));
        }
        let w = fx.mem.read_u32(r + 8);
        fx.mem.write_u32(r + 0x28, w);
        let i = fx.mem.i32(D + WAVE_I) + 1;
        fx.mem.set32(D + WAVE_I, if i >= 0x10 { 0 } else { i });
    }
    waves(fx, sp);

    let n = fx.mem.i32(D);
    if n > 0 && n < 0xF0 {
        let r = D + SPARKS + 0x130 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        copy4(fx, r + 0x20, D + 0x30);
        for j in 0..0x10u32 {
            let b = fx.rand_from(OT, RR_SPARK) >> 23;
            fx.mem.set8(r + 8 + j, rem2(b, 4));
            fx.random_table_vec(r + 0x30 + 0x10 * j, f(0x3F33_3333));
        }
    }
    sparks(fx, sp);

    let bu = D + BURST;
    if fx.mem.i32(D) == 0x26 {
        fx.mem.write_u8(bu, 1);
        fx.mem.write_u32(bu + 4, 0);
        copy4(fx, bu + B_POS, D + 0x50);
        for j in 0..0x80u32 {
            let b = fx.rand_from(OT, RR_BURST) >> 23;
            fx.mem.set8(bu + B_CELLS + j, rem2(b, 4));
            fx.random_table_vec(bu + B_DIRS + 0x10 * j, 0.5);
        }
    }
    if fx.mem.read_u8(bu) == 1 {
        burst(fx, sp, bu);
    }

    let n = fx.mem.i32(D);
    let idx = match n {
        0xEF => 2,
        0xD1 => 1,
        0xB3 => 0,
        _ => -1,
    };
    if idx >= 0 {
        blast_spawn(fx, idx as u32);
    }
    blasts(fx, sp);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x12F - n, 0, 0x20);
    let k = Fx::clamp_i(n, 0, 0x20);
    let lt = (s0.wrapping_mul(k.wrapping_mul(-96)) >> 10) + 0x80;
    fx.set_stage_light(lt & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0xA9 {
        ret = 1;
    } else {
        if n >= 0xB4 && n < 0x10F {
            ret = 6;
        }
        if n == 0x10F {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x130 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// The screen point of `p` (0x204D80 through sp + 0x5E0 into sp + 0x9A0)
/// at `o`: x / y ((x16 >> 4) - 0x700, clamped to 0..512), Z (0x17BC10),
/// 1.0.
fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    let m = fx.read_m4(sp + CAM);
    fx.project_at(p, &m, sp + 0x9A0, sp + W);
    let x = (i32::from(fx.mem.read_u16(sp + 0x9A0)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x9A2)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x9A0) << 8) >> 40;
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
    let d = sp + 0x8D0;
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
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x930, sp + 0x938);
    fx.texflush(regs);
    let age = fx.mem.i32(D + LENS + 4);
    let al = (((0x20 - age) << 6) >> 5) as u8;
    rgba(fx, sp + 0x8E0, [0x80; 3], al);
    copy8(fx, sp + 0x8F8, sp + 0x8E0);
    copy8(fx, sp + 0x910, sp + 0x8E0);
    let c = D + 0x80;
    for i in 0..8i32 {
        let f20 = ee::add(1.0, ee::div(ee::mul(f(0x3E19_999A), ee::from_i32(8 - i)), 8.0));
        for k in 0..8u32 {
            let s0 = FAN + 0x10 * k;
            let x = fix_at(fx, c, R_FAN[0]);
            let y = fix_at(fx, c + 4, R_FAN[1]);
            uv(fx, sp + 0x8E8, ((x << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x8EA, ((y << 4) & 0x3FFF) as u32);
            let x = fix_at(fx, s0, R_FAN[2]);
            let y = fix_at(fx, s0 + 4, R_FAN[3]);
            uv(fx, sp + 0x900, ((x << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x902, ((y << 4) & 0x3FFF) as u32);
            let x = fix_at(fx, s0 + 0x10, R_FAN[4]);
            let y = fix_at(fx, s0 + 0x14, R_FAN[5]);
            uv(fx, sp + 0x918, ((x << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x91A, ((y << 4) & 0x3FFF) as u32);
            fx.vu_sub4(sp + Q0, s0, c);
            fx.vu_sub4(sp + Q1, s0 + 0x10, c);
            fx.vu_scale4(sp + Q0, sp + Q0, f20);
            fx.vu_scale4(sp + Q1, sp + Q1, f20);
            fx.vu_add4(sp + Q0, sp + Q0, c);
            fx.vu_add4(sp + Q1, sp + Q1, c);
            let x = fix_at(fx, c, R_FAN[6]);
            let y = fix_at(fx, c + 4, R_FAN[7]);
            xyz_far(fx, sp + 0x8F0, x, y);
            let x = fix_at(fx, sp + Q0, R_FAN[8]);
            let y = fix_at(fx, sp + Q0 + 4, R_FAN[9]);
            xyz_far(fx, sp + 0x908, x, y);
            let x = fix_at(fx, sp + Q1, R_FAN[10]);
            let y = fix_at(fx, sp + Q1 + 4, R_FAN[11]);
            xyz_far(fx, sp + 0x920, x, y);
            fx.submit(d, 0, 0);
        }
    }
    let layer = fx.mem.read_u32(sp + 0xEC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + LENS + 4) + 1;
    fx.mem.set32(D + LENS + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + LENS, 0);
    }
}

/// Head `idx` of life `life` and its ring record [F 0x1E00A74..0x1E00C80].
fn launch(fx: &mut Fx, sp: u32, idx: u32, life: u32) {
    let hd = D + HEADS + 0x40 * idx;
    fx.mem.write_u32(hd + 8, life);
    fx.mem.write_u8(hd, 1);
    fx.mem.write_u32(hd + 4, 0);
    let rg = D + RINGS + 0x350 * idx;
    fx.mem.write_u8(rg, 1);
    fx.mem.write_u32(rg + 4, 0);
    copy4(fx, hd + 0x20, D + 0x30);
    copy4(fx, rg + 0x10, D + 0x30);
    fx.random_table_vec(sp + P, 0.0);
    fx.vu_add4(sp + P, sp + P, D + 0x70);
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
        fx.random_table_vec(rg + 0x150 + 0x10 * j, 1.5);
    }
}

/// The heads (sprite 7, along their direction by the in / out ramp) and
/// what they leave [F 0x1E00D20..0x1E012A0].
fn heads(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    textured(fx, sp, 0, 0x1C);
    camera(fx, sp);
    corners(fx, sp, 5.0, true);
    for k in 0..3u32 {
        let hd = D + HEADS + 0x40 * k;
        if fx.mem.read_u8(hd) == 0 {
            continue;
        }
        uvs(fx, sp, (0x40 * k) << 4, 0, (0x40 * (k + 1)) << 4, 0x400);
        rgba(fx, sp + 0x958, [0xFF, 0x80, 0x80], 0x80);
        let (age, life) = (fx.mem.i32(hd + 4), fx.mem.i32(hd + 8));
        let (q, _) = fx.div(age << 15, life);
        let f20 = ee::div(ee::from_i32(age), ee::from_i32(life));
        let f21 = ee::mul(150.0, fx.tsin(q));
        let a = fx.mem.i32(hd + 0xC);
        fx.mem.set32(hd + 0xC, if a + 1 >= 8 { 0 } else { a + 1 });
        let b = fx.mem.i32(hd + 0x10);
        fx.mem.set32(hd + 0x10, if b + 1 >= 0x20 { 0 } else { b + 1 });
        let pf = puff(k, a as u32);
        let sm = smoke(k, b as u32);
        fx.vu_scale4(sp + P, hd + 0x30, f20);
        fx.vu_add4(sp + P, sp + P, hd + 0x20);
        let y = fx.mem.f(sp + P + 4);
        fx.mem.setf(sp + P + 4, ee::add(y, f21));
        draw(fx, sp, Key::Fix(R_HEAD));
        fx.mem.write_u8(pf, 1);
        fx.mem.write_u32(pf + 4, 0);
        copy4(fx, pf + 0x10, sp + P);
        for j in 0..4u32 {
            let b = fx.rand_from(OT, RR_PUFF) >> 23;
            fx.mem.set8(pf + 8 + j, rem2(b, 8));
            fx.random_table_vec(pf + 0x20 + 0x10 * j, f(0x3EB3_3333));
        }
        fx.mem.write_u8(sm, 1);
        fx.mem.write_u32(sm + 4, 0);
        copy4(fx, sm + 0x10, sp + P);
        for j in 0..4u32 {
            let b = fx.rand_from(OT, RR_SMOKE) >> 23;
            fx.mem.set8(sm + 8 + j, b % 3);
            fx.random_table_vec(sm + 0x20 + 0x10 * j, f(0x3F33_3333));
        }
        let age = fx.mem.i32(hd + 4) + 1;
        fx.mem.set32(hd + 4, age);
        if fx.mem.i32(hd + 8) < age {
            fx.mem.write_u8(hd, 0);
        }
    }
}

/// The puffs (8 updates) [F 0x1E012A0..0x1E01638].
fn puffs(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    textured(fx, sp, 1, 0x14);
    camera(fx, sp);
    for h in 0..3u32 {
        for j in 0..8u32 {
            let r = puff(h, j);
            if fx.mem.read_u8(r) == 0 {
                continue;
            }
            let age = fx.mem.i32(r + 4);
            corners(fx, sp, half(7.5, 8 - age, 8.0), true);
            let age = fx.mem.i32(r + 4);
            rgba(fx, sp + 0x958, [0xFF, 0x40, 0x16], (((8 - age) << 7) >> 3) as u8);
            let age = fx.mem.i32(r + 4);
            let f20 = spread(fx, age, 8, 0x3ECC_CCCD, 0x3F19_999A);
            for i in 0..4u32 {
                let b = i32::from(fx.mem.read_u8(r + 8 + i));
                cell_uvs(fx, sp, b);
                sprite(fx, sp, r + 0x20 + 0x10 * i, f20, r + 0x10, Key::Fix(R_PUFF));
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 9 {
                fx.mem.write_u8(r, 0);
            }
        }
    }
}

/// The smokes (32 updates) [F 0x1E01638..0x1E019E0].
fn smokes(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    textured(fx, sp, 1, 0x10);
    camera(fx, sp);
    for h in 0..3u32 {
        for j in 0..0x20u32 {
            let r = smoke(h, j);
            if fx.mem.read_u8(r) == 0 {
                continue;
            }
            let age = fx.mem.i32(r + 4);
            corners(fx, sp, half(2.5, 0x20 - age, 32.0), true);
            let age = fx.mem.i32(r + 4);
            rgba(fx, sp + 0x958, [0xFF, 0x40, 0x16], ((0x20 - age).wrapping_mul(200) >> 5) as u8);
            let age = fx.mem.i32(r + 4);
            let f20 = spread(fx, age, 0x20, 0x3E4C_CCCD, 0x3F4C_CCCD);
            for i in 0..4u32 {
                let b = i32::from(fx.mem.read_u8(r + 8 + i));
                cell_uvs(fx, sp, b);
                sprite(fx, sp, r + 0x20 + 0x10 * i, f20, r + 0x10, Key::Fix(R_SMOKE));
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x21 {
                fx.mem.write_u8(r, 0);
            }
        }
    }
}

/// The rings (32 updates): a far glow, 16 + 32 sprites [F
/// 0x1E019E0..0x1E022A8].
fn rings(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    tile_head(fx, sp);
    camera(fx, sp);
    for k in 0..3u32 {
        let r = D + RINGS + 0x350 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        textured(fx, sp, 1, 0x10);
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        corners(fx, sp, 35.0, true);
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, 0x20, 1);
        let al = fixu(fx, ee::mul(255.0, ee::sub(1.0, t)), R_GLOW) as u8;
        rgba(fx, sp + 0x958, [0xFF, 0x64, 0x32], al);
        copy4(fx, sp + P, r + 0x10);
        draw(fx, sp, Key::Far);

        textured(fx, sp, 0, 0x14);
        let age = fx.mem.i32(r + 4);
        corners(fx, sp, half(10.0, age, 32.0), true);
        let age = fx.mem.i32(r + 4);
        rgba(fx, sp + 0x958, [0x80; 3], (((0x20 - age) << 7) >> 5) as u8);
        let age = fx.mem.i32(r + 4);
        let f21 = fx.tan_falloff_p(age, 0x20, 1);
        for i in 0..0x10u32 {
            let b = i32::from(fx.mem.read_u8(r + 0x20 + i));
            cell_uvs(fx, sp, b);
            sprite(fx, sp, r + 0x30 + 0x10 * i, f21, r + 0x10, Key::Fix(R_RING1));
        }

        tile_desc(fx, sp, false);
        tile_verts(fx, sp, 0xBF00_0000, 0x3F00_0000);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let rp = Fx::ramp_above(age, 32.0, 21.0);
        let al = fixu(fx, ee::mul(128.0, rp), R_RING2A) as u8;
        rgba(fx, sp + 0x780, [0xFF, 0x80, 0x80], al);
        let age = fx.mem.i32(r + 4);
        let f23 = fx.tan_falloff_p(age, 0x20, 1);
        let f22 = rise(fx, r, 5.0);
        for i in 0..0x20u32 {
            let b = i32::from(fx.mem.read_u8(r + 0x130 + i));
            let c = rem2(b, 4);
            let m = |x: i32| (x & 0x3FFF) as u32;
            let (u0, u1) = (m(c << 10), m(((c << 6) + 0x40) << 4));
            tile_uvs(fx, sp, u0, u1);
            fx.vu_scale4(sp + P, r + 0x150 + 0x10 * i, f23);
            fx.vu_add4(sp + P, sp + P, r + 0x10);
            let y = fx.mem.f(sp + P + 4);
            fx.mem.setf(sp + P + 4, ee::add(y, f22));
            tile(fx, sp, ((i << 16) as i32) >> 5, R_RING2);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The tile descriptor at sp + 0x770 (9 registers, prim 0x24 | 0x800).
fn tile_head(fx: &mut Fx, sp: u32) {
    let d = sp + 0x770;
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
}

/// The tile's ALPHA preset 0, texture D+0x1C (filter 1) and CLAMP 5,
/// the texture first when `tex_first`.
fn tile_desc(fx: &mut Fx, sp: u32, tex_first: bool) {
    let d = sp + 0x770;
    let rh = fx.mem.read_u32(D + 0x1C);
    if tex_first {
        fx.desc_texture(d, rh, 1);
        fx.desc_clamp(d, 5);
    }
    let p = fx.alpha_preset(0);
    fx.desc_alpha(d, p);
    if !tex_first {
        fx.desc_texture(d, rh, 1);
        fx.desc_clamp(d, 5);
    }
}

/// The tile's corners (n, n) (p, n) (n, p) (p, p), z 0, at sp + 0x6A0..
fn tile_verts(fx: &mut Fx, sp: u32, n: u32, p: u32) {
    for (k, (x, y)) in [(n, n), (p, n), (n, p), (p, p)].into_iter().enumerate() {
        let v = sp + C0 + 0x10 * k as u32;
        fx.mem.write_u32(v, x);
        fx.mem.write_u32(v + 4, y);
        fx.mem.write_u32(v + 8, 0);
        fx.mem.write_u32(v + 0xC, 0x3F80_0000);
    }
}

/// The tile's UVs: `u0` / `u1` across, 0 / 0x400 down.
fn tile_uvs(fx: &mut Fx, sp: u32, u0: u32, u1: u32) {
    uv(fx, sp + 0x788, u0);
    uv(fx, sp + 0x78A, 0);
    uv(fx, sp + 0x798, u1);
    uv(fx, sp + 0x79A, 0);
    uv(fx, sp + 0x7A8, u0);
    uv(fx, sp + 0x7AA, 0x400);
    uv(fx, sp + 0x7B8, u1);
    uv(fx, sp + 0x7BA, 0x400);
}

/// k (0.5 (-0.0109 age^2)) (0x1A7508) of the record at `r`.
fn rise(fx: &mut Fx, r: u32, k: f32) -> f32 {
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let p = Fx::powf2(age);
    ee::mul(k, ee::mul(0.5, ee::mul(f(0xBC32_864D), p)))
}

/// The tile about sp + 0x760: the camera turn turned by `angle` about z
/// (sp + 0x720), its corners through it about the point (sp + 0x6E0..),
/// projected (0x204F70) and submitted at the mean w.
fn tile(fx: &mut Fx, sp: u32, angle: i32, ra: u32) {
    for c in 0..4u32 {
        copy4(fx, sp + 0x720 + 0x10 * c, sp + TURN + 0x10 * c);
    }
    fx.rotate_axis(sp + 0x720, angle, 2);
    for c in 0..4u32 {
        fx.vu_apply_to(sp + Q0 + 0x10 * c, sp + 0x720, sp + C0 + 0x10 * c);
        fx.vu_add4(sp + Q0 + 0x10 * c, sp + Q0 + 0x10 * c, sp + P);
    }
    let d = sp + 0x770;
    if crate::fxlib2::project(fx, sp + Q0, PROJ_OUT, sp + CAM, d, sp + W) {
        let w = fx.mem.f(sp + W);
        let k = fixu(fx, w, ra) as i32;
        fx.submit(d, OT, k);
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

/// The waves (16 updates) in a packet layer [F 0x1E023B4..0x1E02B0C].
fn waves(fx: &mut Fx, sp: u32) {
    let d = sp + 0x840;
    quad_head(fx, d);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, LENS_CLAMP);
    let n = fx.mem.i32(D);
    let c = Fx::clamp_i(n, 0, 0x20);
    let f21 = ee::mul(100.0, ee::div(ee::from_i32(c), 32.0));
    fx.mem.write_u8(sp + 0xD0, 0);
    let (v0, v1) = (D + WAVE_V0, D + WAVE_V1);
    let xyz = [sp + 0x860, sp + 0x878, sp + 0x890, sp + 0x8A8];
    for k in 0..0x10u32 {
        let r = D + WAVES + 0x40 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        fx.camera_vp_to(sp + CAM);
        fx.vu_add4(sp + P, r + 0x30, D + 0x60);
        fx.translate_rows(sp + IDM, sp + TURN, sp + P);
        let m = fx.read_m4(sp + IDM);
        fx.mul_into(Some(sp + CAM), &m);
        let f20 = ee::div(ee::from_i32(fx.mem.i32(r + 4)), 16.0);
        for s6 in 0..9u32 {
            let a = ((s6 << 16) >> 3) as i32;
            let f22 = fx.tsin(a);
            let f0 = fx.tcos(a);
            let rr = r + 8 + 4 * s6;
            let o = 0x30 * s6;
            let g = |fx: &mut Fx| ee::mul(ee::mul(2.0, fx.mem.f(D + 0x90)), f20);
            let pos = |y: f32| if y < 0.0 { 0.0 } else { y };
            let r0 = g(fx);
            let r0 = pos(ee::sub(ee::sub(r0, 15.0), 15.0));
            let r1 = g(fx);
            let r1 = pos(ee::sub(r1, 15.0));
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
            // a0 / a1 as the point loop's last luis left them (2.0, 1.0),
            // a2 the translation, a3 as 0x188420 left it (the turn's
            // first word).
            let a3 = fx.mem.read_u32(sp + TURN);
            fx.texflush([0x4000_0000, 0x3F80_0000, sp + P, a3]);
            let h = fx.mem.i32(D + 4);
            let (_, regs) = fx.unit_draw(h, 1, sp + 0x8B8, sp + 0x8C0);
            fx.texflush(regs);
        }
        let age = fx.mem.i32(r + 4);
        let cc = fx.tcos((age << 14) >> 4);
        let al = fixu(fx, ee::mul(f21, cc), R_WAVE) as u8;
        rgba(fx, sp + 0x850, [0xE4, 0x40, 0x19], al);
        copy8(fx, sp + 0x868, sp + 0x850);
        rgba(fx, sp + 0x880, [0, 0, 0], 0);
        copy8(fx, sp + 0x898, sp + 0x880);
        for s6 in 0..8u32 {
            let (a, b) = (0x30 * s6, 0x30 * (s6 + 1));
            for (half, ras) in [(0u32, [RP_WAVE[0], RP_WAVE[1]]), (0x10, [RP_WAVE[2], RP_WAVE[3]])] {
                let (i0, o0) = (0x10, if half == 0 { 0 } else { 0x20 });
                let vs = [v1 + a + i0, v1 + b + i0, v1 + a + o0, v1 + b + o0];
                if fx.project_n(&vs, sp + CAM, d, sp + W, ras[0]) == 1 {
                    for (uvo, xy) in [(0x858u32, 0x860u32), (0x870, 0x878), (0x888, 0x890), (0x8A0, 0x8A8)] {
                        fx.uv_from_xy(sp + uvo, sp + xy, 0, 0);
                    }
                    let vs = [v0 + a + i0, v0 + b + i0, v0 + a + o0, v0 + b + o0];
                    quad_far(fx, sp, vs, d, xyz, 0, ras[1]);
                }
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r, 0);
        }
    }
    if fx.mem.read_u8(sp + 0xD0) == 1 {
        let layer = fx.mem.read_u32(sp + 0xEC);
        fx.layer_end(layer, OT, 0);
    }
}

/// A sprite of `dir` x `k` about `pos`, raised by `up`, drawn.
fn sprite_up(fx: &mut Fx, sp: u32, dir: u32, k: f32, pos: u32, up: f32, key: Key) {
    fx.vu_scale4(sp + P, dir, k);
    fx.vu_add4(sp + P, sp + P, pos);
    let y = fx.mem.f(sp + P + 4);
    fx.mem.setf(sp + P + 4, ee::add(y, up));
    draw(fx, sp, key);
}

/// The sparks (16 x 0x130, 16 updates): 16 sprites out from the node, 16
/// drawn back in, both rising 5 age / 16 [F 0x1E02CC0..0x1E03350].
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
        let s = ee::add(f(0x3E4C_CCCD), ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(0x10 - age)), 16.0));
        corners(fx, sp, s, true);
        let age = fx.mem.i32(r + 4);
        let sn = fx.tsin((age << 15) >> 4);
        let al = fixu(fx, ee::mul(228.0, sn), R_SPARK1A) as u8;
        rgba(fx, sp + 0x958, [0xFF, 0x46, 0x1E], al);
        let age = fx.mem.i32(r + 4);
        let f21 = spread(fx, age, 0x10, 0x3DCC_CCCD, 0x3F66_6666);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let f20 = ee::div(ee::mul(5.0, age), 16.0);
        for j in 0..0x10u32 {
            let b = i32::from(fx.mem.read_u8(r + 8 + j));
            cell_uvs(fx, sp, b);
            sprite_up(fx, sp, r + 0x30 + 0x10 * j, f21, r + 0x20, f20, Key::Fix(R_SPARK1));
        }
        tex(fx, sp, 0x14);
        alpha(fx, sp, 0);
        let age = fx.mem.i32(r + 4);
        corners(fx, sp, half(5.0, age, 16.0), true);
        let age = fx.mem.i32(r + 4);
        let c = fx.tcos((age << 14) >> 4);
        let al = fixu(fx, ee::mul(96.0, c), R_SPARK2A) as u8;
        rgba(fx, sp + 0x958, [0x8A; 3], al);
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, 0x10, 1);
        let f21 = ee::mul(f(0xBF33_3333), ee::add(f(0x3E99_999A), ee::mul(f(0x3F33_3333), t)));
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let f20 = ee::div(ee::mul(5.0, age), 16.0);
        for j in 0..0x10u32 {
            let b = i32::from(fx.mem.read_u8(r + 8 + j)) + 4;
            cell_uvs(fx, sp, b);
            sprite_up(fx, sp, r + 0x30 + 0x10 * j, f21, r + 0x20, f20, Key::Fix(R_SPARK2));
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The burst (32 updates): a far glow, a 32 x 2 quad disc, 128 far
/// sprites [F 0x1E0326C..0x1E03B64].
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
    rgba(fx, sp + 0x958, [0xFF, 0x80, 0x30], al);
    corners(fx, sp, 48.0, false);
    project_sprite(fx, sp, sp + C0, Key::Far);

    let q = sp + 0x7E8;
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
    rgba(fx, sp + 0x7F8, [0xFF, 0xE4, 0x94], ((0x20 - age).wrapping_mul(255) >> 5) as u8);
    copy8(fx, sp + 0x808, sp + 0x7F8);
    rgba(fx, sp + 0x818, [0xFF, 0x1C, 0x12], 0);
    copy8(fx, sp + 0x828, sp + 0x818);
    let xyz = [sp + 0x800, sp + 0x810, sp + 0x820, sp + 0x830];
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
    rgba(fx, sp + 0x958, [0xFF, 0x80, 0x30], ((0x20 - age).wrapping_mul(255) >> 5) as u8);
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

/// A blast at the target [F 0x1E03BA4..0x1E03E78]: 64 sprites (halfword
/// cells, 2.7), 128 trail directions (3.75), 64 sprites (cells 5..7, 2.7).
fn blast_spawn(fx: &mut Fx, idx: u32) {
    fx.mem.write_u32(D + BLAST_I, idx);
    let r = blast(fx.mem.read_u32(D + BLAST_I));
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 1);
    fx.mem.write_u32(r + 0x90, 0);
    fx.mem.write_u32(r + 0x94, 0);
    fx.mem.write_u32(r + 0x98, 0);
    fx.mem.write_u32(r + 0x9C, 0x3F80_0000);
    fx.vu_add4(r + 0x90, r + 0x90, D + 0x70);
    for j in 0..0x40u32 {
        let b = fx.rand_from(OT, RR_BLAST1) >> 23;
        fx.mem.write_u16(r + 8 + 2 * j, rem2(b, 8) as u16);
        fx.random_table_vec(r + 0xA0 + 0x10 * j, f(0x3FE6_6666));
    }
    for j in 0..0x80u32 {
        fx.random_table_vec(r + 0x4A0 + 0x10 * j, 2.75);
    }
    for j in 0..0x40u32 {
        let b = fx.rand_from(OT, RR_BLAST3) >> 23;
        fx.mem.set8(r + 0xCA0 + j, b % 3 + 5);
        fx.random_table_vec(r + 0xCE0 + 0x10 * j, f(0x402C_CCCD));
    }
}

/// The blasts (48 updates) [F 0x1E03E78..0x1E04C38].
fn blasts(fx: &mut Fx, sp: u32) {
    for k in 0..3u32 {
        let r = blast(k);
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let pos = r + 0x90;
        // 64 far sprites, texture 2.
        head(fx, sp);
        tex(fx, sp, 0x18);
        clamp5(fx, sp);
        alpha(fx, sp, 1);
        camera(fx, sp);
        let age = fx.mem.i32(r + 4);
        corners(fx, sp, ee::mul(18.0, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(age)), 32.0))), true);
        let age = fx.mem.i32(r + 4);
        rgba(fx, sp + 0x958, [0xE6, 0x8A, 0x4C], ((0x20 - age).wrapping_mul(200) >> 5) as u8);
        let age = fx.mem.i32(r + 4);
        let f22 = fx.tan_falloff_p(age, 0x20, 1);
        for j in 0..0x40u32 {
            fx.vu_scale4(sp + P, r + 0xCE0 + 0x10 * j, f22);
            fx.vu_add4(sp + P, sp + P, pos);
            fx.vu_add4(sp + Q0, sp + C0, sp + P);
            fx.vu_add4(sp + Q1, sp + C1, sp + P);
            let b = i32::from(fx.mem.read_u8(r + 0xCA0 + j));
            cell_uvs(fx, sp, b);
            project_sprite(fx, sp, sp + Q0, Key::Far);
        }

        // 64 far sprites, texture 1, alternating.
        head(fx, sp);
        tex(fx, sp, 0x14);
        clamp5(fx, sp);
        camera(fx, sp);
        let age = fx.mem.i32(r + 4);
        corners(fx, sp, ee::mul(15.0, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(age)), 32.0))), true);
        let age = fx.mem.i32(r + 4);
        let a255 = ((0x20 - age).wrapping_mul(255) >> 5) as u8;
        let a128 = ((0x20 - age).wrapping_mul(48) >> 5) as u8;
        fx.mem.write_u8(sp + 0x5D0, a255);
        fx.mem.write_u8(sp + 0xF0, a128);
        for j in 0..0x40u32 {
            let age = fx.mem.i32(r + 4);
            let mut f22 = fx.tan_falloff_p(age, 0x20, 1);
            if j % 2 == 0 {
                f22 = ee::mul(f22, f(0x3F66_6666));
                alpha(fx, sp, 2);
                rgba(fx, sp + 0x958, [0x80, 0xBC, 0xFF], a128);
            } else {
                alpha(fx, sp, 1);
                rgba(fx, sp + 0x958, [0xC8, 0x64, 0x32], a255);
            }
            fx.vu_scale4(sp + P, r + 0xA0 + 0x10 * j, f22);
            fx.vu_add4(sp + P, sp + P, pos);
            fx.vu_add4(sp + Q0, sp + C0, sp + P);
            fx.vu_add4(sp + Q1, sp + C1, sp + P);
            let b = i32::from(fx.mem.read_u16(r + 8 + 2 * j));
            cell_uvs(fx, sp, b);
            project_sprite(fx, sp, sp + Q0, Key::Far);
        }

        // A far glow 195 at the target.
        head(fx, sp);
        tex(fx, sp, 0x10);
        clamp5(fx, sp);
        alpha(fx, sp, 1);
        fx.camera_vp_to(sp + CAM);
        fx.translate_rows(sp + IDM, sp + TURN, pos);
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, 0x20, 1);
        let s = ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), t));
        vec4(fx, sp + P, s, s, s);
        fx.scale_rows(sp + IDM, sp + P);
        let m = fx.read_m4(sp + IDM);
        fx.mul_into(Some(sp + CAM), &m);
        let age = fx.mem.i32(r + 4);
        let al = fixu(fx, ee::mul(200.0, ee::div(ee::from_i32(0x20 - age), 32.0)), R_BLASTC) as u8;
        rgba(fx, sp + 0x958, [0xFF, 0x5E, 0x26], al);
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        corners(fx, sp, 195.0, false);
        project_sprite(fx, sp, sp + C0, Key::Far);

        // 128 tiles.
        tile_head(fx, sp);
        tile_desc(fx, sp, true);
        camera(fx, sp);
        tile_verts(fx, sp, 0xBF80_0000, 0x3F80_0000);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let rp = Fx::ramp_above(age, 32.0, 21.0);
        let al = fixu(fx, ee::mul(128.0, rp), R_TRAILA) as u8;
        rgba(fx, sp + 0x780, [0xFF, 0x80, 0x80], al);
        let age = fx.mem.i32(r + 4);
        let f22 = fx.tan_falloff_p(age, 0x20, 1);
        let f23 = rise(fx, r, 7.0);
        for g in 0..0x80u32 {
            let c = rem2(g as i32, 4) << 6;
            let m = |x: i32| (x & 0x3FFF) as u32;
            tile_uvs(fx, sp, m(c << 4), m((c + 0x40) << 4));
            let k = ee::add(f22, ee::mul(f(0x3BE5_6042), 0.0));
            fx.vu_scale4(sp + P, r + 0x4A0 + 0x10 * g, k);
            fx.vu_add4(sp + P, sp + P, pos);
            let y = fx.mem.f(sp + P + 4);
            fx.mem.setf(sp + P + 4, ee::add(y, f23));
            tile(fx, sp, ((g << 16) as i32) >> 7, R_BLASTD);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}
