//! BMODULE b701 (card 147), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02CA0, data area 0x1ED0000) [F] (helper A12, 31
//! Oct 2026): two pools: 320 tasks of 0x40 bytes at D+0x380 (alive +0x36,
//! age +0x32, step +0x24 called by direct `jal`s; spawned by 0x1E034D0)
//! and 320 of 0x70 bytes at D+0x5380 (alive +0x58, step +0x44 through
//! `jalr`, 0x1E02BE0 an idle step; spawned by 0x1E03640), drawn with
//! rings of quads for the 0x40-byte tasks (3307e6a8ec30 at 0x1E00550,
//! `billboard::ring_t`; 8bd9f4ccd910 at 0x1E008D0, `billboard::ring_t20`),
//! quads (add5d81dd3fe at 0x1E00C00, `billboard::quad`) and an arc ring
//! (e6dce326551d at 0x1E00EC0, `billboard::arc_strips`), UVs by uv_rect4
//! 0x1E00100, and the record's tube (0x1E037D0, [`tube`]: gouraud strips
//! round the 10-point trails of the attacker's nodes 0xA and 6, kept by
//! c769f66b5e38 at 0x1E00080, `spawn::history`). Update 81 the red tube
//! (to 91, fading by 29 after); 87 at node 6 halfway to the target: eight
//! red and twenty dark rings, fifty red quads flying out, a white arc
//! ring to 195 and a camera light (0x1F7E20 slots 4 / 5); 29 / 58 / 90
//! twenty-six white quads about node 1 and to 87 three a update. Returns
//! 1 on update 81, 4 on 145, 2 on 91, 8 once the 150-update timer
//! +0xE280 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, exe::camera_slots45, spawn::history, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 320;
/// The 0x40-byte tasks.
const POOL_A: u32 = D + 0x380;
/// The 0x70-byte tasks.
const POOL_B: u32 = D + 0x5380;
/// Pool A's steps.
const TRAIL: u32 = 0x01E0_1AB0;
const DARK: u32 = 0x01E0_1CF0;
const RED: u32 = 0x01E0_1EA0;
const CAMERA: u32 = 0x01E0_29F0;
/// Pool B's steps.
const FLY: u32 = 0x01E0_2030;
const ARC: u32 = 0x01E0_22D0;
const WHITE: u32 = 0x01E0_2410;
const WHITE_B: u32 = 0x01E0_25E0;
const IDLE: u32 = 0x01E0_2BE0;
/// The return address of the pool B loop's `jalr` (0x1E03184 + 8).
const TASK_RA: u64 = 0x01E0_318C;
const POS: u32 = D + 0x2D0;
const TEX4: u32 = D + 0x340;
const CAM: u32 = D + 0xDF80;
const TRAIL_Q: u32 = D + 0xDFD0;
const TRAIL_P: u32 = D + 0xE070;
const TEX: u32 = D + 0xE260;
const TIMER: u32 = D + 0xE280;
const COLOUR: u32 = D + 0xE284;
const NODES: u32 = D + 0xE2CC;
const KIND: u32 = D + 0xE2E6;
const FACING: u32 = D + 0xE2EA;
const COUNTER: u32 = D + 0xE2EC;
const BYTES: u32 = D + 0xE2F0;
const SINES: u32 = D + 0xE4F0;
const SPR: u32 = 0x7000_0000;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02CA0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x170);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0xE28C, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x170);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xE660);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xE2E8, param);
    fx.mem.set32(TIMER, 0x96);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, node) in [1, 6, 0xA, 4, 5, 6].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // 0x1E02BF0(0, D): the textures.
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(TEX4 + 4 * k as u32, t);
    }
    for k in 0xB..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(TEX4 + 4 * k as u32, t);
    }
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.fix_residue(y, 0x01E0_2E38);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2E4C, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2E84, 0x1E);
        fx.mem.set8(BYTES + 0x100 + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_2EBC, 0x14);
        fx.mem.set8(BYTES + 0x100 + k, v - 0xA);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x40 * k;
        let t = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == TRAIL {
            let s = fx.enter(0x60);
            fx.saved(s, &[(0x40, 0x01E0_2F30), (0x30, u64::from(slot)), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
            trail(fx, t);
            fx.leave(0x60);
        }
        for (f, ra) in [(DARK, 0x01E0_2F50u64), (RED, 0x01E0_2F70)] {
            if fx.mem.read_u32(t + 0x24) == f {
                let s = fx.enter(0x20);
                fx.saved(s, &[(0x10, ra), (0, u64::from(D))]);
                ring_a(fx, t, f == RED);
                fx.leave(0x20);
            }
        }
        if fx.mem.read_u32(t + 0x24) == CAMERA {
            let s = fx.enter(0x30);
            fx.saved(s, &[(0x20, 0x01E0_2F90), (0x10, 0), (0, u64::from(D))]);
            camera(fx, t);
            fx.leave(0x30);
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x60, u64::from(D), 0x01E0_2FB4);
    for k in 0..TASKS {
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
            fx.mem.write_u32(D, 0x4168_0000);
            let z = if fx.mem.i16(FACING) != 0 { 0xC170_0000 } else { 0x4170_0000 };
            fx.mem.write_u32(D + 8, z);
            if fx.mem.i16(t + 0x30) == 0 {
                tube(fx, t, TRAIL_P, TRAIL_Q);
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
                billboard::ring_t20(fx, uv::uv_rect4, D, t, sp + 0xE0, BYTES);
            } else {
                billboard::ring_t(fx, uv::uv_rect4, D, t, sp + 0xE0, BYTES);
            }
        }
    }
    // The entry's s1: the idle pool B tasks counted here.
    let mut idle = 0u32;
    for k in 0..TASKS {
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
        // The steps' prologues: the entry's s0 = D (and s1 the idle count).
        if f == FLY {
            let s = fx.enter(0x20);
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
            fly(fx, t, idle);
            fx.leave(0x20);
            continue;
        }
        let s = fx.enter(0x30);
        fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(idle)), (0, u64::from(D))]);
        match f {
            ARC => arc(fx, t),
            WHITE => white(fx, t, false),
            WHITE_B => white(fx, t, true),
            _ => panic!("b701: unknown task step {f:#x}"),
        }
        fx.leave(0x30);
    }
    for k in 0..TASKS {
        let t = POOL_B + 0x70 * k;
        // The original reads the cell frame of pool A's task k (+0x34).
        let ta = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == FLY {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(ta + 0x34);
            cell(fx, sp + 0x90, 0x9B, sub);
            fx.mem.set16(D + 0x94, 0);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x40);
            fx.mem.set16(D + 0x9A, 0x40);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
        }
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (g, local) in [(WHITE, 0xA0u32), (WHITE_B, 0xB0)] {
            if f(fx) != g {
                continue;
            }
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(ta + 0x34);
            cell(fx, sp + local, 0x14, sub);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x51 => 1,
        0x91 => 4,
        0x5B => 2,
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

/// 0x1E034D0(D, step, 0): the first free 0x40-byte task cleared and set
/// off: alive 1, the colour D+0xE284 at +0x28 / +0x2C, D+0x2D0 as i16 at
/// +0, the kind D+0xE2E6 at +0x30, +0x20 = 0, +0x24 the step; `saves` the
/// caller's s0..s4, `ra` the return address.
fn spawn_a(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(POOL_A + 0x40 * k + 0x36) == 0) {
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

/// 0x1E03640(D, step, 0): the first free 0x70-byte task cleared and set
/// off at D+0x2D0 (i16 +0, float +0x10), the colour D+0xE284 at +0x60 /
/// +0x64, the kind at +0x50.
fn spawn_b(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(POOL_B + 0x70 * k + 0x58) == 0) {
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

/// 0x1E01770(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the spawns (frame 0x50: ra, the entry's s1 = 0, s0 = D; then s0 =
/// D, s1 a loop count). The spawns' saves: s0 = D, s1, the entry's s2 =
/// 0, s3 = the parameter, s4 the runner's (0 [H]).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_2EFC), (0x10, 0), (0, u64::from(D))]);
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
    if c(fx) == 0x51 {
        fx.mem.set16(KIND, 0);
        spawn_a(fx, TRAIL, saves(s1), 0x01E0_1804);
    }
    if c(fx) == 0x57 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2);
        node_position_saving(fx, h, slot, sp + 0x30, u64::from(D), Some(s1), 0x01E0_183C);
        let h = fx.target_handle();
        unit_position_saving(fx, h, sp + 0x40, u64::from(D), 0x01E0_1850);
        let w = fx.mem.read_u32(sp + 0x30);
        fx.mem.write_u32(POS, w);
        let w = fx.mem.read_u32(sp + 0x34);
        fx.mem.write_u32(POS + 4, w);
        let a = fx.mem.f(sp + 0x38);
        let z = ee::add(a, ee::div(ee::sub(fx.mem.f(sp + 0x48), a), 2.0));
        fx.mem.setf(POS + 8, z);
        fx.mem.set16(KIND, 1);
        for k in 0..8u64 {
            spawn_a(fx, RED, saves(k), 0x01E0_18A8);
        }
        for k in 0..0x14u64 {
            spawn_a(fx, DARK, saves(k), 0x01E0_18D0);
        }
        for k in 0..0x32u64 {
            spawn_b(fx, FLY, saves(k), 0x01E0_18F8);
        }
        s1 = 0x32;
        spawn_b(fx, ARC, saves(s1), 0x01E0_191C);
        spawn_a(fx, CAMERA, saves(s1), 0x01E0_1930);
    }
    let v = c(fx);
    if v == 0x1D || v == 0x3A || v == 0x5A {
        for (nra, gra) in [(0x01E0_1980u32, 0x01E0_19ACu32), (0x01E0_19DC, 0x01E0_1A08)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES);
            node_position_saving(fx, h, slot, POS, u64::from(D), Some(s1), nra);
            let y = ee::sub(fx.mem.f(POS + 4), 1.5);
            fx.mem.setf(POS + 4, y);
            for k in 0..0xDu64 {
                spawn_b(fx, WHITE, saves(k), gra);
            }
            s1 = 0xD;
        }
    }
    if c(fx) < 0x58 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, POS, u64::from(D), Some(s1), 0x01E0_1A50);
        let y = ee::sub(fx.mem.f(POS + 4), 1.5);
        fx.mem.setf(POS + 4, y);
        for k in 0..3u64 {
            spawn_b(fx, WHITE_B, saves(k), 0x01E0_1A7C);
        }
    }
    fx.leave(0x50);
}

/// 0x1E01AB0 (D, task): the trail: red (0xFF, 0x20, 0x20), length +6 10;
/// on its first update both trails filled with the attacker's nodes 6
/// (+0xDFD0) / 0xA (+0xE070), then (kind 0) each update to 90 a point
/// pushed onto each (c769f66b5e38); 0x80 to update 90, then down by 29.
fn trail(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    l2::rgb(fx, t + 0x28, 0xFF, 0x20, 0x20);
    // The step's s0 / s1: the caller's (D, 0) until the fill loops.
    let mut s01 = [u64::from(D), 0u64];
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 5);
        if fx.mem.i16(t + 0x30) == 0 {
            fx.mem.set16(t + 0xE, 0x1E);
            for (slot, buf, ra) in [(NODES + 2, TRAIL_Q, 0x01E0_1B2Cu32), (NODES + 4, TRAIL_P, 0x01E0_1B88)] {
                let h = fx.self_handle();
                let s = fx.mem.i16(slot);
                node_position_saving(fx, h, s, sp + 0x50, s01[0], Some(s01[1]), ra);
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
    if fx.mem.i16(COUNTER) < 0x5B {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x1F {
            fx.mem.write_u8(t + 0x2B, a - 0x1D);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(t + 0x30) == 0 && fx.mem.i16(COUNTER) < 0x5B {
        for (slot, buf, ra) in [(NODES + 2, TRAIL_Q, 0x01E0_1C68u32), (NODES + 4, TRAIL_P, 0x01E0_1CA0)] {
            let h = fx.self_handle();
            let s = fx.mem.i16(slot);
            node_position_saving(fx, h, s, sp + 0x50, s01[0], Some(s01[1]), ra);
            history(fx, buf + 0x90, 0xA, sp + 0x50);
        }
    }
    fx.mem.set16(t + 6, 0xA);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E01CF0 (`dark`: size 10, dark red, radius growing by 10..29, row
/// 0..9, life 5..11; in by 0x38 under 0x71, out by 10) / 0x1E01EA0
/// (`red`: size 6, red, radius growing by 3..18 from the start, row 0..9,
/// life 10..29; 0x80 during the life, out by 10) (D, task): a ring of the
/// 0x40-byte task kind (cell 0..3, random angles, a random angle +0x3A).
fn ring_a(fx: &mut Fx, t: u32, red: bool) {
    if fx.mem.i16(t + 0x36) == 1 {
        if red {
            fx.mem.set16(t + 0x38, 6);
            let v = fx.rand_from(t, 0x01E0_1EC8);
            fx.mem.set16(t + 0x34, v % 4);
            l2::rgb(fx, t + 0x28, 0xFF, 0x40, 0x40);
            let v = rmod(fx, t, 0x01E0_1EFC, 0xFFFF);
            fx.mem.set32(t + 0x18, v);
            let v = rmod(fx, t, 0x01E0_1F18, 0xFFFF);
            fx.mem.set32(t + 0x10, v);
            fx.mem.set16(t + 0x30, 0);
            let v = rmod(fx, t, 0x01E0_1F38, 0x168);
            fx.mem.set16(t + 0x3A, v);
            let v = fx.rand_from(t, 0x01E0_1F54);
            fx.mem.set16(t + 0xE, v % 0x10 + 3);
            let v = rmod(fx, t, 0x01E0_1F74, 0xA);
            fx.mem.set16(t + 8, v);
            let v = rmod(fx, t, 0x01E0_1F90, 0x14);
            fx.mem.set16(t + 6, v + 0xA);
        } else {
            fx.mem.set16(t + 0x38, 0xA);
            let v = fx.rand_from(t, 0x01E0_1D18);
            fx.mem.set16(t + 0x34, v % 4);
            for (o, ra) in [(0x10u32, 0x01E0_1D34u32), (0x14, 0x01E0_1D50), (0x18, 0x01E0_1D6C)] {
                let v = rmod(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
            l2::rgb(fx, t + 0x28, 0x20, 8, 8);
            fx.mem.set16(t + 0x30, 0);
            let v = rmod(fx, t, 0x01E0_1D98, 0x168);
            fx.mem.set16(t + 0x3A, v);
            let v = rmod(fx, t, 0x01E0_1DB4, 0x14);
            fx.mem.set16(t + 0xE, v + 0xA);
            let v = rmod(fx, t, 0x01E0_1DD4, 0xA);
            fx.mem.set16(t + 8, v);
            let v = rmod(fx, t, 0x01E0_1DF0, 7);
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

/// 0x1E029F0 (D, task): a camera light (+0xDF80: 128, 4, 2; +0xDF90 the
/// task's point; +0xDFA0 (0, 120, w)), its w up by 0.2 to 1, then down by
/// 0.1 to 0.1 (the task ending); 0x1F7E20 slots 4 / 5 from it, 0 once the
/// task ends or the timer is at 1.
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
    let (p, ra) = if fx.mem.i16(t + 0x36) == 0 { (0, 0x01E0_2B90) } else { (CAM, 0x01E0_2BA4) };
    camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E02030 (D, task): a red quad of size 100 growing by 10 to 200 (a
/// random cell row), flying (-50..49) / 60 a update, turned by the
/// camera's angles; in by 0x40 under 0x80 for 20 updates, out by 9.
/// `idle` the entry's s1.
fn fly(fx: &mut Fx, t: u32, idle: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x64);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2060);
        fx.mem.set16(t + 0x56, (v >> 4) % 3);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_2094u32), (0x30, 0x01E0_20C0), (0x38, 0x01E0_20EC)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_2144, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(idle), 0x01E0_2160);
        for (o, ra) in [(4u32, 0x01E0_2168u32), (2, 0x01E0_2188), (0, 0x01E0_21A8)] {
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
        let s = fx.mem.i16(t + 8);
        if s < 0xC8 {
            fx.mem.set16(t + 8, s + 0xA);
        }
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
        for o in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(t + 0x30 + o));
            fx.mem.setf(t + 0x10 + o, v);
        }
    }
    for (o, by) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + o).wrapping_add(fx.mem.i16(t + by) as u32);
        fx.mem.write_u32(t + o, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(idle), 0x01E0_22B0);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E022D0 (D, task): a white arc ring (red inner, radius 220 growing
/// by 0.4, angles 10 / 10) facing the camera turned a quarter; its outer
/// alpha in by 0x60 under 0xC0 for 8 updates, out by 15; it ends on
/// update 195.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.write_u32(t + 0x4C, 0x435C_0000);
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
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2398);
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

/// 0x1E02410 (size 130; in by 0x40 under 0x80 for 10 updates, out by 9;
/// 0.8 a update) / 0x1E025E0 (`b`: size 150; in by 0x60 under 0xC0 for 6
/// updates, out by 19; 0.6 a update) (D, task): a white quad (cell row 2..4)
/// set 8.6 out along a random angle and drifting along it, turned by the
/// camera's angles.
fn white(fx: &mut Fx, t: u32, b: bool) {
    let f = if b { WHITE_B } else { WHITE };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, if b { 0x96 } else { 0x82 });
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, f + 0x38);
        fx.mem.set16(t + 0x56, (v >> 4) % 3 + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, f + 0x70);
        fx.mem.set16(t + 0x5A, (v >> 4) % 0x168);
        step_along(fx, t, f32::from_bits(0x4109_999A));
    }
    let a = fx.mem.read_u8(t + 0x63);
    let (life, cap, up, floor, down) = if b { (6, 0xC0, 0x60, 0x15, 0x13) } else { (0xA, 0x80, 0x40, 0xB, 9) };
    if fx.mem.i16(t + 0x54) < life {
        if a < cap {
            fx.mem.write_u8(t + 0x63, a + up);
        }
    } else if a >= floor {
        fx.mem.write_u8(t + 0x63, a - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let by = if b { 0x3F19_999A } else { 0x3F4C_CCCD };
        step_along(fx, t, f32::from_bits(by));
    }
    let ra = if b { 0x01E0_2788 } else { 0x01E0_25B8 };
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), ra);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// z += by sin(+0x5A), x += by cos(+0x5A) (D+0xE4F0's sines).
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

/// 0x1E037D0(D, task, p, q) (this record only): a tube along the n = i16
/// +6 points `p` (16 bytes apart): for each segment k (k + 1 < n), 18
/// rings of 4 untextured gouraud strips (0x109BC0 additive, 0x109DD0)
/// round the segment at angles -90 + 10 j, radius D+0 (x) / D+8 (z), the
/// y offset -(p[0].y - q[0].y), each strip's first and third vertices
/// nudged 0.2 in +z / -z / +x / -x; the colour +0x28 at the first two
/// vertices, its alpha then less alpha / n at the last two (falling along
/// the tube); projected with the camera, far Z, no key [F].
fn tube(fx: &mut Fx, t: u32, p: u32, q: u32) {
    tube_by(fx, t, p, q, false);
}

/// b564's 0x1E03230 [F word-compare]: [`tube`] with the ring's z radius
/// D+8 shifted back by 0.6 of itself (z = D+8 (cos - 0.6)) and each strip
/// keyed by its mean w (0x1A2C70) instead of the far Z.
pub(crate) fn tube_b(fx: &mut Fx, t: u32, p: u32, q: u32) {
    tube_by(fx, t, p, q, true);
}

/// The ring layout of a [`tube`]: the first ring index (its angle -90 +
/// 10 x first), the ring count end (exclusive), the strip nudge (bits)
/// and, for the mean-w key, the return address of its 0x1A2C70 (helper
/// F1).
#[derive(Clone, Copy)]
pub(crate) struct TubeRings {
    pub first: i32,
    pub end: i32,
    pub nudge: u32,
    pub key_ra: u32,
    /// b397 / card 575's compile (0x1E02D00): a 0x210 frame (every local
    /// 0x10 lower), z = D+8 (cos - 1), no y offset, key 0 without the far
    /// Z.
    pub plain: bool,
}

/// b701 / b564's rings: 0..18, nudge 0.2.
const RINGS: TubeRings = TubeRings { first: 0, end: 0x12, nudge: 0x3E4C_CCCD, key_ra: 0x01E0_35F0, plain: false };

/// [`tube_b`] on the ring layout `r` (b205 / card 562's 0x1E02CE0: rings
/// 2..15, nudge 0.3).
pub(crate) fn tube_b_with(fx: &mut Fx, t: u32, p: u32, q: u32, r: TubeRings) {
    tube_rings(fx, t, p, q, true, r);
}

fn tube_by(fx: &mut Fx, t: u32, p: u32, q: u32, b: bool) {
    tube_rings(fx, t, p, q, b, RINGS);
}

fn tube_rings(fx: &mut Fx, t: u32, p: u32, q: u32, b: bool, r: TubeRings) {
    let frame = if r.plain { 0x210 } else { 0x220 };
    // The locals at their 0x220-frame offsets from here.
    let sp = fx.enter(frame).wrapping_sub(0x220 - frame);
    let ot = OT;
    let d = sp + 0x188;
    let n = fx.mem.i16(t + 6);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    for k in 0..4 {
        let b = fx.mem.read_u8(t + 0x28 + k);
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
    let f23 = if b && !r.plain { ee::mul(f32::from_bits(0x3F19_999A), f25) } else { f25 };
    let mut seg = p;
    let mut i = 0;
    while i < n - 1 {
        l2::colour_strip4(fx, d, sp + 0x21C, 1);
        let alpha = i32::from(fx.mem.read_u8(t + 0x2B));
        let q = fx.div(alpha, n).0 & 0xFF;
        let v = fx.mem.read_u8(sp + 0x21F).wrapping_sub(q as u8);
        fx.mem.write_u8(sp + 0x21F, v);
        l2::colour_strip4(fx, d, sp + 0x21C, 2);
        let mut deg = -0x5Ai32 + 0xA * r.first;
        for _ in r.first..r.end {
            let s = l2::sin_deg(fx, D + 0xD8, deg as i16 as i32, 0);
            let f21 = ee::mul(fx.mem.f(a), s);
            let c = l2::sin_deg(fx, D + 0xD8, deg as i16 as i32, 1);
            let f22 = if b || r.plain { ee::sub(ee::mul(f25, c), f23) } else { ee::mul(f23, c) };
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
                if !r.plain {
                    l2::add_column(fx, m, 1, f24);
                }
                let nudge = f32::from_bits(r.nudge);
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
                    if r.plain {
                        fx.submit(d, ot, 0);
                    } else if b {
                        let w = fx.mem.f(sp + 0x110);
                        fx.fix_residue(w, r.key_ra);
                        let key = ee::fixunssfsi(w) as i32;
                        fx.submit(d, ot, key);
                    } else {
                        l2::far_z_desc(fx, d, 1);
                        fx.submit(d, ot, 0);
                    }
                }
            }
            deg += 0xA;
        }
        seg += 0x10;
        i += 1;
    }
    fx.leave(frame);
}
