//! BMODULE b611 (card 48), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02490, data area 0x1ED0000) [F] (helper C8, 31
//! Oct 2026): b546's program family - a pool of 450 tasks (0x70 bytes at
//! +0x2A0, alive +0x58, step +0x44 called through `jalr`) drawn with the
//! second library. The emitter 0x1E01190 (`emit`): on update 1 the stage
//! light (0x1E020D0, dimming to 0x30 then back); updates 0xB..0xE a
//! whole-window unit flash each (0x1E021B0, through the unit drawn into the
//! target in a packet layer, 0x1E02DB0); updates 0x1F..0x58 two sparks a
//! update from the attacker's nodes 0xC / 0x1D (0x1E019C0,
//! `billboard::ring_quads`); updates 0x42..0x115 three pairs of dots
//! circling the attacker's node 1 (0x1E01DB0, `billboard::quad`), their
//! radii growing at the update counts 0x8C / 0xA0 / 0xB4. Returns 1 on
//! update 0xA5, 4 on 0x138, 2 on 0x131, 6 on 0xB0..0x12B, 8 once the
//! 320-update timer +0xC8E0 has run out.
//!
//! Steps 0x1E01BD0 (a ring) and 0x1E01ED0 (a flare, texture (4, 0xF)) and
//! their draws are in the record but never spawned [F: no `addiu` of
//! their addresses outside the entry's draw tests]; not ported.
//!
//! Library at its own addresses: uv_rect (0x1E00080), `ring_quads`
//! (0x1E00920), `quad` (0x1E00ED0).
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0x1C2;
/// The step functions a task holds (+0x44), as the record's addresses.
const SPARK: u32 = 0x01E0_19C0;
const DOT: u32 = 0x01E0_1DB0;
const LIGHT: u32 = 0x01E0_20D0;
const FLASH: u32 = 0x01E0_21B0;
/// The return address of the entry's `jalr` into a step (0x1E02878 + 8).
const TASK_RA: u64 = 0x01E0_2880;
const SINES: u32 = D + 0xD8;
const DOTS: u32 = D + 0xC7B0;
const SLOPE: u32 = D + 0xC8B4;
const TIMER: u32 = D + 0xC8E0;
const COLOUR: u32 = D + 0xC8E4;
const LAYER: u32 = D + 0xC8F4;
const FLASH_DESC: u32 = D + 0xC8F8;
const RADII: u32 = D + 0xC904;
const GROWTH: u32 = D + 0xC944;
const NODES: u32 = D + 0xC994;
const TARGET_NODE: u32 = D + 0xC9A4;
const ANGLES: u32 = D + 0xC9BC;
const LIFTS: u32 = D + 0xC9DC;
const INDEX: u32 = D + 0xCA00;
const PAIRS: u32 = D + 0xCA02;
const COUNTER: u32 = D + 0xCA08;
const BYTES1: u32 = D + 0xCA0A;

/// The entry 0x1E02490.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0xC8EC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xCD80);
    l2::sine_table(fx, D + 0xCC0C);
    l2::sine_table(fx, SINES);
    fx.mem.set16(D + 0xCA06, param);
    fx.mem.set32(TIMER, 0x140);
    for (k, n) in [1, 0xC, 0x1D, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    let tex = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, tex);
    for k in 0..2u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xC8C0 + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(v, 0x01E0_2688);
    fx.mem.set16(D + 0xCA04, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for (table, ra, m) in [(BYTES1, 0x01E0_26B8u32, 0x1E), (D + 0xCB0A, 0x01E0_270C, 0x28)] {
        for k in 0..0x100u32 {
            let r = fx.rand_from(D, ra);
            let (_, h) = fx.div(r, m);
            fx.mem.write_u8(table + k, (h - m / 2) as u8);
        }
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(D + 0xCB0A + k, 0);
    }
    for (k, v) in [0, 0xB4, 0, 0xB4, 0].into_iter().enumerate() {
        fx.mem.set16(ANGLES + 2 * k as u32, v);
    }
    for (k, v) in [10.0f32, 25.0, 40.0, 55.0, 70.0, 60.0].into_iter().enumerate() {
        fx.mem.setf(RADII + 4 * k as u32, v);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // a3 as the calls leave it, for the unit flash's first 0x207BB0 [F].
    let mut a3 = emit(fx, param);
    // s1 counts the flash tasks (skipped here, run by the unit flash).
    let mut flashes = 0i32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 {
            continue;
        }
        if f == FLASH {
            flashes += 1;
            continue;
        }
        let regs = [D, flashes as u32];
        run_task(fx, f, slot + 0x2A0, regs, &mut a3);
    }
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == SPARK {
            cell(fx, t, D + 0xC8C0, sp + 0x70);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES1);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if fx.mem.read_u32(t + 0x44) == DOT {
            cell(fx, t, D + 0xC8C4, sp + 0x90);
            fx.mem.set16(D + 0xBE, 5);
            for o in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * o);
                fx.mem.write_u32(sp + 0x60 + 4 * o, w);
            }
            billboard::quad(fx, uv::uv_rect, D, t, 0x10, sp + 0x60);
            a3 = 0;
        }
    }
    if flashes != 0 {
        // a3 at the flash's first 0x207BB0: what the last call left, 0
        // after a quad or a ring (their projections' 0x20C670 loop counts
        // a3 down to 0) [run, as b546].
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xA5 => 1,
        0x138 => 4,
        0x131 => 2,
        0xB0..=0x12B => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The draw's head: texture `tex` into D+0x88, the atlas cell 0x14 frame
/// +0x56 (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, tex: u32, r: u32) {
    let w = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, w);
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// A step called through the entry's `jalr`; `regs` its s0 / s1 (D, the
/// flash count).
fn run_task(fx: &mut Fx, f: u32, t: u32, regs: [u32; 2], a3: &mut u32) {
    match f {
        SPARK => spark(fx, t, regs, a3),
        DOT => dot(fx, t, regs, a3),
        LIGHT => light(fx, t, regs),
        _ => panic!("b611: unknown task step {f:#x}"),
    }
}

/// 0x1E01190(D, 0): the update counter +0xCA08 and the emitters; returns
/// the a3 its last call left.
fn emit(fx: &mut Fx, param: i32) -> u32 {
    let mut a3 = 0u32;
    let sp = fx.enter(0xA0);
    let s3 = param as i64 as u64;
    fx.saved(sp, &[(0x70, 0x01E0_2840), (0x30, s3), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let entry = [D, 0, 0, s3 as u32, 0];
    if fx.mem.i16(COUNTER) == 1 {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        fx.node_position(h, s, sp + 0x90);
        let h = fx.target_handle();
        let s = fx.mem.i16(TARGET_NODE);
        fx.node_position(h, s, sp + 0x80);
        for (o, a) in [(4u32, 8u32), (0, 4)] {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x80 + a), fx.mem.f(sp + 0x90 + a)), 40.0);
            fx.mem.setf(SLOPE + o, v);
        }
    }
    if matches!(fx.mem.i16(COUNTER), 0x8C | 0xA0 | 0xB4) {
        let p = fx.mem.i16(PAIRS) + 1;
        fx.mem.set16(PAIRS, p);
    }
    let mut k = 0u32;
    while (k as i32) < fx.mem.i16(PAIRS) {
        let g = fx.mem.f(GROWTH + 4 * k);
        if ee_lt(g, 40.0) {
            fx.mem.setf(GROWTH + 4 * k, ee::add(g, 1.0));
        }
        let l = fx.mem.i16(LIFTS + 2 * k);
        if l < 0xB4 {
            fx.mem.set16(LIFTS + 2 * k, l + 3);
        }
        k += 1;
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, LIGHT, 0, &entry, 0x01E0_13DC, &mut a3);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x1F..0x59).contains(&c) {
        for (k, ra) in [(1u32, 0x01E0_1434u32), (2, 0x01E0_1468)] {
            let h = fx.self_handle();
            let s = fx.mem.i16(NODES + 2 * k);
            node_position(fx, h, s, D + 0x250, &mut a3);
            spawn(fx, SPARK, 0, &entry, ra, &mut a3);
        }
    }
    if (0xB..0xF).contains(&fx.mem.i16(COUNTER)) {
        spawn(fx, FLASH, 0, &entry, 0x01E0_14A0, &mut a3);
    }
    if fx.mem.i16(COUNTER) < 0x9B {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        node_position(fx, h, s, D + 0x250, &mut a3);
        fx.mem.setf(D + 0x254, 30.0);
    }
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES);
    node_position(fx, h, s, sp + 0x80, &mut a3);
    let c = fx.mem.i16(COUNTER);
    if (0x42..0x116).contains(&c) {
        for k in 0..3u32 {
            fx.mem.set16(INDEX, k as i32);
            let p = DOTS + 0x10 * k;
            let regs = [D + 0x10 * k, D + 2 * k, D + 4 * k, sp + 0x88, k];
            for (half, lift, ra) in [(0, 7.0f32, 0x01E0_16ECu32), (0xB4, 11.0, 0x01E0_18A8)] {
                for o in 0..3 {
                    fx.mem.write_u32(p + 4 * o, 0);
                }
                let r = fx.mem.f(RADII + 4 * k);
                let a = i32::from((fx.mem.i16(ANGLES + 2 * k) + half) as i16);
                let s = l2::sin_deg(fx, SINES, a, 0);
                let v = ee::add(fx.mem.f(p + 8), ee::mul(r, s));
                fx.mem.setf(p + 8, v);
                let s = l2::sin_deg(fx, SINES, a, 1);
                let v = ee::add(fx.mem.f(p), ee::mul(r, s));
                fx.mem.setf(p, v);
                let b = fx.mem.i16(LIFTS + 2 * k);
                let s = l2::sin_deg(fx, SINES, b, 0);
                let base = if half == 0 { 35.0 } else { 40.0 };
                let y = ee::add(lift, ee::add(base, ee::mul(r, s)));
                let v = ee::add(fx.mem.f(p + 4), y);
                fx.mem.setf(p + 4, v);
                let v = ee::add(fx.mem.f(p), fx.mem.f(sp + 0x80));
                fx.mem.setf(p, v);
                let z = ee::add(fx.mem.f(sp + 0x88), ee::mul(fx.mem.f(SLOPE + 4), fx.mem.f(GROWTH + 4 * k)));
                let v = ee::add(fx.mem.f(p + 8), z);
                fx.mem.setf(p + 8, v);
                for o in 0..4 {
                    let w = fx.mem.read_u32(p + 4 * o);
                    fx.mem.write_u32(D + 0x250 + 4 * o, w);
                }
                spawn(fx, DOT, 0, &regs, ra, &mut a3);
            }
            let a = fx.mem.i16(ANGLES + 2 * k);
            let a = if k % 2 == 1 { a + 0xA } else { a - 0xA };
            fx.mem.set16(ANGLES + 2 * k, a);
            let a = fx.mem.i16(ANGLES + 2 * k);
            if (k % 2 == 1 && a >= 0x168) || (k % 2 == 0 && a < -0x167) {
                let (_, m) = fx.div(a, 0x168);
                fx.mem.set16(ANGLES + 2 * k, m);
            }
        }
    }
    fx.leave(0xA0);
    a3
}

/// 0x1F6830 as [`Fx::node_position`]; a valid handle's object index runs
/// 0x1F40D0, which leaves a3 = (index < 0x54) [F].
fn node_position(fx: &mut Fx, h: i32, s: i32, out: u32, a3: &mut u32) {
    fx.node_position(h, s, out);
    let (side, slot) = (h >> 8, h & 0xFF);
    if (0..3).contains(&side) && slot < 0xB {
        let i = fx.object_index(side, slot);
        if i >= 0 {
            *a3 = u32::from(i < 0x54);
        }
    }
}

/// rand from `ra` with s0 = `s0`; a3 = the new state's low word - 1 (its
/// __muldi3 0x1A0EA8's low product word) [F, run A8].
fn rand_a3(fx: &mut Fx, s0: u32, ra: u32, a3: &mut u32) -> i32 {
    let r = fx.rand_from(s0, ra);
    *a3 = (fx.rand_state() as u32).wrapping_sub(1);
    r
}

fn ee_lt(a: f32, b: f32) -> bool {
    crate::exe::ee_lt(a, b)
}

/// 0x1E02C20(D, step, arg): the first free task cleared and set off at
/// D+0x250 with the colour +0xC8E4 (twice), the dot index +0xCA00; `regs`
/// the caller's s0..s4 (u32::MAX as s3: 64-bit all ones), `ra` the return
/// address.
fn spawn(fx: &mut Fx, f: u32, arg: u32, regs: &[u32; 5], ra: u32, a3: &mut u32) -> i32 {
    let sp = fx.enter(0x60);
    let r = |k: usize| if k == 3 && regs[3] == u32::MAX { u64::MAX } else { u64::from(regs[k]) };
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, r(4)), (0x30, r(3)), (0x20, r(2)), (0x10, r(1)), (0, r(0))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                *a3 = u32::from(b);
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, arg);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand % m (HI) from a step's rand returning to `ra` (s0 = the task).
fn rand_mod(fx: &mut Fx, t: u32, ra: u32, m: i32, a3: &mut u32) -> i32 {
    let v = rand_a3(fx, t, ra, a3);
    fx.div(v, m).1
}

/// The alpha byte `a` up by `up` while below `max` while `rising`, else
/// down by `down` while at least `min`, else the task ends.
fn fade(fx: &mut Fx, t: u32, a: u32, rising: bool, up: (u8, u8), down: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if rising {
        if v < up.0 {
            fx.mem.write_u8(t + a, v.wrapping_add(up.1));
        }
    } else if v >= down.0 {
        fx.mem.write_u8(t + a, v.wrapping_sub(down.1));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E019C0: a spark ring (quads of size 2, a random cell 0..3 frame) of
/// radius 20..119 (10..29 from update 0x46), turned randomly, row +8 1..8,
/// growing by +0xE, white, fading in by 0x2A until its life +6 (2..4),
/// then out by 0xB.
fn spark(fx: &mut Fx, t: u32, regs: [u32; 2], a3: &mut u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(regs[1])), (0, u64::from(regs[0]))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4000_0000);
        let r = rand_a3(fx, t, 0x01E0_19F0, a3);
        fx.mem.set16(t + 0x56, r.wrapping_rem(4));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        *a3 = 0xFF;
        for (o, ra) in [(0x20u32, 0x01E0_1A24u32), (0x24, 0x01E0_1A40), (0x28, 0x01E0_1A5C)] {
            let v = rand_mod(fx, t, ra, 0xFFFF, a3);
            fx.mem.set32(t + o, v);
        }
        let v = if fx.mem.i16(COUNTER) < 0x46 { rand_mod(fx, t, 0x01E0_1A8C, 0x64, a3) + 0x14 } else { rand_mod(fx, t, 0x01E0_1AB4, 0x14, a3) + 0xA };
        fx.mem.set16(t + 0x50, v);
        let v = rand_mod(fx, t, 0x01E0_1AD8, 0x168, a3);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_1AF4, 3, a3);
        fx.mem.set16(t + 0xE, v + 1);
        let r = rand_a3(fx, t, 0x01E0_1B14, a3);
        fx.mem.set16(t + 8, r.wrapping_rem(8) + 1);
        let v = rand_mod(fx, t, 0x01E0_1B34, 3, a3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, 0x63, rising, (0x80, 0x2A), (0xD, 0xB));
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E01DB0: a dot (a quad of size 70 growing to 90, cell frame 2..7),
/// pale blue, fading in by 0x60 for 0x14 updates then out by 9, facing the
/// camera (0x205580 into +0x20).
fn dot(fx: &mut Fx, t: u32, regs: [u32; 2], a3: &mut u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(regs[0]))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x46);
        fx.mem.set16(t + 0xA, 0x46);
        let v = rand_mod(fx, t, 0x01E0_1DDC, 6, a3);
        fx.mem.set16(t + 0x56, v + 2);
        fx.mem.write_u32(t + 0x34, 0x3F00_0000);
        l2::rgb(fx, t + 0x60, 0xA0, 0xA0, 0xFF);
        *a3 = 0xFF;
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x14;
    fade(fx, t, 0x63, rising, (0xC0, 0x60), (0xB, 9));
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s < 0x5A {
                fx.mem.set16(t + o, s + 1);
            }
        }
    }
    // 0x205580's frame: ra, the step's s1 (the caller's: the flash count),
    // s0 = the task.
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, 0x01E0_1EA8), (0x10, u64::from(regs[1])), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E020D0: the stage light +0x50 from 0x80 down by 10 to 0x30 while
/// the update count is below 0x12C, then back up by 10 to 0x80; at the
/// timer's 3 the light 0x80 and the task ends.
fn light(fx: &mut Fx, t: u32, _regs: [u32; 2]) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let v = fx.mem.i16(t + 0x50);
    if fx.mem.i16(COUNTER) < 0x12C {
        fx.mem.set16(t + 0x50, if v < 0x31 { 0x30 } else { v - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if v < 0x80 { v + 0xA } else { 0x80 });
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if fx.mem.read_u32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
    fx.leave(0x20);
}

/// 0x1E021B0(D, t): the flash's frame sprite (the target over the window,
/// the colour +0x60, UVs inset by the size +8 / +0xA over 512) into the
/// descriptor at +0xC8F8, no ordering table; growing by 30 from update
/// 0x24, alpha rising by 9 below 0x80 for 0xE updates, from 0x29 falling
/// by 7. `regs` the unit flash's s0 / s1 (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_2EBC), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f21 / f20: the runner's (0) [H].
    fx.mem.write_u32(sp + 4, 0);
    fx.mem.write_u32(sp, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x30, 0x30, 0x30);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, xy) in [(0x20u32, 0x6FF8u16), (0x30, 0x8FF8)] {
        fx.mem.write_u16(d + at, xy);
        fx.mem.write_u16(d + at + 2, xy);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    fx.mem.setf(d + 0x18, ee::div(w, 512.0));
    fx.mem.setf(d + 0x1C, ee::div(h, 512.0));
    fx.mem.setf(d + 0x28, ee::div(ee::sub(512.0, w), 512.0));
    fx.mem.setf(d + 0x2C, ee::div(ee::sub(512.0, h), 512.0));
    fx.submit(d, 0, 0);
    if fx.mem.i16(t + 0x54) >= 0x24 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o) + 0x1E;
            fx.mem.set16(t + o, s);
        }
    }
    let n = fx.mem.i16(t + 0x54);
    if n < 0xE {
        fade(fx, t, 0x63, true, (0x80, 9), (0, 0));
    } else if n >= 0x29 {
        fade(fx, t, 0x63, false, (0, 0), (9, 7));
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x40);
}

/// 0x1E02DB0(D): in a packet layer (0x2078A0) the attacker drawn into the
/// target (0x1F8270 filter 0, its TEX0 / TEX1 into the sprite descriptor
/// at sp + 0x58; ALPHA 0x48), TEXFLUSH before and after; every flash task
/// stepped (0x1E021B0); the layer linked into the ordering table at key 0
/// when any task was alive.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xB0);
    let d = sp + 0x58;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    // a0..a3 at the first 0x207BB0: 5, the descriptor, 0x26, the caller's.
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x98, sp + 0xA0);
    fx.texflush(regs);
    let mut alive = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        alive += 1;
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            flash(fx, slot + 0x2A0, [k, alive]);
        }
    }
    if alive != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}
