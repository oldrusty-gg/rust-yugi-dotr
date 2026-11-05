//! BMODULE b130 (card 553), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E013E0) [F]. The entry
//! takes (D, param). b159's family (card 170) with its globals 8 lower: a
//! pool of 220 tasks of 0x40 bytes at D+0x284 (alive +0x36, step +0x24 as
//! a record address, steps by direct `jal`), spawned by 0x1E01A00 (b159's
//! spawn with the index D+0x3B58), the sines D+0x3B60, timer 0xAA. Nodes:
//! the attacker's 0x6D (D+0x3B48), the target's 1 (D+0x3B4A), the
//! attacker's 0x6C (D+0x3B4C).
//!
//! Live steps (the emitter 0x1E005A0's `jal`s): b159's grey puff
//! (0x1E00AB0, word for word but the sine table), a blue puff 0x1E00E30
//! (b159's 0x1E008E0 with size 5, colour (0x80, 0x80, 0xFF), +0x30 = 0
//! growing by rand % 14 + 1, drift 16), the trail 0x1E00FF0 (stepped
//! inline in the entry: (0x1F, 0x10, 0x8F), alpha 0x80 then from update
//! 0x6B down by 15, ten segments ... seven: +0x32 = 7; ended when the timer
//! reads 50), a ring 0x1E010B0 (b159's ring with colours (0x10, 0x10,
//! 0x40) for index 0 / 1 else (0x80, 0x40, 0x80), alphas 0x70 / 0xB0 /
//! 0x70, growth 1 / 6 / 2 up to - / 50 / 30, fading from 11 / 41 / 21)
//! and a disc 0x1E012D0 (colour (0x20, 0x20, 0x40), alpha 0x80, growing by
//! 6 to 50, fading from 41, turning 1 a update). Drawn twice each (mode 0
//! then 1): the grey puff by b159's sprite 0x1E01B20, the blue puff by
//! 0x1E02730 (the sprite scaled 0.1), the trail by b159's trail 0x1E01E60
//! with 18 quads of 20 degrees, rings 0.3 apart growing by 0.5, the ring by
//! 0x1E022D0 (two copies stepped back from the task (from update 0x29) or
//! from the attacker's node 0x6D (before) by k / 14 of the node 0x6D ->
//! node 0x6C vector, each a ring of 18 quads of 20 degrees between +0x38 -
//! 10 and +0x38 from +0x3A + rand % 360), the disc by 0x1E028E0 (six
//! 60-degree fans from the task's point, each drawn twice: the task's
//! colour on sprite 0x97 (UV order 1), then (0x20, 0x20, 0x40) with the
//! task's alpha on sprite 0x96).
//!
//! The emitter: on 0x6A forty of each puff and ten discs (index 3) at the
//! target's position; on 1 the trail at node 0x6D; below 0x6A two rings a
//! update at node 0x6D (index 0 below 0x63, else 1). From 0x15 D+0xC0 /
//! D+0xC8 creep up by 0.01 below 0.5 (soft double compares), D+0xC4 =
//! 0.1. Returns 1 on 0x60, 4 on 0xA5, 2 on 0x6A, 8 once the timer has run
//! out. The steps 0x1E00890 / 0x1E00CA0 and their draw 0x1E01CC0 are
//! never spawned (not ported).
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{spawn as lspawn, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0xDC;
const POOL: u32 = D + 0x284;
const POS: u32 = D + 0x80;
const HIST: u32 = D + 0x3A60;
const TIMER: u32 = D + 0x3B00;
const COLOUR: u32 = D + 0x3B04;
const OTW: u32 = D + 0x3B0C;
const MEAN: u32 = D + 0x3B34;
const NODE: u32 = D + 0x3B48;
const ANGLE: u32 = D + 0x3B52;
const INDEX: u32 = D + 0x3B58;
const COUNTER: u32 = D + 0x3B5C;
const TAB: u32 = D + 0x3B60;

const PUFF: u32 = 0x01E0_0AB0;
const PUFF_B: u32 = 0x01E0_0E30;
const TRAIL: u32 = 0x01E0_0FF0;
const RING: u32 = 0x01E0_10B0;
const DISC: u32 = 0x01E0_12D0;

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
    fx.mem.set16(D + 0x3B5A, param);
    fx.mem.set32(TIMER, 0xAA);
    for o in [0xC0u32, 0xC4, 0xC8] {
        fx.mem.set32(D + o, 0);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x6D, 1);
    fx.mem.set16(NODE, r);
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(NODE + 2, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x6C, 0);
    fx.mem.set16(NODE + 4, r);
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0xD8, t);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x10C, t);
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, or the history's end after its fill.
    let mut s1 = param as i64 as u64;
    if fx.mem.i16(COUNTER) == 0 {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        node_position_saving(fx, h, n, sp + 0x60, u64::from(D), Some(u64::from(HIST)), 0x01E0_1500);
        for k in 0..9u32 {
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
                fx.mem.write_u32(HIST + 0x10 * k + 4 * c, w);
            }
        }
        s1 = u64::from(HIST + 0x90);
    }
    fx.saved(sp - 0x70, &[(0x50, 0x01E0_1540), (0x40, 0), (0x30, 0), (0x20, 0), (0x10, s1), (0, u64::from(D))]);
    let n = fx.mem.i16(NODE);
    lspawn::node_history(fx, HIST + 0x90, 0xA, n);
    emit(fx, s1);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let call = |fx: &mut Fx, size: u32, ra: u32, g: fn(&mut Fx, u32)| {
            let s = fx.enter(size);
            if size == 0x20 {
                fx.saved(s, &[(0x10, u64::from(ra)), (0, u64::from(D))]);
            } else {
                fx.saved(s, &[(0x20, u64::from(ra)), (0x10, u64::from(k)), (0, u64::from(D))]);
            }
            g(fx, t);
            fx.leave(size);
        };
        if fx.mem.read_u32(t + 0x24) == PUFF {
            call(fx, 0x30, 0x01E0_15A0, puff);
        }
        if fx.mem.read_u32(t + 0x24) == PUFF_B {
            call(fx, 0x30, 0x01E0_15E0, puff_b);
        }
        if fx.mem.read_u32(t + 0x24) == TRAIL {
            l2::rgb(fx, t + 0x28, 0x1F, 0x10, 0x8F);
            if fx.mem.i16(t + 0x36) == 1 {
                fx.mem.set16(t + 0x38, 5);
                fx.mem.write_u8(t + 0x2B, 0x80);
                fx.mem.set16(t + 0x36, 2);
            }
            if fx.mem.i16(COUNTER) >= 0x6B {
                fade_out(fx, t, 0x11, 0xF);
            }
            fx.mem.set16(t + 0x32, 7);
            if fx.mem.i32(TIMER) == 0x32 {
                fx.mem.set16(t + 0x36, 0);
            }
        }
        if fx.mem.read_u32(t + 0x24) == RING {
            call(fx, 0x20, 0x01E0_16A0, ring);
        }
        if fx.mem.read_u32(t + 0x24) == DISC {
            call(fx, 0x20, 0x01E0_16C0, disc);
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x50, u64::from(D), 0x01E0_16E4);
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
        if dlt(fx, v, D05, D, 0x01E0_18F0, 0x01E0_1900) {
            let v = ee::add(fx.mem.f(D + 0xC0), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0xC0, v);
        }
        fx.mem.write_u32(D + 0xC4, 0x3DCC_CCCD);
        let v = fx.mem.f(D + 0xC8);
        if dlt(fx, v, D05, D, 0x01E0_1938, 0x01E0_1948) {
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
        0xA5 => 4,
        0x6A => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw of one live task (index `k`, `count` the live tasks so
/// far: the entry's s3 / s2, s1 = D + 0x40 k): each draw twice (mode 0,
/// then 1) into the descriptor sp + 0x70.
fn draw(fx: &mut Fx, sp: u32, t: u32, k: u32, count: u32) {
    let d = sp + 0x70;
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
    const TRAIL_F: (u32, u32, u32, u32) = (0x1F0, 0x10, 0xA0, 5);
    const RING_F: (u32, u32, u32, u32) = (0x180, 0x10, 0xA0, 5);
    const DISC_F: (u32, u32, u32, u32) = (0x120, 0, 0x60, 2);
    if fx.mem.read_u32(t + 0x24) == PUFF {
        pair(fx, sprite_puff, SPRITE, 0x01E0_1770);
    }
    if fx.mem.read_u32(t + 0x24) == PUFF_B {
        pair(fx, sprite_b, SPRITE_B, 0x01E0_17F0);
    }
    if fx.mem.read_u32(t + 0x24) == TRAIL {
        pair(fx, trail, TRAIL_F, 0x01E0_1830);
    }
    if fx.mem.read_u32(t + 0x24) == RING {
        pair(fx, ring18, RING_F, 0x01E0_1870);
    }
    if fx.mem.read_u32(t + 0x24) == DISC {
        pair(fx, fans, DISC_F, 0x01E0_18B0);
    }
}

/// 0x1E005A0(D, 0) (frame 0x50: ra, the entry's s2 = 0, s1, s0 = D): the
/// spawn colour (0x80, 0, 0, 0), the update counter and the spawns. Its
/// s0 / s1 as the record leaves them are tracked for the frames below it.
fn emit(fx: &mut Fx, s1: u64) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x30, 0x01E0_154C), (0x20, 0), (0x10, s1), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    let at = |fx: &mut Fx, h: i32, slot: u32, s0: u64, s1: u64, ra: u32| {
        let n = fx.mem.i16(slot);
        node_position_saving(fx, h, n, sp + 0x40, s0, Some(s1), ra);
    };
    let pos = |fx: &mut Fx| {
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    };
    // The spawn's saves: the emitter's s0 / s1, s2 = D, s3 / s4 the
    // runner's 0.
    let go = |fx: &mut Fx, f: u32, s0: u64, s1: u64, ra: u32| {
        spawn(fx, f, [s0, s1, u64::from(D), 0, 0], ra);
    };
    let angle = |fx: &mut Fx, s0: u64, ra: u32| {
        let r = rmod(fx, s0 as u32, ra, 0x168);
        fx.mem.set16(ANGLE, r);
    };
    let (mut s0, mut s1) = (u64::from(D), s1);
    let h = fx.self_handle();
    at(fx, h, NODE, s0, s1, 0x01E0_05EC);
    let h = fx.target_handle();
    at(fx, h, NODE + 2, s0, s1, 0x01E0_0604);
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x40, s0, 0x01E0_0618);
    if fx.mem.i16(COUNTER) == 0x6A {
        s0 = 0;
        while s0 < 0x28 {
            angle(fx, s0, 0x01E0_0634);
            pos(fx);
            go(fx, PUFF, s0, s1, 0x01E0_0670);
            go(fx, PUFF_B, s0, s1, 0x01E0_0684);
            s0 += 1;
        }
    }
    let h = fx.self_handle();
    at(fx, h, NODE, s0, s1, 0x01E0_06B0);
    if fx.mem.i16(COUNTER) == 1 {
        s0 = 0;
        angle(fx, s0, 0x01E0_06CC);
        pos(fx);
        go(fx, TRAIL, s0, s1, 0x01E0_0708);
        s0 = 1;
    }
    let c = fx.mem.i16(COUNTER);
    if c < 0x6A {
        s0 = 2;
        s1 = 0;
        while s1 < s0 {
            angle(fx, s0, 0x01E0_0754);
            pos(fx);
            let i = i32::from(fx.mem.i16(COUNTER) >= 0x63);
            fx.mem.set16(INDEX, i);
            go(fx, RING, s0, s1, 0x01E0_07BC);
            s1 += 1;
        }
    }
    let h = fx.target_handle();
    at(fx, h, NODE + 2, s0, s1, 0x01E0_07E8);
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x40, s0, 0x01E0_07FC);
    if fx.mem.i16(COUNTER) == 0x6A {
        s0 = 0;
        while s0 < 0xA {
            angle(fx, s0, 0x01E0_0818);
            pos(fx);
            fx.mem.set16(INDEX, 3);
            go(fx, DISC, s0, s1, 0x01E0_085C);
            s0 += 1;
        }
    }
    fx.leave(0x50);
}

/// 0x1E01A00(D, step, 0) (frame 0x60: the caller's s0..s4, `ra`): the
/// first free task of 220 cleared and set off at D+0x80 (as i16) with the
/// colour D+0x3B04 (twice), +0x30 = D+0x3B58 (b159's spawn).
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
    let f = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(k, s));
    let r = fix(fx, f, ra);
    fx.mem.set16(t + at, r);
}

/// 0x1E00AB0 (D, task) (b159's 0x1E006E0): a grey puff.
fn puff(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x3F, 0x3F, 0x3F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0);
        let r = rmod(fx, t, 0x01E0_0AF4, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, 0x01E0_0B14, 0x168);
        fx.mem.set16(t + 0x30, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 8 {
        fade_in(fx, t, 0xA0, 8);
    } else {
        fade_out(fx, t, 7, 5);
    }
    age(fx, t);
    let s = fx.mem.i16(t + 0x38);
    if s < 0xD {
        fx.mem.set16(t + 0x38, s + 1);
    }
    let n = fx.mem.i16(t + 0x32);
    if fx.div(n, 5).1 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 7 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    wobble(fx, t, 8, 0x3A, 0, 13.0, 0x01E0_0C14);
    wobble(fx, t, 0xA, 0x30, 1, 13.0, 0x01E0_0C4C);
    wobble(fx, t, 0xC, 0x3A, 1, 13.0, 0x01E0_0C84);
}

/// 0x1E00E30 (D, task): a blue puff (b159's 0x1E008E0 with size 5, colour
/// (0x80, 0x80, 0xFF), +0x30 from 0 growing by rand % 14 + 1, drift 16).
fn puff_b(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 5);
        let v = fx.rand_from(t, 0x01E0_0E60);
        fx.mem.set16(t + 0x34, v % 4);
        fx.rand_from(t, 0x01E0_0E7C);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0xFF);
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, t, 0x01E0_0E98, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    age(fx, t);
    let r = rmod(fx, t, 0x01E0_0F04, 0xE);
    let v = fx.mem.i16(t + 0x30).wrapping_add((r + 1) as i16 as i32);
    fx.mem.set16(t + 0x30, v);
    wobble(fx, t, 8, 0x3A, 0, 16.0, 0x01E0_0F60);
    wobble(fx, t, 0xA, 0x30, 1, 16.0, 0x01E0_0F98);
    wobble(fx, t, 0xC, 0x3A, 1, 16.0, 0x01E0_0FD0);
}

/// 0x1E010B0 (D, task): a ring at random angles, (0x10, 0x10, 0x40) for
/// index +0x30 0 / 1 else (0x80, 0x40, 0x80), alpha 0x70 / 0xB0 / 0x70 at
/// its start; radius +0x38 from 0 growing by 1 (index 0, out by 16 from
/// 11), 6 (1: to 50, out from 41) or 2 (else: to 30, out from 21), its
/// start angle +0x3A turning by 12 a update.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x30) < 2 {
        l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x40);
    } else {
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0x80);
    }
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, ra) in [(0x10u32, 0x01E0_1114u32), (0x14, 0x01E0_1130), (0x18, 0x01E0_114C)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x38, 0);
        fx.mem.set16(t + 0x36, 2);
        match fx.mem.i16(t + 0x30) {
            0 | 2 => fx.mem.write_u8(t + 0x2B, 0x70),
            1 => fx.mem.write_u8(t + 0x2B, 0xB0),
            _ => {}
        }
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
            if r >= 0x29 {
                fade_out(fx, t, 0x11, 0x10);
            }
            let r = fx.mem.i16(t + 0x38);
            if r < 0x32 {
                fx.mem.set16(t + 0x38, r + 6);
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

/// 0x1E012D0 (D, task): a disc (0x20, 0x20, 0x40) at random angles, alpha
/// 0x80, radius +0x38 growing by 6 to 50, out by 16 from 41, turning 1.
fn disc(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x40);
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, ra) in [(0x10u32, 0x01E0_130Cu32), (0x14, 0x01E0_1328), (0x18, 0x01E0_1344)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x38, 0);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0x80);
    }
    if fx.mem.i16(t + 0x38) >= 0x29 {
        fade_out(fx, t, 0x11, 0x10);
    }
    let r = fx.mem.i16(t + 0x38);
    if r < 0x32 {
        fx.mem.set16(t + 0x38, r + 6);
    }
    let a = fx.mem.i16(t + 0x3A) + 1;
    fx.mem.set16(t + 0x3A, a);
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

/// 0x1E01B20 (D, task, desc, mode) (b159's 0x1E01380): a camera-facing
/// quad of half size i16 +0x38 at the task's position (+ offsets / 10),
/// UVs of sprite 0x99 cell +0x34, texture D+0x10C, keyed by its mean w.
fn sprite_puff(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xB0, 0x99, c);
        setup(fx, d, sp + 0xB0, t + 0x28, D + 0x10C);
    } else {
        camera_angles_saving(fx, sp + 0x100, u64::from(OT), u64::from(d), 0x01E0_1BBC);
        trans_tenth(fx, t, sp + 0xA0);
        l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, OT);
    }
    fx.leave(0x110);
}

/// 0x1E02730 (D, task, desc, mode): [`sprite_puff`] scaled 0.1 (frame
/// 0x120).
fn sprite_b(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x120);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xC0, 0x99, c);
        setup(fx, d, sp + 0xC0, t + 0x28, D + 0x10C);
    } else {
        for k in 0..3 {
            fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
        }
        camera_angles_saving(fx, sp + 0x110, u64::from(OT), u64::from(d), 0x01E0_27E0);
        trans_tenth(fx, t, sp + 0xA0);
        l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, sp + 0x110, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xD0, OT);
    }
    fx.leave(0x120);
}

/// 0x1E01E60(D, task, desc, mode) (b159's 0x1E016C0 with 18 quads of 20
/// degrees, the outer ring 0.3 out, the radius growing by 0.5 a segment;
/// both modes draw).
fn trail(fx: &mut Fx, t: u32, _d: u32, _mode: i32) {
    use ee::{add, mul};
    let sp = fx.enter(0x1F0);
    fx.mem.write_u32(sp + 0xFC, t);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 0xC8, OT);
    let s1 = sp + 0x158;
    let n = fx.mem.i16(t + 0x32);
    l2::desc_gouraud4(fx, s1, 0);
    l2::fullscreen4(fx, s1);
    fx.mem.set32(sp + 0xE0, n);
    fx.mem.set32(sp + 0xB0, 0);
    for k in 0..4 {
        let b = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(sp + 0x1EC + k, b);
    }
    fx.mem.set32(sp + 0xD0, n - 1);
    let mut f20 = 0.0f32;
    let mut a = 0i32;
    let bump = |fx: &mut Fx, at: u32, r: f32, deg: i32, cos: i32| {
        let c = l2::sin_deg(fx, TAB, deg, cos);
        let v = add(fx.mem.f(at), mul(r, c));
        fx.mem.setf(at, v);
    };
    while a < n - 1 {
        let s3 = HIST + 0x10 * a as u32;
        l2::colour_strip4(fx, s1, sp + 0x1EC, 1);
        let al = i32::from(fx.mem.read_u8(t + 0x2B));
        let q = fx.div(al, n).0;
        let b = fx.mem.read_u8(sp + 0x1EF).wrapping_sub(q as u8);
        fx.mem.write_u8(sp + 0x1EF, b);
        l2::colour_strip4(fx, s1, sp + 0x1EC, 2);
        let mut s2 = 0i32;
        for _ in 0..0x12 {
            for o in [0x110u32, 0x120] {
                for c in 0..3 {
                    let w = fx.mem.read_u32(s3 + 4 * c);
                    fx.mem.write_u32(sp + o + 4 * c, w);
                }
            }
            let s4 = (s2 + 0x14) as i16 as i32;
            let r = f20;
            bump(fx, sp + 0x114, r, s2, 1);
            bump(fx, sp + 0x110, r, s2, 0);
            bump(fx, sp + 0x118, r, s2, 1);
            bump(fx, sp + 0x124, r, s4, 1);
            bump(fx, sp + 0x120, r, s4, 0);
            bump(fx, sp + 0x128, r, s4, 1);
            for o in [0x130u32, 0x140] {
                for c in 0..3 {
                    let w = fx.mem.read_u32(s3 + 0x10 + 4 * c);
                    fx.mem.write_u32(sp + o + 4 * c, w);
                }
            }
            let r2 = || add(f20, f32::from_bits(0x3E99_999A));
            bump(fx, sp + 0x134, r2(), s2, 1);
            bump(fx, sp + 0x130, r2(), s2, 0);
            bump(fx, sp + 0x138, r2(), s2, 1);
            bump(fx, sp + 0x144, r2(), s4, 1);
            bump(fx, sp + 0x140, r2(), s4, 0);
            bump(fx, sp + 0x148, r2(), s4, 1);
            if l2::project(fx, sp + 0x110, SPR, 0, s1, sp + 0x100) {
                l2::far_z_desc(fx, s1, 1);
                let ot = fx.mem.read_u32(sp + 0xC8);
                fx.submit(s1, ot, 0);
            }
            s2 = s4;
        }
        f20 = add(f20, 0.5);
        a += 1;
        fx.mem.set32(sp + 0xB0, a);
    }
    fx.leave(0x1F0);
}

/// 0x1E022D0(D, task, desc, mode) (frame 0x180; f20..f23 the runner's (0)
/// [H]): mode 0 a 9-register strip3 header by hand (prim 0x24 | 0x800,
/// ALPHA 0x80_0000_0048), the RGBAQ of +0x28 at +0x10, UVs of sprite 0x96
/// cell +0x34 (uv_rect4 order 0), texture D+0x10C. Mode 1: two copies (k =
/// 1, 2) stepped back by k / 14 of the vector node 0x6D -> node 0x6C from
/// the task's point (from update 0x29) or the attacker's node 0x6D, each
/// turned by the task's angles +0x10 and drawn as 18 quads of 20 degrees
/// between radius +0x38 - 10 and +0x38 (y 0) from +0x3A + rand % 360,
/// keyed by the mean w D+0x3B34 on D+0x3B0C.
fn ring18(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    use ee::{div, from_i32, mul, sub};
    let sp = fx.enter(0x180);
    for k in 0..4 {
        fx.mem.write_u32(sp + 4 * k, 0);
    }
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0x140, 0x96, c);
        fx.mem.write_u16(d + 2, 0x24);
        fx.mem.write_u16(d, 9);
        fx.mem.write_u64(d + 8, (4u64 << 32) | 0x2424_2421);
        fx.mem.write_u32(d + 4, 0);
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        fx.desc_alpha(d, 0x80_0000_0048);
        fx.mem.write_u32(d + 0x14, 0);
        for o in [0x27u32, 0x37, 0x47, 0x57] {
            fx.mem.write_u8(d + o, 0);
        }
        l2::rgbaq(fx, sp + 0x178, t + 0x28);
        let (a, b) = (fx.mem.read_u32(sp + 0x178), fx.mem.read_u32(sp + 0x17C));
        fx.mem.write_u32(d + 0x10, a);
        fx.mem.write_u32(d + 0x14, b);
        uv::uv_rect4(fx, d, sp + 0x140, 0);
        let w = fx.mem.read_u32(D + 0x10C);
        fx.desc_texture(d, w, 1);
    } else {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        node_position_saving(fx, h, n, sp + 0x150, u64::from(d), Some(u64::from(t)), 0x01E0_23F0);
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE + 4);
        node_position_saving(fx, h, n, sp + 0x160, u64::from(d), Some(u64::from(t)), 0x01E0_240C);
        let v: [f32; 3] = std::array::from_fn(|c| {
            let a = fx.mem.f(sp + 0x150 + 4 * c as u32);
            let b = fx.mem.f(sp + 0x160 + 4 * c as u32);
            div(sub(a, b), 14.0)
        });
        for o in [0xB4u32, 0xC4, 0xD4, 0xE4] {
            fx.mem.set32(sp + o, 0);
        }
        for s7 in 1..3 {
            let k = from_i32(s7);
            for c in 0..3u32 {
                let from = if fx.mem.i16(COUNTER) >= 0x29 { from_i32(fx.mem.i16(t + 2 * c)) } else { fx.mem.f(sp + 0x150 + 4 * c) };
                fx.mem.setf(sp + 0xF0 + 4 * c, sub(from, mul(v[c as usize], k)));
            }
            l2::xform_vp(fx, sp + 0x100, 0, t + 0x10, sp + 0xF0);
            let r = rmod(fx, d, 0x01E0_252C, 0x168);
            let s5 = r as i16 as i32;
            let mut s4 = 0i32;
            for _ in 0..0x12 {
                for (step, outer, inner) in [(0, 0xD0u32, 0xB0u32), (0x14, 0xE0, 0xC0)] {
                    let deg = (fx.mem.i16(t + 0x3A) + step + s4 + s5) as i16 as i32;
                    let c = l2::sin_deg(fx, TAB, deg, 1);
                    let s = l2::sin_deg(fx, TAB, deg, 0);
                    let r = fx.mem.i16(t + 0x38);
                    fx.mem.setf(sp + outer, mul(c, from_i32(r)));
                    fx.mem.setf(sp + outer + 8, mul(s, from_i32(r)));
                    fx.mem.setf(sp + inner, mul(c, from_i32(r - 0xA)));
                    fx.mem.setf(sp + inner + 8, mul(s, from_i32(r - 0xA)));
                }
                if l2::project(fx, sp + 0xB0, SPR, sp + 0x100, d, MEAN) {
                    let key = fixu(fx, MEAN, 0x01E0_26B0);
                    let ot = fx.mem.read_u32(OTW);
                    fx.submit(d, ot, key);
                }
                s4 = (s4 + 0x14) as i16 as i32;
            }
        }
    }
    fx.leave(0x180);
}

/// 0x1E028E0(D, task, desc, mode) (frame 0x120): mode 0 the strip3 header,
/// UVs of sprite 0x9C cell +0x34, texture D+0x10C (no colour). Mode 1 six
/// fans of 60 degrees (from +0x3A) of radius +0x38 about the task's point
/// (y 0), turned by its angles +0x10, each drawn twice: the task's colour
/// on sprite 0x97 (UV order 1), then (0x20, 0x20, 0x40) with the task's
/// alpha on sprite 0x96; keyed by the mean w D+0x3B34 on D+0x3B0C.
fn fans(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    use ee::{from_i32, mul};
    let sp = fx.enter(0x120);
    let w = fx.mem.read_u32(0x01E0_2C00);
    fx.mem.write_u32(sp + 0x11C, w);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0x100, 0x9C, c);
        l2::desc_strip3(fx, d);
        l2::uv_strip3(fx, d, sp + 0x100, 0);
        let w = fx.mem.read_u32(D + 0x10C);
        fx.desc_texture(d, w, 1);
    } else {
        for c in 0..3u32 {
            let v = from_i32(fx.mem.i16(t + 2 * c));
            fx.mem.setf(sp + 0xB0 + 4 * c, v);
        }
        l2::xform_vp(fx, sp + 0xC0, 0, t + 0x10, sp + 0xB0);
        for o in [0x74u32, 0x84, 0x94, 0xA4] {
            fx.mem.set32(sp + o, 0);
        }
        let mut s1 = 0i32;
        for _ in 0..6 {
            for (step, o) in [(0, 0x90u32), (0x23, 0xA0)] {
                let deg = (fx.mem.i16(t + 0x3A) + step + s1) as i16 as i32;
                let c = l2::sin_deg(fx, TAB, deg, 1);
                let r = from_i32(fx.mem.i16(t + 0x38));
                fx.mem.setf(sp + o, mul(r, c));
                let s = l2::sin_deg(fx, TAB, deg, 0);
                let r = from_i32(fx.mem.i16(t + 0x38));
                fx.mem.setf(sp + o + 8, mul(r, s));
                let z = if step == 0 { 0x70 } else { 0x80 };
                fx.mem.set32(sp + z, 0);
                fx.mem.set32(sp + z + 8, 0);
            }
            l2::colour_strip3(fx, d, t + 0x28, 0);
            let c = fx.mem.i16(t + 0x34);
            l2::rect(fx, sp + 0x100, 0x97, c);
            l2::uv_strip3(fx, d, sp + 0x100, 1);
            if l2::project(fx, sp + 0x70, SPR, sp + 0xC0, d, MEAN) {
                let key = fixu(fx, MEAN, 0x01E0_2B04);
                let ot = fx.mem.read_u32(OTW);
                fx.submit(d, ot, key);
            }
            let a = fx.mem.read_u8(t + 0x2B);
            fx.mem.write_u8(sp + 0x11F, a);
            l2::colour_strip3(fx, d, sp + 0x11C, 0);
            let c = fx.mem.i16(t + 0x34);
            l2::rect(fx, sp + 0x100, 0x96, c);
            l2::uv_strip3(fx, d, sp + 0x100, 0);
            if l2::project(fx, sp + 0x70, SPR, sp + 0xC0, d, MEAN) {
                let key = fixu(fx, MEAN, 0x01E0_2B7C);
                let ot = fx.mem.read_u32(OTW);
                fx.submit(d, ot, key);
            }
            s1 = (s1 + 0x3C) as i16 as i32;
        }
    }
    fx.leave(0x120);
}
