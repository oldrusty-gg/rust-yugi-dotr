//! BMODULE b528 (card 620), hand-ported from the record (linked at
//! 0x1E00000; one 2,848-word function 0x1E00080 in a 0x5A0 frame with D
//! spilled to sp + 0xF0 and the OT in s0, data area a0 = 0x1ED0000, 0x7C20
//! bytes) [F]. Written fresh from the raw listing
//! (the project notes). Slots: the camera sp + 0x380 (model + 0x40,
//! turn + 0x80), corners sp + 0x440 / 0x450, vertices sp + 0x480 / 0x490,
//! the placed point sp + 0x4A0, the quad descriptor sp + 0x4B0, the sprite
//! descriptor sp + 0x540, the frame texture sp + 0x528 / 0x530, the mean w
//! sp + 0x59C. Init: nodes 0xC (clear) / 0x16 / 0x20 / 0x2B / 0x30, the
//! target's position D+0xA0, the unit radius D+0xB0, a ring of 17 (inner
//! 15, outer 40, middle) points at D+0x2E80. Every update the camera turn
//! with roll, the nodes (D+0x30.. / D+0x70..) and the unit's point D+0x90.
//! On 0x55 + 30 n (n < 5) a head (D+0xC0, three points flying from the
//! nodes to the target over 0x19 updates, dropping two trails: 3 x 8 x
//! 0x60 at D+0x130, 3 x 32 x 0x60 at D+0xA30) and a mark (D+0x2E40); on
//! age 0x17 a band record (3 x 48 x 0x60 at D+0x31C0: two ring bands and a
//! sprite, 0x31 updates). On 0x31 a cloud (D+0x67C0, 2 x 64 sparks). On
//! 9..0x10B a wave record a update (48 x 0x2C at D+0x73E0: a ring of nine
//! rays round the unit in a packet layer with the attacker drawn, frame
//! UVs). The stage dims. Returns 1 on 0x63, 6 on 0x6E..0x114, 2 on 0x115,
//! 8 once the count reaches 0x136. b592 (card 598) is its sibling
//! ([`program_b592`]): one node, three heads, no cloud, in a 0x590 frame
//! whose slots are these less 0x10 (`s` = sp - 0x10 below).
use crate::b182::{alpha, head, rgba};
use crate::b440::{copy8, copy_z, fix, rem2, words};
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};
use crate::fxlib2::project;

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
const ONE: u32 = 0x3F80_0000;
const QUAD_REGS: u64 = 0x4214_2142_1421;
const WAVE_CLAMP: u64 = 0x0800_0080_000A;

const VP: u32 = 0x380;
const MODEL: u32 = 0x3C0;
const TURN: u32 = 0x400;
const C0: u32 = 0x440;
const C1: u32 = 0x450;
const V0: u32 = 0x480;
const V1: u32 = 0x490;
const PL: u32 = 0x4A0;
const Q: u32 = 0x4B0;
const S: u32 = 0x540;
const MEAN: u32 = 0x59C;

const HEAD: u32 = D + 0xC0;
const TRAIL_A: u32 = D + 0x130;
const TRAIL_B: u32 = D + 0xA30;
const MARK: u32 = D + 0x2E40;
const RING: u32 = D + 0x2E80;
const BAND_IDX: u32 = D + 0x31B0;
const BANDS: u32 = D + 0x31C0;
const CLOUD: u32 = D + 0x67C0;
const WAVE_IDX: u32 = D + 0x7070;
const RAYS: u32 = D + 0x7080;
const WAVES: u32 = D + 0x73E0;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x5A0);
    fx.mem.write_u32(sp + 0xF0, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(0x5A0);
    ret
}

/// rand >> 23 from `ra` (s0 = the OT).
fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(OT, ra) >> 23
}

/// 0x1A7230 fabsf (its argument left at its sp).
fn fabsf(fx: &mut Fx, x: f32) -> f32 {
    fx.mem.write_f32(fx.sp - 0x10, x);
    f32::from_bits(x.to_bits() & 0x7FFF_FFFF)
}

fn sin16(fx: &mut Fx, a: i32) -> f32 {
    fx.table_trig(a, false)
}

fn cos16(fx: &mut Fx, a: i32) -> f32 {
    fx.table_trig(a, true)
}

/// (x, y, 0, 1) at `p`.
fn vec2(fx: &mut Fx, p: u32, x: f32, y: f32) {
    fx.mem.setf(p, x);
    fx.mem.setf(p + 4, y);
    fx.mem.write_u32(p + 8, 0);
    fx.mem.write_u32(p + 0xC, ONE);
}

fn init(fx: &mut Fx) -> i32 {
    // The flags and counters the record clears after this are all inside it.
    fx.bzero(D, 0x7C20);
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
    for (at, node, clear) in [(0x1Cu32, 0xC, 1), (0x20, 0x16, 0), (0x24, 0x20, 0), (0x60, 0x2B, 0), (0x64, 0x30, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0xA0);
    let h = fx.mem.i32(D + 4);
    let r = crate::blib::afterimage::unit_radius(fx, h);
    fx.mem.setf(D + 0xB0, r);
    for k in 0..0x11u32 {
        let a = ((k << 16) as i32) >> 4;
        let p = RING + 0x30 * k;
        let s = ee::mul(15.0, sin16(fx, a));
        let c = ee::mul(15.0, cos16(fx, a));
        vec2(fx, p, s, c);
        let s = ee::mul(40.0, sin16(fx, a));
        let c = ee::mul(40.0, cos16(fx, a));
        vec2(fx, p + 0x10, s, c);
        fx.vu_add4(p + 0x20, p, p + 0x10);
        fx.vu_scale4(p + 0x20, p + 0x20, 0.5);
    }
    0
}

/// The sprite descriptor: prim 0x26 | 0x800, ALPHA preset 1, texture D +
/// `tex`, clamp 5.
fn sprite_desc(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + S;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 1);
    let t = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, t, 1);
    fx.desc_clamp(d, 5);
}

/// The camera's view-projection x the identity.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MODEL);
    mul_mem(fx, sp + VP, sp + MODEL);
}

/// The sprite's UV rectangle.
fn uvs(fx: &mut Fx, sp: u32, u0: i32, v0: i32, u1: i32, v1: i32) {
    for (at, v) in [(0x558u32, u0), (0x55A, v0), (0x568, u1), (0x56A, v1)] {
        let h = fx.mem.read_u16(sp + at);
        fx.mem.write_u16(sp + at, (h & 0xC000) | (v & 0x3FFF) as u16);
    }
}

/// The 32 x 32 cell `b` of a 4-wide sheet.
fn cell(fx: &mut Fx, sp: u32, b: i32) {
    let u = rem2(b, 4) << 5;
    let v = (b >> 2) << 5;
    uvs(fx, sp, u << 4, v << 4, (u + 0x20) << 4, (v + 0x20) << 4);
}

/// Corners (-s, -s) / (s, s), turned to the camera when `turned`.
fn corners(fx: &mut Fx, sp: u32, s: f32, turned: bool) {
    let n = ee::neg(s);
    vec2(fx, sp + C0, n, n);
    vec2(fx, sp + C1, s, s);
    if turned {
        fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
        fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
    }
}

/// The two vertices at the placed point.
fn vertices(fx: &mut Fx, sp: u32) {
    fx.vu_add4(sp + V0, sp + C0, sp + PL);
    fx.vu_add4(sp + V1, sp + C1, sp + PL);
}

/// The vertices projected and submitted at their mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    if project(fx, sp + V0, PROJ_OUT, sp + VP, sp + S, sp + MEAN) {
        let w = fx.mem.f(sp + MEAN);
        let key = fix(fx, w, ra);
        fx.submit(sp + S, OT, key as i32);
    }
}

/// The far Z on the XYZ2 at `first`, copied to `rest`.
fn far(fx: &mut Fx, first: u32, rest: &[u32]) {
    let x = fx.mem.read_u64(first);
    fx.mem.write_u64(first, (x & KEEP) | FAR_Z);
    for &r in rest {
        copy_z(fx, r, first);
    }
}

/// The sprite's two XYZ2 at far Z, submitted at key 0.
fn draw_far(fx: &mut Fx, sp: u32) {
    far(fx, sp + 0x560, &[sp + 0x570]);
    fx.submit(sp + S, OT, 0);
}

/// The quad's four XYZ2 at far Z.
fn quad_far(fx: &mut Fx, sp: u32) {
    far(fx, sp + 0x4D0, &[sp + 0x4E8, sp + 0x500, sp + 0x518]);
}

/// The quad's four ST pairs.
fn sts(fx: &mut Fx, sp: u32, s0: f32, s1: f32, t0: f32, t1: f32) {
    for (at, s, t) in [(0x4C8u32, s0, t0), (0x4E0, s1, t0), (0x4F8, s0, t1), (0x510, s1, t1)] {
        fx.mem.setf(sp + at, s);
        fx.mem.setf(sp + at + 4, t);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    for k in 0..3u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x1C + 4 * k);
        fx.node_position(h, s, D + 0x30 + 0x10 * k);
    }
    for k in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x60 + 4 * k);
        fx.node_position(h, s, D + 0x70 + 0x10 * k);
    }
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x90);
    let n = fx.mem.i32(D + 0xB4);
    if n < 5 && fx.mem.i32(D) == 30 * n + 0x55 {
        spawn_head(fx, sp);
    }
    sprite_desc(fx, sp, 0x10);
    camera(fx, sp);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    corners(fx, sp, 10.0, true);
    if fx.mem.read_u8(HEAD) != 0 {
        heads(fx, sp);
    }
    sprite_desc(fx, sp, 0x14);
    camera(fx, sp);
    for k in 0..24u32 {
        let t = TRAIL_A + 0x60 * k;
        if fx.mem.read_u8(t) != 0 {
            trail(fx, sp, t, 8, 10.5, [0xFF, 0x64, 0x3C], |x| (x << 7) >> 3, 0x3ECC_CCCD, 0x3F19_999A, 0x01E0_0F28);
        }
    }
    sprite_desc(fx, sp, 0x10);
    camera(fx, sp);
    for k in 0..96u32 {
        let t = TRAIL_B + 0x60 * k;
        if fx.mem.read_u8(t) != 0 {
            trail(fx, sp, t, 0x20, 6.5, [0xFF, 0x50, 0x36], |x| (200 * x) >> 5, 0x3E4C_CCCD, 0x3F4C_CCCD, 0x01E0_12C8);
        }
    }
    sprite_desc(fx, sp, 0x10);
    camera(fx, sp);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    corners(fx, sp, 50.0, true);
    if fx.mem.read_u8(MARK) != 0 {
        mark(fx, sp, MARK, 8, 3, [0xFF, 0x48, 0x22], 0x01E0_1514);
    }
    let q = sp + Q;
    head(fx, q, 0x34, 0xC, QUAD_REGS, 0);
    alpha(fx, q, 1);
    let t = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(q, t, 1);
    fx.desc_clamp(q, 0);
    sprite_desc(fx, sp, 0x10);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    corners(fx, sp, 60.0, false);
    for k in 0..0x90u32 {
        let r = BANDS + 0x60 * k;
        if fx.mem.read_u8(r) != 0 {
            band(fx, sp, r);
        }
    }
    if fx.mem.i32(D) == 0x31 {
        spawn_cloud(fx);
    }
    if fx.mem.read_u8(CLOUD) == 1 {
        cloud(fx, sp);
    }
    let c = fx.mem.i32(D);
    if (9..0x10C).contains(&c) {
        spawn_wave(fx);
    }
    waves(fx, sp);
    let c = fx.mem.i32(D);
    let a = Fx::clamp_i(0x135 - c, 0, 0x20);
    let b = Fx::clamp_i(c, 0, 0x20);
    let l = (a.wrapping_mul((b - (b << 2)) << 5) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);
    let c = fx.mem.i32(D);
    if c == 0x63 {
        fx.mem.write_u32(sp + 0xC0, 1);
    } else {
        if (0x6E..0x115).contains(&c) {
            fx.mem.write_u32(sp + 0xC0, 6);
        }
        if c == 0x115 {
            fx.mem.write_u32(sp + 0xC0, 2);
        }
    }
    let n = fx.mem.i32(D) + 1;
    fx.mem.set32(D, n);
    if fx.mem.i32(D) >= 0x136 {
        fx.mem.write_u32(sp + 0xC0, 8);
    }
    fx.mem.i32(sp + 0xC0)
}

/// [F 0x1E004F4] The head and the mark at the three nodes; each point's
/// flight vector = (target + a 0.5 random vector) - the node, x kept.
fn spawn_head(fx: &mut Fx, sp: u32) {
    let pl = sp + PL;
    fx.mem.write_u8(HEAD, 1);
    fx.mem.write_u32(HEAD + 4, 0);
    fx.mem.write_u8(MARK, 1);
    fx.mem.write_u32(MARK + 4, 0);
    for i in 0..3u32 {
        let node = D + 0x30 + 0x10 * i;
        let at = HEAD + 0x10 + 0x10 * i;
        words(fx, at, node);
        words(fx, MARK + 0x10 + 0x10 * i, node);
        fx.random_table_vec(pl, 0.5);
        fx.vu_add4(pl, pl, D + 0xA0);
        let x = fx.mem.read_u32(at);
        fx.mem.write_u32(pl, x);
        fx.vu_sub4(at + 0x30, pl, at);
    }
    for at in [D + 0xB4, D + 0xB8] {
        let v = fx.mem.i32(at) + 1;
        fx.mem.set32(at, v);
    }
    if fx.mem.i32(D + 0xB8) > 0 {
        fx.mem.write_u32(D + 0xB8, 0);
    }
    let v = fx.mem.i32(D + 0x2E30) + 1;
    fx.mem.set32(D + 0x2E30, v);
    if v > 0 {
        fx.mem.write_u32(D + 0x2E30, 0);
    }
}

/// [F 0x1E007D4] The head's three sprites, each update leaving a trail A
/// (index +8 % 8) and a trail B (index +0xC % 32) entry; on age 0x17 a
/// band record with a random turn.
fn heads(fx: &mut Fx, sp: u32) {
    let pl = sp + PL;
    rgba(fx, sp + 0x550, [0xFF, 0x80, 0x50], 0xFF);
    let f20 = ee::div(ee::from_i32(fx.mem.i32(HEAD + 4)), 24.0);
    let ia = fx.mem.i32(HEAD + 8) as u32;
    let v = ia as i32 + 1;
    fx.mem.set32(HEAD + 8, if v < 8 { v } else { 0 });
    let mut ib = fx.mem.i32(HEAD + 0xC) as u32;
    let v = ib as i32 + 1;
    fx.mem.set32(HEAD + 0xC, if v < 0x20 { v } else { 0 });
    for i in 0..3u32 {
        let at = HEAD + 0x10 + 0x10 * i;
        fx.vu_scale4(pl, at + 0x30, f20);
        fx.vu_add4(pl, pl, at);
        vertices(fx, sp);
        draw(fx, sp, 0x01E0_0928);
        let t = TRAIL_A + 0x300 * i + 0x60 * ia;
        fx.mem.write_u8(t, 1);
        fx.mem.write_u32(t + 4, 0);
        words(fx, t + 0x10, pl);
        for k in 0..4u32 {
            let r = rnd(fx, 0x01E0_0988);
            fx.mem.set8(t + 8 + k, rem2(r, 8));
            fx.random_table_vec(t + 0x20 + 0x10 * k, 0.5);
        }
        let t = TRAIL_B + 0xC00 * i + 0x60 * ib;
        fx.mem.write_u8(t, 1);
        fx.mem.write_u32(t + 4, 0);
        words(fx, t + 0x10, pl);
        for k in 0..4u32 {
            let r = rnd(fx, 0x01E0_0A20);
            let (_, m) = fx.div(r, 3);
            fx.mem.set8(t + 8 + k, m);
            fx.random_table_vec(t + 0x20 + 0x10 * k, 1.0);
        }
        if fx.mem.i32(HEAD + 4) == 0x17 {
            // fp, trail B's index, is the band's index from here on.
            ib = spawn_band(fx, sp, i);
        }
    }
    let n = fx.mem.i32(HEAD + 4) + 1;
    fx.mem.set32(HEAD + 4, n);
    if n >= 0x19 {
        fx.mem.write_u8(HEAD, 0);
    }
}

/// A band's matrix: a random x / y / z turn.
fn band_turn(fx: &mut Fx, r: u32, ras: [u32; 3]) {
    let m = r + 0x20;
    fx.identity(m);
    for (axis, sh) in [(0, 15), (1, 15), (2, 16)] {
        let v = rnd(fx, ras[axis as usize]);
        fx.rotate_axis(m, (rem2(v, 256) << sh) >> 8, axis);
    }
}

/// [F 0x1E00A6C] Band record `D[0x31B0 + 4 i]` of group `i` at the placed
/// point, its matrix a random x / y / z turn.
fn spawn_band(fx: &mut Fx, sp: u32, i: u32) -> u32 {
    let idx = BAND_IDX + 4 * i;
    let n = fx.mem.i32(idx) as u32;
    let r = BANDS + 0x1200 * i + 0x60 * n;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    words(fx, r + 0x10, sp + PL);
    band_turn(fx, r, [0x01E0_0AC8, 0x01E0_0AFC, 0x01E0_0B30]);
    let v = fx.mem.i32(idx) + 1;
    fx.mem.set32(idx, if v < 0x30 { v } else { 0 });
    n
}

/// [F 0x1E00CFC / 0x1E0108C] A trail entry: 4 sprites `rgb` at alpha `a`,
/// `w` (0.5 + (n - age) / 2 n) wide, out by `lo` + `hi` falloff of age / n,
/// n + 1 updates.
#[allow(clippy::too_many_arguments)]
fn trail(fx: &mut Fx, sp: u32, t: u32, n: i32, w: f32, rgb: [u8; 3], a: fn(i32) -> i32, lo: u32, hi: u32, ra: u32) {
    let pl = sp + PL;
    let age = fx.mem.i32(t + 4);
    let f1 = ee::mul(0.5, ee::from_i32(n - age));
    let s = ee::mul(w, ee::add(0.5, ee::div(f1, ee::from_i32(n))));
    corners(fx, sp, s, true);
    let age = fx.mem.i32(t + 4);
    rgba(fx, sp + 0x550, rgb, a(n - age) as u8);
    let f0 = fx.tan_falloff_p(age, n, 1);
    let f20 = ee::add(f32::from_bits(lo), ee::mul(f32::from_bits(hi), f0));
    for k in 0..4u32 {
        let b = i32::from(fx.mem.read_u8(t + 8 + k));
        cell(fx, sp, b);
        fx.vu_scale4(pl, t + 0x20 + 0x10 * k, f20);
        fx.vu_add4(pl, pl, t + 0x10);
        vertices(fx, sp);
        draw(fx, sp, ra);
    }
    let v = fx.mem.i32(t + 4) + 1;
    fx.mem.set32(t + 4, v);
    if v > n {
        fx.mem.write_u8(t, 0);
    }
}

/// [F 0x1E014E4] The mark: a sprite at each of its `pts` points, `rgb` at
/// 255 (1 - falloff of age / n), far Z, n + 1 updates.
fn mark(fx: &mut Fx, sp: u32, mk: u32, n: i32, pts: u32, rgb: [u8; 3], ra: u32) {
    let age = fx.mem.i32(mk + 4);
    let f0 = fx.tan_falloff_p(age, n, 1);
    let a = fix(fx, ee::mul(255.0, ee::sub(1.0, f0)), ra);
    rgba(fx, sp + 0x550, rgb, a as u8);
    for i in 0..pts {
        words(fx, sp + PL, mk + 0x10 + 0x10 * i);
        vertices(fx, sp);
        if project(fx, sp + V0, PROJ_OUT, sp + VP, sp + S, sp + MEAN) {
            draw_far(fx, sp);
        }
    }
    let v = fx.mem.i32(mk + 4) + 1;
    fx.mem.set32(mk + 4, v);
    if v > n {
        fx.mem.write_u8(mk, 0);
    }
}

/// [F 0x1E01840] A band record: two passes (half turn apart) of the ring
/// as textured quads at its matrix, scaled 0.1 + 0.9 falloff, then a
/// sprite at its point; 0x31 updates.
fn band(fx: &mut Fx, sp: u32, r: u32) {
    let (vp, m) = (sp + VP, sp + MODEL);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let f0 = Fx::ramp_above(age, 48.0, 24.0);
    rgba(fx, sp + 0x4C0, [0x40, 0x10, 8], 0);
    copy8(fx, sp + 0x4D8, sp + 0x4C0);
    let a = fix(fx, ee::mul(255.0, f0), 0x01E0_18AC);
    rgba(fx, sp + 0x4F0, [0xFF, 0x50, 0x32], a as u8);
    copy8(fx, sp + 0x508, sp + 0x4F0);
    let mut turn = 0i32;
    for _ in 0..2 {
        fx.camera_vp_to(vp);
        let t = [fx.mem.f(r + 0x10), fx.mem.f(r + 0x14), fx.mem.f(r + 0x18)];
        let tm = fx.trs(None, None, Some(t));
        fx.write_m4(m, &tm);
        mul_mem(fx, m, r + 0x20);
        fx.rotate_axis(m, turn >> 1, 0);
        let age = fx.mem.i32(r + 4);
        let f0 = fx.tan_falloff_p(age, 0x30, 1);
        let f = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), f0));
        vec2(fx, sp + PL, f, f);
        fx.scale_rows(m, sp + PL);
        mul_mem(fx, vp, m);
        band_ring(fx, sp, r, RING, [0x01E0_1AC8, 0x01E0_1C08]);
        turn = turn.wrapping_add(0x8000);
    }
    band_end(fx, sp, r, 128.0, [0xFF, 0x80, 0x50], 0x01E0_1D54);
}

/// The ring as ST quads scrolling by age % 32 under the matrix in `vp`.
fn band_ring(fx: &mut Fx, sp: u32, r: u32, ring: u32, ras: [u32; 2]) {
    let (vp, q) = (sp + VP, sp + Q);
    let v = rem2(fx.mem.i32(r + 4), 32);
    let f21 = ee::div(ee::mul(-1.0, ee::from_i32(v)), 32.0);
    let f22 = ee::add(0.5, f21);
    let f23 = ee::add(1.0, f21);
    for k in 0..0x10u32 {
        let (a, b) = (ring + 0x30 * k, ring + 0x30 * (k + 1));
        let f24 = ee::div(ee::mul(6.0, ee::from_i32(k as i32)), 16.0);
        let f20 = ee::div(ee::mul(6.0, ee::from_i32(k as i32 + 1)), 16.0);
        sts(fx, sp, f24, f20, f21, f22);
        if fx.project_n(&[a, b, a + 0x20, b + 0x20], vp, q, sp + MEAN, ras[0]) == 1 {
            quad_far(fx, sp);
            fx.submit(q, OT, 0);
        }
        sts(fx, sp, f24, f20, f23, f22);
        if fx.project_n(&[a + 0x10, b + 0x10, a + 0x20, b + 0x20], vp, q, sp + MEAN, ras[1]) == 1 {
            quad_far(fx, sp);
            fx.submit(q, OT, 0);
        }
    }
}

/// The band's closing sprite at its point, scaled 0.1 + 0.9 falloff, and
/// its age.
fn band_end(fx: &mut Fx, sp: u32, r: u32, top: f32, rgb: [u8; 3], ra: u32) {
    let (vp, m) = (sp + VP, sp + MODEL);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let f0 = Fx::ramp_above(age, 48.0, 24.0);
    let a = fix(fx, ee::mul(top, f0), ra);
    rgba(fx, sp + 0x550, rgb, a as u8);
    fx.camera_vp_to(vp);
    fx.translate_rows(m, sp + TURN, r + 0x10);
    let age = fx.mem.i32(r + 4);
    let f0 = fx.tan_falloff_p(age, 0x30, 1);
    let f = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), f0));
    fx.mem.setf(sp + PL, f);
    fx.mem.setf(sp + PL + 4, f);
    fx.mem.setf(sp + PL + 8, f);
    fx.mem.write_u32(sp + PL + 0xC, ONE);
    fx.scale_rows(m, sp + PL);
    mul_mem(fx, vp, m);
    if project(fx, sp + C0, PROJ_OUT, vp, sp + S, sp + MEAN) {
        draw_far(fx, sp);
    }
    let n = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, n);
    if n >= 0x31 {
        fx.mem.write_u8(r, 0);
    }
}

/// [F 0x1E01F44] The cloud: 64 sparks at each of the two nodes D+0x70, the
/// vectors of 0.8 with y = |y| / 2.
fn spawn_cloud(fx: &mut Fx) {
    fx.mem.write_u8(CLOUD, 1);
    fx.mem.write_u32(CLOUD + 4, 0);
    for i in 0..2u32 {
        words(fx, D + 0x6850 + 0x10 * i, D + 0x70 + 0x10 * i);
        for k in 0..0x40u32 {
            let r = rnd(fx, 0x01E0_1F98);
            fx.mem.set8(CLOUD + 8 + 0x40 * i + k, rem2(r, 8));
            let v = D + 0x6870 + 0x400 * i + 0x10 * k;
            fx.random_table_vec(v, f32::from_bits(0x3F4C_CCCD));
            let y = fx.mem.f(v + 4);
            let y = fabsf(fx, y);
            fx.mem.setf(v + 4, ee::mul(0.5, y));
        }
    }
}

/// [F 0x1E0203C] The cloud's sprites: white at 70 cos(age / 32 quarter),
/// 6 (0.5 + age / 64) wide, out by 0.3 + 0.7 falloff, 0x21 updates.
fn cloud(fx: &mut Fx, sp: u32) {
    let (d, pl) = (sp + S, sp + PL);
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    fx.desc_clamp(d, 5);
    let t = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, t, 1);
    alpha(fx, d, 0);
    camera(fx, sp);
    let age = fx.mem.i32(CLOUD + 4);
    let c = cos16(fx, (age << 14) >> 5);
    let a = fix(fx, ee::mul(70.0, c), 0x01E0_20EC);
    rgba(fx, sp + 0x550, [0xFF, 0xFF, 0xFF], a as u8);
    let age = fx.mem.i32(CLOUD + 4);
    let f1 = ee::mul(0.5, ee::from_i32(age));
    let s = ee::mul(6.0, ee::add(0.5, ee::div(f1, 32.0)));
    corners(fx, sp, s, true);
    let age = fx.mem.i32(CLOUD + 4);
    let f0 = fx.tan_falloff_p(age, 0x20, 1);
    let f20 = ee::add(f32::from_bits(0x3E99_999A), ee::mul(f32::from_bits(0x3F33_3333), f0));
    for i in 0..2u32 {
        for k in 0..0x40u32 {
            let b = i32::from(fx.mem.read_u8(CLOUD + 8 + 0x40 * i + k));
            cell(fx, sp, b);
            fx.vu_scale4(pl, D + 0x6870 + 0x400 * i + 0x10 * k, f20);
            fx.vu_add4(pl, pl, D + 0x6850 + 0x10 * i);
            vertices(fx, sp);
            draw(fx, sp, 0x01E0_2318);
        }
    }
    let n = fx.mem.i32(CLOUD + 4) + 1;
    fx.mem.set32(CLOUD + 4, n);
    if n >= 0x21 {
        fx.mem.write_u8(CLOUD, 0);
    }
}

/// [F 0x1E023C8] Wave record D[0x7070]: eight lengths 0.25 + 1 - 0.5
/// (rand % 256) / 256, the ninth the first.
fn spawn_wave(fx: &mut Fx) {
    let n = fx.mem.i32(WAVE_IDX) as u32;
    let r = WAVES + 0x2C * n;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    for k in 0..8u32 {
        let v = rnd(fx, 0x01E0_2404);
        let f2 = ee::div(ee::mul(0.5, ee::from_i32(rem2(v, 256))), 256.0);
        fx.mem.setf(r + 8 + 4 * k, ee::add(0.25, ee::sub(1.0, f2)));
    }
    let x = fx.mem.read_u32(r + 8);
    fx.mem.write_u32(r + 0x28, x);
    let v = fx.mem.i32(WAVE_IDX) + 1;
    fx.mem.set32(WAVE_IDX, if v < 0x30 { v } else { 0 });
}

/// [F 0x1E024AC] The waves: each live record's ray ring round the unit,
/// the attacker drawn into a packet layer by the first.
fn waves(fx: &mut Fx, sp: u32) {
    let q = sp + Q;
    head(fx, q, 0x34, 0xC, QUAD_REGS, 0x800);
    alpha(fx, q, 1);
    fx.desc_clamp(q, WAVE_CLAMP);
    let c = Fx::clamp_i(0x10C - fx.mem.i32(D), 0, 0x20);
    let f20 = ee::mul(64.0, ee::div(ee::from_i32(c), 32.0));
    fx.mem.write_u8(sp + 0xD0, 0);
    for k in 0..0x30u32 {
        let r = WAVES + 0x2C * k;
        if fx.mem.read_u8(r) != 0 {
            wave(fx, sp, r, f20);
        }
    }
    if fx.mem.read_u8(sp + 0xD0) == 1 {
        let l = fx.mem.read_u32(sp + 0xEC);
        fx.layer_end(l, OT, 0);
    }
}

/// A ray point and its outer twin 0x1B0 on (x the length at `len`).
fn ray_point(fx: &mut Fx, at: u32, len: u32, f: f32, s: f32, c: f32) {
    let (x, y) = (ee::mul(f, s), ee::mul(f, c));
    vec2(fx, at, x, y);
    let g = fx.mem.f(len);
    let gx = ee::mul(g, x);
    let g = fx.mem.f(len);
    let gy = ee::mul(g, y);
    vec2(fx, at + 0x1B0, gx, gy);
}

/// Nine rays at radii `base` less 2 `cut`, less `cut` (never below 0) and
/// `base`, base = 1.5 radius (D+`rad`) age / 48.
fn rays(fx: &mut Fx, r: u32, rays: u32, rad: u32, cut: f32) {
    let f22 = ee::div(ee::from_i32(fx.mem.i32(r + 4)), 48.0);
    for k in 0..9u32 {
        let a = ((k << 16) as i32) >> 3;
        let s = sin16(fx, a);
        let c = cos16(fx, a);
        let len = r + 8 + 4 * k;
        let at = rays + 0x30 * k;
        let base = ee::mul(ee::mul(1.5, fx.mem.f(D + rad)), f22);
        let mut f = ee::sub(ee::sub(base, cut), cut);
        if ee_lt(f, 0.0) {
            f = 0.0;
        }
        ray_point(fx, at, len, f, s, c);
        let mut f = ee::sub(base, cut);
        if ee_lt(f, 0.0) {
            f = 0.0;
        }
        ray_point(fx, at + 0x10, len, f, s, c);
        ray_point(fx, at + 0x20, len, base, s, c);
    }
}

/// The eight ray sectors: each outer quad projected for its screen points
/// (the frame UVs), then the inner quad drawn with them into the layer.
fn ray_quads(fx: &mut Fx, sp: u32, rays: u32, ras: [u32; 4]) {
    let (vp, q, mean) = (sp + VP, sp + Q, sp + MEAN);
    for k in 0..8u32 {
        let (a, b) = (rays + 0x30 * k, rays + 0x30 * (k + 1));
        for (o, i, r0, r1) in [(0x1B0u32, 0u32, ras[0], ras[1]), (0x1D0, 0x20, ras[2], ras[3])] {
            if fx.project_n(&[a + 0x1C0, b + 0x1C0, a + o, b + o], vp, q, mean, r0) != 1 {
                continue;
            }
            for (uv, xyz) in [(0x4C8u32, 0x4D0u32), (0x4E0, 0x4E8), (0x4F8, 0x500), (0x510, 0x518)] {
                fx.uv_from_xy(sp + uv, sp + xyz, 0, 0);
            }
            if fx.project_n(&[a + 0x10, b + 0x10, a + i, b + i], vp, q, mean, r1) == 1 {
                quad_far(fx, sp);
                fx.submit(q, 0, 0);
            }
        }
    }
}

fn wave(fx: &mut Fx, sp: u32, r: u32, f20: f32) {
    let (vp, m) = (sp + VP, sp + MODEL);
    fx.camera_vp_to(vp);
    fx.translate_rows(m, sp + TURN, D + 0x90);
    mul_mem(fx, vp, m);
    rays(fx, r, RAYS, 0xB0, 3.5);
    if fx.mem.read_u8(sp + 0xD0) == 0 {
        fx.mem.write_u8(sp + 0xD0, 1);
        let l = fx.layer_begin();
        fx.mem.write_u32(sp + 0xEC, l);
        // a0..a3 as the ray loop leaves them (the last angle, 0x20B370's a1,
        // the 1.5 and the 1.0 words).
        fx.texflush([0x1_0000, m, 0x3FC0_0000, ONE]);
        let h = fx.mem.i32(D + 4);
        let (_, regs) = fx.unit_draw(h, 1, sp + 0x528, sp + 0x530);
        fx.texflush(regs);
    }
    let a = fix(fx, f20, 0x01E0_27AC);
    rgba(fx, sp + 0x4C0, [0xFF, 0x50, 0x14], a as u8);
    copy8(fx, sp + 0x4D8, sp + 0x4C0);
    rgba(fx, sp + 0x4F0, [0x80, 0, 0], 0);
    copy8(fx, sp + 0x508, sp + 0x4F0);
    ray_quads(fx, sp, RAYS, [0x01E0_285C, 0x01E0_28D8, 0x01E0_29D8, 0x01E0_2A54]);
    let n = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, n);
    if n >= 0x31 {
        fx.mem.write_u8(r, 0);
    }
}

// ------------------------------------------------------------ b592

const H2: u32 = D + 0x60;
const TRAIL_A2: u32 = D + 0xF0;
const TRAIL_B2: u32 = D + 0x12F0;
const MARK_IDX2: u32 = D + 0x36F0;
const MARK2: u32 = D + 0x3700;
const RING2: u32 = D + 0x3760;
const BAND_IDX2: u32 = D + 0x3A90;
const BANDS2: u32 = D + 0x3AA0;
const WAVE_IDX2: u32 = D + 0x3BC0;
const RAYS2: u32 = D + 0x3BD0;
const WAVES2: u32 = D + 0x3F30;

/// b592 (card 598): its entry 0x1E00080 (D at sp + 0xF0, the OT in s0).
pub fn program_b592(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x590);
    fx.mem.write_u32(sp + 0xF0, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init_b592(fx) } else { step_b592(fx, sp) };
    fx.leave(0x590);
    ret
}

fn init_b592(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x4B30);
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
    let r = fx.request_node(h, 0x19, 1);
    fx.mem.set32(D + 0x1C, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x40);
    let h = fx.mem.i32(D + 4);
    let r = crate::blib::afterimage::unit_radius(fx, h);
    fx.mem.setf(D + 0x50, r);
    for k in 0..0x11u32 {
        let a = ((k << 16) as i32) >> 4;
        let p = RING2 + 0x30 * k;
        let s = ee::mul(10.0, sin16(fx, a));
        let c = ee::mul(10.0, cos16(fx, a));
        vec2(fx, p, s, c);
        let s = ee::mul(50.0, sin16(fx, a));
        let c = ee::mul(50.0, cos16(fx, a));
        vec2(fx, p + 0x10, s, c);
        fx.vu_add4(p + 0x20, p, p + 0x10);
        fx.vu_scale4(p + 0x20, p + 0x20, 0.5);
    }
    0
}

fn step_b592(fx: &mut Fx, sp: u32) -> i32 {
    let s = sp - 0x10;
    fx.camera_turn_roll(s + TURN);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x1C);
    fx.node_position(h, n, D + 0x20);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    let n = fx.mem.i32(D + 0x54);
    if n < 3 && fx.mem.i32(D) == 20 * n + 0x52 {
        spawn_head_b592(fx, s);
    }
    sprite_desc(fx, s, 0x10);
    camera(fx, s);
    uvs(fx, s, 0x400, 0, 0x600, 0x200);
    corners(fx, s, 10.0, true);
    for k in 0..3u32 {
        if fx.mem.read_u8(H2 + 0x30 * k) != 0 {
            head_b592(fx, s, k);
        }
    }
    sprite_desc(fx, s, 0x14);
    camera(fx, s);
    for k in 0..48u32 {
        let t = TRAIL_A2 + 0x60 * k;
        if fx.mem.read_u8(t) != 0 {
            trail(fx, s, t, 0x10, 10.5, [0xFF, 0x4B, 0x2D], |x| (x << 7) >> 4, 0x3ECC_CCCD, 0x3F19_999A, 0x01E0_0E80);
        }
    }
    sprite_desc(fx, s, 0x10);
    camera(fx, s);
    for k in 0..96u32 {
        let t = TRAIL_B2 + 0x60 * k;
        if fx.mem.read_u8(t) != 0 {
            trail(fx, s, t, 0x20, 6.5, [0xFF, 0x37, 0x23], |x| (200 * x) >> 5, 0x3E4C_CCCD, 0x3F4C_CCCD, 0x01E0_1228);
        }
    }
    sprite_desc(fx, s, 0x10);
    camera(fx, s);
    uvs(fx, s, 0x600, 0, 0x800, 0x200);
    corners(fx, s, 50.0, true);
    for k in 0..3u32 {
        let m = MARK2 + 0x20 * k;
        if fx.mem.read_u8(m) != 0 {
            mark(fx, s, m, 0x10, 1, [0xFF, 0x43, 0x23], 0x01E0_1478);
        }
    }
    let q = s + Q;
    head(fx, q, 0x34, 0xC, QUAD_REGS, 0);
    alpha(fx, q, 1);
    let t = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(q, t, 1);
    fx.desc_clamp(q, 0);
    sprite_desc(fx, s, 0x10);
    uvs(fx, s, 0x400, 0, 0x600, 0x200);
    corners(fx, s, 50.0, false);
    for k in 0..3u32 {
        let r = BANDS2 + 0x60 * k;
        if fx.mem.read_u8(r) != 0 {
            band_b592(fx, s, r);
        }
    }
    let c = fx.mem.i32(D);
    if (0x11..0xBE).contains(&c) {
        spawn_wave_b592(fx);
    }
    waves_b592(fx, s);
    let c = fx.mem.i32(D);
    let a = Fx::clamp_i(0x104 - c, 0, 0x20);
    let b = Fx::clamp_i(c, 0, 0x20);
    let l = (a.wrapping_mul((b - (b << 2)) << 5) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);
    let c = fx.mem.i32(D);
    if c == 0x82 {
        fx.mem.write_u32(sp + 0xC0, 1);
    } else {
        if (0x8D..0xE4).contains(&c) {
            fx.mem.write_u32(sp + 0xC0, 6);
        }
        if c == 0xE4 {
            fx.mem.write_u32(sp + 0xC0, 2);
        }
    }
    let n = fx.mem.i32(D) + 1;
    fx.mem.set32(D, n);
    if fx.mem.i32(D) >= 0x105 {
        fx.mem.write_u32(sp + 0xC0, 8);
    }
    fx.mem.i32(sp + 0xC0)
}

/// [F b592 0x1E00444] A head and a mark at the node; the flight vector =
/// the target (a 0.0 random vector added) - the node, x kept.
fn spawn_head_b592(fx: &mut Fx, s: u32) {
    let pl = s + PL;
    let h = H2 + 0x30 * fx.mem.read_u32(D + 0x58);
    fx.mem.write_u8(h, 1);
    fx.mem.write_u32(h + 4, 0);
    let m = MARK2 + 0x20 * fx.mem.read_u32(MARK_IDX2);
    fx.mem.write_u8(m, 1);
    fx.mem.write_u32(m + 4, 0);
    words(fx, h + 0x10, D + 0x20);
    words(fx, m + 0x10, D + 0x20);
    fx.random_table_vec(pl, 0.0);
    fx.vu_add4(pl, pl, D + 0x40);
    let x = fx.mem.read_u32(h + 0x10);
    fx.mem.write_u32(pl, x);
    fx.vu_sub4(h + 0x20, pl, h + 0x10);
    let v = fx.mem.i32(D + 0x54) + 1;
    fx.mem.set32(D + 0x54, v);
    for at in [D + 0x58, MARK_IDX2] {
        let v = fx.mem.i32(at) + 1;
        fx.mem.set32(at, if v < 3 { v } else { 0 });
    }
}

/// [F b592 0x1E00718] Head `k`: its sprite and the trail entries (A % 16,
/// B % 32); on age 0x3A a band record; 0x3B updates.
fn head_b592(fx: &mut Fx, s: u32, k: u32) {
    let pl = s + PL;
    let h = H2 + 0x30 * k;
    rgba(fx, s + 0x550, [0xFF, 0x55, 0x2D], 0xFF);
    let f20 = ee::div(ee::from_i32(fx.mem.i32(h + 4)), 58.0);
    let ia = fx.mem.i32(h + 8) as u32;
    let v = ia as i32 + 1;
    fx.mem.set32(h + 8, if v < 0x10 { v } else { 0 });
    let ib = fx.mem.i32(h + 0xC) as u32;
    let v = ib as i32 + 1;
    fx.mem.set32(h + 0xC, if v < 0x20 { v } else { 0 });
    fx.vu_scale4(pl, h + 0x20, f20);
    fx.vu_add4(pl, pl, h + 0x10);
    vertices(fx, s);
    draw(fx, s, 0x01E0_0880);
    let t = TRAIL_A2 + 0x600 * k + 0x60 * ia;
    fx.mem.write_u8(t, 1);
    fx.mem.write_u32(t + 4, 0);
    words(fx, t + 0x10, pl);
    for j in 0..4u32 {
        let r = rnd(fx, 0x01E0_08E0);
        fx.mem.set8(t + 8 + j, rem2(r, 8));
        fx.random_table_vec(t + 0x20 + 0x10 * j, 0.5);
    }
    let t = TRAIL_B2 + 0xC00 * k + 0x60 * ib;
    fx.mem.write_u8(t, 1);
    fx.mem.write_u32(t + 4, 0);
    words(fx, t + 0x10, pl);
    for j in 0..4u32 {
        let r = rnd(fx, 0x01E0_0978);
        let (_, m) = fx.div(r, 3);
        fx.mem.set8(t + 8 + j, m);
        fx.random_table_vec(t + 0x20 + 0x10 * j, 1.0);
    }
    if fx.mem.i32(h + 4) == 0x3A {
        let n = fx.mem.read_u32(BAND_IDX2);
        let r = BANDS2 + 0x60 * n;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x10, pl);
        band_turn(fx, r, [0x01E0_0A28, 0x01E0_0A5C, 0x01E0_0A90]);
        let v = fx.mem.i32(BAND_IDX2) + 1;
        fx.mem.set32(BAND_IDX2, if v < 3 { v } else { 0 });
    }
    let n = fx.mem.i32(h + 4) + 1;
    fx.mem.set32(h + 4, n);
    if n >= 0x3B {
        fx.mem.write_u8(h, 0);
    }
}

/// [F b592 0x1E017A8] A band record: four passes (x turned by 0x8000 p / 4,
/// y by 0x2000 p / 4), scaled (0.1 + 0.9 falloff^2) (0.7 + 0.3 p / 4), then
/// the closing sprite.
fn band_b592(fx: &mut Fx, s: u32, r: u32) {
    let (vp, m) = (s + VP, s + MODEL);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let f0 = Fx::ramp_above(age, 48.0, 24.0);
    rgba(fx, s + 0x4C0, [0x40, 0x10, 8], 0);
    copy8(fx, s + 0x4D8, s + 0x4C0);
    let a = fix(fx, ee::mul(255.0, f0), 0x01E0_1814);
    rgba(fx, s + 0x4F0, [0xFF, 0x4B, 0x2D], a as u8);
    copy8(fx, s + 0x508, s + 0x4F0);
    for p in 0..4i32 {
        fx.camera_vp_to(vp);
        let t = [fx.mem.f(r + 0x10), fx.mem.f(r + 0x14), fx.mem.f(r + 0x18)];
        let tm = fx.trs(None, None, Some(t));
        fx.write_m4(m, &tm);
        mul_mem(fx, m, r + 0x20);
        fx.rotate_axis(m, (p * 0x8000) >> 2, 0);
        fx.rotate_axis(m, (p * 0x2000) >> 2, 1);
        let age = fx.mem.i32(r + 4);
        let f0 = fx.tan_falloff_p(age, 0x30, 2);
        let f4 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), f0));
        let f1 = ee::div(ee::mul(f32::from_bits(0x3E99_999A), ee::from_i32(p)), 4.0);
        let f = ee::mul(f4, ee::add(f32::from_bits(0x3F33_3333), f1));
        vec2(fx, s + PL, f, f);
        fx.scale_rows(m, s + PL);
        mul_mem(fx, vp, m);
        band_ring(fx, s, r, RING2, [0x01E0_1A88, 0x01E0_1BB8]);
    }
    band_end(fx, s, r, 255.0, [0xFF, 0x8A, 0x5A], 0x01E0_1D00);
}

/// [F b592 0x1E01EF4] Wave record D[0x3BC0]: a 0.5 random offset (+0x30),
/// eight lengths 0.1 + 1 - 0.2 (rand % 256) / 256, the ninth the first.
fn spawn_wave_b592(fx: &mut Fx) {
    let n = fx.mem.read_u32(WAVE_IDX2);
    let r = WAVES2 + 0x40 * n;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    fx.random_table_vec(r + 0x30, 0.5);
    for k in 0..8u32 {
        let v = rnd(fx, 0x01E0_1F30);
        let f2 = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(rem2(v, 256))), 256.0);
        fx.mem.setf(r + 8 + 4 * k, ee::add(f32::from_bits(0x3DCC_CCCD), ee::sub(1.0, f2)));
    }
    let x = fx.mem.read_u32(r + 8);
    fx.mem.write_u32(r + 0x28, x);
    let v = fx.mem.i32(WAVE_IDX2) + 1;
    fx.mem.set32(WAVE_IDX2, if v < 0x30 { v } else { 0 });
}

fn waves_b592(fx: &mut Fx, s: u32) {
    let sp = s + 0x10;
    let q = s + Q;
    head(fx, q, 0x34, 0xC, QUAD_REGS, 0x800);
    alpha(fx, q, 1);
    fx.desc_clamp(q, WAVE_CLAMP);
    let c = Fx::clamp_i(fx.mem.i32(D) - 0x10, 0, 0x20);
    let f20 = ee::mul(200.0, ee::div(ee::from_i32(c), 32.0));
    fx.mem.write_u8(sp + 0xD0, 0);
    for k in 0..0x30u32 {
        let r = WAVES2 + 0x40 * k;
        if fx.mem.read_u8(r) != 0 {
            wave_b592(fx, s, r, f20);
        }
    }
    if fx.mem.read_u8(sp + 0xD0) == 1 {
        let l = fx.mem.read_u32(sp + 0xEC);
        fx.layer_end(l, OT, 0);
    }
}

/// [F b592 0x1E020BC] One wave: rays at the unit's point + its offset, the
/// attacker drawn by the first, (FF, 1E, 14) at f20 cos(age / 48 quarter).
fn wave_b592(fx: &mut Fx, s: u32, r: u32, f20: f32) {
    let sp = s + 0x10;
    let (vp, m, pl) = (s + VP, s + MODEL, s + PL);
    fx.camera_vp_to(vp);
    fx.vu_add4(pl, r + 0x30, D + 0x30);
    fx.translate_rows(m, s + TURN, pl);
    mul_mem(fx, vp, m);
    rays(fx, r, RAYS2, 0x50, 5.0);
    if fx.mem.read_u8(sp + 0xD0) == 0 {
        fx.mem.write_u8(sp + 0xD0, 1);
        let l = fx.layer_begin();
        fx.mem.write_u32(sp + 0xEC, l);
        // a0 / a1 as the ray loop's last luis left them (1.5, 1.0), a2 the
        // translation, a3 as 0x188420 left it (the turn's first word).
        let a3 = fx.mem.read_u32(s + TURN);
        fx.texflush([0x3FC0_0000, ONE, pl, a3]);
        let h = fx.mem.i32(D + 4);
        let (_, regs) = fx.unit_draw(h, 1, s + 0x528, s + 0x530);
        fx.texflush(regs);
    }
    let x = fx.mem.i32(r + 4) << 14;
    fx.mult(0x2AAA_AAAB, x);
    let q = (fx.hilo().1 >> 3).wrapping_add(((x as u32) >> 31) as i32);
    let c = cos16(fx, q);
    let a = fix(fx, ee::mul(f20, c), 0x01E0_232C);
    rgba(fx, s + 0x4C0, [0xFF, 0x1E, 0x14], a as u8);
    copy8(fx, s + 0x4D8, s + 0x4C0);
    rgba(fx, s + 0x4F0, [0x80, 0x20, 0x20], 0);
    copy8(fx, s + 0x508, s + 0x4F0);
    ray_quads(fx, s, RAYS2, [0x01E0_23E4, 0x01E0_2460, 0x01E0_2550, 0x01E0_25CC]);
    let n = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, n);
    if n >= 0x31 {
        fx.mem.write_u8(r, 0);
    }
}
