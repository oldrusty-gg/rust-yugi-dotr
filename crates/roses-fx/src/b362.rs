//! BMODULE b362 (card 42), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02B60, data area 0x1ED0000) [F] (helper A12, 31
//! Oct 2026): b482's unit-flash family on a pool 0x10 higher (400 tasks of
//! 0x70 bytes at D+0x2B0, alive +0x58, step +0x44 through `jalr`;
//! spawned by 0x1E03250 = `spawn::task400` at D+0x2B0 with the colour
//! D+0xB204 and the kind D+0xB270) drawn with quads (add5d81dd3fe at
//! 0x1E01330, `billboard::quad`), rings of quads (7369422df7f8 at
//! 0x1E00D80, `billboard::ring_quads`) and bands (3fb2be4df9d4 at
//! 0x1E00920, `quads4::ring_band`), UVs by uv_rect 0x1E00080, plus b482's
//! unit flash (0x1E033E0, d1cf0abc65df at other offsets) with a dark
//! growing flash step (0x1E02880). Update 1 the stage light and a frame
//! quad; every third update to 234 a flash; to 225 a white ring a update
//! at the attacker's node 0x55; 69..155 on odd updates a white quad near
//! node 0x55 flying to the target (bursting there after 70 updates into a
//! ring and a green band); to 200 every third update two rings at nodes
//! 0x5F / 0x5A. Returns 1 on update 130, 4 on 250, 3 on 235, 6 on
//! 141..229, 8 once the 260-update timer +0xB200 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The pool's first task.
const POOL: u32 = D + 0x2B0;
/// The step functions a task holds (+0x44), as the record's addresses.
const QUAD: u32 = 0x01E0_1B10;
const RING_N: u32 = 0x01E0_1EE0;
const RING_A: u32 = 0x01E0_20F0;
const RING_B: u32 = 0x01E0_22B0;
const BAND: u32 = 0x01E0_2490;
const LIGHT: u32 = 0x01E0_25A0;
const FRAME: u32 = 0x01E0_26A0;
/// The flash tasks, run by the unit flash 0x1E033E0.
const FLASH: u32 = 0x01E0_2880;
/// The return address of the entry's `jalr` into a step (0x1E02E70 + 8).
const TASK_RA: u64 = 0x01E0_2E78;
const POS: u32 = D + 0x250;
const TEX: u32 = D + 0xB1E0;
const TIMER: u32 = D + 0xB200;
const COLOUR: u32 = D + 0xB204;
const LAYER: u32 = D + 0xB214;
const FLASH_DESC: u32 = D + 0xB218;
const EYE: u32 = D + 0xB220;
const NODES: u32 = D + 0xB240;
const TARGET_NODE: u32 = D + 0xB250;
const SIDE: u32 = D + 0xB260;
const KIND: u32 = D + 0xB270;
const FACING: u32 = D + 0xB274;
const COUNTER: u32 = D + 0xB278;
const BYTES: u32 = D + 0xB27A;
const BYTES_B: u32 = D + 0xB37A;
const SINES: u32 = D + 0xB47C;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02B60.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0xB20C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB5F0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB276, param);
    fx.mem.set32(TIMER, 0x104);
    for (k, node) in [0x55, 0x5F, 0x5A].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_2CCC);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2CFC, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0xB4..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2D54, 0x14);
        fx.mem.set8(BYTES + k, v - 0xA);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2DA4, 0x28);
        fx.mem.set8(BYTES_B + k, v - 0x14);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(BYTES_B + k, 0);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s1: the flash tasks counted here (the steps' saved s1).
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = POOL + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        // The steps' prologues: the entry's s0 = D, s1 = the flashes.
        let frame = match f {
            0 => continue,
            FLASH => {
                flashes += 1;
                continue;
            }
            QUAD => 0x50,
            RING_N => 0x30,
            FRAME => 0x80,
            RING_A | RING_B | BAND | LIGHT => 0x20,
            _ => panic!("b362: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match frame {
            0x50 => {
                fx.saved(s, &[(0x30, TASK_RA), (0x20, u64::from(flashes)), (0x10, u64::from(D))]);
                // swc1 f20: the runner's (0) [H].
                fx.mem.write_u32(s, 0);
            }
            0x30 => fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(flashes)), (0, u64::from(D))]),
            _ => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]),
        }
        match f {
            QUAD => quad(fx, t, [slot, k]),
            RING_N => ring(fx, t, RING_N),
            RING_A => ring(fx, t, RING_A),
            RING_B => ring(fx, t, RING_B),
            BAND => band(fx, t, flashes),
            LIGHT => light(fx, t),
            _ => frame_quad(fx, s, t),
        }
        fx.leave(frame);
    }
    // a3 as the draws' last library call left it for the unit flash's
    // first TEXFLUSH: 0 after a quad or a ring of quads, the last XYZ2
    // drawn after a band (b491's lesson) [run].
    let mut a3 = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == QUAD {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x70);
            let s = fx.mem.i16(t + 0x56);
            fx.mem.set16(D + 0x94, s << 6);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x40);
            fx.mem.set16(D + 0x9A, 0x40);
            let o = fx.mem.i16(t + 0x52);
            fx.mem.set16(D + 0xBE, o);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
            a3 = 0;
        }
        for (g, local) in [(RING_N, 0x80u32), (RING_A, 0x90), (RING_B, 0xA0)] {
            if f(fx) != g {
                continue;
            }
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + local);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if f(fx) == BAND {
            for o in 0..4 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * o);
                fx.mem.write_u32(sp + 0x60 + 4 * o, w);
            }
            let before = fx.out.prims.len();
            quads4::ring_band(fx, D, t, 0x21, sp + 0x60);
            a3 = last_xyz2(fx, before);
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
        0x82 => 1,
        0xFA => 4,
        0xEB => 3,
        0x8D..=0xE5 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
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

/// The atlas cell 0x14 frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E03250(D, step, 0): `spawn::task400` on the pool at D+0x2B0 (the
/// colour D+0xB204, the kind D+0xB270): the first free task cleared and
/// set off at D+0x250; `saves` the caller's s0..s4, `ra` the return.
fn spawn(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x70 * k + 0x58) == 0) {
        let t = POOL + 0x70 * k;
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
        let s = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x50, s);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

/// 0x1E015F0(D, 0): the update counter and the spawns (frame 0x60: ra,
/// the entry's s3 = the parameter, s2 = 0, s1 = 0, s0 = D, the runner's
/// f21 / f20 (0 [H]); then s3 = D, s0..s2 the loops'). The spawns' saves:
/// s0..s2, s3 = D, the runner's s4 (0 [H]).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x50, 0x01E0_2E38), (0x40, p), (0x30, 0), (0x20, 0), (0x10, u64::from(D))]);
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let (mut s0, mut s1, mut s2) = (u64::from(D), 0u64, 0u64);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let go = |fx: &mut Fx, f: u32, ra: u32, r: [u64; 3]| spawn(fx, f, [r[0], r[1], r[2], u64::from(D), 0], ra);
    if c(fx) == 0x45 {
        let eye = fx.camera_eye(0);
        fx.mem.write_u32(EYE, eye);
        let at = fx.camera_at30(0);
        fx.mem.write_u32(EYE + 4, at);
        let x = fx.mem.f(eye);
        let near = x <= 0.0;
        let side = if fx.mem.i16(FACING) != 0 { if near { 2 } else { 0 } } else if near { 0 } else { 2 };
        fx.mem.set16(SIDE, side);
    }
    let v = c(fx);
    if (0x45..0x9C).contains(&v) && v % 2 == 1 {
        s1 = 1;
        s0 = 0;
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, POS, s0, Some(s1), 0x01E0_1790);
        for (o, ra) in [(0u32, 0x01E0_1798u32), (4, 0x01E0_17CC), (8, 0x01E0_1800)] {
            let r = rmod(fx, s0 as u32, ra, 0x28);
            let x = ee::add(fx.mem.f(POS + o), ee::from_i32(r - 0x14));
            fx.mem.setf(POS + o, x);
        }
        go(fx, QUAD, 0x01E0_183C, [s0, s1, s2]);
        s0 = 1;
    }
    let v = c(fx);
    if v > 0 && v < 0xC9 && v % 3 == 1 {
        s1 = 2;
        for k in 0..2u64 {
            s2 = k;
            s0 = u64::from(D) + 2 * k;
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2 + 2 * k as u32);
            node_position_saving(fx, h, slot, POS, s0, Some(s1), 0x01E0_18FC);
            let a = rmod(fx, s0 as u32, 0x01E0_1904, 0x28) - 0x14;
            let b = rmod(fx, s0 as u32, 0x01E0_1928, 0x28) - 0x14;
            let z = rmod(fx, s0 as u32, 0x01E0_194C, 0x28) - 0x14;
            for (o, r) in [(0u32, a), (4, b), (8, z)] {
                let q = ee::add(fx.mem.f(POS + o), ee::div(ee::from_i32(r), 5.0));
                fx.mem.setf(POS + o, q);
            }
            go(fx, RING_N, 0x01E0_19B8, [s0, s1, s2]);
        }
        s2 = 2;
        s0 = u64::from(D) + 4;
    }
    let v = c(fx);
    if v % 3 == 1 && v < 0xEB {
        go(fx, FLASH, 0x01E0_1A2C, [s0, s1, s2]);
    }
    if c(fx) == 1 {
        go(fx, FRAME, 0x01E0_1A58, [s0, s1, s2]);
        go(fx, LIGHT, 0x01E0_1A84, [s0, s1, s2]);
    }
    if c(fx) < 0xE2 {
        let h = fx.target_handle();
        unit_position_saving(fx, h, POS, s0, 0x01E0_1AB0);
        fx.mem.write_u32(POS + 4, 0x420C_0000);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, POS, s0, Some(s1), 0x01E0_1AD4);
        go(fx, RING_A, 0x01E0_1AE8, [s0, s1, s2]);
    }
    fx.leave(0x60);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// Byte +0x63 up by `up` under `cap` while `rising`, else down by `down`
/// while at least `floor` (else the task ends).
fn fade(fx: &mut Fx, t: u32, rising: bool, cap: u8, up: u8, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if rising {
        if a < cap {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(up));
        }
    } else if a >= floor {
        fx.mem.write_u8(t + 0x63, a - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E01B10 (D, task): a grey quad of size 20 (cell 0..2 stepping, the
/// orientation D+0xB260) flying from near the attacker's node 0x55 toward
/// the target (randomly offset by (-50..49) / 7 in x / y): from update 51
/// to 69 by a twentieth (z an 18.5th) of the way a update; on 70 it ends
/// with a ring of quads and a band there; in by 10 under 0x80 for 90
/// updates, out by 9. `regs` the entry's s3 / s4 (the slot, its index).
fn quad(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x14);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_1B4C);
        fx.mem.set16(t + 0x56, (v >> 4) % 3);
        let v = fx.rand_from(t, 0x01E0_1B6C);
        fx.mem.set16(t + 0x52, (v >> 4) % 2);
        let s = fx.mem.i16(SIDE);
        fx.mem.set16(t + 0x52, s);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        let sp = fx.sp;
        let h = fx.target_handle();
        unit_position_saving(fx, h, sp + 0x40, u64::from(t), 0x01E0_1BE4);
        let v = fx.rand_from(t, 0x01E0_1BEC);
        let a = ee::from_i32((v >> 4) % 100 - 0x32);
        let v = fx.rand_from(t, 0x01E0_1C14);
        let b = (v >> 4) % 100 - 0x32;
        let x = ee::add(fx.mem.f(sp + 0x40), ee::div(a, 7.0));
        fx.mem.setf(sp + 0x40, x);
        let y = ee::add(fx.mem.f(sp + 0x44), ee::div(ee::from_i32(b), 7.0));
        fx.mem.setf(sp + 0x44, y);
        for (o, by) in [(0u32, 20.0f32), (4, 20.0), (8, 18.5)] {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x40 + o), fx.mem.f(t + 0x10 + o)), by);
            fx.mem.setf(t + 0x30 + o, v);
        }
        let v = rmod(fx, t, 0x01E0_1CBC, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1CD8);
        for (o, ra) in [(4u32, 0x01E0_1CE0u32), (2, 0x01E0_1D00), (0, 0x01E0_1D20)] {
            let v = fx.rand_from(t, ra);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
        for (o, ra) in [(0x20u32, 0x01E0_1D40u32), (0x24, 0x01E0_1D5C), (0x28, 0x01E0_1D78)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    let rising = fx.mem.i16(t + 0x54) < 0x5A;
    fade(fx, t, rising, 0x80, 0xA, 0xB, 9);
    let c = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, if c >= 3 { 0 } else { c });
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1E08);
    let a = fx.mem.i16(t + 0x54);
    if (0x33..0x46).contains(&a) {
        for o in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(t + 0x30 + o));
            fx.mem.setf(t + 0x10 + o, v);
        }
    } else if a == 0x46 {
        fx.mem.set16(t + 0x58, 0);
        for k in 0..4 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        // The spawns' saves: s0 = the task, s1 = D, the entry's s2 = 0,
        // s3 = the slot, s4 = its index.
        let saves = [u64::from(t), u64::from(D), 0, u64::from(regs[0]), u64::from(regs[1])];
        spawn(fx, RING_B, saves, 0x01E0_1E9C);
        spawn(fx, BAND, saves, 0x01E0_1EB0);
    }
    age(fx, t);
}

/// 0x1E01EE0 (`RING_N`: size 6, radius 20..119 (10..109 from update 70)),
/// 0x1E020F0 (`RING_A`: size 6, radius 10..59), 0x1E022B0 (`RING_B`: size
/// 30, radius 5..14 growing by 20..59) (D, task): a white ring of quads
/// with random angles: cell 0..3, a random angle, growth 1..3 (only
/// RING_B grows), row 1..8, life 2..4; in by 0x2A under 0x80, out by 11.
fn ring(fx: &mut Fx, t: u32, f: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let size = match f {
            RING_B => 0x41F0_0000u32,
            _ => 0x40C0_0000,
        };
        fx.mem.write_u32(t + 0x48, size);
        // RING_N's frame is 0x30: its sites 8 further on.
        let b = if f == RING_N { 8 } else { 0 };
        let v = fx.rand_from(t, f + 0x28 + b);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        for (o, at) in [(0x20u32, 0x5Cu32), (0x24, 0x78), (0x28, 0x94)] {
            let v = rmod(fx, t, f + at + b, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        // RING_N's later sites 0x48 further on (its radius has two
        // branches).
        let (r, o) = match f {
            RING_N => {
                let late = fx.mem.i16(COUNTER) >= 0x46;
                let r = if late { rmod(fx, t, 0x01E0_1FD4, 0x64) + 0xA } else { rmod(fx, t, 0x01E0_1FAC, 0x64) + 0x14 };
                (r, 0x118u32)
            }
            RING_A => (rmod(fx, t, f + 0xB0, 0x32) + 0xA, 0xD0),
            _ => (rmod(fx, t, f + 0xB0, 0xA) + 5, 0xD0),
        };
        fx.mem.set16(t + 0x50, r);
        let v = rmod(fx, t, f + o, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = if f == RING_B { rmod(fx, t, f + o + 0x1C, 0x28) + 0x14 } else { rmod(fx, t, f + o + 0x1C, 3) + 1 };
        fx.mem.set16(t + 0xE, v);
        let v = fx.rand_from(t, f + o + 0x3C);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, f + o + 0x5C, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, rising, 0x80, 0x2A, 0xD, 0xB);
    if f == RING_B && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02490 (D, task): a band (inner radius 50 growing by 1; grey inner,
/// green outer) facing the camera turned a quarter; in by 0x40 under 0xC0
/// for 4 updates, out by 12. `flashes` the entry's s1.
fn band(fx: &mut Fx, t: u32, flashes: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4248_0000);
        l2::rgb(fx, t + 0x60, 0xCF, 0xCF, 0xCF);
        l2::rgb(fx, t + 0x64, 8, 0x80, 8);
        let v = rmod(fx, t, 0x01E0_24E4, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 4;
    fade(fx, t, rising, 0xC0, 0x40, 0xE, 0xC);
    let r = ee::add(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, r);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_2570);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E025A0 (D, task): the stage light (0x1F7F10) down by 10 to 0x70
/// while the timer is at least 26, then up by 10 to 0x80 (the task ending
/// once there); 0x80 and the task's end when the timer is at 3.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let l = fx.mem.i16(t + 0x50);
    if fx.mem.i32(TIMER) >= 0x1A {
        fx.mem.set16(t + 0x50, if l < 0x71 { 0x70 } else { l - 0xA });
    } else {
        if l < 0x80 {
            fx.mem.set16(t + 0x50, l + 0xA);
        } else {
            fx.mem.set16(t + 0x50, 0x80);
            fx.mem.set16(t + 0x58, 0);
        }
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

/// 0x1E026A0 (D, task): a full-window frame quad (colour 0x78 / 0xA0 /
/// 0x78, the task's alpha, ALPHA 0x44; b232's 0x1E00F80 with the UVs
/// inset by +0x48 (10) over 512) fading in by 10 to 0x50 until update
/// 235, then out by 4.
fn frame_quad(fx: &mut Fx, sp: u32, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x28;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.mem.read_u8(t + 0x63);
    for (k, v) in [0x78u8, 0xA0, 0x78, a].into_iter().enumerate() {
        fx.mem.write_u8(d + 0x10 + k as u32, v);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, xy) in [(0x20u32, 0x6FF8u16), (0x30, 0x8FF8)] {
        fx.mem.write_u16(d + at, xy);
        fx.mem.write_u16(d + at + 2, xy);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    let w = fx.mem.f(t + 0x48);
    let lo = ee::div(w, 512.0);
    fx.mem.setf(d + 0x18, lo);
    let lo = ee::div(fx.mem.f(t + 0x48), 512.0);
    fx.mem.setf(d + 0x1C, lo);
    let hi = ee::div(ee::sub(512.0, fx.mem.f(t + 0x48)), 512.0);
    fx.mem.setf(d + 0x28, hi);
    let hi = ee::div(ee::sub(512.0, fx.mem.f(t + 0x48)), 512.0);
    fx.mem.setf(d + 0x2C, hi);
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    // After 0x207E20: a2 = the flags | 0x200, a3 = the flags.
    fx.frame_desc(d, 1, [0x226, 0x26]);
    // 0x207FC0's `sq ra` upper half (0) under 0x207EF0 (as b232).
    fx.mem.write_u64(sp - 0x28, 0);
    fx.submit(d, OT, 0);
    let rising = fx.mem.i16(t + 0x54) < 0xEB;
    fade(fx, t, rising, 0x50, 0xA, 6, 4);
    age(fx, t);
}

/// 0x1E02880(D, t): b482's flash step (the target over the window, UVs
/// inset by +8 / 10 and +0xA / 10 over 512, into the descriptor at
/// +0xB218, no ordering table) in dark grey (0x28), the inset growing by
/// 55 a update: alpha up by 0x2A below 0x80 for 3 updates, from update 5
/// down by 12. `regs` the unit flash's s0 / s1 (the slot index, the live
/// count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_34EC), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f20 / f21: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x28, 0x28, 0x28);
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
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let v = fx.mem.i16(t + o) + 0x37;
            fx.mem.set16(t + o, v);
        }
    }
    let a = fx.mem.i16(t + 0x54);
    if a < 3 {
        let v = fx.mem.read_u8(t + 0x63);
        if v < 0x80 {
            fx.mem.write_u8(t + 0x63, v.wrapping_add(0x2A));
        }
    } else if a >= 5 {
        let v = fx.mem.read_u8(t + 0x63);
        if v >= 0xE {
            fx.mem.write_u8(t + 0x63, v - 0xC);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E033E0(D) (d1cf0abc65df at other offsets, b482's 0x1E03DF0): in a
/// packet layer (0x2078A0) the attacker drawn into the target (0x1F8270
/// filter 0; ALPHA 0x48), TEXFLUSH before and after; every flash task
/// stepped (0x1E02880); the layer linked into the ordering table at key 0
/// when any task was alive.
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
        let t = POOL + 0x70 * k;
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
