//! BMODULE b457 (card 544), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E063E0, data area a0 = 0x1ED0000, 0x7DF0 bytes)
//! [F]: s6 = D, the OT (0x206E40) in s0, s1 the
//! parameter; every call copies the 0x44-byte rodata table 0x1E07B60 to
//! sp + 0xB0 (the streaks' alpha wave). Init: the handles (+4 / +8), the
//! attacker's rotation (+0x2C), textures (4, 0xF) and (5, 0..3), nodes 1
//! (cleared, +0xC) / 0xB / 0x3D (+0x10 / +0x14), the stage light ramp
//! (+0xA0: 2 / 0x158) and the random bytes (+0x7C08). Every update the
//! target's position (sp + 0x100). Update 0x28 a shake and the streaks
//! (+0xA4, `radiate::streaks_b`) on node +0x10, their alpha from the
//! wave while they run; 0x2E eight rings (+0x3E5C, `band::bands_b`)
//! staggered at node +0xC; 0x32 two tubes (+0x7D38, `segtube::tubes`)
//! rising there to 140 (+0x7D08 the pair of points); 0x14..0x77 a spray
//! (+0x205C, `spray::sprays_b`) a update at the node; 0x68 a flash at the
//! attacker; 0x6A / 0x93 / 0xA2 four strokes (+0x4E18, `dash::strokes`)
//! rising 150 from the attacker / beside the target, with the fade
//! (+0x49DC); 0x6C the fade; 0x6D nine tilted bands (+0x413C,
//! `band::bands_d`) about the target; 0x7A the black fade (+0x49E4);
//! 0xB2..0x147 on even updates two spikes (+0xDC) at the target; 0xBE the
//! fade and a long shake; 0xBE..0x14A a rising stroke, ..0x146 a flash
//! and a spike about the target; 0xC8 the target tinted black and the haze
//! (+0x49EC, `haze::screen`, 0x82 counts), 0x14A the tint back. The stage
//! light: on 2..0x84 0x1F7F10 (0, then 2 (n - 0x64) from 0x64) and
//! 0x1F7EF0 (2 (n - 2) to 0x41, then 0x80), else the ramp. Returns 6 on
//! 0xBE..0x146, 2 on 0x147, 1 on 0xB4, 8 on 0x15E (no drawing then) and
//! at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `streaks_b`
//! 0x1E003A0, `flash_b` 0x1E011A0, `spikes` 0x1E01C10, `strokes`
//! 0x1E026F0, `fade` 0x1E032C0, `fade_black` 0x1E03790, `sprays_b`
//! 0x1E03B00, `bands_d` 0x1E04310, `tubes` 0x1E04B20, `bands_b`
//! 0x1E05490, `haze::screen` 0x1E05BD0 [`crate::blib`].
use crate::b015::unit_tint_saving;
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const STREAKS: u32 = D + 0xA4;
const SPIKES: u32 = D + 0xDC;
const FLASH: u32 = D + 0x193C;
const SPRAYS: u32 = D + 0x205C;
const RINGS: u32 = D + 0x3E5C;
const BANDS: u32 = D + 0x413C;
const FADE: u32 = D + 0x49DC;
const BLACK: u32 = D + 0x49E4;
const HAZE: u32 = D + 0x49EC;
const STROKES: u32 = D + 0x4E18;
const PATHS: u32 = D + 0x51D8;
const BYTES: u32 = D + 0x7C08;
const TUBE_PTS: u32 = D + 0x7D08;
const TUBES: u32 = D + 0x7D38;
/// The light byte 0x1F7EF0 sets.
const LIGHT2: u32 = 0x002D_F5B0;
/// The rodata alpha wave copied to sp + 0xB0.
const WAVE: u32 = 0x01E0_7B60;

/// The entry 0x1E063E0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x180);
    for k in (0..0x40u32).step_by(8) {
        let v = fx.mem.read_u64(WAVE + k);
        fx.mem.write_u64(sp + 0xB0 + k, v);
    }
    let w = fx.mem.read_u32(WAVE + 0x40);
    fx.mem.write_u32(sp + 0xF0, w);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x180);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x7DF0).
    fx.bzero(D, 0x7DF0);
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
    for (k, (node, clear)) in [(1, 1), (0xB, 0), (0x3D, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x158);
    // The entry's s0 = 4 (the texture loop's), s1 the parameter, s2 the
    // runner's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_652C, [Some(4), Some(param as u32), None]);
    0
}

/// rand() >> 4 with the entry's s0 (the OT) saved in rand's frame.
fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, u64::from(OT));
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand() >> 4
}

/// (200 ((rand >> 4) % m) - off) / 100.
fn jit(fx: &mut Fx, ra: u32, m: i32, off: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(200 * r - off), 100.0)
}

/// ((rand >> 4) % m - off) / 100.
fn jit1(fx: &mut Fx, ra: u32, m: i32, off: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - off), 100.0)
}

fn set(fx: &mut Fx, r: u32, bytes: &[(u32, u8)], words: &[(u32, u32)]) {
    for &(o, v) in bytes {
        fx.mem.write_u8(r + o, v);
    }
    for &(o, v) in words {
        fx.mem.write_u32(r + o, v);
    }
}

fn node(fx: &mut Fx, out: u32, s1: u64, ra: u32) {
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, slot, out, u64::from(OT), Some(s1), ra);
}

fn tint(fx: &mut Fx, sp: u32, c: u8, s1: u64, ra: u32) {
    for k in 0..4 {
        fx.mem.write_u8(sp + 0x17C + k, c);
    }
    let t = fx.mem.i32(D + 8);
    unit_tint_saving(fx, t, sp + 0x17C, [u64::from(OT), s1], ra);
}

/// A stroke record `i` along the path just jagged (the spec at sp + 0x150).
fn stroke_fields(fx: &mut Fx, sp: u32, i: i32, path: u32) {
    let r = STROKES + ((i as u32) << 5);
    fx.mem.write_u8(r + 4, 0x86);
    let n = fx.mem.u8(sp + 0x150) - 1;
    fx.mem.set8(r + 8, n);
    fx.mem.write_u32(r + 0x14, 0x3E4C_CCCD);
    set(fx, r, &[(9, 1), (0xA, 0), (0xB, 2), (0xC, 7), (0xD, 3), (0xE, 2), (0x18, 0x60), (0x19, 0x60), (0x1A, 0x60), (0x1B, 0x20)], &[(0x1C, path)]);
}

/// The 0x102A50 spec at sp + 0x150: `n` points from sp + 0x120 to sp +
/// 0x130 into the path of stroke `i`. Returns the path.
fn jag_path(fx: &mut Fx, sp: u32, i: i32, n: i32) -> u32 {
    let path = PATHS + 0x168 * i as u32;
    fx.mem.set32(sp + 0x150, n);
    fx.mem.write_u32(sp + 0x154, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x170, path);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x120 + 4 * c);
        fx.mem.write_u32(sp + 0x158 + 4 * c, w);
    }
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x130 + 4 * c);
        fx.mem.write_u32(sp + 0x164 + 4 * c, w);
    }
    blib::bolt::jag(fx, sp + 0x150);
    path
}

/// Updates 0x6A / 0x93 / 0xA2 [F 0x1E06A0C]: four strokes rising 150 from
/// sp + 0x110 (the attacker's point, or beside the target) and the fade.
/// Returns s1.
fn strokes(fx: &mut Fx, sp: u32, n: i32) -> u64 {
    match n {
        0x6A => {
            let h = fx.mem.i32(D + 4);
            fx.unit_point(1, h, sp + 0x110);
        }
        _ => {
            let near = fx.mem.read_u32(D + 0x30) == 0x8000;
            let x = if (n == 0x93) == near { 0xC220_0000 } else { 0x4220_0000 };
            fx.mem.write_u32(sp + 0x110, x);
            fx.mem.write_u32(sp + 0x114, 0x41F0_0000);
            let k = if n == 0x93 { 0x3F4C_CCCD } else { 0x3FCC_CCCD };
            let z = ee::mul(f32::from_bits(k), fx.mem.f(sp + 0x108));
            fx.mem.setf(sp + 0x118, z);
            set_counter4(fx, FADE, 8, 0, 0xFF);
            fx.mem.write_u8(FADE + 4, 0x21);
        }
    }
    let mut s1 = 0u64;
    for s4 in 0..4i32 {
        copy_v4(fx, sp + 0x110, sp + 0x130);
        copy_v4(fx, sp + 0x130, sp + 0x120);
        let y = ee::add(fx.mem.f(sp + 0x124), 150.0);
        fx.mem.setf(sp + 0x124, y);
        let (f20, f21, f22) = match s4 {
            3 => (30.0, 50.0, 20.0),
            2 => (50.0, 90.0, -5.0),
            1 => (-10.0, 60.0, 30.0),
            _ => (-30.0, 70.0, -10.0),
        };
        s1 = u64::from(sp + 0x130);
        let v = ee::add(fx.mem.f(sp + 0x134), f21);
        fx.mem.setf(sp + 0x134, v);
        if fx.mem.read_u32(D + 0x30) == 0x8000 {
            let v = ee::add(fx.mem.f(sp + 0x130), f20);
            fx.mem.setf(sp + 0x130, v);
            let v = ee::add(fx.mem.f(sp + 0x138), f22);
            fx.mem.setf(sp + 0x138, v);
        } else {
            let v = ee::sub(fx.mem.f(sp + 0x130), f20);
            fx.mem.setf(sp + 0x130, v);
            let v = ee::sub(fx.mem.f(sp + 0x138), f22);
            fx.mem.setf(sp + 0x138, v);
        }
        let i = first_free(fx, STROKES, 0x20, 0x1E);
        if i < 0 {
            continue;
        }
        let path = jag_path(fx, sp, i, 0x14);
        let r = STROKES + ((i as u32) << 5);
        s1 = u64::from(D + ((i as u32) << 5));
        set_counter4(fx, r, 0x14, s4 & 0xFF, 0xFF);
        stroke_fields(fx, sp, i, path);
        fx.mem.write_u8(r + 0xF, 1);
    }
    s1
}

/// Update 0x6D [F 0x1E06DC0]: nine tilted bands about the target.
fn bands(fx: &mut Fx) {
    for s3 in 0..9i32 {
        let r = BANDS + 0x5C * s3 as u32;
        let s2 = 8 * s3;
        set_counter4(fx, r, 0xFF, 0, 0xFF);
        set(fx, r, &[(4, 0x80), (0xC, 0x30), (8, 2), (9, 1), (0xA, 5), (0xE, 1)], &[]);
        fx.mem.set8(r + 7, (s3 >> 1) + 0xC);
        fx.mem.set8(r + 0xB, (s3 + 1) & 3);
        fx.mem.setf(r + 0x10, ee::mul(25.0, ee::from_i32(s3 + 1)));
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.setf(r + 0x18, ee::add(10.0, ee::from_i32(s2)));
        fx.mem.setf(r + 0x1C, ee::from_i32(s2));
        set(fx, r, &[(0x24, 0x60), (0x25, 0x60), (0x26, 0x60), (0x28, 0x60), (0x29, 0x60), (0x2A, 0x60), (0x27, 0x80), (0x2B, 0x10)], &[(0x2C, 0)]);
        fx.mem.setf(r + 0x30, ee::sub(180.0, ee::mul(3.0, ee::from_i32(s3))));
        fx.mem.write_u32(r + 0x34, 0);
        fx.mem.set32(r + 0x50, 0x30);
        if s3 & 1 != 0 {
            let v = ee::mul(ee::from_i32(0x30), -1.0);
            fx.fix_residue(v, 0x01E0_6EDC);
            fx.mem.set32(r + 0x50, ee::fixsfsi(v));
        }
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut s1 = param as i64 as u64;
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x100, u64::from(OT), 0x01E0_654C);
    let n = fx.mem.u16(D);
    match n {
        0x15E => return 8,
        0x14A => tint(fx, sp, 0x80, s1, 0x01E0_7048),
        0xC8 => {
            tint(fx, sp, 0, s1, 0x01E0_6FCC);
            set_counter4(fx, HAZE, 0x82, 0, 0);
            set(fx, HAZE, &[(0xC, 1), (0xD, 0xCF), (0xE, 0x10), (0x12, 1), (0x10, 3), (0x11, 0)], &[]);
            let t = fx.mem.read_u32(D + 8);
            fx.mem.write_u32(HAZE + 0x14, t);
            set(fx, HAZE, &[(0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x80)], &[]);
        }
        0xBE => {
            set_counter4(fx, FADE, 0x18, 0, 0xFF);
            fx.mem.write_u8(FADE + 4, 1);
            set_counter4(fx, SHAKE, 0xF5, 0, 0xFF);
            fx.mem.write_u8(SHAKE + 5, 0);
            fx.mem.write_u8(SHAKE + 4, 8);
            set(fx, SHAKE, &[], &[(0x14, 0x4000_0000), (0x18, 0x3FCC_CCCD), (0x1C, 0x3F80_0000), (0x30, 0), (0x34, 0x42A0_0000), (0x50, 0x40C0_0000)]);
            copy_v4(fx, sp + 0x100, SHAKE + 0x20);
        }
        0x7A => {
            set_counter4(fx, BLACK, 8, 0, 0xFF);
            fx.mem.write_u8(BLACK + 4, 0x82);
        }
        0x6D => {
            bands(fx);
            s1 = u64::from(D + 0x5C * 9);
        }
        0x6C => {
            set_counter4(fx, FADE, 0xA, 0, 0xFF);
            fx.mem.write_u8(FADE + 4, 1);
        }
        0xA2 | 0x93 | 0x6A => s1 = strokes(fx, sp, n),
        0x68 => {
            let i = first_free(fx, FLASH, 0x4C, 0x18);
            if i >= 0 {
                let r = FLASH + 0x4C * i as u32;
                s1 = u64::from(0x4C * i as u32);
                set_counter4(fx, r, 0x30, 0, 0xFF);
                set(fx, r, &[(5, 0), (4, 0x85), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x10), (8, 0x80)], &[(0x14, 0), (0x10, 0x4348_0000), (0x14, 0x41A0_0000)]);
                let h = fx.mem.i32(D + 4);
                fx.unit_point(1, h, r + 0x1C);
                let y = ee::add(fx.mem.f(r + 0x20), 140.0);
                fx.mem.setf(r + 0x20, y);
            }
        }
        0x28 => {
            set_counter4(fx, SHAKE, 0x8C, 0, 0xFF);
            fx.mem.write_u8(SHAKE + 5, 1);
            let b = fx.mem.read_u8(D + 0x14);
            fx.mem.write_u8(SHAKE + 6, b);
            fx.mem.write_u8(SHAKE + 4, 4);
            set(fx, SHAKE, &[], &[(0x14, 0x3FB3_3333), (0x18, 0x3FB3_3333), (0x1C, 0x3FB3_3333), (0x30, 0), (0x34, 0x4220_0000)]);
            copy_v4(fx, sp + 0x100, SHAKE + 0x20);
            fx.mem.write_u32(SHAKE + 0x50, 0x4040_0000);
            let r = STREAKS;
            set_counter4(fx, r, 0x8C, 0, 0xFF);
            set(fx, r, &[(4, 0), (9, 2), (0xA, 0xFF), (8, 0xA), (0xB, 6)], &[]);
            let h = fx.mem.read_u32(D + 4);
            fx.mem.write_u32(r + 0xC, h);
            set(fx, r, &[(0x14, 0x80), (0x15, 0x80), (0x16, 0x80), (0x17, 0x10), (0x10, 0x80), (0x11, 0x80), (0x12, 0x80), (0x13, 0), (5, 1)], &[]);
            let b = fx.mem.read_u8(D + 0x10);
            fx.mem.write_u8(r + 6, b);
        }
        0x32 => {
            node(fx, sp + 0x110, s1, 0x01E0_6724);
            for c in 0..3u32 {
                let w = fx.mem.read_u32(sp + 0x110 + 4 * c);
                fx.mem.write_u32(TUBE_PTS + 0xC + 4 * c, w);
            }
            for c in 0..3u32 {
                let w = fx.mem.read_u32(TUBE_PTS + 0xC + 4 * c);
                fx.mem.write_u32(TUBE_PTS + 4 * c, w);
            }
            let y = ee::sub(fx.mem.f(TUBE_PTS + 4), 18.0);
            fx.mem.setf(TUBE_PTS + 4, y);
            fx.mem.setf(TUBE_PTS + 0x10, y);
            for s2 in 0..2u32 {
                let r = TUBES + 0x2C * s2;
                set_counter4(fx, r, 0x82, 0, 0xFF);
                set(fx, r, &[(7, 0x24), (8, 1), (0xB, 0x40), (9, 3), (0xA, 2)], &[(0xC, TUBE_PTS)]);
                if s2 != 0 {
                    set(fx, r, &[], &[(0x18, 0x3BA3_D70A), (0x1C, 0x3CA3_D70A), (0x10, 0x4000_0000), (0x10, 0x3F80_0000)]);
                } else {
                    set(fx, r, &[], &[(0x18, 0xBBA3_D70A), (0x1C, 0x3D23_D70A), (0x10, 0x4040_0000), (0x10, 0x4000_0000)]);
                }
                set(fx, r, &[(0x28, 0x80), (0x29, 0x80), (0x2A, 0x80), (0x2B, 0x40)], &[(0x20, 0x4140_0000), (0x24, 0)]);
            }
            s1 = u64::from(D + 0x58);
        }
        0x2E => {
            node(fx, sp + 0x110, s1, 0x01E0_6610);
            for k in 0..8i32 {
                let r = RINGS + 0x5C * k as u32;
                set_counter4(fx, r, 0x80, k & 0xFF, 0xFF);
                set(fx, r, &[(4, 0), (7, 0x24), (0xC, 2), (8, 0), (9, 2), (0xA, 0), (0xB, 0)], &[]);
                let f2 = ee::mul(f32::from_bits(0x3F19_999A), ee::from_i32(k));
                fx.mem.setf(r + 0x10, ee::add(12.5, f2));
                fx.mem.setf(r + 0x14, ee::sub(11.5, f2));
                set(fx, r, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x28, 0x80), (0x29, 0x80), (0x2A, 0x80), (0x2B, 0x20), (0x27, 0x20)], &[(0x1C, 0), (0x18, 0), (0x20, 0x3F00_0000)]);
                copy_v4(fx, sp + 0x110, r + 0x2C);
                let y = ee::sub(fx.mem.f(r + 0x30), 18.0);
                fx.mem.setf(r + 0x30, y);
            }
            s1 = 8;
        }
        _ => {}
    }
    if fx.mem.u8(TUBES) > 0 {
        let y = fx.mem.f(TUBE_PTS + 0x10);
        if crate::exe::ee_lt(y, 140.0) {
            fx.mem.setf(TUBE_PTS + 0x10, ee::add(y, 1.5));
        }
    }
    let n = fx.mem.u16(D);
    if (0x14..0x78).contains(&n) {
        s1 = spray(fx, sp, s1);
    }
    let n = fx.mem.u16(D);
    if (0xB2..0x148).contains(&n) && n & 1 == 0 {
        s1 = 0;
        while s1 < 2 {
            spike_pair(fx, sp);
            s1 += 1;
        }
    }
    let n = fx.mem.u16(D);
    if (0xBE..0x14B).contains(&n) {
        s1 = rising_stroke(fx, sp);
    }
    let n = fx.mem.u16(D);
    if (0xBE..0x147).contains(&n) {
        s1 = sparks(fx, sp, s1);
    }
    if fx.mem.u8(STREAKS) > 0 && fx.mem.u8(STREAKS + 7) == 0 {
        let k = ((fx.mem.u16(D) - 2) & 0xF) as u32;
        let a = fx.mem.u8(sp + 0xB0 + 4 * k) << 2;
        fx.mem.set8(STREAKS + 0x13, a);
    }
    let n = fx.mem.u16(D);
    if (2..0x85).contains(&n) {
        if n < 0x64 {
            fx.set_stage_light(0);
        } else {
            fx.set_stage_light(((n - 0x64) << 1) & 0xFF);
        }
        let n = fx.mem.u16(D);
        let l = if n < 0x42 { ((n - 2) << 1) & 0xFF } else { 0x80 };
        fx.mem.write_u8(LIGHT2, l as u8);
    } else {
        light_ramp(fx, D + 0xA0, n, 0x01E0_7980, [Some(u64::from(OT)), Some(s1)]);
    }
    shake_saving(fx, SHAKE, u64::from(OT), 0x01E0_798C, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let ot = OT;
    blib::fade::fade(fx, FADE, ot);
    blib::fade::fade_black(fx, BLACK, ot);
    // fade / fade_black's prologues save the entry's s6 (D) and ra in
    // their frames (0xF0 / 0xE0) where the streaks' first TEXFLUSH reads a0
    // (run b457: vertex 1's stale XYZ low word); s5 [H] not modelled.
    let f = fx.sp - 0xE0;
    fx.saved(f, &[(0x60, u64::from(D)), (0x70, 0x01E0_79A4)]);
    let (_, spike_a3) = blib::spike::spikes_a3(fx, 0x01E0_1C10, SPIKES, ot, 0x3C);
    blib::radiate::streaks_b(fx, 0x01E0_03A0, STREAKS, ot, 1);
    blib::spray::sprays_b(fx, 0x01E0_3B00, SPRAYS, ot, 0x50);
    let (_, band_a3) = blib::band::bands_d_a3(fx, 0x01E0_4310, BANDS, ot, 0x18);
    let (_, tube_a3) = blib::segtube::tubes_a3(fx, 0x01E0_4B20, TUBES, ot, 4);
    blib::band::bands_b(fx, 0x01E0_5490, RINGS, ot, 8);
    let (_, flash_a3) = blib::star::flash_b_a3(fx, 0x01E0_11A0, FLASH, ot, 0x18);
    let (_, stroke_a3) = blib::dash::strokes_a3(fx, 0x01E0_26F0, STROKES, ot, 0x1E);
    // [H] the a3 the haze reports with its first TEXFLUSH: the strokes',
    // the flashes', the tubes', the bands' (the rings / sprays / streaks
    // not chained), else the spikes'.
    let a3 = stroke_a3.or(flash_a3).or(tube_a3).or(band_a3).or(spike_a3).unwrap_or(0);
    blib::haze::screen(fx, 0x01E0_5BD0, HAZE, ot, 1, a3);
    let n = fx.mem.u16(D);
    if (0xBE..0x147).contains(&n) {
        6
    } else if n == 0x147 {
        2
    } else if n == 0xB4 {
        1
    } else {
        0
    }
}

/// Updates 0x14..0x77 [F 0x1E070B8]: a spray at node +0xC. Returns s1.
fn spray(fx: &mut Fx, sp: u32, s1: u64) -> u64 {
    node(fx, sp + 0x110, s1, 0x01E0_70C8);
    let i = first_free(fx, SPRAYS, 0x60, 0x50);
    if i < 0 {
        return s1;
    }
    let r = SPRAYS + 0x60 * i as u32;
    set_counter4(fx, r, 0x40, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 2);
    let v = rand4(fx, 0x01E0_7118);
    fx.mem.set8(r + 9, v);
    fx.mem.write_u8(r + 0xA, 0);
    let v = rand4(fx, 0x01E0_712C) & 7;
    fx.mem.set8(r + 0xB, v);
    set(fx, r, &[(0xC, 0), (0xD, 2)], &[(0x10, BYTES), (0x24, 0)]);
    set(fx, r, &[(0x28, 0x80), (0x29, 0x80), (0x2A, 0x80), (0x2B, 0x60)], &[]);
    let x = fx.mem.read_u32(sp + 0x110);
    fx.mem.write_u32(r + 0x3C, x);
    let y = ee::sub(fx.mem.f(sp + 0x114), 18.0);
    fx.mem.setf(r + 0x40, y);
    let z = fx.mem.read_u32(sp + 0x118);
    fx.mem.write_u32(r + 0x44, z);
    fx.mem.write_u8(r + 8, 6);
    fx.mem.write_u32(r + 0x1C, 0x41A0_0000);
    let v = rand4(fx, 0x01E0_71A4) % 2;
    fx.mem.setf(r + 0x14, ee::add(12.0, ee::div(ee::from_i32(200 * v - 200), 100.0)));
    fx.mem.write_u32(r + 0x20, 0x41C8_0000);
    let v = ee::add(0.5, jit1(fx, 0x01E0_720C, 0x50, 0x28));
    fx.mem.setf(r + 0x4C, v);
    u64::from(0x60 * i as u32)
}

/// Even updates 0xB2..0x147 [F 0x1E07278]: a spike at the target.
fn spike_pair(fx: &mut Fx, sp: u32) {
    let j = first_free(fx, SPIKES, 0x68, 0x3C);
    if j < 0 {
        return;
    }
    let r = SPIKES + 0x68 * j as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    set(fx, r, &[(4, 5), (8, 0x18), (5, 0), (0xA, 0), (0xB, 2), (0xC, 3)], &[(0x14, 0x4140_0000), (0x1C, 0x42C8_0000)]);
    let v = rand4(fx, 0x01E0_72F8);
    fx.mem.set8(r + 9, v);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    set(fx, r, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x10), (0x27, 0x50)], &[]);
    for (o, ra) in [(0x58u32, 0x01E0_7340), (0x5C, 0x01E0_7350), (0x60, 0x01E0_7360)] {
        let v = rand4(fx, ra) & 0x7F;
        fx.mem.set32(r + o, v);
    }
    copy_v4(fx, sp + 0x100, r + 0x28);
    let y = ee::add(fx.mem.f(r + 0x2C), 80.0);
    fx.mem.setf(r + 0x2C, y);
}

/// Updates 0xBE..0x14A [F 0x1E073D4]: a stroke from 140 above the target
/// to a jittered point 50 below. Returns s1.
fn rising_stroke(fx: &mut Fx, sp: u32) -> u64 {
    copy_v4(fx, sp + 0x100, sp + 0x130);
    copy_v4(fx, sp + 0x130, sp + 0x120);
    let y = ee::add(fx.mem.f(sp + 0x124), 140.0);
    fx.mem.setf(sp + 0x124, y);
    let y = ee::sub(fx.mem.f(sp + 0x134), 50.0);
    fx.mem.setf(sp + 0x134, y);
    let x = ee::add(fx.mem.f(sp + 0x130), jit(fx, 0x01E0_7458, 0x32, 0x1388));
    fx.mem.setf(sp + 0x130, x);
    let z = ee::add(fx.mem.f(sp + 0x138), jit(fx, 0x01E0_74B0, 0x1E, 0xBB8));
    fx.mem.setf(sp + 0x138, z);
    let s1 = u64::from(sp + 0x130);
    let i = first_free(fx, STROKES, 0x20, 0x1E);
    if i < 0 {
        return s1;
    }
    let path = jag_path(fx, sp, i, 0x19);
    let count = (rand4(fx, 0x01E0_758C) & 7) + 9;
    let r = STROKES + ((i as u32) << 5);
    set_counter4(fx, r, count & 0xFF, 0, 0xFF);
    stroke_fields(fx, sp, i, path);
    u64::from((i as u32) << 5)
}

/// Updates 0xBE..0x146 [F 0x1E07638]: a flash and a spike at a jittered
/// point about the target. Returns s1.
fn sparks(fx: &mut Fx, sp: u32, s1_in: u64) -> u64 {
    let mut s1 = s1_in;
    let x = ee::add(fx.mem.f(sp + 0x100), jit1(fx, 0x01E0_7644, 0xBB8, 0x5DC));
    fx.mem.setf(sp + 0x140, x);
    let y = ee::add(fx.mem.f(sp + 0x104), jit1(fx, 0x01E0_7684, 0xBB8, 0x5DC));
    fx.mem.setf(sp + 0x144, y);
    let z = ee::add(fx.mem.f(sp + 0x108), jit1(fx, 0x01E0_76C4, 0x7D0, 0x3E8));
    fx.mem.setf(sp + 0x148, z);
    let i = first_free(fx, FLASH, 0x4C, 0x10);
    if i >= 0 {
        let r = FLASH + 0x4C * i as u32;
        s1 = u64::from(D + 0x4C * i as u32);
        set_counter4(fx, r, 0xC, 0, 0xFF);
        set(fx, r, &[(5, 0), (4, 0x81), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x10), (8, 0x80)], &[(0x14, 0), (0x10, 0x43C8_0000)]);
        copy_v4(fx, sp + 0x140, r + 0x1C);
    }
    let j = first_free(fx, SPIKES, 0x68, 0x3C);
    if j >= 0 {
        let r = SPIKES + 0x68 * j as u32;
        s1 = u64::from(D + 0x68 * j as u32);
        fx.mem.write_u8(r + 4, 0x85);
        set_counter4(fx, r, 0xC, 0, 0xFF);
        set(fx, r, &[(0x27, 0x40), (0x23, 0x10), (5, 0), (0xA, 0), (0xB, 2), (0xC, 4), (8, 0x1E)], &[(0x14, 0x40B0_0000), (0x1C, 0x4270_0000)]);
        let v = rand4(fx, 0x01E0_7848);
        fx.mem.set8(r + 9, v);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        set(fx, r, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x24, 0x80)], &[]);
        copy_v4(fx, sp + 0x140, r + 0x28);
    }
    s1
}
