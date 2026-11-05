//! BMODULE b543 (card 500), hand-ported from the record (linked at
//! 0x1E00000; the entry 0x1E00C30 in a 0x710 frame with D in s0 and the
//! OT in s1, data area a0 = 0x1ED0000, 0xB190 bytes; the camera callbacks
//! 0x1E04F10 / 0x1E04FD0 with D at 0x1E05100) [F]. b511's family (cards 498 / 507, [`crate::b511`]) on private
//! copies of its parts at this record's own stack offsets (they do not
//! shift uniformly): the init lays out one flower of 8 rings x 16 shards
//! (D+0x160, 26.7 long / 11.7 wide tilted by the side, cells D+0x2160),
//! four thin quads (D+0xACF0) and a ribbon group (D+0xAF40, points
//! D+0xAF70, its transform D+0xAED0). Every update the camera turn with
//! roll (sp + 0x3C0), the attacker's point D+0x30, six nodes 5 / 6 / 0xF /
//! 0xB / 0x24 / 0xC (D+0x70..) and node 0x28 (D+0x140), six aims about
//! node 5 (D+0xD0..) and the target's screen point D+0x40. A lens at the
//! target from 0x90 for 0x31 updates (D+0x5EE0); on 0x81 ten flowers at
//! nodes 6 / 0xF / 0xB / 0x24 and the aims (D+0x21E0) with 64 sparks each
//! (D+0x22C0); on 0x90 a burst at the target (D+0x4E00: puffs, sparks,
//! halo, rays); 32 gather records of 32 falling 4-segment lines (D+0x5EF0)
//! from node 0xC on 0x50..0x5A and node 5 on 0x64..0x6D; on 1..0x34 the
//! attacker-point sprites (D+0xA2F0, 16 x 0xA0); on 0x57 / 0x6A a burst of
//! four quads and a sprite at node 0x24 / 6 (D+0xADF0); from 0x3C for 0x15
//! updates the target drawn with a camera of its own (0x1F82B0 with
//! [`crate::b848::camera_pre`] / [`crate::b848::camera_post`] on D+0xAE40
//! and the distance D+0xC) between two quads; from 0x82 the ribbon from
//! node 5 to the target. The stage light 0x80 - 80 clamp(0xE0 - n)
//! clamp(n) / 1024; returns 1 / 6 / 2 / 8 on 0x86 / 0x91..0xBF / 0xC0 /
//! 0xE1.
use crate::b182::{alpha, head, rgba, uv};
use crate::b440::{copy8, copy_z, fix, rem2, words};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::LENS_CLAMP;

const D: u32 = crate::bmodule::DATA_AREA;
const FRAME: u32 = 0x710;
const PROJ_OUT: u32 = 0x002D_3600;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
/// The camera's view-projection, the model matrix and the camera turn.
const M: u32 = 0x340;
const M2: u32 = 0x380;
const TURN: u32 = 0x3C0;
/// The sprite corners and their vertices.
const C0: u32 = 0x400;
const V0: u32 = 0x440;
/// The work vector.
const W: u32 = 0x4C0;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const DESC: u32 = 0x528;
/// The quad descriptor (prim 0x34, 12 registers).
const QUAD: u32 = 0x670;
/// The lens' triangle descriptor.
const TRI: u32 = 0x5F8;
/// The projections' mean w.
const WOUT: u32 = 0x70C;
/// The ribbon group, its points and transform, the point it follows.
const RIBBON: u32 = D + 0xAF40;
const RIB_PTS: u32 = D + 0xAF70;
const RIB_X: u32 = D + 0xAED0;
const RIB_AT: u32 = D + 0xB170;
/// The camera pass's saved camera input.
const SAVED: u32 = D + 0xAE40;
const D_STATIC: u32 = 0x01E0_5100;

const LENS_RA: [u32; 12] = [
    0x01E0_16F8, 0x01E0_1704, 0x01E0_1748, 0x01E0_1754, 0x01E0_1798, 0x01E0_17A4, 0x01E0_1858, 0x01E0_1864, 0x01E0_18D0, 0x01E0_18DC, 0x01E0_1948, 0x01E0_1954,
];

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E00C30 of b543 (card 500).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 23
}

fn vec4(fx: &mut Fx, p: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        fx.mem.setf(p + 4 * k as u32, *x);
    }
}

/// The flower of 8 rings x 16 shards at D+0x160 (cells D+0x2160) [F
/// 0x1E00D9C..0x1E00FAC]: b511's flower_init with its matrix at sp +
/// 0x480 and vector at sp + 0x4C0.
fn flower_init(fx: &mut Fx, sp: u32) {
    let (m, vv) = (sp + 0x480, sp + 0x4C0);
    let (len, width, half) = (f(0xC1D5_999A), f(0x413B_3333), f(0x3FD9_999A));
    for i in 0..8i32 {
        let f20 = ee::div(ee::mul(len, ee::from_i32(i + 1)), 8.0);
        let f22 = ee::div(ee::mul(len, ee::from_i32(i)), 7.0);
        let s = ee::from_i32(fx.mem.i32(D + 0xC));
        let f22 = ee::mul(-1.0, ee::mul(f22, s));
        let f21 = ee::div(ee::mul(width, ee::from_i32(7 - i)), 7.0);
        let s7 = (i << 13) >> 3;
        for j in 0..0x10i32 {
            let a = (((j << 16) >> 4).wrapping_add(s7)) & 0xFFFF;
            let sn = fx.tsin(a);
            fx.mem.setf(vv, ee::mul(f21, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(vv + 4, ee::mul(f21, cs));
            fx.mem.setf(vv + 8, f22);
            fx.mem.write_u32(vv + 0xC, 0x3F80_0000);
            fx.point_along(m, vv);
            let r = rnd(fx, 0x01E0_0E8C);
            fx.rotate_axis(m, (rem2(r, 256) << 16) >> 8, 2);
            let q = D + 0x400 * i as u32 + 0x40 * j as u32 + 0x160;
            let (n, p) = (ee::neg(half), half);
            vec4(fx, q, [n, 0.0, 0.0, 1.0]);
            vec4(fx, q + 0x10, [p, 0.0, 0.0, 1.0]);
            vec4(fx, q + 0x20, [n, 0.0, f20, 1.0]);
            vec4(fx, q + 0x30, [p, 0.0, f20, 1.0]);
            for k in 0..4u32 {
                fx.vu_apply_to(q + 0x10 * k, m, q + 0x10 * k);
            }
            let r = rnd(fx, 0x01E0_0F40);
            fx.mem.set8(D + 0x10 * i as u32 + j as u32 + 0x2160, rem2(r, 4) + 4);
        }
    }
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0xB190);
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
    for (k, node) in [5, 6, 0xF, 0xB, 0x24, 0xC].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set32(D + 0x50 + 4 * k as u32, r);
    }
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x28, 0);
    fx.mem.set32(D + 0x130, r);
    fx.mem.write_u32(D + 0x150, 0);
    flower_init(fx, sp);
    fx.mem.write_u8(D + 0x21E0, 0);
    fx.mem.write_u32(D + 0x22B0, 0);
    fx.mem.write_u8(D + 0x22C0, 0);
    fx.mem.write_u32(D + 0x4DF0, 0);
    fx.mem.write_u8(D + 0x4E00, 0);
    fx.mem.write_u8(D + 0x5EE0, 0);
    fx.mem.write_u32(D + 0x5EE8, 0);
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x5EF0 + 0x220 * k, 0);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0xA2F0 + 0xA0 * k, 0);
    }
    // Four thin quads (D+0xACF0 + 0x40 k), each turned a quarter of a half
    // turn on from the last, the odd ones shorter.
    let n = ee::neg(f(0x3D4C_CCCD));
    for k in 0..4i32 {
        let h = ee::mul(2.5, ee::sub(1.0, ee::mul(f(0x3ECC_CCCD), ee::from_i32(rem2(k, 2)))));
        let c = sp + 0x400;
        vec4(fx, c, [n, ee::neg(h), 0.0, 1.0]);
        vec4(fx, c + 0x10, [f(0x3D4C_CCCD), ee::neg(h), 0.0, 1.0]);
        vec4(fx, c + 0x20, [n, h, 0.0, 1.0]);
        vec4(fx, c + 0x30, [f(0x3D4C_CCCD), h, 0.0, 1.0]);
        fx.identity(sp + 0x480);
        fx.rotate_axis(sp + 0x480, (k * 0x8000) >> 2, 2);
        for m in 0..4u32 {
            fx.vu_apply_to(D + 0xACF0 + 0x40 * k as u32 + 0x10 * m, sp + 0x480, c + 0x10 * m);
        }
    }
    for k in 0..2u32 {
        fx.mem.write_u8(D + 0xADF0 + 0x20 * k, 0);
    }
    fx.mem.write_u8(D + 0xAE30, 0);
    fx.xform_init(RIB_X);
    fx.texture5(sp + 0x4E0, 4);
    // The ribbon group: texture 4, grey 0x80, flags 0x8003, 60 wide, cell
    // (0x10, 0, 0x10, 0x20), fade 0x30, life 0x32, 0x20 slots.
    let rh = fx.mem.read_u32(sp + 0x4E0);
    fx.mem.write_u32(RIBBON, rh);
    fx.mem.write_u32(RIBBON + 8, 0x8003);
    for o in [6u32, 5, 4, 7] {
        fx.mem.write_u8(RIBBON + o, 0x80);
    }
    for (o, h) in [(0x10u32, 0x10u16), (0x12, 0), (0x14, 0x10), (0x16, 0x20), (0x18, 0x10), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)] {
        fx.mem.write_u16(RIBBON + o, h);
    }
    fx.mem.write_u32(RIBBON + 0xC, 0x4270_0000);
    for (o, h) in [(0x22u32, 0x30u16), (0x24, 0), (0x26, 0x32), (0x28, 0x20), (0x2A, 0), (0x2C, 1)] {
        fx.mem.write_u16(RIBBON + o, h);
    }
    fx.mem.write_u8(D + 0xB180, 6);
    0
}

fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + M);
    fx.identity(sp + M2);
    crate::blib::exe::mul_mem(fx, sp + M, sp + M2);
}

/// The screen point of `p` (0x204D80 through sp + 0x340 into sp + 0x700)
/// into `o` (x / y clamped to 0..512) [F 0x1E01450..0x1E01540].
fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    let m = fx.read_m4(sp + M);
    fx.project_at(p, &m, sp + 0x700, sp + 0x70C);
    let x = (i32::from(fx.mem.read_u16(sp + 0x700)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x702)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x700) << 8) >> 40;
    fx.mem.setf(o + 8, z as u32 as f32);
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

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    for k in 0..6u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x50 + 4 * k);
        fx.node_position(h, s, D + 0x70 + 0x10 * k);
    }
    let h = fx.mem.i32(D + 4);
    let s = fx.mem.i32(D + 0x130);
    fx.node_position(h, s, D + 0x140);
    // Six aims about node 5.
    let (a, b, c) = (f(0x4073_3333), f(0x3FCC_CCCD), 2.0f32);
    for (k, (x, y)) in [(a, 0.0), (-a, 0.0), (c, b), (-c, b), (c, -b), (-c, -b)].into_iter().enumerate() {
        vec4(fx, D + 0xD0 + 0x10 * k as u32, [x, y, 0.0, 1.0]);
    }
    for k in 0..6u32 {
        let p = D + 0xD0 + 0x10 * k;
        fx.vu_add4(p, p, D + 0x70);
    }
    camera(fx, sp);
    screen_point(fx, sp, D + 0x20, D + 0x40);

    lens(fx, sp);
    if fx.mem.i32(D) == 0x81 {
        flower_spawn(fx, sp);
    }
    flowers(fx, sp);
    sparks(fx, sp);
    if fx.mem.i32(D) == 0x90 {
        burst_spawn(fx);
    }
    burst(fx, sp);
    gather(fx, sp);
    attacker_sprites(fx, sp);
    quads(fx, sp);
    camera_pass(fx, sp);
    ribbon(fx);

    let n = fx.mem.i32(D);
    let s1 = Fx::clamp_i(0xE0 - n, 0, 0x20);
    let n = fx.mem.i32(D);
    let c = Fx::clamp_i(n, 0, 0x20);
    let l = (fx.mult(s1, c.wrapping_mul(-80)) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xB0);
    if n == 0x86 {
        ret = 1;
    } else {
        if (0x91..0xC0).contains(&n) {
            ret = 6;
        }
        if n == 0xC0 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xE1 {
        ret = 8;
    }
    fx.mem.set32(sp + 0xB0, ret);
    ret
}

/// The lens at the target's screen point (D+0x5EE0 for 0x31 updates) [F
/// 0x1E01540..0x1E01A24]: b511's lens with its triangle at sp + 0x5F8 and
/// the fan's vertices at sp + 0x440.
fn lens(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x90 {
        fx.mem.write_u8(D + 0x5EE0, 1);
        fx.mem.write_u32(D + 0x5EE4, 0);
    }
    if fx.mem.read_u8(D + 0x5EE0) != 1 {
        return;
    }
    let t = sp + TRI;
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0x800);
    alpha(fx, t, 0);
    fx.desc_clamp(t, LENS_CLAMP);
    let a = crate::b182::clamp_regs(fx, t, LENS_CLAMP);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xEC, layer);
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x658, sp + 0x660);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0x5EE4);
    let x = (0x30 - age) << 6;
    fx.mult(0x2AAA_AAAB, x);
    let al = (fx.hilo().1 >> 3).wrapping_add(((x as u32) >> 31) as i32);
    rgba(fx, sp + 0x608, [0x80; 3], al as u8);
    copy8(fx, sp + 0x620, sp + 0x608);
    copy8(fx, sp + 0x638, sp + 0x608);
    for j in 0..8i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f(0x3E19_999A), ee::from_i32(8 - j)), 8.0));
        crate::b424::fan(fx, t, D + 0x40, sp + V0, s, 0, &LENS_RA);
    }
    let layer = fx.mem.read_u32(sp + 0xEC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x5EE4) + 1;
    fx.mem.set32(D + 0x5EE4, age);
    if age >= 0x31 {
        fx.mem.write_u8(D + 0x5EE0, 0);
    }
}

/// The ten flower points: nodes 6 / 0xF / 0xB / 0x24 and the six aims
/// (the jump table 0x1E05080).
const SOURCES: [u32; 10] = [0x80, 0x90, 0xA0, 0xB0, 0xD0, 0xE0, 0xF0, 0x100, 0x110, 0x120];

/// On 0x81 the flower record (D+0x21E0, 0xD0: ten angles +8, ten points
/// +0x30) and the spark record (D+0x22C0, 0x2B30: ten x 64 cells +8, ten
/// points +0x290, ten x 64 0.7 table vectors +0x330) [F
/// 0x1E01A24..0x1E01D74].
fn flower_spawn(fx: &mut Fx, sp: u32) {
    let k = fx.mem.i32(D + 0x150) as u32;
    let fb = D + 0xD0 * k + 0x21E0;
    fx.mem.write_u8(fb, 1);
    fx.mem.write_u32(fb + 4, 0);
    let m = fx.mem.i32(D + 0x22B0) as u32;
    let tb = D + 0x2B30 * m + 0x22C0;
    fx.mem.write_u8(tb, 1);
    fx.mem.write_u32(tb + 4, 0);
    for s in 0..10u32 {
        let w = sp + W;
        words(fx, w, D + SOURCES[s as usize]);
        let r = rnd(fx, 0x01E0_1C48);
        fx.mem.set32(fb + 8 + 4 * s, (rem2(r, 256) << 16) >> 8);
        words(fx, fb + 0x30 + 0x10 * s, w);
        words(fx, tb + 0x290 + 0x10 * s, w);
        for j in 0..0x40u32 {
            let r = rnd(fx, 0x01E0_1CC8);
            fx.mem.set8(tb + 8 + 0x40 * s + j, rem2(r, 8));
            fx.random_table_vec(tb + 0x330 + 0x400 * s + 0x10 * j, f(0x3F33_3333));
        }
    }
    let k = fx.mem.i32(D + 0x150) + 1;
    fx.mem.set32(D + 0x150, k);
    if k > 0 {
        fx.mem.write_u32(D + 0x150, 0);
    }
    let m = fx.mem.i32(D + 0x22B0) + 1;
    fx.mem.set32(D + 0x22B0, m);
    if m > 0 {
        fx.mem.write_u32(D + 0x22B0, 0);
    }
}

/// The sprite descriptor (prim 0x26 | 0x800) with texture D+`tex`, clamp
/// 5 and (with `preset`) its ALPHA.
fn sprite_desc(fx: &mut Fx, sp: u32, tex: u32, preset: Option<i32>) {
    let d = sp + DESC;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    if let Some(p) = preset {
        alpha(fx, d, p);
    }
}

/// The sprite descriptor in the later order: ALPHA preset 1, texture
/// D+`tex`, clamp 5.
fn sprite_desc_atc(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + DESC;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
}

/// The quad colour (sp + 0x680 and its three copies).
fn quad_colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    rgba(fx, sp + 0x680, c, a);
    for o in [0x698u32, 0x6B0, 0x6C8] {
        copy8(fx, sp + o, sp + 0x680);
    }
}

/// The 8 x 16 shards of the flower through the matrix at sp + 0x340,
/// keyed by their mean w.
fn shards(fx: &mut Fx, sp: u32) {
    let q = sp + QUAD;
    let mm = fx.read_m4(sp + M);
    for i in 0..8u32 {
        for j in 0..0x10u32 {
            let cb = i32::from(fx.mem.read_u8(D + 0x10 * i + j + 0x2160));
            let c = rem2(cb, 4) << 5;
            let r = (cb >> 2) << 5;
            let f4 = ee::from_i32(c);
            let f2 = ee::from_i32(r);
            let f5 = ee::div(ee::add(32.0, f4), 128.0);
            let f4 = ee::div(f4, 128.0);
            let f3 = ee::div(f2, 64.0);
            let f0 = ee::div(ee::add(16.0, f2), 64.0);
            for (o, x) in [(0x688u32, f4), (0x68C, f0), (0x6A0, f5), (0x6A4, f0), (0x6B8, f4), (0x6BC, f3), (0x6D0, f5), (0x6D4, f3)] {
                fx.mem.setf(sp + o, x);
            }
            let petal = D + 0x400 * i + 0x40 * j + 0x160;
            if fx.project_desc(petal, PROJ_OUT, Some(&mm), q, sp + WOUT) {
                let w = fx.mem.f(sp + WOUT);
                let key = fix(fx, w, 0x01E0_207C);
                fx.submit(q, OT, key as i32);
            }
        }
    }
}

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1) at sp + 0x400 / 0x410,
/// turned by the camera turn with `turn`.
fn corners(fx: &mut Fx, sp: u32, s: f32, turn: bool) {
    let n = ee::neg(s);
    vec4(fx, sp + C0, [n, n, 0.0, 1.0]);
    vec4(fx, sp + C0 + 0x10, [s, s, 0.0, 1.0]);
    if turn {
        fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
        fx.vu_apply_to(sp + C0 + 0x10, sp + TURN, sp + C0 + 0x10);
    }
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    let d = sp + DESC;
    uv(fx, d + 0x18, u0);
    uv(fx, d + 0x1A, v0);
    uv(fx, d + 0x28, u1);
    uv(fx, d + 0x2A, v1);
}

/// The 4 x 8 cell `b` of the texture into the sprite's UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    uvs(fx, sp, ((c << 9) & 0x3FFF) as u32, ((r << 9) & 0x3FFF) as u32, ((((c << 5) + 0x20) << 4) & 0x3FFF) as u32, ((((r << 5) + 0x20) << 4) & 0x3FFF) as u32);
}

/// The sprite at sp + 0x4C0 (= dir x f + at); its vertices sp + 0x440 /
/// 0x450.
fn place(fx: &mut Fx, sp: u32, dir: u32, f: f32, at: u32) {
    fx.vu_scale4(sp + W, dir, f);
    fx.vu_add4(sp + W, sp + W, at);
    fx.vu_add4(sp + V0, sp + C0, sp + W);
    fx.vu_add4(sp + V0 + 0x10, sp + C0 + 0x10, sp + W);
}

/// The sprite projected and submitted at its mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    if crate::fxlib2::project(fx, sp + V0, PROJ_OUT, sp + M, sp + DESC, sp + WOUT) {
        let w = fx.mem.f(sp + WOUT);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// The sprite at the point `at` projected at far Z.
fn draw_far(fx: &mut Fx, sp: u32, at: u32) {
    words(fx, sp + W, at);
    fx.vu_add4(sp + V0, sp + C0, sp + W);
    fx.vu_add4(sp + V0 + 0x10, sp + C0 + 0x10, sp + W);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    far_sprite(fx, sp);
}

fn far_sprite(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    if crate::fxlib2::project(fx, sp + V0, PROJ_OUT, sp + M, d, sp + WOUT) {
        far_z(fx, d + 0x20, &[d + 0x30]);
        fx.submit(d, OT, 0);
    }
}

fn far_z(fx: &mut Fx, at: u32, copies: &[u32]) {
    let x = fx.mem.read_u64(at);
    fx.mem.write_u64(at, (x & KEEP) | FAR_Z);
    for &c in copies {
        copy_z(fx, c, at);
    }
}

/// The ten flowers [F 0x1E01D74..0x1E023CC].
fn flowers(fx: &mut Fx, sp: u32) {
    let q = sp + QUAD;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(q, rh, 1);
    alpha(fx, q, 1);
    fx.desc_clamp(q, 0);
    let fb = D + 0x21E0;
    if fx.mem.read_u8(fb) == 0 {
        return;
    }
    let left = 0xA - fx.mem.i32(fb + 4);
    let x = fx.mult(left, 0x56);
    fx.mult(0x6666_6667, x);
    let a = (fx.hilo().1 >> 2).wrapping_add(((x as u32) >> 31) as i32);
    quad_colour(fx, sp, [0x30, 0x40, 0xFF], a as u8);
    let age = ee::from_i32(fx.mem.i32(fb + 4));
    let s = ee::mul(ee::add(0.5, ee::div(ee::mul(0.5, age), 10.0)), 1.5);
    vec4(fx, sp + W, [s, s, s, 1.0]);
    for k in 0..10u32 {
        fx.camera_vp_to(sp + M);
        fx.identity(sp + M2);
        fx.translate_rows(sp + M2, sp + M2, fb + 0x30 + 0x10 * k);
        fx.rotate_axis(sp + M2, 0, 0);
        let ang = fx.mem.i32(fb + 8 + 4 * k);
        fx.rotate_axis(sp + M2, ang, 2);
        fx.scale_rows(sp + M2, sp + W);
        crate::blib::exe::mul_mem(fx, sp + M, sp + M2);
        shards(fx, sp);
    }
    sprite_desc(fx, sp, 0x14, Some(1));
    camera(fx, sp);
    corners(fx, sp, 70.0, true);
    let age = fx.mem.i32(fb + 4);
    let tf = fx.tan_falloff_p(age, 0xA, 1);
    let a = fix(fx, ee::mul(100.0, ee::sub(1.0, tf)), 0x01E0_21EC);
    rgba(fx, sp + DESC + 0x10, [0x32, 0x6A, 0xFF], a as u8);
    for k in 0..10u32 {
        draw_far(fx, sp, fb + 0x30 + 0x10 * k);
    }
    let age = fx.mem.i32(fb + 4) + 1;
    fx.mem.set32(fb + 4, age);
    if age >= 0xB {
        fx.mem.write_u8(fb, 0);
    }
}

/// The ten flowers' 64 sparks each [F 0x1E023CC..0x1E02734].
fn sparks(fx: &mut Fx, sp: u32) {
    sprite_desc(fx, sp, 0x10, Some(0));
    camera(fx, sp);
    let tb = D + 0x22C0;
    if fx.mem.read_u8(tb) == 0 {
        return;
    }
    let age = ee::from_i32(fx.mem.i32(tb + 4));
    let s = ee::mul(f(0x4053_3333), ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0)));
    corners(fx, sp, s, true);
    let a = ((0x20 - fx.mem.i32(tb + 4)) << 6) >> 5;
    rgba(fx, sp + DESC + 0x10, [0xBC; 3], a as u8);
    let age = fx.mem.i32(tb + 4);
    let f20 = fx.tan_falloff_p(age, 0x20, 1);
    for k in 0..10u32 {
        for j in 0..0x40u32 {
            place(fx, sp, tb + 0x330 + 0x400 * k + 0x10 * j, f20, tb + 0x290 + 0x10 * k);
            let c = i32::from(fx.mem.read_u8(tb + 8 + 0x40 * k + j));
            cell_uvs(fx, sp, c);
            draw(fx, sp, 0x01E0_2688);
        }
    }
    let age = fx.mem.i32(tb + 4) + 1;
    fx.mem.set32(tb + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(tb, 0);
    }
}

/// The burst record (D+0x4E00, 0x10E0) at the target [F
/// 0x1E02734..0x1E029C0]: b511's burst_spawn.
fn burst_spawn(fx: &mut Fx) {
    fx.mem.write_u32(D + 0x4DF0, 0);
    let c = fx.mem.i32(D + 0x4DF0) as u32;
    let r = D + 0x10E0 * c + 0x4E00;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 1);
    vec4(fx, r + 0x90, [0.0, 0.0, 0.0, 1.0]);
    fx.vu_add4(r + 0x90, r + 0x90, D + 0x20);
    for j in 0..0x40u32 {
        let b = rnd(fx, 0x01E0_2858);
        fx.mem.write_u16(r + 8 + 2 * j, rem2(b, 8) as u16);
        fx.random_table_vec(r + 0xA0 + 0x10 * j, f(0x402C_CCCD));
    }
    for j in 0..0x80u32 {
        fx.random_table_vec(r + 0x4A0 + 0x10 * j, 3.75);
    }
    for j in 0..0x40u32 {
        let b = rnd(fx, 0x01E0_2938);
        let (_, h) = fx.div(b, 3);
        fx.mem.set8(r + 0xCA0 + j, h + 5);
        fx.random_table_vec(r + 0xCE0 + 0x10 * j, f(0x402C_CCCD));
    }
}

/// The burst [F 0x1E029C0..0x1E03704]: b511's with no puff or spark
/// alpha and rays fading by their step.
fn burst(fx: &mut Fx, sp: u32) {
    let r = D + 0x4E00;
    if fx.mem.read_u8(r) == 0 {
        return;
    }
    let pt = r + 0x90;
    let d = sp + DESC;
    // Puffs.
    sprite_desc(fx, sp, 0x18, Some(1));
    camera(fx, sp);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let s = ee::mul(18.0, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), age), 48.0)));
    corners(fx, sp, s, true);
    rgba(fx, d + 0x10, [0x38, 0x6C, 0xFF], 0);
    let age = fx.mem.i32(r + 4);
    let f22 = fx.tan_falloff_p(age, 0x30, 1);
    for j in 0..0x40u32 {
        fx.vu_scale4(sp + W, r + 0xCE0 + 0x10 * j, f22);
        fx.vu_add4(sp + W, sp + W, pt);
        fx.vu_add4(sp + V0, sp + C0, sp + W);
        fx.vu_add4(sp + V0 + 0x10, sp + C0 + 0x10, sp + W);
        let c = i32::from(fx.mem.read_u8(r + 0xCA0 + j));
        cell_uvs(fx, sp, c);
        far_sprite(fx, sp);
    }
    // Sparks in two colours.
    sprite_desc(fx, sp, 0x10, None);
    camera(fx, sp);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let s = ee::mul(15.0, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), age), 48.0)));
    corners(fx, sp, s, true);
    fx.mem.write_u8(sp + 0x33F, 0);
    fx.mem.write_u8(sp + 0xD0, 0);
    for j in 0..0x40u32 {
        let age = fx.mem.i32(r + 4);
        let mut f22 = fx.tan_falloff_p(age, 0x30, 1);
        if j % 2 == 0 {
            f22 = ee::mul(f22, f(0x3F4C_CCCD));
            alpha(fx, d, 2);
            rgba(fx, d + 0x10, [0x80, 0xFF, 0xFF], 0);
        } else {
            alpha(fx, d, 1);
            rgba(fx, d + 0x10, [0x1E, 0x5A, 0xFF], 0);
        }
        fx.vu_scale4(sp + W, r + 0xA0 + 0x10 * j, f22);
        fx.vu_add4(sp + W, sp + W, pt);
        fx.vu_add4(sp + V0, sp + C0, sp + W);
        fx.vu_add4(sp + V0 + 0x10, sp + C0 + 0x10, sp + W);
        let c = i32::from(fx.mem.read_u16(r + 8 + 2 * j));
        cell_uvs(fx, sp, c);
        far_sprite(fx, sp);
    }
    // The halo, scaled by the spread at the point.
    sprite_desc(fx, sp, 0x14, Some(1));
    fx.camera_vp_to(sp + M);
    fx.translate_rows(sp + M2, sp + TURN, pt);
    let age = fx.mem.i32(r + 4);
    let tf = fx.tan_falloff_p(age, 0x30, 1);
    let s = ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), tf));
    vec4(fx, sp + W, [s, s, s, 1.0]);
    fx.scale_rows(sp + M2, sp + W);
    crate::blib::exe::mul_mem(fx, sp + M, sp + M2);
    let left = ee::from_i32(0x30 - fx.mem.i32(r + 4));
    let a = fix(fx, ee::mul(200.0, ee::div(left, 48.0)), 0x01E0_3234);
    rgba(fx, d + 0x10, [0x3A, 0x5E, 0xFF], a as u8);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    vec4(fx, sp + C0, [-195.0, -195.0, 0.0, 1.0]);
    vec4(fx, sp + C0 + 0x10, [195.0, 195.0, 0.0, 1.0]);
    if crate::fxlib2::project(fx, sp + C0, PROJ_OUT, sp + M, d, sp + WOUT) {
        far_z(fx, d + 0x20, &[d + 0x30]);
        fx.submit(d, OT, 0);
    }
    // The rays.
    sprite_desc(fx, sp, 0x14, Some(1));
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    camera(fx, sp);
    corners(fx, sp, 1.0, true);
    let f23 = ee::div(ee::from_i32(0x30 - fx.mem.i32(r + 4)), 48.0);
    let age = fx.mem.i32(r + 4);
    let f22 = fx.tan_falloff_p(age, 0x30, 1);
    for m in 0..0x80u32 {
        let dir = r + 0x4A0 + 0x10 * m;
        for s6 in 0..6i32 {
            let x = (s6 + 6) * 255;
            fx.mult(0x2AAA_AAAB, x);
            let q = (fx.hilo().1 >> 1).wrapping_add(((x as u32) >> 31) as i32);
            let a = fix(fx, ee::mul(f23, ee::from_i32(q)), 0x01E0_35AC);
            rgba(fx, d + 0x10, [0x80, 0xF0, 0xFF], a as u8);
            let f12 = ee::add(f22, ee::mul(f(0x3BE5_6042), ee::from_i32(s6)));
            place(fx, sp, dir, f12, pt);
            draw(fx, sp, 0x01E0_3664);
        }
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x31 {
        fx.mem.write_u8(r, 0);
    }
}

/// The y drop -0.0109 x 250 / 2 x f^2 of the point at `p`.
fn drop_y(fx: &mut Fx, p: u32, t: f32) {
    let pw = fx.powf(t, 2.0);
    let d = ee::mul(250.0, ee::mul(0.5, ee::mul(f(0xBC32_864D), pw)));
    let y = ee::add(fx.mem.f(p + 4), d);
    fx.mem.setf(p + 4, y);
}

/// The gather records (D+0x5EF0 + 0x220 k, 32): on 0x50..0x5A one at node
/// 0xC, on 0x64..0x6D one at node 5, with 32 0.2 table vectors (+0x20)
/// about the point (+0x10); drawn for 9 updates as 32 falling lines of
/// four segments (prim 0x11, (FF, 78, 1E) fading by 255 (8 - age) / 8 x
/// sin) [F 0x1E03704..0x1E03C90].
fn gather(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let mut on = false;
    if (0x50..0x5B).contains(&n) {
        words(fx, sp + W, D + 0xC0);
        on = true;
    }
    let n = fx.mem.i32(D);
    if (0x64..0x6E).contains(&n) {
        words(fx, sp + W, D + 0x70);
        on = true;
    }
    if on {
        let c = fx.mem.i32(D + 0x5EE8) as u32;
        let r = D + 0x220 * c + 0x5EF0;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x10, sp + W);
        for j in 0..0x20u32 {
            fx.random_table_vec(r + 0x20 + 0x10 * j, f(0x3E4C_CCCD));
        }
        let c = fx.mem.i32(D + 0x5EE8) + 1;
        fx.mem.set32(D + 0x5EE8, c);
        if c >= 0x20 {
            fx.mem.write_u32(D + 0x5EE8, 0);
        }
    }
    let d = sp + 0x4F0;
    head(fx, d, 0x11, 4, 0x4141, 0);
    alpha(fx, d, 1);
    camera(fx, sp);
    let (w0, w1) = (sp + W, sp + W + 0x10);
    for k in 0..0x20u32 {
        let r = D + 0x220 * k + 0x5EF0;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let f21 = ee::div(ee::from_i32(8 - age), 8.0);
        let f20 = ee::div(ee::from_i32(age), 8.0);
        let at = r + 0x10;
        for v in 0..0x20u32 {
            let dir = r + 0x20 + 0x10 * v;
            for j in 0..4i32 {
                if j == 0 {
                    let sn = fx.tsin((j * 0x4000) >> 2);
                    let a = fix(fx, ee::mul(f21, ee::mul(255.0, sn)), 0x01E0_3900);
                    rgba(fx, d + 0x10, [0xFF, 0x78, 0x1E], a as u8);
                    let sn = fx.tsin(((j + 1) << 14) >> 2);
                    let a = fix(fx, ee::mul(f21, ee::mul(255.0, sn)), 0x01E0_394C) & 0xFF;
                    fx.mem.write_u8(sp + 0xD0, a as u8);
                    rgba(fx, d + 0x20, [0xFF, 0x78, 0x1E], a as u8);
                    let f22 = ee::add(f20, ee::mul(f(0x3D4C_CCCD), ee::from_i32(j)));
                    fx.vu_scale4(w0, dir, f22);
                    fx.vu_add4(w0, w0, at);
                    drop_y(fx, w0, f22);
                } else {
                    let a = fx.mem.read_u8(sp + 0xD0);
                    rgba(fx, d + 0x10, [0xFF, 0x78, 0x1E], a);
                    let sn = fx.tsin(((j + 1) << 14) >> 2);
                    let a = fix(fx, ee::mul(f21, ee::mul(255.0, sn)), 0x01E0_3AD8) & 0xFF;
                    fx.mem.write_u8(sp + 0xD0, a as u8);
                    rgba(fx, d + 0x20, [0xFF, 0x78, 0x1E], a as u8);
                    for c in 0..4u32 {
                        let x = fx.mem.read_u32(w1 + 4 * c);
                        fx.mem.write_u32(w0 + 4 * c, x);
                    }
                }
                let f22 = ee::add(f20, ee::mul(f(0x3D4C_CCCD), ee::from_i32(j + 1)));
                fx.vu_scale4(w1, dir, f22);
                fx.vu_add4(w1, w1, at);
                drop_y(fx, w1, f22);
                words(fx, sp + V0, w0);
                words(fx, sp + V0 + 0x10, w1);
                if crate::fxlib2::project(fx, sp + V0, PROJ_OUT, sp + M, d, sp + WOUT) {
                    let w = fx.mem.f(sp + WOUT);
                    let key = fix(fx, w, 0x01E0_3BFC);
                    fx.submit(d, OT, key as i32);
                }
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 9 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The attacker-point sprites (D+0xA2F0 + 0xA0 (n % 16)): on 1..0x34 one
/// at the attacker's point 5 lower with 8 0.7 table vectors (y x 0.7)
/// drawn for 0x11 updates (5-wide white sprites of texture D+0x10) [F
/// 0x1E03C90..0x1E0411C].
fn attacker_sprites(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x35 {
        let r = D + 0xA0 * rem2(n, 0x10) as u32 + 0xA2F0;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x10, D + 0x30);
        let y = ee::add(fx.mem.f(r + 0x14), -5.0);
        fx.mem.setf(r + 0x14, y);
        for j in 0..8u32 {
            let b = rnd(fx, 0x01E0_3D44);
            fx.mem.set8(r + 8 + j, rem2(b, 8));
            let p = r + 0x20 + 0x10 * j;
            fx.random_table_vec(p, f(0x3F33_3333));
            let y = ee::mul(fx.mem.f(p + 4), f(0x3F33_3333));
            fx.mem.setf(p + 4, y);
        }
    }
    for k in 0..0x10u32 {
        let r = D + 0xA0 * k + 0xA2F0;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        sprite_desc(fx, sp, 0x10, Some(0));
        camera(fx, sp);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let s = ee::mul(5.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 16.0)));
        corners(fx, sp, s, true);
        let a = ((0x10 - fx.mem.i32(r + 4)) << 6) >> 4;
        rgba(fx, sp + DESC + 0x10, [0xFF; 3], a as u8);
        let age = fx.mem.i32(r + 4);
        let tf = fx.tan_falloff_p(age, 0x10, 1);
        let f20 = ee::add(f(0x3DCC_CCCD), ee::mul(f(0x3F66_6666), tf));
        for j in 0..8u32 {
            let c = i32::from(fx.mem.read_u8(r + 8 + j));
            cell_uvs(fx, sp, c);
            place(fx, sp, r + 0x20 + 0x10 * j, f20, r + 0x10);
            draw(fx, sp, 0x01E0_4094);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The quad bursts (D+0xADF0 + 0x20 k, 2): on 0x57 at node 0x24, on 0x6A
/// at node 6, drawn for 9 updates as the four thin quads of D+0xACF0
/// facing the camera at far Z and a 1.5-wide sprite, (FF, E8, D8) fading
/// by 255 cos [F 0x1E0411C..0x1E04864].
fn quads(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let pick = if n == 0x6A {
        Some((1u32, 0x80u32))
    } else if n == 0x57 {
        Some((0, 0x90))
    } else {
        None
    };
    if let Some((k, src)) = pick {
        let r = D + 0x20 * k + 0xADF0;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        fx.mem.write_u32(r + 8, 0x3F80_0000);
        words(fx, sp + W, D + src);
        words(fx, r + 0x10, sp + W);
    }
    let q = sp + 0x580;
    head(fx, q, 0x24, 9, 0x4_2424_2421, 0x800);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 5);
    for (o, x) in [(0x18u32, 0u32), (0x1A, 0x200), (0x28, 0x200), (0x2A, 0x200), (0x38, 0), (0x3A, 0x400), (0x48, 0x200), (0x4A, 0x400)] {
        uv(fx, q + o, x);
    }
    sprite_desc_atc(fx, sp, 0x14);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    camera(fx, sp);
    for k in 0..2u32 {
        let r = D + 0x20 * k + 0xADF0;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let c = fx.tcos((age << 14) >> 3);
        let a = fix(fx, ee::mul(255.0, c), 0x01E0_4454);
        rgba(fx, q + 0x10, [0xFF, 0xE8, 0xD8], a as u8);
        let f20 = fx.mem.f(r + 8);
        for s in 0..4u32 {
            for m in 0..4u32 {
                let c = sp + C0 + 0x10 * m;
                fx.vu_scale4(c, D + 0xACF0 + 0x40 * s + 0x10 * m, f20);
                fx.vu_apply_to(c, sp + TURN, c);
                fx.vu_add4(sp + V0 + 0x10 * m, c, r + 0x10);
            }
            if crate::fxlib2::project(fx, sp + V0, PROJ_OUT, sp + M, q, sp + WOUT) {
                far_z(fx, q + 0x20, &[q + 0x30, q + 0x40, q + 0x50]);
                fx.submit(q, OT, 0);
            }
        }
        let f20 = fx.mem.f(r + 8);
        corners(fx, sp, ee::mul(1.5, f20), true);
        let age = fx.mem.i32(r + 4);
        let c = fx.tcos((age << 14) >> 3);
        let a = fix(fx, ee::mul(255.0, c), 0x01E0_46AC);
        rgba(fx, sp + DESC + 0x10, [0xFF, 0xE8, 0xD8], a as u8);
        words(fx, sp + W, r + 0x10);
        fx.vu_add4(sp + V0, sp + C0, sp + W);
        fx.vu_add4(sp + V0 + 0x10, sp + C0 + 0x10, sp + W);
        far_sprite(fx, sp);
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 9 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The quad's STs (four corners, (s, t) each).
fn quad_st(fx: &mut Fx, sp: u32, st: [u32; 8]) {
    for (k, o) in [0x688u32, 0x68C, 0x6A0, 0x6A4, 0x6B8, 0x6BC, 0x6D0, 0x6D4].into_iter().enumerate() {
        fx.mem.write_u32(sp + o, st[k]);
    }
}

/// The camera pass (D+0xAE30 from 0x3C for 0x15 updates) [F
/// 0x1E04864..0x1E04D10]: a 1.5-high quad of texture D+0x1C at node 0x28
/// (moved -side in z), alpha 128 (0.5 + 0.5 (t % 2)) / 3; then in a
/// packet layer the target drawn with its own camera (0x1F82B0 with
/// b848's camera_pre / camera_post on D+0xAE40, the distance D+0xC, D at
/// 0x1E05100) and the same quad mirrored, alpha 128 (0.5 + 0.5 (t % 2)),
/// clamped as the lens.
fn camera_pass(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x3C {
        fx.mem.write_u8(D + 0xAE30, 1);
        fx.mem.write_u32(D + 0xAE34, 0);
    }
    if fx.mem.read_u8(D + 0xAE30) != 1 {
        return;
    }
    let d = sp + QUAD;
    head(fx, d, 0x34, 0xC, 0x4214_2142_1421, 0);
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    let x = ee::mul(1.5, side);
    let c = sp + C0;
    vec4(fx, c, [ee::neg(x), 1.5, 0.0, 1.0]);
    vec4(fx, c + 0x10, [x, 1.5, 0.0, 1.0]);
    vec4(fx, c + 0x20, [ee::neg(x), -1.5, 0.0, 1.0]);
    vec4(fx, c + 0x30, [x, -1.5, 0.0, 1.0]);
    fx.camera_vp_to(sp + M);
    fx.identity(sp + M2);
    words(fx, sp + W, D + 0x140);
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    let z = ee::add(fx.mem.f(sp + W + 8), ee::mul(-1.0, side));
    fx.mem.setf(sp + W + 8, z);
    fx.translate_rows(sp + M2, sp + M2, sp + W);
    crate::blib::exe::mul_mem(fx, sp + M, sp + M2);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    const ONE: u32 = 0x3F80_0000;
    quad_st(fx, sp, [ONE, 0, 0, 0, ONE, ONE, 0, ONE]);
    let t = fx.mem.i32(D + 0xAE34);
    let h = ee::add(0.5, ee::mul(0.5, ee::from_i32(rem2(t, 2))));
    let a = fix(fx, ee::div(ee::mul(128.0, h), 3.0), 0x01E0_4A90);
    quad_colour(fx, sp, [0x80; 3], a as u8);
    let m = fx.read_m4(sp + M);
    // a0..a3 at the TEXFLUSH after the answered 0x2078A0: as the submit
    // (or the rejected projection) leaves them.
    let regs = if fx.project_desc(c, PROJ_OUT, Some(&m), d, sp + WOUT) {
        fx.submit(d, OT, 0);
        [d, OT, 0, d]
    } else {
        [c, PROJ_OUT, sp + M, d]
    };
    let layer = fx.layer_begin();
    fx.texflush(regs);
    fx.mem.write_u32(D_STATIC, D);
    let h = fx.mem.i32(D + 8);
    let (side, slot) = (h >> 8, h & 0xFF);
    let i = if (0..3).contains(&side) && slot < 0xB { fx.object_index(side, slot) as u32 } else { 0 };
    let hand = std::cell::Cell::new([i, 0, 0]);
    let mut pcb = |fx: &mut Fx, p: u32| {
        let (v, r) = crate::b848::camera_pre(fx, p, hand.get()[0], SAVED, D + 0xC, 0xC0, 0x30);
        hand.set(r);
        v
    };
    let mut qcb = |fx: &mut Fx, p: u32| {
        let (v, r) = crate::b848::camera_post(fx, p, hand.get(), SAVED);
        hand.set(r);
        v
    };
    let (_, r) = fx.unit_target_pre_post(h, 1, sp + 0x6E8, sp + 0x6F0, (0x01E0_4F10, &mut pcb), Some((0x01E0_4FD0, &mut qcb)));
    fx.texflush(r);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let t = fx.mem.i32(D + 0xAE34);
    let h = ee::add(0.5, ee::mul(0.5, ee::from_i32(rem2(t, 2))));
    let a = fix(fx, ee::mul(128.0, h), 0x01E0_4BF4);
    quad_colour(fx, sp, [0x80; 3], a as u8);
    quad_st(fx, sp, [0, 0, ONE, 0, 0, ONE, ONE, ONE]);
    let m = fx.read_m4(sp + M);
    if fx.project_desc(c, PROJ_OUT, Some(&m), d, sp + WOUT) {
        fx.submit(d, 0, 0);
    }
    fx.layer_end(layer, OT, 0);
    let t = fx.mem.i32(D + 0xAE34) + 1;
    fx.mem.set32(D + 0xAE34, t);
    if t >= 0x15 {
        fx.mem.write_u8(D + 0xAE30, 0);
    }
}

/// The ribbon from node 5 (taken on 0x82) to the target (1 / 6 of the way
/// each update), its points fading by 5 [F 0x1E04D10..0x1E04E00].
fn ribbon(fx: &mut Fx) {
    let n = fx.mem.i32(D);
    if n == 0x82 {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x50);
        fx.node_position(h, s, RIB_AT);
    }
    if fx.mem.i32(D) >= 0x82 {
        fx.ribbon_push(RIBBON, RIB_PTS, RIB_AT);
        let k = i32::from(fx.mem.read_u8(D + 0xB180));
        fx.approach_by(RIB_AT, D + 0x20, k);
    }
    // Its frame (the TRS matrix at entry sp - 0x40) is the ribbon's
    // missing-texture TEX0 / TEX1 residue.
    fx.xform_world_saving(RIB_X, 0x01E0_4D90, [Some(D), Some(OT), None, None]);
    if fx.mem.i32(D) >= 0x82 {
        fx.ribbon_draw(RIBBON, RIB_PTS, RIB_X);
        let mut k = 0;
        while k < fx.mem.i16(RIBBON + 0x2A) {
            fx.step_down(RIB_PTS + 0xC + 16 * k as u32, 0, 5);
            k += 1;
        }
    }
}
