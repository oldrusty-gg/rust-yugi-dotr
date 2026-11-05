//! BMODULE b520 (card 284), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01730, data area 0x1ED0000) [F]: b278's task pool (400 tasks of 0x70 bytes at D+0x2A0, alive
//! +0x58, step +0x44). From update 128 to 311 three violet sparks a update
//! leave the attacker's node 0x1D (0x1E01230), drifting toward node 0x25
//! in x for 20 updates, 3 a update in z (by the facing) and slowly rising,
//! spinning; each is drawn as a ring of 1..2 camera-facing quads turned
//! about z (9dae9a5cb92b at 0x1E00920, `quads4::ring_quads_z`, UVs by
//! uv_rect 0x1E00080). The record's other draws (0x1E01700: four quads,
//! 0x1E01710: arc strips 0x1E00ED0) belong to empty steps nothing spawns.
//! Returns 1 on update 140, 4 on 360, 2 on 355, 6 on 151..349, 8 once the
//! 370-update timer +0xB210 has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task can hold (+0x44), as the record's addresses:
/// the spark, two that return at once (drawn as quads / arcs) and one the
/// entry skips.
const SPARK: u32 = 0x01E0_1230;
const QUADS: u32 = 0x01E0_1700;
const ARCS: u32 = 0x01E0_1710;
const SKIPPED: u32 = 0x01E0_1720;
/// The entry's `jalr` return (0x1E01B4C + 8): the step's prologue saves
/// its ra at the entry's sp - 0x30 (the quads' stale TEX1 when the
/// texture is missing).
const TASK_RA: u64 = 0x01E0_1B54;
/// Data-area fields above 0x10000 (the original's `lui 1` pairs).
const TIMER: u32 = D + 0xB210;
const NODE_POS: u32 = D + 0xB1D0;
const FACING: u32 = D + 0xB2B4;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;

/// The entry 0x1E01730.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, D + 0xB4BC);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x172);
    for (k, (node, clear)) in [(0x1D, 1), (0x25, 0), (0x35, 0), (0x30, 0), (0x2F, 0), (0x2C, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + 0xB244 + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x298 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0xB1F0 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x70);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x74)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rand_mod(fx, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0xB4..0x100u32 {
        let v = rand_mod(fx, 0xA);
        fx.mem.set8(BYTES + k, v - 5);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand();
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let h = fx.self_handle();
    let slot = fx.mem.i16(D + 0xB244);
    fx.node_position(h, slot, NODE_POS);
    if (0x80..0x138).contains(&fx.mem.i16(COUNTER)) {
        for k in 0..4 {
            let w = fx.mem.read_u32(NODE_POS + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        for ra in [0x01E0_1AECu64, 0x01E0_1B00, 0x01E0_1B14] {
            spawn(fx, SPARK, 0, &[(0x50, ra), (0x30, 0), (0x20, param as i64 as u64), (0x10, u64::from(D)), (0, 0)]);
        }
    }
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x2E4) {
            0 | SKIPPED | QUADS | ARCS => {}
            SPARK => spark(fx, t + 0x2A0),
            f => panic!("b520: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x2E4);
        for (g, tex, local) in [(SPARK, 0xB1F8u32, 0x50u32), (QUADS, 0xB1F0, 0x60)] {
            if f != g {
                continue;
            }
            let tex = fx.mem.read_u32(D + tex);
            fx.mem.write_u32(D + 0x88, tex);
            let sub = fx.mem.i16(t + 0x2F6);
            l2::rect(fx, sp + local, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            let mode = if g == SPARK {
                let c = fx.mem.i16(t + 0x2AE);
                fx.mem.set16(D + 0xB8, c);
                fx.mem.set16(D + 0xBE, 0);
                0x10
            } else {
                fx.mem.set16(D + 0xB8, 4);
                let v = fx.rand();
                fx.mem.set16(D + 0xBE, v % 4);
                0x21
            };
            fx.mem.set16(D + 0xB0, 0);
            quads4::ring_quads_z(fx, uv::uv_rect, D, t + 0x2A0, mode, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == ARCS {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t + 0x2A0, 0x21, NODE_POS);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x8C => 1,
        0x168 => 4,
        0x163 => 2,
        0x97..=0x15D => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E01DF0(D, step, arg): the first free task cleared and set off at
/// D+0x250 with the colour D+0xB214 (twice), angle flag D+0xB2B0; 1 when
/// one was free. `saves`: its prologue's (the entry's registers).
fn spawn(fx: &mut Fx, f: u32, arg: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(D + 0xB214), fx.mem.read_u8(D + 0xB215), fx.mem.read_u8(D + 0xB216));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(D + 0xB217);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(D + 0xB2B0);
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

/// 0x1E01230(D, task): a violet spark. First step: +8 3..5 (+0xA = +8 +
/// 4), 1..2 quads (+0xE), size 13..25, cell frame 2..5 (signed), violet,
/// a rise / drift (-50..49) / 400 (+0x34 / +0x30), radius 0, spin
/// direction +0x5C, angle +0x5A random, spin speeds +4 / +2 / +0 in
/// -1024..1023 over random +0x20 / +0x24 / +0x28 (after the camera
/// angles there), x step += (node 0x25 - node 0x1D) / 7. Fades in by 0x60
/// for 64 updates, then out by 15; every third age the size grows by 1 up
/// to 11; z += 3 (by the facing) and y += rise - 0.09; x += step for 20
/// updates; angles += speeds; from update 351 of the effect its age is held
/// at 90; +0x5A += 2 or -= 2; radius += 4; size += 1.6.
fn spark(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x30, TASK_RA), (0x20, u64::from(t - 0x2A0)), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rand_mod(fx, 3);
        fx.mem.set16(t + 8, v + 3);
        let w = fx.mem.i16(t + 8) + 4;
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand();
        fx.mem.set16(t + 0xE, (v >> 4) % 2 + 1);
        let v = rand_mod(fx, 0xD);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xD));
        let v = fx.rand();
        fx.mem.set16(t + 0x56, (v >> 4) % 4 + 2);
        l2::rgb(fx, t + 0x60, 0x80, 0x10, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = rand_mod(fx, 0x64);
        fx.mem.setf(t + 0x34, ee::from_i32(v - 0x32));
        let v = rand_mod(fx, 0x64);
        fx.mem.setf(t + 0x30, ee::from_i32(v - 0x32));
        for o in [0x34u32, 0x30] {
            let v = ee::div(fx.mem.f(t + o), 400.0);
            fx.mem.setf(t + o, v);
        }
        fx.mem.set16(t + 0x50, 0);
        let v = fx.rand();
        fx.mem.set16(t + 0x5C, v % 2);
        let v = rand_mod(fx, 0x168);
        fx.mem.set16(t + 0x5A, v);
        l2::camera_angles_to(fx, t + 0x20);
        for o in [4u32, 2, 0] {
            let v = fx.rand();
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
        for o in [0x20u32, 0x24, 0x28] {
            let v = rand_mod(fx, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        for (at, out) in [(0xB244u32, 0x40u32), (0xB246, 0x50)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + at);
            fx.node_position(h, slot, sp + out);
        }
        let dx = ee::div(ee::sub(fx.mem.f(sp + 0x50), fx.mem.f(sp + 0x40)), 7.0);
        let x = ee::add(fx.mem.f(t + 0x3C), dx);
        fx.mem.setf(t + 0x3C, x);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x40 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x60));
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x63, a - 0xF);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let age = fx.mem.i16(t + 0x54);
    if fx.div(age, 3).1 == 1 {
        let s = fx.mem.f(t + 0x48);
        if s < 11.0 {
            fx.mem.setf(t + 0x48, ee::add(s, 1.0));
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 3.0) } else { ee::add(z, 3.0) };
        fx.mem.setf(t + 0x18, z);
        let rise = ee::sub(fx.mem.f(t + 0x34), f32::from_bits(0x3DB8_51EC));
        let y = ee::add(fx.mem.f(t + 0x14), rise);
        fx.mem.setf(t + 0x14, y);
    }
    if (1..0x14).contains(&fx.mem.i16(t + 0x54)) {
        let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x3C));
        fx.mem.setf(t + 0x10, x);
    }
    for (o, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.i32(t + o).wrapping_add(fx.mem.i16(t + s));
        fx.mem.set32(t + o, v);
    }
    if fx.mem.i16(COUNTER) >= 0x15F {
        fx.mem.set16(t + 0x54, 0x5A);
    }
    let d = if fx.mem.i16(t + 0x5C) != 0 { 2 } else { -2 };
    let a = fx.mem.i16(t + 0x5A) + d;
    fx.mem.set16(t + 0x5A, a);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + 4;
        fx.mem.set16(t + 0x50, r);
    }
    let s = ee::add(fx.mem.f(t + 0x48), f32::from_bits(0x3FCC_CCCD));
    fx.mem.setf(t + 0x48, s);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x60);
}
