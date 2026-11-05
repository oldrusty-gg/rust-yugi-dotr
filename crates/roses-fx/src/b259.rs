//! BMODULE b259 (card 378), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02BB0, data area 0x1ED0000) [F] (helper A10, 31
//! Oct 2026): b759's program shape (300 tasks of 0x70 bytes at D+0x2A0,
//! alive +0x58, step +0x44 called through `jalr`; spawned by 0x1E033B0)
//! drawn with the second library: turned quads (b0ec19f6c988 at
//! 0x1E01190, `quads4::quad_turned`), single quads (add5d81dd3fe at
//! 0x1E00ED0, `billboard::quad`), rings of quads (7369422df7f8 at
//! 0x1E00920), arcs (e6dce326551d at 0x1E01470, `billboard::arc_strips`)
//! and a full-window tint (561a6131a635 at 0x1E017D0, `quads4::tint4`),
//! UVs by uv_rect 0x1E00080, plus b546's unit flash (0x1E03540: the
//! attacker drawn into the target in a packet layer, then the flash task
//! 0x1E02970 stepped). Update 1: the flash and the stage dimmed to 0x20
//! (0x1E028B0); update 56 a grey tint keyed by a point 60 behind the
//! attacker and a quad rising at the attacker's node 0x14; 11..159 small
//! red rings at the attacker's nodes 0x14 / 0x1E; 91..239 two grey puffs
//! a update there drifting off, each bursting on its update 44 (from
//! update 159) into a red ring and an arc. Returns 1 on update 124, 4 on
//! 305, 2 on 285, 6 on 135..279, 8 once the 320-update timer +0x8650 has
//! run out.
//!
//! The step 0x1E01FD0 (an arc at a node) is never spawned [F: the spawn
//! `jal`s name 0x1E02970 / 0x1E028B0 / 0x1E02150 / 0x1E02250 / 0x1E01B90 /
//! 0x1E02590 / 0x1E023D0 / 0x1E02760]: not ported, nor its draw case.
use crate::b494::camera_angles_saving;
use crate::b590::project_residue;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 300;
const PUFF: u32 = 0x01E0_1B90;
const ARC_N: u32 = 0x01E0_1FD0;
const TINT: u32 = 0x01E0_2150;
const RISE: u32 = 0x01E0_2250;
const RED: u32 = 0x01E0_23D0;
const SMALL: u32 = 0x01E0_2590;
const ARC: u32 = 0x01E0_2760;
const LIGHT: u32 = 0x01E0_28B0;
/// The flash task, run by the unit flash 0x1E03540.
const FLASH: u32 = 0x01E0_2970;
/// The return address of the entry's `jalr` into a step (0x1E02F20 + 8).
const TASK_RA: u64 = 0x01E0_2F28;
const POS: u32 = D + 0x250;
const TEX: u32 = D + 0x8630;
const TIMER: u32 = D + 0x8650;
const COLOUR: u32 = D + 0x8654;
const LAYER: u32 = D + 0x866C;
const FLASH_DESC: u32 = D + 0x8670;
const TINT_W: u32 = D + 0x867C;
const NODES: u32 = D + 0x8690;
const SPAWN_K: u32 = D + 0x86BE;
const FACING: u32 = D + 0x86C2;
const COUNTER: u32 = D + 0x86C6;
const BYTES: u32 = D + 0x86C8;
const BYTES_B: u32 = D + 0x87C8;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02BB0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    let e = fx.camera_eye(0);
    fx.mem.write_u32(D + 0x8664, e);
    let a = fx.camera_at30(0);
    fx.mem.write_u32(D + 0x8668, a);
    fx.mem.write_u32(D + 0x865C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x8A40);
    l2::sine_table(fx, D + 0x88C8);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x86C4, param);
    fx.mem.set32(TIMER, 0x140);
    for (k, node) in [0x14, 0x1E, 1, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x86A0, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_2DD4);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2E04, 0x28);
        fx.mem.set8(BYTES + k, v - 0x14);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2E54, 0x28);
        fx.mem.set8(BYTES_B + k, v - 0x14);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(BYTES_B + k, 0);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s1: the flash tasks counted here (the steps' saved s1).
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        // The step's prologue: the entry's s0 = D, s1 = the flashes.
        let frame = match f {
            0 => continue,
            FLASH => {
                flashes += 1;
                continue;
            }
            PUFF | RISE | SMALL => 0x30,
            TINT | RED | ARC => 0x20,
            LIGHT => 0x10,
            _ => panic!("b259: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match frame {
            0x30 => fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(flashes)), (0, u64::from(D))]),
            0x20 => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]),
            _ => fx.saved(s, &[(0, TASK_RA)]),
        }
        match f {
            PUFF => puff(fx, t, slot, k),
            TINT => tint(fx, t),
            RISE => rise(fx, t),
            RED => red(fx, t),
            SMALL => small(fx, t),
            ARC => arc(fx, t),
            _ => light(fx, t),
        }
        fx.leave(frame);
    }
    // a3 as the draws' last library call left it, for the unit flash's
    // first 0x207BB0 (as b180) [run].
    let mut a3 = 0u32;
    let last_xyz2 = |fx: &mut Fx, before: usize| match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => 0,
    };
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == PUFF {
            tex(fx, TEX);
            cell(fx, t, sp + 0x80);
            fx.mem.set16(D + 0xBE, 0);
            quads4::quad_turned(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
            a3 = 0;
        }
        // ARC_N tasks are never spawned (the draw case at 0x1E03000: 0x24
        // strips through arc_strips).
        debug_assert_ne!(fx.mem.read_u32(t + 0x44), ARC_N);
        if fx.mem.read_u32(t + 0x44) == TINT {
            let h = fx.self_handle();
            fx.unit_position(h, sp + 0x70);
            // 0x1F71A0's frame below the entry's (its saved ra, s0 = D).
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_3060), (0, u64::from(D))]);
            let z = fx.mem.f(sp + 0x78);
            let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 60.0) } else { ee::sub(z, 60.0) };
            fx.mem.setf(sp + 0x78, z);
            let p = [fx.mem.f(sp + 0x70), fx.mem.f(sp + 0x74), fx.mem.f(sp + 0x78)];
            let mut x = fx.mem.read_u64(sp + 0xE8);
            if let Some(w) = fx.project(p, None, &mut x) {
                fx.mem.write_u64(sp + 0xE8, x);
                fx.mem.setf(TINT_W, w);
            }
            project_residue(fx, sp, sp + 0x70);
            let w = fx.mem.f(TINT_W);
            quads4::tint4(fx, t, 0x21, w);
            if let Some(p) = fx.out.prims.last() {
                let d = p.desc;
                a3 = u32::from(fx.mem.read_u16(d + 2));
            }
        }
        if fx.mem.read_u32(t + 0x44) == RISE {
            tex(fx, TEX);
            cell(fx, t, sp + 0x90);
            fx.mem.set16(D + 0xBE, 0);
            billboard::quad(fx, uv::uv_rect, D, t, 0x21, t + 0x10);
            a3 = 0;
        }
        for (f, local, rows) in [(RED, 0xA0u32, 4), (SMALL, 0xB0, 5)] {
            if fx.mem.read_u32(t + 0x44) == f {
                tex(fx, TEX + 4);
                cell(fx, t, sp + local);
                fx.mem.set16(D + 0xB8, rows);
                fx.mem.set16(D + 0xB0, 0);
                billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
                a3 = 0;
                fx.mem.set16(D + 0xB8, 0);
                fx.mem.set16(D + 0xB0, 0);
            }
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x12);
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            a3 = last_xyz2(fx, before);
            fx.mem.set16(D + 0xB8, 0);
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
        0x7C => 1,
        0x131 => 4,
        0x11D => 2,
        0x87..=0x117 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// The atlas cell 0x14 frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E033B0(D, step, 0): the first free task cleared and set off at
/// D+0x250 (i16 +0, float +0x10) with the colour D+0x8654 (twice), kind
/// D+0x86BE (+0x50); `saves` the caller's s0..s4, `ra` the return address.
fn spawn(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        let t = D + 0x2A0 + 0x70 * k;
        fx.bzero(t, 0x70);
        fx.mem.set16(t + 0x58, 1);
        for o in [0x60u32, 0x64] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + o + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        for k in 0..3 {
            let w = fx.mem.read_u32(POS + 4 * k);
            fx.mem.write_u32(t + 0x10 + 4 * k, w);
        }
        let s = fx.mem.i16(SPAWN_K);
        fx.mem.set16(t + 0x50, s);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

fn node_pos(fx: &mut Fx, k: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES + 2 * k);
    fx.node_position(h, slot, POS);
}

/// 0x1E019A0(D, 0): the update counter and the emitters (frame 0x20: ra,
/// the entry's s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_2EE8), (0, u64::from(D))]);
    // The spawns' saves: s0 = D, the entry's s1 = s2 = 0, s3 = the
    // parameter, s4 the runner's (0 [H]).
    let saves = [u64::from(D), 0, 0, param as i64 as u64, 0];
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 1 {
        spawn(fx, FLASH, saves, 0x01E0_19F0);
        fx.mem.set16(SPAWN_K, 0);
        fx.mem.set16(SPAWN_K, 1);
        spawn(fx, LIGHT, saves, 0x01E0_1A20);
    }
    if c(fx) == 0x38 {
        spawn(fx, TINT, saves, 0x01E0_1A4C);
        spawn(fx, RISE, saves, 0x01E0_1A60);
    }
    if (0x5B..0xF0).contains(&c(fx)) {
        node_pos(fx, 0);
        spawn(fx, PUFF, saves, 0x01E0_1AB8);
        node_pos(fx, 1);
        spawn(fx, PUFF, saves, 0x01E0_1AEC);
    }
    if (0xB..0xA0).contains(&c(fx)) {
        node_pos(fx, 0);
        spawn(fx, SMALL, saves, 0x01E0_1B44);
        node_pos(fx, 1);
        spawn(fx, SMALL, saves, 0x01E0_1B78);
    }
    fx.leave(0x20);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn odd(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54).wrapping_rem(2) == 1
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends
/// (`end`) or the byte is cleared.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8, end: bool) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else if end {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.write_u8(t + at, 0);
    }
}

/// rand % 0x800 - 0x400 (C remainder).
fn spread(fx: &mut Fx, t: u32, ra: u32) -> i32 {
    let v = fx.rand_from(t, ra);
    v % 0x800 - 0x400
}

/// 0x1E01B90 (D, task): a grey puff (size 10 growing to 25, cell (rand >>
/// 4) % 3) thrown off at (+0x30, +0x34) = (-50..49) / 400 a update (y also
/// down by 0.4), z by +0x38 (2.2 growing by 0.1 to 4.5) away from the
/// facing, angles random; in by 0x40 under 0x80 for 50 updates, out by 9;
/// on its update 44 it ends, from effect update 159 bursting into a red
/// ring and an arc; grey 0x80 / 0x70 by its age's parity; its age stays
/// 90 from effect update 301.
fn puff(fx: &mut Fx, t: u32, slot: u32, k: u32) {
    let f = PUFF;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xA);
        fx.mem.set16(t + 0xA, 0xA);
        let v = fx.rand_from(t, f + 0x34);
        let r = fx.div(v >> 4, 3).1;
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x56, r);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, f + 0x68, 0x64);
        fx.mem.setf(t + 0x34, ee::from_i32(v - 0x32));
        let v = rmod(fx, t, f + 0x94, 0x64);
        fx.mem.setf(t + 0x30, ee::from_i32(v - 0x32));
        let v = ee::div(fx.mem.f(t + 0x34), 400.0);
        fx.mem.setf(t + 0x34, v);
        let v = ee::div(fx.mem.f(t + 0x30), 400.0);
        fx.mem.setf(t + 0x30, v);
        fx.mem.write_u32(t + 0x38, 0x400C_CCCD);
        let v = fx.rand_from(t, f + 0xEC);
        let r = fx.div(v >> 4, 0x32).1;
        let y = ee::add(fx.mem.f(t + 0x14), ee::div(ee::from_i32(r - 0x19), 20.0));
        fx.mem.setf(t + 0x14, y);
        let v = rmod(fx, t, f + 0x12C, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), f + 0x148);
        for (o, ra) in [(4u32, 0x150u32), (2, 0x170), (0, 0x190)] {
            let v = spread(fx, t, f + ra);
            fx.mem.set16(t + o, v);
        }
        let v = rmod(fx, t, f + 0x1B0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, f + 0x1CC);
        fx.mem.set32(t + 0x20, v % 0x4000 - 0x2000);
        fx.mem.set32(t + 0x24, 0x4000);
    }
    if fx.mem.i16(t + 0x54) < 0x32 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let by = fx.mem.f(t + 0x38);
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, by) } else { ee::add(z, by) };
        fx.mem.setf(t + 0x18, z);
        let s = fx.mem.f(t + 0x38);
        if s < 4.5 {
            fx.mem.setf(t + 0x38, ee::add(s, f32::from_bits(0x3DCC_CCCD)));
        }
        let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
        fx.mem.setf(t + 0x10, x);
        let dy = ee::sub(fx.mem.f(t + 0x34), f32::from_bits(0x3ECC_CCCD));
        let y = ee::add(fx.mem.f(t + 0x14), dy);
        fx.mem.setf(t + 0x14, y);
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s < 0x19 {
                fx.mem.set16(t + o, s + 1);
            }
        }
    }
    if fx.mem.i16(t + 0x54) == 0x2C {
        for o in 0..4u32 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * o);
            fx.mem.write_u32(POS + 4 * o, w);
        }
        if fx.mem.i16(COUNTER) >= 0x9F {
            // The spawns' saves: the step's s0 = the task, s1 = D, the
            // entry's s2 = 0, s3 = the slot, s4 = its index.
            let saves = [u64::from(t), u64::from(D), 0, u64::from(slot), u64::from(k)];
            spawn(fx, RED, saves, 0x01E0_1F08);
            spawn(fx, ARC, saves, 0x01E0_1F1C);
        }
        fx.mem.set16(t + 0x58, 0);
    }
    let g = if odd(fx, t) { 0x80 } else { 0x70 };
    l2::rgb(fx, t + 0x60, g, g, g);
    age(fx, t);
    if fx.mem.i16(COUNTER) >= 0x12D {
        fx.mem.set16(t + 0x54, 0x5A);
    }
    let a = fx.mem.i16(t + 0x5A) + 0x28;
    fx.mem.set16(t + 0x5A, a);
}

/// 0x1E02150 (D, task): the tint's colours (grey 0x80 both), life 10;
/// both alphas in by 0x2A under 0x80, then out by 0xC (the task ends when
/// the first is below 0xE, the second is cleared).
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 6, 0xA);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0x80, 0x2A);
        fade_in(fx, t, 0x67, 0x80, 0x2A);
    } else {
        fade_out(fx, t, 0x63, 0xE, 0xC, true);
        fade_out(fx, t, 0x67, 0xE, 0xC, false);
    }
    age(fx, t);
}

/// 0x1E02250 (D, task): a quad at the attacker's node 0x14 rising by +0x34
/// (3, slowing by 0.1 to 0) a update, size 28 shrinking by 1 after its
/// update 8, facing +0x24 0x8000 / 0; in by 0x2A under 0x80 for 8 updates,
/// out by 9.
fn rise(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x1C);
        fx.mem.set16(t + 0xA, 0x1C);
        fx.mem.write_u32(t + 0x34, 0x4040_0000);
        fx.mem.set16(t + 0x56, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        let a = if fx.mem.i16(FACING) != 0 { 0x8000 } else { 0 };
        fx.mem.set32(t + 0x24, a);
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fade_in(fx, t, 0x63, 0x80, 0x2A);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s > 0 {
                fx.mem.set16(t + o, s - 1);
            }
        }
        let v = fx.mem.f(t + 0x34);
        if v > 0.0 {
            fx.mem.setf(t + 0x34, ee::sub(v, f32::from_bits(0x3DCC_CCCD)));
        }
    }
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    crate::b076::node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_2394);
    let y = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x14, y);
    age(fx, t);
}

/// 0x1E023D0 (D, task): a red ring (size 8, cell 4, radius 10..29 growing
/// by 4..10, row 1..8, life 20..22): 0xC0, then out by 0xB; white / red by
/// its age's parity.
fn red(fx: &mut Fx, t: u32) {
    let f = RED;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4100_0000);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0x60, 0x60);
        let v = rmod(fx, t, f + 0x48, 0x14);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, f + 0x68, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0x84, 7);
        fx.mem.set16(t + 0xE, v + 4);
        let v = fx.rand_from(t, f + 0xA4);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, f + 0xC4, 3);
        fx.mem.set16(t + 6, v + 0x14);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade_out(fx, t, 0x63, 0xD, 0xB, true);
    }
    if odd(fx, t) {
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    } else {
        l2::rgb(fx, t + 0x60, 0x80, 0x60, 0x60);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02590 (D, task): a small red ring at a node (size 2, cell 3, radius
/// 4..11 growing by 1, row 1..8, life 5..7): 0xC0, then out by 0xB; pale
/// blue / dark by its age's parity; ends from effect update 241.
fn small(fx: &mut Fx, t: u32) {
    let f = SMALL;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4000_0000);
        fx.mem.set16(t + 0x56, 3);
        l2::rgb(fx, t + 0x60, 0xFF, 0x60, 0x60);
        let v = fx.rand_from(t, f + 0x50);
        fx.mem.set16(t + 0x50, v % 8 + 4);
        let v = rmod(fx, t, f + 0x70, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0xE, 1);
        let v = fx.rand_from(t, f + 0x94);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, f + 0xB4, 3);
        fx.mem.set16(t + 6, v + 5);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade_out(fx, t, 0x63, 0xD, 0xB, true);
    }
    if odd(fx, t) {
        l2::rgb(fx, t + 0x60, 0xCF, 0xCF, 0xFF);
    } else {
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x80);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
    if fx.mem.i16(COUNTER) >= 0xF1 {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E02760 (D, task): an arc (outer radius 80, inner grey 0xC0, outer
/// red, angles 20 / 20) facing the camera turned 0x4000; in by 0x50 under
/// 0xA0 for 4 updates, out by 0x13; yellow / grey by its age's parity.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x64, 0xC0, 0x40, 0x40);
        fx.mem.set16(t + 6, 4);
        fx.mem.write_u32(t + 0x4C, 0x42A0_0000);
        fx.mem.set16(t + 0x5A, 0x14);
        fx.mem.set16(t + 0x5C, 0x14);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x67, 0xA0, 0x50);
    } else {
        fade_out(fx, t, 0x67, 0x15, 0x13, true);
    }
    if odd(fx, t) {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0x80);
    } else {
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x60);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2880);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E028B0 (D, task): the stage light (0x1F7F10) down by 10 to 0x20
/// while the timer is at 21 or more, then back up by 10 to 0x80 (the task
/// ending there).
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let l = fx.mem.i16(t + 0x50);
    if fx.mem.i32(TIMER) >= 0x15 {
        fx.mem.set16(t + 0x50, if l < 0x21 { 0x20 } else { l - 0xA });
    } else {
        if l < 0x80 {
            fx.mem.set16(t + 0x50, l + 0xA);
        } else {
            fx.mem.set16(t + 0x50, 0x80);
            fx.mem.set16(t + 0x58, 0);
        }
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    let l = fx.mem.read_u8(t + 0x50);
    fx.set_stage_light(i32::from(l));
}

/// 0x1E02970(D, t): the flash's frame sprite (the target over the whole
/// window, grey 0x80) into the descriptor at +0x8670, no ordering table;
/// alpha up by 0x12 below 0x70 for 250 updates, then down by 9. `regs` the
/// unit flash's s0 / s1 (the slot index, the flashes so far).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3650), (0x10, u64::from(regs[1])), (0, u64::from(regs[0]))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
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
    for (o, v) in [(0x18u32, 0u32), (0x1C, 0), (0x28, 0x3F80_0000), (0x2C, 0x3F80_0000)] {
        fx.mem.write_u32(d + o, v);
    }
    fx.submit(d, 0, 0);
    if fx.mem.i16(t + 0x54) < 0xFA {
        fade_in(fx, t, 0x63, 0x70, 0x12);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03540(D): in a packet layer (0x2078A0) the attacker drawn into the
/// target (0x1F8270 filter 0; ALPHA 0x48), TEXFLUSH before and after;
/// every flash task stepped (0x1E02970); the layer linked into the
/// ordering table at key 0 when any ran (b546's).
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
    let mut ran = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) != 0 && fx.mem.read_u32(t + 0x44) == FLASH {
            ran += 1;
            flash(fx, t, [k, ran]);
        }
    }
    if ran != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}
