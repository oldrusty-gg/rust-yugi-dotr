//! BMODULE b558 (card 198), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E048B0, data area a0 = 0x1ED0000, 0x6F10 bytes)
//! [F]: b526's library (B11) on its own
//! program. s0 = D, s1 = the OT (0x206E40), a 0xB0 frame (the target
//! sp + 0x80). Update 2 a long flash (+0x5184, 16 x 0x4C) at node 0x1A
//! and a long weak shake; 100 the fade (+0x6E04), a flash and sixteen
//! bolt segments (+0x5644, 0x40) fanned round the target at angles
//! s5 x (30 +- 5) degrees (libm cosf / sinf 0x1A7158 / 0x1A7348), a band
//! (+0x10E4, 0x5C) at node 0x1A and a shake; 125 the fade, a spike (+0xA4,
//! 0x68), 200 sprites (+0x1C64, 0x44) thrown from the target, four bands,
//! a flash and a shake. Updates 2..31 six sprites a update converging on
//! node 0x1A, 18..99 a spike at node 0x1A, 36..91 a band at (then from 60
//! following) node 0x1A, 54..83 five sprites a update from node 0x16,
//! 158..179 five from node 1. The spawners (+0x6544) and beads (+0x6724)
//! are drawn, never set. Returns 6 on updates 97 and 125..153, 2 on 154,
//! 1 on 90 / 115, 8 on 220 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080 (0538257fe1fa),
//! `flash_b` 0x1E003A0, `spikes` 0x1E00E10, `bolts_b` 0x1E018F0,
//! `segment_spawns_b` 0x1E02380, `beads_at` 0x1E026C0, `fade` 0x1E03340,
//! `drops` 0x1E03810, `bands_b` 0x1E04170 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SPIKES: u32 = D + 0xA4;
const BANDS: u32 = D + 0x10E4;
const SPRITES: u32 = D + 0x1C64;
const FLASH: u32 = D + 0x5184;
const SEGS: u32 = D + 0x5644;
const SPAWNS: u32 = D + 0x6544;
const BEADS: u32 = D + 0x6724;
const FADE: u32 = D + 0x6E04;
const BYTES: u32 = D + 0x6E0C;

/// The entry 0x1E048B0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x6F10).
    fx.bzero(D, 0x6F10);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x16, 0), (0x14, 0x1A, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xD6);
    // The entry's s0 = D, s1 = 4 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_49B8, [Some(D), Some(4), Some(param as u32)]);
    0
}

fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 4
}

/// ((rand >> 4) % m - half) / 100.
fn jit(fx: &mut Fx, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

/// (200 x ((rand >> 4) % m) - `sub`) / 100.
fn jit200(fx: &mut Fx, ra: u32, m: i32, sub: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r.wrapping_mul(200) - sub), 100.0)
}

/// A flash (+0x5184) from 0x101C30: `count`, bytes +4 / +5 / +0xA / +9 /
/// +8, grey `c`, +0x10 `size`; at the target unless `node`.
fn flash(fx: &mut Fx, sp: u32, count: i32, b: [u8; 5], c: u8, size: u32, node: bool) {
    let i = first_free(fx, FLASH, 0x4C, 0x10);
    if i < 0 {
        return;
    }
    let r = FLASH + 0x4C * i as u32;
    set_counter4(fx, r, count, 0, 0xFF);
    fx.mem.write_u8(r + 5, b[1]);
    if node {
        let v = fx.mem.read_u8(D + 0x14);
        fx.mem.write_u8(r + 6, v);
    }
    for (o, v) in [(4u32, b[0]), (7, 9), (0xA, b[2]), (9, b[3]), (8, b[4]), (0xC, c), (0xD, c), (0xE, c), (0xF, 0xF0)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, size);
    fx.mem.write_u32(r + 0x14, 0);
    if !node {
        copy_v4(fx, sp + 0x80, r + 0x1C);
    }
}

/// The strike shake (+0x40) at the target: 32 counts, mode `mode`.
fn shake(fx: &mut Fx, sp: u32, mode: u8) {
    set_counter4(fx, D + 0x40, 0x20, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, mode);
    fx.mem.write_u32(D + 0x54, 0x4000_0000);
    fx.mem.write_u32(D + 0x58, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    copy_v4(fx, sp + 0x80, D + 0x60);
}

/// Update 2 [F 0x1E04A14]: a long flash at node 0x1A, a long weak shake.
fn opening(fx: &mut Fx, sp: u32) {
    flash(fx, sp, 0x64, [0x83, 1, 0x40, 0x20, 0x80], 0x80, 0x4348_0000, true);
    set_counter4(fx, D + 0x40, 0x64, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 1);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0x46, b);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x58, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x4080_0000);
}

/// Update 100 [F 0x1E04B0C]: fade, flash, sixteen bolt segments fanned
/// round the target, a band at node 0x1A, a shake.
fn fan(fx: &mut Fx, sp: u32) -> Option<u64> {
    let mut s4 = None;
    set_counter4(fx, FADE, 8, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    flash(fx, sp, 0x1E, [0x81, 0, 0x60, 0x10, 0xC0], 0xF0, 0x4496_0000, false);
    let mut s5 = 0i32;
    while s5 < 0x10 {
        let i = first_free(fx, SEGS, 0x40, 0x3C);
        if i < 0 {
            s5 += 1;
            continue;
        }
        let k = jit200(fx, 0x01E0_4C08, 5, 0x1F4);
        let deg = ee::mul(ee::from_i32(s5), ee::add(30.0, k));
        let delay = rand4(fx, 0x01E0_4C68) & 7;
        let r = SEGS + 0x40 * i as u32;
        s4 = Some(u64::from(r + 0x2C));
        set_counter4(fx, r, 0x18, delay, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 1);
        for (c, o) in [(0u32, 0x28u32), (4, 0x2C), (8, 0x30)] {
            let v = fx.mem.read_u32(sp + 0x80 + c);
            fx.mem.write_u32(r + o, v);
            fx.mem.write_u32(r + 0x1C + c, v);
        }
        let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), deg), 180.0);
        let cs = libm_cosf(fx, a);
        let x = ee::add(fx.mem.f(r + 0x28), ee::mul(40.0, cs));
        fx.mem.setf(r + 0x28, x);
        let sn = fx.libm_sinf(a);
        let y = ee::add(fx.mem.f(r + 0x2C), ee::mul(40.0, sn));
        fx.mem.setf(r + 0x2C, y);
        fx.mem.write_u8(r + 8, 8);
        fx.mem.write_u32(r + 0x10, 0x3F4C_CCCD);
        fx.mem.write_u32(r + 0x14, 0x3ECC_CCCD);
        for (o, v) in [(0xBu32, 1u8), (0xD, 4), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x28)] {
            fx.mem.write_u8(r + o, v);
        }
        s5 += 1;
    }
    let i = first_free(fx, BANDS, 0x5C, 0x20);
    if i >= 0 {
        let r = BANDS + 0x5C * i as u32;
        set_counter4(fx, r, 0x20, 0, 0xFF);
        for (o, v) in [(7u32, 0x24u8), (0xC, 2), (8, 0), (9, 2), (0xA, 0), (0xB, 5)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x40A0_0000);
        fx.mem.write_u32(r + 0x14, 0);
        for o in [0x2Bu32, 0x27, 0x28, 0x29, 0x2A, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.set32(r + 0x3C, 0x4000);
        fx.mem.write_u32(r + 0x44, 0);
        fx.mem.write_u32(r + 0x40, 0);
        fx.mem.write_u8(r + 5, 0);
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14));
        node_position_saving(fx, h, s, r + 0x2C, u64::from(D), Some(u64::from(OT)), 0x01E0_4E38);
        fx.mem.write_u32(r + 0x18, 0x3FCC_CCCD);
        fx.mem.write_u32(r + 0x1C, 0x3FC0_0000);
    }
    shake(fx, sp, 8);
    s4
}

/// Update 125 [F 0x1E04EC8]: fade, a spike, 200 sprites, four bands, a
/// flash and a shake at the target.
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 0xC, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    let i = first_free(fx, SPIKES, 0x68, 0x28);
    if i >= 0 {
        let r = SPIKES + 0x68 * i as u32;
        set_counter4(fx, r, 0x1E, 0, 0xFF);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u32(r + 0x14, 0x4040_0000);
        fx.mem.write_u32(r + 0x1C, 0x4220_0000);
        for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0xA0)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = rand4(fx, 0x01E0_4F68);
        fx.mem.set8(r + 9, x);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x40)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x80, r + 0x28);
        fx.mem.write_u32(r + 0x58, 0x40);
        fx.mem.write_u32(r + 0x5C, 0x80);
        fx.mem.write_u32(r + 0x60, 0x100);
    }
    let mut s3 = 0;
    while s3 < 0xC8 {
        let i = first_free(fx, SPRITES, 0x44, 0xC8);
        if i < 0 {
            s3 += 1;
            continue;
        }
        let delay = rand4(fx, 0x01E0_5008) & 0xF;
        let r = SPRITES + 0x44 * i as u32;
        set_counter4(fx, r, 0x20, delay, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 0), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        let v = rand4(fx, 0x01E0_5048);
        fx.mem.set8(r + 9, v & 7);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let f = ee::add(120.0, jit200(fx, 0x01E0_506C, 0x14, 0x7D0));
        fx.mem.setf(r + 0x10, f);
        fx.mem.write_u32(r + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(r + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let f = jit(fx, 0x01E0_50FC, 0x118, 0x8C);
        fx.mem.setf(r + 0x2C, f);
        let f = ee::add(f32::from_bits(0x3F4C_CCCD), jit(fx, 0x01E0_5138, 0x118, 0x8C));
        fx.mem.setf(r + 0x30, f);
        let f = jit(fx, 0x01E0_5180, 0x118, 0x8C);
        fx.mem.setf(r + 0x34, f);
        s3 += 1;
    }
    for s4 in 0..4u32 {
        let r = BANDS + 0x5C * s4;
        set_counter4(fx, r, 0x20, (2 * s4) as i32 & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 1), (8, 3), (9, 2), (0xA, 6), (0xB, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x4000_0000);
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0x4000_0000);
        fx.mem.write_u32(r + 0x1C, 0x3F80_0000);
        fx.mem.write_u32(r + 0x20, 0x3F00_0000);
        for (o, v) in [(0x28u32, 0x80u8), (0x29, 0x80), (0x2A, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0x10), (0x2B, 0x40)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x80, r + 0x2C);
        fx.mem.set32(r + 0x3C, 0x4000);
        fx.mem.write_u32(r + 0x44, 0);
        fx.mem.write_u32(r + 0x40, 0);
    }
    flash(fx, sp, 0x20, [0x81, 0, 0x60, 0x10, 0xC0], 0x80, 0x4448_0000, false);
    shake(fx, sp, 1);
}

/// Updates 2..31 [F 0x1E053FC]: six sprites a update converging on node
/// 0x1A (sp + 0x90) from up to 9 away.
fn converge(fx: &mut Fx, sp: u32) -> bool {
    let mut got = false;
    let mut s5 = 0;
    while s5 < 6 {
        let i = first_free(fx, SPRITES, 0x44, 0xC8);
        if i < 0 {
            s5 += 1;
            continue;
        }
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14));
        node_position_saving(fx, h, s, sp + 0x90, u64::from(D), Some(u64::from(OT)), 0x01E0_542C);
        let f20 = ee::add(10.0, jit200(fx, 0x01E0_5434, 6, 0x258));
        fx.fix_residue(f20, 0x01E0_548C);
        let life = ee::fixsfsi(f20);
        let r = SPRITES + 0x44 * i as u32;
        set_counter4(fx, r, life & 0xFF, 0, 0xFF);
        for (o, v) in [(4u32, 0x41u8), (8, 0), (0xB, 2), (9, 0), (0xA, 0), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x4220_0000);
        fx.mem.write_u32(r + 0x14, 0xC000_0000);
        fx.mem.write_u32(r + 0x1C, 0);
        fx.mem.write_u32(r + 0x18, 0);
        for (k, ra) in [0x01E0_54FCu32, 0x01E0_5550, 0x01E0_55A4].iter().enumerate() {
            let f = jit200(fx, *ra, 0x12, 0x708);
            fx.mem.setf(sp + 0xA0 + 4 * k as u32, f);
        }
        got = true;
        for k in 0..3u32 {
            let o = ee::add(fx.mem.f(sp + 0x90 + 4 * k), fx.mem.f(sp + 0xA0 + 4 * k));
            fx.mem.setf(r + 0x20 + 4 * k, o);
        }
        for k in 0..3u32 {
            let v = ee::neg(ee::div(fx.mem.f(sp + 0xA0 + 4 * k), f20));
            fx.mem.setf(r + 0x2C + 4 * k, v);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(r + o, 0);
        }
        s5 += 1;
    }
    got
}

/// Updates 18..99 [F 0x1E05684]: a spike at node 0x1A.
fn node_spike(fx: &mut Fx) {
    let i = first_free(fx, SPIKES, 0x68, 0x28);
    if i < 0 {
        return;
    }
    let r = SPIKES + 0x68 * i as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x3FD9_999A);
    fx.mem.write_u32(r + 0x1C, 0x41A0_0000);
    fx.mem.write_u8(r + 8, 0x10);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(r + 6, b);
    for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 3)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = rand4(fx, 0x01E0_5718);
    fx.mem.set8(r + 9, x);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x40)] {
        fx.mem.write_u8(r + o, v);
    }
    for (o, ra) in [(0x58u32, 0x01E0_5760u32), (0x5C, 0x01E0_5774), (0x60, 0x01E0_5788)] {
        let v = rand4(fx, ra) & 0x7F;
        fx.mem.set32(r + o, v);
    }
}

/// Updates 36..91 [F 0x1E057B4]: a band at node 0x1A (riding it below
/// update 60, else placed there once).
fn node_band(fx: &mut Fx) {
    let i = first_free(fx, BANDS, 0x5C, 0x20);
    if i < 0 {
        return;
    }
    let r = BANDS + 0x5C * i as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    for (o, v) in [(7u32, 0x24u8), (0xC, 2), (8, 0), (9, 2), (0xA, 0), (0xB, 5)] {
        fx.mem.write_u8(r + o, v);
    }
    let f = ee::add(1.5, jit(fx, 0x01E0_5824, 0xBE, 0x5F));
    fx.mem.setf(r + 0x10, f);
    fx.mem.write_u32(r + 0x14, 0);
    for o in [0x2Bu32, 0x27, 0x28, 0x29, 0x2A, 0x24, 0x25, 0x26] {
        fx.mem.write_u8(r + o, 0x80);
    }
    for (o, ra) in [(0x3Cu32, 0x01E0_5898u32), (0x40, 0x01E0_58AC), (0x44, 0x01E0_58C0)] {
        let v = rand4(fx, ra) & 0x7FFF;
        fx.mem.set32(r + o, v);
    }
    if fx.mem.u16(D) < 0x3C {
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0x14);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u32(r + 0x1C, 0x3F00_0000);
        fx.mem.write_u32(r + 0x18, 0x3F00_0000);
    } else {
        fx.mem.write_u8(r + 5, 0);
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14));
        node_position_saving(fx, h, s, r + 0x2C, u64::from(D), Some(u64::from(OT)), 0x01E0_5914);
        fx.mem.write_u32(r + 0x1C, 0x3F4C_CCCD);
        fx.mem.write_u32(r + 0x18, 0x3F4C_CCCD);
    }
}

/// Five sprites a update from the node in slot D+`slot` (sp + 0x90):
/// life `base` +- 3 (0x1A2B80), +0x10 `size`, the position jittered
/// (m1, h1) (none when `None`, y lowered by 1), the velocity (m2, h2).
struct Puff {
    slot: u32,
    base: f32,
    size: u32,
    jitter: Option<(i32, i32)>,
    vel: (i32, i32),
    ras: [u32; 10],
}

fn puff(fx: &mut Fx, sp: u32, p: &Puff) {
    let ra = &p.ras;
    let mut s3 = 0;
    while s3 < 5 {
        let i = first_free(fx, SPRITES, 0x44, 0xC8);
        if i < 0 {
            s3 += 1;
            continue;
        }
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + p.slot));
        node_position_saving(fx, h, s, sp + 0x90, u64::from(D), Some(u64::from(OT)), ra[0]);
        let f12 = ee::add(p.base, jit200(fx, ra[1], 6, 0x258));
        fx.fix_residue(f12, ra[2]);
        let life = ee::fixsfsi(f12);
        let r = SPRITES + 0x44 * i as u32;
        set_counter4(fx, r, life & 0xFF, 0, 0xFF);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u8(r + 8, 2);
        fx.mem.write_u8(r + 0xB, 1);
        let v = rand4(fx, ra[3]);
        fx.mem.set8(r + 9, v & 3);
        for (o, v) in [(0xAu32, 5u8), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x40)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, p.size);
        fx.mem.write_u32(r + 0x14, 0x4120_0000);
        fx.mem.write_u32(r + 0x1C, 0);
        fx.mem.write_u32(r + 0x18, 0);
        match p.jitter {
            Some((m, half)) => {
                for k in 0..3u32 {
                    let f = ee::add(fx.mem.f(sp + 0x90 + 4 * k), jit(fx, ra[4 + k as usize], m, half));
                    fx.mem.setf(r + 0x20 + 4 * k, f);
                }
            }
            None => {
                let w = fx.mem.read_u32(sp + 0x90);
                fx.mem.write_u32(r + 0x20, w);
                let y = ee::sub(fx.mem.f(sp + 0x94), 1.0);
                fx.mem.setf(r + 0x24, y);
                let w = fx.mem.read_u32(sp + 0x98);
                fx.mem.write_u32(r + 0x28, w);
            }
        }
        let (m, half) = p.vel;
        for k in 0..3u32 {
            let f = jit(fx, ra[7 + k as usize], m, half);
            fx.mem.setf(r + 0x2C + 4 * k, f);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(r + o, 0);
        }
        s3 += 1;
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x80, u64::from(D), 0x01E0_49D8);
    // The entry's s4 (saved by segment_spawns_b's prologue at sp - 0x30:
    // its upper half is the missing-texture TEX0 of the drops): the
    // runner's (0), 4 after the strike's bands, the last bolt segment's
    // +0x2C (as `s2 + 0x5670`) after the fan, sp + 0xA4 after a converging
    // sprite.
    let mut s4 = 0u64;
    match fx.mem.u16(D) {
        0xDC => return 8,
        0x7D => {
            strike(fx, sp);
            s4 = 4;
        }
        0x64 => {
            if let Some(v) = fan(fx, sp) {
                s4 = v;
            }
        }
        2 => opening(fx, sp),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0x20).contains(&n) && converge(fx, sp) {
        s4 = u64::from(sp + 0xA4);
    }
    let n = fx.mem.u16(D);
    if (0x12..0x64).contains(&n) {
        node_spike(fx);
    }
    let n = fx.mem.u16(D);
    if (0x24..0x5C).contains(&n) {
        node_band(fx);
    }
    let n = fx.mem.u16(D);
    if (0x36..0x54).contains(&n) {
        let p = Puff {
            slot: 0x10,
            base: 20.0,
            size: 0x42A0_0000,
            jitter: Some((0x64, 0x32)),
            vel: (0x28, 0x14),
            ras: [0x01E0_5974, 0x01E0_597C, 0x01E0_59D4, 0x01E0_5A10, 0x01E0_5A58, 0x01E0_5A98, 0x01E0_5AD8, 0x01E0_5B18, 0x01E0_5B54, 0x01E0_5B90],
        };
        puff(fx, sp, &p);
    }
    let n = fx.mem.u16(D);
    if (0x9E..0xB4).contains(&n) {
        let p = Puff {
            slot: 0xC,
            base: 32.0,
            size: 0x4348_0000,
            jitter: None,
            vel: (0x50, 0x28),
            ras: [0x01E0_5C2C, 0x01E0_5C34, 0x01E0_5C8C, 0x01E0_5CC8, 0, 0, 0, 0x01E0_5D38, 0x01E0_5D74, 0x01E0_5DB0],
        };
        puff(fx, sp, &p);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_5E0C, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5E18, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves (as b229): the runner's s6 (0) and ra.
    fx.saved(sp - 0xF0, &[(0x70, 0), (0x80, 0x01E0_5E24)]);
    blib::band::bands_b(fx, 0x01E0_4170, BANDS, OT, 0x20);
    blib::spike::spikes(fx, 0x01E0_0E10, SPIKES, OT, 0x28);
    blib::star::flash_b(fx, 0x01E0_03A0, FLASH, OT, 0x10);
    blib::emit::segment_spawns_b(fx, 0x01E0_2380, SPAWNS, 8);
    // Its 0x80 frame's saves (s0 = D, s1 = the OT, ra) as b526.
    fx.saved(sp - 0x80, &[(0x10, u64::from(D)), (0x20, u64::from(OT)), (0x50, s4), (0x58, 0), (0x60, 0x01E0_5E64)]);
    blib::bolt::bolts_b(fx, 0x01E0_18F0, SEGS, OT, 0x3C);
    blib::beam::beads_at(fx, 0x01E0_26C0, 0x34, 0x3C, BEADS, OT, 0x14);
    blib::drop::drops(fx, SPRITES, OT, 0xC8);
    let n = fx.mem.u16(D);
    if (0x7D..0x9A).contains(&n) || n == 0x61 {
        6
    } else if n == 0x9A {
        2
    } else if n == 0x73 || n == 0x5A {
        1
    } else {
        0
    }
}
