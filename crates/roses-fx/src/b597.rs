//! BMODULE b597 (card 227), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03540) [F]. b529's family
//! (card 47): 130 tasks of 0x70 at D+0x2A0 (alive +0x58, step +0x44 called
//! through `jalr`), spawned by 0x1E03E10 = `spawn::task70` on
//! 57042c7c1333's layout (colour D+0x9FF4, never set; point D+0x250; +0x50
//! from D+0xA0AA), the sines D+0xA2B4 and D+0xD8, timer D+0x9FF0 = 375, the
//! OT at D+0x80 and D+0x9FFC. Nodes: the attacker's 1 / 0x19 / 0x62 / 0x30
//! / 0x2F / 0x2C at D+0xA030.., the target's 1 at D+0xA040; textures (4,
//! 0xF) D+0x29C, (5, 0..3) D+0x9FD0..; the facing D+0xA0AE; jitter bytes
//! D+0xA0B4 (% 30 - 15) / D+0xA1B4 (% 40 - 20); 36 cells D+0xA050 (rand %
//! 8).
//!
//! The emitter 0x1E01640: update 1 the window tint 0x1E03040 (b529's frame
//! sprite, in while the timer is at 21 or more); from update 15 a petal
//! 0x1E01D70 a update at the attacker's node 1 (36 in all, kinds 0..35)
//! that rises, swings to the gather point D+0x3BB0 (set on update 30 at
//! node 0x19, 10 back along the facing, 7 up), circles it (update 0xDD..)
//! and flies to the target in a circle of radius clamp(target handle / 13,
//! 10, 60) from age 0xCD; 0x50..0x15E a grey ring 0x1E025B0 (b529's
//! ring_grey) at node 0x19; 15..0xC8 two white motes 0x1E02460 a update;
//! 0x118..0x154 two blue sparks 0x1E02750 a update circling the target's
//! node and a white ring 0x1E02EA0 at the target; on 0xAF two arcs 0x1E02A20
//! (b529's arc, red / white) and eight white rings 0x1E02CC0 at the point.
//! 0x1E028C0 / 0x1E03230 (and the draw 0x1E03FA0) are never spawned [F].
//! Returns 1 on update 270, 3 on 360, 6 on 281..354, 8 once the timer has
//! run out.
use crate::b494::camera_angles_saving;
use crate::b764::{cell, enter_step_with};
use crate::blib::spawn as bspawn;
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0x82;
const POOL: bspawn::Pool70 = bspawn::Pool70 { base: 0x2A0, n: 130, colour: 0x9FF4, pos: 0x250, slot: 0xA0AA };
const POS: u32 = D + 0x250;
const POINT: u32 = D + 0x3BB0;
const FLY: u32 = D + 0x9FC0;
const TEX: u32 = D + 0x9FD0;
const TIMER: u32 = D + 0x9FF0;
const OT_AT: u32 = D + 0x9FFC;
const RADIUS: u32 = D + 0xA01C;
const NODES: u32 = D + 0xA030;
const TARGET_NODE: u32 = D + 0xA040;
const CELLS: u32 = D + 0xA050;
const PETALS: u32 = D + 0xA09A;
const SPIN: u32 = D + 0xA0A4;
const KIND: u32 = D + 0xA0AA;
const FACING: u32 = D + 0xA0AE;
const PARAM: u32 = D + 0xA0B0;
const COUNTER: u32 = D + 0xA0B2;
const BYTES: u32 = D + 0xA0B4;
const BYTES_B: u32 = D + 0xA1B4;
const SINES: u32 = D + 0xA2B4;

const PETAL: u32 = 0x01E0_1D70;
const MOTE: u32 = 0x01E0_2460;
const RING_G: u32 = 0x01E0_25B0;
const SPARK: u32 = 0x01E0_2750;
const ARC: u32 = 0x01E0_2A20;
const RING_W: u32 = 0x01E0_2CC0;
const RING_T: u32 = 0x01E0_2EA0;
const FRAME: u32 = 0x01E0_3040;
/// The entry's `jalr` return address.
const TASK_RA: u64 = 0x01E0_38A0;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E03540.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xA420);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(PARAM, param);
    fx.mem.set32(TIMER, 0x177);
    for (k, (node, clear)) in [(1, 1), (0x19, 0), (0x62, 0), (0x30, 0), (0x2F, 0), (0x2C, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..4u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_3730);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3760, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_37B4, 0x28);
        fx.mem.set8(BYTES_B + k, v - 0x14);
    }
    for k in 0..0x24u32 {
        let v = fx.rand_from(k, 0x01E0_3804);
        fx.mem.set16(CELLS + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        // The steps' saves: the entry's s0 = 0, s1 = D, s2 = D + 0x70 k.
        match f {
            0 | 0x01E0_3230 => {}
            PETAL => petal(fx, t, D + 0x70 * k),
            MOTE => mote(fx, t),
            RING_G => ring_grey(fx, t),
            SPARK => spark(fx, t),
            ARC => arc(fx, t),
            RING_W => ring_white(fx, t),
            RING_T => ring_target(fx, t),
            FRAME => frame(fx, t),
            _ => panic!("b597: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        let pos = |fx: &mut Fx| {
            for c in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * c);
                fx.mem.write_u32(sp + 0x50 + 4 * c, w);
            }
        };
        if f(fx) == PETAL {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let frame = fx.mem.i16(t + 0x56);
            uv::grid_rect(fx, sp + 0x60, frame);
            for k in 0..4 {
                let w = fx.mem.read_u32(sp + 0x60 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, w);
            }
            let kind = fx.mem.i16(t + 0x52);
            let c = fx.mem.i16(CELLS.wrapping_add((2 * kind) as u32));
            fx.mem.set16(D + 0xBE, c);
            pos(fx);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
        }
        if f(fx) == MOTE {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x70, 0x14);
            fx.mem.set16(D + 0xBE, 5);
            pos(fx);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
        }
        if f(fx) == RING_G {
            ring_draw(fx, t, sp + 0x80, 3);
        }
        if f(fx) == SPARK {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x90, 0x14);
            let c = fx.mem.i16(t + 0x56);
            fx.mem.set16(D + 0x94, c << 5);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x20);
            fx.mem.set16(D + 0x9A, 0x3F);
            fx.mem.set16(D + 0xBE, 0);
            pos(fx);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
        }
        debug_assert_ne!(f(fx), 0x01E0_28C0);
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x18);
            for c in 0..4 {
                let w = fx.mem.read_u32(POINT + 4 * c);
                fx.mem.write_u32(sp + 0x50 + 4 * c, w);
            }
            let z = fx.mem.f(sp + 0x58);
            let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 1.5) } else { ee::sub(z, 1.5) };
            fx.mem.setf(sp + 0x58, z);
            billboard::arc_strips_flat(fx, D, t, 0x20, sp + 0x50);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == RING_W {
            ring_draw(fx, t, sp + 0xA0, 8);
        }
        if f(fx) == RING_T {
            ring_draw(fx, t, sp + 0xB0, 8);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x10E => 1,
        0x168 => 3,
        0x119..=0x162 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's ring draw: texture (5, 1), the cell 0x14 frame +0x56,
/// `rows` + 1 copies (D+0xB8), layout 0, `billboard::ring_quads`.
fn ring_draw(fx: &mut Fx, t: u32, local: u32, rows: i32) {
    let w = fx.mem.read_u32(TEX + 4);
    fx.mem.write_u32(D + 0x88, w);
    cell(fx, t, local, 0x14);
    fx.mem.set16(D + 0xB8, rows);
    fx.mem.set16(D + 0xB0, 0);
    billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
    fx.mem.set16(D + 0xB8, 0);
    fx.mem.set16(D + 0xB0, 0);
}

/// 0x1E03E10(D, step, 0) = `spawn::task70` (frame 0x60: the emitter's s0 /
/// s1 = D / s2, ra).
fn spawn(fx: &mut Fx, f: u32, s0: u64, s2: u64, ra: u32) {
    bspawn::task70(fx, D, POOL, f, 0, &[(0, s0), (0x10, u64::from(D)), (0x20, s2), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
}

/// The jittered point: x / z += rand % 60 - 30, y = 30 + rand % 20 - 10.
fn jitter(fx: &mut Fx, s0: u32, ras: [u32; 3]) {
    fx.mem.write_u32(POS + 4, 0x41F0_0000);
    for (o, ra, m, off) in [(0u32, ras[0], 0x3C, 0x1E), (8, ras[1], 0x3C, 0x1E), (4, ras[2], 0x14, 0xA)] {
        let v = rmod(fx, s0, ra, m) - off;
        let p = ee::add(fx.mem.f(POS + o), ee::from_i32(v));
        fx.mem.setf(POS + o, p);
    }
}

/// 0x1E01640(D, 0) (frame 0x40: ra, the entry's s2 = param, s1 = D, s0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_3860), (0x20, param as i64 as u64), (0x10, u64::from(D)), (0, 0)]);
    let p = param as i64 as u64;
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    if c == 1 {
        spawn(fx, FRAME, 0, p, 0x01E0_1698);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0x8D {
        fx.mem.set16(KIND, 0);
    }
    if fx.mem.i16(COUNTER) >= 0x8D {
        let v = fx.mem.f(FLY + 4);
        if v < 50.0 {
            fx.mem.setf(FLY + 4, ee::add(v, 1.5));
        }
        let v = fx.mem.f(FLY);
        if v < 80.0 {
            fx.mem.setf(FLY, ee::add(v, 1.0));
        }
    }
    if fx.mem.i16(COUNTER) >= 0xF {
        let n = fx.mem.i16(PETALS);
        if n < 0x24 {
            let h = fx.self_handle();
            let node = fx.mem.i16(NODES);
            fx.node_position(h, node, POS);
            let n = fx.mem.i16(PETALS);
            fx.mem.set16(KIND, n);
            jitter(fx, 0, [0x01E0_17C8, 0x01E0_17FC, 0x01E0_1830]);
            spawn(fx, PETAL, 0, p, 0x01E0_186C);
            let n = fx.mem.i16(PETALS) + 1;
            fx.mem.set16(PETALS, n);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x50..0x15F).contains(&c) {
        let h = fx.self_handle();
        let node = fx.mem.i16(NODES + 2);
        fx.node_position(h, node, POS);
        spawn(fx, RING_G, 0, p, 0x01E0_18E0);
    }
    let c = fx.mem.i16(COUNTER);
    if (0xF..0xC9).contains(&c) {
        for k in 0..2u64 {
            let h = fx.self_handle();
            let node = fx.mem.i16(NODES);
            fx.node_position(h, node, POS);
            jitter(fx, k as u32, [0x01E0_1938, 0x01E0_196C, 0x01E0_19A0]);
            spawn(fx, MOTE, k, p, 0x01E0_19DC);
        }
    }
    if fx.mem.i16(COUNTER) == 0x1E {
        let h = fx.self_handle();
        let node = fx.mem.i16(NODES + 2);
        fx.node_position(h, node, POINT);
        let z = fx.mem.f(POINT + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 10.0) } else { ee::add(z, 10.0) };
        fx.mem.setf(POINT + 8, z);
        let y = ee::add(fx.mem.f(POINT + 4), 7.0);
        fx.mem.setf(POINT + 4, y);
    }
    let s = fx.mem.i16(SPIN) + 5;
    fx.mem.set16(SPIN, s);
    let c = fx.mem.i16(COUNTER);
    if (0x118..0x155).contains(&c) {
        for k in 0..2i32 {
            let h = fx.target_handle();
            let node = fx.mem.i16(NODES);
            fx.node_position(h, node, POS);
            fx.mem.write_u32(POS + 4, 0x4220_0000);
            let a = fx.mem.i16(SPIN) + if k == 1 { 0xB4 } else { 0 };
            let a = a as i16 as i32;
            let cos = l2::sin_deg(fx, SINES, a, 1);
            let x = ee::add(fx.mem.f(POS), ee::mul(10.0, cos));
            fx.mem.setf(POS, x);
            let sin = l2::sin_deg(fx, SINES, a, 0);
            let z = ee::add(fx.mem.f(POS + 8), ee::mul(10.0, sin));
            fx.mem.setf(POS + 8, z);
            spawn(fx, SPARK, k as u64, k as u64, 0x01E0_1C14);
        }
        let h = fx.target_handle();
        let node = fx.mem.i16(TARGET_NODE);
        fx.node_position(h, node, POS);
        let v = rmod(fx, 2, 0x01E0_1C5C, 5);
        let y = ee::add(fx.mem.f(POS + 4), ee::from_i32(v));
        fx.mem.setf(POS + 4, y);
        spawn(fx, RING_T, 2, 2, 0x01E0_1C94);
    }
    if fx.mem.i16(COUNTER) == 0xAF {
        for kind in 1..3 {
            fx.mem.set16(KIND, kind);
            // s0: 2 after the motes' loop (0xF..0xC8 runs on 0xAF).
            spawn(fx, ARC, 2, p, if kind == 1 { 0x01E0_1CD0 } else { 0x01E0_1CF4 });
        }
        for c in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * c);
            fx.mem.write_u32(POS + 4 * c, w);
        }
        for k in 0..8u64 {
            spawn(fx, RING_W, k, p, 0x01E0_1D30);
        }
    }
    fx.leave(0x40);
}

fn enter_step(fx: &mut Fx, frame: u32, n: u32) -> u32 {
    enter_step_with(fx, frame, n, [0, u64::from(D)], TASK_RA)
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn odd_age(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54).wrapping_rem(2) == 1
}

/// +a under `cap` up by `by`.
fn up(fx: &mut Fx, t: u32, a: u32, cap: u8, by: u8) {
    let v = fx.mem.read_u8(t + a);
    if v < cap {
        fx.mem.write_u8(t + a, v.wrapping_add(by));
    }
}

/// +a at `floor` or more down by `by`, else the task ends.
fn down(fx: &mut Fx, t: u32, a: u32, floor: u8, by: u8) {
    let v = fx.mem.read_u8(t + a);
    if v >= floor {
        fx.mem.write_u8(t + a, v - by);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The alpha +0x63 down by `by` again (no end) while at `floor` or more.
fn down_more(fx: &mut Fx, t: u32, floor: u8, by: u8) {
    let v = fx.mem.read_u8(t + 0x63);
    if v >= floor {
        fx.mem.write_u8(t + 0x63, v - by);
    }
}

fn turn(fx: &mut Fx, t: u32, ras: [u32; 3]) {
    for (o, ra) in [0x20u32, 0x24, 0x28].into_iter().zip(ras) {
        let v = rmod(fx, t, ra, 0xFFFF);
        fx.mem.set32(t + o, v);
    }
}

fn hold(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        down(fx, t, 0x63, 0xD, 0xB);
    }
}

fn add_vel(fx: &mut Fx, t: u32) {
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(t + 0x10 + 4 * k), fx.mem.f(t + 0x30 + 4 * k));
        fx.mem.setf(t + 0x10 + 4 * k, v);
    }
}

/// 0x1E01D70 (D, task; frame 0x60: ra, the entry's s2 / s1 = D / s0 = 0):
/// a petal (10 x 10, cell (rand >> 4) % 8, white, rising by 1.2 for its
/// updates 11..34; 78..107 a 30th of the way to the point + 15 (cos, sin)
/// of 10 kind degrees; circling the point at 15 while the counter is
/// 0xDD..0x167 (up to age 0xCC); from age 0xCD (the radius clamp(target
/// handle / 13, 10, 60)) a 30th of the way to the target's node + the
/// circle a update for 30 updates, then on the circle (turning by 10 by
/// the facing, the height + +0x48 sin(+0x5A + 4 spin), +0x48 up by 0.5 to
/// 8)); white 0xFF / 0xC0 by its age's parity; in by 10 under 0x80 until
/// update 350, then out by 4 (twice).
fn petal(fx: &mut Fx, t: u32, s2: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x30, TASK_RA), (0x20, u64::from(s2)), (0x10, u64::from(D)), (0, 0)]);
    let mut s0 = 0u64;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xA);
        fx.mem.set16(t + 0xA, 0xA);
        fx.mem.write_u32(t + 0x48, 0);
        let r = fx.rand_from(0, 0x01E0_1DAC);
        fx.mem.set16(t + 0x56, (r >> 4) % 8);
        fx.mem.write_u32(t + 0x34, 0x3F99_999A);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x52, k);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x5C, 0);
    }
    if fx.mem.i16(COUNTER) < 0x15E {
        up(fx, t, 0x63, 0x80, 0xA);
        up(fx, t, 0x67, 0x80, 0xA);
    } else {
        down(fx, t, 0x63, 6, 4);
        down_more(fx, t, 6, 4);
    }
    if fx.mem.i16(t + 0x54) % 2 == 0 {
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
    } else {
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    }
    let a = fx.mem.i16(t + 0x54);
    if (0xB..0x23).contains(&a) {
        let v = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
        fx.mem.setf(t + 0x14, v);
    } else if (0x4E..0x6C).contains(&a) {
        if a == 0x4E {
            s0 = u64::from(sp + 0x50);
            for c in 0..4 {
                let w = fx.mem.read_u32(POINT + 4 * c);
                fx.mem.write_u32(sp + 0x50 + 4 * c, w);
            }
            let deg = (fx.mem.i16(t + 0x50) * 10) as i16 as i32;
            let c = ee::mul(15.0, l2::sin_deg(fx, SINES, deg, 1));
            fx.mem.setf(sp + 0x40, c);
            let s = ee::mul(15.0, l2::sin_deg(fx, SINES, deg, 0));
            fx.mem.setf(sp + 0x44, s);
            let f = |fx: &mut Fx, o: u32| fx.mem.f(sp + o);
            let vx = ee::div(ee::sub(ee::add(f(fx, 0x50), f(fx, 0x40)), fx.mem.f(t + 0x10)), 30.0);
            fx.mem.setf(t + 0x30, vx);
            let vy = ee::div(ee::sub(ee::add(f(fx, 0x54), f(fx, 0x44)), fx.mem.f(t + 0x14)), 30.0);
            fx.mem.setf(t + 0x34, vy);
            let vz = ee::div(ee::sub(f(fx, 0x58), fx.mem.f(t + 0x18)), 30.0);
            fx.mem.setf(t + 0x38, vz);
            fx.mem.set16(t + 0x5A, deg);
        }
        add_vel(fx, t);
    }
    if fx.mem.i16(t + 0x54) < 0xCD {
        let c = fx.mem.i16(COUNTER);
        if (0xDD..0x168).contains(&c) {
            let h = fx.self_handle();
            let n = fx.mem.i16(NODES + 2);
            fx.node_position(h, n, sp + 0x50);
            let a = fx.mem.i16(t + 0x5A);
            let c = ee::mul(15.0, l2::sin_deg(fx, SINES, a, 1));
            fx.mem.setf(sp + 0x40, c);
            let a = fx.mem.i16(t + 0x5A);
            let s = ee::mul(15.0, l2::sin_deg(fx, SINES, a, 0));
            fx.mem.setf(sp + 0x44, s);
            let x = ee::add(fx.mem.f(POINT), fx.mem.f(sp + 0x40));
            fx.mem.setf(t + 0x10, x);
            let y = ee::add(fx.mem.f(POINT + 4), fx.mem.f(sp + 0x44));
            fx.mem.setf(t + 0x14, y);
            let a = fx.mem.i16(t + 0x5A) + 2;
            fx.mem.set16(t + 0x5A, a);
        }
    } else {
        let circle = |fx: &mut Fx| {
            let h = fx.target_handle();
            let n = fx.mem.i16(TARGET_NODE);
            fx.node_position(h, n, sp + 0x50);
            let a = fx.mem.i16(t + 0x5A);
            let c = ee::mul(fx.mem.f(RADIUS), l2::sin_deg(fx, SINES, a, 1));
            fx.mem.setf(sp + 0x40, c);
            let a = fx.mem.i16(t + 0x5A);
            let s = ee::mul(fx.mem.f(RADIUS), l2::sin_deg(fx, SINES, a, 0));
            fx.mem.setf(sp + 0x48, s);
        };
        if fx.mem.i16(t + 0x54) == 0xCD {
            let h = fx.target_handle();
            let r = ee::div(ee::from_i32(h), 13.0);
            fx.mem.setf(RADIUS, r);
            if !(r <= 60.0) {
                fx.mem.setf(RADIUS, 60.0);
            }
            if fx.mem.f(RADIUS) < 10.0 {
                fx.mem.setf(RADIUS, 10.0);
            }
            fx.mem.set16(t + 0x5A, 0);
            s0 = u64::from(sp + 0x50);
            circle(fx);
            let f = |fx: &mut Fx, o: u32| fx.mem.f(sp + o);
            let vx = ee::div(ee::sub(ee::add(f(fx, 0x50), f(fx, 0x40)), fx.mem.f(t + 0x10)), 30.0);
            fx.mem.setf(t + 0x30, vx);
            let vy = ee::div(ee::sub(f(fx, 0x54), fx.mem.f(t + 0x14)), 30.0);
            fx.mem.setf(t + 0x34, vy);
            let vz = ee::div(ee::sub(ee::add(f(fx, 0x58), f(fx, 0x48)), fx.mem.f(t + 0x18)), 30.0);
            fx.mem.setf(t + 0x38, vz);
        }
        if fx.mem.i16(t + 0x54) < 0xEB {
            add_vel(fx, t);
        } else {
            s0 = u64::from(sp + 0x50);
            circle(fx);
            let x = ee::add(fx.mem.f(sp + 0x50), fx.mem.f(sp + 0x40));
            fx.mem.setf(t + 0x10, x);
            let z = ee::add(fx.mem.f(sp + 0x58), fx.mem.f(sp + 0x48));
            fx.mem.setf(t + 0x18, z);
            let a = (fx.mem.i16(t + 0x5A) + 4 * fx.mem.i16(SPIN)) as i16 as i32;
            let s = l2::sin_deg(fx, SINES, a, 0);
            let y = ee::add(fx.mem.f(sp + 0x54), ee::mul(fx.mem.f(t + 0x48), s));
            fx.mem.setf(t + 0x14, y);
            let a = fx.mem.i16(t + 0x5A);
            let a = if fx.mem.i16(FACING) != 0 { a - 0xA } else { a + 0xA };
            fx.mem.set16(t + 0x5A, a);
            let w = fx.mem.f(t + 0x48);
            if w < 8.0 {
                fx.mem.setf(t + 0x48, ee::add(w, 0.5));
            }
        }
    }
    camera_angles_saving(fx, t + 0x20, s0, u64::from(t), 0x01E0_2438);
    age(fx, t);
    fx.leave(0x60);
}

/// 0x1E02460 (D, task; frame 0x20): a white mote (10 x 6, cell rand % 8)
/// rising by 2.5, in by 0x60 under 0xC0 for 10 updates, then out by 7.
fn mote(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xA);
        fx.mem.set16(t + 0xA, 6);
        fx.mem.write_u32(t + 0x48, 0);
        let r = fx.rand_from(t, 0x01E0_2494);
        fx.mem.set16(t + 0x56, r % 8);
        fx.mem.write_u32(t + 0x34, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x5C, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        up(fx, t, 0x63, 0xC0, 0x60);
        up(fx, t, 0x67, 0xC0, 0x60);
    } else {
        down(fx, t, 0x63, 9, 7);
        down_more(fx, t, 9, 7);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
        fx.mem.setf(t + 0x14, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2590);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E025B0 (D, task; frame 0x20): b529's grey ring (0xC0, size 1..2,
/// cell 4, turned at random, radius 10..89, +8 1..8, at 0x80 for 3..6
/// updates then out by 11).
fn ring_grey(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_25D4);
        fx.mem.setf(t + 0x48, ee::from_i32(v % 2 + 1));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        turn(fx, t, [0x01E0_261C, 0x01E0_2638, 0x01E0_2654]);
        let v = rmod(fx, t, 0x01E0_2670, 0x50);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_2690, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_26AC);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = fx.rand_from(t, 0x01E0_26CC);
        fx.mem.set16(t + 6, v % 4 + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02750 (D, task; frame 0x20): a blue spark (30 x 60 growing by 2,
/// cell rand % 8, (0x20, 0x20, 0x80) twice) rising by 0.5, in by 0x40
/// under 0x80 for 20 updates, then out by 6.
fn spark(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x1E);
        fx.mem.set16(t + 0xA, 0x3C);
        fx.mem.write_u32(t + 0x48, 0);
        let r = fx.rand_from(t, 0x01E0_2784);
        fx.mem.set16(t + 0x56, r % 8);
        fx.mem.write_u32(t + 0x34, 0x3F00_0000);
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x80);
        l2::rgb(fx, t + 0x64, 0x20, 0x20, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x5C, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        up(fx, t, 0x63, 0x80, 0x40);
        up(fx, t, 0x67, 0x80, 0x40);
    } else {
        down(fx, t, 0x63, 8, 6);
        down_more(fx, t, 8, 6);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let v = fx.mem.i16(t + o) + 2;
            fx.mem.set16(t + o, v);
        }
        let v = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
        fx.mem.setf(t + 0x14, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2898);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02A20 (D, task; frame 0x20): b529's arc by its kind +0x50: 0 radius
/// 30 white over (0x80, 0x80, 0xFF) for 300 updates, flickering grey 0x8F
/// / 0x70; 1 radius 350 white over (0xFF, 0xC0, 0xC0) for 12; 2 radius 160
/// (0xFF, 0x80, 0x80) over white for 12, the outer in by 0x40; then out by
/// 11 (2: the outer by 15); turned by the camera, +0x20 0x4000, +0x24 0.
fn arc(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        let kind = fx.mem.i16(t + 0x50);
        let cfg = match kind {
            0 => Some((0x41F0_0000u32, 0x12C, (0x60u32, 0xFF, 0xFF, 0xFF), (0x64u32, 0x80, 0x80, 0xFF))),
            1 => Some((0x43AF_0000, 0xC, (0x60, 0xFF, 0xFF, 0xFF), (0x64, 0xFF, 0xC0, 0xC0))),
            2 => Some((0x4320_0000, 0xC, (0x64, 0xFF, 0xFF, 0xFF), (0x60, 0xFF, 0x80, 0x80))),
            _ => None,
        };
        if let Some((r, life, a, b)) = cfg {
            fx.mem.write_u32(t + 0x48, r);
            fx.mem.write_u32(t + 0x4C, 0);
            fx.mem.set16(t + 6, life);
            l2::rgb(fx, t + a.0, a.1, a.2, a.3);
            l2::rgb(fx, t + b.0, b.1, b.2, b.3);
        }
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let kind = fx.mem.i16(t + 0x50);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        match kind {
            0 => up(fx, t, 0x63, 0xC0, 0xE),
            1 => up(fx, t, 0x63, 0xC0, 0x40),
            2 => up(fx, t, 0x67, 0xC0, 0x40),
            _ => {}
        }
    } else {
        match kind {
            0 | 1 => down(fx, t, 0x63, 0xD, 0xB),
            2 => down(fx, t, 0x67, 0x11, 0xF),
            _ => {}
        }
    }
    if fx.mem.i16(t + 0x50) == 0 {
        let g = if odd_age(fx, t) { 0x8F } else { 0x70 };
        l2::rgb(fx, t + 0x60, g, g, g);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2C94);
    fx.mem.write_u32(t + 0x20, 0x4000);
    fx.mem.write_u32(t + 0x24, 0);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02CC0 (D, task; frame 0x20): b640's white ring (size 8..22, cell 4,
/// turned at random, +0x52 counting from 1, radius 20..49 growing by +0xE
/// = 15..34, +8 1..8, at 0x80 for 5..14 updates then out by 11).
fn ring_white(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2CE4, 0xF);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 8));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        turn(fx, t, [0x01E0_2D28, 0x01E0_2D44, 0x01E0_2D60]);
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_2D84, 0x1E);
        fx.mem.set16(t + 0x50, v + 0x14);
        let v = rmod(fx, t, 0x01E0_2DA4, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2DC0, 0x14);
        fx.mem.set16(t + 0xE, v + 0xF);
        let v = fx.rand_from(t, 0x01E0_2DE0);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_2E00, 0xA);
        fx.mem.set16(t + 6, v + 5);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t);
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    let v = fx.mem.i16(t + 0x50).wrapping_add(fx.mem.i16(t + 0xE));
    fx.mem.set16(t + 0x50, v);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02EA0 (D, task; frame 0x20): a white ring at the target (size 5..11,
/// cell 4, turned at random, radius 60..259, +8 1..8, at 0x80 for 3..7
/// updates then out by 11).
fn ring_target(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2EC4, 7);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 5));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        turn(fx, t, [0x01E0_2F08, 0x01E0_2F24, 0x01E0_2F40]);
        let v = rmod(fx, t, 0x01E0_2F5C, 0xC8);
        fx.mem.set16(t + 0x50, v + 0x3C);
        let v = rmod(fx, t, 0x01E0_2F7C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_2F98);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_2FB8, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03040 (D, task; frame 0x90): b529's window sprite (0x6FF8 ..
/// 0x8FF8) sampling the frame (0x207EF0, filter 1) at ST inset by +0x48
/// (1.5) / 512, colour (0x78, 0x78, 0x90, alpha +0x63), ALPHA 0x44, key 0;
/// in by 8 below 0x50 while the timer is at 21 or more, then out by 7.
fn frame(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3FC0_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for (k, c) in [0x78u8, 0x78, 0x90].into_iter().enumerate() {
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
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x50, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(512.0, s), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(512.0, s), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i32(TIMER) >= 0x15 {
        up(fx, t, 0x63, 0x50, 8);
    } else {
        down(fx, t, 0x63, 9, 7);
    }
    age(fx, t);
    fx.leave(0x90);
}
