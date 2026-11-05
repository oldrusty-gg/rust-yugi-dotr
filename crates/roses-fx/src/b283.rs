//! BMODULE b283 (card 90), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03810, data area 0x1ED0000) [F]: a pool of 150 tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step
//! +0x44 through `jalr`; spawn 0x1E04320, b701's on the colour D+0xA264 /
//! flag D+0xA2C2) drawn with the second library: rings of quads
//! (`billboard::ring_quads` 0x1E00B80 on the bytes D+0xA2CC), flat arcs
//! (`arc_strips_flat` 0x1E01130), sparks (`quads4::sparks` 0x1E01490),
//! tints (`quads4::tint4` 0x1E009B0) and four green trails drawn as the
//! untextured gouraud tube (`tubetrail::tube_gouraud(GOURAUD4)`
//! 0x1E044B0) along 70-point histories D+0x48D0 + 0x460 k. Globals past
//! D+0xA000: points D+0xA060 + 0x10 k (the attacker's nodes 0x2C / 0x43
//! twice), heads D+0xA100 + 0x10 k, timer D+0xA260 = 340, OT D+0xA26C,
//! the tint's w D+0xA274, 250.0 at D+0xA280, nodes D+0xA294.., the
//! target's node 1 D+0xA2A4, facing D+0xA2C6, counter D+0xA2CA, sines
//! D+0xA4CC. The emitter 0x1E01890: until update 0xC7 the points at the
//! nodes, then to 0xEF their z drifting 0.08 along the facing; 0x22 two
//! green arcs round nodes 0x2C / 0x43 (0x1E03120); 0xC4 two wide arcs
//! there (0x1E026F0); 0xC7 four trails (0x1E029A0: each a head circling
//! its point (radius +0x30, 8 degrees a update, pulled toward the target
//! from update 1 to 0xF8), pushed into its history for 0x32 updates,
//! shedding grey quad rings (0x1E01EA0) for 0x2D updates) each with an arc
//! at its oldest point (0x1E024B0); 0xFA at the target seven spark rings
//! (0x1E03260), two tints (0x1E036C0, white / pale green) and an arc
//! (0x1E03400); 0xFB..0x12C a quad ring a update at the target (0x1E03500).
//! The steps 0x1E020C0 / 0x1E022B0 / 0x1E025F0 / 0x1E027F0 / 0x1E02F40 are
//! never spawned [F: no spawn names them]: not ported, nor their draw
//! cases. Returns 1 / 4 / 2 / 6 / 8 on 0xF0 / 0x140 / 0x122 / 0xFB..0x121
//! / timer out.
use crate::blib::{billboard, quads4, spawn as bspawn, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0x96;
const QUADS_A: u32 = 0x01E0_1EA0;
const ARC_HIST: u32 = 0x01E0_24B0;
const ARC_WIDE: u32 = 0x01E0_26F0;
const TRAIL: u32 = 0x01E0_29A0;
const ARC_NODE: u32 = 0x01E0_3120;
const SPARKS: u32 = 0x01E0_3260;
const ARC_TARGET: u32 = 0x01E0_3400;
const QUADS_B: u32 = 0x01E0_3500;
const TINT: u32 = 0x01E0_36C0;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3AB8;
const POINTS: u32 = D + 0xA060;
const HEADS: u32 = D + 0xA100;
const TEX: u32 = D + 0xA240;
const TIMER: u32 = D + 0xA260;
const COLOUR: u32 = D + 0xA264;
const OT_AT: u32 = D + 0xA26C;
const TINT_W: u32 = D + 0xA274;
const NODES: u32 = D + 0xA294;
const FLAG: u32 = D + 0xA2C2;
const FACING: u32 = D + 0xA2C6;
const COUNTER: u32 = D + 0xA2CA;
const BYTES: u32 = D + 0xA2CC;
const SINES: u32 = D + 0xA4CC;

/// The history of trail `k` (70 points of 0x10, newest first).
fn history(k: i32) -> u32 {
    (D as i32 + 0x48D0 + 0x460 * k) as u32
}

/// The entry 0x1E03810.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x170);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x170);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xA640);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xA2C8, param);
    fx.mem.set32(TIMER, 0x154);
    for (k, n) in [0x2C, 0x43, 1].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xA2A4, r);
    for (k, id) in [0xEu32, 0xF].into_iter().enumerate() {
        let tex = fx.texture(4, id as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k as u32, tex);
    }
    let tex = fx.texture(5, 0);
    fx.mem.write_u32(TEX, tex);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.fix_residue(v, 0x01E0_39A0);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_39D0);
        let (_, h) = fx.div(r, 0x1E);
        fx.mem.write_u8(BYTES + k, (h - 0xF) as u8);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_3A24);
        let (_, h) = fx.div(r, 0x28);
        fx.mem.write_u8(D + 0xA3CC + k, (h - 0x14) as u8);
    }
    fx.mem.write_u32(D + 0xA280, 0x437A_0000);
    0
}

/// The atlas cell (0x106960 into `r`, frame +0x56) at D+0x94..+0xA3 and
/// the texture D+0xA240 at D+0x88.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let w = fx.mem.read_u32(TEX);
    fx.mem.write_u32(D + 0x88, w);
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// A quad ring of `n` quads, the orientation rand % 4 (rand from `ra`).
fn quads(fx: &mut Fx, t: u32, r: u32, n: i32, ra: u32) {
    cell(fx, t, r);
    fx.mem.set16(D + 0xB8, n);
    let v = fx.rand_from(D, ra);
    fx.mem.set16(D + 0xBE, v.wrapping_rem(4));
    fx.mem.set16(D + 0xB0, 0);
    billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
    fx.mem.set16(D + 0xB8, 0);
}

/// 0x1F71A0 (the target's position into `out`) with its frame (ra, s0).
fn target_at(fx: &mut Fx, sp: u32, out: u32, s0: u32, ra: u32) {
    let h = fx.target_handle();
    fx.unit_position(h, out);
    fx.saved(sp - 0x20, &[(0x10, u64::from(ra)), (0, u64::from(s0))]);
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
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
        if f == 0 {
            continue;
        }
        run_task(fx, f, slot + 0x2A0, k);
    }
    let mut live = 0u64;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        live = u64::from((live as i32 + 1) as i16 as u16);
        let f = fx.mem.read_u32(t + 0x44);
        if f == QUADS_A {
            quads(fx, t, sp + 0x70, 3, 0x01E0_3B68);
        }
        if f == ARC_WIDE {
            fx.mem.set16(D + 0xB8, 0x24);
            let h = fx.self_handle();
            let i = fx.mem.i16(t + 0x50);
            let s = fx.mem.i16((NODES as i32 + 2 * i) as u32);
            crate::b076::node_position_saving(fx, h, s, sp + 0x60, u64::from(D), Some(live), 0x01E0_3CFC);
            billboard::arc_strips_flat(fx, D, t, 0x21, sp + 0x60);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == ARC_HIST {
            fx.mem.set16(D + 0xB8, 0x24);
            let i = fx.mem.i16(t + 0x50);
            copy3(fx, sp + 0x60, history(i));
            billboard::arc_strips_flat(fx, D, t, 0x21, sp + 0x60);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == TRAIL {
            let i = fx.mem.i16(t + 0x50);
            tubetrail::tube_gouraud(fx, &tubetrail::GOURAUD4, D, t, history(i), 0);
        }
        if f == ARC_NODE {
            fx.mem.set16(D + 0xB8, 0x24);
            let h = fx.self_handle();
            let i = fx.mem.i16(t + 0x50);
            let s = fx.mem.i16((NODES as i32 + 2 * i) as u32);
            crate::b076::node_position_saving(fx, h, s, sp + 0x60, u64::from(D), Some(live), 0x01E0_3F84);
            billboard::arc_strips_flat(fx, D, t, 0x21, sp + 0x60);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == SPARKS {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x1F), (0x9A, 0x7F), (0xB8, 0)] {
                fx.mem.set16(D + o, v);
            }
            target_at(fx, sp, sp + 0x60, D, 0x01E0_3FF0);
            fx.mem.write_u32(sp + 0x64, 0x4248_0000);
            let frame = fx.mem.i16(t + 0x56);
            l2::rect(fx, sp + 0xB0, 0x14, frame);
            for c in 0..4 {
                let v = fx.mem.read_u32(sp + 0xB0 + 4 * c);
                fx.mem.write_u32(D + 0x94 + 4 * c, v);
            }
            let h = fx.mem.i16(D + 0x9A) >> 1;
            fx.mem.set16(D + 0x9A, h);
            fx.mem.set16(D + 0xB8, 0x18);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, sp + 0x60);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == ARC_TARGET {
            fx.mem.set16(D + 0xB8, 0x24);
            target_at(fx, sp, sp + 0x60, D, 0x01E0_408C);
            fx.mem.write_u32(sp + 0x64, 0x4248_0000);
            billboard::arc_strips_flat(fx, D, t, 0x21, sp + 0x60);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == QUADS_B {
            quads(fx, t, sp + 0xC0, 3, 0x01E0_4118);
        }
        if f == TINT {
            target_at(fx, sp, sp + 0x60, D, 0x01E0_4178);
            let z = fx.mem.f(sp + 0x68);
            let back = fx.mem.i16(FACING) != 0;
            fx.mem.setf(sp + 0x68, if back { ee::sub(z, 30.0) } else { ee::add(z, 30.0) });
            let v = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
            let mut xy = fx.mem.read_u64(sp + 0xF8);
            if let Some(w) = fx.project(v, None, &mut xy) {
                fx.mem.write_u64(sp + 0xF8, xy);
                fx.mem.setf(TINT_W, w);
            }
            crate::b590::project_residue(fx, sp, sp + 0x60);
            match fx.mem.i16(t + 0x50) {
                0 => {
                    let w = fx.mem.f(TINT_W);
                    quads4::tint4(fx, t, 1, w);
                }
                1 => {
                    let w = fx.mem.f(TINT_W);
                    quads4::tint4(fx, t, 0x20, w);
                }
                _ => {}
            }
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xF0 => 1,
        0x140 => 4,
        0x122 => 2,
        0xFB..=0x121 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn run_task(fx: &mut Fx, f: u32, t: u32, k: u32) {
    match f {
        QUADS_A => quads_step(fx, t, QUADS_A_K),
        ARC_HIST => arc_hist(fx, t),
        ARC_WIDE => arc_wide(fx, t),
        TRAIL => trail(fx, t, k),
        ARC_NODE => arc_node(fx, t),
        SPARKS => sparks(fx, t),
        ARC_TARGET => arc_target(fx, t),
        QUADS_B => quads_step(fx, t, QUADS_B_K),
        TINT => tint(fx, t),
        _ => panic!("b283: unknown task step {f:#x}"),
    }
}

/// 0x1E01890(D, 0): the update counter +0xA2CA, the points and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x10, 0x01E0_3A88), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let n = fx.mem.i16(COUNTER);
    if n < 0xC7 {
        let calls = [(2u32, sp + 0x20, 0x01E0_18F0u32), (0, POINTS, 0x01E0_1914), (1, POINTS + 0x10, 0x01E0_1938), (0, POINTS + 0x20, 0x01E0_195C), (1, POINTS + 0x30, 0x01E0_1980)];
        for (i, out, ra) in calls {
            let h = fx.self_handle();
            let s = fx.mem.i16(NODES + 2 * i);
            crate::b076::node_position_saving(fx, h, s, out, u64::from(D), Some(0), ra);
        }
    } else if n < 0xF0 {
        for k in 0..4u32 {
            let a = POINTS + 8 + 0x10 * k;
            let z = fx.mem.f(a);
            let by = f32::from_bits(0x3DA3_D70A);
            let back = fx.mem.i16(FACING) != 0;
            fx.mem.setf(a, if back { ee::sub(z, by) } else { ee::add(z, by) });
        }
    }
    let regs = |fx: &mut Fx, f: u32, ra: u32| spawn(fx, f, [u64::from(D), 0, param as i64 as u64, 0, 0], ra);
    if fx.mem.i16(COUNTER) == 0x22 {
        fx.mem.set16(FLAG, 0);
        regs(fx, ARC_NODE, 0x01E0_1B8C);
        fx.mem.set16(FLAG, 1);
        regs(fx, ARC_NODE, 0x01E0_1BB0);
    }
    if fx.mem.i16(COUNTER) == 0xFA {
        for ra in [0x1BDCu32, 0x1BF0, 0x1C04, 0x1C18, 0x1C2C, 0x1C40, 0x1C54] {
            regs(fx, SPARKS, 0x01E0_0000 | ra);
        }
        fx.mem.set16(FLAG, 0);
        regs(fx, TINT, 0x01E0_1C74);
        fx.mem.set16(FLAG, 1);
        regs(fx, TINT, 0x01E0_1C98);
        regs(fx, ARC_TARGET, 0x01E0_1CAC);
    }
    let n = fx.mem.i16(COUNTER);
    if (0xFB..0x12D).contains(&n) {
        regs(fx, QUADS_B, 0x01E0_1CE4);
    }
    if fx.mem.i16(COUNTER) == 0xC7 {
        for (k, ra) in [(0, [0x1D1Cu32, 0x1D30]), (1, [0x1D54, 0x1D68]), (2, [0x1D8C, 0x1DA0]), (3, [0x1DC4, 0x1DD8])] {
            fx.mem.set16(FLAG, k);
            regs(fx, TRAIL, 0x01E0_0000 | ra[0]);
            regs(fx, ARC_HIST, 0x01E0_0000 | ra[1]);
        }
    }
    if fx.mem.i16(COUNTER) == 0xC4 {
        fx.mem.set16(FLAG, 0);
        regs(fx, ARC_WIDE, 0x01E0_1E10);
        fx.mem.set16(FLAG, 1);
        regs(fx, ARC_WIDE, 0x01E0_1E34);
        for v in [3, 4, 5] {
            fx.mem.set16(FLAG, v);
        }
    }
    if fx.mem.i16(COUNTER) == 0xD2 {
        fx.mem.set16(FLAG, 0);
    }
    fx.leave(0x50);
}

/// 0x1E04320(D, step, 0): the first free task cleared and set off at
/// D+0x250 with the colour D+0xA264 (twice), the flag D+0xA2C2 at +0x50;
/// `regs` the caller's s0..s4, `ra` the return address.
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
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(FLAG);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand % m (HI) from a step's rand returning to `ra` (s0 `s0`).
fn rand_mod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The alpha byte `a` up by `up.1` while below `up.0` when `rising`, else
/// down by `down.1` while at least `down.0` (the task ending below when
/// `ends`).
fn fade(fx: &mut Fx, t: u32, a: u32, rising: bool, up: (u8, u8), down: (u8, u8), ends: bool) {
    let v = fx.mem.read_u8(t + a);
    if rising {
        if v < up.0 {
            fx.mem.write_u8(t + a, v.wrapping_add(up.1));
        }
    } else if v >= down.0 {
        fx.mem.write_u8(t + a, v.wrapping_sub(down.1));
    } else if ends {
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

/// The steps' frames: 0x20 (ra +0x10, the entry's s0 = D) or 0x30 (ra
/// +0x20, s1 = 0 +0x10, s0 = D); 0x70 adds s2 (the slot) at +0x20 and ra
/// at +0x30.
fn enter_step(fx: &mut Fx, frame: u32, slot: u32) -> u32 {
    let sp = fx.enter(frame);
    let d = u64::from(D);
    match frame {
        0x20 => fx.saved(sp, &[(0x10, TASK_RA), (0, d)]),
        0x30 => fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, d)]),
        _ => fx.saved(sp, &[(0x30, TASK_RA), (0x20, u64::from(slot)), (0x10, 0), (0, d)]),
    }
    sp
}

/// The quad rings' constants: size base, +0xE rand / base, rand return
/// addresses (7), whether the centre is at the target.
struct QuadsK {
    size: (i32, i32),
    turn: (i32, i32),
    ras: [u32; 7],
    target: bool,
    frame: u32,
}

/// 0x1E01EA0: size 10..19 at the history of trail +0x50 (a 0x30 frame).
const QUADS_A_K: QuadsK = QuadsK { size: (10, 10), turn: (10, 6), ras: [0x1ECC, 0x1EF8, 0x1F8C, 0x1FAC, 0x1FC8, 0x1FE8, 0x2004], target: false, frame: 0x30 };
/// 0x1E03500: size 30..59 at the target (50 up).
const QUADS_B_K: QuadsK = QuadsK { size: (30, 30), turn: (30, 26), ras: [0x3524, 0x3550, 0x357C, 0x359C, 0x35B8, 0x35D8, 0x35F4], target: true, frame: 0x20 };

/// 0x1E01EA0 / 0x1E03500: a grey quad ring (cell rand % 3, size rand %
/// a + b, radius +0x50 rand % 60 + 10 growing by +0xE, turn rand % 360,
/// rows rand % 35, in by 0x40 for rand % 3 + 3 updates, then out by 0xE).
fn quads_step(fx: &mut Fx, t: u32, q: QuadsK) {
    let sp = enter_step(fx, q.frame, 0);
    let ra = |k: usize| 0x01E0_0000 | q.ras[k];
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rand_mod(fx, t, ra(0), q.size.0) + q.size.1;
        fx.mem.setf(t + 0x48, ee::from_i32(v));
        let v = rand_mod(fx, t, ra(1), 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        if !q.target {
            let i = fx.mem.i16(t + 0x50);
            copy3(fx, t + 0x10, history(i));
        }
        let v = rand_mod(fx, t, ra(2), 0x3C);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rand_mod(fx, t, ra(3), 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, ra(4), q.turn.0);
        fx.mem.set16(t + 0xE, v + q.turn.1);
        let v = rand_mod(fx, t, ra(5), 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, ra(6), 3);
        fx.mem.set16(t + 6, v + 3);
        if q.target {
            target_at(fx, sp, t + 0x10, t, 0x01E0_3620);
            fx.mem.write_u32(t + 0x14, 0x4248_0000);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, 0x63, rising, (0xC0, 0x40), (0x10, 0xE), true);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
    fx.leave(q.frame);
}

/// The arcs' first update: radius `r`, white over `outer`, +0x5A / +0x5C
/// 10, the outer alpha 0.
fn arc_start(fx: &mut Fx, t: u32, r: u32, outer: Option<(i32, i32, i32)>) {
    fx.mem.write_u32(t + 0x48, r);
    l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    if let Some((a, b, c)) = outer {
        l2::rgb(fx, t + 0x64, a, b, c);
    }
    fx.mem.set16(t + 0x5A, 0xA);
    fx.mem.set16(t + 0x5C, 0xA);
    fx.mem.set16(t + 0x58, 2);
    fx.mem.write_u8(t + 0x67, 0);
}

/// The camera-facing turn (0x205580 into +0x20, + 0x4000) from `ra`.
fn face(fx: &mut Fx, t: u32, ra: u32) {
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), ra);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
}

/// The green outer colour by age: (0x10, 0x80, 0x10) on odd updates, else
/// `even`.
fn green(fx: &mut Fx, t: u32, even: (i32, i32, i32)) {
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x64, 0x10, 0x80, 0x10);
    } else {
        l2::rgb(fx, t + 0x64, even.0, even.1, even.2);
    }
}

/// 0x1E024B0: an arc of radius 100 at a history's oldest point, in by 0x26
/// until update 0x122, then out by 8; flickering green.
fn arc_hist(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 0);
    if fx.mem.i16(t + 0x58) == 1 {
        arc_start(fx, t, 0x42C8_0000, None);
    }
    let rising = fx.mem.i16(COUNTER) < 0x122;
    fade(fx, t, 0x63, rising, (0xC0, 0x26), (0xA, 8), true);
    green(fx, t, (0x10, 0xF0, 0x10));
    face(fx, t, 0x01E0_25BC);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E026F0: a grey / green arc of radius 150 at node +0x50, in by 0x40
/// below 0x80 until update 0x12C, then out by 8.
fn arc_wide(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4316_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x10, 0xFF, 0x10);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(COUNTER) < 0x12C;
    fade(fx, t, 0x63, rising, (0x80, 0x40), (0xA, 8), true);
    face(fx, t, 0x01E0_27BC);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03120: an arc of radius 40 at node +0x50, in by 0xE until update
/// 0x12C, flickering green.
fn arc_node(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 0);
    if fx.mem.i16(t + 0x58) == 1 {
        arc_start(fx, t, 0x4220_0000, None);
    }
    let rising = fx.mem.i16(COUNTER) < 0x12C;
    fade(fx, t, 0x63, rising, (0xC0, 0xE), (0xA, 8), true);
    green(fx, t, (0x20, 0xF0, 0x20));
    face(fx, t, 0x01E0_322C);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03400: an arc of radius 320 at the target (white over (0x40, 0xFF,
/// 0x40)), in by 0x60 until update 0x12C.
fn arc_target(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 0);
    if fx.mem.i16(t + 0x58) == 1 {
        arc_start(fx, t, 0x43A0_0000, Some((0x40, 0xFF, 0x40)));
    }
    let rising = fx.mem.i16(COUNTER) < 0x12C;
    fade(fx, t, 0x63, rising, (0xC0, 0x60), (0xA, 8), true);
    face(fx, t, 0x01E0_34CC);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03260: a spark ring of radius 1000 turned at random, grey over
/// dark, in by 0x40 until update 0x12C, flickering green.
fn sparks(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x447A_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x10, 0x10, 0x10);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0x14);
        fx.mem.set16(t + 0x5C, 2);
        fx.mem.set16(t + 0x56, 4);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, ra) in [(0x20u32, 0x01E0_32E0), (0x24, 0x01E0_32FC), (0x28, 0x01E0_3318)] {
            let v = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    let rising = fx.mem.i16(COUNTER) < 0x12C;
    fade(fx, t, 0x63, rising, (0xC0, 0x40), (0xA, 8), true);
    green(fx, t, (0x20, 0xF0, 0x20));
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E036C0: the tint (+0x50 0: white, 1: pale yellow over pale green)
/// in by 0x2A to 0x80 for 0x10 updates, then out by 4.
fn tint(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 0);
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0xF0);
            l2::rgb(fx, t + 0x64, 0xF0, 0xF0, 0xF0);
            fx.mem.set16(t + 6, 0x10);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xD0, 0xD0, 0x80);
            l2::rgb(fx, t + 0x64, 0x80, 0xC0, 0x80);
            fx.mem.set16(t + 6, 0x10);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade(fx, t, 0x63, true, (0x80, 0x2A), (0, 0), false);
        fade(fx, t, 0x67, true, (0x80, 0x2A), (0, 0), false);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (6, 4), true);
        let v = fx.mem.read_u8(t + 0x67);
        fx.mem.write_u8(t + 0x67, if v >= 6 { v - 4 } else { 0 });
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E029A0: trail +0x50 (k): 8 points (to 0x34) of history k; on its
/// first update the history filled with point k (D+0xA060 + 0x10 k), the
/// head D+0xA100 + 0x10 k there, the turn +0x5A 90 k degrees, the pull
/// +0x38 a fiftieth of the z from the attacker's node 1 to the target,
/// half width 20; 0xC0 until update 0x122, then out by 7 and the width
/// down by 1; green by age; from update 1 the head at the point + the
/// radius +0x30 (growing by 1.5 below 10 for 0x19 updates, then falling by
/// 0.5 above 8) cos / sin +0x5A (+8 a update), its z pulled by +0x38 until
/// update 0xF9; pushed into the history for 0x32 updates; for 0x2D
/// updates a quad ring (0x1E01EA0) spawned at the history's newest point.
fn trail(fx: &mut Fx, t: u32, k: u32) {
    let slot = t - 0x2A0;
    let sp = enter_step(fx, 0x70, slot);
    let f = fx.mem.i16(t + 0x50);
    let pt = (POINTS as i32 + 0x10 * f) as u32;
    let head = (HEADS as i32 + 0x10 * f) as u32;
    let mut s2 = u64::from(slot);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 6, 8);
        fx.mem.write_u8(t + 0x63, 0);
        let h = history(f);
        for j in 0..0x45u32 {
            copy3(fx, h + 0x10 * j, pt);
        }
        copy3(fx, head, pt);
        l2::rgb(fx, t + 0x64, 0x80, 0x10, 0x10);
        fx.mem.set16(t + 0x5A, 0);
        for (v, a) in [(1, 0xB4), (2, 0x5A), (3, 0x10E)] {
            if fx.mem.i16(t + 0x50) == v {
                fx.mem.set16(t + 0x5A, a);
            }
        }
        target_at(fx, sp, sp + 0x50, t, 0x01E0_2B38);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES + 4);
        crate::b076::node_position_saving(fx, h, s, sp + 0x40, u64::from(t), Some(u64::from(D)), 0x01E0_2B58);
        let pull = ee::div(ee::sub(fx.mem.f(sp + 0x58), fx.mem.f(sp + 0x48)), 50.0);
        fx.mem.setf(t + 0x38, pull);
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let n = fx.mem.i16(t + 6);
    if n < 0x34 {
        fx.mem.set16(t + 6, n + 1);
    }
    if fx.mem.i16(COUNTER) < 0x122 {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (9, 7), true);
        let w = fx.mem.f(t + 0x48);
        if !(w <= 0.0) {
            fx.mem.setf(t + 0x48, ee::sub(w, 1.0));
        }
    }
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x60, 0x10, 0x80, 0x10);
    } else {
        l2::rgb(fx, t + 0x60, 0x10, 0xFF, 0x10);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let f = fx.mem.i16(t + 0x50);
        let head = (HEADS as i32 + 0x10 * f) as u32;
        if fx.mem.i16(COUNTER) < 0xF9 {
            let z = ee::add(fx.mem.f(head + 8), fx.mem.f(t + 0x38));
            fx.mem.setf(head + 8, z);
        }
        for (c, cos) in [(0u32, 1), (4, 0)] {
            let a = fx.mem.i16(t + 0x5A);
            let f = fx.mem.i16(t + 0x50);
            s2 = u64::from((f << 4) as u32);
            let s = l2::sin_deg(fx, SINES, a, cos);
            let pt = (POINTS as i32 + 0x10 * f) as u32;
            let head = (HEADS as i32 + 0x10 * f) as u32;
            let v = ee::add(fx.mem.f(pt + c), ee::mul(fx.mem.f(t + 0x30), s));
            fx.mem.setf(head + c, v);
        }
        if (0..4).contains(&fx.mem.i16(t + 0x50)) {
            let a = fx.mem.i16(t + 0x5A) + 8;
            fx.mem.set16(t + 0x5A, a);
        }
        let r = fx.mem.f(t + 0x30);
        if fx.mem.i16(t + 0x54) < 0x19 {
            if ee_lt(r, 10.0) {
                fx.mem.setf(t + 0x30, ee::add(r, 1.5));
            }
        } else if !(r <= 8.0) {
            fx.mem.setf(t + 0x30, ee::sub(r, 0.5));
        }
    }
    if fx.mem.i16(t + 0x54) < 0x32 {
        let f = fx.mem.i16(t + 0x50);
        let head = (HEADS as i32 + 0x10 * f) as u32;
        bspawn::history(fx, history(f) + 0x450, 0x46, head);
    }
    if fx.mem.i16(t + 0x54) < 0x2D {
        let f = fx.mem.i16(t + 0x50);
        copy3(fx, D + 0x250, history(f));
        fx.mem.set16(FLAG, f);
        spawn(fx, QUADS_A, [u64::from(t), u64::from(D), s2, u64::from(k), 0], 0x01E0_2F0C);
    }
    age(fx, t);
    fx.leave(0x70);
}
