//! BMODULE b589 (card 450), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02AC0, data area 0x1ED0000, 0x7180 bytes) [F]
//!: b205's layout (150 tasks of 0x40 at D+0x380
//! by direct `jal`s, 150 of 0x70 at D+0x2900 by `jalr`, the globals
//! D+0x6AA0..D+0x7010 word for word; the tasks 0x1E02A00 counted, never
//! spawned) with the trail on pool B: update 0x6E a green trail
//! (0x1E01AB0) along nodes 0x16 / 0x17 drawn by the record's tube
//! (0x1E03470: one ring of four strips a segment, the task's nudge +0x48
//! falling by 0.09 a segment); 0x75 at node 0x16 halfway to the target
//! eight green rings (b701's red ring), fifty pale green fliers (b205's),
//! b205's arc ring, two green light planes (b544's) and a camera light;
//! 0x62 an object op task (0x1F8380(2, ..), to update 0x88). The steps
//! 0x1E02240 / 0x1E02400 / 0x1E025C0 are drawn but never spawned (not
//! ported). Returns 1 on update 0x67, 4 on 0xB4, 2 on 0x8C, 6 on
//! 0x72..0x86, 8 once the 0xBE-update timer D+0x6DA0 has run out.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, exe::camera_slots45, quads4, spawn::history, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS_A: u32 = 150;
const TASKS_B: u32 = 150;
/// The 0x40-byte tasks.
const POOL_A: u32 = D + 0x380;
/// The 0x70-byte tasks.
const POOL_B: u32 = D + 0x2900;
/// Pool A's step.
const RING: u32 = 0x01E0_1CE0;
/// Pool B's steps.
const PLANE: u32 = 0x01E0_1920;
const TRAIL: u32 = 0x01E0_1AB0;
const FLY: u32 = 0x01E0_1E70;
const ARC: u32 = 0x01E0_2100;
const CAMERA: u32 = 0x01E0_2800;
const OBJECT: u32 = 0x01E0_2980;
/// Counted by the pool B loop, never spawned.
const IDLE: u32 = 0x01E0_2A00;
/// The return address of the pool B loop's `jalr` (0x1E02DD4 + 8).
const TASK_RA: u64 = 0x01E0_2DDC;
const POS: u32 = D + 0x2D0;
const TEX4: u32 = D + 0x340;
const CAM: u32 = D + 0x6AA0;
const TRAIL_Q: u32 = D + 0x6AF0;
const TRAIL_P: u32 = D + 0x6B90;
const TEX: u32 = D + 0x6D80;
const TIMER: u32 = D + 0x6DA0;
const COLOUR: u32 = D + 0x6DA4;
const NODES: u32 = D + 0x6DEC;
const TURN: u32 = D + 0x6DFE;
const KIND: u32 = D + 0x6E06;
const FACING: u32 = D + 0x6E0A;
const COUNTER: u32 = D + 0x6E0C;
const BYTES: u32 = D + 0x6E10;
const SINES: u32 = D + 0x7010;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02AC0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x170);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x6DAC, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x170);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x7180);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x6E08, param);
    fx.mem.set32(TIMER, 0xBE);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, node) in [1, 0x16, 0x17, 5, 5, 6].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // 0x1E02A10(0, D): the textures.
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(TEX4 + 4 * k as u32, t);
    }
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(TEX4 + 4 * k as u32, t);
    }
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.fix_residue(y, 0x01E0_2C0C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2C18, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2C4C, 0x1E);
        fx.mem.set8(BYTES + 0x100 + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_2C80, 0x14);
        fx.mem.set8(BYTES + 0x100 + k, v - 0xA);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS_A {
        let t = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == RING {
            let s = fx.enter(0x20);
            fx.saved(s, &[(0x10, 0x01E0_2CF0), (0, u64::from(D))]);
            ring(fx, t);
            fx.leave(0x20);
        }
    }
    let h = fx.target_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x60, u64::from(D), 0x01E0_2D14);
    for k in 0..TASKS_A {
        let t = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == RING {
            let w = fx.mem.read_u32(TEX4 + 0x3C);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            cell(fx, sp + 0x70, 0x9B, sub);
            billboard::ring_t20(fx, uv::uv_rect4, D, t, sp + 0xE0, BYTES);
        }
    }
    // The entry's s1: the idle tasks counted here (none are spawned).
    let mut idle = 0u32;
    for k in 0..TASKS_B {
        let slot = D + 0x70 * k;
        let t = POOL_B + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        if f == IDLE {
            idle += 1;
            continue;
        }
        // The steps' prologues: the entry's s0 = D, s1 the idle count, s2
        // 0, s3 the slot.
        match f {
            FLY | PLANE => {
                let s = fx.enter(0x20);
                fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
                if f == FLY {
                    fly(fx, t, idle);
                } else {
                    plane(fx, t);
                }
                fx.leave(0x20);
            }
            TRAIL => {
                let s = fx.enter(0x60);
                fx.saved(s, &[(0x40, TASK_RA), (0x30, u64::from(slot)), (0x20, 0), (0x10, u64::from(idle)), (0, u64::from(D))]);
                trail(fx, t, idle);
                fx.leave(0x60);
            }
            OBJECT => {
                let s = fx.enter(0x40);
                fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(idle)), (0, u64::from(D))]);
                object(fx, t);
                fx.leave(0x40);
            }
            _ => {
                let s = fx.enter(0x30);
                fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(idle)), (0, u64::from(D))]);
                match f {
                    ARC => arc(fx, t),
                    CAMERA => camera(fx, t),
                    _ => panic!("b589: unknown task step {f:#x}"),
                }
                fx.leave(0x30);
            }
        }
    }
    for k in 0..TASKS_B {
        let t = POOL_B + 0x70 * k;
        // The original reads the cell frame of pool A's task k (+0x34).
        let ta = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == PLANE {
            let w = fx.mem.read_u32(TEX4 + 0x3C);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, sp + 0x80, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
        }
        if f(fx) == TRAIL {
            if fx.mem.i16(t + 0x50) == 0 {
                fx.mem.write_u32(D + 4, 0);
            }
            if fx.mem.i16(t + 0x50) == 1 {
                fx.mem.write_u32(D + 4, 0x3F80_0000);
            }
            fx.mem.write_u32(D, 0x3FC0_0000);
            let z = if fx.mem.i16(FACING) != 0 { 0xC040_0000 } else { 0x4040_0000 };
            fx.mem.write_u32(D + 8, z);
            if fx.mem.i16(t + 0x50) == 0 {
                tube(fx, t, TRAIL_P, TRAIL_Q);
            }
        }
        if f(fx) == FLY {
            let w = fx.mem.read_u32(TEX4 + 0x3C);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(ta + 0x34);
            cell(fx, sp + 0x90, 0x9B, sub);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
        }
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x67 => 1,
        0xB4 => 4,
        0x8C => 2,
        0x72..=0x86 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` frame `sub` (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E03200(D, step, 0) (b205's 0x1E02A80): the first free 0x40-byte
/// task set off.
fn spawn_a(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS_A).find(|&k| fx.mem.i16(POOL_A + 0x40 * k + 0x36) == 0) {
        let t = POOL_A + 0x40 * k;
        fx.bzero(t, 0x40);
        fx.mem.set16(t + 0x36, 1);
        colour(fx, t, [0x28, 0x2C]);
        l2::to_i16x4(fx, t, POS, 0);
        let s = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x30, s);
        fx.mem.write_u32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, step);
    }
    fx.leave(0x60);
}

/// 0x1E03320(D, step, 0): b205's 0x1E02BA0 with the turn D+0x6DFE at
/// +0x5A.
fn spawn_b(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS_B).find(|&k| fx.mem.i16(POOL_B + 0x70 * k + 0x58) == 0) {
        let t = POOL_B + 0x70 * k;
        fx.bzero(t, 0x70);
        fx.mem.set16(t + 0x58, 1);
        colour(fx, t, [0x60, 0x64]);
        l2::to_i16x4(fx, t, POS, 0);
        for k in 0..3 {
            let w = fx.mem.read_u32(POS + 4 * k);
            fx.mem.write_u32(t + 0x10 + 4 * k, w);
        }
        let s = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x50, s);
        let s = fx.mem.i16(TURN);
        fx.mem.set16(t + 0x5A, s);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

fn colour(fx: &mut Fx, t: u32, at: [u32; 2]) {
    for o in at {
        let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
        l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
        let a = fx.mem.read_u8(COLOUR + 3);
        fx.mem.write_u8(t + o + 3, a);
    }
}

/// 0x1E01760(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the spawns (frame 0x50: ra, the entry's s1 = 0, s0 = D; then s0 =
/// D, s1 a loop count). The spawns' saves: s0 = D, s1, the entry's s2 =
/// 0, s3 = the parameter, s4 the runner's (0 [H]).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_2CBC), (0x10, 0), (0, u64::from(D))]);
    let p = param as i64 as u64;
    let saves = |s1: u64| [u64::from(D), s1, 0, p, 0];
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let mut s1 = 0u64;
    if c(fx) == 0x6E {
        fx.mem.set16(KIND, 0);
        spawn_b(fx, TRAIL, saves(s1), 0x01E0_17B4);
    }
    if c(fx) == 0x75 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2);
        crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(D), Some(s1), 0x01E0_17DC);
        let h = fx.target_handle();
        crate::b076::unit_position_saving(fx, h, sp + 0x40, u64::from(D), 0x01E0_17F0);
        let w = fx.mem.read_u32(sp + 0x30);
        fx.mem.write_u32(POS, w);
        let w = fx.mem.read_u32(sp + 0x34);
        fx.mem.write_u32(POS + 4, w);
        let a = fx.mem.f(sp + 0x38);
        let z = ee::add(a, ee::div(ee::sub(fx.mem.f(sp + 0x48), a), 2.0));
        fx.mem.setf(POS + 8, z);
        fx.mem.set16(KIND, 1);
        for k in 0..8u64 {
            spawn_a(fx, RING, saves(k), 0x01E0_1840);
        }
        for k in 0..0x32u64 {
            spawn_b(fx, FLY, saves(k), 0x01E0_1868);
        }
        s1 = 0x32;
        spawn_b(fx, ARC, saves(s1), 0x01E0_188C);
        fx.mem.set16(TURN, 0x2D);
        fx.mem.set16(KIND, 0);
        spawn_b(fx, PLANE, saves(s1), 0x01E0_18AC);
        fx.mem.set16(TURN, 0x87);
        fx.mem.set16(KIND, 1);
        spawn_b(fx, PLANE, saves(s1), 0x01E0_18D0);
        spawn_b(fx, CAMERA, saves(s1), 0x01E0_18E4);
    }
    if c(fx) == 0x62 {
        spawn_b(fx, OBJECT, saves(s1), 0x01E0_1908);
    }
    fx.leave(0x50);
}

/// 0x1E01920 (D, task): b544's light plane in green (0x60, 0xFF, 0x60) /
/// (0x40, 0xFF, 0x40): kind +0x50 0: 8 x 300 growing by 60, 1: 8 x 100
/// growing by 20, 2: 18 x 400; in by 0x40 for 4 updates, then out by 9.
fn plane(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0x60, 0xFF, 0x60);
        l2::rgb(fx, t + 0x64, 0x40, 0xFF, 0x40);
        for (kind, w, h) in [(0, 0x4100_0000u32, 0x4396_0000u32), (1, 0x4100_0000, 0x42C8_0000), (2, 0x4190_0000, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, w);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let v = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 4 {
        if v < 0x80 {
            fx.mem.write_u8(t + 0x63, v.wrapping_add(0x40));
        }
    } else if v >= 0xB {
        fx.mem.write_u8(t + 0x63, v - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    for (kind, dh) in [(0, 60.0f32), (1, 20.0)] {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(t + 0x54) != 0 {
            let h = ee::add(fx.mem.f(t + 0x34), dh);
            fx.mem.setf(t + 0x34, h);
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01AB0 (D, task): b205's trail on a 0x70-byte task in green (0x60,
/// 0xA0, 0x60): nudge +0x48 0.4; kind 0 the trails of nodes 0x16
/// (+0x6AF0) / 0x17 (+0x6B90) to update 0x76; alpha up by 0x12 under
/// 0x80 for 11 updates, then down by 0x1D; length +6 from 1 up to 8.
/// `idle` the entry's s1.
fn trail(fx: &mut Fx, t: u32, idle: u32) {
    let sp = fx.sp;
    l2::rgb(fx, t + 0x60, 0x60, 0xA0, 0x60);
    let mut s01 = [u64::from(D), u64::from(idle)];
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3ECC_CCCD);
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.set16(t + 0xE, 0x1E);
            for (slot, buf, ra) in [(NODES + 2, TRAIL_Q, 0x01E0_1B28u32), (NODES + 4, TRAIL_P, 0x01E0_1B78)] {
                let h = fx.self_handle();
                let s = fx.mem.i16(slot);
                crate::b076::node_position_saving(fx, h, s, sp + 0x50, s01[0], Some(s01[1]), ra);
                for k in 0..9u32 {
                    for c in 0..3 {
                        let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
                        fx.mem.write_u32(buf + 0x10 * k + 4 * c, w);
                    }
                }
                s01 = [u64::from(sp + 0x54), u64::from(sp + 0x58)];
            }
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.set16(t + 0xE, 7);
        }
        fx.mem.set16(t + 6, 1);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xB {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x12);
        }
    } else if a >= 0x1F {
        fx.mem.write_u8(t + 0x63, a - 0x1D);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x50) == 0 && fx.mem.i16(COUNTER) < 0x77 {
        for (slot, buf, ra) in [(NODES + 2, TRAIL_Q, 0x01E0_1C4Cu32), (NODES + 4, TRAIL_P, 0x01E0_1C78)] {
            let h = fx.self_handle();
            let s = fx.mem.i16(slot);
            crate::b076::node_position_saving(fx, h, s, sp + 0x50, s01[0], Some(s01[1]), ra);
            history(fx, buf + 0x90, 0xA, sp + 0x50);
        }
    }
    let n = fx.mem.i16(t + 6);
    if n < 8 {
        fx.mem.set16(t + 6, n + 1);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01CE0 (D, task): b701's red ring (0x1E01EA0) in green (0x80, 0xFF,
/// 0x80): size 6, cell 0..3, radius growing by 3..18 from the start, row
/// 0..9, life 10..29; 0x80 during the life, out by 10.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        let v = fx.rand_from(t, 0x01E0_1D08);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, 0x80, 0xFF, 0x80);
        let v = rmod(fx, t, 0x01E0_1D3C, 0xFFFF);
        fx.mem.set32(t + 0x18, v);
        let v = rmod(fx, t, 0x01E0_1D58, 0xFFFF);
        fx.mem.set32(t + 0x10, v);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, 0x01E0_1D78, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = fx.rand_from(t, 0x01E0_1D94);
        fx.mem.set16(t + 0xE, v % 0x10 + 3);
        let v = rmod(fx, t, 0x01E0_1DB4, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1DD0, 0x14);
        fx.mem.set16(t + 6, v + 0xA);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xA);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E02800 (D, task): b205's camera light at +0x6AA0 on a 0x70-byte
/// task.
fn camera(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    if fx.mem.i16(t + 0x58) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0x4000_0000), (0xC, 0), (0x20, 0), (0x24, 0x42F0_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        for k in 0..3 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(CAM + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.set16(t + 0x50, 0);
    }
    let w = fx.mem.f(CAM + 0x28);
    if fx.mem.i16(t + 0x50) == 0 {
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, f32::from_bits(0x3E4C_CCCD)));
        } else {
            fx.mem.set16(t + 0x50, 1);
        }
    } else if w <= f32::from_bits(0x3DCC_CCCD) {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, f32::from_bits(0x3DCC_CCCD)));
    }
    let (p, ra) = if fx.mem.i16(t + 0x58) == 0 { (0, 0x01E0_2934) } else { (CAM, 0x01E0_2948) };
    camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E02980 (D, task): the object op 0x1F8380(2, 0, the record's floats
/// 0x1E03980 / 0x1E03988 at sp + 0x38 / + 0x3C, 0) each update, to 0x88.
fn object(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    for (to, from) in [(0x38u32, 0x01E0_3980u32), (0x3C, 0x01E0_3988)] {
        let w = fx.mem.read_u32(from);
        fx.mem.write_u32(sp + to, w);
    }
    fx.boundary(0x001F_8380, "object op 0x1F8380", [2, 0, sp + 0x38, sp + 0x3C]);
    if fx.mem.i16(COUNTER) == 0x88 {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E01E70 (D, task): b205's flier in pale green (0x80, 0xFF, 0x80):
/// size 4..11, a random cell row, flying (-50..49) / 60 a update, turned
/// by the camera's angles; in by 0x40 under 0x80 for 20 updates, out by
/// 9. `idle` the entry's s1.
fn fly(fx: &mut Fx, t: u32, idle: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_1E94);
        fx.mem.set16(t + 8, v % 8 + 4);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_1EBC);
        fx.mem.set16(t + 0x56, (v >> 4) % 3);
        l2::rgb(fx, t + 0x60, 0x80, 0xFF, 0x80);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_1EF0u32), (0x30, 0x01E0_1F1C), (0x38, 0x01E0_1F48)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_1FA0, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(idle), 0x01E0_1FBC);
        for (o, ra) in [(4u32, 0x01E0_1FC4u32), (2, 0x01E0_1FE4), (0, 0x01E0_2004)] {
            let v = fx.rand_from(t, ra);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x14 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(t + 0x30 + o));
            fx.mem.setf(t + 0x10 + o, v);
        }
    }
    for (o, by) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + o).wrapping_add(fx.mem.i16(t + by) as u32);
        fx.mem.write_u32(t + o, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(idle), 0x01E0_20E0);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E02100 (D, task): b205's arc ring word for word (green, radius 180).
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.write_u32(t + 0x4C, 0x4334_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0xFF, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 8 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a + 0x60);
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x67, a - 0xF);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_21C8);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x4C), f32::from_bits(0x3ECC_CCCD));
        fx.mem.setf(t + 0x4C, r);
    }
    if fx.mem.i16(COUNTER) == 0xC3 {
        fx.mem.set16(t + 0x58, 0);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E03470(D, task, p, q) (this record only): a ribbon tube along the n
/// = i16 +6 points `p` (16 bytes apart): for each segment k (k + 1 < n)
/// one ring at angle 0 of 4 untextured gouraud strips (0x109BC0 additive,
/// 0x109DD0), radius D+0 (x) / D+8 (z = D+8 (cos - 0.6)), the y offset
/// -(p[0].y - q[0].y), each strip's first and third vertices nudged by
/// the task's +0x48 in +z / -z / +x / -x (the nudge falling by 0.09 a
/// segment, not under 0); the colour +0x60 at the first two vertices, its
/// alpha less alpha / n at the last two (falling along the tube, not
/// under 0); keyed by the mean w.
fn tube(fx: &mut Fx, t: u32, p: u32, q: u32) {
    let sp = fx.enter(0x220);
    let d = sp + 0x188;
    let n = fx.mem.i16(t + 6);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    for k in 0..4 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x21C + k, b);
    }
    let a = sp + 0x160;
    for k in 0..4 {
        let w = fx.mem.read_u32(p + 4 * k);
        fx.mem.write_u32(a + 4 * k, w);
        let w = fx.mem.read_u32(q + 4 * k);
        fx.mem.write_u32(sp + 0x170 + 4 * k, w);
    }
    for c in 0..3 {
        let v = ee::sub(fx.mem.f(a + 4 * c), fx.mem.f(sp + 0x170 + 4 * c));
        fx.mem.setf(a + 4 * c, v);
    }
    let w = fx.mem.read_u32(D);
    fx.mem.write_u32(a, w);
    let y = ee::neg(fx.mem.f(a + 4));
    fx.mem.setf(a + 4, y);
    let w = fx.mem.read_u32(D + 8);
    fx.mem.write_u32(a + 8, w);
    let f25 = fx.mem.f(a + 8);
    let f24 = fx.mem.f(a + 4);
    let mut nudge = fx.mem.f(t + 0x48);
    let f23 = ee::mul(f32::from_bits(0x3F19_999A), f25);
    let alpha = i32::from(fx.mem.read_u8(t + 0x63));
    let fall = fx.div(alpha, n).0 as i16 as i32;
    let mut seg = p;
    let mut i = 0;
    while i < n - 1 {
        l2::colour_strip4(fx, d, sp + 0x21C, 1);
        let v = i32::from(fx.mem.read_u8(sp + 0x21F)) - fall;
        fx.mem.write_u8(sp + 0x21F, if v > 0 { v as u8 } else { 0 });
        l2::colour_strip4(fx, d, sp + 0x21C, 2);
        let deg = 0;
        let s = l2::sin_deg(fx, D + 0xD8, deg, 0);
        let f21 = ee::mul(fx.mem.f(a), s);
        let c = l2::sin_deg(fx, D + 0xD8, deg, 1);
        let f22 = ee::sub(ee::mul(f25, c), f23);
        for s0 in 0..4 {
            let m = sp + 0x120;
            for (row, from) in [(0u32, seg), (0x10, seg), (0x20, seg + 0x10), (0x30, seg + 0x10)] {
                for c in 0..3 {
                    let w = fx.mem.read_u32(from + 4 * c);
                    fx.mem.write_u32(m + row + 4 * c, w);
                }
            }
            l2::add_column(fx, m, 0, f21);
            l2::add_column(fx, m, 2, f22);
            l2::add_column(fx, m, 1, f24);
            let (o, up) = match s0 {
                0 => (8u32, true),
                1 => (8, false),
                2 => (0, true),
                _ => (0, false),
            };
            for row in [0u32, 0x20] {
                let at = m + row + o;
                let v = fx.mem.f(at);
                fx.mem.setf(at, if up { ee::add(v, nudge) } else { ee::sub(v, nudge) });
            }
            if l2::project(fx, m, SPR, 0, d, sp + 0x110) {
                let w = fx.mem.f(sp + 0x110);
                fx.fix_residue(w, 0x01E0_3828);
                let key = ee::fixunssfsi(w) as i32;
                fx.submit(d, OT, key);
            }
        }
        nudge = ee::sub(nudge, f32::from_bits(0x3DB8_51EC));
        if crate::exe::ee_lt(nudge, 0.0) {
            nudge = 0.0;
        }
        seg += 0x10;
        i += 1;
    }
    fx.leave(0x220);
}
