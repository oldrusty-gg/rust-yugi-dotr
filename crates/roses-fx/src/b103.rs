//! BMODULE b103 (card 62), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02B30, data area a0 = 0x1ED0000) [F] (helper B9, 31
//! Oct 2026). A point (+0x250) at the midpoint of the attacker's nodes 0xB
//! / 0x1A until update 292, then moving to the target (+0x350) by 1 / 18 a
//! step, trailing a streamer (+0x70, points +0xB0); a frame quad (0x80,
//! 0x80, 0x80, 0x60) on 90..157; from 146 a disc (+0x2A0) and a facing
//! quad (+0x2B0 / +0x310) at the point (billboard +0x230 growing to 1 by
//! 0.05); on 292..325 sparks (+0x410) at the point turned to look at the
//! target, respawning above 30; from 292 a second spark cloud (+0xCB0)
//! growing by 1.4 at the point; from 310 a full-window tint (+0x14F8).
//! Returns 1 on update 300, 6 on 310..325, 2 on 326, 8 after 340.
//!
//! Library (record addresses): `colours4` 0x1E00080, `frame_quad_rgba`
//! 0x1E00120, `tint` 0x1E00240, `disc` 0x1E003A0, `facing::quad`
//! 0x1E00900, `scatter` 0x1E01190, `trail::sparks` 0x1E01350,
//! `streamer::push` / `draw` 0x1E020B0 / 0x1E02190.
use crate::b262::tint;
use crate::blib::{facing, spawn, streamer, trail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const STREAMER: u32 = 0x01E0_2190;

/// The entry 0x1E02B30 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
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

/// A spark cloud `g` (+0x30 particles): flags `flags`, half size 0.5,
/// spread 5, 128 particles, 8 frames.
fn sparks_init(fx: &mut Fx, g: u32, tex: u32, flags: u32) {
    fx.mem.write_u32(D + g, tex);
    fx.mem.write_u32(D + g + 4, flags);
    bytes(fx, &[(g + 0xA, 0x80), (g + 9, 0x80), (g + 8, 0x80), (g + 0xB, if flags == 3 { 0 } else { 0x80 })]);
    fx.mem.write_u32(D + g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(D + g + 0x24, 0x40A0_0000);
    halves(fx, &[(g + 0x10, 0), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x20), (g + 0x18, 0), (g + 0x1A, 0), (g + 0x1C, 0x80), (g + 0x1E, 0x40)]);
    fx.mem.write_u32(D + g + 0xC, D + g + 0x30);
    fx.mem.write_u32(D + g + 0x28, 0x40A0_0000);
    halves(fx, &[(g + 0x2C, 0x80)]);
    bytes(fx, &[(g + 0x2E, 8), (g + 0x2F, 0)]);
    spawn::scatter(fx, D + g, 0xC, 4.0);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1500);
    fx.xform_init(D);
    fx.xform_init(D + 0x230);
    for o in [0x238u32, 0x234, 0x230] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for x in [0x330u32, 0x3A0, 0xC40] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x14F0u32, 0xB, 1), (0x14F4, 0x1A, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x14E0, 0x154);
    fx.mem.set32(D + 0x14E4, 1);
    fx.mem.set32(D + 0x14E8, param);
    fx.mem.set32(D + 0x14EC, 0x12);
    bytes(fx, &[(0x14FA, 0xC0), (0x14F9, 0x80), (0x14F8, 0xA0), (0x14FB, 0)]);
    // The disc (+0x2A0): radius 4.
    fx.mem.write_u32(D + 0x2A0, 0);
    bytes(fx, &[(0x2A6, 0xA0), (0x2A5, 0x60), (0x2A4, 0x80), (0x2A7, 0)]);
    fx.mem.write_u32(D + 0x2A8, 0x4080_0000);
    halves(fx, &[(0x2AC, 0x24), (0x2AE, 0)]);
    // The facing quad (+0x2B0, its quad +0x310), texture 5 / 2.
    fx.texture5(sp + 0xB0, 2);
    fx.mem.write_u32(D + 0x2E8, 3);
    halves(fx, &[(0x2B0, 0), (0x2B2, 0), (0x2B4, 0x40), (0x2B6, 0x40), (0x2B8, 0), (0x2BA, 0), (0x2BC, 0x80), (0x2BE, 0x80)]);
    fx.mem.write_u32(D + 0x2E0, 0x3F80_0000);
    bytes(fx, &[(0x2E6, 0x80), (0x2E5, 0x80), (0x2E4, 0x80), (0x2E7, 0)]);
    halves(fx, &[(0x2F8, 0), (0x2FA, 2), (0x2FC, 3), (0x2FE, 4), (0x300, 0), (0x302, 0x3E7)]);
    let tex = fx.mem.read_u32(sp + 0xB0);
    for o in [0x328u32, 0x324, 0x320] {
        fx.mem.write_u32(D + o, 0);
    }
    fx.mem.write_u32(D + 0x318, 0x4040_0000);
    fx.mem.write_u32(D + 0x310, tex);
    bytes(fx, &[(0x31E, 0x80), (0x31D, 0x80), (0x31C, 0x80), (0x31F, 0x80)]);
    // The streamer (+0x70), texture 5 / 0.
    fx.texture5(sp + 0xB0, 0);
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(D + 0x70, tex);
    bytes(fx, &[(0x76, 0x80), (0x75, 0x80), (0x74, 0x80), (0x77, 0x50)]);
    fx.mem.write_u32(D + 0x78, 0x8003);
    fx.mem.write_u32(D + 0x7C, 0x41A0_0000);
    halves(fx, &[(0x80, 0x9C), (0x82, 0), (0x84, 0x40), (0x86, 0x40), (0x88, 0x9C), (0x8A, 0), (0x8C, 0x100), (0x8E, 0x80)]);
    fx.mem.write_u32(D + 0xA0, 0x3DCC_CCCD);
    halves(fx, &[(0xA4, 0), (0xA6, 0x10), (0xA8, 0x18), (0xAA, 0), (0xAC, 1)]);
    // Two spark clouds (+0x410, +0xCB0), texture 5 / 1.
    fx.texture5(sp + 0xB0, 1);
    let tex = fx.mem.read_u32(sp + 0xB0);
    sparks_init(fx, 0x410, tex, 0x2000_0003);
    let tex = fx.mem.read_u32(sp + 0xB0);
    sparks_init(fx, 0xCB0, tex, 3);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x14E4);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x350);
    let near = n(fx) < 0x124;
    if near {
        for (slot, out) in [(0x14F0u32, sp + 0x80), (0x14F4, sp + 0x90)] {
            let h = fx.self_handle();
            let s = fx.mem.i32(D + slot);
            fx.node_position(h, s, out);
        }
        fx.midpoint(sp + 0x80, sp + 0x90, D + 0x250);
    } else {
        let m = fx.mem.i32(D + 0x14EC);
        fx.approach_by(D + 0x250, D + 0x350, m);
        if m != 0 {
            fx.mem.set32(D + 0x14EC, m - 1);
        }
        streamer::push(fx, D + 0x70, D + 0xB0, D + 0x250);
    }
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x80);
    for c in 0..4u32 {
        let v = fx.mem.read_u32(D + 0x250 + 4 * c);
        fx.mem.write_u32(D + 0xC60 + 4 * c, v);
    }
    for c in 0..4u32 {
        let v = fx.mem.read_u32(D + 0xC60 + 4 * c);
        fx.mem.write_u32(D + 0x3C0 + 4 * c, v);
    }
    fx.look_angles(sp + 0x80, D + 0x350, D + 0x3B0);
    // s0 = D, s1 = the OT, s2 = sp + 0x90 (the node branch) or the
    // parameter (-1, 64 bits), s3 the runner's 0.
    let s2 = if near { u64::from(sp + 0x90) } else { u64::MAX };
    let regs = [Some(D), Some(OT), None, Some(0)];
    for (x, ra, bill) in [(0u32, 0x01E0_2FBC, false), (0x230, 0x01E0_2FC4, true), (0x330, 0x01E0_2FCC, true), (0x3A0, 0x01E0_2FD4, false), (0xC40, 0x01E0_2FDC, false)] {
        if bill {
            fx.xform_billboard_saving(D + x, ra, regs);
            fx.saved(sp - 0xF0, &[(0x40, s2)]);
        } else {
            fx.xform_world_saving(D + x, ra, regs);
            fx.saved(sp - 0xD0, &[(0x40, s2)]);
        }
    }
    if (0x5A..0x9E).contains(&n(fx)) {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) >= 0x136 {
        tint(fx, D + 0x14F8);
        if n(fx) < 0x146 {
            fx.step_up(D + 0x14FB, 0xFF, 0x10);
        } else {
            fx.step_down(D + 0x14FB, 0, 0x20);
        }
    }
    if n(fx) >= 0x92 {
        fx.disc(D + 0x2A0, D + 0x230);
        facing::quad(fx, OT, D + 0x2B0, D + 0x310, D + 0x230);
        if n(fx) < 0x124 {
            fx.approach(D + 0x230, 1.0, f32::from_bits(0x3D4C_CCCD));
            fx.step_up(D + 0x2A7, 0xFF, 0x20);
            fx.step_up(D + 0x2E7, 0x80, 0x10);
        }
    }
    if (0x124..0x146).contains(&n(fx)) {
        trail::sparks(fx, OT, D + 0x410, D + 0x3A0);
        if n(fx) >= 0x147 {
            fx.step_down(D + 0x41B, 0, 8);
        }
        let grow = f32::from_bits(0x3F99_999A);
        let mut k = 0u32;
        while (k as i32) < fx.mem.i16(D + 0x43C) {
            let p = D + 0x10 * k;
            k += 1;
            if fx.mem.i16(p + 0x44E) < 0 {
                continue;
            }
            for o in [0x440u32, 0x444, 0x448] {
                let v = ee::mul(fx.mem.f(p + o), grow);
                fx.mem.setf(p + o, v);
            }
            if ee_le(fx.mem.f(p + 0x444), 30.0) {
                continue;
            }
            let spread = ee::fixsfsi(ee::mul(10.0, fx.mem.f(D + 0x438)));
            let a = fx.rand_from(D, 0x01E0_31A4) & 0xFFFF;
            let r = fx.rand_from(D, 0x01E0_31C0);
            let (_, h) = fx.div(r, spread);
            fx.mem.write_u32(p + 0x444, 0x4080_0000);
            let rad = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
            let c = fx.tcos(a);
            fx.mem.setf(p + 0x448, ee::mul(rad, c));
            let s = fx.tsin(a);
            fx.mem.setf(p + 0x440, ee::mul(rad, s));
            let r = fx.rand_from(D, 0x01E0_3210);
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0x44E, -h);
        }
        streamer::draw(fx, STREAMER, OT, D + 0x70, D + 0xB0, D);
        let mut k = 0u32;
        while (k as i32) < fx.mem.i16(D + 0xAA) {
            fx.step_down(D + 0xBC + 0x10 * k, 0, 5);
            k += 1;
        }
    }
    if n(fx) == 0x146 {
        fx.mem.write_u8(D + 0xCBB, 0x80);
        let mut k = 0u32;
        while (k as i32) < fx.mem.i16(D + 0xCDC) {
            fx.mem.write_u8(D + 0xCEC + 0x10 * k, 0x80);
            k += 1;
        }
    }
    if n(fx) >= 0x124 {
        trail::sparks(fx, OT, D + 0xCB0, D + 0xC40);
        if n(fx) >= 0x147 {
            fx.step_down(D + 0xCBB, 0, 8);
        }
        let grow = f32::from_bits(0x3FB3_3333);
        let mut k = 0u32;
        while (k as i32) < fx.mem.i16(D + 0xCDC) {
            let p = D + 0x10 * k;
            k += 1;
            if fx.mem.i16(p + 0xCEE) < 0 {
                continue;
            }
            for o in [0xCE0u32, 0xCE4, 0xCE8] {
                let v = ee::mul(fx.mem.f(p + o), grow);
                fx.mem.setf(p + o, v);
            }
            if fx.mem.i16(p + 0xCEE) >= 9 {
                fx.random_dir(sp + 0xA0, 1.0);
                for (o, s) in [(0xCE4u32, 0xA4u32), (0xCE8, 0xA8), (0xCE0, 0xA0)] {
                    let v = fx.mem.read_u32(sp + s);
                    fx.mem.write_u32(p + o, v);
                }
                let r = fx.rand_from(D, 0x01E0_3398);
                let (_, h) = fx.div(r, 0xC);
                fx.mem.set16(p + 0xCEE, -h);
                let a = fx.mem.read_u8(D + 0xCBB);
                fx.mem.write_u8(p + 0xCEC, a);
            }
        }
    }
    if fx.mem.i32(D + 0x14EC) == 0 {
        fx.approach(D + 0x230, 10.0, 1.0);
        fx.step_down(D + 0x2A7, 0, 0x10);
        fx.step_down(D + 0x2E7, 0, 8);
        fx.step_down(D + 0x77, 0, 5);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x14E4, m);
    let left = fx.mem.i32(D + 0x14E0) - 1;
    fx.mem.set32(D + 0x14E0, left);
    if m == 0x146 {
        2
    } else if m == 0x12C {
        1
    } else if (0x136..0x146).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
