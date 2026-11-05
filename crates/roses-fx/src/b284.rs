//! BMODULE b284 (card 447), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E024D0, data area 0x1ED0000) [F]: an older task pool of 200 tasks of 0x40 bytes at D+0x2F0 (alive
//! +0x36, step +0x24 as a record address) drawn with the second library:
//! camera-facing rings at the task (07c172c43e56 at 0x1E009F0,
//! `quads4::ring_facing_i`) and at D+0x20 with a mode (383a1b86ead2 at
//! 0x1E00F70, `quads4::ring_at_m`), UVs by uv_rect 0x1E00150. The
//! attacker's node 0x1C keeps a ten-entry history at D+0x3560..+0x35F0
//! (a72bfb6bb70a at 0x1E00080, `spawn::node_history`). From update 67 to
//! 274 purple smoke (two kinds) and from 71 a red mote leave node 0x1C,
//! drifting towards the target. Returns 1 on update 60, 4 on 320, 2 on
//! 300, 6 on 71..294, 8 once the 339-update timer +0x3750 has run out.
//!
//! The steps 0x1E01990 (returns 0), 0x1E020C0, 0x1E02230, 0x1E02340 and
//! the entry's case 0x1E022F0 (an object op through 0x1F8380) are never
//! spawned [F: the emitter's `jal`s name 0x1E01D30 / 0x1E01620 /
//! 0x1E019A0 only]; not ported.
use crate::blib::{quads4, spawn, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
/// The step functions a task holds (+0x24), as the record's addresses.
const MOTE: u32 = 0x01E0_1620;
const SMOKE_W: u32 = 0x01E0_19A0;
const SMOKE_V: u32 = 0x01E0_1D30;
const TEX: u32 = D + 0x34F0;
const TIMER: u32 = D + 0x3750;
const COLOUR: u32 = D + 0x3754;
const NODE: u32 = D + 0x377C;
const COUNTER: u32 = D + 0x3798;
const BYTES: u32 = D + 0x379A;

/// The entry 0x1E024D0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x375C, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3B10);
    l2::sine_table(fx, D + 0x399C);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3796, param);
    fx.mem.set32(TIMER, 0x153);
    for (k, node) in [0x1C, 0x3A, 0x40, 0x47, 0x4E].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(k, 0x01E0_25D8);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..8u32 {
        let v = fx.rand_from(k, 0x01E0_260C);
        fx.mem.set16(D + 0xC0 + 2 * k, v % 4);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    if fx.mem.i16(COUNTER) == 1 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE);
        fx.node_position(h, slot, sp + 0xA0);
        for k in 0..9u32 {
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
                fx.mem.write_u32(D + 0x3560 + 0x10 * k + 4 * c, w);
            }
        }
    }
    let slot = fx.mem.i16(NODE);
    spawn::node_history(fx, D + 0x35F0, 0xA, slot);
    for k in 0..TASKS {
        let s = D + 0x40 * k;
        let t = s + 0x2F0;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == MOTE {
            mote(fx, t);
        }
        if fx.mem.read_u32(t + 0x24) == SMOKE_W {
            smoke(fx, t, s, false);
        }
        if fx.mem.read_u32(t + 0x24) == SMOKE_V {
            smoke(fx, t, s, true);
        }
        // 0x1E020C0 / 0x1E02230 / 0x1E022F0 / 0x1E02340: never spawned.
        debug_assert!(!matches!(fx.mem.read_u32(t + 0x24), 0x01E0_20C0 | 0x01E0_2230 | 0x01E0_22F0 | 0x01E0_2340 | 0x01E0_1990));
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    // 0x1F71A0's frame below the entry's (its saved ra, the entry's s0).
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_27FC), (0, 0)]);
    // The entry's s0: the live tasks counted in the draw loop.
    let mut s0 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2F0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        s0 += 1;
        if fx.mem.read_u32(t + 0x24) == MOTE {
            cell(fx, t, sp + 0x60);
            fx.mem.set16(D + 0xB8, 3);
            let v = fx.rand_from(s0, 0x01E0_2870);
            fx.mem.set16(D + 0xBE, (v >> 4) % 4);
            let v = fx.rand_from(s0, 0x01E0_2890);
            fx.mem.set16(D + 0xC0, (v >> 4) % 4);
            quads4::ring_facing_i(fx, uv::uv_rect, D, t, 0, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (f, r, sites) in [(SMOKE_W, 0x80u32, [0x01E0_2A04u32, 0x01E0_2A24]), (SMOKE_V, 0x90, [0x01E0_2B3C, 0x01E0_2B5C])] {
            if fx.mem.read_u32(t + 0x24) != f {
                continue;
            }
            centre(fx, t);
            cell(fx, t, sp + r);
            if fx.mem.i16(t + 0x32) >= 4 {
                fx.mem.set16(D + 0xB8, 3);
                let v = fx.rand_from(s0, sites[0]);
                fx.mem.set16(D + 0xBE, (v >> 4) % 4);
                let v = fx.rand_from(s0, sites[1]);
                fx.mem.set16(D + 0xC0, (v >> 4) % 4);
                centre(fx, t);
                quads4::ring_at_m(fx, uv::uv_rect, D, t, 0x20, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x3C => 1,
        0x140 => 4,
        0x12C => 2,
        0x47..=0x126 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The task's i16 position as floats into D+0x20.
pub(crate) fn centre(fx: &mut Fx, t: u32) {
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(D + 0x20 + 4 * k, v);
    }
}

/// Texture (5, 2) and the atlas cell 0x14 / +0x34 (0x106960 into `r`)
/// copied to D+0x94..+0xA3.
pub(crate) fn cell(fx: &mut Fx, t: u32, r: u32) {
    let w = fx.mem.read_u32(TEX + 8);
    fx.mem.write_u32(D + 0x88, w);
    let sub = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// rand % m (HI) from a step's `jal` at `site` (rand's frame: s0 = `t`).
pub(crate) fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// 0x1A2B80 (float to integer) from `site` with its frame's residue.
pub(crate) fn fix(fx: &mut Fx, v: f32, site: u32) -> i32 {
    fx.fix_residue(v, site + 8);
    ee::fixsfsi(v)
}

/// 0x1E014D0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x40: ra, the entry's s1 = D / s0 = 0).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, 0x01E0_2648), (0x10, u64::from(D)), (0, 0)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    for (from, steps) in [(0x47, &[SMOKE_V, MOTE][..]), (0x43, &[SMOKE_W][..])] {
        let c = fx.mem.i16(COUNTER);
        if (from..0x113).contains(&c) {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODE);
            fx.node_position(h, slot, sp + 0x30);
            for k in 0..3 {
                let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
                fx.mem.write_u32(D + 0x250 + 4 * k, w);
            }
            for &f in steps {
                spawn(fx, f);
            }
        }
    }
    fx.leave(0x40);
}

/// 0x1E02C80 (b566 0x1E02E70) (D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour D+0x3754 (twice), +0x30 = D+0x3794; 1
/// when one was free.
pub(crate) fn spawn(fx: &mut Fx, step: u32) -> i32 {
    fx.enter(0x60);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x326 + 0x40 * k) == 0) {
        Some(k) => {
            let t = D + 0x2F0 + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            let a = fx.mem.i16(D + 0x3794);
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

/// 0x1E01620 (D, task): a red mote ring (frame 0x60: the entry's s1 = D,
/// s0 = 0): size 40 growing by 1 (and 1 / 3 more), cell frame random 0..1
/// then counting to 5 on odd ages, radius growing from 0 by 5..9, row
/// 0..24, life 26..35, +0xC 0 (x step on odd ages); fades in by 0x60 under
/// 0x61, out by 9 (from update 300 always out); its z drifts towards the
/// target by the distance / 25 (/ 45 from age 24); random angles +0x10 /
/// +0x14 / +0x18.
fn mote(fx: &mut Fx, t: u32) {
    let s = MOTE;
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x20, 0x01E0_26E4), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x28);
        let v = fx.rand_from(t, s + 0x30);
        fx.mem.set16(t + 0x34, (v >> 4) % 2);
        l2::rgb(fx, t + 0x28, 0x70, 0x10, 0x10);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, s + 0x60, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, s + 0x7C, 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rmod(fx, t, s + 0x9C, 0x19);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, s + 0xB8, 0xA);
        fx.mem.set16(t + 6, v + 0x1A);
        fx.mem.set16(t + 0xC, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(COUNTER) < 0x12C && fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x61 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x60));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x30);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x40);
    let dz = ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48));
    fx.mem.setf(sp + 0x38, dz);
    let (q, site, grow) = if fx.mem.i16(t + 0x32) < 0x18 { (25.0, s + 0x1E4, 1) } else { (45.0, s + 0x22C, 3) };
    let dz = ee::div(fx.mem.f(sp + 0x38), q);
    fx.mem.setf(sp + 0x38, dz);
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), dz);
    let z = fix(fx, z, site);
    fx.mem.set16(t + 4, z);
    let g = fx.mem.i16(t + 0x38);
    fx.mem.set16(t + 0x38, g + grow);
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let x = fx.mem.i16(t) + fx.mem.i16(t + 0xC);
        fx.mem.set16(t, x);
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
        let g = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, g + 1);
    }
    angles(fx, t, s + 0x2E8);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x60);
}

/// Random angles +0x10 / +0x14 / +0x18 (rand % 360 from the `jal`s at
/// `at`, + 0x1C, + 0x38).
pub(crate) fn angles(fx: &mut Fx, t: u32, at: u32) {
    for (o, k) in [(0x10u32, 0u32), (0x14, 0x1C), (0x18, 0x38)] {
        let v = rmod(fx, t, at + k, 0x168);
        fx.mem.set32(t + o, v);
    }
}

/// 0x1E019A0 (`v` false) / 0x1E01D30 (`v` true) (D, task): purple smoke
/// (frame 0x70: the entry's s2 = the slot `s`, s1 = D, s0 = 0): size
/// 12..23 growing by 2 (under 60) and 1 on ages of 3 k + 1 (0x1E01D30: odd
/// ages), cell frame random 0..1 then counting to 5 on odd ages, the
/// radius +0x30 kept in +0xA and growing from 0 by 3..7, row 0..34, life
/// 10..19 (32..56), +0xC the x distance of nodes 0x3A - 0x1C; fades in by
/// 0x80, out by 7 before update 300, by 9 after it; its z drifts towards
/// the target by the distance / 100 (/ 40); random angles.
fn smoke(fx: &mut Fx, t: u32, s: u32, v: bool) {
    let f = if v { SMOKE_V } else { SMOKE_W };
    let sp = fx.enter(0x70);
    let ra = if v { 0x01E0_2728 } else { 0x01E0_2708 };
    fx.saved(sp, &[(0x30, ra), (0x20, u64::from(s)), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        let r = rmod(fx, t, f + 0x28, 0xC);
        fx.mem.set16(t + 0x38, r + 0xC);
        let r = fx.rand_from(t, f + 0x50);
        fx.mem.set16(t + 0x34, (r >> 4) % 2);
        l2::rgb(fx, t + 0x28, 0x80, 0x20, 0xFF);
        let r = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, r);
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, t, f + 0x88, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rmod(fx, t, f + 0xA4, 5);
        fx.mem.set16(t + 0xE, r + 3);
        let r = rmod(fx, t, f + 0xC4, 0x23);
        fx.mem.set16(t + 8, r);
        let life = if v { rmod(fx, t, f + 0xE0, 0x19) + 0x20 } else { rmod(fx, t, f + 0xE0, 0xA) + 0xA };
        fx.mem.set16(t + 6, life);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE);
        fx.node_position(h, slot, sp + 0x40);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE + 2);
        fx.node_position(h, slot, sp + 0x50);
        let dx = ee::sub(fx.mem.f(sp + 0x50), fx.mem.f(sp + 0x40));
        let dx = fix(fx, dx, f + 0x13C);
        fx.mem.set16(t + 0xC, dx);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x2B);
        if fx.mem.i16(COUNTER) < 0x12C && a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x80));
        }
    } else if fx.mem.i16(COUNTER) < 0x12C {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 9 {
            fx.mem.write_u8(t + 0x2B, a - 7);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(COUNTER) >= 0x12D {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x2B, a - 9);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let g = fx.mem.i16(t + 0x38);
        if g < 0x3C {
            fx.mem.set16(t + 0x38, g + 2);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x40);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x50);
    if fx.mem.i16(t + 0x32) != 0 {
        let dz = ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(sp + 0x58));
        fx.mem.setf(sp + 0x48, dz);
        let dz = ee::div(dz, if v { 40.0 } else { 100.0 });
        fx.mem.setf(sp + 0x48, dz);
        let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), dz);
        let z = fix(fx, z, f + 0x288);
        fx.mem.set16(t + 4, z);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 5 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    let age = fx.mem.i16(t + 0x32);
    let grow = if v { age % 2 == 1 } else { fx.div(age, 3).1 == 1 };
    if grow {
        let g = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, g + 1);
    }
    angles(fx, t, f + 0x310);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x70);
}
