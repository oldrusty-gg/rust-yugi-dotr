//! BMODULE b515 (card 639), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01570, data area 0x1ED0000) [F]: b278's family with 500 tasks (0x70 bytes at +0x2A0) drawn as
//! rings of camera-facing quads (98b02c86a827 at 0x1E00920,
//! `billboard::ring_facing`, UVs by uv_rect 0x1E00080). From update 120 to
//! 380 three or four (odd / even updates) yellow motes a update leave the
//! attacker's nodes 0x32..0x5C (0x1E01130: drifting to the target, falling
//! by 0.2, growing by 4.3); from 28 to 340 as many rings of six quads
//! spin there (0x1E013C0). Returns 1 on update 190, 3 on 440, 4 on 445, 6
//! on 201..429, 8 once the 449-update timer +0xDDB0 has run out.
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 500;
/// The step functions a task holds (+0x44), as the record's addresses.
const MOTE: u32 = 0x01E0_1130;
const RING: u32 = 0x01E0_13C0;
/// The entry's `jal` / `jalr` returns (0x1E01920 + 8, 0x1E01950 + 8):
/// the emitter's and the steps' prologue saves (and the ring step's rand
/// frames) are the rings' stale TEX0 / TEX1 when the texture is missing.
const EMIT_RA: u64 = 0x01E0_1928;
const TASK_RA: u64 = 0x01E0_1958;
/// Data-area fields above 0x10000 (the original's `lui 1` pairs).
const TIMER: u32 = D + 0xDDB0;
const NODES: u32 = D + 0xDDE8;
const COUNTER: u32 = D + 0xDE1E;
const BYTES: u32 = D + 0xDE20;
const BYTES2: u32 = D + 0xDF20;

/// The entry 0x1E01570.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    fx.mem.write_u32(D + 0xDDBC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xE190);
    l2::sine_table(fx, D + 0xE020);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xDE1C, param);
    fx.mem.set32(TIMER, 0x1C1);
    for (k, (node, clear)) in [(0x32, 1), (0x35, 0), (0x38, 0), (0x41, 0), (0x4A, 0), (0x50, 0), (0x5C, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xDDF8, r);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0xDD90 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x70);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x74)), 180.0);
    fx.mem.set16(D + 0xDE1A, ee::fixsfsi(y));
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
        fx.mem.set8(BYTES2 + k, h - 0x14);
    }
    for k in 100..0xB4u32 {
        fx.mem.write_u8(BYTES2 + k, 0);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) != 0 && fx.mem.read_u32(t + 0x2E4) != 0 {
            match fx.mem.read_u32(t + 0x2E4) {
                MOTE => mote(fx, t + 0x2A0),
                RING => ring(fx, t + 0x2A0),
                f => panic!("b515: unknown task step {f:#x}"),
            }
        }
    }
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        for (f, local) in [(MOTE, 0x50u32), (RING, 0x60)] {
            if fx.mem.read_u32(t + 0x2E4) != f {
                continue;
            }
            let sub = fx.mem.i16(t + 0x2F6);
            l2::rect(fx, sp + local, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            let tex = fx.mem.read_u32(D + 0xDD90);
            fx.mem.write_u32(D + 0x88, tex);
            let table = if f == MOTE {
                fx.mem.set16(D + 0xC0, 5);
                fx.mem.set16(D + 0xB8, 1);
                fx.mem.set16(D + 0xBE, 0);
                BYTES
            } else {
                fx.mem.set16(D + 0xB8, 6);
                fx.mem.set16(D + 0xC0, 0);
                BYTES2
            };
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x2B0 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, v);
            }
            billboard::ring_facing(fx, uv::uv_rect, D, t + 0x2A0, 0x10, table);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xBE => 1,
        0x1BD => 4,
        0x1B8 => 3,
        0xC9..=0x1AD => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E00E90(D): the update counter and the two emitters: three (odd
/// updates, nodes D+0xDDE8..) or four (even, nodes D+0xDDEE..) tasks a
/// update, each at its node.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x40, EMIT_RA), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    for (range, f) in [(0x78..0x17D, MOTE), (0x1C..0x155, RING)] {
        let n = fx.mem.i16(COUNTER);
        if !range.contains(&n) {
            continue;
        }
        let count = if n % 2 == 1 { 3 } else { 4 };
        for k in 0..count {
            let base = if fx.mem.i16(COUNTER) % 2 == 1 { NODES } else { NODES + 6 };
            let h = fx.self_handle();
            let slot = fx.mem.i16(base + 2 * k);
            fx.node_position(h, slot, D + 0x250);
            spawn(fx, f, 0);
        }
    }
    fx.leave(0x50);
}

/// 0x1E01BD0(D, step, arg): the first free task cleared and set off at
/// D+0x250 with the colour D+0xDDB4 (twice), angle flag D+0xDE16; 1 when
/// one was free.
fn spawn(fx: &mut Fx, f: u32, arg: u32) -> i32 {
    fx.enter(0x60);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(D + 0xDDB4), fx.mem.read_u8(D + 0xDDB5), fx.mem.read_u8(D + 0xDDB6));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(D + 0xDDB7);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(D + 0xDE16);
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

/// 0x1E01130(D, task): a mote. First step: size 35, a random cell frame
/// (-7..7), yellow-white, angle 0 / random, a velocity (-20..19) / (50, 60,
/// 60), z speed (target z - node z) / 40. Fades in by 0xC0 for 47
/// updates, then out by 14; from its second update moves, falls by 0.2,
/// z += speed, size += 4.3.
fn mote(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x420C_0000);
        let v = fx.rand();
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x80);
        fx.mem.set16(t + 0x5A, 0);
        let a = rand_mod(fx, 0x168);
        fx.mem.set16(t + 0x5C, a);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x52, 0);
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
        let slot = fx.mem.i16(NODES);
        fx.node_position(h, slot, sp + 0x40);
        let dz = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48)), 40.0);
        fx.mem.setf(t + 0x3C, dz);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x2F {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0xC0));
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
        fx.mem.setf(t + 0x10, x);
        let y = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
        fx.mem.setf(t + 0x14, ee::sub(y, f32::from_bits(0x3E4C_CCCD)));
        let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x38));
        fx.mem.setf(t + 0x18, z);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x3C));
        fx.mem.setf(t + 0x18, z);
        let s = ee::add(fx.mem.f(t + 0x48), f32::from_bits(0x4089_999A));
        fx.mem.setf(t + 0x48, s);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x50);
}

/// 0x1E013C0(D, task): a spinning ring (drawn as 6 quads). First step:
/// size 40, cell frame / orientation rand % 8 (signed), yellow-white, a
/// random angle, radius +0x50 = 10..59 growing by +0xE (2..9) a update,
/// life +6 (2..5) updates at full alpha, then fading by 8; size += 3 a
/// update. Its rand frames stay below its own.
fn ring(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    let _ = sp;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4220_0000);
        let v = fx.rand_from(t, 0x01E0_13E8);
        fx.mem.set16(t + 0x56, v % 8);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x80);
        let v = fx.rand_from(t, 0x01E0_141C);
        let (_, h) = fx.div(v, 0x168);
        fx.mem.set16(t + 0x5A, h);
        let v = fx.rand_from(t, 0x01E0_1438);
        fx.mem.set16(t + 0xA, v % 8);
        let a = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, a);
        let v = fx.rand_from(t, 0x01E0_145C);
        let (_, h) = fx.div(v, 0x32);
        fx.mem.set16(t + 0x50, h + 0xA);
        let v = fx.rand_from(t, 0x01E0_147C);
        fx.mem.set16(t + 0xE, v % 8 + 2);
        let v = fx.rand_from(t, 0x01E0_149C);
        let (_, h) = fx.div(v, 0x23);
        fx.mem.set16(t + 8, h);
        let v = fx.rand_from(t, 0x01E0_14B8);
        fx.mem.set16(t + 6, v % 4 + 2);
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
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let s = ee::add(fx.mem.f(t + 0x48), 3.0);
    fx.mem.setf(t + 0x48, s);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}
