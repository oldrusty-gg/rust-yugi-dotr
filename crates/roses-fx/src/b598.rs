//! BMODULE b598 (card 599), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05E50, data area a0 = 0x1ED0000, 0xC6E0 bytes)
//! [F]: s6 = D, the OT (0x206E40) in s0, s1 the
//! parameter. Init: the handles (+4 / +8), the attacker's rotation
//! (+0x2C), textures (4, 0xF) and (5, 0..4), nodes 1 (cleared, +0xC) /
//! 0xB / 0x3D / 0x55 (+0x10..+0x18), the stage light ramp (+0xA0: 2 /
//! 0x154) and the random bytes (+0xC5D8). Every update the target's
//! position (sp + 0xB0). Update 8 the streaks (+0x4990,
//! `radiate::streaks_b`) on node +0xC; 0x14 nine tilted bands (+0x3CB4,
//! `band::bands_d`) about the target; 0x96 / 0xA6 / 0xBE the black fade
//! (+0x455C); 0x9A / 0xB2 four strokes (+0x97E8, `dash::strokes`) rising
//! 160 about the target, the fade (+0x4554) and an 800-wide flash
//! (+0x1904); 0x97..0x136 35 sparks a update at a random bearing and
//! distance 10..190 round the target, each a flat quad (+0x49C8,
//! `flatquad::quads`) and a line rising 180 (+0x9018, `flatquad::lines`);
//! 0xCA the fade and a shake; 0xCA..0x12E a rising stroke, ..0x136 a flash
//! and a spike (+0xA4) about the target; 0xD4 the target tinted black and
//! the haze (+0x4564, `haze::screen`, 0x70 counts), 0x13A the tint back.
//! Returns 6 on 0xCA..0x136, 2 on 0x137, 1 on 0xC0, 8 on 0x15A (no
//! drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash_b`
//! 0x1E003A0, `spikes` 0x1E00E10, `bolts_b` 0x1E018F0 (+0x23B4 never
//! armed), `strokes` 0x1E02380, `fade` 0x1E02F50, `fade_black` 0x1E03420,
//! `bands_d` 0x1E03790, `haze::screen` 0x1E03FA0, `streaks_b` 0x1E047B0,
//! `flatquad::quads` 0x1E055B0, `flatquad::lines` 0x1E05BB0
//! [`crate::blib`].
use crate::b015::unit_tint_saving;
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const SPIKES: u32 = D + 0xA4;
const FLASH: u32 = D + 0x1904;
const BOLTS: u32 = D + 0x23B4;
const BANDS: u32 = D + 0x3CB4;
const FADE: u32 = D + 0x4554;
const BLACK: u32 = D + 0x455C;
const HAZE: u32 = D + 0x4564;
const STREAKS: u32 = D + 0x4990;
const QUADS: u32 = D + 0x49C8;
const LINES: u32 = D + 0x9018;
const STROKES: u32 = D + 0x97E8;
const PATHS: u32 = D + 0x9BA8;
const BYTES: u32 = D + 0xC5D8;

/// The entry 0x1E05E50 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x130);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x130);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0xC6E0).
    fx.bzero(D, 0xC6E0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..5 {
        fx.texture(5, k);
    }
    for (k, (node, clear)) in [(1, 1), (0xB, 0), (0x3D, 0), (0x55, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x154);
    // The entry's s0 = 5 (the texture loop's), s1 the parameter, s2 the
    // runner's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5F84, [Some(5), Some(param as u32), None]);
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

fn tint(fx: &mut Fx, sp: u32, c: u8, s1: u64, ra: u32) {
    for k in 0..4 {
        fx.mem.write_u8(sp + 0x12C + k, c);
    }
    let t = fx.mem.i32(D + 8);
    unit_tint_saving(fx, t, sp + 0x12C, [u64::from(OT), s1], ra);
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

/// Updates 0x9A / 0xB2 [F 0x1E0623C]: four strokes rising 160 about the
/// target, the fade and a flash. Returns s1.
fn strokes(fx: &mut Fx, sp: u32, n: i32, s1_in: u64) -> u64 {
    let k = if n == 0x9A { 2.0 } else { 1.5 };
    let z = ee::mul(k, fx.mem.f(sp + 0xB8));
    fx.mem.setf(sp + 0xC8, z);
    fx.mem.write_u32(sp + 0xC4, 0);
    set_counter4(fx, FADE, 0xC, 0, 0xFF);
    fx.mem.write_u32(sp + 0xC0, 0);
    fx.mem.write_u8(FADE + 4, 1);
    let mut s1 = s1_in;
    for s4 in 0..4i32 {
        copy_v4(fx, sp + 0xC0, sp + 0xE0);
        copy_v4(fx, sp + 0xE0, sp + 0xD0);
        let y = ee::add(fx.mem.f(sp + 0xD4), 160.0);
        fx.mem.setf(sp + 0xD4, y);
        s1 = u64::from(sp + 0xE0);
        let (f20, base, f22, ra) = match s4 {
            3 => (30.0, 70.0, 20.0, 0x01E0_64AC),
            2 => (50.0, 110.0, -5.0, 0x01E0_643C),
            1 => (-10.0, 80.0, 30.0, 0x01E0_63CC),
            _ => (-30.0, 90.0, -10.0, 0x01E0_635C),
        };
        let f21 = ee::add(base, jit(fx, ra, 0xF, 0x5DC));
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
        if i < 0 {
            continue;
        }
        let path = jag_path(fx, sp, i, 0x14);
        let r = STROKES + ((i as u32) << 5);
        s1 = u64::from(D + ((i as u32) << 5));
        set_counter4(fx, r, 0x14, s4 & 0xFF, 0xFF);
        fx.mem.write_u8(r + 4, 6);
        stroke_fields(fx, sp, i, path);
        fx.mem.write_u8(r + 0xF, 1);
    }
    let i = first_free(fx, FLASH, 0x4C, 0x24);
    if i < 0 {
        return s1;
    }
    let r = FLASH + 0x4C * i as u32;
    set_counter4(fx, r, 0xE, 0, 0xFF);
    set(fx, r, &[(5, 0), (4, 0x85), (7, 9), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xF, 0xC0), (0xA, 0), (9, 0), (8, 0x80)], &[(0x14, 0), (0x10, 0x4448_0000), (0x14, 0x4270_0000)]);
    copy_v4(fx, sp + 0xC0, r + 0x1C);
    let y = ee::add(fx.mem.f(r + 0x20), 160.0);
    fx.mem.setf(r + 0x20, y);
    u64::from(0x4C * i as u32)
}

/// Update 0x14 [F 0x1E060B4]: nine tilted bands about the target.
fn bands(fx: &mut Fx) {
    for k in 0..9i32 {
        let r = BANDS + 0x5C * k as u32;
        let s3 = 8 * k;
        set_counter4(fx, r, 0xFF, 0, 0xFF);
        set(fx, r, &[(4, 0x80), (0xC, 0x38), (8, 2), (9, 1), (0xA, 5), (0xE, 2)], &[]);
        fx.mem.set8(r + 7, (k >> 1) + 0xC);
        fx.mem.set8(r + 0xB, (k + 1) & 3);
        fx.mem.setf(r + 0x10, ee::mul(25.0, ee::from_i32(k + 1)));
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.setf(r + 0x18, ee::add(10.0, ee::from_i32(s3)));
        fx.mem.setf(r + 0x1C, ee::from_i32(s3));
        let b = fx.mem.read_u8(D + 4);
        fx.mem.write_u8(r + 0xD, b);
        set(fx, r, &[(0x24, 0x60), (0x25, 0x60), (0x26, 0x60), (0x28, 0x60), (0x29, 0x60), (0x2A, 0x60), (0x27, 0x80), (0x2B, 0x10)], &[(0x2C, 0)]);
        fx.mem.setf(r + 0x30, ee::sub(180.0, ee::mul(3.0, ee::from_i32(k))));
        fx.mem.write_u32(r + 0x34, 0);
        fx.mem.set32(r + 0x50, 0x30);
        if k & 1 != 0 {
            let v = ee::mul(ee::from_i32(0x30), -1.0);
            fx.fix_residue(v, 0x01E0_61D8);
            fx.mem.set32(r + 0x50, ee::fixsfsi(v));
        }
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut s1 = param as i64 as u64;
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xB0, u64::from(OT), 0x01E0_5FA4);
    let n = fx.mem.u16(D);
    // The dispatch's 0x14 test clears s1 in its delay slot: every update
    // not taken by an earlier case passes it [F 0x1E0601C; run].
    if ![0x15A, 0x13A, 0xD4, 0xCA, 0xB2, 0x9A, 0xBE, 0xA6, 0x96].contains(&n) {
        s1 = 0;
    }
    match n {
        0x15A => return 8,
        0x13A => tint(fx, sp, 0x80, s1, 0x01E0_68F8),
        0xD4 => {
            tint(fx, sp, 0, s1, 0x01E0_687C);
            set_counter4(fx, HAZE, 0x70, 0, 0);
            set(fx, HAZE, &[(0xC, 1), (0xD, 0xCF), (0xE, 0x10), (0x12, 1), (0x10, 3), (0x11, 0)], &[]);
            let t = fx.mem.read_u32(D + 8);
            fx.mem.write_u32(HAZE + 0x14, t);
            set(fx, HAZE, &[(0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x80)], &[]);
        }
        0xCA => {
            set_counter4(fx, FADE, 0x18, 0, 0xFF);
            fx.mem.write_u8(FADE + 4, 1);
            set_counter4(fx, SHAKE, 0x74, 0, 0xFF);
            fx.mem.write_u8(SHAKE + 5, 0);
            fx.mem.write_u8(SHAKE + 4, 8);
            set(fx, SHAKE, &[], &[(0x14, 0x4000_0000), (0x18, 0x3FCC_CCCD), (0x1C, 0x3F80_0000), (0x30, 0), (0x34, 0x42A0_0000), (0x50, 0x40C0_0000)]);
            copy_v4(fx, sp + 0xB0, SHAKE + 0x20);
        }
        0xB2 | 0x9A => s1 = strokes(fx, sp, n, s1),
        0xBE | 0xA6 | 0x96 => {
            let (count, kind) = if n == 0x96 { (4, 4) } else { (0xC, 2) };
            set_counter4(fx, BLACK, count, 0, 0xFF);
            fx.mem.write_u8(BLACK + 4, kind);
        }
        0x14 => {
            bands(fx);
            s1 = 9;
        }
        8 => {
            let r = STREAKS;
            set_counter4(fx, r, 0x18, 0, 0xFF);
            set(fx, r, &[(4, 0x80), (9, 2), (0xA, 0xFF), (8, 0x10), (0xB, 8)], &[]);
            let h = fx.mem.read_u32(D + 4);
            fx.mem.write_u32(r + 0xC, h);
            set(fx, r, &[(0x14, 0x80), (0x15, 0x80), (0x16, 0x80), (0x17, 0), (0x10, 0x90), (0x11, 0x90), (0x12, 0x90), (0x13, 0x40), (5, 1)], &[]);
            let b = fx.mem.read_u8(D + 0xC);
            fx.mem.write_u8(r + 6, b);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0xCA..0x12F).contains(&n) {
        s1 = rising_stroke(fx, sp);
    }
    let n = fx.mem.u16(D);
    if (0xCA..0x137).contains(&n) {
        s1 = sparks(fx, sp, s1);
    }
    let n = fx.mem.u16(D);
    if (0x97..0x137).contains(&n) {
        for _ in 0..0x23 {
            s1 = spark_quad(fx, sp, s1);
        }
    }
    shake_saving(fx, SHAKE, u64::from(OT), 0x01E0_71AC, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_71B8, [Some(u64::from(OT)), Some(s1)]);
    let ot = OT;
    blib::fade::fade(fx, FADE, ot);
    blib::fade::fade_black(fx, BLACK, ot);
    // fade / fade_black's prologues save the entry's s6 (D) and ra in
    // their frames (0xF0 / 0xE0) where the streaks' first TEXFLUSH reads a0
    // (vertex 1's stale XYZ low word) unless a later call drew over it
    // (run b598, as b457); s5 [H] not modelled.
    let f = fx.sp - 0xE0;
    fx.saved(f, &[(0x60, u64::from(D)), (0x70, 0x01E0_71D0)]);
    let (_, spike_a3) = blib::spike::spikes_a3(fx, 0x01E0_0E10, SPIKES, ot, 0x3C);
    let (_, bolt_a3) = blib::bolt::bolts_b_a3(fx, 0x01E0_18F0, BOLTS, ot, 0x64);
    let (_, band_a3) = blib::band::bands_d_a3(fx, 0x01E0_3790, BANDS, ot, 0x18);
    let (_, quad_a3) = blib::flatquad::quads_a3(fx, 0x01E0_55B0, QUADS, ot, 0xFA);
    let (_, line_a3) = blib::flatquad::lines_a3(fx, 0x01E0_5BB0, LINES, ot, 0x32);
    let (_, flash_a3) = blib::star::flash_b_a3(fx, 0x01E0_03A0, FLASH, ot, 0x24);
    let (_, stroke_a3) = blib::dash::strokes_a3(fx, 0x01E0_2380, STROKES, ot, 0x1E);
    // [H] the a3 the haze reports with its first TEXFLUSH: the strokes',
    // the flashes', the lines', the quads', the bands', the bolts', the
    // spikes', else 0.
    let a3 = stroke_a3.or(flash_a3).or(line_a3).or(quad_a3).or(band_a3).or(bolt_a3).or(spike_a3).unwrap_or(0);
    blib::haze::screen(fx, 0x01E0_3FA0, HAZE, ot, 1, a3);
    blib::radiate::streaks_b(fx, 0x01E0_47B0, STREAKS, ot, 1);
    let n = fx.mem.u16(D);
    if (0xCA..0x137).contains(&n) {
        6
    } else if n == 0x137 {
        2
    } else if n == 0xC0 {
        1
    } else {
        0
    }
}

/// Updates 0xCA..0x12E [F 0x1E06928]: a stroke from 140 above the target
/// to a jittered point 50 below. Returns s1.
fn rising_stroke(fx: &mut Fx, sp: u32) -> u64 {
    copy_v4(fx, sp + 0xB0, sp + 0xE0);
    copy_v4(fx, sp + 0xE0, sp + 0xD0);
    let y = ee::add(fx.mem.f(sp + 0xD4), 140.0);
    fx.mem.setf(sp + 0xD4, y);
    let y = ee::sub(fx.mem.f(sp + 0xE4), 50.0);
    fx.mem.setf(sp + 0xE4, y);
    let x = ee::add(fx.mem.f(sp + 0xE0), jit(fx, 0x01E0_69AC, 0x32, 0x1388));
    fx.mem.setf(sp + 0xE0, x);
    let z = ee::add(fx.mem.f(sp + 0xE8), jit(fx, 0x01E0_6A04, 0x1E, 0xBB8));
    fx.mem.setf(sp + 0xE8, z);
    let s1 = u64::from(sp + 0xE0);
    let i = first_free(fx, STROKES, 0x20, 0x1E);
    if i < 0 {
        return s1;
    }
    let path = jag_path(fx, sp, i, 0x19);
    let count = (rand4(fx, 0x01E0_6AE8) & 7) + 9;
    let r = STROKES + ((i as u32) << 5);
    set_counter4(fx, r, count & 0xFF, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0x86);
    stroke_fields(fx, sp, i, path);
    u64::from((i as u32) << 5)
}

/// Updates 0xCA..0x136 [F 0x1E06C08]: a flash and a spike at a jittered
/// point about the target. Returns s1.
fn sparks(fx: &mut Fx, sp: u32, s1_in: u64) -> u64 {
    let mut s1 = s1_in;
    let x = ee::add(fx.mem.f(sp + 0xB0), jit1(fx, 0x01E0_6C14, 0xBB8, 0x5DC));
    fx.mem.setf(sp + 0xF0, x);
    let y = ee::add(fx.mem.f(sp + 0xB4), jit1(fx, 0x01E0_6C54, 0xBB8, 0x5DC));
    fx.mem.setf(sp + 0xF4, y);
    let z = ee::add(fx.mem.f(sp + 0xB8), jit1(fx, 0x01E0_6C94, 0x7D0, 0x3E8));
    fx.mem.setf(sp + 0xF8, z);
    let i = first_free(fx, FLASH, 0x4C, 0x10);
    if i >= 0 {
        let r = FLASH + 0x4C * i as u32;
        s1 = u64::from(D + 0x4C * i as u32);
        set_counter4(fx, r, 0xC, 0, 0xFF);
        set(fx, r, &[(5, 0), (4, 0x81), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x10), (8, 0x80)], &[(0x14, 0), (0x10, 0x43C8_0000)]);
        copy_v4(fx, sp + 0xF0, r + 0x1C);
    }
    let j = first_free(fx, SPIKES, 0x68, 0x3C);
    if j >= 0 {
        let r = SPIKES + 0x68 * j as u32;
        s1 = u64::from(D + 0x68 * j as u32);
        fx.mem.write_u8(r + 4, 0x85);
        set_counter4(fx, r, 0xC, 0, 0xFF);
        set(fx, r, &[(0x27, 0x40), (0x23, 0x10), (5, 0), (0xA, 0), (0xB, 2), (0xC, 4), (8, 0x1E)], &[(0x14, 0x40B0_0000), (0x1C, 0x4270_0000)]);
        let v = rand4(fx, 0x01E0_6E18);
        fx.mem.set8(r + 9, v);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        set(fx, r, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x24, 0x80)], &[]);
        copy_v4(fx, sp + 0xF0, r + 0x28);
    }
    s1
}

/// One of 35 sparks a update 0x97..0x136 [F 0x1E06EA8]: at a random
/// bearing (+-90 degrees) and distance (10..190) round the target, a flat
/// quad and a line rising 180 from it. Returns s1.
fn spark_quad(fx: &mut Fx, sp: u32, s1_in: u64) -> u64 {
    let mut s1 = s1_in;
    let f20 = ee::add(100.0, jit(fx, 0x01E0_6EB0, 0x5A, 0x2328));
    let deg = jit(fx, 0x01E0_6F04, 0xB4, 0x4650);
    let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), deg), 180.0);
    let s = fx.libm_sinf(a);
    fx.mem.setf(sp + 0xC0, ee::mul(f20, s));
    let c = libm_cosf(fx, a);
    fx.mem.setf(sp + 0xC8, ee::mul(f20, c));
    let i = first_free(fx, QUADS, 0x48, 0xFA);
    if i >= 0 {
        let r = QUADS + 0x48 * i as u32;
        s1 = u64::from(D + 0x48 * i as u32);
        set_counter4(fx, r, 0xC, 0, 0xFF);
        set(fx, r, &[(5, 0), (8, 4), (0xB, 2), (0xA, 9), (9, 0), (0xE, 0x10), (0xD, 0x40), (0xC, 0x80), (0xF, 0x60)], &[(0x14, 0x3F80_0000), (0x10, 0x3F80_0000), (0x1C, 0x3F80_0000), (0x18, 0x3F80_0000)]);
        let x = ee::add(fx.mem.f(sp + 0xB0), fx.mem.f(sp + 0xC0));
        fx.mem.setf(r + 0x30, x);
        fx.mem.write_u32(r + 0x34, 0x4000_0000);
        let z = ee::add(fx.mem.f(sp + 0xB8), fx.mem.f(sp + 0xC8));
        fx.mem.setf(r + 0x38, z);
    }
    let j = first_free(fx, LINES, 0x28, 0x32);
    if j >= 0 {
        let r = LINES + 0x28 * j as u32;
        s1 = u64::from(D + 0x28 * j as u32);
        set_counter4(fx, r, 2, 0, 0xFF);
        set(fx, r, &[(4, 0x40), (9, 0x60), (0xE, 0x60), (8, 0x80), (0xB, 0x18), (0xD, 0x60), (0xC, 0x80)], &[]);
        let v = (rand4(fx, 0x01E0_70F8) & 0x1F) + 0x40;
        fx.mem.set8(r + 0xF, v);
        let x = ee::add(fx.mem.f(sp + 0xB0), fx.mem.f(sp + 0xC0));
        fx.mem.setf(r + 0x10, x);
        fx.mem.write_u32(r + 0x14, 0x4334_0000);
        let z = ee::add(fx.mem.f(sp + 0xB8), fx.mem.f(sp + 0xC8));
        fx.mem.setf(r + 0x18, z);
        let x = ee::add(fx.mem.f(sp + 0xB0), fx.mem.f(sp + 0xC0));
        fx.mem.setf(r + 0x1C, x);
        fx.mem.write_u32(r + 0x20, 0x4000_0000);
        let z = ee::add(fx.mem.f(sp + 0xB8), fx.mem.f(sp + 0xC8));
        fx.mem.setf(r + 0x24, z);
    }
    s1
}
