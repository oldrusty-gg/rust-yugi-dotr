//! BMODULE b573 (card 80), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03C60, data area 0x1ED0000) [F]. The entry takes (D, param). A task pool of 140 tasks of 0x40
//! bytes at D+0x2B0 (alive +0x36, step +0x24 as a record address), spawned
//! by 0x1E04A20, drawn with the second library: rings of three quads with
//! a mode (383a1b86ead2 at 0x1E00BC0, `quads4::ring_at_m`), 18-strip
//! camera-facing bands (7f395ce1b4cc at 0x1E01120, `quads4::band18`), far-Z
//! quarter quads (a6d0a94b7c36 at 0x1E00920, `quads4::quarters_far`) and
//! the record's own flash sprite (0x1E04B50), smoke quads (0x1E04ED0) and
//! ring of 18 quads (0x1E05110). Update 1 dims the stage (0x1E038E0); on
//! updates 46, 67, 87, 107, 127 bursts gather about the attacker at five
//! points beside it, from 12 to 166 and 195 to 220 sparks run along its
//! node 0x50 / 0x39 path; from 220 a spiralling column rises (node 0x39
//! start), from 354 to 373 the target's random nodes throw sparks, and on
//! 156 the frame is tinted (0x1E03A10). Returns 1 on update 290, 3 on 380,
//! 6 on 301..374, 8 once the 400-update timer D+0x2B20 has run out.
//!
//! The step 0x1E039C0 (an object op through 0x1F8380, the entry's own case
//! for it) is never spawned [F: the emitter's spawn `jal`s name the
//! steps]; it is not ported.
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib::ALPHA_ADD;
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0x8C;
const POOL: u32 = D + 0x2B0;
const COLOUR: u32 = D + 0x2B24;
const TIMER: u32 = D + 0x2B20;
const OTW: u32 = D + 0x2B2C;
const RADIUS: u32 = D + 0x2B3C;
const SPIRAL: u32 = D + 0x2B40;
const NODES: u32 = D + 0x2B54;
const TNODES: u32 = D + 0x2B64;
const SUBS: u32 = D + 0x2B74;
const ANGLE: u32 = D + 0x2BB2;
const SPIN: u32 = D + 0x2BB4;
const INDEX: u32 = D + 0x2BB8;
const FACING: u32 = D + 0x2BBA;
const RING: u32 = D + 0x2BBC;
const COUNTER: u32 = D + 0x2BC0;
const BYTES: u32 = D + 0x2BC2;
const TAB: u32 = D + 0x2DC4;
const POINTS: u32 = D + 0x25E0;
const TRAIL: u32 = D + 0x2630;
const COLUMN: u32 = D + 0x2A90;
const HEAD: u32 = D + 0x2AE0;
const POS: u32 = D + 0x250;

const BURSTS: [i32; 5] = [0x2E, 0x43, 0x57, 0x6B, 0x7F];

fn burst(c: i32) -> Option<u32> {
    BURSTS.iter().position(|&b| b == c).map(|k| k as u32)
}

/// rand % m (HI) from the `jal` at `site` (rand's frame: `s0`, ra).
fn rmod(fx: &mut Fx, s0: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, site + 8);
    fx.div(v, m).1
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 0x1A2C30 / 0x1A3118 / 0x1A38F8: (float) (2.5 + (double) v) through
/// the soft-float double add (exact; one rounding to float) [F].
fn plus_2_5(v: f32) -> f32 {
    (2.5f64 + f64::from(v)) as f32
}

/// The entry 0x1E03C60 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1A0);
    fx.mem.write_u32(OTW, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x1A0);
    ret
}

/// The eight target nodes D+0x2B64: a random node 1..n-1 (1 for 0) of the
/// target's n, the first requested clearing (rand's frame: s0 = n).
fn target_nodes(fx: &mut Fx, site: u32) {
    for k in 0..8u32 {
        let h = fx.target_handle();
        let n = fx.unit_nodes(h);
        let v = fx.rand_from(n as u32, site + 8);
        let mut r = fx.div(v, n).1;
        if r == 0 {
            r += 1;
        }
        let h = fx.target_handle();
        let q = fx.request_node(h, r, if k == 0 { 1 } else { 0 });
        fx.mem.set16(TNODES + 2 * k, q);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2F30);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x2BBE, param);
    fx.mem.set32(TIMER, 0x190);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xF0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xF4)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for (k, (node, clear)) in [(0x30, 1), (0x3F, 0), (0x50, 0), (0x39, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    target_nodes(fx, 0x01E0_3D7C);
    // 0x1E03BB0(0, D): textures.
    for k in 0..5u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x270 + 4 * k, t);
    }
    for (k, id) in (0xEu32..0x10).enumerate() {
        let t = fx.texture(4, id as i32);
        fx.mem.write_u32(D + 0x2A8 + 4 * k as u32, t);
    }
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x2B00 + 4 * k, t);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3E08, 0xC);
        fx.mem.set8(BYTES + k, v - 6);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(k, 0x01E0_3E44);
        fx.mem.set8(D + 0x2CC2 + k, v % 8 - 4);
    }
    for k in 0..0x1Eu32 {
        let v = rmod(fx, D + 2 * k, 0x01E0_3E7C, 0xF);
        fx.mem.set16(SUBS + 2 * k, v);
    }
    fx.mem.write_u32(RADIUS, 0x4040_0000);
    0
}

/// The steps (their addresses, also the tasks' +0x24) and the entry's
/// return address after each `jal`.
const STEPS: [(u32, u64); 15] = [
    (0x01E0_2590, 0x01E0_3EF8),
    (0x01E0_26E0, 0x01E0_3F1C),
    (0x01E0_27D0, 0x01E0_3F40),
    (0x01E0_2960, 0x01E0_3F64),
    (0x01E0_2AB0, 0x01E0_3F88),
    (0x01E0_2C00, 0x01E0_3FAC),
    (0x01E0_2E60, 0x01E0_3FD0),
    (0x01E0_2F90, 0x01E0_3FF4),
    (0x01E0_3080, 0x01E0_4018),
    (0x01E0_3160, 0x01E0_403C),
    (0x01E0_3370, 0x01E0_4060),
    (0x01E0_3570, 0x01E0_4084),
    (0x01E0_36E0, 0x01E0_40A8),
    (0x01E0_38E0, 0x01E0_40CC),
    (0x01E0_3A10, 0x01E0_4140),
];

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let s = fx.mem.read_u32(t + 0x24);
        if let Some(&(a, ra)) = STEPS.iter().find(|p| p.0 == s) {
            run_step(fx, a, ra, t);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x70);
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_4164), (0, 0)]);
    let z = ee::sub(fx.mem.f(sp + 0x78), 25.0);
    fx.mem.setf(sp + 0x78, z);
    let mut count = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        count += 1;
        draw(fx, sp, t, count);
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0x122 {
        1
    } else if c == 0x17C {
        3
    } else if (0x12D..0x177).contains(&c) {
        6
    } else if fx.mem.i32(TIMER) <= 0 {
        8
    } else {
        0
    }
}

fn run_step(fx: &mut Fx, a: u32, ra: u64, t: u32) {
    match a {
        0x01E0_2590 => sparks_a(fx, t, ra),
        0x01E0_26E0 => flare(fx, t, ra),
        0x01E0_27D0 => glow(fx, t, ra),
        0x01E0_2960 => quarters(fx, t, ra),
        0x01E0_2AB0 => halo(fx, t, ra),
        0x01E0_2C00 => sparks_b(fx, t, ra),
        0x01E0_2E60 => dark_band(fx, t, ra),
        0x01E0_2F90 => smoke(fx, t, ra),
        0x01E0_3080 => yellow(fx, t, ra),
        0x01E0_3160 => sparks_c(fx, t, ra, 0x01E0_3160, (0x14, 0xA), (0x1E, 0x1E), 0xF, 0xA, 0x81, 0xB, 9, false),
        0x01E0_3370 => sparks_c(fx, t, ra, 0x01E0_3370, (0x1E, 0x28), (0x64, 0x50), 0x2D, 3, 0x80, 0x1F, 0x1D, true),
        0x01E0_3570 => blue_band(fx, t, ra),
        0x01E0_36E0 => sparks_c(fx, t, ra, 0x01E0_36E0, (0x1E, 0x28), (0x64, 0x50), 0x7D, 3, 0x80, 0x1F, 0x1D, true),
        0x01E0_38E0 => dim(fx, t),
        _ => tint(fx, t, ra),
    }
}

/// A step's frame of 0x20 (ra, s0 = the entry's 0) or 0x30 (ra, s1 = D,
/// s0 = 0).
fn frame(fx: &mut Fx, size: u32, ra: u64) {
    let sp = fx.enter(size);
    if size == 0x20 {
        fx.saved(sp, &[(0x10, ra), (0, 0)]);
    } else {
        fx.saved(sp, &[(0x20, ra), (0x10, u64::from(D)), (0, 0)]);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// The alpha +0x2B: up by `up` while under `cap`, or (when `out`) down by
/// `down` to the floor `floor`, then the task ends.
fn fade_in(fx: &mut Fx, t: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a < cap {
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(up));
    }
}

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

/// 0x1E02590 (D, task): grey-blue sparks: size 1, radius from +0x30 kept
/// in +0xA, row 0..34, growth 2..6, life 2..4; in by 0x40, out by 7.
fn sparks_a(fx: &mut Fx, t: u32, ra: u64) {
    frame(fx, 0x20, ra);
    let s = 0x01E0_2590;
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 1);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x80);
        let r = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, r);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, s + 0x40, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, s + 0x5C, 5);
        fx.mem.set16(t + 0xE, v + 2);
        let v = rmod(fx, t, s + 0x7C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, s + 0x98, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0xC, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x81, 0x40);
    } else {
        fade_out(fx, t, 0x2B, 9, 7, true);
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E026E0 (D, task): a white-red flare band, size 30, three updates
/// in by 0x50, out by 9.
fn flare(fx: &mut Fx, t: u32, ra: u64) {
    frame(fx, 0x20, ra);
    l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
    l2::rgb(fx, t + 0x2C, 0x1F, 0x1F, 0xFF);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x1E);
        let v = rmod(fx, t, 0x01E0_272C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 3 {
        fade_in(fx, t, 0xA0, 0x50);
    } else {
        fade_out(fx, t, 0x2B, 0xB, 9, true);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E027D0 (D, task): the attacker's glow sprite: size 4 growing,
/// alternating grey; from update 281 it turns and grows to 90.
fn glow(fx: &mut Fx, t: u32, ra: u64) {
    frame(fx, 0x30, ra);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 4);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0xE, 1);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0x46 {
        fade_in(fx, t, 0x90, 0xE);
    } else {
        fade_out(fx, t, 0x2B, 0xF, 0xD, true);
        let s = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, s + 9);
    }
    let g = if fx.mem.i16(t + 0x32) % 2 == 1 { 0x80 } else { 0x60 };
    l2::rgb(fx, t + 0x28, g, g, g);
    let s = fx.mem.i16(t + 0x38);
    if s < 0x1E {
        fx.mem.set16(t + 0x38, s + 2);
    }
    if fx.mem.i16(COUNTER) >= 0x119 {
        let a = fx.mem.i16(t + 0x3A);
        if a < 0x600 {
            fx.mem.set16(t + 0x3A, a + 0x30);
        }
    }
    if fx.mem.i16(COUNTER) >= 0x119 {
        let s = fx.mem.i16(t + 0x38);
        if s < 0x5A {
            fx.mem.set16(t + 0x38, s + 1);
        }
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02960 (D, task): the blue quarters, size (3, 8) widening by 8,
/// turning 10 a update; two updates in by 0x40, out by 0xD.
fn quarters(fx: &mut Fx, t: u32, ra: u64) {
    frame(fx, 0x20, ra);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x2E);
        l2::rgb(fx, t + 0x28, 0x30, 0x30, 0x80);
        l2::rgb(fx, t + 0x2C, 0x30, 0x30, 0x80);
        fx.mem.set16(t + 8, 3);
        fx.mem.set16(t + 0xA, 8);
        let e = if fx.mem.i16(t + 0x3A) == 0 { 0 } else { 1 };
        fx.mem.set16(t + 0xE, e);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        let a = fx.mem.read_u8(t + 0x2F);
        fx.mem.write_u8(t + 0x2F, a.wrapping_add(0x40));
    } else {
        fade_out(fx, t, 0x2B, 0xF, 0xD, true);
        fade_out(fx, t, 0x2F, 0xF, 0xD, false);
    }
    let w = fx.mem.i16(t + 0xA);
    fx.mem.set16(t + 0xA, w + 8);
    age(fx, t);
    let a = fx.mem.i16(t + 0x3A);
    fx.mem.set16(t + 0x3A, a + 0xA);
    fx.leave(0x20);
}

/// 0x1E02AB0 (D, task): the yellow halo ring: cell 0..6, ten quads of
/// size 4..6; in by 0xC for 20 updates, out by 9 from update 221.
fn halo(fx: &mut Fx, t: u32, ra: u64) {
    frame(fx, 0x30, ra);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0);
        let v = rmod(fx, t, 0x01E0_2AD4, 7);
        fx.mem.set16(t + 0x34, v);
        fx.mem.set16(t + 0x36, 2);
        let v = rmod(fx, t, 0x01E0_2AF8, 3);
        fx.mem.set16(t + 8, v + 4);
        fx.mem.set16(t + 6, 0xA);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x20);
        l2::rgb(fx, t + 0x2C, 0x80, 0x80, 0x20);
    }
    if fx.mem.i16(t + 0x32) < 0x14 {
        fade_in(fx, t, 0x80, 0xC);
        let a = fx.mem.read_u8(t + 0x2F);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2F, a.wrapping_add(0xC));
        }
    } else if fx.mem.i16(COUNTER) >= 0xDD {
        fade_out(fx, t, 0x2B, 0xB, 9, true);
        fade_out(fx, t, 0x2F, 0xB, 9, false);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02C00 (D, task): the trail sparks (yellow before update 195, blue
/// after; larger and longer from 221).
fn sparks_b(fx: &mut Fx, t: u32, ra: u64) {
    frame(fx, 0x30, ra);
    let s = 0x01E0_2C00;
    if fx.mem.i16(t + 0x36) == 1 {
        let early = fx.mem.i16(COUNTER) < 0xC3;
        fx.mem.set16(t + 0x38, if early { 1 } else { 4 });
        if fx.mem.i16(COUNTER) >= 0xDD {
            fx.mem.set16(t + 0x38, 0x12);
        }
        let v = fx.rand_from(t, s + 0x64);
        fx.mem.set16(t + 0x34, v % 4);
        if fx.mem.i16(COUNTER) < 0xC3 {
            l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x20);
        } else {
            l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x80);
        }
        let r = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, r);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, s + 0xC4, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let e = if fx.mem.i16(COUNTER) < 0xC3 { 1 } else { 5 };
        fx.mem.set16(t + 0xE, e);
        if fx.mem.i16(COUNTER) >= 0xDD {
            fx.mem.set16(t + 0xE, 0xF);
        }
        let v = rmod(fx, t, s + 0x120, 0x23);
        fx.mem.set16(t + 8, v);
        if fx.mem.i16(COUNTER) < 0xC3 {
            let v = rmod(fx, t, s + 0x150, 3);
            fx.mem.set16(t + 6, v + 1);
        } else {
            let v = rmod(fx, t, s + 0x178, 3);
            fx.mem.set16(t + 6, v + 4);
        }
        if fx.mem.i16(COUNTER) >= 0xDD {
            let v = rmod(fx, t, s + 0x1AC, 3);
            fx.mem.set16(t + 6, v + 6);
        }
        fx.mem.set16(t + 0xC, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x81, 0x40);
    } else {
        fade_out(fx, t, 0x2B, 0xB, 9, true);
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02E60 (D, task): the dark band (inner grey 0x10, outer cyan),
/// size 23 growing by 6 once it fades out by 0xE.
fn dark_band(fx: &mut Fx, t: u32, ra: u64) {
    frame(fx, 0x20, ra);
    l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x10);
    l2::rgb(fx, t + 0x2C, 0x1F, 0x80, 0x80);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x17);
        let v = rmod(fx, t, 0x01E0_2EAC, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x34, 0);
        let v = rmod(fx, t, 0x01E0_2ECC, 0xA);
        let e = fx.mem.i16(t + 0xE) + (v + 3) as i16 as i32;
        fx.mem.set16(t + 0xE, e);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 0xE) {
        fade_in(fx, t, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x2B, 0x10, 0xE, true);
        let s = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, s + 6);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02F90 (D, task): the grey smoke quad (size 6, cell 2); in by 0x14
/// for 8 updates, out by 5 from update 151.
fn smoke(fx: &mut Fx, t: u32, ra: u64) {
    frame(fx, 0x30, ra);
    l2::rgb(fx, t + 0x28, 0x4F, 0x4F, 0x4F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        let v = rmod(fx, t, 0x01E0_2FD0, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x34, 2);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 8 {
        fade_in(fx, t, 0xA0, 0x14);
    } else if fx.mem.i16(COUNTER) >= 0x97 {
        fade_out(fx, t, 0x2B, 7, 5, true);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03080 (D, task): yellow, cell 10; in by 0xC for 20 updates, out by
/// 9 from update 221 (never drawn: the entry has no case for it).
fn yellow(fx: &mut Fx, t: u32, ra: u64) {
    frame(fx, 0x30, ra);
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x20);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x20);
        fx.mem.set16(t + 0x34, 0xA);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0x14 {
        fade_in(fx, t, 0x80, 0xC);
    } else if fx.mem.i16(COUNTER) >= 0xDD {
        fade_out(fx, t, 0x2B, 0xB, 9, true);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03160 / 0x1E03370 / 0x1E036E0 (D, task) at `s`: white (blue for
/// 0x36E0) sparks: size rand % size.0 + size.1, cell 0..7, the radius
/// +0x30 kept in +0xA and set to rand % radius.0 + radius.1, growth
/// `grow`, row 0..34, life 0..2 + `life`; in by 0x40 (under `cap`), out
/// by `down` (floor `floor`); every third update bright, the cell
/// counting 0..7 (always when `cycle`).
#[allow(clippy::too_many_arguments)]
fn sparks_c(fx: &mut Fx, t: u32, ra: u64, s: u32, size: (i32, i32), radius: (i32, i32), grow: i32, life: i32, cap: u8, floor: u8, down: u8, cycle: bool) {
    frame(fx, 0x20, ra);
    let blue = s == 0x01E0_36E0;
    if fx.mem.i16(t + 0x36) == 1 {
        let v = rmod(fx, t, s + 0x1C, size.0);
        fx.mem.set16(t + 0x38, v + size.1);
        let v = fx.rand_from(t, s + 0x44);
        fx.mem.set16(t + 0x34, v % 8);
        if blue {
            l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x80);
        } else {
            l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        }
        let r = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, r);
        let v = rmod(fx, t, s + 0x74, radius.0);
        fx.mem.set16(t + 0x30, v + radius.1);
        let v = rmod(fx, t, s + 0x94, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0xE, grow);
        let v = rmod(fx, t, s + 0xB8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, s + 0xD4, 3);
        fx.mem.set16(t + 6, v + life);
        fx.mem.set16(t + 0xC, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, cap, 0x40);
    } else {
        fade_out(fx, t, 0x2B, floor, down, true);
    }
    let n = fx.mem.i16(t + 0x32);
    let g = if fx.div(n, 3).1 == 1 { 0xC0 } else { 0x40 };
    l2::rgb(fx, t + 0x28, g, g, g);
    let n = fx.mem.i16(t + 0x32);
    if cycle || fx.div(n, 3).1 == 1 {
        let c = fx.mem.i16(t + 0x34);
        fx.mem.set16(t + 0x34, if c < 7 { c + 1 } else { 0 });
    }
    if cycle {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03570 (D, task): the blue column band (size 500); in by 0x40 for
/// 4 updates, flickering grey / bright until update 375, then out by 0xE.
fn blue_band(fx: &mut Fx, t: u32, ra: u64) {
    frame(fx, 0x30, ra);
    l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
    l2::rgb(fx, t + 0x2C, 0x80, 0x80, 0xF0);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x1F4);
        let v = rmod(fx, t, 0x01E0_35C4, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0xE, 4);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 0xE) {
        fade_in(fx, t, 0x80, 0x40);
    } else {
        let c = fx.mem.i16(COUNTER);
        if c < 0x177 {
            let g = if c % 2 == 1 { 0x80 } else { 0xC0 };
            l2::rgb(fx, t + 0x28, g, g, g);
        } else {
            fade_out(fx, t, 0x2B, 0x10, 0xE, true);
        }
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E038E0 (D, task): the stage light (0x1F7F10) from +0x30: 0x80 down
/// by 10 to 0x30 while the timer is at least 21, then back up by 10 to
/// 0x80 (the task ends there).
fn dim(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x10);
    fx.saved(sp, &[(0, 0x01E0_40CC)]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x30, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i32(TIMER) >= 0x15 {
        let v = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x30, if v < 0x31 { 0x30 } else { v - 0xA });
    } else {
        let v = fx.mem.i16(t + 0x30);
        if v < 0x80 {
            fx.mem.set16(t + 0x30, v + 0xA);
        } else {
            fx.mem.set16(t + 0x30, 0x80);
            fx.mem.set16(t + 0x36, 0);
        }
        if fx.mem.i16(t + 0x30) >= 0x81 {
            fx.mem.set16(t + 0x30, 0x80);
        }
    }
    let v = fx.mem.read_u8(t + 0x30);
    fx.set_stage_light(i32::from(v));
    fx.leave(0x10);
}

/// 0x1E03A10 (D, task): the frame tint (frame 0x90; its descriptor at
/// sp + 0x38 = the library frame quad's, [`Fx::frame_quad_rgba`] from this
/// caller) in (0x80, 0x80, 0x80, alpha +0x2B): in by 0xB to 0x68 for 175
/// updates, then out by 0xC; the task ends when the timer reads 9.
fn tint(fx: &mut Fx, t: u32, ra: u64) {
    let a = fx.mem.read_u8(t + 0x2B);
    fx.frame_quad_rgba([0x80, 0x80, 0x80, a]);
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, ra), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x32) < 0xAF {
        fade_in(fx, t, 0x68, 0xB);
    } else {
        fade_out(fx, t, 0x2B, 0xE, 0xC, true);
    }
    age(fx, t);
    if fx.mem.i32(TIMER) == 9 {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.leave(0x90);
}

/// 0x1E04A20(D, step, a2): a new task (first free of 140) with the spawn
/// colour D+0x2B24 (both colours), at D+0x250, index D+0x2BB8, angle
/// D+0x2BB2.
fn spawn(fx: &mut Fx, step: u32) {
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
        let c: [i32; 4] = std::array::from_fn(|k| i32::from(fx.mem.read_u8(COLOUR + k as u32)));
        l2::rgb(fx, t + 0x28, c[0], c[1], c[2]);
        fx.mem.write_u8(t + 0x2B, c[3] as u8);
        let c: [i32; 4] = std::array::from_fn(|k| i32::from(fx.mem.read_u8(COLOUR + k as u32)));
        l2::rgb(fx, t + 0x2C, c[0], c[1], c[2]);
        fx.mem.write_u8(t + 0x2F, c[3] as u8);
        l2::to_i16x4(fx, t, POS, 0);
        let i = fx.mem.i16(INDEX);
        fx.mem.set16(t + 0x30, i);
        let a = fx.mem.i16(ANGLE);
        fx.mem.set16(t + 0x3A, a);
        fx.mem.set32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, step);
    }
    fx.leave(0x60);
}

/// Spins D+0x254 / D+0x250 by the radius D+0x2B3C at `deg` (sine, then
/// cosine from the table D+0x2DC4).
fn offset(fx: &mut Fx, deg: i32) {
    let s = l2::sin_deg(fx, TAB, deg, 0);
    let v = ee::add(fx.mem.f(POS + 4), ee::mul(fx.mem.f(RADIUS), s));
    fx.mem.setf(POS + 4, v);
    let c = l2::sin_deg(fx, TAB, deg, 1);
    let v = ee::add(fx.mem.f(POS), ee::mul(fx.mem.f(RADIUS), c));
    fx.mem.setf(POS, v);
}

/// The trail point index D+0x2BBC on by one (0..69).
fn ring_next(fx: &mut Fx) {
    let r = fx.mem.i16(RING) + 1;
    fx.mem.set16(RING, r);
    if fx.mem.i16(RING) >= 0x46 {
        fx.mem.set16(RING, 0);
    }
}

/// 0x1E015C0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x70).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x70);
    // The entry's ra, s3 (the runner's: 0), s2 = param, s1 = D, s0 = 0.
    fx.saved(sp, &[(0x40, 0x01E0_3EC4), (0x30, 0), (0x20, param as i64 as u64), (0x10, u64::from(D)), (0, 0)]);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x60);
    l2::rgb(fx, COLOUR, 0x80, 0, 0);
    fx.mem.write_u8(COLOUR + 3, 0);
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    let c = fx.mem.i16(COUNTER);
    if c == 1 {
        spawn(fx, 0x01E0_38E0);
    }
    if c == 0x9C {
        spawn(fx, 0x01E0_3A10);
    }
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x50);
    if let Some(i) = burst(c) {
        let x = fx.mem.read_u32(sp + 0x50);
        fx.mem.write_u32(POS, x);
        let y = plus_2_5(fx.mem.f(sp + 0x54));
        fx.mem.setf(POS + 4, y);
        let z = fx.mem.read_u32(sp + 0x58);
        fx.mem.write_u32(POS + 8, z);
        let angles = if fx.mem.i16(FACING) != 0 { [0x14, 0xA0, 0x136, 0x5A, 0xE6] } else { [0xA0, 0x14, 0xE6, 0x5A, 0x136] };
        offset(fx, angles[i as usize]);
        fx.mem.set16(INDEX, i as i32);
        copy4(fx, POINTS + 0x10 * i, POS);
        fx.mem.set16(ANGLE, 0);
        spawn(fx, 0x01E0_2960);
        fx.mem.set16(ANGLE, 0x5A);
        spawn(fx, 0x01E0_2960);
        spawn(fx, 0x01E0_2F90);
    }
    if c == 0x7D {
        spawn(fx, 0x01E0_3080);
    }
    if c == 0x7D {
        spawn(fx, 0x01E0_2AB0);
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES + 2);
    fx.node_position(h, n, sp + 0x50);
    if c == 0x96 {
        spawn(fx, 0x01E0_27D0);
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES);
    fx.node_position(h, n, sp + 0x50);
    if let Some(i) = burst(c) {
        copy4(fx, POS, POINTS + 0x10 * i);
        spawn(fx, 0x01E0_2590);
        spawn(fx, 0x01E0_2590);
        spawn(fx, 0x01E0_26E0);
    }
    if (0x2E..0x8F).contains(&c) {
        let i = BURSTS.iter().filter(|&&b| c >= b).count() as i32 - 1;
        fx.mem.set16(INDEX, i);
        spawn(fx, 0x01E0_2590);
    }
    if (0xC..0xA7).contains(&c) {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 4);
        let r = fx.mem.i16(RING);
        fx.node_position(h, n, (TRAIL as i32 + 0x10 * r) as u32);
        let r = fx.mem.i16(RING);
        fx.mem.set16(INDEX, r);
        spawn(fx, 0x01E0_2C00);
        ring_next(fx);
    }
    if let Some(i) = burst(c) {
        fx.mem.set16(INDEX, 5 + i as i32);
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 4);
        let i = fx.mem.i16(INDEX) - 5;
        fx.node_position(h, n, (COLUMN as i32 + 0x10 * i) as u32);
        spawn(fx, 0x01E0_26E0);
    }
    if (0xC3..0xDD).contains(&c) {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 6);
        let r = fx.mem.i16(RING);
        fx.node_position(h, n, (TRAIL as i32 + 0x10 * r) as u32);
        let r = fx.mem.i16(RING);
        fx.mem.set16(INDEX, r);
        spawn(fx, 0x01E0_2C00);
        ring_next(fx);
    }
    if (0xDC..0x16B).contains(&c) {
        if c == 0xDC {
            let h = fx.self_handle();
            let n = fx.mem.i16(NODES + 6);
            fx.node_position(h, n, HEAD);
        }
        let r = fx.mem.i16(RING);
        let p = (TRAIL as i32 + 0x10 * r) as u32;
        copy4(fx, p, HEAD);
        for k in 0..3u32 {
            let r = fx.mem.i16(RING);
            let p = (TRAIL as i32 + 0x10 * r) as u32 + 4 * k;
            let v = ee::add(fx.mem.f(p), fx.mem.f(HEAD + 0x10 + 4 * k));
            fx.mem.setf(p, v);
        }
        let r = fx.mem.i16(RING);
        fx.mem.set16(INDEX, r);
        spawn(fx, 0x01E0_2C00);
        if fx.mem.i16(COUNTER) >= 0xEC {
            spawn(fx, 0x01E0_2E60);
        }
        if fx.mem.i16(COUNTER) >= 0xF0 {
            spawn(fx, 0x01E0_3160);
        }
        let c = fx.mem.i16(COUNTER);
        if c == 0x162 {
            spawn(fx, 0x01E0_3570);
        }
        let c = fx.mem.i16(COUNTER);
        if (0x162..0x171).contains(&c) {
            spawn(fx, 0x01E0_36E0);
        }
        ring_next(fx);
        let c = fx.mem.i16(COUNTER);
        if c >= 0xF0 {
            let up = fx.mem.i16(FACING) == 0;
            let d = if c >= 0x12C { 2.0 } else { f32::from_bits(0x3F33_3333) };
            let y = fx.mem.f(HEAD + 8);
            fx.mem.setf(HEAD + 8, if up { ee::add(y, d) } else { ee::sub(y, d) });
            let a = fx.mem.i16(SPIN);
            let s = l2::sin_deg(fx, TAB, a, 0);
            let v = ee::mul(fx.mem.f(SPIRAL), s);
            fx.mem.setf(HEAD + 0x10, v);
            let a = fx.mem.i16(SPIN);
            let cs = l2::sin_deg(fx, TAB, a, 1);
            let v = ee::mul(2.0, ee::mul(fx.mem.f(SPIRAL), cs));
            fx.mem.setf(HEAD + 0x14, v);
            let c = fx.mem.i16(COUNTER);
            let r = fx.mem.f(SPIRAL);
            let tenth = f32::from_bits(0x3DCC_CCCD);
            if c < 0x15F {
                let cap = if c >= 0x140 { 1.0 } else { 3.0 };
                if crate::exe::ee_lt(r, cap) {
                    fx.mem.setf(SPIRAL, ee::add(r, tenth));
                }
            } else if crate::exe::ee_le(r, 0.0) {
                fx.mem.set32(SPIRAL, 0);
            } else {
                fx.mem.setf(SPIRAL, ee::sub(r, f32::from_bits(0x3F19_999A)));
            }
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x162..0x176).contains(&c) {
        if fx.div(c, 5).1 == 1 {
            target_nodes(fx, 0x01E0_2438);
        } else {
            for k in 0..3u32 {
                let v = fx.rand_from(k, 0x01E0_24CC);
                fx.mem.set16(INDEX, v % 8);
                let h = fx.target_handle();
                let i = fx.mem.i16(INDEX);
                let n = fx.mem.i16((TNODES as i32 + 2 * i) as u32);
                fx.node_position(h, n, sp + 0x50);
                copy3(fx, POS, sp + 0x50);
                spawn(fx, 0x01E0_3370);
            }
        }
    }
    let a = fx.mem.i16(SPIN) + 9;
    fx.mem.set16(SPIN, a);
    let a = fx.mem.i16(SPIN);
    if a >= 0x168 {
        let r = fx.div(a, 0x168).1;
        fx.mem.set16(SPIN, r);
    }
    fx.leave(0x70);
}

/// The atlas rectangle of sprite `id` frame `sub` (into the entry's local
/// `local`) as D+0x94.
fn rect_d(fx: &mut Fx, local: u32, id: i32, sub: i32) {
    l2::rect(fx, local, id, sub);
    copy4(fx, D + 0x94, local);
}

/// The ring centre D+0x20 = the vec at `p` (xyz).
fn centre(fx: &mut Fx, p: u32) {
    copy3(fx, D + 0x20, p);
}

fn indexed(base: u32, i: i32) -> u32 {
    (base as i32 + 0x10 * i) as u32
}

/// The entry's draw of one live task (`count` = the live tasks so far,
/// the entry's s0, saved by rand's frame).
fn draw(fx: &mut Fx, sp: u32, t: u32, count: u32) {
    let s = fx.mem.read_u32(t + 0x24);
    let pos = sp + 0x70;
    match s {
        0x01E0_2590 => {
            let sub = fx.mem.i16(t + 0x34);
            rect_d(fx, sp + 0x80, 0x9B, sub);
            let w = fx.mem.read_u32(D + 0x2AC);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 0xF);
            fx.mem.set16(D + 0xBE, 0);
            let i = fx.mem.i16(t + 0xA);
            centre(fx, indexed(POINTS, i));
            quads4::ring_at_m(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        0x01E0_26E0 => {
            let i = fx.mem.i16(t + 0x30);
            let p = if i < 5 { indexed(POINTS, i) } else { indexed(D + 0x2A40, i) };
            copy3(fx, pos, p);
            quads4::band18(fx, D, t, 1, pos);
        }
        0x01E0_27D0 => {
            let h = fx.self_handle();
            fx.unit_position(h, pos);
            // 0x1F71A0's frame below the entry's (its saved ra, the
            // entry's s0 = the live count).
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_4338), (0, u64::from(count))]);
            let x = fx.mem.read_u32(pos);
            fx.mem.write_u32(D + 0x20, x);
            let y = ee::add(2.5, fx.mem.f(pos + 4));
            fx.mem.setf(D + 0x24, y);
            let z = fx.mem.read_u32(pos + 8);
            fx.mem.write_u32(D + 0x28, z);
            flash(fx, t, sp + 0x108);
        }
        0x01E0_2960 => {
            let w = fx.mem.read_u32(D + 0x2AC);
            fx.mem.write_u32(D + 0x88, w);
            rect_d(fx, sp + 0x90, 0x9A, 1);
            let i = fx.mem.i16(t + 0x30);
            copy3(fx, pos, indexed(POINTS, i));
            fx.mem.set16(D + 0xBE, 0);
            quads4::quarters_far(fx, D, t, sp + 0x108, pos);
        }
        0x01E0_2AB0 => {
            let w = fx.mem.read_u32(D + 0x27C);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            rect_d(fx, sp + 0xA0, 0x1E, sub);
            fx.mem.set16(D + 0xB8, 1);
            let h = fx.self_handle();
            fx.unit_position(h, pos);
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_4484), (0, u64::from(count))]);
            let y = plus_2_5(fx.mem.f(pos + 4));
            fx.mem.setf(pos + 4, y);
            fx.mem.set16(D + 0xBE, 0);
            ring18(fx, t, pos);
            fx.mem.set16(D + 0xB8, 0);
        }
        0x01E0_2C00 => {
            let sub = fx.mem.i16(t + 0x34);
            rect_d(fx, sp + 0xB0, 0x9B, sub);
            let w = fx.mem.read_u32(D + 0x2AC);
            fx.mem.write_u32(D + 0x88, w);
            let n = if fx.mem.i16(COUNTER) < 0xDC { 0xF } else { 8 };
            fx.mem.set16(D + 0xB8, n);
            fx.mem.set16(D + 0xBE, 0);
            let i = fx.mem.i16(t + 0xA);
            centre(fx, indexed(TRAIL, i));
            quads4::ring_at_m(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        0x01E0_2E60 | 0x01E0_3570 => {
            let i = fx.mem.i16(t + 0x30);
            copy3(fx, pos, indexed(TRAIL, i));
            quads4::band18(fx, D, t, 1, pos);
        }
        0x01E0_2F90 => {
            let i = fx.mem.i16(t + 0x30);
            copy3(fx, pos, indexed(POINTS, i));
            smoke_quads(fx, t, sp + 0x108, pos);
        }
        0x01E0_3160 | 0x01E0_3370 | 0x01E0_36E0 => {
            let (local, n, site) = match s {
                0x01E0_3160 => (0xC0, 3, 0x01E0_46D0),
                0x01E0_3370 => (0xD0, 3, 0x01E0_47A0),
                _ => (0xE0, 0xA, 0x01E0_48F0),
            };
            let sub = fx.mem.i16(t + 0x34);
            rect_d(fx, sp + local, 0x14, sub);
            let w = fx.mem.read_u32(D + 0x2B08);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, n);
            let v = fx.rand_from(count, site + 8);
            fx.mem.set16(D + 0xBE, v % 8);
            if s == 0x01E0_3370 {
                let h = fx.target_handle();
                let i = fx.mem.i16(t + 0xA);
                let n = fx.mem.i16((TNODES as i32 + 2 * i) as u32);
                crate::b076::node_position_saving(fx, h, n, pos, u64::from(count), Some(u64::from(D)), 0x01E0_47E0);
                for k in 0..3u32 {
                    let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                    fx.mem.setf(D + 0x20 + 4 * k, v);
                }
            } else {
                let i = fx.mem.i16(t + 0xA);
                centre(fx, indexed(TRAIL, i));
            }
            quads4::ring_at_m(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        _ => {}
    }
}

/// 0x1E04B50(D, task, desc, 0): the glow sprite (frame 0x140): an
/// additive sprite (UVs (1, 1, 62, 127), texture D+0x2B04) of size i16
/// +0x38 at D+0x20 facing the camera (scale 0.1), drawn twice (mirrored
/// in x the second time), each at the far Z and submitted twice at key 0.
fn flash(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x140);
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, (4u64 << 32) | 0x2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    fx.desc_alpha(d, ALPHA_ADD);
    crate::c061::rgbaq_at(fx, sp + 0x138, t + 0x28, d + 0x10);
    for (k, v) in [1u16, 1, 0x3E, 0x7F].into_iter().enumerate() {
        fx.mem.write_u16(sp + 0xD0 + 2 * k as u32, v);
    }
    uv::uv_rect(fx, d, sp + 0xD0, 0);
    let tex = fx.mem.read_u32(D + 0x2B04);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x120);
    copy3(fx, sp + 0xA0, D + 0x20);
    l2::xform_vp(fx, sp + 0xE0, sp + 0xB0, sp + 0x120, sp + 0xA0);
    for mirror in [false, true] {
        let size = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, size, size);
        fx.mem.set32(sp + 0x70, 0);
        fx.mem.set32(sp + 0x90, 0);
        if mirror {
            for o in [0x60u32, 0x80] {
                let v = ee::mul(fx.mem.f(sp + o), -1.0);
                fx.mem.setf(sp + o, v);
            }
        }
        if l2::project(fx, sp + 0x60, SPR, sp + 0xE0, d, sp + 0xC0) {
            l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, 0);
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, 0);
        }
    }
    fx.leave(0x140);
}

/// 0x1E04ED0(D, task, desc, pos): the smoke quad (frame 0x130): a strip3
/// of size i16 +0x38 at `pos` facing the camera (scale 0.1), UVs of
/// sprite 0x9B (0x14 for the 0x2F90 smoke) cell +0x34, texture D+0x2B00
/// (D+0x2AC otherwise); the smoke at the far Z keyed by its mean w,
/// otherwise drawn +0x30 times through 0x109870.
fn smoke_quads(fx: &mut Fx, t: u32, d: u32, pos: u32) {
    let sp = fx.enter(0x130);
    copy4(fx, sp + 0x50, pos);
    let ot = OT;
    let smoke = fx.mem.read_u32(t + 0x24) == 0x01E0_2F90;
    let sub = fx.mem.i16(t + 0x34);
    l2::rect(fx, sp + 0xC0, 0x9B, sub);
    if smoke {
        let sub = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xC0, 0x14, sub);
    }
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    l2::uv_strip3(fx, d, sp + 0xC0, 0);
    let at = if fx.mem.read_u32(t + 0x24) == 0x01E0_2F90 { D + 0x2B00 } else { D + 0x2AC };
    let tex = fx.mem.read_u32(at);
    fx.desc_texture(d, tex, 1);
    l2::camera_angles_to(fx, sp + 0x120);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
    }
    copy3(fx, sp + 0xA0, sp + 0x50);
    l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, sp + 0x120, sp + 0xA0);
    let size = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x60, size, size);
    if fx.mem.read_u32(t + 0x24) == 0x01E0_2F90 {
        if l2::project(fx, sp + 0x60, SPR, sp + 0xD0, d, sp + 0x110) {
            if fx.mem.read_u32(t + 0x24) == 0x01E0_2F90 {
                l2::far_z_desc(fx, d, 0);
            }
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x110)) as i32;
            fx.submit(d, ot, key);
        }
    } else {
        let mut k = 0i32;
        while k < fx.mem.i16(t + 0x30) {
            l2::project_submit(fx, sp + 0x60, d, sp + 0xD0, ot);
            k = (k + 1) as i16 as i32;
        }
    }
    fx.leave(0x130);
}

/// 0x1E05110(D, task, 0, pos): the halo ring (frame 0x200): 18 textured
/// strip3 quads (20 degrees each from i16 +0x3A) between radius D+0x2B3C
/// and that + 1 at `pos`, facing the camera (x + 0x4000, scale 1), each
/// quad's UVs sprite 0x1E cell i16 D+0x2B74 + 2k; colours +0x28 (inner
/// vertices 0, 1) / +0x2C (outer 2, 3); keyed by the mean w.
fn ring18(fx: &mut Fx, t: u32, pos: u32) {
    let sp = fx.enter(0x200);
    copy4(fx, sp + 0x90, pos);
    let d = sp + 0x170;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 1);
    l2::colour_strip3(fx, d, t + 0x2C, 2);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xF0 + 4 * k, 0x3F80_0000);
    }
    copy3(fx, sp + 0xE0, sp + 0x90);
    l2::camera_angles_to(fx, sp + 0x150);
    let x = fx.mem.i32(sp + 0x150).wrapping_add(0x4000);
    fx.mem.set32(sp + 0x150, x);
    l2::xform_vp(fx, sp + 0x100, sp + 0xF0, sp + 0x150, sp + 0xE0);
    for o in [0xA4u32, 0xB4, 0xC4, 0xD4] {
        fx.mem.set32(sp + o, 0);
    }
    let mut a = 0i32;
    for k in 0..0x12u32 {
        let sub = fx.mem.i16(SUBS + 2 * k);
        rect_d(fx, sp + 0x160, 0x1E, sub);
        let order = fx.mem.i16(D + 0xBE);
        l2::uv_strip3(fx, d, D + 0x94, order);
        let s = (fx.mem.i16(t + 0x3A) + a) as i16 as i32;
        let e = (a + (fx.mem.i16(t + 0x3A) + 0x14)) as i16 as i32;
        for (deg, inner, outer) in [(s, 0xC0u32, 0xA0u32), (e, 0xD0, 0xB0)] {
            let c = l2::sin_deg(fx, TAB, deg, 1);
            let v = ee::mul(fx.mem.f(RADIUS), c);
            fx.mem.setf(sp + inner, v);
            let sn = l2::sin_deg(fx, TAB, deg, 0);
            let v = ee::mul(fx.mem.f(RADIUS), sn);
            fx.mem.setf(sp + inner + 8, v);
            let c = l2::sin_deg(fx, TAB, deg, 1);
            let v = ee::mul(ee::add(1.0, fx.mem.f(RADIUS)), c);
            fx.mem.setf(sp + outer, v);
            let sn = l2::sin_deg(fx, TAB, deg, 0);
            let v = ee::mul(ee::add(1.0, fx.mem.f(RADIUS)), sn);
            fx.mem.setf(sp + outer + 8, v);
        }
        if l2::project(fx, sp + 0xA0, SPR, sp + 0x100, d, sp + 0x140) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x140)) as i32;
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, key);
        }
        a = (a + 0x14) as i16 as i32;
    }
    fx.leave(0x200);
}
