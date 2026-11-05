//! BMODULE b404 (card 489), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03E50, data area a0 = 0x1ED0000, 0x5EE0 bytes)
//! [F]: s0 = D, s1 = the OT (0x206E40), s2 the
//! parameter then offsets. b381's library at the same addresses (b596's
//! layout). Init sets the spark-emitter group (+0xA4, `spawn::emitters` /
//! `sprite::sparks`: update lists 0x1E04848 / 0x1E04838 at nodes 0x24 /
//! 0x2E). Update 2 node 0x2E read (+0x5DCC, 5 lower); 75, 83 and 94 at
//! the target a spike (+0x452C, 0x68 apart, 50), a flash (+0x44E0), four
//! bands (+0x597C, delayed 0..3, turned about x / y / z by update) and a
//! shake (+0x40), on 94 first 180 bouncing sprites (+0xFB8, delayed s /
//! 18); on even updates 76..94 a thinner spike. The fade (+0x44D8) is
//! stepped, never set [F]. Returns 6 on updates 75..122, 2 on 123, 1 on
//! 65, 8 on 163 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080 (0538257fe1fa),
//! `sparks` 0x1E003A0, `fan_ring` 0x1E00C70, `emitters` 0x1E00F40,
//! `bands_b` 0x1E01390, `flash_b` 0x1E01AD0, `drops` 0x1E02540, `fade`
//! 0x1E02EA0, `spikes` 0x1E03370 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SPRITES: u32 = D + 0xFB8;
const FADE: u32 = D + 0x44D8;
const FLASH: u32 = D + 0x44E0;
const SPIKES: u32 = D + 0x452C;
const BANDS: u32 = D + 0x597C;
const BYTES: u32 = D + 0x5DDC;

/// The entry 0x1E03E50 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x60);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x5EE0).
    fx.bzero(D, 0x5EE0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..4 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x2E, 1), (0x10, 0x24, 0), (0x14, 1, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The emitter group (+0xA4).
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xCC, b);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_4848);
    fx.mem.write_u32(D + 0xB0, 0x01E0_4838);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x20);
    fx.mem.write_u32(D + 0xA8, D + 0x7E8);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    fx.mem.write_u16(D + 0xD8, 0);
    fx.mem.write_u32(D + 0xDC, 0x435C_0000);
    fx.mem.write_u32(D + 0xE0, 0x42F0_0000);
    fx.mem.write_u32(D + 0xE4, 0x4110_0000);
    fx.mem.set16(D + 0xA0, 0x45);
    fx.mem.set16(D + 0xA2, 0x9D);
    // The entry's s0 = D, s1 = 4 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_3FB4, [Some(D), Some(4), Some(param as u32)]);
    0
}

fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 4
}

/// ((rand >> 4) % m - half) / 100.
fn jit(fx: &mut Fx, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

/// Update 94 [F 0x1E0404C]: 180 sprites, delayed s / 18.
fn sprites(fx: &mut Fx, sp: u32) {
    for s3 in 0..0xB4i32 {
        let i = first_free(fx, SPRITES, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let count = ((rand4(fx, 0x01E0_4070) & 0xF) + 0x20) & 0xFF;
        let r = SPRITES + 0x44 * i as u32;
        let p = i64::from(0x38E3_8E39i32) * i64::from(s3);
        let hi = (p >> 32) as i32;
        fx.set_hilo(p as i32, hi);
        let delay = (hi >> 2) + ((s3 as u32) >> 31) as i32;
        set_counter4(fx, r, count, delay & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0x42u8), (8, 3), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        let v = (rand4(fx, 0x01E0_40DC) & 3) + 2;
        fx.mem.set8(r + 9, v);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u32(r + 0x10, 0x42A0_0000);
        fx.mem.write_u32(r + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(r + 0x18, 0xBD75_C28F);
        let y = fx.mem.read_u32(D + 0x5DD0);
        fx.mem.write_u32(r + 0x1C, y);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let f = jit(fx, 0x01E0_4144, 0x96, 0x4B);
        fx.mem.setf(r + 0x2C, f);
        let f = ee::add(f32::from_bits(0x3F33_3333), jit(fx, 0x01E0_4180, 0x64, 0x32));
        fx.mem.setf(r + 0x30, f);
        let f = jit(fx, 0x01E0_41C8, 0x96, 0x4B);
        fx.mem.setf(r + 0x34, f);
    }
}

/// Updates 75 / 83 / 94 [F 0x1E0420C]: the strike at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    let i = first_free(fx, SPIKES, 0x68, 0x32);
    if i >= 0 {
        let r = SPIKES + 0x68 * i as u32;
        set_counter4(fx, r, 0x20, 0, 0xFF);
        for (o, v) in [(4u32, 7u8), (0xA, 3), (0xC, 3)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x14, 0x3FC0_0000);
        fx.mem.write_u32(r + 0x1C, 0x42B4_0000);
        for (o, v) in [(0x23u32, 0x80u8), (0x27, 0x20), (8, 0x78), (5, 0), (0xB, 2)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = rand4(fx, 0x01E0_429C);
        fx.mem.set8(r + 9, x);
        fx.mem.write_u32(r + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0x50, r + 0x28);
        for o in [0x60u32, 0x5C, 0x58] {
            fx.mem.write_u32(r + o, 0);
        }
    }
    // The flash (+0x44E0).
    let a = FLASH;
    set_counter4(fx, a, 0x20, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xA, 0x40), (9, 0x10), (8, 0x80)] {
        fx.mem.write_u8(a + o, v);
    }
    fx.mem.write_u32(a + 0x14, 0);
    fx.mem.write_u32(a + 0x10, 0x447A_0000);
    copy_v4(fx, sp + 0x50, a + 0x1C);
    // Four bands (+0x597C, 0x5C apart, 12), delayed 0..3.
    for s3 in 0..4i32 {
        let i = first_free(fx, BANDS, 0x5C, 0xC);
        if i < 0 {
            continue;
        }
        let b = BANDS + 0x5C * i as u32;
        set_counter4(fx, b, 0x1C, s3 & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 1), (8, 2), (9, 2), (0xA, 6), (0xB, 0)] {
            fx.mem.write_u8(b + o, v);
        }
        fx.mem.write_u32(b + 0x10, 0x3F80_0000);
        fx.mem.write_u32(b + 0x14, 0);
        fx.mem.write_u32(b + 0x18, 0x3F80_0000);
        fx.mem.write_u32(b + 0x1C, 0x3F00_0000);
        fx.mem.write_u32(b + 0x20, 0x3E80_0000);
        for (o, v) in [(0x25u32, 0x60u8), (0x26, 0x60), (0x24, 0xC0), (0x29, 0x60), (0x2A, 0x60), (0x28, 0xC0), (0x27, 0x10), (0x2B, 0x30)] {
            fx.mem.write_u8(b + o, v);
        }
        copy_v4(fx, sp + 0x50, b + 0x2C);
        match fx.mem.u16(D) {
            0x5E => fx.mem.set32(b + 0x44, -0x2000),
            0x53 => fx.mem.set32(b + 0x44, 0x3000),
            0x4B => {
                fx.mem.set32(b + 0x3C, 0x4000);
                fx.mem.set32(b + 0x40, 0x3000);
                fx.mem.set32(b + 0x44, 0x2000);
            }
            _ => {}
        }
    }
    // The camera shake (+0x40) at the target.
    set_counter4(fx, D + 0x40, 0x20, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x3F80_0000);
    fx.mem.write_u32(D + 0x58, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x5C, 0x3FB3_3333);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    copy_v4(fx, sp + 0x50, D + 0x60);
}

/// Even updates 76..94 [F 0x1E04588]: a thinner spike at the target.
fn spike(fx: &mut Fx, sp: u32) {
    let i = first_free(fx, SPIKES, 0x68, 0x32);
    if i < 0 {
        return;
    }
    let r = SPIKES + 0x68 * i as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4020_0000);
    fx.mem.write_u32(r + 0x1C, 0x41C8_0000);
    for (o, v) in [(8u32, 0x28u8), (5, 0), (0xA, 1), (0xB, 2), (0xC, 5)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = rand4(fx, 0x01E0_460C);
    fx.mem.set8(r + 9, x);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x60)] {
        fx.mem.write_u8(r + o, v);
    }
    for (o, ra) in [(0x58u32, 0x01E0_4654u32), (0x5C, 0x01E0_4668), (0x60, 0x01E0_467C)] {
        let v = rand4(fx, ra) & 0x7F;
        fx.mem.set32(r + o, v);
    }
    copy_v4(fx, sp + 0x50, r + 0x28);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(D), 0x01E0_3FD4);
    match fx.mem.u16(D) {
        0xA3 => return 8,
        0x4B | 0x53 => strike(fx, sp),
        // The sprite loop falls through into the strike [F 0x1E04204].
        0x5E => {
            sprites(fx, sp);
            strike(fx, sp);
        }
        2 => {
            let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
            node_position_saving(fx, h, s, D + 0x5DCC, u64::from(D), Some(u64::from(OT)), 0x01E0_402C);
            let y = ee::sub(fx.mem.f(D + 0x5DD0), 5.0);
            fx.mem.setf(D + 0x5DD0, y);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x4B..0x5F).contains(&n) && n & 1 == 0 {
        spike(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_46BC, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_46C8, [Some(u64::from(D)), Some(u64::from(OT))]);
    let n = fx.mem.u16(D);
    blib::spawn::emitters(fx, 0x01E0_0F40, D + 0xA4, n);
    blib::sprite::sparks(fx, D + 0xE8, OT, 0x20);
    blib::star::flash_b(fx, 0x01E0_1AD0, FLASH, OT, 1);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the runner's s6 (0) at +0x70 and ra at +0x80.
    fx.saved(sp - 0xF0, &[(0x70, 0), (0x80, 0x01E0_4700)]);
    blib::spike::spikes(fx, 0x01E0_3370, SPIKES, OT, 0x32);
    blib::drop::drops(fx, SPRITES, OT, 0xC8);
    blib::band::bands_b(fx, 0x01E0_1390, BANDS, OT, 0xC);
    let n = fx.mem.u16(D);
    if n < 0x4B {
        if n == 0x41 {
            1
        } else {
            0
        }
    } else if n < 0x7B {
        6
    } else if n == 0x7B {
        2
    } else {
        0
    }
}
