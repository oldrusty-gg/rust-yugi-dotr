//! BMODULE b292 (card 185), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02560, data area 0x1ED0000, 0xCF30 bytes) [F]
//!: b522's layout of b703's program (400 tasks of
//! 0x70 at D+0x2A0, alive +0x58, step +0x44 through `jalr`, 0x1E02550 the
//! idle step; nodes D+0xCB44, angle D+0xCBA2, kind D+0xCBB2, facing
//! D+0xCBB6, counter D+0xCBBA, sines D+0xCDBC) spawned by 0x1E02B40
//! (b522's spawn without the +0x52 copy) and drawn with quads, standing
//! quads, light columns and b703's ribbon (0x1E02CE0, word for word).
//! Update 0x11 four columns pulled along nodes 0x14 -> 0x15 (b703's
//! 0x1E02BD0 word for word); on 0x32 / 0x47 / 0x5A / 0x7E ten white motes
//! (b377's 0x1E02150) at node 0x41 and on 0x3C / 0x50 / 0x64 / 0x7E ten at
//! node 0x49; 0x6B a green ribbon between the histories of nodes 0x14 /
//! 0x15; odd 0x7B..0x95 a grey frame tint (b020's window sprite); 0x83..
//! 0x87 a flickering standing quad at node 0x15. Returns 1 on update 0x79,
//! 4 on 0xBE, 2 on 0xAA, 6 on 0x84..0xA4, 8 once the 0xCD-update timer
//! D+0xCB10 has run out.
use crate::b076::node_position_saving;
use crate::blib::{billboard, quads4, spawn as hist, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const MOTE: u32 = 0x01E0_1AC0;
const PULLED: u32 = 0x01E0_1C90;
const FLICKER: u32 = 0x01E0_1F30;
const TRAILS: u32 = 0x01E0_20B0;
const TINT: u32 = 0x01E0_22F0;
const IDLE: u32 = 0x01E0_2550;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_2870;
/// The two node histories (20 points of 16 bytes).
const HIST: [u32; 2] = [D + 0xB1D0, D + 0xB310];
const TEX: u32 = D + 0xCAF0;
const TIMER: u32 = D + 0xCB10;
const COLOUR: u32 = D + 0xCB14;
const NODES: u32 = D + 0xCB44;
const SPAWN_A: u32 = D + 0xCBA2;
const FLICKERS: u32 = D + 0xCBA8;
const SPAWN_K: u32 = D + 0xCBB2;
const FACING: u32 = D + 0xCBB6;
const COUNTER: u32 = D + 0xCBBA;
const BYTES: u32 = D + 0xCBBC;
const TAB: u32 = D + 0xCDBC;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn node(fx: &mut Fx, k: u32) -> i32 {
    fx.mem.i16(NODES + 2 * k)
}

/// The entry 0x1E02560 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    fx.mem.write_u32(D + 0xCB1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xCF30);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xCBB8, param);
    fx.mem.set32(TIMER, 0xCD);
    for (k, n) in [0x41, 0x49, 0x14, 0x15, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xCB54, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..4 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x70);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x74)), 180.0);
    fx.fix_residue(y, 0x01E0_2750);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_2780, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_27D4);
        fx.mem.set16(D + 0xCB64 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
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
        let frame = match f {
            PULLED => 0x50,
            TRAILS => 0x40,
            TINT => 0x80,
            _ => 0x30,
        };
        let s = fx.enter(frame);
        if frame == 0x80 {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        }
        match f {
            MOTE => mote(fx, t),
            PULLED => pulled(fx, t, s),
            FLICKER => flicker(fx, t),
            TRAILS => trails(fx, t, s),
            TINT => tint(fx, t, s),
            _ => panic!("b292: unknown task step {f:#x}"),
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
        draw(fx, sp, t, s0, k);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x79 => 1,
        0xBE => 4,
        0xAA => 2,
        0x84..=0xA4 => 6,
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

fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32, k: u32) {
    let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
    if f(fx) == MOTE {
        tex(fx, TEX);
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x50, 0x14, c);
        let v = fx.mem.i16(t + 0x52);
        fx.mem.set16(D + 0xBE, v);
        billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
    }
    if f(fx) == PULLED {
        tex(fx, D + 0x29C);
        cell(fx, sp + 0x60, 0x9A, 0);
        fx.mem.set16(D + 0xBE, 0);
        quads4::sprite_quads(fx, D, t, 0, t + 0x10);
    }
    if f(fx) == FLICKER {
        let kind = fx.mem.i16(t + 0x50);
        let c = fx.div(kind, 3).1;
        tex(fx, (TEX as i32 + 4 + 4 * c) as u32);
        fx.mem.set16(D + 0xBE, 0);
        for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x40), (0x9A, 0x40)] {
            fx.mem.set16(D + o, v);
        }
        billboard::quad_standing(fx, uv::uv_rect, D, t, 0x21, t + 0x10);
        billboard::quad_standing(fx, uv::uv_rect, D, t, 0x21, t + 0x10);
    }
    if f(fx) == TRAILS {
        let saves = [u64::from(s0), u64::from(D), u64::from(t - 0x2A0), u64::from(k), 0, 0x01E0_2A50];
        crate::b703::ribbon(fx, t, HIST[0], HIST[1], saves);
    }
}

/// 0x1E02B40(D, step, 0): b522's spawn without the +0x52 copy (colour
/// D+0xCB14, kind D+0xCBB2 into +0x50, angle D+0xCBA2 into +0x5A); its
/// frame's saves the emitter's s0 = D / s1 / s2, the return address `ra`.
fn spawn(fx: &mut Fx, step: u32, s1: u64, s2: u64, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, s1), (0x20, s2), (0x50, u64::from(ra))]);
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
            for k in 0..3 {
                let w = fx.mem.read_u32(POS + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let s = fx.mem.i16(SPAWN_K);
            fx.mem.set16(t + 0x50, s);
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

/// 0x1E016F0(D, 0): the update counter and the emitters (frame 0x40: ra,
/// the entry's s2 = param, s1 = D, s0 = 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_2830), (0x20, p), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    // The emitter's s1 / s2 (the entry's D / param until a loop sets them).
    let (mut s1, mut s2) = (u64::from(D), p);
    if [0x32, 0x47, 0x5A, 0x7E].contains(&c(fx)) {
        let h = fx.self_handle();
        let slot = node(fx, 0);
        node_position_saving(fx, h, slot, POS, u64::from(D), Some(s1), 0x01E0_1780);
        for j in 0..0xA {
            let a = 0x24 * j;
            fx.mem.set16(SPAWN_A, a);
            spawn(fx, MOTE, j as u64, a as u64, 0x01E0_17AC);
        }
        (s1, s2) = (0xA, 0x168);
    }
    if [0x3C, 0x50, 0x64, 0x7E].contains(&c(fx)) {
        let h = fx.self_handle();
        let slot = node(fx, 1);
        node_position_saving(fx, h, slot, POS, u64::from(D), Some(s1), 0x01E0_183C);
        for j in 0..0xA {
            let a = 0x24 * j;
            fx.mem.set16(SPAWN_A, a);
            spawn(fx, MOTE, a as u64, j as u64, 0x01E0_1868);
        }
        (s1, s2) = (0x168, 0xA);
    }
    if c(fx) == 0x6B {
        spawn(fx, TRAILS, s1, s2, 0x01E0_18C4);
    }
    if (0x7B..0x97).contains(&c(fx)) && c(fx) % 2 == 1 {
        spawn(fx, TINT, s1, s2, 0x01E0_191C);
    }
    if (0x83..=0x87).contains(&c(fx)) {
        let h = fx.self_handle();
        let slot = node(fx, 3);
        node_position_saving(fx, h, slot, POS, u64::from(D), Some(s1), 0x01E0_1984);
        let k = fx.mem.i16(FLICKERS);
        fx.mem.set16(SPAWN_K, k);
        spawn(fx, FLICKER, s1, s2, 0x01E0_19B0);
        let k = fx.mem.i16(FLICKERS) + 1;
        fx.mem.set16(FLICKERS, k);
    }
    if c(fx) == 0x11 {
        let ras = [0x01E0_1A10, 0x01E0_1A40, 0x01E0_1A74, 0x01E0_1AA8];
        for ((a, kind), ra) in [(0, 0), (0x5A, 0), (0x2D, 1), (0x87, 1)].into_iter().zip(ras) {
            fx.mem.set16(SPAWN_A, a);
            fx.mem.set16(SPAWN_K, kind);
            spawn(fx, PULLED, s1, s2, ra);
        }
    }
    fx.leave(0x40);
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

/// 0x1E01AC0 (D, task): b377's white mote (0x1E02150, word for word but
/// the sines D+0xCDBC): size 40, cell 2..4, +0x52 a random 0..3 kind,
/// thrown 2 out at its spawn angle, then drifting 0.5 a update; in by 0x60
/// under 0xC0 for 6 updates, out by 0x13.
fn mote(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x28);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_1AF8);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        drift(fx, t, 2.0);
        let v = fx.rand_from(t, 0x01E0_1B88);
        fx.mem.set16(t + 0x52, (v >> 4).wrapping_rem(4));
    }
    if fx.mem.i16(t + 0x54) < 6 {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0x15, 0x13);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        drift(fx, t, 0.5);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1C60);
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

/// 0x1E01C90 (D, task): b703's 0x1E02BD0 word for word (b377's light
/// column; its nodes D+0xCB48 / D+0xCB4A = 0x14 -> 0x15 here): +0x10 node
/// 0x14's point pulled toward node 0x15's by +0x52 / 25.
fn pulled(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0xFF);
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
    let h = fx.self_handle();
    let slot = node(fx, 2);
    node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_1E58);
    let h = fx.self_handle();
    let slot = node(fx, 3);
    node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_1E78);
    for k in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x30 + 4 * k), fx.mem.f(t + 0x10 + 4 * k)), 25.0);
        fx.mem.setf(sp + 0x30 + 4 * k, v);
    }
    for k in 0..3u32 {
        let v = ee::mul(fx.mem.f(sp + 0x30 + 4 * k), by);
        add(fx, t + 0x10 + 4 * k, v);
    }
}

/// 0x1E01F30 (D, task): a white standing quad of height 50 (cell 0..2),
/// turned 0x4000 (0xC000 when facing), a random angle +0x5C; in by 0x60
/// under 0xC0 for 5 updates, out by 9; +0xA up by 40 a update; white on
/// odd ages, grey (0xC0) on even.
fn flicker(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x32);
        fx.mem.set16(t + 0xA, 0);
        let v = fx.rand_from(t, 0x01E0_1F64);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let turn = if fx.mem.i16(FACING) != 0 { 0xC000 } else { 0x4000 };
        fx.mem.write_u32(t + 0x20, turn);
        let v = rmod(fx, t, 0x01E0_1FC4, 0x168);
        fx.mem.set16(t + 0x5C, v);
    }
    if fx.mem.i16(t + 0x54) < 5 {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    let h = fx.mem.i16(t + 0xA) + 0x28;
    fx.mem.set16(t + 0xA, h);
    age(fx, t);
    let c = if fx.mem.i16(t + 0x54) % 2 == 1 { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x60, c, c, c);
}

/// 0x1E020B0 (D, task): b703's ribbon task on two histories: green (0x20,
/// 0xC0, 0x20) every update; on its first update size 5, +0xE 30 and the
/// histories filled (19 points) with nodes 0x14 / 0x15; in by 0x3C under
/// 0xF0 to update 0x8B, then out by 9; to update 0x89 each history takes
/// its node's point (`spawn::history`, 20 points); +6 = 10.
fn trails(fx: &mut Fx, t: u32, sp: u32) {
    l2::rgb(fx, t + 0x60, 0x20, 0xC0, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        fx.mem.set16(t + 0xE, 0x1E);
        for (k, ra) in [(0u32, 0x01E0_2118u32), (1, 0x01E0_2184)] {
            let h = fx.self_handle();
            let slot = node(fx, 2 + k);
            node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ra);
            for e in 0..0x13u32 {
                l2::copy3(fx, HIST[k as usize] + 0x10 * e, sp + 0x30);
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) < 0x8C {
        fade_in(fx, t, 0x63, 0xF0, 0x3C);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(COUNTER) < 0x8A {
        for (k, ra) in [(0u32, 0x01E0_2268u32), (1, 0x01E0_22A0)] {
            let h = fx.self_handle();
            let slot = node(fx, 2 + k);
            node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ra);
            hist::history(fx, HIST[k as usize] + 0x130, 0x14, sp + 0x30);
        }
    }
    fx.mem.set16(t + 6, 0xA);
    age(fx, t);
}

/// 0x1E022F0 (D, task; frame 0x80 at `sp`: ra, the entry's s0 = 0): b020's
/// window sprite (0x6FF8 .. 0x8FF8) sampling the frame (0x207EF0, filter
/// 1) at ST inset by +0x48 / 512, colour (0x80, 0x88, 0x80, alpha +0x63),
/// ALPHA 0x44, key 0; before age 15 in by 6 below 0x20 and the inset up by
/// 0.3 below 100, then out by 4 (ending under 6) and the inset down by 0.2
/// to 0.
fn tint(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
        fx.mem.set16(t + 0x58, 2);
        l2::rgb(fx, t + 0x60, 0x80, 0x88, 0x80);
    }
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let c = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, c);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, v) in [(d + 0x20, 0x6FF8u16), (d + 0x30, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for o in [0x18u32, 0x1C] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(d + o, ee::div(s, 512.0));
    }
    for o in [0x28u32, 0x2C] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(d + o, ee::div(ee::sub(512.0, s), 512.0));
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(t + 0x54) < 0xF {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x20 {
            fx.mem.write_u8(t + 0x63, a + 6);
        }
        let w = fx.mem.f(t + 0x48);
        if w < 100.0 {
            fx.mem.setf(t + 0x48, ee::add(w, f32::from_bits(0x3E99_999A)));
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 6 {
            fx.mem.write_u8(t + 0x63, a - 4);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let w = fx.mem.f(t + 0x48);
        if w <= 0.0 {
            fx.mem.write_u32(t + 0x48, 0);
        } else {
            fx.mem.setf(t + 0x48, ee::sub(w, f32::from_bits(0x3E4C_CCCD)));
        }
    }
    age(fx, t);
}
