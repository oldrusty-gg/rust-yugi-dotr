//! BMODULE b449 (card 585), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E052F0, data area a0 = 0x1ED0000) [F] (helper B8, 31
//! Oct 2026): b063's family (s3 = D, s0 = the OT, s1 the parameter then
//! loop counters). Node bursts (+0xA4, motes +0xE8, four nodes) every
//! update; on update 16 the attacker radiating from its node 1 (+0x3388,
//! `radiate::streaks`, all eight directions), its far alpha pulsed by the
//! entry's 16-step table (rodata 0x1E05D60, copied to sp + 0x50 each call)
//! while 16 <= n < 116; on 84 the fade (+0x6C74), a ribbon (+0x340C), 200
//! bouncing sprites (+0x3754), four bands (+0x3474, delays 2 k), a wide
//! flash (+0x33C0) and a camera shake at the target. The stage light
//! pulsed by 0x1031C0 (+0xA0). Returns 1 on update 74, 2 on 113, 6 on
//! 84..112, 8 on 190 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `streaks` 0x1E015F0, `flash`
//! 0x1E02410, `ribbon` 0x1E02FC0, `bands` 0x1E03B50, `bounce` 0x1E04340,
//! `fade` 0x1E04DB0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x6C7C;
/// The alpha pulse table (rodata, 0x44 bytes).
const TABLE: u32 = 0x01E0_5D60;

/// The entry 0x1E052F0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    for k in 0..0x11u32 {
        let v = fx.mem.read_u32(TABLE + 4 * k);
        fx.mem.write_u32(sp + 0x50 + 4 * k, v);
    }
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x6D80);
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
    for (at, node, clear) in [(0xCu32, 0x39, 1), (0x10, 0xB, 0), (0x14, 0x2C, 0), (0x18, 0x19, 0), (0x1C, 1, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): specs 0x1E05D38.. (four), motes +0xE8 /
    // +0xE08.
    for (dst, src) in [(0xCCu32, 0xCu32), (0xCD, 0x10), (0xCE, 0x14), (0xD7, 0x14), (0xCF, 0x18)] {
        let b = fx.mem.read_u8(D + src);
        fx.mem.write_u8(D + dst, b);
    }
    for (k, spec) in [0x01E0_5D38u32, 0x01E0_5D40, 0x01E0_5D48, 0x01E0_5D50].into_iter().enumerate() {
        fx.mem.write_u32(D + 0xAC + 4 * k as u32, spec);
    }
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x3C);
    fx.mem.write_u32(D + 0xA8, D + 0xE08);
    fx.mem.write_u8(D + 0xD5, 0xF0);
    fx.mem.write_u8(D + 0xD6, 0);
    fx.mem.write_u16(D + 0xD8, 0x33);
    fx.mem.write_u16(D + 0xDA, 0x3C);
    fx.mem.write_u32(D + 0xDC, 0x4334_0000);
    fx.mem.write_u32(D + 0xE0, 0x4334_0000);
    fx.mem.write_u32(D + 0xE4, 0x40E0_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xB8);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_551C, [Some(OT), Some(3), None]);
    0
}

/// Update 16 [F 0x1E05574]: the radiating unit (+0x3388) from node +0x1C.
fn radiate_start(fx: &mut Fx) {
    let r = D + 0x3388;
    set_counter4(fx, r, 0x4C, 0, 0xFF);
    for (o, v) in [(4u32, 1u8), (9, 2), (0xA, 0xFF), (8, 0x12), (0xB, 9)] {
        fx.mem.write_u8(r + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(r + 0xC, h);
    for (o, v) in [(0x16u32, 0xD0u8), (0x15, 0x90), (0x14, 0x80), (0x17, 0x18), (0x12, 0xD0), (0x11, 0x90), (0x10, 0x80), (0x13, 0x40), (5, 1)] {
        fx.mem.write_u8(r + o, v);
    }
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(r + 6, b);
}

/// rand with the entry's s0 (the OT) from `ra`, (x >> 4) % m.
fn rand_mod(fx: &mut Fx, ra: u32, m: i32) -> i32 {
    let x = fx.rand_from(OT, ra) >> 4;
    fx.div(x, m).1
}

/// Update 84 [F 0x1E05604]: at the target (sp + 0xA0).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0xA0;
    set_counter4(fx, D + 0x6C74, 0xC, 0, 0xFF);
    fx.mem.write_u8(D + 0x6C78, 1);
    let r = D + 0x340C;
    set_counter4(fx, r, 0x1E, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4020_0000);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_5680);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, at, r + 0x28);
    fx.mem.write_u32(D + 0x3464, 0x40);
    fx.mem.write_u32(D + 0x3468, 0x80);
    fx.mem.write_u32(D + 0x346C, 0x100);
    for _ in 0..0xC8 {
        let i = first_free(fx, D + 0x3754, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(OT, 0x01E0_572C) >> 4) & 0xF;
        let b = D + 0x3754 + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, delay, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 1), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(b + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_577C) >> 4;
        fx.mem.set8(b + 9, x & 7);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(b + o, 0x80);
        }
        let h = rand_mod(fx, 0x01E0_57A8, 0x14);
        let f = ee::div(ee::from_i32(h * 200 - 0x7D0), 100.0);
        fx.mem.setf(b + 0x10, ee::add(120.0, f));
        fx.mem.write_u32(b + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(b + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(b + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        let h = rand_mod(fx, 0x01E0_5848, 0xE6);
        fx.mem.setf(b + 0x2C, ee::div(ee::from_i32(h - 0x73), 100.0));
        let h = rand_mod(fx, 0x01E0_5894, 0xE6);
        let f = ee::div(ee::from_i32(h - 0x73), 100.0);
        fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), f));
        let h = rand_mod(fx, 0x01E0_58F0, 0xE6);
        fx.mem.setf(b + 0x34, ee::div(ee::from_i32(h - 0x73), 100.0));
    }
    for k in 0..4u32 {
        let r = D + 0x3474 + 0x5C * k;
        set_counter4(fx, r, 0x20, (2 * k) as i32, 0xFF);
        for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 1), (8, 2), (9, 2), (0xA, 6), (0xB, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x4000_0000);
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0x4000_0000);
        fx.mem.write_u32(r + 0x1C, 0x3F80_0000);
        fx.mem.write_u32(r + 0x20, 0x3F00_0000);
        for (o, v) in [(0x28u32, 0x80u8), (0x29, 0x80), (0x2A, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0x10), (0x2B, 0x40)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        fx.mem.write_u32(r + 0x3C, 0x4000);
    }
    let r = D + 0x33C0;
    set_counter4(fx, r, 0x20, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    for (o, v) in [(0xFu32, 0xF0u8), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x60), (9, 0x10), (8, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0);
    fx.mem.write_u32(r + 0x10, 0x4448_0000);
    copy_v4(fx, at, r + 0x1C);
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xA0, u64::from(OT), 0x01E0_5544);
    // The entry's s1 at the library calls: the parameter or the last
    // loop's counter.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0xBE => return 8,
        0x54 => {
            strike(fx, sp);
            // The band loop's offset 0x5C x 3 (s1 after the 200 bounces).
            s1 = 0x114;
        }
        0x10 => radiate_start(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x10..0x74).contains(&n) {
        let v = fx.mem.u8(sp + 0x50 + 4 * ((n - 0x10) & 0xF) as u32);
        fx.mem.set8(D + 0x339B, v * 5);
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_5B98, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5BA8, [Some(u64::from(OT)), Some(s1)]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x3C);
    blib::radiate::streaks(fx, 0x01E0_15F0, D + 0x3388, OT, 1);
    blib::star::flash(fx, 0x01E0_2410, D + 0x33C0, OT, 1);
    blib::streak::ribbon(fx, 0x01E0_2FC0, D + 0x340C, OT, 1);
    blib::band::bands(fx, 0x01E0_3B50, D + 0x3474, OT, 8);
    blib::sprite::bounce(fx, D + 0x3754, OT, 0xC8);
    blib::fade::fade(fx, D + 0x6C74, OT);
    // fade's saved s6 (the runner's 0) and ra stay below the entry's
    // frame: the next update's first TEXFLUSH reports them as a0 / a1
    // (the streaks' stale vertex XYZ) [run].
    fx.saved(sp - 0xF0, &[(0x70, 0), (0x80, 0x01E0_5C40)]);
    let n = fx.mem.u16(D);
    if (0x54..0x71).contains(&n) {
        6
    } else if n == 0x71 {
        2
    } else if n == 0x4A {
        1
    } else {
        0
    }
}
