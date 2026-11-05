//! BMODULE b181 (card 138), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E038B0, data area a0 = 0x1ED0000) [F] (helper A14,
//! 31 Oct 2026): the "a0 = D" shake / halo / spikes_b / beads / fade_c
//! family (s2 = D, s1 = the parameter, s0 = the OT). On update 39 a
//! bead-trail emitter (+0x15C, riding the attacker's node 0x40, beads pool
//! of 100 at +0x1D8); on 51 at the target a fade (+0xEC), a wide halo
//! (+0xA0), a spike (+0xF4) and a camera shake (+0x40); object ops 0x1F83B0
//! from the record's table 0x1E03FA0. Returns 1 on update 41, 6 on 51..71,
//! 2 on 72, 8 on 105 (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` (ac7dccf5ae36) 0x1E00080,
//! `spikes_b` 0x1E00440, `halo` 0x1E01170, `beads_at` (4c6a2da07857)
//! 0x1E01EC0, `bead_trails_c` (88444d92ecd4) 0x1E02DD0, `fade_c`
//! 0x1E03340 [`crate::blib`].
use crate::b035::object_ops;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x2438;
/// The object-op table (rodata, 0x14-byte entries up to a 0 start).
const OPS: u32 = 0x01E0_3FA0;

/// The entry 0x1E038B0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x50);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x50);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x2540);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x40, 1);
    fx.mem.set32(D + 0xC, r);
    // The entry's s0 = 1 (the texture loop's), s1 = the parameter, s2 = D.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_3998, [Some(1), Some(param as u32), Some(D)]);
    0
}

/// Update 39 [F 0x1E039F0]: the bead-trail emitter (+0x15C) at node +0xC.
fn emitter_start(fx: &mut Fx, param: i32) {
    let e = D + 0x15C;
    set_counter4(fx, e, 0x1F, 0, 0xFF);
    fx.mem.write_u8(e + 4, 0);
    fx.mem.write_u8(e + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(e + 6, b);
    fx.mem.write_u32(e + 8, D + 0x1D8);
    for (o, v) in [(0x10u32, 0x64u8), (0x12, 4), (0x11, 0x12), (0x13, 0), (0x16, 2), (0x14, 4)] {
        fx.mem.write_u8(e + o, v);
    }
    fx.mem.write_u32(e + 0x18, 0x3ECC_CCCD);
    fx.mem.write_u32(e + 0x1C, 0x3F00_0000);
    fx.mem.write_u32(e + 0x20, 0x4234_0000);
    fx.mem.write_u32(e + 0x24, 0xBFC0_0000);
    fx.mem.write_u32(e + 0xC, BYTES);
    for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
        fx.mem.write_u8(e + o, 0x80);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, e + 0x3C, u64::from(OT), Some(i64::from(param) as u64), 0x01E0_3AA4);
    copy_v4(fx, e + 0x3C, e + 0x4C);
}

/// Update 51 [F 0x1E03ACC]: at the target (sp + 0x40).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x40;
    set_counter4(fx, D + 0xEC, 4, 0, 0xFF);
    fx.mem.write_u8(D + 0xF0, 1);
    let g = D + 0xA0;
    set_counter4(fx, g, 0x1C, 0, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x4416_0000);
    fx.mem.write_u32(g + 0x14, 0x41A0_0000);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x20), (8, 0x80)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    copy_v4(fx, at, g + 0x1C);
    fx.mem.write_u32(g + 0x18, 0x41A0_0000);
    let r = D + 0xF4;
    set_counter4(fx, r, 0x18, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4033_3333);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 0xA, 0);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 3);
    fx.mem.write_u8(r + 8, 0xA0);
    let x = fx.rand_from(OT, 0x01E0_3BE8);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    fx.mem.write_u8(r + 0x23, 0x20);
    fx.mem.write_u8(r + 0x27, 0x80);
    copy_v4(fx, at, r + 0x28);
    fx.mem.set32(r + 0x58, 0x40);
    fx.mem.set32(r + 0x5C, 0x80);
    fx.mem.set32(r + 0x60, 0x100);
    let s = D + 0x40;
    set_counter4(fx, s, 0x18, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4270_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_3CCC);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x40, u64::from(OT), 0x01E0_39C0);
    match fx.mem.u16(D) {
        0x69 => return 8,
        0x33 => strike(fx, sp),
        0x27 => emitter_start(fx, param),
        _ => {}
    }
    blib::shake::camera_shake(fx, D + 0x40);
    blib::emit::bead_trails_c(fx, 0x01E0_2DD0, D + 0x15C, 1);
    blib::beam::beads_at(fx, 0x01E0_1EC0, 0x40, 0x44, D + 0x1D8, OT, 0x64);
    blib::halo::halo(fx, 0x01E0_1170, D + 0xA0, OT, 1);
    blib::fade::fade_c(fx, D + 0xEC, OT);
    blib::spike::spikes_b(fx, 0x01E0_0440, D + 0xF4, OT, 1);
    // spikes_b's frame (0x220) keeps the caller's s4..s6 (0) and its ra at
    // sp + 0x50..0x80: the next update's bead samples' stale w words
    // (bead_trails_c's sp + 0xE0 + 0x10 k) [run: 126 data-area bytes].
    fx.saved(sp - 0x220, &[(0x50, 0), (0x60, 0), (0x70, 0), (0x80, 0x01E0_3D4C)]);
    let n = fx.mem.u16(D);
    object_ops(fx, OPS, n);
    let n = fx.mem.u16(D);
    if (0x33..0x48).contains(&n) {
        6
    } else if n == 0x48 {
        2
    } else if n == 0x29 {
        1
    } else {
        0
    }
}
