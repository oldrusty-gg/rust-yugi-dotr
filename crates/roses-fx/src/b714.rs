//! BMODULE b714 (card 353), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E032B0, data area 0x1ED0000) [F]: b018's family (200 tasks of 0x40 bytes at D+0x478, alive +0x36,
//! step +0x24; spawn 0x1E040B0 = b018's on colour D+0x3B44, parameter
//! D+0x3B9A and the point D+0x2D0) with eight attacker nodes D+0x3B88..
//! (4 / 5 / 6 / 0xD / 0xE / 0xF / 0x17 / 0x1A; the first six swapped to 8 /
//! 0xA / 0xC / 7 / 9 / 0xB on update 0x67 and back on 0x74) and seven
//! 10-point histories: D+0x3A90 (node 0) filled on 0x56, D+0x36D0 /
//! 0x3770 / 0x3810 / 0x38B0 / 0x3950 / 0x39F0 (nodes 0..5) filled on 0x6A
//! and pushed below update 0x72. The emitter 0x1E01DB0 counts D+0x3B9E:
//! on 1 and 0x75 six node lines 0x1E02300 (one per node 0..5); every third
//! update from 1 below 0x6A an ember cloud 0x1E02450 (`ring_t`) at a
//! random node 0..5; on 0x13 / 0x47 / 0x2C / 0x62 six smoke sprites
//! 0x1E02E50 at node 0x17 (2 lower); on 1 the object op task 0x1E03230
//! (0x1F8380(2, 0, ..) each update to timer 0xA); on 0x6A three
//! `ribbon_cross` (0x1E02DA0) and three `ribbon_g` (0x1E02840) tasks
//! between the histories; on 0x6F at node 0 eight `ring_t20` rings
//! 0x1E02C30, eight `ring_t20` sparks 0x1E02AA0 (`tstep::spark`), thirty
//! `ring_t` embers 0x1E028F0 (`tstep::ember`), a `fan18` fire disc
//! 0x1E02760 and b018's camera light 0x1E02FF0. The steps 0x1E025D0 /
//! 0x1E03170 are tested but never spawned (not ported). Returns 1 on update
//! 0x64, 4 on 0x78, 2 on 0x6E, 8 once the 190-update timer D+0x3B40 has run
//! out. No 0x1F8100.
use crate::blib::{billboard, quads4, spawn as bspawn, spline, tstep, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
const POOL: u32 = D + 0x478;

const LINE: u32 = 0x01E0_2300;
const CLOUD: u32 = 0x01E0_2450;
const FIRE: u32 = 0x01E0_2760;
const RIBBON: u32 = 0x01E0_2840;
const EMBER: u32 = 0x01E0_28F0;
const SPARK: u32 = 0x01E0_2AA0;
const RING: u32 = 0x01E0_2C30;
const CROSS: u32 = 0x01E0_2DA0;
const SMOKE: u32 = 0x01E0_2E50;
const LIGHT: u32 = 0x01E0_2FF0;
const OBJECT: u32 = 0x01E0_3230;

const POS: u32 = D + 0x2D0;
const LAMP: u32 = D + 0x3680;
const KEEP: u32 = D + 0x36C0;
/// The six node histories (nodes 0..5) and the seventh (node 0).
const HISTS: [u32; 6] = [D + 0x36D0, D + 0x3770, D + 0x3810, D + 0x38B0, D + 0x3950, D + 0x39F0];
const HIST7: u32 = D + 0x3A90;
const TIMER: u32 = D + 0x3B40;
const COLOUR: u32 = D + 0x3B44;
const NODES: u32 = D + 0x3B88;
const PARAM: u32 = D + 0x3B9A;
const COUNTER: u32 = D + 0x3B9E;
const BYTES: u32 = D + 0x3BA2;
const SINES: u32 = D + 0x3CA4;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E032B0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1D0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x3B4C, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x1D0);
    ret
}

/// The first six attacker nodes requested again (the first clearing).
fn nodes6(fx: &mut Fx, ids: [i32; 6]) {
    for (k, n) in ids.into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3E10);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3B9C, param);
    fx.mem.set32(TIMER, 0xBE);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, n) in [4, 5, 6, 0xD, 0xE, 0xF, 0x17, 0x1A].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x340 + 4 * k as u32, t);
    }
    for k in 0xD..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x374 + 4 * (k as u32 - 0xD), t);
    }
    // rand's frame: s0 = D.
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_3434);
        let r = fx.div(v, 0x1E).1;
        fx.mem.write_u8(BYTES + k, (r - 0xF) as u8);
    }
    0
}

/// A history's nine points from node slot `k`'s position (through `local`).
fn fill(fx: &mut Fx, k: u32, local: u32, hist: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES + 2 * k);
    fx.node_position(h, n, local);
    for i in 0..9 {
        for c in 0..3 {
            let w = fx.mem.read_u32(local + 4 * c);
            fx.mem.write_u32(hist + 0x10 * i + 4 * c, w);
        }
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    if counter(fx) == 0x56 {
        fill(fx, 0, sp + 0xC0, HIST7);
    }
    for (k, h) in HISTS.into_iter().enumerate() {
        if counter(fx) == 0x6A {
            fill(fx, k as u32, sp + 0xD0 + 0x10 * k as u32, h);
        }
    }
    if counter(fx) == 0x67 {
        nodes6(fx, [8, 0xA, 0xC, 7, 9, 0xB]);
    }
    if counter(fx) == 0x74 {
        nodes6(fx, [4, 5, 6, 0xD, 0xE, 0xF]);
    }
    let n = fx.mem.i16(NODES);
    bspawn::node_history(fx, HIST7 + 0x90, 0xA, n);
    for (k, h) in HISTS.into_iter().enumerate() {
        if counter(fx) < 0x72 {
            let n = fx.mem.i16(NODES + 2 * k as u32);
            bspawn::node_history(fx, h + 0x90, 0xA, n);
        }
    }
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            LINE => run_step(fx, t, 0x40, 0x01E0_3924, |fx, t, sp| line(fx, t, sp)),
            CLOUD => run_step(fx, t, 0x20, 0x01E0_3948, |fx, t, sp| cloud(fx, t, sp)),
            FIRE => run_step(fx, t, 0x20, 0x01E0_3988, |fx, t, sp| fire(fx, t, sp)),
            RIBBON => ribbon_step(fx, t),
            EMBER => run_step(fx, t, 0x20, 0x01E0_3A48, |fx, t, _| {
                tstep::ember(fx, EMBER, t);
            }),
            SPARK => run_step(fx, t, 0x20, 0x01E0_3A68, |fx, t, _| {
                tstep::spark(fx, SPARK, t);
            }),
            RING => run_step(fx, t, 0x20, 0x01E0_3A88, |fx, t, sp| ring(fx, t, sp)),
            CROSS => cross_step(fx, t),
            SMOKE => run_step(fx, t, 0x30, 0x01E0_3B50, |fx, t, sp| smoke(fx, t, sp)),
            LIGHT => run_step(fx, t, 0x30, 0x01E0_3B70, |fx, t, sp| light(fx, t, sp)),
            OBJECT => {
                for (to, from) in [(0x1CCu32, 0x01E0_4300u32), (0x1C8, 0x01E0_4308)] {
                    let w = fx.mem.read_u32(from);
                    fx.mem.write_u32(sp + to, w);
                }
                fx.boundary(0x001F_8380, "object op 0x1F8380", [2, 0, sp + 0x1CC, sp + 0x1C8]);
                if fx.mem.i32(TIMER) == 0xA {
                    fx.mem.set16(t + 0x36, 0);
                }
            }
            _ => {}
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) != 0 {
            draw(fx, sp, t);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x64 => 1,
        0x78 => 4,
        0x6E => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The texture at `tex` and the atlas cell `id` frame +0x34 (0x106960
/// into `r`) copied to D+0x88 / D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, tex: u32, r: u32, id: i32) {
    let v = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, v);
    let f = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, f);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// The entry's draw case for a live task (`sp` the entry's frame, the
/// descriptor at sp + 0x138).
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let d = sp + 0x138;
    match fx.mem.read_u32(t + 0x24) {
        LINE => line_draw(fx, t, d),
        CLOUD => {
            cell(fx, t, D + 0x37C, sp + 0x60, 0x9E);
            billboard::ring_t(fx, uv::uv_rect4, D, t, d, BYTES);
        }
        FIRE => {
            cell(fx, t, D + 0x37C, sp + 0x70, 0x9B);
            quads4::fan18(fx, D, t, d);
        }
        RIBBON => {
            for (k, (a, b)) in [(HISTS[0], HISTS[3]), (HISTS[1], HISTS[4]), (HISTS[2], HISTS[5])].into_iter().enumerate() {
                if fx.mem.i16(t + 0x30) == k as i32 {
                    spline::ribbon_g(fx, t, a, b);
                }
            }
        }
        EMBER => {
            cell(fx, t, D + 0x37C, sp + 0x80, 0x9B);
            billboard::ring_t(fx, uv::uv_rect4, D, t, d, BYTES);
        }
        SPARK => {
            cell(fx, t, D + 0x344, sp + 0x90, 0xA);
            billboard::ring_t20(fx, uv::uv_rect4, D, t, d, BYTES);
        }
        RING => {
            cell(fx, t, D + 0x37C, sp + 0xA0, 0x9F);
            billboard::ring_t20(fx, uv::uv_rect4, D, t, d, BYTES);
        }
        CROSS => {
            for (k, a) in [HISTS[0], HISTS[1], HISTS[2]].into_iter().enumerate() {
                if fx.mem.i16(t + 0x30) == k as i32 {
                    spline::ribbon_cross(fx, t, a);
                }
            }
        }
        SMOKE => {
            cell(fx, t, D + 0x348, sp + 0xB0, 0x14);
            sprite(fx, t, d);
            sprite(fx, t, d);
        }
        _ => {}
    }
}

/// 0x109A90(d, c0, c1): a gouraud line header (4 values, flags 0x11,
/// ALPHA 0x48), RGBAQ c0 / c1 with Q 1, the vertex alphas cleared.
fn desc_line(fx: &mut Fx, d: u32, c0: u32, c1: u32) {
    fx.mem.set16(d + 2, 0x11);
    fx.mem.set16(d, 4);
    fx.mem.write_u64(d + 8, 0x4141);
    fx.mem.write_u32(d + 4, 0);
    fx.desc_alpha(d, 0x80_0000_0048);
    for k in 0..4 {
        let v = fx.mem.read_u8(c0 + k);
        fx.mem.write_u8(d + 0x10 + k, v);
        let v = fx.mem.read_u8(c1 + k);
        fx.mem.write_u8(d + 0x20 + k, v);
    }
    fx.mem.write_u32(d + 0x24, 0x3F80_0000);
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    fx.mem.write_u8(d + 0x1F, 0);
    fx.mem.write_u8(d + 0x2F, 0);
}

/// 0x1E041D0(D, task, desc, 0) (frame 0x70): a vertical line from the
/// task's i16 point up by i16 +6, coloured +0x28 at both ends, keyed by
/// its mean w (submitted whether or not it is on screen); the second
/// vertex's X copied from the first's after the projection.
fn line_draw(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x70);
    desc_line(fx, d, t + 0x28, t + 0x28);
    fx.bzero(sp + 0x40, 0x20);
    let f = |fx: &mut Fx, o: u32| ee::from_i32(fx.mem.i16(t + o));
    let y = f(fx, 2);
    fx.mem.setf(sp + 0x44, y);
    let h = f(fx, 6);
    fx.mem.setf(sp + 0x54, ee::add(y, h));
    for (to, o) in [(0x40u32, 0u32), (0x48, 4), (0x50, 0), (0x58, 4)] {
        let v = f(fx, o);
        fx.mem.setf(sp + to, v);
    }
    l2::project(fx, sp + 0x40, 0x7000_0000, 0, d, sp + 0x60);
    let x = fx.mem.read_u16(d + 0x18);
    fx.mem.write_u16(d + 0x28, x);
    let key = ee::fixunssfsi(fx.mem.f(sp + 0x60)) as i32;
    let ot = fx.mem.read_u32(D + 0x3B4C);
    fx.submit(d, ot, key);
    fx.leave(0x70);
}

/// 0x1E005A0(D, task, desc) (frame 0x110): one textured square (9 values,
/// FST, ALPHA 0x48) of the i16 size +0x38 facing the camera (its angles,
/// scale 0.1) at the i16 point +0 / +2 / +4 plus i16 +8 / +0xA / +0xC over
/// 10; colour +0x28, UVs from D+0x94 (orientation 0), texture D+0x88;
/// keyed by its mean w into D+0x80.
fn sprite(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x110);
    crate::c061::desc_uv4(fx, d);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    crate::c061::rgbaq_at(fx, sp + 0x108, t + 0x28, d + 0x10);
    uv::uv_rect4(fx, d, D + 0x94, 0);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    crate::c061::scale_tenth(fx, sp + 0x90);
    l2::camera_angles_to(fx, sp + 0xF0);
    for k in 0..3 {
        let off = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + 2 * k)), off);
        fx.mem.setf(sp + 0x80 + 4 * k, v);
    }
    l2::xform_vp(fx, sp + 0xB0, sp + 0x90, sp + 0xF0, sp + 0x80);
    let size = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x40, size, size);
    if l2::project(fx, sp + 0x40, 0x7000_0000, sp + 0xB0, d, sp + 0xA0) {
        let key = ee::fixunssfsi(fx.mem.f(sp + 0xA0)) as i32;
        let ot = fx.mem.read_u32(D + 0x80);
        fx.submit(d, ot, key);
    }
    fx.leave(0x110);
}

/// A step's frame and its saves of the entry's s0 = D (and s1 = 0 in a
/// 0x30 / 0x40 frame).
fn run_step(fx: &mut Fx, t: u32, size: u32, ra: u32, f: impl FnOnce(&mut Fx, u32, u32)) {
    let sp = fx.enter(size);
    let (ra, d) = (u64::from(ra), u64::from(D));
    if size == 0x20 {
        fx.saved(sp, &[(0x10, ra), (0, d)]);
    } else {
        fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, d)]);
    }
    f(fx, t, sp);
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

/// Alpha +0x2B down by `down` while at least `floor`, else the task ends.
fn fall(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= floor {
        fx.mem.write_u8(t + 0x2B, a - down);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E02300 (frame `sp`, 0x40): the line at node slot +0x30 (pale gold
/// (0xA0, 0x80, 8)): alpha up by 0xC below 0x80 while the timer is at
/// least 0x10 until age 0x66, then out by 0xA; out by 0xA too once the
/// timer is under 0xF; its point the node's (0x1A2B80) and +6 = 0x32
/// below age 0x65.
fn line(fx: &mut Fx, t: u32, sp: u32) {
    let h = fx.self_handle();
    let k = fx.mem.i16(t + 0x30);
    let n = fx.mem.i16(NODES.wrapping_add((2 * k) as u32));
    fx.node_position(h, n, sp + 0x30);
    l2::rgb(fx, t + 0x28, 0xA0, 0x80, 8);
    if first(fx, t) {
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0x66 {
        if fx.mem.i32(TIMER) >= 0x10 {
            let a = fx.mem.read_u8(t + 0x2B);
            if a < 0x80 {
                fx.mem.write_u8(t + 0x2B, a + 0xC);
            }
        }
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    if fx.mem.i32(TIMER) < 0xF {
        fall(fx, t, 0x11, 0xA);
    }
    grow(fx, t);
    if age(fx, t) < 0x65 {
        for (k, ra) in [(0u32, 0x01E0_2414u32), (1, 0x01E0_2420), (2, 0x01E0_242C)] {
            let v = fx.mem.f(sp + 0x30 + 4 * k);
            fx.fix_residue(v, ra);
            fx.mem.set16(t + 2 * k, ee::fixsfsi(v));
        }
        fx.mem.set16(t + 6, 0x32);
    }
}

/// 0x1E02450: an ember cloud (size 0xA, frame rand % 2 + 2, +0x14 rand %
/// 0xFFFF, (0x30, 0x28, 8), radius +0x30 from 0 growing by rand % 9 + 3
/// after update 0, angle rand % 360, +8 rand % 10, life rand % 7 + 5):
/// alpha up by 0x30 (below 0x61) for the life, then out by 0xA.
fn cloud(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0xA);
        let v = fx.rand_from(t, 0x01E0_2478);
        fx.mem.set16(t + 0x34, (v & 1) + 2);
        let v = fx.rand_from(t, 0x01E0_2498);
        let r = fx.div(v, 0xFFFF).1;
        fx.mem.set32(t + 0x14, r);
        l2::rgb(fx, t + 0x28, 0x30, 0x28, 8);
        fx.mem.set16(t + 0x30, 0);
        let v = fx.rand_from(t, 0x01E0_24C4);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
        let v = fx.rand_from(t, 0x01E0_24E0);
        let r = fx.div(v, 9).1;
        fx.mem.set16(t + 0xE, r + 3);
        let v = fx.rand_from(t, 0x01E0_2500);
        let r = fx.div(v, 0xA).1;
        fx.mem.set16(t + 8, r);
        let v = fx.rand_from(t, 0x01E0_251C);
        let r = fx.div(v, 7).1;
        fx.mem.set16(t + 6, r + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x61 {
            fx.mem.write_u8(t + 0x2B, a + 0x30);
        }
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, v);
    }
    grow(fx, t);
}

/// 0x1E02760: b018's fire disc (radius 0x14A, (0xC0, 0x70, 0) / (0x20,
/// 0x10, 0) rim alpha 0x10, a random angle): alpha 0x80 for 5 updates,
/// then out by 0xA.
fn fire(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x14A);
        l2::rgb(fx, t + 0x28, 0xC0, 0x70, 0);
        l2::rgb(fx, t + 0x2C, 0x20, 0x10, 0);
        let v = fx.rand_from(t, 0x01E0_27B4);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0x10);
    }
    if age(fx, t) < 5 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    grow(fx, t);
}

/// 0x1E02C30: b018's ember ring with size 5.
fn ring(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 5);
        let v = fx.rand_from(t, 0x01E0_2C58);
        fx.mem.set16(t + 0x34, v & 3);
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        let v = fx.rand_from(t, 0x01E0_2C8C);
        let r = fx.div(v, 0xFFFF).1;
        fx.mem.set32(t + 0x18, r);
        fx.mem.set16(t + 0x30, 0);
        let v = fx.rand_from(t, 0x01E0_2CAC);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
        let v = fx.rand_from(t, 0x01E0_2CC8);
        fx.mem.set16(t + 0xE, (v & 0xF) + 3);
        let v = fx.rand_from(t, 0x01E0_2CE8);
        let r = fx.div(v, 0xA).1;
        fx.mem.set16(t + 8, r);
        let v = fx.rand_from(t, 0x01E0_2D04);
        let r = fx.div(v, 3).1;
        fx.mem.set16(t + 6, r + 1);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, v);
    grow(fx, t);
}

/// 0x1E02E50: a smoke sprite (size 0x32, grey 0x20, frame 0 stepping to 7
/// every third update, two random angles +0x30 / +0x3A): alpha 0x80 for
/// 10 updates, then out by 0xA; its offset +8 / +0xC moves by 4 (sin,
/// cos) of +0x3A (0x108910 on D+0x3CA4) each update (0x1A2B80).
fn smoke(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x32);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x20);
        let v = fx.rand_from(t, 0x01E0_2E9C);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x30, r);
        let v = fx.rand_from(t, 0x01E0_2EB8);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0xA {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    let a = age(fx, t);
    if fx.div(a, 3).1 == 1 {
        let f = fx.mem.i16(t + 0x34);
        if f < 7 {
            fx.mem.set16(t + 0x34, f + 1);
        }
    }
    grow(fx, t);
    for (o, cos, ra) in [(8u32, 0, 0x01E0_2F90u32), (0xC, 1, 0x01E0_2FC8)] {
        let deg = fx.mem.i16(t + 0x3A);
        let s = l2::sin_deg(fx, SINES, deg, cos);
        let m = ee::mul(4.0, s);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), m);
        fx.fix_residue(v, ra);
        fx.mem.set16(t + o, ee::fixsfsi(v));
    }
}

/// The entry's inline step for 0x1E02840: the ribbon (0x40, 0x30, 0),
/// width 5, alpha 0x60 for 0xC updates, then out by 9; +6 = 10.
fn ribbon_step(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x40, 0x30, 0);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 5);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0xC {
        fx.mem.write_u8(t + 0x2B, 0x60);
    } else {
        fall(fx, t, 0xB, 9);
    }
    fx.mem.set16(t + 6, 0xA);
    grow(fx, t);
}

/// The entry's inline step for 0x1E02DA0: the cross ribbon (0xA0, 0x80,
/// 8), +6 = 9, width 6, alpha 0x60 for 0xC updates, then out by 9.
fn cross_step(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0xA0, 0x80, 8);
        fx.mem.set16(t + 6, 9);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.set16(t + 0x38, 6);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0xC {
        fx.mem.write_u8(t + 0x2B, 0x60);
    } else {
        fall(fx, t, 0xB, 9);
    }
    grow(fx, t);
}

/// 0x1E02FF0: b018's camera light (D+0x3680, camera 0's slots 4 / 5) with
/// the timer D+0x3B40.
fn light(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
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

/// 0x1E01DB0(D, 0) (frame 0x50: ra +0x20, s1 +0x10 = 0, s0 = D): the spawn
/// colour (0x80, 0, 0, 0), the update counter and the spawns; `s1` the
/// emitter's s1 at each spawn (s2 the entry's, the parameter).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x20, 0x01E0_3470), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    let self_node = |fx: &mut Fx, k: u32, out: u32| {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 2 * k);
        fx.node_position(h, n, out);
    };
    let to_pos = |fx: &mut Fx, from: u32| {
        for k in 0..3 {
            let w = fx.mem.read_u32(from + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    };
    if counter(fx) == 0x33 {
        self_node(fx, 1, KEEP);
    }
    let c = counter(fx);
    if (0x34..0x41).contains(&c) {
        self_node(fx, 1, sp + 0x30);
        to_pos(fx, KEEP);
    }
    let mut s1 = 0u64;
    let c = counter(fx);
    if c == 1 || c == 0x75 {
        for (k, ra) in [0x01E0_1E9Cu32, 0x01E0_1EB8, 0x01E0_1ED4, 0x01E0_1EF0, 0x01E0_1F0C, 0x01E0_1F28].into_iter().enumerate() {
            fx.mem.set16(PARAM, k as i32);
            spawn(fx, LINE, ra, s1, p);
        }
    }
    let c = counter(fx);
    if fx.div(c, 3).1 == 1 && c < 0x6A {
        let v = fx.rand_from(D, 0x01E0_1F60);
        let k = fx.div(v, 6).1 as i16 as i32;
        s1 = k as i64 as u64;
        self_node(fx, k as u32, sp + 0x30);
        to_pos(fx, sp + 0x30);
        spawn(fx, CLOUD, 0x01E0_1FCC, s1, p);
    }
    for (a, b, ra) in [(0x2C, 0x62, 0x01E0_203Cu32), (0x13, 0x47, 0x01E0_20C0)] {
        let c = counter(fx);
        if c == a || c == b {
            self_node(fx, 6, sp + 0x30);
            for k in 0..6u64 {
                s1 = k;
                let x = fx.mem.read_u32(sp + 0x30);
                fx.mem.write_u32(POS, x);
                let y = fx.mem.f(sp + 0x34);
                fx.mem.setf(POS + 4, ee::sub(y, 2.0));
                let z = fx.mem.read_u32(sp + 0x38);
                fx.mem.write_u32(POS + 8, z);
                spawn(fx, SMOKE, ra, s1, p);
            }
            s1 = 6;
        }
    }
    if counter(fx) == 1 {
        spawn(fx, OBJECT, 0x01E0_20F4, s1, p);
    }
    if counter(fx) == 0x6A {
        for (k, ra) in [0x01E0_211Cu32, 0x01E0_2138, 0x01E0_2154].into_iter().enumerate() {
            fx.mem.set16(PARAM, k as i32);
            spawn(fx, CROSS, ra, s1, p);
        }
        for (k, ra) in [0x01E0_216Cu32, 0x01E0_2188, 0x01E0_21A4].into_iter().enumerate() {
            fx.mem.set16(PARAM, k as i32);
            spawn(fx, RIBBON, ra, s1, p);
        }
    }
    if counter(fx) == 0x6F {
        self_node(fx, 0, sp + 0x30);
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x40);
        to_pos(fx, sp + 0x30);
        fx.mem.set16(PARAM, 1);
        for k in 0..8u64 {
            s1 = k;
            spawn(fx, RING, 0x01E0_2218, s1, p);
        }
    }
    if counter(fx) == 0x6F {
        self_node(fx, 0, sp + 0x30);
        to_pos(fx, sp + 0x30);
        fx.mem.set16(PARAM, 1);
        for k in 0..8u64 {
            s1 = k;
            spawn(fx, SPARK, 0x01E0_2288, s1, p);
        }
        for k in 0..0x1Eu64 {
            s1 = k;
            spawn(fx, EMBER, 0x01E0_22B0, s1, p);
        }
        s1 = 0x1E;
        spawn(fx, FIRE, 0x01E0_22D4, s1, p);
        spawn(fx, LIGHT, 0x01E0_22E8, s1, p);
    }
    fx.leave(0x50);
}

/// 0x1E040B0(D, step, 0) (frame 0x60: ra +0x50, s4 / s3 the runner's (0)
/// [H], s2 the parameter, s1 the emitter's, s0 = D): b018's spawn on
/// colour D+0x3B44 and parameter D+0x3B9A.
fn spawn(fx: &mut Fx, f: u32, ra: u32, s1: u64, s2: u64) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, s2), (0x10, s1), (0, u64::from(D))]);
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
