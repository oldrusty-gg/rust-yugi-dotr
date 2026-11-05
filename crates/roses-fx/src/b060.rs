//! BMODULE b060 (card 231), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E013C0, data area 0x1ED0000, 0x3CD0 bytes) [F]
//!: b058's (card 435) frame and draw shapes on
//! this record's own emitter and steps. A pool of 220 tasks of 0x40 at
//! D+0x284 (alive +0x36, step +0x24 by direct `jal`), spawned by
//! 0x1E01890 (b058's spawn: colour D+0x3B04, point D+0x80, +0x30 from
//! D+0x3B58), the sines D+0x3B60, timer D+0x3B00 = 180, OT D+0x3B0C.
//! Nodes D+0x3B48..: the attacker's 0x28 / 2 at the start, its 0xC / 0x21
//! / 0x35 / 2 on update 1 and 0x28 / 2 again on 0x71.
//!
//! The emitter 0x1E00080: violet motes 0x1E009B0 two at a time at node k
//! (k = 0..3) on three updates each (0x46 / 0x5A / 0x70, 0x3D / 0x52 /
//! 0x66, 0x3A / 0x51 / 0x64, 0x44 / 0x5B / 0x6C), ten at node 1 on 0x85 /
//! 0xB0 / 0xC7; on 0x75..0x77 at node 0 ten grey puffs 0x1E00BB0, twenty
//! sparks 0x1E00ED0 and six fans 0x1E00D80 (each jittered by rand % 10 -
//! 5). The steps 0x1E01080 / 0x1E01220 are tested but never spawned [F].
//! Draws (twice each, mode 0 then 1): the motes by b058's 0.1-scaled
//! sprite (0x1E01B50: sprite 0x14, texture D+0xD8), the puffs by its
//! camera-facing quad (0x1E019B0), the sparks by the 0.1 sprite with 0x9B
//! / D+0x10C (0x1E01D00), the fans by nine 8-degree slivers drawn twice
//! (0x1E01EB0: sprites 0x8F then 0x8C, texture D+0x108). From 0x15 D+0xC0
//! / D+0xC8 creep up by 0.01 below 0.5, D+0xC4 = 0.1. Returns 1 on 0x6B, 4
//! on 0x91, 2 on 0x84, 6 on 0x76..0x7E, 8 once the timer has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0xDC;
const POOL: u32 = D + 0x284;
const POS: u32 = D + 0x80;
const TIMER: u32 = D + 0x3B00;
const COLOUR: u32 = D + 0x3B04;
const OTW: u32 = D + 0x3B0C;
const MEAN_W: u32 = D + 0x3B34;
const NODE: u32 = D + 0x3B48;
const ANGLE: u32 = D + 0x3B52;
const INDEX: u32 = D + 0x3B58;
const COUNTER: u32 = D + 0x3B5C;
const TAB: u32 = D + 0x3B60;

const MOTE: u32 = 0x01E0_09B0;
const PUFF: u32 = 0x01E0_0BB0;
const FAN: u32 = 0x01E0_0D80;
const SPARK: u32 = 0x01E0_0ED0;
const UNSPAWNED_A: u32 = 0x01E0_1080;
const UNSPAWNED_B: u32 = 0x01E0_1220;

const D05: u64 = 0x3FE0_0000_0000_0000;

/// The entry 0x1E013C0 (D, param) (frame 0xF0).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xF0);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0xF0);
    ret
}

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
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

/// The attacker's nodes `list` (node, clear) into D+0x3B48...
fn nodes(fx: &mut Fx, list: &[(i32, i32)]) {
    for (k, &(node, clear)) in list.iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3CD0);
    l2::sine_table(fx, TAB);
    fx.mem.set16(D + 0x3B5A, param);
    fx.mem.set32(TIMER, 0xB4);
    for o in [0xC0u32, 0xC4, 0xC8] {
        fx.mem.set32(D + o, 0);
    }
    nodes(fx, &[(0x28, 1), (2, 0)]);
    for k in 0xDu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x104 + 4 * (k - 0xD), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0xD8, t);
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param as i64 as u64);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        // The step's prologue: ra, the entry's s1 = k, s0 = D.
        let call = |fx: &mut Fx, size: u32, ra: u32, g: fn(&mut Fx, u32)| {
            let s = fx.enter(size);
            match size {
                0x20 => fx.saved(s, &[(0x10, u64::from(ra)), (0, u64::from(D))]),
                _ => fx.saved(s, &[(0x20, u64::from(ra)), (0x10, u64::from(k)), (0, u64::from(D))]),
            }
            g(fx, t);
            fx.leave(size);
        };
        match fx.mem.read_u32(t + 0x24) {
            MOTE => call(fx, 0x30, 0x01E0_14CC, mote),
            PUFF => call(fx, 0x30, 0x01E0_14F0, puff),
            FAN => call(fx, 0x20, 0x01E0_1510, fan),
            SPARK => call(fx, 0x30, 0x01E0_1530, spark),
            f @ (UNSPAWNED_A | UNSPAWNED_B) => panic!("b060: unspawned step {f:#x} live"),
            _ => {}
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x50, u64::from(D), 0x01E0_1594);
    let z = ee::sub(fx.mem.f(sp + 0x58), 25.0);
    fx.mem.setf(sp + 0x58, z);
    let mut count = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        count += 1;
        draw(fx, sp, t, k, count);
    }
    if fx.mem.i16(COUNTER) >= 0x15 {
        let v = fx.mem.f(D + 0xC0);
        if dlt(fx, v, D05, D, 0x01E0_1760, 0x01E0_1770) {
            let v = ee::add(fx.mem.f(D + 0xC0), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0xC0, v);
        }
        fx.mem.write_u32(D + 0xC4, 0x3DCC_CCCD);
        let v = fx.mem.f(D + 0xC8);
        if dlt(fx, v, D05, D, 0x01E0_17A8, 0x01E0_17B8) {
            let v = ee::add(fx.mem.f(D + 0xC8), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0xC8, v);
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x6B => 1,
        0x91 => 4,
        0x84 => 2,
        0x76..=0x7E => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw of one live task (index `k`, `count` the live tasks so
/// far: the entry's s3 / s2, s1 = D + 0x40 k): each draw twice (mode 0,
/// then 1) into the descriptor sp + 0x60.
fn draw(fx: &mut Fx, sp: u32, t: u32, k: u32, count: u32) {
    let d = sp + 0x60;
    let regs = [u64::from(D), u64::from(D + 0x40 * k), u64::from(count), u64::from(k)];
    // (frame size, the s0 slot, the ra slot, the saved registers past s3).
    let pair = |fx: &mut Fx, g: fn(&mut Fx, u32, u32, i32), lay: (u32, u32, u32, u32), ra: u32| {
        let (size, base, r, more) = lay;
        for (mode, ra) in [(0, ra), (1, ra + 0x14)] {
            let f = fx.sp - size;
            for (j, v) in regs.iter().enumerate() {
                fx.saved(f + base + 0x10 * j as u32, &[(0, *v)]);
            }
            for j in 0..more {
                fx.saved(f + base + 0x40 + 0x10 * j, &[(0, 0)]);
            }
            fx.saved(f, &[(r, u64::from(ra))]);
            g(fx, t, d, mode);
        }
    };
    const SPRITE: (u32, u32, u32, u32) = (0x110, 0, 0x50, 1);
    const SPRITE_B: (u32, u32, u32, u32) = (0x120, 0, 0x50, 1);
    const FAN_F: (u32, u32, u32, u32) = (0x110, 0, 0x60, 2);
    match fx.mem.read_u32(t + 0x24) {
        MOTE => pair(fx, sprite_mote_b, SPRITE_B, 0x01E0_15E4),
        PUFF => pair(fx, sprite_puff, SPRITE, 0x01E0_1620),
        FAN => pair(fx, fan_draw, FAN_F, 0x01E0_1660),
        SPARK => pair(fx, sprite_spark_b, SPRITE_B, 0x01E0_16A0),
        _ => {}
    }
}

/// 0x1E00080(D, 0) (frame 0x60: ra, the entry's s3 (the runner's 0 [H]),
/// s2 = 0, s1 = the parameter, s0 = D): the spawn colour (0x80, 0, 0, 0),
/// the update counter, the node requests on 1 / 0x71 and the spawns. Its
/// s1..s3 as the record leaves them are tracked for the frames below it
/// (the node calls', rand's and the spawn's).
fn emit(fx: &mut Fx, s1: u64) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x40, 0x01E0_1498), (0x30, 0), (0x20, 0), (0x10, s1), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    if fx.mem.i16(COUNTER) == 0x71 {
        nodes(fx, &[(0x28, 1), (2, 0)]);
    }
    if fx.mem.i16(COUNTER) == 1 {
        nodes(fx, &[(0xC, 1), (0x21, 0), (0x35, 0), (2, 0)]);
    }
    // (s1, s2, s3) of the emitter.
    let mut r: [u64; 3] = [s1, 0, 0];
    let go = |fx: &mut Fx, f: u32, r: &[u64; 3], ra: u32| {
        spawn(fx, f, [u64::from(D), r[0], r[1], r[2], 0], ra);
    };
    let at = |fx: &mut Fx, slot: u32, r: &[u64; 3], ra: u32| {
        let h = fx.self_handle();
        let n = fx.mem.i16(slot);
        node_position_saving(fx, h, n, sp + 0x50, u64::from(D), Some(r[0]), ra);
    };
    let p = |fx: &mut Fx, o: u32| fx.mem.f(sp + 0x50 + o);
    // Two motes at node `slot` on the `list` updates: the angle, x / z
    // by rand % 4 - 2 (signed), y one lower; (the rand ras, the spawn ra).
    let pairs: [(u32, u32, [i32; 3], [u32; 3], u32); 4] = [
        (NODE, 0x01E0_0188, [0x46, 0x5A, 0x70], [0x01E0_01BC, 0x01E0_01D8, 0x01E0_0228], 0x01E0_0274),
        (NODE + 2, 0x01E0_02A0, [0x3D, 0x52, 0x66], [0x01E0_02D4, 0x01E0_02F0, 0x01E0_0340], 0x01E0_038C),
        (NODE + 4, 0x01E0_03B8, [0x3A, 0x51, 0x64], [0x01E0_03EC, 0x01E0_0408, 0x01E0_0458], 0x01E0_04A4),
        (NODE + 6, 0x01E0_04D0, [0x44, 0x5B, 0x6C], [0x01E0_0504, 0x01E0_0520, 0x01E0_0570], 0x01E0_05BC),
    ];
    for (slot, ra_at, list, ras, ra_go) in pairs {
        at(fx, slot, &r, ra_at);
        r[0] = 0;
        if list.contains(&fx.mem.i16(COUNTER)) {
            while r[0] < 2 {
                let a = rmod(fx, D, ras[0], 0x168);
                fx.mem.set16(ANGLE, a);
                let v = fx.rand_from(D, ras[1]);
                let x = ee::sub(ee::add(p(fx, 0), ee::from_i32(v % 4)), 2.0);
                fx.mem.setf(POS, x);
                let y = ee::sub(p(fx, 4), 1.0);
                fx.mem.setf(POS + 4, y);
                let v = fx.rand_from(D, ras[2]);
                let z = ee::sub(ee::add(p(fx, 8), ee::from_i32(v % 4)), 2.0);
                fx.mem.setf(POS + 8, z);
                go(fx, MOTE, &r, ra_go);
                r[0] += 1;
            }
        }
    }
    // POS axis `o` = the node point + rand % 10 - 5.
    let jitter = |fx: &mut Fx, o: u32, ra: u32| {
        let v = rmod(fx, D, ra, 0xA);
        let q = ee::sub(ee::add(p(fx, o), ee::from_i32(v)), 5.0);
        fx.mem.setf(POS + o, q);
    };
    at(fx, NODE, &r, 0x01E0_05E8);
    r[2] = 0;
    if [0x75, 0x76, 0x77].contains(&fx.mem.i16(COUNTER)) {
        while r[2] < 0xA {
            let a = rmod(fx, D, 0x01E0_061C, 0x168);
            fx.mem.set16(ANGLE, a);
            jitter(fx, 0, 0x01E0_0638);
            r[0] = u64::from(sp + 0x54);
            jitter(fx, 4, 0x01E0_066C);
            r[1] = u64::from(sp + 0x58);
            jitter(fx, 8, 0x01E0_06A4);
            go(fx, PUFF, &r, 0x01E0_06EC);
            r[2] += 1;
        }
        r[2] = 0;
        while r[2] < 0x14 {
            jitter(fx, 0, 0x01E0_0708);
            jitter(fx, 4, 0x01E0_073C);
            jitter(fx, 8, 0x01E0_0770);
            go(fx, SPARK, &r, 0x01E0_07B4);
            r[2] += 1;
        }
        r[2] = 0;
        while r[2] < 6 {
            jitter(fx, 0, 0x01E0_07D0);
            jitter(fx, 4, 0x01E0_0804);
            jitter(fx, 8, 0x01E0_0838);
            go(fx, FAN, &r, 0x01E0_087C);
            r[2] += 1;
        }
    }
    at(fx, NODE + 2, &r, 0x01E0_08A8);
    r[0] = 0;
    if [0x85, 0xB0, 0xC7].contains(&fx.mem.i16(COUNTER)) {
        while r[0] < 0xA {
            let a = rmod(fx, D, 0x01E0_08DC, 0x168);
            fx.mem.set16(ANGLE, a);
            jitter(fx, 0, 0x01E0_08F8);
            let y = ee::sub(p(fx, 4), 5.0);
            fx.mem.setf(POS + 4, y);
            jitter(fx, 8, 0x01E0_0938);
            go(fx, MOTE, &r, 0x01E0_097C);
            r[0] += 1;
        }
    }
    fx.leave(0x60);
}

/// 0x1E01890(D, step, 0) (frame 0x60: the caller's s0..s4, `ra`): the
/// first free task of 220 cleared and set off at D+0x80 (as i16) with the
/// colour D+0x3B04 (twice), +0x30 = D+0x3B58 (b058's).
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
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
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
    let p = ee::from_i32(fx.mem.i16(t + at));
    let r = fix(fx, ee::add(p, ee::mul(k, s)), ra);
    fx.mem.set16(t + at, r);
}

/// 0x1E009B0 (D, task): a violet mote (0x2F, 0x20, 0x2F): size 0 up by 5
/// to 0x28, cell up every third update to 7, +0x30 rand % 30 - 15; in by
/// 9 below 0x60 for 8 updates, then out by 4; wobbling by 6 / 4 / 6.
fn mote(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x2F, 0x20, 0x2F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0);
        let r = rmod(fx, t, 0x01E0_09F4, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, 0x01E0_0A14, 0x1E);
        fx.mem.set16(t + 0x30, r - 0xF);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 8 {
        fade_in(fx, t, 0x60, 9);
    } else {
        fade_out(fx, t, 6, 4);
    }
    age(fx, t);
    let s = fx.mem.i16(t + 0x38);
    if s < 0x28 {
        fx.mem.set16(t + 0x38, s + 5);
    }
    let n = fx.mem.i16(t + 0x32);
    if fx.div(n, 3).1 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 7 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    wobble(fx, t, 8, 0x3A, 0, 6.0, 0x01E0_0B18);
    wobble(fx, t, 0xA, 0x30, 1, 4.0, 0x01E0_0B50);
    wobble(fx, t, 0xC, 0x3A, 1, 6.0, 0x01E0_0B88);
}

/// 0x1E00BB0 (D, task): a dark puff (0x10 x 3): size 7 up by 1 to 0xA,
/// cell rand % 7, +0x30 rand % 360; in by 0x30 below 0x60 for 2 updates,
/// then out by 4; wobbling by 13.
fn puff(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x10);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 7);
        let r = rmod(fx, t, 0x01E0_0BF8, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rmod(fx, t, 0x01E0_0C14, 7);
        fx.mem.set16(t + 0x34, r);
        let r = rmod(fx, t, 0x01E0_0C30, 0x168);
        fx.mem.set16(t + 0x30, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        fade_in(fx, t, 0x60, 0x30);
    } else {
        fade_out(fx, t, 6, 4);
    }
    age(fx, t);
    let s = fx.mem.i16(t + 0x38);
    if s < 0xA {
        fx.mem.set16(t + 0x38, s + 1);
    }
    wobble(fx, t, 8, 0x3A, 0, 13.0, 0x01E0_0CF4);
    wobble(fx, t, 0xA, 0x30, 1, 13.0, 0x01E0_0D2C);
    wobble(fx, t, 0xC, 0x3A, 1, 13.0, 0x01E0_0D64);
}

/// 0x1E00D80 (D, task; frame 0x20): a fan (0x20, 0x20, 0x40), angles
/// +0x10.. rand % 0xFFFF, growth +0xE = rand % 5 + 2: alpha up by 0x30 for
/// 2 updates; from size 0xB out by 9; size up by +0xE below 0x32; +0x3A
/// turning by 2.
fn fan(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x40);
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, ra) in [(0x10u32, 0x01E0_0DBCu32), (0x14, 0x01E0_0DD8), (0x18, 0x01E0_0DF4)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        let r = rmod(fx, t, 0x01E0_0E10, 5);
        fx.mem.set16(t + 0xE, r + 2);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x38, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x30));
    }
    age(fx, t);
    if fx.mem.i16(t + 0x38) >= 0xB {
        fade_out(fx, t, 0xB, 9);
    }
    let s = fx.mem.i16(t + 0x38);
    if s < 0x32 {
        let v = s + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x38, v);
    }
    let a = fx.mem.i16(t + 0x3A) + 2;
    fx.mem.set16(t + 0x3A, a);
}

/// 0x1E00ED0 (D, task): a grey spark (0x40 x 3, set once): size 5, cell
/// rand % 4, +0x30 / +0x3A rand % 360; alpha up by 0x2A for 3 updates,
/// then out by 5; wobbling by 15.
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 5);
        let v = fx.rand_from(t, 0x01E0_0F00);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x40);
        let r = rmod(fx, t, 0x01E0_0F34, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, t, 0x01E0_0F50, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 3 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x2A));
    } else {
        fade_out(fx, t, 7, 5);
    }
    age(fx, t);
    wobble(fx, t, 8, 0x3A, 0, 15.0, 0x01E0_0FF0);
    wobble(fx, t, 0xA, 0x30, 1, 15.0, 0x01E0_1028);
    wobble(fx, t, 0xC, 0x3A, 1, 15.0, 0x01E0_1060);
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

/// 0x1E019B0 (D, task, desc, mode) (b058's 0x1E020A0): a camera-facing
/// quad of half size i16 +0x38 at the task's position (+ offsets / 10),
/// UVs of sprite 0x14 cell +0x34, texture D+0xD8, keyed by the mean w.
fn sprite_puff(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xB0, 0x14, c);
        setup(fx, d, sp + 0xB0, t + 0x28, D + 0xD8);
    } else {
        camera_angles_saving(fx, sp + 0x100, u64::from(OT), u64::from(d), 0x01E0_1A4C);
        trans_tenth(fx, t, sp + 0xA0);
        l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, OT);
    }
    fx.leave(0x110);
}

/// b058's 0x1E02240 (D, task, desc, mode): [`sprite_puff`] scaled 0.1
/// (frame 0x120) with sprite `id` / texture `tex`.
fn sprite_b(fx: &mut Fx, t: u32, d: u32, mode: i32, id: i32, tex: u32, ra: u32) {
    let sp = fx.enter(0x120);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xC0, id, c);
        setup(fx, d, sp + 0xC0, t + 0x28, tex);
    } else {
        camera_angles_saving(fx, sp + 0x110, u64::from(OT), u64::from(d), ra);
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

/// 0x1E01B50: the motes' 0.1 sprite (0x14, D+0xD8).
fn sprite_mote_b(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    sprite_b(fx, t, d, mode, 0x14, D + 0xD8, 0x01E0_1BEC);
}

/// 0x1E01D00: the sparks' 0.1 sprite (0x9B, D+0x10C).
fn sprite_spark_b(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    sprite_b(fx, t, d, mode, 0x9B, D + 0x10C, 0x01E0_1D9C);
}

/// 0x1E01EB0 (D, task, desc, mode; frame 0x110): mode 0 the strip3 of
/// sprite 0x8D cell +0x34, colour +0x28, texture D+0x108; mode 1 nine
/// slivers of a fan 8 degrees wide, 40 apart from +0x3A, between the
/// centre and radius +0x38 (y 0), turned by +0x10 at the task's i16 point,
/// each drawn with sprite 0x8F then 0x8C, keyed by its mean w (D+0x3B34)
/// into D+0x3B0C.
fn fan_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    use ee::mul;
    let sp = fx.enter(0x110);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0x100, 0x8D, c);
        setup(fx, d, sp + 0x100, t + 0x28, D + 0x108);
        fx.leave(0x110);
        return;
    }
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xB0 + 4 * k, v);
    }
    l2::xform_vp(fx, sp + 0xC0, 0, t + 0x10, sp + 0xB0);
    for o in [0x74u32, 0x84, 0x94, 0xA4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let mut s1 = 0i32;
    for _ in 0..9 {
        let a = (fx.mem.i16(t + 0x3A) + s1) as i16 as i32;
        let c = l2::sin_deg(fx, TAB, a, 1);
        let r = ee::from_i32(fx.mem.i16(t + 0x38));
        fx.mem.setf(sp + 0x90, mul(r, c));
        let s = l2::sin_deg(fx, TAB, a, 0);
        let r = ee::from_i32(fx.mem.i16(t + 0x38));
        fx.mem.setf(sp + 0x98, mul(r, s));
        fx.mem.write_u32(sp + 0x70, 0);
        fx.mem.write_u32(sp + 0x78, 0);
        let b = (fx.mem.i16(t + 0x3A) + 8 + s1) as i16 as i32;
        let c = l2::sin_deg(fx, TAB, b, 1);
        let r = ee::from_i32(fx.mem.i16(t + 0x38));
        fx.mem.setf(sp + 0xA0, mul(r, c));
        let s = l2::sin_deg(fx, TAB, b, 0);
        let r = ee::from_i32(fx.mem.i16(t + 0x38));
        fx.mem.setf(sp + 0xA8, mul(r, s));
        fx.mem.write_u32(sp + 0x80, 0);
        fx.mem.write_u32(sp + 0x88, 0);
        for (id, tex, ra) in [(0x8F, true, 0x01E0_20D0u32), (0x8C, false, 0x01E0_212C)] {
            let c = fx.mem.i16(t + 0x34);
            l2::rect(fx, sp + 0x100, id, c);
            l2::uv_strip3(fx, d, sp + 0x100, 0);
            if tex {
                let w = fx.mem.read_u32(D + 0x108);
                fx.desc_texture(d, w, 1);
            }
            if l2::project(fx, sp + 0x70, SPR, sp + 0xC0, d, MEAN_W) {
                let w = fx.mem.f(MEAN_W);
                fx.fix_residue(w, ra);
                let key = ee::fixunssfsi(w) as i32;
                let ot = fx.mem.read_u32(OTW);
                fx.submit(d, ot, key);
            }
        }
        s1 = (s1 + 0x28) as i16 as i32;
    }
    fx.leave(0x110);
}
