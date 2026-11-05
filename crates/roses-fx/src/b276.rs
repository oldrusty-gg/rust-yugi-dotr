//! BMODULE b276 (card 332), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01760, data area 0x1ED0000) [F]: b278's family (400 tasks at +0x2A0, rings of one camera-facing
//! quad each by 98b02c86a827 at 0x1E00920, `billboard::ring_facing`).
//! From update 158 to 250 two yellow motes a update leave the attacker's
//! node 0x13 (0x1E01060), from 200 to 350 two more start 60 above the
//! target (0x1E013C0); both drift (z by (target z - node z) / 70 a
//! update), grow by 2 a update and fade; the first rise by 1 and are cut
//! at update 246, the second sink by 2 to height 35. Returns 1 on update
//! 215, 3 on 370, 6 on 226..359, 8 once the 377-update timer +0xB1F0 has
//! run out.
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const RISE: u32 = 0x01E0_1060;
const SINK: u32 = 0x01E0_13C0;
/// The entry's `jal` / `jalr` returns (0x1E01A50 + 8, 0x1E01A80 + 8):
/// the emitter's and the steps' prologue saves are the rings' stale TEX0
/// / TEX1 when the texture is missing.
const EMIT_RA: u64 = 0x01E0_1A58;
const TASK_RA: u64 = 0x01E0_1A88;
/// Data-area fields above 0x10000 (the original's `lui 1` pairs).
const TIMER: u32 = D + 0xB1F0;
const COUNTER: u32 = D + 0xB25E;
const BYTES: u32 = D + 0xB260;

/// The entry 0x1E01760.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    fx.mem.write_u32(D + 0xB1FC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB5D0);
    l2::sine_table(fx, D + 0xB460);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB25C, param);
    fx.mem.set32(TIMER, 0x179);
    for (at, node, clear) in [(0xB228u32, 0x13, 1), (0xB22A, 0x1C, 0), (0xB22C, 0xD, 0)] {
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

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) != 0 && fx.mem.read_u32(t + 0x2E4) != 0 {
            let f = fx.mem.read_u32(t + 0x2E4);
            mote(fx, t + 0x2A0, f == SINK);
        }
    }
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        for (f, local) in [(RISE, 0x50u32), (SINK, 0x60)] {
            if fx.mem.read_u32(t + 0x2E4) != f {
                continue;
            }
            let sub = fx.mem.i16(t + 0x2F6);
            l2::rect(fx, sp + local, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + local + 4 * k);
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
        0xD7 => 1,
        0x172 => 3,
        0xE2..=0x167 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E00E90(D): the update counter and the two emitters.
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, EMIT_RA), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if (0x9E..0xFB).contains(&fx.mem.i16(COUNTER)) {
        for _ in 0..2 {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + 0xB228);
            fx.node_position(h, slot, D + 0x250);
            let y = ee::sub(fx.mem.f(D + 0x254), 0.5);
            fx.mem.setf(D + 0x254, y);
            spawn(fx, RISE, 0);
        }
    }
    if (0xC8..0x15F).contains(&fx.mem.i16(COUNTER)) {
        for _ in 0..2 {
            let h = fx.target_handle();
            fx.unit_position(h, D + 0x250);
            let y = ee::add(fx.mem.f(D + 0x254), 60.0);
            fx.mem.setf(D + 0x254, y);
            spawn(fx, SINK, 0);
        }
    }
    fx.leave(0x40);
}

/// 0x1E01CF0(D, step, arg): the first free task cleared and set off at
/// D+0x250 with the colour D+0xB1F4 (twice), angle flag D+0xB256; 1 when
/// one was free.
fn spawn(fx: &mut Fx, f: u32, arg: u32) -> i32 {
    fx.enter(0x60);
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

/// 0x1E01060 (`sink` false) / 0x1E013C0 (true)(D, task): a mote. First
/// step: size 35, a random cell frame (-7..7), yellow-white, random
/// angles, a velocity (-20..19) / (50, 60, 60), z speed (target z - node
/// z) / 70; size += 2 a update. 0x1E01060: fades in by 0xC0 for 65
/// updates then out by 11, moves for its updates 11..69, rises by 1, cut
/// at update 246. 0x1E013C0: fades in for 60, moves from its update 9,
/// sinks by 2 while above 35 (else moves again), its age held at 60 from
/// update 351.
fn mote(fx: &mut Fx, t: u32, sink: bool) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x420C_0000);
        let v = fx.rand();
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x80);
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
        let v = rand_mod(fx, if sink { 0x23 } else { 0x1E });
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, 5);
        fx.mem.set16(t + 6, v + 3);
        for o in [0x30u32, 0x34, 0x38] {
            let v = rand_mod(fx, 0x28);
            fx.mem.setf(t + o, ee::from_i32(v - 0x14));
        }
        for (o, q) in [(0x30u32, 50.0), (0x34, 60.0), (0x38, 60.0)] {
            let v = ee::div(fx.mem.f(t + o), q);
            fx.mem.setf(t + o, v);
        }
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x30);
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0xB228);
        fx.node_position(h, slot, sp + 0x40);
        let dz = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48)), 70.0);
        fx.mem.setf(t + 0x3C, dz);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < if sink { 0x3C } else { 0x41 } {
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
    let age = fx.mem.i16(t + 0x54);
    let moving = if sink { age >= 9 } else { (0xB..0x46).contains(&age) };
    if moving {
        move_xz(fx, t);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        if sink {
            let y = fx.mem.f(t + 0x14);
            if ee_le(y, 35.0) {
                move_xz(fx, t);
            } else {
                fx.mem.setf(t + 0x14, ee::sub(y, 2.0));
            }
        } else {
            let y = ee::add(fx.mem.f(t + 0x14), 1.0);
            fx.mem.setf(t + 0x14, y);
        }
        let s = ee::add(fx.mem.f(t + 0x48), 2.0);
        fx.mem.setf(t + 0x48, s);
    }
    if sink {
        let n = fx.mem.i16(t + 0x54) + 1;
        fx.mem.set16(t + 0x54, n);
        if fx.mem.i16(COUNTER) >= 0x15F {
            fx.mem.set16(t + 0x54, 0x3C);
        }
    } else {
        if fx.mem.i16(COUNTER) >= 0xF6 {
            fx.mem.set16(t + 0x58, 0);
        }
        let n = fx.mem.i16(t + 0x54) + 1;
        fx.mem.set16(t + 0x54, n);
    }
    fx.leave(0x50);
}

/// x += vx, z += vz.
fn move_xz(fx: &mut Fx, t: u32) {
    for o in [0u32, 8] {
        let v = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(t + 0x30 + o));
        fx.mem.setf(t + 0x10 + o, v);
    }
}
