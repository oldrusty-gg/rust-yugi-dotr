//! BMODULE b572 (card 444), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E024D0, data area 0x1ED0000) [F]: an older task pool of 170 tasks of 0x40 bytes at D+0x2A0 (alive
//! +0x36, step +0x24 as a record address) drawn with the second library:
//! rings of camera-facing quads at the task (737e393f7a6a at 0x1E00920,
//! `quads4::ring_at_mt`) or at D+0x20 (383a1b86ead2 at 0x1E00EA0,
//! `quads4::ring_at_m`), UVs by uv_rect 0x1E00080. From update 137 to 311
//! dark smoke columns rise from the attacker's node 0x23 (their heights
//! and depths in two tables of 170 floats, D+0x2D94 / D+0x303C, drifting
//! to the target); on odd updates 69..309 white rings between nodes 0x22
//! and 0x25; from 291 to 364 dark puffs about the target's node 1; white
//! flashes at nodes 0xC (updates 13, 43) and 0x37 (30). Returns 1 on update
//! 180, 4 on 375, 2 on 355, 6 on 191..344, 8 once the 395-update timer
//! +0x2D70 has run out.
//!
//! The steps 0x1E02250 (a frame tint through 0x1F7E20), 0x1E023C0 (the
//! stage light 0x1F7F10) and 0x1E02480 / the entry's case for it (an
//! object op through 0x1F8380) are never spawned [F: the emitter's
//! `jal`s name 0x1E019E0 / 0x1E01830 / 0x1E01E90 / 0x1E020A0 only]; not
//! ported.
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xAA;
/// The step functions a task holds (+0x24), as the record's addresses.
const RING: u32 = 0x01E0_1830;
const SMOKE: u32 = 0x01E0_19E0;
const PUFF: u32 = 0x01E0_1E90;
const FLASH: u32 = 0x01E0_20A0;
const TIMER: u32 = D + 0x2D70;
const COLOUR: u32 = D + 0x2D74;
/// The smoke columns' heights / depths, one per slot +0xA.
const HEIGHTS: u32 = D + 0x2D94;
const DEPTHS: u32 = D + 0x303C;
const NEXT: u32 = D + 0x3304;
const SLOT: u32 = D + 0x330A;
const FACING: u32 = D + 0x330E;
const COUNTER: u32 = D + 0x3310;
const BYTES: u32 = D + 0x3312;
const BYTES2: u32 = D + 0x3412;

/// The entry 0x1E024D0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x2D7C, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3680);
    l2::sine_table(fx, D + 0x3514);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x330C, param);
    fx.mem.set32(TIMER, 0x18B);
    for (at, node, clear) in [(0x32F0u32, 0x23, 1), (0x32F2, 0x22, 0), (0x32F4, 0x25, 0), (0x32F8, 0xC, 0), (0x32FA, 0x37, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x32FE, r);
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x288 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x268, t);
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0x2D20 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for (table, site) in [(BYTES, 0x01E0_267C), (BYTES2, 0x01E0_26B8)] {
        for k in 0..0x100u32 {
            let v = fx.rand_from(k, site);
            fx.mem.set8(table + k, v % 16 - 8);
        }
    }
    for k in 0xB4..0x100u32 {
        fx.mem.write_u8(BYTES2 + k, 0);
    }
    for k in 0..8u32 {
        let v = fx.rand_from(k, 0x01E0_2714);
        fx.mem.set16(D + 0xC0 + 2 * k, v % 4);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let s = D + 0x40 * k;
        let t = s + 0x2A0;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == RING {
            ring(fx, t, 0x01E0_2784, false);
        }
        if fx.mem.read_u32(t + 0x24) == SMOKE {
            smoke(fx, t, s);
        }
        if fx.mem.read_u32(t + 0x24) == PUFF {
            puff(fx, t);
        }
        if fx.mem.read_u32(t + 0x24) == FLASH {
            ring(fx, t, 0x01E0_27E8, true);
        }
        // 0x1E02250 / 0x1E023C0 / 0x1E02480: never spawned (see above).
        debug_assert!(!matches!(fx.mem.read_u32(t + 0x24), 0x01E0_2250 | 0x01E0_23C0 | 0x01E0_2480));
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    // 0x1F71A0's frame below the entry's (its saved ra, the entry's s0).
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_289C), (0, 0)]);
    // The entry's s0: the live tasks counted in the draw loop (rand's
    // frame).
    let mut s0 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        s0 += 1;
        let f = fx.mem.read_u32(t + 0x24);
        if f == RING {
            cell(fx, t, D + 0x2D24, sp + 0x60);
            fx.mem.set16(D + 0xB8, 3);
            rand_orient(fx, s0, 0x01E0_2908);
            quads4::ring_at_mt(fx, uv::uv_rect, D, t, 0x10, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x24) == SMOKE {
            cell(fx, t, D + 0x260, sp + 0x70);
            strip_uv(fx, t);
            if fx.mem.i16(t + 0x32) >= 2 {
                fx.mem.set16(D + 0xB8, 3);
                rand_orient(fx, s0, 0x01E0_29D8);
                let x = ee::from_i32(fx.mem.i16(t));
                fx.mem.setf(D + 0x20, x);
                let j = fx.mem.i16(t + 0xA) as u32;
                let w = fx.mem.read_u32(HEIGHTS.wrapping_add(4 * j));
                fx.mem.write_u32(D + 0x24, w);
                let j = fx.mem.i16(t + 0xA) as u32;
                let w = fx.mem.read_u32(DEPTHS.wrapping_add(4 * j));
                fx.mem.write_u32(D + 0x28, w);
                let mode = if fx.mem.i16(COUNTER) < 0xAA { 0x21 } else { 0x20 };
                quads4::ring_at_m(fx, uv::uv_rect, D, t, mode, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
        }
        if fx.mem.read_u32(t + 0x24) == PUFF {
            cell(fx, t, D + 0x260, sp + 0x80);
            strip_uv(fx, t);
            fx.mem.set16(D + 0xB8, 3);
            rand_orient(fx, s0, 0x01E0_2B08);
            quads4::ring_at_mt(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x24) == FLASH {
            cell(fx, t, D + 0x2D24, sp + 0x90);
            fx.mem.set16(D + 0xB8, 8);
            rand_orient(fx, s0, 0x01E0_2B98);
            quads4::ring_at_mt(fx, uv::uv_rect, D, t, 0x20, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xB4 => 1,
        0x177 => 4,
        0x163 => 2,
        0xBF..=0x158 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The texture `tex` and the atlas cell 0x14 / +0x34 (0x106960 into `r`)
/// copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, tex: u32, r: u32) {
    let w = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, w);
    let sub = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// The smoke strip: texture (5, 2), UV (32 frame + 1, 1) size (31, 63).
fn strip_uv(fx: &mut Fx, t: u32) {
    let w = fx.mem.read_u32(D + 0x2D28);
    fx.mem.write_u32(D + 0x88, w);
    let f = fx.mem.i16(t + 0x34);
    fx.mem.set16(D + 0x94, (f << 5) + 1);
    fx.mem.set16(D + 0x96, 1);
    fx.mem.set16(D + 0x98, 0x1F);
    fx.mem.set16(D + 0x9A, 0x3F);
}

/// D+0xBE = rand % 4 from the entry's `jal` at `site`.
fn rand_orient(fx: &mut Fx, s0: u32, site: u32) {
    let v = fx.rand_from(s0, site + 8);
    fx.mem.set16(D + 0xBE, v % 4);
}

/// rand % m (HI) from a step's `jal` at `site` (rand's frame: s0 = `t`).
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// 0x1E01400(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x50: ra, the entry's s1 = D / s0 = 0).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_2750), (0x10, u64::from(D)), (0, 0)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    // A smoke column at node 0x23, 1.6 up and 0.6 towards the facing.
    if (0x89..0x138).contains(&c) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0x32F0);
        fx.node_position(h, slot, sp + 0x30);
        let x = fx.mem.read_u32(sp + 0x30);
        fx.mem.write_u32(D + 0x250, x);
        let y = ee::add(f32::from_bits(0x3FCC_CCCD), fx.mem.f(sp + 0x34));
        fx.mem.setf(D + 0x254, y);
        let z0 = fx.mem.f(sp + 0x38);
        let dz = f32::from_bits(0x3F19_999A);
        let z = if fx.mem.i16(FACING) != 0 { ee::add(dz, z0) } else { ee::sub(z0, dz) };
        fx.mem.setf(D + 0x258, z);
        let s = fx.mem.i16(NEXT);
        fx.mem.set16(SLOT, s);
        spawn(fx, SMOKE);
        let s = fx.mem.i16(NEXT) + 1;
        fx.mem.set16(NEXT, s);
        if fx.mem.i16(NEXT) >= 0xAA {
            fx.mem.set16(NEXT, 0);
        }
    }
    let c = fx.mem.i16(COUNTER);
    // A ring half way between nodes 0x22 and 0x25 (odd updates).
    if (0x45..0x136).contains(&c) && c % 2 == 1 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0x32F2);
        fx.node_position(h, slot, sp + 0x30);
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0x32F4);
        fx.node_position(h, slot, sp + 0x40);
        for k in 0..3 {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x40 + 4 * k), fx.mem.f(sp + 0x30 + 4 * k)), 2.0);
            fx.mem.setf(sp + 0x40 + 4 * k, v);
        }
        for k in 0..3 {
            let v = ee::add(fx.mem.f(sp + 0x30 + 4 * k), fx.mem.f(sp + 0x40 + 4 * k));
            fx.mem.setf(D + 0x250 + 4 * k, v);
        }
        let c = fx.mem.i16(COUNTER);
        let s = if fx.div(c, 5).1 == 1 { 1 } else { 2 };
        fx.mem.set16(SLOT, s);
        spawn(fx, RING);
    }
    let c = fx.mem.i16(COUNTER);
    // A puff about the target's node 1.
    if (0x123..0x16D).contains(&c) {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x30);
        let h = fx.target_handle();
        let slot = fx.mem.i16(D + 0x32FE);
        fx.node_position(h, slot, sp + 0x30);
        let r = rmod(fx, D, 0x01E0_16B0, 0x28);
        let x = ee::sub(ee::add(fx.mem.f(sp + 0x30), ee::from_i32(r)), 20.0);
        fx.mem.setf(D + 0x250, x);
        let r = rmod(fx, D, 0x01E0_16E4, 0xA);
        let y = ee::sub(fx.mem.f(sp + 0x34), ee::from_i32(r));
        fx.mem.setf(D + 0x254, y);
        let r = rmod(fx, D, 0x01E0_1714, 0x28);
        let z = ee::sub(ee::add(fx.mem.f(sp + 0x38), ee::from_i32(r)), 20.0);
        fx.mem.setf(D + 0x258, z);
        spawn(fx, PUFF);
    }
    // White flashes at nodes 0xC (updates 13, 43) and 0x37 (30).
    for (at, on) in [(0x32F8u32, [0xD, 0x2B]), (0x32FA, [0x1E, 0x1E])] {
        let c = fx.mem.i16(COUNTER);
        if on.contains(&c) {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + at);
            fx.node_position(h, slot, sp + 0x30);
            for k in 0..3 {
                let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
                fx.mem.write_u32(D + 0x250 + 4 * k, w);
            }
            spawn(fx, FLASH);
        }
    }
    fx.leave(0x50);
}

/// 0x1E02C90(D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour D+0x2D74 (twice), +0x30 = D+0x330A; 1
/// when one was free.
fn spawn(fx: &mut Fx, step: u32) -> i32 {
    fx.enter(0x60);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2D6 + 0x40 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            let a = fx.mem.i16(SLOT);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E01830 (size 20, growth 1, widening 5..9) / 0x1E020A0 (`flash`: 30,
/// 2, 8..12)
/// (D, task): a white ring (frame 0x20): the radius +0x30 kept in +0xA
/// and growing from 0 by `wid`, row 0..24, life 4..13, a random angle;
/// fades in by 0x60 under 0xC0, out by 19; the cell frame counting to 5
/// on odd ages. `ra`: the entry's return address.
fn ring(fx: &mut Fx, t: u32, ra: u64, flash: bool) {
    let (s, wid) = if flash { (FLASH, 8) } else { (RING, 5) };
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, ra), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, if flash { 0x1E } else { 0x14 });
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0xFF, 0xFF, 0xFF);
        let r = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, r);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, s + 0x44, 5);
        fx.mem.set16(t + 0xE, v + wid);
        let v = rmod(fx, t, s + 0x64, 0x19);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, s + 0x80, 0xA);
        fx.mem.set16(t + 6, v + 4);
        let v = rmod(fx, t, s + 0xA0, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0xC, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x60));
        }
    } else if a >= 0x15 {
        fx.mem.write_u8(t + 0x2B, a - 0x13);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 5 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, s + if flash { 2 } else { 1 });
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
}

/// 0x1E019E0 (D, task): a smoke column (frame 0x70: the entry's s2 = the
/// slot `s`, s1 = D, s0 = 0): size 10..15 growing by 2 (under 60) and
/// 2..5 on odd ages, black, its slot +0xA (= +0x30) in the height / depth
/// tables (the task's y - 0.7, z), angle +0x3A random turning by 1, row
/// 0..34, life 37..61, +0xC the x distance of nodes 0x22 - 0x23 (as an
/// integer), +0x10 a random 0..3; fades in by 0x40 (while its age is under
/// 314, as the original compares), out by 7 before update 314, by 9 after
/// 314; the height sinks by 1.8 above 35 and the depth drifts by the
/// target's distance / 83 (/ 70 from update 300); dark grey from 297.
fn smoke(fx: &mut Fx, t: u32, s: u32) {
    let sp = fx.enter(0x70);
    fx.saved(sp, &[(0x30, 0x01E0_27A8), (0x20, u64::from(s)), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        let v = rmod(fx, t, 0x01E0_1A08, 6);
        fx.mem.set16(t + 0x38, v + 0xA);
        let v = fx.rand_from(t, 0x01E0_1A30);
        fx.mem.set16(t + 0x34, (v >> 4) % 2);
        l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x10);
        let j = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, j);
        let j = fx.mem.i16(t + 0xA) as u32;
        let y = ee::from_i32(fx.mem.i16(t + 2));
        fx.mem.setf(HEIGHTS.wrapping_add(4 * j), y);
        let j = fx.mem.i16(t + 0xA) as u32;
        let z = ee::from_i32(fx.mem.i16(t + 4));
        fx.mem.setf(DEPTHS.wrapping_add(4 * j), z);
        let j = fx.mem.i16(t + 0xA) as u32;
        let p = HEIGHTS.wrapping_add(4 * j);
        let y = ee::sub(fx.mem.f(p), f32::from_bits(0x3F33_3333));
        fx.mem.setf(p, y);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, 0x01E0_1AC4, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, 0x01E0_1AE0, 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rmod(fx, t, 0x01E0_1B00, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1B1C, 0x19);
        fx.mem.set16(t + 6, v + 0x25);
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0x32F0);
        fx.node_position(h, slot, sp + 0x40);
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0x32F2);
        fx.node_position(h, slot, sp + 0x50);
        let dx = ee::sub(fx.mem.f(sp + 0x50), fx.mem.f(sp + 0x40));
        fx.mem.set16(t + 0xC, ee::fixsfsi(dx));
        let v = fx.rand_from(t, 0x01E0_1B88);
        fx.mem.set32(t + 0x10, (v >> 4) % 4);
        fx.mem.set16(t + 0x36, 2);
    }
    let age = fx.mem.i16(t + 0x32);
    if age < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x2B);
        if age < 0x13A && a < 0x81 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        }
    } else if fx.mem.i16(COUNTER) < 0x13A {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 9 {
            fx.mem.write_u8(t + 0x2B, a - 7);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(COUNTER) >= 0x13B {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x2B, a - 9);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38);
        if s < 0x3C {
            fx.mem.set16(t + 0x38, s + 2);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x40);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x50);
    if fx.mem.i16(t + 0x32) != 0 {
        let dz = ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(sp + 0x58));
        fx.mem.setf(sp + 0x48, dz);
        let q = if fx.mem.i16(COUNTER) < 0x12C { 83.0 } else { 70.0 };
        let dz = ee::div(fx.mem.f(sp + 0x48), q);
        fx.mem.setf(sp + 0x48, dz);
        let j = fx.mem.i16(t + 0xA) as u32;
        let p = DEPTHS.wrapping_add(4 * j);
        let z = ee::add(fx.mem.f(p), fx.mem.f(sp + 0x48));
        fx.mem.setf(p, z);
    }
    let j = fx.mem.i16(t + 0xA) as u32;
    let p = HEIGHTS.wrapping_add(4 * j);
    let y = fx.mem.f(p);
    if !(y <= 35.0) {
        fx.mem.setf(p, ee::sub(y, f32::from_bits(0x3FE6_6666)));
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 6 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    if fx.mem.i16(COUNTER) >= 0x129 {
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x20);
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let v = fx.rand_from(t, 0x01E0_1DFC);
        let s = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, s + (v % 4 + 2) as i16 as i32);
    }
    let a = fx.mem.i16(t + 0x3A);
    let turn = if fx.mem.i16(t + 0xE) % 2 == 1 { 1 } else { -1 };
    fx.mem.set16(t + 0x3A, a + turn);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x70);
}

/// 0x1E01E90 (D, task): a black puff (frame 0x30: the entry's s1 = D, s0
/// = 0): size 70 growing by 1, radius 20..169 growing by 5..9, row 0..24,
/// life 4..10, a random angle; fades in by 0x40 under 0x80, out by 9; dark
/// grey from update 297; the cell frame counting to 5 on odd ages; rises
/// by 1 a update.
fn puff(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_27C8), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x46);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x10);
        let r = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, r);
        let v = rmod(fx, t, 0x01E0_1ED8, 0x96);
        fx.mem.set16(t + 0x30, v + 0x14);
        let v = rmod(fx, t, 0x01E0_1EF8, 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rmod(fx, t, 0x01E0_1F18, 0x19);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1F34, 7);
        fx.mem.set16(t + 6, v + 4);
        let v = rmod(fx, t, 0x01E0_1F54, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0xC, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(COUNTER) >= 0x129 {
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x20);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 5 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, s + 1);
    }
    let y = fx.mem.i16(t + 2) + 1;
    fx.mem.set16(t + 2, y);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x30);
}
