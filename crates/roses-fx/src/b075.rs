//! BMODULE b075 (card 452), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E052D0, data area a0 = 0x1ED0000) [F] (helper B4, 31
//! Oct 2026): b076's "a0 = D" family (s1 = D, s0 = the OT). Node bursts
//! (+0xA0, motes +0xE4 from the attacker's nodes 0xC / 0x33) every update;
//! on update 112 an arc pair (+0x1068, nodes 0x1B / 0x17) and a bead-trail
//! emitter (+0x26C8, `emit::bead_trails`, riding node 0x1B, beads pool of
//! 64 at +0x10C8); on 116 at the target a camera shake (+0x40), a flash
//! (+0xFB4) and a ribbon (+0x1000). Returns 1 on update 107, 2 on 114, 4
//! on 176, 8 on 190 (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `beads` 0x1E015F0, `arcs`
//! 0x1E023C0, `flash` 0x1E03640, `bead_trails` 0x1E041F0, `ribbon`
//! 0x1E04740 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x2A74;

/// The entry 0x1E052D0 (a0 = D, a1 = param; s1 = D, s0 = the OT, s2 =
/// the parameter).
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
    fx.bzero(D, 0x2B80);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x17, 0), (0x14, 0xC, 0), (0x18, 0x33, 0), (0x1C, 0x1B, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA0): specs 0x1E05B28 / 0x1E05B38, motes +0xE4 /
    // +0x7E4, nodes from +0x14 / +0x18.
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xC8, b);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(D + 0xC9, b);
    fx.mem.write_u32(D + 0xA8, 0x01E0_5B28);
    fx.mem.write_u32(D + 0xAC, 0x01E0_5B38);
    fx.mem.write_u32(D + 0xA0, D + 0xE4);
    fx.mem.write_u8(D + 0xD0, 0x20);
    fx.mem.write_u32(D + 0xA4, D + 0x7E4);
    fx.mem.write_u8(D + 0xD1, 0x32);
    fx.mem.write_u8(D + 0xD2, 0);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xD3, b);
    fx.mem.set16(D + 0xD4, 0x6A);
    fx.mem.set16(D + 0xD6, 0x7E);
    fx.mem.write_u32(D + 0xD8, 0x42F0_0000);
    fx.mem.write_u32(D + 0xDC, 0x42F0_0000);
    fx.mem.write_u32(D + 0xE0, 0x40A0_0000);
    // The entry's s0 = 2 (the texture loop's), s1 = D, s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5494, [Some(2), Some(D), Some(param as u32)]);
    0
}

/// Update 112 [F 0x1E054EC]: the arc pair (+0x1068, points +0x2754, nodes
/// +0x1C / +0x10) and the emitter (+0x26C8) at node +0x1C.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0x1068;
    set_counter4(fx, g, 0x10, 0, 0xFF);
    fx.mem.write_u8(g + 4, 0xD);
    fx.mem.write_u32(g + 8, D + 0x2754);
    fx.mem.write_u8(g + 0x30, 0x32);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    fx.mem.write_u32(g + 0x3C, 0x40C0_0000);
    fx.mem.write_u32(g + 0x40, 0x4170_0000);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0x80), (0x4A, 0x80), (0x49, 0), (0x48, 0x40), (0x4E, 0x80), (0x4D, 0), (0x4C, 0x10), (0x4F, 0x80)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 0x54, 0x3F80_0000);
    let mut k = 0u32;
    while k < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0xC + 2 * k, -1);
        fx.mem.set16(g + 0x1E + 2 * k, -1);
        k += 1;
    }
    let e = D + 0x26C8;
    set_counter4(fx, e, 0xE, 0, 0xFF);
    fx.mem.write_u8(e + 4, 0);
    fx.mem.write_u8(e + 5, 1);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(e + 6, b);
    fx.mem.write_u32(e + 8, D + 0x10C8);
    for (o, v) in [(0x10u32, 0x40u8), (0x12, 0xA), (0x11, 0x18), (0x13, 0)] {
        fx.mem.write_u8(e + o, v);
    }
    fx.mem.write_u32(e + 0x18, 0x3E4C_CCCD);
    fx.mem.write_u32(e + 0x1C, 0x3E99_999A);
    fx.mem.write_u32(e + 0x20, 0x41C8_0000);
    fx.mem.write_u32(e + 0xC, BYTES);
    for (o, v) in [(0x2Au32, 0x80u8), (0x29, 0x10), (0x28, 0x40), (0x2B, 0xA0)] {
        fx.mem.write_u8(e + o, v);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x1C);
    node_position_saving(fx, h, n, e + 0x3C, u64::from(OT), Some(u64::from(D)), 0x01E0_566C);
    copy_v4(fx, e + 0x3C, e + 0x4C);
}

/// Update 116 [F 0x1E05694]: at the target (sp + 0x40).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x40;
    let s = D + 0x40;
    set_counter4(fx, s, 0x18, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
    let g = D + 0xFB4;
    set_counter4(fx, g, 0x18, 0, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x43FA_0000);
    fx.mem.write_u32(g + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0xA0), (9, 0x20), (8, 0x80), (0xF, 0xD0)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    copy_v4(fx, at, g + 0x1C);
    let r = D + 0x1000;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    fx.mem.write_u8(r + 0x27, 0xC0);
    fx.mem.write_u8(r + 0x23, 0);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 0xA, 1);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 3);
    fx.mem.write_u8(r + 8, 0x3C);
    let x = fx.rand_from(OT, 0x01E0_5810);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    copy_v4(fx, at, r + 0x28);
    for o in [0x60u32, 0x5C, 0x58] {
        fx.mem.set32(r + o, 0x234);
    }
}

/// `motes` called from the entry, leaving its frame's saved registers
/// (0x1D0: f20 +0 (0: the runner's), the caller's s0..s3 +0x10..+0x40):
/// the bead trail's curve samples (sp + 0x104 + 0x10 k of its 0x2E0
/// frame) keep these words as their w, copied into the beads' origin w
/// [run b075; F 0x1E00410].
pub(crate) fn motes_saving(fx: &mut Fx, recs: u32, ot: u32, n: i32, s: [u64; 4]) -> i32 {
    let r = blib::mote::motes(fx, recs, ot, n);
    let f = fx.sp - 0x1D0;
    fx.mem.write_u32(f, 0);
    for (k, v) in s.iter().enumerate() {
        fx.saved(f + 0x10 + 0x10 * k as u32, &[(0, *v)]);
    }
    r
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x40, u64::from(OT), 0x01E0_54BC);
    match fx.mem.u16(D) {
        0xBE => return 8,
        0x74 => strike(fx, sp),
        0x70 => arcs_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_5898, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA0, n);
    motes_saving(fx, D + 0xE4, OT, 0x20, [u64::from(OT), u64::from(D), i64::from(param) as u64, 0]);
    blib::emit::bead_trails(fx, 0x01E0_41F0, D + 0x26C8, 1);
    blib::beam::beads(fx, 0x01E0_15F0, D + 0x10C8, OT, 0x40);
    blib::star::flash(fx, 0x01E0_3640, D + 0xFB4, OT, 1);
    blib::arc::arcs(fx, 0x01E0_23C0, D + 0x1068, OT, 1);
    blib::streak::ribbon(fx, 0x01E0_4740, D + 0x1000, OT, 1);
    match fx.mem.u16(D) {
        0xB0 => 4,
        0x72 => 2,
        0x6B => 1,
        _ => 0,
    }
}
