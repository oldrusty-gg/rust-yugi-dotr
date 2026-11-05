//! BMODULE b636 (card 666), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01120, data area 0x1ED0000) [F]: b538's older task pool: 200 tasks of 0x40 bytes at D+0x478
//! (alive +0x36, step +0x24) drawn as rings of camera-facing quads built in
//! the entry's descriptor sp + 0x90 (e18ba1c23957 at 0x1E004D0,
//! `quads4::ring_into`, UVs by uv_rect4 0x1E00080). Up to update 105 one
//! dark-blue ring a update at the attacker's node 0x11 (0x1E00A70, six
//! quads, following the node); from 107 to 304 one tall blue ring
//! (0x1E00C50, 18 quads) and every fourth update a red one (0x1E00E60, 12
//! quads), both sliding 5 / 6 a update along the facing. Returns 1 on
//! update 105, 4 on 325, 2 on 315, 6 on 116..309, 8 once the 355-update
//! timer +0x37A0 has run out.
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
/// The step functions a task holds (+0x24), as the record's addresses.
const DARK: u32 = 0x01E0_0A70;
const BLUE: u32 = 0x01E0_0C50;
const RED: u32 = 0x01E0_0E60;
/// The entry's `jal` return from the emitter (0x1E01374 + 8).
const EMIT_RA: u64 = 0x01E0_137C;
const TIMER: u32 = D + 0x37A0;
const COUNTER: u32 = D + 0x37FE;
const BYTES: u32 = D + 0x3802;
const BYTES2: u32 = D + 0x3902;
const SINES: u32 = D + 0x3A04;

/// The entry 0x1E01120.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x120);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x37AC, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0x120);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3B70);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x37FC, param);
    fx.mem.set32(TIMER, 0x163);
    for k in 0..3 {
        fx.mem.write_u32(D + 0x310 + 4 * k, 0);
    }
    for (k, node) in [0x11, 5, 6, 0xD, 0xE, 0xF, 0x17, 0x1A].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(D + 0x37E8 + 2 * k as u32, r);
    }
    textures(fx);
    for k in 0..0x100u32 {
        let v = rand_mod(fx, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rand_mod(fx, 0x1E);
        fx.mem.set8(BYTES2 + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand();
        fx.mem.set8(BYTES2 + k, v % 4 - 2);
    }
    for k in 0..0x64u32 {
        let v = rand_mod(fx, 0xC);
        fx.mem.set8(BYTES2 + k, v - 6);
    }
    for k in 0xB4..0x100u32 {
        let v = fx.rand();
        fx.mem.set8(BYTES2 + k, v % 8 - 4);
    }
    0
}

/// 0x1E01070(0, D): the effect textures 0..2 at D+0x340, 13..15 at
/// D+0x374, the attacker's 0..2 at D+0x3780.
fn textures(fx: &mut Fx) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_1258)]);
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x340 + 4 * k as u32, t);
    }
    for k in 0xD..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x374 + 4 * (k as u32 - 0xD), t);
    }
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0x3780 + 4 * k as u32, t);
    }
    fx.leave(0x40);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = D + 0x40 * k;
        if fx.mem.i16(t + 0x4AE) == 0 {
            continue;
        }
        for (f, ra) in [(DARK, 0x01E0_13B0u64), (BLUE, 0x01E0_13D0), (RED, 0x01E0_13F0)] {
            if fx.mem.read_u32(t + 0x49C) == f {
                ring(fx, t + 0x478, f, ra);
            }
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    // 0x1F71A0's frame below the entry's (its saved ra 0x1E01414 and s0
    // = D): the stale half of the first rect copy (D+0x9C..+0xA3).
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_1414), (0, u64::from(D))]);
    for k in 0..TASKS {
        let t = D + 0x40 * k;
        if fx.mem.i16(t + 0x4AE) == 0 {
            continue;
        }
        for (f, tex, local, count, table) in [(DARK, 0x3780u32, 0x60u32, 6, BYTES2), (BLUE, 0x3784, 0x70, 0x12, BYTES), (RED, 0x3788, 0x80, 0xC, BYTES)] {
            if fx.mem.read_u32(t + 0x49C) != f {
                continue;
            }
            let tex = fx.mem.read_u32(D + tex);
            fx.mem.write_u32(D + 0x88, tex);
            let sub = fx.mem.i16(t + 0x4AC);
            l2::rect(fx, sp + local, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, count);
            quads4::ring_into(fx, uv::uv_rect4, D, t + 0x478, sp + 0x90, table);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x69 => 1,
        0x145 => 4,
        0x13B => 2,
        0x74..=0x135 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E00850(D, 0): the spawn colour D+0x37A4 (0x80, 0, 0, 0), the update
/// counter and the emitters (frame 0x30: ra, the entry's s0 = D).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x10, EMIT_RA), (0, u64::from(D))]);
    fx.mem.write_u8(D + 0x37A4, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(D + 0x37A4 + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + at);
        fx.node_position(h, slot, out);
    };
    if fx.mem.i16(COUNTER) == 0x33 {
        node(fx, 0x37EA, D + 0x36C0);
    }
    if (0x34..0x41).contains(&fx.mem.i16(COUNTER)) {
        node(fx, 0x37EA, sp + 0x20);
        for k in 0..3 {
            let w = fx.mem.read_u32(D + 0x36C0 + 4 * k);
            fx.mem.write_u32(D + 0x2D0 + 4 * k, w);
        }
    }
    let place = |fx: &mut Fx, up: f32| {
        node(fx, 0x37E8, sp + 0x20);
        let x = fx.mem.read_u32(sp + 0x20);
        fx.mem.write_u32(D + 0x2D0, x);
        let y = ee::add(up, fx.mem.f(sp + 0x24));
        fx.mem.setf(D + 0x2D4, y);
        let z = fx.mem.read_u32(sp + 0x28);
        fx.mem.write_u32(D + 0x2D8, z);
    };
    if fx.mem.i16(COUNTER) < 0x6A {
        place(fx, 1.0);
        spawn(fx, DARK, 0x01E0_0968);
    }
    if (0x6B..0x131).contains(&fx.mem.i16(COUNTER)) {
        place(fx, 4.0);
        spawn(fx, BLUE, 0x01E0_09D4);
    }
    let n = fx.mem.i16(COUNTER);
    if n % 4 == 1 && (0x6B..0x131).contains(&n) {
        place(fx, 4.0);
        spawn(fx, RED, 0x01E0_0A60);
    }
    fx.leave(0x30);
}

/// 0x1E01650(D, step, arg = 0): the first free task cleared and set off at
/// D+0x2D0 (as i16) with the colour D+0x37A4 (twice), +0x30 = D+0x37FA; 1
/// when one was free (its saves: ra, the entry's s2 = -1 / s1 = 0, the
/// emitter's s0 = D).
fn spawn(fx: &mut Fx, f: u32, ra: u64) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, ra), (0x20, u64::MAX), (0x10, 0), (0, u64::from(D))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x40 * k + 0x4AE) == 0) {
        Some(k) => {
            let t = D + 0x478 + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(D + 0x37A4), fx.mem.read_u8(D + 0x37A5), fx.mem.read_u8(D + 0x37A6));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(D + 0x37A7);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x2D0, 0);
            let a = fx.mem.i16(D + 0x37FA);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand % m (HI, the remainder).
fn rand_mod(fx: &mut Fx, m: i32) -> i32 {
    let v = fx.rand();
    fx.div(v, m).1
}

/// 0x1E00A70 / 0x1E00C50 / 0x1E00E60 (D, task): a ring. First step: size
/// +0x38 (dark 25 before update 84 else 5; blue 100; red 110), a random
/// cell frame +0x34 (-7..7), +0x14 random, dark-blue / dark-red colour, a
/// random angle +0x3A, radius growth +0xE 10..34, table row +8 0..9, life
/// +6 (dark 2..8, blue 4..18, red 5..19). Fades in by 0x30 for its life,
/// then out by 10; radius += growth. The dark ring follows node 0x11; the
/// others slide 5 / 6 a update along the facing (z += -k cos(facing -
/// 180)) and grow by 5.
fn ring(fx: &mut Fx, t: u32, f: u32, ra: u64) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        let size = match f {
            DARK if fx.mem.i16(COUNTER) < 0x54 => 0x19,
            DARK => 5,
            BLUE => 0x64,
            _ => 0x6E,
        };
        fx.mem.set16(t + 0x38, size);
        let v = fx.rand();
        fx.mem.set16(t + 0x34, v % 8);
        let v = rand_mod(fx, 0xFFFF);
        fx.mem.set32(t + 0x14, v);
        if f == RED {
            l2::rgb(fx, t + 0x28, 0x80, 8, 0x30);
        } else {
            l2::rgb(fx, t + 0x28, 0x30, 8, 0x80);
        }
        fx.mem.set16(t + 0x30, 0);
        let v = rand_mod(fx, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rand_mod(fx, 0x19);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = rand_mod(fx, 0xA);
        fx.mem.set16(t + 8, v);
        let life = match f {
            DARK => rand_mod(fx, 7) + 2,
            BLUE => fx.rand() % 8 + 0xB,
            _ => fx.rand() % 8 + 0xC,
        };
        fx.mem.set16(t + 6, life);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x61 {
            fx.mem.write_u8(t + 0x2B, a + 0x30);
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xA);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    if f == DARK {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0x37E8);
        fx.node_position(h, slot, sp + 0x30);
        for k in 0..3 {
            let v = ee::fixsfsi(fx.mem.f(sp + 0x30 + 4 * k));
            fx.mem.set16(t + 2 * k, v);
        }
    } else {
        let h = fx.self_handle();
        fx.unit_rotation(h, sp + 0x30);
        let yaw = ee::fixsfsi(ee::div(ee::from_i32(fx.mem.i32(sp + 0x34)), 180.0)) as i16 as i32;
        let c = l2::sin_deg(fx, SINES, yaw - 0xB4, 1);
        let k = if f == BLUE { 5.0 } else { 6.0 };
        let dz = ee::fixsfsi(ee::mul(k, ee::neg(c))) as i16 as i32;
        let z = fx.mem.i16(t + 4) + dz;
        fx.mem.set16(t + 4, z);
        let s = fx.mem.i16(t + 0x38) + 5;
        fx.mem.set16(t + 0x38, s);
    }
    fx.leave(0x40);
}
