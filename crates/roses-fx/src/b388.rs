//! BMODULE b388 (card 134), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E016C0, data area 0x1ED0000, 0xB630 bytes) [F]
//!: b236's family (a pool of 400 tasks, 0x70
//! bytes at +0x2A0: alive +0x58, step +0x44 as a record address; the spawn
//! `spawn::task400_b4`) with a lens. From update 0x3D a disc task a update
//! (0x1E00CB0, from 0x137 two) and a second disc (0x1E01520) to 0x153,
//! growing by 4 a update about the attacker's node 0x32 (D+0xB1D0, moved
//! along z by 3 a update on 0xDD..0x110 by the unit's facing); they are
//! drawn in a packet layer (0x1E01CA0: TEXFLUSH, the frame copied to TBP
//! 0x2000 (0x2095A0 / 0x207F50), TEXFLUSH) as 36 quads each between their
//! two radii, textured with the copy at their own screen places (inner
//! edge shifted by the unit's height), unlinked. On update 0x3C an arc fan
//! (0x1E005F0, `billboard::arc_strips`), camera slots 4 / 5 (0x1E007F0,
//! 0x1F7E20) and a full-window frame tint (0x1E00AB0, 0x207EF0). Returns
//! 1 on update 0x107, 4 on 0x168, 2 on 0x159, 6 on 0x112..0x153, 8 once
//! the 0x16C-update timer +0xB210 has run out.
use crate::blib::{billboard, spawn};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const FAN: u32 = 0x01E0_05F0;
const CAMERA: u32 = 0x01E0_07F0;
const TINT: u32 = 0x01E0_0AB0;
const DISC_A: u32 = 0x01E0_0CB0;
const DISC_B: u32 = 0x01E0_1520;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u32 = 0x01E0_19A0;
const TIMER: u32 = D + 0xB210;
const COUNTER: u32 = D + 0xB2BC;
/// The attacker's node 0x32 (taken below update 0xB4).
const POINT: u32 = D + 0xB1D0;
/// Camera slots 4 / 5's parameters.
const CAM: u32 = D + 0xB1A0;
const SINES: u32 = D + 0xB4C0;
/// The lens' descriptors (pointers into 0x1E01CA0's frame) and layer.
const SPRITE_PTR: u32 = D + 0xB228;
const QUAD_PTR: u32 = D + 0xB22C;
const LAYER: u32 = D + 0xB224;

/// The entry 0x1E016C0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx) };
    fx.leave(0x60);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2BA, param);
    fx.mem.set32(TIMER, 0x16C);
    for (at, node, clear) in [(0xB248u32, 0x30, 1), (0xB24A, 0x31, 0), (0xB24C, 0x32, 0), (0xB24E, 0x30, 0), (0xB250, 0x2F, 0), (0xB252, 0x2C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB258, r);
    for k in 0..2u32 {
        let t = fx.texture(4, 0xE + k as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x50);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x54)), 180.0);
    fx.fix_residue(y, 0x01E0_1880);
    fx.mem.set16(D + 0xB2B8, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(k, 0x01E0_18B0);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(D + 0xB2BE + k, h - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_1904);
        fx.mem.set16(D + 0xB268 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x2E4);
        if f != 0 && f != DISC_A {
            run_task(fx, f, t + 0x2A0, [0, u64::from(D)], TASK_RA);
        }
    }
    let mut alive = 0i32;
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        alive += 1;
        if fx.mem.read_u32(t + 0x2E4) == FAN {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t + 0x2A0, 0x21, t + 0x2B0);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    lens(fx, alive);
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x107 => 1,
        0x168 => 4,
        0x159 => 2,
        0x112..=0x153 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// A task's step `f` on task `t`, called with the caller's s0 / s1 = `s`
/// from return address `ra`.
fn run_task(fx: &mut Fx, f: u32, t: u32, s: [u64; 2], ra: u32) {
    match f {
        FAN => fan(fx, t, s, ra),
        CAMERA => camera(fx, t, s, ra),
        TINT => tint(fx, t, s, ra),
        DISC_A => disc_a(fx, t, s, ra),
        DISC_B => disc_b(fx, t, s, ra),
        _ => panic!("b388: unknown task step {f:#x}"),
    }
}

/// 0x1E003E0(D, 0): the update counter +0xB2BC and the emitters (frame
/// 0x20: ra, the entry's s0 = 0).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_1960), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) < 0xB4 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0xB24C);
        fx.node_position(h, slot, POINT);
    }
    let n = fx.mem.i16(COUNTER);
    if (0xDD..0x111).contains(&n) {
        let z = fx.mem.f(POINT + 8);
        let z = if fx.mem.i16(D + 0xB2B8) != 0 { ee::sub(z, 3.0) } else { ee::add(z, 3.0) };
        fx.mem.setf(POINT + 8, z);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x3D..0x154).contains(&n) {
        task(fx, DISC_A, 0x01E0_0508);
        task(fx, DISC_B, 0x01E0_0518);
        if fx.mem.i16(COUNTER) >= 0x137 {
            task(fx, DISC_A, 0x01E0_0548);
        }
    }
    if fx.mem.i16(COUNTER) == 0x3C {
        for k in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        task(fx, FAN, 0x01E0_059C);
        task(fx, CAMERA, 0x01E0_05B0);
    }
    if fx.mem.i16(COUNTER) == 0x3C {
        task(fx, TINT, 0x01E0_05DC);
    }
    fx.leave(0x20);
}

/// 0x1E01B10(D, step, 0) from the emitter: its saves are the emitter's s0
/// = D, the entry's s1 = D / s2 = -1 and the runner's s3 / s4 = 0.
fn task(fx: &mut Fx, f: u32, ra: u32) {
    spawn::task400_b4(fx, D, f, 0, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, u64::MAX), (0x10, u64::from(D)), (0, u64::from(D))]);
}

/// Fades the byte at `a` in by `up` below `cap` while the task is younger
/// than `young`, else out by 9 (the task ends below 11).
fn fade(fx: &mut Fx, t: u32, a: u32, young: bool, up: u8, cap: u8) {
    let b = fx.mem.read_u8(a);
    if young {
        if b < cap {
            fx.mem.write_u8(a, b.wrapping_add(up));
        }
    } else if b >= 0xB {
        fx.mem.write_u8(a, b - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E005F0: the arc fan (white outside, violet inside; radius +0x4C
/// from 4 growing by 0.5 below update 0xA9 (to 100), 3.5 below 0xDC (to
/// 230), 5.5 from 0xFE (to 350)); its alpha +0x67 in by 0x60 below 0xC0
/// before update 0x154, then out by 9; faces the camera (0x205580) turned
/// by 0x4000 a update, at the point.
fn fan(fx: &mut Fx, t: u32, s: [u64; 2], ra: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, u64::from(ra)), (0x10, s[1]), (0, s[0])]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4080_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let young = fx.mem.i16(COUNTER) < 0x154;
    fade(fx, t, t + 0x67, young, 0x60, 0xC0);
    let n = fx.mem.i16(COUNTER);
    let r = fx.mem.f(t + 0x4C);
    if n < 0xA9 {
        if ee_lt(r, 100.0) {
            fx.mem.setf(t + 0x4C, ee::add(r, 0.5));
        }
    } else if n < 0xDC {
        if ee_lt(r, 230.0) {
            fx.mem.setf(t + 0x4C, ee::add(r, 3.5));
        }
    } else if n >= 0xFE && ee_lt(r, 350.0) {
        fx.mem.setf(t + 0x4C, ee::add(r, 5.5));
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_0794);
    for k in 0..4 {
        let w = fx.mem.read_u32(POINT + 4 * k);
        fx.mem.write_u32(t + 0x10 + 4 * k, w);
    }
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E007F0: camera slots 4 / 5 (0x1F7E20) from D+0xB1A0 (128, 4, 3, 5;
/// the point; 0, w from 10 up by 5 to 100, z up by 0.05 to 1, then the
/// 4 / 3 / 5 and 5 / 3 / 6 by the task's parity; from update 0x155 z down
/// by 0.1, the task's end at 0.1 or the timer at 1).
fn camera(fx: &mut Fx, t: u32, s: [u64; 2], ra: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, u64::from(ra)), (0, s[0])]);
    if fx.mem.i16(t + 0x58) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0x4040_0000), (0xC, 0x40A0_0000), (0x20, 0), (0x24, 0x4120_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.set16(t + 0x50, 0);
    }
    if fx.mem.i16(COUNTER) < 0x155 {
        for k in 0..3 {
            let w = fx.mem.read_u32(POINT + 4 * k);
            fx.mem.write_u32(CAM + 0x10 + 4 * k, w);
        }
        let w = fx.mem.f(CAM + 0x24);
        if ee_lt(w, 100.0) {
            fx.mem.setf(CAM + 0x24, ee::add(w, 5.0));
        }
    }
    let z = fx.mem.f(CAM + 0x28);
    if fx.mem.i16(COUNTER) < 0x155 {
        if ee_lt(z, 1.0) {
            fx.mem.setf(CAM + 0x28, ee::add(z, f32::from_bits(0x3D4C_CCCD)));
        } else {
            let odd = {
                let a = fx.mem.i16(t + 0x54);
                let r = a & 1;
                if a < 0 && r != 0 {
                    r - 2
                } else {
                    r
                }
            };
            let v: [u32; 3] = if odd == 1 { [0x4080_0000, 0x4040_0000, 0x40A0_0000] } else { [0x40A0_0000, 0x4040_0000, 0x40C0_0000] };
            for (k, w) in v.into_iter().enumerate() {
                fx.mem.write_u32(CAM + 4 + 4 * k as u32, w);
            }
        }
    } else if ee_le(z, f32::from_bits(0x3DCC_CCCD)) {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(z, f32::from_bits(0x3DCC_CCCD)));
    }
    if fx.mem.read_u32(TIMER) == 1 {
        fx.mem.set16(t + 0x58, 0);
    }
    let (p, ra) = if fx.mem.i16(t + 0x58) == 0 { (0, 0x01E0_0A7C) } else { (CAM, 0x01E0_0A90) };
    crate::blib::exe::camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E00AB0: a full-window sprite (far Z, ST over +0x48 / 512 .. 1 -
/// that) of the frame (0x207EF0, bilinear) in grey, ALPHA 0x44, alpha
/// +0x63 up by 0x14 below 0x50 while the timer reads 21 or more, then out
/// by 9.
fn tint(fx: &mut Fx, t: u32, s: [u64; 2], ra: u32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, u64::from(ra)), (0x10, s[1]), (0, s[0])]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4080_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x48 + k, b);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (o, xy) in [(0x58u32, 0x6FF8u16), (0x68, 0x8FF8)] {
        fx.mem.write_u16(sp + o, xy);
        fx.mem.write_u16(sp + o + 2, xy);
        let v = fx.mem.read_u64(sp + o);
        fx.mem.write_u64(sp + o, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(sp + o + 7, 0);
    }
    let s0 = ee::div(fx.mem.f(t + 0x48), 512.0);
    fx.mem.setf(sp + 0x50, s0);
    let s0 = ee::div(fx.mem.f(t + 0x48), 512.0);
    fx.mem.setf(sp + 0x54, s0);
    let s1 = ee::div(ee::sub(512.0, fx.mem.f(t + 0x48)), 512.0);
    fx.mem.setf(sp + 0x60, s1);
    let s1 = ee::div(ee::sub(512.0, fx.mem.f(t + 0x48)), 512.0);
    fx.mem.setf(sp + 0x64, s1);
    let old = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, 0x80_0000_0044);
    // a2 / a3 as 0x207E20 leaves them: the flags | 0x200, the old flags.
    fx.frame_desc(d, 1, [old | 0x200, old]);
    fx.submit(d, OT, 0);
    let young = fx.mem.i32(TIMER) >= 0x15;
    fade(fx, t, t + 0x63, young, 0x14, 0x50);
    age(fx, t);
    fx.leave(0x90);
}

/// A disc's start (0x1E00CB0 / 0x1E01520): colours, three random angles
/// (rand % 0xFFFFFF), radius 10, then each update radius +4 and the outer
/// radius `outer` x it, the alpha +0x67 in by `up` below `cap` for 6
/// updates, then out by 9; placed at the point.
fn disc(fx: &mut Fx, t: u32, s: [u64; 2], ra: u32, a: bool) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, u64::from(ra)), (0x10, s[1]), (0, s[0])]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.write_u8(t + 0x67, 0);
        l2::rgb(fx, t + 0x60, 0x38, 0x38, 0x38);
        if a {
            l2::rgb(fx, t + 0x64, 0x48, 0x28, 0x88);
        } else {
            l2::rgb(fx, t + 0x64, 0xF0, 0xEF, 0xFF);
        }
        fx.mem.set16(t + 0x58, 2);
        let sites: [u32; 3] = if a {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + 0xB24C);
            fx.node_position(h, slot, t + 0x10);
            fx.mem.write_u32(t + 0x14, 0x4204_0000);
            let v = fx.rand_from(t, 0x01E0_0D34) >> 4;
            let (_, h) = fx.div(v, 0xC8);
            fx.mem.setf(t + 0x34, ee::from_i32(h - 0x64));
            [0x01E0_0D64, 0x01E0_0D84, 0x01E0_0DA4]
        } else {
            [0x01E0_1580, 0x01E0_15A0, 0x01E0_15C0]
        };
        for (o, site) in [0x20u32, 0x24, 0x28].into_iter().zip(sites) {
            let v = fx.rand_from(t, site);
            let (_, h) = fx.div(v, 0x00FF_FFFF);
            fx.mem.set32(t + o, h);
        }
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, if a { 0xA } else { 1 });
    }
    let r = ee::add(fx.mem.f(t + 0x48), 4.0);
    fx.mem.setf(t + 0x48, r);
    let k = if a { f32::from_bits(0x3E99_999A) } else { f32::from_bits(0x3DCC_CCCD) };
    fx.mem.setf(t + 0x4C, ee::mul(k, r));
    let young = fx.mem.i16(t + 0x54) < 6;
    fade(fx, t, t + 0x67, young, if a { 0x15 } else { 0x20 }, if a { 0x80 } else { 0xC0 });
    age(fx, t);
    for k in 0..3 {
        let w = fx.mem.read_u32(POINT + 4 * k);
        fx.mem.write_u32(t + 0x10 + 4 * k, w);
    }
    fx.leave(0x30);
}

fn disc_a(fx: &mut Fx, t: u32, s: [u64; 2], ra: u32) {
    disc(fx, t, s, ra, true);
}

fn disc_b(fx: &mut Fx, t: u32, s: [u64; 2], ra: u32) {
    disc(fx, t, s, ra, false);
}

/// One ST coordinate from a projected XYZ halfword: (v - base) / 16
/// clamped to 0..512, over 512.
fn st(v: u16, base: i32) -> f32 {
    let mut f = ee::div(ee::from_i32(base - i32::from(v)), -16.0);
    if ee_lt(f, 0.0) {
        f = 0.0;
    }
    if !ee_le(f, 512.0) {
        f = 512.0;
    }
    ee::div(f, 512.0)
}

/// 0x1E00EB0(D, t): the disc as 36 quads (the lens descriptor at
/// D+0xB22C, colours +0x60 inner / +0x64 outer) between radius +0x48 at
/// height 0 and +0x4C at +0x34, from angle 0 by +0x5A degrees, each +0x5C
/// wide, through the task's TRS (rotation +0x20, place +0x10) x the
/// camera; ST = the vertices' own screen texels in the frame copy (the
/// outer ones' y by the task's i16 +0xA + 0x700), unlinked.
fn lens_draw(fx: &mut Fx, t: u32, saves: &[(u32, u64)]) {
    let sp = fx.enter(0x140);
    fx.saved(sp, saves);
    let q = fx.mem.read_u32(QUAD_PTR);
    for (k, (c, at)) in [(0x60u32, 0x10u32), (0x60, 0x28), (0x64, 0x40), (0x64, 0x58)].into_iter().enumerate() {
        let tmp = sp + 0x120 + 8 * k as u32;
        l2::rgbaq(fx, tmp, t + c);
        let (a, b) = (fx.mem.read_u32(tmp), fx.mem.read_u32(tmp + 4));
        let q = fx.mem.read_u32(QUAD_PTR);
        fx.mem.write_u32(q + at, a);
        fx.mem.write_u32(q + at + 4, b);
    }
    for k in 0..3 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(sp + 0xD0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xE0, 0, t + 0x20, sp + 0xD0);
    let (f20, f21) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let h = fx.mem.read_u32(t + 0x34);
    fx.mem.write_u32(sp + 0x94, h);
    fx.mem.write_u32(sp + 0xA4, h);
    fx.mem.write_u32(sp + 0xB4, 0);
    fx.mem.write_u32(sp + 0xC4, 0);
    let span = fx.mem.i16(t + 0x5C);
    let mut s1 = 0i32;
    for _ in 0..0x24 {
        let c = l2::sin_deg(fx, SINES, s1, 1);
        let s = l2::sin_deg(fx, SINES, s1, 0);
        fx.mem.setf(sp + 0xB0, ee::mul(c, f20));
        fx.mem.setf(sp + 0xB8, ee::mul(s, f20));
        fx.mem.setf(sp + 0x90, ee::mul(c, f21));
        fx.mem.setf(sp + 0x98, ee::mul(s, f21));
        let s5 = (span + s1) as i16 as i32;
        let c = l2::sin_deg(fx, SINES, s5, 1);
        let s = l2::sin_deg(fx, SINES, s5, 0);
        fx.mem.setf(sp + 0xC0, ee::mul(c, f20));
        fx.mem.setf(sp + 0xC8, ee::mul(s, f20));
        fx.mem.setf(sp + 0xA0, ee::mul(c, f21));
        fx.mem.setf(sp + 0xA8, ee::mul(s, f21));
        s1 = (s1 + fx.mem.i16(t + 0x5A)) as i16 as i32;
        if !l2::project(fx, sp + 0x90, 0x7000_0000, sp + 0xE0, q, sp + 0x80) {
            continue;
        }
        let y_outer = ((fx.mem.i16(t + 0xA) + 0x700) << 4) - 8;
        for (xyz, stq, yb) in [(0x20u32, 0x18u32, y_outer), (0x38, 0x30, y_outer), (0x50, 0x48, 0x6FF8), (0x68, 0x60, 0x6FF8)] {
            let q = fx.mem.read_u32(QUAD_PTR);
            let (x, y) = (fx.mem.read_u16(q + xyz), fx.mem.read_u16(q + xyz + 2));
            let (u, v) = (st(x, 0x6FF8), st(y, yb));
            fx.mem.setf(q + stq, u);
            fx.mem.setf(q + stq + 4, v);
        }
        let q = fx.mem.read_u32(QUAD_PTR);
        fx.submit(q, 0, 0);
    }
    fx.leave(0x140);
}

/// 0x1E01CA0(D): the lens layer (frame 0x130: the sprite descriptor at
/// sp + 0x48, the quad at sp + 0xA0): TEXFLUSH, ALPHA 0x48, the frame
/// copied to 0x2095A0's TBP for the quad's TEX0 / TEX1 (0x207F50, point
/// sampled), TEXFLUSH; the disc tasks stepped and drawn; the layer at key
/// 0.
fn lens(fx: &mut Fx, alive: i32) {
    let sp = fx.enter(0x130);
    // ra, the entry's s2 (past the last task), s1 = D, s0 = the count.
    fx.saved(sp, &[(0x30, 0x01E0_1A44), (0x20, u64::from(D + 0x70 * TASKS)), (0x10, u64::from(D)), (0, alive as u64)]);
    let (d, q) = (sp + 0x48, sp + 0xA0);
    fx.mem.write_u32(SPRITE_PTR, d);
    fx.mem.write_u32(QUAD_PTR, q);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u16(q + 2, 0x34);
    fx.mem.write_u16(q, 0xC);
    fx.mem.write_u64(q + 8, 0x4214_2142_1421);
    fx.mem.write_u32(q + 4, 0);
    fx.texflush([0xC, 0x34, d, 0x4_2421]);
    let old = u32::from(fx.mem.read_u16(q + 2));
    fx.desc_alpha(q, 0x80_0000_0048);
    let dest = fx.copy_dest([q, 0x48, old | 0x200, old]);
    let regs = fx.frame_texture_packet(dest, 0, q + 0x78, q + 0x80);
    fx.texflush(regs);
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        // The lens' s0 = k, s1 = the task's slot, s2 = D (its callees'
        // saves); its s3 = the entry's (400), s4 / s5 the runner's 0.
        let s = [u64::from(k), u64::from(t)];
        let draw_saves = |ra: u32| [(0x70u32, u64::from(ra)), (0x60, 0), (0x50, 0), (0x40, 0x190), (0x30, u64::from(D)), (0x20, u64::from(t)), (0x10, u64::from(k))];
        if fx.mem.read_u32(t + 0x2E4) == DISC_A {
            disc_a(fx, t + 0x2A0, s, 0x01E0_1E18);
            lens_draw(fx, t + 0x2A0, &draw_saves(0x01E0_1E24));
        }
        if fx.mem.read_u32(t + 0x2E4) == DISC_B {
            disc_b(fx, t + 0x2A0, s, 0x01E0_1E48);
            lens_draw(fx, t + 0x2A0, &draw_saves(0x01E0_1E54));
        }
    }
    let layer = fx.mem.read_u32(LAYER);
    fx.layer_end(layer, OT, 0);
    fx.leave(0x130);
}
