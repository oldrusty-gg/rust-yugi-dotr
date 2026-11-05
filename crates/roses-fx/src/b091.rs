//! BMODULE b091 (card 215), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05380, data area a0 = 0x1ED0000) [F] (helper B7, 31
//! Oct 2026): the "a0 = D" family with s0 = D, s1 = the OT (rand's and the
//! library frames' saved s0 is D), s2 the parameter, then loop counters.
//! The stage light pulsed by 0x1031C0 (+0xA0: from updates 2 / 0x90).
//! Bouncing sprites (+0x648, `sprite::bounce`, 200 x 0x44): on update 4 a
//! ring of 36 from the attacker's node 0x33, two a update on 2..36 from
//! nodes 0x33 / 0x29, on 89 a burst of 200 at the target; on 69..87 one
//! spiral swirl a update (+0x3B70, `swirl::arms`, 60 x 0x5C; the record's
//! 0x1E05180 at node 0x60), on 88 nine at the target; on 89 the fade
//! (+0x3B68), a flash (+0x384), three ribbons (+0x510) and a camera shake.
//! The arcs (+0xA4) and bars (+0x3D0) are drawn but never filled. Returns
//! 1 on update 79, 2 on 86, 6 on 89..109, 8 on 150 (and at once for -2 /
//! -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `arcs`
//! 0x1E00410, `flash` 0x1E01690, `bars` 0x1E02240, `ribbon` 0x1E02C50,
//! `bounce` 0x1E037E0, `fade` 0x1E04250, `arms` 0x1E04790 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x5100;
const SPRITES: u32 = D + 0x648;
const SWIRLS: u32 = D + 0x3B70;
const PI: u32 = 0x4049_0FDB;

/// The entry 0x1E05380 (a0 = D, a1 = param; s0 = D, s1 = the OT, s2 =
/// param).
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
    fx.bzero(D, 0x5200);
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
    for (at, node, clear) in [(0xCu32, 0x33, 1), (0x10, 0x29, 0), (0x14, 0x62, 0), (0x18, 0x60, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.write_u16(D + 0xA0, 2);
    fx.mem.write_u16(D + 0xA2, 0x90);
    // The entry's s0 = D, s1 = the OT, s2 = 2 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_54D4, [Some(D), Some(OT), Some(2)]);
    0
}

/// (rand() >> 4) % `m` (`div`, LO / HI kept) from return address `ra`.
fn rand_rem(fx: &mut Fx, ra: u32, m: i32) -> i32 {
    let x = fx.rand_from(D, ra) >> 4;
    fx.div(x, m).1
}

/// A sprite (0x44) at node position sp + 0x80 copied to +0x20.
fn at_node(fx: &mut Fx, sp: u32, r: u32, slot: u32, ra: u32) {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(slot);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(D), Some(u64::from(OT)), ra);
    for c in 0..3u32 {
        let v = fx.mem.read_u32(sp + 0x80 + 4 * c);
        fx.mem.write_u32(r + 0x20 + 4 * c, v);
    }
}

/// Update 4 [F 0x1E05538]: a ring of 36 sprites from node +0xC.
fn ring(fx: &mut Fx, sp: u32) {
    for k in 0..0x24 {
        let i = first_free(fx, SPRITES, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(D, 0x01E0_556C) >> 4) & 0xF;
        let r = SPRITES + 0x44 * i as u32;
        set_counter4(fx, r, 0x34, delay, 0xFF);
        let v = ((fx.rand_from(D, 0x01E0_55A0) >> 4) & 3) << 4;
        fx.mem.set8(r + 4, v);
        fx.mem.write_u8(r + 8, 0);
        fx.mem.write_u8(r + 0xB, 1);
        fx.mem.write_u8(r + 0xA, 5);
        let v = (fx.rand_from(D, 0x01E0_55D0) >> 4) & 3;
        fx.mem.set8(r + 9, v);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x50)] {
            fx.mem.write_u8(r + o, v);
        }
        let v = rand_rem(fx, 0x01E0_5600, 5).wrapping_mul(200) - 500;
        fx.mem.setf(r + 0x10, ee::add(50.0, ee::div(ee::from_i32(v), 100.0)));
        let v = rand_rem(fx, 0x01E0_566C, 3).wrapping_mul(200) - 300;
        fx.mem.setf(r + 0x14, ee::add(15.0, ee::div(ee::from_i32(v), 100.0)));
        fx.mem.write_u32(r + 0x18, 0xBCF5_C28F);
        fx.mem.write_u32(r + 0x1C, 0);
        at_node(fx, sp, r, D + 0xC, 0x01E0_56F4);
        let a = ee::mul(10.0, ee::from_i32(k));
        let v = rand_rem(fx, 0x01E0_5730, 0x28) - 0x14;
        let speed = ee::add(0.5, ee::div(ee::from_i32(v), 100.0));
        let a = ee::div(ee::mul(f32::from_bits(PI), a), 180.0);
        let c = libm_cosf(fx, a);
        fx.mem.setf(r + 0x2C, ee::mul(speed, c));
        let s = fx.libm_sinf(a);
        fx.mem.setf(r + 0x34, ee::mul(speed, s));
        fx.mem.write_u32(r + 0x30, 0x3F4C_CCCD);
    }
}

/// A swirl's common fields (0x5C): count `count` after `delay`, `rings`
/// rings, the outer alpha `outer`, at `at`, arm width +0x14 `width`.
fn swirl(fx: &mut Fx, r: u32, count: i32, delay: i32, rings: u8, outer: u8, at: u32, width: u32) {
    set_counter4(fx, r, count, delay, 0xFF);
    for (o, v) in [(4u32, 0u8), (5, 0), (8, 1), (9, 0), (0xA, rings)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x1C, 0x4100_0000);
    for (o, v) in [(0x26u32, 0x80u8), (0x25, 0x80), (0x24, 0x80), (0x27, 0xF0), (0x2A, 0x10), (0x29, 0x30), (0x28, 0x80), (0x2B, outer)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, at, r + 0x2C);
    fx.mem.write_u32(r + 0x18, 0x4170_0000);
    fx.mem.write_u32(r + 0x14, width);
}

/// The swirl's turn: about z, facing the attacker's side.
fn swirl_turn(fx: &mut Fx, r: u32) {
    let z = if fx.mem.read_u32(D + 0x30) == 0x8000 { -0x4000 } else { 0x4000 };
    fx.mem.set32(r + 0x54, z);
}

/// Update 88 [F 0x1E057FC]: nine swirls at the target (sp + 0x70), three
/// angles x three delays.
fn swirls9(fx: &mut Fx, sp: u32) {
    for j in 0..3 {
        for k in 0..3 {
            let i = first_free(fx, SWIRLS, 0x5C, 0x3C);
            if i < 0 {
                continue;
            }
            let r = SWIRLS + 0x5C * i as u32;
            swirl(fx, r, 0x14, (j + 1) & 0xFF, 0xC, 0, sp + 0x70, 0x40C0_0000);
            let a = ee::mul(120.0, ee::from_i32(k));
            fx.mem.setf(r + 0xC, a);
            let a = ee::add(45.0, fx.mem.f(r + 0xC));
            fx.mem.setf(r + 0x10, a);
            fx.mem.write_u32(r + 0x20, 0x3FE6_6666);
            swirl_turn(fx, r);
        }
    }
}

/// The record's 0x1E05180(D) [F] (169e17b1ad0f, b091 only): one swirl at
/// node +0x18 (its frame 0x50: the entry's s0 / s1 / s2 and ra saved), a
/// random start angle.
fn swirl_at_node(fx: &mut Fx, s2: u64) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x30, 0x01E0_6118), (0x20, s2), (0x10, u64::from(OT)), (0, u64::from(D))]);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x18);
    node_position_saving(fx, h, n, sp + 0x40, u64::from(D), Some(u64::from(OT)), 0x01E0_51AC);
    let i = first_free(fx, SWIRLS, 0x5C, 0x3C);
    if i >= 0 {
        let r = SWIRLS + 0x5C * i as u32;
        swirl(fx, r, 0xC, 0, 0x10, 0x20, sp + 0x40, 0x4140_0000);
        let x = fx.rand_from(0, 0x01E0_5298) >> 4;
        let v = fx.div(x, 0x168).1.wrapping_mul(200) - 0x8CA0;
        fx.mem.setf(r + 0xC, ee::add(360.0, ee::div(ee::from_i32(v), 100.0)));
        let a = ee::add(60.0, fx.mem.f(r + 0xC));
        fx.mem.setf(r + 0x10, a);
        fx.mem.write_u32(r + 0x20, 0x3F80_0000);
        swirl_turn(fx, r);
    }
    fx.leave(0x50);
}

/// Update 89 [F 0x1E0598C]: the fade, a flash and three ribbons at the
/// target (sp + 0x70), a burst of 200 sprites there and the shake.
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x70;
    set_counter4(fx, D + 0x3B68, 8, 0, 0xFF);
    fx.mem.write_u8(D + 0x3B6C, 1);
    let r = D + 0x384;
    set_counter4(fx, r, 0x20, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x447A_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x20), (8, 0xA0), (0xF, 0xD0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, at, r + 0x1C);
    for k in 0..3u32 {
        let r = D + 0x510 + 0x68 * k;
        fx.mem.write_u8(r + 4, 4);
        if k == 0 {
            set_counter4(fx, r, 0x1C, 0, 0xFF);
            fx.mem.write_u8(r + 0xA, 1);
            let f = fx.mem.read_u8(r + 4) | 1;
            fx.mem.write_u8(r + 4, f);
            fx.mem.write_u32(r + 0x14, 0x4020_0000);
            fx.mem.write_u32(r + 0x1C, 0x4220_0000);
            fx.mem.write_u8(r + 0x23, 0);
            fx.mem.write_u8(r + 0x27, 0x80);
            fx.mem.write_u8(r + 8, 0xA0);
            fx.mem.set32(r + 0x58, 0x40);
            fx.mem.set32(r + 0x5C, 0x80);
            fx.mem.set32(r + 0x60, 0x100);
        } else {
            set_counter4(fx, r, 0x20, 0, 0xFF);
            fx.mem.write_u8(r + 0xA, 1);
            let f = fx.mem.read_u8(r + 4) | 2;
            fx.mem.write_u8(r + 4, f);
            fx.mem.write_u32(r + 0x14, 0x3FC0_0000);
            fx.mem.write_u32(r + 0x1C, 0x42C8_0000);
            fx.mem.write_u32(r + 0x18, 0x3ECC_CCCD);
            fx.mem.write_u8(r + 0x23, 0x80);
            fx.mem.write_u8(r + 0x27, 0x40);
            fx.mem.write_u8(r + 8, 0x19);
            fx.mem.set32(r + 0x50, if k & 1 != 0 { 0x4000 } else { 0xC000 });
        }
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 3);
        let x = fx.rand_from(D, 0x01E0_5BB8);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
    }
    for _ in 0..0xC8 {
        let i = first_free(fx, SPRITES, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(D, 0x01E0_5C50) >> 4) & 0xF;
        let r = SPRITES + 0x44 * i as u32;
        set_counter4(fx, r, 0x18, delay, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 1), (0xB, 2), (0xA, 0), (9, 3), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        let v = rand_rem(fx, 0x01E0_5CBC, 0x14).wrapping_mul(200) - 2000;
        fx.mem.setf(r + 0x10, ee::add(100.0, ee::div(ee::from_i32(v), 100.0)));
        fx.mem.write_u32(r + 0x14, 0xBF80_0000);
        fx.mem.write_u32(r + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3u32 {
            let v = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, v);
        }
        let v = rand_rem(fx, 0x01E0_5D4C, 0xB4) - 0x5A;
        fx.mem.setf(r + 0x2C, ee::div(ee::from_i32(v), 100.0));
        let v = rand_rem(fx, 0x01E0_5D98, 0xB4) - 0x5A;
        fx.mem.setf(r + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), ee::div(ee::from_i32(v), 100.0)));
        let v = rand_rem(fx, 0x01E0_5DF4, 0xB4) - 0x5A;
        fx.mem.setf(r + 0x34, ee::div(ee::from_i32(v), 100.0));
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FC0_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}

/// Updates 2..36 [F 0x1E05EF0]: a sprite from each of nodes +0xC / +0x10.
fn trickle(fx: &mut Fx, sp: u32) {
    for k in 0..2u32 {
        let i = first_free(fx, SPRITES, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let r = SPRITES + 0x44 * i as u32;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        let v = ((fx.rand_from(D, 0x01E0_5F4C) >> 4) & 3) << 4;
        fx.mem.set8(r + 4, v);
        fx.mem.write_u8(r + 8, 0);
        fx.mem.write_u8(r + 0xA, 5);
        fx.mem.write_u8(r + 0xB, 1);
        let v = (fx.rand_from(D, 0x01E0_5F7C) >> 4) & 3;
        fx.mem.set8(r + 9, v);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x40)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x41F0_0000);
        fx.mem.write_u32(r + 0x14, 0x4110_0000);
        fx.mem.write_u32(r + 0x1C, 0);
        fx.mem.write_u32(r + 0x18, 0);
        at_node(fx, sp, r, D + 0xC + 4 * k, 0x01E0_5FD8);
        for (o, ra) in [(0x2Cu32, 0x01E0_5FFCu32), (0x30, 0x01E0_6048), (0x34, 0x01E0_6094)] {
            let v = rand_rem(fx, ra, 0xA) - 5;
            fx.mem.setf(r + o, ee::div(ee::from_i32(v), 100.0));
        }
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x70, u64::from(D), 0x01E0_54FC);
    match fx.mem.u16(D) {
        0x96 => return 8,
        0x59 => strike(fx, sp),
        0x58 => swirls9(fx, sp),
        4 => ring(fx, sp),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0x25).contains(&n) {
        trickle(fx, sp);
    }
    let n = fx.mem.u16(D);
    if (0x45..0x58).contains(&n) {
        swirl_at_node(fx, i64::from(param) as u64);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_6124, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6134, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0x3B68, OT);
    blib::arc::arcs(fx, 0x01E0_0410, D + 0xA4, OT, 1);
    blib::star::flash(fx, 0x01E0_1690, D + 0x384, OT, 1);
    blib::cross::bars(fx, 0x01E0_2240, D + 0x3D0, OT, 4);
    blib::streak::ribbon(fx, 0x01E0_2C50, D + 0x510, OT, 3);
    blib::sprite::bounce(fx, SPRITES, OT, 0xC8);
    blib::swirl::arms(fx, 0x01E0_4790, SWIRLS, OT, 0x3C);
    let n = fx.mem.u16(D);
    if (0x59..0x6E).contains(&n) {
        6
    } else if n == 0x56 {
        2
    } else if n == 0x4F {
        1
    } else {
        0
    }
}
