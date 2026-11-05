//! BMODULE b218 (card 216), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03BC0, data area 0x1ED0000, 0x3DE0 bytes) [F]
//!: the tstep family (cards 56 / 225) on a pool
//! of 150 tasks of 0x40 bytes at D+0x380 (alive +0x36, step +0x24),
//! spawned by 0x1E04890 (`tstep::spawn`'s body on colour D+0x3B04, point
//! D+0x2D0 and parameter D+0x3B62). Three 10-point histories of the
//! attacker's nodes 0x2B / 0x2C / 0x40 at D+0x2970 / D+0x2A10 / D+0x2AB0
//! (the first two filled on update 0x49, the second twice: the original
//! fills D+0x2A10 again for node 0x40), pushed by `spawn::node_history`
//! every update.
//!
//! The emitter 0x1E02160 counts D+0x3B68: on 0x49 three `ribbon_g` /
//! `ribbon_tex` ribbons (0x1E02AB0 / 0x2B60 / 0x2C10, stepped inline by
//! the entry) and a `ribbon_tex1` blue glow (0x1E02CC0, `tstep::blue_glow`);
//! on 0x50 the drifting blue ribbon 0x1E027F0 (its two histories D+0x2C90
//! / D+0x2D30 from D+0x2A10, swayed +-8 a update by the attacker's facing,
//! drawn three times by the record's 0x1E049B0 = `ribbon_tex` through the
//! task's TRS), two trailing streaks 0x1E030E0 (histories D+0x37D0 /
//! D+0x3870, drawn by the record's 0x1E05710 = a textured `ribbon_cross`)
//! and forty `sprite_drift` motes 0x1E033F0 at nodes 0x1A / 0x6A; on
//! 0x50..0x6F a camera-facing ribbon 0x1E02D90 a update (a copy of D+0x2D30
//! into one of 14 histories D+0x2E70 + 0xA0 k, drawn by the record's
//! 0x1E05030); on 0x67 twenty `band9_tex` sliver rings 0x1E02F10, twenty
//! motes 0x1E03560 and a `fan18_far0` flash 0x1E03700; below 0x32 two
//! motes 0x1E032F0 a update at nodes 0x1A / 0x6A; on 1 a task 0x1E03BB0
//! that no step or draw names (it holds its slot). The spawn point also
//! follows the attacker's position D+0x320 / D+0x330 (pushed in z by the
//! facing) on 0x70..0xE0. Returns 1 on update 0x5C, 4 on 0x70, 2 on 0x66,
//! 8 once the 120-update timer D+0x3B00 has run out. No 0x1F8100.
//!
//! The steps 0x1E037E0 / 0x1E03AA0 and the object op 0x1E03B60 are tested
//! by the entry but never spawned [F: the emitter's spawns]; not ported.
use crate::blib::{billboard, quads4, spawn as bspawn, spline, tstep, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
const POOL: u32 = D + 0x380;
const POS: u32 = D + 0x2D0;
const TIMER: u32 = D + 0x3B00;
const COLOUR: u32 = D + 0x3B04;
const NODES: u32 = D + 0x3B48;
const PARAM: u32 = D + 0x3B62;
const NEXT: u32 = D + 0x3B64;
const COUNTER: u32 = D + 0x3B68;
const BYTES: u32 = D + 0x3B6C;
const SINES: u32 = D + 0x3C6C;
const HIST_A: u32 = D + 0x2970;
const HIST_B: u32 = D + 0x2A10;
const HIST_C: u32 = D + 0x2AB0;
const DRIFT_A: u32 = D + 0x2C90;
const DRIFT_B: u32 = D + 0x2D30;
const TRAILS: u32 = D + 0x2E70;
const TEX_F: u32 = D + 0x37C;
const TEX_E: u32 = D + 0x378;
const TEX5: u32 = D + 0x2900;

const DRIFT: u32 = 0x01E0_27F0;
const GREEN: u32 = 0x01E0_2AB0;
const GREY: u32 = 0x01E0_2B60;
const DARK: u32 = 0x01E0_2C10;
const GLOW: u32 = 0x01E0_2CC0;
const CAMRIB: u32 = 0x01E0_2D90;
const SLIVER: u32 = 0x01E0_2F10;
const STREAK: u32 = 0x01E0_30E0;
const MOTE_S: u32 = 0x01E0_32F0;
const MOTE_N: u32 = 0x01E0_33F0;
const MOTE_F: u32 = 0x01E0_3560;
const FLASH: u32 = 0x01E0_3700;
const IDLE: u32 = 0x01E0_3BB0;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E03BC0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1E0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x3B0C, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x1E0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3DE0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3B66, param);
    fx.mem.set32(TIMER, 0x78);
    for o in [0u32, 4, 8] {
        fx.mem.write_u32(D + 0x310 + o, 0);
    }
    for (k, (n, clear)) in [(0x2B, 1), (0x2C, 0), (0x1A, 0), (0x6A, 0), (0x40, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x368 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x348, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX5 + 4 * k as u32, t);
    }
    // rand's frame: s0 = D.
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_3D10);
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
    emit(fx, param);
    if counter(fx) == 0x49 {
        fill(fx, NODES, sp + 0x110, HIST_A);
    }
    if counter(fx) == 0x49 {
        fill(fx, NODES + 2, sp + 0x120, HIST_B);
    }
    if counter(fx) == 0x49 {
        fill(fx, NODES + 8, sp + 0x130, HIST_B);
    }
    // node_history's saves: ra, s4 / s3 the runner's (0) [H], s2 the
    // parameter (D+0x2A10 after the fills), s1 = 0, s0 = D.
    let s2 = if counter(fx) == 0x49 { u64::from(HIST_B + 0x90) } else { param as i64 as u64 };
    for (hist, node, ra) in [(HIST_A, NODES, 0x01E0_3E60u32), (HIST_B, NODES + 2, 0x01E0_3E70), (HIST_C, NODES + 8, 0x01E0_3E80)] {
        let f = fx.sp.wrapping_sub(0x70);
        fx.saved(f, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, s2), (0x10, 0), (0, u64::from(D))]);
        let n = fx.mem.i16(node);
        bspawn::node_history(fx, hist + 0x90, 0xA, n);
    }
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            DRIFT => frame(fx, t, 0x40, 0x01E0_3EB4, drift),
            GREEN => ribbon_inline(fx, t, (0x10, 0x60, 0x10), 0x60),
            GREY => ribbon_inline(fx, t, (0x60, 0x60, 0x60), 0x80),
            DARK => ribbon_inline(fx, t, (0x20, 0x20, 0x20), 0x80),
            GLOW => {
                let f = fx.sp.wrapping_sub(0x20);
                fx.saved(f, &[(0x10, 0x01E0_40D0), (0, u64::from(D))]);
                tstep::blue_glow(fx, t);
            }
            CAMRIB => frame(fx, t, 0x30, 0x01E0_40F0, camrib),
            SLIVER => frame(fx, t, 0x30, 0x01E0_4110, sliver),
            STREAK => frame(fx, t, 0x40, 0x01E0_4130, streak),
            MOTE_S => frame(fx, t, 0x20, 0x01E0_4150, mote_s),
            MOTE_N => frame(fx, t, 0x30, 0x01E0_4170, mote_n),
            MOTE_F => frame(fx, t, 0x30, 0x01E0_4190, mote_f),
            FLASH => frame(fx, t, 0x20, 0x01E0_41B0, flash),
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
        0x5C => 1,
        0x70 => 4,
        0x66 => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// A step's frame and its saves of the entry's s0 = D (and s1 = 0 in a
/// 0x30 / 0x40 frame).
fn frame(fx: &mut Fx, t: u32, size: u32, ra: u32, f: fn(&mut Fx, u32, u32)) {
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

/// Alpha +0x2F down by 7 while at least 9, else 0.
fn fall_2f(fx: &mut Fx, t: u32) {
    let a = fx.mem.read_u8(t + 0x2F);
    fx.mem.write_u8(t + 0x2F, if a >= 9 { a - 7 } else { 0 });
}

fn rmod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// The entry's inline steps for 0x1E02AB0 / 0x2B60 / 0x2C10: on the first
/// update colour `c`, +6 = 9, alpha 0x80, width 6; alpha `young` for 0x78
/// updates, then out by 9.
fn ribbon_inline(fx: &mut Fx, t: u32, c: (i32, i32, i32), young: u8) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, c.0, c.1, c.2);
        fx.mem.set16(t + 6, 9);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.set16(t + 0x38, 6);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0x78 {
        fx.mem.write_u8(t + 0x2B, young);
    } else {
        fall(fx, t, 0xB, 9);
    }
    grow(fx, t);
}

/// The attacker's facing (0x1F7370 into `local`, its y word / 180 through
/// 0x1A2B80 returning to `ra`).
fn facing(fx: &mut Fx, local: u32, ra: u32) -> i32 {
    let h = fx.self_handle();
    fx.unit_rotation(h, local);
    let f = ee::div(ee::from_i32(fx.mem.i32(local + 4)), 180.0);
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}

/// 0x1E027F0: the drifting blue ribbon ((0x60, 0x60, 0xC0) both colours,
/// +6 = 8, alphas 0x80 / 0x40, width 6, scale 10 at the origin): its two
/// histories D+0x2C90 (z less 10) / D+0x2D30 copied from D+0x2A10 on the
/// first update; alpha 0x80 for 0x1B updates, then out by 0xF (+0x2F by
/// 7); from update 1 both histories' z move by 8 a update, - when the
/// facing / 180 is not 0.
fn drift(fx: &mut Fx, t: u32, sp: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0x60, 0x60, 0xC0);
        l2::rgb(fx, t + 0x2C, 0x60, 0x60, 0xC0);
        fx.mem.set16(t + 6, 8);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.write_u8(t + 0x2F, 0x40);
        for i in 0..10 {
            for k in 0..4 {
                let w = fx.mem.read_u32(HIST_B + 0x10 * i + 4 * k);
                fx.mem.write_u32(DRIFT_A + 0x10 * i + 4 * k, w);
                fx.mem.write_u32(DRIFT_B + 0x10 * i + 4 * k, w);
            }
        }
        for i in 0..10 {
            let z = fx.mem.f(DRIFT_A + 0x10 * i + 8);
            fx.mem.setf(DRIFT_A + 0x10 * i + 8, ee::sub(z, 10.0));
        }
        for o in [2u32, 0, 4] {
            fx.mem.set16(t + o, 0);
        }
        for o in [0xAu32, 8, 0xC] {
            fx.mem.set16(t + o, 0xA);
        }
        fx.mem.set16(t + 0x38, 6);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0x1B {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x11, 0xF);
        fall_2f(fx, t);
    }
    let v = facing(fx, sp + 0x30, 0x01E0_29FC) as i16;
    if age(fx, t) != 0 {
        for i in 0..10 {
            for h in [DRIFT_A, DRIFT_B] {
                let p = h + 0x10 * i + 8;
                let z = fx.mem.f(p);
                fx.mem.setf(p, if v != 0 { ee::sub(z, 8.0) } else { ee::add(z, 8.0) });
            }
        }
    }
    grow(fx, t);
}

/// 0x1E02D90: a camera-facing ribbon ((0x30, 0x30, 0x80) both colours, +6
/// = 8, alphas 0x80 / 0x40, scale 10 at the origin, width 6): D+0x2D30's
/// ten points copied into its history D+0x2E70 + 0xA0 +0x30 on the first
/// update; alpha 0x80 on update 0, then out by 0xF (+0x2F by 7).
fn camrib(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0x30, 0x30, 0x80);
        l2::rgb(fx, t + 0x2C, 0x30, 0x30, 0x80);
        fx.mem.set16(t + 6, 8);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.write_u8(t + 0x2F, 0x40);
        for o in [2u32, 0, 4] {
            fx.mem.set16(t + o, 0);
        }
        for o in [0xAu32, 8, 0xC] {
            fx.mem.set16(t + o, 0xA);
        }
        let h = trail(fx, t);
        for i in 0..10 {
            for k in 0..4 {
                let w = fx.mem.read_u32(DRIFT_B + 0x10 * i + 4 * k);
                fx.mem.write_u32(h + 0x10 * i + 4 * k, w);
            }
        }
        fx.mem.set16(t + 0x38, 6);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) <= 0 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x11, 0xF);
        fall_2f(fx, t);
    }
    grow(fx, t);
}

/// The task's history D+0x2E70 + 0xA0 x i16 +0x30.
fn trail(fx: &mut Fx, t: u32) -> u32 {
    let p = fx.mem.i16(t + 0x30);
    TRAILS.wrapping_add((0xA0 * p) as u32)
}

/// 0x1E02F10: a sliver ring (size 0 growing by +8 = rand % 3 + 4 from
/// update 1, +0x10 rand & 0x3FFF - 0x2000, +0x14 rand % 0xFFFF, frame rand
/// % 7, +6 = 0x14; (0xC0, 0x80, 0) / (0, 0xA0, 0xA0) by +0x30, rim (0xA0,
/// 8, 8)): alpha up by 0x40 below 0x80 for 2 updates, then down by 0x13
/// once past size 0x28 and by 0xF once the timer is under 10.
fn sliver(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        let v = fx.rand_from(t, 0x01E0_2F3C);
        fx.mem.set32(t + 0x10, (v & 0x3FFF) - 0x2000);
        let r = rmod(fx, t, 0x01E0_2F5C, 0xFFFF);
        fx.mem.set32(t + 0x14, r);
        let r = rmod(fx, t, 0x01E0_2F78, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
        let r = rmod(fx, t, 0x01E0_2F9C, 3);
        fx.mem.set16(t + 8, r + 4);
        fx.mem.set16(t + 6, 0x14);
        if fx.mem.i16(t + 0x30) == 0 {
            l2::rgb(fx, t + 0x28, 0xC0, 0x80, 0);
        } else {
            l2::rgb(fx, t + 0x28, 0, 0xA0, 0xA0);
        }
        l2::rgb(fx, t + 0x2C, 0xA0, 8, 8);
    }
    if age(fx, t) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        }
    } else {
        if fx.mem.i16(t + 0x38) >= 0x29 {
            fall(fx, t, 0x15, 0x13);
        }
        if fx.mem.i32(TIMER) < 0xA {
            fall(fx, t, 0x11, 0xF);
        }
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x38) + fx.mem.i16(t + 8);
        fx.mem.set16(t + 0x38, v);
    }
    grow(fx, t);
}

/// 0x1E030E0: a trailing streak ((0x30, 0x30, 0x80), +6 = 9, +0xA rand %
/// 0x11 + 0x2C, +8 (rand & 7) + 5, width 2): its history D+0x2E70 + 0xA0
/// +0x30 filled with, and every update pushed (`spawn::history`) by,
/// D+0x2C90 (+0x30 = 0xF) or D+0x2D00; alpha up by 0x1B below 0xC0 for
/// 0x1C updates, then out by 9.
fn streak(fx: &mut Fx, t: u32, sp: u32) {
    let point = |fx: &mut Fx, t: u32| if fx.mem.i16(t + 0x30) == 0xF { DRIFT_A } else { D + 0x2D00 };
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0x30, 0x30, 0x80);
        fx.mem.set16(t + 6, 9);
        fx.mem.write_u8(t + 0x2B, 0);
        let r = rmod(fx, t, 0x01E0_3128, 0x11);
        fx.mem.set16(t + 0xA, r + 0x2C);
        let v = fx.rand_from(t, 0x01E0_3148);
        fx.mem.set16(t + 8, (v & 7) + 5);
        let p = point(fx, t);
        for k in 0..4 {
            let w = fx.mem.read_u32(p + 4 * k);
            fx.mem.write_u32(sp + 0x30 + 4 * k, w);
        }
        let h = trail(fx, t);
        for i in 0..10 {
            for k in 0..3 {
                let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
                fx.mem.write_u32(h + 0x10 * i + 4 * k, w);
            }
        }
        fx.mem.set16(t + 0x38, 2);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0x1C {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x1B));
        }
    } else {
        fall(fx, t, 0xB, 9);
    }
    grow(fx, t);
    let p = point(fx, t);
    for k in 0..4 {
        let w = fx.mem.read_u32(p + 4 * k);
        fx.mem.write_u32(sp + 0x30 + 4 * k, w);
    }
    let h = trail(fx, t);
    bspawn::history(fx, h + 0x90, 0xA, sp + 0x30);
}

/// 0x1E032F0: a small mote (size 0 growing by 2 to 0x28, frame rand % 7,
/// grey 0x80, +0x30 / +0x3A rand % 360): alpha 0x80 for 0x13 updates, then
/// out by 0xB.
fn mote_s(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        let r = rmod(fx, t, 0x01E0_3314, 7);
        fx.mem.set16(t + 0x34, r);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let r = rmod(fx, t, 0x01E0_3340, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, t, 0x01E0_335C, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0x13 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0xD, 0xB);
    }
    let s = fx.mem.i16(t + 0x38);
    if s < 0x28 {
        fx.mem.set16(t + 0x38, s + 2);
    }
    grow(fx, t);
}

/// The motes' first update: size 0x3C, frame rand % 7, grey 0x80, +0x30 /
/// +0x3A rand % 360 (rands returning to `ra`).
fn mote_start(fx: &mut Fx, t: u32, ra: [u32; 3]) {
    fx.mem.set16(t + 0x38, 0x3C);
    let r = rmod(fx, t, ra[0], 7);
    fx.mem.set16(t + 0x34, r);
    l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
    let r = rmod(fx, t, ra[1], 0x168);
    fx.mem.set16(t + 0x30, r);
    let r = rmod(fx, t, ra[2], 0x168);
    fx.mem.set16(t + 0x3A, r);
    fx.mem.set16(t + 0x36, 2);
}

/// i16 `o` += `by` x the sine (cos 0) / cosine of the i16 angle `a` (in
/// degrees, the table D+0x3C6C), through 0x1A2B80 returning to `ra`.
fn push(fx: &mut Fx, t: u32, o: u32, a: u32, cos: i32, by: f32, ra: u32) {
    let deg = fx.mem.i16(t + a);
    let s = l2::sin_deg(fx, SINES, deg, cos);
    let m = ee::mul(by, s);
    let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), m);
    fx.fix_residue(v, ra);
    fx.mem.set16(t + o, ee::fixsfsi(v));
}

/// 0x1E033F0: a mote at the nodes 0x1A / 0x6A drifting 24 a update along
/// +0x3A in x / z: alpha 0x80 for 7 updates, then out by 0x14.
fn mote_n(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        mote_start(fx, t, [0x01E0_3420, 0x01E0_344C, 0x01E0_3468]);
    }
    if age(fx, t) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x16, 0x14);
    }
    push(fx, t, 8, 0x3A, 0, 24.0, 0x01E0_34F8);
    push(fx, t, 0xC, 0x3A, 1, 24.0, 0x01E0_3530);
    grow(fx, t);
}

/// 0x1E03560: a mote drifting 14 a update along +0x3A in x / z and +0x30
/// in y: alpha 0x80 for 0xC updates, then out by 0x14.
fn mote_f(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        mote_start(fx, t, [0x01E0_3590, 0x01E0_35BC, 0x01E0_35D8]);
    }
    if age(fx, t) < 0xC {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x16, 0x14);
    }
    push(fx, t, 8, 0x3A, 0, 14.0, 0x01E0_3668);
    push(fx, t, 0xA, 0x30, 1, 14.0, 0x01E0_36A0);
    push(fx, t, 0xC, 0x3A, 1, 14.0, 0x01E0_36D8);
    grow(fx, t);
}

/// 0x1E03700: the white flash disc (size 400, (0xF0, 0xF0, 0xF0) centre,
/// (0x10, 0x10, 0xF0) rim alpha 0, a random angle): alpha up by 0x60 for
/// 2 updates, then out by 0xF.
fn flash(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x190);
        l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0xF0);
        l2::rgb(fx, t + 0x2C, 0x10, 0x10, 0xF0);
        let r = rmod(fx, t, 0x01E0_3754, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x60));
    } else {
        fall(fx, t, 0x11, 0xF);
    }
    grow(fx, t);
}

/// The texture at `tex` and the atlas cell `id` frame +0x34 (0x106960
/// into `r`) copied to D+0x88 / D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, tex: Option<u32>, r: u32, id: i32) {
    if let Some(tex) = tex {
        let v = fx.mem.read_u32(tex);
        fx.mem.write_u32(D + 0x88, v);
    }
    let f = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, f);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// The entry's draw case for a live task (`sp` the entry's frame, the
/// descriptor at sp + 0x148).
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let d = sp + 0x148;
    match fx.mem.read_u32(t + 0x24) {
        DRIFT => {
            cell(fx, t, Some(TEX_F), sp + 0x60, 0x97);
            fx.mem.set16(D + 0xBE, 0);
            for _ in 0..3 {
                ribbon_trs(fx, t, DRIFT_A, DRIFT_B);
            }
        }
        GREEN => spline::ribbon_g(fx, t, HIST_A, HIST_B),
        GREY => {
            cell(fx, t, Some(TEX_E), sp + 0x70, 0x8C);
            fx.mem.set16(D + 0x96, 0xF);
            for o in [0x98u32, 0x9A] {
                let v = fx.mem.i16(D + o) >> 1;
                fx.mem.set16(D + o, v);
            }
            spline::ribbon_tex(fx, D, t, HIST_A, HIST_B);
            cell(fx, t, Some(TEX_F), sp + 0x80, 0x96);
            spline::ribbon_tex(fx, D, t, HIST_A, HIST_B);
        }
        DARK => spline::ribbon_g(fx, t, HIST_A, HIST_C),
        GLOW => {
            cell(fx, t, Some(TEX_F), sp + 0x90, 0x96);
            spline::ribbon_tex1(fx, D, t, HIST_A, HIST_B);
            cell(fx, t, None, sp + 0xA0, 0x97);
            spline::ribbon_tex1(fx, D, t, HIST_A, HIST_B);
            spline::ribbon_tex1(fx, D, t, HIST_A, HIST_B);
        }
        CAMRIB => {
            cell(fx, t, Some(TEX_F), sp + 0xB0, 0x9B);
            let h = trail(fx, t);
            ribbon_cam(fx, t, h);
        }
        SLIVER => {
            cell(fx, t, Some(TEX_F), sp + 0xC0, 0x9B);
            fx.mem.set16(D + 0xB8, 1);
            quads4::band9_tex(fx, D, t, d);
            fx.mem.set16(D + 0xB8, 0);
        }
        FLASH => quads4::fan18_far0(fx, D, t, d),
        STREAK => {
            let v = fx.mem.read_u32(TEX_F);
            fx.mem.write_u32(D + 0x88, v);
            for (o, v) in [(0x94u32, 1), (0x96, 0x10), (0x98, 0x1F), (0x9A, 1), (0xBE, 1)] {
                fx.mem.set16(D + o, v);
            }
            cell(fx, t, None, sp + 0xD0, 0x97);
            let h = trail(fx, t);
            ribbon_cross_tex(fx, t, h);
        }
        MOTE_S | MOTE_N | MOTE_F => {
            let r = match fx.mem.read_u32(t + 0x24) {
                MOTE_S => sp + 0xE0,
                MOTE_N => sp + 0xF0,
                _ => sp + 0x100,
            };
            cell(fx, t, Some(TEX5), r, 0x14);
            let blend = if fx.mem.read_u32(t + 0x24) == MOTE_F { 0 } else { 1 };
            billboard::sprite_drift(fx, 0x01E0_1F40, uv::uv_rect4, D, t, blend);
        }
        _ => {}
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 0x108020(local, c) and the 8 bytes it builds copied to `to`.
fn rgbaq_to(fx: &mut Fx, local: u32, c: u32, to: u32) {
    l2::rgbaq(fx, local, c);
    for k in 0..2 {
        let w = fx.mem.read_u32(local + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The task's TRS words: the i16 point +0 / +2 / +4 at `trans`, the i16
/// scale +8 / +0xA / +0xC over 10 at `scale`.
fn trs_words(fx: &mut Fx, t: u32, trans: u32, scale: u32) {
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(trans + 4 * k, v);
    }
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 8 + 2 * k));
        fx.mem.setf(scale + 4 * k, v);
    }
    for k in 0..3u32 {
        let v = fx.mem.f(scale + 4 * k);
        fx.mem.setf(scale + 4 * k, ee::div(v, 10.0));
    }
}

/// 0x1E049B0(D, task, a, b): `spline::ribbon_tex` (its frame 0x60 larger)
/// projected through the task's TRS (0x1097D0 into sp + 0xF0: scale sp +
/// 0xE0, point sp + 0xD0) instead of the camera [F fdiff].
fn ribbon_trs(fx: &mut Fx, t: u32, a: u32, b: u32) {
    let sp = fx.enter(0x390);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x2B8;
    l2::desc_strip3(fx, d);
    for (local, c, to) in [(0x348u32, 0x28u32, 0x10u32), (0x350, 0x2C, 0x28), (0x358, 0x28, 0x40), (0x360, 0x2C, 0x58)] {
        rgbaq_to(fx, sp + local, t + c, d + to);
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let rh2 = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, rh2, 1);
    let (ca, cb) = (sp + 0x388, sp + 0x38C);
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(ca + k, v);
        let v = fx.mem.read_u8(t + 0x2C + k);
        fx.mem.write_u8(cb + k, v);
    }
    let (pa, pb) = (sp + 0x1F0, sp + 0x230);
    for (k, o) in [0u32, 0x10, 0x20, 0x30].into_iter().enumerate() {
        copy4(fx, pa + 0x10 * k as u32, a + o);
    }
    for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
        copy4(fx, pb + 0x10 * k as u32, b + o);
    }
    trs_words(fx, t, sp + 0xD0, sp + 0xE0);
    l2::xform_vp(fx, sp + 0xF0, sp + 0xE0, 0, sp + 0xD0);
    let div = n - 1;
    fx.mem.set32(sp + 0xB0, n - 2);
    let mut s0 = 0i32;
    while s0 < fx.mem.i32(sp + 0xB0) {
        rgbaq_to(fx, sp + 0x368, ca, d + 0x10);
        rgbaq_to(fx, sp + 0x370, cb, d + 0x28);
        let al = i32::from(fx.mem.read_u8(t + 0x2B));
        let q = fx.div(al, div).0;
        let v = fx.mem.read_u8(ca + 3).wrapping_sub(q as u8);
        fx.mem.write_u8(ca + 3, v);
        // div1: LO1, not the LO / HI the other divisions leave.
        let al = i32::from(fx.mem.read_u8(t + 0x2F));
        let q = if div != 0 { al.wrapping_div(div) } else { 0 };
        let v = fx.mem.read_u8(cb + 3).wrapping_sub(q as u8);
        fx.mem.write_u8(cb + 3, v);
        rgbaq_to(fx, sp + 0x378, ca, d + 0x40);
        rgbaq_to(fx, sp + 0x380, cb, d + 0x58);
        let (qa, qb, ends) = if s0 != 0 {
            (a.wrapping_add((0x10 * s0) as u32), b.wrapping_add((0x10 * (s0 - 1)) as u32), (0x270u32, 0x280u32))
        } else {
            (pa, pb, (0x290, 0x2A0))
        };
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            spline::spline(fx, sp + 0x170, qa, f20, 0, [0.0; 4]);
            spline::spline(fx, sp + 0x1B0, qb, f20, 0, [0.0; 4]);
            f20 = ee::add(f20, 0.25);
            spline::spline(fx, sp + ends.0, qa, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x180, sp + ends.0);
            spline::spline(fx, sp + ends.1, qb, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x1C0, sp + ends.1);
            for (to, from) in [(0x130u32, 0x170u32), (0x140, 0x180), (0x150, 0x1B0), (0x160, 0x1C0)] {
                copy3(fx, sp + to, sp + from);
            }
            if l2::project(fx, sp + 0x130, 0x7000_0000, sp + 0xF0, d, sp + 0xC0) {
                fx.submit(d, OT, 0);
            }
        }
        s0 += 1;
    }
    fx.leave(0x390);
}

/// 0x1E05030(D, task, h): a textured strip3 ribbon along the one history
/// `h` (n = +6 points), each piece the points at t / t + 1/4 of entries s
/// - 1 and s widened by +-3 in x turned to face the camera (0x1095C0,
/// 0x1099E0 about y by -(camera yaw / 180) on D+0xD8), through the task's
/// TRS (0x1097D0 into sp + 0x110) at key 0; colours as `ribbon_tex`
/// (+0x28 near / +0x2C far, alphas down by +0x2B / +0x2F over n - 1); the
/// first segment on stack copies h0 h0 h1 h2 for both sides [F].
fn ribbon_cam(fx: &mut Fx, t: u32, h: u32) {
    let sp = fx.enter(0x3C0);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x2E8;
    l2::desc_strip3(fx, d);
    for (local, c, to) in [(0x378u32, 0x28u32, 0x10u32), (0x380, 0x2C, 0x28), (0x388, 0x28, 0x40), (0x390, 0x2C, 0x58)] {
        rgbaq_to(fx, sp + local, t + c, d + to);
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let rh2 = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, rh2, 1);
    let (ca, cb) = (sp + 0x3B8, sp + 0x3BC);
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(ca + k, v);
        let v = fx.mem.read_u8(t + 0x2C + k);
        fx.mem.write_u8(cb + k, v);
    }
    let pa = sp + 0x260;
    for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
        copy4(fx, pa + 0x10 * k as u32, h + o);
    }
    trs_words(fx, t, sp + 0xF0, sp + 0x100);
    l2::camera_angles_to(fx, sp + 0x150);
    let yaw = fx.mem.i32(sp + 0x154);
    fx.mult(yaw, 0xB60B_60B7_u32 as i32);
    fx.mem.set32(sp + 0x154, (yaw / 180).wrapping_neg());
    l2::xform_vp(fx, sp + 0x110, sp + 0x100, 0, sp + 0xF0);
    let div = n - 1;
    fx.mem.set32(sp + 0xB0, div);
    fx.mem.set32(sp + 0xC0, n - 2);
    let mut s0 = 0i32;
    while s0 < fx.mem.i32(sp + 0xC0) {
        rgbaq_to(fx, sp + 0x398, ca, d + 0x10);
        rgbaq_to(fx, sp + 0x3A0, cb, d + 0x28);
        let al = i32::from(fx.mem.read_u8(t + 0x2B));
        let q = fx.div(al, div).0;
        let v = fx.mem.read_u8(ca + 3).wrapping_sub(q as u8);
        fx.mem.write_u8(ca + 3, v);
        let al = i32::from(fx.mem.read_u8(t + 0x2F));
        let q = if div != 0 { al.wrapping_div(div) } else { 0 };
        let v = fx.mem.read_u8(cb + 3).wrapping_sub(q as u8);
        fx.mem.write_u8(cb + 3, v);
        rgbaq_to(fx, sp + 0x3A8, ca, d + 0x40);
        rgbaq_to(fx, sp + 0x3B0, cb, d + 0x58);
        let (qa, qb, ends) = if s0 != 0 {
            (h.wrapping_add((0x10 * (s0 - 1)) as u32), h.wrapping_add((0x10 * s0) as u32), (0x2A0u32, 0x2B0u32))
        } else {
            (pa, pa, (0x2C0, 0x2D0))
        };
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            l2::quad_xy(fx, sp + 0x1A0, 3.0, 0.0);
            l2::rotate4(fx, sp + 0x1A0, sp + 0x150, 0, D + 0xD8);
            spline::spline(fx, sp + 0x1E0, qa, f20, 0, [0.0; 4]);
            spline::spline(fx, sp + 0x220, qb, f20, 0, [0.0; 4]);
            f20 = ee::add(f20, 0.25);
            spline::spline(fx, sp + ends.0, qa, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x1F0, sp + ends.0);
            spline::spline(fx, sp + ends.1, qb, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x230, sp + ends.1);
            for (to, from) in [(0x160u32, 0x1E0u32), (0x170, 0x1F0), (0x180, 0x220), (0x190, 0x230)] {
                copy3(fx, sp + to, sp + from);
            }
            for (v, o) in [(0x160u32, 0x1A0u32), (0x170, 0x1B0), (0x180, 0x1C0), (0x190, 0x1D0)] {
                for k in 0..3 {
                    let x = fx.mem.f(sp + v + 4 * k);
                    let y = fx.mem.f(sp + o + 4 * k);
                    fx.mem.setf(sp + v + 4 * k, ee::add(x, y));
                }
            }
            if l2::project(fx, sp + 0x160, 0x7000_0000, sp + 0x110, d, sp + 0xE0) {
                fx.submit(d, OT, 0);
            }
        }
        s0 += 1;
    }
    fx.leave(0x3C0);
}

/// 0x1E05710(D, task, a): `spline::ribbon_cross` on a textured strip3
/// (0x109DD0 then 0x109B50, UVs D+0x94 in D+0xBE's order, texture D+0x88
/// filter 1; RGBAQs through 0x108020) with two strips a piece: pushed by w
/// = +0x38 / 10 in y (+ on the first and third vertices, - on the second
/// and fourth), then in x by w and z by w / 2 [F].
fn ribbon_cross_tex(fx: &mut Fx, t: u32, a: u32) {
    let sp = fx.enter(0x250);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 10.0);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x198;
    l2::fullscreen4(fx, d);
    l2::desc_strip3(fx, d);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let rh2 = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, rh2, 1);
    let c = sp + 0x24C;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(c + k, v);
    }
    let n3 = 3 * n;
    let pa = sp + 0xE0;
    for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
        copy4(fx, pa + 0x10 * k as u32, a + o);
    }
    fx.mem.set32(sp + 0xB0, n - 1);
    let mut s0 = 0i32;
    while s0 < fx.mem.i32(sp + 0xB0) {
        let (q, end) = if s0 != 0 { (a.wrapping_add((0x10 * (s0 - 1)) as u32), 0x170u32) } else { (pa, 0x180) };
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            spline::spline(fx, sp + 0xC0, q, f20, 0, [0.0; 4]);
            f20 = ee::add(f20, 0.25);
            spline::spline(fx, sp + end, q, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0xD0, sp + end);
            rgbaq_to(fx, sp + 0x228, c, d + 0x10);
            rgbaq_to(fx, sp + 0x230, c, d + 0x28);
            let al = i32::from(fx.mem.read_u8(t + 0x2B));
            let dq = fx.div(al, n3).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(c + 3)) - dq;
            fx.mem.write_u8(c + 3, if v > 0 { v as u8 } else { 0 });
            rgbaq_to(fx, sp + 0x238, c, d + 0x40);
            rgbaq_to(fx, sp + 0x240, c, d + 0x58);
            for s1 in 0..2 {
                for (to, from) in [(0x120u32, 0xC0u32), (0x130, 0xC0), (0x140, 0xD0), (0x150, 0xD0)] {
                    copy3(fx, sp + to, sp + from);
                }
                let adj = |fx: &mut Fx, at: u32, by: f32, plus: bool| {
                    let x = fx.mem.f(sp + at);
                    fx.mem.setf(sp + at, if plus { ee::add(x, by) } else { ee::sub(x, by) });
                };
                if s1 == 0 {
                    adj(fx, 0x124, w, true);
                    adj(fx, 0x144, w, true);
                    adj(fx, 0x134, w, false);
                    adj(fx, 0x154, w, false);
                } else {
                    let half = ee::div(w, 2.0);
                    adj(fx, 0x120, w, true);
                    adj(fx, 0x140, w, true);
                    adj(fx, 0x130, w, false);
                    adj(fx, 0x150, w, false);
                    adj(fx, 0x128, half, true);
                    adj(fx, 0x148, half, true);
                    adj(fx, 0x138, half, false);
                    adj(fx, 0x158, half, false);
                }
                if l2::project(fx, sp + 0x120, 0x7000_0000, 0, d, sp + 0x160) {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
                    fx.submit(d, OT, key);
                }
            }
        }
        s0 += 1;
    }
    fx.leave(0x250);
}

/// The attacker's node (slot at `node`) into `out`.
fn self_node(fx: &mut Fx, node: u32, out: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(node);
    fx.node_position(h, n, out);
}

/// The spawn point D+0x2D0 = `p` + (0, -10, 40) and the parameter `k`.
fn point_at(fx: &mut Fx, p: u32, k: i32) {
    let x = fx.mem.f(p);
    fx.mem.setf(POS, x);
    let y = fx.mem.f(p + 4);
    fx.mem.setf(POS + 4, ee::sub(y, 10.0));
    let z = fx.mem.f(p + 8);
    fx.mem.setf(POS + 8, ee::add(40.0, z));
    fx.mem.set16(PARAM, k);
}

/// The spawn point D+0x2D0 = the node's position less 1 in y.
fn point_node(fx: &mut Fx, node: u32, local: u32) {
    self_node(fx, node, local);
    let x = fx.mem.f(local);
    fx.mem.setf(POS, x);
    let y = fx.mem.f(local + 4);
    fx.mem.setf(POS + 4, ee::sub(y, 1.0));
    let z = fx.mem.f(local + 8);
    fx.mem.setf(POS + 8, z);
}

/// 0x1E02160(D, 0) (frame 0x70: ra +0x40, s3 +0x30 the runner's (0) [H],
/// s2 +0x20 the parameter, s1 +0x10 = 0, s0 = D): the spawn colour (0x80,
/// 0, 0, 0), the update counter and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x70);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x40, 0x01E0_3D4C), (0x30, 0), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    // (s1, s2, s3) the emitter's at each spawn.
    let mut s = [0u64, p, 0];
    if counter(fx) == 0x49 {
        spawn(fx, GREEN, 0x01E0_21B8, s);
    }
    if counter(fx) == 0x49 {
        spawn(fx, GREY, 0x01E0_21DC, s);
    }
    if counter(fx) == 0x49 {
        spawn(fx, DARK, 0x01E0_2200, s);
    }
    if counter(fx) == 0x49 {
        spawn(fx, GLOW, 0x01E0_2224, s);
    }
    if counter(fx) == 0x50 {
        spawn(fx, DRIFT, 0x01E0_2248, s);
    }
    let c = counter(fx);
    if (0x50..0x70).contains(&c) {
        let v = fx.mem.i16(NEXT);
        fx.mem.set16(PARAM, v);
        spawn(fx, CAMRIB, 0x01E0_2280, s);
        let v = fx.mem.i16(NEXT) + 1;
        fx.mem.set16(NEXT, v);
        if fx.mem.i16(NEXT) >= 0xE {
            fx.mem.set16(NEXT, 0);
        }
    }
    if counter(fx) == 0x67 {
        for k in 0..0x14u64 {
            for o in [0u32, 4, 8] {
                let w = fx.mem.read_u32(D + 0x2CC0 + o);
                fx.mem.write_u32(POS + o, w);
            }
            s[0] = k;
            spawn(fx, SLIVER, 0x01E0_22E0, s);
        }
        for k in 0..0x14u64 {
            s[0] = k;
            spawn(fx, MOTE_F, 0x01E0_2308, s);
        }
        s[0] = 0x14;
        spawn(fx, FLASH, 0x01E0_232C, s);
    }
    if counter(fx) == 0x50 {
        fx.mem.set16(PARAM, 0xF);
        spawn(fx, STREAK, 0x01E0_2358, s);
        fx.mem.set16(PARAM, 0x10);
        spawn(fx, STREAK, 0x01E0_2374, s);
    }
    if counter(fx) == 0x33 {
        self_node(fx, NODES + 2, D + 0x2960);
    }
    let c = counter(fx);
    if (0x34..0x41).contains(&c) {
        self_node(fx, NODES + 2, sp + 0x50);
        for _ in 0..6 {
            for o in [0u32, 4, 8] {
                let w = fx.mem.read_u32(D + 0x2960 + o);
                fx.mem.write_u32(POS + o, w);
            }
        }
    }
    if counter(fx) == 1 {
        let h = fx.self_handle();
        fx.unit_position(h, D + 0x320);
        let h = fx.self_handle();
        fx.unit_position(h, D + 0x330);
    }
    let c = counter(fx);
    if (0x70..0xC3).contains(&c) {
        let v = facing(fx, sp + 0x60, 0x01E0_2488);
        let z = fx.mem.f(D + 0x328);
        let by = f32::from_bits(0x4019_999A);
        fx.mem.setf(D + 0x328, if v != 0 { ee::sub(z, by) } else { ee::add(z, by) });
        point_at(fx, D + 0x320, 0);
    }
    if counter(fx) == 0x50 {
        point_node(fx, NODES + 4, sp + 0x50);
        fx.mem.set16(PARAM, 0);
        s[0] = u64::from(sp + 0x54);
        s[1] = u64::from(sp + 0x58);
        for k in 0..0x14u64 {
            s[2] = k;
            spawn(fx, MOTE_N, 0x01E0_2570, s);
        }
        point_node(fx, NODES + 6, sp + 0x50);
        fx.mem.set16(PARAM, 0);
        for k in 0..0x14u64 {
            s[2] = k;
            spawn(fx, MOTE_N, 0x01E0_25D8, s);
        }
        s[2] = 0x14;
    }
    if counter(fx) < 0x32 {
        s[0] = u64::from(sp + 0x54);
        s[1] = u64::from(sp + 0x58);
        point_node(fx, NODES + 4, sp + 0x50);
        spawn(fx, MOTE_S, 0x01E0_2650, s);
        point_node(fx, NODES + 6, sp + 0x50);
        spawn(fx, MOTE_S, 0x01E0_26A0, s);
    }
    let c = counter(fx);
    if (0xAB..0xE1).contains(&c) {
        let z = fx.mem.f(D + 0x338);
        fx.mem.setf(D + 0x338, ee::sub(z, f32::from_bits(0x4073_3333)));
        point_at(fx, D + 0x330, 1);
    }
    let c = counter(fx);
    if (0x70..0xBF).contains(&c) {
        point_at(fx, D + 0x320, 0);
    }
    let c = counter(fx);
    if (0xA9..0xDD).contains(&c) {
        point_at(fx, D + 0x330, 1);
    }
    if counter(fx) == 1 {
        spawn(fx, IDLE, 0x01E0_27D4, s);
    }
    fx.leave(0x70);
}

/// 0x1E04890(D, step, 0) (frame 0x60: ra +0x50, s4 the runner's (0) [H],
/// s3 / s2 / s1 the emitter's, s0 = D): the first free of the 150 tasks
/// cleared and set off at D+0x2D0 (as i16) with the colour D+0x3B04
/// (twice) and +0x30 = D+0x3B62; +0x20 = 0, +0x24 = the step.
fn spawn(fx: &mut Fx, f: u32, ra: u32, s: [u64; 3]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, s[2]), (0x20, s[1]), (0x10, s[0]), (0, u64::from(D))]);
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
