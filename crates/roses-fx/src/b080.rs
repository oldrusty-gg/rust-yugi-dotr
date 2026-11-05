//! BMODULE b080 (card 438), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03070, data area 0x1ED0000) [F]: b018's task program (card 56) on other globals: 150 tasks of 0x40
//! bytes at D+0x478 (alive +0x36, step +0x24); spawn 0x1E03A80 = b018's
//! 0x1E03640 on colour D+0x2CE4, parameter D+0x2D42 and the point D+0x2D0.
//! Four 10-point histories D+0x2A50 / D+0x2AF0 / D+0x2B90 / D+0x2C30 of the
//! attacker's nodes 0x1B / 0x1D / 0x1C (filled on updates 0xB2 / 0xB2 /
//! 0xC0 / 1, pushed by `spawn::node_history`). The emitter 0x1E01CE0 counts
//! D+0x2D46: eight drifting grey sprites 0x1E023C0 at node 0x30 on 0x1B /
//! 0x42 / 0x61 / 0x82 / 0xB9 / 0x10E and at node 0x39 on 0x2E / 0x50 / 0x72
//! / 0x9D / 0xB9 / 0x10E; a tube task 0x1E02670 on 0xB2 (between the
//! histories of nodes 0x1B / 0x1D); on 0xBA / 0xBB eight `ring_t20` ember
//! rings 0x1E02A90 (b018's ring) at node 0x1D (z the target's); on 0xBA
//! eight `tstep::spark` rings, twenty `tstep::ember` clouds, twenty grey
//! glints 0x1E02210, a `fan18` disc 0x1E02590 and the camera light
//! 0x1E02C60 (b018's at D+0x2A00). The steps 0x1E02130 / 0x2C00 /
//! 0x1E02DE0 / 0x2EA0 (object op) / 0x1E02EF0 and the draws 0x1E01960 /
//! 0x1E03BA0 / 0x1E01520 are tested but never spawned (not ported).
//! Returns 1 on update 0xB0, 4 on 0xDC, 2 on 0xBA, 8 once the 230-update
//! timer D+0x2CE0 has run out. No 0x1F8100.
use crate::b018::{cell, ring, run_step};
use crate::b076::unit_position_saving;
use crate::blib::{billboard, quads4, spawn as bspawn, tstep, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;

const GLINT: u32 = 0x01E0_2210;
const DRIFT: u32 = 0x01E0_23C0;
const DISC: u32 = 0x01E0_2590;
const TUBE: u32 = 0x01E0_2670;
const EMBER: u32 = 0x01E0_2750;
const SPARK: u32 = 0x01E0_2900;
const RING: u32 = 0x01E0_2A90;
const LIGHT: u32 = 0x01E0_2C60;

const TASKS: u32 = 150;
const POOL: u32 = D + 0x478;
const POS: u32 = D + 0x2D0;
const TEX: u32 = D + 0x340;
const LAMP: u32 = D + 0x2A00;
const KEEP: u32 = D + 0x2A40;
const HIST_A: u32 = D + 0x2A50;
const HIST_B: u32 = D + 0x2AF0;
const HIST_C: u32 = D + 0x2B90;
const HIST_D: u32 = D + 0x2C30;
const WAVE: u32 = D + 0x2CD0;
const TIMER: u32 = D + 0x2CE0;
const COLOUR: u32 = D + 0x2CE4;
const OT2: u32 = D + 0x2CEC;
const NODES: u32 = D + 0x2D28;
const TARGET_NODE: u32 = D + 0x2D36;
const ANGLE: u32 = D + 0x2D3E;
const PARAM: u32 = D + 0x2D42;
const PARAM_COPY: u32 = D + 0x2D44;
const COUNTER: u32 = D + 0x2D46;
const BYTES: u32 = D + 0x2D4A;
const SINES: u32 = D + 0x2E4C;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E03070 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1C0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0x1C0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x2FC0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(PARAM_COPY, param);
    fx.mem.set32(TIMER, 0xE6);
    for o in [0u32, 4, 8] {
        fx.mem.write_u32(D + 0x310 + o, 0);
    }
    for (k, (n, clear)) in [(0x1B, 1), (0x1D, 0), (0x1C, 0), (0x30, 0), (0x39, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 0);
    fx.mem.set16(TARGET_NODE, r);
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    for k in 0xB..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(TEX + 0x2C + 4 * (k as u32 - 0xB), t);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_31CC);
        let r = fx.div(v, 0x1E).1;
        fx.mem.write_u8(BYTES + k, (r - 0xF) as u8);
    }
    0
}

/// A history's nine points from `node`'s position (through `local`).
fn fill(fx: &mut Fx, node: u32, local: u32, hist: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(node);
    fx.node_position(h, n, local);
    for i in 0..9 {
        for k in 0..3 {
            let w = fx.mem.read_u32(local + 4 * k);
            fx.mem.write_u32(hist + 0x10 * i + 4 * k, w);
        }
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    if counter(fx) == 0xB2 {
        fill(fx, NODES + 2, sp + 0xE0, HIST_B);
    }
    if counter(fx) == 0xB2 {
        fill(fx, NODES, sp + 0xF0, HIST_A);
    }
    if counter(fx) == 0xC0 {
        fill(fx, NODES + 4, sp + 0x100, HIST_C);
    }
    if counter(fx) == 1 {
        fill(fx, NODES + 4, sp + 0x110, HIST_D);
    }
    for (hist, node) in [(HIST_B, 4u32), (HIST_A, 2), (HIST_C, 4), (HIST_D, 4)] {
        let n = fx.mem.i16(NODES + node);
        bspawn::node_history(fx, hist + 0x90, 0xA, n);
    }
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            GLINT => glint(fx, t),
            DRIFT => drift(fx, t),
            DISC => {
                let c = bspawn::disc("5225257f1409").expect("constants");
                bspawn::disc_step_with(fx, c, DISC, t);
            }
            TUBE => tube_step(fx, t),
            EMBER => {
                tstep::ember(fx, EMBER, t);
            }
            SPARK => {
                tstep::spark(fx, SPARK, t);
            }
            RING => run_step(fx, t, 0x20, 0x01E0_34C0, |fx, t| ring(fx, RING, t)),
            LIGHT => run_step(fx, t, 0x30, 0x01E0_3530, light),
            f => {
                debug_assert!(false, "b080: unported task step {f:#x}");
            }
        }
    }
    let h = fx.target_handle();
    // Its frame's ra slot (the upper word zero) is the garbage half of the
    // first draw's atlas rect.
    unit_position_saving(fx, h, sp + 0x70, u64::from(D), 0x01E0_35E4);
    let d = sp + 0x128;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            GLINT => {
                glint_draw(fx, t, d, 0);
                glint_draw(fx, t, d, 1);
            }
            DRIFT => {
                cell(fx, t, TEX + 8, sp + 0x80, 0x14);
                crate::b709::drift_draw(fx, 0x01E0_05A0, t, d);
                crate::b709::drift_draw(fx, 0x01E0_05A0, t, d);
            }
            DISC => {
                cell(fx, t, TEX + 0x3C, sp + 0x90, 0x9B);
                quads4::fan18(fx, D, t, d);
            }
            TUBE => {
                let w = fx.mem.i16(t + 0x30);
                if w == 0 {
                    fx.mem.write_u32(D + 4, 0);
                }
                if w == 1 {
                    fx.mem.write_u32(D + 4, 0x3F80_0000);
                }
                fx.mem.write_u32(D, 0x4020_0000);
                fx.mem.write_u32(D + 8, 0x4020_0000);
                if fx.mem.i16(t + 0x30) == 0 {
                    tube(fx, t, HIST_A, HIST_B);
                }
                if fx.mem.i16(t + 0x30) == 1 {
                    tube(fx, t, HIST_A, HIST_C);
                }
            }
            EMBER => {
                cell(fx, t, TEX + 0x3C, sp + 0xA0, 0x9B);
                billboard::ring_t(fx, uv::uv_rect4, D, t, d, BYTES);
            }
            SPARK => {
                cell(fx, t, TEX + 0x3C, sp + 0xB0, 0x9B);
                billboard::ring_t20(fx, uv::uv_rect4, D, t, d, BYTES);
            }
            RING => {
                cell(fx, t, TEX + 0x3C, sp + 0xC0, 0x9B);
                billboard::ring_t20(fx, uv::uv_rect4, D, t, d, BYTES);
            }
            _ => {}
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0xB0 => 1,
        0xDC => 4,
        0xBA => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

fn rm(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

/// Alpha `held` while younger than `life`, then down by `by` (over below
/// `floor`).
fn hold_fade(fx: &mut Fx, t: u32, held: u8, life: i32, floor: u8, by: u8) {
    if fx.mem.i16(t + 0x32) < life {
        fx.mem.write_u8(t + 0x2B, held);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= floor {
            fx.mem.write_u8(t + 0x2B, a - by);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
}

/// 0x1E02210 (D, task): a grey glint (size 10, cell 0, (0x30, 0x30,
/// 0x30), +0x30 / +0x3A rand % 360, speed +0xE rand % 15 + 3); 0x70 for
/// 10 updates, then out by 10; moving by speed x sin +0x3A (x), cos +0x30
/// (y), cos +0x3A (z).
fn glint(fx: &mut Fx, t: u32) {
    let at = GLINT;
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xA);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x30, 0x30, 0x30);
        let v = rm(fx, t, at + 0x4C, 0x168);
        fx.mem.set16(t + 0x30, v);
        let v = rm(fx, t, at + 0x68, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rm(fx, t, at + 0x84, 0xF);
        fx.mem.set16(t + 0xE, v + 3);
        fx.mem.set16(t + 0x36, 2);
    }
    hold_fade(fx, t, 0x70, 0xA, 0x11, 0xA);
    age(fx, t);
    for (o, deg, cos, ra) in [(8u32, 0x3Au32, 0, 0x124u32), (0xA, 0x30, 1, 0x15C), (0xC, 0x3A, 1, 0x194)] {
        let a = fx.mem.i16(t + deg);
        let s = l2::sin_deg(fx, SINES, a, cos);
        let k = ee::from_i32(fx.mem.i16(t + 0xE));
        let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(k, s));
        let r = fix(fx, v, at + ra);
        fx.mem.set16(t + o, r);
    }
    fx.leave(0x30);
}

/// 0x1E023C0 (D, task): b709's drifting grey sprite 0x1E04220 with size
/// 65, colour (0x10, 0x10, 0x10), alpha 0x70 for 10 updates (then out by
/// 10), drifting by 10 sin +0x3A (x), 4 cos +0x30 (y), 10 cos +0x3A (z).
fn drift(fx: &mut Fx, t: u32) {
    let at = DRIFT;
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x41);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x10);
        let v = rm(fx, t, at + 0x4C, 0x168);
        fx.mem.set16(t + 0x30, v);
        let v = rm(fx, t, at + 0x68, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    hold_fade(fx, t, 0x70, 0xA, 0x11, 0xA);
    if fx.mem.i16(t + 0x32) % 3 == 1 && fx.mem.i16(t + 0x34) < 7 {
        let v = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, v);
    }
    age(fx, t);
    for (o, deg, cos, k, ra) in [(8u32, 0x3Au32, 0, 10.0f32, 0x140u32), (0xA, 0x30, 1, 4.0, 0x178), (0xC, 0x3A, 1, 10.0, 0x1B0)] {
        let a = fx.mem.i16(t + deg);
        let s = l2::sin_deg(fx, SINES, a, cos);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(k, s));
        let r = fix(fx, v, at + ra);
        fx.mem.set16(t + o, r);
    }
    fx.leave(0x30);
}

/// 0x1E02670 (D, task): the tube's step: colour (0x80, 0x40, 8) every
/// update, width 5, life +0xE 10 (kind +0x30 0) / 7 (kind 1); alpha 0x80
/// for the life, then down by 0x1D (over below 0x1F); +6 = 10 points.
fn tube_step(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    l2::rgb(fx, t + 0x28, 0x80, 0x40, 8);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 5);
        if fx.mem.i16(t + 0x30) == 0 {
            fx.mem.set16(t + 0xE, 0xA);
        }
        if fx.mem.i16(t + 0x30) == 1 {
            fx.mem.set16(t + 0xE, 7);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    let life = fx.mem.i16(t + 0xE);
    hold_fade(fx, t, 0x80, life, 0x1F, 0x1D);
    fx.mem.set16(t + 6, 0xA);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02C60: b018's camera light 0x1E02B70 on D+0x2A00 and the timer
/// D+0x2CE0.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0x4000_0000), (0xC, 0), (0x20, 0), (0x24, 0x42F0_0000), (0x28, 0)] {
            fx.mem.write_u32(LAMP + o, v);
        }
        for k in 0..3 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(LAMP + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    let lv = fx.mem.f(LAMP + 0x28);
    if fx.mem.i16(t + 0x30) == 0 {
        if lv < 1.0 {
            fx.mem.setf(LAMP + 0x28, ee::add(lv, f32::from_bits(0x3E4C_CCCD)));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else {
        let tenth = f32::from_bits(0x3DCC_CCCD);
        if lv <= tenth {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.setf(LAMP + 0x28, ee::sub(lv, tenth));
        }
    }
    let p = if fx.mem.i16(t + 0x36) == 0 { 0 } else { LAMP };
    crate::blib::exe::camera_slots45(fx, p);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E03D70 (D, task, desc, mode): mode 0 sets `desc` up (a strip3,
/// sprite 0x83 frame +0x34, colour +0x28, texture D+0x374); mode 1 draws
/// the camera-facing square of half size i16 +0x38 as its four quarters
/// (scale 0.1) at the task's point + its offsets / 10, each keyed by its
/// mean w (z offset: the y offset / 100).
fn glint_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x130);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xC0, 0x83, c);
        l2::desc_strip3(fx, d);
        l2::colour_strip3(fx, d, t + 0x28, 0);
        l2::uv_strip3(fx, d, sp + 0xC0, 0);
        let w = fx.mem.read_u32(D + 0x374);
        fx.desc_texture(d, w, 1);
    } else {
        for k in 0..3 {
            fx.mem.write_u32(sp + 0xA0 + 4 * k, 0x3DCC_CCCD);
        }
        l2::camera_angles_to(fx, sp + 0x120);
        // z takes the y offset divided by 10 a second time (the record's
        // register reuse), not the z offset.
        let ox = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
        let oy = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
        let oz = ee::div(oy, 10.0);
        for (k, off) in [(0u32, ox), (1, oy), (2, oz)] {
            let p = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0xB0 + 4 * k, ee::add(p, off));
        }
        l2::xform_vp(fx, sp + 0xD0, sp + 0xA0, sp + 0x120, sp + 0xB0);
        for k in 0..4 {
            let s = fx.mem.i16(t + 0x38);
            l2::quad_ints(fx, sp + 0x60, s, s, k);
            if l2::project(fx, sp + 0x60, SPR, sp + 0xD0, d, sp + 0x110) {
                let w = fx.mem.f(sp + 0x110);
                fx.fix_residue(w, 0x01E0_3F14);
                let key = ee::fixunssfsi(w) as i32;
                fx.submit(d, OT, key);
            }
        }
    }
    fx.leave(0x130);
}

/// 0x1E010C0 (D, task, p, q): b701's tube 0x1E037D0 with seven rings 30
/// degrees apart (from -90) and no far Z (key 0): along the n = i16 +6
/// points `p`, 4 gouraud strips a ring of radius D+0 (x) / D+8 (z), the y
/// offset -(p[0].y - q[0].y), each strip nudged 0.2; the colour +0x28 at
/// the first two vertices, its alpha less alpha / n at the last two.
fn tube(fx: &mut Fx, t: u32, p: u32, q: u32) {
    let sp = fx.enter(0x220);
    let d = sp + 0x188;
    let n = fx.mem.i16(t + 6);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    for k in 0..4 {
        let b = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(sp + 0x21C + k, b);
    }
    let a = sp + 0x160;
    for k in 0..4 {
        let w = fx.mem.read_u32(p + 4 * k);
        fx.mem.write_u32(a + 4 * k, w);
        let w = fx.mem.read_u32(q + 4 * k);
        fx.mem.write_u32(sp + 0x170 + 4 * k, w);
    }
    for c in 0..3 {
        let v = ee::sub(fx.mem.f(a + 4 * c), fx.mem.f(sp + 0x170 + 4 * c));
        fx.mem.setf(a + 4 * c, v);
    }
    let w = fx.mem.read_u32(D);
    fx.mem.write_u32(a, w);
    let y = ee::neg(fx.mem.f(a + 4));
    fx.mem.setf(a + 4, y);
    let w = fx.mem.read_u32(D + 8);
    fx.mem.write_u32(a + 8, w);
    let f23 = fx.mem.f(a + 8);
    let f24 = fx.mem.f(a + 4);
    let mut seg = p;
    let mut i = 0;
    while i < n - 1 {
        l2::colour_strip4(fx, d, sp + 0x21C, 1);
        let alpha = i32::from(fx.mem.read_u8(t + 0x2B));
        let q = fx.div(alpha, n).0 & 0xFF;
        let v = fx.mem.read_u8(sp + 0x21F).wrapping_sub(q as u8);
        fx.mem.write_u8(sp + 0x21F, v);
        l2::colour_strip4(fx, d, sp + 0x21C, 2);
        let mut deg = -0x5Ai32;
        for _ in 0..7 {
            let s = l2::sin_deg(fx, D + 0xD8, deg as i16 as i32, 0);
            let f21 = ee::mul(fx.mem.f(a), s);
            let c = l2::sin_deg(fx, D + 0xD8, deg as i16 as i32, 1);
            let f22 = ee::mul(f23, c);
            for s0 in 0..4 {
                let m = sp + 0x120;
                for (row, from) in [(0u32, seg), (0x10, seg), (0x20, seg + 0x10), (0x30, seg + 0x10)] {
                    for c in 0..3 {
                        let w = fx.mem.read_u32(from + 4 * c);
                        fx.mem.write_u32(m + row + 4 * c, w);
                    }
                }
                l2::add_column(fx, m, 0, f21);
                l2::add_column(fx, m, 2, f22);
                l2::add_column(fx, m, 1, f24);
                let nudge = f32::from_bits(0x3E4C_CCCD);
                let (o, up) = match s0 {
                    0 => (8u32, true),
                    1 => (8, false),
                    2 => (0, true),
                    _ => (0, false),
                };
                for row in [0u32, 0x20] {
                    let at = m + row + o;
                    let v = fx.mem.f(at);
                    fx.mem.setf(at, if up { ee::add(v, nudge) } else { ee::sub(v, nudge) });
                }
                if l2::project(fx, m, SPR, 0, d, sp + 0x110) {
                    fx.submit(d, OT, 0);
                }
            }
            deg += 0x1E;
        }
        seg += 0x10;
        i += 1;
    }
    fx.leave(0x220);
}

/// 0x1E01CE0(D, 0) (frame 0x50: ra +0x20, s1 +0x10, s0 = D): the spawn
/// colour (0x80, 0, 0, 0), the update counter and the spawns; D+0x2CD0 =
/// 100 sin of the angle D+0x2D3E (up by 5 a update, mod 360).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_3208), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    let self_node = |fx: &mut Fx, node: u32, out: u32| {
        let h = fx.self_handle();
        let n = fx.mem.i16(node);
        fx.node_position(h, n, out);
    };
    if counter(fx) == 0x33 {
        self_node(fx, NODES + 2, KEEP);
    }
    let c = counter(fx);
    if (0x34..0x41).contains(&c) {
        self_node(fx, NODES + 2, sp + 0x30);
        for k in 0..3 {
            let w = fx.mem.read_u32(KEEP + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    }
    let mut s1 = 0u64;
    if counter(fx) == 0xB2 {
        fx.mem.set16(PARAM, 0);
        spawn(fx, TUBE, 0x01E0_1DC0, s1);
    }
    if counter(fx) == 0xC0 {
        fx.mem.set16(PARAM, 1);
    }
    for (node, at, ra) in [(NODES + 6, [0x1B, 0x42, 0x61, 0x82, 0xB9, 0x10E], 0x01E0_1E74u32), (NODES + 8, [0x2E, 0x50, 0x72, 0x9D, 0xB9, 0x10E], 0x01E0_1F24)] {
        if at.contains(&counter(fx)) {
            self_node(fx, node, sp + 0x30);
            for k in 0..3 {
                let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
                fx.mem.write_u32(POS + 4 * k, w);
            }
            fx.mem.set16(PARAM, 1);
            for k in 0..8u64 {
                s1 = k;
                spawn(fx, DRIFT, ra, s1);
            }
        }
    }
    let at = |fx: &mut Fx| {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 2);
        fx.node_position(h, n, sp + 0x30);
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x40);
        for (to, from) in [(0u32, 0x30u32), (4, 0x34), (8, 0x48)] {
            let w = fx.mem.read_u32(sp + from);
            fx.mem.write_u32(POS + to, w);
        }
        fx.mem.set16(PARAM, 1);
    };
    let c = counter(fx);
    if c == 0xBA || c == 0xBB {
        at(fx);
        for k in 0..8u64 {
            s1 = k;
            spawn(fx, RING, 0x01E0_1FB8, s1);
        }
    }
    if counter(fx) == 0xBA {
        at(fx);
        for k in 0..8u64 {
            s1 = k;
            spawn(fx, SPARK, 0x01E0_203C, s1);
        }
        for k in 0..0x14u64 {
            s1 = k;
            spawn(fx, EMBER, 0x01E0_2064, s1);
        }
        for k in 0..0x14u64 {
            s1 = k;
            spawn(fx, GLINT, 0x01E0_208C, s1);
        }
        s1 = 0x14;
        spawn(fx, DISC, 0x01E0_20B0, s1);
        spawn(fx, LIGHT, 0x01E0_20C4, s1);
    }
    let a = fx.mem.i16(ANGLE);
    let s = l2::sin_deg(fx, SINES, a, 0);
    fx.mem.setf(WAVE, ee::mul(100.0, s));
    let a = fx.mem.i16(ANGLE) + 5;
    fx.mem.set16(ANGLE, a);
    let a = fx.mem.i16(ANGLE);
    if a >= 0x168 {
        let r = fx.div(a, 0x168).1;
        fx.mem.set16(ANGLE, r);
    }
    fx.leave(0x50);
}

/// 0x1E03A80(D, step, 0) = b018's spawn 0x1E03640 (frame 0x60: ra +0x50,
/// s4 / s3 / s2 the entry's, s1 the emitter's, s0 = D) on this record's
/// globals.
fn spawn(fx: &mut Fx, f: u32, ra: u32, s1: u64) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, u64::MAX), (0x10, s1), (0, u64::from(D))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x36 + 0x40 * k) == 0) {
        Some(k) => {
            let t = POOL + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            let a = fx.mem.i16(PARAM);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}
