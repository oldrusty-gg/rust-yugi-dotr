//! BMODULE b230 (card 179), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E050A0, data area a0 = 0x1ED0000) [F] (helper B4, 31
//! Oct 2026): b772's library (s0 = D, s1 = the OT). From update 71 four
//! bouncing sprites a update (pool of 250 at +0x1284) thrown out in a
//! turning cross from 5 below the attacker's node 5; on update 223 a
//! bead-trail emitter (+0xE64, riding node 0x59, beads pool of 40 at
//! +0xA4); on 224 at the target a fade (+0x1230), a flash (+0x1238), three
//! staggered ribbons (+0x1090) and a camera shake (+0x40); object ops
//! 0x1F83B0 from the record's table 0x1E05C30; the stage light pulsed by
//! 0x1031C0 (+0xA0: down on update 2, up on 327). Returns 1 on update 214,
//! 2 on 221, 6 on 224..252, 8 on 333 (and at once for a parameter of -2 /
//! -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `arcs`
//! 0x1E00410 (+0xEF0, never started), `flash` 0x1E01690, `ribbon`
//! 0x1E02240, `fade` 0x1E02DD0, `bounce` 0x1E03310, `beads` 0x1E03D80,
//! `bead_trails` 0x1E04B50 [`crate::blib`].
use crate::b035::{light_ramp, object_ops};
use crate::b772::flash_saving;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x54EC;
const POOL: u32 = D + 0x1284;
/// The object-op table (rodata, 0x14-byte entries up to a 0 start).
const OPS: u32 = 0x01E0_5C30;

/// The entry 0x1E050A0 (a0 = D, a1 = param; s0 = D, s1 = the OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x55F0);
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
    for (at, node, clear) in [(0xCu32, 5, 1), (0x10, 0x58, 0), (0x14, 0x5A, 0), (0x18, 0x59, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x147);
    // The entry's s0 = D, s1 = the OT, s2 = 2 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_51F4, [Some(D), Some(OT), Some(2)]);
    0
}

/// (rand >> 4) % m - off, over 100 (rand called from `ra`).
fn spread(fx: &mut Fx, ra: u32, m: i32, off: i32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h - off), 100.0)
}

/// Update 223 [F 0x1E0524C]: the emitter (+0xE64) at node +0x18.
fn emitter_start(fx: &mut Fx) {
    let e = D + 0xE64;
    set_counter4(fx, e, 6, 0, 0xFF);
    fx.mem.write_u8(e + 4, 2);
    fx.mem.write_u8(e + 5, 1);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(e + 6, b);
    fx.mem.write_u32(e + 8, D + 0xA4);
    for (o, v) in [(0x10u32, 0x28u8), (0x12, 0x14), (0x11, 0x28), (0x13, 1), (0x16, 2), (0x14, 4)] {
        fx.mem.write_u8(e + o, v);
    }
    fx.mem.write_u32(e + 0x18, 0x4000_0000);
    fx.mem.write_u32(e + 0x1C, 0x3ECC_CCCD);
    fx.mem.write_u32(e + 0x20, 0x42F0_0000);
    fx.mem.write_u32(e + 0xC, BYTES);
    for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
        fx.mem.write_u8(e + o, 0x80);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x18);
    node_position_saving(fx, h, n, e + 0x3C, u64::from(D), Some(u64::from(OT)), 0x01E0_52F4);
    copy_v4(fx, e + 0x3C, e + 0x4C);
}

/// Update 224 [F 0x1E0531C]: at the target (sp + 0x70).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x70;
    set_counter4(fx, D + 0x1230, 0xC, 0, 0xFF);
    fx.mem.write_u8(D + 0x1234, 1);
    let g = D + 0x1238;
    set_counter4(fx, g, 0x20, 0, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x80);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x42C8_0000);
    fx.mem.write_u32(g + 0x14, 0x4248_0000);
    for (o, v) in [(0xCu32, 0xF0u8), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x40), (9, 0x10), (8, 0x80), (0xF, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    copy_v4(fx, at, g + 0x1C);
    fx.mem.write_u32(g + 0x18, 0x4270_0000);
    for k in 0..3i32 {
        let r = D + 0x1090 + 0x68 * k as u32;
        set_counter4(fx, r, 0x1C, (k * 2) & 0xFF, 0xFF);
        fx.mem.write_u8(r + 4, 4);
        let b = fx.mem.read_u8(r + 4) | 3;
        fx.mem.write_u8(r + 4, b);
        fx.mem.write_u8(r + 0xA, 1);
        fx.mem.write_u8(r + 0xC, 4);
        fx.mem.write_u32(r + 0x14, 0x4020_0000);
        fx.mem.write_u32(r + 0x1C, 0x42F0_0000);
        fx.mem.write_u8(r + 0x23, 0x80);
        fx.mem.write_u8(r + 0x27, 0x10);
        fx.mem.write_u8(r + 8, 0x78);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xB, 2);
        let x = fx.rand_from(D, 0x01E0_5478);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}

/// From update 71 [F 0x1E05564]: four sprites thrown out at 360 - (6 n +
/// 90 k) degrees from 5 below node +0xC (sp + 0x80).
fn spin(fx: &mut Fx, sp: u32) {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(D), Some(u64::from(OT)), 0x01E0_5588);
    let y = ee::sub(fx.mem.f(sp + 0x84), 5.0);
    fx.mem.setf(sp + 0x84, y);
    for k in 0..4i32 {
        let i = first_free(fx, POOL, 0x44, 0xFA);
        if i < 0 {
            continue;
        }
        let b = POOL + 0x44 * i as u32;
        set_counter4(fx, b, 0x26, 0, 0xFF);
        let x = fx.rand_from(D, 0x01E0_55FC) >> 4;
        fx.mem.set8(b + 4, (x & 3) << 4);
        fx.mem.write_u8(b + 8, 0);
        fx.mem.write_u8(b + 0xB, 1);
        fx.mem.write_u8(b + 0xA, 5);
        let x = fx.rand_from(D, 0x01E0_562C) >> 4;
        fx.mem.set8(b + 9, x & 3);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x50)] {
            fx.mem.write_u8(b + o, v);
        }
        for (o, ra, base) in [(0x10u32, 0x01E0_565Cu32, 80.0f32), (0x14, 0x01E0_56C8, 10.0)] {
            let x = fx.rand_from(D, ra) >> 4;
            let (_, h) = fx.div(x, 5);
            let f = ee::div(ee::from_i32(h * 200 - 0x1F4), 100.0);
            fx.mem.setf(b + o, ee::add(base, f));
        }
        fx.mem.write_u32(b + 0x18, 0xBCF5_C28F);
        let y = fx.mem.read_u32(sp + 0x84);
        fx.mem.write_u32(b + 0x1C, y);
        let x = fx.mem.read_u32(sp + 0x80);
        fx.mem.write_u32(b + 0x20, x);
        fx.mem.write_u32(b + 0x24, y);
        let z = fx.mem.read_u32(sp + 0x88);
        fx.mem.write_u32(b + 0x28, z);
        let n = fx.mem.u16(D);
        let (_, m) = fx.div(n, 0x168);
        // mula.s / madd.s: the products rounded, then the sum.
        let acc = ee::mul(6.0, ee::from_i32(m));
        let deg = ee::sub(360.0, ee::add(acc, ee::mul(90.0, ee::from_i32(k))));
        let f = spread(fx, 0x01E0_57BC, 0x50, 0x28);
        let v = ee::add(f32::from_bits(0x3F4C_CCCD), f);
        let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), deg), 180.0);
        let c = libm_cosf(fx, a);
        fx.mem.setf(b + 0x2C, ee::mul(v, c));
        let s = fx.libm_sinf(a);
        fx.mem.setf(b + 0x34, ee::mul(v, s));
        fx.mem.write_u32(b + 0x30, 0x3ECC_CCCD);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x70, u64::from(D), 0x01E0_521C);
    match fx.mem.u16(D) {
        0x14D => return 8,
        0xE0 => strike(fx, sp),
        0xDF => emitter_start(fx),
        _ => {}
    }
    if fx.mem.u16(D) >= 0x47 {
        spin(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_5894, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_58A4, [Some(u64::from(D)), Some(u64::from(OT))]);
    let n = fx.mem.u16(D);
    object_ops(fx, OPS, n);
    blib::fade::fade(fx, D + 0x1230, OT);
    blib::arc::arcs(fx, 0x01E0_0410, D + 0xEF0, OT, 1);
    blib::streak::ribbon(fx, 0x01E0_2240, D + 0x1090, OT, 4);
    flash_saving(fx, 0x01E0_1690, D + 0x1238, 1, 0x01E0_59BC);
    blib::emit::bead_trails(fx, 0x01E0_4B50, D + 0xE64, 1);
    blib::beam::beads(fx, 0x01E0_3D80, D + 0xA4, OT, 0x28);
    blib::sprite::bounce(fx, POOL, OT, 0xFA);
    let n = fx.mem.u16(D);
    if (0xE0..0xFD).contains(&n) {
        6
    } else if n == 0xDD {
        2
    } else if n == 0xD6 {
        1
    } else {
        0
    }
}
