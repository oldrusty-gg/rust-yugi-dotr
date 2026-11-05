//! BMODULE b508 (card 445), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05DE0, data area a0 = 0x1ED0000) [F] (helper D9,
//! 31 Oct 2026): b078's family (s0 = D, s1 = the OT) on a 0x1E10 layout.
//! Node bursts (+0xA4, motes +0xE8, two specs) every update; on update 2
//! two smears (+0x1A74, `smear::smears_c`) between the attacker's nodes
//! 0x21 / 0x22 both ways; on 83 at the target the fade (+0x1A6C), a
//! ribbon (+0x1034), twelve bands (+0x109C, three turns of four), sixteen
//! beads (+0x14EC), a wide flash (+0xFE8) and a camera shake. Frame rays
//! (+0xFB8) stepped, never set. Returns 1 on update 73, 2 on 112, 6 on
//! 83..111, 8 on 150 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `frame_rays` 0x1E015F0, `flash`
//! 0x1E02010, `ribbon` 0x1E02BC0, `bands` 0x1E03750, `fade` 0x1E03F40,
//! `beads` 0x1E04480, `smears_c` 0x1E05250 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x1D04;
const SMEARS: u32 = D + 0x1A74;

/// The entry 0x1E05DE0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x1E10);
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
    for (at, node, clear) in [(0xCu32, 0x2C, 1), (0x10, 0x17, 0), (0x14, 0xA, 0), (0x18, 0x33, 0), (0x1C, 0x21, 0), (0x20, 0x22, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): specs 0x1E06860 / 0x1E06850, motes +0xE8 /
    // +0x7E8.
    for (dst, src) in [(0xCCu32, 0xCu32), (0xCD, 0x10), (0xCE, 0x14), (0xCF, 0x18)] {
        let b = fx.mem.read_u8(D + src);
        fx.mem.write_u8(D + dst, b);
    }
    for (k, spec) in [0x01E0_6860u32, 0x01E0_6850, 0x01E0_6850, 0x01E0_6860].into_iter().enumerate() {
        fx.mem.write_u32(D + 0xAC + 4 * k as u32, spec);
    }
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x20);
    fx.mem.write_u32(D + 0xA8, D + 0x7E8);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    fx.mem.write_u16(D + 0xD8, 0);
    fx.mem.write_u32(D + 0xDC, 0x4396_0000);
    fx.mem.write_u32(D + 0xE4, 0x4170_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x90);
    // The entry's s0 = D, s1 = the OT, s2 = 3 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5FE4, [Some(D), Some(OT), Some(3)]);
    0
}

/// Update 2 [F 0x1E0603C]: the two smears, nodes +0x1C / +0x20 both ways.
fn smears_start(fx: &mut Fx) {
    for k in 0..2u32 {
        let r = SMEARS + 0x48 * k;
        set_counter4(fx, r, 0x70, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let (a, b) = if k == 0 { (0x1C, 0x20) } else { (0x20, 0x1C) };
        let v = fx.mem.read_u8(D + a);
        fx.mem.write_u8(r + 0x21, v);
        let v = fx.mem.read_u8(D + b);
        fx.mem.write_u8(r + 0x22, v);
        fx.mem.write_u32(r + 8, D + 0x1B04);
        for (o, v) in [(0x1Eu32, 0x20u8), (0x1F, 5), (0x20, 1)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x2C, 0x4220_0000);
        fx.mem.write_u32(r + 0x28, 0x3E99_999A);
        for o in 0x34..0x38u32 {
            fx.mem.write_u8(r + o, 0x80);
        }
        let len = fx.mem.read_u8(r + 0x1F) as u32;
        for s in 0..=len {
            fx.mem.set16(r + 0xC + 2 * s, -1);
        }
        fx.mem.write_u32(r + 0x30, 0xBF80_0000);
    }
}

/// Update 83 [F 0x1E06164]: at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
    set_counter4(fx, D + 0x1A6C, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0x1A70, 1);
    let r = D + 0x1034;
    set_counter4(fx, r, 0x30, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4020_0000);
    fx.mem.write_u32(r + 0x1C, 0x4270_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(D, 0x01E0_61E0);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, at, r + 0x28);
    fx.mem.write_u32(D + 0x108C, 0x40);
    fx.mem.write_u32(D + 0x1090, 0x80);
    fx.mem.write_u32(D + 0x1094, 0x100);
    for k in 0..0xCu32 {
        let r = D + 0x109C + 0x5C * k;
        let turn = k >> 2;
        let delay = ((k % 3) << 1) + (turn << 2);
        set_counter4(fx, r, 0x30, (delay & 0xFF) as i32, 0xFF);
        for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 1), (8, 2), (9, 2), (0xA, 6), (0xB, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x3FC0_0000);
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0x4040_0000);
        fx.mem.write_u32(r + 0x1C, 0x4000_0000);
        fx.mem.write_u32(r + 0x20, 0x3E80_0000);
        for (o, v) in [(0x28u32, 0x80u8), (0x29, 0x80), (0x2A, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0x10), (0x2B, 0x30)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        match turn {
            2 => fx.mem.write_u32(r + 0x44, 0x1000),
            1 => fx.mem.write_u32(r + 0x44, 0x3000),
            _ => {
                fx.mem.write_u32(r + 0x3C, 0x4000);
                fx.mem.write_u32(r + 0x40, 0x3000);
                fx.mem.write_u32(r + 0x44, 0x2000);
            }
        }
    }
    for k in 0..0x10u32 {
        let r = D + 0x14EC + 0x58 * k;
        set_counter4(fx, r, 0x20, ((2 * k) & 0xFF) as i32, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(4);
        fx.mem.write_u8(r, c);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x3C);
        let x = fx.rand_from(D, 0x01E0_6438);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u8(r + 0xA, 1);
        fx.mem.write_u8(r + 0xC, 2);
        let x = fx.rand_from(D, 0x01E0_645C);
        fx.mem.set8(r + 0xB, (x >> 4) & 3);
        fx.mem.write_u32(r + 0x1C, 0x42F0_0000);
        fx.mem.write_u32(r + 0x20, 0x4000_0000);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x4060_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
    }
    let r = D + 0xFE8;
    set_counter4(fx, r, 0x34, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    for (o, v) in [(0xFu32, 0xF0u8), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x80), (9, 0x80), (8, 0x10)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0);
    fx.mem.write_u32(r + 0x10, 0x44BB_8000);
    copy_v4(fx, at, r + 0x1C);
    let s = D + 0x40;
    set_counter4(fx, s, 0x30, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3F99_999A);
    fx.mem.write_u32(s + 0x18, 0x4000_0000);
    fx.mem.write_u32(s + 0x1C, 0x3F99_999A);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_600C);
    match fx.mem.u16(D) {
        0x96 => return 8,
        0x53 => strike(fx, sp),
        2 => smears_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_65F0, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6600, [Some(u64::from(D)), Some(u64::from(OT))]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x20);
    // Never set: only its countdown runs (a3 not reached).
    blib::window::frame_rays(fx, 0x01E0_15F0, D + 0xFB8, OT, 1, 0);
    blib::star::flash(fx, 0x01E0_2010, D + 0xFE8, OT, 1);
    blib::streak::ribbon(fx, 0x01E0_2BC0, D + 0x1034, OT, 1);
    blib::band::bands(fx, 0x01E0_3750, D + 0x109C, OT, 0xC);
    blib::fade::fade(fx, D + 0x1A6C, OT);
    blib::beam::beads(fx, 0x01E0_4480, D + 0x14EC, OT, 0x10);
    blib::smear::smears_c(fx, 0x01E0_5250, SMEARS, OT, 2);
    let n = fx.mem.u16(D);
    if (0x53..0x70).contains(&n) {
        6
    } else if n == 0x70 {
        2
    } else if n == 0x49 {
        1
    } else {
        0
    }
}
