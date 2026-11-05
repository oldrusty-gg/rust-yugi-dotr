//! BMODULE b375 (card 190), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04F60, data area a0 = 0x1ED0000) [F] (helper B6, 31
//! Oct 2026): b468's family (s3 = D, s0 = the OT). Node bursts (+0xA0,
//! motes +0xE4, nodes 0x45 / 0x4D / 0x31) every update; on update 50 a
//! slow cone (+0x4864, `cone::cones`) and 80 bouncing sprites (+0x12D0)
//! on node 0x31; on 144 the fade (+0x485C), six cones (delays k) at the
//! target turned to face the attacker, a wide flash (+0x1144), a ribbon
//! (+0x47F0), 200 bouncing sprites and a camera shake on the target's
//! position. The crossing bars (+0x1190) are drawn but never started.
//! The stage light pulsed by 0x1031C0 (+0x4858). Returns 1 on update 134,
//! 2 on 141, 8 on 201 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `ribbon` 0x1E015F0, `bars`
//! 0x1E02180, `flash` 0x1E02B90, `bounce` 0x1E03740, `fade` 0x1E041B0,
//! `cones` 0x1E046F0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b245::{bounce_spawn, Bounce};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x4A14;
const CONES: u32 = D + 0x4864;
const BOUNCE: u32 = D + 0x12D0;

/// The entry 0x1E04F60 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x4B20);
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
    for (at, node, clear) in [(0xCu32, 0x45, 1), (0x10, 0x4D, 0), (0x14, 0x31, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA0): specs 0x1E05BC0 / 0x1E05BD0 / 0x1E05BE0,
    // motes +0xE4 / +0x7E4, three nodes.
    for (o, src) in [(0xC8u32, 0xCu32), (0xC9, 0x10), (0xCA, 0x14)] {
        let b = fx.mem.read_u8(D + src);
        fx.mem.write_u8(D + o, b);
    }
    fx.mem.write_u32(D + 0xA8, 0x01E0_5BC0);
    fx.mem.write_u32(D + 0xAC, 0x01E0_5BD0);
    fx.mem.write_u32(D + 0xB0, 0x01E0_5BE0);
    fx.mem.write_u32(D + 0xA0, D + 0xE4);
    fx.mem.write_u8(D + 0xD0, 0x20);
    fx.mem.write_u32(D + 0xA4, D + 0x7E4);
    fx.mem.write_u8(D + 0xD1, 0x3C);
    fx.mem.write_u8(D + 0xD2, 0);
    fx.mem.write_u16(D + 0xD4, 0);
    fx.mem.write_u32(D + 0xD8, 0x4316_0000);
    fx.mem.write_u32(D + 0xDC, 0);
    fx.mem.write_u32(D + 0xE0, 0x40C0_0000);
    fx.mem.set16(D + 0x4858, 2);
    fx.mem.set16(D + 0x485A, 0xC3);
    // The entry's s0 = the OT, s1 = 2 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_510C, [Some(OT), Some(2), None]);
    0
}

/// (rand >> 4) % m - h, over 100.
fn spread(fx: &mut Fx, ra: u32, m: i32, h: i32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, r) = fx.div(x, m);
    ee::div(ee::from_i32(r - h), 100.0)
}

/// Update 50 [F 0x1E05164]: a slow cone and 80 sprites on node +0x14 (sp
/// + 0x60).
fn rise(fx: &mut Fx, sp: u32, s1: u64) {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x14);
    node_position_saving(fx, h, n, sp + 0x60, u64::from(OT), Some(s1), 0x01E0_5178);
    let c = CONES;
    set_counter4(fx, c, 0x10, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, 2)] {
        fx.mem.write_u8(c + o, v);
    }
    fx.mem.write_u32(c + 0xC, 0x3F80_0000);
    fx.mem.write_u32(c + 0x10, 0x3DCC_CCCD);
    fx.mem.write_u32(c + 0x14, 0x3E99_999A);
    fx.mem.write_u32(c + 0x18, 0);
    fx.mem.write_u32(c + 0x1C, 0x3CF5_C28F);
    for (o, v) in [(0x23u32, 0x80u8), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x26, 0xC0), (0x25, 0), (0x24, 0), (0x27, 0)] {
        fx.mem.write_u8(c + o, v);
    }
    copy_v4(fx, sp + 0x60, c + 0x28);
    for _ in 0..0x50 {
        let i = first_free(fx, BOUNCE, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(OT, 0x01E0_528C) >> 4) & 7;
        let b = BOUNCE + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, delay, 0xFF);
        for (o, x) in [(4u32, 0u8), (8, 1), (0xB, 2), (0xA, 0), (9, 3), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(b + o, x);
        }
        fx.mem.write_u32(b + 0x10, 0x41A0_0000);
        fx.mem.write_u32(b + 0x14, 0);
        fx.mem.write_u32(b + 0x18, 0xBD4C_CCCD);
        fx.mem.write_u32(b + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        let v = spread(fx, 0x01E0_532C, 0x50, 0x28);
        fx.mem.setf(b + 0x2C, v);
        let v = spread(fx, 0x01E0_5378, 0x14, 0xA);
        fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), v));
        let v = spread(fx, 0x01E0_53D4, 0x50, 0x28);
        fx.mem.setf(b + 0x34, v);
    }
}

/// Update 144 [F 0x1E05434]: everything at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x50;
    set_counter4(fx, D + 0x485C, 0xA, 0, 0xFF);
    fx.mem.write_u8(D + 0x4860, 1);
    for k in 0..6u32 {
        let c = CONES + 0x48 * k;
        set_counter4(fx, c, 0x18, (k & 0xFF) as i32, 0xFF);
        for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, 2), (0xA, 0x20), (9, 7)] {
            fx.mem.write_u8(c + o, v);
        }
        fx.mem.write_u32(c + 0xC, 0x4000_0000);
        fx.mem.write_u32(c + 0x10, 0x3E99_999A);
        fx.mem.write_u32(c + 0x14, 0x3FC0_0000);
        fx.mem.write_u32(c + 0x18, 0);
        fx.mem.write_u32(c + 0x1C, 0x3DF5_C28F);
        for (o, v) in [(0x22u32, 0xA0u8), (0x21, 0xA0), (0x20, 0xA0), (0x23, 0), (0x26, 0xC0), (0x25, 0x20), (0x24, 0x10), (0x27, 0x80)] {
            fx.mem.write_u8(c + o, v);
        }
        copy_v4(fx, at, c + 0x28);
        let turn = if fx.mem.read_u32(D + 0x30) == 0x8000 { 0x4000 } else { -0x4000 };
        fx.mem.set32(c + 0x38, turn);
    }
    let r = D + 0x1144;
    set_counter4(fx, r, 0x20, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u32(r + 0x10, 0x4448_0000);
    fx.mem.write_u32(r + 0x18, 0);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x10), (8, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, at, r + 0x1C);
    let r = D + 0x47F0;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4020_0000);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_5668);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, at, r + 0x28);
    fx.mem.write_u32(r + 0x58, 0x40);
    fx.mem.write_u32(r + 0x5C, 0x80);
    fx.mem.write_u32(r + 0x60, 0x100);
    let v = Bounce { count: 0x20, b8: 1, base: 120.0, vy: 0xBF4C_CCCD, spread: (0xE6, 0x73), s0: OT, ras: [0x01E0_5714, 0x01E0_5780, 0x01E0_5820, 0x01E0_586C, 0x01E0_58C8] };
    bounce_spawn(fx, BOUNCE, at, &v);
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
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_5980);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_5134);
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0xC9 => return 8,
        0x90 => {
            strike(fx, sp);
            s1 = 0xC8;
        }
        0x32 => {
            rise(fx, sp, s1);
            s1 = 0x50;
        }
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_59A0, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0x4858, n, 0x01E0_59B0, [Some(u64::from(OT)), Some(s1)]);
    blib::fade::fade(fx, D + 0x485C, OT);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA0, n);
    blib::mote::motes(fx, D + 0xE4, OT, 0x20);
    blib::star::flash(fx, 0x01E0_2B90, D + 0x1144, OT, 1);
    blib::cross::bars(fx, 0x01E0_2180, D + 0x1190, OT, 4);
    blib::streak::ribbon(fx, 0x01E0_15F0, D + 0x47F0, OT, 1);
    blib::sprite::bounce(fx, BOUNCE, OT, 0xC8);
    blib::cone::cones(fx, 0x01E0_46F0, CONES, OT, 6);
    match fx.mem.u16(D) {
        0x8D => 2,
        0x86 => 1,
        _ => 0,
    }
}
