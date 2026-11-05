//! BMODULE b434 (card 579), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02560, data area 0x1ED0000) [F] (helper A12, 31
//! Oct 2026): b422's pool and entry shape (400 tasks of 0x70 bytes at
//! D+0x2A0, alive +0x58, step +0x44 through `jalr`, 0x1E02550 an idle
//! step; spawned by 0x1E02C70 = `spawn::task400`) with its own emitter and
//! steps (the local 0x1E01190, dd92b9aa4fca, 1268 words): drifting grey
//! quads (add5d81dd3fe at 0x1E00ED0, `billboard::quad`; the drift +0x30 x
//! i16 +0x52 added to the point), rising quads, two kinds of rings
//! (7369422df7f8 at 0x1E00920, `billboard::ring_quads`, 12 or 4 rows on
//! the byte tables D+0xB3BA / D+0xB2BA) and a full-window blue-grey frame
//! quad (b232's 0x1E00F80 at other offsets). The attacker's node 0x42 is
//! read every update; 0x5B..0x77 drifting quads above it (and four
//! discarded random offsets), 0xD1..0x103 rings on odd updates, 0xDD..0x13F
//! three rising quads a update, 0x105..0x167 rings at the target every
//! fifth update, 0x56 the frame quad. Returns 1 on update 250, 4 on 370, 2
//! on 360, 6 on 261..354, 8 once the 380-update timer +0xB210 has run out.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, spawn::task400, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const DRIFT: u32 = 0x01E0_1A50;
const RISE: u32 = 0x01E0_1D30;
const RING12: u32 = 0x01E0_2060;
const RING4: u32 = 0x01E0_2210;
const FRAME: u32 = 0x01E0_23A0;
const IDLE: u32 = 0x01E0_2550;
/// The return address of the entry's `jalr` into a step (0x1E028B8 + 8).
const TASK_RA: u64 = 0x01E0_28C0;
const POS: u32 = D + 0x250;
const AIM: u32 = D + 0xB1D0;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const NODES: u32 = D + 0xB244;
const TARGET_NODE: u32 = D + 0xB254;
const FACING: u32 = D + 0xB2B4;
const COUNTER: u32 = D + 0xB2B8;
const BYTES4: u32 = D + 0xB2BA;
const BYTES12: u32 = D + 0xB3BA;
const SINES: u32 = D + 0xB4BC;

/// rand % m (HI) from a `jal` returning to `ra` (rand's frame: s0 = `s0`).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02560.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x17C);
    for (k, node) in [0x42, 0x34, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x298 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_2750);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_2780, 0x1E);
        fx.mem.set8(BYTES4 + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_27D4, 0x28);
        fx.mem.set8(BYTES12 + k, v - 0x14);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_2824);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        // The step's prologue: the entry's s0 = 0, s1 = D.
        let frame = match f {
            0 | IDLE => continue,
            DRIFT | RISE => 0x30,
            RING12 | RING4 => 0x20,
            FRAME => 0x90,
            _ => panic!("b434: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        }
        match f {
            DRIFT => drift(fx, t),
            RISE => rise(fx, t),
            RING12 => ring(fx, t, true),
            RING4 => ring(fx, t, false),
            _ => frame_quad(fx, s, t),
        }
        fx.leave(frame);
    }
    // The entry's s0: the live tasks counted in the draw loop.
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        let sub = |fx: &mut Fx| fx.mem.i16(t + 0x56);
        if fx.mem.read_u32(t + 0x44) == DRIFT {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let s = sub(fx);
            crate::b615::cell(fx, sp + 0x60, 0x14, s);
            fx.mem.set16(D + 0xBE, 0);
            let n = ee::from_i32(fx.mem.i16(t + 0x52));
            for o in 0..3 {
                let v = ee::add(fx.mem.f(t + 0x10 + 4 * o), ee::mul(fx.mem.f(t + 0x30 + 4 * o), n));
                fx.mem.setf(sp + 0x50 + 4 * o, v);
            }
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
        }
        if fx.mem.read_u32(t + 0x44) == RISE {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let s = sub(fx);
            crate::b615::cell(fx, sp + 0x70, 0x14, s);
            fx.mem.set16(D + 0xBE, 0);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        for (f, at, rows, ra, bytes) in [(RING12, 0x80u32, 0xC, 0x01E0_2AA0u32, BYTES12), (RING4, 0x90, 4, 0x01E0_2B40, BYTES4)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            let s = sub(fx);
            crate::b615::cell(fx, sp + at, 0x14, s);
            fx.mem.set16(D + 0xB8, rows);
            let v = fx.rand_from(live, ra);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, bytes);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xFA => 1,
        0x172 => 4,
        0x168 => 2,
        0x105..=0x162 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E01190(D, 0): the update counter and the spawns (frame 0x30: ra, the
/// entry's s0 = 0, the runner's f20 (0 [H]); s0 = D). The spawns' saves:
/// the runner's s4 / s3 (0 [H]), the entry's s2 = the parameter, s1 = D,
/// the emitter's s0 = D.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_2880), (0x10, 0)]);
    fx.mem.write_u32(sp, 0);
    let p = param as i64 as u64;
    let go = |fx: &mut Fx, f: u32, ra: u32| {
        task400(fx, D, f, 0, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, u64::from(D)), (0, u64::from(D))]);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let node = |fx: &mut Fx, h: i32, slot: u32, out: u32, ra: u32| {
        let s = fx.mem.i16(slot);
        node_position_saving(fx, h, s, out, u64::from(D), Some(u64::from(D)), ra);
    };
    let aim_to_pos = |fx: &mut Fx| {
        for k in 0..4 {
            let w = fx.mem.read_u32(AIM + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    };
    let r = |fx: &mut Fx, ra: u32, m: i32, off: i32| ee::from_i32(rmod(fx, D, ra, m) - off);
    // A point 60 above the aim, (-25..24) / 20 across in x and y, `dz`
    // toward the facing.
    let above = |fx: &mut Fx, ras: [u32; 2], dz: f32| {
        aim_to_pos(fx);
        let a = r(fx, ras[0], 0x32, 0x19);
        let b = r(fx, ras[1], 0x32, 0x19);
        fx.mem.write_u32(POS + 4, 0x4270_0000);
        let a = ee::div(a, 20.0);
        fx.mem.write_u32(POS, 0);
        fx.mem.setf(POS, ee::add(0.0, a));
        let y = ee::add(fx.mem.f(POS + 4), ee::div(b, 20.0));
        fx.mem.setf(POS + 4, y);
        let z = fx.mem.f(POS + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, dz) } else { ee::add(z, dz) };
        fx.mem.setf(POS + 8, z);
    };
    let jitter = |fx: &mut Fx, o: u32, ra: u32, m: i32, off: i32| {
        let v = r(fx, ra, m, off);
        let x = ee::add(fx.mem.f(POS + o), v);
        fx.mem.setf(POS + o, x);
    };
    let h = fx.self_handle();
    node(fx, h, NODES, AIM, 0x01E0_11E0);
    if (0x97..0x160).contains(&c(fx)) {
        fx.mem.write_u32(AIM, 0);
        aim_to_pos(fx);
    }
    if (0x5B..0x78).contains(&c(fx)) {
        above(fx, [0x01E0_1288, 0x01E0_12AC], 5.0);
        go(fx, DRIFT, 0x01E0_1354);
        aim_to_pos(fx);
        jitter(fx, 0, 0x01E0_1380, 0x1E, 0xF);
        jitter(fx, 4, 0x01E0_13B4, 0x1E, 0xF);
        aim_to_pos(fx);
        fx.mem.write_u32(POS + 4, 0x4248_0000);
        jitter(fx, 0, 0x01E0_1414, 0x1E, 0xF);
        jitter(fx, 4, 0x01E0_1448, 0x1E, 0xF);
    }
    if (0xD1..0x104).contains(&c(fx)) {
        above(fx, [0x01E0_14C8, 0x01E0_14EC], 6.0);
        if c(fx) % 2 == 1 {
            go(fx, RING4, 0x01E0_15C0);
        }
    }
    if (0xDD..0x140).contains(&c(fx)) {
        above(fx, [0x01E0_1610, 0x01E0_1634], 6.0);
        for ra in [0x01E0_16DC, 0x01E0_16F0, 0x01E0_1704] {
            go(fx, RISE, ra);
        }
    }
    if (0x105..0x168).contains(&c(fx)) {
        for (nra, ras, gra) in [
            (0x01E0_1748u32, [0x01E0_1750u32, 0x01E0_1784, 0x01E0_17B8], 0x01E0_181Cu32),
            (0x01E0_183C, [0x01E0_1844, 0x01E0_1878, 0x01E0_18AC], 0x01E0_1910),
            (0x01E0_1930, [0x01E0_1938, 0x01E0_196C, 0x01E0_19A0], 0x01E0_1A08),
        ] {
            let h = fx.target_handle();
            node(fx, h, TARGET_NODE, POS, nra);
            for (o, ra) in [0u32, 4, 8].into_iter().zip(ras) {
                jitter(fx, o, ra, 0x14, 0xA);
            }
            if c(fx) % 5 == 1 {
                go(fx, RING12, gra);
            }
        }
    }
    if c(fx) == 0x56 {
        go(fx, FRAME, 0x01E0_1A34);
    }
    fx.leave(0x30);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// Byte +0x63 up by `up` while under `cap` and `rising`, else down by
/// `down` while at least `floor` (else the task ends).
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

/// Grey 0x80 on odd updates, 0x70 on even ones; the cell row +0x56 one on
/// (mod 8).
fn flicker(fx: &mut Fx, t: u32) {
    let g = if fx.mem.i16(t + 0x54) % 2 == 1 { 0x80 } else { 0x70 };
    l2::rgb(fx, t + 0x60, g, g, g);
    let s = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, if s >= 8 { 0 } else { s });
}

/// 0x1E01A50 (D, task): a grey quad of size 2..7 growing by one every
/// third update to 16, a random cell row, a drift (-25..24) / 55 in x / y
/// and / 80 in z scaled by +0x52 (25, down by one a update from update 187
/// while at least 6) at draw time; in by 0xC under 0x80 for 190 updates,
/// out by 9; turned by the camera's angles.
fn drift(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_1A7C, 6);
        fx.mem.set16(t + 8, v + 2);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_1AA4);
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_1AE0u32), (0x30, 0x01E0_1B0C), (0x38, 0x01E0_1B38)] {
            let v = rmod(fx, t, ra, 0x32);
            fx.mem.setf(t + o, ee::from_i32(v - 0x19));
        }
        for (o, by) in [(0x34u32, 55.0f32), (0x30, 55.0), (0x38, 80.0)] {
            let q = ee::div(fx.mem.f(t + o), by);
            fx.mem.setf(t + o, q);
        }
        fx.mem.set16(t + 0x52, 0x19);
        let v = rmod(fx, t, 0x01E0_1BA0, 0x168);
        fx.mem.set16(t + 0x5C, v);
    }
    let rising = fx.mem.i16(t + 0x54) < 0xBE;
    fade(fx, t, rising, 0x80, 0xC, 0xB, 9);
    flicker(fx, t);
    if fx.mem.i16(t + 0x54) % 3 == 1 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s < 0x10 {
                fx.mem.set16(t + o, s + 1);
            }
        }
    }
    if fx.mem.i16(COUNTER) >= 0xBB {
        let n = fx.mem.i16(t + 0x52);
        if n >= 6 {
            fx.mem.set16(t + 0x52, n - 1);
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1D08);
    age(fx, t);
}

/// 0x1E01D30 (D, task): a grey quad 8..14 x 5 widening by one every third
/// update to 35, a random cell row, rising: 3.8 a update toward the facing
/// in z, every third update (-50..49) / 100 in x and that less 0.8 in y;
/// in by 0x40 under 0x80 for 85 updates, out by 9; its age held at 90
/// from update 361.
fn rise(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_1D5C, 7);
        fx.mem.set16(t + 8, v + 8);
        fx.mem.set16(t + 0xA, 5);
        let v = fx.rand_from(t, 0x01E0_1D84);
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_1DC0u32), (0x30, 0x01E0_1DEC)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30] {
            let q = ee::div(fx.mem.f(t + o), 100.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_1E38, 0x168);
        fx.mem.set16(t + 0x5C, v);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x55;
    fade(fx, t, rising, 0x80, 0x40, 0xB, 9);
    flicker(fx, t);
    if fx.mem.i16(t + 0x54) % 3 == 1 {
        let s = fx.mem.i16(t + 8);
        if s < 0x23 {
            fx.mem.set16(t + 8, s + 1);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let up = f32::from_bits(0x4073_3333);
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, up) } else { ee::add(z, up) };
        fx.mem.setf(t + 0x18, z);
        if fx.mem.i16(t + 0x54) % 3 == 1 {
            let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
            fx.mem.setf(t + 0x10, x);
            let y = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
            fx.mem.setf(t + 0x14, ee::sub(y, f32::from_bits(0x3F4C_CCCD)));
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2010);
    age(fx, t);
    if fx.mem.i16(COUNTER) >= 0x169 {
        fx.mem.set16(t + 0x54, 0x5A);
    }
}

/// 0x1E02060 (`wide`) / 0x1E02210 (D, task): a grey ring of quads: size
/// 4..10 (1..4), cell row 0..2, radius 30..59 (10..69), a random angle,
/// growth 16..35 a update (only the first grows), row 0..34, life 3..5;
/// in by 0x40 under 0xC0, out by 14.
fn ring(fx: &mut Fx, t: u32, wide: bool) {
    let ras: [u32; 7] = if wide {
        [0x01E0_2084, 0x01E0_20B0, 0x01E0_20DC, 0x01E0_20FC, 0x01E0_2118, 0x01E0_2138, 0x01E0_2154]
    } else {
        [0x01E0_2234, 0x01E0_2260, 0x01E0_228C, 0x01E0_22AC, 0x01E0_22C8, 0x01E0_22E8, 0x01E0_2304]
    };
    if fx.mem.i16(t + 0x58) == 1 {
        let s = if wide { rmod(fx, t, ras[0], 7) + 4 } else { rmod(fx, t, ras[0], 4) + 1 };
        fx.mem.setf(t + 0x48, ee::from_i32(s));
        let v = rmod(fx, t, ras[1], 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = if wide { rmod(fx, t, ras[2], 0x1E) + 0x1E } else { rmod(fx, t, ras[2], 0x3C) + 0xA };
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, t, ras[3], 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, ras[4], 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, ras[5], 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, ras[6], 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, rising, 0xC0, 0x40, 0x10, 0xE);
    if wide && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E023A0 (D, task): a full-window frame quad (colour 0x78 / 0x78 /
/// 0xA0, the task's alpha, ALPHA 0x44; b232's 0x1E00F80 with the
/// descriptor at sp + 0x38) fading in by 6 to 0x48 until update 360, then
/// out by 6.
fn frame_quad(fx: &mut Fx, sp: u32, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.mem.read_u8(t + 0x63);
    for (k, v) in [0x78u8, 0x78, 0xA0, a].into_iter().enumerate() {
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
    for (o, v) in [(0x18u32, 0u32), (0x1C, 0), (0x28, 0x3F80_0000), (0x2C, 0x3F80_0000)] {
        fx.mem.write_u32(d + o, v);
    }
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    // After 0x207E20: a2 = the flags | 0x200, a3 = the flags.
    fx.frame_desc(d, 1, [0x226, 0x26]);
    // 0x207FC0's `sq ra` upper half (0) under 0x207EF0 (as b232).
    fx.mem.write_u64(sp - 0x28, 0);
    fx.submit(d, OT, 0);
    let rising = fx.mem.i16(COUNTER) < 0x168;
    fade(fx, t, rising, 0x48, 6, 8, 6);
    age(fx, t);
}
