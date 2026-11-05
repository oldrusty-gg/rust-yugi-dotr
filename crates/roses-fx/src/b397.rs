//! BMODULE b397 (card 575), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02440, data area 0x1ED0000, 0x7080 bytes) [F]
//!: b205's program in violet with one byte table
//! (the globals from the nodes on 8 lower, the sines at D+0x6F08), b701's
//! plain tube compiled in a 0x210 frame (0x1E02D00, `b701::tube_b_with`
//! plain: z = D+8 (cos - 1), no y offset, key 0) along the trails of the
//! attacker's nodes 0xA / 0xC, and flares instead of the object op:
//! update 0x58 the violet tube; 0x5B at node 0xC (and the target's depth)
//! eight violet and twenty dark rings, fifty violet quads, a violet arc
//! ring and a camera light; on 0x35 / 0x42 / 0x4B / 0x5C three violet
//! flares each at nodes 0x12 / 0x19, on 0x39 / 0x43 / 0x4C at nodes 0x23
//! / 0x2A. The steps 0x1E02110 / 0x1E021D0 (an object op) / 0x1E02220 are
//! never spawned (not ported). Returns 1 on update 0x53, 4 on 0x82, 2 on
//! 0x5D, 8 once the 0x8C-update timer D+0x6DA0 has run out.
use crate::b494::camera_angles_saving;
use crate::b701::{tube_b_with, TubeRings};
use crate::blib::{billboard, exe::camera_slots45, spawn::history, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS_A: u32 = 150;
const TASKS_B: u32 = 150;
/// The 0x40-byte tasks.
const POOL_A: u32 = D + 0x380;
/// The 0x70-byte tasks.
const POOL_B: u32 = D + 0x2900;
/// Pool A's steps.
const TRAIL: u32 = 0x01E0_1540;
const DARK: u32 = 0x01E0_1730;
const RED: u32 = 0x01E0_18E0;
const CAMERA: u32 = 0x01E0_1F90;
/// Pool B's steps.
const FLY: u32 = 0x01E0_1A70;
const ARC: u32 = 0x01E0_1D00;
const FLARE: u32 = 0x01E0_1E20;
/// The return address of the pool B loop's `jalr` (0x1E028BC + 8).
const TASK_RA: u64 = 0x01E0_28C4;
const POS: u32 = D + 0x2D0;
const TEX4: u32 = D + 0x340;
const CAM: u32 = D + 0x6AA0;
const TRAIL_Q: u32 = D + 0x6AF0;
const TRAIL_P: u32 = D + 0x6B90;
const TEX: u32 = D + 0x6D80;
const TIMER: u32 = D + 0x6DA0;
const COLOUR: u32 = D + 0x6DA4;
/// 100 sin of the angle D+0x6DFA (5 a update), set every update.
const SWING: u32 = D + 0x6D70;
const NODES: u32 = D + 0x6DE4;
const SWING_A: u32 = D + 0x6DFA;
const KIND: u32 = D + 0x6DFE;
const FACING: u32 = D + 0x6E02;
const COUNTER: u32 = D + 0x6E04;
const BYTES: u32 = D + 0x6E08;
const SINES: u32 = D + 0x6F08;
/// The tube: b701's 18 rings and 0.2 nudge in the plain compile.
const RINGS: TubeRings = TubeRings { first: 0, end: 0x12, nudge: 0x3E4C_CCCD, key_ra: 0, plain: true };

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02440.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x6DAC, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x7080);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x6E00, param);
    fx.mem.set32(TIMER, 0x8C);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, node) in [0xA, 0xC, 0x12, 0x19, 0x23, 0x2A].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // 0x1E023A0(0, D): the textures.
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(TEX4 + 4 * k as u32, t);
    }
    for k in 0xB..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(TEX4 + 4 * k as u32, t);
    }
    for k in 0..1 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_2588);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2594, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS_A {
        let slot = D + 0x40 * k;
        let t = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == TRAIL {
            let s = fx.enter(0x60);
            fx.saved(s, &[(0x40, 0x01E0_2604), (0x30, u64::from(k)), (0x20, u64::from(slot)), (0x10, 0), (0, u64::from(D))]);
            trail(fx, t);
            fx.leave(0x60);
        }
        for (f, ra) in [(DARK, 0x01E0_2628u64), (RED, 0x01E0_2648)] {
            if fx.mem.read_u32(t + 0x24) == f {
                let s = fx.enter(0x20);
                fx.saved(s, &[(0x10, ra), (0, u64::from(D))]);
                ring_a(fx, t, f == RED);
                fx.leave(0x20);
            }
        }
        if fx.mem.read_u32(t + 0x24) == CAMERA {
            let s = fx.enter(0x30);
            fx.saved(s, &[(0x20, 0x01E0_2668), (0x10, 0), (0, u64::from(D))]);
            camera(fx, t);
            fx.leave(0x30);
        }
    }
    let h = fx.target_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x60, u64::from(D), 0x01E0_271C);
    for k in 0..TASKS_A {
        let t = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == TRAIL {
            if fx.mem.i16(t + 0x30) == 0 {
                fx.mem.write_u32(D + 4, 0);
            }
            if fx.mem.i16(t + 0x30) == 1 {
                fx.mem.write_u32(D + 4, 0x3F80_0000);
            }
            fx.mem.write_u32(D, 0x4090_0000);
            let z = if fx.mem.i16(FACING) != 0 { 0xC090_0000 } else { 0x4090_0000 };
            fx.mem.write_u32(D + 8, z);
            if fx.mem.i16(t + 0x30) == 0 {
                tube_b_with(fx, t, TRAIL_P, TRAIL_Q, RINGS);
            }
        }
        for (f, local, t20) in [(DARK, 0x70u32, false), (RED, 0x80, true)] {
            if fx.mem.read_u32(t + 0x24) != f {
                continue;
            }
            let w = fx.mem.read_u32(TEX4 + 0x3C);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            cell(fx, sp + local, 0x9B, sub);
            if t20 {
                billboard::ring_t20(fx, uv::uv_rect4, D, t, sp + 0xC8, BYTES);
            } else {
                billboard::ring_t(fx, uv::uv_rect4, D, t, sp + 0xC8, BYTES);
            }
        }
    }
    for k in 0..TASKS_B {
        let t = POOL_B + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        // The steps' prologues: the entry's s0 = D, s1 = 0.
        let frame = if f == FLARE { 0x30 } else { 0x20 };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
        }
        match f {
            FLY => fly(fx, t, 0),
            ARC => arc(fx, t),
            FLARE => flare(fx, t),
            _ => panic!("b397: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS_B {
        let t = POOL_B + 0x70 * k;
        // The original reads the cell frame of pool A's task k (+0x34).
        let ta = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
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
        if f(fx) == FLARE {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(ta + 0x34);
            cell(fx, sp + 0xA0, 0x14, sub);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x10, t + 0x10);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x53 => 1,
        0x82 => 4,
        0x5D => 2,
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

/// 0x1E02AA0(D, step, 0) (b205's 0x1E02A80, kind D+0x6DFE): the first free 0x40-byte
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

/// 0x1E02BC0(D, step, 0) (b205's 0x1E02BA0, kind D+0x6DFE).
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

/// 0x1E01220(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter,
/// the spawns and the swing (frame 0x50: ra, the entry's s1 = 0, s0 = D;
/// then s0 = D, s1 a loop count). The spawns' saves: s0 = D, s1, the
/// entry's s2 = the parameter, s3 / s4 the runner's (0 [H]).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_25D0), (0x10, 0), (0, u64::from(D))]);
    let p = param as i64 as u64;
    let saves = |s1: u64| [u64::from(D), s1, p, 0, 0];
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let mut s1 = 0u64;
    if c(fx) == 0x58 {
        fx.mem.set16(KIND, 0);
        spawn_a(fx, TRAIL, saves(s1), 0x01E0_1274);
    }
    if c(fx) == 0x5B {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2);
        crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(D), Some(s1), 0x01E0_129C);
        let h = fx.target_handle();
        crate::b076::unit_position_saving(fx, h, sp + 0x40, u64::from(D), 0x01E0_12B0);
        let w = fx.mem.read_u32(sp + 0x30);
        fx.mem.write_u32(POS, w);
        let w = fx.mem.read_u32(sp + 0x34);
        fx.mem.write_u32(POS + 4, w);
        let w = fx.mem.read_u32(sp + 0x48);
        fx.mem.write_u32(POS + 8, w);
        fx.mem.set16(KIND, 1);
        for k in 0..8u64 {
            spawn_a(fx, RED, saves(k), 0x01E0_12E8);
        }
        for k in 0..0x14u64 {
            spawn_a(fx, DARK, saves(k), 0x01E0_1310);
        }
        for k in 0..0x32u64 {
            spawn_b(fx, FLY, saves(k), 0x01E0_1338);
        }
        s1 = 0x32;
        spawn_b(fx, ARC, saves(s1), 0x01E0_135C);
        spawn_a(fx, CAMERA, saves(s1), 0x01E0_1370);
    }
    // (updates, the two nodes with the node fetches' and spawns' returns)
    let fans: [(&[i32], [(u32, u32, u32); 2]); 2] = [
        (&[0x35, 0x42, 0x4B, 0x5C], [(4, 0x01E0_13BC, 0x01E0_13D4), (6, 0x01E0_13FC, 0x01E0_1414)]),
        (&[0x39, 0x43, 0x4C], [(8, 0x01E0_1468, 0x01E0_1480), (0xA, 0x01E0_14A8, 0x01E0_14C0)]),
    ];
    for (hits, nodes) in fans {
        if hits.contains(&c(fx)) {
            for (o, node_ra, ra) in nodes {
                let h = fx.self_handle();
                let slot = fx.mem.i16(NODES + o);
                crate::b076::node_position_saving(fx, h, slot, POS, u64::from(D), Some(s1), node_ra);
                for k in 0..3u64 {
                    spawn_b(fx, FLARE, saves(k), ra);
                }
                s1 = 3;
            }
        }
    }
    let a = fx.mem.i16(SWING_A);
    let v = l2::sin_deg(fx, SINES, a, 0);
    fx.mem.setf(SWING, ee::mul(100.0, v));
    let a = fx.mem.i16(SWING_A) + 5;
    fx.mem.set16(SWING_A, a);
    let a = fx.mem.i16(SWING_A);
    if a >= 0x168 {
        let (_, m) = fx.div(a, 0x168);
        fx.mem.set16(SWING_A, m);
    }
    fx.leave(0x50);
}

/// 0x1E01540 (D, task): b205's trail in violet (0x80, 0x20, 0xFF), +0xE
/// 10, the trails of nodes 0xA (+0x6AF0) / 0xC (+0x6B90) every update,
/// alpha 0x80 while its age is under +0xE, length +6 5.
fn trail(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    l2::rgb(fx, t + 0x28, 0x80, 0x20, 0xFF);
    let mut s01 = [u64::from(D), 0u64];
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 5);
        if fx.mem.i16(t + 0x30) == 0 {
            fx.mem.set16(t + 0xE, 0xA);
            for (slot, buf, ra) in [(NODES, TRAIL_Q, 0x01E0_15B4u32), (NODES + 2, TRAIL_P, 0x01E0_1604)] {
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
        if fx.mem.i16(t + 0x30) == 1 {
            fx.mem.set16(t + 0xE, 7);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 0xE) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x1F {
            fx.mem.write_u8(t + 0x2B, a - 0x1D);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(t + 0x30) == 0 {
        for (slot, buf, ra) in [(NODES, TRAIL_Q, 0x01E0_16BCu32), (NODES + 2, TRAIL_P, 0x01E0_16E8)] {
            let h = fx.self_handle();
            let s = fx.mem.i16(slot);
            crate::b076::node_position_saving(fx, h, s, sp + 0x50, s01[0], Some(s01[1]), ra);
            history(fx, buf + 0x90, 0xA, sp + 0x50);
        }
    }
    fx.mem.set16(t + 6, 5);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E01730 (`dark`: size 10, (0x20, 0x10, 8), growth 3..18, row 0..9,
/// life 5..11; in by 0x38 under 0x71, out by 10) / 0x1E018E0 (`red`: size
/// 6, violet (0x80, 0x40, 0xFF), growth 3..18 from the start, row 0..9,
/// life 10..29; 0x80 during the life, out by 10) (D, task): b205's
/// 0x40-byte rings.
fn ring_a(fx: &mut Fx, t: u32, red: bool) {
    if fx.mem.i16(t + 0x36) == 1 {
        if red {
            fx.mem.set16(t + 0x38, 6);
            let v = fx.rand_from(t, 0x01E0_1908);
            fx.mem.set16(t + 0x34, v % 4);
            l2::rgb(fx, t + 0x28, 0x80, 0x40, 0xFF);
            let v = rmod(fx, t, 0x01E0_193C, 0xFFFF);
            fx.mem.set32(t + 0x18, v);
            let v = rmod(fx, t, 0x01E0_1958, 0xFFFF);
            fx.mem.set32(t + 0x10, v);
            fx.mem.set16(t + 0x30, 0);
            let v = rmod(fx, t, 0x01E0_1978, 0x168);
            fx.mem.set16(t + 0x3A, v);
            let v = fx.rand_from(t, 0x01E0_1994);
            fx.mem.set16(t + 0xE, v % 0x10 + 3);
            let v = rmod(fx, t, 0x01E0_19B4, 0xA);
            fx.mem.set16(t + 8, v);
            let v = rmod(fx, t, 0x01E0_19D0, 0x14);
            fx.mem.set16(t + 6, v + 0xA);
        } else {
            fx.mem.set16(t + 0x38, 0xA);
            let v = fx.rand_from(t, 0x01E0_1758);
            fx.mem.set16(t + 0x34, v % 4);
            for (o, ra) in [(0x10u32, 0x01E0_1604u32), (0x14, 0x01E0_1790), (0x18, 0x01E0_17AC)] {
                let v = rmod(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
            l2::rgb(fx, t + 0x28, 0x20, 0x10, 8);
            fx.mem.set16(t + 0x30, 0);
            let v = rmod(fx, t, 0x01E0_17D8, 0x168);
            fx.mem.set16(t + 0x3A, v);
            let v = fx.rand_from(t, 0x01E0_17F4);
            fx.mem.set16(t + 0xE, v % 0x10 + 3);
            let v = rmod(fx, t, 0x01E0_1814, 0xA);
            fx.mem.set16(t + 8, v);
            let v = rmod(fx, t, 0x01E0_1830, 7);
            fx.mem.set16(t + 6, v + 5);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if red {
            fx.mem.write_u8(t + 0x2B, 0x80);
        } else if a < 0x71 {
            fx.mem.write_u8(t + 0x2B, a + 0x38);
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xA);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if red || fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E01F90 (D, task): b205's camera light at +0x6AA0 (word for word).
fn camera(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0x4000_0000), (0xC, 0), (0x20, 0), (0x24, 0x42F0_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        for k in 0..3 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(CAM + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    let w = fx.mem.f(CAM + 0x28);
    if fx.mem.i16(t + 0x30) == 0 {
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, f32::from_bits(0x3E4C_CCCD)));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else if w <= f32::from_bits(0x3DCC_CCCD) {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, f32::from_bits(0x3DCC_CCCD)));
    }
    let (p, ra) = if fx.mem.i16(t + 0x36) == 0 { (0, 0x01E0_20C4) } else { (CAM, 0x01E0_20D8) };
    camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E01A70 (D, task): a small violet quad (size 4..11, a random cell
/// row) flying (-50..49) / 60 a update, turned by the camera's angles; in
/// by 0x40 under 0x80 for 20 updates, out by 9. `flashes` the entry's s1.
fn fly(fx: &mut Fx, t: u32, flashes: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_1A94);
        fx.mem.set16(t + 8, v % 8 + 4);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_1ABC);
        fx.mem.set16(t + 0x56, (v >> 4) % 3);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_1980u32), (0x30, 0x01E0_1B1C), (0x38, 0x01E0_1B48)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_1BA0, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_1BBC);
        for (o, ra) in [(4u32, 0x01E0_1A54u32), (2, 0x01E0_1BE4), (0, 0x01E0_1C04)] {
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
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_1CE0);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01D00 (D, task): b701's arc ring (0x1E022D0) in violet: outer
/// white, inner (0x80, 0x40, 0xFF), radius 130 growing by 0.4; outer
/// alpha in by 0x60 under 0xC0 for 8 updates, out by 15 (no end update;
/// the entry's s1 = 0 under 0x205580).
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4302_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
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
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_1DC0);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x4C), f32::from_bits(0x3ECC_CCCD));
        fx.mem.setf(t + 0x4C, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// z / x += `k` sin / cos of the heading +0x5A (D+0x6F08's sines).
fn sway(fx: &mut Fx, t: u32, k: f32) {
    let d = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, SINES, d, 0);
    let v = ee::add(fx.mem.f(t + 0x18), ee::mul(k, s));
    fx.mem.setf(t + 0x18, v);
    let d = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, SINES, d, 1);
    let v = ee::add(fx.mem.f(t + 0x10), ee::mul(k, c));
    fx.mem.setf(t + 0x10, v);
}

/// 0x1E01E20 (D, task): b418's flare (0x1E01B10) at size 80 (cell frame
/// 2..4), a random heading: in by 0x40 under 0x80 for 10 updates, then
/// out by 9; z / x += 0.2 sin / cos of the heading; faces the camera.
fn flare(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x50);
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
        let v = fx.rand_from(t, 0x01E0_1E58);
        let (_, m) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, m + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_1E90);
        let (_, m) = fx.div(v >> 4, 0x168);
        fx.mem.set16(t + 0x5A, m);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xA {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        sway(fx, t, f32::from_bits(0x3E4C_CCCD));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1F68);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}
