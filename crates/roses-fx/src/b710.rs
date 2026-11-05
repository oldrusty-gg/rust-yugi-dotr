//! BMODULE b710 (card 12), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06870, data area a0 = 0x1ED0000) [F] (helper A19,
//! 1 Nov 2026): b468's family (s3 = D, s1 = the parameter then the loop
//! counts, s0 = the OT, s2 the inner counts / free slots) with the
//! recompiled swirl arms (1c4c37ddd46b), smears (239ad2781e06) and the
//! coloured fade ec32fd6b7a72 (`fade::fade_rgb_c`). On 2 two smears
//! (+0x5AC4, 0x48 each: 0x70 counts, between the attacker's nodes 0x1B /
//! 0x1C) and a long soft shake on node 0x1A; on 2..32 the stage light
//! (0x1F7F10) 2 (n - 2) and the other light (0x1F7EF0) 4 (n - 2), else the
//! ramp 0x1031C0 (+0xA0: down on 2, up on 0xC2); on 20..55 a drop an
//! update from node 0x36; on 65..81 a swirl arm an update by the local
//! 0x1E064D0 (8821dab7ca46) at node slot +0x10; on 82 a white fade
//! (+0x5AB8) and a bead-trail emitter (+0x5D54, `bead_trails_c` into
//! `beads_at 40 44` +0x5DD0) at node slot +0xC; on 82..126 six drops an
//! update at the target; on 110 at the target an orange fade, a bar 3000
//! long (+0x24E0), a halo of 200 (+0xA4), a spike (+0x2530) and a shake.
//! Returns 1 on 72, 6 on 82..138, 2 on 139, 8 on 200 (and at once for -2
//! / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `halo`
//! 0x1E00440, `arms_c` 0x1E01190, `bars_c` 0x1E01B90, `spikes_b`
//! 0x1E02660, `drops` 0x1E03390, `fade_rgb_c` 0x1E03F10, `smears_d`
//! 0x1E04450, `beads_at 40 44` 0x1E05050, `bead_trails_c` 0x1E05F60
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
const BAR: u32 = D + 0x24E0;
const SPIKE: u32 = D + 0x2530;
const DROPS: u32 = D + 0x2598;
const FADE: u32 = D + 0x5AB8;
const SMEARS: u32 = D + 0x5AC4;
const EMITTER: u32 = D + 0x5D54;
const BEADS: u32 = D + 0x5DD0;
const BYTES: u32 = D + 0x8030;
/// The light byte 0x1F7EF0 sets (as b101).
const LIGHT2: u32 = 0x002D_F5B0;

/// The entry 0x1E06870 (a0 = D, a1 = param).
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
    fx.bzero(D, 0x8130);
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
    for (o, node, clear) in [(0xCu32, 1, 1), (0x10, 0x10, 0), (0x14, 0x1B, 0), (0x18, 0x1C, 0), (0x1C, 0x36, 0), (0x20, 0x1A, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + o, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xC2);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's), s2 = the
    // runner's 0 [H, as b201].
    rand_bytes(fx, BYTES, 0x100, 0x01E0_69F0, [Some(OT), Some(3), Some(0)]);
    0
}

/// Update 2 [F 0x1E06A54]: two smears between nodes +0x14 / +0x18 and the
/// shake on node +0x20.
fn start(fx: &mut Fx) {
    for k in 0..2u32 {
        let r = SMEARS + 0x48 * k;
        set_counter4(fx, r, 0x70, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let (a, b) = if k == 0 { (0x14u32, 0x18u32) } else { (0x18, 0x14) };
        let v = fx.mem.read_u8(D + a);
        fx.mem.write_u8(r + 0x21, v);
        let v = fx.mem.read_u8(D + b);
        fx.mem.write_u8(r + 0x22, v);
        fx.mem.write_u32(r + 8, D + 0x5B54);
        for (o, v) in [(0x1Eu32, 0x20u8), (0x1F, 8), (0x20, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x2C, 0x4220_0000);
        fx.mem.write_u32(r + 0x28, 0x3F00_0000);
        for (o, v) in [(0x34u32, 0x80u8), (0x35, 0x80), (0x36, 0x80), (0x37, 0x60)] {
            fx.mem.write_u8(r + o, v);
        }
        let mut j = 0;
        while j < fx.mem.u8(r + 0x1F) + 1 {
            fx.mem.set16(r + 0xC + 2 * j as u32, -1);
            j += 1;
        }
        fx.mem.write_u32(r + 0x30, 0xBF80_0000);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x70, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(D + 0x20);
    fx.mem.write_u8(s + 6, b);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FF3_3333);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4120_0000);
    fx.mem.write_u32(s + 0x50, 0x4000_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_6CE4);
}

/// Update 0x52 [F 0x1E06CEC]: a white fade and the bead-trail emitter at
/// node slot +0xC.
fn flash(fx: &mut Fx, param: i32) {
    set_counter4(fx, FADE, 0x10, 0, 0xFF);
    for o in [8u32, 9, 0xA] {
        fx.mem.write_u8(FADE + o, 0xFF);
    }
    fx.mem.write_u8(FADE + 4, 1);
    let e = EMITTER;
    set_counter4(fx, e, 0x20, 0, 0xFF);
    fx.mem.write_u8(e + 4, 0);
    fx.mem.write_u8(e + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(e + 6, b);
    fx.mem.write_u32(e + 8, BEADS);
    for (o, v) in [(0x10u32, 0x64u8), (0x12, 8), (0x11, 0x12), (0x13, 0), (0x16, 2), (0x14, 4)] {
        fx.mem.write_u8(e + o, v);
    }
    fx.mem.write_u32(e + 0x18, 0x3F00_0000);
    fx.mem.write_u32(e + 0x1C, 0x3F00_0000);
    fx.mem.write_u32(e + 0x20, 0x4234_0000);
    fx.mem.write_u32(e + 0x24, 0x3F00_0000);
    fx.mem.write_u32(e + 0xC, BYTES);
    for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
        fx.mem.write_u8(e + o, 0x80);
    }
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, slot, e + 0x3C, u64::from(OT), Some(param as i64 as u64), 0x01E0_6DD0);
    copy_v4(fx, e + 0x3C, e + 0x4C);
}

/// Update 0x6E [F 0x1E06DF8]: an orange fade, a bar, a halo, a spike and
/// the shake at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 0x10, 0, 0xFF);
    fx.mem.write_u8(FADE + 0xA, 0xFF);
    fx.mem.write_u8(FADE + 9, 0x20);
    fx.mem.write_u8(FADE + 8, 0);
    fx.mem.write_u8(FADE + 4, 0x82);
    let b = BAR;
    fx.mem.write_u8(b + 5, 0);
    copy_v4(fx, sp + 0x50, b + 0x20);
    set_counter4(fx, b, 0x20, 1, 0xFF);
    fx.mem.write_u8(b + 4, 0x81);
    fx.mem.write_u32(b + 0x10, 0x453B_8000);
    fx.mem.write_u32(b + 0x14, 0x42C8_0000);
    for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (9, 0), (0xA, 0), (8, 0x40), (0xF, 0xC0)] {
        fx.mem.write_u8(b + o, v);
    }
    fx.mem.write_u32(b + 0x1C, 0);
    fx.mem.write_u32(b + 0x18, 0);
    set_counter4(fx, HALO, 0x20, 0, 0xFF);
    fx.mem.write_u8(HALO + 5, 0);
    fx.mem.write_u8(HALO + 4, 0x81);
    fx.mem.write_u8(HALO + 7, 9);
    fx.mem.write_u32(HALO + 0x10, 0x4348_0000);
    fx.mem.write_u32(HALO + 0x14, 0);
    for (o, v) in [(0xFu32, 0xD0u8), (0xC, 0xD0), (0xD, 0xD0), (0xE, 0xD0), (0xA, 0x40), (9, 0x10), (8, 0x80)] {
        fx.mem.write_u8(HALO + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(HALO + o, 0);
    }
    copy_v4(fx, sp + 0x50, HALO + 0x1C);
    let r = SPIKE;
    set_counter4(fx, r, 0x1E, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4000_0000);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_6FA4);
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
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_7090);
}

/// ((rand >> 4) % m + add) / 100 [F].
fn spread(fx: &mut Fx, ra: u32, m: i32, add: i32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h + add), 100.0)
}

/// The local 0x1E064D0(D) [F]: a swirl arm at the attacker's node slot
/// +0x10 (its frame 0x40: the caller's s0 / s1 / ra saved).
fn arm(fx: &mut Fx, s1: u64) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0, u64::from(OT)), (0x10, s1), (0x20, 0x01E0_70D0)]);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, slot, sp + 0x30, u64::from(OT), Some(u64::from(D)), 0x01E0_64F8);
    let i = first_free(fx, ARMS, 0x5C, 0x64);
    if i >= 0 {
        let r = ARMS + 0x5C * i as u32;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        for (o, v) in [(4u32, 0u8), (5, 0), (8, 1), (9, 0), (0xA, 0x10)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x40C0_0000);
        for (o, v) in [(0x2Au32, 0x80u8), (0x29, 0x10), (0x28, 0x60), (0x2B, 0x40), (0x27, 0xB0), (0x24, 0xB0), (0x25, 0xB0), (0x26, 0xB0)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x30, r + 0x2C);
        fx.mem.write_u32(r + 0x18, 0x4170_0000);
        fx.mem.write_u32(r + 0x14, 0x4140_0000);
        let x = fx.rand_from(i as u32, 0x01E0_6774) >> 4;
        let (_, h) = fx.div(x, 0x168);
        let a = ee::add(360.0, ee::div(ee::from_i32(h.wrapping_mul(200).wrapping_sub(0x8CA0)), 100.0));
        fx.mem.setf(r + 0xC, a);
        let e = ee::add(60.0, fx.mem.f(r + 0xC));
        fx.mem.setf(r + 0x10, e);
        fx.mem.write_u32(r + 0x20, 0x3F80_0000);
    }
    fx.leave(0x40);
}

/// Updates 0x14..0x37 [F 0x1E070F0]: a drop from node slot +0x1C (sp +
/// 0x60).
fn small_drop(fx: &mut Fx, sp: u32) {
    let i = first_free(fx, DROPS, 0x44, 0xC8);
    if i < 0 {
        return;
    }
    let r = DROPS + 0x44 * i as u32;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    let x = fx.rand_from(OT, 0x01E0_714C);
    fx.mem.set8(r + 4, ((x >> 4) & 3) << 4);
    fx.mem.write_u8(r + 8, 1);
    fx.mem.write_u8(r + 0xA, 5);
    fx.mem.write_u8(r + 0xB, 1);
    let x = fx.rand_from(OT, 0x01E0_71BC);
    fx.mem.set8(r + 9, (x >> 4) & 3);
    for o in [0xCu32, 0xD, 0xE] {
        fx.mem.write_u8(r + o, 0x80);
    }
    let x = fx.rand_from(OT, 0x01E0_7220);
    fx.mem.set8(r + 0xF, ((x >> 4) & 0x1F) + 0x60);
    fx.mem.write_u32(r + 0x10, 0x4248_0000);
    fx.mem.write_u32(r + 0x14, 0x4100_0000);
    fx.mem.write_u32(r + 0x1C, 0);
    fx.mem.write_u32(r + 0x18, 0);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x1C));
    node_position_saving(fx, h, slot, sp + 0x60, u64::from(OT), Some(0), 0x01E0_72AC);
    for c in 0..3 {
        let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
        fx.mem.write_u32(r + 0x20 + 4 * c, w);
    }
    for (o, ra) in [(0x2Cu32, 0x01E0_72FCu32), (0x30, 0x01E0_7350), (0x34, 0x01E0_73A4)] {
        let v = spread(fx, ra, 0x1E, -0xF);
        fx.mem.setf(r + o, v);
    }
}

/// Updates 0x52..0x7E [F 0x1E07420]: six drops at the target (sp + 0x50).
fn drops(fx: &mut Fx, sp: u32) {
    for _ in 0..6 {
        let i = first_free(fx, DROPS, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let r = DROPS + 0x44 * i as u32;
        let x = fx.rand_from(OT, 0x01E0_7454);
        set_counter4(fx, r, 0x20, (x >> 4) & 0xF, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 0), (0xB, 2), (0xA, 0), (9, 3), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x60)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_754C) >> 4;
        let (_, q) = fx.div(x, 0x14);
        let w = ee::add(100.0, ee::div(ee::from_i32(q.wrapping_mul(200).wrapping_sub(2000)), 100.0));
        fx.mem.setf(r + 0x10, w);
        fx.mem.write_u32(r + 0x14, 0x3F4C_CCCD);
        fx.mem.write_u32(r + 0x18, 0xBD75_C28F);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let v = spread(fx, 0x01E0_7658, 0xB4, -0x5A);
        fx.mem.setf(r + 0x2C, v);
        let v = spread(fx, 0x01E0_76AC, 0xB4, -0x5A);
        fx.mem.setf(r + 0x30, ee::add(f32::from_bits(0x3ECC_CCCD), v));
        let v = spread(fx, 0x01E0_7714, 0xB4, -0x5A);
        fx.mem.setf(r + 0x34, v);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, then the last loop's count.
    let mut s1 = param as i64 as u64;
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_6A18);
    match fx.mem.u16(D) {
        0xC8 => return 8,
        0x6E => strike(fx, sp),
        0x52 => flash(fx, param),
        2 => {
            start(fx);
            s1 = 9;
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x41..0x52).contains(&n) {
        arm(fx, s1);
    }
    let n = fx.mem.u16(D);
    if (0x14..0x38).contains(&n) {
        small_drop(fx, sp);
        s1 = 1;
    }
    let n = fx.mem.u16(D);
    if (0x52..0x7F).contains(&n) {
        drops(fx, sp);
        s1 = 6;
    }
    let n = fx.mem.u16(D);
    if (2..0x21).contains(&n) {
        fx.set_stage_light(((n - 2) << 1) & 0xFF);
        fx.mem.write_u8(LIGHT2, (((n - 2) << 2) & 0xFF) as u8);
    } else {
        light_ramp(fx, D + 0xA0, n, 0x01E0_77E0, [Some(u64::from(OT)), Some(s1)]);
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_77EC, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::halo::halo(fx, 0x01E0_0440, HALO, OT, 1);
    blib::swirl::arms_c(fx, 0x01E0_1190, ARMS, OT, 0x64);
    blib::cross::bars_c(fx, 0x01E0_1B90, BAR, OT, 1);
    blib::spike::spikes_b(fx, 0x01E0_2660, SPIKE, OT, 1);
    // What the drops' last calls leave below is the bead-trail path's w
    // (run b710).
    blib::drop::drops_saving(fx, DROPS, OT, 0xC8, 0x01E0_3628, 0x01E0_3EB4);
    blib::fade::fade_rgb_c(fx, FADE, OT);
    blib::smear::smears_d(fx, 0x01E0_4450, SMEARS, OT, 2);
    blib::emit::bead_trails_c(fx, 0x01E0_5F60, EMITTER, 1);
    blib::beam::beads_at(fx, 0x01E0_5050, 0x40, 0x44, BEADS, OT, 0x64);
    let n = fx.mem.u16(D);
    if (0x52..0x8B).contains(&n) {
        6
    } else if n == 0x8B {
        2
    } else if n == 0x48 {
        1
    } else {
        0
    }
}
