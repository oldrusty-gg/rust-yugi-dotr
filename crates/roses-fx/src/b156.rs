//! BMODULE b156 (card 651), hand-ported from the record (linked at
//! 0x1E00000; the one function 0x1E00080 in a 0x620 frame with D spilled
//! to sp + 0xCC and the OT in s3, data area a0 = 0x1ED0000, 0xABD0 bytes)
//! [F]. Written cold from the raw listing
//! (the project notes; nothing ported above 0.42). Slots: the camera sp
//! + 0x320 (model + 0x40, turn + 0x80), corners sp + 0x3E0.., vertices sp
//! + 0x420.., a matrix sp + 0x460, the placed point sp + 0x4A0, the sprite
//! descriptor sp + 0x4B8, the textured quad sp + 0x510, the ring quad sp +
//! 0x588, the mean w sp + 0x61C; the return slot sp + 0xD0. Sprite UVs
//! come from the EXE's cell table 0x28AE70 (8 bytes a cell: u0, u1, v0,
//! v1). Init: 4 textures, the target's position D+0x20, nodes 0x58 (clear)
//! / 0x69, a ring of 17 inner / outer points (radius 10 / 20, D+0x1ED0).
//! Every update the camera turn with roll and the two node points (D+0x40
//! / D+0x50); on 1..0x31 a puff record (32 x 0xD0 at D+0x60) at the nodes;
//! on 0x32 a dust ring at the nodes (D+0x1A60: 64 sparks and a 16-quad
//! ring each) and two spinning cards (D+0x20F0) that drop a trail record
//! every update (2 x 32 x 0xA0 at D+0x21A0); on 0x4A 256 shards (D+0x49A0),
//! 256 sparks (D+0x9AB0) at the target and a frame flash (D+0xABC0).
//! Returns 1 on 0x40, 2 on 0x4A, 8 once the count reaches 0x8B.
use crate::b182::{alpha, clamp_regs, head, rgba};
use crate::b440::{copy8, fix, rem2, vec4, words};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::far_xy;

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const ONE: u32 = 0x3F80_0000;
const CELL_UV: u32 = 0x0028_AE70;

const PUFFS: u32 = 0x60;
const DUST: u32 = 0x1A60;
const RING: u32 = 0x1ED0;
const CARDS: u32 = 0x20F0;
const TRAILS: u32 = 0x21A0;
const SHARDS: u32 = 0x49A0;
const SPARKS: u32 = 0x9AB0;
const FLASH: u32 = 0xABC0;

/// The record's working slots.
struct Fr {
    sp: u32,
    vp: u32,
    model: u32,
    turn: u32,
    c0: u32,
    c1: u32,
    v0: u32,
    m: u32,
    pl: u32,
    d: u32,
    g: u32,
    q: u32,
    mean: u32,
}

impl Fr {
    fn new(sp: u32) -> Fr {
        Fr {
            sp,
            vp: sp + 0x320,
            model: sp + 0x360,
            turn: sp + 0x3A0,
            c0: sp + 0x3E0,
            c1: sp + 0x3F0,
            v0: sp + 0x420,
            m: sp + 0x460,
            pl: sp + 0x4A0,
            d: sp + 0x4B8,
            g: sp + 0x510,
            q: sp + 0x588,
            mean: sp + 0x61C,
        }
    }
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x620);
    fx.mem.write_u32(sp + 0xCC, D);
    fx.mem.write_u32(sp + 0xD0, 0);
    let f = Fr::new(sp);
    let ret = if param >= 0 { init(fx) } else { step(fx, &f) };
    fx.leave(0x620);
    ret
}

/// rand >> 23 from `ra` with the caller's s0 (sp + 0x4E0 from the puffs'
/// UV pointer on, a puff record's cell pointer in its spawn).
fn rnd(fx: &mut Fx, s0: u32, ra: u32) -> i32 {
    fx.rand_from(s0, ra) >> 23
}

/// ((r % 256) << 16) >> 8 of a rand from `ra`.
fn rand_angle(fx: &mut Fx, s0: u32, ra: u32) -> i32 {
    let r = rnd(fx, s0, ra);
    (rem2(r, 256) << 16) >> 8
}

// ------------------------------------------------------------ init

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xABD0);
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
    fx.unit_position(t, D + 0x20);
    for (k, (node, clear)) in [(0x58, 1), (0x69, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0x30 + 4 * k as u32, r);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + PUFFS + 0xD0 * k, 0);
    }
    fx.mem.write_u8(D + DUST, 0);
    for k in 0..0x11i32 {
        let a = (k << 16) >> 4;
        let sn = fx.tsin(a);
        let cs = fx.tcos(a);
        let p = D + RING + 0x20 * k as u32;
        fx.mem.setf(p, ee::mul(10.0, sn));
        fx.mem.setf(p + 4, ee::mul(10.0, cs));
        fx.mem.write_u32(p + 8, 0);
        fx.mem.write_u32(p + 0xC, ONE);
        fx.mem.setf(p + 0x10, ee::mul(20.0, sn));
        fx.mem.setf(p + 0x14, ee::mul(20.0, cs));
        fx.mem.write_u32(p + 0x18, 0);
        fx.mem.write_u32(p + 0x1C, ONE);
    }
    fx.mem.write_u8(D + CARDS, 0);
    for j in 0..2u32 {
        for k in 0..0x20u32 {
            fx.mem.write_u8(D + TRAILS + 0x1400 * j + 0xA0 * k, 0);
        }
    }
    fx.mem.write_u8(D + SHARDS, 0);
    fx.mem.write_u8(D + SPARKS, 0);
    fx.mem.write_u8(D + FLASH, 0);
    0
}

// ------------------------------------------------------------ shared parts

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, d: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, d + 0x18, u0);
    uv(fx, d + 0x1A, v0);
    uv(fx, d + 0x28, u1);
    uv(fx, d + 0x2A, v1);
}

/// The sprite's UVs from the EXE cell table entry of `c` (u0 +0, u1 +2,
/// v0 +4, v1 +6).
fn cell_uvs(fx: &mut Fx, f: &Fr, c: u32) {
    let t = CELL_UV + 8 * c;
    let (u0, u1, v0, v1) = (fx.mem.read_u16(t), fx.mem.read_u16(t + 2), fx.mem.read_u16(t + 4), fx.mem.read_u16(t + 6));
    uvs(fx, f.d, u32::from(u0), u32::from(v0), u32::from(u1), u32::from(v1));
}

/// The quads' UVs for cell `c` of a 4-wide strip ((c << 10), 0) ..
/// ((c << 6) + 0x40) << 4, 0x400).
fn strip_uvs(fx: &mut Fx, g: u32, c: i32) {
    let u0 = ((c << 10) & 0x3FFF) as u32;
    let u1 = ((((c << 6) + 0x40) << 4) & 0x3FFF) as u32;
    for (o, x) in [(0x18u32, u0), (0x1A, 0), (0x28, u1), (0x2A, 0), (0x38, u0), (0x3A, 0x400), (0x48, u1), (0x4A, 0x400)] {
        uv(fx, g + o, x);
    }
}

fn camera(fx: &mut Fx, f: &Fr) {
    fx.camera_vp_to(f.vp);
    fx.identity(f.model);
    let id = fx.read_m4(f.model);
    fx.mul_into(Some(f.vp), &id);
}

/// The sprite descriptor's head (prim 0x26 | 0x800), then clamp 5.
fn sprite_head(fx: &mut Fx, f: &Fr) {
    head(fx, f.d, 0x26, 5, 0x4_2421, 0x800);
    fx.desc_clamp(f.d, 5);
}

/// The sprite's texture D + `tex` and its ALPHA preset.
fn tex_alpha(fx: &mut Fx, d: u32, tex: u32, preset: i32) {
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    alpha(fx, d, preset);
}

/// The corners (-s, -s, 0, 1) and (s, s, 0, 1) turned by the camera turn.
fn corners(fx: &mut Fx, f: &Fr, s: f32) {
    let n = ee::neg(s);
    vec4(fx, f.c0, [n.to_bits(), n.to_bits(), 0, ONE]);
    vec4(fx, f.c1, [s.to_bits(), s.to_bits(), 0, ONE]);
    fx.vu_apply_to(f.c0, f.turn, f.c0);
    fx.vu_apply_to(f.c1, f.turn, f.c1);
}

/// The four quad corners (-+s, -+s, 0, 1) in sprite order.
fn corners4(fx: &mut Fx, f: &Fr, s: f32) {
    let (n, p) = (ee::neg(s).to_bits(), s.to_bits());
    for (k, (x, y)) in [(n, n), (p, n), (n, p), (p, p)].into_iter().enumerate() {
        vec4(fx, f.c0 + 0x10 * k as u32, [x, y, 0, ONE]);
    }
}

fn colour(fx: &mut Fx, d: u32, c: [u8; 3], a: u8) {
    rgba(fx, d + 0x10, c, a);
}

fn corners_at(fx: &mut Fx, f: &Fr) {
    fx.vu_add4(f.v0, f.c0, f.pl);
    fx.vu_add4(f.v0 + 0x10, f.c1, f.pl);
}

/// The placed point (= dir x s + at) and the sprite's vertices around it.
fn place(fx: &mut Fx, f: &Fr, dir: u32, s: f32, at: u32) {
    fx.vu_scale4(f.pl, dir, s);
    fx.vu_add4(f.pl, f.pl, at);
    corners_at(fx, f);
}

/// The placed point (= dir x s, y + `dy`, + at) and the sprite's vertices.
fn place_dy(fx: &mut Fx, f: &Fr, dir: u32, s: f32, dy: f32, at: u32) {
    fx.vu_scale4(f.pl, dir, s);
    let y = ee::add(fx.mem.f(f.pl + 4), dy);
    fx.mem.setf(f.pl + 4, y);
    fx.vu_add4(f.pl, f.pl, at);
    corners_at(fx, f);
}

/// `d` projected from `input` and submitted at the mean w (0x1A2C70 from
/// `ra`).
fn draw_at(fx: &mut Fx, f: &Fr, input: u32, d: u32, ra: u32) {
    if crate::fxlib2::project(fx, input, PROJ_OUT, f.vp, d, f.mean) {
        let w = fx.mem.f(f.mean);
        let key = fix(fx, w, ra);
        fx.submit(d, OT, key as i32);
    }
}

fn draw(fx: &mut Fx, f: &Fr, ra: u32) {
    draw_at(fx, f, f.v0, f.d, ra);
}

/// a cos((age << 14) >> sh) through 0x1A2C70 from `ra`.
fn cos_alpha(fx: &mut Fx, age: i32, sh: u32, a: f32, ra: u32) -> u8 {
    let c = fx.tcos((age << 14) >> sh);
    fix(fx, ee::mul(a, c), ra) as u8
}

/// s (0.5 + 0.5 x / n).
fn half_grow(s: f32, x: i32, n: f32) -> f32 {
    let g = ee::div(ee::mul(0.5, ee::from_i32(x)), n);
    ee::mul(s, ee::add(0.5, g))
}

/// 0.1 + 0.9 x.
fn tenth(x: f32) -> f32 {
    ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), x))
}

// ------------------------------------------------------------ step

fn step(fx: &mut Fx, f: &Fr) -> i32 {
    let sp = f.sp;
    fx.camera_turn_roll(f.turn);
    for k in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x30 + 4 * k);
        fx.node_position(h, s, D + 0x40 + 0x10 * k);
    }
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x32 {
        puff_spawn(fx, rem2(n, 0x20) as u32);
    }
    puffs(fx, f);
    if fx.mem.i32(D) == 0x32 {
        dust_spawn(fx, sp);
    }
    if fx.mem.read_u8(D + DUST) == 1 {
        dust(fx, f);
    }
    if fx.mem.i32(D) == 0x32 {
        cards_spawn(fx, sp);
    }
    if fx.mem.read_u8(D + CARDS) == 1 {
        cards(fx, f);
    }
    trails(fx, f);
    if fx.mem.i32(D) == 0x4A {
        shards_spawn(fx, f);
    }
    if fx.mem.read_u8(D + SHARDS) == 1 {
        shards(fx, f);
    }
    if fx.mem.i32(D) == 0x4A {
        let r = D + SPARKS;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for k in 0..0x100u32 {
            let c = rnd(fx, sp + 0x4E0, 0x01E0_1ECC);
            fx.mem.set8(r + 8 + k, rem2(c, 4));
            fx.random_table_vec(r + 0x110 + 0x10 * k, f32::from_bits(0x3F99_999A));
        }
    }
    if fx.mem.read_u8(D + SPARKS) == 1 {
        sparks(fx, f);
    }
    if fx.mem.i32(D) == 0x4A {
        fx.mem.write_u8(D + FLASH, 1);
        fx.mem.write_u32(D + FLASH + 4, 0);
    }
    if fx.mem.read_u8(D + FLASH) == 1 {
        flash(fx, f);
    }
    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xD0);
    if n == 0x40 {
        ret = 1;
    } else if n == 0x4A {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x8B {
        ret = 8;
    }
    fx.mem.set32(sp + 0xD0, ret);
    ret
}

/// A puff record (D+0x60 + 0xD0 k: the node points +0x10, 4 cells and 0.3
/// vectors a node +8 / +0x30, a 0.7 vector a node +0xB0) [F b156
/// 0x1E00318..0x1E00418].
fn puff_spawn(fx: &mut Fx, k: u32) {
    let r = D + PUFFS + 0xD0 * k;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    for j in 0..2u32 {
        words(fx, r + 0x10 + 0x10 * j, D + 0x40 + 0x10 * j);
        fx.random_table_vec(r + 0xB0 + 0x10 * j, f32::from_bits(0x3F33_3333));
        for s in 0..4u32 {
            let c = rnd(fx, r - PUFFS + 4 * j + s, 0x01E0_03B8);
            fx.mem.set8(r + 8 + 4 * j + s, rem2(c, 4));
            fx.random_table_vec(r + 0x30 + 0x40 * j + 0x10 * s, f32::from_bits(0x3E99_999A));
        }
    }
}

/// The 32 puff records [F b156 0x1E00418..0x1E00A50]: per node four grey
/// (64, 64, 64) sprites 2.5 (0.5 + age / 64) wide at alpha 96 cos(age /
/// 128 turn) and one red (FF, 12, 0E) glow 5 (0.5 + age / 64) wide at 128
/// cos, rising 15 age / 32 along their vectors x age / 32, 0x21 updates.
fn puffs(fx: &mut Fx, f: &Fr) {
    let d = f.d;
    sprite_head(fx, f);
    camera(fx, f);
    for k in 0..0x20u32 {
        let r = D + PUFFS + 0xD0 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        tex_alpha(fx, d, 0x10, 0);
        let age = fx.mem.i32(r + 4);
        let a = cos_alpha(fx, age, 5, 96.0, 0x01E0_04F8);
        colour(fx, d, [0x64; 3], a);
        let age = fx.mem.i32(r + 4);
        corners(fx, f, half_grow(2.5, age, 32.0));
        let age = fx.mem.i32(r + 4);
        let f21 = ee::div(ee::from_i32(age), 32.0);
        let f20 = ee::mul(15.0, f21);
        for j in 0..2u32 {
            for s in 0..4u32 {
                let c = u32::from(fx.mem.read_u8(r + 8 + 4 * j + s));
                cell_uvs(fx, f, c);
                place_dy(fx, f, r + 0x30 + 0x40 * j + 0x10 * s, f21, f20, r + 0x10 + 0x10 * j);
                draw(fx, f, 0x01E0_0754);
            }
        }
        tex_alpha(fx, d, 0x14, 1);
        let age = fx.mem.i32(r + 4);
        let a = cos_alpha(fx, age, 5, 128.0, 0x01E0_0808);
        colour(fx, d, [0xFF, 0x12, 0x0E], a);
        uvs(fx, d, 0, 0, 0x800, 0x800);
        let age = fx.mem.i32(r + 4);
        corners(fx, f, half_grow(5.0, age, 32.0));
        let age = fx.mem.i32(r + 4);
        let f21 = ee::div(ee::from_i32(age), 32.0);
        let f20 = ee::mul(15.0, f21);
        for j in 0..2u32 {
            place_dy(fx, f, r + 0xB0 + 0x10 * j, f21, f20, r + 0x10 + 0x10 * j);
            draw(fx, f, 0x01E0_09D8);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// On 0x32 the dust (D+0x1A60: the node points +0x50, 32 cells a node +8,
/// 0.7 vectors with z halved a node +0x70) [F b156 0x1E00A64..0x1E00B48].
fn dust_spawn(fx: &mut Fx, sp: u32) {
    let s0 = sp + 0x4E0;
    let r = D + DUST;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    for j in 0..2u32 {
        words(fx, r + 0x50 + 0x10 * j, D + 0x40 + 0x10 * j);
        for k in 0..0x20u32 {
            let c = rnd(fx, s0, 0x01E0_0AB8);
            fx.mem.set8(r + 8 + 0x20 * j + k, rem2(c, 4));
            let v = r + 0x70 + 0x200 * j + 0x10 * k;
            fx.random_table_vec(v, f32::from_bits(0x3F33_3333));
            let z = ee::mul(fx.mem.f(v + 8), 0.5);
            fx.mem.setf(v + 8, z);
        }
    }
}

/// The dust [F b156 0x1E00B5C..0x1E010DC]: per node 32 grey (40, 40, 40)
/// sprites 4 (0.5 + age / 32) wide at alpha 128 cos(age / 64 turn), reach
/// falloff(age, 16), then a 16-quad ring (inner 10 / outer 20, ST floats,
/// (80, 40, 20) at 48 ramp(age, 16, 8) inside, (FF, 80, 64) at 255 ramp
/// outside) at each node scaled by the falloff, 0x11 updates.
fn dust(fx: &mut Fx, f: &Fr) {
    let (d, r) = (f.d, D + DUST);
    sprite_head(fx, f);
    tex_alpha(fx, d, 0x10, 0);
    camera(fx, f);
    let age = fx.mem.i32(r + 4);
    let a = cos_alpha(fx, age, 4, 128.0, 0x01E0_0C0C);
    colour(fx, d, [0x40; 3], a);
    let age = fx.mem.i32(r + 4);
    corners(fx, f, half_grow(4.0, age, 16.0));
    let age = fx.mem.i32(r + 4);
    let f20 = fx.tan_falloff_p(age, 0x10, 1);
    for j in 0..2u32 {
        for k in 0..0x20u32 {
            let c = u32::from(fx.mem.read_u8(r + 8 + 0x20 * j + k));
            cell_uvs(fx, f, c);
            place(fx, f, r + 0x70 + 0x200 * j + 0x10 * k, f20, r + 0x50 + 0x10 * j);
            draw(fx, f, 0x01E0_0E14);
        }
    }
    let q = f.q;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    fx.desc_clamp(q, 5);
    tex_alpha(fx, q, 0x18, 1);
    let age = fx.mem.i32(r + 4);
    let f20 = Fx::ramp_above(ee::from_i32(age), 16.0, 8.0);
    let a = fix(fx, ee::mul(48.0, f20), 0x01E0_0F10);
    rgba(fx, q + 0x10, [0x80, 0x40, 0x20], a as u8);
    copy8(fx, q + 0x28, q + 0x10);
    let a = fix(fx, ee::mul(255.0, f20), 0x01E0_0F5C);
    rgba(fx, q + 0x40, [0xFF, 0x80, 0x64], a as u8);
    copy8(fx, q + 0x58, q + 0x40);
    let age = fx.mem.i32(r + 4);
    let tf = fx.tan_falloff_p(age, 0x10, 1);
    vec4(fx, f.pl, [tf.to_bits(), tf.to_bits(), 0, ONE]);
    for j in 0..2u32 {
        fx.camera_vp_to(f.vp);
        let p = r + 0x50 + 0x10 * j;
        let t = [fx.mem.f(p), fx.mem.f(p + 4), fx.mem.f(p + 8)];
        let m = fx.trs(None, None, Some(t));
        fx.write_m4(f.model, &m);
        fx.scale_rows(f.model, f.pl);
        let mm = fx.read_m4(f.model);
        fx.mul_into(Some(f.vp), &mm);
        for k in 0..0x10u32 {
            for (o, x) in [(0x18u32, 0x3DCC_CCCDu32), (0x1C, 0), (0x30, 0x3F66_6666), (0x34, 0), (0x48, 0), (0x4C, ONE), (0x60, ONE), (0x64, ONE)] {
                fx.mem.write_u32(q + o, x);
            }
            let a = D + RING + 0x20 * k;
            if fx.project_n(&[a, a + 0x20, a + 0x10, a + 0x30], f.vp, q, f.mean, 0x01E0_106C) == 1 {
                let w = fx.mem.f(f.mean);
                let key = fix(fx, w, 0x01E0_1080);
                fx.submit(q, OT, key as i32);
            }
        }
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x11 {
        fx.mem.write_u8(r, 0);
    }
}

/// On 0x32 the two cards (D+0x20F0: cells rand % 3 +8, matrices turned
/// about z by a random angle +0x10, the node points +0x90) [F b156
/// 0x1E010F0..0x1E011A0].
fn cards_spawn(fx: &mut Fx, sp: u32) {
    let s0 = sp + 0x4E0;
    let r = D + CARDS;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    for j in 0..2u32 {
        let c = rnd(fx, s0, 0x01E0_1114);
        let (_, h) = fx.div(c, 3);
        let m = r + 0x10 + 0x40 * j;
        fx.identity(m);
        fx.mem.set8(r + 8 + j, h);
        let a = rand_angle(fx, s0, 0x01E0_1140);
        fx.rotate_axis(m, a, 2);
        words(fx, r + 0x90 + 0x10 * j, D + 0x40 + 0x10 * j);
    }
}

/// The two cards [F b156 0x1E011B4..0x1E01618]: a 3-wide textured quad
/// (texture D+0x1C, grey 80 at alpha 80, its cell of a 4-wide strip),
/// turned by its matrix then the camera turn, moving from its node
/// toward the target by age / 24 in y / z (x doubled, as the code adds
/// the node's x twice); each update a trail record (D+0x21A0 + 0x1400 j +
/// 0xA0 (age % 32): the card's point +0x10, 8 cells +8, 0.7 vectors
/// +0x20), 0x19 updates.
fn cards(fx: &mut Fx, f: &Fr) {
    let s0 = f.sp + 0x4E0;
    let (g, r) = (f.g, D + CARDS);
    head(fx, g, 0x24, 9, 0x4_2424_2421, 0x800);
    fx.desc_clamp(g, 0);
    tex_alpha(fx, g, 0x1C, 0);
    camera(fx, f);
    rgba(fx, g + 0x10, [0x80; 3], 0x80);
    let age = fx.mem.i32(r + 4);
    let f21 = ee::div(ee::from_i32(age), 24.0);
    for j in 0..2u32 {
        corners4(fx, f, 3.0);
        let c = i32::from(fx.mem.read_u8(r + 8 + j));
        strip_uvs(fx, g, c);
        let p = r + 0x90 + 0x10 * j;
        fx.vu_sub4(f.pl, D + 0x20, p);
        let x = fx.mem.f(p);
        fx.mem.setf(f.pl, x);
        let y = ee::mul(fx.mem.f(f.pl + 4), f21);
        fx.mem.setf(f.pl + 4, y);
        let z = ee::mul(fx.mem.f(f.pl + 8), f21);
        fx.mem.setf(f.pl + 8, z);
        fx.vu_add4(f.pl, f.pl, p);
        let m = r + 0x10 + 0x40 * j;
        for k in 0..4u32 {
            let c = f.c0 + 0x10 * k;
            fx.vu_apply_to(c, m, c);
            fx.vu_apply_to(c, f.turn, c);
            fx.vu_add4(f.v0 + 0x10 * k, c, f.pl);
        }
        draw_at(fx, f, f.v0, g, 0x01E0_14E4);
        let age = fx.mem.i32(r + 4);
        let t = D + TRAILS + 0x1400 * j + 0xA0 * rem2(age, 0x20) as u32;
        fx.mem.write_u8(t, 1);
        fx.mem.write_u32(t + 4, 0);
        words(fx, t + 0x10, f.pl);
        for s in 0..8u32 {
            let c = rnd(fx, s0, 0x01E0_1568);
            fx.mem.set8(t + 8 + s, rem2(c, 4));
            fx.random_table_vec(t + 0x20 + 0x10 * s, f32::from_bits(0x3F33_3333));
        }
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x19 {
        fx.mem.write_u8(r, 0);
    }
}

/// The 2 x 32 trail records [F b156 0x1E01618..0x1E0198C]: eight (80, 20,
/// 18) sprites 5 (0.5 + (32 - age) / 64) wide at alpha 128 cos(age / 128
/// turn), reach 0.2 + 0.8 falloff(age, 32), 0x21 updates.
fn trails(fx: &mut Fx, f: &Fr) {
    let d = f.d;
    sprite_head(fx, f);
    tex_alpha(fx, d, 0x10, 1);
    camera(fx, f);
    for j in 0..2u32 {
        for m in 0..0x20u32 {
            let r = D + TRAILS + 0x1400 * j + 0xA0 * m;
            if fx.mem.read_u8(r) == 0 {
                continue;
            }
            let age = fx.mem.i32(r + 4);
            let a = cos_alpha(fx, age, 5, 128.0, 0x01E0_16E4);
            colour(fx, d, [0x80, 0x20, 0x18], a);
            let age = fx.mem.i32(r + 4);
            corners(fx, f, half_grow(5.0, 0x20 - age, 32.0));
            let age = fx.mem.i32(r + 4);
            let tf = fx.tan_falloff_p(age, 0x20, 1);
            let f20 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf));
            for k in 0..8u32 {
                let c = u32::from(fx.mem.read_u8(r + 8 + k));
                cell_uvs(fx, f, c);
                place(fx, f, r + 0x20 + 0x10 * k, f20, r + 0x10);
                draw(fx, f, 0x01E0_18FC);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x21 {
                fx.mem.write_u8(r, 0);
            }
        }
    }
}

/// On 0x4A the 256 shards (D+0x49A0: cells rand % 3 +8, unit quads
/// turned about z by a random angle +0x110 + 0x40 k, 3.0 vectors +0x4110)
/// [F b156 0x1E0199C..0x1E01AD8].
fn shards_spawn(fx: &mut Fx, f: &Fr) {
    let s0 = f.sp + 0x4E0;
    let r = D + SHARDS;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    let (n, p) = (0xBF00_0000u32, 0x3F00_0000u32);
    for k in 0..0x100u32 {
        let c = rnd(fx, s0, 0x01E0_19D4);
        let (_, h) = fx.div(c, 3);
        fx.identity(f.m);
        fx.mem.set8(r + 8 + k, h);
        let a = rand_angle(fx, s0, 0x01E0_1A00);
        fx.rotate_axis(f.m, a, 2);
        let q = r + 0x110 + 0x40 * k;
        for (s, (x, y)) in [(n, n), (p, n), (n, p), (p, p)].into_iter().enumerate() {
            vec4(fx, q + 0x10 * s as u32, [x, y, 0, ONE]);
        }
        for s in 0..4u32 {
            fx.vu_apply_to(q + 0x10 * s, f.m, q + 0x10 * s);
        }
        fx.random_table_vec(r + 0x4110 + 0x10 * k, 3.0);
    }
}

/// The shards [F b156 0x1E01AEC..0x1E01E88]: textured quads (texture
/// D+0x1C, grey at alpha 128 ramp(age, 64, 32)) of a 4-wide strip, turned
/// with the camera, reach 0.1 + 0.9 falloff(age, 64) dropping 5 (0.5
/// (-0.0109 age^2)) from the target, 0x41 updates.
fn shards(fx: &mut Fx, f: &Fr) {
    let (g, r) = (f.g, D + SHARDS);
    head(fx, g, 0x24, 9, 0x4_2424_2421, 0x800);
    tex_alpha(fx, g, 0x1C, 0);
    fx.desc_clamp(g, 5);
    camera(fx, f);
    let age = fx.mem.i32(r + 4);
    let rp = Fx::ramp_above(ee::from_i32(age), 64.0, 32.0);
    let a = fix(fx, ee::mul(128.0, rp), 0x01E0_1BA0);
    rgba(fx, g + 0x10, [0x80; 3], a as u8);
    let age = fx.mem.i32(r + 4);
    let tf = fx.tan_falloff_p(age, 0x40, 1);
    let f20 = tenth(tf);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let p2 = Fx::powf2(age);
    let f21 = ee::mul(5.0, ee::mul(0.5, ee::mul(f32::from_bits(0xBC32_864D), p2)));
    for k in 0..0x100u32 {
        let c = i32::from(fx.mem.read_u8(r + 8 + k));
        strip_uvs(fx, g, c);
        fx.vu_scale4(f.pl, r + 0x4110 + 0x10 * k, f20);
        let y = ee::add(fx.mem.f(f.pl + 4), f21);
        fx.mem.setf(f.pl + 4, y);
        fx.vu_add4(f.pl, f.pl, D + 0x20);
        let q = r + 0x110 + 0x40 * k;
        for s in 0..4u32 {
            let c = f.c0 + 0x10 * s;
            fx.vu_apply_to(c, f.turn, q + 0x10 * s);
            fx.vu_add4(f.v0 + 0x10 * s, c, f.pl);
        }
        draw_at(fx, f, f.v0, g, 0x01E0_1E18);
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x41 {
        fx.mem.write_u8(r, 0);
    }
}

/// The 256 sparks at the target (D+0x9AB0: cells +8, 1.2 vectors +0x110)
/// [F b156 0x1E01F20..0x1E022F4]: 10 wide, every third grey (preset 2, 0.8
/// x the reach, alpha 100 ramp) else (80, 38, 26) at 128 ramp(age, 32, 16),
/// reach 0.1 + 0.9 falloff(age, 32), 0x21 updates.
fn sparks(fx: &mut Fx, f: &Fr) {
    let (d, r) = (f.d, D + SPARKS);
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, f);
    corners(fx, f, 5.0);
    let age = fx.mem.i32(r + 4);
    let tf = fx.tan_falloff_p(age, 0x20, 1);
    let f24 = tenth(tf);
    let age = fx.mem.i32(r + 4);
    let rp = Fx::ramp_above(ee::from_i32(age), 32.0, 16.0);
    let f21 = ee::mul(f32::from_bits(0x3F4C_CCCD), f24);
    let f22 = ee::mul(100.0, rp);
    let f23 = ee::mul(128.0, rp);
    for k in 0..0x100u32 {
        let f20 = if fx.div(k as i32, 3).1 == 0 {
            alpha(fx, d, 2);
            let a = fix(fx, f22, 0x01E0_20DC);
            colour(fx, d, [0x80; 3], a as u8);
            f21
        } else {
            alpha(fx, d, 1);
            let a = fix(fx, f23, 0x01E0_2120);
            colour(fx, d, [0x80, 0x38, 0x26], a as u8);
            f24
        };
        let c = u32::from(fx.mem.read_u8(r + 8 + k));
        cell_uvs(fx, f, c);
        place(fx, f, r + 0x110 + 0x10 * k, f20, D + 0x20);
        draw(fx, f, 0x01E0_2284);
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(r, 0);
    }
}

/// The frame flash (D+0xABC0) [F b156 0x1E02328..0x1E02598]: the frame
/// copied (0x207F50 into sp + 0x4F8 / 0x500) and drawn as eight
/// full-window sprites zoomed in 34 (s + 1) / 8 texels, grey at alpha 32
/// (0x1C - age) / 28, 0x1D updates.
fn flash(fx: &mut Fx, f: &Fr) {
    let (sp, d) = (f.sp, f.d);
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 0);
    fx.desc_clamp(d, 5);
    let a = clamp_regs(fx, d, 5);
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x4F8, sp + 0x500);
    fx.texflush(regs);
    let age = fx.mem.i32(D + FLASH + 4);
    let x = (0x1C - age) << 5;
    fx.mult(0x9249_2493u32 as i32, x);
    let al = ((fx.hilo().1.wrapping_add(x)) >> 4).wrapping_add(((x as u32) >> 31) as i32);
    rgba(fx, d + 0x10, [0x80; 3], al as u8);
    far_xy(fx, d + 0x20, 0x7000, 0x7000);
    far_xy(fx, d + 0x30, 0x9000, 0x9000);
    for s in 0..8i32 {
        let v = ((s + 1) * 34) >> 3;
        let lo = ((v << 4) & 0x3FFF) as u32;
        let hi = (((0x200 - v) << 4) & 0x3FFF) as u32;
        uvs(fx, d, lo, lo, hi, hi);
        fx.submit(d, 0, 0);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + FLASH + 4) + 1;
    fx.mem.set32(D + FLASH + 4, age);
    if age >= 0x1D {
        fx.mem.write_u8(D + FLASH, 0);
    }
}
