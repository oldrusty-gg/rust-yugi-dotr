//! BMODULE b265 (card 204), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04EF0, data area a0 = 0x1ED0000) [F] (helper D8,
//! 31 Oct 2026): b078's family without the streaks (s0 = D, s1 = the OT,
//! s2 the parameter then loop counters). Node bursts (+0xA4, motes +0xE8,
//! two specs) and the object ops of the table 0x1E059D0 every update; on
//! update 86 a radiating record (+0xFE8) written but never drawn (no
//! `streaks` in this record); on 135 the fade (+0x733C), 300 bouncing
//! sprites (+0x238C), four bands (+0x20AC), a wide flash (+0x1020) and a
//! camera shake at the target; on even updates 135..153 a ribbon there
//! (+0x106C). Frame rays (+0xFB8) stepped, never set. Returns 1 on update
//! 125, 2 on 164, 6 on 135..163, 8 on 220 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `frame_rays` 0x1E015F0, `flash`
//! 0x1E02010, `ribbon` 0x1E02BC0, `bands` 0x1E03750, `bounce` 0x1E03F40,
//! `fade` 0x1E049B0 [`crate::blib`].
use crate::b035::{light_ramp, object_ops};
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x7344;
/// The object-op table (rodata, 0x14-byte entries up to a 0 start).
const OPS: u32 = 0x01E0_59D0;

/// The entry 0x1E04EF0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x60);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x7450);
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
    for (at, node, clear) in [(0xCu32, 0x14, 1), (0x10, 0xC, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): specs 0x1E059B8 / 0x1E059C0, motes +0xE8 /
    // +0x7E8.
    for (dst, src) in [(0xCCu32, 0xCu32), (0xCD, 0x10)] {
        let b = fx.mem.read_u8(D + src);
        fx.mem.write_u8(D + dst, b);
    }
    fx.mem.write_u32(D + 0xAC, 0x01E0_59B8);
    fx.mem.write_u32(D + 0xB0, 0x01E0_59C0);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x20);
    fx.mem.write_u32(D + 0xA8, D + 0x7E8);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    fx.mem.write_u16(D + 0xD8, 0);
    fx.mem.write_u32(D + 0xDC, 0x430C_0000);
    fx.mem.write_u32(D + 0xE0, 0x42A0_0000);
    fx.mem.write_u32(D + 0xE4, 0x4080_0000);
    fx.mem.set16(D + 0xA0, 0x81);
    fx.mem.set16(D + 0xA2, 0xD6);
    // The entry's s0 = D, s1 = the OT, s2 = 3 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5074, [Some(D), Some(OT), Some(3)]);
    0
}

/// Update 86 [F 0x1E050CC]: a radiating record (+0xFE8) from node +0x1C,
/// never drawn here.
fn radiate_start(fx: &mut Fx) {
    let r = D + 0xFE8;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (9, 2), (0xA, 0xFF), (8, 0x12), (0xB, 0xA)] {
        fx.mem.write_u8(r + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(r + 0xC, h);
    for (o, v) in [(0x16u32, 0x80u8), (0x15, 0x80), (0x14, 0x80), (0x17, 0x10), (0x12, 0x80), (0x11, 0x80), (0x10, 0x80), (0x13, 0x40), (5, 1)] {
        fx.mem.write_u8(r + o, v);
    }
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(r + 6, b);
}

/// rand with the entry's s0 (D) from `ra`, (x >> 4) % m.
fn rand_mod(fx: &mut Fx, ra: u32, m: i32) -> i32 {
    let x = fx.rand_from(D, ra) >> 4;
    fx.div(x, m).1
}

/// Update 135 [F 0x1E05154]: at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x50;
    set_counter4(fx, D + 0x733C, 0xC, 0, 0xFF);
    fx.mem.write_u8(D + 0x7340, 1);
    for _ in 0..0x12C {
        let i = first_free(fx, D + 0x238C, 0x44, 0x12C);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(D, 0x01E0_51A8) >> 4) & 0xF;
        let b = D + 0x238C + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, delay, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 1), (0xB, 2), (0xA, 0), (9, 3)] {
            fx.mem.write_u8(b + o, v);
        }
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(b + o, 0x80);
        }
        let h = rand_mod(fx, 0x01E0_5214, 5);
        let f = ee::div(ee::from_i32(h * 200 - 0x1F4), 100.0);
        fx.mem.setf(b + 0x10, ee::add(40.0, f));
        fx.mem.write_u32(b + 0x14, 0);
        fx.mem.write_u32(b + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(b + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        let h = rand_mod(fx, 0x01E0_52AC, 0xC8);
        fx.mem.setf(b + 0x2C, ee::div(ee::from_i32(h - 0x64), 100.0));
        let h = rand_mod(fx, 0x01E0_52F8, 0xC8);
        let f = ee::div(ee::from_i32(h - 0x64), 100.0);
        fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), f));
        let h = rand_mod(fx, 0x01E0_5354, 0xC8);
        fx.mem.setf(b + 0x34, ee::div(ee::from_i32(h - 0x64), 100.0));
    }
    for k in 0..4u32 {
        let r = D + 0x20AC + 0x5C * k;
        set_counter4(fx, r, 0x28, (2 * k) as i32, 0xFF);
        for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 1), (8, 2), (9, 2), (0xA, 6), (0xB, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x40A0_0000);
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        fx.mem.write_u32(r + 0x1C, 0x3E99_999A);
        fx.mem.write_u32(r + 0x20, 0x3F00_0000);
        for (o, v) in [(0x25u32, 0x60u8), (0x26, 0x60), (0x24, 0xC0), (0x29, 0x60), (0x2A, 0x60), (0x28, 0xC0), (0x27, 0x10), (0x2B, 0x30)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        fx.mem.write_u32(r + 0x3C, 0x4000);
    }
    let r = D + 0x1020;
    set_counter4(fx, r, 0x28, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0xC0), (9, 0x20), (8, 0x40)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0);
    fx.mem.write_u32(r + 0x10, 0x43C8_0000);
    copy_v4(fx, at, r + 0x1C);
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3FB3_3333);
    fx.mem.write_u32(s + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4270_0000);
    fx.mem.write_u32(s + 0x50, 0x4080_0000);
    copy_v4(fx, at, D + 0x60);
}

/// Even updates 135..153 [F 0x1E055E4]: a ribbon at the target.
fn ribbon_start(fx: &mut Fx, sp: u32) {
    let i = first_free(fx, D + 0x106C, 0x68, 0x1E);
    if i < 0 {
        return;
    }
    let r = D + 0x106C + 0x68 * i as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4060_0000);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    for (o, v) in [(8u32, 0x50u8), (5, 0), (0xA, 1), (0xB, 2), (0xC, 5)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(D, 0x01E0_5678);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    fx.mem.write_u8(r + 0x23, 0);
    fx.mem.write_u8(r + 0x27, 0x60);
    for (o, ra) in [(0x58u32, 0x01E0_56C4u32), (0x5C, 0x01E0_56DC), (0x60, 0x01E0_56F4)] {
        let x = fx.rand_from(D, ra) >> 4;
        fx.mem.set32(r + o, x & 0x7F);
    }
    copy_v4(fx, sp + 0x50, r + 0x28);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(D), 0x01E0_509C);
    match fx.mem.u16(D) {
        0xDC => return 8,
        0x87 => strike(fx, sp),
        0x56 => radiate_start(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x87..0x9A).contains(&n) && n & 1 == 0 {
        ribbon_start(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_5734, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5744, [Some(u64::from(D)), Some(u64::from(OT))]);
    let n = fx.mem.u16(D);
    object_ops(fx, OPS, n);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x20);
    // Never set: only its countdown runs (a3 not reached).
    blib::window::frame_rays(fx, 0x01E0_15F0, D + 0xFB8, OT, 1, 0);
    blib::star::flash(fx, 0x01E0_2010, D + 0x1020, OT, 1);
    blib::streak::ribbon(fx, 0x01E0_2BC0, D + 0x106C, OT, 0x28);
    blib::band::bands(fx, 0x01E0_3750, D + 0x20AC, OT, 8);
    blib::sprite::bounce(fx, D + 0x238C, OT, 0x12C);
    blib::fade::fade(fx, D + 0x733C, OT);
    // fade's saved s6 (the runner's 0) and ra (as b449) [run].
    fx.saved(sp - 0xF0, &[(0x70, 0), (0x80, 0x01E0_58A8)]);
    let n = fx.mem.u16(D);
    if (0x87..0xA4).contains(&n) {
        6
    } else if n == 0xA4 {
        2
    } else if n == 0x7D {
        1
    } else {
        0
    }
}
