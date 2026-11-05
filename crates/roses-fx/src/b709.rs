//! BMODULE b709 (card 596), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04730, data area 0x1ED0000, 0x3830 bytes) [F]
//!: b636's older pool (180 tasks of 0x40 at
//! D+0x478, alive +0x36 = D+0x4AE + 0x40 k, step +0x24, spawned by
//! 0x1E054A0 = b636's spawn on colour D+0x3364 / kind D+0x33BA) with the
//! globals at D+0x3360.. (timer D+0x3360 = 0x116, OT D+0x336C, nodes
//! D+0x33A8.. (0x1B / 0x19 / 0x29 / 0x28, 0x42 / 0x49 / 0x64), counter
//! D+0x33BE, tables D+0x33C2 / 0x34C2 / 0x35C2, sines D+0x36C4). The
//! emitter 0x1E02300 follows the attacker's nodes 0x1B / 0x29 (D+0x31B0 /
//! 0x31C0) and the target's 0x19 / 0x28 (D+0x320 / 0x330), their gaps at
//! D+0x20 / D+0x30; every update to 0xC4 a node-0x1B spark ring (0x38C0,
//! drawn as a strip ring 0x1E059F0 pushed along the gap) and a spark
//! (0x3090, riding the node's point, `billboard::ring_t` + the strip ring)
//! and to 0xBE the same at node 0x29; 0xB0..0xCA odd two domes (0x2E10,
//! 0x1E055C0 twelve quads between the two points); 0xB1..0xC3 blue rings
//! (0x3260, `quads4::ring_into`); 0xB9..0xBE even / 0xC2..0xC4 odd rings
//! 0x3480 / 0x35F0; 0xBE / 0xC4 a disc (`spawn::disc_step` 5225257f1409,
//! `quads4::fan18`), three grey rings 0x3B20 (`ring_t`) and ten strip
//! rings (`task40::ring_strip`, `billboard::ring_strip3`); 0x79 / 0x84 /
//! 0x8E / 0x9B / 0xB1 / 0xB7 eight drifting sprites (0x4220, 0x1E005A0) at
//! node 0x49 / 0x64; update 1 the cross ribbon (0x3CA0, 0x1E01CF0) along
//! node 0x42's ten-point history D+0x3290. Returns 1 on update 0xB4, 4 on
//! 0xD2, 2 on 0xC8, 6 on 0xBF..0xC2, 8 once the timer has run out.
//!
//! The steps 0x1E02BF0, 0x33D0, 0x3750, 0x3E20, 0x43F0, 0x4570 and the
//! object op 0x4630 are tested by the entry but never spawned [F: the
//! emitter names 0x2E10 / 0x3090 / 0x3260 / 0x3480 / 0x35F0 / 0x38C0 /
//! 0x3B20 / 0x3CA0 / 0x3F90 / 0x4140 / 0x4220 only]; not ported. No
//! 0x1F8100.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b148::{facing_of, rmod};
use crate::blib::{billboard, quads4, spawn, spline, task40, uv};
use crate::bus::Bus;
use crate::c061::{desc_uv4, rgbaq_at, to_degrees_neg};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 180;
const TIMER: u32 = D + 0x3360;
const COLOUR: u32 = D + 0x3364;
const OT2: u32 = D + 0x336C;
const NODES: u32 = D + 0x33A8;
const KIND: u32 = D + 0x33BA;
const COUNTER: u32 = D + 0x33BE;
const TABLE: u32 = D + 0x33C2;
const TABLE2: u32 = D + 0x34C2;
const TABLE3: u32 = D + 0x35C2;
const SINES: u32 = D + 0x36C4;
const SPR: u32 = 0x7000_0000;

const DOME: u32 = 0x01E0_2E10;
const SPARK: u32 = 0x01E0_3090;
const BLUE: u32 = 0x01E0_3260;
const PURPLE: u32 = 0x01E0_3480;
const ORANGE: u32 = 0x01E0_35F0;
const STRIPS: u32 = 0x01E0_38C0;
const GREY: u32 = 0x01E0_3B20;
const CROSS: u32 = 0x01E0_3CA0;
const STRIP: u32 = 0x01E0_3F90;
const DISC: u32 = 0x01E0_4140;
const DRIFT: u32 = 0x01E0_4220;

/// The offsets / constants b709's steps and draws take from their record
/// (b225, card 140, runs the same code on globals shifted by +0x28 / +0x2A
/// with a few constants changed [F fdiff]).
pub(crate) struct Rec {
    /// The update counter (i16).
    pub counter: u32,
    /// The dome draw's sine table, the OT word it submits into and its key
    /// word.
    pub sines: u32,
    pub ot2: u32,
    pub key: u32,
    /// The dome's +0x30 step before its slow-down (2 after).
    pub dome_step: i32,
    /// The dome draw's two rows' heights swapped (b225: 30 sin (+0x30 + 8)
    /// first).
    pub rows_swapped: bool,
    /// The orange strip ring's +0xE base and fade.
    pub orange_e: i32,
    pub orange_fade: u8,
    /// The grey ring's size and +0xE base.
    pub grey_size: i32,
    pub grey_e: i32,
}

const B709: Rec = Rec { counter: COUNTER, sines: SINES, ot2: OT2, key: D + 0x3394, dome_step: 8, rows_swapped: false, orange_e: 6, orange_fade: 0xA, grey_size: 0x10, grey_e: 0xA };

/// The entry 0x1E04730 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1E0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x1E0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3830);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x33BC, param);
    fx.mem.set32(TIMER, 0x116);
    for k in 0..3 {
        fx.mem.write_u32(D + 0x310 + 4 * k, 0);
    }
    for (o, n) in [(0u32, 0x1B), (2, 0x19), (4, 0x29), (6, 0x28), (0xA, 0x42), (0xC, 0x49), (0xE, 0x64)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(o == 0));
        fx.mem.set16(NODES + o, r);
    }
    // 0x1E04680(0, D): the textures (frame 0x40: the entry's s0 = D, s1 =
    // 0, s2 = param, ra).
    let f = fx.enter(0x40);
    fx.saved(f, &[(0, u64::from(D)), (0x10, 0), (0x20, param as i64 as u64), (0x30, 0x01E0_4850)]);
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x340 + 4 * k as u32, t);
    }
    for k in 0xD..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x374 + 4 * (k as u32 - 0xD), t);
    }
    for k in 0..4 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0x3340 + 4 * k as u32, t);
    }
    fx.leave(0x40);
    let f = facing_of(fx, sp + 0x110, 0x01E0_488C);
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_489C, 0x14);
        fx.mem.set8(TABLE + k, v - 0xA);
    }
    for k in 0xB4..0x100u32 {
        let v = if f != 0 { rmod(fx, D, 0x01E0_48D8, 6) + 2 } else { -(rmod(fx, D, 0x01E0_4904, 6) + 2) };
        fx.mem.set8(TABLE + k, v);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_4944, 0x1E);
        fx.mem.set8(TABLE2 + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_4978, 0x1E) - 0xF;
        let v = match v {
            0..=4 => v + 7,
            -4..=-1 => v - 7,
            _ => v,
        };
        fx.mem.set8(TABLE3 + k, v);
    }
    0
}

/// The step frames' saves: the entry's s0..s(n-1) (D, 0, the slot's D +
/// 0x40 k) and the return address `ra` above them.
fn frame(fx: &mut Fx, low: u32, size: u32, n: u32, k: u32, ra: u32) {
    let vals = [u64::from(D), 0, u64::from(D + 0x40 * k)];
    let base = low - size;
    for i in 0..n {
        fx.saved(base + 0x10 * i, &[(0, vals[i as usize])]);
    }
    fx.saved(base, &[(0x10 * n, u64::from(ra))]);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    let dd = u64::from(D);
    let c = fx.mem.i16(COUNTER);
    if c == 1 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 0xA);
        fx.node_position(h, slot, sp + 0x120);
        for i in 0..9 {
            for k in 0..3 {
                let w = fx.mem.read_u32(sp + 0x120 + 4 * k);
                fx.mem.write_u32(D + 0x3290 + 0x10 * i + 4 * k, w);
            }
        }
    }
    if fx.mem.i16(COUNTER) == 0x18 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2);
        fx.node_position(h, slot, sp + 0x130);
        for i in 0..9 {
            for k in 0..3 {
                let w = fx.mem.read_u32(sp + 0x130 + 4 * k);
                fx.mem.write_u32(D + 0x31F0 + 0x10 * i + 4 * k, w);
            }
        }
    }
    // The entry's s2 here: the last history filled (param when none).
    let c = fx.mem.i16(COUNTER);
    let s2 = if c >= 0x18 { u64::from(D + 0x3280) } else { u64::from(D + 0x3320) };
    for (p, o, ra) in [(0x3320u32, 0xAu32, 0x01E0_4AC8u32), (0x3280, 2, 0x01E0_4AD8)] {
        let n = fx.mem.i16(NODES + o);
        spawn::node_history(fx, D + p, 0xA, n);
        fx.saved(fx.sp - 0x70, &[(0, dd), (0x10, 0), (0x20, s2), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
    }
    let low = fx.sp;
    for k in 0..TASKS {
        let t = D + 0x478 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            DOME => {
                dome(fx, &B709, DOME, t);
                frame(fx, low, 0x50, 3, k, 0x01E0_4B30);
            }
            SPARK => {
                spark(fx, t);
                frame(fx, low, 0x30, 2, k, 0x01E0_4B50);
            }
            BLUE => {
                blue(fx, BLUE, t);
                frame(fx, low, 0x20, 1, k, 0x01E0_4B70);
            }
            PURPLE => {
                purple(fx, t);
                frame(fx, low, 0x20, 1, k, 0x01E0_4C30);
            }
            ORANGE => {
                orange(fx, &B709, ORANGE, t);
                frame(fx, low, 0x20, 1, k, 0x01E0_4C50);
            }
            STRIPS => {
                strips(fx, &B709, STRIPS, t);
                frame(fx, low, 0x30, 2, k, 0x01E0_4C90);
            }
            GREY => {
                grey(fx, &B709, GREY, t);
                frame(fx, low, 0x20, 1, k, 0x01E0_4CB0);
            }
            CROSS => {
                cross(fx, t);
                frame(fx, low, 0x20, 1, k, 0x01E0_4CD0);
            }
            STRIP => {
                task40::ring_strip(fx, STRIP, D, t);
                frame(fx, low, 0x20, 1, k, 0x01E0_4D10);
            }
            DISC => {
                let c = spawn::disc("5225257f1409").expect("constants");
                spawn::disc_step_with(fx, c, DISC, t);
                frame(fx, low, 0x20, 1, k, 0x01E0_4D30);
            }
            DRIFT => {
                drift(fx, t);
                frame(fx, low, 0x30, 2, k, 0x01E0_4D50);
            }
            f => {
                debug_assert!(false, "b709: unported task step {f:#x}");
            }
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x50, dd, 0x01E0_4E04);
    let d = sp + 0x148;
    for k in 0..TASKS {
        let t = D + 0x478 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        fx.mem.set16(D + 0xB8, 0);
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        let set_rect = |fx: &mut Fx, tex: u32, local: u32, id: i32| {
            let w = fx.mem.read_u32(D + tex);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            l2::rect(fx, local, id, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
        };
        if f(fx) == DOME {
            dome_draw(fx, &B709, t, d, 0);
            dome_draw(fx, &B709, t, d, 1);
        }
        if f(fx) == SPARK {
            set_rect(fx, 0x37C, sp + 0x60, 0x9B);
            fx.mem.set16(D + 0xB8, 3);
            strip_ring(fx, 0x01E0_59F0, t, d, TABLE);
            fx.mem.set16(D + 0xB8, 4);
            if fx.mem.i16(COUNTER) < 0xB6 {
                billboard::ring_t(fx, uv::uv_rect4, D, t, d, TABLE);
            }
        }
        if f(fx) == BLUE {
            set_rect(fx, 0x37C, sp + 0x70, 0x9B);
            fx.mem.set16(D + 0xB8, 4);
            quads4::ring_into(fx, uv::uv_rect4, D, t, d, TABLE);
        }
        if f(fx) == PURPLE {
            set_rect(fx, 0x3340, sp + 0x80, 0x14);
            fx.mem.set16(D + 0xB8, 4);
            quads4::ring_into(fx, uv::uv_rect4, D, t, d, TABLE);
        }
        if f(fx) == ORANGE {
            set_rect(fx, 0x344, sp + 0x90, 0xA);
            fx.mem.set16(D + 0xB8, 6);
            strip_ring(fx, 0x01E0_59F0, t, d, TABLE);
        }
        if f(fx) == STRIPS {
            set_rect(fx, 0x344, sp + 0xB0, 0xA);
            fx.mem.set16(D + 0xB8, 0xA);
            strip_ring(fx, 0x01E0_59F0, t, d, TABLE);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == GREY {
            set_rect(fx, 0x334C, sp + 0xC0, 0x14);
            fx.mem.set16(D + 0xB8, 0xF);
            billboard::ring_t(fx, uv::uv_rect4, D, t, d, TABLE2);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == CROSS {
            let w = fx.mem.read_u32(D + 0x37C);
            fx.mem.write_u32(D + 0x88, w);
            set_rect(fx, 0x334C, sp + 0xD0, 0x14);
            for (o, v) in [(0x94u32, 0xF), (0x96, 0x10), (0x98, 0xF), (0x9A, 1)] {
                fx.mem.set16(D + o, v);
            }
            cross_draw(fx, t, D + 0x3290);
        }
        if f(fx) == STRIP {
            set_rect(fx, 0x37C, sp + 0xF0, 0x9A);
            fx.mem.set16(D + 0xB8, 5);
            billboard::ring_strip3(fx, D, t, d, TABLE3);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == DISC {
            quads4::fan18(fx, D, t, d);
        }
        if f(fx) == DRIFT {
            set_rect(fx, 0x348, sp + 0x100, 0x14);
            drift_draw(fx, 0x01E0_05A0, t, d);
            drift_draw(fx, 0x01E0_05A0, t, d);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xB4 => 1,
        0xD2 => 4,
        0xC8 => 2,
        0xBF..=0xC2 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E054A0(D, step, 0): b636's spawn on this record's globals (180
/// tasks, colour D+0x3364, kind D+0x33BA); frame 0x60 with the emitter's
/// s0 = D / s1 and the entry's s2 = param (-1), s3 / s4 the runner's (0
/// [H]).
fn spawn_task(fx: &mut Fx, step: u32, s1: u64, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0, u64::from(D)), (0x10, s1), (0x20, u64::MAX), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x40 * k + 0x4AE) == 0) {
        Some(k) => {
            let t = D + 0x478 + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x2D0, 0);
            let a = fx.mem.i16(KIND);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E02300(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter,
/// the followed points and the spawns (frame 0x60: the entry's s0 = D, s1
/// = 0, ra; its s0 = D, s1 the loops' counts).
fn emit(fx: &mut Fx) {
    let dd = u64::from(D);
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0, dd), (0x10, 0), (0x20, 0x01E0_4A0C)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let mut s1 = 0u64;
    let node = |fx: &mut Fx, o: u32, out: u32, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + o);
        node_position_saving(fx, h, slot, out, dd, Some(s1), ra);
    };
    let gap = |fx: &mut Fx, to: u32, a: u32, b: u32| {
        for k in 0..3 {
            let v = ee::sub(fx.mem.f(D + a + 4 * k), fx.mem.f(D + b + 4 * k));
            fx.mem.setf(D + to + 4 * k, v);
        }
    };
    if c(fx) < 0xC4 {
        node(fx, 0, D + 0x31B0, s1, 0x01E0_2358);
    }
    if c(fx) < 0xC4 {
        node(fx, 2, D + 0x320, s1, 0x01E0_2380);
    }
    if c(fx) < 0xC4 {
        gap(fx, 0x20, 0x320, 0x31B0);
    }
    if c(fx) < 0xBB {
        node(fx, 4, D + 0x31C0, s1, 0x01E0_23E8);
    }
    if c(fx) < 0xBB {
        node(fx, 6, D + 0x330, s1, 0x01E0_2410);
    }
    if c(fx) < 0xC4 {
        gap(fx, 0x30, 0x330, 0x31C0);
    }
    for (to, from) in [(0x40u32, 0x31B0u32), (0x50, 0x31C0)] {
        for k in 0..4 {
            let w = fx.mem.read_u32(D + from + 4 * k);
            fx.mem.write_u32(D + to + 4 * k, w);
        }
    }
    // Pushed back along the facing by 3 a update.
    let push = |fx: &mut Fx, local: u32, ra: u32, z: u32| {
        let f = facing_of(fx, sp + local, ra);
        let cs = l2::sin_deg(fx, SINES, (f - 0xB4) as i16 as i32, 1);
        let v = ee::add(fx.mem.f(D + z), ee::mul(3.0, ee::neg(cs)));
        fx.mem.setf(D + z, v);
    };
    if c(fx) == 0xC4 {
        push(fx, 0x40, 0x01E0_24E8, 0x31B8);
    }
    if (0xBB..0xBF).contains(&c(fx)) {
        push(fx, 0x50, 0x01E0_2574, 0x31C8);
    }
    let at = |fx: &mut Fx, from: u32| {
        for k in 0..3 {
            let w = fx.mem.read_u32(from + 4 * k);
            fx.mem.write_u32(D + 0x2D0 + 4 * k, w);
        }
    };
    // (update, point, return addresses: disc, grey rings, strip rings).
    for (when, p, ra) in [(0xBE, 0x31C0u32, [0x01E0_25E4u32, 0x01E0_25FC, 0x01E0_2624]), (0xC4, 0x31B0, [0x01E0_2674, 0x01E0_268C, 0x01E0_26B4])] {
        if c(fx) == when {
            at(fx, D + p);
            spawn_task(fx, DISC, s1, ra[0]);
            for i in 0..3u64 {
                spawn_task(fx, GREY, i, ra[1]);
            }
            for i in 0..0xAu64 {
                spawn_task(fx, STRIP, i, ra[2]);
            }
            s1 = 0xA;
        }
    }
    let n = c(fx);
    if (0xB0..0xCB).contains(&n) && n & 1 == 1 {
        fx.mem.set16(KIND, 0);
        spawn_task(fx, DOME, s1, 0x01E0_271C);
        fx.mem.set16(KIND, 1);
        spawn_task(fx, DOME, s1, 0x01E0_2738);
    }
    // (below, node, kind, return addresses: node, strips, spark, the
    // four strips, the update of the four).
    for (below, o, kind, ra, four) in [(0xC5, 0u32, 0, [0x01E0_2760u32, 0x01E0_2790, 0x01E0_27A4, 0x01E0_27CC], 0xC2), (0xBF, 4, 1, [0x01E0_2808, 0x01E0_283C, 0x01E0_2850, 0x01E0_2878], 0xBD)] {
        if c(fx) < below {
            node(fx, o, sp + 0x30, s1, ra[0]);
            at(fx, sp + 0x30);
            fx.mem.set16(KIND, kind);
            spawn_task(fx, STRIPS, s1, ra[1]);
            spawn_task(fx, SPARK, s1, ra[2]);
            if c(fx) == four {
                for i in 0..4u64 {
                    spawn_task(fx, STRIPS, i, ra[3]);
                }
                s1 = 4;
            }
        }
    }
    for (from, below, o, ra) in [(0xB1, 0xBF, 6u32, [0x01E0_28BC, 0x01E0_28EC]), (0xBB, 0xC4, 2, [0x01E0_2920, 0x01E0_2950])] {
        let n = c(fx);
        if (from..below).contains(&n) {
            node(fx, o, sp + 0x30, s1, ra[0]);
            at(fx, sp + 0x30);
            fx.mem.set16(KIND, 0);
            spawn_task(fx, BLUE, s1, ra[1]);
        }
    }
    // (range, odd, node, point, return addresses: node, purple, orange).
    for (from, below, odd, o, p, ra) in [(0xB9, 0xBF, 0, 4u32, 0x31C0u32, [0x01E0_29A0u32, 0x01E0_29D0, 0x01E0_29F4]), (0xC2, 0xC5, 1, 0, 0x31B0, [0x01E0_2A54, 0x01E0_2A84, 0x01E0_2AA8])] {
        let n = c(fx);
        if (from..below).contains(&n) && n & 1 == odd {
            node(fx, o, sp + 0x30, s1, ra[0]);
            at(fx, D + p);
            spawn_task(fx, PURPLE, 0, ra[1]);
            spawn_task(fx, ORANGE, 0, ra[2]);
            s1 = 1;
        }
    }
    for (when, o, ra) in [([0x84, 0x9B, 0xB7], 0xCu32, [0x01E0_2AF8u32, 0x01E0_2B28]), ([0x79, 0x8E, 0xB1], 0xE, [0x01E0_2B78, 0x01E0_2BA8])] {
        if when.contains(&c(fx)) {
            node(fx, o, sp + 0x30, s1, ra[0]);
            at(fx, sp + 0x30);
            for i in 0..8u64 {
                spawn_task(fx, DRIFT, i, ra[1]);
            }
            s1 = 8;
        }
    }
    if c(fx) == 1 {
        spawn_task(fx, CROSS, s1, 0x01E0_2BDC);
    }
    fx.leave(0x60);
}

/// rand % m from the step at `at` (its s0 = `s0`, the `jal` at `at + ra -
/// 8`).
fn rm(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    rmod(fx, s0, ra, m)
}

fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

/// Alpha `held` while younger than +6, then down by 10 (over below 0x11).
fn hold_fade(fx: &mut Fx, t: u32, held: u8, life: i32) {
    hold_fade_by(fx, t, held, life, 0xA);
}

/// [`hold_fade`] fading by `by`.
pub(crate) fn hold_fade_by(fx: &mut Fx, t: u32, held: u8, life: i32, by: u8) {
    if fx.mem.i16(t + 0x32) < life {
        fx.mem.write_u8(t + 0x2B, held);
    } else {
        fade_by(fx, t, by);
    }
}

/// Alpha up by 0x38 (below 0x71) while younger than +6, then down by 10.
fn rise_fade(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x71 {
            fx.mem.write_u8(t + 0x2B, a + 0x38);
        }
    } else {
        fade(fx, t);
    }
}

fn fade(fx: &mut Fx, t: u32) {
    fade_by(fx, t, 0xA);
}

/// Alpha down by `by` (the task over below 0x11).
pub(crate) fn fade_by(fx: &mut Fx, t: u32, by: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - by);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

fn grow(fx: &mut Fx, t: u32) {
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
}

/// 0x1E02E10 (D, task): a dome (purple (0x80, 0x50, 0xC8) every update;
/// +0x10 -0x4000 facing, else -0xC000, +0x18 0x3800, size 15, +6 the kind,
/// +0x30 90, alpha in by 0x40 for 4 updates then out by 0xD); size up by
/// 12 from update 0xBC (kind 0) / 0xB3 (kind 1); +0x30 up by 8 (`r`'s
/// step; 2 from update 0xAB / 0xA1), +0xE up by rand % 3 + 1, +0x3A by 5
/// (mod 360). `at` the step's record address.
pub(crate) fn dome(fx: &mut Fx, r: &Rec, at: u32, t: u32) {
    let sp = fx.enter(0x50);
    l2::rgb(fx, t + 0x28, 0x80, 0x50, 0xC8);
    if fx.mem.i16(t + 0x36) == 1 {
        let f = facing_of(fx, sp + 0x40, at + 0x80);
        fx.mem.set32(t + 0x10, if f != 0 { -0x4000 } else { -0xC000 });
        fx.mem.write_u32(t + 0x14, 0);
        fx.mem.set32(t + 0x18, 0x3800);
        fx.mem.set16(t + 0x34, 0);
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 6, k);
        fx.mem.set16(t + 0x30, 0x5A);
        fx.mem.set16(t + 0x38, 0xF);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 4 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xD);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let c = fx.mem.i16(r.counter);
    let kind = fx.mem.i16(t + 6);
    if kind == 0 && c >= 0xBC {
        let s = fx.mem.i16(t + 0x38) + 0xC;
        fx.mem.set16(t + 0x38, s);
    }
    let kind = fx.mem.i16(t + 6);
    if kind == 1 && fx.mem.i16(r.counter) >= 0xB3 {
        let s = fx.mem.i16(t + 0x38) + 0xC;
        fx.mem.set16(t + 0x38, s);
    }
    let kind = fx.mem.i16(t + 6);
    let c = fx.mem.i16(r.counter);
    let mut s0 = 0i32;
    if kind == 0 {
        s0 = if c < 0xAB { r.dome_step } else { 2 };
    }
    if kind == 1 {
        s0 = if fx.mem.i16(r.counter) < 0xA1 { r.dome_step } else { 2 };
    }
    age(fx, t);
    let v = rm(fx, s0 as u32, at + 0x1F4, 3);
    let e = fx.mem.i16(t + 0xE) + (v + 1);
    fx.mem.set16(t + 0xE, e);
    let r = fx.mem.i16(t + 0x30) + s0;
    fx.mem.set16(t + 0x30, r);
    let a = fx.mem.i16(t + 0x3A) + 5;
    fx.mem.set16(t + 0x3A, a);
    let a = fx.mem.i16(t + 0x3A);
    if a >= 0x168 {
        fx.mem.set16(t + 0x3A, a % 0x168);
    }
    fx.leave(0x50);
}

/// The rings' shared first update: size, cell frame rand & `cell`, a
/// colour, angles, +0x3A the kind (+0x30 cleared), +0xE, row +8 rand % 10
/// and life +6.
fn spark(fx: &mut Fx, t: u32) {
    let at = SPARK;
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        let v = fx.rand_from(t, at + 0x30);
        fx.mem.set16(t + 0x34, v & 3);
        l2::rgb(fx, t + 0x28, 0xA0, 0x60, 0);
        let v = rm(fx, t, at + 0x64, 0xFFFF);
        fx.mem.set32(t + 0x18, v);
        let v = rm(fx, t, at + 0x80, 0xFFFF);
        fx.mem.set32(t + 0x10, v);
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, k);
        fx.mem.set16(t + 0x30, 0);
        let v = rm(fx, t, at + 0xA8, 0x11);
        fx.mem.set16(t + 0xE, v + 7);
        let v = rm(fx, t, at + 0xC8, 0xA);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, at + 0xE4);
        fx.mem.set16(t + 6, (v & 1) + 2);
        fx.mem.set16(t + 0x36, 2);
    }
    let life = fx.mem.i16(t + 6);
    hold_fade(fx, t, 0x80, life);
    for (k, ra) in [(0u32, 0x15Cu32), (1, 0x174), (2, 0x18C)] {
        let i = fx.mem.i16(t + 0x3A) as u32;
        let v = fx.mem.f(D + 0x31B0 + 0x10 * i + 4 * k);
        let x = fix(fx, v, at + ra);
        fx.mem.set16(t + 2 * k, x);
    }
    grow(fx, t);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03260 (D, task): a blue ring (size 10, cell 0..3, (0x60, 8, 0xA0),
/// angles +0x18 / +0x10 rand % 0xFFFF, +0xE 7..23, life 2..3); 0x80 for its
/// life, then out by 10; radius up by +0xE. `at` the step's record
/// address.
pub(crate) fn blue(fx: &mut Fx, at: u32, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xA);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v & 3);
        l2::rgb(fx, t + 0x28, 0x60, 8, 0xA0);
        let v = rm(fx, t, at + 0x5C, 0xFFFF);
        fx.mem.set32(t + 0x18, v);
        let v = rm(fx, t, at + 0x78, 0xFFFF);
        fx.mem.set32(t + 0x10, v);
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, k);
        fx.mem.set16(t + 0x30, 0);
        let v = rm(fx, t, at + 0xA0, 0x11);
        fx.mem.set16(t + 0xE, v + 7);
        let v = rm(fx, t, at + 0xC0, 0xA);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, at + 0xDC);
        fx.mem.set16(t + 6, (v & 1) + 2);
        fx.mem.set16(t + 0x36, 2);
    }
    let life = fx.mem.i16(t + 6);
    hold_fade(fx, t, 0x80, life);
    grow(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03480 (D, task): a purple ring (size 25, cell 0..7, (0x40, 8,
/// 0x80), +0x3A rand % 360, +0xE 7..22, life 3..9); in by 0x38 for its
/// life, then out by 10; radius up by +0xE after the first update.
fn purple(fx: &mut Fx, t: u32) {
    let at = PURPLE;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x19);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v & 7);
        l2::rgb(fx, t + 0x28, 0x40, 8, 0x80);
        fx.mem.set16(t + 0x30, 0);
        let v = rm(fx, t, at + 0x5C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = fx.rand_from(t, at + 0x78);
        fx.mem.set16(t + 0xE, (v & 0xF) + 7);
        let v = rm(fx, t, at + 0x98, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rm(fx, t, at + 0xB4, 7);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x36, 2);
    }
    rise_fade(fx, t);
    if fx.mem.i16(t + 0x32) != 0 {
        grow(fx, t);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E035F0 (D, task): an orange strip ring (size 14, cell 0..3, (0x80,
/// 0x40, 0), +0x18 rand % 360 turning by 5, +0x3A the kind, +0xE 6..23,
/// life 8..27); 0x80 for its life, then out by 10 (`r`'s +0xE base /
/// fade). `at` the step's record address.
pub(crate) fn orange(fx: &mut Fx, r: &Rec, at: u32, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xE);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v & 3);
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        let v = rm(fx, t, at + 0x5C, 0x168);
        fx.mem.set32(t + 0x18, v);
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, k);
        fx.mem.set16(t + 0x30, 0);
        let v = rm(fx, t, at + 0x84, 0x12);
        fx.mem.set16(t + 0xE, v + r.orange_e);
        let v = rm(fx, t, at + 0xA4, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rm(fx, t, at + 0xC0, 0x14);
        fx.mem.set16(t + 6, v + 8);
        fx.mem.set16(t + 0x36, 2);
    }
    let life = fx.mem.i16(t + 6);
    hold_fade_by(fx, t, 0x80, life, r.orange_fade);
    let a = fx.mem.i32(t + 0x18) + 5;
    fx.mem.set32(t + 0x18, a);
    grow(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E038C0 (D, task): a blue strip ring (size 2 (13 from update 0x8D),
/// cell 0..3, (0x40, 0x40, 0x80), +0x18 rand % 360 turning by +-5 by the
/// kind, +0xE 4..13, life 9..24); 0x80 for its life, then out by 10; from
/// update 0xBA (kind 1) / 0xC1 (kind 0) deep blue and growing by +0xE +
/// 0x58. `at` the step's record address.
pub(crate) fn strips(fx: &mut Fx, r: &Rec, at: u32, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 2);
        let v = fx.rand_from(t, at + 0x30);
        fx.mem.set16(t + 0x34, v & 3);
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x80);
        let v = rm(fx, t, at + 0x78, 0x168);
        fx.mem.set32(t + 0x18, v);
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, k);
        fx.mem.set16(t + 0x30, 0);
        let v = rm(fx, t, at + 0xA0, 0xA);
        fx.mem.set16(t + 0xE, v + 4);
        let v = rm(fx, t, at + 0xC0, 0xA);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, at + 0xDC);
        fx.mem.set16(t + 6, (v & 0xF) + 9);
        fx.mem.set16(t + 0x36, 2);
    }
    let life = fx.mem.i16(t + 6);
    hold_fade(fx, t, 0x80, life);
    let turn = if fx.mem.i16(t + 0x3A) != 0 { 5 } else { -5 };
    let a = fx.mem.i32(t + 0x18) + turn;
    fx.mem.set32(t + 0x18, a);
    for (kind, from) in [(1, 0xBA), (0, 0xC1)] {
        if fx.mem.i16(t + 0x3A) == kind {
            if fx.mem.i16(r.counter) < from {
                grow(fx, t);
            } else {
                l2::rgb(fx, t + 0x28, 0x20, 0x20, 0xC0);
                let r = fx.mem.i16(t + 0x30) + (fx.mem.i16(t + 0xE) + 0x58);
                fx.mem.set16(t + 0x30, r);
            }
        }
    }
    if fx.mem.i16(r.counter) >= 0x8D {
        fx.mem.set16(t + 0x38, 0xD);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03B20 (D, task): a grey ring (size 16, cell 0..7, +0x18 rand %
/// 0xFFFF, (0xA0, 0xA0, 0xA0), +0x3A rand % 360, +0xE 10..27, life 3..9);
/// in by 0x38 for its life, then out by 10; radius up by +0xE after the
/// first update (`r`'s size / +0xE base). `at` the step's record address.
pub(crate) fn grey(fx: &mut Fx, r: &Rec, at: u32, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, r.grey_size);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v & 7);
        let v = rm(fx, t, at + 0x44, 0xFFFF);
        fx.mem.set32(t + 0x18, v);
        l2::rgb(fx, t + 0x28, 0xA0, 0xA0, 0xA0);
        fx.mem.set16(t + 0x30, 0);
        let v = rm(fx, t, at + 0x70, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rm(fx, t, at + 0x8C, 0x12);
        fx.mem.set16(t + 0xE, v + r.grey_e);
        let v = rm(fx, t, at + 0xAC, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rm(fx, t, at + 0xC8, 7);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x36, 2);
    }
    rise_fade(fx, t);
    if fx.mem.i16(t + 0x32) != 0 {
        grow(fx, t);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03CA0 (D, task): the cross ribbon's task (colours (0xC0, 0xC0,
/// 0xC0) / (0xC0, 0xC0, 0), 9 points, alpha 0x40, cell 1, size 10): alpha
/// 0xC0 to update 0x28, out by 9 (held at 0) to 0x8C, 0xC0 again to 0xAF,
/// then out by 9 (over below 0xB); +0x18 up by 6 (mod 360).
fn cross(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x2C, 0xC0, 0xC0, 0);
        fx.mem.set16(t + 6, 9);
        fx.mem.write_u8(t + 0x2B, 0x40);
        fx.mem.set16(t + 0x34, 1);
        fx.mem.set16(t + 0x38, 0xA);
        fx.mem.set16(t + 0x36, 2);
    }
    let n = fx.mem.i16(t + 0x32);
    let a = fx.mem.read_u8(t + 0x2B);
    if n < 0xAF {
        if n < 0x28 || n >= 0x8C {
            fx.mem.write_u8(t + 0x2B, 0xC0);
        } else if a >= 0xB {
            fx.mem.write_u8(t + 0x2B, a - 9);
        } else {
            fx.mem.write_u8(t + 0x2B, 0);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    age(fx, t);
    let v = fx.mem.i32(t + 0x18) + 6;
    fx.mem.set32(t + 0x18, v);
    let v = v as i16 as i32;
    let v = if v < 0x168 { v } else { (v % 0x168) as i16 as i32 };
    fx.mem.set32(t + 0x18, v);
    fx.leave(0x20);
}

/// 0x1E04220 (D, task): a drifting grey sprite (size 123, cell 0 up by 1
/// every third update to 7, +0x30 / +0x3A rand % 360); 0x80 for 10
/// updates, then out by 10; drifting by 14 sin +0x3A (x), 4 cos +0x30 (y),
/// 14 cos +0x3A (z).
fn drift(fx: &mut Fx, t: u32) {
    let at = DRIFT;
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x7B);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x40);
        let v = rm(fx, t, at + 0x4C, 0x168);
        fx.mem.set16(t + 0x30, v);
        let v = rm(fx, t, at + 0x68, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    hold_fade(fx, t, 0x80, 0xA);
    if fx.mem.i16(t + 0x32) % 3 == 1 && fx.mem.i16(t + 0x34) < 7 {
        let v = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, v);
    }
    age(fx, t);
    for (o, deg, cos, k, ra) in [(8u32, 0x3Au32, 0, 14.0f32, 0x140u32), (0xA, 0x30, 1, 4.0, 0x178), (0xC, 0x3A, 1, 14.0, 0x1B0)] {
        let a = fx.mem.i16(t + deg);
        let s = l2::sin_deg(fx, SINES, a, cos);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(k, s));
        let r = fix(fx, v, at + ra);
        fx.mem.set16(t + o, r);
    }
    fx.leave(0x30);
}

/// 0x1E005A0 (D, task, desc, _) at record address `at` (b192's 0x1E00780
/// is word-equal but for its jal targets [F fdiff]): one textured square (9 values, FST,
/// ALPHA 0x48) into `desc` of the i16 size +0x38 facing the camera, at the
/// task's point plus its drift / 10 (scale 0.1), coloured +0x28, UVs from
/// D+0x94 (order 0), texture D+0x88; keyed by its mean w into D+0x80.
pub(crate) fn drift_draw(fx: &mut Fx, at: u32, t: u32, d: u32) {
    let sp = fx.enter(0x110);
    desc_uv4(fx, d);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, sp + 0x108, t + 0x28, d + 0x10);
    uv::uv_rect4(fx, d, D + 0x94, 0);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0x90 + 4 * k, 0x3DCC_CCCD);
    }
    crate::b494::camera_angles_saving(fx, sp + 0xF0, u64::from(d), u64::from(t), at + 0xDC);
    for k in 0..3 {
        let p = ee::from_i32(fx.mem.i16(t + 2 * k));
        let o = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
        fx.mem.setf(sp + 0x80 + 4 * k, ee::add(p, o));
    }
    l2::xform_vp(fx, sp + 0xB0, sp + 0x90, sp + 0xF0, sp + 0x80);
    let v = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x40, v, v);
    if l2::project(fx, sp + 0x40, SPR, sp + 0xB0, d, sp + 0xA0) {
        let key = ee::fixunssfsi(fx.mem.f(sp + 0xA0)) as i32;
        let ot = fx.mem.read_u32(D + 0x80);
        fx.submit(d, ot, key);
    }
    fx.leave(0x110);
}

/// 0x1E055C0 (D, task, desc, mode): mode 0 sets `desc` up (9 values, FST,
/// ALPHA 0x48, the rect 0x97 frame +0x34 in order 0, texture D+0x37C,
/// colour +0x28); mode 1 draws twelve quads of a dome band: rows at
/// heights 30 sin +0x30 / 30 sin (+0x30 + 8), radii 90 cos s / 90 cos (s
/// + 14) (s = +0x30 at most 160), 30 degrees each from +0x3A, scaled by
/// size / 250, turned by +0x10, at the kind's point + its gap / 50 times
/// +0xE; keyed by mean w (D+0x3394) into D+0x336C (`r`'s key / OT; b225
/// swaps the rows' heights).
pub(crate) fn dome_draw(fx: &mut Fx, r: &Rec, t: u32, d: u32, mode: i32) {
    use ee::mul;
    let sp = fx.enter(0x150);
    if mode == 0 {
        let sub = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0x130, 0x97, sub);
        desc_uv4(fx, d);
        fx.desc_alpha(d, 0x80_0000_0048);
        fx.mem.write_u32(d + 0x14, 0);
        for o in [0x27u32, 0x37, 0x47, 0x57] {
            fx.mem.write_u8(d + o, 0);
        }
        rgbaq_at(fx, sp + 0x148, t + 0x28, d + 0x10);
        uv::uv_rect4(fx, d, sp + 0x130, 0);
        let tex = fx.mem.read_u32(D + 0x37C);
        fx.desc_texture(d, tex, 1);
        fx.leave(0x150);
        return;
    }
    let i = 0x10 * fx.mem.i16(t + 6) as u32;
    let e = ee::from_i32(fx.mem.i16(t + 0xE));
    for k in 0..3 {
        let a = fx.mem.f(D + 0x31B0 + i + 4 * k);
        let b = fx.mem.f(D + 0x320 + i + 4 * k);
        let g = ee::div(ee::sub(b, a), 50.0);
        fx.mem.setf(sp + 0xD0 + 4 * k, ee::add(a, mul(e, g)));
    }
    let s = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 250.0);
    l2::set3(fx, sp + 0xE0, s, s, s);
    l2::xform_vp(fx, sp + 0xF0, sp + 0xE0, t + 0x10, sp + 0xD0);
    let p = fx.mem.i16(t + 0x30);
    let p8 = (p + 8) as i16 as i32;
    let first = if r.rows_swapped { p8 } else { p };
    let y = mul(30.0, l2::sin_deg(fx, r.sines, first, 0));
    fx.mem.setf(sp + 0x94, y);
    fx.mem.setf(sp + 0xA4, y);
    let second = if r.rows_swapped { fx.mem.i16(t + 0x30) } else { (fx.mem.i16(t + 0x30) + 8) as i16 as i32 };
    let y = mul(30.0, l2::sin_deg(fx, r.sines, second, 0));
    fx.mem.setf(sp + 0xB4, y);
    fx.mem.setf(sp + 0xC4, y);
    let s0 = p.min(0xA0);
    let f20 = mul(90.0, l2::sin_deg(fx, r.sines, s0, 1));
    let f21 = mul(90.0, l2::sin_deg(fx, r.sines, (s0 + 0xE) as i16 as i32, 1));
    let mut s2 = 0i32;
    for _ in 0..0xC {
        let a = (fx.mem.i16(t + 0x3A) + s2) as i16 as i32;
        let c = l2::sin_deg(fx, r.sines, a, 1);
        let s = l2::sin_deg(fx, r.sines, a, 0);
        fx.mem.setf(sp + 0xB0, mul(c, f21));
        fx.mem.setf(sp + 0xB8, mul(s, f21));
        let c = l2::sin_deg(fx, r.sines, a, 1);
        fx.mem.setf(sp + 0x90, mul(f20, c));
        let s = l2::sin_deg(fx, r.sines, a, 0);
        fx.mem.setf(sp + 0x98, mul(f20, s));
        let b = (fx.mem.i16(t + 0x3A) + 0x1E + s2) as i16 as i32;
        let c = l2::sin_deg(fx, r.sines, b, 1);
        let s = l2::sin_deg(fx, r.sines, b, 0);
        fx.mem.setf(sp + 0xC0, mul(c, f21));
        fx.mem.setf(sp + 0xC8, mul(s, f21));
        fx.mem.setf(sp + 0xA0, mul(c, f20));
        fx.mem.setf(sp + 0xA8, mul(s, f20));
        if l2::project(fx, sp + 0x90, SPR, sp + 0xF0, d, r.key) {
            let key = ee::fixunssfsi(fx.mem.f(r.key)) as i32;
            let ot = fx.mem.read_u32(r.ot2);
            fx.submit(d, ot, key);
        }
        s2 = (s2 + 0x1E) as i16 as i32;
    }
    fx.leave(0x150);
}

/// 0x1E059F0 (D, task, desc, table): a ring of D+0xB8 textured strips
/// (strip3, colours +0x2C for corners 0 / 1, +0x28 for 2 / 3, UVs from
/// D+0x94 in order 0, texture D+0x88), each a quad of size i16 +0x38
/// pushed out by the bytes `table` [k + i16 +8 (+0, +0x64, +0xB4)] / 20
/// times i16 +0x30 + k / 2, turned about z by +0x10, corners 0 / 1 then
/// (1, 0, 0) / (-1, 0, 0) and 2 / 3 pushed by five times the kind's gap
/// (D+0x20 + 0x10 +0x3A) in x / y; unturned (the task's angles copied
/// over the camera's but not passed) at the kind's point (D+0x40 + 0x10 +0x3A, scale 0.1), each keyed by its mean w
/// into D+0x80.
/// `at` the draw's record address (b225: 0x1E04890).
pub(crate) fn strip_ring(fx: &mut Fx, at: u32, t: u32, d: u32, table: u32) {
    let sp = fx.enter(0x150);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x2C, 1);
    l2::colour_strip3(fx, d, t + 0x28, 2);
    l2::uv_strip3(fx, d, D + 0x94, 0);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xE0 + 4 * k, 0x3DCC_CCCD);
    }
    crate::b494::camera_angles_saving(fx, sp + 0x140, u64::from(table), u64::from(d), at + 0xAC);
    for k in 0..3 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(sp + 0x140 + 4 * k, w);
    }
    let i = 0x10 * fx.mem.i16(t + 0x3A) as u32;
    let f20 = ee::mul(5.0, fx.mem.f(D + 0x20 + i));
    let f21 = ee::mul(5.0, fx.mem.f(D + 0x24 + i));
    for k in 0..3 {
        let w = fx.mem.read_u32(D + 0x40 + i + 4 * k);
        fx.mem.write_u32(sp + 0xD0 + 4 * k, w);
    }
    // The rotation pointer is 0 (the copied angles unused) [F 0x1E05ABC].
    l2::xform_vp(fx, sp + 0x100, sp + 0xE0, 0, sp + 0xD0);
    let q = sp + 0x90;
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0xB8) {
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, q, s, s);
        for (axis, off) in [(0, 0u32), (1, 0x64), (2, 0xB4)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::from_i32(fx.mem.i8(table.wrapping_add(k as u32).wrapping_add(i as u32).wrapping_add(off)));
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + (k >> 1));
            l2::add_column(fx, q, axis, ee::mul(ee::div(b, 20.0), r));
        }
        l2::rotate4(fx, q, t + 0x10, 2, D + 0xD8);
        for (o, v) in [(4u32, 0u32), (0x14, 0), (0, 0x3F80_0000), (0x10, 0xBF80_0000), (8, 0), (0x18, 0)] {
            fx.mem.write_u32(q + o, v);
        }
        for (o, v) in [(0x20u32, f20), (0x24, f21), (0x30, f20), (0x34, f21)] {
            let w = ee::add(fx.mem.f(q + o), v);
            fx.mem.setf(q + o, w);
        }
        if l2::project(fx, q, SPR, sp + 0x100, d, sp + 0xF0) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0xF0)) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x150);
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 0x1E01CF0 (D, task, history): the cross ribbon: along the i16 +6
/// points of `history` (four strips a segment by `spline::spline` basis
/// 0), four textured strips each (strip3, colours +0x28 / +0x2C, UVs from
/// D+0x94 in D+0xBE's orientation, texture D+0x88) crossing at the curve:
/// +-y by size / 10, then +-x by it with z by half; its alpha down by
/// alpha / (3 n) a strip (to 0); at the far Z, key 0.
fn cross_draw(fx: &mut Fx, t: u32, p: u32) {
    use ee::{add, div, sub};
    let sp = fx.enter(0x260);
    let w = div(ee::from_i32(fx.mem.i16(t + 0x38)), 10.0);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x1A8;
    l2::desc_strip3(fx, d);
    for (local, c, to) in [(0x238u32, 0x28u32, 0x10u32), (0x240, 0x2C, 0x28), (0x248, 0x28, 0x40), (0x250, 0x2C, 0x58)] {
        rgbaq_at(fx, sp + local, t + c, d + to);
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    let colour = sp + 0x25C;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(colour + k, v);
    }
    copy4(fx, sp + 0xE0, p);
    copy4(fx, sp + 0xF0, p);
    copy4(fx, sp + 0x100, p + 0x10);
    copy4(fx, sp + 0x110, p + 0x20);
    crate::b494::camera_angles_saving(fx, sp + 0x170, n as i64 as u64, u64::from(D), 0x01E0_1ED4);
    to_degrees_neg(fx, sp + 0x174);
    fx.mem.set32(sp + 0xB0, n - 1);
    let s7 = 3 * n;
    let mut s0 = 0i32;
    while s0 < n - 1 {
        let s5 = p.wrapping_add((0x10 * (s0 - 1)) as u32);
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            let (src, o) = if s0 != 0 { (s5, 0x180u32) } else { (sp + 0xE0, 0x190) };
            spline::spline(fx, sp + 0xC0, src, f20, 0, [0.0; 4]);
            f20 = add(f20, 0.25);
            spline::spline(fx, sp + o, src, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0xD0, sp + o);
            l2::colour_strip3(fx, d, colour, 1);
            let al = i32::from(fx.mem.read_u8(t + 0x2B));
            let q = fx.div(al, s7).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(colour + 3)) - q;
            fx.mem.write_u8(colour + 3, if v > 0 { v as u8 } else { 0 });
            l2::colour_strip3(fx, d, colour, 2);
            for s1 in 0..4 {
                for (to, from) in [(0x120u32, 0xC0u32), (0x130, 0xC0), (0x140, 0xD0), (0x150, 0xD0)] {
                    for c in 0..3 {
                        let w = fx.mem.read_u32(sp + from + 4 * c);
                        fx.mem.write_u32(sp + to + 4 * c, w);
                    }
                }
                let bump = |fx: &mut Fx, o: u32, by: f32, up: bool| {
                    let v = fx.mem.f(sp + o);
                    fx.mem.setf(sp + o, if up { add(v, by) } else { sub(v, by) });
                };
                match s1 {
                    0 | 1 => {
                        bump(fx, 0x134, w, s1 == 0);
                        bump(fx, 0x154, w, s1 == 0);
                    }
                    _ => {
                        let h = div(w, 2.0);
                        bump(fx, 0x130, w, s1 == 2);
                        bump(fx, 0x150, w, s1 == 2);
                        bump(fx, 0x138, h, s1 == 2);
                        bump(fx, 0x158, h, s1 == 2);
                    }
                }
                if l2::project(fx, sp + 0x120, SPR, 0, d, sp + 0x160) {
                    l2::far_z(fx, &[d + 0x68, d + 0x50, d + 0x38, d + 0x20]);
                    fx.submit(d, OT, 0);
                }
            }
        }
        s0 += 1;
    }
    fx.leave(0x260);
}
