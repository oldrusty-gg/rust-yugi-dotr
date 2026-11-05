//! BMODULE b609 (card 541), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E055C0, data area a0 = 0x1ED0000) [F] (helper C10,
//! 31 Oct 2026): b559's family (s0 = D, s1 = the OT, s2 = the parameter)
//! with the streaks (+0x1B34) and bouncing sprites (+0xA4). On update 64
//! two flashes (+0x33CC) on the attacker's nodes 0x23 / 0x2A and a long
//! camera shake (+0x40); on 78 two long flashes there; on 128 two segment
//! spawners (+0x478C, into the bolt pool +0x388C) on the nodes; on 176
//! the streaks from node 2; on 191 the fade (+0x504C), a shake at the
//! target, eight bead beams on node 2 (+0x496C), a flash and four jagged
//! bolts (0x102A50) from nodes 2 / 0x2A to the target; on 192 two spawners
//! at the target; on even updates 78..399 two ribbons a update from the
//! nodes (+0x1B6C); on 78..149 eight bouncing sprites a update from the
//! nodes; on odd updates 193..396 two flashes and two ribbons a update
//! round the target; the stage light pulsed by 0x1031C0 (+0xA0). Returns
//! 1 on update 182, 6 on 192..396, 2 on 397, 8 on 420 (and at once for a
//! parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `streaks`
//! 0x1E00410, `flash` 0x1E01230, `ribbon` 0x1E01DE0, `bolts` 0x1E02970,
//! `segment_spawns` 0x1E034C0, `beads` 0x1E03840, `fade` 0x1E04610,
//! `bounce` 0x1E04B50 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b586::segment_spawns_saving;
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x5054;
const FLASHES: u32 = D + 0x33CC;
const BOLTS: u32 = D + 0x388C;
const SPAWNERS: u32 = D + 0x478C;
const RIBBONS: u32 = D + 0x1B6C;
const SPRITES: u32 = D + 0xA4;

/// The entry 0x1E055C0 (a0 = D, a1 = param; s0 = D, s1 = the OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1C0);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x1C0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x5160);
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
    for (at, node, clear) in [(0xCu32, 0x23, 1), (0x10, 0x2A, 0), (0x14, 2, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x19E);
    // The entry's s0 = D, s1 = the OT, s2 = 2 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_56FC, [Some(D), Some(OT), Some(2)]);
    0
}

fn flash_bytes(fx: &mut Fx, r: u32, b4: u8, c: u8, a: u8, b9: u8, b8: u8) {
    fx.mem.write_u8(r + 4, b4);
    fx.mem.write_u8(r + 7, 9);
    for (o, v) in [(0xAu32, a), (9, b9), (8, b8), (0xF, c), (0xC, c), (0xD, c), (0xE, c)] {
        fx.mem.write_u8(r + o, v);
    }
}

/// A segment spawner (+0x478C + 0x3C i) feeding the bolt pool: count /
/// delay / c, node `node` (None: at `at`), range `r`.
fn spawner(fx: &mut Fx, s: u32, c: [i32; 3], node: Option<u8>, spread: (u8, i32), at: u32) {
    set_counter4(fx, s, c[0], c[1], c[2]);
    match node {
        Some(n) => {
            fx.mem.write_u8(s + 5, 1);
            fx.mem.write_u8(s + 6, n);
            fx.mem.write_u8(s + 4, 2);
        }
        None => {
            fx.mem.write_u8(s + 5, 0);
            fx.mem.write_u8(s + 4, 0);
        }
    }
    fx.mem.write_u32(s + 8, BOLTS);
    for (o, v) in [(0xCu32, 0x28u8), (0xD, spread.0), (0xE, 1), (0xF, 0), (0x10, 5), (0x11, 0xC)] {
        fx.mem.write_u8(s + o, v);
    }
    fx.mem.set32(s + 0x14, spread.1);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3ECC_CCCD);
    for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20)] {
        fx.mem.write_u8(s + o, v);
    }
    if node.is_none() {
        for c in 0..3u32 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(s + 0x24 + 4 * c, w);
        }
    }
    for o in [0x38u32, 0x34, 0x30] {
        fx.mem.write_u32(s + o, 0);
    }
}

/// The spawner slot (no check of the free index: -1 writes the record
/// before the pool).
fn spawner_slot(fx: &mut Fx) -> u32 {
    let i = first_free(fx, SPAWNERS, 0x3C, 8);
    SPAWNERS.wrapping_add(i.wrapping_mul(0x3C) as u32)
}

/// Update 64 [F 0x1E05784]: two flashes on the nodes, the long shake.
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

/// Update 78 [F 0x1E058B0]: a long flash on each node.
fn glow(fx: &mut Fx) {
    for k in 0..2u32 {
        let i = first_free(fx, FLASHES, 0x4C, 0x10);
        if i < 0 {
            continue;
        }
        let r = FLASHES + 0x4C * i as u32;
        set_counter4(fx, r, 0xFF, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        flash_bytes(fx, r, 5, 0xC0, 0x40, 0x20, 0x80);
        fx.mem.write_u32(r + 0x10, 0x4316_0000);
        fx.mem.write_u32(r + 0x14, 0);
    }
}

/// Update 176 [F 0x1E05A84]: the streaks from node +0x14.
fn streaks_start(fx: &mut Fx) {
    let s = D + 0x1B34;
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

/// Update 191 [F 0x1E05B0C]: the fade, the shake at the target, eight
/// bead beams on node +0x14, a flash and four jagged bolts to the target.
fn bolts_start(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0x504C, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0x5050, 1);
    let s = D + 0x40;
    fx.mem.write_u8(s + 5, 0);
    set_counter4(fx, s, 0xD0, 0, 0xFF);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3FB3_3333);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4320_0000);
    copy_v4(fx, sp + 0x80, D + 0x60);
    fx.mem.write_u32(s + 0x50, 0x4100_0000);
    fx.mem.write_u32(D + 0x68, 0);
    for k in 0..8i32 {
        let r = D + 0x496C + 0x58 * k as u32;
        set_counter4(fx, r, 0x20, k & 0xFF, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(4);
        fx.mem.write_u8(r, c);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x1E);
        fx.mem.set8(r + 9, k);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xC, 2);
        let x = fx.rand_from(D, 0x01E0_5C20) >> 4;
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
        node_position_saving(fx, h, n, r + 0x28, u64::from(D), Some(u64::from(OT)), 0x01E0_5C7C);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x14);
    node_position_saving(fx, h, n, sp + 0xA0, u64::from(D), Some(u64::from(OT)), 0x01E0_5CA0);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x10);
    node_position_saving(fx, h, n, sp + 0xB0, u64::from(D), Some(u64::from(OT)), 0x01E0_5CB4);
    let i = first_free(fx, FLASHES, 0x4C, 0x10);
    if i >= 0 {
        let r = FLASHES + 0x4C * i as u32;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        flash_bytes(fx, r, 0x80, 0xC0, 0x40, 0x20, 0x80);
        fx.mem.write_u32(r + 0x10, 0x42C8_0000);
        fx.mem.write_u32(r + 0x14, 0x4316_0000);
        fx.mem.write_u32(r + 0x18, 0x4270_0000);
        copy_v4(fx, sp + 0xA0, r + 0x1C);
    }
    let p = sp + 0xC0;
    for j in 0..4u32 {
        fx.mem.set32(p, 6);
        fx.mem.write_u32(p + 4, 0x3E99_999A);
        fx.mem.write_u32(p + 0x20, sp + 0xF0);
        let from = sp + 0xA0 + 0x10 * (j & 1);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(from + 4 * c);
            fx.mem.write_u32(p + 8 + 4 * c, w);
        }
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * c);
            fx.mem.write_u32(p + 0x14 + 4 * c, w);
        }
        blib::bolt::jag(fx, p);
        for s in 0..6u32 {
            let i = first_free(fx, BOLTS, 0x40, 0x3C);
            if i < 0 {
                continue;
            }
            let b = BOLTS + ((i as u32) << 6);
            set_counter4(fx, b, 0xD0, (j & 1) as i32, 0xFF);
            fx.mem.write_u8(b + 5, 0);
            fx.mem.write_u8(b + 4, 0);
            for c in 0..6u32 {
                let w = fx.mem.read_u32(sp + 0xF0 + 12 * s + 4 * c);
                fx.mem.write_u32(b + 0x1C + 4 * c, w);
            }
            fx.mem.write_u8(b + 8, 9);
            fx.mem.write_u32(b + 0x10, 0x3F33_3333);
            fx.mem.write_u32(b + 0x14, 0x3E4C_CCCD);
            for (o, v) in [(0xBu32, 1u8), (0xD, 3), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)] {
                fx.mem.write_u8(b + o, v);
            }
        }
    }
}

/// Even updates 78..399 [F 0x1E06038]: a ribbon from each node.
fn ribbons(fx: &mut Fx) {
    for k in 0..2u32 {
        let i = first_free(fx, RIBBONS, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let r = RIBBONS + 0x68 * i as u32;
        set_counter4(fx, r, 0x10, 0, 0xFF);
        fx.mem.write_u8(r + 4, 5);
        if fx.mem.u16(D) < 0xBB {
            fx.mem.write_u32(r + 0x14, 0x3F00_0000);
            fx.mem.write_u32(r + 0x1C, 0x4120_0000);
            fx.mem.write_u8(r + 8, 8);
        } else {
            fx.mem.write_u32(r + 0x14, 0x4000_0000);
            fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
            fx.mem.write_u8(r + 8, 0x12);
        }
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 3);
        let x = fx.rand_from(D, 0x01E0_6120);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x23, 0x10);
        fx.mem.write_u8(r + 0x27, 0x40);
        for (o, ra) in [(0x58u32, 0x01E0_6170u32), (0x5C, 0x01E0_6188), (0x60, 0x01E0_61A0)] {
            let x = fx.rand_from(D, ra) >> 4;
            fx.mem.set32(r + o, x & 0x7F);
        }
    }
}

/// (rand >> 4) % 8 as the compiler's signed remainder, x 200 - 800, over
/// 100.
fn jit8(fx: &mut Fx, ra: u32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let mut m = x & 7;
    if x < 0 && m != 0 {
        m -= 8;
    }
    ee::div(ee::from_i32(m * 200 - 0x320), 100.0)
}

/// Updates 78..149 [F 0x1E061E4]: eight bouncing sprites a update from
/// the nodes (+0xC / +0x10 alternately, sp + 0xA0).
fn fountain(fx: &mut Fx, sp: u32) {
    for k in 0..8u32 {
        let i = first_free(fx, SPRITES, 0x44, 0x64);
        if i < 0 {
            continue;
        }
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0xC + 4 * (k & 1));
        node_position_saving(fx, h, n, sp + 0xA0, u64::from(D), Some(u64::from(OT)), 0x01E0_6230);
        let x = fx.rand_from(D, 0x01E0_6238) >> 4;
        let (_, m) = fx.div(x, 6);
        let f = ee::div(ee::from_i32(m * 200 - 0x258), 100.0);
        let life = ee::add(16.0, f);
        fx.fix_residue(life, 0x01E0_62A0);
        let c = ee::fixsfsi(life);
        let b = SPRITES + 0x44 * i as u32;
        set_counter4(fx, b, c & 0xFF, 0, 0xFF);
        fx.mem.write_u8(b + 4, 0x40);
        fx.mem.write_u8(b + 8, 0);
        fx.mem.write_u8(b + 0xB, 2);
        fx.mem.write_u8(b + 9, 4);
        fx.mem.write_u8(b + 0xA, 0);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(b + o, 0x80);
        }
        fx.mem.write_u32(b + 0x10, 0x4170_0000);
        fx.mem.write_u32(b + 0x14, 0xBF00_0000);
        fx.mem.write_u32(b + 0x1C, 0);
        fx.mem.write_u32(b + 0x18, 0);
        for (c, ra) in [(0u32, 0x01E0_6320u32), (4, 0x01E0_6384), (8, 0x01E0_63E8)] {
            let v = jit8(fx, ra);
            fx.mem.setf(sp + 0xB0 + c, v);
        }
        for c in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(sp + 0xA0 + c), fx.mem.f(sp + 0xB0 + c));
            fx.mem.setf(b + 0x20 + c, v);
        }
        for c in [0u32, 4, 8] {
            let v = ee::neg(ee::div(fx.mem.f(sp + 0xB0 + c), life));
            fx.mem.setf(b + 0x2C + c, v);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(b + o, 0);
        }
    }
}

/// Odd updates 193..396 [F 0x1E064F0]: two flashes and two ribbons a
/// update round the target (+-10 in x, y, +-5 in z; sp + 0x90).
fn crackle(fx: &mut Fx, sp: u32) {
    let p = sp + 0x90;
    for _ in 0..2 {
        for (c, ra, m, off) in [(0u32, 0x01E0_6504u32, 0x7D0, 0x3E8), (4, 0x01E0_6550, 0x7D0, 0x3E8), (8, 0x01E0_659C, 0x3E8, 0x1F4)] {
            let x = fx.rand_from(D, ra) >> 4;
            let (_, h) = fx.div(x, m);
            let f = ee::div(ee::from_i32(h - off), 100.0);
            let v = ee::add(fx.mem.f(sp + 0x80 + c), f);
            fx.mem.setf(p + c, v);
        }
        let i = first_free(fx, FLASHES, 0x4C, 0x10);
        if i >= 0 {
            let r = FLASHES + 0x4C * i as u32;
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
        fx.mem.write_u8(r + 0x27, 0xC0);
        fx.mem.write_u8(r + 0x23, 0);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 3);
        fx.mem.write_u8(r + 8, 0x28);
        let x = fx.rand_from(D, 0x01E0_6748);
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
    unit_position_saving(fx, t, sp + 0x80, u64::from(D), 0x01E0_5724);
    match fx.mem.u16(D) {
        0x1A4 => return 8,
        0xC0 => {
            for _ in 0..2 {
                let s = spawner_slot(fx);
                spawner(fx, s, [0xD0, 0, 3], None, (0xC, 0xFA0), sp + 0x80);
            }
        }
        0xBF => bolts_start(fx, sp),
        0xB0 => streaks_start(fx),
        0x80 => {
            for k in 0..2u32 {
                let s = spawner_slot(fx);
                let n = fx.mem.read_u8(D + 0xC + 4 * k);
                spawner(fx, s, [0x40, 4, 0], Some(n), (8, 0x44C), sp);
            }
        }
        0x4E => glow(fx),
        0x40 => charge(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x4E..0x190).contains(&n) && n & 1 == 0 {
        ribbons(fx);
    }
    let n = fx.mem.u16(D);
    if (0x4E..0x96).contains(&n) {
        fountain(fx, sp);
    }
    let n = fx.mem.u16(D);
    if (0xC0..0x18D).contains(&n) && n & 1 != 0 {
        crackle(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_67CC, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_67DC, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0x504C, OT);
    // fade's 0xF0 frame saves the runner's s6 (0) at +0x70 and ra at
    // +0x80: the streaks' first TEXFLUSH reads them as vertex 1's / 2's
    // stale XYZ (as b229) [run].
    fx.saved(sp - 0xF0, &[(0x70, 0), (0x80, 0x01E0_67EC)]);
    blib::radiate::streaks(fx, 0x01E0_0410, D + 0x1B34, OT, 1);
    blib::star::flash(fx, 0x01E0_1230, FLASHES, OT, 0x10);
    blib::streak::ribbon(fx, 0x01E0_1DE0, RIBBONS, OT, 0x3C);
    segment_spawns_saving(fx, 0x01E0_34C0, SPAWNERS, 8, 0x01E0_683C);
    blib::bolt::bolts(fx, 0x01E0_2970, BOLTS, OT, 0x28);
    blib::beam::beads(fx, 0x01E0_3840, D + 0x496C, OT, 0x14);
    blib::sprite::bounce(fx, SPRITES, OT, 0x64);
    let n = fx.mem.u16(D);
    if (0xC0..0x18D).contains(&n) {
        6
    } else if n == 0x18D {
        2
    } else if n == 0xB6 {
        1
    } else {
        0
    }
}
