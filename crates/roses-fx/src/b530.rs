//! BMODULE b530 (card 641), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E021B0, data area 0x1ED0000) [F]: b425's pool of 500 tasks (0x70 bytes at +0x2A0, alive +0x58,
//! step +0x44 as a record address; spawn 0x1E031A0 = `b425::spawn`) drawn
//! with the second library: rings (`billboard::ring_quads`, 0x1E00920, a
//! random orientation every draw) and quads (`billboard::quad`,
//! 0x1E00ED0), UVs by uv_rect 0x1E00080. From update 20 to 170 two rings
//! a update grow at the attacker's node 0x6A (two kinds before update
//! 106, two after); from 170 to 295 every 10th update a column of 20
//! slots starts there, each live slot spawning a blue and a pink cross of
//! four quads a update while its radius shrinks by 1. Returns 1 on update
//! 174, 4 on 325, 2 on 315, 6 on 185..309, 8 once the 340-update timer
//! +0xDF20 has run out.
use crate::b425::spawn;
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 500;
/// The step functions a task holds (+0x44), as the record's addresses.
const RING_A: u32 = 0x01E0_1510;
const BLUE: u32 = 0x01E0_1720;
const PINK: u32 = 0x01E0_1870;
const RING_D: u32 = 0x01E0_19A0;
const RING_E: u32 = 0x01E0_1BA0;
const RING_F: u32 = 0x01E0_1D40;
/// Skipped by the entry's step loop (never spawned) [F].
const IDLE: u32 = 0x01E0_1ED0;
/// The return address of the entry's `jalr` into a step (0x1E025A0 + 8).
const TASK_RA: u64 = 0x01E0_25A8;
const TIMER: u32 = D + 0xDF20;
const NODE: u32 = D + 0xDFA4;
const FACING: u32 = D + 0xE064;
const COUNTER: u32 = D + 0xE068;
const BYTES: u32 = D + 0xE06A;
const BYTES2: u32 = D + 0xE16A;
const SINES: u32 = D + 0xE26C;
const SLOT_ON: u32 = D + 0xE030;
const SLOT_R: u32 = D + 0xDF44;
const SLOT_Y: u32 = D + 0xDD94;
const SLOT_A: u32 = D + 0xE008;
const SLOT_NEXT: u32 = D + 0xE058;

/// The entry 0x1E021B0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xE0);
    fx.mem.write_u32(D + 0xDF2C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xE0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xE3E0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xE066, param);
    fx.mem.set32(TIMER, 0x154);
    for (at, node, clear) in [(0xDFA4u32, 0x6A, 1), (0xDFA6, 0x34, 0), (0xDFA8, 0x35, 0), (0xDFAA, 0x30, 0), (0xDFAC, 0x2F, 0), (0xDFAE, 0x2C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xDFB4, r);
    for k in 0..2u32 {
        let t = fx.texture(4, 0xE + k as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k, t);
    }
    for k in 0..4u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xDF00 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for table in [BYTES, BYTES2] {
        for k in 0..0x100u32 {
            let v = fx.rand();
            let (_, h) = fx.div(v, 0x1E);
            fx.mem.set8(table + k, h - 0xF);
        }
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand();
        fx.mem.set16(D + 0xDFC4 + 2 * k, v % 8);
    }
    for k in 0..0x1Eu32 {
        fx.mem.write_u32(SLOT_R + 4 * k, 0x4220_0000);
    }
    for k in 0..0x14u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v >> 4, 0x168);
        fx.mem.set16(SLOT_A + 2 * k, h);
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
        if f != 0 && f != IDLE {
            let t = slot + 0x2A0;
            match f {
                RING_A => ring_a(fx, t, 0x30),
                BLUE => cross(fx, t, false),
                PINK => cross(fx, t, true),
                RING_D => ring_d(fx, t, 0x30),
                RING_E => ring_a(fx, t, 0x20),
                RING_F => ring_d(fx, t, 0x20),
                _ => panic!("b530: unknown task step {f:#x}"),
            }
        }
    }
    for k in 0..TASKS {
        let s = D + 0x70 * k;
        if fx.mem.i16(s + 0x2F8) == 0 {
            continue;
        }
        let t = s + 0x2A0;
        let ring = |fx: &mut Fx, tex: u32, local: u32, site: u32| {
            let w = fx.mem.read_u32(D + tex);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + local);
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand_from(k, site + 8);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
        };
        if fx.mem.read_u32(t + 0x44) == RING_A {
            ring(fx, 0xDF00, 0x80, 0x01E0_264C);
        }
        if fx.mem.read_u32(t + 0x44) == BLUE {
            let w = fx.mem.read_u32(D + 0xDF04);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            let u = if sub < 2 { sub } else { sub - 2 };
            fx.mem.set16(D + 0x94, u << 6);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x3F);
            fx.mem.set16(D + 0x9A, 0x3F);
            fx.mem.set16(D + 0xBE, 5);
            quads(fx, t, sp + 0x70);
        }
        if fx.mem.read_u32(t + 0x44) == PINK {
            let w = fx.mem.read_u32(D + 0xDF08);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x90);
            fx.mem.set16(D + 0x98, 0x3F);
            fx.mem.set16(D + 0x9A, 0x3F);
            fx.mem.set16(D + 0xBE, 5);
            quads(fx, t, sp + 0x70);
        }
        if fx.mem.read_u32(t + 0x44) == RING_D {
            ring(fx, 0xDF0C, 0xA0, 0x01E0_2F20);
        }
        if fx.mem.read_u32(t + 0x44) == RING_E {
            ring(fx, 0xDF00, 0xB0, 0x01E0_2FC0);
        }
        if fx.mem.read_u32(t + 0x44) == RING_F {
            ring(fx, 0xDF0C, 0xC0, 0x01E0_3060);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xAE => 1,
        0x145 => 4,
        0x13B => 2,
        0xB9..=0x135 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell 0x14 / +0x56 (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// Four quads at the target's position (z 10 toward the facing) pushed
/// out by radius +0x48 at angles +0x5A, +90, +180, +270 degrees, lifted
/// by +0x34.
fn quads(fx: &mut Fx, t: u32, p: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(p + 4 * k, w);
    }
    for turn in [0, 0x5A, 0xB4, 0x10E] {
        let h = fx.target_handle();
        fx.unit_position(h, p);
        let z = fx.mem.f(p + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 10.0) } else { ee::add(z, 10.0) };
        fx.mem.setf(p + 8, z);
        let a = (fx.mem.i16(t + 0x5A) + turn) as i16 as i32;
        let c = l2::sin_deg(fx, SINES, a, 1);
        let z = ee::add(fx.mem.f(p + 8), ee::mul(fx.mem.f(t + 0x48), c));
        fx.mem.setf(p + 8, z);
        let a = (fx.mem.i16(t + 0x5A) + turn) as i16 as i32;
        let s = l2::sin_deg(fx, SINES, a, 0);
        let x = ee::add(fx.mem.f(p), ee::mul(fx.mem.f(t + 0x48), s));
        fx.mem.setf(p, x);
        let y = ee::add(fx.mem.f(p + 4), fx.mem.f(t + 0x34));
        fx.mem.setf(p + 4, y);
        billboard::quad(fx, uv::uv_rect, D, t, 0x10, p);
    }
}

/// 0x1E01190(D, 0): the update counter, the rings and the column slots.
/// Frame 0x60 (the entry's s4 / s3 = 0, s2 = D, s1 = 0, s0 = the
/// parameter).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x50, 0x01E0_2568), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, 0), (0, p)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE);
    fx.node_position(h, slot, D + 0x250);
    let first = |ra: u32| [(0x50u32, u64::from(ra)), (0x40, u64::from(D)), (0x30, 0), (0x20, u64::from(D)), (0x10, 0), (0, p)];
    let c = fx.mem.i16(COUNTER);
    if (0x14..0xAB).contains(&c) {
        if c < 0x6A {
            spawn(fx, RING_E, &first(0x01E0_122C));
            spawn(fx, RING_F, &first(0x01E0_1240));
        } else {
            spawn(fx, RING_A, &first(0x01E0_125C));
            spawn(fx, RING_D, &first(0x01E0_1270));
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0xAA..0x128).contains(&c) && fx.div(c, 0xA).1 == 1 {
        let j = fx.mem.i16(SLOT_NEXT);
        if j < 0x14 {
            let j = j as u32;
            let on = fx.mem.i16(SLOT_ON + 2 * j) + 1;
            fx.mem.set16(SLOT_ON + 2 * j, on);
            fx.mem.write_u32(SLOT_R + 4 * j, 0x4220_0000);
            fx.mem.write_u32(SLOT_Y + 16 * j, 0x41A0_0000);
            let j = fx.mem.i16(SLOT_NEXT) + 1;
            fx.mem.set16(SLOT_NEXT, j);
            if fx.mem.i16(SLOT_NEXT) >= 0x14 {
                fx.mem.set16(SLOT_NEXT, 0);
            }
        }
    }
    for k in 0..0x14u32 {
        if fx.mem.i16(SLOT_ON + 2 * k) == 0 {
            continue;
        }
        fx.mem.set16(D + 0xE060, k as i32);
        let w = fx.mem.read_u32(SLOT_R + 4 * k);
        fx.mem.write_u32(D + 0xDF3C, w);
        let a = fx.mem.i16(SLOT_A + 2 * k);
        fx.mem.set16(D + 0xE002, a);
        let w = fx.mem.read_u32(SLOT_Y + 16 * k);
        fx.mem.write_u32(D + 0xDEF4, w);
        let regs = |ra: u32| [(0x50u32, u64::from(ra)), (0x40, u64::from(D)), (0x30, u64::from(D + 16 * k)), (0x20, u64::from(D + 4 * k)), (0x10, u64::from(D + 2 * k)), (0, u64::from(k))];
        spawn(fx, BLUE, &regs(0x01E0_1408));
        spawn(fx, PINK, &regs(0x01E0_141C));
        let r = ee::sub(fx.mem.f(SLOT_R + 4 * k), 1.0);
        fx.mem.setf(SLOT_R + 4 * k, r);
        let y = ee::sub(fx.mem.f(SLOT_Y + 16 * k), f32::from_bits(0x3F99_999A));
        fx.mem.setf(SLOT_Y + 16 * k, y);
        if fx.mem.f(SLOT_R + 4 * k) < 0.0 {
            fx.mem.set16(SLOT_ON + 2 * k, 0);
        }
        let a = fx.mem.i16(SLOT_A + 2 * k) + 0xA;
        fx.mem.set16(SLOT_A + 2 * k, a);
    }
    fx.leave(0x60);
}

/// rand % m (HI) from a step's `jal` at `site` (its s0 = the task).
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// A step's prologue (frame `bytes`): ra, the entry's s1 = 0 (when
/// saved) and s0 = the task's slot (t - 0x2A0).
fn prologue(fx: &mut Fx, t: u32, bytes: u32) {
    let sp = fx.enter(bytes);
    let slot = u64::from(t - 0x2A0);
    if bytes == 0x20 {
        fx.saved(sp, &[(0x10, TASK_RA), (0, slot)]);
    } else {
        fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, slot)]);
    }
}

/// Alpha in by 0x40 under 0xC0 for the task's life +6, then out by 14.
fn fade(fx: &mut Fx, t: u32) {
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
}

/// The cell frame +0x56 cycling 0..7.
fn cycle(fx: &mut Fx, t: u32) {
    let v = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, if v >= 8 { 0 } else { v });
}

/// 1 toward the facing from update 108.
fn drift(fx: &mut Fx, t: u32) {
    if fx.mem.i16(COUNTER) >= 0x6C {
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 1.0) } else { ee::add(z, 1.0) };
        fx.mem.setf(t + 0x18, z);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01510 (frame 0x30, drifting from update 108) / 0x1E01BA0 (frame
/// 0x20): a grey ring of quad size 1..4, radius +0x50 from 0 growing by
/// +0xE (2..5) for 90 updates, row +8 0..34, life +6 3..5.
fn ring_a(fx: &mut Fx, t: u32, frame: u32) {
    prologue(fx, t, frame);
    let a = frame == 0x30;
    let sites = if a { [0x01E0_1534u32, 0x01E0_1578, 0x01E0_1594, 0x01E0_15B4, 0x01E0_15D0] } else { [0x01E0_1BBC, 0x01E0_1C00, 0x01E0_1C1C, 0x01E0_1C3C, 0x01E0_1C58] };
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, sites[0] + 8);
        fx.mem.setf(t + 0x48, ee::from_i32(v % 4 + 1));
        fx.mem.set16(t + 0x56, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x50, 0);
        let v = rmod(fx, t, sites[1], 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, sites[2] + 8);
        fx.mem.set16(t + 0xE, v % 4 + 2);
        let v = rmod(fx, t, sites[3], 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, sites[4], 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    fade(fx, t);
    cycle(fx, t);
    let age_now = fx.mem.i16(t + 0x54);
    if age_now < 0x5A && age_now != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    if a {
        drift(fx, t);
    }
    age(fx, t);
    fx.leave(frame);
}

/// 0x1E019A0 (frame 0x30, drifting from update 108) / 0x1E01D40 (frame
/// 0x20): a grey ring of quad size 1..3, radius +0x50 10..39, row +8,
/// life +6 3..5.
fn ring_d(fx: &mut Fx, t: u32, frame: u32) {
    prologue(fx, t, frame);
    let d = frame == 0x30;
    let sites = if d {
        [0x01E0_19C4u32, 0x01E0_1A04, 0x01E0_1A24, 0x01E0_1A40, 0x01E0_1A60, 0x01E0_1A7C]
    } else {
        [0x01E0_1D5C, 0x01E0_1D9C, 0x01E0_1DBC, 0x01E0_1DD8, 0x01E0_1DF8, 0x01E0_1E14]
    };
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, sites[0], 3);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 1));
        fx.mem.set16(t + 0x56, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, sites[1], 0x1E);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, sites[2], 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, sites[3] + 8);
        fx.mem.set16(t + 0xE, v % 4 + 2);
        let v = rmod(fx, t, sites[4], 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, sites[5], 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    fade(fx, t);
    cycle(fx, t);
    if d {
        drift(fx, t);
    }
    age(fx, t);
    fx.leave(frame);
}

/// 0x1E01720 (blue: size 70 growing by 1 to 90, in by 0x60 for 13
/// updates) / 0x1E01870 (pink: size 85 growing by 1, in for 9 updates),
/// frame 0x30: a cross quad, out by 9 after, facing the camera; its age
/// held at 20 from update 296.
fn cross(fx: &mut Fx, t: u32, pink: bool) {
    prologue(fx, t, 0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        let size = if pink { 0x55 } else { 0x46 };
        fx.mem.set16(t + 8, size);
        fx.mem.set16(t + 0xA, size);
        let v = fx.rand_from(t, if pink { 0x01E0_18A4 } else { 0x01E0_1754 });
        fx.mem.set16(t + 0x56, (v >> 4) % 4);
        if pink {
            l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        } else {
            l2::rgb(fx, t + 0x60, 0xA0, 0xA0, 0xFF);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < if pink { 9 } else { 0xD } {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x60);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let w = fx.mem.i16(t + o);
            if pink || w < 0x5A {
                fx.mem.set16(t + o, w + 1);
            }
        }
    }
    let base = fx.sp.wrapping_sub(0x30);
    let ra = if pink { 0x01E0_1958 } else { 0x01E0_1820 };
    fx.saved(base, &[(0x20, ra), (0x10, u64::from(D)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
    if fx.mem.i16(COUNTER) >= 0x128 {
        fx.mem.set16(t + 0x54, 0x14);
    }
    age(fx, t);
    fx.leave(0x30);
}
