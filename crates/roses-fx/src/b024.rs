//! BMODULE b024 (card 295), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04D60, data area a0 = 0x1ED0000) [F] (helper B7, 31
//! Oct 2026): the "a0 = D" family with s4 = D, s1 = the OT, s0 the
//! parameter (rand's and the library frames' saved s0). Node bursts
//! (+0xA0, motes +0xE4) every update; on update 56 an arc (+0xFB4,
//! `arc::arcs`) between the attacker's nodes 0x2A / 0x26; on 60 at the
//! target a ray burst (+0x13C0, `ray::rays`: 50 rays spinning), a flash
//! (+0x1214) and a camera shake; on 61 four bead beams (+0x1260). Returns 1
//! on update 52, 2 on 59, 8 on 116 (and at once for -2 / -3).
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
const BYTES: u32 = D + 0x1434;

/// The entry 0x1E04D60 (a0 = D, a1 = param; s4 = D, s1 = the OT, s0 =
/// param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x1540);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x14, 0), (0x14, 0x43, 0), (0x18, 0x2A, 0), (0x1C, 0x26, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA0): specs 0x1E05648 / 0x1E05638, motes +0xE4 /
    // +0x7E4.
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xC8, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xC9, b);
    fx.mem.write_u32(D + 0xA8, 0x01E0_5648);
    fx.mem.write_u32(D + 0xAC, 0x01E0_5638);
    fx.mem.write_u32(D + 0xA0, D + 0xE4);
    fx.mem.write_u8(D + 0xD0, 0x20);
    fx.mem.write_u32(D + 0xA4, D + 0x7E4);
    fx.mem.write_u8(D + 0xD1, 0x32);
    fx.mem.write_u8(D + 0xD2, 0);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xD3, b);
    fx.mem.write_u16(D + 0xD4, 0x11);
    fx.mem.write_u16(D + 0xD6, 0x1B);
    fx.mem.write_u32(D + 0xD8, 0x4320_0000);
    fx.mem.write_u32(D + 0xDC, 0x4302_0000);
    fx.mem.write_u32(D + 0xE0, 0x40C0_0000);
    // The entry's s0 = the parameter, s1 = the OT, s2 = 1 (the texture
    // loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4F2C, [Some(param as u32), Some(OT), Some(1)]);
    0
}

/// rand() with the caller's s0 = `s0` (64 bits) from return address `ra`.
fn rand64(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand()
}

/// Update 56 [F 0x1E04F90]: the arc (+0xFB4) from node +0x18 to node
/// +0x1C, points at +0x1014.
fn arc_start(fx: &mut Fx) {
    let g = D + 0xFB4;
    set_counter4(fx, g, 0xC, 0, 0xFF);
    fx.mem.write_u8(g + 4, 0xD);
    fx.mem.write_u32(g + 8, D + 0x1014);
    fx.mem.write_u8(g + 0x30, 0x20);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F99_999A);
    fx.mem.write_u32(g + 0x3C, 0x40C0_0000);
    fx.mem.write_u32(g + 0x40, 0x41A0_0000);
    for (k, v) in [(0x45u32, 0x20u8), (0x46, 0x20), (0x44, 0x80), (0x47, 0x80), (0x49, 0x20), (0x4A, 0x20), (0x48, 0x80), (0x4D, 0x20), (0x4E, 0x20), (0x4C, 0x80), (0x4F, 0xF0)] {
        fx.mem.write_u8(g + k, v);
    }
    fx.mem.write_u32(g + 0x54, 0x3F4C_CCCD);
    let mut k = 0u32;
    while k < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * k, -1);
        fx.mem.set16(g + 0xC + 2 * k, -1);
        k += 1;
    }
}

/// Update 60 [F 0x1E05074]: at the target (sp + 0x60) the ray burst, the
/// shake and a flash.
fn strike(fx: &mut Fx, sp: u32, s0: u64) {
    let at = sp + 0x60;
    let r = D + 0x13C0;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 8, 0x32);
    let x = rand64(fx, s0, 0x01E0_50A4);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x18, 0x4270_0000);
    fx.mem.write_u32(r + 0xC, BYTES);
    fx.mem.write_u32(r + 0x10, 0x4020_0000);
    fx.mem.write_u32(r + 0x14, 0x4020_0000);
    for (o, v) in [(0x1Du32, 0u8), (0x1E, 0), (0x1C, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x80)] {
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
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x4080_0000);
    copy_v4(fx, at, D + 0x60);
    let r = D + 0x1214;
    set_counter4(fx, r, 0x18, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x4416_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (9, 0x20), (0xA, 0x20), (8, 0x80), (0xF, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, at, r + 0x1C);
}

/// Update 61 [F 0x1E05240]: four bead beams (+0x1260, 0x58 each) at the
/// target; the slot from 0x101C30 is taken as a byte (a full pool gives
/// slot 0xFF, as the original). Returns the entry's s0 after (the last
/// count).
fn beams(fx: &mut Fx, sp: u32, mut s0: u64) -> u64 {
    for k in 0..4i32 {
        let i = (first_free(fx, D + 0x1260, 0x58, 4) & 0xFF) as u32;
        let count = (((rand64(fx, s0, 0x01E0_5274) >> 4) & 3) + 0x14) & 0xFF;
        s0 = count as u64;
        let delay = (rand64(fx, s0, 0x01E0_528C) >> 4) & 7;
        let r = D + 0x1260 + 0x58 * i;
        set_counter4(fx, r, count, delay, 0xFF);
        fx.mem.write_u8(r + 4, 1);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x1E);
        let x = rand64(fx, s0, 0x01E0_52E4);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.setf(r + 0x1C, ee::add(10.0, ee::from_i32(k << 2)));
        fx.mem.write_u32(r + 0x20, 0);
        fx.mem.write_u32(r + 0x10, BYTES);
        let v = ee::sub(1.5, ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(k)));
        fx.mem.setf(r + 0x14, v);
        for (o, v) in [(0x26u32, 0x20u8), (0x25, 0x20), (0x24, 0xC0), (0x27, 0xF0)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x60, r + 0x28);
    }
    s0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut s0 = i64::from(param) as u64;
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, s0, 0x01E0_4F54);
    match fx.mem.u16(D) {
        0x74 => return 8,
        0x3D => s0 = beams(fx, sp, s0),
        0x3C => strike(fx, sp, s0),
        0x38 => arc_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, s0, 0x01E0_53CC, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA0, n);
    blib::mote::motes(fx, D + 0xE4, OT, 0x20);
    blib::beam::beads(fx, 0x01E0_21A0, D + 0x1260, OT, 4);
    blib::ray::rays(fx, 0x01E0_2F70, D + 0x13C0, OT, 1);
    blib::star::flash(fx, 0x01E0_15F0, D + 0x1214, OT, 1);
    blib::arc::arcs(fx, 0x01E0_3AE0, D + 0xFB4, OT, 1);
    match fx.mem.u16(D) {
        0x3B => 2,
        0x34 => 1,
        _ => 0,
    }
}
