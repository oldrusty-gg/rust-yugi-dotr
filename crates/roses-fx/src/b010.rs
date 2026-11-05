//! BMODULE b010 (card 433), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05D10, data area a0 = 0x1ED0000) [F] (helper D8,
//! 31 Oct 2026): b553's library at the same addresses with its own program
//! (s4 = D, s0 = the OT, s1 the parameter then loop counters). Node bursts
//! (+0xA4, motes +0xE8, four specs) every update; on update 86 the
//! attacker radiating from its node 0x1B (+0xFE8, `radiate::streaks`); on
//! 99 the fade (+0x58AC), six ribbons at the target (+0x106C, delayed 2 k,
//! growing with k, turned by the unit's facing), 200 bouncing sprites
//! (+0x238C), four bands (+0x20AC), a wide flash (+0x1020) and a camera
//! shake. Frame rays (+0xFB8) stepped, never set. Returns 1 on update 89, 2
//! on 128, 6 on 99..127, 8 on 150 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `frame_rays` 0x1E015F0, `streaks`
//! 0x1E02010, `flash` 0x1E02E30, `ribbon` 0x1E039E0, `bands` 0x1E04570,
//! `bounce` 0x1E04D60, `fade` 0x1E057D0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x58B4;

/// The entry 0x1E05D10 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x59C0);
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
    for (at, node, clear) in [(0xCu32, 0x32, 1), (0x10, 0x2A, 0), (0x14, 0xB, 0), (0x18, 0x13, 0), (0x1C, 0x1B, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): specs 0x1E067B8.. (four), motes +0xE8 /
    // +0x7E8.
    for (dst, src) in [(0xCCu32, 0x10u32), (0xCD, 0xC), (0xCE, 0x18), (0xCF, 0x14), (0xD7, 0x10)] {
        let b = fx.mem.read_u8(D + src);
        fx.mem.write_u8(D + dst, b);
    }
    for (k, spec) in [0x01E0_67B8u32, 0x01E0_67D0, 0x01E0_67C8, 0x01E0_67E0].into_iter().enumerate() {
        fx.mem.write_u32(D + 0xAC + 4 * k as u32, spec);
    }
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x20);
    fx.mem.write_u32(D + 0xA8, D + 0x7E8);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    fx.mem.write_u16(D + 0xD8, 0);
    fx.mem.write_u32(D + 0xDC, 0x4348_0000);
    fx.mem.write_u32(D + 0xE0, 0x42A0_0000);
    fx.mem.write_u32(D + 0xE4, 0x4100_0000);
    fx.mem.set16(D + 0xA0, 0x4E);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5F04, [Some(OT), Some(3), None]);
    0
}

/// Update 86 [F 0x1E05F5C]: the radiating unit (+0xFE8) from node +0x1C.
fn radiate_start(fx: &mut Fx) {
    let r = D + 0xFE8;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    for (o, v) in [(4u32, 0x80u8), (9, 2), (0xA, 0xFF), (8, 0x12), (0xB, 0xA)] {
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

/// rand with the entry's s0 (the OT) from `ra`, (x >> 4) % m.
fn rand_mod(fx: &mut Fx, ra: u32, m: i32) -> i32 {
    let x = fx.rand_from(OT, ra) >> 4;
    fx.div(x, m).1
}

/// Update 99 [F 0x1E05FE4]: at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
    set_counter4(fx, D + 0x58AC, 0xC, 0, 0xFF);
    fx.mem.write_u8(D + 0x58B0, 1);
    for k in 0..6i32 {
        let i = first_free(fx, D + 0x106C, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let r = D + 0x106C + 0x68 * i as u32;
        fx.mem.write_u8(r + 4, 4);
        set_counter4(fx, r, 0x1C, (2 * k) & 0xFF, 0xFF);
        let f = fx.mem.read_u8(r + 4) | 2;
        fx.mem.write_u8(r + 4, f);
        for (o, v) in [(0x27u32, 0x80u8), (0x23, 0x80), (5, 0), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0x32)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_60B8);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
        let t = ee::from_i32(k);
        fx.mem.setf(r + 0x1C, ee::add(40.0, ee::mul(40.0, t)));
        fx.mem.setf(r + 0x14, ee::sub(3.0, ee::mul(0.5, t)));
        fx.mem.setf(r + 0x18, ee::add(0.5, ee::mul(f32::from_bits(0x3ECC_CCCD), t)));
        let turn = if fx.mem.i32(D + 0x30) == 0x8000 { -0x4000 } else { 0x4000 };
        fx.mem.set32(r + 0x48, turn);
    }
    for _ in 0..0xC8 {
        let i = first_free(fx, D + 0x238C, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(OT, 0x01E0_61E0) >> 4) & 0xF;
        let b = D + 0x238C + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, delay, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 1), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(b + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_6230) >> 4;
        fx.mem.set8(b + 9, x & 7);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(b + o, 0x80);
        }
        let h = rand_mod(fx, 0x01E0_625C, 0x1E);
        let f = ee::div(ee::from_i32(h * 200 - 0xBB8), 100.0);
        fx.mem.setf(b + 0x10, ee::add(150.0, f));
        fx.mem.write_u32(b + 0x14, 0xBDCC_CCCD);
        fx.mem.write_u32(b + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(b + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        let h = rand_mod(fx, 0x01E0_62FC, 0xE6);
        fx.mem.setf(b + 0x2C, ee::div(ee::from_i32(h - 0x73), 100.0));
        let h = rand_mod(fx, 0x01E0_6348, 0xE6);
        let f = ee::div(ee::from_i32(h - 0x73), 100.0);
        fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), f));
        let h = rand_mod(fx, 0x01E0_63A4, 0xE6);
        fx.mem.setf(b + 0x34, ee::div(ee::from_i32(h - 0x73), 100.0));
    }
    for k in 0..4u32 {
        let r = D + 0x20AC + 0x5C * k;
        set_counter4(fx, r, 0x20, (2 * k) as i32, 0xFF);
        for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 1), (8, 2), (9, 2), (0xA, 6), (0xB, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x4000_0000);
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0x4000_0000);
        fx.mem.write_u32(r + 0x1C, 0x3F80_0000);
        fx.mem.write_u32(r + 0x20, 0x3F00_0000);
        for (o, v) in [(0x28u32, 0x80u8), (0x29, 0x80), (0x2A, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0x10), (0x2B, 0x40)] {
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
    for (o, v) in [(0xFu32, 0xF0u8), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x60), (9, 0x10), (8, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0);
    fx.mem.write_u32(r + 0x10, 0x4448_0000);
    copy_v4(fx, at, r + 0x1C);
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(OT), 0x01E0_5F2C);
    // The entry's s1 at the library calls: the parameter or the last
    // loop's value.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0x96 => return 8,
        0x63 => {
            strike(fx, sp);
            // The band loop's offset 0x5C x 3.
            s1 = 0x114;
        }
        0x56 => radiate_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_6614, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6624, [Some(u64::from(OT)), Some(s1)]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x20);
    // Never set: only its countdown runs (a3 not reached).
    blib::window::frame_rays(fx, 0x01E0_15F0, D + 0xFB8, OT, 1, 0);
    blib::radiate::streaks(fx, 0x01E0_2010, D + 0xFE8, OT, 1);
    blib::star::flash(fx, 0x01E0_2E30, D + 0x1020, OT, 1);
    blib::streak::ribbon(fx, 0x01E0_39E0, D + 0x106C, OT, 0x28);
    blib::band::bands(fx, 0x01E0_4570, D + 0x20AC, OT, 8);
    blib::sprite::bounce(fx, D + 0x238C, OT, 0xC8);
    blib::fade::fade(fx, D + 0x58AC, OT);
    // fade's saved s6 (the runner's 0) and ra (as b449) [run].
    fx.saved(sp - 0xF0, &[(0x70, 0), (0x80, 0x01E0_66D0)]);
    let n = fx.mem.u16(D);
    if (0x63..0x80).contains(&n) {
        6
    } else if n == 0x80 {
        2
    } else if n == 0x59 {
        1
    } else {
        0
    }
}
