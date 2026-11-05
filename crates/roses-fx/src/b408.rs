//! BMODULE b408 (card 529), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06150, data area a0 = 0x1ED0000, 0x7C00 bytes)
//! [F]: b370's family with s6 = D, s7 = the
//! OT (0x206E40), s0 = the parameter then pointers, s1..s5 counters /
//! pointers. Update 2 a long flash (+0x5BD8) and a long shake at node 9,
//! two bolt emitters (+0x6F98, `emit::segment_spawns_b`, pool +0x6098) on
//! nodes 9 / 0xE; 50 the attacker's streaks (+0x430C, node 0xF); 60 the
//! fade (+0x7858), a shake at the target, a flash there and a flash at
//! node 9, eight bead beams (+0x7178) on node 9, 24 bolts started (four
//! helices of six, their points redrawn every update 60..200 by 0x102CB0
//! `bolt::helix` from node 9 to the target: radius 5 +- 2 by rand, phase
//! 90 k, step +- 47 / 67 degrees), two scrolling tubes (+0x7BA0,
//! `segtube::tubes`) on 24 points (+0x7960) node -> target, a flash at
//! the target and four bolt emitters there; 200 eight bead beams at the
//! target. Odd updates 3..189 a spike on node 9 (wider from 60), 61..179
//! a spike at the target; updates 2..37 ten sprites (+0xA4, `drop::drops`)
//! from nodes 9 / 0xE. The frame-ray record +0x4348 (`window::frame_rays_b`)
//! is stepped, never set. Returns 6 on updates 60..196, 2 on 197, 1 on
//! 50, 8 on 232 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `streaks_b`
//! 0x1E003A0, `flash_b` 0x1E011A0, `spikes` 0x1E01C10, `bolts_b`
//! 0x1E026F0, `segment_spawns_b` 0x1E03180, `beads_at` 0x1E034C0, `fade`
//! 0x1E04140, `drops` 0x1E04610, `tubes` 0x1E04F70, `frame_rays_b`
//! 0x1E058E0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const DROPS: u32 = D + 0xA4;
const STREAKS: u32 = D + 0x430C;
const RAYS: u32 = D + 0x4348;
const SPIKES: u32 = D + 0x4378;
const FLASH: u32 = D + 0x5BD8;
const BOLTS: u32 = D + 0x6098;
const EMIT: u32 = D + 0x6F98;
const BEADS: u32 = D + 0x7178;
const FADE: u32 = D + 0x7858;
const BYTES: u32 = D + 0x7860;
const PTS: u32 = D + 0x7960;
const TUBES: u32 = D + 0x7BA0;

/// The entry's saved registers s0..s5 (64 bits) as the callees' prologues
/// save them.
#[derive(Clone, Copy)]
struct R {
    s: [u64; 6],
}

/// The entry 0x1E06150 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1D0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x1D0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0x7C00).
    fx.bzero(D, 0x7C00);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..3 {
        fx.texture(5, k);
    }
    for (o, node, clear) in [(0xCu32, 9, 1), (0x10, 0xE, 0), (0x14, 0xF, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + o, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xE2);
    // The entry's s0 = 3 (the texture loop's), s1 / s2 the runner's (0).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_6260, [Some(3), Some(0), Some(0)]);
    0
}

/// rand() >> 4 with the entry's s0 saved in rand's frame.
fn rand4(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand() >> 4
}

/// node_position(self, D[at], out) with the entry's s0 / s1.
fn node_at(fx: &mut Fx, at: u32, out: u32, r: &R, ra: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + at));
    node_position_saving(fx, h, s, out, r.s[0], Some(r.s[1]), ra);
}

/// A flash record's common fields: kind `k` (+4), unit flag `unit` (+5,
/// node D[0xC] at +6), colour 0x80 / `c`, size `size`.
fn flash_fields(fx: &mut Fx, f: u32, unit: u8, k: u8, c: u8, size: u32) {
    fx.mem.write_u8(f + 5, unit);
    if unit != 0 {
        let v = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(f + 6, v);
    }
    for (o, v) in [(4u32, k), (7, 9), (0xA, 0x40), (9, 0x20), (8, 0x80), (0xF, c), (0xC, c), (0xD, c), (0xE, c)] {
        fx.mem.write_u8(f + o, v);
    }
    fx.mem.write_u32(f + 0x10, size);
}

/// Update 2 [F 0x1E062CC]: a long flash and shake at node 9 and two bolt
/// emitters on nodes D[0xC] / D[0x10].
fn start(fx: &mut Fx, r: &mut R) {
    let i = first_free(fx, FLASH, 0x4C, 0x10);
    if i >= 0 {
        r.s[0] = u64::from(0x4C * i as u32);
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 0x3A, 0, 0xFF);
        flash_fields(fx, f, 1, 0x85, 0x80, 0x4416_0000);
        fx.mem.write_u32(f + 0x14, 0);
    }
    set_counter4(fx, D + 0x40, 0xFA, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 1);
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(D + 0x46, v);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x3F99_999A);
    fx.mem.write_u32(D + 0x58, 0x3F99_999A);
    fx.mem.write_u32(D + 0x5C, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x4000_0000);
    for s0 in 0..2u32 {
        let i = first_free(fx, EMIT, 0x3C, 8);
        let e = EMIT.wrapping_add((0x3C * i) as u32);
        set_counter4(fx, e, 0x30, s0 as i32, 0);
        fx.mem.write_u8(e + 5, 1);
        let v = fx.mem.read_u8(D + 0xC + 4 * s0);
        fx.mem.write_u8(e + 6, v);
        fx.mem.write_u8(e + 4, 2);
        fx.mem.write_u32(e + 8, BOLTS);
        for (o, v) in [(0xCu32, 0x3Cu8), (0xD, 9), (0xE, 1), (0xF, 0), (0x10, 5), (0x11, 0xC)] {
            fx.mem.write_u8(e + o, v);
        }
        fx.mem.set32(e + 0x14, 0x7D0);
        fx.mem.write_u32(e + 0x18, 0x3F80_0000);
        fx.mem.write_u32(e + 0x1C, 0x3ECC_CCCD);
        for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20)] {
            fx.mem.write_u8(e + o, v);
        }
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(e + o, 0);
        }
        r.s[2] = u64::from(e);
    }
    r.s[0] = 2;
    r.s[1] = u64::from(D + 8);
}

/// Update 50 [F 0x1E0649C]: the attacker's streaks from node 0xF.
fn streaks(fx: &mut Fx) {
    let s = STREAKS;
    set_counter4(fx, s, 0x1C, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (9, 2), (0xA, 0xFF), (8, 8), (0xB, 0x18)] {
        fx.mem.write_u8(s + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(s + 0xC, h);
    for (o, v) in [(0x14u32, 0x80u8), (0x15, 0x80), (0x16, 0x80), (0x17, 0), (0x10, 0x90), (0x11, 0x90), (0x12, 0x90), (0x13, 0x40), (5, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let v = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(s + 6, v);
}

/// Eight bead-beam records on `pos` (a point to copy, or `None`: node 9
/// through node_position) [F 0x1E06728 / 0x1E06AE8]: length `len`,
/// size / drift `size` / `drift`.
fn beads(fx: &mut Fx, r: &mut R, len: u8, size: u32, drift: u32, pos: Option<u32>, ras: [u32; 2]) {
    for s1 in 0..8u32 {
        let s0 = D + 0x58 * s1;
        let b = s0 + 0x7178;
        r.s[0] = u64::from(s0);
        r.s[1] = u64::from(s1);
        set_counter4(fx, b, 0x20, s1 as i32, 0xFF);
        let c = fx.mem.read_u8(b).wrapping_sub(4);
        fx.mem.write_u8(b, c);
        for (o, v) in [(4u32, 5u8), (5, 0), (8, len), (9, s1 as u8), (0xA, 0), (0xC, 2)] {
            fx.mem.write_u8(b + o, v);
        }
        let v = rand4(fx, r.s[0], ras[0]) & 3;
        fx.mem.set8(b + 0xB, v);
        fx.mem.write_u32(b + 0x1C, size);
        fx.mem.write_u32(b + 0x20, drift);
        fx.mem.write_u32(b + 0x10, BYTES);
        fx.mem.write_u32(b + 0x14, 0x4000_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(b + o, 0x80);
        }
        match pos {
            Some(p) => copy_v4(fx, p, b + 0x28),
            None => node_at(fx, 0xC, b + 0x28, r, ras[1]),
        }
    }
    r.s[0] = u64::from(D + 0x58 * 8);
    r.s[1] = 8;
}

/// Update 60 [F 0x1E0651C]: the fade, a shake at the target (sp + 0xA0),
/// a flash there and one at node 9 (sp + 0xB0), eight bead beams on node
/// 9, the 24 bolts started, two tubes on the points, a flash and four
/// bolt emitters at the target.
fn strike(fx: &mut Fx, sp: u32, r: &mut R) {
    set_counter4(fx, FADE, 0x10, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    set_counter4(fx, D + 0x40, 0xA8, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 8);
    fx.mem.write_u32(D + 0x54, 0x3F80_0000);
    fx.mem.write_u32(D + 0x58, 0x3FB3_3333);
    fx.mem.write_u32(D + 0x5C, 0x4000_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4320_0000);
    copy_v4(fx, sp + 0xA0, D + 0x60);
    fx.mem.write_u32(D + 0x90, 0x4100_0000);
    fx.mem.write_u32(D + 0x68, 0);
    node_at(fx, 0xC, sp + 0xB0, r, 0x01E0_65BC);
    let i = first_free(fx, FLASH, 0x4C, 0x10);
    if i >= 0 {
        r.s[0] = u64::from(0x4C * i as u32);
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 0x14, 0, 0xFF);
        flash_fields(fx, f, 0, 0x80, 0xF0, 0x42C8_0000);
        fx.mem.write_u32(f + 0x14, 0x4270_0000);
        fx.mem.write_u32(f + 0x18, 0x4248_0000);
        copy_v4(fx, sp + 0xB0, f + 0x1C);
    }
    let i = first_free(fx, FLASH, 0x4C, 0x10);
    r.s[1] = 0;
    if i >= 0 {
        r.s[0] = u64::from(0x4C * i as u32);
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 0x80, 0xC, 0xFF);
        flash_fields(fx, f, 1, 0x82, 0xB0, 0x4416_0000);
        fx.mem.write_u32(f + 0x14, 0);
        fx.mem.write_u32(f + 0x18, 0);
    }
    beads(fx, r, 0x32, 0x4270_0000, 0xBF19_999A, None, [0x01E0_6770, 0x01E0_67C4]);
    // The 24 bolts: four groups of six, delays 0 / 1 / 0 / 1.
    for s5 in 0..4u32 {
        for s4 in 0..6u32 {
            let s0 = 6 * s5 + s4;
            let b = BOLTS + (s0 << 6);
            set_counter4(fx, b, 0x91, (s5 & 1) as i32, 0xFF);
            fx.mem.write_u8(b + 5, 0);
            fx.mem.write_u8(b + 4, 0);
            fx.mem.write_u8(b + 8, 9);
            fx.mem.write_u32(b + 0x10, 0x3F33_3333);
            fx.mem.write_u32(b + 0x14, 0x3E4C_CCCD);
            for (o, v) in [(0xBu32, 1u8), (0xD, 3), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)] {
                fx.mem.write_u8(b + o, v);
            }
        }
    }
    r.s = [24, 1, 24, u64::from(D + (23 << 6)), 6, 4];
    for s1 in 0..2u32 {
        let t = TUBES + 0x2C * s1;
        set_counter4(fx, t, 0x8C, 0, 0xFF);
        for (o, v) in [(7u32, 0xCu8), (8, 0x17), (0xB, 8), (9, 2), (0xA, 2)] {
            fx.mem.write_u8(t + o, v);
        }
        fx.mem.write_u32(t + 0xC, PTS);
        fx.mem.write_u32(t + 0x14, 0x3F80_0000);
        fx.mem.write_u32(t + 0x18, 0x3CA3_D70A);
        fx.mem.write_u32(t + 0x1C, 0x3E99_999A);
        if s1 != 0 {
            // Written 2.0 then 1.0.
            fx.mem.write_u32(t + 0x10, 0x3F80_0000);
            fx.mem.write_u32(t + 0x20, 0x3F4C_CCCD);
            for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
                fx.mem.write_u8(t + o, 0xC0);
            }
        } else {
            fx.mem.write_u32(t + 0x10, 0x4000_0000);
            fx.mem.write_u32(t + 0x20, 0x3FCC_CCCD);
            for o in [0x28u32, 0x29, 0x2A] {
                fx.mem.write_u8(t + o, 0x80);
            }
            fx.mem.write_u8(t + 0x2B, 0x50);
        }
        fx.mem.write_u32(t + 0x24, 0);
    }
    r.s[0] = u64::from(D + 0x58);
    r.s[1] = 0;
    let i = first_free(fx, FLASH, 0x4C, 0x10);
    if i >= 0 {
        r.s[0] = u64::from(0x4C * i as u32);
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 0x96, 0, 0xFF);
        fx.mem.write_u8(f + 5, 0);
        let v = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(f + 6, v);
        flash_fields(fx, f, 0, 0x85, 0xC0, 0x447A_0000);
        fx.mem.write_u32(f + 0x14, 0);
        copy_v4(fx, sp + 0xA0, f + 0x1C);
    }
    for s1 in 0..4u32 {
        let e = EMIT + 0x3C * s1;
        set_counter4(fx, e, 0x8C, s1 as i32, 3);
        fx.mem.write_u8(e + 5, 0);
        fx.mem.write_u32(e + 8, BOLTS);
        fx.mem.write_u8(e + 4, 0);
        for (o, v) in [(0xCu32, 0x3Cu8), (0xD, 0xC), (0xE, 1), (0xF, 0), (0x10, 6), (0x11, 0x12)] {
            fx.mem.write_u8(e + o, v);
        }
        fx.mem.set32(e + 0x14, 0x1770);
        fx.mem.write_u32(e + 0x18, 0x3F4C_CCCD);
        fx.mem.write_u32(e + 0x1C, 0x3ECC_CCCD);
        for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x30)] {
            fx.mem.write_u8(e + o, v);
        }
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
            fx.mem.write_u32(e + 0x24 + 4 * c, w);
        }
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(e + o, 0);
        }
    }
    r.s[0] = u64::from(D + 0x3C * 4);
    r.s[1] = 4;
}

/// Odd updates 3..189 (node 9) / 61..179 (the target, sp + 0xA0) [F
/// 0x1E06BE4 / 0x1E06F88]: a spike.
fn spike(fx: &mut Fx, sp: u32, r: &mut R, near: bool, ra: [u32; 4]) {
    let i = first_free(fx, SPIKES, 0x68, 0x3C);
    if i < 0 {
        return;
    }
    let s0 = 0x68 * i as u32;
    r.s[0] = u64::from(s0);
    let q = SPIKES + s0;
    set_counter4(fx, q, if near { 0x12 } else { 0x18 }, 0, 0xFF);
    fx.mem.write_u8(q + 4, 5);
    if near {
        if fx.mem.u16(D) < 0x3C {
            fx.mem.write_u32(q + 0x14, 0x4000_0000);
            fx.mem.write_u32(q + 0x1C, 0x41A0_0000);
            fx.mem.write_u8(q + 8, 0x10);
        } else {
            fx.mem.write_u32(q + 0x14, 0x4060_0000);
            fx.mem.write_u32(q + 0x1C, 0x420C_0000);
            fx.mem.write_u8(q + 8, 0x14);
        }
        fx.mem.write_u8(q + 5, 1);
        let v = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(q + 6, v);
    } else {
        fx.mem.write_u32(q + 0x14, 0x4060_0000);
        fx.mem.write_u32(q + 0x1C, 0x4248_0000);
        fx.mem.write_u8(q + 8, 0x14);
        fx.mem.write_u8(q + 5, 0);
    }
    for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 3)] {
        fx.mem.write_u8(q + o, v);
    }
    let x = rand4(fx, r.s[0], ra[0]);
    fx.mem.set8(q + 9, x);
    fx.mem.write_u32(q + 0x10, BYTES);
    fx.mem.write_u32(q + 0x18, 0x3F80_0000);
    let (a, b) = if near { (0x10u8, 0x40u8) } else { (0x20, 0x80) };
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, a), (0x27, b)] {
        fx.mem.write_u8(q + o, v);
    }
    for (o, ra) in [(0x58u32, ra[1]), (0x5C, ra[2]), (0x60, ra[3])] {
        let v = rand4(fx, r.s[0], ra) & 0x7F;
        fx.mem.set32(q + o, v);
    }
    if !near {
        copy_v4(fx, sp + 0xA0, q + 0x28);
    }
}

/// Updates 60..200 [F 0x1E06D44]: the 24 tube points from node 9 (sp +
/// 0xB0, copied to sp + 0xC0) to the target, and the four helices of six
/// bolt segments (sp + 0xD0: n 6, radius 5 + (2 (rand & 1) - 2), phase 90
/// k, step 47 / -67 by k's parity, out sp + 0x100).
fn bolt_points(fx: &mut Fx, sp: u32, r: &mut R) {
    node_at(fx, 0xC, sp + 0xB0, r, 0x01E0_6D54);
    copy_v4(fx, sp + 0xB0, sp + 0xC0);
    r.s = [r.s[0], u64::from(sp + 0xB4), u64::from(sp + 0xB8), u64::from(sp + 0xA8), r.s[4], u64::from(sp + 0xA4)];
    let step: [f32; 3] = std::array::from_fn(|c| {
        let c = 4 * c as u32;
        ee::div(ee::sub(fx.mem.f(sp + 0xA0 + c), fx.mem.f(sp + 0xB0 + c)), 24.0)
    });
    for k in 0..0x18u32 {
        let fk = ee::from_i32(k as i32);
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0xB0 + 4 * c), ee::mul(step[c as usize], fk));
            fx.mem.setf(PTS + 12 * k + 4 * c, v);
        }
    }
    let p = sp + 0xD0;
    let mut s0 = 0u32;
    for s4 in 0..4u32 {
        r.s[0] = u64::from(s0);
        fx.mem.set32(p, 6);
        let m = rand4(fx, r.s[0], 0x01E0_6E28) & 1;
        let f3 = ee::div(ee::from_i32(200 * m - 200), 100.0);
        fx.mem.setf(p + 4, ee::add(5.0, f3));
        fx.mem.setf(p + 8, ee::mul(90.0, ee::from_i32(s4 as i32)));
        fx.mem.write_u32(p + 0xC, if s4 & 1 != 0 { 0x423C_0000 } else { 0xC286_0000 });
        for (dst, src) in [(0x10u32, 0xB0u32), (0x14, 0xB4), (0x18, 0xB8), (0x1C, 0xA0), (0x20, 0xA4), (0x24, 0xA8)] {
            let w = fx.mem.read_u32(sp + src);
            fx.mem.write_u32(p + dst, w);
        }
        fx.mem.write_u32(p + 0x28, sp + 0x100);
        blib::bolt::helix(fx, p);
        for k in 0..6u32 {
            let b = BOLTS + (s0 << 6);
            for c in 0..6u32 {
                let w = fx.mem.read_u32(sp + 0x100 + 0xC * k + 4 * c);
                fx.mem.write_u32(b + 0x1C + 4 * c, w);
            }
            s0 += 1;
        }
    }
    r.s[0] = u64::from(s0);
    r.s[4] = 4;
}

/// ((rand >> 4) % 30 x 200 - 3000) / 100.
fn jit(fx: &mut Fx, s0: u64, ra: u32) -> f32 {
    let v = rand4(fx, s0, ra);
    let (_, r) = fx.div(v, 0x1E);
    ee::div(ee::from_i32(200 * r - 0xBB8), 100.0)
}

/// Updates 2..37 [F 0x1E070D0]: ten sprites from node 9 / 0xE
/// (alternating D[0xC] / D[0x10]), thrown outwards.
fn sprites(fx: &mut Fx, sp: u32, r: &mut R) {
    for s3 in 0..10u32 {
        let i = first_free(fx, DROPS, 0x44, 0xC8);
        r.s[0] = i as i64 as u64;
        if i < 0 {
            continue;
        }
        node_at(fx, 0xC + 4 * (s3 & 1), sp + 0xB0, r, 0x01E0_7108);
        let v = rand4(fx, r.s[0], 0x01E0_7110);
        let (_, m) = fx.div(v, 6);
        let f20 = ee::add(12.0, ee::div(ee::from_i32(200 * m - 0x258), 100.0));
        fx.fix_residue(f20, 0x01E0_7168);
        let count = ee::fixsfsi(f20) & 0xFF;
        let s0 = D + 0x44 * i as u32;
        r.s[0] = u64::from(s0);
        let p = s0 + 0xA4;
        set_counter4(fx, p, count, 0, 0xFF);
        fx.mem.write_u8(p + 4, 0x41);
        fx.mem.write_u8(p + 8, 0);
        fx.mem.write_u8(p + 0xB, 2);
        let v = rand4(fx, r.s[0], 0x01E0_71A4) & 7;
        fx.mem.set8(p + 9, v);
        fx.mem.write_u8(p + 0xA, 0);
        for o in [0xCu32, 0xD, 0xE] {
            fx.mem.write_u8(p + o, 0x80);
        }
        fx.mem.write_u8(p + 0xF, 0x60);
        fx.mem.write_u32(p + 0x10, 0x42A0_0000);
        fx.mem.write_u32(p + 0x14, 0xC0A0_0000);
        fx.mem.write_u32(p + 0x1C, 0);
        fx.mem.write_u32(p + 0x18, 0);
        for (c, ra) in [(0u32, 0x01E0_71E8), (4, 0x01E0_723C), (8, 0x01E0_7290)] {
            let f = jit(fx, r.s[0], ra);
            fx.mem.setf(sp + 0xC0 + c, f);
        }
        r.s[1] = u64::from(sp + 0xC0);
        r.s[2] = u64::from(sp + 0xC4);
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0xB0 + 4 * c), fx.mem.f(sp + 0xC0 + 4 * c));
            fx.mem.setf(p + 0x20 + 4 * c, v);
        }
        for c in 0..3u32 {
            let v = ee::neg(ee::div(fx.mem.f(sp + 0xC0 + 4 * c), f20));
            fx.mem.setf(p + 0x2C + 4 * c, v);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(p + o, 0);
        }
    }
    r.s[3] = 10;
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    // The entry's s0 = the parameter (64 bits), s1..s5 the runner's 0.
    let mut r = R { s: [param as i64 as u64, 0, 0, 0, 0, 0] };
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xA0, r.s[0], 0x01E0_6280);
    match fx.mem.u16(D) {
        0xE8 => return 8,
        0xC8 => beads(fx, &mut r, 0x28, 0x42F0_0000, 0xBF4C_CCCD, Some(sp + 0xA0), [0x01E0_6B30, 0]),
        0x3C => strike(fx, sp, &mut r),
        0x32 => streaks(fx),
        2 => start(fx, &mut r),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0xBE).contains(&n) && n & 1 != 0 {
        spike(fx, sp, &mut r, true, [0x01E0_6CA4, 0x01E0_6CF0, 0x01E0_6D04, 0x01E0_6D18]);
    }
    let n = fx.mem.u16(D);
    if (0x3C..0xC9).contains(&n) {
        bolt_points(fx, sp, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0x3C..0xB4).contains(&n) && n & 1 != 0 {
        spike(fx, sp, &mut r, false, [0x01E0_700C, 0x01E0_7054, 0x01E0_7068, 0x01E0_707C]);
    }
    let n = fx.mem.u16(D);
    if (2..0x26).contains(&n) {
        sprites(fx, sp, &mut r);
    }
    shake_saving(fx, D + 0x40, r.s[0], 0x01E0_7364, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_7370, [Some(r.s[0]), Some(r.s[1])]);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the entry's s0..s6 (s6 = D) and ra.
    fx.saved(sp - 0xF0, &[(0x10, r.s[0]), (0x20, r.s[1]), (0x30, r.s[2]), (0x40, r.s[3]), (0x50, r.s[4]), (0x60, r.s[5]), (0x70, u64::from(D)), (0x80, 0x01E0_737C)]);
    blib::radiate::streaks_b(fx, 0x01E0_03A0, STREAKS, OT, 1);
    // The ray record is never set: only its countdown runs (a3 / f20 / f21
    // not reached).
    blib::window::frame_rays_b(fx, 0x01E0_58E0, RAYS, OT, 1, 0, [0, 0]);
    blib::star::flash_b(fx, 0x01E0_11A0, FLASH, OT, 0x10);
    blib::spike::spikes(fx, 0x01E0_1C10, SPIKES, OT, 0x3C);
    blib::emit::segment_spawns_b(fx, 0x01E0_3180, EMIT, 8);
    // segment_spawns_b's 0x80 frame saves s0..s4 and ra (b370: the stale
    // TEX1 / TEX0 of bolts_b's missing-texture strips).
    fx.saved(sp - 0x80, &[(0x10, r.s[0]), (0x20, r.s[1]), (0x30, r.s[2]), (0x40, r.s[3]), (0x50, r.s[4]), (0x60, 0x01E0_73CC)]);
    blib::bolt::bolts_b(fx, 0x01E0_26F0, BOLTS, OT, 0x3C);
    blib::beam::beads_at(fx, 0x01E0_34C0, 0x34, 0x3C, BEADS, OT, 0x14);
    blib::drop::drops(fx, DROPS, OT, 0xC8);
    blib::segtube::tubes(fx, 0x01E0_4F70, TUBES, OT, 2);
    let n = fx.mem.u16(D);
    if (0x3C..0xC5).contains(&n) {
        6
    } else if n == 0xC5 {
        2
    } else if n == 0x32 {
        1
    } else {
        0
    }
}
