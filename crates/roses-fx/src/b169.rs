//! BMODULE b169 (card 373) and its twin b591 (card 387), hand-ported from
//! the records (linked at 0x1E00000, entries 0x1E04710 / 0x1E03B80, data
//! area 0x1ED0000) [F]: two drifting ribbons (`blib::driftribbon`, +0x70 / +0x10C0,
//! 256 points each, width 150 growing 10 an age, x scaled 1.03 a shift)
//! fed every update from the attacker's node 4 at x = 3 sin of an angle
//! advancing 0x800 (two angles +0x2DF8 / +0x2DFC), drawn from update 52
//! (alpha up by 6 to 0x60, down from 380); 64 flat sprites (+0x2260,
//! `flatsprite::sprites`) falling 3 an update round node 0x18 with the bob
//! amplitude +0x22B4 swinging -30..30, respawned below -200 (the record's
//! 0x1E05490 `fans`); 64 puffs (+0x2730, `Fx::sprites`) rising, x 1.05
//! under 20, respawned above 20; a particle cloud (+0x2C00) on 21..49; the
//! unit's afterimage (+0x2DC0, 64 strips, amplitude 20) from 127 with the
//! target tinted (0x1F75F0); the stage light down to 0x20, up again from
//! 380; three random greys 0x7F.. on the groups each update. Returns 1 on
//! update 117, 6 on 127..379, 2 on 380, 8 after 396. b591 (the [`V`]
//! table): no cloud, the sprites start 0.1 (rand % 2000) below, the puffs
//! from update 30, the ribbons from 73, the afterimage from 128
//! (colour 0x80 / 0x40 / 0x70), the end at 340, returns 1 on 118.
//!
//! Library (record addresses): `colours4` 0x1E00080, `afterimage::strips`
//! 0x1E00120, `Fx::sprites` 0x1E00790, `flatsprite::sprites` 0x1E02080,
//! `particle_quads` 0x1E03030, `driftribbon::push` 0x1E03BC0,
//! `driftribbon::draw` 0x1E03D30; the executable's 0x1F6A60
//! [`crate::b107::target_radius`], 0x103660 [`crate::b079::sub3`],
//! 0x1F75F0 [`crate::b101::unit_tint`].
use crate::b079::sub3;
use crate::b101::unit_tint;
use crate::b107::target_radius;
use crate::b142::fstep_up;
use crate::b436::fstep_down;
use crate::blib::{afterimage, driftribbon, flatsprite};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const N: u32 = D + 0x2DE0;
const LEFT: u32 = D + 0x2DDC;
const BITS: u32 = D + 0x2DE8;
const LIGHT: u32 = D + 0x2E04;
const RIB_A: u32 = D + 0x70;
const RIB_B: u32 = D + 0x10C0;
const SPRITES: u32 = D + 0x2260;
const PUFFS: u32 = D + 0x2730;
const CLOUD: u32 = D + 0x2C00;
const IMAGE: u32 = D + 0x2DC0;
const TENTH: f32 = f32::from_bits(0x3DCC_CCCD);

/// What differs between the two records.
struct V {
    /// The entry's frame (the texture handle at frame - 0x10).
    frame: u32,
    left: i32,
    /// The afterimage's colour (+4, +5, +6).
    image_colour: [u8; 3],
    /// The sprites start 0.1 (rand % 2000) below (b591).
    sprite_drop: bool,
    cloud: bool,
    /// Updates: the ribbons drawn from; the puffs from; the
    /// afterimage (and the return 6) from; the end; returns 1 at; the
    /// fans' respawn alpha 0x40 from (b169 else 0).
    ribbons_from: i32,
    puffs_from: i32,
    image_from: i32,
    end: i32,
    one: i32,
    fan_alpha_from: i32,
    fan_alpha_else: bool,
    /// rand's return addresses: the cloud (2), the sprites' init (angle,
    /// drop, frame), the puffs' init (frame, angle, speed, rise), the
    /// greys (3), the puffs' respawn (angle, speed, rise, frame), the
    /// fans' respawn angle; the xforms' (7).
    r_cloud: [u32; 2],
    r_sprite: [u32; 3],
    r_puff: [u32; 4],
    r_grey: [u32; 3],
    r_respawn: [u32; 4],
    r_fan: u32,
    r_xform: [u32; 7],
}

const B169: V = V {
    frame: 0xA0,
    left: 0x18C,
    image_colour: [0x40, 0x80, 0x80],
    sprite_drop: false,
    cloud: true,
    ribbons_from: 0x34,
    puffs_from: 0,
    image_from: 0x7F,
    end: 0x17C,
    one: 0x75,
    fan_alpha_from: 0x2B,
    fan_alpha_else: true,
    r_cloud: [0x01E0_4908, 0x01E0_493C],
    r_sprite: [0x01E0_4A74, 0, 0x01E0_4AC4],
    r_puff: [0x01E0_4BA4, 0x01E0_4BC0, 0x01E0_4BDC, 0x01E0_4C2C],
    r_grey: [0x01E0_4FF8, 0x01E0_5020, 0x01E0_5048],
    r_respawn: [0x01E0_52A8, 0x01E0_52C4, 0x01E0_5314, 0x01E0_5354],
    r_fan: 0x01E0_55F8,
    r_xform: [0x01E0_4F7C, 0x01E0_4F84, 0x01E0_4F8C, 0x01E0_4F94, 0x01E0_4F9C, 0x01E0_4FA4, 0x01E0_4FAC],
};

const B591: V = V {
    frame: 0x90,
    left: 0x168,
    image_colour: [0x80, 0x40, 0x70],
    sprite_drop: true,
    cloud: false,
    ribbons_from: 0x49,
    puffs_from: 0x1E,
    image_from: 0x80,
    end: 0x154,
    one: 0x76,
    fan_alpha_from: 0x3C,
    fan_alpha_else: false,
    r_cloud: [0x01E0_3D7C, 0x01E0_3DB0],
    r_sprite: [0x01E0_3EE8, 0x01E0_3F04, 0x01E0_3F6C],
    r_puff: [0x01E0_404C, 0x01E0_4068, 0x01E0_4084, 0x01E0_40D4],
    r_grey: [0x01E0_44A0, 0x01E0_44C8, 0x01E0_44F0],
    r_respawn: [0x01E0_4700, 0x01E0_471C, 0x01E0_476C, 0x01E0_47AC],
    r_fan: 0x01E0_4A58,
    r_xform: [0x01E0_4424, 0x01E0_442C, 0x01E0_4434, 0x01E0_443C, 0x01E0_4444, 0x01E0_444C, 0x01E0_4454],
};

/// The entry 0x1E04710.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B169)
}

/// b591's entry 0x1E03B80.
pub fn program_b591(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B591)
}

fn run(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let sp = fx.enter(v.frame);
    let ret = if param >= 0 { init(fx, sp, param, v) } else { step(fx, sp, v) };
    fx.leave(v.frame);
    ret
}

fn halves(fx: &mut Fx, at: u32, v: &[(u32, i32)]) {
    for &(k, x) in v {
        fx.mem.set16(at + k, x);
    }
}

/// A speed (+0 / +8 of the particle at `p`) from the 16-bit angle `a`:
/// `r` cos (0x20ABF0) into +8, `r` sin (0x20ACC0) into +0.
fn fan_dir(fx: &mut Fx, p: u32, a: i32, r: f32) {
    let c = fx.table_trig(a, true);
    fx.mem.setf(p + 8, ee::mul(r, c));
    let s = fx.table_trig(a, false);
    fx.mem.setf(p, ee::mul(r, s));
}

fn rand16(fx: &mut Fx, s0: u32, ra: u32) -> i32 {
    fx.rand_from(s0, ra) & 0xFFFF
}

fn rem(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let r = fx.rand_from(s0, ra);
    fx.div(r, m).1
}

/// rand() % 2^k (the compiler's signed remainder by masks).
fn rand_pow2(fx: &mut Fx, ra: u32, m: i32) -> i32 {
    fx.rand_from(D, ra).wrapping_rem(m)
}

/// A puff's respawn [F 0x1E04BB8 / 0x1E052A0]: speed 0.1 (rand % 60 -
/// 30) along a random angle (rand's frames from the return addresses
/// `ras`, s0 = D), rise 0.1 (rand % `m` - `off`) at `ras[2]`.
fn puff(fx: &mut Fx, p: u32, ras: [u32; 3], m: i32, off: i32) {
    let a = rand16(fx, D, ras[0]);
    let h = rem(fx, D, ras[1], 0x3C) - 0x1E;
    let r = ee::mul(TENTH, ee::from_i32(h));
    fan_dir(fx, p + 0x2790, a, r);
    let h = rem(fx, D, ras[2], m) - off;
    fx.mem.setf(p + 0x2794, ee::mul(TENTH, ee::from_i32(h)));
}

fn init(fx: &mut Fx, sp: u32, param: i32, v: &V) -> i32 {
    let tex_at = sp + v.frame - 0x10;
    fx.bzero(D, 0x2E10);
    for x in [0u32, 0x2110, 0x2180, 0x21F0, 0x26C0, 0x2B90, 0x2D50] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2DECu32, 0x18, 1), (0x2DF0, 0x1A, 0), (0x2DF4, 4, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(LEFT, v.left);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x2DE4, param);
    let r = target_radius(fx);
    fx.mem.setf(D + 0x2E00, r);
    fx.mem.write_u8(LIGHT, 0x80);
    fx.mem.write_u32(BITS, 0);
    fx.mem.write_u32(D + 0x2DF8, 0x4000);
    // The afterimage (+0x2DC0): 64 strips, phase step 0x1000, amplitude 20.
    fx.mem.write_u32(IMAGE, 0x0004_1000);
    for (o, c) in [(6u32, v.image_colour[2]), (5, v.image_colour[1]), (4, v.image_colour[0]), (7, 0x80)] {
        fx.mem.write_u8(IMAGE + o, c);
    }
    fx.mem.write_u32(IMAGE + 0x10, 0);
    fx.mem.write_u32(IMAGE + 0xC, 0x1000);
    fx.mem.write_u16(IMAGE + 8, 0);
    fx.mem.write_u32(IMAGE + 0x14, 0x41A0_0000);
    let t = fx.target_handle();
    fx.mem.set16(IMAGE + 0xA, t);
    for o in [0x1Au32, 0x19, 0x18, 0x1B] {
        fx.mem.write_u8(IMAGE + o, 0x80);
    }
    fx.texture4(tex_at, 0xF);
    // The cloud (+0x2C00): 16 quads of size 5, TEXEFF 15.
    let g = CLOUD;
    let tex = fx.mem.read_u32(tex_at);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x0008_0083);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0x80), (0x2A, 0xC0), (0x24, 0x20), (0x26, 0x20), (0x2C, 0x80), (0x2E, 0x40)]);
    fx.mem.write_u32(g + 0xC, 0x40A0_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 2), (0x13C, 1), (0x140, 8), (0x136, 0), (0x138, 0)]);
    for (k, v) in [(0xAu32, 0x70u8), (9, 0xA0), (8, 0xA0), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, v);
    }
    for k in 0..0x10i32 {
        let p = g + 16 * k as u32;
        for (c, ra) in [(0u32, v.r_cloud[0]), (1, v.r_cloud[1])] {
            let r = fx.rand_from(D, ra);
            let base = fx.mem.f(g + 0x10 + 4 * c);
            let h = fx.div(r, 6).1;
            fx.mem.setf(p + 0x30 + 4 * c, ee::add(base, ee::from_i32(h - 3)));
        }
        let z = fx.mem.read_u32(g + 0x18);
        fx.mem.write_u32(p + 0x38, z);
        let a = fx.mem.i16(g + 0x13A).wrapping_neg();
        let t = fx.mult(a, k);
        fx.mem.set16(p + 0x3E, t);
        fx.mem.set16(p + 0x3C, 0);
        let n = fx.mem.i16(g + 0x140) - 1;
        fx.mem.set16(g + 0x13E, n);
    }
    fx.texture5(tex_at, 0);
    // The flat sprites (+0x2260): 64 of the attacker's texture 0, flags
    // 0x58000003, half size 1.5 growing 5 an age, bobbing 50 at 0x400,
    // cell (0, 0x30, 0x20, 0x20) of a 0x100 x 0x80 sheet, 14 frames every
    // 100 updates.
    let g = SPRITES;
    let tex = fx.mem.read_u32(tex_at);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x5800_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0xC, 0x3FC0_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0x30), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0x30), (0x3C, 0x100), (0x3E, 0x80)]);
    fx.mem.write_u32(g + 0x40, D + 0x22C0);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4000_0000);
    fx.mem.write_u32(g + 0x50, 0x40A0_0000);
    fx.mem.write_u32(g + 0x54, 0x4248_0000);
    halves(fx, g, &[(0x58, 0x400), (0x5A, 0x40), (0x5C, 0xE), (0x5E, 0x64)]);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = D + 16 * k as u32;
        let a = rand16(fx, D, v.r_sprite[0]);
        if v.sprite_drop {
            let h = rem(fx, D, v.r_sprite[1], 0x7D0);
            fx.mem.setf(p + 0x22C4, ee::mul(TENTH, ee::neg(ee::from_i32(h))));
        } else {
            fx.mem.write_u32(p + 0x22C4, 0);
        }
        let r = fx.mem.f(g + 0x4C);
        fan_dir(fx, p + 0x22C0, a, r);
        fx.mem.set16(p + 0x22CE, k.wrapping_neg());
        fx.mem.write_u8(p + 0x22CC, 0);
        let h = rem(fx, D, v.r_sprite[2], 0xE);
        fx.mem.set8(p + 0x22CD, h);
        k += 1;
    }
    // The puffs (+0x2730): 64 sprites, flags 0x70000003, half size 2,
    // turned (0, 0x400, 0), 8 frames every 100 updates.
    let g = PUFFS;
    let tex = fx.mem.read_u32(tex_at);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0xC, 0x4000_0000);
    fx.mem.write_u32(g + 0x20, 0);
    fx.mem.write_u32(g + 0x24, 0x400);
    fx.mem.write_u32(g + 0x28, 0);
    halves(fx, g, &[(0x30, 0), (0x32, 0x30), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0x30), (0x3C, 0x100), (0x3E, 0x80)]);
    fx.mem.write_u32(g + 0x40, D + 0x2790);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4000_0000);
    halves(fx, g, &[(0x5A, 0x40), (0x5C, 8), (0x5E, 0x64)]);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = D + 16 * k as u32;
        fx.mem.set16(p + 0x279E, 1);
        fx.mem.write_u8(p + 0x279C, 0);
        let f = rand_pow2(fx, v.r_puff[0], 8);
        fx.mem.set8(p + 0x279D, f);
        puff(fx, p, [v.r_puff[1], v.r_puff[2], v.r_puff[3]], 0xC8, 0);
        k += 1;
    }
    // The two ribbons (+0x70, +0x10C0): 256 slots, width 150 growing 10
    // an age, x scaled 1.03, cell (0x10, 0, 0x20, 0x30), fade 8, life 64.
    for g in [RIB_A, RIB_B] {
        let tex = fx.mem.read_u32(tex_at);
        fx.mem.write_u32(g, tex);
        fx.mem.write_u32(g + 8, 0x8003);
        for (o, v) in [(6u32, 0x80u8), (5, 0x80), (4, 0x80), (7, 0)] {
            fx.mem.write_u8(g + o, v);
        }
        halves(fx, g, &[(0x10, 0x10), (0x12, 0), (0x14, 0x20), (0x16, 0x30), (0x18, 0x10), (0x1A, 0), (0x1C, 0x20), (0x1E, 0x30)]);
        fx.mem.write_u32(g + 0xC, 0x4316_0000);
        fx.mem.write_u32(g + 0x34, 0x4120_0000);
        fx.mem.write_u32(g + 0x30, 0x3F83_D70A);
        halves(fx, g, &[(0x3A, 8), (0x3C, 0), (0x3E, 0x40), (0x40, 0x100), (0x42, 0), (0x44, 1)]);
    }
    0
}

/// The record's 0x1E05490(D, ot) [F] (27a64145f6ae, b169 only): the bob
/// swing, the flat sprites drawn and stepped: each live sprite's height
/// +4 -= 3, its speed x 1.05 while the height is at most 0, else x 0.8;
/// below -200 a respawn (random angle, age -k, height 0, alpha 0x40 from
/// update 43); the group's alpha fading by 16 from 381.
fn fans(fx: &mut Fx, v: &V) {
    fx.enter(0x50);
    if bit(fx.mem.read_u32(BITS), 0x1F) {
        if fstep_down(fx, D + 0x22B4, -30.0, 1.5) == 0 {
            let w = fx.mem.read_u32(BITS) & 0x7FFF_FFFF;
            fx.mem.write_u32(BITS, w);
        }
    } else if fstep_up(fx, D + 0x22B4, 30.0, 1.5) == 0 {
        let w = fx.mem.read_u32(BITS) | 0x8000_0000;
        fx.mem.write_u32(BITS, w);
    }
    flatsprite::sprites(fx, OT, SPRITES, D + 0x21F0);
    let mut k = 0;
    while k < fx.mem.i16(SPRITES + 0x5A) {
        let p = D + 16 * k as u32;
        let s0 = k as u32;
        k += 1;
        if fx.mem.i16(p + 0x22CE) < 0 {
            continue;
        }
        let d = ee::sub(fx.mem.f(p + 0x22C4), 3.0);
        fx.mem.setf(p + 0x22C4, d);
        let s = if ee_le(d, 0.0) { f32::from_bits(0x3F86_6666) } else { f32::from_bits(0x3F4C_CCCD) };
        for o in [0x22C8u32, 0x22C0] {
            let v = ee::mul(fx.mem.f(p + o), s);
            fx.mem.setf(p + o, v);
        }
        if ee_lt(fx.mem.f(p + 0x22C4), -200.0) {
            let a = rand16(fx, s0, v.r_fan);
            fx.mem.set16(p + 0x22CE, (s0 as i32).wrapping_neg());
            let r = fx.mem.f(SPRITES + 0x4C);
            fan_dir(fx, p + 0x22C0, a, r);
            fx.mem.write_u32(p + 0x22C4, 0);
            if fx.mem.i32(N) >= v.fan_alpha_from {
                fx.mem.write_u8(p + 0x22CC, 0x40);
            } else if v.fan_alpha_else {
                fx.mem.write_u8(p + 0x22CC, 0);
            }
        }
    }
    if fx.mem.i32(N) > v.end {
        fx.step_down(SPRITES + 0xB, 0, 0x10);
    }
    fx.leave(0x50);
}

/// A ribbon's new point: the target's point (0x1F65D0 kind 1) 20 up (or
/// down),
/// less the attacker's node 0x18 (`node`), over 80 into `v`; then the
/// node 4 into `p` with x = 3 sin(`angle`), pushed on `g` / `pts`, the
/// angle advanced 0x800.
fn feed(fx: &mut Fx, v: u32, up: bool, node: u32, g: u32, pts: u32, p: u32, angle: u32) {
    let t = fx.target_handle();
    fx.unit_point(1, t, v);
    let y = if up { ee::add(fx.mem.f(v + 4), 20.0) } else { ee::sub(fx.mem.f(v + 4), 20.0) };
    fx.mem.setf(v + 4, y);
    sub3(fx, v, node);
    for c in [0u32, 4, 8] {
        let q = ee::div(fx.mem.f(v + c), 80.0);
        fx.mem.setf(v + c, q);
    }
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x2DF4);
    fx.node_position(h, s, p);
    let a = fx.mem.i32(angle);
    let s = fx.table_trig(a, false);
    fx.mem.setf(p, ee::mul(3.0, s));
    driftribbon::push(fx, g, pts, p);
    let a = fx.mem.i32(angle).wrapping_add(0x800);
    fx.mem.set32(angle, a);
}

fn step(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    let node = |fx: &mut Fx, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x2DEC);
        fx.node_position(h, s, out);
    };
    node(fx, D + 0x2BB0);
    node(fx, D + 0x2210);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x60);
    node(fx, sp + 0x70);
    fx.look_angles(sp + 0x70, sp + 0x60, D + 0x2200);
    node(fx, D + 0x26E0);
    feed(fx, D + 0x90, false, sp + 0x70, RIB_A, D + 0xC0, D + 0x2130, D + 0x2DF8);
    feed(fx, D + 0x10E0, true, sp + 0x70, RIB_B, D + 0x1110, D + 0x21A0, D + 0x2DFC);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x2D70);
    // s0 = D, s1 = the OT, s2 = sp + 0x70, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(sp + 0x70), Some(0)];
    fx.xform_world_saving(D, v.r_xform[0], regs);
    fx.xform_billboard_saving(D + 0x2110, v.r_xform[1], regs);
    fx.xform_billboard_saving(D + 0x2180, v.r_xform[2], regs);
    fx.xform_billboard_saving(D + 0x2B90, v.r_xform[3], regs);
    fx.xform_world_saving(D + 0x21F0, v.r_xform[4], regs);
    fx.xform_world_saving(D + 0x26C0, v.r_xform[5], regs);
    fx.xform_world_saving(D + 0x2D50, v.r_xform[6], regs);
    if fx.mem.i32(N) >= v.end {
        fx.step_up(LIGHT, 0x80, 4);
    } else {
        fx.step_down(LIGHT, 0x20, 4);
    }
    let l = fx.mem.u8(LIGHT);
    fx.set_stage_light(l);
    for (o, ra) in [(0xAu32, v.r_grey[0]), (9, v.r_grey[1]), (8, v.r_grey[2])] {
        let v = (rand_pow2(fx, ra, 0x80) + 0x7F) as u8;
        fx.mem.write_u8(SPRITES + o, v);
        for g in [RIB_B, RIB_A] {
            fx.mem.write_u8(g + o - 4, v);
        }
    }
    let n = fx.mem.i32(N);
    if v.cloud && (0x15..0x32).contains(&n) {
        for k in [0x88u32, 0x84, 0x80] {
            fx.mem.write_u32(sp + k, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0x2B90, CLOUD, sp + 0x80);
        if fx.mem.i32(N) >= 0x22 {
            fx.step_down(CLOUD + 0xB, 0, 2);
        }
    }
    if fx.mem.i32(N) >= v.ribbons_from {
        for (g, pts) in [(RIB_A, D + 0xC0), (RIB_B, D + 0x1110)] {
            driftribbon::draw(fx, OT, g, pts, D);
            if fx.mem.i32(N) >= v.end {
                fx.step_down(g + 7, 0, 6);
            } else {
                fx.step_up(g + 7, 0x60, 6);
            }
        }
    }
    fans(fx, v);
    if fx.mem.i32(N) >= v.puffs_from {
        fx.sprites(PUFFS, D + 0x26C0);
        if fx.mem.i32(N) >= v.end {
            fx.step_down(PUFFS + 0xB, 0, 8);
        }
        puffs(fx, v);
    }
    if fx.mem.i32(N) >= v.image_from {
        let a = 0x80 - i32::from(fx.mem.read_u8(IMAGE + 7));
        fx.mem.set8(IMAGE + 0x1B, a);
        afterimage::strips(fx, IMAGE, D + 0x2D70);
        let h = fx.mem.i16(IMAGE + 0xA);
        unit_tint(fx, h, IMAGE + 0x18);
        if fx.mem.i32(N) > v.end {
            fx.step_down(IMAGE + 7, 0, 8);
        }
    }
    let n = fx.mem.i32(N) + 1;
    fx.mem.set32(N, n);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if n == v.end {
        let t = fx.target_handle();
        fx.unit_add_colour(t, 0);
        2
    } else if n == v.one {
        1
    } else if (v.image_from..v.end).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}

/// The puffs stepped: each live puff rises 1 (alpha up by 8 to 0x80 while
/// at most 10 high, then down), its speed x 1.05 under 20; at 20 a
/// respawn.
fn puffs(fx: &mut Fx, v: &V) {
    let mut k = 0;
    while k < fx.mem.i16(PUFFS + 0x5A) {
        let p = D + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x279E) < 0 {
            continue;
        }
        let h = ee::add(fx.mem.f(p + 0x2794), 1.0);
        fx.mem.setf(p + 0x2794, h);
        if fx.mem.read_u8(p + 0x279C) != 0 {
            if ee_le(fx.mem.f(p + 0x2794), 10.0) {
                fx.step_up(p + 0x279C, 0x80, 8);
            } else {
                fx.step_down(p + 0x279C, 0, 8);
            }
        }
        if ee_lt(fx.mem.f(p + 0x2794), 20.0) {
            let s = f32::from_bits(0x3F86_6666);
            for o in [0x2798u32, 0x2790] {
                let v = ee::mul(fx.mem.f(p + o), s);
                fx.mem.setf(p + o, v);
            }
        }
        if ee_lt(fx.mem.f(p + 0x2794), 20.0) {
            continue;
        }
        puff(fx, p, [v.r_respawn[0], v.r_respawn[1], v.r_respawn[2]], 0x3C, 0x1E);
        fx.mem.write_u8(p + 0x279C, 1);
        let f = rand_pow2(fx, v.r_respawn[3], 8);
        fx.mem.set8(p + 0x279D, f);
    }
}
