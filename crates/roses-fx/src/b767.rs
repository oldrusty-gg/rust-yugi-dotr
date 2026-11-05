//! BMODULE b767 (card 539), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05B80, data area a0 = 0x1ED0000, 0x99A0 bytes)
//! [F]: s6 = D, the OT (0x206E40) in fp, s1 the
//! parameter. Init: the handles (+4 / +8), the attacker's rotation
//! (+0x2C), textures (4, 0xF) and (5, 0..3), nodes 1 (cleared, +0xC) /
//! 0xB / 0x3D / 0x55 (+0x10..+0x18), the stage light ramp (+0xA0: 0x68 /
//! 0x20E) and the random bytes (+0x9890). Every update the target's
//! position (sp + 0xB0). Update 0x69 a long shake and the bolt mask
//! (+0x9990) 3, halved on 0x99 / 0xC4; 0xF0 / 0x104 / 0x120 / 0x13C the
//! black fade (+0x663C); 0xF4 / 0x116 / 0x12C four strokes (+0x6AA0,
//! `dash::strokes`, jagged paths 0x102A50 rising 180 from about the
//! target) with the fade (+0x6634) and a flash 800 wide (+0x1904); 0xF8
//! nine tilted bands (+0x5D94, `band::bands_d`) growing about the
//! target; 0x15E the frame fade window (+0x6644, 0x15E..0x190); 0x184 the
//! fade and a shake; 0x18E the target tinted black and the haze (+0x6674,
//! `haze::screen`, 0x70 counts); 0x1F4 the tint back. From 0x69 to 0x17C
//! on the updates the mask lets through, three bolts (+0x23B4,
//! `bolt::bolts_b`) and a flash at a random turn round node +0xC (rising
//! from 0x16B); 0x184..0x1E8 a stroke rising from the target; 0x184..0x1F0
//! a flash and a spike (+0xA4) about it. Returns 6 on 0x184..0x1F0, 2 on
//! 0x1F1, 1 on 0x17A, 8 on 0x214 (no drawing then) and at once for -2 /
//! -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash_b`
//! 0x1E003A0, `spikes` 0x1E00E10, `bolts_b` 0x1E018F0, `strokes`
//! 0x1E02380, `fade` 0x1E02F50, `fade_black` 0x1E03420, `sprays_b`
//! 0x1E03790 (+0x3CB4 never armed), `bands_d` 0x1E03FA0, `bands_b`
//! 0x1E047B0 (+0x5AB4 never armed), `haze::screen` 0x1E04EF0,
//! `frame_fade_b` 0x1E05700 [`crate::blib`].
use crate::b015::unit_tint_saving;
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const SPIKES: u32 = D + 0xA4;
const FLASH: u32 = D + 0x1904;
const BOLTS: u32 = D + 0x23B4;
const SPRAYS: u32 = D + 0x3CB4;
const BANDS_B: u32 = D + 0x5AB4;
const BANDS: u32 = D + 0x5D94;
const FADE: u32 = D + 0x6634;
const BLACK: u32 = D + 0x663C;
const WINDOW: u32 = D + 0x6644;
const HAZE: u32 = D + 0x6674;
const STROKES: u32 = D + 0x6AA0;
const PATHS: u32 = D + 0x6E60;
const BYTES: u32 = D + 0x9890;
const MASK: u32 = D + 0x9990;
/// [H] the runner's s0 at the entry.
const RUNNER_S0: u64 = 0;

/// The entry 0x1E05B80 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1A0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x1A0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x99A0).
    fx.bzero(D, 0x99A0);
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
    for (k, (node, clear)) in [(1, 1), (0xB, 0), (0x3D, 0), (0x55, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    fx.mem.set16(D + 0xA0, 0x68);
    fx.mem.set16(D + 0xA2, 0x20E);
    // The entry's s0 = 4 (the texture loop's), s1 the parameter, s2 the
    // runner's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5CB4, [Some(4), Some(param as u32), None]);
    0
}

/// rand() >> 4 with the entry's s0 (64 bits) saved in rand's frame.
fn rand4(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand() >> 4
}

/// (200 ((rand >> 4) % m) - off) / 100.
fn jit(fx: &mut Fx, s0: u64, ra: u32, m: i32, off: i32) -> f32 {
    let v = rand4(fx, s0, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(200 * r - off), 100.0)
}

/// ((rand >> 4) % m - off) / 100.
fn jit1(fx: &mut Fx, s0: u64, ra: u32, m: i32, off: i32) -> f32 {
    let v = rand4(fx, s0, ra);
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

/// 0x20C0B0(out, 0, rot, 0): the rotation alone, its 0x70 frame (the
/// caller's s0..s3 +0x20.., ra +0x60) left below the caller's sp.
fn rot_into(fx: &mut Fx, out: u32, rot: u32, s: [u64; 4], ra: u32) {
    let r = [fx.mem.i32(rot), fx.mem.i32(rot + 4), fx.mem.i32(rot + 8)];
    let m = fx.trs(None, Some(r), None);
    fx.write_m4(out, &m);
    let f = fx.sp - 0x70;
    fx.saved(f, &[(0x20, s[0]), (0x30, s[1]), (0x40, s[2]), (0x50, s[3]), (0x60, u64::from(ra))]);
}

/// 0x20C540(out, m, v).
fn turn(fx: &mut Fx, out: u32, m: u32, v: u32) {
    let m = fx.read_m4(m);
    let x = fx.read_v4(v);
    let r = fx.transform(Some(&m), x);
    fx.write_v4(out, r);
}

fn tint(fx: &mut Fx, sp: u32, c: u8, s: [u64; 2], ra: u32) {
    for k in 0..4 {
        fx.mem.write_u8(sp + 0x19C + k, c);
    }
    let t = fx.mem.i32(D + 8);
    unit_tint_saving(fx, t, sp + 0x19C, s, ra);
}

/// A stroke record `i` along the path just jagged (the spec at sp + 0x100).
fn stroke_fields(fx: &mut Fx, sp: u32, i: i32, path: u32) {
    let r = STROKES + ((i as u32) << 5);
    let n = fx.mem.u8(sp + 0x100) - 1;
    fx.mem.set8(r + 8, n);
    fx.mem.write_u32(r + 0x14, 0x3E4C_CCCD);
    set(fx, r, &[(9, 1), (0xA, 0), (0xB, 2), (0xC, 7), (0xD, 3), (0xE, 2), (0x18, 0x60), (0x19, 0x60), (0x1A, 0x60), (0x1B, 0x20)], &[(0x1C, path)]);
}

/// The 0x102A50 spec at sp + 0x100: `n` points from sp + 0xD0 to sp + 0xE0
/// into the path of stroke `i`. Returns the path.
fn jag_path(fx: &mut Fx, sp: u32, i: i32, n: i32) -> u32 {
    let path = PATHS + 0x168 * i as u32;
    fx.mem.set32(sp + 0x100, n);
    fx.mem.write_u32(sp + 0x104, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x120, path);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0xD0 + 4 * c);
        fx.mem.write_u32(sp + 0x108 + 4 * c, w);
    }
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0xE0 + 4 * c);
        fx.mem.write_u32(sp + 0x114 + 4 * c, w);
    }
    blib::bolt::jag(fx, sp + 0x100);
    path
}

/// Updates 0xF4 / 0x116 / 0x12C [F 0x1E05F24]: four strokes rising from
/// about the target, the fade and a flash. Returns s0.
fn strokes(fx: &mut Fx, sp: u32, n: i32, s0_in: u64) -> u64 {
    let z = fx.mem.f(sp + 0xB8);
    let (x, count, k) = match n {
        0x12C => (0, 0x10, 0x3FE6_6666),
        0x116 => (0xC220_0000, 0xC, 0x400C_CCCD),
        _ => (0x41F0_0000, 0x10, 0x4000_0000),
    };
    fx.mem.write_u32(sp + 0xC0, x);
    fx.mem.setf(sp + 0xC8, ee::mul(f32::from_bits(k), z));
    if fx.mem.read_u32(D + 0x30) != 0x8000 {
        let v = ee::mul(fx.mem.f(sp + 0xC0), -1.0);
        fx.mem.setf(sp + 0xC0, v);
    }
    fx.mem.write_u32(sp + 0xC4, 0x41F0_0000);
    set_counter4(fx, FADE, count, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    let _ = s0_in;
    let mut s0: u64 = count as u64;
    for s4 in 0..4i32 {
        copy_v4(fx, sp + 0xC0, sp + 0xE0);
        copy_v4(fx, sp + 0xE0, sp + 0xD0);
        let y = ee::add(fx.mem.f(sp + 0xD4), 180.0);
        fx.mem.setf(sp + 0xD4, y);
        let (f20, base, f22, ra) = match s4 {
            3 => (30.0, 60.0, 20.0, 0x01E0_61F8),
            2 => (50.0, 100.0, -5.0, 0x01E0_6190),
            1 => (-10.0, 70.0, 30.0, 0x01E0_6120),
            _ => (-30.0, 80.0, -10.0, 0x01E0_60B0),
        };
        let f21 = ee::add(base, jit(fx, s0, ra, 10, 1000));
        let v = ee::add(fx.mem.f(sp + 0xE4), f21);
        fx.mem.setf(sp + 0xE4, v);
        if fx.mem.read_u32(D + 0x30) == 0x8000 {
            let v = ee::add(fx.mem.f(sp + 0xE0), f20);
            fx.mem.setf(sp + 0xE0, v);
            let v = ee::add(fx.mem.f(sp + 0xE8), f22);
            fx.mem.setf(sp + 0xE8, v);
        } else {
            let v = ee::sub(fx.mem.f(sp + 0xE0), f20);
            fx.mem.setf(sp + 0xE0, v);
            let v = ee::sub(fx.mem.f(sp + 0xE8), f22);
            fx.mem.setf(sp + 0xE8, v);
        }
        let i = first_free(fx, STROKES, 0x20, 0x1E);
        s0 = i as i64 as u64;
        if i < 0 {
            continue;
        }
        let path = jag_path(fx, sp, i, 0x14);
        let r = STROKES + ((i as u32) << 5);
        s0 = u64::from(D + ((i as u32) << 5));
        set_counter4(fx, r, 0x14, s4 & 0xFF, 0xFF);
        fx.mem.write_u8(r + 4, 6);
        stroke_fields(fx, sp, i, path);
        fx.mem.write_u8(r + 0xF, 1);
    }
    let i = first_free(fx, FLASH, 0x4C, 0x24);
    if i < 0 {
        return s0;
    }
    let r = FLASH + 0x4C * i as u32;
    s0 = u64::from(0x4C * i as u32);
    set_counter4(fx, r, 0xE, 0, 0xFF);
    set(fx, r, &[(5, 0), (4, 0x85), (7, 9), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xF, 0xC0), (0xA, 0), (9, 0), (8, 0x80)], &[(0x14, 0), (0x10, 0x4448_0000), (0x14, 0x4270_0000)]);
    copy_v4(fx, sp + 0xC0, r + 0x1C);
    let y = ee::add(fx.mem.f(r + 0x20), 180.0);
    fx.mem.setf(r + 0x20, y);
    s0
}

/// Update 0xF8 [F 0x1E06498]: nine tilted bands about the target.
fn bands(fx: &mut Fx) {
    for s2 in 0..9i32 {
        let r = BANDS + 0x5C * s2 as u32;
        let s1 = 8 * s2;
        set_counter4(fx, r, 0xFF, 0, 0xFF);
        set(fx, r, &[(4, 0x80), (0xC, 0x30), (8, 2), (9, 1), (0xA, 5), (0xE, 1)], &[]);
        fx.mem.set8(r + 7, (s2 >> 1) + 0xC);
        fx.mem.set8(r + 0xB, (s2 + 1) & 3);
        fx.mem.setf(r + 0x10, ee::mul(25.0, ee::from_i32(s2 + 1)));
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.setf(r + 0x18, ee::add(10.0, ee::from_i32(s1)));
        fx.mem.setf(r + 0x1C, ee::from_i32(s1));
        let b = fx.mem.read_u8(D + 4);
        fx.mem.write_u8(r + 0xD, b);
        set(fx, r, &[(0x24, 0x60), (0x25, 0x60), (0x26, 0x60), (0x28, 0x60), (0x29, 0x60), (0x2A, 0x60), (0x27, 0x80), (0x2B, 0x10)], &[(0x2C, 0)]);
        fx.mem.setf(r + 0x30, ee::sub(180.0, ee::mul(3.0, ee::from_i32(s2))));
        fx.mem.write_u32(r + 0x34, 0);
        fx.mem.set32(r + 0x50, 0x30);
        if s2 & 1 != 0 {
            let v = ee::mul(ee::from_i32(0x30), -1.0);
            fx.fix_residue(v, 0x01E0_65B4);
            fx.mem.set32(r + 0x50, ee::fixsfsi(v));
        }
    }
}

/// The shake (+0x40): `count`, kind 8, scales `k`, on (`node`) or not.
fn shake(fx: &mut Fx, sp: u32, count: i32, node: bool, k: [u32; 3], amp: u32, rate: u32) {
    set_counter4(fx, SHAKE, count, 0, 0xFF);
    if node {
        fx.mem.write_u8(SHAKE + 5, 1);
        let b = fx.mem.read_u8(D + 0x18);
        fx.mem.write_u8(SHAKE + 6, b);
    } else {
        fx.mem.write_u8(SHAKE + 5, 0);
    }
    fx.mem.write_u8(SHAKE + 4, 8);
    for (o, v) in [0x14u32, 0x18, 0x1C].into_iter().zip(k) {
        fx.mem.write_u32(SHAKE + o, v);
    }
    fx.mem.write_u32(SHAKE + 0x30, 0);
    fx.mem.write_u32(SHAKE + 0x34, amp);
    if node {
        copy_v4(fx, sp + 0xB0, SHAKE + 0x20);
        fx.mem.write_u32(SHAKE + 0x50, rate);
    } else {
        fx.mem.write_u32(SHAKE + 0x50, rate);
        copy_v4(fx, sp + 0xB0, SHAKE + 0x20);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut s0 = RUNNER_S0;
    let mut s1 = param as i64 as u64;
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xB0, s0, 0x01E0_5CD4);
    let n = fx.mem.u16(D);
    match n {
        0x214 => return 8,
        0x1F4 => tint(fx, sp, 0x80, [s0, s1], 0x01E0_6700),
        0x18E => {
            tint(fx, sp, 0, [s0, s1], 0x01E0_6684);
            set_counter4(fx, HAZE, 0x70, 0, 0);
            set(fx, HAZE, &[(0xC, 1), (0xD, 0xCF), (0xE, 0x10), (0x12, 1), (0x10, 3), (0x11, 0)], &[]);
            let t = fx.mem.read_u32(D + 8);
            fx.mem.write_u32(HAZE + 0x14, t);
            set(fx, HAZE, &[(0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x80)], &[]);
        }
        0x184 => {
            set_counter4(fx, FADE, 0x18, 0, 0xFF);
            fx.mem.write_u8(FADE + 4, 1);
            shake(fx, sp, 0x74, false, [0x4000_0000, 0x3FCC_CCCD, 0x3F80_0000], 0x42A0_0000, 0x40C0_0000);
        }
        0xF8 => {
            bands(fx);
            s0 = u64::from(D + 0x5C * 9);
            s1 = 72;
        }
        0x12C | 0x116 | 0xF4 => s0 = strokes(fx, sp, n, s0),
        0x15E => {
            fx.mem.set16(WINDOW, 0x15E);
            fx.mem.set16(WINDOW + 2, 0x190);
            fx.mem.write_u8(WINDOW + 5, 6);
            fx.mem.write_u8(WINDOW + 6, 2);
            for o in [8u32, 0x14, 0xC, 0x18, 0x10] {
                fx.mem.write_u32(WINDOW + o, 0);
            }
            for o in [0x20u32, 0x1C, 0x28, 0x24] {
                fx.mem.set32(WINDOW + o, -2);
            }
            for o in [0x2Eu32, 0x2D, 0x2C] {
                fx.mem.write_u8(WINDOW + o, 0x60);
            }
        }
        0xC4 | 0x99 => {
            let v = fx.mem.i32(MASK) >> 1;
            fx.mem.set32(MASK, v);
        }
        0x13C | 0x120 | 0x104 | 0xF0 => {
            let (count, kind) = match n {
                0x13C => (0xE, 2),
                0x120 => (0xC, 2),
                0x104 => (0x10, 2),
                _ => (4, 4),
            };
            set_counter4(fx, BLACK, count, 0, 0xFF);
            fx.mem.write_u8(BLACK + 4, kind);
        }
        0x69 => {
            fx.mem.set32(MASK, 3);
            shake(fx, sp, 0x8C, true, [0x3F99_999A; 3], 0x41F0_0000, 0x4000_0000);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x184..0x1E9).contains(&n) {
        (s0, s1) = rising_stroke(fx, sp, s0, s1);
    }
    let n = fx.mem.u16(D);
    if (0x184..0x1F1).contains(&n) {
        (s0, s1) = sparks(fx, sp, s0);
    }
    let n = fx.mem.u16(D);
    if (0x69..0x17D).contains(&n) && n & fx.mem.i32(MASK) == 0 {
        (s0, s1) = bolts(fx, sp, s0, s1);
    }
    shake_saving(fx, SHAKE, s0, 0x01E0_701C, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_7028, [Some(s0), Some(s1)]);
    let ot = OT;
    blib::fade::fade(fx, FADE, ot);
    blib::fade::fade_black(fx, BLACK, ot);
    let (_, spike_a3) = blib::spike::spikes_a3(fx, 0x01E0_0E10, SPIKES, ot, 0x3C);
    let (_, bolt_a3) = blib::bolt::bolts_b_a3(fx, 0x01E0_18F0, BOLTS, ot, 0x64);
    blib::spray::sprays_b(fx, 0x01E0_3790, SPRAYS, ot, 0x50);
    let (_, band_a3) = blib::band::bands_d_a3(fx, 0x01E0_3FA0, BANDS, ot, 0x18);
    blib::band::bands_b(fx, 0x01E0_47B0, BANDS_B, ot, 8);
    let (_, flash_a3) = blib::star::flash_b_a3(fx, 0x01E0_03A0, FLASH, ot, 0x24);
    let (_, stroke_a3) = blib::dash::strokes_a3(fx, 0x01E0_2380, STROKES, ot, 0x1E);
    // The a3 the haze reports with its first TEXFLUSH and passes on to the
    // frame fade (the haze's own when it draws) [run]: the strokes', the
    // flashes', the bands', the bolts', the spikes' (sprays_b / bands_b
    // never count), else [H] 0.
    let a3 = stroke_a3.or(flash_a3).or(band_a3).or(bolt_a3).or(spike_a3).unwrap_or(0);
    let (_, a3) = blib::haze::screen_a3(fx, 0x01E0_4EF0, HAZE, ot, 1, a3);
    let n = fx.mem.u16(D);
    blib::window::frame_fade_b(fx, 0x01E0_5700, WINDOW, ot, n, a3);
    let n = fx.mem.u16(D);
    if (0x184..0x1F1).contains(&n) {
        6
    } else if n == 0x1F1 {
        2
    } else if n == 0x17A {
        1
    } else {
        0
    }
}

/// Updates 0x184..0x1E8 [F 0x1E06730]: a stroke from 140 above the target
/// to a jittered point 50 below. Returns (s0, s1).
fn rising_stroke(fx: &mut Fx, sp: u32, _s0: u64, s1_in: u64) -> (u64, u64) {
    copy_v4(fx, sp + 0xB0, sp + 0xE0);
    copy_v4(fx, sp + 0xE0, sp + 0xD0);
    let y = ee::add(fx.mem.f(sp + 0xD4), 140.0);
    fx.mem.setf(sp + 0xD4, y);
    let y = ee::sub(fx.mem.f(sp + 0xE4), 50.0);
    fx.mem.setf(sp + 0xE4, y);
    let s0 = u64::from(sp + 0xE0);
    let x = ee::add(fx.mem.f(sp + 0xE0), jit(fx, s0, 0x01E0_67B4, 0x32, 0x1388));
    fx.mem.setf(sp + 0xE0, x);
    let z = ee::add(fx.mem.f(sp + 0xE8), jit(fx, s0, 0x01E0_680C, 0x1E, 0xBB8));
    fx.mem.setf(sp + 0xE8, z);
    let _ = s1_in;
    let s1 = u64::from(sp + 0xD0);
    let i = first_free(fx, STROKES, 0x20, 0x1E);
    if i < 0 {
        return (s0, s1);
    }
    let path = jag_path(fx, sp, i, 0x19);
    let count = (rand4(fx, s0, 0x01E0_68E8) & 7) + 9;
    let r = STROKES + ((i as u32) << 5);
    set_counter4(fx, r, count & 0xFF, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0x86);
    stroke_fields(fx, sp, i, path);
    (u64::from((i as u32) << 5), s1)
}

/// Updates 0x184..0x1F0 [F 0x1E06994]: a flash and a spike at a jittered
/// point about the target. Returns (s0, s1).
fn sparks(fx: &mut Fx, sp: u32, s0_in: u64) -> (u64, u64) {
    let mut s0 = s0_in;
    let x = ee::add(fx.mem.f(sp + 0xB0), jit1(fx, s0, 0x01E0_69A0, 0xBB8, 0x5DC));
    fx.mem.setf(sp + 0xF0, x);
    let y = ee::add(fx.mem.f(sp + 0xB4), jit1(fx, s0, 0x01E0_69E0, 0xBB8, 0x5DC));
    fx.mem.setf(sp + 0xF4, y);
    let z = ee::add(fx.mem.f(sp + 0xB8), jit1(fx, s0, 0x01E0_6A20, 0x7D0, 0x3E8));
    fx.mem.setf(sp + 0xF8, z);
    let i = first_free(fx, FLASH, 0x4C, 0x10);
    if i >= 0 {
        let r = FLASH + 0x4C * i as u32;
        s0 = u64::from(D + 0x4C * i as u32);
        set_counter4(fx, r, 0xC, 0, 0xFF);
        set(fx, r, &[(5, 0), (4, 0x81), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x10), (8, 0x80)], &[(0x14, 0), (0x10, 0x43C8_0000)]);
        copy_v4(fx, sp + 0xF0, r + 0x1C);
    }
    let j = first_free(fx, SPIKES, 0x68, 0x3C);
    if j >= 0 {
        let r = SPIKES + 0x68 * j as u32;
        s0 = u64::from(D + 0x68 * j as u32);
        fx.mem.write_u8(r + 4, 0x85);
        set_counter4(fx, r, 0xC, 0, 0xFF);
        set(fx, r, &[(0x27, 0x40), (0x23, 0x10), (5, 0), (0xA, 0), (0xB, 2), (0xC, 4), (8, 0x1E)], &[(0x14, 0x40B0_0000), (0x1C, 0x4270_0000)]);
        let v = rand4(fx, s0, 0x01E0_6BA0);
        fx.mem.set8(r + 9, v);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        set(fx, r, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x24, 0x80)], &[]);
        copy_v4(fx, sp + 0xF0, r + 0x28);
    }
    (s0, 1)
}

/// Updates 0x69..0x17C the mask lets through [F 0x1E06C44]: three bolts
/// and a flash at a random turn (radius 6 +- 1) round node +0xC, rising
/// 1.2 an update from 0x16B. Returns (s0, s1).
fn bolts(fx: &mut Fx, sp: u32, s0_in: u64, s1_in: u64) -> (u64, u64) {
    let (mut s0, mut s1) = (s0_in, s1_in);
    loop {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
        node_position_saving(fx, h, slot, sp + 0xC0, s0, Some(s1), 0x01E0_6C54);
        for (o, ra) in [(0x180u32, 0x01E0_6C5C), (0x184, 0x01E0_6C6C), (0x188, 0x01E0_6C7C)] {
            let v = rand4(fx, s0, ra) & 0xFFFF;
            fx.mem.set32(sp + o, v);
        }
        rot_into(fx, sp + 0x130, sp + 0x180, [s0, s1, 0, 0], 0x01E0_6C9C);
        fx.mem.write_u32(sp + 0x178, 0);
        fx.mem.write_u32(sp + 0x170, 0);
        let y = ee::add(6.0, jit(fx, s0, 0x01E0_6CA8, 3, 0x12C));
        fx.mem.setf(sp + 0x174, y);
        turn(fx, sp + 0xD0, sp + 0x130, sp + 0x170);
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0xC0 + 4 * c), fx.mem.f(sp + 0xD0 + 4 * c));
            fx.mem.setf(sp + 0xC0 + 4 * c, v);
        }
        s0 = u64::from(sp + 0xC4);
        let n = fx.mem.u16(D);
        if n >= 0x16B {
            let v = ee::add(fx.mem.f(sp + 0xC4), ee::mul(f32::from_bits(0x3F99_999A), ee::from_i32(n - 0x163)));
            fx.mem.setf(sp + 0xC4, v);
        }
        let mut s5 = 0i32;
        while s5 < 3 {
            let i = first_free(fx, BOLTS, 0x40, 0x64);
            if i < 0 {
                break;
            }
            let r = BOLTS + ((i as u32) << 6);
            s1 = u64::from(D + ((i as u32) << 6));
            set_counter4(fx, r, 8, s5 & 0xFF, 0xFF);
            fx.mem.write_u8(r + 5, 0);
            fx.mem.write_u8(r + 4, 0x84);
            for c in 0..3u32 {
                let w = fx.mem.read_u32(sp + 0xC0 + 4 * c);
                fx.mem.write_u32(r + 0x28 + 4 * c, w);
                fx.mem.write_u32(r + 0x1C + 4 * c, w);
            }
            for (c, ra) in [(0u32, 0x01E0_6E04), (4, 0x01E0_6E5C), (8, 0x01E0_6EB4)] {
                let v = ee::add(fx.mem.f(r + 0x28 + c), jit(fx, s0, ra, 6, 0x258));
                fx.mem.setf(r + 0x28 + c, v);
            }
            s5 += 1;
            set(fx, r, &[(8, 6), (0xB, 1), (0xD, 4), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x18)], &[(0x10, 0x3F80_0000), (0x14, 0x3ECC_CCCD)]);
        }
        let i = first_free(fx, FLASH, 0x4C, 0x24);
        if i >= 0 {
            let r = FLASH + 0x4C * i as u32;
            s0 = u64::from(D + 0x4C * i as u32);
            set_counter4(fx, r, 0xE, 0, 0xFF);
            set(fx, r, &[(5, 0), (4, 0x85), (7, 9), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xF, 0xC0), (0xA, 0), (9, 0), (8, 0x80)], &[(0x14, 0), (0x10, 0x42F0_0000), (0x14, 0)]);
            copy_v4(fx, sp + 0xC0, r + 0x1C);
        }
        if s5 + 1 >= 2 {
            break;
        }
    }
    (s0, s1)
}
