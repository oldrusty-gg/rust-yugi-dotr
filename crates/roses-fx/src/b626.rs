//! BMODULE b626 (card 221), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06500, data area a0 = 0x1ED0000) [F] (helper B10,
//! 31 Oct 2026): s3 = D, s0 = the OT (0x206E40), s1 the parameter then the
//! spawn loops' counters. Update 82 an arc (+0xC8) from node 0x64 to 0x60;
//! 90 sixty bouncing sprites (+0xFE8, 0x44 apart) thrown from node 0x56
//! round the attacker's facing; 93 the attacker's echoes (+0xA4,
//! `radiate::echoes`); 94 bead trails (+0x2408) from node 0x64; 98 an arch
//! (+0x768) from the attacker's point to the target, copied to a free slot
//! (0x54 apart) on every update 98..145; 128 at the target three sheets
//! (+0x1FD8, 0x3C apart), a flash (+0xF9C), a ribbon (+0x23A0), eight
//! beads (+0x2484, 0x58 apart), the fade (+0x2398) and a camera shake
//! (+0x40). The stage light pulsed by 0x1031C0 (+0xA0). Returns 6 on
//! updates 128..148, 2 on 149, 1 on 118, 8 on 175 (no drawing then) and
//! at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `arcs`
//! 0x1E00410, `arches` 0x1E01690, `echoes` 0x1E01F80, `bounce`
//! 0x1E026C0, `flash` 0x1E03130, `sheets` 0x1E03CE0, `fade` 0x1E04250,
//! `ribbon` 0x1E04790, `beads` 0x1E05320, `bead_trails_b` 0x1E060F0
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PI: u32 = 0x4049_0FDB;

/// The entry 0x1E06500 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0x3350).
    fx.bzero(D, 0x3350);
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
    for (at, node, clear) in [(0xCu32, 0x64, 1), (0x10, 0x60, 0), (0x14, 0x56, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xA9);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's), s2 the runner's.
    rand_bytes(fx, D + 0x3244, 0x100, 0x01E0_663C, [Some(OT), Some(3), None]);
    0
}

/// rand >> 4 & `mask` with the compiler's signed-remainder fix-up (`m`
/// = mask + 1).
fn rand_rem(fx: &mut Fx, ra: u32, mask: i32) -> i32 {
    let v = fx.rand_from(OT, ra) >> 4;
    let mut r = v & mask;
    if v < 0 && r != 0 {
        r -= mask + 1;
    }
    r
}

/// Update 90 [F 0x1E067EC]: sixty bouncing sprites from node 0x56.
fn sprites(fx: &mut Fx, sp: u32) {
    for s1 in 0..0x3Ci32 {
        let i = first_free(fx, D + 0xFE8, 0x44, 0x3C);
        if i < 0 {
            continue;
        }
        let r = D + 0x44 * i as u32;
        set_counter4(fx, r + 0xFE8, 0x28, (s1 >> 1) & 0xFF, 0xFF);
        let v = fx.rand_from(OT, 0x01E0_6848) >> 4;
        fx.mem.set8(r + 0xFEC, (v & 3) << 4);
        fx.mem.write_u8(r + 0xFF0, 1);
        fx.mem.write_u8(r + 0xFF3, 1);
        fx.mem.write_u8(r + 0xFF2, 5);
        let v = fx.rand_from(OT, 0x01E0_6878) >> 4;
        fx.mem.set8(r + 0xFF1, v & 3);
        for k in [0xFF4u32, 0xFF5, 0xFF6, 0xFF7] {
            fx.mem.write_u8(r + k, 0x80);
        }
        let v = rand_rem(fx, 0x01E0_68A4, 1);
        let f = ee::add(6.0, ee::div(ee::from_i32(200 * v - 200), 100.0));
        fx.mem.setf(r + 0xFF8, f);
        let v = rand_rem(fx, 0x01E0_6914, 3);
        let f = ee::add(13.0, ee::div(ee::from_i32(200 * v - 400), 100.0));
        fx.mem.setf(r + 0xFFC, f);
        fx.mem.write_u32(r + 0x1000, 0xBCA3_D70A);
        fx.mem.write_u32(r + 0x1004, 0);
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14));
        node_position_saving(fx, h, s, sp + 0x70, u64::from(OT), Some(s1 as u64), 0x01E0_69A0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
            fx.mem.write_u32(r + 0x1008 + 4 * c, w);
        }
        let mut f20 = if fx.mem.read_u32(D + 0x30) == 0x8000 { 180.0 } else { 0.0 };
        f20 = ee::add(f20, ee::mul(7.0, ee::from_i32(s1)));
        let v = fx.rand_from(OT, 0x01E0_6A04) >> 4;
        let (_, m) = fx.div(v, 0x28);
        let f21 = ee::add(f32::from_bits(0x3F33_3333), ee::div(ee::from_i32(m - 0x14), 100.0));
        let a = ee::div(ee::mul(f32::from_bits(PI), f20), 180.0);
        let c = libm_cosf(fx, a);
        fx.mem.setf(r + 0x1014, ee::mul(f21, c));
        let s = fx.libm_sinf(a);
        fx.mem.setf(r + 0x101C, ee::mul(f21, s));
        fx.mem.write_u32(r + 0x1018, 0x3E99_999A);
    }
}

/// Update 128 [F 0x1E06CE8]: the strike at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0x2398, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0x239C, 1);
    // Three sheets (+0x1FD8, 0x3C apart).
    for s1 in 0..3i32 {
        let r = D + 0x3C * s1 as u32;
        set_counter4(fx, r + 0x1FD8, 0x1C, 0, 0xFF);
        for (o, v) in [(0x1FDDu32, 0u8), (0x1FDC, 0), (0x1FE0, 2), (0x1FE2, 0), (0x1FE1, 6), (0x1FE3, 2), (0x1FE4, 0x80), (0x1FE5, 0x80), (0x1FE6, 0x80), (0x1FE7, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x60, r + 0x2004);
        let dy = ee::add(-1.5, ee::mul(1.5, ee::from_i32(s1)));
        let y = ee::add(fx.mem.f(r + 0x2008), dy);
        fx.mem.setf(r + 0x2008, y);
        fx.mem.write_u32(r + 0x1FE8, 0x4040_0000);
        fx.mem.write_u32(r + 0x1FEC, 0x3F80_0000);
        fx.mem.write_u32(r + 0x1FF0, if s1 == 1 { 0x40A0_0000 } else { 0x4080_0000 });
        fx.mem.set32(r + 0x1FFC, 0x4600);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let v = fx.mem.i32(r + 0x1FFC).wrapping_neg();
            fx.mem.set32(r + 0x1FFC, v);
        }
    }
    // The flash (+0xF9C).
    set_counter4(fx, D + 0xF9C, 0x1C, 0, 0xFF);
    for (o, v) in [(0xFA1u32, 0u8), (0xFA0, 0x80), (0xFA3, 9)] {
        fx.mem.write_u8(D + o, v);
    }
    fx.mem.write_u32(D + 0xFAC, 0x4348_0000);
    fx.mem.write_u32(D + 0xFB0, 0x42A0_0000);
    for (o, v) in [(0xFA8u32, 0xC0u8), (0xFA9, 0xC0), (0xFAA, 0xC0), (0xFA6, 0x80), (0xFA5, 0x40), (0xFA4, 0x20), (0xFAB, 0xD0)] {
        fx.mem.write_u8(D + o, v);
    }
    for o in [0xFE0u32, 0xFDC, 0xFD8] {
        fx.mem.write_u32(D + o, 0);
    }
    copy_v4(fx, sp + 0x60, D + 0xFB8);
    fx.mem.write_u32(D + 0xFB4, 0x41A0_0000);
    // The ribbon (+0x23A0).
    set_counter4(fx, D + 0x23A0, 0x18, 0, 0xFF);
    fx.mem.write_u8(D + 0x23A4, 5);
    fx.mem.write_u32(D + 0x23B4, 0x4033_3333);
    fx.mem.write_u32(D + 0x23BC, 0x4220_0000);
    for (o, v) in [(0x23A5u32, 0u8), (0x23AA, 0), (0x23AB, 2), (0x23AC, 3), (0x23A8, 0xA0)] {
        fx.mem.write_u8(D + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_6F40);
    fx.mem.set8(D + 0x23A9, x >> 4);
    fx.mem.write_u32(D + 0x23B0, D + 0x3244);
    fx.mem.write_u32(D + 0x23B8, 0x3F80_0000);
    for (o, v) in [(0x23C4u32, 0x80u8), (0x23C5, 0x80), (0x23C6, 0x80), (0x23C0, 0x80), (0x23C1, 0x80), (0x23C2, 0x80), (0x23C3, 0x20), (0x23C7, 0x80)] {
        fx.mem.write_u8(D + o, v);
    }
    copy_v4(fx, sp + 0x60, D + 0x23C8);
    fx.mem.write_u32(D + 0x23F8, 0x40);
    fx.mem.write_u32(D + 0x23FC, 0x80);
    fx.mem.write_u32(D + 0x2400, 0x100);
    // Eight beads (+0x2484, 0x58 apart), delayed 0..7.
    for s1 in 0..8i32 {
        let i = first_free(fx, D + 0x2484, 0x58, 0x28);
        if i < 0 {
            continue;
        }
        let v = fx.rand_from(OT, 0x01E0_6FEC) >> 4;
        let count = ((v & 3) + 0x1E) & 0xFF;
        let r = D + 0x58 * i as u32;
        set_counter4(fx, r + 0x2484, count, s1 & 0xFF, 0xFF);
        fx.mem.write_u8(r + 0x2488, 5);
        fx.mem.write_u8(r + 0x2489, 0);
        fx.mem.write_u8(r + 0x248C, 0x14);
        let x = fx.rand_from(OT, 0x01E0_7048);
        fx.mem.set8(r + 0x248D, x >> 4);
        for (o, v) in [(0x248Eu32, 0u8), (0x2490, 2), (0x248F, 3)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x24A0, 0x42A0_0000);
        fx.mem.write_u32(r + 0x24A4, 0xBF80_0000);
        fx.mem.write_u32(r + 0x2494, D + 0x3244);
        fx.mem.write_u32(r + 0x2498, 0x3F99_999A);
        for o in [0x24ABu32, 0x24A8, 0x24A9, 0x24AA] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0x60, r + 0x24AC);
    }
    // The camera shake (+0x40) at the target.
    set_counter4(fx, D + 0x40, 0x20, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x58, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x4080_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_713C);
}

/// Update 98 [F 0x1E06BAC]: the arch template (+0x768) from the
/// attacker's point toward the target over 48 counts.
fn arch(fx: &mut Fx, sp: u32, s1: u64) {
    let a = D + 0x768;
    set_counter4(fx, a, 0x30, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (0xF, 0x90), (0xC, 0x90), (0xD, 0x90), (0xE, 0x90), (0xA, 0xC0), (9, 0), (8, 0xC0), (0xB, 0x10)] {
        fx.mem.write_u8(a + o, v);
    }
    fx.mem.write_u32(a + 0x10, 0x4100_0000);
    fx.mem.write_u32(a + 0x14, 0x4000_0000);
    fx.mem.write_u32(a + 0x18, 0x3F4C_CCCD);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, a + 0x24);
    // 0x1F65D0's 0x50 frame (s0 = OT, s1, s2 (the runner's 0), s3 = D,
    // ra): its zero upper halves are the arches' first two Q words [run].
    fx.saved(sp - 0x50, &[(0, u64::from(OT)), (0x10, s1), (0x20, 0), (0x30, u64::from(D)), (0x40, 0x01E0_6C30)]);
    for c in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x60 + 4 * c), fx.mem.f(a + 0x24 + 4 * c)), 48.0);
        fx.mem.setf(a + 0x34 + 4 * c, v);
    }
    fx.mem.set32(a + 0x44, -0x4000);
    fx.mem.set32(a + 0x4C, 0x600);
    if fx.mem.read_u32(D + 0x30) == 0x8000 {
        return;
    }
    for (o, ra) in [(0x44u32, 0x01E0_6CB8u32), (0x4C, 0x01E0_6CDC)] {
        let f = ee::mul(ee::from_i32(fx.mem.i32(a + o)), -1.0);
        let r = ee::fixsfsi(f);
        fx.fix_residue(f, ra);
        fx.mem.set32(a + o, r);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(OT), 0x01E0_6664);
    // The entry's s1 at the library calls: the parameter or the last
    // loop's counter.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0xAF => return 8,
        0x80 => {
            strike(fx, sp);
            s1 = 8;
        }
        0x62 => arch(fx, sp, s1),
        0x5E => {
            set_counter4(fx, D + 0x2408, 8, 0, 0xFF);
            fx.mem.write_u8(D + 0x240C, 0);
            fx.mem.write_u8(D + 0x240D, 1);
            let b = fx.mem.read_u8(D + 0xC);
            fx.mem.write_u8(D + 0x240E, b);
            fx.mem.write_u32(D + 0x2410, D + 0x2484);
            for (o, v) in [(0x2418u32, 0x28u8), (0x241A, 6), (0x2419, 0x14), (0x241B, 0), (0x241E, 2), (0x241C, 3)] {
                fx.mem.write_u8(D + o, v);
            }
            fx.mem.write_u32(D + 0x2420, 0x3ECC_CCCD);
            fx.mem.write_u32(D + 0x2424, 0x3ECC_CCCD);
            fx.mem.write_u32(D + 0x2428, 0x4234_0000);
            fx.mem.write_u32(D + 0x242C, 0xC020_0000);
            fx.mem.write_u32(D + 0x2414, D + 0x3244);
            for o in [0x2433u32, 0x2430, 0x2431, 0x2432] {
                fx.mem.write_u8(D + o, 0x80);
            }
            let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
            node_position_saving(fx, h, s, D + 0x2444, u64::from(OT), Some(s1), 0x01E0_6B84);
            copy_v4(fx, D + 0x2444, D + 0x2454);
        }
        0x5A => {
            sprites(fx, sp);
            s1 = 0x3C;
        }
        0x5D => {
            set_counter4(fx, D + 0xA4, 0xE, 0, 0xFF);
            for (o, v) in [(0xA8u32, 0u8), (0xAC, 6), (0xAD, 1), (0xAE, 3), (0xAF, 0)] {
                fx.mem.write_u8(D + o, v);
            }
            let h = fx.mem.read_u32(D + 4);
            fx.mem.write_u32(D + 0xB0, h);
            for (o, v) in [(0xB4u32, 0x80u8), (0xB5, 0x80), (0xB6, 0x80), (0xB7, 0x28)] {
                fx.mem.write_u8(D + o, v);
            }
        }
        0x52 => {
            let a = D + 0xC8;
            set_counter4(fx, a, 0x24, 0, 0xFF);
            fx.mem.write_u8(D + 0xCC, 1);
            fx.mem.write_u32(D + 0xD0, D + 0x128);
            fx.mem.write_u8(D + 0xF8, 0x64);
            fx.mem.write_u8(D + 0xF9, 8);
            let b = fx.mem.read_u8(D + 0xC);
            fx.mem.write_u8(D + 0xFA, b);
            let b = fx.mem.read_u8(D + 0x10);
            fx.mem.write_u8(D + 0xFB, b);
            fx.mem.write_u32(D + 0x100, 0x3F80_0000);
            fx.mem.write_u8(D + 0xFC, 0);
            fx.mem.write_u16(D + 0xFE, 0);
            fx.mem.write_u32(D + 0x108, 0x4120_0000);
            for (o, v) in [(0x10Fu32, 0xC0u8), (0x10C, 0xC0), (0x10D, 0xC0), (0x10E, 0xC0), (0x112, 0x80), (0x111, 0x10), (0x110, 0x80)] {
                fx.mem.write_u8(D + o, v);
            }
            let mut k = 0u32;
            while (k as i32) < i32::from(fx.mem.read_u8(D + 0xF9)) + 1 {
                fx.mem.set16(D + 0xE6 + 2 * k, -1);
                fx.mem.set16(D + 0xD4 + 2 * k, -1);
                k += 1;
            }
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x62..0x92).contains(&n) {
        let a = D + 0x768;
        let i = first_free(fx, a, 0x54, 0x19);
        if i >= 0 {
            let r = a + 0x54 * i as u32;
            for k in 0..0x15u32 {
                let w = fx.mem.read_u32(a + 4 * k);
                fx.mem.write_u32(r + 4 * k, w);
            }
            fx.mem.write_u8(r, 0xE);
            for o in [0x3Cu32, 0x38, 0x34, 0x18] {
                fx.mem.write_u32(r + o, 0);
            }
        }
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_71F8, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_7208, [Some(u64::from(OT)), Some(s1)]);
    blib::arc::arcs(fx, 0x01E0_0410, D + 0xC8, OT, 1);
    blib::arch::arches(fx, 0x01E0_1690, D + 0x768, OT, 0x19);
    blib::emit::bead_trails_b(fx, 0x01E0_60F0, D + 0x2408, 1);
    // a3 = what beads left (the echoes' first TEXFLUSH reports it) [run].
    let (_, a3) = blib::beam::beads_a3(fx, 0x01E0_5320, D + 0x2484, OT, 0x28);
    blib::radiate::echoes(fx, 0x01E0_1F80, D + 0xA4, OT, 1, a3);
    blib::star::flash(fx, 0x01E0_3130, D + 0xF9C, OT, 1);
    blib::sprite::bounce(fx, D + 0xFE8, OT, 0x3C);
    blib::sheet::sheets(fx, 0x01E0_3CE0, D + 0x1FD8, OT, 0x10);
    blib::fade::fade(fx, D + 0x2398, OT);
    // fade's 0xF0 frame saves the runner's s6 (0) at +0x70 = sp - 0x80,
    // read by the next update's echoes as vertex 1's stale XYZ [run].
    fx.saved(sp - 0x80, &[(0, 0)]);
    blib::streak::ribbon(fx, 0x01E0_4790, D + 0x23A0, OT, 1);
    let n = fx.mem.u16(D);
    if (0x80..0x95).contains(&n) {
        6
    } else if n == 0x95 {
        2
    } else if n == 0x76 {
        1
    } else {
        0
    }
}
