//! BMODULE b405 (card 490), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04810, data area a0 = 0x1ED0000, 0x8370 bytes)
//! [F]: s6 = D, s7 = the OT (0x206E40), s0 the
//! parameter then counters / pointers; compiled without scheduling (nop
//! delay slots). Update 2 two smears (+0x302C, `smear::smears`, nodes 0xC
//! -> 0xA and 0x16 -> 0x14, pool +0x30BC), at each of nodes 0xC / 0x16
//! four crossbars (+0xEBC, `cross::bars_b`, the 0x50 pool's first free
//! taken unchecked) and a long flash (+0x113C), and a long shake at node
//! 0xC; 88 a shake at the target. On updates 85..128 two arches
//! (+0x2654, `arch::arches_b`) a update from node 1 flying to the target
//! jittered; on every fourth update 88..124 at the target jittered three
//! panels (+0xAC, `panel::panels`), a flash, a spike (+0x1A24) and 30
//! bouncing sprites (+0x32BC). Updates 2..32 dim the stage (0x1F7F10 /
//! 0x1F7EF0 by (n - 2) x 2 / x 4), otherwise the light ramp (+0xA0).
//! Returns 6 on updates 88..140, 2 on 85, 1 on 78, 8 on 187 (no drawing
//! then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `panels`
//! 0x1E003A0, `flash_b` 0x1E008B0, `spikes` 0x1E01320, `fade` 0x1E01E00,
//! `arches_b` 0x1E022D0, `drops` 0x1E02B30, `smears` 0x1E03490, `bars_b`
//! 0x1E03F10 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{aim_angles, first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const FADE: u32 = D + 0xA4;
const PANELS: u32 = D + 0xAC;
const BARS: u32 = D + 0xEBC;
const FLASH: u32 = D + 0x113C;
const SPIKES: u32 = D + 0x1A24;
const ARCHES: u32 = D + 0x2654;
const SMEARS: u32 = D + 0x302C;
const POOL: u32 = D + 0x30BC;
const DROPS: u32 = D + 0x32BC;
const BYTES: u32 = D + 0x826C;
/// The stage's other light byte (0x1F7EF0).
const LIGHT2: u32 = 0x002D_F5B0;

/// The entry 0x1E04810 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0x8370).
    fx.bzero(D, 0x8370);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0xC, 0), (0x14, 0xA, 0), (0x18, 0x16, 0), (0x1C, 0x14, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xB5);
    // The entry's s0 = 3 (the texture loop's), s1 / s2 the runner's 0.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4948, [Some(3), Some(0), Some(0)]);
    0
}

/// 0x101940 `aim_angles(v, out)` leaving its second atan2f's 0x1A5020
/// frame: ra 0x1019B4 at the caller's sp - 0x38 (as b173). When either
/// atan2f's x (v.y, then v.z) is exactly 1.0, 0x1A5020 tail-jumps to atanf
/// 0x1A6EB8, whose frame then sits at the caller's sp - 0x50 and keeps
/// 0x101940's s1 = `v` (`sd`, 64 bits) at - 0x48 [run, store-watched: the
/// next arches' first Q word reads its upper half].
fn aim_saving(fx: &mut Fx, v: u32, out: u32) {
    let tail = fx.mem.read_u32(v + 4) == 0x3F80_0000 || fx.mem.read_u32(v + 8) == 0x3F80_0000;
    aim_angles(fx, v, out);
    let sp = fx.sp;
    fx.mem.write_u64(sp - 0x38, 0x0010_19B4);
    if tail {
        fx.mem.write_u64(sp - 0x48, u64::from(v));
    }
}

/// rand from return address `ra` with the entry's s0 (64 bits) saved in
/// its frame.
fn rand64(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand()
}

/// (200 ((rand >> 4) % m) - half) / 100.
fn spread(fx: &mut Fx, s0: u64, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand64(fx, s0, ra) >> 4;
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(200 * r - half), 100.0)
}

/// ((rand >> 4) % m - half) / 100.
fn jit(fx: &mut Fx, s0: u64, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand64(fx, s0, ra) >> 4;
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

fn sx(v: i32) -> u64 {
    i64::from(v) as u64
}

/// Update 2 [F 0x1E0499C]: two smears, four crossbars and a flash at
/// each of nodes +0x10 / +0x18, a long shake at node +0x10.
fn start(fx: &mut Fx) {
    for s0 in 0..2u32 {
        let r = SMEARS + 0x48 * s0;
        let s2 = D + 8 * s0;
        set_counter4(fx, r, 0x40, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let (a, b) = if s0 == 0 { (0x10u32, 0x14u32) } else { (0x18, 0x1C) };
        let v = fx.mem.read_u8(D + a);
        fx.mem.write_u8(r + 0x21, v);
        let v = fx.mem.read_u8(D + b);
        fx.mem.write_u8(r + 0x22, v);
        fx.mem.write_u32(r + 8, POOL);
        fx.mem.write_u8(r + 0x1E, 0x20);
        fx.mem.write_u8(r + 0x1F, 8);
        fx.mem.write_u8(r + 0x20, 0);
        fx.mem.write_u32(r + 0x2C, 0x4270_0000);
        fx.mem.write_u32(r + 0x28, 0x3F00_0000);
        for o in [0x34u32, 0x35, 0x36] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x37, 0x60);
        let len = fx.mem.u8(r + 0x1F) as u32;
        for k in 0..=len {
            fx.mem.set16(r + 0xC + 2 * k, -1);
        }
        fx.mem.write_u32(r + 0x30, 0xBF80_0000);
        for s5 in 0..4i32 {
            // The first free crossbar, its -1 not checked.
            let i = first_free(fx, BARS, 0x50, 8);
            let b = BARS.wrapping_add((0x50 * i) as u32);
            set_counter4(fx, b, 0x1C, 0, 0xFF);
            fx.mem.write_u8(b + 5, 1);
            fx.mem.write_u8(b + 4, 0x80);
            let v = fx.mem.read_u8(s2 + 0x10);
            fx.mem.write_u8(b + 6, v);
            if s5 == 0 {
                fx.mem.write_u32(b + 0x10, 0x4448_0000);
                fx.mem.write_u32(b + 0x18, 0);
                fx.mem.write_u32(b + 0x14, 0x41A0_0000);
            } else {
                fx.mem.write_u32(b + 0x10, 0x43AF_0000);
                fx.mem.setf(b + 0x18, ee::mul(45.0, ee::from_i32(s5)));
                fx.mem.write_u32(b + 0x14, 0x4160_0000);
            }
            for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0x10), (9, 0x80), (8, 0x80)] {
                fx.mem.write_u8(b + o, v);
            }
            fx.mem.write_u32(b + 0x1C, 0x40C0_0000);
        }
        let f = FLASH + 0x4C * s0;
        set_counter4(fx, f, 0x1C, 0, 0xFF);
        fx.mem.write_u8(f + 5, 1);
        let v = fx.mem.read_u8(s2 + 0x10);
        fx.mem.write_u8(f + 6, v);
        fx.mem.write_u8(f + 4, 0x81);
        fx.mem.write_u8(f + 7, 9);
        fx.mem.write_u32(f + 0x10, 0x4248_0000);
        fx.mem.write_u32(f + 0x14, 0x4080_0000);
        for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0), (9, 0), (8, 0x80)] {
            fx.mem.write_u8(f + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(f + o, 0);
        }
    }
    set_counter4(fx, D + 0x40, 0x3E, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 1);
    let v = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0x46, v);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x3F80_0000);
    fx.mem.write_u32(D + 0x58, 0x3FCC_CCCD);
    fx.mem.write_u32(D + 0x5C, 0x3FCC_CCCD);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
}

/// Updates 85..128 [F 0x1E04CF4]: two arches from node 1 flying to the
/// target (sp + 0x90) jittered.
fn arches(fx: &mut Fx, sp: u32, s: &mut [u64; 2]) {
    for _ in 0..2 {
        let i = first_free(fx, ARCHES, 0x54, 0x1E);
        s[1] = sx(i);
        if i < 0 {
            continue;
        }
        let (h, n) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
        node_position_saving(fx, h, n, sp + 0xA0, s[0], Some(s[1]), 0x01E0_4D24);
        copy_v4(fx, sp + 0xA0, sp + 0xB0);
        s[0] = u64::from(sp + 0xB0);
        let f = ee::add(fx.mem.f(sp + 0xB0), spread(fx, s[0], 0x01E0_4D50, 0x28, 0xFA0));
        fx.mem.setf(sp + 0xB0, f);
        let f = ee::add(fx.mem.f(sp + 0xB4), spread(fx, s[0], 0x01E0_4DA8, 0x28, 0xFA0));
        fx.mem.setf(sp + 0xB4, f);
        let f = if fx.mem.read_u32(D + 0x30) == 0x8000 {
            let j = ee::add(-25.0, spread(fx, s[0], 0x01E0_4E0C, 0x28, 0xFA0));
            ee::add(fx.mem.f(sp + 0xB8), j)
        } else {
            let j = ee::add(25.0, spread(fx, s[0], 0x01E0_4E70, 0x28, 0xFA0));
            ee::add(fx.mem.f(sp + 0xB8), j)
        };
        fx.mem.setf(sp + 0xB8, f);
        let r = ARCHES + 0x54 * i as u32;
        s[1] = u64::from(D + 0x54 * i as u32);
        set_counter4(fx, r, 0x10, 0, 0xFF);
        for (o, v) in [(4u32, 0u8), (5, 0), (7, 6), (0xE, 0xC0), (0xD, 0x20), (0xC, 0x80), (0xF, 0x20), (0xB, 0xC0), (8, 0xC0), (9, 0xC0), (0xA, 0xC0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x4110_0000);
        fx.mem.write_u32(r + 0x14, 0x3FE6_6666);
        fx.mem.write_u32(r + 0x18, 0);
        copy_v4(fx, sp + 0xA0, r + 0x24);
        for c in 0..3u32 {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0xB0 + 4 * c), fx.mem.f(r + 0x24 + 4 * c)), 16.0);
            fx.mem.setf(r + 0x34 + 4 * c, v);
        }
        for c in 0..3u32 {
            let v = ee::sub(fx.mem.f(sp + 0xB0 + 4 * c), fx.mem.f(r + 0x24 + 4 * c));
            fx.mem.setf(sp + 0xC0 + 4 * c, v);
        }
        aim_saving(fx, sp + 0xC0, r + 0x44);
    }
}

/// Every fourth update 88..124 [F 0x1E05028]: at the target jittered
/// three panels, a flash, a spike and 30 bouncing sprites.
fn strike(fx: &mut Fx, sp: u32, s: &mut [u64; 2]) {
    copy_v4(fx, sp + 0x90, sp + 0xA0);
    let f = ee::add(fx.mem.f(sp + 0xA0), spread(fx, s[0], 0x01E0_503C, 0xF, 0x5DC));
    fx.mem.setf(sp + 0xA0, f);
    s[0] = u64::from(sp + 0xA4);
    let f = ee::add(fx.mem.f(sp + 0xA4), spread(fx, s[0], 0x01E0_5094, 0xF, 0x5DC));
    fx.mem.setf(sp + 0xA4, f);
    s[1] = u64::from(sp + 0xA8);
    let f = ee::add(fx.mem.f(sp + 0xA8), spread(fx, s[0], 0x01E0_50EC, 0xA, 0x3E8));
    fx.mem.setf(sp + 0xA8, f);
    let s2 = (rand64(fx, s[0], 0x01E0_5144) >> 4) & 0x3FFF;
    for s4 in 0..3i32 {
        let i = first_free(fx, PANELS, 0x3C, 0x3C);
        if i < 0 {
            continue;
        }
        let q = PANELS + 0x3C * i as u32;
        set_counter4(fx, q, 0x14, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 2), (0xA, 0), (9, 6), (0xB, 2), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        copy_v4(fx, sp + 0xA0, q + 0x2C);
        let dy = ee::add(-1.0, ee::from_i32(s4));
        let y = ee::add(fx.mem.f(q + 0x30), dy);
        fx.mem.setf(q + 0x30, y);
        fx.mem.write_u32(q + 0x10, 0x3F80_0000);
        fx.mem.write_u32(q + 0x14, 0x3F33_3333);
        fx.mem.write_u32(q + 0x18, if s4 == 1 { 0x4040_0000 } else { 0x4000_0000 });
        fx.mem.set32(q + 0x24, s2 + 0x2000);
    }
    let i = first_free(fx, FLASH, 0x4C, 0x1E);
    if i >= 0 {
        let a = FLASH + 0x4C * i as u32;
        set_counter4(fx, a, 0x18, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0x80), (7, 9)] {
            fx.mem.write_u8(a + o, v);
        }
        fx.mem.write_u32(a + 0x10, 0x4348_0000);
        fx.mem.write_u32(a + 0x14, 0x41A0_0000);
        for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x20), (8, 0x60), (0xF, 0xC0)] {
            fx.mem.write_u8(a + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(a + o, 0);
        }
        copy_v4(fx, sp + 0xA0, a + 0x1C);
        fx.mem.write_u32(a + 0x18, 0x4170_0000);
    }
    let i = first_free(fx, SPIKES, 0x68, 0x1E);
    if i >= 0 {
        let q = SPIKES + 0x68 * i as u32;
        set_counter4(fx, q, 0x18, 0, 0xFF);
        fx.mem.write_u8(q + 4, 5);
        fx.mem.write_u32(q + 0x14, 0x4000_0000);
        fx.mem.write_u32(q + 0x1C, 0x41F0_0000);
        for (o, v) in [(5u32, 0u8), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0x32)] {
            fx.mem.write_u8(q + o, v);
        }
        let x = rand64(fx, s[0], 0x01E0_53A4);
        fx.mem.set8(q + 9, x >> 4);
        fx.mem.write_u32(q + 0x10, BYTES);
        fx.mem.write_u32(q + 0x18, 0x3F80_0000);
        for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        copy_v4(fx, sp + 0xA0, q + 0x28);
        fx.mem.write_u32(q + 0x58, 0x40);
        fx.mem.write_u32(q + 0x5C, 0x80);
        fx.mem.write_u32(q + 0x60, 0x100);
    }
    for _ in 0..0x1E {
        let i = first_free(fx, DROPS, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let delay = (rand64(fx, s[0], 0x01E0_5448) >> 4) & 7;
        let q = DROPS + 0x44 * i as u32;
        set_counter4(fx, q, 0x20, delay, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 1), (0xB, 2), (0xA, 0), (9, 3), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        let f = ee::add(90.0, spread(fx, s[0], 0x01E0_54A8, 0x1E, 0xBB8));
        fx.mem.setf(q + 0x10, f);
        fx.mem.write_u32(q + 0x14, 0xBECC_CCCD);
        fx.mem.write_u32(q + 0x18, 0xBD75_C28F);
        fx.mem.write_u32(q + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
            fx.mem.write_u32(q + 0x20 + 4 * c, w);
        }
        let f = jit(fx, s[0], 0x01E0_5538, 0xD2, 0x69);
        fx.mem.setf(q + 0x2C, f);
        let f = jit(fx, s[0], 0x01E0_5574, 0xD2, 0x69);
        fx.mem.setf(q + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), f));
        let f = jit(fx, s[0], 0x01E0_55BC, 0xD2, 0x69);
        fx.mem.setf(q + 0x34, f);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    // The entry's s0 / s1 as the calls save them.
    let mut s = [sx(param), 0u64];
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x90, s[0], 0x01E0_4968);
    match fx.mem.u16(D) {
        0xBB => return 8,
        0x58 => {
            set_counter4(fx, D + 0x40, 0x38, 0, 0xFF);
            fx.mem.write_u8(D + 0x45, 0);
            fx.mem.write_u8(D + 0x44, 8);
            fx.mem.write_u32(D + 0x54, 0x4000_0000);
            fx.mem.write_u32(D + 0x58, 0x3F80_0000);
            fx.mem.write_u32(D + 0x5C, 0x3FE6_6666);
            fx.mem.write_u32(D + 0x70, 0);
            fx.mem.write_u32(D + 0x74, 0x4320_0000);
            fx.mem.write_u32(D + 0x90, 0x40C0_0000);
            copy_v4(fx, sp + 0x90, D + 0x60);
        }
        2 => {
            start(fx);
            s = [2, u64::from(D + 0x90)];
        }
        // The `beq n, 2` delay slot clears s0 [F 0x1E0498C].
        _ => s[0] = 0,
    }
    let n = fx.mem.u16(D);
    if (0x55..0x81).contains(&n) {
        arches(fx, sp, &mut s);
    }
    let n = fx.mem.u16(D);
    if (0x58..0x7D).contains(&n) && n & 3 == 0 {
        strike(fx, sp, &mut s);
    }
    let n = fx.mem.u16(D);
    if (2..0x21).contains(&n) {
        fx.set_stage_light(((n - 2) << 1) & 0xFF);
        let n = fx.mem.u16(D);
        fx.mem.write_u8(LIGHT2, ((n - 2) << 2) as u8);
    } else {
        light_ramp(fx, D + 0xA0, n, 0x01E0_5654, [Some(s[0]), Some(s[1])]);
    }
    shake_saving(fx, D + 0x40, s[0], 0x01E0_5660, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the entry's s6 (D) at +0x70 and ra at +0x80.
    fx.saved(sp - 0xF0, &[(0x70, u64::from(D)), (0x80, 0x01E0_566C)]);
    blib::spike::spikes(fx, 0x01E0_1320, SPIKES, OT, 0x1E);
    blib::star::flash_b(fx, 0x01E0_08B0, FLASH, OT, 0x1E);
    blib::cross::bars_b(fx, 0x01E0_3F10, BARS, OT, 8);
    blib::arch::arches_b(fx, 0x01E0_22D0, ARCHES, OT, 0x1E);
    blib::drop::drops(fx, DROPS, OT, 0xC8);
    blib::smear::smears(fx, 0x01E0_3490, SMEARS, OT, 2);
    blib::panel::panels(fx, 0x01E0_03A0, PANELS, OT, 0x3C);
    let n = fx.mem.u16(D);
    if (0x58..0x8D).contains(&n) {
        6
    } else if n == 0x55 {
        2
    } else if n == 0x4E {
        1
    } else {
        0
    }
}
