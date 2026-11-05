//! BMODULE b615 (card 390), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E022D0, data area 0x1ED0000) [F]: a pool of 150 tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step
//! +0x44 called through `jalr`) drawn with the second library: flat facing
//! rings (9c13e08cfd2f at 0x1E00D80, `quads4::ring_facing_flat`), rings of
//! quads (7369422df7f8 at 0x1E012D0, `billboard::ring_quads`) and a band
//! ring (3fb2be4df9d4 at 0x1E00920, `quads4::ring_band`), UVs by uv_rect
//! 0x1E00080. On odd updates 1..69 two grey ring columns rise from the
//! attacker's node 3 (each sways on +0x5C, drifts to the facing from age 96
//! and spreads from 141); at age 135 each throws three more rings and, six
//! times in ten, a band ring. Returns 1 on update 100, 4 on 230, 2 on 215,
//! 6 on 111..209, 8 once the 240-update timer +0xBC90 has run out.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const COLUMN: u32 = 0x01E0_19F0;
const RING: u32 = 0x01E0_1E30;
const QUADS: u32 = 0x01E0_1FC0;
const BAND: u32 = 0x01E0_21C0;
/// The return address of the entry's `jalr` into a step (0x1E025B8 + 8).
const TASK_RA: u64 = 0x01E0_25C0;
const TEX: u32 = D + 0xBC70;
const TIMER: u32 = D + 0xBC90;
const COLOUR: u32 = D + 0xBC94;
const NODES: u32 = D + 0xBCBC;
const SLOT: u32 = D + 0xBCEA;
const NEXT: u32 = D + 0xBCEC;
const FACING: u32 = D + 0xBCEE;
const COUNTER: u32 = D + 0xBCF2;
const BYTES: u32 = D + 0xBCF4;
const BYTES2: u32 = D + 0xBDF4;
const SINES: u32 = D + 0xBEF4;

/// The entry 0x1E022D0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    fx.mem.write_u32(D + 0xBC9C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xC060);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xBCF0, param);
    fx.mem.set32(TIMER, 0xF0);
    for (k, node) in [3, 0x1C, 0xD].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xBCCA, r);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x298 + 4 * (k as u32 - 0xE), t);
    }
    let t = fx.texture(5, 0);
    fx.mem.write_u32(TEX, t);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // The entry's s0 is D here (rand's frame).
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_248C);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_24E4);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_253C);
        let (_, h) = fx.div(v, 0x28);
        fx.mem.set8(BYTES2 + k, h - 0x14);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 => {}
            COLUMN => column(fx, t),
            RING => ring(fx, t, RING, TASK_RA, 0x4120_0000),
            QUADS => quads(fx, t, QUADS, TASK_RA),
            BAND => band(fx, t, BAND, TASK_RA),
            _ => panic!("b615: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        for (f, r) in [(COLUMN, 0x60u32), (RING, 0x70)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + r, 0x14, sub);
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 6);
            let o = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, o);
            let x = fx.mem.read_u32(t + 0x10);
            fx.mem.write_u32(D + 0x20, x);
            if f == COLUMN {
                let y = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
                fx.mem.setf(D + 0x24, y);
            } else {
                let y = fx.mem.read_u32(t + 0x14);
                fx.mem.write_u32(D + 0x24, y);
            }
            let z = fx.mem.read_u32(t + 0x18);
            fx.mem.write_u32(D + 0x28, z);
            quads4::ring_facing_flat(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == QUADS {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x9B, sub);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 1);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if fx.mem.read_u32(t + 0x44) == BAND {
            for k in 0..4 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x50 + 4 * k, w);
            }
            quads4::ring_band(fx, D, t, 0x21, sp + 0x50);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x64 => 1,
        0xE6 => 4,
        0xD7 => 2,
        0x6F..=0xD1 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
pub(crate) fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// rand % m (HI) from a step's `jal` at `site` (rand's frame: s0 = `t`).
pub(crate) fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// 0x1E01880(D, 0): the update counter and the emitter (frame 0x40: ra,
/// the entry's s2 = the parameter, s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_2590), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    if c > 0 && c < 0x47 && c % 2 == 1 {
        for k in 0..2u64 {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES);
            fx.node_position(h, slot, D + 0x250);
            let s = fx.mem.i16(NEXT);
            fx.mem.set16(SLOT, s);
            spawn(fx, COLUMN, &[(0x50, 0x01E0_1964), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, 2), (0, k)]);
            let s = fx.mem.i16(NEXT) + 1;
            fx.mem.set16(NEXT, s);
            if fx.mem.i16(NEXT) >= 0x78 {
                fx.mem.set16(NEXT, 0);
            }
        }
    }
    fx.leave(0x40);
}

/// 0x1E02910(D, step, 0): the first free of the 150 tasks cleared and set
/// off at D+0x250 (as i16 and its xyz at +0x10) with the colour D+0xBC94
/// (+0x60, +0x64), +0x50 = D+0xBCEA; 1 when one was free. `saves`: its
/// prologue's.
fn spawn(fx: &mut Fx, step: u32, saves: &[(u32, u64)]) -> i32 {
    spawn_at(fx, step, COLOUR, SLOT, saves)
}

/// [`spawn`]'s body with the layout's colour and slot (b547's 0x1E02B00:
/// D+0x4494 / D+0x44EA).
pub(crate) fn spawn_at(fx: &mut Fx, step: u32, colour: u32, slot: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for o in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(colour), fx.mem.read_u8(colour + 1), fx.mem.read_u8(colour + 2));
                l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(colour + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let s = fx.mem.i16(slot);
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

/// 0x1E019F0 (D, task): a ring column (frame 0x40): size +0x48 10, grey,
/// cell row 0..6, angles +0x5A / +0x5C random, orientation +0xA 0..7, the
/// spawn's slot kept in +0xC, its radius +0x50 growing by 15 to +0x52
/// 100..449, row 0..34; in by 0x20 under 0x80 for 147 updates, then out
/// by 8. At age 135 three rings (and, for rand % 10 < 6, a band ring at a
/// point +-15 off) start at it. +0x34 = 3 sin(+0x5C) (its sway, +0x5C
/// stepping by 6); from age 96 to 130 it moves 5 a update to the facing and
/// sinks by 1 while above the target's height - 10; from age 141 it turns
/// by 5, widens by +0xE 10..29, rises by 1 and grows by 1.
fn column(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    let f = COLUMN;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        let v = rmod(fx, t, f + 0x28, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x54, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0x70, 0x168);
        fx.mem.set16(t + 0x5C, v);
        let v = fx.rand_from(t, f + 0x94);
        fx.mem.set16(t + 0xA, v % 8);
        let w = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, w);
        fx.mem.set16(t + 0x50, 0);
        let v = rmod(fx, t, f + 0xB4, 0x15E);
        fx.mem.set16(t + 0x52, v + 0x64);
        let v = rmod(fx, t, f + 0xD4, 0x14);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = rmod(fx, t, f + 0xF4, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0x110, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x93 {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x20);
        }
        let r = fx.mem.i16(t + 0x50);
        if r < fx.mem.i16(t + 0x52) {
            fx.mem.set16(t + 0x50, r + 0xF);
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xA {
            fx.mem.write_u8(t + 0x63, a - 8);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if fx.mem.i16(t + 0x54) == 0x87 {
        for k in 0..4 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        // The spawn's prologue saves the step's s0 = t / s1 = D and the
        // entry's s2 = the slot base / s3 = its index (s4 0).
        let k = (t - D - 0x2A0) / 0x70;
        let saves = |ra: u32| [(0x50, u64::from(ra)), (0x40, 0), (0x30, u64::from(k)), (0x20, u64::from(t - 0x2A0)), (0x10, u64::from(D)), (0, u64::from(t))];
        for ra in [0x01E0_1BD8u32, 0x01E0_1BEC, 0x01E0_1C00] {
            spawn(fx, RING, &saves(ra));
        }
        for (o, site) in [(0x250u32, 0x01E0_1C00u32), (0x254, 0x01E0_1C34), (0x258, 0x01E0_1C68)] {
            let r = rmod(fx, t, site, 0x1E);
            let v = ee::add(fx.mem.f(D + o), ee::from_i32(r - 0xF));
            fx.mem.setf(D + o, v);
        }
        if rmod(fx, t, 0x01E0_1C9C, 0xA) < 6 {
            spawn(fx, BAND, &saves(0x01E0_1CD8));
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x30);
    let a = fx.mem.i16(t + 0x5C);
    let s = l2::sin_deg(fx, SINES, a, 1);
    fx.mem.setf(t + 0x34, ee::mul(3.0, s));
    let age = fx.mem.i16(t + 0x54);
    if (0x60..0x83).contains(&age) {
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(t + 0x18, z);
        let lim = ee::sub(fx.mem.f(sp + 0x34), 10.0);
        let y = fx.mem.f(t + 0x14);
        if lim < y {
            fx.mem.setf(t + 0x14, ee::sub(y, 1.0));
        }
    }
    if fx.mem.i16(t + 0x54) >= 0x8D {
        let a = fx.mem.i16(t + 0x5A) + 5;
        fx.mem.set16(t + 0x5A, a);
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
        let y = ee::add(fx.mem.f(t + 0x14), 1.0);
        fx.mem.setf(t + 0x14, y);
        let g = ee::add(fx.mem.f(t + 0x48), 1.0);
        fx.mem.setf(t + 0x48, g);
    }
    let a = fx.mem.i16(t + 0x5C) + 6;
    fx.mem.set16(t + 0x5C, a);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x40);
}

/// The step frame (0x20: ra, the entry's s0 = D).
fn frame20(fx: &mut Fx, ra: u64) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, ra), (0, u64::from(D))]);
}

/// 0x1E01E30 (D, task) at `f` (b547's 0x1E02070 with `size` 20; `ra` the
/// entry's return address): a ring: size `size` (10), grey, cell row 0..6, a random
/// angle, orientation 0..7, the spawn's slot in +0xC, radius 10..59
/// growing by 16..45, row 0..34, life 3..7; alpha 0x80, then out by 8.
pub(crate) fn ring(fx: &mut Fx, t: u32, f: u32, ra: u64, size: u32) {
    frame20(fx, ra);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, size);
        let v = rmod(fx, t, f + 0x20, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x4C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, f + 0x70);
        fx.mem.set16(t + 0xA, v % 8);
        let w = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, w);
        let v = rmod(fx, t, f + 0x8C, 0x32);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, f + 0xAC, 0x1E);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, f + 0xCC, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xE8, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xA {
            fx.mem.write_u8(t + 0x63, a - 8);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01FC0 (D, task) at `f` (a75a0f66bc56, also b547's 0x1E02200): a
/// ring of quads (layout 1): size +0x48 10..19,
/// cell row 0..3, yellow (0x80, 0x80, 0), random angles +0x20..+0x28,
/// +0x52 counting from 1, radius +0x50 150..299 growing by 1..3, angle
/// +0x5A, row 1..8, life 2..4; alpha 0x60, then out by 11.
pub(crate) fn quads(fx: &mut Fx, t: u32, f: u32, ra: u64) {
    frame20(fx, ra);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x1C, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = fx.rand_from(t, f + 0x50);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0);
        for (o, site) in [(0x20u32, 0x7Cu32), (0x24, 0x98), (0x28, 0xB4)] {
            let v = rmod(fx, t, f + site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, f + 0xD8, 0x96);
        fx.mem.set16(t + 0x50, v + 0x96);
        let v = rmod(fx, t, f + 0xF8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0x114, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, f + 0x13C);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, f + 0x154, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xD {
            fx.mem.write_u8(t + 0x63, a - 0xB);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let c = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, c);
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E021C0 (D, task) at `f` (also b547's 0x1E02400): the band ring: inner radius +0x48 150 growing by
/// 1, light grey inside, blue (8, 8, 0x80) outside; in by 0x40 under 0xC0
/// for 4 updates, out by 9; turned by the camera's angles (its first +
/// 0x4000).
pub(crate) fn band(fx: &mut Fx, t: u32, f: u32, ra: u64) {
    frame20(fx, ra);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4316_0000);
        l2::rgb(fx, t + 0x60, 0xCF, 0xCF, 0xCF);
        l2::rgb(fx, t + 0x64, 8, 8, 0x80);
        let v = rmod(fx, t, f + 0x4C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 4 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let r = ee::add(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, r);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), f + 0xE0);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}
