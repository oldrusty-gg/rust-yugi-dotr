//! BMODULE b600 (card 388), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01980, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): until update 96 the attacker's object op 0x1F83B0 (mode 3,
//! colour 0x80 x 7, a boundary call); from update 173 the frame quad and
//! a tube (+0x70, [`crate::tube`]: 32 points, radius 2.5 growing 1 per
//! ring, 20 rings 2 apart, one more ring each update) aimed from node 3
//! at the target, its red byte flickering; at the target a spark burst
//! (+0x6B0, `blib::trail::sparks`) lit from update 197 and thrown out
//! again as each spark dies; the stage light down to 0x20 until update
//! 400. Returns 1 on update 187, 6 on 197..399, 2 on 400, 8 after 420.
//!
//! Library: `colours4` (0x1E00080), the frame quad (0x1E00120,
//! [`crate::framegs`]), `spawn::scatter` (0x1E00240), `trail::sparks`
//! (0x1E00400) [`crate::blib`], the tube (0x1E01160 / 0x1E01260).
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E01980.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x80);
    ret
}

fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1700);
    for x in [0u32, 0x5D0, 0x640] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x16ECu32, 3, 1), (0x16F0, 1, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x16E0, 0x1A5);
    fx.mem.set32(D + 0x16E4, 1);
    fx.mem.set32(D + 0x16E8, param);
    fx.mem.set32(D + 0x16F4, 0x80);
    fx.texture5(sp + 0x70, 0);
    // The tube (+0x70): 32 points, radius 2.5 (+1 per ring), 20 records
    // at +0x1D0, rings 2 apart, cell (0, 0, 128, 128).
    let g = D + 0x70;
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.mem.write_u32(g, tex);
    for (k, x) in [(6u32, 0x80u8), (5, 0x40), (4, 0x60), (7, 0x80)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 8, 0);
    fx.mem.write_u32(g + 0xC, 0x4020_0000);
    halves(fx, g, &[(0x140, 0x20), (0x142, 0)]);
    fx.mem.write_u32(g + 0x144, 0x4000_0000);
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x80), (0x16, 0x80), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x80)]);
    fx.mem.write_u32(g + 0x148, D + 0x1D0);
    halves(fx, g, &[(0x14C, 0x14), (0x14E, 0)]);
    fx.mem.write_u32(g + 0x150, 0x3F80_0000);
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.tube_setup(g, tex);
    fx.texture4(sp + 0x70, 0xF);
    // The sparks (+0x6B0): 256 at +0x6E0, width 0.3, length 11.
    let g = D + 0x6B0;
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 3);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x40), (8, 0x60), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0x20, 0x3E99_999A);
    fx.mem.write_u32(g + 0x24, 0x4130_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x60), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x60), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, D + 0x6E0);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x16E4);
    let node = |fx: &mut Fx, slot: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + slot);
        fx.node_position(h, s, out);
    };
    let aiming = n(fx) < 0x186;
    if aiming {
        let t = fx.target_handle();
        fx.unit_position(t, sp + 0x40);
        node(fx, 0x16F0, sp + 0x50);
        fx.look_angles(sp + 0x40, sp + 0x50, D + 0x5E0);
        node(fx, 0x16EC, D + 0x5F0);
    }
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x660);
    // s0 = D, s1 = the OT, s2 = sp + 0x50 while aiming, else the parameter
    // (-1, sign-extended: not modelled), s3 the caller's.
    let regs = [Some(D), Some(OT), aiming.then_some(sp + 0x50), None];
    fx.xform_world_saving(D, 0x01E0_1BEC, regs);
    fx.xform_world_saving(D + 0x5D0, 0x01E0_1BF4, regs);
    fx.xform_world_saving(D + 0x640, 0x01E0_1BFC, regs);
    if n(fx) < 0x190 {
        fx.step_down(D + 0x16F4, 0x20, 4);
    } else {
        fx.step_up(D + 0x16F4, 0x80, 4);
    }
    let light = fx.mem.u8(D + 0x16F4);
    fx.set_stage_light(light);
    if n(fx) >= 0xAD {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0x60 {
        let h = fx.self_handle();
        for k in [0x7Cu32, 0x7D, 0x7E, 0x7B, 0x78, 0x79, 0x7A] {
            fx.mem.write_u8(sp + k, 0x80);
        }
        fx.mem.write_u8(sp + 0x7F, 0);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h as u32, 3, 0, sp + 0x78]);
    }
    if n(fx) >= 0xAD {
        let r = fx.rand_from(D, 0x01E0_1CC8);
        fx.mem.set8(D + 0x74, r % 64 + 0x40);
        fx.tube_draw(OT, D + 0x70, D + 0x5D0);
        if n(fx) >= 0x191 {
            fx.step_down(D + 0x77, 0, 0x10);
        }
        let c = fx.mem.i16(D + 0x1BE) + 1;
        fx.mem.set16(D + 0x1BE, c);
    }
    if n(fx) == 0xC5 {
        fx.mem.write_u8(D + 0x6BB, 0x80);
    }
    blib::trail::sparks(fx, OT, D + 0x6B0, D + 0x640);
    if n(fx) >= 0x189 {
        fx.step_down(D + 0x6BB, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x6DC) {
        let p = D + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x6EE) < 0 {
            continue;
        }
        let s = f32::from_bits(0x3FA6_6666);
        for c in 0..3u32 {
            let x = ee::mul(fx.mem.f(p + 0x6E0 + 4 * c), s);
            fx.mem.setf(p + 0x6E0 + 4 * c, x);
        }
        if fx.mem.i16(p + 0x6EE) < 0x11 {
            continue;
        }
        fx.random_dir(sp + 0x60, 1.0);
        for c in [1u32, 2, 0] {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(p + 0x6E0 + 4 * c, w);
        }
        let r = fx.rand_from(D, 0x01E0_1DF0);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0x6EE, h.wrapping_neg());
        let a = if n(fx) >= 0xC5 { 0x80 } else { 0 };
        fx.mem.write_u8(p + 0x6EC, a);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x16E4, m);
    let left = fx.mem.i32(D + 0x16E0) - 1;
    fx.mem.set32(D + 0x16E0, left);
    if m == 0x190 {
        2
    } else if m == 0xBB {
        1
    } else if (0xC5..0x190).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
