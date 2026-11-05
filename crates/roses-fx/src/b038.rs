//! BMODULE b038 (card 6), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01790, data area 0x1ED0000) [F]: b278's family: a pool of 400 tasks (0x70 bytes at +0x2A0, alive
//! +0x58, step +0x44) drawn as rings of one camera-facing quad each
//! (98b02c86a827 at 0x1E00920, `billboard::ring_facing`, UVs by uv_rect
//! 0x1E00080). Every update first stores the camera's +0x20 / +0x30
//! (0x1FACE0 / 0x1FAF40) at D+0xB204 / D+0xB208. The emitter 0x1E00E90:
//! from update 90 to 245 three dark puffs a update (0x1E01090, blend 2,
//! bobbing 120 x (sin +0x5C - sin 225) in y), up to update 150 one grey
//! mote a update (0x1E01400) at the attacker's node 0x13 (shifted 4 in z
//! by the facing, 1.5 down). Returns 1 on update 110, 4 on 310, 2 on 290,
//! 6 on 121..284, 8 once the 317-update timer +0xB1F0 has run out.
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const PUFF: u32 = 0x01E0_1090;
const MOTE: u32 = 0x01E0_1400;
/// The entry's `jal` to the emitter / `jalr` returns (0x1E01AD8 + 8,
/// 0x1E01B08 + 8) and the emitter's `jal`s to the spawn (0x1E00F98 + 8,
/// 0x1E01068 + 8): prologue saves below the entry's frame (the rings'
/// stale TEX0 / TEX1 when the texture is missing).
const EMIT_RA: u64 = 0x01E0_1AE0;
const TASK_RA: u64 = 0x01E0_1B10;
const SPAWN_RA: [u64; 2] = [0x01E0_0FA0, 0x01E0_1070];
/// Data-area fields above 0x10000 (the original's `lui 1` pairs).
const TIMER: u32 = D + 0xB1F0;
const COUNTER: u32 = D + 0xB25E;
const BYTES: u32 = D + 0xB260;
const SINES: u32 = D + 0xB460;

/// The entry 0x1E01790.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    fx.mem.write_u32(D + 0xB1FC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let eye = fx.camera_eye(0);
    fx.mem.write_u32(D + 0xB204, eye);
    let at = fx.camera_at30(0);
    fx.mem.write_u32(D + 0xB208, at);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB5D0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB25C, param);
    fx.mem.set32(TIMER, 0x13D);
    for (at, node, clear) in [(0xB228u32, 0x13, 1), (0xB22A, 0xD, 0), (0xB22C, 0xD, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB238, r);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x298 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0xB1D0 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x70);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x74)), 180.0);
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

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) != 0 && fx.mem.read_u32(t + 0x2E4) != 0 {
            match fx.mem.read_u32(t + 0x2E4) {
                PUFF => puff(fx, t + 0x2A0),
                MOTE => mote(fx, t + 0x2A0),
                f => panic!("b038: unknown task step {f:#x}"),
            }
        }
    }
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x2E4) == PUFF {
            let sub = fx.mem.i16(t + 0x2F6);
            l2::rect(fx, sp + 0x50, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + 0x50 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            let sub = fx.mem.i16(t + 0x2F6);
            fx.mem.set16(D + 0x94, sub << 5);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x20);
            fx.mem.set16(D + 0x9A, 0x40);
            fx.mem.set16(D + 0xC0, 0);
            let tex = fx.mem.read_u32(D + 0xB1D4);
            fx.mem.write_u32(D + 0x88, tex);
            fx.mem.set16(D + 0xB8, 1);
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x2B0 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, v);
            }
            let s = l2::sin_deg(fx, SINES, 0xE1, 0);
            let y = ee::sub(fx.mem.f(D + 0x24), ee::mul(120.0, s));
            fx.mem.setf(D + 0x24, y);
            let a = fx.mem.i16(t + 0x2FC);
            let s = l2::sin_deg(fx, SINES, a, 0);
            let y = ee::add(fx.mem.f(D + 0x24), ee::mul(120.0, s));
            fx.mem.setf(D + 0x24, y);
            fx.mem.set16(D + 0xBE, 0);
            billboard::ring_facing(fx, uv::uv_rect, D, t + 0x2A0, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x2E4) == MOTE {
            let sub = fx.mem.i16(t + 0x2F6);
            l2::rect(fx, sp + 0x60, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + 0x60 + 4 * k);
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
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x6E => 1,
        0x136 => 4,
        0x122 => 2,
        0x79..=0x11C => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The node position D+0x250 shifted 4 in z (back when the facing D+0xB25A
/// is set) and 1.5 down.
fn place(fx: &mut Fx) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(D + 0xB228);
    fx.node_position(h, slot, D + 0x250);
    let z = fx.mem.f(D + 0x258);
    let z = if fx.mem.i16(D + 0xB25A) != 0 { ee::sub(z, 4.0) } else { ee::add(z, 4.0) };
    fx.mem.setf(D + 0x258, z);
    let y = ee::sub(fx.mem.f(D + 0x254), 1.5);
    fx.mem.setf(D + 0x254, y);
}

/// 0x1E00E90(D, 0): the update counter and the two emitters.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, EMIT_RA), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The emitter's s0 / s1 (the spawn's saves): the loop's counter and
    // count, the entry's D / 0 when the first emitter is idle.
    let (mut s0, mut s1) = (u64::from(D), 0u64);
    if (0x5A..0xF6).contains(&fx.mem.i16(COUNTER)) {
        s1 = 3;
        for k in 0..3 {
            place(fx);
            spawn(fx, PUFF, 0, [k, s1], SPAWN_RA[0]);
        }
        s0 = 3;
    }
    if fx.mem.i16(COUNTER) < 0x97 {
        place(fx);
        spawn(fx, MOTE, 0, [s0, s1], SPAWN_RA[1]);
    }
    fx.leave(0x40);
}

/// 0x1E01E10(D, step, arg): the first free task cleared and set off at
/// D+0x250 with the colour D+0xB1F4 (twice), angle flag D+0xB256; 1 when
/// one was free. `s`: the emitter's s0 / s1 (its prologue saves).
fn spawn(fx: &mut Fx, f: u32, arg: u32, s: [u64; 2], ra: u64) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, ra), (0x20, u64::from(D)), (0x10, s[1]), (0, s[0])]);
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

/// The first step's shared part: angle flag +0xA, +0xC = +0x50, +0x50 = 0,
/// +0x52 / +0xE / +8 / +6 random, a velocity (-20..19) / `q` per axis.
fn setup(fx: &mut Fx, t: u32, q: f32) {
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
        let v = ee::div(fx.mem.f(t + o), q);
        fx.mem.setf(t + o, v);
    }
}

/// Fade in by 0xC0 before age `until`, then out by 13 (dead under 15).
fn fade(fx: &mut Fx, t: u32, until: i32) {
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < until {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0xC0));
        }
    } else if a >= 0xF {
        fx.mem.write_u8(t + 0x63, a - 0xD);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn moves(fx: &mut Fx, t: u32) {
    for o in [0u32, 4, 8] {
        let v = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(t + 0x30 + o));
        fx.mem.setf(t + 0x10 + o, v);
    }
}

/// The age held at 100 from update 296 of the effect, then advanced.
fn age(fx: &mut Fx, t: u32) {
    if fx.mem.i16(COUNTER) >= 0x128 {
        fx.mem.set16(t + 0x54, 0x64);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01090(D, task): a dark puff. First step: size 35, a random cell
/// frame, grey 0x20, angles 225, velocity / 30, z speed (target z - node
/// z) / 37. Fades in for 63 updates; moves for its updates 11..69; the
/// cell frame +0x56 cycles 0..7; z += speed, size += 5, +0x5C += 2.
fn puff(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x420C_0000);
        let v = fx.rand();
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x20);
        fx.mem.set16(t + 0x5A, 0xE1);
        fx.mem.set16(t + 0x5C, 0xE1);
        setup(fx, t, 30.0);
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x30);
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0xB228);
        fx.node_position(h, slot, sp + 0x40);
        let dz = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48)), 37.0);
        fx.mem.setf(t + 0x3C, dz);
        fx.mem.set16(t + 0x58, 2);
    }
    fade(fx, t, 0x3F);
    if (0xB..0x46).contains(&fx.mem.i16(t + 0x54)) {
        moves(fx, t);
    }
    let c = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, c);
    if fx.mem.i16(t + 0x56) >= 8 {
        fx.mem.set16(t + 0x56, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x3C));
        fx.mem.setf(t + 0x18, z);
        let s = ee::add(fx.mem.f(t + 0x48), 5.0);
        fx.mem.setf(t + 0x48, s);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let a = fx.mem.i16(t + 0x5C) + 2;
        fx.mem.set16(t + 0x5C, a);
    }
    age(fx, t);
    fx.leave(0x50);
}

/// 0x1E01400(D, task): a grey mote. First step: size 35, a random cell
/// frame, grey 0x80, angle 225 / random, velocity / 60, a drift (node
/// 0x13 - node 0xD) / 8 in z and y. Fades in for 10 updates; moves for its
/// first 70; z / y += drift, size += 2, +0x5A += 2.
fn mote(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x420C_0000);
        let v = fx.rand();
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x5A, 0xE1);
        let a = rand_mod(fx, 0x168);
        fx.mem.set16(t + 0x5C, a);
        setup(fx, t, 60.0);
        for (at, out) in [(0xB228u32, 0x30u32), (0xB22A, 0x40)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + at);
            fx.node_position(h, slot, sp + out);
        }
        let dz = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48)), 8.0);
        fx.mem.setf(t + 0x3C, dz);
        let dy = ee::div(ee::sub(fx.mem.f(sp + 0x34), fx.mem.f(sp + 0x44)), 8.0);
        fx.mem.setf(t + 0x4C, dy);
        fx.mem.set16(t + 0x58, 2);
    }
    fade(fx, t, 0xA);
    if fx.mem.i16(t + 0x54) < 0x46 {
        moves(fx, t);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x3C));
        fx.mem.setf(t + 0x18, z);
        let y = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x4C));
        fx.mem.setf(t + 0x14, y);
        let s = ee::add(fx.mem.f(t + 0x48), 2.0);
        fx.mem.setf(t + 0x48, s);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let a = fx.mem.i16(t + 0x5A) + 2;
        fx.mem.set16(t + 0x5A, a);
    }
    age(fx, t);
    fx.leave(0x50);
}
