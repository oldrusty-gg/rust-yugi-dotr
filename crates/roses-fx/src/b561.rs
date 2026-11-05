//! BMODULE b561 (card 423), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E032F0, data area a0 = 0x1ED0000) [F] (helper B5, 31
//! Oct 2026): b076's "a0 = D" family (s0 = D, s1 = the OT). On update 2
//! three long flashes (+0x2024, kept on the line between the attacker's
//! nodes 0x39 / 0x38) and a long camera shake (+0x40); on 89 a flash on
//! node 0x39 (+0x1904); on updates 20..176 three (from 156 six) bouncing
//! sprites a update gather from about the nodes (+0x2110); on odd updates
//! 89..191 a ribbon from node 0x39 (+0xA4); on 177 the fade (+0x2108), a
//! flash, a ribbon, 180 bouncing sprites, four band rings (+0x70C0,
//! `band::bands`) and a short shake, all at the target; the stage light
//! pulsed by 0x1031C0 (+0xA0). Returns 1 on update 167, 6 on 177..205, 2
//! on 206, 8 on 260 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `ribbon` 0x1E00FC0, `fade` 0x1E01B50, `bounce` 0x1E02090,
//! `bands` 0x1E02B00 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x7230;
const FLASHES: u32 = D + 0x1904;
const LINE: u32 = D + 0x2024;
const RIBBONS: u32 = D + 0xA4;
const SPRITES: u32 = D + 0x2110;

/// The entry 0x1E032F0 (a0 = D, a1 = param; s0 = D, s1 = the OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xF0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0xF0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x7330);
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
    for (at, node, clear) in [(0xCu32, 0x39, 1), (0x10, 0x38, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xFE);
    rand_bytes(fx, BYTES, 0x100, 0x01E0_341C, [Some(D), Some(OT), Some(2)]);
    0
}

/// (rand >> 4) % `m` (a true division) - `h`, over 100.
fn spread(fx: &mut Fx, ra: u32, m: i32, h: i32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let (_, r) = fx.div(x, m);
    ee::div(ee::from_i32(r - h), 100.0)
}

/// (rand >> 4) % `m` for a power of two (the compiler's masks, LO / HI
/// untouched) x 200 - `h`, over 100.
fn spread_pow2(fx: &mut Fx, ra: u32, m: i32, h: i32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let r = x.wrapping_rem(m);
    ee::div(ee::from_i32(r * 200 - h), 100.0)
}

/// Update 2 [F 0x1E03480]: three long flashes 80 / 100 / 120 wide and the
/// long shake on node +0xC.
fn charge(fx: &mut Fx) {
    for k in 0..3u32 {
        let r = LINE + 0x4C * k;
        set_counter4(fx, r, 0xB1, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0x85), (7, 9), (0xA, 0x60), (9, 0x50), (8, 0x10), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0xC0)] {
            fx.mem.write_u8(r + o, v);
        }
        let w = ee::add(80.0, ee::mul(20.0, ee::from_i32(k as i32)));
        fx.mem.setf(r + 0x10, w);
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0);
    }
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

/// Update 89 [F 0x1E035B0]: a flash on node +0xC.
fn glow(fx: &mut Fx) {
    let i = first_free(fx, FLASHES, 0x4C, 0x18);
    if i < 0 {
        return;
    }
    let r = FLASHES + 0x4C * i as u32;
    set_counter4(fx, r, 0x58, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    for (o, v) in [(4u32, 0x81u8), (7, 9), (0xA, 0x60), (9, 0x60), (8, 0x20), (0xF, 0xA0), (0xC, 0xA0), (0xD, 0xA0), (0xE, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, 0x41F0_0000);
    fx.mem.write_u32(r + 0x14, 0x4100_0000);
}

/// Update 177 [F 0x1E03664]: the fade, a flash, a ribbon, 180 bouncing
/// sprites, four band rings and the short shake at the target (sp +
/// 0x90).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x90;
    set_counter4(fx, D + 0x2108, 0xA, 0, 0xFF);
    fx.mem.write_u8(D + 0x210C, 1);
    let i = first_free(fx, FLASHES, 0x4C, 0x18);
    if i >= 0 {
        let r = FLASHES + 0x4C * i as u32;
        set_counter4(fx, r, 0x20, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 1), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x60), (9, 0x60), (8, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x4348_0000);
        fx.mem.write_u32(r + 0x14, 0x41F0_0000);
        copy_v4(fx, at, r + 0x1C);
    }
    let i = first_free(fx, RIBBONS, 0x68, 0x3C);
    if i >= 0 {
        let r = RIBBONS + 0x68 * i as u32;
        fx.mem.write_u8(r + 4, 5);
        set_counter4(fx, r, 0x1C, 0, 0xFF);
        fx.mem.write_u32(r + 0x14, 0x4080_0000);
        fx.mem.write_u32(r + 0x1C, 0x4220_0000);
        for (o, v) in [(0x27u32, 0x70u8), (0x23, 0), (5, 0), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0x3C)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(D, 0x01E0_37E8);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
    }
    for _ in 0..0xB4 {
        let i = first_free(fx, SPRITES, 0x44, 0x12C);
        if i < 0 {
            continue;
        }
        let x = fx.rand_from(D, 0x01E0_3878) >> 4;
        let b = SPRITES + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, x & 7, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 0), (0xB, 2), (0xA, 0), (9, 3), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(b + o, v);
        }
        let v = spread_pow2(fx, 0x01E0_38E0, 8, 0x320);
        fx.mem.setf(b + 0x10, ee::add(40.0, v));
        fx.mem.write_u32(b + 0x14, 0xBF80_0000);
        fx.mem.write_u32(b + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(b + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        let v = spread(fx, 0x01E0_3980, 0xE6, 0x73);
        fx.mem.setf(b + 0x2C, v);
        let v = spread(fx, 0x01E0_39CC, 0xB4, 0x5A);
        fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), v));
        let v = spread(fx, 0x01E0_3A28, 0xE6, 0x73);
        fx.mem.setf(b + 0x34, v);
    }
    for k in 0..4i32 {
        let i = first_free(fx, D + 0x70C0, 0x5C, 8);
        if i < 0 {
            continue;
        }
        let r = D + 0x70C0 + 0x5C * i as u32;
        set_counter4(fx, r, 0x1C, k & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 1), (8, 1), (9, 2), (0xA, 6), (0xB, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x3F00_0000);
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0x3F99_999A);
        fx.mem.write_u32(r + 0x1C, 0x3F4C_CCCD);
        fx.mem.write_u32(r + 0x20, 0x3E80_0000);
        for (o, v) in [(0x28u32, 0x80u8), (0x29, 0x80), (0x2A, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0x10), (0x2B, 0x30)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        fx.mem.set32(r + 0x3C, 0x4000);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}

/// The two nodes into sp + 0xA0 / + 0xB0 [F 0x1E03C3C / 0x1E03D4C].
fn nodes(fx: &mut Fx, sp: u32, ras: [u32; 2]) {
    for (slot, out, ra) in [(0xCu32, 0xA0u32, ras[0]), (0x10, 0xB0, ras[1])] {
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + slot);
        node_position_saving(fx, h, n, sp + out, u64::from(D), Some(u64::from(OT)), ra);
    }
}

/// While the line flashes live [F 0x1E03C30]: flash k at 1 + k quarters
/// of the way from node +0xC to node +0x10.
fn line(fx: &mut Fx, sp: u32) {
    nodes(fx, sp, [0x01E0_3C50, 0x01E0_3C68]);
    let step: [f32; 3] = std::array::from_fn(|c| {
        let c = 4 * c as u32;
        ee::div(ee::sub(fx.mem.f(sp + 0xB0 + c), fx.mem.f(sp + 0xA0 + c)), 4.0)
    });
    for k in 0..3u32 {
        let t = ee::from_i32(k as i32 + 1);
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0xA0 + 4 * c), ee::mul(step[c as usize], t));
            fx.mem.setf(LINE + 0x4C * k + 0x1C + 4 * c, v);
        }
    }
}

/// Updates 20..176 [F 0x1E03D4C]: three (from 156 six) sprites a update
/// from about node +0xC, the midpoint and node +0x10 in turn, falling back
/// onto it over their count (20 + rand % 6 x 2 - 6, 0x1A2B80).
fn gather(fx: &mut Fx, sp: u32) {
    nodes(fx, sp, [0x01E0_3D60, 0x01E0_3D78]);
    for c in 0..3u32 {
        let (a, b) = (fx.mem.f(sp + 0xA0 + 4 * c), fx.mem.f(sp + 0xB0 + 4 * c));
        fx.mem.setf(sp + 0xC0 + 4 * c, ee::add(a, ee::div(ee::sub(b, a), 2.0)));
    }
    let n = if fx.mem.u16(D) < 0x9C { 3 } else { 6 };
    for k in 0..n {
        let i = first_free(fx, SPRITES, 0x44, 0x12C);
        if i < 0 {
            continue;
        }
        let x = fx.rand_from(D, 0x01E0_3E20) >> 4;
        let (_, m) = fx.div(x, 6);
        let life = ee::add(20.0, ee::div(ee::from_i32(m * 200 - 0x258), 100.0));
        let c = ee::fixsfsi(life);
        fx.fix_residue(life, 0x01E0_3E88);
        let b = SPRITES + 0x44 * i as u32;
        set_counter4(fx, b, c & 0xFF, 0, 0xFF);
        fx.mem.write_u8(b + 4, 0x41);
        fx.mem.write_u8(b + 8, 0);
        fx.mem.write_u8(b + 0xB, 2);
        let x = fx.rand_from(D, 0x01E0_3ED0) >> 4;
        fx.mem.set8(b + 9, x & 7);
        fx.mem.write_u8(b + 0xA, 0);
        let x = fx.rand_from(D, 0x01E0_3EEC) >> 4;
        let v = (x & 0x3F) + 0x60;
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.set8(b + o, v);
        }
        fx.mem.write_u32(b + 0x10, 0x41F0_0000);
        fx.mem.write_u32(b + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(b + 0x1C, 0);
        fx.mem.write_u32(b + 0x18, 0);
        for (c, ra) in [(0u32, 0x01E0_3F30u32), (4, 0x01E0_3F94), (8, 0x01E0_3FF8)] {
            let v = spread_pow2(fx, ra, 4, 0x190);
            fx.mem.setf(sp + 0xD0 + c, v);
        }
        let (_, j) = fx.div(k, 3);
        let p = sp + 0xA0 + 16 * j as u32;
        let jx = fx.mem.f(sp + 0xD0);
        let v = ee::add(jx, fx.mem.f(p));
        fx.mem.setf(b + 0x20, v);
        let jy = fx.mem.f(sp + 0xD4);
        let v = ee::add(jy, fx.mem.f(p + 4));
        fx.mem.setf(b + 0x24, v);
        let jz = fx.mem.f(sp + 0xD8);
        let v = ee::add(jz, fx.mem.f(p + 8));
        fx.mem.setf(b + 0x28, v);
        for (o, v) in [(0x2Cu32, jx), (0x30, jy), (0x34, jz)] {
            fx.mem.setf(b + o, ee::neg(ee::div(v, life)));
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(b + o, 0);
        }
    }
}

/// Odd updates 89..191 [F 0x1E04120]: a ribbon from node +0xC (wider from
/// update 150).
fn ribbon(fx: &mut Fx) {
    let i = first_free(fx, RIBBONS, 0x68, 0x3C);
    if i < 0 {
        return;
    }
    let r = RIBBONS + 0x68 * i as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0x85);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 3)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(D, 0x01E0_41A8);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x30)] {
        fx.mem.write_u8(r + o, v);
    }
    if fx.mem.u16(D) >= 0x96 {
        fx.mem.write_u8(r + 8, 0x19);
        fx.mem.write_u32(r + 0x14, 0x400C_CCCD);
        fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
    } else {
        fx.mem.write_u8(r + 8, 0x10);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        fx.mem.write_u32(r + 0x1C, 0x41A0_0000);
    }
    for (o, ra) in [(0x58u32, 0x01E0_423Cu32), (0x5C, 0x01E0_4254), (0x60, 0x01E0_426C)] {
        let x = fx.rand_from(D, ra) >> 4;
        fx.mem.set32(r + o, x & 0x7F);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x90, u64::from(D), 0x01E0_3444);
    match fx.mem.u16(D) {
        0x104 => return 8,
        0xB1 => strike(fx, sp),
        0x59 => glow(fx),
        2 => charge(fx),
        _ => {}
    }
    if fx.mem.u8(LINE + 0x4C) > 0 {
        line(fx, sp);
    }
    let n = fx.mem.u16(D);
    if (0x14..0xB1).contains(&n) {
        gather(fx, sp);
    }
    let n = fx.mem.u16(D);
    if (0x59..0xC1).contains(&n) && n & 1 != 0 {
        ribbon(fx);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_4288, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4298, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0x2108, OT);
    blib::star::flash(fx, 0x01E0_0410, FLASHES, OT, 0x18);
    blib::star::flash(fx, 0x01E0_0410, LINE, OT, 3);
    blib::streak::ribbon(fx, 0x01E0_0FC0, RIBBONS, OT, 0x3C);
    blib::sprite::bounce(fx, SPRITES, OT, 0x12C);
    blib::band::bands(fx, 0x01E0_2B00, D + 0x70C0, OT, 4);
    let n = fx.mem.u16(D);
    if (0xB1..0xCE).contains(&n) {
        6
    } else if n == 0xCE {
        2
    } else if n == 0xA7 {
        1
    } else {
        0
    }
}
