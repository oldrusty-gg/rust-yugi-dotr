//! BMODULE b199 (card 26), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04FE0, data area a0 = 0x1ED0000) [F] (helper A17,
//! 31 Oct 2026): b201's family (the "a0 = D" records compiled without
//! scheduling) with the recompiled streaks 391d5c6072b1 and the scattering
//! drops. On update 2 eighty scatter drops (+0x1E68, 0x3C each; delays k,
//! mask 7, 35 wide, spread 0x514) on the attacker's node 1; on update 21 at
//! that node a halo of 500 (+0x254, 0x40 for 12) and a small one of 80
//! (+0x2A0, 16, radius 12 at the node's position), and the attacker's
//! streaks record (+0x1E30: all eight directions, ten rings, spread 0 then
//! update - 20 on 22..31, far colour (0x30, 0x30, 0x30, 8)); on update 55
//! at the target six cones (+0xA4, 0x48 each, delays 2 k, turned by the
//! facing +0x30), a halo of 1000 (+0x2EC), a spike (+0x338), 100 drops
//! (+0x3A0, 0x44 each; 80 + (rand % 15 x 200 - 1500) / 100 wide, spread
//! (rand % 220 - 110) / 100) and a shake (+0x40); the stage light by
//! 0x1031C0 (+0xA0: down on update 2, up on 104). Returns 1 on update 45,
//! 6 on 55..75, 2 on 76, 8 on 110 (and at once for a parameter of -2 /
//! -3).
//!
//! Library (record addresses): `camera_shake` (ac7dccf5ae36) 0x1E00080,
//! `cones_b` 0x1E00440, `halo` 0x1E00E10, `spikes_b` 0x1E01B60, `drops`
//! 0x1E02890, `streaks_c` 0x1E03410, `scatter_drops` 0x1E043E0
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E04FE0 (a0 = D, a1 = param; s3 = D, s1 = param, s0 = the
/// OT).
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
    fx.bzero(D, 0x3230);
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
    for (k, (node, clear)) in [(1, 1), (6, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x68);
    // The entry's s0 = the OT, s1 = 2 (the texture loop's), s2 = 0 (the
    // runner's [H, as b201]).
    rand_bytes(fx, D + 0x3128, 0x100, 0x01E0_50FC, [Some(OT), Some(2), Some(0)]);
    0
}

/// Update 2 [F 0x1E051D8]: eighty scatter drops on node +0xC.
fn scatter(fx: &mut Fx) {
    for k in 0..0x50u32 {
        let r = D + 0x1E68 + 0x3C * k;
        set_counter4(fx, r, 0xFF, (k & 0xFF) as i32, 7);
        fx.mem.write_u8(r + 5, 1);
        let node = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(r + 6, node);
        fx.mem.write_u8(r + 4, 0);
        for (o, v) in [(8u32, 0u8), (9, 3), (0xA, 2), (0xB, 0), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x420C_0000);
        fx.mem.write_u32(r + 0x14, 0x514);
    }
}

/// Update 21 [F 0x1E0534C]: the two halos at node +0xC and the streaks
/// record.
fn start(fx: &mut Fx, param: i32) {
    let g = D + 0x254;
    set_counter4(fx, g, 0x40, 0xC, 0xFF);
    fx.mem.write_u8(g + 5, 1);
    let node = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(g + 6, node);
    fx.mem.write_u8(g + 4, 0x83);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x43FA_0000);
    fx.mem.write_u32(g + 0x14, 0);
    for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xA, 0x80), (9, 0x80), (8, 0x10), (0xF, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    let g = D + 0x2A0;
    set_counter4(fx, g, 0x10, 0, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0);
    fx.mem.write_u32(g + 0x14, 0x42A0_0000);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x80), (8, 0x20), (0xF, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    fx.mem.write_u32(g + 0x18, 0x4140_0000);
    let h = fx.mem.i32(D + 4);
    let slot = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, slot, g + 0x1C, u64::from(OT), Some(param as i64 as u64), 0x01E0_545C);
    let r = D + 0x1E30;
    set_counter4(fx, r, 0x40, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0);
    for (o, v) in [(9u32, 2u8), (0xA, 0xFF), (8, 0), (0xB, 0xA)] {
        fx.mem.write_u8(r + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(r + 0xC, h);
    for (o, v) in [(0x16u32, 0x80u8), (0x15, 0x80), (0x14, 0x80), (0x17, 0x20), (0x10, 0x30), (0x11, 0x30), (0x12, 0x30), (0x13, 8)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u8(r + 5, 1);
    let node = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, node);
}

/// ((rand >> 4) % 220 - 110) / 100 [F].
fn spread(fx: &mut Fx, ra: u32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, 0xDC);
    ee::div(ee::from_i32(h - 0x6E), 100.0)
}

/// Update 55 [F 0x1E05500]: at the target `at` the six cones, the halo,
/// the spike, the drops and the shake.
fn strike(fx: &mut Fx, at: u32) {
    for k in 0..6u32 {
        let c = D + 0xA4 + 0x48 * k;
        set_counter4(fx, c, 0x18, ((2 * k) & 0xFF) as i32, 0xFF);
        for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, 1), (0xA, 0x40), (9, 7)] {
            fx.mem.write_u8(c + o, v);
        }
        fx.mem.write_u32(c + 0xC, 0x4000_0000);
        fx.mem.write_u32(c + 0x10, 0x3E99_999A);
        fx.mem.write_u32(c + 0x14, 0x3FE6_6666);
        fx.mem.write_u32(c + 0x18, 0);
        fx.mem.write_u32(c + 0x1C, 0x3E4C_CCCD);
        for (o, v) in [(0x22u32, 0x10u8), (0x21, 0x10), (0x20, 0xC0), (0x23, 0), (0x27, 0x60), (0x24, 0x60), (0x25, 0x60), (0x26, 0x60)] {
            fx.mem.write_u8(c + o, v);
        }
        copy_v4(fx, at, c + 0x28);
        let turn = if fx.mem.i32(D + 0x30) == 0x8000 { 0x4000 } else { -0x4000 };
        fx.mem.set32(c + 0x38, turn);
    }
    let g = D + 0x2EC;
    set_counter4(fx, g, 0x1C, 8, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x447A_0000);
    fx.mem.write_u32(g + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x20), (8, 0x80), (0xF, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    copy_v4(fx, at, g + 0x1C);
    let r = D + 0x338;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4033_3333);
    fx.mem.write_u32(r + 0x1C, 0x4270_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_5878);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0x3128);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, at, r + 0x28);
    fx.mem.write_u32(r + 0x58, 0x40);
    fx.mem.write_u32(r + 0x5C, 0x80);
    fx.mem.write_u32(r + 0x60, 0x100);
    for _ in 0..0x64 {
        let i = first_free(fx, D + 0x3A0, 0x44, 0x64);
        if i < 0 {
            continue;
        }
        let r = D + 0x3A0 + 0x44 * i as u32;
        let x = fx.rand_from(OT, 0x01E0_592C);
        set_counter4(fx, r, 0x24, (x >> 4) & 7, 0xFF);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u8(r + 8, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 9, 5);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let x = fx.rand_from(OT, 0x01E0_5A20) >> 4;
        let (_, h) = fx.div(x, 0xF);
        let v = ee::add(80.0, ee::div(ee::from_i32(h.wrapping_mul(200) - 1500), 100.0));
        fx.mem.setf(r + 0x10, v);
        fx.mem.write_u32(r + 0x14, 0xBF19_999A);
        fx.mem.write_u32(r + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let v = spread(fx, 0x01E0_5B2C);
        fx.mem.setf(r + 0x2C, v);
        let v = spread(fx, 0x01E0_5B80);
        fx.mem.setf(r + 0x30, ee::add(1.0, v));
        let v = spread(fx, 0x01E0_5BE4);
        fx.mem.setf(r + 0x34, v);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_5CAC);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, then the last loop's count.
    let mut s1 = param as i64 as u64;
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    // The entry's s0 = the OT: its save in 0x1F71A0's and camera_shake's
    // frames (sp - 0x20) is the drops' missing-texture TEX1 [as b128].
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_5124);
    match fx.mem.u16(D) {
        0x6E => return 8,
        0x37 => {
            strike(fx, sp + 0x50);
            s1 = 0x64;
        }
        0x16..=0x1F => {
            let n = fx.mem.u8(D).wrapping_sub(0x14);
            fx.mem.set8(D + 0x1E38, n);
        }
        0x15 => start(fx, param),
        2 => {
            scatter(fx);
            s1 = 0x50;
        }
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_5CCC, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5CDC, [Some(u64::from(OT)), Some(s1)]);
    blib::cone::cones_b(fx, 0x01E0_0440, D + 0xA4, OT, 6);
    blib::halo::halo(fx, 0x01E0_0E10, D + 0x254, OT, 3);
    blib::spike::spikes_b(fx, 0x01E0_1B60, D + 0x338, OT, 1);
    blib::drop::drops(fx, D + 0x3A0, OT, 0x64);
    blib::radiate::streaks_c(fx, 0x01E0_3410, D + 0x1E30, OT, 1);
    blib::drop::scatter_drops(fx, 0x01E0_43E0, D + 0x1E68, OT, 0x3C);
    let n = fx.mem.u16(D);
    if (0x37..0x4C).contains(&n) {
        6
    } else if n == 0x4C {
        2
    } else if n == 0x2D {
        1
    } else {
        0
    }
}
