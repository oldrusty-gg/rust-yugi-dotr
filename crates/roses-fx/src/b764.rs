//! BMODULE b764 (card 458), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03EC0, data area 0x1ED0000) [F]: b733's (card 82) family on b178's pool of 250 tasks of 0x70 bytes
//! at D+0x2A0 (alive +0x58, step +0x44 through `jalr`; spawn 0x1E04D30 =
//! b733's with the colour D+0xDAC4, D+0xDBAE into +0x5A and the index
//! D+0xDBD4 into +0x50 / +0x52), no unit flash. Drawn with arcs (0x1E009B0,
//! `billboard::arc_strips`), turned quad rings (0x1E00D10,
//! `billboard::ring_quads_turned` on the bytes D+0xDCDE / D+0xDDDE),
//! camera-facing quads (0x1E01430, `billboard::quad`), the full-window tint
//! (0x1E016F0, `quads4::tint4`), trails as the blended tube 0x1E05200
//! (`tubetrail::tube_with(SINGLE_BLEND)`; b733's 0x1E01430 word for word
//! but the addresses) along 20-point histories D+0x7030 + 0x140 k of
//! relative heads D+0xD430 + 0x10 k, each drawn twice (at the two points,
//! the second turned by +0x8000 / +0x4000 about x), and the record's own
//! petal sprite 0x1E04EE0 (an arc piece of 10 degrees between the radii
//! +0x48 / +0x4C on the cells D+0xDB18 through `uv::grid_rect`). Globals
//! past D+0xD000: three points D+0xD930 / D+0xD940 / D+0xD950 at the
//! target (y 45 / 45 / 35; from update 0xEE the first rising to 65, the
//! second falling to 30), six node points D+0xDA00 + 0x10 k (velocities
//! D+0xD960 + 0x10 k), textures D+0xDAA0.., timer D+0xDAC0 = 439, OT
//! D+0xDACC, tint w D+0xDAE4, nodes D+0xDAF8.., the petal count D+0xDBAA,
//! angle D+0xDBAE, trail counter D+0xDBB2, eight glow angles D+0xDBBC..,
//! index D+0xDBD4, facing D+0xDBD8, counter D+0xDBDC, bytes D+0xDBDE /
//! D+0xDCDE / D+0xDDDE, sines D+0xDEE0. The emitter 0x1E018C0: 0x3E..0x77
//! a ring 0x1E02E80 at node 0x5C (and 0x3E an arc 0x1E03BD0), 0x8A..0xA3
//! one at node 0x4D (and 0x97..0xA3 an arc 0x1E03BD0); 0x54..0x1A4 the 72
//! petals 0x1E02440 two a update; 0xA4..0x10D six rings 0x1E02B50 a update
//! (following node 0x4D); 0xDC six glows 0x1E02910 at node 0x4D drifting
//! to the points by update 0x108; 0x109 two arcs 0x1E02A60 and a grey tint
//! 0x1E02D80; 0x10E..0x1A4 at each point an arc 0x1E022D0 and a ring
//! 0x1E03D70 a update; 0x10F..0x195 a trail a update (0x1E035C0 on odd
//! updates, 0x1E02FC0 on even). Returns 1 / 4 / 2 / 6 / 8 on 0x102 / 0x1A4
//! / 0x19F / 0x10D..0x199 / timer out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::b590::project_residue;
use crate::blib::{billboard, quads4, spawn as bspawn, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xFA;
const ARC1: u32 = 0x01E0_22D0;
const PETAL: u32 = 0x01E0_2440;
const GLOW: u32 = 0x01E0_2910;
const ARC2: u32 = 0x01E0_2A60;
const RING_A: u32 = 0x01E0_2B50;
const TINT: u32 = 0x01E0_2D80;
const RING_B: u32 = 0x01E0_2E80;
const TRAIL_A: u32 = 0x01E0_2FC0;
const TRAIL_B: u32 = 0x01E0_35C0;
const ARC3: u32 = 0x01E0_3BD0;
const RING_C: u32 = 0x01E0_3D70;
/// The stub the entry skips (`jr ra`), never spawned.
const STUB: u32 = 0x01E0_3EB0;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_4388;
const HEADS: u32 = D + 0xD430;
const POINTS: u32 = D + 0xD930;
const VELS: u32 = D + 0xD960;
const NODEP: u32 = D + 0xDA00;
const TEX: u32 = D + 0xDAA0;
const TIMER: u32 = D + 0xDAC0;
const COLOUR: u32 = D + 0xDAC4;
const OT_AT: u32 = D + 0xDACC;
const TINT_W: u32 = D + 0xDAE4;
const NODES: u32 = D + 0xDAF8;
const CELLS: u32 = D + 0xDB18;
const PETALS: u32 = D + 0xDBAA;
const ANGLE: u32 = D + 0xDBAE;
const TRAIL_K: u32 = D + 0xDBB2;
const GLOW_A: u32 = D + 0xDBBC;
const INDEX: u32 = D + 0xDBD4;
const FACING: u32 = D + 0xDBD8;
const COUNTER: u32 = D + 0xDBDC;
const BYTES: u32 = D + 0xDBDE;
const BYTES_B: u32 = D + 0xDCDE;
const BYTES_C: u32 = D + 0xDDDE;
const SINES: u32 = D + 0xDEE0;
/// The scratchpad, where the originals project.
const SPR: u32 = 0x7000_0000;

/// The globals the spawn, the petal sprite and the trails read (b574's
/// sit elsewhere).
pub(crate) struct Glob {
    pub colour: u32,
    pub angle: u32,
    pub index: u32,
    pub cells: u32,
    pub sines: u32,
    pub ot_at: u32,
}

const G: Glob = Glob { colour: COLOUR, angle: ANGLE, index: INDEX, cells: CELLS, sines: SINES, ot_at: OT_AT };

pub(crate) fn trail_of(k: i32) -> u32 {
    (D as i32 + 0x7030 + 0x140 * k) as u32
}

fn head_of(k: i32) -> u32 {
    (HEADS as i32 + 0x10 * k) as u32
}

fn point(k: u32) -> u32 {
    POINTS + 0x10 * k
}

/// The entry 0x1E03EC0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x170);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x170);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xE050);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xDBDA, param);
    fx.mem.set32(TIMER, 0x1B7);
    for (k, n) in [0x4D, 0x5C, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xDB08, r);
    let tex = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, tex);
    for k in 0..4u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.fix_residue(v, 0x01E0_40B8);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    // (table, from, to, ra, m, add)
    let fills = [
        (BYTES, 0u32, 0x100u32, 0x01E0_40E8u32, 0x1E, -0xF),
        (BYTES_B, 0, 0x100, 0x01E0_417C, 0x1E, -0xF),
        (BYTES_B, 100, 0xB4, 0x01E0_41DC, 0x14, 0xA),
        (BYTES_C, 0, 0x100, 0x01E0_4234, 0x1E, -0xF),
    ];
    for (n, (table, from, to, ra, m, add)) in fills.into_iter().enumerate() {
        for k in from..to {
            let r = fx.rand_from(D, ra);
            let (_, h) = fx.div(r >> 4, m);
            fx.mem.write_u8(table + k, (h + add) as u8);
        }
        if n == 0 {
            for k in 100..0xB4u32 {
                fx.mem.write_u8(BYTES + k, 0);
            }
        }
    }
    for k in 0..0x24u32 {
        let r = fx.rand_from(D, 0x01E0_4290);
        fx.mem.set16(CELLS + 2 * k, r.wrapping_rem(8));
    }
    for k in 0..8u32 {
        fx.mem.set16(GLOW_A + 2 * k, 0);
    }
    0
}

/// The atlas cell `id` (0x106960 into `r`, frame +0x56) at D+0x94..+0xA3.
pub(crate) fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

pub(crate) fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn arcs(fx: &mut Fx, t: u32, n: i32, mode: i32) {
    fx.mem.set16(D + 0xB8, n);
    billboard::arc_strips(fx, D, t, mode, t + 0x10);
    fx.mem.set16(D + 0xB8, 0);
}

/// A turned quad ring: texture D+0xDAA4, cell 0x14 into `r`, `n` quads in
/// `layout` on `table`.
fn ring(fx: &mut Fx, t: u32, r: u32, n: i32, layout: i32, table: u32) {
    let w = fx.mem.read_u32(TEX + 4);
    fx.mem.write_u32(D + 0x88, w);
    cell(fx, t, r, 0x14);
    fx.mem.set16(D + 0xB8, n);
    fx.mem.set16(D + 0xB0, layout);
    billboard::ring_quads_turned(fx, uv::uv_rect, D, t, 0x20, table);
    fx.mem.set16(D + 0xB8, 0);
    fx.mem.set16(D + 0xB0, 0);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 || f == STUB {
            continue;
        }
        run_task(fx, f, slot + 0x2A0);
    }
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == ARC1 {
            arcs(fx, t, 0xC, 0x10);
        }
        if f == PETAL {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x80, 0x14);
            fx.mem.set16(D + 0xB8, 1);
            fx.mem.set16(D + 0xBE, 0);
            let k = fx.mem.i16(t + 0x50);
            let at = if k < 0x24 {
                Some(0)
            } else if k < 0x48 {
                Some(1)
            } else if k < 0x6C {
                Some(2)
            } else {
                None
            };
            if let Some(p) = at {
                petal(fx, &G, t, 0x20, point(p));
            }
        }
        if f == GLOW {
            glow_draw(fx, t, sp);
        }
        if f == ARC2 {
            arcs(fx, t, 0x24, 0x21);
        }
        if f == RING_A {
            ring(fx, t, sp + 0xA0, 7, 1, BYTES_B);
        }
        if f == TINT {
            let h = fx.target_handle();
            fx.unit_position(h, sp + 0x70);
            // 0x1F71A0's frame below the entry's (its saved ra, s0 = D).
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_4860), (0, u64::from(D))]);
            let z = fx.mem.f(sp + 0x78);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 60.0) } else { ee::add(z, 60.0) };
            fx.mem.setf(sp + 0x78, z);
            let p = [fx.mem.f(sp + 0x70), fx.mem.f(sp + 0x74), fx.mem.f(sp + 0x78)];
            let mut x = fx.mem.read_u64(sp + 0xF8);
            if let Some(w) = fx.project(p, None, &mut x) {
                fx.mem.write_u64(sp + 0xF8, x);
                fx.mem.setf(TINT_W, w);
            }
            project_residue(fx, sp, sp + 0x70);
            let w = fx.mem.f(TINT_W);
            quads4::tint4(fx, t, 0x21, w);
        }
        if f == RING_B {
            ring(fx, t, sp + 0xB0, 4, 0, BYTES_C);
        }
        if f == TRAIL_A || f == TRAIL_B {
            let turn = if f == TRAIL_A { 0x8000 } else { 0x4000 };
            trail_draw(fx, t, turn);
        }
        if f == ARC3 {
            arcs(fx, t, 0x18, 0x20);
        }
        if f == RING_C {
            ring(fx, t, sp + 0xC0, 4, 0, BYTES_C);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x102 => 1,
        0x1A4 => 4,
        0x19F => 2,
        0x10D..=0x199 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The glow 0x1E02910's draw: texture D+0xDAA4, cell 0x14, D+0xBE = 5, a
/// quad at its node point +0x50 (k) pushed 50 (k 0 / 3: +, 1 / 4: -) along
/// x or 20 (k 2: +, 5: -) along y by the sine of its angle D+0xDBBC + 2k.
fn glow_draw(fx: &mut Fx, t: u32, sp: u32) {
    let w = fx.mem.read_u32(TEX + 4);
    fx.mem.write_u32(D + 0x88, w);
    cell(fx, t, sp + 0x90, 0x14);
    fx.mem.set16(D + 0xBE, 5);
    let k = fx.mem.i16(t + 0x50);
    let p = (NODEP as i32 + 0x10 * k) as u32;
    for c in [0u32, 4, 8] {
        let v = fx.mem.read_u32(p + c);
        fx.mem.write_u32(sp + 0x70 + c, v);
    }
    // (k, column, size, add)
    let pushes = [(0, 0u32, 50.0f32, true), (1, 0, 50.0, false), (2, 4, 20.0, true), (3, 0, 50.0, true), (4, 0, 50.0, false), (5, 4, 20.0, false)];
    for (j, c, s, add) in pushes {
        if fx.mem.i16(t + 0x50) != j {
            continue;
        }
        let a = fx.mem.i16((GLOW_A as i32 + 2 * j) as u32);
        let v = l2::sin_deg(fx, D + 0xD8, a, 0);
        let m = ee::mul(s, v);
        let x = fx.mem.f(sp + 0x70 + c);
        fx.mem.setf(sp + 0x70 + c, if add { ee::add(x, m) } else { ee::sub(x, m) });
    }
    billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x70);
}

/// A trail's draw: texture D+0xDAAC, the full cell, D+0xBE = 1; its tube
/// at the second point, then at the first with the angle +0x20 turned by
/// `turn` (put back after).
fn trail_draw(fx: &mut Fx, t: u32, turn: u32) {
    let w = fx.mem.read_u32(TEX + 0xC);
    fx.mem.write_u32(D + 0x88, w);
    for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x3F)] {
        fx.mem.set16(D + o, v);
    }
    fx.mem.set16(D + 0xBE, 1);
    for (p, turned) in [(point(1), false), (point(0), true)] {
        for c in [0u32, 4, 8] {
            let w = fx.mem.read_u32(p + c);
            fx.mem.write_u32(D + 0x20 + c, w);
        }
        let a = fx.mem.read_u32(t + 0x20);
        if turned {
            fx.mem.write_u32(t + 0x20, a.wrapping_add(turn));
        }
        let k = fx.mem.i16(t + 0x50);
        tubetrail::tube_with(fx, &tubetrail::SINGLE_BLEND, D, t, trail_of(k), 0x21);
        if turned {
            fx.mem.write_u32(t + 0x20, a);
        }
    }
}

/// 0x1E04EE0(D, t, mode, pos): one petal, a strip3 piece of 10 degrees
/// from the angle +0x5A - 200 between the radii +0x48 / +0x4C in the x / z
/// plane, coloured +0x60 / +0x64, UVs D+0x94 in D+0xBE's orientation then
/// re-set from `uv::grid_rect` of the cell D+0xDB18 [+0x50 % 36]; through
/// TRS(angles +0x20, pos) x the view-projection; keyed by its mean w on
/// D+0xDACC; blend (mode >> 4).
pub(crate) fn petal(fx: &mut Fx, g: &Glob, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1D0);
    copy4(fx, sp + 0x80, pos);
    let d = sp + 0x140;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 1);
    l2::colour_strip3(fx, d, t + 0x64, 2);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    fx.desc_alpha(d, crate::c061::blend_alpha(mode >> 4));
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x80 + 4 * k);
        fx.mem.write_u32(sp + 0xD0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xE0, 0, t + 0x20, sp + 0xD0);
    for o in [0x94u32, 0xA4, 0xB4, 0xC4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let r0 = fx.mem.f(t + 0x48);
    let r1 = fx.mem.f(t + 0x4C);
    let i = fx.mem.i16(t + 0x50).wrapping_rem(0x24);
    let c = fx.mem.i16(g.cells + 2 * i as u32);
    uv::grid_rect(fx, sp + 0x130, c);
    copy4(fx, D + 0x94, sp + 0x130);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    // (extra degrees, the inner / outer vertices)
    for (e, inner, outer) in [(0, 0xB0u32, 0x90u32), (10, 0xC0, 0xA0)] {
        let a = (fx.mem.i16(t + 0x5A) + e - 200) as i16 as i32;
        let cs = l2::sin_deg(fx, g.sines, a, 1);
        let sn = l2::sin_deg(fx, g.sines, a, 0);
        fx.mem.setf(sp + inner, ee::mul(cs, r0));
        fx.mem.setf(sp + inner + 8, ee::mul(sn, r0));
        fx.mem.setf(sp + outer, ee::mul(cs, r1));
        fx.mem.setf(sp + outer + 8, ee::mul(sn, r1));
    }
    if l2::project(fx, sp + 0x90, SPR, sp + 0xE0, d, sp + 0x120) {
        let key = ee::fixunssfsi(fx.mem.f(sp + 0x120)) as i32;
        let ot = fx.mem.read_u32(g.ot_at);
        fx.submit(d, ot, key);
    }
    fx.leave(0x1D0);
}

fn run_task(fx: &mut Fx, f: u32, t: u32) {
    match f {
        ARC1 => arc1(fx, t),
        PETAL => petal_step(fx, t),
        GLOW => glow(fx, t),
        ARC2 => arc2(fx, t),
        RING_A => ring_a(fx, t),
        TINT => tint(fx, t),
        RING_B => ring_hold(fx, t, &RING_B_CFG),
        TRAIL_A => trail(fx, t, &TRAIL_A_CFG, SINES, [u64::from(D), 0], TASK_RA),
        TRAIL_B => trail(fx, t, &TRAIL_B_CFG, SINES, [u64::from(D), 0], TASK_RA),
        ARC3 => arc3(fx, t),
        RING_C => ring_hold(fx, t, &RING_C_CFG),
        _ => panic!("b764: unknown task step {f:#x}"),
    }
}

/// 0x1E018C0(D, 0): the update counter and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let d = u64::from(D);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_4348), (0x20, p), (0x10, 0), (0, d)]);
    // The emitter's s0 / s2 as the spawns and node calls save them (s1 =
    // D; s3 / s4 the runner's, 0).
    let (mut s0, mut s2) = (d, p);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 1 {
        for (k, y, ra) in [(0u32, 0x4234_0000u32, 0x01E0_1924u32), (1, 0x4234_0000, 0x01E0_1948), (2, 0x420C_0000, 0x01E0_196C)] {
            let h = fx.target_handle();
            unit_position_saving(fx, h, point(k), d, ra);
            fx.mem.write_u32(point(k) + 4, y);
        }
    }
    if fx.mem.i16(COUNTER) == 0xDC {
        for k in 0..8u32 {
            let h = fx.self_handle();
            let s = fx.mem.i16(NODES);
            node_position_saving(fx, h, s, NODEP + 0x10 * k, u64::from(k), Some(d), 0x01E0_19C4);
        }
        (s0, s2) = (8, d + 0x80);
    }
    if fx.mem.i16(COUNTER) >= 0xEE {
        let y = fx.mem.f(point(0) + 4);
        if y < 65.0 {
            fx.mem.setf(point(0) + 4, ee::add(y, 1.0));
        }
        let y = fx.mem.f(point(1) + 4);
        if y > 30.0 {
            fx.mem.setf(point(1) + 4, ee::sub(y, 1.0));
        }
    }
    for k in 0..6u32 {
        let n = fx.mem.i16(COUNTER);
        if n < 0xDC {
            continue;
        }
        let q = NODEP + 0x10 * k;
        let v = VELS + 0x10 * k;
        if n == 0xDC {
            let j = k % 2;
            let z = ee::sub(fx.mem.f(point(j) + 8), fx.mem.f(q + 8));
            fx.mem.setf(v + 8, ee::div(z, 45.0));
            let to = if j == 0 { 65.0 } else { 30.0 };
            let y = ee::sub(to, fx.mem.f(q + 4));
            fx.mem.setf(v + 4, ee::div(y, 45.0));
        }
        let n = fx.mem.i16(COUNTER);
        if (0xDD..0x109).contains(&n) {
            let z = ee::add(fx.mem.f(q + 8), fx.mem.f(v + 8));
            fx.mem.setf(q + 8, z);
            let y = ee::add(fx.mem.f(q + 4), fx.mem.f(v + 4));
            fx.mem.setf(q + 4, y);
        }
    }
    if fx.mem.i16(COUNTER) == 0xDC {
        for k in 0..6u32 {
            copy4(fx, D + 0x250, NODEP + 0x10 * k);
            fx.mem.set16(INDEX, k as i32);
            spawn(fx, GLOW, [d + 0x10 * u64::from(k), d, u64::from(k), 0, 0], 0x01E0_1C68);
        }
        (s0, s2) = (d + 0x60, 6);
    }
    if fx.mem.i16(COUNTER) >= 0xDD {
        for k in 0..8u32 {
            let a = fx.mem.i16(GLOW_A + 2 * k);
            if a < 0xB4 {
                fx.mem.set16(GLOW_A + 2 * k, a + 4);
            }
        }
    }
    let r = |s0: u64, s2: u64| [s0, d, s2, 0, 0];
    if fx.mem.i16(COUNTER) == 0x109 {
        copy4(fx, D + 0x250, point(0));
        fx.mem.set16(INDEX, 0);
        spawn(fx, ARC2, r(s0, s2), 0x01E0_1D68);
        copy4(fx, D + 0x250, point(1));
        spawn(fx, ARC2, r(s0, s2), 0x01E0_1DA4);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x54..0x1A5).contains(&n) && fx.mem.i16(PETALS) < 0x24 {
        let k = fx.mem.i16(PETALS);
        fx.mem.set16(ANGLE, k * 10);
        let k = fx.mem.i16(PETALS);
        fx.mem.set16(INDEX, k);
        spawn(fx, PETAL, r(s0, s2), 0x01E0_1E24);
        let k = fx.mem.i16(PETALS);
        fx.mem.set16(ANGLE, k * 10 + 0xB4);
        let k = fx.mem.i16(PETALS);
        fx.mem.set16(INDEX, k + 0x24);
        spawn(fx, PETAL, r(s0, s2), 0x01E0_1E7C);
        let k = fx.mem.i16(PETALS) + 1;
        fx.mem.set16(PETALS, k);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x10E..0x1A5).contains(&n) {
        for (j, ra) in [(0u32, [0x01E0_1EF8u32, 0x01E0_1F0C]), (1, [0x01E0_1F48, 0x01E0_1F5C])] {
            copy4(fx, D + 0x250, point(j));
            spawn(fx, ARC1, r(s0, s2), ra[0]);
            spawn(fx, RING_C, r(s0, s2), ra[1]);
        }
    }
    let n = fx.mem.i16(COUNTER);
    if (0x10F..0x196).contains(&n) {
        let odd = fx.mem.i16(COUNTER).wrapping_rem(2) == 1;
        let i = fx.mem.i16(TRAIL_K);
        fx.mem.set16(INDEX, i);
        if odd {
            spawn(fx, TRAIL_B, r(0, s2), 0x01E0_1FE0);
        } else {
            spawn(fx, TRAIL_A, r(0, s2), 0x01E0_2050);
        }
        let i = fx.mem.i16(TRAIL_K) + 1;
        fx.mem.set16(TRAIL_K, i);
        if fx.mem.i16(TRAIL_K) >= 0x50 {
            fx.mem.set16(TRAIL_K, 0);
        }
        s0 = 1;
    }
    let n = fx.mem.i16(COUNTER);
    if (0xA4..0x10E).contains(&n) {
        for k in 0..6u64 {
            spawn(fx, RING_A, r(k, s2), 0x01E0_20F0);
        }
        s0 = 6;
    }
    if fx.mem.i16(COUNTER) == 0x109 {
        spawn(fx, TINT, r(s0, s2), 0x01E0_2144);
    }
    // (from, to, node slot, node ra, step, index, spawn ra)
    let rings = [
        (0x3Ei32, 0x78i32, 1u32, 0x01E0_2188u32, RING_B, None, 0x01E0_219Cu32),
        (0x8A, 0xA4, 0, 0x01E0_21E0, RING_B, None, 0x01E0_21F4),
        (0x3E, 0x3F, 1, 0x01E0_222C, ARC3, Some(0), 0x01E0_224C),
        (0x97, 0xA4, 0, 0x01E0_2290, ARC3, Some(1), 0x01E0_22B4),
    ];
    for (from, to, slot, nra, f, index, ra) in rings {
        let n = fx.mem.i16(COUNTER);
        if !(from..to).contains(&n) {
            continue;
        }
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES + 2 * slot);
        node_position_saving(fx, h, s, D + 0x250, s0, Some(d), nra);
        if let Some(i) = index {
            fx.mem.set16(INDEX, i);
        }
        spawn(fx, f, r(s0, s2), ra);
    }
    fx.leave(0x40);
}

/// 0x1E04D30(D, step, 0): b733's spawn on this record's globals (250
/// tasks); `regs` the caller's s0..s4.
fn spawn(fx: &mut Fx, f: u32, regs: [u64; 5], ra: u32) -> i32 {
    spawn_with(fx, &G, f, regs, ra)
}

/// 0x1E04D30 on the globals `g`.
pub(crate) fn spawn_with(fx: &mut Fx, g: &Glob, f: u32, regs: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, regs[4]), (0x30, regs[3]), (0x20, regs[2]), (0x10, regs[1]), (0, regs[0])]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, gr, b) = (fx.mem.read_u8(g.colour), fx.mem.read_u8(g.colour + 1), fx.mem.read_u8(g.colour + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(gr), i32::from(b));
                let a = fx.mem.read_u8(g.colour + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            // 0x107390's `sq ra` upper half (0) (b232's).
            fx.mem.write_u64(sp - 8, 0);
            for c in [0u32, 4, 8] {
                let w = fx.mem.read_u32(D + 0x250 + c);
                fx.mem.write_u32(t + 0x10 + c, w);
            }
            let a = fx.mem.i16(g.angle);
            fx.mem.set16(t + 0x5A, a);
            let i = fx.mem.i16(g.index);
            fx.mem.set16(t + 0x50, i);
            let i = fx.mem.i16(g.index);
            fx.mem.set16(t + 0x52, i);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand % m (HI) from a step's rand returning to `ra` (s0 = the task).
pub(crate) fn rand_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

pub(crate) fn fade_in(fx: &mut Fx, t: u32, a: u32, up: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if v < up.0 {
        fx.mem.write_u8(t + a, v.wrapping_add(up.1));
    }
}

/// Down by `down.1` while at least `down.0`, else the task ends.
pub(crate) fn fade_out(fx: &mut Fx, t: u32, a: u32, down: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if v >= down.0 {
        fx.mem.write_u8(t + a, v.wrapping_sub(down.1));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

pub(crate) fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// A step's frame with the entry's s0 = D (and s1 = 0 when `n` is 2)
/// saved, ra at `frame - 0x10`.
fn enter_step(fx: &mut Fx, frame: u32, n: u32) -> u32 {
    enter_step_with(fx, frame, n, [u64::from(D), 0], TASK_RA)
}

/// A step's frame with the entry's s0 / s1 `vals` (the first `n`) saved,
/// `ra` at `frame - 0x10`.
pub(crate) fn enter_step_with(fx: &mut Fx, frame: u32, n: u32, vals: [u64; 2], ra: u64) -> u32 {
    let sp = fx.enter(frame);
    for k in 0..n {
        fx.saved(sp + 0x10 * k, &[(0, vals[k as usize])]);
    }
    fx.saved(sp, &[(frame - 0x10, ra)]);
    sp
}

/// 0x1E022D0: b733's arc 0x1E02D30 at a point (inner radius 70, outer 100
/// + rand % 80 growing by 10, grey over violet, turns rand % 0xFFFF), the
/// outer alpha in by 0x50 below 0xA0 for 3 updates, then out by 0xC.
fn arc1(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x428C_0000);
        let r = fx.rand_from(t, 0x01E0_22F8);
        let (_, h) = fx.div(r >> 4, 0x50);
        fx.mem.setf(t + 0x4C, ee::add(100.0, ee::from_i32(h)));
        l2::rgb(fx, t + 0x60, 0x70, 0x70, 0x70);
        l2::rgb(fx, t + 0x64, 0x40, 0x20, 0x30);
        for (o, ra) in [(0x20u32, 0x01E0_2354), (0x24, 0x01E0_2370), (0x28, 0x01E0_238C)] {
            let v = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 0x1E);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 3 {
        fade_in(fx, t, 0x67, (0xA0, 0x50));
    } else {
        fade_out(fx, t, 0x67, (0xE, 0xC));
    }
    let r = ee::add(fx.mem.f(t + 0x4C), 10.0);
    fx.mem.setf(t + 0x4C, r);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02440: petal +0x50 (k) of 72 (k < 36 at the first point, else the
/// second): radii 60, both alphas in by 0xC below 0xC0 for 30 updates, out
/// by 0xB from update 0x19B; turned +-0x2000 about z (k < 36 / < 72) and
/// x (by the facing); its cell rand % 3 + 4; white / grey by age; the inner
/// radius down by 5 to 30, then the outer down by 5 to 20 (stages +0x52 0
/// / 1, k >= 36: 2 / 3; 4 / 5 up by 2.5 to 20 / 10); from update 0x119 the
/// y angle turning by +-0x300, on 0xEF..0x118 the x / z angles back by
/// 0x100 to 0.
fn petal_step(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4270_0000);
        fx.mem.write_u32(t + 0x4C, 0x4270_0000);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.set16(t + 0x58, 2);
        let k = fx.mem.i16(t + 0x50);
        if k < 0x24 {
            fx.mem.set16(t + 0x52, 0);
        } else if k < 0x48 {
            fx.mem.set16(t + 0x52, 2);
        } else if k < 0x6C {
            fx.mem.set16(t + 0x52, 4);
        }
        let k = fx.mem.i16(t + 0x50);
        if k < 0x24 {
            fx.mem.set32(t + 0x28, 0x2000);
        } else if k < 0x48 {
            fx.mem.set32(t + 0x28, -0x2000);
        }
        let a = if fx.mem.i16(FACING) != 0 { 0x2000 } else { -0x2000 };
        fx.mem.set32(t + 0x20, a);
        let v = rand_mod(fx, t, 0x01E0_251C, 3);
        fx.mem.set16(t + 8, v + 4);
    }
    if fx.mem.i16(t + 0x54) < 0x1E {
        fade_in(fx, t, 0x63, (0xC0, 0xC));
        fade_in(fx, t, 0x67, (0xC0, 0xC));
    } else if fx.mem.i16(COUNTER) >= 0x19B {
        fade_out(fx, t, 0x63, (0xD, 0xB));
        let v = fx.mem.read_u8(t + 0x67);
        if v >= 0xD {
            fx.mem.write_u8(t + 0x67, v - 0xB);
        }
    }
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0xFF } else { 0x80 };
    l2::rgb(fx, t + 0x60, g, g, g);
    l2::rgb(fx, t + 0x64, g, g, g);
    if fx.mem.i16(COUNTER) != 0 {
        let s = fx.mem.i16(t + 0x52);
        let r0 = fx.mem.f(t + 0x48);
        let r1 = fx.mem.f(t + 0x4C);
        match s {
            0 | 2 => {
                if r0 <= 30.0 {
                    fx.mem.set16(t + 0x52, s + 1);
                } else {
                    fx.mem.setf(t + 0x48, ee::sub(r0, 5.0));
                }
            }
            1 | 3 => {
                if r1 > 20.0 {
                    fx.mem.setf(t + 0x4C, ee::sub(r1, 5.0));
                }
            }
            4 => {
                if r0 < 20.0 {
                    fx.mem.setf(t + 0x48, ee::add(r0, 2.5));
                } else {
                    fx.mem.set16(t + 0x52, s + 1);
                }
            }
            5 => {
                if r1 < 10.0 {
                    fx.mem.setf(t + 0x4C, ee::add(r1, 2.5));
                }
            }
            _ => {}
        }
    }
    let n = fx.mem.i16(COUNTER);
    if n >= 0x119 {
        let k = fx.mem.i16(t + 0x50);
        let d = if k < 0x24 {
            Some(0x300)
        } else if k < 0x48 {
            Some(-0x300)
        } else if k < 0x6C {
            Some(0x300)
        } else {
            None
        };
        if let Some(d) = d {
            let v = fx.mem.i32(t + 0x24).wrapping_add(d);
            fx.mem.set32(t + 0x24, v);
        }
    } else if n >= 0xEF {
        for o in [0x28u32, 0x20] {
            let v = fx.mem.i32(t + o);
            if v > 0 {
                fx.mem.set32(t + o, v - 0x100);
            }
            let v = fx.mem.i32(t + o);
            if v < 0 {
                fx.mem.set32(t + o, v + 0x100);
            }
        }
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02910: glow +0x50 (k): +8 / +0xA 15, cell rand % 6 + 2, +0x34 0.5,
/// its alpha in by 0x60 below 0xC0 to update 0x108, then out by 9; white /
/// light grey by age; facing the camera (0x205580 into +0x20).
fn glow(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xF);
        fx.mem.set16(t + 0xA, 0xF);
        let v = rand_mod(fx, t, 0x01E0_2944, 6);
        fx.mem.set16(t + 0x56, v + 2);
        fx.mem.write_u32(t + 0x34, 0x3F00_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) < 0x109 {
        fade_in(fx, t, 0x63, (0xC0, 0x60));
    } else {
        fade_out(fx, t, 0x63, (0xB, 9));
    }
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2A30);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02A60: an arc fan at a point (inner radius 0, outer 250, orange over
/// grey, 10-degree arcs), its outer alpha in by 0x30 below 0xC0 for 12
/// updates, then out by 0xC; facing the camera turned by 0x4000 about x.
fn arc2(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.write_u32(t + 0x4C, 0x437A_0000);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x40);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xC {
        fade_in(fx, t, 0x67, (0xC0, 0x30));
    } else {
        fade_out(fx, t, 0x67, (0xE, 0xC));
    }
    age(fx, t);
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2B28);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    fx.leave(0x20);
}

/// 0x1E02B50: a white quad ring riding node 0x4D (size 10 (5 before
/// update 0xA4), cell 3, turned -+0x4000 by the facing, radius rand % 10 +
/// 5 growing by rand % 8 + 3 (rand % 10 + 3 before 0xA4), rows rand % 20),
/// 0xC0 for rand % 2 + 1 updates, then out by 0xF.
fn ring_a(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2);
    if fx.mem.i16(t + 0x58) == 1 {
        let s = if fx.mem.i16(COUNTER) < 0xA4 { 0x40A0_0000 } else { 0x4120_0000 };
        fx.mem.write_u32(t + 0x48, s);
        fx.mem.set16(t + 0x56, 3);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let a = if fx.mem.i16(FACING) != 0 { -0x4000 } else { 0x4000 };
        fx.mem.set32(t + 0x20, a);
        let v = rand_mod(fx, t, 0x01E0_2BEC, 0xA);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rand_mod(fx, t, 0x01E0_2C0C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_2C28, 0x168);
        fx.mem.set16(t + 0x5C, v);
        if fx.mem.i16(COUNTER) < 0xA4 {
            let v = rand_mod(fx, t, 0x01E0_2C58, 0xA);
            fx.mem.set16(t + 0xE, v + 3);
        } else {
            let v = fx.rand_from(t, 0x01E0_2C80);
            fx.mem.set16(t + 0xE, v.wrapping_rem(8) + 3);
        }
        let v = rand_mod(fx, t, 0x01E0_2CA4, 0x14);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, 0x01E0_2CC0);
        fx.mem.set16(t + 6, v.wrapping_rem(2) + 1);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_out(fx, t);
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES);
    node_position_saving(fx, h, s, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_2D5C);
    fx.leave(0x30);
}

/// 0xC0 for +6 updates, then out by 0xF.
fn hold_out(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade_out(fx, t, 0x63, (0x11, 0xF));
    }
}

/// 0x1E02D80: b733's grey tint (both alphas in by 0x2A below 0x80 for 12
/// updates, then out by 0xC).
fn tint(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 6, 0xC);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, (0x80, 0x2A));
        fade_in(fx, t, 0x67, (0x80, 0x2A));
    } else {
        fade_out(fx, t, 0x63, (0xE, 0xC));
        let v = fx.mem.read_u8(t + 0x67);
        fx.mem.write_u8(t + 0x67, if v >= 0xE { v - 0xC } else { 0 });
    }
    age(fx, t);
    fx.leave(0x20);
}

/// A still quad ring (0x1E02E80 / 0x1E03D70): size, cell 4, grey, radius
/// rand % m + add, two random turns, rows rand % 20, 0xC0 for rand % 2 + 1
/// updates, then out by 0xF.
struct RingCfg {
    size: u32,
    grey: i32,
    m: i32,
    add: i32,
    /// The five rands' return addresses.
    ra: [u32; 5],
}

const RING_B_CFG: RingCfg = RingCfg { size: 0x4000_0000, grey: 0xFF, m: 0x1E, add: 8, ra: [0x01E0_2EC8, 0x01E0_2EE8, 0x01E0_2F04, 0x01E0_2F20, 0x01E0_2F3C] };
const RING_C_CFG: RingCfg = RingCfg { size: 0x4120_0000, grey: 0x80, m: 0x64, add: 0x1E, ra: [0x01E0_3DB8, 0x01E0_3DD8, 0x01E0_3DF4, 0x01E0_3E10, 0x01E0_3E2C] };

fn ring_hold(fx: &mut Fx, t: u32, c: &RingCfg) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, c.size);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, c.grey, c.grey, c.grey);
        let v = rand_mod(fx, t, c.ra[0], c.m);
        fx.mem.set16(t + 0x50, v + c.add);
        let v = rand_mod(fx, t, c.ra[1], 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, c.ra[2], 0x168);
        fx.mem.set16(t + 0x5C, v);
        let v = rand_mod(fx, t, c.ra[3], 0x14);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, c.ra[4]);
        fx.mem.set16(t + 6, v.wrapping_rem(2) + 1);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_out(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

/// The two trails (0x1E02FC0 / 0x1E035C0): their first jitter divided by
/// `div`, the y heading 90 (A) or (rand >> 4) % 360 (B).
pub(crate) struct TrailCfg {
    pub div: f32,
    pub heading_b: bool,
    /// The half width / the step: rand % m + add tenths.
    pub width: (i32, i32),
    pub step: (i32, i32),
    /// The grey on odd ages.
    pub dim: i32,
    /// The per-update jitter's divisor (tenths: 10).
    pub jdiv: f32,
    /// Init: three jitters, the half width, the step, the heading, (B: the
    /// y heading), the hold.
    pub init: [u32; 8],
    /// Per update: the three jitters.
    pub jitter: [u32; 3],
}

const TRAIL_A_CFG: TrailCfg = TrailCfg {
    div: 2.0,
    heading_b: false,
    width: (0x96, 0x1E),
    step: (0x55, 3),
    dim: 0x80,
    jdiv: 10.0,
    init: [0x01E0_3038, 0x01E0_3064, 0x01E0_3090, 0x01E0_31BC, 0x01E0_31FC, 0x01E0_323C, 0, 0x01E0_3260],
    jitter: [0x01E0_335C, 0x01E0_3388, 0x01E0_33B4],
};
const TRAIL_B_CFG: TrailCfg = TrailCfg {
    div: 10.0,
    heading_b: true,
    width: (0x96, 0x1E),
    step: (0x55, 3),
    dim: 0x80,
    jdiv: 10.0,
    init: [0x01E0_3638, 0x01E0_3664, 0x01E0_3690, 0x01E0_37BC, 0x01E0_37FC, 0x01E0_383C, 0x01E0_3858, 0x01E0_3878],
    jitter: [0x01E0_3974, 0x01E0_39A0, 0x01E0_39CC],
};

/// A trail +0x50 (k): a head D+0xD430 + 0x10 k from 0 jittered by rand %
/// 60 - 30 over `div`, then each update moved +0x4C (rand % 85 + 3 tenths)
/// along its headings (+0x5A in x / z, +0x5C in y) and jittered by rand %
/// 80 - 40 tenths, pushed into its 20-point history for 7 updates while
/// its length +6 grows to 0xF, then shrinking; half width rand % 150 + 30
/// tenths; 0xC0 for rand % 15 + 3 updates, then out by 0x1D; white / grey
/// by age.
pub(crate) fn trail(fx: &mut Fx, t: u32, c: &TrailCfg, sines: u32, vals: [u64; 2], ra: u64) {
    enter_step_with(fx, 0x30, 2, vals, ra);
    let headk = |fx: &mut Fx| head_of(fx.mem.i16(t + 0x50));
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 6, 1);
        fx.mem.write_u8(t + 0x63, 0);
        for o in [0u32, 4, 8] {
            let h = headk(fx);
            fx.mem.write_u32(h + o, 0);
        }
        for (o, ra) in [(0x30u32, c.init[0]), (0x34, c.init[1]), (0x38, c.init[2])] {
            let r = rand_mod(fx, t, ra, 0x3C);
            fx.mem.setf(t + o, ee::from_i32(r - 0x1E));
        }
        for o in [0u32, 4, 8] {
            let j = ee::div(fx.mem.f(t + 0x30 + o), c.div);
            let h = headk(fx);
            let v = ee::add(fx.mem.f(h + o), j);
            fx.mem.setf(h + o, v);
        }
        let k = fx.mem.i16(t + 0x50);
        let (tr, h) = (trail_of(k), head_of(k));
        for j in 0..0x14u32 {
            for o in [0u32, 4, 8] {
                let w = fx.mem.read_u32(h + o);
                fx.mem.write_u32(tr + 0x10 * j + o, w);
            }
        }
        let r = rand_mod(fx, t, c.init[3], c.width.0);
        fx.mem.setf(t + 0x48, ee::from_i32(r + c.width.1));
        let v = ee::div(fx.mem.f(t + 0x48), 10.0);
        fx.mem.setf(t + 0x48, v);
        let r = rand_mod(fx, t, c.init[4], c.step.0);
        fx.mem.setf(t + 0x4C, ee::from_i32(r + c.step.1));
        let v = ee::div(fx.mem.f(t + 0x4C), 10.0);
        fx.mem.setf(t + 0x4C, v);
        let r = rand_mod(fx, t, c.init[5], 0x168);
        fx.mem.set16(t + 0x5A, r);
        if c.heading_b {
            let r = fx.rand_from(t, c.init[6]);
            let (_, a) = fx.div(r >> 4, 0x168);
            fx.mem.set16(t + 0x5C, a);
        } else {
            fx.mem.set16(t + 0x5C, 0x5A);
        }
        let r = rand_mod(fx, t, c.init[7], 0xF);
        fx.mem.set16(t + 8, r + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let n = fx.mem.i16(t + 6);
    if fx.mem.i16(t + 0x54) < 7 {
        if n < 0xF {
            fx.mem.set16(t + 6, n + 1);
        }
    } else if n > 0 {
        fx.mem.set16(t + 6, n - 1);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 8) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade_out(fx, t, 0x63, (0x1F, 0x1D));
    }
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) != 0 { c.dim } else { 0xFF };
    l2::rgb(fx, t + 0x60, g, g, g);
    for (o, ra) in [(0x30u32, c.jitter[0]), (0x34, c.jitter[1]), (0x38, c.jitter[2])] {
        let r = rand_mod(fx, t, ra, 0x50);
        fx.mem.setf(t + o, ee::from_i32(r - 0x28));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        // (column, heading, cos)
        for (o, a, cos) in [(8u32, 0x5Au32, 0), (0, 0x5A, 1), (4, 0x5C, 0)] {
            let a = fx.mem.i16(t + a);
            let s = l2::sin_deg(fx, sines, a, cos);
            let m = ee::mul(fx.mem.f(t + 0x4C), s);
            let h = headk(fx);
            let v = ee::add(fx.mem.f(h + o), m);
            fx.mem.setf(h + o, v);
        }
        for o in [0u32, 4, 8] {
            let j = ee::div(fx.mem.f(t + 0x30 + o), c.jdiv);
            let h = headk(fx);
            let v = ee::add(fx.mem.f(h + o), j);
            fx.mem.setf(h + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 7 {
        let k = fx.mem.i16(t + 0x50);
        bspawn::history(fx, trail_of(k) + 0x130, 0x14, head_of(k));
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03BD0: an arc fan riding its spawn point (+0x50 0: radius 50, ochre
/// over light grey, in for 50 updates; 1: radius 18, +0x34 2, dark grey, in
/// for 4; 2: radius 20, +0x34 2, the spawn's colours, in for 4), its outer
/// alpha in by 0x30 below 0xC0, then out by 0xB; 15-degree arcs facing the
/// camera turned by 0x4000 about x.
fn arc3(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        match fx.mem.i16(t + 0x50) {
            0 => {
                fx.mem.write_u32(t + 0x48, 0);
                fx.mem.write_u32(t + 0x4C, 0x4248_0000);
                fx.mem.set16(t + 6, 0x32);
            }
            k @ (1 | 2) => {
                fx.mem.write_u32(t + 0x48, 0);
                fx.mem.write_u32(t + 0x4C, if k == 1 { 0x4190_0000 } else { 0x41A0_0000 });
                fx.mem.write_u32(t + 0x34, 0x4000_0000);
                fx.mem.set16(t + 6, 4);
            }
            _ => {}
        }
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x40);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x64, 0x40, 0x40, 0x40);
            l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x20);
        }
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x67, (0xC0, 0x30));
    } else {
        fade_out(fx, t, 0x67, (0xD, 0xB));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_3D40);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x20);
}
