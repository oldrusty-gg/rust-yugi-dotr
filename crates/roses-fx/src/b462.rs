//! BMODULE b462 (card 537), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04D60, data area a0 = 0x1ED0000) [F] (helper C26,
//! 3 Nov 2026): b469's family (s0 = D, s1 = the OT). On update 20 a long
//! camera shake on node 0x4C (+0x40), two domes on it (+0x4394, the second
//! turned 0x5000 about z) and a segment spawner (+0x2CC4) feeding the bolt
//! pool; while the first dome counts, the attacker's unit flag 2 (0x1F69C0)
//! and both domes' radius 0.1 + 0.05 k + the distance between nodes 0x4C
//! / 0x4A, their alpha + 1 a update below update 90, then 0x30; on 88 a
//! flash on node 0x4C (+0x199C) and the domes' flag 8 (mirrored); on 90 a
//! shake at the target, eight bead beams from node 0x4C (+0x3CB4, staggered)
//! and four jagged bolts (six segments each, `bolt::bolts` +0x1DC4, counts
//! 0xAA - delay) node -> target, and a long flash on node 0x4C (+0x1904);
//! on 144 two segment spawners at the target; on updates 144..280 two
//! flashes and two ribbons a update at random points about the target; the
//! stage light pulsed by 0x1031C0 (+0xA0: down on update 2, up on 302). The
//! flicker pool (+0x2EA4) is drawn but never filled. Returns 1 on update
//! 134, 6 on 144..280, 2 on 281, 8 on 308 (and at once for a parameter of
//! -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `ribbon` 0x1E00FC0, `bolts` 0x1E01B50, `segment_spawns`
//! 0x1E026A0, `flickers` 0x1E02A20, `beads` 0x1E03500, `domes_550`
//! 0x1E042D0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b369::unit_flag;
use crate::b469::{bolt_bytes, charge2, flash_bytes, jag_at};
use crate::b586::segment_spawns_saving;
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x44D4;
const FLASHES: u32 = D + 0x1904;
const BOLTS: u32 = D + 0x1DC4;
const RIBBONS: u32 = D + 0xA4;
const DOMES: u32 = D + 0x4394;
const SPAWNERS: u32 = D + 0x2CC4;

/// The entry 0x1E04D60 (a0 = D, a1 = param; s0 = D, s1 = the OT).
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
    for k in 0..3 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x4C, 1), (0x10, 0x4A, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x12E);
    // The entry's s0 = D, s1 = the OT, s2 = 3 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4E7C, [Some(D), Some(OT), Some(3)]);
    0
}

/// Update 20 [F 0x1E04EEC]: the long shake on node +0xC, two domes there
/// and a segment spawner (its slot unchecked).
fn domes_start(fx: &mut Fx) {
    let s = D + 0x40;
    set_counter4(fx, s, 0x8C, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, b);
    fx.mem.write_u8(s + 4, 2);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x4080_0000);
    for k in 0..2u32 {
        let r = DOMES + 0x50 * k;
        set_counter4(fx, r, 0xF0, k as i32, 0xFF);
        fx.mem.write_u8(r + 4, 0x80);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 7, 6);
        fx.mem.write_u8(r + 8, 2);
        fx.mem.write_u8(r + 9, 2);
        fx.mem.write_u32(r + 0xC, 0x4000_0000);
        fx.mem.write_u32(r + 0x10, 0);
        for o in [0x1Cu32, 0x1D, 0x1E] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x1F, 0);
        if k != 0 {
            fx.mem.set32(r + 0x30, 0x5000);
        }
    }
    let i = first_free(fx, SPAWNERS, 0x3C, 8);
    let r = SPAWNERS.wrapping_add((i.wrapping_mul(0x3C)) as u32);
    set_counter4(fx, r, 0x40, 4, 0);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    fx.mem.write_u8(r + 4, 0x82);
    fx.mem.write_u32(r + 8, BOLTS);
    for (o, v) in [(0xCu32, 0x28u8), (0xD, 8), (0xE, 1), (0xF, 0), (0x10, 5), (0x11, 0xC)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.set32(r + 0x14, 0x320);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    fx.mem.write_u32(r + 0x1C, 0x3E99_999A);
    for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x38u32, 0x34, 0x30] {
        fx.mem.write_u32(r + o, 0);
    }
}

/// Update 90 [F 0x1E0517C]: the shake at the target (sp + 0x60), eight
/// bead beams on node +0xC, four jagged bolts from its position (sp +
/// 0x70) to the target and a long flash on node +0xC.
fn bolts_start(fx: &mut Fx, sp: u32) {
    let s = D + 0x40;
    set_counter4(fx, s, 0xC8, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4320_0000);
    copy_v4(fx, sp + 0x60, D + 0x60);
    fx.mem.write_u32(s + 0x50, 0x4100_0000);
    fx.mem.write_u32(D + 0x68, 0);
    for k in 0..8i32 {
        let r = D + 0x3CB4 + 0x58 * k as u32;
        set_counter4(fx, r, 0x1C, k & 0xFF, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(4);
        fx.mem.write_u8(r, c);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x19);
        fx.mem.set8(r + 9, k);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xC, 2);
        let x = fx.rand_from(D, 0x01E0_5254) >> 4;
        fx.mem.set8(r + 0xB, x & 3);
        fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
        fx.mem.write_u32(r + 0x20, 0xBF19_999A);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0xC);
        node_position_saving(fx, h, n, r + 0x28, u64::from(D), Some(u64::from(OT)), 0x01E0_52B0);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x70, u64::from(D), Some(u64::from(OT)), 0x01E0_52D8);
    for j in 0..4u32 {
        jag_at(fx, sp, sp + 0x70, 0x3F00_0000);
        for s in 0..6u32 {
            let i = first_free(fx, BOLTS, 0x40, 0x3C);
            if i < 0 {
                break;
            }
            let b = BOLTS + ((i as u32) << 6);
            let a = j + 9 * s;
            set_counter4(fx, b, (0xAA - a as i32) & 0xFF, (a & 0xFF) as i32, 0xFF);
            bolt_bytes(fx, sp, b, s, 9, [0x3F19_999A, 0x3E99_999A], 4);
        }
    }
    let r = FLASHES;
    set_counter4(fx, r, 0xA8, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    flash_bytes(fx, r, 0x85, 0x40, 0x20, 0x80, 0x80);
    fx.mem.write_u8(r + 0xF, 0xC0);
    fx.mem.write_u32(r + 0x14, 0);
    copy_v4(fx, sp + 0x60, r + 0x1C);
    fx.mem.write_u32(r + 0x10, 0x43C8_0000);
}

/// Update 144 [F 0x1E05500]: two segment spawners at the target.
fn spawners(fx: &mut Fx, sp: u32) {
    for k in 0..2u32 {
        let r = SPAWNERS + 0x3C * k;
        set_counter4(fx, r, 0x80, 0, 3);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u32(r + 8, BOLTS);
        for (o, v) in [(0xCu32, 0x28u8), (0xD, 0xC), (0xE, 1), (0xF, 0), (0x10, 5), (0x11, 0xC)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.set32(r + 0x14, 0x1770);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        fx.mem.write_u32(r + 0x1C, 0x3ECC_CCCD);
        for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(r + 0x24 + 4 * c, w);
        }
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(r + o, 0);
        }
    }
}

/// While the first dome counts [F 0x1E055F0]: the unit flag, both domes'
/// radius from the nodes' distance (sp + 0x90 = node +0xC - node +0x10),
/// their alpha.
fn domes_grow(fx: &mut Fx, sp: u32) {
    unit_flag(fx, 2, u64::from(D), 0x01E0_5608);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x70, u64::from(D), Some(u64::from(OT)), 0x01E0_561C);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x10);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(D), Some(u64::from(OT)), 0x01E0_5634);
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(sp + 0x70 + 4 * c), fx.mem.f(sp + 0x80 + 4 * c));
        fx.mem.setf(sp + 0x90 + 4 * c, v);
    }
    for k in 0..2u32 {
        let len = blib::beam::length3(fx, sp + 0x90);
        // adda.s / madd.s: (0.1 + len) + 0.05 k.
        let acc = ee::add(f32::from_bits(0x3DCC_CCCD), len);
        let r = ee::add(acc, ee::mul(f32::from_bits(0x3D4C_CCCD), ee::from_i32(k as i32)));
        let g = DOMES + 0x50 * k;
        fx.mem.setf(g + 0x14, r);
        if fx.mem.u16(D) < 0x5A {
            let a = fx.mem.read_u8(g + 0x1F).wrapping_add(1);
            fx.mem.write_u8(g + 0x1F, a);
        } else {
            fx.mem.write_u8(g + 0x1F, 0x30);
        }
    }
}

/// Updates 144..280 [F 0x1E05730]: twice a update, a point about the
/// target (x, y +- 14 at random, sp + 0x90; its w the stale sp + 0x9C)
/// gets a flash and a short wide ribbon.
fn sparkle(fx: &mut Fx, sp: u32) {
    for _ in 0..2 {
        for (c, ra) in [(0u32, 0x01E0_5738u32), (4, 0x01E0_5784)] {
            let x = fx.rand_from(D, ra) >> 4;
            let (_, m) = fx.div(x, 0xAF0);
            let j = ee::div(ee::from_i32(m - 0x578), 100.0);
            let v = ee::add(fx.mem.f(sp + 0x60 + c), j);
            fx.mem.setf(sp + 0x90 + c, v);
        }
        let w = fx.mem.read_u32(sp + 0x68);
        fx.mem.write_u32(sp + 0x98, w);
        let i = first_free(fx, FLASHES, 0x4C, 0x10);
        if i >= 0 {
            let r = FLASHES + 0x4C * i as u32;
            set_counter4(fx, r, 0xC, 0, 0xFF);
            fx.mem.write_u8(r + 5, 0);
            flash_bytes(fx, r, 1, 0x60, 0x10, 0x40, 0xC0);
            fx.mem.write_u32(r + 0x14, 0);
            fx.mem.write_u32(r + 0x10, 0x4396_0000);
            copy_v4(fx, sp + 0x90, r + 0x1C);
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
        for (o, v) in [(0x27u32, 0xC0u8), (0x23, 0), (5, 0), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0x28)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(D, 0x01E0_5938);
        fx.mem.set8(r + 9, x >> 4);
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
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_4EA4);
    match fx.mem.u16(D) {
        0x134 => return 8,
        0x90 => spawners(fx, sp),
        0x5A => bolts_start(fx, sp),
        0x58 => {
            charge2(fx);
            for k in 0..2u32 {
                let g = DOMES + 0x50 * k;
                let b = fx.mem.read_u8(g + 0xA) | 8;
                fx.mem.write_u8(g + 0xA, b);
            }
        }
        0x14 => domes_start(fx),
        _ => {}
    }
    if fx.mem.read_u8(DOMES) != 0 {
        domes_grow(fx, sp);
    }
    let n = fx.mem.u16(D);
    if (0x90..0x119).contains(&n) {
        sparkle(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_59BC, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_59CC, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::star::flash(fx, 0x01E0_0410, FLASHES, OT, 0x10);
    blib::streak::ribbon(fx, 0x01E0_0FC0, RIBBONS, OT, 0x3C);
    segment_spawns_saving(fx, 0x01E0_26A0, SPAWNERS, 8, 0x01E0_5A08);
    blib::bolt::bolts(fx, 0x01E0_1B50, BOLTS, OT, 0x28);
    blib::flicker::flickers(fx, 0x01E0_2A20, D + 0x2EA4, OT, 0x3C);
    blib::beam::beads(fx, 0x01E0_3500, D + 0x3CB4, OT, 0x14);
    blib::dome::domes_550(fx, 0x01E0_42D0, DOMES, OT, 4);
    let n = fx.mem.u16(D);
    if (0x90..0x119).contains(&n) {
        6
    } else if n == 0x119 {
        2
    } else if n == 0x86 {
        1
    } else {
        0
    }
}
