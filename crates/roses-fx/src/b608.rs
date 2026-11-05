//! BMODULE b608 (card 432), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E059B0, data area a0 = 0x1ED0000) [F] (helper B6, 31
//! Oct 2026): the "a0 = D" family (s0 = D, s1 = the OT). On update 8 four
//! long flashes (+0x53F0 + 0x4C k, 100 + 50 k wide) spread between the
//! attacker's nodes 0xE and 0xC and a long camera shake on node 0xE; on 25
//! an arc pair (+0xE64); on updates 29, 59, 89, 121 and 149 an arch
//! (+0x5AAC + 0x54 k, `arch::arches`) thrown from node 0xC towards the
//! target (+-4 jitter in x / y but on 149), aimed by 0x101940. Every
//! update each moving arch leaves a copy (count 5, still, +0x18 -0.6) in
//! the pool behind it (+0x5C50, 30 records); on its last count three
//! sheets (+0x5520, `sheet::sheets`), a flash (+0x5274), a ribbon (+0x58A4)
//! and 100 bouncing sprites (+0x1004) burst where it is, with the shake
//! there. The stage light pulsed by 0x1031C0 (+0xA0). Returns 1 on update
//! 56, 2 on 63, 6 on 66..200, 8 on 220 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `arcs`
//! 0x1E00410, `flash` 0x1E01690, `ribbon` 0x1E02240, `fade` 0x1E02DD0,
//! `beads` 0x1E03310, `arches` 0x1E040E0, `sheets` 0x1E049D0, `bounce`
//! 0x1E04F40 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{aim_angles, first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const ARCHES: u32 = D + 0x5AAC;
const FLASHES: u32 = D + 0x53F0;
const BYTES: u32 = D + 0x662C;

/// The entry 0x1E059B0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x6730);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x3E, 0), (0x14, 0x45, 0), (0x18, 0xE, 0), (0x1C, 0xC, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xD6);
    // The entry's s0 = D, s1 = the OT, s2 = 4 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5B1C, [Some(D), Some(OT), Some(4)]);
    fx.mem.write_u32(D + 0x6628, 0);
    0
}

/// Update 8 [F 0x1E05BB4]: four flashes and the long shake on node +0x18.
fn charge(fx: &mut Fx) {
    for k in 0..4i32 {
        let r = FLASHES + 0x4C * k as u32;
        set_counter4(fx, r, 0xA0, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x85);
        fx.mem.write_u8(r + 7, 9);
        let w = ee::add(100.0, ee::mul(50.0, ee::from_i32(k)));
        fx.mem.setf(r + 0x10, w);
        fx.mem.write_u32(r + 0x14, 0);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0xC0), (0xA, 0x80), (9, 0x20), (8, 0x60)] {
            fx.mem.write_u8(r + o, v);
        }
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0xA0, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(s + 6, b);
    fx.mem.write_u8(s + 4, 4);
    fx.mem.write_u32(s + 0x14, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
}

/// Update 25 [F 0x1E05CD8]: the arc pair (+0xE64), points +0xEC4.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0xE64;
    set_counter4(fx, g, 0xFF, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0xEC4);
    fx.mem.write_u8(g + 0x30, 0x14);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    fx.mem.write_u8(g + 0x34, 0);
    fx.mem.write_u16(g + 0x36, 0);
    fx.mem.write_u32(g + 0x40, 0x4120_0000);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0xC0), (0x4A, 0xC0), (0x49, 0x10), (0x48, 0x80)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        fx.mem.set16(g + 0xC + 2 * a, -1);
        a += 1;
    }
}

/// ((rand >> 4) % 8 x 200 - 800) / 100 from return address `ra`.
fn jitter(fx: &mut Fx, ra: u32) -> f32 {
    let x = (fx.rand_from(D, ra) >> 4).wrapping_rem(8);
    ee::div(ee::from_i32(x * 200 - 0x320), 100.0)
}

/// Updates 29 / 59 / 89 / 121 / 149 [F 0x1E05DA4]: the next arch from
/// node +0x1C (sp + 0x80) towards the target (sp + 0x70 into sp + 0x90),
/// over 24 updates.
fn throw(fx: &mut Fx, sp: u32) {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x1C);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(D), Some(u64::from(OT)), 0x01E0_5DB8);
    let to = sp + 0x90;
    copy_v4(fx, sp + 0x70, to);
    if fx.mem.u16(D) != 0x95 {
        let v = ee::add(fx.mem.f(to), jitter(fx, 0x01E0_5DF8));
        fx.mem.setf(to, v);
        let v = ee::add(fx.mem.f(to + 4), jitter(fx, 0x01E0_5E5C));
        fx.mem.setf(to + 4, v);
    }
    let r = ARCHES + 0x54 * fx.mem.read_u32(D + 0x6628);
    set_counter4(fx, r, 0x18, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (0xE, 0xC0), (0xD, 0x20), (0xC, 0x80), (0xF, 0x20), (0xB, 0xC0), (8, 0xC0), (9, 0xC0), (0xA, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, 0x4120_0000);
    fx.mem.write_u32(r + 0x14, 0x4000_0000);
    fx.mem.write_u32(r + 0x18, 0);
    copy_v4(fx, sp + 0x80, r + 0x24);
    for c in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(to + 4 * c), fx.mem.f(r + 0x24 + 4 * c)), 24.0);
        fx.mem.setf(r + 0x34 + 4 * c, v);
    }
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(to + 4 * c), fx.mem.f(r + 0x24 + 4 * c));
        fx.mem.setf(sp + 0xA0 + 4 * c, v);
    }
    aim_angles(fx, sp + 0xA0, r + 0x44);
    let k = fx.mem.read_u32(D + 0x6628) + 1;
    fx.mem.write_u32(D + 0x6628, k);
}

/// 100 bouncing sprites (`sprite::bounce`, 0x44 bytes at +0x1004, 250
/// searched) thrown up from `at` [F 0x1E06534]: b245's `bounce_spawn`
/// with +8 = 0, delay rand & 7, +0x10 = 90 + (rand % 30 x 200 - 3000) /
/// 100, +0x14 / +0x18 = -0.4 / -0.06, spreads (rand % 210 - 105) / 100.
fn bounce_burst(fx: &mut Fx, at: u32) {
    let pool = D + 0x1004;
    let spread = |fx: &mut Fx, ra: u32| {
        let x = fx.rand_from(D, ra) >> 4;
        let (_, h) = fx.div(x, 0xD2);
        ee::div(ee::from_i32(h - 0x69), 100.0)
    };
    for _ in 0..100 {
        let i = first_free(fx, pool, 0x44, 0xFA);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(D, 0x01E0_655C) >> 4) & 7;
        let b = pool + 0x44 * i as u32;
        set_counter4(fx, b, 0x1C, delay, 0xFF);
        for (o, x) in [(4u32, 0u8), (8, 0), (0xB, 2), (0xA, 0), (9, 3), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(b + o, x);
        }
        let x = fx.rand_from(D, 0x01E0_65C4) >> 4;
        let (_, h) = fx.div(x, 0x1E);
        let f = ee::div(ee::from_i32(h * 200 - 0xBB8), 100.0);
        fx.mem.setf(b + 0x10, ee::add(90.0, f));
        fx.mem.write_u32(b + 0x14, 0xBECC_CCCD);
        fx.mem.write_u32(b + 0x18, 0xBD75_C28F);
        fx.mem.write_u32(b + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        let x = spread(fx, 0x01E0_6668);
        fx.mem.setf(b + 0x2C, x);
        let x = spread(fx, 0x01E0_66B4);
        fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), x));
        let x = spread(fx, 0x01E0_6710);
        fx.mem.setf(b + 0x34, x);
    }
}

/// Arch `k` (record `a`) on its last count [F 0x1E061E0]: three sheets, a
/// flash, a ribbon, the sprites and the shake at its position.
fn burst(fx: &mut Fx, k: u32, a: u32) {
    let at = a + 0x24;
    for j in 0..3i32 {
        let i = first_free(fx, D + 0x5520, 0x3C, 0xF);
        if i < 0 {
            continue;
        }
        let r = D + 0x5520 + 0x3C * i as u32;
        set_counter4(fx, r, 0x12, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 3), (0xA, 0), (9, 6), (0xB, 2), (0xE, 0x80), (0xD, 0x20), (0xC, 0x40), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        let y = ee::add(fx.mem.f(r + 0x30), ee::add(-1.0, ee::from_i32(j)));
        fx.mem.setf(r + 0x30, y);
        fx.mem.write_u32(r + 0x10, 0x4000_0000);
        fx.mem.write_u32(r + 0x14, 0x3F4C_CCCD);
        fx.mem.write_u32(r + 0x18, if k == 1 { 0x40C0_0000 } else { 0x4080_0000 });
        fx.mem.set32(r + 0x24, 0x4000);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let v = fx.mem.i32(r + 0x24).wrapping_neg();
            fx.mem.set32(r + 0x24, v);
        }
    }
    let i = first_free(fx, D + 0x5274, 0x4C, 5);
    if i >= 0 {
        let r = D + 0x5274 + 0x4C * i as u32;
        set_counter4(fx, r, 0x20, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x80);
        fx.mem.write_u8(r + 7, 9);
        fx.mem.write_u32(r + 0x10, 0x4248_0000);
        fx.mem.write_u32(r + 0x14, 0x4220_0000);
        for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x20), (8, 0x60), (0xF, 0xC0)] {
            fx.mem.write_u8(r + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(r + o, 0);
        }
        copy_v4(fx, at, r + 0x1C);
        fx.mem.write_u32(r + 0x18, 0x41A0_0000);
    }
    let i = first_free(fx, D + 0x58A4, 0x68, 5);
    if i >= 0 {
        let r = D + 0x58A4 + 0x68 * i as u32;
        set_counter4(fx, r, 0x1C, 0, 0xFF);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u32(r + 0x14, 0x4000_0000);
        fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
        for (o, v) in [(5u32, 0u8), (0xA, 2), (0xB, 2), (0xC, 3), (8, 0x64)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(D, 0x01E0_64AC);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x10), (0x27, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x28);
        fx.mem.write_u32(r + 0x58, 0x40);
        fx.mem.write_u32(r + 0x5C, 0x80);
        fx.mem.write_u32(r + 0x60, 0x100);
    }
    bounce_burst(fx, at);
    let s = D + 0x40;
    set_counter4(fx, s, 0x1C, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x70, u64::from(D), 0x01E0_5B48);
    match fx.mem.u16(D) {
        0xDC => return 8,
        0x95 | 0x79 | 0x59 | 0x3B | 0x1D => throw(fx, sp),
        0x19 => arcs_start(fx),
        8 => charge(fx),
        _ => {}
    }
    if fx.mem.read_u8(FLASHES) != 0 {
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0x18);
        node_position_saving(fx, h, n, sp + 0x80, u64::from(D), Some(u64::from(OT)), 0x01E0_6034);
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0x1C);
        node_position_saving(fx, h, n, sp + 0x90, u64::from(D), Some(u64::from(OT)), 0x01E0_604C);
        let d: [f32; 3] = std::array::from_fn(|c| ee::div(ee::sub(fx.mem.f(sp + 0x80 + 4 * c as u32), fx.mem.f(sp + 0x90 + 4 * c as u32)), 4.0));
        for k in 0..4i32 {
            let f = ee::from_i32(k);
            let r = FLASHES + 0x4C * k as u32;
            for c in 0..3u32 {
                let v = ee::add(fx.mem.f(sp + 0x90 + 4 * c), ee::mul(d[c as usize], f));
                fx.mem.setf(r + 0x1C + 4 * c, v);
            }
        }
    }
    for k in 0..5u32 {
        let a = ARCHES + 0x54 * k;
        if fx.mem.read_u8(a) == 0 {
            continue;
        }
        let i = first_free(fx, D + 0x5C50, 0x54, 0x1E);
        if i >= 0 {
            let r = D + 0x5C50 + 0x54 * i as u32;
            for w in 0..0x15u32 {
                let v = fx.mem.read_u32(a + 4 * w);
                fx.mem.write_u32(r + 4 * w, v);
            }
            fx.mem.write_u8(r, 5);
            for o in [0x3Cu32, 0x38, 0x34] {
                fx.mem.write_u32(r + o, 0);
            }
            fx.mem.write_u32(r + 0x18, 0xBF19_999A);
        }
        if fx.mem.read_u8(a) == 1 {
            burst(fx, k, a);
        }
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_6800, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6810, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0x526C, OT);
    blib::arc::arcs(fx, 0x01E0_0410, D + 0xE64, OT, 1);
    blib::streak::ribbon(fx, 0x01E0_2240, D + 0x58A4, OT, 1);
    blib::star::flash(fx, 0x01E0_1690, D + 0x5274, OT, 5);
    blib::star::flash(fx, 0x01E0_1690, FLASHES, OT, 4);
    blib::beam::beads(fx, 0x01E0_3310, D + 0xA4, OT, 0x28);
    blib::arch::arches(fx, 0x01E0_40E0, ARCHES, OT, 0x19);
    blib::sheet::sheets(fx, 0x01E0_49D0, D + 0x5520, OT, 0xF);
    blib::sprite::bounce(fx, D + 0x1004, OT, 0xFA);
    let n = fx.mem.u16(D);
    if (0x42..0xC9).contains(&n) {
        6
    } else if n == 0x3F {
        2
    } else if n == 0x38 {
        1
    } else {
        0
    }
}
