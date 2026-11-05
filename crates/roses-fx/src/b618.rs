//! BMODULE b618 (card 94), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01210, data area 0x1ED0000) [F]: b278's program with other numbers: a pool of 400 tasks (0x70
//! bytes at +0x2A0, alive +0x58, step +0x44, all 0x1E00E90) drawn as rings
//! of one camera-facing quad each (98b02c86a827 at 0x1E00920,
//! `billboard::ring_facing`, UVs by uv_rect 0x1E00080). Every update first
//! stores the camera's +0x20 / +0x30 (0x1FACE0 / 0x1FAF40) at D+0xB204 /
//! D+0xB208 (read by nothing else). From update 170 to 352 three red motes
//! a update leave the attacker's node 6, drift toward the target (z by
//! 1 / 80 of the gap a update), wander for their updates 11..69 and fade
//! out after 100 updates (from update 421 all of them). Returns 1 on update
//! 230, 3 on 435, 6 on 241..424, 8 once the 450-update timer +0xB1F0 has
//! run out.
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The one step function a task holds (+0x44), as the record's address.
const MOTE: u32 = 0x01E0_0E90;
/// The entry's `jalr` return (0x1E01610 + 8) and the emitter's `jal`
/// return into it (0x1E015B8 + 8): the callees' prologue saves (TEX0 /
/// TEX1 of the rings' descriptor when the texture is missing).
const TASK_RA: u64 = 0x01E0_1618;
const SPAWN_RA: u64 = 0x01E0_15C0;
/// Data-area fields above 0x10000 (the original's `lui 1` pairs).
const TIMER: u32 = D + 0xB1F0;
const COUNTER: u32 = D + 0xB25E;
const BYTES: u32 = D + 0xB260;

/// The entry 0x1E01210.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    fx.mem.write_u32(D + 0xB1FC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let eye = fx.camera_eye(0);
    fx.mem.write_u32(D + 0xB204, eye);
    let at = fx.camera_at30(0);
    fx.mem.write_u32(D + 0xB208, at);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB5D0);
    l2::sine_table(fx, D + 0xB460);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB25C, param);
    fx.mem.set32(TIMER, 0x1C2);
    for (at, node, clear) in [(0xB228u32, 6, 1), (0xB22A, 0x1C, 0), (0xB22C, 0xD, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB238, r);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0xB1D0 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x60);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x64)), 180.0);
    fx.mem.set16(D + 0xB25A, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 100..0xB4u32 {
        let v = fx.rand();
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0xB4..0x100u32 {
        let v = fx.rand();
        fx.mem.set8(BYTES + k, v % 16 - 8);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x28);
        fx.mem.set8(D + 0xB360 + k, h - 0x14);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let n = fx.mem.i16(COUNTER);
    let count = if (0xAA..0x161).contains(&n) { 3 } else { 0 };
    for k in 0..count {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0xB228);
        fx.node_position(h, slot, D + 0x250);
        spawn(fx, MOTE, 0, k);
    }
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) != 0 && fx.mem.read_u32(t + 0x2E4) != 0 {
            let f = fx.mem.read_u32(t + 0x2E4);
            assert_eq!(f, MOTE, "b278: unknown task step");
            mote(fx, t + 0x2A0);
        }
    }
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 || fx.mem.read_u32(t + 0x2E4) != MOTE {
            continue;
        }
        let sub = fx.mem.i16(t + 0x2F6);
        l2::rect(fx, sp + 0x50, 0x14, sub);
        for k in 0..4 {
            let v = fx.mem.read_u32(sp + 0x50 + 4 * k);
            fx.mem.write_u32(D + 0x94 + 4 * k, v);
        }
        fx.mem.set16(D + 0xC0, 5);
        let tex = fx.mem.read_u32(D + 0xB1D0);
        fx.mem.write_u32(D + 0x88, tex);
        fx.mem.set16(D + 0xB8, 1);
        for k in 0..3 {
            let v = fx.mem.read_u32(t + 0x2B0 + 4 * k);
            fx.mem.write_u32(D + 0x20 + 4 * k, v);
        }
        fx.mem.set16(D + 0xBE, 0);
        billboard::ring_facing(fx, uv::uv_rect, D, t + 0x2A0, 0x10, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xE6 => 1,
        0x1B3 => 3,
        0xF1..=0x1A8 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E017D0(D, step, arg): the first free task cleared and set off at
/// D+0x250 with the colour D+0xB1F4 (twice), angle flag D+0xB256; 1 when
/// one was free. `k`: the emitter's counter (its saved s3).
fn spawn(fx: &mut Fx, f: u32, arg: u32, k: i32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, SPAWN_RA), (0x30, k as u64), (0x20, 3), (0x10, 0), (0, u64::from(D))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(D + 0xB1F4), fx.mem.read_u8(D + 0xB1F5), fx.mem.read_u8(D + 0xB1F6));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(D + 0xB1F7);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(D + 0xB256);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, arg);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand % m (HI, the remainder).
fn rand_mod(fx: &mut Fx, m: i32) -> i32 {
    let v = fx.rand();
    fx.div(v, m).1
}

/// 0x1E00E90(D, task): a mote. First step: size 15, a random cell frame
/// (-7..7), red-white, random angles, a velocity (-20..19) / 60 per axis,
/// z speed (target z - node z) / 80. Fades in by 0xC0 for 100 updates,
/// then out by 11; moves by its velocity for its updates 11..69; z +=
/// speed and size += 1.5 a update; from update 421 of the effect its age
/// is held at 100 (fading out).
fn mote(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4170_0000);
        let v = fx.rand();
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        let a = rand_mod(fx, 0x168);
        fx.mem.set16(t + 0x5A, a);
        let a = rand_mod(fx, 0x168);
        fx.mem.set16(t + 0x5C, a);
        let v = fx.rand();
        fx.mem.set16(t + 0xA, if v % 2 != 0 { 2 } else { 0 });
        let a = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, a);
        fx.mem.set16(t + 0x50, 0);
        let v = rand_mod(fx, 0x15E);
        fx.mem.set16(t + 0x52, v + 0x64);
        let v = rand_mod(fx, 0x14);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = rand_mod(fx, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, 5);
        fx.mem.set16(t + 6, v + 3);
        for o in [0x30u32, 0x34, 0x38] {
            let v = rand_mod(fx, 0x28);
            fx.mem.setf(t + o, ee::from_i32(v - 0x14));
        }
        for o in [0x30u32, 0x34, 0x38] {
            let v = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, v);
        }
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x30);
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0xB228);
        fx.node_position(h, slot, sp + 0x40);
        let dz = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48)), 80.0);
        fx.mem.setf(t + 0x3C, dz);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x64 {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0xC0));
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xD {
            fx.mem.write_u8(t + 0x63, a - 0xB);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if (0xB..0x46).contains(&fx.mem.i16(t + 0x54)) {
        for o in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(t + 0x30 + o));
            fx.mem.setf(t + 0x10 + o, v);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    if fx.mem.i16(t + 0x54) != 0 {
        let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x3C));
        fx.mem.setf(t + 0x18, z);
        let s = ee::add(fx.mem.f(t + 0x48), 1.5);
        fx.mem.setf(t + 0x48, s);
    }
    if fx.mem.i16(COUNTER) >= 0x1A5 {
        fx.mem.set16(t + 0x54, 0x64);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x60);
}
