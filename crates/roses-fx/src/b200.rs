//! BMODULE b200 (card 241), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E068B0, data area a0 = 0x1ED0000) [F] (helper A19,
//! 1 Nov 2026): b468's family (s3 = D, s1 = the parameter then the loop
//! counts / the last free slot, s0 = the OT, s2 the inner counts) with the
//! recompiled node bursts / motes (1e40f1216565 / 90a569cefb1c), sheets
//! (f694ab0da4da) and smears (239ad2781e06, `smear::smears_d`). Node bursts
//! every update (+0xA4: cue lists 0x1E07B00 / 0x1E07B10 on the attacker's
//! nodes 0xA / 0x2F, groups +0xE8 (32), sprites +0x7E8 (50)); on 2 two
//! smears (+0x4A8C, 0x48 each: 0x63 counts, nodes 0x21 -> 0x1D, offsets
//! (-+1, -1, +-0.5 by the facing)); on 97 an arc (+0xFB8, nodes 0x28 ->
//! 0x24, tracks of 8); on 101 at the target the fade (+0x48F0), five
//! sheets (+0x4960, y - 6 + 3 k, turned -+0x2000 by the facing), a halo of
//! 900 (+0x1338), 100 drops (+0x13D0), a spike (+0x48F8) and a shake (its
//! position the target's +0x60). The stage light by 0x1031C0 (+0xA0: down
//! on 2, up on 0xCE). Returns 1 on 91, 6 on 101..121, 2 on 122, 8 on 212
//! (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes_c`
//! 0x1E00440, `ring_spawn_b` 0x1E00F00, `node_bursts_b` 0x1E01250, `arcs`
//! (fb70e0bee134) 0x1E01730, `halo` 0x1E02B70, `drops` 0x1E038C0, `fade_c`
//! 0x1E04440, `sheets_c` 0x1E049B0, `spikes_b` 0x1E04F80, `smears_d`
//! 0x1E05CB0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BURSTS: u32 = D + 0xA4;
const GROUPS: u32 = D + 0xE8;
const ARC: u32 = D + 0xFB8;
const POOL: u32 = D + 0x1018;
const HALOS: u32 = D + 0x1338;
const DROPS: u32 = D + 0x13D0;
const FADE: u32 = D + 0x48F0;
const SPIKE: u32 = D + 0x48F8;
const SHEETS: u32 = D + 0x4960;
const SMEARS: u32 = D + 0x4A8C;
const BYTES: u32 = D + 0x4B1C;

/// The entry 0x1E068B0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x60);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x4C20);
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
    for (o, node, clear) in [(0xCu32, 0xA, 1), (0x10, 0x2F, 0), (0x14, 0x28, 0), (0x18, 0x24, 0), (0x1C, 0x21, 0), (0x20, 0x1D, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + o, r);
    }
    let p = BURSTS;
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(p + 0x28, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(p + 0x29, b);
    for (o, v) in [(8u32, 0x01E0_7B00u32), (0xC, 0x01E0_7B10), (0, GROUPS), (4, D + 0x7E8)] {
        fx.mem.write_u32(p + o, v);
    }
    fx.mem.write_u8(p + 0x30, 0x20);
    fx.mem.write_u8(p + 0x31, 0x32);
    fx.mem.write_u8(p + 0x32, 0);
    fx.mem.write_u16(p + 0x34, 0);
    fx.mem.write_u32(p + 0x38, 0x4348_0000);
    fx.mem.write_u32(p + 0x3C, 0);
    fx.mem.write_u32(p + 0x40, 0x4120_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xCE);
    // The entry's s0 = the OT, s1 = the parameter, s2 = 3 (the texture
    // loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_6A90, [Some(OT), Some(param as u32), Some(3)]);
    0
}

/// Update 2 [F 0x1E06AF4]: two smears from node +0x1C to +0x20.
fn smears(fx: &mut Fx) {
    for k in 0..2u32 {
        let r = SMEARS + 0x48 * k;
        set_counter4(fx, r, 0x63, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0x1C);
        fx.mem.write_u8(r + 0x21, b);
        let b = fx.mem.read_u8(D + 0x20);
        fx.mem.write_u8(r + 0x22, b);
        fx.mem.write_u32(r + 8, POOL);
        for (o, v) in [(0x1Eu32, 0x32u8), (0x1F, 5), (0x20, 1)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x2C, 0x4248_0000);
        fx.mem.write_u32(r + 0x28, 0x3F11_EB85);
        for (o, v) in [(0x34u32, 0x80u8), (0x35, 0x80), (0x36, 0x80), (0x37, 0x60)] {
            fx.mem.write_u8(r + o, v);
        }
        let mut j = 0;
        while j < fx.mem.u8(r + 0x1F) + 1 {
            fx.mem.set16(r + 0xC + 2 * j as u32, -1);
            j += 1;
        }
        fx.mem.write_u32(r + 0x38, if k != 0 { 0x3F80_0000 } else { 0xBF80_0000 });
        fx.mem.write_u32(r + 0x3C, 0x3F80_0000);
        let back = fx.mem.i32(D + 0x30) == 0x8000;
        fx.mem.write_u32(r + 0x40, if back { 0x3F00_0000 } else { 0xBF00_0000 });
        fx.mem.write_u32(r + 0x30, 0xBF80_0000);
    }
}

/// Update 0x61 [F 0x1E06DCC]: the arc from node +0x14 to +0x18.
fn arc(fx: &mut Fx) {
    let r = ARC;
    set_counter4(fx, r, 0x18, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0xD);
    fx.mem.write_u32(r + 8, POOL);
    fx.mem.write_u8(r + 0x30, 0x32);
    fx.mem.write_u8(r + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(r + 0x32, b);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(r + 0x33, b);
    fx.mem.write_u32(r + 0x38, 0x3FB3_3333);
    fx.mem.write_u32(r + 0x3C, 0x40C0_0000);
    fx.mem.write_u32(r + 0x40, 0x4190_0000);
    for (o, v) in [(0x47u32, 0xC0u8), (0x44, 0xC0), (0x45, 0xC0), (0x46, 0xC0), (0x4A, 0x80), (0x49, 0), (0x48, 0x10), (0x4E, 0xC0), (0x4D, 0), (0x4C, 0x10), (0x4F, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x54, 0x4000_0000);
    let mut j = 0;
    while j < fx.mem.u8(r + 0x31) + 1 {
        fx.mem.set16(r + 0xC + 2 * j as u32, -1);
        fx.mem.set16(r + 0x1E + 2 * j as u32, -1);
        j += 1;
    }
}

/// ((rand >> 4) % m + add) / 100 [F].
fn spread(fx: &mut Fx, ra: u32, m: i32, add: i32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h + add), 100.0)
}

/// Update 0x65 [F 0x1E06ECC]: the fade, sheets, a halo, drops, a spike and
/// the shake at the target (sp + 0x50). Returns the entry's s1 (the last
/// free drop).
fn strike(fx: &mut Fx, sp: u32) -> i64 {
    set_counter4(fx, FADE, 8, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    for k in 0..5u32 {
        let r = SHEETS + 0x3C * k;
        set_counter4(fx, r, 0x14, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 2), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x50, r + 0x2C);
        let d = ee::add(-6.0, ee::mul(3.0, ee::from_i32(k as i32)));
        let y = ee::add(fx.mem.f(r + 0x30), d);
        fx.mem.setf(r + 0x30, y);
        fx.mem.write_u32(r + 0x10, 0x4040_0000);
        fx.mem.write_u32(r + 0x14, 0x3FC0_0000);
        fx.mem.write_u32(r + 0x18, if k == 2 { 0x4040_0000 } else { 0x4000_0000 });
        fx.mem.set32(r + 0x24, 0x2000);
        if fx.mem.i32(D + 0x30) != 0x8000 {
            let v = fx.mem.i32(r + 0x24).wrapping_neg();
            fx.mem.set32(r + 0x24, v);
        }
    }
    let h = HALOS;
    set_counter4(fx, h, 0x1C, 0, 0xFF);
    fx.mem.write_u8(h + 7, 9);
    fx.mem.write_u8(h + 5, 0);
    fx.mem.write_u8(h + 4, 0x81);
    fx.mem.write_u32(h + 0x10, 0x4461_0000);
    fx.mem.write_u32(h + 0x18, 0);
    fx.mem.write_u32(h + 0x14, 0);
    for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xA, 0x80), (9, 0x10), (8, 0x10), (0xF, 0xC0)] {
        fx.mem.write_u8(h + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(h + o, 0);
    }
    copy_v4(fx, sp + 0x50, h + 0x1C);
    let mut s1 = 0i64;
    for _ in 0..0x64 {
        let i = first_free(fx, DROPS, 0x44, 0xC8);
        s1 = i64::from(i);
        if i < 0 {
            continue;
        }
        let r = DROPS + 0x44 * i as u32;
        let x = fx.rand_from(OT, 0x01E0_73C8);
        set_counter4(fx, r, 0x20, (x >> 4) & 0xF, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 1), (0xB, 2), (0xA, 0), (9, 3), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x60)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_74C4) >> 4;
        let (_, q) = fx.div(x, 0x14);
        let w = ee::add(80.0, ee::div(ee::from_i32(q.wrapping_mul(200).wrapping_sub(2000)), 100.0));
        fx.mem.setf(r + 0x10, w);
        fx.mem.write_u32(r + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(r + 0x18, 0xBD23_D70A);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let v = spread(fx, 0x01E0_75D0, 0xB4, -0x5A);
        fx.mem.setf(r + 0x2C, v);
        let v = spread(fx, 0x01E0_7624, 0xB4, -0x5A);
        fx.mem.setf(r + 0x30, ee::add(f32::from_bits(0x3ECC_CCCD), v));
        let v = spread(fx, 0x01E0_768C, 0xB4, -0x5A);
        fx.mem.setf(r + 0x34, v);
    }
    let r = SPIKE;
    fx.mem.write_u8(r + 4, 5);
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    let f = fx.mem.read_u8(r + 4);
    fx.mem.write_u8(r + 4, f | 2);
    fx.mem.write_u32(r + 0x14, 0x3FB3_3333);
    fx.mem.write_u32(r + 0x1C, 0x42A0_0000);
    for (o, v) in [(0x23u32, 0x80u8), (0x27, 0x80), (5, 0), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0x50)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_7764);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    copy_v4(fx, sp + 0x50, r + 0x28);
    let s = D + 0x40;
    set_counter4(fx, s, 0x1C, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_7820);
    s1
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, then the last loop's count / slot.
    let mut s1 = param as i64 as u64;
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_6AB8);
    match fx.mem.u16(D) {
        0xD4 => return 8,
        0x65 => s1 = strike(fx, sp) as u64,
        0x61 => {
            arc(fx);
            s1 = u64::from(fx.mem.read_u8(ARC + 0x31)) + 1;
        }
        2 => {
            smears(fx);
            s1 = 2;
        }
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_7840, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_7850, [Some(u64::from(OT)), Some(s1)]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts_b(fx, BURSTS, n);
    blib::mote::motes_c(fx, GROUPS, OT, 0x20);
    blib::arc::arcs(fx, 0x01E0_1730, ARC, OT, 1);
    blib::halo::halo(fx, 0x01E0_2B70, HALOS, OT, 2);
    blib::drop::drops(fx, DROPS, OT, 0xC8);
    blib::spike::spikes_b(fx, 0x01E0_4F80, SPIKE, OT, 1);
    blib::fade::fade_c(fx, FADE, OT);
    blib::sheet::sheets_c(fx, 0x01E0_49B0, SHEETS, OT, 5);
    blib::smear::smears_d(fx, 0x01E0_5CB0, SMEARS, OT, 2);
    let n = fx.mem.u16(D);
    if (0x65..0x7A).contains(&n) {
        6
    } else if n == 0x7A {
        2
    } else if n == 0x5B {
        1
    } else {
        0
    }
}
