//! BMODULE b224 (card 139), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06CD0, data area a0 = 0x1ED0000) [F] (helper A18,
//! 31 Oct 2026): b468's family (s3 = D, s0 = the OT) with the recompiled
//! streaks 391d5c6072b1 and arches bbe4d9b03097. On update 0x18 the
//! attacker's streaks record (+0x5B8: 0xB0 counts, flag 0, two directions
//! at node +0x10, ten rings, pairs (0x80, 0x80, 0x80) / far (0x80, 0x40,
//! 0x20)), its alpha +0x5CB pulsed from a 16-entry table (rodata
//! 0x1E07AB0, x 4) on 0x18..0xBC; on 0x74 an arc (+0xA4, nodes +0xC ->
//! +0x10, 0x14 wide, five points); on 0xBD the fade (+0xB30), an arch
//! (+0x5F0, arches_c) at the attacker turned to face the target, a halo
//! of 800 (+0x56C) and a spike (+0x504) at the target, 200 drops (+0xB38,
//! 0x44 each: 0x20 counts after rand & 0xF, 120 + (rand % 20 x 200 -
//! 2000) / 100 wide, spread +-1.15 / 0.8 +- 1.15 / +-1.15) and a shake
//! (+0x40) on the target's position; on 0xBD..0xCC the arch copied into a
//! free slot (count 12, still); the stage light by 0x1031C0 (+0xA0: down
//! on 2, up on 0x121). Returns 1 on update 0xB3, 2 on 0xBA, 6 on
//! 0xBD..0xD9, 8 on 0x127 (and at once for a parameter of -2 / -3). The
//! beads (+0x244, 8) are drawn but never spawned.
//!
//! Library (record addresses): `camera_shake` (ac7dccf5ae36) 0x1E00080,
//! `beads_at 40 44` 0x1E00440, `spikes_b` 0x1E01350, `halo` 0x1E02080,
//! `streaks_c` 0x1E02DD0, `arcs` 0x1E03DA0, `arches_c` 0x1E051E0, `fade_c`
//! 0x1E05BE0, `drops` 0x1E06150 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const ARCHES: u32 = D + 0x5F0;
const DROPS: u32 = D + 0xB38;
/// The alpha table (0x44 bytes of rodata copied to sp + 0x50 on entry).
const TABLE: u32 = 0x01E0_7AB0;

/// The entry 0x1E06CD0 (a0 = D, a1 = param; s3 = D, s1 = param, s0 = the
/// OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    for k in 0..0x11u32 {
        let w = fx.mem.read_u32(TABLE + 4 * k);
        fx.mem.write_u32(sp + 0x50 + 4 * k, w);
    }
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x4160);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (at, node, clear) in [(0xCu32, 6, 1), (0x10, 2, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x121);
    // The entry's s0 = the OT, s1 = 1 (the texture loop's), s2 = the
    // runner's 0 [H, as b201].
    rand_bytes(fx, D + 0x4058, 0x100, 0x01E0_6E18, [Some(OT), Some(1), Some(0)]);
    0
}

/// Update 0x18 [F 0x1E06E7C]: the attacker's streaks record.
fn streaks_start(fx: &mut Fx) {
    let r = D + 0x5B8;
    set_counter4(fx, r, 0xB0, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (9, 2), (0xA, 0xFF), (8, 0), (0xB, 0xA)] {
        fx.mem.write_u8(r + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(r + 0xC, h);
    for (o, v) in [(0x16u32, 0x20u8), (0x15, 0x40), (0x14, 0x80), (0x17, 0), (0x12, 0x80), (0x11, 0x80), (0x10, 0x80), (0x13, 0), (5, 1)] {
        fx.mem.write_u8(r + o, v);
    }
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(r + 6, b);
    fx.mem.write_u8(r + 8, 0x12);
}

/// Update 0x74 [F 0x1E06F14]: the arc (+0xA4), points +0x104.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0xA4;
    set_counter4(fx, g, 0x54, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x104);
    fx.mem.write_u8(g + 0x30, 0x14);
    fx.mem.write_u8(g + 0x31, 5);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F99_999A);
    fx.mem.write_u8(g + 0x34, 0);
    fx.mem.set16(g + 0x36, 0);
    fx.mem.write_u32(g + 0x40, 0x4120_0000);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0xC0), (0x4A, 0x10), (0x49, 0x20), (0x48, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        fx.mem.set16(g + 0xC + 2 * a, -1);
        a += 1;
    }
}

/// ((rand >> 4) % m + add) as f32 / 100 [F].
fn spread(fx: &mut Fx, ra: u32, m: i32, add: i32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h + add), 100.0)
}

/// Update 0xBD [F 0x1E06FF4]: the fade, the arch at the attacker (sp +
/// 0xB0), the halo and spike at the target (sp + 0xA0), the drops, the
/// shake.
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0xB30, 0xA, 0, 0xFF);
    fx.mem.write_u8(D + 0xB34, 1);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, sp + 0xB0);
    let a = ARCHES;
    set_counter4(fx, a, 0x10, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (0xE, 0x20), (0xD, 0x80), (0xC, 0xC0), (0xF, 0x10), (0xB, 0xC0), (8, 0xC0), (9, 0xC0), (0xA, 0xC0)] {
        fx.mem.write_u8(a + o, v);
    }
    fx.mem.write_u32(a + 0x10, 0x41A0_0000);
    fx.mem.write_u32(a + 0x14, 0x3FC0_0000);
    fx.mem.write_u32(a + 0x18, 0);
    copy_v4(fx, sp + 0xB0, a + 0x24);
    fx.mem.write_u32(a + 0x38, 0);
    fx.mem.write_u32(a + 0x34, 0);
    fx.mem.write_u32(a + 0x3C, 0xC110_0000);
    fx.mem.set32(a + 0x44, -0x4000);
    fx.mem.set32(a + 0x4C, 0x4000);
    if fx.mem.read_u32(D + 0x30) != 0x8000 {
        let v = ee::mul(fx.mem.f(a + 0x3C), -1.0);
        fx.mem.setf(a + 0x3C, v);
        let f = ee::mul(ee::from_i32(fx.mem.i32(a + 0x44)), -1.0);
        let r = ee::fixsfsi(f);
        fx.fix_residue(f, 0x01E0_7120);
        fx.mem.set32(a + 0x44, r);
        let z = ee::sub(fx.mem.f(a + 0x2C), 20.0);
        fx.mem.setf(a + 0x2C, z);
    } else {
        let z = ee::add(fx.mem.f(a + 0x2C), 20.0);
        fx.mem.setf(a + 0x2C, z);
    }
    let r = D + 0x56C;
    set_counter4(fx, r, 0x20, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u32(r + 0x10, 0x4448_0000);
    fx.mem.write_u32(r + 0x18, 0);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x10), (8, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, sp + 0xA0, r + 0x1C);
    let r = D + 0x504;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4020_0000);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_724C);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0x4058);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, sp + 0xA0, r + 0x28);
    fx.mem.write_u32(r + 0x58, 0x40);
    fx.mem.write_u32(r + 0x5C, 0x80);
    fx.mem.write_u32(r + 0x60, 0x100);
    for _ in 0..0xC8 {
        let i = first_free(fx, DROPS, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let r = DROPS + 0x44 * i as u32;
        let x = fx.rand_from(OT, 0x01E0_7300);
        set_counter4(fx, r, 0x20, (x >> 4) & 0xF, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 0), (0xB, 2), (0xA, 0), (9, 3), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_73F4) >> 4;
        let (_, h) = fx.div(x, 0x14);
        let w = ee::add(120.0, ee::div(ee::from_i32(h.wrapping_mul(200).wrapping_sub(2000)), 100.0));
        fx.mem.setf(r + 0x10, w);
        fx.mem.write_u32(r + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(r + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let v = spread(fx, 0x01E0_7500, 0xE6, -0x73);
        fx.mem.setf(r + 0x2C, v);
        let v = spread(fx, 0x01E0_7554, 0xE6, -0x73);
        fx.mem.setf(r + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), v));
        let v = spread(fx, 0x01E0_75BC, 0xE6, -0x73);
        fx.mem.setf(r + 0x34, v);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_7684);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xA0, u64::from(OT), 0x01E0_6E40);
    // The entry's s1 at the library calls: the parameter or the drop
    // loop's count.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0x127 => return 8,
        0xBD => {
            strike(fx, sp);
            s1 = 0xC8;
        }
        0x74 => arcs_start(fx),
        0x18 => streaks_start(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x18..0xBD).contains(&n) {
        let b = fx.mem.read_u8(sp + 0x50 + 4 * ((n - 0x18) & 0xF) as u32);
        fx.mem.write_u8(D + 0x5CB, b << 2);
    }
    let n = fx.mem.u16(D);
    if (0xBD..0xCD).contains(&n) {
        let i = first_free(fx, ARCHES, 0x54, 0x10);
        if i >= 0 {
            let r = ARCHES + 0x54 * i as u32;
            for k in 0..0x15u32 {
                let w = fx.mem.read_u32(ARCHES + 4 * k);
                fx.mem.write_u32(r + 4 * k, w);
            }
            fx.mem.write_u8(r, 0xC);
            for o in [0x3Cu32, 0x38, 0x34, 0x18] {
                fx.mem.write_u32(r + o, 0);
            }
        }
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_77F4, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_7804, [Some(u64::from(OT)), Some(s1)]);
    blib::fade::fade_c(fx, D + 0xB30, OT);
    // fade_c's frame (0xC0: s0 +0x10, s1 +0x20, s3 +0x40, ra +0x50): the
    // saved s3 = D is the stale XYZ low word the streaks' first TEXFLUSH
    // reports as a1 [run].
    fx.saved(sp - 0xC0, &[(0x10, u64::from(OT)), (0x20, s1), (0x40, u64::from(D)), (0x50, 0x01E0_7814)]);
    blib::arc::arcs(fx, 0x01E0_3DA0, D + 0xA4, OT, 1);
    blib::radiate::streaks_c(fx, 0x01E0_2DD0, D + 0x5B8, OT, 1);
    blib::spike::spikes_b(fx, 0x01E0_1350, D + 0x504, OT, 1);
    blib::halo::halo(fx, 0x01E0_2080, D + 0x56C, OT, 1);
    blib::beam::beads_at(fx, 0x01E0_0440, 0x40, 0x44, D + 0x244, OT, 8);
    blib::arch::arches_c(fx, 0x01E0_51E0, ARCHES, OT, 0x10);
    blib::drop::drops(fx, DROPS, OT, 0xC8);
    let n = fx.mem.u16(D);
    if (0xBD..0xDA).contains(&n) {
        6
    } else if n == 0xBA {
        2
    } else if n == 0xB3 {
        1
    } else {
        0
    }
}
