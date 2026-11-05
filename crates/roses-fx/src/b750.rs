//! BMODULE b750 (card 364), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02F40, data area 0x1ED0000, 0xB630 bytes) [F]
//!: b482's program (400 tasks of 0x70 bytes at
//! D+0x2A0, alive +0x58, step +0x44 through `jalr`, the flash tasks
//! 0x1E02C80 counted and stepped by the unit flash 0x1E038A0 = b482's
//! 0x1E03DF0; every global where b482 has it) with other steps, spawned by
//! 0x1E036F0 (b590's spawn with the kind D+0xB2B0 into +0x50 / +0x52 and
//! the angle D+0xB2A2 into +0x5A) and drawn with the second library: arcs
//! (e6dce326551d at 0x1E00ED0), rings of quads (7369422df7f8 at
//! 0x1E00920), quads (add5d81dd3fe at 0x1E01230), sparks (21cb40d76bec at
//! 0x1E014F0) and light columns (792ca1ad9378 at 0x1E018F0). Update 1 the
//! stage light and three dark red flashes of the target; 0x48 at node
//! 0x58 a red arc (b588's, radius 220), twenty red quads flying out
//! (b701's), forty spark bursts (b735's with no +0x4C, red / dark red or
//! pink) and four light columns (red / violet, 1300 / 800 high);
//! 0x48..0x4D four growing grey rings a update there. Returns 1 on update
//! 0x3E, 4 on 0x87, 2 on 0x66, 6 on 0x49..0x60, 8 once the 0x8C-update
//! timer D+0xB210 has run out.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const ARC: u32 = 0x01E0_2040;
const RING: u32 = 0x01E0_2130;
const FLY: u32 = 0x01E0_22E0;
const SPARK_A: u32 = 0x01E0_2580;
const SPARK_B: u32 = 0x01E0_2770;
const COLUMN: u32 = 0x01E0_2960;
const LIGHT: u32 = 0x01E0_2B70;
/// The flash tasks, run by the unit flash 0x1E038A0.
const FLASH: u32 = 0x01E0_2C80;
/// The return address of the entry's `jalr` into a step (0x1E03250 + 8).
const TASK_RA: u64 = 0x01E0_3258;
const POS: u32 = D + 0x250;
/// Node 0x58's point, lifted 3.3 a update over 0x97..0x15F.
const AIM: u32 = D + 0xB1D0;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const LAYER: u32 = D + 0xB224;
const FLASH_DESC: u32 = D + 0xB228;
const NODES: u32 = D + 0xB244;
const SPAWN_A: u32 = D + 0xB2A2;
const SPAWN_K: u32 = D + 0xB2B0;
const COLOUR: u32 = D + 0xB214;
const FACING: u32 = D + 0xB2B4;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;
const SINES: u32 = D + 0xB4BC;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02F40 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x8C);
    for (k, node) in [0x58, 0x34, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_3138);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3168, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_31BC);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

/// The last XYZ2 of the last primitive drawn since `before` (0 when none).
fn last_xyz2(fx: &mut Fx, before: usize) -> u32 {
    match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => 0,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// D+0x94.. = (0, 0, `w`, 0x40) as i16.
fn strip(fx: &mut Fx, w: i32) {
    for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, w), (0x9A, 0x40)] {
        fx.mem.set16(D + o, v);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s0: the flash tasks counted here (the steps' saved s0).
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 => continue,
            FLASH => {
                flashes += 1;
                continue;
            }
            _ => {}
        }
        let s = fx.enter(0x20);
        fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(flashes))]);
        match f {
            ARC => arc(fx, t),
            RING => ring(fx, t),
            FLY => fly(fx, t),
            SPARK_A => sparks(fx, t, SPARK_A, ((0xC0, 0x40, 0x40), 0x14, 0x82, 0xA)),
            SPARK_B => sparks(fx, t, SPARK_B, ((0x40, 0x20, 0x20), 4, 0x96, 8)),
            COLUMN => column(fx, t),
            LIGHT => light(fx, t),
            _ => panic!("b750: unknown task step {f:#x}"),
        }
        fx.leave(0x20);
    }
    // a3 as the draws' last library call left it, for the unit flash's
    // first 0x207BB0: 0 after a quad, a ring, sparks or columns, the last
    // XYZ2 after an arc.
    let mut a3 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            a3 = last_xyz2(fx, before);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == RING {
            tex(fx, TEX);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x60, 0x14, c);
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand_from(flashes, 0x01E0_3350);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == FLY {
            tex(fx, TEX + 4);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x9B, c);
            strip(fx, 0x40);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
            a3 = 0;
        }
        for (g, local, tx) in [(SPARK_A, 0x80u32, TEX + 4), (SPARK_B, 0x90, TEX + 8)] {
            if f(fx) != g {
                continue;
            }
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + local, 0x9B, c);
            tex(fx, tx);
            strip(fx, 0x20);
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            fx.mem.set16(D + 0xB8, 0xC);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == COLUMN {
            tex(fx, D + 0x29C);
            cell(fx, sp + 0xA0, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
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
        0x3E => 1,
        0x87 => 4,
        0x66 => 2,
        0x49..=0x60 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E036F0(D, step, 0): b590's spawn with this record's kind / angle
/// globals; its frame's saves the emitter's s0 = D / s1, the entry's s2 =
/// D / s3 = param, the runner's s4 (0 [H]) and the return address `ra`.
fn spawn(fx: &mut Fx, step: u32, s1: u32, param: i32, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, u64::from(s1)), (0x20, u64::from(D)), (0x30, param as i64 as u64), (0x40, 0), (0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        Some(k) => {
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
            let s = fx.mem.i16(SPAWN_K);
            fx.mem.set16(t + 0x50, s);
            let s = fx.mem.i16(SPAWN_K);
            fx.mem.set16(t + 0x52, s);
            let a = fx.mem.i16(SPAWN_A);
            fx.mem.set16(t + 0x5A, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

fn node0(fx: &mut Fx, out: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    fx.node_position(h, slot, out);
}

/// 0x1E01C60(D, 0): the update counter and the spawns (frame 0x30: ra,
/// the entry's s1 = 0, s0 = 0; then s0 = D, s1 a loop count).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3218), (0x10, 0), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if (0x97..0x160).contains(&c(fx)) {
        node0(fx, AIM);
        let y = ee::add(fx.mem.f(AIM + 4), f32::from_bits(0x4053_3333));
        fx.mem.setf(AIM + 4, y);
        for k in 0..4 {
            let w = fx.mem.read_u32(AIM + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    }
    if c(fx) == 1 {
        spawn(fx, LIGHT, 0, param, 0x01E0_1D50);
    }
    for ra in [0x01E0_1D7C, 0x01E0_1DA8, 0x01E0_1DD4] {
        if c(fx) == 1 {
            spawn(fx, FLASH, 0, param, ra);
        }
    }
    if c(fx) == 0x48 {
        node0(fx, POS);
        spawn(fx, ARC, 0, param, 0x01E0_1E20);
        for (f, ra) in [(FLY, 0x01E0_1E3Cu32), (SPARK_A, 0x01E0_1E78), (SPARK_B, 0x01E0_1EB8)] {
            for k in 0..0x14 {
                spawn(fx, f, k, param, ra);
            }
        }
        let ras = [0x01E0_1F08, 0x01E0_1F38, 0x01E0_1F6C, 0x01E0_1FA0];
        for ((a, kind), ra) in [(0, 0), (0x5A, 0), (0x2D, 1), (0x87, 1)].into_iter().zip(ras) {
            fx.mem.set16(SPAWN_A, a);
            fx.mem.set16(SPAWN_K, kind);
            spawn(fx, COLUMN, 0x14, param, ra);
        }
    }
    if (0x48..0x4E).contains(&c(fx)) {
        node0(fx, POS);
        for k in 0..4 {
            spawn(fx, RING, k, param, 0x01E0_2000);
        }
    }
    fx.leave(0x30);
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

/// 0x1E02040 (D, task): b588's arc ring (0x1E02500) in red: outer radius
/// 220, its outer alpha in by 0x60 for 14 updates, then out by 9,
/// spinning.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x435C_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xE {
        fade_in(fx, t, 0x67, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x67, 0xB, 9);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2100);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02130 (D, task): b703's growing grey ring (0x1E02370) with size
/// 25..44, growth 26..55 and life 3..5: radius 10..49; in by 0x40 under
/// 0xC0, out by 0xE.
fn ring(fx: &mut Fx, t: u32) {
    let f = RING;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 0x14);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x19));
        let v = rmod(fx, t, f + 0x50, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x7C, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, f + 0x9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB8, 0x1E);
        fx.mem.set16(t + 0xE, v + 0x1A);
        let v = rmod(fx, t, f + 0xD8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xF4, 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0x10 {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E022E0 (D, task): b701's fly (0x1E02030, word for word): a red quad
/// of size 100 growing by 10 to 200, flying (-50..49) / 60 a update,
/// turned by the camera's angles; in by 0x40 under 0x80 for 20 updates,
/// out by 9 (the entry's s1 = 0 under 0x205580).
fn fly(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x64);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2310);
        fx.mem.set16(t + 0x56, (v >> 4) % 3);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_2344u32), (0x30, 0x01E0_2370), (0x38, 0x01E0_239C)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_23F4, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2410);
        for (o, ra) in [(4u32, 0x01E0_2418u32), (2, 0x01E0_2438), (0, 0x01E0_2458)] {
            let v = fx.rand_from(t, ra);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x14 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let s = fx.mem.i16(t + 8);
        if s < 0xC8 {
            fx.mem.set16(t + 8, s + 0xA);
        }
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
        for o in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(t + 0x30 + o));
            fx.mem.setf(t + 0x10 + o, v);
        }
    }
    for (o, by) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + o).wrapping_add(fx.mem.i16(t + by) as u32);
        fx.mem.write_u32(t + o, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2560);
    age(fx, t);
}

/// 0x1E02580 / 0x1E02770 (`f`) (D, task): b735's spark burst (0x1E02850)
/// with +0x4C left 0: size 200 growing by (rand % 80 + k.2) / 10 a update;
/// kind (rand >> 4) % 2: 0 `k.0` on black, 1 pink (0xFF, 0xA0, 0xA0) on
/// grey 0x30; +0x5A 60, +0x5C `k.1`; 0x80 for `k.3` updates, out by 5.
fn sparks(fx: &mut Fx, t: u32, f: u32, k: ((i32, i32, i32), i32, i32, i32)) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.setf(t + 0x4C, ee::sub(200.0, 58.0));
        let v = fx.rand_from(t, f + 0x40);
        fx.mem.set16(t + 0x50, (v >> 4) % 2);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, k.0 .0, k.0 .1, k.0 .2);
            l2::rgb(fx, t + 0x64, 0, 0, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xFF, 0xA0, 0xA0);
            l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        }
        fx.mem.set16(t + 0x5A, 0x3C);
        fx.mem.set16(t + 0x5C, k.1);
        let v = rmod(fx, t, f + 0xDC, 0x50);
        fx.mem.setf(t + 0x34, ee::from_i32(v + k.2));
        let v = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, ra) in [(0x20u32, f + 0x124), (0x24, f + 0x140), (0x28, f + 0x15C)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < k.3 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, 7, 5);
    }
    let s = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, s);
    fx.mem.write_u32(t + 0x4C, 0);
    age(fx, t);
}

/// 0x1E02960 (D, task): b735's light column (0x1E02A50) in red / violet:
/// size 23 / 23 / 6, height 1300 / 800 / 100 by +0x50 growing by 80 / 40
/// / 5, turning by 6; in by 0x60 under 0xC0 for 4 updates, out by 0xD.
fn column(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0x60, 0x60);
        l2::rgb(fx, t + 0x64, 0x80, 0x40, 0xFF);
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x30, 0x41B8_0000);
            fx.mem.write_u32(t + 0x34, 0x44A2_8000);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x30, 0x41B8_0000);
            fx.mem.write_u32(t + 0x34, 0x4448_0000);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            fx.mem.write_u32(t + 0x30, 0x40C0_0000);
            fx.mem.write_u32(t + 0x34, 0x42C8_0000);
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xC0);
            l2::rgb(fx, t + 0x64, 0xFF, 0x40, 0x40);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0xF, 0xD);
    }
    for (kind, by) in [(0, 80.0f32), (1, 40.0), (2, 5.0)] {
        if fx.mem.i16(t + 0x50) == kind {
            if fx.mem.i16(t + 0x54) != 0 {
                add(fx, t + 0x34, by);
            }
            let a = fx.mem.i16(t + 0x5A) + 6;
            fx.mem.set16(t + 0x5A, a);
        }
    }
    age(fx, t);
}

/// 0x1E02B70 (D, task): b482's stage light (0x1E030E0) down by 10 to 0x30
/// for 40 updates, back up by 10 to 0x80 once the timer is under 30; 0x80
/// and the task's end when the timer is at 3.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x28 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x1E {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    age(fx, t);
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
}

/// 0x1E02C80(D, t): b482's flash step (0x1E031F0) in dark red (0x50,
/// 0x20, 0x20): alpha up by 9 below 0x80 for 100 updates, then down by 7.
/// `regs` the unit flash's s0 / s1 (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_39AC), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f20 / f21: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x50, 0x20, 0x20);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, xy) in [(0x20u32, 0x6FF8u16), (0x30, 0x8FF8)] {
        fx.mem.write_u16(d + at, xy);
        fx.mem.write_u16(d + at + 2, xy);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    for (o, v) in [(0x18u32, ee::div(w, 512.0)), (0x1C, ee::div(h, 512.0)), (0x28, ee::div(ee::sub(512.0, w), 512.0)), (0x2C, ee::div(ee::sub(512.0, h), 512.0))] {
        fx.mem.setf(d + o, v);
    }
    fx.submit(d, 0, 0);
    if fx.mem.i16(t + 0x54) < 0x64 {
        fade_in(fx, t, 0x63, 0x80, 9);
    } else {
        fade_out(fx, t, 0x63, 9, 7);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E038A0(D) (d1cf0abc65df's twin, b482's 0x1E03DF0): in a packet
/// layer the attacker drawn into the target (0x1F8270 filter 0; ALPHA
/// 0x48), TEXFLUSH before and after; every flash task stepped
/// (0x1E02C80); the layer linked into the ordering table at key 0 when
/// any task was alive.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xB0);
    let d = sp + 0x58;
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
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x98, sp + 0xA0);
    fx.texflush(regs);
    // s1 counts every live task (the `bne`'s delay slot).
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        if fx.mem.read_u32(t + 0x44) == FLASH {
            flash(fx, t, [k, live]);
        }
    }
    if live != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}
