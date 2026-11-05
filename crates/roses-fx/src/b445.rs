//! BMODULE b445 / b845 (cards 582 / 674, twins), hand-ported from the
//! records (linked at 0x1E00000, entries 0x1E03FE0 / 0x1E03FA0, data area
//! 0x1ED0000) [F] as one program with a [`Rec`]
//! of their differences. b764's family on the 250-task pool (spawn
//! 0x1E04BD0 / 0x1E04BA0 = b764's on the colour D+0xDB34, the angle
//! D+0xDC1E and the index D+0xDC44; the entry keeps D in s0 and counts the
//! drawn tasks in s1), no petals. Drawn with arcs (0x1E009B0), turned quad
//! rings (0x1E00D10, `billboard::ring_quads_turned` on the bytes D+0xDD4E),
//! camera-facing quads (0x1E012E0, `billboard::quad`), sparks (0x1E015A0,
//! `quads4::sparks`), trails as b733's blended tube (0x1E04D80 / 0x1E04D50,
//! word for word but the addresses: `tubetrail::tube_with(SINGLE_BLEND)`)
//! drawn twice each, and a window sprite (b178's in green / yellow).
//! Globals past D+0xD000: ten orb points D+0xD930 + 0x10 k, two points
//! D+0xDA70 / D+0xDA80 at the attacker's node 0x2B / 7 every update,
//! textures D+0xDB10.., timer D+0xDB30 = 340 / 320, OT D+0xDB3C, nodes
//! D+0xDB68.., cells D+0xDB88, orb count D+0xDC1A, trail counter D+0xDC22,
//! facing D+0xDC48, counter D+0xDC4C, bytes D+0xDC4E (100..0xB3 cleared) /
//! D+0xDD4E / D+0xDE4E, sines D+0xDF50. The emitter 0x1E019A0: 1 a stage
//! light 0x1E03C90; 0x2D / every third update from 0x37 to 0x11D an arc
//! 0x1E01DE0 at the node; 0xAD a window sprite; 0x8C..0x140 every 13th /
//! 10th update an orb 0x1E026F0 at the node with a glow 0x1E02B30 (ten in
//! all); each orb at age 32 bursts: an arc 0x1E021E0, five rings 0x1E02330,
//! seven spark bursts 0x1E02570, ten rings 0x1E02CA0, seven trails
//! 0x1E03080 and a window sprite; 0x2E..0x11D a trail 0x1E03680 on odd
//! updates (the trail counter still moves on even ones), 0x2E..0x13B a ring
//! 0x1E02E80 a update. Returns 1 / 4 / 2 / 6 / 8 on 0xA3 / 0x14F (0x127) /
//! 0x140 (0x118) / 0xAE..0x144 (0x112) / timer out.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::b764::{age, cell, copy4, enter_step_with, fade_in, fade_out, rand_mod, trail_of, Glob, TrailCfg};
use crate::blib::{billboard, quads4, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xFA;
const ARC1: u32 = 0x01E0_1DE0;
const ARC2: u32 = 0x01E0_21E0;
const RING_X: u32 = 0x01E0_2330;
const SPARKS: u32 = 0x01E0_2570;
const ORB: u32 = 0x01E0_26F0;
/// b445's addresses; b845's from here on sit 0x40 lower (`Rec::late`).
const GLOW: u32 = 0x01E0_2B30;
const RING_Y: u32 = 0x01E0_2CA0;
const RING_Z: u32 = 0x01E0_2E80;
const TRAIL_A: u32 = 0x01E0_3080;
const TRAIL_B: u32 = 0x01E0_3680;
const LIGHT: u32 = 0x01E0_3C90;
const WINDOW: u32 = 0x01E0_3D50;
const STUB: u32 = 0x01E0_3FD0;
const POINTS: u32 = D + 0xD930;
const NODE_A: u32 = D + 0xDA70;
const NODE_B: u32 = D + 0xDA80;
const TEX: u32 = D + 0xDB10;
const TIMER: u32 = D + 0xDB30;
const COLOUR: u32 = D + 0xDB34;
const OT_AT: u32 = D + 0xDB3C;
const NODES: u32 = D + 0xDB68;
const CELLS: u32 = D + 0xDB88;
const ORBS: u32 = D + 0xDC1A;
const ANGLE: u32 = D + 0xDC1E;
const TRAIL_K: u32 = D + 0xDC22;
const INDEX: u32 = D + 0xDC44;
const FACING: u32 = D + 0xDC48;
const COUNTER: u32 = D + 0xDC4C;
const BYTES: u32 = D + 0xDC4E;
const BYTES_B: u32 = D + 0xDD4E;
const BYTES_C: u32 = D + 0xDE4E;
const SINES: u32 = D + 0xDF50;

const G: Glob = Glob { colour: COLOUR, angle: ANGLE, index: INDEX, cells: CELLS, sines: SINES, ot_at: OT_AT };

/// The steps' saved s0 / s1: the entry's (s0 = D, s1 the draw count, 0
/// while the steps run).
const VALS: [u64; 2] = [D as u64, 0];

/// What the twins differ in.
struct Rec {
    frame: u32,
    timer: i32,
    node0: i32,
    textures: u32,
    /// unit_rotation's output local.
    rot: u32,
    fix_ra: u32,
    fill_ra: [u32; 4],
    emit_ra: u32,
    task_ra: u64,
    /// The arc draw's 0x1F6830, the glow / ring draws' rands.
    draw_ra: [u32; 3],
    /// The cell locals of the orb / glow / ring Y / ring Z draws.
    cells: [u32; 4],
    /// The spark bursts' draw: a 0x8C cell of texture D+0x298 (b845), else
    /// texture D+0xDB24 and the fixed UVs (0, 0, 0x1F, 0x7F).
    sparks_cell: bool,
    /// The steps from 0x1E02B30 (and the orb's tail) sit this much lower.
    late: u32,
    /// The orb point sinking 0.3 a update (b445 only).
    sink: bool,
    /// The emitter's orb period.
    orb_every: i32,
    /// The arc colours: (lo, hi, lo) green (b445) or (hi, hi, lo) yellow.
    yellow: bool,
    /// The spark bursts' speed (tenths).
    burst: f32,
    /// The window's colour and its first kind's fade-in test (b445: the
    /// counter below 0x140; b845: the timer at least 0x15).
    window_rgb: [u8; 3],
    window_by_timer: bool,
    /// Returns 4 / 2 and the end of the 6 range.
    ret: (i32, i32, i32),
}

const B445: Rec = Rec {
    frame: 0xD0,
    timer: 0x154,
    node0: 0x2B,
    textures: 6,
    rot: 0xC0,
    fix_ra: 0x01E0_41D4,
    fill_ra: [0x01E0_4204, 0x01E0_429C, 0x01E0_42F4, 0x01E0_4350],
    emit_ra: 0x01E0_4408,
    task_ra: 0x01E0_4448,
    draw_ra: [0x01E0_44C4, 0x01E0_4748, 0x01E0_4834],
    cells: [0x80, 0x90, 0xA0, 0xB0],
    sparks_cell: false,
    late: 0,
    sink: true,
    orb_every: 0xD,
    yellow: false,
    burst: 400.0,
    window_rgb: [0x68, 0x80, 0x68],
    window_by_timer: false,
    ret: (0x14F, 0x140, 0x144),
};

const B845: Rec = Rec {
    frame: 0xE0,
    timer: 0x140,
    node0: 7,
    textures: 5,
    rot: 0xD0,
    fix_ra: 0x01E0_4194,
    fill_ra: [0x01E0_41C4, 0x01E0_425C, 0x01E0_42B4, 0x01E0_4310],
    emit_ra: 0x01E0_43C8,
    task_ra: 0x01E0_4408,
    draw_ra: [0x01E0_4484, 0x01E0_4720, 0x01E0_480C],
    cells: [0x90, 0xA0, 0xB0, 0xC0],
    sparks_cell: true,
    late: 0x40,
    sink: false,
    orb_every: 0xA,
    yellow: true,
    burst: 750.0,
    window_rgb: [0x80, 0x80, 0x68],
    window_by_timer: true,
    ret: (0x127, 0x118, 0x112),
};

fn point(k: i32) -> u32 {
    (POINTS as i32 + 0x10 * k) as u32
}

/// The entry 0x1E03FE0 (card 582).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, &B445, param)
}

/// The entry 0x1E03FA0 (card 674, b845).
pub fn program_b845(fx: &mut Fx, param: i32) -> i32 {
    run(fx, &B845, param)
}

fn run(fx: &mut Fx, r: &Rec, param: i32) -> i32 {
    let sp = fx.enter(r.frame);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, r, sp, param) } else { step(fx, r, sp, param) };
    fx.leave(r.frame);
    ret
}

fn init(fx: &mut Fx, r: &Rec, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xE0C0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xDC4A, param);
    fx.mem.set32(TIMER, r.timer);
    for (k, n) in [r.node0, 0x5C, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let v = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, v);
    }
    let t = fx.target_handle();
    let v = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xDB78, v);
    for (k, id) in [(0u32, 0xE), (1, 0xF)] {
        let tex = fx.texture(4, id);
        fx.mem.write_u32(D + 0x298 + 4 * k, tex);
    }
    for k in 0..r.textures {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + r.rot);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + r.rot + 4)), 180.0);
    fx.fix_residue(v, r.fix_ra);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for (n, table) in [BYTES, BYTES_B, BYTES_C].into_iter().enumerate() {
        for k in 0..0x100u32 {
            let v = fx.rand_from(D, r.fill_ra[n]);
            let (_, h) = fx.div(v >> 4, 0x1E);
            fx.mem.write_u8(table + k, (h - 0xF) as u8);
        }
        if n == 0 {
            for k in 100..0xB4u32 {
                fx.mem.write_u8(BYTES + k, 0);
            }
        }
    }
    for k in 0..0x24u32 {
        let v = fx.rand_from(D, r.fill_ra[3]);
        fx.mem.set16(CELLS + 2 * k, v.wrapping_rem(8));
    }
    for k in 0..8u32 {
        fx.mem.set16(D + 0xDC2C + 2 * k, 0);
    }
    0
}

fn ring_turned(fx: &mut Fx, t: u32, n: i32) {
    fx.mem.set16(D + 0xB8, n);
    fx.mem.set16(D + 0xB0, 0);
    billboard::ring_quads_turned(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
    fx.mem.set16(D + 0xB8, 0);
    fx.mem.set16(D + 0xB0, 0);
}

fn orb_pos(fx: &mut Fx, t: u32, sp: u32) {
    let p = point(fx.mem.i16(t + 0x50));
    for c in [0u32, 4, 8] {
        let w = fx.mem.read_u32(p + c);
        fx.mem.write_u32(sp + 0x60 + c, w);
    }
}

fn step(fx: &mut Fx, r: &Rec, sp: u32, param: i32) -> i32 {
    emit(fx, r, param);
    let late = |a: u32| a - r.late;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 || f == late(STUB) {
            continue;
        }
        run_task(fx, r, f, slot + 0x2A0, k);
    }
    let mut drawn = 0u64;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        drawn += 1;
        let f = fx.mem.read_u32(t + 0x44);
        if f == ARC1 {
            fx.mem.set16(D + 0xB8, 0x24);
            let h = fx.self_handle();
            let s = fx.mem.i16(NODES);
            node_position_saving(fx, h, s, sp + 0x60, u64::from(D), Some(drawn), r.draw_ra[0]);
            billboard::arc_strips(fx, D, t, 0x21, sp + 0x60);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == ARC2 {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == RING_X {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            let frame = fx.mem.i16(t + 0x56);
            l2::rect(fx, sp + 0x70, 0x9B, frame);
            copy4(fx, D + 0x94, sp + 0x70);
            ring_turned(fx, t, 0xA);
        }
        if f == SPARKS {
            if r.sparks_cell {
                let frame = fx.mem.i16(t + 0x56);
                l2::rect(fx, sp + 0x80, 0x8C, frame);
                copy4(fx, D + 0x94, sp + 0x80);
                let w = fx.mem.read_u32(D + 0x298);
                fx.mem.write_u32(D + 0x88, w);
            } else {
                let w = fx.mem.read_u32(TEX + 0x14);
                fx.mem.write_u32(D + 0x88, w);
                for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x1F), (0x9A, 0x7F)] {
                    fx.mem.set16(D + o, v);
                }
            }
            for c in [0u32, 4, 8] {
                let w = fx.mem.read_u32(t + 0x10 + c);
                fx.mem.write_u32(D + 0x20 + c, w);
            }
            fx.mem.set16(D + 0xB8, 0x18);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == ORB {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + r.cells[0], 0x14);
            fx.mem.set16(D + 0xBE, 5);
            orb_pos(fx, t, sp);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
        }
        if f == late(GLOW) {
            cell(fx, t, sp + r.cells[1], 0x14);
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            let v = fx.rand_from(D, r.draw_ra[1]);
            fx.mem.set16(D + 0xBE, v.wrapping_rem(4));
            orb_pos(fx, t, sp);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
        }
        if f == late(RING_Y) {
            let w = fx.mem.read_u32(TEX + 0xC);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + r.cells[2], 0x14);
            fx.mem.set16(D + 0xB8, 2);
            fx.mem.set16(D + 0xB0, 0);
            let v = fx.rand_from(D, r.draw_ra[2]);
            fx.mem.set16(D + 0xBE, v.wrapping_rem(8));
            billboard::ring_quads_turned(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if f == late(RING_Z) {
            let w = fx.mem.read_u32(TEX + 0x10);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + r.cells[3], 0x14);
            ring_turned(fx, t, 2);
        }
        if f == late(TRAIL_A) || f == late(TRAIL_B) {
            let a = f == late(TRAIL_A);
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x3F)] {
                fx.mem.set16(D + o, v);
            }
            fx.mem.set16(D + 0xBE, 1);
            let (first, second, turn) = if a { (t + 0x10, t + 0x10, 0x8000) } else { (NODE_B, NODE_A, 0x4000) };
            for (p, turned) in [(first, false), (second, true)] {
                for c in [0u32, 4, 8] {
                    let w = fx.mem.read_u32(p + c);
                    fx.mem.write_u32(D + 0x20 + c, w);
                }
                let ang = fx.mem.read_u32(t + 0x20);
                if turned {
                    fx.mem.write_u32(t + 0x20, ang.wrapping_add(turn));
                }
                let k = fx.mem.i16(t + 0x50);
                tubetrail::tube_with(fx, &tubetrail::SINGLE_BLEND, D, t, trail_of(k), 0x21);
                if turned {
                    fx.mem.write_u32(t + 0x20, ang);
                }
            }
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    let n = fx.mem.i16(COUNTER);
    if n == 0xA3 {
        1
    } else if n == r.ret.0 {
        4
    } else if n == r.ret.1 {
        2
    } else if (0xAE..=r.ret.2).contains(&n) {
        6
    } else if fx.mem.i32(TIMER) > 0 {
        0
    } else {
        8
    }
}

fn trail_cfg(r: &Rec, b: bool) -> TrailCfg {
    let l = |a: u32| if a == 0 { 0 } else { a - r.late };
    if b {
        TrailCfg {
            div: 10.0,
            heading_b: true,
            width: (0x32, 0x14),
            step: (0x19, 3),
            dim: 0x40,
            jdiv: 15.0,
            init: [0x01E0_36F8, 0x01E0_3724, 0x01E0_3750, 0x01E0_387C, 0x01E0_38BC, 0x01E0_38FC, 0x01E0_3918, 0x01E0_3938].map(l),
            jitter: [0x01E0_3A34, 0x01E0_3A60, 0x01E0_3A8C].map(l),
        }
    } else {
        TrailCfg {
            div: 2.0,
            heading_b: false,
            width: (0x96, 0x32),
            step: (0x7D, 3),
            dim: 0x80,
            jdiv: 10.0,
            init: [0x01E0_30F8, 0x01E0_3124, 0x01E0_3150, 0x01E0_327C, 0x01E0_32BC, 0x01E0_32FC, 0, 0x01E0_3320].map(l),
            jitter: [0x01E0_341C, 0x01E0_3448, 0x01E0_3474].map(l),
        }
    }
}

fn run_task(fx: &mut Fx, r: &Rec, f: u32, t: u32, k: u32) {
    let late = |a: u32| a - r.late;
    match f {
        ARC1 => arc1(fx, r, t),
        ARC2 => arc2(fx, r, t),
        RING_X => ring_x(fx, r, t),
        SPARKS => sparks(fx, r, t),
        ORB => orb(fx, r, t, k),
        _ if f == late(GLOW) => glow(fx, r, t),
        _ if f == late(RING_Y) => ring_y(fx, r, t),
        _ if f == late(RING_Z) => ring_z(fx, r, t),
        _ if f == late(TRAIL_A) => crate::b764::trail(fx, t, &trail_cfg(r, false), SINES, VALS, r.task_ra),
        _ if f == late(TRAIL_B) => crate::b764::trail(fx, t, &trail_cfg(r, true), SINES, VALS, r.task_ra),
        _ if f == late(LIGHT) => light(fx, r, t),
        _ if f == late(WINDOW) => window(fx, r, t),
        _ => panic!("b445: unknown task step {f:#x}"),
    }
}

/// 0x1E019A0(D, 0): the update counter, the two node points and the
/// spawns.
fn emit(fx: &mut Fx, r: &Rec, param: i32) {
    let sp = fx.enter(0x30);
    let d = u64::from(D);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x20, u64::from(r.emit_ra)), (0x10, 0), (0, d)]);
    // The spawns' saved s0..s4: the emitter's s0 (the entry's D, the loop
    // counts), s1 = D, the entry's s2 (param), s3 / s4 (the runner's, 0).
    let late = |a: u32| a - r.late;
    let spawn = |fx: &mut Fx, f: u32, s0: u64, ra: u32| crate::b764::spawn_with(fx, &G, f, [s0, d, p, 0, 0], ra);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    for (at, ra) in [(NODE_A, 0x01E0_19F0u32), (NODE_B, 0x01E0_1A14)] {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        node_position_saving(fx, h, s, at, d, Some(d), ra);
    }
    if fx.mem.i16(COUNTER) == 0x2D {
        fx.mem.set16(INDEX, 0);
        spawn(fx, ARC1, d, 0x01E0_1A4C);
    }
    let n = fx.mem.i16(COUNTER);
    if n >= 0x37 && n.wrapping_rem(3) == 1 && n < 0x11E {
        fx.mem.set16(INDEX, 1);
        spawn(fx, ARC1, d, 0x01E0_1AB0);
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, late(LIGHT), d, 0x01E0_1ADC);
    }
    if fx.mem.i16(COUNTER) == 0xAD {
        fx.mem.set16(INDEX, 0);
        spawn(fx, late(WINDOW), d, 0x01E0_1B14);
    }
    let n = fx.mem.i16(COUNTER);
    if n >= 0x8C && n.wrapping_rem(r.orb_every) == 1 && n < 0x141 && fx.mem.i16(ORBS) < 0xA {
        let k = fx.mem.i16(ORBS);
        fx.mem.set16(INDEX, k);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        let k = fx.mem.i16(ORBS);
        node_position_saving(fx, h, s, point(k), d, Some(d), 0x01E0_1BB0);
        spawn(fx, ORB, d, 0x01E0_1BC4);
        spawn(fx, late(GLOW), d, 0x01E0_1BD8);
        let k = fx.mem.i16(ORBS) + 1;
        fx.mem.set16(ORBS, k);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x2E..0x11E).contains(&n) {
        let i = fx.mem.i16(TRAIL_K);
        fx.mem.set16(INDEX, i);
        if fx.mem.i16(COUNTER).wrapping_rem(2) == 1 {
            spawn(fx, late(TRAIL_B), 0, 0x01E0_1C78);
        }
        let i = fx.mem.i16(TRAIL_K) + 1;
        fx.mem.set16(TRAIL_K, i);
        if fx.mem.i16(TRAIL_K) >= 0x50 {
            fx.mem.set16(TRAIL_K, 0);
        }
    }
    let n = fx.mem.i16(COUNTER);
    if (0x2E..0x13C).contains(&n) {
        copy4(fx, D + 0x250, NODE_B);
        spawn(fx, late(RING_Z), 0, 0x01E0_1DA0);
    }
    fx.leave(0x30);
}

/// (lo, hi, lo) in b445's green, (hi, hi, lo) in b845's yellow.
fn col(r: &Rec, lo: i32, hi: i32) -> (i32, i32, i32) {
    if r.yellow {
        (hi, hi, lo)
    } else {
        (lo, hi, lo)
    }
}

fn rgb(fx: &mut Fx, p: u32, c: (i32, i32, i32)) {
    l2::rgb(fx, p, c.0, c.1, c.2);
}

/// 0x1E01DE0: an arc fan at the attacker's node (index 0: radius 100, in
/// by 10 below 0x80 for 20 updates, out by 0xB from update 0x119; 1: radius
/// 10 + rand % 50, darker, in by 0x40 for 4, then out), white outside, its
/// colour flickering by age and dimmer from update 0xC8; facing the camera
/// turned by 0x4000 about x.
fn arc1(fx: &mut Fx, r: &Rec, t: u32) {
    enter_step_with(fx, 0x30, 2, VALS, r.task_ra);
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x4C, 0x42C8_0000);
            l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
            rgb(fx, t + 0x60, col(r, 0x80, 0xFF));
            fx.mem.set16(t + 0x52, 0x14);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            let v = rand_mod(fx, t, 0x01E0_1E60, 0x32);
            fx.mem.setf(t + 0x4C, ee::add(10.0, ee::from_i32(v)));
            if fx.mem.i16(COUNTER) < 0xC8 {
                l2::rgb(fx, t + 0x64, 0x40, 0x40, 0x40);
                rgb(fx, t + 0x60, col(r, 0x40, 0x80));
            } else {
                l2::rgb(fx, t + 0x64, 0x20, 0x20, 0x20);
                rgb(fx, t + 0x60, col(r, 0x20, 0x40));
            }
            fx.mem.set16(t + 0x52, 4);
        }
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let k = fx.mem.i16(t + 0x50);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 0x52) {
        if k == 0 {
            fade_in(fx, t, 0x67, (0x80, 0xA));
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fade_in(fx, t, 0x67, (0x80, 0x40));
        }
    } else {
        if k == 0 && fx.mem.i16(COUNTER) >= 0x119 {
            fade_out(fx, t, 0x67, (0xD, 0xB));
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fade_out(fx, t, 0x67, (0xD, 0xB));
        }
    }
    let odd = fx.mem.i16(t + 0x54).wrapping_rem(2) == 1;
    let early = fx.mem.i16(COUNTER) < 0xC8;
    if fx.mem.i16(t + 0x50) == 0 {
        let c = match (early, odd) {
            (true, true) => col(r, 0x80, 0xFF),
            (true, false) => col(r, 0x80, 0xC0),
            (false, true) => col(r, 0x60, 0xC0),
            (false, false) => col(r, 0x40, 0x80),
        };
        rgb(fx, t + 0x60, c);
    }
    if fx.mem.i16(t + 0x50) == 1 {
        let early = fx.mem.i16(COUNTER) < 0xC8;
        let c = match (early, odd) {
            (true, true) => (0x80, 0x80, 0xFF),
            (true, false) => (0x80, 0x80, 0xC0),
            (false, true) => (0x40, 0x40, 0x80),
            (false, false) => (0x40, 0x40, 0x60),
        };
        rgb(fx, t + 0x60, c);
    }
    age(fx, t);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_21B0);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    fx.leave(0x30);
}

/// 0x1E021E0: the orb's burst arc fan (radius 280, pale yellow over white,
/// in by 0x60 below 0xC0 for 6 updates, then out by 0x13, flickering),
/// facing the camera turned by 0x4000 about x.
fn arc2(fx: &mut Fx, r: &Rec, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, r.task_ra);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x438C_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x80);
        fx.mem.set16(t + 0x52, 6);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 0x52) {
        fade_in(fx, t, 0x67, (0xC0, 0x60));
    } else {
        fade_out(fx, t, 0x67, (0x15, 0x13));
    }
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, 0x80);
    age(fx, t);
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2304);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    fx.leave(0x20);
}

/// 0x1E02330: a burst ring of 10 cells 0x9B (size rand % 10 + 10, cell
/// rand % 4, radius rand % 20 + 10 growing by rand % 15 + 10, random
/// angles, rows rand % 8 + 1), 0x80 for rand % 10 + 13 updates, then out
/// by 0xB; yellow / red / blue in turn.
fn ring_x(fx: &mut Fx, r: &Rec, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, r.task_ra);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rand_mod(fx, t, 0x01E0_2354, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = fx.rand_from(t, 0x01E0_2380);
        fx.mem.set16(t + 0x56, v.wrapping_rem(4));
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rand_mod(fx, t, 0x01E0_23B4, 0x14);
        fx.mem.set16(t + 0x50, v + 0xA);
        for (o, ra) in [(0x5Au32, 0x01E0_23D4u32), (0x5C, 0x01E0_23F4)] {
            let v = fx.rand_from(t, ra);
            let (_, h) = fx.div(v >> 4, 0x168);
            fx.mem.set16(t + o, h);
        }
        let v = rand_mod(fx, t, 0x01E0_2414, 0xF);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = fx.rand_from(t, 0x01E0_2434);
        fx.mem.set16(t + 8, v.wrapping_rem(8) + 1);
        let v = rand_mod(fx, t, 0x01E0_2454, 0xA);
        fx.mem.set16(t + 6, v + 0xD);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t);
    let c = match fx.mem.i16(t + 0x54).wrapping_rem(3) {
        1 => (0xC0, 0xC0, 0x40),
        2 => (0xC0, 0x40, 0x40),
        _ => (0x40, 0x40, 0xC0),
    };
    rgb(fx, t + 0x60, c);
    let n = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, n);
    let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, v);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x80 for +6 updates, then out by 0xB.
fn hold(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, (0xD, 0xB));
    }
}

/// 0x1E02570: a white / grey spark burst (radius 200 growing by `burst`
/// tenths a update, its inner 400 less, three random turns, the y angle
/// turning 0x100), 0x60 for 6 updates, then out by 0xE.
fn sparks(fx: &mut Fx, r: &Rec, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, r.task_ra);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.setf(t + 0x4C, ee::sub(200.0, 58.0));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0x40);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.setf(t + 0x34, r.burst);
        let v = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, ra) in [(0x20u32, 0x01E0_260Cu32), (0x24, 0x01E0_2628), (0x28, 0x01E0_2644)] {
            let v = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 6 {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        fade_out(fx, t, 0x63, (0x10, 0xE));
    }
    let r0 = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r0);
    fx.mem.setf(t + 0x4C, ee::sub(r0, 400.0));
    let a = fx.mem.i32(t + 0x24).wrapping_add(0x100);
    fx.mem.set32(t + 0x24, a);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E026F0: orb +0x50 (k) at its point D+0xD930 + 0x10 k: a violet
/// camera-facing glow (70 x 70, cell rand % 8 cycling to 8, white / light
/// grey by age), in by 0x60 below 0xC0 for 20 updates, then out by 9; to
/// age 32 its point drifting 5 a update along z away from the facing (b445:
/// and sinking 0.3); at age 32 its burst (spawned from this step with its
/// own s0..s4: the loop count, t, D, the entry's slot index, 0).
fn orb(fx: &mut Fx, r: &Rec, t: u32, slot: u32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, r.task_ra), (0x20, u64::from(t - 0x2A0)), (0x10, 0), (0, u64::from(D))]);
    let late = |a: u32| a - r.late;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x46);
        fx.mem.set16(t + 0xA, 0x46);
        let v = fx.rand_from(D, 0x01E0_2728);
        fx.mem.set16(t + 0x56, v.wrapping_rem(8));
        fx.mem.write_u32(t + 0x34, 0x3F00_0000);
        l2::rgb(fx, t + 0x60, 0xA0, 0xA0, 0xFF);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        fade_in(fx, t, 0x63, (0xC0, 0x60));
    } else {
        fade_out(fx, t, 0x63, (0xB, 9));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(D), u64::from(t), 0x01E0_27BC);
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    let c = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, c);
    if fx.mem.i16(t + 0x56) >= 8 {
        fx.mem.set16(t + 0x56, 0);
    }
    let a = fx.mem.i16(t + 0x54);
    if a != 0 {
        let p = point(fx.mem.i16(t + 0x50));
        if a < 0x21 {
            let z = fx.mem.f(p + 8);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
            fx.mem.setf(p + 8, z);
        }
        if r.sink && fx.mem.i16(t + 0x54) < 0x21 {
            let p = point(fx.mem.i16(t + 0x50));
            let y = ee::sub(fx.mem.f(p + 4), f32::from_bits(0x3E99_999A));
            fx.mem.setf(p + 4, y);
        }
    }
    let mut s0 = u64::from(D);
    if fx.mem.i16(t + 0x54) == 0x20 {
        let regs = |s0: u64| [s0, u64::from(t), u64::from(D), u64::from(slot), 0];
        let spawn = |fx: &mut Fx, f: u32, s0: u64, ra: u32| crate::b764::spawn_with(fx, &G, f, regs(s0), ra);
        let p = point(fx.mem.i16(t + 0x50));
        copy4(fx, D + 0x250, p);
        spawn(fx, ARC2, s0, late(0x01E0_2978));
        for k in 0..5u64 {
            spawn(fx, RING_X, k, late(0x01E0_2994));
        }
        for k in 0..7u64 {
            spawn(fx, SPARKS, k, late(0x01E0_29D0));
        }
        for k in 0..10u64 {
            spawn(fx, late(RING_Y), k, late(0x01E0_2A10));
        }
        for k in 0..7u64 {
            let i = fx.mem.i16(TRAIL_K);
            fx.mem.set16(INDEX, i);
            spawn(fx, late(TRAIL_A), k, late(0x01E0_2A68));
            let i = fx.mem.i16(TRAIL_K) + 1;
            fx.mem.set16(TRAIL_K, i);
            if fx.mem.i16(TRAIL_K) >= 0x50 {
                fx.mem.set16(TRAIL_K, 0);
            }
        }
        s0 = 7;
        fx.mem.set16(INDEX, 1);
        spawn(fx, late(WINDOW), s0, late(0x01E0_2AF0));
    }
    camera_angles_saving(fx, t + 0x20, s0, u64::from(t), late(0x01E0_2AFC));
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E02B30: the orb's glow (size 80, 70 x 70, cell rand % 8 cycling to
/// 7, yellow / blue by age, 0x80 for 34 updates), facing the camera, its
/// angle +0x5A turning by 10.
fn glow(fx: &mut Fx, r: &Rec, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, r.task_ra);
    let late = |a: u32| a - r.late;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42A0_0000);
        fx.mem.set16(t + 8, 0x46);
        fx.mem.set16(t + 0xA, 0x46);
        let v = fx.rand_from(t, late(0x01E0_2B64));
        fx.mem.set16(t + 0x56, v.wrapping_rem(8));
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 6, 5);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x22 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 {
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0);
    } else {
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0xC0);
    }
    let c = fx.mem.i16(t + 0x56);
    fx.mem.set16(t + 0x56, if c < 7 { c + 1 } else { 0 });
    age(fx, t);
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, late(0x01E0_2C4C));
    let a = fx.mem.i16(t + 0x5A) + 0xA;
    fx.mem.set16(t + 0x5A, a);
    let a = fx.mem.i16(t + 0x5A);
    if a >= 0x168 {
        fx.mem.set16(t + 0x5A, a.wrapping_rem(0x168));
    }
    fx.leave(0x20);
}

/// 0x1E02CA0: a dark burst ring (size rand % 40 + 50, cell (rand >> 4) %
/// 5 stepping to 8 where it ends, radius rand % 250 + 100, random angles,
/// rows rand % 8 + 1), 0x80 for rand % 10 + 13 updates, then out by 0xB.
fn ring_y(fx: &mut Fx, r: &Rec, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, r.task_ra);
    let l = |a: u32| a - r.late;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rand_mod(fx, t, l(0x01E0_2CC4), 0x28);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x32));
        let v = fx.rand_from(t, l(0x01E0_2CF0));
        let (_, h) = fx.div(v >> 4, 5);
        fx.mem.set16(t + 0x56, h);
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
        let v = rand_mod(fx, t, l(0x01E0_2D20), 0xFA);
        fx.mem.set16(t + 0x50, v + 0x64);
        for (o, ra) in [(0x5Au32, l(0x01E0_2D40)), (0x5C, l(0x01E0_2D60))] {
            let v = fx.rand_from(t, ra);
            let (_, h) = fx.div(v >> 4, 0x168);
            fx.mem.set16(t + o, h);
        }
        let v = rand_mod(fx, t, l(0x01E0_2D80), 0xF);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = fx.rand_from(t, l(0x01E0_2DA0));
        fx.mem.set16(t + 8, v.wrapping_rem(8) + 1);
        let v = rand_mod(fx, t, l(0x01E0_2DC0), 0xA);
        fx.mem.set16(t + 6, v + 0xD);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t);
    step_cell(fx, t);
    let n = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, n);
    age(fx, t);
    fx.leave(0x20);
}

/// From the second update the cell +0x56 steps on; the task ends at 8.
fn step_cell(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) != 0 {
        let c = fx.mem.i16(t + 0x56) + 1;
        fx.mem.set16(t + 0x56, c);
        if fx.mem.i16(t + 0x56) >= 8 {
            fx.mem.set16(t + 0x58, 0);
        }
    }
}

/// 0x1E02E80: a grey ring at the second node point (size 2, cell 2 stepping
/// to 8 where it ends, radius rand % 80 + 20, random angles, rows rand % 8
/// + 1), 0x80 for rand % 10 + 13 updates, then out by 0xB; grey by age.
fn ring_z(fx: &mut Fx, r: &Rec, t: u32) {
    enter_step_with(fx, 0x20, 1, VALS, r.task_ra);
    let l = |a: u32| a - r.late;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4000_0000);
        fx.mem.set16(t + 0x56, 2);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rand_mod(fx, t, l(0x01E0_2EC8), 0x50);
        fx.mem.set16(t + 0x50, v + 0x14);
        for (o, ra) in [(0x5Au32, l(0x01E0_2EE8)), (0x5C, l(0x01E0_2F08))] {
            let v = fx.rand_from(t, ra);
            let (_, h) = fx.div(v >> 4, 0x168);
            fx.mem.set16(t + o, h);
        }
        let v = rand_mod(fx, t, l(0x01E0_2F28), 0xF);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = fx.rand_from(t, l(0x01E0_2F48));
        fx.mem.set16(t + 8, v.wrapping_rem(8) + 1);
        let v = rand_mod(fx, t, l(0x01E0_2F68), 0xA);
        fx.mem.set16(t + 6, v + 0xD);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t);
    step_cell(fx, t);
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0x80 } else { 0x60 };
    l2::rgb(fx, t + 0x60, g, g, g);
    let n = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, n);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03C90: the stage light (0x1F7F10) +0x50 from 0x80 down by 10 to
/// 0x30, back up by 10 to 0x80 once the timer is below 0x15 (then the task
/// ends).
fn light(fx: &mut Fx, r: &Rec, t: u32) {
    let sp = fx.enter(0x10);
    fx.saved(sp, &[(0, r.task_ra)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let v = fx.mem.i16(t + 0x50);
    if fx.mem.i32(TIMER) >= 0x15 {
        fx.mem.set16(t + 0x50, if v < 0x31 { 0x30 } else { v - 0xA });
    } else {
        if v < 0x80 {
            fx.mem.set16(t + 0x50, v + 0xA);
        } else {
            fx.mem.set16(t + 0x50, 0x80);
            fx.mem.set16(t + 0x58, 0);
        }
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    let v = fx.mem.read_u8(t + 0x50);
    fx.set_stage_light(i32::from(v));
    fx.leave(0x10);
}

/// 0x1E03D50: b178's window sprite (0x6FF8 .. 0x8FF8) sampling the frame
/// at ST 0 .. 1, colour `window_rgb` with alpha +0x63, key 0; kind +0x50 0:
/// ALPHA 0x44, in by 8 below 0x50 (see [`Rec::window_by_timer`]), then out
/// by 7; 1: ALPHA 0x48, in by 0x28 below 0x50 for 5 updates, then out by
/// 0xC.
fn window(fx: &mut Fx, r: &Rec, t: u32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, r.task_ra), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for (k, c) in r.window_rgb.into_iter().enumerate() {
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
    let w = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x50, ee::div(w, 512.0));
    let w = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(w, 512.0));
    let w = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(512.0, w), 512.0));
    let w = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(512.0, w), 512.0));
    // 0x207EF0's a2 / a3: what 0x207E20 left (the flags with / without
    // 0x200), as in b178's window.
    let flags = u32::from(fx.mem.read_u16(d + 2));
    if fx.mem.i16(t + 0x50) == 0 {
        fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    }
    if fx.mem.i16(t + 0x50) == 1 {
        fx.desc_alpha(d, (0x80u64 << 32) | 0x48);
    }
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(t + 0x50) == 0 {
        let fade = if r.window_by_timer { fx.mem.i32(TIMER) >= 0x15 } else { fx.mem.i16(COUNTER) < 0x140 };
        if fade {
            fade_in(fx, t, 0x63, (0x50, 8));
        } else {
            fade_out(fx, t, 0x63, (9, 7));
        }
    }
    if fx.mem.i16(t + 0x50) == 1 {
        if fx.mem.i16(t + 0x54) < 5 {
            fade_in(fx, t, 0x63, (0x50, 0x28));
        } else {
            fade_out(fx, t, 0x63, (0xE, 0xC));
        }
    }
    age(fx, t);
    fx.leave(0x90);
}
