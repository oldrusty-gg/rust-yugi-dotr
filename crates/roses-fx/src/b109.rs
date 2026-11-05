//! BMODULE b109 (card 162), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05EE0, data area a0 = 0x1ED0000) [F] (helper B4, 31
//! Oct 2026): b076's "a0 = D" family. Node bursts (+0xA0, motes +0xE4 from
//! the attacker's nodes 0xC / 0x3C) every update; on update 99 an arc
//! (+0xC34, nodes 0x23 / 0x1C); on 101 at the target a fade (+0x15EC),
//! three shrinking sheets (+0x122C + 0x3C k, `sheet::sheets`, stacked 1
//! apart, turned 0x4400 about z, mirrored by the attacker's facing), two
//! flashes (+0x1054), 200 bouncing sprites (+0x16C4, b245's
//! `bounce_spawn`), four bars (+0x10EC) and a camera shake (+0x40). Returns
//! 1 on update 92, 2 on 99, 6 on 102..126, 8 on 200 (and at once for a
//! parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `flash` 0x1E015F0, `bars`
//! 0x1E021A0, `arcs` 0x1E02BB0, `sheets` 0x1E03E30, `fade` 0x1E043A0,
//! `ribbon` 0x1E048E0 (+0x15F4, never started), `bounce` 0x1E05470
//! [`crate::blib`].
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::b245::{bounce_spawn, Bounce};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E05EE0 (a0 = D, a1 = param; s0 = D, s1 = the OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x60);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x4CF0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(4, 2);
    for k in 0..3 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0xC, 0), (0x14, 0x3C, 0), (0x18, 0x23, 0), (0x1C, 0x1C, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA0): specs 0x1E06A70 / 0x1E06A80, motes +0xE4 /
    // +0x464, nodes from +0x10 / +0x14.
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xC8, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xC9, b);
    fx.mem.write_u32(D + 0xA8, 0x01E0_6A70);
    fx.mem.write_u32(D + 0xAC, 0x01E0_6A80);
    fx.mem.write_u32(D + 0xA0, D + 0xE4);
    fx.mem.write_u8(D + 0xD0, 0x10);
    fx.mem.write_u32(D + 0xA4, D + 0x464);
    fx.mem.write_u8(D + 0xD1, 0x32);
    fx.mem.write_u8(D + 0xD2, 0);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xD3, b);
    fx.mem.set16(D + 0xD4, 0x32);
    fx.mem.set16(D + 0xD6, 0x4A);
    fx.mem.write_u32(D + 0xD8, 0x4316_0000);
    fx.mem.write_u32(D + 0xDC, 0x42F0_0000);
    fx.mem.write_u32(D + 0xE0, 0x40C0_0000);
    // The entry's s0 = D, s1 = the OT, s2 = 3 (the texture loop's).
    rand_bytes(fx, D + 0x4BE4, 0x100, 0x01E0_60BC, [Some(D), Some(OT), Some(3)]);
    0
}

/// Update 99 [F 0x1E06114]: the arc (+0xC34, points +0xC94), nodes +0x18
/// / +0x1C.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0xC34;
    set_counter4(fx, g, 0x1E, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0xC94);
    fx.mem.write_u8(g + 0x30, 0x3C);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    for (o, v) in [(0x44u32, 0xC0u8), (0x45, 0xC0), (0x46, 0xC0), (0x47, 0x80), (0x4A, 0x80), (0x49, 0x40), (0x48, 0x20)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        fx.mem.set16(g + 0xC + 2 * a, -1);
        a += 1;
    }
}

/// b109's (0x1E06438).
const BOUNCE: Bounce = Bounce { count: 0x20, b8: 2, base: 100.0, vy: 0xBF66_6666, spread: (0xE6, 0x73), s0: D, ras: [0x01E0_6460, 0x01E0_64C8, 0x01E0_655C, 0x01E0_65A8, 0x01E0_6604] };

/// Update 101 [F 0x1E061D4]: at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x50;
    set_counter4(fx, D + 0x15EC, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0x15F0, 1);
    for k in 0..3i32 {
        let r = D + 0x122C + 0x3C * k as u32;
        set_counter4(fx, r, 0x14, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 1), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        let y = fx.mem.f(r + 0x30);
        fx.mem.setf(r + 0x30, ee::add(y, ee::add(-1.0, ee::from_i32(k))));
        fx.mem.write_u32(r + 0x10, 0x4000_0000);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        fx.mem.write_u32(r + 0x18, if k == 1 { 0x4080_0000 } else { 0x4040_0000 });
        fx.mem.set32(r + 0x24, 0x4400);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let v = fx.mem.i32(r + 0x24);
            fx.mem.set32(r + 0x24, v.wrapping_neg());
        }
    }
    for k in 0..2u32 {
        let r = D + 0x1054 + 0x4C * k;
        set_counter4(fx, r, 0x1C, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x80);
        fx.mem.write_u8(r + 7, 9);
        if k != 0 {
            fx.mem.write_u32(r + 0x10, 0x4396_0000);
            fx.mem.write_u32(r + 0x14, 0x4248_0000);
            fx.mem.write_u32(r + 0x18, 0x420C_0000);
        } else {
            let b = fx.mem.read_u8(r + 4) | 1;
            fx.mem.write_u8(r + 4, b);
            fx.mem.write_u32(r + 0x10, 0x43FA_0000);
            fx.mem.write_u32(r + 0x18, 0);
            fx.mem.write_u32(r + 0x14, 0);
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xA, 0x80), (9, 0x10), (8, 0x10), (0xF, 0xC0)] {
            fx.mem.write_u8(r + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(r + o, 0);
        }
        copy_v4(fx, at, r + 0x1C);
    }
    bounce_spawn(fx, D + 0x16C4, at, &BOUNCE);
    for k in 0..4i32 {
        let r = D + 0x10EC + 0x50 * k as u32;
        set_counter4(fx, r, 0x1C, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x80);
        fx.mem.write_u32(r + 0x10, if k == 2 { 0x44FA_0000 } else { 0x4496_0000 });
        fx.mem.write_u32(r + 0x14, 0x4248_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0x80), (9, 0x40), (8, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.setf(r + 0x18, ee::mul(f32::from_bits(0x4234_0000), ee::from_i32(k)));
        fx.mem.write_u32(r + 0x1C, 0x4000_0000);
        copy_v4(fx, at, r + 0x20);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(D), 0x01E0_67C0);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(D), 0x01E0_60E4);
    match fx.mem.u16(D) {
        0xC8 => return 8,
        0x65 => strike(fx, sp),
        0x63 => arcs_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_67E0, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA0, n);
    blib::mote::motes(fx, D + 0xE4, OT, 0x10);
    blib::star::flash(fx, 0x01E0_15F0, D + 0x1054, OT, 2);
    blib::cross::bars(fx, 0x01E0_21A0, D + 0x10EC, OT, 4);
    blib::arc::arcs(fx, 0x01E0_2BB0, D + 0xC34, OT, 1);
    blib::sheet::sheets(fx, 0x01E0_3E30, D + 0x122C, OT, 0x10);
    blib::fade::fade(fx, D + 0x15EC, OT);
    blib::sprite::bounce(fx, D + 0x16C4, OT, 0xC8);
    blib::streak::ribbon(fx, 0x01E0_48E0, D + 0x15F4, OT, 2);
    let n = fx.mem.u16(D);
    if (0x66..0x7F).contains(&n) {
        6
    } else if n == 0x63 {
        2
    } else if n == 0x5C {
        1
    } else {
        0
    }
}
