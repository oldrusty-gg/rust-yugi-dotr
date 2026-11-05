//! BMODULE b757 (card 223), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02EE0, data area 0x1ED0000) [F]: b065's program shape (b590's layout: 400 tasks of 0x70 bytes at
//! D+0x2A0, alive +0x58, step +0x44 through `jalr`, 0x1E02ED0 the idle
//! step), spawned by 0x1E03770 (a2350b28c0ee, record-local here), drawn
//! with rings of quads (7369422df7f8 at 0x1E00920,
//! `billboard::ring_quads`), arcs (e6dce326551d at 0x1E00ED0,
//! `billboard::arc_strips`), textured arcs (3a2db45c5a9a at 0x1E014F0,
//! `quads4::arc_strips_tex`) and single quads (add5d81dd3fe at 0x1E01230,
//! `billboard::quad`). Update 1: the stage dimmed (0x1E02BC0) and camera
//! slots 4 / 5 (0x1E029A0); to update 220 dark smoke rings at node 0x24
//! every update; updates 68 / 110 / 127 / 144 / 192 and 118 / 136 / 192:
//! thirteen white motes at nodes 0xE / 0x3F; 161 a frame flash; 195..220
//! at node 0x23's point (4 toward the camera side) textured arcs, grey
//! rings and on 195 a widening blue band. Returns 1 on update 181, 4 on
//! 265, 2 on 240, 6 on 192..234, 8 once the 270-update timer D+0xB210 has
//! run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const SMOKE: u32 = 0x01E0_1E00;
const ARC_T: u32 = 0x01E0_1FD0;
const MOTE: u32 = 0x01E0_24A0;
const RING: u32 = 0x01E0_2670;
const BAND: u32 = 0x01E0_2880;
const CAMERA: u32 = 0x01E0_29A0;
const LIGHT: u32 = 0x01E0_2BC0;
const FLASH: u32 = 0x01E0_2CD0;
const IDLE: u32 = 0x01E0_2ED0;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3298;
const TIMER: u32 = D + 0xB210;
const COLOUR: u32 = D + 0xB214;
const NODE_A: u32 = D + 0xB244;
const NODE_B: u32 = D + 0xB246;
const NODE_C: u32 = D + 0xB248;
const NODE_D: u32 = D + 0xB24A;
const SPAWN_K: u32 = D + 0xB2B0;
const FACING: u32 = D + 0xB2B4;
const COUNTER: u32 = D + 0xB2B8;
const BYTES_A: u32 = D + 0xB2BA;
const BYTES_B: u32 = D + 0xB3BA;
const TAB: u32 = D + 0xB4BC;
const CAM: u32 = D + 0xB1A0;
/// Node 0x23's point on update 195 (4 toward the camera side).
const AIM: u32 = D + 0xB1D0;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02EE0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x10E);
    for (k, node) in [0xE, 0x3F, 0x23, 0x24, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE_A + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0xB1F0 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_30D0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3100, 0x1E);
        fx.mem.set8(BYTES_A + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3154, 0x1E);
        fx.mem.set8(BYTES_B + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, k, 0x01E0_31AC, 0x1E);
        fx.mem.set8(BYTES_B + k, v);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_31FC);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 || f == IDLE {
            continue;
        }
        // The step's prologue: ra, the entry's s1 = D, s0 = 0.
        let frame = match f {
            ARC_T | MOTE => 0x30,
            CAMERA => 0x50,
            FLASH => 0x90,
            _ => 0x20,
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        }
        match f {
            SMOKE => smoke(fx, t),
            ARC_T => arc_t(fx, t),
            MOTE => mote(fx, t),
            RING => ring(fx, t),
            BAND => band(fx, t),
            CAMERA => camera(fx, t, s),
            LIGHT => light(fx, t),
            FLASH => flash(fx, t, s),
            _ => panic!("b757: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    // The entry's s0: the live tasks counted in the draw loop.
    let mut s0 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        s0 += 1;
        draw(fx, sp, t, s0);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xB5 => 1,
        0x109 => 4,
        0xF0 => 2,
        0xC0..=0xEA => 6,
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

/// D+0x94.. = a 32 x 64 cell at column +0x56 of the texture D+0xB1F4.
fn strip_cell(fx: &mut Fx, t: u32) {
    let c = fx.mem.i16(t + 0x56);
    for (o, v) in [(0x94u32, c << 5), (0x96, 0), (0x98, 0x20), (0x9A, 0x40)] {
        fx.mem.set16(D + o, v);
    }
}

fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    match f {
        SMOKE | RING => {
            let (local, n, ra, table) = if f == SMOKE { (0x50, 2, 0x01E0_3364, BYTES_A) } else { (0x90, 4, 0x01E0_3608, BYTES_B) };
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + local, 0x14, c);
            let w = fx.mem.read_u32(D + 0xB1F4);
            fx.mem.write_u32(D + 0x88, w);
            strip_cell(fx, t);
            fx.mem.set16(D + 0xB8, n);
            let v = fx.rand_from(s0, ra);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, table);
            fx.mem.set16(D + 0xB8, 0);
        }
        ARC_T => {
            let kind = |fx: &mut Fx| fx.mem.i16(t + 0x50);
            if kind(fx) == 0 {
                let c = fx.mem.i16(t + 0x56);
                cell(fx, sp + 0x60, 0x97, c);
                let w = fx.mem.read_u32(D + 0x29C);
                fx.mem.write_u32(D + 0x88, w);
            }
            if kind(fx) == 1 {
                let c = fx.mem.i16(t + 0x56);
                cell(fx, sp + 0x70, 0x9B, c);
                let w = fx.mem.read_u32(D + 0x29C);
                fx.mem.write_u32(D + 0x88, w);
            }
            if kind(fx) == 2 {
                let w = fx.mem.read_u32(D + 0xB1F4);
                fx.mem.write_u32(D + 0x88, w);
                fx.mem.set16(D + 0xBE, 0);
                strip_cell(fx, t);
            }
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, 0x24);
            if kind(fx) == 0 {
                fx.mem.set16(D + 0xB8, 0x18);
            }
            if kind(fx) == 2 {
                fx.mem.set16(D + 0xB8, 6);
            }
            fx.mem.set16(D + 0xBE, 0);
            quads4::arc_strips_tex(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        MOTE => {
            let w = fx.mem.read_u32(D + 0xB1F0);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x14, c);
            fx.mem.set16(D + 0xBE, 0);
            billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
        }
        BAND => {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, AIM);
            fx.mem.set16(D + 0xB8, 0);
        }
        _ => {}
    }
}

/// 0x1E03770(D, step, arg): the first free task cleared and set off at
/// D+0x250 (i16 +0, float +0x10) with the colour D+0xB214 (twice), index
/// D+0xB2B0 (+0x50).
fn spawn(fx: &mut Fx, step: u32) {
    fx.enter(0x60);
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
        let s = fx.mem.i16(SPAWN_K);
        fx.mem.set16(t + 0x50, s);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

fn node_pos(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(at);
    fx.node_position(h, slot, out);
}

/// 0x1E01900(D, 0): the update counter and the emitters (frame 0x30: ra,
/// the entry's s1 = D, s0 = 0).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3258), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 1 {
        spawn(fx, LIGHT);
    }
    if c(fx) == 1 {
        spawn(fx, CAMERA);
    }
    if c(fx) == 0xA1 {
        spawn(fx, FLASH);
    }
    for (at, ups) in [(NODE_A, &[0x44, 0x6E, 0x7F, 0x90, 0xC0][..]), (NODE_B, &[0x76, 0x88, 0xC0][..])] {
        if ups.contains(&c(fx)) {
            node_pos(fx, at, POS);
            let y = ee::sub(fx.mem.f(POS + 4), 1.5);
            fx.mem.setf(POS + 4, y);
            for _ in 0..0xD {
                spawn(fx, MOTE);
            }
        }
    }
    let toward = |fx: &mut Fx, at: u32, by: f32| {
        let z = fx.mem.f(at);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, by) } else { ee::add(z, by) };
        fx.mem.setf(at, z);
    };
    if c(fx) == 0xC3 {
        node_pos(fx, NODE_C, AIM);
        toward(fx, AIM + 8, 4.0);
    }
    if (0xC3..0xDD).contains(&c(fx)) {
        let h = fx.target_handle();
        fx.unit_position(h, POS);
        let y = ee::sub(fx.mem.f(POS + 4), 20.0);
        fx.mem.setf(POS + 4, y);
        toward(fx, POS + 8, 5.0);
        for k in 0..3 {
            let w = fx.mem.read_u32(AIM + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        fx.mem.set16(SPAWN_K, 0);
        if c(fx) % 2 == 1 {
            spawn(fx, ARC_T);
        }
        fx.mem.set16(SPAWN_K, 1);
        if c(fx) % 2 == 1 {
            spawn(fx, ARC_T);
        }
        fx.mem.set16(SPAWN_K, 2);
        if c(fx) % 2 == 0 {
            spawn(fx, ARC_T);
        }
        spawn(fx, RING);
        if c(fx) == 0xC3 {
            spawn(fx, BAND);
        }
    }
    if c(fx) < 0xDD {
        node_pos(fx, NODE_D, POS);
        spawn(fx, SMOKE);
    }
    fx.leave(0x30);
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends
/// (`end`) or the byte is cleared.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8, end: bool) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else if end {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.write_u8(t + at, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

fn young_fade(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x15, 0x13, true);
    }
}

/// 0x1E01E00 (D, task): a dark smoke ring (size 80..119 growing by 1, cell
/// 0..2), radius 5..14 growing by 5..14, life 5..12.
fn smoke(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_1E24, 0x28);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x50));
        let v = rmod(fx, t, 0x01E0_1E50, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x18, 0x18, 0x18);
        let v = rmod(fx, t, 0x01E0_1E7C, 0xA);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, 0x01E0_1E9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_1EB8, 0xA);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rmod(fx, t, 0x01E0_1ED8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, 0x01E0_1EF4);
        fx.mem.set16(t + 6, v % 8 + 5);
        fx.mem.set16(t + 0x58, 2);
    }
    young_fade(fx, t);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x48, 1.0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02670 (D, task): a grey ring (size 80..139 growing by 1, cell
/// 0..2), radius 5..14 growing by 20..49, turning 2 either way, life
/// 9..16.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2694, 0x3C);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x50));
        let v = rmod(fx, t, 0x01E0_26C0, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x50, 0x50, 0x50);
        let v = rmod(fx, t, 0x01E0_26EC, 0xA);
        fx.mem.set16(t + 0x50, v + 5);
        let v = fx.rand_from(t, 0x01E0_270C);
        fx.mem.set16(t + 0x52, v % 2);
        let v = rmod(fx, t, 0x01E0_2728, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2744, 0x1E);
        fx.mem.set16(t + 0xE, v + 0x14);
        let v = rmod(fx, t, 0x01E0_2764, 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, 0x01E0_2780);
        fx.mem.set16(t + 6, v % 8 + 9);
        fx.mem.set16(t + 0x58, 2);
    }
    young_fade(fx, t);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x48, 1.0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let a = fx.mem.i16(t + 0x5A) + if fx.mem.i16(t + 0x52) == 0 { 2 } else { -2 };
    fx.mem.set16(t + 0x5A, a);
    age(fx, t);
}

/// 0x1E01FD0 (D, task): a textured arc (radius 20 growing by 35..42.9, the
/// inner +0x4C a share of it), three kinds by +0x50 (kind 0 spinning
/// +0x24 by 0x80 either way); 0x80, out by 0xB.
fn arc_t(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(20.0, 58.0));
        let odd = |fx: &mut Fx| fx.mem.i16(COUNTER) % 2 == 1;
        let colours: [((i32, i32, i32), (i32, i32, i32)); 3] = [((0x80, 0xC0, 0x80), (0x80, 0xFF, 0x80)), ((0x60, 0x80, 0x60), (0x40, 0xFF, 0x40)), ((0x50, 0x50, 0x50), (0x60, 0x60, 0x60))];
        let dark = [8, 8, 0x28];
        let light = [0x38, 0x38, 0x30];
        for kind in 0..3 {
            if fx.mem.i16(t + 0x50) == kind {
                let (even_c, odd_c) = colours[kind as usize];
                if !odd(fx) {
                    l2::rgb(fx, t + 0x64, even_c.0, even_c.1, even_c.2);
                    let g = dark[kind as usize];
                    l2::rgb(fx, t + 0x60, g, g, g);
                }
                if odd(fx) {
                    l2::rgb(fx, t + 0x64, odd_c.0, odd_c.1, odd_c.2);
                    let g = light[kind as usize];
                    l2::rgb(fx, t + 0x60, g, g, g);
                }
                if kind == 1 {
                    fx.mem.write_u32(t + 0x34, 0x4120_0000);
                }
                if kind == 2 {
                    fx.mem.write_u32(t + 0x34, 0x428C_0000);
                }
                fx.mem.set16(t + 6, if kind == 0 { 8 } else { 0xA });
            }
        }
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.set16(t + 0x5A, 0xF);
            fx.mem.set16(t + 0x5C, 0xF);
            let v = fx.rand_from(t, 0x01E0_2270);
            fx.mem.set16(t + 0x52, (v >> 4) % 2);
        }
        for (kind, a, b) in [(1, 0xA, 3), (2, 0x3C, 0x3C)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.set16(t + 0x5A, a);
                fx.mem.set16(t + 0x5C, b);
            }
        }
        let v = rmod(fx, t, 0x01E0_22D0, 0x50);
        fx.mem.setf(t + 0x38, ee::from_i32(v + 0x15E));
        let v = ee::div(fx.mem.f(t + 0x38), 10.0);
        fx.mem.setf(t + 0x38, v);
        fx.mem.write_u32(t + 0x30, 0x43AF_0000);
        let v = ee::div(fx.mem.f(t + 0x30), 10.0);
        fx.mem.setf(t + 0x30, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, 0x01E0_232C, 0xFFFF);
        fx.mem.set32(t + 0x24, v);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x67, 0x80);
    } else {
        fade_out(fx, t, 0x67, 0xD, 0xB, true);
    }
    for (kind, share) in [(0, 0x3E4C_CCCDu32), (1, 0x3F33_3333), (2, 0x3F33_3333)] {
        if fx.mem.i16(t + 0x50) == kind {
            let s = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x38));
            fx.mem.setf(t + 0x48, s);
            fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(share), s));
            if kind == 0 {
                let by = if fx.mem.i16(t + 0x52) == 0 { 0x80 } else { -0x80 };
                let a = fx.mem.i32(t + 0x24).wrapping_add(by);
                fx.mem.set32(t + 0x24, a);
            } else {
                let v = fx.mem.f(t + 0x38);
                add(fx, t + 0x34, v);
            }
        }
    }
    age(fx, t);
}

/// 0x1E024A0 (D, task): a white mote (size 100, cell 2..4) thrown 6.6 out
/// at a random angle, then drifting 0.5 a update; in by 0x40 for 10
/// updates, out by 9 (b065's 0x1E02240).
fn mote(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x64);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_24D8);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2510);
        let r = fx.div(v >> 4, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        drift(fx, t, f32::from_bits(0x40D3_3333));
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        drift(fx, t, 0.5);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2640);
    age(fx, t);
}

/// z += `by` sin +0x5A, x += `by` cos +0x5A.
fn drift(fx: &mut Fx, t: u32, by: f32) {
    let a = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, TAB, a, 0);
    add(fx, t + 0x18, ee::mul(by, s));
    let a = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, TAB, a, 1);
    add(fx, t + 0x10, ee::mul(by, c));
}

/// 0x1E02880 (D, task): a grey / blue band (radius 200 + 0.4 a update)
/// facing the camera turned 0x4000; in by 0x30 under 0xC0 for 34 updates,
/// out by 0xF.
fn band(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4348_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x22 {
        fade_in(fx, t, 0x67, 0xC0, 0x30);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF, true);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2940);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x4C, f32::from_bits(0x3ECC_CCCD));
    }
    age(fx, t);
}

/// 0x1E029A0 (D, task): camera slots 4 / 5 (0x1F7E20) from D+0xB1A0 (128,
/// 100, 100, 100; node 0x23's point; 0, 123, w rising by 0.3 to 1 for 210
/// updates, then down by 0.3); cleared when the task ends (w at 0.1, or
/// the timer at 1).
fn camera(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x42C8_0000), (8, 0x42C8_0000), (0xC, 0x42C8_0000), (0x20, 0), (0x24, 0x42F6_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        for k in 0..3 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(CAM + 0x10 + 4 * k, w);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.set16(t + 0x50, 0);
    }
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_C);
    crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_2A80);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
        fx.mem.write_u32(CAM + 0x10 + 4 * k, w);
    }
    let w = fx.mem.f(CAM + 0x28);
    let step = f32::from_bits(0x3E99_999A);
    if fx.mem.i16(t + 0x54) < 0xD2 {
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, step));
        }
    } else if w <= f32::from_bits(0x3DCC_CCCD) {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, step));
    }
    let (p, ra) = if fx.mem.i16(t + 0x58) == 0 { (0, 0x01E0_2B6C) } else { (CAM, 0x01E0_2B80) };
    crate::blib::exe::camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
}

/// 0x1E02BC0 (D, task): the stage light (0x1F7F10) down by 10 to 0x30 for
/// 151 updates, back up by 10 to 0x80 once the timer is under 30; 0x80 and
/// the task's end when the timer is at 3.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x97 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x1E {
        let l = fx.mem.i16(t + 0x50);
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

/// 0x1E02CD0 (D, task): a frame flash (b590's 0x1E02450: grey 0x80 / 0x80
/// / 0x90, ST from +0x48 = 2.5): in by 0x16 under 0x58 until update 250,
/// then out by 4.
fn flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x90);
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
    if fx.mem.i16(COUNTER) < 0xFA {
        fade_in(fx, t, 0x63, 0x58, 0x16);
    } else {
        fade_out(fx, t, 0x63, 6, 4, true);
    }
    age(fx, t);
}
