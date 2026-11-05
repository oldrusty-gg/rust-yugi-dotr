//! BMODULE b164 (card 172), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E055C0, data area a0 = 0x1ED0000, 0x7850 bytes)
//! [F]: b586's library (B4) with streaks on
//! its own unscheduled program (nops in the delay slots). s0 = D, s1 =
//! the OT (0x206E40), a 0x1D0 frame (the target sp + 0x90). Update 2 a
//! long flash (+0x193C, 24 x 0x4C) at node 0x26 and a long shake; 89 the
//! attacker's streaks (+0xA4) and a flash; 150 the fade (+0x421C) and six
//! jagged bolts (0x102A50, five segments each into +0x205C) between nodes
//! 0x26 / 0x15 (odd ones from node 0x26 to 60 above it); 220 a flash at
//! node 0x22 and a shake at the target; 222 eight beads (+0x3B3C) at node
//! 0x22 and four bolts (six segments) from node 0x26 to the target.
//! Updates 20..149 three sprites a update (+0x4224, 0x44) falling onto
//! nodes 0x26 / 0x24 / 0x22; odd 89..345 a ribbon (+0xDC, 0x68) at node
//! slot D+0x20; 161..221 a bolt segment and a flash about node 0x22;
//! 222..361 twice a flash and a ribbon about the target. The object ops
//! table 0x1E06E50 ([`crate::b035::object_ops`]), the segment spawners
//! (+0x395C) drawn never set. Returns 6 on updates 222..358, 2 on 359, 1
//! on 212, 8 on 380 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080 (2fb7df12f780),
//! `streaks` 0x1E00410, `flash` 0x1E01230, `ribbon` 0x1E01DE0, `bolts`
//! 0x1E02970, `segment_spawns` 0x1E034C0, `beads` 0x1E03840, `fade`
//! 0x1E04610, `bounce` 0x1E04B50 [`crate::blib`].
use crate::b035::{light_ramp, object_ops};
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b586::segment_spawns_saving;
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const STREAKS: u32 = D + 0xA4;
const RIBBONS: u32 = D + 0xDC;
const FLASH: u32 = D + 0x193C;
const BOLTS: u32 = D + 0x205C;
const SPAWNS: u32 = D + 0x395C;
const BEADS: u32 = D + 0x3B3C;
const FADE: u32 = D + 0x421C;
const SPRITES: u32 = D + 0x4224;
const BYTES: u32 = D + 0x7744;
const OPS: u32 = 0x01E0_6E50;

/// The entry 0x1E055C0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1D0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x1D0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0x7850).
    fx.bzero(D, 0x7850);
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
    for (at, node, clear) in [(0xCu32, 0x26, 1), (0x10, 0x24, 0), (0x14, 0x22, 0), (0x18, 0x15, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x176);
    // The entry's s0 = D, s1 = the OT, s2 = 2 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_571C, [Some(D), Some(OT), Some(2)]);
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

/// (200 x ((rand >> 4) % m) - `sub`) / 100.
fn jit200(fx: &mut Fx, ra: u32, m: i32, sub: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r.wrapping_mul(200) - sub), 100.0)
}

fn node_pos(fx: &mut Fx, slot: u32, out: u32, ra: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + slot));
    node_position_saving(fx, h, s, out, u64::from(D), Some(u64::from(OT)), ra);
}

/// A flash from 0x101C30: `count` / `delay`; bytes +4 / +7 / +0xA / +9 /
/// +8, colour `c`, +0x10 / +0x14; on node slot D+0xC when `on_node`.
fn flash(fx: &mut Fx, count: i32, delay: i32, b: [u8; 5], c: u8, size: [u32; 2], on_node: bool) -> Option<u32> {
    let i = first_free(fx, FLASH, 0x4C, 0x18);
    if i < 0 {
        return None;
    }
    let r = FLASH + 0x4C * i as u32;
    set_counter4(fx, r, count, delay, 0xFF);
    if on_node {
        fx.mem.write_u8(r + 5, 1);
        let v = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(r + 6, v);
    } else {
        fx.mem.write_u8(r + 5, 0);
    }
    for (o, v) in [(4u32, b[0]), (7, b[1]), (0xA, b[2]), (9, b[3]), (8, b[4]), (0xF, c), (0xC, c), (0xD, c), (0xE, c)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, size[0]);
    fx.mem.write_u32(r + 0x14, size[1]);
    Some(r)
}

/// Update 2 [F 0x1E05798]: a long flash on node 0x26 and a long shake.
fn opening(fx: &mut Fx) {
    flash(fx, 0xFF, 0, [0x83, 9, 0x60, 0x60, 0x20], 0x80, [0x42F0_0000, 0], true);
    let s = D + 0x40;
    set_counter4(fx, s, 0xFF, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, v);
    fx.mem.write_u8(s + 4, 4);
    fx.mem.write_u32(s + 0x14, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x4040_0000);
}

/// Update 89 [F 0x1E058A4]: the attacker's streaks from node 0x22 and a
/// flash on node 0x26.
fn streaks_on(fx: &mut Fx) {
    let s = STREAKS;
    set_counter4(fx, s, 0x1C, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (9, 2), (0xA, 0xFF), (8, 0xC), (0xB, 0x18)] {
        fx.mem.write_u8(s + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(s + 0xC, h);
    for (o, v) in [(0x14u32, 0x80u8), (0x15, 0x80), (0x16, 0x80), (0x17, 0), (0x10, 0x80), (0x11, 0x80), (0x12, 0x80), (0x13, 0x30), (5, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(s + 6, b);
    flash(fx, 0x18, 2, [0x81, 9, 0x60, 0x60, 0x20], 0xA0, [0x4248_0000, 0x4170_0000], true);
}

/// A jagged bolt (0x102A50) of `n` points from sp + 0xB0 to `to`, its
/// segments (`count`, delay `delay`, kind +4 `k4`, +8 `k8`, widths, alpha)
/// into the bolt pool (`pool` records).
fn bolt(fx: &mut Fx, sp: u32, n: i32, to: u32, pool: i32, seg: (i32, i32, u8, u8, u32, u32, u8)) {
    fx.mem.set32(sp + 0xD0, n);
    fx.mem.write_u32(sp + 0xD4, 0x3F00_0000);
    fx.mem.write_u32(sp + 0xF0, sp + 0x100);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0xB0 + 4 * c);
        fx.mem.write_u32(sp + 0xD8 + 4 * c, w);
    }
    for c in 0..3u32 {
        let w = fx.mem.read_u32(to + 4 * c);
        fx.mem.write_u32(sp + 0xE4 + 4 * c, w);
    }
    blib::bolt::jag(fx, sp + 0xD0);
    let (count, delay, k4, k8, w0, w1, alpha) = seg;
    let mut s2 = 0;
    while s2 < n {
        let i = first_free(fx, BOLTS, 0x40, pool);
        if i < 0 {
            break;
        }
        let r = BOLTS + ((i as u32) << 6);
        set_counter4(fx, r, count, delay & 0xFF, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, k4);
        let p = sp + 0x100 + 0xC * s2 as u32;
        for c in 0..3u32 {
            let w = fx.mem.read_u32(p + 4 * c);
            fx.mem.write_u32(r + 0x1C + 4 * c, w);
        }
        for c in 0..3u32 {
            let w = fx.mem.read_u32(p + 0xC + 4 * c);
            fx.mem.write_u32(r + 0x28 + 4 * c, w);
        }
        fx.mem.write_u8(r + 8, k8);
        fx.mem.write_u32(r + 0x10, w0);
        fx.mem.write_u32(r + 0x14, w1);
        for (o, v) in [(0xBu32, 1u8), (0xD, 3), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, alpha)] {
            fx.mem.write_u8(r + o, v);
        }
        s2 += 1;
    }
}

/// Update 150 [F 0x1E059D0]: the fade and six jagged bolts of five
/// segments, even ones node 0x26 -> node 0x15, odd ones node 0x26 -> 60
/// above it (delay 1).
fn bolts_150(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 0x10, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    node_pos(fx, 0x18, sp + 0xB0, 0x01E0_5A04);
    for s4 in 0..6i32 {
        let s5 = s4 & 1;
        if s5 != 0 {
            node_pos(fx, 0xC, sp + 0xB0, 0x01E0_5A30);
            copy_v4(fx, sp + 0xB0, sp + 0xC0);
            let y = ee::add(fx.mem.f(sp + 0xC4), 60.0);
            fx.mem.setf(sp + 0xC4, y);
        } else {
            node_pos(fx, 0xC, sp + 0xB0, 0x01E0_5A8C);
            node_pos(fx, 0x18, sp + 0xC0, 0x01E0_5AA0);
        }
        bolt(fx, sp, 5, sp + 0xC0, 0x64, (0x30, s5, 0x80, 6, 0x3F4C_CCCD, 0x3E99_999A, 0x20));
    }
}

/// Update 220 [F 0x1E05C14]: a flash at node 0x22 (sp + 0xB0), a shake
/// at the target.
fn flash_220(fx: &mut Fx, sp: u32) {
    node_pos(fx, 0x14, sp + 0xB0, 0x01E0_5C28);
    if let Some(r) = flash(fx, 0x18, 0, [0x80, 9, 0x40, 0x20, 0x80], 0xF0, [0x42C8_0000, 0x42DC_0000], false) {
        fx.mem.write_u32(r + 0x18, 0x4270_0000);
        copy_v4(fx, sp + 0xB0, r + 0x1C);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x8C, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4320_0000);
    copy_v4(fx, sp + 0x90, s + 0x20);
    fx.mem.write_u32(s + 0x50, 0x4100_0000);
    fx.mem.write_u32(s + 0x28, 0);
}

/// Update 222 [F 0x1E05D74]: eight beads at node 0x22 and four bolts of
/// six segments from node 0x26 to the target.
fn beads_222(fx: &mut Fx, sp: u32) {
    for s3 in 0..8i32 {
        let r = BEADS + 0x58 * s3 as u32;
        set_counter4(fx, r, 0x20, s3 & 0xFF, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(4);
        fx.mem.write_u8(r, c);
        for (o, v) in [(4u32, 5u8), (5, 0), (8, 0x1E), (9, s3 as u8), (0xA, 0), (0xC, 2)] {
            fx.mem.write_u8(r + o, v);
        }
        let v = (rand4(fx, 0x01E0_5DEC) & 3) + 4;
        fx.mem.set8(r + 0xB, v);
        fx.mem.write_u32(r + 0x1C, 0x4248_0000);
        fx.mem.write_u32(r + 0x20, 0);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x3F4C_CCCD);
        for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x27, 0x60)] {
            fx.mem.write_u8(r + o, v);
        }
        node_pos(fx, 0x14, r + 0x28, 0x01E0_5E4C);
    }
    node_pos(fx, 0xC, sp + 0xB0, 0x01E0_5E70);
    for _ in 0..4 {
        bolt(fx, sp, 6, sp + 0x90, 0x3C, (0x8C, 0, 1, 9, 0x3F33_3333, 0x3E4C_CCCD, 0x20));
    }
}

/// Updates 222..361 [F 0x1E06014]: twice a flash and a ribbon at a point
/// about the target (sp + 0xA0, +-14).
fn crackle(fx: &mut Fx, sp: u32) {
    for _ in 0..2 {
        for (k, ra) in [0x01E0_6028u32, 0x01E0_6074, 0x01E0_60C0].iter().enumerate() {
            let j = jit(fx, *ra, 0xAF0, 0x578);
            let v = ee::add(fx.mem.f(sp + 0x90 + 4 * k as u32), j);
            fx.mem.setf(sp + 0xA0 + 4 * k as u32, v);
        }
        if let Some(r) = flash(fx, 0xC, 0, [0, 6, 0x60, 0x60, 0], 0xC0, [0x42C8_0000, 0x4190_0000], false) {
            copy_v4(fx, sp + 0xA0, r + 0x1C);
            fx.mem.write_u32(r + 0x18, 0x40A0_0000);
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
        for (o, v) in [(0x27u32, 0x70u8), (0x23, 0), (5, 0), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0x28)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = rand4(fx, 0x01E0_6270);
        fx.mem.set8(r + 9, x);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0xA0, r + 0x28);
    }
}

/// Updates 20..149 [F 0x1E06304]: a sprite falling onto each of nodes
/// 0x26 / 0x24 / 0x22 (sp + 0xB0) over its count (20 +- 6, 0x1A2B80).
fn gather(fx: &mut Fx, sp: u32) {
    for s2 in 0..3u32 {
        let i = first_free(fx, SPRITES, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        node_pos(fx, 0xC + 4 * s2, sp + 0xB0, 0x01E0_634C);
        let f20 = ee::add(20.0, jit200(fx, 0x01E0_6354, 6, 0x258));
        fx.fix_residue(f20, 0x01E0_63BC);
        let life = ee::fixsfsi(f20);
        let r = SPRITES + 0x44 * i as u32;
        set_counter4(fx, r, life & 0xFF, 0, 0xFF);
        for (o, v) in [(4u32, 0x41u8), (8, 0), (0xB, 2)] {
            fx.mem.write_u8(r + o, v);
        }
        let v = rand4(fx, 0x01E0_6404) & 7;
        fx.mem.set8(r + 9, v);
        fx.mem.write_u8(r + 0xA, 0);
        let c = (rand4(fx, 0x01E0_6420) & 0x3F) + 0x60;
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.set8(r + o, c);
        }
        fx.mem.write_u32(r + 0x10, 0x41A0_0000);
        fx.mem.write_u32(r + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(r + 0x1C, 0);
        fx.mem.write_u32(r + 0x18, 0);
        for (k, ra) in [0x01E0_6464u32, 0x01E0_64C4, 0x01E0_6524].iter().enumerate() {
            let f = jit200(fx, *ra, 3, 0x12C);
            fx.mem.setf(sp + 0xC0 + 4 * k as u32, f);
        }
        for k in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0xB0 + 4 * k), fx.mem.f(sp + 0xC0 + 4 * k));
            fx.mem.setf(r + 0x20 + 4 * k, v);
        }
        for k in 0..3u32 {
            let v = ee::neg(ee::div(fx.mem.f(sp + 0xC0 + 4 * k), f20));
            fx.mem.setf(r + 0x2C + 4 * k, v);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(r + o, 0);
        }
    }
}

/// Odd updates 89..345 [F 0x1E06628]: a ribbon on node slot D+0x20
/// (thicker from update 150).
fn node_ribbon(fx: &mut Fx) {
    let i = first_free(fx, RIBBONS, 0x68, 0x3C);
    if i < 0 {
        return;
    }
    let r = RIBBONS + 0x68 * i as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0x85);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0x20);
    fx.mem.write_u8(r + 6, b);
    for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 3)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = rand4(fx, 0x01E0_66B0);
    fx.mem.set8(r + 9, x);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    fx.mem.write_u8(r + 0x23, 0);
    fx.mem.write_u8(r + 0x27, 0x30);
    if fx.mem.u16(D) >= 0x96 {
        fx.mem.write_u8(r + 8, 0x19);
        fx.mem.write_u32(r + 0x14, 0x400C_CCCD);
        fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
    } else {
        fx.mem.write_u8(r + 8, 0x10);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        fx.mem.write_u32(r + 0x1C, 0x41A0_0000);
    }
    for (o, ra) in [(0x58u32, 0x01E0_6744u32), (0x5C, 0x01E0_675C), (0x60, 0x01E0_6774)] {
        let v = rand4(fx, ra) & 0x7F;
        fx.mem.set32(r + o, v);
    }
}

/// Updates 161..221 [F 0x1E067A0]: a bolt segment and a flash at a point
/// about node 0x22 (sp + 0xC0, +-2.5).
fn sparks(fx: &mut Fx, sp: u32) -> u64 {
    let mut s6 = u64::from(sp + 0xC8);
    node_pos(fx, 0x14, sp + 0xB0, 0x01E0_67B4);
    for (k, ra) in [0x01E0_67BCu32, 0x01E0_680C, 0x01E0_685C].iter().enumerate() {
        let j = jit(fx, *ra, 0x1F4, 0xFA);
        let v = ee::add(fx.mem.f(sp + 0xB0 + 4 * k as u32), j);
        fx.mem.setf(sp + 0xC0 + 4 * k as u32, v);
    }
    let i = first_free(fx, BOLTS, 0x40, 0x64);
    if i >= 0 {
        let r = BOLTS + ((i as u32) << 6);
        s6 = u64::from(r + 0x30);
        set_counter4(fx, r, 0xE, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 4);
        for k in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xC0 + 4 * k);
            fx.mem.write_u32(r + 0x28 + 4 * k, w);
            fx.mem.write_u32(r + 0x1C + 4 * k, w);
        }
        for (k, ra) in [0x01E0_692Cu32, 0x01E0_6978, 0x01E0_69C4].iter().enumerate() {
            let j = jit(fx, *ra, 0x2BC, 0x15E);
            let a = r + 0x28 + 4 * k as u32;
            let v = ee::add(fx.mem.f(a), j);
            fx.mem.setf(a, v);
        }
        fx.mem.write_u8(r + 8, 5);
        fx.mem.write_u32(r + 0x10, 0x3F80_0000);
        fx.mem.write_u32(r + 0x14, 0x3E99_999A);
        for (o, v) in [(0xBu32, 1u8), (0xD, 3), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x30)] {
            fx.mem.write_u8(r + o, v);
        }
    }
    if let Some(r) = flash(fx, 0xC, 0, [5, 9, 0, 0, 0x80], 0xD0, [0x42C8_0000, 0], false) {
        copy_v4(fx, sp + 0xC0, r + 0x1C);
    }
    s6
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x90, u64::from(D), 0x01E0_5744);
    match fx.mem.u16(D) {
        0x17C => return 8,
        0xDE => beads_222(fx, sp),
        0xDC => flash_220(fx, sp),
        0x96 => bolts_150(fx, sp),
        0x59 => streaks_on(fx),
        2 => opening(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0xDE..0x16A).contains(&n) {
        crackle(fx, sp);
    }
    let n = fx.mem.u16(D);
    if (0x14..0x96).contains(&n) {
        gather(fx, sp);
    }
    let n = fx.mem.u16(D);
    if (0x59..0x15A).contains(&n) && n & 1 != 0 {
        node_ribbon(fx);
    }
    let n = fx.mem.u16(D);
    // The entry's s6: the runner's (0) unless the sparks ran (sp + 0xC8,
    // then the bolt segment's +0x30).
    let mut s6 = 0u64;
    if (0xA1..0xDE).contains(&n) {
        s6 = sparks(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_6B1C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6B2C, [Some(u64::from(D)), Some(u64::from(OT))]);
    object_ops(fx, OPS, n);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves s6 (+0x70) and ra (+0x80): the streaks' first
    // TEXFLUSH a0 / a1 (vertex 1's / 2's stale XYZ low words) [run].
    fx.saved(sp - 0xF0, &[(0x70, s6), (0x78, 0), (0x80, 0x01E0_6C08), (0x88, 0)]);
    blib::radiate::streaks(fx, 0x01E0_0410, STREAKS, OT, 1);
    blib::star::flash(fx, 0x01E0_1230, FLASH, OT, 0x18);
    blib::streak::ribbon(fx, 0x01E0_1DE0, RIBBONS, OT, 0x3C);
    segment_spawns_saving(fx, 0x01E0_34C0, SPAWNS, 8, 0x01E0_6C58);
    blib::bolt::bolts(fx, 0x01E0_2970, BOLTS, OT, 0x64);
    blib::beam::beads(fx, 0x01E0_3840, BEADS, OT, 0x14);
    blib::sprite::bounce(fx, SPRITES, OT, 0xC8);
    let n = fx.mem.u16(D);
    if (0xDE..0x167).contains(&n) {
        6
    } else if n == 0x167 {
        2
    } else if n == 0xD4 {
        1
    } else {
        0
    }
}
