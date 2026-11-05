//! BMODULE b084 (card 300), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02E50, data area 0x1ED0000, 0x7A00 bytes) [F]
//!: b170's family (card 316): 50 tasks of 0x70
//! bytes at D+0x2A0, b170's globals and spawn (0x1E03610), a head point
//! D+0x74A0 following the attacker (node 1's x, node 0x15's y / z to
//! 0x98, then along the facing by 0.08 to 0xA9 and 4.5 to 0xD1), its copy
//! D+0x74B0 to 0xA9. Drawn with ring quads (`billboard::ring_quads` on the
//! bytes D+0x768C), gouraud rings (`quads4::ring_band`), the gouraud tube
//! (`tubetrail::tube_gouraud(GOURAUD2_44)` 0x1E03750) along the head's
//! 70-point history D+0x18B0 and six orbiting 70-point histories D+0x1D10 +
//! 0x460 k (heads D+0x74C0 + 0x10 k), and target tints (`quads4::tint4`).
//! To update 0xBE two grey ring sparks a update at nodes 0x15 / 0x1E; 0x4B
//! two shrinking rings at those nodes; 0x74 two growing rings at the head /
//! its copy; 0xAA the head's tube and six orbiting tubes; 0xD2 two discs,
//! five dark sparks and a tint at the head; from 0xD3 to 0x121 a spark a
//! update there with the disc size D+0x7640 growing by 10 to 800. The step
//! 0x1E01FA0 is drawn but never spawned (not ported). Returns 1 on update
//! 0xCD, 4 on 0x136, 2 on 0x12C, 6 on 0xD8..0x126, 8 once the 0x14A-update
//! timer D+0x7620 has run out.
use crate::blib::{billboard, quads4, spawn as hist, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 50;
const POS: u32 = D + 0x250;
const SPARK: u32 = 0x01E0_1A20;
const SPARK_B: u32 = 0x01E0_1BD0;
const SPARK_C: u32 = 0x01E0_1DB0;
const ORBIT: u32 = 0x01E0_22E0;
const TUBE: u32 = 0x01E0_2640;
const SHRINK: u32 = 0x01E0_2800;
const GROW: u32 = 0x01E0_2990;
const DISC: u32 = 0x01E0_2B90;
const TINT: u32 = 0x01E0_2D00;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_30A0;
const TRAIL: u32 = D + 0x18B0;
const ORBITS: u32 = D + 0x1D10;
const HEAD: u32 = D + 0x74A0;
const HEAD_B: u32 = D + 0x74B0;
const HEADS: u32 = D + 0x74C0;
const TEX: u32 = D + 0x7600;
const TIMER: u32 = D + 0x7620;
const COLOUR: u32 = D + 0x7624;
const TINT_W: u32 = D + 0x7634;
const SIZE: u32 = D + 0x7640;
const NODES: u32 = D + 0x7654;
const SPAWN_K: u32 = D + 0x7682;
const FACING: u32 = D + 0x7686;
const COUNTER: u32 = D + 0x768A;
const BYTES: u32 = D + 0x768C;
const TAB: u32 = D + 0x788C;

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn orbit(k: i32) -> u32 {
    (ORBITS as i32 + 0x460 * k) as u32
}

fn head(k: i32) -> u32 {
    (HEADS as i32 + 0x10 * k) as u32
}

/// The entry 0x1E02E50 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x140);
    fx.mem.write_u32(D + 0x762C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x140);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x7A00);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x7688, param);
    fx.mem.set32(TIMER, 0x14A);
    for (k, n) in [0x15, 0x1E, 1].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x7664, r);
    for (k, id) in [(0u32, 0xE), (1, 0xF)] {
        let t = fx.texture(4, id);
        fx.mem.write_u32(D + 0x298 + 4 * k, t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    let t = fx.texture(5, 0);
    fx.mem.write_u32(TEX, t);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_2FAC);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_2FD4);
        let r = fx.div(r, 0x1E).1;
        fx.mem.set8(BYTES + k, r - 0xF);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_301C);
        let r = fx.div(r, 0x28).1;
        fx.mem.set8(D + 0x778C + k, r - 0x14);
    }
    fx.mem.write_u32(SIZE, 0x437A_0000);
    0
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    copy4(fx, D + 0x94, r);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        // The step's prologue: ra, the entry's s1 = 0, s0 = D.
        let frame = if f == SPARK || f == TINT { 0x20 } else { 0x30 };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
        }
        match f {
            SPARK => spark(fx, t),
            SPARK_B => spark_b(fx, t, false),
            SPARK_C => spark_b(fx, t, true),
            ORBIT => orbit_step(fx, t),
            TUBE => tube_step(fx, t),
            SHRINK => shrink(fx, t),
            GROW => grow(fx, t),
            DISC => disc(fx, t),
            TINT => tint(fx, t),
            _ => panic!("b084: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    let mut drawn = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        drawn += 1;
        draw(fx, sp, t, drawn);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0xCD => 1,
        0x136 => 4,
        0x12C => 2,
        0xD8..=0x126 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw cases for task `t` (`drawn` its s1).
fn draw(fx: &mut Fx, sp: u32, t: u32, drawn: u32) {
    let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
    for (g, local, mode, b8, b0) in [(SPARK, 0x70u32, 0x20, 5, 0), (SPARK_B, 0x80, 0x21, 5, 0), (SPARK_C, 0x90, 0x21, 0xA, 1)] {
        if f(fx) == g {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + local, 0x14, c);
            fx.mem.set16(D + 0xB8, b8);
            fx.mem.set16(D + 0xB0, b0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, mode, BYTES);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
    }
    if f(fx) == ORBIT {
        let k = fx.mem.i16(t + 0x50);
        tubetrail::tube_gouraud(fx, &tubetrail::GOURAUD2_44, D, t, orbit(k), 0);
    }
    if f(fx) == TUBE {
        tubetrail::tube_gouraud(fx, &tubetrail::GOURAUD2_44, D, t, TRAIL, 0);
    }
    if f(fx) == SHRINK {
        let h = fx.self_handle();
        let k = fx.mem.i16(t + 0x50);
        let slot = fx.mem.i16((NODES as i32 + 2 * k) as u32);
        crate::b076::node_position_saving(fx, h, slot, sp + 0x60, u64::from(D), Some(u64::from(drawn)), 0x01E0_3358);
        quads4::ring_band(fx, D, t, 0x10, sp + 0x60);
    }
    if f(fx) == GROW {
        match fx.mem.i16(t + 0x50) {
            0 => copy4(fx, sp + 0x60, HEAD),
            1 => copy4(fx, sp + 0x60, HEAD_B),
            _ => {}
        }
        let mode = if counter(fx) < 0xA0 { 0x20 } else { 0x11 };
        quads4::ring_band(fx, D, t, mode, sp + 0x60);
    }
    if f(fx) == DISC {
        copy4(fx, sp + 0x60, t + 0x10);
        quads4::ring_band(fx, D, t, 0x10, sp + 0x60);
    }
    if f(fx) == TINT {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x60);
        // 0x1F71A0's frame below the entry's (its saved ra, s0 = D).
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_3498), (0, u64::from(D))]);
        let z = fx.mem.f(sp + 0x68);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 30.0) } else { ee::add(z, 30.0) };
        fx.mem.setf(sp + 0x68, z);
        let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
        let mut x = fx.mem.read_u64(sp + 0xC8);
        if let Some(w) = fx.project(p, None, &mut x) {
            fx.mem.write_u64(sp + 0xC8, x);
            fx.mem.setf(TINT_W, w);
        }
        crate::b590::project_residue(fx, sp, sp + 0x60);
        for (kind, mode) in [(0, 1), (1, 0x20)] {
            if fx.mem.i16(t + 0x50) == kind {
                let w = fx.mem.f(TINT_W);
                quads4::tint4(fx, t, mode, w);
            }
        }
    }
}

/// 0x1E03610(D, step, 0): b170's spawn; its frame's saves: the emitter's
/// s0 = D, the entry's s1 = 0 / s2 = param, the runner's s3 / s4 (0 [H])
/// and the return address `ra`.
fn spawn(fx: &mut Fx, step: u32, param: i32, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, 0), (0x20, param as i64 as u64), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
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
            l2::copy3(fx, t + 0x10, POS);
            let s = fx.mem.i16(SPAWN_K);
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

/// 0x1F6830 for the emitter (s0 = D, s1 = the entry's 0).
fn enode(fx: &mut Fx, k: u32, out: u32, ra: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES + 2 * k);
    crate::b076::node_position_saving(fx, h, slot, out, u64::from(D), Some(0), ra);
}

/// 0x1E01590(D, 0): the update counter, the head point and the emitters
/// (frame 0x50: ra, the entry's s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x10, 0x01E0_3070), (0, u64::from(D))]);
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    enode(fx, 2, sp + 0x20, 0x01E0_15C0);
    enode(fx, 0, sp + 0x30, 0x01E0_15D8);
    let x = fx.mem.f(sp + 0x20);
    fx.mem.setf(HEAD, x);
    let c = counter(fx);
    if c < 0x99 {
        let z = fx.mem.f(sp + 0x38);
        fx.mem.setf(HEAD + 8, z);
    } else if c < 0xD2 {
        let by = if c < 0xAA { f32::from_bits(0x3DA3_D70A) } else { 4.5 };
        let z = fx.mem.f(HEAD + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, by) } else { ee::add(z, by) };
        fx.mem.setf(HEAD + 8, z);
    }
    if counter(fx) < 0xAA {
        copy4(fx, HEAD_B, HEAD);
    }
    if counter(fx) < 0xDC {
        let y = fx.mem.f(sp + 0x34);
        fx.mem.setf(HEAD + 4, y);
    }
    if counter(fx) < 0xBF {
        enode(fx, 0, POS, 0x01E0_1710);
        spawn(fx, SPARK, param, 0x01E0_1724);
        enode(fx, 1, POS, 0x01E0_173C);
        spawn(fx, SPARK, param, 0x01E0_1750);
    }
    if counter(fx) >= 0xD3 {
        copy4(fx, POS, HEAD);
        if counter(fx) < 0x122 {
            spawn(fx, SPARK_B, param, 0x01E0_17A4);
        }
        let s = fx.mem.f(SIZE);
        if crate::exe::ee_lt(s, 800.0) {
            fx.mem.setf(SIZE, ee::add(s, 10.0));
        }
    }
    if counter(fx) == 0xD2 {
        copy4(fx, POS, HEAD);
        spawn(fx, DISC, param, 0x01E0_181C);
        spawn(fx, DISC, param, 0x01E0_1830);
        for ra in [0x01E0_1844, 0x01E0_1858, 0x01E0_186C, 0x01E0_1880, 0x01E0_1894] {
            spawn(fx, SPARK_C, param, ra);
        }
    }
    for (c, step, ras) in [(0x74, GROW, [0x01E0_18BCu32, 0x01E0_18D8]), (0x4B, SHRINK, [0x01E0_1900, 0x01E0_191C])] {
        if counter(fx) == c {
            fx.mem.set16(SPAWN_K, 0);
            spawn(fx, step, param, ras[0]);
            fx.mem.set16(SPAWN_K, 1);
            spawn(fx, step, param, ras[1]);
        }
    }
    if counter(fx) == 0xAA {
        spawn(fx, TUBE, param, 0x01E0_1940);
        let ras = [0x01E0_1958, 0x01E0_1974, 0x01E0_1990, 0x01E0_19AC, 0x01E0_19C8, 0x01E0_19E4];
        for (k, ra) in ras.into_iter().enumerate() {
            fx.mem.set16(SPAWN_K, k as i32);
            spawn(fx, ORBIT, param, ra);
        }
    }
    if counter(fx) == 0xD2 {
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, TINT, param, 0x01E0_1A0C);
    }
    fx.leave(0x50);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// The alpha +0x63 up by 0x40 under 0xC0 for +6 updates, then down by 0x13
/// (the task ends under 0x15).
fn spark_fade(fx: &mut Fx, t: u32) {
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0x15 {
        fx.mem.write_u8(t + 0x63, a - 0x13);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The three random angles +0x20 / +0x24 / +0x28 (rand % 0xFFFF).
fn angles(fx: &mut Fx, s0: u32, t: u32, ras: [u32; 3]) {
    for (k, ra) in ras.into_iter().enumerate() {
        let v = fx.rand_from(s0, ra);
        let r = fx.div(v, 0xFFFF).1;
        fx.mem.set32(t + 0x20 + 4 * k as u32, r);
    }
}

/// 0x1E01A20 (D, task): a grey ring spark: size 10, cell 4, turned at
/// random, radius 10..59, life 1..3, b703's ring fade.
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        angles(fx, t, t, [0x01E0_1A68, 0x01E0_1A84, 0x01E0_1AA0]);
        let v = fx.rand_from(t, 0x01E0_1ABC);
        let r = fx.div(v, 0x32).1;
        fx.mem.set16(t + 0x50, r + 0xA);
        let v = fx.rand_from(t, 0x01E0_1ADC);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        let v = fx.rand_from(t, 0x01E0_1AF8);
        let r = fx.div(v, 3).1;
        fx.mem.set16(t + 0xE, r + 1);
        let v = fx.rand_from(t, 0x01E0_1B18);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = fx.rand_from(t, 0x01E0_1B38);
        let r = fx.div(v, 3).1;
        fx.mem.set16(t + 6, r + 1);
        fx.mem.set16(t + 0x58, 2);
    }
    spark_fade(fx, t);
    age(fx, t);
}

/// 0x1E01BD0 / 0x1E01DB0 (`dark`) (D, task): ring sparks at the head with
/// radius D+0x7640 / 1.4 + rand % 100 (0x1A2B80 every update into s0, the
/// rands' saved s0): grey, size 20, life 1..3; dark (0x20 x3), growing by
/// 20..49, life 80..82.
fn spark_b(fx: &mut Fx, t: u32, dark: bool) {
    let base = if dark { 0x01E0_1DB0u32 } else { 0x01E0_1BD0 };
    let v = ee::div(fx.mem.f(SIZE), f32::from_bits(0x3FB3_3333));
    fx.fix_residue(v, base + 0x38);
    let s0 = ee::fixsfsi(v) as i16 as i32;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.set16(t + 0x56, 4);
        let g = if dark { 0x20 } else { 0x80 };
        l2::rgb(fx, t + 0x60, g, g, g);
        let s = s0 as u32;
        angles(fx, s, t, [base + 0x78, base + 0x94, base + 0xB0]);
        let v = fx.rand_from(s, base + 0xCC);
        let r = fx.div(v, 0x64).1;
        fx.mem.set16(t + 0x50, s0 + r);
        let v = fx.rand_from(s, base + 0xEC);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        let v = fx.rand_from(s, base + 0x108);
        let r = if dark { fx.div(v, 0x1E).1 + 0x14 } else { fx.div(v, 3).1 + 1 };
        fx.mem.set16(t + 0xE, r);
        let v = fx.rand_from(s, base + 0x128);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = fx.rand_from(s, base + 0x148);
        let r = fx.div(v, 3).1;
        fx.mem.set16(t + 6, r + if dark { 0x50 } else { 1 });
        fx.mem.set16(t + 0x58, 2);
    }
    spark_fade(fx, t);
    if dark {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// The tube tasks' alpha 0xC0 to update 0xD6, then down by 7 with the size
/// down by 1 to 0; colour +0x60 (0x80, 0x10, 0xFF) / (0x40, 0x10, 0x80) by
/// the age's parity; +6 up to 0x44 points.
fn tube_common(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 6);
    if n < 0x44 {
        fx.mem.set16(t + 6, n + 1);
    }
    if counter(fx) < 0xD7 {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 9 {
            fx.mem.write_u8(t + 0x63, a - 7);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let s = fx.mem.f(t + 0x48);
        if !crate::exe::ee_le(s, 0.0) {
            fx.mem.setf(t + 0x48, ee::sub(s, 1.0));
        }
    }
    if fx.mem.i16(t + 0x54) % 2 == 0 {
        l2::rgb(fx, t + 0x60, 0x80, 0x10, 0xFF);
    } else {
        l2::rgb(fx, t + 0x60, 0x40, 0x10, 0x80);
    }
}

/// 0x1E022E0 (D, task): an orbiting tube: its head D+0x74C0 + 0x10 k (k =
/// +0x50) circles the head at radius +0x30 (up by 1.5 to 10) in x / y by
/// +0x5A (+5 a update), z along the facing by 4.5 to update 0xD1; size 30,
/// (0x80, 0x10, 0x10) outer.
fn orbit_step(fx: &mut Fx, t: u32) {
    let k = fx.mem.i16(t + 0x50);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 6, 8);
        fx.mem.write_u8(t + 0x63, 0);
        for e in 0..0x45u32 {
            l2::copy3(fx, orbit(k) + 0x10 * e, HEAD);
        }
        l2::copy3(fx, head(k), HEAD);
        l2::rgb(fx, t + 0x64, 0x80, 0x10, 0x10);
        let v = fx.rand_from(t, 0x01E0_23CC);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        fx.mem.write_u32(t + 0x48, 0x41F0_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    tube_common(fx, t);
    if fx.mem.i16(t + 0x54) != 0 {
        let k = fx.mem.i16(t + 0x50);
        if counter(fx) < 0xD2 {
            let z = fx.mem.f(head(k) + 8);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 4.5) } else { ee::add(z, 4.5) };
            fx.mem.setf(head(k) + 8, z);
        }
        for (axis, cos) in [(0u32, 1), (4, 0)] {
            let a = fx.mem.i16(t + 0x5A);
            let s = l2::sin_deg(fx, TAB, a, cos);
            let k = fx.mem.i16(t + 0x50);
            let v = ee::add(fx.mem.f(HEAD + axis), ee::mul(fx.mem.f(t + 0x30), s));
            fx.mem.setf(head(k) + axis, v);
        }
        let a = fx.mem.i16(t + 0x5A) + 5;
        fx.mem.set16(t + 0x5A, a);
        let r = fx.mem.f(t + 0x30);
        if crate::exe::ee_lt(r, 10.0) {
            fx.mem.setf(t + 0x30, ee::add(r, 1.5));
        }
    }
    if fx.mem.i16(t + 0x54) < 0x32 {
        let k = fx.mem.i16(t + 0x50);
        hist::history(fx, orbit(k) + 0x450, 0x46, head(k));
    }
    age(fx, t);
}

/// 0x1E02640 (D, task): the head's tube (size 100, (0x40, 0x10, 0x80)
/// outer), the history taking the head to age 0x31.
fn tube_step(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 6, 8);
        fx.mem.write_u8(t + 0x63, 0);
        for e in 0..0x45u32 {
            l2::copy3(fx, TRAIL + 0x10 * e, HEAD);
        }
        l2::rgb(fx, t + 0x64, 0x40, 0x10, 0x80);
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    tube_common(fx, t);
    if fx.mem.i16(t + 0x54) < 0x32 {
        hist::history(fx, D + 0x1D00, 0x46, HEAD);
    }
    age(fx, t);
}

/// 0x1E02800 (D, task): a grey ring at node 0x15 / 0x1E (kind) of inner
/// radius 200, down by 10 to 100 from update 0xAD, outer (0x40, 0x10,
/// 0x80) / (0x70, 0x20, 0xF0) by the age's parity; in by 0xE under 0xC0 to
/// age 0x8B, then out by 8.
fn shrink(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = fx.rand_from(t, 0x01E0_2848);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x8C {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0xE));
        }
    } else if a >= 0xA {
        fx.mem.write_u8(t + 0x63, a - 8);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x64, 0x40, 0x10, 0x80);
    } else {
        l2::rgb(fx, t + 0x64, 0x70, 0x20, 0xF0);
    }
    if counter(fx) >= 0xAD {
        let r = fx.mem.f(t + 0x48);
        if !crate::exe::ee_le(r, 100.0) {
            fx.mem.setf(t + 0x48, ee::sub(r, 10.0));
        }
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_295C);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E02990 (D, task): a white / violet ring at the head (kind 0) or its
/// copy (1), inner radius 50 growing by 10 below 300 from age 0x24 (kind 1
/// to update 0xAB, then down by 10 to 100); in by 0x10 under 0xC0 to update
/// 0x106, then out by 5.
fn grow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4248_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x40, 0x10, 0x80);
        let v = fx.rand_from(t, 0x01E0_29EC);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if counter(fx) < 0x107 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x10));
        }
    } else if a >= 7 {
        fx.mem.write_u8(t + 0x63, a - 5);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let up = |fx: &mut Fx| {
        if fx.mem.i16(t + 0x54) >= 0x24 {
            let r = fx.mem.f(t + 0x48);
            if crate::exe::ee_lt(r, 300.0) {
                fx.mem.setf(t + 0x48, ee::add(r, 10.0));
            }
        }
    };
    if fx.mem.i16(t + 0x50) == 0 {
        up(fx);
    }
    if fx.mem.i16(t + 0x50) == 1 {
        if counter(fx) < 0xAC {
            up(fx);
        } else {
            let r = fx.mem.f(t + 0x48);
            if !crate::exe::ee_le(r, 100.0) {
                fx.mem.setf(t + 0x48, ee::sub(r, 10.0));
            }
        }
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2B54);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E02B90 (D, task): a disc ring of inner radius D+0x7640 (every
/// update), outer (0x40, 0x10, 0x80) / (0x60, 0x10, 0xA0) by the age's
/// parity; in by 0x2A under 0x80 to age 0x4F, then out by 8.
fn disc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let s = fx.mem.read_u32(SIZE);
        fx.mem.write_u32(t + 0x48, s);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x40, 0x10, 0x80);
        let v = fx.rand_from(t, 0x01E0_2BEC);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x50 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x2A));
        }
    } else if a >= 0xA {
        fx.mem.write_u8(t + 0x63, a - 8);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let s = fx.mem.read_u32(SIZE);
    fx.mem.write_u32(t + 0x48, s);
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x64, 0x40, 0x10, 0x80);
    } else {
        l2::rgb(fx, t + 0x64, 0x60, 0x10, 0xA0);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2CC4);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E02D00 (D, task): the target tint (kind 0 grey 16 updates, 1 (0xD0,
/// 0xD0, 0x80) / (0xC0, 0xC0, 0x80) 16); in by 0x12 under 0x80, out by 4.
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0xF0);
            l2::rgb(fx, t + 0x64, 0xF0, 0xF0, 0xF0);
            fx.mem.set16(t + 6, 0x10);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xD0, 0xD0, 0x80);
            l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0x80);
            fx.mem.set16(t + 6, 0x10);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        for at in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + at);
            if a < 0x80 {
                fx.mem.write_u8(t + at, a + 0x12);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 6 {
            fx.mem.write_u8(t + 0x63, a - 4);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x67);
        fx.mem.write_u8(t + 0x67, if a >= 6 { a - 4 } else { 0 });
    }
    age(fx, t);
}
