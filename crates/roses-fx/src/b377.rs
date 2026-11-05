//! BMODULE b377 (card 146), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02F40, data area 0x1ED0000) [F] (helper C11, 31
//! Oct 2026): b235's program (400 tasks of 0x70 bytes at D+0x2A0, alive
//! +0x58, step +0x44 through `jalr`, 0x1E02F30 the idle step; spawn
//! 0x1E036D0 = b235's with the index at D+0xB2B4) on another layout and
//! program, drawn with the second library: tall smoke rings (0x1E03880,
//! `billboard::ring_quads_tall`), arcs (0x1E00920 `billboard::arc_strips`),
//! textured arcs (0x1E00F40 `quads4::arc_strips_tex`), light columns
//! (0x1E01350 `quads4::sprite_quads`) and quads (0x1E00C80
//! `billboard::quad`). Update 1: a frame flash (0x1E02C80), camera slots 4
//! / 5 (0x1E02A50) and four white-blue light columns between the
//! attacker's nodes 0x23 / 0x22; to update 11 purple arcs pulled along the
//! same line; 28..224 rings drawn along the line (one more every fifth
//! update, to 11 a update); updates 118 / 191 and 141 / 191 ten white
//! motes round nodes 0x67 / 0x70; 193..234 textured arcs at the target
//! (20 down). Returns 1 on update 183, 4 on 280, 2 on 260, 6 on 194..254,
//! 8 once the 290-update timer D+0xB210 has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const RING: u32 = 0x01E0_1CA0;
const ARC: u32 = 0x01E0_1F60;
const MOTE: u32 = 0x01E0_2150;
const ARC_T: u32 = 0x01E0_2320;
const COLUMN: u32 = 0x01E0_27B0;
const CAMERA: u32 = 0x01E0_2A50;
const FLASH: u32 = 0x01E0_2C80;
/// A step that only returns 0 (skipped by the entry).
const IDLE: u32 = 0x01E0_2F30;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3250;
const TIMER: u32 = D + 0xB210;
const COLOUR: u32 = D + 0xB214;
const NODE_A: u32 = D + 0xB244;
const NODE_B: u32 = D + 0xB246;
const NODE_C: u32 = D + 0xB248;
const NODE_D: u32 = D + 0xB24A;
const SPAWN_A: u32 = D + 0xB2A2;
const ARC_K: u32 = D + 0xB2AE;
const RINGS: u32 = D + 0xB2B0;
const SPAWN_K: u32 = D + 0xB2B4;
const SIDE: u32 = D + 0xB2B8;
const COUNTER: u32 = D + 0xB2BC;
const BYTES: u32 = D + 0xB2BE;
const TAB: u32 = D + 0xB4C0;
const CAM: u32 = D + 0xB1A0;
const AIM: u32 = D + 0xB1D0;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02F40 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2BA, param);
    fx.mem.set32(TIMER, 0x122);
    for (k, node) in [0x23, 0x22, 0x67, 0x70, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE_A + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    for k in 0..2u32 {
        let t = fx.texture(4, 0xE + k as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k, t);
    }
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xB1F0 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_3130);
    fx.mem.set16(SIDE, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3160, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_31B4);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
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
            RING | ARC => 0x40,
            MOTE | ARC_T => 0x30,
            COLUMN | CAMERA => 0x50,
            FLASH => 0x90,
            _ => panic!("b377: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        match f {
            RING => ring(fx, t, s),
            ARC => arc(fx, t, s),
            MOTE => mote(fx, t),
            ARC_T => arc_t(fx, t),
            COLUMN => column(fx, t, s),
            CAMERA => camera(fx, t, s),
            _ => flash(fx, t, s),
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
        0xB7 => 1,
        0x118 => 4,
        0x104 => 2,
        0xC2..=0xFE => 6,
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

/// D+0x94..+0x9B = (cell << 5, 0, 0x20, 0x40) as i16.
fn strip_cell(fx: &mut Fx, c: i32) {
    for (o, v) in [(0x94u32, c << 5), (0x96, 0), (0x98, 0x20), (0x9A, 0x40)] {
        fx.mem.set16(D + o, v);
    }
}

fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32) {
    if fx.mem.read_u32(t + 0x44) == RING {
        tex(fx, D + 0x29C);
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x50, 0x9B, c);
        tex(fx, D + 0xB1F0);
        let c = fx.mem.i16(t + 0x56);
        strip_cell(fx, c);
        fx.mem.set16(D + 0xB8, 4);
        let v = fx.rand_from(s0, 0x01E0_3324);
        fx.mem.set16(D + 0xBE, v % 4);
        fx.mem.set16(D + 0xB0, 0);
        billboard::ring_quads_tall(fx, uv::uv_rect, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    if fx.mem.read_u32(t + 0x44) == ARC {
        fx.mem.set16(D + 0xB8, 0x24);
        billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if fx.mem.read_u32(t + 0x44) == MOTE {
        tex(fx, D + 0xB1F4);
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x60, 0x14, c);
        let b = fx.mem.i16(t + 0x52);
        fx.mem.set16(D + 0xBE, b);
        billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
    }
    if fx.mem.read_u32(t + 0x44) == ARC_T {
        let kind = |fx: &mut Fx| fx.mem.i16(t + 0x50);
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
            tex(fx, D + 0xB1F0);
            fx.mem.set16(D + 0xBE, 0);
            let c = fx.mem.i16(t + 0x56);
            strip_cell(fx, c);
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
        cell(fx, sp + 0x90, 0x9A, 0);
        fx.mem.set16(D + 0xBE, 0);
        quads4::sprite_quads(fx, D, t, 0, t + 0x10);
    }
}

/// 0x1E036D0(D, step, arg): the first free task cleared and set off at
/// D+0x250 (i16 +0, float +0x10) with the colour D+0xB214 (twice), index
/// D+0xB2B4 (+0x50 / +0x52), angle D+0xB2A2 (+0x5A) (b235's spawn).
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

/// 0x1E016C0(D, 0): the update counter and the emitters (frame 0x40: ra,
/// the entry's s2 = param, s1 = D, s0 = 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_3210), (0x20, param as i64 as u64), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if (0x1C..0xE1).contains(&c(fx)) {
        let mut k = 0;
        let mut a = 0i32;
        while k < fx.mem.i16(RINGS) {
            fx.mem.set16(SPAWN_K, a);
            spawn(fx, RING);
            a = i32::from((a + 6) as i16);
            k += 1;
        }
        let n = fx.mem.i16(RINGS);
        if c(fx).wrapping_rem(5) == 0 && n < 0xB {
            fx.mem.set16(RINGS, n + 1);
        }
    }
    if c(fx) < 0xC {
        let k = fx.mem.i16(ARC_K);
        fx.mem.set16(SPAWN_K, k);
        spawn(fx, ARC);
        let k = fx.mem.i16(ARC_K) + 1;
        fx.mem.set16(ARC_K, k);
    }
    if c(fx) == 0xC3 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE_A);
        fx.node_position(h, slot, AIM);
    }
    if c(fx) == 0xC1 {
        let h = fx.target_handle();
        fx.unit_position(h, AIM);
    }
    if (0xC1..0xEB).contains(&c(fx)) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE_A);
        fx.node_position(h, slot, POS);
        fx.mem.write_u32(POS + 4, 0x4220_0000);
        let z = fx.mem.f(POS + 8);
        let z = if fx.mem.i16(SIDE) != 0 { ee::sub(z, 4.0) } else { ee::add(z, 4.0) };
        fx.mem.setf(POS + 8, z);
        for k in 0..4 {
            let w = fx.mem.read_u32(AIM + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        let y = ee::sub(fx.mem.f(POS + 4), 20.0);
        fx.mem.setf(POS + 4, y);
        fx.mem.set16(SPAWN_K, 0);
        if c(fx).wrapping_rem(2) == 1 {
            spawn(fx, ARC_T);
        }
        fx.mem.set16(SPAWN_K, 1);
        if c(fx).wrapping_rem(2) == 1 {
            spawn(fx, ARC_T);
        }
        fx.mem.set16(SPAWN_K, 2);
        if c(fx).wrapping_rem(2) == 0 {
            spawn(fx, ARC_T);
        }
    }
    if c(fx) == 1 {
        for (a, k) in [(0, 0), (0x5A, 0), (0x2D, 1), (0x87, 1)] {
            fx.mem.set16(SPAWN_A, a);
            fx.mem.set16(SPAWN_K, k);
            spawn(fx, COLUMN);
        }
        spawn(fx, FLASH);
        spawn(fx, CAMERA);
    }
    for (ups, node) in [([0x76, 0xBF], NODE_C), ([0x8D, 0xBF], NODE_D)] {
        if ups.contains(&c(fx)) {
            let h = fx.self_handle();
            let slot = fx.mem.i16(node);
            fx.node_position(h, slot, POS);
            let mut a = 0i32;
            for _ in 0..10 {
                fx.mem.set16(SPAWN_A, a);
                spawn(fx, MOTE);
                a = i32::from((a + 0x24) as i16);
            }
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

/// +0x10 = node 0x23's point pulled toward node 0x22's by `by` / `div` (the
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

/// 0x1E01CA0 (D, task): a blue ring (size 8..27 growing by 2, cell 0..2,
/// radius 5..14 growing by 2..6, turned rand % 360, life 1..2) at +0x52 /
/// 50 of the node line 0x23 -> 0x22; in by 0x40 under 0xC0, out by 0x13.
fn ring(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_1CCC, 0x14);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 8));
        let v = rmod(fx, t, 0x01E0_1CF8, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x80);
        let v = rmod(fx, t, 0x01E0_1D24, 0xA);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, 0x01E0_1D44, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_1D60, 5);
        fx.mem.set16(t + 0xE, v + 2);
        let v = rmod(fx, t, 0x01E0_1D80, 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, 0x01E0_1D9C);
        fx.mem.set16(t + 6, v.wrapping_rem(2) + 1);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x15, 0x13);
    }
    let by = ee::from_i32(fx.mem.i16(t + 0x52));
    pull(fx, t, sp, 50.0, by, [0x01E0_1E30, 0x01E0_1E50]);
    // A rand whose value is dropped.
    fx.rand_from(t, 0x01E0_1EEC);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x48, 2.0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E01F60 (D, task): a purple arc (radius 30 + 6 x +0x50) on the line
/// node 0x23 -> 0x22 at +0x50 / 10, facing the camera turned 0x4000; in by
/// 0x13 under 0xC0 for 140 updates, out by 9 (b235's 0x1E02370).
fn arc(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let k = ee::from_i32(fx.mem.i16(t + 0x50));
        fx.mem.setf(t + 0x4C, ee::add(30.0, ee::mul(6.0, k)));
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
        l2::rgb(fx, t + 0x64, 0x30, 0x10, 0x60);
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
    pull(fx, t, sp, 10.0, by, [0x01E0_2060, 0x01E0_2080]);
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2120);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02150 (D, task): a white mote (size 40, cell 2..4, +0x52 a random
/// 0..3 kind) thrown 2 out at its spawn angle, then drifting 0.5 a update;
/// in by 0x60 under 0xC0 for 6 updates, out by 0x13.
fn mote(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x28);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2188);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        drift(fx, t, 2.0);
        let v = fx.rand_from(t, 0x01E0_2218);
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
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_22F0);
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

/// 0x1E02320 (D, task): b235's textured arc (0x1E026E0) with +0x34 = 10
/// for kinds 1 / 2 and the growth (350..429) / 10.
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
        let v = rmod(fx, t, 0x01E0_2600, 0x50);
        fx.mem.setf(t + 0x38, ee::from_i32(v + 0x15E));
        let v = ee::div(fx.mem.f(t + 0x38), 10.0);
        fx.mem.setf(t + 0x38, v);
        fx.mem.write_u32(t + 0x30, 0x43AF_0000);
        let v = ee::div(fx.mem.f(t + 0x30), 10.0);
        fx.mem.setf(t + 0x30, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, 0x01E0_265C, 0xFFFF);
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

/// 0x1E027B0 (D, task): b235's light column (0x1E02B70) in white / blue
/// on the node line 0x23 -> 0x22.
fn column(fx: &mut Fx, t: u32, sp: u32) {
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
    pull(fx, t, sp, 25.0, by, [0x01E0_2978, 0x01E0_2998]);
}

/// 0x1E02A50 (D, task): camera slots 4 / 5 (0x1F7E20) from D+0xB1A0 (128,
/// 400, 2, 0; node 0x23's point; 0, 123, w rising by 0.3 to 1 for 210
/// updates, then down by 0.3); cleared when the task ends (w at 0.1, or
/// the timer at 1) (b235's 0x1E02FE0).
fn camera(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x43C8_0000), (8, 0x4000_0000), (0xC, 0), (0x20, 0), (0x24, 0x42F6_0000), (0x28, 0)] {
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
    let slot = fx.mem.i16(NODE_A);
    crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_2B34);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
        fx.mem.write_u32(CAM + 0x10 + 4 * k, w);
    }
    let w = fx.mem.f(CAM + 0x28);
    let step = f32::from_bits(0x3E99_999A);
    if fx.mem.i16(t + 0x54) < 0xD2 {
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, step));
        }
    } else if w <= f32::from_bits(0x3DCC_CCCD) {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, step));
    }
    let (p, ra) = if fx.mem.i16(t + 0x58) == 0 { (0, 0x01E0_2C20) } else { (CAM, 0x01E0_2C34) };
    crate::blib::exe::camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
}

/// 0x1E02C80 (D, task): b235's frame flash (0x1E03310) in grey-violet 0x80
/// / 0x70 / 0x90 (0x90 / 0x80 / 0xA0 on update 190), ST from +0x48 = 2.5
/// growing by 0.3 to 5 from update 191: in by 0x16 under 0x58 (and from
/// update 191 by 0x1A under 0x68) until update 250, then out by 4.
fn flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x70, 0x90);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) == 0xBE {
        l2::rgb(fx, t + 0x60, 0x90, 0x80, 0xA0);
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
    if fx.mem.i16(COUNTER) >= 0xBF {
        let s = fx.mem.f(t + 0x48);
        if s < 5.0 {
            fx.mem.setf(t + 0x48, ee::add(s, f32::from_bits(0x3E99_999A)));
        }
    }
    if fx.mem.i16(COUNTER) < 0xFA {
        fade_in(fx, t, 0x63, 0x58, 0x16);
        if fx.mem.i16(COUNTER) >= 0xBF {
            fade_in(fx, t, 0x63, 0x68, 0x1A);
        }
    } else {
        fade_out(fx, t, 0x63, 6, 4);
    }
    age(fx, t);
}
