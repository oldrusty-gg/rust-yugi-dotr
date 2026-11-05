//! BMODULE b053 (card 399), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E010F0) [F]. The entry
//! takes (D, param). b159's family (card 170) with a second pool: 15
//! linked ring records of 0x7C bytes at D+0x110 (alive +0x68, stepped by
//! 0x1E00620, drawn by 0x1E019C0, added by 0x1E01730) besides 200 tasks of
//! 0x40 bytes at D+0x854 (alive +0x36, step +0x24, spawned by 0x1E01890:
//! b159's spawn with +0x30 = D+0x3B5A and +0x3A = D+0x3B54), the sines
//! D+0x3B64, timer D+0x3B00 = 140, the counter D+0x3B5E, the attacker's
//! node 1 (D+0x3B48).
//!
//! The rings (a tornado): each record a band between heights +4 and +0x14
//! (+0x14 rising by 2 a update until 5 above +4, +4 by 2 up to 60, then
//! the record ends when +0x14 has passed it), radii 10 + height / 2
//! (+0x6A / +0x6C), the angle +0x72 down by 12 a update; while alive 1
//! with +0x14 above 0 it hands its top (+0x6C, +0x14, angle - 15) to a new
//! record linked after it (while the timer is at least 15). Drawn as four
//! 90-degree quads (sprite 0x96, texture D+0x10C, the scale D+0xC0, at the
//! attacker's node 0 - 3 in y) from its own top ring to the previous
//! record's (or its own bottom ring for the first).
//!
//! Tasks: a frame flash 0x1E00870 (stepped inline in the entry: in by
//! 0x40 to 0x80, out by 9, colour (0xF0, 0x80, 0x40), a full-window
//! gouraud quad), red streaks 0x1E008F0 (a vertical line from y +2 of
//! length 10 turned by random angles at the target, drawn twice with the
//! angles negated, far Z, by 0x1E00080), magenta sparks 0x1E009F0 (b130's
//! 0.1 sprite 0x1E01E30: sprite 0x9B, texture D+0x10C), dark rings
//! 0x1E00BD0 (0x1E01FE0: eighteen quads between +0x38 - 1 and +0x38,
//! sprites 0x8D / 0x8C, texture D+0x108), b159-style motes 0x1E00D00
//! (b159's sprite 0x1E022F0: 0x14, D+0xD8) and rising discs 0x1E00F20 at
//! the attacker's node (0x1E02490: twelve quads between +0x38 - 2 and
//! +0x38 scaled (0.08, 1, 0.08), each twice: sprite 0x8F then 0x8C).
//!
//! The emitter 0x1E00220: the attacker's yaw / 180 - 180 (D+0x3B50) and
//! -20 cos of it (s0); on 20 the first ring; on 0x69..0x6C five dark
//! rings at the node (z + s0); from 0x1D while the timer is at least 21
//! three motes a update, and a disc every third update; on 0x6A 65 sparks
//! (z + s0). The entry: on 0x69 the flash, on 0x68..0x6B five streaks a
//! update. From 0x15 D+0xC0 / D+0xC8 creep up by 0.01 below 0.5, D+0xC4 =
//! 0.1. Returns 1 on 0x60, 4 on 0x75, 2 on 0x6B, 8 once the timer has run
//! out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib::ALPHA_ADD;
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const RINGS: u32 = 0xF;
const RPOOL: u32 = D + 0x110;
const TASKS: u32 = 0xC8;
const POOL: u32 = D + 0x854;
const POS: u32 = D + 0x80;
const TIMER: u32 = D + 0x3B00;
const COLOUR: u32 = D + 0x3B04;
const OTW: u32 = D + 0x3B0C;
const LAST: u32 = D + 0x3B10;
const TOP: u32 = D + 0x3B1C;
const MEAN: u32 = D + 0x3B34;
const NODE: u32 = D + 0x3B48;
const YAW: u32 = D + 0x3B50;
const ANGLE2: u32 = D + 0x3B54;
const ANGLE: u32 = D + 0x3B5A;
const COUNTER: u32 = D + 0x3B5E;
const TAB: u32 = D + 0x3B64;

const FLASH: u32 = 0x01E0_0870;
const STREAK: u32 = 0x01E0_08F0;
const SPARK: u32 = 0x01E0_09F0;
const DARK: u32 = 0x01E0_0BD0;
const MOTE: u32 = 0x01E0_0D00;
const DISC: u32 = 0x01E0_0F20;

const D05: u64 = 0x3FE0_0000_0000_0000;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x100);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x100);
    ret
}

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// rand() % `m` with rand's frame saving a 64-bit s0.
fn rmod64(fx: &mut Fx, s0: u64, ra: u32, m: i32) -> i32 {
    let f = fx.sp.wrapping_sub(0x10);
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    let v = fx.rand();
    fx.div(v, m).1
}

fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

fn fixu(fx: &mut Fx, at: u32, ra: u32) -> i32 {
    let v = fx.mem.f(at);
    fx.fix_residue(v, ra);
    ee::fixunssfsi(v) as i32
}

/// 0x1A2E08's unpack of the double `x` at `p` (b159's).
fn unpack_d(fx: &mut Fx, x: u64, p: u32) {
    fx.mem.write_u32(p + 4, (x >> 63) as u32);
    let e = ((x >> 52) & 0x7FF) as u32;
    let m = x & 0xF_FFFF_FFFF_FFFF;
    if e == 0 {
        fx.mem.write_u32(p, 2);
    } else if e == 0x7FF {
        if m == 0 {
            fx.mem.write_u32(p, 4);
        } else {
            fx.mem.write_u32(p, u32::from(m & (1 << 51) != 0));
            fx.mem.write_u64(p + 0x10, m);
        }
    } else {
        fx.mem.write_u64(p + 0x10, (m << 8) | (1 << 60));
        fx.mem.write_u32(p + 8, e.wrapping_sub(0x3FF));
        fx.mem.write_u32(p, 3);
    }
}

/// (double) `v` < `b` through 0x1A2C30 (from `ra1`) and 0x17BCA0 (from
/// `ra2`) with the frames they leave (b159's `dlt`).
fn dlt(fx: &mut Fx, v: f32, b: u64, s0: u32, ra1: u32, ra2: u32) -> bool {
    let sp = fx.sp;
    fx.fix_residue(v, ra1);
    let f = sp - 0x30;
    let (class, sign, exp, mant) = (fx.mem.read_u32(f), fx.mem.read_u32(f + 4), fx.mem.read_u32(f + 8), fx.mem.read_u32(f + 0xC));
    let g = sp - 0x60;
    fx.mem.write_u32(g, class);
    fx.mem.write_u32(g + 4, sign);
    fx.mem.write_u32(g + 8, exp);
    fx.mem.write_u64(g + 0x10, u64::from(mant) << 30);
    fx.mem.write_u64(g + 0x20, 0x001A_2C60);
    let bits = v.to_bits();
    let a = if (bits >> 23) & 0xFF == 0 { u64::from(bits >> 31) << 63 } else { f64::from(v).to_bits() };
    fx.saved(sp - 0x10, &[(0, u64::from(ra2))]);
    let h = sp - 0x70;
    fx.mem.write_u64(h + 0x40, a);
    fx.mem.write_u64(h + 0x48, b);
    fx.mem.write_u64(h + 0x50, u64::from(s0));
    fx.mem.write_u64(h + 0x58, 0x0017_BCB0);
    unpack_d(fx, a, h);
    unpack_d(fx, b, h + 0x20);
    f64::from_bits(a) < f64::from_bits(b)
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3CD0);
    l2::sine_table(fx, TAB);
    fx.mem.set16(D + 0x3B5C, param);
    fx.mem.set32(TIMER, 0x8C);
    let h = fx.self_handle();
    fx.request_node(h, 1, 1);
    for o in [0xC0u32, 0xC4, 0xC8] {
        fx.mem.set32(D + o, 0);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(NODE, r);
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0xD8, t);
    for k in 0xEu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x108 + 4 * (k - 0xE), t);
    }
    0
}

/// The saves of a callee of the entry's: (frame size, the s0 slot, the ra
/// slot), `regs` the entry's s0.. as many as the callee saves.
fn saves(fx: &mut Fx, size: u32, base: u32, r: u32, regs: &[u64], ra: u32) {
    let f = fx.sp - size;
    for (j, v) in regs.iter().enumerate() {
        fx.saved(f + base + 0x10 * j as u32, &[(0, *v)]);
    }
    fx.saved(f, &[(r, u64::from(ra))]);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let s1 = param as i64 as u64;
    emit(fx, s1);
    for k in 0..RINGS {
        let r = RPOOL + 0x7C * k;
        if fx.mem.i16(r + 0x68) == 0 {
            continue;
        }
        let s = fx.enter(0x30);
        fx.saved(s, &[(0x20, 0x01E0_1200), (0x10, u64::from(k)), (0, u64::from(D))]);
        ring(fx, r);
        fx.leave(0x30);
    }
    let mut count = 0u64;
    for k in 0..RINGS {
        let r = RPOOL + 0x7C * k;
        if fx.mem.i16(r + 0x68) == 0 {
            continue;
        }
        count += 1;
        let regs = [u64::from(D), u64::from(r - 0x110), count, u64::from(k), 0, 0, 0];
        for (mode, ra) in [(0, 0x01E0_123C), (1, 0x01E0_1250)] {
            saves(fx, 0x150, 0, 0x70, &regs, ra);
            ring_draw(fx, r, sp + 0x70, mode);
        }
    }
    fx.mem.set16(ANGLE, 0);
    fx.mem.set32(POS + 0xC, 0);
    // The entry's s1 after the ring draws: D + 0x7C x 15.
    let regs = |s1: u64| [u64::from(D), s1, count, u64::from(RINGS), 0];
    if fx.mem.i16(COUNTER) == 0x69 {
        spawn(fx, FLASH, regs(u64::from(D + 0x7C * RINGS)), 0x01E0_128C);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x68..=0x6B).contains(&c) {
        for j in 0..5u64 {
            spawn(fx, STREAK, regs(j), 0x01E0_12D8);
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x60, u64::from(D), 0x01E0_12FC);
    let z = ee::sub(fx.mem.f(sp + 0x68), 25.0);
    fx.mem.setf(sp + 0x68, z);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == FLASH {
            flash(fx, t);
        }
        let call = |fx: &mut Fx, size: u32, ra: u32, g: fn(&mut Fx, u32)| {
            let s = fx.enter(size);
            match size {
                0x20 => fx.saved(s, &[(0x10, u64::from(ra)), (0, u64::from(D))]),
                0x30 => fx.saved(s, &[(0x20, u64::from(ra)), (0x10, u64::from(D + 0x40 * k)), (0, u64::from(D))]),
                _ => fx.saved(s, &[(0x20, u64::from(ra)), (0x10, u64::from(D + 0x40 * k)), (0, u64::from(D))]),
            }
            g(fx, t);
            fx.leave(size);
        };
        if fx.mem.read_u32(t + 0x24) == STREAK {
            call(fx, 0x20, 0x01E0_13C8, streak);
        }
        if fx.mem.read_u32(t + 0x24) == SPARK {
            call(fx, 0x30, 0x01E0_13E8, spark);
        }
        if fx.mem.read_u32(t + 0x24) == DARK {
            call(fx, 0x20, 0x01E0_1408, dark);
        }
        if fx.mem.read_u32(t + 0x24) == MOTE {
            call(fx, 0x30, 0x01E0_1428, mote);
        }
        if fx.mem.read_u32(t + 0x24) == DISC {
            call(fx, 0x40, 0x01E0_1448, disc);
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x60, u64::from(D), 0x01E0_146C);
    let mut count = 0u64;
    let mut s4 = 0u64;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        count += 1;
        draw(fx, sp, t, k, count, &mut s4);
    }
    if fx.mem.i16(COUNTER) >= 0x15 {
        let v = fx.mem.f(D + 0xC0);
        if dlt(fx, v, D05, D, 0x01E0_1620, 0x01E0_1630) {
            let v = ee::add(fx.mem.f(D + 0xC0), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0xC0, v);
        }
        fx.mem.write_u32(D + 0xC4, 0x3DCC_CCCD);
        let v = fx.mem.f(D + 0xC8);
        if dlt(fx, v, D05, D, 0x01E0_1668, 0x01E0_1678) {
            let v = ee::add(fx.mem.f(D + 0xC8), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0xC8, v);
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x60 => 1,
        0x75 => 4,
        0x6B => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw of one live task (index `k`, `count` the live tasks so
/// far: the entry's s2 / s3, s1 = D + 0x40 k; s4 the OT once a flash has
/// been drawn): each draw twice (mode 0, then 1) into the descriptor sp +
/// 0x70, the streaks once by 0x1E00080.
fn draw(fx: &mut Fx, sp: u32, t: u32, k: u32, count: u64, s4: &mut u64) {
    let d = sp + 0x70;
    let s = fx.mem.read_u32(t + 0x24);
    if s == FLASH {
        let ot = fx.mem.read_u32(OTW);
        *s4 = u64::from(ot);
        l2::colour_strip4(fx, d, t + 0x28, 0);
        l2::desc_gouraud4(fx, d, 0);
        l2::fullscreen4(fx, d);
        fx.submit(d, ot, 0);
    }
    let regs = [u64::from(D), u64::from(D + 0x40 * k), u64::from(k), count, *s4, 0];
    // (frame size, the ra slot, the saved registers from s0).
    let pair = |fx: &mut Fx, g: fn(&mut Fx, u32, u32, i32), size: u32, r: u32, n: usize, ra: u32| {
        for (mode, ra) in [(0, ra), (1, ra + 0x14)] {
            saves(fx, size, 0, r, &regs[..n], ra);
            g(fx, t, d, mode);
        }
    };
    if fx.mem.read_u32(t + 0x24) == STREAK {
        saves(fx, 0xD0, 0, 0x30, &regs[..3], 0x01E0_14F8);
        let ot = fx.mem.read_u32(OTW);
        streak_draw(fx, ot, t, d, sp + 0x60);
    }
    if fx.mem.read_u32(t + 0x24) == SPARK {
        pair(fx, sprite_b, 0x120, 0x50, 5, 0x01E0_1520);
    }
    if fx.mem.read_u32(t + 0x24) == DARK {
        pair(fx, dark_draw, 0x110, 0x60, 6, 0x01E0_1560);
    }
    if fx.mem.read_u32(t + 0x24) == MOTE {
        pair(fx, sprite_mote, 0x110, 0x50, 5, 0x01E0_15A0);
    }
    if fx.mem.read_u32(t + 0x24) == DISC {
        pair(fx, disc_draw, 0x120, 0x60, 6, 0x01E0_15E0);
    }
}

/// 0x1E00220(D, 0) (frame 0x60: ra, the entry's s2 = 0, s1 = the
/// parameter, s0 = D): the spawn colour (0x80, 0, 0, 0), the counter, the
/// yaw and the spawns. Its s0..s2 as the record leaves them are tracked
/// for the frames below it.
fn emit(fx: &mut Fx, s1: u64) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x30, 0x01E0_11E0), (0x20, 0), (0x10, s1), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x50);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x54)), 180.0);
    let v = fix(fx, y, 0x01E0_0290);
    fx.mem.set16(YAW, v);
    let v = fx.mem.i16(YAW) - 0xB4;
    fx.mem.set16(YAW, v);
    let a = fx.mem.i16(YAW);
    let c = l2::sin_deg(fx, TAB, a, 1);
    let s0v = fix(fx, ee::mul(20.0, ee::neg(c)), 0x01E0_02C4);
    let s0 = s0v as i64 as u64;
    let mut s1 = s1;
    // The spawns' saves: the emitter's s0 / s1, s2 = D, s3 / s4 the
    // runner's 0.
    let go = |fx: &mut Fx, f: u32, s1: u64, ra: u32| {
        spawn(fx, f, [s0, s1, u64::from(D), 0, 0], ra);
    };
    if fx.mem.i16(COUNTER) == 0x14 {
        for k in 0..3 {
            fx.mem.write_u8(COLOUR + k, 0x20);
        }
        s1 = 0;
        fx.mem.set16(D + 0x3B58, 1);
        fx.mem.set32(TOP, 0);
        fx.mem.set16(ANGLE, 1);
        fx.mem.set16(ANGLE2, 0);
        add_ring(fx, [s0, s1, u64::from(D)], 0x01E0_0310);
        s1 = 1;
    }
    let at = |fx: &mut Fx, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        node_position_saving(fx, h, n, sp + 0x40, s0, Some(s1), ra);
    };
    let angles = |fx: &mut Fx, ras: (u32, u32)| {
        let a = rmod64(fx, s0, ras.0, 0x168);
        fx.mem.set16(ANGLE, a);
        let a = rmod64(fx, s0, ras.1, 0x168);
        fx.mem.set16(ANGLE2, a);
    };
    let pos = |fx: &mut Fx, dz: bool| {
        for o in [0u32, 4] {
            let w = fx.mem.read_u32(sp + 0x40 + o);
            fx.mem.write_u32(POS + o, w);
        }
        let z = if dz { ee::add(fx.mem.f(sp + 0x48), ee::from_i32(s0v)) } else { fx.mem.f(sp + 0x48) };
        fx.mem.setf(POS + 8, z);
    };
    let c = fx.mem.i16(COUNTER);
    if (0x69..=0x6C).contains(&c) {
        at(fx, s1, 0x01E0_036C);
        s1 = 0;
        while s1 < 5 {
            pos(fx, true);
            angles(fx, (0x01E0_039C, 0x01E0_03B8));
            go(fx, DARK, s1, 0x01E0_03DC);
            s1 += 1;
        }
    }
    if fx.mem.i16(COUNTER) >= 0x1D && fx.mem.i32(TIMER) >= 0x15 {
        at(fx, s1, 0x01E0_0428);
        s1 = 0;
        while s1 < 3 {
            pos(fx, false);
            angles(fx, (0x01E0_0448, 0x01E0_0464));
            go(fx, MOTE, s1, 0x01E0_0488);
            s1 += 1;
        }
    }
    let c = fx.mem.i16(COUNTER);
    if c >= 0x1D && fx.mem.i32(TIMER) >= 0x15 && fx.div(c, 3).1 == 1 {
        at(fx, s1, 0x01E0_04F0);
        s1 = 0;
        pos(fx, false);
        angles(fx, (0x01E0_0510, 0x01E0_052C));
        go(fx, DISC, s1, 0x01E0_0550);
        s1 = 1;
    }
    if fx.mem.i16(COUNTER) == 0x6A {
        at(fx, s1, 0x01E0_0588);
        s1 = 0;
        while s1 < 0x41 {
            pos(fx, true);
            angles(fx, (0x01E0_05B8, 0x01E0_05D4));
            go(fx, SPARK, s1, 0x01E0_05F8);
            s1 += 1;
        }
    }
    fx.leave(0x60);
}

/// 0x1E01890(D, step, 0) (frame 0x60: the caller's s0..s4, `ra`): the
/// first free task of 200 cleared and set off at D+0x80 (as i16) with the
/// colour D+0x3B04 (twice), +0x30 = D+0x3B5A, +0x3A = D+0x3B54.
fn spawn(fx: &mut Fx, f: u32, s: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, s[4]), (0x30, s[3]), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x40 * k + 0x36) == 0) {
        Some(k) => {
            let t = POOL + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            let a = fx.mem.i16(ANGLE);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            let a = fx.mem.i16(ANGLE2);
            fx.mem.set16(t + 0x3A, a);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E01730(D, 0x1E00620, 0) (frame 0x40: the caller's s0..s2, `ra`): the
/// first free ring record of 15 cleared and set off at D+0x80 (+0x30),
/// angle +0x72 = D+0x3B54, +0x6E = D+0x3B5A, heights +4 = D+0x3B1C, +0x14
/// = that - 18, linked after D+0x3B10 (+0x50 back, the old last's +0x54
/// forward) and made the last; +0x74 = +0x76 = 1.
fn add_ring(fx: &mut Fx, s: [u64; 3], ra: u32) -> i32 {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, u64::from(ra)), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    let ret = match (0..RINGS).find(|&k| fx.mem.i16(RPOOL + 0x7C * k + 0x68) == 0) {
        Some(k) => {
            let r = RPOOL + 0x7C * k;
            fx.bzero(r, 0x7C);
            fx.mem.set16(r + 0x68, 1);
            for o in [0u32, 4, 8, 0xC] {
                let w = fx.mem.read_u32(POS + o);
                fx.mem.write_u32(r + 0x30 + o, w);
            }
            let a = fx.mem.i16(ANGLE2);
            fx.mem.set16(r + 0x72, a);
            let w = fx.mem.read_u32(LAST);
            fx.mem.write_u32(r + 0x50, w);
            let a = fx.mem.i16(ANGLE);
            fx.mem.set16(r + 0x6E, a);
            let top = fx.mem.f(TOP);
            fx.mem.setf(r + 4, top);
            fx.mem.setf(r + 0x14, ee::sub(top, 18.0));
            let w = fx.mem.read_u32(LAST);
            fx.mem.write_u32(r + 0x50, w);
            if w != 0 {
                fx.mem.write_u32(w + 0x54, r);
            }
            fx.mem.set16(r + 0x74, 1);
            fx.mem.set16(r + 0x76, 1);
            fx.mem.write_u32(LAST, r);
            1
        }
        None => 0,
    };
    fx.leave(0x40);
    ret
}

/// 0x1E00620 (D, record; frame 0x30): a ring record's step.
fn ring(fx: &mut Fx, r: u32) {
    use ee::{add, div, sub};
    for o in [0x60u32, 0x64] {
        l2::rgb(fx, r + o, 0x80, 0x80, 0x80);
    }
    fx.mem.write_u8(r + 0x63, 0x2F);
    fx.mem.write_u8(r + 0x67, 0x2F);
    let a = fx.mem.i16(r + 0x72) - 0xC;
    fx.mem.set16(r + 0x72, a);
    let a = fx.mem.i16(r + 0x72);
    if a < -0x167 {
        let m = fx.div(a, 0x168).1;
        fx.mem.set16(r + 0x72, m);
    }
    let f = add(10.0, div(fx.mem.f(r + 4), 2.0));
    let v = fix(fx, f, 0x01E0_06CC);
    fx.mem.set16(r + 0x6A, v);
    let f = add(10.0, div(fx.mem.f(r + 0x14), 2.0));
    let v = fix(fx, f, 0x01E0_06FC);
    fx.mem.set16(r + 0x6C, v);
    if fx.mem.i16(r + 0x6A) < 0 {
        fx.mem.set16(r + 0x6A, 0);
    }
    if fx.mem.i16(r + 0x6C) < 0 {
        fx.mem.set16(r + 0x6C, 0);
    }
    let a = fx.mem.f(r + 4);
    if crate::exe::ee_le(a, 60.0) {
        let a = add(a, 2.0);
        fx.mem.setf(r + 4, a);
        let b = fx.mem.f(r + 0x14);
        if !crate::exe::ee_le(sub(a, b), 5.0) {
            fx.mem.setf(r + 0x14, add(b, 2.0));
        }
    } else {
        let b = add(fx.mem.f(r + 0x14), 2.0);
        fx.mem.setf(r + 0x14, b);
        if !crate::exe::ee_le(b, fx.mem.f(r + 4)) {
            let n = fx.mem.read_u32(r + 0x54);
            if n != 0 {
                fx.mem.write_u32(n + 0x50, 0);
            }
            fx.mem.set16(r + 0x68, 0);
        }
    }
    if fx.mem.i16(r + 0x68) == 1 && !crate::exe::ee_le(fx.mem.f(r + 0x14), 0.0) {
        let v = ee::from_i32(fx.mem.i16(r + 0x6C));
        fx.mem.setf(D + 0x3B14, v);
        fx.mem.write_u32(LAST, r);
        fx.mem.set16(D + 0x3B58, 1);
        let b = fx.mem.read_u32(r + 0x14);
        fx.mem.write_u32(TOP, b);
        fx.mem.set16(ANGLE, 1);
        let a = fx.mem.i16(r + 0x72) - 0xF;
        fx.mem.set16(ANGLE2, a);
        if fx.mem.i32(TIMER) >= 0xF && add_ring(fx, [u64::from(r), u64::from(D), 0], 0x01E0_0848) != 0 {
            fx.mem.set16(r + 0x68, 2);
        }
    }
}

/// 0x1E019C0 (D, record, desc, mode; frame 0x150): mode 0 a strip3 with
/// the record's colours (+0x60 top, +0x64 bottom, through 0x108020's
/// locals), texture D+0x10C; mode 1 four 90-degree quads (from 0) in the
/// frame of the attacker's node 0 - 3 in y scaled by D+0xC0 (sprite 0x96
/// cell 0, UV order 1): the previous record's top ring (+0x6C at +0x72,
/// its height +0x14 copied into this record's +4) or this record's bottom
/// (+0x6A), to this record's top (+0x6C at +0x72); each keyed by its mean
/// w (D+0x3B34).
fn ring_draw(fx: &mut Fx, r: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x150);
    let prev = fx.mem.read_u32(r + 0x50);
    if mode == 0 {
        l2::desc_strip3(fx, d);
        for (k, (o, c)) in [(0x130u32, 0x60u32), (0x138, 0x60), (0x140, 0x64), (0x148, 0x64)].into_iter().enumerate() {
            l2::rgbaq(fx, sp + o, r + c);
            for w in 0..2 {
                let v = fx.mem.read_u32(sp + o + 4 * w);
                fx.mem.write_u32(d + 0x10 + 0x18 * k as u32 + 4 * w, v);
            }
        }
        let w = fx.mem.read_u32(D + 0x10C);
        fx.desc_texture(d, w, 1);
    } else {
        let h = fx.self_handle();
        node_position_saving(fx, h, 0, sp + 0x120, u64::from(D), Some(u64::from(prev)), 0x01E0_1ABC);
        let x = fx.mem.read_u32(sp + 0x120);
        fx.mem.write_u32(sp + 0x110, x);
        let y = ee::sub(fx.mem.f(sp + 0x124), 3.0);
        fx.mem.setf(sp + 0x114, y);
        let z = fx.mem.read_u32(sp + 0x128);
        fx.mem.write_u32(sp + 0x118, z);
        l2::xform_vp(fx, sp + 0xD0, D + 0xC0, 0, sp + 0x110);
        l2::rect(fx, sp + 0xC0, 0x96, 0);
        l2::uv_strip3(fx, d, sp + 0xC0, 1);
        let ring2 = |fx: &mut Fx, at: u32, rad: i32, deg: i32| {
            let c = l2::sin_deg(fx, TAB, deg, 1);
            fx.mem.setf(sp + at, ee::mul(ee::from_i32(rad), c));
            let s = l2::sin_deg(fx, TAB, deg, 0);
            fx.mem.setf(sp + at + 8, ee::mul(ee::from_i32(rad), s));
        };
        let mut s2 = 0i32;
        for _ in 0..4 {
            if prev != 0 {
                let rad = fx.mem.i16(prev + 0x6C);
                let a = (s2 + fx.mem.i16(prev + 0x72)) as i16 as i32;
                ring2(fx, 0x80, rad, a);
                let a = (s2 + fx.mem.i16(prev + 0x72) - 0x19) as i16 as i32;
                ring2(fx, 0x90, rad, a);
                let w = fx.mem.read_u32(prev + 0x14);
                fx.mem.write_u32(r + 4, w);
            } else {
                let rad = fx.mem.i16(r + 0x6A);
                let a = (s2 + fx.mem.i16(r + 0x72)) as i16 as i32;
                ring2(fx, 0x80, rad, a);
                ring2(fx, 0x90, rad, a);
            }
            let rad = fx.mem.i16(r + 0x6C);
            let a = (s2 + fx.mem.i16(r + 0x72)) as i16 as i32;
            ring2(fx, 0xA0, rad, a);
            let a = (s2 + fx.mem.i16(r + 0x72) - 0x19) as i16 as i32;
            ring2(fx, 0xB0, rad, a);
            for (o, h) in [(0x84u32, 4u32), (0x94, 4), (0xA4, 0x14), (0xB4, 0x14)] {
                let w = fx.mem.read_u32(r + h);
                fx.mem.write_u32(sp + o, w);
            }
            if l2::project(fx, sp + 0x80, SPR, sp + 0xD0, d, MEAN) {
                let key = fixu(fx, MEAN, 0x01E0_1DD8);
                let ot = fx.mem.read_u32(OTW);
                fx.submit(d, ot, key);
            }
            s2 = (s2 + 0x5A) as i16 as i32;
        }
    }
    fx.leave(0x150);
}

/// The frame flash (0x1E00870's body, inline in the entry): in by 0x40
/// to 0x80 (age +0x30 0 then 1), out by 9; colour (0xF0, 0x80, 0x40).
fn flash(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x30) == 0 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0xB {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.write_u8(t + 0x2B, a - 9);
        }
    }
    l2::rgb(fx, t + 0x28, 0xF0, 0x80, 0x40);
}

/// 0x1E008F0 (D, task; frame 0x20): a red streak: y +2 from 0 up by 3 to
/// 0x47 with length +6 = 10, random angles +0x10..+0x18.
fn streak(fx: &mut Fx, t: u32) {
    let s = fx.mem.i16(t + 0x30);
    if s == 0 {
        fx.mem.set16(t + 2, 0);
        fx.mem.write_u8(t + 0x2B, 0x30);
        fx.mem.write_u8(t + 0x2F, 0xFF);
        for (o, ra) in [(0x10u32, 0x01E0_0920u32), (0x14, 0x01E0_093C), (0x18, 0x01E0_0958)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        fx.mem.set16(t + 0x30, 1);
    } else if s == 1 {
        fx.mem.set16(t + 6, 0xA);
        let y = fx.mem.i16(t + 2) + 3;
        fx.mem.set16(t + 2, y);
        if fx.mem.i16(t + 2) >= 0x47 {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    l2::rgb(fx, t + 0x28, 0xFF, 0x3F, 0x3F);
    l2::rgb(fx, t + 0x2C, 0xFF, 0x3F, 0x3F);
}

/// 0x1E009F0 (D, task): a magenta spark (alpha 0x80 for +6 = rand % 15 +
/// 1 updates, then out by 9), drifting +0xE = rand % 10 + 10.
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.rand_from(t, 0x01E0_0A1C);
        l2::rgb(fx, t + 0x28, 0xFF, 0, 0xFF);
        let a = rmod(fx, t, 0x01E0_0A38, 0x168);
        fx.mem.set16(t + 0x3A, a);
        let a = rmod(fx, t, 0x01E0_0A54, 0x168);
        fx.mem.set16(t + 0x30, a);
        let a = rmod(fx, t, 0x01E0_0A70, 0xA);
        fx.mem.set16(t + 0xE, a + 0xA);
        let a = rmod(fx, t, 0x01E0_0A90, 0xF);
        fx.mem.set16(t + 6, a + 1);
        fx.mem.set16(t + 0x38, 6);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0xB, 9);
    }
    age(fx, t);
    let e = ee::from_i32(fx.mem.i16(t + 0xE));
    wobble(fx, t, 8, 0x3A, 0, e, 0x01E0_0B3C);
    let e = ee::from_i32(fx.mem.i16(t + 0xE));
    wobble(fx, t, 0xA, 0x30, 1, e, 0x01E0_0B74);
    let e = ee::from_i32(fx.mem.i16(t + 0xE));
    wobble(fx, t, 0xC, 0x3A, 1, e, 0x01E0_0BAC);
}

/// 0x1E00BD0 (D, task; frame 0x20): a dark ring (0x10, 0x10, 8; alpha
/// 0x60) at random angles, radius +0x38 growing by +0xE = rand % 5 + 2
/// below 50, out by 9 from 11, turning 12 a update.
fn dark(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x10, 0x10, 8);
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, ra) in [(0x10u32, 0x01E0_0C0Cu32), (0x14, 0x01E0_0C28), (0x18, 0x01E0_0C44)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        let r = rmod(fx, t, 0x01E0_0C60, 5);
        fx.mem.set16(t + 0xE, r + 2);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x38, 0);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0x60);
    }
    if fx.mem.i16(t + 0x38) >= 0xB {
        fade_out(fx, t, 0xB, 9);
    }
    let r = fx.mem.i16(t + 0x38);
    if r < 0x32 {
        let e = fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x38, r + e);
    }
    let a = fx.mem.i16(t + 0x3A) + 0xC;
    fx.mem.set16(t + 0x3A, a);
}

/// 0x1E00D00 (D, task): b159's mote (0x1E004C0) with colour (0x3F, 0x20,
/// 0x3F), size 2 up every fourth update to 12, in by 6 to 0x30 for 8,
/// out by 4, drift 16 / 4 / 16.
fn mote(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x3F, 0x20, 0x3F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 2);
        let r = rmod(fx, t, 0x01E0_0D48, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, 0x01E0_0D68, 0x3C);
        fx.mem.set16(t + 0x30, r - 0x1E);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 8 {
        fade_in(fx, t, 0x30, 6);
    } else {
        fade_out(fx, t, 6, 4);
    }
    age(fx, t);
    if fx.mem.i16(t + 0x32) % 4 == 1 {
        let s = fx.mem.i16(t + 0x38);
        if s < 0xC {
            fx.mem.set16(t + 0x38, s + 1);
        }
    }
    let n = fx.mem.i16(t + 0x32);
    if fx.div(n, 3).1 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 7 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    wobble(fx, t, 8, 0x3A, 0, 16.0, 0x01E0_0E90);
    wobble(fx, t, 0xA, 0x30, 1, 4.0, 0x01E0_0EC8);
    wobble(fx, t, 0xC, 0x3A, 1, 16.0, 0x01E0_0F00);
}

/// 0x1E00F20 (D, task; frame 0x40): a rising disc at the attacker's node
/// (y - 8 + the rise +0x30, up every other update), radius 100 sin +0xA
/// (+0xA from 20 by 5 below 90), in by 6 below 0x30 for 8, out by 4 once
/// +0xA is 70.
fn disc(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    l2::rgb(fx, t + 0x28, 0x28, 0x10, 0x28);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xA);
        let r = rmod(fx, t, 0x01E0_0F68, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x30, 0);
        fx.mem.set16(t + 0xA, 0x14);
        fx.mem.set16(t + 0x36, 2);
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE);
    node_position_saving(fx, h, n, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_0FB0);
    let f = fx.mem.f(sp + 0x30);
    let x = fix(fx, f, 0x01E0_0FB8);
    fx.mem.set16(t, x);
    let y = ee::add(ee::sub(fx.mem.f(sp + 0x34), 8.0), ee::from_i32(fx.mem.i16(t + 0x30)));
    let y = fix(fx, y, 0x01E0_0FE8);
    fx.mem.set16(t + 2, y);
    let f = fx.mem.f(sp + 0x38);
    let z = fix(fx, f, 0x01E0_0FF4);
    fx.mem.set16(t + 4, z);
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let v = fx.mem.i16(t + 0x30) + 1;
        fx.mem.set16(t + 0x30, v);
    }
    let a = fx.mem.i16(t + 0xA);
    let s = l2::sin_deg(fx, TAB, a, 0);
    let r = fix(fx, ee::mul(100.0, s), 0x01E0_1048);
    fx.mem.set16(t + 0x38, r);
    let a = fx.mem.i16(t + 0xA);
    if a < 0x5A {
        fx.mem.set16(t + 0xA, a + 5);
    }
    if fx.mem.i16(t + 0x32) < 8 {
        fade_in(fx, t, 0x30, 6);
    } else if fx.mem.i16(t + 0xA) >= 0x46 {
        fade_out(fx, t, 6, 4);
    }
    age(fx, t);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

fn fade_in(fx: &mut Fx, t: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a < cap {
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(up));
    }
}

/// Alpha +0x2B down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= floor {
        fx.mem.write_u8(t + 0x2B, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// i16 `at` += `k` x (sine or cosine of i16 `deg`) through 0x1A2B80.
fn wobble(fx: &mut Fx, t: u32, at: u32, deg: u32, cos: i32, k: f32, ra: u32) {
    let a = fx.mem.i16(t + deg);
    let s = l2::sin_deg(fx, TAB, a, cos);
    let f = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(k, s));
    let r = fix(fx, f, ra);
    fx.mem.set16(t + at, r);
}

/// 0x109A90(d, c1, c2): a two-vertex gouraud descriptor (4 values, flags
/// 0x11, regs RGBAQ XYZF2 x 2, additive), the colours at +0x10 / +0x20
/// with Q 1.0 and their alphas' top bytes +0x1F / +0x2F cleared.
fn line_desc(fx: &mut Fx, d: u32, c1: u32, c2: u32) {
    fx.enter(0x40);
    fx.mem.set16(d + 2, 0x11);
    fx.mem.set16(d, 4);
    fx.mem.write_u64(d + 8, 0x4141);
    fx.mem.set32(d + 4, 0);
    fx.desc_alpha(d, ALPHA_ADD);
    for k in 0..4 {
        let b = fx.mem.read_u8(c1 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
        let b = fx.mem.read_u8(c2 + k);
        fx.mem.write_u8(d + 0x20 + k, b);
    }
    fx.mem.write_u32(d + 0x24, 0x3F80_0000);
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    fx.mem.write_u8(d + 0x1F, 0);
    fx.mem.write_u8(d + 0x2F, 0);
    fx.leave(0x40);
}

/// 0x1E00080(ot, task, desc, at; frame 0xD0): the streak: a line from y
/// +2 to +2 + +6 (each floored at 0) turned by the task's angles +0x10 at
/// `at`, then by the negated angles, each at the far Z (0x109930).
fn streak_draw(fx: &mut Fx, ot: u32, t: u32, d: u32, at: u32) {
    let sp = fx.enter(0xD0);
    for k in 0..4 {
        let w = fx.mem.read_u32(at + 4 * k);
        fx.mem.write_u32(sp + 0x40 + 4 * k, w);
    }
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
        fx.mem.write_u32(sp + 0x70 + 4 * k, w);
    }
    line_desc(fx, d, t + 0x28, t + 0x2C);
    fx.bzero(sp + 0x50, 0x20);
    let y = ee::from_i32(fx.mem.i16(t + 2));
    fx.mem.setf(sp + 0x54, y);
    let y2 = ee::add(fx.mem.f(sp + 0x54), ee::from_i32(fx.mem.i16(t + 6)));
    fx.mem.setf(sp + 0x64, y2);
    for o in [0x54u32, 0x64] {
        if crate::exe::ee_lt(fx.mem.f(sp + o), 0.0) {
            fx.mem.set32(sp + o, 0);
        }
    }
    for k in 0..4 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(sp + 0x80 + 4 * k, w);
    }
    for pass in 0..2u64 {
        if pass == 1 {
            for k in 0..3 {
                let v = fx.mem.i32(sp + 0x80 + 4 * k).wrapping_neg();
                fx.mem.set32(sp + 0x80 + 4 * k, v);
            }
        }
        l2::xform_vp(fx, sp + 0x90, 0, sp + 0x80, sp + 0x70);
        // 0x109930(in, desc, m, ot): its mean w at its own sp + 0x30.
        let f = fx.sp - 0x40;
        fx.saved(f, &[(0, pass), (0x10, u64::from(d)), (0x20, 0x01E0_01F0)]);
        if l2::project(fx, sp + 0x50, SPR, sp + 0x90, d, f + 0x30) {
            for o in [0x18u32, 0x28] {
                let v = fx.mem.read_u64(d + o);
                fx.mem.write_u64(d + o, (v & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFF << 32));
            }
            fx.submit(d, ot, 0);
        }
    }
    fx.leave(0xD0);
}

/// The trans vector at `out`: the task's i16 position + i16 +8 / +0xA /
/// +0xC over 10 (integer division by the magic multiply).
fn trans_tenth(fx: &mut Fx, t: u32, out: u32) {
    for k in 0..3u32 {
        let off = fx.mem.i16(t + 8 + 2 * k);
        fx.mult(0x6666_6667, off);
        let hi = fx.hilo().1;
        let q = (hi >> 2).wrapping_add(((off as u32) >> 31) as i32);
        let v = fx.mem.i16(t + 2 * k).wrapping_add(q);
        fx.mem.setf(out + 4 * k, ee::from_i32(v));
    }
}

fn setup(fx: &mut Fx, d: u32, r: u32, c: u32, tex: u32) {
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, c, 0);
    l2::uv_strip3(fx, d, r, 0);
    let w = fx.mem.read_u32(tex);
    fx.desc_texture(d, w, 1);
}

/// 0x1E022F0 (D, task, desc, mode) (b159's 0x1E01520): a camera-facing
/// quad of half size i16 +0x38, sprite 0x14 cell +0x34, texture D+0xD8.
fn sprite_mote(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xB0, 0x14, c);
        setup(fx, d, sp + 0xB0, t + 0x28, D + 0xD8);
    } else {
        camera_angles_saving(fx, sp + 0x100, u64::from(OT), u64::from(d), 0x01E0_238C);
        trans_tenth(fx, t, sp + 0xA0);
        l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, OT);
    }
    fx.leave(0x110);
}

/// 0x1E01E30 (D, task, desc, mode): the sprite scaled 0.1 (frame 0x120;
/// b130's 0x1E02730 with sprite 0x9B, texture D+0x10C).
fn sprite_b(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x120);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xC0, 0x9B, c);
        setup(fx, d, sp + 0xC0, t + 0x28, D + 0x10C);
    } else {
        camera_angles_saving(fx, sp + 0x110, u64::from(OT), u64::from(d), 0x01E0_1ECC);
        for k in 0..3 {
            fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
        }
        trans_tenth(fx, t, sp + 0xA0);
        l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, sp + 0x110, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xD0, OT);
    }
    fx.leave(0x120);
}

/// The i16 position as floats at `out`.
fn position(fx: &mut Fx, t: u32, out: u32) {
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(out + 4 * k, v);
    }
}

/// (r cos a, r sin a) into x / z at `sp + at`.
fn xz(fx: &mut Fx, at: u32, rad: i32, deg: i32) {
    let c = l2::sin_deg(fx, TAB, deg, 1);
    fx.mem.setf(at, ee::mul(ee::from_i32(rad), c));
    let s = l2::sin_deg(fx, TAB, deg, 0);
    fx.mem.setf(at + 8, ee::mul(ee::from_i32(rad), s));
}

/// 0x1E01FE0 (D, task, desc, mode; frame 0x110): mode 0 the strip3 of
/// sprite 0x8D cell +0x34, texture D+0x108; mode 1 (texture D+0x108,
/// sprite 0x8C) eighteen 20-degree quads in y = 0 between radius +0x38 -
/// 1 and +0x38 from +0x3A, turned by +0x10 at the task's point, each keyed
/// by its mean w.
fn dark_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0x100, 0x8D, c);
        setup(fx, d, sp + 0x100, t + 0x28, D + 0x108);
    } else {
        position(fx, t, sp + 0xB0);
        l2::xform_vp(fx, sp + 0xC0, 0, t + 0x10, sp + 0xB0);
        for o in [0x74u32, 0x84, 0x94, 0xA4] {
            fx.mem.set32(sp + o, 0);
        }
        let w = fx.mem.read_u32(D + 0x108);
        fx.desc_texture(d, w, 1);
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0x100, 0x8C, c);
        l2::uv_strip3(fx, d, sp + 0x100, 0);
        let mut s1 = 0i32;
        for _ in 0..0x12 {
            let r = fx.mem.i16(t + 0x38);
            let a = (fx.mem.i16(t + 0x3A) + s1) as i16 as i32;
            xz(fx, sp + 0x90, r, a);
            xz(fx, sp + 0x70, r - 1, a);
            let a = (fx.mem.i16(t + 0x3A) + 0x14 + s1) as i16 as i32;
            xz(fx, sp + 0xA0, r, a);
            xz(fx, sp + 0x80, r - 1, a);
            if l2::project(fx, sp + 0x70, SPR, sp + 0xC0, d, MEAN) {
                let key = fixu(fx, MEAN, 0x01E0_229C);
                let ot = fx.mem.read_u32(OTW);
                fx.submit(d, ot, key);
            }
            s1 = (s1 + 0x14) as i16 as i32;
        }
    }
    fx.leave(0x110);
}

/// 0x1E02490 (D, task, desc, mode; frame 0x120): mode 0 the strip3 of
/// sprite 0x97 cell +0x34, texture D+0x10C; mode 1 twelve 30-degree quads
/// between radius +0x38 - 2 (y 0) and +0x38 (y 3) from +0x3A, scaled
/// (0.08, 1, 0.08) and turned by +0x10 at the task's point, each drawn
/// with sprite 0x8F (texture D+0x108) then 0x8C, keyed by the mean w.
fn disc_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x120);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0x110, 0x97, c);
        setup(fx, d, sp + 0x110, t + 0x28, D + 0x10C);
    } else {
        position(fx, t, sp + 0xB0);
        for (o, v) in [(0xC0u32, 0x3DA3_D70Au32), (0xC4, 0x3F80_0000), (0xC8, 0x3DA3_D70A)] {
            fx.mem.write_u32(sp + o, v);
        }
        l2::xform_vp(fx, sp + 0xD0, sp + 0xC0, t + 0x10, sp + 0xB0);
        for (o, v) in [(0x74u32, 0x4040_0000u32), (0x84, 0x4040_0000), (0x94, 0), (0xA4, 0)] {
            fx.mem.write_u32(sp + o, v);
        }
        let mut s1 = 0i32;
        for _ in 0..0xC {
            let r = fx.mem.i16(t + 0x38);
            let a = (fx.mem.i16(t + 0x3A) + s1) as i16 as i32;
            xz(fx, sp + 0x90, r - 2, a);
            xz(fx, sp + 0x70, r, a);
            let a = (fx.mem.i16(t + 0x3A) + 0x1E + s1) as i16 as i32;
            xz(fx, sp + 0xA0, r - 2, a);
            xz(fx, sp + 0x80, r, a);
            for (id, ra, tex) in [(0x8F, 0x01E0_2768u32, true), (0x8C, 0x01E0_27C4, false)] {
                let c = fx.mem.i16(t + 0x34);
                l2::rect(fx, sp + 0x110, id, c);
                l2::uv_strip3(fx, d, sp + 0x110, 0);
                if tex {
                    let w = fx.mem.read_u32(D + 0x108);
                    fx.desc_texture(d, w, 1);
                }
                if l2::project(fx, sp + 0x70, SPR, sp + 0xD0, d, MEAN) {
                    let key = fixu(fx, MEAN, ra);
                    let ot = fx.mem.read_u32(OTW);
                    fx.submit(d, ot, key);
                }
            }
            s1 = (s1 + 0x1E) as i16 as i32;
        }
    }
    fx.leave(0x120);
}
