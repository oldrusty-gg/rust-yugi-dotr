//! BMODULE b583 (card 386), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E056C0, data area a0 = 0x1ED0000, 0xAD20 bytes)
//! [F]: s6 = D, the OT (0x206E40) at sp + 0xB8.
//! Init: the handles (+4 / +8), the attacker's rotation (+0x2C), textures
//! (4, 0xF) and (5, 0..3), nodes 0xB (cleared, +0xC) / 0x14 / 0x1D / 0x26 /
//! 0x2F / 1 (+0x10..+0x20), the stage light ramp (+0xA0: 2 / 0x1A4) and the
//! random bytes (+0xAC1C). Every update the target's position (sp + 0xD0).
//! Update 0x37: five big sprites (+0x5054, `drop::drops`, 0xAF counts) on
//! nodes +0xC..+0x1C, their alpha + 8 a update to 0x48; 0x58 ten bolt
//! spawners (+0x9D18, `emit::segment_spawns_b`, into the bolts +0x7798) on
//! the same nodes; 0x78 ten sprites more (+0x5054 + 0x44 (k + 5)) thrown
//! from the nodes to points round the target in 48 updates, each with a
//! spawner and a band (+0xA428, `band::bands_b`) there; from 0x78 the
//! sprites 5..14 shed four drops (+0xA4) a update while above 1 and on
//! their last count a flash (+0x6CE8) and a spike (+0x5488); 0xA8 the
//! haze (+0xA7C0, `haze::screen`, 0xF5 counts, on the target) and a long
//! shake, the target tinted black (0x1F75F0) until 0x19A; 0xA8..0x196 twice
//! an update two bolts and a flash at a random point round the target.
//! Returns 6 on 0xA8..0x1AE, 2 on 0x1AF, 1 on 0x9E, 8 on 0x1AA (no drawing
//! then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `streaks_b`
//! 0x1E003A0 (+0x5450 never armed), `flash_b` 0x1E011A0, `spikes`
//! 0x1E01C10, `bolts_b` 0x1E026F0, `segment_spawns_b` 0x1E03180, `fade`
//! 0x1E034C0 (+0xA420 never armed), `drops` 0x1E03990, `bands_b`
//! 0x1E042F0, `frame_fade_b` 0x1E04A30 (+0xABEC never armed),
//! `haze::screen` 0x1E04EB0 [`crate::blib`].
use crate::b015::unit_tint_saving;
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const DROPS: u32 = D + 0xA4;
const BIG: u32 = D + 0x5054;
const STREAK: u32 = D + 0x5450;
const SPIKES: u32 = D + 0x5488;
const FLASH: u32 = D + 0x6CE8;
const BOLTS: u32 = D + 0x7798;
const EMIT: u32 = D + 0x9D18;
const FADE: u32 = D + 0xA420;
const BANDS: u32 = D + 0xA428;
const HAZE: u32 = D + 0xA7C0;
const WINDOW: u32 = D + 0xABEC;
const BYTES: u32 = D + 0xAC1C;
/// [H] the runner's s1 at the entry.
const RUNNER_S1: u64 = 0;

/// The entry 0x1E056C0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x170);
    fx.mem.write_u32(sp + 0xB8, OT);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x170);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0xAD20).
    fx.bzero(D, 0xAD20);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..4 {
        fx.texture(5, k);
    }
    for (k, (node, clear)) in [(0xB, 1), (0x14, 0), (0x1D, 0), (0x26, 0), (0x2F, 0), (1, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x1A4);
    // The entry's s0 = 4 (the texture loop's), s1 / s2 the runner's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5814, [Some(4), None, None]);
    0
}

/// rand() >> 4 with the entry's s0 (64 bits) saved in rand's frame.
fn rand4(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand() >> 4
}

/// (200 ((rand >> 4) % m) - half) / 100.
fn jit(fx: &mut Fx, s0: u64, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand4(fx, s0, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(200 * r - half), 100.0)
}

fn set(fx: &mut Fx, r: u32, bytes: &[(u32, u8)], words: &[(u32, u32)]) {
    for &(o, v) in bytes {
        fx.mem.write_u8(r + o, v);
    }
    for &(o, v) in words {
        fx.mem.write_u32(r + o, v);
    }
}

/// 0x20C0B0(out, 0, rot, 0): the rotation alone, its 0x70 frame (the
/// caller's s0..s3 +0x20.., ra +0x60) left below the caller's sp.
fn rot_into(fx: &mut Fx, out: u32, rot: u32, s: [u64; 4], ra: u32) {
    let r = [fx.mem.i32(rot), fx.mem.i32(rot + 4), fx.mem.i32(rot + 8)];
    let m = fx.trs(None, Some(r), None);
    fx.write_m4(out, &m);
    let f = fx.sp - 0x70;
    fx.saved(f, &[(0x20, s[0]), (0x30, s[1]), (0x40, s[2]), (0x50, s[3]), (0x60, u64::from(ra))]);
}

/// 0x20C540(out, m, v).
fn turn(fx: &mut Fx, out: u32, m: u32, v: u32) {
    let m = fx.read_m4(m);
    let x = fx.read_v4(v);
    let r = fx.transform(Some(&m), x);
    fx.write_v4(out, r);
}

/// A big sprite's common fields (+0x5054 + 0x44 k) after its count.
fn big_fields(fx: &mut Fx, r: u32) {
    set(fx, r, &[(8, 3), (0xB, 2), (9, 0), (0xA, 5)], &[]);
}

/// A bolt spawner's fields (+0x9D18 + 0x3C k): +4 `k4`, jitter `j`.
fn spawner(fx: &mut Fx, r: u32, k4: u8, j: u32) {
    fx.mem.write_u8(r + 4, k4);
    fx.mem.write_u32(r + 8, BOLTS);
    set(fx, r, &[(0xC, 0x96), (0xD, 5), (0xE, 1), (0xF, 0), (0x10, 5), (0x11, 6)], &[(0x14, j), (0x18, 0x3F80_0000), (0x1C, 0x3ECC_CCCD)]);
    set(fx, r, &[(0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20)], &[]);
}

/// Update 0x78 [F 0x1E05AA8]: ten sprites thrown from the nodes to points
/// round the target, each with a bolt spawner and a band. Returns s0.
fn throw(fx: &mut Fx, sp: u32) -> u64 {
    let mut s0 = 0i32;
    while s0 < 10 {
        let s1 = 10 * s0;
        let (_, m) = fx.div(s0, 5);
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC + 4 * m as u32));
        node_position_saving(fx, h, s, sp + 0xE0, s0 as u64, Some(s1 as u64), 0x01E0_5AD8);
        for (c, ra) in [(0u32, 0x01E0_5AE0), (4, 0x01E0_5B40)] {
            let v = rand4(fx, s0 as u64, ra);
            let r = v % 8;
            let x = ee::add(fx.mem.f(sp + 0xD0 + c), ee::div(ee::from_i32(200 * r - 0x320), 100.0));
            fx.mem.setf(sp + 0xF0 + c, x);
        }
        let z = fx.mem.read_u32(sp + 0xD8);
        fx.mem.write_u32(sp + 0xF8, z);
        let b = BIG + 0x44 * (s0 as u32 + 5);
        set_counter4(fx, b, 0x30, s1 & 0xFF, 0xFF);
        set(fx, b, &[(5, 0), (4, 0xC5)], &[]);
        big_fields(fx, b);
        set(fx, b, &[(0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)], &[(0x10, 0x43C8_0000), (0x14, 0), (0x1C, 0), (0x18, 0)]);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xE0 + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        for c in 0..3u32 {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0xF0 + 4 * c), fx.mem.f(sp + 0xE0 + 4 * c)), 48.0);
            fx.mem.setf(b + 0x2C + 4 * c, v);
        }
        let i = first_free(fx, EMIT, 0x3C, 0x1E);
        let e = EMIT.wrapping_add((0x3C * i) as u32);
        set_counter4(fx, e, 0x30, s1 & 0xFF, 1);
        fx.mem.write_u8(e + 5, 0);
        spawner(fx, e, 4, 0xBB8);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xE0 + 4 * c);
            fx.mem.write_u32(e + 0x24 + 4 * c, w);
        }
        for c in 0..3u32 {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0xF0 + 4 * c), fx.mem.f(sp + 0xE0 + 4 * c)), 48.0);
            fx.mem.setf(e + 0x30 + 4 * c, v);
        }
        let i = first_free(fx, BANDS, 0x5C, 0xA);
        if i >= 0 {
            let r = BANDS + 0x5C * i as u32;
            set_counter4(fx, r, 0x20, s1 & 0xFF, 0xFF);
            set(fx, r, &[(5, 0), (7, 0x24), (0xC, 2), (8, 0), (9, 2), (0xA, 0), (0xB, 5)], &[(0x10, 0x4040_0000), (0x14, 0x4020_0000)]);
            set(fx, r, &[(0x28, 0x80), (0x29, 0x80), (0x2A, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x2B, 0x80), (0x27, 0x80)], &[(0x3C, 0x4000), (0x18, 0x3F80_0000), (0x1C, 0x3F66_6666)]);
            copy_v4(fx, sp + 0xE0, r + 0x2C);
        }
        s0 += 1;
    }
    s0 as u64
}

/// Update 0x58 [F 0x1E05928]: ten bolt spawners on nodes +0xC..+0x1C.
fn spawners(fx: &mut Fx) {
    for s1 in 0..10u32 {
        let i = first_free(fx, EMIT, 0x3C, 0x1E);
        let e = EMIT.wrapping_add((0x3C * i) as u32);
        set_counter4(fx, e, 0x8E, 0, 0);
        fx.mem.write_u8(e + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * (s1 >> 1));
        fx.mem.write_u8(e + 6, b);
        spawner(fx, e, 6, 0xC80);
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(e + o, 0);
        }
    }
}

/// From 0x78 [F 0x1E061B8]: sprites 5..14 shed four drops a update while
/// above 1 and on their last count a flash and a spike. Returns s0.
fn shed(fx: &mut Fx, sp: u32, mut s1: u64) -> (u64, u64) {
    for s0 in 5..15u32 {
        let b = BIG + 0x44 * s0;
        if fx.mem.u8(b) >= 2 {
            for s2 in 0..4u64 {
                let i = first_free(fx, DROPS, 0x44, 0xC8);
                s1 = i as i64 as u64;
                if i < 0 {
                    continue;
                }
                let v = rand4(fx, u64::from(s0), 0x01E0_61FC);
                let (_, r) = fx.div(v, 6);
                let f20 = ee::add(16.0, ee::div(ee::from_i32(200 * r - 0x258), 100.0));
                fx.fix_residue(f20, 0x01E0_6254);
                let count = ee::fixsfsi(f20) & 0xFF;
                let p = DROPS + 0x44 * i as u32;
                set_counter4(fx, p, count, 0, 0xFF);
                set(fx, p, &[(4, 0x40), (8, 0), (0xB, 2)], &[]);
                let v = (rand4(fx, u64::from(s0), 0x01E0_6290) + 2) & 3;
                fx.mem.set8(p + 9, v);
                set(fx, p, &[(0xA, 0), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x50)], &[(0x10, 0x4270_0000), (0x14, 0), (0x1C, 0), (0x18, 0)]);
                for (k, ra) in [(0u32, 0x01E0_62D4), (4, 0x01E0_62E4), (8, 0x01E0_62F4)] {
                    let v = rand4(fx, u64::from(s0), ra) & 0xFFFF;
                    fx.mem.set32(sp + 0x150 + k, v);
                }
                rot_into(fx, sp + 0x100, sp + 0x150, [u64::from(s0), s1, s2, u64::from(b - 0x5054)], 0x01E0_6314);
                fx.mem.write_u32(sp + 0x148, 0);
                fx.mem.write_u32(sp + 0x140, 0);
                let r1 = rand4(fx, u64::from(s0), 0x01E0_6320) & 0xFF;
                s1 = r1 as u64;
                let r2 = rand4(fx, u64::from(s0), 0x01E0_632C) & 0xFF;
                fx.mem.setf(sp + 0x144, ee::div(ee::from_i32(r1 - r2), 20.0));
                turn(fx, sp + 0xF0, sp + 0x100, sp + 0x140);
                for c in 0..3u32 {
                    let v = ee::neg(ee::div(fx.mem.f(sp + 0xF0 + 4 * c), f20));
                    fx.mem.setf(p + 0x2C + 4 * c, v);
                }
                for c in 0..3u32 {
                    let v = ee::add(fx.mem.f(b + 0x20 + 4 * c), ee::mul(20.0, fx.mem.f(p + 0x2C + 4 * c)));
                    fx.mem.setf(p + 0x20 + 4 * c, v);
                }
                for o in [0x40u32, 0x3C, 0x38] {
                    fx.mem.write_u32(p + o, 0);
                }
            }
        }
        if fx.mem.u8(b) != 1 {
            continue;
        }
        let i = first_free(fx, FLASH, 0x4C, 0x24);
        if i >= 0 {
            let r = FLASH + 0x4C * i as u32;
            s1 = u64::from(r - 0x6CE8);
            set_counter4(fx, r, 0x1C, 0, 0xFF);
            set(fx, r, &[(5, 0), (4, 0x80), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x40), (9, 0x10), (8, 0x60)], &[(0x18, 0x4170_0000), (0x14, 0x41F0_0000), (0x10, 0x4348_0000)]);
            for c in 0..3u32 {
                let v = ee::add(fx.mem.f(b + 0x20 + 4 * c), fx.mem.f(b + 0x2C + 4 * c));
                fx.mem.setf(r + 0x1C + 4 * c, v);
            }
        }
        let i = first_free(fx, SPIKES, 0x68, 0x3C);
        if i >= 0 {
            let r = SPIKES + 0x68 * i as u32;
            s1 = u64::from(r - 0x5488);
            fx.mem.write_u8(r + 4, 0x85);
            set_counter4(fx, r, 0x1A, 0, 0xFF);
            set(fx, r, &[(0x27, 0x60), (0x23, 0), (5, 0), (0xA, 0), (0xB, 2), (0xC, 7), (8, 0x1E)], &[(0x14, 0x4020_0000), (0x1C, 0x4220_0000)]);
            let v = rand4(fx, u64::from(s0), 0x01E0_656C);
            fx.mem.set8(r + 9, v);
            fx.mem.write_u32(r + 0x10, BYTES);
            fx.mem.write_u32(r + 0x18, 0x3F80_0000);
            set(fx, r, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x24, 0x80)], &[]);
            for (o, ra) in [(0x58u32, 0x01E0_65AC), (0x5C, 0x01E0_65BC), (0x60, 0x01E0_65CC)] {
                let v = rand4(fx, u64::from(s0), ra) & 0x7F;
                fx.mem.set32(r + o, v);
            }
            for c in 0..3u32 {
                let v = ee::add(fx.mem.f(b + 0x20 + 4 * c), fx.mem.f(b + 0x2C + 4 * c));
                fx.mem.setf(r + 0x28 + 4 * c, v);
            }
        }
    }
    (15, s1)
}

/// 0xA8..0x196 [F 0x1E06634]: twice two bolts and a flash at a random
/// point round the target. Returns s0.
fn strikes(fx: &mut Fx, sp: u32, mut s0: u64) -> u64 {
    fx.mem.set32(sp + 0xC0, 0);
    while fx.mem.i32(sp + 0xC0) < 2 {
        for (c, ra, m, half) in [(0u32, 0x01E0_6640u32, 0xBB8, 0x5DC), (4, 0x01E0_6684, 0xBB8, 0x5DC), (8, 0x01E0_66C8, 0x5DC, 0x2EE)] {
            let v = rand4(fx, s0, ra);
            let (_, r) = fx.div(v, m);
            let x = ee::add(fx.mem.f(sp + 0xD0 + c), ee::div(ee::from_i32(r - half), 100.0));
            fx.mem.setf(sp + 0xF0 + c, x);
        }
        for s5 in 0..2 {
            let i = first_free(fx, BOLTS, 0x40, 0x96);
            if i < 0 {
                break;
            }
            s0 = u64::from((i as u32) << 6) + u64::from(D);
            let b = BOLTS + ((i as u32) << 6);
            set_counter4(fx, b, 8, s5 & 0xFF, 0xFF);
            set(fx, b, &[(5, 0), (4, 0x84)], &[]);
            for c in 0..3u32 {
                let w = fx.mem.read_u32(sp + 0xF0 + 4 * c);
                fx.mem.write_u32(b + 0x28 + 4 * c, w);
                fx.mem.write_u32(b + 0x1C + 4 * c, w);
            }
            for (c, ra) in [(0u32, 0x01E0_677C), (4, 0x01E0_67D4), (8, 0x01E0_682C)] {
                let j = jit(fx, s0, ra, 9, 0x384);
                let v = ee::add(fx.mem.f(b + 0x28 + c), j);
                fx.mem.setf(b + 0x28 + c, v);
            }
            set(fx, b, &[(8, 6)], &[(0x10, 0x3F80_0000), (0x14, 0x3ECC_CCCD)]);
            set(fx, b, &[(0xB, 1), (0xD, 4), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)], &[]);
        }
        let i = first_free(fx, FLASH, 0x4C, 0x24);
        if i >= 0 {
            s0 = u64::from(D + 0x4C * i as u32);
            let r = FLASH + 0x4C * i as u32;
            set_counter4(fx, r, 0xE, 0, 0xFF);
            set(fx, r, &[(5, 0), (4, 0x85), (7, 9), (0xF, 0xD0), (0xC, 0xD0), (0xD, 0xD0), (0xE, 0xD0), (0xA, 0), (9, 0), (8, 0x80)], &[(0x14, 0), (0x10, 0x437A_0000), (0x14, 0)]);
            copy_v4(fx, sp + 0xF0, r + 0x1C);
        }
        let k = fx.mem.i32(sp + 0xC0) + 1;
        fx.mem.set32(sp + 0xC0, k);
    }
    s0
}

fn tint(fx: &mut Fx, sp: u32, c: u8, s0: u64, ra: u32) {
    for o in 0..4u32 {
        fx.mem.write_u8(sp + 0x16C + o, c);
    }
    let t = fx.mem.i32(D + 8);
    unit_tint_saving(fx, t, sp + 0x16C, [s0, RUNNER_S1], ra);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut s0 = param as i64 as u64;
    let mut s1 = RUNNER_S1;
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xD0, s0, 0x01E0_5834);
    match fx.mem.u16(D) {
        0x1AA => return 8,
        0x19A => tint(fx, sp, 0x80, s0, 0x01E0_6150),
        0xA8 => {
            tint(fx, sp, 0, s0, 0x01E0_6004);
            set_counter4(fx, HAZE, 0xF5, 0, 0);
            set(fx, HAZE, &[(0xC, 1), (0xD, 0xCF), (0xE, 0x10), (0x12, 1), (0x10, 3), (0x11, 0)], &[]);
            let t = fx.mem.read_u32(D + 8);
            fx.mem.write_u32(HAZE + 0x14, t);
            set(fx, HAZE, &[(0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x80)], &[]);
            set_counter4(fx, SHAKE, 0xF5, 0, 0xFF);
            set(fx, SHAKE, &[(5, 0), (4, 8)], &[(0x14, 0x4000_0000), (0x18, 0x3FCC_CCCD), (0x1C, 0x3F80_0000), (0x30, 0), (0x34, 0x42A0_0000), (0x50, 0x40C0_0000)]);
            copy_v4(fx, sp + 0xD0, SHAKE + 0x20);
        }
        0x78 => {
            s0 = throw(fx, sp);
            s1 = 100;
        }
        0x58 => {
            spawners(fx);
            s1 = 10;
        }
        0x37 => {
            for s2 in 0..5u32 {
                let b = BIG + 0x44 * s2;
                set_counter4(fx, b, 0xAF, 0, 0xFF);
                fx.mem.write_u8(b + 5, 1);
                let h = fx.mem.read_u8(D + 0xC + 4 * s2);
                fx.mem.write_u8(b + 6, h);
                fx.mem.write_u8(b + 4, 0x85);
                big_fields(fx, b);
                set(fx, b, &[(0xE, 0x80), (0xD, 0x80), (0xC, 0x80), (0xF, 8)], &[(0x10, 0x43C8_0000), (0x14, 0), (0x1C, 0), (0x18, 0)]);
            }
            s0 = u64::from(D + 0x44 * 5);
            s1 = u64::from(D + 0x14);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x37..0x49).contains(&n) {
        for k in 0..5u32 {
            let a = BIG + 0x44 * k + 0xF;
            let v = fx.mem.read_u8(a).wrapping_add(8);
            fx.mem.write_u8(a, v);
        }
    }
    if fx.mem.u16(D) >= 0x78 {
        (s0, s1) = shed(fx, sp, s1);
    }
    let n = fx.mem.u16(D);
    if (0xA8..0x197).contains(&n) {
        s0 = strikes(fx, sp, s0);
        s1 = u64::from(sp + 0xF0);
    }
    shake_saving(fx, SHAKE, s0, 0x01E0_69A4, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_69B0, [Some(s0), Some(s1)]);
    let ot = fx.mem.read_u32(sp + 0xB8);
    blib::fade::fade(fx, FADE, ot);
    blib::radiate::streaks_b(fx, 0x01E0_03A0, STREAK, ot, 1);
    let (_, flash_a3) = blib::star::flash_b_a3(fx, 0x01E0_11A0, FLASH, ot, 0x24);
    let (_, spike_a3) = blib::spike::spikes_a3(fx, 0x01E0_1C10, SPIKES, ot, 0x3C);
    blib::emit::segment_spawns_b(fx, 0x01E0_3180, EMIT, 0x1E);
    // segment_spawns_b's 0x80 frame: the entry's s0 / s1 (+0x10 / +0x20)
    // and ra (+0x60), a later missing texture's stale TEX0 (as b238) [F];
    // s2..s4 not written [H].
    fx.saved(sp - 0x80, &[(0x10, s0), (0x20, s1), (0x60, 0x01E0_6A04)]);
    let (_, bolt_a3) = blib::bolt::bolts_b_a3(fx, 0x01E0_26F0, BOLTS, ot, 0x96);
    blib::band::bands_b(fx, 0x01E0_42F0, BANDS, ot, 0xA);
    let (_, drop_a3) = blib::drop::drops_a3(fx, DROPS, ot, 0xC8);
    let (_, big_a3) = blib::drop::drops_a3(fx, BIG, ot, 0xF);
    let n = fx.mem.u16(D);
    // The frame fade window is never armed: it draws nothing and keeps a3.
    blib::window::frame_fade_b(fx, 0x01E0_4A30, WINDOW, ot, n, 0);
    // The a3 the haze reports with its first TEXFLUSH [run]: the last
    // drawing call's (big sprites, drops, bolts, spikes, flashes; the
    // bands' [H] not chained), else [H] 0.
    let a3 = big_a3.or(drop_a3).or(bolt_a3).or(spike_a3).or(flash_a3).unwrap_or(0);
    blib::haze::screen(fx, 0x01E0_4EB0, HAZE, ot, 1, a3);
    let n = fx.mem.u16(D);
    if (0xA8..0x1AF).contains(&n) {
        6
    } else if n == 0x1AF {
        2
    } else if n == 0x9E {
        1
    } else {
        0
    }
}
