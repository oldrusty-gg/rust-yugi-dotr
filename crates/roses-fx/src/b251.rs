//! BMODULE b251 (card 245), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E054D0, data area a0 = 0x1ED0000) [F] (helper B5, 31
//! Oct 2026): b368's program (s3 = D, s0 = the OT) for the attacker's
//! nodes 0x64 / 0x60 / 0x56. On updates 7, 45 and 79 thirty bouncing
//! sprites thrown in a fan from node 0x56 (+0x7B4); on 51 an arc pair
//! (+0xC8) from node 0x64 to node 0x60 and a bead-trail emitter on node
//! 0x64 (+0x1BD4, `emit::bead_trails_b`, beads at +0x1C50); on 57 at the
//! target the fade (+0x1B64), three sheets (+0x17A4), a flash (+0x768), a
//! ribbon (+0x1B6C) and a short camera shake; the stage light pulsed by
//! 0x1031C0 (+0xA0). Returns 1 on update 47, 6 on 57..77, 2 on 78, 8 on
//! 120 (and at once for -2 / -3).
//!
//! Library (record addresses): as b368 [`crate::b368`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x2A10;
const SPRITES: u32 = D + 0x7B4;

/// The entry 0x1E054D0 (a0 = D, a1 = param; s3 = D, s0 = the OT, s1 =
/// param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x2B10);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..3 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x64, 1), (0x10, 0x60, 0), (0x14, 0x56, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x72);
    rand_bytes(fx, BYTES, 0x100, 0x01E0_560C, [Some(OT), Some(3), None]);
    0
}

/// (rand >> 4) % `m` for a power of two (masks, LO / HI untouched).
fn rem_pow2(fx: &mut Fx, ra: u32, m: i32) -> i32 {
    (fx.rand_from(OT, ra) >> 4).wrapping_rem(m)
}

/// Updates 7 / 45 / 79 [F 0x1E05688]: thirty sprites from node +0x14 in a
/// fan (12 k degrees, speed 0.5 +- 0.2, rise 0.3).
fn spray(fx: &mut Fx, sp: u32) {
    let ot = u64::from(OT);
    for k in 0..0x1Ei32 {
        let i = first_free(fx, SPRITES, 0x44, 0x3C);
        if i < 0 {
            continue;
        }
        let x = fx.rand_from(OT, 0x01E0_56BC) >> 4;
        let b = SPRITES + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, x & 3, 0xFF);
        let x = fx.rand_from(OT, 0x01E0_56F0) >> 4;
        fx.mem.set8(b + 4, (x & 3) << 4);
        for (o, v) in [(8u32, 1u8), (0xB, 1), (0xA, 5)] {
            fx.mem.write_u8(b + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_5720) >> 4;
        fx.mem.set8(b + 9, x & 3);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x60)] {
            fx.mem.write_u8(b + o, v);
        }
        let m = rem_pow2(fx, 0x01E0_5750, 2);
        fx.mem.setf(b + 0x10, ee::add(6.0, ee::div(ee::from_i32(m * 200 - 0xC8), 100.0)));
        let m = rem_pow2(fx, 0x01E0_57C0, 4);
        fx.mem.setf(b + 0x14, ee::add(13.0, ee::div(ee::from_i32(m * 200 - 0x190), 100.0)));
        fx.mem.write_u32(b + 0x18, 0xBCA3_D70A);
        fx.mem.write_u32(b + 0x1C, 0);
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0x14);
        node_position_saving(fx, h, n, sp + 0x70, ot, Some(k as u64), 0x01E0_584C);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        let deg = ee::mul(12.0, ee::from_i32(k));
        let x = fx.rand_from(OT, 0x01E0_5888) >> 4;
        let (_, m) = fx.div(x, 0x28);
        let speed = ee::add(0.5, ee::div(ee::from_i32(m - 0x14), 100.0));
        let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), deg), 180.0);
        let c = libm_cosf(fx, a);
        fx.mem.setf(b + 0x2C, ee::mul(speed, c));
        let s = fx.libm_sinf(a);
        fx.mem.setf(b + 0x34, ee::mul(speed, s));
        fx.mem.write_u32(b + 0x30, 0x3E99_999A);
    }
}

/// Update 51 [F 0x1E05954]: the arc pair (+0xC8) and the bead-trail
/// emitter (+0x1BD4) on node +0xC.
fn arcs_start(fx: &mut Fx, s1: u64) {
    let g = D + 0xC8;
    set_counter4(fx, g, 0x24, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x128);
    fx.mem.write_u8(g + 0x30, 0x64);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    fx.mem.write_u8(g + 0x34, 0);
    fx.mem.set16(g + 0x36, 0);
    fx.mem.write_u32(g + 0x40, 0x4120_0000);
    for (k, v) in [(0x47u32, 0xC0u8), (0x44, 0xC0), (0x45, 0xC0), (0x46, 0xC0), (0x4A, 0x10), (0x49, 0x80), (0x48, 0x10)] {
        fx.mem.write_u8(g + k, v);
    }
    let mut k = 0u32;
    while k < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * k, -1);
        fx.mem.set16(g + 0xC + 2 * k, -1);
        k += 1;
    }
    let e = D + 0x1BD4;
    set_counter4(fx, e, 9, 4, 0xFF);
    fx.mem.write_u8(e + 4, 0);
    fx.mem.write_u8(e + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(e + 6, b);
    fx.mem.write_u32(e + 8, D + 0x1C50);
    for (o, v) in [(0x10u32, 0x28u8), (0x12, 0xA), (0x11, 0x1C), (0x13, 0), (0x16, 2), (0x14, 3)] {
        fx.mem.write_u8(e + o, v);
    }
    fx.mem.write_u32(e + 0x18, 0x3ECC_CCCD);
    fx.mem.write_u32(e + 0x1C, 0x3ECC_CCCD);
    fx.mem.write_u32(e + 0x20, 0x4248_0000);
    fx.mem.write_u32(e + 0x24, 0xBF00_0000);
    fx.mem.write_u32(e + 0xC, BYTES);
    for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
        fx.mem.write_u8(e + o, 0x80);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, e + 0x3C, u64::from(OT), Some(s1), 0x01E0_5AC4);
    copy_v4(fx, e + 0x3C, e + 0x4C);
}

/// Update 57 [F 0x1E05AEC]: at the target (sp + 0x60) the fade, three
/// stacked turning sheets, a flash, a ribbon and the short shake.
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
    set_counter4(fx, D + 0x1B64, 0xA, 0, 0xFF);
    fx.mem.write_u8(D + 0x1B68, 1);
    for k in 0..3u32 {
        let r = D + 0x17A4 + 0x3C * k;
        set_counter4(fx, r, 0x1C, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 2), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        let dy = ee::add(f32::from_bits(0xBF19_999A), ee::mul(f32::from_bits(0x3F19_999A), ee::from_i32(k as i32)));
        let y = ee::add(fx.mem.f(r + 0x30), dy);
        fx.mem.setf(r + 0x30, y);
        fx.mem.write_u32(r + 0x10, 0x3FC0_0000);
        fx.mem.write_u32(r + 0x14, 0x3F19_999A);
        fx.mem.write_u32(r + 0x18, if k == 1 { 0x4020_0000 } else { 0x3FE6_6666 });
        fx.mem.set32(r + 0x24, 0x3C00);
        if fx.mem.i32(D + 0x30) != 0x8000 {
            let v = fx.mem.i32(r + 0x24).wrapping_neg();
            fx.mem.set32(r + 0x24, v);
        }
    }
    let r = D + 0x768;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x40), (9, 0x10), (8, 0x80), (0xF, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, 0x4248_0000);
    fx.mem.write_u32(r + 0x14, 0x4220_0000);
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, at, r + 0x1C);
    let r = D + 0x1B6C;
    set_counter4(fx, r, 0x18, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4000_0000);
    fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_5D44);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, at, r + 0x28);
    fx.mem.set32(r + 0x58, 0x40);
    fx.mem.set32(r + 0x5C, 0x80);
    fx.mem.set32(r + 0x60, 0x100);
    // The shake's count is the 0x20 left in a1 by the ribbon's +0x23
    // store [F 0x1E05D78].
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x4080_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_5E18);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let ot = u64::from(OT);
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, ot, 0x01E0_5634);
    // s1: the parameter (64-bit) unless a case's loop ran.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0x78 => return 8,
        0x39 => {
            strike(fx, sp);
            s1 = 3;
        }
        0x33 => arcs_start(fx, s1),
        0x4F | 0x2D | 7 => {
            spray(fx, sp);
            s1 = 0x1E;
        }
        _ => {}
    }
    shake_saving(fx, D + 0x40, ot, 0x01E0_5E38, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5E48, [Some(ot), Some(s1)]);
    blib::arc::arcs(fx, 0x01E0_0410, D + 0xC8, OT, 1);
    blib::emit::bead_trails_b(fx, 0x01E0_50C0, D + 0x1BD4, 1);
    blib::beam::beads(fx, 0x01E0_42F0, D + 0x1C50, OT, 0x28);
    blib::star::flash(fx, 0x01E0_2100, D + 0x768, OT, 1);
    blib::sprite::bounce(fx, SPRITES, OT, 0x3C);
    blib::sheet::sheets(fx, 0x01E0_2CB0, D + 0x17A4, OT, 0x10);
    blib::fade::fade(fx, D + 0x1B64, OT);
    blib::streak::ribbon(fx, 0x01E0_3760, D + 0x1B6C, OT, 1);
    let n = fx.mem.u16(D);
    if (0x39..0x4E).contains(&n) {
        6
    } else if n == 0x4E {
        2
    } else if n == 0x2F {
        1
    } else {
        0
    }
}
