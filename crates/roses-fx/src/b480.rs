//! BMODULE b480 (card 251), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E062A0, data area a0 = 0x1ED0000) [F] (helper B4, 31
//! Oct 2026): b076's "a0 = D" family (s3 = D, s0 = the OT). Node bursts
//! (+0xA4, motes +0xE8 from the attacker's nodes 0x17 / 0x1E) every
//! update; on update 79 an arc (+0xAA8, nodes 0x32 / 0x30); on 88 at the
//! target a fade (+0x1750), three shrinking sheets (+0xC48 + 0x3C k,
//! `sheet::sheets`, stacked 2 apart, turned 0x2000 about z, mirrored by
//! the attacker's facing), a flash (+0x1758), 200 bouncing sprites
//! (+0x183C), a ribbon (+0x16E8) and a camera shake (+0x40). Returns 1 on
//! update 78, 2 on 85, 6 on 88..116, 8 on 200 (and at once for a
//! parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `arcs` 0x1E015F0, `flash`
//! 0x1E02870, `beads` 0x1E03420 (+0x1008, never started), `ribbon`
//! 0x1E041F0, `fade` 0x1E04D80, `bounce` 0x1E052C0, `sheets` 0x1E05D30
//! [`crate::blib`].
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::b245::{bounce_spawn_sized, Bounce};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E062A0 (a0 = D, a1 = param; s3 = D, s0 = the OT).
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
    fx.bzero(D, 0x4E60);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x17, 0), (0x14, 0x1E, 0), (0x18, 0x32, 0), (0x1C, 0x30, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): specs 0x1E06DA8 / 0x1E06DB8, motes +0xE8 /
    // +0x468, nodes from +0x10 / +0x14.
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xCC, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_6DA8);
    fx.mem.write_u32(D + 0xB0, 0x01E0_6DB8);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x10);
    fx.mem.write_u32(D + 0xA8, D + 0x468);
    fx.mem.write_u8(D + 0xD5, 0x28);
    fx.mem.write_u8(D + 0xD6, 0);
    fx.mem.set16(D + 0xD8, 0);
    fx.mem.write_u32(D + 0xDC, 0x42B4_0000);
    fx.mem.write_u32(D + 0xE0, 0x433E_0000);
    fx.mem.write_u32(D + 0xE4, 0x40E0_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xC2);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's), s2: the caller's.
    rand_bytes(fx, D + 0x4D5C, 0x100, 0x01E0_646C, [Some(OT), Some(3), None]);
    0
}

/// Update 79 [F 0x1E064C4]: the arc (+0xAA8, points +0xB08), nodes +0x18
/// / +0x1C.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0xAA8;
    set_counter4(fx, g, 0x14, 0, 0xFF);
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

/// b480's (0x1E06794).
const BOUNCE: Bounce = Bounce { count: 0x18, b8: 2, base: 60.0, vy: 0xBF4C_CCCD, spread: (0xB4, 0x5A), s0: OT, ras: [0x01E0_67BC, 0x01E0_6824, 0x01E0_68C4, 0x01E0_6910, 0x01E0_696C] };

/// Update 88 [F 0x1E06594]: at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x50;
    set_counter4(fx, D + 0x1750, 8, 0, 0xFF);
    fx.mem.write_u8(D + 0x1754, 1);
    for k in 0..3i32 {
        let r = D + 0xC48 + 0x3C * k as u32;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 1), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        let d = ee::add(-2.0, ee::mul(2.0, ee::from_i32(k)));
        let y = fx.mem.f(r + 0x30);
        fx.mem.setf(r + 0x30, ee::add(y, d));
        fx.mem.write_u32(r + 0x10, 0x3FE6_6666);
        fx.mem.write_u32(r + 0x14, 0x3FC0_0000);
        fx.mem.write_u32(r + 0x18, if k == 1 { 0x4040_0000 } else { 0x4000_0000 });
        fx.mem.set32(r + 0x24, 0x2000);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let v = fx.mem.i32(r + 0x24);
            fx.mem.set32(r + 0x24, v.wrapping_neg());
        }
    }
    let g = D + 0x1758;
    set_counter4(fx, g, 0x1C, 0, 0xFF);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u32(g + 0x10, 0x4461_0000);
    fx.mem.write_u32(g + 0x18, 0);
    fx.mem.write_u32(g + 0x14, 0);
    for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xA, 0x80), (9, 0x10), (8, 0x10), (0xF, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    copy_v4(fx, at, g + 0x1C);
    bounce_spawn_sized(fx, D + 0x183C, at, &BOUNCE, (0xA, 0x3E8));
    let r = D + 0x16E8;
    fx.mem.write_u8(r + 4, 5);
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    let b = fx.mem.read_u8(r + 4) | 2;
    fx.mem.write_u8(r + 4, b);
    fx.mem.write_u32(r + 0x14, 0x3FB3_3333);
    fx.mem.write_u32(r + 0x1C, 0x42A0_0000);
    fx.mem.write_u8(r + 0x23, 0x80);
    fx.mem.write_u8(r + 0x27, 0x80);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 0xA, 2);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 3);
    fx.mem.write_u8(r + 8, 0x50);
    let x = fx.rand_from(OT, 0x01E0_6A38);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0x4D5C);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    copy_v4(fx, at, r + 0x28);
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
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_6AF4);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_6494);
    match fx.mem.u16(D) {
        0xC8 => return 8,
        0x58 => strike(fx, sp),
        0x4F => arcs_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_6B14, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x10);
    blib::arc::arcs(fx, 0x01E0_15F0, D + 0xAA8, OT, 1);
    blib::star::flash(fx, 0x01E0_2870, D + 0x1758, OT, 3);
    blib::beam::beads(fx, 0x01E0_3420, D + 0x1008, OT, 0x14);
    blib::streak::ribbon(fx, 0x01E0_41F0, D + 0x16E8, OT, 1);
    blib::fade::fade(fx, D + 0x1750, OT);
    blib::sprite::bounce(fx, D + 0x183C, OT, 0xC8);
    blib::sheet::sheets(fx, 0x01E0_5D30, D + 0xC48, OT, 0x10);
    let n = fx.mem.u16(D);
    if (0x58..0x75).contains(&n) {
        6
    } else if n == 0x55 {
        2
    } else if n == 0x4E {
        1
    } else {
        0
    }
}
