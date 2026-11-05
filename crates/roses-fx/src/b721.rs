//! BMODULE b721 (card 78), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E039E0, data area 0x1ED0000, 0x9BE0 bytes) [F]
//!: the b363 / b764 family on a pool of 100
//! tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step +0x44 through `jalr`;
//! spawn 0x1E044B0 = `spawn::task70` on 808b8d306d7e's layout: colour
//! D+0x97C4, the point D+0x250, the index D+0x9860 into +0x50). Drawn with
//! quad rings (0x1E00A90, `billboard::ring_quads` on the bytes D+0x996A /
//! D+0x986A, UVs uv_rect_q 0x1E00110), arc quads (0x1E01040,
//! `billboard::arc_quads`), flat arcs (0x1E00730,
//! `billboard::arc_strips_flat`), target tints (0x1E00560, `quads4::tint4`
//! at the target +- 30 projected into D+0x97D4), b363's band (0x1E05190 =
//! [`crate::b363::band`]), trails as the blended tube 0x1E04640
//! (`tubetrail::tube_with(SINGLE_BLEND)`) along 20-point histories D+0x2E90
//! + 0x140 k of heads D+0x9290 + 0x10 k, and the record's own petal ring
//! 0x1E055B0 (18 strip3 pieces of 20 degrees between the radii +0x48 /
//! +0x4C, each on the cell D+0x9814 [k]). Globals past D+0x9000: the point
//! P D+0x9790 (riding node 0x3D away from node 0x3E until update 0xAD,
//! then drifting along z and down to 44), textures (5, 0..3) D+0x97A0..,
//! (4, 0xA..0xF) D+0x288.., timer D+0x97C0 = 0x200, OT D+0x97CC, tint w
//! D+0x97D4, the growth F D+0x97D8 (30, +1 a update to 300 from update
//! 0x41, +10 on 0x140..0x14A), nodes D+0x97F4.. (0x3D clear / 0x3E / 0xD),
//! target 1 D+0x9804, cells D+0x9814, trail counter D+0x9856, the band
//! angle D+0x985A, index D+0x9860, facing D+0x9864, param D+0x9866,
//! counter D+0x9868, bytes D+0x986A / D+0x996A, sines D+0x9A6C. Returns 1
//! / 3 / 6 / 8 on 0x136 / 0x1E5 / 0x141..0x1DF / timer out.
use crate::b494::camera_angles_saving;
use crate::b590::project_residue;
use crate::blib::billboard::{self, uv_rect_q};
use crate::blib::{quads4, spawn as bspawn, tubetrail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 100;
const RING: u32 = 0x01E0_1D90;
const ARCQ: u32 = 0x01E0_1F20;
const PETALS: u32 = 0x01E0_20D0;
const RING_B: u32 = 0x01E0_23D0;
const BAND: u32 = 0x01E0_25B0;
const TRAIL: u32 = 0x01E0_2720;
const ARC: u32 = 0x01E0_2D30;
const ARC_B: u32 = 0x01E0_2FA0;
const BAND_B: u32 = 0x01E0_3270;
const TINT: u32 = 0x01E0_3400;
const CAMERA: u32 = 0x01E0_3590;
const LIGHT: u32 = 0x01E0_3720;
const WINDOW: u32 = 0x01E0_3800;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3D20;
const POOL: bspawn::Pool70 = bspawn::Pool70 { base: 0x2A0, n: 100, colour: 0x97C4, pos: 0x250, slot: 0x9860 };
const POS: u32 = D + 0x250;
const CAM: u32 = D + 0x2E60;
const P: u32 = D + 0x9790;
const TEX: u32 = D + 0x97A0;
const TIMER: u32 = D + 0x97C0;
const OT_AT: u32 = D + 0x97CC;
const TINT_W: u32 = D + 0x97D4;
const GROW: u32 = D + 0x97D8;
const NODES: u32 = D + 0x97F4;
const TARGET_NODE: u32 = D + 0x9804;
const CELLS: u32 = D + 0x9814;
const TRAIL_K: u32 = D + 0x9856;
const BAND_A: u32 = D + 0x985A;
const INDEX: u32 = D + 0x9860;
const FACING: u32 = D + 0x9864;
const COUNTER: u32 = D + 0x9868;
const BYTES: u32 = D + 0x986A;
const BYTES_B: u32 = D + 0x996A;
const SINES: u32 = D + 0x9A6C;
/// The scratchpad, where the originals project.
const SPR: u32 = 0x7000_0000;

fn hist(k: i32) -> u32 {
    (D as i32 + 0x2E90 + 0x140 * k) as u32
}

fn head(k: i32) -> u32 {
    (D as i32 + 0x9290 + 0x10 * k) as u32
}

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E039E0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x180);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x180);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x9BE0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x9866, param);
    fx.mem.set32(TIMER, 0x200);
    for (k, n) in [0x3D, 0x3E, 0xD].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for k in 0..6u32 {
        let tex = fx.texture(4, 0xA + k as i32);
        fx.mem.write_u32(D + 0x288 + 4 * k, tex);
    }
    for k in 0..4u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xE0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xE4)), 180.0);
    fx.fix_residue(v, 0x01E0_3B74);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    fx.mem.write_u32(GROW, 0x41F0_0000);
    for k in 0..0x100u32 {
        let r = fx.rand_from(k, 0x01E0_3BB4);
        let v = fx.div(r, 0x1E).1;
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 100..0xB4u32 {
        fx.mem.write_u8(BYTES + k, 0);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(k, 0x01E0_3C44);
        let v = fx.div(r, 0x1E).1;
        fx.mem.write_u8(BYTES_B + k, (v - 0xF) as u8);
    }
    for k in 0..0x1Eu32 {
        let r = fx.rand_from(k, 0x01E0_3C94);
        fx.mem.set16(CELLS + 2 * k, r.wrapping_rem(8));
    }
    0
}

/// The atlas cell `id` (0x106960 into `r`, frame +0x56) at D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, frame);
    copy4(fx, D + 0x94, r);
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 {
            continue;
        }
        run_task(fx, f, slot + 0x2A0);
    }
    let mut drawn = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        drawn = (drawn as i16).wrapping_add(1) as i32 as u32;
        draw(fx, sp, slot + 0x2A0, drawn);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x136 => 1,
        0x1E5 => 3,
        0x141..=0x1DF => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The ring draws' orientation D+0xBE = rand % 4 (rand's s0 = `drawn`).
fn ring_draw(fx: &mut Fx, t: u32, drawn: u32, ra: u32, mode: i32, table: u32) {
    fx.mem.set16(D + 0xB8, 5);
    let r = fx.rand_from(drawn, ra);
    fx.mem.set16(D + 0xBE, r.wrapping_rem(4));
    fx.mem.set16(D + 0xB0, 0);
    billboard::ring_quads(fx, uv_rect_q, D, t, mode, table);
    fx.mem.set16(D + 0xB8, 0);
}

/// The entry's draw case for task `t` (`drawn` its s0).
fn draw(fx: &mut Fx, sp: u32, t: u32, drawn: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    if f == RING {
        tex(fx, TEX + 0xC);
        cell(fx, t, sp + 0x70, 0x14);
        ring_draw(fx, t, drawn, 0x01E0_3DD0, 0x20, BYTES_B);
    }
    if f == ARCQ {
        tex(fx, D + 0x29C);
        if fx.mem.i16(t + 0x50) == 0 {
            cell(fx, t, sp + 0x80, 0x9B);
        } else {
            cell(fx, t, sp + 0x90, 0x97);
        }
        copy4(fx, sp + 0x60, t + 0x10);
        fx.mem.set16(D + 0xB8, 9);
        billboard::arc_quads(fx, uv_rect_q, D, t, 0x20, sp + 0x60);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == PETALS {
        tex(fx, TEX + 8);
        cell(fx, t, sp + 0xA0, 0x14);
        fx.mem.set16(D + 0xB8, 1);
        fx.mem.set16(D + 0xBE, 0);
        petals(fx, t, 0x20, t + 0x10);
    }
    if f == RING_B {
        tex(fx, TEX + 4);
        cell(fx, t, sp + 0xB0, 0x14);
        ring_draw(fx, t, drawn, 0x01E0_3FD0, 0x10, BYTES);
    }
    if f == BAND {
        tex(fx, D + 0x29C);
        cell(fx, t, sp + 0xC0, 0x8C);
        copy4(fx, sp + 0x60, P);
        crate::b363::band(fx, t, sp + 0x60, 0x20);
    }
    if f == TRAIL {
        tex(fx, TEX);
        for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x3F)] {
            fx.mem.set16(D + o, v);
        }
        for c in [0u32, 4, 8] {
            let w = fx.mem.read_u32(P + c);
            fx.mem.write_u32(D + 0x20 + c, w);
        }
        fx.mem.set16(D + 0xBE, 1);
        let k = fx.mem.i16(t + 0x50);
        tubetrail::tube_with(fx, &tubetrail::SINGLE_BLEND, D, t, hist(k), 0x21);
    }
    if f == ARC {
        copy4(fx, sp + 0x60, P);
        fx.mem.set16(D + 0xB8, 0x12);
        billboard::arc_strips_flat(fx, D, t, 0x10, sp + 0x60);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == ARC_B {
        copy4(fx, sp + 0x60, P);
        fx.mem.set16(D + 0xB8, 0x24);
        if fx.mem.i16(t + 0x50) == 0 {
            billboard::arc_strips_flat(fx, D, t, 0x10, sp + 0x60);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            billboard::arc_strips_flat(fx, D, t, 0x21, sp + 0x60);
        }
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == BAND_B {
        tex(fx, D + 0x29C);
        cell(fx, t, sp + 0xD0, 0x97);
        copy4(fx, sp + 0x60, P);
        crate::b363::band(fx, t, sp + 0x60, 0x20);
    }
    if f == TINT {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x60);
        // 0x1F71A0's frame below the entry's (its saved ra, s0 = drawn).
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_42E8), (0, u64::from(drawn))]);
        let z = fx.mem.f(sp + 0x68);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 30.0) } else { ee::add(z, 30.0) };
        fx.mem.setf(sp + 0x68, z);
        let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
        let mut x = fx.mem.read_u64(sp + 0x108);
        if let Some(w) = fx.project(p, None, &mut x) {
            fx.mem.write_u64(sp + 0x108, x);
            fx.mem.setf(TINT_W, w);
        }
        project_residue(fx, sp, sp + 0x60);
        let k = fx.mem.i16(t + 0x50);
        let mode = match k {
            0 => Some(1),
            1 | 2 => Some(0x20),
            _ => None,
        };
        if let Some(m) = mode {
            let w = fx.mem.f(TINT_W);
            quads4::tint4(fx, t, m, w);
        }
    }
}

fn run_task(fx: &mut Fx, f: u32, t: u32) {
    match f {
        RING => ring(fx, t),
        ARCQ => arcq(fx, t),
        PETALS => petals_step(fx, t),
        RING_B => ring_b(fx, t),
        BAND => band_step(fx, t),
        TRAIL => trail(fx, t),
        ARC => arc(fx, t),
        ARC_B => arc_b(fx, t),
        BAND_B => band_b(fx, t),
        TINT => tint(fx, t),
        CAMERA => camera(fx, t),
        LIGHT => light(fx, t),
        WINDOW => window(fx, t),
        _ => panic!("b721: unknown task step {f:#x}"),
    }
}

/// A step's prologue: `frame` bytes, the entry's s0 (0) and s1 (D) (the
/// first `n`) at +0 / +0x10, ra at `ra_at`.
fn enter_step(fx: &mut Fx, frame: u32, n: u32, ra_at: u32) -> u32 {
    let sp = fx.enter(frame);
    let vals = [0u64, u64::from(D)];
    for k in 0..n {
        fx.saved(sp + 0x10 * k, &[(0, vals[k as usize])]);
    }
    fx.saved(sp, &[(ra_at, TASK_RA)]);
    sp
}

/// 0x1E044B0(D, step, 0) from the emitter (its s0 / s1 = `s`, s2 = D, the
/// runner's s3 / s4 (0 [H])).
fn spawn(fx: &mut Fx, f: u32, s: [u64; 2], ra: u32) {
    bspawn::task70(fx, D, POOL, f, 0, &[(0, s[0]), (0x10, s[1]), (0x20, u64::from(D)), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
}

/// The trail counter into the index, then up by one, wrapping at 0x50.
fn trail_spawn(fx: &mut Fx, s: [u64; 2], ra: u32) {
    let k = fx.mem.i16(TRAIL_K);
    fx.mem.set16(INDEX, k);
    spawn(fx, TRAIL, s, ra);
    let k = fx.mem.i16(TRAIL_K) + 1;
    fx.mem.set16(TRAIL_K, k);
    if fx.mem.i16(TRAIL_K) >= 0x50 {
        fx.mem.set16(TRAIL_K, 0);
    }
}

/// 0x1E01490(D, 0) (frame 0x60: the entry's s0 = 0, s1 = D, s2 = the
/// param, ra).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x30, 0x01E0_3CF0), (0x20, param as i64 as u64), (0x10, u64::from(D)), (0, 0)]);
    let mut s: [u64; 2] = [0, u64::from(D)];
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    if counter(fx) == 1 {
        spawn(fx, WINDOW, s, 0x01E0_14E8);
    }
    if counter(fx) == 1 {
        spawn(fx, LIGHT, s, 0x01E0_1514);
    }
    if counter(fx) == 1 {
        spawn(fx, CAMERA, s, 0x01E0_1540);
    }
    for (c, i, ra) in [(0x41, 0, 0x01E0_1578u32), (0x3C, 1, 0x01E0_15B4), (0x44, 0, 0x01E0_15EC)] {
        if counter(fx) == c {
            fx.mem.set16(INDEX, i);
            spawn(fx, ARC_B, s, ra);
        }
    }
    let c = counter(fx);
    if c == 0xAD || c == 0x140 {
        fx.mem.set16(INDEX, 0);
        spawn(fx, TINT, s, 0x01E0_1630);
        fx.mem.set16(INDEX, 1);
        spawn(fx, TINT, s, 0x01E0_1654);
    }
    if counter(fx) == 0x1C2 {
        fx.mem.set16(INDEX, 0);
        spawn(fx, TINT, s, 0x01E0_168C);
        fx.mem.set16(INDEX, 2);
        spawn(fx, TINT, s, 0x01E0_16B0);
    }
    let c = counter(fx);
    if c < 0xAD {
        let h = fx.self_handle();
        let n0 = fx.mem.i16(NODES);
        crate::b076::node_position_saving(fx, h, n0, P, s[0], Some(s[1]), 0x01E0_16EC);
        let h = fx.self_handle();
        let n1 = fx.mem.i16(NODES + 2);
        crate::b076::node_position_saving(fx, h, n1, sp + 0x40, s[0], Some(s[1]), 0x01E0_170C);
        // P += P - node 0x3E's point (as the original).
        for c in [0u32, 4, 8] {
            let p = fx.mem.f(P + c);
            let d = ee::sub(p, fx.mem.f(sp + 0x40 + c));
            fx.mem.setf(P + c, ee::add(p, d));
        }
    } else if c < 0x140 && c >= 0xEC {
        let z = fx.mem.f(P + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, f32::from_bits(0x3FDC_28F6)) } else { ee::add(z, f32::from_bits(0x3FDA_E148)) };
        fx.mem.setf(P + 8, z);
        let y = fx.mem.f(P + 4);
        if !crate::exe::ee_le(y, 44.0) {
            fx.mem.setf(P + 4, ee::sub(y, f32::from_bits(0x3E99_999A)));
        }
    }
    if counter(fx) == 0x140 {
        fx.mem.write_u32(P + 4, 0x4230_0000);
    }
    let c = counter(fx);
    if (0x37..0x56).contains(&c) {
        copy4(fx, POS, P);
        spawn(fx, RING_B, s, 0x01E0_18D0);
    }
    if counter(fx) == 0x3E {
        copy4(fx, POS, P);
        let y = ee::add(fx.mem.f(POS + 4), 5.0);
        fx.mem.setf(POS + 4, y);
        for (i, ra) in [(0, 0x01E0_1944u32), (1, 0x01E0_1968), (2, 0x01E0_198C)] {
            fx.mem.set16(INDEX, i);
            spawn(fx, PETALS, s, ra);
        }
    }
    let c = counter(fx);
    if (0x3E..0x74).contains(&c) {
        for k in 0..3u64 {
            s[0] = k;
            trail_spawn(fx, s, 0x01E0_19E4);
        }
        s[0] = 3;
    }
    if counter(fx) >= 0x14A {
        copy4(fx, POS, P);
        if counter(fx) < 0x1C8 {
            spawn(fx, ARCQ, s, 0x01E0_1ABC);
        }
        let c = counter(fx);
        if (0x159..0x1C8).contains(&c) {
            spawn(fx, RING, s, 0x01E0_1AF4);
        }
    }
    if counter(fx) >= 0x41 {
        fx.mem.set16(INDEX, 0);
        if counter(fx) < 0x1CD {
            spawn(fx, ARC, s, 0x01E0_1B44);
        }
        if counter(fx) < 0x1CD {
            spawn(fx, BAND, s, 0x01E0_1B70);
        }
        let c = counter(fx);
        if (0xAD..0x13F).contains(&c) {
            spawn(fx, BAND_B, s, 0x01E0_1BA8);
        }
        if counter(fx) < 0x65 {
            spawn(fx, ARCQ, s, 0x01E0_1BD4);
        }
        let g = fx.mem.f(GROW);
        if crate::exe::ee_lt(g, 300.0) {
            fx.mem.setf(GROW, ee::add(g, 1.0));
        }
        let c = counter(fx);
        if (0x140..0x14B).contains(&c) {
            let g = fx.mem.f(GROW);
            fx.mem.setf(GROW, ee::add(g, 10.0));
        }
    }
    let c = counter(fx);
    if (0xB5..0x1D7).contains(&c) {
        let n = if c < 0x14A { 1 } else { 2 };
        s[0] = n;
        for k in 0..n {
            s[1] = k;
            trail_spawn(fx, s, 0x01E0_1CD8);
        }
    }
    if counter(fx) >= 0xDD {
        let a = fx.mem.i16(BAND_A) - 1;
        fx.mem.set16(BAND_A, a);
    }
    fx.leave(0x60);
}

fn rand_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// The alpha byte `a` down by `down.1` while at least `down.0`, else the
/// task ends.
fn fade_out(fx: &mut Fx, t: u32, a: u32, down: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if v >= down.0 {
        fx.mem.write_u8(t + a, v.wrapping_sub(down.1));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The alpha byte `a` up by `up.1` while below `up.0`.
fn fade_in(fx: &mut Fx, t: u32, a: u32, up: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if v < up.0 {
        fx.mem.write_u8(t + a, v.wrapping_add(up.1));
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn young(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6)
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x58) == 1
}

fn age_parity(fx: &mut Fx, t: u32) -> i32 {
    fx.mem.i16(t + 0x54).wrapping_rem(2)
}

/// The quad rings' radius rand % m + add, turn rand % 360, growth rand %
/// 20 + 16, row rand % 35 and life rand % 14 + 3.
fn ring_common(fx: &mut Fx, t: u32, ras: [u32; 5], radius: (i32, i32)) {
    let r = rand_mod(fx, t, ras[0], radius.0);
    fx.mem.set16(t + 0x50, r + radius.1);
    let r = rand_mod(fx, t, ras[1], 0x168);
    fx.mem.set16(t + 0x5A, r);
    let r = rand_mod(fx, t, ras[2], 0x14);
    fx.mem.set16(t + 0xE, r + 0x10);
    let r = rand_mod(fx, t, ras[3], 0x23);
    fx.mem.set16(t + 8, r);
    let r = rand_mod(fx, t, ras[4], 0xE);
    fx.mem.set16(t + 6, r + 3);
    fx.mem.set16(t + 0x58, 2);
}

/// 0x1E01D90: a white quad ring (size rand % 25 + 5, cell rand % 8, radius
/// rand % 100 + 50), in by 0x40 below 0xC0 for rand % 14 + 3 updates, then
/// out by 0xE.
fn ring(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0x10);
    if first(fx, t) {
        let r = rand_mod(fx, t, 0x01E0_1DB4, 0x19);
        fx.mem.setf(t + 0x48, ee::from_i32(r + 5));
        let r = fx.rand_from(t, 0x01E0_1DE0);
        fx.mem.set16(t + 0x56, r.wrapping_rem(8));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        ring_common(fx, t, [0x01E0_1E14, 0x01E0_1E34, 0x01E0_1E50, 0x01E0_1E70, 0x01E0_1E8C], (0x64, 0x32));
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, (0xC0, 0x40));
    } else {
        fade_out(fx, t, 0x63, (0x10, 0xE));
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01F20: b363's arc of quads (radius 10 growing by 10.5 for 4
/// updates then 10.8), violet, turned randomly; in by 0x40 below 0xC0 for
/// 8 updates, then out by 9.
fn arcq(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0x10);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        l2::rgb(fx, t + 0x60, 0xCF, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 8, 8);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        let r = rand_mod(fx, t, 0x01E0_1F88, 0x168);
        fx.mem.set16(t + 0x5A, r);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, ra) in [(0x20u32, 0x01E0_1FB0u32), (0x24, 0x01E0_1FCC), (0x28, 0x01E0_1FE8)] {
            let r = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        fx.mem.set16(t + 0x5A, 0x2D);
        fx.mem.set16(t + 0x5C, 3);
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fade_in(fx, t, 0x63, (0xC0, 0x40));
    } else {
        fade_out(fx, t, 0x63, (0xB, 9));
    }
    let g = if fx.mem.i16(t + 0x54) < 4 { 10.5 } else { f32::from_bits(0x412C_CCCD) };
    let r = ee::add(fx.mem.f(t + 0x48), g);
    fx.mem.setf(t + 0x48, r);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E020D0 (frame 0x30): a petal ring (+0x50 0 / 1 / 2: radii 14 / 20 /
/// 26 and 6 less, held 0x37 / 0x2D / 0x23 updates), both alphas in by 0xC
/// below 0x80, then out by 9 while growing by 1.5; grey by age parity;
/// facing-turned 0 / 0x8000 for 10 updates, then spinning by +-0x80.
fn petals_step(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2, 0x20);
    if first(fx, t) {
        let k = fx.mem.i16(t + 0x50);
        let r = match k {
            0 => Some(14.0f32),
            1 => Some(20.0),
            2 => Some(26.0),
            _ => None,
        };
        if let Some(r) = r {
            fx.mem.setf(t + 0x48, r);
            fx.mem.setf(t + 0x4C, ee::sub(r, 6.0));
        }
        let r = rand_mod(fx, t, 0x01E0_2184, 7);
        fx.mem.set16(t + 0x56, r);
        fx.mem.set16(t + 0x58, 2);
        let r = rand_mod(fx, t, 0x01E0_21A8, 3);
        fx.mem.set16(t + 8, r + 4);
        for (k, n) in [(0, 0x37), (1, 0x2D), (2, 0x23)] {
            if fx.mem.i16(t + 0x50) == k {
                fx.mem.set16(t + 6, n);
            }
        }
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, (0x80, 0xC));
        fade_in(fx, t, 0x67, (0x80, 0xC));
    } else {
        fade_out(fx, t, 0x63, (0xB, 9));
        let v = fx.mem.read_u8(t + 0x67);
        if v >= 0xB {
            fx.mem.write_u8(t + 0x67, v - 9);
        }
        let r = ee::add(fx.mem.f(t + 0x48), 1.5);
        fx.mem.setf(t + 0x48, r);
        fx.mem.setf(t + 0x4C, ee::sub(r, 6.0));
    }
    let g = if age_parity(fx, t) == 1 { 0x50 } else { 0x60 };
    l2::rgb(fx, t + 0x60, g, g, g);
    l2::rgb(fx, t + 0x64, g, g, g);
    if fx.mem.i16(t + 0x54) < 0xA {
        let v = if fx.mem.i16(FACING) != 0 { 0 } else { 0x8000 };
        fx.mem.write_u32(t + 0x24, v);
    } else {
        let d = if fx.mem.i16(t + 0x50).wrapping_rem(2) == 1 { 0x80 } else { -0x80 };
        let v = fx.mem.i32(t + 0x24).wrapping_add(d);
        fx.mem.set32(t + 0x24, v);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E023D0: a big white quad ring (size rand % 30 + 70 growing by 6 to
/// age 0x8C, cell rand % 8, radius rand % 50 + 100 growing by rand % 20 +
/// 16 to age 0x5A), in by 0x40 below 0xC0 for rand % 14 + 3 updates, then
/// out by 0xE.
fn ring_b(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0x10);
    if first(fx, t) {
        let r = rand_mod(fx, t, 0x01E0_23F4, 0x1E);
        fx.mem.setf(t + 0x48, ee::from_i32(r + 0x46));
        let r = fx.rand_from(t, 0x01E0_2420);
        fx.mem.set16(t + 0x56, r.wrapping_rem(8));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        ring_common(fx, t, [0x01E0_2454, 0x01E0_2474, 0x01E0_2490, 0x01E0_24B0, 0x01E0_24CC], (0x32, 0x64));
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, (0xC0, 0x40));
    } else {
        fade_out(fx, t, 0x63, (0x10, 0xE));
    }
    let a = fx.mem.i16(t + 0x54);
    if a < 0x5A && a != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    if fx.mem.i16(t + 0x54) < 0x8C {
        let v = ee::add(fx.mem.f(t + 0x48), 6.0);
        fx.mem.setf(t + 0x48, v);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E025B0 (frame 0x30): a violet band at P (radius F, turned randomly),
/// its angle +0x50 from 90 by rand % 7 a update to 270; in by 0x80 below
/// 0x80 for 8 updates, then out by 9.
fn band_step(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2, 0x20);
    if first(fx, t) {
        for (o, ra) in [(0x20u32, 0x01E0_25DCu32), (0x24, 0x01E0_25F8), (0x28, 0x01E0_2614)] {
            let r = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        fx.mem.set16(t + 0x56, 0);
        l2::rgb(fx, t + 0x60, 0x40, 0x20, 0x80);
        fx.mem.set16(t + 0x50, 0x5A);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x63, 0);
        let g = fx.mem.read_u32(GROW);
        fx.mem.write_u32(t + 0x48, g);
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fade_in(fx, t, 0x63, (0x80, 0x80));
    } else {
        fade_out(fx, t, 0x63, (0xB, 9));
    }
    age(fx, t);
    if fx.mem.i16(t + 0x50) < 0x10E {
        let r = rand_mod(fx, t, 0x01E0_26D0, 7);
        let v = fx.mem.i16(t + 0x50) + (r as i16 as i32);
        fx.mem.set16(t + 0x50, v);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    fx.leave(0x30);
}

/// The jitter +0x30.. = rand % m - m / 2.
fn jitter(fx: &mut Fx, t: u32, ras: [u32; 3], m: i32) {
    for (o, ra) in [0x30u32, 0x34, 0x38].into_iter().zip(ras) {
        let r = rand_mod(fx, t, ra, m);
        fx.mem.setf(t + o, ee::from_i32(r - m / 2));
    }
}

/// head[c] += the jitter c / 10.
fn head_jitter(fx: &mut Fx, t: u32) {
    for c in [0u32, 4, 8] {
        let j = ee::div(fx.mem.f(t + 0x30 + c), 10.0);
        let h = head(fx.mem.i16(t + 0x50));
        let v = ee::add(fx.mem.f(h + c), j);
        fx.mem.setf(h + c, v);
    }
}

/// +o = (rand % m + add) / 10.
fn tenths(fx: &mut Fx, t: u32, o: u32, ra: u32, m: i32, add: i32) {
    let r = rand_mod(fx, t, ra, m);
    fx.mem.setf(t + o, ee::from_i32(r + add));
    let v = ee::div(fx.mem.f(t + o), 10.0);
    fx.mem.setf(t + o, v);
}

/// 0x1E02720 (frame 0x30): b363's 0x1E04090 trail on 20-point histories:
/// a head D+0x9290 + 0x10 k from the jitter tenths, heading +0x5A / +0x5C
/// by +0x4C, width +0x48; +6 growing to 15 for 7 updates then shrinking;
/// 0xC0 for rand % 15 + 3 updates then out by 0x1D; dark grey by age
/// parity.
fn trail(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2, 0x20);
    let k = |fx: &mut Fx| fx.mem.i16(t + 0x50);
    if first(fx, t) {
        fx.mem.set16(t + 6, 1);
        fx.mem.write_u8(t + 0x63, 0);
        for o in [0u32, 4, 8] {
            let h = head(k(fx));
            fx.mem.write_u32(h + o, 0);
        }
        jitter(fx, t, [0x01E0_2798, 0x01E0_27C4, 0x01E0_27F0], 0x1E);
        head_jitter(fx, t);
        let (tr, h) = (hist(k(fx)), head(k(fx)));
        for j in 0..0x14u32 {
            for o in [0u32, 4, 8] {
                let w = fx.mem.read_u32(h + o);
                fx.mem.write_u32(tr + 0x10 * j + o, w);
            }
        }
        tenths(fx, t, 0x48, 0x01E0_291C, 0x32, 5);
        tenths(fx, t, 0x4C, 0x01E0_295C, 0x55, 3);
        let r = rand_mod(fx, t, 0x01E0_299C, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rand_mod(fx, t, 0x01E0_29B8, 0x168);
        fx.mem.set16(t + 0x5C, r);
        let r = rand_mod(fx, t, 0x01E0_29D4, 0xF);
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
    let g = if age_parity(fx, t) == 0 { 0x40 } else { 0x20 };
    l2::rgb(fx, t + 0x60, g, g, g);
    jitter(fx, t, [0x01E0_2AD0, 0x01E0_2AFC, 0x01E0_2B28], 0x28);
    if fx.mem.i16(t + 0x54) != 0 {
        for (c, a, cos) in [(8u32, 0x5Au32, 0), (0, 0x5A, 1), (4, 0x5C, 1)] {
            let a = fx.mem.i16(t + a);
            let s = l2::sin_deg(fx, SINES, a, cos);
            let m = ee::mul(fx.mem.f(t + 0x4C), s);
            let h = head(k(fx));
            let v = ee::add(fx.mem.f(h + c), m);
            fx.mem.setf(h + c, v);
        }
        head_jitter(fx, t);
    }
    if fx.mem.i16(t + 0x54) < 7 {
        let k = k(fx);
        bspawn::history(fx, hist(k) + 0x130, 0x14, head(k));
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02D30 (frame 0x30): a flat arc at P from 0 growing by F / 20 (inner
/// half of it unless +0xE is 2), blue over grey (+0x50 2: teal over dark
/// grey, turned randomly; 0 turned randomly, else +0x4000 about x); the
/// inner alpha in by 0xC below 0x80 for 0x12 (2: 0x1E) updates, then out
/// by 9 (2: 7).
fn arc(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2, 0x20);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0);
        if fx.mem.i16(t + 0x50) != 2 {
            l2::rgb(fx, t + 0x60, 0x10, 0x10, 0x80);
            l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        } else {
            l2::rgb(fx, t + 0x60, 0x10, 0x80, 0x80);
            l2::rgb(fx, t + 0x64, 0x10, 0x10, 0x10);
        }
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xE, k);
        let k = fx.mem.i16(t + 0x50);
        if k == 0 || k == 2 {
            for (o, ra) in [(0x20u32, 0x01E0_2DE8u32), (0x24, 0x01E0_2E04), (0x28, 0x01E0_2E20)] {
                let r = rand_mod(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, r);
            }
        } else {
            fx.mem.write_u32(t + 0x20, 0x4000);
            fx.mem.write_u32(t + 0x24, 0);
            fx.mem.write_u32(t + 0x28, 0);
        }
        fx.mem.set16(t + 0x5A, 0x14);
        fx.mem.set16(t + 0x5C, 0x14);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 6, 0);
        let n = if fx.mem.i16(t + 0x50) == 2 { 0x1E } else { 0x12 };
        fx.mem.set16(t + 8, n);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 8) {
        fade_in(fx, t, 0x67, (0x80, 0xC));
    } else if fx.mem.i16(t + 0x50) != 2 {
        fade_out(fx, t, 0x67, (0xB, 9));
    } else {
        fade_out(fx, t, 0x67, (9, 7));
    }
    if fx.mem.i16(t + 0xE) != 2 {
        let r = fx.mem.f(t + 0x48);
        fx.mem.setf(t + 0x4C, ee::sub(r, ee::mul(0.5, r)));
    }
    if fx.mem.i16(t + 6) < 0 {
        fx.mem.write_u32(t + 0x4C, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let g = ee::div(fx.mem.f(GROW), 20.0);
        let r = ee::add(fx.mem.f(t + 0x48), g);
        fx.mem.setf(t + 0x48, r);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02FA0 (frame 0x30): a camera-facing flat arc at P of radius F /
/// 1.5 (+0x50 0: blue / violet by age parity over grey, in by 0x10 below
/// 0xC0 until update 0x1D7; 1: grey, in by 0x18 for 0x1E updates), then
/// out by 9; turned +0x4000.
fn arc_b(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2, 0x20);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0x10, 0x10, 0x80);
            l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0x10, 0x10, 0x80);
            l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        }
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let k = fx.mem.i16(t + 0x50);
    if k == 0 {
        if age_parity(fx, t) == 1 {
            l2::rgb(fx, t + 0x60, 0x10, 0x10, 0x80);
        } else {
            l2::rgb(fx, t + 0x60, 0x40, 0x10, 0x80);
        }
    } else if k == 1 {
        let g = if age_parity(fx, t) == 1 { 0x40 } else { 0x60 };
        l2::rgb(fx, t + 0x60, g, g, g);
    }
    if fx.mem.i16(t + 0x50) == 0 {
        if counter(fx) < 0x1D7 {
            fade_in(fx, t, 0x63, (0xC0, 0x10));
        } else {
            fade_out(fx, t, 0x63, (0xB, 9));
        }
    }
    if fx.mem.i16(t + 0x50) == 0 {
        let r = ee::div(fx.mem.f(GROW), 1.5);
        fx.mem.setf(t + 0x48, r);
    }
    if fx.mem.i16(t + 0x50) == 1 {
        if fx.mem.i16(t + 0x54) < 0x1E {
            fade_in(fx, t, 0x63, (0xC0, 0x18));
        } else {
            fade_out(fx, t, 0x63, (0xB, 9));
        }
    }
    if fx.mem.i16(t + 0x50) == 1 {
        let r = ee::div(fx.mem.f(GROW), 1.5);
        fx.mem.setf(t + 0x48, r);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_323C);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03270 (frame 0x30): a violet band at P (radius 5 F, its turn
/// D+0x985A, facing-turned +-0x4000 about x), its angle +0x50 from 90 by
/// rand % 7 + 1 a update to 270; in by 0x80 below 0x80 for 11 updates,
/// then out by 9; ended on update 0x140.
fn band_b(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2, 0x20);
    if first(fx, t) {
        fx.mem.set16(t + 0x56, 0);
        l2::rgb(fx, t + 0x60, 0x40, 0x20, 0x80);
        fx.mem.set16(t + 0x50, 0x5A);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x63, 0);
        let r = ee::mul(5.0, fx.mem.f(GROW));
        fx.mem.setf(t + 0x48, r);
        let a = fx.mem.i16(BAND_A);
        fx.mem.set16(t + 0x5A, a);
    }
    if fx.mem.i16(t + 0x54) < 0xB {
        fade_in(fx, t, 0x63, (0x80, 0x80));
    } else {
        fade_out(fx, t, 0x63, (0xB, 9));
    }
    let v = if fx.mem.i16(FACING) != 0 { -0x4000 } else { 0x4000 };
    fx.mem.set32(t + 0x20, v);
    age(fx, t);
    if fx.mem.i16(t + 0x50) < 0x10E {
        let r = rand_mod(fx, t, 0x01E0_3388, 7);
        let v = fx.mem.i16(t + 0x50) + ((r + 1) as i16 as i32);
        fx.mem.set16(t + 0x50, v);
    } else {
        fx.mem.set16(t + 0x50, 0x10E);
    }
    if counter(fx) == 0x140 {
        fx.mem.set16(t + 0x58, 0);
    }
    fx.leave(0x30);
}

/// 0x1E03400: the target's tint (kind 0 white, 1 pale yellow, 2 pale
/// violet), both alphas in by 0x12 below 0x80 for 0x14 updates, then out
/// by 4.
fn tint(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0x10);
    if first(fx, t) {
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0xF0);
            l2::rgb(fx, t + 0x64, 0xF0, 0xF0, 0xF0);
            fx.mem.set16(t + 6, 0x14);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xD0, 0xD0, 0x80);
            l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0x80);
            fx.mem.set16(t + 6, 0x14);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            l2::rgb(fx, t + 0x60, 0xF0, 0xE0, 0xFF);
            l2::rgb(fx, t + 0x64, 0xF0, 0xE0, 0xFF);
            fx.mem.set16(t + 6, 0x14);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, (0x80, 0x12));
        fade_in(fx, t, 0x67, (0x80, 0x12));
    } else {
        fade_out(fx, t, 0x63, (6, 4));
        let v = fx.mem.read_u8(t + 0x67);
        fx.mem.write_u8(t + 0x67, if v >= 6 { v - 4 } else { 0 });
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03590 (frame 0x30): camera slots 4 / 5 (0x1F7E20) from D+0x2E60
/// (128, 2, 2, 400; P; 0, 123, w rising by 0.3 to 1 while the timer is at
/// least 0x1F, then down by 0.3); cleared when the task ends (w at 0.1, or
/// the timer at 1).
fn camera(fx: &mut Fx, t: u32) {
    let sp = enter_step(fx, 0x30, 2, 0x20);
    if first(fx, t) {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4000_0000), (8, 0x4000_0000), (0xC, 0x43C8_0000), (0x20, 0), (0x24, 0x42F6_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.set16(t + 0x50, 0);
    }
    for c in [0u32, 4, 8] {
        let w = fx.mem.read_u32(P + c);
        fx.mem.write_u32(CAM + 0x10 + c, w);
    }
    let w = fx.mem.f(CAM + 0x28);
    let step = f32::from_bits(0x3E99_999A);
    if fx.mem.i32(TIMER) >= 0x1F {
        if crate::exe::ee_lt(w, 1.0) {
            fx.mem.setf(CAM + 0x28, ee::add(w, step));
        }
    } else if crate::exe::ee_le(w, f32::from_bits(0x3DCC_CCCD)) {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, step));
    }
    let (p, ra) = if fx.mem.i16(t + 0x58) == 0 { (0, 0x01E0_36C8) } else { (CAM, 0x01E0_36DC) };
    crate::blib::exe::camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03720: the stage light, 0x80 down by 10 to 0x50 while the timer is
/// above 0x1E, then up by 10 to 0x80; on timer 3 back to 0x80 and over.
fn light(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0x10);
    if first(fx, t) {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let v = fx.mem.i16(t + 0x50);
    if fx.mem.i32(TIMER) >= 0x1F {
        fx.mem.set16(t + 0x50, if v < 0x51 { 0x50 } else { v - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if v < 0x80 { v + 0xA } else { 0x80 });
    }
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
    fx.leave(0x20);
}

/// 0x1E03800 (frame 0x80): b363's grey window sprite (ALPHA 0x44, key 0),
/// in by 9 below 0x48 for 0x138 updates, then out by 9.
fn window(fx: &mut Fx, t: u32) {
    let sp = enter_step(fx, 0x80, 1, 0x10);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..3u32 {
        fx.mem.write_u8(sp + 0x38 + k, 0x80);
    }
    let a = fx.mem.read_u8(t + 0x63);
    fx.mem.write_u8(sp + 0x3B, a);
    fx.mem.write_u32(sp + 0x3C, 0x3F80_0000);
    for (at, v) in [(sp + 0x48, 0x6FF8u16), (sp + 0x58, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for o in [0x40u32, 0x44] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(ee::add(s, 0.0), 512.0));
    }
    for o in [0x50u32, 0x54] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(ee::sub(ee::add(512.0, s), 0.0), 512.0));
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, 0x80_0000_0044);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(t + 0x54) < 0x138 {
        fade_in(fx, t, 0x63, (0x48, 9));
    } else {
        fade_out(fx, t, 0x63, (0xB, 9));
    }
    age(fx, t);
    fx.leave(0x80);
}

/// 0x1E055B0(D, t, mode, pos) (frame 0x1E0): eighteen petals 20 degrees
/// apart from the angle +0x5A - 200, each a strip3 piece of 20 degrees
/// between the radii +0x48 / +0x4C in the x / z plane on its own atlas cell
/// (0x14, D+0x9814 [k]), coloured +0x60 / +0x64, UVs in D+0xBE's
/// orientation; through TRS(angles +0x20, pos) x the view-projection; keyed
/// by its mean w on D+0x97CC; blend (mode >> 4).
fn petals(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1E0);
    copy4(fx, sp + 0x90, pos);
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
        let w = fx.mem.read_u32(sp + 0x90 + 4 * k);
        fx.mem.write_u32(sp + 0xE0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xF0, 0, t + 0x20, sp + 0xE0);
    for o in [0xA4u32, 0xB4, 0xC4, 0xD4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let r0 = fx.mem.f(t + 0x48);
    let r1 = fx.mem.f(t + 0x4C);
    let mut s1 = 0i32;
    for k in 0..0x12u32 {
        let c = fx.mem.i16(CELLS + 2 * k);
        l2::rect(fx, sp + 0x140, 0x14, c);
        copy4(fx, D + 0x94, sp + 0x140);
        let order = fx.mem.i16(D + 0xBE);
        l2::uv_strip3(fx, d, D + 0x94, order);
        // (extra degrees, the r0 / r1 vertices)
        for (e, inner, outer) in [(0, 0xC0u32, 0xA0u32), (0x14, 0xD0, 0xB0)] {
            let a = (fx.mem.i16(t + 0x5A) + e + s1 - 200) as i16 as i32;
            let cs = l2::sin_deg(fx, SINES, a, 1);
            let sn = l2::sin_deg(fx, SINES, a, 0);
            fx.mem.setf(sp + inner, ee::mul(cs, r0));
            fx.mem.setf(sp + inner + 8, ee::mul(sn, r0));
            fx.mem.setf(sp + outer, ee::mul(cs, r1));
            fx.mem.setf(sp + outer + 8, ee::mul(sn, r1));
        }
        if l2::project(fx, sp + 0xA0, SPR, sp + 0xF0, d, sp + 0x130) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x130)) as i32;
            let ot = fx.mem.read_u32(OT_AT);
            fx.submit(d, ot, key);
        }
        s1 = (s1 + 0x14) as i16 as i32;
    }
    fx.leave(0x1E0);
}
