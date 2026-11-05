//! BMODULE b136 (card 310), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E019D0) [F]. The entry
//! takes (D, param). A task pool of 180 tasks of 0x40 bytes at D+0x284
//! (alive +0x36, step +0x24 as a record address), spawned by 0x1E01F00
//! (b636's spawn with this record's offsets), b064's family: steps
//! 0x1E008C0 (b064's violet mote, word for word but the sine table
//! D+0x3664), two puffs 0x1E00AE0 / 0x1E014B0, a rising flare 0x1E00CA0
//! that keeps a 10-point trail and bursts into puffs, three flickers
//! 0x1E01670 / 0x1E01780 / 0x1E01840, and an object op (0x1F8380, inline
//! in the entry for tasks of step 0x1E01950). Drawn by the record's
//! 9-quad crown (0x1E020D0), a scaled turned sprite (0x1E02400), a set-up
//! only tint (0x1E02020, mode 0) and 58c265bfc632 `quads4::sprite_spec`.
//! From update 95 every tenth update a flare at the attacker's node (0x23
//! / 0xB / 0x10 / 0x15 / 0x1A). Returns 1 on update 120, 3 on 230, 6 on
//! 131..224, 8 once the 258-update timer D+0x3600 has run out.
use crate::b076::{node_position_saving, request_node_saving, unit_position_saving};
use crate::blib::{quads4, spawn as lspawn};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0xB4;
const POOL: u32 = D + 0x284;
const POS: u32 = D + 0x80;
const TEX: u32 = D + 0xD0;
const HIST: u32 = D + 0x2FC0;
const TIMER: u32 = D + 0x3600;
const COLOUR: u32 = D + 0x3604;
const OTW: u32 = D + 0x360C;
const MEAN: u32 = D + 0x3634;
const NODE: u32 = D + 0x3648;
const ANGLE: u32 = D + 0x3652;
const SEL: u32 = D + 0x3658;
const PARAM: u32 = D + 0x365A;
const COUNTER: u32 = D + 0x365C;
const INDEX: u32 = D + 0x3660;
const TAB: u32 = D + 0x3664;

const DRIFT: u32 = 0x01E0_08C0;
const PUFF: u32 = 0x01E0_0AE0;
const FLARE: u32 = 0x01E0_0CA0;
const PUFF_B: u32 = 0x01E0_14B0;
const FLICKER: u32 = 0x01E0_1670;
const GLOW: u32 = 0x01E0_1780;
const FLICKER_B: u32 = 0x01E0_1840;
const OBJECT_OP: u32 = 0x01E0_1950;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x110);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
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

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x37D0);
    l2::sine_table(fx, TAB);
    fx.mem.set16(PARAM, param);
    fx.mem.set32(TIMER, 0x102);
    for o in [0xC0u32, 0xC4, 0xC8] {
        fx.mem.set32(D + o, 0);
    }
    for k in 0..0x10u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    0
}

/// The steps, their frames and the entry's return address after the call.
const STEPS: [(u32, u32, u32); 7] = [
    (DRIFT, 0x30, 0x01E0_1A98),
    (PUFF, 0x30, 0x01E0_1AB8),
    (FLARE, 0x60, 0x01E0_1AD8),
    (PUFF_B, 0x30, 0x01E0_1AF8),
    (FLICKER, 0x20, 0x01E0_1B18),
    (GLOW, 0x20, 0x01E0_1B38),
    (FLICKER_B, 0x20, 0x01E0_1B58),
];

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        for &(a, size, ra) in STEPS.iter() {
            if fx.mem.read_u32(t + 0x24) == a {
                let s = fx.enter(size);
                // The entry's s0 = k, s1 = 0, s2 = D.
                match size {
                    0x20 => fx.saved(s, &[(0x10, u64::from(ra)), (0, u64::from(k))]),
                    0x30 => fx.saved(s, &[(0x20, u64::from(ra)), (0x10, 0), (0, u64::from(k))]),
                    _ => fx.saved(s, &[(0x30, u64::from(ra)), (0x20, u64::from(D)), (0x10, 0), (0, u64::from(k))]),
                }
                match a {
                    DRIFT => drift(fx, t),
                    PUFF => puff(fx, t, (0xCF, 0x50, 0x50), 6, 13.0, 0),
                    FLARE => flare(fx, t),
                    PUFF_B => puff(fx, t, (0x4F, 0x1F, 0x1F), 7, 7.0, PUFF_B - PUFF),
                    FLICKER => flicker(fx, t, (0x8F, 0x1F, 0x10), 0x11, 0xA, 0),
                    GLOW => glow(fx, t),
                    _ => flicker(fx, t, (0x2F, 0x1F, 0x20), 0x10, 0xE, FLICKER_B - FLICKER),
                }
                fx.leave(size);
            }
        }
        if fx.mem.read_u32(t + 0x24) == OBJECT_OP {
            for (to, from) in [(0x10Cu32, 0x2600u32), (0x108, 0x2608)] {
                let w = fx.mem.read_u32(0x01E0_0000 + from);
                fx.mem.write_u32(sp + to, w);
            }
            fx.boundary(0x001F_8380, "object op 0x1F8380", [6, 2, sp + 0x10C, sp + 0x108]);
            if fx.mem.i16(COUNTER) == 0x5E {
                fx.mem.set16(t + 0x36, 0);
            }
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x50, u64::from(TASKS), 0x01E0_1BDC);
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
        for (o, ra) in [(0xC0u32, 0x01E0_1DE8u32), (0xC8, 0x01E0_1E30)] {
            let v = fx.mem.f(D + o);
            fx.fix_residue(v, ra);
            fx.saved(sp - 0x10, &[(0, u64::from(ra + 0x10))]);
            if f64::from(v) < 0.5 {
                let r = ee::add(v, f32::from_bits(0x3C23_D70A));
                fx.mem.setf(D + o, r);
            }
            if o == 0xC0 {
                fx.mem.write_u32(D + 0xC4, 0x3DCC_CCCD);
            }
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0x78 {
        1
    } else if c == 0xE6 {
        3
    } else if (0x83..0xE1).contains(&c) {
        6
    } else if fx.mem.i32(TIMER) > 0 {
        0
    } else {
        8
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// i16 `at` += `k` x (sine or cosine of i16 `deg`) through 0x1A2B80.
fn wobble(fx: &mut Fx, t: u32, at: u32, deg: u32, cos: i32, k: f32, ra: u32) {
    let a = fx.mem.i16(t + deg);
    let s = l2::sin_deg(fx, TAB, a, cos);
    let f = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(k, s));
    let r = fix(fx, f, ra);
    fx.mem.set16(t + at, r);
}

/// 0x1E008C0 (D, task): b064's violet mote drifting about its point.
fn drift(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x3F, 0x30, 0x3F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0);
        let r = rmod(fx, t, 0x01E0_0904, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, 0x01E0_0924, 0x1E);
        fx.mem.set16(t + 0x30, r - 0xF);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x60 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(9));
        }
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_sub(0xA));
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
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
    wobble(fx, t, 8, 0x3A, 0, 10.0, 0x01E0_0A4C);
    wobble(fx, t, 0xA, 0x30, 1, 4.0, 0x01E0_0A84);
    wobble(fx, t, 0xC, 0x3A, 1, 10.0, 0x01E0_0ABC);
}

/// 0x1E00AE0 / 0x1E014B0 (D, task): a puff: colour `rgb`, size `size`,
/// random angles +0x3A / +0x30 and cell rand % 4; alpha up by 0x14 (under
/// 0xA0) for 8 updates, then down by 5 (the task ends under 7); swaying
/// by `k` (sin +0x3A, cos +0x30, cos +0x3A). `d`: the record offset of
/// the step from 0x1E00AE0.
fn puff(fx: &mut Fx, t: u32, rgb: (i32, i32, i32), size: i32, k: f32, d: u32) {
    l2::rgb(fx, t + 0x28, rgb.0, rgb.1, rgb.2);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, size);
        let r = rmod(fx, t, 0x01E0_0B28 + d, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let v = fx.rand_from(t, 0x01E0_0B44 + d);
        fx.mem.set16(t + 0x34, v % 4);
        let r = rmod(fx, t, 0x01E0_0B60 + d, 0x168);
        fx.mem.set16(t + 0x30, r);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 8 {
        if a < 0xA0 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x14));
        }
    } else if a >= 7 {
        fx.mem.write_u8(t + 0x2B, a.wrapping_sub(5));
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    age(fx, t);
    wobble(fx, t, 8, 0x3A, 0, k, 0x01E0_0C0C + d);
    wobble(fx, t, 0xA, 0x30, 1, k, 0x01E0_0C44 + d);
    wobble(fx, t, 0xC, 0x3A, 1, k, 0x01E0_0C7C + d);
}

/// 0x1E01670 / 0x1E01840 (D, task): a flicker: on its first update colour
/// `rgb`, +0x38 0 and random +0x10 / +0x14 / +0x18 (rand % 0xFFFF); alpha
/// up by 0x40 for 2 updates, then down by `down` (the task ends under
/// `floor`); +0x14 += 0x400, +0x38 and the age count up.
fn flicker(fx: &mut Fx, t: u32, rgb: (i32, i32, i32), floor: u8, down: u8, d: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0);
        l2::rgb(fx, t + 0x28, rgb.0, rgb.1, rgb.2);
        for (k, ra) in [0x01E0_16ACu32, 0x01E0_16C8, 0x01E0_16E4].into_iter().enumerate() {
            let r = rmod(fx, t, ra + d, 0xFFFF);
            fx.mem.set32(t + 0x10 + 4 * k as u32, r);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 2 {
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
    } else if a >= floor {
        fx.mem.write_u8(t + 0x2B, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let r = fx.mem.i32(t + 0x14).wrapping_add(0x400);
    fx.mem.set32(t + 0x14, r);
    let s = fx.mem.i16(t + 0x38) + 1;
    fx.mem.set16(t + 0x38, s);
    age(fx, t);
}

/// 0x1E01780 (D, task): a glow: colour (0x8F, 0x1F, 0xF0), +0x38 5,
/// angles (0x4000, 0x400, 0); alpha up by 0x26 for 5 updates, then down
/// by 10 (the task ends under 0x11).
fn glow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 5);
        l2::rgb(fx, t + 0x28, 0x8F, 0x1F, 0xF0);
        fx.mem.set32(t + 0x10, 0x4000);
        fx.mem.set32(t + 0x14, 0x400);
        fx.mem.set32(t + 0x18, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 5 {
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x26));
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a.wrapping_sub(0xA));
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let s = fx.mem.i16(t + 0x38) + 1;
    fx.mem.set16(t + 0x38, s);
    age(fx, t);
}

/// The task's i16 position as floats at D+0x80 (the spawn point).
fn point_from(fx: &mut Fx, t: u32) {
    for c in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * c));
        fx.mem.setf(POS + 4 * c, v);
    }
}

/// D+0x88 = the task's z + rand % 14 - 7 (rand's s0 the loop's `s0`).
fn jitter_z(fx: &mut Fx, t: u32, s0: u32, ra: u32) {
    let r = rmod(fx, s0, ra, 0xE);
    let z = fx.mem.i16(t + 4) + r - 7;
    fx.mem.setf(POS + 8, ee::from_i32(z));
}

/// 0x1E00CA0 (D, task): the flare: white, a random angle +0x3A, alpha
/// 0xFF; its point pushed into trail +0x30 (D+0x2FC0 + 0xA0 i, 10
/// points); it flies along the attacker's facing (8 a update; with
/// param 1 after 22 updates back and turned about +0x3A), bursts on update
/// 21 (param 1: 20 puffs, two glows) or 22 (13 dark puffs, 10 puffs), and
/// flickers (three a update, two jittered in z) until 29 (param 1) / 23;
/// from update 28 its alpha falls by 0x33 (the task ends under 0x11).
fn flare(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    l2::rgb(fx, t + 0x28, 0xFF, 0xFF, 0xFF);
    if fx.mem.i16(t + 0x36) == 1 {
        let s0 = entry_s0(fx);
        let r = rmod(fx, s0, 0x01E0_0CE8, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.write_u8(t + 0x2B, 0xFF);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    for c in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * c));
        fx.mem.setf(sp + 0x40 + 4 * c, v);
    }
    let i = fx.mem.i16(t + 0x30);
    if (0..10).contains(&i) {
        lspawn::history(fx, HIST + 0x90 + 0xA0 * i as u32, 0xA, sp + 0x40);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x50);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x54)), 180.0);
    let yaw = fix(fx, y, 0x01E0_0EF0) as i16 as i32;
    let back = |fx: &mut Fx, ra: u32, add: bool| {
        let c = l2::sin_deg(fx, TAB, yaw - 0xB4, 1);
        let f = ee::mul(8.0, ee::neg(c));
        let z = ee::from_i32(fx.mem.i16(t + 4));
        let r = fix(fx, if add { ee::add(z, f) } else { ee::sub(z, f) }, ra);
        fx.mem.set16(t + 4, r);
    };
    let flicks = |fx: &mut Fx, s0: u32, ras: [u32; 5]| {
        point_from(fx, t);
        spawn(fx, FLICKER, 0, ras[0], [s0, t]);
        jitter_z(fx, t, s0, ras[1]);
        spawn(fx, FLICKER, 0, ras[2], [s0, t]);
        jitter_z(fx, t, s0, ras[3]);
        spawn(fx, FLICKER, 0, ras[4], [s0, t]);
    };
    // The bursts: `n` spawns of `step`, each at a random angle D+0x3652.
    let burst = |fx: &mut Fx, n: i32, step: u32, rand_ra: u32, ra: u32| {
        let mut s0 = 0i32;
        while s0 < n {
            let r = rmod(fx, s0 as u32, rand_ra, 0x168);
            fx.mem.set16(ANGLE, r);
            point_from(fx, t);
            spawn(fx, step, 0, ra, [s0 as u32, t]);
            s0 = (s0 + 1) as i16 as i32;
        }
        s0 as u32
    };
    let mut s0 = 0u32;
    let mut s0_set = false;
    if fx.mem.i16(PARAM) == 1 {
        if fx.mem.i16(t + 0x32) < 0x16 {
            back(fx, 0x01E0_0F48, true);
        } else {
            back(fx, 0x01E0_0F84, false);
            let a = fx.mem.i16(t + 0x3A);
            let s = l2::sin_deg(fx, TAB, a, 0);
            let y = ee::sub(ee::from_i32(fx.mem.i16(t + 2)), ee::mul(8.0, s));
            let r = fix(fx, y, 0x01E0_0FBC);
            fx.mem.set16(t + 2, r);
            let a = fx.mem.i16(t + 0x3A);
            let c = l2::sin_deg(fx, TAB, a, 1);
            let x = ee::sub(ee::from_i32(fx.mem.i16(t)), ee::mul(8.0, c));
            let r = fix(fx, x, 0x01E0_0FF4);
            fx.mem.set16(t, r);
        }
        if fx.mem.i16(t + 0x32) == 0x15 {
            s0 = burst(fx, 0x14, PUFF, 0x01E0_1018, 0x01E0_1078);
            s0_set = true;
            point_from(fx, t);
            spawn(fx, GLOW, 0, 0x01E0_10EC, [s0, t]);
            spawn(fx, GLOW, 0, 0x01E0_1100, [s0, t]);
        }
        if fx.mem.i16(t + 0x32) < 0x1D {
            let s = if s0_set { s0 } else { entry_s0(fx) };
            flicks(fx, s, [0x01E0_1160, 0x01E0_1168, 0x01E0_11A4, 0x01E0_11AC, 0x01E0_11E8]);
        }
    } else {
        if fx.mem.i16(t + 0x32) < 0x17 {
            back(fx, 0x01E0_1234, true);
        }
        if fx.mem.i16(t + 0x32) == 0x16 {
            burst(fx, 0xD, PUFF_B, 0x01E0_1258, 0x01E0_12B8);
            s0 = burst(fx, 0xA, PUFF, 0x01E0_12EC, 0x01E0_134C);
            s0_set = true;
        }
        if fx.mem.i16(t + 0x32) < 0x17 {
            let s = if s0_set { s0 } else { entry_s0(fx) };
            flicks(fx, s, [0x01E0_13D0, 0x01E0_13D8, 0x01E0_1414, 0x01E0_141C, 0x01E0_1458]);
        }
    }
    age(fx, t);
    if fx.mem.i16(t + 0x32) >= 0x1C {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_sub(0x33));
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
}

/// The flare step's s0 before its loops: the entry's s0 (the task index),
/// as its frame saved it.
fn entry_s0(fx: &mut Fx) -> u32 {
    fx.mem.read_u32(fx.sp)
}

/// 0x1E01F00(D, step, arg): the first free task cleared and set off at
/// D+0x80 (as i16) with the colour D+0x3604 (twice), +0x30 = D+0x3658; 1
/// when one was free. Its frame saves the caller's s0 / s1 (`s01`), s2 =
/// D, s3 / s4 (the entry's s3 and the runner's s4: 0 [H]) and `ra`.
fn spawn(fx: &mut Fx, step: u32, arg: u32, ra: u32, s01: [u32; 2]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x20, u64::from(D)), (0x10, u64::from(s01[1])), (0, u64::from(s01[0]))]);
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
            let a = fx.mem.i16(SEL);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, arg);
            fx.mem.write_u32(t + 0x24, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E002E0(D, 0): the update counter, the node point and the spawns
/// (frame 0x50: ra, the entry's s2 = D, s1 = 0, s0 = param).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_1A64), (0x20, u64::from(D)), (0x10, 0), (0, p)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE);
    node_position_saving(fx, h, n, sp + 0x40, p, Some(0), 0x01E0_032C);
    const PICKS: [(i32, i32, u32); 10] = [
        (0x5E, 0x23, 0x01E0_0354),
        (0x68, 0xB, 0x01E0_0380),
        (0x72, 0x10, 0x01E0_03AC),
        (0x7C, 0x15, 0x01E0_03D8),
        (0x86, 0x1A, 0x01E0_0404),
        (0x90, 0x23, 0x01E0_0430),
        (0x9A, 0xB, 0x01E0_045C),
        (0xA4, 0x10, 0x01E0_0488),
        (0xAE, 0x15, 0x01E0_04B4),
        (0xB8, 0x1A, 0x01E0_04E0),
    ];
    for (at, node, ra) in PICKS {
        if fx.mem.i16(COUNTER) == at {
            let h = fx.self_handle();
            let r = request_node_saving(fx, h, node, 1, [Some(p), Some(0), Some(u64::from(D))], ra);
            fx.mem.set16(NODE, r);
        }
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE);
    node_position_saving(fx, h, n, sp + 0x40, p, Some(0), 0x01E0_04FC);
    let c = fx.mem.i16(COUNTER);
    let (mut s0, mut s1) = (param as u32, 0u32);
    if (0x5F..=0xB9).contains(&c) && (c - 0x5F) % 10 == 0 {
        let i = fx.mem.i16(INDEX);
        fx.mem.set16(SEL, i);
        s0 = sp + 0x44;
        s1 = sp + 0x48;
        for k in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        spawn(fx, FLARE, 0, 0x01E0_05B4, [s0, s1]);
        let i = fx.mem.i16(INDEX);
        if (0..10).contains(&i) {
            let a = HIST + 0xA0 * i as u32;
            for e in 0..10u32 {
                for k in 0..3u32 {
                    let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
                    fx.mem.write_u32(a + 0x10 * e + 4 * k, w);
                }
            }
        }
        for ra in [0x01E0_0844u32, 0x01E0_0858, 0x01E0_086C] {
            spawn(fx, FLICKER, 0, ra, [s0, s1]);
        }
        let i = fx.mem.i16(INDEX) + 1;
        fx.mem.set16(INDEX, i);
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, OBJECT_OP, 0, 0x01E0_089C, [s0, s1]);
    }
    fx.leave(0x50);
}

/// The draws of the task (each draw function's frame saves the entry's
/// s0 = the task's pool base, s1 = the count, s2 = D, s3 = k; s4.. the
/// runner's: 0 [H]).
fn draw(fx: &mut Fx, sp: u32, t: u32, k: u32, count: u32) {
    let s = fx.mem.read_u32(t + 0x24);
    let d = sp + 0x78;
    let saves = |fx: &mut Fx, size: u32, ra: u32| {
        let f = fx.sp - size;
        let (r, s3) = match size {
            0x150 => (0x90, 0x40),
            0x130 => (0x50, 0x30),
            _ => (0x30, 0x40),
        };
        if size == 0x50 {
            fx.saved(f, &[(r, u64::from(ra)), (0x20, u64::from(D)), (0x10, u64::from(count)), (0, u64::from(D + 0x40 * k))]);
        } else {
            fx.saved(f, &[(r, u64::from(ra)), (s3, u64::from(k)), (s3 - 0x10, u64::from(D)), (s3 - 0x20, u64::from(count)), (s3 - 0x30, u64::from(D + 0x40 * k))]);
        }
    };
    let crown_pair = |fx: &mut Fx, ra: u32| {
        saves(fx, 0x150, ra);
        crown(fx, t, d, 0);
        saves(fx, 0x150, ra + 0x14);
        crown(fx, t, d, 1);
    };
    if s == DRIFT {
        crown_pair(fx, 0x01E0_1C2C);
    }
    if s == PUFF {
        saves(fx, 0x130, 0x01E0_1C68);
        sprite(fx, t, d, 0);
        saves(fx, 0x130, 0x01E0_1C7C);
        sprite(fx, t, d, 1);
    }
    if s == FLARE {
        saves(fx, 0x50, 0x01E0_1CA8);
        tint(fx, t, d, 0);
        saves(fx, 0x50, 0x01E0_1CBC);
        tint(fx, t, d, 1);
    }
    if s == PUFF_B {
        let w = fx.mem.read_u32(TEX + 0x3C);
        fx.mem.write_u32(sp + 0x60, w);
        fx.mem.set32(sp + 0x68, 0);
        fx.mem.set32(sp + 0x64, 0x99);
        fx.mem.set32(sp + 0x6C, 0);
        let ot = fx.mem.read_u32(OTW);
        quads4::sprite_spec(fx, ot, t, sp + 0x60);
    }
    if s == FLICKER {
        crown_pair(fx, 0x01E0_1D28);
    }
    if s == GLOW {
        crown_pair(fx, 0x01E0_1D68);
    }
    if s == FLICKER_B {
        crown_pair(fx, 0x01E0_1DA8);
    }
}

/// Mode 0 of the draws: rect `id` (cell +0x34) into the local `r`, the
/// strip3 header, colour +0x28, UVs (order `order`), texture word `tex`.
fn setup(fx: &mut Fx, t: u32, d: u32, r: u32, id: i32, order: i32, tex: u32) {
    let c = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, c);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    l2::uv_strip3(fx, d, r, order);
    fx.desc_texture(d, tex, 1);
}

/// 0x1E020D0(D, task, desc, mode): a crown of 9 quads (40 degrees each)
/// between radius i16 +0x38 - 10 (y 0) and +0x38 (y rand % 10 - 5, rand's
/// frame s0 = desc), scaled (0.8, 1, 0.8) and turned by the task's angles
/// +0x10 at its position; UVs of sprite 0x97 cell +0x34, texture D+0x10C;
/// each keyed by its mean w (D+0x3634) on D+0x360C.
fn crown(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x150);
    if mode == 0 {
        let w = fx.mem.read_u32(TEX + 0x3C);
        setup(fx, t, d, sp + 0x140, 0x97, 0, w);
    } else {
        for c in 0..3u32 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * c));
            fx.mem.setf(sp + 0xE0 + 4 * c, v);
        }
        l2::set3(fx, sp + 0xF0, f32::from_bits(0x3F4C_CCCD), 1.0, f32::from_bits(0x3F4C_CCCD));
        l2::xform_vp(fx, sp + 0x100, sp + 0xF0, t + 0x10, sp + 0xE0);
        for o in [0xA4u32, 0xB4, 0xC4, 0xD4] {
            fx.mem.set32(sp + o, 0);
        }
        let mut a = 0i32;
        for _ in 0..9 {
            for (deg, inner, outer) in [((fx.mem.i16(t + 0x3A) + a) as i16 as i32, 0xA0u32, 0xC0u32), ((fx.mem.i16(t + 0x3A) + 0x28 + a) as i16 as i32, 0xB0, 0xD0)] {
                let c = l2::sin_deg(fx, TAB, deg, 1);
                let s = l2::sin_deg(fx, TAB, deg, 0);
                let r = fx.mem.i16(t + 0x38);
                fx.mem.setf(sp + outer, ee::mul(c, ee::from_i32(r)));
                fx.mem.setf(sp + outer + 8, ee::mul(s, ee::from_i32(r)));
                fx.mem.setf(sp + inner, ee::mul(c, ee::from_i32(r - 0xA)));
                fx.mem.setf(sp + inner + 8, ee::mul(s, ee::from_i32(r - 0xA)));
            }
            for (o, ra) in [(0xC4u32, 0x01E0_2334u32), (0xD4, 0x01E0_2360)] {
                let r = rmod(fx, d, ra, 0xA);
                fx.mem.setf(sp + o, ee::from_i32(r - 5));
            }
            if l2::project(fx, sp + 0xA0, SPR, sp + 0x100, d, MEAN) {
                let m = fx.mem.f(MEAN);
                fx.fix_residue(m, 0x01E0_23A8);
                let key = ee::fixunssfsi(m) as i32;
                let ot = fx.mem.read_u32(OTW);
                fx.submit(d, ot, key);
            }
            a = (a + 0x28) as i16 as i32;
        }
    }
    fx.leave(0x150);
}

/// 0x1E02400(D, task, desc, mode): a quad of half size i16 +0x38 scaled
/// 0.1 and turned by the task's angles +0x10 at its position (+ offsets /
/// 10), UVs of sprite 0x99 cell +0x34, texture D+0x10C; at the far Z,
/// keyed by its mean w.
fn sprite(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x130);
    if mode == 0 {
        let w = fx.mem.read_u32(TEX + 0x3C);
        setup(fx, t, d, sp + 0xD0, 0x99, 0, w);
    } else {
        l2::camera_angles_to(fx, sp + 0x120);
        for k in 0..3u32 {
            let off = fx.mem.i16(t + 8 + 2 * k);
            fx.mult(0x6666_6667, off);
            let hi = fx.hilo().1;
            let q = (hi >> 2).wrapping_add(((off as u32) >> 31) as i32);
            let v = fx.mem.i16(t + 2 * k).wrapping_add(q);
            fx.mem.setf(sp + 0xB0 + 4 * k, ee::from_i32(v));
        }
        for k in 0..3u32 {
            fx.mem.write_u32(sp + 0xC0 + 4 * k, 0x3DCC_CCCD);
        }
        l2::xform_vp(fx, sp + 0xE0, sp + 0xC0, t + 0x10, sp + 0xB0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        if l2::project(fx, sp + 0x60, SPR, sp + 0xE0, d, sp + 0xA0) {
            l2::far_z_desc(fx, d, 0);
            let m = fx.mem.f(sp + 0xA0);
            fx.fix_residue(m, 0x01E0_25AC);
            let key = ee::fixunssfsi(m) as i32;
            fx.submit(d, OT, key);
        }
    }
    fx.leave(0x130);
}

/// 0x1E02020(D, task, desc, mode): mode 0 only: rect 0x96 (cell +0x34),
/// the strip3 header, colour +0x28, UVs (order 1), texture (4, 15); mode
/// 1 draws nothing.
fn tint(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x50);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0x40, 0x96, c);
        l2::desc_strip3(fx, d);
        l2::colour_strip3(fx, d, t + 0x28, 0);
        l2::uv_strip3(fx, d, sp + 0x40, 1);
        let w = fx.texture(4, 0xF);
        fx.desc_texture(d, w, 1);
    }
    fx.leave(0x50);
}
