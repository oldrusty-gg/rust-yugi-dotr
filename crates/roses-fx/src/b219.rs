//! BMODULE b219 (card 69), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02920, data area 0x1ED0000) [F]: b082's layout (a pool of 150 tasks of 0x70 bytes at D+0x2A0,
//! D+0x4Cxx counters) with b547's quads / band steps, drawn with bands
//! (853ed233d9de at 0x1E00920, `quads4::ring_band_fy`; 3fb2be4df9d4 at
//! 0x1E00D80, `quads4::ring_band`) and rings of quads (7369422df7f8 at
//! 0x1E01730, `billboard::ring_quads`). On updates 64..200 one widening
//! band a update leaves the attacker's node 0x37 (3 to the facing); at its
//! age 23 (it ages by 2) it throws, at a point within the target's radius
//! of its node 1, three quad rings and a band ring (for rand % 10 < 6).
//! The steps 0x1E021D0 / 0x1E02670 (and their draws) are never spawned and
//! not ported. Returns 1 on update 100, 4 on 235, 2 on 215, 6 on 111..209,
//! 8 once the 270-update timer +0x4C10 has run out.
use crate::b615::{cell, rmod, spawn_at};
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const WAVE: u32 = 0x01E0_1E20;
const QUADS: u32 = 0x01E0_2360;
const BAND: u32 = 0x01E0_2560;
/// The return address of the entry's `jalr` into a step (0x1E02BB8 + 8).
const TASK_RA: u64 = 0x01E0_2BC0;
const TIMER: u32 = D + 0x4C10;
const COLOUR: u32 = D + 0x4C14;
const NODES: u32 = D + 0x4C3C;
const TARGET_NODE: u32 = D + 0x4C4C;
const SLOT: u32 = D + 0x4C6A;
const NEXT: u32 = D + 0x4C6C;
const FACING: u32 = D + 0x4C6E;
const COUNTER: u32 = D + 0x4C72;
const BYTES: u32 = D + 0x4C74;
const BYTES2: u32 = D + 0x4D74;
const SINES: u32 = D + 0xD8;

/// The entry 0x1E02920.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    fx.mem.write_u32(D + 0x4C1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4FE0);
    l2::sine_table(fx, D + 0x4E74);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4C70, param);
    fx.mem.set32(TIMER, 0x10E);
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for (k, node) in [0x37, 0x1C, 0xD].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x288 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    let t = fx.texture(5, 0);
    fx.mem.write_u32(D + 0x4BF0, t);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // The entry's s0 is D here (rand's frame).
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_2AA0);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_2AF4);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_2B44);
        let (_, h) = fx.div(v, 0x28);
        fx.mem.set8(BYTES2 + k, h - 0x14);
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
        match f {
            0 => {}
            WAVE => wave(fx, t, k),
            QUADS => quads(fx, t, QUADS),
            BAND => band(fx, t, BAND),
            _ => panic!("b219: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == WAVE {
            for k in 0..4 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x50 + 4 * k, w);
            }
            let a = fx.mem.i16(t + 0x5C);
            let c = l2::sin_deg(fx, SINES, a, 1);
            let z = ee::add(fx.mem.f(sp + 0x58), ee::mul(fx.mem.f(t + 0x30), c));
            fx.mem.setf(sp + 0x58, z);
            let a = fx.mem.i16(t + 0x5C);
            let s = l2::sin_deg(fx, SINES, a, 0);
            let x = ee::add(fx.mem.f(sp + 0x50), ee::mul(fx.mem.f(t + 0x30), s));
            fx.mem.setf(sp + 0x50, x);
            quads4::ring_band_fy(fx, D, t, 0x20, sp + 0x50);
        }
        if fx.mem.read_u32(t + 0x44) == QUADS {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x9B, sub);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 1);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if fx.mem.read_u32(t + 0x44) == BAND {
            for k in 0..4 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x50 + 4 * k, w);
            }
            quads4::ring_band(fx, D, t, 0x21, sp + 0x50);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x64 => 1,
        0xEB => 4,
        0xD7 => 2,
        0x6F..=0xD1 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E01CE0(D, 0): the update counter and the emitter (frame 0x40: ra,
/// the entry's s2 = the parameter, s1 = 0, s0 = D): the attacker's node
/// 0x37 moved 3 to the facing, one WAVE a update on 64..200.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_2B90), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    fx.node_position(h, slot, D + 0x250);
    let z = fx.mem.f(D + 0x258);
    let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 3.0) } else { ee::add(z, 3.0) };
    fx.mem.setf(D + 0x258, z);
    let c = fx.mem.i16(COUNTER);
    if (0x40..0xC9).contains(&c) {
        let s = fx.mem.i16(NEXT);
        fx.mem.set16(SLOT, s);
        spawn_at(fx, WAVE, COLOUR, SLOT, &[(0x50, 0x01E0_1DC0), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, 1), (0, 0)]);
        let s = fx.mem.i16(NEXT) + 1;
        fx.mem.set16(NEXT, s);
        if fx.mem.i16(NEXT) >= 0x78 {
            fx.mem.set16(NEXT, 0);
        }
    }
    fx.leave(0x40);
}

/// 0x1E01E20 (D, task): a band (frame 0x50: ra, the entry's s3 = k, s2 =
/// the task's base, s1 = 0, s0 = D): inner radius +0x48 from 5 by 7.5 below
/// 200, its width +0x34 by 0.3 below 15 (outer +0x4C = inner - width),
/// sway +0x30 by 0.2 below 10 (+0x5C by 9); white, blue (8) on even ages;
/// the outer alpha in by 0x40 under 0x80 for 60 ages, then out by 9; its
/// angles (0x4000, 0, 0). It ages by 2 a update; at age 23 it throws three
/// QUADS and (rand % 10 < 6) a BAND at the target's node 1 plus 2 (rand %
/// r - r / 2) in each axis (r its radius); it moves 2.5 to the facing.
fn wave(fx: &mut Fx, t: u32, k: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x40, TASK_RA), (0x30, u64::from(k)), (0x20, u64::from(t - 0x2A0)), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x3C {
        let a = fx.mem.read_u8(t + 0x67);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x67, a + 0x40);
        }
    } else {
        let a = fx.mem.read_u8(t + 0x67);
        if a < 0xB {
            fx.mem.set16(t + 0x58, 0);
        } else {
            fx.mem.write_u8(t + 0x67, a - 9);
        }
    }
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
    } else {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 8);
    }
    fx.mem.write_u32(t + 0x20, 0x4000);
    let w = fx.mem.f(t + 0x34);
    if w < 15.0 {
        fx.mem.setf(t + 0x34, ee::add(w, f32::from_bits(0x3E99_999A)));
    }
    let s = fx.mem.f(t + 0x30);
    if s < 10.0 {
        fx.mem.setf(t + 0x30, ee::add(s, f32::from_bits(0x3E4C_CCCD)));
    }
    let r = fx.mem.f(t + 0x48);
    if r < 200.0 {
        fx.mem.setf(t + 0x48, ee::add(r, 7.5));
    }
    let o = ee::sub(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x4C, o);
    let a = fx.mem.i16(t + 0x5C) + 9;
    fx.mem.set16(t + 0x5C, a);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if fx.mem.i16(t + 0x54) == 0x17 {
        let h = fx.target_handle();
        let rad = crate::blib::afterimage::unit_radius(fx, h);
        fx.fix_residue(rad, 0x01E0_2024);
        let r = ee::fixsfsi(rad) as i16 as i32;
        let h = fx.target_handle();
        let slot = fx.mem.i16(TARGET_NODE);
        fx.node_position(h, slot, D + 0x250);
        let half = r >> 1;
        for (o, ra) in [(0x250u32, 0x01E0_204Cu32), (0x254, 0x01E0_207C), (0x258, 0x01E0_20AC)] {
            let v = fx.rand_from(r as u32, ra);
            let (_, m) = fx.div(v, r);
            let c = ee::add(fx.mem.f(D + o), ee::from_i32((m - half) << 1));
            fx.mem.setf(D + o, c);
        }
        let v = fx.rand_from(r as u32, 0x01E0_20DC);
        let (_, m) = fx.div(v, 0xA);
        if m < 6 {
            let saves = |ra: u32| [(0x50, u64::from(ra)), (0x40, 0), (0x30, u64::from(D)), (0x20, u64::from(t)), (0x10, half as i64 as u64), (0, r as i64 as u64)];
            for ra in [0x01E0_2110u32, 0x01E0_2124, 0x01E0_2138] {
                spawn_at(fx, QUADS, COLOUR, SLOT, &saves(ra));
            }
            spawn_at(fx, BAND, COLOUR, SLOT, &saves(0x01E0_214C));
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 2.5) } else { ee::add(z, 2.5) };
        fx.mem.setf(t + 0x18, z);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x50);
}

/// The step frame (0x20: ra, the entry's s0 = D).
fn frame20(fx: &mut Fx) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
}

/// 0x1E02360 (D, task) at `f`: b615's quads (a75a0f66bc56) with the colour
/// (0x80, 0x80, 8).
fn quads(fx: &mut Fx, t: u32, f: u32) {
    quads_with(fx, t, f, [0x80, 0x80, 8], 0x96, TASK_RA);
}

/// [`quads`] at `f` with its colour, base radius (b247 0x1E02030: (0xC0,
/// 0x50, 0x50), 200) and the entry's `jalr` return address.
pub(crate) fn quads_with(fx: &mut Fx, t: u32, f: u32, colour: [i32; 3], radius: i32, task_ra: u64) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, task_ra), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x1C, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = fx.rand_from(t, f + 0x50);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, colour[0], colour[1], colour[2]);
        for (o, site) in [(0x20u32, 0x7Cu32), (0x24, 0x98), (0x28, 0xB4)] {
            let v = rmod(fx, t, f + site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, f + 0xD8, radius);
        fx.mem.set16(t + 0x50, v + radius);
        let v = rmod(fx, t, f + 0xF8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0x114, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, f + 0x13C);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, f + 0x154, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xD {
            fx.mem.write_u8(t + 0x63, a - 0xB);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let c = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, c);
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E02560 (D, task) at `f`: b615's band ring with inner radius 120,
/// grey (0x80) inside, yellow (0x80, 0x80, 8) outside.
fn band(fx: &mut Fx, t: u32, f: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42F0_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 8);
        let v = rmod(fx, t, f + 0x4C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 4 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let r = ee::add(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, r);
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), f + 0xE0);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}
