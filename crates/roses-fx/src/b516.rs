//! BMODULE b516 (card 640), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03030, data area 0x1ED0000, 0xCF30 bytes) [F]
//!: b703's program shape (400 tasks of 0x70 at
//! D+0x2A0, alive +0x58, step +0x44 through `jalr`, 0x1E03020 the idle
//! step; eight 20-point node histories D+0xB1D0 + 0x140 k), spawned by
//! 0x1E03860 (b522's spawn: kind D+0xCBB0 into +0x50 / +0x52, angle
//! D+0xCBA2) and drawn with the second library. Nodes: the attacker's
//! 0x13 / 0x15 / 0x25 / 0x30 / 1 / 0x2C / 0x37 / 0x35 (D+0xCB44..), the
//! target's 1 (D+0xCB54); timer 0xE6.
//!
//! Update 0x90 an orange ribbon task (0x1E02AC0: four histories each of
//! nodes 0x13 and 0x15 stacked 2.5 apart, kept for 12 updates, drawn as
//! three ribbons twice by 0x1E03A10 = b703's ribbon with the `b` side's
//! alpha down by 3 while above 4). On 0x9A at node 0x15 two green columns
//! (0x1E02170, b544's with its colours by the update, `sprite_quads` x2),
//! an arc (0x1E02340, b065's with kind colours, `arc_strips`) and ten
//! rings (0x1E02880, b418's with the kind into +0xC, `ring_facing`); on
//! 0x84 three times at node 0x25 two columns, an arc (6 lower / higher)
//! and ten rings; 0x78..0x8C sparkles (0x1E02470, b544's in green,
//! billboard::quad on cell 0x9B with the row by +0x56) at nodes 0x25 /
//! 0x37 / 0x35 and a white mote (0x1E026D0, b751's drifting 7.6 then 1,
//! billboard::quad cell 0x14); below 0xC9 two motes a update at node 1,
//! 13 lower and 7 out. Returns 1 on 0x7A, 4 on 0xE1, 2 on 0xA5, 6 on
//! 0x85..0x9F, 8 once the timer D+0xCB10 has run out.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, spawn as hist, spline, uv};
use crate::bus::Bus;
use crate::c061::rgbaq_at;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const COLUMN: u32 = 0x01E0_2170;
const ARC: u32 = 0x01E0_2340;
const SPARKLE: u32 = 0x01E0_2470;
const MOTE: u32 = 0x01E0_26D0;
const RING: u32 = 0x01E0_2880;
const RIBBONS: u32 = 0x01E0_2AC0;
const IDLE: u32 = 0x01E0_3020;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3388;
/// The eight node histories (20 points of 16 bytes, 0x140 apart).
const HIST: u32 = D + 0xB1D0;
const TEX: u32 = D + 0xCAF0;
const TIMER: u32 = D + 0xCB10;
const COLOUR: u32 = D + 0xCB14;
const NODES: u32 = D + 0xCB44;
const ANGLE: u32 = D + 0xCBA2;
const KIND: u32 = D + 0xCBB0;
const FACING: u32 = D + 0xCBB4;
const COUNTER: u32 = D + 0xCBB8;
const BYTES: u32 = D + 0xCBBA;
const TAB: u32 = D + 0xCDBC;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The entry 0x1E03030 (s0 = param / the task pointer, s1 = the index,
/// s2 = the live count, s4 = D).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    fx.mem.write_u32(D + 0xCB1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xCF30);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xCBB6, param);
    fx.mem.set32(TIMER, 0xE6);
    for (k, n) in [0x13, 0x15, 0x25, 0x30, 1, 0x2C, 0x37, 0x35].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xCB54, r);
    for k in 0..2u32 {
        let t = fx.texture(4, 0xE + k as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k, t);
    }
    for k in 0..4u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_326C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_329C, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_32EC);
        fx.mem.set16(D + 0xCB64 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let p = D + 0x70 * k;
        let t = p + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 || f == IDLE {
            continue;
        }
        // The step's prologue: the entry's s0 = the pointer, s1 = the
        // index, s2 = 0.
        let frame = match f {
            MOTE => 0x30,
            RIBBONS => 0x50,
            _ => 0x20,
        };
        let s = fx.enter(frame);
        match frame {
            0x20 => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(p))]),
            0x30 => fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(k)), (0, u64::from(p))]),
            _ => fx.saved(s, &[(0x30, TASK_RA), (0x20, 0), (0x10, u64::from(k)), (0, u64::from(p))]),
        }
        match f {
            COLUMN => column(fx, t),
            ARC => arc(fx, t, k),
            SPARKLE => sparkle(fx, t, k),
            MOTE => mote(fx, t),
            RING => ring(fx, t),
            RIBBONS => ribbons(fx, t, s, p),
            _ => panic!("b516: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    // The entry's s2: the live tasks counted in the draw loop.
    let mut live = 0u32;
    for k in 0..TASKS {
        let p = D + 0x70 * k;
        let t = p + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        draw(fx, sp, t, p, k, live);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x7A => 1,
        0xE1 => 4,
        0xA5 => 2,
        0x85..=0x9F => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    copy4(fx, D + 0x94, r);
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

fn draw(fx: &mut Fx, sp: u32, t: u32, p: u32, k: u32, live: u32) {
    let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
    if f(fx) == COLUMN {
        tex(fx, D + 0x29C);
        cell(fx, sp + 0x80, 0x9A, 0);
        fx.mem.set16(D + 0xBE, 0);
        quads4::sprite_quads(fx, D, t, 0, t + 0x10);
        quads4::sprite_quads(fx, D, t, 0, t + 0x10);
    }
    if f(fx) == ARC {
        fx.mem.set16(D + 0xB8, 0x24);
        billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f(fx) == SPARKLE {
        tex(fx, TEX);
        let sub = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x90, 0x9B, sub);
        let o = fx.mem.i16(t + 0x52);
        fx.mem.set16(D + 0xBE, o);
        let c = fx.mem.i16(t + 0x56);
        if c < 2 {
            fx.mem.set16(D + 0x94, c << 6);
            fx.mem.set16(D + 0x96, 0);
        } else {
            fx.mem.set16(D + 0x94, (c - 2) << 6);
            fx.mem.set16(D + 0x96, 0x40);
        }
        fx.mem.set16(D + 0x98, 0x40);
        fx.mem.set16(D + 0x9A, 0x40);
        billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
    }
    if f(fx) == MOTE {
        tex(fx, TEX + 4);
        let sub = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0xA0, 0x14, sub);
        fx.mem.set16(D + 0xBE, 0);
        billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
    }
    if f(fx) == RING {
        let sub = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0xB0, 0x14, sub);
        fx.mem.set16(D + 0xC0, 0);
        let at = if fx.mem.i16(t + 0x52) == 0 { TEX + 8 } else { TEX + 0xC };
        tex(fx, at);
        fx.mem.set16(D + 0xB8, 0xA);
        for c in 0..3 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * c);
            fx.mem.write_u32(D + 0x20 + 4 * c, w);
        }
        fx.mem.set16(D + 0xBE, 0);
        billboard::ring_facing(fx, uv::uv_rect, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f(fx) == RIBBONS {
        // Two loops over the pairs (j, j + 4); the entry's s3 / s5 / s6 as
        // each loop keeps them.
        for j in 0..3u32 {
            let saves = [u64::from(p), u64::from(k), u64::from(live), u64::from(j), u64::from(D), u64::from(D + 0x140 * j), u64::from(t), 0, 0, 0x01E0_36CC];
            ribbon(fx, t, HIST + 0x140 * j, HIST + 0x140 * (j + 4), saves);
        }
        for j in 0..3u32 {
            let saves = [u64::from(p), u64::from(k), u64::from(live), u64::from(D + 0x140 * j), u64::from(D), u64::from(t), u64::from(j), 0, 0, 0x01E0_3734];
            ribbon(fx, t, HIST + 0x140 * j, HIST + 0x140 * (j + 4), saves);
        }
    }
}

/// 0x1E03860(D, step, 0): b522's spawn on this record's globals (colour
/// D+0xCB14, kind D+0xCBB0 into +0x50 / +0x52, angle D+0xCBA2 into +0x5A);
/// its frame's saves the emitter's s0 / s1 / s2 = D, the runner's s3 (0
/// [H]), the entry's s4 = D and `ra`.
fn spawn(fx: &mut Fx, step: u32, s0: u64, s1: u64, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, s0), (0x10, s1), (0x20, u64::from(D)), (0x30, 0), (0x40, u64::from(D)), (0x50, u64::from(ra))]);
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
            let s = fx.mem.i16(KIND);
            fx.mem.set16(t + 0x50, s);
            let s = fx.mem.i16(KIND);
            fx.mem.set16(t + 0x52, s);
            let a = fx.mem.i16(ANGLE);
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

/// 0x1E01CF0(D, 0): the update counter and the spawns (frame 0x40: ra, the
/// entry's s2 = 0, s1 (the runner's, 0 [H]), s0 = the parameter; then s2 =
/// D, s0 / s1 loop counts).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let mut s0 = param as i64 as u64;
    let mut s1 = 0u64;
    fx.saved(sp, &[(0x30, 0x01E0_3348), (0x20, 0), (0x10, s1), (0, s0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let node = |fx: &mut Fx, k: u32, s0: u64, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, slot, POS, s0, Some(s1), ra);
    };
    if c(fx) == 0x90 {
        fx.mem.set16(KIND, 0);
        spawn(fx, RIBBONS, s0, s1, 0x01E0_1D54);
    }
    if c(fx) == 0x9A {
        node(fx, 1, s0, s1, 0x01E0_1D8C);
        fx.mem.set16(ANGLE, 0x2D);
        fx.mem.set16(KIND, 0);
        spawn(fx, COLUMN, s0, s1, 0x01E0_1DBC);
        fx.mem.set16(ANGLE, 0x87);
        fx.mem.set16(KIND, 1);
        spawn(fx, COLUMN, s0, s1, 0x01E0_1DF0);
        fx.mem.set16(KIND, 1);
        spawn(fx, ARC, s0, s1, 0x01E0_1E14);
        s0 = 0;
        while s0 < 0xA {
            spawn(fx, RING, s0, s1, 0x01E0_1E30);
            s0 += 1;
        }
    }
    if c(fx) == 0x84 {
        s0 = 0;
        while s0 < 3 {
            s1 = s0;
            node(fx, 2, s0, s1, 0x01E0_1E98);
            fx.mem.set16(ANGLE, 0x2D);
            fx.mem.set16(KIND, 0);
            spawn(fx, COLUMN, s0, s1, 0x01E0_1EC8);
            fx.mem.set16(ANGLE, 0x87);
            fx.mem.set16(KIND, 0);
            spawn(fx, COLUMN, s0, s1, 0x01E0_1EF8);
            if s1 == 1 {
                let y = ee::sub(fx.mem.f(POS + 4), 6.0);
                fx.mem.setf(POS + 4, y);
            }
            if s1 == 2 {
                let y = ee::add(fx.mem.f(POS + 4), 6.0);
                fx.mem.setf(POS + 4, y);
            }
            spawn(fx, ARC, s0, s1, 0x01E0_1F5C);
            s1 = 0;
            while s1 < 0xA {
                spawn(fx, RING, s0, s1, 0x01E0_1F78);
                s1 += 1;
            }
            s0 += 1;
        }
        s1 = s0;
    }
    if (0x78..0x8D).contains(&c(fx)) {
        for (k, ra_node, ra) in [(2u32, 0x01E0_2004u32, 0x01E0_2018u32), (6, 0x01E0_2038, 0x01E0_204C), (7, 0x01E0_206C, 0x01E0_2080)] {
            node(fx, k, s0, s1, ra_node);
            spawn(fx, SPARKLE, s0, s1, ra);
        }
        spawn(fx, MOTE, s0, s1, 0x01E0_2098);
    }
    if c(fx) < 0xC9 {
        node(fx, 4, s0, s1, 0x01E0_20CC);
        let y = ee::sub(fx.mem.f(POS + 4), 13.0);
        fx.mem.setf(POS + 4, y);
        let z = fx.mem.f(POS + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 7.0) } else { ee::add(z, 7.0) };
        fx.mem.setf(POS + 8, z);
        spawn(fx, MOTE, s0, s1, 0x01E0_213C);
        spawn(fx, MOTE, s0, s1, 0x01E0_2150);
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

/// 0x1E02170 (D, task): b544's column: size 6, green ((0xA0, 0xFF, 0xA0) /
/// (0x80, 0xFF, 0x80)) before update 0x8D, else red ((0xFF, 0x80, 0x80)
/// both); by kind +0x50 (18, 1000) / (18, 250) / (18, 400) at +0x30 /
/// +0x34; in by 0x40 under 0x80 for 4 updates, out by 15; from age 1 +0x34
/// grows by 40 (kind 0) / 20 (kind 1).
fn column(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        if fx.mem.i16(COUNTER) < 0x8D {
            l2::rgb(fx, t + 0x60, 0xA0, 0xFF, 0xA0);
            l2::rgb(fx, t + 0x64, 0x80, 0xFF, 0x80);
        } else {
            l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
            l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
        }
        for (kind, h) in [(0, 0x447A_0000u32), (1, 0x437A_0000), (2, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, 0x4190_0000);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x11, 0xF);
    }
    for (kind, by) in [(0, 40.0f32), (1, 20.0)] {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(t + 0x54) != 0 {
            add(fx, t + 0x34, by);
        }
    }
    age(fx, t);
}

/// 0x1E02340 (D, task; the camera's frame s1 = the entry's index `k`):
/// b065's arc ring: radius 120, white outside, inside (0x40, 0xFF, 0x40)
/// for kind 0 / (0xFF, 0x80, 0x40) for kind 1, angles 10 / 10; its outer
/// alpha in by 0x60 under 0xC0 for 4 updates, then out by 15; turned by
/// the camera + 0x4000.
fn arc(fx: &mut Fx, t: u32, k: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x42F0_0000);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
            l2::rgb(fx, t + 0x60, 0x40, 0xFF, 0x40);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
            l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x40);
        }
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x67, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(k), 0x01E0_2444);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02470 (D, task; the camera's frame s1 = `k`): b544's sparkle in
/// green (0x40, 0xFF, 0x40): size 130 x 65, cell row (rand >> 4) % 3, a
/// velocity (rand % 100 - 50) / 60 never applied, +0x5C rand % 360, spins
/// +4 / +2 / +0 added to the camera turn (overwritten by it), kind 0..3;
/// in by 0x40 under 0x80 for 10 updates, out by 9.
fn sparkle(fx: &mut Fx, t: u32, k: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x82);
        let w = fx.mem.i16(t + 8) >> 1;
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_24A4);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r);
        l2::rgb(fx, t + 0x60, 0x40, 0xFF, 0x40);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_24D8u32), (0x30, 0x01E0_2504), (0x38, 0x01E0_2530)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_2588, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(k), 0x01E0_25A4);
        for (o, ra) in [(4u32, 0x01E0_25ACu32), (2, 0x01E0_25CC), (0, 0x01E0_25EC)] {
            let v = fx.rand_from(t, ra);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
        let v = fx.rand_from(t, 0x01E0_260C);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    for (a, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + a).wrapping_add(fx.mem.i16(t + s) as u32);
        fx.mem.write_u32(t + a, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(k), 0x01E0_26AC);
    age(fx, t);
}

/// 0x1E026D0 (D, task; s1 = D): b751's white mote: size 150, cell 2..4, a
/// random angle +0x5A ((rand >> 4) % 360), thrown 7.6 out along it, then
/// drifting 1 a update; in by 0x60 under 0xC0 for 6 updates, out by 0x13;
/// turned by the camera.
fn mote(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x96);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2708);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2740);
        let r = fx.div(v >> 4, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        let k = f32::from_bits(0x40F3_3333);
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, TAB, a, 0);
        add(fx, t + 0x18, ee::mul(k, s));
        let a = fx.mem.i16(t + 0x5A);
        let c = l2::sin_deg(fx, TAB, a, 1);
        add(fx, t + 0x10, ee::mul(k, c));
    }
    if fx.mem.i16(t + 0x54) < 6 {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0x15, 0x13);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, TAB, a, 0);
        add(fx, t + 0x18, s);
        let a = fx.mem.i16(t + 0x5A);
        let c = l2::sin_deg(fx, TAB, a, 1);
        add(fx, t + 0x10, c);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2858);
    age(fx, t);
}

/// 0x1E02880 (D, task): b418's ring: size 60, cell row (rand >> 4) % 8,
/// white, angles +0x5A / +0x5C rand % 360, +0xA 2 on odd rands else 0,
/// +0xC the kind, radius rand % 40 + 5 growing by (rand >> 4) % 70 + 45
/// from age 1, row 0..34, life 3..7; in by 0xC0 under 0xC0, out by 15;
/// grey 0x80 on odd ages, 0x60 on even.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4270_0000);
        let v = fx.rand_from(t, 0x01E0_28A8);
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let v = rmod(fx, t, 0x01E0_28E0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_28FC, 0x168);
        fx.mem.set16(t + 0x5C, v);
        let v = fx.rand_from(t, 0x01E0_2918);
        fx.mem.set16(t + 0xA, if v % 2 != 0 { 2 } else { 0 });
        let kind = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, kind);
        let v = rmod(fx, t, 0x01E0_2950, 0x28);
        fx.mem.set16(t + 0x50, v + 5);
        let v = fx.rand_from(t, 0x01E0_2970);
        let r = fx.div(v >> 4, 0x46).1;
        fx.mem.set16(t + 0xE, r + 0x2D);
        let v = rmod(fx, t, 0x01E0_2994, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_29B0, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0xC0);
    } else {
        fade_out(fx, t, 0x63, 0x11, 0xF);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let g = if fx.mem.i16(t + 0x54) % 2 == 1 { 0x80 } else { 0x60 };
    l2::rgb(fx, t + 0x60, g, g, g);
    age(fx, t);
}

/// 0x1E02AC0 (D, task; frame 0x50 at `sp`, the entry's pointer `p`): an
/// orange (0xFF, 0x80, 0x20) ribbon task, size 5; kind 0: +0xE 30 and on
/// its first update the histories 0..3 filled (19 points) with node 0x13
/// stacked 2.5 down a history, 4..7 with node 0x15; alpha 0xFF for 12
/// updates, then down by 9; kind 0 for 12 updates pushes those points
/// (`spawn::history`); +6 = 13.
fn ribbons(fx: &mut Fx, t: u32, sp: u32, p: u32) {
    l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x20);
    let mut s0 = u64::from(p);
    let down = f32::from_bits(0x4020_0000);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.set16(t + 0xE, 0x1E);
            for (n, first, ra) in [(0u32, 0u32, 0x01E0_2B38u32), (1, 4, 0x01E0_2CCC)] {
                let h = fx.self_handle();
                let slot = fx.mem.i16(NODES + 2 * n);
                node_position_saving(fx, h, slot, sp + 0x40, s0, Some(u64::from(t)), ra);
                for j in 0..4u32 {
                    if j != 0 {
                        let y = ee::sub(fx.mem.f(sp + 0x44), down);
                        fx.mem.setf(sp + 0x44, y);
                    }
                    s0 = u64::from(sp + 0x44);
                    let h0 = HIST + 0x140 * (first + j);
                    for e in 0..0x13u32 {
                        l2::copy3(fx, h0 + 0x10 * e, sp + 0x40);
                    }
                }
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0xC {
        fx.mem.write_u8(t + 0x63, 0xFF);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x50) == 0 && fx.mem.i16(t + 0x54) < 0xC {
        for (n, first, ra) in [(0u32, 0u32, 0x01E0_2EC0u32), (1, 4, 0x01E0_2F68)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2 * n);
            node_position_saving(fx, h, slot, sp + 0x40, s0, Some(u64::from(t)), ra);
            for j in 0..4u32 {
                if j != 0 {
                    let y = ee::sub(fx.mem.f(sp + 0x44), down);
                    fx.mem.setf(sp + 0x44, y);
                }
                s0 = u64::from(sp + 0x44);
                hist::history(fx, HIST + 0x140 * (first + j) + 0x130, 0x14, sp + 0x40);
            }
        }
    }
    fx.mem.set16(t + 6, 0xD);
    age(fx, t);
}

/// 0x1E03A10 (D, task, a, b): b703's ribbon (0x1E03E20, the same frame)
/// with the `b` side's alpha down by 3 a strip while above 4 (else 0)
/// instead of by alpha / (3 n); `saves` the draw loop's s0..s7, fp and the
/// return address.
fn ribbon(fx: &mut Fx, t: u32, a: u32, b: u32, saves: [u64; 10]) {
    use ee::add;
    let sp = fx.enter(0x300);
    let offs = [0u32, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90];
    let list: Vec<(u32, u64)> = offs.iter().zip(saves).map(|(&o, v)| (o, v)).collect();
    fx.saved(sp + 0x10, &list);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 0xD0, a);
    fx.mem.write_u32(sp + 0xC0, b);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x238;
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    let (ca, cb) = (sp + 0x2F8, sp + 0x2FC);
    for c in [ca, cb] {
        for k in 0..4 {
            let v = fx.mem.read_u8(t + 0x60 + k);
            fx.mem.write_u8(c + k, v);
        }
    }
    fx.mem.write_u8(ca + 3, 0);
    let (pa, pb) = (sp + 0x170, sp + 0x1B0);
    for (p, src) in [(pa, a), (pb, b)] {
        copy4(fx, p, src);
        copy4(fx, p + 0x10, src);
        copy4(fx, p + 0x20, src + 0x10);
        copy4(fx, p + 0x30, src + 0x20);
    }
    rgbaq_at(fx, sp + 0x2C8, ca, d + 0x10);
    rgbaq_at(fx, sp + 0x2D0, ca, d + 0x20);
    rgbaq_at(fx, sp + 0x2D8, cb, d + 0x30);
    rgbaq_at(fx, sp + 0x2E0, cb, d + 0x40);
    fx.mem.set32(sp + 0xB0, n - 1);
    let mut s7 = 0;
    while s7 < n - 1 {
        let off = (0x10 * (s7 - 1)) as u32;
        let (s2, s3) = (a.wrapping_add(off), b.wrapping_add(off));
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            rgbaq_at(fx, sp + 0x2E8, cb, d + 0x30);
            let v = i32::from(fx.mem.read_u8(cb + 3));
            fx.mem.write_u8(cb + 3, if v - 4 > 0 { (v - 3) as u8 } else { 0 });
            rgbaq_at(fx, sp + 0x2F0, cb, d + 0x40);
            let (qa, qb, oa, ob) = if s7 != 0 { (s2, s3, 0x1F0u32, 0x200u32) } else { (pa, pb, 0x210, 0x220) };
            spline::spline(fx, sp + 0x130, qa, f20, 0, [0.0; 4]);
            spline::spline(fx, sp + 0x150, qb, f20, 0, [0.0; 4]);
            f20 = add(f20, 0.25);
            spline::spline(fx, sp + oa, qa, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x140, sp + oa);
            spline::spline(fx, sp + ob, qb, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x160, sp + ob);
            for (to, from) in [(0xF0u32, 0x130u32), (0x100, 0x140), (0x110, 0x150), (0x120, 0x160)] {
                l2::copy3(fx, sp + to, sp + from);
            }
            if l2::project(fx, sp + 0xF0, SPR, 0, d, sp + 0xE0) {
                fx.submit(d, OT, 0);
            }
        }
        s7 += 1;
    }
    fx.leave(0x300);
}
