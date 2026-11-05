//! BMODULE b039 (card 613), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04200, data area 0x1ED0000) [F] (helper D11, 31
//! Oct 2026): b169's program without the afterimage: two drifting ribbons
//! (`blib::driftribbon`, +0x70 / +0x10C0, cell (0x10, 0, 0x20, 0x30) of a
//! 0x100 x 0x80 sheet) fed every update from the attacker's node 0x1C at x
//! = 3 sin of an angle advancing 0x800 (+0x3970 / +0x3974), drawn every
//! update (alpha up by 6 to 0x60, down from 320); 64 flat sprites
//! (+0x2260, `flatsprite::sprites`) falling 3 an update round node 0x18
//! with the bob amplitude +0x22B4 swinging -30..30 (bit 0x1D of +0x3960;
//! the record's 0x1E04FB0 `fans`); 256 puffs (+0x2730, `Fx::sprites`,
//! texture 1, half size 20) from update 88 rising from -20, x 1.05 under
//! 20, respawned at 20; a particle cloud (+0x3800) on 21..49; from 40 a
//! full-window tint (+0x3950: 0xE0, 0x70, 0xB0; `b262::tint`) whose alpha
//! swings 0x20..0x40 by 4 (bit 0x1F of +0x3960), 0x80 on 320 then fading;
//! the stage light down to 0x20, up again from 320; three random greys
//! 0x7F.. on the groups each update. Returns 1 on update 78, 6 on 88..319,
//! 2 on 320, 8 after 340.
//!
//! Library (record addresses): `colours4` 0x1E00080, `b262::tint`
//! 0x1E00120 (65049dfc9548), `Fx::sprites` 0x1E00280, `flatsprite::sprites`
//! 0x1E01B70, `particle_quads` 0x1E02B20, `driftribbon::push` 0x1E036B0,
//! `driftribbon::draw` 0x1E03820; the executable's 0x1F6A60
//! [`crate::b107::target_radius`], 0x103660 [`crate::b079::sub3`].
use crate::b079::sub3;
use crate::b107::target_radius;
use crate::b142::fstep_up;
use crate::b262::tint;
use crate::b436::fstep_down;
use crate::blib::{driftribbon, flatsprite};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const N: u32 = D + 0x3958;
const LEFT: u32 = D + 0x3954;
const BITS: u32 = D + 0x3960;
const LIGHT: u32 = D + 0x397C;
const TINT: u32 = D + 0x3950;
const RIB_A: u32 = D + 0x70;
const RIB_B: u32 = D + 0x10C0;
const SPRITES: u32 = D + 0x2260;
const PUFFS: u32 = D + 0x2730;
const CLOUD: u32 = D + 0x3800;
const END: i32 = 0x140;
const TENTH: f32 = f32::from_bits(0x3DCC_CCCD);

/// The entry 0x1E04200.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xA0);
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

/// A puff's speed: 0.1 (rand % 60 - 30) along a random angle (rand's
/// frames from the return addresses `ras`, s0 = D).
fn puff_dir(fx: &mut Fx, p: u32, ras: [u32; 2]) {
    let a = rand16(fx, D, ras[0]);
    let h = rem(fx, D, ras[1], 0x3C) - 0x1E;
    let r = ee::mul(TENTH, ee::from_i32(h));
    fan_dir(fx, p + 0x2790, a, r);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3980);
    for x in [0u32, 0x2110, 0x2180, 0x21F0, 0x26C0, 0x3790] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x3964u32, 0x18, 1), (0x3968, 0x1A, 0), (0x396C, 0x1C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(LEFT, 0x154);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x395C, param);
    let r = target_radius(fx);
    fx.mem.setf(D + 0x3978, r);
    fx.mem.write_u8(LIGHT, 0x80);
    fx.mem.write_u32(BITS, 0);
    fx.mem.write_u32(D + 0x3970, 0x4000);
    for (o, v) in [(2u32, 0xB0u8), (1, 0x70), (0, 0xE0), (3, 0)] {
        fx.mem.write_u8(TINT + o, v);
    }
    fx.texture5(sp + 0x90, 0);
    // The two ribbons (+0x70, +0x10C0): 256 slots, width 150 growing 10
    // an age, x scaled 1.03, cell (0x10, 0, 0x20, 0x30), fade 8, life 64.
    for g in [RIB_A, RIB_B] {
        let tex = fx.mem.read_u32(sp + 0x90);
        fx.mem.write_u32(g, tex);
        fx.mem.write_u32(g + 8, 0x8003);
        for (o, v) in [(6u32, 0x80u8), (5, 0x80), (4, 0x80), (7, 0)] {
            fx.mem.write_u8(g + o, v);
        }
        halves(fx, g, &[(0x10, 0x10), (0x12, 0), (0x14, 0x20), (0x16, 0x30), (0x18, 0x10), (0x1A, 0), (0x1C, 0x100), (0x1E, 0x80)]);
        fx.mem.write_u32(g + 0xC, 0x4316_0000);
        fx.mem.write_u32(g + 0x34, 0x4120_0000);
        fx.mem.write_u32(g + 0x30, 0x3F83_D70A);
        halves(fx, g, &[(0x3A, 8), (0x3C, 0), (0x3E, 0x40), (0x40, 0x100), (0x42, 0), (0x44, 1)]);
    }
    // The flat sprites (+0x2260): 64 of the attacker's texture 0, flags
    // 0x58000003, half size 1.5 growing 5 an age, bobbing 50 at 0x400,
    // cell (0, 0x30, 0x20, 0x20) of a 0x100 x 0x80 sheet, 14 frames every
    // 100 updates.
    let g = SPRITES;
    let tex = fx.mem.read_u32(sp + 0x90);
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
        let a = rand16(fx, D, 0x01E0_44B8);
        fx.mem.write_u32(p + 0x22C4, 0);
        let r = fx.mem.f(g + 0x4C);
        fan_dir(fx, p + 0x22C0, a, r);
        fx.mem.set16(p + 0x22CE, k.wrapping_neg());
        fx.mem.write_u8(p + 0x22CC, 0);
        let h = rem(fx, D, 0x01E0_4508, 0xE);
        fx.mem.set8(p + 0x22CD, h);
        k += 1;
    }
    // The puffs (+0x2730): 256 sprites of the attacker's texture 1 (0x20,
    // 0x10, 0x80, alpha 0x40), flags 0x70000003, half size 20, turned (0,
    // 0x400, 0), cell (0, 0, 0x18, 0x20) of 0x80 x 0x40, 10 frames an
    // update.
    fx.texture4(sp + 0x90, 1);
    let g = PUFFS;
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0003);
    for (k, v) in [(0xAu32, 0x20u8), (9, 0x10), (8, 0x80), (0xB, 0x40)] {
        fx.mem.write_u8(g + k, v);
    }
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    fx.mem.write_u32(g + 0x20, 0);
    fx.mem.write_u32(g + 0x24, 0x400);
    fx.mem.write_u32(g + 0x28, 0);
    halves(fx, g, &[(0x30, 0), (0x32, 0), (0x34, 0x18), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x2790);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4000_0000);
    halves(fx, g, &[(0x5A, 0x100), (0x5C, 0xA), (0x5E, 1)]);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = D + 16 * k as u32;
        fx.mem.set16(p + 0x279E, 1);
        fx.mem.write_u8(p + 0x279C, 0);
        let f = rem(fx, D, 0x01E0_4600, 0xA);
        fx.mem.set8(p + 0x279D, f);
        puff_dir(fx, p, [0x01E0_461C, 0x01E0_4638]);
        let h = rem(fx, D, 0x01E0_4688, 0x190) - 0xC8;
        fx.mem.setf(p + 0x2794, ee::mul(TENTH, ee::from_i32(h)));
        k += 1;
    }
    // The cloud (+0x3800): 16 quads of size 5, TEXEFF 15.
    fx.texture4(sp + 0x90, 0xF);
    let g = CLOUD;
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x0008_0083);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0x80), (0x2A, 0xC0), (0x24, 0x20), (0x26, 0x20), (0x2C, 0x80), (0x2E, 0x40)]);
    fx.mem.write_u32(g + 0xC, 0x40A0_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 2), (0x13C, 1), (0x140, 8), (0x136, 0), (0x138, 0)]);
    for (k, v) in [(0xAu32, 0x80u8), (9, 0x70), (8, 0x70), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, v);
    }
    for k in 0..0x10i32 {
        let p = g + 16 * k as u32;
        for (c, ra) in [(0u32, 0x01E0_477Cu32), (1, 0x01E0_47B0)] {
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
    for c in [8u32, 4, 0] {
        fx.mem.write_u32(D + 0x26C0 + c, 0x4000_0000);
    }
    0
}

/// The record's 0x1E04FB0(D, ot) [F] (6360e0e35ed2, b039 only): the bob
/// swing (bit 0x1D), the flat sprites drawn and stepped: each live
/// sprite's height +4 -= 3, its speed x 1.05 while the height is at most
/// 0, else x 0.8; below -200 a respawn (random angle, age -k, height 0,
/// alpha 0x40 from update 43, else 0); the group's alpha fading by 16
/// from 321.
fn fans(fx: &mut Fx) {
    fx.enter(0x50);
    if bit(fx.mem.read_u32(BITS), 0x1D) {
        if fstep_down(fx, D + 0x22B4, -30.0, 1.5) == 0 {
            let w = fx.mem.read_u32(BITS) & 0xDFFF_FFFF;
            fx.mem.write_u32(BITS, w);
        }
    } else if fstep_up(fx, D + 0x22B4, 30.0, 1.5) == 0 {
        let w = fx.mem.read_u32(BITS) | 0x2000_0000;
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
            let a = rand16(fx, s0, 0x01E0_5118);
            fx.mem.set16(p + 0x22CE, (s0 as i32).wrapping_neg());
            let r = fx.mem.f(SPRITES + 0x4C);
            fan_dir(fx, p + 0x22C0, a, r);
            fx.mem.write_u32(p + 0x22C4, 0);
            let a = if fx.mem.i32(N) >= 0x2B { 0x40 } else { 0 };
            fx.mem.write_u8(p + 0x22CC, a);
        }
    }
    if fx.mem.i32(N) > END {
        fx.step_down(SPRITES + 0xB, 0, 0x10);
    }
    fx.leave(0x50);
}

/// A ribbon's new point: the target's point (0x1F65D0 kind 1) 20 up (or
/// down), less the attacker's node 0x18 (`node`), over 80 into `v`; then
/// the node 0x1C into `p` with x = 3 sin(`angle`), pushed on `g` / `pts`,
/// the angle advanced 0x800.
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
    let s = fx.mem.i32(D + 0x396C);
    fx.node_position(h, s, p);
    let a = fx.mem.i32(angle);
    let s = fx.table_trig(a, false);
    fx.mem.setf(p, ee::mul(3.0, s));
    driftribbon::push(fx, g, pts, p);
    let a = fx.mem.i32(angle).wrapping_add(0x800);
    fx.mem.set32(angle, a);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let node = |fx: &mut Fx, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x3964);
        fx.node_position(h, s, out);
    };
    node(fx, D + 0x37B0);
    node(fx, D + 0x2210);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x60);
    node(fx, sp + 0x70);
    fx.look_angles(sp + 0x70, sp + 0x60, D + 0x2200);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x26E0);
    feed(fx, D + 0x90, false, sp + 0x70, RIB_A, D + 0xC0, D + 0x2130, D + 0x3970);
    feed(fx, D + 0x10E0, true, sp + 0x70, RIB_B, D + 0x1110, D + 0x21A0, D + 0x3974);
    // s0 = D, s1 = the OT, s2 = sp + 0x70, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(sp + 0x70), Some(0)];
    fx.xform_world_saving(D, 0x01E0_4A2C, regs);
    fx.xform_billboard_saving(D + 0x2110, 0x01E0_4A34, regs);
    fx.xform_billboard_saving(D + 0x2180, 0x01E0_4A3C, regs);
    fx.xform_billboard_saving(D + 0x3790, 0x01E0_4A44, regs);
    fx.xform_world_saving(D + 0x21F0, 0x01E0_4A4C, regs);
    fx.xform_world_saving(D + 0x26C0, 0x01E0_4A54, regs);
    if fx.mem.i32(N) >= END {
        fx.step_up(LIGHT, 0x80, 4);
    } else {
        fx.step_down(LIGHT, 0x20, 4);
    }
    let l = fx.mem.u8(LIGHT);
    fx.set_stage_light(l);
    if fx.mem.i32(N) >= 0x28 {
        if fx.mem.i32(N) == END {
            fx.mem.write_u8(TINT + 3, 0x80);
        }
        tint(fx, TINT);
        if fx.mem.i32(N) >= END {
            fx.step_down(TINT + 3, 0, 8);
        } else if bit(fx.mem.read_u32(BITS), 0x1F) {
            if fx.step_down(TINT + 3, 0x20, 4) == 0 {
                let w = fx.mem.read_u32(BITS) & 0x7FFF_FFFF;
                fx.mem.write_u32(BITS, w);
            }
        } else if fx.step_up(TINT + 3, 0x40, 4) == 0 {
            let w = fx.mem.read_u32(BITS) | 0x8000_0000;
            fx.mem.write_u32(BITS, w);
        }
    }
    for (o, ra) in [(0xAu32, 0x01E0_4B60u32), (9, 0x01E0_4B88), (8, 0x01E0_4BB0)] {
        let v = (fx.rand_from(D, ra).wrapping_rem(0x80) + 0x7F) as u8;
        fx.mem.write_u8(SPRITES + o, v);
        for g in [RIB_B, RIB_A] {
            fx.mem.write_u8(g + o - 4, v);
        }
    }
    let n = fx.mem.i32(N);
    if (0x15..0x32).contains(&n) {
        for k in [0x88u32, 0x84, 0x80] {
            fx.mem.write_u32(sp + k, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0x3790, CLOUD, sp + 0x80);
        if fx.mem.i32(N) >= 0x22 {
            fx.step_down(CLOUD + 0xB, 0, 2);
        }
    }
    for (g, pts) in [(RIB_A, D + 0xC0), (RIB_B, D + 0x1110)] {
        driftribbon::draw(fx, OT, g, pts, D);
        if fx.mem.i32(N) >= END {
            fx.step_down(g + 7, 0, 6);
        } else {
            fx.step_up(g + 7, 0x60, 6);
        }
    }
    fans(fx);
    if fx.mem.i32(N) >= 0x58 {
        fx.sprites(PUFFS, D + 0x26C0);
        if fx.mem.i32(N) >= END {
            fx.step_down(PUFFS + 0xB, 0, 4);
        }
        puffs(fx);
    }
    let n = fx.mem.i32(N) + 1;
    fx.mem.set32(N, n);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if n == END {
        let t = fx.target_handle();
        fx.unit_add_colour(t, 0);
        2
    } else if n == 0x4E {
        1
    } else if (0x58..END).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}

/// The puffs stepped: each live puff rises 1 (alpha up by 4 to 0x40 while
/// at most 0 high, then down), its speed x 1.05 under 20; at 20 a respawn
/// from -20 + 0.1 (rand % 60 - 30).
fn puffs(fx: &mut Fx) {
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
            if ee_le(fx.mem.f(p + 0x2794), 0.0) {
                fx.step_up(p + 0x279C, 0x40, 4);
            } else {
                fx.step_down(p + 0x279C, 0, 4);
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
        puff_dir(fx, p, [0x01E0_4E10, 0x01E0_4E2C]);
        let h = rem(fx, D, 0x01E0_4E7C, 0x3C) - 0x1E;
        fx.mem.setf(p + 0x2794, ee::add(-20.0, ee::mul(TENTH, ee::from_i32(h))));
        fx.mem.write_u8(p + 0x279C, 1);
        let f = rem(fx, D, 0x01E0_4EC4, 0xA);
        fx.mem.set8(p + 0x279D, f);
    }
}
