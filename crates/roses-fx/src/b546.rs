//! BMODULE b546 (card 661), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E021D0, data area 0x1ED0000) [F] (helper C7, 31
//! Oct 2026): a pool of 450 tasks (0x70 bytes at +0x2A0, alive +0x58,
//! step +0x44 called through `jalr`) drawn with the second library. On
//! update 1 a whole-window unit flash (0x1E01F90: the attacker drawn into
//! the target through 0x1F8270 in a packet layer, `Fx::unit_draw`, then a
//! frame sprite) and the stage light (0x1E01ED0, dimming to 0x20 then
//! back); on updates 0x3F..0xC7 three motes a update leave the attacker's
//! node 0x10 (0x1E01730, `quads4::quad_turned`), each bursting on its
//! 30th..34th update into a ring of quads (0x1E01B90,
//! `billboard::ring_quads`) and an arc band (0x1E01D80,
//! `billboard::arc_strips`). Returns 1 on update 0x7C, 4 on 0x104, 2 on
//! 0xFA, 6 on 0x87..0xF4, 8 once the 270-update timer +0xC7F0 has run out.
//!
//! Library at its own addresses: uv_rect (0x1E00080), `ring_quads`
//! (0x1E00920), `quad_turned` (0x1E00ED0), `arc_strips` (0x1E011B0).
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0x1C2;
/// The step functions a task holds (+0x44), as the record's addresses.
const MOTE: u32 = 0x01E0_1730;
const RING: u32 = 0x01E0_1B90;
const ARC: u32 = 0x01E0_1D80;
const LIGHT: u32 = 0x01E0_1ED0;
const FLASH: u32 = 0x01E0_1F90;
/// The return address of the entry's `jalr` into a step (0x1E02540 + 8).
const TASK_RA: u64 = 0x01E0_2548;
const TIMER: u32 = D + 0xC7F0;
const COLOUR: u32 = D + 0xC7F4;
const FLASH_DESC: u32 = D + 0xC810;
const COUNTER: u32 = D + 0xC866;
const ANGLE_FLAG: u32 = D + 0xC85E;
const NODES: u32 = D + 0xC830;
const BYTES2: u32 = D + 0xC968;

/// The entry 0x1E021D0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let e = fx.camera_eye(0);
    fx.mem.write_u32(D + 0xC804, e);
    let a = fx.camera_at30(0);
    fx.mem.write_u32(D + 0xC808, a);
    fx.mem.write_u32(D + 0xC7FC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xCBE0);
    l2::sine_table(fx, D + 0xCA68);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xC864, param);
    fx.mem.set32(TIMER, 0x10E);
    for (k, n) in [0x10, 0x1E, 1, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xC840, r);
    let tex = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, tex);
    for k in 0..2u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xC7D0 + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    fx.fix_residue(v, 0x01E0_23F0);
    fx.mem.set16(D + 0xC862, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for (table, ra) in [(D + 0xC868, 0x01E0_2420u32), (BYTES2, 0x01E0_2474)] {
        for k in 0..0x100u32 {
            let r = fx.rand_from(D, ra);
            let (_, h) = fx.div(r, 0x28);
            fx.mem.write_u8(table + k, (h - 0x14) as u8);
        }
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(BYTES2 + k, 0);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    // s1 counts the flash tasks (skipped here, run by the unit flash).
    let mut flashes = 0i32;
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
            flashes += 1;
            continue;
        }
        let regs = [D, flashes as u32, 0, slot, k];
        run_task(fx, f, slot + 0x2A0, &regs);
    }
    let mut a3 = 0u32;
    let last_value = |fx: &mut Fx, before: usize| match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => 0,
    };
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == MOTE {
            cell(fx, t, D + 0xC7D0, sp + 0x60);
            fx.mem.set16(D + 0xBE, 0);
            quads4::quad_turned(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
            a3 = 0;
        }
        if fx.mem.read_u32(t + 0x44) == RING {
            cell(fx, t, D + 0xC7D4, sp + 0x70);
            fx.mem.set16(D + 0xB8, 3);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, D + 0xC868);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x12);
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            a3 = last_value(fx, before);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    if flashes != 0 {
        // a3 at the flash's first 0x207BB0 = the a3 the draws' last
        // library call left [run, traced]: 0 after a quad or a ring (their
        // projections' 0x20C670 loop counts a3 down to 0), after an arc the
        // last XYZ2 its far-Z pass (0x109C60) loaded (0 when it drew
        // nothing).
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x7C => 1,
        0x104 => 4,
        0xFA => 2,
        0x87..=0xF4 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The draw's head: texture `tex` into D+0x88, the atlas cell 0x14 frame
/// +0x56 (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, tex: u32, r: u32) {
    let w = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, w);
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// A step called through the entry's `jalr`; `regs` its s0..s4 (D, the
/// flash count, 0, the slot, its index).
fn run_task(fx: &mut Fx, f: u32, t: u32, regs: &[u32; 5]) {
    match f {
        MOTE => mote(fx, t, regs),
        RING => ring(fx, t, regs),
        ARC => arc(fx, t, regs),
        LIGHT => light(fx, t),
        _ => panic!("b546: unknown task step {f:#x}"),
    }
}

/// 0x1E01510(D, 0): the update counter +0xC866 and the emitters.
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_2508), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let regs = [D, 0, 0, u32::MAX, 0];
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, FLASH, 0, &regs, 0x01E0_1560);
        fx.mem.set16(ANGLE_FLAG, 0);
        fx.mem.set16(ANGLE_FLAG, 1);
        spawn(fx, LIGHT, 0, &regs, 0x01E0_1590);
    }
    if (0x3F..0xC8).contains(&fx.mem.i16(COUNTER)) {
        for (site, ra) in [(0x01E0_15DCu32, 0x01E0_162Cu32), (0x01E0_1654, 0x01E0_16A4), (0x01E0_16CC, 0x01E0_171C)] {
            let h = fx.self_handle();
            let s = fx.mem.i16(NODES);
            fx.node_position(h, s, D + 0x250);
            let r = fx.rand_from(D, site);
            let (_, m) = fx.div(r >> 4, 0x14);
            let y = ee::add(fx.mem.f(D + 0x254), ee::div(ee::from_i32(m - 0xA), 10.0));
            fx.mem.setf(D + 0x254, y);
            spawn(fx, MOTE, 0, &regs, ra);
        }
    }
    fx.leave(0x20);
}

/// 0x1E027E0(D, step, arg): the first free task cleared and set off at
/// D+0x250 with the colour +0xC7F4 (twice), angle flag +0xC85E; `regs` the
/// caller's s0..s4 (u32::MAX: 64-bit all ones), `ra` the return address.
fn spawn(fx: &mut Fx, f: u32, arg: u32, regs: &[u32; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    let r = |k: usize| if regs[k] == u32::MAX { u64::MAX } else { u64::from(regs[k]) };
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, r(4)), (0x30, r(3)), (0x20, r(2)), (0x10, r(1)), (0, r(0))]);
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
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(ANGLE_FLAG);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, arg);
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

/// 0x205580(t + 0x20) from a step (its saves below the step's frame: ra,
/// the step's s1 = D, s0 = t).
fn angles(fx: &mut Fx, t: u32, ra: u32) {
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, u64::from(ra)), (0x10, u64::from(D)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
}

/// The alpha byte `a` up by `up` while below `max` before the life
/// `life`, then down by `down` while at least `min`, else the task ends.
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

/// 0x1E01730: a mote of size 5 growing to 15, drifting by a random
/// velocity / 150 (x, y) and toward the target (z / 30), y jittered,
/// turning, fading in for 0x28 updates then out by 9, flickering grey; on
/// its update +0x52 (30..34) a ring and an arc at its position.
fn mote(fx: &mut Fx, t: u32, regs: &[u32; 5]) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(regs[1])), (0, u64::from(regs[0]))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 5);
        fx.mem.set16(t + 0xA, 5);
        fx.mem.set16(t + 0x56, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        let v = rand_mod(fx, t, 0x01E0_1784, 0x64);
        fx.mem.setf(t + 0x34, ee::from_i32(v - 0x32));
        let v = rand_mod(fx, t, 0x01E0_17B0, 0x64);
        fx.mem.setf(t + 0x30, ee::from_i32(v - 0x32));
        for o in [0x34u32, 0x30] {
            let v = ee::div(fx.mem.f(t + o), 150.0);
            fx.mem.setf(t + o, v);
        }
        let v = rand_mod(fx, t, 0x01E0_17FC, 0x50);
        fx.mem.setf(t + 0x3C, ee::div(ee::from_i32(v - 0x28), 10.0));
        fx.mem.write_u32(t + 0x38, 0x400C_CCCD);
        let r = fx.rand_from(t, 0x01E0_1848);
        let (_, v) = fx.div(r >> 4, 0x32);
        let y = ee::add(fx.mem.f(t + 0x14), ee::div(ee::from_i32(v - 0x19), 20.0));
        fx.mem.setf(t + 0x14, y);
        let v = rand_mod(fx, t, 0x01E0_1888, 0x168);
        fx.mem.set16(t + 0x5C, v);
        angles(fx, t, 0x01E0_18A4);
        for (o, ra) in [(4u32, 0x01E0_18ACu32), (2, 0x01E0_18CC), (0, 0x01E0_18EC)] {
            let r = fx.rand_from(t, ra);
            fx.mem.set16(t + o, r.wrapping_rem(0x800) - 0x400);
        }
        let v = rand_mod(fx, t, 0x01E0_190C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let r = fx.rand_from(t, 0x01E0_1928);
        fx.mem.set32(t + 0x20, r.wrapping_rem(0x4000) - 0x2000);
        fx.mem.set32(t + 0x24, 0x4000);
        let r = fx.rand_from(t, 0x01E0_1950);
        let (_, v) = fx.div(r >> 4, 5);
        fx.mem.set16(t + 0x52, v + 0x1E);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        fx.node_position(h, s, sp + 0x30);
        let tg = fx.target_handle();
        fx.unit_position(tg, sp + 0x40);
        let dz = ee::div(ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(sp + 0x38)), 30.0);
        fx.mem.setf(t + 0x38, dz);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x28;
    fade(fx, t, 0x63, rising, (0x80, 0x40), (0xB, 9));
    if fx.mem.i16(t + 0x54) != 0 {
        let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x38));
        fx.mem.setf(t + 0x18, z);
        let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
        fx.mem.setf(t + 0x10, x);
        let y = ee::add(fx.mem.f(t + 0x14), ee::sub(fx.mem.f(t + 0x34), f32::from_bits(0x3DCC_CCCD)));
        fx.mem.setf(t + 0x14, y);
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s < 0xF {
                fx.mem.set16(t + o, s + 1);
            }
        }
    }
    if fx.mem.i16(t + 0x54) == fx.mem.i16(t + 0x52) {
        for k in 0..4 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        let caller = [t, D, regs[2], regs[3], regs[4]];
        spawn(fx, RING, 0, &caller, 0x01E0_1AD0);
        spawn(fx, ARC, 0, &caller, 0x01E0_1AE4);
        fx.mem.set16(t + 0x58, 0);
    }
    let g = if odd(fx.mem.i16(t + 0x54)) { 0x80 } else { 0x70 };
    l2::rgb(fx, t + 0x60, g, g, g);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if fx.mem.i16(COUNTER) >= 0xFB {
        fx.mem.set16(t + 0x54, 0x5A);
    }
    let a = fx.mem.i16(t + 0x5A) + 0x28;
    fx.mem.set16(t + 0x5A, a);
    fx.leave(0x50);
}

/// 0x1E01B90: a ring of quads (size 8, cell frame 4) of radius 10..29
/// growing by +0xE (4..10) a update, row +8 1..8, life +6 20..22; bright
/// before its life, then fading by 0xB; flickering red / white.
fn ring(fx: &mut Fx, t: u32, regs: &[u32; 5]) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(regs[1])), (0, u64::from(regs[0]))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4100_0000);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0x60, 0x60);
        let v = rand_mod(fx, t, 0x01E0_1BE0, 0x14);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rand_mod(fx, t, 0x01E0_1C00, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_1C1C, 7);
        fx.mem.set16(t + 0xE, v + 4);
        let r = fx.rand_from(t, 0x01E0_1C3C);
        fx.mem.set16(t + 8, r.wrapping_rem(8) + 1);
        let v = rand_mod(fx, t, 0x01E0_1C5C, 3);
        fx.mem.set16(t + 6, v + 0x14);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0xD, 0xB));
    }
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    } else {
        l2::rgb(fx, t + 0x60, 0x80, 0x60, 0x60);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    if fx.mem.i32(TIMER) < 0x23 {
        fx.mem.set16(t + 0x54, 0x1E);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E01D80: an arc band (inner radius 0, outer 80, steps 20 degrees)
/// turning by 0x4000 a update, its outer alpha rising by 0x50 for 4
/// updates then falling by 0x13; flickering yellow.
fn arc(fx: &mut Fx, t: u32, regs: &[u32; 5]) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(regs[0]))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x64, 0xC0, 0x40, 0x40);
        fx.mem.set16(t + 6, 4);
        fx.mem.write_u32(t + 0x4C, 0x42A0_0000);
        fx.mem.set16(t + 0x5A, 0x14);
        fx.mem.set16(t + 0x5C, 0x14);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, 0x67, rising, (0xA0, 0x50), (0x15, 0x13));
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0x80);
    } else {
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x60);
    }
    angles(fx, t, 0x01E0_1EA0);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01ED0: the stage light +0x50 from 0x80 down by 10 to 0x20, then
/// (timer below 0x15) back up by 10 to 0x80, the task ending there.
fn light(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x10);
    fx.saved(sp, &[(0, TASK_RA)]);
    if fx.mem.i16(t + 0x58) == 1 {
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

/// 0x1E01F90: the flash's frame sprite (the target over the window, the
/// colour +0x60) into the descriptor at +0xC810, no ordering table; alpha
/// rising by 0x12 below 0x70 until update 0xF0, then falling by 9.
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_2A80), (0x10, u64::from(regs[1])), (0, u64::from(regs[0]))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
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
    for (o, v) in [(0x18u32, 0u32), (0x1C, 0), (0x28, 0x3F80_0000), (0x2C, 0x3F80_0000)] {
        fx.mem.write_u32(d + o, v);
    }
    fx.submit(d, 0, 0);
    let rising = fx.mem.i16(t + 0x54) < 0xF0;
    fade(fx, t, 0x63, rising, (0x70, 0x12), (0xB, 9));
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E02970(D): in a packet layer (0x2078A0) the attacker drawn into the
/// target (0x1F8270 filter 0, its TEX0 / TEX1 into the sprite descriptor
/// at sp + 0x58; ALPHA 0x48), TEXFLUSH before and after; every flash task
/// stepped (0x1E01F90); the layer linked into the ordering table at key 0
/// when one ran.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xB0);
    let d = sp + 0x58;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(D + 0xC80C, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    // a0..a3 at the first 0x207BB0: 5, the descriptor, 0x26, the caller's.
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x98, sp + 0xA0);
    fx.texflush(regs);
    let mut ran = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            ran += 1;
            flash(fx, slot + 0x2A0, [k, ran]);
        }
    }
    if ran != 0 {
        let layer = fx.mem.read_u32(D + 0xC80C);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}
