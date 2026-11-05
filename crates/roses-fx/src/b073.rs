//! BMODULE b073 (card 626), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02D30) [F]. The entry
//! takes (D, param). A task pool of 150 tasks of 0x40 bytes at D+0x2A0
//! (alive +0x36, step +0x24 as a record address), spawned by 0x1E03840,
//! drawn with the second library: rings of quads with a mode (383a1b86ead2
//! at 0x1E009B0, `quads4::ring_at_m`), 18-strip discs (0cb01f69a41a at
//! 0x1E00F10 `quads4::fan18`, 095f4ce60e44 at 0x1E03970 `fan18_far`), far-Z
//! quarter quads (09eeeb9282c1 at 0x1E03BD0, `quads4::quarters_far_at`) and
//! the record's own ribbons between two node histories (0x1E03E70 /
//! 0x1E04890: Lagrange-smoothed gouraud strips, four steps a segment) and
//! quarter quads at the task's position keyed by depth (0x1E05290). The
//! node histories D+0x28F0.. are kept by 0x1E00080 (8a49c57fb3bb,
//! `spawn::history`). Update 111 / 151: glow discs and blue quarters at the
//! attacker's nodes 0x43 / 0x4A; 196..255 sparks at nodes 9 / 0x3C; 255 the
//! ribbons (frozen copies from 263); 260 the burst at the target (disc,
//! rings, 35 + 10 sparks). Returns 1 on update 250, 4 on 300, 2 on 270, 6
//! on 261..264, 8 once the 310-update timer D+0x3590 has run out.
//!
//! The step 0x1E01740 is drawn through 0x1E03BC0, an empty function [F]:
//! its tasks draw nothing (the atlas rectangle D+0x94 is still written).
use crate::blib::{quads4, spawn, spline, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0x96;
const POOL: u32 = D + 0x2A0;
const POS: u32 = D + 0x250;
const TIMER: u32 = D + 0x3590;
const COLOUR: u32 = D + 0x3594;
const OTW: u32 = D + 0x359C;
const NODES: u32 = D + 0x35B8;
const ANGLE: u32 = D + 0x35CA;
const INDEX: u32 = D + 0x35CE;
const COUNTER: u32 = D + 0x35D4;
const BYTES: u32 = D + 0x35D6;
const BYTES_B: u32 = D + 0x36D6;

/// The node slot k (D+0x35B8 + 2k: nodes 0x43, 0x44, 0x4A, 0x4B, 0xF, 9,
/// 0x3C of the attacker).
fn node(fx: &mut Fx, k: u32) -> i32 {
    fx.mem.i16(NODES + 2 * k)
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// rand % m (HI) from return address `ra` (rand's frame: `s0`).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02D30 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1A0);
    fx.mem.write_u32(OTW, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x1A0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3950);
    l2::sine_table(fx, D + 0x37D8);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x35D2, param);
    fx.mem.set32(TIMER, 0x136);
    for (k, (n, clear)) in [(0x43, 1), (0x44, 0), (0x4A, 0), (0x4B, 0), (0xF, 0), (9, 0), (0x3C, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0xBu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x28C + 4 * (k - 0xB), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x268, t);
    for k in 0..4u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x3570 + 4 * k, t);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2EA4, 0x14);
        fx.mem.set8(BYTES + k, v - 0xA);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_2ED8);
        fx.mem.set8(BYTES + k, v % 8);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2F10, 0x1E);
        fx.mem.set8(BYTES_B + k, v - 0xF);
    }
    0
}

/// The steps (their addresses, also the tasks' +0x24), the entry's return
/// address after each `jal` and the step's frame size.
const STEPS: [(u32, u32, u32); 12] = [
    (0x01E0_1740, 0x01E0_2F80, 0x70),
    (0x01E0_1970, 0x01E0_2FA0, 0x20),
    (0x01E0_1B50, 0x01E0_2FC0, 0x40),
    (0x01E0_1EE0, 0x01E0_2FE0, 0x30),
    (0x01E0_2090, 0x01E0_3000, 0x20),
    (0x01E0_2180, 0x01E0_3020, 0x20),
    (0x01E0_22E0, 0x01E0_3040, 0x20),
    (0x01E0_2450, 0x01E0_3060, 0x20),
    (0x01E0_25C0, 0x01E0_3080, 0x50),
    (0x01E0_2860, 0x01E0_30A0, 0x50),
    (0x01E0_2A70, 0x01E0_30C0, 0x20),
    (0x01E0_2BD0, 0x01E0_30E0, 0x20),
];

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        for &(a, ra, size) in STEPS.iter() {
            if fx.mem.read_u32(t + 0x24) == a {
                let s = fx.enter(size);
                // The step's prologue: ra, then the entry's s3 = k, s2 =
                // D + 0x40 k (0x70 frame only), s1 = 0, s0 = D.
                match size {
                    0x20 => fx.saved(s, &[(0x10, u64::from(ra)), (0, u64::from(D))]),
                    0x70 => fx.saved(s, &[(0x40, u64::from(ra)), (0x30, u64::from(k)), (0x20, u64::from(D + 0x40 * k)), (0x10, 0), (0, u64::from(D))]),
                    _ => fx.saved(s, &[(0x20, u64::from(ra)), (0x10, 0), (0, u64::from(D))]),
                }
                run_step(fx, a, t, s);
                fx.leave(size);
            }
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_3104), (0, u64::from(D))]);
    let z = ee::sub(fx.mem.f(sp + 0x58), 25.0);
    fx.mem.setf(sp + 0x58, z);
    let mut count = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        count += 1;
        draw(fx, sp, t, k, count);
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0xFA {
        1
    } else if c == 0x12C {
        4
    } else if c == 0x10E {
        2
    } else if (0x105..0x109).contains(&c) {
        6
    } else if fx.mem.i32(TIMER) > 0 {
        0
    } else {
        8
    }
}

fn run_step(fx: &mut Fx, a: u32, t: u32, sp: u32) {
    match a {
        0x01E0_1740 => motes(fx, t, sp),
        0x01E0_1970 => sparks(fx, t),
        0x01E0_1B50 => ribbon_a(fx, t, sp),
        0x01E0_1EE0 => ribbon_b(fx, t),
        0x01E0_2090 => disc(fx, t),
        0x01E0_2180 => quarters(fx, t, 0x28, 0x1F4, 7, 0x80, 0x15, 0x13, 0x46, 0x46),
        0x01E0_22E0 => burst(fx, t, 0x01E0_22E0, 5, (0xC8, 0x32), None),
        0x01E0_2450 => burst(fx, t, 0x01E0_2450, 0xF, (0x32, 0xA), Some((0xA, 0xC))),
        0x01E0_25C0 => glow(fx, t, sp, false),
        0x01E0_2860 => glow(fx, t, sp, true),
        0x01E0_2A70 => quarters(fx, t, 7, 0x19, 0xB, 0x80, 0xF, 0xD, 0x14, 2),
        _ => quarters(fx, t, 7, 7, 7, 0x80, 0xF, 0xD, 3, 3),
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends
/// (`end`) or the byte is cleared.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8, end: bool) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else if end {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.write_u8(t + at, 0);
    }
}

/// The step offset (+8 / +0xA / +0xC) = (node b - node a) / 5 as i16.
fn fifth(fx: &mut Fx, t: u32, sp: u32, a: u32, b: u32, ra: u32) {
    let h = fx.self_handle();
    let n = node(fx, a);
    fx.node_position(h, n, sp + 0x50);
    let h = fx.self_handle();
    let n = node(fx, b);
    fx.node_position(h, n, sp + 0x60);
    for k in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x60 + 4 * k), fx.mem.f(sp + 0x50 + 4 * k)), 5.0);
        fx.mem.setf(sp + 0x50 + 4 * k, v);
    }
    for k in 0..3u32 {
        let v = fx.mem.f(sp + 0x50 + 4 * k);
        fx.fix_residue(v, ra + 0xC * k);
        fx.mem.set16(t + 8 + 2 * k, ee::fixsfsi(v));
    }
}

/// 0x1E01740 (D, task): a white mote (size 200, cell 1) stepping from node
/// 0x44 to 0x43 (+0x30 = 0) or 0x4B to 0x4A (1); alpha 0x80 for 7
/// updates. Never visible (drawn through the empty 0x1E03BC0).
fn motes(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xC8);
        fx.mem.set16(t + 0x34, 1);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        if fx.mem.i16(t + 0x30) == 0 {
            fifth(fx, t, sp, 1, 0, 0x01E0_1824);
        }
        if fx.mem.i16(t + 0x30) == 1 {
            fifth(fx, t, sp, 3, 2, 0x01E0_18DC);
        }
        let v = rmod(fx, t, 0x01E0_1900, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    }
    age(fx, t);
}

/// 0x1E01970 (D, task): white sparks: size 80 (+3 a update), cell 0..3
/// counting to 6 on odd updates, radius 0..29 growing by 13..25, row
/// 0..29, life 7..16; in by 0x40, out by 0xF.
fn sparks(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x50);
        let v = fx.rand_from(t, 0x01E0_1998);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, 0xFF, 0xFF, 0xFF);
        let v = rmod(fx, t, 0x01E0_19CC, 0x1E);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, t, 0x01E0_19E8, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, 0x01E0_1A04, 0xD);
        fx.mem.set16(t + 0xE, v + 0xD);
        let v = rmod(fx, t, 0x01E0_1A24, 0x1E);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1A40, 0xA);
        fx.mem.set16(t + 6, v + 7);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x2B, 0x81, 0x40);
    } else {
        fade_out(fx, t, 0x2B, 0x11, 0xF, true);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 6 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, s + 3);
    }
    age(fx, t);
}

/// The four node histories (slot, first entry) of 10 points each.
const HISTORIES: [(u32, u32); 4] = [(0, D + 0x28F0), (1, D + 0x2990), (2, D + 0x2A30), (3, D + 0x2AD0)];

/// 0x1E01B50 (D, task): the ribbons' head: on the first update every
/// history filled with its node's point; +6 (the points drawn) up to 6;
/// alpha 0xC0 for 10 updates, then out by 0xF; spawns 0x1E01EE0 on update
/// 8; each update the four histories take their node's point.
fn ribbon_a(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x80);
        for &(slot, first) in HISTORIES.iter() {
            let h = fx.self_handle();
            let n = node(fx, slot);
            fx.node_position(h, n, sp + 0x30);
            for k in 0..9u32 {
                copy3(fx, first + 0x10 * k, sp + 0x30);
            }
        }
        fx.mem.set16(t + 6, 0);
        fx.mem.write_u8(t + 0x2B, 0);
        let v = rmod(fx, t, 0x01E0_1D18, 0x11);
        fx.mem.set16(t + 0xA, v + 0x2C);
        let v = fx.rand_from(t, 0x01E0_1D38);
        fx.mem.set16(t + 8, v % 8 + 5);
        fx.mem.set16(t + 0x34, 0);
        let v = rmod(fx, t, 0x01E0_1D5C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x38, 0x1E);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fx.mem.write_u8(t + 0x2B, 0xC0);
    } else {
        fade_out(fx, t, 0x2B, 0x11, 0xF, true);
    }
    if fx.mem.i16(t + 0x32) == 8 {
        spawn_task(fx, 0x01E0_1EE0, 0);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let n = fx.mem.i16(t + 6);
        if n < 6 {
            fx.mem.set16(t + 6, n + 1);
        }
    }
    for &(slot, first) in HISTORIES.iter() {
        let h = fx.self_handle();
        let n = node(fx, slot);
        fx.node_position(h, n, sp + 0x30);
        spawn::history(fx, first + 0x90, 0xA, sp + 0x30);
    }
    age(fx, t);
}

/// 0x1E01EE0 (D, task): the ribbons' frozen copy: the four histories
/// copied (D+0x2F30.., 10 points each), six points drawn; in by 0x40 under
/// 0xC0 for 7 updates, then out by 9.
fn ribbon_b(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x40);
        for k in 0..0xAu32 {
            for (to, from) in [(0x2F30u32, 0x28F0u32), (0x2FD0, 0x2990), (0x3070, 0x2A30), (0x3110, 0x2AD0)] {
                copy3(fx, D + to + 0x10 * k, D + from + 0x10 * k);
            }
        }
        fx.mem.set16(t + 6, 6);
        fx.mem.write_u8(t + 0x2B, 0);
        let v = rmod(fx, t, 0x01E0_1FB4, 0x11);
        fx.mem.set16(t + 0xA, v + 0x2C);
        let v = fx.rand_from(t, 0x01E0_1FD4);
        fx.mem.set16(t + 8, v % 8 + 5);
        fx.mem.set16(t + 0x34, 0);
        let v = rmod(fx, t, 0x01E0_1FF8, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x38, 0x1E);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 7 {
        fade_in(fx, t, 0x2B, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x2B, 0xB, 9, true);
    }
    age(fx, t);
}

/// 0x1E02090 (D, task): the burst disc (radius 430; centre grey 0xC0, rim
/// blue); in by 0x50 for 5 updates, out by 0x13.
fn disc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x1AE);
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x2C, 0x80, 0x80, 0xFF);
        let v = rmod(fx, t, 0x01E0_20E4, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if fx.mem.i16(t + 0x32) < 5 {
        fade_in(fx, t, 0x2B, 0xF0, 0x50);
    } else {
        fade_out(fx, t, 0x2B, 0x15, 0x13, true);
    }
    age(fx, t);
}

/// 0x1E02180 / 0x1E02A70 / 0x1E02BD0 (D, task): blue quarter quads of half
/// extents (`w`, `h`); in by 0x40 (under `cap`) for `phase` updates, then
/// out by `down` (floor `floor`); the height grows by `grow0` (+0xE = 0:
/// angle 0) or `grow1`, the angle by 10 a update.
#[allow(clippy::too_many_arguments)]
fn quarters(fx: &mut Fx, t: u32, w: i32, h: i32, phase: i32, cap: u8, floor: u8, down: u8, grow0: i32, grow1: i32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        let blue = if w == 0x28 { 0xFF } else { 0x80 };
        l2::rgb(fx, t + 0x28, 0x30, 0x30, blue);
        l2::rgb(fx, t + 0x2C, 0x30, 0x30, blue);
        fx.mem.set16(t + 8, w);
        fx.mem.set16(t + 0xA, h);
        let e = if fx.mem.i16(t + 0x3A) == 0 { 0 } else { 1 };
        fx.mem.set16(t + 0xE, e);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if fx.mem.i16(t + 0x32) < phase {
        fade_in(fx, t, 0x2B, cap, 0x40);
        fade_in(fx, t, 0x2F, cap, 0x40);
    } else {
        fade_out(fx, t, 0x2B, floor, down, true);
        fade_out(fx, t, 0x2F, floor, down, false);
    }
    let g = if fx.mem.i16(t + 0xE) == 0 { grow0 } else { grow1 };
    let v = fx.mem.i16(t + 0xA) + g;
    fx.mem.set16(t + 0xA, v);
    age(fx, t);
    let a = fx.mem.i16(t + 0x3A) + 0xA;
    fx.mem.set16(t + 0x3A, a);
}

/// 0x1E022E0 / 0x1E02450 (D, task) at `s`: blue burst sparks: size
/// `size`, cell 0, the radius +0x30 kept in +0xA and set to rand %
/// radius.0 + radius.1, growth 5..12 (or rand % grow.0 + grow.1), row
/// 0..34, life 5..9; in by 0x40, out by 7.
fn burst(fx: &mut Fx, t: u32, s: u32, size: i32, radius: (i32, i32), grow: Option<(i32, i32)>) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, size);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0xFF);
        let r = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, r);
        let v = rmod(fx, t, s + 0x48, radius.0);
        fx.mem.set16(t + 0x30, v + radius.1);
        let v = rmod(fx, t, s + 0x68, 0x168);
        fx.mem.set16(t + 0x3A, v);
        match grow {
            None => {
                let v = fx.rand_from(t, s + 0x84);
                fx.mem.set16(t + 0xE, v % 8 + 5);
            }
            Some((m, add)) => {
                let v = rmod(fx, t, s + 0x84, m);
                fx.mem.set16(t + 0xE, v + add);
            }
        }
        let v = rmod(fx, t, s + 0xA4, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, s + 0xC0, 5);
        fx.mem.set16(t + 6, v + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x2B, 0x81, 0x40);
    } else {
        fade_out(fx, t, 0x2B, 9, 7, true);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    age(fx, t);
}

/// 0x1E025C0 / 0x1E02860 (D, task): a glow disc (radius 60, grey centre,
/// dark rim) at the point D+0x20 + 0x10 i16 +0x30: 0x25C0 moves it from
/// node 0x43 (0x4A for +0x30 = 1) towards 0x44 (0x4B) by +0xE / 20 of the
/// way, 0x2860 (`side`) sits at node 0xF pushed by +-1.3 in x; in by 0x40
/// for 11 updates, then out by 0xC.
fn glow(fx: &mut Fx, t: u32, sp: u32, side: bool) {
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x38, 0x3C);
        l2::rgb(fx, t + 0x2C, 0x10, 0x10, 8);
        fx.mem.set16(t + 0xE, 0);
        let ra = if side { 0x01E0_28BC } else { 0x01E0_261C };
        let v = rmod(fx, t, ra, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if fx.mem.i16(t + 0x32) < 0xB {
        fade_in(fx, t, 0x2B, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x2B, 0x11, 0xC, true);
    }
    let at = |fx: &mut Fx| (D as i32 + 0x20 + 0x10 * fx.mem.i16(t + 0x30)) as u32;
    if side {
        let h = fx.self_handle();
        let n = node(fx, 4);
        fx.node_position(h, n, sp + 0x30);
        let p = at(fx);
        copy3(fx, p, sp + 0x30);
        let x = if fx.mem.i16(t + 0x30) == 0 { 0x3FA6_6666 } else { 0xBFA6_6666 };
        fx.mem.write_u32(sp + 0x30, x);
        fx.mem.set32(sp + 0x34, 0);
        fx.mem.set32(sp + 0x38, 0);
        for k in 0..3u32 {
            let p = at(fx) + 4 * k;
            let v = ee::add(fx.mem.f(p), fx.mem.f(sp + 0x30 + 4 * k));
            fx.mem.setf(p, v);
        }
    } else {
        let (a, b) = if fx.mem.i16(t + 0x30) == 0 { (0, 1) } else { (2, 3) };
        let h = fx.self_handle();
        let n = node(fx, a);
        fx.node_position(h, n, sp + 0x30);
        let h = fx.self_handle();
        let n = node(fx, b);
        fx.node_position(h, n, sp + 0x40);
        let p = at(fx);
        copy3(fx, p, sp + 0x30);
        for k in 0..3u32 {
            let v = ee::sub(fx.mem.f(sp + 0x40 + 4 * k), fx.mem.f(sp + 0x30 + 4 * k));
            fx.mem.setf(sp + 0x30 + 4 * k, v);
        }
        for k in 0..3u32 {
            let v = ee::div(fx.mem.f(sp + 0x30 + 4 * k), 20.0);
            fx.mem.setf(sp + 0x30 + 4 * k, v);
        }
        for k in 0..3u32 {
            let e = ee::from_i32(fx.mem.i16(t + 0xE));
            let p = at(fx) + 4 * k;
            let v = ee::add(fx.mem.f(p), ee::mul(fx.mem.f(sp + 0x30 + 4 * k), e));
            fx.mem.setf(p, v);
        }
    }
    let e = fx.mem.i16(t + 0xE) + 1;
    fx.mem.set16(t + 0xE, e);
    age(fx, t);
}

/// 0x1E03840(D, step, a2): a new task (first free of 150) with the spawn
/// colour D+0x3594 (both colours), at D+0x250, index D+0x35CE, angle
/// D+0x35CA.
fn spawn_task(fx: &mut Fx, step: u32, a2: u32) {
    fx.enter(0x60);
    let mut task = None;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            task = Some(t);
            break;
        }
    }
    if let Some(t) = task {
        for at in [0x28u32, 0x2C] {
            let c: [i32; 3] = std::array::from_fn(|k| i32::from(fx.mem.read_u8(COLOUR + k as u32)));
            l2::rgb(fx, t + at, c[0], c[1], c[2]);
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + at + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        let i = fx.mem.i16(INDEX);
        fx.mem.set16(t + 0x30, i);
        let a = fx.mem.i16(ANGLE);
        fx.mem.set16(t + 0x3A, a);
        fx.mem.write_u32(t + 0x20, a2);
        fx.mem.write_u32(t + 0x24, step);
    }
    fx.leave(0x60);
}

/// The glow discs and their two quarter quads at node slot `slot`.
fn glows(fx: &mut Fx, sp: u32, slot: u32, index: i32, ra: u32) {
    let h = fx.self_handle();
    let n = node(fx, slot);
    fx.node_position(h, n, sp + 0x40);
    let v = rmod(fx, D, ra, 0x168);
    fx.mem.set16(ANGLE, v);
    fx.mem.set16(INDEX, index);
    spawn_task(fx, 0x01E0_25C0, 0);
    fx.mem.set16(ANGLE, 0);
    spawn_task(fx, 0x01E0_2A70, 0);
    fx.mem.set16(ANGLE, 0x5A);
    spawn_task(fx, 0x01E0_2A70, 0);
}

/// Two sparks at node slot `slot`, 2 below it.
fn spark_pair(fx: &mut Fx, sp: u32, slot: u32) {
    let h = fx.self_handle();
    let n = node(fx, slot);
    fx.node_position(h, n, sp + 0x40);
    let x = fx.mem.read_u32(sp + 0x40);
    fx.mem.write_u32(POS, x);
    let y = ee::sub(fx.mem.f(sp + 0x44), 2.0);
    fx.mem.setf(POS + 4, y);
    let z = fx.mem.read_u32(sp + 0x48);
    fx.mem.write_u32(POS + 8, z);
    for _ in 0..2 {
        spawn_task(fx, 0x01E0_1970, 0);
    }
}

/// D+0x250 = node slot `slot`'s point plus rand % 6 - 3 per axis.
fn jitter(fx: &mut Fx, sp: u32, slot: u32, ras: [(u32, u32); 3]) {
    let h = fx.self_handle();
    let n = node(fx, slot);
    fx.node_position(h, n, sp + 0x40);
    for (k, (s0, ra)) in ras.into_iter().enumerate() {
        let r = rmod(fx, s0, ra, 6);
        let k = k as u32;
        let v = ee::sub(ee::add(fx.mem.f(sp + 0x40 + 4 * k), ee::from_i32(r)), 3.0);
        fx.mem.setf(POS + 4 * k, v);
    }
}

/// 0x1E011A0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x60).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    // The entry's ra, s2 = param, s1 = 0, s0 = D.
    fx.saved(sp, &[(0x30, 0x01E0_2F4C), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    l2::rgb(fx, COLOUR, 0x80, 0, 0);
    fx.mem.write_u8(COLOUR + 3, 0);
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    let c = fx.mem.i16(COUNTER);
    if c == 0x6F {
        glows(fx, sp, 0, 0, 0x01E0_1208);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0x97 {
        glows(fx, sp, 2, 1, 0x01E0_1294);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0xFF {
        spawn_task(fx, 0x01E0_1B50, 0);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0x104 {
        let h = fx.self_handle();
        let n = node(fx, 2);
        fx.node_position(h, n, sp + 0x40);
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x50);
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_1354), (0, u64::from(D))]);
        let x = fx.mem.read_u32(sp + 0x50);
        fx.mem.write_u32(POS, x);
        let y = fx.mem.read_u32(sp + 0x44);
        fx.mem.write_u32(POS + 4, y);
        let z = fx.mem.read_u32(sp + 0x58);
        fx.mem.write_u32(POS + 8, z);
        spawn_task(fx, 0x01E0_2090, 0);
        fx.mem.set16(ANGLE, 0);
        spawn_task(fx, 0x01E0_2180, 0);
        fx.mem.set16(ANGLE, 0x5A);
        spawn_task(fx, 0x01E0_2180, 0);
        for _ in 0..0x23 {
            spawn_task(fx, 0x01E0_22E0, 0);
        }
        for _ in 0..0xA {
            spawn_task(fx, 0x01E0_2450, 0);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if [0xC4, 0xD8, 0xEC, 0xFF].contains(&c) {
        spark_pair(fx, sp, 5);
    }
    let c = fx.mem.i16(COUNTER);
    if [0xCE, 0xE2, 0xF5, 0xFF].contains(&c) {
        spark_pair(fx, sp, 6);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 1 {
        let s1 = sp + 0x44;
        jitter(fx, sp, 1, [(D, 0x01E0_15C8), (D, 0x01E0_15FC), (s1, 0x01E0_1634)]);
        fx.mem.set16(INDEX, 0);
        jitter(fx, sp, 3, [(s1, 0x01E0_1688), (s1, 0x01E0_16BC), (s1, 0x01E0_16F0)]);
        fx.mem.set16(INDEX, 0);
    }
    fx.leave(0x60);
}

/// The atlas rectangle of sprite `id` frame `sub` (into the entry's local
/// `local`) as D+0x94.
fn rect_d(fx: &mut Fx, local: u32, id: i32, sub: i32) {
    l2::rect(fx, local, id, sub);
    copy4(fx, D + 0x94, local);
}

/// D+0x20 = the task's i16 position (as floats).
fn centre_task(fx: &mut Fx, t: u32) {
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(D + 0x20 + 4 * k, v);
    }
}

/// The entry's draw of one live task (k its index, `count` the live tasks
/// so far: the entry's s3 / s1).
fn draw(fx: &mut Fx, sp: u32, t: u32, k: u32, count: u32) {
    let s = fx.mem.read_u32(t + 0x24);
    let sub = |fx: &mut Fx| fx.mem.i16(t + 0x34);
    match s {
        0x01E0_1740 => {
            let w = fx.mem.read_u32(D + 0x28C);
            fx.mem.write_u32(D + 0x88, w);
            let c = sub(fx);
            rect_d(fx, sp + 0x60, 0x6E, c);
        }
        0x01E0_1970 | 0x01E0_22E0 | 0x01E0_2450 => {
            let (tex, id, local, n, ra, table) = match s {
                0x01E0_1970 => (0x3578, 0x14, 0x70, 8, 0x01E0_31F8, BYTES),
                0x01E0_22E0 => (0x29C, 0x9B, 0xB0, 7, 0x01E0_34B8, BYTES_B),
                _ => (0x357C, 0x14, 0xC0, 5, 0x01E0_3580, BYTES_B),
            };
            let w = fx.mem.read_u32(D + tex);
            fx.mem.write_u32(D + 0x88, w);
            let c = sub(fx);
            rect_d(fx, sp + local, id, c);
            fx.mem.set16(D + 0xB8, n);
            let v = fx.rand_from(D, ra);
            fx.mem.set16(D + 0xBE, v % 4);
            centre_task(fx, t);
            quads4::ring_at_m(fx, uv::uv_rect, D, t, 0x20, table);
            fx.mem.set16(D + 0xB8, 0);
        }
        0x01E0_1B50 | 0x01E0_1EE0 => {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 1), (0x96, 0x10), (0x98, 0x1F), (0x9A, 1)] {
                fx.mem.set16(D + o, v);
            }
            let c = sub(fx);
            let a = s == 0x01E0_1B50;
            rect_d(fx, sp + if a { 0x80 } else { 0x90 }, 0x97, c);
            fx.mem.set16(D + 0x98, 1);
            for o in [0x20u32, 0x24, 0x28] {
                fx.mem.set32(D + o, 0);
            }
            fx.mem.set16(D + 0xBE, 1);
            let saves = |ra: u32| [(0xC0u32, u64::from(ra)), (0x60, u64::from(k)), (0x50, u64::from(D + 0x40 * k)), (0x40, u64::from(count)), (0x30, u64::from(D))];
            if a {
                ribbon(fx, t, D + 0x28F0, D + 0x2990, false, &saves(0x01E0_32FC));
                ribbon(fx, t, D + 0x2A30, D + 0x2AD0, false, &saves(0x01E0_3310));
            } else {
                ribbon(fx, t, D + 0x2F30, D + 0x2FD0, true, &saves(0x01E0_33AC));
                ribbon(fx, t, D + 0x3070, D + 0x3110, true, &saves(0x01E0_33C0));
            }
        }
        0x01E0_2090 => quads4::fan18(fx, D, t, sp + 0x110),
        0x01E0_2180 => {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            rect_d(fx, sp + 0xA0, 0x9A, 1);
            fx.mem.set16(D + 0xBE, 0);
            quarters_task(fx, t, sp + 0x110);
        }
        0x01E0_25C0 | 0x01E0_2860 => {
            let w = fx.mem.read_u32(D + 0x3570);
            fx.mem.write_u32(D + 0x88, w);
            let c = sub(fx);
            rect_d(fx, sp + if s == 0x01E0_25C0 { 0xD0 } else { 0xE0 }, 0x14, c);
            quads4::fan18_far(fx, D, t, sp + 0x110);
        }
        0x01E0_2A70 | 0x01E0_2BD0 => {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            rect_d(fx, sp + if s == 0x01E0_2A70 { 0xF0 } else { 0x100 }, 0x9A, 1);
            quads4::quarters_far_at(fx, D, t, sp + 0x110);
        }
        _ => {}
    }
}

/// The point of the Lagrange curve (44769f26f5eb's basis 0) through the
/// four points `p` at `t` into `out` (xyz; w left as it was), summed as the
/// original's accumulator chain sums it.
fn curve(fx: &mut Fx, out: u32, p: [u32; 4], t: f32) {
    use ee::{add, mul};
    let w = spline::weights(t, 0).unwrap_or([0.0; 4]);
    for c in 0..3u32 {
        let v: [f32; 4] = std::array::from_fn(|i| fx.mem.f(p[i] + 4 * c));
        let acc = add(mul(v[2], w[2]), add(mul(v[0], w[0]), mul(v[1], w[1])));
        fx.mem.setf(out + 4 * c, add(acc, mul(v[3], w[3])));
    }
}

/// 0x1E03E70 / 0x1E04890 (`frozen`) (D, task, a, b): a ribbon between the
/// point histories `a` and `b` (i16 +6 points; the first segment's point
/// before is the first point): per segment four gouraud strips (t = 0,
/// 0.25, 0.5, 0.75; vertices a(t), a(t + 1/4), b(t), b(t + 1/4) through
/// the curve [`curve`]) at key 0. 0x3E70 colours them +0x28 with the
/// alpha of the later pair down by alpha / (2 n) a strip; 0x4890 has
/// alpha 0 at the a side [F].
fn ribbon(fx: &mut Fx, t: u32, mut a: u32, mut b: u32, frozen: bool, saves: &[(u32, u64)]) {
    let frame = if frozen { 0x380 } else { 0x390 };
    let sp = fx.enter(frame);
    fx.saved(sp, saves);
    let o = if frozen { 0x10 } else { 0 };
    let d = sp + 0x2D8 - o;
    let colour = sp + if frozen { 0x37C } else { 0x38C };
    let ot = OT;
    let n = fx.mem.i16(t + 6);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(colour + k, v);
    }
    // The first segment's point arrays: (a0, a0, a1, a2) / (b0, b0, b1, b2).
    let (pa, pb, a0, b0) = (sp + 0x1B0 - o, sp + 0x1F0 - o, sp + 0x2B0 - o, sp + 0x2C0 - o);
    copy4(fx, a0, a);
    copy4(fx, pa, a0);
    copy4(fx, pa + 0x10, a0);
    copy4(fx, pa + 0x20, a + 0x10);
    copy4(fx, pa + 0x30, a + 0x20);
    copy4(fx, b0, b);
    copy4(fx, pb, b0);
    copy4(fx, pb + 0x10, b0);
    copy4(fx, pb + 0x20, b + 0x10);
    copy4(fx, pb + 0x30, b + 0x20);
    // The point arrays' words the loop keeps in registers / locals.
    let kept: [(u32, u32); 11] = if frozen {
        [(0xD4, 0x1B0), (0xD8, 0x1C0), (0xDC, 0x1D0), (0xE0, 0x1B4), (0xE4, 0x1A4), (0xE8, 0x1C4), (0xEC, 0x1D4), (0xF0, 0x1B8), (0xF4, 0x1A8), (0xF8, 0x1C8), (0xFC, 0x1D8)]
    } else {
        [(0xDC, 0x1C0), (0xE0, 0x1D0), (0xE4, 0x1E0), (0xE8, 0x1C4), (0xEC, 0x1B4), (0xF0, 0x1D4), (0xF4, 0x1E4), (0xF8, 0x1C8), (0xFC, 0x1B8), (0x100, 0x1D8), (0x104, 0x1E8)]
    };
    if frozen {
        fx.mem.set32(sp + 0x100, n - 1);
    } else {
        fx.mem.write_u32(sp + 0x108, ot);
        fx.mem.set32(sp + 0x110, n - 1);
    }
    for (to, from) in kept {
        let w = fx.mem.read_u32(sp + from);
        fx.mem.write_u32(sp + to, w);
    }
    if frozen {
        let c = sp + 0x358;
        crate::c061::rgbaq_at(fx, c, colour, d + 0x30);
        crate::c061::rgbaq_at(fx, c + 8, colour, d + 0x40);
        fx.mem.write_u8(colour + 3, 0);
        crate::c061::rgbaq_at(fx, c + 0x10, colour, d + 0x10);
        crate::c061::rgbaq_at(fx, c + 0x18, colour, d + 0x20);
    }
    // Locals: (a t, b t, a t', b t') per path and their copies.
    let (near, first) = if frozen { ([0x220u32, 0x230, 0x240, 0x250], [0x260u32, 0x270, 0x280, 0x290]) } else { ([0x230, 0x240, 0x250, 0x260], [0x270, 0x280, 0x290, 0x2A0]) };
    let copies = if frozen { [0x160u32, 0x180, 0x170, 0x190] } else { [0x170, 0x190, 0x180, 0x1A0] };
    let quad = sp + 0x130 - o;
    let mean = sp + 0x120 - o;
    let mut seg = 0i32;
    while seg < n - 1 {
        let mut tt = 0.0f32;
        for _ in 0..4 {
            if !frozen {
                let c = sp + 0x368;
                crate::c061::rgbaq_at(fx, c, colour, d + 0x10);
                crate::c061::rgbaq_at(fx, c + 8, colour, d + 0x30);
                let a = i32::from(fx.mem.read_u8(t + 0x2B));
                let q = fx.div(a, 2 * n).0 as i16 as i32;
                let v = i32::from(fx.mem.read_u8(colour + 3)) - q;
                fx.mem.write_u8(colour + 3, if v > 0 { v as u8 } else { 0 });
                crate::c061::rgbaq_at(fx, c + 0x10, colour, d + 0x20);
                crate::c061::rgbaq_at(fx, c + 0x18, colour, d + 0x40);
            }
            let nt = ee::add(tt, 0.25);
            let (pts_a, pts_b, l) = if seg == 0 {
                let pa4 = [pa, pa + 0x10, pa + 0x20, pa + 0x30];
                let pb4 = [pb, pb + 0x10, pb + 0x20, pb + 0x30];
                (pa4, pb4, first)
            } else {
                let pa4 = [a - 0x10, a, a + 0x10, a + 0x20];
                let pb4 = [b - 0x10, b, b + 0x10, b + 0x20];
                (pa4, pb4, near)
            };
            curve(fx, sp + l[0], pts_a, tt);
            copy4(fx, sp + copies[0], sp + l[0]);
            curve(fx, sp + l[1], pts_b, tt);
            copy4(fx, sp + copies[1], sp + l[1]);
            curve(fx, sp + l[2], pts_a, nt);
            copy4(fx, sp + copies[2], sp + l[2]);
            curve(fx, sp + l[3], pts_b, nt);
            copy4(fx, sp + copies[3], sp + l[3]);
            l2::copy3(fx, quad, sp + copies[0]);
            l2::copy3(fx, quad + 0x10, sp + copies[2]);
            l2::copy3(fx, quad + 0x20, sp + copies[1]);
            l2::copy3(fx, quad + 0x30, sp + copies[3]);
            if l2::project(fx, quad, SPR, 0, d, mean) {
                fx.submit(d, ot, 0);
            }
            tt = nt;
        }
        a += 0x10;
        b += 0x10;
        seg += 1;
    }
    fx.leave(frame);
}

/// 0x1E05290(D, task, desc, 0): `quads4::quarters_far_at` at the task's
/// i16 position (frame 0x130), each quarter keyed by its mean w (0x1A2C70)
/// at D+0x80 [F].
fn quarters_task(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x130);
    let (quad, mean, scale, out, rot, trans) = (sp + 0x50, sp + 0xB0, sp + 0xA0, sp + 0xC0, sp + 0x100, sp + 0x90);
    l2::desc_strip3(fx, d);
    for (local, c, to) in [(0x110u32, 0x2Cu32, 0x10u32), (0x118, 0x2C, 0x28), (0x120, 0x2C, 0x40), (0x128, 0x28, 0x58)] {
        crate::c061::rgbaq_at(fx, sp + local, t + c, d + to);
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(scale + 4 * k, 0x3D4C_CCCD);
    }
    l2::camera_angles_to(fx, rot);
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(trans + 4 * k, v);
    }
    l2::xform_vp(fx, out, scale, rot, trans);
    let z = fx.mem.i16(t + 0x3A);
    fx.mem.set32(rot + 8, z);
    fx.mem.set32(rot + 4, 0);
    fx.mem.set32(rot, 0);
    for q in 0..4 {
        let (a, b) = (fx.mem.i16(t + 8), fx.mem.i16(t + 0xA));
        l2::quad_ints(fx, quad, a, b, q);
        l2::rotate4(fx, quad, rot, 2, D + 0xD8);
        if l2::project(fx, quad, SPR, out, d, mean) {
            let m = fx.mem.f(mean);
            fx.fix_residue(m, 0x01E0_5434);
            let key = ee::fixunssfsi(m) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x130);
}
