//! BMODULE b419 (card 495), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02B30, data area a0 = 0x1ED0000) [F] (helper C8,
//! 31 Oct 2026): b244's parts twice, in one entry. Two points (+0x410 /
//! +0x510) sit at the attacker's node 0x35 until updates 0x66 / 0x7A,
//! then close on the target 8 to either side in x (+0x610 / +0x680, by
//! 1 / 30 a step) trailing a streamer each (+0x70 / +0x230, points +0xB0 /
//! +0x270); from 0x65 / 0x79 a disc (+0x460 / +0x560) and a facing quad
//! (+0x470 / +0x570) at the point (billboards +0x3F0 / +0x4F0, growing to
//! 5 from 0x84 / 0x98), sparks (+0x740 / +0xFE0) until 0xA1; a frame quad
//! (0x80, 0x80, 0x80, 0x60) from 0x66; from 0x98 a full-window tint
//! (+0x182C). Returns 1 on update 0x7A, 6 on 0x84..0xA1, 2 on 0xA2, 8
//! after 0xB4.
//!
//! Library (record addresses) as b244's: `colours4` 0x1E00080,
//! `frame_quad_rgba` 0x1E00120, `tint` 0x1E00240, `disc` 0x1E003A0,
//! `facing::quad` 0x1E00900, `scatter` 0x1E01190, `trail::sparks`
//! 0x1E01350, `streamer::push` / `draw` 0x1E020B0 / 0x1E02190.
use crate::b262::tint;
use crate::blib::{facing, spawn, streamer, trail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const STREAMER: u32 = 0x01E0_2190;
const N: u32 = D + 0x1814;
const LEFT: u32 = D + 0x1810;
const NODE: u32 = D + 0x1824;

/// The entry 0x1E02B30 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
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
    fx.bzero(D, 0x1830);
    for x in [0u32, 0x3F0, 0x4F0, 0x5F0, 0x660, 0x6D0, 0xF70] {
        fx.xform_init(D + x);
    }
    for (at, n, clear) in [(0x1824u32, 0x35, 1), (0x1828, 0x34, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(LEFT, 0xB4);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x1818, param);
    fx.mem.set32(D + 0x181C, 0x1E);
    fx.mem.set32(D + 0x1820, 0x1E);
    bytes(fx, &[(0x182E, 0xE0), (0x182D, 0xB0), (0x182C, 0x70), (0x182F, 0)]);
    // Discs (+0x460, +0x560): radius 4.
    for g in [0x460u32, 0x560] {
        fx.mem.write_u32(D + g, 0);
        bytes(fx, &[(g + 6, 0xE0), (g + 5, 0xB0), (g + 4, 0x70), (g + 7, 0xFF)]);
        fx.mem.write_u32(D + g + 8, 0x4080_0000);
        halves(fx, &[(g + 0xC, 0x24), (g + 0xE, 0)]);
    }
    // Facing quads (+0x470 / +0x570, quads +0x4D0 / +0x5D0), texture 5 / 0.
    fx.texture5(sp + 0xA0, 0);
    for (g, q) in [(0x470u32, 0x4D0u32), (0x570, 0x5D0)] {
        fx.mem.write_u32(D + g + 0x38, 3);
        halves(fx, &[(g, 0), (g + 2, 0), (g + 4, 0x40), (g + 6, 0x40), (g + 8, 0), (g + 0xA, 0), (g + 0xC, 0x80), (g + 0xE, 0x80)]);
        fx.mem.write_u32(D + g + 0x30, 0x3F80_0000);
        bytes(fx, &[(g + 0x36, 0x80), (g + 0x35, 0x80), (g + 0x34, 0x80), (g + 0x37, 0x80)]);
        halves(fx, &[(g + 0x48, 0), (g + 0x4A, 2), (g + 0x4C, 3), (g + 0x4E, 4), (g + 0x50, 0), (g + 0x52, 0x3E7)]);
        let tex = fx.mem.read_u32(sp + 0xA0);
        for o in [0x18u32, 0x14, 0x10] {
            fx.mem.write_u32(D + q + o, 0);
        }
        fx.mem.write_u32(D + q + 8, 0x4040_0000);
        fx.mem.write_u32(D + q, tex);
        bytes(fx, &[(q + 0xE, 0x80), (q + 0xD, 0x80), (q + 0xC, 0x80), (q + 0xF, 0x80)]);
    }
    // Streamers (+0x70 / +0x230), texture 5 / 1.
    fx.texture5(sp + 0xA0, 1);
    for g in [0x70u32, 0x230] {
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.mem.write_u32(D + g, tex);
        bytes(fx, &[(g + 6, 0x80), (g + 5, 0x80), (g + 4, 0x80), (g + 7, 0x80)]);
        fx.mem.write_u32(D + g + 8, 0x8003);
        fx.mem.write_u32(D + g + 0xC, 0x41A0_0000);
        halves(fx, &[(g + 0x10, 0x60), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x20), (g + 0x18, 0x60), (g + 0x1A, 0), (g + 0x1C, 0x80), (g + 0x1E, 0x40)]);
        fx.mem.write_u32(D + g + 0x30, 0x3D4C_CCCD);
        halves(fx, &[(g + 0x34, 0), (g + 0x36, 0x10), (g + 0x38, 0x18), (g + 0x3A, 0), (g + 0x3C, 1)]);
    }
    // Spark clouds (+0x740 / +0xFE0, particles +0x770 / +0x1010).
    for g in [0x740u32, 0xFE0] {
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.mem.write_u32(D + g, tex);
        fx.mem.write_u32(D + g + 4, 0x2000_0003);
        bytes(fx, &[(g + 0xA, 0x80), (g + 9, 0x80), (g + 8, 0x80), (g + 0xB, 0x80)]);
        fx.mem.write_u32(D + g + 0x20, 0x3E99_999A);
        fx.mem.write_u32(D + g + 0x24, 0x4120_0000);
        halves(fx, &[(g + 0x10, 0), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x20), (g + 0x18, 0), (g + 0x1A, 0), (g + 0x1C, 0x80), (g + 0x1E, 0x40)]);
        fx.mem.write_u32(D + g + 0xC, D + g + 0x30);
        fx.mem.write_u32(D + g + 0x28, 0x40A0_0000);
        halves(fx, &[(g + 0x2C, 0x80)]);
        bytes(fx, &[(g + 0x2E, 8), (g + 0x2F, 0)]);
        spawn::scatter(fx, D + g, 0xC, 4.0);
    }
    0
}

/// Point `p` at the node until update `leave`, then 1 / `steps` of the way
/// to `to` (counting down) with the streamer `g` (points `pts`) pushed.
fn mover(fx: &mut Fx, p: u32, leave: i32, to: u32, steps: u32, g: u32, pts: u32) {
    if fx.mem.i32(N) < leave {
        let h = fx.self_handle();
        let s = fx.mem.i32(NODE);
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
/// 0) [as b244].
fn floats_saved(fx: &mut Fx, sp: u32) {
    for k in 0..5u32 {
        fx.mem.write_u32(sp - 0xD0 + 4 * k, 0);
    }
}

/// A spark cloud `g` at the transform `x` (fading from `fade`), each live
/// particle (from `base`) pushed out by 1.2 and respawned in a flat ring
/// at height 4 above 30 (rand's frame: the entry's s0 = D; `ras` the
/// conversion's and the three rands' return addresses).
fn sparks(fx: &mut Fx, g: u32, x: u32, fade: i32, base: u32, ras: [u32; 4]) {
    trail::sparks(fx, OT, g, x);
    if fx.mem.i32(N) >= fade {
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

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    for out in [0x610u32, 0x680] {
        let t = fx.target_handle();
        fx.unit_position(t, D + out);
    }
    let (a, b) = (fx.mem.f(D + 0x610), fx.mem.f(D + 0x680));
    if crate::exe::ee_lt(fx.mem.f(D + 0x618), 0.0) {
        fx.mem.setf(D + 0x610, ee::add(a, 8.0));
        fx.mem.setf(D + 0x680, ee::sub(b, 8.0));
    } else {
        fx.mem.setf(D + 0x610, ee::sub(a, 8.0));
        fx.mem.setf(D + 0x680, ee::add(b, 8.0));
    }
    mover(fx, D + 0x410, 0x66, D + 0x610, D + 0x181C, D + 0x70, D + 0xB0);
    mover(fx, D + 0x510, 0x7A, D + 0x680, D + 0x1820, D + 0x230, D + 0x270);
    for (p, to, at, out, o) in [(0x410u32, 0x610u32, 0x6F0u32, 0x6E0u32, 0x80u32), (0x510, 0x680, 0xF90, 0xF80, 0x90)] {
        let h = fx.self_handle();
        fx.unit_position(h, sp + o);
        for c in 0..4u32 {
            let v = fx.mem.read_u32(D + p + 4 * c);
            fx.mem.write_u32(D + at + 4 * c, v);
        }
        fx.look_angles(sp + o, D + to, D + out);
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x90, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(sp + 0x90), Some(0)];
    fx.xform_world_saving(D, 0x01E0_3184, regs);
    floats_saved(fx, sp);
    for (x, ra) in [(0x3F0u32, 0x01E0_318Cu32), (0x4F0, 0x01E0_3194), (0x5F0, 0x01E0_319C), (0x660, 0x01E0_31A4)] {
        fx.xform_billboard_saving(D + x, ra, regs);
    }
    for (x, ra) in [(0x6D0u32, 0x01E0_31ACu32), (0xF70, 0x01E0_31B4)] {
        fx.xform_world_saving(D + x, ra, regs);
        floats_saved(fx, sp);
    }
    if n(fx) >= 0x66 {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) >= 0x98 {
        tint(fx, D + 0x182C);
        if n(fx) < 0xA2 {
            fx.step_up(D + 0x182F, 0xFF, 0x10);
        } else {
            fx.step_down(D + 0x182F, 0, 0x10);
        }
    }
    for (from, grow, disc, g, q, x) in [(0x65, 0x84, 0x460u32, 0x470u32, 0x4D0u32, 0x3F0u32), (0x79, 0x98, 0x560, 0x570, 0x5D0, 0x4F0)] {
        if n(fx) >= from {
            fx.disc(D + disc, D + x);
            facing::quad(fx, OT, D + g, D + q, D + x);
            if n(fx) >= grow {
                fx.approach(D + x, 5.0, f32::from_bits(0x3E4C_CCCD));
                fx.step_down(D + disc + 7, 0, 0x10);
                fx.step_down(D + g + 0x37, 0, 8);
            }
        }
    }
    let m = n(fx);
    if (0x66..0xA2).contains(&m) {
        sparks(fx, D + 0x740, D + 0x6D0, 0x85, D + 0x770, [0x01E0_33E8, 0x01E0_33F0, 0x01E0_340C, 0x01E0_345C]);
        streamer::draw(fx, STREAMER, OT, D + 0x70, D + 0xB0, D);
        let mut j = 0u32;
        while (j as i32) < fx.mem.i16(D + 0xAA) {
            fx.step_down(D + 0xBC + 0x10 * j, 0, 8);
            j += 1;
        }
    }
    let m = n(fx);
    if (0x7A..0xA2).contains(&m) {
        sparks(fx, D + 0xFE0, D + 0xF70, 0x99, D + 0x1010, [0x01E0_35A0, 0x01E0_35A8, 0x01E0_35C4, 0x01E0_3614]);
        streamer::draw(fx, STREAMER, OT, D + 0x230, D + 0x270, D);
        let mut j = 0u32;
        while (j as i32) < fx.mem.i16(D + 0x26A) {
            fx.step_down(D + 0x27C + 0x10 * j, 0, 3);
            j += 1;
        }
    }
    for (steps, x, disc, g, s) in [(0x181Cu32, 0x3F0u32, 0x460u32, 0x470u32, 0x70u32), (0x1820, 0x4F0, 0x560, 0x570, 0x230)] {
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
    if m == 0xA2 {
        2
    } else if m == 0x7A {
        1
    } else if (0x84..0xA2).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
