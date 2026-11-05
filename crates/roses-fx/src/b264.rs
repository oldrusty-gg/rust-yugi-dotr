//! BMODULE b264 (card 568), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03DE0, data area 0x1ED0000) [F]: a pool of 250 tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step
//! +0x44 through `jalr`; spawn 0x1E04650 with the colour D+0xDE54, the
//! index D+0xDEB8 into +0x50 / +0x52 and D+0xDEAA into +0x5A) drawn with
//! the second library: flat arcs (0x1E009B0), rings of quads (0x1E00D10 on
//! the bytes D+0xDFC2), single quads (0x1E01E10), sparks (0x1E020D0) and
//! sixteen-a-burst trails drawn as the blended tube 0x1E012C0
//! (`tubetrail::tube_with(SINGLE_BLEND)`) along 20-point histories
//! D+0x7030 + 0x140 k of relative heads D+0xD430 + 0x10 k; the unit flash
//! 0x1E04800 (the attacker drawn into the render target, a red frame
//! sprite 0x1E03AA0 from update 1). Globals past D+0xD000: twenty points
//! D+0xD930 + 0x10 j following the attacker's node 0x35, from update 0xE4
//! + 15 j flying to the target over 39 updates (velocities D+0xDA70..,
//! jitter D+0xDBB0..), the deltas D+0xDCF0.. between points, textures
//! D+0xDE30.., timer D+0xDE50 = 410, OT D+0xDE5C, nodes D+0xDE88..,
//! facing D+0xDEBC, counter D+0xDEC0, sines D+0xE0C4. The emitter
//! 0x1E024D0: on update 0xE4 + 15 j (below 0x15A) at point j a rising
//! quad (0x1E02C20), a burst (0x1E02D50: on its update 0x2E a spark ring
//! 0x1E029E0, an arc 0x1E02B10, three quad rings 0x1E03030 and sixteen
//! trails 0x1E035B0) and two quad rings; from 0x4D every fourth update one
//! of sixteen arcs (0x1E03200) between nodes 0x36 / 0x38. The step
//! 0x1E03450 is never spawned [F]: not ported, nor its draw case. Returns
//! 1 / 4 / 2 / 6 / 8 on 0x108 / 0x195 / 0x18C / 0x113..0x186 / timer out.
use crate::blib::{billboard, quads4, spawn as bspawn, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xFA;
const SPARK_RING: u32 = 0x01E0_29E0;
const ARC: u32 = 0x01E0_2B10;
const RISER: u32 = 0x01E0_2C20;
const BURST: u32 = 0x01E0_2D50;
const QUADS: u32 = 0x01E0_3030;
const ARC_NODES: u32 = 0x01E0_3200;
const TRAIL: u32 = 0x01E0_35B0;
const FLASH: u32 = 0x01E0_3AA0;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_4150;
const HEADS: u32 = D + 0xD430;
const POINTS: u32 = D + 0xD930;
const DELTAS: u32 = D + 0xDCF0;
const VELOCITY: u32 = D + 0xDA70;
const TEX: u32 = D + 0xDE30;
const TIMER: u32 = D + 0xDE50;
const COLOUR: u32 = D + 0xDE54;
const OT_AT: u32 = D + 0xDE5C;
const LAYER: u32 = D + 0xDE64;
const FLASH_DESC: u32 = D + 0xDE68;
const NODES: u32 = D + 0xDE88;
const ARC_K: u32 = D + 0xDEAE;
const ARCS_N: u32 = D + 0xDEB4;
const INDEX: u32 = D + 0xDEB8;
const BURSTS: u32 = D + 0xDEBA;
const FACING: u32 = D + 0xDEBC;
const COUNTER: u32 = D + 0xDEC0;
const BYTES: u32 = D + 0xDEC2;
const BYTES_B: u32 = D + 0xDFC2;
const SINES: u32 = D + 0xE0C4;

fn trail_of(k: i32) -> u32 {
    (D as i32 + 0x7030 + 0x140 * k) as u32
}

/// The entry 0x1E03DE0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xE230);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xDEBE, param);
    fx.mem.set32(TIMER, 0x19A);
    for (k, n) in [0x35, 0x36, 0x38, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xDE98, r);
    for (k, id) in [0xEu32, 0xF].into_iter().enumerate() {
        let tex = fx.texture(4, id as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k as u32, tex);
    }
    for k in 0..5u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(v, 0x01E0_3FD8);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_4008);
        let (_, h) = fx.div(r, 0x1E);
        fx.mem.write_u8(BYTES + k, (h - 0xF) as u8);
    }
    for k in 100..0xB4u32 {
        let r = fx.rand_from(D, 0x01E0_4064);
        fx.mem.write_u8(BYTES + k, r.wrapping_rem(4) as u8);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_40BC);
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
    emit(fx, param);
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
        run_task(fx, f, slot + 0x2A0, [flashes, slot, k]);
    }
    let mut a3 = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == SPARK_RING {
            cell(fx, t, sp + 0x70, 0x8C);
            let w = fx.mem.read_u32(D + 0x298);
            fx.mem.write_u32(D + 0x88, w);
            let w = fx.mem.read_u32(TEX + 0x10);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x1F), (0x9A, 0x7F), (0xB8, 0)] {
                fx.mem.set16(D + o, v);
            }
            copy3(fx, D + 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0x18);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == ARC || f == BURST {
            fx.mem.set16(D + 0xB8, 0x18);
            let before = fx.out.prims.len();
            billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
            a3 = last_xyz2(fx, before, a3);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == RISER {
            let w = fx.mem.read_u32(TEX + 0xC);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x80, 0x14);
            fx.mem.set16(D + 0xBE, 5);
            copy3(fx, sp + 0x60, t + 0x10);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
            a3 = 0;
        }
        if f == QUADS {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x90, 0x14);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if f == ARC_NODES {
            fx.mem.set16(D + 0xB8, 0x12);
            let before = fx.out.prims.len();
            billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
            a3 = last_xyz2(fx, before, a3);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == TRAIL {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x3F)] {
                fx.mem.set16(D + o, v);
            }
            copy3(fx, D + 0x20, t + 0x10);
            fx.mem.set16(D + 0xBE, 1);
            let i = fx.mem.i16(t + 0x50);
            let before = fx.out.prims.len();
            let runs = fx.mem.i16(t + 0xE) < fx.mem.i16(t + 6) - 1;
            tubetrail::tube_with(fx, &tubetrail::SINGLE_BLEND, D, t, trail_of(i), 0x21);
            if !runs {
                // No segment: a3 = 0x1097D0's trans (the tube's sp + 0x1E0).
                a3 = fx.sp - 0x350 + 0x1E0;
            }
            // a3 = the first vertex's XYZ2 (+0x20), the last the far-Z
            // chain (0x109C60 kind 0: +0x68, +0x50, +0x38, +0x20) loads.
            if let Some(p) = fx.out.prims.get(before..).and_then(|p| p.last()) {
                let d = p.desc;
                a3 = fx.mem.read_u32(d + 0x20);
            }
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
        0x108 => 1,
        0x195 => 4,
        0x18C => 2,
        0x113..=0x186 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// `regs`: the entry's s1 (the flash tasks so far), s3 (the slot), s4
/// (the index) at the `jalr`.
fn run_task(fx: &mut Fx, f: u32, t: u32, regs: [u32; 3]) {
    match f {
        SPARK_RING => spark_ring(fx, t),
        ARC => arc(fx, t, regs),
        RISER => riser(fx, t, regs),
        BURST => burst(fx, t, regs),
        QUADS => quads(fx, t, regs),
        ARC_NODES => arc_nodes(fx, t, regs),
        TRAIL => trail(fx, t, regs),
        _ => panic!("b264: unknown task step {f:#x}"),
    }
}

/// 0x1E024D0(D, 0): the update counter +0xDEC0, the points and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x80);
    let d = u64::from(D);
    fx.saved(sp, &[(0x50, 0x01E0_4110), (0x40, 0), (0x30, param as i64 as u64), (0x20, 0), (0x10, 0), (0, d)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    for j in 0..20u32 {
        let (s1, s3) = (0xE4 + 15 * j as i32, 0x10C + 15 * j as i32);
        let p = POINTS + 0x10 * j;
        let n = fx.mem.i16(COUNTER);
        if n < 0xC9 {
            continue;
        }
        if n < s1 {
            let h = fx.self_handle();
            let s = fx.mem.i16(NODES);
            crate::b076::node_position_saving(fx, h, s, p, u64::from(j), Some(s1 as u64), 0x01E0_2568);
            continue;
        }
        if n == s1 {
            let h = fx.target_handle();
            fx.unit_position(h, sp + 0x60);
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_2598), (0, u64::from(j))]);
            let v = VELOCITY + 0x10 * j;
            for c in [8u32, 4] {
                let x = ee::div(ee::sub(fx.mem.f(sp + 0x60 + c), fx.mem.f(p + c)), 39.0);
                fx.mem.setf(v + c, x);
            }
            let jit = D + 0xDBB0 + 0x10 * j;
            for (c, ra) in [(0u32, 0x01E0_25EC), (4, 0x01E0_261C), (8, 0x01E0_264C)] {
                let r = fx.rand_from(j, ra);
                let (_, h) = fx.div(r >> 4, 0x14);
                fx.mem.setf(jit + c, ee::from_i32(h - 0xA));
            }
            for c in [0u32, 4, 8] {
                let x = ee::div(fx.mem.f(jit + c), 20.0);
                fx.mem.setf(jit + c, x);
            }
        }
        if fx.mem.i16(COUNTER) < s3 {
            let v = VELOCITY + 0x10 * j;
            for c in [8u32, 4] {
                let x = ee::add(fx.mem.f(p + c), fx.mem.f(v + c));
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
    let saves = |s0: u64, s1: u64| [s0, s1, d + 0x140, 0x238, d];
    for j in 0..20u32 {
        let s0 = 0xE4 + 15 * j as i32;
        let n = fx.mem.i16(COUNTER);
        if n >= 0x15A || n != s0 {
            continue;
        }
        let r = saves(s0 as u64, u64::from(j));
        let b = fx.mem.i16(BURSTS);
        fx.mem.set16(INDEX, b);
        spawn(fx, RISER, r, 0x01E0_2884);
        spawn(fx, BURST, r, 0x01E0_2898);
        spawn(fx, QUADS, r, 0x01E0_28AC);
        spawn(fx, QUADS, r, 0x01E0_28C0);
        let b = fx.mem.i16(BURSTS) + 1;
        fx.mem.set16(BURSTS, b);
    }
    let r = saves(0x210, 20);
    let n = fx.mem.i16(COUNTER);
    if n >= 0x4D && n.wrapping_rem(4) == 1 {
        let a = fx.mem.i16(ARCS_N);
        if a < 0x10 {
            fx.mem.set16(INDEX, a);
            spawn(fx, ARC_NODES, r, 0x01E0_2974);
            let a = fx.mem.i16(ARCS_N) + 1;
            fx.mem.set16(ARCS_N, a);
        }
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, FLASH, r, 0x01E0_29BC);
    }
    fx.leave(0x80);
}

/// 0x1E04650(D, step, 0): the first free task cleared and set off at
/// D+0x250 with the colour D+0xDE54 (twice), the index D+0xDEB8 at +0x50 /
/// +0x52 and D+0xDEAA at +0x5A; `regs` the caller's s0..s4.
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
            let a = fx.mem.i16(D + 0xDEAA);
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

/// The alpha byte `a` up by `up.1` while below `up.0` when `rising`, else
/// down by `down.1` while at least `down.0` (the task ending below).
fn fade(fx: &mut Fx, t: u32, a: u32, rising: bool, up: (u8, u8), down: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if rising {
        if v < up.0 {
            fx.mem.write_u8(t + a, v.wrapping_add(up.1));
        }
    } else if v >= down.0 {
        fx.mem.write_u8(t + a, v.wrapping_sub(down.1));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn odd(v: i32) -> bool {
    v.wrapping_rem(2) == 1
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// A step's frame with the entry's s0 = D, s1 = the flash count, s2 = 0
/// saved (as many as `n`), ra at `ra_at`.
fn enter_step(fx: &mut Fx, frame: u32, n: u32, ra_at: u32, base: u32, flashes: u32) -> u32 {
    let sp = fx.enter(frame);
    let vals = [u64::from(D), u64::from(flashes), 0];
    for k in 0..n {
        fx.saved(sp + base + 0x10 * k, &[(0, vals[k as usize])]);
    }
    fx.saved(sp, &[(ra_at, TASK_RA)]);
    sp
}

fn camera(fx: &mut Fx, t: u32, s0: u32, s1: u32, ra: u32) {
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(s0), u64::from(s1), ra);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
}

/// 0x1E029E0: a spark ring of radius 500 rising by 10, white over grey,
/// +0x60 for 6 updates then out by 0xE, inner 350 less, +0x24 + 0x100.
fn spark_ring(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0x10, 0, 0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x43FA_0000);
        fx.mem.setf(t + 0x4C, ee::sub(500.0, 58.0));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.write_u32(t + 0x34, 0x42C8_0000);
        let v = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set32(t + 0x20, 0x4000);
    }
    if fx.mem.i16(t + 0x54) < 6 {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0x10, 0xE));
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::sub(r, 350.0));
    let a = fx.mem.i32(t + 0x24).wrapping_add(0x100);
    fx.mem.set32(t + 0x24, a);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02B10: a camera-facing arc of radius 300, white over pale blue, in
/// by 0x30 for 10 updates, then out by 0xF growing by 2.
fn arc(fx: &mut Fx, t: u32, regs: [u32; 3]) {
    enter_step(fx, 0x20, 1, 0x10, 0, regs[0]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4396_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        fx.mem.set16(t + 6, 0xA);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0xFF);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    out_growing(fx, t);
    camera(fx, t, t, regs[0], 0x01E0_2BF4);
    age(fx, t);
    fx.leave(0x20);
}

/// In by 0x30 below 0xC0 for +6 updates, then out by 0xF (ending under
/// 0x11) and the radius + 2.
fn out_growing(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        let v = fx.mem.read_u8(t + 0x63);
        if v < 0xC0 {
            fx.mem.write_u8(t + 0x63, v.wrapping_add(0x30));
        }
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0x11, 0xF));
        let r = ee::add(fx.mem.f(t + 0x48), 2.0);
        fx.mem.setf(t + 0x48, r);
    }
}

/// 0x1E02C20: a camera-facing quad (50 x 50, cells 3 / 5 alternating, orange) at
/// point +0x52, in by 0x60 for 0x32 updates, then out by 9.
fn riser(fx: &mut Fx, t: u32, regs: [u32; 3]) {
    enter_step(fx, 0x30, 2, 0x20, 0, regs[0]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x32);
        fx.mem.set16(t + 0xA, 0x32);
        fx.mem.set16(t + 0x56, 3);
        fx.mem.write_u32(t + 0x34, 0x3F00_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xA0, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x32;
    if rising {
        let v = fx.mem.read_u8(t + 0x63);
        if v < 0xC0 {
            fx.mem.write_u8(t + 0x63, v.wrapping_add(0x60));
        }
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0xB, 9));
    }
    let c = if fx.mem.i16(t + 0x56) == 3 { 5 } else { 3 };
    fx.mem.set16(t + 0x56, c);
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2CF0);
    let i = fx.mem.i16(t + 0x52);
    copy4(fx, t + 0x10, (POINTS as i32 + 0x10 * i) as u32);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02D50: the burst: a camera-facing arc of radius 80 at point +0x52
/// (in for 0x32 updates, then out growing), grey by age; on its update
/// 0x2E a spark ring, an arc, three quad rings and sixteen trails.
fn burst(fx: &mut Fx, t: u32, regs: [u32; 3]) {
    enter_step(fx, 0x40, 3, 0x30, 0, regs[0]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42A0_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        fx.mem.set16(t + 6, 0x32);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0xFF);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    out_growing(fx, t);
    let g = if odd(fx.mem.i16(t + 0x54)) { 0x8F } else { 0x70 };
    l2::rgb(fx, t + 0x64, g, g, g);
    camera(fx, t, D, t, 0x01E0_2E94);
    let i = fx.mem.i16(t + 0x52);
    copy4(fx, t + 0x10, (POINTS as i32 + 0x10 * i) as u32);
    if fx.mem.i16(t + 0x54) == 0x2E {
        copy4(fx, D + 0x250, t + 0x10);
        let r = |s0: u64| [s0, u64::from(t), u64::from(D), u64::from(regs[1]), u64::from(regs[2])];
        spawn(fx, SPARK_RING, r(u64::from(D)), 0x01E0_2F18);
        spawn(fx, ARC, r(u64::from(D)), 0x01E0_2F2C);
        for ra in [0x2F40u32, 0x2F54, 0x2F68] {
            spawn(fx, QUADS, r(u64::from(D)), 0x01E0_0000 | ra);
        }
        for k in 0..0x10u64 {
            let a = fx.mem.i16(ARC_K);
            fx.mem.set16(INDEX, a);
            spawn(fx, TRAIL, r(k), 0x01E0_2F9C);
            let a = fx.mem.i16(ARC_K) + 1;
            fx.mem.set16(ARC_K, a);
            if fx.mem.i16(ARC_K) >= 0x50 {
                fx.mem.set16(ARC_K, 0);
            }
        }
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E03030: a white quad ring at point +0x52 (size 10, cell rand % 7,
/// turn rand % 360, row rand % 8, radius rand % 50 + 20 growing by rand %
/// 5 + 3, rows rand % 35), 0xC0 for rand % 5 + 3 updates, then out by 0xC.
fn quads(fx: &mut Fx, t: u32, regs: [u32; 3]) {
    enter_step(fx, 0x30, 2, 0x20, 0, regs[0]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        let v = rand_mod(fx, t, 0x01E0_3060, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let v = rand_mod(fx, t, 0x01E0_308C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_30A8);
        fx.mem.set16(t + 0xA, v.wrapping_rem(8));
        let i = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, i);
        let v = rand_mod(fx, t, 0x01E0_30CC, 0x32);
        fx.mem.set16(t + 0x50, v + 0x14);
        let v = rand_mod(fx, t, 0x01E0_30EC, 5);
        fx.mem.set16(t + 0xE, v + 3);
        let v = rand_mod(fx, t, 0x01E0_310C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_3128, 5);
        fx.mem.set16(t + 6, v + 3);
        let i = fx.mem.i16(t + 0x52);
        copy4(fx, t + 0x10, (POINTS as i32 + 0x10 * i) as u32);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0xE, 0xC));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03200: arc +0x50 (k) of sixteen: dark over yellow (by the update
/// parity), inner radius 25, at node 0x38 plus k fifths of the way to
/// node 0x36; the outer alpha in by 0x18 until update 0x140, then out by
/// 0xC.
fn arc_nodes(fx: &mut Fx, t: u32, regs: [u32; 3]) {
    let sp = fx.enter(0x70);
    let d = u64::from(D);
    fx.saved(sp, &[(0x40, TASK_RA), (0x30, 0), (0x20, u64::from(regs[0])), (0x10, d)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x20);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x60);
        fx.mem.set16(t + 6, 0x8C);
        fx.mem.write_u32(t + 0x4C, 0x41C8_0000);
        fx.mem.set16(t + 0x5A, 0x14);
        fx.mem.set16(t + 0x5C, 0x14);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(COUNTER) < 0x140;
    fade(fx, t, 0x67, rising, (0xC0, 0x18), (0xE, 0xC));
    if odd(fx.mem.i16(COUNTER)) {
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x20);
    } else {
        l2::rgb(fx, t + 0x64, 0xA0, 0xA0, 0x30);
    }
    let k = ee::from_i32(fx.mem.i16(t + 0x50));
    let (st, s1) = (u64::from(t), Some(d));
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES + 4);
    crate::b076::node_position_saving(fx, h, s, t + 0x10, st, s1, 0x01E0_3358);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES + 2);
    crate::b076::node_position_saving(fx, h, s, sp + 0x60, st, s1, 0x01E0_3378);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES + 4);
    crate::b076::node_position_saving(fx, h, s, sp + 0x50, st, s1, 0x01E0_3398);
    for c in [0u32, 4, 8] {
        let step = ee::div(ee::sub(fx.mem.f(sp + 0x60 + c), fx.mem.f(sp + 0x50 + c)), 5.0);
        let v = ee::add(fx.mem.f(t + 0x10 + c), ee::mul(k, step));
        fx.mem.setf(t + 0x10 + c, v);
    }
    camera(fx, t, t, D, 0x01E0_340C);
    age(fx, t);
    fx.leave(0x70);
}

/// 0x1E035B0: trail +0x50 (k, its delta +0x52 = k % 6): a head D+0xD430 +
/// 0x10 k from 0 circling (radius +0x4C rand % 125 + 3 tenths, angle
/// +0x5A) and jittered by rand % 80 - 40 twelfths a update, pushed into
/// its 20-point history for 7 updates while its length +6 grows to 0xF,
/// then shrinking; half width 20; 0xC0 for rand % 15 + 3 updates, then out
/// by 0x1D; white / grey by age.
fn trail(fx: &mut Fx, t: u32, regs: [u32; 3]) {
    enter_step(fx, 0x30, 2, 0x20, 0, regs[0]);
    let headk = |fx: &mut Fx| (HEADS as i32 + 0x10 * fx.mem.i16(t + 0x50)) as u32;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 6, 1);
        fx.mem.write_u8(t + 0x63, 0);
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x52, k.wrapping_rem(6));
        let h = headk(fx);
        for c in [0u32, 4, 8] {
            fx.mem.write_u32(h + c, 0);
        }
        let i = fx.mem.i16(t + 0x52);
        copy3(fx, t + 0x30, (DELTAS as i32 + 0x10 * i) as u32);
        let k = fx.mem.i16(t + 0x50);
        let (tr, h) = (trail_of(k), (HEADS as i32 + 0x10 * k) as u32);
        for j in 0..0x14u32 {
            copy3(fx, tr + 0x10 * j, h);
        }
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        let v = ee::div(fx.mem.f(t + 0x48), 10.0);
        fx.mem.setf(t + 0x48, v);
        let r = rand_mod(fx, t, 0x01E0_3730, 0x7D);
        fx.mem.setf(t + 0x4C, ee::from_i32(r + 3));
        let v = ee::div(fx.mem.f(t + 0x4C), 10.0);
        fx.mem.setf(t + 0x4C, v);
        let r = fx.rand_from(t, 0x01E0_3770);
        let (_, a) = fx.div(r >> 4, 0x168);
        fx.mem.set16(t + 0x5A, a);
        let r = rand_mod(fx, t, 0x01E0_3790, 0xF);
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
        fade(fx, t, 0x63, false, (0, 0), (0x1F, 0x1D));
    }
    let g = if odd(fx.mem.i16(t + 0x54)) { 0x80 } else { 0xFF };
    l2::rgb(fx, t + 0x60, g, g, g);
    for (c, ra) in [(0x30u32, 0x01E0_388C), (0x34, 0x01E0_38B8), (0x38, 0x01E0_38E4)] {
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

/// 0x1E03AA0(D, t): the flash's frame sprite (the attacker over the
/// window, red, UVs inset by +8 / +0xA tenths over 512) into the
/// descriptor at +0xDE68 twice, no ordering table; from update 0x4F red
/// flickering 0xC0 / 0xB0; in by 0xA below 0x80 until update 0x15E, then
/// out by 0xD. `k` the unit flash's slot index.
fn flash(fx: &mut Fx, t: u32, k: u32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_4904), (0x20, u64::from(k)), (0x10, u64::from(D))]);
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
    fx.submit(d, 0, 0);
    let n = fx.mem.i16(COUNTER);
    if n >= 0x4F {
        let r = if odd(n) { 0xC0 } else { 0xB0 };
        l2::rgb(fx, t + 0x60, r, 0, 0);
    }
    let rising = fx.mem.i16(COUNTER) < 0x15E;
    fade(fx, t, 0x63, rising, (0x80, 0xA), (0xF, 0xD));
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E04800(D): in a packet layer (0x2078A0) the attacker drawn into the
/// render target (0x1F8270 filter 0, its TEX0 / TEX1 into the sprite
/// descriptor at sp + 0x48; ALPHA 0x48), TEXFLUSH before and after; every
/// flash task stepped; the layer linked into the ordering table at key 0.
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
