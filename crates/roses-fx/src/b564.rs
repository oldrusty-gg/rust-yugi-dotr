//! BMODULE b564 (card 473), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02790, data area 0x1ED0000) [F] (helper A12, 31
//! Oct 2026): b701's program at other offsets and in blue: pools of 150 x
//! 0x40 at D+0x380 (direct `jal`s) and 250 x 0x70 at D+0x2900 (`jalr`; the
//! flash tasks 0x1E02420 counted and run by b482's unit flash 0x1E036B0),
//! `billboard::ring_t` / `ring_t20` rings, quads, an arc ring and b701's
//! tube at 0x1E03230 (`b701::tube_b`: the z radius shifted, keyed by the
//! mean w) along the trails of the attacker's nodes 0x1E / 0x1F. Updates
//! 1..5 five dark flashes (to 80); 47..83 five white quads a update at
//! node 1; 68 the blue tube; 72 at node 0x1F halfway to the target: eight
//! blue and twenty dark rings, fifty small blue quads, a blue arc ring, a
//! camera light. Returns 1 on update 62, 4 on 95, 2 on 72, 8 once the
//! 99-update timer +0x9960 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, exe::camera_slots45, spawn::history, uv};
use crate::b701::tube_b;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS_A: u32 = 150;
const TASKS_B: u32 = 250;
/// The 0x40-byte tasks.
const POOL_A: u32 = D + 0x380;
/// The 0x70-byte tasks.
const POOL_B: u32 = D + 0x2900;
/// Pool A's steps.
const TRAIL: u32 = 0x01E0_14B0;
const DARK: u32 = 0x01E0_16F0;
const RED: u32 = 0x01E0_18A0;
const CAMERA: u32 = 0x01E0_2230;
/// Pool B's steps.
const FLY: u32 = 0x01E0_1A30;
const ARC: u32 = 0x01E0_1CC0;
const WHITE: u32 = 0x01E0_1E00;
const WHITE_B: u32 = 0x01E0_1FD0;
/// The flash tasks, run by the unit flash 0x1E036B0.
const FLASH: u32 = 0x01E0_2420;
/// The return address of the pool B loop's `jalr` (0x1E02C7C + 8).
const TASK_RA: u64 = 0x01E0_2C84;
const POS: u32 = D + 0x2D0;
const TEX4: u32 = D + 0x340;
const CAM: u32 = D + 0x9660;
const TRAIL_Q: u32 = D + 0x96B0;
const TRAIL_P: u32 = D + 0x9750;
const TEX: u32 = D + 0x9940;
const TIMER: u32 = D + 0x9960;
const COLOUR: u32 = D + 0x9964;
const LAYER: u32 = D + 0x9974;
const FLASH_DESC: u32 = D + 0x9978;
const NODES: u32 = D + 0x99AC;
const KIND: u32 = D + 0x99C6;
const FACING: u32 = D + 0x99CA;
const COUNTER: u32 = D + 0x99CC;
const BYTES: u32 = D + 0x99D0;
const SINES: u32 = D + 0x9BD0;


fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02790.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x170);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x996C, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x170);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x9D40);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x99C8, param);
    fx.mem.set32(TIMER, 0x63);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, node) in [1, 0x1E, 0x1F, 4, 5, 6].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // 0x1E026E0(0, D): the textures.
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
    fx.fix_residue(y, 0x01E0_292C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2940, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2978, 0x1E);
        fx.mem.set8(BYTES + 0x100 + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_29B0, 0x14);
        fx.mem.set8(BYTES + 0x100 + k, v - 0xA);
    }
    0
}

/// The last XYZ2 of the last primitive drawn since `before` (0 when none).
fn last_xyz2(fx: &mut Fx, before: usize) -> u32 {
    match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => 0,
    }
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
            fx.saved(s, &[(0x40, 0x01E0_2A24), (0x30, u64::from(slot)), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
            trail(fx, t);
            fx.leave(0x60);
        }
        for (f, ra) in [(DARK, 0x01E0_2A48u64), (RED, 0x01E0_2A68)] {
            if fx.mem.read_u32(t + 0x24) == f {
                let s = fx.enter(0x20);
                fx.saved(s, &[(0x10, ra), (0, u64::from(D))]);
                ring_a(fx, t, f == RED);
                fx.leave(0x20);
            }
        }
        if fx.mem.read_u32(t + 0x24) == CAMERA {
            let s = fx.enter(0x30);
            fx.saved(s, &[(0x20, 0x01E0_2A88), (0x10, 0), (0, u64::from(D))]);
            camera(fx, t);
            fx.leave(0x30);
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x70, u64::from(D), 0x01E0_2AA8);
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
            fx.mem.write_u32(D, 0x4060_0000);
            let z = if fx.mem.i16(FACING) != 0 { 0xC0C0_0000 } else { 0x40C0_0000 };
            fx.mem.write_u32(D + 8, z);
            if fx.mem.i16(t + 0x30) == 0 {
                tube_b(fx, t, TRAIL_P, TRAIL_Q);
            }
        }
        for (f, local, t20) in [(DARK, 0x80u32, false), (RED, 0x90, true)] {
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
    // The entry's s1: the flash tasks counted here.
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
            WHITE_B => white_b(fx, t),
            _ => panic!("b564: unknown task step {f:#x}"),
        }
        fx.leave(0x30);
    }
    // a3 as the draws' last library call left it for the unit flash's
    // first TEXFLUSH: 0 after a quad, the last XYZ2 after an arc (b482).
    let mut a3 = 0u32;
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
            cell(fx, sp + 0xA0, 0x9B, sub);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
            a3 = 0;
        }
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            a3 = last_xyz2(fx, before);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (g, local) in [(WHITE, 0xB0u32), (WHITE_B, 0xC0)] {
            if f(fx) != g {
                continue;
            }
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(ta + 0x34);
            cell(fx, sp + local, 0x14, sub);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
            a3 = 0;
        }
    }
    if flashes != 0 {
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x3E => 1,
        0x5F => 4,
        0x48 => 2,
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

/// 0x1E02F30(D, step, 0) (b701's 0x1E034D0 for 150 tasks): the first free
/// 0x40-byte task set off.
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

/// 0x1E030A0(D, step, 0) (b701's 0x1E03640 for 250 tasks at D+0x2900).
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
    fx.saved(sp, &[(0x20, 0x01E0_29F0), (0x10, 0), (0, u64::from(D))]);
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
    if c(fx) == 0x44 {
        fx.mem.set16(KIND, 0);
        spawn_a(fx, TRAIL, saves(s1), 0x01E0_12B4);
    }
    if c(fx) == 0x48 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 4);
        node_position_saving(fx, h, slot, sp + 0x30, u64::from(D), Some(s1), 0x01E0_12EC);
        let h = fx.target_handle();
        unit_position_saving(fx, h, sp + 0x40, u64::from(D), 0x01E0_1300);
        let w = fx.mem.read_u32(sp + 0x30);
        fx.mem.write_u32(POS, w);
        let w = fx.mem.read_u32(sp + 0x34);
        fx.mem.write_u32(POS + 4, w);
        let a = fx.mem.f(sp + 0x38);
        let z = ee::add(a, ee::div(ee::sub(fx.mem.f(sp + 0x48), a), 2.0));
        fx.mem.setf(POS + 8, z);
        fx.mem.set16(KIND, 1);
        for k in 0..8u64 {
            spawn_a(fx, RED, saves(k), 0x01E0_1358);
        }
        for k in 0..0x14u64 {
            spawn_a(fx, DARK, saves(k), 0x01E0_1380);
        }
        for k in 0..0x32u64 {
            spawn_b(fx, FLY, saves(k), 0x01E0_13A8);
        }
        s1 = 0x32;
        spawn_b(fx, ARC, saves(s1), 0x01E0_13CC);
        spawn_a(fx, CAMERA, saves(s1), 0x01E0_13E0);
    }
    if c(fx) < 6 {
        spawn_b(fx, FLASH, saves(s1), 0x01E0_1410);
    }
    if (0x2F..0x54).contains(&c(fx)) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, POS, u64::from(D), Some(s1), 0x01E0_1450);
        let y = ee::sub(fx.mem.f(POS + 4), 1.5);
        fx.mem.setf(POS + 4, y);
        for k in 0..5u64 {
            spawn_b(fx, WHITE_B, saves(k), 0x01E0_147C);
        }
    }
    fx.leave(0x50);
}

/// 0x1E014B0 (D, task): b701's trail in blue (0x20, 0x20, 0x80), the
/// trails of nodes 0x1F (+0x96B0) / 0x1E (+0x9750) to update 74, length
/// +6 5.
fn trail(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x80);
    let mut s01 = [u64::from(D), 0u64];
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 5);
        if fx.mem.i16(t + 0x30) == 0 {
            fx.mem.set16(t + 0xE, 0x1E);
            for (slot, buf, ra) in [(NODES + 4, TRAIL_Q, 0x01E0_152Cu32), (NODES + 2, TRAIL_P, 0x01E0_1588)] {
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
    if fx.mem.i16(COUNTER) < 0x4B {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x1F {
            fx.mem.write_u8(t + 0x2B, a - 0x1D);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(t + 0x30) == 0 && fx.mem.i16(COUNTER) < 0x4B {
        for (slot, buf, ra) in [(NODES + 4, TRAIL_Q, 0x01E0_1668u32), (NODES + 2, TRAIL_P, 0x01E0_16A0)] {
            let h = fx.self_handle();
            let s = fx.mem.i16(slot);
            node_position_saving(fx, h, s, sp + 0x50, s01[0], Some(s01[1]), ra);
            history(fx, buf + 0x90, 0xA, sp + 0x50);
        }
    }
    fx.mem.set16(t + 6, 5);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E016F0 (`dark`: size 10, dark blue, growth 3..18, row 0..9, life
/// 4..10; in by 0x38 under 0x71, out by 15) / 0x1E018A0 (`red`: size 6,
/// pale blue, growth 3..18 from the start, row 0..9, life 4..11; 0x80
/// during the life, out by 15) (D, task): b701's 0x40-byte rings.
fn ring_a(fx: &mut Fx, t: u32, red: bool) {
    if fx.mem.i16(t + 0x36) == 1 {
        if red {
            fx.mem.set16(t + 0x38, 6);
            let v = fx.rand_from(t, 0x01E0_18C8);
            fx.mem.set16(t + 0x34, v % 4);
            l2::rgb(fx, t + 0x28, 0x60, 0x60, 0xFF);
            let v = rmod(fx, t, 0x01E0_18FC, 0xFFFF);
            fx.mem.set32(t + 0x18, v);
            let v = rmod(fx, t, 0x01E0_1918, 0xFFFF);
            fx.mem.set32(t + 0x10, v);
            fx.mem.set16(t + 0x30, 0);
            let v = rmod(fx, t, 0x01E0_1938, 0x168);
            fx.mem.set16(t + 0x3A, v);
            let v = fx.rand_from(t, 0x01E0_1954);
            fx.mem.set16(t + 0xE, v % 0x10 + 3);
            let v = rmod(fx, t, 0x01E0_1974, 0xA);
            fx.mem.set16(t + 8, v);
            let v = fx.rand_from(t, 0x01E0_1990);
            fx.mem.set16(t + 6, v % 8 + 4);
        } else {
            fx.mem.set16(t + 0x38, 0xA);
            let v = fx.rand_from(t, 0x01E0_1718);
            fx.mem.set16(t + 0x34, v % 4);
            for (o, ra) in [(0x10u32, 0x01E0_1734u32), (0x14, 0x01E0_1750), (0x18, 0x01E0_176C)] {
                let v = rmod(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
            l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x60);
            fx.mem.set16(t + 0x30, 0);
            let v = rmod(fx, t, 0x01E0_1798, 0x168);
            fx.mem.set16(t + 0x3A, v);
            let v = fx.rand_from(t, 0x01E0_17B4);
            fx.mem.set16(t + 0xE, v % 0x10 + 3);
            let v = rmod(fx, t, 0x01E0_17D4, 0xA);
            fx.mem.set16(t + 8, v);
            let v = rmod(fx, t, 0x01E0_17F0, 7);
            fx.mem.set16(t + 6, v + 4);
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
        fx.mem.write_u8(t + 0x2B, a - 0xF);
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

/// 0x1E02230 (D, task): b701's camera light at +0x9660.
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
    let (p, ra) = if fx.mem.i16(t + 0x36) == 0 { (0, 0x01E0_23D0) } else { (CAM, 0x01E0_23E4) };
    camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E01A30 (D, task): a small blue quad (size 4..11, a random cell
/// row) flying (-50..49) / 60 a update, turned by the camera's angles; in
/// by 0x40 under 0x80 for 10 updates, out by 15. `flashes` the entry's s1.
fn fly(fx: &mut Fx, t: u32, flashes: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_1A54);
        fx.mem.set16(t + 8, v % 8 + 4);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_1A7C);
        fx.mem.set16(t + 0x56, (v >> 4) % 3);
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_1AB0u32), (0x30, 0x01E0_1ADC), (0x38, 0x01E0_1B08)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_1B60, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_1B7C);
        for (o, ra) in [(4u32, 0x01E0_1B84u32), (2, 0x01E0_1BA4), (0, 0x01E0_1BC4)] {
            let v = fx.rand_from(t, ra);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xA {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x63, a - 0xF);
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
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_1CA0);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01CC0 (D, task): b701's arc ring in blue (radius 120).
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.write_u32(t + 0x4C, 0x42F0_0000);
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0xFF);
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
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1D88);
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

/// 0x1E01E00 (D, task): b701's 0x1E02410 (never spawned here [F]).
fn white(fx: &mut Fx, t: u32) {
    let f = WHITE;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x82);
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
        step_along(fx, t, f32::from_bits(0x3F4C_CCCD));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), f + 0x1A8);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01FD0 (D, task): a white-blue quad of size 130 (pale blue before
/// update 60), cell row 2..4, set 8.6 out along a random angle and
/// drifting 0.6 a update along it; in by 0x40 under 0x80 for 10 updates,
/// out by 9 (by 19 from update 74, when its age jumps by 6).
fn white_b(fx: &mut Fx, t: u32) {
    let f = WHITE_B;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x82);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, f + 0x38);
        fx.mem.set16(t + 0x56, (v >> 4) % 3 + 2);
        let g = if fx.mem.i16(COUNTER) < 0x3C { 0xC0 } else { 0x80 };
        l2::rgb(fx, t + 0x60, g, g, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2074);
        fx.mem.set16(t + 0x5A, (v >> 4) % 0x168);
        step_along(fx, t, f32::from_bits(0x4109_999A));
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xA {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if fx.mem.i16(COUNTER) < 0x4A {
        if a >= 0xB {
            fx.mem.write_u8(t + 0x63, a - 9);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    } else if a >= 0x15 {
        fx.mem.write_u8(t + 0x63, a - 0x13);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        step_along(fx, t, f32::from_bits(0x3F19_999A));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_21E8);
    if fx.mem.i16(COUNTER) == 0x4A {
        let n = fx.mem.i16(t + 0x54) + 6;
        fx.mem.set16(t + 0x54, n);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// z += by sin(+0x5A), x += by cos(+0x5A) (D+0x9BD0's sines).
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

/// 0x1E02420(D, t): b482's flash step (the target over the window, UVs
/// inset by +8 / 10 and +0xA / 10 over 512, into the descriptor at
/// +0x9978, no ordering table) in dark blue-grey (0x38, 0x38, 0x40): alpha
/// up by 9 below 0x80 to update 80, then from its age 41 down by 9.
/// `regs` the unit flash's s0 / s1 (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_37BC), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f20 / f21: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x38, 0x38, 0x40);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, xy) in [(0x20u32, 0x6FF8u16), (0x30, 0x8FF8)] {
        fx.mem.write_u16(d + at, xy);
        fx.mem.write_u16(d + at + 2, xy);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    for (o, v) in [(0x18u32, ee::div(w, 512.0)), (0x1C, ee::div(h, 512.0)), (0x28, ee::div(ee::sub(512.0, w), 512.0)), (0x2C, ee::div(ee::sub(512.0, h), 512.0))] {
        fx.mem.setf(d + o, v);
    }
    fx.submit(d, 0, 0);
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(COUNTER) < 0x50 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 9);
        }
    } else if fx.mem.i16(t + 0x54) >= 0x29 {
        if a >= 0xB {
            fx.mem.write_u8(t + 0x63, a - 9);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x40);
}

/// 0x1E036B0(D) (b482's 0x1E03DF0 at other offsets): in a packet layer
/// (0x2078A0) the attacker drawn into the target (0x1F8270 filter 0;
/// ALPHA 0x48), TEXFLUSH before and after; the flash tasks among pool B's
/// first 150 stepped (0x1E02420); the layer linked at key 0 when any of
/// them was alive.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xB0);
    let d = sp + 0x58;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x98, sp + 0xA0);
    fx.texflush(regs);
    let mut live = 0u32;
    for k in 0..0x96u32 {
        let t = POOL_B + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        if fx.mem.read_u32(t + 0x44) == FLASH {
            flash(fx, t, [k, live]);
        }
    }
    if live != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}
