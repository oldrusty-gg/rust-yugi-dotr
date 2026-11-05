//! BMODULE b459 (card 546), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E042D0, data area a0 = 0x1ED0000) [F] (helper B5, 31
//! Oct 2026): b469's program with other nodes and times. On update 2 two
//! long flashes on the attacker's nodes 0x15 / 0x1E (+0x1904), a bolt
//! between them (+0x1DC4) and a long camera shake (+0x40); on 33 a third
//! flash (+0x199C); on 37 the shake moves to the target, eight bead beams
//! (+0x3CB4) and four jagged bolts node -> target; on 76 the flashes
//! widen; on 90 a wide flash at the target and two segment spawners
//! (+0x2CC4); on even updates 2..191 two ribbons a update from the nodes
//! (+0xA4); on updates 90..254 b469's flashes and ribbons about the target;
//! the stage light pulsed by 0x1031C0 (+0xA0). The flicker pool (+0x2EA4)
//! is drawn but never filled. Returns 1 on update 80, 6 on 90..254, 2 on
//! 255, 8 on 270 (and at once for -2 / -3).
//!
//! Library (record addresses): as b469 [`crate::b469`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::b469::{beads8, bolt_bytes, charge2, flash_bytes, jag_from, ribbon_from, sparkle, strike};
use crate::b586::segment_spawns_saving;
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x4394;
const FLASHES: u32 = D + 0x1904;
const BOLTS: u32 = D + 0x1DC4;

/// The entry 0x1E042D0 (a0 = D, a1 = param; s0 = D, s1 = the OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1A0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x1A0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x44A0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..2 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x15, 1), (0x10, 0x1E, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x108);
    rand_bytes(fx, BYTES, 0x100, 0x01E0_43EC, [Some(D), Some(OT), Some(2)]);
    0
}

/// Update 2 [F 0x1E04468]: two long flashes on nodes +0xC / +0x10, a bolt
/// between them and the long shake (both skipped when the bolt pool is
/// full).
fn charge(fx: &mut Fx) {
    for k in 0..2u32 {
        let r = FLASHES + 0x4C * k;
        set_counter4(fx, r, 0xA0, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        flash_bytes(fx, r, 0x85, 0x40, 0x20, 0x80, 0xC0);
        fx.mem.write_u32(r + 0x10, 0x42C8_0000);
        fx.mem.write_u32(r + 0x14, 0);
    }
    let i = first_free(fx, BOLTS, 0x40, 0x3C);
    if i < 0 {
        return;
    }
    let b = BOLTS + ((i as u32) << 6);
    set_counter4(fx, b, 0x46, 0, 0xFF);
    fx.mem.write_u8(b + 5, 1);
    fx.mem.write_u8(b + 4, 1);
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(b + 9, v);
    let v = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(b + 0xA, v);
    fx.mem.write_u8(b + 8, 5);
    fx.mem.write_u32(b + 0x10, 0x3F80_0000);
    fx.mem.write_u32(b + 0x14, 0x3E99_999A);
    for (o, v) in [(0xBu32, 1u8), (0xD, 4), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x30)] {
        fx.mem.write_u8(b + o, v);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0xFA, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, v);
    fx.mem.write_u8(s + 4, 4);
    fx.mem.write_u32(s + 0x14, 0x3F99_999A);
    fx.mem.write_u32(s + 0x18, 0x3F99_999A);
    fx.mem.write_u32(s + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x4000_0000);
}

/// Update 37 [F 0x1E046A4]: the shake at the target (sp + 0x60), eight
/// bead beams on node +0xC and four jagged bolts node -> target (counts
/// 221 - delay).
fn bolts_start(fx: &mut Fx, sp: u32) {
    let s = D + 0x40;
    set_counter4(fx, s, 0xC8, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3FB3_3333);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4320_0000);
    copy_v4(fx, sp + 0x60, D + 0x60);
    fx.mem.write_u32(s + 0x50, 0x4100_0000);
    fx.mem.write_u32(D + 0x68, 0);
    beads8(fx, sp, [0x01E0_4798, 0x01E0_47F4, 0x01E0_4818]);
    for j in 0..4u32 {
        jag_from(fx, sp, 0x3F00_0000);
        for s in 0..6u32 {
            let i = first_free(fx, BOLTS, 0x40, 0x3C);
            if i < 0 {
                break;
            }
            let b = BOLTS + ((i as u32) << 6);
            let delay = j + 9 * s;
            set_counter4(fx, b, (0xDDu32.wrapping_sub(delay) & 0xFF) as i32, (delay & 0xFF) as i32, 0xFF);
            bolt_bytes(fx, sp, b, s, 9, [0x3F33_3333, 0x3E4C_CCCD], 3);
        }
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_4414);
    match fx.mem.u16(D) {
        0x10E => return 8,
        0x5A => strike(fx, sp, D + 0x199C, [0xA8, 0xA8]),
        0x4C => {
            fx.mem.write_u32(D + 0x1960, 0x4348_0000);
            fx.mem.write_u32(D + 0x1914, 0x4348_0000);
        }
        0x25 => bolts_start(fx, sp),
        0x21 => charge2(fx),
        2 => charge(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0xC0).contains(&n) && n & 1 == 0 {
        let look = [(0x3F19_999A, 0x4170_0000, 8), (0x4000_0000, 0x41F0_0000, 0x12)];
        for k in 0..2u32 {
            ribbon_from(fx, D + 0xC + 4 * k, 0x28, look, [0x01E0_4C48, 0x01E0_4C98, 0x01E0_4CB0, 0x01E0_4CC8], BYTES);
        }
    }
    let n = fx.mem.u16(D);
    if (0x5A..0xFF).contains(&n) {
        sparkle(fx, sp, [0x01E0_4D20, 0x01E0_4D6C, 0x01E0_4F20]);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_4FA4, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4FB4, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::star::flash(fx, 0x01E0_0410, FLASHES, OT, 0x10);
    blib::streak::ribbon(fx, 0x01E0_0FC0, D + 0xA4, OT, 0x3C);
    segment_spawns_saving(fx, 0x01E0_26A0, D + 0x2CC4, 8, 0x01E0_4FF0);
    blib::bolt::bolts(fx, 0x01E0_1B50, BOLTS, OT, 0x28);
    blib::flicker::flickers(fx, 0x01E0_2A20, D + 0x2EA4, OT, 0x3C);
    blib::beam::beads(fx, 0x01E0_3500, D + 0x3CB4, OT, 0x14);
    let n = fx.mem.u16(D);
    if (0x5A..0xFF).contains(&n) {
        6
    } else if n == 0xFF {
        2
    } else if n == 0x50 {
        1
    } else {
        0
    }
}
