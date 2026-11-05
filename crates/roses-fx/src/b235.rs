//! BMODULE b235 (card 182), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03580, data area 0x1ED0000) [F] (helper A10, 31
//! Oct 2026): b065's program shape (b590's layout: 400 tasks of 0x70 bytes
//! at D+0x2A0, alive +0x58, step +0x44 through `jalr`, 0x1E03570 the idle
//! step), spawned by 0x1E03DC0 (b065's spawn), drawn with the second
//! library: smoke rings (7369422df7f8 at 0x1E00920,
//! `billboard::ring_quads`), arcs (e6dce326551d at 0x1E00ED0,
//! `billboard::arc_strips`), textured arcs (3a2db45c5a9a at 0x1E01860,
//! `quads4::arc_strips_tex`), light columns (792ca1ad9378 at 0x1E014F0,
//! `quads4::sprite_quads`) and single quads (add5d81dd3fe at 0x1E01230,
//! `billboard::quad`). Update 1: the stage dimmed to 0x40 (0x1E03200), a
//! frame flash (0x1E03310), camera slots 4 / 5 (0x1E02FE0) and four light
//! columns between the attacker's nodes 0x28 / 0x29; to update 11 dark
//! red arcs pulled along the same line (0x1E02370); updates 50 / 72 and
//! 61 / 82: thirteen white motes at node 0xA; 118..138 at the target (13
//! down) three pink smoke rings a update, textured purple arcs (three
//! kinds by the update's parity) and growing pink arcs (0x1E02560, also
//! to 130). Returns 1 on update 109, 4 on 180, 2 on 155, 6 on 120..149, 8
//! once the 185-update timer D+0xB210 has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const SMOKE: u32 = 0x01E0_2190;
const ARC: u32 = 0x01E0_2370;
const GROW: u32 = 0x01E0_2560;
const ARC_T: u32 = 0x01E0_26E0;
const COLUMN: u32 = 0x01E0_2B70;
const MOTE: u32 = 0x01E0_2E10;
const CAMERA: u32 = 0x01E0_2FE0;
const LIGHT: u32 = 0x01E0_3200;
const FLASH: u32 = 0x01E0_3310;
/// A step that only returns 0 (skipped by the entry).
const IDLE: u32 = 0x01E0_3570;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3938;
const TIMER: u32 = D + 0xB210;
const COLOUR: u32 = D + 0xB214;
const NODE_A: u32 = D + 0xB244;
const NODE_B: u32 = D + 0xB246;
const NODE_C: u32 = D + 0xB248;
const SPAWN_A: u32 = D + 0xB2A2;
const ARC_K: u32 = D + 0xB2A6;
const SPAWN_K: u32 = D + 0xB2B2;
const COUNTER: u32 = D + 0xB2BA;
const BYTES: u32 = D + 0xB2BC;
const BYTES_B: u32 = D + 0xB3BC;
const TAB: u32 = D + 0xB4BC;
const CAM: u32 = D + 0xB1A0;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E03580 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B8, param);
    fx.mem.set32(TIMER, 0xB9);
    for (k, node) in [0x28, 0x29, 0xA, 0x14, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE_A + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0xB1F0 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_3770);
    fx.mem.set16(D + 0xB2B6, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_37A0, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_37F4, 0x1E);
        fx.mem.set8(BYTES_B + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, k, 0x01E0_384C, 0x1E);
        fx.mem.set8(BYTES_B + k, v);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_389C);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 || f == IDLE {
            continue;
        }
        // The step's prologue: ra, the entry's s1 = D, s0 = 0.
        let frame = match f {
            ARC => 0x40,
            ARC_T | MOTE => 0x30,
            COLUMN | CAMERA => 0x50,
            FLASH => 0x90,
            _ => 0x20,
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        }
        match f {
            SMOKE => smoke(fx, t),
            ARC => arc(fx, t, s),
            GROW => grow(fx, t),
            ARC_T => arc_t(fx, t),
            COLUMN => column(fx, t, s),
            MOTE => mote(fx, t),
            CAMERA => camera(fx, t, s),
            LIGHT => light(fx, t),
            FLASH => flash(fx, t, s),
            _ => panic!("b235: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    // The entry's s0: the live tasks counted in the draw loop.
    let mut s0 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        s0 += 1;
        draw(fx, sp, t, s0);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x6D => 1,
        0xB4 => 4,
        0x9B => 2,
        0x78..=0x95 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    if f == SMOKE {
        tex(fx, D + 0xB1F0);
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x50, 0x14, c);
        fx.mem.set16(D + 0xB8, 6);
        let v = fx.rand_from(s0, 0x01E0_39E4);
        fx.mem.set16(D + 0xBE, v % 4);
        fx.mem.set16(D + 0xB0, 1);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES_B);
        fx.mem.set16(D + 0xB8, 0);
    }
    for g in [ARC, GROW] {
        if fx.mem.read_u32(t + 0x44) == g {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let f = fx.mem.read_u32(t + 0x44);
    if f == ARC_T {
        let kind = |fx: &mut Fx| fx.mem.i16(t + 0x50);
        if kind(fx) == 0 {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x60, 0x97, c);
            tex(fx, D + 0x29C);
        }
        if kind(fx) == 1 {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x9B, c);
            tex(fx, D + 0x29C);
        }
        if kind(fx) == 2 {
            tex(fx, D + 0xB1F4);
            fx.mem.set16(D + 0xBE, 0);
            let c = fx.mem.i16(t + 0x56);
            for (o, v) in [(0x94u32, c << 5), (0x96, 0), (0x98, 0x20), (0x9A, 0x40)] {
                fx.mem.set16(D + o, v);
            }
        }
        fx.mem.set16(D + 0xB8, 0);
        for k in 0..3 {
            let v = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(D + 0x20 + 4 * k, v);
        }
        fx.mem.set16(D + 0xB8, 0x24);
        if kind(fx) == 2 {
            fx.mem.set16(D + 0xB8, 6);
        }
        fx.mem.set16(D + 0xBE, 0);
        quads4::arc_strips_tex(fx, D, t, 0x20, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if fx.mem.read_u32(t + 0x44) == COLUMN {
        tex(fx, D + 0x29C);
        cell(fx, sp + 0x80, 0x9A, 0);
        fx.mem.set16(D + 0xBE, 0);
        quads4::sprite_quads(fx, D, t, 0, t + 0x10);
    }
    if fx.mem.read_u32(t + 0x44) == MOTE {
        tex(fx, D + 0xB1F8);
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x90, 0x14, c);
        fx.mem.set16(D + 0xBE, 0);
        billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
    }
}

/// 0x1E03DC0(D, step, arg): the first free task cleared and set off at
/// D+0x250 (i16 +0, float +0x10) with the colour D+0xB214 (twice), index
/// D+0xB2B2 (+0x50 / +0x52), angle D+0xB2A2 (+0x5A) (b065's spawn).
fn spawn(fx: &mut Fx, step: u32) {
    fx.enter(0x60);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        let t = D + 0x2A0 + 0x70 * k;
        fx.bzero(t, 0x70);
        fx.mem.set16(t + 0x58, 1);
        for o in [0x60u32, 0x64] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + o + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        for k in 0..3 {
            let w = fx.mem.read_u32(POS + 4 * k);
            fx.mem.write_u32(t + 0x10 + 4 * k, w);
        }
        let s = fx.mem.i16(SPAWN_K);
        fx.mem.set16(t + 0x50, s);
        let s = fx.mem.i16(SPAWN_K);
        fx.mem.set16(t + 0x52, s);
        let a = fx.mem.i16(SPAWN_A);
        fx.mem.set16(t + 0x5A, a);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

/// 0x1E01C70(D, 0): the update counter and the emitters (frame 0x30: ra,
/// the entry's s1 = D, s0 = 0).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_38F8), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 1 {
        spawn(fx, LIGHT);
    }
    if c(fx) == 1 {
        spawn(fx, FLASH);
    }
    if c(fx) == 1 {
        spawn(fx, CAMERA);
    }
    if c(fx) < 0xC {
        let k = fx.mem.i16(ARC_K);
        fx.mem.set16(SPAWN_K, k);
        spawn(fx, ARC);
        let k = fx.mem.i16(ARC_K) + 1;
        fx.mem.set16(ARC_K, k);
    }
    if c(fx) == 1 {
        for (a, k) in [(0, 0), (0x5A, 0), (0x2D, 1), (0x87, 1)] {
            fx.mem.set16(SPAWN_A, a);
            fx.mem.set16(SPAWN_K, k);
            spawn(fx, COLUMN);
        }
    }
    let at_target = |fx: &mut Fx| {
        let h = fx.target_handle();
        fx.unit_position(h, POS);
        let y = ee::sub(fx.mem.f(POS + 4), 13.0);
        fx.mem.setf(POS + 4, y);
    };
    if (0x76..0x8B).contains(&c(fx)) {
        at_target(fx);
        for _ in 1..4 {
            spawn(fx, SMOKE);
        }
        fx.mem.set16(SPAWN_K, 0);
        if c(fx) % 2 == 1 {
            spawn(fx, ARC_T);
        }
        fx.mem.set16(SPAWN_K, 1);
        if c(fx) % 2 == 1 {
            spawn(fx, ARC_T);
        }
        fx.mem.set16(SPAWN_K, 2);
        if c(fx) % 2 == 0 {
            spawn(fx, ARC_T);
        }
        spawn(fx, GROW);
    }
    if (0x76..0x83).contains(&c(fx)) {
        at_target(fx);
        spawn(fx, GROW);
    }
    for ups in [[0x32, 0x48], [0x3D, 0x52]] {
        if ups.contains(&c(fx)) {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODE_C);
            fx.node_position(h, slot, POS);
            let y = ee::sub(fx.mem.f(POS + 4), 1.5);
            fx.mem.setf(POS + 4, y);
            for _ in 0..0xD {
                spawn(fx, MOTE);
            }
        }
    }
    fx.leave(0x30);
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

/// +0x10 = node 0x28's point pulled toward node 0x29's by `by` / `div` (the
/// step's local sp + 0x30), the frames 0x1F6830 leaves (s0 = the task, s1
/// = D).
fn pull(fx: &mut Fx, t: u32, sp: u32, div: f32, by: f32, ra: [u32; 2]) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_A);
    crate::b076::node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), ra[0]);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_B);
    crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ra[1]);
    for k in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x30 + 4 * k), fx.mem.f(t + 0x10 + 4 * k)), div);
        fx.mem.setf(sp + 0x30 + 4 * k, v);
    }
    for k in 0..3u32 {
        let v = ee::mul(fx.mem.f(sp + 0x30 + 4 * k), by);
        add(fx, t + 0x10 + 4 * k, v);
    }
}

/// 0x1E02190 (D, task): a pink smoke ring (size 15..34, cell 0..2) whose
/// radius 10..49 grows by 26..65, turning by -5..4 a update, life 3..5;
/// in by 0x40 under 0xC0, out by 0xE.
fn smoke(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_21B4, 0x14);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xF));
        let v = rmod(fx, t, 0x01E0_21E0, 3);
        l2::rgb(fx, t + 0x60, 0x80, 0x60, 0x70);
        fx.mem.set16(t + 0x56, v);
        let v = rmod(fx, t, 0x01E0_220C, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = fx.rand_from(t, 0x01E0_222C);
        let r = fx.div(v >> 8, 0xA).1;
        fx.mem.set16(t + 0x52, r - 5);
        let v = rmod(fx, t, 0x01E0_2250, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_226C, 0x28);
        fx.mem.set16(t + 0xE, v + 0x1A);
        let v = rmod(fx, t, 0x01E0_228C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_22A8, 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x10, 0xE);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let a = fx.mem.i16(t + 0x5A) + fx.mem.i16(t + 0x52);
    fx.mem.set16(t + 0x5A, a);
    age(fx, t);
}

/// 0x1E02370 (D, task): a dark red arc (radius 15 + 5 x +0x50) on the line
/// node 0x28 -> 0x29 at +0x50 / 10, facing the camera turned 0x4000; in by
/// 0x13 under 0xC0 for 140 updates, out by 9.
fn arc(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let k = ee::from_i32(fx.mem.i16(t + 0x50));
        fx.mem.setf(t + 0x4C, ee::add(15.0, ee::mul(5.0, k)));
        l2::rgb(fx, t + 0x60, 0x18, 0x18, 0x18);
        l2::rgb(fx, t + 0x64, 0x30, 0x10, 0x20);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x8C {
        fade_in(fx, t, 0x67, 0xC0, 0x13);
    } else {
        fade_out(fx, t, 0x67, 0xB, 9);
    }
    let by = ee::from_i32(fx.mem.i16(t + 0x50));
    pull(fx, t, sp, 10.0, by, [0x01E0_2470, 0x01E0_2490]);
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2530);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02560 (D, task): a pink arc at the target (radius 11 growing by 10,
/// the inner 4x it; +0x34 15..44 growing by 20; turned rand % 360, angle
/// +0x24 rand % 0xFFFF); in by 0x60 under 0xC0 for 4 updates, out by 0x13.
fn grow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4130_0000);
        fx.mem.write_u32(t + 0x48, 0x4130_0000);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0x40);
        let v = rmod(fx, t, 0x01E0_25B8, 0x168);
        fx.mem.set16(t + 0x52, v);
        fx.mem.set16(t + 0x5A, 0x3C);
        fx.mem.set16(t + 0x5C, 6);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, 0x01E0_25EC, 0x1E);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0xF));
        let v = rmod(fx, t, 0x01E0_2618, 0xFFFF);
        fx.mem.set32(t + 0x24, v);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x67, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x67, 0x15, 0x13);
    }
    add(fx, t + 0x34, 20.0);
    add(fx, t + 0x48, 10.0);
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(t + 0x4C, ee::mul(4.0, s));
    age(fx, t);
}

/// 0x1E026E0 (D, task): a textured arc at the target (radius 140 growing by
/// 45..52.9, the inner +0x4C a share of it), three kinds by +0x50 (purple
/// on black / bright by the effect update's parity); 0x80, out by 5.
fn arc_t(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x430C_0000);
        fx.mem.setf(t + 0x4C, ee::sub(140.0, 58.0));
        let parity = |fx: &mut Fx| fx.mem.i16(COUNTER).wrapping_rem(2);
        let colours: [((i32, i32, i32), (i32, i32, i32)); 3] = [((0x80, 0x20, 0xC0), (0x80, 0x40, 0xFF)), ((0x60, 0x20, 0x80), (0x80, 0x40, 0xFF)), ((0xC0, 0x80, 0x80), (0xFF, 0xA0, 0xA0))];
        let dark = [0, 0, 0x10];
        for kind in 0..3 {
            if fx.mem.i16(t + 0x50) == kind {
                let (even_c, odd_c) = colours[kind as usize];
                if parity(fx) == 0 {
                    l2::rgb(fx, t + 0x64, even_c.0, even_c.1, even_c.2);
                    let g = dark[kind as usize];
                    l2::rgb(fx, t + 0x60, g, g, g);
                }
                if parity(fx) == 1 {
                    l2::rgb(fx, t + 0x64, odd_c.0, odd_c.1, odd_c.2);
                    l2::rgb(fx, t + 0x60, 0x30, 0x30, 0x30);
                }
                if kind != 0 {
                    fx.mem.write_u32(t + 0x34, 0x42C8_0000);
                }
                fx.mem.set16(t + 6, if kind == 0 { 8 } else { 0xA });
            }
        }
        for (kind, a, b) in [(0, 0xA, 0xA), (1, 0xA, 3), (2, 0x3C, 0x3C)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.set16(t + 0x5A, a);
                fx.mem.set16(t + 0x5C, b);
            }
        }
        let v = rmod(fx, t, 0x01E0_29C0, 0x50);
        fx.mem.setf(t + 0x38, ee::from_i32(v + 0x1C2));
        let v = ee::div(fx.mem.f(t + 0x38), 10.0);
        fx.mem.setf(t + 0x38, v);
        fx.mem.write_u32(t + 0x30, 0x43AF_0000);
        let v = ee::div(fx.mem.f(t + 0x30), 10.0);
        fx.mem.setf(t + 0x30, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, 0x01E0_2A1C, 0xFFFF);
        fx.mem.set32(t + 0x24, v);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x67, 0x80);
    } else {
        fade_out(fx, t, 0x67, 7, 5);
    }
    for (kind, share) in [(0, 0x3ECC_CCCDu32), (1, 0x3F33_3333), (2, 0x3F33_3333)] {
        if fx.mem.i16(t + 0x50) == kind {
            let s = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x38));
            fx.mem.setf(t + 0x48, s);
            fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(share), s));
            if kind != 0 {
                let v = fx.mem.f(t + 0x38);
                add(fx, t + 0x34, v);
            }
        }
    }
    age(fx, t);
}

/// 0x1E02B70 (D, task): a pink light column (size 6; 3 / 8 wide, 70 / 30 /
/// 400 high by +0x50, the first two growing by 30 / 10) on the node line
/// 0x28 -> 0x29 at +0x52 (to 25) / 25; in by 0x40 for 4 updates, out by 9
/// (b065's 0x1E02410).
fn column(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x80);
        l2::rgb(fx, t + 0x64, 0xFF, 0x40, 0x80);
        for (kind, w, h) in [(0, 0x4040_0000u32, 0x428C_0000u32), (1, 0x4040_0000, 0x41F0_0000), (2, 0x4100_0000, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, w);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x52, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    for (kind, by) in [(0, 30.0f32), (1, 10.0)] {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(t + 0x54) != 0 {
            add(fx, t + 0x34, by);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let n = fx.mem.i16(t + 0x52);
        if n < 0x19 {
            fx.mem.set16(t + 0x52, n + 1);
        }
    }
    age(fx, t);
    let a = fx.mem.i16(t + 0x5A) + 6;
    fx.mem.set16(t + 0x5A, a);
    let by = ee::from_i32(fx.mem.i16(t + 0x52));
    pull(fx, t, sp, 25.0, by, [0x01E0_2D38, 0x01E0_2D58]);
}

/// 0x1E02E10 (D, task): a white mote (size 130, cell 2..4) thrown 6.6 out
/// at a random angle, then drifting 0.5 a update; in by 0x40 for 10
/// updates, out by 9 (b065's 0x1E02240, word-identical).
fn mote(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x82);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2E48);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2E80);
        let r = fx.div(v >> 4, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        drift(fx, t, f32::from_bits(0x40D3_3333));
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        drift(fx, t, 0.5);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2FB0);
    age(fx, t);
}

/// z += `by` sin +0x5A, x += `by` cos +0x5A.
fn drift(fx: &mut Fx, t: u32, by: f32) {
    let a = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, TAB, a, 0);
    add(fx, t + 0x18, ee::mul(by, s));
    let a = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, TAB, a, 1);
    add(fx, t + 0x10, ee::mul(by, c));
}

/// 0x1E02FE0 (D, task): camera slots 4 / 5 (0x1F7E20) from D+0xB1A0 (128,
/// 10, 10, 10; node 0x29's point; 0, 123, w rising by 0.3 to 1 for 160
/// updates, then down by 0.3); cleared when the task ends (w at 0.1, or
/// the timer at 2).
fn camera(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4120_0000), (8, 0x4120_0000), (0xC, 0x4120_0000), (0x20, 0), (0x24, 0x42F6_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        for k in 0..3 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(CAM + 0x10 + 4 * k, w);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.set16(t + 0x50, 0);
    }
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_B);
    crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_30C0);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
        fx.mem.write_u32(CAM + 0x10 + 4 * k, w);
    }
    let w = fx.mem.f(CAM + 0x28);
    let step = f32::from_bits(0x3E99_999A);
    if fx.mem.i16(t + 0x54) < 0xA0 {
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, step));
        }
    } else if w <= f32::from_bits(0x3DCC_CCCD) {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, step));
    }
    let (p, ra) = if fx.mem.i16(t + 0x58) == 0 { (0, 0x01E0_31AC) } else { (CAM, 0x01E0_31C0) };
    crate::blib::exe::camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 2 {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
}

/// 0x1E03200 (D, task): the stage light (0x1F7F10) down by 10 to 0x40 for
/// 40 updates, back up by 10 to 0x80 once the timer is under 30; 0x80 and
/// the task's end when the timer is at 3.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x28 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x41 { 0x40 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x1E {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    age(fx, t);
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
}

/// 0x1E03310 (D, task): a frame flash (b590's 0x1E02450 with pink-grey
/// 0x90 / 0x78 / 0x80, 0xA0 / 0x80 / 0x90 on update 118, ST from +0x48 =
/// 2.5): in by 0x10 under 0x40 (and from update 119 by 0x1A under 0x68)
/// until update 145, then out by 4.
fn flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0x90, 0x78, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) == 0x76 {
        l2::rgb(fx, t + 0x60, 0xA0, 0x80, 0x90);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let c = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x48 + k, c);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x50, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(512.0, s), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(512.0, s), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(COUNTER) < 0x91 {
        fade_in(fx, t, 0x63, 0x40, 0x10);
        if fx.mem.i16(COUNTER) >= 0x77 {
            fade_in(fx, t, 0x63, 0x68, 0x1A);
        }
    } else {
        fade_out(fx, t, 0x63, 6, 4);
    }
    age(fx, t);
}
