//! BMODULE b254 (card 246), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03510, data area a0 = 0x1ED0000) [F] (helper B4, 31
//! Oct 2026): b076's "a0 = D" family. On update 2 the attacker's node 0x13
//! (+0xBE2C, its height +0xBE30 used below); on 12 four crossing bars
//! (+0xBCEC + 0x50 k, `cross::bars`), a flash (+0xAC, `star::flash`) and a
//! camera shake (+0x40), all at the attacker's node 0x19; on 45 thirty-six
//! bouncing sprites round the attacker's node 1 (`sprite::bounce`, pool of
//! 500 at +0x228C, the first 300 searched); on 82..138 three a update from
//! between nodes 1 / 0x39; on 95 two flashes at the target, a fade (+0xA4)
//! and a second shake; on 95..138 ten sprites and a ribbon (+0xA2C,
//! `streak::ribbon`, pool of 60) a update at the target (mirrored by the
//! attacker's facing +0x30); the stage light pulsed by 0x1031C0 (+0xA0:
//! down on update 2, up on 194). Returns 1 on update 85, 6 on 95..151, 2
//! on 152, 8 on 200 (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `ribbon` 0x1E00FC0, `bounce` 0x1E01B50, `fade` 0x1E025C0,
//! `bars` 0x1E02B00 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const POOL: u32 = D + 0x228C;
/// The node 0x13's height (written on update 2).
const HEIGHT: u32 = D + 0xBE30;

/// The entry 0x1E03510 (a0 = D, a1 = param; s0 = D, s1 = the OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xBF40);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x19, 0), (0x14, 0x39, 0), (0x18, 0x13, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xC2);
    // The entry's s0 = D, s1 = the OT, s2 = 2 (the texture loop's).
    rand_bytes(fx, D + 0xBE3C, 0x100, 0x01E0_3670, [Some(D), Some(OT), Some(2)]);
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

/// Mirrored by the attacker's facing: unless +0x30 is 0x8000, f x -1.
fn facing(fx: &mut Fx, p: u32) {
    if fx.mem.read_u32(D + 0x30) != 0x8000 {
        let f = fx.mem.f(p);
        fx.mem.setf(p, ee::mul(f, -1.0));
    }
}

/// Update 12 [F 0x1E03700]: four bars (+0xBCEC + 0x50 k), a flash (+0xAC)
/// and the shake (+0x40), all at node +0x10.
fn bars_start(fx: &mut Fx) {
    for k in 0..4i32 {
        let r = D + 0xBCEC + 0x50 * k as u32;
        set_counter4(fx, r, 0x12, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0x10);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 4, 0x80);
        if k == 0 {
            fx.mem.write_u32(r + 0x10, 0x4416_0000);
            fx.mem.write_u32(r + 0x18, 0x4234_0000);
            fx.mem.write_u32(r + 0x14, 0x4170_0000);
        } else {
            fx.mem.write_u32(r + 0x10, 0x4348_0000);
            let a = f32::from_bits(0x4234_0000);
            let f = ee::mul(a, ee::from_i32(k));
            fx.mem.setf(r + 0x18, ee::add(a, f));
            fx.mem.write_u32(r + 0x14, 0x4120_0000);
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0xC0), (9, 0x20), (8, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x40C0_0000);
    }
    let g = D + 0xAC;
    set_counter4(fx, g, 0x12, 0, 0xFF);
    fx.mem.write_u8(g + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(g + 6, b);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x42B4_0000);
    fx.mem.write_u32(g + 0x14, 0x4100_0000);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x40), (9, 0x10), (8, 0x80), (0xF, 0xD0)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x12, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(s + 6, b);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x4100_0000);
}

/// Update 45 [F 0x1E03994]: 36 sprites in a ring (10 degrees apart) round
/// node +0xC (sp + 0x90).
fn ring_start(fx: &mut Fx, sp: u32) {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x90, u64::from(D), Some(u64::from(OT)), 0x01E0_39A8);
    for k in 0..0x24i32 {
        let i = first_free(fx, POOL, 0x44, 0x12C);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(D, 0x01E0_39DC) >> 4) & 7;
        let b = POOL + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, delay, 0xFF);
        let x = fx.rand_from(D, 0x01E0_3A10) >> 4;
        fx.mem.set8(b + 4, (x & 3) << 4);
        fx.mem.write_u8(b + 8, 1);
        fx.mem.write_u8(b + 0xB, 1);
        fx.mem.write_u8(b + 0xA, 5);
        let x = fx.rand_from(D, 0x01E0_3A40) >> 4;
        fx.mem.set8(b + 9, x & 3);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x50)] {
            fx.mem.write_u8(b + o, v);
        }
        // A signed (rand >> 4) % 8.
        let x = fx.rand_from(D, 0x01E0_3A70) >> 4;
        let mut m = x & 7;
        if x < 0 && m != 0 {
            m -= 8;
        }
        let f = ee::div(ee::from_i32(m * 200 - 0x320), 100.0);
        fx.mem.setf(b + 0x10, ee::add(f32::from_bits(0x4302_0000), f));
        let f = spread200(fx, 0x01E0_3AE0, 3, 0x12C);
        fx.mem.setf(b + 0x14, ee::add(6.0, f));
        fx.mem.write_u32(b + 0x18, 0xBCA3_D70A);
        let y = fx.mem.read_u32(HEIGHT);
        fx.mem.write_u32(b + 0x1C, y);
        let deg = ee::mul(10.0, ee::from_i32(k));
        let f = spread(fx, 0x01E0_3B80, 0xBE, 0x5F);
        let r = ee::add(1.5, f);
        let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), deg), 180.0);
        let c = libm_cosf(fx, a);
        let x = fx.mem.f(sp + 0x90);
        fx.mem.setf(b + 0x20, ee::add(x, ee::mul(r, c)));
        let y = fx.mem.read_u32(HEIGHT);
        fx.mem.write_u32(b + 0x24, y);
        let s = fx.libm_sinf(a);
        let z = fx.mem.f(sp + 0x98);
        fx.mem.setf(b + 0x28, ee::add(z, ee::mul(r, s)));
        let f = spread(fx, 0x01E0_3C4C, 0x50, 0x28);
        let v = ee::add(0.5, f);
        let c = libm_cosf(fx, a);
        fx.mem.setf(b + 0x2C, ee::mul(v, c));
        let s = fx.libm_sinf(a);
        fx.mem.setf(b + 0x34, ee::mul(v, s));
        fx.mem.write_u32(b + 0x30, 0x3ECC_CCCD);
    }
}

/// Update 95 [F 0x1E03CEC]: the fade (+0xA4), two flashes (+0xAC + 0x4C k:
/// 800 wide, then 200) and the shake at the target (sp + 0x80).
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0xA4, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0xA8, 1);
    for k in 0..2u32 {
        let r = D + 0xAC + 0x4C * k;
        set_counter4(fx, r, 0x48, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 7, 9);
        if k == 0 {
            fx.mem.write_u8(r + 4, 0x83);
            fx.mem.write_u32(r + 0x10, 0x4448_0000);
        } else {
            fx.mem.write_u8(r + 4, 3);
            fx.mem.write_u32(r + 0x10, 0x4348_0000);
        }
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0xC0);
        }
        fx.mem.write_u8(r + 0xA, 0x60);
        fx.mem.write_u8(r + 9, 0x10);
        fx.mem.write_u8(r + 8, 0x80);
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0);
        copy_v4(fx, sp + 0x80, r + 0x1C);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x48, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x80, D + 0x60);
}

/// Updates 82..138 [F 0x1E03EA8]: three sprites a update thrown up from
/// the midpoint of nodes +0xC / +0x14 (sp + 0x90, height +0xBE30).
fn rise(fx: &mut Fx, sp: u32) {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x90, u64::from(D), Some(u64::from(OT)), 0x01E0_3EC8);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x14);
    node_position_saving(fx, h, n, sp + 0xA0, u64::from(D), Some(u64::from(OT)), 0x01E0_3EE0);
    for (p, q) in [(sp + 0x90, sp + 0xA0), (sp + 0x98, sp + 0xA8)] {
        let a = fx.mem.f(p);
        let d = ee::sub(fx.mem.f(q), a);
        fx.mem.setf(p, ee::add(a, ee::mul(0.5, d)));
    }
    let y = fx.mem.read_u32(HEIGHT);
    fx.mem.write_u32(sp + 0x94, y);
    for _ in 0..3 {
        let i = first_free(fx, POOL, 0x44, 0x1F4);
        if i < 0 {
            continue;
        }
        let b = POOL + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, 0, 0xFF);
        let x = fx.rand_from(D, 0x01E0_3F90) >> 4;
        fx.mem.set8(b + 4, (x & 3) << 4);
        fx.mem.write_u8(b + 8, 1);
        for (o, v) in [(0xEu32, 0x80u8), (0xD, 0xA0), (0xC, 0xC0), (0xF, 0x60)] {
            fx.mem.write_u8(b + o, v);
        }
        let f = spread200(fx, 0x01E0_3FD4, 0x1E, 0xBB8);
        fx.mem.setf(b + 0x10, ee::add(90.0, f));
        let f = spread200(fx, 0x01E0_4040, 5, 0x1F4);
        fx.mem.setf(b + 0x14, ee::add(10.0, f));
        fx.mem.write_u8(b + 0xB, 1);
        fx.mem.write_u8(b + 0xA, 5);
        let x = fx.rand_from(D, 0x01E0_40B8) >> 4;
        fx.mem.set8(b + 9, x & 3);
        fx.mem.write_u32(b + 0x18, 0);
        let y = fx.mem.read_u32(HEIGHT);
        fx.mem.write_u32(b + 0x1C, y);
        for (k, ra) in [0x01E0_40E4u32, 0x01E0_4134, 0x01E0_4184].into_iter().enumerate() {
            let f = spread(fx, ra, 0x190, 0xC8);
            let a = fx.mem.f(sp + 0x90 + 4 * k as u32);
            fx.mem.setf(b + 0x20 + 4 * k as u32, ee::add(a, f));
        }
        let f = spread(fx, 0x01E0_41D4, 0x1E, 0xF);
        fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3E4C_CCCD), f));
        let f = spread(fx, 0x01E0_4230, 0x3C, 0x1E);
        fx.mem.setf(b + 0x2C, f);
        let f = spread(fx, 0x01E0_427C, 0xB4, 0x5A);
        fx.mem.setf(b + 0x34, ee::add(1.5, f));
        facing(fx, b + 0x34);
    }
}

/// Updates 95..138 [F 0x1E04330]: ten sprites and a ribbon a update at the
/// target (sp + 0x80).
fn spray(fx: &mut Fx, sp: u32) {
    for _ in 0..10 {
        let i = first_free(fx, POOL, 0x44, 0x1F4);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(D, 0x01E0_4360) >> 4) & 7;
        let b = POOL + 0x44 * i as u32;
        set_counter4(fx, b, 0x24, delay, 0xFF);
        fx.mem.write_u8(b + 4, 0x42);
        fx.mem.write_u8(b + 8, 0);
        fx.mem.write_u8(b + 0xB, 2);
        fx.mem.write_u8(b + 0xA, 0);
        let x = fx.rand_from(D, 0x01E0_43B0) >> 4;
        fx.mem.set8(b + 9, x & 3);
        for o in [0xCu32, 0xD, 0xE, 0xF] {
            fx.mem.write_u8(b + o, 0x80);
        }
        fx.mem.write_u32(b + 0x10, 0x4270_0000);
        fx.mem.write_u32(b + 0x14, 0xBF00_0000);
        fx.mem.write_u32(b + 0x18, 0xBDCC_CCCD);
        let y = fx.mem.read_u32(HEIGHT);
        fx.mem.write_u32(b + 0x1C, y);
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * k);
            fx.mem.write_u32(b + 0x20 + 4 * k, w);
        }
        let f = spread(fx, 0x01E0_4420, 0x78, 0x3C);
        fx.mem.setf(b + 0x2C, f);
        let f = spread(fx, 0x01E0_446C, 0x78, 0x3C);
        fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3FE6_6666), f));
        let f = spread(fx, 0x01E0_44C8, 0xBE, 0x5F);
        fx.mem.setf(b + 0x34, f);
        facing(fx, b + 0x34);
    }
    let i = first_free(fx, D + 0xA2C, 0x68, 0x3C);
    if i < 0 {
        return;
    }
    let r = D + 0xA2C + 0x68 * i as u32;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4000_0000);
    fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
    fx.mem.write_u8(r + 8, 0xC);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 0xA, 0);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 6);
    let x = fx.rand_from(D, 0x01E0_45D8);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0xBE3C);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    fx.mem.write_u8(r + 0x23, 0x10);
    fx.mem.write_u8(r + 0x27, 0x80);
    let x = fx.rand_from(D, 0x01E0_4628) >> 4;
    fx.mem.set32(r + 0x60, x & 0x3FF);
    if fx.mem.read_u32(D + 0x30) != 0x8000 {
        let v = fx.mem.i32(r + 0x60);
        fx.mem.set32(r + 0x60, v.wrapping_neg());
    }
    copy_v4(fx, sp + 0x80, r + 0x28);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x80, u64::from(D), 0x01E0_3698);
    match fx.mem.u16(D) {
        0xC8 => return 8,
        0x5F => strike(fx, sp),
        0x2D => ring_start(fx, sp),
        0xC => bars_start(fx),
        2 => {
            let h = fx.mem.i32(D + 4);
            let n = fx.mem.i32(D + 0x18);
            node_position_saving(fx, h, n, D + 0xBE2C, u64::from(D), Some(u64::from(OT)), 0x01E0_36F8);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x52..0x8B).contains(&n) {
        rise(fx, sp);
    }
    let n = fx.mem.u16(D);
    if (0x5F..0x8B).contains(&n) {
        spray(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_468C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_469C, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0xA4, OT);
    blib::star::flash(fx, 0x01E0_0410, D + 0xAC, OT, 5);
    blib::streak::ribbon(fx, 0x01E0_0FC0, D + 0xA2C, OT, 0x3C);
    blib::sprite::bounce(fx, POOL, OT, 0x1F4);
    blib::cross::bars(fx, 0x01E0_2B00, D + 0xBCEC, OT, 4);
    let n = fx.mem.u16(D);
    if (0x5F..0x98).contains(&n) {
        6
    } else if n == 0x98 {
        2
    } else if n == 0x55 {
        1
    } else {
        0
    }
}
