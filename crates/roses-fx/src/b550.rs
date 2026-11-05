//! BMODULE b550 (card 79), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E041B0, data area 0x1ED0000) [F] (helper A23, 3
//! Nov 2026): b731's program (card 51) with its globals 0xA8 higher, a
//! unit flash and a new emitter, on the same pool of 200 tasks (0x70 bytes
//! at +0x2A0, alive +0x58, step +0x44 through `jalr`, the flash steps
//! 0x1E03ED0 skipped and counted; spawned by 0x1E04D10 with the colour
//! +0xBA14 and the slot +0xBD42 into +0x50). b731's step bodies are shared
//! (motes, spark rings, the opening swirl) at this record's addresses. The
//! emitter 0x1E01C10 counts +0xBD4A: on update 1 the stage light
//! (0x1E03C60, down to 0x40 while the timer is at least 31), a frame tint
//! (0x1E03D30) and six points round the attacker's node 1 (+0xB950 +
//! 0x10 k, pushed 5 / 12 apart); on 0x1E..0x26 a whole-window unit flash
//! each (0x1E03ED0, b611's, through the unit drawn into the target in a
//! packet layer, 0x1E05CF0); on 0x23 six swaying violet arcs at the six
//! points (0x1E03590, drawn twice); on 0x66 six slot-fanned orbs
//! (0x1E02130) with their tubes (0x1E03090) from the points; on 0x87 seven
//! spark rings at node 1 (0x1E03A90). The steps 0x1E02BF0 / 0x1E033D0 /
//! 0x1E03770 (b731's closing motes, spark ring and swirl) are drawn but
//! never spawned (not ported). Returns 1 on update 0x92, 4 on 0xBE, 2 on
//! 0xAF, 6 on 0x9D..0xA9, 8 once the 200-update timer +0xBA10 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::b731::{fade, flicker, mote, sparks, sparks_c, swirl, SparksCfg, MOTE_A, MOTE_WIDE};
use crate::blib::{billboard, quads4, spawn::history, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
/// The step functions a task holds (+0x44), as the record's addresses.
const ORB: u32 = 0x01E0_2130;
const MOTE: u32 = 0x01E0_2860;
const SPARKS: u32 = 0x01E0_29F0;
const MOTE_W: u32 = 0x01E0_2DA0;
const ARC: u32 = 0x01E0_2F50;
const TUBE: u32 = 0x01E0_3090;
const ARC_B: u32 = 0x01E0_3590;
const SWIRL_OUT: u32 = 0x01E0_3910;
const SPARKS_C: u32 = 0x01E0_3A90;
const LIGHT: u32 = 0x01E0_3C60;
const FRAME: u32 = 0x01E0_3D30;
const FLASH: u32 = 0x01E0_3ED0;
/// The return address of the entry's `jalr` into a step (0x1E044C8 + 8).
const TASK_RA: u64 = 0x01E0_44D0;
const SINES: u32 = D + 0xBF4C;
const CENTRES: u32 = D + 0xB1D0;
const POINTS: u32 = D + 0xB950;
const TEX: u32 = D + 0xB9F0;
const TIMER: u32 = D + 0xBA10;
const COLOUR: u32 = D + 0xBA14;
const LAYER: u32 = D + 0xBA24;
const FLASH_DESC: u32 = D + 0xBA28;
const RADII: u32 = D + 0xBA34;
const NODE: u32 = D + 0xBC24;
const ANGLES: u32 = D + 0xBC44;
const SLOT: u32 = D + 0xBD42;
const NEXT: u32 = D + 0xBD44;
const YAW: u32 = D + 0xBD46;
const COUNTER: u32 = D + 0xBD4A;
const BYTES: u32 = D + 0xBD4C;
const BYTES_B: u32 = D + 0xBE4C;

/// 0x1E029F0: b731's 0x1E02800 in violet.
const SPARKS_VIOLET: SparksCfg = SparksCfg { size: (0x1E, 0x19, 0), colour: (0x80, 0x40, 0xFF), cell: true, radius: (0x15E, 0x12C) };

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// rand (the caller's s0 = `s0`) from `ra`, mod `m` (truncated).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn copy3(fx: &mut Fx, from: u32, to: u32) {
    for o in 0..3 {
        let w = fx.mem.read_u32(from + 4 * o);
        fx.mem.write_u32(to + 4 * o, w);
    }
}

/// The entry 0x1E041B0 (a0 = D, a1 = param; s0 = D, s1 = the flash
/// count, s2 = 0, s3 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x110);
    fx.mem.write_u32(D + 0xBA1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x110);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xC0C0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xBD48, param);
    fx.mem.set32(TIMER, 0xC8);
    for (at, n, clear) in [(NODE, 1, 1), (D + 0xBC26, 0x1D, 0), (D + 0xBC28, 0x34, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(at, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xBC32, r);
    for k in 0xAu32..0x10 {
        let tex = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x260 + 4 * k, tex);
    }
    let tex = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, tex);
    for k in 0..3u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x100);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0x104)), 180.0);
    fx.fix_residue(v, 0x01E0_435C);
    fx.mem.set16(YAW, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_438C, 0x1E);
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_43E4);
        fx.mem.write_u8(BYTES + k, v.wrapping_rem(4) as u8);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_443C, 0x28);
        fx.mem.write_u8(BYTES_B + k, (v - 0x14) as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // s1 counts the flash tasks (skipped here, run by the unit flash).
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
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
        // The steps' prologues: the entry's s0 = D, s1 = the flashes, s2
        // = 0, s3 = the slot, s4 = k.
        let frame = match f {
            ORB => 0x40,
            TUBE => 0x60,
            ARC_B => 0x30,
            FRAME => 0x80,
            LIGHT => 0x10,
            MOTE | SPARKS | MOTE_W | ARC | SWIRL_OUT | SPARKS_C => 0x20,
            _ => panic!("b550: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match frame {
            0x40 => fx.saved(s, &[(0x30, TASK_RA), (0x20, 0), (0x10, u64::from(flashes)), (0, u64::from(D))]),
            0x60 => fx.saved(s, &[(0x40, TASK_RA), (0x30, u64::from(slot)), (0x20, 0), (0x10, u64::from(flashes)), (0, u64::from(D))]),
            0x30 => fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(flashes)), (0, u64::from(D))]),
            0x10 => fx.saved(s, &[(0, TASK_RA)]),
            _ => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]),
        }
        match f {
            ORB => orb(fx, t, slot, k),
            MOTE => mote(fx, t, MOTE, &MOTE_A),
            MOTE_W => mote(fx, t, MOTE_W, &MOTE_WIDE),
            SPARKS => sparks(fx, t, SPARKS, &SPARKS_VIOLET),
            SPARKS_C => sparks_c(fx, t, SPARKS_C),
            ARC => arc(fx, t, flashes),
            TUBE => tube_step(fx, s, t),
            ARC_B => arc_b(fx, t),
            SWIRL_OUT => swirl(fx, t, SWIRL_OUT, true),
            LIGHT => light(fx, t),
            _ => frame_quad(fx, s, t),
        }
        fx.leave(frame);
    }
    // a3 as the draws' last library call left it for the unit flash's
    // first TEXFLUSH: 0 after a quad or a ring, the last XYZ2 drawn after
    // a far-Z arc (b362 / b432's lesson) [run].
    let mut a3 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        draw(fx, sp, t, &mut a3);
    }
    if flashes != 0 {
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x92 => 1,
        0xBE => 4,
        0xAF => 2,
        0x9D..=0xA9 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The draw's head: texture `tex` into D+0x88, the atlas cell `id` frame
/// +0x56 (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, tex: u32, id: i32, r: u32) {
    let w = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, w);
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// An orb's point (the slot +0x50's centre plus its radius turned by its
/// angle in x / y) into `out`.
fn orb_point(fx: &mut Fx, t: u32, out: u32) {
    for (o, cos) in [(0u32, 1), (4, 0)] {
        let i = fx.mem.i16(t + 0x50) as u32;
        let a = fx.mem.i16(ANGLES + 2 * i);
        let s = l2::sin_deg(fx, SINES, a, cos);
        let v = ee::add(fx.mem.f(CENTRES + 0x10 * i + o), ee::mul(fx.mem.f(RADII + 4 * i), s));
        fx.mem.setf(out + o, v);
    }
    let i = fx.mem.i16(t + 0x50) as u32;
    let z = fx.mem.f(CENTRES + 0x10 * i + 8);
    fx.mem.setf(out + 8, z);
}

/// The entry's draw of one live task by its step (s0 = D); `a3` as its
/// last library call leaves it.
fn draw(fx: &mut Fx, sp: u32, t: u32, a3: &mut u32) {
    let f = fx.mem.read_u32(t + 0x44);
    let motes = |fx: &mut Fx, tex: u32, r: u32, mode: i32| {
        cell(fx, t, tex, 0x14, r);
        fx.mem.set16(D + 0xB8, 6);
        let v = fx.mem.i16(t + 0xA);
        fx.mem.set16(D + 0xC0, v);
        copy3(fx, t + 0x10, D + 0x20);
        quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, mode, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    };
    if f == ORB {
        cell(fx, t, TEX, 0x14, sp + 0x80);
        fx.mem.set16(D + 0xBE, 0);
        orb_point(fx, t, sp + 0x70);
        billboard::quad_b_twice(fx, uv::uv_rect, D, t, 0x20, sp + 0x70);
        *a3 = 0;
    }
    if f == MOTE {
        motes(fx, TEX, sp + 0x90, 0x20);
        *a3 = 0;
    }
    if f == SPARKS {
        cell(fx, t, D + 0x29C, 0x9B, sp + 0xA0);
        fx.mem.set16(D + 0xB8, 0xA);
        fx.mem.set16(D + 0xB0, 1);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
        *a3 = 0;
        fx.mem.set16(D + 0xB8, 0);
        fx.mem.set16(D + 0xB0, 0);
    }
    if f == ARC {
        fx.mem.set16(D + 0xB8, 0x18);
        let before = fx.out.prims.len();
        billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
        *a3 = last_xyz2(fx, before);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == MOTE_W {
        motes(fx, TEX + 4, sp + 0xC0, 0x10);
        *a3 = 0;
    }
    if f == TUBE {
        cell(fx, t, TEX, 0x14, sp + 0xD0);
        let v = fx.mem.i16(D + 0x96) + 0xF;
        fx.mem.set16(D + 0x96, v);
        fx.mem.set16(D + 0x98, 0x1F);
        fx.mem.set16(D + 0x9A, 1);
        let h = fx.self_handle();
        unit_position_saving(fx, h, sp + 0x70, u64::from(D), 0x01E0_49A0);
        for o in [0x20u32, 0x24, 0x28] {
            fx.mem.write_u32(D + o, 0);
        }
        fx.mem.set16(D + 0xBE, 2);
        let i = fx.mem.i16(t + 0x50) as u32;
        tubetrail::tube(fx, D, t, D + 0x5A50 + 0x460 * i, 0);
        // The tube's mean-w quads leave 0 [H].
        *a3 = 0;
    }
    if f == ARC_B {
        fx.mem.set16(D + 0xB8, 0x18);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE);
        node_position_saving(fx, h, s, sp + 0x70, u64::from(D), None, 0x01E0_4AB0);
        for _ in 0..2 {
            let i = fx.mem.i16(t + 0x50) as u32;
            let before = fx.out.prims.len();
            billboard::arc_strips_flat(fx, D, t, 0x21, POINTS + 0x10 * i);
            *a3 = last_xyz2(fx, before);
        }
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == SPARKS_C {
        cell(fx, t, TEX, 0x14, sp + 0xF0);
        fx.mem.set16(D + 0xB8, 5);
        fx.mem.set16(D + 0xB0, 0);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
        *a3 = 0;
        fx.mem.set16(D + 0xB8, 0);
        fx.mem.set16(D + 0xB0, 0);
    }
}

/// The last XYZ2 of the last primitive drawn since `before` (0 when none),
/// as b491's.
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

/// 0x1E04D10(D, step, 0): the first free of the 200 tasks cleared and set
/// off at D+0x250 (as i16 and its xyz at +0x10) with the colour +0xBA14
/// (+0x60, +0x64), +0x50 = +0xBD42; 1 when one was free. `r` the caller's
/// s0..s4.
fn spawn(fx: &mut Fx, step: u32, r: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, r[4]), (0x30, r[3]), (0x20, r[2]), (0x10, r[1]), (0, r[0])]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for o in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            copy3(fx, D + 0x250, t + 0x10);
            let s = fx.mem.i16(SLOT);
            fx.mem.set16(t + 0x50, s);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// +0xBD44 up by one, back to 0 at 120.
fn next_slot(fx: &mut Fx) {
    let n = fx.mem.i16(NEXT) + 1;
    fx.mem.set16(NEXT, n);
    if fx.mem.i16(NEXT) >= 0x78 {
        fx.mem.set16(NEXT, 0);
    }
}

/// 0x1E01C10(D, 0) (frame 0x40: ra, the entry's s2 = 0, s1 = 0, s0 = D):
/// the update counter +0xBD4A and the emitters.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_4490), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
    let p = param as i64 as u64;
    // s0 / s1 as the loops leave them; s2 = D, s3 = param, s4 the
    // runner's (0 [H]).
    let (mut s0, mut s1) = (u64::from(D), 0u64);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 1 {
        for k in 0..6u32 {
            s0 = u64::from(k);
            s1 = u64::from(D + 0x10 * k);
            let h = fx.self_handle();
            let s = fx.mem.i16(NODE);
            node_position_saving(fx, h, s, POINTS + 0x10 * k, s0, Some(s1), 0x01E0_1C8C);
        }
        s0 = 6;
        s1 = u64::from(D + 0x60);
        for (o, d) in [(0u32, -5.0f32), (4, 12.0), (0x10, -12.0), (0x20, -5.0), (0x24, -12.0), (0x30, 5.0), (0x34, 12.0), (0x40, 12.0), (0x50, 5.0), (0x54, -12.0)] {
            let v = fx.mem.f(POINTS + o);
            let v = if d < 0.0 { ee::sub(v, -d) } else { ee::add(v, d) };
            fx.mem.setf(POINTS + o, v);
        }
    }
    if fx.mem.i16(COUNTER) == 0x66 {
        for k in 0..6u32 {
            s0 = u64::from(D + 0x10 * k);
            s1 = u64::from(k);
            for o in 0..4 {
                let w = fx.mem.read_u32(POINTS + 0x10 * k + 4 * o);
                fx.mem.write_u32(D + 0x250 + 4 * o, w);
            }
            let v = fx.mem.i16(NEXT);
            fx.mem.set16(SLOT, v);
            let r = [s0, s1, u64::from(D), p, 0];
            spawn(fx, ORB, r, 0x01E0_1E58);
            spawn(fx, TUBE, r, 0x01E0_1E6C);
            next_slot(fx);
        }
        s0 = u64::from(D + 0x60);
        s1 = 6;
    }
    let r = [s0, s1, u64::from(D), p, 0];
    if fx.mem.i16(COUNTER) == 0x87 {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE);
        node_position_saving(fx, h, s, D + 0x250, s0, Some(s1), 0x01E0_1F10);
        let mut ra = 0x01E0_1F24u32;
        for _ in 0..7 {
            spawn(fx, SPARKS_C, r, ra);
            ra += 0x14;
        }
    }
    if fx.mem.i16(COUNTER) == 0x23 {
        let mut ra = 0x01E0_1FD4u32;
        for k in 0..6 {
            fx.mem.set16(SLOT, k);
            spawn(fx, ARC_B, r, ra);
            ra += 0x24;
        }
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, LIGHT, r, 0x01E0_20B4);
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, FRAME, r, 0x01E0_20E0);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x1E..0x27).contains(&c) {
        spawn(fx, FLASH, r, 0x01E0_2118);
    }
    fx.leave(0x40);
}

/// 0x1E02130 (frame 0x40: s1 = D, s2 = the slot index): b731's orb with
/// its radius scale +0x34 ((rand >> 4) % 35 + 13), in for 54 updates,
/// +0x5A up by 10 to 200 and its lift 0.35 a update.
fn orb(fx: &mut Fx, t: u32, slot: u32, k: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xA);
        fx.mem.set16(t + 0xA, 0xA);
        let v = fx.rand_from(t, 0x01E0_2168);
        let v = fx.div(v >> 4, 0x23).1;
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0xD));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        for o in 0..3 {
            let i = fx.mem.i16(t + 0x50) as u32;
            let w = fx.mem.read_u32(t + 0x10 + 4 * o);
            fx.mem.write_u32(CENTRES + 0x10 * i + 4 * o, w);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (n, ra, b) in [(0, 0x01E0_2230u32, -0x48), (1, 0x01E0_2270, 0), (2, 0x01E0_22B0, 0x48), (3, 0x01E0_22F0, 0x90), (4, 0x01E0_2330, 0xD8)] {
            if fx.mem.i16(t + 0x50) == n {
                let v = rmod(fx, t, ra, 0x90) + b;
                let i = fx.mem.i16(t + 0x50) as u32;
                fx.mem.set16(ANGLES + 2 * i, v);
            }
        }
        let i = fx.mem.i16(t + 0x50) as u32;
        fx.mem.write_u32(RADII + 4 * i, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x36 {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
        }
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0x10, 0xE));
        let a = fx.mem.read_u8(t + 0x67);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x67, a - 0xE);
        }
    }
    flicker(fx, t);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_244C);
    if fx.mem.i16(t + 0x54) == 0x35 {
        orb_point(fx, t, D + 0x250);
        let i = u64::from(fx.mem.i16(t + 0x50) as u32);
        let r = [u64::from(t), u64::from(D), i, u64::from(slot), u64::from(k)];
        let mut ra = 0x01E0_2544u32;
        for f in [MOTE, MOTE, MOTE, SPARKS, SPARKS, ARC, MOTE_W, MOTE_W, MOTE_W, SWIRL_OUT] {
            spawn(fx, f, r, ra);
            ra += 0x14;
        }
    }
    age(fx, t);
    let a = fx.mem.i16(t + 0x54);
    if (0x22..0x35).contains(&a) {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s < 0x32 {
                fx.mem.set16(t + o, s + 5);
            }
        }
        let s = fx.mem.i16(t + 0x5A);
        if s < 0xC8 {
            fx.mem.set16(t + 0x5A, s + 0xA);
        }
        let i = fx.mem.i16(t + 0x50) as u32;
        let c = CENTRES + 0x10 * i;
        let z = fx.mem.f(c + 8);
        let z = if fx.mem.i16(YAW) != 0 { ee::sub(z, 8.5) } else { ee::add(z, 8.5) };
        fx.mem.setf(c + 8, z);
        let y = ee::sub(fx.mem.f(c + 4), f32::from_bits(0x3EB3_3333));
        fx.mem.setf(c + 4, y);
        orb_point(fx, t, D + 0x250);
        if fx.mem.i16(t + 0x54) < 0x33 {
            let i = u64::from(fx.mem.i16(t + 0x50) as u32);
            spawn(fx, SPARKS_C, [u64::from(t), u64::from(D), i, u64::from(slot), u64::from(k)], 0x01E0_280C);
        }
    }
    let a = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let r = ee::mul(fx.mem.f(t + 0x34), s);
    let i = fx.mem.i16(t + 0x50) as u32;
    fx.mem.setf(RADII + 4 * i, r);
}

/// 0x1E02F50: b731's arc with its outer flicker violet ((0x80, 0x40,
/// 0xFF) / (0x70, 0x30, 0xCF)).
fn arc(fx: &mut Fx, t: u32, flashes: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4396_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 2;
    fade(fx, t, 0x63, rising, (0x80, 0x40), (0xD, 0xB));
    violet(fx, t);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_3064);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// The outer colour flickering violet: (0x80, 0x40, 0xFF) on the odd
/// ages, `even` on the even.
fn violet_by(fx: &mut Fx, t: u32, even: (i32, i32, i32)) {
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x64, 0x80, 0x40, 0xFF);
    } else {
        l2::rgb(fx, t + 0x64, even.0, even.1, even.2);
    }
}

fn violet(fx: &mut Fx, t: u32) {
    violet_by(fx, t, (0x70, 0x30, 0xCF));
}

/// 0x1E03590 (frame 0x30: s1 = D): a pale arc (0xDF) at its slot's point
/// (+0xB950 + 0x10 slot, drawn twice), its inner radius 55 + 15
/// sin(+0x52) (+0x52 from rand % 360 by 8 a update), the outer violet; in
/// by 12 below 0x80 for 150 updates, then out by 11; facing the camera,
/// spun by 0x4000 a update.
fn arc_b(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x425C_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xDF, 0xDF, 0xDF);
        l2::rgb(fx, t + 0x64, 0x80, 0x40, 0xFF);
        let v = rmod(fx, t, 0x01E0_35F0, 0x168);
        fx.mem.set16(t + 0x52, v);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x96;
    fade(fx, t, 0x63, rising, (0x80, 0xC), (0xD, 0xB));
    violet_by(fx, t, (0x70, 0x20, 0xCF));
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_36CC);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    let a = fx.mem.i16(t + 0x52);
    let s = l2::sin_deg(fx, SINES, a, 0);
    fx.mem.setf(t + 0x48, ee::add(55.0, ee::mul(15.0, s)));
    let a = fx.mem.i16(t + 0x52) + 8;
    fx.mem.set16(t + 0x52, a);
    let a = fx.mem.i16(t + 0x52);
    if a >= 0x168 {
        let (_, m) = fx.div(a, 0x168);
        fx.mem.set16(t + 0x52, m);
    }
    age(fx, t);
}

/// 0x1E03090: b731's tube in for 54 updates, up to 40 points.
fn tube_step(fx: &mut Fx, sp: u32, t: u32) {
    let p = sp + 0x50;
    orb_point(fx, t, p);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        let i = fx.mem.i16(t + 0x50) as u32;
        let trail = D + 0x5A50 + 0x460 * i;
        for j in 0..0x46u32 {
            copy3(fx, p, trail + 0x10 * j);
        }
        fx.mem.set16(t + 6, 0x14);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0x56, 4);
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x36 {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
        }
    } else {
        let s = fx.mem.f(t + 0x48);
        if crate::exe::ee_lt(s, 0.0) {
            fx.mem.set16(t + 0x58, 0);
        } else {
            fx.mem.setf(t + 0x48, ee::sub(s, 1.5));
        }
    }
    if fx.mem.i16(t + 0x54) < 0x53 {
        let v = fx.mem.i16(t + 0x5A) + 5;
        fx.mem.set16(t + 0x5A, v);
    }
    if fx.mem.i16(t + 0x54) < 0x75 {
        let i = fx.mem.i16(t + 0x50) as u32;
        history(fx, D + 0x5EA0 + 0x460 * i, 0x46, p);
    }
    if fx.mem.i16(t + 0x54) < 0x50 {
        flicker(fx, t);
    }
    if fx.mem.i16(t + 0x54) != 0 && fx.mem.i16(t + 6) < 0x28 {
        let v = fx.mem.i16(t + 6) + 1;
        fx.mem.set16(t + 6, v);
    }
    age(fx, t);
}

/// 0x1E03C60 (frame 0x10): the stage light +0x50 from 0x80 down by 10 to
/// 0x40 while the timer is at least 31, then back up by 10 to 0x80 (the
/// task ending there).
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let l = fx.mem.i16(t + 0x50);
    if fx.mem.i32(TIMER) >= 0x1F {
        fx.mem.set16(t + 0x50, if l < 0x41 { 0x40 } else { l - 0xA });
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
    age(fx, t);
    let l = fx.mem.read_u8(t + 0x50);
    fx.set_stage_light(i32::from(l));
}

/// 0x1E03D30 (frame 0x80, the descriptor at sp + 0x28): b731's frame tint
/// in (0x70, 0x60, 0x80), in for 175 updates.
fn frame_quad(fx: &mut Fx, sp: u32, t: u32) {
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
    for (k, v) in [0x70u8, 0x60, 0x80, a].into_iter().enumerate() {
        fx.mem.write_u8(d + 0x10 + k as u32, v);
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
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [0x226, 0x26]);
    // 0x207FC0's `sq ra` upper half (0) under 0x207EF0 (as b232).
    fx.mem.write_u64(sp - 0x28, 0);
    fx.submit(d, OT, 0);
    let rising = fx.mem.i16(t + 0x54) < 0xAF;
    fade(fx, t, 0x63, rising, (0x50, 0xA), (0xB, 9));
    age(fx, t);
}

/// 0x1E03ED0(D, t): b611's flash step (the target over the window, UVs
/// inset by +8 / 10 and +0xA / 10 over 512, into the descriptor at
/// +0xBA28, no ordering table) in dark grey (0x30), growing by 30 from age
/// 101; alpha up by 9 below 0x80 for 14 updates, from age 101 down by 7.
/// `regs` the unit flash's s0 / s1 (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_5DFC), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f21 / f20: the runner's (0) [H].
    fx.mem.write_u32(sp + 4, 0);
    fx.mem.write_u32(sp, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x30, 0x30, 0x30);
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
    fx.mem.setf(d + 0x18, ee::div(w, 512.0));
    fx.mem.setf(d + 0x1C, ee::div(h, 512.0));
    fx.mem.setf(d + 0x28, ee::div(ee::sub(512.0, w), 512.0));
    fx.mem.setf(d + 0x2C, ee::div(ee::sub(512.0, h), 512.0));
    fx.submit(d, 0, 0);
    if fx.mem.i16(t + 0x54) >= 0x65 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o) + 0x1E;
            fx.mem.set16(t + o, s);
        }
    }
    let n = fx.mem.i16(t + 0x54);
    if n < 0xE {
        fade(fx, t, 0x63, true, (0x80, 9), (0, 0));
    } else if n >= 0x65 {
        fade(fx, t, 0x63, false, (0, 0), (9, 7));
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E05CF0(D): b611's unit flash: in a packet layer (0x2078A0) the
/// attacker drawn into the target (0x1F8270 filter 0, its TEX0 / TEX1 into
/// the sprite descriptor at sp + 0x58; ALPHA 0x48), TEXFLUSH before and
/// after; every flash task stepped (0x1E03ED0); the layer linked into the
/// ordering table at key 0 when any task was alive.
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
    let mut alive = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        alive += 1;
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            flash(fx, slot + 0x2A0, [k, alive]);
        }
    }
    if alive != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}
