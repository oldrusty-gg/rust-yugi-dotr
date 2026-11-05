//! BMODULE b495 (card 254), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02D80, data area 0x1ED0000, 0xF060 bytes) [F]
//!: b544's trail program on two pools - A, 320
//! tasks of 0x40 bytes at D+0x380 (spawned once by 0x1E03450, never
//! stepped or drawn), B, 320 tasks of 0x70 at D+0x5380 (alive +0x58,
//! step +0x44 through `jalr`, 0x1E02CC0 the idle step; spawned by
//! 0x1E035C0, c9381311efc0, `spawn::task400_b4`'s layout copy: colour
//! D+0xEC84, point D+0x2D0, +0x50 from D+0xECE6) - drawn with quads
//! (`billboard::quad`, uv_rect4), arcs (`billboard::arc_strips`), flat
//! facing rings (`quads4::ring_facing_flat`) and b703's gouraud ribbon
//! (0x1E03750, a register compile) between 10-point histories D+0xDFD0 +
//! 0xA0 j. Update 0x13 the object op 0x1F8380(2, ..) to update 0x8B; 0x2A
//! / 0x43 thirteen white motes at each of nodes 0x4A / 0x50; 0x5C two red
//! trails (nodes 0x21 / 0x22 and 0x38 / 0x39, four histories each, 1.4
//! apart in y); 0x5F ten grey rings at each of nodes 0x21 / 0x22, a red
//! arc ring and pool A's task. The draw loop reads some cells from pool A
//! at the same index (D+0x40 k + 0x3B4 / + 0x38A) as the record does.
//! Returns 1 on update 0x51, 4 on 0x91, 2 on 0x5B, 8 once the 0x96-update
//! timer D+0xEC80 has run out.
use crate::b494::camera_angles_saving;
use crate::b076::node_position_saving;
use crate::blib::{billboard, quads4, spawn, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 320;
/// Pool A (0x40-byte tasks) and pool B (0x70-byte tasks).
const POOL_A: u32 = D + 0x380;
const POOL_B: u32 = D + 0x5380;
const AT: u32 = D + 0x2D0;
const ARC: u32 = 0x01E0_1A50;
const MOTE: u32 = 0x01E0_1B90;
const RING: u32 = 0x01E0_1EE0;
const TRAIL: u32 = 0x01E0_2140;
const PLACED: u32 = 0x01E0_2A40;
const OBJECT: u32 = 0x01E0_2C30;
const IDLE: u32 = 0x01E0_2CC0;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3024;
/// The 10-point histories (0xA0 bytes each).
const HIST: u32 = D + 0xDFD0;
const TEX5: u32 = D + 0xEC60;
const TIMER: u32 = D + 0xEC80;
const COLOUR: u32 = D + 0xEC84;
const OTW: u32 = D + 0xEC8C;
const NODES: u32 = D + 0xECCC;
const SPAWN_K: u32 = D + 0xECE6;
const FACING: u32 = D + 0xECEA;
const COUNTER: u32 = D + 0xECEC;
const BYTES: u32 = D + 0xECF0;
const BYTES_B: u32 = D + 0xEDF0;
const SINES: u32 = D + 0xEEF0;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn rmod4(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v >> 4, m).1
}

fn hist(j: u32) -> u32 {
    HIST + 0xA0 * j
}

/// The entry 0x1E02D80 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xE0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xE0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xF060);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xECE8, param);
    fx.mem.set32(TIMER, 0x96);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, n) in [0x21, 0x22, 0x38, 0x39, 0x4A, 0x50].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // 0x1E02CD0(0, D): the textures (frame 0x40: ra, the entry's s2 = D,
    // s1 = param, s0 = 0).
    let f = fx.enter(0x40);
    fx.saved(f, &[(0x30, 0x01E0_2EE8), (0x20, u64::from(D)), (0x10, param as i64 as u64), (0, 0)]);
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x340 + 4 * k as u32, t);
    }
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x378 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX5 + 4 * k as u32, t);
    }
    fx.leave(0x40);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.fix_residue(y, 0x01E0_2F24);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_2F38, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_2F70, 0x1E);
        fx.mem.set8(BYTES_B + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, k, 0x01E0_2FA8, 0x14);
        fx.mem.set8(BYTES_B + k, v - 0xA);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    let mut idle = 0i32;
    for k in 0..TASKS {
        let t = POOL_B + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        if f == IDLE {
            idle = (idle + 1) as i16 as i32;
            continue;
        }
        // The entry's s0 = the idle count, s1 = D + 0x70 k, s2 = D, s3 = 0.
        let (s0, s1) = (idle as i64 as u64, u64::from(D + 0x70 * k));
        let frame = match f {
            RING => 0x20,
            TRAIL => 0x60,
            OBJECT => 0x40,
            _ => 0x30,
        };
        let s = fx.enter(frame);
        match frame {
            0x20 => fx.saved(s, &[(0x10, TASK_RA), (0, s0)]),
            0x60 => fx.saved(s, &[(0x40, TASK_RA), (0x30, 0), (0x20, u64::from(D)), (0x10, s1), (0, s0)]),
            _ => fx.saved(s, &[(0x20, TASK_RA), (0x10, s1), (0, s0)]),
        }
        match f {
            ARC => arc(fx, t),
            MOTE => mote(fx, t),
            RING => ring(fx, t),
            TRAIL => trail(fx, t, s),
            OBJECT => object(fx, t, s),
            _ => panic!("b495: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    // The draw loop: s0 = D + 0x70 k, s1 = D + 0x40 k, s3 the live count,
    // s7 = k.
    let mut s3 = 0u32;
    for k in 0..TASKS {
        let t = POOL_B + 0x70 * k;
        let a = D + 0x40 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        s3 += 1;
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == MOTE {
            let w = fx.mem.read_u32(TEX5);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(a + 0x3B4);
            cell(fx, sp + 0xA0, 0x14, c);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
        }
        if f(fx) == RING {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xC0, 0x14, c);
            let w = fx.mem.read_u32(TEX5 + 4);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 6);
            let v = fx.mem.i16(a + 0x38A);
            fx.mem.set16(D + 0xC0, v);
            for c in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * c);
                fx.mem.write_u32(D + 0x20 + 4 * c, w);
            }
            quads4::ring_facing_flat(fx, uv::uv_rect4, D, t, 0x20, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == TRAIL {
            let base = [u64::from(D + 0x70 * k), u64::from(a), u64::from(D), u64::from(s3)];
            if fx.mem.i16(t + 0x50) == 0 {
                for j in 0..4u32 {
                    let s = [base[0], base[1], base[2], base[3], u64::from(j), u64::from(D + 0xA0 * j), u64::from(t), u64::from(k), 0, 0x01E0_3308];
                    crate::b703::ribbon_saving(fx, t, hist(j), hist(j + 4), s);
                }
            }
            if fx.mem.i16(t + 0x50) == 1 {
                for j in 10..14u32 {
                    let s = [base[0], base[1], base[2], base[3], u64::from(D + 0x640 + 0xA0 * (j - 10)), u64::from(t), u64::from(j), u64::from(k), 0, 0x01E0_3368];
                    crate::b703::ribbon_saving(fx, t, hist(j), hist(j + 4), s);
                }
            }
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x51 => 1,
        0x91 => 4,
        0x5B => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E035C0(D, step, 0): `spawn::task70` on this record's pool B, its
/// frame's saves the emitter's s0 = D / s1, the entry's s2 = D / s3 = 0.
fn spawn_b(fx: &mut Fx, step: u32, s1: u64, ra: u32) -> i32 {
    let p = spawn::pool70_b4("c9381311efc0").expect("layout");
    let saves = [(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, s1), (0, u64::from(D))];
    spawn::task70(fx, D, p, step, 0, &saves)
}

/// 0x1E03450(D, step, 0): the first free task of pool A (alive +0x36)
/// cleared and set off: alive 1, the colour D+0xEC84 (bytes) with its
/// alpha at +0x28 and +0x2C, D+0x2D0 as i16 at +0 (0x107390), +0x30 from
/// D+0xECE6, `arg` +0x20, `step` +0x24. Saves as [`spawn_b`]'s.
fn spawn_a(fx: &mut Fx, step: u32, s1: u64, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, s1), (0, u64::from(D))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL_A + 0x40 * k + 0x36) == 0) {
        Some(k) => {
            let t = POOL_A + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for o in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            l2::to_i16x4(fx, t, AT, 0);
            let s = fx.mem.i16(SPAWN_K);
            fx.mem.set16(t + 0x30, s);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

fn node_pos(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES + 2 * k);
    fx.node_position(h, slot, out);
}

/// 0x1E01500(D, 0): the colour (0x80, 0, 0, 0), the update counter and
/// the spawns (frame 0x30: ra, the entry's s1 = param, s0 = 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_2FE8), (0x10, param as i64 as u64), (0, 0)]);
    for (o, v) in [(0u32, 0x80u8), (1, 0), (2, 0), (3, 0)] {
        fx.mem.write_u8(COLOUR + o, v);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    // The emitter's s1: the entry's param until a loop counts in it.
    let mut s1 = param as i64 as u64;
    if c(fx) == 0x5C {
        fx.mem.set16(SPAWN_K, 0);
        spawn_b(fx, TRAIL, s1, 0x01E0_1594);
        fx.mem.set16(SPAWN_K, 1);
        spawn_b(fx, TRAIL, s1, 0x01E0_15B8);
    }
    if c(fx) == 0x13 {
        spawn_b(fx, OBJECT, s1, 0x01E0_15E4);
    }
    if c(fx) == 0x2A || c(fx) == 0x43 {
        for (node, ra) in [(4u32, 0x01E0_1654u32), (5, 0x01E0_16B0)] {
            node_pos(fx, node, AT);
            let y = ee::sub(fx.mem.f(AT + 4), 1.5);
            fx.mem.setf(AT + 4, y);
            for k in 0..0xDu64 {
                s1 = k;
                spawn_b(fx, MOTE, s1, ra);
            }
            s1 = 0xD;
        }
    }
    if c(fx) == 0x5F {
        for (node, ra) in [(0u32, 0x01E0_1710u32), (1, 0x01E0_1758)] {
            node_pos(fx, node, AT);
            for k in 0..0xAu64 {
                s1 = k;
                spawn_b(fx, RING, s1, ra);
            }
            s1 = 0xA;
        }
        spawn_b(fx, ARC, s1, 0x01E0_177C);
        spawn_a(fx, PLACED, s1, 0x01E0_1790);
    }
    fx.leave(0x30);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01A50 (D, task): b564's arc ring in red (radius 220), ending on
/// update 0xC3.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.write_u32(t + 0x4C, 0x435C_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 8 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a + 0x60);
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x67, a - 0xF);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1B18);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x4C), f32::from_bits(0x3ECC_CCCD));
        fx.mem.setf(t + 0x4C, r);
    }
    if fx.mem.i16(COUNTER) == 0xC3 {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
}

/// 0x1E01B90 (D, task): b646's white mote with size 130, pushed out by
/// 8.6 at first.
fn mote(fx: &mut Fx, t: u32) {
    let swing = |fx: &mut Fx, k: f32| {
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let z = ee::add(fx.mem.f(t + 0x18), ee::mul(k, s));
        fx.mem.setf(t + 0x18, z);
        let a = fx.mem.i16(t + 0x5A);
        let c = l2::sin_deg(fx, SINES, a, 1);
        let x = ee::add(fx.mem.f(t + 0x10), ee::mul(k, c));
        fx.mem.setf(t + 0x10, x);
    };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x82);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = rmod4(fx, t, 0x01E0_1BC8, 3);
        fx.mem.set16(t + 0x56, v + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod4(fx, t, 0x01E0_1C00, 0x168);
        fx.mem.set16(t + 0x5A, v);
        swing(fx, f32::from_bits(0x4109_999A));
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xA {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        swing(fx, f32::from_bits(0x3F4C_CCCD));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1D38);
    age(fx, t);
}

/// 0x1E01EE0 (D, task): a grey ring (size 10..24, cell 0..6, angles
/// +0x5A / +0x5C random, +0xA rand % 8, +0xC the spawn index, radius
/// 10..29 growing by 15..39, +0x52 300..749, row 0..34, life 3..7): in by
/// 0x20 under 0x80, out by 8 (the task ends under 10); grey 0x80 on odd
/// ages, 0xC0 on even.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod4(fx, t, 0x01E0_1F04, 0xF);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = rmod(fx, t, 0x01E0_1F34, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_1F60, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_1F7C, 0x168);
        fx.mem.set16(t + 0x5C, v);
        let v = fx.rand_from(t, 0x01E0_1F98);
        fx.mem.set16(t + 0xA, v % 8);
        let i = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, i);
        let v = rmod4(fx, t, 0x01E0_1FBC, 0x14);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_1FE0, 0x1C2);
        fx.mem.set16(t + 0x52, v + 0x12C);
        let v = rmod(fx, t, 0x01E0_2000, 0x19);
        fx.mem.set16(t + 0xE, v + 0xF);
        let v = rmod(fx, t, 0x01E0_2020, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_203C, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x20));
        }
    } else if a >= 0xA {
        fx.mem.write_u8(t + 0x63, a - 8);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let g = if fx.mem.i16(t + 0x54) % 2 == 1 { 0x80 } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// The trail's nodes (slot, first history, the 0x1F6830 return addresses
/// of the first fill and of the history steps).
const TRAIL_NODES: [[(u32, u32, u32, u32); 2]; 2] = [
    [(0, 0, 0x01E0_21BC, 0x01E0_2738), (1, 4, 0x01E0_22FC, 0x01E0_27EC)],
    [(2, 10, 0x01E0_244C, 0x01E0_28C4), (3, 14, 0x01E0_258C, 0x01E0_2978)],
];

/// 0x1E02140 (D, task): b544's red trail (0xC0, 0x20, 0x20; frame 0x60)
/// on 10-point histories: on its first update +0x48 5, +0xE 30 and its
/// histories filled (9 points each); alpha 0xFF until update 0x69, then
/// down by 0x15 (the task ends under 0x17); until update 0x61 the
/// histories take the nodes' points; +6 6.
fn trail(fx: &mut Fx, t: u32, sp: u32) {
    let y_down = |fx: &mut Fx| {
        let y = ee::sub(fx.mem.f(sp + 0x54), f32::from_bits(0x3FB3_3333));
        fx.mem.setf(sp + 0x54, y);
    };
    l2::rgb(fx, t + 0x60, 0xC0, 0x20, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        let kind = fx.mem.i16(t + 0x50);
        if (0..2).contains(&kind) {
            fx.mem.set16(t + 0xE, 0x1E);
            for (slot, j0, ra, _) in TRAIL_NODES[kind as usize] {
                let h = fx.self_handle();
                let n = fx.mem.i16(NODES + 2 * slot);
                node_position_saving(fx, h, n, sp + 0x50, u64::from(t), Some(u64::from(D)), ra);
                for j in j0..j0 + 4 {
                    if j != j0 {
                        y_down(fx);
                    }
                    let a = hist(j);
                    for e in 0..9u32 {
                        for c in 0..3u32 {
                            let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
                            fx.mem.write_u32(a + 0x10 * e + 4 * c, w);
                        }
                    }
                }
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) < 0x6A {
        fx.mem.write_u8(t + 0x63, 0xFF);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x17 {
            fx.mem.write_u8(t + 0x63, a - 0x15);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    for kind in 0..2i32 {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(COUNTER) < 0x62 {
            for (slot, j0, _, ra) in TRAIL_NODES[kind as usize] {
                let h = fx.self_handle();
                let n = fx.mem.i16(NODES + 2 * slot);
                node_position_saving(fx, h, n, sp + 0x50, u64::from(t), Some(u64::from(D)), ra);
                for j in j0..j0 + 4 {
                    if j != j0 {
                        y_down(fx);
                    }
                    spawn::history(fx, hist(j) + 0x90, 0xA, sp + 0x50);
                }
            }
        }
    }
    fx.mem.set16(t + 6, 6);
    age(fx, t);
}

/// 0x1E02C30 (D, task): the object op 0x1F8380(2, 0, the record's floats
/// 0x1E03D00 / 0x1E03D08 at sp + 0x38 / + 0x3C, 0) each update; the task
/// ends from update 0x8C.
fn object(fx: &mut Fx, t: u32, sp: u32) {
    for (to, from) in [(0x38u32, 0x01E0_3D00u32), (0x3C, 0x01E0_3D08)] {
        let w = fx.mem.read_u32(from);
        fx.mem.write_u32(sp + to, w);
    }
    fx.boundary(0x001F_8380, "object op 0x1F8380", [2, 0, sp + 0x38, sp + 0x3C]);
    age(fx, t);
    if fx.mem.i16(COUNTER) >= 0x8C {
        fx.mem.set16(t + 0x58, 0);
    }
}
