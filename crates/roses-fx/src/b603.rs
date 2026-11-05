//! BMODULE b603 (card 261), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04380, data area a0 = 0x1ED0000) [F] (helper A18,
//! 31 Oct 2026): b468's family (s3 = D, s0 = the OT) with the recompiled
//! bands 34a784d12fb6 (`band::bands_c`). On update 22 at the attacker's
//! node slot +0x14 (never requested: 0) four bars (+0xC70, 0x50 each: the
//! first 1000 long, the others 500 at 45 k degrees), a halo of 180 (+0xC24)
//! and a shake (+0x40, mode 4); on 34..81 (odd) a spike an update (+0xDB0,
//! 0x68 each, 60 slots: 16 counts, flags 0x85, 1.1 / 20, random turns);
//! on 90 the fade (+0x5B30), four bands at the target (+0xA4, 0x5C each,
//! 32 slots: 0x20 counts after 2 k, 36 points, radius 1 growing 0.6 /
//! 0.5, turned 0x4000), a halo of 900, a spike (1.8 / 40), 200 drops
//! (+0x2610, 0x44 each: 0x24 counts after rand & 7, 50 + (rand % 20 x 200
//! - 2000) / 100 wide, spread (rand % 150 - 75) / 100, y + 1) and a shake
//! on the target's position; the stage light by 0x1031C0 (+0xA0: down on
//! 2, up on 0x86). Returns 1 on update 80, 6 on 90..110, 2 on 111, 8 on
//! 140 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` (ac7dccf5ae36) 0x1E00080,
//! `halo` 0x1E00440, `bars_c` 0x1E01190, `spikes_b` 0x1E01C60, `drops`
//! 0x1E02990, `fade_c` 0x1E03510, `bands_c` 0x1E03A80 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BANDS: u32 = D + 0xA4;
const HALO: u32 = D + 0xC24;
const BARS: u32 = D + 0xC70;
const SPIKES: u32 = D + 0xDB0;
const DROPS: u32 = D + 0x2610;
const FADE: u32 = D + 0x5B30;
const BYTES: u32 = D + 0x5B38;

/// The entry 0x1E04380 (a0 = D, a1 = param; s3 = D, s1 = param, s0 = the
/// OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x5C40);
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
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x21, 1);
    fx.mem.set32(D + 0xC, r);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x86);
    // The entry's s0 = the OT, s1 = 2 (the texture loop's), s2 = the
    // runner's 0 [H, as b201].
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4484, [Some(OT), Some(2), Some(0)]);
    0
}

/// ((rand >> 4) % m + add) / 100 [F].
fn spread(fx: &mut Fx, ra: u32, m: i32, add: i32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h + add), 100.0)
}

/// A halo's fields (counts set by the caller).
fn halo_at(fx: &mut Fx, size: u32, alpha: u8, pos: u32) {
    let r = HALO;
    for (o, v) in [(4u32, 0x81u8), (7, 9)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, size);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xFu32, alpha), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x40), (9, 0x10), (8, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, pos, r + 0x1C);
}

/// Update 0x16 [F 0x1E044DC]: four bars, a halo and the shake at the
/// attacker's node slot +0x14 (sp + 0x60).
fn start(fx: &mut Fx, sp: u32, param: i32) {
    let h = fx.mem.i32(D + 4);
    let slot = fx.mem.i32(D + 0x14);
    node_position_saving(fx, h, slot, sp + 0x60, u64::from(OT), Some(param as i64 as u64), 0x01E0_44F0);
    for k in 0..4i32 {
        let r = BARS + 0x50 * k as u32;
        set_counter4(fx, r, 0x16, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 4, 0x80);
        if k == 0 {
            fx.mem.write_u32(r + 0x10, 0x447A_0000);
            fx.mem.write_u32(r + 0x18, 0);
            fx.mem.write_u32(r + 0x14, 0x41A0_0000);
        } else {
            fx.mem.write_u32(r + 0x10, 0x43FA_0000);
            fx.mem.setf(r + 0x18, ee::mul(45.0, ee::from_i32(k)));
            fx.mem.write_u32(r + 0x14, 0x4170_0000);
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0xC0), (9, 0x50), (8, 0x10)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x40C0_0000);
        copy_v4(fx, sp + 0x60, r + 0x20);
    }
    set_counter4(fx, HALO, 0x44, 0, 0xFF);
    fx.mem.write_u8(HALO + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(HALO + 6, b);
    halo_at(fx, 0x4334_0000, 0xD0, sp + 0x60);
    let s = D + 0x40;
    set_counter4(fx, s, 0x44, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 4);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x60, s + 0x20);
}

/// A spike's common fields (+9 a rand from `ra`).
fn spike_common(fx: &mut Fx, r: u32, ra: u32) {
    fx.mem.write_u8(r + 0xA, 0);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 3);
    let x = fx.rand_from(OT, ra);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
}

/// Update 0x5A [F 0x1E04868]: the fade, bands, halo, spike, drops and the
/// shake at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 0xA, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    for s1 in 0..4i32 {
        let i = first_free(fx, BANDS, 0x5C, 0x20);
        if i < 0 {
            continue;
        }
        let r = BANDS + 0x5C * i as u32;
        set_counter4(fx, r, 0x20, (s1 << 1) & 0xFF, 0xFF);
        for (o, v) in [(5u32, 0u8), (7, 0x24), (0xC, 2), (8, 0), (9, 2), (0xA, 0), (0xB, 5)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x3F80_0000);
        fx.mem.write_u32(r + 0x14, 0);
        for o in [0x28u32, 0x29, 0x2A, 0x24, 0x25, 0x26, 0x2B, 0x27] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.set32(r + 0x3C, 0x4000);
        fx.mem.write_u32(r + 0x18, 0x3F19_999A);
        fx.mem.write_u32(r + 0x1C, 0x3F00_0000);
        copy_v4(fx, sp + 0x50, r + 0x2C);
    }
    set_counter4(fx, HALO, 0x20, 0, 0xFF);
    fx.mem.write_u8(HALO + 5, 0);
    halo_at(fx, 0x4461_0000, 0xC0, sp + 0x50);
    let i = first_free(fx, SPIKES, 0x68, 0x3C);
    if i >= 0 {
        let r = SPIKES + 0x68 * i as u32;
        fx.mem.write_u8(r + 4, 5);
        set_counter4(fx, r, 0x1C, 0, 0xFF);
        fx.mem.write_u32(r + 0x14, 0x3FE6_6666);
        fx.mem.write_u32(r + 0x1C, 0x4220_0000);
        fx.mem.write_u8(r + 0x27, 0x40);
        fx.mem.write_u8(r + 0x23, 0);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0xA0);
        spike_common(fx, r, 0x01E0_4DB8);
        copy_v4(fx, sp + 0x50, r + 0x28);
        fx.mem.write_u32(r + 0x58, 0x40);
        fx.mem.write_u32(r + 0x5C, 0x80);
        fx.mem.write_u32(r + 0x60, 0x100);
    }
    for _ in 0..0xC8 {
        let i = first_free(fx, DROPS, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let r = DROPS + 0x44 * i as u32;
        let x = fx.rand_from(OT, 0x01E0_4F94);
        set_counter4(fx, r, 0x24, (x >> 4) & 7, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 0), (0xB, 2), (0xA, 0), (9, 5), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_5088) >> 4;
        let (_, h) = fx.div(x, 0x14);
        let w = ee::add(50.0, ee::div(ee::from_i32(h.wrapping_mul(200).wrapping_sub(2000)), 100.0));
        fx.mem.setf(r + 0x10, w);
        fx.mem.write_u32(r + 0x14, 0xBF19_999A);
        fx.mem.write_u32(r + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let v = spread(fx, 0x01E0_5194, 0x96, -0x4B);
        fx.mem.setf(r + 0x2C, v);
        let v = spread(fx, 0x01E0_51E8, 0x96, -0x4B);
        fx.mem.setf(r + 0x30, ee::add(1.0, v));
        let v = spread(fx, 0x01E0_524C, 0x96, -0x4B);
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
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_5314);
}

/// Updates 0x22..0x51 (odd) [F 0x1E05358]: a spike at the attacker's node.
fn spike(fx: &mut Fx) {
    let i = first_free(fx, SPIKES, 0x68, 0x3C);
    if i < 0 {
        return;
    }
    let r = SPIKES + 0x68 * i as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0x85);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    spike_common(fx, r, 0x01E0_546C);
    fx.mem.write_u8(r + 0x23, 0);
    fx.mem.write_u8(r + 0x27, 0x30);
    fx.mem.write_u8(r + 8, 0x10);
    fx.mem.write_u32(r + 0x14, 0x3F8C_CCCD);
    fx.mem.write_u32(r + 0x1C, 0x41A0_0000);
    for (o, ra) in [(0x58u32, 0x01E0_5620u32), (0x5C, 0x01E0_564C), (0x60, 0x01E0_5678)] {
        let x = fx.rand_from(OT, ra);
        fx.mem.set32(r + o, (x >> 4) & 0x7F);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, then the last loop's count.
    let mut s1 = param as i64 as u64;
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_44AC);
    match fx.mem.u16(D) {
        0x8C => return 8,
        0x5A => {
            strike(fx, sp);
            s1 = 0xC8;
        }
        0x16 => {
            start(fx, sp, param);
            s1 = 4;
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x22..0x52).contains(&n) && n & 1 != 0 {
        spike(fx);
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_56A8, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_56B8, [Some(u64::from(OT)), Some(s1)]);
    blib::fade::fade_c(fx, FADE, OT);
    blib::halo::halo(fx, 0x01E0_0440, HALO, OT, 1);
    blib::cross::bars_c(fx, 0x01E0_1190, BARS, OT, 4);
    blib::spike::spikes_b(fx, 0x01E0_1C60, SPIKES, OT, 0x3C);
    blib::drop::drops(fx, DROPS, OT, 0xC8);
    blib::band::bands_c(fx, 0x01E0_3A80, BANDS, OT, 0x20);
    let n = fx.mem.u16(D);
    if (0x5A..0x6F).contains(&n) {
        6
    } else if n == 0x6F {
        2
    } else if n == 0x50 {
        1
    } else {
        0
    }
}
