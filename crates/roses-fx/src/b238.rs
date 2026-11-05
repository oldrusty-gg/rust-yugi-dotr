//! BMODULE b238 (card 141), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05550, data area a0 = 0x1ED0000, 0x75C0 bytes)
//! [F]: s6 = D, the OT (0x206E40) at sp + 0xB8,
//! s0 the parameter then sp + 0x100 (the jitter vector, fp / s7 its y /
//! z). Init: the handles (+4 / +8), the attacker's rotation (+0x2C),
//! textures (4, 0xF) and (5, 0..2), nodes 0x2B (cleared, +0xC) / 0x34 /
//! 0x3E / 0x47 / 0x50 / 0x59 (+0x10..+0x20), the stage light ramp (+0xA0:
//! 2 / 0x2B6) and the random bytes (+0x74C0). Every update the lift +0x74AC
//! (2.3 + 0.2 (n & 3) below update 440, + 0.08 a update to 524), the
//! target (sp + 0xD0), nodes +0xC / +0x10 (sp + 0xF0 / + 0x100) and their
//! midpoint (sp + 0xE0, z -+ the lift by D+0x30). Update 2: twenty
//! scatter drops (+0xAC, `drop::scatter_drops_b`, 0x3C bytes, on nodes
//! +0xC / +0x10) and a shake at the midpoint; 100 four flashes (+0x46FC,
//! `star::flash_b`) on nodes +0x14..+0x20; 160 forty more drops (nodes
//! +0x14..+0x20), four flashes there, the dome (+0x736C, `dome::domes`)
//! at the midpoint, four five-segment arcs (0x102F20, b173's
//! `arc_points`) from nodes +0x14..+0x20 to the midpoint as bolts (+0x51AC,
//! `bolt::bolts_b`) and eight bead beams (+0x6C8C, `beam::beads_at`);
//! 350 everything cut short; 534 the fade (+0xA4), a flash and eight bead
//! beams on node +0xC; 582 a spike, a flash and a shake at the target.
//! While the dome lives it follows the midpoint (from 534 drifting to the
//! target over 48 updates, six sprites (+0xEBC, `drop::drops`) a update
//! from it) and then the target (growing 0.7 a update), its alpha + 2 to
//! 0x80. Updates 80..159 eight sprites from nodes +0x14..+0x20; even
//! updates 100..533 spikes (+0x2E9C, `spike::spikes`) on nodes
//! +0x14..+0x20, 160..581 at the midpoint; 582..642 two bursts of three
//! bolts and a flash about the target. Returns 6 on updates 582..642, 2 on
//! 643, 1 on 572, 8 on 700 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash_b`
//! 0x1E003A0, `spikes` 0x1E00E10, `bolts_b` 0x1E018F0,
//! `segment_spawns_b` 0x1E02380 (its pool +0x6AAC never armed),
//! `scatter_drops_b` 0x1E026C0, `beads_at` 0x1E03060, `drops` 0x1E03CE0,
//! `domes` 0x1E04640, `fade` 0x1E05080 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const FADE: u32 = D + 0xA4;
const SCATTER: u32 = D + 0xAC;
const DROPS: u32 = D + 0xEBC;
const SPIKES: u32 = D + 0x2E9C;
const FLASH: u32 = D + 0x46FC;
const BOLTS: u32 = D + 0x51AC;
const EMIT: u32 = D + 0x6AAC;
const BEADS: u32 = D + 0x6C8C;
const DOME: u32 = D + 0x736C;
const LIFT: u32 = D + 0x74AC;
const DRIFT: u32 = D + 0x74B0;
const BYTES: u32 = D + 0x74C0;
/// [H] the runner's s1 at the entry.
const RUNNER_S1: u64 = 0;

/// The entry 0x1E05550 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x190);
    fx.mem.write_u32(sp + 0xB8, OT);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x190);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0x75C0).
    fx.bzero(D, 0x75C0);
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
    for (k, (node, clear)) in [(0x2B, 1), (0x34, 0), (0x3E, 0), (0x47, 0), (0x50, 0), (0x59, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x2B6);
    // The entry's s0 = 3 (the texture loop's), s1 / s2 the runner's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_56A4, [Some(3), None, None]);
    0
}

/// rand() >> 4 with the entry's s0 (sp + 0x100).
fn rand4(fx: &mut Fx, sp: u32, ra: u32) -> i32 {
    fx.rand_from(sp + 0x100, ra) >> 4
}

/// ((rand >> 4) % m x 200 - half) / 100.
fn jit(fx: &mut Fx, sp: u32, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand4(fx, sp, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(200 * r - half), 100.0)
}

/// A scatter drop (+0xAC + 0x3C k) on node D[0xC + 4 (k / 10)], `w` long.
fn scatter(fx: &mut Fx, k: u32, w: i32) {
    let r = SCATTER + 0x3C * k;
    let (_, m) = fx.div(k as i32, 7);
    set_counter4(fx, r, 0xFF, m & 0xFF, 7);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC + 4 * (k / 10));
    fx.mem.write_u8(r + 6, b);
    for (o, v) in [(4u32, 0u8), (8, 0), (9, 4), (0xA, 2), (0xB, 0), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, 0x41A0_0000);
    fx.mem.set32(r + 0x14, w);
}

/// The shake (+0x40): `count` counts at the position `pos` (written by
/// the caller), kind `kind`, amplitude `amp`, rate `rate`.
fn shake(fx: &mut Fx, count: i32, kind: u8, amp: u32, rate: u32) {
    set_counter4(fx, SHAKE, count, 0, 0xFF);
    fx.mem.write_u8(SHAKE + 5, 0);
    fx.mem.write_u8(SHAKE + 4, kind);
    fx.mem.write_u32(SHAKE + 0x14, amp);
    fx.mem.write_u32(SHAKE + 0x18, amp);
    fx.mem.write_u32(SHAKE + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(SHAKE + 0x30, 0);
    fx.mem.write_u32(SHAKE + 0x34, 0x4220_0000);
    fx.mem.write_u32(SHAKE + 0x50, rate);
}

/// A flash record's bytes: (offset, value) pairs then the sizes.
fn flash_set(fx: &mut Fx, f: u32, bytes: &[(u32, u8)], words: &[(u32, u32)]) {
    for &(o, v) in bytes {
        fx.mem.write_u8(f + o, v);
    }
    for &(o, v) in words {
        fx.mem.write_u32(f + o, v);
    }
}

/// Update 2 [F 0x1E05860].
fn start(fx: &mut Fx, sp: u32) {
    for k in 0..0x14 {
        scatter(fx, k, 0x12C);
    }
    shake(fx, 0xFF, 4, 0x3FCC_CCCD, 0x4080_0000);
    copy_v4(fx, sp + 0xE0, SHAKE + 0x20);
}

/// Update 100 [F 0x1E05980]: four flashes on nodes +0x14..+0x20.
fn flashes(fx: &mut Fx) {
    for k in 0..4u32 {
        let f = FLASH + 0x4C * k;
        set_counter4(fx, f, 0xFF, 8, 0xFF);
        fx.mem.write_u8(f + 5, 1);
        let b = fx.mem.read_u8(D + 0x14 + 4 * k);
        fx.mem.write_u8(f + 6, b);
        flash_set(
            fx,
            f,
            &[(4, 0x85), (7, 9)],
            &[(0x10, 0x4316_0000), (0x14, 0)],
        );
        flash_set(fx, f, &[(0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x80), (8, 0x20), (0xF, 0xC0)], &[(0x44, 0), (0x40, 0), (0x3C, 0)]);
    }
}

/// Update 160 [F 0x1E05A1C].
fn burst(fx: &mut Fx, sp: u32) {
    fx.mem.write_u8(SHAKE, 0xFF);
    fx.mem.write_u8(SHAKE + 4, 8);
    for k in 0x14..0x3C {
        scatter(fx, k, 0x15E);
    }
    for k in 0..4u32 {
        let i = first_free(fx, FLASH, 0x4C, 0x24);
        let f = FLASH.wrapping_add((0x4C * i) as u32);
        set_counter4(fx, f, 0x12, 0, 0xFF);
        fx.mem.write_u8(f + 5, 1);
        let b = fx.mem.read_u8(D + 0x14 + 4 * k);
        fx.mem.write_u8(f + 6, b);
        flash_set(fx, f, &[(4, 0x80), (7, 9)], &[(0x10, 0x4248_0000), (0x14, 0x41A0_0000)]);
        flash_set(fx, f, &[(0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x60), (9, 0x60), (8, 0), (0xF, 0xA0)], &[(0x44, 0), (0x40, 0), (0x3C, 0), (0x18, 0x4120_0000)]);
    }
    // The dome (one of the loop's: blez after the first).
    set_counter4(fx, DOME, 0xFF, 0x18, 0xFF);
    for (o, v) in [(0xAu32, 8u8), (7, 9), (8, 2), (9, 2)] {
        fx.mem.write_u8(DOME + o, v);
    }
    fx.mem.write_u32(DOME + 0xC, 0x4000_0000);
    fx.mem.write_u32(DOME + 0x10, 0xBD23_D70A);
    fx.mem.write_u32(DOME + 0x14, 0x4020_0000);
    fx.mem.write_u32(DOME + 0x18, 0);
    copy_v4(fx, sp + 0xE0, DOME + 0x20);
    for o in [0x1Cu32, 0x1D, 0x1E] {
        fx.mem.write_u8(DOME + o, 0x80);
    }
    fx.mem.write_u8(DOME + 0x1F, 0);
    // Four arcs of five bolts from nodes +0x14..+0x20 to the midpoint.
    let (f20, f21) = (fx.mem.read_u32(sp + 0xE4), fx.mem.read_u32(sp + 0xE8));
    let p = sp + 0x110;
    for s1 in 0..4u32 {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14 + 4 * s1));
        node_position_saving(fx, h, s, sp + 0xF0, u64::from(sp + 0x100), Some(u64::from(s1)), 0x01E0_5C64);
        fx.mem.set32(p, 5);
        fx.mem.write_u32(p + 4, 0x40A0_0000);
        let phase = [0x4334_0000u32, 0x4302_0000, 0, 0x4234_0000][s1 as usize];
        fx.mem.write_u32(p + 8, phase);
        fx.mem.write_u32(p + 0x28, sp + 0x140);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xF0 + 4 * c);
            fx.mem.write_u32(p + 0x10 + 4 * c, w);
        }
        let x = fx.mem.read_u32(sp + 0xE0);
        fx.mem.write_u32(p + 0x1C, x);
        fx.mem.write_u32(p + 0x20, f20);
        fx.mem.write_u32(p + 0x24, f21);
        crate::b173::arc_points(fx, p);
        let mut s4 = 0;
        while s4 < fx.mem.i32(p) {
            let i = first_free(fx, BOLTS, 0x40, 0x64);
            if i < 0 {
                break;
            }
            let b = BOLTS + 0x40 * i as u32;
            set_counter4(fx, b, 0xFF, s4 & 0xFF, 0xFF);
            fx.mem.write_u8(b + 5, 0);
            fx.mem.write_u8(b + 4, 1);
            let q = sp + 0x140 + 0xC * s4 as u32;
            for c in 0..3u32 {
                let w = fx.mem.read_u32(q + 4 * c);
                fx.mem.write_u32(b + 0x1C + 4 * c, w);
            }
            for c in 0..3u32 {
                let w = fx.mem.read_u32(q + 0xC + 4 * c);
                fx.mem.write_u32(b + 0x28 + 4 * c, w);
            }
            bolt_bytes(fx, b, 3, 5);
            s4 += 1;
        }
    }
    beads(fx, sp, 0xA, 0x41A0_0000, 0xBECC_CCCD, 0x3ECC_CCCD, None, 0x01E0_5E48);
}

/// A bolt record's common fields: kind `k8` (+8), +0xD `kd`.
fn bolt_bytes(fx: &mut Fx, b: u32, k8: u8, kd: u8) {
    fx.mem.write_u8(b + 8, k8);
    fx.mem.write_u32(b + 0x10, 0x3F80_0000);
    fx.mem.write_u32(b + 0x14, 0x3ECC_CCCD);
    fx.mem.write_u8(b + 0xB, 1);
    fx.mem.write_u8(b + 0xD, kd);
    fx.mem.write_u8(b + 0xC, 0);
    for o in [0x18u32, 0x19, 0x1A] {
        fx.mem.write_u8(b + o, 0x80);
    }
    fx.mem.write_u8(b + 0x1B, 0x20);
}

/// Eight bead beams (+0x6C8C): +8 `k8`, length `len`, +0x20 `bend`, +0x14
/// `w`; at the midpoint, or (`Some(ra)`) on node +0xC.
#[allow(clippy::too_many_arguments)]
fn beads(fx: &mut Fx, sp: u32, k8: u8, len: u32, bend: u32, w: u32, node_ra: Option<u32>, rand_ra: u32) {
    for k in 0..8u32 {
        let b = BEADS + 0x58 * k;
        set_counter4(fx, b, 0x20, k as i32, 0xFF);
        let c = fx.mem.read_u8(b).wrapping_sub(4);
        fx.mem.write_u8(b, c);
        for (o, v) in [(4u32, 5u8), (5, 0), (8, k8), (9, k as u8), (0xA, 0), (0xC, 2)] {
            fx.mem.write_u8(b + o, v);
        }
        let v = rand4(fx, sp, rand_ra) & 3;
        fx.mem.set8(b + 0xB, v);
        fx.mem.write_u32(b + 0x1C, len);
        fx.mem.write_u32(b + 0x20, bend);
        fx.mem.write_u32(b + 0x10, BYTES);
        fx.mem.write_u32(b + 0x14, w);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(b + o, 0x80);
        }
        match node_ra {
            None => copy_v4(fx, sp + 0xE0, b + 0x28),
            Some(ra) => {
                let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
                node_position_saving(fx, h, s, b + 0x28, u64::from(sp + 0x100), Some(u64::from(BEADS - 0x6C8C + 0x58 * k)), ra);
            }
        }
    }
}

/// Update 350 [F 0x1E05ED0]: every flash, bolt, drop, the shake and the
/// dome cut short.
fn cut(fx: &mut Fx) {
    fx.mem.write_u8(SHAKE, 0xB8);
    fx.mem.write_u8(DOME, 0xFF);
    for k in 0..4u32 {
        fx.mem.write_u8(FLASH + 0x4C * k, 0xB8);
    }
    for k in 0..0x64u32 {
        let b = BOLTS + 0x40 * k;
        if fx.mem.read_u8(b) > 0 {
            fx.mem.write_u8(b, 0xA8);
        }
    }
    for k in 0x14..0x3Cu32 {
        fx.mem.write_u8(SCATTER + 0x3C * k, 0xB8);
    }
}

/// Update 534 [F 0x1E05F64]: the fade, a flash at the midpoint, eight
/// bead beams on node +0xC.
fn fade(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 0x10, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    let i = first_free(fx, FLASH, 0x4C, 0x24);
    if i >= 0 {
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 0x14, 0, 0xFF);
        flash_set(fx, f, &[(5, 0), (4, 0x80), (7, 9), (0xA, 0x40), (9, 0x20), (8, 0x80), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0)], &[(0x10, 0x42C8_0000), (0x14, 0x4270_0000), (0x18, 0x4248_0000)]);
        copy_v4(fx, sp + 0xE0, f + 0x1C);
    }
    beads(fx, sp, 0x32, 0x4270_0000, 0xBF19_999A, 0x4000_0000, Some(0x01E0_60E0), 0x01E0_608C);
}

/// Update 582 [F 0x1E060FC]: a spike, a flash and a shake at the target.
fn strike(fx: &mut Fx, sp: u32, mut s1: u64) -> u64 {
    let i = first_free(fx, SPIKES, 0x68, 0x3C);
    if i >= 0 {
        s1 = u64::from(0x68 * i as u32);
        let q = SPIKES + 0x68 * i as u32;
        set_counter4(fx, q, 0x20, 0, 0xFF);
        fx.mem.write_u8(q + 4, 5);
        fx.mem.write_u32(q + 0x14, 0x4026_6666);
        fx.mem.write_u32(q + 0x1C, 0x4220_0000);
        for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0xA0)] {
            fx.mem.write_u8(q + o, v);
        }
        let v = rand4(fx, sp, 0x01E0_6180);
        fx.mem.set8(q + 9, v);
        fx.mem.write_u32(q + 0x10, BYTES);
        fx.mem.write_u32(q + 0x18, 0x3F80_0000);
        for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        copy_v4(fx, sp + 0xD0, q + 0x28);
        fx.mem.set32(q + 0x58, 0x40);
        fx.mem.set32(q + 0x5C, 0x80);
        fx.mem.set32(q + 0x60, 0x100);
    }
    let i = first_free(fx, FLASH, 0x4C, 0x24);
    if i >= 0 {
        let f = FLASH + 0x4C * i as u32;
        s1 = u64::from(0x4C * i as u32);
        set_counter4(fx, f, 0x22, 4, 0xFF);
        flash_set(fx, f, &[(5, 0), (4, 0x81), (7, 9)], &[(0x10, 0x4416_0000), (0x14, 0x4396_0000)]);
        flash_set(fx, f, &[(0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x20), (8, 0x80), (0xF, 0xC0)], &[(0x44, 0), (0x40, 0), (0x3C, 0)]);
        copy_v4(fx, sp + 0xD0, f + 0x1C);
    }
    set_counter4(fx, SHAKE, 0x40, 0, 0xFF);
    fx.mem.write_u8(SHAKE + 5, 0);
    fx.mem.write_u8(SHAKE + 4, 8);
    fx.mem.write_u32(SHAKE + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(SHAKE + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(SHAKE + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(SHAKE + 0x30, 0);
    fx.mem.write_u32(SHAKE + 0x34, 0x4220_0000);
    fx.mem.write_u32(SHAKE + 0x50, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, SHAKE + 0x20, u64::from(sp + 0x100), 0x01E0_6314);
    s1
}

/// A sprite (+0xEBC, 0x44 bytes) `count` counts at `pos` + the jitter
/// sp + 0x100.. (`add`) or at `pos` moving by the jitter / `div`.
fn sprite_fields(fx: &mut Fx, p: u32, alpha: u8, len: u32, bend: u32) {
    fx.mem.write_u8(p + 4, 0x41);
    fx.mem.write_u8(p + 8, 0);
    fx.mem.write_u8(p + 0xB, 2);
    fx.mem.write_u8(p + 9, 3);
    fx.mem.write_u8(p + 0xA, 0);
    if alpha == 0x80 {
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(p + o, 0x80);
        }
    } else {
        for o in [0xCu32, 0xD, 0xE] {
            fx.mem.write_u8(p + o, 0x80);
        }
        fx.mem.write_u8(p + 0xF, alpha);
    }
    fx.mem.write_u32(p + 0x10, len);
    fx.mem.write_u32(p + 0x14, bend);
    fx.mem.write_u32(p + 0x1C, 0);
    fx.mem.write_u32(p + 0x18, 0);
}

/// The dome's tail [F 0x1E06328..0x1E0669C] (while it counts).
fn dome_follow(fx: &mut Fx, sp: u32, s1: &mut u64) {
    let n = fx.mem.u16(D);
    if n == 0x216 {
        for c in 0..3u32 {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0xD0 + 4 * c), fx.mem.f(sp + 0xE0 + 4 * c)), 48.0);
            fx.mem.setf(DRIFT + 4 * c, v);
        }
    }
    let n = fx.mem.u16(D);
    if (0x216..0x247).contains(&n) {
        let k = ee::from_i32(n - 0x216);
        for c in [2u32, 1, 0] {
            let v = ee::add(fx.mem.f(sp + 0xE0 + 4 * c), ee::mul(fx.mem.f(DRIFT + 4 * c), k));
            fx.mem.setf(sp + 0xE0 + 4 * c, v);
        }
        let (f20, f21) = (fx.mem.read_u32(sp + 0xE4), fx.mem.read_u32(sp + 0xE8));
        for _ in 0..6 {
            let i = first_free(fx, DROPS, 0x44, 0x78);
            *s1 = i as i64 as u64;
            if i < 0 {
                continue;
            }
            fx.fix_residue(26.0, 0x01E0_6448);
            let p = DROPS + 0x44 * i as u32;
            *s1 = u64::from(p - 0xEBC);
            set_counter4(fx, p, 26, 0, 0xFF);
            sprite_fields(fx, p, 0x80, 0x4270_0000, 0xBF00_0000);
            for (c, ra) in [(0u32, 0x01E0_64BC), (4, 0x01E0_6510), (8, 0x01E0_6564)] {
                let f = jit(fx, sp, ra, 0xE, 0x578);
                fx.mem.setf(sp + 0x100 + c, f);
            }
            let x = fx.mem.read_u32(sp + 0xE0);
            fx.mem.write_u32(p + 0x20, x);
            fx.mem.write_u32(p + 0x24, f20);
            fx.mem.write_u32(p + 0x28, f21);
            for c in 0..3u32 {
                let v = ee::div(fx.mem.f(sp + 0x100 + 4 * c), 26.0);
                fx.mem.setf(p + 0x2C + 4 * c, v);
            }
            for o in [0x40u32, 0x3C, 0x38] {
                fx.mem.write_u32(p + o, 0);
            }
        }
    }
    let n = fx.mem.u16(D);
    if n >= 0x246 {
        copy_v4(fx, sp + 0xD0, DOME + 0x20);
        let v = ee::add(fx.mem.f(DOME + 0x14), f32::from_bits(0x3F33_3333));
        fx.mem.setf(DOME + 0x14, v);
        fx.mem.write_u32(DOME + 0xC, 0x4080_0000);
    } else {
        copy_v4(fx, sp + 0xE0, DOME + 0x20);
        let v = fx.mem.read_u32(LIFT);
        fx.mem.write_u32(DOME + 0x14, v);
    }
    let a = fx.mem.u8(DOME + 0x1F);
    if a < 0x80 {
        fx.mem.set8(DOME + 0x1F, a + 2);
    }
}

/// Updates 80..159 [F 0x1E066B8]: eight sprites from nodes +0x14..+0x20
/// (two each), thrown outwards.
fn sprites(fx: &mut Fx, sp: u32, s1: &mut u64) {
    for s2 in 0..8u32 {
        let i = first_free(fx, DROPS, 0x44, 0x78);
        *s1 = i as i64 as u64;
        if i < 0 {
            continue;
        }
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14 + 4 * (s2 >> 1)));
        node_position_saving(fx, h, s, sp + 0xF0, u64::from(sp + 0x100), Some(i as i64 as u64), 0x01E0_66F4);
        let v = rand4(fx, sp, 0x01E0_66FC);
        let (_, r) = fx.div(v, 4);
        let f20 = ee::add(10.0, ee::div(ee::from_i32(200 * r - 0x190), 100.0));
        fx.fix_residue(f20, 0x01E0_675C);
        let count = ee::fixsfsi(f20) & 0xFF;
        let p = DROPS + 0x44 * i as u32;
        *s1 = u64::from(p - 0xEBC);
        set_counter4(fx, p, count, 0, 0xFF);
        sprite_fields(fx, p, 0x60, 0x420C_0000, 0xBFC0_0000);
        for (c, ra) in [(0u32, 0x01E0_67D4), (4, 0x01E0_6830), (8, 0x01E0_688C)] {
            let f = jit(fx, sp, ra, 8, 0x320);
            fx.mem.setf(sp + 0x100 + c, f);
        }
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0xF0 + 4 * c), fx.mem.f(sp + 0x100 + 4 * c));
            fx.mem.setf(p + 0x20 + 4 * c, v);
        }
        for c in 0..3u32 {
            let v = ee::neg(ee::div(fx.mem.f(sp + 0x100 + 4 * c), f20));
            fx.mem.setf(p + 0x2C + 4 * c, v);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(p + o, 0);
        }
    }
}

/// A spike's common fields (+0x10 .. +0x27, the three random turns).
fn spike_tail(fx: &mut Fx, sp: u32, q: u32, ra: [u32; 4]) {
    let v = rand4(fx, sp, ra[0]);
    fx.mem.set8(q + 9, v);
    fx.mem.write_u32(q + 0x10, BYTES);
    fx.mem.write_u32(q + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x10), (0x27, 0x40)] {
        fx.mem.write_u8(q + o, v);
    }
    for (o, ra) in [(0x58u32, ra[1]), (0x5C, ra[2]), (0x60, ra[3])] {
        let v = rand4(fx, sp, ra) & 0x7F;
        fx.mem.set32(q + o, v);
    }
}

/// Even updates 100..533 [F 0x1E06988]: a spike on each of nodes
/// +0x14..+0x20.
fn node_spikes(fx: &mut Fx, sp: u32) {
    for s3 in 2..6u32 {
        let i = first_free(fx, SPIKES, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let q = SPIKES + 0x68 * i as u32;
        set_counter4(fx, q, 0x10, 0, 0xFF);
        fx.mem.write_u8(q + 4, 5);
        fx.mem.write_u32(q + 0x14, 0x3F4C_CCCD);
        fx.mem.write_u32(q + 0x1C, 0x4170_0000);
        fx.mem.write_u8(q + 8, 6);
        let b = fx.mem.read_u8(D + 0xC + 4 * s3);
        fx.mem.write_u8(q + 6, b);
        for (o, v) in [(5u32, 1u8), (0xA, 0), (0xB, 2), (0xC, 3)] {
            fx.mem.write_u8(q + o, v);
        }
        spike_tail(fx, sp, q, [0x01E0_6A20, 0x01E0_6A68, 0x01E0_6A78, 0x01E0_6A88]);
    }
}

/// Even updates 160..581 [F 0x1E06AD0]: a spike at the midpoint.
fn mid_spike(fx: &mut Fx, sp: u32, s1: &mut u64) {
    let i = first_free(fx, SPIKES, 0x68, 0x3C);
    if i < 0 {
        return;
    }
    *s1 = u64::from(0x68 * i as u32);
    let q = SPIKES + 0x68 * i as u32;
    set_counter4(fx, q, 0xA, 0, 0xFF);
    fx.mem.write_u8(q + 4, 5);
    let v = fx.mem.read_u32(LIFT);
    fx.mem.write_u32(q + 0x14, v);
    fx.mem.write_u32(q + 0x1C, 0x41A0_0000);
    for (o, v) in [(8u32, 0x28u8), (5, 0), (0xA, 0), (0xB, 2), (0xC, 3)] {
        fx.mem.write_u8(q + o, v);
    }
    spike_tail(fx, sp, q, [0x01E0_6B54, 0x01E0_6BA0, 0x01E0_6BB4, 0x01E0_6BC8]);
    copy_v4(fx, sp + 0xE0, q + 0x28);
}

/// Updates 582..642 [F 0x1E06C18]: twice three bolts about a jittered
/// point by the target and a flash there.
fn target_bolts(fx: &mut Fx, sp: u32, s1: &mut u64) {
    fx.mem.set32(sp + 0xC0, 0);
    while fx.mem.i32(sp + 0xC0) < 2 {
        for (c, ra) in [(0u32, 0x01E0_6C24), (4, 0x01E0_6C64), (8, 0x01E0_6CA4)] {
            let v = rand4(fx, sp, ra);
            let (_, r) = fx.div(v, 0xC80);
            let f = ee::add(fx.mem.f(sp + 0xD0 + c), ee::div(ee::from_i32(r - 0x640), 100.0));
            fx.mem.setf(sp + 0x100 + c, f);
        }
        for s5 in 0..3 {
            let i = first_free(fx, BOLTS, 0x40, 0x64);
            if i < 0 {
                break;
            }
            let b = BOLTS + 0x40 * i as u32;
            *s1 = u64::from(b - 0x51AC);
            set_counter4(fx, b, 0x10, s5 & 0xFF, 0xFF);
            fx.mem.write_u8(b + 5, 0);
            fx.mem.write_u8(b + 4, 0);
            for c in 0..3u32 {
                let w = fx.mem.read_u32(sp + 0x100 + 4 * c);
                fx.mem.write_u32(b + 0x28 + 4 * c, w);
                fx.mem.write_u32(b + 0x1C + 4 * c, w);
            }
            for (c, ra) in [(0u32, 0x01E0_6D50), (4, 0x01E0_6DA8), (8, 0x01E0_6E00)] {
                let j = jit(fx, sp, ra, 9, 0x384);
                let v = ee::add(fx.mem.f(b + 0x28 + c), j);
                fx.mem.setf(b + 0x28 + c, v);
            }
            bolt_bytes(fx, b, 9, 4);
        }
        let i = first_free(fx, FLASH, 0x4C, 0x24);
        if i >= 0 {
            let f = FLASH + 0x4C * i as u32;
            *s1 = u64::from(f - 0x46FC);
            set_counter4(fx, f, 0xE, 0, 0xFF);
            flash_set(fx, f, &[(5, 0), (4, 5), (7, 9), (0xF, 0xD0), (0xC, 0xD0), (0xD, 0xD0), (0xE, 0xD0), (0xA, 0), (9, 0), (8, 0x80)], &[(0x14, 0), (0x10, 0x437A_0000), (0x14, 0)]);
            copy_v4(fx, sp + 0x100, f + 0x1C);
        }
        let k = fx.mem.i32(sp + 0xC0) + 1;
        fx.mem.set32(sp + 0xC0, k);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let n = fx.mem.u16(D);
    if n < 0x1B8 {
        let v = ee::add(f32::from_bits(0x4013_3333), ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(n & 3)));
        fx.mem.setf(LIFT, v);
    } else if n < 0x20D {
        let v = ee::add(fx.mem.f(LIFT), f32::from_bits(0x3DA3_D70A));
        fx.mem.setf(LIFT, v);
    }
    let s0 = param as i64 as u64;
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xD0, s0, 0x01E0_5738);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, s, sp + 0xF0, s0, None, 0x01E0_5748);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, s, sp + 0x100, u64::from(sp + 0x100), None, 0x01E0_575C);
    for c in 0..3u32 {
        let a = fx.mem.f(sp + 0xF0 + 4 * c);
        let v = ee::add(a, ee::div(ee::sub(fx.mem.f(sp + 0x100 + 4 * c), a), 2.0));
        fx.mem.setf(sp + 0xE0 + 4 * c, v);
    }
    let z = if fx.mem.read_u32(D + 0x30) == 0x8000 { ee::sub(fx.mem.f(sp + 0xE8), fx.mem.f(LIFT)) } else { ee::add(fx.mem.f(sp + 0xE8), fx.mem.f(LIFT)) };
    fx.mem.setf(sp + 0xE8, z);
    // The entry's s1 [F]: the runner's ([H] 0) on updates 350 / 534 / 582
    // until set, 0 on the others (a dispatch delay slot), then the blocks'.
    let mut s1 = RUNNER_S1;
    match fx.mem.u16(D) {
        0x2BC => return 8,
        0x246 => s1 = strike(fx, sp, s1),
        0x216 => {
            fade(fx, sp);
            s1 = u64::from(D + 0x2C0);
        }
        0x15E => cut(fx),
        0xA0 => {
            burst(fx, sp);
            s1 = u64::from(D + 0x2C0);
        }
        0x64 => {
            flashes(fx);
            s1 = 4;
        }
        2 => {
            start(fx, sp);
            s1 = u64::from(D + 0x4B0);
        }
        _ => s1 = 0,
    }
    if fx.mem.u8(DOME) > 0 && fx.mem.u8(DOME + 1) == 0 {
        dome_follow(fx, sp, &mut s1);
    }
    let n = fx.mem.u16(D);
    if (0x50..0xA0).contains(&n) {
        sprites(fx, sp, &mut s1);
    }
    let n = fx.mem.u16(D);
    if (0x64..0x216).contains(&n) && n & 1 == 0 {
        node_spikes(fx, sp);
        s1 = u64::from(D + 0x18);
    }
    let n = fx.mem.u16(D);
    if (0xA0..0x246).contains(&n) && n & 1 == 0 {
        mid_spike(fx, sp, &mut s1);
    }
    let n = fx.mem.u16(D);
    if (0x246..0x283).contains(&n) {
        target_bolts(fx, sp, &mut s1);
    }
    let s0 = u64::from(sp + 0x100);
    shake_saving(fx, SHAKE, s0, 0x01E0_6F74, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6F80, [Some(s0), Some(s1)]);
    let ot = fx.mem.read_u32(sp + 0xB8);
    blib::fade::fade(fx, FADE, ot);
    blib::drop::scatter_drops_b(fx, 0x01E0_26C0, SCATTER, ot, 0x3C);
    blib::drop::drops(fx, DROPS, ot, 0x78);
    blib::star::flash_b(fx, 0x01E0_03A0, FLASH, ot, 0x24);
    blib::spike::spikes(fx, 0x01E0_0E10, SPIKES, ot, 0x3C);
    blib::emit::segment_spawns_b(fx, 0x01E0_2380, EMIT, 8);
    // segment_spawns_b's 0x80 frame saves s0..s4 (+0x10..) and ra (+0x60):
    // ra is the stale TEX0 bolts_b's strips report when their texture is
    // missing (as b370) [run]; s2..s4 [H] 0.
    fx.saved(sp - 0x80, &[(0x10, s0), (0x20, s1), (0x30, 0), (0x40, 0), (0x50, 0), (0x60, 0x01E0_6FDC)]);
    blib::bolt::bolts_b(fx, 0x01E0_18F0, BOLTS, ot, 0x64);
    blib::beam::beads_at(fx, 0x01E0_3060, 0x34, 0x3C, BEADS, ot, 0x14);
    blib::dome::domes(fx, 0x01E0_4640, DOME, ot, 4);
    let n = fx.mem.u16(D);
    if (0x246..0x283).contains(&n) {
        6
    } else if n == 0x283 {
        2
    } else if n == 0x23C {
        1
    } else {
        0
    }
}
