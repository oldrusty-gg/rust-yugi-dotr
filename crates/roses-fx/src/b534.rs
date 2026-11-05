//! BMODULE b534 (card 421), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03DB0, data area a0 = 0x1ED0000) [F] (helper B4, 31
//! Oct 2026): b076's "a0 = D" family. On update 2 the attacker's node 0x48
//! (+0x9A14, its height +0x9A18 used below); on 15, 42, 138 thirty-six
//! bouncing sprites in a ring round node 1 (pool of 500 at +0x11C4, the
//! first 300 searched); on 50 + 12 k (k = 0..5) an arc (+0x9694, nodes
//! 0x2F / 0x2E or 0x3C / 0x3B in turn); on 54 + 12 k at the target three
//! shrinking sheets (pool of 6 at +0x1D4, side by side, turned -+0x1800
//! about z, mirrored by the attacker's facing), a flash (pool of 4 at
//! +0xA4), 100 bouncing sprites and a camera shake (+0x40); on even updates
//! 54..114 a ribbon (pool of 30 at +0x594); the stage light pulsed by
//! 0x1031C0 (+0xA0: down on update 48, up on 164). Returns 1 on update 44,
//! 6 on 54..126, 2 on 127, 8 on 170 (and at once for a parameter of -2 /
//! -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `sheets` 0x1E00FC0, `arcs` 0x1E01530, `ribbon` 0x1E027B0,
//! `bounce` 0x1E03340 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const POOL: u32 = D + 0x11C4;
const BYTES: u32 = D + 0x9A24;
/// The node 0x48's height (written on update 2).
const HEIGHT: u32 = D + 0x9A18;

/// The entry 0x1E03DB0 (a0 = D, a1 = param; s0 = D, s1 = the OT).
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
    fx.bzero(D, 0x9B30);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x2F, 0), (0x14, 0x2E, 0), (0x18, 0x3C, 0), (0x1C, 0x3B, 0), (0x20, 0x48, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 0x30);
    fx.mem.set16(D + 0xA2, 0xA4);
    // The entry's s0 = D, s1 = the OT, s2 = 3 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_3F38, [Some(D), Some(OT), Some(3)]);
    0
}

/// (rand >> 4) % m - off, over 100 (rand called from `ra`).
fn spread(fx: &mut Fx, ra: u32, m: i32, off: i32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h - off), 100.0)
}

/// ((rand >> 4) % m) x 200 - off, over 100.
fn spread200(fx: &mut Fx, ra: u32, m: i32, off: i32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h * 200 - off), 100.0)
}

/// Updates 15 / 42 / 138 [F 0x1E04058]: 36 sprites in a ring (10 degrees
/// apart) round node +0xC (sp + 0x70).
fn ring_start(fx: &mut Fx, sp: u32) {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x70, u64::from(D), Some(u64::from(OT)), 0x01E0_406C);
    for k in 0..0x24i32 {
        let i = first_free(fx, POOL, 0x44, 0x12C);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(D, 0x01E0_40A0) >> 4) & 7;
        let b = POOL + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, delay, 0xFF);
        let x = fx.rand_from(D, 0x01E0_40D4) >> 4;
        fx.mem.set8(b + 4, (x & 3) << 4);
        fx.mem.write_u8(b + 8, 0);
        fx.mem.write_u8(b + 0xB, 1);
        fx.mem.write_u8(b + 0xA, 5);
        let x = fx.rand_from(D, 0x01E0_4104) >> 4;
        fx.mem.set8(b + 9, x & 3);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x50)] {
            fx.mem.write_u8(b + o, v);
        }
        let f = spread200(fx, 0x01E0_4134, 0xA, 0x3E8);
        fx.mem.setf(b + 0x10, ee::add(150.0, f));
        let f = spread200(fx, 0x01E0_41A0, 3, 0x12C);
        fx.mem.setf(b + 0x14, ee::add(7.0, f));
        fx.mem.write_u32(b + 0x18, 0xBCA3_D70A);
        let y = fx.mem.read_u32(HEIGHT);
        fx.mem.write_u32(b + 0x1C, y);
        let deg = ee::mul(10.0, ee::from_i32(k));
        let f = spread(fx, 0x01E0_4240, 0xBE, 0x5F);
        let r = ee::add(2.0, f);
        let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), deg), 180.0);
        let c = libm_cosf(fx, a);
        let x = fx.mem.f(sp + 0x70);
        fx.mem.setf(b + 0x20, ee::add(x, ee::mul(r, c)));
        let y = fx.mem.read_u32(HEIGHT);
        fx.mem.write_u32(b + 0x24, y);
        let s = fx.libm_sinf(a);
        let z = fx.mem.f(sp + 0x78);
        fx.mem.setf(b + 0x28, ee::add(z, ee::mul(r, s)));
        let f = spread(fx, 0x01E0_430C, 0x50, 0x28);
        let v = ee::add(0.5, f);
        let c = libm_cosf(fx, a);
        fx.mem.setf(b + 0x2C, ee::mul(v, c));
        let s = fx.libm_sinf(a);
        fx.mem.setf(b + 0x34, ee::mul(v, s));
        fx.mem.write_u32(b + 0x30, 0x3ECC_CCCD);
    }
}

/// Updates 50 + 12 k [F 0x1E043AC]: the arc (+0x9694, points +0x96F4),
/// nodes +0x18 / +0x1C (updates 62, 86, 110) or +0x10 / +0x14.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0x9694;
    set_counter4(fx, g, 9, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x96F4);
    fx.mem.write_u8(g + 0x30, 0x32);
    fx.mem.write_u8(g + 0x31, 8);
    let (a, b) = match fx.mem.u16(D) {
        0x6E | 0x56 | 0x3E => (Some(0x18u32), 0x1Cu32),
        0x62 | 0x4A | 0x32 => (Some(0x10), 0x14),
        _ => (None, 0),
    };
    if let Some(a) = a {
        let v = fx.mem.read_u8(D + a);
        fx.mem.write_u8(g + 0x32, v);
        let v = fx.mem.read_u8(D + b);
        fx.mem.write_u8(g + 0x33, v);
    }
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0xC0), (0x4A, 0), (0x49, 0), (0x48, 0x80)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut k = 0u32;
    while k < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0xC + 2 * k, -1);
        fx.mem.set16(g + 0x1E + 2 * k, -1);
        k += 1;
    }
}

/// Updates 54 + 12 k [F 0x1E0456C]: at the target (sp + 0x60).
fn burst(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
    for k in 0..3i32 {
        let i = first_free(fx, D + 0x1D4, 0x3C, 6);
        if i < 0 {
            continue;
        }
        let r = D + 0x1D4 + 0x3C * i as u32;
        set_counter4(fx, r, 0x10, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 1), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        fx.mem.write_u32(r + 0x10, 0x4000_0000);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        fx.mem.write_u32(r + 0x18, if k == 1 { 0x4020_0000 } else { 0x3FE6_6666 });
        let x = fx.mem.f(r + 0x2C);
        fx.mem.setf(r + 0x2C, ee::add(x, ee::add(-1.0, ee::from_i32(k))));
        match fx.mem.u16(D) {
            0x72 | 0x5A | 0x42 => fx.mem.set32(r + 0x24, 0x1800),
            0x66 | 0x4E | 0x36 => fx.mem.set32(r + 0x24, -0x1800),
            _ => {}
        }
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let v = fx.mem.i32(r + 0x24);
            fx.mem.set32(r + 0x24, v.wrapping_neg());
        }
    }
    let i = first_free(fx, D + 0xA4, 0x4C, 4);
    if i >= 0 {
        let r = D + 0xA4 + 0x4C * i as u32;
        set_counter4(fx, r, 0xC, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 7, 9);
        fx.mem.write_u8(r + 4, 0x81);
        fx.mem.write_u32(r + 0x10, 0x4416_0000);
        fx.mem.write_u32(r + 0x18, 0);
        fx.mem.write_u32(r + 0x14, 0);
        for (o, v) in [(0xFu32, 0xF0u8), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x20), (9, 0x20), (8, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(r + o, 0);
        }
        copy_v4(fx, at, r + 0x1C);
    }
    // 100 bouncing sprites (0x1E04800).
    for _ in 0..0x64 {
        let i = first_free(fx, POOL, 0x44, 0x1F4);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(D, 0x01E0_4828) >> 4) & 3;
        let b = POOL + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, delay, 0xFF);
        for (o, x) in [(4u32, 2u8), (8, 2), (0xB, 2), (0xA, 0), (9, 3), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(b + o, x);
        }
        let f = spread200(fx, 0x01E0_4890, 0xF, 0x5DC);
        fx.mem.setf(b + 0x10, ee::add(50.0, f));
        fx.mem.write_u32(b + 0x14, 0xBECC_CCCD);
        fx.mem.write_u32(b + 0x18, 0xBDB8_51EC);
        let y = fx.mem.read_u32(HEIGHT);
        fx.mem.write_u32(b + 0x1C, y);
        for c in 0..3 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        let f = spread(fx, 0x01E0_493C, 0xB4, 0x5A);
        fx.mem.setf(b + 0x2C, f);
        let f = spread(fx, 0x01E0_4988, 0xB4, 0x5A);
        fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), f));
        let f = spread(fx, 0x01E0_49E4, 0xB4, 0x5A);
        fx.mem.setf(b + 0x34, f);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 9, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(D), 0x01E0_4AA0);
}

/// Even updates 54..114 [F 0x1E04AD4]: a ribbon at the target.
fn ribbon_start(fx: &mut Fx, sp: u32) {
    let i = first_free(fx, D + 0x594, 0x68, 0x1E);
    if i < 0 {
        return;
    }
    let r = D + 0x594 + 0x68 * i as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(r + 0x1C, 0x41C8_0000);
    fx.mem.write_u8(r + 8, 0x28);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 0xA, 2);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 5);
    let x = fx.rand_from(D, 0x01E0_4B70);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    fx.mem.write_u8(r + 0x23, 0);
    fx.mem.write_u8(r + 0x27, 0x60);
    for (o, ra) in [(0x58u32, 0x01E0_4BC0u32), (0x5C, 0x01E0_4BD8), (0x60, 0x01E0_4BF0)] {
        let x = fx.rand_from(D, ra) >> 4;
        fx.mem.set32(r + o, x & 0x7F);
    }
    copy_v4(fx, sp + 0x60, r + 0x28);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_3F60);
    match fx.mem.u16(D) {
        0xAA => return 8,
        0x72 | 0x66 | 0x5A | 0x4E | 0x42 | 0x36 => burst(fx, sp),
        0x6E | 0x62 | 0x56 | 0x4A | 0x3E | 0x32 => arcs_start(fx),
        0x8A | 0x2A | 0xF => ring_start(fx, sp),
        2 => {
            let h = fx.mem.i32(D + 4);
            let n = fx.mem.i32(D + 0x20);
            node_position_saving(fx, h, n, D + 0x9A14, u64::from(D), Some(u64::from(OT)), 0x01E0_4050);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x36..0x73).contains(&n) && n & 1 == 0 {
        ribbon_start(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_4C30, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4C40, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::star::flash(fx, 0x01E0_0410, D + 0xA4, OT, 4);
    blib::sheet::sheets(fx, 0x01E0_0FC0, D + 0x1D4, OT, 0x10);
    blib::sprite::bounce(fx, POOL, OT, 0x1F4);
    blib::streak::ribbon(fx, 0x01E0_27B0, D + 0x594, OT, 0x1E);
    blib::arc::arcs(fx, 0x01E0_1530, D + 0x9694, OT, 1);
    let n = fx.mem.u16(D);
    if (0x36..0x7F).contains(&n) {
        6
    } else if n == 0x7F {
        2
    } else if n == 0x2C {
        1
    } else {
        0
    }
}
