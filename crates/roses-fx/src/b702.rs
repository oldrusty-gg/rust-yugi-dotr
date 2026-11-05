//! BMODULE b702 (card 220), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02D30, data area 0x1ED0000, 0xB870 bytes) [F]
//!: b757's program with every global after the
//! pool 0x140 higher (b590's 400-task pool of 0x70 bytes at D+0x2A0, alive
//! +0x58, step +0x44 through `jalr`, 0x1E02D20 the idle step; a 20-point
//! history of node 0x23 at D+0xB1D0), spawned by 0x1E03750 (780c0891d8d9,
//! `spawn::task400_b4`'s layout copy: colour D+0xB354, +0x50 from
//! D+0xB3F0) and drawn with the second library: rings of quads (a
//! 7369422df7f8 copy at 0x1E038E0, `billboard::ring_quads`), arcs
//! (e6dce326551d at 0x1E009B0), textured arcs (3a2db45c5a9a at 0x1E00FD0)
//! and single quads (add5d81dd3fe at 0x1E00D10). Update 1 the stage light
//! and camera slots 4 / 5 on node 0x5F; 2..249 a dark smoke ring a update
//! at node 0x23 (0xB6..0xBD four more along the line back to the
//! history's oldest point); updates 0x27 / 0x4E / 0x7A and 0x1A / 0x3D /
//! 0x64 / 0x7D thirteen white motes at nodes 0x59 / 0x5F; 0xB9..0xFA at
//! the target (20 up) textured arcs (kinds by the update's parity) and a
//! grey ring a update, on 0xB9 a red band, a frame flash and ten white
//! rings. Returns 1 on update 0xAF, 4 on 0x11D, 2 on 0x109, 6 on
//! 0xBA..0x103, 8 once the 0x122-update timer D+0xB350 has run out.
use crate::blib::{billboard, quads4, spawn, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const SMOKE: u32 = 0x01E0_1A00;
const RING: u32 = 0x01E0_1BD0;
const RING_W: u32 = 0x01E0_1DE0;
const ARC_T: u32 = 0x01E0_1FD0;
const MOTE: u32 = 0x01E0_24A0;
const BAND: u32 = 0x01E0_2670;
const CAMERA: u32 = 0x01E0_2790;
const LIGHT: u32 = 0x01E0_29C0;
const FLASH: u32 = 0x01E0_2AD0;
const IDLE: u32 = 0x01E0_2D20;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3178;
const CAM: u32 = D + 0xB1A0;
/// Node 0x23's history (20 points of 16 bytes).
const HIST: u32 = D + 0xB1D0;
const TEX: u32 = D + 0xB330;
const TIMER: u32 = D + 0xB350;
const NODE_A: u32 = D + 0xB384;
const NODE_B: u32 = D + 0xB386;
const NODE_C: u32 = D + 0xB388;
const SPAWN_K: u32 = D + 0xB3F0;
const FACING: u32 = D + 0xB3F4;
const COUNTER: u32 = D + 0xB3F8;
const BYTES_A: u32 = D + 0xB3FA;
const BYTES_B: u32 = D + 0xB4FA;
/// Never written (zero): the white rings' table.
const BYTES_C: u32 = D + 0xB5FA;
const TAB: u32 = D + 0xB6FC;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02D30 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0xB35C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB870);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB3F6, param);
    fx.mem.set32(TIMER, 0x122);
    for (k, n) in [0x23, 0x59, 0x5F, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODE_A + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB394, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_2F20);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_2F50, 0x1E);
        fx.mem.set8(BYTES_A + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_2FA4, 0x1E);
        fx.mem.set8(BYTES_B + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, k, 0x01E0_2FFC, 0x1E);
        fx.mem.set8(BYTES_B + k, v);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_304C, 0x1E);
        fx.mem.set8(BYTES_B + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.set8(BYTES_B + k, 0);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_30DC);
        fx.mem.set16(D + 0xB3A4 + 2 * k, v % 8);
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
            ARC_T | MOTE => 0x30,
            CAMERA => 0x50,
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
            RING => ring(fx, t, RING, (0x3C, 0x50, 0x50, 0x1E, 0x14, 9, 1.0), true),
            RING_W => ring(fx, t, RING_W, (0x3C, 0xC8, 0xFF, 0x46, 0x3C, 0xF, 5.0), false),
            ARC_T => arc_t(fx, t),
            MOTE => mote(fx, t),
            BAND => band(fx, t),
            CAMERA => camera(fx, t, s),
            LIGHT => light(fx, t),
            FLASH => flash(fx, t, s),
            _ => panic!("b702: unknown task step {f:#x}"),
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
        0xAF => 1,
        0x11D => 4,
        0x109 => 2,
        0xBA..=0x103 => 6,
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

/// D+0x94.. = a 32 x 64 cell at column +0x56.
fn strip_cell(fx: &mut Fx, t: u32) {
    let c = fx.mem.i16(t + 0x56);
    for (o, v) in [(0x94u32, c << 5), (0x96, 0), (0x98, 0x20), (0x9A, 0x40)] {
        fx.mem.set16(D + o, v);
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32) {
    let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
    if f(fx) == SMOKE {
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x50, 0x14, c);
        tex(fx, TEX);
        strip_cell(fx, t);
        fx.mem.set16(D + 0xB8, 3);
        let v = fx.rand_from(s0, 0x01E0_3244);
        fx.mem.set16(D + 0xBE, v % 4);
        fx.mem.set16(D + 0xB0, 0);
        let c = fx.mem.i16(COUNTER);
        let mode = if (0x61..0xA0).contains(&c) || fx.mem.i16(t + 0x54) >= 2 { 0x20 } else { 0x21 };
        billboard::ring_quads(fx, uv::uv_rect, D, t, mode, BYTES_A);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f(fx) == RING {
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x60, 0x14, c);
        tex(fx, TEX);
        strip_cell(fx, t);
        fx.mem.set16(D + 0xB8, 4);
        let v = fx.rand_from(s0, 0x01E0_3380);
        fx.mem.set16(D + 0xBE, v % 4);
        fx.mem.set16(D + 0xB0, 0);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f(fx) == RING_W {
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x70, 0x14, c);
        tex(fx, TEX + 4);
        fx.mem.set16(D + 0xB8, 4);
        let v = fx.rand_from(s0, 0x01E0_3420);
        fx.mem.set16(D + 0xBE, v % 4);
        fx.mem.set16(D + 0xB0, 0);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x10, BYTES_C);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f(fx) == ARC_T {
        let kind = |fx: &mut Fx| fx.mem.i16(t + 0x50);
        if kind(fx) == 0 {
            tex(fx, TEX);
            strip_cell(fx, t);
        }
        if kind(fx) == 1 {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x9B, c);
            tex(fx, D + 0x29C);
        }
        if kind(fx) == 2 {
            tex(fx, TEX);
            fx.mem.set16(D + 0xBE, 0);
            strip_cell(fx, t);
        }
        fx.mem.set16(D + 0xB8, 0);
        for k in 0..3 {
            let v = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(D + 0x20 + 4 * k, v);
        }
        fx.mem.set16(D + 0xB8, 0x24);
        if kind(fx) == 0 {
            fx.mem.set16(D + 0xB8, 6);
        }
        if kind(fx) == 2 {
            fx.mem.set16(D + 0xB8, 6);
        }
        fx.mem.set16(D + 0xBE, 0);
        quads4::arc_strips_tex(fx, D, t, 0x20, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f(fx) == MOTE {
        tex(fx, TEX + 4);
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x90, 0x14, c);
        fx.mem.set16(D + 0xBE, 0);
        billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
    }
    if f(fx) == BAND {
        fx.mem.set16(D + 0xB8, 0x24);
        billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
}

/// 0x1E03750(D, step, 0): `spawn::task70` on this record's layout, its
/// frame's saves the emitter's s0 = D / s1 and the entry's s2 = param.
fn spawn(fx: &mut Fx, step: u32, s1: u32, param: i32, ra: u32) -> i32 {
    let p = spawn::pool70_b4("780c0891d8d9").expect("layout");
    let saves = [(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, param as i64 as u64), (0x10, u64::from(s1)), (0, u64::from(D))];
    spawn::task70(fx, D, p, step, 0, &saves)
}

fn node_pos(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(at);
    fx.node_position(h, slot, out);
}

/// 0x1E013E0(D, 0): the update counter and the emitters (frame 0x40: f20
/// .. f22, the entry's s0 = 0 / s1 = D, ra).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_3138), (0x20, u64::from(D)), (0x10, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    // The emitter's s1: the entry's D until a loop counts in it.
    let mut s1 = D;
    if (2..0xFA).contains(&c(fx)) {
        node_pos(fx, NODE_A, POS);
        spawn(fx, SMOKE, s1, param, 0x01E0_1470);
    }
    if (0xB6..0xBE).contains(&c(fx)) {
        node_pos(fx, NODE_A, POS);
        let mut d = [0.0f32; 3];
        for k in 0..3u32 {
            d[k as usize] = ee::div(ee::sub(fx.mem.f(POS + 4 * k), fx.mem.f(HIST + 4 * k)), 4.0);
        }
        for k in 0..4u32 {
            s1 = k;
            for (j, dj) in d.iter().enumerate() {
                let at = POS + 4 * j as u32;
                let v = ee::sub(fx.mem.f(at), *dj);
                fx.mem.setf(at, v);
            }
            spawn(fx, SMOKE, s1, param, 0x01E0_155C);
        }
        s1 = 4;
    }
    if c(fx) == 1 {
        node_pos(fx, NODE_A, POS);
        for e in 0..0x13u32 {
            l2::copy3(fx, HIST + 0x10 * e, POS);
        }
        spawn(fx, LIGHT, s1, param, 0x01E0_1618);
        spawn(fx, CAMERA, s1, param, 0x01E0_162C);
    }
    if (1..0xFB).contains(&c(fx)) {
        node_pos(fx, NODE_A, POS);
        spawn::history(fx, HIST + 0x130, 0x14, POS);
    }
    if (0xB9..0xFB).contains(&c(fx)) {
        let h = fx.target_handle();
        fx.unit_position(h, POS);
        let y = ee::sub(fx.mem.f(POS + 4), 20.0);
        fx.mem.setf(POS + 4, y);
        fx.mem.set16(SPAWN_K, 0);
        if c(fx) % 2 == 1 {
            spawn(fx, ARC_T, s1, param, 0x01E0_171C);
        }
        fx.mem.set16(SPAWN_K, 1);
        if c(fx) % 2 == 1 {
            spawn(fx, ARC_T, s1, param, 0x01E0_176C);
        }
        fx.mem.set16(SPAWN_K, 2);
        if c(fx) % 2 == 0 {
            spawn(fx, ARC_T, s1, param, 0x01E0_17B8);
        }
        spawn(fx, RING, s1, param, 0x01E0_17CC);
        if c(fx) == 0xB9 {
            spawn(fx, BAND, s1, param, 0x01E0_17F8);
        }
        if c(fx) == 0xB9 {
            spawn(fx, FLASH, s1, param, 0x01E0_1824);
        }
        if c(fx) == 0xB9 {
            for k in 0..0xA {
                s1 = k;
                spawn(fx, RING_W, s1, param, 0x01E0_1858);
            }
            s1 = 0xA;
        }
    }
    for (at, ups, ra) in [(NODE_B, &[0x27, 0x4E, 0x7A][..], 0x01E0_1900u32), (NODE_C, &[0x1A, 0x3D, 0x64, 0x7D][..], 0x01E0_19B4)] {
        if ups.contains(&c(fx)) {
            node_pos(fx, at, POS);
            let y = ee::sub(fx.mem.f(POS + 4), 1.5);
            fx.mem.setf(POS + 4, y);
            for k in 0..0xD {
                s1 = k;
                spawn(fx, MOTE, s1, param, ra);
            }
            s1 = 0xD;
        }
    }
    let _ = s1;
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

fn young_fade(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x15, 0x13);
    }
}

/// 0x1E01A00 (D, task): b757's dark smoke ring with size 100..159,
/// growth 2..6 and life 9..16.
fn smoke(fx: &mut Fx, t: u32) {
    let f = SMOKE;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 0x3C);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x64));
        let v = rmod(fx, t, f + 0x50, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x18, 0x18, 0x18);
        let v = rmod(fx, t, f + 0x7C, 0xA);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, f + 0x9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB8, 5);
        fx.mem.set16(t + 0xE, v + 2);
        let v = rmod(fx, t, f + 0xD8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, f + 0xF4);
        fx.mem.set16(t + 6, v % 8 + 9);
        fx.mem.set16(t + 0x58, 2);
    }
    young_fade(fx, t);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x48, 1.0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E01BD0 / 0x1E01DE0 (`f`) (D, task): b757's ring with `k` = (size
/// m, size base, grey, growth m, growth base, life base, size step):
/// 0x1BD0 b757's word for word (turning 2 either way, `turns`), 0x1DE0
/// white, size 200..259 growing by 5, radius growth 60..129, life
/// 15..22, not turning.
fn ring(fx: &mut Fx, t: u32, f: u32, k: (i32, i32, i32, i32, i32, i32, f32), turns: bool) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, k.0);
        fx.mem.setf(t + 0x48, ee::from_i32(v + k.1));
        let v = rmod(fx, t, f + 0x50, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, k.2, k.2, k.2);
        let v = rmod(fx, t, f + 0x7C, 0xA);
        fx.mem.set16(t + 0x50, v + 5);
        let v = fx.rand_from(t, f + 0x9C);
        fx.mem.set16(t + 0x52, v % 2);
        let v = rmod(fx, t, f + 0xB8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xD4, k.3);
        fx.mem.set16(t + 0xE, v + k.4);
        let v = rmod(fx, t, f + 0xF4, 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, f + 0x110);
        fx.mem.set16(t + 6, v % 8 + k.5);
        fx.mem.set16(t + 0x58, 2);
    }
    young_fade(fx, t);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x48, k.6);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    if turns {
        let a = fx.mem.i16(t + 0x5A) + if fx.mem.i16(t + 0x52) == 0 { 2 } else { -2 };
        fx.mem.set16(t + 0x5A, a);
    }
    age(fx, t);
}

/// 0x1E01FD0 (D, task): b757's textured arc with radius 140 and kind 0's
/// +0x5A / +0x5C 60.
fn arc_t(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x430C_0000);
        fx.mem.setf(t + 0x4C, ee::sub(140.0, 58.0));
        let odd = |fx: &mut Fx| fx.mem.i16(COUNTER) % 2 == 1;
        let colours: [((i32, i32, i32), (i32, i32, i32)); 3] = [((0x80, 0xC0, 0x80), (0x80, 0xFF, 0x80)), ((0x60, 0x80, 0x60), (0x40, 0xFF, 0x40)), ((0x50, 0x50, 0x50), (0x60, 0x60, 0x60))];
        let dark = [8, 8, 0x28];
        let light = [0x38, 0x38, 0x30];
        for kind in 0..3 {
            if fx.mem.i16(t + 0x50) == kind {
                let (even_c, odd_c) = colours[kind as usize];
                if !odd(fx) {
                    l2::rgb(fx, t + 0x64, even_c.0, even_c.1, even_c.2);
                    let g = dark[kind as usize];
                    l2::rgb(fx, t + 0x60, g, g, g);
                }
                if odd(fx) {
                    l2::rgb(fx, t + 0x64, odd_c.0, odd_c.1, odd_c.2);
                    let g = light[kind as usize];
                    l2::rgb(fx, t + 0x60, g, g, g);
                }
                if kind == 1 {
                    fx.mem.write_u32(t + 0x34, 0x4120_0000);
                }
                if kind == 2 {
                    fx.mem.write_u32(t + 0x34, 0x428C_0000);
                }
                fx.mem.set16(t + 6, if kind == 0 { 8 } else { 0xA });
            }
        }
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.set16(t + 0x5A, 0x3C);
            fx.mem.set16(t + 0x5C, 0x3C);
            let v = fx.rand_from(t, 0x01E0_2270);
            fx.mem.set16(t + 0x52, (v >> 4) % 2);
        }
        for (kind, a, b) in [(1, 0xA, 3), (2, 0x3C, 0x3C)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.set16(t + 0x5A, a);
                fx.mem.set16(t + 0x5C, b);
            }
        }
        let v = rmod(fx, t, 0x01E0_22D0, 0x50);
        fx.mem.setf(t + 0x38, ee::from_i32(v + 0x15E));
        let v = ee::div(fx.mem.f(t + 0x38), 10.0);
        fx.mem.setf(t + 0x38, v);
        fx.mem.write_u32(t + 0x30, 0x43AF_0000);
        let v = ee::div(fx.mem.f(t + 0x30), 10.0);
        fx.mem.setf(t + 0x30, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, 0x01E0_232C, 0xFFFF);
        fx.mem.set32(t + 0x24, v);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x67, 0x80);
    } else {
        fade_out(fx, t, 0x67, 0xD, 0xB);
    }
    for (kind, share) in [(0, 0x3E4C_CCCDu32), (1, 0x3F33_3333), (2, 0x3F33_3333)] {
        if fx.mem.i16(t + 0x50) == kind {
            let s = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x38));
            fx.mem.setf(t + 0x48, s);
            fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(share), s));
            if kind == 0 {
                let by = if fx.mem.i16(t + 0x52) == 0 { 0x80 } else { -0x80 };
                let a = fx.mem.i32(t + 0x24).wrapping_add(by);
                fx.mem.set32(t + 0x24, a);
            } else {
                let v = fx.mem.f(t + 0x38);
                add(fx, t + 0x34, v);
            }
        }
    }
    age(fx, t);
}

/// 0x1E024A0 (D, task): b757's white mote with size 130.
fn mote(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x82);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_24D8);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2510);
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
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2640);
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

/// 0x1E02670 (D, task): a white / red band (radius 500 + 0.4 a update)
/// facing the camera turned 0x4000; in by 0x30 under 0xC0 for 30 updates,
/// out by 0xF.
fn band(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x43FA_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x1E {
        fade_in(fx, t, 0x67, 0xC0, 0x30);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2730);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x4C, f32::from_bits(0x3ECC_CCCD));
    }
    age(fx, t);
}

/// 0x1E02790 (D, task): b065's camera slots 4 / 5 from D+0xB1A0 (128, 10,
/// 5, 5; node 0x5F's point; 0, 123, w rising by 0.3 to 1 for 180
/// updates, then down by 0.3); cleared when the task ends (w at 0.1, or
/// the timer at 2).
fn camera(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4120_0000), (8, 0x40A0_0000), (0xC, 0x40A0_0000), (0x20, 0), (0x24, 0x42F6_0000), (0x28, 0)] {
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
    let slot = fx.mem.i16(NODE_C);
    crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_2874);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
        fx.mem.write_u32(CAM + 0x10 + 4 * k, w);
    }
    let w = fx.mem.f(CAM + 0x28);
    let step = f32::from_bits(0x3E99_999A);
    if fx.mem.i16(t + 0x54) < 0xB4 {
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, step));
        }
    } else if w <= f32::from_bits(0x3DCC_CCCD) {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, step));
    }
    let (p, ra) = if fx.mem.i16(t + 0x58) == 0 { (0, 0x01E0_2960) } else { (CAM, 0x01E0_2974) };
    crate::blib::exe::camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 2 {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
}

/// 0x1E029C0 (D, task): b757's stage light (word for word).
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x97 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
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

/// 0x1E02AD0 (D, task): a reddish frame flash (0x88 / 0x80 / 0x80, ST from
/// +0x48 = 2.5 growing by 0.2 to 6 from update 0xBA): in by 0x16 under
/// 0x58 until update 0x103, then out by 4.
fn flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0x88, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
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
    if fx.mem.i16(COUNTER) >= 0xBA {
        let s = fx.mem.f(t + 0x48);
        if s < 6.0 {
            fx.mem.setf(t + 0x48, ee::add(s, f32::from_bits(0x3E4C_CCCD)));
        }
    }
    if fx.mem.i16(COUNTER) < 0x104 {
        fade_in(fx, t, 0x63, 0x58, 0x16);
    } else {
        fade_out(fx, t, 0x63, 6, 4);
    }
    age(fx, t);
}
