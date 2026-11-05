//! BMODULE b152 (card 114), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E036F0) [F]. b597's sibling
//! (card 227, b597.rs): 130 tasks of 0x70 at D+0x2A0 (alive +0x58, step +0x44
//! called through `jalr`), spawned by 0x1E03F90 = `spawn::task70` on b597's
//! layout; D 0xA420, the sines D+0xA2B4 / D+0xD8, timer D+0x9FF0 = 0x1C8,
//! the OT at D+0x80 and D+0x9FFC. Nodes: the attacker's 1 / 0x63 / 0x62 /
//! 0x30 / 0x2F / 0x2C at D+0xA030.., the target's 1 at D+0xA040; textures
//! (4, 0xF) D+0x29C, (5, 0..3) D+0x9FD0..; the facing D+0xA0AE; jitter bytes
//! D+0xA0B4 / D+0xA1B4; 36 cells D+0xA050.
//!
//! The emitter 0x1E01640: update 1 the window tint 0x1E031F0 (b597's frame
//! in (0x90, 0x78, 0x78), in while the counter is under 0x1A4); on 0x24 the
//! stage light 0x1E03110 (b640's, 250 updates); 0x51..0x9F a flash task
//! 0x1E033E0 a update (never run through `jalr`: counted, and while any is
//! alive the entry calls the unit flash 0x1E044F0, which steps them); from
//! 0x53 a petal 0x1E01E60 a update (b597's petal on later ages, 36 in all);
//! 0x50..0x15E a grey ring 0x1E02680 (b529's); 0x50..0xC8 two white motes
//! 0x1E02530 (b597's) a update; on 0xB7 the gather point D+0x3BB0; 0x172..
//! 0x1A9 two grey sparks 0x1E02820 (b597's in 0x20) and a white ring
//! 0x1E02F70 (b597's) at the target; on the odd updates 0x10B..0x119 a bloom
//! 0x1E02990 (drawn by 0x1E04120 at the point); on 0x122 two arcs 0x1E02AF0
//! (b529's: white / (0x40, 0x10, 0x10) over 0x30) and eight white rings
//! 0x1E02D90 (b640's) at the point. Returns 1 on update 0x163, 3 on 0x1C2,
//! 6 on 0x16E..0x1BC, 8 once the timer has run out.
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
const LAYER: u32 = D + 0xA004;
const FLASH_DESC: u32 = D + 0xA008;
const FLASH_DESC2: u32 = D + 0xA00C;
const FLASH_W: u32 = D + 0xA018;
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

const PETAL: u32 = 0x01E0_1E60;
const MOTE: u32 = 0x01E0_2530;
const RING_G: u32 = 0x01E0_2680;
const SPARK: u32 = 0x01E0_2820;
const BLOOM: u32 = 0x01E0_2990;
const ARC: u32 = 0x01E0_2AF0;
const RING_W: u32 = 0x01E0_2D90;
const RING_T: u32 = 0x01E0_2F70;
const LIGHT: u32 = 0x01E0_3110;
const FRAME: u32 = 0x01E0_31F0;
const FLASH: u32 = 0x01E0_33E0;
/// The entry's `jalr` return address.
const TASK_RA: u64 = 0x01E0_3A58;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E036F0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x170);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x170);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xA420);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(PARAM, param);
    fx.mem.set32(TIMER, 0x1C8);
    for (k, (node, clear)) in [(1, 1), (0x63, 0), (0x62, 0), (0x30, 0), (0x2F, 0), (0x2C, 0)].into_iter().enumerate() {
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
    fx.unit_rotation(h, sp + 0xD0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.fix_residue(y, 0x01E0_38E8);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3918, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_396C, 0x28);
        fx.mem.set8(BYTES_B + k, v - 0x14);
    }
    for k in 0..0x24u32 {
        let v = fx.rand_from(k, 0x01E0_39BC);
        fx.mem.set16(CELLS + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s0: the flash tasks counted so far (s1 = 0, s2 = D).
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        let s0 = flashes;
        match f {
            0 => {}
            FLASH => flashes += 1,
            PETAL => petal(fx, t, s0),
            MOTE => mote(fx, t, s0),
            RING_G => ring_grey(fx, t, s0),
            SPARK => spark(fx, t, s0),
            BLOOM => bloom(fx, t, s0),
            ARC => arc(fx, t, s0),
            RING_W => ring_white(fx, t, s0),
            RING_T => ring_target(fx, t, s0),
            LIGHT => light(fx, t, s0),
            FRAME => frame(fx, t, s0),
            _ => panic!("b152: unknown task step {f:#x}"),
        }
    }
    // The entry's s1: the live tasks counted in the draw loop.
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        let pos = |fx: &mut Fx| {
            for c in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * c);
                fx.mem.write_u32(sp + 0x60 + 4 * c, w);
            }
        };
        if f(fx) == PETAL {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let frame = fx.mem.i16(t + 0x56);
            uv::grid_rect(fx, sp + 0x70, frame);
            for k in 0..4 {
                let w = fx.mem.read_u32(sp + 0x70 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, w);
            }
            let kind = fx.mem.i16(t + 0x52);
            let c = fx.mem.i16(CELLS.wrapping_add((2 * kind) as u32));
            fx.mem.set16(D + 0xBE, c);
            pos(fx);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
        }
        if f(fx) == MOTE {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x80, 0x14);
            fx.mem.set16(D + 0xBE, 5);
            pos(fx);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
        }
        if f(fx) == RING_G {
            ring_draw(fx, t, sp + 0x90, 3);
        }
        if f(fx) == SPARK {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0xA0, 0x14);
            let c = fx.mem.i16(t + 0x56);
            fx.mem.set16(D + 0x94, c << 5);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x20);
            fx.mem.set16(D + 0x9A, 0x3F);
            fx.mem.set16(D + 0xBE, 0);
            pos(fx);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
        }
        if f(fx) == BLOOM {
            for c in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * c);
                fx.mem.write_u32(D + 0x20 + 4 * c, w);
            }
            bloom_draw(fx, t, flashes, live);
        }
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x18);
            billboard::arc_strips_flat(fx, D, t, 0x21, POINT);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == RING_W {
            ring_draw(fx, t, sp + 0xB0, 8);
        }
        if f(fx) == RING_T {
            ring_draw(fx, t, sp + 0xC0, 8);
        }
    }
    if flashes != 0 {
        unit_flash(fx, flashes, live);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x163 => 1,
        0x1C2 => 3,
        0x16E..=0x1BC => 6,
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

/// 0x1E03F90(D, step, 0) = `spawn::task70` (frame 0x60: the emitter's s0 /
/// s1 = D / s2, the entry's s3 = param / s4, ra).
fn spawn(fx: &mut Fx, f: u32, s0: u64, s2: u64, param: i32, ra: u32) {
    let saves = [(0, s0), (0x10, u64::from(D)), (0x20, s2), (0x30, param as i64 as u64), (0x40, 0), (0x50, u64::from(ra))];
    bspawn::task70(fx, D, POOL, f, 0, &saves);
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

/// 0x1E01640(D, 0) (frame 0x40: ra, the entry's s2 = D, s1 = 0, s0 = 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_3A18), (0x20, u64::from(D)), (0x10, 0), (0, 0)]);
    let d = u64::from(D);
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    if c == 1 {
        spawn(fx, FRAME, 0, d, param, 0x01E0_1698);
    }
    if fx.mem.i16(COUNTER) == 0x24 {
        spawn(fx, LIGHT, 0, d, param, 0x01E0_16C4);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x51..0xA0).contains(&c) {
        spawn(fx, FLASH, 0, d, param, 0x01E0_16FC);
    }
    if fx.mem.i16(COUNTER) == 0x8D {
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
    if fx.mem.i16(COUNTER) >= 0x53 {
        let n = fx.mem.i16(PETALS);
        if n < 0x24 {
            let h = fx.self_handle();
            let node = fx.mem.i16(NODES);
            fx.node_position(h, node, POS);
            let n = fx.mem.i16(PETALS);
            fx.mem.set16(KIND, n);
            jitter(fx, 0, [0x01E0_182C, 0x01E0_1860, 0x01E0_1894]);
            spawn(fx, PETAL, 0, d, param, 0x01E0_18D0);
            let n = fx.mem.i16(PETALS) + 1;
            fx.mem.set16(PETALS, n);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x50..0x15F).contains(&c) {
        let h = fx.self_handle();
        let node = fx.mem.i16(NODES + 2);
        fx.node_position(h, node, POS);
        spawn(fx, RING_G, 0, d, param, 0x01E0_1944);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x50..0xC9).contains(&c) {
        for k in 0..2u32 {
            let h = fx.self_handle();
            let node = fx.mem.i16(NODES);
            fx.node_position(h, node, POS);
            jitter(fx, k, [0x01E0_199C, 0x01E0_19D0, 0x01E0_1A04]);
            spawn(fx, MOTE, u64::from(k), d, param, 0x01E0_1A40);
        }
    }
    if fx.mem.i16(COUNTER) == 0xB7 {
        let h = fx.self_handle();
        let node = fx.mem.i16(NODES + 2);
        fx.node_position(h, node, POINT);
        let z = fx.mem.f(POINT + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 10.0) } else { ee::add(z, 10.0) };
        fx.mem.setf(POINT + 8, z);
        let y = ee::add(fx.mem.f(POINT + 4), 8.0);
        fx.mem.setf(POINT + 4, y);
    }
    let s = fx.mem.i16(SPIN) + 5;
    fx.mem.set16(SPIN, s);
    let c = fx.mem.i16(COUNTER);
    if (0x172..0x1AA).contains(&c) {
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
            spawn(fx, SPARK, k as u64, k as u64, param, 0x01E0_1C7C);
        }
        let h = fx.target_handle();
        let node = fx.mem.i16(TARGET_NODE);
        fx.node_position(h, node, POS);
        let v = rmod(fx, 2, 0x01E0_1CC4, 5);
        let y = ee::add(fx.mem.f(POS + 4), ee::from_i32(v));
        fx.mem.setf(POS + 4, y);
        spawn(fx, RING_T, 2, 2, param, 0x01E0_1CFC);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x10B..=0x119).contains(&c) && c % 2 == 1 {
        spawn(fx, BLOOM, 0, d, param, 0x01E0_1D7C);
    }
    if fx.mem.i16(COUNTER) == 0x122 {
        for kind in 1..3 {
            fx.mem.set16(KIND, kind);
            spawn(fx, ARC, 0, d, param, if kind == 1 { 0x01E0_1DB8 } else { 0x01E0_1DDC });
        }
        for c in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * c);
            fx.mem.write_u32(POS + 4 * c, w);
        }
        for k in 0..8u64 {
            spawn(fx, RING_W, k, d, param, 0x01E0_1E18);
        }
    }
    fx.leave(0x40);
}

/// A step's frame: the entry's s0 (the flash count so far) and s1 (0).
fn enter_step(fx: &mut Fx, frame: u32, n: u32, s0: u32) -> u32 {
    enter_step_with(fx, frame, n, [u64::from(s0), 0], TASK_RA)
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

/// 0x1E01E60 (D, task; frame 0x60: ra, the entry's s2 = D / s1 = 0 / s0):
/// b597's petal on later ages: rising by 1.2 for ages 11..44; 0x67..0x84 a
/// 30th of the way to the point + 15 (cos, sin) of 10 kind degrees;
/// circling the point at 15 while the counter is 0x100..0x167 (up to age
/// 0xEF); from age 0xF0 to the target's circle (30 updates from 0x10E on
/// it, turning by -10, the height + +0x48 sin(+0x5A + 4 spin), +0x48 up by
/// 0.5 to 8); in by 10 under 0x80 until update 0x1AE, then out by 4.
fn petal(fx: &mut Fx, t: u32, entry_s0: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x30, TASK_RA), (0x20, u64::from(D)), (0x10, 0), (0, u64::from(entry_s0))]);
    let mut s0 = u64::from(entry_s0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xA);
        fx.mem.set16(t + 0xA, 0xA);
        fx.mem.write_u32(t + 0x48, 0);
        let r = fx.rand_from(entry_s0, 0x01E0_1E9C);
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
    if fx.mem.i16(COUNTER) < 0x1AE {
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
    if (0xB..0x2D).contains(&a) {
        let v = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
        fx.mem.setf(t + 0x14, v);
    } else if (0x67..0x85).contains(&a) {
        if a == 0x67 {
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
    if fx.mem.i16(t + 0x54) < 0xF0 {
        let c = fx.mem.i16(COUNTER);
        if (0x100..0x168).contains(&c) {
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
        if fx.mem.i16(t + 0x54) == 0xF0 {
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
        if fx.mem.i16(t + 0x54) < 0x10E {
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
            let a = fx.mem.i16(t + 0x5A) - 0xA;
            fx.mem.set16(t + 0x5A, a);
            let w = fx.mem.f(t + 0x48);
            if w < 8.0 {
                fx.mem.setf(t + 0x48, ee::add(w, 0.5));
            }
        }
    }
    camera_angles_saving(fx, t + 0x20, s0, u64::from(t), 0x01E0_2504);
    age(fx, t);
    fx.leave(0x60);
}

/// 0x1E02530 (D, task; frame 0x20): b597's white mote (10 x 6, cell rand %
/// 8) rising by 2.5, in by 0x60 under 0xC0 for 10 updates, then out by 7.
fn mote(fx: &mut Fx, t: u32, s0: u32) {
    enter_step(fx, 0x20, 1, s0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xA);
        fx.mem.set16(t + 0xA, 6);
        fx.mem.write_u32(t + 0x48, 0);
        let r = fx.rand_from(t, 0x01E0_2564);
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
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2660);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02680 (D, task; frame 0x20): b529's grey ring (0xC0, size 1..2,
/// cell 4, turned at random, radius 10..89, +8 1..8, at 0x80 for 3..6
/// updates then out by 11).
fn ring_grey(fx: &mut Fx, t: u32, s0: u32) {
    enter_step(fx, 0x20, 1, s0);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_26A4);
        fx.mem.setf(t + 0x48, ee::from_i32(v % 2 + 1));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        turn(fx, t, [0x01E0_26EC, 0x01E0_2708, 0x01E0_2724]);
        let v = rmod(fx, t, 0x01E0_2740, 0x50);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_2760, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_277C);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = fx.rand_from(t, 0x01E0_279C);
        fx.mem.set16(t + 6, v % 4 + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02820 (D, task; frame 0x20): b597's spark in grey 0x20 (30 x 60
/// growing by 2, cell rand % 8) rising by 0.5, in by 0x40 under 0x80 for 20
/// updates, then out by 6.
fn spark(fx: &mut Fx, t: u32, s0: u32) {
    enter_step(fx, 0x20, 1, s0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x1E);
        fx.mem.set16(t + 0xA, 0x3C);
        fx.mem.write_u32(t + 0x48, 0);
        let r = fx.rand_from(t, 0x01E0_2854);
        fx.mem.set16(t + 0x56, r % 8);
        fx.mem.write_u32(t + 0x34, 0x3F00_0000);
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x20);
        l2::rgb(fx, t + 0x64, 0x20, 0x20, 0x20);
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
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2968);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02990 (D, task; frame 0x20): a bloom at the point (+0x48 from 0
/// growing by 5 under 90; grey 0x80 / 0x60 by its age's parity), in by 0xE
/// under 0x90 for 50 updates, then out by 0xD (+0x48 up by 0.5 more).
fn bloom(fx: &mut Fx, t: u32, s0: u32) {
    enter_step(fx, 0x20, 1, s0);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x10, 0x10, 0x10);
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.set16(t + 0xE, 1);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x32 {
        up(fx, t, 0x63, 0x90, 0xE);
    } else {
        down(fx, t, 0x63, 0xF, 0xD);
        let v = ee::add(fx.mem.f(t + 0x48), 0.5);
        fx.mem.setf(t + 0x48, v);
    }
    let g = if odd_age(fx, t) { 0x80 } else { 0x60 };
    l2::rgb(fx, t + 0x60, g, g, g);
    let v = fx.mem.f(t + 0x48);
    if v < 90.0 {
        fx.mem.setf(t + 0x48, ee::add(v, 5.0));
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02AF0 (D, task; frame 0x20): b597's arc with kind 2 in (0x40, 0x10,
/// 0x10) over grey 0x30.
fn arc(fx: &mut Fx, t: u32, s0: u32) {
    enter_step(fx, 0x20, 1, s0);
    if fx.mem.i16(t + 0x58) == 1 {
        let kind = fx.mem.i16(t + 0x50);
        let cfg = match kind {
            0 => Some((0x41F0_0000u32, 0x12C, (0x60u32, 0xFF, 0xFF, 0xFF), (0x64u32, 0x80, 0x80, 0xFF))),
            1 => Some((0x43AF_0000, 0xC, (0x60, 0xFF, 0xFF, 0xFF), (0x64, 0xFF, 0xC0, 0xC0))),
            2 => Some((0x4320_0000, 0xC, (0x64, 0x30, 0x30, 0x30), (0x60, 0x40, 0x10, 0x10))),
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
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2D64);
    fx.mem.write_u32(t + 0x20, 0x4000);
    fx.mem.write_u32(t + 0x24, 0);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02D90 (D, task; frame 0x20): b640's white ring (size 8..22, cell 4,
/// turned at random, +0x52 counting from 1, radius 20..49 growing by +0xE
/// = 15..34, +8 1..8, at 0x80 for 5..14 updates then out by 11).
fn ring_white(fx: &mut Fx, t: u32, s0: u32) {
    enter_step(fx, 0x20, 1, s0);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2DB4, 0xF);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 8));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        turn(fx, t, [0x01E0_2DF8, 0x01E0_2E14, 0x01E0_2E30]);
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_2E54, 0x1E);
        fx.mem.set16(t + 0x50, v + 0x14);
        let v = rmod(fx, t, 0x01E0_2E74, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2E90, 0x14);
        fx.mem.set16(t + 0xE, v + 0xF);
        let v = fx.rand_from(t, 0x01E0_2EB0);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_2ED0, 0xA);
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

/// 0x1E02F70 (D, task; frame 0x20): b597's white ring at the target (size
/// 5..11, cell 4, turned at random, radius 60..259, +8 1..8, at 0x80 for
/// 3..7 updates then out by 11).
fn ring_target(fx: &mut Fx, t: u32, s0: u32) {
    enter_step(fx, 0x20, 1, s0);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2F94, 7);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 5));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        turn(fx, t, [0x01E0_2FD8, 0x01E0_2FF4, 0x01E0_3010]);
        let v = rmod(fx, t, 0x01E0_302C, 0xC8);
        fx.mem.set16(t + 0x50, v + 0x3C);
        let v = rmod(fx, t, 0x01E0_304C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_3068);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_3088, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03110 (D, task; frame 0x20): b640's stage light +0x50 from 0x80
/// down by 10 to 0x30 for 250 updates, then back up by 10 to 0x80; at the
/// timer's 3 the light 0x80 and the task ends.
fn light(fx: &mut Fx, t: u32, s0: u32) {
    enter_step(fx, 0x20, 1, s0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let v = fx.mem.i16(t + 0x50);
    if fx.mem.i16(t + 0x54) < 0xFA {
        fx.mem.set16(t + 0x50, if v < 0x31 { 0x30 } else { v - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if v < 0x80 { v + 0xA } else { 0x80 });
    }
    age(fx, t);
    if fx.mem.read_u32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
    fx.leave(0x20);
}

/// 0x1E031F0 (D, task; frame 0x90): b597's window sprite in (0x90, 0x78,
/// 0x78, alpha +0x63): in by 8 below 0x50 while the counter is under
/// 0x1A4, then out by 7.
fn frame(fx: &mut Fx, t: u32, s0: u32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(s0))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3FC0_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for (k, c) in [0x90u8, 0x78, 0x78].into_iter().enumerate() {
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
    if fx.mem.i16(COUNTER) < 0x1A4 {
        up(fx, t, 0x63, 0x50, 8);
    } else {
        down(fx, t, 0x63, 9, 7);
    }
    age(fx, t);
    fx.leave(0x90);
}

/// 0x1E04120(D, task): the bloom's two half quads (0x1095C0 of size +0x48,
/// the right / left half: x 0 at the centre, the left mirrored) at the
/// point (scale 0.1, camera angles with y 0; 0x1097D0), each projected
/// (0x204D80, all four corners or nothing), colour +0x60 (0x108020), STQ
/// over the texture by 1 / w, Z 0xFFFFFF, ALPHA 0x48, texture (5, 3)
/// (filter 0), key 0. A 0x210 frame (the descriptor at sp + 0x160).
fn bloom_draw(fx: &mut Fx, t: u32, s0: u32, s1: u32) {
    let sp = fx.enter(0x210);
    let d = sp + 0x160;
    fx.mem.set16(d + 2, 0x34);
    fx.mem.set16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    fx.desc_alpha(d, 0x80_0000_0048);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xF0 + 4 * k, 0x3DCC_CCCD);
    }
    camera_angles_saving(fx, sp + 0x150, u64::from(s0), u64::from(s1), 0x01E0_41B4);
    fx.mem.write_u32(sp + 0x154, 0);
    for c in 0..3 {
        let w = fx.mem.read_u32(POINT + 4 * c);
        fx.mem.write_u32(sp + 0xE0 + 4 * c, w);
    }
    l2::xform_vp(fx, sp + 0x110, sp + 0xF0, sp + 0x150, sp + 0xE0);
    for k in 0..2 {
        let s = fx.mem.f(t + 0x48);
        l2::quad_xy(fx, sp + 0xA0, s, s);
        fx.mem.write_u32(sp + 0xB0, 0);
        fx.mem.write_u32(sp + 0xD0, 0);
        if k == 1 {
            for a in [sp + 0xA0, sp + 0xC0] {
                let v = ee::mul(fx.mem.f(a), -1.0);
                fx.mem.setf(a, v);
            }
        }
        let m = fx.read_m4(sp + 0x110);
        let ok = (0..4u32).all(|i| fx.project_at(sp + 0xA0 + 0x10 * i, &m, sp + 0x180 + 0x18 * i, sp + 0x100 + 4 * i));
        if !ok {
            continue;
        }
        for i in 0..4u32 {
            l2::rgbaq(fx, sp + 0x1F0 + 8 * i, t + 0x60);
            for c in 0..2 {
                let w = fx.mem.read_u32(sp + 0x1F0 + 8 * i + 4 * c);
                fx.mem.write_u32(sp + 0x170 + 0x18 * i + 4 * c, w);
            }
        }
        let w = |fx: &mut Fx, i: u32| fx.mem.f(sp + 0x100 + 4 * i);
        for i in 0..4u32 {
            let q = ee::div(1.0, w(fx, i));
            fx.mem.setf(sp + 0x174 + 0x18 * i, q);
        }
        for i in 0..4u32 {
            let a = sp + 0x180 + 0x18 * i;
            fx.set_xyz_z(a, 0xFF_FFFF);
            fx.mem.write_u8(a + 7, 0);
        }
        // S / T: (0, 0), (1, 0), (0, 1), (1, 1) over w.
        for (i, (s, tt)) in [(0.0f32, 0.0f32), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)].into_iter().enumerate() {
            let i = i as u32;
            let v = ee::div(s, w(fx, i));
            fx.mem.setf(sp + 0x178 + 0x18 * i, v);
            let v = ee::div(tt, w(fx, i));
            fx.mem.setf(sp + 0x17C + 0x18 * i, v);
        }
        let tex = fx.mem.read_u32(TEX + 0xC);
        fx.desc_texture(d, tex, 0);
        let ot = fx.mem.read_u32(OT_AT);
        fx.submit(d, ot, 0);
    }
    fx.leave(0x210);
}

/// 0x1E033E0(D, task) (frame 0x40: f20 / f21, the unit flash's s0 = D / s1
/// = the slot): the flash: the window into the flash descriptor (UVs inset
/// by +8 / 10 and +0xA / 10 over 512), no ordering table; first (0x40, 8,
/// 8), alpha 0; the insets grow by 23..37 a update; on its first update
/// the alpha in by 0x60 under 0x60, then out by 10 (the task ends under
/// 0xC).
fn flash(fx: &mut Fx, t: u32, k: u32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_46F4), (0x20, u64::from(k)), (0x10, u64::from(D))]);
    // swc1 f20 / f21: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x40, 8, 8);
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
    for (o, v) in [(0x18u32, ee::div(w, 512.0)), (0x1C, ee::div(h, 512.0)), (0x28, ee::div(ee::sub(512.0, w), 512.0)), (0x2C, ee::div(ee::sub(512.0, h), 512.0))] {
        fx.mem.setf(d + o, v);
    }
    fx.submit(d, 0, 0);
    for (o, ra) in [(8u32, 0x01E0_3618u32), (0xA, 0x01E0_3648)] {
        let v = rmod(fx, t, ra, 0xF);
        let s = fx.mem.i16(t + o) + ((v + 0x17) as i16 as i32);
        fx.mem.set16(t + o, s);
    }
    if fx.mem.i16(t + 0x54) <= 0 {
        up(fx, t, 0x63, 0x60, 0x60);
    } else {
        down(fx, t, 0x63, 0xC, 0xA);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E044F0(D) (frame 0x140: ra, the entry's s2 = D, s1 = the live count,
/// s0 = the flash count): in a packet layer (0x2078A0) the attacker drawn
/// into the target (0x1F8270 filter 0, its TEX0 / TEX1 into the sprite
/// descriptor at sp + 0x58; ALPHA 0x48), a second descriptor at sp + 0xB0
/// (ALPHA 0x48, unused); the attacker's position 30 along z by the facing
/// projected (its w D+0xA018); TEXFLUSH before and after; every flash task
/// stepped; the layer linked into the ordering table at the key w.
fn unit_flash(fx: &mut Fx, flashes: u32, live: u32) {
    let sp = fx.enter(0x140);
    fx.saved(sp, &[(0x30, 0x01E0_3ED4), (0x20, u64::from(D)), (0x10, u64::from(live)), (0, u64::from(flashes))]);
    let d = sp + 0x58;
    let d2 = sp + 0xB0;
    fx.mem.write_u32(FLASH_DESC, d);
    fx.mem.write_u32(FLASH_DESC2, d2);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u16(d2 + 2, 0x34);
    fx.mem.write_u16(d2, 0xC);
    fx.mem.write_u64(d2 + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d2 + 4, 0);
    fx.texflush([0xC, 0x34, d, 0x4_2421]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    fx.unit_draw(h, 0, sp + 0x98, sp + 0xA0);
    fx.desc_alpha(d2, 0x80_0000_0048);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x40);
    let z = fx.mem.f(sp + 0x48);
    let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 30.0) } else { ee::sub(z, 30.0) };
    fx.mem.setf(sp + 0x48, z);
    let p = [fx.mem.f(sp + 0x40), fx.mem.f(sp + 0x44), fx.mem.f(sp + 0x48)];
    let mut x = fx.mem.read_u64(sp + 0xD0);
    // a0..a3 as 0x204D80 (no matrix) leaves them [run]: on screen 0 (the
    // Z word's low half), the XYZ value's low word, sp - 0x10 (its vector),
    // the camera record 0x20E2B0 pointed at; off screen 0x20B2C0's (the
    // matrix stack index x 0x70, the index) [H].
    let cam = 0x002E_D760u32.wrapping_add(fx.mem.read_u32(0x002E_D6C0).wrapping_mul(0x1A0));
    let regs = if let Some(w) = fx.project(p, None, &mut x) {
        fx.mem.write_u64(sp + 0xD0, x);
        fx.mem.setf(FLASH_W, w);
        [0, x as u32, sp - 0x10, cam]
    } else {
        let i = fx.mem.read_u32(0x002E_D6A0);
        [i.wrapping_mul(0x70), i, sp - 0x10, cam]
    };
    fx.texflush(regs);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == FLASH {
            flash(fx, t, k);
        }
    }
    let v = fx.mem.f(FLASH_W);
    fx.fix_residue(v, 0x01E0_4724);
    let key = ee::fixunssfsi(v) as i32;
    let layer = fx.mem.read_u32(LAYER);
    fx.layer_end(layer, OT, key);
    fx.leave(0x140);
}
