//! BMODULE b111 (card 307), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02F00, data area 0x1ED0000) [F]: b402's task program (card 249) with every global 0x10 lower
//! (150 x 0x70 tasks at D+0x2A0, alive +0x58, step +0x44 through `jalr`,
//! 0x1E02EF0 only returns; spawned by 0x1E03670 = b402's 0x1E034C0 with
//! the colour D+0x44A4 and the slot D+0x4502), a 420-update timer
//! D+0x44A0, the attacker's nodes 3 / 0x34 / 0x35 / 0x30 / 0x2F / 0x2C, the
//! target's node 1 and one texture (4, 0xF). The emitter 0x1E01870 counts
//! D+0x450A: on 1..0x154 at node 3 a growing white / red band (0x1E024F0,
//! `ring_band_f`) a update, with a small red flare ring (0x1E022F0,
//! `ring_quads`) from 0xD2; 0x8D..0xDC a band at the target (0x1E026B0);
//! on 0x84 / 0xD7 a flickering band (0x1E02A10) at node 3; on 0xB9 a
//! closing ring (0x1E02860) at the target, the point D+0x4470 then drawn
//! a 30th of the way back to node 3 each update to 0xD6 with a larger
//! flare ring (0x1E02BE0) there; on 0x155..0x159 a band (0x1E02DE0) at each
//! of the nodes 0x34 / 0x35 / 0x30 / 0x2F / 0x2C. Returns 1 on update
//! 0x79, 4 on 0x17C, 2 on 0xD7, 6 on 0x84..0xD1, 8 once the timer has run
//! out.
//!
//! The steps 0x1E01D30 / 0x1E02160 are tested by the entry but never
//! spawned (0x1E02160 only by 0x1E01D30) [F]; not ported.
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
const FLARE_A: u32 = 0x01E0_22F0;
const BAND_A: u32 = 0x01E0_24F0;
const BAND_B: u32 = 0x01E0_26B0;
const RING_C: u32 = 0x01E0_2860;
const RING_D: u32 = 0x01E0_2A10;
const FLARE_B: u32 = 0x01E0_2BE0;
const BAND_E: u32 = 0x01E0_2DE0;
const IDLE: u32 = 0x01E0_2EF0;
const UNSPAWNED: [u32; 2] = [0x01E0_1D30, 0x01E0_2160];
/// The return address of the entry's `jalr` into a step (0x1E031B8 + 8).
const TASK_RA: u64 = 0x01E0_31C0;
const POS: u32 = D + 0x250;
const POINT: u32 = D + 0x4470;
const TIMER: u32 = D + 0x44A0;
const NODES: u32 = D + 0x44D4;
const TNODE: u32 = D + 0x44E4;
const SLOT: u32 = D + 0x4502;
const NEXT: u32 = D + 0x4504;
const COUNTER: u32 = D + 0x450A;
const BYTES: u32 = D + 0x450C;
const BYTES_B: u32 = D + 0x460C;
const POOL: Pool70 = Pool70 { base: 0x2A0, n: 150, colour: 0x44A4, pos: 0x250, slot: 0x4502 };

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// The entry 0x1E02F00 (s0 = D, s1 = 0, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0x44AC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4880);
    l2::sine_table(fx, D + 0x470C);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4508, param);
    fx.mem.set32(TIMER, 0x1A4);
    for (k, node) in [3, 0x34, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TNODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_306C);
    fx.mem.set16(D + 0x4506, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_3094, 0x1E);
        fx.mem.set8(BYTES + k, r - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_30E4);
        fx.mem.write_u8(BYTES + k, v.wrapping_rem(4) as u8);
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_3134, 0x28);
        fx.mem.set8(BYTES_B + k, r - 0x14);
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
        let frame = match f {
            0 | IDLE => continue,
            BAND_A | BAND_B | RING_C => 0x30,
            FLARE_A | FLARE_B | RING_D | BAND_E => 0x20,
            _ => panic!("b111: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        // The steps' saves: ra, the entry's s1 = 0, s0 = D.
        if frame == 0x30 {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
        } else {
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
        }
        match f {
            FLARE_A => flare(fx, t, FLARE_A, (5, 2), (0x80, 0x30, 0x30), (0x1E, 0xA)),
            FLARE_B => flare(fx, t, FLARE_B, (3, 4), (0xC0, 0x50, 0x50), (0x28, 0x14)),
            BAND_A => band_a(fx, t),
            BAND_B => band_b(fx, t),
            RING_C => ring_c(fx, t),
            RING_D => ring_d(fx, t),
            _ => band_e(fx, t),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if UNSPAWNED.contains(&f) {
            panic!("b111: unported draw case {f:#x}");
        }
        let at = |fx: &mut Fx, from: u32| {
            for o in 0..4 {
                let w = fx.mem.read_u32(from + 4 * o);
                fx.mem.write_u32(sp + 0x50 + 4 * o, w);
            }
        };
        let flare_draw = |fx: &mut Fx, r: u32| {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            l2::rect(fx, sp + r, 0x97, sub);
            for k in 0..4 {
                let x = fx.mem.read_u32(sp + r + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, x);
            }
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 1);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        };
        let g = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if g(fx) == FLARE_A {
            flare_draw(fx, 0x80);
        }
        if g(fx) == BAND_A {
            at(fx, t + 0x10);
            quads4::ring_band_f(fx, D, t, 0x21, sp + 0x50);
        }
        if g(fx) == BAND_B {
            at(fx, t + 0x10);
            quads4::ring_band_f(fx, D, t, 0x21, sp + 0x50);
        }
        if g(fx) == RING_C {
            at(fx, POINT);
            quads4::ring_band_f(fx, D, t, 0x20, sp + 0x50);
        }
        if g(fx) == RING_D {
            at(fx, t + 0x10);
            quads4::ring_band_f(fx, D, t, 0x21, sp + 0x50);
        }
        if g(fx) == FLARE_B {
            flare_draw(fx, 0x90);
        }
        if g(fx) == BAND_E {
            at(fx, t + 0x10);
            quads4::ring_band_f(fx, D, t, 0x20, sp + 0x50);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x79 => 1,
        0x17C => 4,
        0xD7 => 2,
        0x84..=0xD1 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E01870(D, 0) (frame 0x60: ra, the entry's s2 = param, s1 = 0, s0 =
/// D; s2 = D): the update counter D+0x450A, the spawn point and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_3180), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let dd = u64::from(D);
    // The emitter's s0 / s1 (the frames of its callees save them).
    let (mut s0, mut s1) = (dd, 0u64);
    let go = |fx: &mut Fx, f: u32, s0: u64, s1: u64, ra: u32| {
        let saves = [(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, dd), (0x10, s1), (0, s0)];
        spawn::task70(fx, D, POOL, f, 0, &saves);
    };
    let node = |fx: &mut Fx, target: bool, slot: u32, out: u32, s0: u64, s1: u64, ra: u32| {
        let h = if target { fx.target_handle() } else { fx.self_handle() };
        let s = fx.mem.i16(slot);
        node_position_saving(fx, h, s, out, s0, Some(s1), ra);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    if c > 0 && c < 0x47 && c % 2 == 1 {
        for k in 0..2u64 {
            node(fx, false, NODES, POS, k, 2, 0x01E0_1908);
            let v = fx.mem.i16(NEXT);
            fx.mem.set16(SLOT, v);
            let v = fx.mem.i16(NEXT) + 1;
            fx.mem.set16(NEXT, v);
            if fx.mem.i16(NEXT) >= 0x78 {
                fx.mem.set16(NEXT, 0);
            }
        }
        s0 = 2;
        s1 = 2;
    }
    let c = fx.mem.i16(COUNTER);
    if c > 0 && c < 0x155 {
        node(fx, false, NODES, POS, s0, s1, 0x01E0_1988);
        fx.mem.set16(SLOT, 0);
        go(fx, BAND_A, s0, s1, 0x01E0_19A0);
        if fx.mem.i16(COUNTER) >= 0xD2 {
            go(fx, FLARE_A, s0, s1, 0x01E0_19C4);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x8D..0xDD).contains(&c) {
        node(fx, true, TNODE, POS, s0, s1, 0x01E0_19F8);
        fx.mem.set16(SLOT, 1);
        go(fx, BAND_B, s0, s1, 0x01E0_1A14);
    }
    if fx.mem.i16(COUNTER) == 0x84 {
        node(fx, false, NODES, POS, s0, s1, 0x01E0_1A3C);
        fx.mem.set16(SLOT, 0);
        go(fx, RING_D, s0, s1, 0x01E0_1A54);
    }
    if fx.mem.i16(COUNTER) == 0xB9 {
        node(fx, true, TNODE, POINT, s0, s1, 0x01E0_1A7C);
        go(fx, RING_C, s0, s1, 0x01E0_1A90);
    }
    let c = fx.mem.i16(COUNTER);
    if (0xBA..0xD7).contains(&c) {
        node(fx, false, NODES, sp + 0x40, s0, s1, 0x01E0_1AC4);
        s0 = u64::from(sp + 0x50);
        node(fx, true, TNODE, sp + 0x50, s0, s1, 0x01E0_1AE0);
        for o in [0u32, 4, 8] {
            let d = ee::div(ee::sub(fx.mem.f(sp + 0x40 + o), fx.mem.f(sp + 0x50 + o)), 30.0);
            let v = ee::add(fx.mem.f(POINT + o), d);
            fx.mem.setf(POINT + o, v);
        }
        for o in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * o);
            fx.mem.write_u32(POS + 4 * o, w);
        }
        go(fx, FLARE_B, s0, s1, 0x01E0_1B70);
    } else if c >= 0xD7 {
        if c == 0xD7 {
            node(fx, false, NODES, POS, s0, s1, 0x01E0_1BA8);
            fx.mem.set16(SLOT, 1);
            go(fx, RING_D, s0, s1, 0x01E0_1BC4);
        }
        if fx.mem.i16(COUNTER) < 0x155 {
            node(fx, false, NODES, POINT, s0, s1, 0x01E0_1BEC);
        }
    }
    let ends = [(0x155, 2u32, 0x01E0_1C14u32, 0x01E0_1C28u32), (0x156, 4, 0x01E0_1C50, 0x01E0_1C64), (0x157, 6, 0x01E0_1C8C, 0x01E0_1CA0), (0x158, 8, 0x01E0_1CC8, 0x01E0_1CDC), (0x159, 0xA, 0x01E0_1D04, 0x01E0_1D18)];
    for (n, o, ra, ra2) in ends {
        if fx.mem.i16(COUNTER) == n {
            node(fx, false, NODES + o, POS, s0, s1, ra);
            go(fx, BAND_E, s0, s1, ra2);
        }
    }
    fx.leave(0x60);
}

/// +0x63 in by 0x40 below 0xC0 while `rising`, else out by `down` (the
/// task ends below `floor`).
fn fade(fx: &mut Fx, t: u32, rising: bool, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if rising {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= floor {
        fx.mem.write_u8(t + 0x63, a - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The bands' first step: +0x48 = `inner`, white (0xCF) / `rim` colours,
/// a random angle (rand from `ra`), alpha +0x67 0.
fn band_first(fx: &mut Fx, t: u32, inner: u32, white: (i32, i32, i32), ra: u32) {
    fx.mem.write_u32(t + 0x48, inner);
    l2::rgb(fx, t + 0x60, white.0, white.1, white.2);
    l2::rgb(fx, t + 0x64, 0x80, 8, 8);
    let v = rmod(fx, t, ra, 0x168);
    fx.mem.set16(t + 0x5A, v);
    fx.mem.set16(t + 0x58, 2);
    fx.mem.write_u8(t + 0x67, 0);
}

/// Three random angles +0x20 / +0x24 / +0x28 (rand % 0xFFFF from `ras`).
fn angles(fx: &mut Fx, t: u32, ras: [u32; 3]) {
    for (o, ra) in [0x20u32, 0x24, 0x28].into_iter().zip(ras) {
        let v = rmod(fx, t, ra, 0xFFFF);
        fx.mem.set32(t + o, v);
    }
}

/// 0x1E022F0 / 0x1E02BE0 (D, task) at `at` (b611's 0x1E01ED0 with other
/// sizes, colours and radii): a flare ring of quads (size rand % s.0 +
/// s.1, cell rand % 4, three random angles, radius rand % r.0 + r.1
/// growing by 1..3, row 1 + rand % 8, life 2..4) at 0x60, then out by 11.
fn flare(fx: &mut Fx, t: u32, at: u32, s: (i32, i32), c: (i32, i32, i32), r: (i32, i32)) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, at + 0x24, s.0);
        fx.mem.setf(t + 0x48, ee::from_i32(v + s.1));
        let v = fx.rand_from(t, at + 0x50);
        fx.mem.set16(t + 0x56, v.wrapping_rem(4));
        l2::rgb(fx, t + 0x60, c.0, c.1, c.2);
        angles(fx, t, [at + 0x84, at + 0xA0, at + 0xBC]);
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, at + 0xE0, r.0);
        fx.mem.set16(t + 0x50, v + r.1);
        let v = rmod(fx, t, at + 0x100, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, at + 0x11C, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, at + 0x13C);
        fx.mem.set16(t + 8, v.wrapping_rem(8) + 1);
        let v = rmod(fx, t, at + 0x15C, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xD {
            fx.mem.write_u8(t + 0x63, a - 0xB);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, v);
    age(fx, t);
}

/// 0x1E024F0 (D, task) (frame 0x30: s0 = the task, s1 = D): a white / red
/// band from radius 0, three random angles; in by 0x40 for 10 updates,
/// then out by 14; on the first spawn slot (+0x50 = 0) it widens by 3.5
/// (2 on 0x84..0xDB), the outer edge 2 inside.
fn band_a(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        band_first(fx, t, 0, (0xCF, 0xCF, 0xCF), 0x01E0_2548);
        angles(fx, t, [0x01E0_2570, 0x01E0_258C, 0x01E0_25A8]);
    }
    let rising = fx.mem.i16(t + 0x54) < 0xA;
    fade(fx, t, rising, 0x10, 0xE);
    if fx.mem.i16(t + 0x50) == 0 {
        let c = fx.mem.i16(COUNTER);
        let by = if (0x84..0xDC).contains(&c) { 2.0 } else { 3.5 };
        let r = ee::add(fx.mem.f(t + 0x48), by);
        fx.mem.setf(t + 0x48, r);
        fx.mem.setf(t + 0x4C, ee::sub(r, 2.0));
    }
    age(fx, t);
}

/// 0x1E026B0 (D, task) (frame 0x30): [`band_a`] widening by 8.5 before
/// update 0xC8, then 4.5, the outer edge 4 inside, on every slot.
fn band_b(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        band_first(fx, t, 0, (0xCF, 0xCF, 0xCF), 0x01E0_2708);
        angles(fx, t, [0x01E0_2730, 0x01E0_274C, 0x01E0_2768]);
    }
    let rising = fx.mem.i16(t + 0x54) < 0xA;
    fade(fx, t, rising, 0x10, 0xE);
    let by = if fx.mem.i16(COUNTER) < 0xC8 { 8.5 } else { 4.5 };
    let r = ee::add(fx.mem.f(t + 0x48), by);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::sub(r, 4.0));
    age(fx, t);
}

/// +0x64 red (0x80 / 0xC0 on odd / even ages) or, `blue`, blue.
fn flicker(fx: &mut Fx, t: u32, blue: bool) {
    let hi = if fx.mem.i16(t + 0x54) % 2 == 1 { 0x80 } else { 0xC0 };
    if blue {
        l2::rgb(fx, t + 0x64, 8, 8, hi);
    } else {
        l2::rgb(fx, t + 0x64, hi, 8, 8);
    }
}

/// 0x1E02860 (D, task) (frame 0x30: s0 = the task, s1 = D): a pink / red
/// ring from radius 10 growing by 10 to 100, in by 0x40 to update 0x153,
/// then out by 9; flickering red; turned by the camera's angles (the first
/// + 0x4000).
fn ring_c(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        band_first(fx, t, 0x4120_0000, (0xCF, 0x8F, 0x8F), 0x01E0_28BC);
    }
    let rising = fx.mem.i16(COUNTER) < 0x154;
    fade(fx, t, rising, 0xB, 9);
    flicker(fx, t, false);
    let r = fx.mem.f(t + 0x48);
    if r < 100.0 {
        fx.mem.setf(t + 0x48, ee::add(r, 10.0));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_29D4);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E02A10 (D, task) (frame 0x20: s0 = the task): a white ring of radius
/// 100 (outer 0), in by 0x40 for 4 updates, then out by 9; flickering red
/// on slot 0, blue on slot 1; turned by the camera's angles (+ 0x4000).
fn ring_d(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        band_first(fx, t, 0x42C8_0000, (0xCF, 0xCF, 0xCF), 0x01E0_2A64);
    }
    let rising = fx.mem.i16(t + 0x54) < 4;
    fade(fx, t, rising, 0xB, 9);
    match fx.mem.i16(t + 0x50) {
        0 => flicker(fx, t, false),
        1 => flicker(fx, t, true),
        _ => {}
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2BA8);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E02DE0 (D, task) (frame 0x20): a white / red band at a quarter
/// turn, from radius 0 widening by 3 (the outer edge 5 outside); in by
/// 0x40 for 13 updates, then out by 9.
fn band_e(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        band_first(fx, t, 0, (0xCF, 0xCF, 0xCF), 0x01E0_2E30);
        fx.mem.write_u32(t + 0x20, 0x4000);
    }
    let rising = fx.mem.i16(t + 0x54) < 0xD;
    fade(fx, t, rising, 0xB, 9);
    let r = ee::add(fx.mem.f(t + 0x48), 3.0);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::add(5.0, r));
    age(fx, t);
}
