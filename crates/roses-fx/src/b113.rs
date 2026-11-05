//! BMODULE b113 (card 38), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03F50, data area a0 = 0x1ED0000) [F] (helper B9, 31
//! Oct 2026). A point (+0x210) held at the attacker's node 0x12 pushed out
//! along node 0xF -> 0x12 until update 158, then moving to the target
//! (+0xC20) by 1 / 16 a step; from update 25 at it (three transform
//! records +0 / +0x1F0 / +0xC00): 128 sprites (+0x3A0) until 189, a disc
//! (+0x260), a point-set strip (+0x310 / +0x270) and eight jagged strands
//! (+0x70, 0x30 apart, ends +0xF60) re-aimed when their counter runs out,
//! growing to 1 by 0.05 before update 100; from update 175 a full-window
//! tint (+0xFF8) and from 175 a cloud of 60 squares (+0xC70) spreading by
//! 1.05. Returns 1 on update 164, 6 on 174..189, 3 on 190 (camera slots
//! cleared), 8 after 220.
//!
//! Library (record addresses): `tint` 0x1E00120, `pointquad::setup` /
//! `draw` / `squares` 0x1E00280 / 0x1E005C0 / 0x1E011A0, `disc` 0x1E01780,
//! `sprites` 0x1E01CE0, `strand::draw` 0x1E035D0.
use crate::b028::square_points;
use crate::b079::sub3;
use crate::b142::fstep_up;
use crate::b262::tint;
use crate::blib::{exe::camera_slots45, pointquad, strand};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E03F50 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1000);
    for x in [0u32, 0x1F0, 0xC00] {
        fx.xform_init(D + x);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x12, 1);
    fx.mem.set32(D + 0xFF0, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0xF, 0);
    fx.mem.set32(D + 0xFF4, r);
    fx.mem.set32(D + 0xFE0, 0xDC);
    fx.mem.set32(D + 0xFE4, 1);
    fx.mem.set32(D + 0xFE8, param);
    fx.mem.set32(D + 0xFEC, 0x10);
    for (o, v) in [(0xFF9u32, 0x80u8), (0xFFA, 0x80), (0xFF8, 0xE0), (0xFFB, 0)] {
        fx.mem.write_u8(D + o, v);
    }
    // The sprites (+0x3A0): 128 particles at +0x400, texture 5 / 1.
    fx.texture5(sp + 0xA0, 1);
    let g = D + 0x3A0;
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x7000_0003);
    for o in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + o, 0x80);
    }
    fx.mem.write_u32(g + 0xC, 0x3F00_0000);
    for o in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + o, 0x800);
    }
    for (o, v) in [(0x30u32, 0u16), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)] {
        fx.mem.write_u16(g + o, v);
    }
    fx.mem.write_u32(g + 0x40, D + 0x400);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4170_0000);
    fx.mem.write_u16(g + 0x5A, 0x80);
    fx.mem.write_u16(g + 0x5C, 8);
    fx.mem.write_u16(g + 0x5E, 0);
    let mut k = 0u32;
    while (k as i32) < fx.mem.i16(g + 0x5A) {
        let p = D + 0x400 + 0x10 * k;
        fx.random_dir(sp + 0x70, 4.0);
        for c in 0..3u32 {
            let v = fx.mem.read_u32(sp + 0x70 + 4 * c);
            fx.mem.write_u32(p + 4 * c, v);
        }
        let r = fx.rand();
        let (_, m) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, -m);
        fx.mem.write_u8(p + 0xC, 0x30);
        fx.mem.write_u8(p + 0xD, 0);
        k += 1;
    }
    // The squares (+0xC70): 60 points, half size 0.5.
    fx.texture4(sp + 0xA0, 0xF);
    let g = D + 0xC70;
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x2008_0001);
    for (o, v) in [(0x10u32, 0xB0u16), (0x12, 0x40), (0x14, 0x20), (0x16, 0x20)] {
        fx.mem.write_u16(g + o, v);
    }
    fx.mem.write_u32(g + 0xC, 0x3F00_0000);
    square_points(fx, g);
    for (o, v) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0xE0), (0xB, 0x80)] {
        fx.mem.write_u8(g + o, v);
    }
    // The disc (+0x260): radius 5, 0x24 triangles.
    let g = D + 0x260;
    fx.mem.write_u32(g, 0);
    for (o, v) in [(6u32, 0x80u8), (5, 0x80), (4, 0xE0), (7, 0)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 8, 0x40A0_0000);
    fx.mem.write_u16(g + 0xC, 0x24);
    fx.mem.write_u16(g + 0xE, 0);
    // The point set (+0x270) and its look (+0x310).
    fx.mem.write_u32(D + 0x348, 0x401);
    let g = D + 0x310;
    for (o, v) in [(0u32, 0xB0u16), (2, 0x20), (4, 0x20), (6, 0x20), (8, 0xB0), (0xA, 0x20), (0xC, 0x100), (0xE, 0x100)] {
        fx.mem.write_u16(g + o, v);
    }
    fx.mem.write_u32(g + 0x30, 0x3F80_0000);
    for (o, v) in [(0x36u32, 0x80u8), (0x35, 0x80), (0x34, 0xE0), (0x37, 0)] {
        fx.mem.write_u8(g + o, v);
    }
    for (o, v) in [(0x48u32, 0u16), (0x4A, 0), (0x4C, 0), (0x4E, 1), (0x50, 0), (0x52, 0x3E7)] {
        fx.mem.write_u16(g + o, v);
    }
    fx.mem.write_u32(D + 0x278, 1);
    let t = fx.mem.read_u32(sp + 0xA0);
    pointquad::setup(fx, 0x01E0_0280, D + 0x310, D + 0x270, t, 8.0);
    // The camera slots' record (+0x370).
    fx.mem.write_u32(D + 0x374, 0x3F00_0000);
    fx.mem.write_u32(D + 0x378, 0x3F40_0000);
    fx.mem.write_u32(D + 0x37C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x370, 0x3F80_0000);
    fx.mem.write_u32(D + 0x390, 0);
    fx.mem.write_u32(D + 0x394, 0x4220_0000);
    fx.mem.write_u32(D + 0x398, 0);
    // Eight strands (+0x70, 0x30 apart), texture 5 / 0, ends +0xF60.
    fx.texture5(sp + 0xA0, 0);
    for k in 0..8u32 {
        let g = D + 0x70 + 0x30 * k;
        let t = fx.mem.read_u32(sp + 0xA0);
        fx.mem.write_u32(g, t);
        fx.mem.write_u32(g + 4, 0);
        for (o, v) in [(0x10u32, 0x80u16), (0x12, 0x40), (0x14, 0x20), (0x16, 0x40)] {
            fx.mem.write_u16(g + o, v);
        }
        for (o, v) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, 0x40)] {
            fx.mem.write_u8(g + o, v);
        }
        fx.mem.write_u16(g + 0xC, 0x10);
        fx.mem.set16(g + 0x28, -(k as i32 + 1));
        fx.mem.write_u16(g + 0xE, 0x14);
        fx.mem.write_u32(g + 0x20, 0);
        fx.mem.write_u32(g + 0x24, 0);
        fx.random_dir(D + 0xF60 + 0x10 * k, 6.0);
    }
    for o in [0x1F8u32, 0x1F4, 0x1F0, 0xC08, 0xC04, 0xC00] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0xFE4);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0xC20);
    let near = n(fx) < 0x9F;
    if near {
        for (slot, out) in [(0xFF0u32, D + 0x210), (0xFF0, sp + 0x80), (0xFF4, sp + 0x90)] {
            let h = fx.self_handle();
            let s = fx.mem.i32(D + slot);
            fx.node_position(h, s, out);
        }
        sub3(fx, sp + 0x80, sp + 0x90);
        fx.add3(D + 0x210, sp + 0x80);
    } else {
        let m = fx.mem.i32(D + 0xFEC);
        fx.approach_by(D + 0x210, D + 0xC20, m);
        if m != 0 {
            fx.mem.set32(D + 0xFEC, m - 1);
        }
    }
    for c in 0..3u32 {
        let v = fx.mem.read_u32(D + 0x210 + 4 * c);
        fx.mem.write_u32(D + 0x380 + 4 * c, v);
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x90 (the node branch) or the
    // parameter (-1, 64 bits), s3 the runner's.
    let s2 = if near { u64::from(sp + 0x90) } else { u64::MAX };
    let regs = [Some(D), Some(OT), None, Some(0)];
    fx.xform_world_saving(D, 0x01E0_4520, regs);
    fx.saved(sp - 0xD0, &[(0x40, s2)]);
    for (x, ra) in [(0x1F0u32, 0x01E0_4528), (0xC00, 0x01E0_4530)] {
        fx.xform_billboard_saving(D + x, ra, regs);
        fx.saved(sp - 0xF0, &[(0x40, s2)]);
    }
    if n(fx) >= 0xAE {
        tint(fx, D + 0xFF8);
        if n(fx) < 0xBE {
            fx.step_up(D + 0xFFB, 0xFF, 0x10);
        } else {
            fx.step_down(D + 0xFFB, 0, 0x20);
        }
    }
    if n(fx) >= 0xAF {
        if ee_lt(fx.mem.f(D + 0xC00), 0.5) {
            let v = ee::add(fx.mem.f(D + 0xC08), f32::from_bits(0x3E4C_CCCD));
            for o in [0xC08u32, 0xC04, 0xC00] {
                fx.mem.setf(D + o, v);
            }
        }
        for o in [0x68u32, 0x64, 0x60] {
            fx.mem.write_u32(sp + o, 0x3F86_6666);
        }
        pointquad::squares(fx, OT, D + 0xC70, D + 0xC00, sp + 0x60);
        fx.step_down(D + 0xC7B, 0, 8);
    }
    if n(fx) >= 0x19 {
        if n(fx) < 0xBE {
            fx.sprites(D + 0x3A0, D + 0x1F0);
        }
        fx.disc(D + 0x260, D + 0x1F0);
        pointquad::draw(fx, OT, D + 0x310, D + 0x270, D + 0x1F0);
        if n(fx) < 0xBE {
            camera_slots45(fx, D + 0x370);
            // 0x1F7E20's frame (s0 = D, ra): the strands' missing-texture
            // TEX0 / TEX1 [run].
            fx.saved(sp - 0x20, &[(0, u64::from(D)), (0x10, 0x01E0_4678)]);
        }
        if n(fx) < 0x64 {
            fx.approach(D + 0x1F0, 1.0, f32::from_bits(0x3D4C_CCCD));
            fx.step_up(D + 0x347, 0x80, 0x10);
            fx.step_up(D + 0x267, 0xFF, 0x20);
            fstep_up(fx, D + 0x398, 20.0, 2.0);
        }
        for o in [0x68u32, 0x64, 0x60] {
            fx.mem.write_u32(sp + o, 0);
        }
        for k in 0..8u32 {
            let g = D + 0x70 + 0x30 * k;
            let e = D + 0xF60 + 0x10 * k;
            strand::draw(fx, 0x01E0_35D0, OT, g, D + 0x1F0, sp + 0x60, e);
            if fx.mem.i16(g + 0x28) == 0 {
                fx.random_dir(e, 6.0);
            }
        }
    }
    if fx.mem.i32(D + 0xFEC) == 0 {
        fx.approach(D + 0x1F0, 10.0, 1.0);
        fx.step_down(D + 0x347, 0, 8);
        fx.step_down(D + 0x267, 0, 0x10);
        for k in 0..8u32 {
            fx.step_down(D + 0x7B + 0x30 * k, 0, 4);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0xFE4, m);
    let left = fx.mem.i32(D + 0xFE0) - 1;
    fx.mem.set32(D + 0xFE0, left);
    if m == 0xBE {
        camera_slots45(fx, 0);
        3
    } else if m == 0xA4 {
        1
    } else if (0xAE..0xBE).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
