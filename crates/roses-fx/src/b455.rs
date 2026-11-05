//! BMODULE b455 (card 636), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05B40, data area a0 = 0x1ED0000, 0x56F0 bytes)
//! [F]: s0 = D, s1 = the OT (0x206E40), s2 the
//! parameter then offsets; every call first copies the record's 0x44-byte
//! table 0x1E06670 to sp + 0x60. Init sets the spark-emitter group
//! (+0xA4, `spawn::emitters` / `sprite::sparks`, update lists 0x1E06658 /
//! 0x1E06648 at nodes 0x62 / 0x6A, late window 0x56..0x5C). Update 2 node
//! 0x6A read (+0x55DC) and the attacker's streaks (+0x47C0, 112 counts;
//! alpha on 2..113 the table byte (n - 2) & 15 x 6); 85 an arc (+0xFB8, 16
//! counts) between nodes 0x1E / 0x1F; 88 at the target the fade (+0x47B8),
//! 180 bouncing sprites (+0x1298, delayed s / 18, each 4 x the delay
//! higher), a flash (+0x47F8), three panels (+0x5474, `panel::panels`)
//! and a shake (+0x40); 88..96 a spike a update (+0x4844, 0x68 apart).
//! Returns 6 on updates 88..118, 2 on 119, 1 on 78, 8 on 160 (no drawing
//! then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080 (0538257fe1fa),
//! `sparks` 0x1E003A0, `fan_ring` 0x1E00C70, `emitters` 0x1E00F40,
//! `streaks_b` 0x1E01390, `flash_b` 0x1E02190, `drops` 0x1E02C00, `fade`
//! 0x1E03560, `arcs_b` 0x1E03A30, `spikes` 0x1E04B50, `panels` 0x1E05630
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const ARCS: u32 = D + 0xFB8;
const SPRITES: u32 = D + 0x1298;
const FADE: u32 = D + 0x47B8;
const STREAKS: u32 = D + 0x47C0;
const FLASH: u32 = D + 0x47F8;
const SPIKES: u32 = D + 0x4844;
const PANELS: u32 = D + 0x5474;
const BYTES: u32 = D + 0x55EC;
const TABLE: u32 = 0x01E0_6670;

/// The entry 0x1E05B40 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    for k in (0..0x40u32).step_by(8) {
        let v = fx.mem.read_u64(TABLE + k);
        fx.mem.write_u64(sp + 0x60 + k, v);
    }
    let v = fx.mem.read_u32(TABLE + 0x40);
    fx.mem.write_u32(sp + 0xA0, v);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x56F0).
    fx.bzero(D, 0x56F0);
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
    for (at, node, clear) in [(0xCu32, 0x6A, 1), (0x10, 0x62, 0), (0x14, 0x1E, 0), (0x18, 0x1F, 0), (0x1C, 1, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The emitter group (+0xA4).
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xCC, b);
    fx.mem.write_u8(D + 0xD7, b);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_6658);
    fx.mem.write_u32(D + 0xB0, 0x01E0_6648);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x20);
    fx.mem.write_u32(D + 0xA8, D + 0x7E8);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    fx.mem.write_u16(D + 0xD8, 0x56);
    fx.mem.write_u16(D + 0xDA, 0x5C);
    fx.mem.write_u32(D + 0xDC, 0x4334_0000);
    fx.mem.write_u32(D + 0xE0, 0x42F0_0000);
    fx.mem.write_u32(D + 0xE4, 0x40E0_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x9A);
    // The entry's s0 = D, s1 = 3 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5D10, [Some(D), Some(3), Some(param as u32)]);
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

/// Update 2 [F 0x1E05D6C]: node 0x6A read and the attacker's streaks.
fn streaks(fx: &mut Fx) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, s, D + 0x55DC, u64::from(D), Some(u64::from(OT)), 0x01E0_5D7C);
    let r = STREAKS;
    set_counter4(fx, r, 0x70, 0, 0xFF);
    for (o, v) in [(4u32, 1u8), (9, 2), (0xA, 0xFF), (8, 0x1E), (0xB, 5)] {
        fx.mem.write_u8(r + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(r + 0xC, h);
    for (o, v) in [(0x16u32, 0xFFu8), (0x15, 0xC0), (0x14, 0x80), (0x17, 0), (0x12, 0xFF), (0x11, 0xC0), (0x10, 0x80), (0x13, 0xFF), (5, 1)] {
        fx.mem.write_u8(r + o, v);
    }
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(r + 6, b);
}

/// Update 85 [F 0x1E05DFC]: the arc between the nodes D[0x14] / D[0x18].
fn arc(fx: &mut Fx) {
    set_counter4(fx, ARCS, 0x10, 0, 0xFF);
    fx.mem.write_u8(ARCS + 4, 1);
    fx.mem.write_u32(ARCS + 8, D + 0x1018);
    fx.mem.write_u8(ARCS + 0x30, 0x32);
    fx.mem.write_u8(ARCS + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(ARCS + 0x32, b);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(ARCS + 0x33, b);
    fx.mem.write_u32(ARCS + 0x38, 0x3F80_0000);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0xC0), (0x4A, 0x80), (0x49, 0x40), (0x48, 0)] {
        fx.mem.write_u8(ARCS + o, v);
    }
    let mut k = 0u32;
    while (k as i32) < fx.mem.u8(ARCS + 0x31) + 1 {
        fx.mem.write_u16(ARCS + 0xC + 2 * k, 0xFFFF);
        fx.mem.write_u16(ARCS + 0x1E + 2 * k, 0xFFFF);
        k += 1;
    }
}

/// Update 88 [F 0x1E05EB0]: the strike at the target (sp + 0xB0).
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 0xA, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    // 180 sprites, delayed s / 18.
    for s4 in 0..0xB4i32 {
        let i = first_free(fx, SPRITES, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let count = ((rand4(fx, 0x01E0_5EF0) & 0xF) + 0x20) & 0xFF;
        let r = SPRITES + 0x44 * i as u32;
        let p = i64::from(0x38E3_8E39i32) * i64::from(s4);
        let hi = (p >> 32) as i32;
        fx.set_hilo(p as i32, hi);
        let delay = (hi >> 2) + ((s4 as u32) >> 31) as i32;
        set_counter4(fx, r, count, delay & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0x42u8), (8, 1), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        let v = (rand4(fx, 0x01E0_5F5C) & 3) + 2;
        fx.mem.set8(r + 9, v);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u32(r + 0x10, 0x42A0_0000);
        fx.mem.write_u32(r + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(r + 0x18, 0xBDA3_D70A);
        let y = fx.mem.read_u32(D + 0x55E0);
        fx.mem.write_u32(r + 0x1C, y);
        let x = fx.mem.read_u32(sp + 0xB0);
        fx.mem.write_u32(r + 0x20, x);
        let h = ee::add(fx.mem.f(sp + 0xB4), ee::mul(4.0, ee::from_i32(delay)));
        fx.mem.setf(r + 0x24, h);
        let z = fx.mem.read_u32(sp + 0xB8);
        fx.mem.write_u32(r + 0x28, z);
        let f = jit(fx, 0x01E0_5FE0, 0x50, 0x28);
        fx.mem.setf(r + 0x2C, f);
        let f = ee::add(f32::from_bits(0x3F66_6666), jit(fx, 0x01E0_601C, 0x64, 0x32));
        fx.mem.setf(r + 0x30, f);
        let f = jit(fx, 0x01E0_6064, 0x50, 0x28);
        fx.mem.setf(r + 0x34, f);
    }
    // The flash (+0x47F8).
    let a = FLASH;
    set_counter4(fx, a, 0x20, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x60), (9, 0x10), (8, 0xC0)] {
        fx.mem.write_u8(a + o, v);
    }
    fx.mem.write_u32(a + 0x14, 0);
    fx.mem.write_u32(a + 0x10, 0x4416_0000);
    copy_v4(fx, sp + 0xB0, a + 0x1C);
    // Three panels (+0x5474, 0x3C apart).
    for s3 in 0..3i32 {
        let i = first_free(fx, PANELS, 0x3C, 6);
        if i < 0 {
            continue;
        }
        let r = PANELS + 0x3C * i as u32;
        set_counter4(fx, r, 0x14, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 2), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0xB0, r + 0x2C);
        fx.mem.write_u32(r + 0x10, 0x4000_0000);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        fx.mem.write_u32(r + 0x18, if s3 == 1 { 0x4090_0000 } else { 0x4060_0000 });
        let x = ee::add(fx.mem.f(r + 0x2C), ee::add(-1.5, ee::mul(1.5, ee::from_i32(s3))));
        fx.mem.setf(r + 0x2C, x);
    }
    // The camera shake (+0x40) at the target.
    set_counter4(fx, D + 0x40, 0x20, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x4000_0000);
    fx.mem.write_u32(D + 0x58, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    copy_v4(fx, sp + 0xB0, D + 0x60);
}

/// Updates 88..96 [F 0x1E0631C]: a spike at the target.
fn spike(fx: &mut Fx, sp: u32) {
    let i = first_free(fx, SPIKES, 0x68, 0x1E);
    if i < 0 {
        return;
    }
    let r = SPIKES + 0x68 * i as u32;
    set_counter4(fx, r, 0x12, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4080_0000);
    fx.mem.write_u32(r + 0x1C, 0x4234_0000);
    for (o, v) in [(8u32, 0x12u8), (5, 0), (0xA, 1), (0xB, 2), (0xC, 6)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = rand4(fx, 0x01E0_63A0);
    fx.mem.set8(r + 9, x);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x60)] {
        fx.mem.write_u8(r + o, v);
    }
    for (o, ra) in [(0x58u32, 0x01E0_63E8u32), (0x5C, 0x01E0_63FC), (0x60, 0x01E0_6410)] {
        let v = rand4(fx, ra) & 0x7F;
        fx.mem.set32(r + o, v);
    }
    for o in [0x50u32, 0x4C, 0x48] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, sp + 0xB0, r + 0x28);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xB0, u64::from(D), 0x01E0_5D30);
    match fx.mem.u16(D) {
        0xA0 => return 8,
        0x58 => strike(fx, sp),
        0x55 => arc(fx),
        2 => streaks(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0x72).contains(&n) {
        let b = fx.mem.u8(sp + 0x60 + 4 * ((n - 2) & 0xF) as u32);
        fx.mem.set8(STREAKS + 0x13, b * 6);
    }
    let n = fx.mem.u16(D);
    if (0x58..0x61).contains(&n) {
        spike(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_645C, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6468, [Some(u64::from(D)), Some(u64::from(OT))]);
    let n = fx.mem.u16(D);
    blib::spawn::emitters(fx, 0x01E0_0F40, D + 0xA4, n);
    blib::sprite::sparks(fx, D + 0xE8, OT, 0x20);
    blib::arc::arcs_b(fx, 0x01E0_3A30, ARCS, OT, 1);
    blib::star::flash_b(fx, 0x01E0_2190, FLASH, OT, 1);
    blib::spike::spikes(fx, 0x01E0_4B50, SPIKES, OT, 0x1E);
    blib::drop::drops(fx, SPRITES, OT, 0xC8);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the runner's s6 (0) at +0x70 and ra at
    // +0x80 (as b381: the streaks' stale vertex-1 XYZ).
    fx.saved(sp - 0xF0, &[(0x70, 0), (0x80, 0x01E0_64D0)]);
    blib::radiate::streaks_with(fx, 0x01E0_1390, STREAKS, OT, 1, &blib::radiate::STREAKS_B);
    blib::panel::panels(fx, 0x01E0_5630, PANELS, OT, 6);
    let n = fx.mem.u16(D);
    if n < 0x58 {
        if n == 0x4E {
            1
        } else {
            0
        }
    } else if n < 0x77 {
        6
    } else if n == 0x77 {
        2
    } else {
        0
    }
}
