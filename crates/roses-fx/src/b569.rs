//! BMODULE b569 (card 443), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E024F0, data area 0x1ED0000) [F]: b631's task pool of 400 (0x70 bytes at +0x2A0, alive +0x58, step
//! +0x44 as a record address) drawn with the second library: quads
//! (`billboard::quad`, 0x1E00920, UVs by uv_rect 0x1E00080), spark fans
//! (`quads4::sparks`, 21cb40d76bec at 0x1E00BE0) and the record's own
//! four-quad sprite keyed by its mean w (0x1E00FE0, [`sprite_quads_w`]).
//! In eleven windows from update 35 to 205 three grey sparks a update
//! leave the attacker's node 0x2F (0x37 in the last three); from 207 to
//! 220 four bubbles and two coloured fans a update start at node 0x13,
//! where on update 207 four light planes open. Returns 1 on update 197, 4
//! on 255, 2 on 207, 8 once the 260-update timer +0xB210 has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::c061::rgbaq_at;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const PLANE: u32 = 0x01E0_1AB0;
const FAN: u32 = 0x01E0_1C60;
const BUBBLE: u32 = 0x01E0_1E90;
const SPARK: u32 = 0x01E0_21B0;
const IDLE: u32 = 0x01E0_24E0;
/// The return address of the entry's `jalr` into a step (0x1E027F0 + 8).
const TASK_RA: u64 = 0x01E0_27F8;
const TIMER: u32 = D + 0xB210;
const COLOUR: u32 = D + 0xB214;
const COUNTER: u32 = D + 0xB2B8;
const FACING: u32 = D + 0xB2B4;
const BYTES: u32 = D + 0xB2BA;
/// Node 0x13 on update 207.
const POINT: u32 = D + 0xB1D0;
/// The scratchpad, where the originals project.
const SPR: u32 = 0x7000_0000;

/// The entry 0x1E024F0.
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
    l2::sine_table(fx, D + 0xB4BC);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x104);
    for (at, node, clear) in [(0xB244u32, 0x2F, 1), (0xB246, 0x37, 0), (0xB248, 0x13, 0), (0xB24A, 0x30, 0), (0xB24C, 0x2F, 0), (0xB24E, 0x2C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    for k in 0..2u32 {
        let t = fx.texture(4, 0xE + k as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k, t);
    }
    let t = fx.texture(5, 0);
    fx.mem.write_u32(D + 0xB1F0, t);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand();
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f != 0 && f != IDLE {
            let t = slot + 0x2A0;
            match f {
                PLANE => plane(fx, t),
                FAN => fan(fx, t),
                BUBBLE => bubble(fx, t, false),
                SPARK => bubble(fx, t, true),
                _ => panic!("b569: unknown task step {f:#x}"),
            }
        }
    }
    for k in 0..TASKS {
        let s = D + 0x70 * k;
        if fx.mem.i16(s + 0x2F8) == 0 {
            continue;
        }
        let t = s + 0x2A0;
        if fx.mem.read_u32(t + 0x44) == PLANE {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, sp + 0x50, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            sprite_quads_w(fx, D, t, 0, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == FAN {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x60, 0x96, sub);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            fx.mem.set16(D + 0xB8, 0xC);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (f, local, mode) in [(BUBBLE, 0x70u32, 0x20), (SPARK, 0x80, 0x11)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            let w = fx.mem.read_u32(D + 0xB1F0);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + local, 0x9B, sub);
            let o = fx.mem.i16(t + 0x52);
            fx.mem.set16(D + 0xBE, o);
            let sub = fx.mem.i16(t + 0x56);
            let (u, v) = if sub < 2 { (sub << 6, 0) } else { ((sub - 2) << 6, 0x40) };
            fx.mem.set16(D + 0x94, u);
            fx.mem.set16(D + 0x96, v);
            fx.mem.set16(D + 0x98, 0x40);
            fx.mem.set16(D + 0x9A, 0x40);
            billboard::quad(fx, uv::uv_rect, D, t, mode, t + 0x10);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xC5 => 1,
        0xFF => 4,
        0xCF => 2,
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

/// 0x1E00FE0(D, task, mode, pos): `quads4::sprite_quads` (792ca1ad9378)
/// with a 0x200 frame and no far Z: four textured quads (strip3, FST) of
/// the task's size (f32 +0x30 / +0x34 as integers, 0x109640's four
/// layouts) turned by +0x5A about z, billboarded at `pos` (scale 0.05),
/// coloured by (mode >> 4) 0: +0x64 x3 / +0x60, 1: +0x64 x2 / +0x60 x2;
/// UVs from D+0x94 in D+0xBE's orientation, texture D+0x88; each visible
/// quad keyed by its mean w (0x1A2C70) on D+0x80 [F].
pub fn sprite_quads_w(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x200);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x60 + 4 * k, w);
    }
    let d = sp + 0x130;
    l2::desc_strip3(fx, d);
    match mode >> 4 {
        0 => {
            rgbaq_at(fx, sp + 0x1C0, t + 0x64, d + 0x10);
            rgbaq_at(fx, sp + 0x1C8, t + 0x64, d + 0x28);
            rgbaq_at(fx, sp + 0x1D0, t + 0x64, d + 0x40);
            rgbaq_at(fx, sp + 0x1D8, t + 0x60, d + 0x58);
        }
        1 => {
            rgbaq_at(fx, sp + 0x1E0, t + 0x64, d + 0x10);
            rgbaq_at(fx, sp + 0x1E8, t + 0x64, d + 0x28);
            rgbaq_at(fx, sp + 0x1F0, t + 0x60, d + 0x40);
            rgbaq_at(fx, sp + 0x1F8, t + 0x60, d + 0x58);
        }
        _ => {}
    }
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xC0 + 4 * k, 0x3D4C_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x120);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x60 + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xC0, sp + 0x120, sp + 0xB0);
    let a = fx.mem.i16(t + 0x5A);
    fx.mem.set32(sp + 0x128, a);
    fx.mem.set32(sp + 0x124, 0);
    fx.mem.set32(sp + 0x120, 0);
    for k in 0..4 {
        let w = ee::fixsfsi(fx.mem.f(t + 0x30));
        let h = ee::fixsfsi(fx.mem.f(t + 0x34));
        l2::quad_ints(fx, sp + 0x70, w, h, k);
        l2::rotate4(fx, sp + 0x70, sp + 0x120, 2, d0 + 0xD8);
        if l2::project(fx, sp + 0x70, SPR, sp + 0xE0, d, sp + 0xD0) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0xD0)) as i32;
            let ot = fx.mem.read_u32(d0 + 0x80);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x200);
}

/// 0x1E012A0(D, 0): the update counter and the spawns. Frame 0x30 (the
/// entry's s1 = D, s0 = 0); 0x1E02B80's saves: the entry's s4 / s3 = 0,
/// s2 = the parameter, the loop's s1, s0 = D.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_27B8), (0x10, u64::from(D)), (0, 0)]);
    let p = param as i64 as u64;
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // (window start, end, node slot, the spawns' return addresses)
    const WINDOWS: [(i32, i32, u32, u32); 11] = [
        (0x23, 0x2C, 0xB244, 0x01E0_1330),
        (0x33, 0x3D, 0xB244, 0x01E0_13B8),
        (0x48, 0x52, 0xB244, 0x01E0_1440),
        (0x5D, 0x67, 0xB244, 0x01E0_14C8),
        (0x72, 0x7D, 0xB244, 0x01E0_1550),
        (0x9C, 0xA2, 0xB244, 0x01E0_15D8),
        (0xAF, 0xB8, 0xB244, 0x01E0_1660),
        (0xC5, 0xC9, 0xB244, 0x01E0_16E8),
        (0xA2, 0xA8, 0xB246, 0x01E0_1770),
        (0xB8, 0xBF, 0xB246, 0x01E0_17F8),
        (0xC8, 0xCE, 0xB246, 0x01E0_1880),
    ];
    for (a, b, slot, ra) in WINDOWS {
        let c = fx.mem.i16(COUNTER);
        if c < a || c >= b {
            continue;
        }
        let h = fx.self_handle();
        let s = fx.mem.i16(D + slot);
        fx.node_position(h, s, D + 0x250);
        for k in 0..3u64 {
            spawn(fx, SPARK, &saves(ra, p, k));
        }
    }
    if fx.mem.i16(COUNTER) == 0xCF {
        let h = fx.self_handle();
        let s = fx.mem.i16(D + 0xB248);
        fx.node_position(h, s, POINT);
    }
    let c = fx.mem.i16(COUNTER);
    if (0xCF..0xDD).contains(&c) {
        for k in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        for k in 0..4u64 {
            spawn(fx, BUBBLE, &saves(0x01E0_194C, p, k));
        }
        fx.mem.set16(D + 0xB2B0, 0);
        for k in 0..2u64 {
            spawn(fx, FAN, &saves(0x01E0_1994, p, k));
        }
        if fx.mem.i16(COUNTER) == 0xCF {
            for (angle, kind, ra) in [(0, 0, 0x01E0_19F8), (0x5A, 0, 0x01E0_1A28), (0x2D, 1, 0x01E0_1A5C), (0x87, 1, 0x01E0_1A90)] {
                fx.mem.set16(D + 0xB2A2, angle);
                fx.mem.set16(D + 0xB2B0, kind);
                spawn(fx, PLANE, &saves(ra, p, 2));
            }
        }
    }
    fx.leave(0x30);
}

fn saves(ra: u32, p: u64, s1: u64) -> [(u32, u64); 6] {
    [(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, s1), (0, u64::from(D))]
}

/// 0x1E02B80(D, step, 0): the first free task cleared and set off at
/// D+0x250 with the colour +0xB214 twice, kind +0xB2B0 (+0x50), angle
/// +0xB2A2 (+0x5A). `saves`: its prologue's.
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
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
            let a = fx.mem.i16(D + 0xB2B0);
            fx.mem.set16(t + 0x50, a);
            let a = fx.mem.i16(D + 0xB2A2);
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

/// rand % m (HI) from a step's `jal` at `site` (its s0 = the task).
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// (rand >> 4) % m (HI) from `site`.
fn rmod4(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v >> 4, m).1
}

/// 0x205580(t + 0x20) from a step of frame 0x30 (its saves: ra, the
/// step's s1 = D, s0 = t).
fn angles(fx: &mut Fx, t: u32, ra: u32) {
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, u64::from(ra)), (0x10, u64::from(D)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
}

/// 0x1E01AB0: a light plane (frame 0x20): kind +0x50 0: 15 x 200 growing
/// by 0.3 x 120 a update, 1: 15 x 120 growing by 0.3 x 60, 2: 8 x 400;
/// in by 0x40 for 12 updates, then out by 7; turning by 6 degrees.
fn plane(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0xFF);
        for (kind, w, h) in [(0, 0x4170_0000u32, 0x4348_0000u32), (1, 0x4170_0000, 0x42F0_0000), (2, 0x4100_0000, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, w);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x52, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xC {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 9 {
        fx.mem.write_u8(t + 0x63, a - 7);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    for (kind, dh) in [(0, 120.0f32), (1, 60.0)] {
        if fx.mem.i16(t + 0x50) == kind {
            let h = ee::add(fx.mem.f(t + 0x34), dh);
            fx.mem.setf(t + 0x34, h);
            let w = ee::add(fx.mem.f(t + 0x30), f32::from_bits(0x3E99_999A));
            fx.mem.setf(t + 0x30, w);
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    let a = fx.mem.i16(t + 0x5A) + 6;
    fx.mem.set16(t + 0x5A, a);
    fx.leave(0x20);
}

/// 0x1E01C60: a spark fan (frame 0x20): radii 200 / 142 growing by +0x34
/// (56.6..86.5) and 0.3 of it, kind +0x50 (rand % 3: violet, blue-white,
/// green), full alpha for 8 updates, then out by 14.
fn fan(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.setf(t + 0x4C, ee::sub(200.0, 58.0));
        let v = rmod4(fx, t, 0x01E0_1C98, 3);
        fx.mem.set16(t + 0x50, v);
        for (kind, c, o) in [(0, (0xC0, 0x80, 0xFF), 0), (1, (0xA0, 0xA0, 0xFF), 0x30), (2, (0x80, 0xFF, 0x80), 0x30)] {
            if fx.mem.i16(t + 0x50) == kind {
                l2::rgb(fx, t + 0x60, c.0, c.1, c.2);
                l2::rgb(fx, t + 0x64, o, o, o);
            }
        }
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 0x1E);
        let v = rmod(fx, t, 0x01E0_1D68, 0x12C);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0x226));
        let s = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, s);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, site) in [(0x20u32, 0x01E0_1DB0u32), (0x24, 0x01E0_1DCC), (0x28, 0x01E0_1DE8)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3E99_999A), r));
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01E90 (a bubble: size 130 growing by 5 to 200, square, drift +-50
/// / 60 each way) and 0x1E021B0 (a grey spark: 50 x 25 growing by 3 to
/// 100 (height half), rising 0..49 / 60, the side drift / 80), frame 0x30
/// (s1 = D, s0 = the task): a random cell frame (rand >> 4) % 3, the
/// camera's angles plus random spins, an orientation +0x52; in by 0x40
/// for 12 updates, then out by 9; 0.8 a update toward (bubble) or away
/// from (spark) the facing.
fn bubble(fx: &mut Fx, t: u32, spark: bool) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    // The rands' `jal`s from the cell frame on.
    let sites: [u32; 9] = if spark {
        [0x01E0_21E4, 0x01E0_2218, 0x01E0_2240, 0x01E0_226C, 0x01E0_22CC, 0x01E0_22F0, 0x01E0_2310, 0x01E0_2330, 0x01E0_2350]
    } else {
        [0x01E0_1EC0, 0x01E0_1EF4, 0x01E0_1F20, 0x01E0_1F4C, 0x01E0_1FA4, 0x01E0_1FC8, 0x01E0_1FE8, 0x01E0_2008, 0x01E0_2028]
    };
    if fx.mem.i16(t + 0x58) == 1 {
        let size = if spark { 0x32 } else { 0x82 };
        fx.mem.set16(t + 8, size);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, if spark { w >> 1 } else { w });
        let v = rmod4(fx, t, sites[0], 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        let (o1, o2) = (0x34u32, 0x30u32);
        let v = if spark { rmod(fx, t, sites[1], 0x32) } else { rmod(fx, t, sites[1], 0x64) - 0x32 };
        fx.mem.setf(t + o1, ee::from_i32(v));
        let v = rmod(fx, t, sites[2], 0x64) - 0x32;
        fx.mem.setf(t + o2, ee::from_i32(v));
        let v = rmod(fx, t, sites[3], 0x64) - 0x32;
        fx.mem.setf(t + 0x38, ee::from_i32(v));
        for o in [0x34u32, 0x30] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let q = ee::div(fx.mem.f(t + 0x38), if spark { 80.0 } else { 60.0 });
        fx.mem.setf(t + 0x38, q);
        let v = rmod(fx, t, sites[4], 0x168);
        fx.mem.set16(t + 0x5C, v);
        angles(fx, t, sites[5]);
        for (o, site) in [4u32, 2, 0].into_iter().zip([sites[5], sites[6], sites[7]]) {
            let v = fx.rand_from(t, site + 8);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
        let v = fx.rand_from(t, sites[8] + 8);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xC {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let (cap, by) = if spark { (0x64, 3) } else { (0xC8, 5) };
        let w = fx.mem.i16(t + 8);
        if w < cap {
            fx.mem.set16(t + 8, w + by);
        }
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, if spark { w >> 1 } else { w });
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for (p, v) in [(0x10u32, 0x30u32), (0x14, 0x34), (0x18, 0x38)] {
            let x = ee::add(fx.mem.f(t + p), fx.mem.f(t + v));
            fx.mem.setf(t + p, x);
        }
        let z = fx.mem.f(t + 0x18);
        let toward = (fx.mem.i16(FACING) != 0) != spark;
        let d = f32::from_bits(0x3F4C_CCCD);
        fx.mem.setf(t + 0x18, if toward { ee::sub(z, d) } else { ee::add(z, d) });
    }
    for (a, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + a).wrapping_add(fx.mem.i16(t + s) as u32);
        fx.mem.write_u32(t + a, v);
    }
    angles(fx, t, if spark { 0x01E0_24B4 } else { 0x01E0_2188 });
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}
