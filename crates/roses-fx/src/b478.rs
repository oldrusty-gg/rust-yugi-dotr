//! BMODULE b478 (card 413), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05D30, data area a0 = 0x1ED0000) [F] (helper B3, 30
//! Oct 2026): b245's family. Node bursts (+0xA4, motes +0xE8 from the
//! attacker's nodes 0x3E / 0x45) every update; on update 91 an arc pair
//! (+0xAA8, nodes 0x21 / 0x22); on 98 at the target a short fade (+0x19A8),
//! a flash (+0x19B0), one wide ribbon (+0x1328), 200 bouncing sprites
//! (+0x19FC) and a camera shake (+0x40); the bead beams (+0xC48) are never
//! started. Returns 1 on update 88, 2 on 95, 6 on 98..126, 8 on 162 (and
//! at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `arcs` 0x1E015F0, `flash`
//! 0x1E02870, `beads` 0x1E03420, `ribbon` 0x1E041F0, `fade` 0x1E04D80,
//! `bounce` 0x1E052C0 [`crate::blib`].
use crate::b076::{copy_v4, rand_bytes, shake_saving};
use crate::b245::{bounce_spawn, Bounce};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E05D30 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x5020);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x3E, 0), (0x14, 0x45, 0), (0x18, 0x21, 0), (0x1C, 0x22, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): spec 0x1E06828 / 0x1E06830, motes +0xE8 /
    // +0x468.
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xCC, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_6828);
    fx.mem.write_u32(D + 0xB0, 0x01E0_6830);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x10);
    fx.mem.write_u32(D + 0xA8, D + 0x468);
    fx.mem.write_u8(D + 0xD5, 0x28);
    fx.mem.write_u8(D + 0xD6, 0);
    fx.mem.set16(D + 0xD8, 0);
    fx.mem.write_u32(D + 0xDC, 0x42F0_0000);
    fx.mem.write_u32(D + 0xE0, 0x433E_0000);
    fx.mem.write_u32(D + 0xE4, 0x40C0_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x9C);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's), s2: the caller's.
    rand_bytes(fx, D + 0x4F1C, 0x100, 0x01E0_5EFC, [Some(OT), Some(3), None]);
    0
}

/// Update 91 [F 0x1E05F54]: the arc pair (+0xAA8), points +0xB08.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0xAA8;
    set_counter4(fx, g, 0x10, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0xB08);
    fx.mem.write_u8(g + 0x30, 0x14);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F8C_CCCD);
    fx.mem.write_u8(g + 0x34, 0);
    fx.mem.set16(g + 0x36, 0);
    fx.mem.write_u32(g + 0x40, 0x4120_0000);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0xC0), (0x4A, 0x20), (0x49, 0xC0), (0x48, 0x10)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        fx.mem.set16(g + 0xC + 2 * a, -1);
        a += 1;
    }
}

/// Update 98 [F 0x1E06024]: at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
    set_counter4(fx, D + 0x19A8, 2, 0, 0xFF);
    fx.mem.write_u8(D + 0x19AC, 4);
    let f = D + 0x19B0;
    set_counter4(fx, f, 0x20, 0, 0xFF);
    fx.mem.write_u8(f + 5, 0);
    fx.mem.write_u8(f + 4, 0x81);
    fx.mem.write_u8(f + 7, 9);
    fx.mem.write_u32(f + 0x10, 0x447A_0000);
    fx.mem.write_u32(f + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x20), (8, 0x80), (0xF, 0xD0)] {
        fx.mem.write_u8(f + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(f + o, 0);
    }
    copy_v4(fx, at, f + 0x1C);
    // The ribbon (+0x1328; the loop runs once: k = 0, the wide one).
    let r = D + 0x1328;
    fx.mem.write_u8(r + 4, 4);
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    let b = fx.mem.read_u8(r + 4) | 1;
    fx.mem.write_u8(r + 4, b);
    fx.mem.write_u8(r + 0xA, 1);
    fx.mem.write_u8(r + 0xC, 3);
    fx.mem.write_u32(r + 0x14, 0x4000_0000);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    fx.mem.write_u8(r + 0x23, 0);
    fx.mem.write_u8(r + 0x27, 0x80);
    fx.mem.write_u8(r + 8, 0xA0);
    fx.mem.write_u32(r + 0x58, 0x40);
    fx.mem.write_u32(r + 0x5C, 0x80);
    fx.mem.write_u32(r + 0x60, 0x100);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 0xB, 2);
    let x = fx.rand_from(OT, 0x01E0_6258);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0x4F1C);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    copy_v4(fx, at, r + 0x28);
    let v = Bounce { count: 0x18, b8: 2, base: 100.0, vy: 0xBF80_0000, spread: (0xB4, 0x5A), s0: OT, ras: [0x01E0_62EC, 0x01E0_6354, 0x01E0_63E4, 0x01E0_6430, 0x01E0_648C] };
    bounce_spawn(fx, D + 0x19FC, at, &v);
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FC0_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, sp + 0x60);
    match fx.mem.u16(D) {
        0xA2 => return 8,
        0x62 => strike(fx, sp),
        0x5B => arcs_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_6578, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x10);
    blib::arc::arcs(fx, 0x01E0_15F0, D + 0xAA8, OT, 1);
    blib::star::flash(fx, 0x01E0_2870, D + 0x19B0, OT, 1);
    blib::beam::beads(fx, 0x01E0_3420, D + 0xC48, OT, 0x14);
    blib::streak::ribbon(fx, 0x01E0_41F0, D + 0x1328, OT, 0x10);
    blib::fade::fade(fx, D + 0x19A8, OT);
    blib::sprite::bounce(fx, D + 0x19FC, OT, 0xC8);
    let n = fx.mem.u16(D);
    if (0x62..0x7F).contains(&n) {
        6
    } else if n == 0x5F {
        2
    } else if n == 0x58 {
        1
    } else {
        0
    }
}
