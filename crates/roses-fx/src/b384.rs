//! BMODULE b384 (card 341), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03020, data area 0x1ED0000) [F]: b279's pool and layout (150 x 0x70 tasks at D+0x2A0, alive
//! +0x58, step +0x44 through `jalr`, 0x1E03010 only returns; spawned by
//! 0x1E03790 = `spawn::task70` with the colour D+0x44A4 and the slot
//! D+0x4506), a 210-update timer D+0x44A0, the attacker's nodes 0x33 /
//! 0x3A / 1, the target's node 1 and textures (4, 0xA..0xF), (4, 0), (5,
//! 0). The emitter 0x1E01710 counts D+0x450E: to update 0x75 the point
//! D+0x4470 follows node 1 (x) / 0x33 (y, z), drifting along z by a
//! growing D+0x44C4 (- when the attacker's yaw is non-zero) and bobbing by
//! 3 sin(D+0x44FC) to 0x66 (then rising 3 a update); 0x76..0xA0 it drifts
//! 4 along z and sinks 0.3. On 0xF a growing ring (0x1E02800) at the
//! point; 0x2D..0x5A a shrinking arc of quads (0x1E02A60) at the point,
//! 0x76..0xA0 a widening one (0x1E02D40) there; 0x41..0xA0 a band
//! (0x1E023F0) at node 0x33; on 0xA0 a closing ring (0x1E02630), seven
//! spinning arcs of quads (0x1E02130) and six wide rings (0x1E01F90 =
//! b149's 0x1E01AC0 with size 5 and growth 6..15). Returns 1 on update
//! 0x96, 4 on 0xAA, 2 on 0xA0, 8 once the timer has run out.
//!
//! The step 0x1E01B60 is tested by the entry but never spawned [F:
//! spawners.py]; not ported.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::spawn::{self, Pool70};
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const WIDE: u32 = 0x01E0_1F90;
const SPIN: u32 = 0x01E0_2130;
const BAND: u32 = 0x01E0_23F0;
const CLOSE: u32 = 0x01E0_2630;
const GROW: u32 = 0x01E0_2800;
const ARC_IN: u32 = 0x01E0_2A60;
const ARC_OUT: u32 = 0x01E0_2D40;
const IDLE: u32 = 0x01E0_3010;
const UNSPAWNED: [u32; 1] = [0x01E0_1B60];
/// The return address of the entry's `jalr` into a step (0x1E03230 + 8).
const TASK_RA: u64 = 0x01E0_3238;
const POS: u32 = D + 0x250;
const POINT: u32 = D + 0x4470;
const TEX: u32 = D + 0x29C;
const TIMER: u32 = D + 0x44A0;
const DRIFT: u32 = D + 0x44C4;
const NODE_A: u32 = D + 0x44D8;
const NODE_C: u32 = D + 0x44DC;
const TNODE: u32 = D + 0x44E8;
const BOB: u32 = D + 0x44FC;
const SLOT: u32 = D + 0x4506;
const FACING: u32 = D + 0x450A;
const COUNTER: u32 = D + 0x450E;
const BYTES: u32 = D + 0x4510;
const POOL: Pool70 = Pool70 { base: 0x2A0, n: 150, colour: 0x44A4, pos: 0x250, slot: 0x4506 };

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// The entry 0x1E03020 (s0 = D, s1 = 0 then the live count, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xE0);
    fx.mem.write_u32(D + 0x44AC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xE0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4880);
    l2::sine_table(fx, D + 0x4710);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x450C, param);
    fx.mem.set32(TIMER, 0xD2);
    for (k, node) in [0x33, 0x3A, 1].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE_A + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TNODE, r);
    for k in 0xA..0x10u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x260 + 4 * k, t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    let t = fx.texture(5, 0);
    fx.mem.write_u32(D + 0x4480, t);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.fix_residue(y, 0x01E0_3178);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    fx.mem.set16(BOB, -0x3C);
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_31A8, 0x1E);
        fx.mem.set8(BYTES + k, r - 0xF);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 | IDLE => continue,
            WIDE | SPIN | BAND | CLOSE | GROW | ARC_IN | ARC_OUT => {}
            _ => panic!("b384: unknown task step {f:#x}"),
        }
        let s = fx.enter(0x20);
        // The steps' saves: ra, the entry's s0 = D.
        fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
        match f {
            WIDE => wide(fx, t),
            SPIN => spin(fx, t),
            BAND => band(fx, t),
            CLOSE => close(fx, t),
            GROW => grow(fx, t),
            ARC_IN => arc(fx, t, ARC_IN),
            _ => arc(fx, t, ARC_OUT),
        }
        fx.leave(0x20);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        draw(fx, sp, t);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x96 => 1,
        0xAA => 4,
        0xA0 => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw of one live task by its step (s0 = D).
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    if UNSPAWNED.contains(&f) {
        panic!("b384: unported draw case {f:#x}");
    }
    let cell = |fx: &mut Fx, r: u32, id: i32| {
        let sub = fx.mem.i16(t + 0x56);
        l2::rect(fx, sp + r, id, sub);
        for k in 0..4 {
            let x = fx.mem.read_u32(sp + r + 4 * k);
            fx.mem.write_u32(D + 0x94 + 4 * k, x);
        }
    };
    let tex = |fx: &mut Fx| {
        let w = fx.mem.read_u32(TEX);
        fx.mem.write_u32(D + 0x88, w);
    };
    let at = |fx: &mut Fx, from: u32| {
        for o in 0..4 {
            let w = fx.mem.read_u32(from + 4 * o);
            fx.mem.write_u32(sp + 0x50 + 4 * o, w);
        }
    };
    let g = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
    if g(fx) == WIDE {
        cell(fx, 0x70, 0x9B);
        tex(fx);
        fx.mem.set16(D + 0xB8, 6);
        let v = fx.mem.i16(t + 0xA);
        fx.mem.set16(D + 0xC0, v);
        for o in [0u32, 4, 8] {
            let w = fx.mem.read_u32(t + 0x10 + o);
            fx.mem.write_u32(D + 0x20 + o, w);
        }
        quads4::ring_facing_flat(fx, uv::uv_rect, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    if g(fx) == SPIN {
        tex(fx);
        if fx.mem.i16(t + 0x50) == 0 {
            cell(fx, 0x80, 0x9B);
        } else {
            cell(fx, 0x90, 0x97);
        }
        at(fx, t + 0x10);
        fx.mem.set16(D + 0xB8, 9);
        billboard::arc_quads(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
        fx.mem.set16(D + 0xB8, 0);
    }
    if g(fx) == BAND {
        at(fx, POINT);
        quads4::ring_band_f(fx, D, t, 0x21, sp + 0x50);
    }
    if g(fx) == CLOSE {
        at(fx, t + 0x10);
        quads4::ring_band_f(fx, D, t, 0x21, sp + 0x50);
    }
    if g(fx) == GROW {
        at(fx, POINT);
        quads4::ring_band_f(fx, D, t, 0x21, sp + 0x50);
    }
    if g(fx) == ARC_IN {
        tex(fx);
        if fx.mem.i16(t + 0x50) == 0 {
            cell(fx, 0xA0, 0x9B);
        } else {
            cell(fx, 0xB0, 0x9B);
        }
        at(fx, POINT);
        fx.mem.set16(D + 0xB8, 9);
        billboard::arc_quads(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
        fx.mem.set16(D + 0xB8, 0);
    }
    if g(fx) == ARC_OUT {
        tex(fx);
        cell(fx, 0xC0, 0x9B);
        at(fx, t + 0x10);
        fx.mem.set16(D + 0xB8, 9);
        billboard::arc_quads(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
        fx.mem.set16(D + 0xB8, 0);
    }
}

/// 0x1E01710(D, 0) (frame 0x50: ra, the entry's s1 = 0, s0 = D): the
/// update counter D+0x450E, the point D+0x4470 and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    use ee::{add, mul, sub};
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_31F8), (0x10, 0), (0, u64::from(D))]);
    let dd = u64::from(D);
    let p = param as i64 as u64;
    // The emitter's s1: 0, then sp + 0x40 once the first block has run.
    let mut s1 = 0u64;
    // The spawns' saves: s0 = D, s1, the entry's s2 = the parameter, s3 /
    // s4 the runner's (0).
    let go = |fx: &mut Fx, f: u32, s1: u64, jal: u32| {
        let saves = [(0x50, u64::from(jal + 8)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, s1), (0, dd)];
        spawn::task70(fx, D, POOL, f, 0, &saves);
    };
    let point_to_pos = |fx: &mut Fx| {
        for o in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * o);
            fx.mem.write_u32(POS + 4 * o, w);
        }
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    if c < 0x76 {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE_A);
        node_position_saving(fx, h, s, sp + 0x30, dd, Some(0), 0x01E0_1754);
        s1 = u64::from(sp + 0x40);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE_C);
        node_position_saving(fx, h, s, sp + 0x40, dd, Some(s1), 0x01E0_1770);
        let x = fx.mem.read_u32(sp + 0x40);
        fx.mem.write_u32(POINT, x);
        let y = fx.mem.read_u32(sp + 0x34);
        fx.mem.write_u32(POINT + 4, y);
        let z = fx.mem.read_u32(sp + 0x38);
        fx.mem.write_u32(POINT + 8, z);
        let z = fx.mem.f(POINT + 8);
        let z = if fx.mem.i16(FACING) != 0 { sub(z, fx.mem.f(DRIFT)) } else { add(z, fx.mem.f(DRIFT)) };
        fx.mem.setf(POINT + 8, z);
        if fx.mem.i16(COUNTER) < 0x67 {
            let a = fx.mem.i16(BOB);
            let s = l2::sin_deg(fx, D + 0xD8, a, 0);
            let y = add(fx.mem.f(POINT + 4), mul(3.0, s));
            fx.mem.setf(POINT + 4, y);
        } else {
            let y = add(fx.mem.f(POINT + 4), 3.0);
            fx.mem.setf(POINT + 4, y);
        }
    } else {
        if c < 0xA1 {
            let z = fx.mem.f(POINT + 8);
            let z = if fx.mem.i16(FACING) != 0 { sub(z, 4.0) } else { add(z, 4.0) };
            fx.mem.setf(POINT + 8, z);
            let y = sub(fx.mem.f(POINT + 4), f32::from_bits(0x3E99_999A));
            fx.mem.setf(POINT + 4, y);
        }
        point_to_pos(fx);
        if fx.mem.i16(COUNTER) == 0xA0 {
            fx.mem.set16(SLOT, 0);
            go(fx, CLOSE, s1, 0x01E0_18B8);
            for jal in [0x01E0_18CCu32, 0x01E0_18E0, 0x01E0_18F4, 0x01E0_1908, 0x01E0_191C] {
                go(fx, SPIN, s1, jal);
            }
            for jal in [0x01E0_1930u32, 0x01E0_1944, 0x01E0_1958, 0x01E0_196C, 0x01E0_1980, 0x01E0_1994] {
                go(fx, WIDE, s1, jal);
            }
            fx.mem.set16(SLOT, 0);
            go(fx, SPIN, s1, 0x01E0_19AC);
            go(fx, SPIN, s1, 0x01E0_19C0);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if c >= 0x10 {
        let d = fx.mem.f(DRIFT);
        if d < 10.0 {
            fx.mem.setf(DRIFT, add(d, 0.5));
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x41..0xA1).contains(&c) {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE_A);
        node_position_saving(fx, h, s, POS, dd, Some(s1), 0x01E0_1A40);
        fx.mem.set16(SLOT, 0);
        go(fx, BAND, s1, 0x01E0_1A50);
    }
    if fx.mem.i16(COUNTER) == 0xF {
        go(fx, GROW, s1, 0x01E0_1A74);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x2D..0x5B).contains(&c) {
        point_to_pos(fx);
        go(fx, ARC_IN, s1, 0x01E0_1AC4);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x76..0xA1).contains(&c) {
        point_to_pos(fx);
        go(fx, ARC_OUT, s1, 0x01E0_1B14);
    }
    if fx.mem.i16(COUNTER) >= 0xF {
        let b = fx.mem.i16(BOB);
        if b < 0x5A {
            fx.mem.set16(BOB, b + 3);
        }
    }
    fx.leave(0x50);
}

/// +0x63 in by `up` under 0xC0 while `rising`, else out by 9 (the task
/// ends below 11).
fn fade(fx: &mut Fx, t: u32, rising: bool, up: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if rising {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(up));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The five colours a task picks by `k` (+0x52).
fn palette(fx: &mut Fx, at: u32, k: i32) {
    match k {
        0 => l2::rgb(fx, at, 0xFF, 0x80, 0x80),
        1 => l2::rgb(fx, at, 0x80, 0xFF, 8),
        2 => l2::rgb(fx, at, 0x80, 0x80, 0xFF),
        3 => l2::rgb(fx, at, 0xFF, 0xFF, 0x80),
        4 => l2::rgb(fx, at, 0x80, 0x40, 0xFF),
        _ => {}
    }
}

/// Three random angles +0x20 / +0x24 / +0x28 (rand % 0xFFFF from `ras`).
fn angles(fx: &mut Fx, t: u32, ras: [u32; 3]) {
    for (o, ra) in [0x20u32, 0x24, 0x28].into_iter().zip(ras) {
        let v = rmod(fx, t, ra, 0xFFFF);
        fx.mem.set32(t + o, v);
    }
}

/// +0x52 the next colour of [`palette`] into +0x64 (0..4 round).
fn cycle(fx: &mut Fx, t: u32) {
    let k = fx.mem.i16(t + 0x52);
    palette(fx, t + 0x64, k);
    let k = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, k);
    if fx.mem.i16(t + 0x52) >= 5 {
        fx.mem.set16(t + 0x52, 0);
    }
}

/// 0x1E01F90 (D, task) (b149's 0x1E01AC0 with size 5 and growth 6..15): a
/// wide ring of quads (cell rand % 4, grey, a random angle, +0xA rand % 8,
/// +0xC the slot, radius 10..59 growing by 6..15 from age 1, row 0..34,
/// life 3..7) at 0x80, then out by 8.
fn wide(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        let v = fx.rand_from(t, 0x01E0_1FB8);
        fx.mem.set16(t + 0x56, v.wrapping_rem(4));
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_1FEC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_2008);
        fx.mem.set16(t + 0xA, v.wrapping_rem(8));
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        let v = rmod(fx, t, 0x01E0_202C, 0x32);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_204C, 0xA);
        fx.mem.set16(t + 0xE, v + 6);
        let v = rmod(fx, t, 0x01E0_206C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_2088, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xA {
            fx.mem.write_u8(t + 0x63, a - 8);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    age(fx, t);
}

/// 0x1E02130 (D, task): a spinning arc of quads (radius 10, outer 0, a
/// palette colour over a red rim, random angles, from 45 degrees spanning
/// 1..4); in by 0x40 for 8 updates, then out by 9; widening by 0.5 / 0.4
/// for 4 updates, then 0.8 / 0.4.
fn spin(fx: &mut Fx, t: u32) {
    use ee::add;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        l2::rgb(fx, t + 0x60, 0xCF, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 8, 8);
        let v = rmod(fx, t, 0x01E0_2184, 5);
        fx.mem.set16(t + 0x52, v);
        let k = fx.mem.i16(t + 0x52);
        palette(fx, t + 0x60, k);
        let v = rmod(fx, t, 0x01E0_2264, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        angles(fx, t, [0x01E0_228C, 0x01E0_22A8, 0x01E0_22C4]);
        fx.mem.set16(t + 0x5A, 0x2D);
        let v = fx.rand_from(t, 0x01E0_22E8);
        fx.mem.set16(t + 0x5C, v.wrapping_rem(4) + 1);
    }
    let rising = fx.mem.i16(t + 0x54) < 8;
    fade(fx, t, rising, 0x40);
    let (a, b) = if fx.mem.i16(t + 0x54) < 4 { (0.5, f32::from_bits(0x3ECC_CCCD)) } else { (f32::from_bits(0x3F4C_CCCD), f32::from_bits(0x3ECC_CCCD)) };
    let r = add(fx.mem.f(t + 0x48), a);
    fx.mem.setf(t + 0x48, r);
    let o = add(fx.mem.f(t + 0x4C), b);
    fx.mem.setf(t + 0x4C, o);
    age(fx, t);
}

/// 0x1E023F0 (D, task): a white band over a palette rim from radius 0
/// widening by 3 (the outer edge 2 inside), random angles; in by 0x40 for
/// 13 updates, then out by 9.
fn band(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0xCF, 0xCF, 0xCF);
        l2::rgb(fx, t + 0x64, 0x80, 8, 8);
        let v = rmod(fx, t, 0x01E0_2440, 5);
        fx.mem.set16(t + 0x52, v);
        let k = fx.mem.i16(t + 0x52);
        palette(fx, t + 0x64, k);
        let v = rmod(fx, t, 0x01E0_2520, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        angles(fx, t, [0x01E0_2548, 0x01E0_2564, 0x01E0_2580]);
    }
    let rising = fx.mem.i16(t + 0x54) < 0xD;
    fade(fx, t, rising, 0x40);
    let r = ee::add(fx.mem.f(t + 0x48), 3.0);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::sub(r, 2.0));
    age(fx, t);
}

/// 0x1E02630 (D, task): a white ring of radius 300 (outer 0), its rim
/// cycling the palette; in by 0x40 for 16 updates, then out by 9; turned
/// by the camera's angles (the first + 0x4000).
fn close(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4396_0000);
        l2::rgb(fx, t + 0x60, 0xCF, 0xCF, 0xCF);
        let v = rmod(fx, t, 0x01E0_2670, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    cycle(fx, t);
    let rising = fx.mem.i16(t + 0x54) < 0x10;
    fade(fx, t, rising, 0x40);
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_27CC);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E02800 (D, task): a white ring from radius 5 (outer 0), its rim
/// cycling the palette; in by 0x18 for 0x87 updates, then out by 9; it
/// grows by 10 on ages 0x10..0x22 and by 0.7 while under 30 from age 1;
/// turned by the camera's angles (the first + 0x4000).
fn grow(fx: &mut Fx, t: u32) {
    use ee::add;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        l2::rgb(fx, t + 0x60, 0xCF, 0xCF, 0xCF);
        l2::rgb(fx, t + 0x64, 0x80, 0x40, 0x40);
        let v = rmod(fx, t, 0x01E0_2854, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    cycle(fx, t);
    let rising = fx.mem.i16(t + 0x54) < 0x87;
    fade(fx, t, rising, 0x18);
    let a = fx.mem.i16(t + 0x54);
    if (0x10..0x23).contains(&a) {
        let r = add(fx.mem.f(t + 0x48), 10.0);
        fx.mem.setf(t + 0x48, r);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_29E4);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.f(t + 0x48);
        if r < 30.0 {
            fx.mem.setf(t + 0x48, add(r, f32::from_bits(0x3F33_3333)));
        }
    }
    age(fx, t);
}

/// 0x1E02A60 / 0x1E02D40 (D, task): an arc of quads in a palette colour
/// (both +0x60 / +0x64), random angles, from 45 degrees spanning 5; the
/// first from radius 10..29 shrinking by 1 (the outer 6 beyond), in by
/// 0x40 for 8 updates; the second from 0..9 widening by 2 (the outer 6
/// inside), in for 3; then out by 9.
fn arc(fx: &mut Fx, t: u32, at: u32) {
    use ee::{add, sub};
    let shrink = at == ARC_IN;
    // The second's code is one word shorter after its first rand.
    let k = if shrink { 0 } else { 0x2DC };
    if fx.mem.i16(t + 0x58) == 1 {
        let r = if shrink { rmod(fx, t, 0x01E0_2A84, 0x14) + 0xA } else { rmod(fx, t, 0x01E0_2D64, 0xA) };
        fx.mem.setf(t + 0x48, ee::from_i32(r));
        l2::rgb(fx, t + 0x60, 0xCF, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 8, 8);
        let v = rmod(fx, t, 0x01E0_2AD4 + k, 5);
        fx.mem.set16(t + 0x52, v);
        let c = fx.mem.i16(t + 0x52);
        if (0..5).contains(&c) {
            palette(fx, t + 0x60, c);
            palette(fx, t + 0x64, c);
        }
        let v = rmod(fx, t, 0x01E0_2C18 + k, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        angles(fx, t, [0x01E0_2C40 + k, 0x01E0_2C5C + k, 0x01E0_2C78 + k]);
        fx.mem.set16(t + 0x5A, 0x2D);
        fx.mem.set16(t + 0x5C, 5);
    }
    let rising = fx.mem.i16(t + 0x54) < if shrink { 8 } else { 3 };
    fade(fx, t, rising, 0x40);
    if shrink {
        let r = sub(fx.mem.f(t + 0x48), 1.0);
        fx.mem.setf(t + 0x48, r);
        fx.mem.setf(t + 0x4C, add(6.0, r));
    } else {
        let r = add(fx.mem.f(t + 0x48), 2.0);
        fx.mem.setf(t + 0x48, r);
        fx.mem.setf(t + 0x4C, sub(r, 6.0));
    }
    age(fx, t);
}
