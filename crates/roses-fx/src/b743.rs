//! BMODULE b743 (card 84), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03920, data area 0x1ED0000) [F] (helper A23, 3
//! Nov 2026): b433's program (card 192) on a pool of 200 tasks (0x70
//! bytes at +0x2A0, alive +0x58, step +0x44 through `jalr`; spawned by
//! 0x1E043D0 with the colour +0xD274 and the slot +0xD59A into +0x50),
//! drawn with the second library (`uv_rect`, `ring_facing_flat_z`,
//! `ring_quads`, `arc_strips_flat`, `tubetrail::tube`, `quad_b_twice`).
//! The emitter 0x1E01C70 counts +0xD5A2: on update 1 the stage light
//! (0x1E03820, down to 0x30 while the timer is at least 21); on the odd
//! updates 0x9B..0xC8 at the attacker's node 1 (+-10 / +-25) an orb
//! (0x1E020C0, `quad_b_twice` round its slot's centre +0xCAD0 + 0x10 k
//! turned by +0xD49C / radius +0xD28C, k = +0xD59C cycling 0..119; at age
//! 55 it bursts into three motes 0x1E02540, two spark rings 0x1E026D0 and
//! an arc 0x1E02C20; it sinks by 8 from age 34) and its tube (0x1E02D60, a
//! 15-point history +0x5A50 + 0xF0 k drawn by `tube`); on the odd updates
//! 0x34..0x99 two falling orbs (0x1E028D0, flying to the attacker's node
//! 1 from 50..199 above, sinking by 15) with their tubes (0x1E03080); on
//! update 0x3B two arcs riding the attacker's nodes 0x1D / 0x34
//! (0x1E033E0). The band step 0x1E03560 is drawn but never spawned (not
//! ported). Returns 1 on update 200, 4 on 325, 2 on 265, 6 on 211..259, 8
//! once the 330-update timer +0xD270 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, spawn::history, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
/// The step functions a task holds (+0x44), as the record's addresses.
const ORB: u32 = 0x01E0_20C0;
const MOTE: u32 = 0x01E0_2540;
const SPARKS: u32 = 0x01E0_26D0;
const FALL: u32 = 0x01E0_28D0;
const ARC: u32 = 0x01E0_2C20;
const TUBE: u32 = 0x01E0_2D60;
const FALL_TUBE: u32 = 0x01E0_3080;
const ARC_B: u32 = 0x01E0_33E0;
const LIGHT: u32 = 0x01E0_3820;
/// The return address of the entry's `jalr` into a step (0x1E03C20 + 8).
const TASK_RA: u64 = 0x01E0_3C28;
const SINES: u32 = D + 0xD7A4;
const CENTRES: u32 = D + 0xCAD0;
const TEX: u32 = D + 0xD250;
const TIMER: u32 = D + 0xD270;
const COLOUR: u32 = D + 0xD274;
const RADII: u32 = D + 0xD28C;
const NODE1: u32 = D + 0xD47C;
const TARGET_NODE: u32 = D + 0xD48A;
const ANGLES: u32 = D + 0xD49C;
const SLOT: u32 = D + 0xD59A;
const NEXT: u32 = D + 0xD59C;
const YAW: u32 = D + 0xD59E;
const COUNTER: u32 = D + 0xD5A2;
const BYTES: u32 = D + 0xD5A4;
const BYTES_B: u32 = D + 0xD6A4;

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// rand (the caller's s0 = `s0`) from `ra`, mod `m` (truncated).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn odd(n: i32) -> bool {
    n % 2 == 1
}

fn copy3(fx: &mut Fx, from: u32, to: u32) {
    for o in 0..3 {
        let w = fx.mem.read_u32(from + 4 * o);
        fx.mem.write_u32(to + 4 * o, w);
    }
}

/// The alpha byte `a` up by `up.1` while below `up.0` while `rising`, else
/// down by `down.1` while at least `down.0`, else the task ends.
fn fade(fx: &mut Fx, t: u32, a: u32, rising: bool, up: (u8, u8), down: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if rising {
        if v < up.0 {
            fx.mem.write_u8(t + a, v.wrapping_add(up.1));
        }
    } else if v >= down.0 {
        fx.mem.write_u8(t + a, v.wrapping_sub(down.1));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// Both alphas (+0x63 / +0x67) in by 0x2A below 0x80 while `rising`;
/// else, when `out`, out by 14 (the task ending once +0x63 is below 16).
fn fade2(fx: &mut Fx, t: u32, rising: bool, out: bool) {
    if rising {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
        }
    } else if out {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x67);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x67, a - 0xE);
        }
    }
}

/// White on the odd ages, light grey on the even.
fn flicker(fx: &mut Fx, t: u32) {
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    } else {
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
    }
}

/// The entry 0x1E03920 (a0 = D, a1 = param; s0 = param, s1 = D, s2 = 0).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xE0);
    fx.mem.write_u32(D + 0xD27C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xE0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xD910);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xD5A0, param);
    fx.mem.set32(TIMER, 0x14A);
    for (at, n, clear) in [(NODE1, 1, 1), (D + 0xD47E, 0x1D, 0), (D + 0xD480, 0x34, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(at, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for k in 0xAu32..0x10 {
        let tex = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x260 + 4 * k, tex);
    }
    let tex = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, tex);
    let tex = fx.texture(5, 0);
    fx.mem.write_u32(TEX, tex);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.fix_residue(v, 0x01E0_3AC0);
    fx.mem.set16(YAW, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3AF0, 0x1E);
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(k, 0x01E0_3B4C);
        fx.mem.write_u8(BYTES + k, v.wrapping_rem(4) as u8);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3BA4, 0x28);
        fx.mem.write_u8(BYTES_B + k, (v - 0x14) as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
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
        // The steps' prologues: the entry's s0 = the slot, s1 = D, s2 = 0,
        // s3 = k.
        let frame = match f {
            ORB => 0x40,
            FALL => 0x40,
            TUBE | FALL_TUBE => 0x60,
            ARC_B => 0x30,
            MOTE | SPARKS | ARC | LIGHT => 0x20,
            _ => panic!("b743: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match f {
            ORB => fx.saved(s, &[(0x30, TASK_RA), (0x20, 0), (0x10, u64::from(D)), (0, u64::from(slot))]),
            FALL | ARC_B => fx.saved(s, &[(frame - 0x10, TASK_RA), (0x10, u64::from(D)), (0, u64::from(slot))]),
            TUBE | FALL_TUBE => fx.saved(s, &[(0x40, TASK_RA), (0x30, u64::from(k)), (0x20, 0), (0x10, u64::from(D)), (0, u64::from(slot))]),
            _ => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(slot))]),
        }
        match f {
            ORB => orb(fx, t, k),
            MOTE => mote(fx, t),
            SPARKS => sparks(fx, t),
            FALL => fall(fx, s, t),
            ARC => arc(fx, t),
            TUBE => tube_step(fx, s, t, false),
            FALL_TUBE => tube_step(fx, s, t, true),
            ARC_B => arc_b(fx, t),
            _ => light(fx, t),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        draw(fx, sp, t, k);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xC8 => 1,
        0x145 => 4,
        0x109 => 2,
        0xD3..=0x103 => 6,
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
/// angle) into `out`.
fn orb_point(fx: &mut Fx, t: u32, out: u32) {
    let i = fx.mem.i16(t + 0x50) as u32;
    let a = fx.mem.i16(ANGLES + 2 * i);
    let s = l2::sin_deg(fx, SINES, a, 1);
    let v = ee::add(fx.mem.f(CENTRES + 0x10 * i), ee::mul(fx.mem.f(RADII + 4 * i), s));
    fx.mem.setf(out, v);
    let i = fx.mem.i16(t + 0x50) as u32;
    let y = fx.mem.f(CENTRES + 0x10 * i + 4);
    fx.mem.setf(out + 4, y);
    let i = fx.mem.i16(t + 0x50) as u32;
    let a = fx.mem.i16(ANGLES + 2 * i);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let v = ee::add(fx.mem.f(CENTRES + 0x10 * i + 8), ee::mul(fx.mem.f(RADII + 4 * i), s));
    fx.mem.setf(out + 8, v);
}

/// The entry's draw of one live task by its step (s0 = k).
fn draw(fx: &mut Fx, sp: u32, t: u32, k: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    let orb_quad = |fx: &mut Fx, r: u32| {
        cell(fx, t, TEX, 0x14, r);
        fx.mem.set16(D + 0xBE, 0);
        orb_point(fx, t, sp + 0x60);
        billboard::quad_b_twice(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
    };
    let tube = |fx: &mut Fx, r: u32, ra: u32| {
        cell(fx, t, TEX, 0x14, r);
        let v = fx.mem.i16(D + 0x96) + 0xF;
        fx.mem.set16(D + 0x96, v);
        fx.mem.set16(D + 0x98, 0x1F);
        fx.mem.set16(D + 0x9A, 1);
        let h = fx.self_handle();
        unit_position_saving(fx, h, sp + 0x60, u64::from(k), ra);
        for o in [0x20u32, 0x24, 0x28] {
            fx.mem.write_u32(D + o, 0);
        }
        fx.mem.set16(D + 0xBE, 2);
        let i = fx.mem.i16(t + 0x50) as u32;
        tubetrail::tube(fx, D, t, D + 0x5A50 + 0xF0 * i, 0);
    };
    let arcs = |fx: &mut Fx| {
        fx.mem.set16(D + 0xB8, 0x18);
        billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    };
    if f == ORB {
        orb_quad(fx, sp + 0x70);
    }
    if f == MOTE {
        cell(fx, t, TEX, 0x14, sp + 0x80);
        fx.mem.set16(D + 0xB8, 6);
        let v = fx.mem.i16(t + 0xA);
        fx.mem.set16(D + 0xC0, v);
        copy3(fx, t + 0x10, D + 0x20);
        quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == SPARKS {
        cell(fx, t, D + 0x29C, 0x9B, sp + 0x90);
        fx.mem.set16(D + 0xB8, 0xA);
        fx.mem.set16(D + 0xB0, 1);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
        fx.mem.set16(D + 0xB8, 0);
        fx.mem.set16(D + 0xB0, 0);
    }
    if f == ARC {
        arcs(fx);
    }
    if f == FALL {
        orb_quad(fx, sp + 0xA0);
    }
    if f == TUBE {
        tube(fx, sp + 0xB0, 0x01E0_40E8);
    }
    if f == FALL_TUBE {
        tube(fx, sp + 0xC0, 0x01E0_41B0);
    }
    if f == ARC_B {
        arcs(fx);
    }
}

/// The emitter's registers at a call: s0, s1 (s2 = D; s3 / s4 the
/// runner's, 0 [H]).
#[derive(Clone, Copy)]
struct Regs(u64, u64, u64);

/// 0x1E043D0(D, step, 0): the first free of the 200 tasks cleared and set
/// off at D+0x250 (as i16 and its xyz at +0x10) with the colour +0xD274
/// (+0x60, +0x64), +0x50 = +0xD59A; 1 when one was free.
fn spawn(fx: &mut Fx, step: u32, r: Regs, s3: u64, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, s3), (0x20, r.2), (0x10, r.1), (0, r.0)]);
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

/// +0xD59C up by one, back to 0 at 120.
fn next_slot(fx: &mut Fx) {
    let n = fx.mem.i16(NEXT) + 1;
    fx.mem.set16(NEXT, n);
    if fx.mem.i16(NEXT) >= 0x78 {
        fx.mem.set16(NEXT, 0);
    }
}

/// 0x1E01C70(D, 0) (frame 0x40: ra, the entry's s2 = 0, s1 = D, s0 =
/// param): the update counter +0xD5A2 and the emitters.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_3BF8), (0x20, 0), (0x10, u64::from(D)), (0, p)]);
    let mut r = Regs(p, u64::from(D), u64::from(D));
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    if (0x9A..0xC9).contains(&c) && odd(c) {
        let n = if c == 0x9A { 0x1E } else { 1 };
        r.1 = n as u64;
        for k in 0..n {
            r.0 = k as u64;
            let h = fx.self_handle();
            let s = fx.mem.i16(NODE1);
            node_position_saving(fx, h, s, D + 0x250, r.0, Some(r.1), 0x01E0_1D28);
            let v = rmod(fx, r.0 as u32, 0x01E0_1D30, 0x14) - 0xA;
            let x = ee::add(fx.mem.f(D + 0x250), ee::from_i32(v));
            fx.mem.setf(D + 0x250, x);
            let v = rmod(fx, r.0 as u32, 0x01E0_1D64, 0x32) - 0x19;
            let y = ee::add(fx.mem.f(D + 0x254), ee::from_i32(v));
            fx.mem.setf(D + 0x254, y);
            let v = fx.mem.i16(NEXT);
            fx.mem.set16(SLOT, v);
            spawn(fx, ORB, r, 0, 0x01E0_1DB8);
            spawn(fx, TUBE, r, 0, 0x01E0_1DCC);
            next_slot(fx);
        }
        r.0 = n as u64;
    }
    let h = fx.self_handle();
    let s = fx.mem.i16(NODE1);
    node_position_saving(fx, h, s, D + 0x250, r.0, Some(r.1), 0x01E0_1E58);
    for (o, ra, m, b) in [(0u32, 0x01E0_1E60u32, 0x50, -0x28), (8, 0x01E0_1E94, 0x50, -0x28), (4, 0x01E0_1EC8, 0x96, 0x64)] {
        let v = rmod(fx, r.0 as u32, ra, m) + b;
        let p = ee::add(fx.mem.f(D + 0x250 + o), ee::from_i32(v));
        fx.mem.setf(D + 0x250 + o, p);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x34..0x9A).contains(&c) && odd(c) {
        r.0 = 2;
        for k in 0..2 {
            r.1 = k;
            let v = fx.mem.i16(NEXT);
            fx.mem.set16(SLOT, v);
            spawn(fx, FALL, r, 0, 0x01E0_1F98);
            spawn(fx, FALL_TUBE, r, 0, 0x01E0_1FAC);
            next_slot(fx);
        }
        r.1 = 2;
    }
    if fx.mem.i16(COUNTER) == 0x3B {
        fx.mem.set16(SLOT, 1);
        spawn(fx, ARC_B, r, 0, 0x01E0_2054);
        fx.mem.set16(SLOT, 2);
        spawn(fx, ARC_B, r, 0, 0x01E0_2078);
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, LIGHT, r, 0, 0x01E0_20A4);
    }
    fx.leave(0x40);
}

/// 0x1E020C0 (frame 0x40: s1 = D, s2 = the slot index): an orb at its
/// slot's centre (+0x10.. into +0xCAD0 + 0x10 k), the slot's angle rand %
/// 360 turning by 18 to age 32 and radius (rand % 220 + 50) / 10; size 15,
/// grey, flickering white; in by 0x2A for 52 updates, then out by 14; at
/// age 55 three motes, two spark rings and an arc at its point; from age
/// 34 its centre moves 8 along z (- when the attacker's yaw is non-zero).
fn orb(fx: &mut Fx, t: u32, k: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xF);
        fx.mem.set16(t + 0xA, 0xF);
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
        let v = rmod(fx, t, 0x01E0_2188, 0x168);
        let i = fx.mem.i16(t + 0x50) as u32;
        fx.mem.set16(ANGLES + 2 * i, v);
        let v = rmod(fx, t, 0x01E0_21B0, 0xDC);
        let i = fx.mem.i16(t + 0x50) as u32;
        fx.mem.setf(RADII + 4 * i, ee::from_i32(v + 0x32));
        let i = fx.mem.i16(t + 0x50) as u32;
        let r = ee::div(fx.mem.f(RADII + 4 * i), 10.0);
        fx.mem.setf(RADII + 4 * i, r);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x34;
    fade2(fx, t, rising, true);
    flicker(fx, t);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_22EC);
    if fx.mem.i16(t + 0x54) == 0x37 {
        let i = fx.mem.i16(t + 0x50) as u32;
        orb_point(fx, t, D + 0x250);
        let r = Regs(u64::from(t), u64::from(D), u64::from(i));
        let mut ra = 0x01E0_23E4u32;
        for f in [MOTE, MOTE, MOTE, SPARKS, SPARKS, ARC] {
            spawn(fx, f, r, u64::from(k), ra);
            ra += 0x14;
        }
    }
    if fx.mem.i16(t + 0x54) < 0x21 {
        let v = fx.mem.i16(t + 0x5A) + 5;
        fx.mem.set16(t + 0x5A, v);
        let i = fx.mem.i16(t + 0x50) as u32;
        let a = fx.mem.i16(ANGLES + 2 * i) + 0x12;
        fx.mem.set16(ANGLES + 2 * i, a);
    }
    age(fx, t);
    if fx.mem.i16(t + 0x54) >= 0x22 {
        let i = fx.mem.i16(t + 0x50) as u32;
        let z = fx.mem.f(CENTRES + 0x10 * i + 8);
        let z = if fx.mem.i16(YAW) != 0 { ee::sub(z, 8.0) } else { ee::add(z, 8.0) };
        fx.mem.setf(CENTRES + 0x10 * i + 8, z);
    }
}

/// 0x1E02540: a grey mote ring (ring_facing_flat_z of 6 from the bytes
/// +0xD5A4) of size 40, cell rand % 7, turned rand % 360, orientation +0xA
/// rand % 8, radius 10..59 growing by 16..45; alpha 0x80 for 3..7 updates,
/// then out by 8.
fn mote(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4220_0000);
        let v = rmod(fx, t, 0x01E0_2568, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_2594, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_25B0);
        fx.mem.set16(t + 0xA, v.wrapping_rem(8));
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        let v = rmod(fx, t, 0x01E0_25D4, 0x32);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_25F4, 0x1E);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, 0x01E0_2614, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_2630, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0xA, 8));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    age(fx, t);
}

/// 0x1E026D0: a green spark ring (size 10..19, cell rand % 4, (0x80,
/// 0xFF, 0x80)) turned by rand % 0xFFFF on each axis, radius 150..299
/// growing by 1..3, row 1..8; alpha 0x60 for 2..4 updates, then out by 11.
fn sparks(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_26F4, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = fx.rand_from(t, 0x01E0_2720);
        fx.mem.set16(t + 0x56, v.wrapping_rem(4));
        l2::rgb(fx, t + 0x60, 0x80, 0xFF, 0x80);
        for (o, ra) in [(0x20u32, 0x01E0_2754u32), (0x24, 0x01E0_2770), (0x28, 0x01E0_278C)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_27B0, 0x96);
        fx.mem.set16(t + 0x50, v + 0x96);
        let v = rmod(fx, t, 0x01E0_27D0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_27EC, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, 0x01E0_280C);
        fx.mem.set16(t + 8, v.wrapping_rem(8) + 1);
        let v = rmod(fx, t, 0x01E0_282C, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0xD, 0xB));
    }
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, v);
    age(fx, t);
}

/// 0x1E028D0 (frame 0x40: s1 = D; the attacker's node 1 at sp + 0x30): a
/// falling orb: its slot's centre from its point, moving a fifteenth of
/// the way to the node a update in x / z and sinking by 15; angle rand %
/// 360, radius 0; size 7, flickering; in by 0x2A for 10 updates, then out
/// by 14 once below 50; it ends from update 154.
fn fall(fx: &mut Fx, sp: u32, t: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i16(NODE1);
    node_position_saving(fx, h, s, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_2904);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 7);
        fx.mem.set16(t + 0xA, 7);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        for o in 0..3 {
            let i = fx.mem.i16(t + 0x50) as u32;
            let w = fx.mem.read_u32(t + 0x10 + 4 * o);
            fx.mem.write_u32(CENTRES + 0x10 * i + 4 * o, w);
        }
        for o in [0u32, 8] {
            let i = fx.mem.i16(t + 0x50) as u32;
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x30 + o), fx.mem.f(CENTRES + 0x10 * i + o)), 15.0);
            fx.mem.setf(t + 0x30 + o, v);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, 0x01E0_2A0C, 0x168);
        let i = fx.mem.i16(t + 0x50) as u32;
        fx.mem.set16(ANGLES + 2 * i, v);
        let i = fx.mem.i16(t + 0x50) as u32;
        fx.mem.write_u32(RADII + 4 * i, 0);
    }
    fall_fade(fx, t);
    flicker(fx, t);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2B54);
    age(fx, t);
    let i = fx.mem.i16(t + 0x50) as u32;
    let c = CENTRES + 0x10 * i;
    let y = ee::sub(fx.mem.f(c + 4), 15.0);
    fx.mem.setf(c + 4, y);
    let z = ee::add(fx.mem.f(c + 8), fx.mem.f(t + 0x38));
    fx.mem.setf(c + 8, z);
    let x = ee::add(fx.mem.f(c), fx.mem.f(t + 0x30));
    fx.mem.setf(c, x);
    if fx.mem.i16(COUNTER) >= 0x9A {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The falling orbs' fade: in for 10 updates, then out once the slot's
/// centre is below 50.
fn fall_fade(fx: &mut Fx, t: u32) {
    let rising = fx.mem.i16(t + 0x54) < 0xA;
    let out = !rising && {
        let i = fx.mem.i16(t + 0x50) as u32;
        crate::exe::ee_lt(fx.mem.f(CENTRES + 0x10 * i + 4), 50.0)
    };
    fade2(fx, t, rising, out);
}

/// 0x1E02C20: a white / green arc ring (arc_strips_flat of 24 at its
/// point) of inner radius 80, the outer colour flickering (0x80, 0xFF,
/// 0x80) / (0x70, 0xCF, 0x70); in by 0x40 for 2 updates, then out by 11;
/// facing the camera, spun by 0x4000 a update.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42A0_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0xFF, 0x80);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 2;
    fade(fx, t, 0x63, rising, (0x80, 0x40), (0xD, 0xB));
    arc_tail(fx, t, 0x01E0_2D34);
    age(fx, t);
}

/// The arcs' outer flicker, the camera's angles (0x205580 from `ra`, s1 =
/// D) and the spin.
fn arc_tail(fx: &mut Fx, t: u32, ra: u32) {
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x64, 0x80, 0xFF, 0x80);
    } else {
        l2::rgb(fx, t + 0x64, 0x70, 0xCF, 0x70);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), ra);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
}

/// 0x1E02D60 / 0x1E03080 (frame 0x60, the point at sp + 0x50): an orb's
/// tube: its slot's orb point pushed into the 15-point history +0x5A50 +
/// 0xF0 k (filled with it on the first update) until age 117, flickering
/// white, 2 points growing to 8 (12), size 7 (5), cell 4; the alphas as
/// the orb's (the falling one's ends from update 154).
fn tube_step(fx: &mut Fx, sp: u32, t: u32, falling: bool) {
    let p = sp + 0x50;
    orb_point(fx, t, p);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        let i = fx.mem.i16(t + 0x50) as u32;
        let trail = D + 0x5A50 + 0xF0 * i;
        for j in 0..0xFu32 {
            copy3(fx, p, trail + 0x10 * j);
        }
        fx.mem.set16(t + 6, 2);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0x56, 4);
        fx.mem.write_u32(t + 0x48, if falling { 0x40A0_0000 } else { 0x40E0_0000 });
        fx.mem.set16(t + 0x58, 2);
    }
    if falling {
        fall_fade(fx, t);
    } else {
        let rising = fx.mem.i16(t + 0x54) < 0x6E;
        fade2(fx, t, rising, true);
    }
    if fx.mem.i16(t + 0x54) < 0x53 {
        let v = fx.mem.i16(t + 0x5A) + 5;
        fx.mem.set16(t + 0x5A, v);
    }
    if fx.mem.i16(t + 0x54) < 0x75 {
        let i = fx.mem.i16(t + 0x50) as u32;
        history(fx, D + 0x5B30 + 0xF0 * i, 0xF, p);
    }
    flicker(fx, t);
    let max = if falling { 0xC } else { 8 };
    if fx.mem.i16(t + 0x54) != 0 && fx.mem.i16(t + 6) < max {
        let v = fx.mem.i16(t + 6) + 1;
        fx.mem.set16(t + 6, v);
    }
    age(fx, t);
    if falling && fx.mem.i16(COUNTER) >= 0x9A {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E033E0 (frame 0x30: s1 = D): [`arc`] of inner radius 70 in pale
/// grey (0xDF), in by 12 below 0x80 for 250 updates, riding the
/// attacker's node +0xD47C + 2 x slot (0x1D / 0x34).
fn arc_b(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x428C_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xDF, 0xDF, 0xDF);
        l2::rgb(fx, t + 0x64, 0x80, 0xFF, 0x80);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 0xFA;
    fade(fx, t, 0x63, rising, (0x80, 0xC), (0xD, 0xB));
    arc_tail(fx, t, 0x01E0_34FC);
    let h = fx.self_handle();
    let n = fx.mem.i16(t + 0x50) as u32;
    let s = fx.mem.i16(NODE1 + 2 * n);
    node_position_saving(fx, h, s, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_3530);
    age(fx, t);
}

/// 0x1E03820: the stage light +0x50 from 0x80 down by 10 to 0x30 while
/// the timer is at least 21, then back up by 10 to 0x80; at the timer's 3
/// the light 0x80 and the task ends.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let l = fx.mem.i16(t + 0x50);
    if fx.mem.i32(TIMER) >= 0x15 {
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    age(fx, t);
    if fx.mem.read_u32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
}
