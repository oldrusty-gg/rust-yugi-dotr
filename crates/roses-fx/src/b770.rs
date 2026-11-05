//! BMODULE b770 (card 95), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02EC0, data area 0x1ED0000, 0x4C80 bytes) [F]
//!: b232's layout family (150 tasks of 0x70
//! bytes at D+0x2A0, alive +0x58, step +0x44 through `jalr`; spawned by
//! 0x1E03800 = b272's spawn with the colour D+0x44A4 and the kind
//! D+0x4906) drawn with the second library: facing rings turned about z
//! (2f5a28a746a2 at 0x1E012D0, `quads4::ring_facing_flat_z`, uv_rect
//! 0x1E00080), a full-window tint (561a6131a635 at 0x1E01100) and the
//! record's six-spoke fan [`spokes`] (0x1E00D70); plus a unit flash
//! (0x1E03C50: b232's 0x1E01ED0 with the attacker's node 0x43 projected
//! into the sprite's first XYZ2 until update 0xE6, the layer keyed by
//! that point's w) stepping b232's flash sprites (0x1E02C80, grey 0x28).
//! Update 5 a violet tint at 500 left or right of the attacker (by the
//! camera's x on update 2); 0..0xBD red rings a update at nodes 0x24 /
//! 0x4E (grey ones every eighth); 0x32..0xA9 every 16th a red ring at node
//! 0x43; 0xBE six wide red rings there; 0xBE..0x107 red and grey rings at
//! node 0x43; 0xBE..0xF9 a red fan at node 0x41; 0xC8 the two flashes;
//! 0x108..0x130 white rings at the target. The steps 0x1E02610 / 0x1E02770
//! / 0x1E02B10 are drawn but never spawned [F: the spawns name 0x2490 /
//! 0x2C80 / 0x1F60 / 0x22E0 / 0x2950 / 0x20F0 / 0x1BA0 / 0x1D80 only]: not
//! ported. Returns 1 on update 0xAF, 4 on 0x140, 2 on 0x136, 6 on
//! 0xBA..0x12B, 8 once the 369-update timer D+0x44A0 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::b590::project_residue;
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0x96;
const RING_W: u32 = 0x01E0_1BA0;
const RING_N: u32 = 0x01E0_1D80;
const RING_R: u32 = 0x01E0_1F60;
const RING_T: u32 = 0x01E0_20F0;
const RING_G: u32 = 0x01E0_22E0;
const TINT: u32 = 0x01E0_2490;
const FAN: u32 = 0x01E0_2950;
/// The flash sprites, run by the unit flash 0x1E03C50.
const FLASH: u32 = 0x01E0_2C80;
/// The return address of the entry's `jalr` into a step (0x1E03150 + 8).
const TASK_RA: u64 = 0x01E0_3158;
const POS: u32 = D + 0x250;
/// The camera's x on update 2.
const CAM_X: u32 = D + 0x4470;
const TEX: u32 = D + 0x4480;
const TIMER: u32 = D + 0x44A0;
const COLOUR: u32 = D + 0x44A4;
const LAYER: u32 = D + 0x44B4;
const FLASH_DESC: u32 = D + 0x44B8;
const TINT_W: u32 = D + 0x4884;
const FLASH_W: u32 = D + 0x4888;
const NODES: u32 = D + 0x489C;
const TARGET_NODE: u32 = D + 0x48AA;
const SPAWN_K: u32 = D + 0x4906;
const FACING: u32 = D + 0x490A;
const COUNTER: u32 = D + 0x490E;
const BYTES: u32 = D + 0x4910;
const TAB: u32 = D + 0x4B10;
const SPR: u32 = 0x7000_0000;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn node(fx: &mut Fx, k: u32) -> i32 {
    fx.mem.i16(NODES + 2 * k)
}

/// The entry 0x1E02EC0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x180);
    fx.mem.write_u32(D + 0x44AC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x180);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4C80);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x490C, param);
    fx.mem.set32(TIMER, 0x171);
    for (k, n) in [0x41, 0x43, 0x24, 0x4E, 0x40].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for k in 0xA..0x10u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x288 + 4 * (k - 0xA), t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xE0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xE4)), 180.0);
    fx.fix_residue(y, 0x01E0_3058);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = the loop count.
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3080, 0x14);
        fx.mem.set8(BYTES + k, v - 0xA);
    }
    for k in 0..0x1Eu32 {
        let v = rmod(fx, k, 0x01E0_30CC, 0xF);
        fx.mem.set16(D + 0x48BC + 2 * k, v);
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
            RING_W | RING_N => ring_w(fx, t, f),
            RING_R => ring_r(fx, t),
            RING_T => ring_t(fx, t),
            RING_G => ring_g(fx, t),
            TINT => tint(fx, t),
            FAN => fan(fx, t),
            _ => panic!("b770: unknown task step {f:#x}"),
        }
        fx.leave(0x20);
    }
    let e = fx.camera_eye(0);
    if fx.mem.i16(COUNTER) == 2 {
        let x = fx.mem.read_u32(e);
        fx.mem.write_u32(CAM_X, x);
    }
    // a3 as the draws' last library call left it, for the unit flash's
    // first 0x207BB0.
    let mut a3 = 0u32;
    let mut alive = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        alive += 1;
        let f = fx.mem.read_u32(t + 0x44);
        // (step, local, cell, texture, rows)
        let rings = [
            (RING_W, 0x80u32, 0x9B, D + 0x29C, 6),
            (RING_N, 0x90, 0x9B, D + 0x29C, 1),
            (RING_G, 0xA0, 0x14, TEX + 4, 1),
            (RING_R, 0xB0, 0x9B, D + 0x29C, 1),
            (RING_T, 0xC0, 0x14, TEX, 0xA),
        ];
        for (g, local, id, tex, rows) in rings {
            if f != g {
                continue;
            }
            let sub = fx.mem.i16(t + 0x56);
            l2::rect(fx, sp + local, id, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            let w = fx.mem.read_u32(tex);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, rows);
            let h = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, h);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, BYTES);
            // ring_facing leaves a3 = 0 (b098's rule) [run].
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == TINT {
            let p = sp + 0x70;
            let h = fx.self_handle();
            unit_position_saving(fx, h, p, u64::from(flashes), 0x01E0_34F8);
            let x = if fx.mem.f(CAM_X) <= 0.0 { 0x43FA_0000 } else { 0xC3FA_0000 };
            fx.mem.write_u32(p, x);
            let v = [fx.mem.f(p), fx.mem.f(p + 4), fx.mem.f(p + 8)];
            let mut xyz = fx.mem.read_u64(sp + 0x108);
            if let Some(w) = fx.project(v, None, &mut xyz) {
                fx.mem.write_u64(sp + 0x108, xyz);
                fx.mem.setf(TINT_W, w);
            }
            project_residue(fx, sp, p);
            let w = fx.mem.f(TINT_W);
            quads4::tint4(fx, t, 0x21, w);
            // tint4 leaves a3 = the flags 0x207E20 loaded (b259's) [H].
            if let Some(p) = fx.out.prims.last() {
                let d = p.desc;
                a3 = u32::from(fx.mem.read_u16(d + 2));
            }
        }
        if f == FAN {
            let p = sp + 0x70;
            let h = fx.self_handle();
            let slot = node(fx, 0);
            node_position_saving(fx, h, slot, p, u64::from(flashes), Some(u64::from(alive)), 0x01E0_366C);
            let z = fx.mem.f(p + 8);
            let dz = f32::from_bits(0x3F8C_CCCD);
            let z = if fx.mem.i16(FACING) != 0 { ee::add(z, dz) } else { ee::sub(z, dz) };
            fx.mem.setf(p + 8, z);
            let y = ee::add(fx.mem.f(p + 4), f32::from_bits(0x3ECC_CCCD));
            fx.mem.setf(p + 4, y);
            let before = fx.out.prims.len();
            spokes(fx, D, t, 0x21, p);
            // [H] the last XYZ2 drawn, as after an arc.
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
        0xAF => 1,
        0x140 => 4,
        0x136 => 2,
        0xBA..=0x12B => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E00D70 (51069f8b19a6, this record only) `spokes(D, task, mode,
/// pos)`: six gouraud strips 60 degrees apart from +0x5A, each 4 degrees
/// wide between the radii +0x48 (at height -+0x34) and +0x4C (at -+0x3C)
/// in the x / z plane, coloured +0x60 (inner) / +0x64 (outer); through
/// the task's angles +0x20 at `pos` (scale 0.1); blend (mode >> 4) (0:
/// 0x42), depth mode & 15 (0 the mean w, 1 the far Z at key 0, else key
/// -1) [F].
fn spokes(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1F0);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xA0 + 4 * k, w);
    }
    let d = sp + 0x160;
    l2::colour_strip4(fx, d, t + 0x60, 2);
    l2::colour_strip4(fx, d, t + 0x64, 1);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    let b = (mode >> 4) as i16 as i64;
    let alpha = if b == 0 { 0x80_0000_0042u64 } else { ((b << 2) as u64) | 0x80_0000_0040 };
    fx.desc_alpha(d, alpha);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0x100 + 4 * k, 0x3DCC_CCCD);
        let w = fx.mem.read_u32(sp + 0xA0 + 4 * k);
        fx.mem.write_u32(sp + 0xF0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x110, sp + 0x100, t + 0x20, sp + 0xF0);
    let depth = mode & 0xF;
    for (o, h) in [(0xB4u32, 0x34u32), (0xC4, 0x34), (0xD4, 0x3C), (0xE4, 0x3C)] {
        let y = ee::neg(fx.mem.f(t + h));
        fx.mem.setf(sp + o, y);
    }
    let (r0, r1) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let tab = d0 + 0xD8;
    let mut s2 = 0i32;
    for _ in 0..6 {
        let s4 = s2 as i16 as i32;
        let a = (fx.mem.i16(t + 0x5A) + s4) as i16 as i32;
        let e = (s4 + (fx.mem.i16(t + 0x5A) + 4)) as i16 as i32;
        for (deg, r, x, z) in [(a, r0, 0xB0u32, 0xB8u32), (a, r1, 0xD0, 0xD8), (e, r0, 0xC0, 0xC8), (e, r1, 0xE0, 0xE8)] {
            let c = l2::sin_deg(fx, tab, deg, 1);
            fx.mem.setf(sp + x, ee::mul(r, c));
            let sn = l2::sin_deg(fx, tab, deg, 0);
            fx.mem.setf(sp + z, ee::mul(r, sn));
        }
        if l2::project(fx, sp + 0xB0, SPR, sp + 0x110, d, sp + 0x150) {
            let ot = fx.mem.read_u32(d0 + 0x80);
            match depth {
                0 => {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x150)) as i32;
                    fx.submit(d, ot, key);
                }
                1 => {
                    l2::far_z_desc(fx, d, 1);
                    fx.submit(d, ot, 0);
                }
                _ => fx.submit(d, ot, -1),
            }
        }
        s2 = (s2 + 0x3C) as i16 as i32;
    }
    fx.leave(0x1F0);
}

/// 0x1E03800(D, step, 0): b272's spawn (the first free task cleared and
/// set off at D+0x250 with the colour D+0x44A4 (twice), kind D+0x4906);
/// its frame's saves the emitter's s0 = D / s1, the entry's s2 = D / s3 =
/// param, the runner's s4 (0 [H]) and the return address `ra`.
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
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// Node `k`'s point into POS (0x1F6830's frame below the emitter's: s0 =
/// D, s1 = the emitter's).
fn node_pos(fx: &mut Fx, k: u32, s1: u32, ra: u32) {
    let h = fx.self_handle();
    let slot = node(fx, k);
    node_position_saving(fx, h, slot, POS, u64::from(D), Some(u64::from(s1)), ra);
}

/// 0x1E01820(D, 0): the update counter and the emitters (frame 0x40: ra,
/// the entry's s1 = 0, s0 = 0; then s0 = D, s1 a loop count).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, 0x01E0_3118), (0x10, 0), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 5 {
        spawn(fx, TINT, 0, param, 0x01E0_185C);
    }
    if c(fx) == 0xC8 {
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, FLASH, 0, param, 0x01E0_1884);
        fx.mem.set16(SPAWN_K, 1);
        spawn(fx, FLASH, 0, param, 0x01E0_18A0);
    }
    if (0..0xBE).contains(&c(fx)) {
        for (k, ra_n, ra_r, ra_g) in [(2, 0x01E0_18D0, 0x01E0_18E8, 0x01E0_1920), (3, 0x01E0_1938, 0x01E0_1950, 0x01E0_1988)] {
            node_pos(fx, k, 0, ra_n);
            fx.mem.set16(SPAWN_K, 0);
            spawn(fx, RING_R, 0, param, ra_r);
            if c(fx) % 8 == 1 {
                spawn(fx, RING_G, 0, param, ra_g);
            }
        }
    }
    if (0xBE..0xFA).contains(&c(fx)) {
        node_pos(fx, 0, 0, 0x01E0_19BC);
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, FAN, 0, param, 0x01E0_19D4);
    }
    if (0x108..0x131).contains(&c(fx)) {
        let h = fx.target_handle();
        let slot = fx.mem.i16(TARGET_NODE);
        node_position_saving(fx, h, slot, sp + 0x30, u64::from(D), Some(0), 0x01E0_1A08);
        let h = fx.target_handle();
        unit_position_saving(fx, h, POS, u64::from(D), 0x01E0_1A1C);
        let y = fx.mem.read_u32(sp + 0x34);
        fx.mem.write_u32(POS + 4, y);
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, RING_T, 0, param, 0x01E0_1A3C);
    }
    if (0xBE..0x108).contains(&c(fx)) {
        node_pos(fx, 1, 0, 0x01E0_1A70);
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, RING_W, 0, param, 0x01E0_1A88);
        spawn(fx, RING_G, 0, param, 0x01E0_1A9C);
    }
    if c(fx) == 0xBE {
        node_pos(fx, 1, 0, 0x01E0_1AC4);
        fx.mem.set16(SPAWN_K, 1);
        for k in 0..6 {
            spawn(fx, RING_W, k, param, 0x01E0_1AE8);
        }
    }
    let n = c(fx);
    // s1 = 6 after the loop on 0xBE only.
    let s1 = if n == 0xBE { 6 } else { 0 };
    if (0x32..0xAA).contains(&n) && n % 0x10 == 1 {
        node_pos(fx, 1, s1, 0x01E0_1B64);
        fx.mem.set16(SPAWN_K, 1);
        spawn(fx, RING_N, s1, param, 0x01E0_1B80);
    }
    fx.leave(0x40);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Alpha 0x80 while young (+0x54 < +6), then out by 8 (the task ends
/// below 10).
fn hold_fade(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xA {
            fx.mem.write_u8(t + 0x63, a - 8);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
}

/// 0x1E01BA0 / 0x1E01D80 (`f`, the same body) (D, task): a red (0xC0,
/// 0x10, 0x10) facing ring: size +0x48 4 (kind 1: 10) growing by 1, row
/// 0..3, orientation 0..7, the kind in +0xC, radius from 0 growing by
/// 5..12 (kind 1: 9..20), row 0..34, life 3..7; alpha 0x80, then out by 8.
fn ring_w(fx: &mut Fx, t: u32, f: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4080_0000);
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x48, 0x4120_0000);
        }
        let v = fx.rand_from(t, f + 0x40);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0xC0, 0x10, 0x10);
        let v = fx.rand_from(t, f + 0x74);
        fx.mem.set16(t + 0xA, v % 8);
        let w = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, w);
        fx.mem.set16(t + 0x50, 0);
        let v = fx.rand_from(t, f + 0x9C);
        fx.mem.set16(t + 0xE, v % 8 + 5);
        if fx.mem.i16(t + 0xC) == 1 {
            let v = rmod(fx, t, f + 0xD0, 0xC);
            fx.mem.set16(t + 0xE, v + 9);
        }
        let v = rmod(fx, t, f + 0xF4, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0x110, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_fade(fx, t);
    grow(fx, t, 1.0);
    age(fx, t);
}

/// Size +0x48 up by `by` and the radius +0x50 by +0xE, from the second
/// update.
fn grow(fx: &mut Fx, t: u32, by: f32) {
    if fx.mem.i16(t + 0x54) != 0 {
        let g = ee::add(fx.mem.f(t + 0x48), by);
        fx.mem.setf(t + 0x48, g);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
}

/// 0x1E01F60 (D, task): [`ring_w`] of size 4 only, no wide kind.
fn ring_r(fx: &mut Fx, t: u32) {
    let f = RING_R;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4080_0000);
        let v = fx.rand_from(t, f + 0x28);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0xC0, 0x10, 0x10);
        let v = fx.rand_from(t, f + 0x5C);
        fx.mem.set16(t + 0xA, v % 8);
        let w = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, w);
        fx.mem.set16(t + 0x50, 0);
        let v = fx.rand_from(t, f + 0x84);
        fx.mem.set16(t + 0xE, v % 8 + 5);
        let v = rmod(fx, t, f + 0xA4, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xC0, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_fade(fx, t);
    grow(fx, t, 1.0);
    age(fx, t);
}

/// 0x1E020F0 (D, task): a white facing ring of size 3 (kind 1: 10), row
/// 0..3, orientation 0..7, radius 100..249 growing by 2..6 (kind 1:
/// 9..20), row 0..34, life 3..7, angle +0x5A 0..2; alpha 0x80, then out
/// by 8.
fn ring_t(fx: &mut Fx, t: u32) {
    let f = RING_T;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4040_0000);
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x48, 0x4120_0000);
        }
        let v = fx.rand_from(t, f + 0x40);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let v = fx.rand_from(t, f + 0x74);
        fx.mem.set16(t + 0xA, v % 8);
        let w = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, w);
        let v = rmod(fx, t, f + 0x98, 0x96);
        fx.mem.set16(t + 0x50, v + 0x64);
        let v = rmod(fx, t, f + 0xB8, 5);
        fx.mem.set16(t + 0xE, v + 2);
        if fx.mem.i16(t + 0xC) == 1 {
            let v = rmod(fx, t, f + 0xEC, 0xC);
            fx.mem.set16(t + 0xE, v + 9);
        }
        let v = rmod(fx, t, f + 0x110, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0x12C, 5);
        fx.mem.set16(t + 6, v + 3);
        let v = rmod(fx, t, f + 0x14C, 3);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_fade(fx, t);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E022E0 (D, task): b394's facing ring (0x1E01AA0, word for word):
/// size 4 growing by 2, row 0, grey, orientation 0..7, the kind in +0xC,
/// radius from 0 growing by 5..12 (kind 1: 9..20), row 0..34, life 3..7;
/// alpha 0x80, then out by 8.
fn ring_g(fx: &mut Fx, t: u32) {
    let f = RING_G;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4080_0000);
        fx.mem.set16(t + 0x56, 0);
        fx.rand_from(t, f + 0x2C);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = fx.rand_from(t, f + 0x50);
        fx.mem.set16(t + 0xA, v % 8);
        let w = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, w);
        fx.mem.set16(t + 0x50, 0);
        let v = fx.rand_from(t, f + 0x78);
        fx.mem.set16(t + 0xE, v % 8 + 5);
        if fx.mem.i16(t + 0xC) == 1 {
            let v = rmod(fx, t, f + 0xA4, 0xC);
            fx.mem.set16(t + 0xE, v + 9);
        }
        let v = rmod(fx, t, f + 0xC8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xE4, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_fade(fx, t);
    grow(fx, t, 2.0);
    age(fx, t);
}

/// 0x1E02490 (D, task): the violet (0x80, 0x10, 0x40) tint's alphas: both
/// in by 12 under 0x80 for 10 updates; to update 0x162 breathing (kind
/// +0x50 0: both out by 12 while above 0x10, to kind 1 once the bottom is
/// under 0x11; 1: in by 12 under 0x80, to kind 0 once the bottom is
/// full); then out by 8 (the task ends with the top under 10).
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x80, 0x10, 0x40);
        l2::rgb(fx, t + 0x64, 0x80, 0x10, 0x40);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let n = fx.mem.i16(t + 0x54);
    if n < 0xA {
        fade_in(fx, t, 0x63, 0x80, 0xC);
        fade_in(fx, t, 0x67, 0x80, 0xC);
    } else if n < 0x163 {
        if fx.mem.i16(t + 0x50) == 0 {
            let a = fx.mem.read_u8(t + 0x63);
            if a >= 0x11 {
                fx.mem.write_u8(t + 0x63, a - 0xC);
            }
            let b = fx.mem.read_u8(t + 0x67);
            if b < 0x11 {
                fx.mem.set16(t + 0x50, 1);
            } else {
                fx.mem.write_u8(t + 0x67, b - 0xC);
            }
        } else {
            fade_in(fx, t, 0x63, 0x80, 0xC);
            let b = fx.mem.read_u8(t + 0x67);
            if b < 0x80 {
                fx.mem.write_u8(t + 0x67, b + 0xC);
            } else {
                fx.mem.set16(t + 0x50, 0);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xA {
            fx.mem.write_u8(t + 0x63, a - 8);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let b = fx.mem.read_u8(t + 0x67);
        if b >= 0xA {
            fx.mem.write_u8(t + 0x67, b - 8);
        }
    }
    age(fx, t);
}

/// 0x1E02950 (D, task): a red (0xF0, 0x48, 0x48) fan of six spokes, radii
/// 40 / 10.5 (+0x48 growing by 4), facing the camera (+0x20 + 0x4000),
/// spun 0x200 a update about z (from a random +0x28), +0x5A random; the
/// outer colour (0xF0, 8, 0x80) / (0x80, 8, 0x40) by the age's parity;
/// in by 0x15 under 0x80 for 6 updates, then out by 9.
fn fan(fx: &mut Fx, t: u32) {
    let f = FAN;
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x48, 0x4220_0000);
            fx.mem.write_u32(t + 0x4C, 0x4128_0000);
        }
        camera_angles_saving(fx, t + 0x20, u64::from(t), 0, f + 0x44);
        let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
        fx.mem.write_u32(t + 0x20, v);
        let v = rmod(fx, t, f + 0x54, 0xFFFF);
        fx.mem.set32(t + 0x28, v);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x60, 0xF0, 0x48, 0x48);
        let v = rmod(fx, t, f + 0x94, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 6 {
        fade_in(fx, t, 0x67, 0x80, 0x15);
    } else {
        let a = fx.mem.read_u8(t + 0x67);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x67, a - 9);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let r = ee::add(fx.mem.f(t + 0x48), 4.0);
    fx.mem.setf(t + 0x48, r);
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x64, 0xF0, 8, 0x80);
    } else {
        l2::rgb(fx, t + 0x64, 0x80, 8, 0x40);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, f + 0x17C);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    let v = fx.mem.read_u32(t + 0x28).wrapping_add(0x200);
    fx.mem.write_u32(t + 0x28, v);
    age(fx, t);
}

/// 0x1E02C80(D, t): b232's flash sprite (0x1E01120) in grey 0x28 with the
/// descriptor at D+0x44B8, sines D+0x4B10: the attacker over the window
/// from x +8 on, x 10 cos(+0x5A) (+0x5A + 0x19 a update, from 0 or 180 by
/// +0x50); alpha rising by 10 below 0x80 until update 0x5A, then falling
/// by 9. `regs` the unit flash's s0..s2.
fn flash(fx: &mut Fx, t: u32, regs: [u32; 3]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_3D64), (0x20, u64::from(regs[2])), (0x10, u64::from(regs[1])), (0, u64::from(regs[0]))]);
    let x0 = fx.mem.i16(t + 8);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x28, 0x28, 0x28);
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.set16(t + 0x5A, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.set16(t + 0x5A, 0xB4);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, x, y) in [(0x20u32, x0 + 0x700, 0x6FF8u16), (0x30, x0 + 0x900, 0x8FF8)] {
        fx.mem.write_u16(d + at, ((x << 4) - 8) as u16);
        fx.mem.write_u16(d + at + 2, y);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    for (o, v) in [(0x18u32, 0u32), (0x1C, 0), (0x28, 0x3F80_0000), (0x2C, 0x3F80_0000)] {
        fx.mem.write_u32(d + o, v);
    }
    fx.submit(d, 0, 0);
    fx.submit(d, 0, 0);
    let a = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, TAB, a, 1);
    let v = ee::mul(10.0, c);
    fx.fix_residue(v, 0x01E0_2E34);
    fx.mem.set16(t + 8, ee::fixsfsi(v));
    let a = fx.mem.i16(t + 0x5A) + 0x19;
    fx.mem.set16(t + 0x5A, a);
    if fx.mem.i16(t + 0x54) < 0x5A {
        fade_in(fx, t, 0x63, 0x80, 0xA);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x63, a - 9);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E03C50(D): b232's unit flash (0x1E01ED0) with a 0xC0 frame (the
/// descriptor at sp + 0x68): in a packet layer (0x2078A0) the target drawn
/// into the render target (0x1F8270 filter 0; ALPHA 0x48), TEXFLUSH before
/// and after; until update 0xE6 the attacker's node 0x43 projected
/// (0x204D80) into the sprite's first XYZ2 and its w D+0x4888; every flash
/// task stepped (0x1E02C80); the layer linked into the ordering table at
/// fix(w) when any task lives.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xC0);
    let d = sp + 0x68;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.target_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0xA8, sp + 0xB0);
    fx.texflush(regs);
    if fx.mem.i16(COUNTER) < 0xE6 {
        let h = fx.self_handle();
        let slot = node(fx, 1);
        node_position_saving(fx, h, slot, sp + 0x50, 0, Some(0), 0x01E0_3D18);
        let p = sp + 0x50;
        let v = [fx.mem.f(p), fx.mem.f(p + 4), fx.mem.f(p + 8)];
        let mut xyz = fx.mem.read_u64(d + 0x20);
        if let Some(w) = fx.project(v, None, &mut xyz) {
            fx.mem.write_u64(d + 0x20, xyz);
            fx.mem.setf(FLASH_W, w);
        }
        project_residue(fx, sp, p);
    }
    let mut live = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        live += 1;
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            flash(fx, slot + 0x2A0, [k, live, slot]);
        }
    }
    if live != 0 {
        let w = fx.mem.f(FLASH_W);
        let key = ee::fixunssfsi(w) as i32;
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, key);
    }
    fx.leave(0xC0);
}
