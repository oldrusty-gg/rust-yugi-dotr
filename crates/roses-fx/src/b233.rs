//! BMODULE b233 (card 180), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03D20, data area 0x1ED0000) [F]: b207's program shape on 150 tasks of 0x40 bytes at D+0x2F0
//! (spawn 0x1E04720 on colour D+0x3934, point D+0x250, +0x30 = D+0x396E,
//! +0x3A = D+0x3968). Attacker nodes 0x36 / 0x37 / 0xC / 0x14 (D+0x395E..);
//! histories D+0x28C0 (node 0x36) / D+0x2960 (node 0x37), filled on update
//! 0xA3, pushed by `spawn::node_history`. The emitter 0x1E01E60 counts
//! D+0x3972: on 1 the object op 0x1F8380(4, ..) (task 0x1E03900) and a
//! screen tint 0x1E03980; on 0x42 a second tint 0x1E03B60; on 0x59 / 0x6F
//! / 0xAA ten spinning sprites 0x1E03090 at node 0xC, on 0x64 / 0x78 / 0xAA
//! ten at node 0x14 (the record's sprite 0x1E04850); on 0xA3 a `ribbon_g`
//! ribbon 0x1E02FE0, a hidden glow 0x1E02F10 and two ribbons 0x1E031F0
//! (`ribbon_tex` twice) / 0x1E032D0 (`ribbon_tex1`) between the
//! histories; on 0xA5..0xB1 three a update of a drifting sprite 0x1E02D60
//! (at thirds of node 0x37's first history step) and a streak 0x1E033B0
//! (the record's 0x1E04A30, from node 0x36's point to node 0x37's, kept in
//! the 60-entry table D+0x31B0); on 0xA7 a disc 0x1E03490 (`fan18_far0`),
//! three cross quads 0x1E03570 (`quarters_far`, 0 / 45 / 90 degrees) and
//! the camera light 0x1E036D0. Steps 0x1E02640 / 0x2860 / 0x29C0 / 0x2B60
//! / 0x2C50 / 0x3840 are tested but never spawned (not ported). Returns 1
//! on update 0xA0, 4 on 0xD2, 6 on 0xAB..0xBD, 8 once the 240-update timer
//! D+0x3930 has run out (2 on 0x4AB, never reached). No 0x1F8100.
use crate::blib::{quads4, spawn as bspawn, spline, uv};
use crate::bus::Bus;
use crate::c061::{desc_uv4, rgbaq_at};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;

const DRIFT: u32 = 0x01E0_2D60;
const GLOW: u32 = 0x01E0_2F10;
const RIBBON: u32 = 0x01E0_2FE0;
const SPRITE: u32 = 0x01E0_3090;
const TEX2: u32 = 0x01E0_31F0;
const TEX1: u32 = 0x01E0_32D0;
const STREAK: u32 = 0x01E0_33B0;
const DISC: u32 = 0x01E0_3490;
const CROSS: u32 = 0x01E0_3570;
const LIGHT: u32 = 0x01E0_36D0;
const OBJECT: u32 = 0x01E0_3900;
const TINT: u32 = 0x01E0_3980;
const TINT2: u32 = 0x01E0_3B60;

const POOL: u32 = D + 0x2F0;
const POS: u32 = D + 0x250;
const LAMP: u32 = D + 0x2870;
const HIST_A: u32 = D + 0x28C0;
const HIST_B: u32 = D + 0x2960;
const DRIFTS: u32 = D + 0x2DF0;
const ENDS: u32 = D + 0x31B0;
const TIMER: u32 = D + 0x3930;
const COLOUR: u32 = D + 0x3934;
const OT2: u32 = D + 0x393C;
const INDEX: u32 = D + 0x395C;
const NODES: u32 = D + 0x395E;
const ANGLE: u32 = D + 0x3968;
const PARAM: u32 = D + 0x396E;
const COUNTER: u32 = D + 0x3972;
const SINES: u32 = D + 0x3A74;
const TEX_E: u32 = D + 0x2E8;
const TEX_F: u32 = D + 0x2EC;
const TEX5: u32 = D + 0x2DD4;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E03D20 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x190);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x190);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3BE0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3970, param);
    fx.mem.set32(TIMER, 0xF0);
    for (k, (n, clear)) in [(0x36, 1), (0x37, 0), (0xC, 0), (0x14, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2D8 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x2B8, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0x2DD0 + 4 * k as u32, t);
    }
    // rand's frame: s0 = D.
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_3E4C);
        let r = fx.div(v, 0x1E).1;
        fx.mem.write_u8(D + 0x3974 + k, (r - 0xF) as u8);
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

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    if counter(fx) == 0xA3 {
        fill(fx, NODES, sp + 0xD0, HIST_A);
    }
    if counter(fx) == 0xA3 {
        fill(fx, NODES + 2, sp + 0xE0, HIST_B);
    }
    let n = fx.mem.i16(NODES);
    bspawn::node_history(fx, HIST_A + 0x90, 0xA, n);
    let n = fx.mem.i16(NODES + 2);
    bspawn::node_history(fx, HIST_B + 0x90, 0xA, n);
    for k in 0..150 {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            DRIFT => run_step(fx, t, 0x30, 0x01E0_4030, drift),
            GLOW => run_step(fx, t, 0x20, 0x01E0_4050, glow),
            RIBBON => ribbon_step(fx, t),
            SPRITE => run_step(fx, t, 0x30, 0x01E0_4118, sprite),
            TEX2 => run_step(fx, t, 0x20, 0x01E0_4138, ribbon_fade),
            TEX1 => run_step(fx, t, 0x20, 0x01E0_4158, ribbon_fade),
            STREAK => run_step(fx, t, 0x20, 0x01E0_4178, streak),
            DISC => run_step(fx, t, 0x20, 0x01E0_4198, disc),
            CROSS => run_step(fx, t, 0x20, 0x01E0_41B8, cross),
            LIGHT => run_step(fx, t, 0x30, 0x01E0_41D8, light),
            OBJECT => {
                for (to, from) in [(0x18Cu32, 0x01E0_4D80u32), (0x188, 0x01E0_4D88)] {
                    let w = fx.mem.read_u32(from);
                    fx.mem.write_u32(sp + to, w);
                }
                fx.boundary(0x001F_8380, "object op 0x1F8380", [4, 0, sp + 0x18C, sp + 0x188]);
                if fx.mem.i16(t + 0x32) < 0x41 {
                    fx.mem.set16(t + 0x36, 0);
                }
                let n = fx.mem.i16(t + 0x32) + 1;
                fx.mem.set16(t + 0x32, n);
            }
            TINT => run_step(fx, t, 0x80, 0x01E0_4288, |fx, t| tint(fx, t, true)),
            TINT2 => run_step(fx, t, 0x80, 0x01E0_42A8, |fx, t| tint(fx, t, false)),
            _ => {}
        }
    }
    let d = sp + 0xF8;
    for k in 0..150 {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            DRIFT => {
                tcell(fx, t, TEX5, sp + 0x60, 0x14);
                square(fx, t, d);
            }
            RIBBON => spline::ribbon_g(fx, t, HIST_A, HIST_B),
            SPRITE => {
                tcell(fx, t, TEX5, sp + 0x70, 0x14);
                square(fx, t, d);
            }
            TEX2 => {
                tcell(fx, t, TEX_F, sp + 0x80, 0x97);
                fx.mem.set16(D + 0xBE, 0);
                tcell(fx, t, TEX_F, sp + 0x90, 0x97);
                let h = fx.mem.i16(D + 0x9A) >> 1;
                fx.mem.set16(D + 0x9A, h);
                fx.mem.set16(D + 0xBE, 0);
                spline::ribbon_tex(fx, D, t, HIST_B, HIST_A);
                fx.mem.set16(D + 0xBE, 0);
                spline::ribbon_tex(fx, D, t, HIST_B, HIST_A);
                fx.mem.set16(D + 0xBE, 0);
            }
            TEX1 => {
                tcell(fx, t, TEX_F, sp + 0xA0, 0x97);
                fx.mem.set16(D + 0xBE, 0);
                tcell(fx, t, TEX_E, sp + 0xB0, 0x8C);
                for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x1E), (0x9A, 0x1E)] {
                    fx.mem.set16(D + o, v);
                }
                fx.mem.set16(D + 0xBE, 0);
                spline::ribbon_tex1(fx, D, t, HIST_B, HIST_A);
                fx.mem.set16(D + 0xBE, 0);
            }
            STREAK => streak_draw(fx, t, d),
            DISC => quads4::fan18_far0(fx, D, t, d),
            CROSS => {
                cell(fx, TEX_F, sp + 0xC0, 0x9A, 1);
                for k in 0..3 {
                    let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                    fx.mem.setf(sp + 0x50 + 4 * k, v);
                }
                quads4::quarters_far(fx, D, t, d, sp + 0x50);
            }
            _ => {}
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    let c = counter(fx);
    match c {
        0xA0 => 1,
        0xD2 => 4,
        0x4AB => 2,
        _ if (0xAB..0xBE).contains(&c) => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The texture at `tex` and the atlas cell `id` frame `f` (0x106960 into
/// `r`) copied to D+0x88 / D+0x94..+0xA3.
fn cell(fx: &mut Fx, tex: u32, r: u32, id: i32, f: i32) {
    let v = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, v);
    l2::rect(fx, r, id, f);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// [`cell`] at the task's frame +0x34.
fn tcell(fx: &mut Fx, t: u32, tex: u32, r: u32, id: i32) {
    let f = fx.mem.i16(t + 0x34);
    cell(fx, tex, r, id, f);
}

/// A step's frame and its saves of the entry's s0 = D (and s1 = 0 in a
/// 0x30 frame).
fn run_step(fx: &mut Fx, t: u32, size: u32, ra: u32, f: impl FnOnce(&mut Fx, u32)) {
    let sp = fx.enter(size);
    let (ra, d) = (u64::from(ra), u64::from(D));
    if size == 0x30 {
        fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, d)]);
    } else {
        fx.saved(sp, &[(0x10, ra), (0, d)]);
    }
    f(fx, t);
    fx.leave(size);
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    fx.mem.i16(t + 0x32)
}

fn grow(fx: &mut Fx, t: u32) {
    let n = age(fx, t) + 1;
    fx.mem.set16(t + 0x32, n);
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x36) == 1
}

/// Alpha `o` down by `down` while at least `floor`, else the task ends.
fn fall(fx: &mut Fx, t: u32, o: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + o);
    if a >= floor {
        fx.mem.write_u8(t + o, a - down);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

fn rmod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// The i16 field `o` moved by the float `by` (0x1A2B80 from `ra`).
fn nudge(fx: &mut Fx, t: u32, o: u32, by: f32, ra: u32) {
    let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), by);
    fx.fix_residue(v, ra);
    fx.mem.set16(t + o, ee::fixsfsi(v));
}

/// 0x1E02D60: a drifting sprite (size 0x14, frame 5, grey (0x40, 0x40,
/// 0x40), angle rand % 360; its drift D+0x2DF0 + 0x10 +0x30 set to half
/// the x step between the histories' first points, thrice): 0x80 for 4
/// updates, then out by 0xF; +8 / +0xA / +0xC moved by the drift.
fn drift(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x14);
        fx.mem.set16(t + 0x34, 5);
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x40);
        let r = rmod(fx, t, 0x01E0_2DB0, 0x168);
        fx.mem.set16(t + 0x3A, r);
        for k in 0..3u32 {
            let v = ee::div(ee::sub(fx.mem.f(HIST_B), fx.mem.f(HIST_A)), 2.0);
            let i = fx.mem.i16(t + 0x30);
            fx.mem.setf(DRIFTS.wrapping_add((i << 4) as u32) + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 4 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x2B, 0x11, 0xF);
    }
    for (k, ra) in [(0u32, 0x01E0_2E98u32), (1, 0x01E0_2EC0), (2, 0x01E0_2EE8)] {
        let i = fx.mem.i16(t + 0x30);
        let by = fx.mem.f(DRIFTS.wrapping_add((i << 4) as u32) + 4 * k);
        nudge(fx, t, 8 + 2 * k, by, ra);
    }
    grow(fx, t);
}

/// 0x1E02F10: a glow never drawn ((0x20, 0x20, 0x80), +6 = 9, size 2,
/// angle rand % 360): alpha up by 0x1B (below 0xC0) for 10 updates, then
/// out by 0xB.
fn glow(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x80);
        fx.mem.set16(t + 6, 9);
        fx.mem.write_u8(t + 0x2B, 0);
        let r = rmod(fx, t, 0x01E0_2F50, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x38, 2);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0xA {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x2B, a + 0x1B);
        }
    } else {
        fall(fx, t, 0x2B, 0xD, 0xB);
    }
    grow(fx, t);
}

/// The entry's inline step for 0x1E02FE0: the ribbon (0x80, 0x10, 0xF0),
/// +6 = 9, width 6, alpha 0x60 for 7 updates, then out by 9.
fn ribbon_step(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0x80, 0x10, 0xF0);
        fx.mem.set16(t + 6, 9);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.set16(t + 0x38, 6);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x60);
    } else {
        fall(fx, t, 0x2B, 0xB, 9);
    }
    grow(fx, t);
}

/// 0x1E03090: a spinning sprite (size 0x3C, frame 5, grey, +0x30 / +0x3A
/// random angles); 0x80 for 7 updates, then out by 0xF; +8 / +0xC moved by
/// 16 sin / cos(+0x3A).
fn sprite(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x3C);
        fx.mem.set16(t + 0x34, 5);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let r = rmod(fx, t, 0x01E0_30E0, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, t, 0x01E0_30FC, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x2B, 0x11, 0xF);
    }
    for (o, cos, ra) in [(8u32, 0, 0x01E0_318Cu32), (0xC, 1, 0x01E0_31C4)] {
        let deg = fx.mem.i16(t + 0x3A);
        let s = l2::sin_deg(fx, SINES, deg, cos);
        nudge(fx, t, o, ee::mul(16.0, s), ra);
    }
    grow(fx, t);
}

/// 0x1E031F0 / 0x1E032D0: a white ribbon (both colours (0x80, 0x80,
/// 0x80), alphas 0x80, +6 = 9, size 6): 0x80 for 7 updates, then both
/// alphas out by 0xB (the task ends with the first).
fn ribbon_fade(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x2C, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 6, 9);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.write_u8(t + 0x2F, 0x80);
        fx.mem.set16(t + 0x38, 6);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.write_u8(t + 0x2F, 0x80);
    } else {
        fall(fx, t, 0x2B, 0xD, 0xB);
        let a = fx.mem.read_u8(t + 0x2F);
        if a >= 0xD {
            fx.mem.write_u8(t + 0x2F, a - 0xB);
        }
    }
    grow(fx, t);
}

/// 0x1E033B0: a streak (size 0 growing by 2, frame 2, (0x40, 0x10, 0x80),
/// +8 / +0xA / +0xC from 0 up by 1 an update): alpha up by 0x20 for 4
/// updates, then out by 5.
fn streak(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        fx.mem.set16(t + 0x34, 2);
        for o in [8u32, 0xA, 0xC] {
            fx.mem.set16(t + o, 0);
        }
        l2::rgb(fx, t + 0x28, 0x40, 0x10, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 4 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x20));
    } else {
        fall(fx, t, 0x2B, 7, 5);
    }
    for o in [8u32, 0xA, 0xC] {
        let v = fx.mem.i16(t + o) + 1;
        fx.mem.set16(t + o, v);
    }
    let s = fx.mem.i16(t + 0x38) + 2;
    fx.mem.set16(t + 0x38, s);
    grow(fx, t);
}

/// 0x1E03490: the disc (size 0xFA, (0xC0, 0xC0, 0xC0) / (0x80, 0x20,
/// 0xF0) rim alpha 0, angle rand % 360): alpha up by 0x60 on ages 0 / 1,
/// then out by 6.
fn disc(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0xFA);
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x2C, 0x80, 0x20, 0xF0);
        let r = rmod(fx, t, 0x01E0_34E4, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x60));
    } else {
        fall(fx, t, 0x2B, 8, 6);
    }
    grow(fx, t);
}

/// 0x1E03570: a cross quad (size 6, (0x20, 0x20, 0x80) / (0x80, 0x20,
/// 0x80) rim alpha 0, +8 = 0x14, +0xA 0x96 / 0x64 / 0xFA by the angle 0 /
/// 45 / 90): both alphas up by 0x40 (below 0x80) on ages 0 / 1, then out
/// by 5; +0xA up by 0x28 and the angle by 10 an update.
fn cross(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 6);
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x80);
        l2::rgb(fx, t + 0x2C, 0x80, 0x20, 0x80);
        fx.mem.set16(t + 8, 0x14);
        for (a, v) in [(0, 0x96), (0x2D, 0x64), (0x5A, 0xFA)] {
            if fx.mem.i16(t + 0x3A) == a {
                fx.mem.set16(t + 0xA, v);
            }
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 2 {
        for o in [0x2Bu32, 0x2F] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x40);
            }
        }
    } else {
        fall(fx, t, 0x2B, 7, 5);
        let a = fx.mem.read_u8(t + 0x2F);
        fx.mem.write_u8(t + 0x2F, if a >= 7 { a - 5 } else { 0 });
    }
    let v = fx.mem.i16(t + 0xA) + 0x28;
    fx.mem.set16(t + 0xA, v);
    grow(fx, t);
    let v = fx.mem.i16(t + 0x3A) + 10;
    fx.mem.set16(t + 0x3A, v);
}

/// 0x1E036D0: the camera light D+0x2870 (camera 0's slots 4 / 5, 0x1F7E20;
/// (0, 4, 4, 4), (0, 70, level)) at the task's point: its level D+0x2898
/// up by 0.2 to 1, then down by 0.1 to 0.1, when the task ends (the slots
/// cleared); it ends with the timer at 1 too.
fn light(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        for (o, v) in [(0u32, 0u32), (4, 0x4080_0000), (8, 0x4080_0000), (0xC, 0x4080_0000), (0x20, 0), (0x24, 0x428C_0000), (0x28, 0)] {
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

/// 0x1E03980 (`inset`: the ST inset by +0x30 / 512) / 0x1E03B60 (ST 0..1):
/// a window sprite (0x6FF8 .. 0x8FF8) sampling the frame (0x207EF0, filter
/// 1) built at the step's sp + 0x28, grey (0x80, 0x80, 0x80, alpha +0x2B),
/// ALPHA 0x44, key 0 in the module OT; alpha up by 0xC below 0x60 for 0x41
/// updates, then out by 0xC (0x1E03B60: up by 8 below 0x40 for 0x37, out
/// by 9); +0x30 counts up to 0x1E.
fn tint(fx: &mut Fx, t: u32, inset: bool) {
    let sp = fx.sp;
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..3 {
        fx.mem.write_u8(sp + 0x38 + k, 0x80);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    fx.mem.write_u8(sp + 0x3B, a);
    fx.mem.write_u32(sp + 0x3C, 0x3F80_0000);
    for (at, v) in [(sp + 0x48, 0x6FF8u16), (sp + 0x58, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    let s = if inset { ee::div(ee::from_i32(fx.mem.i16(t + 0x30)), 512.0).to_bits() } else { 0 };
    for (o, v) in [(0x40u32, s), (0x44, s), (0x50, 0x3F80_0000), (0x54, 0x3F80_0000)] {
        fx.mem.write_u32(sp + o, v);
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let (n, cap, up, floor, down) = if inset { (0x41, 0x60, 0xC, 0xE, 0xC) } else { (0x37, 0x40, 8, 0xB, 9) };
    if age(fx, t) < n {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < cap {
            fx.mem.write_u8(t + 0x2B, a + up);
        }
    } else {
        fall(fx, t, 0x2B, floor, down);
    }
    grow(fx, t);
    let v = fx.mem.i16(t + 0x30);
    if v < 0x1E {
        fx.mem.set16(t + 0x30, v + 1);
    }
}

/// 0x1E04850(D, task, desc) (frame 0x110): one textured square (9 values,
/// FST, ALPHA 0x48) into `desc` of the i16 size +0x38 facing the camera
/// at the task's i16 point plus its drift +8 / +0xA / +0xC over 10 (scale
/// 0.1), coloured +0x28, UVs D+0x94 in orientation 0, texture D+0x88;
/// keyed by its mean w into D+0x80 (`billboard::sprite_drift` into the
/// caller's descriptor).
fn square(fx: &mut Fx, t: u32, d: u32) {
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
    crate::b494::camera_angles_saving(fx, sp + 0xF0, u64::from(d), u64::from(t), 0x01E0_4930);
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

/// 0x1E04A30(D, task, desc) (frame 0xB0): the streak quad: cell 0x9B frame
/// +0x34 as its UVs, texture (4, 0xF), colour +0x28, ALPHA 0x48; the
/// corners (0, +-size / 10) moved to the table's two points A / B (D+0x31B0
/// / D+0x31C0 + 0x20 +0x30), the B side further by a third of B - A times
/// +8 / +0xA / +0xC; through the camera, keyed by the mean w into D+0x393C
/// twice.
fn streak_draw(fx: &mut Fx, t: u32, d: u32) {
    use ee::{add, div, from_i32, mul, sub};
    let sp = fx.enter(0xB0);
    let f = fx.mem.i16(t + 0x34);
    l2::rect(fx, sp + 0x90, 0x9B, f);
    desc_uv4(fx, d);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, sp + 0xA8, t + 0x28, d + 0x10);
    uv::uv_rect4(fx, d, sp + 0x90, 0);
    let tex = fx.mem.read_u32(TEX_F);
    fx.desc_texture(d, tex, 1);
    let h = div(from_i32(fx.mem.i16(t + 0x38)), 10.0);
    l2::quad_xy(fx, sp + 0x40, 0.0, h);
    let e = ENDS.wrapping_add((fx.mem.i16(t + 0x30) << 5) as u32);
    let a = [fx.mem.f(e), fx.mem.f(e + 4), fx.mem.f(e + 8)];
    let b = [fx.mem.f(e + 0x10), fx.mem.f(e + 0x14), fx.mem.f(e + 0x18)];
    let dv: [f32; 3] = std::array::from_fn(|k| mul(div(sub(b[k], a[k]), 3.0), from_i32(fx.mem.i16(t + 8 + 2 * k as u32))));
    for k in 0..3u32 {
        for q in [0x40u32, 0x60] {
            let v = add(fx.mem.f(sp + q + 4 * k), a[k as usize]);
            fx.mem.setf(sp + q + 4 * k, v);
        }
    }
    for k in 0..3u32 {
        for q in [0x50u32, 0x70] {
            let v = add(fx.mem.f(sp + q + 4 * k), add(b[k as usize], dv[k as usize]));
            fx.mem.setf(sp + q + 4 * k, v);
        }
    }
    if l2::project(fx, sp + 0x40, SPR, 0, d, sp + 0x80) {
        for _ in 0..2 {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x80)) as i32;
            let ot = fx.mem.read_u32(OT2);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0xB0);
}

/// 0x1E04720(D, step, 0) (frame 0x60: ra +0x50, the runner's s4 (0) [H],
/// the emitter's s3 = D / s2 / s1 / s0): the first free task cleared and
/// set off at D+0x250 (as i16) with the colour D+0x3934 (twice), +0x30 =
/// D+0x396E and +0x3A = D+0x3968.
fn spawn(fx: &mut Fx, f: u32, ra: u32, s: [u64; 3]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, u64::from(D)), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    let ret = match (0..150u32).find(|&k| fx.mem.i16(POOL + 0x36 + 0x40 * k) == 0) {
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
            let p = fx.mem.i16(PARAM);
            fx.mem.set16(t + 0x30, p);
            let a = fx.mem.i16(ANGLE);
            fx.mem.set16(t + 0x3A, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// Node slot `k`'s position into the emitter's sp + 0x50.
fn node(fx: &mut Fx, sp: u32, k: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES + 2 * k);
    fx.node_position(h, n, sp + 0x50);
}

fn pos_from(fx: &mut Fx, p: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(p + 4 * k);
        fx.mem.write_u32(POS + 4 * k, w);
    }
}

/// The spawn point (x, y - 10, 40 + z) of the unit point at `p`.
fn at_unit(fx: &mut Fx, p: u32) {
    let x = fx.mem.f(p);
    fx.mem.setf(POS, x);
    let y = fx.mem.f(p + 4);
    fx.mem.setf(POS + 4, ee::sub(y, 10.0));
    let z = fx.mem.f(p + 8);
    fx.mem.setf(POS + 8, ee::add(40.0, z));
}

/// 0x1E01E60(D, 0) (frame 0x70: ra +0x40, s3 the runner's (0) [H], s2 =
/// the parameter, s1 = 0, s0 = D): the spawn colour (0x80, 0, 0, 0), the
/// update counter, the spawn points and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x70);
    let p = param as i64 as u64;
    let d = u64::from(D);
    fx.saved(sp, &[(0x40, 0x01E0_3E88), (0x30, 0), (0x20, p), (0x10, 0), (0, d)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    // The emitter's s0 / s1 / s2.
    let mut s = [d, 0u64, p];
    if counter(fx) == 0xA3 {
        spawn(fx, RIBBON, 0x01E0_1EB8, s);
        spawn(fx, TEX2, 0x01E0_1EDC, s);
        spawn(fx, GLOW, 0x01E0_1F00, s);
        spawn(fx, TEX1, 0x01E0_1F24, s);
    }
    if counter(fx) == 1 {
        spawn(fx, TINT, 0x01E0_1F48, s);
        spawn(fx, OBJECT, 0x01E0_1F6C, s);
    }
    for (slot, at, ra) in [(2u32, [0x59, 0x6F, 0xAA], 0x01E0_1FDCu32), (3, [0x64, 0x78, 0xAA], 0x01E0_2060)] {
        if at.contains(&counter(fx)) {
            node(fx, sp, slot);
            for k in 0..10u64 {
                s[0] = k;
                pos_from(fx, sp + 0x50);
                spawn(fx, SPRITE, ra, s);
            }
            s[0] = 10;
        }
    }
    if counter(fx) == 0xA7 {
        node(fx, sp, 0);
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x60);
        for (to, from) in [(0u32, 0x60u32), (4, 0x54), (8, 0x68)] {
            let w = fx.mem.read_u32(sp + from);
            fx.mem.write_u32(POS + to, w);
        }
        spawn(fx, DISC, 0x01E0_20D8, s);
        for (k, (a, ra)) in [(0, 0x01E0_20F4u32), (0x2D, 0x01E0_2118), (0x5A, 0x01E0_213C)].into_iter().enumerate() {
            fx.mem.set16(PARAM, k as i32);
            fx.mem.set16(ANGLE, a);
            spawn(fx, CROSS, ra, s);
        }
        spawn(fx, LIGHT, 0x01E0_2150, s);
    }
    let c = counter(fx);
    if (0xA5..0xB2).contains(&c) {
        use ee::{add, div, from_i32, mul, sub};
        node(fx, sp, 0);
        s[0] = u64::from(sp + 0x64);
        s[1] = u64::from(sp + 0x68);
        for k in 0..3u64 {
            s[2] = k;
            let f = from_i32(k as i32);
            let i = fx.mem.i16(INDEX);
            fx.mem.set16(PARAM, i);
            for (hist, ends, step, ra) in [(HIST_B, ENDS + 0x10, DRIFT, 0x01E0_2270u32), (HIST_A, ENDS, STREAK, 0x01E0_2348)] {
                for j in 0..3u32 {
                    let v = div(sub(fx.mem.f(hist + 0x10 + 4 * j), fx.mem.f(hist + 4 * j)), 3.0);
                    fx.mem.setf(sp + 0x60 + 4 * j, v);
                }
                for j in 0..3u32 {
                    let v = add(fx.mem.f(hist + 4 * j), mul(fx.mem.f(sp + 0x60 + 4 * j), f));
                    fx.mem.setf(POS + 4 * j, v);
                }
                let e = ends.wrapping_add((fx.mem.i16(PARAM) << 5) as u32);
                for j in 0..3u32 {
                    let v = fx.mem.read_u32(POS + 4 * j);
                    fx.mem.write_u32(e + 4 * j, v);
                }
                spawn(fx, step, ra, s);
            }
            let i = fx.mem.i16(INDEX) + 1;
            fx.mem.set16(INDEX, i);
            if fx.mem.i16(INDEX) >= 0x3C {
                fx.mem.set16(INDEX, 0);
            }
        }
    }
    if counter(fx) == 0x42 {
        spawn(fx, TINT2, 0x01E0_239C, s);
    }
    if counter(fx) == 1 {
        let h = fx.self_handle();
        fx.unit_position(h, D + 0x290);
        let h = fx.self_handle();
        fx.unit_position(h, D + 0x2A0);
    }
    let c = counter(fx);
    if (0x70..0xC3).contains(&c) {
        let z = fx.mem.f(D + 0x298);
        fx.mem.setf(D + 0x298, ee::sub(z, f32::from_bits(0x4019_999A)));
        at_unit(fx, D + 0x290);
        fx.mem.set16(PARAM, 0);
    }
    let c = counter(fx);
    if (0xAB..0xE1).contains(&c) {
        let z = fx.mem.f(D + 0x2A8);
        fx.mem.setf(D + 0x2A8, ee::sub(z, f32::from_bits(0x4073_3333)));
        at_unit(fx, D + 0x2A0);
        fx.mem.set16(PARAM, 1);
    }
    let c = counter(fx);
    if (0x70..0xBF).contains(&c) {
        at_unit(fx, D + 0x290);
        fx.mem.set16(PARAM, 0);
    }
    let c = counter(fx);
    if (0xA9..0xDD).contains(&c) {
        at_unit(fx, D + 0x2A0);
        fx.mem.set16(PARAM, 1);
    }
    let c = counter(fx);
    if (0x30..=0x33).contains(&c) {
        node(fx, sp, 0);
        pos_from(fx, sp + 0x50);
        fx.mem.set16(PARAM, 0);
    }
    let c = counter(fx);
    if (0x33..=0x36).contains(&c) {
        node(fx, sp, 1);
        pos_from(fx, sp + 0x50);
        fx.mem.set16(PARAM, 1);
    }
    fx.leave(0x70);
}
