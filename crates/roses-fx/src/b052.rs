//! BMODULE b052 (card 430), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02A00, data area 0x1ED0000) [F]: b018's program and globals ([`b018::L56`]: 200 tasks of 0x40
//! bytes at D+0x478, spawn 0x1E03260 = b018's 0x1E03640) with its own
//! emitter and two new steps. Attacker nodes 0x19 / 0x4C / 0xE / 0xF
//! (D+0x3868..), the target's node 1 (cleared); timer 0x95; the histories
//! D+0x3770 (node 0x19) / D+0x36D0 (node 0x4C) filled on update 0x18 and
//! pushed by `spawn::node_history`. The emitter 0x1E01760 counts D+0x387E:
//! every update two streak rings 0x1E01C90 (k twelfths of the way from node
//! 0xE to node 0xF, drawn by the record's 0x1E03730); on 0x18 b018's ribbon
//! 0x1E02160 (stepped but never drawn) and cross ribbon 0x1E026C0
//! (`ribbon_cross`); on 0x1A sixteen b018 ember rings 0x1E02550 at node
//! 0xE; on 0x4C eight more at node 0xF, eight sparks 0x1E023C0 (b018's
//! spark with life rand % 10 + 5, out by 0xF), thirty `tstep::ember`
//! clouds 0x1E02210, b018's fire disc 0x1E02080 and camera light
//! 0x1E02770. Steps 0x1E01A70 / 0x1EE0 / 0x28F0 and the object op 0x29B0
//! are tested but never spawned (not ported, nor the draw 0x1E03380).
//! Returns 1 on update 0x42, 4 on 0x56, 2 on 0x4C, 8 once the timer has
//! run out. No 0x1F8100.
use crate::b018::{self, L56};
use crate::b076::node_position_saving;
use crate::blib::{billboard, quads4, spawn as bspawn, spline, tstep, uv};
use crate::bus::Bus;
use crate::c061::{desc_uv4, rgbaq_at};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;

const STREAK: u32 = 0x01E0_1C90;
const FIRE: u32 = 0x01E0_2080;
const RIBBON: u32 = 0x01E0_2160;
const EMBER: u32 = 0x01E0_2210;
const SPARK: u32 = 0x01E0_23C0;
const RING: u32 = 0x01E0_2550;
const CROSS: u32 = 0x01E0_26C0;
const LIGHT: u32 = 0x01E0_2770;

const POOL: u32 = D + 0x478;
const POS: u32 = D + 0x2D0;
const HIST_A: u32 = D + 0x36D0;
const HIST_B: u32 = D + 0x3770;
const KEEP: u32 = D + 0x36C0;
const TIMER: u32 = D + 0x3820;
const COLOUR: u32 = D + 0x3824;
const OT2: u32 = D + 0x382C;
const NODES: u32 = D + 0x3868;
const PARAM: u32 = D + 0x387A;
const COUNTER: u32 = D + 0x387E;
const BYTES: u32 = D + 0x3882;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E02A00 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x170);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x170);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3AF0);
    l2::sine_table(fx, D + 0x3984);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x387C, param);
    fx.mem.set32(TIMER, 0x95);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, (n, clear)) in [(0x19, 1), (0x4C, 0), (0xE, 0), (0xF, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x3876, r);
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
        let v = fx.rand_from(D, 0x01E0_2B3C);
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

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let l = &L56;
    emit(fx, param);
    if counter(fx) == 0x18 {
        fill(fx, NODES, sp + 0xB0, HIST_B);
    }
    if counter(fx) == 0x18 {
        fill(fx, NODES + 2, sp + 0xC0, HIST_A);
    }
    let n = fx.mem.i16(NODES);
    bspawn::node_history(fx, HIST_B + 0x90, 0xA, n);
    let n = fx.mem.i16(NODES + 2);
    bspawn::node_history(fx, HIST_A + 0x90, 0xA, n);
    for k in 0..200 {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            STREAK => streak(fx, t),
            FIRE => b018::run_step(fx, t, 0x20, 0x01E0_2CE0, |fx, t| b018::fire(fx, FIRE, t)),
            RIBBON => b018::ribbon_step(fx, t),
            EMBER => b018::run_step(fx, t, 0x20, 0x01E0_2DA0, |fx, t| {
                tstep::ember(fx, EMBER, t);
            }),
            SPARK => b018::run_step(fx, t, 0x20, 0x01E0_2DC0, spark),
            RING => b018::run_step(fx, t, 0x20, 0x01E0_2DE0, |fx, t| b018::ring(fx, RING, t)),
            CROSS => b018::cross_step(fx, t),
            LIGHT => b018::run_step(fx, t, 0x30, 0x01E0_2EA8, |fx, t| b018::light(fx, l, t)),
            _ => {}
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    let d = sp + 0xD8;
    let (tex, tex_d) = (D + 0x340, D + 0x374);
    for k in 0..200 {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            STREAK => {
                b018::cell(fx, t, tex_d + 8, sp + 0x60, 0x9B);
                streaks(fx, t, d, BYTES);
            }
            FIRE => {
                b018::cell(fx, t, tex_d + 8, sp + 0x70, 0x9B);
                quads4::fan18(fx, D, t, d);
            }
            EMBER => {
                b018::cell(fx, t, tex_d + 8, sp + 0x80, 0x9B);
                billboard::ring_t(fx, uv::uv_rect4, D, t, d, BYTES);
            }
            SPARK => {
                b018::cell(fx, t, tex + 4, sp + 0x90, 0xA);
                billboard::ring_t20(fx, uv::uv_rect4, D, t, d, BYTES);
            }
            RING => {
                b018::cell(fx, t, tex_d + 8, sp + 0xA0, 0x9B);
                billboard::ring_t20(fx, uv::uv_rect4, D, t, d, BYTES);
            }
            CROSS => spline::ribbon_cross(fx, t, HIST_A),
            _ => {}
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x42 => 1,
        0x56 => 4,
        0x4C => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E023C0: `tstep::spark` with life rand % 10 + 5 and out by 0xF: size
/// 2, frame rand & 3, colour (0x80, 0x40, 0), +0x18 / +0x10 two rand %
/// 0xFFFF, +0x30 = 0, angle rand % 360, +0xE = (rand & 0xF) + 3, +8 =
/// rand % 10; alpha 0x80 for the life, then down by 0xF (ends below
/// 0x11); +0x30 += +0xE every update.
fn spark(fx: &mut Fx, t: u32) {
    let at = SPARK;
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 2);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v & 3);
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        for (o, ra) in [(0x18u32, 0x5Cu32), (0x10, 0x78)] {
            let v = fx.rand_from(t, at + ra);
            let r = fx.div(v, 0xFFFF).1;
            fx.mem.set32(t + o, r);
        }
        fx.mem.set16(t + 0x30, 0);
        let v = fx.rand_from(t, at + 0x98);
        let r = fx.div(v, 360).1;
        fx.mem.set16(t + 0x3A, r);
        let v = fx.rand_from(t, at + 0xB4);
        fx.mem.set16(t + 0xE, (v & 0xF) + 3);
        let v = fx.rand_from(t, at + 0xD4);
        let r = fx.div(v, 10).1;
        fx.mem.set16(t + 8, r);
        let v = fx.rand_from(t, at + 0xF0);
        let r = fx.div(v, 10).1;
        fx.mem.set16(t + 6, r + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a - 0xF);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    let v = fx.mem.i16(t + 0x30).wrapping_add(fx.mem.i16(t + 0xE));
    fx.mem.set16(t + 0x30, v);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E01C90 (D, task) (frame 0x70: ra +0x40, the entry's s2 / s1 / s0
/// +0x30 / +0x20 / +0x10, f21 / f20 the entry's (0) [H]): a streak ring
/// (size 6, frame 2, k = rand % 12 at +0x3A, (0x80, 0x40, 0), +0x18 rand
/// % 0xFFFF, growth 7 - k / 2, row rand % 10, life rand % 3 + 1); 0x80 for
/// its life, then out by 0xA; the radius +0x30 up by the growth; the point
/// k tenths of the way from node 0xE to node 0xF.
fn streak(fx: &mut Fx, t: u32) {
    use ee::{add, div, from_i32, mul, sub};
    let sp = fx.enter(0x70);
    let s2 = u64::from(t - 0x478);
    fx.saved(sp, &[(0x40, 0x01E0_2CA0), (0x30, s2), (0x20, 0), (0x10, u64::from(D))]);
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        let v = fx.rand_from(t, 0x01E0_1CCC);
        let r = fx.div(v, 0xC).1;
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 2);
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        let v = fx.rand_from(t, 0x01E0_1D00);
        let r = fx.div(v, 0xFFFF).1;
        fx.mem.set32(t + 0x18, r);
        fx.mem.set16(t + 0x30, 0);
        let k = fx.mem.i16(t + 0x3A) >> 1;
        fx.mem.set16(t + 0xE, 7 - k);
        let v = fx.rand_from(t, 0x01E0_1D34);
        let r = fx.div(v, 0xA).1;
        fx.mem.set16(t + 8, r);
        let v = fx.rand_from(t, 0x01E0_1D50);
        let r = fx.div(v, 3).1;
        fx.mem.set16(t + 6, r + 1);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a - 0xA);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    let r = fx.mem.i16(t + 0x30).wrapping_add(fx.mem.i16(t + 0xE));
    fx.mem.set16(t + 0x30, r);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES + 4);
    node_position_saving(fx, h, s, sp + 0x50, u64::from(t), Some(u64::from(D)), 0x01E0_1DE4);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES + 6);
    node_position_saving(fx, h, s, sp + 0x60, u64::from(t), Some(u64::from(sp + 0x60)), 0x01E0_1E00);
    let n1 = [fx.mem.f(sp + 0x50), fx.mem.f(sp + 0x54), fx.mem.f(sp + 0x58)];
    let n2 = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
    let step: [f32; 3] = std::array::from_fn(|i| div(sub(n2[i], n1[i]), 10.0));
    for (i, ra) in [0x01E0_1E70u32, 0x01E0_1E94, 0x01E0_1EB8].into_iter().enumerate() {
        let k = from_i32(fx.mem.i16(t + 0x3A));
        let v = add(n1[i], mul(step[i], k));
        fx.fix_residue(v, ra);
        fx.mem.set16(t + 2 * i as u32, ee::fixsfsi(v));
    }
    fx.leave(0x70);
}

/// 0x1E03730(D, task, desc, table) (frame 0x150): `billboard::ring_t20`
/// with the bytes `table` read at +0 / +0x32 / +0x64 and each quad's Z set
/// far (0xFFFFFF) before it is keyed by its mean w into D+0x80.
fn streaks(fx: &mut Fx, t: u32, d: u32, table: u32) {
    let sp = fx.enter(0x150);
    let l = |o: u32| sp + o - 0x10;
    desc_uv4(fx, d);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, l(0x158), t + 0x28, d + 0x10);
    uv::uv_rect4(fx, d, D + 0x94, 0);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(l(0xE0) + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, l(0x140));
    for k in 0..3 {
        let v = fx.mem.read_u32(l(0x140) + 4 * k).wrapping_add(fx.mem.read_u32(t + 0x10 + 4 * k));
        fx.mem.write_u32(l(0x140) + 4 * k, v);
    }
    for k in 0..3 {
        let p = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(l(0xD0) + 4 * k, p);
    }
    l2::xform_vp(fx, l(0x100), l(0xE0), l(0x140), l(0xD0));
    let q = l(0x90);
    let mut k = 0i32;
    while k < 0x14 {
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, q, s, s);
        for (axis, off) in [(0, 0u32), (1, 0x32), (2, 0x64)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::from_i32(fx.mem.i8(table.wrapping_add(k as u32).wrapping_add(i as u32).wrapping_add(off)));
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + (k >> 1));
            l2::add_column(fx, q, axis, ee::mul(ee::div(b, 20.0), r));
        }
        for (o, v) in [(4u32, 0u32), (0x14, 0), (0, 0x3F80_0000), (0x10, 0xBF80_0000), (8, 0), (0x18, 0)] {
            fx.mem.write_u32(q + o, v);
        }
        if l2::project(fx, q, SPR, l(0x100), d, l(0xF0)) {
            l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
            let key = ee::fixunssfsi(fx.mem.f(l(0xF0))) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x150);
}

/// Node slot `k`'s position into the emitter's sp + 0x40 and the spawn
/// point; the parameter 1.
fn at_node(fx: &mut Fx, sp: u32, k: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES + 2 * k);
    fx.node_position(h, n, sp + 0x40);
}

fn to_pos(fx: &mut Fx, sp: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
        fx.mem.write_u32(POS + 4 * k, w);
    }
    fx.mem.set16(PARAM, 1);
}

/// 0x1E01760(D, 0) (frame 0x60: ra +0x30, s2 +0x20 = the parameter, s1
/// +0x10 = 0, s0 = D): the spawn colour (0x80, 0, 0, 0), the update
/// counter and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let l = &L56;
    let sp = fx.enter(0x60);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_2B78), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    if counter(fx) == 0x33 {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 2);
        fx.node_position(h, n, KEEP);
    }
    let c = counter(fx);
    if (0x34..0x41).contains(&c) {
        at_node(fx, sp, 1);
        for k in 0..3 {
            let w = fx.mem.read_u32(KEEP + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    }
    let (mut s1, mut s2) = (0u64, p);
    if counter(fx) == 0x18 {
        b018::spawn(fx, l, RIBBON, 0x01E0_183C, s1, s2);
    }
    if counter(fx) == 0x18 {
        b018::spawn(fx, l, CROSS, 0x01E0_1860, s1, s2);
    }
    b018::spawn(fx, l, STREAK, 0x01E0_1874, s1, s2);
    b018::spawn(fx, l, STREAK, 0x01E0_1888, s1, s2);
    if counter(fx) == 0x1A {
        at_node(fx, sp, 2);
        to_pos(fx, sp);
        s1 = u64::from(sp + 0x44);
        for k in 0..16u64 {
            s2 = k;
            b018::spawn(fx, l, RING, 0x01E0_18EC, s1, s2);
        }
        s2 = 16;
        let y = fx.mem.f(sp + 0x44);
        fx.mem.setf(POS + 4, ee::add(1.0, y));
    }
    if counter(fx) == 0x4C {
        at_node(fx, sp, 3);
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x50);
        to_pos(fx, sp);
        for k in 0..8u64 {
            s1 = k;
            b018::spawn(fx, l, RING, 0x01E0_1988, s1, s2);
        }
    }
    if counter(fx) == 0x4C {
        at_node(fx, sp, 3);
        to_pos(fx, sp);
        for k in 0..8u64 {
            s1 = k;
            b018::spawn(fx, l, SPARK, 0x01E0_19F8, s1, s2);
        }
        for k in 0..0x1Eu64 {
            s1 = k;
            b018::spawn(fx, l, EMBER, 0x01E0_1A20, s1, s2);
        }
        s1 = 0x1E;
        b018::spawn(fx, l, FIRE, 0x01E0_1A44, s1, s2);
        b018::spawn(fx, l, LIGHT, 0x01E0_1A58, s1, s2);
    }
    fx.leave(0x60);
}
