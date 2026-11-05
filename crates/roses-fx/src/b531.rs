//! BMODULE b531 (card 92), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E059F0, data area 0x3420 bytes) [F] (helper E17,
//! 4 Nov 2026) from E16's map: a pool of 150 tasks of 0x40 bytes at
//! D+0x2B0 (alive +0x36, age +0x32, step +0x24 compared by address)
//! spawned by 0x1E06880 (b086's: colours D+0x3154, the point D+0x250 as
//! i16, +0x30 = D+0x3194, +0x3A = D+0x318E), drawn with the second
//! library: `quads4::band18_turned` (0x1E010F0), `quads4::ring_spun`
//! (0x1E01500, the bytes D+0x31AE), `spline::tube` (0x1E01E20 along the
//! 10-point trails D+0x29A0 / D+0x2D60 + 0xA0 k, the first kept by
//! `spawn::history` 0x1E00080), `billboard::sprite_square` (0x1E02640);
//! the record's own 0x1E00DE0 (b850's nine slivers on its own descriptor
//! at a position) and 0x1E01970 (`ring_spun` with the last two vertices
//! pinned to the x axis). The emitter 0x1E02830: the stage light 0x5760
//! and two sprites 0x47A0 / 0x49C0 on update 1; on 0x37 six orbs
//! (0x3CF0 bands + 0x4C60 sprites) at the attacker's node D+0x3186, six
//! tubes 0x3EE0 / six 0x4460; they drift to 0x9B, then (from 0xBD, one
//! more every few updates, D+0x319A) chase the target's z with a
//! +-30 sine sway; the point D+0x2860 between nodes D+0x317C / 0x3180
//! (from 0xF5 moving to the target's z, 0x4F50 rings); 0x3A30 slivers on
//! 0x39..0xE6 (D+0x3198 growing), 0x5220 / 0x3C00 bands by the target
//! distance, 0x5380 rings at the hands and orbs, 0x54D0 bands on 0xBB /
//! 0xCD, a white window 0x5830 on 0xF2 and two full-window flashes 0x5050
//! on 0xE8 / 0x12C. Returns 1 / 4 / 2 / 6 on 0xC3 / 0x154 / 0x140 /
//! 0xCE..0x13A, 8 once the timer D+0x3150 (0x17C) is out.
use crate::blib::{billboard, quads4, spawn as bspawn, spline, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0x96;
const POOL: u32 = D + 0x2B0;
const POS: u32 = D + 0x250;
const POINT: u32 = D + 0x2860;
const ORBS: u32 = D + 0x2870;
const SWAY: u32 = D + 0x2900;
const TUBE_A: u32 = D + 0x29A0;
const HIST: u32 = D + 0x2A30;
const TUBE_B: u32 = D + 0x2D60;
const ANCHOR: u32 = D + 0x3120;
const TEX5: u32 = D + 0x3130;
const TIMER: u32 = D + 0x3150;
const COLOUR: u32 = D + 0x3154;
const OT_AT: u32 = D + 0x315C;
const NODES: u32 = D + 0x317C;
const TURN: u32 = D + 0x318E;
const INDEX: u32 = D + 0x3194;
const SIZE: u32 = D + 0x3198;
const WAVES: u32 = D + 0x319A;
const ANGLES: u32 = D + 0x319C;
const LANDED: u32 = D + 0x31AA;
const COUNTER: u32 = D + 0x31AC;
const BYTES: u32 = D + 0x31AE;
const SINES: u32 = D + 0x32B0;

const SLIVERS: u32 = 0x01E0_3A30;
const BAND_A: u32 = 0x01E0_3C00;
const ORB: u32 = 0x01E0_3CF0;
const TUBE1: u32 = 0x01E0_3EE0;
const RING_A: u32 = 0x01E0_4300;
const TUBE2: u32 = 0x01E0_4460;
const SPRITE_A: u32 = 0x01E0_47A0;
const SPRITE_B: u32 = 0x01E0_49C0;
const ORB_SPRITE: u32 = 0x01E0_4C60;
const RING_B: u32 = 0x01E0_4DB0;
const BAND_B: u32 = 0x01E0_4F50;
const FLASH: u32 = 0x01E0_5050;
const BAND_C: u32 = 0x01E0_5220;
const RING_C: u32 = 0x01E0_5380;
const BAND_D: u32 = 0x01E0_54D0;
const BAND_E: u32 = 0x01E0_5630;
const LIGHT: u32 = 0x01E0_5760;
const WINDOW: u32 = 0x01E0_5830;

/// The steps in the entry's test order (its `jal` to step i returns to
/// 0x1E05C58 + 0x20 i).
const STEPS: [u32; 18] = [SLIVERS, BAND_A, ORB, TUBE1, RING_A, TUBE2, SPRITE_A, SPRITE_B, ORB_SPRITE, RING_B, BAND_B, FLASH, BAND_C, RING_C, BAND_D, BAND_E, LIGHT, WINDOW];

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn timer(fx: &mut Fx) -> i32 {
    fx.mem.i32(TIMER)
}

/// rand % m through `div` (HI / LO set).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// rand % 2^n through `andi` (the C remainder of a signed value; no div).
fn rand_and(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    fx.rand_from(s0, ra).wrapping_rem(m)
}

fn sqrtf_from(fx: &mut Fx, x: f32, ra: u32) -> f32 {
    let a = fx.sp.wrapping_sub(0x10);
    fx.mem.write_u64(a, u64::from(ra));
    fx.sqrtf(x)
}

/// |`z` - (the float at `at`)| as the original takes it: sqrtf(d d).
fn gap(fx: &mut Fx, z: f32, at: u32, ra: u32) -> f32 {
    let d = ee::sub(z, fx.mem.f(at));
    sqrtf_from(fx, ee::mul(d, d), ra)
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

fn add16(fx: &mut Fx, a: u32, d: i32) {
    let v = fx.mem.i16(a).wrapping_add(d);
    fx.mem.set16(a, v);
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x36) == 1
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    fx.mem.i16(t + 0x32)
}

fn grow_age(fx: &mut Fx, t: u32) {
    add16(fx, t + 0x32, 1);
}

fn kill(fx: &mut Fx, t: u32) {
    fx.mem.set16(t + 0x36, 0);
}

fn odd(n: i32) -> bool {
    n.wrapping_rem(2) == 1
}

/// The byte at `at` up by `up` while below `cap`.
fn rise(fx: &mut Fx, at: u32, cap: u8, up: u8) {
    let v = fx.mem.read_u8(at);
    if v < cap {
        fx.mem.write_u8(at, v.wrapping_add(up));
    }
}

/// The byte at `t + at` down by `down` while at least `floor`, else the
/// task ends.
fn fall(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8) {
    let v = fx.mem.read_u8(t + at);
    if v >= floor {
        fx.mem.write_u8(t + at, v.wrapping_sub(down));
    } else {
        kill(fx, t);
    }
}

/// The byte at `at` down by `down` while at least `floor` (no end).
fn fall_only(fx: &mut Fx, at: u32, floor: u8, down: u8) {
    let v = fx.mem.read_u8(at);
    if v >= floor {
        fx.mem.write_u8(at, v.wrapping_sub(down));
    }
}

/// D+0x250 = the point D+0x2860 raised by 5.
fn pos_at_point(fx: &mut Fx) {
    let x = fx.mem.f(POINT);
    fx.mem.setf(POS, x);
    let y = ee::add(5.0, fx.mem.f(POINT + 4));
    fx.mem.setf(POS + 4, y);
    let z = fx.mem.f(POINT + 8);
    fx.mem.setf(POS + 8, z);
}

/// `out` = the point raised by 5 (the draws' position).
fn raised(fx: &mut Fx, out: u32) {
    let x = fx.mem.f(POINT);
    fx.mem.setf(out, x);
    let y = ee::add(5.0, fx.mem.f(POINT + 4));
    fx.mem.setf(out + 4, y);
    let z = fx.mem.f(POINT + 8);
    fx.mem.setf(out + 8, z);
}

/// The task's i16 point as floats at `out` (xyz).
fn point_f(fx: &mut Fx, t: u32, out: u32) {
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(out + 4 * k, v);
    }
}

/// Orb `i`'s position plus its sway into `out`.
fn orb_at(fx: &mut Fx, t: u32, out: u32) {
    for k in 0..3u32 {
        let i = fx.mem.i16(t + 0x30) as u32;
        let a = fx.mem.f(ORBS.wrapping_add(0x10 * i) + 4 * k);
        let b = fx.mem.f(SWAY.wrapping_add(0x10 * i) + 4 * k);
        fx.mem.setf(out + 4 * k, ee::add(a, b));
    }
}

/// The entry 0x1E059F0(D, param) (frame 0x180).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x180);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x180);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3420);
    let p = param as i64 as u64;
    l2::sine_table_saving(fx, SINES, u64::from(D), 0, 0x01E0_5A40);
    l2::sine_table_saving(fx, D + 0xD8, u64::from(D), 0, 0x01E0_5A48);
    fx.mem.set16(D + 0x31A8, param);
    fx.mem.set32(TIMER, 0x17C);
    let s = [Some(u64::from(D)), Some(0), Some(p)];
    for (k, n) in [0x1Bi32, 0x17, 0x25, 0x21, 0x5C, 0x60, 0x66, 0x6A].into_iter().enumerate() {
        let h = fx.self_handle();
        let clear = i32::from(k == 0);
        let r = crate::b076::request_node_saving(fx, h, n, clear, s, 0x01E0_5A68 + 0x18 * k as u32);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0xEu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x2A8 + 4 * (k - 0xE), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x278, t);
    for k in 0..4u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX5 + 4 * k, t);
    }
    for k in 0x64..0xB4u32 {
        let r = rmod(fx, D, 0x01E0_5B80, 0xC);
        fx.mem.write_u8(BYTES + k, (r - 4) as u8);
    }
    for k in 0..0x64u32 {
        let r = rmod(fx, D, 0x01E0_5BB4, 0x14);
        fx.mem.write_u8(BYTES + k, (r - 0xA) as u8);
    }
    for k in 0xB4..0x100u32 {
        let r = rmod(fx, D, 0x01E0_5BE8, 6);
        fx.mem.write_u8(BYTES + k, (r - 3) as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        for (i, &f) in STEPS.iter().enumerate() {
            if fx.mem.read_u32(t + 0x24) == f {
                run_step(fx, f, t, k, 0x01E0_5C58 + 0x20 * i as u32);
            }
        }
    }
    let pos = sp + 0x60;
    let mut count = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        count += 1;
        draw(fx, sp, t, pos, count);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0xC3 => 1,
        0x154 => 4,
        0x140 => 2,
        0xCE..=0x13A => 6,
        _ if timer(fx) > 0 => 0,
        _ => 8,
    }
}

/// The texture at `tex` and the atlas cell (`id`, +0x34) (0x106960 into
/// the entry's local `r`) copied to D+0x88 / D+0x94..+0xA3.
fn cell(fx: &mut Fx, tex: u32, r: u32, id: i32, sub: i32) {
    let v = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, v);
    l2::rect(fx, r, id, sub);
    copy4(fx, D + 0x94, r);
}

/// The tubes' UV rectangle (1, 0x10, 0x1F, 1) and texture D+0x3130.
fn tube_cell(fx: &mut Fx) {
    let v = fx.mem.read_u32(TEX5);
    fx.mem.write_u32(D + 0x88, v);
    for (o, v) in [(0x94u32, 1), (0x96, 0x10), (0x98, 0x1F), (0x9A, 1)] {
        fx.mem.set16(D + o, v);
    }
}

/// D+0x20 = the task's i16 point as floats.
fn centre_at_task(fx: &mut Fx, t: u32) {
    point_f(fx, t, D + 0x20);
}

/// The entry's draw case for a live task (`count` the live tasks so far,
/// the entry's s1).
fn draw(fx: &mut Fx, sp: u32, t: u32, pos: u32, count: u32) {
    let f = fx.mem.read_u32(t + 0x24);
    let c = fx.mem.i16(t + 0x34);
    match f {
        SLIVERS => {
            cell(fx, D + 0x2AC, sp + 0x70, 0x9B, c);
            fx.mem.set16(D + 0xB8, 1);
            fx.mem.set16(D + 0xBE, 0);
            raised(fx, pos);
            slivers(fx, t, pos);
            fx.mem.set16(D + 0xB8, 0);
        }
        BAND_A => {
            raised(fx, pos);
            quads4::band18_turned(fx, D, t, 2, pos);
        }
        ORB => {
            orb_at(fx, t, pos);
            quads4::band18_turned(fx, D, t, 0, pos);
        }
        ORB_SPRITE => {
            cell(fx, TEX5, sp + 0x80, 0x14, c);
            orb_at(fx, t, pos);
            billboard::sprite_square(fx, 0x01E0_2640, uv::uv_rect, D, t, 0, pos);
        }
        TUBE1 => {
            tube_cell(fx);
            for o in [0x20u32, 0x24, 0x28] {
                fx.mem.write_u32(D + o, 0);
            }
            let i = fx.mem.i16(t + 0x30) as u32;
            spline::tube(fx, 0x01E0_1E20, D, t, TUBE_A.wrapping_add(0xA0 * i), 2);
        }
        RING_A => {
            cell(fx, TEX5, sp + 0x90, 0x14, c);
            fx.mem.set16(D + 0xB8, 0x14);
            let r = rand_and(fx, D, 0x01E0_6198, 8);
            fx.mem.set16(D + 0xBE, r);
            centre_at_task(fx, t);
            quads4::ring_spun(fx, uv::uv_rect, D, t, 1, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        TUBE2 => {
            tube_cell(fx);
            fx.mem.set16(D + 0xBE, 0);
            raised(fx, D + 0x20);
            let i = fx.mem.i16(t + 0x30) as u32;
            spline::tube(fx, 0x01E0_1E20, D, t, TUBE_B.wrapping_add(0xA0 * i), 2);
        }
        SPRITE_A => {
            cell(fx, D + 0x313C, sp + 0xA0, 0x14, c);
            let s = fx.mem.i16(SIZE) >> 1;
            fx.mem.set16(t + 0x38, s);
            raised(fx, pos);
            billboard::sprite_square(fx, 0x01E0_2640, uv::uv_rect, D, t, 0, pos);
        }
        SPRITE_B => {
            cell(fx, D + 0x313C, sp + 0xB0, 0x14, c);
            raised(fx, pos);
            cell(fx, D + 0x3138, sp + 0xC0, 0x14, 3);
            let keep = fx.mem.read_u32(t + 0x18);
            let v = fx.mem.i16(SIZE);
            let hi = ((i64::from(v) * 0x5555_5556) >> 32) as i32;
            fx.mult(0x5555_5556, v);
            let q = hi + ((v as u32) >> 31) as i32;
            fx.mem.set16(t + 0x38, q);
            billboard::sprite_square(fx, 0x01E0_2640, uv::uv_rect, D, t, 0, pos);
            let n = fx.mem.read_u32(t + 0x18).wrapping_neg();
            fx.mem.write_u32(t + 0x18, n);
            let v = fx.mem.read_u32(D + 0x3134);
            fx.mem.write_u32(D + 0x88, v);
            billboard::sprite_square(fx, 0x01E0_2640, uv::uv_rect, D, t, 0, pos);
            fx.mem.write_u32(t + 0x18, keep);
        }
        RING_B => {
            cell(fx, D + 0x2AC, sp + 0xD0, 0x9A, c);
            fx.mem.set16(D + 0xB8, 0x14);
            fx.mem.set16(D + 0xBE, 0);
            centre_at_task(fx, t);
            ring_pinned(fx, t, 1, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        BAND_B | BAND_C | BAND_E => {
            point_f(fx, t, pos);
            quads4::band18_turned(fx, D, t, 1, pos);
        }
        BAND_D => {
            point_f(fx, t, pos);
            quads4::band18_turned(fx, D, t, 0, pos);
        }
        FLASH => {
            let d = sp + 0xF0;
            l2::colour_strip4(fx, d, t + 0x28, 1);
            l2::colour_strip4(fx, d, t + 0x2C, 2);
            l2::desc_gouraud4(fx, d, 0);
            // 0x109BC0's frame (0x20: ra, s0 = d) at the entry's level [run:
            // a later 0x106960 copies its +0x18 (ra's upper half, 0) into
            // D+0x9C].
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_65B4), (0, u64::from(d))]);
            l2::fullscreen4(fx, d);
            fx.submit(d, OT, 0);
            let _ = count;
        }
        RING_C => {
            let tex = if fx.mem.i16(t + 0x34) == 1 { TEX5 } else { D + 0x3138 };
            cell(fx, tex, sp + 0xE0, 0x14, c);
            fx.mem.set16(D + 0xB8, 2);
            fx.mem.set16(D + 0xBE, 0);
            centre_at_task(fx, t);
            quads4::ring_spun(fx, uv::uv_rect, D, t, 1, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        _ => {}
    }
}

// ------------------------------------------------------------ record draws

/// 0x1E00DE0(D, task, pos) (frame 0x1D0): b850's nine slivers (2 degrees
/// every 40 from i16 +0x3A, radius i16 +0x38 to +0x38 + 20, colour +0x28)
/// on its own descriptor sp + 0x140 at `pos` (copied to sp + 0x80), turned
/// by the task's angles +0x10 (scale 1); UVs D+0x94 in D+0xBE's
/// orientation, texture D+0x88; each visible one submitted D+0xB8 + 1
/// times at its mean w on D+0x80.
fn slivers(fx: &mut Fx, t: u32, pos: u32) {
    let sp = fx.enter(0x1D0);
    copy4(fx, sp + 0x80, pos);
    let d = sp + 0x140;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    fx.mem.write_u32(sp + 0xE0, 0x3F80_0000);
    fx.mem.write_u32(sp + 0xE4, 0x3F80_0000);
    copy3(fx, sp + 0xD0, sp + 0x80);
    fx.mem.write_u32(sp + 0xE8, 0x3F80_0000);
    l2::xform_vp(fx, sp + 0xF0, sp + 0xE0, t + 0x10, sp + 0xD0);
    for o in [0x94u32, 0xA4, 0xB4, 0xC4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let tab = D + 0xD8;
    let mut s4 = 0i32;
    for _ in 0..9 {
        let d1 = (fx.mem.i16(t + 0x3A) + s4) as i16 as i32;
        let d2 = (s4 + (fx.mem.i16(t + 0x3A) + 2)) as i16 as i32;
        for (deg, inner, outer) in [(d1, 0xB0u32, 0x90u32), (d2, 0xC0, 0xA0)] {
            for (o, wide) in [(inner, false), (outer, true)] {
                let c = l2::sin_deg(fx, tab, deg, 1);
                let r = fx.mem.i16(t + 0x38) + if wide { 0x14 } else { 0 };
                fx.mem.setf(sp + o, ee::mul(ee::from_i32(r), c));
                let s = l2::sin_deg(fx, tab, deg, 0);
                let r = fx.mem.i16(t + 0x38) + if wide { 0x14 } else { 0 };
                fx.mem.setf(sp + o + 8, ee::mul(ee::from_i32(r), s));
            }
        }
        if l2::project(fx, sp + 0x90, SPR, sp + 0xF0, d, sp + 0x130) {
            let mut k = 0;
            while k < fx.mem.i16(D + 0xB8) + 1 {
                let w = fx.mem.f(sp + 0x130);
                fx.fix_residue(w, 0x01E0_1078);
                let key = ee::fixunssfsi(w) as i32;
                let ot = fx.mem.read_u32(D + 0x80);
                fx.submit(d, ot, key);
                k += 1;
            }
        }
        s4 = (s4 + 0x28) as i16 as i32;
    }
    fx.leave(0x1D0);
}

/// 0x1E01970(D, task, mode, table) (frame 0x200): `quads4::ring_spun`
/// with each quad's last two vertices set to (-(i16 +0x38 >> 1), 0, 0) /
/// (+0x38 >> 1, 0, 0) after the push, before the turn [F fdiff vs
/// 0x1E01500].
fn ring_pinned(fx: &mut Fx, t: u32, mode: i32, table: u32) {
    use crate::c061::{blend_alpha, desc_uv4, rgbaq_at, scale_tenth};
    let sp = fx.enter(0x200);
    let d = sp + 0x168;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha(2));
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, sp + 0x1F8, t + 0x28, d + 0x10);
    let order = fx.mem.i16(D + 0xBE);
    uv::uv_rect(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    let count = match fx.mem.i16(D + 0xB8) {
        0 => 0x14,
        n => n,
    };
    scale_tenth(fx, sp + 0xF0);
    l2::camera_angles_to(fx, sp + 0x150);
    copy3(fx, sp + 0xE0, D + 0x20);
    l2::xform_vp(fx, sp + 0x110, sp + 0xF0, sp + 0x150, sp + 0xE0);
    let tab = D + 0xD8;
    let mut k = 0i32;
    while k < count {
        let size = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0xA0, size, size);
        let row = table.wrapping_add(k as u32);
        let half = k >> 1;
        for (axis, off) in [(0, 0u32), (1, 0x64), (2, 0xB4)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::div(ee::from_i32(fx.mem.i8(row.wrapping_add(i as u32).wrapping_add(off))), 20.0);
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + half);
            l2::add_column(fx, sp + 0xA0, axis, ee::mul(b, r));
        }
        fx.mem.write_u32(sp + 0xC4, 0);
        fx.mem.write_u32(sp + 0xD4, 0);
        let h = fx.mem.i16(t + 0x38) >> 1;
        fx.mem.setf(sp + 0xC0, ee::from_i32(h.wrapping_neg()));
        let h = fx.mem.i16(t + 0x38) >> 1;
        fx.mem.setf(sp + 0xD0, ee::from_i32(h));
        fx.mem.write_u32(sp + 0xC8, 0);
        fx.mem.write_u32(sp + 0xD8, 0);
        for axis in 0..3 {
            l2::rotate4(fx, sp + 0xA0, t + 0x10, axis, tab);
        }
        if (0..=2).contains(&mode) && l2::project(fx, sp + 0xA0, SPR, sp + 0x110, d, sp + 0x100) {
            let ot = fx.mem.read_u32(D + 0x80);
            match mode {
                0 => {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x100)) as i32;
                    fx.submit(d, ot, key);
                }
                1 => {
                    l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                    fx.submit(d, ot, 0);
                }
                _ => fx.submit(d, ot, -1),
            }
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x200);
}

// ------------------------------------------------------------ spawn / emitter

/// 0x1E06880(D, step, 0) (frame 0x60: ra +0x50, the caller's s4..s0 at
/// +0x40..+0): the first free task cleared, the colour D+0x3154 twice,
/// the point D+0x250 as i16, +0x30 = D+0x3194, +0x3A = D+0x318E.
fn spawn(fx: &mut Fx, f: u32, s: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, s[4]), (0x30, s[3]), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x40 * k + 0x36) == 0) {
        let t = POOL + 0x40 * k;
        fx.bzero(t, 0x40);
        fx.mem.set16(t + 0x36, 1);
        for o in [0x28u32, 0x2C] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + o + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        let v = fx.mem.i16(INDEX);
        fx.mem.set16(t + 0x30, v);
        let v = fx.mem.i16(TURN);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.write_u32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, f);
    }
    fx.leave(0x60);
}

/// D+0x250 = the float vector at `from` (xyz).
fn pos_from(fx: &mut Fx, from: u32) {
    copy3(fx, POS, from);
}

/// 0x1E02830(D, 0) (frame 0x90: ra +0x60, the entry's s4..s0 +0x50..
/// +0x10 (s0 = D, s1 = 0, s2 = the param, s3 / s4 the runner's, 0 [H]),
/// f20 +0 (0 [H])).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x90);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x60, 0x01E0_5C24), (0x50, 0), (0x40, 0), (0x30, p), (0x20, 0), (0x10, u64::from(D))]);
    fx.mem.write_u32(sp, 0);
    let d = u64::from(D);
    let mut s: [u64; 5] = [d, 0, p, 0, 0];
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    add16(fx, COUNTER, 1);
    if [0xBD, 0xC2, 0xC7, 0xCE, 0xD3, 0xD8].contains(&counter(fx)) {
        add16(fx, WAVES, 1);
    }
    if counter(fx) == 1 {
        spawn(fx, LIGHT, s, 0x01E0_28EC);
    }
    if counter(fx) == 1 {
        spawn(fx, SPRITE_A, s, 0x01E0_2910);
    }
    if counter(fx) == 1 {
        spawn(fx, SPRITE_B, s, 0x01E0_2934);
    }
    if counter(fx) == 0xF2 {
        spawn(fx, WINDOW, s, 0x01E0_2958);
    }
    fx.mem.set16(INDEX, 0);
    if counter(fx) == 0xE8 {
        spawn(fx, FLASH, s, 0x01E0_2980);
    }
    fx.mem.set16(INDEX, 1);
    if counter(fx) == 0x12C {
        spawn(fx, FLASH, s, 0x01E0_29AC);
    }
    if counter(fx) == 0x37 {
        let h = fx.self_handle();
        crate::b076::unit_position_saving(fx, h, sp + 0x70, d, 0x01E0_29D0);
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 0xA);
        crate::b076::node_position_saving(fx, h, n, ANCHOR, d, Some(s[1]), 0x01E0_29E8);
        let z = fx.mem.read_u32(sp + 0x78);
        fx.mem.write_u32(ANCHOR + 8, z);
        for k in 0..6u32 {
            fx.mem.set16(INDEX, k as i32);
            for c in 0..3u32 {
                let i = fx.mem.i16(INDEX) as u32;
                let v = fx.mem.read_u32(ANCHOR + 4 * c);
                fx.mem.write_u32(ORBS.wrapping_add(0x10 * i) + 4 * c, v);
                fx.mem.write_u32(POS + 4 * c, v);
            }
            spawn(fx, ORB, s, 0x01E0_2A50 + 0x78 * k);
            spawn(fx, ORB_SPRITE, s, 0x01E0_2A64 + 0x78 * k);
        }
        for k in 0..6u32 {
            s[1] = u64::from(k);
            s[2] = u64::from(D + 0xA0 * k);
            s[3] = u64::from(D + 0x10 * k);
            fx.mem.set16(INDEX, k as i32);
            spawn(fx, TUBE1, s, 0x01E0_2CE4);
            for j in 0..0xAu32 {
                copy3(fx, TUBE_A + 0xA0 * k + 0x10 * j, ORBS + 0x10 * k);
            }
        }
        s[1] = 6;
        s[2] = u64::from(D + 0x3C0);
        s[3] = u64::from(D + 0x60);
        for k in 0..6u32 {
            s[2] = u64::from(k);
            s[1] = u64::from(D + 0xA0 * k);
            fx.mem.set16(INDEX, k as i32);
            spawn(fx, TUBE2, s, 0x01E0_2D68);
            for j in 0..0xAu32 {
                copy3(fx, TUBE_B + 0xA0 * k + 0x10 * j, POINT);
            }
        }
        s[2] = 6;
        s[1] = u64::from(D + 0x3C0);
    }
    if counter(fx) == 0xAA {
        for k in 0..6u32 {
            fx.mem.set16(INDEX, k as i32);
            for j in 0..0xAu32 {
                copy3(fx, TUBE_A + 0xA0 * k + 0x10 * j, ORBS + 0x10 * k);
            }
        }
    }
    let c = counter(fx);
    if (0x37..0x9C).contains(&c) {
        let h = fx.self_handle();
        crate::b076::unit_position_saving(fx, h, sp + 0x70, d, 0x01E0_2E78);
        drift(fx, sp);
    }
    if counter(fx) >= 0xBD {
        s[2] = u64::from(sp + 0x80);
        let mut k = 0i32;
        while k < fx.mem.i16(WAVES) {
            s[3] = k as u64;
            s[1] = u64::from(D + 0x10 * k as u32);
            let h = fx.target_handle();
            crate::b076::unit_position_saving(fx, h, sp + 0x70, d, 0x01E0_3114);
            let h = fx.self_handle();
            crate::b076::unit_position_saving(fx, h, sp + 0x80, d, 0x01E0_3128);
            for o in [0u32, 4, 8] {
                let v = ee::div(ee::sub(fx.mem.f(sp + 0x70 + o), fx.mem.f(sp + 0x80 + o)), 15.0);
                fx.mem.setf(sp + 0x80 + o, v);
            }
            let a = ORBS + 0x10 * k as u32 + 8;
            let v = ee::add(fx.mem.f(a), fx.mem.f(sp + 0x88));
            fx.mem.setf(a, v);
            k += 1;
        }
        s[3] = k as u64;
        s[1] = u64::from(D + 0x10 * k as u32);
        let mut k = 0i32;
        while k < fx.mem.i16(WAVES) {
            s[4] = k as u64;
            s[3] = k as u64;
            s[1] = u64::from(D + 2 * k as u32);
            s[2] = u64::from(D + 0x10 * k as u32);
            let ang = ANGLES + 2 * k as u32;
            let o = SWAY + 0x10 * k as u32;
            let (at, by) = match k {
                0 => (Some(o + 4), -30.0f32),
                1 => (Some(o), 30.0),
                2 => (Some(o + 4), 30.0),
                3 => (Some(o + 4), -30.0),
                4 => (Some(o), -30.0),
                5 => (Some(o + 4), 30.0),
                _ => (None, 0.0),
            };
            if let Some(at) = at {
                let a = fx.mem.i16(ang);
                let v = l2::sin_deg(fx, SINES, a, 0);
                fx.mem.setf(at, ee::mul(by, v));
            }
            add16(fx, ang, 0xF);
            k += 1;
        }
        s[4] = k as u64;
        s[3] = k as u64;
        s[1] = u64::from(D + 2 * k as u32);
        s[2] = u64::from(D + 0x10 * k as u32);
    }
    let mut k = 0i32;
    while k < fx.mem.i16(WAVES) {
        fx.mem.set16(INDEX, k);
        let o = 0x10 * k as u32;
        for c in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(SWAY + o + c), fx.mem.f(ORBS + o + c));
            fx.mem.setf(sp + 0x70 + c, v);
        }
        bspawn::history(fx, HIST + 0xA0 * k as u32, 0xA, sp + 0x70);
        k += 1;
    }
    let h = fx.target_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x70, d, 0x01E0_33B4);
    s[1] = u64::from(sp + 0x78);
    let tz = fx.mem.f(sp + 0x78);
    let f20 = gap(fx, tz, POINT + 8, 0x01E0_33CC);
    s[2] = u64::from(sp + 0x80);
    if counter(fx) < 0xF5 {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES);
        crate::b076::node_position_saving(fx, h, n, sp + 0x70, d, Some(s[1]), 0x01E0_33F4);
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 4);
        crate::b076::node_position_saving(fx, h, n, sp + 0x80, d, Some(s[1]), 0x01E0_3410);
        for o in [0u32, 4, 8] {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x70 + o), fx.mem.f(sp + 0x80 + o)), 2.0);
            fx.mem.setf(sp + 0x80 + o, v);
        }
        for o in [0u32, 4, 8] {
            let v = ee::sub(fx.mem.f(sp + 0x70 + o), fx.mem.f(sp + 0x80 + o));
            fx.mem.setf(POINT + o, v);
        }
    } else {
        let h = fx.target_handle();
        crate::b076::unit_position_saving(fx, h, sp + 0x70, d, 0x01E0_34A8);
        let h = fx.self_handle();
        crate::b076::unit_position_saving(fx, h, sp + 0x80, d, 0x01E0_34C0);
        for o in [0u32, 4, 8] {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x70 + o), fx.mem.f(sp + 0x80 + o)), 40.0);
            fx.mem.setf(sp + 0x80 + o, v);
        }
        if !(f20 <= 3.0) {
            let v = ee::add(fx.mem.f(POINT + 8), fx.mem.f(sp + 0x88));
            fx.mem.setf(POINT + 8, v);
            pos_at_point(fx);
            spawn(fx, BAND_B, s, 0x01E0_356C);
        }
    }
    let c = counter(fx);
    if (0x39..0xE7).contains(&c) {
        pos_at_point(fx);
        fx.mem.set16(INDEX, 0);
        spawn(fx, SLIVERS, s, 0x01E0_35C4);
        let v = fx.mem.i16(SIZE);
        if v < 0x140 {
            fx.mem.set16(SIZE, v + 2);
        }
    }
    let h = fx.target_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x70, d, 0x01E0_35F0);
    let tz = fx.mem.f(sp + 0x78);
    let g = gap(fx, tz, POINT + 8, 0x01E0_3604);
    if g < 3.0 {
        if counter(fx) >= 0x104 {
            add16(fx, LANDED, 1);
            add16(fx, SIZE, 0x14);
            if counter(fx) < 0x12D {
                pos_at_point(fx);
                spawn(fx, BAND_C, s, 0x01E0_3690);
            }
        }
    } else {
        let c = counter(fx);
        if fx.div(c, 3).1 == 1 {
            pos_at_point(fx);
            fx.mem.set16(INDEX, 0);
            spawn(fx, BAND_A, s, 0x01E0_36F8);
        }
    }
    let c = counter(fx);
    if (0x32..0xF3).contains(&c) {
        s[2] = u64::from(sp + 0x74);
        for (n, ra_n, ra_s) in [(0xAu32, 0x01E0_372Cu32, 0x01E0_3764u32), (0xE, 0x01E0_377C, 0x01E0_37B0)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + n);
            crate::b076::node_position_saving(fx, h, slot, sp + 0x70, d, Some(s[1]), ra_n);
            pos_from(fx, sp + 0x70);
            fx.mem.set16(INDEX, 1);
            spawn(fx, RING_C, s, ra_s);
        }
    }
    let c = counter(fx);
    if (0x32..0xF3).contains(&c) {
        s[2] = u64::from(sp + 0x74);
        for (n, ra_n, ra_s) in [(0u32, 0x01E0_37E4u32, 0x01E0_3818u32), (4, 0x01E0_3830, 0x01E0_3860)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + n);
            crate::b076::node_position_saving(fx, h, slot, sp + 0x70, d, Some(s[1]), ra_n);
            pos_from(fx, sp + 0x70);
            fx.mem.set16(INDEX, 0);
            spawn(fx, RING_C, s, ra_s);
        }
    }
    let c = counter(fx);
    if (0x38..0xB5).contains(&c) {
        fx.mem.set16(INDEX, 1);
        for k in 0..6u32 {
            s[3] = u64::from(k);
            s[2] = u64::from(D + 0x10 * k);
            pos_from(fx, ORBS + 0x10 * k);
            spawn(fx, RING_C, s, 0x01E0_38BC);
        }
        s[3] = 6;
        s[2] = u64::from(D + 0x60);
    }
    for (at, n, ra_n, ras) in [(0xBBi32, 0xAu32, 0x01E0_3910u32, [0x01E0_3940u32, 0x01E0_395C, 0x01E0_3978]), (0xCD, 0xE, 0x01E0_39A0, [0x01E0_39D0, 0x01E0_39EC, 0x01E0_3A08])] {
        if counter(fx) == at {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + n);
            crate::b076::node_position_saving(fx, h, slot, sp + 0x70, d, Some(s[1]), ra_n);
            pos_from(fx, sp + 0x70);
            for (i, ra) in ras.into_iter().enumerate() {
                fx.mem.set16(INDEX, i as i32);
                spawn(fx, BAND_D, s, ra);
            }
        }
    }
    fx.leave(0x90);
}

/// 0x37..0x9B: the six orbs D+0x2870 + 0x10 k drift by 1/100 toward the
/// attacker's position (sp + 0x70) offset by (dx, dy) about the anchor
/// D+0x3120 (y - 5 throughout), the z straight.
fn drift(fx: &mut Fx, sp: u32) {
    let x = fx.mem.f(sp + 0x70);
    let y = fx.mem.f(sp + 0x74);
    let z = fx.mem.f(sp + 0x78);
    // The target x / y per orb as the original adds them (f3 = 10, f0 = 20).
    let xs = [ee::add(10.0, x), ee::add(20.0, x), ee::add(10.0, x), ee::sub(x, 10.0), ee::sub(x, 20.0), ee::sub(x, 10.0)];
    let ys = [y, ee::add(10.0, y), ee::add(20.0, y), y, ee::add(10.0, y), ee::add(20.0, y)];
    for k in 0..6u32 {
        let o = ORBS + 0x10 * k;
        let v = ee::add(fx.mem.f(o), ee::div(ee::sub(xs[k as usize], fx.mem.f(ANCHOR)), 100.0));
        fx.mem.setf(o, v);
        let v = ee::add(fx.mem.f(o + 4), ee::div(ee::sub(ee::sub(ys[k as usize], 5.0), fx.mem.f(ANCHOR + 4)), 100.0));
        fx.mem.setf(o + 4, v);
        let v = ee::add(fx.mem.f(o + 8), ee::div(ee::sub(z, fx.mem.f(ANCHOR + 8)), 100.0));
        fx.mem.setf(o + 8, v);
    }
}

// ------------------------------------------------------------ steps

/// The entry's step-loop registers: s0 = D, s1 = 0, s2 = D + 0x40 k, s3 =
/// k, s4 / s5 the runner's (0 [H]).
fn regs(k: u32) -> [u64; 6] {
    [u64::from(D), 0, u64::from(D + 0x40 * k), u64::from(k), 0, 0]
}

/// A step's prologue: frame `size`, ra at `ra_at`, the entry's s0..s(n-1)
/// at 0x10 i.
fn enter_step(fx: &mut Fx, size: u32, n: usize, ra_at: u32, k: u32, ra: u32) -> u32 {
    let sp = fx.enter(size);
    let e = regs(k);
    for (i, v) in e.iter().enumerate().take(n) {
        fx.saved(sp + 0x10 * i as u32, &[(0, *v)]);
    }
    fx.saved(sp, &[(ra_at, u64::from(ra))]);
    sp
}

fn run_step(fx: &mut Fx, f: u32, t: u32, k: u32, ra: u32) {
    match f {
        SLIVERS => {
            enter_step(fx, 0x30, 2, 0x20, k, ra);
            slivers_step(fx, t);
            fx.leave(0x30);
        }
        BAND_A => {
            enter_step(fx, 0x20, 1, 0x10, k, ra);
            band_a(fx, t);
            fx.leave(0x20);
        }
        ORB => {
            let sp = enter_step(fx, 0x50, 2, 0x20, k, ra);
            orb(fx, sp, t, k);
            fx.leave(0x50);
        }
        TUBE1 => {
            let sp = enter_step(fx, 0x50, 2, 0x20, k, ra);
            tube1(fx, sp, t);
            fx.leave(0x50);
        }
        RING_A | RING_B => {
            enter_step(fx, 0x20, 1, 0x10, k, ra);
            if f == RING_A {
                ring_a(fx, t);
            } else {
                ring_b(fx, t);
            }
            fx.leave(0x20);
        }
        TUBE2 => {
            let sp = enter_step(fx, 0x80, 6, 0x60, k, ra);
            tube2(fx, sp, t);
            fx.leave(0x80);
        }
        SPRITE_A => {
            enter_step(fx, 0x30, 2, 0x20, k, ra);
            sprite_a(fx, t);
            fx.leave(0x30);
        }
        SPRITE_B => {
            enter_step(fx, 0x30, 2, 0x20, k, ra);
            sprite_b(fx, t, k);
            fx.leave(0x30);
        }
        ORB_SPRITE => {
            let sp = enter_step(fx, 0x50, 2, 0x20, k, ra);
            orb_sprite(fx, sp, t);
            fx.leave(0x50);
        }
        BAND_B => {
            enter_step(fx, 0x20, 1, 0x10, k, ra);
            band_b(fx, t);
            fx.leave(0x20);
        }
        FLASH => {
            enter_step(fx, 0x20, 1, 0x10, k, ra);
            flash(fx, t);
            fx.leave(0x20);
        }
        BAND_C => {
            enter_step(fx, 0x20, 1, 0x10, k, ra);
            band_c(fx, t);
            fx.leave(0x20);
        }
        RING_C => {
            enter_step(fx, 0x20, 1, 0x10, k, ra);
            ring_c(fx, t);
            fx.leave(0x20);
        }
        BAND_D => {
            enter_step(fx, 0x20, 1, 0x10, k, ra);
            band_d(fx, t);
            fx.leave(0x20);
        }
        BAND_E => {
            enter_step(fx, 0x20, 1, 0x10, k, ra);
            band_e(fx, t);
            fx.leave(0x20);
        }
        LIGHT => {
            enter_step(fx, 0x20, 1, 0x10, k, ra);
            light(fx, t);
            fx.leave(0x20);
        }
        WINDOW => {
            let sp = enter_step(fx, 0x80, 1, 0x10, k, ra);
            window(fx, sp, t);
            fx.leave(0x80);
        }
        _ => {}
    }
}

/// 0x1E03A30 (frame 0x30): a sliver set (size 50 falling by +0xE = rand %
/// 3 + 4, angles rand % 0x4000 / % 0xFFFF, cell rand % 4; blue (0x20 /
/// 0x40, 0x20, 0x80) by +0x30); in by 10 below 0x80 for 12 updates, then
/// out by 0x13 once small, ended below size -12 or out by 15 below timer
/// 10.
fn slivers_step(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x32);
        let r = rand_and(fx, t, 0x01E0_3A60, 0x4000);
        fx.mem.set32(t + 0x10, r);
        let r = rmod(fx, t, 0x01E0_3A80, 0xFFFF);
        fx.mem.set32(t + 0x14, r);
        let r = rand_and(fx, t, 0x01E0_3A9C, 4);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
        let r = rmod(fx, t, 0x01E0_3AC0, 3);
        fx.mem.set16(t + 0xE, r + 4);
        fx.mem.set16(t + 6, 1);
        if fx.mem.i16(t + 0x30) == 0 {
            l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x80);
        } else {
            l2::rgb(fx, t + 0x28, 0x40, 0x20, 0x80);
        }
    }
    if age(fx, t) < 0xC {
        rise(fx, t + 0x2B, 0x80, 0xA);
    } else {
        if fx.mem.i16(t + 0x38) < 0xC {
            fall(fx, t, 0x2B, 0x15, 0x13);
        }
        if fx.mem.i16(t + 0x38) < -0xC {
            kill(fx, t);
        }
        if timer(fx) < 0xA {
            fall(fx, t, 0x2B, 0x11, 0xF);
        }
    }
    grow_age(fx, t);
    let v = fx.mem.i16(t + 0x38) - fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x38, v);
}

/// 0x1E03C00 (frame 0x20): a band (size D+0x3198, grey / (0x20, 0x20,
/// 0x80), longitude rand % 360) facing the camera (x + 0x4000), alpha 0xA0
/// then out by 0x3A.
fn band_a(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let v = fx.mem.i16(SIZE);
        fx.mem.set16(t + 0x38, v);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x2C, 0x20, 0x20, 0x80);
        let r = rmod(fx, t, 0x01E0_3C54, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) <= 0 {
        fx.mem.write_u8(t + 0x2B, 0xA0);
    } else {
        fall(fx, t, 0x2B, 0x3D, 0x3A);
    }
    face(fx, t, 0x01E0_3CB8);
    grow_age(fx, t);
}

/// The task's angles +0x10 = the camera's (0x205580: the step's s0 = the
/// task, s1 = the entry's 0), x + 0x4000.
fn face(fx: &mut Fx, t: u32, ra: u32) {
    crate::b494::camera_angles_saving(fx, t + 0x10, u64::from(t), 0, ra);
    let x = fx.mem.i32(t + 0x10).wrapping_add(0x4000);
    fx.mem.set32(t + 0x10, x);
}

/// The target's z (0x1F71A0 into sp + 0x30 from the step) against orb
/// +0x30's z.
fn orb_gap(fx: &mut Fx, sp: u32, t: u32, ra: u32, ra_sqrt: u32) -> f32 {
    let h = fx.target_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x30, u64::from(t), ra);
    let i = fx.mem.i16(t + 0x30) as u32;
    let z = fx.mem.f(sp + 0x38);
    gap(fx, z, ORBS.wrapping_add(0x10 * i) + 8, ra_sqrt)
}

/// 0x1E03CF0 (frame 0x50): an orb's band (size 0 growing by 2 to 0x28,
/// (0xC0, 0xC0, 0xC0) / (0xF0, 0x20, 0x20)), flickering 0xA0 / 0x80,
/// facing the camera; once within 5 of the target's z the burst: a band
/// 0x5630 and two rings 0x4300 at the orb (D+0x3194 = 1), and its end.
fn orb(fx: &mut Fx, sp: u32, t: u32, k: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x2C, 0xF0, 0x20, 0x20);
        let r = rmod(fx, t, 0x01E0_3D48, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    let a = age(fx, t);
    let v = if a <= 0 || odd(a) { 0xA0 } else { 0x80 };
    fx.mem.write_u8(t + 0x2B, v);
    let s = fx.mem.i16(t + 0x38);
    if s < 0x28 {
        fx.mem.set16(t + 0x38, s + 2);
    }
    grow_age(fx, t);
    let g = orb_gap(fx, sp, t, 0x01E0_3DF0, 0x01E0_3E10);
    if g < 5.0 {
        let i = fx.mem.i16(t + 0x30) as u32;
        copy3(fx, POS, ORBS.wrapping_add(0x10 * i));
        fx.mem.set16(INDEX, 1);
        let e = regs(k);
        let s = [u64::from(t), u64::from(D), e[2], e[3], e[4]];
        spawn(fx, BAND_E, s, 0x01E0_3E84);
        spawn(fx, RING_A, s, 0x01E0_3E98);
        spawn(fx, RING_A, s, 0x01E0_3EAC);
        kill(fx, t);
    }
    crate::b494::camera_angles_saving(fx, t + 0x10, u64::from(t), u64::from(D), 0x01E0_3EBC);
    let x = fx.mem.i32(t + 0x10).wrapping_add(0x4000);
    fx.mem.set32(t + 0x10, x);
}

/// The six thresholds of the tubes' growth by kind +0x30.
const KINDS: [(i32, i32); 6] = [(0xBD, 0), (0xC2, 1), (0xC7, 2), (0xCE, 3), (0xD3, 4), (0xD8, 5)];

/// 0x1E03EE0 (frame 0x50): an orb's tube (magenta, +0xA rand % 17 + 44,
/// +8 rand % 8 + 5, longitude rand % 360): its points +6 and size +0x38
/// grow (to 8 / 0x28) from its kind's update; in by 0x12 for 12 updates,
/// out by 0x3F once within 5 of the target's z.
fn tube1(fx: &mut Fx, sp: u32, t: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0x80, 0x10, 0x80);
        fx.mem.set16(t + 6, 0);
        fx.mem.write_u8(t + 0x2B, 0);
        let r = rmod(fx, t, 0x01E0_3F24, 0x11);
        fx.mem.set16(t + 0xA, r + 0x2C);
        let r = rand_and(fx, t, 0x01E0_3F44, 8);
        fx.mem.set16(t + 8, r + 5);
        let r = rmod(fx, t, 0x01E0_3F64, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x38, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    for (at, kind) in KINDS {
        if counter(fx) >= at && fx.mem.i16(t + 0x30) == kind {
            let v = fx.mem.i16(t + 6);
            if v < 8 {
                fx.mem.set16(t + 6, v + 1);
            }
        }
    }
    for (at, kind) in KINDS {
        if counter(fx) >= at && fx.mem.i16(t + 0x30) == kind {
            let v = fx.mem.i16(t + 0x38);
            if v < 0x28 {
                fx.mem.set16(t + 0x38, v + 4);
            }
        }
    }
    let g = orb_gap(fx, sp, t, 0x01E0_4234, 0x01E0_4254);
    if g < 5.0 {
        fx.mem.set16(t + 0x36, 3);
    }
    if age(fx, t) < 0xC {
        rise(fx, t + 0x2B, 0x80, 0x12);
    } else if fx.mem.i16(t + 0x36) == 3 {
        fall(fx, t, 0x2B, 0x41, 0x3F);
    }
    grow_age(fx, t);
}

/// 0x1E04300 / 0x1E04DB0: a ring of `ring_spun` sprites (size 0x50, cell
/// 1, grey / (0x20, 0x20, 0x40)), its radius +0x30 growing by +0xE; in by
/// 0x40 for +6 updates, then out by 10.
fn ring_body(fx: &mut Fx, t: u32) {
    if age(fx, t) < fx.mem.i16(t + 6) {
        rise(fx, t + 0x2B, 0x81, 0x40);
    } else {
        fall(fx, t, 0x2B, 0x11, 0xA);
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, v);
    }
    grow_age(fx, t);
}

/// 0x1E04300 (frame 0x20): +0x18 rand % 0xFFFF, grey, +0x3A rand % 360,
/// +0xE rand % 18 + 80, +8 rand % 20, +6 rand % 7 + 3.
fn ring_a(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x50);
        fx.mem.set16(t + 0x34, 1);
        let r = rmod(fx, t, 0x01E0_432C, 0xFFFF);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.set32(t + 0x18, r);
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, t, 0x01E0_4358, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rmod(fx, t, 0x01E0_4374, 0x12);
        fx.mem.set16(t + 0xE, r + 0x50);
        let r = rmod(fx, t, 0x01E0_4394, 0x14);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, 0x01E0_43B0, 7);
        fx.mem.set16(t + 6, r + 3);
        fx.mem.set16(t + 0x36, 2);
    }
    ring_body(fx, t);
}

/// 0x1E04DB0 (frame 0x20): angles rand % 360, (0x20, 0x20, 0x40), +0x3A
/// rand % 360, +0xE rand % 80 + 120, +8 rand % 20, +6 rand % 7 + 23.
fn ring_b(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x50);
        fx.mem.set16(t + 0x34, 1);
        for (o, ra) in [(0x10u32, 0x01E0_4DDCu32), (0x14, 0x01E0_4DF8)] {
            let r = rmod(fx, t, ra, 0x168);
            fx.mem.set32(t + o, r);
        }
        let r = rmod(fx, t, 0x01E0_4E14, 0x168);
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x40);
        fx.mem.set32(t + 0x18, r);
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, t, 0x01E0_4E40, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rmod(fx, t, 0x01E0_4E5C, 0x50);
        fx.mem.set16(t + 0xE, r + 0x78);
        let r = rmod(fx, t, 0x01E0_4E7C, 0x14);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, 0x01E0_4E98, 7);
        fx.mem.set16(t + 6, r + 0x17);
        fx.mem.set16(t + 0x36, 2);
    }
    ring_body(fx, t);
}

/// 0x1E04460 (frame 0x80; its rand's s0 the entry's D): a tube about the
/// point (dark violet, 9 points, angles rand % 0xFFFF, +0xA rand % 17 +
/// 44, +8 rand % 8 + 5, +0xE / +0x3A rand % 360, size 2): in by 0x1B for
/// 12 updates, then out by 0x1D within 3 of the target's z; its ten
/// points D+0x2D60 + 0xA0 (+0x30) + 0x10 j = 20 (sin, cos, cos)(+0x3A -
/// 10 j); +0x3A turns by 18.
fn tube2(fx: &mut Fx, sp: u32, t: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0x40, 0x10, 0x80);
        fx.mem.set16(t + 6, 9);
        for (o, ra) in [(0x10u32, 0x01E0_44B8u32), (0x14, 0x01E0_44D4), (0x18, 0x01E0_44F0)] {
            let r = rmod(fx, D, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        fx.mem.write_u8(t + 0x2B, 0);
        let r = rmod(fx, D, 0x01E0_4510, 0x11);
        fx.mem.set16(t + 0xA, r + 0x2C);
        let r = rand_and(fx, D, 0x01E0_4530, 8);
        fx.mem.set16(t + 8, r + 5);
        let r = rmod(fx, D, 0x01E0_4550, 0x168);
        fx.mem.set16(t + 0xE, r);
        let r = rmod(fx, D, 0x01E0_456C, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x38, 2);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0xC {
        rise(fx, t + 0x2B, 0xC0, 0x1B);
    } else {
        let h = fx.target_handle();
        crate::b076::unit_position_saving(fx, h, sp + 0x70, u64::from(D), 0x01E0_45D0);
        let z = fx.mem.f(sp + 0x78);
        if gap(fx, z, POINT + 8, 0x01E0_45E4) < 3.0 {
            fall(fx, t, 0x2B, 0x1F, 0x1D);
        }
    }
    grow_age(fx, t);
    let mut s1 = 0i32;
    for j in 0..0xAu32 {
        let deg = (fx.mem.i16(t + 0x3A) + s1) as i16 as i32;
        for (c, cos) in [(0u32, 0), (4, 1), (8, 1)] {
            let v = l2::sin_deg(fx, SINES, deg, cos);
            let i = fx.mem.i16(t + 0x30) as u32;
            fx.mem.setf(TUBE_B.wrapping_add(0xA0 * i) + 0x10 * j + c, ee::mul(20.0, v));
        }
        s1 = (s1 - 0xA) as i16 as i32;
    }
    add16(fx, t + 0x3A, 0x12);
    let v = fx.mem.i16(t + 0x3A);
    if v >= 0x168 {
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
    }
}

/// The two sprites' set-up (size 0, (0x30, 0x30, 0x80) twice, +8 = 7, +0xA
/// = 0x19).
fn sprite_first(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        l2::rgb(fx, t + 0x28, 0x30, 0x30, 0x80);
        l2::rgb(fx, t + 0x2C, 0x30, 0x30, 0x80);
        fx.mem.set16(t + 8, 7);
        fx.mem.set16(t + 0xA, 0x19);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
}

fn grey2(fx: &mut Fx, t: u32, v: i32) {
    l2::rgb(fx, t + 0x28, v, v, v);
    l2::rgb(fx, t + 0x2C, v, v, v);
}

/// Size = D+0x3198 / 3 (the magic multiply); +0x18 turns by 4.
fn size_third(fx: &mut Fx, t: u32) {
    let v = fx.mem.i16(SIZE);
    let hi = ((i64::from(v) * 0x5555_5556) >> 32) as i32;
    fx.mult(0x5555_5556, v);
    let q = hi + ((v as u32) >> 31) as i32;
    fx.mem.set16(t + 0x38, q);
    let r = fx.mem.read_u32(t + 0x18).wrapping_add(4);
    fx.mem.write_u32(t + 0x18, r);
}

/// 0x1E047A0 (frame 0x30): the point's grey sprite (0x48 / 0x50 flicker,
/// 0x40 once landed; cell rand % 8, 1 once landed), in by 0x30 below 0x60
/// for 11 updates; size D+0x3198 / 3; out by 9 from the ninth landing.
fn sprite_a(fx: &mut Fx, t: u32) {
    sprite_first(fx, t);
    if age(fx, t) < 0xB {
        rise(fx, t + 0x2B, 0x60, 0x30);
        rise(fx, t + 0x2F, 0x60, 0x30);
    }
    let r = rand_and(fx, t, 0x01E0_4854, 8);
    fx.mem.set16(t + 0x34, r);
    if fx.mem.i16(LANDED) == 0 {
        if odd(age(fx, t)) {
            grey2(fx, t, 0x48);
        } else {
            grey2(fx, t, 0x50);
        }
    } else {
        grey2(fx, t, 0x40);
    }
    grow_age(fx, t);
    size_third(fx, t);
    if fx.mem.i16(LANDED) != 0 {
        fx.mem.set16(t + 0x34, 1);
    }
    if fx.mem.i16(LANDED) >= 9 {
        fall(fx, t, 0x2B, 0xB, 9);
    }
}

/// 0x1E049C0 (frame 0x30): the point's flickering sprite pair (0x60 /
/// 0xA0), in by 0x40 below 0x80 for 11 updates, by 0x30 below 0xF0 once
/// landed; on the first landing four rings 0x4DB0 at the point and its
/// end; out by 9 from the eleventh.
fn sprite_b(fx: &mut Fx, t: u32, k: u32) {
    sprite_first(fx, t);
    if age(fx, t) < 0xB {
        rise(fx, t + 0x2B, 0x80, 0x40);
        rise(fx, t + 0x2F, 0x80, 0x40);
    } else if fx.mem.i16(LANDED) != 0 {
        rise(fx, t + 0x2B, 0xF0, 0x30);
        rise(fx, t + 0x2F, 0xF0, 0x30);
    }
    let r = rand_and(fx, t, 0x01E0_4AB4, 8);
    fx.mem.set16(t + 0x34, r);
    if odd(age(fx, t)) {
        grey2(fx, t, 0x60);
    } else {
        grey2(fx, t, 0xA0);
    }
    grow_age(fx, t);
    size_third(fx, t);
    if fx.mem.i16(LANDED) == 1 {
        pos_at_point(fx);
        fx.mem.set16(INDEX, 0);
        let e = regs(k);
        let s = [u64::from(t), u64::from(D), e[2], e[3], e[4]];
        for ra in [0x01E0_4BD4u32, 0x01E0_4BE8, 0x01E0_4BFC, 0x01E0_4C10] {
            spawn(fx, RING_B, s, ra);
        }
        kill(fx, t);
    }
    if fx.mem.i16(LANDED) >= 0xB {
        fall(fx, t, 0x2B, 0xB, 9);
    }
}

/// 0x1E04C60 (frame 0x50): an orb's sprite (0xC0 grey, size up by 1 to
/// 0x14, flickering 0x90 / 0x80, turning by 4), ended within 3 of the
/// target's z.
fn orb_sprite(fx: &mut Fx, sp: u32, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, 0x01E0_4CA4, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    let a = age(fx, t);
    let v = if a <= 0 || odd(a) { 0x90 } else { 0x80 };
    fx.mem.write_u8(t + 0x2B, v);
    let s = fx.mem.i16(t + 0x38);
    if s < 0x14 {
        fx.mem.set16(t + 0x38, s + 1);
    }
    grow_age(fx, t);
    let g = orb_gap(fx, sp, t, 0x01E0_4D4C, 0x01E0_4D6C);
    let r = fx.mem.read_u32(t + 0x18).wrapping_add(4);
    fx.mem.write_u32(t + 0x18, r);
    if g < 3.0 {
        kill(fx, t);
    }
}

/// 0x1E04F50 (frame 0x20): a band at the point (size 0x5A growing by 1,
/// (0x10..) / white, turned 0x4000 about z), alpha +0xA0 for 7 updates,
/// then out by 8.
fn band_b(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x5A);
        l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x10);
        l2::rgb(fx, t + 0x2C, 0xFF, 0xFF, 0xFF);
        fx.mem.write_u32(t + 0x18, 0x4000);
        let r = rmod(fx, t, 0x01E0_4FA8, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 7 {
        rise(fx, t + 0x2B, 0xA0, 0xA0);
    } else {
        fall(fx, t, 0x2B, 0xB, 8);
    }
    if age(fx, t) != 0 {
        add16(fx, t + 0x38, 1);
    }
    grow_age(fx, t);
}

/// 0x1E05050 (frame 0x20): a full-window flash (kind +0x3A = the spawn's
/// +0x30): kind 0 (0xF0, 0xF0, 0x80) in by 0x10 below 0xA0 for 15
/// updates, then out by 9; kind 1 (0xFF, 0xFF, 0xC0) by 0xD below 0xD0,
/// out by 6; +0x30 = 1 once the first alpha tops out.
fn flash(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let v = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x30, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    let kind = fx.mem.i16(t + 0x3A);
    let (cap, up, floor, down, rgb) = match kind {
        0 => (0xA0u8, 0x10u8, 0xBu8, 9u8, (0xF0, 0xF0, 0x80)),
        1 => (0xD0, 0xD, 8, 6, (0xFF, 0xFF, 0xC0)),
        _ => {
            grow_age(fx, t);
            return;
        }
    };
    if age(fx, t) < 0xF {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < cap {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(up));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
        rise(fx, t + 0x2F, cap, up);
    } else {
        fall(fx, t, 0x2B, floor, down);
        fall_only(fx, t + 0x2F, floor, down);
    }
    l2::rgb(fx, t + 0x28, rgb.0, rgb.1, rgb.2);
    l2::rgb(fx, t + 0x2C, rgb.0, rgb.1, rgb.2);
    grow_age(fx, t);
}

/// 0x1E05220 (frame 0x20): a landing band (size 0x64 growing by 15,
/// (0x10..) / (0x2F, 0x2F, 0xFF), angles rand % 0xFFFF), alpha 0x10, the
/// rim in by 0xC0 for 20 updates, then out by 8.
fn band_c(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x64);
        l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x10);
        l2::rgb(fx, t + 0x2C, 0x2F, 0x2F, 0xFF);
        for (o, ra) in [(0x10u32, 0x01E0_5274u32), (0x14, 0x01E0_5290), (0x18, 0x01E0_52AC)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        let r = rmod(fx, t, 0x01E0_52C8, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0x10);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 0x14 {
        rise(fx, t + 0x2F, 0xC0, 0xC0);
    } else {
        fall(fx, t, 0x2F, 0xB, 8);
    }
    if age(fx, t) != 0 {
        add16(fx, t + 0x38, 0xF);
    }
    grow_age(fx, t);
}

/// 0x1E05380 (frame 0x20): a ring of 2 `ring_spun` sprites (size 10, cell
/// = the spawn's +0x30, grey, +0x18 rand % 0xFFFF, radius +0x30 rand %
/// 100 + 20, +0x3A rand % 360, +0xE rand % 18 + 80, +8 rand % 20), alpha
/// in by 0x40 on its first update, then out by 15.
fn ring_c(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0xA);
        let v = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x34, v);
        let r = rmod(fx, t, 0x01E0_53B0, 0xFFFF);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.set32(t + 0x18, r);
        let r = rmod(fx, t, 0x01E0_53DC, 0x64);
        fx.mem.set16(t + 0x30, r + 0x14);
        let r = rmod(fx, t, 0x01E0_53FC, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rmod(fx, t, 0x01E0_5418, 0x12);
        fx.mem.set16(t + 0xE, r + 0x50);
        let r = rmod(fx, t, 0x01E0_5438, 0x14);
        fx.mem.set16(t + 8, r);
        fx.mem.set16(t + 6, 1);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        rise(fx, t + 0x2B, 0x81, 0x40);
    } else {
        fall(fx, t, 0x2B, 0x11, 0xF);
    }
    grow_age(fx, t);
}

/// 0x1E054D0 (frame 0x20): a hand band (size 0 growing by 1 / 2 / 3 by
/// the spawn's +0x30, inner +6 = size - 2, (0x10..) / (0x20..), turned
/// 0x4000 about x), the rim in by 0xC0 for 2 updates, then out by 0xE.
fn band_d(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x10);
        l2::rgb(fx, t + 0x2C, 0x20, 0x20, 0x20);
        fx.mem.write_u32(t + 0x10, 0x4000);
        fx.mem.write_u32(t + 0x14, 0);
        fx.mem.write_u32(t + 0x18, 0);
        let r = rmod(fx, t, 0x01E0_552C, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 2 {
        rise(fx, t + 0x2F, 0xC0, 0xC0);
    } else {
        fall(fx, t, 0x2F, 0x10, 0xE);
    }
    if age(fx, t) != 0 {
        for (kind, by) in [(0, 1), (1, 2), (2, 3)] {
            if fx.mem.i16(t + 0x30) == kind {
                add16(fx, t + 0x38, by);
            }
        }
    }
    let v = fx.mem.i16(t + 0x38) - 2;
    fx.mem.set16(t + 6, v);
    grow_age(fx, t);
}

/// 0x1E05630 (frame 0x20): an orb's burst band (size 0x2D0, (0xF0..) /
/// (0x80, 0x80, 0x20) or (0x80, 0x20, 0x20) by +0x30), facing the camera;
/// alpha 0xA0, then out by 0x30.
fn band_e(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x2D0);
        match fx.mem.i16(t + 0x30) {
            0 => {
                l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0xF0);
                l2::rgb(fx, t + 0x2C, 0x80, 0x80, 0x20);
            }
            1 => {
                l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0xF0);
                l2::rgb(fx, t + 0x2C, 0x80, 0x20, 0x20);
            }
            _ => {}
        }
        let r = rmod(fx, t, 0x01E0_56C8, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    face(fx, t, 0x01E0_56F4);
    if age(fx, t) <= 0 {
        fx.mem.write_u8(t + 0x2B, 0xA0);
    } else {
        fall(fx, t, 0x2B, 0x33, 0x30);
    }
    grow_age(fx, t);
}

/// 0x1E05760 (frame 0x20, b491's 0x1E03210): the stage light (0x1F7F10)
/// down by 10 to 0x30 while the timer is at 31 or more, then back up by 10
/// to 0x80; 0x80 and the task's end on timer 2.
fn light(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x30, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    let l = fx.mem.i16(t + 0x30);
    if timer(fx) >= 0x1F {
        fx.mem.set16(t + 0x30, if l < 0x31 { 0x30 } else { l - 0xA });
    } else {
        fx.mem.set16(t + 0x30, if l < 0x80 { l + 0xA } else { 0x80 });
    }
    if timer(fx) == 2 {
        fx.set_stage_light(0x80);
        kill(fx, t);
    } else {
        let l = fx.mem.read_u8(t + 0x30);
        fx.set_stage_light(i32::from(l));
    }
}

/// 0x1E05830 (frame 0x80, b233's 0x1E03B60): a window sprite (0x6FF8 ..
/// 0x8FF8) sampling the frame (0x207EF0, filter 1) at ST 0..1, grey with
/// alpha +0x2B, ALPHA 0x44, key 0; in by 10 below 0x50 for 0x55 updates,
/// then out by 9; +0x30 counts up to 0x1E.
fn window(fx: &mut Fx, sp: u32, t: u32) {
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..3 {
        fx.mem.write_u8(sp + 0x38 + k, 0x80);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    fx.mem.write_u8(sp + 0x3B, a);
    fx.mem.write_u32(sp + 0x3C, 0x3F80_0000);
    for (at, v) in [(sp + 0x48, 0x6FF8u16), (sp + 0x58, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for (o, v) in [(0x40u32, 0), (0x44, 0), (0x50, 0x3F80_0000), (0x54, 0x3F80_0000)] {
        fx.mem.write_u32(sp + o, v);
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if age(fx, t) < 0x55 {
        rise(fx, t + 0x2B, 0x50, 0xA);
    } else {
        fall(fx, t, 0x2B, 0xB, 9);
    }
    grow_age(fx, t);
    let v = fx.mem.i16(t + 0x30);
    if v < 0x1E {
        fx.mem.set16(t + 0x30, v + 1);
    }
}
