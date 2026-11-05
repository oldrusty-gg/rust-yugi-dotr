//! BMODULE b240 (card 120), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04C60, data area a0 = 0x1ED0000) [F] (helper B4, 31
//! Oct 2026): b076's "a0 = D" family (s3 = D, s0 = the OT). Node bursts
//! (+0xA4, motes +0xE8 from the attacker's nodes 0xB / 0x11) every update;
//! two strikes at the target, on updates 80 and 98 (98 with a first wide
//! flash): a fade (+0x1268), three shrinking sheets (`sheet::sheets`, pool
//! of 6 at +0xEA8; on 80 stacked 1 apart and turned 0x4000 about z, on 98
//! side by side), a flash (pool of 4 at +0xC38), 200 bouncing sprites
//! (pool of 400 at +0x1340), four bars (+0xD68) and a camera shake (+0x40);
//! the stage light pulsed by 0x1031C0 (+0xA0: down on update 80, up on
//! 149). Returns 1 on updates 70 and 88, 6 on 77 and 98..126, 2 on 127, 8
//! on 155 (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `flash` 0x1E015F0, `bars`
//! 0x1E021A0, `sheets` 0x1E02BB0, `fade` 0x1E03120, `ribbon` 0x1E03660
//! (+0x1270, never started), `bounce` 0x1E041F0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E04C60 (a0 = D, a1 = param; s3 = D, s0 = the OT, s1 =
/// the parameter).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x60);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x7E80);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(4, 2);
    for k in 0..3 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0xB, 0), (0x14, 0x11, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): specs 0x1E05860 / 0x1E05870, motes +0xE8 /
    // +0x468, nodes from +0x10 / +0x14.
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xCC, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_5860);
    fx.mem.write_u32(D + 0xB0, 0x01E0_5870);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x10);
    fx.mem.write_u32(D + 0xA8, D + 0x468);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    fx.mem.set16(D + 0xD8, 0);
    fx.mem.write_u32(D + 0xDC, 0x4334_0000);
    fx.mem.write_u32(D + 0xE0, 0);
    fx.mem.write_u32(D + 0xE4, 0x40E0_0000);
    fx.mem.set16(D + 0xA0, 0x50);
    fx.mem.set16(D + 0xA2, 0x95);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's), s2: the caller's.
    rand_bytes(fx, D + 0x7D80, 0x100, 0x01E0_4E08, [Some(OT), Some(3), None]);
    0
}

/// (rand >> 4) % m - off, over 100 (rand called from `ra`).
fn spread(fx: &mut Fx, ra: u32, m: i32, off: i32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h - off), 100.0)
}

/// A flash in the pool of 4 at +0xC38 at `at`: wide (flag 1, 500) or 300
/// x 50 x 35 [F 0x1E04E60 / 0x1E050EC].
fn flash_start(fx: &mut Fx, at: u32, wide: bool) {
    let i = first_free(fx, D + 0xC38, 0x4C, 4);
    if i < 0 {
        return;
    }
    let r = D + 0xC38 + 0x4C * i as u32;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    let c = if wide {
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u8(r + 7, 9);
        let b = fx.mem.read_u8(r + 4) | 1;
        fx.mem.write_u8(r + 4, b);
        fx.mem.write_u32(r + 0x10, 0x43FA_0000);
        fx.mem.write_u32(r + 0x18, 0);
        fx.mem.write_u32(r + 0x14, 0);
        0xC0
    } else {
        fx.mem.write_u8(r + 4, 0x80);
        fx.mem.write_u8(r + 7, 9);
        fx.mem.write_u32(r + 0x10, 0x4396_0000);
        fx.mem.write_u32(r + 0x14, 0x4248_0000);
        fx.mem.write_u32(r + 0x18, 0x420C_0000);
        0x80
    };
    for (o, v) in [(0xCu32, c), (0xD, c), (0xE, c), (0xA, 0x60), (9, 0x10), (8, 0x80), (0xF, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, at, r + 0x1C);
}

/// Updates 80 / 98 [F 0x1E04F38]: at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x50;
    set_counter4(fx, D + 0x1268, 0xC, 0, 0xFF);
    fx.mem.write_u8(D + 0x126C, 1);
    for k in 0..3i32 {
        let i = first_free(fx, D + 0xEA8, 0x3C, 6);
        if i < 0 {
            continue;
        }
        let r = D + 0xEA8 + 0x3C * i as u32;
        set_counter4(fx, r, 0x14, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 1), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        fx.mem.write_u32(r + 0x10, 0x4000_0000);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        fx.mem.write_u32(r + 0x18, if k == 1 { 0x4080_0000 } else { 0x4040_0000 });
        // On update 80 stacked in y and turned; else side by side in x.
        let (angle, axis) = if fx.mem.u16(D) == 0x50 { (0x4000, 0x30) } else { (0, 0x2C) };
        fx.mem.set32(r + 0x24, angle);
        let v = fx.mem.f(r + axis);
        fx.mem.setf(r + axis, ee::add(v, ee::add(-1.0, ee::from_i32(k))));
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let v = fx.mem.i32(r + 0x24);
            fx.mem.set32(r + 0x24, v.wrapping_neg());
        }
    }
    flash_start(fx, at, true);
    // 200 bouncing sprites (0x1E051D4).
    let first = fx.mem.u16(D) == 0x50;
    for _ in 0..0xC8 {
        let i = first_free(fx, D + 0x1340, 0x44, 0x190);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(OT, 0x01E0_51FC) >> 4) & 0xF;
        let b = D + 0x1340 + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, delay, 0xFF);
        for (o, x) in [(4u32, 0u8), (8, 2), (0xB, 2), (0xA, 0), (9, 3), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(b + o, x);
        }
        let x = fx.rand_from(OT, 0x01E0_5264) >> 4;
        let (_, h) = fx.div(x, 0xF);
        let f = ee::div(ee::from_i32(h * 200 - 0x5DC), 100.0);
        fx.mem.setf(b + 0x10, ee::add(60.0, f));
        fx.mem.write_u32(b + 0x14, 0xBF80_0000);
        fx.mem.write_u32(b + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(b + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        let (x, y) = if first {
            let x = spread(fx, 0x01E0_5310, 0x118, 0x8C);
            (x, spread(fx, 0x01E0_535C, 0x78, 0x3C))
        } else {
            let x = spread(fx, 0x01E0_53C0, 0x78, 0x3C);
            (x, spread(fx, 0x01E0_540C, 0xE6, 0x73))
        };
        fx.mem.setf(b + 0x2C, x);
        fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), y));
        let z = spread(fx, 0x01E0_5468, 0xE6, 0x73);
        fx.mem.setf(b + 0x34, z);
    }
    for k in 0..4i32 {
        let r = D + 0xD68 + 0x50 * k as u32;
        set_counter4(fx, r, 0x1C, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x80);
        fx.mem.write_u32(r + 0x10, if k == 2 { 0x44FA_0000 } else { 0x4496_0000 });
        fx.mem.write_u32(r + 0x14, 0x4248_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0x80), (9, 0x40), (8, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.setf(r + 0x18, ee::mul(f32::from_bits(0x4234_0000), ee::from_i32(k)));
        fx.mem.write_u32(r + 0x1C, 0x4000_0000);
        copy_v4(fx, at, r + 0x20);
    }
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
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_5620);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_4E30);
    // The entry's s1 when 0x1031C0 saves it: the parameter or the bars
    // loop's end.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0x9B => return 8,
        0x50 => {
            strike(fx, sp);
            s1 = 4;
        }
        0x62 => {
            flash_start(fx, sp + 0x50, false);
            strike(fx, sp);
            s1 = 4;
        }
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_5640, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5650, [Some(u64::from(OT)), Some(s1)]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x10);
    blib::star::flash(fx, 0x01E0_15F0, D + 0xC38, OT, 4);
    blib::cross::bars(fx, 0x01E0_21A0, D + 0xD68, OT, 4);
    blib::sheet::sheets(fx, 0x01E0_2BB0, D + 0xEA8, OT, 0x10);
    blib::fade::fade(fx, D + 0x1268, OT);
    blib::sprite::bounce(fx, D + 0x1340, OT, 0x190);
    blib::streak::ribbon(fx, 0x01E0_3660, D + 0x1270, OT, 2);
    let n = fx.mem.u16(D);
    if (0x62..0x7F).contains(&n) {
        6
    } else {
        match n {
            0x7F => 2,
            0x4D => 6,
            0x58 | 0x46 => 1,
            _ => 0,
        }
    }
}
