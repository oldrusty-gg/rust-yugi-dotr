//! BMODULE b380 (card 454), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04DF0, data area a0 = 0x1ED0000) [F] (helper A19,
//! 1 Nov 2026): b468's family (s3 = D, s1 = the parameter then the loop
//! counts, s0 = the OT, s2 the last free slot) with the recompiled swirl
//! arms 1c4c37ddd46b (`swirl::arms_c`) and sheets f694ab0da4da
//! (`sheet::sheets_c`). On 20..48 two drops an update (+0x2598: 0x16
//! counts, 50 wide, from the attacker's node slot +0x14); on 70..109 a
//! swirl arm an update by the local 0x1E04A50 (+0xF0, 0x5C each, 100
//! slots, at node slot +0x10, from 360 + (rand % 360 x 200 - 36000) / 100
//! degrees over 60); on 90 the fade (+0x5AB8), three sheets at the target
//! (+0x5AC0, y - 1 + k), a halo of 50 (+0xA4), a spike (+0x2530), 200
//! drops and a shake, the shake's position the target's (+0x60). The
//! stage light by 0x1031C0 (+0xA0: down on 2, up on 0x90). Returns 1 on
//! 80, 6 on 90..118, 2 on 119, 8 on 150 (and at once for -2 / -3). The bars
//! +0x24E0 are drawn but never spawned.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `halo`
//! 0x1E00440, `arms_c` 0x1E01190, `bars_c` 0x1E01B90, `spikes_b`
//! 0x1E02660, `drops` 0x1E03390, `fade_c` 0x1E03F10, `sheets_c` 0x1E04480
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const HALO: u32 = D + 0xA4;
const ARMS: u32 = D + 0xF0;
const BARS: u32 = D + 0x24E0;
const SPIKE: u32 = D + 0x2530;
const DROPS: u32 = D + 0x2598;
const FADE: u32 = D + 0x5AB8;
const SHEETS: u32 = D + 0x5AC0;
const BYTES: u32 = D + 0x5E80;

/// The entry 0x1E04DF0 (a0 = D, a1 = param).
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
    fx.bzero(D, 0x5F80);
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
    for (o, node, clear) in [(0xCu32, 1, 1), (0x10, 4, 0), (0x14, 0x52, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + o, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x90);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's), s2 = the
    // runner's 0 [H, as b201].
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4F24, [Some(OT), Some(3), Some(0)]);
    0
}

/// ((rand >> 4) % m + add) / 100 [F].
fn spread(fx: &mut Fx, ra: u32, m: i32, add: i32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h + add), 100.0)
}

/// The local 0x1E04A50(D) [F]: a swirl arm at the attacker's node slot
/// +0x10 (its frame 0x40: the caller's s0 / s1 / ra saved).
fn arm(fx: &mut Fx, s1: u64) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0, u64::from(OT)), (0x10, s1), (0x20, 0x01E0_5724)]);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, slot, sp + 0x30, u64::from(OT), Some(u64::from(D)), 0x01E0_4A78);
    let i = first_free(fx, ARMS, 0x5C, 0x64);
    if i >= 0 {
        let r = ARMS + 0x5C * i as u32;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        for (o, v) in [(4u32, 0u8), (5, 0), (8, 2), (9, 0), (0xA, 0x10)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x40C0_0000);
        for (o, v) in [(0x2Au32, 0x10u8), (0x29, 0x10), (0x28, 0xC0), (0x2B, 0x40), (0x27, 0xC0), (0x24, 0xC0), (0x25, 0xC0), (0x26, 0xC0)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x30, r + 0x2C);
        fx.mem.write_u32(r + 0x18, 0x4170_0000);
        fx.mem.write_u32(r + 0x14, 0x4140_0000);
        let x = fx.rand_from(i as u32, 0x01E0_4CF4) >> 4;
        let (_, h) = fx.div(x, 0x168);
        let a = ee::add(360.0, ee::div(ee::from_i32(h.wrapping_mul(200).wrapping_sub(0x8CA0)), 100.0));
        fx.mem.setf(r + 0xC, a);
        let e = ee::add(60.0, fx.mem.f(r + 0xC));
        fx.mem.setf(r + 0x10, e);
        fx.mem.write_u32(r + 0x20, 0x3F80_0000);
    }
    fx.leave(0x40);
}

/// Update 0x5A [F 0x1E04F70]: the fade, three sheets, a halo, a spike,
/// 200 drops and the shake at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 0xC, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    for k in 0..3u32 {
        let r = SHEETS + 0x3C * k;
        set_counter4(fx, r, 0x1C, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 2), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x50, r + 0x2C);
        let d = ee::add(-1.0, ee::from_i32(k as i32));
        let y = ee::add(fx.mem.f(r + 0x30), d);
        fx.mem.setf(r + 0x30, y);
        fx.mem.write_u32(r + 0x10, 0x4040_0000);
        fx.mem.write_u32(r + 0x14, 0x3FCC_CCCD);
        fx.mem.write_u32(r + 0x18, if k == 1 { 0x4080_0000 } else { 0x4040_0000 });
        fx.mem.set32(r + 0x24, 0x4000);
    }
    set_counter4(fx, HALO, 0x20, 0, 0xFF);
    fx.mem.write_u8(HALO + 5, 0);
    fx.mem.write_u8(HALO + 4, 0x80);
    fx.mem.write_u8(HALO + 7, 9);
    fx.mem.write_u32(HALO + 0x10, 0x4248_0000);
    fx.mem.write_u32(HALO + 0x14, 0x4270_0000);
    for (o, v) in [(0xFu32, 0xD0u8), (0xC, 0xD0), (0xD, 0xD0), (0xE, 0xD0), (0xA, 0x40), (9, 0x10), (8, 0x80)] {
        fx.mem.write_u8(HALO + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(HALO + o, 0);
    }
    copy_v4(fx, sp + 0x50, HALO + 0x1C);
    fx.mem.write_u32(HALO + 0x18, 0x420C_0000);
    let r = SPIKE;
    set_counter4(fx, r, 0x1E, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4019_999A);
    fx.mem.write_u32(r + 0x1C, 0x4270_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_52A8);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    fx.mem.write_u8(r + 0x23, 0x20);
    fx.mem.write_u8(r + 0x27, 0x80);
    copy_v4(fx, sp + 0x50, r + 0x28);
    fx.mem.write_u32(r + 0x58, 0x40);
    fx.mem.write_u32(r + 0x5C, 0x80);
    fx.mem.write_u32(r + 0x60, 0x100);
    for _ in 0..0xC8 {
        let i = first_free(fx, DROPS, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let r = DROPS + 0x44 * i as u32;
        let x = fx.rand_from(OT, 0x01E0_535C);
        set_counter4(fx, r, 0x20, (x >> 4) & 0xF, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 0), (0xB, 2), (0xA, 0), (9, 3), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x60)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_5454) >> 4;
        let (_, h) = fx.div(x, 0x14);
        let w = ee::add(100.0, ee::div(ee::from_i32(h.wrapping_mul(200).wrapping_sub(2000)), 100.0));
        fx.mem.setf(r + 0x10, w);
        fx.mem.write_u32(r + 0x14, 0x3F4C_CCCD);
        fx.mem.write_u32(r + 0x18, 0xBD23_D70A);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let v = spread(fx, 0x01E0_5560, 0xB4, -0x5A);
        fx.mem.setf(r + 0x2C, v);
        let v = spread(fx, 0x01E0_55B4, 0xB4, -0x5A);
        fx.mem.setf(r + 0x30, ee::add(f32::from_bits(0x3ECC_CCCD), v));
        let v = spread(fx, 0x01E0_561C, 0xB4, -0x5A);
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
    fx.mem.write_u32(s + 0x50, 0x4080_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_56E4);
}

/// Updates 0x14..0x30 [F 0x1E05744]: two drops from the attacker's node
/// slot +0x14 (sp + 0x60).
fn drops(fx: &mut Fx, sp: u32) {
    for s1 in 0..2u64 {
        let i = first_free(fx, DROPS, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let r = DROPS + 0x44 * i as u32;
        set_counter4(fx, r, 0x16, 0, 0xFF);
        let x = fx.rand_from(OT, 0x01E0_57A0);
        fx.mem.set8(r + 4, ((x >> 4) & 3) << 4);
        fx.mem.write_u8(r + 8, 1);
        fx.mem.write_u8(r + 0xA, 5);
        fx.mem.write_u8(r + 0xB, 1);
        let x = fx.rand_from(OT, 0x01E0_5810);
        fx.mem.set8(r + 9, (x >> 4) & 3);
        for o in [0xCu32, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let x = fx.rand_from(OT, 0x01E0_5874);
        fx.mem.set8(r + 0xF, ((x >> 4) & 0x1F) + 0x60);
        fx.mem.write_u32(r + 0x10, 0x4248_0000);
        fx.mem.write_u32(r + 0x14, 0x4120_0000);
        fx.mem.write_u32(r + 0x1C, 0);
        fx.mem.write_u32(r + 0x18, 0);
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14));
        node_position_saving(fx, h, slot, sp + 0x60, u64::from(OT), Some(s1), 0x01E0_5900);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        for (o, ra) in [(0x2Cu32, 0x01E0_5950u32), (0x30, 0x01E0_59A4), (0x34, 0x01E0_59F8)] {
            let v = spread(fx, ra, 0x3C, -0x1E);
            fx.mem.setf(r + o, v);
        }
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, then the last loop's count.
    let mut s1 = param as i64 as u64;
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_4F4C);
    match fx.mem.u16(D) {
        0x96 => return 8,
        0x5A => {
            strike(fx, sp);
            s1 = 0xC8;
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x46..0x6E).contains(&n) {
        arm(fx, s1);
    }
    let n = fx.mem.u16(D);
    if (0x14..0x31).contains(&n) {
        drops(fx, sp);
        s1 = 2;
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_5A6C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5A7C, [Some(u64::from(OT)), Some(s1)]);
    blib::halo::halo(fx, 0x01E0_0440, HALO, OT, 1);
    blib::swirl::arms_c(fx, 0x01E0_1190, ARMS, OT, 0x64);
    blib::cross::bars_c(fx, 0x01E0_1B90, BARS, OT, 1);
    blib::spike::spikes_b(fx, 0x01E0_2660, SPIKE, OT, 1);
    blib::drop::drops(fx, DROPS, OT, 0xC8);
    blib::fade::fade_c(fx, FADE, OT);
    blib::sheet::sheets_c(fx, 0x01E0_4480, SHEETS, OT, 0x10);
    let n = fx.mem.u16(D);
    if (0x5A..0x77).contains(&n) {
        6
    } else if n == 0x77 {
        2
    } else if n == 0x50 {
        1
    } else {
        0
    }
}
