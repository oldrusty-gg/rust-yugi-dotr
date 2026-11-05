//! BMODULE b213 (card 177), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04D00, data area a0 = 0x1ED0000) [F] (helper B4, 31
//! Oct 2026): b076's "a0 = D" family. On update 2 thirty bouncing sprites
//! in a ring round the attacker's node 0x45 (pool of 60 searched at
//! +0xAC8); on 25 an arc (+0xA0, nodes 0x32 / 0x31); on 26 at the target a
//! fade (+0xA58), three shrinking sheets (+0x698 + 0x3C k, stacked 0.8
//! apart, turned 0x4800 about z, mirrored by the attacker's facing), two
//! flashes (+0x4C0), 200 bouncing sprites, four bars (+0x558), a ribbon
//! (+0xA60) and a camera shake (+0x40); object ops 0x1F83B0 from the
//! record's table 0x1E05C70. Returns 1 on update 17, 2 on 24, 6 on 27..47,
//! 8 on 75 (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `bars` 0x1E00FC0, `arcs` 0x1E019D0, `sheets` 0x1E02C50,
//! `fade` 0x1E031C0, `ribbon` 0x1E03700, `bounce` 0x1E04290
//! [`crate::blib`].
use crate::b035::object_ops;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b245::{bounce_spawn_sized, Bounce};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const POOL: u32 = D + 0xAC8;
const BYTES: u32 = D + 0x3FE8;
/// The object-op table (rodata, 0x14-byte entries up to a 0 start).
const OPS: u32 = 0x01E0_5C70;

/// The entry 0x1E04D00 (a0 = D, a1 = param; s0 = D, s1 = the OT).
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
    fx.bzero(D, 0x40F0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(4, 2);
    for k in 0..4 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x45, 0), (0x14, 0x3D, 0), (0x18, 0x32, 0), (0x1C, 0x31, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The entry's s0 = D, s1 = the OT, s2 = 4 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4E6C, [Some(D), Some(OT), Some(4)]);
    0
}

/// ((rand >> 4) % m) x 200 - off, over 100.
fn spread200(fx: &mut Fx, ra: u32, m: i32, off: i32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h * 200 - off), 100.0)
}

/// Update 2 [F 0x1E04ED0]: thirty sprites 12 degrees apart round node
/// +0x10 (sp + 0x70).
fn ring_start(fx: &mut Fx, sp: u32) {
    for k in 0..0x1Ei32 {
        let i = first_free(fx, POOL, 0x44, 0x3C);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(D, 0x01E0_4F04) >> 4) & 7;
        let b = POOL + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, delay, 0xFF);
        let x = fx.rand_from(D, 0x01E0_4F38) >> 4;
        fx.mem.set8(b + 4, (x & 3) << 4);
        fx.mem.write_u8(b + 8, 0);
        fx.mem.write_u8(b + 0xB, 1);
        fx.mem.write_u8(b + 0xA, 5);
        let x = fx.rand_from(D, 0x01E0_4F68) >> 4;
        fx.mem.set8(b + 9, x & 3);
        for o in [0xCu32, 0xD, 0xE, 0xF] {
            fx.mem.write_u8(b + o, 0x80);
        }
        let f = spread200(fx, 0x01E0_4F94, 3, 0x12C);
        fx.mem.setf(b + 0x10, ee::add(7.0, f));
        let f = spread200(fx, 0x01E0_5000, 6, 0x258);
        fx.mem.setf(b + 0x14, ee::add(12.0, f));
        fx.mem.write_u32(b + 0x18, 0xBCA3_D70A);
        fx.mem.write_u32(b + 0x1C, 0);
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0x10);
        node_position_saving(fx, h, n, sp + 0x70, u64::from(D), Some(u64::from(OT)), 0x01E0_5088);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        let deg = ee::mul(12.0, ee::from_i32(k));
        let x = fx.rand_from(D, 0x01E0_50C4) >> 4;
        let (_, h) = fx.div(x, 0x28);
        let v = ee::add(f32::from_bits(0x3F19_999A), ee::div(ee::from_i32(h - 0x14), 100.0));
        let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), deg), 180.0);
        let c = libm_cosf(fx, a);
        fx.mem.setf(b + 0x2C, ee::mul(v, c));
        let s = fx.libm_sinf(a);
        fx.mem.setf(b + 0x34, ee::mul(v, s));
        fx.mem.write_u32(b + 0x30, 0x3E99_999A);
    }
}

/// Update 25 [F 0x1E05194]: the arc (+0xA0, points +0x100), nodes +0x18
/// / +0x1C.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0xA0;
    set_counter4(fx, g, 4, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x100);
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

/// b213's (0x1E054C8).
const BOUNCE: Bounce = Bounce { count: 0x18, b8: 2, base: 60.0, vy: 0xBF4C_CCCD, spread: (0xB4, 0x5A), s0: D, ras: [0x01E0_54F0, 0x01E0_5558, 0x01E0_55F8, 0x01E0_5644, 0x01E0_56A0] };

/// Update 26 [F 0x1E05254]: at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
    set_counter4(fx, D + 0xA58, 0xC, 0, 0xFF);
    fx.mem.write_u8(D + 0xA5C, 1);
    for k in 0..3i32 {
        let r = D + 0x698 + 0x3C * k as u32;
        set_counter4(fx, r, 0x14, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 1), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        let p = f32::from_bits(0x3F4C_CCCD);
        let d = ee::add(f32::from_bits(0xBF4C_CCCD), ee::mul(p, ee::from_i32(k)));
        let y = fx.mem.f(r + 0x30);
        fx.mem.setf(r + 0x30, ee::add(y, d));
        fx.mem.write_u32(r + 0x10, 0x4000_0000);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        fx.mem.write_u32(r + 0x18, if k == 1 { 0x4040_0000 } else { 0x4000_0000 });
        fx.mem.set32(r + 0x24, 0x4800);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let v = fx.mem.i32(r + 0x24);
            fx.mem.set32(r + 0x24, v.wrapping_neg());
        }
    }
    for k in 0..2u32 {
        let r = D + 0x4C0 + 0x4C * k;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u8(r + 7, 9);
        if k != 0 {
            fx.mem.write_u32(r + 0x10, 0x42C8_0000);
            fx.mem.write_u32(r + 0x14, 0x4220_0000);
            fx.mem.write_u32(r + 0x18, 0x41A0_0000);
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
    bounce_spawn_sized(fx, POOL, at, &BOUNCE, (0xA, 0x3E8));
    for k in 0..4i32 {
        let r = D + 0x558 + 0x50 * k as u32;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u32(r + 0x10, if k == 2 { 0x44AF_0000 } else { 0x4461_0000 });
        fx.mem.write_u32(r + 0x14, 0x4248_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x40), (0xA, 0x80), (9, 0x40), (8, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.setf(r + 0x18, ee::mul(f32::from_bits(0x4234_0000), ee::from_i32(k)));
        fx.mem.write_u32(r + 0x1C, 0x4080_0000);
        copy_v4(fx, at, r + 0x20);
    }
    let r = D + 0xA60;
    fx.mem.write_u8(r + 4, 5);
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    let b = fx.mem.read_u8(r + 4) | 2;
    fx.mem.write_u8(r + 4, b);
    fx.mem.write_u32(r + 0x14, 0x3FCC_CCCD);
    fx.mem.write_u32(r + 0x1C, 0x42A0_0000);
    fx.mem.write_u8(r + 0x23, 0x80);
    fx.mem.write_u8(r + 0x27, 0x80);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 0xA, 2);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 3);
    fx.mem.write_u8(r + 8, 0x80);
    let x = fx.rand_from(D, 0x01E0_5854);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
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
    unit_position_saving(fx, t, D + 0x60, u64::from(D), 0x01E0_5910);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_4E94);
    match fx.mem.u16(D) {
        0x4B => return 8,
        0x1A => strike(fx, sp),
        0x19 => arcs_start(fx),
        2 => ring_start(fx, sp),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_5930, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    object_ops(fx, OPS, n);
    blib::star::flash(fx, 0x01E0_0410, D + 0x4C0, OT, 2);
    blib::cross::bars(fx, 0x01E0_0FC0, D + 0x558, OT, 4);
    blib::arc::arcs(fx, 0x01E0_19D0, D + 0xA0, OT, 1);
    blib::sheet::sheets(fx, 0x01E0_2C50, D + 0x698, OT, 0x10);
    blib::fade::fade(fx, D + 0xA58, OT);
    blib::sprite::bounce(fx, POOL, OT, 0xC8);
    blib::streak::ribbon(fx, 0x01E0_3700, D + 0xA60, OT, 1);
    let n = fx.mem.u16(D);
    if (0x1B..0x30).contains(&n) {
        6
    } else if n == 0x18 {
        2
    } else if n == 0x11 {
        1
    } else {
        0
    }
}
