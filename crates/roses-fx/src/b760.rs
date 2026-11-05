//! BMODULE b760 (card 291), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02B00, data area a0 = 0x1ED0000, 0xDB00 bytes)
//! [F]: b521's task program (0x70 tasks, `jalr`
//! steps) with a 450-task pool at D+0x11B0 and no trails. The emitter
//! 0x1E00CE0 keeps b521's ten spinning points (D+0xD700, radii never set)
//! and the attacker's point 25 down (D+0xD7A0, z sliding by a fiftieth of
//! the units' z gap over 0xAA..0xDC); 0x40 twenty-four white rings
//! (0x1E01F30 = b521's ring with radius 200..299) 15 degrees apart at
//! the attacker, 30 up; 0xA1..0x140 four grey quads (0x1E02110: size
//! 80..119, sinking 2.6 and sliding 2.4 along z by the facing) a update
//! at the attacker's nodes 0x1D / 0x3C, 0xA1..0x13A two white quads
//! (0x1E024A0: size 5..17, spinning) and on odd updates 0x3D..0x7D two
//! small white flashes (0x1E02820: size 25..42, 10 updates in). The steps
//! 0x1E01A10 (b521's quad) and 0x1E01D50 (b521's ring) are drawn but never
//! spawned, not ported. Returns 1 / 4 / 2 / 6 / 8 on 0x96 / 0x172 / 0x163
//! / 0xA1..0x15D / timer out.
use crate::b076::{node_position_saving, request_node_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 450;
/// The step functions a task holds (+0x44), as the record's addresses.
const RING: u32 = 0x01E0_1F30;
const GREY: u32 = 0x01E0_2110;
const WHITE: u32 = 0x01E0_24A0;
const FLASH: u32 = 0x01E0_2820;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_2D54;
const POINTS: u32 = D + 0xD700;
const CENTRE: u32 = D + 0xD7A0;
const SELF_POS: u32 = D + 0xD7B0;
const SLIDE: u32 = D + 0xD7C8;
const TEX5: u32 = D + 0xD7D0;
const TIMER: u32 = D + 0xD7F0;
const COLOUR: u32 = D + 0xD7F4;
const OTW: u32 = D + 0xD7FC;
const RADII: u32 = D + 0xD804;
const ANGLES: u32 = D + 0xD844;
const NODES: u32 = D + 0xD858;
const FACING: u32 = D + 0xD878;
const INDEX: u32 = D + 0xD882;
const SIDE: u32 = D + 0xD884;
const COUNTER: u32 = D + 0xD888;
const BYTES: u32 = D + 0xD88A;
const SINES: u32 = D + 0xD98C;

/// The entry 0x1E02B00 (a0 = D, a1 = param; frame 0xD0).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    fx.mem.write_u32(OTW, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let p = param as i64 as u64;
    let d = u64::from(D);
    fx.bzero(D, 0xDB00);
    l2::sine_table_saving(fx, SINES, d, 0, 0x01E0_2B58);
    l2::sine_table_saving(fx, D + 0xD8, d, 0, 0x01E0_2B60);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2C8 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX5 + 4 * k as u32, t);
    }
    let s = [Some(d), Some(d + 0xC), Some(p)];
    for (k, (node, clear, ra)) in [(1, 1, 0x01E0_2BD0u32), (0x1D, 0, 0x01E0_2BF0), (0x3C, 0, 0x01E0_2C10), (0x1D, 0, 0x01E0_2C30), (0x3C, 0, 0x01E0_2C50)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = request_node_saving(fx, h, node, clear, s, ra);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_2C94);
    fx.mem.set16(SIDE, ee::fixsfsi(y));
    fx.mem.set16(D + 0xD886, param);
    fx.mem.set32(TIMER, 0x186);
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_2CC8);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.set8(BYTES + k, 0);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x11B0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x44) {
            0 => {}
            RING => ring_step(fx, t),
            GREY => grey_step(fx, t),
            WHITE | FLASH => quad_step(fx, t),
            f => panic!("b760: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x11B0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == RING {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, sub);
            let w = fx.mem.read_u32(TEX5 + 8);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 5);
            let o = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, o);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            quads4::ring_facing_flat(fx, billboard::uv_rect_q, D, t, 0x10, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == GREY || f == WHITE || f == FLASH {
            let (tex, local, mode) = match f {
                GREY => (TEX5 + 8, sp + 0x90, 0x10),
                WHITE => (TEX5 + 4, sp + 0xA0, 0x20),
                _ => (TEX5 + 4, sp + 0xB0, 0x20),
            };
            let w = fx.mem.read_u32(tex);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, local, sub);
            if f != GREY {
                for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x3F)] {
                    fx.mem.set16(D + o, v);
                }
            }
            fx.mem.set16(D + 0xBE, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x50 + 4 * k, w);
            }
            billboard::quad(fx, billboard::uv_rect_q, D, t, mode, sp + 0x50);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x96 => 1,
        0x172 => 4,
        0x163 => 2,
        0xA1..=0x15D => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x106960(r, 0x14, sub) and its 16 bytes copied to D+0x94.
fn cell(fx: &mut Fx, r: u32, sub: i32) {
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// rand % m (HI) from the emitter's `jal` at `site` (rand's frame: s0 = D).
fn erand(fx: &mut Fx, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(D, site + 8);
    fx.div(v, m).1
}

/// 0x1E00CE0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the spawns (frame 0xB0: ra, the runner's s4 / s3 (0), the entry's
/// s2 = the parameter, s1 = 0, s0 = D, the runner's f20 (0 [H])).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0xB0);
    let p = param as i64 as u64;
    let d = u64::from(D);
    fx.saved(sp, &[(0x60, 0x01E0_2D28), (0x50, 0), (0x40, 0), (0x30, p), (0x20, 0), (0x10, d)]);
    fx.mem.write_u32(sp, 0);
    // The emitter's s1..s4 at its calls (s0 = D throughout).
    let mut s1 = 0u64;
    for (k, v) in [0x80u8, 0, 0, 0].into_iter().enumerate() {
        fx.mem.write_u8(COLOUR + k as u32, v);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 1 {
        let h = fx.self_handle();
        unit_position_saving(fx, h, SELF_POS, d, 0x01E0_0D80);
        let x = fx.mem.read_u32(SELF_POS);
        fx.mem.write_u32(CENTRE, x);
        let y = ee::sub(fx.mem.f(SELF_POS + 4), 25.0);
        fx.mem.setf(CENTRE + 4, y);
        let z = fx.mem.read_u32(SELF_POS + 8);
        fx.mem.write_u32(CENTRE + 8, z);
        let h = fx.self_handle();
        unit_position_saving(fx, h, sp + 0x80, d, 0x01E0_0DE4);
        let h = fx.target_handle();
        unit_position_saving(fx, h, sp + 0x90, d, 0x01E0_0DF8);
        let dz = ee::sub(fx.mem.f(sp + 0x98), fx.mem.f(sp + 0x88));
        fx.mem.setf(SLIDE, ee::div(dz, 50.0));
    }
    let c = fx.mem.i16(COUNTER);
    if (0xAA..0xDD).contains(&c) {
        let z = ee::add(fx.mem.f(CENTRE + 8), fx.mem.f(SLIDE));
        fx.mem.setf(CENTRE + 8, z);
    }
    let h = fx.target_handle();
    let slot = fx.mem.i16(NODES + 2);
    node_position_saving(fx, h, slot, sp + 0x70, d, Some(s1), 0x01E0_0E88);
    for k in 0..10u32 {
        let a = fx.mem.i16(ANGLES + 2 * k);
        let c = l2::sin_deg(fx, SINES, a, 1);
        let r = fx.mem.f(RADII + 4 * k);
        fx.mem.setf(POINTS + 0x10 * k, ee::mul(r, c));
        let a = fx.mem.i16(ANGLES + 2 * k);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let r = fx.mem.f(RADII + 4 * k);
        fx.mem.setf(POINTS + 8 + 0x10 * k, ee::mul(r, s));
        let a = fx.mem.i16(ANGLES + 2 * k) + 4;
        fx.mem.set16(ANGLES + 2 * k, a);
        let a = fx.mem.i16(ANGLES + 2 * k);
        if a >= 0x168 {
            let (_, h) = fx.div(a, 0x168);
            fx.mem.set16(ANGLES + 2 * k, h);
        }
    }
    s1 = 10;
    let (s3, s4) = (d + 0x28, d + 0xA0);
    let mut s2 = d + 0x14;
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_102C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    let f = fx.mem.i16(FACING) - 0xB4;
    fx.mem.set16(FACING, f);
    let c = fx.mem.i16(COUNTER);
    if (0x40..0x141).contains(&c) {
        for _ in 0..2 {
            for k in 0..3 {
                let w = fx.mem.read_u32(CENTRE + 4 * k);
                fx.mem.write_u32(D + 0x250 + 4 * k, w);
            }
        }
    }
    if fx.mem.i16(COUNTER) == 0x40 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, sp + 0x70, d, Some(s1), 0x01E0_1110);
        let h = fx.self_handle();
        unit_position_saving(fx, h, sp + 0x70, d, 0x01E0_1124);
        let mut a = 0i32;
        for k in 0..0x18u64 {
            let x = fx.mem.read_u32(sp + 0x70);
            fx.mem.write_u32(D + 0x250, x);
            fx.mem.write_u32(D + 0x254, 0x41F0_0000);
            let z = fx.mem.read_u32(sp + 0x78);
            fx.mem.write_u32(D + 0x258, z);
            fx.mem.set16(INDEX, a);
            spawn_task(fx, RING, 0, [d, k, a as i64 as u64, s3, s4], 0x01E0_1170);
            a = (a + 0xF) as i16 as i32;
        }
        s1 = 0x18;
        s2 = a as i64 as u64;
    }
    let side = |fx: &mut Fx, site: u32| -> f32 {
        if fx.mem.i16(SIDE) != 0 {
            ee::from_i32(erand(fx, site, 0x1E) - 0x14)
        } else {
            ee::from_i32(erand(fx, site + 0x2C, 0x1E) - 0xA)
        }
    };
    let c = fx.mem.i16(COUNTER);
    if (0xA1..0x141).contains(&c) {
        // (node slot, the node call's jal, the spawn's jal) for each group
        // (the last computes a point but spawns nothing).
        for (k, (slot, at, spawn)) in [(1u32, 0x01E0_11C4u32, Some(0x01E0_1298u32)), (2, 0x01E0_12B8, Some(0x01E0_1384)), (1, 0x01E0_13A4, Some(0x01E0_1470)), (2, 0x01E0_1490, Some(0x01E0_155C)), (1, 0x01E0_157C, None)].into_iter().enumerate() {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2 * slot);
            node_position_saving(fx, h, slot, sp + 0x70, d, Some(s1), at + 8);
            let f20 = side(fx, at + 0x1C);
            let r = erand(fx, at + 0x74, 10);
            let x = ee::sub(ee::add(fx.mem.f(sp + 0x70), ee::from_i32(r)), 5.0);
            fx.mem.setf(D + 0x250, x);
            let y = fx.mem.read_u32(sp + 0x74);
            fx.mem.write_u32(D + 0x254, y);
            let v = ee::add(fx.mem.f(sp + 0x78), f20);
            fx.mem.setf(D + 0x258, v);
            if k == 0 {
                s1 = u64::from(sp + 0x74);
                s2 = u64::from(sp + 0x78);
            }
            if let Some(j) = spawn {
                spawn_task(fx, GREY, 0, [d, s1, s2, s3, s4], j + 8);
            }
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0xA1..0x13B).contains(&c) {
        for (slot, at, j) in [(1u32, 0x01E0_1678u32, 0x01E0_174Cu32), (2, 0x01E0_176C, 0x01E0_1838)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2 * slot);
            node_position_saving(fx, h, slot, sp + 0x70, d, Some(s1), at + 8);
            let f20 = side(fx, at + 0x1C);
            let r = erand(fx, at + 0x74, 10);
            let x = ee::sub(ee::add(fx.mem.f(sp + 0x70), ee::from_i32(r)), 5.0);
            fx.mem.setf(D + 0x250, x);
            let y = fx.mem.read_u32(sp + 0x74);
            fx.mem.write_u32(D + 0x254, y);
            let v = ee::add(fx.mem.f(sp + 0x78), f20);
            fx.mem.setf(D + 0x258, v);
            s1 = u64::from(sp + 0x74);
            s2 = u64::from(sp + 0x78);
            spawn_task(fx, WHITE, 0, [d, s1, s2, s3, s4], j + 8);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x3D..0x7E).contains(&c) && c & 1 == 1 {
        for (slot, at, sp_at) in [(3u32, 0x01E0_189Cu32, 0x01E0_1934u32), (4, 0x01E0_1954, 0x01E0_19E4)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2 * slot);
            node_position_saving(fx, h, slot, sp + 0x70, d, Some(s1), at + 8);
            let a = ee::from_i32(erand(fx, at + 8, 0x14) - 0xA);
            let b = erand(fx, at + 0x2C, 0x14);
            let a = ee::div(a, 15.0);
            let v = ee::add(fx.mem.f(sp + 0x70), a);
            fx.mem.setf(D + 0x250, v);
            let y = fx.mem.read_u32(sp + 0x74);
            fx.mem.write_u32(D + 0x254, y);
            let b = ee::div(ee::from_i32(b - 0xA), 15.0);
            let v = ee::add(fx.mem.f(sp + 0x78), b);
            fx.mem.setf(D + 0x258, v);
            s1 = u64::from(sp + 0x74);
            s2 = u64::from(sp + 0x78);
            spawn_task(fx, FLASH, 0, [d, s1, s2, s3, s4], sp_at + 8);
        }
    }
    let _ = (s1, s2);
    fx.leave(0xB0);
}

/// 0x1E03260(D, step, arg): a new task (the first free of 450) with the
/// colour D+0xD7F4 (both +0x60 / +0x64), at D+0x250, index D+0xD882 in
/// +0x50 / +0x52; 1, or 0 (frame 0x60: the caller's s0..s4 and `ra`).
fn spawn_task(fx: &mut Fx, step: u32, arg: u32, s: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, s[4]), (0x30, s[3]), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x1208 + 0x70 * k) == 0) else {
        fx.leave(0x60);
        return 0;
    };
    let t = D + 0x11B0 + 0x70 * k;
    fx.bzero(t, 0x70);
    fx.mem.set16(t + 0x58, 1);
    for o in [0x60u32, 0x64] {
        let (r, g, b) = (fx.mem.u8(COLOUR), fx.mem.u8(COLOUR + 1), fx.mem.u8(COLOUR + 2));
        l2::rgb(fx, t + o, r, g, b);
        let a = fx.mem.read_u8(COLOUR + 3);
        fx.mem.write_u8(t + o + 3, a);
    }
    l2::to_i16x4(fx, t, D + 0x250, 0);
    for k in 0..3 {
        let w = fx.mem.read_u32(D + 0x250 + 4 * k);
        fx.mem.write_u32(t + 0x10 + 4 * k, w);
    }
    let v = fx.mem.i16(INDEX);
    fx.mem.set16(t + 0x50, v);
    let v = fx.mem.i16(INDEX);
    fx.mem.set16(t + 0x52, v);
    fx.mem.write_u32(t + 0x40, arg);
    fx.mem.write_u32(t + 0x44, step);
    fx.leave(0x60);
    1
}

/// rand % m (HI) from a step's `jal` at `site` (rand's frame: s0 = `t`).
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// rand % 0x1000 - 0x800 (the record's signed `andi` remainder).
fn spin(fx: &mut Fx, t: u32, site: u32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    v % 0x1000 - 0x800
}

/// 0x1E01F30 (D, task; frame 0x20: ra, the entry's s0 = D): b521's white
/// facing ring (0x1E01790) with the radius +0x50 200..299.
fn ring_step(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    let f = RING;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x1C, 0x1E);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x64));
        let v = fx.rand_from(t, f + 0x50);
        fx.mem.set16(t + 0x56, v % 8);
        l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0xF0);
        let v = rmod(fx, t, f + 0x7C, 0x64);
        fx.mem.set16(t + 0x50, v + 0xC8);
        let v = rmod(fx, t, f + 0x9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB8, 0x14);
        fx.mem.set16(t + 0xE, v + 0x24);
        let v = rmod(fx, t, f + 0xD8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xF4, 0xA);
        fx.mem.set16(t + 6, v + 7);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x2A));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let age = fx.mem.i16(t + 0x54);
    if age < 0x5A && age != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    if fx.mem.i16(t + 0x54) < 0x8C {
        let s = ee::add(fx.mem.f(t + 0x48), 6.0);
        fx.mem.setf(t + 0x48, s);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// The first update shared by the quad steps (`sites`: the jal offsets of
/// the drift / angle / camera / spin / turn rands): drift +0x30 / +0x34
/// (-50..49) / 400, turn +0x5C -10..9, angle +0x5A, the camera's angles
/// in +0x20 (then overwritten by rand % 0xFFFF), spins +4 / +2 / +0.
fn quad_rest(fx: &mut Fx, t: u32, f: u32, s1: u64, sites: [u32; 10]) {
    let v = rmod(fx, t, f + sites[0], 100);
    fx.mem.setf(t + 0x34, ee::from_i32(v - 0x32));
    let v = rmod(fx, t, f + sites[1], 100);
    fx.mem.setf(t + 0x30, ee::from_i32(v - 0x32));
    let y = ee::div(fx.mem.f(t + 0x34), 400.0);
    fx.mem.setf(t + 0x34, y);
    let x = ee::div(fx.mem.f(t + 0x30), 400.0);
    fx.mem.setf(t + 0x30, x);
    let v = rmod(fx, t, f + sites[2], 0x14);
    fx.mem.set16(t + 0x5C, v - 0xA);
    let v = rmod(fx, t, f + sites[3], 0x168);
    fx.mem.set16(t + 0x5A, v);
    camera_angles_saving(fx, t + 0x20, u64::from(t), s1, f + sites[4] + 8);
    for (o, k) in [(4u32, 5usize), (2, 6), (0, 7)] {
        let v = spin(fx, t, f + sites[k]);
        fx.mem.set16(t + o, v);
    }
    for (o, site) in [(0x20u32, sites[8]), (0x24, sites[9]), (0x28, sites[9] + 0x1C)] {
        let v = rmod(fx, t, f + site, 0xFFFF);
        fx.mem.set32(t + o, v);
    }
}

/// The alpha: in by `step` under `top` for `n` updates, then out by `out`
/// down to `low` (then over).
fn fade(fx: &mut Fx, t: u32, n: i32, top: u8, step: u8, low: u8, out: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < n {
        if a < top {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(step));
        }
    } else if a >= low {
        fx.mem.write_u8(t + 0x63, a - out);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// From update 1: sinking by 2.6, sliding 2.4 along z (back when D+0xD884
/// is set).
fn sink(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) != 0 {
        let y = ee::sub(fx.mem.f(t + 0x14), f32::from_bits(0x4026_6666));
        fx.mem.setf(t + 0x14, y);
        let k = f32::from_bits(0x4019_999A);
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(SIDE) != 0 { ee::sub(z, k) } else { ee::add(z, k) };
        fx.mem.setf(t + 0x18, z);
    }
}

fn spin_on(fx: &mut Fx, t: u32) {
    for (o, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + o).wrapping_add(fx.mem.i16(t + s) as u32);
        fx.mem.write_u32(t + o, v);
    }
}

fn grow(fx: &mut Fx, t: u32) {
    let s = fx.mem.f(t + 0x48);
    if crate::exe::ee_lt(s, 150.0) {
        fx.mem.setf(t + 0x48, ee::add(s, 1.0));
    }
}

/// 0x1E02110 (D, task; frame 0x30: ra, the entry's s1 = 0, s0 = D): a grey
/// quad (0xC0) of size +0xA 80..119 (by 2 to 80 every third update), +0x48
/// 5..104 (to 150 by 1), cell row (rand >> 4) % 3, in by 0x60 under 0xC0
/// for 60 updates, then out by 15; the camera's angles every update.
fn grey_step(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    let f = GREY;
    let d = u64::from(D);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 0x28);
        fx.mem.set16(t + 0xA, v + 0x50);
        let w = fx.mem.i16(t + 0xA);
        fx.mem.set16(t + 8, w);
        let v = fx.rand_from(t, f + 0x54);
        let (_, h) = fx.div(v >> 4, 100);
        fx.mem.setf(t + 0x48, ee::add(5.0, ee::from_i32(h)));
        let v = fx.rand_from(t, f + 0x84);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        fx.mem.set16(t + 0x58, 2);
        quad_rest(fx, t, f, d, [0xB0, 0xDC, 0x128, 0x148, 0x164, 0x16C, 0x18C, 0x1AC, 0x1CC, 0x1E8]);
    }
    fade(fx, t, 0x3C, 0xC0, 0x60, 0x11, 0xF);
    let age = fx.mem.i16(t + 0x54);
    if fx.div(age, 3).1 == 1 {
        let w = fx.mem.i16(t + 0xA);
        if w < 0x50 {
            fx.mem.set16(t + 0xA, w + 2);
        }
        let w = fx.mem.i16(t + 0xA);
        fx.mem.set16(t + 8, w);
    }
    sink(fx, t);
    grow(fx, t);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    camera_angles_saving(fx, t + 0x20, u64::from(t), d, f + 0x370);
    fx.leave(0x30);
}

/// 0x1E024A0 / 0x1E02820 (D, task): white quads (0xFF) spinning by +4 /
/// +2 / +0, +8 = 2.5 +0xA; 0x24A0 (frame 0x30: ra, the entry's s1 = 0,
/// s0 = D): +0xA 2..7, in by 0x15 under 0x80 for 60 updates, sinking and
/// growing as 0x2110; 0x2820 (frame 0x20: ra, s0 = D): +0xA 10..17, in for
/// 10 updates, then out by 9.
fn quad_step(fx: &mut Fx, t: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    let white = f == WHITE;
    let frame = if white { 0x30 } else { 0x20 };
    let sp = fx.enter(frame);
    let d = u64::from(D);
    let s1 = if white { d } else { 0 };
    if white {
        fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, d)]);
    } else {
        fx.saved(sp, &[(0x10, TASK_RA), (0, d)]);
    }
    if fx.mem.i16(t + 0x58) == 1 {
        if white {
            let v = rmod(fx, t, f + 0x24, 6);
            fx.mem.set16(t + 0xA, v + 2);
        } else {
            let v = fx.rand_from(t, f + 0x24);
            fx.mem.set16(t + 0xA, v % 8 + 0xA);
        }
        let v = ee::mul(2.5, ee::from_i32(fx.mem.i16(t + 0xA)));
        fx.fix_residue(v, f + 0x60);
        fx.mem.set16(t + 8, ee::fixsfsi(v));
        let v = fx.rand_from(t, f + 0x68);
        let (_, h) = fx.div(v >> 4, 100);
        fx.mem.setf(t + 0x48, ee::add(5.0, ee::from_i32(h)));
        let v = fx.rand_from(t, f + 0x98);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        quad_rest(fx, t, f, s1, [0xC4, 0xF0, 0x13C, 0x15C, 0x178, 0x180, 0x1A0, 0x1C0, 0x1E0, 0x1FC]);
    }
    if white {
        fade(fx, t, 0x3C, 0x80, 0x15, 0xB, 9);
        sink(fx, t);
        spin_on(fx, t);
        grow(fx, t);
    } else {
        fade(fx, t, 0xA, 0x80, 0x15, 0xB, 9);
        spin_on(fx, t);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(frame);
}
