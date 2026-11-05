//! BMODULE b274 (card 511), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02B30, data area a0 = 0x1ED0000) [F] (helper C9,
//! 31 Oct 2026): b419's skeleton. Two points (+0x410 / +0x510) sit at the
//! attacker's nodes 9 / 0xA until update 0x32, then close on the target
//! (+0x610 / +0x680, by 1 / 18 a step) trailing a streamer each (+0x70 /
//! +0x230, points +0xB0 / +0x270); discs (+0x460 / +0x560) and facing quads
//! (+0x470 / +0x570) at the points (billboards +0x3F0 / +0x4F0, growing to
//! 1 by 0.05 before 0x32), two spark clouds (+0x740 / +0xFE0) on
//! 0x32..0x53 and a third (+0x1880, transform +0x1810, lit on 0x54) from
//! 0x32; a frame quad on 0x32..0x9D and from 0x44 a full-window tint
//! (+0x20CC). Returns 1 on update 0x3A, 6 on 0x44..0x53, 2 on 0x54, 8
//! after 0x64.
//!
//! Library (record addresses) as b419's: `frame_quad_rgba` 0x1E00120,
//! `tint` 0x1E00240, `disc` 0x1E003A0, `facing::quad` 0x1E00900, `scatter`
//! 0x1E01190, `trail::sparks` 0x1E01350, `streamer::push` / `draw`
//! 0x1E020B0 / 0x1E02190.
use crate::b262::tint;
use crate::blib::{facing, spawn, streamer, trail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const STREAMER: u32 = 0x01E0_2190;
const N: u32 = D + 0x20B4;
const LEFT: u32 = D + 0x20B0;

/// The entry 0x1E02B30 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn bytes(fx: &mut Fx, at: &[(u32, u8)]) {
    for &(o, v) in at {
        fx.mem.write_u8(D + o, v);
    }
}

fn halves(fx: &mut Fx, at: &[(u32, i32)]) {
    for &(o, v) in at {
        fx.mem.set16(D + o, v);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x20D0);
    let tenth = 0x3DCC_CCCDu32;
    for x in [0u32, 0x3F0, 0x4F0, 0x5F0, 0x660, 0x6D0, 0xF70, 0x1810] {
        fx.xform_init(D + x);
        if x == 0x3F0 || x == 0x4F0 {
            for o in [8u32, 4, 0] {
                fx.mem.write_u32(D + x + o, tenth);
            }
        }
    }
    for (at, n, clear) in [(0x20C4u32, 9, 1), (0x20C8, 0xA, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(LEFT, 0x64);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x20B8, param);
    fx.mem.set32(D + 0x20BC, 0x12);
    fx.mem.set32(D + 0x20C0, 0x12);
    bytes(fx, &[(0x20CE, 0x80), (0x20CD, 0xA0), (0x20CC, 0xC0), (0x20CF, 0)]);
    // Discs (+0x460, +0x560): radius 4.
    for g in [0x460u32, 0x560] {
        fx.mem.write_u32(D + g, 0);
        bytes(fx, &[(g + 6, 0x60), (g + 5, 0x80), (g + 4, 0xA0), (g + 7, 0)]);
        fx.mem.write_u32(D + g + 8, 0x4080_0000);
        halves(fx, &[(g + 0xC, 0x24), (g + 0xE, 0)]);
    }
    // Facing quads (+0x470 / +0x570, quads +0x4D0 / +0x5D0), texture 5 / 2.
    fx.texture5(sp + 0xB0, 2);
    for (g, q) in [(0x470u32, 0x4D0u32), (0x570, 0x5D0)] {
        fx.mem.write_u32(D + g + 0x38, 3);
        halves(fx, &[(g, 0), (g + 2, 0), (g + 4, 0x40), (g + 6, 0x40), (g + 8, 0), (g + 0xA, 0), (g + 0xC, 0x80), (g + 0xE, 0x80)]);
        fx.mem.write_u32(D + g + 0x30, 0x3F80_0000);
        bytes(fx, &[(g + 0x36, 0x80), (g + 0x35, 0x80), (g + 0x34, 0x80), (g + 0x37, 0)]);
        halves(fx, &[(g + 0x48, 0), (g + 0x4A, 2), (g + 0x4C, 3), (g + 0x4E, 4), (g + 0x50, 0), (g + 0x52, 0x3E7)]);
        let tex = fx.mem.read_u32(sp + 0xB0);
        for o in [0x18u32, 0x14, 0x10] {
            fx.mem.write_u32(D + q + o, 0);
        }
        fx.mem.write_u32(D + q + 8, 0x4040_0000);
        fx.mem.write_u32(D + q, tex);
        bytes(fx, &[(q + 0xE, 0x80), (q + 0xD, 0x80), (q + 0xC, 0x80), (q + 0xF, 0x80)]);
    }
    // Streamers (+0x70 / +0x230), texture 5 / 0.
    fx.texture5(sp + 0xB0, 0);
    for g in [0x70u32, 0x230] {
        let tex = fx.mem.read_u32(sp + 0xB0);
        fx.mem.write_u32(D + g, tex);
        bytes(fx, &[(g + 6, 0x80), (g + 5, 0x80), (g + 4, 0x80), (g + 7, 0x30)]);
        fx.mem.write_u32(D + g + 8, 0x8003);
        fx.mem.write_u32(D + g + 0xC, 0x41A0_0000);
        halves(fx, &[(g + 0x10, 0x9C), (g + 0x12, 0), (g + 0x14, 0x40), (g + 0x16, 0x40), (g + 0x18, 0x9C), (g + 0x1A, 0), (g + 0x1C, 0x100), (g + 0x1E, 0x80)]);
        fx.mem.write_u32(D + g + 0x30, 0x3D4C_CCCD);
        halves(fx, &[(g + 0x34, 0), (g + 0x36, 0x10), (g + 0x38, 0x18), (g + 0x3A, 0), (g + 0x3C, 1)]);
    }
    // Spark clouds (+0x740 / +0xFE0 / +0x1880, particles +0x30 on),
    // texture 5 / 1; the third unlit (flags 3, alpha 0).
    fx.texture5(sp + 0xB0, 1);
    for (g, flags, a) in [(0x740u32, 0x2000_0003u32, 0x80u8), (0xFE0, 0x2000_0003, 0x80), (0x1880, 3, 0)] {
        let tex = fx.mem.read_u32(sp + 0xB0);
        fx.mem.write_u32(D + g, tex);
        fx.mem.write_u32(D + g + 4, flags);
        bytes(fx, &[(g + 0xA, 0x80), (g + 9, 0x80), (g + 8, 0x80), (g + 0xB, a)]);
        fx.mem.write_u32(D + g + 0x20, 0x3F00_0000);
        fx.mem.write_u32(D + g + 0x24, 0x40A0_0000);
        halves(fx, &[(g + 0x10, 0), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x20), (g + 0x18, 0), (g + 0x1A, 0), (g + 0x1C, 0x80), (g + 0x1E, 0x40)]);
        fx.mem.write_u32(D + g + 0xC, D + g + 0x30);
        fx.mem.write_u32(D + g + 0x28, 0x40A0_0000);
        halves(fx, &[(g + 0x2C, 0x80)]);
        bytes(fx, &[(g + 0x2E, 8), (g + 0x2F, 0)]);
        spawn::scatter(fx, D + g, 0xC, 4.0);
    }
    0
}

/// Point `p` at the node `node` until update 0x32, then 1 / `steps` of the
/// way to `to` (counting down) with the streamer `g` (points `pts`) pushed.
fn mover(fx: &mut Fx, p: u32, node: u32, to: u32, steps: u32, g: u32, pts: u32) {
    if fx.mem.i32(N) < 0x32 {
        let h = fx.self_handle();
        let s = fx.mem.i32(node);
        fx.node_position(h, s, p);
    } else {
        let m = fx.mem.i32(steps);
        fx.approach_by(p, to, m);
        if m != 0 {
            fx.mem.set32(steps, m - 1);
        }
        streamer::push(fx, g, pts, p);
    }
}

/// What 0x1038D0's 0x20C0B0 leaves at sp - 0xD0.. besides its register
/// saves: its prologue's `swc1 f20..f24`, the entry's f20 (the runner's
/// 0) [as b244 / b419].
fn floats_saved(fx: &mut Fx, sp: u32) {
    for k in 0..5u32 {
        fx.mem.write_u32(sp - 0xD0 + 4 * k, 0);
    }
}

/// A spark cloud `g` at the transform `x` (fading from 0x55), each live
/// particle (from `base`) pushed out by 1.2 and respawned in a flat ring
/// at height 4 above 30 (rand's frame: the entry's s0 = D; `ras` the
/// conversion's and the three rands' return addresses) [b419's].
fn sparks(fx: &mut Fx, g: u32, x: u32, base: u32, ras: [u32; 4]) {
    trail::sparks(fx, OT, g, x);
    if fx.mem.i32(N) >= 0x55 {
        fx.step_down(g + 0xB, 0, 8);
    }
    let grow = f32::from_bits(0x3F99_999A);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(g + 0x2C) {
        let p = base + 0x10 * j;
        j += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        for o in [0u32, 4, 8] {
            let v = ee::mul(fx.mem.f(p + o), grow);
            fx.mem.setf(p + o, v);
        }
        if ee_le(fx.mem.f(p + 4), 30.0) {
            continue;
        }
        let v = ee::mul(10.0, fx.mem.f(g + 0x28));
        fx.fix_residue(v, ras[0]);
        let spread = ee::fixsfsi(v);
        let a = fx.rand_from(D, ras[1]) & 0xFFFF;
        let r = fx.rand_from(D, ras[2]);
        let (_, h) = fx.div(r, spread);
        fx.mem.write_u32(p + 4, 0x4080_0000);
        let rad = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(rad, c));
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(rad, s));
        let r = fx.rand_from(D, ras[3]);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, -h);
    }
}

/// The third cloud (+0x1880 at +0x1810, particles +0x18B0): pushed out by
/// 1.4, from life 9 thrown again in a random direction of length 1
/// (0x1043F0 into sp + 0xA0) with the cloud's alpha.
fn burst(fx: &mut Fx, sp: u32) {
    let g = D + 0x1880;
    trail::sparks(fx, OT, g, D + 0x1810);
    if fx.mem.i32(N) >= 0x55 {
        fx.step_down(g + 0xB, 0, 8);
    }
    let grow = f32::from_bits(0x3FB3_3333);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(g + 0x2C) {
        let p = D + 0x18B0 + 0x10 * j;
        j += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        for o in [0u32, 4, 8] {
            let v = ee::mul(fx.mem.f(p + o), grow);
            fx.mem.setf(p + o, v);
        }
        if fx.mem.i16(p + 0xE) < 9 {
            continue;
        }
        fx.random_dir(sp + 0xA0, 1.0);
        for (o, s) in [(4u32, 0xA4u32), (8, 0xA8), (0, 0xA0)] {
            let w = fx.mem.read_u32(sp + s);
            fx.mem.write_u32(p + o, w);
        }
        let r = fx.rand_from(D, 0x01E0_37C8);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, -h);
        let a = fx.mem.read_u8(g + 0xB);
        fx.mem.write_u8(p + 0xC, a);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    for out in [0x610u32, 0x680] {
        let t = fx.target_handle();
        fx.unit_position(t, D + out);
    }
    mover(fx, D + 0x410, D + 0x20C4, D + 0x610, D + 0x20BC, D + 0x70, D + 0xB0);
    mover(fx, D + 0x510, D + 0x20C8, D + 0x680, D + 0x20C0, D + 0x230, D + 0x270);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x80);
    for c in 0..4u32 {
        let v = fx.mem.read_u32(D + 0x410 + 4 * c);
        fx.mem.write_u32(D + 0x1830 + 4 * c, v);
    }
    for c in 0..4u32 {
        let v = fx.mem.read_u32(D + 0x1830 + 4 * c);
        fx.mem.write_u32(D + 0x6F0 + 4 * c, v);
    }
    fx.look_angles(sp + 0x80, D + 0x610, D + 0x6E0);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x90);
    for c in 0..4u32 {
        let v = fx.mem.read_u32(D + 0x510 + 4 * c);
        fx.mem.write_u32(D + 0xF90 + 4 * c, v);
    }
    fx.look_angles(sp + 0x90, D + 0x680, D + 0xF80);
    // s0 = D, s1 = the OT, s2 = sp + 0x90, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(sp + 0x90), Some(0)];
    fx.xform_world_saving(D, 0x01E0_31E8, regs);
    floats_saved(fx, sp);
    for (x, ra) in [(0x3F0u32, 0x01E0_31F0u32), (0x4F0, 0x01E0_31F8), (0x5F0, 0x01E0_3200), (0x660, 0x01E0_3208)] {
        fx.xform_billboard_saving(D + x, ra, regs);
    }
    for (x, ra) in [(0x6D0u32, 0x01E0_3210u32), (0xF70, 0x01E0_3218), (0x1810, 0x01E0_3220)] {
        fx.xform_world_saving(D + x, ra, regs);
        floats_saved(fx, sp);
    }
    if (0x32..0x9E).contains(&n(fx)) {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) >= 0x44 {
        tint(fx, D + 0x20CC);
        if n(fx) < 0x54 {
            fx.step_up(D + 0x20CF, 0xFF, 0x10);
        } else {
            fx.step_down(D + 0x20CF, 0, 0x20);
        }
    }
    fx.disc(D + 0x460, D + 0x3F0);
    fx.disc(D + 0x560, D + 0x4F0);
    facing::quad(fx, OT, D + 0x470, D + 0x4D0, D + 0x3F0);
    facing::quad(fx, OT, D + 0x570, D + 0x5D0, D + 0x4F0);
    if n(fx) < 0x32 {
        for (x, disc, g) in [(0x3F0u32, 0x460u32, 0x470u32), (0x4F0, 0x560, 0x570)] {
            fx.approach(D + x, 1.0, f32::from_bits(0x3D4C_CCCD));
            fx.step_up(D + disc + 7, 0xFF, 0x20);
            fx.step_up(D + g + 0x37, 0x80, 0x10);
        }
    }
    let m = n(fx);
    if (0x32..0x54).contains(&m) {
        sparks(fx, D + 0x740, D + 0x6D0, D + 0x770, [0x01E0_3430, 0x01E0_3438, 0x01E0_3454, 0x01E0_34A4]);
        sparks(fx, D + 0xFE0, D + 0xF70, D + 0x1010, [0x01E0_3584, 0x01E0_358C, 0x01E0_35A8, 0x01E0_35F8]);
        for (s, pts, cnt) in [(0x70u32, 0xB0u32, 0xAAu32), (0x230, 0x270, 0x26A)] {
            streamer::draw(fx, STREAMER, OT, D + s, D + pts, D);
            let mut j = 0u32;
            while (j as i32) < fx.mem.i16(D + cnt) {
                fx.step_down(D + pts + 0xC + 0x10 * j, 0, 3);
                j += 1;
            }
        }
    }
    if n(fx) == 0x54 {
        fx.mem.write_u8(D + 0x188B, 0x80);
        let mut j = 0u32;
        while (j as i32) < fx.mem.i16(D + 0x18AC) {
            fx.mem.write_u8(D + 0x18BC + 0x10 * j, 0x80);
            j += 1;
        }
    }
    if n(fx) >= 0x32 {
        burst(fx, sp);
    }
    for (steps, x, disc, g, s) in [(0x20BCu32, 0x3F0u32, 0x460u32, 0x470u32, 0x70u32), (0x20C0, 0x4F0, 0x560, 0x570, 0x230)] {
        if fx.mem.i32(D + steps) == 0 {
            fx.approach(D + x, 10.0, 1.0);
            fx.step_down(D + disc + 7, 0, 0x10);
            fx.step_down(D + g + 0x37, 0, 8);
            fx.step_down(D + s + 7, 0, 5);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0x54 {
        2
    } else if m == 0x3A {
        1
    } else if (0x44..0x54).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
