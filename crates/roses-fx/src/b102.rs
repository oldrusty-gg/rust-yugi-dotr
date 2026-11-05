//! BMODULE b102 (card 306), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01980, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): b600's family. From update 103 a tube (+0x70,
//! [`crate::tube`]: 32 points, radius 7, 36 rings 4 apart) from the
//! attacker's node 3 aimed at node 2, its red byte flickering; a column
//! of sparks (+0x640, 128, animated) rising round the attacker,
//! respawned above 150 and fading in from update 117; at the target's
//! animated position a spark burst (+0x16E0) lit from update 152; the
//! frame quad from update 190 and the stage light down to 0x20 until
//! update 310. Returns 1 on update 142, 6 on 152..309, 2 on 310, 8 after
//! 327.
//!
//! Library: `colours4` (0x1E00080), the frame quad (0x1E00120,
//! [`crate::framegs`]), `spawn::scatter` (0x1E00240), `trail::sparks`
//! (0x1E00400) [`crate::blib`], the tube (0x1E01160 / 0x1E01260).
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const TENTH: u32 = 0x3DCC_CCCD;

/// The entry 0x1E01980.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

/// Spark k of the column (record *(D + 0x64C) + 16 k) at a random angle,
/// radius (rand % 60) / 10, height (rand % `high`) / 10 [F 0x1E01B70 /
/// 0x1E01F28]; the rands return to `ras`.
fn place(fx: &mut Fx, k: u32, high: i32, ras: [u32; 3]) {
    let a = fx.rand_from(D, ras[0]) & 0xFFFF;
    let r = fx.rand_from(D, ras[1]);
    let (_, h) = fx.div(r, 0x3C);
    let radius = ee::mul(f32::from_bits(TENTH), ee::from_i32(h));
    let r = fx.rand_from(D, ras[2]);
    let (_, h) = fx.div(r, high);
    let e = fx.mem.read_u32(D + 0x64C).wrapping_add(16 * k);
    fx.mem.setf(e + 4, ee::mul(f32::from_bits(TENTH), ee::from_i32(h)));
    let c = fx.tcos(a);
    let e = fx.mem.read_u32(D + 0x64C).wrapping_add(16 * k);
    fx.mem.setf(e + 8, ee::mul(radius, c));
    let s = fx.tsin(a);
    let e = fx.mem.read_u32(D + 0x64C).wrapping_add(16 * k);
    fx.mem.setf(e, ee::mul(radius, s));
}

/// A white animated spark group at `g` (records at g + 0x30, cell (0, 0,
/// 32, 64), 5 frames, a step every 100 updates), scattered.
#[allow(clippy::too_many_arguments)]
fn sparks_group(fx: &mut Fx, g: u32, tex: u32, flags: u32, alpha: u8, width: u32, count: i32) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, flags);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, alpha)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0x20, width);
    fx.mem.write_u32(g + 0x24, 0x4130_0000);
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x20), (0x16, 0x40), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x80)]);
    fx.mem.write_u32(g + 0xC, g + 0x30);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, count);
    fx.mem.write_u8(g + 0x2E, 5);
    fx.mem.write_u8(g + 0x2F, 0x64);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2730);
    for x in [0u32, 0x5D0, 0x1670] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x271Cu32, 3, 1), (0x2720, 2, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2710, 0x147);
    fx.mem.set32(D + 0x2714, 1);
    fx.mem.set32(D + 0x2718, param);
    fx.mem.set32(D + 0x2724, 0x80);
    fx.texture5(sp + 0xB0, 0);
    // The tube (+0x70): 32 points, radius 7, 36 records at +0x1D0, rings
    // 4 apart, cell (0, 0, 128, 128).
    let g = D + 0x70;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    for (k, x) in [(6u32, 0x40u8), (5, 0x40), (4, 0x80), (7, 0x80)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 8, 0);
    fx.mem.write_u32(g + 0xC, 0x40E0_0000);
    halves(fx, g, &[(0x140, 0x20), (0x142, 0)]);
    fx.mem.write_u32(g + 0x144, 0x4080_0000);
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x80), (0x16, 0x80), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x80)]);
    fx.mem.write_u32(g + 0x148, D + 0x1D0);
    halves(fx, g, &[(0x14C, 0x24), (0x14E, 0)]);
    fx.mem.write_u32(g + 0x150, 0x3F80_0000);
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.tube_setup(g, tex);
    fx.texture5(sp + 0xB0, 1);
    let tex = fx.mem.read_u32(sp + 0xB0);
    sparks_group(fx, D + 0x640, tex, 0x6000_0003, 0x40, 0x4000_0000, 0x80);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x66C) {
        place(fx, k as u32, 0x5DC, [0x01E0_1B78, 0x01E0_1B94, 0x01E0_1BC0]);
        let e = fx.mem.read_u32(D + 0x64C).wrapping_add(16 * k as u32);
        fx.mem.write_u8(e + 0xC, 0);
        let r = fx.rand_from(D, 0x01E0_1C34);
        let (_, h) = fx.div(r, 5);
        let e = fx.mem.read_u32(D + 0x64C).wrapping_add(16 * k as u32);
        fx.mem.set8(e + 0xD, h);
        k += 1;
    }
    let tex = fx.mem.read_u32(sp + 0xB0);
    sparks_group(fx, D + 0x16E0, tex, 3, 0, 0x4040_0000, 0x100);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x170C) {
        let r = fx.rand_from(D, 0x01E0_1D10);
        let (_, h) = fx.div(r, 5);
        let e = fx.mem.read_u32(D + 0x16EC).wrapping_add(16 * k as u32);
        fx.mem.set8(e + 0xD, h);
        k += 1;
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2714);
    let node = |fx: &mut Fx, slot: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + slot);
        fx.node_position(h, s, out);
    };
    let aiming = n(fx) < 0xBE;
    if aiming {
        node(fx, 0x271C, sp + 0x80);
        node(fx, 0x2720, sp + 0x90);
        fx.look_angles(sp + 0x80, sp + 0x90, D + 0x5E0);
        node(fx, 0x271C, D + 0x5F0);
    }
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x1690);
    // s0 = D, s1 = the OT, s2 = sp + 0x90 while aiming, else the parameter
    // (-1, sign-extended: not modelled), s3 the caller's.
    let regs = [Some(D), Some(OT), aiming.then_some(sp + 0x90), None];
    fx.xform_world_saving(D, 0x01E0_1DD4, regs);
    fx.xform_world_saving(D + 0x5D0, 0x01E0_1DDC, regs);
    fx.xform_world_saving(D + 0x1670, 0x01E0_1DE4, regs);
    if n(fx) < 0x136 {
        fx.step_down(D + 0x2724, 0x20, 4);
    } else {
        fx.step_up(D + 0x2724, 0x80, 4);
    }
    let light = fx.mem.u8(D + 0x2724);
    fx.set_stage_light(light);
    if n(fx) >= 0xBE {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) >= 0x67 {
        let r = fx.rand_from(D, 0x01E0_1E58);
        fx.mem.set8(D + 0x74, r % 64 + 0x40);
        fx.tube_draw(OT, D + 0x70, D + 0x5D0);
        if n(fx) >= 0x10F {
            fx.step_down(D + 0x77, 0, 8);
        }
        let c = fx.mem.i16(D + 0x1BE) + 1;
        fx.mem.set16(D + 0x1BE, c);
    }
    blib::trail::sparks(fx, OT, D + 0x640, D + 0x5D0);
    if n(fx) >= 0x10F {
        fx.step_down(D + 0x64B, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x66C) {
        let p = D + 16 * k as u32;
        if fx.mem.i16(p + 0x67E) >= 0 {
            let y = ee::add(fx.mem.f(p + 0x674), 4.0);
            fx.mem.setf(p + 0x674, y);
            if !ee_le(y, 150.0) {
                place(fx, k as u32, 0x1E, [0x01E0_1F30, 0x01E0_1F4C, 0x01E0_1F78]);
                fx.mem.set16(p + 0x67E, 0);
                let a = if n(fx) >= 0x75 { 1 } else { 0 };
                fx.mem.write_u8(p + 0x67C, a);
            }
            if fx.mem.read_u8(p + 0x67C) != 0 {
                fx.step_up(p + 0x67C, 0x80, 0x10);
            }
        }
        k += 1;
    }
    if n(fx) == 0x98 {
        fx.mem.write_u8(D + 0x16EB, 0x80);
    }
    blib::trail::sparks(fx, OT, D + 0x16E0, D + 0x1670);
    if n(fx) >= 0x137 {
        fx.step_down(D + 0x16EB, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x170C) {
        let p = D + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x171E) < 0 {
            continue;
        }
        let s = f32::from_bits(0x3FA6_6666);
        for c in 0..3u32 {
            let x = ee::mul(fx.mem.f(p + 0x1710 + 4 * c), s);
            fx.mem.setf(p + 0x1710 + 4 * c, x);
        }
        if fx.mem.i16(p + 0x171E) < 0x11 {
            continue;
        }
        fx.random_dir(sp + 0xA0, 1.0);
        for c in [1u32, 2, 0] {
            let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
            fx.mem.write_u32(p + 0x1710 + 4 * c, w);
        }
        let r = fx.rand_from(D, 0x01E0_2110);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0x171E, h.wrapping_neg());
        let a = if n(fx) >= 0x98 { 0x80 } else { 0 };
        fx.mem.write_u8(p + 0x171C, a);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2714, m);
    let left = fx.mem.i32(D + 0x2710) - 1;
    fx.mem.set32(D + 0x2710, left);
    if m == 0x136 {
        2
    } else if m == 0x8E {
        1
    } else if (0x98..0x136).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
