//! BMODULE b058 (card 435), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01880) [F]. The entry
//! takes (D, param). b159's family (card 170) with its globals 0x600
//! lower: a pool of 180 tasks of 0x40 bytes at D+0x284 (alive +0x36, step
//! +0x24 as a record address, steps by direct `jal`), spawned by
//! 0x1E01DE0 (b159's spawn: colour D+0x3504, point D+0x80, +0x30 from
//! D+0x3558), the sines D+0x3560, timer D+0x3500 = 450. Nodes D+0x3548..:
//! the attacker's 0x11 at the start, then from update 0x91 its 0x1C /
//! 0x2E / 0x36 / 0xC.
//!
//! Steps: 0x1E00B40 (b159's violet mote with colour (0x2F, 0x20, 0x2F)),
//! 0x1E00D60 (a grey puff (0x28 x 3): size 2 + 1 a update to 0x19, cell
//! rand % 7, in by 5 to 0x60 for 20 updates, out by 5, drift 50 / 4 /
//! 50), 0x1E00F40 (an orange puff (0x60, 0x30, 0): in by 9 for 10, out by
//! 10, cells up every third update, drift 10 with y down), 0x1E01120 (an
//! orange spark (0x80, 0x40, 0): its +0xA = 20 sin +0x30, +0x30 up by 6,
//! drift 16), 0x1E012B0 (a zigzag of 8 points D+0x3100 + 0x80 index, x
//! 20 k, z a random walk of +- 30, turned by the attacker's yaw - 0x4000
//! (+ rand % 0xC000 - 0x6000 for index > 0); in by 0x1E to 0xF0 for 28,
//! out by 3), 0x1E01600 (a dim ember (0x40, 0x40, 0x1F), size 15 down by
//! 1, +0x3A by the attacker's side and the update) and the stage light
//! 0x1E017A0 (0x1F7F10: 0x80 down by 10 to 0x30, up by 7 to 0x80 once the
//! timer is under 26). Draws (twice each, mode 0 then 1): the grey
//! puffs by b159's camera-facing quad (0x1E020A0: sprite 0x14, texture
//! D+0xD8), the motes and embers by its twin 0x1E01F00 (0x99, D+0x10C), the
//! orange puffs by 0x1E02240 (b130's 0.1-scaled sprite with 0x14, D+0xD8),
//! the zigzags by 0x1E023F0 (seven quads between successive points, half
//! width 10 - k, turned by +0x10 at the task's point, sprite 0x96) and the
//! sparks once by `quads4::sprite_spec` (sprite 0x9B, D+0x10C).
//!
//! The emitter 0x1E00260: on update 1 the light; on 0x51..0x81 three
//! orange puffs a update about node 0 (+- 18 / 5 / 18); on 0xB3 / 0x102 /
//! 0x14D / 0x1F0 / 0x215 / 0x254 twenty motes at node 0, on 0xDF / 0x14F /
//! 0x1FE / 0x236 at node 1, on 0xD3 / 0x23C at node 2, on 0xAB / 0xF8 /
//! 0x21F / 0x257 at node 3; on 0x14C / 0x14F / 0x152 fifteen grey puffs
//! (+- 10) at nodes 0 and 1, on 0x14F also fifteen sparks and seven
//! zigzags (40 degrees apart) at each; camera slots 2 / 3 off (0x1F7EB0)
//! when the timer is at 1. From 0x15 D+0xC0 / D+0xC8 creep up by 0.01
//! below 0.5, D+0xC4 = 0.1. Returns 1 on 0x15F, 4 on 0x1A4, 2 on 0x187,
//! 6 on 0x16A..0x181, 8 once the timer has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{exe as lexe, quads4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0xB4;
const POOL: u32 = D + 0x284;
const POS: u32 = D + 0x80;
const TIMER: u32 = D + 0x3500;
const COLOUR: u32 = D + 0x3504;
const OTW: u32 = D + 0x350C;
const NODE: u32 = D + 0x3548;
const ANGLE: u32 = D + 0x3552;
const INDEX: u32 = D + 0x3558;
const COUNTER: u32 = D + 0x355C;
const TAB: u32 = D + 0x3560;
const ZIG: u32 = D + 0x3100;

const MOTE: u32 = 0x01E0_0B40;
const PUFF: u32 = 0x01E0_0D60;
const PUFF_O: u32 = 0x01E0_0F40;
const SPARK: u32 = 0x01E0_1120;
const ZIGZAG: u32 = 0x01E0_12B0;
const EMBER: u32 = 0x01E0_1600;
const LIGHT: u32 = 0x01E0_17A0;

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
    fx.bzero(D, 0x36D0);
    l2::sine_table(fx, TAB);
    fx.mem.set16(D + 0x355A, param);
    fx.mem.set32(TIMER, 0x1C2);
    for o in [0xC0u32, 0xC4, 0xC8] {
        fx.mem.set32(D + o, 0);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x11, 1);
    fx.mem.set16(NODE, r);
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0xD8, t);
    for k in 0xEu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x108 + 4 * (k - 0xE), t);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    if fx.mem.i16(COUNTER) == 0x91 {
        for (k, (node, clear)) in [(0x1C, 1), (0x2E, 0), (0x36, 0), (0xC, 0)].into_iter().enumerate() {
            let h = fx.self_handle();
            let r = fx.request_node(h, node, clear);
            fx.mem.set16(NODE + 2 * k as u32, r);
        }
    }
    emit(fx, param as i64 as u64);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let call = |fx: &mut Fx, size: u32, ra: u32, g: fn(&mut Fx, u32)| {
            let s = fx.enter(size);
            match size {
                0x10 => fx.saved(s, &[(0, u64::from(ra))]),
                0x70 => fx.saved(s, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, u64::from(D + 0x40 * k)), (0x20, 0), (0x10, u64::from(k)), (0, u64::from(D))]),
                _ => fx.saved(s, &[(0x20, u64::from(ra)), (0x10, u64::from(k)), (0, u64::from(D))]),
            }
            g(fx, t);
            fx.leave(size);
        };
        if fx.mem.read_u32(t + 0x24) == MOTE {
            call(fx, 0x30, 0x01E0_1A00, mote);
        }
        if fx.mem.read_u32(t + 0x24) == PUFF {
            call(fx, 0x30, 0x01E0_1A20, puff);
        }
        if fx.mem.read_u32(t + 0x24) == PUFF_O {
            call(fx, 0x30, 0x01E0_1A40, puff_o);
        }
        if fx.mem.read_u32(t + 0x24) == SPARK {
            call(fx, 0x30, 0x01E0_1A60, spark);
        }
        if fx.mem.read_u32(t + 0x24) == ZIGZAG {
            call(fx, 0x70, 0x01E0_1A80, zigzag);
        }
        if fx.mem.read_u32(t + 0x24) == EMBER {
            call(fx, 0x30, 0x01E0_1AA0, ember);
        }
        if fx.mem.read_u32(t + 0x24) == LIGHT {
            call(fx, 0x10, 0x01E0_1AC0, light);
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x50, u64::from(D), 0x01E0_1AE4);
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
        if dlt(fx, v, D05, D, 0x01E0_1CB0, 0x01E0_1CC0) {
            let v = ee::add(fx.mem.f(D + 0xC0), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0xC0, v);
        }
        fx.mem.write_u32(D + 0xC4, 0x3DCC_CCCD);
        let v = fx.mem.f(D + 0xC8);
        if dlt(fx, v, D05, D, 0x01E0_1CF8, 0x01E0_1D08) {
            let v = ee::add(fx.mem.f(D + 0xC8), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0xC8, v);
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x15F => 1,
        0x1A4 => 4,
        0x187 => 2,
        0x16A..=0x181 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw of one live task (index `k`, `count` the live tasks so
/// far: the entry's s3 / s2, s1 = D + 0x40 k): each draw twice (mode 0,
/// then 1) into the descriptor sp + 0x70; the sparks once by
/// `quads4::sprite_spec` (spec sp + 0x60).
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
    const ZIG_F: (u32, u32, u32, u32) = (0x130, 0x10, 0x80, 3);
    if fx.mem.read_u32(t + 0x24) == MOTE {
        pair(fx, sprite_puff, SPRITE, 0x01E0_1B34);
    }
    if fx.mem.read_u32(t + 0x24) == PUFF {
        pair(fx, sprite_mote, SPRITE, 0x01E0_1B70);
    }
    if fx.mem.read_u32(t + 0x24) == PUFF_O {
        pair(fx, sprite_b, SPRITE_B, 0x01E0_1BB0);
    }
    if fx.mem.read_u32(t + 0x24) == SPARK {
        let w = fx.mem.read_u32(D + 0x10C);
        fx.mem.write_u32(sp + 0x60, w);
        fx.mem.set32(sp + 0x68, 0);
        fx.mem.set32(sp + 0x64, 0x9B);
        fx.mem.set32(sp + 0x6C, 0);
        let ot = fx.mem.read_u32(OTW);
        quads4::sprite_spec(fx, ot, t, sp + 0x60);
    }
    if fx.mem.read_u32(t + 0x24) == ZIGZAG {
        pair(fx, zigzag_draw, ZIG_F, 0x01E0_1C30);
    }
    if fx.mem.read_u32(t + 0x24) == EMBER {
        pair(fx, sprite_puff, SPRITE, 0x01E0_1C70);
    }
}

/// 0x1E00260(D, 0) (frame 0x70: ra, the entry's s4 / s3 / s2 = 0, s1 =
/// the parameter, s0 = D): the spawn colour (0x80, 0, 0, 0), the update
/// counter and the spawns. Its s0..s4 as the record leaves them are
/// tracked for the frames below it (the node calls', rand's and the
/// spawn's).
fn emit(fx: &mut Fx, s1: u64) {
    let sp = fx.enter(0x70);
    fx.saved(sp, &[(0x50, 0x01E0_19CC), (0x40, 0), (0x30, 0), (0x20, 0), (0x10, s1), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    // (s1, s2, s3, s4) of the emitter.
    let mut r: [u64; 4] = [s1, 0, 0, 0];
    let go = |fx: &mut Fx, f: u32, r: &[u64; 4], ra: u32| {
        spawn(fx, f, [u64::from(D), r[0], r[1], r[2], r[3]], ra);
    };
    if fx.mem.i16(COUNTER) == 1 {
        go(fx, LIGHT, &r, 0x01E0_02BC);
    }
    let at = |fx: &mut Fx, slot: u32, r: &[u64; 4], ra: u32| {
        let h = fx.self_handle();
        let n = fx.mem.i16(slot);
        node_position_saving(fx, h, n, sp + 0x60, u64::from(D), Some(r[0]), ra);
    };
    let angle = |fx: &mut Fx, ra: u32| {
        let a = rmod(fx, D, ra, 0x168);
        fx.mem.set16(ANGLE, a);
    };
    // POS = the node point + rand % m - h (per axis, the rand's ra).
    let jitter = |fx: &mut Fx, o: u32, m: i32, h: f32, ra: u32| {
        let v = rmod(fx, D, ra, m);
        let p = ee::sub(ee::add(fx.mem.f(sp + 0x60 + o), ee::from_i32(v)), h);
        fx.mem.setf(POS + o, p);
    };
    let at_node = |fx: &mut Fx| {
        for o in [0u32, 4, 8] {
            let w = fx.mem.read_u32(sp + 0x60 + o);
            fx.mem.write_u32(POS + o, w);
        }
    };
    at(fx, NODE, &r, 0x01E0_02D4);
    let c = fx.mem.i16(COUNTER);
    if (0x51..0x82).contains(&c) {
        r[0] = 0;
        while r[0] < 3 {
            fx.mem.set16(INDEX, 0);
            fx.mem.set32(POS + 0xC, 0);
            angle(fx, 0x01E0_0300);
            jitter(fx, 0, 0x24, 18.0, 0x01E0_031C);
            jitter(fx, 4, 0xA, 5.0, 0x01E0_0350);
            jitter(fx, 8, 0x24, 18.0, 0x01E0_0384);
            go(fx, PUFF_O, &r, 0x01E0_03C8);
            r[0] += 1;
        }
    }
    // Twenty motes at a node on the listed updates (the first list also
    // resets the index and the point's w).
    let motes = |fx: &mut Fx, r: &mut [u64; 4], slot: u32, ra_at: u32, list: &[i32], reset: bool, ras: (u32, u32)| {
        at(fx, slot, r, ra_at);
        r[0] = 0;
        let c = fx.mem.i16(COUNTER);
        if list.contains(&c) {
            while r[0] < 0x14 {
                if reset {
                    fx.mem.set16(INDEX, 0);
                    fx.mem.set32(POS + 0xC, 0);
                }
                angle(fx, ras.0);
                at_node(fx);
                go(fx, MOTE, r, ras.1);
                r[0] += 1;
            }
        }
    };
    motes(fx, &mut r, NODE, 0x01E0_03F0, &[0xB3, 0x102, 0x14D, 0x1F0, 0x215, 0x254], true, (0x01E0_044C, 0x01E0_0488));
    motes(fx, &mut r, NODE + 2, 0x01E0_04B0, &[0xDF, 0x14F, 0x1FE, 0x236], false, (0x01E0_04F0, 0x01E0_052C));
    motes(fx, &mut r, NODE + 4, 0x01E0_0558, &[0xD3, 0x23C], false, (0x01E0_0580, 0x01E0_05BC));
    motes(fx, &mut r, NODE + 6, 0x01E0_05E8, &[0xAB, 0xF8, 0x21F, 0x257], false, (0x01E0_0628, 0x01E0_0664));
    // The bursts at nodes 0 and 1: (the node call's ra, the first loop's
    // rand ras / spawn ra, its reset, the second's, the zigzags' spawn ra).
    type Burst = (u32, u32, [u32; 4], bool, [u32; 4], u32);
    let bursts: [Burst; 2] = [
        (NODE, 0x01E0_0690, [0x01E0_06C8, 0x01E0_06E4, 0x01E0_0724, 0x01E0_076C], true, [0x01E0_079C, 0x01E0_07B8, 0x01E0_07F4, 0x01E0_0838], 0x01E0_08A4),
        (NODE + 2, 0x01E0_08D8, [0x01E0_090C, 0x01E0_0928, 0x01E0_0968, 0x01E0_09B0], false, [0x01E0_09E0, 0x01E0_09FC, 0x01E0_0A38, 0x01E0_0A7C], 0x01E0_0AEC),
    ];
    for (slot, ra_at, ras1, reset1, ras2, ra_zig) in bursts {
        at(fx, slot, &r, ra_at);
        r[2] = 0;
        let c = fx.mem.i16(COUNTER);
        let grey = |fx: &mut Fx, r: &mut [u64; 4], f: u32, reset: bool, ras: [u32; 4]| {
            r[2] = 0;
            while r[2] < 0xF {
                if reset {
                    fx.mem.set16(INDEX, 0);
                    fx.mem.set32(POS + 0xC, 0);
                }
                angle(fx, ras[0]);
                jitter(fx, 0, 0x14, 10.0, ras[1]);
                r[0] = u64::from(sp + 0x64);
                let w = fx.mem.read_u32(sp + 0x64);
                fx.mem.write_u32(POS + 4, w);
                jitter(fx, 8, 0x14, 10.0, ras[2]);
                r[1] = u64::from(sp + 0x68);
                go(fx, f, r, ras[3]);
                r[2] += 1;
            }
        };
        if [0x14C, 0x14F, 0x152].contains(&c) {
            grey(fx, &mut r, PUFF, reset1, ras1);
        }
        r[2] = 0;
        if fx.mem.i16(COUNTER) == 0x14F {
            grey(fx, &mut r, SPARK, true, ras2);
            r[2] = 0;
            r[3] = 0;
            while r[2] < 7 {
                fx.mem.set16(INDEX, r[2] as i32);
                fx.mem.set16(ANGLE, r[3] as i32);
                at_node(fx);
                go(fx, ZIGZAG, &r, ra_zig);
                r[3] = i64::from((r[3] as i32 + 0x28) as i16) as u64;
                r[2] += 1;
            }
        }
    }
    if fx.mem.i32(TIMER) == 1 {
        fx.saved(sp - 0x20, &[(0, u64::from(D)), (0x10, 0x01E0_0B20)]);
        lexe::camera_slot(fx, 0, 2, 0);
        lexe::camera_slot(fx, 0, 3, 0);
    }
    fx.leave(0x70);
}

/// 0x1E01DE0(D, step, 0) (frame 0x60: the caller's s0..s4, `ra`): the
/// first free task of 180 cleared and set off at D+0x80 (as i16) with the
/// colour D+0x3504 (twice), +0x30 = D+0x3558 (b159's).
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

/// i16 `at` += `k` x (sine or cosine of i16 `deg`) through 0x1A2B80
/// (`sub`: minus).
fn wobble(fx: &mut Fx, t: u32, at: u32, deg: u32, cos: i32, k: f32, sub: bool, ra: u32) {
    let a = fx.mem.i16(t + deg);
    let s = l2::sin_deg(fx, TAB, a, cos);
    let p = ee::from_i32(fx.mem.i16(t + at));
    let f = if sub { ee::sub(p, ee::mul(k, s)) } else { ee::add(p, ee::mul(k, s)) };
    let r = fix(fx, f, ra);
    fx.mem.set16(t + at, r);
}

/// 0x1E00B40 (D, task) (b159's 0x1E004C0 with colour (0x2F, 0x20, 0x2F)).
fn mote(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x2F, 0x20, 0x2F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0);
        let r = rmod(fx, t, 0x01E0_0B84, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, 0x01E0_0BA4, 0x1E);
        fx.mem.set16(t + 0x30, r - 0xF);
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
    wobble(fx, t, 8, 0x3A, 0, 10.0, false, 0x01E0_0CCC);
    wobble(fx, t, 0xA, 0x30, 1, 4.0, false, 0x01E0_0D04);
    wobble(fx, t, 0xC, 0x3A, 1, 10.0, false, 0x01E0_0D3C);
}

/// 0x1E00D60 (D, task): a grey puff.
fn puff(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x28, 0x28, 0x28);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 2);
        let r = rmod(fx, t, 0x01E0_0DA8, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rmod(fx, t, 0x01E0_0DC4, 7);
        fx.mem.set16(t + 0x34, r);
        let r = rmod(fx, t, 0x01E0_0DE0, 0x3C);
        fx.mem.set16(t + 0x30, r - 0x1E);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0x14 {
        fade_in(fx, t, 0x60, 5);
    } else {
        fade_out(fx, t, 7, 5);
    }
    age(fx, t);
    let s = fx.mem.i16(t + 0x38);
    if s < 0x19 {
        fx.mem.set16(t + 0x38, s + 1);
    }
    wobble(fx, t, 8, 0x3A, 0, 50.0, false, 0x01E0_0EA8);
    wobble(fx, t, 0xA, 0x30, 1, 4.0, false, 0x01E0_0EE0);
    wobble(fx, t, 0xC, 0x3A, 1, 50.0, false, 0x01E0_0F18);
}

/// 0x1E00F40 (D, task): an orange puff.
fn puff_o(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x60, 0x30, 0);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 2);
        let r = rmod(fx, t, 0x01E0_0F88, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, 0x01E0_0FA8, 0x1E);
        fx.mem.set16(t + 0x30, r - 0xF);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fade_in(fx, t, 0x60, 9);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    age(fx, t);
    let n = fx.mem.i16(t + 0x32);
    if fx.div(n, 3).1 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 7 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    wobble(fx, t, 8, 0x3A, 0, 10.0, false, 0x01E0_1094);
    wobble(fx, t, 0xA, 0x30, 1, 10.0, true, 0x01E0_10CC);
    wobble(fx, t, 0xC, 0x3A, 1, 10.0, false, 0x01E0_1104);
}

/// 0x1E01120 (D, task): an orange spark rising and falling (+0xA = 20 x
/// the sine of +0x30, +0x30 up by 6).
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 1);
        let v = fx.rand_from(t, 0x01E0_114C);
        fx.mem.set16(t + 0x34, v % 4);
        fx.rand_from(t, 0x01E0_1168);
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, t, 0x01E0_1184, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    age(fx, t);
    let a = fx.mem.i16(t + 0x30) + 6;
    fx.mem.set16(t + 0x30, a);
    wobble(fx, t, 8, 0x3A, 0, 16.0, false, 0x01E0_122C);
    let a = fx.mem.i16(t + 0x30);
    let s = l2::sin_deg(fx, TAB, a, 0);
    let r = fix(fx, ee::mul(20.0, s), 0x01E0_1250);
    fx.mem.set16(t + 0xA, r);
    wobble(fx, t, 0xC, 0x3A, 1, 16.0, false, 0x01E0_1288);
}

/// 0x1E012B0 (D, task; frame 0x70): a zigzag of 8 points at D+0x3100 +
/// 0x80 index (x 20 k, z a random walk of rand % 60 - 30), turned (+0x14)
/// by the attacker's yaw - 0x4000, + rand % 0xC000 - 0x6000 past index 0.
fn zigzag(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    l2::rgb(fx, t + 0x28, 0x60, 0x30, 0);
    if fx.mem.i16(t + 0x36) == 1 {
        let h = fx.self_handle();
        fx.unit_rotation(h, sp + 0x60);
        let y = ee::from_i32(fx.mem.i32(sp + 0x64));
        let v = fix(fx, y, 0x01E0_1318);
        fx.mem.set32(t + 0x14, v.wrapping_sub(0x4000));
        let i = fx.mem.i16(t + 0x30);
        if i != 0 {
            let r = rmod(fx, D, 0x01E0_1374, 0xC000);
            let v = fx.mem.i32(t + 0x14).wrapping_add(r - 0x6000);
            fx.mem.set32(t + 0x14, v);
        }
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x38, 0x3C);
        let i = fx.mem.i16(t + 0x30);
        let base = if (0..7).contains(&i) { ZIG + 0x80 * i as u32 } else { D };
        for k in 0..8u32 {
            fx.mem.setf(base + 0x10 * k, ee::from_i32(20 * k as i32));
        }
        for k in 0..8u32 {
            let p = base + 0x10 * k;
            if k == 0 {
                fx.mem.set32(p + 8, 0);
            } else {
                let r = rmod(fx, p, 0x01E0_1504, 0x3C);
                let z = ee::sub(ee::add(fx.mem.f(p - 0x10 + 8), ee::from_i32(r)), 30.0);
                fx.mem.setf(p + 8, z);
            }
        }
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0x1C {
        fade_in(fx, t, 0xF0, 0x1E);
    } else {
        fade_out(fx, t, 5, 3);
    }
    age(fx, t);
}

/// 0x1E01600 (D, task): a dim ember (size 15 down by 1), its +0x3A rand %
/// 360 below update 0x82, else rand % 120 - 60 (attacker side 0, slot <
/// 10) or + 120.
fn ember(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x1F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xF);
        let h = fx.self_handle();
        let early = fx.mem.i16(COUNTER) < 0x82;
        let a = match (h < 0xA, early) {
            (true, true) => rmod(fx, t, 0x01E0_166C, 0x168),
            (true, false) => rmod(fx, t, 0x01E0_1690, 0x78) - 0x3C,
            (false, true) => rmod(fx, t, 0x01E0_16C8, 0x168),
            (false, false) => rmod(fx, t, 0x01E0_16EC, 0x78) + 0x78,
        };
        fx.mem.set16(t + 0x3A, a);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 8 {
        fade_in(fx, t, 0x60, 0xF);
    } else {
        fade_out(fx, t, 0x15, 0xF);
    }
    age(fx, t);
    let s = fx.mem.i16(t + 0x38);
    if s != 0 {
        fx.mem.set16(t + 0x38, s - 1);
    }
}

/// 0x1E017A0 (D, task; frame 0x10): the stage light (0x1F7F10) +0x30 from
/// 0x80 down by 10 to 0x30, up by 7 to 0x80 once the timer is under 26.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x30, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    let l = fx.mem.i16(t + 0x30);
    if fx.mem.i32(TIMER) >= 0x1A {
        fx.mem.set16(t + 0x30, if l < 0x31 { 0x30 } else { l - 0xA });
    } else {
        fx.mem.set16(t + 0x30, if l < 0x80 { l + 7 } else { 0x80 });
        if fx.mem.i16(t + 0x30) >= 0x81 {
            fx.mem.set16(t + 0x30, 0x80);
        }
    }
    let l = fx.mem.read_u8(t + 0x30);
    fx.set_stage_light(i32::from(l));
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

/// 0x1E01F00 / 0x1E020A0 (D, task, desc, mode) (b159's 0x1E01380 /
/// 0x1E01520): a camera-facing quad of half size i16 +0x38 at the task's
/// position (+ offsets / 10), UVs of sprite `id` cell +0x34, texture
/// `tex`, keyed by the mean w (0x109870).
fn sprite(fx: &mut Fx, t: u32, d: u32, mode: i32, id: i32, tex: u32, ra: u32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xB0, id, c);
        setup(fx, d, sp + 0xB0, t + 0x28, tex);
    } else {
        camera_angles_saving(fx, sp + 0x100, u64::from(OT), u64::from(d), ra);
        trans_tenth(fx, t, sp + 0xA0);
        l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, OT);
    }
    fx.leave(0x110);
}

fn sprite_puff(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    sprite(fx, t, d, mode, 0x99, D + 0x10C, 0x01E0_1F9C);
}

fn sprite_mote(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    sprite(fx, t, d, mode, 0x14, D + 0xD8, 0x01E0_213C);
}

/// 0x1E02240 (D, task, desc, mode): [`sprite_mote`] scaled 0.1 (frame
/// 0x120; b130's 0x1E02730 with sprite 0x14, texture D+0xD8).
fn sprite_b(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x120);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xC0, 0x14, c);
        setup(fx, d, sp + 0xC0, t + 0x28, D + 0xD8);
    } else {
        camera_angles_saving(fx, sp + 0x110, u64::from(OT), u64::from(d), 0x01E0_22DC);
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

/// 0x1E023F0 (D, task, desc, mode; frame 0x130): mode 0 the strip3 of
/// sprite 0x96 cell +0x34, colour +0x28, texture D+0x10C; mode 1 seven
/// quads in y = 0 between the zigzag's successive points k / k + 1 (half
/// width (20 - 2 k) / 2 and (18 - 2 k) / 2 in z), turned by the task's
/// angles +0x10 at its i16 position, each keyed by its mean w (0x109870).
fn zigzag_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    use ee::{add, div, sub};
    let sp = fx.enter(0x130);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xE0, 0x96, c);
        setup(fx, d, sp + 0xE0, t + 0x28, D + 0x10C);
    } else {
        for k in 0..3u32 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0xD0 + 4 * k, v);
        }
        l2::xform_vp(fx, sp + 0xF0, 0, t + 0x10, sp + 0xD0);
        let i = fx.mem.i16(t + 0x30);
        if !(0..7).contains(&i) {
            // The entry's s1 / s2 (never reached: indices are 0..6).
            fx.leave(0x130);
            return;
        }
        let mut s2 = ZIG + 0x80 * i as u32;
        let mut s1 = s2 + 0x10;
        for o in [0xC4u32, 0xB4, 0xA4, 0x94] {
            fx.mem.set32(sp + o, 0);
        }
        let mut f20 = 20.0f32;
        for _ in 0..7 {
            let f2 = div(f20, 2.0);
            for o in [0x90u32, 0xA0] {
                let w = fx.mem.read_u32(s1);
                fx.mem.write_u32(sp + o, w);
            }
            for o in [0xB0u32, 0xC0] {
                let w = fx.mem.read_u32(s2);
                fx.mem.write_u32(sp + o, w);
            }
            f20 = sub(f20, 2.0);
            let f1 = div(f20, 2.0);
            let v = sub(fx.mem.f(s2 + 8), f2);
            fx.mem.setf(sp + 0xB8, v);
            let v = add(fx.mem.f(s2 + 8), f2);
            fx.mem.setf(sp + 0xC8, v);
            s2 += 0x10;
            let v = sub(fx.mem.f(s1 + 8), f1);
            fx.mem.setf(sp + 0x98, v);
            let v = add(fx.mem.f(s1 + 8), f1);
            fx.mem.setf(sp + 0xA8, v);
            s1 += 0x10;
            l2::project_submit(fx, sp + 0x90, d, sp + 0xF0, OT);
        }
    }
    fx.leave(0x130);
}

#[allow(dead_code)]
const _SPR: u32 = SPR;
