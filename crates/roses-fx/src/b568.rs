//! BMODULE b568 (card 449), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03960, data area a0 = 0x1ED0000) [F] (helper B6, 31
//! Oct 2026): b063's family (s3 = D, s0 = the OT). On update 70 at the
//! target the fade (+0x50A0), a wide flash (+0xA4), five sheets in a row
//! (+0x5110, `sheet::sheets`), 180 bouncing sprites (+0xF0, floor at node
//! 0xC's height), 30 falling sprays (+0x523C, `spray::sprays`) and a camera
//! shake there; on updates 71, 130 and 151 a ring of 36 small sprites
//! thrown out from node 0x39 in 10-degree steps. The stage light pulsed
//! by 0x1031C0 (+0xA0). Returns 1 on update 60, 6 on 70..98, 2 on 99, 8 on
//! 200 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `bounce` 0x1E00FC0, `fade` 0x1E01A30, `sheets` 0x1E01F70,
//! `ribbon` 0x1E024E0, `sprays` 0x1E03070 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x613C;
const BOUNCE: u32 = D + 0xF0;

/// The entry 0x1E03960 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x6240);
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
    for (at, node, clear) in [(0xCu32, 0x39, 1), (0x10, 0xC, 0), (0x14, 0x38, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xC2);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_3A9C, [Some(OT), Some(3), None]);
    0
}

/// (rand >> 4) % m from return address `ra` (the remainder by `div`).
fn rem(fx: &mut Fx, ra: u32, m: i32) -> i32 {
    let x = fx.rand_from(OT, ra) >> 4;
    fx.div(x, m).1
}

/// Update 70 [F 0x1E03B0C]: everything at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
    set_counter4(fx, D + 0x50A0, 8, 0, 0xFF);
    fx.mem.write_u8(D + 0x50A4, 1);
    let r = D + 0xA4;
    set_counter4(fx, r, 0x20, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x10), (9, 0x20), (8, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0);
    fx.mem.write_u32(r + 0x10, 0x4448_0000);
    copy_v4(fx, at, r + 0x1C);
    for k in 0..5i32 {
        let r = D + 0x5110 + 0x3C * k as u32;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 2), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        let dx = ee::add(f32::from_bits(0xC066_6666), ee::mul(f32::from_bits(0x3FE6_6666), ee::from_i32(k)));
        let x = ee::add(fx.mem.f(r + 0x2C), dx);
        fx.mem.setf(r + 0x2C, x);
        fx.mem.write_u32(r + 0x10, 0x3F99_999A);
        fx.mem.write_u32(r + 0x14, 0x3F33_3333);
        fx.mem.write_u32(r + 0x18, if k == 2 { 0x4019_999A } else { 0x3FE6_6666 });
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x10);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(OT), Some(5), 0x01E0_3CE0);
    let spread = |fx: &mut Fx, ra: u32| ee::div(ee::from_i32(rem(fx, ra, 0xD2) - 0x69), 100.0);
    for _ in 0..0xB4 {
        let i = first_free(fx, BOUNCE, 0x44, 0x12C);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(OT, 0x01E0_3D14) >> 4) & 0xF;
        let b = BOUNCE + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, delay, 0xFF);
        for (o, x) in [(4u32, 0u8), (8, 1), (0xB, 2), (0xA, 0), (9, 3), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x60)] {
            fx.mem.write_u8(b + o, x);
        }
        let h = rem(fx, 0x01E0_3D84, 0x14);
        let v = ee::add(80.0, ee::div(ee::from_i32(h * 200 - 0x7D0), 100.0));
        fx.mem.setf(b + 0x10, v);
        fx.mem.write_u32(b + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(b + 0x18, 0xBD4C_CCCD);
        let y = fx.mem.read_u32(sp + 0x84);
        fx.mem.write_u32(b + 0x1C, y);
        let x = fx.mem.read_u32(at);
        fx.mem.write_u32(b + 0x20, x);
        let y = ee::sub(fx.mem.f(at + 4), 10.0);
        fx.mem.setf(b + 0x24, y);
        let z = fx.mem.read_u32(at + 8);
        fx.mem.write_u32(b + 0x28, z);
        let v = spread(fx, 0x01E0_3E38);
        fx.mem.setf(b + 0x2C, v);
        let v = spread(fx, 0x01E0_3E84);
        fx.mem.setf(b + 0x30, ee::add(0.5, v));
        let v = spread(fx, 0x01E0_3EDC);
        fx.mem.setf(b + 0x34, v);
    }
    for k in 0..0x1Ei32 {
        let i = first_free(fx, D + 0x523C, 0x60, 0x14);
        if i < 0 {
            continue;
        }
        let r = D + 0x523C + 0x60 * i as u32;
        set_counter4(fx, r, 0x28, (k >> 2) & 0xFF, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 2);
        fx.mem.write_u8(r + 8, 0x1E);
        let x = fx.rand_from(OT, 0x01E0_3FAC);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u8(r + 0xA, 1);
        let x = fx.rand_from(OT, 0x01E0_3FC8);
        fx.mem.set8(r + 0xB, (x >> 4) & 7);
        fx.mem.write_u8(r + 0xC, 0);
        fx.mem.write_u8(r + 0xD, 2);
        fx.mem.write_u32(r + 0x10, BYTES);
        let x = (fx.rand_from(OT, 0x01E0_3FF4) >> 4).wrapping_rem(4);
        let v = ee::add(10.0, ee::div(ee::from_i32(x * 200 - 0x190), 100.0));
        fx.mem.setf(r + 0x14, v);
        fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
        fx.mem.write_u32(r + 0x20, 0x42A0_0000);
        fx.mem.write_u32(r + 0x24, 0);
        for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
            fx.mem.write_u8(r + o, 0x60);
        }
        let v = ee::sub(2.0, ee::mul(f32::from_bits(0x3DB8_51EC), ee::from_i32(k)));
        fx.mem.setf(r + 0x4C, v);
        let x = fx.mem.read_u32(at);
        fx.mem.write_u32(r + 0x3C, x);
        let y = ee::sub(fx.mem.f(at + 4), 20.0);
        fx.mem.setf(r + 0x40, y);
        let z = fx.mem.read_u32(at + 8);
        fx.mem.write_u32(r + 0x44, z);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x24, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F8C_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F8C_CCCD);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_4154);
}

/// Updates 71 / 130 / 151 [F 0x1E0415C]: 36 sprites thrown out in a ring
/// from node +0xC (sp + 0x70), floor at node +0x10's height (sp + 0x80).
fn ring(fx: &mut Fx, sp: u32, s1: u64) {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x70, u64::from(OT), Some(s1), 0x01E0_4170);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x10);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(OT), Some(s1), 0x01E0_4184);
    for k in 0..0x24i32 {
        let i = first_free(fx, BOUNCE, 0x44, 0x12C);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(OT, 0x01E0_41B8) >> 4) & 7;
        let b = BOUNCE + 0x44 * i as u32;
        set_counter4(fx, b, 0x28, delay, 0xFF);
        let m = (fx.rand_from(OT, 0x01E0_41EC) >> 4) & 3;
        fx.mem.set8(b + 4, m << 4);
        fx.mem.write_u8(b + 8, 0);
        fx.mem.write_u8(b + 0xB, 1);
        fx.mem.write_u8(b + 0xA, 5);
        let c = (fx.rand_from(OT, 0x01E0_421C) >> 4) & 3;
        fx.mem.set8(b + 9, c);
        for (o, x) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x50)] {
            fx.mem.write_u8(b + o, x);
        }
        let h = rem(fx, 0x01E0_424C, 3);
        fx.mem.setf(b + 0x10, ee::add(10.0, ee::div(ee::from_i32(h * 200 - 0x12C), 100.0)));
        let h = rem(fx, 0x01E0_42B8, 3);
        fx.mem.setf(b + 0x14, ee::add(15.0, ee::div(ee::from_i32(h * 200 - 0x12C), 100.0)));
        fx.mem.write_u32(b + 0x18, 0xBCA3_D70A);
        let y = fx.mem.read_u32(sp + 0x84);
        fx.mem.write_u32(b + 0x1C, y);
        let x = fx.mem.read_u32(sp + 0x70);
        fx.mem.write_u32(b + 0x20, x);
        fx.mem.write_u32(b + 0x24, y);
        let z = fx.mem.read_u32(sp + 0x78);
        fx.mem.write_u32(b + 0x28, z);
        let a = ee::mul(10.0, ee::from_i32(k));
        let r = ee::add(0.5, ee::div(ee::from_i32(rem(fx, 0x01E0_436C, 0x50) - 0x28), 100.0));
        let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), a), 180.0);
        let c = libm_cosf(fx, a);
        fx.mem.setf(b + 0x2C, ee::mul(r, c));
        let s = fx.libm_sinf(a);
        fx.mem.setf(b + 0x34, ee::mul(r, s));
        fx.mem.write_u32(b + 0x30, 0x3ECC_CCCD);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(OT), 0x01E0_3AC4);
    // The entry's s1 at the library calls: the parameter or the last
    // loop's counter.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0xC8 => return 8,
        0x97 | 0x82 | 0x47 => {
            ring(fx, sp, s1);
            s1 = 0x24;
        }
        0x46 => {
            strike(fx, sp);
            s1 = 0x1E;
        }
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_4454, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4464, [Some(u64::from(OT)), Some(s1)]);
    blib::star::flash(fx, 0x01E0_0410, D + 0xA4, OT, 1);
    blib::sprite::bounce(fx, BOUNCE, OT, 0x12C);
    blib::streak::ribbon(fx, 0x01E0_24E0, D + 0x50A8, OT, 1);
    blib::fade::fade(fx, D + 0x50A0, OT);
    blib::sheet::sheets(fx, 0x01E0_1F70, D + 0x5110, OT, 5);
    blib::spray::sprays(fx, 0x01E0_3070, D + 0x523C, OT, 0x28);
    let n = fx.mem.u16(D);
    if (0x46..0x63).contains(&n) {
        6
    } else if n == 0x63 {
        2
    } else if n == 0x3C {
        1
    } else {
        0
    }
}
