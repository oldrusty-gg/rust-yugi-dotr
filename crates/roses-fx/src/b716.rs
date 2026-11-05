//! BMODULE b716 (card 422), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06370, data area a0 = 0x1ED0000, 0x71F0 bytes)
//! [F]: s0 = D, s1 = the OT (0x206E40), s2 the
//! parameter then counters / pointers; compiled without scheduling (nop
//! delay slots). b405's library (smears, crossbars, flash_b, spikes,
//! drops, bands_b, fade, shake) plus arcs_b and streaks_b. Update 8 two
//! smears (+0x6B60, `smear::smears`, nodes +0x10 / +0x14 -> +0xC, pool
//! +0x6BF0); 9 a crossbar on each live smear (+0x6940, `cross::bars_b`);
//! 30 36 sprites (+0x2484, `drop::drops`) thrown in a fan from node +0x28;
//! 32 a shake at node +0x28; 56..97 eight sprites a update from node +0xC;
//! 83 four arcs (+0x69E0, `arc::arcs_b`, nodes +0x18.. -> +0x28); 99 at
//! the target the fade (+0x66EC), 32 bands (+0xA4, `band::bands_b`), a
//! flash (+0x672C), a spike (+0xC24), 200 sprites and a shake; 109 the
//! attacker's streaks (+0x66F4, `radiate::streaks_b`); 32..107 the local
//! 0x1E06100 keeps four flashes on the line from node +0x28 to the
//! midpoint of nodes +0x1C / +0x20. Update 31 re-requests the nodes (+0x28
//! now node 0x16). Returns 6 on updates 99..139, 2 on 140, 1 on 89, 8 on
//! 180 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash_b`
//! 0x1E003A0, `bars_b` 0x1E00E10, `arcs_b` 0x1E01710, `smears` 0x1E02830,
//! `streaks_b` 0x1E032B0, `spikes` 0x1E040B0, `drops` 0x1E04B90, `fade`
//! 0x1E054F0, `bands_b` 0x1E059C0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BANDS: u32 = D + 0xA4;
const SPIKES: u32 = D + 0xC24;
const DROPS: u32 = D + 0x2484;
const FADE: u32 = D + 0x66EC;
const STREAKS: u32 = D + 0x66F4;
const FLASH: u32 = D + 0x672C;
const BARS: u32 = D + 0x6940;
const ARCS: u32 = D + 0x69E0;
const SMEARS: u32 = D + 0x6B60;
const POOL: u32 = D + 0x6BF0;
const BYTES: u32 = D + 0x70F0;

/// The entry 0x1E06370 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0xA0);
    ret
}

/// The node requests +0xC..+0x28 (+0x28: node `last`).
fn request_nodes(fx: &mut Fx, last: i32) {
    for (at, node, clear) in [(0xCu32, 0x33, 1), (0x10, 0x3B, 0), (0x14, 0x3E, 0), (0x18, 0x12, 0), (0x1C, 0x13, 0), (0x20, 0x14, 0), (0x24, 0x15, 0), (0x28, last, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x71F0).
    fx.bzero(D, 0x71F0);
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
    request_nodes(fx, 0x55);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xAE);
    // The entry's s0 = D, s1 = 4 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_64E0, [Some(D), Some(4), Some(param as u32)]);
    0
}

fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 4
}

/// (200 ((rand >> 4) % m) - half) / 100.
fn spread(fx: &mut Fx, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(200 * r - half), 100.0)
}

/// ((rand >> 4) % m - half) / 100.
fn jit(fx: &mut Fx, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

/// +-0.4 by the attacker's side (D + 0x30 == 0x8000: -0.4).
fn side(fx: &mut Fx) -> u32 {
    if fx.mem.read_u32(D + 0x30) == 0x8000 {
        0xBECC_CCCD
    } else {
        0x3ECC_CCCD
    }
}

/// Update 8 [F 0x1E06570]: two smears from nodes +0x10 / +0x14 to +0xC.
fn smears(fx: &mut Fx) {
    for s2 in 0..2u32 {
        let r = SMEARS + 0x48 * s2;
        set_counter4(fx, r, 0xC8, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let v = fx.mem.read_u8(D + if s2 == 0 { 0x10 } else { 0x14 });
        fx.mem.write_u8(r + 0x21, v);
        let v = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(r + 0x22, v);
        fx.mem.write_u32(r + 8, POOL);
        fx.mem.write_u8(r + 0x1E, 0x50);
        fx.mem.write_u8(r + 0x1F, 4);
        fx.mem.write_u8(r + 0x20, 0);
        fx.mem.write_u32(r + 0x2C, 0x41F0_0000);
        fx.mem.write_u32(r + 0x28, 0x3F11_EB85);
        for o in [0x34u32, 0x35, 0x36] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x37, 0x60);
        let len = fx.mem.u8(r + 0x1F) as u32;
        for k in 0..=len {
            fx.mem.set16(r + 0xC + 2 * k, -1);
        }
        fx.mem.write_u32(r + 0x3C, 0x3F80_0000);
        let v = side(fx);
        fx.mem.write_u32(r + 0x40, v);
        fx.mem.write_u32(r + 0x30, 0xBF80_0000);
    }
}

/// Update 9 [F 0x1E0668C]: a crossbar on each smear whose w is not -1.
fn bars(fx: &mut Fx) {
    for s5 in 0..2u32 {
        if fx.mem.read_u32(SMEARS + 0x48 * s5 + 0x30) == 0xBF80_0000 {
            continue;
        }
        let b = BARS + 0x50 * s5;
        set_counter4(fx, b, 0x10, 0, 0xFF);
        fx.mem.write_u8(b + 5, 1);
        let v = fx.mem.read_u8(D + 0x10 + 4 * s5);
        fx.mem.write_u8(b + 6, v);
        fx.mem.write_u8(b + 4, 0x80);
        fx.mem.write_u32(b + 0x10, 0x4396_0000);
        fx.mem.write_u32(b + 0x14, 0x4080_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80), (0xA, 0x80), (9, 0), (8, 0x10)] {
            fx.mem.write_u8(b + o, v);
        }
        fx.mem.write_u32(b + 0x18, 0x42B4_0000);
        fx.mem.write_u32(b + 0x1C, 0);
        fx.mem.write_u32(b + 0x44, 0x3F80_0000);
        let v = side(fx);
        fx.mem.write_u32(b + 0x48, v);
    }
}

/// Update 30 [F 0x1E06778]: 36 sprites thrown in a fan from node +0x28.
fn fan(fx: &mut Fx, sp: u32) {
    let (h, n) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x28));
    node_position_saving(fx, h, n, sp + 0x90, u64::from(D), Some(u64::from(OT)), 0x01E0_6788);
    for s3 in 0..0x24i32 {
        let i = first_free(fx, DROPS, 0x44, 0xFA);
        if i < 0 {
            continue;
        }
        let delay = rand4(fx, 0x01E0_67B0) & 0xF;
        let q = DROPS + 0x44 * i as u32;
        set_counter4(fx, q, 0x28, delay, 0xFF);
        let v = (rand4(fx, 0x01E0_67E0) & 3) << 4;
        fx.mem.set8(q + 4, v);
        fx.mem.write_u8(q + 8, 3);
        fx.mem.write_u8(q + 0xB, 1);
        fx.mem.write_u8(q + 0xA, 5);
        let v = rand4(fx, 0x01E0_680C) & 3;
        fx.mem.set8(q + 9, v);
        for o in [0xCu32, 0xD, 0xE] {
            fx.mem.write_u8(q + o, 0x80);
        }
        fx.mem.write_u8(q + 0xF, 0x50);
        let f = ee::add(10.0, spread(fx, 0x01E0_6834, 3, 0x12C));
        fx.mem.setf(q + 0x10, f);
        let r = spread(fx, 0x01E0_688C, 3, 0x12C);
        let f20 = ee::mul(10.0, ee::from_i32(s3));
        fx.mem.setf(q + 0x14, ee::add(15.0, r));
        fx.mem.write_u32(q + 0x18, 0xBCA3_D70A);
        let y = fx.mem.read_u32(sp + 0x94);
        fx.mem.write_u32(q + 0x1C, y);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x90 + 4 * c);
            fx.mem.write_u32(q + 0x20 + 4 * c, w);
        }
        let r = jit(fx, 0x01E0_6924, 0x50, 0x28);
        let f21 = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), f20), 180.0);
        let f20 = ee::add(0.5, r);
        let c = libm_cosf(fx, f21);
        fx.mem.setf(q + 0x2C, ee::mul(f20, c));
        let s = fx.libm_sinf(f21);
        fx.mem.setf(q + 0x34, ee::mul(f20, s));
        fx.mem.write_u32(q + 0x30, 0x3ECC_CCCD);
    }
}

/// Update 83 [F 0x1E06A24]: four arcs from nodes +0x18.. to node +0x28.
fn arcs(fx: &mut Fx) {
    for s2 in 0..4u32 {
        let r = ARCS + 0x60 * s2;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        fx.mem.write_u8(r + 4, 3);
        fx.mem.write_u32(r + 8, POOL);
        fx.mem.write_u8(r + 0x30, 0x50);
        fx.mem.write_u8(r + 0x31, 8);
        let v = fx.mem.read_u8(D + 0x18 + 4 * s2);
        fx.mem.write_u8(r + 0x32, v);
        let v = fx.mem.read_u8(D + 0x28);
        fx.mem.write_u8(r + 0x33, v);
        fx.mem.write_u32(r + 0x38, 0x3F80_0000);
        fx.mem.write_u32(r + 0x40, 0x4248_0000);
        fx.mem.write_u8(r + 0x34, 0);
        for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0x80), (0x49, 0x40), (0x4A, 0x40), (0x48, 0x10), (0x4D, 0x40), (0x4E, 0x40), (0x4C, 0x10), (0x4F, 0x40)] {
            fx.mem.write_u8(r + o, v);
        }
        let len = fx.mem.u8(r + 0x31) as u32;
        for k in 0..=len {
            fx.mem.set16(r + 0x1E + 2 * k, -1);
            fx.mem.set16(r + 0xC + 2 * k, -1);
        }
    }
}

/// Update 99 [F 0x1E06B14]: at the target (sp + 0x80) the fade, 32
/// bands, a flash, a spike, 200 sprites and a shake.
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 0xC, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    for _ in 0..0x20 {
        let i = first_free(fx, BANDS, 0x5C, 0x20);
        if i < 0 {
            continue;
        }
        let delay = rand4(fx, 0x01E0_6B54) & 0xF;
        let b = BANDS + 0x5C * i as u32;
        set_counter4(fx, b, 0x20, delay, 0xFF);
        for (o, v) in [(5u32, 0u8), (7, 0x24), (0xC, 2), (8, 1), (9, 2), (0xA, 0), (0xB, 4)] {
            fx.mem.write_u8(b + o, v);
        }
        let f = ee::add(3.0, jit(fx, 0x01E0_6BB4, 0xBE, 0x5F));
        fx.mem.setf(b + 0x10, f);
        fx.mem.write_u32(b + 0x14, 0);
        for o in [0x28u32, 0x29, 0x2A, 0x24, 0x25, 0x26, 0x27, 0x2B] {
            fx.mem.write_u8(b + o, 0x80);
        }
        for (o, ra) in [(0x3Cu32, 0x01E0_6C24u32), (0x40, 0x01E0_6C34), (0x44, 0x01E0_6C44)] {
            let v = rand4(fx, ra) & 0x7FFF;
            fx.mem.set32(b + o, v);
        }
        let f = ee::add(2.0, jit(fx, 0x01E0_6C54, 0xBE, 0x5F));
        fx.mem.setf(b + 0x18, f);
        fx.mem.setf(b + 0x1C, ee::sub(f, f32::from_bits(0x3ECC_CCCD)));
        copy_v4(fx, sp + 0x80, b + 0x2C);
    }
    let i = first_free(fx, FLASH, 0x4C, 6);
    if i >= 0 {
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 0x30, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9)] {
            fx.mem.write_u8(f + o, v);
        }
        fx.mem.write_u32(f + 0x10, 0x4448_0000);
        fx.mem.write_u32(f + 0x14, 0);
        for (o, v) in [(0xFu32, 0xF0u8), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0), (9, 0), (8, 0xC0)] {
            fx.mem.write_u8(f + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(f + o, 0);
        }
        copy_v4(fx, sp + 0x80, f + 0x1C);
    }
    let i = first_free(fx, SPIKES, 0x68, 0x3C);
    if i >= 0 {
        let q = SPIKES + 0x68 * i as u32;
        fx.mem.write_u8(q + 4, 5);
        set_counter4(fx, q, 0x2E, 0, 0xFF);
        fx.mem.write_u32(q + 0x14, 0x3FE6_6666);
        fx.mem.write_u32(q + 0x1C, 0x4270_0000);
        for (o, v) in [(0x27u32, 0x40u8), (0x23, 0), (5, 0), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0xC8)] {
            fx.mem.write_u8(q + o, v);
        }
        let x = rand4(fx, 0x01E0_6E34);
        fx.mem.set8(q + 9, x);
        fx.mem.write_u32(q + 0x10, BYTES);
        fx.mem.write_u32(q + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(q + o, 0x80);
        }
        copy_v4(fx, sp + 0x80, q + 0x28);
        fx.mem.write_u32(q + 0x58, 0x40);
        fx.mem.write_u32(q + 0x5C, 0x80);
        fx.mem.write_u32(q + 0x60, 0x100);
    }
    for _ in 0..0xC8 {
        let i = first_free(fx, DROPS, 0x44, 0xFA);
        if i < 0 {
            continue;
        }
        let delay = rand4(fx, 0x01E0_6ECC) & 0xF;
        let q = DROPS + 0x44 * i as u32;
        set_counter4(fx, q, 0x2E, delay, 0xFF);
        for (o, v) in [(4u32, 1u8), (8, 1), (0xB, 2), (0xA, 0), (9, 3), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        let f = ee::add(100.0, spread(fx, 0x01E0_6F2C, 0x14, 0x7D0));
        fx.mem.setf(q + 0x10, f);
        fx.mem.write_u32(q + 0x14, 0xBF80_0000);
        fx.mem.write_u32(q + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(q + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * c);
            fx.mem.write_u32(q + 0x20 + 4 * c, w);
        }
        let f = jit(fx, 0x01E0_6FAC, 0xBE, 0x5F);
        fx.mem.setf(q + 0x2C, f);
        let f = ee::add(1.0, jit(fx, 0x01E0_6FE8, 0xBE, 0x5F));
        fx.mem.setf(q + 0x30, f);
        let f = jit(fx, 0x01E0_702C, 0xBE, 0x5F);
        fx.mem.setf(q + 0x34, f);
    }
    set_counter4(fx, D + 0x40, 0x32, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x58, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(D), 0x01E0_70C8);
}

/// Updates 56..97 [F 0x1E07168]: eight sprites a update from node +0xC.
fn sprites(fx: &mut Fx, sp: u32) {
    let (h, n) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, n, sp + 0x90, u64::from(D), Some(u64::from(OT)), 0x01E0_7178);
    for _ in 0..8 {
        let i = first_free(fx, DROPS, 0x44, 0xFA);
        if i < 0 {
            continue;
        }
        let r = rand4(fx, 0x01E0_71A0) % 4;
        let f20 = ee::add(12.0, ee::div(ee::from_i32(200 * r - 400), 100.0));
        fx.fix_residue(f20, 0x01E0_7200);
        let count = ee::fixsfsi(f20) & 0xFF;
        let q = DROPS + 0x44 * i as u32;
        set_counter4(fx, q, count, 0, 0xFF);
        fx.mem.write_u8(q + 4, 0x41);
        fx.mem.write_u8(q + 8, 2);
        fx.mem.write_u8(q + 0xB, 2);
        let v = rand4(fx, 0x01E0_723C) & 7;
        fx.mem.set8(q + 9, v);
        fx.mem.write_u8(q + 0xA, 0);
        let v = (rand4(fx, 0x01E0_7250) & 0x3F) + 0x60;
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.set8(q + o, v);
        }
        fx.mem.write_u32(q + 0x10, 0x4270_0000);
        fx.mem.write_u32(q + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(q + 0x1C, 0);
        fx.mem.write_u32(q + 0x18, 0);
        let f21 = spread(fx, 0x01E0_728C, 7, 0x2BC);
        let g = spread(fx, 0x01E0_72E0, 7, 0x2BC);
        fx.mem.setf(q + 0x2C, ee::neg(ee::div(f21, f20)));
        fx.mem.setf(q + 0x30, ee::neg(ee::div(g, f20)));
        let mut f = ee::add(1.0, jit(fx, 0x01E0_7350, 0x64, 0x32));
        fx.mem.setf(q + 0x34, f);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            f = ee::mul(f, -1.0);
            fx.mem.setf(q + 0x34, f);
        }
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0x90 + 4 * c), ee::mul(3.0, fx.mem.f(q + 0x2C + 4 * c)));
            fx.mem.setf(q + 0x20 + 4 * c, v);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(q + o, 0);
        }
    }
}

/// The streaks of update 109 [F 0x1E070D4].
fn streaks(fx: &mut Fx) {
    let s = STREAKS;
    set_counter4(fx, s, 0x30, 0, 0xFF);
    for (o, v) in [(4u32, 0x80u8), (9, 1), (0xA, 0xFF), (8, 0), (0xB, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(s + 0xC, h);
    for (o, v) in [(0x14u32, 0x80u8), (0x15, 0x80), (0x16, 0x80), (0x17, 0x60), (0x10, 0x80), (0x11, 0x80), (0x12, 0x80), (0x13, 0x60), (5, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, v);
}

/// 0x1E06100(D, from, to) [F]: on updates from..=to four flashes (+0x672C,
/// the first four of six) on the line from node +0x28 to the midpoint of
/// nodes +0x1C / +0x20, started on `from` (count to - from), bright on
/// odd updates. Its 0xD0 frame keeps its points at sp + 0xB0 / + 0xC0.
fn line_flashes(fx: &mut Fx, from: i32, to: i32) {
    // The prologue saves the entry's s0 / s1 (D / the OT), the runner's
    // s6 / s7 / fp (0) and ra on every update, in range or not (fp's upper
    // half clears the u16 pair the smears' descriptor UV keeps the top bits
    // of, caller sp - 0x38) [run]; the entry's s2..s5 / f20..f22 [H] left.
    let f = fx.sp - 0xD0;
    fx.saved(f, &[(0x10, u64::from(D)), (0x20, u64::from(OT)), (0x70, 0), (0x80, 0), (0x90, 0), (0xA0, 0x01E0_7430)]);
    let n = fx.mem.u16(D);
    if n < from || to < n {
        return;
    }
    let sp = fx.enter(0xD0);
    let h = fx.mem.i32(D + 4);
    let s = fx.mem.i32(D + 0x1C);
    node_position_saving(fx, h, s, sp + 0xB0, to as u32 as u64, Some(u64::from(OT)), 0x01E0_6174);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x20));
    node_position_saving(fx, h, s, sp + 0xC0, to as u32 as u64, Some(u64::from(OT)), 0x01E0_6184);
    for c in 0..3u32 {
        let v = ee::div(ee::add(fx.mem.f(sp + 0xB0 + 4 * c), fx.mem.f(sp + 0xC0 + 4 * c)), 2.0);
        fx.mem.setf(sp + 0xB0 + 4 * c, v);
    }
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x28));
    node_position_saving(fx, h, s, sp + 0xC0, to as u32 as u64, Some(u64::from(OT)), 0x01E0_61E8);
    let mut step = [0f32; 3];
    for (c, st) in step.iter_mut().enumerate() {
        let c = c as u32;
        *st = ee::div(ee::sub(fx.mem.f(sp + 0xB0 + 4 * c), fx.mem.f(sp + 0xC0 + 4 * c)), 4.0);
    }
    for s0 in 0..4u32 {
        let k = ee::from_i32(s0 as i32);
        for (c, st) in step.iter().enumerate() {
            let c = c as u32;
            let v = ee::add(fx.mem.f(sp + 0xC0 + 4 * c), ee::mul(*st, k));
            fx.mem.setf(sp + 0xB0 + 4 * c, v);
        }
        let r = FLASH + 0x4C * s0;
        if fx.mem.u16(D) == from {
            set_counter4(fx, r, (to - from) & 0xFF, 0, 0xFF);
            for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 4)] {
                fx.mem.write_u8(r + o, v);
            }
            fx.mem.write_u32(r + 0x10, 0x4334_0000);
            fx.mem.write_u32(r + 0x14, 0);
            fx.mem.write_u32(r + 0x18, 0x4120_0000);
            for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x80), (8, 0x10)] {
                fx.mem.write_u8(r + o, v);
            }
            for o in [0x44u32, 0x40, 0x3C] {
                fx.mem.write_u32(r + o, 0);
            }
        }
        let a = if fx.mem.u16(D) & 1 != 0 { 0xC0 } else { 0x70 };
        fx.mem.write_u8(r + 0xF, a);
        copy_v4(fx, sp + 0xB0, r + 0x1C);
    }
    fx.leave(0xD0);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x80, u64::from(D), 0x01E0_6500);
    match fx.mem.u16(D) {
        0xB4 => return 8,
        0x6D => streaks(fx),
        0x63 => strike(fx, sp),
        0x53 => arcs(fx),
        0x20 => {
            set_counter4(fx, D + 0x40, 0x43, 0, 0xFF);
            fx.mem.write_u8(D + 0x45, 1);
            let v = fx.mem.read_u8(D + 0x28);
            fx.mem.write_u8(D + 0x46, v);
            fx.mem.write_u8(D + 0x44, 4);
            fx.mem.write_u32(D + 0x54, 0x3FC0_0000);
            fx.mem.write_u32(D + 0x58, 0x3FC0_0000);
            fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
            fx.mem.write_u32(D + 0x70, 0);
            fx.mem.write_u32(D + 0x74, 0x42A0_0000);
            fx.mem.write_u32(D + 0x90, 0x4080_0000);
        }
        0x1E => fan(fx, sp),
        9 => bars(fx),
        8 => smears(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x38..0x62).contains(&n) {
        sprites(fx, sp);
    }
    line_flashes(fx, 0x20, 0x6B);
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_7438, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_7444, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, FADE, OT);
    blib::smear::smears(fx, 0x01E0_2830, SMEARS, OT, 2);
    blib::arc::arcs_b(fx, 0x01E0_1710, ARCS, OT, 4);
    blib::cross::bars_b(fx, 0x01E0_0E10, BARS, OT, 2);
    blib::star::flash_b(fx, 0x01E0_03A0, FLASH, OT, 6);
    blib::radiate::streaks_b(fx, 0x01E0_32B0, STREAKS, OT, 1);
    blib::spike::spikes(fx, 0x01E0_40B0, SPIKES, OT, 0x3C);
    blib::drop::drops(fx, DROPS, OT, 0xFA);
    blib::band::bands_b(fx, 0x01E0_59C0, BANDS, OT, 0x20);
    if fx.mem.u16(D) == 0x1F {
        request_nodes(fx, 0x16);
    }
    let n = fx.mem.u16(D);
    if (0x63..0x8C).contains(&n) {
        6
    } else if n == 0x8C {
        2
    } else if n == 0x59 {
        1
    } else {
        0
    }
}
