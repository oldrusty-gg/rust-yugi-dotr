//! BMODULE b077 (card 159), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05970, data area a0 = 0x1ED0000) [F] (helper B3, 30
//! Oct 2026): b245's family with b076's bursts. Node bursts (+0xA0, motes
//! +0xE4, nodes 0x32 / 0x3A, updates 92..106) every update; on update 102
//! an arc pair (+0x1144, nodes 0x1A / 0x18); on 106 at the target a fade
//! (+0x4C90), two flashes (+0x14C4 + 0x4C k: a wide one and a narrower
//! tilted one), two ribbons (+0x4BBC + 0x68 k, turned -+0x4000), 200
//! bouncing sprites (+0x169C), four bars (+0x155C + 0x50 k at 45-degree
//! steps, the third 3000 long) and a camera shake (+0x40) on the target's
//! position; the stage light pulsed by 0x1031C0 (+0x4C8C: down on update
//! 2, up on 176); object ops from the record's table 0x1E06590. Returns 1
//! on update 96, 2 on 103, 8 on 182 (and at once for a parameter of -2 /
//! -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `arcs` 0x1E015F0, `ribbon`
//! 0x1E02870, `bars` 0x1E03400, `flash` 0x1E03E10, `bounce` 0x1E049C0,
//! `fade` 0x1E05430 [`crate::blib`].
use crate::b035::{light_ramp, object_ops};
use crate::b076::{copy_v4, rand_bytes, shake_saving};
use crate::b245::{bounce_spawn, Bounce};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const OPS: u32 = 0x01E0_6590;

/// The entry 0x1E05970 (a0 = D, a1 = param).
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
    fx.bzero(D, 0x4DA0);
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
    for (at, node, clear) in [(0xCu32, 0x1A, 1), (0x10, 0x18, 0), (0x14, 0x32, 0), (0x18, 0x3A, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA0): spec 0x1E06568 / 0x1E06570, motes +0xE4 /
    // +0x7E4.
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xC8, b);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(D + 0xC9, b);
    fx.mem.write_u32(D + 0xA8, 0x01E0_6568);
    fx.mem.write_u32(D + 0xAC, 0x01E0_6570);
    fx.mem.write_u32(D + 0xA0, D + 0xE4);
    fx.mem.write_u8(D + 0xD0, 0x20);
    fx.mem.write_u32(D + 0xA4, D + 0x7E4);
    fx.mem.write_u8(D + 0xD1, 0x3C);
    fx.mem.write_u8(D + 0xD2, 0);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xD3, b);
    fx.mem.set16(D + 0xD4, 0x5C);
    fx.mem.set16(D + 0xD6, 0x6A);
    fx.mem.write_u32(D + 0xD8, 0x4316_0000);
    fx.mem.write_u32(D + 0xDC, 0x4248_0000);
    fx.mem.write_u32(D + 0xE0, 0x4100_0000);
    fx.mem.set16(D + 0x4C8C, 2);
    fx.mem.set16(D + 0x4C8E, 0xB0);
    // The entry's s0 = D, s1 = the OT, s2 = 3 (the texture loop's).
    rand_bytes(fx, D + 0x4C98, 0x100, 0x01E0_5B38, [Some(D), Some(OT), Some(3)]);
    0
}

/// Update 102 [F 0x1E05B90]: the arc pair (+0x1144), points +0x11A4.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0x1144;
    set_counter4(fx, g, 0x12, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x11A4);
    fx.mem.write_u8(g + 0x30, 0x32);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    for (o, v) in [(0x47u32, 0xC0u8), (0x44, 0xC0), (0x45, 0xC0), (0x46, 0xC0), (0x4A, 0x80), (0x49, 0x10), (0x48, 0x10)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0xC + 2 * a, -1);
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        a += 1;
    }
}

/// Update 106 [F 0x1E05C4C]: at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x50;
    set_counter4(fx, D + 0x4C90, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0x4C94, 1);
    for k in 0..2u32 {
        let f = D + 0x4C * k + 0x14C4;
        set_counter4(fx, f, 0x20, 0, 0xFF);
        fx.mem.write_u8(f + 5, 0);
        fx.mem.write_u8(f + 4, 0x80);
        fx.mem.write_u8(f + 7, 9);
        if k != 0 {
            fx.mem.write_u32(f + 0x10, 0x4396_0000);
            fx.mem.write_u32(f + 0x14, 0x4248_0000);
            fx.mem.write_u32(f + 0x18, 0x420C_0000);
        } else {
            let b = fx.mem.read_u8(f + 4) | 1;
            fx.mem.write_u8(f + 4, b);
            fx.mem.write_u32(f + 0x10, 0x43FA_0000);
            fx.mem.write_u32(f + 0x18, 0);
            fx.mem.write_u32(f + 0x14, 0);
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xA, 0x80), (9, 0x40), (8, 0x20), (0xF, 0xC0)] {
            fx.mem.write_u8(f + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(f + o, 0);
        }
        copy_v4(fx, at, f + 0x1C);
        let r = D + 0x68 * k + 0x4BBC;
        fx.mem.write_u8(r + 4, 4);
        set_counter4(fx, r, 0x1E, 0, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(8);
        fx.mem.write_u8(r, c);
        fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
        fx.mem.write_u8(r + 0x27, 0x80);
        fx.mem.write_u8(r + 0x23, 0);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xA, 2);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 3);
        fx.mem.write_u8(r + 8, 0x4D);
        let x = fx.rand_from(D, 0x01E0_5DEC);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, D + 0x4C98);
        fx.mem.write_u32(r + 0x14, 0x4000_0000);
        fx.mem.write_u32(r + 0x18, 0x3F00_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
        fx.mem.set32(r + 0x50, if k != 0 { 0x4000 } else { -0x4000 });
    }
    let v = Bounce { count: 0x20, b8: 1, base: 120.0, vy: 0xBF4C_CCCD, spread: (0xE6, 0x73), s0: D, ras: [0x01E0_5EB8, 0x01E0_5F24, 0x01E0_5FC4, 0x01E0_6010, 0x01E0_606C] };
    bounce_spawn(fx, D + 0x169C, at, &v);
    for k in 0..4i32 {
        let c = D + 0x50 * k as u32 + 0x155C;
        set_counter4(fx, c, 0x1C, 0, 0xFF);
        fx.mem.write_u8(c + 5, 0);
        fx.mem.write_u8(c + 4, 0x80);
        fx.mem.write_u32(c + 0x10, if k == 2 { 0x453B_8000 } else { 0x44BB_8000 });
        fx.mem.write_u32(c + 0x14, 0x4248_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0x80), (9, 0x40), (8, 0x20)] {
            fx.mem.write_u8(c + o, v);
        }
        fx.mem.setf(c + 0x18, ee::mul(45.0, ee::from_i32(k)));
        fx.mem.write_u32(c + 0x1C, 0x40C0_0000);
        copy_v4(fx, at, c + 0x20);
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
    fx.unit_position(t, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, sp + 0x50);
    match fx.mem.u16(D) {
        0xB6 => return 8,
        0x6A => strike(fx, sp),
        0x66 => arcs_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_6250, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0x4C8C, n, 0x01E0_6260, [Some(u64::from(D)), Some(u64::from(OT))]);
    let n = fx.mem.u16(D);
    object_ops(fx, OPS, n);
    blib::fade::fade(fx, D + 0x4C90, OT);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA0, n);
    blib::mote::motes(fx, D + 0xE4, OT, 0x20);
    blib::arc::arcs(fx, 0x01E0_15F0, D + 0x1144, OT, 1);
    blib::star::flash(fx, 0x01E0_3E10, D + 0x14C4, OT, 2);
    blib::cross::bars(fx, 0x01E0_3400, D + 0x155C, OT, 4);
    blib::streak::ribbon(fx, 0x01E0_2870, D + 0x4BBC, OT, 2);
    blib::sprite::bounce(fx, D + 0x169C, OT, 0xC8);
    match fx.mem.u16(D) {
        0x67 => 2,
        0x60 => 1,
        _ => 0,
    }
}
