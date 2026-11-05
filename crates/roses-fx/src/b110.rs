//! BMODULE b110 (card 393), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E042D0, data area a0 = 0x1ED0000) [F] (helper B5, 31
//! Oct 2026): b469's program with other nodes and times. On update 2 a
//! long flash on the attacker's node 0x15 (+0x1904), 24 flickering sprites
//! there (+0x2EA4, `flicker::flickers`, random delays), two bolts from
//! nodes 0x16 / 0x14 to it (+0x1DC4) and a long camera shake (+0x40); on 70
//! the shake moves to the target, a flash (+0x1950), eight bead beams
//! (+0x3CB4) and five jagged bolts node -> target; on 85 the first flash
//! widens; on 118 a wide flash at the target (+0x1950) and two segment
//! spawners (+0x2CC4); on even updates 2..249 a ribbon from node 0x15
//! (+0xA4); on updates 118..254 b469's flashes and ribbons about the
//! target; the stage light pulsed by 0x1031C0 (+0xA0). Returns 1 on update
//! 108, 6 on 118..254, 2 on 255, 8 on 270 (and at once for -2 / -3).
//!
//! Library (record addresses): as b469 [`crate::b469`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::b469::{beads8, bolt_bytes, flash_bytes, jag_from, ribbon_from, sparkle, strike};
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
    for (at, node, clear) in [(0xCu32, 0x15, 1), (0x10, 0x16, 0), (0x14, 0x14, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x108);
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4404, [Some(D), Some(OT), Some(2)]);
    0
}

/// Update 2 [F 0x1E04474]: the flash, 24 flickers and two bolts on node
/// +0xC, the long shake.
fn charge(fx: &mut Fx) {
    let r = FLASHES;
    set_counter4(fx, r, 0xFF, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    flash_bytes(fx, r, 0x85, 0x40, 0x20, 0x80, 0xC0);
    fx.mem.write_u32(r + 0x10, 0x4348_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for k in 0..0x18u32 {
        let x = fx.rand_from(D, 0x01E0_44F8) >> 4;
        let r = D + 0x2EA4 + 0x3C * k;
        set_counter4(fx, r, 0x46, x & 0xF, 0xF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(r + 6, b);
        for (o, v) in [(4u32, 0u8), (8, 0), (9, 4), (0xA, 2), (0xB, 0), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x41A0_0000);
        fx.mem.set32(r + 0x14, 0x258);
    }
    for k in 0..2u32 {
        let i = first_free(fx, BOLTS, 0x40, 0x3C);
        if i < 0 {
            break;
        }
        let b = BOLTS + ((i as u32) << 6);
        set_counter4(fx, b, 0x46, k as i32, 0xFF);
        fx.mem.write_u8(b + 5, 1);
        fx.mem.write_u8(b + 4, 1);
        let v = fx.mem.read_u8(D + 0x10 + 4 * k);
        fx.mem.write_u8(b + 9, v);
        let v = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(b + 0xA, v);
        fx.mem.write_u8(b + 8, 3);
        fx.mem.write_u32(b + 0x10, 0x3FC0_0000);
        fx.mem.write_u32(b + 0x14, 0x3E99_999A);
        for (o, v) in [(0xBu32, 1u8), (0xD, 4), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)] {
            fx.mem.write_u8(b + o, v);
        }
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0xFA, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, v);
    fx.mem.write_u8(s + 4, 4);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
}

/// Update 70 [F 0x1E046C0]: the shake to the target (sp + 0x60), a flash
/// (+0x1950) and eight bead beams on node +0xC, five jagged bolts node ->
/// target (counts 180 - delay).
fn bolts_start(fx: &mut Fx, sp: u32) {
    let s = D + 0x40;
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x34, 0x4320_0000);
    copy_v4(fx, sp + 0x60, D + 0x60);
    fx.mem.write_u32(s + 0x50, 0x4100_0000);
    fx.mem.write_u32(D + 0x68, 0);
    let r = D + 0x1950;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    flash_bytes(fx, r, 0x80, 0x40, 0x20, 0x80, 0xF0);
    fx.mem.write_u32(r + 0x10, 0x4348_0000);
    fx.mem.write_u32(r + 0x14, 0x4316_0000);
    fx.mem.write_u32(r + 0x18, 0x40C0_0000);
    beads8(fx, sp, [0x01E0_47F4, 0x01E0_4850, 0x01E0_4878]);
    for j in 0..5u32 {
        jag_from(fx, sp, 0x3F40_0000);
        for s in 0..6u32 {
            let i = first_free(fx, BOLTS, 0x40, 0x3C);
            if i < 0 {
                break;
            }
            let b = BOLTS + ((i as u32) << 6);
            let delay = j + 8 * s;
            set_counter4(fx, b, (0xB4u32.wrapping_sub(delay) & 0xFF) as i32, (delay & 0xFF) as i32, 0xFF);
            bolt_bytes(fx, sp, b, s, 8, [0x3F66_6666, 0x3E99_999A], 4);
        }
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_442C);
    match fx.mem.u16(D) {
        0x10E => return 8,
        0x76 => strike(fx, sp, D + 0x1950, [0x84, 0x80]),
        0x55 => fx.mem.write_u32(D + 0x1914, 0x4396_0000),
        0x46 => bolts_start(fx, sp),
        2 => charge(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0xFA).contains(&n) && n & 1 == 0 {
        let look = [(0x3F00_0000, 0x4120_0000, 8), (0x4060_0000, 0x4270_0000, 0x24)];
        ribbon_from(fx, D + 0xC, 0x46, look, [0x01E0_4C84, 0x01E0_4CD4, 0x01E0_4CEC, 0x01E0_4D04], BYTES);
    }
    let n = fx.mem.u16(D);
    if (0x76..0xFF).contains(&n) {
        sparkle(fx, sp, [0x01E0_4D44, 0x01E0_4D90, 0x01E0_4F40]);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_4FC4, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4FD4, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::star::flash(fx, 0x01E0_0410, FLASHES, OT, 0x10);
    blib::streak::ribbon(fx, 0x01E0_0FC0, D + 0xA4, OT, 0x3C);
    segment_spawns_saving(fx, 0x01E0_26A0, D + 0x2CC4, 8, 0x01E0_5010);
    blib::bolt::bolts(fx, 0x01E0_1B50, BOLTS, OT, 0x28);
    blib::flicker::flickers(fx, 0x01E0_2A20, D + 0x2EA4, OT, 0x3C);
    blib::beam::beads(fx, 0x01E0_3500, D + 0x3CB4, OT, 0x14);
    let n = fx.mem.u16(D);
    if (0x76..0xFF).contains(&n) {
        6
    } else if n == 0xFF {
        2
    } else if n == 0x6C {
        1
    } else {
        0
    }
}
