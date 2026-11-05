//! BMODULE b425 (card 632), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E020C0, data area 0x1ED0000) [F]: a task pool of 500 (0x70 bytes at +0x2A0, alive +0x58, step
//! +0x44 as a record address) drawn with the second library: facing rings
//! (`billboard::ring_facing`, 98b02c86a827 at 0x1E00BE0) and quads
//! (`billboard::quad`, add5d81dd3fe at 0x1E00920), UVs by uv_rect
//! 0x1E00080. From update 68 to 250 one or two (odd / even updates) yellow
//! motes a update and one dark ring leave the attacker's node 0x1E,
//! drifting toward the target; from 130 to 295 every 15th update a new
//! column of three quads (one of 20 slots, its radius and angle stepping)
//! rises there. Returns 1 on update 111, 4 on 325, 5 on 315, 6 on
//! 122..309, 8 once the 340-update timer +0xDF20 has run out.
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 500;
/// The step functions a task holds (+0x44), as the record's addresses.
const MOTE: u32 = 0x01E0_1660;
const COLUMN: u32 = 0x01E0_1B10;
const RING: u32 = 0x01E0_1C80;
const IDLE: u32 = 0x01E0_20B0;
/// The return address of the entry's `jalr` into a step (0x1E02508 + 8).
const TASK_RA: u64 = 0x01E0_2510;
const TIMER: u32 = D + 0xDF20;
const COLOUR: u32 = D + 0xDF24;
const NODE: u32 = D + 0xDFA4;
const FACING: u32 = D + 0xE064;
const COUNTER: u32 = D + 0xE068;
const BYTES: u32 = D + 0xE06A;
const BYTES2: u32 = D + 0xE16A;
/// The degree sine table 0x108910 reads.
const SINES: u32 = D + 0xE26C;
/// The column slots: alive (i16 +0xE030), radius (f32 +0xDF44), y (f32,
/// stride 16, +0xDD94), angle (i16 +0xE008); next slot +0xE058.
const SLOT_ON: u32 = D + 0xE030;
const SLOT_R: u32 = D + 0xDF44;
const SLOT_Y: u32 = D + 0xDD94;
const SLOT_A: u32 = D + 0xE008;
const SLOT_NEXT: u32 = D + 0xE058;

/// The entry 0x1E020C0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0xDF2C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xE3E0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xE066, param);
    fx.mem.set32(TIMER, 0x154);
    for (at, node, clear) in [(0xDFA4u32, 0x1E, 1), (0xDFA6, 0x34, 0), (0xDFA8, 0x35, 0), (0xDFAA, 0x30, 0), (0xDFAC, 0x2F, 0), (0xDFAE, 0x2C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xDFB4, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xDF00 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES2 + k, h - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x14);
        fx.mem.set8(BYTES2 + k, h - 0xA);
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
                MOTE => mote(fx, t),
                COLUMN => column(fx, t),
                RING => ring(fx, t),
                _ => panic!("b425: unknown task step {f:#x}"),
            }
        }
    }
    for k in 0..TASKS {
        let s = D + 0x70 * k;
        if fx.mem.i16(s + 0x2F8) == 0 {
            continue;
        }
        let t = s + 0x2A0;
        if fx.mem.read_u32(t + 0x44) == MOTE {
            cell(fx, t, sp + 0x80);
            sub_uv(fx, t);
            if fx.mem.i16(t + 6) == 0 {
                cell(fx, t, sp + 0x90);
                let w = fx.mem.read_u32(D + 0xDF08);
                fx.mem.write_u32(D + 0x88, w);
            } else {
                let w = fx.mem.read_u32(D + 0xDF04);
                fx.mem.write_u32(D + 0x88, w);
            }
            fx.mem.set16(D + 0xC0, 5);
            fx.mem.set16(D + 0xB8, 1);
            centre(fx, t);
            fx.mem.set16(D + 0xBE, 0);
            billboard::ring_facing(fx, uv::uv_rect, D, t, 0x10, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == COLUMN {
            let w = fx.mem.read_u32(D + 0xDF04);
            fx.mem.write_u32(D + 0x88, w);
            sub_uv(fx, t);
            fx.mem.set16(D + 0xBE, 5);
            let p = sp + 0x70;
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(p + 4 * k, w);
            }
            for turn in [0, 0x78, 0xF0] {
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
        if fx.mem.read_u32(t + 0x44) == RING {
            cell(fx, t, sp + 0xA0);
            let w = fx.mem.read_u32(D + 0xDF08);
            fx.mem.write_u32(D + 0x88, w);
            let o = fx.mem.i16(t + 6);
            fx.mem.set16(D + 0xC0, o);
            fx.mem.set16(D + 0xB8, 3);
            centre(fx, t);
            billboard::ring_facing(fx, uv::uv_rect, D, t, 0x20, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x6F => 1,
        0x145 => 4,
        0x13B => 5,
        0x7A..=0x135 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell 0x14 / the task's +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// The UV rect from +0x56 (cells of 64 along u, the frames 2.. one row
/// on, v 0 either way [F]) and size 0x3F.
fn sub_uv(fx: &mut Fx, t: u32) {
    let sub = fx.mem.i16(t + 0x56);
    let u = if sub < 2 { sub } else { sub - 2 };
    fx.mem.set16(D + 0x94, u << 6);
    fx.mem.set16(D + 0x96, 0);
    fx.mem.set16(D + 0x98, 0x3F);
    fx.mem.set16(D + 0x9A, 0x3F);
}

/// The task's position into the ring centre D+0x20.
fn centre(fx: &mut Fx, t: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(D + 0x20 + 4 * k, w);
    }
}

/// 0x1E01150(D, 0): the update counter, the motes / rings and the column
/// slots. Frame 0x70 (the entry's s5..s3 = 0, s2 = the parameter, s1 =
/// D, s0 = 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x70);
    fx.saved(sp, &[(0x60, 0x01E0_24D0), (0x50, 0), (0x40, 0), (0x30, 0), (0x20, param as i64 as u64), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let node = |fx: &mut Fx| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE);
        fx.node_position(h, slot, D + 0x250);
    };
    node(fx);
    let mut s0: u64 = 0;
    let c = fx.mem.i16(COUNTER);
    if (0x44..0xFB).contains(&c) {
        let n = if c % 2 == 1 { 1 } else { 2 };
        let lift = |fx: &mut Fx| {
            let y = ee::sub(fx.mem.f(D + 0x254), 3.5);
            fx.mem.setf(D + 0x254, y);
            let z = fx.mem.f(D + 0x258);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 4.0) } else { ee::add(z, 4.0) };
            fx.mem.setf(D + 0x258, z);
        };
        for k in 0..n {
            node(fx);
            lift(fx);
            spawn(fx, MOTE, &[(0x50, 0x01E0_129C), (0x40, 0), (0x30, 0), (0x20, param as i64 as u64), (0x10, n as u64), (0, k as u64)]);
        }
        node(fx);
        lift(fx);
        spawn(fx, RING, &[(0x50, 0x01E0_1354), (0x40, 0), (0x30, 0), (0x20, param as i64 as u64), (0x10, n as u64), (0, 0)]);
        s0 = 1;
    }
    let c = fx.mem.i16(COUNTER);
    if (0x82..0x128).contains(&c) && fx.div(c, 0xF).1 == 1 {
        let j = fx.mem.i16(SLOT_NEXT);
        if j < 0x14 {
            let j = j as u32;
            let on = fx.mem.i16(SLOT_ON + 2 * j) + 1;
            fx.mem.set16(SLOT_ON + 2 * j, on);
            fx.mem.write_u32(SLOT_Y + 16 * j, 0xC170_0000);
            let v = fx.rand_from(s0 as u32, 0x01E0_13F4);
            let r = ee::add(10.0, ee::from_i32((v >> 4) % 8));
            fx.mem.setf(SLOT_R + 4 * j, r);
            let v = fx.rand_from(s0 as u32, 0x01E0_1468);
            let (_, h) = fx.div(v >> 4, 0x168);
            fx.mem.set16(SLOT_A + 2 * j, h);
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
        spawn(fx, COLUMN, &[(0x50, 0x01E0_1568), (0x40, u64::from(k)), (0x30, u64::from(k)), (0x20, u64::from(D + 16 * k)), (0x10, u64::from(D + 4 * k)), (0, u64::from(D + 2 * k))]);
        let y = ee::add(fx.mem.f(SLOT_Y + 16 * k), 1.5);
        fx.mem.setf(SLOT_Y + 16 * k, y);
        if !(y <= 18.0) {
            fx.mem.set16(SLOT_ON + 2 * k, 0);
        }
        let a = fx.mem.i16(SLOT_A + 2 * k);
        fx.mem.set16(SLOT_A + 2 * k, if k % 2 == 0 { a + 0xA } else { a - 0xA });
    }
    fx.leave(0x70);
}

/// 0x1E02B60(D, step, 0): the first free task cleared and set off at
/// D+0x250 with the colour +0xDF24 twice, slot +0xE060 (+0x50 / +0x52),
/// angle +0xE002 (+0x5A), lift +0xDEF4 (+0x34), radius +0xDF3C (+0x48).
/// `saves`: its prologue's. Also b530's 0x1E031A0 (the same body).
pub(crate) fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
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
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let s = fx.mem.i16(D + 0xE060);
            fx.mem.set16(t + 0x50, s);
            fx.mem.set16(t + 0x52, s);
            let a = fx.mem.i16(D + 0xE002);
            fx.mem.set16(t + 0x5A, a);
            let w = fx.mem.read_u32(D + 0xDEF4);
            fx.mem.write_u32(t + 0x34, w);
            let w = fx.mem.read_u32(D + 0xDF3C);
            fx.mem.write_u32(t + 0x48, w);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand % m (HI) from a step's `jal` at `site` (its s0 = the task).
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// (rand >> 4) % m (no division) from `site`.
fn r4(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    (v >> 4) % m
}

/// The mote / ring launch shared by 0x1E01660 and 0x1E01C80 from the
/// spins on: angles +0x5A / +0x5C, flip +0xA, the slot moved to +0xC,
/// +0x52 by `period`, size +0xE 10..29, row +8, drift +0x30 / 50, +0x34 /
/// 60, +0x38 / 60 (each rand % 40 - 20), +0x3C the target's height
/// difference / 60. `sites`: the `jal`s of the seven rands from +0x5A on.
fn launch(fx: &mut Fx, t: u32, sites: [u32; 9], period: impl Fn(&mut Fx, u32, u32) -> i32, sp: u32) {
    let v = rmod(fx, t, sites[0], 0x168);
    fx.mem.set16(t + 0x5A, v);
    let v = rmod(fx, t, sites[1], 0x168);
    fx.mem.set16(t + 0x5C, v);
    let v = fx.rand_from(t, sites[2] + 8);
    fx.mem.set16(t + 0xA, if v % 2 != 0 { 2 } else { 0 });
    let s = fx.mem.i16(t + 0x50);
    fx.mem.set16(t + 0xC, s);
    fx.mem.set16(t + 0x50, 0);
    let p = period(fx, t, sites[3]);
    fx.mem.set16(t + 0x52, p);
    let v = rmod(fx, t, sites[4], 0x14);
    fx.mem.set16(t + 0xE, v + 0xA);
    let v = rmod(fx, t, sites[5], 0x23);
    fx.mem.set16(t + 8, v);
    for (o, site) in [(0x30u32, sites[6]), (0x34, sites[7]), (0x38, sites[8])] {
        let v = rmod(fx, t, site, 0x28);
        fx.mem.setf(t + o, ee::from_i32(v - 0x14));
    }
    let q = ee::div(fx.mem.f(t + 0x30), 50.0);
    fx.mem.setf(t + 0x30, q);
    for o in [0x34u32, 0x38] {
        let q = ee::div(fx.mem.f(t + o), 60.0);
        fx.mem.setf(t + o, q);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x30);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE);
    fx.node_position(h, slot, sp + 0x40);
    let d = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48)), 60.0);
    fx.mem.setf(t + 0x3C, d);
    fx.mem.set16(t + 0x58, 2);
}

/// The drift both share: from age 11 to 69 x += 1.4 +0x30, z += +0x38.
fn drift(fx: &mut Fx, t: u32) {
    let age = fx.mem.i16(t + 0x54);
    if (0xB..0x46).contains(&age) {
        let x = ee::add(fx.mem.f(t + 0x10), ee::mul(f32::from_bits(0x3FB3_3333), fx.mem.f(t + 0x30)));
        fx.mem.setf(t + 0x10, x);
        let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x38));
        fx.mem.setf(t + 0x18, z);
    }
}

/// The rise both share (not on the first update): z += +0x3C, size +0x48
/// += 1.5, y down by 2 while above 35.
fn rise(fx: &mut Fx, t: u32) -> bool {
    if fx.mem.i16(t + 0x54) == 0 {
        return false;
    }
    let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x3C));
    fx.mem.setf(t + 0x18, z);
    let s = ee::add(fx.mem.f(t + 0x48), 1.5);
    fx.mem.setf(t + 0x48, s);
    let y = fx.mem.f(t + 0x14);
    if !(y <= 35.0) {
        fx.mem.setf(t + 0x14, ee::sub(y, 2.0));
    }
    true
}

/// The age: held at `hold` from update 0x123 / 0x128, counted.
fn age(fx: &mut Fx, t: u32, from: i32, hold: i32) {
    if fx.mem.i16(COUNTER) >= from {
        fx.mem.set16(t + 0x54, hold);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// Grow +0x48 by 1.5 below 45.
fn grow(fx: &mut Fx, t: u32) {
    let s = fx.mem.f(t + 0x48);
    if s < 45.0 {
        fx.mem.setf(t + 0x48, ee::add(s, 1.5));
    }
}

/// Alpha: below age 75 up by `up` under `cap`, else down by `down` (the
/// task ends at or below `floor`).
fn fade(fx: &mut Fx, t: u32, until: i32, cap: u8, up: u8, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < until {
        if a < cap {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(up));
        }
    } else if a >= floor {
        fx.mem.write_u8(t + 0x63, a - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E01660: a yellow mote ring (frame 0x60): size 25 growing to 45,
/// cell frame random, its cell row flickering (+6 = 1 always [F]).
fn mote(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41C8_0000);
        let v = r4(fx, t, 0x01E0_1688, 8);
        fx.mem.set16(t + 0x56, v);
        fx.mem.set16(t + 6, 1);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x80);
        launch(fx, t, [0x01E0_16C8, 0x01E0_16E4, 0x01E0_1700, 0x01E0_173C, 0x01E0_175C, 0x01E0_177C, 0x01E0_1798, 0x01E0_17C4, 0x01E0_17F0], |fx, t, site| rmod(fx, t, site, 0x15E) + 0x64, sp);
    }
    grow(fx, t);
    fade(fx, t, 0x4B, 0xC0, 0xC0, 0xD, 0xB);
    drift(fx, t);
    if fx.mem.i16(t + 6) == 1 {
        let v = r4(fx, t, 0x01E0_1984, 4);
        fx.mem.set16(t + 0x56, v);
    }
    if fx.mem.i16(t + 6) == 0 {
        let age = fx.mem.i16(t + 0x54);
        if fx.mem.i16(t + 0x5A) == 0 {
            if age % 4 == 1 {
                let v = fx.mem.i16(t + 0x56) + 1;
                fx.mem.set16(t + 0x56, v);
            }
            if fx.mem.i16(t + 0x56) >= 8 {
                fx.mem.set16(t + 0x5A, 1);
            }
        } else {
            if age % 4 == 1 {
                let v = fx.mem.i16(t + 0x56) - 1;
                fx.mem.set16(t + 0x56, v);
            }
            if fx.mem.i16(t + 0x56) <= 0 {
                fx.mem.set16(t + 0x5A, 0);
            }
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    rise(fx, t);
    age(fx, t, 0x123, 0x64);
    fx.leave(0x60);
}

/// 0x1E01B10: a column quad (frame 0x30): size 130 growing by 3 to 160,
/// fading in by 0x60 for 5 updates, then out by 9; a random cell frame and
/// the camera's angles every update.
fn column(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x82);
        fx.mem.set16(t + 0xA, 0x82);
        let v = r4(fx, t, 0x01E0_1B3C, 4);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    fade(fx, t, 5, 0xC0, 0x60, 0xB, 9);
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let w = fx.mem.i16(t + o);
            if w < 0xA0 {
                fx.mem.set16(t + o, w + 3);
            }
        }
    }
    let v = r4(fx, t, 0x01E0_1C04, 4);
    fx.mem.set16(t + 0x56, v);
    let base = sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, 0x01E0_1C30), (0x10, u64::from(D)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
    age(fx, t, 0x128, 0x14);
    fx.leave(0x30);
}

/// 0x1E01C80: a dark ring (frame 0x50): size 25 growing to 45, radius
/// +0x50 growing by 5, its cell frame stepping every +0x52 (10..24)
/// updates.
fn ring(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41C8_0000);
        let v = r4(fx, t, 0x01E0_1CA8, 8);
        fx.mem.set16(t + 0x56, v);
        let v = r4(fx, t, 0x01E0_1CC8, 4);
        fx.mem.set16(t + 6, v);
        l2::rgb(fx, t + 0x60, 0x30, 0x30, 0x10);
        launch(fx, t, [0x01E0_1D00, 0x01E0_1D1C, 0x01E0_1D38, 0x01E0_1D74, 0x01E0_1D98, 0x01E0_1DB8, 0x01E0_1DD4, 0x01E0_1E00, 0x01E0_1E2C], |fx, t, site| {
            let v = fx.rand_from(t, site + 8);
            fx.div(v >> 4, 0xF).1 + 0xA
        }, sp);
    }
    grow(fx, t);
    fade(fx, t, 0x4B, 0x80, 0x80, 6, 4);
    drift(fx, t);
    let age_now = fx.mem.i16(t + 0x54);
    let period = fx.mem.i16(t + 0x52);
    if fx.div(age_now, period).1 == 1 {
        let v = fx.mem.i16(t + 0x56) + 1;
        fx.mem.set16(t + 0x56, v);
    }
    if fx.mem.i16(t + 0x56) >= 8 {
        fx.mem.set16(t + 0x56, 0);
    }
    if rise(fx, t) {
        let r = fx.mem.i16(t + 0x50) + 5;
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t, 0x123, 0x64);
    fx.leave(0x50);
}
