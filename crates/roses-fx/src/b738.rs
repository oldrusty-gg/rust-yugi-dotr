//! BMODULE b738 (card 501), hand-ported from the record (linked at
//! 0x1E00000, the entry 0x1E00080 in a 0x870 frame, data area a0 =
//! 0x1ED0000 kept in the stack slot sp + 0xCC, 0xC1C0 bytes; one
//! unit-target post callback 0x1E05B00 (0x220 frame) with its static D at
//! 0x1E06080) [F]. Unoptimised code like b216's
//! (s3 = the OT, the sprite descriptor sp + 0x7D8 rebuilt before each
//! pool). Every update the camera turn (sp + 0x5F0), node 0x33 (kind 1,
//! D+0x30), nodes 0x36 / 0x38 (D+0x50 / 0x60), the self point (D+0x70)
//! and the screen points of D+0x70 / D+0x90 (the target's position) into
//! D+0x80 / D+0xA0; the travelling point D+0xB0 = D+0x30 + (D+0x90 -
//! D+0x30) clamp(n - 0x61, 0, 0x21) / 33 and the scale D+0xC0 = 1 + 4.5
//! clamp(n - 0x82, 0, 0x30) / 48 (f21 = 1 - that ratio, kept for the
//! glow and shards). Then: on 0x82 a window flash (FF, E8, E0) for 0x19
//! updates (D+0xC1B0) and a frame-copy fan about D+0xA0 for 0x31 updates
//! (D+0x5B20); on 0x61 one emitter (D+0xE0) with a mote record (D+0x2B30,
//! 3 x 0x350) moving node 0x33 -> the target, spawning into the sub-pools
//! D+0x120 (16 x 0x60) / D+0x720 (16 x 0x240); on 1..0x63 a spark record
//! a frame (D+0x3520, 32 x 0x130); on 0x82 a burst at the target
//! (D+0x5B30, 3 x 0x10E0); on 0x16..0x2F a node glow at node 0x36 / 0x38
//! (the one on the camera's side); from 0 the attacker pass (D+0x8DD0,
//! 0xAC updates: 0x1F8280 with the sphere callback, then a fan about
//! D+0x80); on 0x61 a glow at D+0xB0 (D+0xBF20, 0x52 updates); on
//! 0x62..0xB1 a shard record a frame (D+0xA020, 16 x 0x1F0: 6 shards of
//! 10 quads by 0x205210 in the colour table D+0x9FF0); on 0x61 a beam
//! record (D+0xBF30, 0x280: 8 beams of up to 16 quads to the target). The
//! stage light 0x80 - 96 clamp(n, 0, 0x20) clamp(0xD2 - n, 0, 0x20) /
//! 1024. Returns 1 on 0x78, 6 on 0x83..0xB1, 2 on 0xB2, 8 once the count
//! reaches 0xD3.
use crate::b182::{alpha, far, head, rgba, uv};
use crate::b424::{camera, fan, fan_desc};
use crate::b440::{copy8, div12, fix, fixs, frame_mask_packet, rem2, vec4, words};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::far_xy;

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const ONE: u32 = 0x3F80_0000;
const CB: u32 = 0x01E0_5B00;
const D_STATIC: u32 = 0x01E0_6080;
const SHARD_CLAMP: u64 = 0x0000_01FC_0123_40BA;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x870);
    fx.mem.write_u32(sp + 0xCC, D);
    fx.mem.write_u32(sp + 0xD0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(0x870);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xC1C0);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let side = fx.unit_side(h);
    fx.mem.set32(D + 0xC, side);
    for k in 0..4u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let r = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x20, r);
    for (o, node, kind) in [(0x24u32, 0x33, 1), (0x40, 0x36, 0), (0x44, 0x38, 0)] {
        let h = fx.mem.i32(D + 4);
        let s = fx.request_node(h, node, kind);
        fx.mem.set32(D + o, s);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x90);
    // The sphere: 17 rows of 17 points, radius 80 (D+0x8DE0, rows 0x110).
    for r in 0..0x11u32 {
        let s2 = (r << 15) as i32;
        let c = fx.tcos(s2 >> 4);
        let f21 = ee::mul(80.0, c);
        let a = Fx::powf2(80.0);
        let b = Fx::powf2(f21);
        let f20 = fx.sqrtf(ee::sub(a, b));
        for k in 0..0x11u32 {
            let s6 = ((k << 16) as i32) >> 4;
            let p = D + 0x8DE0 + 0x110 * r + 0x10 * k;
            let s = fx.tsin(s6);
            fx.mem.setf(p, ee::mul(f20, s));
            fx.mem.setf(p + 4, f21);
            let c = fx.tcos(s6);
            fx.mem.setf(p + 8, ee::mul(f20, c));
            fx.mem.write_u32(p + 0xC, ONE);
        }
    }
    // The shards' colour table: 11 entries (D+0x9FF0).
    for k in 0..0xBu32 {
        let f21 = ee::div(ee::from_i32(k as i32), 10.0);
        let f20 = ee::sub(1.0, f21);
        let p = D + 0x9FF0 + 4 * k;
        let b = fix(fx, ee::mul(100.0, f20), 0x01E0_0464);
        fx.mem.write_u8(p, b as u8);
        let b = fix(fx, ee::mul(70.0, f20), 0x01E0_0480);
        fx.mem.write_u8(p + 1, b as u8);
        let f20 = ee::mul(255.0, f20);
        let b = fix(fx, ee::add(f20, ee::mul(128.0, f21)), 0x01E0_04B4);
        fx.mem.write_u8(p + 2, b as u8);
        let b = fix(fx, f20, 0x01E0_04C8);
        fx.mem.write_u8(p + 3, b as u8);
    }
    0
}

/// The screen point of `p` (0x204D80 through sp + 0x570 into sp + 0x860)
/// into `o`.
fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    let (mo, xo) = (sp + 0x570, sp + 0x860);
    let m = fx.read_m4(mo);
    fx.project_at(p, &m, xo, sp + 0x86C);
    let x = (i32::from(fx.mem.read_u16(xo)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(xo + 2)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(xo) << 8) >> 40;
    fx.mem.setf(o + 8, z as u32 as f32);
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

/// b424's camera (vp at sp + 0x570, identity at sp + 0x5B0).
fn cam(fx: &mut Fx, sp: u32) {
    camera(fx, sp + 0x570 - 0x250);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x5F0);
    let h = fx.mem.i32(D + 4);
    let s = fx.mem.i32(D + 0x24);
    fx.node_position(h, s, D + 0x30);
    for k in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x40 + 4 * k);
        fx.node_position(h, s, D + 0x50 + 0x10 * k);
    }
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x70);
    cam(fx, sp);
    screen_point(fx, sp, D + 0x70, D + 0x80);
    cam(fx, sp);
    screen_point(fx, sp, D + 0x90, D + 0xA0);

    // The travelling point D+0xB0 and the scale D+0xC0.
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(n - 0x61, 0, 0x21)), 33.0);
    let t = sp + 0x6B0;
    fx.vu_sub4(t, D + 0x90, D + 0x30);
    fx.vu_scale4(t, t, f20);
    fx.vu_add4(D + 0xB0, t, D + 0x30);
    let n = fx.mem.i32(D);
    let x = ee::from_i32(Fx::clamp_i(n - 0x82, 0, 0x30));
    let s = ee::add(1.0, ee::div(ee::mul(4.5, x), 48.0));
    for k in 0..3u32 {
        fx.mem.setf(D + 0xC0 + 4 * k, s);
    }
    fx.mem.write_u32(D + 0xCC, ONE);
    let n = fx.mem.i32(D);
    let x = ee::from_i32(Fx::clamp_i(n - 0x82, 0, 0x30));
    let f21 = ee::sub(1.0, ee::div(x, 48.0));

    flash(fx, sp);
    frame_fan(fx, sp);
    emitter(fx, sp);
    sparks(fx, sp);
    let hi = bursts(fx, sp);
    node_glow(fx, sp);
    attacker_pass(fx, sp, hi);
    glow(fx, sp, f21);
    shards(fx, sp, f21);
    beams(fx, sp);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0xD2 - n, 0, 0x20);
    let n = fx.mem.i32(D);
    let k = Fx::clamp_i(n, 0, 0x20);
    let l = ((k.wrapping_mul(-3) << 5).wrapping_mul(s0) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xD0);
    if n == 0x78 {
        ret = 1;
    } else {
        if (0x83..0xB2).contains(&n) {
            ret = 6;
        }
        if n == 0xB2 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xD3 {
        ret = 8;
    }
    fx.mem.set32(sp + 0xD0, ret);
    ret
}

/// x / 48 as the record's `mult 0x2AAAAAAB; sra 3` (+ the sign bit).
fn div48(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    let hi = fx.hilo().1;
    (hi >> 3).wrapping_add(((x as u32) >> 31) as i32)
}

/// The window flash on 0x82 (0x19 updates, D+0xC1B0) [F 0x1E00944..0x1E00AEC].
fn flash(fx: &mut Fx, sp: u32) {
    let fl = D + 0xC1B0;
    if fx.mem.i32(D) == 0x82 {
        fx.mem.write_u8(fl, 1);
        fx.mem.write_u32(fl + 4, 0);
    }
    if fx.mem.read_u8(fl) != 1 {
        return;
    }
    let t = sp + 0x830;
    head(fx, t, 6, 3, 0x441, 0);
    alpha(fx, t, 0);
    let age = fx.mem.i32(fl + 4);
    let q = ee::div(ee::from_i32(0x18 - age), 24.0);
    let a = fix(fx, ee::mul(128.0, q), 0x01E0_09FC);
    rgba(fx, t + 0x10, [0xFF, 0xE8, 0xE0], a as u8);
    far_xy(fx, t + 0x18, 0x7000, 0x7000);
    far_xy(fx, t + 0x20, 0x9000, 0x9000);
    fx.submit(t, OT, 0);
    let age = fx.mem.i32(fl + 4) + 1;
    fx.mem.set32(fl + 4, age);
    if age >= 0x19 {
        fx.mem.write_u8(fl, 0);
    }
}

const FAN_T: [u32; 12] = [
    0x01E0_0CC4, 0x01E0_0CD4, 0x01E0_0D18, 0x01E0_0D24, 0x01E0_0D68, 0x01E0_0D74, 0x01E0_0E3C, 0x01E0_0E4C, 0x01E0_0EB8, 0x01E0_0EC4, 0x01E0_0F30,
    0x01E0_0F3C,
];
const FAN_A: [u32; 12] = [
    0x01E0_43A4, 0x01E0_43B4, 0x01E0_4404, 0x01E0_4410, 0x01E0_4460, 0x01E0_446C, 0x01E0_4540, 0x01E0_4550, 0x01E0_45C0, 0x01E0_45CC, 0x01E0_4640,
    0x01E0_464C,
];

/// The frame-copy fan about the target's screen point (0x31 updates from
/// 0x82): 8 fans 1 + 0.2 (8 - j) / 8, white, alpha 64 (0x30 - age) / 48
/// [F 0x1E00AF0..0x1E01008].
fn frame_fan(fx: &mut Fx, sp: u32) {
    let ff = D + 0x5B20;
    if fx.mem.i32(D) == 0x82 {
        fx.mem.write_u8(ff, 1);
        fx.mem.write_u32(ff + 4, 0);
    }
    if fx.mem.read_u8(ff) != 1 {
        return;
    }
    let d = sp + 0x760;
    let a = fan_desc(fx, d - 0x4C0, 0);
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, a, 1, d + 0x60, d + 0x68);
    fx.texflush(regs);
    let age = fx.mem.i32(ff + 4);
    let al = div48(fx, (0x30 - age) << 6);
    rgba(fx, d + 0x10, [0xFF; 3], al as u8);
    copy8(fx, d + 0x28, d + 0x10);
    copy8(fx, d + 0x40, d + 0x10);
    for j in 0..8i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(8 - j)), 8.0));
        fan(fx, d, D + 0xA0, sp + 0x670, s, 0, &FAN_T);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(ff + 4) + 1;
    fx.mem.set32(ff + 4, age);
    if age >= 0x31 {
        fx.mem.write_u8(ff, 0);
    }
}

/// The sprite descriptor sp + 0x7D8 (prim 0x26 | 0x800, 5 registers).
fn sprite_head(fx: &mut Fx, d: u32) {
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
}

fn texture(fx: &mut Fx, d: u32, at: u32) {
    let rh = fx.mem.read_u32(D + at);
    fx.desc_texture(d, rh, 1);
}

/// UVs (0x600, 0) .. (0x800, 0x200).
fn uv_fixed(fx: &mut Fx, d: u32) {
    uv(fx, d + 0x18, 0x600);
    uv(fx, d + 0x1A, 0);
    uv(fx, d + 0x28, 0x800);
    uv(fx, d + 0x2A, 0x200);
}

/// UVs of atlas cell `c` (4 x 2 cells of 0x200).
fn uv_cell(fx: &mut Fx, d: u32, c: i32) {
    let (u, w) = (c & 3, c >> 2);
    uv(fx, d + 0x18, ((u << 9) & 0x3FFF) as u32);
    uv(fx, d + 0x1A, ((w << 9) & 0x3FFF) as u32);
    uv(fx, d + 0x28, ((((u << 5) + 0x20) << 4) & 0x3FFF) as u32);
    uv(fx, d + 0x2A, ((((w << 5) + 0x20) << 4) & 0x3FFF) as u32);
}

/// The record's working slots: the corners (sp + 0x630 / 0x640), the
/// placed corners (0x670 / 0x680), the centre (0x6B0), the camera (0x570
/// / 0x5B0), the turn (0x5F0), the descriptor (0x7D8), w (0x86C).
#[derive(Clone, Copy)]
struct S {
    c: u32,
    p: u32,
    m: u32,
    turn: u32,
    at: u32,
    d: u32,
    w: u32,
}

fn slots(sp: u32) -> S {
    S { c: sp + 0x630, p: sp + 0x670, m: sp + 0x570, turn: sp + 0x5F0, at: sp + 0x6B0, d: sp + 0x7D8, w: sp + 0x86C }
}

/// The corners (-s, -s) / (s, s), turned by the camera when `turn`.
fn corners(fx: &mut Fx, s: &S, r: f32, turn: bool) {
    let n = (-r).to_bits();
    vec4(fx, s.c, [n, n, 0, ONE]);
    vec4(fx, s.c + 0x10, [r.to_bits(), r.to_bits(), 0, ONE]);
    if turn {
        fx.vu_apply_to(s.c, s.turn, s.c);
        fx.vu_apply_to(s.c + 0x10, s.turn, s.c + 0x10);
    }
}

/// How a projected sprite is submitted: far Z with key 0, key 0, or keyed
/// by 0x1A2C70(w) from the return address.
#[derive(Clone, Copy)]
enum K {
    Far,
    Zero,
    W(u32),
}

fn submit(fx: &mut Fx, s: &S, k: K) {
    match k {
        K::Far => {
            far(fx, s.d + 0x20, &[s.d + 0x30]);
            fx.submit(s.d, OT, 0);
        }
        K::Zero => fx.submit(s.d, OT, 0),
        K::W(ra) => {
            let w = fx.mem.f(s.w);
            let key = fix(fx, w, ra);
            fx.submit(s.d, OT, key as i32);
        }
    }
}

/// The corners about the centre projected and submitted.
fn draw(fx: &mut Fx, s: &S, k: K) {
    fx.vu_add4(s.p, s.c, s.at);
    fx.vu_add4(s.p + 0x10, s.c + 0x10, s.at);
    if crate::fxlib2::project(fx, s.p, PROJ_OUT, s.m, s.d, s.w) {
        submit(fx, s, k);
    }
}

/// The corners themselves projected (a matrix placed sprite).
fn draw_c(fx: &mut Fx, s: &S, k: K) {
    if crate::fxlib2::project(fx, s.c, PROJ_OUT, s.m, s.d, s.w) {
        submit(fx, s, k);
    }
}

/// The centre = `vv` x `k` + `p`, drawn.
fn mote(fx: &mut Fx, s: &S, vv: u32, k: f32, p: u32, key: K) {
    fx.vu_scale4(s.at, vv, k);
    fx.vu_add4(s.at, s.at, p);
    draw(fx, s, key);
}

fn r_rem(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let x = fx.rand_from(s0, ra) >> 23;
    rem2(x, m)
}

/// 255 x >> `sh` as the record's `sll 8; subu; sra`.
fn a255(x: i32, sh: u32) -> u8 {
    (((x << 8) - x) >> sh) as u8
}

/// The emitter (D+0xE0: +4 age, +8 life, +0xC / +0x10 the sub-pool
/// cursors, +0x20 position, +0x30 velocity) with its mote records (D+0x2B30,
/// 3 x 0x350), the sub-pools A (D+0x120, 16 x 0x60) / B (D+0x720, 16 x
/// 0x240: 32 motes) and the motes drawn [F 0x1E0100C..0x1E02774].
fn emitter(fx: &mut Fx, sp: u32) {
    let s = slots(sp);
    let d = s.d;
    let ci = D + 0x2B20;
    if fx.mem.i32(D) == 0x61 {
        let e = D + 0x40 * fx.mem.read_u32(D + 0xD4);
        fx.mem.write_u8(e + 0xE0, 1);
        fx.mem.write_u32(e + 0xE4, 0);
        fx.mem.set32(e + 0xE8, 0x21);
        let j = fx.mem.i32(ci);
        fx.mult(0x350, j);
        let c = ci + 0x10 + 0x350 * j as u32;
        fx.mem.write_u8(c, 1);
        fx.mem.write_u32(c + 4, 0);
        words(fx, e + 0x100, D + 0x30);
        words(fx, c + 0x10, D + 0x30);
        fx.random_table_vec(s.at, 0.0);
        fx.vu_add4(s.at, s.at, D + 0x90);
        let x = fx.mem.read_u32(e + 0x100);
        fx.mem.write_u32(s.at, x);
        fx.vu_sub4(e + 0x110, s.at, e + 0x100);
        for k in 0..0x10u32 {
            let x = r_rem(fx, D, 0x01E0_1118, 8);
            fx.mem.write_u8(c + 0x20 + k, x as u8);
            fx.random_table_vec(c + 0x30 + 0x10 * k, 1.0);
        }
        for k in 0..0x20u32 {
            let x = r_rem(fx, D, 0x01E0_1170, 4);
            fx.mem.write_u8(c + 0x130 + k, x as u8);
            fx.random_table_vec(c + 0x150 + 0x10 * k, 1.0);
        }
        let x = fx.mem.i32(D + 0xD0) + 1;
        fx.mem.set32(D + 0xD0, x);
        let x = fx.mem.i32(D + 0xD4) + 1;
        fx.mem.set32(D + 0xD4, if x > 0 { 0 } else { x });
        let x = fx.mem.i32(ci) + 1;
        fx.mem.set32(ci, if x < 3 { x } else { 0 });
    }

    // The emitter drawn (keyed), spawning into its sub-pools.
    sprite_head(fx, d);
    alpha(fx, d, 1);
    texture(fx, d, 0x10);
    fx.desc_clamp(d, 5);
    cam(fx, sp);
    uv(fx, d + 0x18, 0x400);
    uv(fx, d + 0x1A, 0);
    uv(fx, d + 0x28, 0x600);
    uv(fx, d + 0x2A, 0x200);
    corners(fx, &s, 20.0, true);
    let s0 = d + 0x28;
    let e = D;
    if fx.mem.read_u8(e + 0xE0) != 0 {
        rgba(fx, d + 0x10, [0xC8, 0x78, 0xFF], 0xFF);
        let f20 = ee::div(ee::from_i32(fx.mem.i32(e + 0xE4)), ee::from_i32(fx.mem.i32(e + 0xE8)));
        let a0 = fx.mem.i32(e + 0xEC);
        fx.mem.set32(e + 0xEC, if a0 + 1 < 0x10 { a0 + 1 } else { 0 });
        let a1 = fx.mem.i32(e + 0xF0);
        fx.mem.set32(e + 0xF0, if a1 + 1 < 0x10 { a1 + 1 } else { 0 });
        let sa = D + 0x120 + 0x60 * a0 as u32;
        let sb = D + 0x720 + 0x240 * a1 as u32;
        fx.vu_scale4(s.at, e + 0x110, f20);
        fx.vu_add4(s.at, s.at, e + 0x100);
        draw(fx, &s, K::W(0x01E0_1540));
        fx.mem.write_u8(sa, 1);
        fx.mem.write_u32(sa + 4, 0);
        words(fx, sa + 0x10, s.at);
        for k in 0..4u32 {
            let x = r_rem(fx, s0, 0x01E0_15A8, 8);
            fx.mem.write_u8(sa + 8 + k, x as u8);
            fx.random_table_vec(sa + 0x20 + 0x10 * k, f(0x3F4C_CCCD));
        }
        fx.mem.write_u8(sb, 1);
        fx.mem.write_u32(sb + 4, 0);
        words(fx, sb + 0x30, s.at);
        for k in 0..0x20u32 {
            let x = fx.rand_from(s0, 0x01E0_1640) >> 23;
            let (_, r) = fx.div(x, 3);
            fx.mem.write_u8(sb + 8 + k, r as u8);
            fx.random_table_vec(sb + 0x40 + 0x10 * k, 1.5);
        }
        let age = fx.mem.i32(e + 0xE4) + 1;
        fx.mem.set32(e + 0xE4, age);
        if fx.mem.i32(e + 0xE8) < age {
            fx.mem.write_u8(e + 0xE0, 0);
        }
    }

    // Sub-pool A (texture +0x14, 4 motes a record, keyed).
    sprite_head(fx, d);
    alpha(fx, d, 1);
    texture(fx, d, 0x14);
    fx.desc_clamp(d, 5);
    cam(fx, sp);
    for j in 0..0x10u32 {
        let r = D + 0x120 + 0x60 * j;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let k = ee::mul(12.0, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(0x10 - age)), 16.0)));
        corners(fx, &s, k, true);
        let age = fx.mem.i32(r + 4);
        rgba(fx, d + 0x10, [0x64, 0x46, 0xFF], (((0x10 - age) << 7) >> 4) as u8);
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, 0x10, 1);
        let f20 = ee::add(f(0x3ECC_CCCD), ee::mul(f(0x3F19_999A), t));
        for k in 0..4u32 {
            let c = i32::from(fx.mem.read_u8(r + 8 + k));
            uv_cell(fx, d, c);
            mote(fx, &s, r + 0x20 + 0x10 * k, f20, r + 0x10, K::W(0x01E0_1A38));
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r, 0);
        }
    }

    // Sub-pool B (texture +0x10, 32 motes a record, keyed).
    sprite_head(fx, d);
    alpha(fx, d, 1);
    texture(fx, d, 0x10);
    fx.desc_clamp(d, 5);
    cam(fx, sp);
    for j in 0..0x10u32 {
        let r = D + 0x720 + 0x240 * j;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let k = ee::mul(1.5, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(0x10 - age)), 16.0)));
        corners(fx, &s, k, true);
        let age = fx.mem.i32(r + 4);
        rgba(fx, d + 0x10, [0x64, 0x46, 0xFF], a255(0x10 - age, 4));
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, 0x10, 1);
        let f20 = ee::add(f(0x3DCC_CCCD), ee::mul(f(0x3F66_6666), t));
        for k in 0..0x20u32 {
            let c = i32::from(fx.mem.read_u8(r + 8 + k));
            uv_cell(fx, d, c);
            mote(fx, &s, r + 0x40 + 0x10 * k, f20, r + 0x30, K::W(0x01E0_1DF8));
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r, 0);
        }
    }

    // The mote records: a glow, 16 big motes, 32 small ones (9 updates).
    sprite_head(fx, d);
    cam(fx, sp);
    for j in 0..3u32 {
        let c = ci + 0x10 + 0x350 * j;
        if fx.mem.read_u8(c) == 0 {
            continue;
        }
        alpha(fx, d, 1);
        texture(fx, d, 0x10);
        fx.desc_clamp(d, 5);
        uv_fixed(fx, d);
        corners(fx, &s, 100.0, true);
        let age = fx.mem.i32(c + 4);
        let t = fx.tan_falloff_p(age, 8, 1);
        let a = fix(fx, ee::mul(255.0, ee::sub(1.0, t)), 0x01E0_2060);
        rgba(fx, d + 0x10, [0x64, 0x46, 0xFF], a as u8);
        words(fx, s.at, c + 0x10);
        draw(fx, &s, K::Far);

        alpha(fx, d, 0);
        texture(fx, d, 0x14);
        fx.desc_clamp(d, 5);
        let age = fx.mem.i32(c + 4);
        let k = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(age)), 8.0)));
        corners(fx, &s, k, true);
        let age = fx.mem.i32(c + 4);
        rgba(fx, d + 0x10, [0x7D, 0x64, 0xFF], (((8 - age) << 7) >> 3) as u8);
        let age = fx.mem.i32(c + 4);
        let f22 = fx.tan_falloff_p(age, 8, 1);
        for k in 0..0x10u32 {
            let cell = i32::from(fx.mem.read_u8(c + 0x20 + k));
            uv_cell(fx, d, cell);
            mote(fx, &s, c + 0x30 + 0x10 * k, f22, c + 0x10, K::W(0x01E0_23F8));
        }

        alpha(fx, d, 1);
        texture(fx, d, 0x10);
        fx.desc_clamp(d, 5);
        let age = fx.mem.i32(c + 4);
        let k = ee::mul(2.0, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(8 - age)), 8.0)));
        corners(fx, &s, k, true);
        let age = fx.mem.i32(c + 4);
        rgba(fx, d + 0x10, [0x64, 0x46, 0xFF], a255(8 - age, 3));
        let age = fx.mem.i32(c + 4);
        let f22 = fx.tan_falloff_p(age, 8, 1);
        for k in 0..0x20u32 {
            let cell = i32::from(fx.mem.read_u8(c + 0x130 + k));
            uv_cell(fx, d, cell);
            mote(fx, &s, c + 0x150 + 0x10 * k, f22, c + 0x10, K::W(0x01E0_26C0));
        }
        let age = fx.mem.i32(c + 4) + 1;
        fx.mem.set32(c + 4, age);
        if age >= 9 {
            fx.mem.write_u8(c, 0);
        }
    }
}

/// The spark records (32 x 0x130: +8 16 cells, +0x20 position, +0x30 16
/// vectors 0.7), one a frame on 1..0x63 at node 0x33 [F 0x1E02778..0x1E02E74].
fn sparks(fx: &mut Fx, sp: u32) {
    let s = slots(sp);
    let d = s.d;
    let n = fx.mem.i32(D);
    let s0 = d + 0x28;
    if n > 0 && n < 0x64 {
        let r = D + 0x3520 + 0x130 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x20, D + 0x30);
        for k in 0..0x10u32 {
            let x = r_rem(fx, s0, 0x01E0_2800, 4);
            fx.mem.write_u8(r + 8 + k, x as u8);
            fx.random_table_vec(r + 0x30 + 0x10 * k, f(0x3F33_3333));
        }
    }
    sprite_head(fx, d);
    fx.desc_clamp(d, 5);
    cam(fx, sp);
    for i in 0..0x20u32 {
        let r = D + 0x3520 + 0x130 * i;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        texture(fx, d, 0x10);
        alpha(fx, d, 1);
        let age = fx.mem.i32(r + 4);
        let k = ee::mul(0.5, ee::add(f(0x3E4C_CCCD), ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(0x20 - age)), 32.0)));
        corners(fx, &s, k, true);
        let age = fx.mem.i32(r + 4);
        let c = fx.tcos((age << 14) >> 5);
        let a = fix(fx, ee::mul(255.0, c), 0x01E0_29CC);
        rgba(fx, d + 0x10, [0x64, 0x46, 0xFF], a as u8);
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, 0x20, 1);
        let f20 = ee::add(f(0x3DCC_CCCD), ee::mul(f(0x3F66_6666), t));
        for k in 0..0x10u32 {
            let cell = i32::from(fx.mem.read_u8(r + 8 + k));
            uv_cell(fx, d, cell);
            mote(fx, &s, r + 0x30 + 0x10 * k, f20, r + 0x20, K::Zero);
        }
        texture(fx, d, 0x18);
        alpha(fx, d, 1);
        let age = fx.mem.i32(r + 4);
        let k = ee::mul(5.0, ee::add(f(0x3E4C_CCCD), ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(age)), 32.0)));
        corners(fx, &s, k, true);
        let age = fx.mem.i32(r + 4);
        let c = fx.tcos((age << 14) >> 5);
        let a = fix(fx, ee::mul(64.0, c), 0x01E0_2C64);
        rgba(fx, d + 0x10, [0x64, 0x46, 0xFF], a as u8);
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, 0x20, 1);
        let f20 = ee::mul(f(0xBF33_3333), ee::add(f(0x3DCC_CCCD), ee::mul(f(0x3F66_6666), t)));
        for k in 0..0x10u32 {
            let cell = i32::from(fx.mem.read_u8(r + 8 + k)) + 4;
            uv_cell(fx, d, cell);
            mote(fx, &s, r + 0x30 + 0x10 * k, f20, r + 0x20, K::Zero);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The bursts at the target (3 x 0x10E0 at D+0x5B30, index D+0x5B28: +4
/// age from 1, +8 64 halfword cells, +0x90 position, +0xA0 64 vectors 2.7,
/// +0x4A0 128 vectors 3.75, +0xCA0 64 byte cells 5..7, +0xCE0 64 vectors
/// 2.7), 0x31 updates [F 0x1E02E78..0x1E03DCC]. Returns the upper half of
/// t0 after the last 0x188420 (`lq t0, 0x10(a1)`: the camera turn's row 1
/// z / w at the glow), 0 when no burst is drawn.
fn bursts(fx: &mut Fx, sp: u32) -> u64 {
    let mut hi = 0;
    let s = slots(sp);
    let d = s.d;
    let s0 = d + 0x28;
    if fx.mem.i32(D) == 0x82 {
        fx.mem.set32(D + 0x5B28, 0);
        let g = D + 0x5B30;
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 1);
        vec4(fx, g + 0x90, [0, 0, 0, ONE]);
        fx.vu_add4(g + 0x90, g + 0x90, D + 0x90);
        for k in 0..0x40u32 {
            let x = r_rem(fx, s0, 0x01E0_2FE8, 8);
            fx.mem.write_u16(g + 8 + 2 * k, x as u16);
            fx.random_table_vec(g + 0xA0 + 0x10 * k, f(0x402C_CCCD));
        }
        for k in 0..0x80u32 {
            fx.random_table_vec(g + 0x4A0 + 0x10 * k, 3.75);
        }
        for k in 0..0x40u32 {
            let x = fx.rand_from(s0, 0x01E0_30DC) >> 23;
            let (_, r) = fx.div(x, 3);
            fx.mem.write_u8(g + 0xCA0 + k, (r + 5) as u8);
            fx.random_table_vec(g + 0xCE0 + 0x10 * k, f(0x402C_CCCD));
        }
    }
    for i in 0..3u32 {
        let g = D + 0x5B30 + 0x10E0 * i;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let pos = g + 0x90;
        // Big sparks (texture +0x18), keyed.
        sprite_head(fx, d);
        texture(fx, d, 0x18);
        fx.desc_clamp(d, 5);
        alpha(fx, d, 1);
        cam(fx, sp);
        let age = fx.mem.i32(g + 4);
        let k = ee::mul(20.0, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(age)), 48.0)));
        corners(fx, &s, k, true);
        let age = fx.mem.i32(g + 4);
        let a = div48(fx, (0x30 - age).wrapping_mul(100));
        rgba(fx, d + 0x10, [0x64, 0x46, 0xFF], a as u8);
        let age = fx.mem.i32(g + 4);
        let f23 = fx.tan_falloff_p(age, 0x30, 1);
        for k in 0..0x40u32 {
            let cell = i32::from(fx.mem.read_u8(g + 0xCA0 + k));
            uv_cell(fx, d, cell);
            mote(fx, &s, g + 0xCE0 + 0x10 * k, f23, pos, K::W(0x01E0_3480));
        }
        // Two-blend motes (texture +0x14), keyed.
        sprite_head(fx, d);
        texture(fx, d, 0x14);
        fx.desc_clamp(d, 5);
        cam(fx, sp);
        let age = fx.mem.i32(g + 4);
        let k = ee::mul(15.0, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(age)), 48.0)));
        corners(fx, &s, k, true);
        let age = fx.mem.i32(g + 4);
        let x = 0x30 - age;
        let ao = div48(fx, (x << 8) - x) as u8;
        fx.mem.write_u8(sp + 0x550, ao);
        let ae = div48(fx, x << 7) as u8;
        fx.mem.write_u8(sp + 0xF0, ae);
        for k in 0..0x40u32 {
            let age = fx.mem.i32(g + 4);
            let mut f23 = fx.tan_falloff_p(age, 0x30, 1);
            if rem2(k as i32, 2) == 0 {
                f23 = ee::mul(f23, f(0x3F4C_CCCD));
                alpha(fx, d, 2);
                rgba(fx, d + 0x10, [0x80, 0xFF, 0x40], ae);
            } else {
                alpha(fx, d, 1);
                rgba(fx, d + 0x10, [0x64, 0x46, 0xFF], ao);
            }
            fx.vu_scale4(s.at, g + 0xA0 + 0x10 * k, f23);
            fx.vu_add4(s.at, s.at, pos);
            let cell = i32::from(fx.mem.read_u16(g + 8 + 2 * k));
            uv_cell(fx, d, cell);
            draw(fx, &s, K::W(0x01E0_3828));
        }
        // The glow (texture +0x10) in a scaled matrix at the burst, keyed.
        sprite_head(fx, d);
        texture(fx, d, 0x10);
        fx.desc_clamp(d, 5);
        alpha(fx, d, 1);
        fx.camera_vp_to(s.m);
        hi = fx.mem.read_u64(s.turn + 0x18);
        fx.translate_rows(sp + 0x5B0, s.turn, pos);
        let age = fx.mem.i32(g + 4);
        let t = fx.tan_falloff_p(age, 0x30, 1);
        let k = ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), t));
        vec4(fx, s.at, [k.to_bits(), k.to_bits(), k.to_bits(), ONE]);
        fx.scale_rows(sp + 0x5B0, s.at);
        crate::blib::exe::mul_mem(fx, s.m, sp + 0x5B0);
        let age = fx.mem.i32(g + 4);
        let a = fix(fx, ee::mul(200.0, ee::div(ee::from_i32(0x30 - age), 48.0)), 0x01E0_3980);
        rgba(fx, d + 0x10, [0x64, 0x46, 0xFF], a as u8);
        uv_fixed(fx, d);
        corners(fx, &s, 200.0, false);
        draw_c(fx, &s, K::W(0x01E0_3A78));
        // 128 streaks of six sprites (texture +0x10), keyed by w.
        sprite_head(fx, d);
        texture(fx, d, 0x10);
        fx.desc_clamp(d, 5);
        alpha(fx, d, 1);
        uv_fixed(fx, d);
        cam(fx, sp);
        corners(fx, &s, 1.0, true);
        let age = fx.mem.i32(g + 4);
        let f23 = ee::div(ee::from_i32(0x30 - age), 48.0);
        let age = fx.mem.i32(g + 4);
        let f24 = fx.tan_falloff_p(age, 0x30, 1);
        for j in 0..0x80u32 {
            for s4 in 0..6i32 {
                let q = div12(fx, (s4 + 6) * 255);
                let a = fix(fx, ee::mul(f23, ee::from_i32(q)), 0x01E0_3C6C);
                rgba(fx, d + 0x10, [0xC8, 0x64, 0xFF], a as u8);
                let k = ee::add(f24, ee::mul(f(0x3BE5_6042), ee::from_i32(s4)));
                fx.vu_scale4(s.at, g + 0x4A0 + 0x10 * j, k);
                fx.vu_add4(s.at, s.at, pos);
                draw(fx, &s, K::W(0x01E0_3D2C));
            }
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x31 {
            fx.mem.write_u8(g, 0);
        }
    }
    hi
}

/// The node glow on 0x16..0x2F (no record): a sprite 8 at node 0x36 or
/// 0x38 (the one on the camera eye's side), alpha 255 clamp(n - 0x15, 0, 8)
/// clamp(0x30 - n, 0, 8) / 64 (0.8 + 0.2 (n % 2)), far Z [F
/// 0x1E03DD0..0x1E04160].
fn node_glow(fx: &mut Fx, sp: u32) {
    let s = slots(sp);
    let d = s.d;
    sprite_head(fx, d);
    texture(fx, d, 0x10);
    alpha(fx, d, 1);
    fx.desc_clamp(d, 5);
    cam(fx, sp);
    uv_fixed(fx, d);
    let n = fx.mem.i32(D);
    let f20 = ee::from_i32(Fx::clamp_i(n - 0x15, 0, 8));
    let n = fx.mem.i32(D);
    let k = ee::from_i32(Fx::clamp_i(0x30 - n, 0, 8));
    let f3 = ee::div(ee::mul(f20, k), 64.0);
    if crate::exe::ee_le(f3, 0.0) {
        return;
    }
    let n = fx.mem.i32(D);
    let m = ee::from_i32(rem2(n, 2));
    let f1 = ee::add(f(0x3F4C_CCCD), ee::mul(f(0x3E4C_CCCD), m));
    let a = fix(fx, ee::mul(ee::mul(255.0, f3), f1), 0x01E0_3F90);
    rgba(fx, d + 0x10, [0xFF, 0x64, 0x46], a as u8);
    corners(fx, &s, 8.0, true);
    let eye = fx.camera_eye(0);
    for k in 0..4u32 {
        let x = fx.mem.read_u32(eye + 4 * k);
        fx.mem.write_u32(sp + 0x6C0 + 4 * k, x);
    }
    let ex = fx.mem.f(sp + 0x6C0);
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    let p = if crate::exe::ee_lt(ee::mul(ex, side), 0.0) { D + 0x50 } else { D + 0x60 };
    words(fx, s.at, p);
    draw(fx, &s, K::Far);
}

/// The attacker pass (0xAC updates from 0): the attacker drawn into its
/// target with the sphere callback, composited by one fan 1.0 about its
/// screen point D+0x80, grey, alpha 128 ratio(age, 48) ramp_above(age,
/// 171, 123) [F 0x1E04164..0x1E04744]. `hi`: the upper half of t0 at the
/// 0x1F8280 call (see [`bursts`]).
fn attacker_pass(fx: &mut Fx, sp: u32, hi: u64) {
    let at = D + 0x8DD0;
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(at, 1);
        fx.mem.write_u32(at + 4, 0);
    }
    if fx.mem.read_u8(at) != 1 {
        return;
    }
    let d = sp + 0x760;
    let a = fan_desc(fx, d - 0x4C0, 0);
    let layer = fx.layer_begin();
    fx.texflush(a);
    fx.mem.write_u32(D_STATIC, D);
    let (_, regs) = fx.unit_draw_post(1, d + 0x60, d + 0x68, CB, &mut |fx, p| callback(fx, p, hi));
    fx.texflush(regs);
    let ramp = |fx: &mut Fx| {
        let age = ee::from_i32(fx.mem.i32(at + 4));
        let r = Fx::ratio_below(age, 48.0);
        let age = ee::from_i32(fx.mem.i32(at + 4));
        ee::mul(r, Fx::ramp_above(age, 171.0, 123.0))
    };
    let f22 = ramp(fx);
    let f1 = ramp(fx);
    let a0 = fix(fx, ee::mul(128.0, f22), 0x01E0_4338) & 0xFF;
    let a1 = fix(fx, ee::mul(128.0, f1), 0x01E0_4344);
    rgba(fx, d + 0x10, [0x80; 3], a0 as u8);
    rgba(fx, d + 0x28, [0x80; 3], a1 as u8);
    copy8(fx, d + 0x40, d + 0x28);
    fan(fx, d, D + 0x80, sp + 0x670, 1.0, 0, &FAN_A);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(at + 4) + 1;
    fx.mem.set32(at + 4, age);
    if age >= 0xAC {
        fx.mem.write_u8(at, 0);
    }
}

/// The glow at the travelling point on 0x61 (0x52 updates, D+0xBF20): 45
/// in the D+0xC0-scaled matrix, alpha 255 (0.7 + 0.3 (age % 2)) f21, far Z
/// [F 0x1E04748..0x1E04A94].
fn glow(fx: &mut Fx, sp: u32, f21: f32) {
    let s = slots(sp);
    let d = s.d;
    let g = D + 0xBF20;
    if fx.mem.i32(D) == 0x61 {
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
    }
    if fx.mem.read_u8(g) != 1 {
        return;
    }
    sprite_head(fx, d);
    texture(fx, d, 0x10);
    fx.desc_clamp(d, 5);
    alpha(fx, d, 1);
    let id = sp + 0x5B0;
    fx.camera_vp_to(s.m);
    fx.identity(id);
    fx.translate_rows(id, id, D + 0xB0);
    fx.scale_rows(id, D + 0xC0);
    crate::blib::exe::mul_mem(fx, s.m, id);
    let age = fx.mem.i32(g + 4);
    let m = ee::from_i32(rem2(age, 2));
    let f1 = ee::add(f(0x3F33_3333), ee::mul(f(0x3E99_999A), m));
    let a = fix(fx, ee::mul(ee::mul(255.0, f1), f21), 0x01E0_48B4);
    rgba(fx, d + 0x10, [0x64, 0x46, 0xFF], a as u8);
    corners(fx, &s, 45.0, true);
    uv_fixed(fx, d);
    draw_c(fx, &s, K::Far);
    let age = fx.mem.i32(g + 4) + 1;
    fx.mem.set32(g + 4, age);
    if age >= 0x52 {
        fx.mem.write_u8(g, 0);
    }
}

/// The shard records (D+0xA020, 16 x 0x1F0: +4 age, +8 6 lives, +0x20 6
/// widths, +0x38 6 lengths, +0x50 6 swings, +0x70 6 turns), one a frame on
/// 0x62..0xB1; each live shard 10 quads (prim 0x34, texture +0x20) along
/// its turn in the D+0xC0-scaled matrix at D+0xB0, the colour table's
/// entries s / s + 1 faded by (1 - tf(age, life)) f21, keyed [F
/// 0x1E04A98..0x1E05304].
fn shards(fx: &mut Fx, sp: u32, f21: f32) {
    let n = fx.mem.i32(D);
    if (0x62..0xB2).contains(&n) {
        let base = D + 0x1F0 * rem2(n, 0x10) as u32;
        let r = base + 0xA020;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for k in 0..6u32 {
            let s0 = base + 4 * k;
            let x = r_rem(fx, s0, 0x01E0_4B20, 0x100);
            let l = fixs(fx, ee::mul(16.0, ee::add(f(0x3F33_3333), ee::div(ee::mul(f(0x3E99_999A), ee::from_i32(x)), 256.0))), 0x01E0_4B84);
            fx.mem.set32(r + 8 + 4 * k, l);
            let x = r_rem(fx, s0, 0x01E0_4B94, 0x100) + 1;
            let v = ee::mul(2.5, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(x)), 256.0)));
            fx.mem.setf(r + 0x20 + 4 * k, v);
            let x = r_rem(fx, s0, 0x01E0_4BF8, 0x100) + 1;
            let v = ee::mul(30.0, ee::add(f(0x3F33_3333), ee::div(ee::mul(f(0x3E99_999A), ee::from_i32(x)), 256.0)));
            fx.mem.setf(r + 0x38 + 4 * k, v);
            let x = r_rem(fx, s0, 0x01E0_4C6C, 0x100) + 1;
            let v = ee::mul(6.5, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(x)), 256.0)));
            fx.mem.setf(r + 0x50 + 4 * k, v);
            fx.random_table_vec(sp + 0x6B0, 1.0);
            let m = r + 0x70 + 0x40 * k;
            fx.point_along(m, sp + 0x6B0);
            let s2 = (k << 16) as i32;
            fx.mult(0x2AAA_AAAB, s2);
            let a = fx.hilo().1.wrapping_add(((s2 as u32) >> 31) as i32);
            fx.rotate_axis(m, a, 2);
        }
    }
    let q = sp + 0x6D0;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, q, 1);
    texture(fx, q, 0x20);
    fx.desc_clamp(q, SHARD_CLAMP);
    let (m, id) = (sp + 0x570, sp + 0x5B0);
    for i in 0..0x10u32 {
        let r = D + 0xA020 + 0x1F0 * i;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        for k in 0..6u32 {
            let age = fx.mem.i32(r + 4);
            let life = fx.mem.i32(r + 8 + 4 * k);
            if life < age {
                continue;
            }
            fx.camera_vp_to(m);
            let t = std::array::from_fn(|c| fx.mem.f(D + 0xB0 + 4 * c as u32));
            let tm = fx.trs(None, None, Some(t));
            fx.write_m4(id, &tm);
            crate::blib::exe::mul_mem(fx, id, r + 0x70 + 0x40 * k);
            fx.scale_rows(id, D + 0xC0);
            crate::blib::exe::mul_mem(fx, m, id);
            let age = fx.mem.i32(r + 4);
            let life = fx.mem.i32(r + 8 + 4 * k);
            let t = fx.tan_falloff_p(age, life, 1);
            let f23 = ee::sub(1.0, t);
            let f22 = ee::add(1.0, ee::sub(1.0, f23));
            for s5 in 0..10i32 {
                let tb = D + 0x9FF0 + 4 * s5 as u32;
                let b = ee::from_i32(i32::from(fx.mem.read_u8(tb + 3)));
                let a0 = fix(fx, ee::mul(f21, ee::mul(b, f23)), 0x01E0_4F10) & 0xFF;
                let b = ee::from_i32(i32::from(fx.mem.read_u8(tb + 7)));
                let a1 = fix(fx, ee::mul(f21, ee::mul(b, f23)), 0x01E0_4F5C);
                for j in 0..3u32 {
                    let c = fx.mem.read_u8(tb + j);
                    fx.mem.write_u8(q + 0x10 + j, c);
                }
                fx.mem.write_u8(q + 0x13, a0 as u8);
                fx.mem.write_u32(q + 0x14, ONE);
                for j in 0..3u32 {
                    let c = fx.mem.read_u8(tb + 4 + j);
                    fx.mem.write_u8(q + 0x40 + j, c);
                }
                fx.mem.write_u8(q + 0x43, a1 as u8);
                fx.mem.write_u32(q + 0x44, ONE);
                copy8(fx, q + 0x28, q + 0x10);
                copy8(fx, q + 0x58, q + 0x40);
                let t1 = rem2(s5, 4);
                let ta = ee::div(ee::mul(0.5, ee::from_i32(t1)), 4.0);
                let tb2 = ee::div(ee::mul(0.5, ee::from_i32(t1 + 1)), 4.0);
                for (o, ss, tt) in [(0x18u32, f(0x3D20_0000), ta), (0x30, f(0x3E58_0000), ta), (0x48, f(0x3D20_0000), tb2), (0x60, f(0x3E58_0000), tb2)] {
                    fx.mem.setf(q + o, ss);
                    fx.mem.setf(q + o + 4, tt);
                }
                let f3 = fx.mem.f(r + 0x38 + 4 * k);
                let f4 = fx.mem.f(r + 0x20 + 4 * k);
                let x0 = ee::div(ee::mul(f4, ee::from_i32(10 - s5)), 10.0);
                let f25 = ee::div(ee::mul(f3, ee::from_i32(s5 + 1)), 10.0);
                let f26 = ee::mul(f23, x0);
                let f24 = ee::div(ee::mul(f3, ee::from_i32(s5)), 10.0);
                let x1 = ee::div(ee::mul(f4, ee::from_i32(9 - s5)), 10.0);
                let f20 = ee::mul(f23, x1);
                let s1 = s5 << 16;
                fx.mult(0x6666_6667, s1);
                let a = ((fx.hilo().1 >> 2).wrapping_add(((s1 as u32) >> 31) as i32)) & 0xFFFF;
                let sn = fx.tsin(a);
                let h = fx.mem.f(r + 0x50 + 4 * k);
                let f27 = ee::mul(f22, ee::mul(h, sn));
                let s1b = (s5 + 1) << 16;
                fx.mult(0x6666_6667, s1b);
                let a = ((fx.hilo().1 >> 2).wrapping_add(((s1b as u32) >> 31) as i32)) & 0xFFFF;
                let sn = fx.tsin(a);
                let h = fx.mem.f(r + 0x50 + 4 * k);
                let f1 = ee::mul(f22, ee::mul(h, sn));
                let p = sp + 0x630;
                let pts = [
                    [f27, ee::add(ee::neg(f26), f27), f24],
                    [f27, ee::add(f26, f27), f24],
                    [f1, ee::add(ee::neg(f20), f1), f25],
                    [f1, ee::add(f20, f1), f25],
                ];
                for (j, v) in pts.iter().enumerate() {
                    let o = p + 0x10 * j as u32;
                    fx.mem.setf(o, v[0]);
                    fx.mem.setf(o + 4, v[1]);
                    fx.mem.setf(o + 8, v[2]);
                    fx.mem.write_u32(o + 0xC, ONE);
                }
                let mm = fx.read_m4(m);
                if fx.project_desc(p, PROJ_OUT, Some(&mm), q, sp + 0x86C) {
                    let w = fx.mem.f(sp + 0x86C);
                    let key = fix(fx, w, 0x01E0_5224);
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
}

/// The beam record on 0x61 (D+0xBF30, index D+0xBF28: +4 age, +0x10 node
/// 0x33's point, +0x20 the step to the target, +0x30 its length, +0x34 8
/// widths, +0x54 8 swings, +0x80 8 turns along the step), 0x41 updates:
/// 8 beams of min(16 ratio(age, 33), 16) quads (prim 0x34, texture +0x20)
/// in the turn at +0x10, (32, 23, FF) alpha 164 (64 - age) / 64, keyed [F
/// 0x1E05308..0x1E059C8].
fn beams(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x61 {
        let i = fx.mem.i32(D + 0xBF28);
        let base = D + 0x280 * i as u32;
        let g = base + 0xBF30;
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        words(fx, g + 0x10, D + 0x30);
        fx.vu_sub4(g + 0x20, D + 0x90, g + 0x10);
        let x2 = Fx::powf2(fx.mem.f(g + 0x20));
        let y2 = Fx::powf2(fx.mem.f(g + 0x24));
        let xy = ee::add(x2, y2);
        let z2 = Fx::powf2(fx.mem.f(g + 0x28));
        let l = fx.sqrtf(ee::add(z2, xy));
        fx.mem.setf(g + 0x30, l);
        for j in 0..8u32 {
            let s0 = base + 0x40 * j;
            let x = r_rem(fx, s0, 0x01E0_542C, 0x100);
            let v = ee::mul(3.5, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(x)), 256.0)));
            fx.mem.setf(g + 0x34 + 4 * j, v);
            let x = r_rem(fx, s0, 0x01E0_548C, 0x100);
            let v = ee::mul(8.0, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(x)), 256.0)));
            fx.mem.setf(g + 0x54 + 4 * j, v);
            let m = g + 0x80 + 0x40 * j;
            fx.point_along(m, g + 0x20);
            let x = r_rem(fx, s0, 0x01E0_5500, 0x100);
            fx.rotate_axis(m, (x << 16) >> 8, 2);
        }
        let x = fx.mem.i32(D + 0xBF28) + 1;
        fx.mem.set32(D + 0xBF28, if x > 0 { 0 } else { x });
    }
    let q = sp + 0x6D0;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, q, 1);
    texture(fx, q, 0x20);
    fx.desc_clamp(q, SHARD_CLAMP);
    let (m, id) = (sp + 0x570, sp + 0x5B0);
    let g = D + 0xBF30;
    if fx.mem.read_u8(g) == 0 {
        return;
    }
    let age = ee::from_i32(fx.mem.i32(g + 4));
    let r = Fx::ratio_below(age, 33.0);
    let mut s6 = fixs(fx, ee::mul(16.0, r), 0x01E0_5658);
    if s6 > 0x10 {
        s6 = 0x10;
    }
    let age = fx.mem.i32(g + 4);
    let f21 = ee::div(ee::from_i32(0x40 - age), 64.0);
    let a = fix(fx, ee::mul(164.0, f21), 0x01E0_56B0);
    rgba(fx, q + 0x10, [0x32, 0x23, 0xFF], a as u8);
    copy8(fx, q + 0x28, q + 0x10);
    copy8(fx, q + 0x40, q + 0x10);
    copy8(fx, q + 0x58, q + 0x10);
    let f20 = ee::add(1.0, ee::mul(1.5, ee::sub(1.0, f21)));
    let mut s2 = 0i32;
    for j in 0..8u32 {
        fx.camera_vp_to(m);
        fx.translate_rows(id, g + 0x80 + 0x40 * j, g + 0x10);
        crate::blib::exe::mul_mem(fx, m, id);
        for s5 in 0..s6 {
            let mm = rem2(s5, 2);
            let ta = ee::div(ee::mul(0.5, ee::from_i32(mm)), 2.0);
            let tb = ee::div(ee::mul(0.5, ee::from_i32(mm + 1)), 2.0);
            for (o, ss, tt) in [(0x18u32, f(0x3D20_0000), ta), (0x30, f(0x3E58_0000), ta), (0x48, f(0x3D20_0000), tb), (0x60, f(0x3E58_0000), tb)] {
                fx.mem.setf(q + o, ss);
                fx.mem.setf(q + o + 4, tt);
            }
            let l = fx.mem.f(g + 0x30);
            let f23 = ee::mul(fx.mem.f(g + 0x34 + 4 * j), f21);
            let f22 = ee::div(ee::mul(l, ee::from_i32(s5 + 1)), 16.0);
            let f24 = ee::div(ee::mul(l, ee::from_i32(s5)), 16.0);
            let a = (((s5 << 16) >> 4).wrapping_add(s2)) & 0xFFFF;
            let c = fx.tcos(a);
            let b = fx.mem.f(g + 0x54 + 4 * j);
            let f25 = ee::mul(f20, ee::mul(b, c));
            let a = ((((s5 + 1) << 16) >> 4).wrapping_add(s2)) & 0xFFFF;
            let c = fx.tcos(a);
            let b = fx.mem.f(g + 0x54 + 4 * j);
            let f1 = ee::mul(f20, ee::mul(b, c));
            let p = sp + 0x670;
            let n23 = ee::neg(f23);
            let pts = [[ee::add(n23, f25), f24], [ee::add(f23, f25), f24], [ee::add(n23, f1), f22], [ee::add(f23, f1), f22]];
            for (k, v) in pts.iter().enumerate() {
                let o = p + 0x10 * k as u32;
                fx.mem.write_u32(o, 0);
                fx.mem.setf(o + 4, v[0]);
                fx.mem.setf(o + 8, v[1]);
                fx.mem.write_u32(o + 0xC, ONE);
            }
            let mm = fx.read_m4(m);
            if fx.project_desc(p, PROJ_OUT, Some(&mm), q, sp + 0x86C) {
                let w = fx.mem.f(sp + 0x86C);
                let key = fix(fx, w, 0x01E0_5928);
                fx.submit(q, OT, key as i32);
            }
        }
        s2 = s2.wrapping_add(0x1A0A);
    }
    let age = fx.mem.i32(g + 4) + 1;
    fx.mem.set32(g + 4, age);
    if age >= 0x41 {
        fx.mem.write_u8(g, 0);
    }
}

/// The callback 0x1E05B00(p) [F]: b424's sphere callback with the window
/// (40, 40, 40, 80) at ALPHA preset 2, then the static D's 17 x 17 sphere
/// (D+0x8DE0) at D+0x70 under three random turns (rand from the packet
/// pointer), 16 x 16 quads (prim 0x34, texture D+0x1C, (80, 80, 80, 64),
/// ST quarters) by 0x100BA0 with far Z. The quad's TEX0 / TEX1 slots (sp +
/// 0x200 / 0x208) hold 0x209800's save of 0x1F0FD0's s1 when the texture
/// is missing: the callback's address and `hi`, t0's upper half at the
/// 0x1F8280 call (0x1F0FD0 keeps the callback in s1, `move128` from t0).
fn callback(fx: &mut Fx, p: u32, hi: u64) -> u32 {
    let sp = fx.enter(0x220);
    fx.mem.write_u64(sp + 0x200, u64::from(CB));
    fx.mem.write_u64(sp + 0x208, hi);
    let d = fx.mem.read_u32(D_STATIC);
    fx.mem.write_u32(sp + 0xB0, d);
    let s0 = frame_mask_packet(fx, p, 0xFF00_0000, [0, 0]);
    let t = sp + 0x158;
    head(fx, t, 6, 3, 0x441, 0);
    alpha(fx, t, 2);
    rgba(fx, sp + 0x168, [0x40; 3], 0x80);
    far_xy(fx, sp + 0x170, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x178, 0x8FF8, 0x8FF8);
    let mut s6 = fx.submit_at(s0, t, 0, 0);
    let q = sp + 0x188;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(d + 0x1C);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 0);
    fx.camera_vp_to(sp + 0xD0);
    fx.identity(sp + 0x110);
    fx.translate_rows(sp + 0x110, sp + 0x110, d + 0x70);
    for (axis, ra) in [0x01E0_5CBCu32, 0x01E0_5CF0, 0x01E0_5D24].into_iter().enumerate() {
        let v = fx.rand_from(s0, ra) >> 23;
        fx.rotate_axis(sp + 0x110, (rem2(v, 0x100) << 16) >> 8, axis as i32);
    }
    crate::blib::exe::mul_mem(fx, sp + 0xD0, sp + 0x110);
    rgba(fx, sp + 0x198, [0x80; 3], 0x64);
    for to in [0x1B0, 0x1C8, 0x1E0] {
        copy8(fx, sp + to, sp + 0x198);
    }
    // a2 / a3 at the closing 0x2096C0: 0x206F90's zeros after a drawn
    // quad, else 0x100BA0's untouched m / desc of the last rejected one.
    let mut a23 = [0u32, 0];
    for i in 0..0x10i32 {
        let s3 = d + 0x8CF0 + 0x110 * i as u32;
        let s5 = s3 + 0x110;
        let ri = rem2(i, 4);
        let f20 = ee::div(ee::from_i32(ri), 4.0);
        let f21 = ee::div(ee::from_i32(ri + 1), 4.0);
        for j in 0..0x10i32 {
            let rj = rem2(j, 4);
            let f2 = ee::div(ee::from_i32(rj), 4.0);
            let f0 = ee::div(ee::from_i32(rj + 1), 4.0);
            fx.mem.setf(sp + 0x1A0, f2);
            fx.mem.setf(sp + 0x1A4, f20);
            fx.mem.setf(sp + 0x1B8, f0);
            fx.mem.setf(sp + 0x1BC, f20);
            fx.mem.setf(sp + 0x1D0, f2);
            fx.mem.setf(sp + 0x1D4, f21);
            fx.mem.setf(sp + 0x1E8, f0);
            fx.mem.setf(sp + 0x1EC, f21);
            let (a, b) = (0x10 * j as u32 + 0xF0, 0x10 * (j + 1) as u32 + 0xF0);
            if fx.project_n(&[s3 + a, s3 + b, s5 + a, s5 + b], sp + 0xD0, q, sp + 0x21C, 0x01E0_5ED4) != 1 {
                a23 = [sp + 0xD0, q];
                continue;
            }
            far(fx, sp + 0x1A8, &[sp + 0x1C0, sp + 0x1D8, sp + 0x1F0]);
            s6 = fx.submit_at(s6, q, 0, 0);
            a23 = [0, 0];
        }
    }
    let r = frame_mask_packet(fx, s6, 0, a23);
    fx.leave(0x220);
    r
}
