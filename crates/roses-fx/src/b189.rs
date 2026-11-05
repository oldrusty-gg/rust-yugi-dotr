//! BMODULE b189 (card 68), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03230, data area 0x1ED0000) [F] (helper A12, 31
//! Oct 2026): b491's pool / spawn (150 tasks of 0x70 bytes at D+0x2A0,
//! alive +0x58, step +0x44 through `jalr`, 0x1E03220 an idle step;
//! spawned by 0x1E03A40 with the colour D+0x44C4 and the kind D+0x4528)
//! drawn with sparks (21cb40d76bec at 0x1E01320, `quads4::sparks`), flat
//! arcs (004542c6809a at 0x1E01720, `billboard::arc_strips_flat`), rings
//! of quads (7369422df7f8 at 0x1E00D70, `billboard::ring_quads`) and a
//! quad (add5d81dd3fe at 0x1E01A80, `billboard::quad`), UVs by uv_rect.
//! A point (+0x4470) rides the attacker's node 0x36 (5 higher) until 105,
//! then drifts back toward node 0x43 (and sideways by (double) k x 0.1
//! through the soft-float chain); from 120 to the target in 16 steps
//! (126..141). 51..162 a red flat arc on it a update and 65..162 a purple
//! ring; 55 a grey glowing quad there; 125..135 white spark rings at node
//! 0x43 on odd updates; on 142 ten dim spark rings opening and closing and
//! a white flat arc, on 164 ten wide and ten red rings and another arc.
//! Returns 1 on update 132, 4 on 205, 2 on 185, 6 on 143..179, 8 once the
//! 220-update timer +0x44C0 has run out.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const BURST: u32 = 0x01E0_2290;
const PURPLE: u32 = 0x01E0_23E0;
const RED_ARC: u32 = 0x01E0_2560;
const WHITE_ARC: u32 = 0x01E0_2850;
const GLOW: u32 = 0x01E0_2990;
const SWING: u32 = 0x01E0_2B00;
const RED_RING: u32 = 0x01E0_2D00;
const WIDE: u32 = 0x01E0_2EA0;
const IDLE: u32 = 0x01E0_3220;
/// The return address of the entry's `jalr` into a step (0x1E03518 + 8).
const TASK_RA: u64 = 0x01E0_3520;
const POS: u32 = D + 0x250;
const POINT: u32 = D + 0x4470;
const TEX: u32 = D + 0x44A0;
const TIMER: u32 = D + 0x44C0;
const COLOUR: u32 = D + 0x44C4;
const K: u32 = D + 0x44E4;
const NODES: u32 = D + 0x44F8;
const TARGET_NODE: u32 = D + 0x4508;
const DIVISOR: u32 = D + 0x4518;
const KIND: u32 = D + 0x4528;
const FACING: u32 = D + 0x452C;
const COUNTER: u32 = D + 0x4530;
const BYTES: u32 = D + 0x4532;
const BYTES_B: u32 = D + 0x4632;
const SINES: u32 = D + 0x4734;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E03230.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    fx.mem.write_u32(D + 0x44CC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x48A0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x452E, param);
    fx.mem.set32(TIMER, 0xDC);
    for (k, node) in [0x36, 0x34, 0x43, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x298 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_33C4);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_33EC, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_343C);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_348C, 0x28);
        fx.mem.set8(BYTES_B + k, v - 0x14);
    }
    fx.mem.set16(DIVISOR, 1);
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
        if f == 0 || f == IDLE {
            continue;
        }
        // The steps' prologues: the entry's s0 = D (and s1 = 0).
        if f == RED_ARC || f == SWING {
            let s = fx.enter(0x30);
            fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
            if f == RED_ARC {
                red_arc(fx, t);
            } else {
                swing(fx, t);
            }
            fx.leave(0x30);
            continue;
        }
        let s = fx.enter(0x20);
        fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
        match f {
            BURST => burst(fx, t),
            PURPLE => ring(fx, t, false),
            RED_RING => ring(fx, t, true),
            WHITE_ARC => white_arc(fx, t),
            GLOW => glow(fx, t),
            WIDE => wide(fx, t),
            _ => panic!("b189: unknown task step {f:#x}"),
        }
        fx.leave(0x20);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == BURST {
            cell(fx, t, sp + 0x60, 0x97);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            let v = fx.mem.i16(D + 0x9A) >> 1;
            fx.mem.set16(D + 0x9A, v);
            fx.mem.set16(D + 0xB8, 0x18);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == PURPLE {
            cell(fx, t, sp + 0x70, 0x14);
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if f(fx) == RED_ARC {
            fx.mem.set16(D + 0xB8, 0x18);
            billboard::arc_strips_flat(fx, D, t, 0x21, POINT);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == WHITE_ARC {
            fx.mem.set16(D + 0xB8, 0x18);
            billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == GLOW {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x80, 0x14);
            fx.mem.set16(D + 0xBE, 5);
            for k in 0..4 {
                let v = fx.mem.read_u32(POINT + 4 * k);
                fx.mem.write_u32(sp + 0x50 + 4 * k, v);
            }
            billboard::quad(fx, uv::uv_rect, D, t, 0x21, sp + 0x50);
        }
        if f(fx) == SWING {
            cell(fx, t, sp + 0x90, 0x14);
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 0x18);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == RED_RING {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0xA0, 0x9B);
            fx.mem.set16(D + 0xB8, 5);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if f(fx) == WIDE {
            cell(fx, t, sp + 0xB0, 0x14);
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 1);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x84 => 1,
        0xCD => 4,
        0xB9 => 2,
        0x8F..=0xB3 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E03A40(D, step, 0) (b491's 0x1E03E50 with the colour D+0x44C4 and
/// the kind D+0x4528): the first free task cleared and set off at D+0x250;
/// `saves` the caller's s0..s4, `ra` the return address.
fn spawn(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        let t = D + 0x2A0 + 0x70 * k;
        fx.bzero(t, 0x70);
        fx.mem.set16(t + 0x58, 1);
        for o in [0x60u32, 0x64] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + o + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        for k in 0..3 {
            let w = fx.mem.read_u32(POS + 4 * k);
            fx.mem.write_u32(t + 0x10 + 4 * k, w);
        }
        let s = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x50, s);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

/// What the soft-float chain (0x1A2C30 / 0x1A31D8 / 0x1A38F8) leaves
/// below the caller's sp `top` (b555's lesson): 0x1A31D8's saves of the
/// caller's s1 / s2 / s3, then 0x1A38F8's unpack of the product.
fn soft_float_saved(fx: &mut Fx, top: u32, p: f64, s123: [u64; 3]) {
    fx.mem.write_u64(top - 0x38, s123[0]);
    fx.mem.write_u64(top - 0x30, s123[1]);
    fx.mem.write_u64(top - 0x28, s123[2]);
    let bits = p.to_bits();
    let exp = ((bits >> 52) & 0x7FF) as u32;
    let frac = bits & ((1 << 52) - 1);
    if exp == 0x7FF {
        if frac != 0 {
            fx.mem.write_u64(top - 0x30, frac);
        }
    } else if exp != 0 {
        fx.mem.write_u32(top - 0x38, exp.wrapping_sub(0x3FF));
        fx.mem.write_u64(top - 0x30, (frac << 8) | (1 << 60));
    }
}

/// 0x1E01D40(D, 0): the update counter, the point and the spawns (frame
/// 0x50: ra, the entry's s1 = 0, s0 = D; then s0 = D, s1 a loop count).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_34E0), (0x10, 0), (0, u64::from(D))]);
    // The spawns' saves: s0 = D, s1, the entry's s2 = the parameter, s3 /
    // s4 the runner's (0 [H]).
    let p = param as i64 as u64;
    let saves = |s1: u64| [u64::from(D), s1, p, 0, 0];
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let at = |fx: &mut Fx, target: bool, slot: u32, out: u32, ra: u32| {
        let h = if target { fx.target_handle() } else { fx.self_handle() };
        let s = fx.mem.i16(slot);
        node_position_saving(fx, h, s, out, u64::from(D), Some(0), ra);
    };
    let to_pos = |fx: &mut Fx| {
        for k in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    };
    if c(fx) < 0x78 {
        at(fx, false, NODES, POINT, 0x01E0_1D88);
        let y = ee::add(fx.mem.f(POINT + 4), 5.0);
        fx.mem.setf(POINT + 4, y);
        if c(fx) == 0x69 {
            at(fx, false, NODES + 4, sp + 0x30, 0x01E0_1DC4);
            let v = ee::div(ee::sub(fx.mem.f(POINT + 4), fx.mem.f(sp + 0x34)), 15.0);
            fx.mem.setf(D + 0x4494, v);
            let v = ee::div(ee::sub(fx.mem.f(POINT), fx.mem.f(sp + 0x30)), 15.0);
            fx.mem.setf(D + 0x4490, v);
        }
        if c(fx) >= 0x69 {
            let k = fx.mem.f(K);
            for (o, by) in [(0u32, D + 0x4490), (4, D + 0x4494)] {
                let v = ee::sub(fx.mem.f(POINT + o), ee::mul(fx.mem.f(by), k));
                fx.mem.setf(POINT + o, v);
            }
            let p = f64::from(k) * f64::from_bits(0x3FB9_9999_9999_999A);
            soft_float_saved(fx, sp, p, [0, p_bits(param), 0]);
            let z = fx.mem.f(POINT + 8);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, p as f32) } else { ee::add(z, p as f32) };
            fx.mem.setf(POINT + 8, z);
            fx.mem.setf(K, ee::add(k, 1.0));
        }
    } else {
        if c(fx) == 0x78 {
            at(fx, true, TARGET_NODE, sp + 0x30, 0x01E0_1F04);
            for (o, to) in [(8u32, D + 0x4488), (4, D + 0x4484), (0, D + 0x4480)] {
                let v = ee::div(ee::sub(fx.mem.f(sp + 0x30 + o), fx.mem.f(POINT + o)), 16.0);
                fx.mem.setf(to, v);
            }
        }
        if (0x7E..0x8E).contains(&c(fx)) {
            for (o, by) in [(8u32, D + 0x4488), (4, D + 0x4484), (0, D + 0x4480)] {
                let v = ee::add(fx.mem.f(POINT + o), fx.mem.f(by));
                fx.mem.setf(POINT + o, v);
            }
        }
    }
    let mut s1 = 0u64;
    if c(fx) == 0x8E {
        to_pos(fx);
        fx.mem.set16(KIND, 0);
        for k in 0..0xAu64 {
            spawn(fx, SWING, saves(k), 0x01E0_1FE4);
        }
        s1 = 0xA;
        spawn(fx, WHITE_ARC, saves(s1), 0x01E0_2018);
    }
    if c(fx) == 0xA4 {
        to_pos(fx);
        fx.mem.set16(KIND, 1);
        for k in 0..0xAu64 {
            spawn(fx, WIDE, saves(k), 0x01E0_206C);
        }
        for k in 0..0xAu64 {
            spawn(fx, RED_RING, saves(k), 0x01E0_20A8);
        }
        s1 = 0xA;
        spawn(fx, WHITE_ARC, saves(s1), 0x01E0_20E0);
    }
    if (0x7D..0x88).contains(&c(fx)) {
        at(fx, false, NODES + 4, POS, 0x01E0_2114);
        let z = fx.mem.f(POS + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(POS + 8, z);
        if c(fx) % 2 == 1 {
            spawn(fx, BURST, saves(s1), 0x01E0_218C);
        }
    }
    if (0x41..0xA3).contains(&c(fx)) {
        to_pos(fx);
        spawn(fx, PURPLE, saves(s1), 0x01E0_21DC);
    }
    if (0x33..0xA3).contains(&c(fx)) {
        to_pos(fx);
        spawn(fx, RED_ARC, saves(s1), 0x01E0_222C);
        if c(fx) == 0x37 {
            spawn(fx, GLOW, saves(s1), 0x01E0_2250);
        }
    }
    if (0x3D..0x4B).contains(&c(fx)) {
        let v = fx.mem.i16(DIVISOR) + 7;
        fx.mem.set16(DIVISOR, v);
    }
    fx.leave(0x50);
}

fn p_bits(param: i32) -> u64 {
    param as i64 as u64
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// Byte +0x63 set to `full` for `life` updates, then down by `down` while
/// at least `floor` (else the task ends).
fn hold(fx: &mut Fx, t: u32, life: i32, full: u8, floor: u8, down: u8) {
    if fx.mem.i16(t + 0x54) < life {
        fx.mem.write_u8(t + 0x63, full);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= floor {
            fx.mem.write_u8(t + 0x63, a - down);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
}

/// Byte +0x63 up by `up` under 0xC0 / `cap` for `life` updates, then down
/// by `down` while at least `floor` (else the task ends).
fn fade(fx: &mut Fx, t: u32, life: i32, cap: u8, up: u8, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < life {
        if a < cap {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(up));
        }
    } else if a >= floor {
        fx.mem.write_u8(t + 0x63, a - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E02290 (D, task): a pale red spark ring (inner radius 50 widening
/// by 40, the outer 50 inside; spans 15 / 15, cell 4, turned a quarter,
/// a random +0x28); 0xC0 for 8 updates, then down by 14.
fn burst(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4248_0000);
        fx.mem.setf(t + 0x4C, ee::sub(50.0, 50.0));
        l2::rgb(fx, t + 0x60, 0xFF, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x64, 0xFF, 0xC0, 0xC0);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x56, 4);
        fx.mem.write_u32(t + 0x34, 0x43C8_0000);
        let q = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, q);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.write_u32(t + 0x20, 0x4000);
        let v = rmod(fx, t, 0x01E0_2334, 0xFFFF);
        fx.mem.set32(t + 0x28, v);
    }
    hold(fx, t, 8, 0xC0, 0x10, 0xE);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
        fx.mem.setf(t + 0x48, r);
        fx.mem.setf(t + 0x4C, ee::sub(r, 50.0));
    }
    age(fx, t);
}

/// 0x1E023E0 (purple, size 10, radius 10..59 fixed) / 0x1E02D00 (`red`:
/// size 20, radius 10..59 growing by 16..45) (D, task): a ring of quads:
/// cell 0..3, a random angle, +0xA rand % 8, +0xC the spawn kind, row
/// 0..34, life 3..7; 0x80 during the life, then down by 8.
fn ring(fx: &mut Fx, t: u32, red: bool) {
    let f = if red { RED_RING } else { PURPLE };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, if red { 0x41A0_0000 } else { 0x4120_0000 });
        let v = fx.rand_from(t, f + 0x28);
        fx.mem.set16(t + 0x56, v % 4);
        if red {
            l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        } else {
            l2::rgb(fx, t + 0x60, 0x40, 0x20, 0x80);
        }
        let v = rmod(fx, t, f + 0x5C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, f + 0x78);
        fx.mem.set16(t + 0xA, v % 8);
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, k);
        let v = rmod(fx, t, f + 0x9C, 0x32);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = if red { rmod(fx, t, f + 0xBC, 0x1E) + 0x10 } else { rmod(fx, t, f + 0xBC, 0xA) + 6 };
        fx.mem.set16(t + 0xE, v);
        let v = rmod(fx, t, f + 0xDC, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xF8, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let life = fx.mem.i16(t + 6);
    hold(fx, t, life, 0x80, 0xA, 8);
    if red && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02560 (D, task): a red flat arc on the point (radius 10 + rand %
/// D+0x4518 growing by 1, spans 15 / 15, the outer colour flickering),
/// facing the camera turned a quarter; in by 0x30 under 0x60 for 2
/// updates, out by 11.
fn red_arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_258C);
        let m = fx.mem.i16(DIVISOR);
        let r = fx.div(v, m).1;
        fx.mem.setf(t + 0x48, ee::add(10.0, ee::from_i32(r)));
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0x68, 0x68, 0x68);
        l2::rgb(fx, t + 0x64, 0x80, 0x40, 0x40);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 2, 0x60, 0x30, 0xD, 0xB);
    let r = if fx.mem.i16(t + 0x54) % 2 == 1 { 0x80 } else { 0x40 };
    l2::rgb(fx, t + 0x64, r, 0x20, 0x20);
    if fx.mem.i16(t + 0x54) != 0 {
        let v = ee::add(fx.mem.f(t + 0x48), 1.0);
        fx.mem.setf(t + 0x48, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_26CC);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02850 (D, task): a white flat arc (radius 250 / 300 by the spawn
/// kind, growing by 1, a red outer, spans 15 / 15) facing the camera
/// turned a quarter; in by 0x60 under 0xC0 for 12 updates, out by 11.
fn white_arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x48, 0x437A_0000);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x48, 0x4396_0000);
        }
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0xC, 0xC0, 0x60, 0xD, 0xB);
    if fx.mem.i16(t + 0x54) != 0 {
        let v = ee::add(fx.mem.f(t + 0x48), 1.0);
        fx.mem.setf(t + 0x48, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2958);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02990 (D, task): a grey glowing quad on the point (5 growing by 1
/// to 30, cell 3 / 5 alternating, turned by the camera's angles); both
/// alphas in by 0x40 under 0xC0 for 130 updates, then +0x63 down by 14
/// twice a update.
fn glow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 5);
        fx.mem.set16(t + 0xA, 5);
        fx.mem.set16(t + 0x56, 3);
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0x40);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x82 {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0xC0 {
                fx.mem.write_u8(t + o, a + 0x40);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        }
    }
    let c = if fx.mem.i16(t + 0x56) == 3 { 5 } else { 3 };
    fx.mem.set16(t + 0x56, c);
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s < 0x1E {
                fx.mem.set16(t + o, s + 1);
            }
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2AD8);
    age(fx, t);
}

/// 0x1E02B00 (D, task): a dim spark ring swinging open and shut: radius
/// 600 sin(+0x50) (+0x50 up by 8, by 6 from 90; the outer 400 inside),
/// spans 15 / 10, a random cell and angles; 0x80 for 20 updates, then the
/// task ends once the radius is under 80; from 91 degrees down by 7.
fn swing(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        fx.mem.setf(t + 0x4C, ee::sub(100.0, 58.0));
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x20);
        l2::rgb(fx, t + 0x64, 0x20, 0x20, 0x20);
        fx.mem.set16(t + 0x50, 0);
        let v = fx.rand_from(t, 0x01E0_2B70);
        fx.mem.set16(t + 0x56, v % 8);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.write_u32(t + 0x34, 0x437A_0000);
        let q = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, q);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, ra) in [(0x20u32, 0x01E0_2BC4u32), (0x24, 0x01E0_2BE0), (0x28, 0x01E0_2BFC)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else if fx.mem.f(t + 0x48) < 80.0 {
        fx.mem.set16(t + 0x58, 0);
    }
    let a = fx.mem.i16(t + 0x50);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let r = ee::mul(600.0, s);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::sub(r, 400.0));
    if fx.mem.i16(t + 0x50) >= 0x5B {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 9 {
            fx.mem.write_u8(t + 0x63, a - 7);
        }
    }
    let a = fx.mem.i16(t + 0x50);
    fx.mem.set16(t + 0x50, if a < 0x5A { a + 8 } else { a + 6 });
    age(fx, t);
}

/// 0x1E02EA0 (D, task): a wide pink ring of quads (random angles,
/// orientation +0x52 from 1 up by one a update): size 10..29, cell 0..3,
/// radius 500..899 growing by 1..3, a random angle, row 1..8, life 2..4;
/// 0x70 during the life, then down by 9.
fn wide(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2EC4, 0x14);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = fx.rand_from(t, 0x01E0_2EF0);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0xCF, 0x50, 0x80);
        for (o, ra) in [(0x20u32, 0x01E0_2F24u32), (0x24, 0x01E0_2F40), (0x28, 0x01E0_2F5C)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_2F80, 0x190);
        fx.mem.set16(t + 0x50, v + 0x1F4);
        let v = rmod(fx, t, 0x01E0_2FA0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2FBC, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, 0x01E0_2FDC);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_2FFC, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    let life = fx.mem.i16(t + 6);
    hold(fx, t, life, 0x70, 0xB, 9);
    let o = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, o);
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
}
