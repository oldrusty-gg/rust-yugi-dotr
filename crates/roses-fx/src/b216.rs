//! BMODULE b216 (card 9), hand-ported from the record (linked at 0x1E00000,
//! the entry 0x1E00080 in a 0x790 frame, data area a0 = 0x1ED0000 kept in
//! the stack slot sp + 0xCC, 0xE060 bytes; one unit-target post callback
//! 0x1E052D0 (0xC0 frame) with its static D at 0x1E05580) [F] (helper
//! D28, 3 Nov 2026), and its twin b711 (card 19, [`program_b711`]: a 0x770
//! frame, D at sp + 0xE0, 0xE1F0 bytes, callback 0x1E054A0 / static D
//! 0x1E05780; see [`B711`]). Unoptimised code: s3 = the OT (0x206E40), the
//! sprite descriptor sp + 0x728 (prim 0x26 | 0x800, UVs sp + 0x740.. /
//! 0x750..) rebuilt before each pool. Every update the camera turn (sp +
//! 0x4E0), node 0x15 (kind 1, D+0x30), nodes 0x16 / 0x17 (D+0x50 / 0x60),
//! the self point (D+0x70) and the screen points of D+0x70 / D+0x80 (the
//! target's position) into D+0x90 / D+0xA0. Then: on 0x64 / 0x78 / 0x8C /
//! 0xA0 a window flash (FF, E8, E1) for 9 updates (D+0xE054) and a
//! frame-copy fan about D+0xA0 for 0x21 updates (D+0x70E0, 8 widening
//! fans); on 0x30 / 0x3A / 0x44 / 0x4E an emitter (D+0xC0, 4 x 0x40) with a
//! mote record (D+0x41D0, 4 x 0x350) at node 0x15, moving to the target;
//! each live emitter sprite spawns into the sub-pools D+0x1C0 (16 x 0x60
//! an emitter) and D+0x19C0 (16 x 0xA0); on 1..0x31 a spark record a frame
//! (D+0x4F10, 16 x 0x130); on 3 the ring between nodes 0x16 / 0x17
//! (D+0x6210, 33 points x 3 radii, 128 motes); on 0x64 / 0x78 / 0x8C /
//! 0xA0 a burst at the target (D+0x70F0, 4 x 0x10E0); from 8 the vortex
//! (D+0xE044, 0xD9 updates: 2 passes of a 33 x 17 grid D+0xB470 by
//! 0x100BA0); from 0 the attacker pass (D+0xE04C, 0xA1 updates: 0x1F8280
//! with the callback, then a widening fan about D+0x90). The stage light
//! 0x80 - 128 clamp(n, 0, 8) clamp(0xE8 - n, 0, 8) / 64. Returns 1 on 0x5A,
//! 6 on 0x65..0xBF, 2 on 0xC0, 8 once the count reaches 0xE9.
use crate::b182::{alpha, far, head, rgba, uv};
use crate::b424::{camera, fan, fan_desc};
use crate::b440::{copy8, fix, frame_mask_packet, rem2, vec4, words};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const ONE: u32 = 0x3F80_0000;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The record's parameters: b216 (card 9) or b711 (card 19) [F].
pub(crate) struct V {
    frame: u32,
    dslot: u32,
    retslot: u32,
    bzero: u32,
    nodes: [i32; 3],
    /// Emitters (and their sub-pools / mote records).
    emitters: u32,
    /// (update, life) of the emitter spawns.
    spawns: &'static [(i32, i32)],
    /// Flash / frame-fan / burst updates (burst index = position).
    beats: &'static [i32],
    flash_len: i32,
    /// Spark spawns while lo < n < hi; the ring on `ring_n`.
    sparks: (i32, i32),
    ring_n: i32,
    /// The vortex's start (b216) or none (b711: the column records).
    vortex: Option<i32>,
    attacker: i32,
    light: i32,
    ret: (i32, (i32, i32), i32, i32),
    /// D offsets: sub-pool A / B, the mote index (records at +0x10), the
    /// sparks, the ring, the frame fan (burst index +8, bursts +0x10), the
    /// attacker pass and the flash.
    da: u32,
    db: u32,
    dc: u32,
    dr: u32,
    dring: u32,
    dff: u32,
    datt: u32,
    dflash: u32,
    /// Colours (the record's (FF, 96, 96) family and its variants).
    c_main: [u8; 3],
    c_pool_a: [u8; 3],
    c_spark2: [u8; 3],
    spark2_a: f32,
    burst_len: [f32; 3],
    burst_s: (f32, f32),
    glow_r: f32,
    cb: u32,
    d_static: u32,
    band: ([u8; 4], [u16; 4]),
    /// sp offset ranges (lo, hi, delta) where this record's locals differ.
    sps: &'static [(u32, u32, i32)],
    /// b216's return addresses (low halves) -> this record's.
    ras: &'static [(u16, u16)],
}

impl V {
    fn s(&self, o: u32) -> u32 {
        match self.sps.iter().find(|r| (r.0..r.1).contains(&o)) {
            Some(r) => o.wrapping_add(r.2 as u32),
            None => o,
        }
    }
    fn ra(&self, a: u32) -> u32 {
        match self.ras.iter().find(|p| u32::from(p.0) == a & 0xFFFF) {
            Some(p) => 0x01E0_0000 | u32::from(p.1),
            None => a,
        }
    }
    fn ras<const N: usize>(&self, a: &[u32; N]) -> [u32; N] {
        a.map(|x| self.ra(x))
    }
    fn b711(&self) -> bool {
        self.vortex.is_none()
    }
}

pub(crate) const B216: V = V {
    frame: 0x790,
    dslot: 0xCC,
    retslot: 0xD0,
    bzero: 0xE060,
    nodes: [0x15, 0x16, 0x17],
    emitters: 4,
    spawns: &[(0x4E, 0x52), (0x44, 0x48), (0x3A, 0x3E), (0x30, 0x34)],
    beats: &[0x64, 0x78, 0x8C, 0xA0],
    flash_len: 8,
    sparks: (0, 0x32),
    ring_n: 3,
    vortex: Some(8),
    attacker: 0,
    light: 0xE8,
    ret: (0x5A, (0x65, 0xC0), 0xC0, 0xE9),
    da: 0x1C0,
    db: 0x19C0,
    dc: 0x41C0,
    dr: 0x4F10,
    dring: 0x6210,
    dff: 0x70E0,
    datt: 0xE04C,
    dflash: 0xE054,
    c_main: [0xFF, 0x96, 0x96],
    c_pool_a: [0xFF, 0x3C, 0x3C],
    c_spark2: [0xFF, 0x60, 0x60],
    spark2_a: 80.0,
    burst_len: [2.0, 1.5, 2.0],
    burst_s: (8.0, 10.0),
    glow_r: 150.0,
    cb: 0x01E0_52D0,
    d_static: 0x01E0_5580,
    band: ([0x80, 0, 0, 0xFF], [0x6BF8, 0x71F8, 0x93F8, 0x8DF8]),
    sps: &[],
    ras: &[],
};

/// b711 (card 19): three emitters, the beats 0x8C / 0xA1 / 0xBB, (FF, A0,
/// 96) colours, sparks on 7..0x2C, the ring on 6, bigger bursts, no vortex
/// but 32 column records (D+0xA0F0, 0x208: 64 strips each) from 9, the
/// attacker pass from 5 as six fans [F B19s.dis].
pub(crate) const B711: V = V {
    frame: 0x770,
    dslot: 0xE0,
    retslot: 0xB0,
    bzero: 0xE1F0,
    nodes: [0x1B, 0x1C, 0x1D],
    emitters: 3,
    spawns: &[(0x70, 0x4B), (0x4B, 0x56), (0x35, 0x57)],
    beats: &[0x8C, 0xA1, 0xBB],
    flash_len: 16,
    sparks: (6, 0x2D),
    ring_n: 6,
    vortex: None,
    attacker: 5,
    light: 0x123,
    ret: (0x82, (0x8D, 0xDB), 0xDB, 0x124),
    da: 0x180,
    db: 0x1380,
    dc: 0x3180,
    dr: 0x3B80,
    dring: 0x4E80,
    dff: 0x5D50,
    datt: 0xA0E0,
    dflash: 0xA0E8,
    c_main: [0xFF, 0xA0, 0x96],
    c_pool_a: [0xFF, 0x46, 0x3C],
    c_spark2: [0xFF, 0x6A, 0x60],
    spark2_a: 64.0,
    burst_len: [2.0, 2.0, 2.5],
    burst_s: (12.0, 15.0),
    glow_r: 170.0,
    cb: 0x01E0_54A0,
    d_static: 0x01E0_5780,
    band: ([0xC8, 0x46, 0, 0xE4], [0x6FF8, 0x6FF8, 0x8FF8, 0x8FF8]),
    sps: &[(0x460, 0x5B0, -0x40), (0x5B0, 0x5E0, -0x38), (0x658, 0x720, -0xB0), (0x728, 0x760, -0x20), (0x780, 0x790, -0x20)],
    ras: &[
        (0x07FC, 0x079C),
        (0x0AD4, 0x0A6C),
        (0x0AE4, 0x0A7C),
        (0x0B28, 0x0AC0),
        (0x0B34, 0x0ACC),
        (0x0B78, 0x0B10),
        (0x0B84, 0x0B1C),
        (0x0C4C, 0x0BE4),
        (0x0C5C, 0x0BF4),
        (0x0CC8, 0x0C60),
        (0x0CD4, 0x0C6C),
        (0x0D40, 0x0CD8),
        (0x0D4C, 0x0CE4),
        (0x0F70, 0x0EF0),
        (0x0FC8, 0x0F48),
        (0x13F0, 0x1378),
        (0x1480, 0x1408),
        (0x1E3C, 0x1DCC),
        (0x25C8, 0x256C),
        (0x2770, 0x2718),
        (0x2A00, 0x29A8),
        (0x2C74, 0x2C1C),
        (0x2DEC, 0x2D94),
        (0x3098, 0x3048),
        (0x31A0, 0x3150),
        (0x3764, 0x3704),
        (0x3854, 0x37F4),
        (0x4170, 0x4148),
        (0x44CC, 0x44AC),
        (0x4584, 0x4568),
        (0x4E4C, 0x4894),
        (0x4E5C, 0x48A4),
        (0x4EA0, 0x48E8),
        (0x4EAC, 0x48F4),
        (0x4EF0, 0x4938),
        (0x4EFC, 0x4944),
        (0x4FCC, 0x4A0C),
        (0x4FDC, 0x4A1C),
        (0x5048, 0x4A88),
        (0x5054, 0x4A94),
        (0x50B8, 0x4AF8),
        (0x50C4, 0x4B04),
    ],
};

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, &B216, param)
}

/// b711's entry 0x1E00080 (card 19).
pub fn program_b711(fx: &mut Fx, param: i32) -> i32 {
    run(fx, &B711, param)
}

fn run(fx: &mut Fx, v: &V, param: i32) -> i32 {
    let sp = fx.enter(v.frame);
    fx.mem.write_u32(sp + v.dslot, D);
    fx.mem.write_u32(sp + v.retslot, 0);
    let ret = if param >= 0 { init(fx, v, sp) } else { step(fx, v, sp) };
    fx.leave(v.frame);
    ret
}

fn init(fx: &mut Fx, v: &V, sp: u32) -> i32 {
    fx.bzero(D, v.bzero);
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
    for (o, node, kind) in [(0x20u32, v.nodes[0], 1), (0x40, v.nodes[1], 0), (0x44, v.nodes[2], 0)] {
        let h = fx.mem.i32(D + 4);
        let s = fx.request_node(h, node, kind);
        fx.mem.set32(D + o, s);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x80);
    // The ring: 33 points at three radii.
    let p0 = sp + v.s(0x5A0);
    for k in 0..0x21u32 {
        let a = ((k << 16) as i32) >> 5;
        let s = fx.tsin(a);
        fx.mem.setf(p0, s);
        let c = fx.tcos(a);
        fx.mem.setf(p0 + 4, c);
        fx.mem.write_u32(p0 + 8, 0);
        fx.mem.write_u32(p0 + 0xC, ONE);
        let p = D + v.dring + 0x30 * k;
        fx.vu_scale4(p + 0x10, p0, f(0x4212_0000));
        fx.vu_scale4(p + 0x20, p0, f(0x4239_999A));
        fx.vu_scale4(p + 0x30, p0, f(0x423C_0000));
    }
    if v.b711() {
        return 0;
    }
    // The vortex's random phases: 33 rows of 17.
    for r in 0..0x21u32 {
        for c in 0..0x11u32 {
            let p = D + 0x44 * r + 4 * c;
            let x = fx.rand_from(p, 0x01E0_03D0) >> 23;
            let x = rem2(x, 0x100);
            fx.mem.set32(p + 0xD780, (x << 16) >> 8);
        }
    }
    0
}

/// The screen point of `p` (0x204D80 through sp + 0x460 into sp + 0x780)
/// into `o`.
fn screen_point(fx: &mut Fx, v: &V, sp: u32, p: u32, o: u32) {
    let (mo, xo) = (sp + v.s(0x460), sp + v.s(0x780));
    let m = fx.read_m4(mo);
    fx.project_at(p, &m, xo, sp + v.s(0x78C));
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

/// b424's camera (vp at sp + 0x460, identity at sp + 0x4A0).
fn cam(fx: &mut Fx, v: &V, sp: u32) {
    camera(fx, sp + v.s(0x460) - 0x250);
}

fn step(fx: &mut Fx, v: &V, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + v.s(0x4E0));
    let h = fx.mem.i32(D + 4);
    let s = fx.mem.i32(D + 0x20);
    fx.node_position(h, s, D + 0x30);
    for k in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x40 + 4 * k);
        fx.node_position(h, s, D + 0x50 + 0x10 * k);
    }
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x70);
    cam(fx, v, sp);
    screen_point(fx, v, sp, D + 0x70, D + 0x90);
    cam(fx, v, sp);
    screen_point(fx, v, sp, D + 0x80, D + 0xA0);

    flash(fx, v, sp);
    frame_fan(fx, v, sp);
    emitters(fx, v, sp);
    sparks(fx, v, sp);
    ring(fx, v, sp);
    bursts(fx, v, sp);
    if v.b711() {
        attacker_pass(fx, v, sp);
        columns(fx, sp);
    } else {
        vortex(fx, sp);
        attacker_pass(fx, v, sp);
    }

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(v.light - n, 0, 8);
    let n = fx.mem.i32(D);
    let k = Fx::clamp_i(n, 0, 8);
    let l = ((k.wrapping_neg() << 7).wrapping_mul(s0) >> 6) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + v.retslot);
    if n == v.ret.0 {
        ret = 1;
    } else {
        if (v.ret.1 .0..v.ret.1 .1).contains(&n) {
            ret = 6;
        }
        if n == v.ret.2 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= v.ret.3 {
        ret = 8;
    }
    fx.mem.set32(sp + v.retslot, ret);
    ret
}

/// The window flash (9 / 17 updates; its descriptor sp + 0x5B0 built every
/// update) [F 0x1E0070C..0x1E008EC].
fn flash(fx: &mut Fx, v: &V, sp: u32) {
    let fl = D + v.dflash;
    if v.beats.contains(&fx.mem.i32(D)) {
        fx.mem.write_u8(fl, 1);
        fx.mem.write_u32(fl + 4, 0);
    }
    let t = sp + v.s(0x5B0);
    head(fx, t, 6, 3, 0x441, 0);
    alpha(fx, t, 0);
    if fx.mem.read_u8(fl) != 1 {
        return;
    }
    let age = fx.mem.i32(fl + 4);
    let q = ee::div(ee::from_i32(v.flash_len - age), ee::from_i32(v.flash_len));
    let a = fix(fx, ee::mul(128.0, q), v.ra(0x01E0_07FC));
    rgba(fx, t + 0x10, [0xFF, 0xE8, 0xE1], a as u8);
    far_xy(fx, t + 0x18, 0x7000, 0x7000);
    far_xy(fx, t + 0x20, 0x9000, 0x9000);
    fx.submit(t, OT, 0);
    let age = fx.mem.i32(fl + 4) + 1;
    fx.mem.set32(fl + 4, age);
    if age >= v.flash_len + 1 {
        fx.mem.write_u8(fl, 0);
    }
}

const FAN_T: [u32; 12] = [
    0x01E0_0AD4, 0x01E0_0AE4, 0x01E0_0B28, 0x01E0_0B34, 0x01E0_0B78, 0x01E0_0B84, 0x01E0_0C4C, 0x01E0_0C5C, 0x01E0_0CC8, 0x01E0_0CD4, 0x01E0_0D40,
    0x01E0_0D4C,
];
const FAN_A: [u32; 12] = [
    0x01E0_4E4C, 0x01E0_4E5C, 0x01E0_4EA0, 0x01E0_4EAC, 0x01E0_4EF0, 0x01E0_4EFC, 0x01E0_4FCC, 0x01E0_4FDC, 0x01E0_5048, 0x01E0_5054, 0x01E0_50B8,
    0x01E0_50C4,
];

/// The frame-copy fan about the target's screen point (0x21 updates): 8
/// fans 1 + 0.2 (8 - j) / 8 in one layer [F 0x1E008F0..0x1E00E18].
fn frame_fan(fx: &mut Fx, v: &V, sp: u32) {
    let ff = D + v.dff;
    if v.beats.contains(&fx.mem.i32(D)) {
        fx.mem.write_u8(ff, 1);
        fx.mem.write_u32(ff + 4, 0);
    }
    if fx.mem.read_u8(ff) != 1 {
        return;
    }
    let d = sp + v.s(0x6B0);
    let a = fan_desc(fx, d - 0x4C0, 0);
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, a, 1, d + 0x60, d + 0x68);
    fx.texflush(regs);
    let age = fx.mem.i32(ff + 4);
    let al = (0x20 - age).wrapping_mul(80) >> 5;
    rgba(fx, d + 0x10, [0x80; 3], al as u8);
    copy8(fx, d + 0x28, d + 0x10);
    copy8(fx, d + 0x40, d + 0x10);
    let ras = v.ras(&FAN_T);
    for j in 0..8i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(8 - j)), 8.0));
        fan(fx, d, D + 0xA0, sp + v.s(0x560), s, 0, &ras);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(ff + 4) + 1;
    fx.mem.set32(ff + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(ff, 0);
    }
}

/// The sprite descriptor sp + 0x728 (prim 0x26 | 0x800, 5 registers).
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

/// The record's working slots: the corners (sp + 0x520 / 0x530), the
/// placed corners (0x560 / 0x570), the centre (0x5A0), the camera (0x460
/// / 0x4A0), the turn (0x4E0), the descriptor (0x728), w (0x78C).
#[derive(Clone, Copy)]
struct S {
    c: u32,
    p: u32,
    m: u32,
    id: u32,
    turn: u32,
    at: u32,
    d: u32,
    w: u32,
}

fn slots(v: &V, sp: u32) -> S {
    S {
        c: sp + v.s(0x520),
        p: sp + v.s(0x560),
        m: sp + v.s(0x460),
        id: sp + v.s(0x4A0),
        turn: sp + v.s(0x4E0),
        at: sp + v.s(0x5A0),
        d: sp + v.s(0x728),
        w: sp + v.s(0x78C),
    }
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

/// The corners about the centre projected: true when the sprite is drawn
/// (far Z first when `far_z`).
fn draw(fx: &mut Fx, s: &S, far_z: bool) -> bool {
    fx.vu_add4(s.p, s.c, s.at);
    fx.vu_add4(s.p + 0x10, s.c + 0x10, s.at);
    if !crate::fxlib2::project(fx, s.p, PROJ_OUT, s.m, s.d, s.w) {
        return false;
    }
    if far_z {
        far(fx, s.d + 0x20, &[s.d + 0x30]);
    }
    fx.submit(s.d, OT, 0);
    true
}

/// The centre = `vv` x `k` + `p`, drawn.
fn mote(fx: &mut Fx, s: &S, vv: u32, k: f32, p: u32, far_z: bool) {
    fx.vu_scale4(s.at, vv, k);
    fx.vu_add4(s.at, s.at, p);
    draw(fx, s, far_z);
}

fn r_rem(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let x = fx.rand_from(s0, ra) >> 23;
    rem2(x, m)
}

/// 255 x / 16 (`sh`) or 255 x / 32 as the record's `sll 8; subu; sra`.
fn a255(x: i32, sh: u32) -> u8 {
    (((x << 8) - x) >> sh) as u8
}

/// The emitters (D+0xC0, 0x40 each: +0 flag, +4 age, +8 life, +0xC / +0x10
/// the sub-pool cursors, +0x20 position, +0x30 velocity) with their mote
/// records (0x350 each), the sub-pools A (16 x 0x60 an emitter) / B (16 x
/// 0xA0) and the motes drawn [F 0x1E00E1C..0x1E0253C].
fn emitters(fx: &mut Fx, v: &V, sp: u32) {
    let s = slots(v, sp);
    let d = s.d;
    let ne = v.emitters as i32;
    let n = fx.mem.i32(D);
    let life = v.spawns.iter().find(|x| x.0 == n).map_or(-1, |x| x.1);
    let ci = D + v.dc;
    if life >= 0 {
        let e = D + 0x40 * fx.mem.read_u32(D + 0xB4);
        fx.mem.write_u8(e + 0xC0, 1);
        fx.mem.write_u32(e + 0xC4, 0);
        fx.mem.set32(e + 0xC8, life);
        let j = fx.mem.i32(ci);
        fx.mult(j, 0x350);
        let c = ci + 0x10 + 0x350 * j as u32;
        fx.mem.write_u8(c, 1);
        fx.mem.write_u32(c + 4, 0);
        words(fx, e + 0xE0, D + 0x30);
        words(fx, c + 0x10, D + 0x30);
        fx.random_table_vec(s.at, 0.0);
        fx.vu_add4(s.at, s.at, D + 0x80);
        let x = fx.mem.read_u32(e + 0xE0);
        fx.mem.write_u32(s.at, x);
        fx.vu_sub4(e + 0xF0, s.at, e + 0xE0);
        for k in 0..0x10u32 {
            let x = r_rem(fx, D, v.ra(0x01E0_0F70), 8);
            fx.mem.write_u8(c + 0x20 + k, x as u8);
            fx.random_table_vec(c + 0x30 + 0x10 * k, 1.0);
        }
        for k in 0..0x20u32 {
            let x = r_rem(fx, D, v.ra(0x01E0_0FC8), 4);
            fx.mem.write_u8(c + 0x130 + k, x as u8);
            fx.random_table_vec(c + 0x150 + 0x10 * k, 1.0);
        }
        let x = fx.mem.i32(D + 0xB0) + 1;
        fx.mem.set32(D + 0xB0, x);
        let x = fx.mem.i32(D + 0xB4) + 1;
        fx.mem.set32(D + 0xB4, if x < ne { x } else { 0 });
        let x = fx.mem.i32(ci) + 1;
        fx.mem.set32(ci, if x < ne { x } else { 0 });
    }

    // The emitters drawn, each spawning into its sub-pools.
    sprite_head(fx, d);
    alpha(fx, d, 1);
    texture(fx, d, 0x10);
    fx.desc_clamp(d, 5);
    cam(fx, v, sp);
    uv(fx, d + 0x18, 0x400);
    uv(fx, d + 0x1A, 0);
    uv(fx, d + 0x28, 0x600);
    uv(fx, d + 0x2A, 0x200);
    corners(fx, &s, 14.0, true);
    let s0 = d + 0x28;
    for i in 0..v.emitters {
        let e = D + 0x40 * i;
        if fx.mem.read_u8(e + 0xC0) == 0 {
            continue;
        }
        rgba(fx, d + 0x10, v.c_main, 0xFF);
        let f20 = ee::div(ee::from_i32(fx.mem.i32(e + 0xC4)), ee::from_i32(fx.mem.i32(e + 0xC8)));
        let a0 = fx.mem.i32(e + 0xCC);
        fx.mem.set32(e + 0xCC, if a0 + 1 < 0x10 { a0 + 1 } else { 0 });
        let a1 = fx.mem.i32(e + 0xD0);
        fx.mem.set32(e + 0xD0, if a1 + 1 < 0x10 { a1 + 1 } else { 0 });
        let sa = D + v.da + 0x600 * i + 0x60 * a0 as u32;
        let sb = D + v.db + 0xA00 * i + 0xA0 * a1 as u32;
        fx.vu_scale4(s.at, e + 0xF0, f20);
        fx.vu_add4(s.at, s.at, e + 0xE0);
        draw(fx, &s, false);
        fx.mem.write_u8(sa, 1);
        fx.mem.write_u32(sa + 4, 0);
        words(fx, sa + 0x10, s.at);
        for k in 0..4u32 {
            let x = r_rem(fx, s0, v.ra(0x01E0_13F0), 8);
            fx.mem.write_u8(sa + 8 + k, x as u8);
            fx.random_table_vec(sa + 0x20 + 0x10 * k, 0.5);
        }
        fx.mem.write_u8(sb, 1);
        fx.mem.write_u32(sb + 4, 0);
        words(fx, sb + 0x10, s.at);
        for k in 0..8u32 {
            let x = fx.rand_from(s0, v.ra(0x01E0_1480)) >> 23;
            let (_, r) = fx.div(x, 3);
            fx.mem.write_u8(sb + 8 + k, r as u8);
            fx.random_table_vec(sb + 0x20 + 0x10 * k, 1.0);
        }
        let age = fx.mem.i32(e + 0xC4) + 1;
        fx.mem.set32(e + 0xC4, age);
        if fx.mem.i32(e + 0xC8) < age {
            fx.mem.write_u8(e + 0xC0, 0);
        }
    }

    // Sub-pool A (texture +0x14, 4 motes a record).
    sprite_head(fx, d);
    alpha(fx, d, 1);
    texture(fx, d, 0x14);
    fx.desc_clamp(d, 5);
    cam(fx, v, sp);
    for i in 0..v.emitters {
        for j in 0..0x10u32 {
            let r = D + v.da + 0x600 * i + 0x60 * j;
            if fx.mem.read_u8(r) == 0 {
                continue;
            }
            let age = fx.mem.i32(r + 4);
            let k = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(0x10 - age)), 16.0)));
            corners(fx, &s, k, true);
            let age = fx.mem.i32(r + 4);
            rgba(fx, d + 0x10, v.c_pool_a, (((0x10 - age) << 7) >> 4) as u8);
            let age = fx.mem.i32(r + 4);
            let t = fx.tan_falloff_p(age, 0x10, 1);
            let f20 = ee::add(f(0x3ECC_CCCD), ee::mul(f(0x3F19_999A), t));
            for k in 0..4u32 {
                let c = i32::from(fx.mem.read_u8(r + 8 + k));
                uv_cell(fx, d, c);
                mote(fx, &s, r + 0x20 + 0x10 * k, f20, r + 0x10, false);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x11 {
                fx.mem.write_u8(r, 0);
            }
        }
    }

    // Sub-pool B (texture +0x10, 8 motes a record).
    sprite_head(fx, d);
    alpha(fx, d, 1);
    texture(fx, d, 0x10);
    fx.desc_clamp(d, 5);
    cam(fx, v, sp);
    for i in 0..v.emitters {
        for j in 0..0x10u32 {
            let r = D + v.db + 0xA00 * i + 0xA0 * j;
            if fx.mem.read_u8(r) == 0 {
                continue;
            }
            let age = fx.mem.i32(r + 4);
            let k = ee::mul(2.5, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(0x10 - age)), 16.0)));
            corners(fx, &s, k, true);
            let age = fx.mem.i32(r + 4);
            rgba(fx, d + 0x10, v.c_main, a255(0x10 - age, 4));
            let age = fx.mem.i32(r + 4);
            let t = fx.tan_falloff_p(age, 0x10, 1);
            let f20 = ee::add(f(0x3DCC_CCCD), ee::mul(f(0x3F66_6666), t));
            for k in 0..8u32 {
                let c = i32::from(fx.mem.read_u8(r + 8 + k));
                uv_cell(fx, d, c);
                mote(fx, &s, r + 0x20 + 0x10 * k, f20, r + 0x10, false);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x11 {
                fx.mem.write_u8(r, 0);
            }
        }
    }

    // The mote records: a glow, 16 big motes, 32 small ones.
    sprite_head(fx, d);
    cam(fx, v, sp);
    for j in 0..v.emitters {
        let c = ci + 0x10 + 0x350 * j;
        if fx.mem.read_u8(c) == 0 {
            continue;
        }
        alpha(fx, d, 1);
        texture(fx, d, 0x10);
        fx.desc_clamp(d, 5);
        uv_fixed(fx, d);
        corners(fx, &s, 150.0, true);
        let age = fx.mem.i32(c + 4);
        let t = fx.tan_falloff_p(age, 0x20, 1);
        let a = fix(fx, ee::mul(128.0, ee::sub(1.0, t)), v.ra(0x01E0_1E3C));
        rgba(fx, d + 0x10, v.c_main, a as u8);
        words(fx, s.at, c + 0x10);
        draw(fx, &s, true);

        alpha(fx, d, 0);
        texture(fx, d, 0x14);
        fx.desc_clamp(d, 5);
        let age = fx.mem.i32(c + 4);
        let k = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(age)), 32.0)));
        corners(fx, &s, k, true);
        let age = fx.mem.i32(c + 4);
        rgba(fx, d + 0x10, v.c_main, (((0x20 - age) << 7) >> 5) as u8);
        let age = fx.mem.i32(c + 4);
        let f20 = fx.tan_falloff_p(age, 0x20, 1);
        for k in 0..0x10u32 {
            let cell = i32::from(fx.mem.read_u8(c + 0x20 + k));
            uv_cell(fx, d, cell);
            mote(fx, &s, c + 0x30 + 0x10 * k, f20, c + 0x10, false);
        }

        alpha(fx, d, 1);
        texture(fx, d, 0x10);
        fx.desc_clamp(d, 5);
        let age = fx.mem.i32(c + 4);
        let k = ee::mul(2.0, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(0x20 - age)), 32.0)));
        corners(fx, &s, k, true);
        let age = fx.mem.i32(c + 4);
        rgba(fx, d + 0x10, v.c_main, a255(0x20 - age, 5));
        let age = fx.mem.i32(c + 4);
        let f20 = fx.tan_falloff_p(age, 0x20, 1);
        for k in 0..0x20u32 {
            let cell = i32::from(fx.mem.read_u8(c + 0x130 + k));
            uv_cell(fx, d, cell);
            mote(fx, &s, c + 0x150 + 0x10 * k, f20, c + 0x10, false);
        }
        let age = fx.mem.i32(c + 4) + 1;
        fx.mem.set32(c + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(c, 0);
        }
    }
}

/// The spark records (16 x 0x130: +8 16 cells, +0x20 position, +0x30 16
/// vectors), one a frame at node 0x15 [F 0x1E02540..0x1E02BF0].
fn sparks(fx: &mut Fx, v: &V, sp: u32) {
    let s = slots(v, sp);
    let d = s.d;
    let n = fx.mem.i32(D);
    let s0 = d + 0x28;
    if n > v.sparks.0 && n < v.sparks.1 {
        let r = D + v.dr + 0x130 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x20, D + 0x30);
        for k in 0..0x10u32 {
            let x = r_rem(fx, s0, v.ra(0x01E0_25C8), 4);
            fx.mem.write_u8(r + 8 + k, x as u8);
            fx.random_table_vec(r + 0x30 + 0x10 * k, 1.0);
        }
    }
    sprite_head(fx, d);
    fx.desc_clamp(d, 5);
    cam(fx, v, sp);
    for i in 0..0x10u32 {
        let r = D + v.dr + 0x130 * i;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        texture(fx, d, 0x10);
        alpha(fx, d, 1);
        let age = fx.mem.i32(r + 4);
        let k = ee::add(f(0x3E4C_CCCD), ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(0x10 - age)), 16.0));
        corners(fx, &s, k, true);
        let age = fx.mem.i32(r + 4);
        let c = fx.tcos((age << 14) >> 4);
        let a = fix(fx, ee::mul(255.0, c), v.ra(0x01E0_2770));
        rgba(fx, d + 0x10, v.c_main, a as u8);
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, 0x10, 1);
        let f20 = ee::add(f(0x3DCC_CCCD), ee::mul(f(0x3F66_6666), t));
        for k in 0..0x10u32 {
            let cell = i32::from(fx.mem.read_u8(r + 8 + k));
            uv_cell(fx, d, cell);
            mote(fx, &s, r + 0x30 + 0x10 * k, f20, r + 0x20, false);
        }
        texture(fx, d, 0x18);
        alpha(fx, d, 1);
        let age = fx.mem.i32(r + 4);
        let k = ee::mul(8.0, ee::add(f(0x3E4C_CCCD), ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(age)), 16.0)));
        corners(fx, &s, k, true);
        let age = fx.mem.i32(r + 4);
        let c = fx.tcos((age << 14) >> 4);
        let a = fix(fx, ee::mul(v.spark2_a, c), v.ra(0x01E0_2A00));
        rgba(fx, d + 0x10, v.c_spark2, a as u8);
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, 0x10, 1);
        let f20 = ee::mul(f(0xBF33_3333), ee::add(f(0x3DCC_CCCD), ee::mul(f(0x3F66_6666), t)));
        for k in 0..0x10u32 {
            let cell = i32::from(fx.mem.read_u8(r + 8 + k)) + 4;
            uv_cell(fx, d, cell);
            mote(fx, &s, r + 0x30 + 0x10 * k, f20, r + 0x20, false);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The ring between nodes 0x16 / 0x17 (0x21 updates): a glow, the band of
/// 32 quads (0x100BA0 x 2: middle radius to inner / outer), 128 motes
/// (cells +0x640, vectors +0x6D0) about the centre +0x6C0 [F
/// 0x1E02BF4..0x1E035A8].
fn ring(fx: &mut Fx, v: &V, sp: u32) {
    let s = slots(v, sp);
    let d = s.d;
    let g = D + v.dring;
    let s0 = d + 0x28;
    if fx.mem.i32(D) == v.ring_n {
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        fx.vu_add4(s.at, D + 0x50, D + 0x60);
        fx.vu_scale4(s.at, s.at, 0.5);
        words(fx, g + 0x6C0, s.at);
        for k in 0..0x80u32 {
            let x = r_rem(fx, s0, v.ra(0x01E0_2C74), 4);
            fx.mem.write_u8(g + 0x640 + k, x as u8);
            fx.random_table_vec(g + 0x6D0 + 0x10 * k, 1.0);
        }
    }
    if fx.mem.read_u8(g) != 1 {
        return;
    }
    sprite_head(fx, d);
    texture(fx, d, 0x10);
    alpha(fx, d, 1);
    fx.desc_clamp(d, 5);
    uv_fixed(fx, d);
    fx.camera_vp_to(s.m);
    fx.translate_rows(s.id, s.turn, g + 0x6C0);
    crate::blib::exe::mul_mem(fx, s.m, s.id);
    let age = fx.mem.i32(g + 4);
    let t = fx.tan_falloff_p(age, 0x20, 1);
    let a = fix(fx, ee::mul(255.0, ee::sub(1.0, t)), v.ra(0x01E0_2DEC));
    rgba(fx, d + 0x10, v.c_main, a as u8);
    corners(fx, &s, 48.0, false);
    if crate::fxlib2::project(fx, s.c, PROJ_OUT, s.m, d, s.w) {
        far(fx, d + 0x20, &[d + 0x30]);
        fx.submit(d, OT, 0);
    }

    let q = sp + v.s(0x658);
    head(fx, q, 0x14, 8, 0x4141_4141, 0);
    alpha(fx, q, 1);
    fx.camera_vp_to(s.m);
    fx.translate_rows(s.id, s.turn, g + 0x6C0);
    let age = fx.mem.i32(g + 4);
    let t = fx.tan_falloff_p(age, 0x20, 1);
    let f20 = ee::add(f(0x3DCC_CCCD), ee::mul(f(0x3F66_6666), t));
    vec4(fx, s.at, [f20.to_bits(), f20.to_bits(), 0, ONE]);
    fx.scale_rows(s.id, s.at);
    crate::blib::exe::mul_mem(fx, s.m, s.id);
    let age = fx.mem.i32(g + 4);
    rgba(fx, q + 0x10, v.c_main, a255(0x20 - age, 5));
    copy8(fx, q + 0x20, q + 0x10);
    rgba(fx, q + 0x30, [0xFF, 0, 0], 0);
    copy8(fx, q + 0x40, q + 0x30);
    let ras = v.ras(&[0x01E0_3098, 0x01E0_31A0]);
    for k in 0..0x20u32 {
        let a = g + 0x30 * k;
        let b = a + 0x30;
        for (o, ra) in [(0x10u32, ras[0]), (0x30, ras[1])] {
            if fx.project_n(&[a + 0x20, b + 0x20, a + o, b + o], s.m, q, s.w, ra) == 1 {
                far(fx, q + 0x18, &[q + 0x28, q + 0x38, q + 0x48]);
                fx.submit(q, OT, 0);
            }
        }
    }

    sprite_head(fx, d);
    texture(fx, d, 0x10);
    alpha(fx, d, 1);
    fx.desc_clamp(d, 5);
    cam(fx, v, sp);
    let age = fx.mem.i32(g + 4);
    rgba(fx, d + 0x10, v.c_main, a255(0x20 - age, 5));
    corners(fx, &s, 1.5, true);
    for k in 0..0x80u32 {
        let cell = i32::from(fx.mem.read_u8(g + 0x640 + k));
        uv_cell(fx, d, cell);
        mote(fx, &s, g + 0x6D0 + 0x10 * k, f20, g + 0x6C0, true);
    }
    let age = fx.mem.i32(g + 4) + 1;
    fx.mem.set32(g + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(g, 0);
    }
}

/// The bursts at the target (4 x 0x10E0 after the frame fan's flag: +4 age
/// from 1, +8 64 halfword cells, +0x90 position, +0xA0 64 vectors, +0x4A0
/// 128 vectors, +0xCA0 64 byte cells 5..7, +0xCE0 64 vectors) [F
/// 0x1E035AC..0x1E04624].
fn bursts(fx: &mut Fx, v: &V, sp: u32) {
    let s = slots(v, sp);
    let d = s.d;
    let s0 = d + 0x28;
    let ff = D + v.dff;
    let n = fx.mem.i32(D);
    if let Some(i) = v.beats.iter().position(|&b| b == n) {
        let i = i as i32;
        fx.mem.set32(ff + 8, i);
        let g = ff + 0x10 + 0x10E0 * i as u32;
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 1);
        vec4(fx, g + 0x90, [0, 0, 0, ONE]);
        fx.vu_add4(g + 0x90, g + 0x90, D + 0x80);
        for k in 0..0x40u32 {
            let x = r_rem(fx, s0, v.ra(0x01E0_3764), 8);
            fx.mem.write_u16(g + 8 + 2 * k, x as u16);
            fx.random_table_vec(g + 0xA0 + 0x10 * k, v.burst_len[0]);
        }
        for k in 0..0x80u32 {
            fx.random_table_vec(g + 0x4A0 + 0x10 * k, v.burst_len[1]);
        }
        for k in 0..0x40u32 {
            let x = fx.rand_from(s0, v.ra(0x01E0_3854)) >> 23;
            let (_, r) = fx.div(x, 3);
            fx.mem.write_u8(g + 0xCA0 + k, (r + 5) as u8);
            fx.random_table_vec(g + 0xCE0 + 0x10 * k, v.burst_len[2]);
        }
    }
    for i in 0..4u32 {
        let g = ff + 0x10 + 0x10E0 * i;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let pos = g + 0x90;
        // Big sparks (texture +0x18).
        sprite_head(fx, d);
        texture(fx, d, 0x18);
        fx.desc_clamp(d, 5);
        alpha(fx, d, 1);
        cam(fx, v, sp);
        let age = fx.mem.i32(g + 4);
        let k = ee::mul(v.burst_s.0, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(age)), 32.0)));
        corners(fx, &s, k, true);
        let age = fx.mem.i32(g + 4);
        rgba(fx, d + 0x10, v.c_main, ((0x20 - age).wrapping_mul(200) >> 5) as u8);
        let age = fx.mem.i32(g + 4);
        let f22 = fx.tan_falloff_p(age, 0x20, 1);
        for k in 0..0x40u32 {
            let cell = i32::from(fx.mem.read_u8(g + 0xCA0 + k));
            uv_cell(fx, d, cell);
            mote(fx, &s, g + 0xCE0 + 0x10 * k, f22, pos, true);
        }
        // Two-blend motes (texture +0x14).
        sprite_head(fx, d);
        texture(fx, d, 0x14);
        fx.desc_clamp(d, 5);
        cam(fx, v, sp);
        let age = fx.mem.i32(g + 4);
        let k = ee::mul(v.burst_s.1, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(age)), 32.0)));
        corners(fx, &s, k, true);
        let age = fx.mem.i32(g + 4);
        let x = 0x20 - age;
        let a128 = ((x << 7) >> 5) as u8;
        let (ae, ao, ce, co) = if v.b711() {
            (a128, (x.wrapping_mul(228) >> 5) as u8, [0x80, 0x40, 0x20], [0xFF, 0x90, 0x60])
        } else {
            (a128, a128, [8, 8, 8], [0xFF, 0xC8, 0xC8])
        };
        for k in 0..0x40u32 {
            let age = fx.mem.i32(g + 4);
            let mut f22 = fx.tan_falloff_p(age, 0x20, 1);
            if rem2(k as i32, 2) == 0 {
                f22 = ee::mul(f22, f(0x3F4C_CCCD));
                alpha(fx, d, 0);
                rgba(fx, d + 0x10, ce, ae);
            } else {
                alpha(fx, d, 1);
                rgba(fx, d + 0x10, co, ao);
            }
            fx.vu_scale4(s.at, g + 0xA0 + 0x10 * k, f22);
            fx.vu_add4(s.at, s.at, pos);
            let cell = i32::from(fx.mem.read_u16(g + 8 + 2 * k));
            uv_cell(fx, d, cell);
            draw(fx, &s, true);
        }
        // The glow (texture +0x10) in a scaled matrix at the burst.
        sprite_head(fx, d);
        texture(fx, d, 0x10);
        fx.desc_clamp(d, 5);
        alpha(fx, d, 1);
        fx.camera_vp_to(s.m);
        fx.translate_rows(s.id, s.turn, pos);
        let age = fx.mem.i32(g + 4);
        let t = fx.tan_falloff_p(age, 0x20, 1);
        let k = ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), t));
        vec4(fx, s.at, [k.to_bits(), k.to_bits(), k.to_bits(), ONE]);
        fx.scale_rows(s.id, s.at);
        crate::blib::exe::mul_mem(fx, s.m, s.id);
        let age = fx.mem.i32(g + 4);
        let (amax, c) = if v.b711() { (255.0, [0xFF, 0x64, 0x32]) } else { (200.0, [0xFF, 0, 0]) };
        let a = fix(fx, ee::mul(amax, ee::div(ee::from_i32(0x20 - age), 32.0)), v.ra(0x01E0_4170));
        rgba(fx, d + 0x10, c, a as u8);
        uv_fixed(fx, d);
        corners(fx, &s, v.glow_r, false);
        if crate::fxlib2::project(fx, s.c, PROJ_OUT, s.m, d, s.w) {
            far(fx, d + 0x20, &[d + 0x30]);
            fx.submit(d, OT, 0);
        }
        // 128 streaks of six sprites (texture +0x10), keyed by w.
        sprite_head(fx, d);
        texture(fx, d, 0x10);
        fx.desc_clamp(d, 5);
        alpha(fx, d, 1);
        uv_fixed(fx, d);
        cam(fx, v, sp);
        corners(fx, &s, 1.0, true);
        let age = fx.mem.i32(g + 4);
        let f23 = ee::div(ee::from_i32(0x20 - age), 32.0);
        let age = fx.mem.i32(g + 4);
        let f22 = fx.tan_falloff_p(age, 0x20, 1);
        let ra = (v.ra(0x01E0_44CC), v.ra(0x01E0_4584));
        for j in 0..0x80u32 {
            for s5 in 0..6i32 {
                let q = crate::b440::div12(fx, (s5 + 6) * 255);
                let a = fix(fx, ee::mul(f23, ee::from_i32(q)), ra.0);
                rgba(fx, d + 0x10, v.c_main, a as u8);
                let k = ee::add(f22, ee::mul(f(0x3BE5_6042), ee::from_i32(s5)));
                fx.vu_scale4(s.at, g + 0x4A0 + 0x10 * j, k);
                fx.vu_add4(s.at, s.at, pos);
                fx.vu_add4(s.p, s.c, s.at);
                fx.vu_add4(s.p + 0x10, s.c + 0x10, s.at);
                if crate::fxlib2::project(fx, s.p, PROJ_OUT, s.m, d, s.w) {
                    let w = fx.mem.f(s.w);
                    let key = fix(fx, w, ra.1);
                    fx.submit(d, OT, key as i32);
                }
            }
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// b216's vortex (D+0xE044, 0xD9 updates from 8): two passes of a 33 x 17
/// grid (D+0xB470, rows 0x110: radius 180 + 40 p + 20 sin(phase), height
/// -3000 + 6000 r / 32) drawn as 32 x 16 quads by 0x100BA0 (prim 0x24,
/// texture +0x1C, (38, 10, 10)), ST drifting [F 0x1E04628..0x1E04C60].
fn vortex(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 8 {
        fx.mem.write_u8(D + 0xE044, 1);
        fx.mem.write_u32(D + 0xE048, 0);
    }
    if fx.mem.read_u8(D + 0xE044) != 1 {
        return;
    }
    let q = sp + 0x5E0;
    head(fx, q, 0x24, 9, 0x4_2424_2421, 0);
    alpha(fx, q, 0);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 0);
    let age = ee::from_i32(fx.mem.i32(D + 0xE048));
    let f20 = Fx::ratio_below(age, 16.0);
    let age = ee::from_i32(fx.mem.i32(D + 0xE048));
    let f0 = Fx::ramp_above(age, 216.0, 184.0);
    let f1 = ee::mul(f20, f0);
    let f22 = ee::mul(64.0, f1);
    let f23 = ee::mul(116.0, f1);
    let (mut phase, mut drift) = (0i32, 0i32);
    for p in 0..2i32 {
        fx.camera_vp_to(sp + 0x460);
        fx.identity(sp + 0x4A0);
        vec4(fx, sp + 0x5A0, [0, 0x4248_0000, 0, ONE]);
        fx.translate_rows(sp + 0x4A0, sp + 0x4A0, sp + 0x5A0);
        crate::blib::exe::mul_mem(fx, sp + 0x460, sp + 0x4A0);
        let f20 = ee::add(180.0, ee::mul(2.0, ee::mul(20.0, ee::from_i32(p))));
        let s4 = phase >> 1;
        for r in 0..0x21u32 {
            let f21 = ee::add(-3000.0, ee::div(ee::mul(6000.0, ee::from_i32(r as i32)), 32.0));
            for c in 0..0x10u32 {
                let ph = fx.mem.i32(D + 0x44 * r + 4 * c + 0xD780);
                let age = fx.mem.i32(D + 0xE048);
                fx.mult(age, 0x8AE);
                let a = ph.wrapping_add(s4).wrapping_add(age.wrapping_mul(0x8AE)) & 0xFFFF;
                let s = fx.tsin(a);
                let f24 = ee::add(f20, ee::mul(20.0, s));
                let s6 = ((c << 16) as i32) >> 4;
                let pt = D + 0x110 * r + 0x10 * c + 0xB470;
                let s = fx.tsin(s6);
                fx.mem.setf(pt, ee::mul(f24, s));
                let cs = fx.tcos(s6);
                fx.mem.setf(pt + 4, ee::mul(f24, cs));
                fx.mem.setf(pt + 8, f21);
                fx.mem.write_u32(pt + 0xC, ONE);
            }
            let row = D + 0x110 * r + 0xB470;
            words(fx, row + 0x100, row);
        }
        let age = fx.mem.i32(D + 0xE048);
        let a0 = (rem2(age.wrapping_add(drift), 0x80) << 16) >> 7;
        let sign = rem2(p, 2) * 2 - 1;
        fx.mult(sign, a0);
        let s0 = sign.wrapping_mul(a0) & 0xFFFF;
        let f24 = fx.tsin(s0);
        fx.tcos(s0);
        let age = fx.mem.i32(D + 0xE048);
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let f25 = ee::add(ee::mul(ee::div(ee::mul(2.0, ee::from_i32(rem2(age, 0x20))), 32.0), side), 0.0);
        for r in 0..0x20i32 {
            let m = rem2(r, 2);
            let t0 = ee::div(ee::from_i32(m), 2.0);
            let t1 = ee::div(ee::from_i32(m + 1), 2.0);
            let s = fx.tsin(((r as u32) << 15) as i32 >> 5);
            let a = if p == 0 { fix(fx, ee::mul(f22, s), 0x01E0_4A80) } else { fix(fx, ee::mul(f23, s), 0x01E0_4A94) };
            let t0 = ee::add(t0, f25);
            let t1 = ee::add(t1, f25);
            rgba(fx, sp + 0x5F0, [0x38, 0x10, 0x10], a as u8);
            let r0 = D + 0x110 * r as u32 + 0xB470;
            let r1 = r0 + 0x110;
            for c in 0..0x10i32 {
                let m = rem2(c, 2);
                let s2 = ee::div(ee::from_i32(m), 2.0);
                let s3 = ee::div(ee::from_i32(m + 1), 2.0);
                let s2 = ee::add(s2, f24);
                let s3 = ee::add(s3, f24);
                for (o, s, t) in [(0x5F8u32, s2, t0), (0x608, s3, t0), (0x618, s2, t1), (0x628, s3, t1)] {
                    fx.mem.setf(sp + o, s);
                    fx.mem.setf(sp + o + 4, t);
                }
                let (a, b) = (0x10 * c as u32, 0x10 * (c + 1) as u32);
                if fx.project_n(&[r0 + a, r0 + b, r1 + a, r1 + b], sp + 0x460, q, sp + 0x78C, 0x01E0_4B8C) == 1 {
                    fx.submit(q, OT, 0xFFFF);
                }
            }
        }
        phase = phase.wrapping_add(0x3_3333);
        drift += 0x20;
    }
    let age = fx.mem.i32(D + 0xE048) + 1;
    fx.mem.set32(D + 0xE048, age);
    if age >= 0xD9 {
        fx.mem.write_u8(D + 0xE044, 0);
    }
}

/// b711's column records (D+0xA0F0, 32 x 0x208: +4 age, +8 64 life bytes,
/// +0x48 64 angles, +0x148 64 x 3 colour bytes), one a frame on 9..0xFA;
/// each live strip a quad (prim 0x34, texture +0x10) 400 f x 20 f in the
/// camera placed at (6000 age / life - 3000, 50 + 360 f cos, 360 f sin)
/// turned about x by its angle (f = 0.5 + 0.5 k / 64), alpha 255
/// ratio(age, 16) ramp_above(age, life, life - 16) clamp(n - 8, 0, 0x20)
/// clamp(0xFB - n, 0, 0x20) / 1024 [F B19s 0x1E04BF4..0x1E05384].
fn columns(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (9..0xFB).contains(&n) {
        let r = D + 0x208 * rem2(n, 0x20) as u32 + 0xA0F0;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for k in 0..0x40u32 {
            let s0 = r - 0xA0F0 + k;
            let c = r_rem(fx, s0, 0x01E0_4C6C, 0x100);
            let l = fix(fx, ee::mul(32.0, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(c)), 256.0))), 0x01E0_4CBC);
            fx.mem.write_u8(r + 8 + k, l as u8);
            let c = r_rem(fx, s0, 0x01E0_4CCC, 0x100);
            fx.mem.set32(r + 0x48 + 4 * k, (c << 16) >> 8);
            for (j, (amax, ra, rf)) in [(200.0f32, 0x01E0_4CFCu32, 0x01E0_4D60u32), (150.0, 0x01E0_4D70, 0x01E0_4DD4), (255.0, 0x01E0_4DE4, 0x01E0_4E48)].into_iter().enumerate() {
                let c = r_rem(fx, s0, ra, 0x100);
                let x = ee::add(f(0x3F19_999A), ee::div(ee::mul(f(0x3ECC_CCCD), ee::from_i32(c)), 256.0));
                let b = fix(fx, ee::mul(amax, x), rf);
                fx.mem.write_u8(r + 0x148 + 3 * k + j as u32, b as u8);
            }
        }
    }
    let q = sp + 0x678;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, q, 1);
    texture(fx, q, 0x10);
    fx.desc_clamp(q, 5);
    let n = fx.mem.i32(D);
    let f20 = ee::from_i32(Fx::clamp_i(n - 8, 0, 0x20));
    let n = fx.mem.i32(D);
    let k = Fx::clamp_i(0xFB - n, 0, 0x20);
    let f21 = ee::div(ee::mul(f20, ee::from_i32(k)), 1024.0);
    for i in 0..0x20u32 {
        let r = D + 0x208 * i + 0xA0F0;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        for k in 0..0x40u32 {
            let age = fx.mem.i32(r + 4);
            let life = i32::from(fx.mem.read_u8(r + 8 + k));
            if life < age {
                continue;
            }
            fx.camera_vp_to(sp + 0x420);
            fx.identity(sp + 0x460);
            let age = fx.mem.i32(r + 4);
            let life = i32::from(fx.mem.read_u8(r + 8 + k));
            let x = ee::div(ee::mul(6000.0, ee::from_i32(age)), ee::from_i32(life));
            let a = fx.mem.i32(r + 0x48 + 4 * k);
            fx.mem.setf(sp + 0x560, ee::sub(x, 3000.0));
            let f20 = ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(k as i32)), 64.0));
            let f22 = ee::mul(360.0, f20);
            let c = fx.tcos(a);
            fx.mem.setf(sp + 0x564, ee::add(50.0, ee::mul(f22, c)));
            let s = fx.tsin(a);
            fx.mem.setf(sp + 0x568, ee::mul(f22, s));
            fx.mem.write_u32(sp + 0x56C, ONE);
            fx.translate_rows(sp + 0x460, sp + 0x460, sp + 0x560);
            fx.rotate_axis(sp + 0x460, a, 0);
            crate::blib::exe::mul_mem(fx, sp + 0x420, sp + 0x460);
            let life = i32::from(fx.mem.read_u8(r + 8 + k));
            let age = fx.mem.i32(r + 4);
            let f22 = Fx::ramp_above(ee::from_i32(age), ee::from_i32(life), ee::from_i32((life & 0xFF) - 0x10));
            let age = fx.mem.i32(r + 4);
            let q16 = Fx::ratio_below(ee::from_i32(age), 16.0);
            let al = fix(fx, ee::mul(f21, ee::mul(ee::mul(255.0, q16), f22)), 0x01E0_5144);
            let cb = r + 0x148 + 3 * k;
            for j in 0..3u32 {
                let b = fx.mem.read_u8(cb + j);
                fx.mem.write_u8(q + 0x10 + j, b);
            }
            fx.mem.write_u8(q + 0x13, al as u8);
            fx.mem.write_u32(q + 0x14, ONE);
            for o in [0x28u32, 0x40, 0x58] {
                copy8(fx, q + o, q + 0x10);
            }
            let f5 = ee::mul(20.0, f20);
            let f2 = -f5;
            let f3 = ee::mul(20.0, f2);
            let f4 = ee::mul(20.0, f5);
            vec4(fx, sp + 0x4E0, [f3.to_bits(), 0, f2.to_bits(), ONE]);
            vec4(fx, sp + 0x4F0, [f3.to_bits(), 0, f5.to_bits(), ONE]);
            vec4(fx, sp + 0x500, [f4.to_bits(), 0, f2.to_bits(), ONE]);
            vec4(fx, sp + 0x510, [f4.to_bits(), 0, f5.to_bits(), ONE]);
            let t2 = rem2(k as i32, 8);
            let s0 = ee::div(ee::mul(32.0, ee::from_i32(rem2(t2, 4))), 128.0);
            let t0 = ee::div(ee::mul(32.0, ee::from_i32(t2 >> 2)), 64.0);
            let s1 = ee::add(0.25, s0);
            let t1 = ee::add(0.5, t0);
            for (o, ss, tt) in [(0x18u32, s0, t0), (0x30, s1, t0), (0x48, s0, t1), (0x60, s1, t1)] {
                fx.mem.setf(q + o, ss);
                fx.mem.setf(q + o + 4, tt);
            }
            let m = fx.read_m4(sp + 0x420);
            if fx.project_desc(sp + 0x4E0, PROJ_OUT, Some(&m), q, sp + 0x76C) {
                fx.submit(q, OT, 0xFFFF);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The attacker pass (0xA1 updates): the attacker drawn into its target
/// with the callback, composited by widening fans about its screen point
/// D+0x90 (b216: one 1.5, alpha 80 ratio(age, 32) / 80 ratio(age, 128),
/// white; b711: six 1 + 0.03 j - 0.075, alpha 48 ratio(age, 45) sin(((j +
/// 1) << 15) / 7), grey) [F 0x1E04C64..0x1E051AC].
fn attacker_pass(fx: &mut Fx, v: &V, sp: u32) {
    let at = D + v.datt;
    if fx.mem.i32(D) == v.attacker {
        fx.mem.write_u8(at, 1);
        fx.mem.write_u32(at + 4, 0);
    }
    if fx.mem.read_u8(at) != 1 {
        return;
    }
    let d = sp + v.s(0x6B0);
    let a = fan_desc(fx, d - 0x4C0, 0);
    let layer = fx.layer_begin();
    fx.texflush(a);
    fx.mem.write_u32(v.d_static, D);
    let band = v.band;
    let (_, regs) = fx.unit_draw_post(1, d + 0x60, d + 0x68, v.cb, &mut |fx, p| callback(fx, p, band));
    fx.texflush(regs);
    let tmp = sp + v.s(0x560);
    let ras = v.ras(&FAN_A);
    if v.b711() {
        let age = ee::from_i32(fx.mem.i32(at + 4));
        let f20 = Fx::ratio_below(age, 45.0);
        let age = ee::from_i32(fx.mem.i32(at + 4));
        let f21 = Fx::ratio_below(age, 45.0);
        for j in 0..6i32 {
            let x = ee::div(ee::mul(f(0x3E19_999A), ee::from_i32(j)), 5.0);
            let s = ee::sub(ee::add(1.0, x), f(0x3D99_999A));
            let a = (j + 1) << 15;
            fx.mult(0x9249_2493u32 as i32, a);
            let hi = fx.hilo().1;
            let s0 = (hi.wrapping_add(a) >> 2).wrapping_add(((a as u32) >> 31) as i32);
            let sn = fx.tsin(s0);
            let a0 = fix(fx, ee::mul(f20, ee::mul(48.0, sn)), 0x01E0_4810) & 0xFF;
            let sn = fx.tsin(s0);
            let a1 = fix(fx, ee::mul(f21, ee::mul(48.0, sn)), 0x01E0_4834);
            rgba(fx, d + 0x10, [0x80; 3], a0 as u8);
            rgba(fx, d + 0x28, [0x80; 3], a1 as u8);
            copy8(fx, d + 0x40, d + 0x28);
            fan(fx, d, D + 0x90, tmp, s, 0, &ras);
        }
    } else {
        let age = ee::from_i32(fx.mem.i32(at + 4));
        let f20 = Fx::ratio_below(age, 32.0);
        let age = ee::from_i32(fx.mem.i32(at + 4));
        let f0 = Fx::ratio_below(age, 128.0);
        let f20 = ee::mul(80.0, f20);
        let f21 = ee::mul(80.0, f0);
        let a0 = fix(fx, f20, 0x01E0_4DDC) & 0xFF;
        let a1 = fix(fx, f21, 0x01E0_4DE8);
        rgba(fx, d + 0x10, [0xFF; 3], a0 as u8);
        rgba(fx, d + 0x28, [0xFF; 3], a1 as u8);
        copy8(fx, d + 0x40, d + 0x28);
        fan(fx, d, D + 0x90, tmp, 1.5, 0, &ras);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(at + 4) + 1;
    fx.mem.set32(at + 4, age);
    if age >= 0xA1 {
        fx.mem.write_u8(at, 0);
    }
}

/// The callback (b216 0x1E052D0 / b711 0x1E054A0)(p) [F]: FRAME_1 masked,
/// the frame drawn (0x207FC0 -1, filter 1) as the texture of a full-window
/// sprite (FF, FF, FF, 80) at ALPHA preset 0, then a band (b216 (80, 00,
/// 00, FF) (0x6BF8, 0x71F8) .. (0x93F8, 0x8DF8); b711 (C8, 46, 00, E4)
/// full-window) at preset 3; FRAME_1 unmasked.
fn callback(fx: &mut Fx, p: u32, band: ([u8; 4], [u16; 4])) -> u32 {
    let sp = fx.enter(0xC0);
    let s0 = frame_mask_packet(fx, p, 0xFF00_0000, [0, 0]);
    let t = sp + 0x68;
    fx.mem.write_u16(t + 2, 0x26);
    fx.mem.write_u16(t, 5);
    fx.mem.write_u64(t + 8, 0x4_2421);
    fx.mem.write_u32(t + 4, 0);
    // 0x2096C0 leaves a1 = FBP | FBW << 16, a2 = FBW << 16, a3 = 0x2E93A0.
    let a = mask_regs(fx);
    fx.boundary(0x0020_EA20, "packet writer 0x20EA20", [s0, a[0], a[1], a[2]]);
    let p1 = fx.texflush_packet(s0);
    // 0x1F0FD0's first ZBUF packet (0x209800, a 0x30 frame) left its `sq
    // ra` (0x1F1044) at 0x1F0FD0's sp - 0x10: this frame's TEX1 slot.
    fx.mem.write_u64(sp + 0xB0, 0x001F_1044);
    fx.mem.write_u64(sp + 0xB8, 0);
    let b2 = u32::from(fx.mem.read_u8(sp + 0xB2));
    let p2 = fx.frame_texture(p1, -1, 1, sp + 0xA8, sp + 0xB0);
    fx.boundary(0x0020_EA20, "packet writer 0x20EA20", [p2, 0, b2, 0]);
    let s1 = fx.texflush_packet(p2);
    alpha(fx, t, 0);
    fx.desc_clamp(t, LENS_CLAMP);
    let fl = fx.mem.read_u16(t + 2);
    fx.mem.write_u16(t + 2, fl | 0x800);
    rgba(fx, sp + 0x78, [0xFF; 3], 0x80);
    uv(fx, sp + 0x80, 0);
    uv(fx, sp + 0x82, 0);
    uv(fx, sp + 0x90, 0x2000);
    uv(fx, sp + 0x92, 0x2000);
    far_xy(fx, sp + 0x88, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x98, 0x8FF8, 0x8FF8);
    let s0 = fx.submit_at(s1, t, 0, 0);
    let u = sp + 0x38;
    head(fx, u, 6, 3, 0x441, 0);
    alpha(fx, u, 3);
    let (c, xy) = band;
    rgba(fx, sp + 0x48, [c[0], c[1], c[2]], c[3]);
    far_xy(fx, sp + 0x50, xy[0], xy[1]);
    far_xy(fx, sp + 0x58, xy[2], xy[3]);
    let v = fx.submit_at(s0, u, 0, 0);
    let r = frame_mask_packet(fx, v, 0, [0, 0]);
    fx.leave(0xC0);
    r
}

/// a1..a3 as 0x2096C0 leaves them.
fn mask_regs(fx: &mut Fx) -> [u32; 3] {
    let a3 = 0x002E_93A0u32;
    let flags = fx.mem.read_u8(a3 + 0x20);
    if flags & 1 != 0 && flags & 2 != 0 {
        let k = fx.mem.read_u32(a3);
        let fbw = (fx.mem.read_u32(0x0029_322C + 0x28 * k) as i32 >> 6) as u32;
        let fbp = fx.mem.read_u32(a3 + 0xC);
        [fbp | (fbw << 16), fbw << 16, 0x28 * k]
    } else {
        let disp = 0x002E_93D0u32;
        let t0 = if fx.mem.read_u32(0x002E_963C) & 1 != 0 { disp + 0x150 } else { disp + 0x60 };
        let fbp = u32::from(fx.mem.read_u16(t0) & 0x1FF);
        let fbw = u32::from(fx.mem.read_u8(t0 + 2) & 0x3F);
        [fbp | (fbw << 16), fbw << 16, a3]
    }
}
