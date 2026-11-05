//! BMODULE b645 (card 608), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E043E0, data area a0 = 0x1ED0000, 0xB840 bytes)
//! [F]: b051's family (s3 = D, fp = the OT, s0
//! the parameter then indices / offsets). Update 2 reads nodes 1 / 0x32
//! (+0xB730, y from 0x32); 8 a long flash (+0x771C), a spike (+0xA4), a
//! long shake and the comet (+0x8E14, `comet::comets_b`) at node 0x31; 0x78
//! a shake at the target. On even updates 8..0x6E a spike at node 0x31;
//! on every eighth update 0x41..0xB9 a bolt (+0x9138, 0x54 apart,
//! `arch::arches_b`) along 21 points (+0x9B10, 240 bytes apart, 0x102F20)
//! from node 0x32 to the target jittered; each bolt walks its points and on
//! its 19th step bursts at its end: three panels (+0x8004,
//! `panel::panels`), a flash, a spike and 25 bouncing sprites. Returns 6
//! on updates 0x55..0xD9, 2 on 0xDA, 1 on 0x4B, 8 on 0xFA (no drawing
//! then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash_b`
//! 0x1E003A0, `arches_b` 0x1E00E10, `spikes` 0x1E01670, `drops`
//! 0x1E02150, `fade` 0x1E02AB0, `panels` 0x1E02F80, `comets_b` 0x1E03490
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b173::arc_points;
use crate::blib::{self, exe::{aim_angles, first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SPIKES: u32 = D + 0xA4;
const DROPS: u32 = D + 0xCD4;
const FADE: u32 = D + 0x7714;
const FLASH: u32 = D + 0x771C;
const PANELS: u32 = D + 0x8004;
const COMETS: u32 = D + 0x8E14;
const BOLTS: u32 = D + 0x9138;
const POINTS: u32 = D + 0x9B10;
const BYTES: u32 = D + 0xB740;

/// The entry 0x1E043E0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x130);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x130);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0xB840).
    fx.bzero(D, 0xB840);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x32, 0), (0x14, 0x31, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xF4);
    // The entry's s0 = 2 (the texture loop's), s1 / s2 the runner's 0.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_44F4, [Some(2), Some(0), Some(0)]);
    0
}

/// 0x101940 `aim_angles(v, out)` leaving its second atan2f's 0x1A5020
/// frame: ra 0x1019B4 at the caller's sp - 0x38 (as b173).
fn aim_saving(fx: &mut Fx, v: u32, out: u32) {
    aim_angles(fx, v, out);
    let sp = fx.sp;
    fx.mem.write_u64(sp - 0x38, 0x0010_19B4);
}

/// rand from return address `ra` with the entry's s0 (64 bits) saved in
/// its frame.
fn rand64(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand()
}

/// ((rand >> 4) % m - half) / 100.
fn jit(fx: &mut Fx, s0: u64, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand64(fx, s0, ra) >> 4;
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

/// (200 ((rand >> 4) % m) - half) / 100 (the bolts' jitter).
fn spread(fx: &mut Fx, s0: u64, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand64(fx, s0, ra) >> 4;
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(200 * r - half), 100.0)
}

/// The s registers the step's calls save.
struct Regs {
    s0: u64,
    s1: u64,
    s2: u64,
    s6: u64,
    /// a0's upper 64 bits (for the shake's saves).
    a0_hi: u64,
}

fn sx(v: i32) -> u64 {
    i64::from(v) as u64
}

/// Update 8 [F 0x1E04598]: a long flash, a spike, a long shake and the
/// comet at node 0x31.
fn charge(fx: &mut Fx, r: &Regs) {
    set_counter4(fx, FLASH, 0x10, 0, 0xFF);
    fx.mem.write_u8(FLASH + 5, 1);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(FLASH + 6, b);
    for (o, v) in [(4u32, 0x80u8), (7, 9), (0xA, 0x80), (9, 0x40), (8, 0), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0)] {
        fx.mem.write_u8(FLASH + o, v);
    }
    fx.mem.write_u32(FLASH + 0x10, 0x41A0_0000);
    fx.mem.write_u32(FLASH + 0x14, 0x4170_0000);
    fx.mem.write_u32(FLASH + 0x18, 0x4120_0000);
    let s = SPIKES;
    set_counter4(fx, s, 0x10, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(s + 6, b);
    for (o, v) in [(4u32, 5u8), (0xA, 0), (0xC, 4)] {
        fx.mem.write_u8(s + o, v);
    }
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x1C, 0x4190_0000);
    fx.mem.write_u8(s + 0x23, 0);
    fx.mem.write_u8(s + 0x27, 0x60);
    fx.mem.write_u8(s + 8, 0x28);
    fx.mem.write_u32(s + 0x58, 0);
    fx.mem.write_u32(s + 0x5C, 0);
    fx.mem.write_u32(s + 0x60, 0x300);
    fx.mem.write_u8(s + 0xB, 2);
    let x = rand64(fx, r.s0, 0x01E0_467C);
    fx.mem.set8(s + 9, x >> 4);
    fx.mem.write_u32(s + 0x10, BYTES);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(s + o, 0x80);
    }
    set_counter4(fx, D + 0x40, 0xFF, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 1);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0x46, b);
    fx.mem.write_u8(D + 0x44, 0);
    fx.mem.write_u32(D + 0x54, 0x4000_0000);
    fx.mem.write_u32(D + 0x58, 0x3FCC_CCCD);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x4080_0000);
    let c = COMETS;
    set_counter4(fx, c, 0xFF, 0, 0xFF);
    fx.mem.write_u8(c + 5, 1);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(c + 6, b);
    fx.mem.write_u8(c + 4, 4);
    fx.mem.write_u32(c + 8, D + 0x8E58);
    fx.mem.write_u32(c + 0x28, 0x28);
    for (o, v) in [(0x1Eu32, 8u8), (0x1F, 0), (0x21, 2), (0x22, 5), (0x23, 4), (0x34, 0x80), (0x35, 0x80), (0x36, 0x80), (0x37, 0x60)] {
        fx.mem.write_u8(c + o, v);
    }
    fx.mem.write_u32(c + 0x2C, 0x4316_0000);
    fx.mem.write_u32(c + 0x30, 0x41F0_0000);
    let n = u32::from(fx.mem.read_u8(c + 0x1E)) + 1;
    for k in 0..n {
        fx.mem.write_u16(c + 0xC + 2 * k, 0xFFFF);
    }
}

/// Even updates 8..0x6E [F 0x1E0490C]: a spike at node 0x31.
fn sparkle(fx: &mut Fx, r: &mut Regs) {
    let i = first_free(fx, SPIKES, 0x68, 0x1E);
    if i < 0 {
        return;
    }
    r.s0 = u64::from(0x68 * i as u32);
    let q = SPIKES + 0x68 * i as u32;
    set_counter4(fx, q, 0x10, 0, 0xFF);
    fx.mem.write_u8(q + 4, 5);
    fx.mem.write_u32(q + 0x14, 0x3F99_999A);
    fx.mem.write_u32(q + 0x1C, 0x4170_0000);
    fx.mem.write_u8(q + 8, 0xA);
    fx.mem.write_u8(q + 5, 1);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(q + 6, b);
    for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 3)] {
        fx.mem.write_u8(q + o, v);
    }
    let x = rand64(fx, r.s0, 0x01E0_49A0);
    fx.mem.set8(q + 9, x >> 4);
    fx.mem.write_u32(q + 0x10, BYTES);
    fx.mem.write_u32(q + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x10), (0x27, 0x40)] {
        fx.mem.write_u8(q + o, v);
    }
    for (o, ra) in [(0x58u32, 0x01E0_49F0u32), (0x5C, 0x01E0_4A04), (0x60, 0x01E0_4A18)] {
        let x = rand64(fx, r.s0, ra);
        fx.mem.set32(q + o, (x >> 4) & 0x7F);
    }
}

/// Every eighth update 0x41..0xB9 [F 0x1E04A50]: a bolt from node 0x32 to
/// the target (sp + 0xA0) jittered.
fn bolt(fx: &mut Fx, sp: u32, r: &mut Regs) {
    let i = first_free(fx, BOLTS, 0x54, 0x1E);
    r.s0 = sx(i);
    if i < 0 {
        return;
    }
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, s, sp + 0xB0, r.s0, Some(r.s1), 0x01E0_4A80);
    r.a0_hi = 0;
    fx.mem.set32(sp + 0xE0, 0x14);
    let f = ee::add(20.0, spread(fx, r.s0, 0x01E0_4A8C, 0xA, 0x3E8));
    fx.mem.setf(sp + 0xE4, f);
    let f = spread(fx, r.s0, 0x01E0_4AE4, 0xB4, 0x4650);
    fx.mem.setf(sp + 0xE8, f);
    let s1 = 240 * i as u32;
    r.s1 = u64::from(s1);
    fx.mem.write_u32(sp + 0x108, POINTS + s1);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0xB0 + 4 * c);
        fx.mem.write_u32(sp + 0xF0 + 4 * c, w);
    }
    for (c, ra) in [(0u32, 0x01E0_4B5Cu32), (1, 0x01E0_4BB4)] {
        let f = ee::add(fx.mem.f(sp + 0xA0 + 4 * c), spread(fx, r.s0, ra, 0xA, 0x3E8));
        fx.mem.setf(sp + 0xFC + 4 * c, f);
    }
    // z: a signed remainder by 4 without `div` (LO / HI kept).
    let v = rand64(fx, r.s0, 0x01E0_4C0C) >> 4;
    let m = v.wrapping_rem(4);
    let f = ee::add(fx.mem.f(sp + 0xA8), ee::div(ee::from_i32(200 * m - 400), 100.0));
    fx.mem.setf(sp + 0x104, f);
    arc_points(fx, sp + 0xE0);
    let s0 = 0x54 * i as u32;
    r.s0 = u64::from(s0);
    r.s2 = u64::from(D + s0);
    let b = BOLTS + s0;
    set_counter4(fx, b, 0x14, 0, 0xFF);
    for (o, v) in [(4u32, 0x40u8), (5, 0), (7, 6), (0xE, 0xC0), (0xD, 0x60), (0xC, 0), (0xF, 0x20), (0xB, 0xC0), (8, 0xC0), (9, 0xC0), (0xA, 0xC0)] {
        fx.mem.write_u8(b + o, v);
    }
    fx.mem.write_u32(b + 0x10, 0x4120_0000);
    fx.mem.write_u32(b + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(b + 0x18, 0);
    fx.mem.write_u32(b + 0x1C, 0x800);
    let v = rand64(fx, r.s0, 0x01E0_4D7C) >> 4;
    fx.mem.set32(b + 0x20, v & 0x3FF);
    let p = POINTS + s1;
    for c in 0..3u32 {
        let w = fx.mem.read_u32(p + 4 * c);
        fx.mem.write_u32(b + 0x24 + 4 * c, w);
    }
    for o in [0x34u32, 0x38, 0x3C] {
        fx.mem.write_u32(b + o, 0);
    }
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(p + 0xC + 4 * c), fx.mem.f(b + 0x24 + 4 * c));
        fx.mem.setf(sp + 0x110 + 4 * c, v);
    }
    aim_saving(fx, sp + 0x110, b + 0x44);
}

/// A bolt's end [F 0x1E04F6C]: three panels, a flash, a spike and 25
/// bouncing sprites at its last point (sp + 0xB0).
fn burst(fx: &mut Fx, sp: u32, s7: u32, r: &mut Regs) {
    let b = BOLTS + 0x54 * s7;
    let p = POINTS + 240 * s7;
    fx.mem.write_u8(b, 0);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(p + 0xE4 + 4 * c);
        fx.mem.write_u32(sp + 0xB0 + 4 * c, w);
    }
    r.s0 = u64::from(sp + 0xB4);
    r.s1 = u64::from(sp + 0xB8);
    for s2 in 0..3i32 {
        r.s2 = s2 as u64;
        let i = first_free(fx, PANELS, 0x3C, 0x3C);
        if i < 0 {
            continue;
        }
        let q = PANELS + 0x3C * i as u32;
        set_counter4(fx, q, 0xE, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 1), (0xA, 0), (9, 6), (0xB, 2), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        copy_v4(fx, sp + 0xB0, q + 0x2C);
        let dy = ee::add(-1.5, ee::mul(1.5, ee::from_i32(s2)));
        let y = ee::add(fx.mem.f(q + 0x30), dy);
        fx.mem.setf(q + 0x30, y);
        fx.mem.write_u32(q + 0x10, 0x3FC0_0000);
        fx.mem.write_u32(q + 0x14, 0x3F80_0000);
        fx.mem.write_u32(q + 0x18, if s2 == 1 { 0x4090_0000 } else { 0x4060_0000 });
        let t = fx.mem.read_u32(b + 0x1C);
        fx.mem.write_u32(q + 0x24, t);
        r.s6 = u64::from(q + 0x24);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let f = ee::mul(ee::from_i32(fx.mem.i32(q + 0x24)), -1.0);
            fx.fix_residue(f, 0x01E0_516C);
            fx.mem.set32(q + 0x24, ee::fixsfsi(f));
        }
    }
    r.s2 = 3;
    let i = first_free(fx, FLASH, 0x4C, 0x1E);
    if i >= 0 {
        r.s2 = u64::from(D + 0x4C * i as u32);
        let a = FLASH + 0x4C * i as u32;
        set_counter4(fx, a, 0x12, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0x80), (7, 9)] {
            fx.mem.write_u8(a + o, v);
        }
        fx.mem.write_u32(a + 0x10, 0x4348_0000);
        fx.mem.write_u32(a + 0x14, 0x41C8_0000);
        for (o, v) in [(0xCu32, 0xF0u8), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x80), (9, 0x20), (8, 0x60), (0xF, 0xC0)] {
            fx.mem.write_u8(a + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(a + o, 0);
        }
        copy_v4(fx, sp + 0xB0, a + 0x1C);
        fx.mem.write_u32(a + 0x18, 0x4170_0000);
    }
    let i = first_free(fx, SPIKES, 0x68, 0x1E);
    r.s6 = 0;
    if i >= 0 {
        r.s2 = u64::from(D + 0x68 * i as u32);
        let q = SPIKES + 0x68 * i as u32;
        set_counter4(fx, q, 0x10, 0, 0xFF);
        fx.mem.write_u8(q + 4, 5);
        fx.mem.write_u32(q + 0x14, 0x40A0_0000);
        fx.mem.write_u32(q + 0x1C, 0x4220_0000);
        for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0x28)] {
            fx.mem.write_u8(q + o, v);
        }
        let x = rand64(fx, r.s0, 0x01E0_52D0);
        fx.mem.set8(q + 9, x >> 4);
        fx.mem.write_u32(q + 0x10, BYTES);
        fx.mem.write_u32(q + 0x18, 0x3F80_0000);
        for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        copy_v4(fx, sp + 0xB0, q + 0x28);
        fx.mem.write_u32(q + 0x58, 0x40);
        fx.mem.write_u32(q + 0x5C, 0x80);
        fx.mem.write_u32(q + 0x60, 0x100);
    }
    for k in 0..0x19u64 {
        r.s6 = k;
        let i = first_free(fx, DROPS, 0x44, 0x190);
        r.s2 = sx(i);
        if i < 0 {
            continue;
        }
        r.s2 = u64::from(D + 0x44 * i as u32);
        let delay = (rand64(fx, r.s0, 0x01E0_5374) >> 4) & 7;
        let q = DROPS + 0x44 * i as u32;
        set_counter4(fx, q, 0x20, delay, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 0), (0xB, 2), (0xA, 0), (9, 3), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        let f = ee::add(90.0, spread(fx, r.s0, 0x01E0_53D0, 0x1E, 0xBB8));
        fx.mem.setf(q + 0x10, f);
        fx.mem.write_u32(q + 0x14, 0xBECC_CCCD);
        fx.mem.write_u32(q + 0x18, 0xBD75_C28F);
        fx.mem.write_u32(q + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xB0 + 4 * c);
            fx.mem.write_u32(q + 0x20 + 4 * c, w);
        }
        let f = jit(fx, r.s0, 0x01E0_5460, 0xD2, 0x69);
        fx.mem.setf(q + 0x2C, f);
        let f = jit(fx, r.s0, 0x01E0_549C, 0xD2, 0x69);
        fx.mem.setf(q + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), f));
        let f = jit(fx, r.s0, 0x01E0_54E4, 0xD2, 0x69);
        fx.mem.setf(q + 0x34, f);
    }
    r.s6 = 0x19;
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut r = Regs { s0: sx(param), s1: 0, s2: 0, s6: 0, a0_hi: 0 };
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xA0, r.s0, 0x01E0_4514);
    match fx.mem.u16(D) {
        0xFA => return 8,
        0x78 => {
            set_counter4(fx, D + 0x40, 0x65, 0, 0xFF);
            fx.mem.write_u8(D + 0x45, 0);
            fx.mem.write_u8(D + 0x44, 8);
            fx.mem.write_u32(D + 0x54, 0x3FE6_6666);
            fx.mem.write_u32(D + 0x58, 0x3FB3_3333);
            fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
            fx.mem.write_u32(D + 0x70, 0);
            fx.mem.write_u32(D + 0x74, 0x42A0_0000);
            fx.mem.write_u32(D + 0x90, 0x4080_0000);
            copy_v4(fx, sp + 0xA0, D + 0x60);
        }
        8 => charge(fx, &r),
        2 => {
            let h = fx.mem.i32(D + 4);
            let s = fx.mem.i32(D + 0xC);
            node_position_saving(fx, h, s, D + 0xB730, r.s0, Some(r.s1), 0x01E0_4570);
            let h = fx.mem.i32(D + 4);
            let s = fx.mem.i32(D + 0x10);
            node_position_saving(fx, h, s, sp + 0xB0, r.s0, Some(r.s1), 0x01E0_4580);
            let y = fx.mem.read_u32(sp + 0xB4);
            fx.mem.write_u32(D + 0xB734, y);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (8..0x6F).contains(&n) && n & 1 == 0 {
        sparkle(fx, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0x41..0xBA).contains(&n) && n & 7 == 0 {
        bolt(fx, sp, &mut r);
    }
    for s7 in 0..0x1Eu32 {
        let b = BOLTS + 0x54 * s7;
        let c = fx.mem.u8(b);
        if c <= 0 {
            continue;
        }
        let k = fx.mem.u8(b + 2) - c;
        let p = POINTS + 240 * s7 + 12 * k as u32;
        for c in 0..3u32 {
            let w = fx.mem.read_u32(p + 4 * c);
            fx.mem.write_u32(b + 0x24 + 4 * c, w);
        }
        if k != 0x13 {
            for c in 0..3u32 {
                let v = ee::sub(fx.mem.f(p + 0xC + 4 * c), fx.mem.f(b + 0x24 + 4 * c));
                fx.mem.setf(sp + 0x120 + 4 * c, v);
            }
            aim_saving(fx, sp + 0x120, b + 0x44);
            continue;
        }
        burst(fx, sp, s7, &mut r);
    }
    let due = fx.mem.read_u8(D + 0x40) != 0 && fx.mem.read_u8(D + 0x41) == 0;
    shake_saving(fx, D + 0x40, r.s0, 0x01E0_5544, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    if due {
        fx.mem.write_u64(sp - 0x48, r.a0_hi);
        fx.mem.write_u64(sp - 0x38, r.a0_hi);
    }
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5550, [Some(r.s0), Some(r.s1)]);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the entry's s0..s6 at +0x10..+0x70 (s4 / s5
    // the bolt loop's ends) and ra at +0x80 [run: s5 is the comet tail's
    // missing-texture TEX1].
    let (s4, s5) = (u64::from(D + 0x54 * 0x1E), u64::from(D + 0xF0 * 0x1E));
    fx.saved(sp - 0xF0, &[(0x10, r.s0), (0x20, r.s1), (0x30, r.s2), (0x40, u64::from(D)), (0x50, s4), (0x60, s5), (0x70, r.s6), (0x80, 0x01E0_555C)]);
    blib::star::flash_b(fx, 0x01E0_03A0, FLASH, OT, 0x1E);
    blib::spike::spikes(fx, 0x01E0_1670, SPIKES, OT, 0x1E);
    blib::drop::drops(fx, DROPS, OT, 0x190);
    blib::arch::arches_b(fx, 0x01E0_0E10, BOLTS, OT, 0x1E);
    blib::panel::panels(fx, 0x01E0_2F80, PANELS, OT, 0x3C);
    blib::comet::comets_b(fx, 0x01E0_3490, COMETS, OT, 1);
    let n = fx.mem.u16(D);
    if (0x55..0xDA).contains(&n) {
        6
    } else if n == 0xDA {
        2
    } else if n == 0x4B {
        1
    } else {
        0
    }
}
