//! BMODULE b700 (card 195), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05C10, data area a0 = 0x1ED0000) [F] (helper B10,
//! 31 Oct 2026): s0 = D, s1 = the OT (0x206E40), s2 the parameter then the
//! spawn loops' counters. Update 2 twenty sprays (+0x2C10, 0x60 apart) from
//! node 0x5B; 24 fifteen ribbons (+0x1FE0, 0x68 apart) on node 0x3C, a
//! flash (+0xF9C) and a long camera shake (+0x40); 124 an arc (+0xC8) from
//! node 0x3D to 0x3B; 129 an arch (+0x768) from the attacker's point to the
//! target, copied to a free slot (0x54 apart) on every update 129..160,
//! and ten more sprays; 152 at the target the fade (+0x1FD8), three sheets
//! (+0x3390, 0x3C apart), a flash, eight ribbons and a shake. The stage
//! light pulsed by 0x1031C0 (+0xA0). The echoes (+0xA4) and bouncing
//! sprites (+0xFE8) are stepped but never set (no store to them [F]).
//! Returns 6 on updates 152..172, 2 on 173, 1 on 142, 8 on 226 (no drawing
//! then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `arcs`
//! 0x1E00410, `arches` 0x1E01690, `echoes` 0x1E01F80, `bounce`
//! 0x1E026C0, `flash` 0x1E03130, `ribbon` 0x1E03CE0, `sprays` 0x1E04870,
//! `sheets` 0x1E05160, `fade` 0x1E056D0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E05C10 (a0 = D, a1 = param).
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
    // 0x199208 memset(D, 0, 0x3850).
    fx.bzero(D, 0x3850);
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
    for (at, node, clear) in [(0xCu32, 0x3C, 1), (0x10, 0x5B, 0), (0x14, 0x3D, 0), (0x18, 0x3B, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xDC);
    // The entry's s0 = D, s1 = the OT, s2 = 4 (the texture loop's).
    rand_bytes(fx, D + 0x3750, 0x100, 0x01E0_5D5C, [Some(D), Some(OT), Some(4)]);
    0
}

fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 4
}

/// The spray's source (sp + 0x70, xyz) into +0x3C.. of the record at `r`.
fn source(fx: &mut Fx, sp: u32, r: u32) {
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
        fx.mem.write_u32(r + 0x3C + 4 * c, w);
    }
}

/// Update 2 [F 0x1E05DD8]: twenty sprays from node 0x5B, 1 lower.
fn sprays_first(fx: &mut Fx, sp: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, s, sp + 0x70, u64::from(D), Some(u64::from(OT)), 0x01E0_5DEC);
    let y = ee::sub(fx.mem.f(sp + 0x74), 1.0);
    fx.mem.setf(sp + 0x74, y);
    for s2 in 0..0x14i32 {
        let i = first_free(fx, D + 0x2C10, 0x60, 0x14);
        if i < 0 {
            continue;
        }
        let r = D + 0x2C10 + 0x60 * i as u32;
        set_counter4(fx, r, 0x3C, (5 * s2) & 0xFF, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 7);
        fx.mem.write_u8(r + 8, 0xC);
        let v = rand4(fx, 0x01E0_5E80);
        fx.mem.set8(r + 9, v);
        fx.mem.write_u8(r + 0xA, 1);
        let v = rand4(fx, 0x01E0_5E9C);
        fx.mem.set8(r + 0xB, v & 3);
        fx.mem.write_u8(r + 0xC, 0);
        fx.mem.write_u8(r + 0xD, 2);
        fx.mem.write_u32(r + 0x10, D + 0x3750);
        let v = rand4(fx, 0x01E0_5EC8);
        let mut m = v & 3;
        if v < 0 && m != 0 {
            m -= 4;
        }
        let f = ee::add(12.0, ee::div(ee::from_i32(200 * m - 400), 100.0));
        fx.mem.setf(r + 0x14, f);
        fx.mem.write_u32(r + 0x1C, 0x4100_0000);
        fx.mem.write_u32(r + 0x20, 0x41C8_0000);
        fx.mem.write_u32(r + 0x24, 0);
        for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let v = rand4(fx, 0x01E0_5F60);
        fx.mem.setf(r + 0x58, ee::from_i32((v & 0x7FF) + 0x100));
        let v = rand4(fx, 0x01E0_5F88);
        let (_, m) = fx.div(v, 0x1E);
        let f = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::from_i32(m - 0xF), 100.0));
        fx.mem.setf(r + 0x4C, f);
        source(fx, sp, r);
    }
}

/// Update 24 [F 0x1E06014]: fifteen ribbons on node 0x3C, the flash and a
/// long shake.
fn ribbons_first(fx: &mut Fx) {
    for s2 in 0..0xFi32 {
        let i = first_free(fx, D + 0x1FE0, 0x68, 0xF);
        if i < 0 {
            continue;
        }
        let r = D + 0x1FE0 + 0x68 * i as u32;
        set_counter4(fx, r, (0x64 - 6 * s2) & 0xFF, (6 * s2) & 0xFF, 0xFF);
        fx.mem.write_u8(r + 4, 0x85);
        fx.mem.write_u32(r + 0x14, 0x4040_0000);
        fx.mem.write_u32(r + 0x1C, 0x4220_0000);
        fx.mem.set8(r + 0xD, 0x58 - 6 * s2);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(r + 6, b);
        for (o, v) in [(0xAu32, 1u8), (0xB, 2), (0xC, 3)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.set8(r + 8, s2 + 1);
        let v = rand4(fx, 0x01E0_60E8);
        fx.mem.set8(r + 9, v);
        fx.mem.write_u32(r + 0x10, D + 0x3750);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for (o, v) in [(0x26u32, 0x80u8), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x25, 0x40), (0x24, 0x80), (0x23, 0x10), (0x27, 0x40)] {
            fx.mem.write_u8(r + o, v);
        }
        for (o, ra) in [(0x58u32, 0x01E0_6138u32), (0x5C, 0x01E0_6150), (0x60, 0x01E0_6168)] {
            let v = rand4(fx, ra);
            fx.mem.set32(r + o, v & 0xFF);
        }
    }
    let a = D + 0xF9C;
    set_counter4(fx, a, 0x80, 0, 0xFF);
    fx.mem.write_u8(a + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(a + 6, b);
    fx.mem.write_u8(a + 4, 0x83);
    fx.mem.write_u8(a + 7, 9);
    fx.mem.write_u32(a + 0x10, 0x4348_0000);
    fx.mem.write_u32(a + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x40), (8, 0x20), (0xF, 0xD0)] {
        fx.mem.write_u8(a + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(a + o, 0);
    }
    // The shake's count is the 0x80 left in a1 by the flash's colour.
    set_counter4(fx, D + 0x40, 0x80, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(D + 0x46, b);
    fx.mem.write_u8(D + 0x44, 4);
    fx.mem.write_u32(D + 0x54, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x58, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x4080_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(D), 0x01E0_6274);
}

/// Update 124 [F 0x1E0627C]: the arc from node 0x3D to 0x3B.
fn arc_start(fx: &mut Fx) {
    set_counter4(fx, D + 0xC8, 0xA4, 0, 0xFF);
    fx.mem.write_u8(D + 0xCC, 1);
    fx.mem.write_u32(D + 0xD0, D + 0x128);
    fx.mem.write_u8(D + 0xF8, 0x14);
    fx.mem.write_u8(D + 0xF9, 8);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xFA, b);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(D + 0xFB, b);
    fx.mem.write_u32(D + 0x100, 0x3F80_0000);
    fx.mem.write_u8(D + 0xFC, 0);
    fx.mem.write_u16(D + 0xFE, 0);
    fx.mem.write_u32(D + 0x108, 0x4120_0000);
    for (o, v) in [(0x10Cu32, 0x80u8), (0x10D, 0x80), (0x10E, 0x80), (0x10F, 0xC0), (0x112, 0x10), (0x111, 0x20), (0x110, 0xC0)] {
        fx.mem.write_u8(D + o, v);
    }
    let mut k = 0u32;
    while (k as i32) < i32::from(fx.mem.read_u8(D + 0xF9)) + 1 {
        fx.mem.set16(D + 0xE6 + 2 * k, -1);
        fx.mem.set16(D + 0xD4 + 2 * k, -1);
        k += 1;
    }
}

/// Update 129 [F 0x1E0634C]: the arch template (+0x768) from the
/// attacker's point toward the target over 28 counts, then ten sprays.
fn arch_start(fx: &mut Fx, sp: u32, s2: u64) {
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, sp + 0x70);
    // 0x1F65D0's 0x50 frame (s0 = D, s1 = OT, s2, s3 (the runner's 0),
    // ra) [F].
    fx.saved(sp - 0x50, &[(0, u64::from(D)), (0x10, u64::from(OT)), (0x20, s2), (0x30, 0), (0x40, 0x01E0_6360)]);
    let a = D + 0x768;
    set_counter4(fx, a, 0x20, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (0xE, 0x20), (0xD, 0x80), (0xC, 0xC0), (0xF, 0x20), (0xB, 0xC0), (8, 0xC0), (9, 0xC0), (0xA, 0xC0)] {
        fx.mem.write_u8(a + o, v);
    }
    fx.mem.write_u32(a + 0x10, 0x4080_0000);
    fx.mem.write_u32(a + 0x14, 0x4000_0000);
    fx.mem.write_u32(a + 0x18, 0x3F4C_CCCD);
    copy_v4(fx, sp + 0x70, a + 0x24);
    for c in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x60 + 4 * c), fx.mem.f(a + 0x24 + 4 * c)), 28.0);
        fx.mem.setf(a + 0x34 + 4 * c, v);
    }
    fx.mem.set32(a + 0x44, -0x4000);
    fx.mem.set32(a + 0x4C, 0x4000);
    if fx.mem.read_u32(D + 0x30) != 0x8000 {
        for (o, ra) in [(0x44u32, 0x01E0_647Cu32), (0x4C, 0x01E0_64A0)] {
            let f = ee::mul(ee::from_i32(fx.mem.i32(a + o)), -1.0);
            let r = ee::fixsfsi(f);
            fx.fix_residue(f, ra);
            fx.mem.set32(a + o, r);
        }
    }
    for s2 in 0..0xAi32 {
        let i = first_free(fx, D + 0x2C10, 0x60, 0x14);
        if i < 0 {
            continue;
        }
        let r = D + 0x2C10 + 0x60 * i as u32;
        set_counter4(fx, r, 0x14, s2 & 0xFF, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u8(r + 8, 0x10);
        let v = rand4(fx, 0x01E0_6514);
        fx.mem.set8(r + 9, v);
        for (o, v) in [(0xAu32, 1u8), (0xB, 3), (0xC, 0), (0xD, 2)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, D + 0x3750);
        let v = rand4(fx, 0x01E0_654C);
        let (_, m) = fx.div(v, 6);
        let f = ee::add(20.0, ee::div(ee::from_i32(200 * m - 600), 100.0));
        fx.mem.setf(r + 0x14, f);
        fx.mem.write_u32(r + 0x20, 0x4220_0000);
        fx.mem.write_u32(r + 0x24, 0);
        for o in [0x28u32, 0x29, 0x2A, 0x2B] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.set32(r + 0x2C, 0x4000);
        for o in [0x34u32, 0x30, 0x5C, 0x58, 0x54] {
            fx.mem.write_u32(r + o, 0);
        }
        let v = rand4(fx, 0x01E0_65F4);
        let (_, m) = fx.div(v, 0x28);
        let f = ee::add(-1.0, ee::div(ee::from_i32(m - 0x14), 100.0));
        fx.mem.setf(r + 0x50, f);
        fx.mem.write_u32(r + 0x4C, 0x3E80_0000);
        fx.mem.write_u32(r + 0x48, 0);
        source(fx, sp, r);
    }
}

/// Update 152 [F 0x1E06684]: the strike at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0x1FD8, 0x12, 0, 0xFF);
    fx.mem.write_u8(D + 0x1FDC, 1);
    // Three sheets (+0x3390, 0x3C apart).
    for s2 in 0..3i32 {
        let r = D + 0x3390 + 0x3C * s2 as u32;
        set_counter4(fx, r, 0x1C, s2 & 0xFF, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0x80), (8, 3), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x60, r + 0x2C);
        let dx = ee::add(-1.5, ee::mul(1.5, ee::from_i32(s2)));
        let x = ee::add(fx.mem.f(r + 0x2C), dx);
        fx.mem.setf(r + 0x2C, x);
        fx.mem.write_u32(r + 0x10, 0x4040_0000);
        fx.mem.write_u32(r + 0x14, 0x3DCC_CCCD);
        fx.mem.write_u32(r + 0x18, if s2 == 1 { 0x40A0_0000 } else { 0x4060_0000 });
    }
    // The flash (+0xF9C).
    let a = D + 0xF9C;
    set_counter4(fx, a, 0x24, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x80), (7, 9)] {
        fx.mem.write_u8(a + o, v);
    }
    fx.mem.write_u32(a + 0x10, 0x4348_0000);
    fx.mem.write_u32(a + 0x14, 0x428C_0000);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x40), (9, 0x80), (8, 0xC0), (0xF, 0xD0)] {
        fx.mem.write_u8(a + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(a + o, 0);
    }
    copy_v4(fx, sp + 0x60, a + 0x1C);
    fx.mem.write_u32(a + 0x18, 0x41A0_0000);
    // Eight ribbons (+0x1FE0, 0x68 apart), delayed 2 apart.
    for s2 in 0..8i32 {
        let r = D + 0x1FE0 + 0x68 * s2 as u32;
        set_counter4(fx, r, 0x18, (2 * s2) & 0xFF, 0xFF);
        fx.mem.write_u8(r + 4, 7);
        let f2 = ee::from_i32(s2);
        fx.mem.setf(r + 0x14, ee::add(1.0, ee::mul(f32::from_bits(0x3E99_999A), f2)));
        fx.mem.setf(r + 0x1C, ee::add(60.0, ee::mul(9.0, f2)));
        for (o, v) in [(0xDu32, 0u8), (5, 0), (0xA, 2), (0xB, 2), (0xC, 3)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.set8(r + 8, 2 * s2 + 0x1E);
        let v = rand4(fx, 0x01E0_6924);
        fx.mem.set8(r + 9, v);
        fx.mem.write_u32(r + 0x10, D + 0x3750);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22, 0x23] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x27, 0);
        copy_v4(fx, sp + 0x60, r + 0x28);
        for o in [0x60u32, 0x5C, 0x58] {
            fx.mem.write_u32(r + o, 0);
        }
    }
    // The camera shake (+0x40) at the target.
    set_counter4(fx, D + 0x40, 0x24, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x3F80_0000);
    fx.mem.write_u32(D + 0x58, 0x4000_0000);
    fx.mem.write_u32(D + 0x5C, 0x4000_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(D), 0x01E0_6A00);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_5D84);
    let s2 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0xE2 => return 8,
        0x98 => strike(fx, sp),
        0x81 => arch_start(fx, sp, s2),
        0x7C => arc_start(fx),
        0x18 => ribbons_first(fx),
        2 => sprays_first(fx, sp),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x81..0xA1).contains(&n) {
        let a = D + 0x768;
        let i = first_free(fx, a, 0x54, 0x19);
        if i >= 0 {
            let r = a + 0x54 * i as u32;
            for k in 0..0x15u32 {
                let w = fx.mem.read_u32(a + 4 * k);
                fx.mem.write_u32(r + 4 * k, w);
            }
            fx.mem.write_u8(r, 0xC);
            for o in [0x3Cu32, 0x38, 0x34, 0x18] {
                fx.mem.write_u32(r + o, 0);
            }
        }
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_6AB8, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6AC8, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::arc::arcs(fx, 0x01E0_0410, D + 0xC8, OT, 1);
    blib::arch::arches(fx, 0x01E0_1690, D + 0x768, OT, 0x19);
    // Never set, so never drawn: the a3 it would report is moot [F].
    blib::radiate::echoes(fx, 0x01E0_1F80, D + 0xA4, OT, 1, 0);
    blib::star::flash(fx, 0x01E0_3130, D + 0xF9C, OT, 1);
    blib::sprite::bounce(fx, D + 0xFE8, OT, 0x3C);
    blib::fade::fade(fx, D + 0x1FD8, OT);
    blib::streak::ribbon(fx, 0x01E0_3CE0, D + 0x1FE0, OT, 0x1E);
    blib::spray::sprays(fx, 0x01E0_4870, D + 0x2C10, OT, 0x14);
    blib::sheet::sheets(fx, 0x01E0_5160, D + 0x3390, OT, 0x10);
    let n = fx.mem.u16(D);
    if (0x98..0xAD).contains(&n) {
        6
    } else if n == 0xAD {
        2
    } else if n == 0x8E {
        1
    } else {
        0
    }
}
