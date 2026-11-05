//! BMODULE b414 (card 492), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E036F0, data area 0x1ED0000, 0xB630 bytes) [F]
//!: b413's (card 518's) neighbour on b482's
//! shape (400 tasks of 0x70 bytes at D+0x2A0, alive +0x58, step +0x44
//! through `jalr`; the spawn 0x1E04100 = `b413::spawn_with` on the colour
//! D+0xB214, +0x50 / +0x52 from D+0xB2B0, +0x5A from D+0xB2A2), drawn with
//! the second library (uv_rect, arc_strips, quad, sprite_quads, sparks,
//! arc_strips_52), b413's quad9 (0x1E04610) and a record-own strip3 bar
//! 0x1E04800. Update 1 the stage light (down to 0x20 / 0x10, then up to
//! 0x30) and camera slots 4 / 5 aimed past node 8 from node 3; 0x3B /
//! 0x9E flares at node 0xC; below 0x8D two arcs a update (the second at
//! nodes 7 / 6 / 5 / 4); 0x74..0x90 quads at nodes 4..8; below 0x85 red
//! and grey quad pairs; 0xC2..0xE5 a big quad; 0xC1 a frame flash; 0xDC 40
//! sparks, 20 rising bars and an arc. Returns 1 on update 0xC3, 4 on
//! 0x122, 2 on 0x109, 6 on 0xCE..0x103, 8 once the 0x12C-update timer
//! +0xB210 has run out.
use crate::b076::node_position_saving;
use crate::b413::{age, angles, drift, fade_in, fade_out, quad9, rmod, spawn_with, spins};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const FLARE: u32 = 0x01E0_1C10;
const ARC_A: u32 = 0x01E0_1E00;
const ARC_B: u32 = 0x01E0_1FF0;
const QUAD_A: u32 = 0x01E0_21C0;
const PAIR_A: u32 = 0x01E0_2420;
const PAIR_B: u32 = 0x01E0_26F0;
const QUAD_B: u32 = 0x01E0_29C0;
const BAR: u32 = 0x01E0_2C20;
const ARC: u32 = 0x01E0_2D80;
const SPARK: u32 = 0x01E0_2E70;
const CAMERA: u32 = 0x01E0_3070;
const LIGHT: u32 = 0x01E0_3390;
const FLASH: u32 = 0x01E0_3510;
/// A step that only returns 0 (skipped by the entry).
const IDLE: u32 = 0x01E0_36E0;
/// The return address of the entry's `jalr` into a step (0x1E03A18 + 8).
const TASK_RA: u64 = 0x01E0_3A20;
const POS: u32 = D + 0x250;
const CAM: u32 = D + 0xB1A0;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const NODES: u32 = D + 0xB244;
const ANGLE: u32 = D + 0xB2A2;
const KIND: u32 = D + 0xB2B0;
const FACING: u32 = D + 0xB2B6;
const COUNTER: u32 = D + 0xB2BA;
const BYTES: u32 = D + 0xB2BC;
const SINES: u32 = D + 0xB4BC;
const SPR: u32 = 0x7000_0000;

fn spawn(fx: &mut Fx, step: u32) -> i32 {
    spawn_with(fx, step, 0xB214, 0xB2B0, 0xB2A2)
}

/// The entry 0x1E036F0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B8, param);
    fx.mem.set32(TIMER, 0x12C);
    for (k, node) in [0xC, 3, 4, 5, 6, 7, 8].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    fx.fix_residue(y, 0x01E0_3900);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3930, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_3984);
        fx.mem.set16(D + 0xB266 + 2 * k, v % 8);
    }
    0
}

/// D+0x94..+0x9B as halfwords: (+0x56 << 5, 0, 0x20, 0x40).
fn cell_row(fx: &mut Fx, t: u32) {
    let sub = fx.mem.i16(t + 0x56);
    fx.mem.set16(D + 0x94, sub << 5);
    fx.mem.set16(D + 0x96, 0);
    fx.mem.set16(D + 0x98, 0x20);
    fx.mem.set16(D + 0x9A, 0x40);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        let frame = match f {
            0 | IDLE => continue,
            ARC_A | ARC_B | PAIR_A | PAIR_B => 0x30,
            CAMERA => 0x40,
            FLASH => 0x90,
            FLARE | QUAD_A | QUAD_B | BAR | ARC | SPARK | LIGHT => 0x20,
            _ => panic!("b414: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        }
        match f {
            FLARE => flare(fx, t),
            ARC_A => arc_ab(fx, t, false),
            ARC_B => arc_ab(fx, t, true),
            QUAD_A => quad(fx, t, false),
            QUAD_B => quad(fx, t, true),
            PAIR_A => pair(fx, t, false),
            PAIR_B => pair(fx, t, true),
            BAR => bar(fx, t),
            ARC => arc(fx, t),
            SPARK => spark(fx, t),
            CAMERA => camera(fx, t, s),
            LIGHT => light(fx, t),
            _ => flash(fx, t, s),
        }
        fx.leave(frame);
    }
    // The draws: s0 counts the live tasks.
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        let at_nodes = |fx: &mut Fx, ras: [u32; 4], draw: &dyn Fn(&mut Fx)| {
            for (n, ra) in [(0xAu32, ras[0]), (8, ras[1]), (6, ras[2]), (4, ras[3])] {
                let h = fx.self_handle();
                let slot = fx.mem.i16(NODES + n);
                node_position_saving(fx, h, slot, t + 0x10, u64::from(live), Some(u64::from(D)), ra);
                draw(fx);
            }
        };
        if f(fx) == FLARE {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            crate::b615::cell(fx, sp + 0x50, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            let kind = fx.mem.i16(t + 0x50);
            let n = if kind < 2 { 2 } else if kind == 2 { 1 } else { 0 };
            for _ in 0..n {
                quads4::sprite_quads(fx, D, t, 0, t + 0x10);
            }
        }
        if f(fx) == ARC_A {
            fx.mem.set16(D + 0xB8, 6);
            quads4::arc_strips_52(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == ARC_B {
            fx.mem.set16(D + 0xB8, 6);
            at_nodes(fx, [0x01E0_3B90, 0x01E0_3BC4, 0x01E0_3BF8, 0x01E0_3C2C], &|fx| quads4::arc_strips_52(fx, D, t, 0x21, t + 0x10));
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == QUAD_A {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            crate::b615::cell(fx, sp + 0x60, 0x9B, sub);
            let o = fx.mem.i16(t + 0x52);
            fx.mem.set16(D + 0xBE, o);
            let sub = fx.mem.i16(t + 0x56);
            if sub < 2 {
                fx.mem.set16(D + 0x94, sub << 6);
                fx.mem.set16(D + 0x96, 0);
            } else {
                fx.mem.set16(D + 0x94, (sub - 2) << 6);
                fx.mem.set16(D + 0x96, 0x40);
            }
            fx.mem.set16(D + 0x98, 0x40);
            fx.mem.set16(D + 0x9A, 0x40);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if f(fx) == PAIR_A {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xBE, 0);
            cell_row(fx, t);
            at_nodes(fx, [0x01E0_3D64, 0x01E0_3D98, 0x01E0_3DCC, 0x01E0_3E00], &|fx| quad9(fx, uv::uv_rect, D, t, 0x20, t + 0x10));
        }
        if f(fx) == PAIR_B {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xBE, 0);
            cell_row(fx, t);
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 0xC);
            node_position_saving(fx, h, slot, t + 0x10, u64::from(live), Some(u64::from(D)), 0x01E0_3E7C);
            quad9(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if f(fx) == QUAD_B {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xBE, 0);
            cell_row(fx, t);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if f(fx) == BAR {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xBE, 0);
            cell_row(fx, t);
            bar_draw(fx, D, t);
        }
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == SPARK {
            let sub = fx.mem.i16(t + 0x56);
            crate::b615::cell(fx, sp + 0x70, 0x96, sub);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
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
        0xC3 => 1,
        0x122 => 4,
        0x109 => 2,
        0xCE..=0x103 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E016B0(D, 0): the update counter and the spawns (frame 0x40: s0 =
/// D, s1 / s2 loop counts).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_39E0), (0x20, 0), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let node = |fx: &mut Fx, n: u32, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + n);
        node_position_saving(fx, h, slot, POS, u64::from(D), Some(s1), ra);
    };
    if c(fx) == 1 {
        spawn(fx, LIGHT);
    }
    if c(fx) == 1 {
        spawn(fx, CAMERA);
    }
    if c(fx) == 0xC1 {
        spawn(fx, FLASH);
    }
    if c(fx) == 0x9E {
        node(fx, 0, u64::from(D), 0x01E0_1798);
        for (a, k) in [(0, 0), (0x5A, 0), (0x2D, 1), (0x87, 1)] {
            fx.mem.set16(ANGLE, a);
            fx.mem.set16(KIND, k);
            spawn(fx, FLARE);
        }
    }
    if c(fx) == 0x3B {
        node(fx, 0, u64::from(D), 0x01E0_1894);
        for a in [0, 0x5A] {
            fx.mem.set16(ANGLE, a);
            fx.mem.set16(KIND, 2);
            spawn(fx, FLARE);
        }
    }
    if c(fx) < 0x8D {
        for s1 in 0..2 {
            fx.mem.set16(KIND, s1);
            spawn(fx, if s1 == 0 { ARC_A } else { ARC_B });
        }
    }
    let v = c(fx);
    if (0x74..0x91).contains(&v) {
        for k in 0..5u32 {
            node(fx, 4 + 2 * k, u64::from(D + 2 * k), 0x01E0_19D0);
            spawn(fx, QUAD_A);
        }
    }
    if c(fx) < 0x85 {
        spawn(fx, PAIR_A);
    }
    if c(fx) < 0x85 {
        spawn(fx, PAIR_B);
    }
    if (0xC2..0xE6).contains(&c(fx)) {
        node(fx, 0xA, 0, 0x01E0_1AF4);
        node(fx, 0, 0, 0x01E0_1B14);
        spawn(fx, QUAD_B);
    }
    if c(fx) == 0xDC {
        node(fx, 0, 0, 0x01E0_1B60);
        for _ in 0..0x28 {
            spawn(fx, SPARK);
        }
        for _ in 0..0x14 {
            spawn(fx, BAR);
        }
        spawn(fx, ARC);
    }
    fx.leave(0x40);
}

/// 0x1E01C10 (D, task): a flare (sprite_quads, drawn twice for kinds 0 /
/// 1): violet; kind 0 18 x 1000 growing by 60, 1 18 x 250 by 20, 2 6 x
/// 100 by 5 in grey / blue turning by 6; in by 0x40 under 0x80 for 4
/// updates, then out by 9.
fn flare(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xF0, 0xE0, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0x40, 0xFF);
        for (k, w, h) in [(0, 0x4190_0000u32, 0x447A_0000u32), (1, 0x4190_0000, 0x437A_0000)] {
            if fx.mem.i16(t + 0x50) == k {
                fx.mem.write_u32(t + 0x30, w);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        if fx.mem.i16(t + 0x50) == 2 {
            fx.mem.write_u32(t + 0x30, 0x40C0_0000);
            fx.mem.write_u32(t + 0x34, 0x42C8_0000);
            l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
            l2::rgb(fx, t + 0x64, 0x40, 0x40, 0xFF);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    for (k, by) in [(0, 60.0f32), (1, 20.0)] {
        if fx.mem.i16(t + 0x50) == k && fx.mem.i16(t + 0x54) != 0 {
            let h = ee::add(fx.mem.f(t + 0x34), by);
            fx.mem.setf(t + 0x34, h);
        }
    }
    if fx.mem.i16(t + 0x50) == 2 {
        if fx.mem.i16(t + 0x54) != 0 {
            let h = ee::add(fx.mem.f(t + 0x34), 5.0);
            fx.mem.setf(t + 0x34, h);
        }
        let a = fx.mem.i16(t + 0x5A) + 6;
        fx.mem.set16(t + 0x5A, a);
    }
    age(fx, t);
}

/// 0x1E01E00 (`b` false: radius 15, z -30..-79, at node 8 each update) /
/// 0x1E01FF0 (true: radius 11, z -15..-44) (D, task): an arc fan
/// (arc_strips_52, 6 strips) white outside, red inside, from a random
/// angle, 60 x 6 degrees, tilted +-0x1000 by the facing; the outer alpha in
/// by 0x60 under 0xC0 for 4 updates, then out by 9; +0x34 falling by 13 /
/// 5 (65 / 35 from update 0x73), +0x4C up by 0.5 (2).
fn arc_ab(fx: &mut Fx, t: u32, b: bool) {
    let o = if b { 0x1F0 } else { 0 };
    if fx.mem.i16(t + 0x58) == 1 {
        let r = if b { 0x4130_0000u32 } else { 0x4170_0000 };
        fx.mem.write_u32(t + 0x4C, r);
        fx.mem.write_u32(t + 0x48, r);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        let v = rmod(fx, t, 0x01E0_1E60 + o, 0x168);
        fx.mem.set16(t + 0x52, v);
        fx.mem.set16(t + 0x5A, 0x3C);
        fx.mem.set16(t + 0x5C, 6);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let (m, lo) = if b { (0x1E, 0xF) } else { (0x32, 0x1E) };
        let v = rmod(fx, t, 0x01E0_1E94 + o, m);
        fx.mem.setf(t + 0x34, ee::from_i32(-(v + lo)));
        let a: i32 = if fx.mem.i16(FACING) != 0 { -0x1000 } else { 0x1000 };
        fx.mem.set32(t + 0x20, a);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x67, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x67, 0xB, 9);
    }
    let (slow, fast) = if b { (5.0f32, 35.0f32) } else { (13.0, 65.0) };
    let (fall, grow) = if fx.mem.i16(COUNTER) < 0x73 { (slow, 0.5f32) } else { (fast, 2.0) };
    let z = ee::sub(fx.mem.f(t + 0x34), fall);
    fx.mem.setf(t + 0x34, z);
    let r = ee::add(fx.mem.f(t + 0x4C), grow);
    fx.mem.setf(t + 0x4C, r);
    if !b {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 0xC);
        node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_1FC0);
    }
    age(fx, t);
}

/// 0x1E021C0 (`big` false: red, 30 square growing by 5 below 100, fading
/// out by 9) / 0x1E029C0 (true: dark red, 70 x 140 growing by 5 below 140
/// (height twice the width), out by 5) (D, task): a quad turned by the
/// camera's angles; drift (-50..49) / 100 unused; in by 0x40 under 0x80
/// for 10 updates.
fn quad(fx: &mut Fx, t: u32, big: bool) {
    let o = if big { 0x800 } else { 0 };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, if big { 0x46 } else { 0x1E });
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, if big { w << 1 } else { w });
        let v = fx.rand_from(t, 0x01E0_21F0 + o + if big { 4 } else { 0 });
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h);
        if big {
            l2::rgb(fx, t + 0x60, 0x80, 0x40, 0x40);
        } else {
            l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        }
        fx.mem.set16(t + 0x58, 2);
        let p = if big { 0x01E0_2A28u32 } else { 0x01E0_2224 };
        drift(fx, t, [p, p + 0x2C, p + 0x58], Some(100.0));
        let q = if big { 0x01E0_2AD8u32 } else { 0x01E0_22D4 };
        let v = rmod(fx, t, q, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), 0, q + 0x1C);
        spins(fx, t, [q + 0x24, q + 0x44, q + 0x64, q + 0x84]);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else if big {
        fade_out(fx, t, 0x63, 7, 5);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let w = fx.mem.i16(t + 8);
        let cap = if big { 0x8C } else { 0x64 };
        if w < cap {
            fx.mem.set16(t + 8, w + 5);
        }
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, if big { w << 1 } else { w });
    }
    let ra = if big { 0x01E0_2C00 } else { 0x01E0_23F8 };
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, ra);
    age(fx, t);
}

/// 0x1E02420 (`grey` false: red 18 x 10, tall by 5 / 10 / 60) / 0x1E026F0
/// (true: grey 25 x 10, by 10 / 20 / 70) (D, task): a standing quad pair
/// (quad9) growing taller (below update 0x5A / 0x79 / after, 2 wider
/// after); drift (-50..49) / 100 unused; in by 0x40 under 0x80 for 10
/// updates, then out by 9; turned 0x7000 / 0x9000 by the facing.
fn pair(fx: &mut Fx, t: u32, grey: bool) {
    let o = if grey { 0x2D0 } else { 0 };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, if grey { 0x19 } else { 0x12 });
        fx.mem.set16(t + 0xA, 0xA);
        let v = fx.rand_from(t, 0x01E0_2458 + o);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h);
        if grey {
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        } else {
            l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        }
        fx.mem.set16(t + 0x58, 2);
        drift(fx, t, [0x01E0_248C + o, 0x01E0_24B8 + o, 0x01E0_24E4 + o], Some(100.0));
        let v = rmod(fx, t, 0x01E0_253C + o, 0x168);
        fx.mem.set16(t + 0x5A, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2558 + o);
        spins(fx, t, [0x01E0_2560 + o, 0x01E0_2580 + o, 0x01E0_25A0 + o, 0x01E0_25C0 + o]);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let n = fx.mem.i16(COUNTER);
        let h = fx.mem.i16(t + 0xA);
        let (a, b, c) = if grey { (0xA, 0x14, 0x46) } else { (5, 0xA, 0x3C) };
        if n < 0x5A {
            fx.mem.set16(t + 0xA, h + a);
        } else if n < 0x79 {
            fx.mem.set16(t + 0xA, h + b);
        } else {
            fx.mem.set16(t + 0xA, h + c);
            let w = fx.mem.i16(t + 8) + 2;
            fx.mem.set16(t + 8, w);
        }
    }
    fx.mem.write_u32(t + 0x24, 0);
    fx.mem.write_u32(t + 0x28, 0);
    let a: u32 = if fx.mem.i16(FACING) != 0 { 0x7000 } else { 0x9000 };
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02C20 (D, task): a white rising bar (0x1E04800): +6 up by 7 below
/// 0x82, +2 up by 2 (the task ends at 0xB5); from 0x3C both alphas down by
/// 9 (the first ends it below 11); random angles; +0x48 = 10 growing by 1.
fn bar(fx: &mut Fx, t: u32) {
    let h = fx.mem.i16(t + 6);
    if h < 0x82 {
        fx.mem.set16(t + 6, h + 7);
    }
    let y = fx.mem.i16(t + 2) + 2;
    fx.mem.set16(t + 2, y);
    l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    if fx.mem.i16(t + 2) >= 0xB5 {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 2, 5);
        fx.mem.set16(t, 0);
        fx.mem.set16(t + 4, 0);
        fx.mem.set16(t + 6, 0);
        angles(fx, t, [0x01E0_2CA0, 0x01E0_2CBC, 0x01E0_2CD8]);
        fx.mem.write_u8(t + 0x63, 0xC0);
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 2) >= 0x3C {
        fade_out(fx, t, 0x63, 0xB, 9);
        let a = fx.mem.read_u8(t + 0x67);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x67, a - 9);
        }
    }
    let w = ee::add(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, w);
}

/// 0x1E02D80 (D, task): an arc ring of radius 220, inner dark red, outer
/// grey; outer alpha in by 0x60 under 0xC0 for 14 updates, then out by 9;
/// turned by the camera's angles (the first + 0x4000).
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x435C_0000);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0x40);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xE {
        fade_in(fx, t, 0x67, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x67, 0xB, 9);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2E40);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E02E70 (D, task): sparks: radius 120 growing by (160..239) / 10 a
/// update, the inner at 0.2 of it; kind (rand >> 4) % 2: 0 pink on black,
/// 1 light pink on grey; alpha 0x80 for 18 updates, then down by 5.
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42F0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(120.0, 58.0));
        let v = fx.rand_from(t, 0x01E0_2EB0);
        fx.mem.set16(t + 0x50, (v >> 4) % 2);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xC0, 0x80, 0x80);
            l2::rgb(fx, t + 0x64, 0, 0, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xFF, 0xA0, 0xA0);
            l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        }
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 0x1E);
        let v = rmod(fx, t, 0x01E0_2F48, 0x50);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0xA0));
        let q = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, q);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        angles(fx, t, [0x01E0_2F90, 0x01E0_2FAC, 0x01E0_2FC8]);
    }
    if fx.mem.i16(t + 0x54) < 0x12 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, 7, 5);
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3E4C_CCCD), r));
    age(fx, t);
}

/// 0x1E03070 (D, task): camera slots 4 / 5 (0x1F7E20) from D+0xB1A0 (128,
/// 4, 0, 0; a point; 0, 10 rising by 5 below 100, w): below update 0xDD the
/// point is node 8 + 1.5 (node 8 - node 3); below 0xFB w up by 0.05 to 1,
/// then the second value 4 / 5 on odd / even updates; from 0xFB w down by
/// 0.1 (the task ends at 0.1, or with the timer at 1); slots cleared when
/// it ends.
fn camera(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0), (0xC, 0), (0x20, 0), (0x24, 0x4120_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.set16(t + 0x50, 0);
    }
    if fx.mem.i16(COUNTER) < 0xDD {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 0xC);
        node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_3138);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2);
        node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_3158);
        for k in 0..3 {
            let d = ee::mul(1.5, ee::sub(fx.mem.f(t + 0x10 + 4 * k), fx.mem.f(sp + 0x30 + 4 * k)));
            fx.mem.setf(sp + 0x30 + 4 * k, d);
        }
        for k in 0..3 {
            let p = ee::add(fx.mem.f(t + 0x10 + 4 * k), fx.mem.f(sp + 0x30 + 4 * k));
            fx.mem.setf(CAM + 0x10 + 4 * k, p);
        }
        let v = fx.mem.f(CAM + 0x24);
        if ee_lt(v, 100.0) {
            fx.mem.setf(CAM + 0x24, ee::add(v, 5.0));
        }
    }
    let w = fx.mem.f(CAM + 0x28);
    if fx.mem.i16(COUNTER) < 0xFB {
        if ee_lt(w, 1.0) {
            fx.mem.setf(CAM + 0x28, ee::add(w, f32::from_bits(0x3D4C_CCCD)));
        } else if fx.mem.i16(t + 0x54) % 2 == 1 {
            fx.mem.write_u32(CAM + 4, 0x4080_0000);
        } else {
            fx.mem.write_u32(CAM + 4, 0x40A0_0000);
        }
    } else if ee_le(w, f32::from_bits(0x3DCC_CCCD)) {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, f32::from_bits(0x3DCC_CCCD)));
    }
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x58, 0);
    }
    let (p, ra) = if fx.mem.i16(t + 0x58) == 0 { (0, 0x01E0_334C) } else { (CAM, 0x01E0_3360) };
    crate::blib::exe::camera_slots45(fx, p);
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    age(fx, t);
}

/// 0x1E03390 (D, task): the stage light down by 10 to 0x20 for 140
/// updates, by 2 to 0x10 to 190, up by 3 to 0x30 to 210, then back up by
/// 10 to 0x80 once the timer is under 30; 0x80 and the task's end when the
/// timer is at 3.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.i16(t + 0x54);
    let l = fx.mem.i16(t + 0x50);
    if a < 0x8C {
        fx.mem.set16(t + 0x50, if l < 0x21 { 0x20 } else { l - 0xA });
    } else if a < 0xBE {
        fx.mem.set16(t + 0x50, if l < 0x11 { 0x10 } else { l - 2 });
    } else if a < 0xD2 {
        fx.mem.set16(t + 0x50, if l < 0x30 { l + 3 } else { 0x30 });
    } else if fx.mem.i32(TIMER) < 0x1E {
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    age(fx, t);
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
}

/// 0x1E03510 (D, task): a frame flash (the frame over the window, ST 0..1,
/// ALPHA 0x44, 0x207EF0 filter 1) in (0x98, 0x80, 0x80): in by 0x14 under
/// 0x50 while the timer is at least 21, then out by 9.
fn flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0x98, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let c = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x48 + k, c);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    fx.mem.write_u32(sp + 0x50, 0);
    fx.mem.write_u32(sp + 0x54, 0);
    fx.mem.write_u32(sp + 0x60, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x64, 0x3F80_0000);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i32(TIMER) >= 0x15 {
        fade_in(fx, t, 0x63, 0x50, 0x14);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    age(fx, t);
}

/// 0x1E04800 (1e1d6fb0b24a) `bar_draw(D, task)`: a textured strip3
/// (0x109B50, colours +0x60 top / +0x64 bottom, UVs D+0x94 in D+0xBE's
/// order, texture D+0x88) from the task's i16 point (+0, +2, +4) +0x48
/// wide, down by i16 +6 (y clamped to 0..300), through TRS(the task's
/// angles +0x20, place +0x10) x the camera; keyed by the mean w into
/// D+0x80.
fn bar_draw(fx: &mut Fx, d0: u32, t: u32) {
    let sp = fx.enter(0x190);
    let d = sp + 0xE0;
    l2::desc_strip3(fx, d);
    for (local, c, to) in [(0x170u32, 0x60u32, 0x10u32), (0x178, 0x60, 0x28), (0x180, 0x64, 0x40), (0x188, 0x64, 0x58)] {
        l2::rgbaq(fx, sp + local, t + c);
        let (a, b) = (fx.mem.read_u32(sp + local), fx.mem.read_u32(sp + local + 4));
        fx.mem.write_u32(d + to, a);
        fx.mem.write_u32(d + to + 4, b);
    }
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let tx = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tx, 1);
    for k in 0..3 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(sp + 0xC0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x80, 0, t + 0x20, sp + 0xC0);
    let p = |fx: &mut Fx, o: u32| ee::from_i32(fx.mem.i16(t + o));
    for k in 0..3u32 {
        let v = p(fx, 2 * k);
        fx.mem.setf(sp + 0x40 + 4 * k, v);
    }
    for k in 0..4u32 {
        let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
        fx.mem.write_u32(sp + 0x50 + 4 * k, w);
    }
    let x = ee::add(fx.mem.f(sp + 0x50), fx.mem.f(t + 0x48));
    fx.mem.setf(sp + 0x50, x);
    for k in 0..3u32 {
        let v = p(fx, 2 * k);
        fx.mem.setf(sp + 0x60 + 4 * k, v);
    }
    let mut y = ee::sub(fx.mem.f(sp + 0x64), p(fx, 6));
    fx.mem.setf(sp + 0x64, y);
    if ee_lt(y, 0.0) {
        y = 0.0;
        fx.mem.write_u32(sp + 0x64, 0);
    }
    let _ = y;
    for k in 0..4u32 {
        let w = fx.mem.read_u32(sp + 0x60 + 4 * k);
        fx.mem.write_u32(sp + 0x70 + 4 * k, w);
    }
    let x = ee::add(fx.mem.f(sp + 0x70), fx.mem.f(t + 0x48));
    fx.mem.setf(sp + 0x70, x);
    for o in [0x44u32, 0x54, 0x64, 0x74] {
        if !ee_le(fx.mem.f(sp + o), 300.0) {
            fx.mem.write_u32(sp + o, 0x4396_0000);
        }
    }
    for o in [0x44u32, 0x54, 0x64, 0x74] {
        if ee_lt(fx.mem.f(sp + o), 0.0) {
            fx.mem.write_u32(sp + o, 0);
        }
    }
    if l2::project(fx, sp + 0x40, SPR, sp + 0x80, d, sp + 0xD0) {
        let key = ee::fixunssfsi(fx.mem.f(sp + 0xD0)) as i32;
        let ot = fx.mem.read_u32(d0 + 0x80);
        fx.submit(d, ot, key);
    }
    fx.leave(0x190);
}
