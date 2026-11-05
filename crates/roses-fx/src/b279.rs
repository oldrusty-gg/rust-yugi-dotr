//! BMODULE b279 (card 184), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02AB0, data area 0x1ED0000) [F] (helper A12, 31
//! Oct 2026): b491's pool and data layout (150 tasks of 0x70 bytes at
//! D+0x2A0, alive +0x58, step +0x44 through `jalr`, 0x1E02AA0 an idle
//! step; spawned by 0x1E03220 = b491's spawn with the kind at D+0x4506)
//! drawn with sparks (21cb40d76bec at 0x1E010D0, `quads4::sparks`), flat
//! arcs (004542c6809a at 0x1E00D70, `billboard::arc_strips_flat`), arcs
//! of quads (6e60344f604f at 0x1E00920, `billboard::arc_quads`) and rings
//! of quads (7369422df7f8 at 0x1E014D0, `billboard::ring_quads`), UVs by
//! uv_rect 0x1E00080. 41..90 a blue arc of quads a update at the
//! attacker's node 5 shrinking, from 41 a white flat arc there for good;
//! 75..190 a grey spark ring a update at node 5 widening; on 202 thirty
//! dim spark rings closing on the target; on 220 a white flat arc, five
//! rings of quads and five wide ones at the target. Returns 1 on update
//! 210, 4 on 230, 2 on 220, 8 once the 251-update timer +0x44A0 has run
//! out.
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
const SPARKS: u32 = 0x01E0_1CE0;
const CLOSING: u32 = 0x01E0_1E80;
const FLAT: u32 = 0x01E0_2020;
const RINGQ: u32 = 0x01E0_2160;
const RINGQ_W: u32 = 0x01E0_2310;
const ARC: u32 = 0x01E0_2500;
const HALO: u32 = 0x01E0_2960;
const IDLE: u32 = 0x01E0_2AA0;
/// The return address of the entry's `jalr` into a step (0x1E02CC0 + 8).
const TASK_RA: u64 = 0x01E0_2CC8;
const POS: u32 = D + 0x250;
const TEX: u32 = D + 0x4480;
const TIMER: u32 = D + 0x44A0;
const COLOUR: u32 = D + 0x44A4;
const NODE: u32 = D + 0x44D8;
const TARGET_NODE: u32 = D + 0x44E8;
const KIND: u32 = D + 0x4506;
const FACING: u32 = D + 0x450A;
const COUNTER: u32 = D + 0x450E;
const BYTES: u32 = D + 0x4510;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02AB0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0x44AC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4880);
    l2::sine_table(fx, D + 0x4710);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x450C, param);
    fx.mem.set32(TIMER, 0xFB);
    for (k, node) in [5, 0x39, 1].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x288 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_2C0C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    fx.mem.set16(D + 0x44FC, -0x3C);
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2C3C, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
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
        if f == 0 || f == IDLE {
            continue;
        }
        // The steps' prologues: the entry's s0 = D (and s1 = 0).
        if f == HALO {
            let s = fx.enter(0x30);
            fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
            halo(fx, t);
            fx.leave(0x30);
            continue;
        }
        let s = fx.enter(0x20);
        fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
        match f {
            SPARKS => spark_ring(fx, t, false),
            CLOSING => spark_ring(fx, t, true),
            FLAT => flat(fx, t),
            RINGQ => ringq(fx, t),
            RINGQ_W => ringq_w(fx, t),
            ARC => arc(fx, t),
            _ => panic!("b279: unknown task step {f:#x}"),
        }
        fx.leave(0x20);
    }
    // The entry's s1: the live tasks counted in the draw loop.
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        for (f, tex, mode) in [(SPARKS, TEX, 0x20), (CLOSING, TEX + 4, 0x21)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            let w = fx.mem.read_u32(tex);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x1F), (0x9A, 0x7F), (0xB8, 0)] {
                fx.mem.set16(D + o, v);
            }
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, v);
            }
            let a = fx.mem.i16(t + 0x5C);
            let s = l2::sin_deg(fx, D + 0xD8, a, 0);
            let x = ee::add(fx.mem.f(D + 0x20), ee::mul(fx.mem.f(t + 0x30), s));
            fx.mem.setf(D + 0x20, x);
            let a = fx.mem.i16(t + 0x5C);
            let c = l2::sin_deg(fx, D + 0xD8, a, 1);
            let y = ee::add(fx.mem.f(D + 0x24), ee::mul(fx.mem.f(t + 0x30), c));
            fx.mem.setf(D + 0x24, y);
            fx.mem.set16(D + 0xB8, 0x18);
            quads4::sparks(fx, D, t, mode, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == FLAT {
            fx.mem.set16(D + 0xB8, 0x18);
            billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == RINGQ {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x60);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == RINGQ_W {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x70);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 1);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            let r = if fx.mem.i16(t + 0x50) == 0 { sp + 0x80 } else { sp + 0x90 };
            cell(fx, t, r);
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODE);
            node_position_saving(fx, h, slot, sp + 0x50, u64::from(D), Some(u64::from(live)), 0x01E0_3078);
            fx.mem.set16(D + 0xB8, 9);
            billboard::arc_quads(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == HALO {
            fx.mem.set16(D + 0xB8, 0x18);
            billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xD2 => 1,
        0xE6 => 4,
        0xDC => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell 0x9B frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x9B, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E03220(D, step, 0) (b491's 0x1E03E50 with the kind at D+0x4506):
/// the first free task cleared and set off at D+0x250 (i16 +0, float
/// +0x10) with the colour D+0x44A4 (twice), kind +0x50; `saves` the
/// caller's s0..s4, `ra` the return address.
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

/// 0x1E01A80(D, 0): the update counter and the emitters (frame 0x30: ra,
/// the entry's s1 = 0, s0 = D; then s0 = D, s1 a loop count).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_2C88), (0x10, 0), (0, u64::from(D))]);
    // The spawns' saves: s0 = D, s1, the entry's s2 = the parameter, s3 /
    // s4 the runner's (0 [H]).
    let p = param as i64 as u64;
    let saves = |s1: u64| [u64::from(D), s1, p, 0, 0];
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let at = |fx: &mut Fx, target: bool, ra: u32| {
        let (h, slot) = if target { (fx.target_handle(), fx.mem.i16(TARGET_NODE)) } else { (fx.self_handle(), fx.mem.i16(NODE)) };
        node_position_saving(fx, h, slot, POS, u64::from(D), Some(0), ra);
    };
    if (0x29..0x5B).contains(&c(fx)) {
        at(fx, false, 0x01E0_1AD4);
        spawn(fx, ARC, saves(0), 0x01E0_1AE8);
    }
    if c(fx) == 0x29 {
        spawn(fx, HALO, saves(0), 0x01E0_1B0C);
    }
    if (0x4B..0xBF).contains(&c(fx)) {
        at(fx, false, 0x01E0_1B40);
        spawn(fx, SPARKS, saves(0), 0x01E0_1B54);
    }
    if c(fx) == 0xCA {
        at(fx, true, 0x01E0_1B7C);
        for k in 0..0x1Eu64 {
            spawn(fx, CLOSING, saves(k), 0x01E0_1B98);
        }
    }
    if c(fx) == 0xDC {
        at(fx, true, 0x01E0_1BE8);
        spawn(fx, FLAT, saves(0), 0x01E0_1BFC);
        at(fx, true, 0x01E0_1C14);
        for k in 0..5u64 {
            spawn(fx, RINGQ, saves(k), 0x01E0_1C30);
        }
        for k in 0..5u64 {
            spawn(fx, RINGQ_W, saves(k), 0x01E0_1C70);
        }
    }
    if c(fx) >= 0xF {
        let v = fx.mem.i16(D + 0x44FC);
        if v < 0x5A {
            fx.mem.set16(D + 0x44FC, v + 3);
        }
    }
    fx.leave(0x30);
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

/// Byte +0x63 up by 0x40 under 0xC0 for `life` updates, then down by
/// `down` while at least `floor` (else the task ends).
fn fade(fx: &mut Fx, t: u32, life: i32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < life {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= floor {
        fx.mem.write_u8(t + 0x63, a - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E01CE0 (grey, inner radius 20 widening by 40 (by 10 the first two
/// updates while it rises 2.5), the outer 38 inside and widening too) /
/// 0x1E01E80 (`closing`: dim, from 1300 closing by 60 while over 40, the
/// outer 300 inside; random angles) (D, task): a ring of sparks (spans
/// 15 / 15, turning +0x24 by 0x100 a update); 0x80 for 19 / 18 updates,
/// then down by 14.
fn spark_ring(fx: &mut Fx, t: u32, closing: bool) {
    let f = if closing { CLOSING } else { SPARKS };
    if fx.mem.i16(t + 0x58) == 1 {
        let (r, g, by) = if closing { (0x44A2_8000u32, 0x20, 0x4416_0000u32) } else { (0x41A0_0000, 0x80, 0x43C8_0000) };
        fx.mem.write_u32(t + 0x48, r);
        let o = ee::sub(f32::from_bits(r), 58.0);
        fx.mem.setf(t + 0x4C, o);
        l2::rgb(fx, t + 0x60, g, g, g);
        l2::rgb(fx, t + 0x64, g, g, g);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.write_u32(t + 0x34, by);
        let q = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, q);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        if closing {
            for (o, ra) in [(0x20u32, f + 0xA0), (0x24, f + 0xBC), (0x28, f + 0xD8)] {
                let v = rmod(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
        } else {
            let v = rmod(fx, t, f + 0x9C, 0xFFFF);
            fx.mem.set32(t + 0x24, v);
            let v = fx.rand_from(t, f + 0xB8);
            fx.mem.set32(t + 0x28, v % 0x2000 - 0x1000);
        }
    }
    if closing {
        hold(fx, t, 0x12, 0x80, 0x10, 0xE);
        let r = fx.mem.f(t + 0x48);
        if !(r <= 40.0) {
            let by = fx.mem.f(t + 0x34);
            fx.mem.setf(t + 0x48, ee::sub(r, by));
        }
        let r = fx.mem.f(t + 0x48);
        fx.mem.setf(t + 0x4C, ee::sub(r, 300.0));
    } else {
        hold(fx, t, 0x13, 0x80, 0x10, 0xE);
        if fx.mem.i16(t + 0x54) < 2 {
            let r = ee::add(fx.mem.f(t + 0x48), ee::div(fx.mem.f(t + 0x34), 4.0));
            fx.mem.setf(t + 0x48, r);
            let y = ee::add(fx.mem.f(t + 0x14), 2.5);
            fx.mem.setf(t + 0x14, y);
        } else {
            let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
            fx.mem.setf(t + 0x48, r);
            let o = ee::add(fx.mem.f(t + 0x4C), fx.mem.f(t + 0x34));
            fx.mem.setf(t + 0x4C, o);
        }
    }
    let a = fx.mem.read_u32(t + 0x24).wrapping_add(0x100);
    fx.mem.write_u32(t + 0x24, a);
    age(fx, t);
}

/// 0x1E02020 (D, task): a white flat arc (radius 810, blue outer, spans
/// 15 / 15) flickering 0xFF / 0xF0, facing the camera turned a quarter;
/// 0xC0 for 3 updates, then down by 11.
fn flat(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x444A_8000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0xFF);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    hold(fx, t, 3, 0xC0, 0xD, 0xB);
    let g = if fx.mem.i16(t + 0x54) % 2 == 1 { 0xFF } else { 0xF0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2128);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02160 (D, task): a blue ring of quads: size 10..19, cell 0..3,
/// radius 5..34 growing by 20..54, a random angle, row 0..34, life 4..7;
/// in by 0x40 under 0xC0, out by 16.
fn ringq(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2184, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = fx.rand_from(t, 0x01E0_21B0);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
        let v = rmod(fx, t, 0x01E0_21E4, 0x1E);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, 0x01E0_2204, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2220, 0x23);
        fx.mem.set16(t + 0xE, v + 0x14);
        let v = rmod(fx, t, 0x01E0_2240, 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, 0x01E0_225C);
        fx.mem.set16(t + 6, v % 4 + 4);
        fx.mem.set16(t + 0x58, 2);
    }
    let life = fx.mem.i16(t + 6);
    fade(fx, t, life, 0x12, 0x10);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02310 (D, task): a wide blue ring of quads (random angles +0x20..,
/// orientation +0x52 = 1): size 20..39, cell 0..3, radius 500..899
/// growing by 1..3, a random angle, row 1..8, life 2..4; 0x60 during the
/// life, then down by 13.
fn ringq_w(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2334, 0x14);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x14));
        let v = fx.rand_from(t, 0x01E0_2360);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
        for (o, ra) in [(0x20u32, 0x01E0_2394u32), (0x24, 0x01E0_23B0), (0x28, 0x01E0_23CC)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_23F0, 0x190);
        fx.mem.set16(t + 0x50, v + 0x1F4);
        let v = rmod(fx, t, 0x01E0_2410, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_242C, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, 0x01E0_244C);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_246C, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    let life = fx.mem.i16(t + 6);
    hold(fx, t, life, 0x60, 0xF, 0xD);
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
}

/// 0x1E02500 (D, task): a blue arc of quads (inner radius 10..29
/// shrinking by 1, the outer 6 beyond; random angles, from 45 degrees
/// spanning 2); in by 0x40 under 0xC0 for 8 updates, out by 9.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2524, 0x14);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0xFF);
        let v = rmod(fx, t, 0x01E0_2574, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, ra) in [(0x20u32, 0x01E0_259Cu32), (0x24, 0x01E0_25B8), (0x28, 0x01E0_25D4)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x5A, 0x2D);
        fx.mem.set16(t + 0x5C, 2);
    }
    fade(fx, t, 8, 0xB, 9);
    let r = ee::sub(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::add(6.0, r));
    age(fx, t);
}

/// 0x1E02960 (D, task): a white flat arc (radius 110, blue outer, spans
/// 15 / 15) at the attacker's node 5, flickering 0xFF / 0xCF, facing the
/// camera turned a quarter; in by 9 under 0xC0 for 20 updates; never
/// ends.
fn halo(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42DC_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0xFF);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 9);
        }
    }
    let g = if fx.mem.i16(t + 0x54) % 2 == 1 { 0xFF } else { 0xCF };
    l2::rgb(fx, t + 0x60, g, g, g);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2A58);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE);
    node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_2A78);
    age(fx, t);
}
