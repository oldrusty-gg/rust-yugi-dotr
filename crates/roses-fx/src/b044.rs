//! BMODULE b044 (card 533), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06480, data area a0 = 0x1ED0000) [F] (helper D8,
//! 31 Oct 2026): b609's program (same library at the same addresses, same
//! data layout up to +0x504C) moved earlier and with two record-local
//! functions: a sprite-ring pool (+0x5054, 0x50 x 0x44 bytes, drawn by
//! 0x1E055C0) fed by two moving emitters (+0x6594, 0x5C bytes, 0x1E060E0);
//! the random bytes moved to +0x664C. On update 20 two long flashes on the
//! attacker's nodes 0x33 / 0x2E, the long camera shake and two segment
//! spawners there; on 28 the two emitters on nodes 0x14 / 0x38; on 32 the
//! streaks from node 2; on 46 the fade, a shake at the target, eight bead
//! beams on node 0x33, a flash and four jagged bolts to the target; on 47
//! two spawners at the target; on even updates 20..254 two ribbons a update
//! from the nodes; on 5..45 four bouncing sprites a update; on odd updates
//! 47..251 two flashes and two ribbons round the target. Returns 1 on
//! update 37, 6 on 47..251, 2 on 252, 8 on 290 (and at once for a parameter
//! of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `streaks`
//! 0x1E00410, `flash` 0x1E01230, `ribbon` 0x1E01DE0, `bolts` 0x1E02970,
//! `segment_spawns` 0x1E034C0, `beads` 0x1E03840, `fade` 0x1E04610,
//! `bounce` 0x1E04B50 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b586::segment_spawns_saving;
use crate::blib::exe::{
    aim_angles, atlas_cell, camera_vp_into, countdown, first_free, libm_cosf, mul_mem, node_source, rot_trans_into,
    set_counter4, tex_rect,
};
use crate::blib::{self, arc::resample};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x664C;
const FLASHES: u32 = D + 0x33CC;
const BOLTS: u32 = D + 0x388C;
const SPAWNERS: u32 = D + 0x478C;
const RIBBONS: u32 = D + 0x1B6C;
const SPRITES: u32 = D + 0xA4;
const RINGS: u32 = D + 0x5054;
const EMITTERS: u32 = D + 0x6594;

/// The entry 0x1E06480 (a0 = D, a1 = param; s0 = D, s1 = the OT).
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
    fx.bzero(D, 0x6750);
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
    for (at, node, clear) in [(0xCu32, 0x33, 1), (0x10, 0x2E, 0), (0x14, 2, 0), (0x18, 0x14, 0), (0x1C, 0x38, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x11C);
    // The entry's s0 = D, s1 = the OT, s2 = 3 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_65EC, [Some(D), Some(OT), Some(3)]);
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
    for (o, v) in [(0xCu32, 0x3Cu8), (0xD, spread.0), (0xE, 1), (0xF, 0), (0x10, 5), (0x11, 0xC)] {
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

/// Update 20 [F 0x1E06668]: a long flash on each node, the long shake,
/// a segment spawner on each node.
fn charge(fx: &mut Fx, sp: u32) {
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
    for k in 0..2u32 {
        let s = spawner_slot(fx);
        let n = fx.mem.read_u8(D + 0xC + 4 * k);
        spawner(fx, s, [0x1A, 4, 0], Some(n), (8, 0x4B0), sp);
    }
}

/// Update 28 [F 0x1E06894]: the two emitters on nodes +0x18 / +0x1C.
fn emitters_start(fx: &mut Fx) {
    for k in 0..2u32 {
        let g = EMITTERS + 0x5C * k;
        set_counter4(fx, g, 0xE, 0, 0xFF);
        fx.mem.write_u8(g + 4, 0);
        fx.mem.write_u8(g + 5, 1);
        let b = fx.mem.read_u8(D + 0x18 + 4 * k);
        fx.mem.write_u8(g + 6, b);
        fx.mem.write_u32(g + 8, RINGS);
        fx.mem.write_u32(g + 0xC, BYTES);
        for (o, v) in [(0x10u32, 0x50u8), (0x11, 0x20), (0x12, 2), (0x13, 2), (0x14, 5), (0x15, 2), (0x16, 1), (0x17, 3)] {
            fx.mem.write_u8(g + o, v);
        }
        fx.mem.write_u32(g + 0x18, 0x3ECC_CCCD);
        fx.mem.write_u32(g + 0x1C, 0x3FC0_0000);
        fx.mem.write_u32(g + 0x20, 0x420C_0000);
        fx.mem.write_u32(g + 0x24, 0x40E0_0000);
        for (o, v) in [(0x28u32, 0x80u8), (0x29, 0x80), (0x2A, 0x80), (0x2B, 0x30)] {
            fx.mem.write_u8(g + o, v);
        }
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0x18 + 4 * k);
        node_position_saving(fx, h, n, g + 0x3C, u64::from(D), Some(u64::from(OT)), 0x01E0_6984);
        copy_v4(fx, g + 0x3C, g + 0x4C);
    }
}

/// Update 32 [F 0x1E069C4]: the streaks from node +0x14.
fn streaks_start(fx: &mut Fx) {
    let s = D + 0x1B34;
    set_counter4(fx, s, 0x1C, 0, 0xFF);
    fx.mem.write_u8(s + 4, 0x80);
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

/// Update 46 [F 0x1E06A4C]: the fade, the shake at the target, eight
/// bead beams on node +0xC, a flash and four jagged bolts to the target.
fn bolts_start(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0x504C, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0x5050, 1);
    let s = D + 0x40;
    set_counter4(fx, s, 0xD0, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
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
        let x = fx.rand_from(D, 0x01E0_6B60) >> 4;
        fx.mem.set8(r + 0xB, x & 3);
        fx.mem.write_u32(r + 0x1C, 0x4248_0000);
        fx.mem.write_u32(r + 0x20, 0xBF19_999A);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0xC);
        node_position_saving(fx, h, n, r + 0x28, u64::from(D), Some(u64::from(OT)), 0x01E0_6BBC);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0xA0, u64::from(D), Some(u64::from(OT)), 0x01E0_6BE0);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x10);
    node_position_saving(fx, h, n, sp + 0xB0, u64::from(D), Some(u64::from(OT)), 0x01E0_6BF4);
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

/// Even updates 20..254 [F 0x1E06F78]: a ribbon from each node.
fn ribbons(fx: &mut Fx) {
    for k in 0..2u32 {
        let i = first_free(fx, RIBBONS, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let r = RIBBONS + 0x68 * i as u32;
        set_counter4(fx, r, 0x10, 0, 0xFF);
        fx.mem.write_u8(r + 4, 5);
        if fx.mem.u16(D) < 0x2E {
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
        let x = fx.rand_from(D, 0x01E0_7060);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x23, 0x10);
        fx.mem.write_u8(r + 0x27, 0x40);
        for (o, ra) in [(0x58u32, 0x01E0_70B0u32), (0x5C, 0x01E0_70C8), (0x60, 0x01E0_70E0)] {
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

/// Updates 5..45 [F 0x1E07124]: four bouncing sprites a update from the
/// nodes (+0xC / +0x10 alternately, sp + 0xA0).
fn fountain(fx: &mut Fx, sp: u32) {
    for k in 0..4u32 {
        let i = first_free(fx, SPRITES, 0x44, 0x64);
        if i < 0 {
            continue;
        }
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0xC + 4 * (k & 1));
        node_position_saving(fx, h, n, sp + 0xA0, u64::from(D), Some(u64::from(OT)), 0x01E0_7170);
        let x = fx.rand_from(D, 0x01E0_7178) >> 4;
        let (_, m) = fx.div(x, 6);
        let f = ee::div(ee::from_i32(m * 200 - 0x258), 100.0);
        let life = ee::add(16.0, f);
        fx.fix_residue(life, 0x01E0_71E0);
        let c = ee::fixsfsi(life);
        let b = SPRITES + 0x44 * i as u32;
        set_counter4(fx, b, c & 0xFF, 0, 0xFF);
        fx.mem.write_u8(b + 4, 0x41);
        fx.mem.write_u8(b + 8, 0);
        fx.mem.write_u8(b + 0xB, 2);
        fx.mem.write_u8(b + 9, 3);
        fx.mem.write_u8(b + 0xA, 0);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(b + o, 0x80);
        }
        fx.mem.write_u32(b + 0x10, 0x41F0_0000);
        fx.mem.write_u32(b + 0x14, 0xBF80_0000);
        fx.mem.write_u32(b + 0x1C, 0);
        fx.mem.write_u32(b + 0x18, 0);
        for (c, ra) in [(0u32, 0x01E0_7260u32), (4, 0x01E0_72C4), (8, 0x01E0_7328)] {
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

/// Odd updates 47..251 [F 0x1E07430]: two flashes and two ribbons a
/// update round the target (+-10 in x, y, +-5 in z; sp + 0x90).
fn crackle(fx: &mut Fx, sp: u32) {
    let p = sp + 0x90;
    for _ in 0..2 {
        for (c, ra, m, off) in [(0u32, 0x01E0_7444u32, 0x7D0, 0x3E8), (4, 0x01E0_7490, 0x7D0, 0x3E8), (8, 0x01E0_74DC, 0x3E8, 0x1F4)] {
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
        let x = fx.rand_from(D, 0x01E0_7688);
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
    unit_position_saving(fx, t, sp + 0x80, u64::from(D), 0x01E0_6614);
    match fx.mem.u16(D) {
        0x122 => return 8,
        0x2F => {
            for _ in 0..2 {
                let s = spawner_slot(fx);
                spawner(fx, s, [0xD0, 0, 3], None, (0xC, 0xFA0), sp + 0x80);
            }
        }
        0x2E => bolts_start(fx, sp),
        0x20 => streaks_start(fx),
        0x1C => emitters_start(fx),
        0x14 => charge(fx, sp),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x14..0xFF).contains(&n) && n & 1 == 0 {
        ribbons(fx);
    }
    let n = fx.mem.u16(D);
    if (5..0x2E).contains(&n) {
        fountain(fx, sp);
    }
    let n = fx.mem.u16(D);
    if (0x2F..0xFC).contains(&n) && n & 1 != 0 {
        crackle(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_770C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_771C, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0x504C, OT);
    // fade's 0xF0 frame saves the runner's s6 (0) at +0x70 and ra at
    // +0x80: the streaks' first TEXFLUSH reads them (as b609) [run].
    fx.saved(sp - 0xF0, &[(0x70, 0), (0x80, 0x01E0_772C)]);
    blib::radiate::streaks(fx, 0x01E0_0410, D + 0x1B34, OT, 1);
    blib::star::flash(fx, 0x01E0_1230, FLASHES, OT, 0x10);
    blib::streak::ribbon(fx, 0x01E0_1DE0, RIBBONS, OT, 0x3C);
    segment_spawns_saving(fx, 0x01E0_34C0, SPAWNERS, 8, 0x01E0_777C);
    blib::bolt::bolts(fx, 0x01E0_2970, BOLTS, OT, 0x3C);
    blib::beam::beads(fx, 0x01E0_3840, D + 0x496C, OT, 0x14);
    blib::sprite::bounce(fx, SPRITES, OT, 0x64);
    emitters(fx, EMITTERS, 2);
    rings(fx, RINGS, OT, 0x50);
    let n = fx.mem.u16(D);
    if (0x2F..0xFC).contains(&n) {
        6
    } else if n == 0xFC {
        2
    } else if n == 0x25 {
        1
    } else {
        0
    }
}

/// 0 or a denormal, as the EE's c.eq.s with 0 sees it.
fn is_zero(v: f32) -> bool {
    v.to_bits() & 0x7F80_0000 == 0
}

fn copy4(fx: &mut Fx, from: u32, to: u32) {
    for k in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 165ea098804c, record-local 0x1E060E0 (232 words) [F]: `emitters(g, _,
/// n) -> 1` steps `n` emitter records (0x5C bytes; a 0x2B0 frame) that
/// leave sprite-ring records (0x44 bytes, [`rings`]) along their path:
/// `emit::bead_trails_b`'s shape without the velocities. Per record:
/// countdown (0x101AF0), nothing on its first count; the position +0x2C
/// from a node (0x101BA0); +0x17 < 2: one record at +0x2C, else +0x17
/// records at the samples of the curve through +0x2C, +0x3C, +0x4C
/// (0x102550 on the path at sp + 0x70, samples from sp + 0xD4). Per record
/// a free one of the pool *(+8) (+0x10 of them, 0x101C30), counted +0x11
/// (0x101AD0): +0x14 = +0x18 (unless 0), +0x18 = +0x1C, +4 = +4, +5 = 0,
/// +8 = +0x12, sizes +0x1C / +0x20 = +0x20 / +0x24, +9 = rand >> 4, +0xA
/// .. +0xD = +0x13 .. +0x16, +0x10 = +0xC, colour +0x24 = +0x28, position
/// +0x38 = the sample, angles +0x28 along (0, 1, 0) (0x101940). Then the
/// history +0x4C = +0x3C, +0x3C = +0x2C.
fn emitters(fx: &mut Fx, g: u32, n: i32) -> i32 {
    emitters_with(fx, g, n, 0x01E0_630C)
}

/// [`emitters`] compiled elsewhere (b196 0x1E010B0, a8e977094cd0: the
/// index arithmetic in registers), with rand's return address `rand_ra`.
pub(crate) fn emitters_with(fx: &mut Fx, g: u32, n: i32, rand_ra: u32) -> i32 {
    let sp = fx.enter(0x2B0);
    let mut g = g;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x5C;
        }
        emitter(fx, sp, g, rand_ra);
        i += 1;
    }
    fx.leave(0x2B0);
    1
}

fn emitter(fx: &mut Fx, sp: u32, g: u32, rand_ra: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    if fx.mem.u8(g) == fx.mem.u8(g + 2) - 1 {
        return;
    }
    node_source(fx, g, g + 0x2C);
    let segs = if fx.mem.u8(g + 0x17) < 2 {
        copy4(fx, g + 0x2C, sp + 0xD4);
        copy4(fx, g + 0x3C, sp + 0xE4);
        1
    } else {
        let p = sp + 0x70;
        fx.mem.set32(p, 3);
        for (k, src) in [0x2Cu32, 0x3C, 0x4C].into_iter().enumerate() {
            for c in 0..3u32 {
                let w = fx.mem.read_u32(g + src + 4 * c);
                fx.mem.write_u32(p + 4 + 4 * k as u32 + 0x20 * c, w);
            }
        }
        resample(fx, p);
        fx.mem.u8(g + 0x17)
    };
    for k in 0..segs {
        let pool = fx.mem.read_u32(g + 8);
        let count = fx.mem.u8(g + 0x10);
        let i = first_free(fx, pool, 0x44, count);
        if i < 0 {
            continue;
        }
        let b = pool.wrapping_add(0x44 * i as u32);
        let c = fx.mem.u8(g + 0x11);
        set_counter4(fx, b, c, 0, 0xFF);
        fx.mem.write_u32(sp + 0x2A8, 0);
        fx.mem.write_u32(sp + 0x2A0, 0);
        fx.mem.write_u32(sp + 0x2A4, 0x3F80_0000);
        let s = fx.mem.f(g + 0x18);
        if !is_zero(s) {
            fx.mem.setf(b + 0x14, s);
        }
        let w = fx.mem.read_u32(g + 0x1C);
        fx.mem.write_u32(b + 0x18, w);
        let v = fx.mem.read_u8(g + 4);
        fx.mem.write_u8(b + 4, v);
        fx.mem.write_u8(b + 5, 0);
        let v = fx.mem.read_u8(g + 0x12);
        fx.mem.write_u8(b + 8, v);
        for (dst, src) in [(0x1Cu32, 0x20u32), (0x20, 0x24)] {
            let w = fx.mem.read_u32(g + src);
            fx.mem.write_u32(b + dst, w);
        }
        let r = fx.rand_from(g, rand_ra);
        fx.mem.set8(b + 9, r >> 4);
        for (dst, src) in [(0xAu32, 0x13u32), (0xB, 0x14), (0xC, 0x15), (0xD, 0x16)] {
            let v = fx.mem.read_u8(g + src);
            fx.mem.write_u8(b + dst, v);
        }
        let w = fx.mem.read_u32(g + 0xC);
        fx.mem.write_u32(b + 0x10, w);
        for c in 0..4u32 {
            let v = fx.mem.read_u8(g + 0x28 + c);
            fx.mem.write_u8(b + 0x24 + c, v);
        }
        let q = sp + 0xD4 + 0x10 * k as u32;
        for c in 0..3u32 {
            let w = fx.mem.read_u32(q + 4 * c);
            fx.mem.write_u32(b + 0x38 + 4 * c, w);
        }
        aim_angles(fx, sp + 0x2A0, b + 0x28);
    }
    copy4(fx, g + 0x3C, g + 0x4C);
    copy4(fx, g + 0x2C, g + 0x3C);
}

/// 0x1A2C70 of `v` into the u16 at `a`.
fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v) as i32;
    fx.mem.set16(a, x);
}

/// A byte as the code converts it (cvt.s.w; a byte is never negative).
fn byte_f(v: i32) -> f32 {
    ee::from_i32(v)
}

/// 03d05b171026, record-local 0x1E055C0 (708 words) [F]: `rings(g, ot,
/// n) -> 1` steps and draws `n` sprite-ring records (0x44 bytes; a 0x1E0
/// frame, the descriptor at sp + 0x158). Per record: countdown (0x101AF0);
/// the own-slot texture +0xA (0x202530(5)); size +0x1C += +0x20; the
/// position +0x38 (from a node with +5, 0x101BA0) through the camera x
/// (angles +0x28, position) (0x20E2B0, 0x20C0B0, 0x20B370); colour +0x26,
/// +0x25, +0x24, Q 1; the atlas cell +0xC of layout +0xB with blend +0xD
/// (0x1020B0, 0x101D20); alpha +0x27 (x count / 8 over the last 8 unless
/// flag 0x40). Then +8 sprites from the byte table *(+0x10) at index +9
/// (u8): the first at the record's origin, each other round it at the
/// angle t[i] x 360 / 255 degrees (libm cosf / sinf 0x1A7158 / 0x1A7348
/// times radius +0x18 in x / z) and the height +0x14 x (elapsed + t[i +
/// 1] & 3) x t[i + 1] / 255 (i += 3); projected (0x204D80, 0x101600) or
/// skipped, half size 0x1018E0(+0x1C) (y x 1.33), corners by the next
/// byte's bits 0x30 (as `sprite::bounce`); flag 0x80 at the far Z with
/// key 0, else at its depth.
fn rings(fx: &mut Fx, g: u32, ot: u32, n: i32) -> i32 {
    rings_with(fx, g, ot, n, 0x1E0)
}

/// [`rings`] compiled with a `frame` (b196 0x1E00440, ea834aa07372: 0x1D0,
/// no s7 save, every local 0x10 lower = at the same caller-relative
/// address, the byte index stepped one at a time); `sp` below is the
/// 0x1E0-frame base.
pub(crate) fn rings_with(fx: &mut Fx, g: u32, ot: u32, n: i32, frame: u32) -> i32 {
    let sp = fx.enter(frame) + frame - 0x1E0;
    let mut g = g;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x44;
        }
        ring(fx, sp, g, ot);
        i += 1;
    }
    fx.leave(frame);
    1
}

fn ring(fx: &mut Fx, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 0xA);
    let tex = fx.texture(5, id);
    let v = ee::add(fx.mem.f(g + 0x1C), fx.mem.f(g + 0x20));
    fx.mem.setf(g + 0x1C, v);
    for k in 0..3u32 {
        let w = fx.mem.read_u32(g + 0x38 + 4 * k);
        fx.mem.write_u32(sp + 0x130 + 4 * k, w);
    }
    node_source(fx, g, sp + 0x130);
    camera_vp_into(fx, sp + 0xE0);
    rot_trans_into(fx, sp + 0xA0, g + 0x28, sp + 0x130);
    mul_mem(fx, sp + 0xE0, sp + 0xA0);
    let mut idx = fx.mem.u8(g + 9);
    let elapsed = fx.mem.u8(g + 2) - fx.mem.u8(g);
    let d = sp + 0x158;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (src, dst) in [(0x26u32, 0x168u32), (0x25, 0x169), (0x24, 0x16A)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + dst, v);
    }
    fx.mem.write_u32(sp + 0x16C, 0x3F80_0000);
    let b = fx.mem.read_u8(g + 0xD);
    fx.mem.write_u8(sp + 0x145, b);
    let (cell, layout) = (fx.mem.u8(g + 0xC), fx.mem.u8(g + 0xB));
    atlas_cell(fx, sp + 0x140, cell, layout);
    tex_rect(fx, d, sp + 0x140, tex);
    let c = fx.mem.u8(g);
    if c < 8 && fx.mem.u8(g + 4) & 0x40 == 0 {
        let k = fx.mem.u8(g + 0x27) >> 3;
        let v = fx.mult(k, c);
        fx.mem.set8(sp + 0x16B, v);
    } else {
        let v = fx.mem.read_u8(g + 0x27);
        fx.mem.write_u8(sp + 0x16B, v);
    }
    let count = fx.mem.u8(g + 8);
    let mut s = 0;
    while s < count {
        if s == 0 {
            for o in [0x128u32, 0x124, 0x120] {
                fx.mem.write_u32(sp + o, 0);
            }
        } else {
            let t = fx.mem.read_u32(g + 0x10);
            let a = fx.mem.u8(t.wrapping_add((idx & 0xFF) as u32));
            let f = ee::div(ee::mul(360.0, byte_f(a)), 255.0);
            let f = ee::mul(f32::from_bits(0x4049_0FDB), f);
            let rad = ee::div(f, 180.0);
            let cs = libm_cosf(fx, rad);
            let v = ee::mul(fx.mem.f(g + 0x18), cs);
            fx.mem.setf(sp + 0x120, v);
            let sn = fx.libm_sinf(rad);
            let v = ee::mul(fx.mem.f(g + 0x18), sn);
            fx.mem.setf(sp + 0x128, v);
            let t = fx.mem.read_u32(g + 0x10);
            let h = fx.mem.u8(t.wrapping_add(((idx + 1) & 0xFF) as u32));
            let y = ee::mul(fx.mem.f(g + 0x14), ee::from_i32(elapsed + (h & 3)));
            let y = ee::div(ee::mul(y, byte_f(h)), 255.0);
            fx.mem.setf(sp + 0x124, y);
            idx = (idx + 3) & 0xFF;
        }
        sprite(fx, sp, g, ot, &mut idx);
        s += 1;
    }
}

fn sprite(fx: &mut Fx, sp: u32, g: u32, ot: u32, idx: &mut i32) {
    let m = fx.read_m4(sp + 0xE0);
    if !fx.project_at(sp + 0x120, &m, sp + 0x1D0, sp + 0x1DC) {
        return;
    }
    if !Fx::on_screen(fx.mem.read_u64(sp + 0x1D0)) {
        return;
    }
    let (size, w) = (fx.mem.f(g + 0x1C), fx.mem.f(sp + 0x1DC));
    let half = fx.view_scale(size, w);
    let t = fx.mem.read_u32(g + 0x10);
    let o = fx.mem.u8(t.wrapping_add((*idx & 0xFF) as u32));
    *idx = (*idx + 1) & 0xFF;
    // (x minus, x plus, y minus, y plus) corner pairs by orientation.
    let (xm, xp, ym, yp): ([u32; 2], [u32; 2], [u32; 2], [u32; 2]) = match o & 0x30 {
        0x10 => ([0x188, 0x178], [0x1A8, 0x198], [0x1AA, 0x18A], [0x19A, 0x17A]),
        0x20 => ([0x1A8, 0x188], [0x198, 0x178], [0x1AA, 0x19A], [0x18A, 0x17A]),
        0x30 => ([0x1A8, 0x198], [0x188, 0x178], [0x19A, 0x17A], [0x1AA, 0x18A]),
        _ => ([0x198, 0x178], [0x1A8, 0x188], [0x18A, 0x17A], [0x1AA, 0x19A]),
    };
    let x = ee::from_i32(fx.mem.u16(sp + 0x1D0));
    for o in xm {
        fix16(fx, sp + o, ee::sub(x, half));
    }
    for o in xp {
        fix16(fx, sp + o, ee::add(x, half));
    }
    let y = ee::from_i32(fx.mem.u16(sp + 0x1D2));
    let hy = ee::mul(f32::from_bits(0x3FAA_3D71), half);
    for o in ym {
        fix16(fx, sp + o, ee::sub(y, hy));
    }
    for o in yp {
        fix16(fx, sp + o, ee::add(y, hy));
    }
    let mut z = if fx.mem.u8(g + 4) & 0x80 != 0 {
        fx.mem.write_u32(sp + 0x1DC, 0);
        0xFF_FFFE
    } else {
        fx.xyz_z(sp + 0x1D0)
    };
    for o in [0x1A8u32, 0x198, 0x188, 0x178] {
        fx.set_xyz_z(sp + o, z);
        z = fx.xyz_z(sp + o);
    }
    for o in [0x1AFu32, 0x19F, 0x18F, 0x17F] {
        fx.mem.write_u8(sp + o, 0);
    }
    let key = ee::fixunssfsi(fx.mem.f(sp + 0x1DC)) as i32;
    fx.submit(sp + 0x158, ot, key);
}
