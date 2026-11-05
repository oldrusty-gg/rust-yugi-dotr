//! Steps of the older 0x40-byte task pools (alive +0x36, age +0x32) shared
//! by the disc-family records (cards 230 / 256 / 312 / 415 / 431 / 519 /
//! 558), timer D+0x3610 [F]. Each step is
//! `(D, t) -> 0`; the frame's saved registers are the caller's to write
//! (as for `spawn::disc_step`), the residues of the calls inside (rand,
//! 0x1F6830 / 0x1F71A0, 0x1A2B80) are written here from `at`, the step's
//! address in the record.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;
use crate::fxlib2 as l2;

/// The records' timer (frames left).
const TIMER: u32 = 0x3610;

// 273ab66c9ff5 (7 users, the stage light on the timer) is never spawned
// in any of its records (fnparity: 0 calls), so it is not ported.

/// 54c2ffacfed1 [4; card 256 0x1E032F0, 96 words]: `camera(D, t) -> 0`,
/// camera 0's slots 4 / 5 (0x1F7E20) from D+0x34F0: on the first update
/// (128, 4, 2, 0) +0x34F0, the task's i16 point as floats +0x3500, (0,
/// 120, 0) +0x3510, +0x30 = 0; while +0x30 is 0 the weight D+0x3518 grows
/// by 0.2 (below 1.0, else +0x30 = 1), then falls by 0.1 (the task over
/// once at most 0.1); the slots cleared (p = 0) on the task's last update;
/// the task also ends when the timer is at 1. Frame 0x30 (s0 = t, s1 = D,
/// ra).
pub fn camera(fx: &mut Fx, at: u32, d: u32, t: u32) -> i32 {
    camera_by(fx, at, d, t, [0x4080_0000, 0x4000_0000, 0, 0x42F0_0000], (0x134, 0x148), (0x34F0, TIMER))
}

/// 0fefeb0ba2fb [1; card 412 (b476) 0x1E034D0, 96 words]:
/// [`camera`] on the slots' block D+`base` (b476: 0x2870) with the timer
/// D+`timer` (0x2DF0) [F fdiff: only the offsets differ].
pub fn camera_at(fx: &mut Fx, at: u32, d: u32, t: u32, base: u32, timer: u32) -> i32 {
    camera_by(fx, at, d, t, [0x4080_0000, 0x4000_0000, 0, 0x42F0_0000], (0x134, 0x148), (base, timer))
}

/// 23ead9b25ae7 [1; card 230 (b046) 0x1E02F80, 92 words]:
/// [`camera`] with +0x34F8 = 4.0 (the 4.0 register reused, one word
/// less: the 0x1F7E20 calls return to +0x130 / +0x144) [F fdiff].
pub fn camera_z4(fx: &mut Fx, at: u32, d: u32, t: u32) -> i32 {
    camera_by(fx, at, d, t, [0x4080_0000, 0x4080_0000, 0, 0x42F0_0000], (0x130, 0x144), (0x34F0, TIMER))
}

/// 88b4cbe1a4e1 [1; card 415 (b484) 0x1E01E90, 96 words]:
/// [`camera`] with (128, 2, 0, 4) +0x34F0 and (0, 140, 0) +0x3510 [F
/// fdiff].
pub fn camera_far(fx: &mut Fx, at: u32, d: u32, t: u32) -> i32 {
    camera_by(fx, at, d, t, [0x4000_0000, 0, 0x4080_0000, 0x430C_0000], (0x134, 0x148), (0x34F0, TIMER))
}

/// [`camera`] with `v` (bits) at +0x34F4 / +0x34F8 / +0x34FC / +0x3514
/// and the 0x1F7E20 calls' return offsets `ra` (cleared, set), on the
/// block D+`at_off.0` (+0x28 the weight) with the timer D+`at_off.1`.
fn camera_by(fx: &mut Fx, at: u32, d: u32, t: u32, v: [u32; 4], ra: (u32, u32), at_off: (u32, u32)) -> i32 {
    let (base, timer) = at_off;
    let sp = fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        let c = d + base;
        for (o, v) in [(0, 0x4300_0000u32), (4, v[0]), (8, v[1]), (0xC, v[2]), (0x20, 0), (0x24, v[3]), (0x28, 0)] {
            fx.mem.write_u32(c + o, v);
        }
        for k in 0..3 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(c + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    let w = fx.mem.f(d + base + 0x28);
    if fx.mem.i16(t + 0x30) == 0 {
        if w < 1.0 {
            fx.mem.setf(d + base + 0x28, ee::add(w, f32::from_bits(0x3E4C_CCCD)));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else {
        let s = f32::from_bits(0x3DCC_CCCD);
        if w <= s {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.setf(d + base + 0x28, ee::sub(w, s));
        }
    }
    let (p, ra) = if fx.mem.i16(t + 0x36) == 0 { (0, at + ra.0) } else { (d + base, at + ra.1) };
    crate::blib::exe::camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, u64::from(ra))]);
    if fx.mem.i32(d + timer) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.leave(0x30);
    0
}

/// rand % m (HI), rand from the step (s0 = `t`) returning to `ra`.
fn rmod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// The constants of one node-mote step.
#[derive(Clone, Copy)]
struct Mote {
    frame: u32,
    size: i32,
    /// +0x30's first value: rand % this (`None`: 0).
    start: Option<i32>,
    /// +0xE = rand % m + b.
    grow: (i32, i32),
    /// +6 = rand % m + b.
    life: (i32, i32),
    fade: u8,
    /// The record's node table (D + this + 2 x the task's +0xA).
    nodes: u32,
    /// rand's return addresses (from the step's start).
    ras: &'static [u32],
}

/// The two first-update bodies and the shared fade: +0x38 size, +0x34 1,
/// +0x18 rand % 0xFFFF, grey 0x50, +0xA = +0x30 (the node), +0x30 start,
/// +0x3A rand % 360, +0xE growth, +8 rand % 20, +6 life; then alpha +0x2B
/// up by 0x40 (while at most 0x80) during the life, else down by `fade`
/// (the task ends below 0x11); +0x30 += +0xE after the first update; age.
fn mote_age(fx: &mut Fx, k: Mote, at: u32, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, k.size);
        fx.mem.set16(t + 0x34, 1);
        let mut ras = k.ras.iter().map(|&o| at + o);
        let r = rmod(fx, t, ras.next().unwrap(), 0xFFFF);
        fx.mem.set32(t + 0x18, r);
        l2::rgb(fx, t + 0x28, 0x50, 0x50, 0x50);
        let n = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, n);
        match k.start {
            Some(m) => {
                let v = rmod(fx, t, ras.next().unwrap(), m);
                fx.mem.set16(t + 0x30, v);
            }
            None => fx.mem.set16(t + 0x30, 0),
        }
        let v = rmod(fx, t, ras.next().unwrap(), 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, ras.next().unwrap(), k.grow.0);
        fx.mem.set16(t + 0xE, v + k.grow.1);
        let v = rmod(fx, t, ras.next().unwrap(), 0x14);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, ras.next().unwrap());
        fx.mem.set16(t + 6, v % k.life.0 + k.life.1);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x81 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - k.fade);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, v);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// d77e34c72a66 [4; card 230 0x1E02B80, 140 words]: `mote_target(D, t)
/// -> 0`: [`mote_age`] (size 12, +0x30 from 0, growth 100..117, life
/// 2..5, fade 10), then the task at the attacker's node D+0x3640[+0xA]
/// (0x1F6830) pulled toward the target: x / y - 10 as they are, z + (the
/// target's z - the attacker's) / 25 (0x1F71A0 twice), i16 by 0x1A2B80.
/// Frame 0x60 (s0 = t, s1 = D, ra).
pub fn mote_target(fx: &mut Fx, at: u32, d: u32, t: u32) -> i32 {
    const K: Mote = Mote { frame: 0x60, size: 0xC, start: None, grow: (0x12, 0x64), life: (4, 2), fade: 0xA, nodes: 0x3640, ras: &[0x34, 0x68, 0x84, 0xA4, 0xC0] };
    let sp = fx.enter(K.frame);
    mote_age(fx, K, at, t);
    let h = fx.self_handle();
    let i = fx.mem.i16(t + 0xA);
    let slot = fx.mem.i16((d + K.nodes).wrapping_add((2 * i) as u32));
    node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(d)), at + 0x17C);
    let h = fx.self_handle();
    unit_position_saving(fx, h, sp + 0x40, u64::from(t), at + 0x190);
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x50, u64::from(t), at + 0x1A4);
    let dz = ee::div(ee::sub(fx.mem.f(sp + 0x58), fx.mem.f(sp + 0x48)), 25.0);
    fx.mem.setf(sp + 0x48, dz);
    let x = fx.mem.f(sp + 0x30);
    fx.fix_residue(x, at + 0x1DC);
    fx.mem.set16(t, ee::fixsfsi(x));
    let y = ee::sub(fx.mem.f(sp + 0x34), 10.0);
    fx.fix_residue(y, at + 0x1F4);
    fx.mem.set16(t + 2, ee::fixsfsi(y));
    let z = ee::add(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48));
    fx.fix_residue(z, at + 0x208);
    fx.mem.set16(t + 4, ee::fixsfsi(z));
    fx.leave(K.frame);
    0
}

/// 4804c3cd4061 [4; card 230 0x1E02DB0, 116 words]: `mote_node(D, t) ->
/// 0`: [`mote_age`] (size 14, +0x30 from rand % 100, growth 140..169,
/// life 3..4, fade 15), then the task at the attacker's node
/// D+0x363C[+0xA] (0x1F6830), i16 by 0x1A2B80. Frame 0x40 (s0 = t, s1 =
/// D, ra).
pub fn mote_node(fx: &mut Fx, at: u32, d: u32, t: u32) -> i32 {
    const K: Mote = Mote { frame: 0x40, size: 0xE, start: Some(0x64), grow: (0x1E, 0x8C), life: (2, 3), fade: 0xF, nodes: 0x363C, ras: &[0x34, 0x64, 0x80, 0x9C, 0xBC, 0xD8] };
    let sp = fx.enter(K.frame);
    mote_age(fx, K, at, t);
    let h = fx.self_handle();
    let i = fx.mem.i16(t + 0xA);
    let slot = fx.mem.i16((d + K.nodes).wrapping_add((2 * i) as u32));
    node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(d)), at + 0x194);
    for (k, ra) in [(0u32, 0x19Cu32), (1, 0x1A8), (2, 0x1B4)] {
        let v = fx.mem.f(sp + 0x30 + 4 * k);
        fx.fix_residue(v, at + ra);
        fx.mem.set16(t + 2 * k, ee::fixsfsi(v));
    }
    fx.leave(K.frame);
    0
}

/// e83340d8a0d0 [2; cards 417 / 431, b477 (card 431) 0x1E01E30, 80 words]
///: `band_grow(D, t) -> 0`, a red tick ring's step: on the
/// first update size -10, angles x rand % 20 - 10 / y rand % 360 / z 0,
/// cell 0, growth +8 rand % 3 + 4, +6 = 20, colour (0x80, 0x20, 0x20);
/// alpha in by 0x40 (below 0x80) for 2 updates, then out by 0x13 once the
/// size is 0x15+ (the task over below 0x15); the size grows from the
/// second update. Frame 0x20 (s0 = t, ra).
pub fn band_grow(fx: &mut Fx, at: u32, _d: u32, t: u32) -> i32 {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, -0xA);
        let v = rmod(fx, t, at + 0x28, 0x14);
        fx.mem.set32(t + 0x10, v - 0xA);
        let v = rmod(fx, t, at + 0x48, 0x168);
        fx.mem.set32(t + 0x14, v);
        fx.mem.set32(t + 0x18, 0);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
        let v = rmod(fx, t, at + 0x74, 3);
        fx.mem.set16(t + 8, v + 4);
        fx.mem.set16(t + 6, 0x14);
        l2::rgb(fx, t + 0x28, 0x80, 0x20, 0x20);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 2 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        }
    } else if fx.mem.i16(t + 0x38) >= 0x15 {
        if a >= 0x15 {
            fx.mem.write_u8(t + 0x2B, a - 0x13);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x38) + fx.mem.i16(t + 8);
        fx.mem.set16(t + 0x38, v);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
    0
}

/// 909eba0ed1be [2; cards 417 / 596, b500 (card 417) 0x1E02830, 108 words]
///: `ring_strip(D, t) -> 0`, the step of a ring of strips
/// (drawn by `billboard::ring_strip3`): on the first update size 16, cell
/// rand % 8, angles +0x10 / +0x14 / +0x18 rand % 0xFFFF, colour (0xA0,
/// 0xA0, 0x10) then (0x80, 0x80, 0x80), radius +0x30 rand % 50 + 300,
/// +0x3A rand % 360, +0xE rand % 18 + 10, row +8 rand % 10, +0xC = 10,
/// life +6 = 2; alpha in by 0x58 (below 0xB1, the byte wrapping) during the
/// life, then out by 0xD (the task over below 0x11). Frame 0x20 (s0 = t,
/// ra).
pub fn ring_strip(fx: &mut Fx, at: u32, _d: u32, t: u32) -> i32 {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x10);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v % 8);
        for (o, ra) in [(0x10u32, 0x44u32), (0x14, 0x60), (0x18, 0x7C)] {
            let v = rmod(fx, t, at + ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        l2::rgb(fx, t + 0x28, 0xA0, 0xA0, 0x10);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, at + 0xBC, 0x32);
        fx.mem.set16(t + 0x30, v + 0x12C);
        let v = rmod(fx, t, at + 0xDC, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, at + 0xF8, 0x12);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = rmod(fx, t, at + 0x118, 0xA);
        fx.mem.set16(t + 8, v);
        fx.mem.set16(t + 0xC, 0xA);
        fx.mem.set16(t + 6, 2);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0xB1 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x58));
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xD);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
    0
}
