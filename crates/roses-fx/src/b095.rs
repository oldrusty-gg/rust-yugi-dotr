//! BMODULE b095 (card 105), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03D10, data area a0 = 0x1ED0000) [F] (helper B4, 31
//! Oct 2026): b076's "a0 = D" family (s3 = D, s0 = the OT). Node bursts
//! (+0xA4, motes +0xE8 from the attacker's nodes 0x17 / 0xB) every update;
//! eight bursts at the target (updates 129 + 10 k, k = 0..7): three
//! shrinking sheets (`sheet::sheets`, pool of 6 at +0xD68, stacked 1 apart,
//! turned about z by 0x4000 / +-0x2800 as the update picks, mirrored by
//! the attacker's facing), a flash (pool of 4 at +0xC38), 100 bouncing
//! sprites (pool of 400 at +0x1D58) and a camera shake (+0x40); on even
//! updates 129..198 a ribbon (pool of 30 at +0x1128); the stage light
//! pulsed by 0x1031C0 (+0xA0: down on update 123, up on 241). Returns 1 on
//! update 119, 6 on 129..211, 2 on 212, 8 on 247 (and at once for a
//! parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `flash` 0x1E015F0, `sheets`
//! 0x1E021A0, `ribbon` 0x1E02710, `bounce` 0x1E032A0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::b245::{bounce_spawn_n, Bounce};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x8798;

/// The entry 0x1E03D10 (a0 = D, a1 = param; s3 = D, s0 = the OT, s1 =
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
    fx.bzero(D, 0x88A0);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x17, 0), (0x14, 0xB, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): specs 0x1E047B8 / 0x1E047D0, motes +0xE8 /
    // +0x468, nodes from +0x10 / +0x14.
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xCC, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_47B8);
    fx.mem.write_u32(D + 0xB0, 0x01E0_47D0);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x10);
    fx.mem.write_u32(D + 0xA8, D + 0x468);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    fx.mem.set16(D + 0xD8, 0);
    fx.mem.write_u32(D + 0xDC, 0x4316_0000);
    fx.mem.write_u32(D + 0xE0, 0);
    fx.mem.write_u32(D + 0xE4, 0x40C0_0000);
    fx.mem.set16(D + 0xA0, 0x7B);
    fx.mem.set16(D + 0xA2, 0xF1);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's), s2: the caller's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_3EBC, [Some(OT), Some(3), None]);
    0
}

/// b095's (0x1E04224).
const BOUNCE: Bounce = Bounce { count: 0x20, b8: 2, base: 50.0, vy: 0xBECC_CCCD, spread: (0xB4, 0x5A), s0: OT, ras: [0x01E0_424C, 0x01E0_42B4, 0x01E0_4354, 0x01E0_43A0, 0x01E0_43FC] };

/// Updates 129 + 10 k [F 0x1E03F5C]: at the target (sp + 0x50).
fn burst(fx: &mut Fx, sp: u32) {
    let at = sp + 0x50;
    for k in 0..3i32 {
        let i = first_free(fx, D + 0xD68, 0x3C, 6);
        if i < 0 {
            continue;
        }
        let r = D + 0xD68 + 0x3C * i as u32;
        set_counter4(fx, r, 0x10, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 1), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        fx.mem.write_u32(r + 0x10, 0x4000_0000);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        fx.mem.write_u32(r + 0x18, if k == 1 { 0x4020_0000 } else { 0x3FE6_6666 });
        let y = fx.mem.f(r + 0x30);
        fx.mem.setf(r + 0x30, ee::add(y, ee::add(-1.0, ee::from_i32(k))));
        match fx.mem.u16(D) {
            0xA9 | 0x95 => fx.mem.set32(r + 0x24, 0x4000),
            0xBD | 0x8B => fx.mem.set32(r + 0x24, 0x2800),
            0xC7 | 0x9F | 0x81 => fx.mem.set32(r + 0x24, -0x2800),
            _ => {}
        }
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let v = fx.mem.i32(r + 0x24);
            fx.mem.set32(r + 0x24, v.wrapping_neg());
        }
    }
    let i = first_free(fx, D + 0xC38, 0x4C, 4);
    if i >= 0 {
        let r = D + 0xC38 + 0x4C * i as u32;
        set_counter4(fx, r, 0xB, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u8(r + 7, 9);
        let b = fx.mem.read_u8(r + 4) | 1;
        fx.mem.write_u8(r + 4, b);
        fx.mem.write_u32(r + 0x10, 0x4416_0000);
        fx.mem.write_u32(r + 0x14, 0x4120_0000);
        fx.mem.write_u32(r + 0x18, 0);
        for (o, v) in [(0xFu32, 0xF0u8), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x20), (9, 0x20), (8, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(r + o, 0);
        }
        copy_v4(fx, at, r + 0x1C);
    }
    bounce_spawn_n(fx, D + 0x1D58, at, &BOUNCE, (0xF, 0x5DC), (0x64, 0x190, 3));
    let s = D + 0x40;
    set_counter4(fx, s, 9, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_44B8);
}

/// Even updates 129..198 [F 0x1E044F4]: a ribbon at the target; its index
/// x 0x68 (the entry's s1 then).
fn ribbon_start(fx: &mut Fx, sp: u32) -> Option<u32> {
    let i = first_free(fx, D + 0x1128, 0x68, 0x1E);
    if i < 0 {
        return None;
    }
    let r = D + 0x1128 + 0x68 * i as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4000_0000);
    fx.mem.write_u32(r + 0x1C, 0x41C8_0000);
    fx.mem.write_u8(r + 8, 0x28);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 0xA, 2);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 5);
    let x = fx.rand_from(OT, 0x01E0_4584);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    fx.mem.write_u8(r + 0x23, 0);
    fx.mem.write_u8(r + 0x27, 0x60);
    for (o, ra) in [(0x58u32, 0x01E0_45D4u32), (0x5C, 0x01E0_45EC), (0x60, 0x01E0_4604)] {
        let x = fx.rand_from(OT, ra) >> 4;
        fx.mem.set32(r + o, x & 0x7F);
    }
    copy_v4(fx, sp + 0x50, r + 0x28);
    Some(0x68 * i as u32)
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_3EE4);
    // The entry's s1 when 0x1031C0 saves it: the parameter, the sprite
    // loop's end or the ribbon's offset.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0xF7 => return 8,
        0xC7 | 0xBD | 0xB3 | 0xA9 | 0x9F | 0x95 | 0x8B | 0x81 => {
            burst(fx, sp);
            s1 = 0x64;
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x81..0xC8).contains(&n) && n & 1 == 0 {
        if let Some(o) = ribbon_start(fx, sp) {
            s1 = u64::from(o);
        }
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_4644, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4654, [Some(u64::from(OT)), Some(s1)]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x10);
    blib::star::flash(fx, 0x01E0_15F0, D + 0xC38, OT, 4);
    blib::sheet::sheets(fx, 0x01E0_21A0, D + 0xD68, OT, 0x10);
    blib::sprite::bounce(fx, D + 0x1D58, OT, 0x190);
    blib::streak::ribbon(fx, 0x01E0_2710, D + 0x1128, OT, 0x1E);
    let n = fx.mem.u16(D);
    if (0x81..0xD4).contains(&n) {
        6
    } else if n == 0xD4 {
        2
    } else if n == 0x77 {
        1
    } else {
        0
    }
}
