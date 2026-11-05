//! BMODULE b210 (card 543), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04B50, data area a0 = 0x1ED0000) [F] (helper C10,
//! 31 Oct 2026): b609's library at the same addresses (without `bounce`)
//! and b559's family layout (s0 = D, s1 = the OT). On update 2 two flashes
//! (+0x193C) on the attacker's nodes 0x23 / 0x2A and a long camera shake
//! (+0x40); on 16 two long flashes there and six segment spawners
//! (+0x395C, into the bolt pool +0x205C: two on the nodes, four on node
//! 1); on 56 the streaks (+0xA4) from node 1; on 68 the fade (+0x421C), a
//! shake at the target, eight bead beams on node 1 (+0x3B3C), a flash and
//! five jagged bolts from node 1 down round a point above it; on 180 a
//! wide flash above the target; on 190 the fade again and six jagged bolts
//! round the target; on even updates 2..329 two ribbons a update (+0xDC)
//! from the nodes (node 1 from 68); on 190..326 four flashes and ribbons a
//! update round the target (+-14); the stage light pulsed by 0x1031C0
//! (+0xA0). Returns 1 on update 180, 6 on 190..326, 2 on 327, 8 on 350
//! (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `streaks`
//! 0x1E00410, `flash` 0x1E01230, `ribbon` 0x1E01DE0, `bolts` 0x1E02970,
//! `segment_spawns` 0x1E034C0, `beads` 0x1E03840, `fade` 0x1E04610
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b586::segment_spawns_saving;
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x4224;
const FLASHES: u32 = D + 0x193C;
const BOLTS: u32 = D + 0x205C;
const SPAWNERS: u32 = D + 0x395C;
const RIBBONS: u32 = D + 0xDC;
const FADE: u32 = D + 0x421C;

/// The entry 0x1E04B50 (a0 = D, a1 = param; s0 = D, s1 = the OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1A0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x1A0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x4330);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..2 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x23, 1), (0x10, 0x2A, 0), (0x14, 1, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x158);
    // The entry's s0 = D, s1 = the OT, s2 = 2 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4C84, [Some(D), Some(OT), Some(2)]);
    0
}

fn flash_bytes(fx: &mut Fx, r: u32, b4: u8, c: u8, a: u8, b9: u8, b8: u8) {
    fx.mem.write_u8(r + 4, b4);
    fx.mem.write_u8(r + 7, 9);
    for (o, v) in [(0xAu32, a), (9, b9), (8, b8), (0xF, c), (0xC, c), (0xD, c), (0xE, c)] {
        fx.mem.write_u8(r + o, v);
    }
}

/// A free flash record (pool of 0x18), or None.
fn flash_slot(fx: &mut Fx) -> Option<u32> {
    let i = first_free(fx, FLASHES, 0x4C, 0x18);
    (i >= 0).then(|| FLASHES + 0x4C * i as u32)
}

/// Update 2 [F 0x1E04D0C]: two flashes on the nodes, the long shake.
fn charge(fx: &mut Fx) {
    for k in 0..2u32 {
        let r = FLASHES + 0x4C * k;
        set_counter4(fx, r, 0x12, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 4, 0x80);
        fx.mem.write_u8(r + 7, 9);
        fx.mem.write_u32(r + 0x10, 0x4248_0000);
        fx.mem.write_u32(r + 0x14, 0x4120_0000);
        for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x10), (9, 0x20), (8, 0x80), (0xF, 0xD0)] {
            fx.mem.write_u8(r + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(r + o, 0);
        }
        fx.mem.write_u32(r + 0x18, 0x4100_0000);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0xFA, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, v);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3F99_999A);
    fx.mem.write_u32(s + 0x18, 0x3F99_999A);
    fx.mem.write_u32(s + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x4000_0000);
}

/// Update 16 [F 0x1E04E38]: a long flash on each node, six spawners.
fn glow(fx: &mut Fx) {
    for k in 0..2u32 {
        let Some(r) = flash_slot(fx) else { continue };
        set_counter4(fx, r, 0xFF, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        flash_bytes(fx, r, 5, 0xC0, 0x40, 0x20, 0x80);
        fx.mem.write_u32(r + 0x10, 0x4316_0000);
        fx.mem.write_u32(r + 0x14, 0);
    }
    for k in 0..6u32 {
        let s = SPAWNERS + 0x3C * k;
        set_counter4(fx, s, 0x26, 8, 0);
        fx.mem.write_u8(s + 5, 1);
        fx.mem.write_u8(s + 4, 2);
        fx.mem.write_u32(s + 8, BOLTS);
        for (o, v) in [(0xCu32, 0x64u8), (0xD, 8), (0xE, 1), (0xF, 0), (0x10, 5), (0x11, 0xC)] {
            fx.mem.write_u8(s + o, v);
        }
        fx.mem.write_u32(s + 0x18, 0x3F80_0000);
        fx.mem.write_u32(s + 0x1C, 0x3ECC_CCCD);
        for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20)] {
            fx.mem.write_u8(s + o, v);
        }
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(s + o, 0);
        }
        if k < 2 {
            let b = fx.mem.read_u8(D + 0xC + 4 * k);
            fx.mem.write_u8(s + 6, b);
            fx.mem.set32(s + 0x14, 0x2BC);
        } else {
            let b = fx.mem.read_u8(D + 0x14);
            fx.mem.write_u8(s + 6, b);
            fx.mem.set32(s + 0x14, 0x514);
        }
    }
}

/// Update 56 [F 0x1E05024]: the streaks from node +0x14.
fn streaks_start(fx: &mut Fx) {
    let s = D + 0xA4;
    fx.mem.write_u8(s + 4, 0);
    set_counter4(fx, s, 0x1C, 0, 0xFF);
    for (o, v) in [(9u32, 2u8), (0xA, 0xFF), (8, 8), (0xB, 0x18)] {
        fx.mem.write_u8(s + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(s + 0xC, h);
    for (o, v) in [(0x14u32, 0x80u8), (0x15, 0x80), (0x16, 0x80), (0x17, 0), (0x10, 0x90), (0x11, 0x90), (0x12, 0x90), (0x13, 0x40), (5, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(s + 6, b);
}

/// A jagged bolt (0x102A50, five segments, sp + 0xA0) from `a` to `b`
/// into the bolt pool: count `c`, delay `j & 1`, width / alpha bytes.
fn jag5(fx: &mut Fx, sp: u32, a: [u32; 3], b: [u32; 3], spread: u32, j: u32, c: i32, w: u8, sizes: [u32; 2], bd: u8) {
    let p = sp + 0xA0;
    fx.mem.set32(p, 5);
    fx.mem.write_u32(p + 4, spread);
    fx.mem.write_u32(p + 0x20, sp + 0xD0);
    for (k, at) in a.into_iter().chain(b).enumerate() {
        let w = fx.mem.read_u32(at);
        fx.mem.write_u32(p + 8 + 4 * k as u32, w);
    }
    blib::bolt::jag(fx, p);
    for s in 0..5u32 {
        let i = first_free(fx, BOLTS, 0x40, 0x64);
        if i < 0 {
            continue;
        }
        let r = BOLTS + ((i as u32) << 6);
        set_counter4(fx, r, c, (j & 1) as i32, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0);
        for k in 0..6u32 {
            let v = fx.mem.read_u32(sp + 0xD0 + 12 * s + 4 * k);
            fx.mem.write_u32(r + 0x1C + 4 * k, v);
        }
        fx.mem.write_u8(r + 8, w);
        fx.mem.write_u32(r + 0x10, sizes[0]);
        fx.mem.write_u32(r + 0x14, sizes[1]);
        for (o, v) in [(0xBu32, 1u8), (0xD, bd), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
    }
}

fn addf(fx: &mut Fx, a: u32, d: f32) {
    let v = ee::add(fx.mem.f(a), d);
    fx.mem.setf(a, v);
}

fn subf(fx: &mut Fx, a: u32, d: f32) {
    let v = ee::sub(fx.mem.f(a), d);
    fx.mem.setf(a, v);
}

/// Update 68 [F 0x1E050AC]: the fade, the shake at the target, eight bead
/// beams on node +0x14, a flash there and five jagged bolts from it down
/// round a point 80 above it.
fn bolts_start(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 0x10, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    let s = D + 0x40;
    fx.mem.write_u8(s + 5, 0);
    set_counter4(fx, s, 0xD0, 0, 0xFF);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3FB3_3333);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4320_0000);
    copy_v4(fx, sp + 0x60, D + 0x60);
    fx.mem.write_u32(s + 0x50, 0x4100_0000);
    fx.mem.write_u32(D + 0x68, 0);
    for k in 0..8i32 {
        let r = D + 0x3B3C + 0x58 * k as u32;
        set_counter4(fx, r, 0x20, k & 0xFF, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(4);
        fx.mem.write_u8(r, c);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x1E);
        fx.mem.set8(r + 9, k);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xC, 2);
        let x = fx.rand_from(D, 0x01E0_51C0) >> 4;
        fx.mem.set8(r + 0xB, x & 3);
        fx.mem.write_u32(r + 0x1C, 0x4248_0000);
        fx.mem.write_u32(r + 0x20, 0xBF19_999A);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0x14);
        node_position_saving(fx, h, n, r + 0x28, u64::from(D), Some(u64::from(OT)), 0x01E0_521C);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x14);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(D), Some(u64::from(OT)), 0x01E0_5240);
    if let Some(r) = flash_slot(fx) {
        set_counter4(fx, r, 0x18, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        flash_bytes(fx, r, 0x80, 0xC0, 0x40, 0x20, 0x80);
        fx.mem.write_u32(r + 0x10, 0x42C8_0000);
        fx.mem.write_u32(r + 0x14, 0x4316_0000);
        fx.mem.write_u32(r + 0x18, 0x4270_0000);
        copy_v4(fx, sp + 0x80, r + 0x1C);
    }
    for j in 0..5u32 {
        copy_v4(fx, sp + 0x80, sp + 0x90);
        addf(fx, sp + 0x94, 80.0);
        match j {
            3 => addf(fx, sp + 0x98, 50.0),
            2 => subf(fx, sp + 0x98, 50.0),
            1 => addf(fx, sp + 0x90, 50.0),
            0 => subf(fx, sp + 0x90, 50.0),
            _ => {}
        }
        jag5(fx, sp, [sp + 0x80, sp + 0x84, sp + 0x88], [sp + 0x90, sp + 0x94, sp + 0x98], 0x3E4C_CCCD, j, 0x80, 8, [0x3F4C_CCCD, 0x3E99_999A], 3);
    }
}

/// Update 180 [F 0x1E0557C]: a wide flash 45 above the target.
fn strike(fx: &mut Fx, sp: u32) {
    copy_v4(fx, sp + 0x60, sp + 0x80);
    addf(fx, sp + 0x84, 45.0);
    let Some(r) = flash_slot(fx) else { return };
    set_counter4(fx, r, 0x18, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    flash_bytes(fx, r, 0x81, 0xC0, 0x40, 0x20, 0x80);
    fx.mem.write_u32(r + 0x10, 0x44C8_0000);
    fx.mem.write_u32(r + 0x14, 0x42C8_0000);
    fx.mem.write_u32(r + 0x18, 0x4270_0000);
    copy_v4(fx, sp + 0x80, r + 0x1C);
}

/// Update 190 [F 0x1E0567C]: the fade again and six jagged bolts round
/// the target (from 60 above to 60 below it, pushed apart by the record's
/// jump table 0x1E06120).
fn crash(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 0xC, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    for j in 0..6u32 {
        copy_v4(fx, sp + 0x60, sp + 0x90);
        copy_v4(fx, sp + 0x90, sp + 0x80);
        addf(fx, sp + 0x84, 60.0);
        subf(fx, sp + 0x94, 60.0);
        match j {
            0 => {
                subf(fx, sp + 0x80, 18.0);
                subf(fx, sp + 0x90, 18.0);
            }
            1 => {
                addf(fx, sp + 0x80, 18.0);
                addf(fx, sp + 0x90, 18.0);
            }
            2 => {
                subf(fx, sp + 0x88, 18.0);
                subf(fx, sp + 0x98, 18.0);
            }
            3 => {
                addf(fx, sp + 0x88, 18.0);
                addf(fx, sp + 0x98, 18.0);
            }
            4 => {
                for a in [0x80u32, 0x90, 0x88, 0x98] {
                    addf(fx, sp + a, 10.0);
                }
            }
            _ => {
                for a in [0x80u32, 0x90, 0x88, 0x98] {
                    subf(fx, sp + a, 10.0);
                }
            }
        }
        jag5(fx, sp, [sp + 0x80, sp + 0x84, sp + 0x88], [sp + 0x90, sp + 0x94, sp + 0x98], 0x3ECC_CCCD, j, 0x8C, 6, [0x3F80_0000, 0x3E99_999A], 4);
    }
}

/// Even updates 2..329 [F 0x1E05A20]: two ribbons, from the nodes before
/// update 68, then both from node +0x14.
fn ribbons(fx: &mut Fx) {
    for k in 0..2u32 {
        let i = first_free(fx, RIBBONS, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let r = RIBBONS + 0x68 * i as u32;
        set_counter4(fx, r, 0x10, 0, 0xFF);
        fx.mem.write_u8(r + 4, 5);
        if fx.mem.u16(D) < 0x44 {
            fx.mem.write_u32(r + 0x14, 0x3F00_0000);
            fx.mem.write_u32(r + 0x1C, 0x4120_0000);
            fx.mem.write_u8(r + 8, 8);
            let b = fx.mem.read_u8(D + 0xC + 4 * k);
            fx.mem.write_u8(r + 6, b);
        } else {
            fx.mem.write_u32(r + 0x14, 0x4060_0000);
            fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
            fx.mem.write_u8(r + 8, 0x12);
            let b = fx.mem.read_u8(D + 0x14);
            fx.mem.write_u8(r + 6, b);
        }
        fx.mem.write_u8(r + 5, 1);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 3);
        let x = fx.rand_from(D, 0x01E0_5B10);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x23, 0x10);
        fx.mem.write_u8(r + 0x27, 0x40);
        for (o, ra) in [(0x58u32, 0x01E0_5B60u32), (0x5C, 0x01E0_5B78), (0x60, 0x01E0_5B90)] {
            let x = fx.rand_from(D, ra) >> 4;
            fx.mem.set32(r + o, x & 0x7F);
        }
    }
}

/// Updates 190..326 [F 0x1E05BD4]: four flashes and ribbons a update
/// round the target (+-14 per axis; sp + 0x70).
fn crackle(fx: &mut Fx, sp: u32) {
    let p = sp + 0x70;
    for _ in 0..4 {
        for (c, ra) in [(0u32, 0x01E0_5BE8u32), (4, 0x01E0_5C34), (8, 0x01E0_5C80)] {
            let x = fx.rand_from(D, ra) >> 4;
            let (_, h) = fx.div(x, 0xAF0);
            let f = ee::div(ee::from_i32(h - 0x578), 100.0);
            let v = ee::add(fx.mem.f(sp + 0x60 + c), f);
            fx.mem.setf(p + c, v);
        }
        if let Some(r) = flash_slot(fx) {
            set_counter4(fx, r, 0xC, 0, 0xFF);
            fx.mem.write_u8(r + 5, 0);
            flash_bytes(fx, r, 1, 0xC0, 0x60, 0x10, 0x40);
            fx.mem.write_u32(r + 0x14, 0);
            fx.mem.write_u32(r + 0x10, 0x4396_0000);
            copy_v4(fx, p, r + 0x1C);
        }
        let i = first_free(fx, RIBBONS, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let r = RIBBONS + 0x68 * i as u32;
        fx.mem.write_u8(r + 4, 5);
        set_counter4(fx, r, 8, 0, 0xFF);
        fx.mem.write_u32(r + 0x14, 0x40A0_0000);
        fx.mem.write_u32(r + 0x1C, 0x4220_0000);
        fx.mem.write_u8(r + 0x27, 0xA0);
        fx.mem.write_u8(r + 0x23, 0);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 3);
        fx.mem.write_u8(r + 8, 0x28);
        let x = fx.rand_from(D, 0x01E0_5E30);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, p, r + 0x28);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_4CAC);
    match fx.mem.u16(D) {
        0x15E => return 8,
        0xBE => crash(fx, sp),
        0xB4 => strike(fx, sp),
        0x44 => bolts_start(fx, sp),
        0x38 => streaks_start(fx),
        0x10 => glow(fx),
        2 => charge(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0x14A).contains(&n) && n & 1 == 0 {
        ribbons(fx);
    }
    let n = fx.mem.u16(D);
    if (0xBE..0x147).contains(&n) {
        crackle(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_5EB4, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5EC4, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the runner's s6 (0) at +0x70 and ra at
    // +0x80: the streaks' first TEXFLUSH reads them as vertex 1's / 2's
    // stale XYZ (as b609) [run].
    fx.saved(sp - 0xF0, &[(0x70, 0), (0x80, 0x01E0_5ED4)]);
    blib::radiate::streaks(fx, 0x01E0_0410, D + 0xA4, OT, 1);
    blib::star::flash(fx, 0x01E0_1230, FLASHES, OT, 0x18);
    blib::streak::ribbon(fx, 0x01E0_1DE0, RIBBONS, OT, 0x3C);
    segment_spawns_saving(fx, 0x01E0_34C0, SPAWNERS, 8, 0x01E0_5F24);
    blib::bolt::bolts(fx, 0x01E0_2970, BOLTS, OT, 0x64);
    blib::beam::beads(fx, 0x01E0_3840, D + 0x3B3C, OT, 0x14);
    let n = fx.mem.u16(D);
    if (0xBE..0x147).contains(&n) {
        6
    } else if n == 0x147 {
        2
    } else if n == 0xB4 {
        1
    } else {
        0
    }
}
