//! BMODULE b529 (card 47), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E035F0, data area 0x1ED0000) [F] (helper E11, 3
//! Nov 2026): a pool of 250 tasks of 0x70 bytes at D+0x2A0 (alive +0x58,
//! step +0x44 called through `jalr`; spawned by 0x1E03DB0, b144's
//! 0x1E04120) drawn as petals (`b764::petal` at 0x1E03F00), flat arcs
//! (`billboard::arc_strips_flat` 0x1E00ED0), sparks (`quads4::sparks`
//! 0x1E01230), a window tint (`quads4::tint4` 0x1E01630) and rings
//! (`billboard::ring_quads` 0x1E00920, UVs by uv_rect 0x1E00080).
//!
//! A point rides the attacker's node 0x3A (D+0x7270) until update 189,
//! then drifts a 50th of the way to the target's node 1 a update until 238.
//! Update 1: the stage dimmed to 0x40 (0x1E03090) until the timer is under
//! 21, a full-window frame tint (0x1E03170, (0x78, 0x78, 0x90), ST inset
//! 1.5 / 512) and a white / blue arc (0x1E023B0). Updates 73..144: a
//! petal a update (0x1E01E90, 72 at 10 degrees, two rings of 36) opening
//! and turning about the point; 146: a white ring of sparks; 164: two
//! more arcs, 20 dark blue arcs (0x1E027F0) and 11 red rings (0x1E02D10);
//! 250..280: wide blue arcs (0x1E02AB0) on even updates, a blue arc and a
//! red ring every update; 288: two arcs and a pale tint at the target
//! (0x1E02C10); until 300 a grey ring (0x1E02EF0) every update. Returns 1
//! on update 231, 4 on 345, 2 on 315, 6 on 242..309, 8 once the 360-update
//! timer D+0x72B0 has run out.
//!
//! The flash step 0x1E03350 is run only by the unit flash 0x1E04200 (both
//! b183's), and nothing spawns a flash task [F: the spawn `jal`s name
//! 0x1E03090 / 0x1E03170 / 0x1E01E90 / 0x1E023B0 / 0x1E027F0 / 0x1E02D10 /
//! 0x1E02650 / 0x1E02AB0 / 0x1E02C10 / 0x1E02EF0]: neither is ported; the
//! entry's draw of 0x1E02240 (an arc at the task) is never reached either.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::b590::project_residue;
use crate::b764::{cell, Glob};
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xFA;
const PETAL: u32 = 0x01E0_1E90;
const ARC_T: u32 = 0x01E0_2240;
const ARC: u32 = 0x01E0_23B0;
const SPARKS: u32 = 0x01E0_2650;
const ARC_D: u32 = 0x01E0_27F0;
const ARC_W: u32 = 0x01E0_2AB0;
const TINT: u32 = 0x01E0_2C10;
const RING: u32 = 0x01E0_2D10;
const RING_G: u32 = 0x01E0_2EF0;
const LIGHT: u32 = 0x01E0_3090;
const FRAME: u32 = 0x01E0_3170;
const FLASH: u32 = 0x01E0_3350;
/// The return address of the entry's `jalr` into a step (0x1E038D8 + 8).
const TASK_RA: u64 = 0x01E0_38E0;
const POS: u32 = D + 0x250;
const POINT: u32 = D + 0x7270;
const DRIFT: u32 = D + 0x7280;
const TEX: u32 = D + 0x7290;
const TIMER: u32 = D + 0x72B0;
const COLOUR: u32 = D + 0x72B4;
const OT_AT: u32 = D + 0x72BC;
const TINT_W: u32 = D + 0x72CC;
const NODES: u32 = D + 0x72E8;
const TARGET_NODE: u32 = D + 0x72F8;
const CELLS: u32 = D + 0x7308;
const PETALS: u32 = D + 0x7398;
const WIDE_N: u32 = D + 0x739A;
const ANGLE: u32 = D + 0x739C;
const INDEX: u32 = D + 0x73A8;
const FACING: u32 = D + 0x73AC;
const COUNTER: u32 = D + 0x73B0;
const BYTES: u32 = D + 0x73B2;
const BYTES_B: u32 = D + 0x74B2;
const SINES: u32 = D + 0x75B4;

const G: Glob = Glob { colour: COLOUR, angle: ANGLE, index: INDEX, cells: CELLS, sines: SINES, ot_at: OT_AT };

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E035F0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x7720);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x73AE, param);
    fx.mem.set32(TIMER, 0x168);
    for (k, (node, clear)) in [(0x3A, 1), (0x34, 0), (0x35, 0), (0x30, 0), (0x2F, 0), (0x2C, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_3790);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = the loop counter.
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_37B8, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3804, 0x28);
        fx.mem.set8(BYTES_B + k, v - 0x14);
    }
    for k in 0..0x24u32 {
        let v = fx.rand_from(k, 0x01E0_384C);
        fx.mem.set16(CELLS + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s0: the flash tasks counted here.
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        // The steps' saves: the entry's s0 = the flashes (0), s1 = 0.
        match f {
            0 => {}
            FLASH => flashes += 1,
            PETAL => petal_step(fx, t),
            ARC => arc(fx, t),
            SPARKS => sparks(fx, t),
            ARC_D => arc_dark(fx, t),
            ARC_W => arc_wide(fx, t),
            TINT => tint(fx, t),
            RING => ring(fx, t),
            RING_G => ring_grey(fx, t),
            LIGHT => light(fx, t),
            FRAME => frame(fx, t),
            _ => panic!("b529: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == PETAL {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x80, 0x14);
            fx.mem.set16(D + 0xB8, 1);
            fx.mem.set16(D + 0xBE, 0);
            crate::b764::petal(fx, &G, t, 0x20, POINT);
        }
        debug_assert_ne!(f(fx), ARC_T);
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x18);
            billboard::arc_strips_flat(fx, D, t, 0x21, POINT);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == SPARKS {
            cell(fx, t, sp + 0x90, 0x14);
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let v = fx.mem.read_u32(POINT + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, 0x24);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, POINT);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == ARC_D {
            fx.mem.set16(D + 0xB8, 0x18);
            billboard::arc_strips_flat(fx, D, t, 0x20, POINT);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == ARC_W {
            fx.mem.set16(D + 0xB8, 1);
            billboard::arc_strips_flat(fx, D, t, 0x20, POINT);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == TINT {
            let h = fx.target_handle();
            fx.unit_position(h, sp + 0x70);
            // 0x1F71A0's frame below the entry's (its saved ra, s0 = the
            // flashes, 0).
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_3B58), (0, 0)]);
            let z = fx.mem.f(sp + 0x78);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 30.0) } else { ee::add(z, 30.0) };
            fx.mem.setf(sp + 0x78, z);
            let p = [fx.mem.f(sp + 0x70), fx.mem.f(sp + 0x74), fx.mem.f(sp + 0x78)];
            let mut x = fx.mem.read_u64(sp + 0xE8);
            if let Some(w) = fx.project(p, None, &mut x) {
                fx.mem.write_u64(sp + 0xE8, x);
                fx.mem.setf(TINT_W, w);
            }
            project_residue(fx, sp, sp + 0x70);
            let w = fx.mem.f(TINT_W);
            quads4::tint4(fx, t, 0x20, w);
        }
        for (step, local, rows) in [(RING, 0xA0u32, 7), (RING_G, 0xB0, 3)] {
            if f(fx) == step {
                let w = fx.mem.read_u32(TEX + 8);
                fx.mem.write_u32(D + 0x88, w);
                cell(fx, t, sp + local, 0x14);
                fx.mem.set16(D + 0xB8, rows);
                fx.mem.set16(D + 0xB0, 0);
                billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
                fx.mem.set16(D + 0xB8, 0);
                fx.mem.set16(D + 0xB0, 0);
            }
        }
    }
    // The unit flash 0x1E04200 (its flash tasks 0x1E03350) runs only when
    // a flash task is alive: never [F].
    debug_assert_eq!(flashes, 0);
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xE7 => 1,
        0x159 => 4,
        0x13B => 2,
        0xF2..=0x135 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E03DB0(D, step, 0): the first free task cleared and set off at
/// D+0x250 (i16 +0, floats +0x10..+0x18) with the colour D+0x72B4
/// (twice), angle D+0x739C (+0x5A), kind D+0x73A8 (+0x50); `saves` the
/// caller's s0..s4, `ra` the return address.
fn spawn(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        let t = D + 0x2A0 + 0x70 * k;
        fx.bzero(t, 0x70);
        fx.mem.set16(t + 0x58, 1);
        for o in [0x60u32, 0x64] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + o + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        for k in 0..3 {
            let w = fx.mem.read_u32(POS + 4 * k);
            fx.mem.write_u32(t + 0x10 + 4 * k, w);
        }
        let s = fx.mem.i16(ANGLE);
        fx.mem.set16(t + 0x5A, s);
        let s = fx.mem.i16(INDEX);
        fx.mem.set16(t + 0x50, s);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

fn copy_point(fx: &mut Fx) {
    for k in 0..4 {
        let w = fx.mem.read_u32(POINT + 4 * k);
        fx.mem.write_u32(POS + 4 * k, w);
    }
}

/// 0x1E01950(D, 0): the update counter, the point and the spawns (frame
/// 0x50: ra, the entry's s1 = s0 = 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_38A0), (0x10, 0), (0, 0)]);
    // The spawns' saves: s0 = D, s1 = the emitter's loop counter (the
    // entry's 0 before a loop), s2 = D, s3 = the parameter, s4 the
    // runner's (0 [H]).
    let d = u64::from(D);
    let p = param as i64 as u64;
    let mut s1 = 0u64;
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, LIGHT, [d, s1, d, p, 0], 0x01E0_198C);
        spawn(fx, FRAME, [d, s1, d, p, 0], 0x01E0_19A0);
    }
    if fx.mem.i16(COUNTER) >= 0x49 && fx.mem.i16(PETALS) < 0x48 {
        let i = fx.mem.i16(PETALS);
        fx.mem.set16(ANGLE, i * 10);
        let i = fx.mem.i16(PETALS);
        fx.mem.set16(INDEX, i);
        spawn(fx, PETAL, [d, s1, d, p, 0], 0x01E0_19EC);
        let i = fx.mem.i16(PETALS) + 1;
        fx.mem.set16(PETALS, i);
    }
    let c = fx.mem.i16(COUNTER);
    if c < 0xBE {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, POINT, d, Some(s1), 0x01E0_1A20);
    } else {
        if c == 0xBE {
            let h = fx.target_handle();
            let slot = fx.mem.i16(TARGET_NODE);
            node_position_saving(fx, h, slot, sp + 0x30, d, Some(s1), 0x01E0_1A4C);
            for o in [8u32, 4, 0] {
                let v = ee::div(ee::sub(fx.mem.f(sp + 0x30 + o), fx.mem.f(POINT + o)), 50.0);
                fx.mem.setf(DRIFT + o, v);
            }
        }
        if fx.mem.i16(COUNTER) < 0xEF {
            for o in [8u32, 4, 0] {
                let v = ee::add(fx.mem.f(POINT + o), fx.mem.f(DRIFT + o));
                fx.mem.setf(POINT + o, v);
            }
        }
    }
    let c = fx.mem.i16(COUNTER);
    if c % 3 == 1 {
        copy_point(fx);
    }
    if fx.mem.i16(COUNTER) == 1 {
        copy_point(fx);
        fx.mem.set16(INDEX, 0);
        spawn(fx, ARC, [d, s1, d, p, 0], 0x01E0_1B5C);
    }
    if fx.mem.i16(COUNTER) == 0xA4 {
        copy_point(fx);
        fx.mem.set16(INDEX, 1);
        spawn(fx, ARC, [d, s1, d, p, 0], 0x01E0_1BA8);
        fx.mem.set16(INDEX, 2);
        spawn(fx, ARC, [d, s1, d, p, 0], 0x01E0_1BC4);
        copy_point(fx);
        fx.mem.set16(INDEX, 0);
        for i in 0..0x14u64 {
            s1 = i;
            spawn(fx, ARC_D, [d, s1, d, p, 0], 0x01E0_1C04);
        }
        for i in 0..0xAu64 {
            s1 = i;
            spawn(fx, RING, [d, s1, d, p, 0], 0x01E0_1C40);
        }
        s1 = 0xA;
        spawn(fx, RING, [d, s1, d, p, 0], 0x01E0_1C78);
    }
    if fx.mem.i16(COUNTER) == 0x92 {
        copy_point(fx);
        fx.mem.set16(INDEX, 0);
        spawn(fx, SPARKS, [d, s1, d, p, 0], 0x01E0_1CC0);
    }
    let c = fx.mem.i16(COUNTER);
    if (0xFA..0x119).contains(&c) {
        copy_point(fx);
        let w = fx.mem.i16(WIDE_N);
        fx.mem.set16(INDEX, w);
        if fx.mem.i16(COUNTER) % 2 == 0 {
            spawn(fx, ARC_W, [d, s1, d, p, 0], 0x01E0_1D38);
        }
        let w = fx.mem.i16(WIDE_N) + 1;
        fx.mem.set16(WIDE_N, w);
    }
    let c = fx.mem.i16(COUNTER);
    if (0xFA..0x119).contains(&c) {
        copy_point(fx);
        fx.mem.set16(INDEX, 1);
        spawn(fx, ARC_D, [d, s1, d, p, 0], 0x01E0_1D9C);
        spawn(fx, RING, [d, s1, d, p, 0], 0x01E0_1DB0);
    }
    if fx.mem.i16(COUNTER) == 0x120 {
        copy_point(fx);
        fx.mem.set16(INDEX, 1);
        spawn(fx, ARC, [d, s1, d, p, 0], 0x01E0_1DFC);
        fx.mem.set16(INDEX, 2);
        spawn(fx, ARC, [d, s1, d, p, 0], 0x01E0_1E18);
        spawn(fx, TINT, [d, s1, d, p, 0], 0x01E0_1E2C);
    }
    if fx.mem.i16(COUNTER) < 0x12D {
        copy_point(fx);
        spawn(fx, RING_G, [d, s1, d, p, 0], 0x01E0_1E70);
    }
    fx.leave(0x50);
}

/// A step's frame (`frame` bytes) with the entry's s0 (and s1 when `n` is
/// 2) saved, both 0, ra at `frame - 0x10`.
fn enter_step(fx: &mut Fx, frame: u32, n: u32) {
    crate::b764::enter_step_with(fx, frame, n, [0, 0], TASK_RA);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn odd_age(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54).wrapping_rem(2) == 1
}

/// +a under `cap` up by `by`.
fn up(fx: &mut Fx, t: u32, a: u32, cap: u8, by: u8) {
    let v = fx.mem.read_u8(t + a);
    if v < cap {
        fx.mem.write_u8(t + a, v.wrapping_add(by));
    }
}

/// +a at `floor` or more down by `by`, else the task ends.
fn down(fx: &mut Fx, t: u32, a: u32, floor: u8, by: u8) {
    let v = fx.mem.read_u8(t + a);
    if v >= floor {
        fx.mem.write_u8(t + a, v - by);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E01E90 (D, task; frame 0x30): a petal: radii +0x48 / +0x4C from 0
/// opening by 0.5 (the inner 36 to 10 then 7, the outer to 6 then 4),
/// grey 0x80 / 0x70 by its age's parity, in by 8 under 0x80 for 30
/// updates, out by 11 from update 301; turned by the camera until update
/// 176, then spinning about z (by 256 + +0x30, the inner ones back), the
/// spin +0x30 up by 32 to 512 from update 241.
fn petal_step(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.write_u32(t + 0x4C, 0);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.set16(t + 0x58, 2);
        let i = fx.mem.i16(t + 0x50);
        if i < 0x24 {
            fx.mem.set16(t + 0x52, 0);
        } else if i < 0x48 {
            fx.mem.set16(t + 0x52, 2);
        }
        let v = rmod(fx, t, 0x01E0_1EF8, 3);
        fx.mem.set16(t + 8, v + 4);
    }
    if fx.mem.i16(t + 0x54) < 0x1E {
        up(fx, t, 0x63, 0x80, 8);
        up(fx, t, 0x67, 0x80, 8);
    } else if fx.mem.i16(COUNTER) >= 0x12D {
        down(fx, t, 0x63, 0xD, 0xB);
        let v = fx.mem.read_u8(t + 0x67);
        if v >= 0xD {
            fx.mem.write_u8(t + 0x67, v - 0xB);
        }
    }
    let g = if odd_age(fx, t) { 0x80 } else { 0x70 };
    l2::rgb(fx, t + 0x60, g, g, g);
    l2::rgb(fx, t + 0x64, g, g, g);
    if fx.mem.i16(COUNTER) != 0 {
        let phase = fx.mem.i16(t + 0x52);
        let grow = |fx: &mut Fx, o: u32, to: f32, next: bool| {
            let r = fx.mem.f(t + o);
            if r < to {
                fx.mem.setf(t + o, ee::add(r, 0.5));
            } else if next {
                fx.mem.set16(t + 0x52, phase + 1);
            }
        };
        match phase {
            0 => grow(fx, 0x48, 10.0, true),
            1 => grow(fx, 0x4C, 7.0, false),
            2 => grow(fx, 0x48, 6.0, true),
            3 => grow(fx, 0x4C, 4.0, false),
            _ => {}
        }
    }
    if fx.mem.i16(COUNTER) < 0xB0 {
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_21D0);
        fx.mem.write_u32(t + 0x20, 0x4000);
    } else {
        let a = ee::from_i32(fx.mem.i32(t + 0x28));
        let by = ee::add(256.0, fx.mem.f(t + 0x30));
        let (v, ra) = if fx.mem.i16(t + 0x50) < 0x24 { (ee::add(a, by), 0x01E0_2190) } else { (ee::sub(a, by), 0x01E0_21BC) };
        fx.fix_residue(v, ra);
        fx.mem.set32(t + 0x28, ee::fixsfsi(v));
    }
    if fx.mem.i16(COUNTER) >= 0xF1 {
        let s = fx.mem.f(t + 0x30);
        if s < 512.0 {
            fx.mem.setf(t + 0x30, ee::add(s, 32.0));
        }
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E023B0 (D, task; frame 0x20): an arc by its kind +0x50: 0 radius 30
/// white over (0x80, 0x80, 0xFF) for 300 updates, in by 14 under 0xC0 and
/// flickering grey 0x8F / 0x70; 1 radius 300 white over (0xC0, 0xC0, 0xFF)
/// for 12, in by 0x40; 2 radius 110 (0x80, 0x80, 0xFF) over white for 12,
/// the outer in by 0x40; then out by 11 (2: the outer by 15); turned by
/// the camera.
fn arc(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        let kind = fx.mem.i16(t + 0x50);
        let cfg = match kind {
            0 => Some((0x41F0_0000u32, 0x12C, (0x60u32, 0xFF, 0xFF, 0xFF), (0x64u32, 0x80, 0x80, 0xFF))),
            1 => Some((0x4396_0000, 0xC, (0x60, 0xFF, 0xFF, 0xFF), (0x64, 0xC0, 0xC0, 0xFF))),
            2 => Some((0x42DC_0000, 0xC, (0x64, 0xFF, 0xFF, 0xFF), (0x60, 0x80, 0x80, 0xFF))),
            _ => None,
        };
        if let Some((r, life, a, b)) = cfg {
            fx.mem.write_u32(t + 0x48, r);
            fx.mem.write_u32(t + 0x4C, 0);
            fx.mem.set16(t + 6, life);
            l2::rgb(fx, t + a.0, a.1, a.2, a.3);
            l2::rgb(fx, t + b.0, b.1, b.2, b.3);
        }
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let kind = fx.mem.i16(t + 0x50);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        match kind {
            0 => up(fx, t, 0x63, 0xC0, 0xE),
            1 => up(fx, t, 0x63, 0xC0, 0x40),
            2 => up(fx, t, 0x67, 0xC0, 0x40),
            _ => {}
        }
    } else {
        match kind {
            0 | 1 => down(fx, t, 0x63, 0xD, 0xB),
            2 => down(fx, t, 0x67, 0x11, 0xF),
            _ => {}
        }
    }
    if fx.mem.i16(t + 0x50) == 0 {
        let g = if odd_age(fx, t) { 0x8F } else { 0x70 };
        l2::rgb(fx, t + 0x60, g, g, g);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2624);
    fx.mem.write_u32(t + 0x20, 0x4000);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02650 (D, task; frame 0x20): a white ring of sparks (radii 110 /
/// 105, kind 1: 80 / 75), height 95, cell 4, in by 14 under 0xC0 for 103
/// updates, then out by 14; turned by the camera.
fn sparks(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        let kind = fx.mem.i16(t + 0x50);
        let r = match kind {
            0 => Some(110.0f32),
            1 => Some(80.0),
            _ => None,
        };
        if let Some(r) = r {
            fx.mem.setf(t + 0x48, r);
            fx.mem.setf(t + 0x4C, ee::sub(r, 5.0));
        }
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.write_u32(t + 0x34, 0x446D_8000);
        let h = fx.mem.f(t + 0x34);
        fx.mem.setf(t + 0x34, ee::div(h, 10.0));
        fx.mem.set16(t + 0x58, 2);
        fx.mem.set16(t + 0x56, 4);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x67 {
        up(fx, t, 0x63, 0xC0, 0xE);
        up(fx, t, 0x67, 0xC0, 0xE);
    } else {
        down(fx, t, 0x63, 0x10, 0xE);
        down(fx, t, 0x67, 0x10, 0xE);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_27C0);
    fx.mem.write_u32(t + 0x20, 0x4000);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E027F0 (D, task; frame 0x20): a dark arc (grey 0x80 over (0x10,
/// 0x10, 0x30), the outer flickering (0x30, 0x30, 0x60 / 0x50)): kind 0
/// radius 5 + rand % 100 for 8 + rand % 18 updates growing by 10..29,
/// width 2; kind 1 100 + rand % 100 for 3..10 growing by 5..14, width 5;
/// turned at random; in by 0x2A under 0x80, then out by 15.
fn arc_dark(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            let v = rmod(fx, t, 0x01E0_2820, 0x64);
            fx.mem.setf(t + 0x48, ee::add(5.0, ee::from_i32(v)));
            fx.mem.write_u32(t + 0x4C, 0x3F80_0000);
            let v = rmod(fx, t, 0x01E0_2858, 0x12);
            fx.mem.set16(t + 6, v + 8);
            fx.mem.set16(t + 0x5C, 2);
            let v = rmod(fx, t, 0x01E0_2880, 0x14);
            fx.mem.set16(t + 0xE, v + 0xA);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            let v = rmod(fx, t, 0x01E0_28B4, 0x64);
            fx.mem.setf(t + 0x48, ee::add(100.0, ee::from_i32(v)));
            fx.mem.write_u32(t + 0x4C, 0x3F80_0000);
            let v = fx.rand_from(t, 0x01E0_28EC);
            fx.mem.set16(t + 6, v % 8 + 3);
            fx.mem.set16(t + 0x5C, 5);
            let v = rmod(fx, t, 0x01E0_2914, 0xA);
            fx.mem.set16(t + 0xE, v + 5);
        }
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x10, 0x10, 0x30);
        fx.mem.set16(t + 0x5A, 0xF);
        camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2968);
        fx.mem.write_u32(t + 0x20, 0x4000);
        turn(fx, t, [0x01E0_2974, 0x01E0_2990, 0x01E0_29AC]);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        up(fx, t, 0x63, 0x80, 0x2A);
    } else {
        down(fx, t, 0x63, 0x11, 0xF);
    }
    let b = if odd_age(fx, t) { 0x60 } else { 0x50 };
    l2::rgb(fx, t + 0x64, 0x30, 0x30, b);
    let r = ee::add(fx.mem.f(t + 0x48), ee::from_i32(fx.mem.i16(t + 0xE)));
    fx.mem.setf(t + 0x48, r);
    age(fx, t);
    fx.leave(0x20);
}

/// +0x20 / +0x24 / +0x28 = rand % 0xFFFF each.
fn turn(fx: &mut Fx, t: u32, ras: [u32; 3]) {
    for (o, ra) in [0x20u32, 0x24, 0x28].into_iter().zip(ras) {
        let v = rmod(fx, t, ra, 0xFFFF);
        fx.mem.set32(t + o, v);
    }
}

/// 0x1E02AB0 (D, task; frame 0x30): a wide arc (radius 600, grey 0x80
/// over (0x40, 0x40, 0xF0)), turned at random, in by 0x40 under 0xC0 until
/// update 300, then out by 15.
fn arc_wide(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4416_0000);
        fx.mem.write_u32(t + 0x4C, 0x3F80_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0xF0);
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 0xA);
        let v = rmod(fx, t, 0x01E0_2B20, 0x14);
        fx.mem.set16(t + 0xE, v + 0xA);
        turn(fx, t, [0x01E0_2B40, 0x01E0_2B5C, 0x01E0_2B78]);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(COUNTER) < 0x12C {
        up(fx, t, 0x63, 0xC0, 0x40);
    } else {
        down(fx, t, 0x63, 0x11, 0xF);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02C10 (D, task; frame 0x20): the pale tint (0xF0 grey both), in by
/// 18 under 0x80 for 16 updates, then out by 4 (the bottom to 0).
fn tint(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0xF0);
        l2::rgb(fx, t + 0x64, 0xF0, 0xF0, 0xF0);
        fx.mem.set16(t + 6, 0x10);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        up(fx, t, 0x63, 0x80, 0x12);
        up(fx, t, 0x67, 0x80, 0x12);
    } else {
        down(fx, t, 0x63, 6, 4);
        let v = fx.mem.read_u8(t + 0x67);
        fx.mem.write_u8(t + 0x67, if v >= 6 { v - 4 } else { 0 });
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02D10 (D, task; frame 0x20): a red ring (0xC0, 0x50, 0x50) of size
/// 5..14, cell 4, turned at random, radius 10..39 growing by 10..29, row
/// +0x52 from 1 stepping, +8 1..8, at 0x80 for 5..14 updates then out by
/// 11.
fn ring(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2D34, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 5));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xC0, 0x50, 0x50);
        turn(fx, t, [0x01E0_2D78, 0x01E0_2D94, 0x01E0_2DB0]);
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_2DD4, 0x1E);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_2DF4, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2E10, 0x14);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = fx.rand_from(t, 0x01E0_2E30);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_2E50, 0xA);
        fx.mem.set16(t + 6, v + 5);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t);
    let r = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, r);
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
    fx.leave(0x20);
}

/// At 0x80 for +6 updates, then out by 11.
fn hold(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        down(fx, t, 0x63, 0xD, 0xB);
    }
}

/// 0x1E02EF0 (D, task; frame 0x20): a grey ring (0xC0) of size 1..2, cell
/// 4, turned at random, radius 10..89, +8 1..8, at 0x80 for 3..6 updates
/// then out by 11.
fn ring_grey(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20, 1);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_2F14);
        fx.mem.setf(t + 0x48, ee::from_i32(v % 2 + 1));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        turn(fx, t, [0x01E0_2F5C, 0x01E0_2F78, 0x01E0_2F94]);
        let v = rmod(fx, t, 0x01E0_2FB0, 0x50);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_2FD0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_2FEC);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = fx.rand_from(t, 0x01E0_300C);
        fx.mem.set16(t + 6, v % 4 + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03090 (D, task; frame 0x30): the stage light (0x1F7F10) down by 10
/// to 0x40 while the timer is at 21 or more, then back up by 10 to 0x80;
/// the task ends when the timer is at 2.
fn light(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30, 2);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let l = fx.mem.i16(t + 0x50);
    if fx.mem.i32(TIMER) >= 0x15 {
        fx.mem.set16(t + 0x50, if l < 0x41 { 0x40 } else { l - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    let l = fx.mem.read_u8(t + 0x50);
    fx.set_stage_light(i32::from(l));
    if fx.mem.i32(TIMER) == 2 {
        fx.mem.set16(t + 0x58, 0);
    }
    fx.leave(0x30);
}

/// 0x1E03170 (D, task; frame 0x90): a window sprite (0x6FF8 .. 0x8FF8)
/// sampling the frame (0x207EF0, filter 1) at ST inset by +0x48 (1.5) /
/// 512, colour (0x78, 0x78, 0x90, alpha +0x63), ALPHA 0x44, key 0; in by
/// 8 below 0x50 until update 300, then out by 7.
fn frame(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3FC0_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for (k, c) in [0x78u8, 0x78, 0x90].into_iter().enumerate() {
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
    if fx.mem.i16(COUNTER) < 0x12C {
        up(fx, t, 0x63, 0x50, 8);
    } else {
        down(fx, t, 0x63, 9, 7);
    }
    age(fx, t);
    fx.leave(0x90);
}
