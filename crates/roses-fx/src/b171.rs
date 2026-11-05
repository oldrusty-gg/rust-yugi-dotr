//! BMODULE b171 (card 174), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E045B0, data area a0 = 0x1ED0000) [F] (helper B3, 30
//! Oct 2026): b076's family, struck twice. On updates 164 (k = 0) and 186
//! (k = 1), at the target's position: an arc (+0xA0 + 0x60 k, nodes from
//! +0xC / +0x10 + 8 k, points +0x160), two ribbons (+0x7B8 + 0x68 i, i = 2
//! k + j; turned +-0x2000 from the attacker's yaw +0x30, the second a half
//! turn round, mirrored for k = 1) each with a bead beam (+0x658 + 0x58 i)
//! on the same angle, four crossing bars (+0x518 + 0x50 j, the first 1200
//! wide, the others 500 at 45-degree steps from the first one's angle), a
//! flash (+0x480 + 0x4C k) and a camera shake (+0x40). Returns 1 on
//! updates 166 and 186, 6 on 173, 7 on 185, 2 on 193, 8 on 260 (and at
//! once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `arcs`
//! 0x1E00410, `beads` 0x1E01690, `flash` 0x1E02460, `ribbon` 0x1E03010,
//! `bars` 0x1E03BA0 [`crate::blib`]; 0x102330 / the shake residue as
//! [`crate::b076`].
use crate::b076::{copy_v4, rand_bytes, shake_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E045B0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xA60);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (at, node, clear) in [(0xCu32, 0x11, 1), (0x10, 0x12, 0), (0x14, 0x1D, 0), (0x18, 0x1E, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The entry's s0 = D, s1 = the OT, s2 = 1 (the texture loop's).
    rand_bytes(fx, D + 0x958, 0x100, 0x01E0_46F0, [Some(D), Some(OT), Some(1)]);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, sp + 0x70);
    match fx.mem.u16(D) {
        0x104 => return 8,
        0xBA | 0xA4 => {
            let k = u32::from(fx.mem.u16(D) != 0xA4);
            strike(fx, sp, k);
        }
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_4CD8, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::star::flash(fx, 0x01E0_2460, D + 0x480, OT, 2);
    blib::arc::arcs(fx, 0x01E0_0410, D + 0xA0, OT, 2);
    blib::beam::beads(fx, 0x01E0_1690, D + 0x658, OT, 4);
    blib::streak::ribbon(fx, 0x01E0_3010, D + 0x7B8, OT, 4);
    blib::cross::bars(fx, 0x01E0_3BA0, D + 0x518, OT, 4);
    match fx.mem.u16(D) {
        0xC1 => 2,
        0xBA => 1,
        0xB9 => 7,
        0xAD => 6,
        0xA6 => 1,
        _ => 0,
    }
}

/// Updates 164 / 186 [F 0x1E04748..0x1E04CB8]; the target's position at
/// sp + 0x70.
fn strike(fx: &mut Fx, sp: u32, k: u32) {
    let at = sp + 0x70;
    // The arc (+0xA0 + 0x60 k).
    let g = D + 0x60 * k + 0xA0;
    set_counter4(fx, g, 0x14, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x160);
    fx.mem.write_u8(g + 0x30, 0x32);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 8 * k + 0xC);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 8 * k + 0x10);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    for (o, v) in [(0x47u32, 0xC0u8), (0x44, 0xC0), (0x45, 0xC0), (0x46, 0xC0), (0x4A, 0x10), (0x49, 0x10), (0x48, 0x80)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        fx.mem.set16(g + 0xC + 2 * a, -1);
        a += 1;
    }
    // Two ribbons, each with a bead beam on its angle.
    for j in 0..2u32 {
        let i = j + 2 * k;
        let r = D + 0x68 * i + 0x7B8;
        fx.mem.write_u8(r + 4, 4);
        set_counter4(fx, r, 0x12, 0xA, 0xFF);
        fx.mem.write_u32(r + 0x1C, 0x41C8_0000);
        fx.mem.write_u8(r + 0x27, 0x80);
        fx.mem.write_u8(r + 0x23, 0x30);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 3);
        fx.mem.write_u8(r + 8, 0x3C);
        let x = fx.rand_from(D, 0x01E0_48CC);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, D + 0x958);
        fx.mem.write_u32(r + 0x14, 0x400C_CCCD);
        fx.mem.write_u32(r + 0x18, 0x3F00_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
        let mut angle: i32 = if fx.mem.i32(D + 0x30) == 0x8000 { -0x2000 } else { 0x2000 };
        if j != 0 {
            angle = angle.wrapping_add(0x8000);
        }
        if k != 0 {
            angle = angle.wrapping_neg();
        }
        fx.mem.set32(r + 0x50, angle);
        let b = D + 0x58 * i + 0x658;
        set_counter4(fx, b, 0x1C, 0xA, 0xFF);
        fx.mem.write_u32(b + 0x1C, 0x42A0_0000);
        fx.mem.write_u32(b + 0x14, 0x3F99_999A);
        fx.mem.write_u32(b + 0x18, 0x3ECC_CCCD);
        fx.mem.write_u8(b + 0xA, 0);
        fx.mem.write_u8(b + 0xC, 2);
        fx.mem.write_u8(b + 8, 0x50);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(b + o, 0x80);
        }
        fx.mem.write_u8(b + 4, 4);
        fx.mem.write_u8(b + 5, 0);
        let x = fx.rand_from(D, 0x01E0_4A28);
        fx.mem.set8(b + 9, x >> 4);
        fx.mem.write_u32(b + 0x10, D + 0x958);
        copy_v4(fx, at, b + 0x28);
        let w = fx.mem.read_u32(r + 0x50);
        fx.mem.write_u32(b + 0x50, w);
    }
    // Four bars (+0x518 + 0x50 j).
    for j in 0..4i32 {
        let c = D + 0x50 * j as u32 + 0x518;
        set_counter4(fx, c, 0x14, 0xA, 0xFF);
        fx.mem.write_u8(c + 5, 0);
        fx.mem.write_u8(c + 4, 0x80);
        if j == 0 {
            fx.mem.write_u32(c + 0x10, 0x4496_0000);
            fx.mem.write_u32(c + 0x18, if k != 0 { 0x42E6_0000 } else { 0x41C8_0000 });
        } else {
            fx.mem.write_u32(c + 0x10, 0x43FA_0000);
            let first = fx.mem.f(D + 0x530);
            fx.mem.setf(c + 0x18, ee::add(first, ee::mul(45.0, ee::from_i32(j))));
        }
        fx.mem.write_u32(c + 0x14, 0x4220_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x20), (0xA, 0x80), (9, 0x10), (8, 0x20)] {
            fx.mem.write_u8(c + o, v);
        }
        fx.mem.write_u32(c + 0x1C, 0x3F80_0000);
        copy_v4(fx, at, c + 0x20);
    }
    // The flash (+0x480 + 0x4C k).
    let f = D + 0x4C * k + 0x480;
    set_counter4(fx, f, 0x18, 0xA, 0xFF);
    fx.mem.write_u8(f + 5, 0);
    fx.mem.write_u8(f + 4, 0x81);
    fx.mem.write_u8(f + 7, 9);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x10), (8, 0x40)] {
        fx.mem.write_u8(f + o, v);
    }
    fx.mem.write_u32(f + 0x14, 0);
    fx.mem.write_u32(f + 0x10, 0x4416_0000);
    copy_v4(fx, at, f + 0x1C);
    // The camera shake (+0x40).
    let s = D + 0x40;
    set_counter4(fx, s, 0x18, 0xA, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}
