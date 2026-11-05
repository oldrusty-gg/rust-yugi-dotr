//! BMODULE b733 (card 82), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E038B0, data area 0x1ED0000) [F]: b264's family on a pool of 200 tasks of 0x70 bytes at D+0x2A0
//! (alive +0x58, step +0x44 through `jalr`; spawn 0x1E04240 with the
//! colour D+0xC3D4, the index D+0xC486 into +0x50 / +0x52 and D+0xC474
//! into +0x5A), no unit flash. Drawn with turned quad rings (0x1E009B0,
//! `billboard::ring_quads_turned` on the bytes D+0xC490 / D+0xC590), arcs
//! (0x1E010D0, `billboard::arc_strips`), the full-window tint (0x1E01F80,
//! `quads4::tint4`), trails drawn as the blended tube 0x1E01430
//! (`tubetrail::tube_with(SINGLE_BLEND)`) along 20-point histories
//! D+0x5A50 + 0x140 k of relative heads D+0xBE50 + 0x10 k, and the record's
//! own petal sprite 0x1E043F0 (a strip3 quad on the cell table D+0xC428
//! through `uv::grid_rect`). Globals past D+0xB000: the target's point
//! D+0xC350 (followed to update 0x96), textures D+0xC3B0.., timer D+0xC3D0
//! = 260, OT D+0xC3DC, tint w D+0xC3F4, nodes D+0xC408.., the petal count
//! D+0xC472, trail counter D+0xC478, facing D+0xC48A, counter D+0xC48E,
//! bytes D+0xC490 / D+0xC590, sines D+0xC690. The emitter 0x1E02150: 0x15
//! ..0x3C four quad rings 0x1E02970 at node 0x34 (z +-0.5 by the facing);
//! 0x58..0x96 two rings (0x1E036C0, 0x1E02EA0) at node 0x23; 0x4B..0x12C
//! the 36 petals 0x1E02510 one a update; 0xA5 a grey tint 0x1E02C30;
//! 0xA6..0xF0 three trails 0x1E03040, an arc 0x1E02D30 and a ring 0x1E027D0
//! at the target a update. The step 0x1E02B40 is never spawned [F]: not
//! ported, nor its draw case. Returns 1 / 3 / 6 / 8 on 0xA0 / 0xFA /
//! 0xAB..0xEF / timer out.
use crate::b590::project_residue;
use crate::blib::{billboard, quads4, spawn as bspawn, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xC8;
const PETAL: u32 = 0x01E0_2510;
const RING1: u32 = 0x01E0_27D0;
const RING2: u32 = 0x01E0_2970;
const ARC_N: u32 = 0x01E0_2B40;
const TINT: u32 = 0x01E0_2C30;
const ARC: u32 = 0x01E0_2D30;
const RING3: u32 = 0x01E0_2EA0;
const TRAIL: u32 = 0x01E0_3040;
const RING4: u32 = 0x01E0_36C0;
/// The stub the entry skips (`jr ra`), never spawned.
const STUB: u32 = 0x01E0_38A0;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3CA8;
const HEADS: u32 = D + 0xBE50;
const POINT: u32 = D + 0xC350;
const TEX: u32 = D + 0xC3B0;
const TIMER: u32 = D + 0xC3D0;
const COLOUR: u32 = D + 0xC3D4;
const OT_AT: u32 = D + 0xC3DC;
const TINT_W: u32 = D + 0xC3F4;
const NODES: u32 = D + 0xC408;
const CELLS: u32 = D + 0xC428;
const PETALS: u32 = D + 0xC472;
const ANGLE: u32 = D + 0xC474;
const TRAIL_K: u32 = D + 0xC478;
const INDEX: u32 = D + 0xC486;
const FACING: u32 = D + 0xC48A;
const COUNTER: u32 = D + 0xC48E;
const BYTES: u32 = D + 0xC490;
const BYTES_B: u32 = D + 0xC590;
const SINES: u32 = D + 0xC690;
/// The scratchpad, where the originals project.
const SPR: u32 = 0x7000_0000;

fn trail_of(k: i32) -> u32 {
    (D as i32 + 0x5A50 + 0x140 * k) as u32
}

fn head_of(k: i32) -> u32 {
    (HEADS as i32 + 0x10 * k) as u32
}

/// The entry 0x1E038B0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xC800);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xC48C, param);
    fx.mem.set32(TIMER, 0x104);
    for (k, n) in [1, 0x23, 0x34, 0x42, 0x46, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xC418, r);
    let tex = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, tex);
    for k in 0..3u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(v, 0x01E0_3AA4);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_3AD4);
        let (_, h) = fx.div(r, 0x1E);
        fx.mem.write_u8(BYTES + k, (h - 0xF) as u8);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_3B24);
        let (_, h) = fx.div(r, 0x28);
        fx.mem.write_u8(BYTES_B + k, (h - 0x14) as u8);
    }
    for k in 100..0xB4u32 {
        let r = fx.rand_from(D, 0x01E0_3B7C);
        let (_, h) = fx.div(r, 0x28);
        fx.mem.write_u8(BYTES_B + k, h as u8);
    }
    for k in 0..0x24u32 {
        let r = fx.rand_from(D, 0x01E0_3BD0);
        fx.mem.set16(CELLS + 2 * k, (r >> 4).wrapping_rem(8));
    }
    for (o, v) in [(0xC47Au32, 0), (0xC47C, 0x5A), (0xC47E, 0xB4), (0xC480, 0xB4)] {
        fx.mem.set16(D + o, v);
    }
    0
}

/// The atlas cell `id` (0x106960 into `r`, frame +0x56) at D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
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
        if f == PETAL {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x70, 0x14);
            fx.mem.set16(D + 0xB8, 1);
            fx.mem.set16(D + 0xBE, 0);
            petal(fx, t, 0x21, POINT);
        }
        // (step, rect local, mode, layout, table)
        let rings = [(RING1, 0x80u32, 0x21, 0, BYTES), (RING2, 0x90, 0x20, 1, BYTES_B)];
        for (g, r, mode, layout, table) in rings {
            if f == g {
                ring(fx, t, sp + r, mode, layout, table);
            }
        }
        debug_assert_ne!(f, ARC_N);
        if f == TINT {
            let h = fx.target_handle();
            fx.unit_position(h, sp + 0x60);
            // 0x1F71A0's frame below the entry's (its saved ra, s0 = D).
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_3EE8), (0, u64::from(D))]);
            let z = fx.mem.f(sp + 0x68);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 60.0) } else { ee::add(z, 60.0) };
            fx.mem.setf(sp + 0x68, z);
            let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
            let mut x = fx.mem.read_u64(sp + 0xE8);
            if let Some(w) = fx.project(p, None, &mut x) {
                fx.mem.write_u64(sp + 0xE8, x);
                fx.mem.setf(TINT_W, w);
            }
            project_residue(fx, sp, sp + 0x60);
            let w = fx.mem.f(TINT_W);
            quads4::tint4(fx, t, 0x21, w);
        }
        if f == ARC {
            fx.mem.set16(D + 0xB8, 0xC);
            billboard::arc_strips(fx, D, t, 0x10, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == RING3 {
            ring(fx, t, sp + 0xA0, 0x21, 1, BYTES_B);
        }
        if f == TRAIL {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x3F)] {
                fx.mem.set16(D + o, v);
            }
            for c in [0u32, 4, 8] {
                let w = fx.mem.read_u32(POINT + c);
                fx.mem.write_u32(D + 0x20 + c, w);
            }
            fx.mem.set16(D + 0xBE, 1);
            let k = fx.mem.i16(t + 0x50);
            tubetrail::tube_with(fx, &tubetrail::SINGLE_BLEND, D, t, trail_of(k), 0x21);
        }
        if f == RING4 {
            ring(fx, t, sp + 0xB0, 0x21, 0, BYTES_B);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xA0 => 1,
        0xFA => 3,
        0xAB..=0xEF => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// A ring of quads: texture D+0xC3B4, cell 0x14 into `r`, five quads in
/// `layout` through ring_quads_turned.
fn ring(fx: &mut Fx, t: u32, r: u32, mode: i32, layout: i32, table: u32) {
    let w = fx.mem.read_u32(TEX + 4);
    fx.mem.write_u32(D + 0x88, w);
    cell(fx, t, r, 0x14);
    fx.mem.set16(D + 0xB8, 5);
    fx.mem.set16(D + 0xB0, layout);
    billboard::ring_quads_turned(fx, uv::uv_rect, D, t, mode, table);
    fx.mem.set16(D + 0xB8, 0);
    fx.mem.set16(D + 0xB0, 0);
}

/// 0x1E043F0(D, t, mode, pos): one petal, a strip3 quad of half size +0x48
/// coloured +0x60 (top) / +0x64 (bottom), UVs D+0x94 in D+0xBE's
/// orientation then re-set from `uv::grid_rect` of the cell D+0xC428 [+0x50
/// % 36]; the quad turned about z by +0x5C (the record's sines) and pushed
/// +0x4C along the angle +0x5A - 200 degrees, through TRS(angles +0x20, pos)
/// x the view-projection; keyed by its mean w on D+0xC3DC; blend (mode >>
/// 4) (0: ALPHA 0x42), the depth bits unread.
fn petal(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1E0);
    copy4(fx, sp + 0x80, pos);
    let d = sp + 0x150;
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
    let r = fx.mem.f(t + 0x4C);
    let a = fx.mem.i16(t + 0x5C);
    fx.mem.set32(sp + 0x138, a);
    let i = fx.mem.i16(t + 0x50).wrapping_rem(0x24);
    let c = fx.mem.i16(CELLS + 2 * i as u32);
    uv::grid_rect(fx, sp + 0x140, c);
    copy4(fx, D + 0x94, sp + 0x140);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let a = (fx.mem.i16(t + 0x5A) - 200) as i16 as i32;
    let x = ee::mul(r, l2::sin_deg(fx, SINES, a, 1));
    let y = ee::mul(r, l2::sin_deg(fx, SINES, a, 0));
    let s = fx.mem.f(t + 0x48);
    l2::quad_xy(fx, sp + 0x90, s, s);
    l2::rotate4(fx, sp + 0x90, sp + 0x130, 2, SINES);
    l2::add_column(fx, sp + 0x90, 0, x);
    l2::add_column(fx, sp + 0x90, 1, y);
    if l2::project(fx, sp + 0x90, SPR, sp + 0xE0, d, sp + 0x120) {
        let key = ee::fixunssfsi(fx.mem.f(sp + 0x120)) as i32;
        let ot = fx.mem.read_u32(OT_AT);
        fx.submit(d, ot, key);
    }
    fx.leave(0x1E0);
}

fn run_task(fx: &mut Fx, f: u32, t: u32) {
    match f {
        PETAL => petal_step(fx, t),
        RING1 => ring1(fx, t),
        RING2 => ring2(fx, t),
        TINT => tint(fx, t),
        ARC => arc(fx, t),
        RING3 => ring3(fx, t),
        TRAIL => trail(fx, t),
        RING4 => ring4(fx, t),
        _ => panic!("b733: unknown task step {f:#x}"),
    }
}

/// 0x1E02150(D, 0): the update counter and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    let d = u64::from(D);
    fx.saved(sp, &[(0x20, 0x01E0_3C68), (0x10, 0), (0, d)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The spawns' saved s0..s4: the emitter's s0 (D, the trail loop's
    // counter), s1 = D, the entry's s2 (param), s3 / s4 (the runner's, 0).
    let r = |s0: u64| [s0, d, param as i64 as u64, 0, 0];
    if fx.mem.i16(COUNTER) < 0x97 {
        let h = fx.target_handle();
        fx.unit_position(h, POINT);
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_21AC), (0, d)]);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x58..0x97).contains(&n) {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES + 2);
        crate::b076::node_position_saving(fx, h, s, D + 0x250, d, Some(d), 0x01E0_21F0);
        spawn(fx, RING4, r(d), 0x01E0_2204);
        spawn(fx, RING3, r(d), 0x01E0_2218);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x15..0x3D).contains(&n) {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES + 4);
        crate::b076::node_position_saving(fx, h, s, D + 0x250, d, Some(d), 0x01E0_225C);
        let z = fx.mem.f(D + 0x258);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 0.5) } else { ee::add(z, 0.5) };
        fx.mem.setf(D + 0x258, z);
        for ra in [0x01E0_22B8u32, 0x01E0_22CC, 0x01E0_22E0, 0x01E0_22F4] {
            spawn(fx, RING2, r(d), ra);
        }
    }
    let n = fx.mem.i16(COUNTER);
    let mut s0 = d;
    if (0xA6..0xF1).contains(&n) {
        for k in 0..3u64 {
            let i = fx.mem.i16(TRAIL_K);
            fx.mem.set16(INDEX, i);
            spawn(fx, TRAIL, r(k), 0x01E0_234C);
            let i = fx.mem.i16(TRAIL_K) + 1;
            fx.mem.set16(TRAIL_K, i);
            if fx.mem.i16(TRAIL_K) >= 0x50 {
                fx.mem.set16(TRAIL_K, 0);
            }
        }
        s0 = 3;
    }
    if fx.mem.i16(COUNTER) == 0xA5 {
        spawn(fx, TINT, r(s0), 0x01E0_23E4);
    }
    let n = fx.mem.i16(COUNTER);
    if (0xA6..0xF1).contains(&n) {
        copy4(fx, D + 0x250, POINT);
        spawn(fx, ARC, r(s0), 0x01E0_2444);
        spawn(fx, RING1, r(s0), 0x01E0_2458);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x4B..0x12D).contains(&n) {
        let k = fx.mem.i16(PETALS);
        if k < 0x24 {
            fx.mem.set16(ANGLE, k * 10);
            let k = fx.mem.i16(PETALS);
            fx.mem.set16(INDEX, k);
            spawn(fx, PETAL, r(s0), 0x01E0_24D8);
            let k = fx.mem.i16(PETALS) + 1;
            fx.mem.set16(PETALS, k);
        }
    }
    fx.leave(0x30);
}

/// 0x1E04240(D, step, 0): b264's spawn on this record's globals (200
/// tasks); `regs` the caller's s0..s4.
fn spawn(fx: &mut Fx, f: u32, regs: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, regs[4]), (0x30, regs[3]), (0x20, regs[2]), (0x10, regs[1]), (0, regs[0])]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            // 0x107390's `sq ra` upper half (0) (b232's).
            fx.mem.write_u64(sp - 8, 0);
            for c in [0u32, 4, 8] {
                let w = fx.mem.read_u32(D + 0x250 + c);
                fx.mem.write_u32(t + 0x10 + c, w);
            }
            let i = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x50, i);
            let i = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x52, i);
            let a = fx.mem.i16(ANGLE);
            fx.mem.set16(t + 0x5A, a);
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
fn rand_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

fn fade_in(fx: &mut Fx, t: u32, a: u32, up: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if v < up.0 {
        fx.mem.write_u8(t + a, v.wrapping_add(up.1));
    }
}

/// Down by `down.1` while at least `down.0`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, a: u32, down: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if v >= down.0 {
        fx.mem.write_u8(t + a, v.wrapping_sub(down.1));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// A step's frame with the entry's s0 = D (and s1 = 0 when `n` is 2)
/// saved, ra at `frame - 0x10`.
fn enter_step(fx: &mut Fx, frame: u32, n: u32) -> u32 {
    let sp = fx.enter(frame);
    let vals = [u64::from(D), 0];
    for k in 0..n {
        fx.saved(sp + 0x10 * k, &[(0, vals[k as usize])]);
    }
    fx.saved(sp, &[(frame - 0x10, TASK_RA)]);
    sp
}

/// 0x1E02510: petal +0x50 (k) of 36: size 1.5 (5.0 / radius 25 in group
/// 0 from its second update), both alphas in by 0xC below 0xC0 for 30
/// updates, out by 0xB from update 0xF1; its cell rand % 3 + 4 (+8), the
/// frame +6 stepping every tenth update; from update 0xA1 the angle +0x5A
/// turning by +-2, the spin +0x5C by +-3 (odd / even k).
fn petal_step(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3FC0_0000);
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
        let v = rand_mod(fx, t, 0x01E0_2590, 3);
        fx.mem.set16(t + 8, v + 4);
        fx.mem.set16(t + 6, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    }
    if fx.mem.i16(t + 0x54) < 0x1E {
        fade_in(fx, t, 0x63, (0xC0, 0xC));
        fade_in(fx, t, 0x67, (0xC0, 0xC));
    } else if fx.mem.i16(COUNTER) >= 0xF1 {
        fade_out(fx, t, 0x63, (0xD, 0xB));
        let v = fx.mem.read_u8(t + 0x67);
        if v >= 0xD {
            fx.mem.write_u8(t + 0x67, v - 0xB);
        }
    }
    let a = fx.mem.i16(t + 0x54);
    if a >= 0x1E && a.wrapping_rem(10) == 1 {
        let n = fx.mem.i16(t + 6) + 1;
        fx.mem.set16(t + 6, n);
        if fx.mem.i16(t + 6) >= 6 {
            fx.mem.set16(t + 6, 0);
        }
    }
    if fx.mem.i16(COUNTER) != 0 {
        match fx.mem.i16(t + 0x52) {
            0 => {
                fx.mem.write_u32(t + 0x4C, 0x41C8_0000);
                fx.mem.write_u32(t + 0x48, 0x40A0_0000);
            }
            2 => {
                fx.mem.write_u32(t + 0x4C, 0x40E0_0000);
                fx.mem.write_u32(t + 0x48, 0x3F4C_CCCD);
            }
            _ => {}
        }
    }
    if fx.mem.i16(COUNTER) >= 0xA1 {
        let k = fx.mem.i16(t + 0x50);
        if k < 0x24 {
            let v = fx.mem.i16(t + 0x5A) + 2;
            fx.mem.set16(t + 0x5A, v);
        } else if k < 0x48 {
            let v = fx.mem.i16(t + 0x5A) - 2;
            fx.mem.set16(t + 0x5A, v);
        }
    }
    let d = if fx.mem.i16(t + 0x50).wrapping_rem(2) == 1 { 3 } else { -3 };
    let v = fx.mem.i16(t + 0x5C) + d;
    fx.mem.set16(t + 0x5C, v);
    age(fx, t);
    fx.leave(0x30);
}

/// The common tail of the rings: 0xC0 for +6 updates, then out by 9.
fn hold_out(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade_out(fx, t, 0x63, (0xB, 9));
    }
}

fn grow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
}

/// 0x1E027D0: a white quad ring at the target (size 2, cell 4, turns rand
/// % 0xFFFF, radius rand % 220 + 20, rand % 30 + 15 set but not grown,
/// rows rand % 20), 0xC0 for rand % 3 + 2 updates, then out by 9.
fn ring1(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4000_0000);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        for (o, ra) in [(0x20u32, 0x01E0_2818), (0x24, 0x01E0_2834), (0x28, 0x01E0_2850)] {
            let v = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let v = rand_mod(fx, t, 0x01E0_286C, 0xDC);
        fx.mem.set16(t + 0x50, v + 0x14);
        let v = rand_mod(fx, t, 0x01E0_288C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_28A8, 0x1E);
        fx.mem.set16(t + 0xE, v + 0xF);
        let v = rand_mod(fx, t, 0x01E0_28C8, 0x14);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_28E4, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_out(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02970: a white quad ring (size 4, cell 4, turned -+0x4000 by the
/// facing, radius rand % 20 + 3 growing by rand % 5 + 2, rows rand % 30),
/// in by 0x40 for rand % 3 + 2 updates (age 10 on update 0x3C), then out
/// by 0xF.
fn ring2(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4080_0000);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let a = if fx.mem.i16(FACING) != 0 { -0x4000 } else { 0x4000 };
        fx.mem.set32(t + 0x20, a);
        let v = rand_mod(fx, t, 0x01E0_29E8, 0x14);
        fx.mem.set16(t + 0x50, v + 3);
        let v = rand_mod(fx, t, 0x01E0_2A08, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_2A24, 5);
        fx.mem.set16(t + 0xE, v + 2);
        let v = rand_mod(fx, t, 0x01E0_2A44, 0x1E);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_2A60, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) == 0x3C {
        fx.mem.set16(t + 0x54, 0xA);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, (0xC0, 0x40));
    } else {
        fade_out(fx, t, 0x63, (0x11, 0xF));
    }
    grow(fx, t);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02C30: the grey tint, both alphas in by 0x2A below 0x80 for 12
/// updates, then out by 0xC (b252's).
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

/// 0x1E02D30: a turned arc at the target (inner radius 100, outer 130 +
/// rand % 80 growing by 20, grey over violet, turns rand % 0xFFFF), the
/// outer alpha in by 0x50 below 0xA0 for 3 updates, then out by 0xC.
fn arc(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        let r = fx.rand_from(t, 0x01E0_2D58);
        let (_, h) = fx.div(r >> 4, 0x50);
        fx.mem.setf(t + 0x4C, ee::add(130.0, ee::from_i32(h)));
        l2::rgb(fx, t + 0x60, 0x70, 0x70, 0x70);
        l2::rgb(fx, t + 0x64, 0x40, 0x20, 0x30);
        for (o, ra) in [(0x20u32, 0x01E0_2DB4), (0x24, 0x01E0_2DD0), (0x28, 0x01E0_2DEC)] {
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
    let r = ee::add(fx.mem.f(t + 0x4C), 20.0);
    fx.mem.setf(t + 0x4C, r);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02EA0: a white quad ring at node 0x23 (size 3, cell 4, turned
/// -+0x2000 by the facing, radius rand % 40 + 10 growing by rand % 30 +
/// 15, rows rand % 20), 0xC0 for rand % 3 + 2 updates, then out by 9.
fn ring3(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2);
    if fx.mem.i16(t + 0x58) == 1 {
        ring_head(fx, t);
        let v = rand_mod(fx, t, 0x01E0_2F18, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rand_mod(fx, t, 0x01E0_2F38, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_2F54, 0x1E);
        fx.mem.set16(t + 0xE, v + 0xF);
        let v = rand_mod(fx, t, 0x01E0_2F74, 0x14);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_2F90, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_out(fx, t);
    grow(fx, t);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02EA0 / 0x1E036C0's first stores: size 3, cell 4, white, turned
/// -+0x2000 by the facing.
fn ring_head(fx: &mut Fx, t: u32) {
    fx.mem.write_u32(t + 0x48, 0x4040_0000);
    fx.mem.set16(t + 0x56, 4);
    l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    let a = if fx.mem.i16(FACING) != 0 { -0x2000 } else { 0x2000 };
    fx.mem.set32(t + 0x20, a);
}

/// 0x1E036C0: 0x1E02EA0's ring with the radius rand % 40 + 10 (index 0) /
/// rand % 20 + 10 (index 1), else 0, growing by rand % 15 + 6.
fn ring4(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2);
    if fx.mem.i16(t + 0x58) == 1 {
        ring_head(fx, t);
        if fx.mem.i16(t + 0x52) == 0 {
            let v = rand_mod(fx, t, 0x01E0_3744, 0x28);
            fx.mem.set16(t + 0x50, v + 0xA);
        }
        if fx.mem.i16(t + 0x52) == 1 {
            let v = rand_mod(fx, t, 0x01E0_3778, 0x14);
            fx.mem.set16(t + 0x50, v + 0xA);
        }
        let v = rand_mod(fx, t, 0x01E0_379C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_37B8, 0xF);
        fx.mem.set16(t + 0xE, v + 6);
        let v = rand_mod(fx, t, 0x01E0_37D8, 0x14);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_37F8, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_out(fx, t);
    grow(fx, t);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03040: trail +0x50 (k, +0x52 = k % 6): a head D+0xBE50 + 0x10 k
/// from 0 jittered by rand % 60 - 30 tenths and pushed 25 along the angle
/// rand % 360, then each update moved +0x4C (rand % 65 + 3 tenths) along
/// its angle +0x5A (turned rand % 30 + 165 once) and jittered by rand % 80
/// - 40 tenths in x / y, pushed into its 20-point history for 7 updates
/// while its length +6 grows to 0xF, then shrinking; half width rand % 60
/// + 3 tenths; 0xC0 for rand % 15 + 3 updates, then out by 0x1D; white /
/// grey by age.
fn trail(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2);
    let headk = |fx: &mut Fx| head_of(fx.mem.i16(t + 0x50));
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 6, 1);
        fx.mem.write_u8(t + 0x63, 0);
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x52, k.wrapping_rem(6));
        for c in [0u32, 4, 8] {
            let h = headk(fx);
            fx.mem.write_u32(h + c, 0);
        }
        for (c, ra) in [(0x30u32, 0x01E0_30D4), (0x34, 0x01E0_3100), (0x38, 0x01E0_312C)] {
            let r = rand_mod(fx, t, ra, 0x3C);
            fx.mem.setf(t + c, ee::from_i32(r - 0x1E));
        }
        for c in [0u32, 4, 8] {
            let j = ee::div(fx.mem.f(t + 0x30 + c), 10.0);
            let h = headk(fx);
            let v = ee::add(fx.mem.f(h + c), j);
            fx.mem.setf(h + c, v);
        }
        let r = fx.rand_from(t, 0x01E0_31E8);
        let (_, a) = fx.div(r >> 4, 0x168);
        fx.mem.set16(t + 0x5A, a);
        for (c, cos) in [(0u32, 1), (4, 0)] {
            let a = fx.mem.i16(t + 0x5A);
            let s = l2::sin_deg(fx, SINES, a, cos);
            let m = ee::mul(25.0, s);
            let h = headk(fx);
            let v = ee::add(fx.mem.f(h + c), m);
            fx.mem.setf(h + c, v);
        }
        let k = fx.mem.i16(t + 0x50);
        let (tr, h) = (trail_of(k), head_of(k));
        for j in 0..0x14u32 {
            for c in [0u32, 4, 8] {
                let w = fx.mem.read_u32(h + c);
                fx.mem.write_u32(tr + 0x10 * j + c, w);
            }
        }
        let r = rand_mod(fx, t, 0x01E0_3304, 0x3C);
        fx.mem.setf(t + 0x48, ee::from_i32(r + 3));
        let v = ee::div(fx.mem.f(t + 0x48), 10.0);
        fx.mem.setf(t + 0x48, v);
        let r = rand_mod(fx, t, 0x01E0_3344, 0x41);
        fx.mem.setf(t + 0x4C, ee::from_i32(r + 3));
        let v = ee::div(fx.mem.f(t + 0x4C), 10.0);
        fx.mem.setf(t + 0x4C, v);
        let r = rand_mod(fx, t, 0x01E0_3384, 0x1E);
        let a = fx.mem.i16(t + 0x5A) + ((r + 0xA5) as i16 as i32);
        fx.mem.set16(t + 0x5A, a);
        let r = fx.rand_from(t, 0x01E0_33B4);
        let (_, a) = fx.div(r >> 4, 0x168);
        fx.mem.set16(t + 0x5C, a);
        let r = rand_mod(fx, t, 0x01E0_33D4, 0xF);
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
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) != 0 { 0x80 } else { 0xFF };
    l2::rgb(fx, t + 0x60, g, g, g);
    for (c, ra) in [(0x30u32, 0x01E0_34D0), (0x34, 0x01E0_34FC), (0x38, 0x01E0_3528)] {
        let r = rand_mod(fx, t, ra, 0x50);
        fx.mem.setf(t + c, ee::from_i32(r - 0x28));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for (c, cos) in [(0u32, 1), (4, 0)] {
            let a = fx.mem.i16(t + 0x5A);
            let s = l2::sin_deg(fx, SINES, a, cos);
            let m = ee::mul(fx.mem.f(t + 0x4C), s);
            let h = headk(fx);
            let v = ee::add(fx.mem.f(h + c), m);
            fx.mem.setf(h + c, v);
        }
        for c in [0u32, 4] {
            let j = ee::div(fx.mem.f(t + 0x30 + c), 10.0);
            let h = headk(fx);
            let v = ee::add(fx.mem.f(h + c), j);
            fx.mem.setf(h + c, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 7 {
        let k = fx.mem.i16(t + 0x50);
        bspawn::history(fx, trail_of(k) + 0x130, 0x14, head_of(k));
    }
    age(fx, t);
    fx.leave(0x30);
}
