//! BMODULE b209 (card 617), hand-ported from the record (linked at
//! 0x1E00000; the one function 0x1E00080 in a 0x450 frame with D spilled
//! to sp + 0xBC and the OT in s0, data area a0 = 0x1ED0000, 0xB7E0 bytes)
//! [F]. b237's family (card 654, b237.rs) at
//! 0.70, written from its own raw listing (the project notes). Slots:
//! the camera sp + 0x1B0 (model + 0x40, turn + 0x80), corners sp + 0x270
//! / 0x280, vertices sp + 0x2B0 / 0x2C0, the placed point sp + 0x2D0, the
//! untextured sprite sp + 0x2E8, the sprite descriptor sp + 0x318, the
//! line descriptor sp + 0x370, the blade quad sp + 0x3B8, the mean w sp +
//! 0x44C; the return slot sp + 0xC0. Init: the target's position D+0x20,
//! node 1 (clear) (D+0x30), a ring of 33 points radius 20 (D+0x60). Every
//! update the camera turn with roll and the node point (D+0x40); from 0 a
//! glow (D+0x50, 0x21 updates: a 40-wide sprite and the line fan to the
//! ring) and 64 blades (D+0x270, two keyed quads each); on 0x1E a white
//! window flash (D+0x1280) that ends the glow and the blades; on
//! 0x1F..0x55 a mote record a update at the node (20 x 0x240 at
//! D+0x1290); on 0x56 256 sparks (D+0x3F90), 32 embers (D+0x50A0) with
//! their trail records (16 x 0x650 at D+0x52D0) and a frame flash
//! (D+0xB7D0). Returns 1 on 0x4C, 2 on 0x56, 8 once the count reaches
//! 0x87.
use crate::b182::{alpha, clamp_regs, head, rgba};
use crate::b440::{copy8, copy_z, fix, rem2, vec4, words};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::far_xy;

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
const ONE: u32 = 0x3F80_0000;
const HALF: u32 = 0x3F00_0000;

const GLOW: u32 = 0x50;
const RING: u32 = 0x60;
const BLADES: u32 = 0x270;
const WHITE: u32 = 0x1280;
const MOTES: u32 = 0x1290;
const SPARKS: u32 = 0x3F90;
const EMBERS: u32 = 0x50A0;
const TRAILS: u32 = 0x52D0;
const FLASH: u32 = 0xB7D0;

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
    w: u32,
    d: u32,
    l: u32,
    q: u32,
    mean: u32,
}

impl Fr {
    fn new(sp: u32) -> Fr {
        Fr {
            sp,
            vp: sp + 0x1B0,
            model: sp + 0x1F0,
            turn: sp + 0x230,
            c0: sp + 0x270,
            c1: sp + 0x280,
            v0: sp + 0x2B0,
            pl: sp + 0x2D0,
            w: sp + 0x2E8,
            d: sp + 0x318,
            l: sp + 0x370,
            q: sp + 0x3B8,
            mean: sp + 0x44C,
        }
    }
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x450);
    fx.mem.write_u32(sp + 0xBC, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let f = Fr::new(sp);
    let ret = if param >= 0 { init(fx) } else { step(fx, &f) };
    fx.leave(0x450);
    ret
}

/// rand >> 23 from `ra` (s0 = the OT).
fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(OT, ra) >> 23
}

/// ((r % 256) << 16) >> 8 of a rand from `ra`.
fn rand_angle(fx: &mut Fx, ra: u32) -> i32 {
    let r = rnd(fx, ra);
    (rem2(r, 256) << 16) >> 8
}

// ------------------------------------------------------------ init

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xB7E0);
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
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 1, 1);
    fx.mem.set32(D + 0x30, r);
    fx.mem.write_u8(D + GLOW, 0);
    for k in 0..0x21i32 {
        let a = (k << 16) >> 5;
        let sn = fx.tsin(a);
        let p = D + RING + 0x10 * k as u32;
        fx.mem.setf(p, ee::mul(20.0, sn));
        let cs = fx.tcos(a);
        fx.mem.setf(p + 4, ee::mul(20.0, cs));
        fx.mem.write_u32(p + 8, 0);
        fx.mem.write_u32(p + 0xC, ONE);
    }
    fx.mem.write_u8(D + BLADES, 0);
    fx.mem.write_u8(D + WHITE, 0);
    for k in 0..0x14u32 {
        fx.mem.write_u8(D + MOTES + 0x240 * k, 0);
    }
    fx.mem.write_u8(D + SPARKS, 0);
    fx.mem.write_u8(D + EMBERS, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + TRAILS + 0x650 * k, 0);
    }
    fx.mem.write_u8(D + FLASH, 0);
    0
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
/// `tex`, ALPHA preset 1 and clamp 5.
fn sprite_desc(fx: &mut Fx, f: &Fr, tex: u32) {
    head(fx, f.d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(f.d, rh, 1);
    alpha(fx, f.d, 1);
    fx.desc_clamp(f.d, 5);
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
    corners_at(fx, f);
}

fn corners_at(fx: &mut Fx, f: &Fr) {
    fx.vu_add4(f.v0, f.c0, f.pl);
    fx.vu_add4(f.v0 + 0x10, f.c1, f.pl);
}

fn project(fx: &mut Fx, f: &Fr, input: u32, d: u32) -> bool {
    crate::fxlib2::project(fx, input, PROJ_OUT, f.vp, d, f.mean)
}

/// `d` projected from `input` and submitted at the mean w (0x1A2C70 from
/// `ra`).
fn draw_at(fx: &mut Fx, f: &Fr, input: u32, d: u32, ra: u32) {
    if project(fx, f, input, d) {
        let w = fx.mem.f(f.mean);
        let key = fix(fx, w, ra);
        fx.submit(d, OT, key as i32);
    }
}

/// The sprite projected from its vertices and submitted at the mean w.
fn draw(fx: &mut Fx, f: &Fr, ra: u32) {
    draw_at(fx, f, f.v0, f.d, ra);
}

/// The far Z on the XYZ2 at `first`, copied to `rest`.
fn far(fx: &mut Fx, first: u32, rest: &[u32]) {
    let x = fx.mem.read_u64(first);
    fx.mem.write_u64(first, (x & KEEP) | FAR_Z);
    for &r in rest {
        copy_z(fx, r, first);
    }
}

/// (0.8 + 0.2 (age % 2)) clamp(age, 0, 32) / 32.
fn glow_f(age: i32) -> f32 {
    let c1 = Fx::clamp_i(age, 0, 0x20);
    let f3 = ee::div(ee::from_i32(c1), 32.0);
    let f2 = ee::from_i32(rem2(age, 2));
    ee::mul(ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), f2)), f3)
}

/// 0.1 + 0.9 x.
fn tenth(x: f32) -> f32 {
    ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), x))
}

/// s (0.5 + 0.5 (n - age) / n).
fn shrink(s: f32, n: i32, age: i32) -> f32 {
    let x = ee::div(ee::mul(0.5, ee::from_i32(n - age)), ee::from_i32(n));
    ee::mul(s, ee::add(0.5, x))
}

// ------------------------------------------------------------ step

fn step(fx: &mut Fx, f: &Fr) -> i32 {
    let sp = f.sp;
    fx.camera_turn_roll(f.turn);
    let h = fx.mem.i32(D + 4);
    let s = fx.mem.i32(D + 0x30);
    fx.node_position(h, s, D + 0x40);
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + GLOW, 1);
        fx.mem.write_u32(D + GLOW + 4, 0);
    }
    if fx.mem.read_u8(D + GLOW) == 1 {
        glow(fx, f);
    }
    if fx.mem.i32(D) == 0 {
        blades_spawn(fx, f);
    }
    if fx.mem.read_u8(D + BLADES) == 1 {
        blades(fx, f);
    }
    if fx.mem.i32(D) == 0x1E {
        fx.mem.write_u8(D + WHITE, 1);
        fx.mem.write_u32(D + WHITE + 4, 0);
        fx.mem.write_u8(D + GLOW, 0);
        fx.mem.write_u8(D + BLADES, 0);
    }
    if fx.mem.read_u8(D + WHITE) == 1 {
        white(fx, f);
    }
    let n = fx.mem.i32(D);
    if (0x1F..0x56).contains(&n) {
        let k = fx.div(n, 0x14).1 as u32;
        let r = D + MOTES + 0x240 * k;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x30, D + 0x40);
        for j in 0..0x20u32 {
            let c = rnd(fx, 0x01E0_0E0C);
            fx.mem.set8(r + 8 + j, rem2(c, 8));
            fx.random_table_vec(r + 0x40 + 0x10 * j, 0.5);
        }
    }
    motes(fx, f);
    if fx.mem.i32(D) == 0x56 {
        let r = D + SPARKS;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for k in 0..0x100u32 {
            let c = rnd(fx, 0x01E0_1230);
            fx.mem.set8(r + 8 + k, rem2(c, 8));
            fx.random_table_vec(r + 0x110 + 0x10 * k, f32::from_bits(0x3F99_999A));
        }
    }
    if fx.mem.read_u8(D + SPARKS) == 1 {
        sparks(fx, f);
    }
    if fx.mem.i32(D) == 0x56 {
        let r = D + EMBERS;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for k in 0..0x20u32 {
            let c = rnd(fx, 0x01E0_1608);
            fx.mem.set8(r + 8 + k, rem2(c, 8));
            fx.random_table_vec(r + 0x30 + 0x10 * k, 3.0);
        }
    }
    if fx.mem.read_u8(D + EMBERS) == 1 {
        embers(fx, f);
    }
    trails(fx, f);
    if fx.mem.i32(D) == 0x56 {
        fx.mem.write_u8(D + FLASH, 1);
        fx.mem.write_u32(D + FLASH + 4, 0);
    }
    if fx.mem.read_u8(D + FLASH) == 1 {
        flash(fx, f);
    }
    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xC0);
    if n == 0x4C {
        ret = 1;
    } else if n == 0x56 {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x87 {
        ret = 8;
    }
    fx.mem.set32(sp + 0xC0, ret);
    ret
}

/// The glow at the node [F b209 0x1E002B0..0x1E006DC]: a 40-wide sprite
/// (E8, E0, BE) at alpha 255 f and 32 line triangles from the node to the
/// ring (FF, D8, C4) at 128 f fading to (64, 64, 64), f = [`glow_f`], 0x21
/// updates.
fn glow(fx: &mut Fx, f: &Fr) {
    let d = f.d;
    sprite_desc(fx, f, 0x10);
    uvs(fx, f, 0, 0, 0x800, 0x800);
    fx.camera_vp_to(f.vp);
    fx.translate_rows(f.model, f.turn, D + 0x40);
    let mm = fx.read_m4(f.model);
    fx.mul_into(Some(f.vp), &mm);
    let age = fx.mem.i32(D + GLOW + 4);
    let f20 = glow_f(age);
    let a = fix(fx, ee::mul(255.0, f20), 0x01E0_041C);
    colour(fx, f, [0xE8, 0xE0, 0xBE], a as u8);
    corners(fx, f, 20.0, false);
    if project(fx, f, f.c0, d) {
        far(fx, d + 0x20, &[d + 0x30]);
        fx.submit(d, OT, 0);
    }
    let l = f.l;
    head(fx, l, 0x13, 6, 0x41_4141, 0);
    alpha(fx, l, 1);
    let a = fix(fx, ee::mul(128.0, f20), 0x01E0_0554);
    rgba(fx, l + 0x10, [0xFF, 0xD8, 0xC4], a as u8);
    rgba(fx, l + 0x20, [0x64, 0x64, 0x64], 0);
    copy8(fx, l + 0x30, l + 0x20);
    vec4(fx, f.pl, [0, 0, 0, ONE]);
    for k in 0..0x20u32 {
        let p = D + RING + 0x10 * k;
        if fx.project_n(&[f.pl, p, p + 0x10], f.vp, l, f.mean, 0x01E0_05F8) == 1 {
            far(fx, l + 0x18, &[l + 0x28, l + 0x38]);
            fx.submit(l, OT, 0);
        }
    }
    let age = fx.mem.i32(D + GLOW + 4) + 1;
    fx.mem.set32(D + GLOW + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + GLOW, 0);
    }
}

/// On 0 the blades (D+0x270): 64 matrices aimed along a random direction
/// and turned about x / y / z by random angles (D+0x280 + 0x40 k) [F b209
/// 0x1E006EC..0x1E007CC].
fn blades_spawn(fx: &mut Fx, f: &Fr) {
    let v = f.pl;
    fx.mem.write_u8(D + BLADES, 1);
    fx.mem.write_u32(D + BLADES + 4, 0);
    for k in 0..0x40u32 {
        let m = D + BLADES + 0x10 + 0x40 * k;
        fx.random_table_vec(v, 1.0);
        fx.point_along(m, v);
        for (axis, ra) in [(0, 0x01E0_0728), (1, 0x01E0_075C), (2, 0x01E0_0790)] {
            let a = rand_angle(fx, ra);
            fx.rotate_axis(m, a, axis);
        }
    }
}

/// The 64 blades at the node [F b209 0x1E007E4..0x1E00C14]: each its
/// matrix spun by age (0x14D, 0x1BC, 0x63), two quads (0, -+1..0, 0..40)
/// with ST floats, colours (FF, D8, C4, 0) / (FF, D8, C4, 48 f) / (20,
/// 10, 0C, 0) x 2, keyed by their mean w, 0x21 updates.
fn blades(fx: &mut Fx, f: &Fr) {
    let q = f.q;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(q, rh, 1);
    alpha(fx, q, 1);
    fx.desc_clamp(q, 5);
    let age = fx.mem.i32(D + BLADES + 4);
    let f1 = glow_f(age);
    let a = fix(fx, ee::mul(48.0, f1), 0x01E0_08D4);
    rgba(fx, q + 0x10, [0xFF, 0xD8, 0xC4], 0);
    rgba(fx, q + 0x28, [0xFF, 0xD8, 0xC4], a as u8);
    rgba(fx, q + 0x40, [0x20, 0x10, 0x0C], 0);
    rgba(fx, q + 0x58, [0x20, 0x10, 0x0C], 0);
    let v = f.c0;
    for k in 0..0x40u32 {
        fx.camera_vp_to(f.vp);
        let m = D + BLADES + 0x10 + 0x40 * k;
        fx.translate_rows(f.model, m, D + 0x40);
        let age = fx.mem.i32(D + BLADES + 4);
        let a = fx.mult(age, 0x14D);
        fx.rotate_axis(f.model, a, 0);
        let age = fx.mem.i32(D + BLADES + 4);
        fx.rotate_axis(f.model, age.wrapping_mul(0x1BC), 1);
        let age = fx.mem.i32(D + BLADES + 4);
        fx.rotate_axis(f.model, age.wrapping_mul(0x63), 2);
        let mm = fx.read_m4(f.model);
        fx.mul_into(Some(f.vp), &mm);
        for (s, y, ra) in [(0u32, 0xBF80_0000u32, 0x01E0_0AD4), (ONE, ONE, 0x01E0_0BC8)] {
            for (o, x) in [(0x18u32, s), (0x1C, HALF), (0x30, HALF), (0x34, HALF), (0x48, s), (0x4C, ONE), (0x60, HALF), (0x64, ONE)] {
                fx.mem.write_u32(q + o, x);
            }
            vec4(fx, v, [0, y, 0, ONE]);
            vec4(fx, v + 0x10, [0, 0, 0, ONE]);
            vec4(fx, v + 0x20, [0, y, 0x4220_0000, ONE]);
            vec4(fx, v + 0x30, [0, 0, 0x4220_0000, ONE]);
            draw_at(fx, f, v, q, ra);
        }
    }
    let age = fx.mem.i32(D + BLADES + 4) + 1;
    fx.mem.set32(D + BLADES + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + BLADES, 0);
    }
}

/// The white window flash (D+0x1280) [F b209 0x1E00C40..0x1E00D8C]: an
/// untextured sprite (prim 6) over the window (FF, E8, D8) at alpha 128 (1
/// - falloff(age, 32)), 0x21 updates.
fn white(fx: &mut Fx, f: &Fr) {
    let w = f.w;
    head(fx, w, 6, 3, 0x441, 0);
    alpha(fx, w, 1);
    let age = fx.mem.i32(D + WHITE + 4);
    let tf = fx.tan_falloff_p(age, 0x20, 1);
    let a = fix(fx, ee::mul(128.0, ee::sub(1.0, tf)), 0x01E0_0CB8);
    rgba(fx, w + 0x10, [0xFF, 0xE8, 0xD8], a as u8);
    far_xy(fx, w + 0x18, 0x7000, 0x7000);
    far_xy(fx, w + 0x20, 0x9000, 0x9000);
    fx.submit(w, OT, 0);
    let age = fx.mem.i32(D + WHITE + 4) + 1;
    fx.mem.set32(D + WHITE + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + WHITE, 0);
    }
}

/// The 20 mote records (D+0x1290 + 0x240 k: point +0x30, 32 cells +8, 0.5
/// vectors +0x40) [F b209 0x1E00E50..0x1E011FC]: 5 (0.5 + (20 - age) /
/// 40) wide (FF, 4B, 28) at alpha 70 sin(age / 20 quarter turn), spread
/// (0.9 + 0.1 (20 - age) / 20) in x / z and rising by -2 + 30 age / 20,
/// 0x15 updates.
fn motes(fx: &mut Fx, f: &Fr) {
    sprite_desc(fx, f, 0x18);
    camera(fx, f);
    for m in 0..0x14u32 {
        let r = D + MOTES + 0x240 * m;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let x = fx.mem.i32(r + 4) << 14;
        fx.mult(0x6666_6667, x);
        let a = (fx.hilo().1 >> 3).wrapping_add(((x as u32) >> 31) as i32);
        let c = fx.tcos(a);
        let al = fix(fx, ee::mul(70.0, c), 0x01E0_0F48);
        colour(fx, f, [0xFF, 0x4B, 0x28], al as u8);
        let age = fx.mem.i32(r + 4);
        corners(fx, f, shrink(5.0, 0x14, age), true);
        let age = fx.mem.i32(r + 4);
        let f4 = ee::div(ee::from_i32(age), 20.0);
        let f20 = ee::add(f32::from_bits(0x3F66_6666), ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(0x14 - age)), 20.0));
        let f21 = ee::add(-2.0, ee::mul(30.0, f4));
        for j in 0..0x20u32 {
            let c = i32::from(fx.mem.read_u8(r + 8 + j));
            cell_uvs(fx, f, c);
            let dir = r + 0x40 + 0x10 * j;
            let x = fx.mem.f(dir);
            fx.mem.setf(f.pl, ee::mul(x, f20));
            let y = fx.mem.f(dir + 4);
            fx.mem.setf(f.pl + 4, ee::add(f21, y));
            let z = fx.mem.f(dir + 8);
            fx.mem.setf(f.pl + 8, ee::mul(z, f20));
            fx.vu_add4(f.pl, f.pl, r + 0x30);
            corners_at(fx, f);
            draw(fx, f, 0x01E0_1194);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x15 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The 256 sparks at the target (D+0x3F90: cells +8, 1.2 vectors +0x110)
/// [F b209 0x1E0128C..0x1E015D4], as b237's.
fn sparks(fx: &mut Fx, f: &Fr) {
    let r = D + SPARKS;
    head(fx, f.d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(f.d, rh, 1);
    fx.desc_clamp(f.d, 5);
    camera(fx, f);
    corners(fx, f, 6.0, true);
    let age = fx.mem.i32(r + 4);
    let tf = fx.tan_falloff_p(age, 0x20, 2);
    let f20 = tenth(tf);
    let age = fx.mem.i32(r + 4);
    let rp = Fx::ramp_above(ee::from_i32(age), 32.0, 16.0);
    let f21 = ee::mul(f32::from_bits(0x3F4C_CCCD), f20);
    let f22 = ee::mul(128.0, rp);
    for k in 0..0x100u32 {
        let f23 = if fx.div(k as i32, 3).1 == 0 {
            alpha(fx, f.d, 2);
            let a = fix(fx, f22, 0x01E0_1410);
            colour(fx, f, [0x80; 3], a as u8);
            f21
        } else {
            alpha(fx, f.d, 1);
            let a = fix(fx, f22, 0x01E0_1454);
            colour(fx, f, [0xFF, 0x50, 0x23], a as u8);
            f20
        };
        let c = i32::from(fx.mem.read_u8(r + 8 + k));
        cell_uvs(fx, f, c);
        place(fx, f, r + 0x110 + 0x10 * k, f23, D + 0x20);
        draw(fx, f, 0x01E0_1584);
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(r, 0);
    }
}

/// The 32 embers at the target (D+0x50A0) with their trail records
/// (D+0x52D0 + 0x650 (age % 16)) [F b209 0x1E01664..0x1E01A80], as b237's.
fn embers(fx: &mut Fx, f: &Fr) {
    let r = D + EMBERS;
    sprite_desc(fx, f, 0x18);
    camera(fx, f);
    corners(fx, f, 3.0, true);
    let age = fx.mem.i32(r + 4);
    let f21 = Fx::ramp_above(ee::from_i32(age), 32.0, 16.0);
    let a = fix(fx, ee::mul(255.0, f21), 0x01E0_1778);
    colour(fx, f, [0xFF, 0x40, 0x20], a as u8);
    let age = fx.mem.i32(r + 4);
    let tf = fx.tan_falloff_p(age, 0x20, 1);
    let f20 = tenth(tf);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let p2 = Fx::powf2(age);
    let f22 = ee::mul(8.0, ee::mul(0.5, ee::mul(f32::from_bits(0xBC32_864D), p2)));
    let idx = rem2(fx.mem.i32(r + 4), 0x10);
    let off = fx.mult(idx, 0x650) as u32;
    let t = D + TRAILS + off;
    fx.mem.write_u8(t, 1);
    fx.mem.write_u32(t + 4, 0);
    fx.mem.setf(t + 0x48, f21);
    for k in 0..0x20u32 {
        for s in 0..2u32 {
            let c = rnd(fx, 0x01E0_1870);
            fx.mem.set8(t + 8 + 2 * k + s, rem2(c, 8));
            fx.random_table_vec(t + 0x250 + 0x20 * k + 0x10 * s, f32::from_bits(0x3E4C_CCCD));
        }
    }
    for k in 0..0x20u32 {
        let c = i32::from(fx.mem.read_u8(r + 8 + k));
        cell_uvs(fx, f, c);
        fx.vu_scale4(f.pl, r + 0x30 + 0x10 * k, f20);
        let y = ee::add(fx.mem.f(f.pl + 4), f22);
        fx.mem.setf(f.pl + 4, y);
        fx.vu_add4(f.pl, f.pl, D + 0x20);
        corners_at(fx, f);
        words(fx, t + 0x50 + 0x10 * k, f.pl);
        draw(fx, f, 0x01E0_1A2C);
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(r, 0);
    }
}

/// The 16 trail records [F b209 0x1E01A80..0x1E01E40], as b237's.
fn trails(fx: &mut Fx, f: &Fr) {
    sprite_desc(fx, f, 0x18);
    camera(fx, f);
    for m in 0..0x10u32 {
        let t = D + TRAILS + 0x650 * m;
        if fx.mem.read_u8(t) == 0 {
            continue;
        }
        let age = fx.mem.i32(t + 4);
        corners(fx, f, shrink(3.0, 0x10, age), true);
        let age = fx.mem.i32(t + 4);
        let rp = Fx::ramp_above(ee::from_i32(age), 16.0, 8.0);
        let w = fx.mem.f(t + 0x48);
        let a = fix(fx, ee::mul(ee::mul(255.0, rp), w), 0x01E0_1BFC);
        colour(fx, f, [0xFF, 0x40, 0x20], a as u8);
        let age = fx.mem.i32(t + 4);
        let tf = fx.tan_falloff_p(age, 0x10, 1);
        let f20 = tenth(tf);
        for k in 0..0x20u32 {
            for s in 0..2u32 {
                let c = i32::from(fx.mem.read_u8(t + 8 + 2 * k + s));
                cell_uvs(fx, f, c);
                place(fx, f, t + 0x250 + 0x20 * k + 0x10 * s, f20, t + 0x50 + 0x10 * k);
                draw(fx, f, 0x01E0_1D88);
            }
        }
        let age = fx.mem.i32(t + 4) + 1;
        fx.mem.set32(t + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(t, 0);
        }
    }
}

/// The frame flash (D+0xB7D0) [F b209 0x1E01E90..0x1E020C8], as b237's
/// (frame texture into sp + 0x358 / 0x360).
fn flash(fx: &mut Fx, f: &Fr) {
    let (sp, d) = (f.sp, f.d);
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 0);
    fx.desc_clamp(d, 5);
    let a = clamp_regs(fx, d, 5);
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x358, sp + 0x360);
    fx.texflush(regs);
    let age = fx.mem.i32(D + FLASH + 4);
    rgba(fx, d + 0x10, [0x80; 3], (0x20 - age) as u8);
    far_xy(fx, d + 0x20, 0x7000, 0x7000);
    far_xy(fx, d + 0x30, 0x9000, 0x9000);
    for s in 0..8i32 {
        let v = ((s + 1) * 34) >> 3;
        let lo = ((v << 4) & 0x3FFF) as u32;
        let hi = (((0x200 - v) << 4) & 0x3FFF) as u32;
        uvs(fx, f, lo, lo, hi, hi);
        fx.submit(d, 0, 0);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + FLASH + 4) + 1;
    fx.mem.set32(D + FLASH + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + FLASH, 0);
    }
}
