//! BMODULE b234 (card 181), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E010D0, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): b489's spark column (+0xE0, green, rising 8 a step) round
//! the attacker's node 0x29, and from update 60 a second spark burst
//! (+0x1110) on the node thrown out again as each spark dies, the frame
//! quad (colour +0x2230, alpha 0x60 fading after 236) and the stage light
//! down to 0x20; on updates 92..235 the target's added colour (0x1F7790)
//! flickering at random. Returns 1 on update 81, 6 on 91..235, 2 on 236,
//! 8 after 255.
//!
//! Library: the frame quad (0x1E00080, [`crate::framegs`]),
//! `spawn::scatter` (0x1E001B0), `trail::sparks` (0x1E00370)
//! [`crate::blib`].
use crate::b057::spark_place;
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E010D0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

/// A spark group at `g` (256 sparks at g + 0x30, cell (0, 0, 32, 16)).
pub(crate) fn sparks_group(fx: &mut Fx, g: u32, tex: u32, flags: u32, alpha: u8, width: u32, length: u32) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, flags);
    for (k, x) in [(0xAu32, 0x70u8), (9, 0x80), (8, 0x70), (0xB, alpha)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0x20, width);
    fx.mem.write_u32(g + 0x24, length);
    for (k, x) in [(0x10u32, 0), (0x12, 0), (0x14, 0x20), (0x16, 0x10), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u32(g + 0xC, g + 0x30);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 8);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2250);
    for x in [0u32, 0x2140, 0x70, 0x21B0] {
        fx.xform_init(D + x);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x29, 1);
    fx.mem.set32(D + 0x2244, r);
    fx.mem.set32(D + 0x2234, 0x100);
    fx.mem.set32(D + 0x2238, 1);
    fx.mem.set32(D + 0x223C, param);
    for k in [0x2248u32, 0x2243, 0x2240, 0x2241, 0x2242, 0x2230, 0x2231, 0x2232] {
        fx.mem.write_u8(D + k, 0x80);
    }
    fx.mem.write_u8(D + 0x2233, 0x60);
    fx.texture5(sp + 0xB0, 0);
    let tex = fx.mem.read_u32(sp + 0xB0);
    sparks_group(fx, D + 0xE0, tex, 0x6000_0003, 0x80, 0x3ECC_CCCD, 0x41B0_0000);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x10C) {
        spark_place(fx, k as u32, 0x7D0, [0x01E0_1254, 0x01E0_1270, 0x01E0_129C]);
        let e = fx.mem.read_u32(D + 0xEC).wrapping_add(16 * k as u32);
        fx.mem.write_u8(e + 0xC, 0);
        k += 1;
    }
    let tex = fx.mem.read_u32(sp + 0xB0);
    sparks_group(fx, D + 0x1110, tex, 3, 0, 0x3E4C_CCCD, 0x4120_0000);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2238);
    let node = |fx: &mut Fx, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x2244);
        fx.node_position(h, s, out);
    };
    let first = n(fx) == 0x3C;
    if first {
        let t = fx.target_handle();
        fx.unit_position(t, D + 0x2220);
        node(fx, D + 0x90);
        let t = fx.target_handle();
        fx.unit_position(t, sp + 0x90);
        node(fx, sp + 0xA0);
        fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x80);
    }
    node(fx, D + 0x21D0);
    node(fx, D + 0x2160);
    // s0 = D, s1 = the OT, s2 = sp + 0xA0 after the set-up update, else
    // the parameter (-1, sign-extended: not modelled), s3 the caller's.
    let regs = [Some(D), Some(OT), first.then_some(sp + 0xA0), None];
    fx.xform_world_saving(D, 0x01E0_1480, regs);
    fx.xform_billboard_saving(D + 0x2140, 0x01E0_1488, regs);
    fx.xform_world_saving(D + 0x70, 0x01E0_1490, regs);
    fx.xform_world_saving(D + 0x21B0, 0x01E0_1498, regs);
    if n(fx) >= 0xEC {
        fx.step_up(D + 0x2248, 0x80, 4);
    } else {
        fx.step_down(D + 0x2248, 0x20, 4);
    }
    let light = fx.mem.u8(D + 0x2248);
    fx.set_stage_light(light);
    if n(fx) >= 0x3C {
        fx.frame_quad(D + 0x2230);
        if n(fx) >= 0xEC {
            fx.step_down(D + 0x2233, 0, 6);
        }
    }
    if (0x5C..0xEC).contains(&n(fx)) {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, D + 0x2240, [D, OT], 0x01E0_1544);
        for (at, ra) in [(0x2242u32, 0x01E0_154C), (0x2241, 0x01E0_1568), (0x2240, 0x01E0_1584)] {
            let r = fx.rand_from(D, ra);
            fx.mem.set8(D + at, r % 128);
        }
    }
    if n(fx) == 0xEC {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, 0, [D, OT], 0x01E0_15C0);
    }
    blib::trail::sparks(fx, OT, D + 0xE0, D + 0x70);
    if n(fx) >= 0xEC {
        fx.step_down(D + 0xEB, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x10C) {
        let p = D + 16 * k as u32;
        if fx.mem.i16(p + 0x11E) >= 0 {
            let y = ee::add(fx.mem.f(p + 0x114), 8.0);
            fx.mem.setf(p + 0x114, y);
            fx.step_up(p + 0x11C, 0x80, 0x10);
            if !ee_le(fx.mem.f(p + 0x114), 200.0) {
                spark_place(fx, k as u32, 0x1E, [0x01E0_1658, 0x01E0_1674, 0x01E0_16A0]);
                fx.mem.write_u8(p + 0x11C, 0);
                fx.mem.set16(p + 0x11E, 0);
            }
        }
        k += 1;
    }
    if n(fx) == 0x3C {
        fx.mem.write_u8(D + 0x111B, 0x80);
    }
    blib::trail::sparks(fx, OT, D + 0x1110, D + 0x21B0);
    if n(fx) >= 0xEC {
        fx.step_down(D + 0x111B, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x113C) {
        let p = D + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x114E) < 0 {
            continue;
        }
        if n(fx) >= 0x3C {
            fx.step_up(p + 0x114C, 0x80, 0x10);
        }
        let s = f32::from_bits(0x3FA6_6666);
        for c in 0..3u32 {
            let x = ee::mul(fx.mem.f(p + 0x1140 + 4 * c), s);
            fx.mem.setf(p + 0x1140 + 4 * c, x);
        }
        if fx.mem.i16(p + 0x114E) < 0x11 {
            continue;
        }
        fx.random_dir(sp + 0x80, 1.0);
        for c in [1u32, 2, 0] {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * c);
            fx.mem.write_u32(p + 0x1140 + 4 * c, w);
        }
        let r = fx.rand_from(D, 0x01E0_1820);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0x114E, h.wrapping_neg());
        fx.mem.write_u8(p + 0x114C, 0);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2238, m);
    let left = fx.mem.i32(D + 0x2234) - 1;
    fx.mem.set32(D + 0x2234, left);
    if m == 0xEC {
        2
    } else if m == 0x51 {
        1
    } else if (0x5B..0xEC).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
