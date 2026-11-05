//! BMODULE b526 (card 255), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05F00, data area a0 = 0x1ED0000, 0x3F10 bytes)
//! [F]: s0 = D, s1 = the OT (0x206E40), s2 the
//! parameter then offsets; a 0x1C0 frame (the target sp + 0x80). Init sets
//! the spark-emitter group (+0xA4, 8 sparks, both update lists the
//! record's 0x1E06EE8, nodes 0x33 / 0x3A). Update 10 four crossbars
//! (+0x3CC8, `cross::bars_b`) and a flash (+0x20D8) at node 0x1C with a
//! long weak shake; 30 a long flash (+0x2040) and a segment spawner
//! (+0x3400, `emit::segment_spawns_b`) at node 0x1C; 70 the attacker's
//! streaks (+0x7A8, `radiate::streaks_b`); 82 the fade (+0x3CC0), a shake,
//! eight beads (+0x35E0, `beam::beads_at`) from node 0x1C, a flash and
//! three jagged bolts (0x102A50, six segments each into +0x2500,
//! `bolt::bolts_b`) from node 0x1C to the target; 83 two spawners at the
//! target. On even updates 30..208 a spike at node 0x1C (+0x7E0), on odd
//! updates 83..219 two flashes and spikes about the target. Returns 6 on
//! updates 83..219, 2 on 220, 1 on 73, 8 on 240 (no drawing then) and at
//! once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080 (0538257fe1fa),
//! `sparks` 0x1E003A0, `fan_ring` 0x1E00C70, `emitters` 0x1E00F40,
//! `streaks_b` 0x1E01390, `flash_b` 0x1E02190, `spikes` 0x1E02C00,
//! `bolts_b` 0x1E036E0, `segment_spawns_b` 0x1E04170, `beads_at` 0x1E044B0,
//! `fade` 0x1E05130, `bars_b` 0x1E05600 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const STREAKS: u32 = D + 0x7A8;
const SPIKES: u32 = D + 0x7E0;
const FLASH: u32 = D + 0x2040;
const SEGS: u32 = D + 0x2500;
const SPAWNS: u32 = D + 0x3400;
const BEADS: u32 = D + 0x35E0;
const FADE: u32 = D + 0x3CC0;
const BARS: u32 = D + 0x3CC8;
const BYTES: u32 = D + 0x3E08;

/// The entry 0x1E05F00 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1C0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x1C0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x3F10).
    fx.bzero(D, 0x3F10);
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
    for (at, node, clear) in [(0xCu32, 0x1C, 1), (0x10, 0x33, 0), (0x14, 0x3A, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The emitter group (+0xA4).
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xCC, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_6EE8);
    fx.mem.write_u32(D + 0xB0, 0x01E0_6EE8);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 8);
    fx.mem.write_u32(D + 0xA8, D + 0x2A8);
    fx.mem.write_u8(D + 0xD5, 0x20);
    fx.mem.write_u8(D + 0xD6, 2);
    fx.mem.write_u16(D + 0xD8, 0);
    fx.mem.write_u32(D + 0xDC, 0x4348_0000);
    fx.mem.write_u32(D + 0xE4, 0x40E0_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xEA);
    // The entry's s0 = D, s1 = 3 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_6068, [Some(D), Some(3), Some(param as u32)]);
    0
}

fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 4
}

/// ((rand >> 4) % m - half) / 100.
fn jit(fx: &mut Fx, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

/// Update 10 [F 0x1E060E0]: four crossbars, a flash and a weak shake at
/// node 0x1C.
fn bars(fx: &mut Fx) {
    for s3 in 0..4i32 {
        let r = BARS + 0x50 * s3 as u32;
        set_counter4(fx, r, 0x14, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 4, 0x80);
        if s3 == 0 {
            fx.mem.write_u32(r + 0x10, 0x4496_0000);
            fx.mem.write_u32(r + 0x18, 0);
            fx.mem.write_u32(r + 0x14, 0x41A0_0000);
        } else {
            fx.mem.write_u32(r + 0x10, 0x4416_0000);
            fx.mem.setf(r + 0x18, ee::mul(45.0, ee::from_i32(s3)));
            fx.mem.write_u32(r + 0x14, 0x4140_0000);
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0x40), (9, 0x10), (8, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x40C0_0000);
    }
    let a = D + 0x20D8;
    set_counter4(fx, a, 0x1C, 0, 0xFF);
    fx.mem.write_u8(a + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(a + 6, b);
    fx.mem.write_u8(a + 4, 0x81);
    fx.mem.write_u8(a + 7, 9);
    fx.mem.write_u32(a + 0x10, 0x4396_0000);
    fx.mem.write_u32(a + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x10), (9, 0x20), (8, 0x80), (0xF, 0xD0)] {
        fx.mem.write_u8(a + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(a + o, 0);
    }
    set_counter4(fx, D + 0x40, 0xFA, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(D + 0x46, b);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x3F99_999A);
    fx.mem.write_u32(D + 0x58, 0x3F99_999A);
    fx.mem.write_u32(D + 0x5C, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x4000_0000);
}

/// A spawner record (+0x3400, 0x3C apart, 8) from 0x101C30 with no check
/// of a full pool [F]: index -1 writes the record before the first.
fn spawner(fx: &mut Fx) -> u32 {
    let i = first_free(fx, SPAWNS, 0x3C, 8);
    SPAWNS.wrapping_add((i.wrapping_mul(0x3C)) as u32)
}

/// Update 30 [F 0x1E062A0]: a long flash and a spawner at node 0x1C.
fn spawn(fx: &mut Fx) {
    let r = FLASH;
    set_counter4(fx, r, 0xA0, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    for (o, v) in [(4u32, 5u8), (7, 9), (0xA, 0x40), (9, 0x20), (8, 0x80), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, 0x4316_0000);
    fx.mem.write_u32(r + 0x14, 0);
    let r = spawner(fx);
    set_counter4(fx, r, 0x40, 4, 0);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    fx.mem.write_u8(r + 4, 2);
    fx.mem.write_u32(r + 8, SEGS);
    for (o, v) in [(0xCu32, 0x28u8), (0xD, 8), (0xE, 1), (0xF, 0), (0x10, 5), (0x11, 0xC)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0x44C);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    fx.mem.write_u32(r + 0x1C, 0x3ECC_CCCD);
    for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x38u32, 0x34, 0x30] {
        fx.mem.write_u32(r + o, 0);
    }
}

/// Update 70 [F 0x1E063EC]: the attacker's streaks.
fn streaks(fx: &mut Fx) {
    let r = STREAKS;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (9, 2), (0xA, 0xFF), (8, 8), (0xB, 0x18)] {
        fx.mem.write_u8(r + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(r + 0xC, h);
    for (o, v) in [(0x14u32, 0x80u8), (0x15, 0x80), (0x16, 0x80), (0x17, 0), (0x10, 0x90), (0x11, 0x90), (0x12, 0x90), (0x13, 0x40), (5, 1)] {
        fx.mem.write_u8(r + o, v);
    }
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
}

/// Update 82 [F 0x1E0646C]: the fade, a shake, eight beads, a flash and
/// three bolts from node 0x1C (sp + 0xA0) to the target. Returns the
/// entry's s6 after (the last bolt's segment count).
fn strike(fx: &mut Fx, sp: u32) -> u64 {
    set_counter4(fx, FADE, 0x10, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    set_counter4(fx, D + 0x40, 0x8C, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 8);
    fx.mem.write_u32(D + 0x54, 0x3F80_0000);
    fx.mem.write_u32(D + 0x58, 0x3FB3_3333);
    fx.mem.write_u32(D + 0x5C, 0x4000_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4320_0000);
    copy_v4(fx, sp + 0x80, D + 0x60);
    fx.mem.write_u32(D + 0x90, 0x4100_0000);
    fx.mem.write_u32(D + 0x68, 0);
    for s3 in 0..8i32 {
        let r = BEADS + 0x58 * s3 as u32;
        set_counter4(fx, r, 0x20, s3 & 0xFF, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(4);
        fx.mem.write_u8(r, c);
        for (o, v) in [(4u32, 5u8), (5, 0), (8, 0x1E), (9, s3 as u8), (0xA, 0), (0xC, 2)] {
            fx.mem.write_u8(r + o, v);
        }
        let v = rand4(fx, 0x01E0_654C) & 3;
        fx.mem.set8(r + 0xB, v);
        fx.mem.write_u32(r + 0x1C, 0x4248_0000);
        fx.mem.write_u32(r + 0x20, 0xBF19_999A);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
        node_position_saving(fx, h, s, r + 0x28, u64::from(D), Some(u64::from(OT)), 0x01E0_65A0);
    }
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, s, sp + 0xA0, u64::from(D), Some(u64::from(OT)), 0x01E0_65C0);
    let i = first_free(fx, FLASH, 0x4C, 0x10);
    if i >= 0 {
        let a = FLASH + 0x4C * i as u32;
        set_counter4(fx, a, 0x18, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0x80), (7, 9), (0xA, 0x40), (9, 0x20), (8, 0x80), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0)] {
            fx.mem.write_u8(a + o, v);
        }
        fx.mem.write_u32(a + 0x10, 0x42C8_0000);
        fx.mem.write_u32(a + 0x14, 0x4316_0000);
        fx.mem.write_u32(a + 0x18, 0x4270_0000);
        copy_v4(fx, sp + 0xA0, a + 0x1C);
    }
    let mut s6 = 0u64;
    for s2 in 0..3i32 {
        fx.mem.set32(sp + 0xC0, 6);
        fx.mem.write_u32(sp + 0xC4, 0x3E80_0000);
        fx.mem.write_u32(sp + 0xE0, sp + 0xF0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
            fx.mem.write_u32(sp + 0xC8 + 4 * c, w);
            let w = fx.mem.read_u32(sp + 0x80 + 4 * c);
            fx.mem.write_u32(sp + 0xD4 + 4 * c, w);
        }
        blib::bolt::jag(fx, sp + 0xC0);
        let mut p = sp + 0xF0;
        s6 = 0;
        while s6 < 6 {
            let i = first_free(fx, SEGS, 0x40, 0x3C);
            if i < 0 {
                break;
            }
            let r = SEGS + 0x40 * i as u32;
            set_counter4(fx, r, 0x8C, s2 & 1, 0xFF);
            fx.mem.write_u8(r + 5, 0);
            fx.mem.write_u8(r + 4, 0);
            for c in 0..3u32 {
                let w = fx.mem.read_u32(p + 4 * c);
                fx.mem.write_u32(r + 0x1C + 4 * c, w);
                let w = fx.mem.read_u32(p + 0xC + 4 * c);
                fx.mem.write_u32(r + 0x28 + 4 * c, w);
            }
            fx.mem.write_u8(r + 8, 9);
            fx.mem.write_u32(r + 0x10, 0x3F33_3333);
            fx.mem.write_u32(r + 0x14, 0x3E4C_CCCD);
            for (o, v) in [(0xBu32, 1u8), (0xD, 3), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)] {
                fx.mem.write_u8(r + o, v);
            }
            s6 += 1;
            p += 0xC;
        }
    }
    s6
}

/// Update 83 [F 0x1E067C4]: two spawners at the target.
fn spawners(fx: &mut Fx, sp: u32) {
    for _ in 0..2 {
        let r = spawner(fx);
        set_counter4(fx, r, 0x80, 0, 3);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u32(r + 8, SEGS);
        for (o, v) in [(0xCu32, 0x28u8), (0xD, 0xC), (0xE, 1), (0xF, 0), (0x10, 5), (0x11, 0xC)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x14, 0xFA0);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        fx.mem.write_u32(r + 0x1C, 0x3ECC_CCCD);
        for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * c);
            fx.mem.write_u32(r + 0x24 + 4 * c, w);
        }
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(r + o, 0);
        }
    }
}

/// Even updates 30..208 [F 0x1E068D0]: a spike at node 0x1C.
fn node_spike(fx: &mut Fx) {
    let i = first_free(fx, SPIKES, 0x68, 0x3C);
    if i < 0 {
        return;
    }
    let r = SPIKES + 0x68 * i as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    if fx.mem.u16(D) < 0x5A {
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        fx.mem.write_u32(r + 0x1C, 0x41A0_0000);
        fx.mem.write_u8(r + 8, 0xC);
    } else {
        fx.mem.write_u32(r + 0x14, 0x4040_0000);
        fx.mem.write_u32(r + 0x1C, 0x4220_0000);
        fx.mem.write_u8(r + 8, 0x14);
    }
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 3)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = rand4(fx, 0x01E0_6990);
    fx.mem.set8(r + 9, x);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x10), (0x27, 0x40)] {
        fx.mem.write_u8(r + o, v);
    }
    for (o, ra) in [(0x58u32, 0x01E0_69DCu32), (0x5C, 0x01E0_69F0), (0x60, 0x01E0_6A04)] {
        let v = rand4(fx, ra) & 0x7F;
        fx.mem.set32(r + o, v);
    }
}

/// Odd updates 83..219 [F 0x1E06A40]: twice a flash and a spike at a point
/// about the target (sp + 0x90; its w the stale sp + 0x9C).
fn crackle(fx: &mut Fx, sp: u32) {
    for _ in 0..2 {
        let x = ee::add(fx.mem.f(sp + 0x80), jit(fx, 0x01E0_6A48, 0x7D0, 0x3E8));
        fx.mem.setf(sp + 0x90, x);
        let y = ee::add(fx.mem.f(sp + 0x84), jit(fx, 0x01E0_6A88, 0x7D0, 0x3E8));
        fx.mem.setf(sp + 0x94, y);
        let z = ee::add(fx.mem.f(sp + 0x88), jit(fx, 0x01E0_6AC8, 0x3E8, 0x1F4));
        fx.mem.setf(sp + 0x98, z);
        let i = first_free(fx, FLASH, 0x4C, 0x10);
        if i >= 0 {
            let a = FLASH + 0x4C * i as u32;
            set_counter4(fx, a, 0xC, 0, 0xFF);
            for (o, v) in [(5u32, 0u8), (4, 1), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x60), (9, 0x10), (8, 0x40)] {
                fx.mem.write_u8(a + o, v);
            }
            fx.mem.write_u32(a + 0x14, 0);
            fx.mem.write_u32(a + 0x10, 0x4396_0000);
            copy_v4(fx, sp + 0x90, a + 0x1C);
        }
        let i = first_free(fx, SPIKES, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let r = SPIKES + 0x68 * i as u32;
        fx.mem.write_u8(r + 4, 5);
        set_counter4(fx, r, 8, 0, 0xFF);
        fx.mem.write_u32(r + 0x14, 0x40A0_0000);
        fx.mem.write_u32(r + 0x1C, 0x4220_0000);
        for (o, v) in [(0x27u32, 0xC0u8), (0x23, 0), (5, 0), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0x28)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = rand4(fx, 0x01E0_6C4C);
        fx.mem.set8(r + 9, x);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0x90, r + 0x28);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x80, u64::from(D), 0x01E0_6088);
    // The runner's s6 (0) unless the bolts ran.
    let mut s6 = 0u64;
    match fx.mem.u16(D) {
        0xF0 => return 8,
        0x53 => spawners(fx, sp),
        0x52 => s6 = strike(fx, sp),
        0x46 => streaks(fx),
        0x1E => spawn(fx),
        0xA => bars(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x1E..0xD2).contains(&n) && n & 1 == 0 {
        node_spike(fx);
    }
    let n = fx.mem.u16(D);
    if (0x53..0xDC).contains(&n) && n & 1 != 0 {
        crackle(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_6CC4, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6CD0, [Some(u64::from(D)), Some(u64::from(OT))]);
    let n = fx.mem.u16(D);
    blib::spawn::emitters(fx, 0x01E0_0F40, D + 0xA4, n);
    blib::sprite::sparks(fx, D + 0xE8, OT, 4);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves s6 at +0x70 and ra at +0x80 (the streaks'
    // stale vertex-1 XYZ).
    fx.saved(sp - 0xF0, &[(0x70, s6), (0x80, 0x01E0_6CF8)]);
    blib::radiate::streaks_with(fx, 0x01E0_1390, STREAKS, OT, 1, &blib::radiate::STREAKS_B);
    blib::star::flash_b(fx, 0x01E0_2190, FLASH, OT, 0x10);
    blib::spike::spikes(fx, 0x01E0_2C00, SPIKES, OT, 0x3C);
    blib::emit::segment_spawns_b(fx, 0x01E0_4170, SPAWNS, 8);
    // Its 0x80 frame's saves (s0 = D +0x10, s1 = the OT +0x20, ra +0x60)
    // are the next bolt strips' TEX0 / TEX1 when the texture is missing
    // [run]; [H] s2..s4 +0x30..+0x50 left.
    fx.saved(sp - 0x80, &[(0x10, u64::from(D)), (0x20, u64::from(OT)), (0x60, 0x01E0_6D38)]);
    blib::bolt::bolts_b(fx, 0x01E0_36E0, SEGS, OT, 0x28);
    blib::cross::bars_b(fx, 0x01E0_5600, BARS, OT, 4);
    blib::beam::beads_at(fx, 0x01E0_44B0, 0x34, 0x3C, BEADS, OT, 0x14);
    let n = fx.mem.u16(D);
    if n < 0x53 {
        if n == 0x49 {
            1
        } else {
            0
        }
    } else if n < 0xDC {
        6
    } else if n == 0xDC {
        2
    } else {
        0
    }
}
