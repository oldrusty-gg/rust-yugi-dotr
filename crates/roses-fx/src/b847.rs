//! BMODULE b847 (card 676), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E062D0, data area 0x1ED0000) [F]: b252's family on a pool of 230 tasks of 0x70 bytes at D+0x2A0
//! (alive +0x58, step +0x44 through `jalr`; spawn 0x1E06DC0 with the
//! colour D+0xD624, the index D+0xD6E4 into +0x50 / +0x52 and D+0xD6D0
//! into +0x5A) and b252's unit flash (0x1E07240: the attacker drawn into
//! a layer, then the flash tasks 0x1E05F90, which the run loop skips and
//! counts). Drawn with arcs (0x1E00B00, `billboard::arc_strips`), quad
//! rings turned about y then x (0x1E019B0, `billboard::ring_quads_yx` on
//! the bytes D+0xD7EE), sparks (0x1E01F70, `quads4::sparks`), the record's
//! own petal sprite 0x1E06F70 (b733's 0x1E043F0 with the cell +0x56), a
//! ribbon 0x1E02370 (b073's 0x1E03E70 on the 0x70 task layout, coloured
//! through 0x108270) between the 40-point histories D+0xCB70 / D+0xCDF0,
//! and trails as the blended tube 0x1E00E60 (`tubetrail::tube_with
//! (SINGLE_BLEND)`) along 20-point histories D+0x6770 + 0x140 k of the
//! relative heads D+0xD070 + 0x10 k. Globals past D+0xC000: the point
//! D+0xD570 (node 0x1D, from update 0x21F flying to the target by the
//! velocities D+0xD5F0.. until 0x28D) and its copy D+0xD5C0 (to 0x213),
//! textures D+0xD600.. (5, 0..4) and D+0x298 / 0x29C (4, 0xE / 0xF), timer
//! D+0xD620 = 799, OT D+0xD62C, layer / flash descriptor D+0xD634 /
//! D+0xD638, arc growth D+0xD648 (3.5 a update to 120 from 0x20D), its
//! offset D+0xD64C = 20, the swing D+0xD650 (20 sin D+0xD6DE), nodes
//! D+0xD664.. (attacker 1 / 0x1D / 9 / 0x12 / 0xA / 0x13, re-picked on
//! 0x48 / 0x4A / 0x5A / 0x6C / 0x7C / 0xA0), target 1 at D+0xD674, the 36
//! cells D+0xD684, petal count D+0xD6CE, trail counter D+0xD6D4, facing
//! D+0xD6E8, counter D+0xD6EC, bytes D+0xD6EE / D+0xD7EE, sines D+0xD8F0.
//! The step 0x1E044D0 is never spawned [F]: not ported, nor its draw case.
//! Returns 1 / 4 / 2 / 6 / 8 on 0x27B / 0x316 / 0x30C / 0x286..0x306 /
//! timer out.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, spawn as bspawn, spline, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xE6;
const ARC_P: u32 = 0x01E0_3D40;
const PETAL: u32 = 0x01E0_3F80;
const RING_A: u32 = 0x01E0_42E0;
const RING_N: u32 = 0x01E0_44D0;
const SPARKS: u32 = 0x01E0_46A0;
const ARC_N: u32 = 0x01E0_4820;
const RIBBON: u32 = 0x01E0_4A90;
const ARC_C: u32 = 0x01E0_4C90;
const TRAIL_A: u32 = 0x01E0_4E60;
const TRAIL_B: u32 = 0x01E0_5460;
const RING_B: u32 = 0x01E0_5B20;
const LIGHT: u32 = 0x01E0_5D00;
const WINDOW: u32 = 0x01E0_5DC0;
const FLASH: u32 = 0x01E0_5F90;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_6688;
const POS: u32 = D + 0x250;
const RIB_A: u32 = D + 0xCB70;
const RIB_B: u32 = D + 0xCDF0;
const HEADS: u32 = D + 0xD070;
const POINT: u32 = D + 0xD570;
const POINT_B: u32 = D + 0xD580;
const POINT_C: u32 = D + 0xD5C0;
const VELOCITY: u32 = D + 0xD5F0;
const TEX: u32 = D + 0xD600;
const TIMER: u32 = D + 0xD620;
const COLOUR: u32 = D + 0xD624;
const OT_AT: u32 = D + 0xD62C;
const LAYER: u32 = D + 0xD634;
const FLASH_DESC: u32 = D + 0xD638;
const GROW: u32 = D + 0xD648;
const OFFSET: u32 = D + 0xD64C;
const SWING: u32 = D + 0xD650;
const NODES: u32 = D + 0xD664;
const TARGET_NODE: u32 = D + 0xD674;
const CELLS: u32 = D + 0xD684;
const PETALS: u32 = D + 0xD6CE;
const ANGLE: u32 = D + 0xD6D0;
const TRAIL_K: u32 = D + 0xD6D4;
const SWING_A: u32 = D + 0xD6DE;
const INDEX: u32 = D + 0xD6E4;
const FACING: u32 = D + 0xD6E8;
const COUNTER: u32 = D + 0xD6EC;
const BYTES: u32 = D + 0xD6EE;
const BYTES_B: u32 = D + 0xD7EE;
const SINES: u32 = D + 0xD8F0;
/// The scratchpad, where the originals project.
const SPR: u32 = 0x7000_0000;

fn hist_of(k: i32) -> u32 {
    (D as i32 + 0x6770 + 0x140 * k) as u32
}

fn head_of(k: i32) -> u32 {
    (HEADS as i32 + 0x10 * k) as u32
}

fn node(fx: &mut Fx, k: u32) -> i32 {
    fx.mem.i16(NODES + 2 * k)
}

/// The entry 0x1E062D0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xE0);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xE0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xDA60);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xD6EA, param);
    fx.mem.set32(TIMER, 0x31F);
    for (k, n) in [1, 0x1D, 9, 0x12, 0xA, 0x13].into_iter().enumerate() {
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
    for k in 0..5u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.fix_residue(v, 0x01E0_64CC);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for (bytes, ra) in [(BYTES, 0x01E0_64FC), (BYTES_B, 0x01E0_654C)] {
        for k in 0..0x100u32 {
            let r = fx.rand_from(D, ra);
            let (_, h) = fx.div(r, 0x1E);
            fx.mem.write_u8(bytes + k, (h - 0xF) as u8);
        }
    }
    for k in 0..0x24u32 {
        let r = fx.rand_from(D, 0x01E0_65A0);
        fx.mem.set16(CELLS + 2 * k, (r >> 4).wrapping_rem(8));
    }
    for (o, v) in [(0xD6D6u32, 0), (0xD6D8, 0x5A), (0xD6DA, 0xB4), (0xD6DC, 0xB4)] {
        fx.mem.set16(D + o, v);
    }
    fx.mem.write_u32(OFFSET, 0x41A0_0000);
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
/// `before` (a3 after a draw at depth mode 1), else `old` (b264's rule).
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
    // first TEXFLUSH (b252 / b264's rules); with no draw the emitter's.
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == ARC_P {
            fx.mem.set16(D + 0xB8, 0x24);
            copy4(fx, sp + 0x70, POINT);
            let z = fx.mem.f(sp + 0x78);
            let o = fx.mem.f(OFFSET);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, o) } else { ee::add(z, o) };
            fx.mem.setf(sp + 0x78, z);
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, 0x21, sp + 0x70);
            a3 = last_xyz2(fx, before, a3);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == PETAL {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x80, 0x14);
            fx.mem.set16(D + 0xB8, 1);
            fx.mem.set16(D + 0xBE, 0);
            petal(fx, t, 0x21, t + 0x10);
            a3 = 0;
        }
        for (g, r, rows, b0) in [(RING_A, 0x90u32, 0xA, 0), (RING_B, 0xC0, 2, 0)] {
            if f == g {
                let w = fx.mem.read_u32(TEX + if g == RING_A { 4 } else { 0x10 });
                fx.mem.write_u32(D + 0x88, w);
                cell(fx, t, sp + r, 0x14);
                fx.mem.set16(D + 0xB8, rows);
                fx.mem.set16(D + 0xB0, b0);
                billboard::ring_quads_yx(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
                a3 = 0;
                fx.mem.set16(D + 0xB8, 0);
                fx.mem.set16(D + 0xB0, 0);
            }
            if g == RING_A {
                debug_assert_ne!(f, RING_N);
            }
        }
        if f == SPARKS {
            cell(fx, t, sp + 0xB0, 0x8C);
            let w = fx.mem.read_u32(D + 0x298);
            fx.mem.write_u32(D + 0x88, w);
            let w = fx.mem.read_u32(TEX + 0xC);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x1F), (0x9A, 0x7F), (0xB8, 0)] {
                fx.mem.set16(D + o, v);
            }
            copy3(fx, D + 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0x18);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == ARC_N {
            fx.mem.set16(D + 0xB8, 0x24);
            let mode = if fx.mem.i16(t + 0x50) == 0 { 0x20 } else { 0x21 };
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, mode, t + 0x10);
            a3 = if mode == 0x21 { last_xyz2(fx, before, a3) } else { 0 };
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == RIBBON {
            let saves = [(0xC0u32, 0x01E0_6A78u64), (0x60, u64::from(slot)), (0x50, u64::from(k)), (0x40, u64::from(flashes)), (0x30, u64::from(D))];
            ribbon(fx, t, RIB_A, RIB_B, &saves);
            a3 = 0;
        }
        if f == ARC_C {
            fx.mem.set16(D + 0xB8, 1);
            billboard::arc_strips(fx, D, t, 0x20, t + 0x10);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == TRAIL_A || f == TRAIL_B {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x3F)] {
                fx.mem.set16(D + o, v);
            }
            if f == TRAIL_B {
                copy3(fx, D + 0x20, POINT_C);
            }
            fx.mem.set16(D + 0xBE, 1);
            let turns: &[bool] = if f == TRAIL_A { &[false, true] } else { &[false] };
            for &turned in turns {
                if f == TRAIL_A {
                    copy3(fx, D + 0x20, t + 0x10);
                }
                let old = fx.mem.read_u32(t + 0x20);
                if turned {
                    fx.mem.write_u32(t + 0x20, old.wrapping_add(0x8000));
                }
                let k = fx.mem.i16(t + 0x50);
                let before = fx.out.prims.len();
                let runs = fx.mem.i16(t + 0xE) < fx.mem.i16(t + 6) - 1;
                tubetrail::tube_with(fx, &tubetrail::SINGLE_BLEND, D, t, hist_of(k), 0x21);
                if !runs {
                    // No segment: a3 = 0x1097D0's trans (the tube's sp + 0x1E0).
                    a3 = fx.sp - 0x350 + 0x1E0;
                }
                // a3 = the first vertex's XYZ2 (+0x20) (b264's rule).
                if let Some(p) = fx.out.prims.get(before..).and_then(|p| p.last()) {
                    let d = p.desc;
                    a3 = fx.mem.read_u32(d + 0x20);
                }
                if turned {
                    fx.mem.write_u32(t + 0x20, old);
                }
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
        0x27B => 1,
        0x316 => 4,
        0x30C => 2,
        0x286..=0x306 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// `flashes`: the entry's s1 (the flash tasks so far) at the `jalr`.
fn run_task(fx: &mut Fx, f: u32, t: u32, flashes: u32) {
    match f {
        ARC_P => arc_p(fx, t, flashes),
        PETAL => petal_step(fx, t, flashes),
        RING_A => ring_a(fx, t),
        SPARKS => sparks(fx, t),
        ARC_N => arc_n(fx, t, flashes),
        RIBBON => ribbon_step(fx, t, flashes),
        ARC_C => arc_c(fx, t),
        TRAIL_A => trail_a(fx, t, flashes),
        TRAIL_B => trail_b(fx, t, flashes),
        RING_B => ring_b(fx, t),
        LIGHT => light(fx, t),
        WINDOW => window(fx, t),
        _ => panic!("b847: unknown task step {f:#x}"),
    }
}

/// The emitter's registers for its callees' saves: s0 (a loop counter, D
/// before the first loop) and the entry's s2 (0) / s3 (param) / s4 (the
/// runner's, 0 [H]).
struct Regs {
    s0: u64,
    param: i32,
}

impl Regs {
    fn spawn(&self, fx: &mut Fx, f: u32, ra: u32) -> u32 {
        spawn(fx, f, [self.s0, u64::from(D), 0, self.param as i64 as u64, 0], ra);
        0
    }
}

/// 0x1F6830(handle, node, out) from the emitter (its frame: s0, s1 = D,
/// ra). Returns a3 as it leaves it (1, b252's [run]).
fn node_at(fx: &mut Fx, r: &Regs, h: i32, slot: i32, out: u32, ra: u32) -> u32 {
    crate::b076::node_position_saving(fx, h, slot, out, r.s0, Some(u64::from(D)), ra);
    1
}

/// 0x1E02D30(D, 0): the update counter, node picks, the point and the
/// spawns. Returns a3 as its last call left it [H: 1 after 0x1F6830, 0
/// after a spawn (b252's), else the entry's 0].
fn emit(fx: &mut Fx, param: i32) -> u32 {
    let mut a3 = 0u32;
    let sp = fx.enter(0x50);
    let mut r = Regs { s0: u64::from(D), param };
    fx.saved(sp, &[(0x20, 0x01E0_6648), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let counter = |fx: &mut Fx| fx.mem.i16(COUNTER);
    for (at, a, b) in [(0x48, 9, 0x12), (0x4A, 0xA, 0x13), (0x5A, 4, 0x17), (0x6C, 6, 0x18)] {
        if counter(fx) == at {
            let h = fx.self_handle();
            let v = fx.request_node(h, a, 1);
            fx.mem.set16(NODES, v);
            let h = fx.self_handle();
            let v = fx.request_node(h, b, 0);
            fx.mem.set16(NODES + 2, v);
            a3 = 0;
        }
    }
    if [0x49, 0x54, 0x63, 0x6D].contains(&counter(fx)) {
        for (slot, ra_n, ra_s) in [(0u32, 0x01E0_2F28u32, 0x01E0_2F54u32), (1, 0x01E0_2F94, 0x01E0_2FC0)] {
            let h = fx.self_handle();
            let s = node(fx, slot);
            a3 = node_at(fx, &r, h, s, POS, ra_n);
            fx.mem.set16(INDEX, 1);
            for k in 0..10u64 {
                r.s0 = k;
                a3 = r.spawn(fx, RING_A, ra_s);
            }
            r.s0 = 10;
        }
    }
    if counter(fx) == 0x7C {
        let h = fx.self_handle();
        let v = fx.request_node(h, 0x10, 1);
        fx.mem.set16(NODES, v);
        a3 = 0;
    }
    let n = counter(fx);
    if (0x7D..0x95).contains(&n) {
        let h = fx.self_handle();
        let s = node(fx, 0);
        node_at(fx, &r, h, s, POS, 0x01E0_3068);
        let y = ee::sub(fx.mem.f(POS + 4), 4.5);
        fx.mem.setf(POS + 4, y);
        fx.mem.set16(INDEX, 0);
        for ra in [0x01E0_309Cu32, 0x01E0_30B0, 0x01E0_30C4, 0x01E0_30D8, 0x01E0_30EC] {
            a3 = r.spawn(fx, ARC_C, ra);
        }
    }
    if counter(fx) == 0xA0 {
        for (k, nd) in [(0u32, 1), (1, 0x1D), (2, 0x1E)] {
            let h = fx.self_handle();
            let v = fx.request_node(h, nd, i32::from(k == 0));
            fx.mem.set16(NODES + 2 * k, v);
        }
        a3 = 0;
    }
    let n = counter(fx);
    if n < 0x21F {
        for (to, ra) in [(POINT, 0x01E0_31A4u32), (POINT_B, 0x01E0_31C8)] {
            let h = fx.self_handle();
            let s = node(fx, 1);
            a3 = node_at(fx, &r, h, s, to, ra);
        }
    } else {
        if n == 0x21F {
            let h = fx.target_handle();
            let s = fx.mem.i16(TARGET_NODE);
            node_at(fx, &r, h, s, sp + 0x30, 0x01E0_31FC);
            let h = fx.target_handle();
            fx.unit_position(h, sp + 0x30);
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(POINT + 8)), 115.0);
            fx.mem.setf(VELOCITY + 8, v);
            let h = fx.target_handle();
            let s = fx.mem.i16(TARGET_NODE);
            a3 = node_at(fx, &r, h, s, sp + 0x30, 0x01E0_325C);
            for c in [4u32, 0] {
                let v = ee::div(ee::sub(fx.mem.f(sp + 0x30 + c), fx.mem.f(POINT + c)), 120.0);
                fx.mem.setf(VELOCITY + c, v);
            }
        }
        if counter(fx) < 0x28D {
            for c in [8u32, 4, 0] {
                let v = ee::add(fx.mem.f(POINT + c), fx.mem.f(VELOCITY + c));
                fx.mem.setf(POINT + c, v);
            }
        }
    }
    let n = counter(fx);
    if (0x17C..0x1F5).contains(&n) && n.wrapping_rem(3) == 1 && fx.mem.i16(PETALS) < 0x24 {
        for (extra, ra) in [(0, 0x01E0_3414u32), (0x24, 0x01E0_34AC)] {
            let p = fx.mem.i16(PETALS);
            let a = if fx.mem.i16(FACING) != 0 { -10 * p } else { 10 * p - 0x8C };
            fx.mem.set16(ANGLE, a);
            let p = fx.mem.i16(PETALS);
            fx.mem.set16(INDEX, p + extra);
            a3 = r.spawn(fx, PETAL, ra);
        }
        let p = fx.mem.i16(PETALS) + 1;
        fx.mem.set16(PETALS, p);
    }
    let n = counter(fx);
    if (0x1F5..0x216).contains(&n) {
        for k in 0..3u64 {
            r.s0 = k;
            trail_spawn(fx, &r, TRAIL_B, 0x01E0_3520);
            a3 = 0;
        }
        r.s0 = 3;
    }
    if counter(fx) == 0x208 {
        fx.mem.set16(INDEX, 0);
        a3 = r.spawn(fx, ARC_P, 0x01E0_35C0);
    }
    let n = counter(fx);
    if (0x208..0x30D).contains(&n) && n.wrapping_rem(3) == 1 {
        fx.mem.set16(INDEX, 1);
        a3 = r.spawn(fx, ARC_P, 0x01E0_3624);
    }
    let n = counter(fx);
    if (0x285..0x303).contains(&n) && n.wrapping_rem(3) == 1 {
        point_pos(fx);
        for ra in [0x01E0_36ECu32, 0x01E0_3700] {
            a3 = r.spawn(fx, SPARKS, ra);
        }
        fx.mem.set16(INDEX, 1);
        for ra in [0x01E0_3724u32, 0x01E0_3738, 0x01E0_374C] {
            a3 = r.spawn(fx, ARC_C, ra);
        }
        for k in 0..7u64 {
            r.s0 = k;
            trail_spawn(fx, &r, TRAIL_A, 0x01E0_3780);
            a3 = 0;
        }
        r.s0 = 7;
    }
    let n = counter(fx);
    if (0x285..0x303).contains(&n) && n.wrapping_rem(6) == 1 {
        a3 = r.spawn(fx, WINDOW, 0x01E0_3840);
    }
    let n = counter(fx);
    if (0x221..0x303).contains(&n) && n.wrapping_rem(3) == 1 {
        point_pos(fx);
        for ra in [0x01E0_3908u32, 0x01E0_391C] {
            a3 = r.spawn(fx, RING_B, ra);
        }
    }
    if counter(fx) >= 0x20D && crate::exe::ee_lt(fx.mem.f(GROW), 120.0) {
        let v = ee::add(fx.mem.f(GROW), 3.5);
        fx.mem.setf(GROW, v);
    }
    if counter(fx) == 1 {
        fx.mem.set16(INDEX, 0);
    }
    if counter(fx) == 1 {
        a3 = r.spawn(fx, LIGHT, 0x01E0_39C4);
    }
    let n = counter(fx);
    if (0x17C..0x1F5).contains(&n) {
        let h = fx.self_handle();
        let s = node(fx, 2);
        node_at(fx, &r, h, s, POS, 0x01E0_3A08);
        fx.mem.set16(INDEX, 0);
        a3 = r.spawn(fx, ARC_N, 0x01E0_3A28);
    }
    let n = counter(fx);
    if (0x98..0x9F).contains(&n) {
        fx.mem.set16(INDEX, 1);
        a3 = r.spawn(fx, FLASH, 0x01E0_3A70);
    }
    if counter(fx) == 0xFA {
        for i in 4..0x10 {
            fx.mem.set16(INDEX, i);
            a3 = r.spawn(fx, ARC_N, 0x01E0_3AAC + 0x24 * (i as u32 - 4));
        }
    }
    if counter(fx) == 0x170 {
        a3 = r.spawn(fx, RIBBON, 0x01E0_3C64);
    }
    if counter(fx) < 0x213 {
        copy4(fx, POINT_C, POINT);
    }
    let n = counter(fx);
    if (0x215..0x28B).contains(&n) {
        let a = fx.mem.i16(SWING_A) + 0x12;
        fx.mem.set16(SWING_A, a);
    }
    let a = fx.mem.i16(SWING_A);
    let s = l2::sin_deg(fx, SINES, a, 0);
    fx.mem.setf(SWING, ee::mul(20.0, s));
    fx.leave(0x50);
    a3
}

/// D+0x250 = the point, z pushed by D+0xD64C away from the facing.
fn point_pos(fx: &mut Fx) {
    copy4(fx, POS, POINT);
    let z = fx.mem.f(POS + 8);
    let o = fx.mem.f(OFFSET);
    let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, o) } else { ee::add(z, o) };
    fx.mem.setf(POS + 8, z);
}

/// A trail spawn: the index D+0xD6D4, which counts up and wraps at 0x50.
fn trail_spawn(fx: &mut Fx, r: &Regs, f: u32, ra: u32) {
    let k = fx.mem.i16(TRAIL_K);
    fx.mem.set16(INDEX, k);
    r.spawn(fx, f, ra);
    let k = fx.mem.i16(TRAIL_K) + 1;
    fx.mem.set16(TRAIL_K, k);
    if fx.mem.i16(TRAIL_K) >= 0x50 {
        fx.mem.set16(TRAIL_K, 0);
    }
}

/// 0x1E06DC0(D, step, 0): b252's spawn on this record's globals; `regs`
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
            l2::to_i16x4(fx, t, POS, 0);
            // 0x107390's `sq ra` upper half (0) (b232's).
            fx.mem.write_u64(sp - 8, 0);
            copy3(fx, t + 0x10, POS);
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

/// (rand >> 4) % m.
fn rand4_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v >> 4, m).1
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

/// A step's frame with the entry's s0 = D (and s1 = the flash count when
/// `n` is 2) saved from `base`, ra at `ra_at`.
fn enter_step(fx: &mut Fx, frame: u32, n: u32, flashes: u32, base: u32, ra_at: u32) -> u32 {
    let sp = fx.enter(frame);
    let vals = [u64::from(D), u64::from(flashes)];
    for k in 0..n {
        fx.saved(sp + base + 0x10 * k, &[(0, vals[k as usize])]);
    }
    fx.saved(sp, &[(ra_at, TASK_RA)]);
    sp
}

/// 0x1E03D40: an arc at the point D+0xD570 (pushed z), kind +0x50: 0 a
/// grey / violet arc of radius 70 + D+0xD648 + the swing, in by 10 below
/// 0xA0 for 30 updates, out by 0xC from update 0x303; 1 radius rand % 40
/// + 20 + D+0xD648 + the swing (set once), in by 0x50 for 5 updates, then
/// out by 0xC; camera-facing, turned +0x4000.
fn arc_p(fx: &mut Fx, t: u32, flashes: u32) {
    enter_step(fx, 0x30, 2, flashes, 0, 0x20);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0x70, 0x70, 0x70);
        l2::rgb(fx, t + 0x64, 0x80, 0x40, 0xFF);
        let k = fx.mem.i16(t + 0x50);
        if k == 0 {
            fx.mem.set16(t + 6, 0x1E);
            fx.mem.write_u32(t + 0x4C, 0x428C_0000);
        } else if k == 1 {
            fx.mem.set16(t + 6, 5);
            let r = rand_mod(fx, t, 0x01E0_3DC8, 0x28);
            let f2 = ee::add(20.0, ee::from_i32(r));
            let f1 = ee::add(fx.mem.f(GROW), f2);
            let sw = fx.mem.f(SWING);
            fx.mem.setf(t + 0x4C, ee::add(sw, f1));
        }
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let k = |fx: &mut Fx| fx.mem.i16(t + 0x50);
    if young(fx, t) {
        if k(fx) == 0 {
            fade_in(fx, t, 0x67, (0xA0, 0xA));
        }
        if k(fx) == 1 {
            fade_in(fx, t, 0x67, (0xA0, 0x50));
        }
    } else {
        if k(fx) == 0 && fx.mem.i16(COUNTER) >= 0x303 {
            fade_out(fx, t, 0x67, (0xE, 0xC));
        }
        if k(fx) == 1 {
            fade_out(fx, t, 0x67, (0xE, 0xC));
        }
    }
    if k(fx) == 0 {
        let f1 = ee::add(70.0, fx.mem.f(GROW));
        let sw = fx.mem.f(SWING);
        fx.mem.setf(t + 0x4C, ee::add(f1, sw));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_3F44);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03F80: petal +0x50 (k) at the point: size 1.5, cell rand % 8, the
/// ring +0x52 = 0 / 2 / 4 by k (36 a ring), spin +8 (rand >> 4) % 2 by +0xE
/// = (rand >> 4) % 6 + 2 from update 0x1F5, start +0x5C; in by 0xC below
/// 0xC0 for 30 updates, out by 0xB from update 0x2EF; +6 counting 0..5 a
/// tenth update; ring 0 radius 25 size 3.5, ring 2 radius 14 size 2.5;
/// from 0x202 rings 0 / 1 turn +-0x100 about z; it rides the point to
/// 0x202.
fn petal_step(fx: &mut Fx, t: u32, flashes: u32) {
    enter_step(fx, 0x30, 2, flashes, 0, 0x20);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x3FC0_0000);
        let v = fx.rand_from(t, 0x01E0_3FB0);
        fx.mem.set16(t + 0x56, v.wrapping_rem(8));
        fx.mem.set16(t + 0x58, 2);
        let k = fx.mem.i16(t + 0x50);
        if k < 0x24 {
            fx.mem.set16(t + 0x52, 0);
        } else if k < 0x48 {
            fx.mem.set16(t + 0x52, 2);
        } else if k < 0x6C {
            fx.mem.set16(t + 0x52, 4);
        }
        let v = fx.rand_from(t, 0x01E0_4018);
        fx.mem.set16(t + 8, (v >> 4).wrapping_rem(2));
        let v = rand4_mod(fx, t, 0x01E0_4038, 6);
        fx.mem.set16(t + 0xE, v + 2);
        let v = rand4_mod(fx, t, 0x01E0_405C, 0x168);
        fx.mem.set16(t + 0x5C, v);
        fx.mem.set16(t + 6, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    }
    if fx.mem.i16(t + 0x54) < 0x1E {
        fade_in(fx, t, 0x63, (0xC0, 0xC));
        fade_in(fx, t, 0x67, (0xC0, 0xC));
    } else if fx.mem.i16(COUNTER) >= 0x2EF {
        fade_out(fx, t, 0x63, (0xD, 0xB));
        let v = fx.mem.read_u8(t + 0x67);
        if v >= 0xD {
            fx.mem.write_u8(t + 0x67, v - 0xB);
        }
    }
    let a = fx.mem.i16(t + 0x54);
    if a >= 0x1E && a.wrapping_rem(10) == 1 {
        let v = fx.mem.i16(t + 6) + 1;
        fx.mem.set16(t + 6, v);
        if fx.mem.i16(t + 6) >= 6 {
            fx.mem.set16(t + 6, 0);
        }
    }
    if fx.mem.i16(COUNTER) != 0 {
        match fx.mem.i16(t + 0x52) {
            0 => {
                fx.mem.write_u32(t + 0x4C, 0x41C8_0000);
                fx.mem.write_u32(t + 0x48, 0x4060_0000);
            }
            2 => {
                fx.mem.write_u32(t + 0x4C, 0x4160_0000);
                fx.mem.write_u32(t + 0x48, 0x4020_0000);
            }
            _ => {}
        }
    }
    if fx.mem.i16(COUNTER) >= 0x202 {
        let k = fx.mem.i16(t + 0x50);
        if k < 0x24 {
            let v = fx.mem.i32(t + 0x28).wrapping_add(0x100);
            fx.mem.set32(t + 0x28, v);
        } else if k < 0x48 {
            let v = fx.mem.i32(t + 0x28).wrapping_sub(0x100);
            fx.mem.set32(t + 0x28, v);
        }
    }
    if fx.mem.i16(COUNTER) < 0x203 {
        copy4(fx, t + 0x10, POINT);
    }
    if fx.mem.i16(COUNTER) >= 0x1F5 {
        let (s, e) = (fx.mem.i16(t + 0x5C), fx.mem.i16(t + 0xE));
        let v = if fx.mem.i16(t + 8) == 1 { s + e } else { s - e };
        fx.mem.set16(t + 0x5C, v);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E042E0: a grey quad ring at a node (size rand % 10 + 5, cell rand %
/// 4, radius rand % 20 + 10 growing by rand % 25 + 20, the turn +0x5A (+0x52
/// 0) or +0x5C (1) (rand >> 4) % 360, row rand % 8 + 1), 0x80 for rand % 10
/// + 13 updates, then out by 0xB; +0x52 counts up.
fn ring_a(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0, 0, 0x10);
    if first(fx, t) {
        let v = rand_mod(fx, t, 0x01E0_4304, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 5));
        let v = fx.rand_from(t, 0x01E0_4330);
        fx.mem.set16(t + 0x56, v.wrapping_rem(4));
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rand_mod(fx, t, 0x01E0_4364, 0x14);
        fx.mem.set16(t + 0x50, v + 0xA);
        if fx.mem.i16(t + 0x52) == 0 {
            let v = rand4_mod(fx, t, 0x01E0_4394, 0x168);
            fx.mem.set16(t + 0x5A, v);
        }
        if fx.mem.i16(t + 0x52) == 1 {
            let v = rand4_mod(fx, t, 0x01E0_43C8, 0x168);
            fx.mem.set16(t + 0x5C, v);
        }
        let v = rand_mod(fx, t, 0x01E0_43EC, 0x19);
        fx.mem.set16(t + 0xE, v + 0x14);
        let v = fx.rand_from(t, 0x01E0_440C);
        fx.mem.set16(t + 8, v.wrapping_rem(8) + 1);
        let v = rand_mod(fx, t, 0x01E0_442C, 0xA);
        fx.mem.set16(t + 6, v + 0xD);
        fx.mem.set16(t + 0x58, 2);
    }
    if young(fx, t) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, (0xD, 0xB));
    }
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, v);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E046A0: sparks on a growing circle (inner radius 100 growing by 55 a
/// update, outer 300 inside it), grey over dark grey, turned randomly
/// (+0x24 turning 0x100 a update); 0x80 for 13 updates, then out by 0xE.
fn sparks(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0, 0, 0x10);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        fx.mem.setf(t + 0x4C, ee::sub(100.0, 58.0));
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.write_u32(t + 0x34, 0x4409_8000);
        let v = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, ra) in [(0x20u32, 0x01E0_4740u32), (0x24, 0x01E0_475C), (0x28, 0x01E0_4778)] {
            let v = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 0xD {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, (0x10, 0xE));
    }
    let inner = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, inner);
    fx.mem.setf(t + 0x4C, ee::sub(inner, 300.0));
    let v = fx.mem.i32(t + 0x24).wrapping_add(0x100);
    fx.mem.set32(t + 0x24, v);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E04820 (frame 0x60): an arc, kind +0x50: 0 at node 9 (radius 70,
/// grey over yellow, in by 0x35 below 0xA0 for 5 updates), k > 0 the
/// (k - 1)th fifteenth of the way from node 0x12 to node 9 each update
/// (radius 100, dark over olive, in by 0xD for 500 updates); out by 0xC;
/// camera-facing, turned +0x4000.
fn arc_n(fx: &mut Fx, t: u32, flashes: u32) {
    let sp = enter_step(fx, 0x60, 2, flashes, 0x10, 0x30);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0x70, 0x70, 0x70);
            l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0x40);
            fx.mem.set16(t + 6, 5);
            fx.mem.write_u32(t + 0x4C, 0x428C_0000);
        }
        if fx.mem.i16(t + 0x50) > 0 {
            l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x20);
            l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x20);
            fx.mem.set16(t + 6, 0x1F4);
            fx.mem.write_u32(t + 0x4C, 0x42C8_0000);
        }
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if young(fx, t) {
        if fx.mem.i16(t + 0x50) == 0 {
            fade_in(fx, t, 0x67, (0xA0, 0x35));
        }
        if fx.mem.i16(t + 0x50) > 0 {
            fade_in(fx, t, 0x67, (0xA0, 0xD));
        }
    } else {
        fade_out(fx, t, 0x67, (0xE, 0xC));
    }
    let mut s1 = u64::from(D);
    if fx.mem.i16(t + 0x50) > 0 {
        let f20 = ee::from_i32(fx.mem.i16(t + 0x50) - 1);
        let h = fx.self_handle();
        let s = node(fx, 1);
        crate::b076::node_position_saving(fx, h, s, t + 0x10, u64::from(t), Some(s1), 0x01E0_49A0);
        let h = fx.self_handle();
        let s = node(fx, 1);
        crate::b076::node_position_saving(fx, h, s, sp + 0x40, u64::from(t), Some(s1), 0x01E0_49C0);
        s1 = u64::from(sp + 0x50);
        let h = fx.self_handle();
        let s = node(fx, 2);
        crate::b076::node_position_saving(fx, h, s, sp + 0x50, u64::from(t), Some(s1), 0x01E0_49E4);
        for c in [0u32, 4, 8] {
            let d = ee::div(ee::sub(fx.mem.f(sp + 0x50 + c), fx.mem.f(sp + 0x40 + c)), 15.0);
            let v = ee::add(fx.mem.f(t + 0x10 + c), ee::mul(f20, d));
            fx.mem.setf(t + 0x10 + c, v);
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), s1, 0x01E0_4A58);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x60);
}

/// 0x1E04A90 (frame 0x40): the ribbon's yellow head: the two 40-point
/// histories D+0xCB70 / D+0xCDF0 filled with nodes 0x1D / 9 once and fed
/// them every update, 38 points drawn; 0xC0 for 200 updates, then out by
/// 9.
fn ribbon_step(fx: &mut Fx, t: u32, flashes: u32) {
    let sp = enter_step(fx, 0x40, 2, flashes, 0, 0x20);
    l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x10);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        for (slot, h0, ra) in [(1u32, RIB_A, 0x01E0_4AF0u32), (2, RIB_B, 0x01E0_4B5C)] {
            let h = fx.self_handle();
            let s = node(fx, slot);
            crate::b076::node_position_saving(fx, h, s, sp + 0x30, u64::from(t), Some(u64::from(D)), ra);
            for k in 0..0x28u32 {
                copy3(fx, h0 + 0x10 * k, sp + 0x30);
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0xC8 {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade_out(fx, t, 0x63, (0xB, 9));
    }
    for (slot, h0, ra) in [(1u32, RIB_A, 0x01E0_4C10u32), (2, RIB_B, 0x01E0_4C48)] {
        let h = fx.self_handle();
        let s = node(fx, slot);
        crate::b076::node_position_saving(fx, h, s, sp + 0x30, u64::from(t), Some(u64::from(D)), ra);
        bspawn::history(fx, h0 + 0x270, 0x28, sp + 0x30);
    }
    fx.mem.set16(t + 6, 0x26);
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E04C90: an arc of inner radius 10, kind +0x50: 0 radius 200 (grey
/// over green, turned randomly about x / y), 1 radius 600 (grey over
/// violet, turned randomly); in by 0x35 below 0xA0 for 6 updates, then out
/// by 0xC.
fn arc_c(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0, 0, 0x10);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0x70, 0x70, 0x70);
            l2::rgb(fx, t + 0x64, 0x30, 0xC0, 0x30);
            fx.mem.set16(t + 6, 6);
            fx.mem.write_u32(t + 0x4C, 0x4348_0000);
            let v = rand_mod(fx, t, 0x01E0_4CFC, 0xFFFF);
            fx.mem.set32(t + 0x24, v);
            let v = fx.rand_from(t, 0x01E0_4D18);
            fx.mem.set32(t + 0x20, v.wrapping_rem(0x4000) - 0x2000);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
            l2::rgb(fx, t + 0x64, 0xC0, 0x80, 0xFF);
            fx.mem.set16(t + 6, 6);
            fx.mem.write_u32(t + 0x4C, 0x4416_0000);
            for (o, ra) in [(0x24u32, 0x01E0_4D80u32), (0x20, 0x01E0_4D9C), (0x28, 0x01E0_4DB8)] {
                let v = rand_mod(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
        }
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x67, (0xA0, 0x35));
    } else {
        fade_out(fx, t, 0x67, (0xE, 0xC));
    }
    age(fx, t);
    fx.leave(0x20);
}

/// The trails' shared update: +6 growing to 15 for 7 updates then
/// shrinking, 0xC0 for +8 updates then out by 0x1D, colour by age parity,
/// jitter rand % 80 - 40 (`ras`).
fn trail_common(fx: &mut Fx, t: u32, even: (i32, i32, i32), odd: (i32, i32, i32), ras: [u32; 3]) {
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
    let c = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 0 { even } else { odd };
    l2::rgb(fx, t + 0x60, c.0, c.1, c.2);
    for (o, ra) in [0x30u32, 0x34, 0x38].into_iter().zip(ras) {
        let r = rand_mod(fx, t, ra, 0x50);
        fx.mem.setf(t + o, ee::from_i32(r - 0x28));
    }
}

/// head[c] += +0x4C x sin_deg(+a, cos).
fn head_step(fx: &mut Fx, t: u32, moves: &[(u32, u32, i32)]) {
    for &(c, a, cos) in moves {
        let a = fx.mem.i16(t + a);
        let s = l2::sin_deg(fx, SINES, a, cos);
        let m = ee::mul(fx.mem.f(t + 0x4C), s);
        let h = head_of(fx.mem.i16(t + 0x50));
        let v = ee::add(fx.mem.f(h + c), m);
        fx.mem.setf(h + c, v);
    }
}

/// head[c] += the jitter c / `div` for each `cs`.
fn head_jitter(fx: &mut Fx, t: u32, cs: &[u32], div: f32) {
    for &c in cs {
        let j = ee::div(fx.mem.f(t + 0x30 + c), div);
        let h = head_of(fx.mem.i16(t + 0x50));
        let v = ee::add(fx.mem.f(h + c), j);
        fx.mem.setf(h + c, v);
    }
}

/// The head zeroed, jittered by rand % 60 - 30 over `div`.
fn head_init(fx: &mut Fx, t: u32, ras: [u32; 3], div: f32) {
    for o in [0u32, 4, 8] {
        let h = head_of(fx.mem.i16(t + 0x50));
        fx.mem.write_u32(h + o, 0);
    }
    for (o, ra) in [0x30u32, 0x34, 0x38].into_iter().zip(ras) {
        let r = rand_mod(fx, t, ra, 0x3C);
        fx.mem.setf(t + o, ee::from_i32(r - 0x1E));
    }
    head_jitter(fx, t, &[0, 4, 8], div);
}

fn hist_fill(fx: &mut Fx, t: u32) {
    let k = fx.mem.i16(t + 0x50);
    let (tr, h) = (hist_of(k), head_of(k));
    for j in 0..0x14u32 {
        copy3(fx, tr + 0x10 * j, h);
    }
}

fn hist_push(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < 7 {
        let k = fx.mem.i16(t + 0x50);
        bspawn::history(fx, hist_of(k) + 0x130, 0x14, head_of(k));
    }
}

/// +o = (rand % m + add) / 10.
fn tenths(fx: &mut Fx, t: u32, o: u32, ra: u32, m: i32, add: i32) {
    let r = rand_mod(fx, t, ra, m);
    fx.mem.setf(t + o, ee::from_i32(r + add));
    let v = ee::div(fx.mem.f(t + o), 10.0);
    fx.mem.setf(t + o, v);
}

/// 0x1E04E60: trail +0x50 (k): a head D+0xD070 + 0x10 k from the jitter
/// / 2 heading +0x5A (rand % 360) / +0x5C (90) by +0x4C (rand % 125 + 3
/// tenths) with jitter tenths, violet; drawn at its spawn point and again
/// turned half a turn about x.
fn trail_a(fx: &mut Fx, t: u32, flashes: u32) {
    enter_step(fx, 0x30, 2, flashes, 0, 0x20);
    if first(fx, t) {
        fx.mem.set16(t + 6, 1);
        fx.mem.write_u8(t + 0x63, 0);
        head_init(fx, t, [0x01E0_4ED8, 0x01E0_4F04, 0x01E0_4F30], 2.0);
        hist_fill(fx, t);
        tenths(fx, t, 0x48, 0x01E0_505C, 0x96, 0x64);
        tenths(fx, t, 0x4C, 0x01E0_509C, 0x7D, 3);
        fx.mem.set16(t + 0x5C, 0x5A);
        let v = rand_mod(fx, t, 0x01E0_50DC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let r = rand_mod(fx, t, 0x01E0_5100, 0xF);
        fx.mem.set16(t + 8, r + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    trail_common(fx, t, (0xC0, 0x80, 0xFF), (0x80, 0x60, 0xC0), [0x01E0_51FC, 0x01E0_5228, 0x01E0_5254]);
    if fx.mem.i16(t + 0x54) != 0 {
        head_step(fx, t, &[(8, 0x5A, 0), (0, 0x5A, 1), (4, 0x5C, 0)]);
        head_jitter(fx, t, &[0, 4, 8], 10.0);
    }
    hist_push(fx, t);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E05460: trail +0x50 (k; +0x52 = k % 6): a head from the jitter
/// tenths pushed 25 along a (rand >> 4) % 360 heading, then heading that +
/// rand % 30 + 165 / +-45 (by the facing) by +0x4C (rand % 65 + 3 tenths)
/// with x / y jitter tenths, white / grey; drawn at the point D+0xD5C0.
fn trail_b(fx: &mut Fx, t: u32, flashes: u32) {
    enter_step(fx, 0x30, 2, flashes, 0, 0x20);
    if first(fx, t) {
        fx.mem.set16(t + 6, 1);
        fx.mem.write_u8(t + 0x63, 0);
        let k = fx.mem.i16(t + 0x50);
        let (_, m) = fx.div(k, 6);
        fx.mem.set16(t + 0x52, m);
        head_init(fx, t, [0x01E0_54F4, 0x01E0_5520, 0x01E0_554C], 10.0);
        let v = rand4_mod(fx, t, 0x01E0_5608, 0x168);
        fx.mem.set16(t + 0x5A, v);
        for (c, cos) in [(0u32, 1), (4, 0)] {
            let a = fx.mem.i16(t + 0x5A);
            let s = l2::sin_deg(fx, SINES, a, cos);
            let m = ee::mul(25.0, s);
            let h = head_of(fx.mem.i16(t + 0x50));
            let v = ee::add(fx.mem.f(h + c), m);
            fx.mem.setf(h + c, v);
        }
        hist_fill(fx, t);
        tenths(fx, t, 0x48, 0x01E0_5724, 0x50, 0x28);
        tenths(fx, t, 0x4C, 0x01E0_5764, 0x41, 3);
        let r = rand_mod(fx, t, 0x01E0_57A4, 0x1E);
        let a = fx.mem.i16(t + 0x5A) + ((r + 0xA5) as i16 as i32);
        fx.mem.set16(t + 0x5A, a);
        let b = if fx.mem.i16(FACING) != 0 { -0x2D } else { 0x2D };
        fx.mem.set16(t + 0x5C, b);
        let r = rand_mod(fx, t, 0x01E0_57F8, 0xF);
        fx.mem.set16(t + 8, r + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    trail_common(fx, t, (0xFF, 0xFF, 0xFF), (0x80, 0x80, 0x80), [0x01E0_58F4, 0x01E0_5920, 0x01E0_594C]);
    if fx.mem.i16(t + 0x54) != 0 {
        head_step(fx, t, &[(8, 0x5C, 0), (0, 0x5A, 1), (4, 0x5A, 0)]);
        head_jitter(fx, t, &[0, 4], 10.0);
    }
    hist_push(fx, t);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E05B20: a grey quad ring at the point (size 40, cell 2, radius rand
/// % 100 + 30, turns (rand >> 4) % 360 about y / x, row rand % 8 + 1), in
/// by 0x40 below 0x80 for rand % 10 + 10 updates, then out by 0xB; 0x80 /
/// 0x60 grey by age parity; +0x52 counts up.
fn ring_b(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1, 0, 0, 0x10);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x4220_0000);
        fx.mem.set16(t + 0x56, 2);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rand_mod(fx, t, 0x01E0_5B68, 0x64);
        fx.mem.set16(t + 0x50, v + 0x1E);
        for (o, ra) in [(0x5Au32, 0x01E0_5B88u32), (0x5C, 0x01E0_5BA8)] {
            let v = rand4_mod(fx, t, ra, 0x168);
            fx.mem.set16(t + o, v);
        }
        let v = rand_mod(fx, t, 0x01E0_5BC8, 0xF);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = fx.rand_from(t, 0x01E0_5BE8);
        fx.mem.set16(t + 8, v.wrapping_rem(8) + 1);
        let v = rand_mod(fx, t, 0x01E0_5C08, 0xA);
        fx.mem.set16(t + 6, v + 0xA);
        fx.mem.set16(t + 0x58, 2);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, (0x80, 0x40));
    } else {
        fade_out(fx, t, 0x63, (0xD, 0xB));
    }
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0x80 } else { 0x60 };
    l2::rgb(fx, t + 0x60, g, g, g);
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E05D00 (frame 0x10): b732's stage light, floor 0x20.
fn light(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x10);
    fx.saved(sp, &[(0, TASK_RA)]);
    if first(fx, t) {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let v = fx.mem.i16(t + 0x50);
    if fx.mem.i32(TIMER) >= 0x15 {
        fx.mem.set16(t + 0x50, if v < 0x21 { 0x20 } else { v - 0xA });
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
    let l = fx.mem.read_u8(t + 0x50);
    fx.set_stage_light(i32::from(l));
    fx.leave(0x10);
}

/// 0x1E05DC0 (frame 0x80): b732's window sprite (colour (0x40, 0x20,
/// 0x80), ALPHA 0x48, key 0), in by 0x28 below 0x50 for 5 updates, then
/// out by 0xE.
fn window(fx: &mut Fx, t: u32) {
    let sp = enter_step(fx, 0x80, 1, 0, 0, 0x10);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for (k, c) in [0x40u8, 0x20, 0x80].into_iter().enumerate() {
        fx.mem.write_u8(sp + 0x38 + k as u32, c);
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
        fx.mem.setf(sp + o, ee::div(s, 512.0));
    }
    for o in [0x50u32, 0x54] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(ee::sub(512.0, s), 512.0));
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(t + 0x54) < 5 {
        fade_in(fx, t, 0x63, (0x50, 0x28));
    } else {
        fade_out(fx, t, 0x63, (0x10, 0xE));
    }
    age(fx, t);
    fx.leave(0x80);
}

/// 0x1E06F70(D, t, mode, pos): b733's petal (0x1E043F0) with the cell
/// +0x56 straight into `uv::grid_rect`.
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
    let c = fx.mem.i16(t + 0x56);
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

/// The point of the Lagrange curve (44769f26f5eb's basis 0) through the
/// four points `p` at `t` into `out` (xyz; w left as it was), summed as the
/// original's accumulator chain sums it (b073's).
fn curve(fx: &mut Fx, out: u32, p: [u32; 4], t: f32) {
    use ee::{add, mul};
    let w = spline::weights(t, 0).unwrap_or([0.0; 4]);
    for c in 0..3u32 {
        let v: [f32; 4] = std::array::from_fn(|i| fx.mem.f(p[i] + 4 * c));
        let acc = add(mul(v[2], w[2]), add(mul(v[0], w[0]), mul(v[1], w[1])));
        fx.mem.setf(out + 4 * c, add(acc, mul(v[3], w[3])));
    }
}

/// 0x1E02370 (D, task, a, b) (frame 0x370): b073's ribbon 0x1E03E70 on
/// the 0x70 task layout: colour +0x60 at sp + 0x36C, each strip's near
/// pair (0x108270 which 1) as it stands and its far pair (which 2) with
/// the alpha down by +0x63 / 2n (floor 0), set just before the strip is
/// projected [F word-compare with b073's].
fn ribbon(fx: &mut Fx, t: u32, mut a: u32, mut b: u32, saves: &[(u32, u64)]) {
    let sp = fx.enter(0x370);
    fx.saved(sp, saves);
    let d = sp + 0x2D8;
    let colour = sp + 0x36C;
    let ot = OT;
    let n = fx.mem.i16(t + 6);
    fx.mem.write_u32(sp + 0x108, ot);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(colour + k, v);
    }
    // The first segment's point arrays: (a0, a0, a1, a2) / (b0, b0, b1, b2).
    let (pa, pb, a0, b0) = (sp + 0x1B0, sp + 0x1F0, sp + 0x2B0, sp + 0x2C0);
    copy4(fx, a0, a);
    copy4(fx, pa, a0);
    copy4(fx, pa + 0x10, a0);
    copy4(fx, pa + 0x20, a + 0x10);
    copy4(fx, pa + 0x30, a + 0x20);
    copy4(fx, b0, b);
    copy4(fx, pb, b0);
    copy4(fx, pb + 0x10, b0);
    copy4(fx, pb + 0x20, b + 0x10);
    copy4(fx, pb + 0x30, b + 0x20);
    // The point arrays' words the loop keeps in registers / locals.
    let kept: [(u32, u32); 11] = [(0xDC, 0x1C0), (0xE0, 0x1D0), (0xE4, 0x1E0), (0xE8, 0x1C4), (0xEC, 0x1B4), (0xF0, 0x1D4), (0xF4, 0x1E4), (0xF8, 0x1C8), (0xFC, 0x1B8), (0x100, 0x1D8), (0x104, 0x1E8)];
    fx.mem.set32(sp + 0x110, n - 1);
    for (to, from) in kept {
        let w = fx.mem.read_u32(sp + from);
        fx.mem.write_u32(sp + to, w);
    }
    // Locals: (a t, b t, a t', b t') per path and their copies.
    let (near, first) = ([0x230u32, 0x240, 0x250, 0x260], [0x270u32, 0x280, 0x290, 0x2A0]);
    let copies = [0x170u32, 0x190, 0x180, 0x1A0];
    let quad = sp + 0x130;
    let mean = sp + 0x120;
    let mut seg = 0i32;
    while seg < n - 1 {
        let mut tt = 0.0f32;
        for _ in 0..4 {
            let nt = ee::add(tt, 0.25);
            let (pts_a, pts_b, l) = if seg == 0 {
                let pa4 = [pa, pa + 0x10, pa + 0x20, pa + 0x30];
                let pb4 = [pb, pb + 0x10, pb + 0x20, pb + 0x30];
                (pa4, pb4, first)
            } else {
                let pa4 = [a - 0x10, a, a + 0x10, a + 0x20];
                let pb4 = [b - 0x10, b, b + 0x10, b + 0x20];
                (pa4, pb4, near)
            };
            curve(fx, sp + l[0], pts_a, tt);
            copy4(fx, sp + copies[0], sp + l[0]);
            curve(fx, sp + l[1], pts_b, tt);
            copy4(fx, sp + copies[1], sp + l[1]);
            curve(fx, sp + l[2], pts_a, nt);
            copy4(fx, sp + copies[2], sp + l[2]);
            curve(fx, sp + l[3], pts_b, nt);
            copy4(fx, sp + copies[3], sp + l[3]);
            l2::copy3(fx, quad, sp + copies[0]);
            l2::copy3(fx, quad + 0x10, sp + copies[2]);
            l2::copy3(fx, quad + 0x20, sp + copies[1]);
            l2::copy3(fx, quad + 0x30, sp + copies[3]);
            l2::colour_strip4(fx, d, colour, 1);
            let al = i32::from(fx.mem.read_u8(t + 0x63));
            let q = fx.div(al, 2 * n).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(colour + 3)) - q;
            fx.mem.write_u8(colour + 3, if v > 0 { v as u8 } else { 0 });
            l2::colour_strip4(fx, d, colour, 2);
            if l2::project(fx, quad, SPR, 0, d, mean) {
                fx.submit(d, ot, 0);
            }
            tt = nt;
        }
        a += 0x10;
        b += 0x10;
        seg += 1;
    }
    fx.leave(0x370);
}

/// 0x1E05F90(D, t): the flash sprite (the attacker over the window, UVs
/// inset by 3 x +8 tenths over 512 both ways) into the descriptor at
/// D+0xD638, no ordering table. Kind +0x50 0: white, in by 0xA below 0x80
/// for 30 updates; 1: grey 0x40 at 0xC0, from update 0xA2 and age 7 its +8
/// growing by 25 and out by 0xD. `k` the unit flash's slot index.
fn flash(fx: &mut Fx, t: u32, k: u32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_7344), (0x20, u64::from(k)), (0x10, u64::from(D))]);
    // swc1 f21 / f20: the runner's (0) [H].
    fx.mem.write_u32(sp + 4, 0);
    fx.mem.write_u32(sp, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    if first(fx, t) {
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u8(t + 0x63, 0);
            l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u8(t + 0x63, 0xC0);
            l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
        }
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
    let w3 = ee::mul(3.0, w);
    fx.mem.setf(d + 0x18, ee::div(w3, 512.0));
    fx.mem.setf(d + 0x1C, ee::div(w3, 512.0));
    fx.mem.setf(d + 0x28, ee::div(ee::sub(512.0, w3), 512.0));
    fx.mem.setf(d + 0x2C, ee::div(ee::sub(512.0, w3), 512.0));
    fx.submit(d, 0, 0);
    if fx.mem.i16(t + 0x50) == 0 && fx.mem.i16(t + 0x54) < 0x1E {
        fade_in(fx, t, 0x63, (0x80, 0xA));
    }
    if fx.mem.i16(t + 0x50) == 1 && fx.mem.i16(COUNTER) >= 0xA2 && fx.mem.i16(t + 0x54) >= 7 {
        let v = fx.mem.i16(t + 8) + 0x19;
        fx.mem.set16(t + 8, v);
        fade_out(fx, t, 0x63, (0xF, 0xD));
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E07240(D): b252's unit flash on this record's globals.
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
