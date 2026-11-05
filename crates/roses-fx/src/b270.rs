//! BMODULE b270 (card 331), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04600, data area 0x1ED0000) [F]: b574's (card 258) sibling on the same 250-task pool and register
//! use (D in s1, the drawn tasks counted in s0): spawn 0x1E05070 = b764's
//! on the colour D+0xD994, the angle D+0xDA7A and the index D+0xDA8C; the
//! petal sprite 0x1E05220 = b764's on the cells D+0xD9E4, the sines
//! D+0xDC98 and the OT D+0xD99C; the trail 0x1E03900 = b764's 0x1E035C0;
//! the tube 0x1E05540 b733's but the addresses. Drawn with plain quad rings
//! (0x1E01260, `billboard::ring_quads` on the bytes D+0xDA96 / D+0xDB96),
//! arcs (0x1E009B0), camera-facing quads (0x1E01960, `billboard::quad`),
//! sparks (0x1E01C20, `quads4::sparks`) and a window sprite (0x1E04400,
//! b178's 0x1E040C0 in another colour). Globals past D+0xD000: three points
//! D+0xD930 / D+0xD940 / D+0xD950 at the target from update 0x12C (y 30,
//! the second rising 0.7 a update on 0x168..0x17C, the third on
//! 0x168..0x190), textures D+0xD970.., timer D+0xD990 = 485, OT D+0xD99C,
//! nodes D+0xD9C4.., cells D+0xD9E4, petal count D+0xDA76, trail counter
//! D+0xDA7E, three ring angles D+0xDA82.., facing D+0xDA90, counter
//! D+0xDA94, bytes D+0xDA96 (100..0xB3 cleared) / D+0xDB96, sines D+0xDC98.
//! The emitter 0x1E02020: 1 a stage light 0x1E04340; 0xBE..0x168 a ring
//! 0x1E033E0 at node 0x25 and a spark 0x1E041A0 6 above node 0x57 a update;
//! 0x14F..0x1CC the 108 petals 0x1E02940 three a update (36 at each
//! point); 0x168..0x1CC five rings (0x1E02EE0 at the first point and
//! circling the three at 40 / 30 / 20, 0x1E031B0) a update; 0x168 two arc
//! fans 0x1E03F10 and the window sprite; 0x168..0x1D1 a glow 0x1E03780 and
//! an arc fan 0x1E03580 at the second point a update; 0x17C..0x1CC every
//! fourth update an arc 0x1E03F10 (kind 2); 0x169..0x1CC a trail a update.
//! The ring step 0x1E02D50 is never spawned [F]: not ported, nor its draw
//! case. Returns 1 / 3 / 6 / 8 on 0x15E / 0x1DB / 0x169..0x1D5 / timer out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::b764::{age, cell, copy4, enter_step_with, fade_in, fade_out, rand_mod, trail_of, Glob, TrailCfg};
use crate::blib::{billboard, quads4, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xFA;
const PETAL: u32 = 0x01E0_2940;
const RING_N: u32 = 0x01E0_2D50;
const RING_E: u32 = 0x01E0_2EE0;
const RING_F: u32 = 0x01E0_31B0;
const RING_G: u32 = 0x01E0_33E0;
const ARC_H: u32 = 0x01E0_3580;
const GLOW: u32 = 0x01E0_3780;
const TRAIL: u32 = 0x01E0_3900;
const ARC_I: u32 = 0x01E0_3F10;
const SPARK: u32 = 0x01E0_41A0;
const LIGHT: u32 = 0x01E0_4340;
const WINDOW: u32 = 0x01E0_4400;
/// The stub the entry skips (`jr ra`), never spawned.
const STUB: u32 = 0x01E0_45F0;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_49A8;
const POINTS: u32 = D + 0xD930;
const TEX: u32 = D + 0xD970;
const TIMER: u32 = D + 0xD990;
const COLOUR: u32 = D + 0xD994;
const OT_AT: u32 = D + 0xD99C;
const NODES: u32 = D + 0xD9C4;
const CELLS: u32 = D + 0xD9E4;
const PETALS: u32 = D + 0xDA76;
const ANGLE: u32 = D + 0xDA7A;
const TRAIL_K: u32 = D + 0xDA7E;
const RING_A: u32 = D + 0xDA82;
const INDEX: u32 = D + 0xDA8C;
const FACING: u32 = D + 0xDA90;
const COUNTER: u32 = D + 0xDA94;
const BYTES: u32 = D + 0xDA96;
const BYTES_B: u32 = D + 0xDB96;
const SINES: u32 = D + 0xDC98;

const G: Glob = Glob { colour: COLOUR, angle: ANGLE, index: INDEX, cells: CELLS, sines: SINES, ot_at: OT_AT };

/// The steps' saved s0 / s1: the entry's (s0 the draw count, 0 while the
/// steps run; s1 = D).
const VALS: [u64; 2] = [0, D as u64];

fn point(k: u32) -> u32 {
    POINTS + 0x10 * k
}

/// The entry 0x1E04600.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xE0);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xE0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xDE10);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xDA92, param);
    fx.mem.set32(TIMER, 0x1E5);
    for (k, n) in [0x25, 0x57, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xD9D4, r);
    let tex = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, tex);
    for k in 0..5u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.fix_residue(v, 0x01E0_47F0);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_4820);
        let (_, h) = fx.div(r >> 4, 0x1E);
        fx.mem.write_u8(BYTES + k, (h - 0xF) as u8);
    }
    for k in 100..0xB4u32 {
        fx.mem.write_u8(BYTES + k, 0);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_48B4);
        let (_, h) = fx.div(r >> 4, 0x1E);
        fx.mem.write_u8(BYTES_B + k, (h - 0xF) as u8);
    }
    for k in 0..0x24u32 {
        let r = fx.rand_from(D, 0x01E0_490C);
        fx.mem.set16(CELLS + 2 * k, r.wrapping_rem(8));
    }
    0
}

fn ring(fx: &mut Fx, t: u32, tex: u32, r: u32, n: i32, table: u32) {
    let w = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, w);
    cell(fx, t, r, 0x14);
    fx.mem.set16(D + 0xB8, n);
    fx.mem.set16(D + 0xB0, 0);
    billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, table);
    fx.mem.set16(D + 0xB8, 0);
    fx.mem.set16(D + 0xB0, 0);
}

fn arcs(fx: &mut Fx, t: u32) {
    fx.mem.set16(D + 0xB8, 0x18);
    billboard::arc_strips(fx, D, t, 0x20, t + 0x10);
    fx.mem.set16(D + 0xB8, 0);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 || f == STUB {
            continue;
        }
        run_task(fx, f, slot + 0x2A0);
    }
    let mut drawn = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        drawn += 1;
        let f = fx.mem.read_u32(t + 0x44);
        if f == PETAL {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x60, 0x14);
            fx.mem.set16(D + 0xB8, 1);
            fx.mem.set16(D + 0xBE, 0);
            let k = fx.mem.i16(t + 0x50);
            let at = if k < 0x24 {
                Some(0)
            } else if k < 0x48 {
                Some(1)
            } else if k < 0x6C {
                Some(2)
            } else {
                None
            };
            if let Some(p) = at {
                crate::b764::petal(fx, &G, t, 0x20, point(p));
            }
        }
        debug_assert_ne!(f, RING_N);
        if f == RING_E {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x80, 0x14);
            fx.mem.set16(D + 0xB8, 5);
            fx.mem.set16(D + 0xB0, 0);
            if fx.mem.i16(t + 0x52) == 0 {
                billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
            }
            if fx.mem.i16(t + 0x52) == 1 {
                billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            }
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if f == RING_F {
            ring(fx, t, TEX + 8, sp + 0x90, 4, BYTES);
        }
        if f == RING_G {
            ring(fx, t, TEX + 4, sp + 0xA0, 2, BYTES_B);
        }
        if f == ARC_H {
            arcs(fx, t);
        }
        if f == GLOW {
            let w = fx.mem.read_u32(TEX + 0x10);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0xB0, 0x14);
            let r = fx.rand_from(drawn, 0x01E0_4DE0);
            fx.mem.set16(D + 0xBE, (r >> 4).wrapping_rem(8));
            copy4(fx, sp + 0x50, point(1));
            billboard::quad(fx, uv::uv_rect, D, t, 0x21, sp + 0x50);
        }
        if f == TRAIL {
            let w = fx.mem.read_u32(TEX + 0xC);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x3F)] {
                fx.mem.set16(D + o, v);
            }
            for c in [0u32, 4, 8] {
                let w = fx.mem.read_u32(point(1) + c);
                fx.mem.write_u32(D + 0x20 + c, w);
            }
            fx.mem.set16(D + 0xBE, 1);
            let k = fx.mem.i16(t + 0x50);
            tubetrail::tube_with(fx, &tubetrail::SINGLE_BLEND, D, t, trail_of(k), 0x21);
        }
        if f == ARC_I {
            arcs(fx, t);
        }
        if f == SPARK {
            let frame = fx.mem.i16(t + 0x56);
            l2::rect(fx, sp + 0xC0, 0x9B, frame);
            copy4(fx, D + 0x94, sp + 0xC0);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 0xC);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x15E => 1,
        0x1DB => 3,
        0x169..=0x1D5 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

const TRAIL_CFG: TrailCfg = TrailCfg {
    div: 10.0,
    heading_b: true,
    width: (0x96, 0x1E),
    step: (0x55, 3),
    dim: 0x80,
    jdiv: 10.0,
    init: [0x01E0_3978, 0x01E0_39A4, 0x01E0_39D0, 0x01E0_3AFC, 0x01E0_3B3C, 0x01E0_3B7C, 0x01E0_3B98, 0x01E0_3BB8],
    jitter: [0x01E0_3CB4, 0x01E0_3CE0, 0x01E0_3D0C],
};

fn run_task(fx: &mut Fx, f: u32, t: u32) {
    match f {
        PETAL => petal_step(fx, t),
        RING_E => ring_e(fx, t),
        RING_F => ring_f(fx, t),
        RING_G => ring_g(fx, t),
        ARC_H => arc_h(fx, t),
        GLOW => glow(fx, t),
        TRAIL => crate::b764::trail(fx, t, &TRAIL_CFG, SINES, VALS, TASK_RA),
        ARC_I => arc_i(fx, t),
        SPARK => spark(fx, t),
        LIGHT => light(fx, t),
        WINDOW => window(fx, t),
        _ => panic!("b270: unknown task step {f:#x}"),
    }
}

/// 0x1E02020(D, 0): the update counter, the points and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    let d = u64::from(D);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x20, 0x01E0_4968), (0x10, d), (0, 0)]);
    // The spawns' saved s0..s4: the emitter's s0 (the entry's 0, the trail
    // loop's count), s1 = D, the entry's s2 (param), s3 / s4 (the
    // runner's, 0).
    let mut s0 = 0u64;
    let spawn = |fx: &mut Fx, f: u32, s0: u64, ra: u32| crate::b764::spawn_with(fx, &G, f, [s0, d, p, 0, 0], ra);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 0x12C {
        for (k, ra) in [(0u32, 0x01E0_2080u32), (1, 0x01E0_20A4), (2, 0x01E0_20C8)] {
            let h = fx.target_handle();
            unit_position_saving(fx, h, point(k), 0, ra);
            fx.mem.write_u32(point(k) + 4, 0x41F0_0000);
        }
    }
    for (k, to) in [(1u32, 0x17D), (2, 0x191)] {
        let n = fx.mem.i16(COUNTER);
        if (0x168..to).contains(&n) {
            let y = ee::add(fx.mem.f(point(k) + 4), f32::from_bits(0x3F33_3333));
            fx.mem.setf(point(k) + 4, y);
        }
    }
    let n = fx.mem.i16(COUNTER);
    if (0x14F..0x1CD).contains(&n) && fx.mem.i16(PETALS) < 0x24 {
        for (j, (add, ra)) in [(0, 0x01E0_21F8u32), (0xB4, 0x01E0_2250), (0, 0x01E0_22A4)].into_iter().enumerate() {
            let k = fx.mem.i16(PETALS);
            fx.mem.set16(ANGLE, k * 10 + add);
            let k = fx.mem.i16(PETALS);
            fx.mem.set16(INDEX, k + 0x24 * j as i32);
            spawn(fx, PETAL, s0, ra);
        }
        let k = fx.mem.i16(PETALS) + 1;
        fx.mem.set16(PETALS, k);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x168..0x1CD).contains(&n) {
        copy4(fx, D + 0x250, point(0));
        fx.mem.set16(INDEX, 0);
        spawn(fx, RING_E, s0, 0x01E0_232C);
        fx.mem.set16(INDEX, 1);
        // (point, ring angle, radius, ra)
        for (j, r, ra) in [(0u32, 40.0f32, 0x01E0_23E0u32), (1, 30.0, 0x01E0_2484), (2, 20.0, 0x01E0_2528)] {
            copy4(fx, D + 0x250, point(j));
            let a = fx.mem.i16(RING_A + 2 * j);
            let s = l2::sin_deg(fx, SINES, a, 0);
            let z = ee::add(fx.mem.f(D + 0x258), ee::mul(r, s));
            fx.mem.setf(D + 0x258, z);
            let a = fx.mem.i16(RING_A + 2 * j);
            let c = l2::sin_deg(fx, SINES, a, 1);
            let x = ee::add(fx.mem.f(D + 0x250), ee::mul(r, c));
            fx.mem.setf(D + 0x250, x);
            spawn(fx, RING_E, s0, ra);
        }
        copy4(fx, D + 0x250, point(0));
        spawn(fx, RING_F, s0, 0x01E0_2564);
        for (j, d) in [(0u32, 0xA), (1, -0xA), (2, 0xA)] {
            let a = fx.mem.i16(RING_A + 2 * j) + d;
            fx.mem.set16(RING_A + 2 * j, a);
        }
    }
    if fx.mem.i16(COUNTER) == 0x168 {
        copy4(fx, D + 0x250, point(0));
        fx.mem.set16(INDEX, 0);
        spawn(fx, ARC_I, s0, 0x01E0_2618);
        fx.mem.set16(INDEX, 1);
        spawn(fx, ARC_I, s0, 0x01E0_2638);
        spawn(fx, WINDOW, s0, 0x01E0_264C);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x168..0x1D2).contains(&n) {
        spawn(fx, GLOW, s0, 0x01E0_2688);
        copy4(fx, D + 0x250, point(1));
        fx.mem.set16(INDEX, 1);
        spawn(fx, ARC_H, s0, 0x01E0_26D4);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x17C..0x1CD).contains(&n) {
        copy4(fx, D + 0x250, point(0));
        fx.mem.set16(INDEX, 2);
        if fx.mem.i16(COUNTER).wrapping_rem(4) == 1 {
            spawn(fx, ARC_I, s0, 0x01E0_2770);
        }
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, LIGHT, s0, 0x01E0_27A0);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x169..0x1CD).contains(&n) {
        let i = fx.mem.i16(TRAIL_K);
        fx.mem.set16(INDEX, i);
        spawn(fx, TRAIL, 0, 0x01E0_27F4);
        let i = fx.mem.i16(TRAIL_K) + 1;
        fx.mem.set16(TRAIL_K, i);
        if fx.mem.i16(TRAIL_K) >= 0x50 {
            fx.mem.set16(TRAIL_K, 0);
        }
        s0 = 1;
    }
    let n = fx.mem.i16(COUNTER);
    if (0xBE..0x169).contains(&n) {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        node_position_saving(fx, h, s, D + 0x250, s0, Some(d), 0x01E0_289C);
        spawn(fx, RING_G, s0, 0x01E0_28B0);
    }
    let n = fx.mem.i16(COUNTER);
    if (0xBE..0x169).contains(&n) {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES + 2);
        node_position_saving(fx, h, s, D + 0x250, s0, Some(d), 0x01E0_28F4);
        let y = ee::add(fx.mem.f(D + 0x254), 6.0);
        fx.mem.setf(D + 0x254, y);
        fx.mem.set16(INDEX, 0);
        spawn(fx, SPARK, s0, 0x01E0_2928);
    }
    fx.leave(0x30);
}

/// 0x1E02940: petal +0x50 (k) of 108 (36 at each point): radii from 0,
/// both alphas in by 8 below 0x80 for 30 updates, out by 0xB from update
/// 0x1CD; its cell rand % 3 + 4; white / grey by age; the inner radius up
/// by 2.5 to 40, then the outer to 30 (k 36..71: 30 / 20, 72..: 20 / 10);
/// from update 0x191 the y angle turning by +-0x300.
fn petal_step(fx: &mut Fx, t: u32) {
    enter_step_with(fx, 0x30, 2, VALS, TASK_RA);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.write_u32(t + 0x4C, 0);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.set16(t + 0x58, 2);
        let k = fx.mem.i16(t + 0x50);
        if k < 0x24 {
            fx.mem.set16(t + 0x52, 0);
        } else if k < 0x48 {
            fx.mem.set16(t + 0x52, 2);
        } else if k < 0x6C {
            fx.mem.set16(t + 0x52, 4);
        }
        let v = rand_mod(fx, t, 0x01E0_29C0, 3);
        fx.mem.set16(t + 8, v + 4);
    }
    if fx.mem.i16(t + 0x54) < 0x1E {
        fade_in(fx, t, 0x63, (0x80, 8));
        fade_in(fx, t, 0x67, (0x80, 8));
    } else if fx.mem.i16(COUNTER) >= 0x1CD {
        fade_out(fx, t, 0x63, (0xD, 0xB));
        let v = fx.mem.read_u8(t + 0x67);
        if v >= 0xD {
            fx.mem.write_u8(t + 0x67, v - 0xB);
        }
    }
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0xFF } else { 0x80 };
    l2::rgb(fx, t + 0x60, g, g, g);
    l2::rgb(fx, t + 0x64, g, g, g);
    if fx.mem.i16(COUNTER) != 0 {
        let s = fx.mem.i16(t + 0x52);
        // (stage, inner cap, outer cap)
        let caps = [(0, 40.0f32, 30.0f32), (2, 30.0, 20.0), (4, 20.0, 10.0)];
        for (even, inner, outer) in caps {
            if s == even {
                let r0 = fx.mem.f(t + 0x48);
                if r0 < inner {
                    fx.mem.setf(t + 0x48, ee::add(r0, 2.5));
                } else {
                    fx.mem.set16(t + 0x52, s + 1);
                }
            } else if s == even + 1 {
                let r1 = fx.mem.f(t + 0x4C);
                if r1 < outer {
                    fx.mem.setf(t + 0x4C, ee::add(r1, 2.5));
                }
            }
        }
    }
    if fx.mem.i16(COUNTER) >= 0x191 {
        let k = fx.mem.i16(t + 0x50);
        let d = if k < 0x24 {
            Some(0x300)
        } else if k < 0x48 {
            Some(-0x300)
        } else if k < 0x6C {
            Some(0x300)
        } else {
            None
        };
        if let Some(d) = d {
            let v = fx.mem.i32(t + 0x24).wrapping_add(d);
            fx.mem.set32(t + 0x24, v);
        }
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02EE0: a quad ring (index +0x52 0: white at the first point, radius
/// rand % 30 + 10 growing by rand % 60 + 40, size rand % 10 + 5 growing by
/// 1 from its second update, white / light grey by age; 1: orange circling
/// a point, radius rand % 50 + 15, still), cell 4, three random turns,
/// rows rand % 20, 0xC0 for rand % 3 + 4 updates, then out by 0xF.
fn ring_e(fx: &mut Fx, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, TASK_RA);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rand_mod(fx, t, 0x01E0_2F04, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 5));
        fx.mem.set16(t + 0x56, 4);
        if fx.mem.i16(t + 0x52) == 0 {
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        }
        if fx.mem.i16(t + 0x52) == 1 {
            l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x10);
        }
        for (o, ra) in [(0x20u32, 0x01E0_2F80u32), (0x24, 0x01E0_2F9C), (0x28, 0x01E0_2FB8)] {
            let v = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        if fx.mem.i16(t + 0x52) == 0 {
            let v = rand_mod(fx, t, 0x01E0_2FE4, 0x1E);
            fx.mem.set16(t + 0x50, v + 0xA);
        }
        if fx.mem.i16(t + 0x52) == 1 {
            let v = rand_mod(fx, t, 0x01E0_3018, 0x32);
            fx.mem.set16(t + 0x50, v + 0xF);
        }
        let v = rand_mod(fx, t, 0x01E0_303C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_3058, 0x3C);
        fx.mem.set16(t + 0xE, v + 0x28);
        let v = rand_mod(fx, t, 0x01E0_3078, 0x14);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_3094, 3);
        fx.mem.set16(t + 6, v + 4);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t, 0xC0);
    if fx.mem.i16(t + 0x52) == 0 {
        let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0xFF } else { 0xC0 };
        l2::rgb(fx, t + 0x60, g, g, g);
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
        if fx.mem.i16(t + 0x54) != 0 {
            let s = ee::add(fx.mem.f(t + 0x48), 1.0);
            fx.mem.setf(t + 0x48, s);
        }
    }
    age(fx, t);
    fx.leave(0x20);
}

/// `full` for +6 updates, then out by 0xF.
fn hold(fx: &mut Fx, t: u32, full: u8) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, full);
    } else {
        fade_out(fx, t, 0x63, (0x11, 0xF));
    }
}

/// 0x1E031B0: a white quad ring at the first point (size rand % 30 + 5,
/// cell rand % 4 stepping to 8 where it ends, three random turns, radius
/// rand % 400 + 20, rows rand % 20), 0xC0 for rand % 3 + 4 updates, then
/// out by 0xF; white / light grey by age.
fn ring_f(fx: &mut Fx, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, TASK_RA);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rand_mod(fx, t, 0x01E0_31D4, 0x1E);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 5));
        let v = fx.rand_from(t, 0x01E0_3200);
        fx.mem.set16(t + 0x56, v.wrapping_rem(4));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        for (o, ra) in [(0x20u32, 0x01E0_3234u32), (0x24, 0x01E0_3250), (0x28, 0x01E0_326C)] {
            let v = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let v = rand_mod(fx, t, 0x01E0_3288, 0x190);
        fx.mem.set16(t + 0x50, v + 0x14);
        let v = rand_mod(fx, t, 0x01E0_32A8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_32C4, 0x14);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_32E0, 3);
        fx.mem.set16(t + 6, v + 4);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t, 0xC0);
    let c = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, c);
    if fx.mem.i16(t + 0x56) >= 8 {
        fx.mem.set16(t + 0x58, 0);
    }
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E033E0: a white quad ring at node 0x25 (size 1, cell 4, three random
/// turns, radius rand % 80 + 5, rand % 60 + 40 set but not grown, rows rand
/// % 20), 0x80 for rand % 2 + 1 updates, then out by 0xF.
fn ring_g(fx: &mut Fx, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, TASK_RA);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        for (o, ra) in [(0x20u32, 0x01E0_3428u32), (0x24, 0x01E0_3444), (0x28, 0x01E0_3460)] {
            let v = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let v = rand_mod(fx, t, 0x01E0_347C, 0x50);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rand_mod(fx, t, 0x01E0_349C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_34B8, 0x3C);
        fx.mem.set16(t + 0xE, v + 0x28);
        let v = rand_mod(fx, t, 0x01E0_34D8, 0x14);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, 0x01E0_34F4);
        fx.mem.set16(t + 6, v.wrapping_rem(2) + 1);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t, 0x80);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03580: a grey / ochre arc fan at the second point (radii 20 / 80
/// growing by 6 / 10, index 1: 20 / 100 growing by 6 / 80; 15-degree arcs
/// every 2, three random turns), its outer alpha in by 0x2A below 0x80 for
/// 3 updates, then out by 0xD.
fn arc_h(fx: &mut Fx, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, TASK_RA);
    if fx.mem.i16(t + 0x58) == 1 {
        for (k, outer) in [(0, 0x42A0_0000u32), (1, 0x42C8_0000)] {
            if fx.mem.i16(t + 0x50) == k {
                fx.mem.write_u32(t + 0x48, 0x41A0_0000);
                fx.mem.write_u32(t + 0x4C, outer);
                fx.mem.write_u32(t + 0x34, 0x4000_0000);
                fx.mem.set16(t + 6, 3);
            }
        }
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x40);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 2);
        for (o, ra) in [(0x20u32, 0x01E0_3634u32), (0x24, 0x01E0_3650), (0x28, 0x01E0_366C)] {
            let v = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x67, (0x80, 0x2A));
    } else {
        fade_out(fx, t, 0x67, (0xF, 0xD));
    }
    for (k, grow) in [(0, 10.0f32), (1, 80.0)] {
        if fx.mem.i16(t + 0x50) == k {
            let r = ee::add(fx.mem.f(t + 0x48), 6.0);
            fx.mem.setf(t + 0x48, r);
            let r = ee::add(fx.mem.f(t + 0x4C), grow);
            fx.mem.setf(t + 0x4C, r);
        }
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03780: a pale yellow glow at the second point (size 200 growing by
/// 10, random angles), both alphas in by 0x1D below 0x58 for 3 updates,
/// then +0x63 out by 0xE twice (the task ends below 0x10); cell 5 then 4;
/// facing the camera (0x205580 into +0x20).
fn glow(fx: &mut Fx, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, TASK_RA);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xC8);
        fx.mem.set16(t + 0xA, 0xC8);
        fx.mem.set16(t + 0x56, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xC0);
        fx.mem.write_u32(t + 0x34, 0);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xC0);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rand_mod(fx, t, 0x01E0_37E8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_3804, 0xB4);
        fx.mem.set16(t + 0x5C, v - 0x5A);
    }
    if fx.mem.i16(t + 0x54) < 3 {
        fade_in(fx, t, 0x63, (0x58, 0x1D));
        fade_in(fx, t, 0x67, (0x58, 0x1D));
    } else {
        fade_out(fx, t, 0x63, (0x10, 0xE));
        let v = fx.mem.read_u8(t + 0x63);
        if v >= 0x10 {
            fx.mem.write_u8(t + 0x63, v - 0xE);
        }
    }
    let c = if fx.mem.i16(t + 0x56) == 0 { 5 } else { 4 };
    fx.mem.set16(t + 0x56, c);
    for o in [8u32, 0xA] {
        let v = fx.mem.i16(t + o) + 0xA;
        fx.mem.set16(t + o, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_38DC);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03F10: an arc fan at the first point (index 0: radius 300, white
/// over pink, in for 12 updates, its z angle turning 0x200 a update, the
/// inner radius up by 6 to 150, the outer by 40; 1: radius 150, facing the
/// camera turned 0x4000 about x, the outer up by 40; 2: radius 20, grey /
/// ochre, in for 4, a band 70 wide out by 100 a update), 15-degree arcs,
/// its outer alpha in by 0x60 below 0xC0, then out by 0xB.
fn arc_i(fx: &mut Fx, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, TASK_RA);
    if fx.mem.i16(t + 0x58) == 1 {
        for (k, outer, n) in [(0, 0x4396_0000u32, 0xC), (1, 0x4316_0000, 0xC), (2, 0x41A0_0000, 4)] {
            if fx.mem.i16(t + 0x50) == k {
                fx.mem.write_u32(t + 0x48, 0);
                fx.mem.write_u32(t + 0x4C, outer);
                fx.mem.write_u32(t + 0x34, 0x4000_0000);
                fx.mem.set16(t + 6, n);
            }
        }
        let k = fx.mem.i16(t + 0x50);
        if k == 0 || k == 1 {
            l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
            l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x40);
        }
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        let v = rand_mod(fx, t, 0x01E0_4030, 0xFFFF);
        fx.mem.set32(t + 0x24, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x67, (0xC0, 0x60));
    } else {
        fade_out(fx, t, 0x67, (0xD, 0xB));
    }
    match fx.mem.i16(t + 0x50) {
        0 => {
            let a = fx.mem.i32(t + 0x24).wrapping_add(0x200);
            fx.mem.set32(t + 0x24, a);
            let r0 = fx.mem.f(t + 0x48);
            if r0 < 150.0 {
                fx.mem.setf(t + 0x48, ee::add(r0, 6.0));
            }
            let r1 = ee::add(fx.mem.f(t + 0x4C), 40.0);
            fx.mem.setf(t + 0x4C, r1);
        }
        1 => {
            camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_411C);
            fx.mem.set32(t + 0x20, 0x4000);
            let r1 = ee::add(fx.mem.f(t + 0x4C), 40.0);
            fx.mem.setf(t + 0x4C, r1);
        }
        2 => {
            let r1 = ee::add(fx.mem.f(t + 0x4C), 100.0);
            fx.mem.setf(t + 0x4C, r1);
            fx.mem.setf(t + 0x48, ee::sub(r1, 70.0));
        }
        _ => {}
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E041A0: a yellow spark burst 6 above node 0x57 (radius 120 growing by
/// (rand % 30 + 20) tenths, its inner 0.05 of it, cell 2, three random
/// turns), 0x80 for 6 updates, then out by 5.
fn spark(fx: &mut Fx, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, TASK_RA);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42F0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(120.0, 58.0));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x80);
        l2::rgb(fx, t + 0x64, 0, 0, 0);
        fx.mem.set16(t + 0x56, 2);
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 1);
        let v = rand_mod(fx, t, 0x01E0_421C, 0x1E);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0x14));
        let v = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, ra) in [(0x20u32, 0x01E0_4264u32), (0x24, 0x01E0_4280), (0x28, 0x01E0_429C)] {
            let v = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 6 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, (7, 5));
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    let r = ee::mul(f32::from_bits(0x3D4C_CCCD), fx.mem.f(t + 0x48));
    fx.mem.setf(t + 0x4C, r);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E04340: the stage light (0x1F7F10) +0x50 from 0x80 down by 10 to
/// 0x40, back up by 10 to 0x80 once the timer is below 0x15.
fn light(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x10);
    fx.saved(sp, &[(0, TASK_RA)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let v = fx.mem.i16(t + 0x50);
    if fx.mem.i32(TIMER) >= 0x15 {
        fx.mem.set16(t + 0x50, if v < 0x41 { 0x40 } else { v - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if v < 0x80 { v + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    let v = fx.mem.read_u8(t + 0x50);
    fx.set_stage_light(i32::from(v));
    fx.leave(0x10);
}

/// 0x1E04400: b178's window sprite (0x6FF8 .. 0x8FF8) sampling the frame
/// (0x207EF0, filter 1) at ST 1 / 512 .. 511 / 512, colour (0x90, 0x90,
/// 0x78, alpha +0x63), ALPHA 0x44, key 0; in by 8 below 0x50 until update
/// 0x1CC, then out by 7.
fn window(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for (k, c) in [0x90u8, 0x90, 0x78].into_iter().enumerate() {
        fx.mem.write_u8(sp + 0x48 + k as u32, c);
    }
    let a = fx.mem.read_u8(t + 0x63);
    fx.mem.write_u8(sp + 0x4B, a);
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    let w = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x50, ee::div(w, 512.0));
    let w = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(w, 512.0));
    let w = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(512.0, w), 512.0));
    let w = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(512.0, w), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(COUNTER) < 0x1CC {
        fade_in(fx, t, 0x63, (0x50, 8));
    } else {
        fade_out(fx, t, 0x63, (9, 7));
    }
    age(fx, t);
    fx.leave(0x90);
}
