//! BMODULE b731 (card 51), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03CA0, data area 0x1ED0000) [F] (helper A23, 3
//! Nov 2026): b743's program family (cards 84 / 192) on a pool of 200
//! tasks (0x70 bytes at +0x2A0, alive +0x58, step +0x44 through `jalr`;
//! spawned by 0x1E04770 with the colour +0xB974 and the slot +0xBC9A into
//! +0x50), drawn with the second library (`uv_rect`, `ring_facing_flat_z`,
//! `ring_quads`, `arc_strips_flat`, `quads4::sparks`, `tubetrail::tube`)
//! and the record's copy of `quad_b_twice` (0x1E05440, word-identical).
//! The emitter 0x1E01C10 counts +0xBCA2: on update 1 the stage light
//! (0x1E03A10, down to 0x40 to age 210) and a frame tint (0x1E03B00); on
//! update 0x58 two arcs riding the attacker's nodes 0x1D / 0x34
//! (0x1E033A0); on 0x5F..0xB4 at the attacker's node 0x44 a shrinking mote
//! ring (0x1E02A00), a spark ring (0x1E031E0) and a closing spark swirl
//! (0x1E03520, `quads4::sparks`); on 0xA5 five orbs (0x1E01F40) fanned
//! by slot (-72 / 0 / 72 / 144 / 216 + rand % 144) with their 70-point
//! tubes (0x1E02EA0); on 0xC6 seven spark rings (0x1E03840). The orbs
//! swell and lift from age 34, dropping spark rings, and at age 53 burst
//! into motes (0x1E02670 / 0x1E02BB0), spark rings (0x1E02800), an arc
//! (0x1E02D60) and an undrawn swirl (0x1E036C0). Returns 1 on update
//! 0xCF, 3 on 0xF5, 6 on 0xD3..0xF0, 8 once the 298-update timer +0xB970
//! has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, spawn::history, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
/// The step functions a task holds (+0x44), as the record's addresses.
const ORB: u32 = 0x01E0_1F40;
const MOTE: u32 = 0x01E0_2670;
const SPARKS: u32 = 0x01E0_2800;
const MOTE_IN: u32 = 0x01E0_2A00;
const MOTE_W: u32 = 0x01E0_2BB0;
const ARC: u32 = 0x01E0_2D60;
const TUBE: u32 = 0x01E0_2EA0;
const SPARKS_B: u32 = 0x01E0_31E0;
const ARC_B: u32 = 0x01E0_33A0;
const SWIRL: u32 = 0x01E0_3520;
const SWIRL_OUT: u32 = 0x01E0_36C0;
const SPARKS_C: u32 = 0x01E0_3840;
const LIGHT: u32 = 0x01E0_3A10;
const FRAME: u32 = 0x01E0_3B00;
/// The return address of the entry's `jalr` into a step (0x1E03FA0 + 8).
const TASK_RA: u64 = 0x01E0_3FA8;
const SINES: u32 = D + 0xBEA4;
const CENTRES: u32 = D + 0xB1D0;
const TEX: u32 = D + 0xB950;
const TIMER: u32 = D + 0xB970;
const COLOUR: u32 = D + 0xB974;
const RADII: u32 = D + 0xB98C;
const NODE: u32 = D + 0xBB7C;
const ANGLES: u32 = D + 0xBB9C;
const SLOT: u32 = D + 0xBC9A;
const NEXT: u32 = D + 0xBC9C;
const YAW: u32 = D + 0xBC9E;
const COUNTER: u32 = D + 0xBCA2;
const BYTES: u32 = D + 0xBCA4;
const BYTES_B: u32 = D + 0xBDA4;

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// rand (the caller's s0 = `s0`) from `ra`, mod `m` (truncated).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn odd(n: i32) -> bool {
    n % 2 == 1
}

fn copy3(fx: &mut Fx, from: u32, to: u32) {
    for o in 0..3 {
        let w = fx.mem.read_u32(from + 4 * o);
        fx.mem.write_u32(to + 4 * o, w);
    }
}

/// The alpha byte `a` up by `up.1` while below `up.0` while `rising`, else
/// down by `down.1` while at least `down.0`, else the task ends.
pub(crate) fn fade(fx: &mut Fx, t: u32, a: u32, rising: bool, up: (u8, u8), down: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if rising {
        if v < up.0 {
            fx.mem.write_u8(t + a, v.wrapping_add(up.1));
        }
    } else if v >= down.0 {
        fx.mem.write_u8(t + a, v.wrapping_sub(down.1));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// Alpha `hold` while the age is below +6, then down by `down.1` while at
/// least `down.0`, else the task ends.
pub(crate) fn hold_fade(fx: &mut Fx, t: u32, hold: u8, down: (u8, u8)) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, hold);
    } else {
        fade(fx, t, 0x63, false, (0, 0), down);
    }
}

/// White on the odd ages, light grey on the even.
pub(crate) fn flicker(fx: &mut Fx, t: u32) {
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    } else {
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
    }
}

/// The entry 0x1E03CA0 (a0 = D, a1 = param; s0 = D, s1 = 0, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x100);
    fx.mem.write_u32(D + 0xB97C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x100);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xC010);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xBCA0, param);
    fx.mem.set32(TIMER, 0x12A);
    for (at, n, clear) in [(NODE, 0x44, 1), (D + 0xBB7E, 0x1D, 0), (D + 0xBB80, 0x34, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(at, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xBB8A, r);
    for k in 0xAu32..0x10 {
        let tex = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x260 + 4 * k, tex);
    }
    let tex = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, tex);
    for k in 0..3u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xF0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xF4)), 180.0);
    fx.fix_residue(v, 0x01E0_3E44);
    fx.mem.set16(YAW, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3E74, 0x1E);
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_3ECC);
        fx.mem.write_u8(BYTES + k, v.wrapping_rem(4) as u8);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3F24, 0x28);
        fx.mem.write_u8(BYTES_B + k, (v - 0x14) as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        // The steps' prologues: the entry's s0 = D, s1 = 0, s2 = the slot,
        // s3 = k.
        let frame = match f {
            ORB => 0x40,
            TUBE => 0x60,
            ARC_B => 0x30,
            FRAME => 0x80,
            MOTE | SPARKS | MOTE_IN | MOTE_W | ARC | SPARKS_B | SWIRL | SWIRL_OUT | SPARKS_C | LIGHT => 0x20,
            _ => panic!("b731: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match frame {
            0x40 => fx.saved(s, &[(0x30, TASK_RA), (0x20, u64::from(slot)), (0x10, 0), (0, u64::from(D))]),
            0x60 => fx.saved(s, &[(0x40, TASK_RA), (0x30, u64::from(k)), (0x20, u64::from(slot)), (0x10, 0), (0, u64::from(D))]),
            0x30 => fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]),
            _ => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]),
        }
        match f {
            ORB => orb(fx, t, k),
            MOTE => mote(fx, t, MOTE, &MOTE_A),
            MOTE_IN => mote(fx, t, MOTE_IN, &MOTE_I),
            MOTE_W => mote(fx, t, MOTE_W, &MOTE_WIDE),
            SPARKS => sparks(fx, t, SPARKS, &SPARKS_RED),
            SPARKS_B => sparks(fx, t, SPARKS_B, &SPARKS_GREY),
            SPARKS_C => sparks_c(fx, t, SPARKS_C),
            ARC => arc(fx, t),
            TUBE => tube_step(fx, s, t),
            ARC_B => arc_b(fx, t),
            SWIRL => swirl(fx, t, SWIRL, false),
            SWIRL_OUT => swirl(fx, t, SWIRL_OUT, true),
            LIGHT => light(fx, t),
            _ => frame_quad(fx, s, t),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        draw(fx, sp, t);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xCF => 1,
        0xF5 => 3,
        0xD3..=0xF0 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The draw's head: texture `tex` into D+0x88, the atlas cell `id` frame
/// +0x56 (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, tex: u32, id: i32, r: u32) {
    let w = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, w);
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// An orb's point (the slot +0x50's centre plus its radius turned by its
/// angle in x / y) into `out`.
fn orb_point(fx: &mut Fx, t: u32, out: u32) {
    for (o, cos) in [(0u32, 1), (4, 0)] {
        let i = fx.mem.i16(t + 0x50) as u32;
        let a = fx.mem.i16(ANGLES + 2 * i);
        let s = l2::sin_deg(fx, SINES, a, cos);
        let v = ee::add(fx.mem.f(CENTRES + 0x10 * i + o), ee::mul(fx.mem.f(RADII + 4 * i), s));
        fx.mem.setf(out + o, v);
    }
    let i = fx.mem.i16(t + 0x50) as u32;
    let z = fx.mem.f(CENTRES + 0x10 * i + 8);
    fx.mem.setf(out + 8, z);
}

/// The entry's draw of one live task by its step (s0 = D).
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    let motes = |fx: &mut Fx, tex: u32, r: u32, mode: i32| {
        cell(fx, t, tex, 0x14, r);
        fx.mem.set16(D + 0xB8, 6);
        let v = fx.mem.i16(t + 0xA);
        fx.mem.set16(D + 0xC0, v);
        copy3(fx, t + 0x10, D + 0x20);
        quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, mode, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    };
    let rings = |fx: &mut Fx, tex: u32, id: i32, r: u32, n: i32, layout: i32| {
        cell(fx, t, tex, id, r);
        fx.mem.set16(D + 0xB8, n);
        fx.mem.set16(D + 0xB0, layout);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
        fx.mem.set16(D + 0xB8, 0);
        fx.mem.set16(D + 0xB0, 0);
    };
    if f == ORB {
        cell(fx, t, TEX, 0x14, sp + 0x70);
        fx.mem.set16(D + 0xBE, 0);
        orb_point(fx, t, sp + 0x60);
        billboard::quad_b_twice(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
    }
    if f == MOTE {
        motes(fx, TEX, sp + 0x80, 0x20);
    }
    if f == SPARKS {
        rings(fx, D + 0x29C, 0x9B, sp + 0x90, 0xA, 1);
    }
    if f == ARC {
        fx.mem.set16(D + 0xB8, 0x18);
        billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == MOTE_IN {
        motes(fx, TEX, sp + 0xA0, 0x20);
    }
    if f == MOTE_W {
        motes(fx, TEX + 4, sp + 0xB0, 0x10);
    }
    if f == TUBE {
        cell(fx, t, TEX, 0x14, sp + 0xC0);
        let v = fx.mem.i16(D + 0x96) + 0xF;
        fx.mem.set16(D + 0x96, v);
        fx.mem.set16(D + 0x98, 0x1F);
        fx.mem.set16(D + 0x9A, 1);
        let h = fx.self_handle();
        unit_position_saving(fx, h, sp + 0x60, u64::from(D), 0x01E0_4460);
        for o in [0x20u32, 0x24, 0x28] {
            fx.mem.write_u32(D + o, 0);
        }
        fx.mem.set16(D + 0xBE, 2);
        let i = fx.mem.i16(t + 0x50) as u32;
        tubetrail::tube(fx, D, t, D + 0x5A50 + 0x460 * i, 0);
    }
    if f == SPARKS_B {
        rings(fx, TEX, 0x14, sp + 0xD0, 5, 1);
    }
    if f == ARC_B {
        fx.mem.set16(D + 0xB8, 0x18);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE);
        node_position_saving(fx, h, s, sp + 0x60, u64::from(D), None, 0x01E0_4570);
        billboard::arc_strips_flat(fx, D, t, 0x21, sp + 0x60);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == SWIRL {
        let w = fx.mem.read_u32(TEX + 8);
        fx.mem.write_u32(D + 0x88, w);
        for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x1F), (0x9A, 0x7F)] {
            fx.mem.set16(D + o, v);
        }
        fx.mem.set16(D + 0xB8, 0);
        copy3(fx, t + 0x10, D + 0x20);
        fx.mem.set16(D + 0xB8, 0x18);
        quads4::sparks(fx, D, t, 0x20, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == SPARKS_C {
        rings(fx, TEX, 0x14, sp + 0xE0, 5, 0);
    }
}

/// 0x1E04770(D, step, 0): the first free of the 200 tasks cleared and set
/// off at D+0x250 (as i16 and its xyz at +0x10) with the colour +0xB974
/// (+0x60, +0x64), +0x50 = +0xBC9A; 1 when one was free. `r` the caller's
/// s0..s3 (s4 the runner's, 0 [H]).
fn spawn(fx: &mut Fx, step: u32, r: [u64; 4], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, r[3]), (0x20, r[2]), (0x10, r[1]), (0, r[0])]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for o in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            copy3(fx, D + 0x250, t + 0x10);
            let s = fx.mem.i16(SLOT);
            fx.mem.set16(t + 0x50, s);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// +0xBC9C up by one, back to 0 at 120.
fn next_slot(fx: &mut Fx) {
    let n = fx.mem.i16(NEXT) + 1;
    fx.mem.set16(NEXT, n);
    if fx.mem.i16(NEXT) >= 0x78 {
        fx.mem.set16(NEXT, 0);
    }
}

/// 0x1E01C10(D, 0) (frame 0x30: ra, the entry's s1 = 0, s0 = D): the
/// update counter +0xBCA2 and the emitters.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3F78), (0x10, 0), (0, u64::from(D))]);
    let p = param as i64 as u64;
    let mut s0 = u64::from(D);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let node = |fx: &mut Fx, s0: u64, ra: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE);
        node_position_saving(fx, h, s, D + 0x250, s0, Some(u64::from(D)), ra);
    };
    if fx.mem.i16(COUNTER) == 0xA5 {
        for k in 0..5u64 {
            s0 = k;
            node(fx, s0, 0x01E0_1C80);
            let v = fx.mem.i16(NEXT);
            fx.mem.set16(SLOT, v);
            spawn(fx, ORB, [s0, u64::from(D), p, 0], 0x01E0_1CAC);
            spawn(fx, TUBE, [s0, u64::from(D), p, 0], 0x01E0_1CC0);
            next_slot(fx);
        }
        s0 = 5;
    }
    let r = [s0, u64::from(D), p, 0];
    if fx.mem.i16(COUNTER) == 0xC6 {
        node(fx, s0, 0x01E0_1D60);
        let mut ra = 0x01E0_1D74u32;
        for _ in 0..7 {
            spawn(fx, SPARKS_C, r, ra);
            ra += 0x14;
        }
    }
    if fx.mem.i16(COUNTER) == 0x58 {
        fx.mem.set16(SLOT, 1);
        spawn(fx, ARC_B, r, 0x01E0_1E28);
        fx.mem.set16(SLOT, 2);
        spawn(fx, ARC_B, r, 0x01E0_1E4C);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x5F..0xB5).contains(&c) {
        node(fx, s0, 0x01E0_1E90);
        spawn(fx, MOTE_IN, r, 0x01E0_1EA4);
        spawn(fx, SPARKS_B, r, 0x01E0_1EB8);
        spawn(fx, SWIRL, r, 0x01E0_1ECC);
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, LIGHT, r, 0x01E0_1EF8);
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, FRAME, r, 0x01E0_1F24);
    }
    fx.leave(0x30);
}

/// 0x1E01F40 (frame 0x40: s1 = D, s2 = the slot index): an orb at its
/// slot's centre (+0x10.. into +0xB1D0 + 0x10 k), its angle by slot (-72 /
/// 0 / 72 / 144 / 216 + rand % 144), its radius +0x34 (20..69) sin(+0x5A);
/// size 10, flickering; in by 0x2A for 60 updates, then out by 14; at age
/// 53 a burst at its point; over ages 34..52 it swells by 5 to 50, +0x5A
/// up by 10 to 180, its centre 8.5 along z (- when the attacker's yaw is
/// non-zero) and 0.65 down, a spark ring at its point until age 51.
fn orb(fx: &mut Fx, t: u32, k: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xA);
        fx.mem.set16(t + 0xA, 0xA);
        let v = rmod(fx, t, 0x01E0_1F78, 0x32);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0x14));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        for o in 0..3 {
            let i = fx.mem.i16(t + 0x50) as u32;
            let w = fx.mem.read_u32(t + 0x10 + 4 * o);
            fx.mem.write_u32(CENTRES + 0x10 * i + 4 * o, w);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (slot, ra, b) in [(0, 0x01E0_203Cu32, -0x48), (1, 0x01E0_207C, 0), (2, 0x01E0_20BC, 0x48), (3, 0x01E0_20FC, 0x90), (4, 0x01E0_213C, 0xD8)] {
            if fx.mem.i16(t + 0x50) == slot {
                let v = rmod(fx, t, ra, 0x90) + b;
                let i = fx.mem.i16(t + 0x50) as u32;
                fx.mem.set16(ANGLES + 2 * i, v);
            }
        }
        let i = fx.mem.i16(t + 0x50) as u32;
        fx.mem.write_u32(RADII + 4 * i, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x3C;
    if rising {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
        }
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0x10, 0xE));
        let a = fx.mem.read_u8(t + 0x67);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x67, a - 0xE);
        }
    }
    flicker(fx, t);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2258);
    let i = u64::from(fx.mem.i16(t + 0x50) as u32);
    if fx.mem.i16(t + 0x54) == 0x35 {
        orb_point(fx, t, D + 0x250);
        let r = [u64::from(t), u64::from(D), i, u64::from(k)];
        let mut ra = 0x01E0_2350u32;
        for f in [MOTE, MOTE, MOTE, SPARKS, SPARKS, ARC, MOTE_W, MOTE_W, MOTE_W, SWIRL_OUT] {
            spawn(fx, f, r, ra);
            ra += 0x14;
        }
    }
    age(fx, t);
    let a = fx.mem.i16(t + 0x54);
    if (0x22..0x35).contains(&a) {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s < 0x32 {
                fx.mem.set16(t + o, s + 5);
            }
        }
        let s = fx.mem.i16(t + 0x5A);
        if s < 0xB4 {
            fx.mem.set16(t + 0x5A, s + 0xA);
        }
        let i = fx.mem.i16(t + 0x50) as u32;
        let c = CENTRES + 0x10 * i;
        let z = fx.mem.f(c + 8);
        let z = if fx.mem.i16(YAW) != 0 { ee::sub(z, 8.5) } else { ee::add(z, 8.5) };
        fx.mem.setf(c + 8, z);
        let y = ee::sub(fx.mem.f(c + 4), f32::from_bits(0x3F26_6666));
        fx.mem.setf(c + 4, y);
        orb_point(fx, t, D + 0x250);
        if fx.mem.i16(t + 0x54) < 0x33 {
            let i = u64::from(fx.mem.i16(t + 0x50) as u32);
            spawn(fx, SPARKS_C, [u64::from(t), u64::from(D), i, u64::from(k)], 0x01E0_2618);
        }
    }
    let a = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let r = ee::mul(fx.mem.f(t + 0x34), s);
    let i = fx.mem.i16(t + 0x50) as u32;
    fx.mem.setf(RADII + 4 * i, r);
}

/// A mote ring's constants (b743's 0x1E02540 and its two variants).
pub(crate) struct MoteCfg {
    pub size: u32,
    pub grey: i32,
    pub radius: (i32, i32),
    pub growth: (i32, i32),
    /// The hold: rand % 5 + 3 (false) or rand % 4 + 4 (true).
    pub hold4: bool,
    pub alpha: u8,
    pub down: (u8, u8),
    /// The radius closes in by the growth while above 0 (0x1E02A00).
    pub closing: bool,
    /// The size grows by 11 a update (0x1E02BB0).
    pub widening: bool,
}

pub(crate) const MOTE_A: MoteCfg = MoteCfg { size: 0x4220_0000, grey: 0x80, radius: (0x32, 0xA), growth: (0x1E, 0x10), hold4: false, alpha: 0x80, down: (0xA, 8), closing: false, widening: false };
const MOTE_I: MoteCfg = MoteCfg { size: 0x4120_0000, grey: 0x80, radius: (0x64, 0xC8), growth: (0x1E, 0x24), hold4: false, alpha: 0x80, down: (0xA, 8), closing: true, widening: false };
pub(crate) const MOTE_WIDE: MoteCfg = MoteCfg { size: 0x4220_0000, grey: 0xFF, radius: (0x32, 0xA), growth: (0x14, 0xC), hold4: true, alpha: 0xC0, down: (0xD, 0xB), closing: false, widening: true };

/// A mote ring step at record address `at` (ring_facing_flat_z of 6 from
/// the bytes +0xBCA4): cell rand % 7, turned rand % 360, orientation +0xA
/// rand % 8, +0xC the spawn's slot.
pub(crate) fn mote(fx: &mut Fx, t: u32, at: u32, c: &MoteCfg) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, c.size);
        let v = rmod(fx, t, at + 0x28, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, c.grey, c.grey, c.grey);
        let v = rmod(fx, t, at + 0x54, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, at + 0x70);
        fx.mem.set16(t + 0xA, v.wrapping_rem(8));
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        let v = rmod(fx, t, at + 0x94, c.radius.0);
        fx.mem.set16(t + 0x50, v + c.radius.1);
        let v = rmod(fx, t, at + 0xB4, c.growth.0);
        fx.mem.set16(t + 0xE, v + c.growth.1);
        let v = rmod(fx, t, at + 0xD4, 0x23);
        fx.mem.set16(t + 8, v);
        let h = if c.hold4 { fx.rand_from(t, at + 0xF0).wrapping_rem(4) + 4 } else { rmod(fx, t, at + 0xF0, 5) + 3 };
        fx.mem.set16(t + 6, h);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_fade(fx, t, c.alpha, c.down);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50);
        let g = fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, if c.closing && r > 0 { r - g } else { r + g });
    }
    if c.widening {
        let v = ee::add(fx.mem.f(t + 0x48), 11.0);
        fx.mem.setf(t + 0x48, v);
    }
    age(fx, t);
}

/// A spark ring's constants (b743's 0x1E026D0 and 0x1E031E0).
pub(crate) struct SparksCfg {
    /// The size: rand % m + b, or a constant (m = 0).
    pub size: (i32, i32, u32),
    pub colour: (i32, i32, i32),
    pub cell: bool,
    pub radius: (i32, i32),
}

const SPARKS_RED: SparksCfg = SparksCfg { size: (0x1E, 0x19, 0), colour: (0xFF, 0x80, 0x80), cell: true, radius: (0x15E, 0x12C) };
const SPARKS_GREY: SparksCfg = SparksCfg { size: (0, 0, 0x3F80_0000), colour: (0x80, 0x80, 0x80), cell: false, radius: (0x15E, 0x12C) };

/// A spark ring step at record address `at`: turned by rand % 0xFFFF on
/// each axis, radius growing by 1..3, row 1..8; alpha 0x60 for 2..4
/// updates, then out by 11.
pub(crate) fn sparks(fx: &mut Fx, t: u32, at: u32, c: &SparksCfg) {
    if fx.mem.i16(t + 0x58) == 1 {
        // 0x1E031E0's rand calls sit 0x3C later than 0x1E02800's.
        let o = if c.cell { 0 } else { 0x3C };
        if c.cell {
            let v = rmod(fx, t, at + 0x24, c.size.0);
            fx.mem.setf(t + 0x48, ee::from_i32(v + c.size.1));
            let v = fx.rand_from(t, at + 0x50);
            fx.mem.set16(t + 0x56, v.wrapping_rem(4));
        } else {
            fx.mem.write_u32(t + 0x48, c.size.2);
            fx.mem.set16(t + 0x56, 4);
        }
        l2::rgb(fx, t + 0x60, c.colour.0, c.colour.1, c.colour.2);
        let at = at.wrapping_sub(o);
        for (k, ra) in [(0x20u32, at + 0x84), (0x24, at + 0xA0), (0x28, at + 0xBC)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + k, v);
        }
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, at + 0xE0, c.radius.0);
        fx.mem.set16(t + 0x50, v + c.radius.1);
        let v = rmod(fx, t, at + 0x100, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, at + 0x11C, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, at + 0x13C);
        fx.mem.set16(t + 8, v.wrapping_rem(8) + 1);
        let v = rmod(fx, t, at + 0x15C, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_fade(fx, t, 0x60, (0xD, 0xB));
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, v);
    age(fx, t);
}

/// 0x1E03840 (record address `at`): a grey spark ring of size 10 growing by 2, cell 4, turned by
/// rand % 0xFFFF on each axis, radius 0..99 growing by 10..29, row 1..8;
/// alpha 0x80 for 2..4 updates, then out by 11.
pub(crate) fn sparks_c(fx: &mut Fx, t: u32, at: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        for (o, ra) in [(0x20u32, at + 0x48), (0x24, at + 0x64), (0x28, at + 0x80)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, at + 0xA4, 0x64);
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, t, at + 0xC0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, at + 0xDC, 0x14);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = fx.rand_from(t, at + 0xFC);
        fx.mem.set16(t + 8, v.wrapping_rem(8) + 1);
        let v = rmod(fx, t, at + 0x11C, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_fade(fx, t, 0x80, (0xD, 0xB));
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, v);
    let v = ee::add(fx.mem.f(t + 0x48), 2.0);
    fx.mem.setf(t + 0x48, v);
    age(fx, t);
}

/// The arcs' outer flicker ((0xFF, 0x80, 0x80) / (0xCF, 0x70, 0x70)), the
/// camera's angles (0x205580 from `ra`, s1 = `s1`) and the spin.
fn arc_tail(fx: &mut Fx, t: u32, s1: u32, ra: u32) {
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
    } else {
        l2::rgb(fx, t + 0x64, 0xCF, 0x70, 0x70);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(s1), ra);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
}

/// 0x1E02D60: a white / red arc ring (arc_strips_flat of 24 at its point)
/// of inner radius 300; in by 0x40 for 2 updates, then out by 11; facing
/// the camera, spun by 0x4000 a update.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4396_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 2;
    fade(fx, t, 0x63, rising, (0x80, 0x40), (0xD, 0xB));
    arc_tail(fx, t, 0, 0x01E0_2E74);
    age(fx, t);
}

/// 0x1E033A0 (frame 0x30: s1 = D): an arc of inner radius 70 in pale grey
/// (0xDF), in by 12 below 0x80 until update 215, then out by 11, riding
/// the attacker's node +0xBB7C + 2 x slot (0x1D / 0x34).
fn arc_b(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x428C_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xDF, 0xDF, 0xDF);
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(COUNTER) < 0xD7;
    fade(fx, t, 0x63, rising, (0x80, 0xC), (0xD, 0xB));
    arc_tail(fx, t, D, 0x01E0_34C4);
    let h = fx.self_handle();
    let n = fx.mem.i16(t + 0x50) as u32;
    let s = fx.mem.i16(NODE + 2 * n);
    node_position_saving(fx, h, s, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_34F8);
    age(fx, t);
}

/// 0x1E02EA0 (frame 0x60, the point at sp + 0x50): an orb's tube: its
/// slot's orb point pushed into the 70-point history +0x5A50 + 0x460 k
/// (filled with it on the first update) until age 117, flickering white
/// to age 80, 20 points growing to 50, size 20 shrinking by 1.5 from age
/// 60 (the task ending below 0), cell 4; both alphas in by 0x2A to age 60.
fn tube_step(fx: &mut Fx, sp: u32, t: u32) {
    let p = sp + 0x50;
    orb_point(fx, t, p);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        let i = fx.mem.i16(t + 0x50) as u32;
        let trail = D + 0x5A50 + 0x460 * i;
        for j in 0..0x46u32 {
            copy3(fx, p, trail + 0x10 * j);
        }
        fx.mem.set16(t + 6, 0x14);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0x56, 4);
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x3C {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
        }
    } else {
        let s = fx.mem.f(t + 0x48);
        if crate::exe::ee_lt(s, 0.0) {
            fx.mem.set16(t + 0x58, 0);
        } else {
            fx.mem.setf(t + 0x48, ee::sub(s, 1.5));
        }
    }
    if fx.mem.i16(t + 0x54) < 0x53 {
        let v = fx.mem.i16(t + 0x5A) + 5;
        fx.mem.set16(t + 0x5A, v);
    }
    if fx.mem.i16(t + 0x54) < 0x75 {
        let i = fx.mem.i16(t + 0x50) as u32;
        history(fx, D + 0x5EA0 + 0x460 * i, 0x46, p);
    }
    if fx.mem.i16(t + 0x54) < 0x50 {
        flicker(fx, t);
    }
    if fx.mem.i16(t + 0x54) != 0 && fx.mem.i16(t + 6) < 0x32 {
        let v = fx.mem.i16(t + 6) + 1;
        fx.mem.set16(t + 6, v);
    }
    age(fx, t);
}

/// 0x1E03520 / 0x1E036C0 (record address `at`): a spark swirl (sparks round its point, inner
/// radius +0x48, outer +0x48 - 300) turned by rand % 0xFFFF on each axis,
/// +0x24 turning by 0x100 a update; 0x1E03520 dark (0x20) closing from
/// 1300 by 60 to 40, alpha 0x80 for 18 updates; 0x1E036C0 grey opening
/// from 20 by 300, alpha 0x80 for 10 updates; then out by 14.
pub(crate) fn swirl(fx: &mut Fx, t: u32, at: u32, opening: bool) {
    if fx.mem.i16(t + 0x58) == 1 {
        let (start, grey, div) = if opening { (20.0f32, 0x80, 2.0f32) } else { (1300.0, 0x20, 10.0) };
        fx.mem.setf(t + 0x48, start);
        fx.mem.setf(t + 0x4C, ee::sub(start, 58.0));
        l2::rgb(fx, t + 0x60, grey, grey, grey);
        l2::rgb(fx, t + 0x64, grey, grey, grey);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.write_u32(t + 0x34, 0x4416_0000);
        let v = ee::div(fx.mem.f(t + 0x34), div);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let o = if opening { 0x9C } else { 0xA0 };
        for (k, ra) in [(0x20u32, at + o), (0x24, at + o + 0x1C), (0x28, at + o + 0x38)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + k, v);
        }
    }
    let hold = if opening { 0xA } else { 0x12 };
    if fx.mem.i16(t + 0x54) < hold {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0x10, 0xE));
    }
    if opening {
        let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
        fx.mem.setf(t + 0x48, r);
        fx.mem.setf(t + 0x4C, ee::sub(r, 300.0));
    } else {
        let r = fx.mem.f(t + 0x48);
        if !crate::exe::ee_le(r, 40.0) {
            let d = fx.mem.f(t + 0x34);
            fx.mem.setf(t + 0x48, ee::sub(r, d));
        }
        let r = fx.mem.f(t + 0x48);
        fx.mem.setf(t + 0x4C, ee::sub(r, 300.0));
    }
    let v = fx.mem.read_u32(t + 0x24).wrapping_add(0x100);
    fx.mem.write_u32(t + 0x24, v);
    age(fx, t);
}

/// 0x1E03A10: the stage light +0x50 from 0x80 down by 10 to 0x40 until age
/// 210, then back up by 10 to 0x80; at the timer's 1 the light and the
/// task's end.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let l = fx.mem.i16(t + 0x50);
    if fx.mem.i16(t + 0x54) < 0xD2 {
        fx.mem.set16(t + 0x50, if l < 0x41 { 0x40 } else { l - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    age(fx, t);
    let l = fx.mem.read_u8(t + 0x50);
    fx.set_stage_light(i32::from(l));
    if fx.mem.read_u32(TIMER) == 1 {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E03B00 (frame 0x80, the descriptor at sp + 0x28): a full-window
/// frame quad (colour 0x80 / 0x60 / 0x70, the task's alpha, ALPHA 0x44)
/// at ST 0..1; in by 10 below 0x50 for 244 updates, then out by 9.
fn frame_quad(fx: &mut Fx, sp: u32, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x28;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.mem.read_u8(t + 0x63);
    for (k, v) in [0x80u8, 0x60, 0x70, a].into_iter().enumerate() {
        fx.mem.write_u8(d + 0x10 + k as u32, v);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, xy) in [(0x20u32, 0x6FF8u16), (0x30, 0x8FF8)] {
        fx.mem.write_u16(d + at, xy);
        fx.mem.write_u16(d + at + 2, xy);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    for (o, v) in [(0x18u32, 0u32), (0x1C, 0), (0x28, 0x3F80_0000), (0x2C, 0x3F80_0000)] {
        fx.mem.write_u32(d + o, v);
    }
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [0x226, 0x26]);
    // 0x207FC0's `sq ra` upper half (0) under 0x207EF0 (as b232).
    fx.mem.write_u64(sp - 0x28, 0);
    fx.submit(d, OT, 0);
    let rising = fx.mem.i16(t + 0x54) < 0xF4;
    fade(fx, t, 0x63, rising, (0x50, 0xA), (0xB, 9));
    age(fx, t);
}
