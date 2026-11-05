//! BMODULE b485 (card 74), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02C00, data area 0x1ED0000, 0x4870 bytes) [F]
//!: b272's task layout (150 tasks
//! of 0x70 at D+0x2A0, alive +0x58, step +0x44 through `jalr`; spawned by
//! 0x1E03330 = b272's 0x1E019E0 with the colour D+0x4494 and the angle
//! D+0x44F2) on b191's steps (offsets re-read) and b092's unit flash
//! (0x1E03470: the attacker drawn into the target in a packet layer, the
//! flash tasks 0x1E029E0 stepped there). The emitter 0x1E017C0 counts
//! D+0x44FA: on 1 the flash and the stage light 0x1E025B0; 0x45..0xE2 a
//! sparkle 0x1E023F0 a update at the attacker's node 0x21 (2 lower); on
//! the odd updates 0xF5..0x10E two homing motes 0x1E01BC0 at node 1; on
//! 0xFA twenty motes and twenty smoke rings 0x1E02040 there; on 0xFC two
//! white arcs 0x1E01A60, the frame window 0x1E02690 and two dark arcs
//! 0x1E02290 at the stale D+0x250. Returns 1 on update 0xFC, 4 on 0x181,
//! 2 on 0x177, 6 on 0x107..0x171, 8 once the 400-update timer D+0x4490
//! has run out.
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
const TIMER: u32 = D + 0x4490;
const COLOUR: u32 = D + 0x4494;
const LAYER: u32 = D + 0x44A4;
const FLASH_DESC: u32 = D + 0x44A8;
const NODE_A: u32 = D + 0x44C4;
const NODE_B: u32 = D + 0x44C6;
const ANGLE_FLAG: u32 = D + 0x44F2;
const COUNTER: u32 = D + 0x44FA;
const BYTES: u32 = D + 0x44FC;
const BYTES_B: u32 = D + 0x45FC;
const SINES: u32 = D + 0x46FC;
const TEX5: u32 = D + 0x4470;
const TEX_F: u32 = D + 0x29C;

const ARC_W: u32 = 0x01E0_1A60;
const MOTE: u32 = 0x01E0_1BC0;
const SMOKE: u32 = 0x01E0_2040;
const ARC_D: u32 = 0x01E0_2290;
const SPARKLE: u32 = 0x01E0_23F0;
const LIGHT: u32 = 0x01E0_25B0;
const WINDOW: u32 = 0x01E0_2690;
const FLASH: u32 = 0x01E0_29E0;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_2F80;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02C00 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0x449C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4870);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x44F8, param);
    fx.mem.set32(TIMER, 0x190);
    for (k, (n, clear)) in [(0x21, 1), (1, 0), (0xD, 0), (0x56, 0), (0x60, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODE_A + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x44D4, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(TEX_F, t);
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX5 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_2D84);
    fx.mem.set16(D + 0x44F6, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frame: s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2DAC, 0x1E);
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_2DFC);
        fx.mem.write_u8(BYTES + k, (v & 3) as u8);
    }
    for k in 0xB4..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_2E54);
        fx.mem.write_u8(BYTES + k, ((v & 0xF) - 8) as u8);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2EA4, 0x28);
        fx.mem.write_u8(BYTES_B + k, (v - 0x14) as u8);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2EEC, 0x14);
        fx.mem.write_u8(BYTES_B + k, v as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    crate::b369::unit_flag(fx, 1, u64::from(D), 0x01E0_2F34);
    emit(fx, param);
    // The entry's s1: the flash tasks counted (not run) here.
    let mut flashes = 0u64;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        if f == FLASH {
            flashes += 1;
            continue;
        }
        // The step's prologue: ra, the entry's s2 = 0, s1 = the flashes,
        // s0 = D.
        let (ra, d) = (TASK_RA, u64::from(D));
        let (frame, saves): (u32, &[(u32, u64)]) = match f {
            ARC_W | ARC_D => (0x30, &[(0x20, ra), (0x10, flashes), (0, d)]),
            WINDOW => (0x90, &[(0x20, ra), (0x10, flashes), (0, d)]),
            MOTE => (0x60, &[(0x30, ra), (0x20, 0), (0x10, flashes), (0, d)]),
            SMOKE | SPARKLE => (0x20, &[(0x10, ra), (0, d)]),
            LIGHT => (0x10, &[(0, ra)]),
            _ => panic!("b485: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        fx.saved(s, saves);
        match f {
            ARC_W => arc(fx, t, 0x01E0_1B98, ((0xC0, 0xC0, 0xC0), (0x80, 0x80, 0x60)), (0x122, 0x18), [(0x50, 0x50, 0x80), (0x60, 0x60, 0xA0)]),
            ARC_D => arc(fx, t, 0x01E0_23C8, ((0x20, 0x20, 0x20), (0x80, 0x80, 0x60)), (0xA, 0x60), [(0x20, 0x20, 0x80), (0x30, 0x30, 0xA0)]),
            MOTE => mote(fx, t, s),
            SMOKE => smoke(fx, t),
            SPARKLE => sparkle(fx, t, flashes),
            LIGHT => light(fx, t),
            WINDOW => window(fx, t, s),
            _ => {}
        }
        fx.leave(frame);
    }
    // a3 as the draws' last library call left it, for the unit flash's
    // first TEXFLUSH.
    let mut a3 = 0u32;
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) != 0 {
            live += 1;
            draw(fx, sp, t, &mut a3);
        }
    }
    if flashes != 0 {
        unit_flash(fx, a3, flashes as u32, live);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0xFC => 1,
        0x181 => 4,
        0x177 => 2,
        0x107..=0x171 => 6,
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

/// sin (cos 0) / cos of the i16 degrees `deg` on D+0x46FC.
fn sin(fx: &mut Fx, deg: i32, cos: i32) -> f32 {
    l2::sin_deg(fx, SINES, deg as i16 as i32, cos)
}

/// 0x205580(t + 0x20) from a step (s0 = t, s1 = `s1`) returning to `ra`.
fn angles(fx: &mut Fx, t: u32, s1: u64, ra: u32) {
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), s1, ra);
}

/// 0x1E01A60 / 0x1E02290: b191's arc at the attacker's node 0x21 facing
/// the camera (colours `c`, +6 = 0x8C, radius +0x4C 165, angles 10): alpha
/// +0x67 up by `up.1` below 0xC0 until update `up.0`, then out by 0xC; the
/// rim colour by the update's parity (`rim`).
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
    let n = fx.mem.i16(NODE_A);
    fx.node_position(h, n, t + 0x10);
    angles(fx, t, u64::from(D), ra);
    grow(fx, t);
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

/// 0x1E01BC0: a homing mote (grey 0x80, size +8 / +0xA = 4 +8, cell
/// (+0x56) rand % 4, its point jittered by (rand % 0xA0 - 0x50) / 10,
/// rise +0x34 = (rand % 40 - 20) / 80, the target's point at radius +0x4C
/// = 5 + rand % 20 and angle +0x5C as its goal, +0x30 / +0x3C the goal's
/// steps over 30 updates, life +6 = rand % 40 + 0x46): for 0x14 updates it
/// rises, grows (+0x48 by 0.3) and moves, to 0x1E it moves to the goal,
/// then circles the target by +0xE a update, growing on odd ages; alpha up
/// by 0x15 below 0x80 for the life, then out by 9.
fn mote(fx: &mut Fx, t: u32, sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 8, 0);
        let v = fx.mem.i16(t + 8) << 2;
        fx.mem.set16(t + 0xA, v);
        fx.mem.write_u32(t + 0x48, 0);
        let v = fx.rand_from(t, 0x01E0_1C00);
        let r = fx.div(v >> 4, 0x14).1;
        fx.mem.setf(t + 0x4C, ee::add(5.0, ee::from_i32(r)));
        let v = fx.rand_from(t, 0x01E0_1C30);
        fx.mem.set16(t + 0x56, (v >> 4) & 3);
        for (o, ra) in [(0x10u32, 0x01E0_1C50u32), (0x18, 0x01E0_1C8C)] {
            let r = rmod(fx, t, ra, 0xA0);
            let x = fx.mem.f(t + o);
            fx.mem.setf(t + o, ee::add(x, ee::div(ee::from_i32(r - 0x50), 10.0)));
        }
        let v = fx.rand_from(t, 0x01E0_1CC8);
        fx.mem.set16(t + 0xE, (v & 0xF) - 8);
        let v = fx.rand_from(t, 0x01E0_1CE8);
        let r = fx.div(v >> 4, 0x28).1;
        let f = ee::from_i32(r - 0x14);
        fx.mem.setf(t + 0x34, f);
        fx.mem.setf(t + 0x34, ee::div(f, 80.0));
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        let r = rmod(fx, t, 0x01E0_1D48, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rmod(fx, t, 0x01E0_1D64, 0x168);
        fx.mem.set16(t + 0x5C, r);
        about_target(fx, t, sp + 0x40);
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE_A);
        fx.node_position(h, n, sp + 0x50);
        let z = ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(t + 0x18));
        fx.mem.setf(t + 0x3C, ee::div(z, 30.0));
        let x = ee::sub(fx.mem.f(sp + 0x40), fx.mem.f(t + 0x10));
        fx.mem.setf(t + 0x30, ee::div(x, 30.0));
        let v = fx.rand_from(t, 0x01E0_1E28);
        let r = fx.div(v >> 4, 0x28).1;
        fx.mem.set16(t + 6, r + 0x46);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        rise(fx, t, 0x80, 0x15);
    } else {
        fall_at(fx, t, 0x63, 0xB, 9);
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
        step_to_goal(fx, t);
    } else if a < 0x1E {
        step_to_goal(fx, t);
    } else {
        about_target(fx, t, sp + 0x40);
        let x = fx.mem.read_u32(sp + 0x40);
        fx.mem.write_u32(t + 0x10, x);
        let z = fx.mem.read_u32(sp + 0x48);
        fx.mem.write_u32(t + 0x18, z);
        let v = fx.mem.i16(t + 0x5C) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x5C, v);
        if age(fx, t) & 1 == 1 {
            let v = fx.mem.i16(t + 8) + 1;
            fx.mem.set16(t + 8, v);
            let v = fx.mem.i16(t + 8) << 2;
            fx.mem.set16(t + 0xA, v);
        }
    }
    angles(fx, t, u64::from(D), 0x01E0_2008);
    grow(fx, t);
    let v = fx.mem.i16(t + 0x5A) + 2;
    fx.mem.set16(t + 0x5A, v);
}

fn step_to_goal(fx: &mut Fx, t: u32) {
    let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x3C));
    fx.mem.setf(t + 0x18, z);
    let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
    fx.mem.setf(t + 0x10, x);
}

/// 0x1E02040: b191's smoke ring ((0x20, 0x20, 0x80) / (0x30, 0x30, 0xA0)
/// by the age's parity, size +0x48 = rand % 10 + 10 growing by 1, cell
/// rand % 4, angles +0x20.. rand % 0xFFFF, radius +0x50 = rand % 20 + 10
/// growing by +0xE = rand % 12 + 10, +0x5A rand % 360, +8 (rand & 7) + 1,
/// life +6 = rand % 3 + 2): alpha 0x80 for the life, then out by 0xB.
fn smoke(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let r = rmod(fx, t, 0x01E0_2064, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(r + 0xA));
        let v = fx.rand_from(t, 0x01E0_2090);
        fx.mem.set16(t + 0x56, v & 3);
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x80);
        for (o, ra) in [(0x20u32, 0x01E0_20C4u32), (0x24, 0x01E0_20E0), (0x28, 0x01E0_20FC)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        let r = rmod(fx, t, 0x01E0_2118, 0x14);
        fx.mem.set16(t + 0x50, r + 0xA);
        let r = rmod(fx, t, 0x01E0_2138, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rmod(fx, t, 0x01E0_2154, 0xC);
        fx.mem.set16(t + 0xE, r + 0xA);
        let v = fx.rand_from(t, 0x01E0_2174);
        fx.mem.set16(t + 8, (v & 7) + 1);
        let r = rmod(fx, t, 0x01E0_2194, 3);
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

/// 0x1E023F0: a rising sparkle (blue (0x30, 0x30, 0x80), size 0x14 x 90,
/// cell (rand >> 4) % 3 + 2 cycling to 8, life +6 = (rand >> 4) % 5 + 5,
/// rise +0x34 = (rand % 80 + 20) / 100 from update 1): alpha up by 0x10
/// below 0x80 for the life, then out by 9. `s1` the entry's (the
/// flashes), live at its 0x205580.
fn sparkle(fx: &mut Fx, t: u32, s1: u64) {
    if first(fx, t) {
        fx.mem.set16(t + 8, 0x14);
        let v = ee::mul(4.5, ee::from_i32(fx.mem.i16(t + 8)));
        fx.fix_residue(v, 0x01E0_2434);
        fx.mem.set16(t + 0xA, ee::fixsfsi(v));
        let v = fx.rand_from(t, 0x01E0_243C);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0x30, 0x30, 0x80);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2474);
        let r = fx.div(v >> 4, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        let v = fx.rand_from(t, 0x01E0_2494);
        let r = fx.div(v >> 4, 5).1;
        fx.mem.set16(t + 6, r + 5);
        let r = rmod(fx, t, 0x01E0_24B8, 0x50);
        let f = ee::from_i32(r + 0x14);
        fx.mem.setf(t + 0x34, f);
        fx.mem.setf(t + 0x34, ee::div(f, 100.0));
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        rise(fx, t, 0x80, 0x10);
    } else {
        fall_at(fx, t, 0x63, 0xB, 9);
    }
    if age(fx, t) != 0 {
        let y = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
        fx.mem.setf(t + 0x14, y);
    }
    let v = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, v);
    if fx.mem.i16(t + 0x56) >= 8 {
        fx.mem.set16(t + 0x56, 0);
    }
    angles(fx, t, s1, 0x01E0_2584);
    grow(fx, t);
}

/// 0x1E025B0: the stage light +0x50 from 0x80: down by 10 to 0x20 while
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

/// 0x1E02690 (frame 0x90): b191's frame window (0x6FF8 .. 0x8FF8, filter
/// 1, ALPHA 0x44, key 0, colour +0x60 = (0x80, 0x80, 0xB0), (0x90, 0x70,
/// 0x80) on update 0x10E) at ST (+0x4C + +0x48 + 10, +0x48 + 10) .. (+0x4C
/// + 512 + +0x48 - 10, 512 + +0x48 - 10) / 512; +0x48 = 10 sin(+0x5A) (+10
/// a update) on 0xA1..0x11C (0 on 0x11D), +0x4C = 10 sin(+0x5C) (+20 a
/// update) on 0x11E..0x1CB; while the timer is at least 0x1F alpha up by
/// 10 below 0x50 before 0x10E and down by 7 to 0x39 from 0x137, then out
/// by 9.
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
    if (0xA1..0x11D).contains(&c) {
        let a = fx.mem.i16(t + 0x5A);
        let s = sin(fx, a, 0);
        fx.mem.setf(t + 0x48, ee::mul(10.0, s));
        fx.mem.set16(t + 0x5A, a + 0xA);
    }
    if counter(fx) == 0x11D {
        fx.mem.set16(t + 0x5A, 0);
        let s = sin(fx, 0, 0);
        fx.mem.setf(t + 0x48, ee::mul(10.0, s));
    }
    let c = counter(fx);
    if (0x11E..0x1CC).contains(&c) {
        let a = fx.mem.i16(t + 0x5C);
        let s = sin(fx, a, 0);
        fx.mem.setf(t + 0x4C, ee::mul(10.0, s));
        fx.mem.set16(t + 0x5C, a + 0x14);
    }
    if counter(fx) == 0x10E {
        l2::rgb(fx, t + 0x60, 0x90, 0x70, 0x80);
    }
    if fx.mem.i32(TIMER) >= 0x1F {
        if counter(fx) < 0x10E {
            let a = fx.mem.read_u8(t + 0x63);
            if a < 0x50 {
                fx.mem.write_u8(t + 0x63, a + 0xA);
            }
        }
        if counter(fx) >= 0x137 {
            let a = fx.mem.read_u8(t + 0x63);
            if a >= 0x39 {
                fx.mem.write_u8(t + 0x63, a - 7);
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

/// The low word of the last XYZ2 of the last primitive submitted since
/// `before` (a3 after a draw at depth mode 1), else `old`.
fn last_xyz2(fx: &mut Fx, before: usize, old: u32) -> u32 {
    match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => old,
    }
}

/// The entry's draw case for a live task (`sp` the entry's frame).
fn draw(fx: &mut Fx, sp: u32, t: u32, a3: &mut u32) {
    match fx.mem.read_u32(t + 0x44) {
        ARC_W | ARC_D => {
            fx.mem.set16(D + 0xB8, 0x24);
            let before = fx.out.prims.len();
            billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
            *a3 = last_xyz2(fx, before, *a3);
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
            fx.mem.setf(sp + 0x60, ee::add(x, ee::mul(2.0, ee::mul(r, s))));
            let a = fx.mem.i16(t + 0x5A);
            let s = sin(fx, a, 0);
            let r = fx.mem.f(t + 0x48);
            let y = fx.mem.f(t + 0x14);
            fx.mem.setf(sp + 0x64, ee::add(y, ee::mul(r, s)));
            let z = fx.mem.read_u32(t + 0x18);
            fx.mem.write_u32(sp + 0x68, z);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
            *a3 = 0;
        }
        SMOKE => {
            cell(fx, t, TEX_F, sp + 0x80, 0x9B);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            *a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        SPARKLE => {
            cell(fx, t, TEX5 + 8, sp + 0x90, 0x14);
            let f = fx.mem.i16(t + 0x56);
            fx.mem.set16(D + 0x94, f << 5);
            for (o, v) in [(0x96u32, 1), (0x98, 0x20), (0x9A, 0x40)] {
                fx.mem.set16(D + o, v);
            }
            billboard::quad_standing(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
            *a3 = 0;
        }
        _ => {}
    }
}

/// 0x1E03330(D, step, 0): the first free task cleared and set off at
/// D+0x250 with the colour D+0x4494 (twice), angle flag D+0x44F2; `regs`
/// the caller's s0..s4, `ra` the return address.
fn spawn(fx: &mut Fx, f: u32, regs: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, regs[4]), (0x30, regs[3]), (0x20, regs[2]), (0x10, regs[1]), (0, regs[0])]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            // 0x107390's `sq ra` upper half (0) (b232's).
            fx.mem.write_u64(sp - 8, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(ANGLE_FLAG);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// The attacker's node (slot at `node`) into D+0x250.
fn self_node(fx: &mut Fx, node: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(node);
    fx.node_position(h, n, D + 0x250);
}

/// 0x1E017C0(D, 0) (frame 0x30: ra, the entry's s1 = 0, s0 = D): the
/// update counter and the spawns (its s1 the loop counter; the spawn's s4
/// the runner's (0) [H], s3 the entry's = the parameter, s2 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_2F40), (0x10, 0), (0, u64::from(D))]);
    let p = param as i64 as u64;
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    let mut s1 = 0u64;
    let regs = |s1: u64| [u64::from(D), s1, 0, p, 0];
    if counter(fx) == 0xFC {
        for (f, ra) in [(ARC_W, 0x01E0_17FCu32), (ARC_W, 0x01E0_1820), (WINDOW, 0x01E0_1844), (ARC_D, 0x01E0_1868), (ARC_D, 0x01E0_188C)] {
            spawn(fx, f, regs(s1), ra);
        }
    }
    if counter(fx) == 1 {
        spawn(fx, FLASH, regs(s1), 0x01E0_18B0);
        spawn(fx, LIGHT, regs(s1), 0x01E0_18D4);
    }
    let c = counter(fx);
    if (0xF5..0x10F).contains(&c) && c % 2 == 1 {
        for k in 0..2u64 {
            s1 = k;
            self_node(fx, NODE_B);
            spawn(fx, MOTE, regs(s1), 0x01E0_1944);
        }
        s1 = 2;
    }
    let c = counter(fx);
    if (0x45..0xE3).contains(&c) {
        self_node(fx, NODE_A);
        let y = ee::sub(fx.mem.f(D + 0x254), 2.0);
        fx.mem.setf(D + 0x254, y);
        spawn(fx, SPARKLE, regs(s1), 0x01E0_19C4);
    }
    if counter(fx) == 0xFA {
        for k in 0..0x14u64 {
            s1 = k;
            self_node(fx, NODE_B);
            spawn(fx, MOTE, regs(s1), 0x01E0_1A08);
            spawn(fx, SMOKE, regs(s1), 0x01E0_1A1C);
        }
    }
    fx.leave(0x30);
}

/// The flash descriptor: colour +0x60.., Q 1, the window inset by +8 / 10
/// and +0xA / 10 over 512, submitted with no ordering table.
fn flash_quad(fx: &mut Fx, t: u32) {
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, xy) in [(d + 0x20, 0x6FF8u16), (d + 0x30, 0x8FF8)] {
        fx.mem.write_u16(at, xy);
        fx.mem.write_u16(at + 2, xy);
        let v = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(at + 7, 0);
    }
    fx.mem.setf(d + 0x18, ee::div(w, 512.0));
    fx.mem.setf(d + 0x1C, ee::div(h, 512.0));
    fx.mem.setf(d + 0x28, ee::div(ee::sub(512.0, w), 512.0));
    fx.mem.setf(d + 0x2C, ee::div(ee::sub(512.0, h), 512.0));
    fx.submit(d, 0, 0);
}

/// 0x1E029E0(D, t): the flash step in grey 0x60: alpha up by 9 below 0x80
/// until update 0x12C, then down by 7 (the task ends under 9). `regs` the
/// unit flash's s0 / s1 (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_3544), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f21 / f20: the runner's (0) [H].
    fx.mem.write_u32(sp + 4, 0);
    fx.mem.write_u32(sp, 0);
    if first(fx, t) {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x60);
        fx.mem.set16(t + 0x58, 2);
    }
    flash_quad(fx, t);
    if counter(fx) < 0x12C {
        rise(fx, t, 0x80, 9);
    } else {
        fall_at(fx, t, 0x63, 9, 7);
    }
    grow(fx, t);
    fx.leave(0x40);
}

/// 0x1E03470(D): b092's unit flash: in a packet layer (0x2078A0) the
/// attacker drawn into the target (0x1F8270 filter 0; ALPHA 0x48), TEXFLUSH
/// before and after; every flash task stepped; the layer linked into the
/// ordering table at key 0 when any task was alive. Its saves: the
/// entry's s0 = D, s1 = the flashes, s2 = the live count, s3 = D + 0x41A0.
fn unit_flash(fx: &mut Fx, a3: u32, flashes: u32, live: u32) {
    let sp = fx.enter(0xB0);
    fx.saved(sp, &[(0x40, 0x01E0_327C), (0x30, u64::from(D + 0x70 * TASKS)), (0x20, u64::from(live)), (0x10, u64::from(flashes)), (0, u64::from(D))]);
    let d = sp + 0x58;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x98, sp + 0xA0);
    fx.texflush(regs);
    let mut alive = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        alive += 1;
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            flash(fx, slot + 0x2A0, [k, alive]);
        }
    }
    if alive != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}
