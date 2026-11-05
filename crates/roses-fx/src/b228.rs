//! BMODULE b228 (card 377), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02770, data area 0x1ED0000) [F]: b743's program (card 84) on a pool of 150 tasks (0x70 bytes at
//! +0x2A0, alive +0x58, step +0x44 through `jalr`; spawned by 0x1E02F80 =
//! `spawn::task70` (colour +0xBC94, slot +0xBCEA into +0x50)), drawn with the
//! second library (`uv_rect`, `ring_facing_flat_z`, `ring_quads`,
//! `quad_b_twice`, the tube `tubetrail::TRAIL3`). The emitter 0x1E01910
//! counts +0xBCF2 and keeps the attacker's node 1 (+-20 at random, 20
//! along z by the facing) at D+0x250: on the odd updates 1..99 an orb
//! there (0x1E01B60: `quad_b_twice` at its slot's centre +0xB4F0 + 0x10 k
//! bobbing 3 sin, k = +0xBCEC cycling 0..119; it drifts 5 along z from age
//! 84 and at age 115 bursts into three grey mote rings (0x1E01EE0, b743's)
//! and two yellow spark rings (0x1E02070)) and its tube (0x1E02270: a
//! 15-point history +0x4470 + 0xF0 k of the centre, drawn from age 84 by
//! `tube_with(TRAIL3)`). The band step 0x1E024B0 is drawn but never spawned
//! (not ported). Returns 1 on update 100, 4 on 235, 3 on 225, 6 on
//! 111..219, 8 once the 250-update timer +0xBC90 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::b731::{self, SparksCfg, MOTE_A};
use crate::blib::spawn::{self, history};
use crate::blib::{billboard, quads4, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const ORB: u32 = 0x01E0_1B60;
const MOTE: u32 = 0x01E0_1EE0;
const SPARKS: u32 = 0x01E0_2070;
const TUBE: u32 = 0x01E0_2270;
/// The return address of the entry's `jalr` into a step (0x1E02A60 + 8).
const TASK_RA: u64 = 0x01E0_2A68;
const SINES: u32 = D + 0xBEF4;
const CENTRES: u32 = D + 0xB4F0;
const TEX: u32 = D + 0xBC70;
const TIMER: u32 = D + 0xBC90;
const NODE1: u32 = D + 0xBCBC;
const SLOT: u32 = D + 0xBCEA;
const NEXT: u32 = D + 0xBCEC;
const YAW: u32 = D + 0xBCEE;
const COUNTER: u32 = D + 0xBCF2;
const BYTES: u32 = D + 0xBCF4;
const BYTES_B: u32 = D + 0xBDF4;

/// 0x1E02070's yellow spark ring (b743's 0x1E026D0 with its size 10..19,
/// colour (0x80, 0x80, 0) and radius 150..299).
const SPARKS_Y: SparksCfg = SparksCfg { size: (0xA, 0xA, 0), colour: (0x80, 0x80, 0), cell: true, radius: (0x96, 0x96) };

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

/// Both alphas (+0x63 / +0x67) in by 0x2A below 0x80 while `rising`; else
/// +0x63 out by 14 twice (the record's second test reads +0x63 again; the
/// task ends when the first finds it below 16).
fn fade2(fx: &mut Fx, t: u32, rising: bool) {
    if rising {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        }
    }
}

/// +0x60 grey `hi` on the odd ages, `lo` on the even.
fn flicker(fx: &mut Fx, t: u32, hi: i32, lo: i32) {
    let g = if odd(fx.mem.i16(t + 0x54)) { hi } else { lo };
    l2::rgb(fx, t + 0x60, g, g, g);
}

/// The entry 0x1E02770 (a0 = D, a1 = param; s0 = D, s1 = 0, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0xBC9C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xC060);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xBCF0, param);
    fx.mem.set32(TIMER, 0xFA);
    for (at, n, clear) in [(NODE1, 1, 1), (D + 0xBCBE, 0x1C, 0), (D + 0xBCC0, 0xD, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(at, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xBCCA, r);
    for k in 0xEu32..0x10 {
        let tex = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x260 + 4 * k, tex);
    }
    for k in 0..2u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(v, 0x01E0_2900);
    fx.mem.set16(YAW, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2930, 0x1E);
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_298C);
        fx.mem.write_u8(BYTES + k, v.wrapping_rem(4) as u8);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_29E4, 0x28);
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
        // The steps' prologues: the entry's s0 = D, s1 = 0.
        let frame = match f {
            ORB | TUBE => 0x30,
            MOTE | SPARKS => 0x20,
            _ => panic!("b228: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match f {
            ORB | TUBE => fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]),
            _ => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]),
        }
        match f {
            ORB => orb(fx, t, slot, k),
            MOTE => b731::mote(fx, t, MOTE, &MOTE_A),
            SPARKS => b731::sparks(fx, t, SPARKS, &SPARKS_Y),
            _ => tube_step(fx, t),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        draw(fx, sp, t);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x64 => 1,
        0xEB => 4,
        0xE1 => 3,
        0x6F..=0xDB => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, id: i32, r: u32) {
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

fn texture(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// 3 sin(+0x5A) (0x108910's second form).
fn bob(fx: &mut Fx, t: u32) -> f32 {
    let a = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, SINES, a, 1);
    ee::mul(3.0, s)
}

/// The entry's draw of one live task by its step (s0 = D).
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    let i = |fx: &mut Fx| fx.mem.i16(t + 0x50) as u32;
    if f == ORB {
        cell(fx, t, 0x14, sp + 0x60);
        for (o, v) in [(0x94u32, 1), (0x96, 1), (0x98, 0x3F), (0x9A, 0x3F)] {
            fx.mem.set16(D + o, v);
        }
        texture(fx, TEX + 4);
        fx.mem.set16(D + 0xBE, 0);
        let c = CENTRES + 0x10 * i(fx);
        let x = fx.mem.f(c);
        fx.mem.setf(sp + 0x50, x);
        let b = bob(fx, t);
        let c = CENTRES + 0x10 * i(fx);
        let y = ee::add(fx.mem.f(c + 4), b);
        fx.mem.setf(sp + 0x54, y);
        let c = CENTRES + 0x10 * i(fx);
        let z = fx.mem.f(c + 8);
        fx.mem.setf(sp + 0x58, z);
        billboard::quad_b_twice(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
    }
    if f == MOTE {
        cell(fx, t, 0x14, sp + 0x70);
        texture(fx, TEX);
        fx.mem.set16(D + 0xB8, 6);
        let v = fx.mem.i16(t + 0xA);
        fx.mem.set16(D + 0xC0, v);
        copy3(fx, t + 0x10, D + 0x20);
        quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == SPARKS {
        texture(fx, D + 0x29C);
        cell(fx, t, 0x9B, sp + 0x80);
        fx.mem.set16(D + 0xB8, 0xA);
        fx.mem.set16(D + 0xB0, 1);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
        fx.mem.set16(D + 0xB8, 0);
        fx.mem.set16(D + 0xB0, 0);
    }
    if f == TUBE {
        texture(fx, TEX);
        cell(fx, t, 0x14, sp + 0x90);
        let h2 = fx.mem.i16(D + 0x9A) >> 1;
        fx.mem.set16(D + 0x9A, h2);
        let v = fx.mem.i16(D + 0x96) + fx.mem.i16(D + 0x9A);
        fx.mem.set16(D + 0x96, v);
        fx.mem.set16(D + 0x9A, 1);
        let h = fx.self_handle();
        unit_position_saving(fx, h, sp + 0x50, u64::from(D), 0x01E0_2D68);
        fx.mem.write_u32(D + 0x20, 0);
        let b = bob(fx, t);
        fx.mem.setf(D + 0x24, b);
        fx.mem.write_u32(D + 0x28, 0);
        fx.mem.set16(D + 0xBE, 3);
        if fx.mem.i16(t + 0x54) >= 0x54 {
            let k = i(fx);
            tubetrail::tube_with(fx, &tubetrail::TRAIL3, D, t, D + 0x4470 + 0xF0 * k, 0);
        }
    }
    if f != ORB && f != MOTE && f != SPARKS && f != TUBE && f != 0 {
        panic!("b228: unported draw case {f:#x}");
    }
}

/// The spawn's registers: s0..s4 at the call.
type Regs = [u64; 5];

/// 0x1E02F80(D, step, 0): `spawn::task70` on this record's layout
/// (4ac5b35fe8b8: 150 tasks, colour +0xBC94, slot +0xBCEA).
fn spawn(fx: &mut Fx, step: u32, r: Regs, ra: u32) -> i32 {
    let p = spawn::pool70_b4("4ac5b35fe8b8").expect("layout");
    let saves = [(0x50, u64::from(ra)), (0x40, r[4]), (0x30, r[3]), (0x20, r[2]), (0x10, r[1]), (0, r[0])];
    spawn::task70(fx, D, p, step, 0, &saves)
}

/// 0x1E01910(D, 0) (frame 0x40: ra, the entry's s2 = param, s1 = 0, s0 =
/// D): the update counter +0xBCF2, the spawn point and the orbs.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_2A38), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODE1);
    node_position_saving(fx, h, s, D + 0x250, u64::from(D), Some(0), 0x01E0_1960);
    for (o, ra) in [(0u32, 0x01E0_1968u32), (4, 0x01E0_199C), (8, 0x01E0_19D0)] {
        let v = rmod(fx, D, ra, 0x28) - 0x14;
        let x = ee::add(fx.mem.f(D + 0x250 + o), ee::from_i32(v));
        fx.mem.setf(D + 0x250 + o, x);
    }
    let z = fx.mem.f(D + 0x258);
    let z = if fx.mem.i16(YAW) != 0 { ee::sub(z, 20.0) } else { ee::add(z, 20.0) };
    fx.mem.setf(D + 0x258, z);
    let c = fx.mem.i16(COUNTER);
    if (1..=100).contains(&c) && odd(c) {
        let r: Regs = [0, 1, u64::from(D), 0, 0];
        let v = fx.mem.i16(NEXT);
        fx.mem.set16(SLOT, v);
        spawn(fx, ORB, r, 0x01E0_1AC8);
        spawn(fx, TUBE, r, 0x01E0_1ADC);
        let n = fx.mem.i16(NEXT) + 1;
        fx.mem.set16(NEXT, n);
        if fx.mem.i16(NEXT) >= 0x78 {
            fx.mem.set16(NEXT, 0);
        }
    }
    fx.leave(0x40);
}

/// 0x1E01B60 (frame 0x30: s0 = the task, s1 = D): an orb at its slot's
/// centre (+0x10.. into +0xB4F0 + 0x10 k); size 7, grey; in by 0x2A for 110
/// updates, then out; at age 115 three mote rings and two spark rings at
/// the centre; spun by 5 a update to age 82; from age 84 the centre moves
/// 5 along z (- when the attacker's yaw is non-zero).
fn orb(fx: &mut Fx, t: u32, slot: u32, k: u32) {
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
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x6E;
    fade2(fx, t, rising);
    flicker(fx, t, 0xFF, 0x80);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1CFC);
    if fx.mem.i16(t + 0x54) == 0x73 {
        let i = fx.mem.i16(t + 0x50) as u32;
        for o in 0..3 {
            let w = fx.mem.read_u32(CENTRES + 0x10 * i + 4 * o);
            fx.mem.write_u32(D + 0x250 + 4 * o, w);
        }
        let r: Regs = [u64::from(t), u64::from(D), u64::from(slot), u64::from(k), 0];
        let mut ra = 0x01E0_1D74u32;
        for f in [MOTE, MOTE, MOTE, SPARKS, SPARKS] {
            spawn(fx, f, r, ra);
            ra += 0x14;
        }
    }
    flicker(fx, t, 0xC0, 0x40);
    if fx.mem.i16(t + 0x54) < 0x53 {
        let v = fx.mem.i16(t + 0x5A) + 5;
        fx.mem.set16(t + 0x5A, v);
    }
    age(fx, t);
    if fx.mem.i16(t + 0x54) >= 0x54 {
        let i = fx.mem.i16(t + 0x50) as u32;
        let z = fx.mem.f(CENTRES + 0x10 * i + 8);
        let z = if fx.mem.i16(YAW) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(CENTRES + 0x10 * i + 8, z);
    }
}

/// 0x1E02270 (frame 0x30: s0 = the task, s1 = D): an orb's tube: its
/// slot's centre pushed into the 15-point history +0x4470 + 0xF0 k (filled
/// with it on the first update) until age 117, 2 points growing to 4, size
/// 4, cell 4, flickering grey; the alphas as the orb's.
fn tube_step(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        let i = fx.mem.i16(t + 0x50) as u32;
        let trail = D + 0x4470 + 0xF0 * i;
        for j in 0..0xFu32 {
            copy3(fx, CENTRES + 0x10 * i, trail + 0x10 * j);
        }
        fx.mem.set16(t + 6, 2);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0x56, 4);
        fx.mem.write_u32(t + 0x48, 0x4080_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x6E;
    fade2(fx, t, rising);
    if fx.mem.i16(t + 0x54) < 0x53 {
        let v = fx.mem.i16(t + 0x5A) + 5;
        fx.mem.set16(t + 0x5A, v);
    }
    if fx.mem.i16(t + 0x54) < 0x75 {
        let i = fx.mem.i16(t + 0x50) as u32;
        history(fx, D + 0x4550 + 0xF0 * i, 0xF, CENTRES + 0x10 * i);
    }
    flicker(fx, t, 0xC0, 0x40);
    if fx.mem.i16(t + 0x54) != 0 && fx.mem.i16(t + 6) < 4 {
        let v = fx.mem.i16(t + 6) + 1;
        fx.mem.set16(t + 6, v);
    }
    age(fx, t);
}
