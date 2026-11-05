//! BMODULE b191 (card 374), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E031A0, data area 0x1ED0000, 0xAC60 bytes) [F]
//!: a pool of 150 tasks of 0x70 bytes at D+0x2A0
//! (alive +0x58, step +0x44 run through `jalr`), spawned by 0x1E03AB0
//! (ae03cf745317, `spawn::task400_b4`'s layout copy: colour D+0xA894, +0x50
//! from D+0xA8EA). Every update the attacker's u16 +0x20A = 1 (0x1F69C0).
//!
//! The emitter 0x1E01580 counts D+0xA8F2: on 0xBE two white arcs
//! 0x1E02050 and two dark arcs 0x1E02850 (`arc_strips_flat` at node 2),
//! the stage light 0x1E02D30 and the frame window 0x1E02E20 (the frame
//! sampled through a window sprite, its ST swaying); on 0xA6..0x122 every
//! third update two spiral ribbons 0x1E01A70 at node 2 (two 20-point
//! histories each, D+0x4470 + 0x280 k / D+0x45B0 + 0x280 k, drawn by the
//! record's 0x1E03C40: a Lagrange-cubic strip3 ribbon at the far Z) and a
//! ring 0x1E02B50 (`ring_quads` on D+0xA8F4); on 0xC4..0x104 smoke rings
//! 0x1E02600 (`ring_quads` on D+0xA9F4) and every tenth update two homing
//! motes 0x1E021D0 (`billboard::quad`, swung by +0x48); on 0xBC twenty of
//! each; on 0x83 / 0x8D / 0x97 / 0xA1 forty drifting quads 0x1E029D0 at
//! nodes 0x56 / 0x60. Returns 1 on update 0xE6, 4 on 0x1C7, 2 on 0x181, 6
//! on 0xF1..0x17B, 8 once the 461-update timer D+0xA890 has run out. No
//! 0x1F8100.
use crate::blib::{billboard, spawn, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
const POOL: u32 = D + 0x2A0;
const POS: u32 = D + 0x250;
const TIMER: u32 = D + 0xA890;
const NODES: u32 = D + 0xA8BC;
const SLOT: u32 = D + 0xA8EA;
const NEXT: u32 = D + 0xA8EC;
const COUNTER: u32 = D + 0xA8F2;
const BYTES: u32 = D + 0xA8F4;
const BYTES_B: u32 = D + 0xA9F4;
const SINES: u32 = D + 0xAAF4;
const TEX5: u32 = D + 0xA870;
const TEX_F: u32 = D + 0x29C;
const HIST_A: u32 = D + 0x4470;
const HIST_B: u32 = D + 0x45B0;

const SPIRAL: u32 = 0x01E0_1A70;
const ARC_W: u32 = 0x01E0_2050;
const MOTE: u32 = 0x01E0_21D0;
const SMOKE: u32 = 0x01E0_2600;
const ARC_D: u32 = 0x01E0_2850;
const DRIFT: u32 = 0x01E0_29D0;
const RING: u32 = 0x01E0_2B50;
const LIGHT: u32 = 0x01E0_2D30;
const WINDOW: u32 = 0x01E0_2E20;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_35A0;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E031A0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0xA89C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xAC60);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xA8F0, param);
    fx.mem.set32(TIMER, 0x1CD);
    for (k, (n, clear)) in [(2, 1), (0x1C, 0), (0xD, 0), (0x56, 0), (0x60, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xA8CC, r);
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x288 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX5 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_3380);
    fx.mem.set16(D + 0xA8EE, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frame: s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_33B0, 0x1E);
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_340C);
        fx.mem.write_u8(BYTES + k, (v & 3) as u8);
    }
    for k in 0xB4..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_346C);
        fx.mem.write_u8(BYTES + k, ((v & 0xF) - 8) as u8);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_34C4, 0x28);
        fx.mem.write_u8(BYTES_B + k, (v - 0x14) as u8);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3514, 0x14);
        fx.mem.write_u8(BYTES_B + k, v as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    crate::b369::unit_flag(fx, 1, u64::from(D), 0x01E0_3564);
    emit(fx, param);
    for k in 0..TASKS {
        let t = POOL + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        // The step's prologue: ra, the entry's s3 = k, s2 = D + 0x70 k, s1
        // = 0, s0 = D.
        let (ra, d, s2, s3) = (TASK_RA, u64::from(D), u64::from(D + 0x70 * k), u64::from(k));
        let (frame, saves): (u32, &[(u32, u64)]) = match f {
            SPIRAL => (0x80, &[(0x50, ra), (0x40, s3), (0x30, s2), (0x20, 0), (0x10, d)]),
            ARC_W | ARC_D | DRIFT | WINDOW => (0x30, &[(0x20, ra), (0x10, 0), (0, d)]),
            MOTE => (0x60, &[(0x30, ra), (0x20, s2), (0x10, 0), (0, d)]),
            SMOKE | RING => (0x20, &[(0x10, ra), (0, d)]),
            LIGHT => (0x10, &[(0, ra)]),
            _ => panic!("b191: unknown task step {f:#x}"),
        };
        let frame = if f == WINDOW { 0x90 } else { frame };
        let s = fx.enter(frame);
        fx.saved(s, saves);
        match f {
            SPIRAL => spiral(fx, t, s),
            ARC_W => arc(fx, t, 0x01E0_21A0, ((0xC0, 0xC0, 0xC0), (0x80, 0x80, 0x60)), (0x122, 0x18), [(0x50, 0x50, 0x80), (0x60, 0x60, 0xA0)]),
            ARC_D => arc(fx, t, 0x01E0_29A0, ((0x20, 0x20, 0x20), (0x80, 0x80, 0x60)), (0xA, 0x60), [(0x20, 0x20, 0x80), (0x30, 0x30, 0xA0)]),
            MOTE => mote(fx, t, s),
            SMOKE => smoke(fx, t),
            DRIFT => drift(fx, t),
            RING => ring(fx, t),
            LIGHT => light(fx, t),
            WINDOW => window(fx, t, s),
            _ => {}
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = POOL + 0x70 * k;
        if fx.mem.i16(t + 0x58) != 0 {
            draw(fx, sp, t);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0xE6 => 1,
        0x1C7 => 4,
        0x181 => 2,
        0xF1..=0x17B => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    fx.mem.i16(t + 0x54)
}

fn grow(fx: &mut Fx, t: u32) {
    let n = age(fx, t) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x58) == 1
}

/// Alpha +0x63 up by `by` while below `cap`.
fn rise(fx: &mut Fx, t: u32, cap: u8, by: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if a < cap {
        fx.mem.write_u8(t + 0x63, a.wrapping_add(by));
    }
}

/// Alpha `o` down by `down` while at least `floor`, else the task ends.
fn fall_at(fx: &mut Fx, t: u32, o: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + o);
    if a >= floor {
        fx.mem.write_u8(t + o, a - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn rgb3(fx: &mut Fx, p: u32, c: (i32, i32, i32)) {
    l2::rgb(fx, p, c.0, c.1, c.2);
}

/// sin (cos 0) / cos of the i16 degrees `deg` on D+0xAAF4.
fn sin(fx: &mut Fx, deg: i32, cos: i32) -> f32 {
    l2::sin_deg(fx, SINES, deg as i16 as i32, cos)
}

/// The task's history pair: D+0x4470 / D+0x45B0 + 0x280 x i16 +0x50.
fn hists(fx: &mut Fx, t: u32) -> (u32, u32) {
    let p = fx.mem.i16(t + 0x50);
    let o = (0x280 * p) as u32;
    (HIST_A.wrapping_add(o), HIST_B.wrapping_add(o))
}

/// 0x1E01A70: a spiral ribbon about node 2 (radii +0x48 = rand % 50 +
/// 0x12 and +0x4C = 18, the second 30 degrees behind and 50 up; both
/// 20-point histories filled flat on the first update): every update the
/// angle +0x5A turns by 12, the histories push the points at radius +0x48
/// (later 5) at height 0 and +0x4C at 5 + jitter (rand % 0x2D - 5 every
/// tenth update, else rand % 5 - 5); alpha up by 0x15 below 0x80 for 7
/// updates from update 0xBF, else down by 5; colour (0x10, 0x10, 0x50) /
/// (8, 8, 0x40) by the age's parity; +6 grows to 0xE; from 0x123 the age
/// holds at 7.
fn spiral(fx: &mut Fx, t: u32, sp: u32) {
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x70);
    let (ha, hb) = hists(fx, t);
    // rand's s0: the entry's D until the first update sets sp + 0x64.
    let mut s0 = D;
    if first(fx, t) {
        let f20 = 50.0f32;
        fx.mem.write_u32(t + 0x34, f20.to_bits());
        let v = fx.rand_from(s0, 0x01E0_1ADC);
        fx.mem.set16(t + 8, v & 1);
        let r = rmod(fx, s0, 0x01E0_1AF8, 0x32);
        fx.mem.setf(t + 0x48, ee::from_i32(r + 0x12));
        fx.mem.write_u32(t + 0x4C, 0x4190_0000);
        l2::rgb(fx, t + 0x64, 0x58, 0x58, 0x58);
        s0 = sp + 0x64;
        fx.mem.write_u32(sp + 0x64, 0);
        let r = rmod(fx, s0, 0x01E0_1B40, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let f21 = fx.mem.f(t + 0x48);
        let a = fx.mem.i16(t + 0x5A);
        let s = sin(fx, a, 0);
        fx.mem.setf(sp + 0x68, ee::mul(f21, s));
        let c = sin(fx, a, 1);
        fx.mem.setf(sp + 0x60, ee::mul(f21, c));
        for i in 0..0x13 {
            for k in 0..3 {
                let w = fx.mem.read_u32(sp + 0x60 + 4 * k);
                fx.mem.write_u32(ha + 0x10 * i + 4 * k, w);
            }
        }
        let f21 = fx.mem.f(t + 0x4C);
        let s = sin(fx, a - 0x1E, 0);
        fx.mem.setf(sp + 0x68, ee::mul(f21, s));
        let c = sin(fx, a - 0x1E, 1);
        fx.mem.setf(sp + 0x60, ee::mul(f21, c));
        fx.mem.setf(sp + 0x64, f20);
        for i in 0..0x13 {
            for k in 0..3 {
                let w = fx.mem.read_u32(sp + 0x60 + 4 * k);
                fx.mem.write_u32(hb + 0x10 * i + 4 * k, w);
            }
        }
        fx.mem.set16(t + 6, 2);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    if age(fx, t) < 7 && counter(fx) >= 0xBF {
        rise(fx, t, 0x80, 0x15);
    } else {
        fall_at(fx, t, 0x63, 7, 5);
    }
    let a = age(fx, t);
    let f20 = if fx.div(a, 0xA).1 == 1 {
        let r = rmod(fx, s0, 0x01E0_1DAC, 0x2D);
        ee::from_i32(r - 5)
    } else {
        let r = rmod(fx, s0, 0x01E0_1DD8, 5);
        ee::from_i32(r - 5)
    };
    let v = fx.mem.i16(t + 0x5A) + 0xC;
    fx.mem.set16(t + 0x5A, v);
    let v = fx.mem.i16(t + 0x5A);
    if v >= 0x168 {
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
    }
    fx.mem.write_u32(sp + 0x64, 0);
    let a = fx.mem.i16(t + 0x5A);
    let f21 = fx.mem.f(t + 0x48);
    let s = sin(fx, a, 0);
    fx.mem.setf(sp + 0x68, ee::mul(f21, s));
    let c = sin(fx, a, 1);
    fx.mem.setf(sp + 0x60, ee::mul(f21, c));
    spawn::history(fx, ha + 0x130, 0x14, sp + 0x60);
    fx.mem.setf(sp + 0x64, ee::add(5.0, f20));
    let f20 = fx.mem.f(t + 0x4C);
    let s = sin(fx, a - 0x1E, 0);
    fx.mem.setf(sp + 0x68, ee::mul(f20, s));
    let c = sin(fx, a - 0x1E, 1);
    fx.mem.setf(sp + 0x60, ee::mul(f20, c));
    spawn::history(fx, hb + 0x130, 0x14, sp + 0x60);
    if age(fx, t) & 1 == 1 {
        l2::rgb(fx, t + 0x60, 0x10, 0x10, 0x50);
    } else {
        l2::rgb(fx, t + 0x60, 8, 8, 0x40);
    }
    if age(fx, t) != 0 {
        let n = fx.mem.i16(t + 6);
        if n < 0xE {
            fx.mem.set16(t + 6, n + 1);
        }
    }
    grow(fx, t);
    if counter(fx) >= 0x123 {
        fx.mem.set16(t + 0x54, 7);
    }
}

/// 0x1E02050 / 0x1E02850: an arc at node 2 facing the camera (colours
/// `c`, +6 = 0x8C, radius +0x4C 165, angles 10): alpha +0x67 up by
/// `up.1` below 0xC0 until update `up.0`, then out by 0xC; the rim colour
/// by the update's parity (`rim`).
fn arc(fx: &mut Fx, t: u32, ra: u32, c: ((i32, i32, i32), (i32, i32, i32)), up: (i32, u8), rim: [(i32, i32, i32); 2]) {
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0);
        rgb3(fx, t + 0x60, c.0);
        rgb3(fx, t + 0x64, c.1);
        fx.mem.set16(t + 6, 0x8C);
        fx.mem.write_u32(t + 0x4C, 0x4325_0000);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if counter(fx) < up.0 {
        let a = fx.mem.read_u8(t + 0x67);
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a.wrapping_add(up.1));
        }
    } else {
        fall_at(fx, t, 0x67, 0xE, 0xC);
    }
    let r = if counter(fx) & 1 == 1 { rim[0] } else { rim[1] };
    rgb3(fx, t + 0x64, r);
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES);
    fx.node_position(h, n, t + 0x10);
    angles(fx, t, ra);
    grow(fx, t);
}

/// 0x205580(t + 0x20) from the step (s0 = t, s1 = D) returning to `ra`,
/// its frames' residue left below the step's sp.
fn angles(fx: &mut Fx, t: u32, ra: u32) {
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), ra);
}

/// The target's position (into `p`) pushed out by +0x4C along the angle
/// +0x5C (x by the cosine, z by the sine).
fn about_target(fx: &mut Fx, t: u32, p: u32) {
    let h = fx.target_handle();
    fx.unit_position(h, p);
    let a = fx.mem.i16(t + 0x5C);
    let c = sin(fx, a, 1);
    let r = fx.mem.f(t + 0x4C);
    let x = fx.mem.f(p);
    fx.mem.setf(p, ee::add(x, ee::mul(r, c)));
    let s = sin(fx, a, 0);
    let r = fx.mem.f(t + 0x4C);
    let z = fx.mem.f(p + 8);
    fx.mem.setf(p + 8, ee::add(z, ee::mul(r, s)));
}

/// 0x1E021D0: a homing mote (grey 0x80, size +8 / +0xA = 4 +8, cell
/// (+0x56) rand % 4, its point jittered by (rand % 0xA0 - 0x50) / 10,
/// rise +0x34 = (rand % 40 + 10) / 80, the target's point at radius +0x4C
/// = 5 + rand % 20 and angle +0x5C as its goal, +0x30 / +0x3C the goal's
/// steps over 45 updates): for 0x14 updates it rises and grows (+0x48 by
/// 0.3), to 0x41 it moves to the goal, then circles the target by +0xE a
/// update; alpha up by 0x15 below 0x80 for 0x96 updates (the size steady),
/// then out by 9 (the size +8 down by 4, +0xA up by 0x10).
fn mote(fx: &mut Fx, t: u32, sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 8, 0);
        let v = fx.mem.i16(t + 8) << 2;
        fx.mem.set16(t + 0xA, v);
        fx.mem.write_u32(t + 0x48, 0);
        let v = fx.rand_from(t, 0x01E0_2210);
        let r = fx.div(v >> 4, 0x14).1;
        fx.mem.setf(t + 0x4C, ee::add(5.0, ee::from_i32(r)));
        let v = fx.rand_from(t, 0x01E0_2240);
        fx.mem.set16(t + 0x56, (v >> 4) & 3);
        for (o, ra) in [(0x10u32, 0x01E0_2260u32), (0x18, 0x01E0_229C)] {
            let r = rmod(fx, t, ra, 0xA0);
            let x = fx.mem.f(t + o);
            fx.mem.setf(t + o, ee::add(x, ee::div(ee::from_i32(r - 0x50), 10.0)));
        }
        let v = fx.rand_from(t, 0x01E0_22D8);
        fx.mem.set16(t + 0xE, (v & 0xF) - 8);
        let v = fx.rand_from(t, 0x01E0_22F8);
        let r = fx.div(v >> 4, 0x28).1;
        let f = ee::from_i32(r + 0xA);
        fx.mem.setf(t + 0x34, f);
        fx.mem.setf(t + 0x34, ee::div(f, 80.0));
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        let r = rmod(fx, t, 0x01E0_2358, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rmod(fx, t, 0x01E0_2374, 0x168);
        fx.mem.set16(t + 0x5C, r);
        about_target(fx, t, sp + 0x40);
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES);
        fx.node_position(h, n, sp + 0x50);
        let z = ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(t + 0x18));
        fx.mem.setf(t + 0x3C, ee::div(z, 45.0));
        let x = ee::sub(fx.mem.f(sp + 0x40), fx.mem.f(t + 0x10));
        fx.mem.setf(t + 0x30, ee::div(x, 45.0));
    }
    if age(fx, t) < 0x96 {
        rise(fx, t, 0x80, 0x15);
    } else {
        fall_at(fx, t, 0x63, 0xB, 9);
        let v = fx.mem.i16(t + 8);
        if v != 0 {
            fx.mem.set16(t + 8, v - 4);
        }
        let v = fx.mem.i16(t + 0xA) + 0x10;
        fx.mem.set16(t + 0xA, v);
    }
    let a = age(fx, t);
    if a < 0x14 {
        let y = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
        fx.mem.setf(t + 0x14, y);
        let v = fx.mem.i16(t + 8) + 1;
        fx.mem.set16(t + 8, v);
        let v = fx.mem.i16(t + 8) << 2;
        fx.mem.set16(t + 0xA, v);
        let s = ee::add(fx.mem.f(t + 0x48), f32::from_bits(0x3E99_999A));
        fx.mem.setf(t + 0x48, s);
    } else if a < 0x41 {
        let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x3C));
        fx.mem.setf(t + 0x18, z);
        let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
        fx.mem.setf(t + 0x10, x);
    } else {
        about_target(fx, t, sp + 0x40);
        let x = fx.mem.read_u32(sp + 0x40);
        fx.mem.write_u32(t + 0x10, x);
        let z = fx.mem.read_u32(sp + 0x48);
        fx.mem.write_u32(t + 0x18, z);
        let v = fx.mem.i16(t + 0x5C) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x5C, v);
    }
    angles(fx, t, 0x01E0_25C0);
    grow(fx, t);
    let v = fx.mem.i16(t + 0x5A) + 2;
    fx.mem.set16(t + 0x5A, v);
}

/// 0x1E02600: a smoke ring ((0x20, 0x20, 0x80) / (0x30, 0x30, 0xA0) by
/// the age's parity, size +0x48 = rand % 10 + 10 growing by 1, cell rand
/// % 4, angles +0x20.. rand % 0xFFFF, radius +0x50 = rand % 20 + 10 growing
/// by +0xE = rand % 12 + 10, +0x5A rand % 360, +8 (rand & 7) + 1, life +6
/// = rand % 3 + 2): alpha 0x80 for the life, then out by 0xB.
fn smoke(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let r = rmod(fx, t, 0x01E0_2624, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(r + 0xA));
        let v = fx.rand_from(t, 0x01E0_2650);
        fx.mem.set16(t + 0x56, v & 3);
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x80);
        for (o, ra) in [(0x20u32, 0x01E0_2684u32), (0x24, 0x01E0_26A0), (0x28, 0x01E0_26BC)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        let r = rmod(fx, t, 0x01E0_26D8, 0x14);
        fx.mem.set16(t + 0x50, r + 0xA);
        let r = rmod(fx, t, 0x01E0_26F8, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rmod(fx, t, 0x01E0_2714, 0xC);
        fx.mem.set16(t + 0xE, r + 0xA);
        let v = fx.rand_from(t, 0x01E0_2734);
        fx.mem.set16(t + 8, (v & 7) + 1);
        let r = rmod(fx, t, 0x01E0_2754, 3);
        fx.mem.set16(t + 6, r + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fall_at(fx, t, 0x63, 0xD, 0xB);
    }
    if age(fx, t) & 1 == 1 {
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x80);
    } else {
        l2::rgb(fx, t + 0x60, 0x30, 0x30, 0xA0);
    }
    let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, v);
    let s = ee::add(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, s);
    grow(fx, t);
}

/// 0x1E029D0: a drifting quad (grey 0x40, size 0x23, cell (rand >> 4) % 3
/// + 2, heading (rand >> 4) % 360): alpha up by 0x40 below 0x80 for 10
/// updates, then out by 9; from update 1 it moves 2.2 along its heading.
fn drift(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 8, 0x23);
        let v = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, v);
        let v = fx.rand_from(t, 0x01E0_2A08);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2A40);
        let r = fx.div(v >> 4, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
    }
    if age(fx, t) < 0xA {
        rise(fx, t, 0x80, 0x40);
    } else {
        fall_at(fx, t, 0x63, 0xB, 9);
    }
    if age(fx, t) != 0 {
        let k = f32::from_bits(0x400C_CCCD);
        let a = fx.mem.i16(t + 0x5A);
        let s = sin(fx, a, 0);
        let z = fx.mem.f(t + 0x18);
        fx.mem.setf(t + 0x18, ee::add(z, ee::mul(k, s)));
        let a = fx.mem.i16(t + 0x5A);
        let c = sin(fx, a, 1);
        let x = fx.mem.f(t + 0x10);
        fx.mem.setf(t + 0x10, ee::add(x, ee::mul(k, c)));
    }
    angles(fx, t, 0x01E0_2B20);
    grow(fx, t);
}

/// 0x1E02B50: a ring of quads (grey 0xA0, size +0x48 = rand % 30 + 100
/// growing by 6 to age 0x8C, cell rand % 8, radius +0x50 = rand % 50 + 100
/// growing by +0xE = rand % 20 + 0x1A from update 1 to 0x5A, +0x5A rand %
/// 360, +8 rand % 0x23, life +6 = rand % 14 + 7): alpha up by 0x2A below
/// 0x80 for the life, then out by 9.
fn ring(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let r = rmod(fx, t, 0x01E0_2B74, 0x1E);
        fx.mem.setf(t + 0x48, ee::from_i32(r + 0x64));
        let v = fx.rand_from(t, 0x01E0_2BA0);
        fx.mem.set16(t + 0x56, v & 7);
        l2::rgb(fx, t + 0x60, 0xA0, 0xA0, 0xA0);
        let r = rmod(fx, t, 0x01E0_2BD4, 0x32);
        fx.mem.set16(t + 0x50, r + 0x64);
        let r = rmod(fx, t, 0x01E0_2BF4, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rmod(fx, t, 0x01E0_2C10, 0x14);
        fx.mem.set16(t + 0xE, r + 0x1A);
        let r = rmod(fx, t, 0x01E0_2C30, 0x23);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, 0x01E0_2C4C, 0xE);
        fx.mem.set16(t + 6, r + 7);
        fx.mem.set16(t + 0x58, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        rise(fx, t, 0x80, 0x2A);
    } else {
        fall_at(fx, t, 0x63, 0xB, 9);
    }
    let a = age(fx, t);
    if a < 0x5A && a != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    if age(fx, t) < 0x8C {
        let s = ee::add(fx.mem.f(t + 0x48), 6.0);
        fx.mem.setf(t + 0x48, s);
    }
    grow(fx, t);
}

/// 0x1E02D30: the stage light +0x50 from 0x80: down by 10 to 0x20 while
/// the timer is at least 0x1F, then up by 10 to 0x80 (the task ends there);
/// 0x80 on timer 2.
fn light(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i32(TIMER) >= 0x1F {
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if v < 0x21 { 0x20 } else { v - 0xA });
    } else {
        let v = fx.mem.i16(t + 0x50);
        if v < 0x80 {
            fx.mem.set16(t + 0x50, v + 0xA);
        } else {
            fx.mem.set16(t + 0x50, 0x80);
            fx.mem.set16(t + 0x58, 0);
        }
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    if fx.mem.i32(TIMER) == 2 {
        fx.set_stage_light(0x80);
    } else {
        let v = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(v));
    }
}

/// 0x1E02E20 (frame 0x90): the frame sampled through a window sprite
/// (0x6FF8 .. 0x8FF8, filter 1, ALPHA 0x44, key 0, colour +0x60 = (0x80,
/// 0x80, 0xB0), (0x90, 0x70, 0x80) from update 0xF1) at ST (+0x4C + +0x48
/// + 10, +0x48 + 10) .. (+0x4C + 512 + +0x48 - 10, 512 + +0x48 - 10) / 512;
/// +0x48 = 10 sin(+0x5A) (+10 a update) on 0xA1..0x103 (0 from 0x104),
/// +0x4C = 10 sin(+0x5C) (+20 a update) on 0x105..0x1CB; alpha up by 10
/// below 0x50 to 0xD7, down by 6 to 0x40 from 0xD9, out by 9 from 0x190.
fn window(fx: &mut Fx, t: u32, sp: u32) {
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xB0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let c = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x48 + k, c);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    let (s48, s4c) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let lo = ee::add(s48, 10.0);
    fx.mem.setf(sp + 0x50, ee::div(ee::add(s4c, lo), 512.0));
    let lo = ee::add(fx.mem.f(t + 0x48), 10.0);
    fx.mem.setf(sp + 0x54, ee::div(lo, 512.0));
    let (s48, s4c) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let hi = ee::sub(ee::add(512.0, s48), 10.0);
    fx.mem.setf(sp + 0x60, ee::div(ee::add(s4c, hi), 512.0));
    let hi = ee::sub(ee::add(512.0, fx.mem.f(t + 0x48)), 10.0);
    fx.mem.setf(sp + 0x64, ee::div(hi, 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let c = counter(fx);
    if (0xA1..0x104).contains(&c) {
        let a = fx.mem.i16(t + 0x5A);
        let s = sin(fx, a, 0);
        fx.mem.setf(t + 0x48, ee::mul(10.0, s));
        fx.mem.set16(t + 0x5A, a + 0xA);
    }
    if counter(fx) == 0x104 {
        fx.mem.set16(t + 0x5A, 0);
        let s = sin(fx, 0, 0);
        fx.mem.setf(t + 0x48, ee::mul(10.0, s));
    }
    let c = counter(fx);
    if (0x105..0x1CC).contains(&c) {
        let a = fx.mem.i16(t + 0x5C);
        let s = sin(fx, a, 0);
        fx.mem.setf(t + 0x4C, ee::mul(10.0, s));
        fx.mem.set16(t + 0x5C, a + 0x14);
    }
    if counter(fx) == 0xF1 {
        l2::rgb(fx, t + 0x60, 0x90, 0x70, 0x80);
    }
    let c = counter(fx);
    if c < 0x190 {
        if c < 0xD8 {
            rise(fx, t, 0x50, 0xA);
        }
        if counter(fx) >= 0xD9 {
            let a = fx.mem.read_u8(t + 0x63);
            if a >= 0x41 {
                fx.mem.write_u8(t + 0x63, a - 6);
            }
        }
    } else {
        fall_at(fx, t, 0x63, 0xB, 9);
    }
    grow(fx, t);
}

/// The texture at `tex` and the atlas cell `id` frame +0x56 (0x106960
/// into `r`) copied to D+0x88 / D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, tex: u32, r: u32, id: i32) {
    let v = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, v);
    let f = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, f);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// The entry's draw case for a live task (`sp` the entry's frame).
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    match fx.mem.read_u32(t + 0x44) {
        SPIRAL => {
            cell(fx, t, TEX_F, sp + 0x60, 0x97);
            let v = fx.mem.i16(D + 0x9A) >> 1;
            fx.mem.set16(D + 0x9A, v);
            let v = fx.mem.i16(D + 0x96) + fx.mem.i16(D + 0x9A);
            fx.mem.set16(D + 0x96, v);
            let h = fx.self_handle();
            fx.unit_position(h, sp + 0x50);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            fx.mem.set16(D + 0xBE, 1);
            let (a, b) = hists(fx, t);
            ribbon(fx, t, a, b);
        }
        ARC_W | ARC_D => {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        MOTE => {
            cell(fx, t, TEX5, sp + 0x70, 0x14);
            let f = fx.mem.i16(t + 0x56);
            fx.mem.set16(D + 0x94, f << 5);
            for (o, v) in [(0x96u32, 0), (0x98, 0x20), (0x9A, 0x80), (0xBE, 0)] {
                fx.mem.set16(D + o, v);
            }
            let a = fx.mem.i16(t + 0x5A);
            let s = sin(fx, a + 0xB4, 0);
            let r = fx.mem.f(t + 0x48);
            let x = fx.mem.f(t + 0x10);
            fx.mem.setf(sp + 0x50, ee::add(x, ee::mul(2.0, ee::mul(r, s))));
            let a = fx.mem.i16(t + 0x5A);
            let s = sin(fx, a, 0);
            let r = fx.mem.f(t + 0x48);
            let y = fx.mem.f(t + 0x14);
            fx.mem.setf(sp + 0x54, ee::add(y, ee::mul(r, s)));
            let z = fx.mem.read_u32(t + 0x18);
            fx.mem.write_u32(sp + 0x58, z);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
        }
        SMOKE => {
            cell(fx, t, TEX_F, sp + 0x80, 0x9B);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        DRIFT => {
            cell(fx, t, TEX5 + 4, sp + 0x90, 0x14);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        RING => {
            cell(fx, t, TEX5 + 4, sp + 0xA0, 0x14);
            fx.mem.set16(D + 0xB8, 5);
            let v = fx.rand_from(D, 0x01E0_3980);
            fx.mem.set16(D + 0xBE, v & 3);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x10, BYTES);
            fx.mem.set16(D + 0xB8, 0);
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

/// 0x108020(local, c) and the 8 bytes it builds copied to `to`.
fn rgbaq_to(fx: &mut Fx, local: u32, c: u32, to: u32) {
    l2::rgbaq(fx, local, c);
    for k in 0..2 {
        let w = fx.mem.read_u32(local + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The Lagrange cubic through `p` (four points' x, y, z) at `t` (p1 at 0,
/// p2 at 1), summed as the original's accumulator chain: ((p0 w0 + p1 w1)
/// + p2 w2) + p3 w3.
fn lagrange(p: [[f32; 3]; 4], t: f32) -> [f32; 3] {
    use ee::{add, div, mul, neg, sub};
    let (tm1, tm2, tp1) = (sub(t, 1.0), sub(t, 2.0), add(1.0, t));
    let w0 = div(mul(neg(t), mul(tm1, tm2)), 6.0);
    let w3 = div(mul(tp1, mul(t, tm1)), 6.0);
    let w1 = div(mul(tm2, mul(tp1, tm1)), 2.0);
    let w2 = div(mul(neg(tp1), mul(t, tm2)), 2.0);
    std::array::from_fn(|c| {
        let f6 = add(mul(p[0][c], w0), mul(p[1][c], w1));
        let acc = add(mul(p[2][c], w2), f6);
        add(acc, mul(p[3][c], w3))
    })
}

fn points(fx: &mut Fx, at: [u32; 4]) -> [[f32; 3]; 4] {
    std::array::from_fn(|i| std::array::from_fn(|c| fx.mem.f(at[i] + 4 * c as u32)))
}

/// 0x1E03C40(D, task, a, b): a textured strip3 ribbon between the
/// histories `a` / `b` (n = +6 points) through D+0x20 (0x1097D0 into sp +
/// 0x230), each segment s two pieces (t 0 .. 0.5 .. 1 of the Lagrange
/// cubic through points s - 1 .. s + 2; the first segment through p0 p0
/// p1 p2); UVs D+0x94 in order 0, texture D+0x88; the near pair coloured
/// +0x60 / +0x64 with +0x60's alpha, the far pair with it less +0x63 / 2n
/// (clamped at 0); all four at the far Z, key 0 into D+0x80 [F].
fn ribbon(fx: &mut Fx, t: u32, a: u32, b: u32) {
    let sp = fx.enter(0x390);
    let n = fx.mem.i16(t + 6);
    for k in 0..3 {
        let w = fx.mem.read_u32(D + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0x160 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x230, 0, 0, sp + 0x160);
    let d = sp + 0x2D8;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 0);
    l2::uv_strip3(fx, d, D + 0x94, 0);
    let rh2 = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, rh2, 1);
    let (ca, cb) = (sp + 0x388, sp + 0x38C);
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(ca + k, v);
        let v = fx.mem.read_u8(t + 0x64 + k);
        fx.mem.write_u8(cb + k, v);
    }
    let fp = 2 * n;
    copy4(fx, sp + 0x2B0, a);
    for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
        let from = if k < 2 { sp + 0x2B0 } else { a + o };
        copy4(fx, sp + 0x1B0 + 0x10 * k as u32, from);
    }
    copy4(fx, sp + 0x2C0, b);
    for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
        let from = if k < 2 { sp + 0x2C0 } else { b + o };
        copy4(fx, sp + 0x1F0 + 0x10 * k as u32, from);
    }
    // The first segment's a-side points kept at sp + 0xD4.. (x, y, z rows).
    for (to, from) in [(0xD4u32, 0x1C0u32), (0xD8, 0x1D0), (0xDC, 0x1E0), (0xE0, 0x1C4), (0xE4, 0x1B4), (0xE8, 0x1D4), (0xEC, 0x1E4), (0xF0, 0x1C8), (0xF4, 0x1B8), (0xF8, 0x1D8), (0xFC, 0x1E8)] {
        let w = fx.mem.read_u32(sp + from);
        fx.mem.write_u32(sp + to, w);
    }
    fx.mem.set32(sp + 0x100, n - 1);
    let mut s0 = 0i32;
    while s0 < fx.mem.i32(sp + 0x100) {
        let (pa, pb) = if s0 != 0 {
            let qa = a.wrapping_add((0x10 * s0) as u32);
            let qb = b.wrapping_add((0x10 * s0) as u32);
            ([qa.wrapping_sub(0x10), qa, qa + 0x10, qa + 0x20], [qb.wrapping_sub(0x10), qb, qb + 0x10, qb + 0x20])
        } else {
            let pa = sp + 0x1B0;
            let pb = sp + 0x1F0;
            ([pa, pa + 0x10, pa + 0x20, pa + 0x30], [pb, pb + 0x10, pb + 0x20, pb + 0x30])
        };
        let mut f20 = 0.0f32;
        for _ in 0..2 {
            let a3 = fx.mem.read_u8(ca + 3);
            fx.mem.write_u8(cb + 3, a3);
            rgbaq_to(fx, sp + 0x368, ca, d + 0x10);
            rgbaq_to(fx, sp + 0x370, cb, d + 0x40);
            let al = i32::from(fx.mem.read_u8(t + 0x63));
            let q = fx.div(al, fp).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(ca + 3)) - q;
            fx.mem.write_u8(ca + 3, if v > 0 { v as u8 } else { 0 });
            let a3 = fx.mem.read_u8(ca + 3);
            fx.mem.write_u8(cb + 3, a3);
            rgbaq_to(fx, sp + 0x378, ca, d + 0x28);
            rgbaq_to(fx, sp + 0x380, cb, d + 0x58);
            for i in 0..2u32 {
                let pts = points(fx, pa);
                let v = lagrange(pts, f20);
                let (out, keep) = if s0 != 0 { (sp + 0x270, sp + 0x170) } else { (sp + 0x290, sp + 0x170) };
                for (c, x) in v.into_iter().enumerate() {
                    fx.mem.setf(out + 4 * c as u32, x);
                }
                copy4(fx, keep + 0x10 * i, out);
                let pts = points(fx, pb);
                let v = lagrange(pts, f20);
                let out = if s0 != 0 { sp + 0x280 } else { sp + 0x2A0 };
                for (c, x) in v.into_iter().enumerate() {
                    fx.mem.setf(out + 4 * c as u32, x);
                }
                copy4(fx, sp + 0x190 + 0x10 * i, out);
                if i == 0 {
                    f20 = ee::add(f20, 0.5);
                }
                if f20 > 9.0 {
                    f20 = 1.0;
                }
            }
            for (to, from) in [(0x120u32, 0x170u32), (0x130, 0x180), (0x140, 0x190), (0x150, 0x1A0)] {
                for k in 0..3 {
                    let w = fx.mem.read_u32(sp + from + 4 * k);
                    fx.mem.write_u32(sp + to + 4 * k, w);
                }
            }
            if l2::project(fx, sp + 0x120, 0x7000_0000, sp + 0x230, d, sp + 0x110) {
                l2::far_z_desc(fx, d, 0);
                let ot = fx.mem.read_u32(D + 0x80);
                fx.submit(d, ot, 0);
            }
        }
        s0 += 1;
    }
    fx.leave(0x390);
}

/// 0x1E01580(D, 0) (frame 0x40: ra +0x30, s2 +0x20 the parameter, s1
/// +0x10 = 0, s0 = D): the update counter and the spawns (its s2 = D, s1 /
/// s0 the loop bound / counter; the spawn's s4 / s3 the runner's (0) [H]).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_3570), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    let (mut s0, mut s1) = (u64::from(D), 0u64);
    let sp_ = |fx: &mut Fx, f: u32, ra: u32, s0: u64, s1: u64| {
        spawn::task70(fx, D, spawn::pool70_b4("ae03cf745317").unwrap(), f, 0, &[(0, s0), (0x10, s1), (0x20, u64::from(D)), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
    };
    if counter(fx) == 0xBE {
        for (f, ra) in [(ARC_W, 0x01E0_15D8u32), (ARC_W, 0x01E0_1604), (WINDOW, 0x01E0_1630), (ARC_D, 0x01E0_165C), (ARC_D, 0x01E0_1688), (LIGHT, 0x01E0_16B4)] {
            sp_(fx, f, ra, s0, s1);
        }
    }
    let c = counter(fx);
    if (0xA6..0x123).contains(&c) && fx.div(c, 3).1 == 1 {
        s1 = 2;
        for k in 0..2u64 {
            s0 = k;
            self_node(fx, NODES, POS);
            let v = fx.mem.i16(NEXT);
            fx.mem.set16(SLOT, v);
            sp_(fx, SPIRAL, 0x01E0_176C, s0, s1);
            let v = fx.mem.i16(NEXT) + 1;
            fx.mem.set16(NEXT, if v >= 0x28 { 0 } else { v });
        }
        s0 = 2;
        sp_(fx, RING, 0x01E0_17E8, s0, s1);
    }
    let c = counter(fx);
    if (0xC4..0x105).contains(&c) && fx.div(c, 0xA).1 == 1 {
        for k in 0..2u64 {
            s0 = k;
            self_node(fx, NODES, POS);
            sp_(fx, MOTE, 0x01E0_1868, s0, s1);
        }
        s0 = 2;
    }
    let c = counter(fx);
    if (0xC4..0x105).contains(&c) {
        sp_(fx, SMOKE, 0x01E0_18C8, s0, s1);
    }
    if counter(fx) == 0xBC {
        for k in 0..0x14u64 {
            s0 = k;
            self_node(fx, NODES, POS);
            sp_(fx, MOTE, 0x01E0_191C, s0, s1);
            sp_(fx, SMOKE, 0x01E0_1930, s0, s1);
        }
    }
    if matches!(counter(fx), 0x83 | 0x8D | 0x97 | 0xA1) {
        for k in 0..0x14u64 {
            s0 = k;
            self_node(fx, NODES + 6, POS);
            sp_(fx, DRIFT, 0x01E0_19D4, s0, s1);
        }
        for k in 0..0x14u64 {
            s0 = k;
            self_node(fx, NODES + 8, POS);
            sp_(fx, DRIFT, 0x01E0_1A30, s0, s1);
        }
    }
    fx.leave(0x40);
}

/// The attacker's node (slot at `node`) into `out`.
fn self_node(fx: &mut Fx, node: u32, out: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(node);
    fx.node_position(h, n, out);
}

