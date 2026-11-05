//! BMODULE b370 (card 534), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06320, data area a0 = 0x1ED0000, 0x7100 bytes)
//! [F]: s6 = D, s0 = the OT (0x206E40), s1 the
//! parameter then pointers, s2..s5 counters. Update 2 a long flash
//! (+0x4E5C) and a long shake at node 0x27, two bolt emitters (+0x621C,
//! `emit::segment_spawns_b`, pool +0x531C) on it; 32 the attacker's
//! streaks (+0x35C4); 46 the fade (+0x6ADC), a shake at the target (sp +
//! 0xB0), eight bead beams (+0x63FC) on node 0x27, two flashes there,
//! six jagged bolts (0x102A50, `bolt::bolts_b`) from the node to the
//! target and two scrolling tubes (+0x70A4, `segtube::tubes`) on 24
//! points (+0x6E64) from the node to 20 beyond the target in z; 80 a
//! flash and three bolt emitters at the target; 115 two domes (+0x6AE4,
//! `dome::domes`) there, their alpha + 2 a update over 115..239; 200 the
//! fade again. Odd updates 3..79 a spike on node 0x27, 81..211 at the
//! target; updates 2..12 twenty sprites (+0xA4, `drop::drops`) from node
//! 0x27. Returns 6 on updates 47..236, 2 on 237, 1 on 37, 8 on 290 (no
//! drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `streaks_b`
//! 0x1E003A0, `flash_b` 0x1E011A0, `spikes` 0x1E01C10, `bolts_b`
//! 0x1E026F0, `segment_spawns_b` 0x1E03180, `beads_at` 0x1E034C0, `fade`
//! 0x1E04140, `drops` 0x1E04610, `domes` 0x1E04F70, `tubes` 0x1E059B0
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const DROPS: u32 = D + 0xA4;
const STREAKS: u32 = D + 0x35C4;
const SPIKES: u32 = D + 0x35FC;
const FLASH: u32 = D + 0x4E5C;
const BOLTS: u32 = D + 0x531C;
const EMIT: u32 = D + 0x621C;
const BEADS: u32 = D + 0x63FC;
const FADE: u32 = D + 0x6ADC;
const DOMES: u32 = D + 0x6AE4;
const BYTES: u32 = D + 0x6D64;
const PTS: u32 = D + 0x6E64;
const TUBES: u32 = D + 0x70A4;

/// The entry 0x1E06320 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1E0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x1E0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x7100).
    fx.bzero(D, 0x7100);
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
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x27, 1);
    fx.mem.set32(D + 0xC, r);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x11C);
    // The entry's s0 = 3 (the texture loop's), s1 = the parameter, s2 the
    // runner's (0).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_640C, [Some(3), Some(param as u32), Some(0)]);
    0
}

/// rand() >> 4 with the entry's s0 (the OT) saved in its frame.
fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(OT, ra) >> 4
}

/// node_position(self, node 0x27 (D[0xC]) or D[at], out) with the entry's
/// s0 (the OT) / s1.
fn node_at(fx: &mut Fx, at: u32, out: u32, s1: u64, ra: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + at));
    node_position_saving(fx, h, s, out, u64::from(OT), Some(s1), ra);
}

/// A flash record's common fields: kind `k` (+4), unit flag `unit` (+5,
/// node D[0xC] at +6), colour 0x80 / `c`, size `size`.
fn flash_fields(fx: &mut Fx, f: u32, unit: u8, k: u8, c: u8, size: u32) {
    fx.mem.write_u8(f + 5, unit);
    if unit != 0 {
        let v = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(f + 6, v);
    }
    for (o, v) in [(4u32, k), (7, 9), (0xA, 0x40), (9, 0x20), (8, 0x80), (0xF, c), (0xC, c), (0xD, c), (0xE, c)] {
        fx.mem.write_u8(f + o, v);
    }
    fx.mem.write_u32(f + 0x10, size);
}

/// Update 2 [F 0x1E06490]: a long flash and shake at node 0x27 and two
/// bolt emitters on the attacker's nodes D[0xC] / D[0x10].
fn start(fx: &mut Fx) {
    let i = first_free(fx, FLASH, 0x4C, 0x10);
    if i >= 0 {
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 0xFF, 0, 0xFF);
        flash_fields(fx, f, 1, 0x85, 0xC0, 0x44BB_8000);
        fx.mem.write_u32(f + 0x14, 0);
    }
    set_counter4(fx, D + 0x40, 0xFA, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 1);
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(D + 0x46, v);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x3F99_999A);
    fx.mem.write_u32(D + 0x58, 0x3F99_999A);
    fx.mem.write_u32(D + 0x5C, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x4000_0000);
    for s1 in 0..2u32 {
        let i = first_free(fx, EMIT, 0x3C, 8);
        let e = EMIT.wrapping_add((0x3C * i) as u32);
        set_counter4(fx, e, 0x1A, 4, 0);
        fx.mem.write_u8(e + 5, 1);
        let v = fx.mem.read_u8(D + 0xC + 4 * s1);
        fx.mem.write_u8(e + 6, v);
        fx.mem.write_u8(e + 4, 2);
        fx.mem.write_u32(e + 8, BOLTS);
        for (o, v) in [(0xCu32, 0x3Cu8), (0xD, 8), (0xE, 1), (0xF, 0), (0x10, 5), (0x11, 0xC)] {
            fx.mem.write_u8(e + o, v);
        }
        fx.mem.set32(e + 0x14, 0x4B0);
        fx.mem.write_u32(e + 0x18, 0x3F80_0000);
        fx.mem.write_u32(e + 0x1C, 0x3ECC_CCCD);
        for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20)] {
            fx.mem.write_u8(e + o, v);
        }
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(e + o, 0);
        }
    }
}

/// Update 32 [F 0x1E06668]: the attacker's streaks.
fn streaks(fx: &mut Fx) {
    let s = STREAKS;
    set_counter4(fx, s, 0x1C, 0, 0xFF);
    for (o, v) in [(4u32, 0x80u8), (9, 2), (0xA, 0xFF), (8, 8), (0xB, 0x18)] {
        fx.mem.write_u8(s + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(s + 0xC, h);
    for (o, v) in [(0x14u32, 0x80u8), (0x15, 0x80), (0x16, 0x80), (0x17, 0), (0x10, 0x90), (0x11, 0x90), (0x12, 0x90), (0x13, 0x40), (5, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let v = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(s + 6, v);
}

/// Update 46 [F 0x1E066E8]: the fade, a shake at the target (sp + 0xB0),
/// beads and flashes on node 0x27 (sp + 0xC0), bolts and tubes from it to
/// the target. Returns the entry's s1 / s5 after it.
fn strike(fx: &mut Fx, sp: u32) -> (u64, u64) {
    set_counter4(fx, FADE, 0x18, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    set_counter4(fx, D + 0x40, 0xBE, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 8);
    fx.mem.write_u32(D + 0x54, 0x3F80_0000);
    fx.mem.write_u32(D + 0x58, 0x3FB3_3333);
    fx.mem.write_u32(D + 0x5C, 0x4000_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4320_0000);
    copy_v4(fx, sp + 0xB0, D + 0x60);
    fx.mem.write_u32(D + 0x90, 0x4100_0000);
    fx.mem.write_u32(D + 0x68, 0);
    for s2 in 0..8u32 {
        let b = BEADS + 0x58 * s2;
        set_counter4(fx, b, 0x20, (s2 & 0xFF) as i32, 0xFF);
        let c = fx.mem.read_u8(b).wrapping_sub(4);
        fx.mem.write_u8(b, c);
        for (o, v) in [(4u32, 5u8), (5, 0), (8, 0x32), (9, s2 as u8), (0xA, 0), (0xC, 2)] {
            fx.mem.write_u8(b + o, v);
        }
        let v = rand4(fx, 0x01E0_67C8) & 3;
        fx.mem.set8(b + 0xB, v);
        fx.mem.write_u32(b + 0x1C, 0x4316_0000);
        fx.mem.write_u32(b + 0x20, 0xBF19_999A);
        fx.mem.write_u32(b + 0x10, BYTES);
        fx.mem.write_u32(b + 0x14, 0x4000_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(b + o, 0x80);
        }
        node_at(fx, 0xC, b + 0x28, u64::from(D + 0x58 * s2), 0x01E0_681C);
    }
    node_at(fx, 0xC, sp + 0xC0, u64::from(D + 0x58 * 8), 0x01E0_683C);
    let i = first_free(fx, FLASH, 0x4C, 0x10);
    if i >= 0 {
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 0x18, 0, 0xFF);
        flash_fields(fx, f, 0, 0x80, 0xF0, 0x42C8_0000);
        fx.mem.write_u32(f + 0x14, 0x4316_0000);
        fx.mem.write_u32(f + 0x18, 0x4270_0000);
        copy_v4(fx, sp + 0xC0, f + 0x1C);
    }
    let i = first_free(fx, FLASH, 0x4C, 0x10);
    if i >= 0 {
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 0x40, 0xC, 0xFF);
        flash_fields(fx, f, 1, 0x84, 0xF0, 0x44BB_8000);
        fx.mem.write_u32(f + 0x14, 0);
        fx.mem.write_u32(f + 0x18, 0);
    }
    let p = sp + 0xE0;
    let mut s5 = 0;
    while s5 < 6 {
        fx.mem.set32(p, 6);
        fx.mem.write_u32(p + 4, 0x3ECC_CCCD);
        fx.mem.write_u32(p + 0x20, sp + 0x110);
        for (dst, src) in [(0x8u32, 0xC0u32), (0xC, 0xC4), (0x10, 0xC8), (0x14, 0xB0), (0x18, 0xB4), (0x1C, 0xB8)] {
            let w = fx.mem.read_u32(sp + src);
            fx.mem.write_u32(p + dst, w);
        }
        blib::bolt::jag(fx, p);
        let mut broke = false;
        for s4 in 0..6u32 {
            let i = first_free(fx, BOLTS, 0x40, 0x3C);
            if i < 0 {
                broke = true;
                break;
            }
            let b = BOLTS + ((i as u32) << 6);
            set_counter4(fx, b, 0xC2, s5 & 1, 0xFF);
            fx.mem.write_u8(b + 5, 0);
            fx.mem.write_u8(b + 4, 0);
            for c in 0..6u32 {
                let w = fx.mem.read_u32(sp + 0x110 + 0xC * s4 + 4 * c);
                fx.mem.write_u32(b + 0x1C + 4 * c, w);
            }
            fx.mem.write_u8(b + 8, 9);
            fx.mem.write_u32(b + 0x10, 0x3F33_3333);
            fx.mem.write_u32(b + 0x14, 0x3E4C_CCCD);
            for (o, v) in [(0xBu32, 1u8), (0xD, 3), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)] {
                fx.mem.write_u8(b + o, v);
            }
        }
        if broke {
            // [F 0x1E06A28] bltzl: s5 + 1 and the outer test.
        }
        s5 += 1;
    }
    let dz: f32 = if fx.mem.read_u32(D + 0x30) == 0x8000 { 20.0 } else { -20.0 };
    copy_v4(fx, sp + 0xC0, sp + 0xD0);
    let sx = ee::div(ee::sub(fx.mem.f(sp + 0xB0), fx.mem.f(sp + 0xC0)), 24.0);
    let sy = ee::div(ee::sub(fx.mem.f(sp + 0xB4), fx.mem.f(sp + 0xC4)), 24.0);
    let sz = ee::div(ee::sub(ee::sub(fx.mem.f(sp + 0xB8), fx.mem.f(sp + 0xC8)), dz), 24.0);
    for a1 in 0..0x18u32 {
        let k = ee::from_i32(a1 as i32);
        let q = PTS + 12 * a1;
        let x = ee::add(fx.mem.f(sp + 0xC0), ee::mul(sx, k));
        fx.mem.setf(q, x);
        let y = ee::add(fx.mem.f(sp + 0xC4), ee::mul(sy, k));
        fx.mem.setf(q + 4, y);
        // adda.s / madd.s: the sum, then the product added.
        let z = ee::add(ee::add(fx.mem.f(sp + 0xC8), dz), ee::mul(sz, k));
        fx.mem.setf(q + 8, z);
    }
    for s2 in 0..2u32 {
        let t = TUBES + 0x2C * s2;
        set_counter4(fx, t, 0xBE, 0, 0xFF);
        for (o, v) in [(7u32, 0xCu8), (8, 0x17), (0xB, 8), (9, 2), (0xA, 2)] {
            fx.mem.write_u8(t + o, v);
        }
        fx.mem.write_u32(t + 0xC, PTS);
        fx.mem.write_u32(t + 0x14, 0x3F80_0000);
        fx.mem.write_u32(t + 0x18, 0x3CA3_D70A);
        fx.mem.write_u32(t + 0x1C, 0x3E99_999A);
        if s2 != 0 {
            fx.mem.write_u32(t + 0x10, 0x3F80_0000);
            fx.mem.write_u32(t + 0x20, 0x4000_0000);
            for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
                fx.mem.write_u8(t + o, 0x80);
            }
        } else {
            fx.mem.write_u32(t + 0x10, 0x4000_0000);
            fx.mem.write_u32(t + 0x20, 0x4080_0000);
            for o in [0x28u32, 0x29, 0x2A] {
                fx.mem.write_u8(t + o, 0x80);
            }
            fx.mem.write_u8(t + 0x2B, 0x40);
        }
        fx.mem.write_u32(t + 0x24, 0);
    }
    (u64::from(D + 0x58), 6)
}

/// Update 80 [F 0x1E06CB0]: a flash and three bolt emitters at the
/// target.
fn target_bolts(fx: &mut Fx, sp: u32) {
    let i = first_free(fx, FLASH, 0x4C, 0x10);
    if i >= 0 {
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 0xA0, 0, 0xFF);
        fx.mem.write_u8(f + 5, 0);
        let v = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(f + 6, v);
        flash_fields(fx, f, 0, 0x83, 0xC0, 0x4416_0000);
        fx.mem.write_u32(f + 0x14, 0);
        copy_v4(fx, sp + 0xB0, f + 0x1C);
    }
    for s2 in 0..3u32 {
        let e = EMIT + 0x3C * s2;
        set_counter4(fx, e, 0x90, s2 as i32, 3);
        fx.mem.write_u8(e + 5, 0);
        fx.mem.write_u32(e + 8, BOLTS);
        fx.mem.write_u8(e + 4, 0);
        for (o, v) in [(0xCu32, 0x3Cu8), (0xD, 0xC), (0xE, 1), (0xF, 0), (0x10, 6), (0x11, 0x10)] {
            fx.mem.write_u8(e + o, v);
        }
        fx.mem.set32(e + 0x14, 0xFA0);
        fx.mem.write_u32(e + 0x18, 0x3F4C_CCCD);
        fx.mem.write_u32(e + 0x1C, 0x3ECC_CCCD);
        for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x30)] {
            fx.mem.write_u8(e + o, v);
        }
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xB0 + 4 * c);
            fx.mem.write_u32(e + 0x24 + 4 * c, w);
        }
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(e + o, 0);
        }
    }
}

/// Update 115 [F 0x1E06E3C]: two domes at the target (the second turned
/// 0x4000 about z), alpha 0.
fn domes(fx: &mut Fx, sp: u32) {
    for s2 in 0..2u32 {
        let d = DOMES + 0x50 * s2;
        set_counter4(fx, d, 0x78, s2 as i32, 0xFF);
        for (o, v) in [(0xAu32, 8u8), (7, 9), (8, 2), (9, 2)] {
            fx.mem.write_u8(d + o, v);
        }
        fx.mem.write_u32(d + 0xC, 0x4040_0000);
        fx.mem.write_u32(d + 0x10, 0xBD23_D70A);
        fx.mem.write_u32(d + 0x14, 0x4248_0000);
        if s2 != 0 {
            fx.mem.set32(d + 0x38, 0x4000);
        }
        fx.mem.write_u32(d + 0x18, 0);
        copy_v4(fx, sp + 0xB0, d + 0x20);
        for o in [0x1Cu32, 0x1D, 0x1E] {
            fx.mem.write_u8(d + o, 0x80);
        }
        fx.mem.write_u8(d + 0x1F, 0);
    }
}

/// Odd updates 3..79 / 81..211 [F 0x1E06F80 / 0x1E070BC]: a spike on node
/// 0x27 or at the target (sp + 0xB0). Returns the entry's s1 (0x68 i).
fn spike(fx: &mut Fx, sp: u32, near: bool, ra: [u32; 4]) -> Option<u64> {
    let i = first_free(fx, SPIKES, 0x68, 0x3C);
    if i < 0 {
        return None;
    }
    let q = SPIKES + 0x68 * i as u32;
    set_counter4(fx, q, 0x1C, 0, 0xFF);
    fx.mem.write_u8(q + 4, 5);
    if near {
        fx.mem.write_u32(q + 0x14, 0x4040_0000);
        fx.mem.write_u32(q + 0x1C, 0x4270_0000);
        fx.mem.write_u8(q + 8, 0x1E);
        fx.mem.write_u8(q + 5, 1);
        let v = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(q + 6, v);
    } else {
        fx.mem.write_u32(q + 0x14, 0x4000_0000);
        fx.mem.write_u32(q + 0x1C, 0x41F0_0000);
        fx.mem.write_u8(q + 8, 0x10);
        fx.mem.write_u8(q + 5, 0);
    }
    for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 3)] {
        fx.mem.write_u8(q + o, v);
    }
    let x = rand4(fx, ra[0]);
    fx.mem.set8(q + 9, x);
    fx.mem.write_u32(q + 0x10, BYTES);
    fx.mem.write_u32(q + 0x14 + 4, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x10), (0x27, 0x40)] {
        fx.mem.write_u8(q + o, v);
    }
    for (o, ra) in [(0x58u32, ra[1]), (0x5C, ra[2]), (0x60, ra[3])] {
        let v = rand4(fx, ra) & 0x7F;
        fx.mem.set32(q + o, v);
    }
    if !near {
        copy_v4(fx, sp + 0xB0, q + 0x28);
    }
    Some(u64::from(0x68 * i as u32))
}

/// ((rand >> 4) % 100 x 200 - 10000) / 100.
fn jit(fx: &mut Fx, ra: u32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, 0x64);
    ee::div(ee::from_i32(200 * r - 0x2710), 100.0)
}

/// Updates 2..12 [F 0x1E07204]: twenty sprites from node 0x27 / 0x2B
/// (alternating D[0xC] / D[0x10]), thrown outwards. Returns the entry's
/// s1 after it.
fn sprites(fx: &mut Fx, sp: u32, mut s1: u64) -> u64 {
    for s4 in 0..0x14u32 {
        let i = first_free(fx, DROPS, 0x44, 0xC8);
        s1 = i as i64 as u64;
        if i < 0 {
            continue;
        }
        node_at(fx, 0xC + 4 * (s4 & 1), sp + 0xC0, s1, 0x01E0_7240);
        let r = rand4(fx, 0x01E0_7248) % 8;
        let f20 = ee::add(16.0, ee::div(ee::from_i32(200 * r - 0x320), 100.0));
        fx.fix_residue(f20, 0x01E0_72A8);
        let count = ee::fixsfsi(f20) & 0xFF;
        let p = DROPS + 0x44 * i as u32;
        s1 = u64::from(p);
        set_counter4(fx, p, count, 0, 0xFF);
        fx.mem.write_u8(p + 4, 0x41);
        fx.mem.write_u8(p + 8, 0);
        fx.mem.write_u8(p + 0xB, 2);
        let v = rand4(fx, 0x01E0_72E4) & 7;
        fx.mem.set8(p + 9, v);
        fx.mem.write_u8(p + 0xA, 0);
        for o in [0xCu32, 0xD, 0xE] {
            fx.mem.write_u8(p + o, 0x80);
        }
        fx.mem.write_u8(p + 0xF, 0x60);
        fx.mem.write_u32(p + 0x10, 0x4348_0000);
        fx.mem.write_u32(p + 0x14, 0xC0C0_0000);
        fx.mem.write_u32(p + 0x1C, 0);
        fx.mem.write_u32(p + 0x18, 0);
        for (c, ra) in [(0u32, 0x01E0_7328), (4, 0x01E0_737C), (8, 0x01E0_73D0)] {
            let f = jit(fx, ra);
            fx.mem.setf(sp + 0xD0 + c, f);
        }
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0xC0 + 4 * c), fx.mem.f(sp + 0xD0 + 4 * c));
            fx.mem.setf(p + 0x20 + 4 * c, v);
        }
        for c in 0..3u32 {
            let v = ee::neg(ee::div(fx.mem.f(sp + 0xD0 + 4 * c), f20));
            fx.mem.setf(p + 0x2C + 4 * c, v);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(p + o, 0);
        }
    }
    s1
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xB0, u64::from(OT), 0x01E0_642C);
    let (mut s1, mut s5) = (param as i64 as u64, 0u64);
    match fx.mem.u16(D) {
        0x122 => return 8,
        0xC8 => {
            set_counter4(fx, FADE, 0x20, 0, 0xFF);
            fx.mem.write_u8(FADE + 4, 0x82);
        }
        0x73 => domes(fx, sp),
        0x50 => target_bolts(fx, sp),
        0x2E => (s1, s5) = strike(fx, sp),
        0x20 => streaks(fx),
        2 => {
            start(fx);
            s1 = 2;
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x73..0xF0).contains(&n) {
        for k in 0..2u32 {
            let a = DOMES + 0x50 * k + 0x1F;
            let v = fx.mem.read_u8(a).wrapping_add(2);
            fx.mem.write_u8(a, v);
        }
    }
    let n = fx.mem.u16(D);
    if (2..0x50).contains(&n) && n & 1 != 0 {
        if let Some(v) = spike(fx, sp, true, [0x01E0_7010, 0x01E0_705C, 0x01E0_7070, 0x01E0_7084]) {
            s1 = v;
        }
    }
    let n = fx.mem.u16(D);
    if (0x50..0xD4).contains(&n) && n & 1 != 0 {
        if let Some(v) = spike(fx, sp, false, [0x01E0_7140, 0x01E0_718C, 0x01E0_71A0, 0x01E0_71B4]) {
            s1 = v;
        }
    }
    let n = fx.mem.u16(D);
    if (2..0xD).contains(&n) {
        s1 = sprites(fx, sp, s1);
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_74A4, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_74B0, [Some(u64::from(OT)), Some(s1)]);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the entry's s0 (the OT), s1, s5, s6 (D) and
    // ra; s6's slot is the streaks' vertex 1 XYZ (their first TEXFLUSH's
    // a0, as b601); s2..s4 [H] left.
    fx.saved(sp - 0xF0, &[(0x10, u64::from(OT)), (0x20, s1), (0x60, s5), (0x70, u64::from(D)), (0x80, 0x01E0_74BC)]);
    blib::radiate::streaks_b(fx, 0x01E0_03A0, STREAKS, OT, 1);
    blib::star::flash_b(fx, 0x01E0_11A0, FLASH, OT, 0x10);
    blib::spike::spikes(fx, 0x01E0_1C10, SPIKES, OT, 0x3C);
    blib::emit::segment_spawns_b(fx, 0x01E0_3180, EMIT, 8);
    // segment_spawns_b's 0x80 frame saves s0..s4 (+0x10..) and ra (+0x60):
    // ra and s4's upper half are the stale TEX1 / TEX0 bolts_b's strips
    // report when their texture is missing [run]; s2..s4 [H] 0.
    fx.saved(sp - 0x80, &[(0x10, u64::from(OT)), (0x20, s1), (0x30, 0), (0x40, 0), (0x50, 0), (0x60, 0x01E0_74FC)]);
    blib::bolt::bolts_b(fx, 0x01E0_26F0, BOLTS, OT, 0x3C);
    blib::beam::beads_at(fx, 0x01E0_34C0, 0x34, 0x3C, BEADS, OT, 0x14);
    blib::drop::drops(fx, DROPS, OT, 0xC8);
    blib::dome::domes(fx, 0x01E0_4F70, DOMES, OT, 8);
    blib::segtube::tubes(fx, 0x01E0_59B0, TUBES, OT, 2);
    let n = fx.mem.u16(D);
    if (0x2F..0xED).contains(&n) {
        6
    } else if n == 0xED {
        2
    } else if n == 0x25 {
        1
    } else {
        0
    }
}
