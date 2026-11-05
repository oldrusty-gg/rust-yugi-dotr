//! BMODULE b117 (card 163), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E022E0) [F]. The entry
//! takes (D, param). b130's family (cards 553 / 170) with b263's ribbon and
//! disc: a pool of 220 tasks of 0x40 bytes at D+0x294 (alive +0x36, step
//! +0x24 as a record address), spawned by 0x1E02AA0 (b130's spawn from
//! index 1; the head step 0x1E019F0 always takes task 0), the sines
//! D+0x3B90, timer D+0x3B30 = 70. Nodes: the attacker's 0x1A (D+0x3B78),
//! the target's 1 (D+0x3B7A), the attacker's 0xD (D+0x3B7C).
//!
//! Steps: 0x1E015D0 (b064's violet mote with +0x30 = rand % 360, drift
//! 12), 0x1E017F0 (a yellow puff (0x80, 0x80, 0) at the head's history
//! point D+0x3A70, in by 0xF0, out by 0x27, drift 10), 0x1E019F0 (the head:
//! (0xFF, 0x80, 0x80) flying from the attacker's node 0x1A toward the
//! target's node 1 by a tenth a update plus the step D+0xD0 (y, z), its
//! points pushed into the history D+0x3A70 (10) and D+0x3B10; ended on
//! 0x32), 0x1E01BA0 (a spark (0x50, 0x50, 0), random angles, size rand %
//! 38 + 5), the flash 0x1E01CE0 (inline in the entry: (0x8F, 0x8F, 0),
//! size 0x11), 0x1E01D80 (b159's ring), 0x1E01F30 (an amber puff (0x80,
//! 0x80, 0x40), size 5, drift 18), 0x1E02110 (an undrawn grey task at the
//! head's point) and 0x1E02260 (the object op 0x1F8380(4, 0, ..), inline in
//! the entry, ended on 0x28). Drawn twice each (mode 0 then 1): the motes
//! by 0x1E02C00 (sprite 0x99), the sparks by 0x1E02E10 (sprite 0x9B, 2 x
//! size, turned by the task's angles + the spins D+0x3B20.. (+0x14, +0x18,
//! +0x14 a draw), far Z), the flash and rings by 0x1E03050 (sprite 0x9B,
//! far Z), the puffs by 0x1E03220 (sprite 0x9B scaled 0.1); texture
//! D+0x11C. From 0x1C the ribbon D+0x3D70 (b263's, 32 slots, life 50, width
//! 10) follows the head's point, with the disc D+0x4010 at the attacker's
//! node fading by 4. Returns 1 on 0x29, 4 on 0x3D, 2 on 0x33, 8 once the
//! timer has run out.
use crate::b494::camera_angles_saving;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0xDC;
const POOL: u32 = D + 0x294;
const POS: u32 = D + 0x80;
const HIST: u32 = D + 0x3A70;
const HEAD: u32 = D + 0x3B10;
const SPIN: u32 = D + 0x3B20;
const TIMER: u32 = D + 0x3B30;
const COLOUR: u32 = D + 0x3B34;
const OTW: u32 = D + 0x3B3C;
const NODE: u32 = D + 0x3B78;
const ANGLE: u32 = D + 0x3B82;
const INDEX: u32 = D + 0x3B88;
const PARAM: u32 = D + 0x3B8A;
const COUNTER: u32 = D + 0x3B8C;
const TAB: u32 = D + 0x3B90;
const TEX: u32 = D + 0x11C;
const RIB_X: u32 = D + 0x3D00;
const RIBBON: u32 = D + 0x3D70;
const RIB_PTS: u32 = D + 0x3DA0;
const DISC_X: u32 = D + 0x3FA0;
const DISC: u32 = D + 0x4010;

const MOTE: u32 = 0x01E0_15D0;
const PUFF_Y: u32 = 0x01E0_17F0;
const HEAD_S: u32 = 0x01E0_19F0;
const SPARK: u32 = 0x01E0_1BA0;
const FLASH: u32 = 0x01E0_1CE0;
const RING: u32 = 0x01E0_1D80;
const PUFF_A: u32 = 0x01E0_1F30;
const GREY: u32 = 0x01E0_2110;
const OBJECT_OP: u32 = 0x01E0_2260;

const D05: u64 = 0x3FE0_0000_0000_0000;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x110);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x110);
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

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4020);
    l2::sine_table(fx, TAB);
    fx.mem.set16(PARAM, param);
    fx.mem.set32(TIMER, 0x46);
    for o in [0xC0u32, 0xC4, 0xC8] {
        fx.mem.set32(D + o, 0);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x1A, 1);
    fx.mem.set16(NODE, r);
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(NODE + 2, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0xD, 0);
    fx.mem.set16(NODE + 4, r);
    for k in 0..3u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0xE0 + 4 * k, t);
    }
    for k in 0xEu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x118 + 4 * (k - 0xE), t);
    }
    fx.xform_init(RIB_X);
    fx.xform_init(DISC_X);
    fx.texture5(sp + 0x70, 0);
    // The ribbon: 32 slots, life 50, width 10, cell (0x10, 0, 0x10, 0x20).
    let g = RIBBON;
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 8, 0x8003);
    for k in [6u32, 5, 4, 7] {
        fx.mem.write_u8(g + k, 0x80);
    }
    for (k, x) in [(0x10u32, 0x10), (0x12, 0), (0x14, 0x10), (0x16, 0x20), (0x18, 0x10), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u32(g + 0xC, 0x4120_0000);
    for (k, x) in [(0x22u32, 0x10), (0x24, 0), (0x26, 0x32), (0x28, 0x20), (0x2A, 0), (0x2C, 1)] {
        fx.mem.set16(g + k, x);
    }
    // The disc: radius 5, 0x20 triangles, (0xD0, 0xA0, 0x70, 0x80).
    fx.mem.write_u32(DISC, 0x8_0000);
    fx.mem.write_u8(DISC + 6, 0x70);
    fx.mem.write_u8(DISC + 5, 0xA0);
    fx.mem.write_u8(DISC + 4, 0xD0);
    fx.mem.write_u8(DISC + 7, 0x80);
    fx.mem.write_u32(DISC + 8, 0x40A0_0000);
    fx.mem.set16(DISC + 0xC, 0x20);
    fx.mem.set16(DISC + 0xE, 0);
    0
}

/// The steps, their frames and the entry's return address after the call.
const STEPS: [(u32, u32, u32); 7] = [
    (MOTE, 0x30, 0x01E0_24E4),
    (PUFF_Y, 0x30, 0x01E0_2508),
    (HEAD_S, 0x50, 0x01E0_2528),
    (SPARK, 0x20, 0x01E0_2548),
    (RING, 0x20, 0x01E0_2600),
    (PUFF_A, 0x30, 0x01E0_2620),
    (GREY, 0x30, 0x01E0_2640),
];

fn run_step(fx: &mut Fx, a: u32, t: u32) {
    match a {
        MOTE => mote(fx, t),
        PUFF_Y => puff(fx, t, (0x80, 0x80, 0), 1, 0, 0xF0, 10.0, [0x01E0_1834, 0x01E0_1854, 0x01E0_195C, 0x01E0_1994, 0x01E0_19CC]),
        HEAD_S => head(fx, t),
        SPARK => spark(fx, t),
        RING => ring(fx, t),
        PUFF_A => puff(fx, t, (0x80, 0x80, 0x40), 5, 6, 0xC0, 18.0, [0x01E0_1F78, 0x01E0_1F98, 0x01E0_2080, 0x01E0_20B8, 0x01E0_20F0]),
        _ => grey(fx, t),
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        for &(a, size, ra) in STEPS.iter() {
            if fx.mem.read_u32(t + 0x24) == a {
                let s = fx.enter(size);
                // The entry's s0 = D, s1 = k.
                match size {
                    0x20 => fx.saved(s, &[(0x10, u64::from(ra)), (0, u64::from(D))]),
                    _ => fx.saved(s, &[(0x20, u64::from(ra)), (0x10, u64::from(k)), (0, u64::from(D))]),
                }
                run_step(fx, a, t);
                fx.leave(size);
            }
            if a == SPARK && fx.mem.read_u32(t + 0x24) == FLASH {
                flash(fx, t);
            }
        }
        if fx.mem.read_u32(t + 0x24) == OBJECT_OP {
            for (to, from) in [(0x10Cu32, 0x3400u32), (0x108, 0x3408)] {
                let w = fx.mem.read_u32(0x01E0_0000 + from);
                fx.mem.write_u32(sp + to, w);
            }
            fx.boundary(0x001F_8380, "object op 0x1F8380", [4, 0, sp + 0x10C, sp + 0x108]);
            if fx.mem.i16(COUNTER) == 0x28 {
                fx.mem.set16(t + 0x36, 0);
            }
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
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
    let c = fx.mem.i16(COUNTER);
    if c == 0x1C {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, sp + 0x60);
        fx.ribbon_push(RIBBON, RIB_PTS, sp + 0x60);
        for k in 0..4u32 {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * k);
            fx.mem.write_u32(DISC_X + 0x20 + 4 * k, w);
        }
    }
    if fx.mem.i16(COUNTER) >= 0x1D {
        fx.ribbon_push(RIBBON, RIB_PTS, HIST);
    }
    // The entry's s0 = D, s1 = D + 0x3700, s2 = the live count, s3 = 220
    // (their frames' matrices are what an untextured ribbon's TEX0 / TEX1
    // slots hold).
    let regs = [Some(D), Some(D + 0x40 * TASKS), Some(count), Some(TASKS)];
    fx.xform_world_saving(RIB_X, 0x01E0_2900, regs);
    fx.xform_billboard_saving(DISC_X, 0x01E0_2908, regs);
    if fx.mem.i16(COUNTER) >= 0x1C {
        fx.ribbon_draw(RIBBON, RIB_PTS, RIB_X);
        let mut k = 0;
        while k < fx.mem.i16(RIBBON + 0x2A) {
            fx.step_down(RIB_PTS + 0xC + 0x10 * k as u32, 0, 5);
            k += 1;
        }
        fx.disc(DISC, DISC_X);
        fx.step_down(DISC + 7, 0, 4);
    }
    if fx.mem.i16(COUNTER) >= 0x15 {
        let v = fx.mem.f(D + 0xC0);
        if dlt(fx, v, D05, D, 0x01E0_2998, 0x01E0_29A8) {
            let v = ee::add(fx.mem.f(D + 0xC0), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0xC0, v);
        }
        fx.mem.write_u32(D + 0xC4, 0x3DCC_CCCD);
        let v = fx.mem.f(D + 0xC8);
        if dlt(fx, v, D05, D, 0x01E0_29E0, 0x01E0_29F0) {
            let v = ee::add(fx.mem.f(D + 0xC8), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0xC8, v);
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x29 => 1,
        0x3D => 4,
        0x33 => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw of one live task (index `k`, `count` the live tasks so
/// far: the entry's s3 / s2, s1 = D + 0x40 k): each draw twice (mode 0,
/// then 1) into the descriptor sp + 0x78.
fn draw(fx: &mut Fx, sp: u32, t: u32, k: u32, count: u32) {
    let d = sp + 0x78;
    let regs = [u64::from(D), u64::from(D + 0x40 * k), u64::from(count), u64::from(k), 0];
    let pair = |fx: &mut Fx, g: fn(&mut Fx, u32, u32, i32), size: u32, ra: u32| {
        for (mode, ra) in [(0, ra), (1, ra + 0x14)] {
            let f = fx.sp - size;
            for (j, v) in regs.iter().enumerate() {
                fx.saved(f + 0x10 * j as u32, &[(0, *v)]);
            }
            fx.saved(f, &[(0x50, u64::from(ra))]);
            g(fx, t, d, mode);
        }
    };
    let s = fx.mem.read_u32(t + 0x24);
    if s == MOTE {
        pair(fx, sprite_mote, 0x110, 0x01E0_2714);
    }
    if fx.mem.read_u32(t + 0x24) == PUFF_Y {
        pair(fx, sprite_puff, 0x120, 0x01E0_2750);
    }
    if fx.mem.read_u32(t + 0x24) == SPARK {
        pair(fx, sprite_spark, 0x120, 0x01E0_2790);
    }
    if fx.mem.read_u32(t + 0x24) == FLASH {
        pair(fx, sprite_far, 0x120, 0x01E0_27D0);
    }
    if fx.mem.read_u32(t + 0x24) == RING {
        pair(fx, sprite_far, 0x120, 0x01E0_2810);
    }
    if fx.mem.read_u32(t + 0x24) == PUFF_A {
        pair(fx, sprite_puff, 0x120, 0x01E0_2850);
    }
}

/// 0x1E01210(D, 0) (frame 0x70): the spawn colour (0x80, 0, 0, 0), the
/// update counter and the spawns.
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x70);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    if c == 1 {
        spawn(fx, OBJECT_OP);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    let pos = |fx: &mut Fx| {
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x50 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    };
    let angle = |fx: &mut Fx, ra: u32| {
        let r = rmod(fx, D, ra, 0x168);
        fx.mem.set16(ANGLE, r);
    };
    let c = fx.mem.i16(COUNTER);
    if (0x30..=0x32).contains(&c) {
        for _ in 0..0xC {
            angle(fx, 0x01E0_12B0);
            pos(fx);
            spawn(fx, MOTE);
        }
        for _ in 0..0x13 {
            angle(fx, 0x01E0_1310);
            pos(fx);
            spawn(fx, PUFF_A);
        }
    }
    if fx.mem.i16(COUNTER) == 0x31 {
        for _ in 0..0x21 {
            angle(fx, 0x01E0_137C);
            pos(fx);
            spawn(fx, SPARK);
        }
        spawn(fx, FLASH);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x1A..0x2D).contains(&c) {
        for _ in 0..0xD {
            angle(fx, 0x01E0_1404);
            spawn(fx, PUFF_Y);
        }
    }
    if fx.mem.i16(COUNTER) == 0x1C {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, sp + 0x60);
        let k = 21.0f32;
        let x = ee::div(ee::sub(fx.mem.f(sp + 0x50), fx.mem.f(sp + 0x60)), k);
        fx.mem.setf(D + 0xD0, x);
        let y = ee::div(ee::sub(ee::add(20.0, fx.mem.f(sp + 0x54)), fx.mem.f(sp + 0x64)), k);
        fx.mem.setf(D + 0xD4, y);
        let z = ee::div(ee::sub(fx.mem.f(sp + 0x58), fx.mem.f(sp + 0x68)), k);
        fx.mem.setf(D + 0xD8, z);
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE);
    fx.node_position(h, n, sp + 0x50);
    if fx.mem.i16(COUNTER) == 0x1C {
        pos(fx);
        spawn(fx, HEAD_S);
        for k in 0..0xAu32 {
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
                fx.mem.write_u32(HIST + 0x10 * k + 4 * c, w);
            }
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x1C..0x2D).contains(&c) {
        for _ in 0..5 {
            angle(fx, 0x01E0_1578);
            spawn(fx, GREY);
        }
    }
    fx.leave(0x70);
}

/// 0x1E02AA0(D, step, 0) (frame 0x60): the head step takes task 0, any
/// other the first free task from 1, cleared and set off at D+0x80 (as
/// i16) with the colour D+0x3B34 (twice), +0x30 = D+0x3B88.
fn spawn(fx: &mut Fx, f: u32) -> i32 {
    let sp = fx.enter(0x60);
    let _ = sp;
    let slot = if f == HEAD_S {
        fx.bzero(POOL, 0x40);
        fx.mem.set16(POOL + 0x36, 1);
        Some(POOL)
    } else {
        match (1..TASKS).find(|&k| fx.mem.i16(POOL + 0x40 * k + 0x36) == 0) {
            Some(k) => {
                let t = POOL + 0x40 * k;
                fx.bzero(t, 0x40);
                fx.mem.set16(t + 0x36, 1);
                Some(t)
            }
            None => None,
        }
    };
    let ret = match slot {
        Some(t) => {
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
    let f = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(k, s));
    let r = fix(fx, f, ra);
    fx.mem.set16(t + at, r);
}

/// 0x1E015D0 (D, task): a violet mote drifting about its point.
fn mote(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x3F, 0x30, 0x3F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0);
        let r = rmod(fx, t, 0x01E0_1614, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, 0x01E0_1634, 0x168);
        fx.mem.set16(t + 0x30, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fade_in(fx, t, 0x60, 9);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    age(fx, t);
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let s = fx.mem.i16(t + 0x38);
        if s < 0x12 {
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
    wobble(fx, t, 8, 0x3A, 0, 12.0, 0x01E0_1758);
    wobble(fx, t, 0xA, 0x30, 1, 12.0, 0x01E0_1790);
    wobble(fx, t, 0xC, 0x3A, 1, 12.0, 0x01E0_17C8);
}

/// 0x1E017F0 / 0x1E01F30 (D, task): a puff of colour `c`, size `size`
/// (the yellow one at the head's history point), in by `up` while the age
/// is below `young` (0: only on its first update), out by 0x27, cells up
/// every fifth update, drift `k`.
#[allow(clippy::too_many_arguments)]
fn puff(fx: &mut Fx, t: u32, c: (i32, i32, i32), size: i32, young: i32, up: u8, k: f32, ra: [u32; 5]) {
    l2::rgb(fx, t + 0x28, c.0, c.1, c.2);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, size);
        let r = rmod(fx, t, ra[0], 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, ra[1], 0x168);
        fx.mem.set16(t + 0x30, r);
        if young == 0 {
            for (o, ra) in [(0u32, 0x01E0_1874u32), (2, 0x01E0_1880), (4, 0x01E0_188C)] {
                let v = fx.mem.f(HIST + 2 * o);
                let r = fix(fx, v, ra);
                fx.mem.set16(t + o, r);
            }
        }
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.i16(t + 0x32);
    if (young == 0 && a <= 0) || (young != 0 && a < young) {
        fade_in(fx, t, up, up);
    } else {
        fade_out(fx, t, 0x29, 0x27);
    }
    age(fx, t);
    let n = fx.mem.i16(t + 0x32);
    if fx.div(n, 5).1 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 7 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    wobble(fx, t, 8, 0x3A, 0, k, ra[2]);
    wobble(fx, t, 0xA, 0x30, 1, k, ra[3]);
    wobble(fx, t, 0xC, 0x3A, 1, k, ra[4]);
}

/// 0x1E01190(to, n, 0, p) (a leaf): the history shifted up one, `p` (3
/// floats) in at `to` - 0x10 (n - 1).
fn push_hist(fx: &mut Fx, top: u32, n: u32, p: u32) {
    for v in 0..n {
        let a = top - 0x10 * v;
        let src = if n - 1 - v != 0 { a - 0x10 } else { p };
        for c in 0..3 {
            let w = fx.mem.read_u32(src + 4 * c);
            fx.mem.write_u32(a + 4 * c, w);
        }
    }
}

/// 0x1E019F0 (D, task) (frame 0x50): the head.
fn head(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    l2::rgb(fx, t + 0x28, 0xFF, 0x80, 0x80);
    l2::rgb(fx, t + 0x2C, 0, 0, 0);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 2);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    for c in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * c));
        fx.mem.setf(sp + 0x30 + 4 * c, v);
    }
    push_hist(fx, D + 0x3B00, 0xA, sp + 0x30);
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), fx.mem.f(D + 0xD8));
    let r = fix(fx, z, 0x01E0_1AB8);
    fx.mem.set16(t + 4, r);
    let y = ee::add(ee::from_i32(fx.mem.i16(t + 2)), fx.mem.f(D + 0xD4));
    let r = fix(fx, y, 0x01E0_1AD8);
    fx.mem.set16(t + 2, r);
    let h = fx.target_handle();
    let n = fx.mem.i16(NODE + 2);
    fx.node_position(h, n, sp + 0x40);
    let dx = ee::div(ee::sub(fx.mem.f(sp + 0x40), fx.mem.f(sp + 0x30)), 10.0);
    let x = ee::add(ee::from_i32(fx.mem.i16(t)), dx);
    let r = fix(fx, x, 0x01E0_1B20);
    fx.mem.set16(t, r);
    fx.mem.write_u8(t + 0x2B, 0xFF);
    for c in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * c));
        fx.mem.setf(HEAD + 4 * c, v);
    }
    if fx.mem.i16(COUNTER) == 0x32 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E01BA0 (D, task): a spark at random angles.
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        let r = rmod(fx, t, 0x01E0_1BC4, 0x26);
        fx.mem.set16(t + 0x38, r + 5);
        let v = fx.rand_from(t, 0x01E0_1BE4);
        fx.mem.set16(t + 0x34, v % 4);
        fx.rand_from(t, 0x01E0_1C00);
        l2::rgb(fx, t + 0x28, 0x50, 0x50, 0);
        for (o, ra) in [(0x10u32, 0x01E0_1C1Cu32), (0x14, 0x01E0_1C38), (0x18, 0x01E0_1C54)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
    } else {
        fade_out(fx, t, 0x11, 0xE);
    }
    age(fx, t);
}

/// The entry's inline step of the flash tasks (0x1E01CE0's body).
fn flash(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x11);
        l2::rgb(fx, t + 0x28, 0x8F, 0x8F, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    age(fx, t);
}

/// 0x1E01D80 (D, task) (b159's 0x1E00B30): a dark violet ring.
fn ring(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x30, 0x30, 0x60);
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, ra) in [(0x10u32, 0x01E0_1DBCu32), (0x14, 0x01E0_1DD8), (0x18, 0x01E0_1DF4)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x38, 0);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0x80);
    }
    let r = fx.mem.i16(t + 0x38);
    match fx.mem.i16(t + 0x30) {
        0 => {
            if r >= 0xB {
                fade_out(fx, t, 0x11, 0x10);
            }
            let r = fx.mem.i16(t + 0x38);
            fx.mem.set16(t + 0x38, r + 1);
        }
        1 => {
            if r >= 0x1A {
                fade_out(fx, t, 0x11, 0x10);
            }
            let r = fx.mem.i16(t + 0x38);
            if r < 0x32 {
                fx.mem.set16(t + 0x38, r + 3);
            }
        }
        _ => {
            if r >= 0x15 {
                fade_out(fx, t, 0x11, 0x10);
            }
            let r = fx.mem.i16(t + 0x38);
            if r < 0x1E {
                fx.mem.set16(t + 0x38, r + 2);
            }
        }
    }
    let a = fx.mem.i16(t + 0x3A) + 0xC;
    fx.mem.set16(t + 0x3A, a);
}

/// 0x1E02110 (D, task): an undrawn grey task at the head's point.
fn grey(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x19);
        let v = fx.rand_from(t, 0x01E0_2140);
        fx.mem.set16(t + 0x34, v % 4);
        fx.rand_from(t, 0x01E0_215C);
        l2::rgb(fx, t + 0x28, 0x48, 0x48, 0x48);
        for (o, ra) in [(0x10u32, 0x01E0_2178u32), (0x14, 0x01E0_2194), (0x18, 0x01E0_21B0)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    for (o, ra) in [(0u32, 0x01E0_21D8u32), (2, 0x01E0_21E4), (4, 0x01E0_21F0)] {
        let v = fx.mem.f(HEAD + 2 * o);
        let r = fix(fx, v, ra);
        fx.mem.set16(t + o, r);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
    } else {
        fade_out(fx, t, 0x21, 0x1F);
    }
    age(fx, t);
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

/// Mode 0 of the sprites: rect `id` (cell +0x34) into `r`, the strip3
/// header, colour +0x28, UVs, texture D+0x11C.
fn setup(fx: &mut Fx, t: u32, d: u32, r: u32, id: i32) {
    let c = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, c);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    l2::uv_strip3(fx, d, r, 0);
    let w = fx.mem.read_u32(TEX);
    fx.desc_texture(d, w, 1);
}

/// 0x1E02C00 (D, task, desc, mode): a camera-facing quad of half size i16
/// +0x38 at the task's position (+ offsets / 10), sprite 0x99 cell +0x34.
fn sprite_mote(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        setup(fx, t, d, sp + 0xB0, 0x99);
    } else {
        camera_angles_saving(fx, sp + 0x100, u64::from(OT), u64::from(d), 0x01E0_2D10);
        trans_tenth(fx, t, sp + 0xA0);
        l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, OT);
    }
    fx.leave(0x110);
}

/// 0x1E02E10 (D, task, desc, mode): a quad 2 x +0x38 turned by the task's
/// angles +0x10.. plus the spins D+0x3B20.. (+0x14, +0x18, +0x14 a draw),
/// sprite 0x9B, at the far Z, keyed by its mean w.
fn sprite_spark(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x120);
    if mode == 0 {
        setup(fx, t, d, sp + 0xC0, 0x9B);
    } else {
        trans_tenth(fx, t, sp + 0xB0);
        for k in 0..4u32 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(sp + 0x110 + 4 * k, w);
        }
        for (k, n) in [(0u32, 0x14), (4, 0x18), (8, 0x14)] {
            let v = fx.mem.i32(SPIN + k).wrapping_add(n);
            fx.mem.set32(SPIN + k, v);
        }
        for k in [0u32, 4, 8] {
            let v = fx.mem.i32(sp + 0x110 + k).wrapping_add(fx.mem.i32(SPIN + k));
            fx.mem.set32(sp + 0x110 + k, v);
        }
        l2::xform_vp(fx, sp + 0xD0, 0, sp + 0x110, sp + 0xB0);
        let h = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, 2.0, h);
        if l2::project(fx, sp + 0x60, SPR, sp + 0xD0, d, sp + 0xA0) {
            l2::far_z_desc(fx, d, 0);
            let key = fixu(fx, sp + 0xA0, 0x01E0_3018);
            fx.submit(d, OT, key);
        }
    }
    fx.leave(0x120);
}

/// 0x1E03050 (D, task, desc, mode): a camera-facing quad of half size i16
/// +0x38, sprite 0x9B, at the far Z, keyed by its mean w.
fn sprite_far(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x120);
    if mode == 0 {
        setup(fx, t, d, sp + 0xC0, 0x9B);
    } else {
        camera_angles_saving(fx, sp + 0x110, u64::from(OT), u64::from(d), 0x01E0_30EC);
        trans_tenth(fx, t, sp + 0xA0);
        l2::xform_vp(fx, sp + 0xD0, 0, sp + 0x110, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        if l2::project(fx, sp + 0x60, SPR, sp + 0xD0, d, sp + 0xB0) {
            l2::far_z_desc(fx, d, 0);
            let key = fixu(fx, sp + 0xB0, 0x01E0_31EC);
            fx.submit(d, OT, key);
        }
    }
    fx.leave(0x120);
}

/// 0x1E03220 (D, task, desc, mode): a camera-facing quad of half size i16
/// +0x38 scaled 0.1 at the task's position + offsets / 10.0, sprite 0x9B.
fn sprite_puff(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x120);
    if mode == 0 {
        setup(fx, t, d, sp + 0xC0, 0x9B);
    } else {
        for k in 0..3 {
            fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
        }
        camera_angles_saving(fx, sp + 0x110, u64::from(OT), u64::from(d), 0x01E0_32D0);
        for k in 0..3u32 {
            let off = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
            let p = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0xA0 + 4 * k, p);
            fx.mem.setf(sp + 0xA0 + 4 * k, ee::add(p, off));
        }
        l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, sp + 0x110, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xD0, OT);
    }
    fx.leave(0x120);
}
