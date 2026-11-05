//! BMODULE b525 (card 351), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03E20, data area a0 = 0x1ED0000) [F] (helper B4, 31
//! Oct 2026): b772's library (s4 = D, s0 = the OT). On update 2 the
//! attacker's node 0xE (+0xC204, its height +0xC208); on updates 2..71 two
//! bouncing sprites a update from each of nodes 0xE / 0x2D (pool of 500
//! at +0x1958, mirrored by the attacker's facing); on 69 a bead-trail
//! emitter (+0xC178, riding node 0xE, beads pool of 40 at +0xB3B8); on 72
//! at the target a fade (+0xA4), a flash (+0xAC), two ribbons (+0xF8) and
//! a camera shake (+0x40); on 83 thirty-six sprites in a ring round node 1;
//! the stage light pulsed by 0x1031C0 (+0xA0: down on update 66, up on
//! 126). Returns 1 on update 62, 6 on 72..100, 2 on 101, 8 on 132 (and at
//! once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `ribbon` 0x1E00FC0, `bounce` 0x1E01B50, `fade` 0x1E025C0,
//! `beads` 0x1E02B00, `bead_trails` 0x1E038D0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0xC214;
const POOL: u32 = D + 0x1958;
const EMITTER: u32 = D + 0xC178;
const BEADS: u32 = D + 0xB3B8;
/// The node 0xE's height (written on update 2).
const HEIGHT: u32 = D + 0xC208;

/// The entry 0x1E03E20 (a0 = D, a1 = param; s4 = D, s0 = the OT, s1 =
/// the parameter).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xC320);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0xE, 0), (0x14, 0x2D, 0), (0x18, 0x41, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 0x42);
    fx.mem.set16(D + 0xA2, 0x7E);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's), s2: the caller's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_3F78, [Some(OT), Some(3), None]);
    0
}

/// (rand >> 4) % m - off, over 100 (rand called from `ra`).
fn spread(fx: &mut Fx, ra: u32, m: i32, off: i32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h - off), 100.0)
}

/// ((rand >> 4) % m) x 200 - off, over 100.
fn spread200(fx: &mut Fx, ra: u32, m: i32, off: i32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h * 200 - off), 100.0)
}

/// (rand >> 4) % m for a power of two m, as the compiler's signed
/// remainder (and, then corrected for a negative value).
fn rem_pow2(fx: &mut Fx, ra: u32, m: i32) -> i32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let mut r = x & (m - 1);
    if x < 0 && r != 0 {
        r -= m;
    }
    r
}

/// Update 69 [F 0x1E04008]: the emitter at node +0x10.
fn emitter_start(fx: &mut Fx, param: i32) {
    let e = EMITTER;
    set_counter4(fx, e, 5, 0, 0xFF);
    fx.mem.write_u8(e + 4, 2);
    fx.mem.write_u8(e + 5, 1);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(e + 6, b);
    fx.mem.write_u32(e + 8, BEADS);
    for (o, v) in [(0x10u32, 0x28u8), (0x12, 0x10), (0x11, 0x20), (0x13, 1), (0x16, 2), (0x14, 4)] {
        fx.mem.write_u8(e + o, v);
    }
    fx.mem.write_u32(e + 0x18, 0x3F80_0000);
    fx.mem.write_u32(e + 0x1C, 0x3F00_0000);
    fx.mem.write_u32(e + 0x20, 0x42C8_0000);
    fx.mem.write_u32(e + 0xC, BYTES);
    for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
        fx.mem.write_u8(e + o, 0x80);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x10);
    node_position_saving(fx, h, n, e + 0x3C, u64::from(OT), Some(i64::from(param) as u64), 0x01E0_4150);
    copy_v4(fx, e + 0x3C, e + 0x4C);
}

/// Update 83 [F 0x1E04188]: 36 sprites in a ring (10 degrees apart) round
/// node +0xC (sp + 0x80).
fn ring_start(fx: &mut Fx, sp: u32, param: i32) {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(OT), Some(i64::from(param) as u64), 0x01E0_419C);
    for k in 0..0x24i32 {
        let i = first_free(fx, POOL, 0x44, 0x1F4);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(OT, 0x01E0_41D0) >> 4) & 7;
        let b = POOL + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, delay, 0xFF);
        let x = fx.rand_from(OT, 0x01E0_4204) >> 4;
        fx.mem.set8(b + 4, (x & 3) << 4);
        fx.mem.write_u8(b + 8, 2);
        fx.mem.write_u8(b + 0xB, 1);
        fx.mem.write_u8(b + 0xA, 5);
        let x = fx.rand_from(OT, 0x01E0_4238) >> 4;
        fx.mem.set8(b + 9, x & 3);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x50)] {
            fx.mem.write_u8(b + o, v);
        }
        let m = rem_pow2(fx, 0x01E0_4268, 8);
        let f = ee::div(ee::from_i32(m * 200 - 0x320), 100.0);
        fx.mem.setf(b + 0x10, ee::add(f32::from_bits(0x4302_0000), f));
        let f = spread200(fx, 0x01E0_42D8, 3, 0x12C);
        fx.mem.setf(b + 0x14, ee::add(6.0, f));
        fx.mem.write_u32(b + 0x18, 0xBCA3_D70A);
        fx.mem.write_u32(b + 0x1C, 0);
        let deg = ee::mul(10.0, ee::from_i32(k));
        let f = spread(fx, 0x01E0_436C, 0xBE, 0x5F);
        let r = ee::add(1.5, f);
        let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), deg), 180.0);
        let c = libm_cosf(fx, a);
        let x = fx.mem.f(sp + 0x80);
        fx.mem.setf(b + 0x20, ee::add(x, ee::mul(r, c)));
        let y = fx.mem.read_u32(HEIGHT);
        fx.mem.write_u32(b + 0x24, y);
        let s = fx.libm_sinf(a);
        let z = fx.mem.f(sp + 0x88);
        fx.mem.setf(b + 0x28, ee::add(z, ee::mul(r, s)));
        let f = spread(fx, 0x01E0_4438, 0x50, 0x28);
        let v = ee::add(0.5, f);
        let c = libm_cosf(fx, a);
        fx.mem.setf(b + 0x2C, ee::mul(v, c));
        let s = fx.libm_sinf(a);
        fx.mem.setf(b + 0x34, ee::mul(v, s));
        fx.mem.write_u32(b + 0x30, 0x3ECC_CCCD);
    }
}

/// Update 72 [F 0x1E044DC]: at the target (sp + 0x70).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x70;
    set_counter4(fx, D + 0xA4, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0xA8, 1);
    let g = D + 0xAC;
    set_counter4(fx, g, 0x20, 0, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x80);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x4396_0000);
    fx.mem.write_u32(g + 0x14, 0x41F0_0000);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0), (8, 0x40), (0xF, 0xF0)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    copy_v4(fx, at, g + 0x1C);
    fx.mem.write_u32(g + 0x18, 0x41F0_0000);
    for k in 0..2u32 {
        let r = D + 0xF8 + 0x68 * k;
        if k != 0 {
            set_counter4(fx, r, 0x1E, 0, 0xFF);
            fx.mem.write_u8(r + 4, 5);
            fx.mem.write_u8(r + 0xA, 0);
            fx.mem.write_u8(r + 0xC, 3);
            fx.mem.write_u32(r + 0x14, 0x4000_0000);
            fx.mem.write_u32(r + 0x1C, 0x4220_0000);
            fx.mem.write_u8(r + 0x23, 0);
            fx.mem.write_u8(r + 0x27, 0x60);
            fx.mem.write_u8(r + 8, 0xA0);
            fx.mem.set32(r + 0x58, 0x40);
            fx.mem.set32(r + 0x5C, 0x80);
            fx.mem.set32(r + 0x60, 0x100);
        } else {
            set_counter4(fx, r, 0x20, 0, 0xFF);
            fx.mem.write_u8(r + 4, 7);
            fx.mem.write_u8(r + 0xA, 1);
            fx.mem.write_u8(r + 0xC, 3);
            fx.mem.write_u32(r + 0x14, 0x3FC0_0000);
            fx.mem.write_u32(r + 0x1C, 0x42F0_0000);
            fx.mem.write_u8(r + 0x23, 0x80);
            fx.mem.write_u8(r + 0x27, 0);
            fx.mem.write_u8(r + 8, 0x78);
        }
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xB, 2);
        let x = fx.rand_from(OT, 0x01E0_46D4);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}

/// Updates 2..71 [F 0x1E047DC]: two sprites a update from each of nodes
/// +0x10 / +0x14 (sp + 0x80).
fn rise(fx: &mut Fx, sp: u32) {
    for j in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0x10 + 4 * j);
        node_position_saving(fx, h, n, sp + 0x80, u64::from(OT), Some(u64::from(j)), 0x01E0_4808);
        for _ in 0..2 {
            let i = first_free(fx, POOL, 0x44, 0x1F4);
            if i < 0 {
                continue;
            }
            let b = POOL + 0x44 * i as u32;
            set_counter4(fx, b, 0x20, 0, 0xFF);
            let x = fx.rand_from(OT, 0x01E0_4860) >> 4;
            fx.mem.set8(b + 4, (x & 3) << 4);
            fx.mem.write_u8(b + 8, 2);
            for (o, v) in [(0xEu32, 0x80u8), (0xD, 0xA0), (0xC, 0xC0), (0xF, 0x60)] {
                fx.mem.write_u8(b + o, v);
            }
            let f = spread200(fx, 0x01E0_48A4, 0x1E, 0xBB8);
            fx.mem.setf(b + 0x10, ee::add(150.0, f));
            let m = rem_pow2(fx, 0x01E0_4910, 2);
            let f = ee::div(ee::from_i32(m * 200 - 0xC8), 100.0);
            fx.mem.setf(b + 0x14, ee::add(6.0, f));
            fx.mem.write_u8(b + 0xB, 1);
            fx.mem.write_u8(b + 0xA, 5);
            let x = fx.rand_from(OT, 0x01E0_4990) >> 4;
            fx.mem.set8(b + 9, x & 3);
            fx.mem.write_u32(b + 0x18, 0);
            fx.mem.write_u32(b + 0x1C, 0);
            for (c, ra) in [0x01E0_49B0u32, 0x01E0_4A00, 0x01E0_4A50].into_iter().enumerate() {
                let f = spread(fx, ra, 0x190, 0xC8);
                let p = fx.mem.f(sp + 0x80 + 4 * c as u32);
                fx.mem.setf(b + 0x20 + 4 * c as u32, ee::add(p, f));
            }
            let f = spread(fx, 0x01E0_4AA0, 0x1E, 0xF);
            fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3E4C_CCCD), f));
            let f = spread(fx, 0x01E0_4AFC, 0x3C, 0x1E);
            fx.mem.setf(b + 0x2C, f);
            let v = if fx.mem.u16(D) < 0x3E {
                let f = spread(fx, 0x01E0_4B54, 0xB4, 0x5A);
                ee::add(f32::from_bits(0x3FA6_6666), f)
            } else {
                spread(fx, 0x01E0_4BB8, 0x1E, 0xF)
            };
            fx.mem.setf(b + 0x34, v);
            if fx.mem.read_u32(D + 0x30) != 0x8000 {
                let v = fx.mem.f(b + 0x34);
                fx.mem.setf(b + 0x34, ee::mul(v, -1.0));
            }
        }
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x70, u64::from(OT), 0x01E0_3FA0);
    // The entry's s1 when 0x1031C0 saves it: the parameter or a loop's end.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0x84 => return 8,
        0x48 => {
            strike(fx, sp);
            s1 = 2;
        }
        0x53 => {
            ring_start(fx, sp, param);
            s1 = 0x24;
        }
        0x45 => emitter_start(fx, param),
        2 => {
            let h = fx.mem.i32(D + 4);
            let n = fx.mem.i32(D + 0x10);
            node_position_saving(fx, h, n, D + 0xC204, u64::from(OT), Some(i64::from(param) as u64), 0x01E0_4000);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0x48).contains(&n) {
        rise(fx, sp);
        s1 = 2;
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_4C54, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4C64, [Some(u64::from(OT)), Some(s1)]);
    blib::fade::fade(fx, D + 0xA4, OT);
    blib::star::flash(fx, 0x01E0_0410, D + 0xAC, OT, 1);
    blib::streak::ribbon(fx, 0x01E0_0FC0, D + 0xF8, OT, 0x3C);
    blib::sprite::bounce(fx, POOL, OT, 0x1F4);
    blib::emit::bead_trails(fx, 0x01E0_38D0, EMITTER, 1);
    blib::beam::beads(fx, 0x01E0_2B00, BEADS, OT, 0x28);
    let n = fx.mem.u16(D);
    if (0x48..0x65).contains(&n) {
        6
    } else if n == 0x65 {
        2
    } else if n == 0x3E {
        1
    } else {
        0
    }
}
