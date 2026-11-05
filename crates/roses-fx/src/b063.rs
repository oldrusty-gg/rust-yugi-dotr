//! BMODULE b063 (card 214), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04DE0, data area a0 = 0x1ED0000) [F] (helper B6, 31
//! Oct 2026): b077's family (s3 = D, s0 = the OT). Node bursts (+0xA4,
//! motes +0xE8, nodes 0x43 / 0x3B) every update; on update 79 an arc pair
//! (+0xFB8, nodes 0x31 / 0x30); on 85 the fade (+0x1A9C) and 30 sprays
//! (+0x1AA4, `spray::sprays`, delays k / 4) falling at the target; on 87
//! six ribbons (+0x141C) widening about the target, a wide flash (+0x1338)
//! and a camera shake there. The stage light pulsed by 0x1031C0 (+0xA0).
//! Returns 1 on update 75, 2 on 82, 6 on 85..105, 8 on 185 (and at once
//! for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `arcs` 0x1E015F0, `flash`
//! 0x1E02870, `ribbon` 0x1E03420, `sprays` 0x1E03FB0, `fade` 0x1E048A0
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x29A4;

/// The entry 0x1E04DE0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x60);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x2AB0);
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
    for (at, node, clear) in [(0xCu32, 0x31, 1), (0x10, 0x30, 0), (0x14, 0x3B, 0), (0x18, 0x43, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): spec 0x1E05748 / 0x1E05758, motes +0xE8 /
    // +0x7E8.
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(D + 0xCC, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_5748);
    fx.mem.write_u32(D + 0xB0, 0x01E0_5758);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x20);
    fx.mem.write_u32(D + 0xA8, D + 0x7E8);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xD7, b);
    fx.mem.write_u16(D + 0xD8, 0);
    fx.mem.write_u32(D + 0xDC, 0x4334_0000);
    fx.mem.write_u32(D + 0xE4, 0x40E0_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xB3);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4F94, [Some(OT), Some(3), None]);
    0
}

/// Update 79 [F 0x1E04FF8]: the arc pair (+0xFB8), points +0x1018.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0xFB8;
    set_counter4(fx, g, 0x15, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x1018);
    fx.mem.write_u8(g + 0x30, 0x32);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0xC0), (0x4A, 0x60), (0x49, 0x60), (0x48, 0x10)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0xC + 2 * a, -1);
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        a += 1;
    }
}

/// Update 85 [F 0x1E050B4]: the fade and 30 sprays above the target (sp +
/// 0x50).
fn sprays_start(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0x1A9C, 0xC, 0, 0xFF);
    fx.mem.write_u8(D + 0x1AA0, 1);
    for k in 0..0x1Ei32 {
        let i = first_free(fx, D + 0x1AA4, 0x60, 0x14);
        if i < 0 {
            continue;
        }
        let r = D + 0x1AA4 + 0x60 * i as u32;
        set_counter4(fx, r, 0x20, (k >> 2) & 0xFF, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 2);
        fx.mem.write_u8(r + 8, 0x14);
        let x = fx.rand_from(OT, 0x01E0_5148);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u8(r + 0xA, 1);
        let x = fx.rand_from(OT, 0x01E0_5164);
        fx.mem.set8(r + 0xB, (x >> 4) & 7);
        fx.mem.write_u8(r + 0xC, 0);
        fx.mem.write_u8(r + 0xD, 2);
        fx.mem.write_u32(r + 0x10, BYTES);
        let x = fx.rand_from(OT, 0x01E0_5190) >> 4;
        let (_, h) = fx.div(x, 6);
        let v = ee::add(14.0, ee::div(ee::from_i32(h * 200 - 0x258), 100.0));
        fx.mem.setf(r + 0x14, v);
        fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
        fx.mem.write_u32(r + 0x20, 0x4270_0000);
        fx.mem.write_u32(r + 0x24, 0);
        for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
            fx.mem.write_u8(r + o, 0x60);
        }
        let v = ee::sub(2.0, ee::mul(f32::from_bits(0x3DB8_51EC), ee::from_i32(k)));
        fx.mem.setf(r + 0x4C, v);
        let x = fx.mem.read_u32(sp + 0x50);
        fx.mem.write_u32(r + 0x3C, x);
        let y = ee::sub(fx.mem.f(sp + 0x54), 20.0);
        fx.mem.setf(r + 0x40, y);
        let z = fx.mem.read_u32(sp + 0x58);
        fx.mem.write_u32(r + 0x44, z);
    }
}

/// Update 87 [F 0x1E05294]: six ribbons, the flash and the shake at the
/// target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    for k in 0..6i32 {
        let i = first_free(fx, D + 0x141C, 0x68, 0x10);
        if i < 0 {
            continue;
        }
        let r = D + 0x141C + 0x68 * i as u32;
        fx.mem.write_u8(r + 4, 4);
        set_counter4(fx, r, 0x24, 0, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(0x10);
        fx.mem.write_u8(r, c);
        for (o, v) in [(0x27u32, 0x60u8), (0x23, 0x28), (5, 0), (0xA, 1), (0xB, 2), (0xC, 4), (8, 0x50)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_5340);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0x50, r + 0x28);
        let y = ee::sub(fx.mem.f(r + 0x2C), 20.0);
        fx.mem.setf(r + 0x2C, y);
        let w = ee::add(20.0, ee::mul(5.0, ee::from_i32(k + 1)));
        fx.mem.setf(r + 0x1C, w);
        let f2 = ee::mul(f32::from_bits(0x3ECC_CCCD), ee::from_i32(k));
        fx.mem.setf(r + 0x14, ee::sub(2.5, f2));
        fx.mem.setf(r + 0x18, ee::add(f32::from_bits(0x3ECC_CCCD), f2));
    }
    let r = D + 0x1338;
    set_counter4(fx, r, 0x18, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x60), (9, 0x10), (8, 0x40)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0);
    fx.mem.write_u32(r + 0x10, 0x44AF_0000);
    copy_v4(fx, sp + 0x50, r + 0x1C);
    let y = ee::sub(fx.mem.f(r + 0x20), 20.0);
    fx.mem.setf(r + 0x20, y);
    let s = D + 0x40;
    set_counter4(fx, s, 0x18, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    for o in [0x14u32, 0x18, 0x1C] {
        fx.mem.write_u32(s + o, 0x4000_0000);
    }
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x50, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_4FBC);
    // The entry's s1 at the library calls: the parameter or the last
    // loop's counter.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0xB9 => return 8,
        0x57 => {
            strike(fx, sp);
            s1 = 6;
        }
        0x55 => {
            sprays_start(fx, sp);
            s1 = 0x1E;
        }
        0x4F => arcs_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_5550, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5560, [Some(u64::from(OT)), Some(s1)]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x20);
    blib::arc::arcs(fx, 0x01E0_15F0, D + 0xFB8, OT, 1);
    blib::star::flash(fx, 0x01E0_2870, D + 0x1338, OT, 3);
    blib::streak::ribbon(fx, 0x01E0_3420, D + 0x141C, OT, 0x10);
    blib::fade::fade(fx, D + 0x1A9C, OT);
    blib::spray::sprays(fx, 0x01E0_3FB0, D + 0x1AA4, OT, 0x28);
    let n = fx.mem.u16(D);
    if (0x55..0x6A).contains(&n) {
        6
    } else if n == 0x52 {
        2
    } else if n == 0x4B {
        1
    } else {
        0
    }
}
