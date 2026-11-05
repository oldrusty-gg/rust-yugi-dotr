//! BMODULE b567 (card 442), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02DA0, data area 0x1ED0000, 0x7180 bytes) [F]
//!: b205's program and layout in red (every
//! global where b205 has it), its tube on rings 4..13 (nudge 0.2) along
//! the trails of the attacker's nodes 0x22 / 0x21 and b564's white quads
//! (0x1E024D0) instead of the object op: update 0xA0 the red tube; 0xA9
//! at node 0x22 halfway to the target eight red and twenty dark rings,
//! fifty small red quads, a red arc ring and a camera light; on 0x57 /
//! 0x6D / 0x88 thirteen white quads thrown from node 0xC, on 0x4C / 0x63
//! / 0x83 six from node 0x32. The steps 0x1E02310 / 0x1E02690 / 0x1E02A50
//! / 0x1E02B10 (an object op) / 0x1E02B60 / 0x1E02CE0 (a flash, counted)
//! are never spawned (not ported). Returns 1 on update 0x9F, 4 on 0xBE, 2
//! on 0xA9, 8 once the 0xD7-update timer D+0x6DA0 has run out.
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
const TRAIL: u32 = 0x01E0_19F0;
const DARK: u32 = 0x01E0_1C00;
const RED: u32 = 0x01E0_1DB0;
const CAMERA: u32 = 0x01E0_28D0;
/// Pool B's steps.
const FLY: u32 = 0x01E0_1F40;
const ARC: u32 = 0x01E0_21D0;
const WHITE: u32 = 0x01E0_24D0;
/// Counted by the pool B loop, never spawned.
const FLASH: u32 = 0x01E0_2CE0;
/// The return address of the pool B loop's `jalr` (0x1E03294 + 8).
const TASK_RA: u64 = 0x01E0_329C;
const POS: u32 = D + 0x2D0;
const TEX4: u32 = D + 0x340;
const CAM: u32 = D + 0x6AA0;
const TRAIL_Q: u32 = D + 0x6AF0;
const TRAIL_P: u32 = D + 0x6B90;
const TEX: u32 = D + 0x6D80;
const TIMER: u32 = D + 0x6DA0;
const COLOUR: u32 = D + 0x6DA4;
const NODES: u32 = D + 0x6DEC;
const KIND: u32 = D + 0x6E06;
const FACING: u32 = D + 0x6E0A;
const COUNTER: u32 = D + 0x6E0C;
const BYTES: u32 = D + 0x6E10;
const SINES: u32 = D + 0x7010;
/// The tube's rings: 4..13, nudge 0.2, keyed by the mean w.
const RINGS: TubeRings = TubeRings { first: 4, end: 0xE, nudge: 0x3E4C_CCCD, key_ra: 0x01E0_3BB8, plain: false };

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02DA0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x180);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x6DAC, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x180);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x7180);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x6E08, param);
    fx.mem.set32(TIMER, 0xD7);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, node) in [0xC, 0x32, 0x22, 0x21, 5, 6].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // 0x1E02CF0(0, D): the textures.
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(TEX4 + 4 * k as u32, t);
    }
    for k in 0xB..0x10 {
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
    fx.fix_residue(y, 0x01E0_2EEC);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2EF8, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2F2C, 0x1E);
        fx.mem.set8(BYTES + 0x100 + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_2F60, 0x14);
        fx.mem.set8(BYTES + 0x100 + k, v - 0xA);
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
            fx.saved(s, &[(0x40, 0x01E0_2FD0), (0x30, u64::from(slot)), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
            trail(fx, t);
            fx.leave(0x60);
        }
        for (f, ra) in [(DARK, 0x01E0_2FF0u64), (RED, 0x01E0_3010)] {
            if fx.mem.read_u32(t + 0x24) == f {
                let s = fx.enter(0x20);
                fx.saved(s, &[(0x10, ra), (0, u64::from(D))]);
                ring_a(fx, t, f == RED);
                fx.leave(0x20);
            }
        }
        if fx.mem.read_u32(t + 0x24) == CAMERA {
            let s = fx.enter(0x30);
            fx.saved(s, &[(0x20, 0x01E0_3030), (0x10, 0), (0, u64::from(D))]);
            camera(fx, t);
            fx.leave(0x30);
        }
    }
    let h = fx.target_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x60, u64::from(D), 0x01E0_30E4);
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
            fx.mem.write_u32(D, 0x40F0_0000);
            let z = if fx.mem.i16(FACING) != 0 { 0xC120_0000 } else { 0x4120_0000 };
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
                billboard::ring_t20(fx, uv::uv_rect4, D, t, sp + 0xE8, BYTES);
            } else {
                billboard::ring_t(fx, uv::uv_rect4, D, t, sp + 0xE8, BYTES);
            }
        }
    }
    // The entry's s1: the flash tasks counted here (none are spawned).
    let mut flashes = 0u32;
    for k in 0..TASKS_B {
        let t = POOL_B + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        if f == FLASH {
            flashes += 1;
            continue;
        }
        // The steps' prologues: the entry's s0 = D (and s1 the flashes).
        if f == FLY {
            let s = fx.enter(0x20);
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
            fly(fx, t, flashes);
            fx.leave(0x20);
            continue;
        }
        let s = fx.enter(0x30);
        fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(flashes)), (0, u64::from(D))]);
        match f {
            ARC => arc(fx, t),
            WHITE => white(fx, t),
            _ => panic!("b567: unknown task step {f:#x}"),
        }
        fx.leave(0x30);
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
        if f(fx) == WHITE {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(ta + 0x34);
            cell(fx, sp + 0xB0, 0x14, sub);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x10, t + 0x10);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x9F => 1,
        0xBE => 4,
        0xA9 => 2,
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

/// 0x1E03590(D, step, 0) (b205's 0x1E02A80, word for word): the first free 0x40-byte
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

/// 0x1E036B0(D, step, 0) (b205's 0x1E02BA0, word for word).
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

/// 0x1E01220(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the spawns (frame 0x50: ra, the entry's s1 = 0, s0 = D; then s0 =
/// D, s1 a loop count). The spawns' saves: s0 = D, s1, the entry's s2 =
/// 0, s3 = the parameter, s4 the runner's (0 [H]).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_2F9C), (0x10, 0), (0, u64::from(D))]);
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
    if c(fx) == 0xA0 {
        fx.mem.set16(KIND, 0);
        spawn_a(fx, TRAIL, saves(s1), 0x01E0_17C4);
    }
    if c(fx) == 0xA9 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 4);
        crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(D), Some(s1), 0x01E0_17EC);
        let h = fx.target_handle();
        crate::b076::unit_position_saving(fx, h, sp + 0x40, u64::from(D), 0x01E0_1800);
        let w = fx.mem.read_u32(sp + 0x30);
        fx.mem.write_u32(POS, w);
        let w = fx.mem.read_u32(sp + 0x34);
        fx.mem.write_u32(POS + 4, w);
        let a = fx.mem.f(sp + 0x38);
        let z = ee::add(a, ee::div(ee::sub(fx.mem.f(sp + 0x48), a), 2.0));
        fx.mem.setf(POS + 8, z);
        fx.mem.set16(KIND, 1);
        for k in 0..8u64 {
            spawn_a(fx, RED, saves(k), 0x01E0_1850);
        }
        for k in 0..0x14u64 {
            spawn_a(fx, DARK, saves(k), 0x01E0_1878);
        }
        for k in 0..0x32u64 {
            spawn_b(fx, FLY, saves(k), 0x01E0_18A0);
        }
        s1 = 0x32;
        spawn_b(fx, ARC, saves(s1), 0x01E0_18C4);
        spawn_a(fx, CAMERA, saves(s1), 0x01E0_18D8);
    }
    for (hits, o, node_ra, n, ra) in [(&[0x57, 0x6D, 0x88][..], 0u32, 0x01E0_1918u32, 0xDu64, 0x01E0_1944u32), (&[0x4C, 0x63, 0x83][..], 2, 0x01E0_1998, 6, 0x01E0_19C4)] {
        if hits.contains(&c(fx)) {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + o);
            crate::b076::node_position_saving(fx, h, slot, POS, u64::from(D), Some(s1), node_ra);
            let y = ee::sub(fx.mem.f(POS + 4), 1.5);
            fx.mem.setf(POS + 4, y);
            for k in 0..n {
                spawn_b(fx, WHITE, saves(k), ra);
            }
            s1 = n;
        }
    }
    fx.leave(0x50);
}

/// 0x1E019F0 (D, task): b205's trail in red (0xFF, 0x20, 0x20), the
/// trails of nodes 0x22 (+0x6AF0) / 0x21 (+0x6B90) to update 0xAF, alpha
/// up by 0x19 under 0x80 to update 0xAF, length +6 10.
fn trail(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    l2::rgb(fx, t + 0x28, 0xFF, 0x20, 0x20);
    let mut s01 = [u64::from(D), 0u64];
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 5);
        if fx.mem.i16(t + 0x30) == 0 {
            fx.mem.set16(t + 0xE, 0x1E);
            for (slot, buf, ra) in [(NODES + 4, TRAIL_Q, 0x01E0_1A64u32), (NODES + 6, TRAIL_P, 0x01E0_1AB4)] {
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
    if fx.mem.i16(COUNTER) < 0xB0 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a + 0x19);
        }
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x1F {
            fx.mem.write_u8(t + 0x2B, a - 0x1D);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(t + 0x30) == 0 && fx.mem.i16(COUNTER) < 0xB0 {
        for (slot, buf, ra) in [(NODES + 4, TRAIL_Q, 0x01E0_1B88u32), (NODES + 6, TRAIL_P, 0x01E0_1BB4)] {
            let h = fx.self_handle();
            let s = fx.mem.i16(slot);
            crate::b076::node_position_saving(fx, h, s, sp + 0x50, s01[0], Some(s01[1]), ra);
            history(fx, buf + 0x90, 0xA, sp + 0x50);
        }
    }
    fx.mem.set16(t + 6, 0xA);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E01C00 (`dark`: size 10, (0x20, 8, 8), growth 3..18, row 0..9,
/// life 5..11; in by 0x38 under 0x71, out by 10) / 0x1E01DB0 (`red`: size
/// 6, red (0xFF, 0x40, 0x40), growth 3..18 from the start, row 0..9, life
/// 10..29; 0x80 during the life, out by 10) (D, task): b205's 0x40-byte
/// rings.
fn ring_a(fx: &mut Fx, t: u32, red: bool) {
    if fx.mem.i16(t + 0x36) == 1 {
        if red {
            fx.mem.set16(t + 0x38, 6);
            let v = fx.rand_from(t, 0x01E0_1DD8);
            fx.mem.set16(t + 0x34, v % 4);
            l2::rgb(fx, t + 0x28, 0xFF, 0x40, 0x40);
            let v = rmod(fx, t, 0x01E0_1E0C, 0xFFFF);
            fx.mem.set32(t + 0x18, v);
            let v = rmod(fx, t, 0x01E0_1E28, 0xFFFF);
            fx.mem.set32(t + 0x10, v);
            fx.mem.set16(t + 0x30, 0);
            let v = rmod(fx, t, 0x01E0_1E48, 0x168);
            fx.mem.set16(t + 0x3A, v);
            let v = fx.rand_from(t, 0x01E0_1E64);
            fx.mem.set16(t + 0xE, v % 0x10 + 3);
            let v = rmod(fx, t, 0x01E0_1E84, 0xA);
            fx.mem.set16(t + 8, v);
            let v = rmod(fx, t, 0x01E0_1EA0, 0x14);
            fx.mem.set16(t + 6, v + 0xA);
        } else {
            fx.mem.set16(t + 0x38, 0xA);
            let v = fx.rand_from(t, 0x01E0_1C28);
            fx.mem.set16(t + 0x34, v % 4);
            for (o, ra) in [(0x10u32, 0x01E0_1C44u32), (0x14, 0x01E0_1C60), (0x18, 0x01E0_1C7C)] {
                let v = rmod(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
            l2::rgb(fx, t + 0x28, 0x20, 8, 8);
            fx.mem.set16(t + 0x30, 0);
            let v = rmod(fx, t, 0x01E0_1CA8, 0x168);
            fx.mem.set16(t + 0x3A, v);
            let v = fx.rand_from(t, 0x01E0_1CC4);
            fx.mem.set16(t + 0xE, v % 0x10 + 3);
            let v = rmod(fx, t, 0x01E0_1CE4, 0xA);
            fx.mem.set16(t + 8, v);
            let v = rmod(fx, t, 0x01E0_1D00, 7);
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

/// 0x1E028D0 (D, task): b205's camera light at +0x6AA0 (word for word).
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
    let (p, ra) = if fx.mem.i16(t + 0x36) == 0 { (0, 0x01E0_2A04) } else { (CAM, 0x01E0_2A18) };
    camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E01F40 (D, task): a small red quad (size 4..11, a random cell
/// row) flying (-50..49) / 60 a update, turned by the camera's angles; in
/// by 0x40 under 0x80 for 20 updates, out by 9. `flashes` the entry's s1.
fn fly(fx: &mut Fx, t: u32, flashes: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_1F64);
        fx.mem.set16(t + 8, v % 8 + 4);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_1F8C);
        fx.mem.set16(t + 0x56, (v >> 4) % 3);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_1FC0u32), (0x30, 0x01E0_1FEC), (0x38, 0x01E0_2018)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_2070, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_208C);
        for (o, ra) in [(4u32, 0x01E0_2094u32), (2, 0x01E0_20B4), (0, 0x01E0_20D4)] {
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
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_21B0);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E021D0 (D, task): b205's arc ring in red (radius 180).
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.write_u32(t + 0x4C, 0x4334_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
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
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2298);
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

/// 0x1E024D0 (D, task): b564's white quad (0x1E01E00, b701's 0x1E02410)
/// of size 130, cell row 2..4, thrown 1.6 along a random heading and
/// drifting 0.6 a update along it; in by 0x40 under 0x80 for 10 updates,
/// out by 9.
fn white(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x82);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2508);
        fx.mem.set16(t + 0x56, (v >> 4) % 3 + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2540);
        fx.mem.set16(t + 0x5A, (v >> 4) % 0x168);
        step_along(fx, t, f32::from_bits(0x3FCC_CCCD));
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
        step_along(fx, t, f32::from_bits(0x3F19_999A));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_266C);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// z += by sin(+0x5A), x += by cos(+0x5A) (D+0x7010's sines).
fn step_along(fx: &mut Fx, t: u32, by: f32) {
    let a = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let z = ee::add(fx.mem.f(t + 0x18), ee::mul(by, s));
    fx.mem.setf(t + 0x18, z);
    let a = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, SINES, a, 1);
    let x = ee::add(fx.mem.f(t + 0x10), ee::mul(by, c));
    fx.mem.setf(t + 0x10, x);
}
