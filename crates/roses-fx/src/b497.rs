//! BMODULE b497 (card 347), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05220, data area a0 = 0x1ED0000) [F] (helper B5, 31
//! Oct 2026): b469's family with a fade (+0x4394, `fade::fade`) and four
//! crossing bars (+0x439C, `cross::bars`); random bytes at +0x44DC. On
//! update 10 the bars, a flash (+0x199C) on the attacker's node 7 and a
//! camera shake at the target (+0x40); on 30 a long flash there (+0x1904)
//! and a segment spawner (+0x2CC4, slot unchecked); on 99 the fade, the
//! shake again, eight bead beams from node 7 (+0x3CB4), a flash and four
//! jagged bolts node -> target; on 76 the flashes widen; on 100 two
//! segment spawners at the target; on even updates 30..209 a ribbon from
//! node 7 (+0xA4); on updates 100..236 two flashes (sized at random) and
//! ribbons a update about the target (x, y +- 14, z +- 6); the stage light
//! pulsed by 0x1031C0 (+0xA0). The flicker pool (+0x2EA4) is drawn but
//! never filled. Returns 1 on update 90, 6 on 100..236, 2 on 237, 8 on 250
//! (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `ribbon` 0x1E00FC0, `bolts` 0x1E01B50, `segment_spawns`
//! 0x1E026A0, `flickers` 0x1E02A20, `beads` 0x1E03500, `fade` 0x1E042D0,
//! `bars` 0x1E04810 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b469::{flash_bytes, jag_from, ribbon_from, spark_ribbon};
use crate::b586::segment_spawns_saving;
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x44DC;
const FLASHES: u32 = D + 0x1904;
const BOLTS: u32 = D + 0x1DC4;
const SPAWNERS: u32 = D + 0x2CC4;
const BARS: u32 = D + 0x439C;

/// The entry 0x1E05220 (a0 = D, a1 = param; s0 = D, s1 = the OT).
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
    fx.bzero(D, 0x45E0);
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
    for (at, node, clear) in [(0xCu32, 7, 1), (0x10, 0xE, 0), (0x14, 0xF, 0), (0x18, 0x15, 0), (0x1C, 0x16, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xF4);
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5384, [Some(D), Some(OT), Some(2)]);
    0
}

/// The shake at the target (sp + 0x60) [F 0x1E0559C / 0x1E057C4]: mode
/// `mode`, its +0x50 = `w`.
pub(crate) fn shake(fx: &mut Fx, sp: u32, mode: u8, w: u32) {
    let s = D + 0x40;
    set_counter4(fx, s, 0x8C, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, mode);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3FB3_3333);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4320_0000);
    copy_v4(fx, sp + 0x60, D + 0x60);
    fx.mem.write_u32(s + 0x50, w);
    fx.mem.write_u32(D + 0x68, 0);
}

/// Update 10 [F 0x1E0540C]: four bars on node +0xC (the first 1200 long,
/// the others 600 at 45 k degrees), a flash there and the shake.
fn bars_start(fx: &mut Fx, sp: u32) {
    for k in 0..4u32 {
        let r = BARS + 0x50 * k;
        set_counter4(fx, r, 0x1C, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 4, 0x80);
        if k == 0 {
            fx.mem.write_u32(r + 0x10, 0x4496_0000);
            fx.mem.write_u32(r + 0x18, 0);
            fx.mem.write_u32(r + 0x14, 0x41A0_0000);
        } else {
            fx.mem.write_u32(r + 0x10, 0x4416_0000);
            let a = ee::mul(45.0, ee::from_i32(k as i32));
            fx.mem.setf(r + 0x18, a);
            fx.mem.write_u32(r + 0x14, 0x4140_0000);
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0x80), (9, 0x10), (8, 0x40)] {
            fx.mem.write_u8(r + o, v);
        }
    }
    let r = D + 0x199C;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x4396_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x10), (9, 0x20), (8, 0x80), (0xF, 0xD0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    shake(fx, sp, 4, 0x4080_0000);
}

/// The spawner slot `first_free` names, unchecked (a full pool gives the
/// record before the first) [F 0x1E056DC / 0x1E05BB0].
fn spawner(fx: &mut Fx) -> u32 {
    let i = first_free(fx, SPAWNERS, 0x3C, 8);
    SPAWNERS.wrapping_add(i.wrapping_mul(0x3C) as u32)
}

/// The spawner bytes from +8: the bolt pool, segment bytes, range +0x14
/// `range`, sizes, colour, no velocity.
pub(crate) fn spawner_bytes(fx: &mut Fx, r: u32, d: u8, range: i32) {
    fx.mem.write_u32(r + 8, BOLTS);
    for (o, v) in [(0xCu32, 0x28u8), (0xD, d), (0xE, 1), (0xF, 0), (0x10, 5), (0x11, 0xC)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.set32(r + 0x14, range);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    fx.mem.write_u32(r + 0x1C, 0x3ECC_CCCD);
    for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20)] {
        fx.mem.write_u8(r + o, v);
    }
}

/// Update 30 [F 0x1E05620]: a long flash on node +0xC and a spawner there
/// (+5 = 1).
fn charge(fx: &mut Fx) {
    let r = FLASHES;
    set_counter4(fx, r, 0xA0, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    flash_bytes(fx, r, 0x85, 0x40, 0x20, 0x80, 0xC0);
    fx.mem.write_u32(r + 0x10, 0x4316_0000);
    fx.mem.write_u32(r + 0x14, 0);
    let r = spawner(fx);
    set_counter4(fx, r, 0x40, 4, 0);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    fx.mem.write_u8(r + 4, 2);
    spawner_bytes(fx, r, 8, 0x44C);
    for o in [0x38u32, 0x34, 0x30] {
        fx.mem.write_u32(r + o, 0);
    }
}

/// Update 99 [F 0x1E057A4]: the fade, the shake, eight bead beams on node
/// +0xC (its position into sp + 0x80), a flash there and four jagged bolts
/// to the target.
fn bolts_start(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0x4394, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0x4398, 1);
    shake(fx, sp, 8, 0x4100_0000);
    for k in 0..8i32 {
        let r = D + 0x3CB4 + 0x58 * k as u32;
        set_counter4(fx, r, 0x20, k & 0xFF, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(4);
        fx.mem.write_u8(r, c);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x1E);
        fx.mem.set8(r + 9, k);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xC, 2);
        let x = fx.rand_from(D, 0x01E0_58B8) >> 4;
        fx.mem.set8(r + 0xB, x & 3);
        fx.mem.write_u32(r + 0x1C, 0x4220_0000);
        fx.mem.write_u32(r + 0x20, 0xBF19_999A);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0xC);
        node_position_saving(fx, h, n, r + 0x28, u64::from(D), Some(u64::from(OT)), 0x01E0_5914);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(D), Some(u64::from(OT)), 0x01E0_5938);
    let i = first_free(fx, FLASHES, 0x4C, 0x10);
    if i >= 0 {
        let r = FLASHES + 0x4C * i as u32;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        flash_bytes(fx, r, 0x80, 0x40, 0x20, 0x80, 0xC0);
        fx.mem.write_u32(r + 0x10, 0x42C8_0000);
        fx.mem.write_u32(r + 0x14, 0x4316_0000);
        fx.mem.write_u32(r + 0x18, 0x4270_0000);
        copy_v4(fx, sp + 0x80, r + 0x1C);
    }
    for j in 0..4u32 {
        jag_from(fx, sp, 0x3E99_999A);
        for s in 0..6u32 {
            let i = first_free(fx, BOLTS, 0x40, 0x3C);
            if i < 0 {
                break;
            }
            let b = BOLTS + ((i as u32) << 6);
            set_counter4(fx, b, 0x8C, (j & 1) as i32, 0xFF);
            bolt_still(fx, sp, b, s);
        }
    }
}

/// Segment `s` of the jagged path into bolt `b` as b469's `bolt_bytes`
/// with +4 = +5 = 0 [F 0x1E05AB4].
pub(crate) fn bolt_still(fx: &mut Fx, sp: u32, b: u32, s: u32) {
    fx.mem.write_u8(b + 5, 0);
    fx.mem.write_u8(b + 4, 0);
    for c in 0..6u32 {
        let v = fx.mem.read_u32(sp + 0xD0 + 12 * s + 4 * c);
        fx.mem.write_u32(b + 0x1C + 4 * c, v);
    }
    fx.mem.write_u8(b + 8, 9);
    fx.mem.write_u32(b + 0x10, 0x3F33_3333);
    fx.mem.write_u32(b + 0x14, 0x3E4C_CCCD);
    for (o, v) in [(0xBu32, 1u8), (0xD, 3), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)] {
        fx.mem.write_u8(b + o, v);
    }
}

/// Update 100 [F 0x1E05B98]: two segment spawners at the target.
fn strike(fx: &mut Fx, sp: u32) {
    for _ in 0..2 {
        let r = spawner(fx);
        set_counter4(fx, r, 0x80, 0, 3);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0);
        spawner_bytes(fx, r, 0xC, 0x1770);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(r + 0x24 + 4 * c, w);
        }
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(r + o, 0);
        }
    }
}

/// (rand >> 4) % `m` - `h`, over 100, from return address `ra`.
pub(crate) fn jitter(fx: &mut Fx, ra: u32, m: i32, h: i32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let (_, r) = fx.div(x, m);
    ee::div(ee::from_i32(r - h), 100.0)
}

/// Updates 100..236 [F 0x1E05E78]: twice a update, a point about the
/// target (x, y +- 14, z +- 6, sp + 0x70; its w the stale sp + 0x7C) gets
/// a flash 20 +- 10 high and a ribbon.
fn sparkle(fx: &mut Fx, sp: u32) {
    for _ in 0..2 {
        for (c, ra, m, h) in [(0u32, 0x01E0_5E80u32, 0xAF0, 0x578), (4, 0x01E0_5ECC, 0xAF0, 0x578), (8, 0x01E0_5F18, 0x4B0, 0x258)] {
            let j = jitter(fx, ra, m, h);
            let v = ee::add(fx.mem.f(sp + 0x60 + c), j);
            fx.mem.setf(sp + 0x70 + c, v);
        }
        let i = first_free(fx, FLASHES, 0x4C, 0x10);
        if i >= 0 {
            let r = FLASHES + 0x4C * i as u32;
            set_counter4(fx, r, 0xC, 0, 0xFF);
            fx.mem.write_u8(r + 5, 0);
            flash_bytes(fx, r, 0, 0x60, 0x10, 0x40, 0xC0);
            fx.mem.write_u32(r + 0x14, 0);
            fx.mem.write_u32(r + 0x10, 0x4396_0000);
            let x = fx.rand_from(D, 0x01E0_6004) >> 4;
            let (_, m) = fx.div(x, 10);
            let v = ee::add(20.0, ee::div(ee::from_i32(m * 200 - 0x3E8), 100.0));
            fx.mem.setf(r + 0x14, v);
            fx.mem.write_u32(r + 0x18, 0x4170_0000);
            copy_v4(fx, sp + 0x70, r + 0x1C);
        }
        spark_ribbon(fx, sp, 0x01E0_6138, BYTES);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_53AC);
    match fx.mem.u16(D) {
        0xFA => return 8,
        0x64 => strike(fx, sp),
        0x4C => {
            fx.mem.write_u32(D + 0x1960, 0x4348_0000);
            fx.mem.write_u32(D + 0x1914, 0x4348_0000);
        }
        0x63 => bolts_start(fx, sp),
        0x1E => charge(fx),
        0xA => bars_start(fx, sp),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x1E..0xD2).contains(&n) && n & 1 == 0 {
        let look = [(0x3F80_0000, 0x41A0_0000, 0xC), (0x4040_0000, 0x4220_0000, 0x14)];
        ribbon_from(fx, D + 0xC, 0x5A, look, [0x01E0_5DB0, 0x01E0_5E00, 0x01E0_5E18, 0x01E0_5E30], BYTES);
    }
    let n = fx.mem.u16(D);
    if (0x64..0xED).contains(&n) {
        sparkle(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_61BC, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_61CC, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0x4394, OT);
    blib::star::flash(fx, 0x01E0_0410, FLASHES, OT, 0x10);
    blib::streak::ribbon(fx, 0x01E0_0FC0, D + 0xA4, OT, 0x3C);
    segment_spawns_saving(fx, 0x01E0_26A0, SPAWNERS, 8, 0x01E0_6218);
    blib::bolt::bolts(fx, 0x01E0_1B50, BOLTS, OT, 0x28);
    blib::cross::bars(fx, 0x01E0_4810, BARS, OT, 4);
    blib::flicker::flickers(fx, 0x01E0_2A20, D + 0x2EA4, OT, 0x3C);
    blib::beam::beads(fx, 0x01E0_3500, D + 0x3CB4, OT, 0x14);
    let n = fx.mem.u16(D);
    if (0x64..0xED).contains(&n) {
        6
    } else if n == 0xED {
        2
    } else if n == 0x5A {
        1
    } else {
        0
    }
}
