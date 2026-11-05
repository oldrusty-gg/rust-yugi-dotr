//! BMODULE b202 (card 117), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02830, data area 0x1ED0000) [F] (helper C8, 31
//! Oct 2026): b546's program family - a pool of 300 tasks (0x70 bytes at
//! +0x2A0, alive +0x58, step +0x44 called through `jalr`) drawn with the
//! second library. The emitter 0x1E01880 (`emit`): on update 1 a window
//! overlay (0x1E023B0, the frame tinted over the window); updates 1..4 a
//! whole-window unit flash each (0x1E02550, through the unit drawn into
//! the target in a packet layer, 0x1E03040); on the even updates
//! 0x4E..0xC8 orbs at the attacker's node 3 (ten on 0x4E, else one;
//! 0x1E01A80, `quads4::ring_facing_flat`) that rise, sway toward the
//! target and on their update 0x4B may burst into a ring (0x1E01F10, the
//! same draw) and a band (0x1E022A0, `quads4::ring_band`). Returns 1 on
//! update 0x8C, 4 on 0x136, 3 on 0x122, 6 on 0x97..0x11C, 8 once the
//! 330-update timer +0x8630 has run out.
//!
//! Step 0x1E020A0 (a ring of quads, texture (4, 0xF), `ring_quads`) and its
//! draw are in the record but never spawned [F: no `addiu` of its address
//! outside the entry's draw test]; not ported.
//!
//! Library at its own addresses: uv_rect (0x1E00080), `ring_band`
//! (0x1E00920), `ring_facing_flat` (0x1E00D80), `ring_quads` (0x1E012D0).
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0x12C;
/// The step functions a task holds (+0x44), as the record's addresses.
const ORB: u32 = 0x01E0_1A80;
const RING: u32 = 0x01E0_1F10;
const BAND: u32 = 0x01E0_22A0;
const OVERLAY: u32 = 0x01E0_23B0;
const FLASH: u32 = 0x01E0_2550;
/// The return address of the entry's `jalr` into a step (0x1E02B30 + 8).
const TASK_RA: u64 = 0x01E0_2B38;
const SINES: u32 = D + 0x889C;
const TEX5: u32 = D + 0x8610;
const TIMER: u32 = D + 0x8630;
const COLOUR: u32 = D + 0x8634;
const LAYER: u32 = D + 0x8644;
const FLASH_DESC: u32 = D + 0x8648;
const NODES: u32 = D + 0x8664;
const TARGET_NODE: u32 = D + 0x8674;
const INDEX: u32 = D + 0x8692;
const NEXT: u32 = D + 0x8694;
const YAW: u32 = D + 0x8696;
const COUNTER: u32 = D + 0x869A;
const BYTES1: u32 = D + 0x869C;
const BYTES2: u32 = D + 0x879C;

/// The entry 0x1E02830.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0x863C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x8A10);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x8698, param);
    fx.mem.set32(TIMER, 0x14A);
    for (k, n) in [3, 0x1C, 0xD].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for (k, id) in [0xEu32, 0xF].into_iter().enumerate() {
        let tex = fx.texture(4, id as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k as u32, tex);
    }
    let tex = fx.texture(5, 0);
    fx.mem.write_u32(TEX5, tex);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(v, 0x01E0_29C4);
    fx.mem.set16(YAW, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_29F4);
        let (_, h) = fx.div(r, 0x1E);
        fx.mem.write_u8(BYTES1 + k, (h - 0xF) as u8);
    }
    for k in 0x64..0xB4u32 {
        let r = fx.rand_from(D, 0x01E0_2A4C);
        fx.mem.write_u8(BYTES1 + k, r.wrapping_rem(4) as u8);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_2AA4);
        let (_, h) = fx.div(r, 0x28);
        fx.mem.write_u8(BYTES2 + k, (h - 0x14) as u8);
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
        let regs = [D, flashes as u32, 0, slot, k];
        run_task(fx, f, slot + 0x2A0, &regs, &mut a3);
    }
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        for (f, r, rows) in [(ORB, sp + 0x70, 6), (RING, sp + 0x80, 4)] {
            if fx.mem.read_u32(t + 0x44) == f {
                cell(fx, t, r);
                let tex = fx.mem.read_u32(TEX5);
                fx.mem.write_u32(D + 0x88, tex);
                fx.mem.set16(D + 0xB8, rows);
                let a = fx.mem.i16(t + 0xA);
                fx.mem.set16(D + 0xC0, a);
                let x = fx.mem.read_u32(t + 0x10);
                fx.mem.write_u32(D + 0x20, x);
                let y = if f == ORB { ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34)) } else { fx.mem.f(t + 0x14) };
                fx.mem.setf(D + 0x24, y);
                let z = fx.mem.read_u32(t + 0x18);
                fx.mem.write_u32(D + 0x28, z);
                quads4::ring_facing_flat(fx, uv::uv_rect, D, t, 0x20, BYTES1);
                a3 = 0;
                fx.mem.set16(D + 0xB8, 0);
            }
        }
        if fx.mem.read_u32(t + 0x44) == BAND {
            for o in 0..4 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * o);
                fx.mem.write_u32(sp + 0x60 + 4 * o, w);
            }
            quads4::ring_band(fx, D, t, 0x21, sp + 0x60);
            a3 = 0;
        }
    }
    if flashes != 0 {
        // a3 at the flash's first 0x207BB0: what the last call left, 0
        // after a ring or a band (their projections' 0x20C670 loop counts
        // a3 down to 0) [run, as b546].
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x8C => 1,
        0x136 => 4,
        0x122 => 3,
        0x97..=0x11C => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The draw's head: the atlas cell 0x14 frame +0x56 (0x106960 into `r`)
/// copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// A step called through the entry's `jalr`; `regs` its s0..s4 (D, the
/// flash count, 0, the slot, its index).
fn run_task(fx: &mut Fx, f: u32, t: u32, regs: &[u32; 5], a3: &mut u32) {
    match f {
        ORB => orb(fx, t, regs, a3),
        RING => ring(fx, t, a3),
        BAND => band(fx, t, regs, a3),
        OVERLAY => overlay(fx, t, a3),
        _ => panic!("b202: unknown task step {f:#x}"),
    }
}

/// 0x1E01880(D, 0): the update counter +0x869A and the emitters; returns
/// the a3 its last call left.
fn emit(fx: &mut Fx, param: i32) -> u32 {
    let mut a3 = 0u32;
    let sp = fx.enter(0x40);
    let s3 = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_2AF8), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The caller's s0..s4 at each spawn: s0 / s1 the entry's (D, 0) or the
    // loop's (count, total), s2 = D, s3 the entry's (the parameter).
    let mut regs = [D, 0, D, s3 as u32, 0];
    let c = fx.mem.i16(COUNTER);
    if (0x4E..0xC9).contains(&c) && c.wrapping_rem(2) == 0 {
        let total = if c == 0x4E { 0xA } else { 1 };
        regs[1] = total as u32;
        for k in 0..total {
            regs[0] = k as u32;
            let h = fx.self_handle();
            let s = fx.mem.i16(NODES);
            node_position(fx, h, s, D + 0x250, &mut a3);
            let r = rand_a3(fx, D, 0x01E0_1944, &mut a3);
            let (_, m) = fx.div(r, 0x14);
            let y = ee::add(fx.mem.f(D + 0x254), ee::from_i32(m));
            fx.mem.setf(D + 0x254, y);
            let i = fx.mem.i16(NEXT);
            fx.mem.set16(INDEX, i);
            spawn(fx, ORB, 0, &regs, 0x01E0_1994, &mut a3);
            let i = fx.mem.i16(NEXT) + 1;
            fx.mem.set16(NEXT, i);
            if fx.mem.i16(NEXT) >= 0x78 {
                fx.mem.set16(NEXT, 0);
            }
        }
        regs[0] = total as u32;
    }
    let c = fx.mem.i16(COUNTER);
    if c > 0 && c < 5 {
        spawn(fx, FLASH, 0, &regs, 0x01E0_1A34, &mut a3);
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, OVERLAY, 0, &regs, 0x01E0_1A60, &mut a3);
    }
    fx.leave(0x40);
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

/// 0x1E02EB0(D, step, arg): the first free task cleared and set off at
/// D+0x250 with the colour +0x8634 (twice), the index +0x8692; `regs`
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

/// 0x1E01A80: an orb (a facing ring of +8 0..34 quads of size 10 growing
/// by 1, cell frame 0..6, radius from 0 by 15 to 100..449, row +0xA) that
/// bobs by 3 x the sine of +0x5C (+0x34), drifts 5 a update along z
/// (sign by the attacker's yaw) and down toward the target's height - 10
/// on its updates 0x24..0x46, opens out from 0x51, and on its update 0x4B
/// may leave a ring and a band (each with chance 6 / 10) and ends.
fn orb(fx: &mut Fx, t: u32, regs: &[u32; 5], a3: &mut u32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(regs[1])), (0, u64::from(regs[0]))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        let v = rand_mod(fx, t, 0x01E0_1AB0, 7, a3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        *a3 = 0x80;
        let v = rand_mod(fx, t, 0x01E0_1ADC, 0x168, a3);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_1AF8, 0x168, a3);
        fx.mem.set16(t + 0x5C, v);
        let r = rand_a3(fx, t, 0x01E0_1B14, a3);
        fx.mem.set16(t + 0xA, r.wrapping_rem(8));
        let i = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, i);
        fx.mem.set16(t + 0x50, 0);
        let v = rand_mod(fx, t, 0x01E0_1B3C, 0x15E, a3);
        fx.mem.set16(t + 0x52, v + 0x64);
        let v = rand_mod(fx, t, 0x01E0_1B5C, 0x14, a3);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = rand_mod(fx, t, 0x01E0_1B7C, 0x23, a3);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_1B98, 5, a3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x57 {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x20);
        }
        let r = fx.mem.i16(t + 0x50);
        if r < fx.mem.i16(t + 0x52) {
            fx.mem.set16(t + 0x50, r + 0xF);
        }
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0xA, 8));
    }
    if fx.mem.i16(t + 0x54) == 0x4B {
        for o in 0..4 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * o);
            fx.mem.write_u32(D + 0x250 + 4 * o, w);
        }
        let caller = [t, D, regs[2], regs[3], regs[4]];
        let r = rand_a3(fx, t, 0x01E0_1C58, a3);
        if fx.div(r >> 4, 0xA).1 < 6 {
            spawn(fx, RING, 0, &caller, 0x01E0_1C90, a3);
        }
        for (o, ra) in [(0u32, 0x01E0_1C98u32), (4, 0x01E0_1CCC), (8, 0x01E0_1D00)] {
            let v = rand_mod(fx, t, ra, 0x1E, a3);
            let p = ee::add(fx.mem.f(D + 0x250 + o), ee::from_i32(v - 0xF));
            fx.mem.setf(D + 0x250 + o, p);
        }
        let r = rand_a3(fx, t, 0x01E0_1D34, a3);
        if fx.div(r >> 4, 0xA).1 < 6 {
            spawn(fx, BAND, 0, &caller, 0x01E0_1D6C, a3);
        }
        fx.mem.set16(t + 0x58, 0);
    }
    let tg = fx.target_handle();
    fx.unit_position(tg, sp + 0x30);
    let a = fx.mem.i16(t + 0x5C);
    let s = l2::sin_deg(fx, SINES, a, 1);
    fx.mem.setf(t + 0x34, ee::mul(3.0, s));
    let n = fx.mem.i16(t + 0x54);
    if (0x24..0x47).contains(&n) {
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(YAW) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(t + 0x18, z);
        let floor = ee::sub(fx.mem.f(sp + 0x34), 10.0);
        let y = fx.mem.f(t + 0x14);
        if crate::exe::ee_lt(floor, y) {
            fx.mem.setf(t + 0x14, ee::sub(y, 1.0));
        }
    }
    if fx.mem.i16(t + 0x54) >= 0x51 {
        let a = fx.mem.i16(t + 0x5A) + 5;
        fx.mem.set16(t + 0x5A, a);
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
        let y = ee::add(fx.mem.f(t + 0x14), 1.0);
        fx.mem.setf(t + 0x14, y);
        let s = ee::add(fx.mem.f(t + 0x48), 1.0);
        fx.mem.setf(t + 0x48, s);
    }
    let n = fx.mem.i16(t + 0x54);
    if n >= 0x24 {
        let g = if n != 0 { 0x80 } else { 0xFF };
        l2::rgb(fx, t + 0x60, g, g, g);
        *a3 = g as u32;
    }
    let a = fx.mem.i16(t + 0x5C) + 6;
    fx.mem.set16(t + 0x5C, a);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x40);
}

/// 0x1E01F10: a burst ring (+8 0..34 quads of size 10, cell frame 0..6,
/// radius 10..59 growing by 16..45, row +0xA), full alpha 0x80 for its
/// life +6 (3..7), then fading by 8.
fn ring(fx: &mut Fx, t: u32, a3: &mut u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        let v = rand_mod(fx, t, 0x01E0_1F38, 7, a3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        *a3 = 0x80;
        let v = rand_mod(fx, t, 0x01E0_1F64, 0x168, a3);
        fx.mem.set16(t + 0x5A, v);
        let r = rand_a3(fx, t, 0x01E0_1F80, a3);
        fx.mem.set16(t + 0xA, r.wrapping_rem(8));
        let i = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, i);
        let v = rand_mod(fx, t, 0x01E0_1FA4, 0x32, a3);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rand_mod(fx, t, 0x01E0_1FC4, 0x1E, a3);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rand_mod(fx, t, 0x01E0_1FE4, 0x23, a3);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_2000, 5, a3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0xA, 8));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E022A0: a band (inner radius 150 growing by 1, light grey inside,
/// dark blue outside) facing the camera (0x205580 into +0x20) and turning
/// by 0x4000 a update, fading in by 0x40 for 4 updates then out by 0xC.
fn band(fx: &mut Fx, t: u32, regs: &[u32; 5], a3: &mut u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(regs[0]))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4316_0000);
        l2::rgb(fx, t + 0x60, 0xCF, 0xCF, 0xCF);
        l2::rgb(fx, t + 0x64, 8, 8, 0x80);
        let v = rand_mod(fx, t, 0x01E0_22F4, 0x168, a3);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 4;
    fade(fx, t, 0x63, rising, (0xC0, 0x40), (0xE, 0xC));
    let s = ee::add(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, s);
    // 0x205580's frame: ra, the step's s1 (the caller's: the flash count),
    // s0 = the task.
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, 0x01E0_2380), (0x10, u64::from(regs[1])), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E023B0: a window sprite (0x6FF8 .. 0x8FF8) sampling the frame
/// (0x207EF0, filter 1) at ST 0..1, colour (0x70, 0x60, 0x80, alpha
/// +0x63), ALPHA 0x44, key 0; in by 0xA below 0x50 until update 0x12C,
/// then out by 9.
fn overlay(fx: &mut Fx, t: u32, a3: &mut u32) {
    let sp = fx.enter(0x80);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for (k, c) in [0x70u8, 0x60, 0x80].into_iter().enumerate() {
        fx.mem.write_u8(sp + 0x38 + k as u32, c);
    }
    let a = fx.mem.read_u8(t + 0x63);
    fx.mem.write_u8(sp + 0x3B, a);
    fx.mem.write_u32(sp + 0x3C, 0x3F80_0000);
    for (at, v) in [(sp + 0x48, 0x6FF8u16), (sp + 0x58, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for (o, v) in [(0x40u32, 0u32), (0x44, 0), (0x50, 0x3F80_0000), (0x54, 0x3F80_0000)] {
        fx.mem.write_u32(sp + o, v);
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    // 0x207EF0's 0x207FC0 leaves a3 = 0 here (the submit 0x206F30 is
    // answered natively, registers left) [run, traced].
    *a3 = 0;
    let rising = fx.mem.i16(t + 0x54) < 0x12C;
    fade(fx, t, 0x63, rising, (0x50, 0xA), (0xB, 9));
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x80);
}

/// 0x1E02550(D, t): the flash's frame sprite (the target over the window,
/// the colour +0x60, UVs inset by the size +8 / +0xA over 512) into the
/// descriptor at +0x8648, no ordering table; growing by 30 from update
/// 0x65, alpha rising by 9 below 0x80 for 0xE updates, from 0x65 falling
/// by 7. `regs` the unit flash's s0 / s1 (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_314C), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f21 / f20: the runner's (0) [H].
    fx.mem.write_u32(sp + 4, 0);
    fx.mem.write_u32(sp, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x10, 0x10, 0x10);
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
    let v = ee::div(w, 512.0);
    fx.mem.setf(d + 0x18, v);
    let v = ee::div(h, 512.0);
    fx.mem.setf(d + 0x1C, v);
    let v = ee::div(ee::sub(512.0, w), 512.0);
    fx.mem.setf(d + 0x28, v);
    let v = ee::div(ee::sub(512.0, h), 512.0);
    fx.mem.setf(d + 0x2C, v);
    fx.submit(d, 0, 0);
    if fx.mem.i16(t + 0x54) >= 0x65 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o) + 0x1E;
            fx.mem.set16(t + o, s);
        }
    }
    let n = fx.mem.i16(t + 0x54);
    if n < 0xE {
        fade(fx, t, 0x63, true, (0x80, 9), (0, 0));
    } else if n >= 0x65 {
        fade(fx, t, 0x63, false, (0, 0), (9, 7));
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x40);
}

/// 0x1E03040(D): in a packet layer (0x2078A0) the attacker drawn into the
/// target (0x1F8270 filter 0, its TEX0 / TEX1 into the sprite descriptor
/// at sp + 0x58; ALPHA 0x48), TEXFLUSH before and after; every flash task
/// stepped (0x1E02550); the layer linked into the ordering table at key 0
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
