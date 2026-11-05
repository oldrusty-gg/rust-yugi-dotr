//! BMODULE b179 (card 653), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03F60, data area a0 = 0x1ED0000) [F] (helper A17,
//! 31 Oct 2026): b201's family (the "a0 = D" records compiled without
//! scheduling) with the recompiled streaks 391d5c6072b1 and a frame fade
//! of its own (7e28b625d220). On updates 8..98 up to seven drops an update
//! (+0xA4, 0x44 each, 300 slots; 0x14 + rand & 3 counts, layout (rand & 3)
//! << 4, 200 wide, falling 10 (0x1B 1/50 a square), tinted 0x50) on the
//! attacker's nodes +0xC..+0x24 (node_position); on updates 104 and 126
//! eighty puffs round the target (0x1C counts after rand & 7, 120 +- 20
//! wide, 12 +- 2 x 2 tall, blend 1, thrown +-9 and drifting a 45th of it);
//! on updates 105 and 127 the frame fade (+0x5320: 24 updates, 6 in, blend
//! 2, 16 high, the corners pushed out 8, (0x60, 0x80, 0x40)), the
//! attacker's streaks record (+0x5350: flag 0x80, eight directions, one
//! ring, pairs 0x80 / alphas 0x40), a spike (+0x5388, 0x68 each, the slot
//! taken without the free test), a halo of 50 .. 60 (+0x52D4), 100 drops
//! (0x18 counts after rand & 7, layout 0x42, blend 2 + rand & 3, 40 wide,
//! spread +-1.25 / 0.8 +- 0.9 / +-1.25) and a shake (+0x40); the stage
//! light by 0x1031C0 (+0xA0: down on update 90, up on 153). Returns 1 on
//! update 95, 6 on 105..139, 2 on 140, 8 on 159 (and at once for a
//! parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` (ac7dccf5ae36) 0x1E00080,
//! `drops` 0x1E00440, `halo` 0x1E00FC0, `spikes_b` 0x1E01D10, `streaks_c`
//! 0x1E02A40, `frame_fade_c` 0x1E03A10 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const DROPS: u32 = D + 0xA4;

/// The entry 0x1E03F60 (a0 = D, a1 = param; s3 = D, s1 = param, s0 = the
/// OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x5560);
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
    for (k, (node, clear)) in [(0x12, 1), (0x1A, 0), (0x22, 0), (0x2A, 0), (0x32, 0), (0x3A, 0), (0x42, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    fx.mem.set16(D + 0xA0, 0x5A);
    fx.mem.set16(D + 0xA2, 0x99);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's), s2 = 0 (the
    // runner's [H, as b201]).
    rand_bytes(fx, D + 0x5458, 0x100, 0x01E0_40F4, [Some(OT), Some(3), Some(0)]);
    0
}

/// ((rand >> 4) % m - m / 2) / 100 [F].
fn spread(fx: &mut Fx, ra: u32, m: i32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h - m / 2), 100.0)
}

/// Updates 104 / 126 [F 0x1E04164]: eighty puffs round the target `at`.
fn puffs(fx: &mut Fx, sp: u32, at: u32) {
    for _ in 0..0x50 {
        let i = first_free(fx, DROPS, 0x44, 0x12C);
        if i < 0 {
            continue;
        }
        let r = DROPS + 0x44 * i as u32;
        let x = fx.rand_from(OT, 0x01E0_4198);
        set_counter4(fx, r, 0x1C, (x >> 4) & 7, 0xFF);
        let x = fx.rand_from(OT, 0x01E0_41CC);
        fx.mem.set8(r + 4, ((x >> 4) & 3) << 4);
        for (o, v) in [(8u32, 0u8), (0xE, 0x80), (0xD, 0xA0), (0xC, 0xC0)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_4250);
        fx.mem.set8(r + 0xF, ((x >> 4) & 0x3F) + 0x20);
        let x = fx.rand_from(OT, 0x01E0_4278) >> 4;
        let (_, h) = fx.div(x, 0x14);
        let v = ee::add(120.0, ee::div(ee::from_i32(h.wrapping_mul(200) - 2000), 100.0));
        fx.mem.setf(r + 0x10, v);
        // A signed `% 4` of the shifted rand.
        let x = fx.rand_from(OT, 0x01E0_42F0) >> 4;
        let mut h = x & 3;
        if x < 0 && h != 0 {
            h -= 4;
        }
        let v = ee::add(12.0, ee::div(ee::from_i32(h * 200 - 400), 100.0));
        fx.mem.setf(r + 0x14, v);
        fx.mem.write_u8(r + 0xB, 1);
        fx.mem.write_u8(r + 0xA, 5);
        let x = fx.rand_from(OT, 0x01E0_439C);
        fx.mem.set8(r + 9, (x >> 4) & 3);
        fx.mem.write_u32(r + 0x18, 0);
        fx.mem.write_u32(r + 0x1C, 0);
        for (k, ra) in [0x01E0_43E8u32, 0x01E0_4434, 0x01E0_4480].into_iter().enumerate() {
            let v = spread(fx, ra, 0x708);
            fx.mem.setf(sp + 0x60 + 4 * k as u32, v);
        }
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(at + 4 * c), fx.mem.f(sp + 0x60 + 4 * c));
            fx.mem.setf(r + 0x20 + 4 * c, v);
        }
        for c in 0..3u32 {
            let v = ee::div(fx.mem.f(sp + 0x60 + 4 * c), 45.0);
            fx.mem.setf(r + 0x2C + 4 * c, v);
        }
    }
}

/// Updates 105 / 127 [F 0x1E045A4]: the frame fade, the streaks record,
/// a spike, the halo, 100 drops and the shake at the target `at`.
fn strike(fx: &mut Fx, at: u32) {
    let n = fx.mem.u16(D);
    let w = D + 0x5320;
    fx.mem.set16(w, n);
    fx.mem.set16(w + 2, n + 0x18);
    fx.mem.write_u8(w + 5, 6);
    fx.mem.write_u8(w + 6, 2);
    fx.mem.write_u32(w + 8, 0x4180_0000);
    for (o, v) in [(0x14u32, -8i32), (0xC, -8), (0x18, 8), (0x10, 8), (0x20, -8), (0x1C, -8), (0x28, 8), (0x24, 8)] {
        fx.mem.set32(w + o, v);
    }
    for (o, v) in [(0x2Eu32, 0x40u8), (0x2D, 0x80), (0x2C, 0x60)] {
        fx.mem.write_u8(w + o, v);
    }
    let r = D + 0x5350;
    set_counter4(fx, r, 0x20, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0x80);
    for (o, v) in [(9u32, 1u8), (0xA, 0xFF), (8, 0), (0xB, 1)] {
        fx.mem.write_u8(r + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(r + 0xC, h);
    for (o, v) in [(0x14u32, 0x80u8), (0x15, 0x80), (0x16, 0x80), (0x17, 0x40), (0x10, 0x80), (0x11, 0x80), (0x12, 0x80), (0x13, 0x40)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u8(r + 5, 1);
    let node = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, node);
    // The spike slot is taken without the free test (-1 lands below the
    // pool).
    let i = first_free(fx, D + 0x5388, 0x68, 2);
    let r = D.wrapping_add(0x5388).wrapping_add((0x68 * i) as u32);
    set_counter4(fx, r, 0x12, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 5), (0xA, 2), (0xC, 6)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0x4060_0000);
    fx.mem.write_u32(r + 0x1C, 0x41A0_0000);
    fx.mem.write_u8(r + 0x23, 0);
    fx.mem.write_u8(r + 0x27, 0x60);
    fx.mem.write_u8(r + 8, 0x28);
    fx.mem.write_u32(r + 0x58, 0);
    fx.mem.write_u32(r + 0x5C, 0);
    fx.mem.write_u32(r + 0x60, 0x300);
    fx.mem.write_u8(r + 0xB, 2);
    let x = fx.rand_from(OT, 0x01E0_487C);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0x5458);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    copy_v4(fx, at, r + 0x28);
    let g = D + 0x52D4;
    set_counter4(fx, g, 0x12, 0, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x80);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x18, 0x4190_0000);
    for (o, v) in [(0xFu32, 0xD0u8), (0xC, 0xD0), (0xD, 0xD0), (0xE, 0xD0), (0xA, 0x10), (9, 0x80), (8, 0x10)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 0x14, 0x4270_0000);
    fx.mem.write_u32(g + 0x10, 0x4248_0000);
    copy_v4(fx, at, g + 0x1C);
    for _ in 0..0x64 {
        let i = first_free(fx, DROPS, 0x44, 0x12C);
        if i < 0 {
            continue;
        }
        let r = DROPS + 0x44 * i as u32;
        let x = fx.rand_from(OT, 0x01E0_4A6C);
        set_counter4(fx, r, 0x18, (x >> 4) & 7, 0xFF);
        for (o, v) in [(4u32, 0x42u8), (8, 2), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_4AFC);
        fx.mem.set8(r + 9, ((x >> 4) & 3) + 2);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u32(r + 0x10, 0x4220_0000);
        fx.mem.write_u32(r + 0x14, 0xBE4C_CCCD);
        fx.mem.write_u32(r + 0x18, 0xBDA3_D70A);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let v = spread(fx, 0x01E0_4C24, 0xFA);
        fx.mem.setf(r + 0x2C, v);
        let v = spread(fx, 0x01E0_4C78, 0xB4);
        fx.mem.setf(r + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), v));
        let v = spread(fx, 0x01E0_4CE0, 0xFA);
        fx.mem.setf(r + 0x34, v);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x12, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x4000_0000);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, s + 0x20);
}

/// Updates 8..98 [F 0x1E04DEC]: a drop from each of the seven nodes.
fn sprinkle(fx: &mut Fx, sp: u32) {
    for s1 in 0..7i32 {
        let i = first_free(fx, DROPS, 0x44, 0x12C);
        if i < 0 {
            continue;
        }
        let r = DROPS + 0x44 * i as u32;
        let x = fx.rand_from(OT, 0x01E0_4E20);
        set_counter4(fx, r, (((x >> 4) & 3) + 0x14) & 0xFF, 0, 0xFF);
        let x = fx.rand_from(OT, 0x01E0_4E5C);
        fx.mem.set8(r + 4, ((x >> 4) & 3) << 4);
        for (o, v) in [(8u32, 0u8), (0xB, 1), (0xA, 5), (9, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x50)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x4348_0000);
        fx.mem.write_u32(r + 0x14, 0x4120_0000);
        fx.mem.write_u32(r + 0x18, 0xBCA3_D70A);
        fx.mem.write_u32(r + 0x1C, 0);
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0xC + 4 * s1 as u32);
        // The entry's s0 = the OT, s1 = the node counter.
        node_position_saving(fx, h, slot, sp + 0x60, u64::from(OT), Some(s1 as u64), 0x01E0_4FAC);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let v = spread(fx, 0x01E0_4FFC, 0x14);
        fx.mem.setf(r + 0x2C, v);
        fx.mem.write_u32(r + 0x30, 0x3E4C_CCCD);
        let v = spread(fx, 0x01E0_506C, 0x14);
        fx.mem.setf(r + 0x34, v);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, then the last loop's count.
    let mut s1 = param as i64 as u64;
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    // The entry's s0 = the OT: its save in 0x1F71A0's and camera_shake's
    // frames (sp - 0x20) is the drops' missing-texture TEX1 [as b128].
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_411C);
    match fx.mem.u16(D) {
        0x9F => return 8,
        0x7F | 0x69 => {
            strike(fx, sp + 0x50);
            s1 = 0x64;
        }
        0x7E | 0x68 => {
            puffs(fx, sp, sp + 0x50);
            s1 = 0x50;
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (8..0x63).contains(&n) {
        sprinkle(fx, sp);
        s1 = 7;
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_50DC, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_50EC, [Some(u64::from(OT)), Some(s1)]);
    blib::drop::drops(fx, DROPS, OT, 0x12C);
    blib::halo::halo(fx, 0x01E0_0FC0, D + 0x52D4, OT, 1);
    blib::spike::spikes_b(fx, 0x01E0_1D10, D + 0x5388, OT, 2);
    let (_, a3) = blib::radiate::streaks_c_a3(fx, 0x01E0_2A40, D + 0x5350, OT, 1);
    let n = fx.mem.u16(D);
    // The streaks record counts through both fade windows [H: a3 = 0
    // otherwise].
    blib::window::frame_fade_c(fx, 0x01E0_3A10, D + 0x5320, OT, n, a3.unwrap_or(0));
    if (0x69..0x8C).contains(&n) {
        6
    } else if n == 0x8C {
        2
    } else if n == 0x5F {
        1
    } else {
        0
    }
}
