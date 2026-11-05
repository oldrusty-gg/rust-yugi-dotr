//! BMODULE b718 (card 128), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03E20) [F]. The entry
//! takes (D, param). b045's family: 150 tasks of 0x40 at D+0x478 (alive
//! +0x36, step +0x24), spawned by 0x1E04BB0 (b538's spawn at D+0x2D0 with
//! the colour D+0x3264, +0x30 = the kind D+0x32BA), the sines D+0x37C4 and
//! D+0xD8, timer D+0x3260 = 150, the OT at D+0x80 and D+0x326C. Nodes: the
//! attacker's 6 / 0xC / 0xE / 0x12 (0x13 / 0x14 / 0x15 overwrite it) at
//! D+0x32A8.., the target's 1 at D+0x32B6 (later a counter); textures (4,
//! 0..2) D+0x340.., (4, 0xD..0xF) D+0x374..; five jitter tables D+0x32C2 +
//! 0x100 k (part signed by the attacker's facing at init).
//!
//! The emitter 0x1E020D0 keeps the attacker's node 6 at D+0x2A30 and a
//! point D+0x320 (the node until update 0x50) pushed along z by the
//! facing; it spawns only fans 0x1E02F40 / 0x1E033E0 (two a update
//! 0x32..0x6D, and fifteen 0x2F40 with ten sparks 0x1E02DB0 on update 0x1A),
//! smoke 0x1E03A30 (until 0x51) and drifts 0x1E03780 (0x53..0x6E) at node
//! 0xC, on 0x53 at the target eight red fans 0x1E03620 and a disc
//! 0x1E03950, on 0x52 the camera light 0x1E03B90. The other steps
//! (0x2510 / 0x2730 / 0x2890 / 0x2A70 / 0x2B50 / 0x2C00 / 0x3180 / 0x3D10
//! / 0x3DD0) and their draws are never spawned [F]. Draws: sparks
//! billboard::ring_t20, fans quads4::ring_into plus (0x2F40) two streak
//! variants 0x1E04CD0 / 0x1E05130 of b045's 0x1E011B0 (here 0x1E01160,
//! with the 3620 fans), drifts b709's drift_draw (0x1E00550), the disc
//! quads4::fan18. Returns 1 on 0x48, 4 on 0x7D, 2 on 0x52, 8 once the
//! timer has run out.
use crate::blib::uv::uv_rect4;
use crate::blib::{billboard, exe as bexe, quads4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0x96;
const POOL: u32 = D + 0x478;
const POS: u32 = D + 0x2D0;
const POINT: u32 = D + 0x320;
const CAM: u32 = D + 0x2A00;
const TIMER: u32 = D + 0x3260;
const COLOUR: u32 = D + 0x3264;
const OTW: u32 = D + 0x326C;
const NODE: u32 = D + 0x32A8;
const SPINNER: u32 = D + 0x32B6;
const KIND: u32 = D + 0x32BA;
const PARAM: u32 = D + 0x32BC;
const COUNTER: u32 = D + 0x32BE;
const TABLES: u32 = D + 0x32C2;
const TAB: u32 = D + 0x37C4;

const SPARK: u32 = 0x01E0_2DB0;
const FAN_A: u32 = 0x01E0_2F40;
const FAN_B: u32 = 0x01E0_33E0;
const FAN_R: u32 = 0x01E0_3620;
const DRIFT: u32 = 0x01E0_3780;
const DISC: u32 = 0x01E0_3950;
const SMOKE: u32 = 0x01E0_3A30;
const LIGHT: u32 = 0x01E0_3B90;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1B0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x1B0);
    ret
}

fn rmod(fx: &mut Fx, m: i32) -> i32 {
    let v = fx.rand();
    fx.div(v, m).1
}

fn fix(fx: &mut Fx, v: f32) -> i32 {
    fx.fix_residue(v, 0);
    ee::fixsfsi(v)
}

fn fixu(fx: &mut Fx, at: u32) -> i32 {
    let v = fx.mem.f(at);
    fx.fix_residue(v, 0);
    ee::fixunssfsi(v) as i32
}

fn table(k: u32) -> u32 {
    TABLES + 0x100 * k
}

/// Table `k` bytes `range` = f(rand).
fn fill(fx: &mut Fx, k: u32, range: std::ops::Range<u32>, f: impl Fn(&mut Fx) -> i32) {
    for i in range {
        let v = f(fx);
        fx.mem.write_u8(table(k) + i, v as u8);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3930);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(PARAM, param);
    fx.mem.set32(TIMER, 0x96);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.set32(D + o, 0);
    }
    for (slot, n, clear) in [(0u32, 6, 1), (2, 0xC, 0), (4, 0xE, 0), (6, 0x12, 0), (6, 0x13, 0), (6, 0x14, 0), (6, 0x15, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODE + slot, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(SPINNER, r);
    for k in 0..3u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x340 + 4 * k, t);
    }
    for k in 0xDu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x374 + 4 * (k - 0xD), t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x100);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x104)), 180.0);
    let yaw = fix(fx, y);
    let signed = move |v: i32| if yaw != 0 { v } else { v.wrapping_neg() };
    fill(fx, 0, 0..0x100, |fx| rmod(fx, 0x1E) - 0xF);
    fill(fx, 0, 0x64..0xB4, |fx| rmod(fx, 0xF));
    fill(fx, 0, 0xB4..0x100, |fx| signed(rmod(fx, 0xF) + 5));
    fill(fx, 1, 0..0x100, |fx| rmod(fx, 0x1E) - 0xF);
    fill(fx, 1, 0x64..0xB4, |fx| fx.rand() & 3);
    fill(fx, 1, 0xB4..0x100, |fx| signed(rmod(fx, 0xF) + 5));
    fill(fx, 2, 0..0x100, |fx| rmod(fx, 0x1E) - 0xF);
    fill(fx, 2, 0x64..0xB4, |fx| rmod(fx, 0x14) + 5);
    fill(fx, 3, 0..0x100, |fx| rmod(fx, 0x1E) - 0xF);
    fill(fx, 3, 0x64..0xB4, |fx| rmod(fx, 0x1E) + 5);
    fill(fx, 4, 0..0x100, |fx| rmod(fx, 0xE) - 7);
    fill(fx, 4, 0xB4..0x100, |fx| signed(rmod(fx, 0xF) + 1));
    fill(fx, 4, 0x64..0xB4, |fx| rmod(fx, 6));
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        // The steps' prologue saves: the entry's s0 = D, s1 = D + 0x40 k.
        let low = fx.sp;
        let small = |fx: &mut Fx, ra: u32| fx.saved(low - 0x20, &[(0x10, u64::from(ra)), (0, u64::from(D))]);
        let mid = |fx: &mut Fx, ra: u32| fx.saved(low - 0x30, &[(0x20, u64::from(ra)), (0x10, u64::from(D + 0x40 * k)), (0, u64::from(D))]);
        match fx.mem.read_u32(t + 0x24) {
            SPARK => {
                spark(fx, t);
                small(fx, 0x01E0_44C0);
            }
            FAN_A => {
                fan(fx, t, 0xB, 0x1C);
                mid(fx, 0x01E0_44E0);
            }
            FAN_B => {
                fan(fx, t, 4, 0);
                mid(fx, 0x01E0_4520);
            }
            FAN_R => {
                burst(fx, t, 0xA, (0xC0, 0x30, 0x30), 0xE);
                small(fx, 0x01E0_4540);
            }
            DRIFT => {
                drift(fx, t);
                mid(fx, 0x01E0_4560);
            }
            DISC => {
                disc(fx, t);
                small(fx, 0x01E0_4580);
            }
            SMOKE => {
                burst(fx, t, 0x32, (0x30, 0x30, 0x30), 7);
                small(fx, 0x01E0_45A0);
            }
            LIGHT => {
                light(fx, t);
                mid(fx, 0x01E0_45C0);
            }
            _ => {}
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    let d = sp + 0x118;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let cell = |fx: &mut Fx, tex: u32, local: u32, id: i32| {
            let w = fx.mem.read_u32(tex);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x34);
            l2::rect(fx, local, id, c);
            for k in 0..4 {
                let w = fx.mem.read_u32(local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, w);
            }
        };
        match fx.mem.read_u32(t + 0x24) {
            SPARK => {
                cell(fx, D + 0x344, sp + 0x80, 0xA);
                billboard::ring_t20(fx, uv_rect4, D, t, d, table(0));
            }
            FAN_A => {
                cell(fx, D + 0x344, sp + 0x90, 0xA);
                fx.mem.set16(D + 0xB8, 0xF);
                streak_draw(fx, t, d, table(0), STREAK_A);
                fx.mem.set16(D + 0xB8, 9);
                streak_draw(fx, t, d, table(1), STREAK_B);
                cell(fx, D + 0x37C, sp + 0xA0, 0x9B);
                quads4::ring_into(fx, uv_rect4, D, t, d, table(0));
                fx.mem.set16(D + 0xB8, 0);
            }
            FAN_B => {
                fx.mem.set16(D + 0xB8, 5);
                cell(fx, D + 0x37C, sp + 0xB0, 0x9B);
                quads4::ring_into(fx, uv_rect4, D, t, d, table(0));
                fx.mem.set16(D + 0xB8, 0);
            }
            FAN_R => {
                fx.mem.set16(D + 0xB8, 0xF);
                cell(fx, D + 0x37C, sp + 0xC0, 0x9B);
                quads4::ring_into(fx, uv_rect4, D, t, d, table(2));
                streak_draw(fx, t, d, table(3), STREAK);
                fx.mem.set16(D + 0xB8, 0);
            }
            DISC => {
                cell(fx, D + 0x37C, sp + 0xD0, 0x9B);
                quads4::fan18(fx, D, t, d);
            }
            SMOKE => {
                cell(fx, D + 0x348, sp + 0xE0, 0x14);
                fx.mem.set16(D + 0xB8, 7);
                quads4::ring_into(fx, uv_rect4, D, t, d, table(4));
                fx.mem.set16(D + 0xB8, 0);
            }
            DRIFT => {
                cell(fx, D + 0x348, sp + 0xF0, 0x14);
                crate::b709::drift_draw(fx, 0x01E0_0550, t, d);
            }
            _ => {}
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x48 => 1,
        0x7D => 4,
        0x52 => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E04BB0(D, step, 0): the first free task from 0 (b538's spawn).
fn spawn(fx: &mut Fx, f: u32) -> i32 {
    let sp = fx.enter(0x60);
    let _ = sp;
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x40 * k + 0x36) == 0) {
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
            let a = fx.mem.i16(KIND);
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

fn set_pos(fx: &mut Fx, from: u32) {
    for c in 0..3 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(POS + 4 * c, w);
    }
}

/// 0x1E020D0(D, 0) (frame 0x70): the spawn colour (0x80, 0, 0, 0), the
/// update counter, node 6 at D+0x2A30, the point D+0x320 and the spawns.
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x70);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE);
    fx.node_position(h, n, D + 0x2A30);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x60);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x64)), 180.0);
    let yaw = fix(fx, y);
    let step = if fx.mem.i16(COUNTER) < 0x51 {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, POINT);
        10.0
    } else {
        3.0
    };
    let z = fx.mem.f(POINT + 8);
    let z = if yaw != 0 { ee::sub(z, step) } else { ee::add(z, step) };
    fx.mem.setf(POINT + 8, z);
    fx.mem.set16(KIND, 0);
    let c = fx.mem.i16(COUNTER);
    if (0x32..0x6E).contains(&c) {
        spawn(fx, FAN_A);
        spawn(fx, FAN_B);
    }
    if fx.mem.i16(COUNTER) < 0x52 {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, sp + 0x40);
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE + 2);
        fx.node_position(h, n, sp + 0x50);
        set_pos(fx, sp + 0x50);
        fx.mem.set16(KIND, 0);
        spawn(fx, SMOKE);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x53..0x6F).contains(&c) {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE + 2);
        fx.node_position(h, n, sp + 0x50);
        set_pos(fx, sp + 0x50);
        fx.mem.set16(KIND, 0);
        spawn(fx, DRIFT);
    }
    fx.mem.set16(KIND, 0);
    if fx.mem.i16(COUNTER) % 2 == 1 {
        let s = fx.mem.i16(SPINNER);
        fx.mem.set16(KIND, s);
        fx.mem.set16(SPINNER, s + 1);
        if fx.mem.i16(SPINNER) >= 0x1E {
            fx.mem.set16(SPINNER, 0);
        }
    }
    if fx.mem.i16(COUNTER) == 0x1A {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE + 4);
        fx.node_position(h, n, sp + 0x40);
        set_pos(fx, sp + 0x40);
        fx.mem.set16(KIND, 1);
        for _ in 0..0xF {
            spawn(fx, FAN_A);
        }
        for _ in 0..0xA {
            spawn(fx, SPARK);
        }
        let y = ee::add(1.0, fx.mem.f(sp + 0x44));
        fx.mem.setf(POS + 4, y);
    }
    if fx.mem.i16(COUNTER) == 0x53 {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE + 6);
        fx.node_position(h, n, sp + 0x40);
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x50);
        set_pos(fx, sp + 0x50);
        fx.mem.set16(KIND, 0);
        for _ in 0..8 {
            spawn(fx, FAN_R);
        }
        spawn(fx, DISC);
    }
    if fx.mem.i16(COUNTER) == 0x52 {
        set_pos(fx, POINT);
        fx.mem.set16(KIND, 1);
        spawn(fx, LIGHT);
    }
    fx.leave(0x70);
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    n
}

/// Alpha +0x2B down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= floor {
        fx.mem.write_u8(t + 0x2B, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// Alpha 0x80 while the age is under +6, then out by `down` (floor + 1).
fn hold(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, floor, down);
    }
}

fn grow(fx: &mut Fx, t: u32, extra: i32) {
    let v = fx.mem.i16(t + 0x30).wrapping_add(fx.mem.i16(t + 0xE)).wrapping_add(extra);
    fx.mem.set16(t + 0x30, v);
}

/// 0x1E02DB0 (D, task): an orange spark ring (size 3, cell rand & 3,
/// turned rand % 0xFFFF (z, x), radius from 0 by 5..21, +8 rand % 10, life
/// 2..7 at 0x80, then out by 10).
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 3);
        let r = fx.rand();
        fx.mem.set16(t + 0x34, r & 3);
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        let r = rmod(fx, 0xFFFF);
        fx.mem.set32(t + 0x18, r);
        let r = rmod(fx, 0xFFFF);
        fx.mem.set32(t + 0x10, r);
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rmod(fx, 0x11);
        fx.mem.set16(t + 0xE, r + 5);
        let r = rmod(fx, 0xA);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, 6);
        fx.mem.set16(t + 6, r + 2);
        fx.mem.set16(t + 0x36, 2);
    }
    hold(fx, t, 0x11, 0xA);
    grow(fx, t, 0);
    age(fx, t);
}

/// 0x1E02F40 (size 11; late growth + 0x1C) / 0x1E033E0 (size 4) (D, task)
/// (frame 0x30): an orange fan at the point D+0x320 (height 50) turning by
/// 5 a update (+0x18; down for kind 0), +0x3A = the kind, radius from rand
/// % 5 by 12..31 (+ `late` from update 0xBC / 0xC3 by kind), +8 rand % 50,
/// life 8..24 at 0x80, then out by 0x20.
fn fan(fx: &mut Fx, t: u32, size: i32, late: i32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, size);
        let r = fx.rand();
        fx.mem.set16(t + 0x34, r & 3);
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        let r = rmod(fx, 0x168);
        fx.mem.set32(t + 0x18, r);
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, k);
        let r = rmod(fx, 5);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, 0x14);
        fx.mem.set16(t + 0xE, r + 0xC);
        let r = rmod(fx, 0x32);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, 0x11);
        fx.mem.set16(t + 6, r + 8);
        fx.mem.set16(t + 0x36, 2);
    }
    hold(fx, t, 0x21, 0x20);
    let v = fx.mem.f(POINT);
    let x = fix(fx, v);
    fx.mem.set16(t, x);
    fx.mem.set16(t + 2, 0x32);
    let v = fx.mem.f(POINT + 8);
    let z = fix(fx, v);
    fx.mem.set16(t + 4, z);
    let kind = fx.mem.i16(t + 0x3A);
    let v = fx.mem.i32(t + 0x18) + if kind != 0 { 5 } else { -5 };
    fx.mem.set32(t + 0x18, v);
    let c = fx.mem.i16(COUNTER);
    if fx.mem.i16(t + 0x3A) == 0 {
        grow(fx, t, if c < 0xBC { 0 } else { late });
    }
    if fx.mem.i16(t + 0x3A) == 1 {
        grow(fx, t, if c < 0xC3 { 0 } else { late });
    }
    age(fx, t);
}

/// 0x1E03620 (red, size 10, life 14..33) / 0x1E03A30 (grey smoke, size
/// 50, life 7..26) (D, task): cell rand & 3, +0x18 rand % 360, +0x3A = the
/// kind, radius from 0 by 12..31, +8 rand % 50; at 0x80 for its life, then
/// out by 0x20.
fn burst(fx: &mut Fx, t: u32, size: i32, c: (i32, i32, i32), life: i32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, size);
        let r = fx.rand();
        fx.mem.set16(t + 0x34, r & 3);
        l2::rgb(fx, t + 0x28, c.0, c.1, c.2);
        let r = rmod(fx, 0x168);
        fx.mem.set32(t + 0x18, r);
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, k);
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, 0x14);
        fx.mem.set16(t + 0xE, r + 0xC);
        let r = rmod(fx, 0x32);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, 0x14);
        fx.mem.set16(t + 6, r + life);
        fx.mem.set16(t + 0x36, 2);
    }
    hold(fx, t, 0x21, 0x20);
    if fx.mem.i16(t + 0x32) != 0 {
        grow(fx, t, 0);
    }
    age(fx, t);
}

/// i16 `at` += `k` x (sine or cosine of i16 `deg`) through 0x1A2B80.
fn wobble(fx: &mut Fx, t: u32, at: u32, deg: u32, cos: i32, k: f32) {
    let a = fx.mem.i16(t + deg);
    let s = l2::sin_deg(fx, TAB, a, cos);
    let f = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(k, s));
    let r = fix(fx, f);
    fx.mem.set16(t + at, r);
}

/// 0x1E03780 (D, task) (frame 0x30): b112's grey drift (size 0x41, alpha
/// 0x70 for 10 updates then out by 10, the cell up every third update to
/// 7, wobbling by 8 / 2 / 8).
fn drift(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x41);
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x40);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fx.mem.write_u8(t + 0x2B, 0x70);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    let n = fx.mem.i16(t + 0x32);
    if fx.div(n, 3).1 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 7 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    age(fx, t);
    wobble(fx, t, 8, 0x3A, 0, 8.0);
    wobble(fx, t, 0xA, 0x30, 1, 2.0);
    wobble(fx, t, 0xC, 0x3A, 1, 8.0);
}

/// 0x1E03950 (D, task): the disc (radius 430, grey 0xC0 centre, 0x10 rim
/// of alpha 0x10) at 0x80 for 5 updates, then out by 12.
fn disc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x1AE);
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x2C, 0x10, 0x10, 0x10);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0x10);
    }
    if fx.mem.i16(t + 0x32) < 5 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x11, 0xC);
    }
    age(fx, t);
}

/// 0x1E03B90 (D, task) (frame 0x30): b350's camera light from D+0x2A00 =
/// (128, 8, 4, 0), the task's point, (0, 180, w).
fn light(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    let _ = sp;
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4100_0000), (8, 0x4080_0000), (0xC, 0), (0x20, 0), (0x24, 0x4334_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        for k in 0..3 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(CAM + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    let w = fx.mem.f(CAM + 0x28);
    if fx.mem.i16(t + 0x30) == 0 {
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, f32::from_bits(0x3E4C_CCCD)));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else if w <= f32::from_bits(0x3DCC_CCCD) {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, f32::from_bits(0x3DCC_CCCD)));
    }
    let p = if fx.mem.i16(t + 0x36) == 0 { 0 } else { CAM };
    bexe::camera_slots45(fx, p);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.leave(0x30);
}

/// A streak compile: the near vertex pair, the far pair and the far
/// factors per axis.
struct Streak {
    near: [u32; 2],
    far: [u32; 2],
    k: [u32; 3],
}

/// 0x1E01160 (b045's 0x1E011B0).
const STREAK: Streak = Streak { near: [0x70, 0x80], far: [0x90, 0xA0], k: [0x3F33_3333; 3] };
/// 0x1E04CD0: the far pair x / y by 0.9.
const STREAK_A: Streak = Streak { near: [0x70, 0x80], far: [0x90, 0xA0], k: [0x3F66_6666, 0x3F66_6666, 0x3F33_3333] };
/// 0x1E05130: vertices 0 / 2 near, 1 / 3 far by 0.9 / 0.9 / 0.5.
const STREAK_B: Streak = Streak { near: [0x70, 0x90], far: [0x80, 0xA0], k: [0x3F66_6666, 0x3F66_6666, 0x3F00_0000] };

/// 0x1E01160 / 0x1E04CD0 / 0x1E05130 (D, task, desc, table) (frame
/// 0x140): b045's streak: D+0xB8 quads (4 x +0x38) turned about x by the
/// camera's first angle (D+0xD8's sines), pushed out by the table bytes [k
/// + +8 (+0, +100, +180)] / 20 x (+0x30 + k / 2) (the far pair by the
/// compile's factors), at the task's point (scale 0.1), on the 9-value
/// descriptor; keyed by the mean w on D+0x80.
fn streak_draw(fx: &mut Fx, t: u32, d: u32, table: u32, s: Streak) {
    use ee::{add, mul};
    let sp = fx.enter(0x140);
    fx.mem.set16(d + 2, 0x24);
    fx.mem.set16(d, 9);
    fx.mem.write_u64(d + 8, (4 << 32) | 0x2424_2421);
    fx.mem.set32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    fx.desc_alpha(d, (0x80 << 32) | 0x48);
    fx.mem.set32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    l2::rgbaq(fx, sp + 0x138, t + 0x28);
    for k in 0..2 {
        let w = fx.mem.read_u32(sp + 0x138 + 4 * k);
        fx.mem.write_u32(d + 0x10 + 4 * k, w);
    }
    uv_rect4(fx, d, D + 0x94, 0);
    let w = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, w, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xC0 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x120);
    for k in 0..3u32 {
        let v = fx.mem.i32(sp + 0x120 + 4 * k).wrapping_add(fx.mem.i32(t + 0x10 + 4 * k));
        fx.mem.set32(sp + 0x120 + 4 * k, v);
    }
    l2::camera_angles_to(fx, sp + 0x120);
    let y = fx.mem.i32(sp + 0x124);
    fx.mult(0xB60B_60B7_u32 as i32, y);
    let hi = fx.hilo().1;
    let q = (hi.wrapping_add(y) >> 7).wrapping_add(((y as u32) >> 31) as i32);
    fx.mem.set32(sp + 0x124, q.wrapping_neg());
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xB0 + 4 * k, v);
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xC0, 0, sp + 0xB0);
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0xB8) {
        let h = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x70, 4.0, h);
        l2::rotate4(fx, sp + 0x70, sp + 0x120, 0, D + 0xD8);
        let row = table.wrapping_add(k as u32).wrapping_add(fx.mem.i16(t + 8) as u32);
        for (axis, (c, off)) in [(0u32, 0u32), (4, 0x64), (8, 0xB4)].into_iter().enumerate() {
            let b = fx.mem.read_u8(row + off) as i8 as i32;
            let f4 = ee::div(ee::from_i32(b), 20.0);
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + (k >> 1));
            let near = mul(f4, r);
            for v in s.near {
                let x = add(fx.mem.f(sp + v + c), near);
                fx.mem.setf(sp + v + c, x);
            }
            let kf = f32::from_bits(s.k[axis]);
            for v in s.far {
                let r = ee::from_i32(fx.mem.i16(t + 0x30) + (k >> 1));
                let far = mul(kf, mul(f4, r));
                let x = add(fx.mem.f(sp + v + c), far);
                fx.mem.setf(sp + v + c, x);
            }
        }
        if l2::project(fx, sp + 0x70, SPR, sp + 0xE0, d, sp + 0xD0) {
            let key = fixu(fx, sp + 0xD0);
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x140);
}
