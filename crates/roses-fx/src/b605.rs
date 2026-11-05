//! BMODULE b605 (card 602), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05AB0, data area a0 = 0x1ED0000) [F] (helper A19,
//! 31 Oct 2026): b468's family with s1 = D, s0 = the parameter (then the
//! last free slot), s2 = the OT; the comets 29cd5954f1de (`comet::comets`),
//! the recompiled sheets f694ab0da4da (`sheet::sheets_c`) and bands
//! 34a784d12fb6 (`band::bands_c`). On update 2 two comets (+0x6964, 0x44
//! each: 0xFF counts, a history of 8 in the shared pool +0x69EC, head 60 /
//! tail 120, cells 5 / 6) and two halos (+0xC24, 0x4C each, 180) on the
//! attacker's nodes 0x30 / 0x6A, and a long shake (+0x40); on 0xB3 / 0xC4
//! at node slot +0xC / +0x10 (sp + 0x80) the fade (+0x695C), a band (+0xA4,
//! 3 / 2 growing 1.3 / 1, turned 0x4000), three sheets (+0x6C6C, record k
//! (not the free slot) at y - 1.5 + 1.5 k, 2 / 1 / 3 or 4, turned 0x4000
//! or -+0x4800 by the facing), a halo of 1200, a spike (3 / 60), 120 drops
//! (+0x26F4, 0x44 each: 0x24 counts after rand & 7, 200 + (rand % 30 x
//! 200 - 3000) / 100 wide) and a shake there; on odd 3..253 a spike on
//! each of the two nodes (flags 0x85, 1.6 / 20); below 257 the shake's
//! node is whichever of nodes +0xC / +0x10 is nearer node +0x14 (0x20C850);
//! the stage light by 0x1031C0 (+0xA0: down on 2, up on 0x126). Returns 1
//! on 0xA9 / 0xBA, 6 on 0xB0 and 0xC4..0xD8, 2 on 0xD9, 8 on 300 (and at
//! once for -2 / -3). The bars +0xD54 are drawn but never spawned.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `halo`
//! 0x1E00440, `bars_c` 0x1E01190, `spikes_b` 0x1E01C60, `drops` 0x1E02990,
//! `fade_c` 0x1E03510, `bands_c` 0x1E03A80, `comets` 0x1E04380,
//! `sheets_c` 0x1E054E0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BANDS: u32 = D + 0xA4;
const HALOS: u32 = D + 0xC24;
const BARS: u32 = D + 0xD54;
const SPIKES: u32 = D + 0xE94;
const DROPS: u32 = D + 0x26F4;
const FADE: u32 = D + 0x695C;
const COMETS: u32 = D + 0x6964;
const SHEETS: u32 = D + 0x6C6C;
const BYTES: u32 = D + 0x6DD4;

/// The entry 0x1E05AB0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x6EE0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..4 {
        fx.texture(5, k);
    }
    for (o, node, clear) in [(0xCu32, 0x30, 1), (0x10, 0x6A, 0), (0x14, 1, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + o, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x126);
    // The entry's s0 = the parameter, s1 = D, s2 = the OT.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5BEC, [Some(param as u32), Some(D), Some(OT)]);
    0
}

/// Update 2 [F 0x1E05C50]: two comets and two halos on the attacker's
/// nodes, the shake.
fn start(fx: &mut Fx) {
    for k in 0..2u32 {
        let r = COMETS + 0x44 * k;
        set_counter4(fx, r, 0xFF, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u32(r + 8, D + 0x69EC);
        fx.mem.write_u32(r + 0x28, 0x28);
        for (o, v) in [(0x1Eu32, 8u8), (0x1F, 0), (0x21, 2), (0x22, 5), (0x23, 6), (0x34, 0x80), (0x35, 0x80), (0x36, 0x80), (0x37, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x2C, 0x4270_0000);
        fx.mem.write_u32(r + 0x30, 0x42F0_0000);
        let mut j = 0;
        while j < fx.mem.u8(r + 0x1E) + 1 {
            fx.mem.set16(r + 0xC + 2 * j as u32, -1);
            j += 1;
        }
    }
    for k in 0..2u32 {
        let r = HALOS + 0x4C * k;
        set_counter4(fx, r, 0xFF, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 4, 0x81);
        fx.mem.write_u8(r + 7, 9);
        fx.mem.write_u32(r + 0x10, 0x4334_0000);
        fx.mem.write_u32(r + 0x14, 0);
        for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x40), (9, 0x10), (8, 0x80), (0xF, 0xD0)] {
            fx.mem.write_u8(r + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(r + o, 0);
        }
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0xFF, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(s + 6, b);
    fx.mem.write_u8(s + 4, 4);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
}

/// ((rand >> 4) % m + add) / 100 [F].
fn spread(fx: &mut Fx, s0: u32, ra: u32, m: i32, add: i32) -> f32 {
    let x = fx.rand_from(s0, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h + add), 100.0)
}

/// Updates 0xB3 / 0xC4 [F 0x1E06110]: the fade, a band, sheets, a halo, a
/// spike, drops and the shake at node slot +0xC + 4 s4 (sp + 0x80).
/// Returns the entry's s0 (the last free slot).
fn strike(fx: &mut Fx, sp: u32, param: i32, s4: u32) -> i64 {
    let h = fx.mem.i32(D + 4);
    let slot = fx.mem.i32(D + 0xC + 4 * s4);
    node_position_saving(fx, h, slot, sp + 0x80, param as i64 as u64, Some(u64::from(D)), 0x01E0_614C);
    set_counter4(fx, FADE, 8, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    let i = first_free(fx, BANDS, 0x5C, 0x20);
    if i >= 0 {
        let r = BANDS + 0x5C * i as u32;
        set_counter4(fx, r, 0x20, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (7, 0x24), (0xC, 2), (8, 0), (9, 2), (0xA, 0), (0xB, 5)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x4040_0000);
        fx.mem.write_u32(r + 0x14, 0x4000_0000);
        for o in [0x28u32, 0x29, 0x2A, 0x24, 0x25, 0x26, 0x2B, 0x27] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.set32(r + 0x3C, 0x4000);
        fx.mem.write_u32(r + 0x18, 0x3FA6_6666);
        fx.mem.write_u32(r + 0x1C, 0x3F80_0000);
        copy_v4(fx, sp + 0x80, r + 0x2C);
    }
    for s3 in 0..3u32 {
        let i = first_free(fx, SHEETS, 0x3C, 6);
        if i < 0 {
            continue;
        }
        // The record written is the loop's, not the free one [F 0x1E064A4].
        let r = SHEETS + 0x3C * s3;
        set_counter4(fx, r, 0x1C, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 3), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x80, r + 0x2C);
        let d = ee::add(-1.5, ee::mul(1.5, ee::from_i32(s3 as i32)));
        let y = ee::add(fx.mem.f(r + 0x30), d);
        fx.mem.setf(r + 0x30, y);
        fx.mem.write_u32(r + 0x10, 0x4000_0000);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        fx.mem.write_u32(r + 0x18, if s3 == 1 { 0x4080_0000 } else { 0x4040_0000 });
        if s4 != 0 {
            fx.mem.set32(r + 0x24, 0x4800);
            if fx.mem.i32(D + 0x30) != 0x8000 {
                let v = fx.mem.i32(r + 0x24).wrapping_neg();
                fx.mem.set32(r + 0x24, v);
            }
        } else {
            fx.mem.set32(r + 0x24, 0x4000);
        }
    }
    let i = first_free(fx, HALOS, 0x4C, 4);
    if i >= 0 {
        let r = HALOS + 0x4C * i as u32;
        set_counter4(fx, r, 0x20, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x81);
        fx.mem.write_u8(r + 7, 9);
        fx.mem.write_u32(r + 0x10, 0x4496_0000);
        fx.mem.write_u32(r + 0x14, 0);
        for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x60), (9, 0x10), (8, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(r + o, 0);
        }
        copy_v4(fx, sp + 0x80, r + 0x1C);
    }
    let i = first_free(fx, SPIKES, 0x68, 0x3C);
    if i >= 0 {
        let r = SPIKES + 0x68 * i as u32;
        fx.mem.write_u8(r + 4, 5);
        set_counter4(fx, r, 0x1C, 0, 0xFF);
        fx.mem.write_u32(r + 0x14, 0x4040_0000);
        fx.mem.write_u32(r + 0x1C, 0x4270_0000);
        for (o, v) in [(0x27u32, 0x60u8), (0x23, 0x10), (5, 0), (0xA, 0), (0xB, 2), (0xC, 4), (8, 0x78)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(i as u32, 0x01E0_6AF8);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0x80, r + 0x28);
        fx.mem.write_u32(r + 0x58, 0x40);
        fx.mem.write_u32(r + 0x5C, 0x80);
        fx.mem.write_u32(r + 0x60, 0x100);
    }
    // The entry's s0 after the drops: the last free slot.
    let mut s0 = 0;
    for _ in 0..0x78 {
        let i = first_free(fx, DROPS, 0x44, 0xFA);
        s0 = i64::from(i);
        if i < 0 {
            continue;
        }
        let r = DROPS + 0x44 * i as u32;
        let s = i as u32;
        let x = fx.rand_from(s, 0x01E0_6CD4);
        set_counter4(fx, r, 0x24, (x >> 4) & 7, 0xFF);
        let x = fx.rand_from(s, 0x01E0_6D08);
        fx.mem.set8(r + 4, ((x >> 4) & 3) << 4);
        fx.mem.write_u8(r + 8, 2);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xA, 0);
        let x = fx.rand_from(s, 0x01E0_6D74);
        fx.mem.set8(r + 9, ((x >> 4) + 4) & 7);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let x = fx.rand_from(s, 0x01E0_6DF0) >> 4;
        let (_, h) = fx.div(x, 0x1E);
        let w = ee::add(200.0, ee::div(ee::from_i32(h.wrapping_mul(200).wrapping_sub(3000)), 100.0));
        fx.mem.setf(r + 0x10, w);
        fx.mem.write_u32(r + 0x14, 0x40A0_0000);
        fx.mem.write_u32(r + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let v = spread(fx, s, 0x01E0_6EF8, 0x96, -0x4B);
        fx.mem.setf(r + 0x2C, v);
        let v = spread(fx, s, 0x01E0_6F4C, 0x96, -0x4B);
        fx.mem.setf(r + 0x30, ee::add(1.0, v));
        let v = spread(fx, s, 0x01E0_6FB0, 0x96, -0x4B);
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
    copy_v4(fx, sp + 0x80, s + 0x20);
    s0
}

/// Odd updates 3..253 [F 0x1E070D4]: a spike on each of the attacker's
/// nodes +0xC / +0x10. Returns the entry's s0.
fn spikes(fx: &mut Fx, mut s0: i64) -> i64 {
    for k in 0..2u32 {
        let i = first_free(fx, SPIKES, 0x68, 0x3C);
        s0 = i64::from(i);
        if i < 0 {
            continue;
        }
        let r = SPIKES + 0x68 * i as u32;
        set_counter4(fx, r, 0x10, 0, 0xFF);
        fx.mem.write_u8(r + 4, 0x85);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 3);
        let x = fx.rand_from(i as u32, 0x01E0_71FC);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x23, 0);
        fx.mem.write_u8(r + 0x27, 0x30);
        fx.mem.write_u8(r + 8, 0x10);
        fx.mem.write_u32(r + 0x14, 0x3FCC_CCCD);
        fx.mem.write_u32(r + 0x1C, 0x41A0_0000);
        for (o, ra) in [(0x58u32, 0x01E0_73B0u32), (0x5C, 0x01E0_73DC), (0x60, 0x01E0_7408)] {
            let x = fx.rand_from(i as u32, ra);
            fx.mem.set32(r + o, (x >> 4) & 0x7F);
        }
    }
    s0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let mut s0 = i64::from(param);
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x70, s0 as u64, 0x01E0_5C14);
    match fx.mem.u16(D) {
        0x12C => return 8,
        0xC4 | 0xB3 => {
            let s4 = u32::from(fx.mem.u16(D) != 0xB3);
            s0 = strike(fx, sp, param, s4);
        }
        2 => start(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0xFF).contains(&n) && n & 1 != 0 {
        s0 = spikes(fx, s0);
    }
    if fx.mem.u16(D) < 0x101 {
        for k in 0..3u32 {
            let h = fx.mem.i32(D + 4);
            let slot = fx.mem.i32(D + 0xC + 4 * k);
            node_position_saving(fx, h, slot, sp + 0x90 + 0x10 * k, s0 as u64, Some(u64::from(D)), 0x01E0_7488);
        }
        for c in 0..3u32 {
            let v = ee::sub(fx.mem.f(sp + 0x90 + 4 * c), fx.mem.f(sp + 0xB0 + 4 * c));
            fx.mem.setf(sp + 0xC0 + 4 * c, v);
        }
        let f20 = blib::beam::length3(fx, sp + 0xC0);
        for c in 0..3u32 {
            let v = ee::sub(fx.mem.f(sp + 0xA0 + 4 * c), fx.mem.f(sp + 0xB0 + 4 * c));
            fx.mem.setf(sp + 0xC0 + 4 * c, v);
        }
        let f0 = blib::beam::length3(fx, sp + 0xC0);
        let node = if ee_lt(f20, f0) { fx.mem.read_u8(D + 0xC) } else { fx.mem.read_u8(D + 0x10) };
        fx.mem.write_u8(D + 0x46, node);
    }
    shake_saving(fx, D + 0x40, s0 as u64, 0x01E0_754C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_755C, [Some(s0 as u64), Some(u64::from(D))]);
    blib::fade::fade_c(fx, FADE, OT);
    blib::halo::halo(fx, 0x01E0_0440, HALOS, OT, 4);
    blib::cross::bars_c(fx, 0x01E0_1190, BARS, OT, 4);
    blib::spike::spikes_b(fx, 0x01E0_1C60, SPIKES, OT, 0x3C);
    blib::drop::drops(fx, DROPS, OT, 0xFA);
    blib::band::bands_c(fx, 0x01E0_3A80, BANDS, OT, 0x20);
    blib::comet::comets(fx, 0x01E0_4380, COMETS, OT, 2);
    blib::sheet::sheets_c(fx, 0x01E0_54E0, SHEETS, OT, 6);
    let n = fx.mem.u16(D);
    if (0xC4..0xD9).contains(&n) {
        6
    } else if n == 0xD9 {
        2
    } else if n == 0xB0 {
        6
    } else if n == 0xBA || n == 0xA9 {
        1
    } else {
        0
    }
}
