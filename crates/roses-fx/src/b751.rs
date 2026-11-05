//! BMODULE b751 (card 267), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02230, data area 0x1ED0000) [F]: a pool of 400 tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step
//! +0x44 called through `jalr`; spawned by 0x1E02910, `spawn::task400`'s
//! body) drawn with the second library: single quads (add5d81dd3fe at
//! 0x1E00C80, `billboard::quad`), an arc (e6dce326551d at 0x1E00920,
//! `billboard::arc_strips`) and textured arcs (3a2db45c5a9a at 0x1E00F40,
//! `quads4::arc_strips_tex`), UVs by uv_rect 0x1E00080. On updates 99 and
//! 103 twelve white motes orbit each of the attacker's nodes 0x2C / 0x36 /
//! 0x42 / 0x4C; on 107..118 three swirling red bands a update at node
//! 0xF, on 118 an arc and 24 more bands there; a frame flash (0x207EF0)
//! from 107. Returns 1 on update 108, 4 on 185, 2 on 160, 6 on 119..154, 8
//! once the 190-update timer +0xB210 has run out.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, spawn::task400, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const MOTE: u32 = 0x01E0_16C0;
const ARC: u32 = 0x01E0_1890;
const SWIRL: u32 = 0x01E0_1980;
const BAND: u32 = 0x01E0_1BF0;
const BAND_B: u32 = 0x01E0_1E30;
const FLASH: u32 = 0x01E0_2070;
/// A step that only returns 0 (skipped by the entry).
const IDLE: u32 = 0x01E0_2220;
/// The return address of the entry's `jalr` into a step (0x1E024F8 + 8).
const TASK_RA: u64 = 0x01E0_2500;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const NODES: u32 = D + 0xB244;
const FACING: u32 = D + 0xB2B4;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;
const SINES: u32 = D + 0xB4BC;

/// The entry 0x1E02230.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0xBE);
    for (k, node) in [0x2C, 0x36, 0x42, 0x4C, 0xF].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x298 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(k, 0x01E0_2410);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_2464);
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
        match f {
            0 | IDLE => {}
            MOTE => mote(fx, t),
            ARC => arc(fx, t),
            SWIRL | BAND | BAND_B => band(fx, t, f),
            FLASH => flash(fx, t),
            _ => panic!("b751: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == MOTE {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x50, 0x14, sub);
            billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (f, r, id, whole) in [(SWIRL, 0x60u32, 0x97, true), (BAND, 0x70, 0x97, true), (BAND_B, 0x80, 0x9B, false)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + r, id, sub);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            if whole {
                for _ in 0..2 {
                    let w = fx.mem.read_u32(TEX + 4);
                    fx.mem.write_u32(D + 0x88, w);
                }
                for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x80), (0x9A, 0x80)] {
                    fx.mem.set16(D + o, v);
                }
            }
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            fx.mem.set16(D + 0xB8, 6);
            fx.mem.set16(D + 0xBE, 0);
            quads4::arc_strips_tex(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x6C => 1,
        0xB9 => 4,
        0xA0 => 2,
        0x77..=0x9A => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
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

/// 0x1E01350(D, 0): the update counter and the emitters (frame 0x30: ra,
/// the entry's s1 = D / s0 = 0; its own s0 = D, s1 the loop counters).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_24C0), (0x10, u64::from(D)), (0, 0)]);
    let p = param as i64 as u64;
    let spawn = |fx: &mut Fx, f: u32, s1: u64, ra: u32| {
        task400(fx, D, f, 0, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, s1), (0, u64::from(D))]);
    };
    let node = |fx: &mut Fx, k: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        fx.node_position(h, slot, D + 0x250);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let mut s1 = u64::from(D);
    for (c0, nodes, ras) in [(0x63, [0u32, 1], [0x01E0_13D4u32, 0x01E0_1430]), (0x67, [2, 3], [0x01E0_14AC, 0x01E0_1508])] {
        if fx.mem.i16(COUNTER) == c0 {
            for (k, ra) in nodes.into_iter().zip(ras) {
                node(fx, k);
                for i in 0..0xCu64 {
                    spawn(fx, MOTE, i, ra);
                }
                s1 = 0xC;
            }
        }
    }
    if fx.mem.i16(COUNTER) == 0x76 {
        node(fx, 4);
        spawn(fx, ARC, s1, 0x01E0_157C);
        for i in 0..0xCu64 {
            spawn(fx, BAND, i, 0x01E0_1598);
        }
        for i in 0..0xCu64 {
            spawn(fx, BAND_B, i, 0x01E0_15D8);
        }
        s1 = 0xC;
    }
    let c = fx.mem.i16(COUNTER);
    if (0x6B..0x77).contains(&c) {
        node(fx, 4);
        for ra in [0x01E0_1658, 0x01E0_166C, 0x01E0_1680] {
            spawn(fx, SWIRL, s1, ra);
        }
    }
    if fx.mem.i16(COUNTER) == 0x6B {
        spawn(fx, FLASH, s1, 0x01E0_16AC);
    }
    fx.leave(0x30);
}

/// Alpha `a`: in by `up` under `cap` for `young` updates, then out by
/// `down` (the task ends below `floor`).
fn fade(fx: &mut Fx, t: u32, a: u32, young: i32, cap: u8, up: u8, floor: u8, down: u8) {
    let v = fx.mem.read_u8(t + a);
    if fx.mem.i16(t + 0x54) < young {
        if v < cap {
            fx.mem.write_u8(t + a, v.wrapping_add(up));
        }
    } else if v >= floor {
        fx.mem.write_u8(t + a, v - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// Pushed out along the angle +0x5A by `k` (the table's sines, 0 on z, 1
/// on x).
fn swing(fx: &mut Fx, t: u32, k: f32) {
    let a = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let z = ee::add(fx.mem.f(t + 0x18), ee::mul(k, s));
    fx.mem.setf(t + 0x18, z);
    let a = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, SINES, a, 1);
    let x = ee::add(fx.mem.f(t + 0x10), ee::mul(k, c));
    fx.mem.setf(t + 0x10, x);
}

/// 0x1E016C0 (D, task): a white mote orbiting its node (frame 0x30): size
/// 120, cell row (rand >> 4) % 3 + 2, angle (rand >> 4) % 360, pushed out
/// by 8.6, then by 0.6 a update; in by 0x60 under 0xC0 for 6 updates, out
/// by 19; turned by the camera's angles.
fn mote(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x78);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_16F8);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_1730);
        let (_, h) = fx.div(v >> 4, 0x168);
        fx.mem.set16(t + 0x5A, h);
        swing(fx, t, f32::from_bits(0x4109_999A));
    }
    fade(fx, t, 0x63, 6, 0xC0, 0x60, 0x15, 0x13);
    if fx.mem.i16(t + 0x54) != 0 {
        swing(fx, t, f32::from_bits(0x3F19_999A));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1868);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E01890 (D, task): the arc: outer radius +0x4C 220, inner orange
/// (0xFF, 0x80, 0x40), outer pink (0xFF, 0xC0, 0xC0) fading in by 0x60
/// under 0xC0 for 14 updates, then out by 9; angles 10 / 10; turned by the
/// camera's angles (its first + 0x4000).
fn arc(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x435C_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x40);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0x67, 0xE, 0xC0, 0x60, 0xB, 9);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1950);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// rand % m (HI) from a `jal` returning to `ra` (rand's frame: s0 = `t`).
fn rmod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// 0x1E01980 (`SWIRL`, frame 0x30) / 0x1E01BF0 (`BAND`) / 0x1E01E30
/// (`BAND_B`) (D, task): a textured band: inner radius +0x48 140 widening
/// by +0x38 (the swirl 15..22.9, the bands 12..16.9) a update, outer +0x4C
/// 82 (then 0.8 x / 0.1 x the inner, 0.1 for `BAND_B`), height +0x34 100
/// (the swirl rising by +0x38), +0x30 15..22.9; two colourings by
/// (rand >> 4) % 2 (dark red, pink / grey; `BAND_B` blue); the swirl faces
/// away from the facing and turns by 0x100 a update, the bands turn at
/// random; alpha 0x80 for 8 updates, then out by 5 (3).
fn band(fx: &mut Fx, t: u32, f: u32) {
    let swirl = f == SWIRL;
    let frame = if swirl { 0x30 } else { 0x20 };
    let sp = fx.enter(frame);
    if swirl {
        fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    } else {
        fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    }
    // The rand `jal`s' return addresses.
    let ras: [u32; 6] = match f {
        SWIRL => [0x01E0_19C8, 0x01E0_1A5C, 0x01E0_1A80, 0x01E0_1AC0, 0x01E0_1B34, 0],
        BAND => [0x01E0_1C30, 0x01E0_1CC8, 0x01E0_1D08, 0x01E0_1D58, 0x01E0_1D74, 0x01E0_1D90],
        _ => [0x01E0_1E70, 0x01E0_1F0C, 0x01E0_1F4C, 0x01E0_1F9C, 0x01E0_1FB8, 0x01E0_1FD4],
    };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x430C_0000);
        fx.mem.setf(t + 0x4C, ee::sub(140.0, 58.0));
        let v = fx.rand_from(t, ras[0]);
        fx.mem.set16(t + 0x50, (v >> 4) % 2);
        // (inner +0x60, outer +0x64) colours by kind.
        let colours: [[[i32; 3]; 2]; 2] = match f {
            SWIRL => [[[0, 0, 0], [0xC0, 0x20, 0x20]], [[0x30, 0x30, 0x30], [0xFF, 0xA0, 0xA0]]],
            BAND => [[[0xC0, 0x20, 0x20], [0, 0, 0]], [[0xFF, 0xA0, 0xA0], [0x30, 0x30, 0x30]]],
            _ => [[[0x20, 0x20, 0xC0], [0x40, 0x40, 0x40]], [[0xA0, 0xA0, 0xFF], [0x30, 0x30, 0x30]]],
        };
        for kind in 0..2 {
            if fx.mem.i16(t + 0x50) == kind {
                let [inner, outer] = colours[kind as usize];
                // The swirl sets the outer first, the bands the inner.
                let order = if swirl { [(0x64u32, outer), (0x60, inner)] } else { [(0x60, inner), (0x64, outer)] };
                for (o, [r, g, b]) in order {
                    l2::rgb(fx, t + o, r, g, b);
                }
            }
        }
        fx.mem.set16(t + 0x5A, 0x3C);
        if swirl {
            let v = fx.rand_from(t, ras[1]);
            let (_, h) = fx.div(v >> 4, 0x38);
            fx.mem.set16(t + 0x5C, h + 6);
            let v = rmod(fx, t, ras[2], 0x50);
            fx.mem.setf(t + 0x38, ee::div(ee::from_i32(v + 0x96), 10.0));
            let v = rmod(fx, t, ras[3], 0x50);
            fx.mem.setf(t + 0x30, ee::div(ee::from_i32(v + 0x96), 10.0));
        } else {
            fx.mem.set16(t + 0x5C, if f == BAND { 0x3C } else { 0xA });
            let v = rmod(fx, t, ras[1], 0x32);
            fx.mem.setf(t + 0x38, ee::div(ee::from_i32(v + 0x78), 10.0));
            let v = rmod(fx, t, ras[2], 0x50);
            fx.mem.setf(t + 0x30, ee::div(ee::from_i32(v + 0x96), 10.0));
        }
        fx.mem.write_u32(t + 0x34, 0x42C8_0000);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        if swirl {
            let a = if fx.mem.i16(FACING) != 0 { 0x4000 } else { -0x4000 };
            fx.mem.set32(t + 0x20, a);
            let v = rmod(fx, t, ras[4], 0xFFFF);
            fx.mem.set32(t + 0x28, v);
        } else {
            for (o, ra) in [(0x20u32, ras[3]), (0x24, ras[4]), (0x28, ras[5])] {
                let v = rmod(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
        }
    }
    let (a, floor, down) = if swirl { (0x67, 7, 5) } else { (0x63, 5, 3) };
    if fx.mem.i16(t + 0x54) < 8 {
        fx.mem.write_u8(t + a, 0x80);
    } else {
        let v = fx.mem.read_u8(t + a);
        if v >= floor {
            fx.mem.write_u8(t + a, v - down);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x38));
    fx.mem.setf(t + 0x48, r);
    match f {
        SWIRL => {
            fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3F4C_CCCD), r));
            let h = ee::add(fx.mem.f(t + 0x34), fx.mem.f(t + 0x38));
            fx.mem.setf(t + 0x34, h);
            let v = fx.mem.read_u32(t + 0x28).wrapping_add(0x100);
            fx.mem.write_u32(t + 0x28, v);
        }
        BAND => fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3DCC_CCCD), r)),
        _ => fx.mem.write_u32(t + 0x4C, 0x3DCC_CCCD),
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(frame);
}

/// 0x1E02070 (D, task): the frame flash (frame 0x80; its descriptor at
/// sp + 0x28 = the library frame quad's, [`Fx::frame_quad_rgba`] from
/// this caller): the frame over the window in (0x90, 0x80, 0x80, alpha
/// +0x63), ALPHA 0x44; in by 0x14 under 0x50 for 30 updates, then out by
/// 9 (the task ends below 11).
fn flash(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0x90, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let c = [fx.mem.read_u8(t + 0x60), fx.mem.read_u8(t + 0x61), fx.mem.read_u8(t + 0x62), fx.mem.read_u8(t + 0x63)];
    fx.frame_quad_rgba(c);
    let sp = fx.enter(0x80);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    fade(fx, t, 0x63, 0x1E, 0x50, 0x14, 0xB, 9);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x80);
}
