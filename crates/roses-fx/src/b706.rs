//! BMODULE b706 (card 75), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05630, data area a0 = 0x1ED0000, 0xB010 bytes)
//! [F]: s6 = D, the OT (0x206E40) at sp + 0xB8.
//! Init: the handles (+4 / +8), the attacker's rotation (+0x2C), textures
//! (4, 0xF) and (5, 0..5), nodes 1 (cleared, +0xC) / 0x3A / 0x39 / 0x3B /
//! 0x3C (+0x10..+0x1C), the stage light ramp (+0xA0: 2 / 0x1BA) and the
//! random bytes (+0xAF04). Every update the target's position (sp + 0xD0).
//! Update 2 the frame fade window (+0x7EF4, 2..0x30); 0x3E / 0x4E / 0x68
//! the black fade (+0x5E6C, 4 / 8 / 8 counts); 0x32 / 0x42 / 0x56 a stroke
//! (+0x8114, `dash::strokes`, along a jagged 20-point path 0x102A50 from
//! node +0x14 / +0x10 / their midpoint, 40 up and +-40 by D+0x30, to node
//! +0x1C... the second node) with the fade (+0x5E64), a flash (+0xA4), 50
//! drops (+0xEB4) and a shake there; 0x70 the fade and a long shake; odd
//! 0x71..0x8B a stroke from a point between nodes +0x10 / +0x14 and a flash;
//! 0x8C the scrolling quad (+0x7FD4, `scrollquad::quads`) between nodes
//! +0x18 / +0x10 / +0x14 / +0x1C; 0x8C..0x167 three bolts (+0x7774,
//! `bolt::bolts_b`) per tenth of the node line; 0xF0 the spike / flash 0
//! (55 counts) moving from node +0x14 to +0x10 until 0x12C; 0x118 the
//! flash 300 wide; 0x118..0x167 drops 0..9 along the line; 0x120 ten
//! drops; 0x168 the fade, the window (+0x7F24, `window::flat`) and the
//! frame fade window (0x168..0x190), two spikes, a flash, the dome
//! (+0x7F34, `dome::domes`) and 300 drops at the target. Returns 6 on
//! 0x168..0x18C, 2 on 0x165, 1 on 0x15E, 8 on 0x1C0 (no drawing then) and
//! at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `fade`
//! 0x1E003A0, `fade_black` 0x1E00870, `bolts_b` 0x1E00BE0, `strokes`
//! 0x1E01670, `flash_b` 0x1E02240, `spikes` 0x1E02CB0, `drops` 0x1E03790,
//! `frame_fade_b` 0x1E040F0, `window::flat` 0x1E04570, `domes` 0x1E046F0,
//! `scrollquad::quads` 0x1E05130 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const FLASH: u32 = D + 0xA4;
const SPIKES: u32 = D + 0x694;
const DROPS: u32 = D + 0xEB4;
const FADE: u32 = D + 0x5E64;
const BLACK: u32 = D + 0x5E6C;
const BOLTS: u32 = D + 0x7774;
const WINDOW: u32 = D + 0x7EF4;
const FLAT: u32 = D + 0x7F24;
const DOME: u32 = D + 0x7F34;
const QUADS: u32 = D + 0x7FD4;
const STROKES: u32 = D + 0x8114;
const PATHS: u32 = D + 0x84D4;
const BYTES: u32 = D + 0xAF04;
/// [H] the runner's s1 at the entry.
const RUNNER_S1: u64 = 0;

/// The entry 0x1E05630 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(sp + 0xB8, OT);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x150);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0xB010).
    fx.bzero(D, 0xB010);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..6 {
        fx.texture(5, k);
    }
    for (k, (node, clear)) in [(1, 1), (0x3A, 0), (0x39, 0), (0x3B, 0), (0x3C, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x1BA);
    // The entry's s0 = 6 (the texture loop's), s1 / s2 the runner's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5770, [Some(6), None, None]);
    0
}

/// rand() >> 4 with the entry's s0 (64 bits) saved in rand's frame.
fn rand4(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand() >> 4
}

/// ((rand >> 4) % m - half) / 100.
fn jit(fx: &mut Fx, s0: u64, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand4(fx, s0, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

/// a + k (b - a) per axis into `out`.
fn lerp3(fx: &mut Fx, a: u32, b: u32, k: f32, out: &[u32]) {
    for c in 0..3u32 {
        let (x, y) = (fx.mem.f(a + 4 * c), fx.mem.f(b + 4 * c));
        let v = ee::add(x, ee::mul(k, ee::sub(y, x)));
        for &o in out {
            fx.mem.setf(o + 4 * c, v);
        }
    }
}

fn node(fx: &mut Fx, slot: u32, out: u32, s0: u64, s1: u64, ra: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + slot));
    node_position_saving(fx, h, s, out, s0, Some(s1), ra);
}

/// The frame fade window's corners (+0xC.. +0x28: x0, x1, x2, x3, y0..).
fn window_corners(fx: &mut Fx, d: i32) {
    for (o, v) in [(0x14u32, -d), (0xC, -d), (0x18, d), (0x10, d), (0x20, -d), (0x1C, -d), (0x28, d), (0x24, d)] {
        fx.mem.set32(WINDOW + o, v);
    }
}

/// A flash record's bytes / words.
fn set(fx: &mut Fx, r: u32, bytes: &[(u32, u8)], words: &[(u32, u32)]) {
    for &(o, v) in bytes {
        fx.mem.write_u8(r + o, v);
    }
    for &(o, v) in words {
        fx.mem.write_u32(r + o, v);
    }
}

/// The shake (+0x40): `count`, kind `kind`, rate `rate` on node +0x10.
fn shake(fx: &mut Fx, count: i32, kind: u8, rate: u32) {
    set_counter4(fx, SHAKE, count, 0, 0xFF);
    fx.mem.write_u8(SHAKE + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(SHAKE + 6, b);
    fx.mem.write_u8(SHAKE + 4, kind);
    for o in [0x14u32, 0x18, 0x1C] {
        fx.mem.write_u32(SHAKE + o, 0x3FF3_3333);
    }
    fx.mem.write_u32(SHAKE + 0x30, 0);
    fx.mem.write_u32(SHAKE + 0x34, 0x4220_0000);
    fx.mem.write_u32(SHAKE + 0x50, rate);
}

/// A stroke `i` along the jagged path from sp + 0x100 to sp + 0x110 (the
/// 0x102A50 spec at sp + 0x120, 20 points, amplitude 1).
fn stroke(fx: &mut Fx, sp: u32, i: i32) {
    let path = PATHS + 0x168 * i as u32;
    fx.mem.set32(sp + 0x120, 0x14);
    fx.mem.write_u32(sp + 0x124, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x140, path);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x100 + 4 * c);
        fx.mem.write_u32(sp + 0x128 + 4 * c, w);
    }
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x110 + 4 * c);
        fx.mem.write_u32(sp + 0x134 + 4 * c, w);
    }
    blib::bolt::jag(fx, sp + 0x120);
    let r = STROKES + ((i as u32) << 5);
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0x86);
    let n = fx.mem.read_u8(sp + 0x120);
    fx.mem.write_u8(r + 8, n);
    fx.mem.write_u32(r + 0x14, 0x3E4C_CCCD);
    for (o, v) in [(0xFu32, 0u8), (9, 5), (0xA, 0), (0xB, 2), (0xC, 7), (0xD, 3), (0xE, 3), (0x18, 0x60), (0x19, 0x60), (0x1A, 0x60), (0x1B, 0x20)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x1C, path);
}

/// Updates 0x32 / 0x42 / 0x56 [F 0x1E058DC]. Returns the entry's s0.
fn strike(fx: &mut Fx, sp: u32, s1: u64) -> u64 {
    let i = first_free(fx, STROKES, 0x20, 0x1E);
    if i < 0 {
        return i as i64 as u64;
    }
    let s0 = i as i64 as u64;
    node(fx, 0x10, sp + 0xE0, s0, s1, 0x01E0_590C);
    node(fx, 0x14, sp + 0xF0, s0, s1, 0x01E0_5920);
    let n = fx.mem.u16(D);
    let (count, f20) = match n {
        0x56 => {
            lerp3(fx, sp + 0xF0, sp + 0xE0, 0.5, &[sp + 0x110, sp + 0x100]);
            (0x12, 0.0)
        }
        0x42 => {
            copy_v4(fx, sp + 0xE0, sp + 0x110);
            copy_v4(fx, sp + 0x110, sp + 0x100);
            (0xC, 40.0)
        }
        _ => {
            copy_v4(fx, sp + 0xF0, sp + 0x110);
            copy_v4(fx, sp + 0x110, sp + 0x100);
            (0xC, -40.0)
        }
    };
    let x = fx.mem.f(sp + 0x100);
    let x = if fx.mem.read_u32(D + 0x30) == 0x8000 { ee::add(x, f20) } else { ee::sub(x, f20) };
    fx.mem.setf(sp + 0x100, x);
    let y = ee::add(fx.mem.f(sp + 0x104), 40.0);
    fx.mem.setf(sp + 0x104, y);
    set_counter4(fx, FADE, count, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    stroke(fx, sp, i);
    let f = first_free(fx, FLASH, 0x4C, 0x14);
    let mut s2: u64 = 0;
    if f >= 0 {
        let r = FLASH + 0x4C * f as u32;
        set_counter4(fx, r, 0xC, 8, 0xFF);
        set(fx, r, &[(5, 0), (4, 0x80), (7, 9), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0), (9, 0), (8, 0x80)], &[(0x14, 0), (0x10, 0x4248_0000), (0x14, 0x4220_0000), (0x18, 0x4120_0000)]);
        copy_v4(fx, sp + 0x110, r + 0x1C);
    }
    let mut s0 = s0;
    while s2 < 0x32 {
        let j = first_free(fx, DROPS, 0x44, 0x12C);
        s0 = j as i64 as u64;
        if j >= 0 {
            let delay = rand4(fx, s0, 0x01E0_5D20) & 7;
            let r = DROPS + 0x44 * j as u32;
            s0 = u64::from(r);
            set_counter4(fx, r, 0x18, delay, 0xFF);
            set(fx, r, &[(4, 0x40), (8, 4), (0xB, 2), (0xA, 0)], &[]);
            let v = (rand4(fx, s0, 0x01E0_5D68) & 3) + 2;
            fx.mem.set8(r + 9, v);
            set(fx, r, &[(0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)], &[(0x10, 0x41F0_0000), (0x14, 0xBECC_CCCD), (0x18, 0xBD75_C28F), (0x1C, 0)]);
            for c in 0..3u32 {
                let w = fx.mem.read_u32(sp + 0x110 + 4 * c);
                fx.mem.write_u32(r + 0x20 + 4 * c, w);
            }
            let v = jit(fx, s0, 0x01E0_5DCC, 0x96, 0x4B);
            fx.mem.setf(r + 0x2C, v);
            let v = ee::add(f32::from_bits(0x3F33_3333), jit(fx, s0, 0x01E0_5E08, 100, 0x32));
            fx.mem.setf(r + 0x30, v);
            let v = jit(fx, s0, 0x01E0_5E50, 0x96, 0x4B);
            fx.mem.setf(r + 0x34, v);
        }
        s2 += 1;
    }
    shake(fx, 0xE, 2, 0x4100_0000);
    s0
}

/// Update 0x168 [F 0x1E061C4]: the fade, both windows, two spikes, a
/// flash, the dome and 300 drops at the target. Returns s0.
fn finale(fx: &mut Fx, sp: u32) -> u64 {
    set_counter4(fx, FADE, 4, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    fx.mem.set16(FLAT, 0x168);
    fx.mem.set16(FLAT + 2, 0x17E);
    fx.mem.write_u32(FLAT + 8, 0x3F80_0000);
    set(fx, FLAT, &[(0xE, 0x80), (0xD, 0x60), (0xC, 0x60), (0xF, 0x80)], &[]);
    fx.mem.set16(WINDOW, 0x168);
    fx.mem.set16(WINDOW + 2, 0x190);
    fx.mem.write_u8(WINDOW + 5, 6);
    fx.mem.write_u8(WINDOW + 6, 2);
    fx.mem.write_u32(WINDOW + 8, 0);
    window_corners(fx, 8);
    for o in [0x2Eu32, 0x2D, 0x2C] {
        fx.mem.write_u8(WINDOW + o, 0x50);
    }
    for s0 in 0..2u64 {
        let i = first_free(fx, SPIKES, 0x68, 0x14);
        let r = SPIKES.wrapping_add((0x68 * i) as u32);
        set_counter4(fx, r, 0x18, 0, 0xFF);
        set(fx, r, &[(4, 5), (0xA, 0), (0xC, 3)], &[(0x14, 0x40A0_0000), (0x1C, 0x41F0_0000)]);
        set(fx, r, &[(0x23, 0), (0x27, 0x20), (8, 0xA0)], &[(0x58, 0x40), (0x5C, 0x80), (0x60, 0x100)]);
        set(fx, r, &[(5, 0), (0xB, 2)], &[]);
        let v = rand4(fx, s0, 0x01E0_6304);
        fx.mem.set8(r + 9, v);
        fx.mem.write_u32(r + 0x10, BYTES);
        set(fx, r, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80)], &[]);
        copy_v4(fx, sp + 0xD0, r + 0x28);
    }
    let i = first_free(fx, FLASH, 0x4C, 0x14);
    let r = FLASH.wrapping_add((0x4C * i) as u32);
    let s0 = u64::from((0x4C * i) as u32);
    set_counter4(fx, r, 0x20, 0, 0xFF);
    set(fx, r, &[(5, 0), (4, 0x80), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x10), (8, 0x40)], &[(0x14, 0), (0x10, 0), (0x14, 0x42C8_0000), (0x18, 0x41A0_0000)]);
    copy_v4(fx, sp + 0xD0, r + 0x1C);
    set_counter4(fx, DOME, 0x18, 0, 0xFF);
    set(fx, DOME, &[(0xA, 0xD), (7, 9), (8, 1), (9, 2)], &[(0xC, 0x40A0_0000), (0x10, 0xBD23_D70A), (0x14, 0), (0x18, 0x4100_0000)]);
    copy_v4(fx, sp + 0xD0, DOME + 0x20);
    set(fx, DOME, &[(0x1C, 0x80), (0x1D, 0x80), (0x1E, 0x80), (0x1F, 0x80)], &[]);
    copy_v4(fx, sp + 0xD0, DOME + 0x20);
    dome_side(fx, true);
    let mut s0 = s0;
    for _ in 0..0x12C {
        let j = first_free(fx, DROPS, 0x44, 0x12C);
        s0 = j as i64 as u64;
        if j < 0 {
            continue;
        }
        let delay = rand4(fx, s0, 0x01E0_6534) & 0xF;
        let r = DROPS + 0x44 * j as u32;
        s0 = u64::from(r);
        set_counter4(fx, r, 0x24, delay, 0xFF);
        set(fx, r, &[(4, 0x40), (8, 0), (0xB, 2), (0xA, 0)], &[]);
        let v = (rand4(fx, s0, 0x01E0_6578) & 3) + 2;
        fx.mem.set8(r + 9, v);
        set(fx, r, &[(0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)], &[(0x10, 0x42C8_0000), (0x14, 0xBF80_0000), (0x18, 0xBD75_C28F), (0x1C, 0)]);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xD0 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let v = jit(fx, s0, 0x01E0_65D8, 0x140, 0xA0);
        fx.mem.setf(r + 0x2C, v);
        let v = ee::add(1.0, jit(fx, s0, 0x01E0_6614, 0x118, 0x8C));
        fx.mem.setf(r + 0x30, v);
        let v = jit(fx, s0, 0x01E0_6658, 0x140, 0xA0);
        fx.mem.setf(r + 0x34, v);
    }
    s0
}

/// The dome's turn (+0x30) and z (+0x28 -+ +0x14 by D+0x30).
fn dome_side(fx: &mut Fx, turn: bool) {
    let r = fx.mem.f(DOME + 0x14);
    if fx.mem.read_u32(D + 0x30) == 0x8000 {
        if turn {
            fx.mem.set32(DOME + 0x30, -0x4000);
        }
        let z2 = ee::sub(fx.mem.f(DOME + 0x28), r);
        fx.mem.setf(DOME + 0x28, z2);
    } else {
        if turn {
            fx.mem.set32(DOME + 0x30, 0x4000);
        }
        let z2 = ee::add(fx.mem.f(DOME + 0x28), r);
        fx.mem.setf(DOME + 0x28, z2);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    // The entry's s0 / s1 as the library prologues and rand save them
    // (the dispatch's s1 never reaches a save: [`node`] calls set it).
    let mut s0 = param as i64 as u64;
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xD0, s0, 0x01E0_5790);
    match fx.mem.u16(D) {
        0x1C0 => return 8,
        0x168 => s0 = finale(fx, sp),
        0x120 => {
            for k in 0..10u32 {
                let r = DROPS + 0x44 * k;
                set_counter4(fx, r, 0x50, 0, 0xFF);
                set(fx, r, &[(4, 0x81), (8, 3), (0xB, 2)], &[]);
                let v = rand4(fx, u64::from(r - 0xEB4), 0x01E0_6130) & 7;
                fx.mem.set8(r + 9, v);
                set(fx, r, &[(0xA, 5), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x40)], &[]);
                let w = ee::sub(220.0, ee::mul(10.0, ee::from_i32(k as i32)));
                fx.mem.setf(r + 0x10, w);
                for o in [0x14u32, 0x1C, 0x18, 0x40, 0x3C, 0x38, 0x34, 0x30, 0x2C] {
                    fx.mem.write_u32(r + o, 0);
                }
            }
            s0 = u64::from(D + 0x44 * 10);
        }
        0x118 => fx.mem.write_u32(FLASH + 0x14, 0x4396_0000),
        0xF0 => {
            let r = SPIKES;
            set_counter4(fx, r, 0x37, 0, 0xFF);
            set(fx, r, &[(4, 5), (0xD, 0), (0xA, 0), (0xC, 3)], &[(0x14, 0x3F4C_CCCD), (0x1C, 0x420C_0000)]);
            set(fx, r, &[(0x23, 0), (0x27, 0x40), (8, 0x78)], &[(0x58, 0x40), (0x5C, 0x80), (0x60, 0x100)]);
            set(fx, r, &[(5, 0), (0xB, 2)], &[]);
            let v = rand4(fx, s0, 0x01E0_6054);
            fx.mem.set8(r + 9, v);
            fx.mem.write_u32(r + 0x10, BYTES);
            set(fx, r, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80)], &[]);
            set_counter4(fx, FLASH, 0x37, 0, 0xFF);
            set(fx, FLASH, &[(5, 0), (4, 0x81), (7, 9), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x80), (9, 0x10), (8, 0x40)], &[(0x14, 0), (0x10, 0), (0x14, 0x4120_0000)]);
        }
        0x8C => {
            let r = QUADS;
            set_counter4(fx, r, 0xFF, 0, 0xFF);
            set(fx, r, &[(4, 0x80), (8, 1), (9, 8), (0xA, 2)], &[(0xC, 0x3F80_0000), (0x10, 0x4000_0000), (0x14, 0), (0x18, 0xBD23_D70A)]);
            set(fx, r, &[(0x1F, 0x80), (0x1C, 0x80), (0x1D, 0x80), (0x1E, 0x80)], &[]);
            s0 = u64::from(D + 0x50);
        }
        0x70 => {
            set_counter4(fx, FADE, 0x12, 0, 0xFF);
            fx.mem.write_u8(FADE + 4, 1);
            shake(fx, 0xF8, 8, 0x40C0_0000);
        }
        0x56 | 0x42 | 0x32 => s0 = strike(fx, sp, RUNNER_S1),
        0x68 | 0x4E => {
            set_counter4(fx, BLACK, 8, 0, 0xFF);
            fx.mem.write_u8(BLACK + 4, 2);
        }
        0x3E => {
            set_counter4(fx, BLACK, 4, 0, 0xFF);
            fx.mem.write_u8(BLACK + 4, 2);
        }
        2 => {
            fx.mem.set16(WINDOW, 2);
            fx.mem.set16(WINDOW + 2, 0x30);
            fx.mem.write_u8(WINDOW + 5, 6);
            fx.mem.write_u8(WINDOW + 6, 1);
            fx.mem.write_u32(WINDOW + 8, 0);
            for o in [0x2Cu32, 0x2D, 0x2E] {
                fx.mem.write_u8(WINDOW + o, 0x80);
            }
            window_corners(fx, 1);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0xF0..0x12D).contains(&n) {
        let f20 = ee::div(ee::from_i32(n - 0xF0), 60.0);
        let s1 = u64::from(sp + 0xF0);
        node(fx, 0x10, sp + 0xF0, s0, s1, 0x01E0_6704);
        s0 = u64::from(sp + 0x100);
        node(fx, 0x14, sp + 0x100, s0, s1, 0x01E0_6718);
        lerp3(fx, sp + 0x100, sp + 0xF0, f20, &[SPIKES + 0x28]);
        copy_v4(fx, SPIKES + 0x28, FLASH + 0x1C);
    }
    s0 = u64::from(sp + 0x110);
    for s3 in 0..4u32 {
        let q = QUADS + 0x50 * s3;
        let s1 = u64::from(D + 0x50 * s3);
        if fx.mem.u8(q) <= 0 {
            continue;
        }
        node(fx, 0x18, sp + 0xE0, s0, s1, 0x01E0_67AC);
        node(fx, 0x14, sp + 0xF0, s0, s1, 0x01E0_67BC);
        node(fx, 0x10, sp + 0x100, s0, s1, 0x01E0_67CC);
        node(fx, 0x1C, sp + 0x110, s0, s1, 0x01E0_67DC);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xE0 + 4 * c);
            fx.mem.write_u32(q + 0x20 + 4 * c, w);
        }
        lerp3(fx, sp + 0xE0, sp + 0x100, f32::from_bits(0x4019_999A), &[q + 0x2C]);
        lerp3(fx, sp + 0xE0, sp + 0xF0, f32::from_bits(0x3F59_999A), &[q + 0x38]);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x110 + 4 * c);
            fx.mem.write_u32(q + 0x44 + 4 * c, w);
        }
        lerp3(fx, sp + 0x110, q + 0x2C, f32::from_bits(0x3F4C_CCCD), &[q + 0x2C]);
    }
    let mut s1 = u64::from(D + 0x140);
    if fx.mem.u8(DOME) > 0 {
        copy_v4(fx, sp + 0xD0, DOME + 0x20);
        dome_side(fx, false);
    }
    let n = fx.mem.u16(D);
    if (0x70..0x8C).contains(&n) && n & 1 != 0 {
        s1 = trail_stroke(fx, sp, s0, s1);
    }
    let n = fx.mem.u16(D);
    if (0x8C..0x168).contains(&n) {
        (s0, s1) = bolts(fx, sp, s0);
    }
    let n = fx.mem.u16(D);
    if (0x118..0x168).contains(&n) {
        node(fx, 0x10, sp + 0xE0, s0, s1, 0x01E0_70BC);
        node(fx, 0x14, sp + 0xF0, s0, s1, 0x01E0_70CC);
        for k in 0..10u32 {
            let f = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(k as i32));
            lerp3(fx, sp + 0xF0, sp + 0xE0, f, &[DROPS + 0x44 * k + 0x20]);
        }
    }
    shake_saving(fx, SHAKE, s0, 0x01E0_7154, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_7160, [Some(s0), Some(s1)]);
    let ot = fx.mem.read_u32(sp + 0xB8);
    blib::fade::fade(fx, FADE, ot);
    blib::fade::fade_black(fx, BLACK, ot);
    let (_, flash_a3) = blib::star::flash_b_a3(fx, 0x01E0_2240, FLASH, ot, 0x14);
    let (_, spike_a3) = blib::spike::spikes_a3(fx, 0x01E0_2CB0, SPIKES, ot, 0x14);
    let (_, drop_a3) = blib::drop::drops_a3(fx, DROPS, ot, 0x12C);
    let (_, bolt_a3) = blib::bolt::bolts_b_a3(fx, 0x01E0_0BE0, BOLTS, ot, 0x1E);
    blib::dash::strokes(fx, 0x01E0_1670, STROKES, ot, 0x1E);
    let n = fx.mem.u16(D);
    // The a3 the last drawing call left [run]: the bolts', the drops', the
    // spikes', the flashes' (the strokes never count in either window:
    // [H] not chained), else ([H]) 0.
    let a3 = bolt_a3.or(drop_a3).or(spike_a3).or(flash_a3).unwrap_or(0);
    blib::window::frame_fade_b(fx, 0x01E0_40F0, WINDOW, ot, n, a3);
    let n = fx.mem.u16(D);
    blib::window::flat(fx, 0x01E0_4570, FLAT, ot, n);
    blib::dome::domes(fx, 0x01E0_46F0, DOME, ot, 2);
    blib::scrollquad::quads(fx, 0x01E0_5130, QUADS, ot, 4);
    let n = fx.mem.u16(D);
    if (0x168..0x18D).contains(&n) {
        6
    } else if n == 0x165 {
        2
    } else if n == 0x15E {
        1
    } else {
        0
    }
}

/// Odd updates 0x71..0x8B [F 0x1E06A34]: a stroke from a point between
/// nodes +0x14 / +0x10 (x jittered, 40 up) and a flash there. Returns s1.
fn trail_stroke(fx: &mut Fx, sp: u32, s0: u64, s1: u64) -> u64 {
    let f20 = ee::add(0.5, jit(fx, s0, 0x01E0_6A3C, 100, 0x32));
    let i = first_free(fx, STROKES, 0x20, 0x1E);
    if i < 0 {
        return s1;
    }
    node(fx, 0x10, sp + 0xE0, s0, s1, 0x01E0_6AAC);
    node(fx, 0x14, sp + 0xF0, s0, s1, 0x01E0_6ABC);
    lerp3(fx, sp + 0xF0, sp + 0xE0, f20, &[sp + 0x110, sp + 0x100]);
    let v = rand4(fx, s0, 0x01E0_6B28);
    let (_, r) = fx.div(v, 0x3C);
    fx.mem.setf(sp + 0x100, ee::div(ee::from_i32(200 * r - 0x1770), 100.0));
    let y = ee::add(fx.mem.f(sp + 0x104), 40.0);
    fx.mem.setf(sp + 0x104, y);
    stroke(fx, sp, i);
    let mut s1 = u64::from((i as u32) << 5);
    let f = first_free(fx, FLASH, 0x4C, 0x50);
    if f >= 0 {
        s1 = u64::from(0x4C * f as u32);
        let r = FLASH + 0x4C * f as u32;
        set_counter4(fx, r, 0xC, 8, 0xFF);
        set(fx, r, &[(5, 0), (4, 0x83), (7, 9), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0), (9, 0), (8, 0x80)], &[(0x14, 0), (0x10, 0x4248_0000), (0x14, 0x41F0_0000)]);
        copy_v4(fx, sp + 0x110, r + 0x1C);
    }
    s1
}

/// Updates 0x8C..0x167 [F 0x1E06DBC]: for each tenth k of the line from
/// node +0x14 to +0x10, three bolts (+0x7774 + 0x40 (k + 10 j)) placed
/// there, the free ones armed (2 counts, jittered ends). Returns (s0, s1).
fn bolts(fx: &mut Fx, sp: u32, s0_in: u64) -> (u64, u64) {
    let mut s0 = s0_in;
    node(fx, 0x10, sp + 0xE0, s0, u64::from(D + 0x140), 0x01E0_6DCC);
    node(fx, 0x14, sp + 0xF0, s0, u64::from(D + 0x140), 0x01E0_6DDC);
    let mut s1 = 0u32;
    fx.mem.set32(sp + 0xC0, 0);
    while fx.mem.i32(sp + 0xC0) < 10 {
        let k = fx.mem.i32(sp + 0xC0);
        s1 = k as u32;
        let s7 = k & 1;
        let f20 = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(k));
        for _ in 0..3 {
            lerp3(fx, sp + 0xF0, sp + 0xE0, f20, &[sp + 0x100]);
            let b = BOLTS + (s1 << 6);
            if fx.mem.read_u8(b) == 0 {
                set_counter4(fx, b, 2, s7 & 0xFF, 0xFF);
                fx.mem.write_u8(b + 5, 0);
                fx.mem.write_u8(b + 4, 0xC4);
                let m = if fx.mem.u16(D) < 0x121 {
                    let m = (rand4(fx, s0, 0x01E0_6EA8) & 3) + 1;
                    s0 = m as u64;
                    fx.mem.write_u8(b + 0xB, 5);
                    fx.mem.write_u32(b + 0x10, 0x3F80_0000);
                    fx.mem.write_u32(b + 0x14, 0x3E99_999A);
                    fx.mem.write_u8(b + 8, 6);
                    m
                } else {
                    let m = (rand4(fx, s0, 0x01E0_6EE4) & 7) + 6;
                    s0 = m as u64;
                    fx.mem.write_u8(b + 0xB, 2);
                    fx.mem.write_u32(b + 0x10, 0x3F80_0000);
                    fx.mem.write_u32(b + 0x14, 0x3ECC_CCCD);
                    fx.mem.write_u8(b + 8, 8);
                    m
                };
                let half = (200 * m) >> 1;
                for (c, ra) in [(0u32, 0x01E0_6F20), (4, 0x01E0_6F88), (8, 0x01E0_6FDC)] {
                    let v = rand4(fx, s0, ra);
                    let (_, r) = fx.div(v, m);
                    let p = fx.mem.f(sp + 0x100 + c);
                    let v = ee::add(p, ee::div(ee::from_i32(200 * r - half), 100.0));
                    fx.mem.setf(b + 0x28 + c, v);
                }
                fx.mem.write_u8(b + 0xD, 3);
                fx.mem.write_u8(b + 0xC, 0);
                for o in [0x18u32, 0x19, 0x1A] {
                    fx.mem.write_u8(b + o, 0x80);
                }
                fx.mem.write_u8(b + 0x1B, 0x30);
            }
            for c in 0..3u32 {
                let w = fx.mem.read_u32(sp + 0x100 + 4 * c);
                fx.mem.write_u32(b + 0x1C + 4 * c, w);
            }
            s1 += 0xA;
        }
        fx.mem.set32(sp + 0xC0, k + 1);
    }
    (s0, u64::from(s1))
}
