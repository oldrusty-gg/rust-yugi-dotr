//! BMODULE b245 (card 217), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05970, data area a0 = 0x1ED0000) [F] (helper B3, 30
//! Oct 2026): b076's family. Node bursts (+0xA4, `burst::node_bursts`,
//! motes +0xE8 from the attacker's nodes 0x5D / 0x53) every update; on
//! update 108 an arc pair (+0x17E8, nodes 0x43 / 0x42); on 113 at the
//! target a flash (+0x1AC8), three ribbons (+0x1C54 + 0x68 k: one wide,
//! two thin turned 0xE000 / 0x6000), 200 bouncing sprites (+0x1D8C, 0x44
//! each: thrown up 1 with random spread, delays up to 15), a camera shake
//! (+0x40) and a fade (+0x52AC); the stage light pulsed by 0x1031C0 (+0xA0:
//! down on update 2, up on 234). Returns 1 on update 103, 2 on 110, 6 on
//! 113..133, 8 on 240 (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `arcs` 0x1E015F0, `flash`
//! 0x1E02870, `bars` 0x1E03420 (+0x1B14, never started), `ribbon`
//! 0x1E03E30, `bounce` 0x1E049C0, `fade` 0x1E05430 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E05970 (a0 = D, a1 = param).
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
    fx.bzero(D, 0x53C0);
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
    for (at, node, clear) in [(0xCu32, 0x5D, 1), (0x10, 0x53, 0), (0x14, 0x43, 0), (0x18, 0x42, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): spec 0x1E06400 / 0x1E063E8, motes +0xE8 /
    // +0xB68.
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(D + 0xCC, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_6400);
    fx.mem.write_u32(D + 0xB0, 0x01E0_63E8);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x30);
    fx.mem.write_u32(D + 0xA8, D + 0xB68);
    fx.mem.write_u8(D + 0xD5, 0x50);
    fx.mem.write_u8(D + 0xD6, 0);
    fx.mem.set16(D + 0xD8, 0);
    fx.mem.write_u32(D + 0xDC, 0x4316_0000);
    fx.mem.write_u32(D + 0xE4, 0x40E0_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xEA);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's), s2: the caller's.
    rand_bytes(fx, D + 0x52B4, 0x100, 0x01E0_5B1C, [Some(OT), Some(3), None]);
    0
}

/// Update 108 [F 0x1E05B74]: the arc pair (+0x17E8), points +0x1848.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0x17E8;
    set_counter4(fx, g, 0x12, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x1848);
    fx.mem.write_u8(g + 0x30, 0x28);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F99_999A);
    fx.mem.write_u8(g + 0x34, 0);
    fx.mem.set16(g + 0x36, 0);
    for (o, v) in [(0x47u32, 0x80u8), (0x44, 0x80), (0x45, 0x80), (0x46, 0x80), (0x4A, 0x80), (0x49, 0x80), (0x48, 0x20)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        fx.mem.set16(g + 0xC + 2 * a, -1);
        a += 1;
    }
}

/// How a record throws its bouncing sprites (`bounce_spawn`).
pub(crate) struct Bounce {
    /// The countdown's count; byte +8; +0x10's base; +0x14.
    pub count: i32,
    pub b8: u8,
    pub base: f32,
    pub vy: u32,
    /// The spreads' rand modulus and offset ((rand % m - off) / 100).
    pub spread: (i32, i32),
    /// rand's caller s0 (its frame residue) and return addresses.
    pub s0: u32,
    pub ras: [u32; 5],
}

/// b245's (0x1E05ED8).
const BOUNCE: Bounce = Bounce { count: 0x18, b8: 2, base: 100.0, vy: 0xBF80_0000, spread: (0xB4, 0x5A), s0: OT, ras: [0x01E0_5F00, 0x01E0_5F68, 0x01E0_5FF8, 0x01E0_6044, 0x01E0_60A0] };

fn spread_by(fx: &mut Fx, v: &Bounce, ra: u32) -> f32 {
    let x = fx.rand_from(v.s0, ra) >> 4;
    let (_, h) = fx.div(x, v.spread.0);
    ee::div(ee::from_i32(h - v.spread.1), 100.0)
}

/// 200 bouncing sprites (`sprite::bounce` records, 0x44 bytes at `pool`)
/// thrown up from `at` [F b245 0x1E05ED8]: delay rand & 15, +0x10 = base +
/// (rand % 20 x 200 - 2000) / 100, +0x14 / +0x18 = vy / -0.09, spreads
/// +0x2C..+0x34 = (rand % m - off) / 100 (+0x30 + 0.8).
pub(crate) fn bounce_spawn(fx: &mut Fx, pool: u32, at: u32, v: &Bounce) {
    bounce_spawn_sized(fx, pool, at, v, (0x14, 0x7D0));
}

/// `bounce_spawn` with +0x10 = base + ((rand >> 4) % m x 200 - off) / 100
/// for `size` = (m, off)).
pub(crate) fn bounce_spawn_sized(fx: &mut Fx, pool: u32, at: u32, v: &Bounce, size: (i32, i32)) {
    bounce_spawn_n(fx, pool, at, v, size, (0xC8, 0xC8, 0xF));
}

/// `bounce_spawn_sized` with `shape` = (sprites thrown, pool records
/// searched, delay mask)).
pub(crate) fn bounce_spawn_n(fx: &mut Fx, pool: u32, at: u32, v: &Bounce, size: (i32, i32), shape: (i32, i32, i32)) {
    for _ in 0..shape.0 {
        let i = first_free(fx, pool, 0x44, shape.1);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(v.s0, v.ras[0]) >> 4) & shape.2;
        let b = pool + 0x44 * i as u32;
        set_counter4(fx, b, v.count, delay, 0xFF);
        for (o, x) in [(4u32, 0u8), (8, v.b8), (0xB, 2), (0xA, 0), (9, 3), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(b + o, x);
        }
        let x = fx.rand_from(v.s0, v.ras[1]) >> 4;
        let (_, h) = fx.div(x, size.0);
        let f = ee::div(ee::from_i32(h * 200 - size.1), 100.0);
        fx.mem.setf(b + 0x10, ee::add(v.base, f));
        fx.mem.write_u32(b + 0x14, v.vy);
        fx.mem.write_u32(b + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(b + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        let x = spread_by(fx, v, v.ras[2]);
        fx.mem.setf(b + 0x2C, x);
        let x = spread_by(fx, v, v.ras[3]);
        fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), x));
        let x = spread_by(fx, v, v.ras[4]);
        fx.mem.setf(b + 0x34, x);
    }
}

/// Update 113 [F 0x1E05C34]: at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
    set_counter4(fx, D + 0x52AC, 8, 0, 0xFF);
    fx.mem.write_u8(D + 0x52B0, 1);
    let f = D + 0x1AC8;
    set_counter4(fx, f, 0x20, 0, 0xFF);
    fx.mem.write_u8(f + 5, 0);
    fx.mem.write_u8(f + 4, 0x81);
    fx.mem.write_u8(f + 7, 9);
    fx.mem.write_u32(f + 0x10, 0x447A_0000);
    fx.mem.write_u32(f + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x20), (8, 0x80), (0xF, 0xD0)] {
        fx.mem.write_u8(f + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(f + o, 0);
    }
    copy_v4(fx, at, f + 0x1C);
    for k in 0..3u32 {
        let r = D + 0x68 * k + 0x1C54;
        fx.mem.write_u8(r + 4, 4);
        if k == 0 {
            set_counter4(fx, r, 0x1C, 0, 0xFF);
            let b = fx.mem.read_u8(r + 4) | 1;
            fx.mem.write_u8(r + 4, b);
            fx.mem.write_u8(r + 0xA, 1);
            fx.mem.write_u8(r + 0xC, 3);
            fx.mem.write_u32(r + 0x14, 0x4000_0000);
            fx.mem.write_u32(r + 0x1C, 0x4220_0000);
            fx.mem.write_u8(r + 0x23, 0);
            fx.mem.write_u8(r + 0x27, 0x80);
            fx.mem.write_u8(r + 8, 0xA0);
            fx.mem.write_u32(r + 0x58, 0x40);
            fx.mem.write_u32(r + 0x5C, 0x80);
            fx.mem.write_u32(r + 0x60, 0x100);
        } else {
            set_counter4(fx, r, 0x20, 0, 0xFF);
            let b = fx.mem.read_u8(r + 4) | 2;
            fx.mem.write_u8(r + 4, b);
            fx.mem.write_u8(r + 0xA, 2);
            fx.mem.write_u8(r + 0xC, 5);
            fx.mem.write_u32(r + 0x14, 0x3F99_999A);
            fx.mem.write_u32(r + 0x1C, 0x42A0_0000);
            fx.mem.write_u32(r + 0x18, 0x3ECC_CCCD);
            fx.mem.write_u8(r + 0x23, 0x80);
            fx.mem.write_u8(r + 0x27, 0x10);
            fx.mem.write_u8(r + 8, 0x1E);
            fx.mem.write_u32(r + 0x50, if k & 1 != 0 { 0x6000 } else { 0xE000 });
        }
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xB, 2);
        let x = fx.rand_from(OT, 0x01E0_5E68);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, D + 0x52B4);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
    }
    bounce_spawn(fx, D + 0x1D8C, at, &BOUNCE);
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FC0_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, sp + 0x60);
    // The entry's s1 when 0x1031C0 saves it: the parameter, or the
    // sprite loop's end.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0xF0 => return 8,
        0x71 => {
            strike(fx, sp);
            s1 = 0xC8;
        }
        0x6C => arcs_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_6188, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6198, [Some(u64::from(OT)), Some(s1)]);
    blib::fade::fade(fx, D + 0x52AC, OT);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x20);
    blib::arc::arcs(fx, 0x01E0_15F0, D + 0x17E8, OT, 1);
    blib::star::flash(fx, 0x01E0_2870, D + 0x1AC8, OT, 1);
    blib::cross::bars(fx, 0x01E0_3420, D + 0x1B14, OT, 4);
    blib::streak::ribbon(fx, 0x01E0_3E30, D + 0x1C54, OT, 3);
    blib::sprite::bounce(fx, D + 0x1D8C, OT, 0xC8);
    let n = fx.mem.u16(D);
    if (0x71..0x86).contains(&n) {
        6
    } else if n == 0x6E {
        2
    } else if n == 0x67 {
        1
    } else {
        0
    }
}
