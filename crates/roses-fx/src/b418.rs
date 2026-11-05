//! BMODULE b418 (card 494), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E027A0, data area 0x1ED0000, 0xB630 bytes) [F]
//!: b354's program shape (400 tasks of 0x70 bytes
//! at D+0x2A0, alive +0x58, step +0x44 through `jalr`, 0x1E02790 the idle
//! step; globals at D+0xB2xx) spawned by 0x1E02F10 (b750's spawn, the
//! same function) and drawn with the second library: arcs (e6dce326551d
//! at 0x1E00920), quads (add5d81dd3fe at 0x1E00C80), facing rings
//! (98b02c86a827 at 0x1E00F40, `billboard::ring_facing`) and sparks
//! (21cb40d76bec at 0x1E014B0). 0x3E..0x52 two white flares a update at
//! node 2; 0x41 there eighteen white flares thrown out 20 degrees apart
//! and a red frame flash; 0x44 eighteen more thrown (rising), eighteen
//! grey facing rings, eighteen red spark bursts and two pink arcs. Returns
//! 1 on update 0x3A, 4 on 0x73, 2 on 0x44, 8 once the 0x78-update timer
//! D+0xB210 has run out.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const FLARE: u32 = 0x01E0_1B10;
const THROWN_A: u32 = 0x01E0_1C90;
const THROWN_B: u32 = 0x01E0_1E40;
const ARC: u32 = 0x01E0_1FF0;
const RING: u32 = 0x01E0_20E0;
const SPARK: u32 = 0x01E0_2340;
const FLASH: u32 = 0x01E0_2540;
const IDLE: u32 = 0x01E0_2790;
/// The return address of the entry's `jalr` into a step (0x1E02AA8 + 8).
const TASK_RA: u64 = 0x01E0_2AB0;
const POS: u32 = D + 0x250;
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
/// 0.6 and 8.6 (the flares' sway and throw).
const SWAY: u32 = 0x3F19_999A;
const THROW: u32 = 0x4109_999A;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// (rand >> 4) % m (HI).
fn rand4_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v >> 4, m).1
}

/// The entry 0x1E027A0 (D, param).
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
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x78);
    for (k, node) in [2, 0x34, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    for k in 0xE..0x10u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x298 + 4 * (k - 0xE), t);
    }
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_2990);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_29C0, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_2A14);
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

/// D+0x20..+0x28 = the task's point.
fn centre(fx: &mut Fx, t: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(D + 0x20 + 4 * k, w);
    }
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
        let (frame, ra_at) = match f {
            FLARE | THROWN_A | THROWN_B => (0x30, 0x20),
            FLASH => (0x80, 0x10),
            _ => (0x20, 0x10),
        };
        let s = fx.enter(frame);
        if ra_at == 0x10 {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        }
        match f {
            FLARE => flare(fx, t),
            THROWN_A => thrown(fx, t, false),
            THROWN_B => thrown(fx, t, true),
            ARC => arc(fx, t),
            RING => ring(fx, t),
            SPARK => sparks(fx, t),
            FLASH => flash(fx, t, s),
            _ => panic!("b418: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        // (step, local, mode): the flares additive at 0x20, the thrown 0x10.
        for (g, local, mode) in [(FLARE, 0x50u32, 0x20), (THROWN_A, 0x60, 0x10), (THROWN_B, 0x70, 0x10)] {
            if f(fx) == g {
                tex(fx, TEX);
                let c = fx.mem.i16(t + 0x56);
                cell(fx, sp + local, 0x14, c);
                fx.mem.set16(D + 0xBE, 0);
                billboard::quad(fx, uv::uv_rect, D, t, mode, t + 0x10);
            }
        }
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == RING {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x14, c);
            fx.mem.set16(D + 0xC0, 0);
            tex(fx, TEX + 4);
            fx.mem.set16(D + 0xB8, 0xA);
            centre(fx, t);
            fx.mem.set16(D + 0xBE, 0);
            billboard::ring_facing(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == SPARK {
            let kind = fx.mem.i16(t + 0x50);
            if kind == 0 {
                let c = fx.mem.i16(t + 0x56);
                cell(fx, sp + 0x90, 0x96, c);
                tex(fx, TEX + 8);
                for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x7F)] {
                    fx.mem.set16(D + o, v);
                }
            }
            if fx.mem.i16(t + 0x50) == 1 {
                let c = fx.mem.i16(t + 0x56);
                cell(fx, sp + 0xA0, 0x96, c);
                tex(fx, D + 0x29C);
            }
            fx.mem.set16(D + 0xB8, 0);
            centre(fx, t);
            fx.mem.set16(D + 0xB8, 0xC);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x3A => 1,
        0x73 => 4,
        0x44 => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E02F10(D, step, 0): b750's spawn (the same function); its frame's
/// saves the emitter's s0 / s1 / s2 = D, the runner's s3 / s4 (0 [H]) and
/// the return address `ra`.
fn spawn(fx: &mut Fx, step: u32, s0: u32, s1: u32, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(s0)), (0x10, u64::from(s1)), (0x20, u64::from(D)), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
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

fn node0(fx: &mut Fx) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    fx.node_position(h, slot, POS);
}

/// 0x1E018B0(D, 0): the update counter and the emitters (frame 0x40: ra,
/// the entry's s2 = param, s1 = D, s0 = 0; then s2 = D and s0 / s1 the
/// fans' counts and angles).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_2A70), (0x20, param as i64 as u64), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let (mut s0, mut s1) = (0u32, D);
    if (0x3E..0x53).contains(&c(fx)) {
        node0(fx);
        spawn(fx, FLARE, s0, s1, 0x01E0_193C);
        spawn(fx, FLARE, s0, s1, 0x01E0_1950);
    }
    if c(fx) == 0x41 {
        node0(fx);
        for k in 0..0x12u32 {
            fx.mem.set16(SPAWN_A, (0x14 * k) as i32);
            spawn(fx, THROWN_A, k, 0x14 * k, 0x01E0_19B4);
        }
        (s0, s1) = (0x12, 0x168);
    }
    if c(fx) == 0x44 {
        node0(fx);
        for k in 0..0x12u32 {
            fx.mem.set16(SPAWN_A, (0x14 * k) as i32);
            spawn(fx, THROWN_B, 0x14 * k, k, 0x01E0_1A4C);
            spawn(fx, RING, 0x14 * k, k, 0x01E0_1A60);
            spawn(fx, SPARK, 0x14 * k, k, 0x01E0_1A74);
        }
        (s0, s1) = (0x168, 0x12);
        spawn(fx, ARC, s0, s1, 0x01E0_1AB8);
        spawn(fx, ARC, s0, s1, 0x01E0_1ACC);
    }
    if c(fx) == 0x41 {
        spawn(fx, FLASH, s0, s1, 0x01E0_1AF8);
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

/// axis `a` += `k` x sin (or cos) of the i16 angle at `t + deg` degrees.
fn sway(fx: &mut Fx, t: u32, a: u32, deg: u32, cos: i32, k: f32) {
    let d = fx.mem.i16(t + deg);
    let s = l2::sin_deg(fx, SINES, d, cos);
    let v = ee::add(fx.mem.f(t + a), ee::mul(k, s));
    fx.mem.setf(t + a, v);
}

/// 0x1E01B10 (D, task): a white flare of size 150 (cell frame 2..4), a
/// random heading +0x5A: in by 0x60 under 0xC0 for 6 updates, then out
/// by 19; z / x += 0.6 sin / cos of the heading a update; faces the
/// camera.
fn flare(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x96);
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
        let v = rand4_mod(fx, t, 0x01E0_1B48, 3);
        fx.mem.set16(t + 0x56, v + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = rand4_mod(fx, t, 0x01E0_1B80, 0x168);
        fx.mem.set16(t + 0x5A, v);
    }
    fade(fx, t, 0x63, 6, 0xC0, 0x60, 0x15, 0x13);
    if fx.mem.i16(t + 0x54) != 0 {
        sway(fx, t, 0x18, 0x5A, 0, f32::from_bits(SWAY));
        sway(fx, t, 0x10, 0x5A, 1, f32::from_bits(SWAY));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1C60);
    age(fx, t);
}

/// 0x1E01C90 / 0x1E01E40 (`rising`) (D, task): a white flare of size 150
/// (cell frame 2..4) thrown 8.6 along the fan angle +0x5A (z, x) on its
/// first update, then drifting 3 a update along it (z or, `rising`, y, and
/// x); in by 0x60 under 0xC0 for 6 updates, then out by 19; faces the
/// camera.
fn thrown(fx: &mut Fx, t: u32, rising: bool) {
    let (rand_ra, angles_ra) = if rising { (0x01E0_1E78, 0x01E0_1FC8) } else { (0x01E0_1CC8, 0x01E0_1E18) };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x96);
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
        let v = rand4_mod(fx, t, rand_ra, 3);
        fx.mem.set16(t + 0x56, v + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        sway(fx, t, 0x18, 0x5A, 0, f32::from_bits(THROW));
        sway(fx, t, 0x10, 0x5A, 1, f32::from_bits(THROW));
    }
    fade(fx, t, 0x63, 6, 0xC0, 0x60, 0x15, 0x13);
    if fx.mem.i16(t + 0x54) != 0 {
        sway(fx, t, if rising { 0x14 } else { 0x18 }, 0x5A, 0, 3.0);
        sway(fx, t, 0x10, 0x5A, 1, 3.0);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), angles_ra);
    age(fx, t);
}

/// 0x1E01FF0 (D, task): b482's arc ring (0x1E02C20) with radius 230, outer
/// pink (0xFF, 0xCF, 0xCF), inner (0xFF, 0x80, 0x80): outer alpha in by
/// 0x60 under 0xC0 for 12 updates, then out by 15; turned by the camera's
/// angles (the first + 0x4000).
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4366_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xCF, 0xCF);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0x67, 0xC, 0xC0, 0x60, 0x11, 0xF);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_20B0);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E020E0 (D, task): a grey facing ring: quad size 30, cell frame
/// (rand >> 4) % 8, white, turns +0x5A / +0x5C random, +0xA 2 or 0, +0xC
/// the spawn kind; radius +0x50 5..44 growing by +0xE 45..114 a update,
/// +0x52 100..449, row +8 0..34, life +6 3..7; in by 0xC0 under 0xC0, out
/// by 15; grey 0x80 on odd ages, 0x60 on even.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41F0_0000);
        let v = fx.rand_from(t, 0x01E0_2108);
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let v = rmod(fx, t, 0x01E0_2140, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_215C, 0x168);
        fx.mem.set16(t + 0x5C, v);
        let v = fx.rand_from(t, 0x01E0_2178);
        fx.mem.set16(t + 0xA, if v % 2 != 0 { 2 } else { 0 });
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, k);
        let v = rmod(fx, t, 0x01E0_21B0, 0x28);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, 0x01E0_21D0, 0x15E);
        fx.mem.set16(t + 0x52, v + 0x64);
        let v = rand4_mod(fx, t, 0x01E0_21F0, 0x46);
        fx.mem.set16(t + 0xE, v + 0x2D);
        let v = rmod(fx, t, 0x01E0_2214, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_2230, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let life = fx.mem.i16(t + 6);
    fade(fx, t, 0x63, life, 0xC0, 0xC0, 0x11, 0xF);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let g = if fx.mem.i16(t + 0x54) % 2 == 1 { 0x80 } else { 0x60 };
    l2::rgb(fx, t + 0x60, g, g, g);
    age(fx, t);
}

/// 0x1E02340 (D, task): b735's spark burst (0x1E02850) in red: size 200
/// growing by (rand % 300 + 550) / 10 a update, +0x4C 0.3 of it; kind
/// (rand >> 4) % 2: 0 (0xC0, 0x80, 0x80) on black, 1 (0xFF, 0xA0, 0xA0)
/// on grey 0x30; +0x5A / +0x5C 30; 0x80 for 8 updates, out by 14.
fn sparks(fx: &mut Fx, t: u32) {
    let f = SPARK;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.setf(t + 0x4C, ee::sub(200.0, 58.0));
        let v = fx.rand_from(t, f + 0x40);
        fx.mem.set16(t + 0x50, (v >> 4) % 2);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xC0, 0x80, 0x80);
            l2::rgb(fx, t + 0x64, 0, 0, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xFF, 0xA0, 0xA0);
            l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        }
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 0x1E);
        let v = rmod(fx, t, f + 0xD8, 0x12C);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0x226));
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
        fade_out(fx, t, 0x63, 0x10, 0xE);
    }
    let s = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, s);
    fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3E99_999A), s));
    age(fx, t);
}

/// 0x1E02540 (D, task): a red (0x90, 0x7F, 0x7F) frame flash (its
/// descriptor at sp + 0x28, ST from +0x48 = 1): for 10 updates alpha up
/// by 0x13 under 0x60 and the ST up by 0.3 under 100, then alpha down by
/// 6 (the task ends under 8) and the ST down by 0.3 to 0.
fn flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for (k, c) in [0x90u8, 0x7F, 0x7F].into_iter().enumerate() {
        fx.mem.write_u8(sp + 0x38 + k as u32, c);
    }
    let a = fx.mem.read_u8(t + 0x63);
    fx.mem.write_u8(sp + 0x3B, a);
    fx.mem.write_u32(sp + 0x3C, 0x3F80_0000);
    for (at, v) in [(sp + 0x48, 0x6FF8u16), (sp + 0x58, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x40, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x44, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x50, ee::div(ee::sub(512.0, s), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(ee::sub(512.0, s), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let step = f32::from_bits(0x3E99_999A);
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0x60, 0x13);
        let s = fx.mem.f(t + 0x48);
        if s < 100.0 {
            fx.mem.setf(t + 0x48, ee::add(s, step));
        }
    } else {
        fade_out(fx, t, 0x63, 8, 6);
        let s = fx.mem.f(t + 0x48);
        if s <= 0.0 {
            fx.mem.write_u32(t + 0x48, 0);
        } else {
            fx.mem.setf(t + 0x48, ee::sub(s, step));
        }
    }
    age(fx, t);
}
