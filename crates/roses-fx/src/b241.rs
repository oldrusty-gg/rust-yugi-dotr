//! BMODULE b241 (card 326), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04150, data area a0 = 0x1ED0000) [F] (helper A20,
//! 1 Nov 2026): s2 = D, s1 = the parameter then the loop counts, s0 = the
//! OT. A cloud of sprites (+0x86C, 160 of 0x40 bytes, the local 0x1E020A0)
//! fed by two emitters: on 2 one between nodes +0xC / +0x10 (+0x4F8, the
//! local 0x1E02CD0), with a halo of 200 and a shake on node +0xC; on 0x55
//! a thrown one from node +0xC to the target over 40 updates (+0x55C, the
//! local 0x1E031E0), the halo and the shake following it, a bead beam
//! (+0xEC) along it and nine swept arcs (+0x24C, `sweep::sweeps`) round
//! its start; on 0x7B four more emitters fanning out at the target, four
//! bead beams, a halo of 600 and a shake. Returns 1 on 0x71, 6 on
//! 0x7B..0x8F, 2 on 0x90, 8 on 0xBC (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `halo`
//! 0x1E00440, `beads_at 40 44` 0x1E01190, `sweeps` 0x1E036A0
//! [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, arc::resample, beam::length3, exe::{aim_angles, atlas_cell, camera_vp_into, countdown, first_free, mul_mem, node_source, rot_trans_into, set_counter4, tex_rect}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const HALO: u32 = D + 0xA0;
const BEADS: u32 = D + 0xEC;
const SWEEPS: u32 = D + 0x24C;
const PATH: u32 = D + 0x4F8;
const THROWN: u32 = D + 0x55C;
const CLOUD: u32 = D + 0x86C;
const BYTES: u32 = D + 0x306C;
const PI: u32 = 0x4049_0FDB;
/// 0x3FAA3D71: the screen aspect.
const ASPECT: u32 = 0x3FAA_3D71;

/// The entry 0x1E04150 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x60);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x3170);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0xF, 1);
    fx.mem.set32(D + 0xC, r);
    // The entry's s0 = the OT, s1 = 1 (the texture loop's), s2 = D.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4238, [Some(OT), Some(1), Some(D)]);
    0
}

fn write8(fx: &mut Fx, base: u32, bytes: &[(u32, u8)]) {
    for &(o, v) in bytes {
        fx.mem.write_u8(base + o, v);
    }
}

fn write32(fx: &mut Fx, base: u32, words: &[(u32, u32)]) {
    for &(o, v) in words {
        fx.mem.write_u32(base + o, v);
    }
}

/// Update 2 [F 0x1E0429C]: the path emitter between nodes +0xC / +0x10,
/// a halo of 200 and the shake on node +0xC.
fn start(fx: &mut Fx, s1: u64) {
    let p = PATH;
    set_counter4(fx, p, 0x54, 0, 0xFF);
    fx.mem.write_u8(p + 4, 0);
    write32(fx, p, &[(8, CLOUD), (0xC, BYTES)]);
    write8(fx, p, &[(0x10, 0xA0), (0x11, 6), (0x12, 8)]);
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(p + 0x13, v);
    let v = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(p + 0x14, v);
    write8(fx, p, &[(0x15, 0), (0x16, 3), (0x17, 2), (0x18, 4)]);
    write32(fx, p, &[(0x1C, 0x3DCC_CCCD), (0x20, 0x3FCC_CCCD), (0x24, 0x3ECC_CCCD), (0x28, 0x4170_0000), (0x2C, 0x3F80_0000)]);
    write8(fx, p, &[(0x30, 0x80), (0x31, 0x80), (0x32, 0x80), (0x33, 0x40)]);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, slot, p + 0x44, u64::from(OT), Some(s1), 0x01E0_436C);
    copy_v4(fx, p + 0x44, p + 0x54);
    set_counter4(fx, HALO, 0x58, 0, 0xFF);
    fx.mem.write_u8(HALO + 5, 1);
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(HALO + 6, v);
    write8(fx, HALO, &[(4, 0x85), (7, 9)]);
    write32(fx, HALO, &[(0x10, 0x4348_0000), (0x14, 0)]);
    write8(fx, HALO, &[(0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0xC0), (0xA, 0x80), (9, 0x40), (8, 0x10)]);
    set_counter4(fx, SHAKE, 0x58, 0, 0xF);
    fx.mem.write_u8(SHAKE + 5, 1);
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(SHAKE + 6, v);
    fx.mem.write_u8(SHAKE + 4, 4);
    write32(fx, SHAKE, &[(0x14, 0x4000_0000), (0x18, 0x3FC0_0000), (0x1C, 0x3F80_0000), (0x30, 0), (0x34, 0x42A0_0000), (0x50, 0x4080_0000)]);
}

/// Update 0x55 [F 0x1E04464]: the thrown emitter from node +0xC to the
/// target (sp + 0x40) over 40 updates, the halo / shake following it, a
/// bead beam along it and nine arcs round its start.
fn throw(fx: &mut Fx, sp: u32, s1: u64) {
    let p = THROWN;
    set_counter4(fx, p, 0x28, 0, 0xFF);
    fx.mem.write_u8(p + 4, 0);
    write32(fx, p, &[(8, CLOUD), (0xC, BYTES)]);
    write8(fx, p, &[(0x10, 0xA0), (0x11, 6), (0x12, 8), (0x13, 0), (0x14, 5), (0x15, 2), (0x16, 4)]);
    write32(fx, p, &[(0x18, 0x4000_0000), (0x1C, 0x3ECC_CCCD), (0x20, 0x4270_0000)]);
    write8(fx, p, &[(0x2C, 0x80), (0x2D, 0x80), (0x2E, 0x80), (0x2F, 0x40)]);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, slot, p + 0x40, u64::from(OT), Some(s1), 0x01E0_450C);
    copy_v4(fx, p + 0x40, p + 0x50);
    copy_v4(fx, p + 0x50, p + 0x30);
    fx.mem.write_u32(p + 0x28, 0);
    for c in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x40 + 4 * c), fx.mem.f(p + 0x30 + 4 * c)), 40.0);
        fx.mem.setf(p + 0x60 + 4 * c, v);
    }
    set_counter4(fx, HALO, 0x28, 1, 0xFF);
    fx.mem.write_u8(HALO + 5, 0);
    copy_v4(fx, p + 0x30, HALO + 0x1C);
    copy_v4(fx, p + 0x60, HALO + 0x2C);
    set_counter4(fx, SHAKE, 0x28, 1, 0xF);
    fx.mem.write_u8(SHAKE + 5, 0);
    fx.mem.write_u8(SHAKE + 4, 4);
    copy_v4(fx, p + 0x30, SHAKE + 0x20);
    copy_v4(fx, p + 0x60, SHAKE + 0x40);
    let b = BEADS;
    set_counter4(fx, b, 0x12, 0, 0xFF);
    write8(fx, b, &[(4, 4), (5, 0), (8, 0x3C)]);
    let x = fx.rand_from(OT, 0x01E0_46A4);
    fx.mem.set8(b + 9, x >> 4);
    write8(fx, b, &[(0xA, 0), (0xB, 5), (0xC, 2)]);
    write32(fx, b, &[(0x1C, 0x41A0_0000), (0x20, 0), (0x10, BYTES), (0x14, 0x3F4C_CCCD), (0x18, 0x3ECC_CCCD)]);
    write8(fx, b, &[(0x27, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80)]);
    copy_v4(fx, p + 0x30, b + 0x28);
    for c in 0..3u32 {
        let v = ee::neg(fx.mem.f(p + 0x60 + 4 * c));
        fx.mem.setf(sp + 0x50 + 4 * c, v);
    }
    aim_angles(fx, sp + 0x50, b + 0x48);
    for s in 0..9u32 {
        let r = SWEEPS + 0x4C * s;
        set_counter4(fx, r, 0x14, 0, 0xFF);
        fx.mem.write_u8(r + 4, 2);
        fx.mem.write_u8(r + 8, 0);
        let x = fx.rand_from(OT, 0x01E0_47D4);
        fx.mem.set8(r + 9, (x >> 4) & 7);
        fx.mem.write_u8(r + 0xA, 2);
        fx.mem.write_u8(r + 0xF, 0);
        fx.mem.setf(r + 0x1C, ee::mul(40.0, ee::from_i32(s as i32)));
        write32(fx, r, &[(0x20, 0), (0x24, 0x41F0_0000)]);
        write8(fx, r, &[(0xB, 6), (0xC, 2)]);
        write32(fx, r, &[(0x14, 0x4170_0000), (0x18, 0x4100_0000)]);
        let x = fx.rand_from(OT, 0x01E0_492C) >> 4;
        let (_, m) = fx.div(x, 100);
        let v = ee::add(3.0, ee::div(ee::from_i32(m - 0x32), 100.0));
        fx.mem.setf(r + 0x28, v);
        fx.mem.write_u32(r + 0x2C, 0x4100_0000);
        for c in 0..3u32 {
            let v = fx.mem.read_u32(p + 0x30 + 4 * c);
            fx.mem.write_u32(r + 0x30 + 4 * c, v);
        }
        write32(fx, r, &[(0x44, 0), (0x40, 0)]);
        let (z, k) = if fx.mem.read_u32(D + 0x30) == 0x8000 {
            (ee::add(fx.mem.f(r + 0x38), fx.mem.f(r + 0x2C)), -0x4000i32)
        } else {
            (ee::sub(fx.mem.f(r + 0x38), fx.mem.f(r + 0x2C)), 0x4000)
        };
        fx.mem.setf(r + 0x38, z);
        fx.mem.set32(r + 0x3C, k);
    }
}

/// Update 0x7B [F 0x1E04B3C]: four emitters fanning out at the target,
/// four bead beams, a halo of 600 and a shake.
fn burst(fx: &mut Fx, sp: u32) {
    for s in 1..5u32 {
        let r = THROWN + 0x70 * s;
        set_counter4(fx, r, 0x28, 0, 0xFF);
        fx.mem.write_u8(r + 4, 1);
        write32(fx, r, &[(8, CLOUD), (0xC, BYTES)]);
        write8(fx, r, &[(0x10, 0xA0), (0x11, 0x14), (0x12, 8), (0x13, 0), (0x14, 4), (0x15, 2), (0x16, 2)]);
        write32(fx, r, &[(0x18, 0x3FCC_CCCD), (0x1C, 0x3ECC_CCCD), (0x20, 0x42A0_0000)]);
        write8(fx, r, &[(0x2C, 0x80), (0x2D, 0x80), (0x2E, 0x80), (0x2F, 0x20)]);
        copy_v4(fx, sp + 0x40, r + 0x50);
        copy_v4(fx, r + 0x50, r + 0x40);
        copy_v4(fx, r + 0x40, r + 0x30);
        fx.mem.write_u32(r + 0x28, 0xBCF5_C28F);
        let v = ee::div(fx.mem.f(THROWN + 0x68), 9.0);
        fx.mem.setf(r + 0x68, v);
        let z = fx.mem.f(r + 0x68);
        let (x, y) = match s {
            1 => (z, 0x3F4C_CCCDu32),
            2 => (ee::neg(z), 0x3F4C_CCCD),
            3 => (ee::sub(z, f32::from_bits(0x3E99_999A)), 0x3E99_999A),
            _ => (ee::neg(ee::sub(z, f32::from_bits(0x3E99_999A))), 0x3E99_999A),
        };
        fx.mem.setf(r + 0x60, x);
        fx.mem.write_u32(r + 0x64, y);
    }
    for s in 0..4u32 {
        let x = fx.rand_from(OT, 0x01E0_4F90) >> 4;
        let r = BEADS + 0x58 * s;
        set_counter4(fx, r, 0x18, x & 7, 0xFF);
        write8(fx, r, &[(4, 5), (5, 0), (8, 0x28)]);
        let x = fx.rand_from(OT, 0x01E0_5028);
        fx.mem.set8(r + 9, x >> 4);
        write8(fx, r, &[(0xA, 0), (0xB, 5), (0xC, 2)]);
        write32(fx, r, &[(0x1C, 0x41A0_0000), (0x20, 0), (0x10, BYTES)]);
        let v = ee::sub(f32::from_bits(0x3F19_999A), ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(s as i32)));
        fx.mem.setf(r + 0x14, v);
        write8(fx, r, &[(0x27, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80)]);
        copy_v4(fx, sp + 0x40, r + 0x28);
    }
    set_counter4(fx, HALO, 0x18, 0, 0xFF);
    fx.mem.write_u8(HALO + 5, 0);
    write8(fx, HALO, &[(4, 0x81), (7, 9)]);
    write32(fx, HALO, &[(0x10, 0x4416_0000), (0x14, 0)]);
    write8(fx, HALO, &[(0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xF, 0xC0), (0xA, 0x80), (9, 0x40), (8, 0x10)]);
    copy_v4(fx, sp + 0x40, HALO + 0x1C);
    write32(fx, HALO, &[(0x34, 0), (0x30, 0), (0x2C, 0)]);
    set_counter4(fx, SHAKE, 0x18, 0, 0xFF);
    fx.mem.write_u8(SHAKE + 5, 0);
    fx.mem.write_u8(SHAKE + 4, 1);
    fx.mem.write_u32(SHAKE + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x40, SHAKE + 0x20);
    write32(fx, SHAKE, &[(0x48, 0), (0x44, 0), (0x40, 0)]);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let s1 = param as i64 as u64;
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x40, u64::from(OT), 0x01E0_4260);
    match fx.mem.u16(D) {
        0xBC => return 8,
        0x7B => burst(fx, sp),
        0x55 => throw(fx, sp, s1),
        2 => start(fx, s1),
        _ => {}
    }
    if fx.mem.read_u8(PATH) != 0 {
        let v = ee::add(fx.mem.f(PATH + 0x28), f32::from_bits(0x3F33_3333));
        fx.mem.setf(PATH + 0x28, v);
    }
    shake_saving(fx, SHAKE, u64::from(OT), 0x01E0_5348, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::halo::halo(fx, 0x01E0_0440, HALO, OT, 1);
    blib::beam::beads_at(fx, 0x01E0_1190, 0x40, 0x44, BEADS, OT, 4);
    emit(fx, &PATH_EMIT, PATH, 1);
    emit(fx, &THROWN_EMIT, THROWN, 5);
    cloud(fx, 0x01E0_20A0, CLOUD, OT, 0xA0);
    blib::sweep::sweeps(fx, 0x01E0_36A0, SWEEPS, OT, 9);
    let n = fx.mem.u16(D);
    if (0x7B..0x90).contains(&n) {
        6
    } else if n == 0x90 {
        2
    } else if n == 0x71 {
        1
    } else {
        0
    }
}

/// The two emitters' record layouts (0x64 / 0x70 bytes).
struct Emit {
    frame: u32,
    stride: u32,
    rand_ra: u32,
    /// The path's three points (newest first) and the sample count.
    path: [u32; 3],
    samples: u32,
    /// 0x90d146ec3ef5: the newest point between two nodes (+0x13 / +0x14,
    /// lerp +0x2C); 0x3f2a6557f361: moved by a velocity +0x60 with +0x64
    /// += +0x28.
    nodes: bool,
    /// The segment's direction (0, 0.8, 0) with +4 1 (0x3f2a6557f361).
    up_flag: bool,
    /// 0x90d146ec3ef5: a direction shorter than +0x1C gets y + 0.8.
    min_len: Option<u32>,
    /// The spawned record's fields: length scale, +0x18, +0x1C sources;
    /// +0xA..+0xC bytes; colour.
    scale: u32,
    f18: u32,
    f1c: u32,
    bytes: u32,
    colour: u32,
    /// 0x3f2a6557f361: +4 0x40 copied as the spawned +4.
    flag40: bool,
}

/// 90d146ec3ef5, the local 0x1E02CD0 (324 words) [F].
const PATH_EMIT: Emit = Emit { frame: 0x2D0, stride: 0x64, rand_ra: 0x01E0_303C, path: [0x34, 0x44, 0x54], samples: 0x18, nodes: true, up_flag: false, min_len: Some(0x1C), scale: 0x20, f18: 0x24, f1c: 0x28, bytes: 0x15, colour: 0x30, flag40: false };
/// 3f2a6557f361, the local 0x1E031E0 (304 words) [F].
const THROWN_EMIT: Emit = Emit { frame: 0x2B0, stride: 0x70, rand_ra: 0x01E0_3508, path: [0x30, 0x40, 0x50], samples: 0x16, nodes: false, up_flag: true, min_len: None, scale: 0x18, f18: 0x1C, f1c: 0x20, bytes: 0x13, colour: 0x2C, flag40: true };

/// 0 or a denormal, as the EE's c.eq.s with 0 sees it.
fn is_zero(v: f32) -> bool {
    v.to_bits() & 0x7F80_0000 == 0
}

/// The emitters `emit(recs, _, n) -> 1`: per record a countdown (nothing
/// on the first count), the newest point, then one segment (fewer than
/// two samples: the newest and last points) or the samples of the curve
/// through the three points (0x102550 on sp + 0x70, samples from sp +
/// 0xD4); per segment a free cloud record of the pool +8 (+0x10 of them,
/// 0x40 bytes), counted +0x11: its length +0x14 (x scale unless 0), angles
/// +0x24 (0x101940) along the segment, origin +0x34 its start, +0x10 the
/// bytes pointer +0xC, +9 rand >> 4. Then the history moves up.
fn emit(fx: &mut Fx, e: &Emit, recs: u32, n: i32) -> i32 {
    let sp = fx.enter(e.frame);
    let mut g = recs;
    for i in 0..n {
        if i != 0 {
            g += e.stride;
        }
        emit_one(fx, e, sp, g);
    }
    fx.leave(e.frame);
    1
}

fn emit_one(fx: &mut Fx, e: &Emit, sp: u32, g: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    if fx.mem.u8(g) == fx.mem.u8(g + 2) - 1 {
        return;
    }
    let [p0, p1, p2] = e.path;
    if e.nodes {
        fx.mem.write_u8(g + 5, 1);
        let v = fx.mem.read_u8(g + 0x13);
        fx.mem.write_u8(g + 6, v);
        node_source(fx, g, sp + 0x2B0);
        let v = fx.mem.read_u8(g + 0x14);
        fx.mem.write_u8(g + 6, v);
        node_source(fx, g, sp + 0x2C0);
        for c in 0..3u32 {
            let d = ee::sub(fx.mem.f(sp + 0x2B0 + 4 * c), fx.mem.f(sp + 0x2C0 + 4 * c));
            let d = ee::mul(fx.mem.f(g + 0x2C), d);
            let v = ee::add(fx.mem.f(sp + 0x2C0 + 4 * c), d);
            fx.mem.setf(g + p0 + 4 * c, v);
        }
    } else {
        let v = ee::add(fx.mem.f(g + 0x64), fx.mem.f(g + 0x28));
        fx.mem.setf(g + 0x64, v);
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(g + p0 + 4 * c), fx.mem.f(g + 0x60 + 4 * c));
            fx.mem.setf(g + p0 + 4 * c, v);
        }
    }
    let segs = if fx.mem.u8(g + e.samples) < 2 {
        copy_v4(fx, g + p0, sp + 0xD4);
        copy_v4(fx, g + p1, sp + 0xE4);
        1
    } else {
        let p = sp + 0x70;
        fx.mem.set32(p, 3);
        for (k, src) in [p0, p1, p2].into_iter().enumerate() {
            for c in 0..3u32 {
                let w = fx.mem.read_u32(g + src + 4 * c);
                fx.mem.write_u32(p + 4 + 4 * k as u32 + 0x20 * c, w);
            }
        }
        resample(fx, p);
        fx.mem.u8(g + e.samples)
    };
    let d = sp + 0x2A0;
    for k in 0..segs.max(0) as u32 {
        let pool = fx.mem.read_u32(g + 8);
        let count = fx.mem.u8(g + 0x10);
        let i = first_free(fx, pool, 0x40, count);
        if i < 0 {
            continue;
        }
        let off = 0x40 * i as u32;
        let rec = |fx: &mut Fx| fx.mem.read_u32(g + 8).wrapping_add(off);
        let b = rec(fx);
        let c = fx.mem.u8(g + 0x11);
        set_counter4(fx, b, c, 0, 0xFF);
        let s0 = sp + 0xD4 + 0x10 * k;
        if e.up_flag && fx.mem.u8(g + 4) & 1 != 0 {
            fx.mem.write_u32(d, 0);
            fx.mem.write_u32(d + 4, 0x3F4C_CCCD);
            fx.mem.write_u32(d + 8, 0);
        } else {
            for c in 0..3u32 {
                let v = ee::sub(fx.mem.f(s0 + 0x10 + 4 * c), fx.mem.f(s0 + 4 * c));
                fx.mem.setf(d + 4 * c, v);
            }
        }
        if let Some(m) = e.min_len {
            let len = length3(fx, d);
            if ee_lt(len, fx.mem.f(g + m)) {
                let v = ee::add(fx.mem.f(d + 4), f32::from_bits(0x3F4C_CCCD));
                fx.mem.setf(d + 4, v);
            }
        }
        let len = length3(fx, d);
        let b = rec(fx);
        fx.mem.setf(b + 0x14, len);
        let s = fx.mem.f(g + e.scale);
        if !is_zero(s) {
            let b = rec(fx);
            let v = ee::mul(fx.mem.f(b + 0x14), fx.mem.f(g + e.scale));
            fx.mem.setf(b + 0x14, v);
        }
        let w = fx.mem.read_u32(g + e.f18);
        let b = rec(fx);
        fx.mem.write_u32(b + 0x18, w);
        fx.mem.write_u8(b + 4, 0);
        if e.flag40 && fx.mem.u8(g + 4) & 0x40 != 0 {
            let b = rec(fx);
            fx.mem.write_u8(b + 4, 0x40);
        }
        let b = rec(fx);
        fx.mem.write_u8(b + 5, 0);
        let v = fx.mem.read_u8(g + 0x12);
        fx.mem.write_u8(b + 8, v);
        let w = fx.mem.read_u32(g + e.f1c);
        let b = rec(fx);
        fx.mem.write_u32(b + 0x1C, w);
        let r = fx.rand_from(g, e.rand_ra);
        let b = rec(fx);
        fx.mem.set8(b + 9, r >> 4);
        for k in 0..3u32 {
            let v = fx.mem.read_u8(g + e.bytes + k);
            let b = rec(fx);
            fx.mem.write_u8(b + 0xA + k, v);
        }
        let w = fx.mem.read_u32(g + 0xC);
        let b = rec(fx);
        fx.mem.write_u32(b + 0x10, w);
        let b = rec(fx);
        for k in 0..4u32 {
            let v = fx.mem.read_u8(g + e.colour + k);
            fx.mem.write_u8(b + 0x20 + k, v);
        }
        for c in 0..3u32 {
            let w = fx.mem.read_u32(s0 + 4 * c);
            let b = rec(fx);
            fx.mem.write_u32(b + 0x34 + 4 * c, w);
        }
        let b = rec(fx);
        aim_angles(fx, d, b + 0x24);
    }
    copy_v4(fx, g + p1, g + p2);
    copy_v4(fx, g + p0, g + p1);
}

/// c9a046199d7e, the local 0x1E020A0 (780 words) [F]: `cloud(recs, ot, n)
/// -> 1` steps and draws `n` records (0x40 bytes; a 0x1D0 frame, the
/// descriptor at sp + 0x148: RGBAQ then UV / XYZ2 x 4). Per record:
/// countdown 0x101AF0; the texture (5, +0xA); the position +0x34 (or a
/// node with +5, 0x101BA0) into sp + 0x120; the camera (0x20E2B0) x the
/// angles +0x24 there (0x20C0B0 / 0x20B370); colour +0x22 / +0x21 / +0x20,
/// Q 1, alpha +0x23 (x count / 8 over the last 8 unless +4 0x40); the
/// atlas cell +0xB of layout 0 (blend +0xC). +8 sprites: the first at the
/// centre, size +0x1C; the others by the byte table +0x10 from +9: an
/// angle 360 b / 255 at radius +0x18 (libm cosf / sinf), a height +0x14
/// (elapsed + b & 3) b / 255, size +0x1C (x 1 - |y| / +0x14 / 8, at least
/// 1 / 8, above +0x14); projected (0x204D80) and on screen (0x101600),
/// half width size / (w / 256) (x ASPECT in y), the corners turned by the
/// next byte & 0x30; key 0x1A2C70(w).
fn cloud(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1D0);
    let mut g = recs;
    for i in 0..n {
        if i != 0 {
            g += 0x40;
        }
        puff(fx, at, sp, g, ot);
    }
    fx.leave(0x1D0);
    1
}

fn puff(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 0xA);
    let t = fx.texture(5, id);
    for c in 0..3u32 {
        let v = fx.mem.read_u32(g + 0x34 + 4 * c);
        fx.mem.write_u32(sp + 0x120 + 4 * c, v);
    }
    node_source(fx, g, sp + 0x120);
    camera_vp_into(fx, sp + 0xD0);
    rot_trans_into(fx, sp + 0x90, g + 0x24, sp + 0x120);
    mul_mem(fx, sp + 0xD0, sp + 0x90);
    let mut s5 = fx.mem.u8(g + 9);
    let elapsed = fx.mem.u8(g + 2) - fx.mem.u8(g);
    let d = sp + 0x148;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (o, f) in [(0x158u32, 0x22u32), (0x159, 0x21), (0x15A, 0x20)] {
        let v = fx.mem.read_u8(g + f);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0x15C, 0x3F80_0000);
    let v = fx.mem.read_u8(g + 0xC);
    fx.mem.write_u8(sp + 0x135, v);
    let cell = fx.mem.u8(g + 0xB);
    atlas_cell(fx, sp + 0x130, cell, 0);
    tex_rect(fx, d, sp + 0x130, t);
    let a = if fx.mem.u8(g) < 8 && fx.mem.u8(g + 4) & 0x40 == 0 {
        let a = fx.mem.u8(g + 0x23) >> 3;
        let c = fx.mem.u8(g);
        fx.mult(a, c)
    } else {
        fx.mem.u8(g + 0x23)
    };
    fx.mem.set8(sp + 0x15B, a);
    let byte = |fx: &mut Fx, s5: i32| {
        let p = fx.mem.read_u32(g + 0x10).wrapping_add((s5 & 0xFF) as u32);
        fx.mem.u8(p)
    };
    let mut k = 0;
    while k < fx.mem.u8(g + 8) {
        let mut size;
        if k == 0 {
            for o in [0x118u32, 0x114, 0x110] {
                fx.mem.write_u32(sp + o, 0);
            }
            size = fx.mem.f(g + 0x1C);
        } else {
            let b = byte(fx, s5);
            let deg = ee::div(ee::mul(360.0, ee::from_i32(b)), 255.0);
            s5 = (s5 + 1) & 0xFF;
            let rad = ee::div(ee::mul(f32::from_bits(PI), deg), 180.0);
            let c = blib::exe::libm_cosf(fx, rad);
            let v = ee::mul(fx.mem.f(g + 0x18), c);
            fx.mem.setf(sp + 0x110, v);
            let rad = ee::div(ee::mul(f32::from_bits(PI), deg), 180.0);
            let s = fx.libm_sinf(rad);
            let v = ee::mul(fx.mem.f(g + 0x18), s);
            fx.mem.setf(sp + 0x118, v);
            let b = byte(fx, s5);
            let h = ee::mul(fx.mem.f(g + 0x14), ee::from_i32(elapsed + (b & 3)));
            let y = ee::div(ee::mul(h, ee::from_i32(b)), 255.0);
            fx.mem.setf(sp + 0x114, y);
            s5 = (s5 + 2) & 0xFF;
            let y = fx.mem.f(sp + 0x114);
            let ay = if ee_lt(y, 0.0) { ee::neg(y) } else { y };
            if ee_le(ay, fx.mem.f(g + 0x14)) {
                size = fx.mem.f(g + 0x1C);
            } else {
                let y = fx.mem.f(sp + 0x114);
                let ay = if ee_lt(y, 0.0) { ee::neg(y) } else { y };
                let mut f = ee::sub(1.0, ee::mul(0.125, ee::div(ay, fx.mem.f(g + 0x14))));
                if ee_lt(f, 0.0) {
                    f = 0.125;
                }
                size = ee::mul(fx.mem.f(g + 0x1C), f);
            }
        }
        corner_quad(fx, at, sp, g, ot, &mut s5, &mut size);
        k += 1;
    }
}

/// One sprite of [`puff`]: projected, its corners by the next byte.
fn corner_quad(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, s5: &mut i32, size: &mut f32) {
    let m = fx.read_m4(sp + 0xD0);
    if !fx.project_at(sp + 0x110, &m, sp + 0x1C0, sp + 0x1CC) {
        return;
    }
    let xyz = fx.mem.read_u64(sp + 0x1C0);
    if !Fx::on_screen(xyz) {
        return;
    }
    *size = ee::div(*size, ee::div(fx.mem.f(sp + 0x1CC), 256.0));
    let f20 = *size;
    let p = fx.mem.read_u32(g + 0x10).wrapping_add((*s5 & 0xFF) as u32);
    let b = fx.mem.u8(p);
    *s5 = (*s5 + 1) & 0xFF;
    let x = ee::from_i32(fx.mem.u16(sp + 0x1C0));
    let y = ee::from_i32(fx.mem.u16(sp + 0x1C2));
    let hy = ee::mul(f32::from_bits(ASPECT), f20);
    // (value, return offset, two slots) for x - w, x + w, y - h, y + h.
    let (vals, slots): ([u32; 4], [[u32; 2]; 4]) = match b & 0x30 {
        0x10 => ([0x52C, 0x57C, 0x5E0, 0x644], [[0x178, 0x168], [0x198, 0x188], [0x19A, 0x17A], [0x18A, 0x16A]]),
        0x20 => ([0x69C, 0x6EC, 0x750, 0x7B4], [[0x198, 0x178], [0x188, 0x168], [0x19A, 0x18A], [0x17A, 0x16A]]),
        0x30 => ([0x80C, 0x85C, 0x8C0, 0x924], [[0x198, 0x188], [0x178, 0x168], [0x18A, 0x16A], [0x19A, 0x17A]]),
        _ => ([0x97C, 0x9CC, 0xA30, 0xA94], [[0x188, 0x168], [0x198, 0x178], [0x17A, 0x16A], [0x19A, 0x18A]]),
    };
    let vs = [ee::sub(x, f20), ee::add(x, f20), ee::sub(y, hy), ee::add(y, hy)];
    for k in 0..4 {
        fx.fix_residue(vs[k], at + vals[k]);
        let v = ee::fixunssfsi(vs[k]) as u16;
        fx.mem.write_u16(sp + slots[k][0], v);
        fx.mem.write_u16(sp + slots[k][1], v);
    }
    let z = fx.xyz_z(sp + 0x1C0);
    for o in [0x198u32, 0x188, 0x178, 0x168] {
        fx.set_xyz_z(sp + o, z);
    }
    for o in [0x19Fu32, 0x18F, 0x17F, 0x16F] {
        fx.mem.write_u8(sp + o, 0);
    }
    let w = fx.mem.f(sp + 0x1CC);
    fx.fix_residue(w, at + 0xBB0);
    let key = ee::fixunssfsi(w) as i32;
    fx.submit(sp + 0x148, ot, key);
}
