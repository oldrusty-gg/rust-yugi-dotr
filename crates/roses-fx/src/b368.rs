//! BMODULE b368 (card 144), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E054D0, data area a0 = 0x1ED0000) [F] (helper B5, 31
//! Oct 2026): b076's "a0 = D" family with s3 = D, s0 = the OT (so rand's
//! and the library calls' saved s0 is the OT). On update 95 an arc pair
//! (+0xC8, `arc::arcs`) between the attacker's nodes 0x41 / 0x37; on 96 at
//! the target eight bead beams (+0x1C50), the fade (+0x1B64), seven
//! stacked sheets (+0x17A4, `sheet::sheets`), a flash (+0x768), a ribbon
//! (+0x1B6C) and a short camera shake; the stage light pulsed by 0x1031C0
//! (+0xA0: from update 86, up on 123). The bead-trail emitter (+0x1BD4,
//! `emit::bead_trails_b`) and the bouncing sprites (+0x7B4) are drawn but
//! never filled. Returns 1 on update 88, 6 on 98..118, 2 on 119, 8 on 129
//! (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `arcs`
//! 0x1E00410, `bounce` 0x1E01690, `flash` 0x1E02100, `sheets` 0x1E02CB0,
//! `fade` 0x1E03220, `ribbon` 0x1E03760, `beads` 0x1E042F0,
//! `bead_trails_b` 0x1E050C0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x2A10;

/// The entry 0x1E054D0 (a0 = D, a1 = param; s3 = D, s0 = the OT, s1 =
/// param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x60);
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
    for (at, node, clear) in [(0xCu32, 0x41, 1), (0x10, 0x37, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 0x56);
    fx.mem.set16(D + 0xA2, 0x7B);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's), s2 the
    // runner's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_55EC, [Some(OT), Some(3), None]);
    0
}

/// Update 95 [F 0x1E05644]: the arc pair (+0xC8) from node +0xC to node
/// +0x10, points at +0x128.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0xC8;
    set_counter4(fx, g, 8, 0, 0xFF);
    fx.mem.write_u8(g + 4, 0xD);
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
    for (k, v) in [(0x47u32, 0xC0u8), (0x44, 0xC0), (0x45, 0xC0), (0x46, 0xC0), (0x4A, 0x80), (0x49, 0), (0x48, 0x10), (0x4E, 0xC0), (0x4D, 0), (0x4C, 0x10), (0x4F, 0xC0)] {
        fx.mem.write_u8(g + k, v);
    }
    fx.mem.write_u32(g + 0x54, 0x4000_0000);
    let mut k = 0u32;
    while k < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * k, -1);
        fx.mem.set16(g + 0xC + 2 * k, -1);
        k += 1;
    }
}

/// Update 96 [F 0x1E05724]: at the target (sp + 0x50) eight bead beams,
/// the fade, seven sheets stacked 2 apart, a flash, a ribbon and the
/// short shake.
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x50;
    let ot = u64::from(OT);
    for k in 0..8i32 {
        let r = D + 0x1C50 + 0x58 * k as u32;
        set_counter4(fx, r, 0x20, k & 0xFF, 0xFF);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x32);
        let x = fx.rand_from(OT, 0x01E0_5780);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xC, 2);
        let x = fx.rand_from(OT, 0x01E0_57A0) >> 4;
        fx.mem.set8(r + 0xB, x & 7);
        fx.mem.write_u32(r + 0x1C, 0x42C8_0000);
        fx.mem.write_u32(r + 0x20, 0x3F80_0000);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x4020_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
    }
    set_counter4(fx, D + 0x1B64, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0x1B68, 1);
    for k in 0..7u32 {
        let r = D + 0x17A4 + 0x3C * k;
        set_counter4(fx, r, 0x1A, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 2), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        let dy = crate::ee::add(-6.0, crate::ee::mul(2.0, crate::ee::from_i32(k as i32)));
        let y = crate::ee::add(fx.mem.f(r + 0x30), dy);
        fx.mem.setf(r + 0x30, y);
        fx.mem.write_u32(r + 0x10, 0x4040_0000);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        fx.mem.write_u32(r + 0x18, if k & 1 != 0 { 0x4040_0000 } else { 0x4000_0000 });
        fx.mem.set32(r + 0x24, 0x4000);
    }
    let r = D + 0x768;
    set_counter4(fx, r, 0x22, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x80), (7, 9), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x40), (9, 0x10), (8, 0x80), (0xF, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, 0x4248_0000);
    fx.mem.write_u32(r + 0x14, 0x4220_0000);
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, at, r + 0x1C);
    let r = D + 0x1B6C;
    set_counter4(fx, r, 0x1E, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4020_0000);
    fx.mem.write_u32(r + 0x1C, 0x420C_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0xB4)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_5A38);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x10), (0x27, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, at, r + 0x28);
    fx.mem.set32(r + 0x58, 0x40);
    fx.mem.set32(r + 0x5C, 0x80);
    fx.mem.set32(r + 0x60, 0x100);
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
    unit_position_saving(fx, t, D + 0x60, ot, 0x01E0_5B10);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let ot = u64::from(OT);
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, ot, 0x01E0_5614);
    // s1: the parameter (64-bit) unless the strike's sheet loop ran.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0x81 => return 8,
        0x60 => {
            strike(fx, sp);
            s1 = 7;
        }
        0x5F => arcs_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, ot, 0x01E0_5B30, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5B40, [Some(ot), Some(s1)]);
    blib::arc::arcs(fx, 0x01E0_0410, D + 0xC8, OT, 1);
    blib::emit::bead_trails_b(fx, 0x01E0_50C0, D + 0x1BD4, 1);
    blib::beam::beads(fx, 0x01E0_42F0, D + 0x1C50, OT, 0x28);
    blib::star::flash(fx, 0x01E0_2100, D + 0x768, OT, 1);
    blib::sprite::bounce(fx, D + 0x7B4, OT, 0x3C);
    blib::streak::ribbon(fx, 0x01E0_3760, D + 0x1B6C, OT, 1);
    blib::sheet::sheets(fx, 0x01E0_2CB0, D + 0x17A4, OT, 0x10);
    blib::fade::fade(fx, D + 0x1B64, OT);
    let n = fx.mem.u16(D);
    if (0x62..0x77).contains(&n) {
        6
    } else if n == 0x77 {
        2
    } else if n == 0x58 {
        1
    } else {
        0
    }
}
