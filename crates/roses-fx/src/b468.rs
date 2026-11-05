//! BMODULE b468 (card 193), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06260, data area a0 = 0x1ED0000) [F] (helper B6, 31
//! Oct 2026): b077's family (s3 = D, s0 = the OT). Node bursts (+0xA0,
//! motes +0xE4, nodes 0x52 / 0x59, updates 54..64) every update; on update
//! 10 four bars (+0x1510 + 0x50 k on node 0x3C, the first 1000 long, the
//! others 400 at 45 k degrees), a flash (+0x14C4) and a camera shake there
//! (+0x40); on 75 an arc pair (+0x1144, nodes 0x3B / 0x3A); on 80 the fade
//! (+0x4BDC), an arch (+0x4BE4, `arch::arches`) at the attacker turned to
//! face the target, a wide flash, a ribbon (+0x4B70) and 200 bouncing
//! sprites (+0x1650) at the target, and the shake on the target's
//! position; on updates 80..95 the arch is copied into a free slot (count
//! 12, still); the stage light pulsed by 0x1031C0 (+0x4BD8). Returns 1 on
//! update 70, 2 on 77, 8 on 150 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `arcs` 0x1E015F0, `ribbon`
//! 0x1E02870, `bars` 0x1E03400, `flash` 0x1E03E10, `bounce` 0x1E049C0,
//! `fade` 0x1E05430, `arches` 0x1E05970 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::b245::{bounce_spawn, Bounce};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const ARCHES: u32 = D + 0x4BE4;

/// The entry 0x1E06260 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x5230);
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
    for (at, node, clear) in [(0xCu32, 0x3B, 1), (0x10, 0x3A, 0), (0x14, 0x52, 0), (0x18, 0x59, 0), (0x1C, 0x3C, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA0): spec 0x1E06FE8 / 0x1E06FF0, motes +0xE4 /
    // +0x7E4.
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xC8, b);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(D + 0xC9, b);
    fx.mem.write_u32(D + 0xA8, 0x01E0_6FE8);
    fx.mem.write_u32(D + 0xAC, 0x01E0_6FF0);
    fx.mem.write_u32(D + 0xA0, D + 0xE4);
    fx.mem.write_u8(D + 0xD0, 0x20);
    fx.mem.write_u32(D + 0xA4, D + 0x7E4);
    fx.mem.write_u8(D + 0xD1, 0x3C);
    fx.mem.write_u8(D + 0xD2, 0);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xD3, b);
    fx.mem.set16(D + 0xD4, 0x36);
    fx.mem.set16(D + 0xD6, 0x40);
    fx.mem.write_u32(D + 0xD8, 0x4334_0000);
    fx.mem.write_u32(D + 0xDC, 0x4316_0000);
    fx.mem.write_u32(D + 0xE0, 0x4100_0000);
    fx.mem.set16(D + 0x4BD8, 2);
    fx.mem.set16(D + 0x4BDA, 0x90);
    // The entry's s0 = the OT, s1 = 2 (the texture loop's).
    rand_bytes(fx, D + 0x5124, 0x100, 0x01E0_6440, [Some(OT), Some(2), None]);
    0
}

/// Update 10 [F 0x1E064A4]: four bars, a flash and the shake on node +0x1C.
fn bars_start(fx: &mut Fx) {
    for k in 0..4i32 {
        let r = D + 0x1510 + 0x50 * k as u32;
        set_counter4(fx, r, 0x10, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0x1C);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 4, 0x80);
        if k == 0 {
            fx.mem.write_u32(r + 0x10, 0x447A_0000);
            fx.mem.write_u32(r + 0x18, 0);
            fx.mem.write_u32(r + 0x14, 0x41A0_0000);
        } else {
            fx.mem.write_u32(r + 0x10, 0x43C8_0000);
            fx.mem.setf(r + 0x18, ee::mul(45.0, ee::from_i32(k)));
            fx.mem.write_u32(r + 0x14, 0x4170_0000);
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0x10), (9, 0x10), (8, 0xC0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x4100_0000);
    }
    let r = D + 0x14C4;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(r + 6, b);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x430C_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x40), (8, 0x10), (0xF, 0xD0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    // a1 = 0x10 left from the flash's byte +8 is the shake's count.
    let s = D + 0x40;
    set_counter4(fx, s, 0x10, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(s + 6, b);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
}

/// Update 75 [F 0x1E0669C]: the arc pair (+0x1144), points +0x11A4.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0x1144;
    set_counter4(fx, g, 0xE, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x11A4);
    fx.mem.write_u8(g + 0x30, 0x32);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3FCC_CCCD);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0xB0), (0x4A, 0x10), (0x49, 0x10), (0x48, 0x80)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0xC + 2 * a, -1);
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        a += 1;
    }
}

/// Update 80 [F 0x1E0675C]: the fade, the arch at the attacker (sp +
/// 0x60), the flash, ribbon and bouncing sprites at the target (sp +
/// 0x50), the shake.
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0x4BDC, 0xA, 0, 0xFF);
    fx.mem.write_u8(D + 0x4BE0, 1);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, sp + 0x60);
    let a = ARCHES;
    set_counter4(fx, a, 0x10, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (0xE, 0x20), (0xD, 0x80), (0xC, 0xC0), (0xF, 0x10), (0xB, 0xC0), (8, 0xC0), (9, 0xC0), (0xA, 0xC0)] {
        fx.mem.write_u8(a + o, v);
    }
    fx.mem.write_u32(a + 0x10, 0x41A0_0000);
    fx.mem.write_u32(a + 0x14, 0x3FC0_0000);
    fx.mem.write_u32(a + 0x18, 0);
    copy_v4(fx, sp + 0x60, a + 0x24);
    fx.mem.write_u32(a + 0x38, 0);
    fx.mem.write_u32(a + 0x34, 0);
    fx.mem.write_u32(a + 0x3C, 0xC0A0_0000);
    fx.mem.set32(a + 0x44, -0x4000);
    if fx.mem.read_u32(D + 0x30) != 0x8000 {
        let v = ee::mul(fx.mem.f(a + 0x3C), -1.0);
        fx.mem.setf(a + 0x3C, v);
        let f = ee::mul(ee::from_i32(fx.mem.i32(a + 0x44)), -1.0);
        let r = ee::fixsfsi(f);
        fx.fix_residue(f, 0x01E0_686C);
        fx.mem.set32(a + 0x44, r);
        let z = ee::sub(fx.mem.f(a + 0x2C), 15.0);
        fx.mem.setf(a + 0x2C, z);
    } else {
        let z = ee::add(fx.mem.f(a + 0x2C), 15.0);
        fx.mem.setf(a + 0x2C, z);
    }
    let r = D + 0x14C4;
    set_counter4(fx, r, 0x20, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u32(r + 0x10, 0x4448_0000);
    fx.mem.write_u32(r + 0x18, 0);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x10), (8, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, sp + 0x50, r + 0x1C);
    let r = D + 0x4B70;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4020_0000);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_699C);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0x5124);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, sp + 0x50, r + 0x28);
    fx.mem.write_u32(r + 0x58, 0x40);
    fx.mem.write_u32(r + 0x5C, 0x80);
    fx.mem.write_u32(r + 0x60, 0x100);
    let v = Bounce { count: 0x20, b8: 1, base: 120.0, vy: 0xBF4C_CCCD, spread: (0xE6, 0x73), s0: OT, ras: [0x01E0_6A48, 0x01E0_6AB4, 0x01E0_6B54, 0x01E0_6BA0, 0x01E0_6BFC] };
    bounce_spawn(fx, D + 0x1650, sp + 0x50, &v);
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_6CB8);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_6468);
    // The entry's s1 at the library calls: the parameter or the last
    // loop's counter.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0x96 => return 8,
        0x50 => {
            strike(fx, sp);
            s1 = 200;
        }
        0x4B => arcs_start(fx),
        0xA => {
            bars_start(fx);
            s1 = 4;
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x50..0x60).contains(&n) {
        let i = first_free(fx, ARCHES, 0x54, 0x10);
        if i >= 0 {
            let r = ARCHES + 0x54 * i as u32;
            for k in 0..0x15u32 {
                let w = fx.mem.read_u32(ARCHES + 4 * k);
                fx.mem.write_u32(r + 4 * k, w);
            }
            fx.mem.write_u8(r, 0xC);
            for o in [0x3Cu32, 0x38, 0x34, 0x18] {
                fx.mem.write_u32(r + o, 0);
            }
        }
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_6D74, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0x4BD8, n, 0x01E0_6D84, [Some(u64::from(OT)), Some(s1)]);
    blib::fade::fade(fx, D + 0x4BDC, OT);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA0, n);
    blib::mote::motes(fx, D + 0xE4, OT, 0x20);
    blib::arc::arcs(fx, 0x01E0_15F0, D + 0x1144, OT, 1);
    blib::star::flash(fx, 0x01E0_3E10, D + 0x14C4, OT, 1);
    blib::cross::bars(fx, 0x01E0_3400, D + 0x1510, OT, 4);
    blib::streak::ribbon(fx, 0x01E0_2870, D + 0x4B70, OT, 1);
    blib::sprite::bounce(fx, D + 0x1650, OT, 0xC8);
    blib::arch::arches(fx, 0x01E0_5970, ARCHES, OT, 0x10);
    match fx.mem.u16(D) {
        0x4D => 2,
        0x46 => 1,
        _ => 0,
    }
}
