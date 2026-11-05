//! BMODULE b128 (card 64), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03490, data area a0 = 0x1ED0000) [F] (helper A16,
//! 31 Oct 2026): b201's family (the "a0 = D" records compiled without
//! scheduling) with the scattering drops 0f8b7add37bb. On update 147 three
//! waves of 30 drops (+0x1E4, 0x3C each; delays k / 2, mask 7, 25 wide,
//! spread 250) on the attacker's nodes 0x10 / 0x12 / 0x14; on update 182 at
//! the target two cones (+0xA0, 0x48 each, turned +-0x2800), a spike
//! (+0x130), a halo of 600 (+0x198) and a shake (+0x40); on odd updates
//! 140..222 an object op (0x1F83B0 mode 6) on the attacker with a grey /
//! (0x80, 0x80, 0x70, 0x40) pair. Returns 1 on update 172, 2 on 179, 6 on
//! 182..204, 8 on 280 (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` (ac7dccf5ae36) 0x1E00080,
//! `cones_b` 0x1E00440, `spikes_b` 0x1E00E10, `halo` 0x1E01B40,
//! `scatter_drops` 0x1E02890 [`crate::blib`].
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E03490 (a0 = D, a1 = param; s4 = D, s1 = param, s0 = the
/// OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x1800);
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
    for (k, (node, clear)) in [(0x10, 1), (0x12, 0), (0x14, 0), (1, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    // The entry's s0 = the OT, s1 = 2 (the texture loop's), s2 = 0 (the
    // runner's [H, as b201]).
    rand_bytes(fx, D + 0x16FC, 0x100, 0x01E0_35CC, [Some(OT), Some(2), Some(0)]);
    0
}

/// Update 147 [F 0x1E03624]: three waves of 30 drops on the nodes +0xC /
/// +0x10 / +0x14 (delays k / 2, countdown mask 7).
fn waves(fx: &mut Fx) {
    for wave in 0..3u32 {
        for k in 0..30u32 {
            let r = D + 0x1E4 + 0x3C * (k + 30 * wave);
            set_counter4(fx, r, 0x40, ((k >> 1) & 0xFF) as i32, 7);
            fx.mem.write_u8(r + 5, 1);
            let node = fx.mem.read_u8(D + 0xC + 4 * wave);
            fx.mem.write_u8(r + 6, node);
            fx.mem.write_u8(r + 4, 0);
            for (o, v) in [(8u32, 1u8), (9, 3), (0xA, 2), (0xB, 0), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
                fx.mem.write_u8(r + o, v);
            }
            fx.mem.write_u32(r + 0x10, 0x41C8_0000);
            fx.mem.write_u32(r + 0x14, 0xFA);
        }
    }
}

/// Update 182 [F 0x1E037D4]: at the target `at` two cones, the spike, the
/// halo and the shake.
fn strike(fx: &mut Fx, at: u32) {
    for k in 0..2u32 {
        let c = D + 0xA0 + 0x48 * k;
        set_counter4(fx, c, 0x14, 0, 0xFF);
        for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, 0)] {
            fx.mem.write_u8(c + o, v);
        }
        fx.mem.write_u32(c + 0xC, 0x4020_0000);
        fx.mem.write_u32(c + 0x10, 0x3DCC_CCCD);
        fx.mem.write_u32(c + 0x14, 0x3E4C_CCCD);
        fx.mem.write_u32(c + 0x18, 0);
        fx.mem.write_u32(c + 0x1C, 0x3CF5_C28F);
        for (o, v) in [(0x23u32, 0x80u8), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x26, 0x80), (0x25, 0), (0x24, 0x40), (0x27, 0)] {
            fx.mem.write_u8(c + o, v);
        }
        copy_v4(fx, at, c + 0x28);
        fx.mem.set32(c + 0x40, if k != 0 { 0x2800 } else { -0x2800 });
    }
    let r = D + 0x130;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    fx.mem.write_u8(r + 4, 7);
    fx.mem.write_u32(r + 0x14, 0x3F99_999A);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_3A70);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0x16FC);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x22u32, 0x80u8), (0x21, 0), (0x20, 0x40), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x23, 0x10), (0x27, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, at, r + 0x28);
    let g = D + 0x198;
    set_counter4(fx, g, 0x1C, 0, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u8(g + 7, 9);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x40), (9, 0), (8, 0x80)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 0x14, 0);
    fx.mem.write_u32(g + 0x10, 0x4416_0000);
    copy_v4(fx, at, g + 0x1C);
    let s = D + 0x40;
    set_counter4(fx, s, 0x18, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, s + 0x20);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    // The entry's s0 = the OT: its save in 0x1F71A0's and camera_shake's
    // frames (sp - 0x20) is the drops' missing-texture TEX1 [run].
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(OT), 0x01E0_35F4);
    match fx.mem.u16(D) {
        0x118 => return 8,
        0xB6 => strike(fx, sp + 0x60),
        0x93 => waves(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x8C..0xDF).contains(&n) && n & 1 != 0 {
        for (o, v) in [(0x78u32, 0x80u8), (0x79, 0x80), (0x7A, 0x80), (0x7E, 0x80), (0x7D, 0x80), (0x7C, 0x80), (0x7B, 0x70), (0x7F, 0x40)] {
            fx.mem.write_u8(sp + o, v);
        }
        let h = fx.mem.i32(D + 4);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h as u32, 6, 0, sp + 0x78]);
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_3C8C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::cone::cones_b(fx, 0x01E0_0440, D + 0xA0, OT, 2);
    blib::spike::spikes_b(fx, 0x01E0_0E10, D + 0x130, OT, 1);
    blib::halo::halo(fx, 0x01E0_1B40, D + 0x198, OT, 1);
    blib::drop::scatter_drops(fx, 0x01E0_2890, D + 0x1E4, OT, 0x5A);
    let n = fx.mem.u16(D);
    if (0xB6..0xCD).contains(&n) {
        6
    } else if n == 0xB3 {
        2
    } else if n == 0xAC {
        1
    } else {
        0
    }
}
