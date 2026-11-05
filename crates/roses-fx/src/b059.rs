//! BMODULE b059 (card 459), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05660, data area a0 = 0x1ED0000) [F] (helper A17,
//! 31 Oct 2026): b199's family (b201's "a0 = D" records) with beads and a
//! sprinkling step. On update 40 the attacker's streaks record (+0x4F0:
//! flag 0x80 (far Z), all eight directions, ten rings, spread 0 then update
//! - 40 on 41..52, pairs (0x80, 0x40, 0x20) / alphas 0x20 / 0x40); on even
//! updates 40..120 the step 0x1E05370 sprinkles 5 x 5 scatter drops
//! (+0x528, 0x3C each, 200 slots; 0x12 counts, mask 0xF, delay rand & 3,
//! 15 wide, spread 0x118) from the nodes +0x10..+0x1C (node_position),
//! their y velocity 0.2 + (rand % 30 - 15) / 100, stopping at the first
//! full pool; on update 95 at the target three cones (+0xA4, 0x48 each,
//! delays 5 k, turned by the facing +0x30), a spike (+0x43C), a halo of 800
//! (+0x4A4), eight beads (+0x17C, 0x58 each; delays rand & 7, 0x14 wide)
//! and a shake (+0x40); the stage light by 0x1031C0 (+0xA0: down on update
//! 2, up on 144). Returns 1 on update 85, 2 on 92, 6 on 95..116, 8 on 150
//! (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` (ac7dccf5ae36) 0x1E00080,
//! `cones_b` 0x1E00440, `beads_at 40 44` 0x1E00E10, `spikes_b` 0x1E01D20,
//! `halo` 0x1E02A50, `streaks_c` 0x1E037A0, `scatter_drops` 0x1E04770
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E05660 (a0 = D, a1 = param; s2 = D, s1 = param, s0 = the
/// OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x50);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x50);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3510);
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
    for (k, (node, clear)) in [(0x1F, 1), (0x12, 0), (0x2B, 0), (0x37, 0), (0x41, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x90);
    // The entry's s0 = 3 (the texture loop's), s1 = the parameter, s2 = D.
    rand_bytes(fx, D + 0x3408, 0x100, 0x01E0_57BC, [Some(3), Some(param as u32), Some(D)]);
    0
}

/// The step 0x1E05370(D) [F]: on even updates 40..120, from each of the
/// nodes +0x10..+0x1C five scatter drops, until the pool is full.
fn sprinkle(fx: &mut Fx) {
    let n = fx.mem.u16(D);
    if !(0x28..0x79).contains(&n) || n & 1 != 0 {
        return;
    }
    let sp = fx.enter(0x60);
    'out: for s1 in 1..5i32 {
        for _ in 0..5 {
            let i = first_free(fx, D + 0x528, 0x3C, 0xC8);
            if i < 0 {
                break 'out;
            }
            let r = D + 0x528 + 0x3C * i as u32;
            let x = fx.rand_from(OT, 0x01E0_5404);
            set_counter4(fx, r, 0x12, x & 3, 0xF);
            fx.mem.write_u8(r + 5, 0);
            fx.mem.write_u8(r + 4, 0);
            fx.mem.write_u8(r + 8, 1);
            let x = fx.rand_from(OT, 0x01E0_5474);
            fx.mem.set8(r + 9, x & 7);
            fx.mem.write_u8(r + 0xA, 2);
            fx.mem.write_u8(r + 0xB, 0);
            for o in [0xFu32, 0xC, 0xD, 0xE] {
                fx.mem.write_u8(r + o, 0x80);
            }
            fx.mem.write_u32(r + 0x10, 0x4170_0000);
            fx.mem.write_u32(r + 0x14, 0x118);
            let h = fx.mem.i32(D + 4);
            let slot = fx.mem.i32(D + 0xC + 4 * s1 as u32);
            // The step's s0 = the slot index, s1 = the node counter.
            node_position_saving(fx, h, slot, sp + 0x50, i as i64 as u64, Some(s1 as u64), 0x01E0_5558);
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
                fx.mem.write_u32(r + 0x18 + 4 * c, w);
            }
            fx.mem.write_u32(r + 0x28, 0x3E4C_CCCD);
            let x = fx.rand_from(OT, 0x01E0_55C4) >> 4;
            let (_, h) = fx.div(x, 0x1E);
            let v = ee::div(ee::from_i32(h - 0xF), 100.0);
            let v = ee::add(fx.mem.f(r + 0x28), v);
            fx.mem.setf(r + 0x28, v);
        }
    }
    fx.leave(0x60);
}

/// Update 40 [F 0x1E058A4]: the streaks record.
fn start(fx: &mut Fx) {
    let r = D + 0x4F0;
    set_counter4(fx, r, 0x40, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0x80);
    for (o, v) in [(9u32, 2u8), (0xA, 0xFF), (8, 0), (0xB, 0xA)] {
        fx.mem.write_u8(r + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(r + 0xC, h);
    for (o, v) in [(0x16u32, 0x20u8), (0x15, 0x40), (0x14, 0x80), (0x17, 0x20), (0x12, 0x20), (0x11, 0x40), (0x10, 0x80), (0x13, 0x40)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u8(r + 5, 1);
    let node = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, node);
}

/// Update 95 [F 0x1E05954]: at the target `at` the three cones, the spike,
/// the halo, the beads and the shake.
fn strike(fx: &mut Fx, at: u32) {
    for k in 0..3u32 {
        let c = D + 0xA4 + 0x48 * k;
        set_counter4(fx, c, 0x14, ((5 * k) & 0xFF) as i32, 0xFF);
        for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, 1), (0xA, 0x20), (9, 7)] {
            fx.mem.write_u8(c + o, v);
        }
        fx.mem.write_u32(c + 0xC, 0x4080_0000);
        fx.mem.write_u32(c + 0x10, 0x3F4C_CCCD);
        fx.mem.write_u32(c + 0x14, 0x3FE6_6666);
        fx.mem.write_u32(c + 0x18, 0);
        fx.mem.write_u32(c + 0x1C, 0x3E4C_CCCD);
        for (o, v) in [(0x22u32, 0u8), (0x21, 0), (0x20, 0x80), (0x23, 0x40), (0x27, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80)] {
            fx.mem.write_u8(c + o, v);
        }
        copy_v4(fx, at, c + 0x28);
        let turn = if fx.mem.i32(D + 0x30) == 0x8000 { 0x4000 } else { -0x4000 };
        fx.mem.set32(c + 0x38, turn);
    }
    let r = D + 0x43C;
    set_counter4(fx, r, 0x20, 0, 0xFF);
    fx.mem.write_u8(r + 4, 7);
    fx.mem.write_u32(r + 0x14, 0x3FB3_3333);
    fx.mem.write_u32(r + 0x1C, 0x4248_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_5C34);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0x3408);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x21u32, 0u8), (0x22, 0), (0x20, 0xC0), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x23, 0x20), (0x27, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, at, r + 0x28);
    let g = D + 0x4A4;
    set_counter4(fx, g, 0x20, 0, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u8(g + 7, 9);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0xC0), (9, 0x60), (8, 0)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 0x14, 0);
    fx.mem.write_u32(g + 0x10, 0x4448_0000);
    copy_v4(fx, at, g + 0x1C);
    for k in 0..8u32 {
        let b = D + 0x17C + 0x58 * k;
        let x = fx.rand_from(OT, 0x01E0_5D38);
        set_counter4(fx, b, 0x20, (x >> 4) & 7, 0xFF);
        fx.mem.write_u8(b + 4, 5);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 8, 0x14);
        let x = fx.rand_from(OT, 0x01E0_5DD0);
        fx.mem.set8(b + 9, x >> 4);
        fx.mem.write_u8(b + 0xA, 2);
        fx.mem.write_u8(b + 0xC, 2);
        let x = fx.rand_from(OT, 0x01E0_5E38);
        fx.mem.set8(b + 0xB, x & 7);
        fx.mem.write_u32(b + 0x1C, 0x428C_0000);
        fx.mem.write_u32(b + 0x20, 0x40A0_0000);
        fx.mem.write_u32(b + 0x10, D + 0x3408);
        fx.mem.write_u32(b + 0x14, 0x3F33_3333);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(b + o, 0x80);
        }
        copy_v4(fx, at, b + 0x28);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3FC0_0000);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, s + 0x20);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, then the beads loop's count.
    let mut s1 = param as i64 as u64;
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    // The entry's s0 = the OT: its save in 0x1F71A0's and camera_shake's
    // frames (sp - 0x20) is the drops' missing-texture TEX1 [as b128].
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x40, u64::from(OT), 0x01E0_57E4);
    match fx.mem.u16(D) {
        0x96 => return 8,
        0x5F => {
            strike(fx, sp + 0x40);
            s1 = 8;
        }
        0x29..=0x34 => {
            let n = fx.mem.u8(D) - 0x28;
            fx.mem.set8(D + 0x4F8, n);
        }
        0x28 => start(fx),
        _ => {}
    }
    sprinkle(fx);
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_603C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_604C, [Some(u64::from(OT)), Some(s1)]);
    blib::cone::cones_b(fx, 0x01E0_0440, D + 0xA4, OT, 3);
    blib::spike::spikes_b(fx, 0x01E0_1D20, D + 0x43C, OT, 1);
    blib::halo::halo(fx, 0x01E0_2A50, D + 0x4A4, OT, 1);
    blib::beam::beads_at(fx, 0x01E0_0E10, 0x40, 0x44, D + 0x17C, OT, 8);
    blib::radiate::streaks_c(fx, 0x01E0_37A0, D + 0x4F0, OT, 1);
    blib::drop::scatter_drops(fx, 0x01E0_4770, D + 0x528, OT, 0xB4);
    let n = fx.mem.u16(D);
    if (0x5F..0x75).contains(&n) {
        6
    } else if n == 0x5C {
        2
    } else if n == 0x55 {
        1
    } else {
        0
    }
}
