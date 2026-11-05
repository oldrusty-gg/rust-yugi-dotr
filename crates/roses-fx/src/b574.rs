//! BMODULE b574 (card 258), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E033F0, data area 0x1ED0000) [F]: b764's (card 458) sibling on the same 250-task pool (spawn
//! 0x1E03D70 = b764's on the colour D+0xD994, the angle D+0xDA7E and the
//! index D+0xDA90; petal sprite 0x1E03F20 = b764's on the cells D+0xD9E8,
//! the sines D+0xDC9C and the OT D+0xD99C; the trail 0x1E02A70 = b764's
//! 0x1E035C0 with a wider, faster head and light grey on odd ages; the tube
//! 0x1E04240 is b733's 0x1E01430 but the addresses), drawn with plain quad
//! rings (0x1E00D10, `billboard::ring_quads` on the bytes D+0xDA9A /
//! D+0xDB9A), arcs (0x1E009B0) and the tint (0x1E01410). The entry keeps D
//! in s1 and counts the drawn tasks in s0. Globals past D+0xD000: the point
//! D+0xD930 at the attacker's node 0x15 to update 0x121, then toward the
//! target's node 1 over 75 updates to 0x16B (velocities D+0xD964 / D+0xD968),
//! textures D+0xD970.., timer D+0xD990 = 420, OT D+0xD99C, tint w D+0xD9B4,
//! nodes D+0xD9C8.., cells D+0xD9E8, petal count D+0xDA7A, trail counter
//! D+0xDA82, facing D+0xDA94, counter D+0xDA98, bytes D+0xDA9A / D+0xDB9A,
//! sines D+0xDC9C. The emitter 0x1E015E0: 0x7D..0x12C the 72 petals
//! 0x1E01B20 two a update; 0xDC two arc fans 0x1E02720 (radii 300 / 200);
//! 0x122 a tint 0x1E02070 and ten rings 0x1E02170; 0x122..0x190 at the point
//! an arc 0x1E03080 and a ring 0x1E03240 a update; 0x16C..0x190 two rings
//! 0x1E02550 and an arc 0x1E02910 a update; 0x16D..0x190 two trails a
//! update. The ring step 0x1E02320 is never spawned [F]: not ported, nor
//! its draw case. Returns 1 / 4 / 2 / 6 / 8 on 0x161 / 0x19F / 0x19A /
//! 0x16C..0x194 / timer out.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::b590::project_residue;
use crate::b764::{age, cell, copy4, enter_step_with, fade_in, fade_out, rand_mod, trail_of, Glob, TrailCfg};
use crate::blib::{billboard, quads4, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xFA;
const PETAL: u32 = 0x01E0_1B20;
const TINT: u32 = 0x01E0_2070;
const RING1: u32 = 0x01E0_2170;
const RING2: u32 = 0x01E0_2320;
const RING3: u32 = 0x01E0_2550;
const ARC_A: u32 = 0x01E0_2720;
const ARC_B: u32 = 0x01E0_2910;
const TRAIL: u32 = 0x01E0_2A70;
const ARC_C: u32 = 0x01E0_3080;
const RING4: u32 = 0x01E0_3240;
/// The stub the entry skips (`jr ra`), never spawned.
const STUB: u32 = 0x01E0_33E0;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_37A0;
const POINT: u32 = D + 0xD930;
const VEL_Y: u32 = D + 0xD964;
const VEL_Z: u32 = D + 0xD968;
const TEX: u32 = D + 0xD970;
const TIMER: u32 = D + 0xD990;
const COLOUR: u32 = D + 0xD994;
const OT_AT: u32 = D + 0xD99C;
const TINT_W: u32 = D + 0xD9B4;
const NODES: u32 = D + 0xD9C8;
const CELLS: u32 = D + 0xD9E8;
const PETALS: u32 = D + 0xDA7A;
const ANGLE: u32 = D + 0xDA7E;
const TRAIL_K: u32 = D + 0xDA82;
const INDEX: u32 = D + 0xDA90;
const FACING: u32 = D + 0xDA94;
const COUNTER: u32 = D + 0xDA98;
const BYTES: u32 = D + 0xDA9A;
const BYTES_B: u32 = D + 0xDB9A;
const SINES: u32 = D + 0xDC9C;

const G: Glob = Glob { colour: COLOUR, angle: ANGLE, index: INDEX, cells: CELLS, sines: SINES, ot_at: OT_AT };

/// The steps' saved s0 / s1: the entry's (s0 the draw count, 0 while the
/// steps run; s1 = D).
const VALS: [u64; 2] = [0, D as u64];

/// The entry 0x1E033F0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xDE10);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xDA96, param);
    fx.mem.set32(TIMER, 0x1A4);
    for (k, n) in [0x15, 0x57, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xD9D8, r);
    let tex = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, tex);
    for k in 0..4u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(v, 0x01E0_35E4);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_3614);
        let (_, h) = fx.div(r >> 4, 0x1E);
        fx.mem.write_u8(BYTES + k, (h - 0xF) as u8);
    }
    for k in 0xB4..0x100u32 {
        fx.mem.write_u8(BYTES + k, 0);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_36AC);
        let (_, h) = fx.div(r >> 4, 0x1E);
        fx.mem.write_u8(BYTES_B + k, (h - 0xF) as u8);
    }
    for k in 0..0x24u32 {
        let r = fx.rand_from(D, 0x01E0_3704);
        fx.mem.set16(CELLS + 2 * k, r.wrapping_rem(8));
    }
    0
}

fn ring(fx: &mut Fx, t: u32, r: u32, table: u32) {
    let w = fx.mem.read_u32(TEX + 4);
    fx.mem.write_u32(D + 0x88, w);
    cell(fx, t, r, 0x14);
    fx.mem.set16(D + 0xB8, 2);
    fx.mem.set16(D + 0xB0, 0);
    billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, table);
    fx.mem.set16(D + 0xB8, 0);
    fx.mem.set16(D + 0xB0, 0);
}

fn arcs(fx: &mut Fx, t: u32, n: i32, mode: i32) {
    fx.mem.set16(D + 0xB8, n);
    billboard::arc_strips(fx, D, t, mode, POINT);
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
    let mut drawn = 0u64;
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
            cell(fx, t, sp + 0x70, 0x14);
            fx.mem.set16(D + 0xB8, 1);
            fx.mem.set16(D + 0xBE, 0);
            crate::b764::petal(fx, &G, t, 0x20, POINT);
        }
        if f == TINT {
            let h = fx.self_handle();
            fx.unit_position(h, sp + 0x60);
            // 0x1F71A0's frame below the entry's (its saved ra, s0 = the
            // draw count).
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_3890), (0, drawn)]);
            let z = fx.mem.f(sp + 0x68);
            let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 60.0) } else { ee::sub(z, 60.0) };
            fx.mem.setf(sp + 0x68, z);
            let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
            let mut x = fx.mem.read_u64(sp + 0xE8);
            if let Some(w) = fx.project(p, None, &mut x) {
                fx.mem.write_u64(sp + 0xE8, x);
                fx.mem.setf(TINT_W, w);
            }
            project_residue(fx, sp, sp + 0x60);
            let w = fx.mem.f(TINT_W);
            quads4::tint4(fx, t, 0x21, w);
        }
        if f == RING1 {
            ring(fx, t, sp + 0x80, BYTES);
        }
        debug_assert_ne!(f, RING2);
        if f == RING3 {
            ring(fx, t, sp + 0xA0, BYTES_B);
        }
        if f == ARC_A {
            arcs(fx, t, 0x24, 0x20);
        }
        if f == ARC_B {
            arcs(fx, t, 0xC, 0x20);
        }
        if f == TRAIL {
            let w = fx.mem.read_u32(TEX + 0xC);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x3F)] {
                fx.mem.set16(D + o, v);
            }
            fx.mem.set16(D + 0xBE, 1);
            for c in [0u32, 4, 8] {
                let w = fx.mem.read_u32(POINT + c);
                fx.mem.write_u32(D + 0x20 + c, w);
            }
            let k = fx.mem.i16(t + 0x50);
            tubetrail::tube_with(fx, &tubetrail::SINGLE_BLEND, D, t, trail_of(k), 0x21);
        }
        if f == ARC_C {
            arcs(fx, t, 0x24, 0x21);
        }
        if f == RING4 {
            ring(fx, t, sp + 0xB0, BYTES);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x161 => 1,
        0x19F => 4,
        0x19A => 2,
        0x16C..=0x194 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

const TRAIL_CFG: TrailCfg = TrailCfg {
    div: 10.0,
    heading_b: true,
    width: (0x82, 0x28),
    step: (0x8C, 0x19),
    dim: 0xC0,
    jdiv: 10.0,
    init: [0x01E0_2AE8, 0x01E0_2B14, 0x01E0_2B40, 0x01E0_2C6C, 0x01E0_2CAC, 0x01E0_2CEC, 0x01E0_2D08, 0x01E0_2D28],
    jitter: [0x01E0_2E24, 0x01E0_2E50, 0x01E0_2E7C],
};

fn run_task(fx: &mut Fx, f: u32, t: u32) {
    match f {
        PETAL => petal_step(fx, t),
        TINT => tint(fx, t),
        RING1 => grow_ring(fx, t, &RING1_CFG),
        RING3 => grow_ring(fx, t, &RING3_CFG),
        ARC_A => arc_a(fx, t),
        ARC_B => arc_b(fx, t),
        TRAIL => crate::b764::trail(fx, t, &TRAIL_CFG, SINES, VALS, TASK_RA),
        ARC_C => arc_c(fx, t),
        RING4 => grow_ring(fx, t, &RING4_CFG),
        _ => panic!("b574: unknown task step {f:#x}"),
    }
}

/// 0x1E015E0(D, 0): the update counter, the point and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    let d = u64::from(D);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x20, 0x01E0_3760), (0x10, d), (0, 0)]);
    // The spawns' saved s0..s4: the emitter's s0 (the entry's 0, the loop
    // counts), s1 = D, the entry's s2 (param), s3 / s4 (the runner's, 0).
    let r = |s0: u64| [s0, d, p, 0, 0];
    let mut s0 = 0u64;
    let spawn = |fx: &mut Fx, f: u32, s0: u64, ra: u32| crate::b764::spawn_with(fx, &G, f, r(s0), ra);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let n = fx.mem.i16(COUNTER);
    if n < 0x122 {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        node_position_saving(fx, h, s, POINT, 0, Some(d), 0x01E0_164C);
    } else {
        if n == 0x122 {
            let h = fx.target_handle();
            let s = fx.mem.i16(D + 0xD9D8);
            node_position_saving(fx, h, s, sp + 0x30, 0, Some(d), 0x01E0_1680);
            let z = ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(POINT + 8));
            fx.mem.setf(VEL_Z, ee::div(z, 75.0));
            let y = ee::sub(fx.mem.f(sp + 0x34), fx.mem.f(POINT + 4));
            fx.mem.setf(VEL_Y, ee::div(y, 75.0));
        }
        if fx.mem.i16(COUNTER) < 0x16C {
            let z = ee::add(fx.mem.f(POINT + 8), fx.mem.f(VEL_Z));
            fx.mem.setf(POINT + 8, z);
            let y = ee::add(fx.mem.f(POINT + 4), fx.mem.f(VEL_Y));
            fx.mem.setf(POINT + 4, y);
        }
    }
    let n = fx.mem.i16(COUNTER);
    if (0x7D..0x12D).contains(&n) && fx.mem.i16(PETALS) < 0x24 {
        let k = fx.mem.i16(PETALS);
        fx.mem.set16(ANGLE, k * 10);
        let k = fx.mem.i16(PETALS);
        fx.mem.set16(INDEX, k);
        spawn(fx, PETAL, s0, 0x01E0_17B8);
        let k = fx.mem.i16(PETALS);
        fx.mem.set16(ANGLE, k * 10 + 0xB4);
        let k = fx.mem.i16(PETALS);
        fx.mem.set16(INDEX, k + 0x24);
        spawn(fx, PETAL, s0, 0x01E0_1810);
        let k = fx.mem.i16(PETALS) + 1;
        fx.mem.set16(PETALS, k);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x16D..0x191).contains(&n) {
        for k in 0..2u64 {
            let i = fx.mem.i16(TRAIL_K);
            fx.mem.set16(INDEX, i);
            spawn(fx, TRAIL, k, 0x01E0_1884);
            let i = fx.mem.i16(TRAIL_K) + 1;
            fx.mem.set16(TRAIL_K, i);
            if fx.mem.i16(TRAIL_K) >= 0x50 {
                fx.mem.set16(TRAIL_K, 0);
            }
        }
        s0 = 2;
    }
    if fx.mem.i16(COUNTER) == 0xDC {
        fx.mem.set16(INDEX, 0);
        spawn(fx, ARC_A, s0, 0x01E0_1928);
        fx.mem.set16(INDEX, 0x24);
        spawn(fx, ARC_A, s0, 0x01E0_194C);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x16C..0x191).contains(&n) {
        copy4(fx, D + 0x250, POINT);
        fx.mem.set16(INDEX, 0);
        spawn(fx, RING3, s0, 0x01E0_19B8);
        spawn(fx, RING3, s0, 0x01E0_19CC);
        spawn(fx, ARC_B, s0, 0x01E0_19E0);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x122..0x191).contains(&n) {
        copy4(fx, D + 0x250, POINT);
        fx.mem.set16(INDEX, 0);
        spawn(fx, ARC_C, s0, 0x01E0_1A4C);
        spawn(fx, RING4, s0, 0x01E0_1A60);
    }
    if fx.mem.i16(COUNTER) == 0x122 {
        copy4(fx, D + 0x250, POINT);
        fx.mem.set16(INDEX, 0);
        spawn(fx, TINT, s0, 0x01E0_1AC0);
        for k in 0..10u64 {
            spawn(fx, RING1, k, 0x01E0_1ADC);
        }
    }
    fx.leave(0x50);
}

/// 0x1E01B20: petal +0x50 (k) of 72 at the point: radii 60, both alphas in
/// by 0xC below 0xC0 for 30 updates, out by 0xB from update 0x191; turned
/// 0x4000 about x; its cell rand % 3 + 4; grey 0x80, from age 30 one channel
/// a step (+6, every tenth update) brightened by 5 in both colours (or
/// dimmed in the top one); the inner radius down by 2.5 to 25, then the
/// outer to 20 (k >= 36: 15 / 10; stages 4 / 5 up by 2.5 to 20 / 10); from
/// update 0x123 the y and x angles turning by +-0x300.
fn petal_step(fx: &mut Fx, t: u32) {
    enter_step_with(fx, 0x30, 2, VALS, TASK_RA);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4270_0000);
        fx.mem.write_u32(t + 0x4C, 0x4270_0000);
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
        let v = rand_mod(fx, t, 0x01E0_1BA4, 3);
        fx.mem.set16(t + 8, v + 4);
        fx.mem.set32(t + 0x20, 0x4000);
        fx.mem.set16(t + 6, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
    }
    if fx.mem.i16(t + 0x54) < 0x1E {
        fade_in(fx, t, 0x63, (0xC0, 0xC));
        fade_in(fx, t, 0x67, (0xC0, 0xC));
    } else if fx.mem.i16(COUNTER) >= 0x191 {
        fade_out(fx, t, 0x63, (0xD, 0xB));
        let v = fx.mem.read_u8(t + 0x67);
        if v >= 0xD {
            fx.mem.write_u8(t + 0x67, v - 0xB);
        }
    }
    let a = fx.mem.i16(t + 0x54);
    if a >= 0x1E && a.wrapping_rem(10) == 1 {
        let n = fx.mem.i16(t + 6) + 1;
        fx.mem.set16(t + 6, n);
        if fx.mem.i16(t + 6) >= 6 {
            fx.mem.set16(t + 6, 0);
        }
    }
    let c = fx.mem.i16(t + 6);
    if (0..6).contains(&c) {
        let ch = (c % 3) as u32;
        let top = fx.mem.read_u8(t + 0x60 + ch);
        fx.mem.write_u8(t + 0x60 + ch, if c < 3 { top.wrapping_add(5) } else { top.wrapping_sub(5) });
        let bottom = fx.mem.read_u8(t + 0x64 + ch);
        fx.mem.write_u8(t + 0x64 + ch, bottom.wrapping_add(5));
    }
    if fx.mem.i16(COUNTER) != 0 {
        let s = fx.mem.i16(t + 0x52);
        let r0 = fx.mem.f(t + 0x48);
        let r1 = fx.mem.f(t + 0x4C);
        match s {
            0 | 2 => {
                let floor = if s == 0 { 25.0 } else { 15.0 };
                if r0 <= floor {
                    fx.mem.set16(t + 0x52, s + 1);
                } else {
                    fx.mem.setf(t + 0x48, ee::sub(r0, 2.5));
                }
            }
            1 | 3 => {
                let floor = if s == 1 { 20.0 } else { 10.0 };
                if r1 > floor {
                    fx.mem.setf(t + 0x4C, ee::sub(r1, 2.5));
                }
            }
            4 => {
                if r0 < 20.0 {
                    fx.mem.setf(t + 0x48, ee::add(r0, 2.5));
                } else {
                    fx.mem.set16(t + 0x52, s + 1);
                }
            }
            5 => {
                if r1 < 10.0 {
                    fx.mem.setf(t + 0x4C, ee::add(r1, 2.5));
                }
            }
            _ => {}
        }
    }
    if fx.mem.i16(COUNTER) >= 0x123 {
        turn_by_k(fx, t, 0x300);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// +0x24, then +0x20 turned by +`d` (k < 36), -`d` (k < 72), +`d` (k <
/// 108).
fn turn_by_k(fx: &mut Fx, t: u32, d: i32) {
    for o in [0x24u32, 0x20] {
        let k = fx.mem.i16(t + 0x50);
        let d = if k < 0x24 {
            d
        } else if k < 0x48 {
            -d
        } else if k < 0x6C {
            d
        } else {
            continue;
        };
        let v = fx.mem.i32(t + o).wrapping_add(d);
        fx.mem.set32(t + o, v);
    }
}

/// 0x1E02070: a grey / ochre tint, both alphas in by 0x2A below 0x80 for
/// 12 updates, then out by 0xC (b764's but the colour).
fn tint(fx: &mut Fx, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, TASK_RA);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x40);
        fx.mem.set16(t + 6, 0xC);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, (0x80, 0x2A));
        fade_in(fx, t, 0x67, (0x80, 0x2A));
    } else {
        fade_out(fx, t, 0x63, (0xE, 0xC));
        let v = fx.mem.read_u8(t + 0x67);
        fx.mem.write_u8(t + 0x67, if v >= 0xE { v - 0xC } else { 0 });
    }
    age(fx, t);
    fx.leave(0x20);
}

/// The growing white quad rings 0x1E02170 / 0x1E02550 / 0x1E03240: size,
/// cell 3, three random turns, radius rand % m + add growing by rand % 60 +
/// grow (from its second update; 0x1E03240's set, not grown), rows rand %
/// 20, rand % 2 + 1 set; the
/// alpha `full` for 10 updates (0x1E02550: to update 0x199), then out.
struct GrowCfg {
    frame: u32,
    size: u32,
    m: i32,
    add: i32,
    grow: i32,
    full: u8,
    /// Held to update 0x199 instead of for 10 updates.
    by_counter: bool,
    /// Out by `down.1` from `down.0`.
    down: (u8, u8),
    /// Grown by +0xE from its second update (not 0x1E03240).
    grows: bool,
    /// The eight rands' return addresses.
    ra: [u32; 8],
}

const RING1_CFG: GrowCfg = GrowCfg {
    frame: 0x20,
    size: 0x40C0_0000,
    m: 0x1E,
    add: 3,
    grow: 0x23,
    full: 0xC0,
    by_counter: false,
    down: (0x15, 0x13),
    grows: true,
    ra: [0x01E0_21B8, 0x01E0_21D4, 0x01E0_21F0, 0x01E0_220C, 0x01E0_222C, 0x01E0_2248, 0x01E0_2268, 0x01E0_2284],
};
const RING3_CFG: GrowCfg = GrowCfg {
    frame: 0x30,
    size: 0x40C0_0000,
    m: 0x1E,
    add: 3,
    grow: 0x28,
    full: 0x80,
    by_counter: true,
    down: (0x11, 0xF),
    grows: true,
    ra: [0x01E0_25A0, 0x01E0_25BC, 0x01E0_25D8, 0x01E0_25F4, 0x01E0_2614, 0x01E0_2630, 0x01E0_2650, 0x01E0_266C],
};
const RING4_CFG: GrowCfg = GrowCfg {
    frame: 0x20,
    size: 0x4040_0000,
    m: 0x64,
    add: 0x1E,
    grow: 0x28,
    full: 0x80,
    by_counter: false,
    down: (0x11, 0xF),
    grows: false,
    ra: [0x01E0_3288, 0x01E0_32A4, 0x01E0_32C0, 0x01E0_32DC, 0x01E0_32FC, 0x01E0_3318, 0x01E0_3338, 0x01E0_3354],
};

fn grow_ring(fx: &mut Fx, t: u32, c: &GrowCfg) {
    enter_step_with(fx, c.frame, c.frame / 0x10 - 1, VALS, TASK_RA);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, c.size);
        fx.mem.set16(t + 0x56, 3);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        for (o, ra) in [(0x20u32, c.ra[0]), (0x24, c.ra[1]), (0x28, c.ra[2])] {
            let v = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let v = rand_mod(fx, t, c.ra[3], c.m);
        fx.mem.set16(t + 0x50, v + c.add);
        let v = rand_mod(fx, t, c.ra[4], 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, c.ra[5], 0x3C);
        fx.mem.set16(t + 0xE, v + c.grow);
        let v = rand_mod(fx, t, c.ra[6], 0x14);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, c.ra[7]);
        fx.mem.set16(t + 6, v.wrapping_rem(2) + 1);
        fx.mem.set16(t + 0x58, 2);
    }
    let held = if c.by_counter { fx.mem.i16(COUNTER) < 0x19A } else { fx.mem.i16(t + 0x54) < 0xA };
    if held {
        fx.mem.write_u8(t + 0x63, c.full);
    } else {
        fade_out(fx, t, 0x63, c.down);
    }
    if c.grows && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
    fx.leave(c.frame);
}

/// 0x1E02720: a grey arc fan at the point (radii 300 / 250, k >= 36: 200 /
/// 150; 10-degree arcs), turned 0x4000 about x, its outer alpha in by 0xA
/// below 0x80 to update 0x18F, then out by 0xD; from update 0x123 its y / x
/// angles turning by +-0x180.
fn arc_a(fx: &mut Fx, t: u32) {
    enter_step_with(fx, 0x30, 2, VALS, TASK_RA);
    if fx.mem.i16(t + 0x58) == 1 {
        let k = fx.mem.i16(t + 0x50);
        if k < 0x24 {
            fx.mem.write_u32(t + 0x48, 0x4396_0000);
            fx.mem.write_u32(t + 0x4C, 0x437A_0000);
        } else if k < 0x48 {
            fx.mem.write_u32(t + 0x48, 0x4348_0000);
            fx.mem.write_u32(t + 0x4C, 0x4316_0000);
        }
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set32(t + 0x20, 0x4000);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(COUNTER) < 0x190 {
        fade_in(fx, t, 0x67, (0x80, 0xA));
    } else {
        fade_out(fx, t, 0x67, (0xF, 0xD));
    }
    if fx.mem.i16(COUNTER) >= 0x123 {
        turn_by_k(fx, t, 0x180);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02910: a light grey / yellow arc at the point (radii 40 / 100
/// growing by 1 / 55, 10-degree pieces every 2, three random turns), the
/// outer alpha in by 0x50 below 0xA0 for 3 updates, then out by 0xF.
fn arc_b(fx: &mut Fx, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, TASK_RA);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4220_0000);
        fx.mem.write_u32(t + 0x4C, 0x42C8_0000);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0x40);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 2);
        for (o, ra) in [(0x20u32, 0x01E0_2978), (0x24, 0x01E0_2994), (0x28, 0x01E0_29B0)] {
            let v = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 3 {
        fade_in(fx, t, 0x67, (0xA0, 0x50));
    } else {
        fade_out(fx, t, 0x67, (0x11, 0xF));
    }
    let r = ee::add(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, r);
    let r = ee::add(fx.mem.f(t + 0x4C), 55.0);
    fx.mem.setf(t + 0x4C, r);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03080: a dark arc fan at the point (radius 10 + (rand >> 4) % 70
/// growing by 1, from update 0x168 25 + (rand >> 4) % 100 growing by 3),
/// facing the camera turned by 0x4000 about x, the outer alpha in by 0x50
/// below 0xA0 for 3 updates, then out by 0xC.
fn arc_c(fx: &mut Fx, t: u32) {
    enter_step_with(fx, 0x30, 2, VALS, TASK_RA);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        let (ra, m, base) = if fx.mem.i16(COUNTER) < 0x168 { (0x01E0_30C8, 0x46, 10.0) } else { (0x01E0_3100, 0x64, 25.0) };
        let r = fx.rand_from(t, ra);
        let (_, h) = fx.div(r >> 4, m);
        fx.mem.setf(t + 0x4C, ee::add(base, ee::from_i32(h)));
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x60);
        l2::rgb(fx, t + 0x64, 0x40, 0x20, 0x30);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 3 {
        fade_in(fx, t, 0x67, (0xA0, 0x50));
    } else {
        fade_out(fx, t, 0x67, (0xE, 0xC));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_31C4);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    let g = if fx.mem.i16(COUNTER) < 0x168 { 1.0 } else { 3.0 };
    let r = ee::add(fx.mem.f(t + 0x4C), g);
    fx.mem.setf(t + 0x4C, r);
    age(fx, t);
    fx.leave(0x30);
}
