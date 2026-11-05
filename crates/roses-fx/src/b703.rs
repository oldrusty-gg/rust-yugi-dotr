//! BMODULE b703 (card 135), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03440, data area 0x1ED0000, 0xCF30 bytes) [F]
//!: b590's task pool (400 tasks of 0x70 bytes at
//! D+0x2A0, alive +0x58, step +0x44 through `jalr`, 0x1E03430 the idle
//! step) with every global after the pool 0x1900 higher (four 20-point
//! node histories D+0xB1D0 / +0xB310 / +0xB450 / +0xB590 in between);
//! spawned by 0x1E03C70 (b590's spawn on the shifted globals) and drawn
//! with the second library: rings of quads (7369422df7f8 at 0x1E009B0),
//! arcs (e6dce326551d at 0x1E00F60), quads (add5d81dd3fe at 0x1E012C0),
//! light columns (792ca1ad9378 at 0x1E019C0), tints (561a6131a635 at
//! 0x1E01D30) and the record's own gouraud ribbon between two histories
//! (0x1E03E20, [`ribbon`], on `spline::spline` 0x1E01580). Update 1 the
//! stage dimmed (0x1E03320) and four columns pulled along the attacker's
//! node line 0x16 -> 0x17; from 10 the four histories of nodes 0x16 /
//! 0x17 / 0x36 / 0x3E (drawn as a violet ribbon three times); 0x76..0x96
//! a ring a update at node 0x17; on 0x5B / 0x66 / 0x77 thirteen motes at
//! node 0x36; on 0x7B at the target an arc, four columns, twenty rings and
//! a blue tint. Returns 1 on update 0x70, 4 on 0xAA, 2 on 0x91, 6 on
//! 0x7B..0x8B, 8 once the 0xB4-update timer D+0xCB10 has run out.
use crate::blib::{billboard, quads4, spawn as hist, spline, uv};
use crate::bus::Bus;
use crate::c061::rgbaq_at;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const RING_A: u32 = 0x01E0_2370;
const RING_B: u32 = 0x01E0_2520;
const TINT: u32 = 0x01E0_26B0;
const COLUMN: u32 = 0x01E0_2840;
const MOTE: u32 = 0x01E0_2A00;
const PULLED: u32 = 0x01E0_2BD0;
const TRAILS: u32 = 0x01E0_2E70;
const ARC: u32 = 0x01E0_3200;
const LIGHT: u32 = 0x01E0_3320;
const IDLE: u32 = 0x01E0_3430;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3750;
/// The four node histories (20 points of 16 bytes).
const HIST: [u32; 4] = [D + 0xB1D0, D + 0xB310, D + 0xB450, D + 0xB590];
const TEX: u32 = D + 0xCAF0;
const TIMER: u32 = D + 0xCB10;
const COLOUR: u32 = D + 0xCB14;
const TINT_W: u32 = D + 0xCB34;
const NODES: u32 = D + 0xCB48;
const SPAWN_A: u32 = D + 0xCBA6;
const SPAWN_K: u32 = D + 0xCBB4;
const FACING: u32 = D + 0xCBB8;
const COUNTER: u32 = D + 0xCBBC;
const BYTES: u32 = D + 0xCBBE;
const TAB: u32 = D + 0xCDC0;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn node(fx: &mut Fx, k: u32) -> i32 {
    fx.mem.i16(NODES + 2 * k)
}

/// The entry 0x1E03440 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    fx.mem.write_u32(D + 0xCB1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xCF30);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xCBBA, param);
    fx.mem.set32(TIMER, 0xB4);
    for (k, n) in [0x16, 0x17, 0x36, 0x3E, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xCB58, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_3634);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3664, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_36B4);
        fx.mem.set16(D + 0xCB68 + 2 * k, v % 8);
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
            MOTE => 0x30,
            TRAILS => 0x40,
            PULLED => 0x50,
            _ => 0x20,
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        }
        match f {
            RING_A => ring(fx, t, RING_A, (4, 0x28, 0xA, 0x14, 0x10, 7, 8), true),
            RING_B => ring(fx, t, RING_B, (0xA, 0x28, 0xA, 0x14, 0x10, 3, 3), false),
            TINT => tint(fx, t),
            COLUMN => column(fx, t),
            MOTE => mote(fx, t),
            PULLED => pulled(fx, t, s),
            TRAILS => trails(fx, t, s),
            ARC => arc(fx, t),
            LIGHT => light(fx, t),
            _ => panic!("b703: unknown task step {f:#x}"),
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
        draw(fx, sp, t, s0, k);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x70 => 1,
        0xAA => 4,
        0x91 => 2,
        0x7B..=0x8B => 6,
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

fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32, k: u32) {
    let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
    for (g, local, n, site) in [(RING_A, 0x70u32, 5, 0x01E0_3800u32), (RING_B, 0x80, 4, 0x01E0_38A0)] {
        if f(fx) == g {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + local, 0x14, c);
            fx.mem.set16(D + 0xB8, n);
            let v = fx.rand_from(s0, site);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    if f(fx) == TINT {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x60);
        // 0x1F71A0's frame below the entry's (its saved ra, s0).
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_3900), (0, u64::from(s0))]);
        let z = fx.mem.f(sp + 0x68);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 50.0) } else { ee::add(z, 50.0) };
        fx.mem.setf(sp + 0x68, z);
        let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
        let mut x = fx.mem.read_u64(sp + 0xE8);
        if let Some(w) = fx.project(p, None, &mut x) {
            fx.mem.write_u64(sp + 0xE8, x);
            fx.mem.setf(TINT_W, w);
        }
        crate::b590::project_residue(fx, sp, sp + 0x60);
        if fx.mem.i16(t + 0x50) == 0 {
            let w = fx.mem.f(TINT_W);
            quads4::tint4(fx, t, 0x20, w);
        }
    }
    for (g, local) in [(COLUMN, 0x90u32), (MOTE, 0xA0), (PULLED, 0xB0), (TRAILS, 0), (ARC, 0)] {
        if f(fx) != g {
            continue;
        }
        match g {
            MOTE => {
                let w = fx.mem.read_u32(TEX);
                fx.mem.write_u32(D + 0x88, w);
                let c = fx.mem.i16(t + 0x56);
                cell(fx, sp + local, 0x14, c);
                fx.mem.set16(D + 0xBE, 0);
                billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
            }
            TRAILS => {
                for ra in [0x01E0_3B10u32, 0x01E0_3B2C, 0x01E0_3B48] {
                    let saves = [u64::from(s0), u64::from(D), u64::from(t - 0x2A0), u64::from(sp + 0xE8), u64::from(k), u64::from(ra)];
                    ribbon(fx, t, HIST[0], HIST[1], saves);
                }
            }
            ARC => {
                fx.mem.set16(D + 0xB8, 0x24);
                billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
                fx.mem.set16(D + 0xB8, 0);
            }
            _ => {
                let w = fx.mem.read_u32(D + 0x29C);
                fx.mem.write_u32(D + 0x88, w);
                cell(fx, sp + local, 0x9A, 0);
                fx.mem.set16(D + 0xBE, 0);
                quads4::sprite_quads(fx, D, t, 0, t + 0x10);
            }
        }
    }
}

/// 0x1E03C70(D, step, 0): [`crate::b590::spawn`] on the shifted globals
/// (colour D+0xCB14, kind D+0xCBB4, angle D+0xCBA6), its frame's saves
/// (s0 = D +0, the emitter's s1 +0x10, return address `ra` +0x50) left.
fn spawn(fx: &mut Fx, step: u32, s1: u32, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, u64::from(s1)), (0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        Some(k) => {
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
            let s = fx.mem.i16(SPAWN_K);
            fx.mem.set16(t + 0x52, s);
            let a = fx.mem.i16(SPAWN_A);
            fx.mem.set16(t + 0x5A, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

fn node_pos(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let slot = node(fx, k);
    fx.node_position(h, slot, out);
}

/// Four columns (angles 0 / 90 kind 0, 45 / 135 kind 1) of step `step`.
fn columns(fx: &mut Fx, step: u32, ras: [u32; 4]) {
    for ((a, k), ra) in [(0, 0), (0x5A, 0), (0x2D, 1), (0x87, 1)].into_iter().zip(ras) {
        fx.mem.set16(SPAWN_A, a);
        fx.mem.set16(SPAWN_K, k);
        spawn(fx, step, D, ra);
    }
}

/// 0x1E01F00(D, 0): the update counter and the emitters (frame 0x30: ra,
/// the entry's s1 = D, s0 = 0).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3710), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    // The emitter's s1: the entry's D until a loop counts in it.
    let mut s1 = D;
    if c(fx) == 1 {
        spawn(fx, LIGHT, D, 0x01E0_1F54);
    }
    if c(fx) == 0xA {
        spawn(fx, TRAILS, D, 0x01E0_1F80);
    }
    if c(fx) == 0x7B {
        let h = fx.target_handle();
        fx.unit_position(h, POS);
        let y = ee::sub(fx.mem.f(POS + 4), 5.0);
        fx.mem.setf(POS + 4, y);
        spawn(fx, ARC, D, 0x01E0_1FD4);
        columns(fx, COLUMN, [0x01E0_2000, 0x01E0_2030, 0x01E0_2064, 0x01E0_2098]);
        for k in 0..0x14 {
            spawn(fx, RING_A, k, 0x01E0_20B4);
        }
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, TINT, 0x14, 0x01E0_20F4);
        s1 = 0x14;
    }
    if c(fx) == 1 {
        columns(fx, PULLED, [0x01E0_2138, 0x01E0_2168, 0x01E0_219C, 0x01E0_21D0]);
    }
    if (0x76..0x97).contains(&c(fx)) {
        node_pos(fx, 1, POS);
        spawn(fx, RING_B, s1, 0x01E0_2228);
    }
    for (hits, ra) in [([0x5B, 0x77], 0x01E0_229Cu32), ([0x66, 0x77], 0x01E0_2334)] {
        if hits.contains(&c(fx)) {
            node_pos(fx, 2, POS);
            let y = ee::sub(fx.mem.f(POS + 4), 1.5);
            fx.mem.setf(POS + 4, y);
            for k in 0..0xD {
                spawn(fx, MOTE, k, ra);
            }
        }
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

/// 0x1E02370 / 0x1E02520 (`f`) (D, task): b590's grey ring with new
/// constants `k` = (size base over rand % 3, radius m / base, growth m /
/// base, life m / base): 0x2370 size 4..6, radius 10..49 growing by
/// 16..35, life 8..14; 0x2520 size 10..12, life 3..5 and its radius
/// fixed (`grows` false).
fn ring(fx: &mut Fx, t: u32, f: u32, k: (i32, i32, i32, i32, i32, i32, i32), grows: bool) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 3);
        fx.mem.setf(t + 0x48, ee::from_i32(v + k.0));
        let v = rmod(fx, t, f + 0x50, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x7C, k.1);
        fx.mem.set16(t + 0x50, v + k.2);
        let v = rmod(fx, t, f + 0x9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB8, k.3);
        fx.mem.set16(t + 0xE, v + k.4);
        let v = rmod(fx, t, f + 0xD8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xF4, k.5);
        fx.mem.set16(t + 6, v + k.6);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0x10 {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if grows && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E026B0 (D, task): b735's tint (kind 0 blue (0x60, 0x60, 0xFF), 1
/// (0, 0x30, 0xF0) / (0x30, 0, 0xF0), 8 updates; 2 grey blue, 22), in by
/// 0x26 under 0xC0, out by 9.
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0x60, 0x60, 0xFF);
            l2::rgb(fx, t + 0x64, 0x60, 0x60, 0xFF);
            fx.mem.set16(t + 6, 8);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0, 0x30, 0xF0);
            l2::rgb(fx, t + 0x64, 0x30, 0, 0xF0);
            fx.mem.set16(t + 6, 8);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            l2::rgb(fx, t + 0x60, 0x58, 0x58, 0x78);
            l2::rgb(fx, t + 0x64, 0x58, 0x58, 0x78);
            fx.mem.set16(t + 6, 0x16);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0x26);
        fade_in(fx, t, 0x67, 0xC0, 0x26);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
        fade_out(fx, t, 0x67, 0xB, 9, false);
    }
    age(fx, t);
}

/// 0x1E02840 (D, task): b745's light column white / blue, 5.5 wide (kinds
/// 0 / 1), 670 / 630 high growing by 30 / 10 (no width growth), turned
/// by 6 a update.
fn column(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0xFF);
        for (kind, w, h) in [(0, 0x40B0_0000u32, 0x4427_8000u32), (1, 0x40B0_0000, 0x441D_8000), (2, 0x4100_0000, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, w);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x52, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    for (kind, dh) in [(0, 30.0f32), (1, 10.0)] {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(t + 0x54) != 0 {
            add(fx, t + 0x34, dh);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let c = fx.mem.i16(t + 0x52);
        if c < 0x19 {
            fx.mem.set16(t + 0x52, c + 1);
        }
    }
    age(fx, t);
    let a = fx.mem.i16(t + 0x5A) + 6;
    fx.mem.set16(t + 0x5A, a);
}

/// 0x1E02A00 (D, task): b745's white mote (word for word).
fn mote(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x64);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2A38);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2A70);
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
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2BA0);
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

/// 0x1E02BD0 (D, task): b377's light column (word for word, its nodes
/// 0x16 -> 0x17): +0x10 node 0x16's point pulled toward node 0x17's by
/// +0x52 / 25.
fn pulled(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0xFF);
        for (kind, w, h) in [(0, 0x4040_0000u32, 0x428C_0000u32), (1, 0x4040_0000, 0x41F0_0000), (2, 0x4100_0000, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, w);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x52, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    for (kind, by) in [(0, 30.0f32), (1, 10.0)] {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(t + 0x54) != 0 {
            add(fx, t + 0x34, by);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let n = fx.mem.i16(t + 0x52);
        if n < 0x19 {
            fx.mem.set16(t + 0x52, n + 1);
        }
    }
    age(fx, t);
    let a = fx.mem.i16(t + 0x5A) + 6;
    fx.mem.set16(t + 0x5A, a);
    let by = ee::from_i32(fx.mem.i16(t + 0x52));
    let h = fx.self_handle();
    let slot = node(fx, 0);
    crate::b076::node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_2D98);
    let h = fx.self_handle();
    let slot = node(fx, 1);
    crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_2DB8);
    for k in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x30 + 4 * k), fx.mem.f(t + 0x10 + 4 * k)), 25.0);
        fx.mem.setf(sp + 0x30 + 4 * k, v);
    }
    for k in 0..3u32 {
        let v = ee::mul(fx.mem.f(sp + 0x30 + 4 * k), by);
        add(fx, t + 0x10 + 4 * k, v);
    }
}

/// 0x1E02E70 (D, task): the ribbon task: violet (0x60, 0x60, 0xC0) every
/// update; on its first update size 5, +0xE 30 and the four histories
/// filled (19 points) with nodes 0x16 / 0x17 / 0x36 / 0x3E; in by 0x3C
/// under 0xF0 to update 0x82, then out by 0x1D; to update 0x83 each
/// history takes its node's point (`spawn::history`, 20 points); +6 = 10.
fn trails(fx: &mut Fx, t: u32, sp: u32) {
    l2::rgb(fx, t + 0x60, 0x60, 0x60, 0xC0);
    let ras_fill = [0x01E0_2ED8u32, 0x01E0_2F44, 0x01E0_2FB4, 0x01E0_3024];
    let ras_step = [0x01E0_3108u32, 0x01E0_3140, 0x01E0_3178, 0x01E0_31B0];
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        fx.mem.set16(t + 0xE, 0x1E);
        for k in 0..4u32 {
            let h = fx.self_handle();
            let slot = node(fx, k);
            crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ras_fill[k as usize]);
            for e in 0..0x13u32 {
                l2::copy3(fx, HIST[k as usize] + 0x10 * e, sp + 0x30);
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) < 0x83 {
        fade_in(fx, t, 0x63, 0xF0, 0x3C);
    } else {
        fade_out(fx, t, 0x63, 0x1F, 0x1D, true);
    }
    if fx.mem.i16(COUNTER) < 0x84 {
        for k in 0..4u32 {
            let h = fx.self_handle();
            let slot = node(fx, k);
            crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ras_step[k as usize]);
            hist::history(fx, HIST[k as usize] + 0x130, 0x14, sp + 0x30);
        }
    }
    fx.mem.set16(t + 6, 0xA);
    age(fx, t);
}

/// 0x1E03200 (D, task): b590's arc with outer radius 150.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4316_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xE {
        fade_in(fx, t, 0x67, 0xC0, 0x30);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF, true);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_32C0);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x4C, f32::from_bits(0x3ECC_CCCD));
    }
    age(fx, t);
}

/// 0x1E03320 (D, task): b745's stage light: down by 10 to 0x30 for 151
/// updates, back up once the timer is under 30.
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

/// 0x1E03E20 (D, task, a, b): a gouraud ribbon between the point
/// histories `a` and `b` (b501's 0x1E047C0 recompiled with a 0x300 frame):
/// i16 +6 points, the first segment's point before is the first point;
/// per segment four strips (vertices a(t), a(t + 1/4), b(t), b(t + 1/4)
/// by `spline::spline` basis 0) at key 0 through the camera (matrix 0);
/// the `a` side coloured +0x60 with alpha 0, the `b` side +0x60 with the
/// later vertex's alpha down by alpha / (3 n) a strip [F]. `saves`: the
/// draw loop's s0 / s1 / s2 / s3 / s4 and the return address.
pub(crate) fn ribbon(fx: &mut Fx, t: u32, a: u32, b: u32, saves: [u64; 6]) {
    ribbon_saving(fx, t, a, b, [saves[0], saves[1], saves[2], saves[3], saves[4], 0, 0, 0, 0, saves[5]]);
}

/// [`ribbon`] with every saved register given: the caller's
/// s0..s7, fp and the return address at sp + 0x10..+0xA0 (b495 / card
/// 254's register compile of the same ribbon draws the same).
pub(crate) fn ribbon_saving(fx: &mut Fx, t: u32, a: u32, b: u32, saves: [u64; 10]) {
    use ee::add;
    let sp = fx.enter(0x300);
    let offs = [0u32, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90];
    let list: Vec<(u32, u64)> = offs.iter().zip(saves).map(|(&o, v)| (o, v)).collect();
    fx.saved(sp + 0x10, &list);
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 0xD0, a);
    fx.mem.write_u32(sp + 0xC0, b);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x238;
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    let (ca, cb) = (sp + 0x2F8, sp + 0x2FC);
    for c in [ca, cb] {
        for k in 0..4 {
            let v = fx.mem.read_u8(t + 0x60 + k);
            fx.mem.write_u8(c + k, v);
        }
    }
    fx.mem.write_u8(ca + 3, 0);
    let (pa, pb) = (sp + 0x170, sp + 0x1B0);
    for (p, src) in [(pa, a), (pb, b)] {
        copy4(fx, p, src);
        copy4(fx, p + 0x10, src);
        copy4(fx, p + 0x20, src + 0x10);
        copy4(fx, p + 0x30, src + 0x20);
    }
    rgbaq_at(fx, sp + 0x2C8, ca, d + 0x10);
    rgbaq_at(fx, sp + 0x2D0, ca, d + 0x20);
    rgbaq_at(fx, sp + 0x2D8, cb, d + 0x30);
    rgbaq_at(fx, sp + 0x2E0, cb, d + 0x40);
    fx.mem.set32(sp + 0xB0, n - 1);
    let al = i32::from(fx.mem.read_u8(t + 0x63));
    let step = fx.div(al, (3 * n) as i16 as i32).0 as i16 as i32;
    let mut s7 = 0;
    while s7 < n - 1 {
        let off = (0x10 * (s7 - 1)) as u32;
        let (s2, s3) = (a.wrapping_add(off), b.wrapping_add(off));
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            rgbaq_at(fx, sp + 0x2E8, cb, d + 0x30);
            let v = i32::from(fx.mem.read_u8(cb + 3)) - step;
            fx.mem.write_u8(cb + 3, if v > 0 { v as u8 } else { 0 });
            rgbaq_at(fx, sp + 0x2F0, cb, d + 0x40);
            let (qa, qb, oa, ob) = if s7 != 0 { (s2, s3, 0x1F0u32, 0x200u32) } else { (pa, pb, 0x210, 0x220) };
            spline::spline(fx, sp + 0x130, qa, f20, 0, [0.0; 4]);
            spline::spline(fx, sp + 0x150, qb, f20, 0, [0.0; 4]);
            f20 = add(f20, 0.25);
            spline::spline(fx, sp + oa, qa, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x140, sp + oa);
            spline::spline(fx, sp + ob, qb, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x160, sp + ob);
            for (to, from) in [(0xF0u32, 0x130u32), (0x100, 0x140), (0x110, 0x150), (0x120, 0x160)] {
                l2::copy3(fx, sp + to, sp + from);
            }
            if l2::project(fx, sp + 0xF0, SPR, 0, d, sp + 0xE0) {
                fx.submit(d, OT, 0);
            }
        }
        s7 += 1;
    }
    fx.leave(0x300);
}
