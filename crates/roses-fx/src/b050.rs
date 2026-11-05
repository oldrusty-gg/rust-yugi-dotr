//! BMODULE b050 (card 446), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04D60, data area a0 = 0x1ED0000) [F] (helper B7, 31
//! Oct 2026): the "a0 = D" family with s0 = D (rand's and the library
//! frames' saved s0), s1 the parameter, s2 = the OT. Node bursts (+0xA0,
//! motes +0xE4) every update; two strikes: arcs (+0xA74 / +0xAD4,
//! `arc::arcs`) between the attacker's nodes 0x1B / 0x16 on update 122 and
//! 0x31 / 0x2C on 140, then on 127 / 145 at the target a ray burst
//! (+0x133C, `ray::rays`), a camera shake, a flash (+0xF34) and five bead
//! beams (+0xFCC). Returns 1 on updates 118 / 136, 6 on 125, 7 on 135, 2
//! on 143, 8 on 216 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `flash` 0x1E015F0, `beads`
//! 0x1E021A0, `rays` 0x1E02F70, `arcs` 0x1E03AE0 [`crate::blib`].
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x1424;

/// The entry 0x1E04D60 (a0 = D, a1 = param; s0 = D, s1 = param, s2 = the
/// OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x1530);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0xC, 0), (0x14, 0x40, 0), (0x18, 0x1B, 0), (0x1C, 0x16, 0), (0x20, 0x31, 0), (0x24, 0x2C, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA0): specs 0x1E05738 / 0x1E05748, motes +0xE4 /
    // +0x2A4.
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xC8, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xC9, b);
    fx.mem.write_u32(D + 0xA8, 0x01E0_5738);
    fx.mem.write_u32(D + 0xAC, 0x01E0_5748);
    fx.mem.write_u32(D + 0xA0, D + 0xE4);
    fx.mem.write_u8(D + 0xD0, 8);
    fx.mem.write_u32(D + 0xA4, D + 0x2A4);
    fx.mem.write_u8(D + 0xD1, 0x32);
    fx.mem.write_u8(D + 0xD2, 0);
    fx.mem.write_u16(D + 0xD4, 0);
    fx.mem.write_u32(D + 0xD8, 0x4320_0000);
    fx.mem.write_u32(D + 0xDC, 0);
    fx.mem.write_u32(D + 0xE0, 0x40C0_0000);
    // The entry's s0 = D, s1 = the parameter, s2 = the OT.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4F44, [Some(D), Some(param as u32), Some(OT)]);
    0
}

/// Updates 122 / 140 [F 0x1E04FB4]: arc `k` (+0xA74 + 0x60 k) between the
/// nodes +0x18 / +0x1C (+ 8 k), points at +0xB34.
fn arc_start(fx: &mut Fx, k: u32) {
    let g = D + 0xA74 + 0x60 * k;
    if k == 0 {
        set_counter4(fx, g, 0xC, 0, 0xFF);
        fx.mem.write_u32(g + 0x38, 0x3FCC_CCCD);
    } else {
        set_counter4(fx, g, 0x10, 0, 0xFF);
        fx.mem.write_u32(g + 0x38, 0x3FA6_6666);
    }
    fx.mem.write_u8(g + 4, 0xD);
    fx.mem.write_u32(g + 8, D + 0xB34);
    fx.mem.write_u8(g + 0x30, 0x40);
    fx.mem.write_u8(g + 0x31, 8);
    let v = fx.mem.read_u8(D + 0x18 + 8 * k);
    fx.mem.write_u8(g + 0x32, v);
    let v = fx.mem.read_u8(D + 0x1C + 8 * k);
    fx.mem.write_u8(g + 0x33, v);
    fx.mem.write_u32(g + 0x3C, 0x40C0_0000);
    fx.mem.write_u32(g + 0x40, 0x41A0_0000);
    for (o, v) in [(0x46u32, 0x80u8), (0x45, 0x10), (0x44, 0x60), (0x47, 0x80), (0x4A, 0x80), (0x49, 0x10), (0x48, 0x60), (0x4E, 0x80), (0x4D, 0x10), (0x4C, 0x60), (0x4F, 0xF0)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 0x54, 0x3F4C_CCCD);
    let mut j = 0u32;
    while j < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * j, -1);
        fx.mem.set16(g + 0xC + 2 * j, -1);
        j += 1;
    }
}

/// Updates 127 / 145 [F 0x1E05104]: at the target (sp + 0x60) ray burst
/// `k` (+0x133C + 0x74 k), the shake, flash `k` (+0xF34 + 0x4C k) and five
/// bead beams (+0xFCC; the slot from 0x101C30 taken as a byte).
fn strike(fx: &mut Fx, sp: u32, k: u32) {
    let at = sp + 0x60;
    let r = D + 0x133C + 0x74 * k;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 8, 0x32);
    let x = fx.rand_from(D, 0x01E0_516C);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x18, 0x4270_0000);
    fx.mem.write_u32(r + 0xC, BYTES);
    fx.mem.write_u32(r + 0x10, 0x4040_0000);
    fx.mem.write_u32(r + 0x14, 0x4020_0000);
    for (o, v) in [(0x1Eu32, 0x80u8), (0x1D, 0x10), (0x1C, 0x60), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, at, r + 0x24);
    fx.mem.set32(r + 0x54, 0x4000);
    if fx.mem.read_u32(D + 0x30) == 0x8000 {
        let v = fx.mem.i32(r + 0x54).wrapping_neg();
        fx.mem.set32(r + 0x54, v);
    }
    fx.mem.set32(r + 0x6C, 0x200);
    let s = D + 0x40;
    set_counter4(fx, s, 0x10, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
    let r = D + 0xF34 + 0x4C * k;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x43FA_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x10), (8, 0x60), (0xF, 0xB0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, at, r + 0x1C);
    for j in 0..5i32 {
        let i = (first_free(fx, D + 0xFCC, 0x58, 0xA) & 0xFF) as u32;
        let count = (((fx.rand_from(D, 0x01E0_5378) >> 4) & 3) + 0x14) & 0xFF;
        let delay = (fx.rand_from(D, 0x01E0_5390) >> 4) & 7;
        let b = D + 0xFCC + 0x58 * i;
        set_counter4(fx, b, count, delay, 0xFF);
        fx.mem.write_u8(b + 4, 1);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 8, 0x1E);
        let x = fx.rand_from(D, 0x01E0_53E8);
        fx.mem.set8(b + 9, x >> 4);
        fx.mem.write_u8(b + 0xA, 0);
        fx.mem.setf(b + 0x1C, ee::add(10.0, ee::from_i32(j << 2)));
        fx.mem.write_u32(b + 0x20, 0);
        fx.mem.write_u32(b + 0x10, BYTES);
        let v = ee::sub(1.5, ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(j)));
        fx.mem.setf(b + 0x14, v);
        for (o, v) in [(0x26u32, 0x80u8), (0x25, 0x10), (0x24, 0x60), (0x27, 0xF0)] {
            fx.mem.write_u8(b + o, v);
        }
        copy_v4(fx, at, b + 0x28);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_4F6C);
    match fx.mem.u16(D) {
        0xD8 => return 8,
        0x7F => strike(fx, sp, 0),
        0x91 => strike(fx, sp, 1),
        0x7A => arc_start(fx, 0),
        0x8C => arc_start(fx, 1),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_54D4, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA0, n);
    blib::mote::motes(fx, D + 0xE4, OT, 8);
    blib::beam::beads(fx, 0x01E0_21A0, D + 0xFCC, OT, 0xA);
    blib::ray::rays(fx, 0x01E0_2F70, D + 0x133C, OT, 2);
    blib::star::flash(fx, 0x01E0_15F0, D + 0xF34, OT, 2);
    blib::arc::arcs(fx, 0x01E0_3AE0, D + 0xA74, OT, 2);
    match fx.mem.u16(D) {
        0x8F => 2,
        0x88 | 0x76 => 1,
        0x87 => 7,
        0x7D => 6,
        _ => 0,
    }
}
