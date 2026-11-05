//! BMODULE b252 (card 70), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03F60, data area 0x1ED0000) [F]: b264's (card 568) sibling. A pool of 250 tasks of 0x70 bytes at
//! D+0x2A0 (alive +0x58, step +0x44 through `jalr`; spawn 0x1E04880 with
//! the colour D+0xDBD4, the index D+0xDC36 into +0x50 / +0x52 and D+0xDC2A
//! into +0x5A) drawn with b264's library: flat arcs (0x1E009B0), rings of
//! quads (0x1E00D10 on the bytes D+0xDD40), the full-window tint
//! (0x1E01E10, `quads4::tint4`), single quads (0x1E01FE0) and trails
//! drawn as the blended tube 0x1E012C0 (`tubetrail::tube_with(SINGLE_BLEND)`)
//! along 20-point histories D+0x7030 + 0x140 k of relative heads D+0xD430
//! + 0x10 k; b264's unit flash (0x1E04A30, the attacker drawn into the
//! render target, a red frame sprite 0x1E03CA0 from update 0x14). Globals
//! past D+0xD000: ten points D+0xD930 + 0x10 j at the attacker's node 0x30
//! until update 0x62, then flying toward the target (velocities D+0xD9D0..
//! over 50 updates to 0x97, jitter D+0xDA70.. from 0x98 to 0xBC), the
//! deltas D+0xDB10.. between points, textures D+0xDBB0.., timer D+0xDBD0 =
//! 270, OT D+0xDBDC, layer / flash descriptor D+0xDBE4 / D+0xDBE8, nodes
//! D+0xDC08.., facing D+0xDC3A, counter D+0xDC3E, sines D+0xDE40. The
//! emitter 0x1E022A0: on 0x19..0xFF eight quad rings 0x1E02C90 a update
//! (points 0..3 on odd updates, 4..7 on even), on 0x9A..0xFF eight more
//! (0x1E02EC0), on 0x19..0x8C one at point 0 (0x1E03090); 0x98 a red arc
//! (0x1E03210) and a red tint (0x1E03310) at point 0; 0x98..0xFF one trail
//! (0x1E03530) a update; 0x18 eight glows (0x1E03A40); 0x14 the flash. The
//! step 0x1E03410 is never spawned [F]: not ported, nor its draw case.
//! Returns 1 / 3 / 6 / 8 on 0x8E / 0x104 / 0x99..0xFE / timer out.
use crate::b494::camera_angles_saving;
use crate::b590::project_residue;
use crate::blib::{billboard, quads4, spawn as bspawn, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xFA;
const RING_A: u32 = 0x01E0_2C90;
const RING_B: u32 = 0x01E0_2EC0;
const RING_C: u32 = 0x01E0_3090;
const ARC: u32 = 0x01E0_3210;
const TINT: u32 = 0x01E0_3310;
const QUAD: u32 = 0x01E0_3410;
const TRAIL: u32 = 0x01E0_3530;
const GLOW: u32 = 0x01E0_3A40;
const FLASH: u32 = 0x01E0_3CA0;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_42D0;
const HEADS: u32 = D + 0xD430;
const POINTS: u32 = D + 0xD930;
const VELOCITY: u32 = D + 0xD9D0;
const JITTER: u32 = D + 0xDA70;
const DELTAS: u32 = D + 0xDB10;
const TEX: u32 = D + 0xDBB0;
const TIMER: u32 = D + 0xDBD0;
const COLOUR: u32 = D + 0xDBD4;
const OT_AT: u32 = D + 0xDBDC;
const LAYER: u32 = D + 0xDBE4;
const FLASH_DESC: u32 = D + 0xDBE8;
const TINT_W: u32 = D + 0xDBF4;
const NODES: u32 = D + 0xDC08;
const ANGLE: u32 = D + 0xDC2A;
const TRAIL_K: u32 = D + 0xDC2E;
const INDEX: u32 = D + 0xDC36;
const FACING: u32 = D + 0xDC3A;
const COUNTER: u32 = D + 0xDC3E;
const BYTES: u32 = D + 0xDC40;
const BYTES_B: u32 = D + 0xDD40;
const SINES: u32 = D + 0xDE40;

fn trail_of(k: i32) -> u32 {
    (D as i32 + 0x7030 + 0x140 * k) as u32
}

fn point(i: i32) -> u32 {
    (POINTS as i32 + 0x10 * i) as u32
}

/// The entry 0x1E03F60.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xDFB0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xDC3C, param);
    fx.mem.set32(TIMER, 0x10E);
    for (k, n) in [0x30, 0x34, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xDC18, r);
    let tex = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, tex);
    for k in 0..4u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(v, 0x01E0_415C);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_418C);
        let (_, h) = fx.div(r, 0x1E);
        fx.mem.write_u8(BYTES + k, (h - 0xF) as u8);
    }
    for k in 100..0xB4u32 {
        let r = fx.rand_from(D, 0x01E0_41E4);
        fx.mem.write_u8(BYTES + k, r.wrapping_rem(4) as u8);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_423C);
        let (_, h) = fx.div(r, 0x28);
        fx.mem.write_u8(BYTES_B + k, (h - 0x14) as u8);
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

/// The low word of the last XYZ2 of the last primitive submitted since
/// `before` (a3 after a draw at depth mode 1), else `old`.
fn last_xyz2(fx: &mut Fx, before: usize, old: u32) -> u32 {
    match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => old,
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let mut a3 = emit(fx, param);
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 {
            continue;
        }
        if f == FLASH {
            flashes = (flashes as i32 + 1) as i16 as u32;
            continue;
        }
        run_task(fx, f, slot + 0x2A0, flashes);
    }
    // a3 as the draws' last library call left it, for the unit flash's
    // first TEXFLUSH (b264's rules); with no draw the emitter's.
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        for (g, r, rows, b0) in [(RING_A, 0x80u32, 5, 1), (RING_B, 0x90, 5, 0), (RING_C, 0xA0, 5, 0)] {
            if f == g {
                let w = fx.mem.read_u32(TEX);
                fx.mem.write_u32(D + 0x88, w);
                cell(fx, t, sp + r, 0x14);
                fx.mem.set16(D + 0xB8, rows);
                fx.mem.set16(D + 0xB0, b0);
                billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
                a3 = 0;
                fx.mem.set16(D + 0xB8, 0);
                fx.mem.set16(D + 0xB0, 0);
            }
        }
        if f == ARC {
            fx.mem.set16(D + 0xB8, 0x18);
            let before = fx.out.prims.len();
            billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
            a3 = last_xyz2(fx, before, a3);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == TINT {
            let h = fx.target_handle();
            fx.unit_position(h, sp + 0x70);
            // 0x1F71A0's frame below the entry's (its saved ra, s0 = D).
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_4530), (0, u64::from(D))]);
            let z = fx.mem.f(sp + 0x78);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 60.0) } else { ee::add(z, 60.0) };
            fx.mem.setf(sp + 0x78, z);
            let p = [fx.mem.f(sp + 0x70), fx.mem.f(sp + 0x74), fx.mem.f(sp + 0x78)];
            let mut x = fx.mem.read_u64(sp + 0xE8);
            if let Some(w) = fx.project(p, None, &mut x) {
                fx.mem.write_u64(sp + 0xE8, x);
                fx.mem.setf(TINT_W, w);
            }
            project_residue(fx, sp, sp + 0x70);
            let w = fx.mem.f(TINT_W);
            let before = fx.out.prims.len();
            quads4::tint4(fx, t, 0x21, w);
            if let Some(p) = fx.out.prims.get(before..).and_then(|p| p.last()) {
                let d = p.desc;
                a3 = u32::from(fx.mem.read_u16(d + 2));
            }
        }
        debug_assert_ne!(f, QUAD);
        if f == TRAIL {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x3F)] {
                fx.mem.set16(D + o, v);
            }
            let i = fx.mem.i16(t + 0x52);
            copy3(fx, D + 0x20, point(i));
            fx.mem.set16(D + 0xBE, 1);
            let k = fx.mem.i16(t + 0x50);
            let before = fx.out.prims.len();
            let runs = fx.mem.i16(t + 0xE) < fx.mem.i16(t + 6) - 1;
            tubetrail::tube_with(fx, &tubetrail::SINGLE_BLEND, D, t, trail_of(k), 0x21);
            if !runs {
                // No segment: a3 = 0x1097D0's trans (the tube's sp + 0x1E0).
                a3 = fx.sp - 0x350 + 0x1E0;
            }
            // a3 = the first vertex's XYZ2 (+0x20) (b264's rule).
            if let Some(p) = fx.out.prims.get(before..).and_then(|p| p.last()) {
                let d = p.desc;
                a3 = fx.mem.read_u32(d + 0x20);
            }
        }
        if f == GLOW {
            let w = fx.mem.read_u32(TEX + 0xC);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x56) << 6;
            for (o, v) in [(0x94u32, c), (0x96, 0), (0x98, 0x3F), (0x9A, 0x3F)] {
                fx.mem.set16(D + o, v);
            }
            let r = fx.rand_from(D, 0x01E0_4744);
            let (_, h) = fx.div(r >> 4, 3);
            fx.mem.set16(D + 0xBE, h);
            copy3(fx, sp + 0x70, t + 0x10);
            billboard::quad(fx, uv::uv_rect, D, t, 0x21, sp + 0x70);
            a3 = 0;
        }
    }
    if flashes != 0 {
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x8E => 1,
        0x104 => 3,
        0x99..=0xFE => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// `flashes`: the entry's s1 (the flash tasks so far) at the `jalr`.
fn run_task(fx: &mut Fx, f: u32, t: u32, flashes: u32) {
    match f {
        RING_A => ring_a(fx, t, flashes),
        RING_B => ring_b(fx, t, flashes),
        RING_C => ring_c(fx, t),
        ARC => arc(fx, t, flashes),
        TINT => tint(fx, t),
        TRAIL => trail(fx, t, flashes),
        GLOW => glow(fx, t, flashes),
        _ => panic!("b252: unknown task step {f:#x}"),
    }
}

/// 0x1E022A0(D, 0): the update counter, the points and the spawns.
/// Returns a3 as its last call left it: 1 after 0x1F6830 (the points
/// below update 0x62), 0 after a spawn [run].
fn emit(fx: &mut Fx, param: i32) -> u32 {
    let mut a3 = 0;
    let sp = fx.enter(0x60);
    let d = u64::from(D);
    fx.saved(sp, &[(0x30, 0x01E0_4290), (0x20, 0), (0x10, 0), (0, d)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    for j in 0..10u32 {
        let p = POINTS + 0x10 * j;
        let s1 = u64::from(D + 0x10 * j);
        let n = fx.mem.i16(COUNTER);
        if n < 0x62 {
            let h = fx.self_handle();
            let s = fx.mem.i16(NODES);
            crate::b076::node_position_saving(fx, h, s, p, u64::from(j), Some(s1), 0x01E0_231C);
            a3 = 1;
            continue;
        }
        if n == 0x62 {
            let h = fx.target_handle();
            fx.unit_position(h, sp + 0x40);
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_2344), (0, u64::from(j))]);
            let v = VELOCITY + 0x10 * j;
            for c in [8u32, 4] {
                let x = ee::div(ee::sub(fx.mem.f(sp + 0x40 + c), fx.mem.f(p + c)), 50.0);
                fx.mem.setf(v + c, x);
            }
            let jit = JITTER + 0x10 * j;
            for (c, ra) in [(0u32, 0x01E0_2398), (4, 0x01E0_23C8), (8, 0x01E0_23F8)] {
                let r = fx.rand_from(j, ra);
                let (_, h) = fx.div(r >> 4, 0x14);
                fx.mem.setf(jit + c, ee::from_i32(h - 0xA));
            }
            for c in [0u32, 4, 8] {
                let x = ee::div(fx.mem.f(jit + c), 20.0);
                fx.mem.setf(jit + c, x);
            }
        }
        let n = fx.mem.i16(COUNTER);
        if n < 0x97 {
            let v = VELOCITY + 0x10 * j;
            for c in [8u32, 4] {
                let x = ee::add(fx.mem.f(p + c), fx.mem.f(v + c));
                fx.mem.setf(p + c, x);
            }
        }
        let n = fx.mem.i16(COUNTER);
        if (0x98..0xBD).contains(&n) {
            let jit = JITTER + 0x10 * j;
            let cs: &[u32] = if n >= 0xA2 { &[8, 4, 0] } else { &[4, 0] };
            for &c in cs {
                let x = ee::add(fx.mem.f(p + c), fx.mem.f(jit + c));
                fx.mem.setf(p + c, x);
            }
        }
    }
    for a in 0..8u32 {
        let (p, q, o) = (POINTS + 0x10 * a, POINTS + 0x10 * (a + 1), DELTAS + 0x10 * a);
        for c in [0u32, 4, 8] {
            let x = ee::div(ee::sub(fx.mem.f(q + c), fx.mem.f(p + c)), 5.0);
            fx.mem.setf(o + c, x);
        }
    }
    // The spawns' saved s0..s4: the loop counter (10 after the points), s1
    // = D + 0xA0, s2 = D, the entry's s3 (param) and s4 (the runner's, 0).
    let r = |s0: u64| [s0, d + 0xA0, d, param as i64 as u64, 0];
    for (f, from, ra_odd, ra_even) in [(RING_A, 0x19, 0x01E0_26D8u32, 0x01E0_2770u32), (RING_B, 0x9A, 0x01E0_2840, 0x01E0_28D8)] {
        let n = fx.mem.i16(COUNTER);
        if (from..0x100).contains(&n) {
            let (first, ra) = if n.wrapping_rem(2) == 1 { (0, ra_odd) } else { (4, ra_even) };
            for k in 0..4 {
                fx.mem.set16(INDEX, first + k);
                spawn(fx, f, r(10), ra + 0x24 * k as u32);
                a3 = 0;
            }
        }
    }
    let n = fx.mem.i16(COUNTER);
    if (0x19..0x8D).contains(&n) {
        copy4(fx, D + 0x250, POINTS);
        spawn(fx, RING_C, r(10), 0x01E0_29A4);
        a3 = 0;
    }
    if fx.mem.i16(COUNTER) == 0x98 {
        copy4(fx, D + 0x250, POINTS);
        spawn(fx, ARC, r(10), 0x01E0_29F8);
        spawn(fx, TINT, r(10), 0x01E0_2A0C);
        a3 = 0;
    }
    if fx.mem.i16(COUNTER) == 0x14 {
        spawn(fx, FLASH, r(10), 0x01E0_2A38);
        a3 = 0;
    }
    let n = fx.mem.i16(COUNTER);
    if (0x98..0x100).contains(&n) {
        let k = fx.mem.i16(TRAIL_K);
        fx.mem.set16(INDEX, k);
        spawn(fx, TRAIL, r(0), 0x01E0_2A90);
        a3 = 0;
        let k = fx.mem.i16(TRAIL_K) + 1;
        fx.mem.set16(TRAIL_K, k);
        if fx.mem.i16(TRAIL_K) >= 0x50 {
            fx.mem.set16(TRAIL_K, 0);
        }
    }
    if fx.mem.i16(COUNTER) == 0x18 {
        for k in 0..8 {
            fx.mem.set16(INDEX, k);
            spawn(fx, GLOW, r(10), 0x01E0_2B28 + 0x24 * k as u32);
            a3 = 0;
        }
    }
    let n = fx.mem.i16(COUNTER);
    if (0x22..0x62).contains(&n) {
        copy4(fx, D + 0x250, POINTS);
    }
    fx.leave(0x60);
    a3
}

/// 0x1E04880(D, step, 0): b264's spawn on this record's globals; `regs`
/// the caller's s0..s4.
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
            copy3(fx, t + 0x10, D + 0x250);
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

fn odd(v: i32) -> bool {
    v.wrapping_rem(2) == 1
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// A step's frame with the entry's s0 = D (and s1 = the flash count when
/// `n` is 2) saved, ra at `frame - 0x10`.
fn enter_step(fx: &mut Fx, frame: u32, n: u32, flashes: u32) -> u32 {
    let sp = fx.enter(frame);
    let vals = [u64::from(D), u64::from(flashes)];
    for k in 0..n {
        fx.saved(sp + 0x10 * k, &[(0, vals[k as usize])]);
    }
    fx.saved(sp, &[(frame - 0x10, TASK_RA)]);
    sp
}

/// In while younger than +6 (up by `up`), then out (`down`).
fn in_out(fx: &mut Fx, t: u32, up: (u8, u8), down: (u8, u8)) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, up);
    } else {
        fade_out(fx, t, 0x63, down);
    }
}

/// The ring radius +0x50 grown by +0xE after the first update, then the
/// age and the point +0x52 into +0x10.
fn ring_tail(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
    let i = fx.mem.i16(t + 0x52);
    copy4(fx, t + 0x10, point(i));
}

/// 0x1E02C90: a white quad ring at point +0x52 (size 4, 6 from update
/// 0x8C, cell 4, turns rand % 0xFFFF, radius rand % 20 + 3 growing by rand
/// % 5 + 2, rows rand % 30), in by 0x40 for rand % 3 + 2 updates, then out
/// by 0xF.
fn ring_a(fx: &mut Fx, t: u32, flashes: u32) {
    enter_step(fx, 0x30, 2, flashes);
    if fx.mem.i16(t + 0x58) == 1 {
        let s = if fx.mem.i16(COUNTER) < 0x8C { 0x4080_0000 } else { 0x40C0_0000 };
        fx.mem.write_u32(t + 0x48, s);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        for (o, ra) in [(0x20u32, 0x01E0_2D04), (0x24, 0x01E0_2D20), (0x28, 0x01E0_2D3C)] {
            let v = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let v = rand_mod(fx, t, 0x01E0_2D58, 0x14);
        fx.mem.set16(t + 0x50, v + 3);
        let v = rand_mod(fx, t, 0x01E0_2D78, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_2D94, 5);
        fx.mem.set16(t + 0xE, v + 2);
        let v = rand_mod(fx, t, 0x01E0_2DB4, 0x1E);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_2DD0, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    in_out(fx, t, (0xC0, 0x40), (0x11, 0xF));
    ring_tail(fx, t);
    fx.leave(0x30);
}

/// The shared first update of 0x1E02EC0 / 0x1E03090: size, cell rand % 7,
/// white, turn rand % 360, row rand % 8, +0xC = the index, radius / growth
/// / rows / life from rand.
fn ring_init(fx: &mut Fx, t: u32, size: u32, ras: [u32; 7], radius: i32, grow: (i32, i32)) {
    fx.mem.write_u32(t + 0x48, size);
    let v = rand_mod(fx, t, ras[0], 7);
    fx.mem.set16(t + 0x56, v);
    l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    let v = rand_mod(fx, t, ras[1], 0x168);
    fx.mem.set16(t + 0x5A, v);
    let v = fx.rand_from(t, ras[2]);
    fx.mem.set16(t + 0xA, v.wrapping_rem(8));
    let i = fx.mem.i16(t + 0x50);
    fx.mem.set16(t + 0xC, i);
    let v = rand_mod(fx, t, ras[3], 0x32);
    fx.mem.set16(t + 0x50, v + radius);
    let v = rand_mod(fx, t, ras[4], grow.0);
    fx.mem.set16(t + 0xE, v + grow.1);
    let v = rand_mod(fx, t, ras[5], 0x23);
    fx.mem.set16(t + 8, v);
    let v = rand_mod(fx, t, ras[6], 5);
    fx.mem.set16(t + 6, v + 3);
    fx.mem.set16(t + 0x58, 2);
}

/// 0x1E02EC0: a white quad ring at point +0x52 (size 4, radius rand % 50 +
/// 10 growing by rand % 20 + 10), 0x80 for rand % 5 + 3 updates, then out
/// by 8.
fn ring_b(fx: &mut Fx, t: u32, flashes: u32) {
    enter_step(fx, 0x30, 2, flashes);
    if fx.mem.i16(t + 0x58) == 1 {
        let ras = [0x01E0_2EF0, 0x01E0_2F1C, 0x01E0_2F38, 0x01E0_2F5C, 0x01E0_2F7C, 0x01E0_2F9C, 0x01E0_2FB8];
        ring_init(fx, t, 0x4080_0000, ras, 0xA, (0x14, 0xA));
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, (0xA, 8));
    }
    ring_tail(fx, t);
    fx.leave(0x30);
}

/// 0x1E03090: a white quad ring where it was spawned (size 3, radius rand
/// % 50 + 20, rand % 30 + 16 set but not grown), 0xC0 for rand % 5 + 3
/// updates, then out by 0xC.
fn ring_c(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0);
    if fx.mem.i16(t + 0x58) == 1 {
        let ras = [0x01E0_30B8, 0x01E0_30E4, 0x01E0_3100, 0x01E0_3124, 0x01E0_3144, 0x01E0_3164, 0x01E0_3180];
        ring_init(fx, t, 0x4040_0000, ras, 0x14, (0x1E, 0x10));
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade_out(fx, t, 0x63, (0xE, 0xC));
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03210: a camera-facing arc of radius 200, pink over red, in by 0x30
/// for 12 updates, then out by 0xF.
fn arc(fx: &mut Fx, t: u32, flashes: u32) {
    enter_step(fx, 0x20, 1, 0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        fx.mem.set16(t + 6, 0xC);
        l2::rgb(fx, t + 0x60, 0xFF, 0xCF, 0xCF);
        l2::rgb(fx, t + 0x64, 0xFF, 0x20, 0x20);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    in_out(fx, t, (0xC0, 0x30), (0x11, 0xF));
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_32DC);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03310: the red tint, both alphas in by 0x2A below 0x80 for 12
/// updates, then out by 0xC.
fn tint(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0xC0, 0x60, 0x60);
        l2::rgb(fx, t + 0x64, 0xC0, 0x60, 0x60);
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

/// 0x1E03530: trail +0x50 (k, its delta +0x52 = k % 6): a head D+0xD430 +
/// 0x10 k from 0 moving by its delta and jittered by rand % 80 - 40
/// twelfths a update, pushed into its 20-point history for 7 updates while
/// its length +6 grows to 0xF, then shrinking; 0xC0 for rand % 15 + 3
/// updates, then out by 0x1D; white / grey by age.
fn trail(fx: &mut Fx, t: u32, flashes: u32) {
    enter_step(fx, 0x30, 2, flashes);
    let headk = |fx: &mut Fx| (HEADS as i32 + 0x10 * fx.mem.i16(t + 0x50)) as u32;
    let deltak = |fx: &mut Fx| (DELTAS as i32 + 0x10 * fx.mem.i16(t + 0x52)) as u32;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 6, 1);
        fx.mem.write_u8(t + 0x63, 0);
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x52, k.wrapping_rem(6));
        let h = headk(fx);
        for c in [0u32, 4, 8] {
            fx.mem.write_u32(h + c, 0);
        }
        let dl = deltak(fx);
        copy3(fx, t + 0x30, dl);
        let k = fx.mem.i16(t + 0x50);
        let (tr, h) = (trail_of(k), (HEADS as i32 + 0x10 * k) as u32);
        for j in 0..0x14u32 {
            copy3(fx, tr + 0x10 * j, h);
        }
        fx.mem.write_u32(t + 0x48, 0x42B4_0000);
        let v = ee::div(fx.mem.f(t + 0x48), 10.0);
        fx.mem.setf(t + 0x48, v);
        let r = rand_mod(fx, t, 0x01E0_36B0, 0x41);
        fx.mem.setf(t + 0x4C, ee::from_i32(r + 3));
        let v = ee::div(fx.mem.f(t + 0x4C), 10.0);
        fx.mem.setf(t + 0x4C, v);
        let r = rand_mod(fx, t, 0x01E0_36F0, 0xF);
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
    let g = if odd(fx.mem.i16(t + 0x54)) { 0x80 } else { 0xFF };
    l2::rgb(fx, t + 0x60, g, g, g);
    for (c, ra) in [(0x30u32, 0x01E0_37EC), (0x34, 0x01E0_3818), (0x38, 0x01E0_3844)] {
        let r = rand_mod(fx, t, ra, 0x50);
        fx.mem.setf(t + c, ee::from_i32(r - 0x28));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for c in [8u32, 0, 4] {
            let dl = deltak(fx);
            let h = headk(fx);
            let v = ee::add(fx.mem.f(h + c), fx.mem.f(dl + c));
            fx.mem.setf(h + c, v);
        }
        for c in [0u32, 4, 8] {
            let j = ee::div(fx.mem.f(t + 0x30 + c), 12.0);
            let h = headk(fx);
            let v = ee::add(fx.mem.f(h + c), j);
            fx.mem.setf(h + c, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 7 {
        let k = fx.mem.i16(t + 0x50);
        let h = (HEADS as i32 + 0x10 * k) as u32;
        bspawn::history(fx, trail_of(k) + 0x130, 0x14, h);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03A40: glow +0x50 (k) at point +0x52: a grey camera-facing quad
/// (22 x 22, 20 x 20 from update 0x99, cell toggling 0 / 1 from rand % 2),
/// both alphas in by 9 below 0x80 until update 0xFF, then out by 0xB (the
/// outer alpha tested twice); bright on update 1 for odd k, else for even
/// k on the other updates.
fn glow(fx: &mut Fx, t: u32, flashes: u32) {
    enter_step(fx, 0x30, 2, flashes);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x16);
        fx.mem.set16(t + 0xA, 0x16);
        let v = fx.rand_from(t, 0x01E0_3A74);
        fx.mem.set16(t + 0x56, v.wrapping_rem(2));
        l2::rgb(fx, t + 0x60, 0x70, 0x70, 0x70);
        l2::rgb(fx, t + 0x64, 0x70, 0x70, 0x70);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(COUNTER) < 0xFF {
        fade_in(fx, t, 0x63, (0x80, 9));
        fade_in(fx, t, 0x67, (0x80, 9));
    } else {
        fade_out(fx, t, 0x63, (0xD, 0xB));
        let v = fx.mem.read_u8(t + 0x63);
        if v >= 0xD {
            fx.mem.write_u8(t + 0x63, v - 0xB);
        }
    }
    if fx.mem.i16(COUNTER) == 0x99 {
        fx.mem.set16(t + 8, 0x14);
        fx.mem.set16(t + 0xA, 0x14);
    }
    let first = fx.mem.i16(COUNTER) == 1;
    let bright = if odd(fx.mem.i16(t + 0x50)) { first } else { !first };
    let g = if bright { 0xC0 } else { 0x70 };
    l2::rgb(fx, t + 0x60, g, g, g);
    let c = if fx.mem.i16(t + 0x56) != 0 { 0 } else { 1 };
    fx.mem.set16(t + 0x56, c);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_3C40);
    let i = fx.mem.i16(t + 0x52);
    copy4(fx, t + 0x10, point(i));
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03CA0(D, t): b264's flash sprite (the attacker over the window,
/// red, UVs inset by +8 / +0xA tenths over 512) into the descriptor at
/// D+0xDBE8 once, no ordering table; in by 0xA below 0x80 for 0x96
/// updates, then out by 9. `k` the unit flash's slot index.
fn flash(fx: &mut Fx, t: u32, k: u32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_4B34), (0x20, u64::from(k)), (0x10, u64::from(D))]);
    // swc1 f21 / f20: the runner's (0) [H].
    fx.mem.write_u32(sp + 4, 0);
    fx.mem.write_u32(sp, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0xC0, 0, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(FLASH_DESC);
    for c in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + c);
        fx.mem.write_u8(d + 0x10 + c, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, xy) in [(0x20u32, 0x6FF8u16), (0x30, 0x8FF8)] {
        fx.mem.write_u16(d + at, xy);
        fx.mem.write_u16(d + at + 2, xy);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    fx.mem.setf(d + 0x18, ee::div(w, 512.0));
    fx.mem.setf(d + 0x1C, ee::div(h, 512.0));
    fx.mem.setf(d + 0x28, ee::div(ee::sub(512.0, w), 512.0));
    fx.mem.setf(d + 0x2C, ee::div(ee::sub(512.0, ee::mul(3.0, h)), 512.0));
    fx.submit(d, 0, 0);
    if fx.mem.i16(t + 0x54) < 0x96 {
        fade_in(fx, t, 0x63, (0x80, 0xA));
    } else {
        fade_out(fx, t, 0x63, (0xB, 9));
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E04A30(D): b264's unit flash (0x1E04800) on this record's globals.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xA0);
    let d = sp + 0x48;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x88, sp + 0x90);
    fx.texflush(regs);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            flash(fx, slot + 0x2A0, k);
        }
    }
    let layer = fx.mem.read_u32(LAYER);
    fx.layer_end(layer, OT, 0);
    fx.leave(0xA0);
}
