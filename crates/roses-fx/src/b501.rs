//! BMODULE b501 (card 194), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03A20) [F]. The entry
//! takes (D, param). A task pool of 130 tasks of 0x40 bytes at D+0x2C0
//! (alive +0x36, step +0x24 as a record address), spawned by 0x1E04690,
//! drawn with the second library: rings of quads with a mode (383a1b86ead2
//! at 0x1E01150, `quads4::ring_at_m`), 18-strip bands (7f395ce1b4cc at
//! 0x1E01ED0, `quads4::band18`), the turned sprite (38821b785abf at
//! 0x1E00E30, `quads4::sprite_turned`), the spline tube (94867f3df00e at
//! 0x1E016B0, `spline::tube`) and the record's own gouraud ribbon between
//! two point histories (0x1E047C0, [`ribbon`]); the histories are kept by
//! 0x1E00080 (a72bfb6bb70a, `spawn::node_history`). Update 3: six glows
//! (0x1E03130); 20: an object op through 0x1F8380 (until update 150);
//! 34..40 / 53 / 55: sparks at the attacker's nodes 9 / 0x12; 49: the
//! ribbons from nodes 0x70 / 0x71 / 0x72 and 0x6E; 50..58 and 90..120:
//! rings at node 0x70's position history D+0x2410 (50 entries) and bands
//! every fourth update; 90: the camera slots 4 / 5 (0x1E03880). Returns 1
//! on update 42, 4 on 90, 2 on 140, 6 on 43..124, 8 once the 160-update
//! timer D+0x2B10 has run out.
//!
//! The function 0x1E03800 (the object op step) is never called: the entry
//! has its body inline for tasks of that step [F 0x1E03F20..0x1E03F7C].
use crate::blib::{billboard, quads4, spawn, spline, uv};
use crate::bus::Bus;
use crate::c061::rgbaq_at;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0x82;
const POOL: u32 = D + 0x2C0;
const POS: u32 = D + 0x250;
/// Node 0x70's positions (50 x 16 bytes).
const TRACK: u32 = D + 0x2410;
const TIMER: u32 = D + 0x2B10;
const COLOUR: u32 = D + 0x2B14;
const INDEX: u32 = D + 0x2B54;
const ANGLE: u32 = D + 0x2B4E;
const TURN: u32 = D + 0x2B50;
const CURSOR: u32 = D + 0x2B5A;
const YAW: u32 = D + 0x2B5C;
const COUNTER: u32 = D + 0x2B5E;
const TAB: u32 = D + 0x2E60;
/// The four point histories (10 points each at +0x2750 / +0x27F0 /
/// +0x2890 / +0x2930).
const HIST_A: u32 = D + 0x2750;
const HIST_D: u32 = D + 0x2930;

fn node(fx: &mut Fx, off: u32) -> i32 {
    fx.mem.i16(D + off)
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// 0x1A2B80 of `v` from `ra` (its frame left), as an integer.
fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

/// The entry 0x1E03A20 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x100);
    fx.mem.write_u32(D + 0x2B1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x100);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2FD0);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x2B58, param);
    fx.mem.set32(TIMER, 0xA0);
    for (k, (n, clear)) in [(9, 1), (0x12, 0), (0x70, 0), (0x71, 0), (0x72, 0), (0x6E, 0), (0x3D, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(D + 0x2B3C + 2 * k as u32, r);
    }
    for k in 0xBu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x2AC + 4 * (k - 0xB), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x288, t);
    for k in 0..4u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x2730 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xE0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xE4)), 180.0);
    let y = fix(fx, y, 0x01E0_3BC8);
    fx.mem.set16(YAW, y);
    let a = D + 0x2B60;
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3BD8, 0x14);
        fx.mem.set8(a + k, v - 0xA);
    }
    for k in 0..0x64u32 {
        let v = fx.rand_from(D, 0x01E0_3C0C);
        fx.mem.set8(a + k, v % 8 - 4);
    }
    let b = D + 0x2C60;
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3C48, 0x14);
        fx.mem.set8(b + k, v - 0xA);
    }
    for k in 0xB4..0x100u32 {
        let v = if fx.mem.i16(YAW) != 0 { -(rmod(fx, D, 0x01E0_3C88, 0x14) + 0xA) } else { rmod(fx, D, 0x01E0_3CB8, 0x14) + 0xA };
        fx.mem.set8(b + k, v);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(b + k, 0);
    }
    let c = D + 0x2D60;
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3D14, 0x1E);
        fx.mem.set8(c + k, v - 0xF);
    }
    for k in 0xB4..0x100u32 {
        let v = if fx.mem.i16(YAW) != 0 { rmod(fx, D, 0x01E0_3D54, 0xF) + 3 } else { -(rmod(fx, D, 0x01E0_3D80, 0xF) + 3) };
        fx.mem.set8(c + k, v);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(c + k, 0);
    }
    0
}

/// The steps (their addresses, also the tasks' +0x24), the entry's return
/// address after each `jal` and the step's frame size (0x1E03800 inline).
const STEPS: [(u32, u32, u32); 9] = [
    (0x01E0_28B0, 0x01E0_3E20, 0x50),
    (0x01E0_2A20, 0x01E0_3E40, 0x51),
    (0x01E0_2D10, 0x01E0_3E60, 0x20),
    (0x01E0_2DE0, 0x01E0_3E80, 0x20),
    (0x01E0_2FE0, 0x01E0_3EA0, 0x20),
    (0x01E0_3130, 0x01E0_3EC0, 0x30),
    (0x01E0_3310, 0x01E0_3EE0, 0x70),
    (0x01E0_3530, 0x01E0_3F00, 0x20),
    (0x01E0_3680, 0x01E0_3F20, 0x30),
];
const OBJECT_OP: u32 = 0x01E0_3800;
const CAMERA: u32 = 0x01E0_3880;

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        for &(a, ra, size) in STEPS.iter() {
            if fx.mem.read_u32(t + 0x24) == a {
                // 0x51: the 0x50 frame saving only s0 / ra.
                let frame = size & !1;
                let s = fx.enter(frame);
                // The step's prologue: the entry's s2 = D + 0x40 k, s1 = 0,
                // s0 = D.
                match size {
                    0x20 | 0x51 => fx.saved(s, &[(0x10, u64::from(ra)), (0, u64::from(D))]),
                    0x30 => fx.saved(s, &[(0x20, u64::from(ra)), (0x10, 0), (0, u64::from(D))]),
                    _ => fx.saved(s, &[(0x30, u64::from(ra)), (0x20, u64::from(D + 0x40 * k)), (0x10, 0), (0, u64::from(D))]),
                }
                run_step(fx, a, t, s);
                fx.leave(frame);
            }
        }
        if fx.mem.read_u32(t + 0x24) == OBJECT_OP {
            for (to, from) in [(0xFCu32, 0x4D00u32), (0xF8, 0x4D08)] {
                let w = fx.mem.read_u32(0x01E0_0000 + from);
                fx.mem.write_u32(sp + to, w);
            }
            fx.boundary(0x001F_8380, "object op 0x1F8380", [2, 0, sp + 0xFC, sp + 0xF8]);
            if fx.mem.i16(COUNTER) == 0x96 {
                fx.mem.set16(t + 0x36, 0);
            }
        }
        if fx.mem.read_u32(t + 0x24) == CAMERA {
            let s = fx.enter(0x10);
            fx.saved(s, &[(0, 0x01E0_3FA0)]);
            camera(fx, t);
            fx.leave(0x10);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x60);
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_3FC4), (0, u64::from(D))]);
    let z = ee::sub(fx.mem.f(sp + 0x68), 25.0);
    fx.mem.setf(sp + 0x68, z);
    let mut count = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        count += 1;
        draw(fx, sp, t, k, count);
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0x2A {
        1
    } else if c == 0x5A {
        4
    } else if c == 0x8C {
        2
    } else if (0x2B..0x7D).contains(&c) {
        6
    } else if fx.mem.i32(TIMER) > 0 {
        0
    } else {
        8
    }
}

fn run_step(fx: &mut Fx, a: u32, t: u32, sp: u32) {
    match a {
        0x01E0_28B0 => ribbon_head(fx, t, sp),
        0x01E0_2A20 => sparks(fx, t, sp),
        0x01E0_2D10 => band(fx, t),
        0x01E0_2DE0 => ring_red(fx, t),
        0x01E0_2FE0 => ring_purple(fx, t, true),
        0x01E0_3130 => glow(fx, t),
        0x01E0_3310 => ribbons(fx, t, sp),
        0x01E0_3530 => ring_purple(fx, t, false),
        _ => flare(fx, t),
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends
/// (`end`) or the byte is cleared.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8, end: bool) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else if end {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.write_u8(t + at, 0);
    }
}

fn odd(n: i32) -> bool {
    n % 2 == 1
}

/// The attacker's node `off` (D + off) at `out`.
fn node_at(fx: &mut Fx, off: u32, out: u32) {
    let h = fx.self_handle();
    let n = node(fx, off);
    fx.node_position(h, n, out);
}

/// [`node_at`] from the entry's draw loop (s1 = `count`), leaving
/// 0x1F6830's frame (s0 = D, ra).
fn node_at_saving(fx: &mut Fx, off: u32, out: u32, count: u32, ra: u32) {
    let h = fx.self_handle();
    let n = node(fx, off);
    crate::b076::node_position_saving(fx, h, n, out, u64::from(D), Some(u64::from(count)), ra);
}

/// 0x1E028B0 (D, task): the blue ribbon's head: on the first update the
/// history D+0x2930 filled with node 0x6E's point; alpha 0xC0 for 11
/// updates, then out by 0x23; the history takes the node's point.
fn ribbon_head(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0xF0);
        node_at(fx, 0x2B46, sp + 0x40);
        for k in 0..9u32 {
            copy3(fx, HIST_D + 0x10 * k, sp + 0x40);
        }
        fx.mem.set16(t + 6, 8);
        fx.mem.write_u8(t + 0x2B, 0);
        let v = rmod(fx, t, 0x01E0_2940, 0x11);
        fx.mem.set16(t + 0xA, v + 0x2C);
        let v = fx.rand_from(t, 0x01E0_2960);
        fx.mem.set16(t + 8, v % 8 + 5);
        fx.mem.set16(t + 0x34, 0);
        let v = rmod(fx, t, 0x01E0_2984, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x38, 5);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xB {
        fx.mem.write_u8(t + 0x2B, 0xC0);
    } else {
        fade_out(fx, t, 0x2B, 0x25, 0x23, true);
    }
    let n = node(fx, 0x2B46);
    spawn::node_history(fx, HIST_D + 0x90, 0xA, n);
    age(fx, t);
}

/// 0x1E03310 (D, task): the three purple ribbons' histories (nodes 0x70 /
/// 0x71 / 0x72 into D+0x2750 / +0x27F0 / +0x2890), as [`ribbon_head`].
fn ribbons(fx: &mut Fx, t: u32, sp: u32) {
    const H: [(u32, u32, u32); 3] = [(0x2B40, 0x2750, 0x40), (0x2B42, 0x27F0, 0x50), (0x2B44, 0x2890, 0x60)];
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x80);
        for (n, first, local) in H {
            node_at(fx, n, sp + local);
            for k in 0..9u32 {
                copy3(fx, D + first + 0x10 * k, sp + local);
            }
        }
        fx.mem.set16(t + 6, 6);
        fx.mem.write_u8(t + 0x2B, 0);
        let v = rmod(fx, t, 0x01E0_3428, 0x11);
        fx.mem.set16(t + 0xA, v + 0x2C);
        let v = fx.rand_from(t, 0x01E0_3448);
        fx.mem.set16(t + 8, v % 8 + 5);
        fx.mem.set16(t + 0x34, 0);
        let v = rmod(fx, t, 0x01E0_346C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x38, 0x1E);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xB {
        fx.mem.write_u8(t + 0x2B, 0xC0);
    } else {
        fade_out(fx, t, 0x2B, 0x25, 0x23, true);
    }
    for (n, first, _) in H {
        let n = node(fx, n);
        spawn::node_history(fx, D + first + 0x90, 0xA, n);
    }
    age(fx, t);
}

/// 0x1E02A20 (D, task): a spark (size 40 + 1 a update, cell 0..5 on odd
/// updates) drifting z toward the target by 1 / 35 a update and down to
/// 15 under the attacker, grey (+0x30 = 0) or red; radius growing by
/// 5..9, random angles; in by 0x20 under 0x41, out by 9.
fn sparks(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x28);
        fx.mem.set16(t + 0x34, 0);
        if fx.mem.i16(t + 0x30) == 0 {
            l2::rgb(fx, t + 0x28, 0x30, 0x30, 0x30);
        } else {
            l2::rgb(fx, t + 0x28, 0x40, 0x10, 0x10);
        }
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, 0x01E0_2A90, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, 0x01E0_2AAC, 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rmod(fx, t, 0x01E0_2AC8, 0x19);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_2AE4, 0xA);
        fx.mem.set16(t + 6, v + 0x14);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x2B, 0x41, 0x20);
    } else {
        fade_out(fx, t, 0x2B, 0xB, 9, true);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x20);
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_2B70), (0, u64::from(t))]);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x30);
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_2B84), (0, u64::from(t))]);
    let dz = ee::sub(fx.mem.f(sp + 0x28), fx.mem.f(sp + 0x38));
    fx.mem.setf(sp + 0x28, dz);
    let dz = ee::div(dz, 35.0);
    fx.mem.setf(sp + 0x28, dz);
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), dz);
    let z = fix(fx, z, 0x01E0_2BCC);
    fx.mem.set16(t + 4, z);
    let floor = ee::sub(fx.mem.f(sp + 0x34), 15.0);
    let y = ee::from_i32(fx.mem.i16(t + 2));
    if !(y <= floor) && odd(fx.mem.i16(t + 0x32)) {
        let v = fx.mem.i16(t + 2) - 1;
        fx.mem.set16(t + 2, v);
    }
    if odd(fx.mem.i16(t + 0x32)) {
        let c = fx.mem.i16(t + 0x34);
        if c < 5 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    let s = fx.mem.i16(t + 0x38) + 1;
    fx.mem.set16(t + 0x38, s);
    for (o, ra) in [(0x10u32, 0x01E0_2C9C), (0x14, 0x01E0_2CB8), (0x18, 0x01E0_2CD4)] {
        let v = rmod(fx, t, ra, 0x168);
        fx.mem.set32(t + o, v);
    }
    age(fx, t);
}

/// 0x1E02D10 (D, task): a white / blue band (size 30, +30 a update) turned
/// 0x4000 about z; 0xA0 for 3 updates, out by 0x23.
fn band(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x1E);
        l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0xF0);
        l2::rgb(fx, t + 0x2C, 0x70, 0x70, 0xF0);
        fx.mem.set32(t + 0x18, 0x4000);
        let i = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, i);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if fx.mem.i16(t + 0x32) < 3 {
        fx.mem.write_u8(t + 0x2B, 0xA0);
    } else {
        fade_out(fx, t, 0x2B, 0x37, 0x23, true);
    }
    age(fx, t);
    let s = fx.mem.i16(t + 0x38) + 0x1E;
    fx.mem.set16(t + 0x38, s);
}

/// 0x1E02DE0 (D, task): a red ring (size 46, cell 0..1 + 1 every third
/// update to 7, flashing grey 0x80 / 0xC0), radius growing by 20..54,
/// life 2..4, turning by -2.
fn ring_red(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x2E);
        let v = fx.rand_from(t, 0x01E0_2E08);
        fx.mem.set16(t + 0x34, v % 2);
        l2::rgb(fx, t + 0x28, 0x80, 0x20, 0x20);
        let i = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, i);
        fx.mem.set16(t + 0x30, 0);
        fx.mem.set16(t + 0x3A, 0);
        let v = rmod(fx, t, 0x01E0_2E48, 0x23);
        fx.mem.set16(t + 0xE, v + 0x14);
        let v = rmod(fx, t, 0x01E0_2E68, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_2E84, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0xC, 0);
        let v = rmod(fx, t, 0x01E0_2EA8, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x2B, 0x11, 0xF, true);
    }
    let a = fx.mem.i16(t + 0x32);
    if fx.div(a, 3).1 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 7 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    let g = if odd(fx.mem.i16(t + 0x32)) { 0x80 } else { 0xC0 };
    l2::rgb(fx, t + 0x28, g, g, g);
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x3A) - 2;
        fx.mem.set16(t + 0x3A, v);
    }
    age(fx, t);
}

/// 0x1E02FE0 / 0x1E03530 (D, task): a purple ring (size 6, cell 0..3),
/// radius growing by 15..49, life 6..8; in by 0x40 under 0x81, out by 9.
/// 0x2FE0 clears the angle +0x3A (`clear`).
fn ring_purple(fx: &mut Fx, t: u32, clear: bool) {
    let ras: [u32; 4] = if clear { [0x01E0_3008, 0x01E0_3048, 0x01E0_3068, 0x01E0_3084] } else { [0x01E0_3558, 0x01E0_3594, 0x01E0_35B4, 0x01E0_35D0] };
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        let v = fx.rand_from(t, ras[0]);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, 0x80, 0x10, 0x40);
        let i = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, i);
        fx.mem.set16(t + 0x30, 0);
        if clear {
            fx.mem.set16(t + 0x3A, 0);
        }
        let v = rmod(fx, t, ras[1], 0x23);
        fx.mem.set16(t + 0xE, v + 0xF);
        let v = rmod(fx, t, ras[2], 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, ras[3], 3);
        fx.mem.set16(t + 6, v + 6);
        fx.mem.set16(t + 0xC, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x2B, 0x81, 0x40);
    } else {
        fade_out(fx, t, 0x2B, 0xB, 9, true);
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    age(fx, t);
}

/// 0x1E03130 (D, task): a grey / blue band (size 60) turned 0x4000 about
/// z: in by 4 under 0x50 for 18 updates, then flickering 0x40 / 0x50 to
/// update 53, out by 0x28 to update 68, 0x50 on 69, flickering to 123,
/// then out by 0xF; ends after 211 updates or on update 138.
fn glow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        l2::rgb(fx, t + 0x28, 0x70, 0x70, 0x70);
        l2::rgb(fx, t + 0x2C, 0x10, 0x10, 0xC0);
        fx.mem.set32(t + 0x18, 0x4000);
        let i = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, i);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    let a = fx.mem.i16(t + 0x32);
    let flicker = |fx: &mut Fx| fx.mem.write_u8(t + 0x2B, if odd(a) { 0x40 } else { 0x50 });
    if a < 0x12 {
        fade_in(fx, t, 0x2B, 0x50, 4);
    } else {
        let c = fx.mem.i16(COUNTER);
        if c < 0x36 {
            flicker(fx);
        } else if c < 0x45 {
            fade_out(fx, t, 0x2B, 0x11, 0x28, false);
        } else if c == 0x45 {
            fx.mem.write_u8(t + 0x2B, 0x50);
        } else if c < 0x7C {
            flicker(fx);
        } else {
            fade_out(fx, t, 0x2B, 0x11, 0xF, true);
        }
    }
    age(fx, t);
    if fx.mem.i16(t + 0x32) >= 0xD3 {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(COUNTER) == 0x8A {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E03680 (D, task): a white flare (size 60, cell 5) at a node,
/// circling (+0x30 a random angle, else still) 10 a update in x / z by
/// the angle +0x3A; 0x80 for 7 updates, out by 0xF.
fn flare(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        fx.mem.set16(t + 0x34, 5);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        if fx.mem.i16(t + 0x30) == 0 {
            let v = rmod(fx, t, 0x01E0_36DC, 0x168);
            fx.mem.set16(t + 0x30, v);
        } else {
            fx.mem.set16(t + 0x30, 0);
        }
        let v = rmod(fx, t, 0x01E0_3704, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x2B, 0x11, 0xF, true);
    }
    if fx.mem.i16(t + 0x30) != 0 {
        for (o, cos, ra) in [(8u32, 0, 0x01E0_37A0), (0xC, 1, 0x01E0_37D8)] {
            let a = fx.mem.i16(t + 0x3A);
            let s = l2::sin_deg(fx, TAB, a, cos);
            let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(10.0, s));
            let v = fix(fx, v, ra);
            fx.mem.set16(t + o, v);
        }
    }
    age(fx, t);
}

/// 0x1E03880 (D, task): camera slots 4 / 5 (0x1F7E20) from D+0x2340
/// (128 / 192 flickering, 4, 2; the task's position; 0, 120, w rising by
/// 0.2 to 1 for 21 updates, then down by 0.1); cleared (0x1F7E20(0)) when
/// the task ends (w at 0.1, or the timer at 1).
fn camera(fx: &mut Fx, t: u32) {
    let c = D + 0x2340;
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0x4000_0000), (0xC, 0), (0x20, 0), (0x24, 0x42F0_0000), (0x28, 0)] {
            fx.mem.write_u32(c + o, v);
        }
        for k in 0..3u32 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(c + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    let w = fx.mem.f(c + 0x28);
    if fx.mem.i16(t + 0x32) < 0x15 {
        if w < 1.0 {
            fx.mem.setf(c + 0x28, ee::add(w, f32::from_bits(0x3E4C_CCCD)));
        }
    } else {
        let tenth = f32::from_bits(0x3DCC_CCCD);
        if w <= tenth {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.setf(c + 0x28, ee::sub(w, tenth));
        }
    }
    let x = if odd(fx.mem.i16(t + 0x32)) { 0x4300_0000 } else { 0x4340_0000 };
    fx.mem.write_u32(c, x);
    age(fx, t);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
    let (p, ra) = if fx.mem.i16(t + 0x36) != 0 { (c, 0x01E0_3A0C) } else { (0, 0x01E0_39F8) };
    crate::blib::exe::camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = D, ra).
    let sp = fx.sp;
    fx.saved(sp - 0x20, &[(0, u64::from(D)), (0x10, ra)]);
}

/// 0x1E04690(D, step, a2): a new task (first free of 130) with the spawn
/// colour D+0x2B14 (both colours), at D+0x250, index D+0x2B54, angle
/// D+0x2B4E.
fn spawn_task(fx: &mut Fx, step: u32) {
    fx.enter(0x60);
    let mut task = None;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            task = Some(t);
            break;
        }
    }
    if let Some(t) = task {
        for at in [0x28u32, 0x2C] {
            let c: [i32; 3] = std::array::from_fn(|k| i32::from(fx.mem.read_u8(COLOUR + k as u32)));
            l2::rgb(fx, t + at, c[0], c[1], c[2]);
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + at + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        let i = fx.mem.i16(INDEX);
        fx.mem.set16(t + 0x30, i);
        let a = fx.mem.i16(ANGLE);
        fx.mem.set16(t + 0x3A, a);
        fx.mem.write_u32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, step);
    }
    fx.leave(0x60);
}

/// D+0x250 = the attacker's node `off` (through the emitter's local).
fn pos_at(fx: &mut Fx, sp: u32, off: u32) {
    node_at(fx, off, sp + 0x50);
    copy3(fx, POS, sp + 0x50);
}

/// The next point of node 0x70's track, a ring task `step` at it.
fn track(fx: &mut Fx, step: u32) {
    let i = fx.mem.i16(CURSOR);
    let h = fx.self_handle();
    let n = node(fx, 0x2B40);
    fx.node_position(h, n, (TRACK as i32 + 0x10 * i) as u32);
    let i = fx.mem.i16(CURSOR);
    fx.mem.set16(INDEX, i);
    let a = fx.mem.i16(TURN);
    fx.mem.set16(ANGLE, a);
    spawn_task(fx, step);
}

fn next_cursor(fx: &mut Fx) {
    let i = fx.mem.i16(CURSOR) + 1;
    fx.mem.set16(CURSOR, i);
    if fx.mem.i16(CURSOR) >= 0x32 {
        fx.mem.set16(CURSOR, 0);
    }
}

/// 0x1E02370(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x70).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x70);
    // The entry's ra, s3 (the runner's 0), s2 = param, s1 = 0, s0 = D.
    fx.saved(sp, &[(0x40, 0x01E0_3DEC), (0x30, 0), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    l2::rgb(fx, COLOUR, 0x80, 0, 0);
    fx.mem.write_u8(COLOUR + 3, 0);
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 0x14 {
        spawn_task(fx, OBJECT_OP);
    }
    if c(fx) == 0x5A {
        spawn_task(fx, CAMERA);
    }
    if c(fx) == 0x22 {
        for _ in 0..0xC {
            pos_at(fx, sp, 0x2B3C);
            spawn_task(fx, 0x01E0_3680);
            pos_at(fx, sp, 0x2B3E);
            spawn_task(fx, 0x01E0_3680);
        }
    }
    if (0x22..0x29).contains(&c(fx)) {
        fx.mem.set16(INDEX, 1);
        pos_at(fx, sp, 0x2B3C);
        spawn_task(fx, 0x01E0_3680);
        pos_at(fx, sp, 0x2B3E);
        spawn_task(fx, 0x01E0_3680);
    }
    if c(fx) == 0x35 || c(fx) == 0x37 {
        fx.mem.set16(INDEX, 0);
        for _ in 0..0xC {
            if c(fx) == 0x35 {
                pos_at(fx, sp, 0x2B3C);
                spawn_task(fx, 0x01E0_3680);
            }
            if c(fx) == 0x37 {
                pos_at(fx, sp, 0x2B3E);
                spawn_task(fx, 0x01E0_3680);
            }
        }
    }
    if (0x5A..0x79).contains(&c(fx)) {
        track(fx, 0x01E0_3530);
        spawn_task(fx, 0x01E0_2DE0);
        next_cursor(fx);
        if c(fx) % 4 == 1 {
            spawn_task(fx, 0x01E0_2D10);
        }
    }
    if (0x32..0x3B).contains(&c(fx)) {
        track(fx, 0x01E0_2FE0);
        next_cursor(fx);
    }
    let a = fx.mem.i16(TURN) - 8;
    fx.mem.set16(TURN, a);
    let a = fx.mem.i16(TURN);
    if a < -0x167 {
        let r = fx.div(a, 0x168).1;
        fx.mem.set16(TURN, r);
    }
    if c(fx) == 0x31 {
        spawn_task(fx, 0x01E0_3310);
    }
    if c(fx) == 0x31 {
        spawn_task(fx, 0x01E0_28B0);
    }
    if c(fx) == 3 {
        for i in 0..6 {
            fx.mem.set16(INDEX, i);
            spawn_task(fx, 0x01E0_3130);
        }
    }
    fx.leave(0x70);
}

/// The atlas rectangle of sprite `id` frame `sub` (into the entry's local
/// `local`) as D+0x94.
fn rect_d(fx: &mut Fx, local: u32, id: i32, sub: i32) {
    l2::rect(fx, local, id, sub);
    copy4(fx, D + 0x94, local);
}

/// D+0x20 = the task's point of node 0x70's track (+0xA).
fn at_track(fx: &mut Fx, t: u32) {
    for k in 0..3u32 {
        let i = fx.mem.i16(t + 0xA);
        let w = fx.mem.read_u32((TRACK as i32 + 0x10 * i) as u32 + 4 * k);
        fx.mem.write_u32(D + 0x20 + 4 * k, w);
    }
}

/// The ribbon's atlas setup (cell 0x97, D+0x98 = 1, centre 0, UV 1).
fn ribbon_setup(fx: &mut Fx, t: u32, local: u32) {
    for (o, v) in [(0x94u32, 1), (0x96, 0x10), (0x98, 0x1F), (0x9A, 1)] {
        fx.mem.set16(D + o, v);
    }
    let c = fx.mem.i16(t + 0x34);
    rect_d(fx, local, 0x97, c);
    fx.mem.set16(D + 0x98, 1);
    for o in [0x20u32, 0x24, 0x28] {
        fx.mem.set32(D + o, 0);
    }
    fx.mem.set16(D + 0xBE, 1);
}

/// The entry's draw of one live task (k its index, `count` the live tasks
/// so far: the entry's s4 / s1).
fn draw(fx: &mut Fx, sp: u32, t: u32, k: u32, count: u32) {
    let s = fx.mem.read_u32(t + 0x24);
    let sub = |fx: &mut Fx| fx.mem.i16(t + 0x34);
    // The entry's s0..s7 / fp / ra as a callee of the draw loop saves them
    // (s3 = sp + 0x68, s5.. the runner's 0).
    let saves = |ra: u32| [(u64::from(D)), u64::from(count), u64::from(D + 0x40 * k), u64::from(sp + 0x68), u64::from(k), u64::from(ra)];
    match s {
        0x01E0_28B0 => {
            let w = fx.mem.read_u32(D + 0x2BC);
            fx.mem.write_u32(D + 0x88, w);
            ribbon_setup(fx, t, sp + 0x80);
            for ra in [0x01E0_408C, 0x01E0_40A0, 0x01E0_40B4] {
                ribbon(fx, t, HIST_A, HIST_D, saves(ra));
            }
        }
        0x01E0_2D10 => {
            node_at_saving(fx, 0x2B40, sp + 0x60, count, 0x01E0_40E4);
            quads4::band18(fx, D, t, 0, sp + 0x60);
        }
        0x01E0_2DE0 | 0x01E0_2FE0 | 0x01E0_3530 => {
            let (local, id, tex) = match s {
                0x01E0_2DE0 => (0x90, 0x14, 0x273C),
                0x01E0_2FE0 => (0xA0, 0x9B, 0x2BC),
                _ => (0xC0, 0x9B, 0x2BC),
            };
            let c = sub(fx);
            rect_d(fx, sp + local, id, c);
            let w = fx.mem.read_u32(D + tex);
            fx.mem.write_u32(D + 0x88, w);
            let table = match s {
                0x01E0_2DE0 => {
                    fx.mem.set16(D + 0xB8, 8);
                    let v = fx.rand_from(D, 0x01E0_4158);
                    fx.mem.set16(D + 0xBE, v % 8);
                    D + 0x2D60
                }
                0x01E0_2FE0 => {
                    fx.mem.set16(D + 0xB8, 0xD);
                    fx.mem.set16(D + 0xBE, 0);
                    D + 0x2C60
                }
                _ => {
                    let n = if fx.mem.i16(COUNTER) < 0xDC { 0xF } else { 8 };
                    fx.mem.set16(D + 0xB8, n);
                    fx.mem.set16(D + 0xBE, 0);
                    D + 0x2B60
                }
            };
            at_track(fx, t);
            quads4::ring_at_m(fx, uv::uv_rect, D, t, 0x20, table);
            fx.mem.set16(D + 0xB8, 0);
        }
        0x01E0_3130 => {
            node_at_saving(fx, 0x2B40, sp + 0x60, count, 0x01E0_42AC);
            node_at_saving(fx, 0x2B46, sp + 0x70, count, 0x01E0_42C4);
            for k in 0..3u32 {
                let v = ee::sub(fx.mem.f(sp + 0x70 + 4 * k), fx.mem.f(sp + 0x60 + 4 * k));
                fx.mem.setf(sp + 0x70 + 4 * k, v);
            }
            for k in 0..3u32 {
                let v = ee::div(fx.mem.f(sp + 0x70 + 4 * k), 20.0);
                fx.mem.setf(sp + 0x70 + 4 * k, v);
            }
            let f = ee::from_i32(fx.mem.i16(t + 0x30));
            for k in 0..3u32 {
                let v = ee::add(fx.mem.f(sp + 0x60 + 4 * k), ee::mul(fx.mem.f(sp + 0x70 + 4 * k), f));
                fx.mem.setf(sp + 0x60 + 4 * k, v);
            }
            quads4::band18(fx, D, t, 1, sp + 0x60);
        }
        0x01E0_3310 => {
            let w = fx.mem.read_u32(D + 0x2BC);
            fx.mem.write_u32(D + 0x88, w);
            ribbon_setup(fx, t, sp + 0xB0);
            let r = saves(0x01E0_443C);
            let at = 0x01E0_16B0;
            fx.saved(fx.sp - 0x2E0, &[(0x20, r[0]), (0x30, r[1]), (0x40, r[2]), (0x50, r[3]), (0x60, r[4]), (0x70, 0), (0x80, 0), (0x90, 0), (0xA0, 0), (0xB0, r[5])]);
            spline::tube(fx, at, D, t, HIST_A, 2);
            ribbon(fx, t, HIST_A, HIST_D, saves(0x01E0_4450));
        }
        0x01E0_3680 => {
            let w = fx.mem.read_u32(D + 0x2738);
            fx.mem.write_u32(D + 0x88, w);
            let c = sub(fx);
            rect_d(fx, sp + 0xD0, 0x14, c);
            fx.mem.set16(D + 0xBE, 0);
            for k in 0..3u32 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                fx.mem.setf(sp + 0x60 + 4 * k, v);
            }
            let uv = billboard::uv_callee(fx, 0x01E0_0F48).unwrap_or(uv::uv_rect);
            quads4::sprite_turned(fx, uv, D, t, 0, sp + 0x60);
        }
        _ => {}
    }
}

/// 0x1E047C0 (D, task, a, b): a gouraud ribbon between the point
/// histories `a` and `b` (i16 +6 points; the first segment's point before
/// is the first point): per segment four strips (t = 0, 0.25, 0.5, 0.75;
/// vertices a(t), a(t + 1/4), b(t), b(t + 1/4) by [`spline::spline`]
/// basis 0) at key 0 through the camera (matrix 0), coloured +0x28 with
/// the later pair's alpha down by alpha / (2 n) a strip [F]. `saves`: the
/// draw loop's s0 / s1 / s2 / s3 / s4 and the return address.
pub(crate) fn ribbon(fx: &mut Fx, t: u32, a: u32, b: u32, saves: [u64; 6]) {
    use ee::add;
    let sp = fx.enter(0x2E0);
    fx.saved(sp, &[(0x10, saves[0]), (0x20, saves[1]), (0x30, saves[2]), (0x40, saves[3]), (0x50, saves[4]), (0x60, 0), (0x70, 0), (0x80, 0), (0x90, 0), (0xA0, saves[5])]);
    fx.mem.write_u32(sp + 0xC0, b);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x228;
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    let colour = sp + 0x2DC;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(colour + k, v);
    }
    let (pa, pb) = (sp + 0x160, sp + 0x1A0);
    for (p, src) in [(pa, a), (pb, b)] {
        copy4(fx, p, src);
        copy4(fx, p + 0x10, src);
        copy4(fx, p + 0x20, src + 0x10);
        copy4(fx, p + 0x30, src + 0x20);
    }
    fx.mem.set32(sp + 0xB0, n - 1);
    let s5 = 2 * n;
    let mut s0 = 0;
    while s0 < n - 1 {
        let off = (0x10 * (s0 - 1)) as u32;
        let (s3, s4) = (a.wrapping_add(off), b.wrapping_add(off));
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            rgbaq_at(fx, sp + 0x2B8, colour, d + 0x10);
            rgbaq_at(fx, sp + 0x2C0, colour, d + 0x30);
            let al = i32::from(fx.mem.read_u8(t + 0x2B));
            let q = fx.div(al, s5).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(colour + 3)) - q;
            fx.mem.write_u8(colour + 3, if v > 0 { v as u8 } else { 0 });
            rgbaq_at(fx, sp + 0x2C8, colour, d + 0x20);
            rgbaq_at(fx, sp + 0x2D0, colour, d + 0x40);
            let (qa, qb, oa, ob) = if s0 != 0 { (s3, s4, 0x1E0u32, 0x1F0u32) } else { (pa, pb, 0x200, 0x210) };
            spline::spline(fx, sp + 0x120, qa, f20, 0, [0.0; 4]);
            spline::spline(fx, sp + 0x140, qb, f20, 0, [0.0; 4]);
            f20 = add(f20, 0.25);
            spline::spline(fx, sp + oa, qa, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x130, sp + oa);
            spline::spline(fx, sp + ob, qb, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x150, sp + ob);
            for (to, from) in [(0xE0u32, 0x120u32), (0xF0, 0x130), (0x100, 0x140), (0x110, 0x150)] {
                copy3(fx, sp + to, sp + from);
            }
            if l2::project(fx, sp + 0xE0, SPR, 0, d, sp + 0xD0) {
                fx.submit(d, OT, 0);
            }
        }
        s0 += 1;
    }
    fx.leave(0x2E0);
}
