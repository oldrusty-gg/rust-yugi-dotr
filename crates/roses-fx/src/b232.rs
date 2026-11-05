//! BMODULE b232 (card 324), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01360, data area 0x1ED0000) [F] (helper C7, 31
//! Oct 2026): a pool of 150 tasks (0x70 bytes at +0x2A0, alive +0x58, step
//! +0x44 called through `jalr`) drawn with the second library and two
//! local routines. The stage light (0x1E00EA0) dims from update 1; on
//! update 0x23 two rings of 18 strips (0x1E00710 drawn by the local
//! 0x1E01B80) at the attacker's node 1, growing and pulsing; from 0x2D a
//! full-window frame quad (0x1E00F80); the node point drifts to the
//! target's node 1 from 0x9B; on the odd updates 0xBF..0xEF and 0xE2..0x12C
//! spark rings rising from it (0x1E00AC0 / 0x1E00970, `quads4::sparks`);
//! from 0xEC a unit flash (0x1E01120 inside the local 0x1E01ED0: the
//! target drawn into the render target through 0x1F8270, `Fx::unit_draw`,
//! in a packet layer, swept by a frame sprite). Returns 1 on update 0xE2,
//! 4 on 0x145, 2 on 0x13B, 6 on 0xED..0x135, 8 once the 360-update timer
//! +0x44B0 has run out.
//!
//! The draw loop's steps 0x1E00BE0 / 0x1E00D50 are never spawned [F: the
//! spawns name 0x1E00710 / 0x970 / 0xAC0 / 0xEA0 / 0xF80 / 0x1120 only];
//! only their draw branches are ported.
use crate::blib::quads4;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0x96;
const RINGS: u32 = 0x01E0_0710;
const SPARKS_UP: u32 = 0x01E0_0970;
const SPARKS: u32 = 0x01E0_0AC0;
const UNSPAWNED_A: u32 = 0x01E0_0BE0;
const UNSPAWNED_B: u32 = 0x01E0_0D50;
const LIGHT: u32 = 0x01E0_0EA0;
const FRAME: u32 = 0x01E0_0F80;
const FLASH: u32 = 0x01E0_1120;
/// The return address of the entry's `jalr` into a step (0x1E015F4 + 8).
const TASK_RA: u64 = 0x01E0_15FC;
const TIMER: u32 = D + 0x44B0;
const COLOUR: u32 = D + 0x44B4;
const OT_AT: u32 = D + 0x44BC;
const FLASH_DESC: u32 = D + 0x44C8;
const NODE: u32 = D + 0x44E4;
const RECTS: u32 = D + 0x4504;
const ANGLE_FLAG: u32 = D + 0x4550;
const FLASHES: u32 = D + 0x4558;
const COUNTER: u32 = D + 0x455A;
const SINES: u32 = D + 0x475C;

/// The entry 0x1E01360.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x48D0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4556, param);
    fx.mem.set32(TIMER, 0x168);
    for (k, n) in [1, 0x34, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0x44F4, r);
    for (k, id) in [0xEu32, 0xF].into_iter().enumerate() {
        let tex = fx.texture(4, id as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k as u32, tex);
    }
    for k in 0..3u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x4490 + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.fix_residue(v, 0x01E0_14F4);
    fx.mem.set16(D + 0x4554, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(k, 0x01E0_151C);
        let (_, h) = fx.div(r, 0x1E);
        fx.mem.write_u8(D + 0x455C + k, (h - 0xF) as u8);
    }
    for k in 0..0x1Eu32 {
        let r = fx.rand_from(k, 0x01E0_1564);
        fx.mem.set16(RECTS + 2 * k, r.wrapping_rem(8));
    }
    0
}

/// The draw's head: texture `tex` into D+0x88, the atlas cell `id` frame
/// +0x56 (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, tex: u32, r: u32, id: i32) {
    let w = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, w);
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// The sparks' draw head: the whole 32 x 64 cell, the task's position in
/// D+0x20..+0x28.
fn sparks_head(fx: &mut Fx, t: u32) {
    for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x1F), (0x9A, 0x3F), (0xB8, 0)] {
        fx.mem.set16(D + o, v);
    }
    for k in 0..3 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(D + 0x20 + 4 * k, w);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.mem.set16(FLASHES, 0);
    emit(fx);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 {
            continue;
        }
        if f == FLASH {
            let n = fx.mem.i16(FLASHES) + 1;
            fx.mem.set16(FLASHES, n);
            continue;
        }
        run_task(fx, f, slot + 0x2A0);
    }
    let mut a3 = 0u32;
    let mut live = 0u64;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        live += 1;
        let f = fx.mem.read_u32(t + 0x44);
        if f == RINGS {
            // 0x1F6830's frame (s0 = the live count, s1 = D): its zero
            // upper halves are the rings' missing-texture TEX0 [run].
            let h = fx.self_handle();
            let s = fx.mem.i16(NODE);
            crate::b076::node_position_saving(fx, h, s, sp + 0x50, live, Some(u64::from(D)), 0x01E0_1680);
            cell(fx, t, D + 0x4494, sp + 0x60, 0x14);
            fx.mem.set16(D + 0xB8, 1);
            fx.mem.set16(D + 0xBE, 0);
            a3 = ring(fx, t, 0x20, sp + 0x50);
        }
        let n = fx.mem.i16(COUNTER);
        if f == SPARKS_UP {
            let w = fx.mem.read_u32(D + 0x4498);
            fx.mem.write_u32(D + 0x88, w);
            sparks_head(fx, t);
            fx.mem.set16(D + 0xB8, 0x24);
            fx.mem.set16(D + 0xBE, 0);
            if n >= 0xF0 {
                quads4::sparks(fx, D, t, 0x20, t + 0x10);
                a3 = 0;
            }
        }
        if f == SPARKS {
            let w = fx.mem.read_u32(D + 0x4498);
            fx.mem.write_u32(D + 0x88, w);
            sparks_head(fx, t);
            fx.mem.set16(D + 0xB8, 0x24);
            fx.mem.set16(D + 0xBE, 0);
            if n < 0xED {
                quads4::sparks(fx, D, t, 0x20, t + 0x10);
                a3 = 0;
            }
            fx.mem.set16(D + 0xB8, 0);
        }
        for (step, r, drawn) in [(UNSPAWNED_A, sp + 0x70, n < 0xED), (UNSPAWNED_B, sp + 0x80, n >= 0xEC)] {
            if f == step {
                cell(fx, t, D + 0x4498, r, 0x8C);
                sparks_head(fx, t);
                if drawn {
                    quads4::sparks(fx, D, t, 0x20, t + 0x10);
                    a3 = 0;
                }
                fx.mem.set16(D + 0xB8, 0);
            }
        }
    }
    if fx.mem.i16(FLASHES) != 0 {
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xE2 => 1,
        0x145 => 4,
        0x13B => 2,
        0xED..=0x135 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn run_task(fx: &mut Fx, f: u32, t: u32) {
    match f {
        RINGS => rings(fx, t),
        SPARKS_UP => sparks_up(fx, t, true),
        SPARKS => sparks_up(fx, t, false),
        LIGHT => light(fx, t),
        FRAME => frame(fx, t),
        _ => panic!("b232: unknown task step {f:#x}"),
    }
}

/// 0x1E00480(D, 0): the update counter +0x455A and the emitters (the node
/// point +0x4470 at the attacker's node 1, from 0x9B drifting by 1/32 a
/// update to the target's node 1 until 0xB9).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x10, 0x01E0_15BC), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let n = fx.mem.i16(COUNTER);
    if n < 0x9B {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE);
        fx.node_position(h, s, D + 0x4470);
    } else {
        if n == 0x9B {
            let t = fx.target_handle();
            let s = fx.mem.i16(D + 0x44F4);
            fx.node_position(t, s, sp + 0x20);
            for (v, a, p) in [(0x4488u32, 0x28u32, 0x4478u32), (0x4484, 0x24, 0x4474)] {
                let d = ee::div(ee::sub(fx.mem.f(sp + a), fx.mem.f(D + p)), 32.0);
                fx.mem.setf(D + v, d);
            }
        }
        if fx.mem.i16(COUNTER) < 0xBA {
            for (p, v) in [(0x4478u32, 0x4488u32), (0x4474, 0x4484)] {
                let x = ee::add(fx.mem.f(D + p), fx.mem.f(D + v));
                fx.mem.setf(D + p, x);
            }
        }
    }
    let regs = [D, 0, 0, 0, 0];
    let n = fx.mem.i16(COUNTER);
    if n == 1 {
        spawn(fx, LIGHT, &regs, 0x01E0_0574);
    }
    if n == 0x23 {
        fx.mem.set16(ANGLE_FLAG, 0);
        spawn(fx, RINGS, &regs, 0x01E0_059C);
        fx.mem.set16(ANGLE_FLAG, 1);
        spawn(fx, RINGS, &regs, 0x01E0_05B8);
    }
    if n == 0x2D {
        spawn(fx, FRAME, &regs, 0x01E0_05DC);
    }
    if n == 0xEC {
        spawn(fx, FLASH, &regs, 0x01E0_0600);
    }
    if (0xBF..0xF1).contains(&n) {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE);
        fx.node_position(h, s, D + 0x250);
        if n.wrapping_rem(2) == 1 {
            spawn(fx, SPARKS, &regs, 0x01E0_066C);
        }
        let v = fx.mem.i16(D + 0x454A) + 3;
        fx.mem.set16(D + 0x454A, v);
    }
    if (0xE2..0x12D).contains(&n) {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE);
        fx.node_position(h, s, D + 0x250);
        fx.mem.write_u32(D + 0x258, 0);
        let y = ee::add(fx.mem.f(D + 0x254), 10.0);
        fx.mem.setf(D + 0x254, y);
        if n.wrapping_rem(2) == 1 {
            spawn(fx, SPARKS_UP, &regs, 0x01E0_0700);
        }
    }
    fx.leave(0x40);
}

/// 0x1E01A40(D, step, 0): the first free task cleared and set off at
/// D+0x250 with the colour +0x44B4 (twice), angle flag +0x4550; `regs` the
/// caller's s0..s4, `ra` the return address.
fn spawn(fx: &mut Fx, f: u32, regs: &[u32; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    let r = |k: usize| u64::from(regs[k]);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, r(4)), (0x30, r(3)), (0x20, r(2)), (0x10, r(1)), (0, r(0))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            // 0x107390's `sq ra` upper half (0) [run, store-watched].
            fx.mem.write_u64(sp - 8, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(ANGLE_FLAG);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand % m (HI) from a step's rand returning to `ra` (s0 = the task).
fn rand_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// The alpha byte `a` up by `up.1` while below `up.0` when `rising`, else
/// down by `down.1` while at least `down.0` (the task ending below when
/// `ends`).
fn fade(fx: &mut Fx, t: u32, a: u32, rising: bool, up: (u8, u8), down: (u8, u8), ends: bool) {
    let v = fx.mem.read_u8(t + a);
    if rising {
        if v < up.0 {
            fx.mem.write_u8(t + a, v.wrapping_add(up.1));
        }
    } else if v >= down.0 {
        fx.mem.write_u8(t + a, v.wrapping_sub(down.1));
    } else if ends {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn odd(v: i32) -> bool {
    v.wrapping_rem(2) == 1
}

/// 0x1E00710: a ring (cell frame 0..6, row +8 4..6, phase +0xE) fading in
/// by 0xC until update 0xF0 then out by 9, flickering grey; radius +0x48
/// up to 20 by 0.5 (inner +0x4C 7 less), its turn +0x5C 20 sin(+0xE),
/// +0xE + 5 and +0x5A + 10 a update.
fn rings(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rand_mod(fx, t, 0x01E0_073C, 7);
        fx.mem.set16(t + 0x56, v);
        fx.mem.set16(t + 0x58, 2);
        let v = rand_mod(fx, t, 0x01E0_0760, 3);
        fx.mem.set16(t + 8, v + 4);
        let v = rand_mod(fx, t, 0x01E0_0780, 0x168);
        fx.mem.set16(t + 0xE, v);
        fx.mem.set16(t + 0x5C, 0);
        fx.mem.set16(t + 0x5A, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 0xF0;
    fade(fx, t, 0x63, rising, (0x80, 0xC), (0xB, 9), true);
    fade(fx, t, 0x67, rising, (0x80, 0xC), (0xB, 9), false);
    let g = if odd(fx.mem.i16(t + 0x54)) { 0x80 } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    l2::rgb(fx, t + 0x64, g, g, g);
    if fx.mem.i16(COUNTER) != 0 {
        let r = fx.mem.f(t + 0x48);
        if ee_lt(r, 20.0) {
            fx.mem.setf(t + 0x48, ee::add(r, 0.5));
        }
        let inner = ee::sub(fx.mem.f(t + 0x48), 7.0);
        fx.mem.setf(t + 0x4C, inner);
        if ee_lt(inner, 0.0) {
            fx.mem.write_u32(t + 0x4C, 0);
        }
    }
    let a = fx.mem.i16(t + 0xE);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let v = ee::mul(20.0, s);
    fx.fix_residue(v, 0x01E0_0924);
    fx.mem.set16(t + 0x5C, ee::fixsfsi(v));
    let e = fx.mem.i16(t + 0xE) + 5;
    fx.mem.set16(t + 0xE, e);
    let y = fx.mem.i16(t + 0x5A) + 0xA;
    fx.mem.set16(t + 0x5A, y);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E00970 / 0x1E00AC0: a spark ring (radius 200, inner 58 less, rising
/// by 95 a update, bright for 0xF / 0xD updates, then fading by 0xE), the
/// rising kind turned by -/+0x1800 by the attacker's facing.
fn sparks_up(fx: &mut Fx, t: u32, turned: bool) {
    let frame = if turned { 0x30 } else { 0x20 };
    let sp = fx.enter(frame);
    if turned {
        fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    } else {
        fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    }
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.setf(t + 0x4C, ee::sub(200.0, 58.0));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.write_u32(t + 0x34, 0x446D_8000);
        let v = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        if turned {
            let a = if fx.mem.i16(D + 0x4554) != 0 { -0x1800 } else { 0x1800 };
            fx.mem.set32(t + 0x20, a);
        }
    }
    let bright = if turned { 0xF } else { 0xD };
    if fx.mem.i16(t + 0x54) < bright {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0x10, 0xE), true);
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::sub(r, 400.0));
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(frame);
}

/// 0x1E00EA0: the stage light +0x50 from 0x80 down by 10 to 0x30 until
/// its update 0x96, then (timer below 0x1E) up by 10 to 0x80; full and the
/// task over at timer 3.
fn light(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let v = fx.mem.i16(t + 0x50);
    if fx.mem.i16(t + 0x54) < 0x97 {
        fx.mem.set16(t + 0x50, if v < 0x31 { 0x30 } else { v - 0xA });
    } else if fx.mem.i32(TIMER) < 0x1E {
        fx.mem.set16(t + 0x50, if v < 0x80 { v + 0xA } else { 0x80 });
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
    fx.leave(0x20);
}

/// 0x1E00F80: a full-window frame quad (colour 0x90 / 0x90 / 0x78, the
/// task's alpha, ALPHA 0x44) fading in by 10 to 0x50 until its update
/// 0x111, then out by 6.
fn frame(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x80);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x28;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.mem.read_u8(t + 0x63);
    for (k, v) in [0x90u8, 0x90, 0x78, a].into_iter().enumerate() {
        fx.mem.write_u8(sp + 0x38 + k as u32, v);
    }
    fx.mem.write_u32(sp + 0x3C, 0x3F80_0000);
    for (at, xy) in [(0x48u32, 0x6FF8u16), (0x58, 0x8FF8)] {
        fx.mem.write_u16(sp + at, xy);
        fx.mem.write_u16(sp + at + 2, xy);
        let v = fx.mem.read_u64(sp + at);
        fx.mem.write_u64(sp + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(sp + at + 7, 0);
    }
    for (o, v) in [(0x40u32, 0u32), (0x44, 0), (0x50, 0x3F80_0000), (0x54, 0x3F80_0000)] {
        fx.mem.write_u32(sp + o, v);
    }
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    // After 0x207E20: a2 = the flags | 0x200, a3 = the flags.
    fx.frame_desc(d, 1, [0x226, 0x26]);
    // 0x207FC0's `sq ra` upper half (0) under 0x207EF0 [run, store-watched]:
    // the rings' 0x1099E0 copies it as their rotation's w.
    fx.mem.write_u64(sp - 0x28, 0);
    fx.submit(d, OT, 0);
    let rising = fx.mem.i16(t + 0x54) < 0x112;
    fade(fx, t, 0x63, rising, (0x50, 0xA), (8, 6), true);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x80);
}

/// 0x1E01120: the flash's frame sprite (the target over the window from x
/// +8 on, colour 0x38 grey, the task's alpha) into the descriptor at
/// +0x44C8 twice, no ordering table; x 10 cos(+0x5A) (+0x5A + 0x19 a
/// update, from 0 or 180 by +0x50); alpha rising by 10 below 0x80 until
/// update 0x50, then falling by 9. `regs` the caller's s0..s2.
fn flash(fx: &mut Fx, t: u32, regs: [u32; 3]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_1FA4), (0x20, u64::from(regs[2])), (0x10, u64::from(regs[1])), (0, u64::from(regs[0]))]);
    let x0 = fx.mem.i16(t + 8);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x38, 0x38, 0x38);
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.set16(t + 0x5A, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.set16(t + 0x5A, 0xB4);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, x, y) in [(0x20u32, x0 + 0x700, 0x6FF8u16), (0x30, x0 + 0x900, 0x8FF8)] {
        fx.mem.write_u16(d + at, ((x << 4) - 8) as u16);
        fx.mem.write_u16(d + at + 2, y);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    for (o, v) in [(0x18u32, 0u32), (0x1C, 0), (0x28, 0x3F80_0000), (0x2C, 0x3F80_0000)] {
        fx.mem.write_u32(d + o, v);
    }
    fx.submit(d, 0, 0);
    fx.submit(d, 0, 0);
    let a = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, SINES, a, 1);
    let v = ee::mul(10.0, c);
    fx.fix_residue(v, 0x01E0_12D4);
    fx.mem.set16(t + 8, ee::fixsfsi(v));
    let a = fx.mem.i16(t + 0x5A) + 0x19;
    fx.mem.set16(t + 0x5A, a);
    let rising = fx.mem.i16(t + 0x54) < 0x50;
    fade(fx, t, 0x63, rising, (0x80, 0xA), (0xB, 9), true);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x40);
}

/// 0x1E01ED0(D): in a packet layer (0x2078A0) the target drawn into the
/// render target (0x1F8270 filter 0, its TEX0 / TEX1 into the sprite
/// descriptor at sp + 0x58; ALPHA 0x48), TEXFLUSH before and after; every
/// flash task stepped (0x1E01120); the layer linked into the ordering
/// table at key 0 when any task lives.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xB0);
    let d = sp + 0x58;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(D + 0x44C4, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    // a0..a3 at the first 0x207BB0: 5, the descriptor, 0x26, the caller's.
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let t = fx.target_handle();
    let (_, regs) = fx.unit_draw(t, 0, sp + 0x98, sp + 0xA0);
    fx.texflush(regs);
    let mut live = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        live += 1;
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            flash(fx, slot + 0x2A0, [k, live, slot]);
        }
    }
    if live != 0 {
        let layer = fx.mem.read_u32(D + 0x44C4);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}

/// 0x1E01B80(D, t, mode, pos): a ring of 18 textured strips (strip3, the
/// cells +0x4504 + 2 k, UVs in D+0xBE's orientation) between radius +0x4C
/// (outer, at the first and second vertex) and +0x48, 20 degrees apart
/// from -200, turned about z by +0x5C and y by +0x5A (0x1099E0), through
/// the task's angles +0x20 at `pos` (0x1097D0); blend mode >> 4 (0x42 when
/// 0); keyed by the mean w. Returns a3 at its last 0x206F30 (0) [run].
fn ring(fx: &mut Fx, t: u32, mode: i32, pos: u32) -> u32 {
    let sp = fx.enter(0x210);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    let d = sp + 0x180;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 1);
    l2::colour_strip3(fx, d, t + 0x64, 2);
    let o = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, o);
    let rh2 = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, rh2, 1);
    let b = mode >> 4;
    let alpha = if b == 0 { 0x80_0000_0042 } else { 0x80_0000_0040 | ((b as i64 as u64) << 2) };
    fx.desc_alpha(d, alpha);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0xB0 + 4 * k);
        fx.mem.write_u32(sp + 0x100 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x110, 0, t + 0x20, sp + 0x100);
    let (r_in, r_out) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let a = fx.mem.i16(t + 0x5A);
    fx.mem.set32(sp + 0x164, a);
    let a = fx.mem.i16(t + 0x5C);
    fx.mem.set32(sp + 0x168, a);
    let mut s3 = 0i32;
    for k in 0..0x12u32 {
        for o in [0xC4u32, 0xD4, 0xE4, 0xF4] {
            fx.mem.write_u32(sp + o, 0);
        }
        let frame = fx.mem.i16(RECTS + 2 * k);
        l2::rect(fx, sp + 0x170, 0x14, frame);
        for c in 0..4 {
            let w = fx.mem.read_u32(sp + 0x170 + 4 * c);
            fx.mem.write_u32(D + 0x94 + 4 * c, w);
        }
        let o = fx.mem.i16(D + 0xBE);
        l2::uv_strip3(fx, d, D + 0x94, o);
        for (deg, inner, outer) in [(s3 - 0xC8, 0xE0u32, 0xC0u32), (s3 - 0xB4, 0xF0, 0xD0)] {
            let deg = deg as i16 as i32;
            let c = l2::sin_deg(fx, SINES, deg, 1);
            let s = l2::sin_deg(fx, SINES, deg, 0);
            fx.mem.setf(sp + inner, ee::mul(c, r_in));
            fx.mem.setf(sp + inner + 8, ee::mul(s, r_in));
            fx.mem.setf(sp + outer, ee::mul(c, r_out));
            fx.mem.setf(sp + outer + 8, ee::mul(s, r_out));
        }
        l2::rotate4(fx, sp + 0xC0, sp + 0x160, 2, SINES);
        l2::rotate4(fx, sp + 0xC0, sp + 0x160, 0, SINES);
        if l2::project(fx, sp + 0xC0, 0x7000_0000, sp + 0x110, d, sp + 0x150) {
            let w = fx.mem.f(sp + 0x150);
            fx.fix_residue(w, 0x01E0_1E50);
            let key = ee::fixunssfsi(w) as i32;
            let ot = fx.mem.read_u32(OT_AT);
            fx.submit(d, ot, key);
        }
        s3 = (s3 + 0x14) as i16 as i32;
    }
    fx.leave(0x210);
    0
}
