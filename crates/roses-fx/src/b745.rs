//! BMODULE b745 (card 85), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E039C0, data area 0x1ED0000, 0xB630 bytes) [F]
//!: b757's program shape (b590's layout: 400
//! tasks of 0x70 bytes at D+0x2A0, alive +0x58, step +0x44 through `jalr`,
//! 0x1E039B0 the idle step; spawned by 0x1E044E0 = [`crate::b590::spawn`])
//! on the second library: rings of quads (7369422df7f8 at 0x1E00920,
//! `billboard::ring_quads`), arcs (e6dce326551d at 0x1E00ED0), quads
//! (add5d81dd3fe at 0x1E01230), textured arcs (3a2db45c5a9a at
//! 0x1E014F0, `quads4::arc_strips_tex`), light columns (792ca1ad9378 at
//! 0x1E01900) and tints (561a6131a635 at 0x1E01C70). Update 1 the stage
//! dimmed (0x1E035F0) and a frame flash (0x1E03700); below 0x63 a smoke
//! ring a update at the attacker's node 0x2A; on 0x4B / 0x5F thirteen
//! white motes at each of nodes 0xA / 0x13; on 0x62 at the target a tint,
//! four columns and a band, and 0x62..0x70 at its point raised to 35 five
//! blue rings of each kind and a textured arc a update. Returns 1 on
//! update 0x62, 4 on 0x91, 2 on 0x82, 6 on 0x63..0x7C, 8 once the
//! 0x96-update timer D+0xB210 has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const SMOKE: u32 = 0x01E0_2290;
const RING_A: u32 = 0x01E0_2510;
const RING_B: u32 = 0x01E0_26C0;
const COLUMN: u32 = 0x01E0_2870;
const ARC_T: u32 = 0x01E0_2A70;
const MOTE: u32 = 0x01E0_2F40;
const RING: u32 = 0x01E0_3110;
const TINT: u32 = 0x01E0_3320;
const BAND: u32 = 0x01E0_34D0;
const LIGHT: u32 = 0x01E0_35F0;
const FLASH: u32 = 0x01E0_3700;
const IDLE: u32 = 0x01E0_39B0;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3D78;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const TINT_W: u32 = D + 0xB234;
/// The target's point on update 0x62.
const AIM: u32 = D + 0xB1D0;
const NODE_A: u32 = D + 0xB248;
const NODE_B: u32 = D + 0xB24A;
const NODE_C: u32 = D + 0xB24C;
const SPAWN_A: u32 = D + 0xB2A6;
const SPAWN_K: u32 = D + 0xB2B4;
const FACING: u32 = D + 0xB2B8;
const COUNTER: u32 = D + 0xB2BC;
const BYTES_A: u32 = D + 0xB2BE;
const BYTES_B: u32 = D + 0xB3BE;
const TAB: u32 = D + 0xB4C0;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E039C0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x190);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x190);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2BA, param);
    fx.mem.set32(TIMER, 0x96);
    for (k, node) in [0xA, 0x13, 0x2A, 0x31, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE_A + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB258, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xF0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xF4)), 180.0);
    fx.fix_residue(y, 0x01E0_3BB4);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3BE4, 0x1E);
        fx.mem.set8(BYTES_A + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3C34, 0x1E);
        fx.mem.set8(BYTES_B + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, k, 0x01E0_3C8C, 0x1E);
        fx.mem.set8(BYTES_B + k, v);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_3CDC);
        fx.mem.set16(D + 0xB268 + 2 * k, v % 8);
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
            SMOKE | ARC_T | MOTE => 0x30,
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
            RING_A => ring_ab(fx, t, RING_A, 0x1E),
            RING_B => ring_ab(fx, t, RING_B, 0xA),
            COLUMN => column(fx, t),
            ARC_T => arc_t(fx, t),
            MOTE => mote(fx, t),
            RING => ring(fx, t),
            TINT => tint(fx, t),
            BAND => band(fx, t),
            LIGHT => light(fx, t),
            FLASH => flash(fx, t, s),
            _ => panic!("b745: unknown task step {f:#x}"),
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
        0x62 => 1,
        0x91 => 4,
        0x82 => 2,
        0x63..=0x7C => 6,
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

/// D+0x94.. = a 32 x 64 cell at column +0x56.
fn strip_cell(fx: &mut Fx, t: u32) {
    let c = fx.mem.i16(t + 0x56);
    for (o, v) in [(0x94u32, c << 5), (0x96, 0), (0x98, 0x20), (0x9A, 0x40)] {
        fx.mem.set16(D + o, v);
    }
}

/// A ring of quads: D+0xB8 `n`, D+0xBE a random orientation (rand from
/// the entry's `jal` at `site`), D+0xB0 `layout`, the byte table.
fn rings(fx: &mut Fx, t: u32, s0: u32, n: i32, site: u32, layout: i32, mode: i32, table: u32) {
    fx.mem.set16(D + 0xB8, n);
    let v = fx.rand_from(s0, site + 8);
    fx.mem.set16(D + 0xBE, v % 4);
    fx.mem.set16(D + 0xB0, layout);
    billboard::ring_quads(fx, uv::uv_rect, D, t, mode, table);
    fx.mem.set16(D + 0xB8, 0);
}

fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32) {
    let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
    if f(fx) == SMOKE {
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x70, 0x14, c);
        let w = fx.mem.read_u32(TEX);
        fx.mem.write_u32(D + 0x88, w);
        strip_cell(fx, t);
        rings(fx, t, s0, 2, 0x01E0_3E40, 0, 0x20, BYTES_A);
    }
    for (g, local, site, layout) in [(RING_A, 0x80u32, 0x01E0_3EE0u32, 0), (RING_B, 0x90, 0x01E0_3F80, 1)] {
        if f(fx) == g {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + local, 0x14, c);
            rings(fx, t, s0, 4, site, layout, 0x21, BYTES_B);
        }
    }
    if f(fx) == COLUMN {
        let w = fx.mem.read_u32(D + 0x29C);
        fx.mem.write_u32(D + 0x88, w);
        cell(fx, sp + 0xA0, 0x9A, 0);
        fx.mem.set16(D + 0xBE, 0);
        quads4::sprite_quads(fx, D, t, 0, t + 0x10);
    }
    if f(fx) == ARC_T {
        let kind = |fx: &mut Fx| fx.mem.i16(t + 0x50);
        if kind(fx) == 0 {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xB0, 0x97, c);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
        }
        if kind(fx) == 1 {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xC0, 0x9B, c);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
        }
        if kind(fx) == 2 {
            let w = fx.mem.read_u32(TEX);
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
    if f(fx) == MOTE {
        let w = fx.mem.read_u32(TEX + 4);
        fx.mem.write_u32(D + 0x88, w);
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0xD0, 0x14, c);
        fx.mem.set16(D + 0xBE, 0);
        billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
    }
    if f(fx) == RING {
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0xE0, 0x14, c);
        let w = fx.mem.read_u32(TEX);
        fx.mem.write_u32(D + 0x88, w);
        strip_cell(fx, t);
        rings(fx, t, s0, 4, 0x01E0_4298, 0, 0x20, BYTES_B);
    }
    if f(fx) == TINT {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x60);
        // 0x1F71A0's frame below the entry's (its saved ra, s0).
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_4300), (0, u64::from(s0))]);
        let z = ee::sub(fx.mem.f(sp + 0x68), 30.0);
        fx.mem.setf(sp + 0x68, z);
        let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
        let mut x = fx.mem.read_u64(sp + 0x118);
        if let Some(w) = fx.project(p, None, &mut x) {
            fx.mem.write_u64(sp + 0x118, x);
            fx.mem.setf(TINT_W, w);
        }
        crate::b590::project_residue(fx, sp, sp + 0x60);
        for (kind, mode) in [(0, 0x20), (1, 0x10), (2, 0x20)] {
            if fx.mem.i16(t + 0x50) == kind {
                let w = fx.mem.f(TINT_W);
                quads4::tint4(fx, t, mode, w);
            }
        }
    }
    if f(fx) == BAND {
        fx.mem.set16(D + 0xB8, 0x24);
        billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
}

fn spawn(fx: &mut Fx, step: u32) {
    crate::b590::spawn(fx, step);
}

fn node_pos(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(at);
    fx.node_position(h, slot, out);
}

/// 0x1E01E40(D, 0): the update counter and the emitters (frame 0x30: ra,
/// the entry's s1 = D, s0 = 0).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3D38), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 1 {
        spawn(fx, LIGHT);
    }
    if c(fx) == 1 {
        spawn(fx, FLASH);
    }
    if c(fx) == 0x62 {
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, TINT);
    }
    if c(fx) == 0x62 {
        let h = fx.target_handle();
        fx.unit_position(h, AIM);
    }
    if (0x62..0x71).contains(&c(fx)) {
        for k in 0..4 {
            let v = fx.mem.read_u32(AIM + 4 * k);
            fx.mem.write_u32(POS + 4 * k, v);
        }
        fx.mem.write_u32(POS + 4, 0x420C_0000);
        for _ in 0..5 {
            spawn(fx, RING_A);
        }
        for _ in 0..5 {
            spawn(fx, RING_B);
        }
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, ARC_T);
        if c(fx) == 0x62 {
            for (a, k) in [(0, 0), (0x5A, 0), (0x2D, 1), (0x87, 1)] {
                fx.mem.set16(SPAWN_A, a);
                fx.mem.set16(SPAWN_K, k);
                spawn(fx, COLUMN);
            }
        }
        if c(fx) == 0x62 {
            spawn(fx, BAND);
        }
    }
    if c(fx) < 0x63 {
        node_pos(fx, NODE_C, POS);
        spawn(fx, SMOKE);
    }
    let n = c(fx);
    if n == 0x4B || n == 0x5F {
        for at in [NODE_A, NODE_B] {
            node_pos(fx, at, POS);
            let y = ee::sub(fx.mem.f(POS + 4), 1.5);
            fx.mem.setf(POS + 4, y);
            for _ in 0..0xD {
                spawn(fx, MOTE);
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

fn young_fade(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x15, 0x13, true);
    }
}

/// 0x1E02290 (D, task): b757's dark smoke ring (size 80..119, below update
/// 0x4E 40..59; growing by 1, cell 0..2), radius 5..14 growing by 5..14
/// (below 0x4E by half that, through 0x1A2B80), life 5..12.
fn smoke(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(COUNTER) < 0x4E {
            let v = rmod(fx, t, 0x01E0_22D4, 0x14);
            fx.mem.setf(t + 0x48, ee::from_i32(v + 0x28));
        } else {
            let v = rmod(fx, t, 0x01E0_2308, 0x28);
            fx.mem.setf(t + 0x48, ee::from_i32(v + 0x50));
        }
        let v = rmod(fx, t, 0x01E0_2338, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x18, 0x18, 0x18);
        let v = rmod(fx, t, 0x01E0_2364, 0xA);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, 0x01E0_2384, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_23A0, 0xA);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rmod(fx, t, 0x01E0_23C0, 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, 0x01E0_23DC);
        fx.mem.set16(t + 6, v % 8 + 5);
        fx.mem.set16(t + 0x58, 2);
    }
    young_fade(fx, t);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x48, 1.0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        if fx.mem.i16(COUNTER) < 0x4E {
            let half = ee::div(ee::from_i32(fx.mem.i16(t + 0xE)), 2.0);
            let r = ee::add(ee::from_i32(fx.mem.i16(t + 0x50)), half);
            fx.fix_residue(r, 0x01E0_24CC);
            fx.mem.set16(t + 0x50, ee::fixsfsi(r));
        } else {
            let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
            fx.mem.set16(t + 0x50, r);
        }
    }
    age(fx, t);
}

/// 0x1E02510 / 0x1E026C0 (D, task): b590's grey ring as a blue one (size
/// 30..44 / 10..24, cell row 0..2, radius +0x50 10..19 growing by 20..39,
/// a random angle, row 0..34, life 3..5).
fn ring_ab(fx: &mut Fx, t: u32, f: u32, size: i32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 0xF);
        fx.mem.setf(t + 0x48, ee::from_i32(v + size));
        let v = rmod(fx, t, f + 0x50, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
        let v = rmod(fx, t, f + 0x7C, 0xA);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, f + 0x9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB8, 0x14);
        fx.mem.set16(t + 0xE, v + 0x14);
        let v = rmod(fx, t, f + 0xD8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xF4, 3);
        fx.mem.set16(t + 6, v + 3);
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
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02870 (D, task): b590's light column (kinds 0 / 1 20 wide, 870 /
/// 630 high), white / red.
fn column(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0x40, 0x40);
        for (kind, w, h) in [(0, 0x41A0_0000u32, 0x4459_8000u32), (1, 0x41A0_0000, 0x441D_8000), (2, 0x4100_0000, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, w);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x52, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 4 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    for (kind, dh, dw) in [(0, 160.0f32, 1.0f32), (1, 80.0, 0.5)] {
        if fx.mem.i16(t + 0x50) == kind {
            if fx.mem.i16(t + 0x54) != 0 {
                let h = ee::add(fx.mem.f(t + 0x34), dh);
                fx.mem.setf(t + 0x34, h);
            }
            if fx.mem.i16(t + 0x54) != 0 {
                let w = ee::add(fx.mem.f(t + 0x30), dw);
                fx.mem.setf(t + 0x30, w);
            }
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let c = fx.mem.i16(t + 0x52);
        if c < 0x19 {
            fx.mem.set16(t + 0x52, c + 1);
        }
    }
    age(fx, t);
}

/// 0x1E02A70 (D, task): b757's textured arc with kind 0 blue ((0x80,
/// 0x80, 0xC0) / (0x80, 0x80, 0xFF)).
fn arc_t(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(20.0, 58.0));
        let odd = |fx: &mut Fx| fx.mem.i16(COUNTER) % 2 == 1;
        let colours: [((i32, i32, i32), (i32, i32, i32)); 3] = [((0x80, 0x80, 0xC0), (0x80, 0x80, 0xFF)), ((0x60, 0x80, 0x60), (0x40, 0xFF, 0x40)), ((0x50, 0x50, 0x50), (0x60, 0x60, 0x60))];
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
            let v = fx.rand_from(t, 0x01E0_2D10);
            fx.mem.set16(t + 0x52, (v >> 4) % 2);
        }
        for (kind, a, b) in [(1, 0xA, 3), (2, 0x3C, 0x3C)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.set16(t + 0x5A, a);
                fx.mem.set16(t + 0x5C, b);
            }
        }
        let v = rmod(fx, t, 0x01E0_2D70, 0x50);
        fx.mem.setf(t + 0x38, ee::from_i32(v + 0x15E));
        let v = ee::div(fx.mem.f(t + 0x38), 10.0);
        fx.mem.setf(t + 0x38, v);
        fx.mem.write_u32(t + 0x30, 0x43AF_0000);
        let v = ee::div(fx.mem.f(t + 0x30), 10.0);
        fx.mem.setf(t + 0x30, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, 0x01E0_2DCC, 0xFFFF);
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

/// 0x1E02F40 (D, task): b757's white mote.
fn mote(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x64);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2F78);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2FB0);
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
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_30E0);
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

/// 0x1E03110 (D, task): b757's grey ring (word for word).
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_3134, 0x3C);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x50));
        let v = rmod(fx, t, 0x01E0_3160, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x50, 0x50, 0x50);
        let v = rmod(fx, t, 0x01E0_318C, 0xA);
        fx.mem.set16(t + 0x50, v + 5);
        let v = fx.rand_from(t, 0x01E0_31AC);
        fx.mem.set16(t + 0x52, v % 2);
        let v = rmod(fx, t, 0x01E0_31C8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_31E4, 0x1E);
        fx.mem.set16(t + 0xE, v + 0x14);
        let v = rmod(fx, t, 0x01E0_3204, 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, 0x01E0_3220);
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

/// 0x1E03320 (D, task): a tint by its kind +0x50 (0 white then dark red,
/// life 10; 1 violet, 32; 2 grey blue, 22); both alphas in by 0x78 under
/// 0xF0 while young, then out by 14 (the task ends when the first is
/// below 16, the second is cleared).
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0xF0);
            l2::rgb(fx, t + 0x64, 0xF0, 0xF0, 0xF0);
            l2::rgb(fx, t + 0x60, 0x9F, 0x5F, 0x5F);
            l2::rgb(fx, t + 0x64, 0x9F, 0x5F, 0x5F);
            fx.mem.set16(t + 6, 0xA);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xE0, 0xB0, 0xF0);
            l2::rgb(fx, t + 0x64, 0xD0, 0xA0, 0xF0);
            fx.mem.set16(t + 6, 0x20);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            l2::rgb(fx, t + 0x60, 0x58, 0x58, 0x78);
            l2::rgb(fx, t + 0x64, 0x58, 0x58, 0x78);
            fx.mem.set16(t + 6, 0x16);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0xF0 {
                fx.mem.write_u8(t + o, a.wrapping_add(0x78));
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x67);
        fx.mem.write_u8(t + 0x67, if a >= 0x10 { a - 0xE } else { 0 });
    }
    age(fx, t);
}

/// 0x1E034D0 (D, task): b757's grey / blue band, radius 120, in for 18
/// updates.
fn band(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x42F0_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x12 {
        fade_in(fx, t, 0x67, 0xC0, 0x30);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF, true);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_3590);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x4C, f32::from_bits(0x3ECC_CCCD));
    }
    age(fx, t);
}

/// 0x1E035F0 (D, task): b757's stage light: down by 10 to 0x30 for 51
/// updates, back up once the timer is under 20.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x33 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x14 {
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

/// 0x1E03700 (D, task): b757's frame flash, bluer from update 0x5F ((0x98,
/// 0x98, 0xB8)), its ST inset +0x48 growing by 1 to 10 from 0x60; in by
/// 0x16 under 0x58 until update 0x7A, then out by 4.
fn flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x90);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) == 0x5F {
        l2::rgb(fx, t + 0x60, 0x98, 0x98, 0xB8);
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
    if fx.mem.i16(COUNTER) >= 0x60 && crate::exe::ee_lt(fx.mem.f(t + 0x48), 10.0) {
        add(fx, t + 0x48, 1.0);
    }
    if fx.mem.i16(COUNTER) < 0x7A {
        fade_in(fx, t, 0x63, 0x58, 0x16);
    } else {
        fade_out(fx, t, 0x63, 6, 4, true);
    }
    age(fx, t);
}
