//! BMODULE b247 (card 244; the module is `b247b` since `b247` is card
//! 247's port), hand-ported from the record (linked at 0x1E00000, entry
//! 0x1E03540, data area 0x1ED0000) [F]: b235's
//! program shape on b750's layout: 400 tasks of 0x70 bytes at D+0x2A0
//! (alive +0x58, step +0x44 through `jalr`, 0x1E03530 the idle step),
//! spawned by 0x1E03E60 (b750's spawn: +0x50 / +0x52 from D+0xB2B0, +0x5A
//! from D+0xB2A2), drawn with the second library (`billboard::arc_strips`
//! 0x1E00ED0, `quads4::arc_strips_tex` 0x1E01230, `billboard::ring_quads`
//! 0x1E00920, `quads4::sprite_quads` 0x1E01640) and b422's textured quad
//! 0x1E04010 ([`crate::b422::spark`], word-identical). Every update first
//! sets the attacker's unit flag +0x20A (0x1F69C0(1)). The emitter
//! 0x1E019B0 counts D+0xB2B8: on 0x25..0x8B the point rides the attacker's
//! node 0x16; on 0x3C a growing arc (0x1E01F70 kind 1, riding node 0x16)
//! and forty sparks (0x1E02FC0); on 0x46 a frame flash (0x1E03300); on
//! 0x47..0x8B at node 0x38 (height 40, z +/- 4 by the facing) textured
//! arcs 0x1E027A0 (kind 0 every third update, kinds 1 / 2 on the odd
//! ones); on 0x99..0xB3 at node 0x47 six arcs 0x1E02C50 a update; on 0xA7
//! at the target (z + 5) a white arc (0x1E01F70 kind 0), then to 0xB1 two
//! rings 0x1E02530 and five violet sparks a update. Returns 1 on update
//! 0x9F, 4 on 0xE3, 2 on 0xDC, 6 on 0xAA..0xD6, 8 once the 230-update
//! timer D+0xB210 has run out.
//!
//! The steps 0x1E02150 / 0x1E02320 are never spawned [F: every `addiu` of
//! them is the entry's draw compare]; only their draw branches are ported.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const ARC: u32 = 0x01E0_1F70;
const RINGQ: u32 = 0x01E0_2150;
const COLUMN: u32 = 0x01E0_2320;
const RING: u32 = 0x01E0_2530;
const ARC_T: u32 = 0x01E0_27A0;
const ARC_W: u32 = 0x01E0_2C50;
const SPARK: u32 = 0x01E0_2FC0;
const FLASH: u32 = 0x01E0_3300;
const IDLE: u32 = 0x01E0_3530;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3858;
const AIM: u32 = D + 0xB1D0;
const TIMER: u32 = D + 0xB210;
const COLOUR: u32 = D + 0xB214;
const NODES: u32 = D + 0xB244;
const SPAWN_A: u32 = D + 0xB2A2;
const SPAWN_K: u32 = D + 0xB2B0;
const FACING: u32 = D + 0xB2B4;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;
const TAB: u32 = D + 0xB4BC;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn copy(fx: &mut Fx, to: u32, from: u32, n: u32) {
    for k in 0..n {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The entry 0x1E03540 (D, param; s0 = 0, s1 = D, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0xE6);
    for (k, node) in [0x47, 0x38, 0x16, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    for (k, id) in [0xEu32, 0xF].into_iter().enumerate() {
        let w = fx.texture(4, id as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k as u32, w);
    }
    for k in 0..2u32 {
        let w = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xB1F0 + 4 * k, w);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(v, 0x01E0_3730);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3760, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_37B4);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    crate::b369::unit_flag(fx, 1, 0, 0x01E0_380C);
    emit(fx, param);
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
        let frame = if f == FLASH { 0x90 } else { 0x30 };
        let s = fx.enter(frame);
        fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        match f {
            ARC => arc(fx, t),
            RING => ring(fx, t),
            ARC_T => arc_t(fx, t),
            ARC_W => arc_w(fx, t),
            SPARK => spark(fx, t),
            FLASH => flash(fx, t, s),
            _ => panic!("b247b: unknown task step {f:#x}"),
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
        0x9F => 1,
        0xE3 => 4,
        0xDC => 2,
        0xAA..=0xD6 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    copy(fx, D + 0x94, r, 4);
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// D+0x94..: the cell column +0x56 of 32 x 64 cells (texture D+0xB1F0).
fn strip_cell(fx: &mut Fx, t: u32) {
    tex(fx, D + 0xB1F0);
    fx.mem.set16(D + 0xBE, 0);
    let c = fx.mem.i16(t + 0x56);
    for (o, v) in [(0x94u32, c << 5), (0x96, 0), (0x98, 0x20), (0x9A, 0x40)] {
        fx.mem.set16(D + o, v);
    }
}

fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32) {
    let kind = |fx: &mut Fx| fx.mem.i16(t + 0x50);
    if fx.mem.read_u32(t + 0x44) == ARC {
        fx.mem.set16(D + 0xB8, 0x24);
        if kind(fx) == 0 {
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        }
        if kind(fx) == 1 {
            billboard::arc_strips(fx, D, t, 0x20, t + 0x10);
        }
        fx.mem.set16(D + 0xB8, 0);
    }
    if fx.mem.read_u32(t + 0x44) == RINGQ {
        tex(fx, D + 0xB1F4);
        for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x40), (0x9A, 0x40)] {
            fx.mem.set16(D + o, v);
        }
        fx.mem.set16(D + 0xB8, 4);
        let v = fx.rand_from(s0, 0x01E0_394C);
        fx.mem.set16(D + 0xBE, v % 4);
        fx.mem.set16(D + 0xB0, 1);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    if fx.mem.read_u32(t + 0x44) == COLUMN {
        tex(fx, D + 0x29C);
        cell(fx, sp + 0x50, 0x9A, 0);
        fx.mem.set16(D + 0xBE, 0);
        quads4::sprite_quads(fx, D, t, 0, t + 0x10);
    }
    if fx.mem.read_u32(t + 0x44) == RING {
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x60, 0x9C, c);
        tex(fx, D + 0x29C);
        tex(fx, D + 0xB1F4);
        for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x80), (0x9A, 0x80), (0xB8, 0)] {
            fx.mem.set16(D + o, v);
        }
        copy(fx, D + 0x20, t + 0x10, 3);
        fx.mem.set16(D + 0xB8, 6);
        fx.mem.set16(D + 0xBE, 0);
        quads4::arc_strips_tex(fx, D, t, 0x20, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if fx.mem.read_u32(t + 0x44) == ARC_T {
        if kind(fx) == 0 {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x97, c);
            tex(fx, D + 0x29C);
        }
        if kind(fx) == 1 {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x9B, c);
            tex(fx, D + 0x29C);
        }
        if kind(fx) == 2 {
            strip_cell(fx, t);
        }
        fx.mem.set16(D + 0xB8, 0);
        copy(fx, D + 0x20, t + 0x10, 3);
        fx.mem.set16(D + 0xB8, 0x24);
        if kind(fx) == 2 {
            fx.mem.set16(D + 0xB8, 6);
        }
        fx.mem.set16(D + 0xBE, 0);
        quads4::arc_strips_tex(fx, D, t, 0x20, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if fx.mem.read_u32(t + 0x44) == ARC_W {
        if kind(fx) == 0 {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x90, 0x9B, c);
            tex(fx, D + 0x29C);
        } else if kind(fx) == 1 {
            strip_cell(fx, t);
        }
        fx.mem.set16(D + 0xB8, 0);
        copy(fx, D + 0x20, t + 0x10, 3);
        fx.mem.set16(D + 0xB8, 6);
        fx.mem.set16(D + 0xBE, 0);
        quads4::arc_strips_tex(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if fx.mem.read_u32(t + 0x44) == SPARK {
        tex(fx, D + 0xB1F4);
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0xA0, 0x14, c);
        fx.mem.set16(D + 0xBE, 0);
        crate::b422::spark(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
    }
}

/// 0x1E03E60(D, step, 0): b750's spawn; its frame's saves the emitter's s0
/// = D / s1, the entry's s2 = param / s3 (0), the runner's s4 (0 [H]) and
/// the return address `ra`.
fn spawn(fx: &mut Fx, step: u32, s1: u32, param: i32, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, u64::from(s1)), (0x20, param as i64 as u64), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
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
            l2::to_i16x4(fx, t, POS, 0);
            copy(fx, t + 0x10, POS, 3);
            let s = fx.mem.i16(SPAWN_K);
            fx.mem.set16(t + 0x50, s);
            let s = fx.mem.i16(SPAWN_K);
            fx.mem.set16(t + 0x52, s);
            let a = fx.mem.i16(SPAWN_A);
            fx.mem.set16(t + 0x5A, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E019B0(D, 0) (frame 0x30: ra, the entry's s1 = D, s0 = 0; its s0 =
/// D, s1 the loop counters): the update counter and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3818), (0x10, u64::from(D)), (0, 0)]);
    let mut s1 = D;
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let node = |fx: &mut Fx, k: u32, s1: u32, ra: u32| {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, n, POS, u64::from(D), Some(u64::from(s1)), ra);
    };
    if fx.mem.i16(COUNTER) == 0xA7 {
        let h = fx.target_handle();
        unit_position_saving(fx, h, AIM, u64::from(D), 0x01E0_1A10);
        let z = ee::add(fx.mem.f(AIM + 4), 5.0);
        fx.mem.setf(AIM + 4, z);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x99..0xB4).contains(&n) {
        node(fx, 0, s1, 0x01E0_1A7C);
        fx.mem.set16(SPAWN_K, 0);
        for k in 0..5 {
            s1 = k;
            spawn(fx, ARC_W, s1, param, 0x01E0_1AA4);
        }
        fx.mem.set16(SPAWN_K, 1);
        s1 = 0;
        spawn(fx, ARC_W, s1, param, 0x01E0_1AF0);
        s1 = 1;
    }
    let n = fx.mem.i16(COUNTER);
    if (0x47..0x8C).contains(&n) {
        node(fx, 1, s1, 0x01E0_1B54);
        fx.mem.write_u32(D + 0x254, 0x4220_0000);
        let z = fx.mem.f(D + 0x258);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 4.0) } else { ee::add(z, 4.0) };
        fx.mem.setf(D + 0x258, z);
        fx.mem.set16(SPAWN_K, 0);
        if fx.mem.i16(COUNTER) % 3 == 1 {
            spawn(fx, ARC_T, s1, param, 0x01E0_1BF0);
        }
        fx.mem.set16(SPAWN_K, 1);
        if fx.mem.i16(COUNTER) % 2 == 1 {
            spawn(fx, ARC_T, s1, param, 0x01E0_1C40);
        }
        fx.mem.set16(SPAWN_K, 2);
        if fx.mem.i16(COUNTER) % 2 == 1 {
            spawn(fx, ARC_T, s1, param, 0x01E0_1C90);
        }
    }
    if fx.mem.i16(COUNTER) == 0x46 {
        spawn(fx, FLASH, s1, param, 0x01E0_1CBC);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x25..0x8C).contains(&n) {
        node(fx, 2, s1, 0x01E0_1D00);
    }
    if fx.mem.i16(COUNTER) == 0xA7 {
        copy(fx, POS, AIM, 4);
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, ARC, s1, param, 0x01E0_1D60);
    }
    let n = fx.mem.i16(COUNTER);
    if (0xA7..0xB2).contains(&n) {
        copy(fx, POS, AIM, 4);
        for k in 0..2 {
            s1 = k;
            spawn(fx, RING, s1, param, 0x01E0_1DC8);
        }
        fx.mem.set16(SPAWN_K, 1);
        for k in 0..5 {
            s1 = k;
            spawn(fx, SPARK, s1, param, 0x01E0_1E18);
        }
        s1 = 5;
    }
    if fx.mem.i16(COUNTER) == 0x3C {
        node(fx, 2, s1, 0x01E0_1E78);
        for (a, k) in [(0, 0), (0x5A, 0), (0x2D, 1), (0x87, 1)] {
            fx.mem.set16(SPAWN_A, a);
            fx.mem.set16(SPAWN_K, k);
        }
        fx.mem.set16(SPAWN_K, 1);
        spawn(fx, ARC, s1, param, 0x01E0_1F08);
        fx.mem.set16(SPAWN_K, 0);
        for k in 0..0x28 {
            s1 = k;
            spawn(fx, SPARK, s1, param, 0x01E0_1F30);
        }
    }
    fx.leave(0x30);
}

/// The colours a textured arc takes on its first update by the update's
/// parity (+0x64 / +0x60).
fn parity_colours(fx: &mut Fx, t: u32) {
    let p = fx.mem.i16(COUNTER) % 2;
    if p == 0 {
        l2::rgb(fx, t + 0x64, 0xC0, 0x20, 0x20);
        l2::rgb(fx, t + 0x60, 0, 0, 0);
    }
    if fx.mem.i16(COUNTER) % 2 == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0xA0, 0xA0);
        l2::rgb(fx, t + 0x60, 0x30, 0x30, 0x30);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// Byte `at` up by `up` while below `cap`.
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

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

/// 0x1E01F70 (D, task): an arc; kind 0 (white / red, outer 220, life 14,
/// out by 9), kind 1 (pink, life 3, out by 15, riding node 0x16 with the
/// inner radius +0x48 growing by 25); spun by 0x4000 a update.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x4C, 0x435C_0000);
            fx.mem.set16(t + 6, 0xE);
            l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
            l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.set16(t + 6, 3);
            l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
            l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        }
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x67, 0xC0, 0x60);
    } else {
        if fx.mem.i16(t + 0x50) == 0 {
            fade_out(fx, t, 0x67, 0xB, 9);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fade_out(fx, t, 0x67, 0x11, 0xF);
        }
    }
    if fx.mem.i16(t + 0x50) == 1 {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 4);
        node_position_saving(fx, h, n, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_20F0);
        add(fx, t + 0x48, 25.0);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2114);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02530 (D, task): a ring at the target (radius 140 growing by 7..11.9,
/// inner a tenth of it), dark red or pink by rand; 0x80 for 8 updates, out
/// by 3; ended from update 0xE2.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x430C_0000);
        fx.mem.setf(t + 0x4C, ee::sub(140.0, 58.0));
        let v = fx.rand_from(t, 0x01E0_2578);
        fx.mem.set16(t + 0x50, (v >> 4) % 2);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xC0, 0x20, 0x20);
            l2::rgb(fx, t + 0x64, 0, 0, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xFF, 0xA0, 0xA0);
            l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        }
        fx.mem.set16(t + 0x5A, 0x3C);
        fx.mem.set16(t + 0x5C, 0x3C);
        let v = rmod(fx, t, 0x01E0_2610, 0x32);
        fx.mem.setf(t + 0x38, ee::from_i32(v + 0x46));
        let v = ee::div(fx.mem.f(t + 0x38), 10.0);
        fx.mem.setf(t + 0x38, v);
        let v = rmod(fx, t, 0x01E0_2650, 0x50);
        fx.mem.setf(t + 0x30, ee::from_i32(v + 0x96));
        let v = ee::div(fx.mem.f(t + 0x30), 10.0);
        fx.mem.setf(t + 0x30, v);
        fx.mem.write_u32(t + 0x34, 0x42C8_0000);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, ra) in [(0x20u32, 0x01E0_26A0u32), (0x24, 0x01E0_26BC), (0x28, 0x01E0_26D8)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, 5, 3);
    }
    let s = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x38));
    fx.mem.setf(t + 0x48, s);
    fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3DCC_CCCD), s));
    age(fx, t);
    if fx.mem.i16(COUNTER) >= 0xE2 {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E027A0 (D, task): a textured arc at node 0x38 (radius 140 growing by
/// 15..22.9, inner 0.4 / 0.7 of it), three kinds by +0x50 (red on black /
/// pink on grey by the update's parity); 0x80, out by 5; ended from update
/// 0x9D.
fn arc_t(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x430C_0000);
        fx.mem.setf(t + 0x4C, ee::sub(140.0, 58.0));
        for kind in 0..3 {
            if fx.mem.i16(t + 0x50) == kind {
                parity_colours(fx, t);
                if kind != 0 {
                    fx.mem.write_u32(t + 0x34, 0x4120_0000);
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
        let v = rmod(fx, t, 0x01E0_2A80, 0x50);
        fx.mem.setf(t + 0x38, ee::from_i32(v + 0x96));
        let v = ee::div(fx.mem.f(t + 0x38), 10.0);
        fx.mem.setf(t + 0x38, v);
        fx.mem.write_u32(t + 0x30, 0x43AF_0000);
        let v = ee::div(fx.mem.f(t + 0x30), 10.0);
        fx.mem.setf(t + 0x30, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, 0x01E0_2ADC, 0xFFFF);
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
    if fx.mem.i16(COUNTER) >= 0x9D {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E02C50 (D, task): a wide textured arc at node 0x47 (radius 140
/// growing by 15..22.9, inner 0.8 of it, height 100 / 400 by kind rising by
/// the same), turned a quarter by the facing, spun by 0x100 a update;
/// 0x80 for 8 updates, out by 5.
fn arc_w(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x430C_0000);
        fx.mem.setf(t + 0x4C, ee::sub(140.0, 58.0));
        for (kind, b) in [(0, 6), (1, 0x14)] {
            if fx.mem.i16(t + 0x50) == kind {
                parity_colours(fx, t);
                fx.mem.set16(t + 0x5A, 0x3C);
                fx.mem.set16(t + 0x5C, b);
            }
        }
        let v = rmod(fx, t, 0x01E0_2E20, 0x50);
        fx.mem.setf(t + 0x38, ee::from_i32(v + 0x96));
        let v = ee::div(fx.mem.f(t + 0x38), 10.0);
        fx.mem.setf(t + 0x38, v);
        let v = rmod(fx, t, 0x01E0_2E60, 0x50);
        fx.mem.setf(t + 0x30, ee::from_i32(v + 0x96));
        let v = ee::div(fx.mem.f(t + 0x30), 10.0);
        fx.mem.setf(t + 0x30, v);
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x34, 0x42C8_0000);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x34, 0x43C8_0000);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let a = if fx.mem.i16(FACING) != 0 { 0x4000 } else { -0x4000 };
        fx.mem.set32(t + 0x20, a);
        let v = rmod(fx, t, 0x01E0_2F00, 0xFFFF);
        fx.mem.set32(t + 0x28, v);
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fx.mem.write_u8(t + 0x67, 0x80);
    } else {
        fade_out(fx, t, 0x67, 7, 5);
    }
    let s = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x38));
    fx.mem.setf(t + 0x48, s);
    fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3F4C_CCCD), s));
    let v = fx.mem.f(t + 0x38);
    add(fx, t + 0x34, v);
    let a = fx.mem.read_u32(t + 0x28).wrapping_add(0x100);
    fx.mem.write_u32(t + 0x28, a);
    age(fx, t);
}

/// (rand % 0x800) - 0x400 (C remainder).
fn turn(fx: &mut Fx, t: u32, ra: u32) -> i32 {
    fx.rand_from(t, ra).wrapping_rem(0x800) - 0x400
}

/// 0x1E02FC0 (D, task): a spark quad (size 10, height growing by 40 a
/// update, pushed along z by +0x38), white (kind 0, life 10) or violet
/// (kind 1, life 22); in by 0x40 under 0x80, out by 9; ended from update
/// 0xE2.
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xA);
        fx.mem.set16(t + 0xA, 0xA);
        fx.mem.set16(t + 0xC, 0);
        let v = fx.rand_from(t, 0x01E0_2FF8);
        fx.mem.set16(t + 0x56, (v >> 4) % 3);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        }
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_3064u32), (0x30, 0x01E0_3090), (0x38, 0x01E0_30BC)] {
            let v = rmod(fx, t, ra, 100);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let v = ee::div(fx.mem.f(t + o), 20.0);
            fx.mem.setf(t + o, v);
        }
        let v = rmod(fx, t, 0x01E0_3114, 0x168);
        fx.mem.set16(t + 0x5A, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_3130);
        for (o, ra) in [(4u32, 0x01E0_3138u32), (2, 0x01E0_3158), (0, 0x01E0_3178)] {
            let v = turn(fx, t, ra);
            fx.mem.set16(t + o, v);
        }
        let v = fx.rand_from(t, 0x01E0_3198);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
        for (o, ra) in [(0x20u32, 0x01E0_31B8u32), (0x24, 0x01E0_31D4), (0x28, 0x01E0_31F0)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.set16(t + 6, 0xA);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.set16(t + 6, 0x16);
        }
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let h = fx.mem.i16(t + 0xA) + 0x28;
        fx.mem.set16(t + 0xA, h);
    }
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 0xC)), fx.mem.f(t + 0x38));
    fx.fix_residue(z, 0x01E0_32B8);
    fx.mem.set16(t + 0xC, ee::fixsfsi(z));
    age(fx, t);
    if fx.mem.i16(COUNTER) >= 0xE2 {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E03300 (D, task): a frame flash (b235's 0x1E03310 with pink-grey
/// 0x98 / 0x80 / 0x80, 0xA8 / 0x80 / 0x80 on update 0xA0, ST from +0x48 =
/// 2.5): in by 0x16 under 0x58 until update 0xB4, then out by 4.
fn flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0x98, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) == 0xA0 {
        l2::rgb(fx, t + 0x60, 0xA8, 0x80, 0x80);
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
    if fx.mem.i16(COUNTER) < 0xB4 {
        fade_in(fx, t, 0x63, 0x58, 0x16);
    } else {
        fade_out(fx, t, 0x63, 6, 4);
    }
    age(fx, t);
}
