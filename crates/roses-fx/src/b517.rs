//! BMODULE b517 (card 549), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01B10, data area 0x1ED0000) [F]: b278's layout (400 tasks at +0x2A0). From update 62 to 205 two
//! blue motes a update leave the attacker's node 0x1D (0x1E015D0, rings
//! of one camera-facing quad by 98b02c86a827 at 0x1E00920,
//! `billboard::ring_facing`), drifting toward the target (z by (target z -
//! node z) / 55 a update), growing by 2; on update 62 two white rings of
//! ten quads (0x1E01960, 7369422df7f8 at 0x1E00E90,
//! `billboard::ring_quads`) open at its nodes 0x2F and 0x3F. Returns 1 on
//! update 80, 3 on 250, 6 on 91..239, 8 once the 260-update timer +0xB1F0
//! has run out.
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const MOTE: u32 = 0x01E0_15D0;
const RING: u32 = 0x01E0_1960;
/// The entry's `jal` / `jalr` returns (0x1E01E20 + 8, 0x1E01E50 + 8):
/// the emitter's and the steps' prologue saves (and the ring step's rand
/// frames) are the draws' stale TEX0 / TEX1 when the texture is missing.
const EMIT_RA: u64 = 0x01E0_1E28;
const TASK_RA: u64 = 0x01E0_1E58;
/// Data-area fields above 0x10000 (the original's `lui 1` pairs).
const TIMER: u32 = D + 0xB1F0;
const COUNTER: u32 = D + 0xB25E;
const BYTES: u32 = D + 0xB260;

/// The entry 0x1E01B10.
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
    fx.mem.set32(TIMER, 0x104);
    for (at, node, clear) in [(0xB228u32, 0x1D, 1), (0xB22A, 0x2F, 0), (0xB22C, 0x3F, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB238, r);
    for k in 0..6u32 {
        let t = fx.texture(4, 0xA + k as i32);
        fx.mem.write_u32(D + 0x288 + 4 * k, t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
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
        fx.mem.write_u8(BYTES + k, 0);
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
            match fx.mem.read_u32(t + 0x2E4) {
                MOTE => mote(fx, t + 0x2A0),
                RING => ring(fx, t + 0x2A0),
                f => panic!("b517: unknown task step {f:#x}"),
            }
        }
    }
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x2E4) == MOTE {
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
        if fx.mem.read_u32(t + 0x2E4) == RING {
            let tex = fx.mem.read_u32(D + 0xB1D0);
            fx.mem.write_u32(D + 0x88, tex);
            let sub = fx.mem.i16(t + 0x2F6);
            l2::rect(fx, sp + 0x60, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + 0x60 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t + 0x2A0, 0x10, BYTES);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x50 => 1,
        0xFA => 3,
        0x5B..=0xEF => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E01440(D): the update counter, two motes a update at node +0xB228
/// (half a unit lower) and, once, two rings at nodes +0xB22A / +0xB22C.
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, EMIT_RA), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if (0x3E..0xCE).contains(&fx.mem.i16(COUNTER)) {
        for _ in 0..2 {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + 0xB228);
            fx.node_position(h, slot, D + 0x250);
            let y = ee::sub(fx.mem.f(D + 0x254), 0.5);
            fx.mem.setf(D + 0x254, y);
            spawn(fx, MOTE, 0);
        }
    }
    if fx.mem.i16(COUNTER) == 0x3E {
        for node in [0xB22Au32, 0xB22C] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + node);
            fx.node_position(h, slot, D + 0x250);
            spawn(fx, RING, 0);
        }
    }
    fx.leave(0x40);
}

/// 0x1E020A0(D, step, arg): the first free task cleared and set off at
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

/// 0x1E015D0(D, task): a mote. First step: size 35, a random cell frame
/// (-7..7), blue (0x80 0x80 0xFF), random angles, a velocity (-20..19) /
/// (50, 60, 60), z speed (target z - node z) / 55. Fades in by 0xC0 for
/// 70 updates (until update 235 of the effect), then out by 11; moves in
/// its updates 11..69; z += speed, size += 2 a update.
fn mote(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x420C_0000);
        let v = fx.rand();
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
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
        for (o, q) in [(0x30u32, 50.0), (0x34, 60.0), (0x38, 60.0)] {
            let v = ee::div(fx.mem.f(t + o), q);
            fx.mem.setf(t + o, v);
        }
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x30);
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0xB228);
        fx.node_position(h, slot, sp + 0x40);
        let dz = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48)), 55.0);
        fx.mem.setf(t + 0x3C, dz);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) < 0xEB && fx.mem.i16(t + 0x54) < 0x46 {
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
    if fx.mem.i16(t + 0x54) != 0 {
        let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x3C));
        fx.mem.setf(t + 0x18, z);
        let s = ee::add(fx.mem.f(t + 0x48), 2.0);
        fx.mem.setf(t + 0x48, s);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x50);
}

/// 0x1E01960(D, task): an opening ring. First step: size 100, cell frame
/// 3, white, radius +0x50 = 10..24 growing by 10 a update, a random angle,
/// UV size +8 = 1 + rand % 8 (signed), life +6 = 30..44 at alpha 0xC0,
/// then fading by 11; flickers white / grey when +0x52 is 1. Its rand
/// frames stay below its own.
fn ring(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    let _ = sp;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        fx.mem.set16(t + 0x56, 3);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let v = fx.rand_from(t, 0x01E0_19A8);
        let (_, h) = fx.div(v, 0xF);
        fx.mem.set16(t + 0x50, h + 0xA);
        let v = fx.rand_from(t, 0x01E0_19C8);
        let (_, h) = fx.div(v, 0x168);
        fx.mem.set16(t + 0x5A, h);
        fx.mem.set16(t + 0xE, 0xA);
        let v = fx.rand_from(t, 0x01E0_19EC);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = fx.rand_from(t, 0x01E0_1A0C);
        let (_, h) = fx.div(v, 0xF);
        fx.mem.set16(t + 6, h + 0x1E);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xD {
            fx.mem.write_u8(t + 0x63, a - 0xB);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if fx.mem.i16(t + 0x52) == 1 {
        let c = if fx.mem.i16(t + 0x54) % 2 == 1 { 0xFF } else { 0x80 };
        l2::rgb(fx, t + 0x60, c, c, c);
    }
    fx.leave(0x20);
}
