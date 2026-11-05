//! BMODULE b354 (card 485), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02AE0, data area 0x1ED0000, 0xB630 bytes) [F]
//!: b641's program (400 tasks of 0x70 bytes at
//! D+0x2A0, alive +0x58, step +0x44 through `jalr`, 0x1E02AD0 the idle
//! step; globals at D+0xB2xx) spawned by 0x1E03290 (b750's spawn, the
//! same function: kind D+0xB2B0 into +0x50 / +0x52, angle D+0xB2A2 into
//! +0x5A) and drawn with the second library: quads (add5d81dd3fe at
//! 0x1E01230), rings of quads (7369422df7f8 at 0x1E00920), sparks
//! (21cb40d76bec at 0x1E014F0) and arcs (e6dce326551d at 0x1E00ED0) about
//! node 4's point D+0xB1D0. Update 1 the stage light; 0x29 at node 5
//! twenty-four white flares fanned 15 degrees apart; 0x2D two green arcs
//! at node 4 and 0x2D..0xA5 there five green flares, three rings and a
//! blue spark burst a update; 0x96 a grey-green frame flash; 0xA8 a green
//! quad 2.5 below. Returns 1 on update 0x23, 4 on 0xCD, 2 on 0xBE, 6 on
//! 0x2E..0xB8, 8 once the 0xD7-update timer D+0xB210 has run out.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const FLARE_A: u32 = 0x01E0_1C30;
const FLARE_B: u32 = 0x01E0_1D80;
const RING: u32 = 0x01E0_1F40;
const SPARK: u32 = 0x01E0_20F0;
const QUAD: u32 = 0x01E0_22F0;
const ARC_A: u32 = 0x01E0_2550;
const ARC_B: u32 = 0x01E0_2660;
const LIGHT: u32 = 0x01E0_2780;
const FLASH: u32 = 0x01E0_2890;
const IDLE: u32 = 0x01E0_2AD0;
/// The return address of the entry's `jalr` into a step (0x1E02DE8 + 8).
const TASK_RA: u64 = 0x01E0_2DF0;
const POS: u32 = D + 0x250;
/// Node 4's point (the arcs' centre).
const AIM: u32 = D + 0xB1D0;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const COLOUR: u32 = D + 0xB214;
const NODES: u32 = D + 0xB244;
const SPAWN_A: u32 = D + 0xB2A2;
const SPAWN_K: u32 = D + 0xB2B0;
const FACING: u32 = D + 0xB2B4;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;
const SINES: u32 = D + 0xB4BC;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// (rand >> 4) % m (HI).
fn rand4_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v >> 4, m).1
}

/// The entry 0x1E02AE0 (D, param).
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
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0xD7);
    for (k, node) in [5, 4, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_2CD0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_2D00, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_2D54);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
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
            FLARE_A | FLARE_B => 0x30,
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
            FLARE_A => flare_a(fx, t),
            FLARE_B => flare_b(fx, t),
            RING => ring(fx, t),
            SPARK => sparks(fx, t),
            QUAD => quad(fx, t),
            ARC_A => arc(fx, t, ARC_A, 0x4120_0000, 3.0),
            ARC_B => arc(fx, t, ARC_B, 0x4348_0000, f32::from_bits(0x3ECC_CCCD)),
            LIGHT => light(fx, t),
            FLASH => flash(fx, t, s),
            _ => panic!("b354: unknown task step {f:#x}"),
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
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        for (g, local) in [(FLARE_A, 0x50u32), (FLARE_B, 0x60)] {
            if f(fx) == g {
                tex(fx, TEX);
                let c = fx.mem.i16(t + 0x56);
                cell(fx, sp + local, 0x14, c);
                fx.mem.set16(D + 0xBE, 0);
                billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
            }
        }
        if f(fx) == RING {
            tex(fx, TEX + 4);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x14, c);
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand_from(s0, 0x01E0_2F88);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == SPARK {
            cell(fx, sp + 0x80, 0x96, 2);
            tex(fx, D + 0x29C);
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            fx.mem.set16(D + 0xB8, 0xC);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == QUAD {
            tex(fx, TEX + 8);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x90, 0x9B, c);
            let o = fx.mem.i16(t + 0x52);
            fx.mem.set16(D + 0xBE, o);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x40), (0x9A, 0x40)] {
                fx.mem.set16(D + o, v);
            }
            fx.mem.set16(D + 0xBE, 0);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if f(fx) == ARC_A {
            fx.mem.set16(D + 0xB8, 0x24);
            let mode = if fx.mem.i16(COUNTER) < 0x3B { 1 } else { 0 };
            billboard::arc_strips(fx, D, t, mode, AIM);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == ARC_B {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x20, AIM);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x23 => 1,
        0xCD => 4,
        0xBE => 2,
        0x2E..=0xB8 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E03290(D, step, 0): b750's spawn (the same function); its frame's
/// saves the emitter's s0 = D / s1 / s2 (the entry's param, or a fan
/// angle), the runner's s3 / s4 (0 [H]) and the return address `ra`.
fn spawn(fx: &mut Fx, step: u32, s1: u32, s2: u64, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, u64::from(s1)), (0x20, s2), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
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

fn node_pos(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES + 2 * k);
    fx.node_position(h, slot, out);
}

fn aim_to_pos(fx: &mut Fx) {
    for k in 0..4 {
        let w = fx.mem.read_u32(AIM + 4 * k);
        fx.mem.write_u32(POS + 4 * k, w);
    }
}

/// 0x1E018F0(D, 0): the update counter and the emitters (frame 0x40: ra,
/// the entry's s2 = param, s1 = D, s0 = 0; then s0 = D, s1 / s2 loop
/// counts).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_2DB0), (0x20, p), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 1 {
        spawn(fx, LIGHT, D, p, 0x01E0_1944);
    }
    if c(fx) == 0x29 {
        node_pos(fx, 0, POS);
        for k in 0..0x18u32 {
            fx.mem.set16(SPAWN_A, 0xF * k as i32);
            spawn(fx, FLARE_A, k, u64::from(0xF * k), 0x01E0_19AC);
        }
    }
    if c(fx) == 0x2D {
        node_pos(fx, 1, AIM);
    }
    if (0x2D..0xA6).contains(&c(fx)) {
        aim_to_pos(fx);
        if c(fx) == 0x2D {
            spawn(fx, ARC_B, D, p, 0x01E0_1A94);
        }
        if c(fx) == 0x2D {
            // s1 cleared in the branch's delay slot first.
            spawn(fx, ARC_A, 0, p, 0x01E0_1AC0);
        }
        for k in 0..5 {
            spawn(fx, FLARE_B, k, p, 0x01E0_1AE0);
        }
        for k in 0..3 {
            spawn(fx, RING, k, p, 0x01E0_1B20);
        }
        spawn(fx, SPARK, 0, p, 0x01E0_1B60);
    }
    // The emitter's s1 after the spawn loops (1) or the entry's D.
    let s1 = if (0x2D..0xA6).contains(&c(fx)) { 1 } else { D };
    if c(fx) == 0x96 {
        spawn(fx, FLASH, s1, p, 0x01E0_1BAC);
    }
    if c(fx) == 0xA8 {
        aim_to_pos(fx);
        let y = ee::sub(fx.mem.f(POS + 4), 2.5);
        fx.mem.setf(POS + 4, y);
        spawn(fx, QUAD, s1, p, 0x01E0_1C14);
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

/// Alpha in by `up` under `cap` while the age is under `until`, else out
/// by `down` (the task ends under `floor`).
fn fade(fx: &mut Fx, t: u32, at: u32, until: i32, cap: u8, up: u8, floor: u8, down: u8) {
    if fx.mem.i16(t + 0x54) < until {
        fade_in(fx, t, at, cap, up);
    } else {
        fade_out(fx, t, at, floor, down);
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

/// axis `a` += 2 sin (or cos) of the i16 angle at `t + deg` degrees.
fn drift(fx: &mut Fx, t: u32, a: u32, deg: u32, cos: i32) {
    let d = fx.mem.i16(t + deg);
    let s = l2::sin_deg(fx, SINES, d, cos);
    let v = ee::add(fx.mem.f(t + a), ee::mul(2.0, s));
    fx.mem.setf(t + a, v);
}

/// 0x1E01C30 (D, task): a white flare of size 50 (cell frame 2..4) at the
/// fan angle +0x5A: in by 0x60 under 0xC0 for 6 updates, then out by 19;
/// y += 2 sin, x += 2 cos of the angle a update; faces the camera.
fn flare_a(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x32);
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
        let v = rand4_mod(fx, t, 0x01E0_1C68, 3);
        fx.mem.set16(t + 0x56, v + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
    }
    fade(fx, t, 0x63, 6, 0xC0, 0x60, 0x15, 0x13);
    if fx.mem.i16(t + 0x54) != 0 {
        drift(fx, t, 0x14, 0x5A, 0);
        drift(fx, t, 0x10, 0x5A, 1);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1D58);
    age(fx, t);
}

/// 0x1E01D80 (D, task): a green (0xAF, 0xFF, 0xAF) flare of size 80
/// (cell frame 2..4), headings +0x5A / +0x5C random: in by 0x60 under 0xC0
/// for 6 updates, then out by 19; z / x += 2 sin / cos of +0x5A and y += 2
/// sin of +0x5C a update; faces the camera.
fn flare_b(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x50);
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
        let v = rand4_mod(fx, t, 0x01E0_1DB8, 3);
        fx.mem.set16(t + 0x56, v + 2);
        l2::rgb(fx, t + 0x60, 0xAF, 0xFF, 0xAF);
        fx.mem.set16(t + 0x58, 2);
        let v = rand4_mod(fx, t, 0x01E0_1DF0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand4_mod(fx, t, 0x01E0_1E10, 0x168);
        fx.mem.set16(t + 0x5C, v);
    }
    fade(fx, t, 0x63, 6, 0xC0, 0x60, 0x15, 0x13);
    if fx.mem.i16(t + 0x54) != 0 {
        drift(fx, t, 0x18, 0x5A, 0);
        drift(fx, t, 0x10, 0x5A, 1);
        drift(fx, t, 0x14, 0x5C, 0);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1F14);
    age(fx, t);
}

/// 0x1E01F40 (D, task): b703's growing grey ring (0x1E02370) with size
/// 10..19 and life 3..5: radius 10..49 growing by 16..35; in by 0x40
/// under 0xC0, out by 0xE.
fn ring(fx: &mut Fx, t: u32) {
    let f = RING;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = rmod(fx, t, f + 0x50, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x7C, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, f + 0x9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB8, 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, f + 0xD8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xF4, 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0x10 {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E020F0 (D, task): b735's spark burst (0x1E02850) in blue: size 80
/// growing by (rand % 50 + 100) / 10 a update, +0x4C a fifth of it; kind
/// (rand >> 4) % 2: 0 (0x80, 0x80, 0xC0) on black, 1 (0xA0, 0xA0, 0xFF)
/// on grey 0x50; +0x5A / +0x5C 30; 0x80 for 8 updates, out by 5.
fn sparks(fx: &mut Fx, t: u32) {
    let f = SPARK;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42A0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(80.0, 58.0));
        let v = fx.rand_from(t, f + 0x40);
        fx.mem.set16(t + 0x50, (v >> 4) % 2);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xC0);
            l2::rgb(fx, t + 0x64, 0, 0, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xA0, 0xA0, 0xFF);
            l2::rgb(fx, t + 0x64, 0x50, 0x50, 0x50);
        }
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 0x1E);
        let v = rmod(fx, t, f + 0xD8, 0x32);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0x64));
        let v = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, ra) in [(0x20u32, f + 0x120), (0x24, f + 0x13C), (0x28, f + 0x158)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, 7, 5);
    }
    let s = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, s);
    fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3E4C_CCCD), s));
    age(fx, t);
}

/// 0x1E022F0 (D, task): b544's spark quad (0x1E016F0) in green (0x40,
/// 0xFF, 0x40), size 50 x 50: cell row (rand >> 4) % 3, drift (unused),
/// angle +0x5C random, the camera's angles each update, orientation +0x52
/// (rand >> 4) % 4; in by 0x40 under 0x80 for 12 updates, out by 9.
fn quad(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x32);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2320);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h);
        l2::rgb(fx, t + 0x60, 0x40, 0xFF, 0x40);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_2354u32), (0x30, 0x01E0_2380), (0x38, 0x01E0_23AC)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_2404, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2420);
        for (o, ra) in [(4u32, 0x01E0_2428u32), (2, 0x01E0_2448), (0, 0x01E0_2468)] {
            let v = fx.rand_from(t, ra);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
        let v = fx.rand_from(t, 0x01E0_2488);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
    }
    fade(fx, t, 0x63, 0xC, 0x80, 0x40, 0xB, 9);
    for (o, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + o).wrapping_add(fx.mem.i16(t + s) as u32);
        fx.mem.write_u32(t + o, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2528);
    age(fx, t);
}

/// 0x1E02550 / 0x1E02660 (`f`) (D, task): b757's band (0x1E02880) in
/// green (0x80, 0xFF, 0x80) inside grey 0xC0, radius `r` growing by `by`
/// a update, facing the camera turned 0x4000; in by 0x30 under 0xC0 for
/// 14 updates, out by 0xF.
fn arc(fx: &mut Fx, t: u32, f: u32, r: u32, by: f32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, r);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0xFF, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0x67, 0xE, 0xC0, 0x30, 0x11, 0xF);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), f + 0xC0);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x4C, by);
    }
    age(fx, t);
}

/// 0x1E02780 (D, task): b757's stage light (0x1E02BC0) down by 10 to 0x30
/// for 20 updates, back up by 10 to 0x80 once the timer is under 20; 0x80
/// and the task's end when the timer is at 3.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x14 {
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

/// 0x1E02890 (D, task): b702's frame flash (0x1E02AD0) in grey-green
/// (0x80, 0x90, 0x80), ST from +0x48 = 2.5 growing by 0.8 to 8 every
/// update: in by 0x16 under 0x58 until update 0xBD, then out by 4.
fn flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x90, 0x80);
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
    let s = fx.mem.f(t + 0x48);
    if s < 8.0 {
        fx.mem.setf(t + 0x48, ee::add(s, f32::from_bits(0x3F4C_CCCD)));
    }
    if fx.mem.i16(COUNTER) < 0xBE {
        fade_in(fx, t, 0x63, 0x58, 0x16);
    } else {
        fade_out(fx, t, 0x63, 6, 4);
    }
    age(fx, t);
}
