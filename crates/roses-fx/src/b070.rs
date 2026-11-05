//! BMODULE b070 (card 551), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E046F0, data area a0 = 0x1ED0000, 0x9980 bytes)
//! [F]: s6 =
//! D, s0 = the OT (0x206E40), s1 the parameter then loop values. The
//! entry first copies the record's 17-word pulse table (0x1E055F0) to sp +
//! 0xA0. Update 2 the attacker's streaks (+0x2024, `radiate::streaks_b`,
//! 255 counts, node 7); every update two bouncing sprites (+0x48BC, 0x44
//! apart) from node 8; 170 ten bolts from node 1 to the target (three
//! jagged 0x102A50, seven arcs 0x102F20), each five segments (+0x233C, 0x40
//! apart, `bolt::bolts_b`), the fade (+0x986C) and a shake (+0x40) to
//! update 334; on updates 170..321 three bolt segments and a flash
//! (+0x1904, 0x4C apart) at a jittered target point; 310 the streaks dim.
//! The streaks' far alpha pulses by the table. Returns 6 on updates
//! 170..330, 2 on 167, 1 on 160, 8 on 440 (no drawing then) and at once
//! for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080 (0538257fe1fa),
//! `spikes` 0x1E003A0, `flash_b` 0x1E00E80, `streaks_b` 0x1E018F0,
//! `fade` 0x1E026F0, `drops` 0x1E02BC0, `bands_b` 0x1E03520, `bolts_b`
//! 0x1E03C60 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PULSE: u32 = 0x01E0_55F0;
const STREAKS: u32 = D + 0x2024;
const SEGS: u32 = D + 0x233C;
const SPRITES: u32 = D + 0x48BC;
const FLASH: u32 = D + 0x1904;

/// The entry 0x1E046F0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x250);
    for k in (0..0x40u32).step_by(8) {
        let v = fx.mem.read_u64(PULSE + k);
        fx.mem.write_u64(sp + 0xA0 + k, v);
    }
    let v = fx.mem.read_u32(PULSE + 0x40);
    fx.mem.write_u32(sp + 0xE0, v);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x250);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x9980).
    fx.bzero(D, 0x9980);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x12, 0), (0x14, 0x2B, 0), (0x18, 7, 0), (0x1C, 8, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x1B2);
    // The entry's s0 = 4 (the texture loop's), s1 = the parameter, s2 the
    // runner's 0.
    rand_bytes(fx, D + 0x9874, 0x100, 0x01E0_485C, [Some(4), Some(param as u32), Some(0)]);
    0
}

fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(OT, ra) >> 4
}

/// ((rand >> 4) % m x `k` - `off`) / 100.
fn spread(fx: &mut Fx, ra: u32, m: i32, k: i32, off: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(k * r - off), 100.0)
}

fn colours(fx: &mut Fx, s: u32, far: [u8; 4], near: [u8; 4]) {
    for (o, v) in [(0x16u32, far[0]), (0x15, far[1]), (0x14, far[2]), (0x17, far[3]), (0x12, near[0]), (0x11, near[1]), (0x10, near[2]), (0x13, near[3])] {
        fx.mem.write_u8(s + o, v);
    }
}

/// Update 170 [F 0x1E04938]: the bolts from node 1 (sp + 0x100) to the
/// target (sp + 0xF0). Returns the entry's s1 after.
fn bolts(fx: &mut Fx, sp: u32, s1: u64) -> u64 {
    set_counter4(fx, STREAKS, 0xFF, 0, 0xFF);
    for (o, v) in [(0x16u32, 0x80u8), (0x15, 0x80), (0x14, 0xC0), (0x17, 0x20), (0x12, 0x80), (0x11, 0x80), (0x10, 0xC0), (0x13, 0xF0)] {
        fx.mem.write_u8(STREAKS + o, v);
    }
    set_counter4(fx, D + 0x986C, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0x9870, 1);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, s, sp + 0x100, u64::from(OT), Some(s1), 0x01E0_49B0);
    let mut k = 0i32;
    while k < 0xA {
        if k < 3 {
            fx.mem.set32(sp + 0x120, 5);
            fx.mem.write_u32(sp + 0x124, 0x3F4C_CCCD);
            fx.mem.write_u32(sp + 0x140, sp + 0x180);
            for c in 0..3u32 {
                let w = fx.mem.read_u32(sp + 0x100 + 4 * c);
                fx.mem.write_u32(sp + 0x128 + 4 * c, w);
                let w = fx.mem.read_u32(sp + 0xF0 + 4 * c);
                fx.mem.write_u32(sp + 0x134 + 4 * c, w);
            }
            blib::bolt::jag(fx, sp + 0x120);
        } else {
            fx.mem.set32(sp + 0x150, 5);
            if k < 7 {
                fx.mem.write_u32(sp + 0x154, 0x4248_0000);
                let a = match k {
                    6 => 0x435C_0000u32,
                    5 => 0x430C_0000,
                    4 => 0x43A0_0000,
                    _ => 0x4220_0000,
                };
                fx.mem.write_u32(sp + 0x158, a);
            } else {
                let f = ee::add(30.0, spread(fx, 0x01E0_4B04, 0xA, 200, 0x3E8));
                fx.mem.setf(sp + 0x154, f);
                let f = spread(fx, 0x01E0_4B5C, 0x168, 200, 0x8CA0);
                fx.mem.setf(sp + 0x158, f);
            }
            fx.mem.write_u32(sp + 0x178, sp + 0x180);
            for c in 0..3u32 {
                let w = fx.mem.read_u32(sp + 0x100 + 4 * c);
                fx.mem.write_u32(sp + 0x160 + 4 * c, w);
                let w = fx.mem.read_u32(sp + 0xF0 + 4 * c);
                fx.mem.write_u32(sp + 0x16C + 4 * c, w);
            }
            crate::b173::arc_points(fx, sp + 0x150);
        }
        let mut p = sp + 0x180;
        let mut full = false;
        for s4 in 0..5i32 {
            let i = first_free(fx, SEGS, 0x40, 0x96);
            if i < 0 {
                full = true;
                break;
            }
            let r = SEGS + 0x40 * i as u32;
            set_counter4(fx, r, 0x8C, s4 & 1, 0xFF);
            fx.mem.write_u8(r + 5, 0);
            fx.mem.write_u8(r + 4, 1);
            for c in 0..3u32 {
                let w = fx.mem.read_u32(p + 4 * c);
                fx.mem.write_u32(r + 0x1C + 4 * c, w);
                let w = fx.mem.read_u32(p + 0xC + 4 * c);
                fx.mem.write_u32(r + 0x28 + 4 * c, w);
            }
            fx.mem.write_u8(r + 8, 6);
            fx.mem.write_u32(r + 0x10, 0x3F33_3333);
            fx.mem.write_u32(r + 0x14, 0x3E4C_CCCD);
            for (o, v) in [(0xBu32, 3u8), (0xD, 3), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)] {
                fx.mem.write_u8(r + o, v);
            }
            p += 0xC;
        }
        let _ = full;
        k += 1;
    }
    let n = fx.mem.u16(D);
    set_counter4(fx, D + 0x40, (0x14E - n) & 0xFF, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 8);
    for o in [0x54u32, 0x58, 0x5C] {
        fx.mem.write_u32(D + o, 0x3FCC_CCCD);
    }
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    copy_v4(fx, sp + 0xF0, D + 0x60);
    0xA
}

/// Every update [F 0x1E04D78]: two sprites from node 8 (sp + 0x100).
/// Returns the entry's s1 after.
fn sprites(fx: &mut Fx, sp: u32, mut s1: u64) -> u64 {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x1C));
    node_position_saving(fx, h, s, sp + 0x100, u64::from(OT), Some(s1), 0x01E0_4D88);
    for s2 in 0..2i32 {
        let i = first_free(fx, SPRITES, 0x44, 0x12C);
        if i < 0 {
            continue;
        }
        s1 = u64::from(D + 0x44 * i as u32);
        let r = SPRITES + 0x44 * i as u32;
        set_counter4(fx, r, 0x20, 0, 0xFF);
        let v = rand4(fx, 0x01E0_4DD0);
        fx.mem.set8(r + 4, (v & 3) << 4);
        if s2 & 1 != 0 {
            fx.mem.write_u8(r + 8, 2);
            for o in [0xFu32, 0xC, 0xD, 0xE] {
                fx.mem.write_u8(r + o, 0x80);
            }
            let f = ee::add(180.0, spread(fx, 0x01E0_4E0C, 0x28, 200, 0xFA0));
            fx.mem.setf(r + 0x10, f);
            fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        } else {
            fx.mem.write_u8(r + 8, 1);
            for (o, v) in [(0xEu32, 0x80u8), (0xD, 0xA0), (0xC, 0xC0), (0xF, 0x40)] {
                fx.mem.write_u8(r + o, v);
            }
            let f = ee::add(20.0, spread(fx, 0x01E0_4E9C, 0xA, 200, 0x3E8));
            fx.mem.setf(r + 0x10, f);
            let f = ee::add(1.0, spread(fx, 0x01E0_4EF4, 0x96, 1, 0x4B));
            fx.mem.setf(r + 0x14, f);
        }
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xA, 0);
        let v = rand4(fx, 0x01E0_4F40);
        fx.mem.set8(r + 9, v & 7);
        fx.mem.write_u32(r + 0x1C, 0);
        fx.mem.write_u32(r + 0x18, 0);
        for (c, ra) in [(0u32, 0x01E0_4F58u32), (1, 0x01E0_4F98), (2, 0x01E0_4FD8)] {
            let f = ee::add(fx.mem.f(sp + 0x100 + 4 * c), spread(fx, ra, 0x3E8, 1, 0x1F4));
            fx.mem.setf(r + 0x20 + 4 * c, f);
        }
        let f = ee::add(f32::from_bits(0x3F19_999A), spread(fx, 0x01E0_5018, 0x32, 1, 0x19));
        fx.mem.setf(r + 0x30, f);
        let f = spread(fx, 0x01E0_5060, 0x14, 1, 0xA);
        fx.mem.setf(r + 0x2C, f);
    }
    s1
}

/// Updates 170..321 [F 0x1E050BC]: three bolt segments and a flash at the
/// target (sp + 0xF0) jittered (sp + 0x110). Returns the entry's s1.
fn strikes(fx: &mut Fx, sp: u32, mut s1: u64) -> u64 {
    for k in (0..16u32).step_by(8) {
        let v = fx.mem.read_u64(sp + 0xF0 + k);
        fx.mem.write_u64(sp + 0x100 + k, v);
    }
    for (c, ra) in [(0u32, 0x01E0_50D0u32), (1, 0x01E0_5114), (2, 0x01E0_5154)] {
        let f = ee::add(fx.mem.f(sp + 0x100 + 4 * c), spread(fx, ra, 0xC80, 1, 0x640));
        fx.mem.setf(sp + 0x110 + 4 * c, f);
    }
    for s5 in 0..3i32 {
        let i = first_free(fx, SEGS, 0x40, 0x96);
        if i < 0 {
            break;
        }
        s1 = u64::from(D + 0x40 * i as u32);
        let r = SEGS + 0x40 * i as u32;
        set_counter4(fx, r, 0xC, s5 & 0xFF, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x110 + 4 * c);
            fx.mem.write_u32(r + 0x28 + 4 * c, w);
            fx.mem.write_u32(r + 0x1C + 4 * c, w);
        }
        for (c, ra) in [(0u32, 0x01E0_5208u32), (1, 0x01E0_5260), (2, 0x01E0_52B8)] {
            let f = ee::add(fx.mem.f(r + 0x28 + 4 * c), spread(fx, ra, 0x18, 200, 0x960));
            fx.mem.setf(r + 0x28 + 4 * c, f);
        }
        fx.mem.write_u8(r + 8, 8);
        fx.mem.write_u32(r + 0x10, 0x3F80_0000);
        fx.mem.write_u32(r + 0x14, 0x3E99_999A);
        for (o, v) in [(0xBu32, 3u8), (0xD, 4), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
    }
    let i = first_free(fx, FLASH, 0x4C, 0x18);
    if i >= 0 {
        s1 = u64::from(0x4C * i as u32);
        let a = FLASH + 0x4C * i as u32;
        set_counter4(fx, a, 0xC, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 4), (7, 9), (0xF, 0xD0), (0xC, 0xD0), (0xD, 0xD0), (0xE, 0xD0), (0xA, 0), (9, 0x60), (8, 0x80)] {
            fx.mem.write_u8(a + o, v);
        }
        fx.mem.write_u32(a + 0x14, 0);
        fx.mem.write_u32(a + 0x10, 0x4348_0000);
        fx.mem.write_u32(a + 0x14, 0x41F0_0000);
        copy_v4(fx, sp + 0x110, a + 0x1C);
        fx.mem.write_u32(a + 0x18, 0x41A0_0000);
    }
    s1
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xF0, u64::from(OT), 0x01E0_487C);
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0x1B8 => return 8,
        0x136 => {
            fx.mem.write_u8(STREAKS + 7, 0);
            colours(fx, STREAKS, [0x30, 0x30, 0x80, 0], [0x30, 0x30, 0x80, 0]);
        }
        0xAA => s1 = bolts(fx, sp, s1),
        2 => {
            set_counter4(fx, STREAKS, 0xFF, 0, 0xFF);
            for (o, v) in [(4u32, 1u8), (9, 2), (0xA, 0xFF), (8, 0xC), (0xB, 4)] {
                fx.mem.write_u8(STREAKS + o, v);
            }
            let h = fx.mem.read_u32(D + 4);
            fx.mem.write_u32(STREAKS + 0xC, h);
            colours(fx, STREAKS, [0x30, 0x30, 0x80, 0], [0x30, 0x30, 0x80, 0]);
            fx.mem.write_u8(STREAKS + 5, 1);
            let b = fx.mem.read_u8(D + 0x18);
            fx.mem.write_u8(STREAKS + 6, b);
        }
        _ => {}
    }
    s1 = sprites(fx, sp, s1);
    let n = fx.mem.u16(D);
    if (0xAA..0x142).contains(&n) {
        s1 = strikes(fx, sp, s1);
    }
    if fx.mem.u8(STREAKS) > 0 && fx.mem.read_u8(STREAKS + 7) == 0 {
        let k = ((fx.mem.u16(D) - 2) & 0xF) as u32;
        let v = fx.mem.read_u8(sp + 0xA0 + 4 * k);
        fx.mem.write_u8(STREAKS + 0x13, v.wrapping_mul(3));
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_5460, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_546C, [Some(u64::from(OT)), Some(s1)]);
    blib::fade::fade(fx, D + 0x986C, OT);
    // fade's 0xF0 frame saves s6 (= D) at +0x70 and ra at +0x80: the
    // streaks' first TEXFLUSH reads them as vertex 1's stale XYZ [run].
    fx.saved(sp - 0xF0, &[(0x70, u64::from(D)), (0x80, 0x01E0_547C)]);
    blib::bolt::bolts_b(fx, 0x01E0_3C60, SEGS, OT, 0x96);
    blib::spike::spikes(fx, 0x01E0_03A0, D + 0xA4, OT, 0x3C);
    blib::star::flash_b(fx, 0x01E0_0E80, FLASH, OT, 0x18);
    blib::radiate::streaks_b(fx, 0x01E0_18F0, STREAKS, OT, 1);
    blib::band::bands_b(fx, 0x01E0_3520, D + 0x205C, OT, 8);
    blib::drop::drops(fx, SPRITES, OT, 0x12C);
    let n = fx.mem.u16(D);
    if (0xAA..0x14B).contains(&n) {
        6
    } else if n == 0xA7 {
        2
    } else if n == 0xA0 {
        1
    } else {
        0
    }
}
