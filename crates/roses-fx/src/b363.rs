//! BMODULE b363 (card 143), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04F70, data area 0x1ED0000) [F]: a pool of 180 tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step
//! +0x44 through `jalr`; spawn 0x1E05BE0(D, step, +0x40, turned) with the
//! colour D+0xDE94 and the index D+0xDEEE into +0x50 / +0x52, the turn
//! D+0xDE60 into +0x20 when `turned`), no unit flash. Drawn with flat arcs
//! (0x1E00850, `billboard::arc_strips_flat`), raised turned quads
//! (0x1E01D30, `quads4::quad_turned_raised`), quad rings (0x1E00BB0,
//! `billboard::ring_quads` on the bytes D+0xDEF8 / D+0xDFF8, the cell
//! orientation rand % 4 picked in the draw loop), arc quads (0x1E018E0,
//! `billboard::arc_quads`), point rings (0x1E01160, `billboard::ring_points`),
//! the record's own band 0x1E06910 (12 strip3 quads of a 30-degree band),
//! a target tint (0x1E00680, `quads4::tint4`) and trails as the blended tube
//! 0x1E05DC0 (`tubetrail::tube_with(SINGLE_BLEND)`) along the 30-point
//! history D+0xD930 (head D+0xDE30) and D+0x5190 + 0x1E0 k (heads D+0xD4D0
//! + 0x10 k). Globals past D+0xC000: node point D+0xDE40, point D+0xDE50,
//! turn D+0xDE60, textures (5, 0..3) D+0xDE70.., timer D+0xDE90 = 330,
//! colour D+0xDE94, OT D+0xDE9C, tint w D+0xDEA4, nodes D+0xDEC0..
//! (attacker 0x50 (clear) / 0x52 / 0xD / 0x15 / 0x27), target 1 at
//! D+0xDED0, trail counter D+0xDEE6, index D+0xDEEE, facing D+0xDEF2,
//! param D+0xDEF4, counter D+0xDEF6, bytes D+0xDEF8 / D+0xDFF8, sines
//! D+0xE0F8; textures (4, 0xE / 0xF) at D+0x298 / 0x29C. Returns 1 / 3 / 6
//! / 8 on 0xDD / 0x140 / 0xE8..0x13B / timer out.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, spawn as bspawn, tubetrail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xB4;
const ARC: u32 = 0x01E0_2940;
const QUAD: u32 = 0x01E0_2A40;
const RING: u32 = 0x01E0_2DE0;
const ARCQ: u32 = 0x01E0_2F70;
const PTS_A: u32 = 0x01E0_3120;
const PTS_B: u32 = 0x01E0_34E0;
const BAND: u32 = 0x01E0_3740;
const TRAIL_A: u32 = 0x01E0_3A30;
const TRAIL_B: u32 = 0x01E0_4090;
const RING_B: u32 = 0x01E0_46A0;
const ARC_B: u32 = 0x01E0_4880;
const TINT: u32 = 0x01E0_4B60;
const LIGHT: u32 = 0x01E0_4CB0;
const WINDOW: u32 = 0x01E0_4D90;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_5390;
const POS: u32 = D + 0x250;
const HIST_A: u32 = D + 0xD930;
const HEADS_B: u32 = D + 0xD4D0;
const HEAD_A: u32 = D + 0xDE30;
const NODE_PT: u32 = D + 0xDE40;
const POINT: u32 = D + 0xDE50;
const TURN: u32 = D + 0xDE60;
const TEX: u32 = D + 0xDE70;
const TIMER: u32 = D + 0xDE90;
const COLOUR: u32 = D + 0xDE94;
const OT_AT: u32 = D + 0xDE9C;
const TINT_W: u32 = D + 0xDEA4;
const NODES: u32 = D + 0xDEC0;
const TARGET_NODE: u32 = D + 0xDED0;
const TRAIL_K: u32 = D + 0xDEE6;
const INDEX: u32 = D + 0xDEEE;
const FACING: u32 = D + 0xDEF2;
const COUNTER: u32 = D + 0xDEF6;
const BYTES: u32 = D + 0xDEF8;
const BYTES_B: u32 = D + 0xDFF8;
const SINES: u32 = D + 0xE0F8;

fn hist_b(k: i32) -> u32 {
    (D as i32 + 0x5190 + 0x1E0 * k) as u32
}

fn head_b(k: i32) -> u32 {
    (HEADS_B as i32 + 0x10 * k) as u32
}

fn node(fx: &mut Fx, k: u32) -> i32 {
    fx.mem.i16(NODES + 2 * k)
}

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E04F70.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x170);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x170);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xE270);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xDEF4, param);
    fx.mem.set32(TIMER, 0x14A);
    for (k, n) in [0x50, 0x52, 0xD, 0x15, 0x27].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for (k, id) in [(0u32, 0xE), (1, 0xF)] {
        let tex = fx.texture(4, id);
        fx.mem.write_u32(D + 0x298 + 4 * k, tex);
    }
    for k in 0..4u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.fix_residue(v, 0x01E0_5144);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    let fills: [(u32, std::ops::Range<u32>, u32, i32, i32); 5] = [
        (BYTES, 0..0x100, 0x01E0_5174, 0x1E, -0xF),
        (BYTES, 100..0xB4, 0x01E0_51CC, 4, 0),
        (BYTES, 0xB4..0x100, 0x01E0_522C, 0xC, -6),
        (BYTES_B, 0..0x100, 0x01E0_527C, 0xA, -5),
        (BYTES_B, 100..0xB4, 0x01E0_52D4, 0xF, 0),
    ];
    for (at, ks, ra, m, add) in fills {
        for k in ks {
            let r = fx.rand_from(D, ra);
            let v = if m == 4 { r.wrapping_rem(4) } else { fx.div(r, m).1 };
            fx.mem.write_u8(at + k, (v + add) as u8);
        }
    }
    for k in 0..100u32 {
        fx.mem.write_u8(BYTES_B + k, 0);
    }
    0
}

/// The atlas cell `id` (0x106960 into `r`, frame +0x56) at D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, frame);
    copy4(fx, D + 0x94, r);
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
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

fn uv_words(fx: &mut Fx, v: [i32; 4]) {
    for (k, x) in v.into_iter().enumerate() {
        fx.mem.set16(D + 0x94 + 2 * k as u32, x);
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
        if f == 0 {
            continue;
        }
        run_task(fx, f, slot + 0x2A0, [u64::from(D), 0, u64::from(slot), u64::from(k), 0]);
    }
    let mut drawn = 0u64;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        drawn = (drawn as i16).wrapping_add(1) as i64 as u64;
        draw(fx, sp, t, drawn);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0xDD => 1,
        0x140 => 3,
        0xE8..=0x13B => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw case for task `t` (`drawn` its s1).
fn draw(fx: &mut Fx, sp: u32, t: u32, drawn: u64) {
    let f = fx.mem.read_u32(t + 0x44);
    if f == ARC {
        fx.mem.set16(D + 0xB8, 0x18);
        billboard::arc_strips_flat(fx, D, t, 0x20, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == QUAD {
        tex(fx, TEX + 4);
        uv_words(fx, [1, 1, 0xFF, 0x3F]);
        fx.mem.set16(D + 0xBE, 5);
        copy3(fx, sp + 0x60, t + 0x10);
        quads4::quad_turned_raised(fx, billboard::uv_rect_m5, D, t, 0x21, sp + 0x60);
    }
    if f == RING {
        tex(fx, TEX + 0xC);
        cell(fx, t, sp + 0x70, 0x14);
        fx.mem.set16(D + 0xB8, 5);
        let r = fx.rand_from(D, 0x01E0_54F0);
        fx.mem.set16(D + 0xBE, r.wrapping_rem(4));
        fx.mem.set16(D + 0xB0, 0);
        billboard::ring_quads(fx, billboard::uv_rect_m5, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == ARCQ {
        tex(fx, D + 0x29C);
        cell(fx, t, sp + 0x80, 0x9B);
        copy4(fx, sp + 0x60, t + 0x10);
        fx.mem.set16(D + 0xB8, 9);
        billboard::arc_quads(fx, billboard::uv_rect_m5, D, t, 0x20, sp + 0x60);
        fx.mem.set16(D + 0xB8, 0);
    }
    for (g, r, ra) in [(PTS_A, 0x90u32, 0x01E0_55F4u32), (PTS_B, 0xA0, 0x01E0_56AC)] {
        if f == g {
            let h = fx.self_handle();
            let s = node(fx, 3);
            crate::b076::node_position_saving(fx, h, s, sp + 0x60, u64::from(D), Some(drawn), ra);
            cell(fx, t, sp + r, 0x9B);
            tex(fx, D + 0x29C);
            fx.mem.set16(D + 0xB8, 0xF);
            fx.mem.set16(D + 0xBE, 0);
            copy3(fx, D + 0x20, t + 0x10);
            billboard::ring_points(fx, billboard::uv_rect_m5, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    if f == BAND {
        if fx.mem.i16(t + 0x52) == 0 {
            tex(fx, TEX + 4);
            uv_words(fx, [0, 0, 0x7F, 0x3F]);
            fx.mem.set16(D + 0xBE, 0);
        }
        if fx.mem.i16(t + 0x52) == 1 {
            tex(fx, D + 0x29C);
            cell(fx, t, sp + 0xB0, 0x97);
            fx.mem.set16(D + 0xBE, 0);
        }
        copy4(fx, sp + 0x60, t + 0x10);
        band(fx, t, sp + 0x60, 0x20);
    }
    if f == TRAIL_A || f == TRAIL_B {
        tex(fx, if f == TRAIL_A { TEX + 4 } else { TEX });
        uv_words(fx, [0, 0, 0x7F, 0x3F]);
        copy3(fx, D + 0x20, t + 0x10);
        fx.mem.set16(D + 0xBE, 1);
        if f == TRAIL_A {
            tubetrail::tube_with(fx, &tubetrail::SINGLE_BLEND, D, t, HIST_A, 0x20);
        } else {
            let k = fx.mem.i16(t + 0x50);
            tubetrail::tube_with(fx, &tubetrail::SINGLE_BLEND, D, t, hist_b(k), 0x21);
        }
    }
    if f == RING_B {
        tex(fx, TEX + 8);
        cell(fx, t, sp + 0xC0, 0x14);
        fx.mem.set16(D + 0xB8, 5);
        let r = fx.rand_from(D, 0x01E0_5978);
        fx.mem.set16(D + 0xBE, r.wrapping_rem(4));
        fx.mem.set16(D + 0xB0, 0);
        let table = if fx.mem.i16(t + 0x52) == 0 { BYTES } else { BYTES_B };
        billboard::ring_quads(fx, billboard::uv_rect_m5, D, t, 0x10, table);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == ARC_B {
        fx.mem.set16(D + 0xB8, 0x18);
        billboard::arc_strips_flat(fx, D, t, 0x21, POINT);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == TINT {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x60);
        // 0x1F71A0's frame below the entry's (its saved ra, s0 = D).
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_5A40), (0, u64::from(D))]);
        let z = fx.mem.f(sp + 0x68);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 80.0) } else { ee::add(z, 80.0) };
        fx.mem.setf(sp + 0x68, z);
        let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
        let mut x = fx.mem.read_u64(sp + 0xF8);
        if let Some(w) = fx.project(p, None, &mut x) {
            fx.mem.write_u64(sp + 0xF8, x);
            fx.mem.setf(TINT_W, w);
        }
        crate::b590::project_residue(fx, sp, sp + 0x60);
        if fx.mem.i16(t + 0x50) == 0 {
            let w = fx.mem.f(TINT_W);
            quads4::tint4(fx, t, 1, w);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            let w = fx.mem.f(TINT_W);
            quads4::tint4(fx, t, 0x20, w);
        }
    }
}

/// `regs`: the run loop's s0 (D), s1 (0), s2 (the slot), s3 (its index),
/// s4 (the runner's, 0 [H]) at the `jalr`.
fn run_task(fx: &mut Fx, f: u32, t: u32, regs: [u64; 5]) {
    match f {
        ARC => arc(fx, t, regs),
        QUAD => quad(fx, t, regs),
        RING => ring(fx, t, regs),
        ARCQ => arcq(fx, t, regs),
        PTS_A => pts_a(fx, t, regs),
        PTS_B => pts_b(fx, t, regs),
        BAND => band_step(fx, t, regs),
        TRAIL_A => trail_a(fx, t, regs),
        TRAIL_B => trail_b(fx, t, regs),
        RING_B => ring_b(fx, t, regs),
        ARC_B => arc_b(fx, t, regs),
        TINT => tint(fx, t, regs),
        LIGHT => light(fx, t, regs),
        WINDOW => window(fx, t, regs),
        _ => panic!("b363: unknown task step {f:#x}"),
    }
}

/// A step's prologue: `frame` bytes, the caller's s0.. (`n` of `regs`) at
/// +`base` + 0x10 k, ra at `ra_at`.
fn enter_step(fx: &mut Fx, frame: u32, regs: [u64; 5], n: usize, base: u32, ra_at: u32) -> u32 {
    let sp = fx.enter(frame);
    for k in 0..n {
        fx.saved(sp + base + 0x10 * k as u32, &[(0, regs[k])]);
    }
    fx.saved(sp, &[(ra_at, TASK_RA)]);
    sp
}

/// 0x1E05BE0(D, step, a2, turned) with the caller's s0..s5 = `regs`.
fn spawn(fx: &mut Fx, f: u32, turned: bool, regs: [u64; 6], ra: u32) -> i32 {
    let sp = fx.enter(0x70);
    fx.saved(sp, &[(0x60, u64::from(ra)), (0x50, regs[5]), (0x40, regs[4]), (0x30, regs[3]), (0x20, regs[2]), (0x10, regs[1]), (0, regs[0])]);
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
            l2::to_i16x4(fx, t, POS, 0);
            // 0x107390's `sq ra` upper half (0) (b232's).
            fx.mem.write_u64(sp - 8, 0);
            copy3(fx, t + 0x10, POS);
            let i = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x50, i);
            let i = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x52, i);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            if turned {
                copy4(fx, t + 0x20, TURN);
            }
            1
        }
        None => 0,
    };
    fx.leave(0x70);
    ret
}

/// The emitter's spawn: s0 = `s0`, s1 = D, the entry's s2 (param) and the
/// runner's s3..s5 (0 [H]).
fn espawn(fx: &mut Fx, s0: u64, param: i32, f: u32, ra: u32) {
    spawn(fx, f, false, [s0, u64::from(D), param as i64 as u64, 0, 0, 0], ra);
}

/// 0x1F6830(handle, node, out) from the emitter (s0 = `s0`, s1 = D).
fn enode(fx: &mut Fx, s0: u64, h: i32, slot: i32, out: u32, ra: u32) {
    crate::b076::node_position_saving(fx, h, slot, out, s0, Some(u64::from(D)), ra);
}

/// rand % m (HI).
fn rand_mod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The index D+0xDEEE = the trail counter.
fn trail_index(fx: &mut Fx) {
    let k = fx.mem.i16(TRAIL_K);
    fx.mem.set16(INDEX, k);
}

/// The trail counter up by one, wrapping at `wrap`.
fn trail_next(fx: &mut Fx, wrap: i32) {
    let k = fx.mem.i16(TRAIL_K) + 1;
    fx.mem.set16(TRAIL_K, k);
    if fx.mem.i16(TRAIL_K) >= wrap {
        fx.mem.set16(TRAIL_K, 0);
    }
}

/// 0x1E02070(D, 0) (frame 0x30: the entry's s0 = D, s1 = 0, ra).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_5360), (0x10, 0), (0, u64::from(D))]);
    let mut s0 = u64::from(D);
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    let h = fx.self_handle();
    let s = node(fx, 0);
    enode(fx, s0, h, s, NODE_PT, 0x01E0_20C0);
    if counter(fx) == 1 {
        espawn(fx, s0, param, WINDOW, 0x01E0_20F0);
    }
    if counter(fx) == 0x41 {
        espawn(fx, s0, param, LIGHT, 0x01E0_2120);
    }
    let c = counter(fx);
    if (0xD0..0xE7).contains(&c) {
        copy4(fx, POS, POINT);
        espawn(fx, s0, param, RING_B, 0x01E0_2184);
        espawn(fx, s0, param, RING_B, 0x01E0_219C);
        for k in 0..4u64 {
            s0 = k;
            fx.mem.set16(INDEX, 0);
            copy4(fx, POS, POINT);
            let r = rand_mod(fx, k as u32, 0x01E0_21DC, 0x14);
            let x = ee::add(fx.mem.f(POS), ee::from_i32(r - 0xA));
            fx.mem.setf(POS, x);
            let r = rand_mod(fx, k as u32, 0x01E0_2210, 0x1E);
            let y = ee::sub(fx.mem.f(POS + 4), ee::from_i32(r));
            fx.mem.setf(POS + 4, y);
            espawn(fx, s0, param, BAND, 0x01E0_224C);
        }
        s0 = 4;
    }
    if counter(fx) == 0xCB {
        fx.mem.set16(INDEX, 0);
        espawn(fx, s0, param, TINT, 0x01E0_22AC);
        fx.mem.set16(INDEX, 1);
        espawn(fx, s0, param, TINT, 0x01E0_22D4);
    }
    let c = counter(fx);
    if (0xCA..0xCC).contains(&c) {
        for (k, ra) in [(2u32, 0x01E0_2318u32), (3, 0x01E0_2338)] {
            let h = fx.self_handle();
            let s = node(fx, k);
            enode(fx, s0, h, s, POS, ra);
        }
    }
    for (lo, hi, nd, ra_n, f, ras) in [(0x5B, 0x97, 4u32, 0x01E0_237Cu32, PTS_B, [0x01E0_23A8u32, 0x01E0_23C0]), (0x64, 0xC9, 0, 0x01E0_2404, PTS_A, [0x01E0_2430, 0x01E0_2448])] {
        let c = counter(fx);
        if (lo..hi).contains(&c) {
            let h = fx.self_handle();
            let s = node(fx, nd);
            enode(fx, s0, h, s, POS, ra_n);
            let y = ee::sub(fx.mem.f(POS + 4), 0.5);
            fx.mem.setf(POS + 4, y);
            for ra in ras {
                espawn(fx, s0, param, f, ra);
            }
        }
    }
    let c = counter(fx);
    if (0xE6..0x12D).contains(&c) {
        let h = fx.target_handle();
        let s = fx.mem.i16(TARGET_NODE);
        enode(fx, s0, h, s, POS, 0x01E0_248C);
        let y = ee::sub(fx.mem.f(POS + 4), 5.0);
        fx.mem.setf(POS + 4, y);
        fx.mem.set16(INDEX, 1);
        espawn(fx, s0, param, ARCQ, 0x01E0_24C8);
        fx.mem.set16(INDEX, 2);
        let c = counter(fx);
        if (0xEC..0x123).contains(&c) && c.wrapping_rem(2) == 1 {
            espawn(fx, s0, param, ARC_B, 0x01E0_2534);
        }
        for (o, ra) in [(0u32, 0x01E0_253Cu32), (8, 0x01E0_2570), (4, 0x01E0_25A4)] {
            let r = rand_mod(fx, s0 as u32, ra, 0x14);
            let v = ee::add(fx.mem.f(POS + o), ee::from_i32(r - 0xA));
            fx.mem.setf(POS + o, v);
        }
        espawn(fx, s0, param, RING, 0x01E0_25E4);
    }
    let c = counter(fx);
    if (0x118..0x128).contains(&c) {
        for k in 0..2u64 {
            s0 = k;
            espawn(fx, s0, param, QUAD, 0x01E0_2628);
        }
        s0 = 2;
    }
    let c = counter(fx);
    if (0x5C..0x9C).contains(&c) {
        s0 = 0;
        let h = fx.self_handle();
        let s = node(fx, 4);
        enode(fx, s0, h, s, POS, 0x01E0_269C);
        trail_index(fx);
        espawn(fx, s0, param, TRAIL_B, 0x01E0_26CC);
        trail_next(fx, 0x46);
        s0 = 1;
    }
    let c = counter(fx);
    if (2..0xC9).contains(&c) {
        trail_index(fx);
        copy4(fx, POS, NODE_PT);
        trail_next(fx, 0x46);
    }
    if counter(fx) == 0xCE {
        let h = fx.self_handle();
        let s = node(fx, 0);
        enode(fx, s0, h, s, POS, 0x01E0_2860);
        s0 = 0;
        trail_index(fx);
        espawn(fx, s0, param, TRAIL_A, 0x01E0_2898);
        trail_next(fx, 0x50);
        s0 = 1;
        fx.mem.set16(INDEX, 1);
        espawn(fx, s0, param, ARC_B, 0x01E0_2920);
    }
    fx.leave(0x30);
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

/// +o = (rand % m + add) / 10.
fn tenths(fx: &mut Fx, t: u32, o: u32, ra: u32, m: i32, add: i32) {
    let r = rand_mod(fx, t, ra, m);
    fx.mem.setf(t + o, ee::from_i32(r + add));
    let v = ee::div(fx.mem.f(t + o), 10.0);
    fx.mem.setf(t + o, v);
}

/// 0x1E02940: a white-violet flat arc (inner +0x48 from 20 growing by 18 a
/// update, outer 30 inside it), 0x80 for 2 updates, then out by 9.
fn arc(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x20, regs, 1, 0, 0x10);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(20.0, 58.0));
        l2::rgb(fx, t + 0x60, 0xC0, 0x80, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 2 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, (0xB, 9));
    }
    let r = ee::add(fx.mem.f(t + 0x48), 18.0);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::sub(r, 30.0));
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02A40 (frame 0x90): a quad dropping from 1000 by 80 a update at the
/// target (x / z jittered by rand % 20 - 10, y 35), turned randomly; at age
/// 11 it spawns a flat arc at itself raised by the drop / 10 turned with
/// it (D+0xDE60 / D+0xDE64 = -180 x the turns).
fn quad(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    let sp = enter_step(fx, 0x90, regs, 5, 0, 0x50);
    let (mut s0, mut s1) = (regs[0], regs[1]);
    if first(fx, t) {
        fx.mem.set16(t + 8, 0xD);
        fx.mem.set16(t + 0xA, 0xA0);
        let h = fx.target_handle();
        fx.unit_position(h, t + 0x10);
        for (o, ra) in [(0x10u32, 0x01E0_2A98u32), (0x18, 0x01E0_2ACC)] {
            let r = rand_mod(fx, t, ra, 0x14);
            let v = ee::add(fx.mem.f(t + o), ee::from_i32(r - 0xA));
            fx.mem.setf(t + o, v);
        }
        fx.mem.write_u32(t + 0x14, 0x420C_0000);
        fx.mem.write_u32(t + 0x34, 0x447A_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let r = rand_mod(fx, t, 0x01E0_2B3C, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rand_mod(fx, t, 0x01E0_2B58, 0xB4);
        fx.mem.set16(t + 0x5C, r - 0x5A);
    }
    if fx.mem.i16(t + 0x54) < 3 {
        fade_in(fx, t, 0x63, (0x80, 0x2A));
        fade_in(fx, t, 0x67, (0x80, 0x2A));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let d = fx.mem.f(t + 0x34);
        if crate::exe::ee_lt(0.0, d) {
            fx.mem.setf(t + 0x34, ee::sub(d, 80.0));
        } else {
            fade_out(fx, t, 0x63, (0x10, 0xE));
            let v = fx.mem.read_u8(t + 0x63);
            if v >= 0x10 {
                fx.mem.write_u8(t + 0x63, v - 0xE);
            }
        }
    }
    if fx.mem.i16(t + 0x54) == 0xB {
        copy4(fx, POS, t + 0x10);
        s1 = u64::from(sp + 0x64);
        s0 = u64::from(sp + 0x74);
        let d = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(sp + 0x64, d);
        fx.mem.write_u32(sp + 0x60, 0);
        fx.mem.write_u32(sp + 0x68, 0);
        let (a, b) = (fx.mem.i16(t + 0x5C), fx.mem.i16(t + 0x5A));
        fx.mem.set32(sp + 0x70, a);
        fx.mem.set32(sp + 0x74, b);
        fx.mem.set32(sp + 0x78, a);
        camera_angles_saving(fx, sp + 0x80, s0, s1, 0x01E0_2CA4);
        let yaw = fx.mem.i32(sp + 0x84);
        fx.mult(0xB60B_60B7_u32 as i32, yaw);
        let v = fx.mem.i32(sp + 0x74).wrapping_sub(yaw / 180);
        fx.mem.set32(sp + 0x74, v);
        l2::rotate_deg(fx, sp + 0x60, sp + 0x70, 2, SINES);
        l2::rotate_deg(fx, sp + 0x60, sp + 0x70, 0, SINES);
        for c in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(POS + c), fx.mem.f(sp + 0x60 + c));
            fx.mem.setf(POS + c, v);
        }
        let a = fx.mem.i16(t + 0x5A);
        fx.mem.set32(TURN + 4, a.wrapping_mul(-180));
        let b = fx.mem.i16(t + 0x5C);
        fx.mem.set32(TURN, b.wrapping_mul(-180));
        spawn(fx, ARC, true, [s0, s1, u64::from(sp + 0x68), u64::from(t), u64::from(D), 0], 0x01E0_2D94);
    }
    camera_angles_saving(fx, t + 0x20, s0, s1, 0x01E0_2DA0);
    fx.mem.write_u32(t + 0x20, 0);
    age(fx, t);
    fx.leave(0x90);
}

/// 0x1E02DE0: a quad ring at the target (size rand % 6 + 2, cell rand % 8,
/// pink, radius rand % 100 + 100, row rand % 35), in by 0x60 below 0xC0 for
/// rand % 14 + 3 updates, then out by 0xE.
fn ring(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x20, regs, 1, 0, 0x10);
    if first(fx, t) {
        let r = rand_mod(fx, t, 0x01E0_2E04, 6);
        fx.mem.setf(t + 0x48, ee::from_i32(r + 2));
        let r = fx.rand_from(t, 0x01E0_2E30);
        fx.mem.set16(t + 0x56, r.wrapping_rem(8));
        l2::rgb(fx, t + 0x60, 0xFF, 0xCF, 0xCF);
        ring_common(fx, t, [0x01E0_2E64, 0x01E0_2E84, 0x01E0_2EA0, 0x01E0_2EC0, 0x01E0_2EDC]);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, (0xC0, 0x60));
    } else {
        fade_out(fx, t, 0x63, (0x10, 0xE));
    }
    age(fx, t);
    fx.leave(0x20);
}

/// The quad rings' radius rand % 100 + 100, turn rand % 360, growth rand
/// % 20 + 20, row rand % 35 and life rand % 14 + 3.
fn ring_common(fx: &mut Fx, t: u32, ras: [u32; 5]) {
    let r = rand_mod(fx, t, ras[0], 0x64);
    fx.mem.set16(t + 0x50, r + 0x64);
    let r = rand_mod(fx, t, ras[1], 0x168);
    fx.mem.set16(t + 0x5A, r);
    let r = rand_mod(fx, t, ras[2], 0x14);
    fx.mem.set16(t + 0xE, r + 0x14);
    let r = rand_mod(fx, t, ras[3], 0x23);
    fx.mem.set16(t + 8, r);
    let r = rand_mod(fx, t, ras[4], 0xE);
    fx.mem.set16(t + 6, r + 3);
    fx.mem.set16(t + 0x58, 2);
}

/// 0x1E02F70: an arc of quads (radius 10 growing by 7.5 for 4 updates then
/// 7.8), violet over dark red, turned randomly; in by 0x40 below 0xC0 for
/// 8 updates, then out by 9.
fn arcq(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x20, regs, 1, 0, 0x10);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        l2::rgb(fx, t + 0x60, 0xCF, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 8, 8);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        let r = rand_mod(fx, t, 0x01E0_2FD8, 0x168);
        fx.mem.set16(t + 0x5A, r);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let r = fx.rand_from(t, 0x01E0_3000);
        fx.mem.set32(t + 0x20, r.wrapping_rem(0x4000) - 0x2000);
        let r = rand_mod(fx, t, 0x01E0_3020, 0xFFFF);
        fx.mem.set32(t + 0x24, r);
        let r = fx.rand_from(t, 0x01E0_303C);
        fx.mem.set32(t + 0x28, r.wrapping_rem(0x4000) - 0x2000);
        fx.mem.set16(t + 0x5A, 0x2D);
        fx.mem.set16(t + 0x5C, 1);
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fade_in(fx, t, 0x63, (0xC0, 0x40));
    } else {
        fade_out(fx, t, 0x63, (0xB, 9));
    }
    let g = if fx.mem.i16(t + 0x54) < 4 { 7.5 } else { f32::from_bits(0x40F9_999A) };
    let r = ee::add(fx.mem.f(t + 0x48), g);
    fx.mem.setf(t + 0x48, r);
    age(fx, t);
    fx.leave(0x20);
}

/// The point rings' shared start: size 100, cell 0.
fn pts_init(fx: &mut Fx, t: u32) {
    fx.mem.write_u32(t + 0x48, 0x42C8_0000);
    fx.mem.set16(t + 0x56, 0);
}

/// 0x1E03120 (frame 0xA0): a point ring riding the way from node 0x50 to
/// node 0x52 (+0x52 / 140 of it, +0x52 growing by rand % (counter - 100,
/// 1..15) + 1 a update), violet, size 100 growing by 2, spinning +-5; in by
/// 0x2A below 0x80 for rand % 6 + 2 updates, then out by 0xD; from 0xA1 it
/// sinks 1.3.
fn pts_a(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    let sp = enter_step(fx, 0xA0, regs, 5, 0x10, 0x60);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    let h = fx.self_handle();
    let s = node(fx, 0);
    crate::b076::node_position_saving(fx, h, s, sp + 0x70, regs[0], Some(regs[1]), 0x01E0_3164);
    let h = fx.self_handle();
    let s = node(fx, 1);
    let s0 = sp + 0x80;
    crate::b076::node_position_saving(fx, h, s, sp + 0x80, u64::from(s0), Some(regs[1]), 0x01E0_3188);
    for c in [0u32, 4, 8] {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x80 + c), fx.mem.f(sp + 0x70 + c)), 140.0);
        fx.mem.setf(sp + 0x80 + c, v);
    }
    let s1 = u64::from(sp + 0x84);
    if first(fx, t) {
        pts_init(fx, t);
        l2::rgb(fx, t + 0x60, 0x40, 0x20, 0x80);
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xA, v);
        let r = rand_mod(fx, s0, 0x01E0_3214, 0xF);
        fx.mem.set16(t + 0x50, r + 1);
        let r = rand_mod(fx, s0, 0x01E0_3234, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rand_mod(fx, s0, 0x01E0_3250, 3);
        fx.mem.set16(t + 0xE, r + 2);
        let r = rand_mod(fx, s0, 0x01E0_326C, 0x23);
        fx.mem.set16(t + 8, r);
        let r = rand_mod(fx, s0, 0x01E0_328C, 6);
        fx.mem.set16(t + 6, r + 2);
        let r = fx.rand_from(s0, 0x01E0_32AC);
        fx.mem.set16(t + 0xC, r.wrapping_rem(2));
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, (0x80, 0x2A));
    } else {
        fade_out(fx, t, 0x63, (0xF, 0xD));
    }
    let f20 = ee::from_i32(fx.mem.i16(t + 0x52));
    let h = fx.self_handle();
    let s = node(fx, 0);
    crate::b076::node_position_saving(fx, h, s, sp + 0x90, u64::from(s0), Some(s1), 0x01E0_3348);
    for c in [0u32, 4, 8] {
        let v = ee::add(fx.mem.f(sp + 0x90 + c), ee::mul(fx.mem.f(sp + 0x80 + c), f20));
        fx.mem.setf(t + 0x10 + c, v);
    }
    if counter(fx) >= 0xA1 {
        let y = ee::sub(fx.mem.f(t + 0x14), f32::from_bits(0x3FA6_6666));
        fx.mem.setf(t + 0x14, y);
    }
    spin(fx, t);
    let v = ee::add(fx.mem.f(t + 0x48), 2.0);
    fx.mem.setf(t + 0x48, v);
    grow(fx, t);
    let m = (counter(fx) - 0x64) as i16 as i32;
    let m = if m <= 0 { 1 } else if m >= 0x10 { 0xF } else { m };
    let r = rand_mod(fx, m as u32, 0x01E0_3474, m);
    let v = fx.mem.i16(t + 0x52) + ((r + 1) as i16 as i32);
    fx.mem.set16(t + 0x52, v);
    age(fx, t);
    fx.leave(0xA0);
}

/// From age 1 +0x5A turns +5 (+0xC 0) or -5.
fn spin(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) != 0 {
        let d = if fx.mem.i16(t + 0xC) == 0 { 5 } else { -5 };
        let v = fx.mem.i16(t + 0x5A) + d;
        fx.mem.set16(t + 0x5A, v);
    }
}

/// From age 1 the radius +0x50 grows by +0xE.
fn grow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
}

/// 0x1E034E0: a point ring at node 0x27 (dark blue before 0x6B, then dark
/// red), size 100 growing by 1, rising rand % 10 / 10, spinning +-5, radius
/// rand % 15 + 1 growing by rand % 8 + 2; in by 0x2A below 0x80 for rand %
/// 2 + 2 updates, then out by 0xD.
fn pts_b(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x20, regs, 1, 0, 0x10);
    if first(fx, t) {
        pts_init(fx, t);
        if counter(fx) < 0x6B {
            l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x80);
        } else {
            l2::rgb(fx, t + 0x60, 0x80, 0x20, 0x20);
        }
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xA, v);
        let r = rand_mod(fx, t, 0x01E0_355C, 0xF);
        fx.mem.set16(t + 0x50, r + 1);
        let r = rand_mod(fx, t, 0x01E0_357C, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = fx.rand_from(t, 0x01E0_3598);
        fx.mem.set16(t + 0xE, r.wrapping_rem(8) + 2);
        let r = rand_mod(fx, t, 0x01E0_35B8, 0x23);
        fx.mem.set16(t + 8, r);
        let r = fx.rand_from(t, 0x01E0_35D4);
        fx.mem.set16(t + 6, r.wrapping_rem(2) + 2);
        let r = fx.rand_from(t, 0x01E0_35F4);
        fx.mem.set16(t + 0xC, r.wrapping_rem(2));
        fx.mem.set16(t + 0x58, 2);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, (0x80, 0x2A));
    } else {
        fade_out(fx, t, 0x63, (0xF, 0xD));
    }
    let r = rand_mod(fx, t, 0x01E0_366C, 0xA);
    let up = ee::div(ee::from_i32(r), 10.0);
    if fx.mem.i16(t + 0x54) != 0 {
        let y = ee::add(fx.mem.f(t + 0x14), up);
        fx.mem.setf(t + 0x14, y);
    }
    spin(fx, t);
    let v = ee::add(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, v);
    grow(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03740: a grey band (+0x52 0: radius 230 growing by 15 below 400,
/// turned randomly about y, x drifting outwards by rand % 5 + 1; 1: radius
/// 80 growing by 25, turned randomly), its angle +0x50 from 90 by rand % 7
/// a update to 270; in by 0x60 below 0xC0 for 7 updates, then +0xF while
/// at least 0x11 (as the original).
fn band_step(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x20, regs, 1, 0, 0x10);
    if first(fx, t) {
        if fx.mem.i16(t + 0x52) == 0 {
            let r = rand_mod(fx, t, 0x01E0_3770, 0xFFFF);
            fx.mem.set32(t + 0x24, r);
            fx.mem.write_u32(t + 0x48, 0x4366_0000);
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
            fx.mem.set16(t + 0x50, 0x5A);
        }
        if fx.mem.i16(t + 0x52) == 1 {
            for (o, ra) in [(0x20u32, 0x01E0_37BCu32), (0x24, 0x01E0_37D8), (0x28, 0x01E0_37F4)] {
                let r = rand_mod(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, r);
            }
            fx.mem.write_u32(t + 0x48, 0x42A0_0000);
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
            fx.mem.set16(t + 0x50, 0x5A);
        }
        fx.mem.set16(t + 0x56, 0);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x63, 0);
    }
    if fx.mem.i16(t + 0x54) < 7 {
        fade_in(fx, t, 0x63, (0xC0, 0x60));
    } else {
        let v = fx.mem.read_u8(t + 0x63);
        if v >= 0x11 {
            fx.mem.write_u8(t + 0x63, v.wrapping_add(0xF));
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let swing = |fx: &mut Fx, ra: u32| {
        if fx.mem.i16(t + 0x50) < 0x10E {
            let r = rand_mod(fx, t, ra, 7);
            let v = fx.mem.i16(t + 0x50) + (r as i16 as i32);
            fx.mem.set16(t + 0x50, v);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    };
    if fx.mem.i16(t + 0x52) == 0 {
        swing(fx, 0x01E0_38A8);
        let r = fx.mem.f(t + 0x48);
        if crate::exe::ee_lt(r, 400.0) {
            fx.mem.setf(t + 0x48, ee::add(r, 15.0));
        }
        let x = fx.mem.f(t + 0x10);
        if crate::exe::ee_lt(0.0, x) {
            let r = rand_mod(fx, t, 0x01E0_3930, 5);
            let v = ee::add(fx.mem.f(t + 0x10), ee::from_i32(r + 1));
            fx.mem.setf(t + 0x10, v);
        } else {
            let r = rand_mod(fx, t, 0x01E0_396C, 5);
            let v = ee::sub(fx.mem.f(t + 0x10), ee::from_i32(r + 1));
            fx.mem.setf(t + 0x10, v);
        }
    }
    if fx.mem.i16(t + 0x52) == 1 {
        swing(fx, 0x01E0_39C4);
        let r = ee::add(fx.mem.f(t + 0x48), 25.0);
        fx.mem.setf(t + 0x48, r);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// The trails' jitter: +0x30.. = rand % m - m / 2.
fn jitter(fx: &mut Fx, t: u32, ras: [u32; 3], m: i32) {
    for (o, ra) in [0x30u32, 0x34, 0x38].into_iter().zip(ras) {
        let r = rand_mod(fx, t, ra, m);
        fx.mem.setf(t + o, ee::from_i32(r - m / 2));
    }
}

/// head[c] += the jitter c / 10 for each `cs`.
fn head_jitter(fx: &mut Fx, t: u32, h: u32, cs: &[u32]) {
    for &c in cs {
        let j = ee::div(fx.mem.f(t + 0x30 + c), 10.0);
        let v = ee::add(fx.mem.f(h + c), j);
        fx.mem.setf(h + c, v);
    }
}

/// 0x1E03A30 (frame 0x40: the caller's s0..s2, ra): the trail at node
/// 0x50: a head D+0xDE30 jittered by rand % 30 - 15 tenths, pushed 5 a
/// update along z (away from the facing) for 0x18 updates; its point D+0x250
/// / D+0xDE50 = head + the task's point; ages 0x18..0x21 ten trails
/// 0x1E04090 from it, age 0x17 a red arc 0x1E04880 at y 42; +6 growing to
/// 0x1C for 30 updates then shrinking every second; 0xC0 for 0x22 updates
/// then out by 7; grey by age parity.
fn trail_a(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x40, regs, 3, 0, 0x30);
    let regs6 = |s0: u64| [s0, u64::from(t), u64::from(D), regs[3], regs[4], 0];
    if first(fx, t) {
        fx.mem.set16(t + 6, 1);
        fx.mem.write_u8(t + 0x63, 0);
        for o in [0u32, 4, 8] {
            fx.mem.write_u32(HEAD_A + o, 0);
        }
        jitter(fx, t, [0x01E0_3A88, 0x01E0_3AB4, 0x01E0_3AE0], 0x1E);
        head_jitter(fx, t, HEAD_A, &[0, 4, 8]);
        for j in 0..0x1Eu32 {
            copy3(fx, HIST_A + 0x10 * j, HEAD_A);
        }
        tenths(fx, t, 0x48, 0x01E0_3BE4, 0x32, 0x258);
        tenths(fx, t, 0x4C, 0x01E0_3C24, 0x55, 3);
        let r = rand_mod(fx, t, 0x01E0_3C64, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rand_mod(fx, t, 0x01E0_3C80, 0x168);
        fx.mem.set16(t + 0x5C, r);
        fx.mem.set16(t + 8, 0x22);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.i16(t + 0x54);
    let n = fx.mem.i16(t + 6);
    if a < 0x1E {
        if n < 0x1C {
            fx.mem.set16(t + 6, n + 1);
        }
    } else if n > 0 && a.wrapping_rem(2) != 0 {
        fx.mem.set16(t + 6, n - 1);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 8) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade_out(fx, t, 0x63, (9, 7));
        let n = fx.mem.i16(t + 6);
        if n != 0 {
            fx.mem.set16(t + 6, n - 1);
        }
    }
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 0 { 0x80 } else { 0x40 };
    l2::rgb(fx, t + 0x60, g, g, g);
    jitter(fx, t, [0x01E0_3DB4, 0x01E0_3DE0, 0x01E0_3E0C], 0x32);
    copy4(fx, POS, HEAD_A);
    for c in [0u32, 4, 8] {
        let v = ee::add(fx.mem.f(POS + c), fx.mem.f(t + 0x10 + c));
        fx.mem.setf(POS + c, v);
    }
    copy4(fx, POINT, POS);
    let a = fx.mem.i16(t + 0x54);
    if a != 0 {
        if a < 0x18 {
            let z = fx.mem.f(HEAD_A + 8);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
            fx.mem.setf(HEAD_A + 8, z);
        }
        let a = fx.mem.i16(t + 0x54);
        if (0x18..0x22).contains(&a) {
            for k in 0..0xAu64 {
                trail_index(fx);
                spawn(fx, TRAIL_B, false, regs6(k), 0x01E0_3F8C);
                trail_next(fx, 0x46);
            }
        }
        if fx.mem.i16(t + 0x54) == 0x17 {
            fx.mem.write_u32(POS + 4, 0x4228_0000);
            fx.mem.set16(INDEX, 0);
            spawn(fx, ARC_B, false, regs6(regs[0]), 0x01E0_4034);
        }
    }
    if fx.mem.i16(t + 0x54) < 0x18 {
        bspawn::history(fx, HIST_A + 0x1D0, 0x1E, HEAD_A);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E04090: trail +0x50 (k): a head D+0xD4D0 + 0x10 k from the jitter
/// tenths heading +0x5A / +0x5C (rand % 360 each) by +0x4C (rand % 85 + 3
/// tenths), width +0x48 (rand % 50 + 5 tenths), jitter rand % 40 - 20
/// tenths; +6 growing to 15 for 7 updates then shrinking; 0xC0 for rand %
/// 15 + 3 updates then out by 0x1D; dark grey by age parity.
fn trail_b(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x30, regs, 2, 0, 0x20);
    let k = |fx: &mut Fx| fx.mem.i16(t + 0x50);
    if first(fx, t) {
        fx.mem.set16(t + 6, 1);
        fx.mem.write_u8(t + 0x63, 0);
        for o in [0u32, 4, 8] {
            let h = head_b(k(fx));
            fx.mem.write_u32(h + o, 0);
        }
        jitter(fx, t, [0x01E0_4108, 0x01E0_4134, 0x01E0_4160], 0x1E);
        for c in [0u32, 4, 8] {
            let h = head_b(k(fx));
            head_jitter(fx, t, h, &[c]);
        }
        let (tr, h) = (hist_b(k(fx)), head_b(k(fx)));
        for j in 0..0x1Eu32 {
            copy3(fx, tr + 0x10 * j, h);
        }
        tenths(fx, t, 0x48, 0x01E0_428C, 0x32, 5);
        tenths(fx, t, 0x4C, 0x01E0_42CC, 0x55, 3);
        let r = rand_mod(fx, t, 0x01E0_430C, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rand_mod(fx, t, 0x01E0_4328, 0x168);
        fx.mem.set16(t + 0x5C, r);
        let r = rand_mod(fx, t, 0x01E0_4344, 0xF);
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
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 0 { 0x40 } else { 0x20 };
    l2::rgb(fx, t + 0x60, g, g, g);
    jitter(fx, t, [0x01E0_4440, 0x01E0_446C, 0x01E0_4498], 0x28);
    if fx.mem.i16(t + 0x54) != 0 {
        for (c, a, cos) in [(8u32, 0x5Au32, 0), (0, 0x5A, 1), (4, 0x5C, 1)] {
            let a = fx.mem.i16(t + a);
            let s = l2::sin_deg(fx, SINES, a, cos);
            let m = ee::mul(fx.mem.f(t + 0x4C), s);
            let h = head_b(k(fx));
            let v = ee::add(fx.mem.f(h + c), m);
            fx.mem.setf(h + c, v);
        }
        for c in [0u32, 4, 8] {
            let h = head_b(k(fx));
            head_jitter(fx, t, h, &[c]);
        }
    }
    if fx.mem.i16(t + 0x54) < 7 {
        let k = k(fx);
        bspawn::history(fx, hist_b(k) + 0x1D0, 0x1E, head_b(k));
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E046A0: a big quad ring at the point (size rand % 30 + 70 growing by
/// 3 to age 0x8C, pink, radius rand % 100 + 100 growing by rand % 20 + 20
/// to age 0x5A), in by 0x60 below 0xC0 for rand % 14 + 3 updates, then out
/// by 0xE.
fn ring_b(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x20, regs, 1, 0, 0x10);
    if first(fx, t) {
        let r = rand_mod(fx, t, 0x01E0_46C4, 0x1E);
        fx.mem.setf(t + 0x48, ee::from_i32(r + 0x46));
        let r = fx.rand_from(t, 0x01E0_46F0);
        fx.mem.set16(t + 0x56, r.wrapping_rem(8));
        l2::rgb(fx, t + 0x60, 0xFF, 0xCF, 0xCF);
        ring_common(fx, t, [0x01E0_4724, 0x01E0_4744, 0x01E0_4760, 0x01E0_4780, 0x01E0_479C]);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, (0xC0, 0x60));
    } else {
        fade_out(fx, t, 0x63, (0x10, 0xE));
    }
    let a = fx.mem.i16(t + 0x54);
    if a < 0x5A && a != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    if fx.mem.i16(t + 0x54) < 0x8C {
        let v = ee::add(fx.mem.f(t + 0x48), 3.0);
        fx.mem.setf(t + 0x48, v);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E04880 (frame 0x30): a red arc drawn at D+0xDE50, kind +0x50: 0
/// radius 550 for 15 updates, 1 radius 80 riding the point for 25, 2
/// radius 80 growing by 8 (inner by (r - 2) / 3) turned randomly for 10;
/// white over red / dark red by age parity; in by 0x40 below 0xC0, then
/// out by 0xB; kinds 0 / 1 camera-facing, turned +0x4000.
fn arc_b(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x30, regs, 2, 0, 0x20);
    if first(fx, t) {
        let k = fx.mem.i16(t + 0x50);
        if k == 0 {
            fx.mem.write_u32(t + 0x48, 0x4409_8000);
            l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
            fx.mem.set16(t + 6, 0xF);
        } else if k == 1 {
            fx.mem.write_u32(t + 0x48, 0x42A0_0000);
            l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
            fx.mem.set16(t + 6, 0x19);
        } else if k == 2 {
            fx.mem.write_u32(t + 0x48, 0x42A0_0000);
            l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
            for (o, ra) in [(0x20u32, 0x01E0_4978u32), (0x24, 0x01E0_4994), (0x28, 0x01E0_49B0)] {
                let r = rand_mod(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, r);
            }
            fx.mem.set16(t + 6, 0xA);
        }
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, (0xC0, 0x40));
    } else {
        fade_out(fx, t, 0x63, (0xD, 0xB));
    }
    if fx.mem.i16(t + 0x50) == 1 {
        copy4(fx, t + 0x10, POINT);
    }
    if fx.mem.i16(t + 0x50) == 2 {
        let r = ee::add(fx.mem.f(t + 0x48), 8.0);
        fx.mem.setf(t + 0x48, r);
        let i = ee::add(fx.mem.f(t + 0x4C), ee::div(ee::sub(r, 2.0), 3.0));
        fx.mem.setf(t + 0x4C, i);
    }
    if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
    } else {
        l2::rgb(fx, t + 0x64, 0xC0, 0x70, 0x70);
    }
    if fx.mem.i16(t + 0x50) != 2 {
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_4B28);
        let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
        fx.mem.set32(t + 0x20, a);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E04B60: the target's tint (kind 0 white, 1 warm), both alphas in by
/// 0x12 below 0x80 for 0x16 updates, then out by 4.
fn tint(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x20, regs, 1, 0, 0x10);
    if first(fx, t) {
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0xF0);
            l2::rgb(fx, t + 0x64, 0xF0, 0xF0, 0xF0);
            fx.mem.set16(t + 6, 0x16);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xF0, 0xD0, 0xA0);
            l2::rgb(fx, t + 0x64, 0xF0, 0xA0, 0xA0);
            fx.mem.set16(t + 6, 0x16);
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

/// 0x1E04CB0: the stage light, 0x80 down by 10 to 0x50 while the timer is
/// above 0x1E, then up by 10 to 0x80; on timer 2 back to 0x80 and over.
fn light(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    enter_step(fx, 0x20, regs, 1, 0, 0x10);
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
    if fx.mem.i32(TIMER) == 2 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
    fx.leave(0x20);
}

/// 0x1E04D90 (frame 0x80): a grey window sprite (ALPHA 0x44, key 0), in by
/// 0xA below 0x50 for 0xF4 updates, then out by 9.
fn window(fx: &mut Fx, t: u32, regs: [u64; 5]) {
    let sp = enter_step(fx, 0x80, regs, 1, 0, 0x10);
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
    if fx.mem.i16(t + 0x54) < 0xF4 {
        fade_in(fx, t, 0x63, (0x50, 0xA));
    } else {
        fade_out(fx, t, 0x63, (0xB, 9));
    }
    age(fx, t);
    fx.leave(0x80);
}

/// 0x1E06910(D, t, pos, mode) (frame 0x200): twelve strip3 quads of a band
/// 30 degrees each from +0x5A about y, its lower edge at radius 90 cos /
/// height 90 sin of +0x50, its upper edge at +0x50 + 20, scaled +0x48 /
/// 1500, turned +0x20 at `pos`; colour +0x60, keyed by mean w.
pub(crate) fn band(fx: &mut Fx, t: u32, pos: u32, mode: i32) {
    let sp = fx.enter(0x200);
    copy4(fx, sp + 0xB0, pos);
    let d = sp + 0x170;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 0);
    fx.desc_alpha(d, crate::c061::blend_alpha((mode >> 4) as i16 as i32));
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    copy3(fx, sp + 0x100, sp + 0xB0);
    let s = ee::div(fx.mem.f(t + 0x48), 1500.0);
    l2::set3(fx, sp + 0x110, s, s, s);
    l2::xform_vp(fx, sp + 0x120, sp + 0x110, t + 0x20, sp + 0x100);
    let a = fx.mem.i16(t + 0x50);
    let y0 = ee::mul(90.0, l2::sin_deg(fx, D + 0xD8, a, 0));
    fx.mem.setf(sp + 0xC4, y0);
    fx.mem.setf(sp + 0xD4, y0);
    let y1 = ee::mul(90.0, l2::sin_deg(fx, D + 0xD8, (a + 0x14) as i16 as i32, 0));
    fx.mem.setf(sp + 0xE4, y1);
    fx.mem.setf(sp + 0xF4, y1);
    let f20 = ee::mul(90.0, l2::sin_deg(fx, D + 0xD8, a, 1));
    let f21 = ee::mul(90.0, l2::sin_deg(fx, D + 0xD8, (a + 0x14) as i16 as i32, 1));
    let mut s1 = 0i32;
    for k in 0..0xC {
        if k == 0 {
            let a = (fx.mem.i16(t + 0x5A) + s1) as i16 as i32;
            let v = ee::mul(f21, l2::sin_deg(fx, D + 0xD8, a, 1));
            fx.mem.setf(sp + 0xE0, v);
            let v = ee::mul(f21, l2::sin_deg(fx, D + 0xD8, a, 0));
            fx.mem.setf(sp + 0xE8, v);
            let v = ee::mul(f20, l2::sin_deg(fx, D + 0xD8, a, 1));
            fx.mem.setf(sp + 0xC0, v);
            let v = ee::mul(f20, l2::sin_deg(fx, D + 0xD8, a, 0));
            fx.mem.setf(sp + 0xC8, v);
        } else {
            for (to, from) in [(0xE0u32, 0xF0u32), (0xE8, 0xF8), (0xC0, 0xD0), (0xC8, 0xD8)] {
                let w = fx.mem.read_u32(sp + from);
                fx.mem.write_u32(sp + to, w);
            }
        }
        let a = (fx.mem.i16(t + 0x5A) + 0x1E + s1) as i16 as i32;
        let v = ee::mul(f21, l2::sin_deg(fx, D + 0xD8, a, 1));
        fx.mem.setf(sp + 0xF0, v);
        let v = ee::mul(f21, l2::sin_deg(fx, D + 0xD8, a, 0));
        fx.mem.setf(sp + 0xF8, v);
        let v = ee::mul(f20, l2::sin_deg(fx, D + 0xD8, a, 1));
        fx.mem.setf(sp + 0xD0, v);
        let v = ee::mul(f20, l2::sin_deg(fx, D + 0xD8, a, 0));
        fx.mem.setf(sp + 0xD8, v);
        if l2::project(fx, sp + 0xC0, 0x7000_0000, sp + 0x120, d, sp + 0x160) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
        s1 = (s1 + 0x1E) as i16 as i32;
    }
    fx.leave(0x200);
}
