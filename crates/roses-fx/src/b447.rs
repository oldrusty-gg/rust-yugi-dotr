//! BMODULE b447 (card 480), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02E90, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): b234's family. At the attacker's node 0x4C a sprite burst
//! (+0x2230, 64 sprites thrown out at 5 and growing 3 % a step, each
//! rethrown after 8 steps) all along, a disc (+0x21B0) from update 157
//! and b489's spark column (+0xE0, rising 8 a step) fading in from update
//! 157; the stage light down to 0x20; on updates 179..285 the target's
//! added colour (0x1F7790) flickering at random. Returns 1 on update 168,
//! 6 on 178..285, 2 on 286, 8 after 305. (A second spark group +0x1110 is
//! set up and never drawn.)
//!
//! Library: `spawn::scatter` (0x1E00120), `trail::sparks` (0x1E002E0)
//! [`crate::blib`], `sprites` (0x1E01040), `disc` (0x1E02930)
//! [`crate::fxlib`].
use crate::b057::spark_place;
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E02E90.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

/// A white spark group at `g` (256 sparks at g + 0x30, cell (0, 0, 32,
/// 32), alpha `alpha`, width / length f32 bits), scattered (life 12,
/// speed 3).
pub(crate) fn sparks_group(fx: &mut Fx, g: u32, tex: u32, flags: u32, alpha: u8, width: u32, length: u32) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, flags);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, alpha)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0x20, width);
    fx.mem.write_u32(g + 0x24, length);
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x20), (0x16, 0x20), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, g + 0x30);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 8);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
}

/// Burst sprite `p` (D + 16 k, fields at +0x2290) thrown in a random
/// direction of length 5 (0x1043F0 into sp + 0x70).
fn burst_throw(fx: &mut Fx, sp: u32, p: u32) {
    fx.random_dir(sp + 0x70, 5.0);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
        fx.mem.write_u32(p + 0x2290 + 4 * c, w);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x26C0);
    for x in [0u32, 0x2140, 0x70, 0x21C0] {
        fx.xform_init(D + x);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x4C, 1);
    fx.mem.set32(D + 0x26B0, r);
    fx.mem.set32(D + 0x26A0, 0x132);
    fx.mem.set32(D + 0x26A4, 1);
    fx.mem.set32(D + 0x26A8, param);
    for k in [0x26B4u32, 0x26AF, 0x26AC, 0x26AD, 0x26AE] {
        fx.mem.write_u8(D + k, 0x80);
    }
    // The disc (+0x21B0), radius 15.
    let g = D + 0x21B0;
    fx.mem.write_u32(g, 0x8_0000);
    for (k, x) in [(6u32, 0x40u8), (5, 0x60), (4, 0x80), (7, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 8, 0x4170_0000);
    fx.mem.set16(g + 0xC, 0x24);
    fx.mem.set16(g + 0xE, 0);
    // a1 is still 1 from the counter's store [F 0x1E02F14].
    fx.texture5(sp + 0xA0, 1);
    // The burst (+0x2230): 64 sprites at +0x2290, size 15.
    let g = D + 0x2230;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x4800_0003);
    for (k, x) in [(0xAu32, 0x50u8), (9, 0x60), (8, 0x80), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0xC, 0x4170_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x2290);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4170_0000);
    fx.mem.write_u32(g + 0x50, 0x4120_0000);
    halves(fx, g, &[(0x5A, 0x40), (0x5C, 8), (0x5E, 0x10)]);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x228A) {
        let p = D + 16 * k as u32;
        burst_throw(fx, sp, p);
        let r = fx.rand_from(D, 0x01E0_3048);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0x229E, h.wrapping_neg());
        fx.mem.write_u8(p + 0x229C, 0x80);
        let r = fx.rand_from(D, 0x01E0_3070);
        fx.mem.set8(p + 0x229D, r % 8);
        k += 1;
    }
    fx.texture5(sp + 0xA0, 0);
    let tex = fx.mem.read_u32(sp + 0xA0);
    sparks_group(fx, D + 0xE0, tex, 0x6000_0003, 0, 0x3E4C_CCCD, 0x41B0_0000);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x10C) {
        spark_place(fx, k as u32, 0x7D0, [0x01E0_315C, 0x01E0_3178, 0x01E0_31A4]);
        let e = fx.mem.read_u32(D + 0xEC).wrapping_add(16 * k as u32);
        fx.mem.write_u8(e + 0xC, 0);
        k += 1;
    }
    let tex = fx.mem.read_u32(sp + 0xA0);
    sparks_group(fx, D + 0x1110, tex, 3, 0, 0x3E4C_CCCD, 0x4120_0000);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x26A4);
    let node = |fx: &mut Fx, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x26B0);
        fx.node_position(h, s, out);
    };
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x2690);
    node(fx, D + 0x90);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x80);
    node(fx, sp + 0x90);
    fx.look_angles(sp + 0x80, sp + 0x90, D + 0x80);
    node(fx, D + 0x21E0);
    node(fx, D + 0x2160);
    // s0 = D, s1 = the OT, s2 = sp + 0x90, s3 the caller's.
    let regs = [Some(D), Some(OT), Some(sp + 0x90), None];
    fx.xform_world_saving(D, 0x01E0_3370, regs);
    fx.xform_billboard_saving(D + 0x2140, 0x01E0_3378, regs);
    fx.xform_world_saving(D + 0x70, 0x01E0_3380, regs);
    fx.xform_world_saving(D + 0x21C0, 0x01E0_3388, regs);
    if n(fx) >= 0x11E {
        fx.step_up(D + 0x26B4, 0x80, 4);
    } else {
        fx.step_down(D + 0x26B4, 0x20, 4);
    }
    let light = fx.mem.u8(D + 0x26B4);
    fx.set_stage_light(light);
    if (0xB3..0x11E).contains(&n(fx)) {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, D + 0x26AC, [D, OT], 0x01E0_33FC);
        for (at, ra) in [(0x26AEu32, 0x01E0_3404), (0x26AD, 0x01E0_3420), (0x26AC, 0x01E0_343C)] {
            let r = fx.rand_from(D, ra);
            fx.mem.set8(D + at, r % 128);
        }
    }
    if n(fx) == 0x11E {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, 0, [D, OT], 0x01E0_3478);
    }
    if n(fx) >= 0x9D {
        fx.disc(D + 0x21B0, D + 0x2140);
        // The disc's fp / ra saves, under the sprites' TEX0 / TEX1 slots.
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_3498)]);
        if n(fx) >= 0x11E {
            fx.step_down(D + 0x21B7, 0, 0x10);
        } else {
            fx.step_up(D + 0x21B7, 0xFF, 0x10);
        }
    }
    fx.sprites(D + 0x2230, D + 0x21C0);
    let m = n(fx);
    if m >= 0x11E {
        fx.step_down(D + 0x223B, 0, 8);
    } else if m >= 0x96 {
        fx.step_up(D + 0x223B, 0x80, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x228A) {
        let p = D + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x229E) >= 0 {
            let s = f32::from_bits(0x3F83_D70A);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 0x2290 + 4 * c), s);
                fx.mem.setf(p + 0x2290 + 4 * c, x);
            }
            fx.step_down(p + 0x229C, 0, 8);
        }
        if fx.mem.i16(p + 0x229E) < 8 {
            continue;
        }
        burst_throw(fx, sp, p);
        fx.mem.set16(p + 0x229E, 0);
        fx.mem.write_u8(p + 0x229C, 0x80);
    }
    blib::trail::sparks(fx, OT, D + 0xE0, D + 0x70);
    let m = n(fx);
    if m >= 0x11F {
        fx.step_down(D + 0xEB, 0, 8);
    } else if (0x9D..0x11E).contains(&m) {
        fx.step_up(D + 0xEB, 0x80, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x10C) {
        let p = D + 16 * k as u32;
        if fx.mem.i16(p + 0x11E) >= 0 {
            let y = ee::add(fx.mem.f(p + 0x114), 8.0);
            fx.mem.setf(p + 0x114, y);
            fx.step_up(p + 0x11C, 0x80, 0x10);
            if !ee_le(fx.mem.f(p + 0x114), 200.0) {
                spark_place(fx, k as u32, 0x1E, [0x01E0_36B0, 0x01E0_36CC, 0x01E0_36F8]);
                fx.mem.write_u8(p + 0x11C, 0);
                fx.mem.set16(p + 0x11E, 0);
            }
        }
        k += 1;
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x26A4, m);
    let left = fx.mem.i32(D + 0x26A0) - 1;
    fx.mem.set32(D + 0x26A0, left);
    if m == 0x11E {
        2
    } else if m == 0xA8 {
        1
    } else if (0xB2..0x11E).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
