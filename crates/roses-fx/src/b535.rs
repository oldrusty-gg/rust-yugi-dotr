//! BMODULE b535 (card 457), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04B50, data area a0 = 0x1ED0000) [F] (helper C26,
//! 3 Nov 2026): b469's family (s0 = D, s1 = the OT). On update 36 a band
//! (+0x250, the first free of 32) at node 0x12, a bead beam (+0x43E0, its
//! delay / +9 the band's slot) and a camera shake at the band's point (band
//! slot 0's); on 50 two domes (+0x160: sizes 3 / 4, the second turned
//! 0x8000 about z, both spinning 0x80 a update about z) whose alpha grows 6
//! a update to 0x60 and which sit at node 0xA aimed away from the target
//! (0x101940) while the first counts; on 63 at the target the fade
//! (+0x158), a ribbon (+0xF0), 200 bouncing sprites (+0x530), four bands
//! (+0x250), a wide flash (+0xA4) and a shake; on 76 a ring of 36 sprites
//! (+0x3A50) thrown out from node 0xA's position 15 lower in 10-degree
//! steps; on updates 51..62 a full-window tint (0xA0, 0xA0, 0xA0, 0x40) of
//! the frame drawn (0x2079D0). The stage light pulsed by 0x1031C0 (+0xA0:
//! down on update 50, up on 114). Returns 1 on update 53, 6 on 63..91, 2
//! on 92, 8 on 120 (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `fade`
//! 0x1E00410, `flash` 0x1E00950, `domes_550` 0x1E01500, `ribbon`
//! 0x1E01F90, `bands` 0x1E02B20, `bounce` 0x1E03310, `beads` 0x1E03D80
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{aim_angles, first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x4438;
const DOMES: u32 = D + 0x160;
const BANDS: u32 = D + 0x250;
const SPRITES: u32 = D + 0x530;
const RING: u32 = D + 0x3A50;
const BEAD: u32 = D + 0x43E0;

/// The entry 0x1E04B50 (a0 = D, a1 = param; s0 = D, s1 = the OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x4540);
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
    for (at, node, clear) in [(0xCu32, 0xA, 1), (0x10, 0x2D, 0), (0x14, 0x12, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 0x32);
    fx.mem.set16(D + 0xA2, 0x72);
    // The entry's s0 = D, s1 = the OT, s2 = 3 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4C94, [Some(D), Some(OT), Some(3)]);
    0
}

/// The shake (+0x40) at the point `at` (count 0x20, kind 1).
fn shake_at(fx: &mut Fx, at: u32) {
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

/// Update 36 [F 0x1E04D04]: one band at node +0x14 (the first free of 32),
/// the bead beam (its delay / +9 the band's slot, kept as the original
/// does: -1 when the pool is full) and the shake at band 0's point.
fn band_start(fx: &mut Fx) {
    let mut i = 0;
    for s2 in 0..1i32 {
        i = first_free(fx, BANDS, 0x5C, 0x20);
        if i < 0 {
            continue;
        }
        let r = BANDS + 0x5C * i as u32;
        set_counter4(fx, r, 0x20, (2 * s2) & 0xFF, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 7, 0x24);
        fx.mem.write_u8(r + 0xC, 2);
        fx.mem.write_u8(r + 8, 0);
        fx.mem.write_u8(r + 9, 2);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xB, 5);
        fx.mem.write_u32(r + 0x10, 0x4040_0000);
        fx.mem.write_u32(r + 0x14, 0x4020_0000);
        for o in [0x28u32, 0x29, 0x2A, 0x24, 0x25, 0x26, 0x2B, 0x27] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.set32(r + 0x3C, 0x4000);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        fx.mem.write_u32(r + 0x1C, 0x3F66_6666);
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0x14);
        node_position_saving(fx, h, n, r + 0x2C, u64::from(D), Some(u64::from(OT)), 0x01E0_4DF4);
    }
    let r = BEAD;
    set_counter4(fx, r, 0x20, i & 0xFF, 0xFF);
    let c = fx.mem.read_u8(r).wrapping_sub(4);
    fx.mem.write_u8(r, c);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 8, 0x50);
    fx.mem.set8(r + 9, i);
    fx.mem.write_u8(r + 0xA, 0);
    fx.mem.write_u8(r + 0xC, 2);
    fx.mem.write_u8(r + 0xB, 3);
    fx.mem.write_u32(r + 0x1C, 0x4270_0000);
    fx.mem.write_u32(r + 0x20, 0xBF4C_CCCD);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x14, 0x3FC0_0000);
    for o in [0x27u32, 0x24, 0x25, 0x26] {
        fx.mem.write_u8(r + o, 0x80);
    }
    copy_v4(fx, BANDS + 0x2C, r + 0x28);
    shake_at(fx, BANDS + 0x2C);
}

/// Update 50 [F 0x1E04F2C]: the two domes.
fn domes_start(fx: &mut Fx) {
    for s2 in 0..2u32 {
        let r = DOMES + 0x50 * s2;
        set_counter4(fx, r, 0x11, 0, 0xFF);
        fx.mem.write_u8(r + 0xA, 4);
        fx.mem.write_u8(r + 7, 9);
        fx.mem.write_u8(r + 8, 1);
        fx.mem.write_u8(r + 9, 2);
        fx.mem.write_u32(r + 0xC, 0x4040_0000);
        fx.mem.write_u32(r + 0x14, 0x4160_0000);
        if s2 == 1 {
            fx.mem.write_u32(r + 0x10, 0xBD23_D70A);
            fx.mem.write_u32(r + 0xC, 0x4080_0000);
            fx.mem.set32(r + 0x38, 0x8000);
            fx.mem.set32(r + 0x48, 0x80);
        } else {
            fx.mem.write_u32(r + 0x10, 0xBDA3_D70A);
            fx.mem.write_u32(r + 0xC, 0x4040_0000);
            fx.mem.set32(r + 0x48, 0x80);
        }
        fx.mem.write_u32(r + 0x18, 0);
        for o in [0x1Cu32, 0x1D, 0x1E] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x1F, 0);
    }
}

/// ((rand >> 4) % m x `mul` + `add`) / 100.
fn jitter(fx: &mut Fx, ra: u32, m: i32, mul: i32, add: i32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let (_, r) = fx.div(x, m);
    ee::div(ee::from_i32(r.wrapping_mul(mul).wrapping_add(add)), 100.0)
}

/// Update 63 [F 0x1E0502C]: at the target (sp + 0xA0) the fade, a ribbon,
/// 200 bouncing sprites, four bands, the wide flash and the shake.
fn strike(fx: &mut Fx, sp: u32) {
    let f = D + 0x158;
    set_counter4(fx, f, 0xC, 0, 0xFF);
    fx.mem.write_u8(f + 4, 1);
    let r = D + 0xF0;
    set_counter4(fx, r, 0x1E, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x400C_CCCD);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 0xA, 0);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 3);
    fx.mem.write_u8(r + 8, 0xA0);
    let x = fx.rand_from(D, 0x01E0_50A8);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    fx.mem.write_u8(r + 0x23, 0);
    fx.mem.write_u8(r + 0x27, 0x60);
    copy_v4(fx, sp + 0xA0, r + 0x28);
    fx.mem.set32(r + 0x58, 0x40);
    fx.mem.set32(r + 0x5C, 0x80);
    fx.mem.set32(r + 0x60, 0x100);
    for _ in 0..0xC8 {
        let i = first_free(fx, SPRITES, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let x = fx.rand_from(D, 0x01E0_5154) >> 4;
        let r = SPRITES + 0x44 * i as u32;
        set_counter4(fx, r, 0x20, x & 0xF, 0xFF);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u8(r + 8, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xA, 0);
        let x = fx.rand_from(D, 0x01E0_51A0) >> 4;
        fx.mem.set8(r + 9, x & 7);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let j = jitter(fx, 0x01E0_51CC, 0x14, 200, -0x7D0);
        fx.mem.setf(r + 0x10, ee::add(60.0, j));
        fx.mem.write_u32(r + 0x14, 0xBECC_CCCD);
        fx.mem.write_u32(r + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let j = jitter(fx, 0x01E0_526C, 0xE6, 1, -0x73);
        fx.mem.setf(r + 0x2C, j);
        let j = jitter(fx, 0x01E0_52B8, 0xE6, 1, -0x73);
        fx.mem.setf(r + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), j));
        let j = jitter(fx, 0x01E0_5314, 0xE6, 1, -0x73);
        fx.mem.setf(r + 0x34, j);
    }
    for s3 in 0..4u32 {
        let r = BANDS + 0x5C * s3;
        set_counter4(fx, r, 0x20, ((2 * s3) & 0xFF) as i32, 0xFF);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u8(r + 7, 0x24);
        fx.mem.write_u8(r + 0xC, 1);
        fx.mem.write_u8(r + 8, 1);
        fx.mem.write_u8(r + 9, 2);
        fx.mem.write_u8(r + 0xA, 6);
        fx.mem.write_u8(r + 0xB, 0);
        fx.mem.write_u32(r + 0x10, 0x4000_0000);
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0x4000_0000);
        fx.mem.write_u32(r + 0x1C, 0x3F80_0000);
        fx.mem.write_u32(r + 0x20, 0x3F00_0000);
        for o in [0x28u32, 0x29, 0x2A, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x27, 0x10);
        fx.mem.write_u8(r + 0x2B, 0x40);
        copy_v4(fx, sp + 0xA0, r + 0x2C);
        fx.mem.set32(r + 0x3C, 0x4000);
    }
    let r = D + 0xA4;
    set_counter4(fx, r, 0x20, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x70), (9, 0x60), (8, 0x10)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0);
    fx.mem.write_u32(r + 0x10, 0x4448_0000);
    copy_v4(fx, sp + 0xA0, r + 0x1C);
    shake_at(fx, sp + 0xA0);
}

/// Update 76 [F 0x1E0556C]: nodes +0xC / +0x10 into sp + 0x80 / 0x90, a
/// ring of 36 sprites thrown out from node +0xC's position 15 lower.
fn ring(fx: &mut Fx, sp: u32) {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(D), Some(u64::from(OT)), 0x01E0_5580);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x10);
    node_position_saving(fx, h, n, sp + 0x90, u64::from(D), Some(u64::from(OT)), 0x01E0_5594);
    for s3 in 0..0x24i32 {
        let x = fx.rand_from(D, 0x01E0_55A8) >> 4;
        let r = RING + 0x44 * s3 as u32;
        set_counter4(fx, r, 0x28, x & 7, 0xFF);
        let x = fx.rand_from(D, 0x01E0_55DC) >> 4;
        fx.mem.set8(r + 4, (x & 3) << 4);
        fx.mem.write_u8(r + 8, 2);
        fx.mem.write_u8(r + 0xB, 1);
        fx.mem.write_u8(r + 0xA, 5);
        let x = fx.rand_from(D, 0x01E0_5610) >> 4;
        fx.mem.set8(r + 9, x & 3);
        for o in [0xCu32, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0xF, 0x50);
        let j = jitter(fx, 0x01E0_5640, 3, 200, -0x12C);
        fx.mem.setf(r + 0x10, ee::add(10.0, j));
        let j = jitter(fx, 0x01E0_56AC, 3, 200, -0x12C);
        fx.mem.setf(r + 0x14, ee::add(15.0, j));
        fx.mem.write_u32(r + 0x18, 0xBCA3_D70A);
        fx.mem.write_u32(r + 0x1C, 0);
        let j = jitter(fx, 0x01E0_5728, 0x12C, 1, -0x96);
        let v = ee::add(fx.mem.f(sp + 0x80), j);
        fx.mem.setf(r + 0x20, v);
        let v = ee::sub(fx.mem.f(sp + 0x84), 15.0);
        fx.mem.setf(r + 0x24, v);
        let j = jitter(fx, 0x01E0_5790, 0x12C, 1, -0x96);
        let v = ee::add(fx.mem.f(sp + 0x88), j);
        fx.mem.setf(r + 0x28, v);
        let f20 = ee::mul(10.0, ee::from_i32(s3));
        let j = jitter(fx, 0x01E0_57F8, 0x50, 1, -0x28);
        let f21 = ee::add(0.5, j);
        let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), f20), 180.0);
        let c = libm_cosf(fx, a);
        fx.mem.setf(r + 0x2C, ee::mul(f21, c));
        let s = fx.libm_sinf(a);
        fx.mem.setf(r + 0x34, ee::mul(f21, s));
        fx.mem.write_u32(r + 0x30, 0x3ECC_CCCD);
    }
}

/// While the first dome counts [F 0x1E058D0]: the domes' alpha up 6 to
/// 0x60, both at node +0xC aimed along node +0xC - the target.
fn domes_follow(fx: &mut Fx, sp: u32) {
    for o in [0x17Fu32, 0x1CF] {
        let a = fx.mem.u8(D + o);
        if a < 0x60 {
            fx.mem.set8(D + o, a + 6);
        }
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(D), Some(u64::from(OT)), 0x01E0_5920);
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(sp + 0x80 + 4 * c), fx.mem.f(sp + 0xA0 + 4 * c));
        fx.mem.setf(sp + 0xB0 + 4 * c, v);
    }
    aim_angles(fx, sp + 0xB0, D + 0x190);
    copy_v4(fx, D + 0x190, D + 0x1E0);
    copy_v4(fx, sp + 0x80, D + 0x1D0);
    copy_v4(fx, D + 0x1D0, D + 0x180);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xA0, u64::from(D), 0x01E0_4CBC);
    match fx.mem.u16(D) {
        0x78 => return 8,
        0x4C => ring(fx, sp),
        0x3F => strike(fx, sp),
        0x32 => domes_start(fx),
        0x24 => band_start(fx),
        _ => {}
    }
    if fx.mem.read_u8(DOMES) != 0 {
        domes_follow(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_59D0, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_59E0, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0x158, OT);
    blib::star::flash(fx, 0x01E0_0950, D + 0xA4, OT, 1);
    blib::streak::ribbon(fx, 0x01E0_1F90, D + 0xF0, OT, 1);
    blib::dome::domes_550(fx, 0x01E0_1500, DOMES, OT, 3);
    blib::band::bands(fx, 0x01E0_2B20, BANDS, OT, 8);
    blib::sprite::bounce(fx, SPRITES, OT, 0xC8);
    blib::sprite::bounce(fx, RING, OT, 0x24);
    blib::beam::beads(fx, 0x01E0_3D80, BEAD, OT, 1);
    let n = fx.mem.u16(D);
    if (0x33..0x3F).contains(&n) {
        for o in [0xCCu32, 0xCD, 0xCE] {
            fx.mem.write_u8(sp + o, 0xA0);
        }
        fx.mem.write_u8(sp + 0xCF, 0x40);
        let a = fx.alpha_preset(0);
        fx.frame_pass(OT, 0, 1, a, sp + 0xCC, 0x01E0_5AD8);
    }
    let n = fx.mem.u16(D);
    if (0x3F..0x5C).contains(&n) {
        6
    } else if n == 0x5C {
        2
    } else if n == 0x35 {
        1
    } else {
        0
    }
}
