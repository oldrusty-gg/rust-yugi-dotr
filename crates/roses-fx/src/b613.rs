//! BMODULE b613 (card 425), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04010, data area 0x1ED0000, 0xA1A0 bytes) [F]
//!. Seven rings of the quad trail (+0x270) pushed
//! at the attacker's nodes 0x20 (updates 73, 91, 115) and 0x11 (64, 82,
//! 100, 116) and drawn until 202; the stage light dims to 0x20 until 394.
//! From 203 to 359 at the target eight zaps (+0x70.., `blib::zap`) run
//! from node 0x26 to points round it and a spark burst (+0x9940) is thrown
//! out again on a flat ring; from 210 a sprite cloud (+0x9670) spreads
//! from it, re-thrown as it fades; from 242 2048 spinning hoops (+0x1530,
//! `blib::hoop`) round the target. The target is frozen (0x1F84D0, a
//! runner boundary) from 300 to 393. Returns 1 on update 200, 6 on 210..393, 2 on 394, 8 after
//! 410.
//!
//! Library: `colours4` (0x1E00080), `sprites` (0x1E00120),
//! `spawn::scatter` (0x1E01A10), `trail::sparks` (0x1E01BD0), `zap::zap`
//! (0x1E02930), `hoop::setup` / `draw` (0x1E02E50 / 0x1E02FE0),
//! `quadtrail::push` / `draw` (0x1E03400 / 0x1E03600).
use crate::b628::{halves, trail_fade, trail_group};
use crate::blib::{self, hoop, quadtrail, zap};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const HOOP_SETUP: u32 = 0x01E0_2E50;
const HOOP_DRAW: u32 = 0x01E0_2FE0;
const TRAIL_DRAW: u32 = 0x01E0_3600;

/// The entry 0x1E04010.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xF0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xF0);
    ret
}

/// What the runner does at the boundary 0x1F84D0(handle, v) (-> 0x1F4500
/// [F]): bit 0 of the unit object's +0x20D = `v` (the motion step and cue
/// player skip a frozen unit).
pub(crate) fn unit_freeze(fx: &mut Fx, handle: i32, v: i32) {
    let (side, slot) = (handle >> 8, handle & 0xFF);
    if !(0..3).contains(&side) || slot >= 0xB {
        return;
    }
    let i = fx.object_index(side, slot);
    if !(0..0x54).contains(&i) {
        return;
    }
    let o = fx.mem.read_u32(crate::world::OBJECT_TABLE + 4 * i as u32);
    if o == 0 {
        return;
    }
    let f = fx.mem.read_u8(o + 0x20D);
    fx.mem.write_u8(o + 0x20D, (f & !1) | (v & 1) as u8);
}

fn colour(fx: &mut Fx, g: u32, bgra: [u8; 4]) {
    for (k, x) in [(0xAu32, bgra[2]), (9, bgra[1]), (8, bgra[0]), (0xB, bgra[3])] {
        fx.mem.write_u8(g + k, x);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xA1A0);
    for x in [0u32, 0x14C0, 0x98D0] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0xA180u32, 0x47, 1), (0xA184, 0x26, 0), (0xA188, 0x11, 0), (0xA18C, 0x20, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0xA170, 0x19A);
    fx.mem.set32(D + 0xA174, 1);
    fx.mem.set32(D + 0xA178, param);
    fx.mem.set32(D + 0xA17C, 0);
    fx.mem.write_u8(D + 0xA190, 0x80);
    fx.texture4(sp + 0xE0, 0xF);
    let tex = |fx: &mut Fx| fx.mem.read_u32(sp + 0xE0);
    let t = tex(fx);
    trail_group(fx, D + 0x270, t);
    fx.mem.set16(D + 0x2B6, 0x24);
    fx.texture5(sp + 0xE0, 0);
    // The sprite cloud (+0x9670): 32 sprites at +0x96D0, size 20.
    let g = D + 0x9670;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x4800_1003);
    colour(fx, g, [0x80, 0x80, 0x80, 0]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    for o in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + o, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x96D0);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4170_0000);
    fx.mem.write_u32(g + 0x50, 0x4120_0000);
    halves(fx, g, &[(0x5A, 0x20), (0x5C, 8), (0x5E, 0x10)]);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = D + 0x96D0 + 16 * k as u32;
        fx.random_dir(sp + 0xA0, 14.0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
            fx.mem.write_u32(p + 4 * c, w);
        }
        let r = fx.rand_from(D, 0x01E0_43D0);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, h.wrapping_neg());
        fx.mem.write_u8(p + 0xC, 0x80);
        let r = fx.rand_from(D, 0x01E0_4400);
        fx.mem.write_u8(p + 0xD, (r % 8) as u8);
        k += 1;
    }
    // The spark burst (+0x9940): 128 sparks at +0x9970, width 0.5,
    // length 5.
    let g = D + 0x9940;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x2000_0003);
    colour(fx, g, [0x80, 0x80, 0x80, 0xFF]);
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x40A0_0000);
    halves(fx, g, &[(0x10, 0x20), (0x12, 0x20), (0x14, 0x20), (0x16, 0x20), (0x18, 0x20), (0x1A, 0x20), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, D + 0x9970);
    fx.mem.write_u32(g + 0x28, 0x40A0_0000);
    fx.mem.set16(g + 0x2C, 0x80);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 4.0);
    // Eight zaps (+0x70 + 0x40 k): 28 points, width 5, cell (0x5A, 0, 6,
    // 32), starting k updates late.
    for k in 0..8i32 {
        let g = D + 0x70 + 0x40 * k as u32;
        let t = tex(fx);
        fx.mem.write_u32(g, t);
        fx.mem.write_u32(g + 4, 0);
        fx.mem.write_u32(g + 0xC, 0x40A0_0000);
        halves(fx, g, &[(0x10, 0x5A), (0x12, 0), (0x14, 6), (0x16, 0x20)]);
        colour(fx, g, [0x80; 4]);
        halves(fx, g, &[(0x28, 0x1C), (0x2C, -k), (0x2A, 0x28)]);
        fx.mem.write_u32(g + 0x20, 0);
        fx.mem.write_u32(g + 0x24, 0);
    }
    // The hoops (+0x1530): 2048 at +0x1670, 4 points at +0x1570, radius
    // 18, delays up to 160, growing 3 a step.
    let g = D + 0x1530;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x400_0000);
    colour(fx, g, [0x80, 0x80, 0x80, 0x10]);
    fx.mem.write_u32(g + 0xC, 0x4190_0000);
    halves(fx, g, &[(0x10, 0x5A), (0x12, 0), (0x14, 6), (0x16, 0x20)]);
    fx.mem.write_u32(g + 0x20, D + 0x1670);
    fx.mem.write_u32(g + 0x24, D + 0x1570);
    halves(fx, g, &[(0x28, 4), (0x2A, 0x800), (0x2C, 0x28), (0x2E, 0), (0x30, 0xA0), (0x32, 3)]);
    hoop::setup(fx, HOOP_SETUP, g);
    fx.mem.write_u32(D + 0x14C4, 0x3FC0_0000);
    0
}

fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0xA174);
    node(fx, 0xA180, sp + 0xA0);
    node(fx, 0xA184, sp + 0xB0);
    let t = fx.target_handle();
    fx.unit_point(1, t, sp + 0xD0);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x14E0);
    fx.look_angles(sp + 0xA0, sp + 0xB0, D + 0x98E0);
    for c in 0..4u32 {
        let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
        fx.mem.write_u32(D + 0x98F0 + 4 * c, w);
    }
    fx.mem.write_u32(D + 0x14D8, 0x1000);
    if [0x49, 0x5B, 0x73].contains(&n(fx)) {
        node(fx, 0xA18C, sp + 0x90);
        quadtrail::push(fx, D + 0x270, sp + 0x90);
    }
    if [0x40, 0x52, 0x64, 0x74].contains(&n(fx)) {
        node(fx, 0xA188, sp + 0x90);
        quadtrail::push(fx, D + 0x270, sp + 0x90);
    }
    // s0 = D, s1 = the OT, s2 the param (-1, 64 bits), s3 the caller's;
    // the last TRS matrix is under the zaps' first XYZ top byte [run].
    for (x, ra) in [(0u32, 0x01E0_4818u32), (0x14C0, 0x01E0_4820), (0x98D0, 0x01E0_482C)] {
        fx.xform_world_saving(D + x, ra, [Some(D), Some(OT), None, None]);
        fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
    }
    // The light's old byte stays in a3 (0x103470 / 0x1032C0 load it).
    let old = u32::from(fx.mem.read_u8(D + 0xA190));
    if n(fx) < 0x18A {
        fx.step_down(D + 0xA190, 0x20, 4);
    } else {
        fx.step_up(D + 0xA190, 0x80, 4);
    }
    let l = i32::from(fx.mem.read_u8(D + 0xA190));
    fx.set_stage_light(l);
    // 0x1F84D0(target, 1 / 0): the target's freeze flag, a runner
    // boundary (a2 / a3 the light step's 4 and old byte).
    for (at, v) in [(0x12C, 1u32), (0x189, 0)] {
        if n(fx) == at {
            let t = fx.target_handle();
            fx.boundary(0x001F_84D0, "target flag 0x1F84D0", [t as u32, v, 4, old]);
            unit_freeze(fx, t, v as i32);
        }
    }
    if n(fx) < 0xCB {
        quadtrail::draw(fx, TRAIL_DRAW, OT, D + 0x270, D);
        trail_fade(fx, D + 0x270);
    }
    if n(fx) >= 0xD2 {
        fx.sprites(D + 0x9670, D + 0x14C0);
        if n(fx) >= 0x18A {
            fx.step_down(D + 0x967B, 0, 8);
        } else {
            fx.step_up(D + 0x967B, 0x80, 2);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x96CA) {
            let p = D + 0x96D0 + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) >= 0 {
                let s = f32::from_bits(0x3F83_D70A);
                for c in 0..3u32 {
                    let x = ee::mul(fx.mem.f(p + 4 * c), s);
                    fx.mem.setf(p + 4 * c, x);
                }
                fx.step_down(p + 0xC, 0, 8);
            }
            if fx.mem.i16(p + 0xE) < 0x10 {
                continue;
            }
            fx.random_dir(sp + 0xA0, 14.0);
            for c in 0..3u32 {
                let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
                fx.mem.write_u32(p + 4 * c, w);
            }
            fx.mem.set16(p + 0xE, 0);
            fx.mem.write_u8(p + 0xC, 0x80);
        }
    }
    if (0xCB..0x168).contains(&n(fx)) {
        for k in 0..8u32 {
            for c in 0..4u32 {
                let w = fx.mem.read_u32(sp + 0xD0 + 4 * c);
                fx.mem.write_u32(sp + 0xC0 + 4 * c, w);
            }
            let a = (0x2000 * k) as i32;
            let c = fx.tcos(a);
            let v = ee::add(fx.mem.f(sp + 0xC0), ee::mul(20.0, c));
            fx.mem.setf(sp + 0xC0, v);
            let s = fx.tsin(a);
            let v = ee::add(fx.mem.f(sp + 0xC4), ee::mul(20.0, s));
            fx.mem.setf(sp + 0xC4, v);
            zap::zap(fx, OT, D + 0x70 + 0x40 * k, D, sp + 0xB0, sp + 0xC0);
        }
    }
    if (0xCB..0x168).contains(&n(fx)) {
        blib::trail::sparks(fx, OT, D + 0x9940, D + 0x98D0);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x996C) {
            let p = D + 0x9970 + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) < 0 {
                continue;
            }
            let s = f32::from_bits(0x3FA6_6666);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 4 * c), s);
                fx.mem.setf(p + 4 * c, x);
            }
            if ee_le(fx.mem.f(p + 4), 50.0) {
                continue;
            }
            let m = ee::fixsfsi(ee::mul(10.0, fx.mem.f(D + 0x9968)));
            let a = fx.rand_from(D, 0x01E0_4CC4) & 0xFFFF;
            let r = fx.rand_from(D, 0x01E0_4CE0);
            let (_, h) = fx.div(r, m);
            fx.mem.write_u32(p + 4, 0x4080_0000);
            let rad = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
            let c = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(rad, c));
            let s = fx.tsin(a);
            fx.mem.setf(p, ee::mul(rad, s));
            let r = fx.rand_from(D, 0x01E0_4D48);
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0xE, h.wrapping_neg());
        }
    }
    if n(fx) >= 0xF2 {
        hoop::draw(fx, HOOP_DRAW, OT, D + 0x1530, D + 0x14C0);
        if n(fx) >= 0x17A {
            fx.step_down(D + 0x153B, 0, 1);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0xA174, m);
    let left = fx.mem.i32(D + 0xA170) - 1;
    fx.mem.set32(D + 0xA170, left);
    if m == 0x18A {
        2
    } else if m == 0xC8 {
        1
    } else if (0xD2..0x18A).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
