//! BMODULE b606 (card 262), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02110, data area 0x1ED0000) [F]: b631's pool of 400 tasks (0x70 bytes at +0x2A0, alive +0x58,
//! step +0x44 as a record address; spawn a6cf8ad55488 at 0x1E02750 =
//! `blib::spawn::task400`) drawn with the second library: rings
//! (`billboard::ring_quads`, 0x1E00920, a random orientation every draw),
//! arc strips (`billboard::arc_strips`, 0x1E00ED0) and quads
//! (`billboard::quad`, 0x1E01230), UVs by uv_rect 0x1E00080. On updates
//! 44 / 55 / 65 / 76 / 84 thirteen white quads burst from the attacker's
//! node 0x3A or 0x46; on 95 a violet arc ring and thirteen rings at each;
//! from 116 to 198 two falling rings a update at each; from 153 to 178
//! thirteen rings a update at node 0x26 (z at the target). A light task
//! dims the stage light (0x1F7F10) from 0x80 to 0x30 and back for the
//! last 20 updates. Returns 1 on update 85, 4 on 215, 2 on 185, 6 on
//! 96..179, 8 once the 220-update timer +0xB210 has run out.
use crate::blib::{billboard, spawn::task400, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const RING_A: u32 = 0x01E0_1930;
const RING_B: u32 = 0x01E0_1AE0;
const QUAD: u32 = 0x01E0_1D00;
const ARC: u32 = 0x01E0_1ED0;
const LIGHT: u32 = 0x01E0_1FF0;
const IDLE: u32 = 0x01E0_2100;
/// The return address of the entry's `jalr` into a step (0x1E02418 + 8).
const TASK_RA: u64 = 0x01E0_2420;
const TIMER: u32 = D + 0xB210;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;
const SINES: u32 = D + 0xB4BC;

/// The entry 0x1E02110.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0xDC);
    for (at, node, clear) in [(0xB244u32, 0x3A, 1), (0xB246, 0x46, 0), (0xB248, 0x26, 0), (0xB24A, 0x23, 0), (0xB24C, 0x2F, 0), (0xB24E, 0x2C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xB1F0 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    fx.mem.set16(D + 0xB2B4, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand();
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 || f == IDLE {
            continue;
        }
        let t = slot + 0x2A0;
        match f {
            RING_A => ring(fx, t, false),
            RING_B => ring(fx, t, true),
            QUAD => quad(fx, t),
            ARC => arc(fx, t),
            LIGHT => light(fx, t),
            _ => panic!("b606: unknown task step {f:#x}"),
        }
    }
    let mut alive = 0u32;
    for k in 0..TASKS {
        let s = D + 0x70 * k;
        if fx.mem.i16(s + 0x2F8) == 0 {
            continue;
        }
        alive += 1;
        let t = s + 0x2A0;
        for (f, local, n, mode, site) in [(RING_A, 0x50u32, 4, 0x21, 0x01E0_24C4u32), (RING_B, 0x60, 3, 0x20, 0x01E0_2568)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            let w = fx.mem.read_u32(D + 0xB1F4);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + local);
            fx.mem.set16(D + 0xB8, n);
            // rand from the entry: its frame saves the entry's s0 (the
            // live tasks so far).
            let v = fx.rand_from(alive, site + 8);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, mode, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == QUAD {
            let w = fx.mem.read_u32(D + 0xB1F0);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x70);
            fx.mem.set16(D + 0xBE, 0);
            billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
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
        0x55 => 1,
        0xD7 => 4,
        0xB9 => 2,
        0x60..=0xB3 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell 0x14 / +0x56 (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// 0x1E014F0(D, 0): the update counter and the spawns (frame 0x50: the
/// entry's s1 = D, s0 = 0). The spawns' saves: the entry's s4 / s3 = 0,
/// s2 = the parameter, the emitter's s1 (the entry's D or a loop count),
/// s0 = D.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x20, 0x01E0_23E0), (0x10, u64::from(D)), (0, 0)]);
    let go = |fx: &mut Fx, f: u32, ra: u32, s1: u64| {
        task400(fx, D, f, 0, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, s1), (0, u64::from(D))]);
    };
    let node = |fx: &mut Fx, slot: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i16(D + slot);
        fx.node_position(h, s, D + 0x250);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let mut s1 = u64::from(D);
    if fx.mem.i16(COUNTER) == 1 {
        go(fx, LIGHT, 0x01E0_1544, s1);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x99..0xB3).contains(&c) {
        node(fx, 0xB248);
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x30);
        let z = fx.mem.read_u32(sp + 0x38);
        fx.mem.write_u32(D + 0x258, z);
        for k in 0..0xDu64 {
            go(fx, RING_A, 0x01E0_15C0, k);
        }
        s1 = 0xD;
    }
    let c = fx.mem.i16(COUNTER);
    if (0x74..0xC7).contains(&c) {
        for (slot, ra) in [(0xB244u32, 0x01E0_1648u32), (0xB246, 0x01E0_16A8)] {
            node(fx, slot);
            for k in 0..2u64 {
                go(fx, RING_B, ra, k);
            }
        }
        s1 = 2;
    }
    if fx.mem.i16(COUNTER) == 0x5F {
        for (slot, arc_ra, ring_ra) in [(0xB244u32, 0x01E0_171Cu32, 0x01E0_1738u32), (0xB246, 0x01E0_1790, 0x01E0_17AC)] {
            node(fx, slot);
            go(fx, ARC, arc_ra, s1);
            for k in 0..0xDu64 {
                go(fx, RING_A, ring_ra, k);
            }
            s1 = 0xD;
        }
    }
    for (at, slot, ra) in [(&[0x2C, 0x41, 0x54][..], 0xB244u32, 0x01E0_1850u32), (&[0x37, 0x4C][..], 0xB246, 0x01E0_18EC)] {
        if !at.contains(&fx.mem.i16(COUNTER)) {
            continue;
        }
        node(fx, slot);
        let y = ee::sub(fx.mem.f(D + 0x254), 1.5);
        fx.mem.setf(D + 0x254, y);
        for k in 0..0xDu64 {
            go(fx, QUAD, ra, k);
        }
    }
    fx.leave(0x50);
}

/// A step's prologue: frame 0x20 (ra, the entry's s0 = 0) or 0x30 (ra,
/// the entry's s1 = D, s0 = 0).
fn prologue(fx: &mut Fx, bytes: u32) {
    let sp = fx.enter(bytes);
    if bytes == 0x20 {
        fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    } else {
        fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    }
}

/// rand % m (HI) from a step's `jal` at `site` (its s0 = the task).
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// 0x205580(t + 0x20) from a step's `jal` at `site` (frame 0x30: the
/// step's s1 = D, s0 = t).
fn angles(fx: &mut Fx, t: u32, site: u32) {
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, u64::from(site + 8)), (0x10, u64::from(D)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
}

/// 0x1E01930 (`falling` false: quad size 8..15, radius 10..19 growing by
/// 16..35) / 0x1E01AE0 (`falling`: size 4..8, radius 20..69, falling by
/// 0.5..1.4 a update), frame 0x20: a grey ring of four (three), life +6
/// 3..5; in by 0x40 for its life, then out by 14.
fn ring(fx: &mut Fx, t: u32, falling: bool) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        let sites: [u32; 7] = if falling {
            [0x01E0_1AFC, 0x01E0_1B28, 0x01E0_1B54, 0x01E0_1B74, 0x01E0_1B90, 0x01E0_1BB0, 0x01E0_1BCC]
        } else {
            [0x01E0_194C, 0x01E0_1978, 0x01E0_19A4, 0x01E0_19C4, 0x01E0_19E0, 0x01E0_1A00, 0x01E0_1A1C]
        };
        let v = if falling { rmod(fx, t, sites[0], 5) + 4 } else { fx.rand_from(t, sites[0] + 8) % 8 + 8 };
        fx.mem.setf(t + 0x48, ee::from_i32(v));
        let v = rmod(fx, t, sites[1], 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = if falling { rmod(fx, t, sites[2], 0x32) + 0x14 } else { rmod(fx, t, sites[2], 0xA) + 0xA };
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, t, sites[3], 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, sites[4], 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, sites[5], 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, sites[6], 3);
        fx.mem.set16(t + 6, v + 3);
        if falling {
            let v = rmod(fx, t, 0x01E0_1BEC, 0xA);
            fx.mem.setf(t + 0x34, ee::from_i32(v + 5));
            let s = ee::div(fx.mem.f(t + 0x34), 10.0);
            fx.mem.setf(t + 0x34, s);
            let v = rmod(fx, t, 0x01E0_1C2C, 0xA);
            fx.mem.setf(t + 0x30, ee::from_i32(v - 5));
            let s = ee::div(fx.mem.f(t + 0x30), 10.0);
            fx.mem.setf(t + 0x30, s);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0x10 {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if falling {
        let y = ee::sub(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
        fx.mem.setf(t + 0x14, y);
    } else if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01D00: a white quad (frame 0x30) of size 100, thrown 6.6 along a
/// random angle, then 0.5 a update; in by 0x40 for 10 updates, then out
/// by 9.
fn quad(fx: &mut Fx, t: u32) {
    prologue(fx, 0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x64);
        fx.mem.set16(t + 0xA, 0x64);
        let v = fx.rand_from(t, 0x01E0_1D38);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_1D70);
        let (_, h) = fx.div(v >> 4, 0x168);
        fx.mem.set16(t + 0x5A, h);
        throw(fx, t, f32::from_bits(0x40D3_3333));
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
        throw(fx, t, 0.5);
    }
    angles(fx, t, 0x01E0_1E98);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// z += `by` sin(+0x5A), x += `by` cos(+0x5A).
fn throw(fx: &mut Fx, t: u32, by: f32) {
    let a = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let z = ee::add(fx.mem.f(t + 0x18), ee::mul(by, s));
    fx.mem.setf(t + 0x18, z);
    let a = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, SINES, a, 1);
    let x = ee::add(fx.mem.f(t + 0x10), ee::mul(by, c));
    fx.mem.setf(t + 0x10, x);
}

/// 0x1E01ED0: the violet arc ring (frame 0x20): outer radius 120 growing
/// by 0.4, its outer alpha in by 0x30 for 14 updates, then out by 15,
/// spinning.
fn arc(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x42F0_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 0xE {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a + 0x30);
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x67, a - 0xF);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    angles(fx, t, 0x01E0_1F88);
    let s = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, s);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x4C), f32::from_bits(0x3ECC_CCCD));
        fx.mem.setf(t + 0x4C, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01FF0: the stage light (frame 0x20): +0x50 from 0x80 down by 10 to
/// 0x30 for 151 updates, then (with fewer than 20 updates left) back up
/// by 10 to 0x80; set every update (0x1F7F10), 0x80 and the task's end
/// when 3 are left.
fn light(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x97 {
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if v < 0x31 { 0x30 } else { v - 0xA });
    } else if fx.mem.i32(TIMER) < 0x14 {
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if v < 0x80 { v + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let v = i32::from(fx.mem.read_u8(t + 0x50));
        fx.set_stage_light(v);
    }
    fx.leave(0x20);
}
