//! BMODULE b748 (card 265), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E050B0, data area a0 = 0x1ED0000) [F] (helper C10,
//! 31 Oct 2026): b367's program (the same library at the same addresses
//! and the same data layout) without the tint and the first frame fade:
//! on update 2 the position of the attacker's node 0x36 (+0x6564); two
//! bouncing sprites a update from each of nodes 0x36 / 0x4A on 64..70,
//! 93..97, 111..117; on 58, 80, 105, 127 thirty-six sprites in a ring
//! round node 1; on 124 at the target the frame fade (+0x64FC), the
//! streaks, five sheets (3 apart), two ribbons, a flash and a shake; the
//! stage light pulsed by 0x1031C0 (+0xA0, after the shake). The arcs
//! (+0x6174) are stepped but never set. Returns 1 on update 114, 6 on
//! 124..152, 2 on 153, 8 on 170 (and at once for -2 / -3).
//!
//! Library (record addresses): as [`crate::b367`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x6574;
const POOL: u32 = D + 0x11C4;
const FADE: u32 = D + 0x64FC;
/// Node 0x36's height (written on update 2).
const HEIGHT: u32 = D + 0x6568;

/// The entry 0x1E050B0 (a0 = D, a1 = param; s4 = D, s0 = the OT, s1 =
/// the parameter).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    // swc1 f20 / f21: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
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
    fx.bzero(D, 0x6680);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x13, 0), (0x14, 0x12, 0), (0x18, 0x36, 0), (0x1C, 0x4A, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xA4);
    // The entry's s0 = the OT, s1 = 4 (the texture loop's), s2: the caller's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_521C, [Some(OT), Some(4), None]);
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

/// Update 2 [F 0x1E052A4]: the position of node +0x18.
fn start(fx: &mut Fx, param: i32) {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x18);
    node_position_saving(fx, h, n, D + 0x6564, u64::from(OT), Some(i64::from(param) as u64), 0x01E0_52B8);
}

/// Updates 58 / 80 / 105 / 127 [F 0x1E052C0]: 36 sprites in a ring
/// (10 degrees apart) round node +0xC (sp + 0x80).
fn ring(fx: &mut Fx, sp: u32, param: i32) {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(OT), Some(i64::from(param) as u64), 0x01E0_52D4);
    for k in 0..0x24i32 {
        let i = first_free(fx, POOL, 0x44, 0x12C);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(OT, 0x01E0_5308) >> 4) & 7;
        let b = POOL + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, delay, 0xFF);
        let x = fx.rand_from(OT, 0x01E0_533C) >> 4;
        fx.mem.set8(b + 4, (x & 3) << 4);
        fx.mem.write_u8(b + 8, 0);
        fx.mem.write_u8(b + 0xB, 1);
        fx.mem.write_u8(b + 0xA, 5);
        let x = fx.rand_from(OT, 0x01E0_536C) >> 4;
        fx.mem.set8(b + 9, x & 3);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x50)] {
            fx.mem.write_u8(b + o, v);
        }
        let f = spread200(fx, 0x01E0_539C, 0xA, 0x3E8);
        fx.mem.setf(b + 0x10, ee::add(150.0, f));
        let f = spread200(fx, 0x01E0_5408, 3, 0x12C);
        fx.mem.setf(b + 0x14, ee::add(8.0, f));
        fx.mem.write_u32(b + 0x18, 0xBCA3_D70A);
        let y = fx.mem.read_u32(HEIGHT);
        fx.mem.write_u32(b + 0x1C, y);
        let deg = ee::mul(10.0, ee::from_i32(k));
        let f = spread(fx, 0x01E0_54A0, 0xBE, 0x5F);
        let r = ee::add(3.0, f);
        let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), deg), 180.0);
        let c = libm_cosf(fx, a);
        let x = fx.mem.f(sp + 0x80);
        fx.mem.setf(b + 0x20, ee::add(x, ee::mul(r, c)));
        let y = fx.mem.read_u32(HEIGHT);
        fx.mem.write_u32(b + 0x24, y);
        let s = fx.libm_sinf(a);
        let z = fx.mem.f(sp + 0x88);
        fx.mem.setf(b + 0x28, ee::add(z, ee::mul(r, s)));
        let f = spread(fx, 0x01E0_5564, 0x50, 0x28);
        let v = ee::add(f32::from_bits(0x3ECC_CCCD), f);
        let c = libm_cosf(fx, a);
        fx.mem.setf(b + 0x2C, ee::mul(v, c));
        let s = fx.libm_sinf(a);
        fx.mem.setf(b + 0x34, ee::mul(v, s));
        fx.mem.write_u32(b + 0x30, 0x3ECC_CCCD);
    }
}

/// Update 124 [F 0x1E0560C]: the frame fade, the streaks, five
/// sheets, two ribbons, a flash and a shake at the target (sp + 0x70).
/// Returns the entry's s1 afterwards.
fn strike(fx: &mut Fx, sp: u32) -> u64 {
    let at = sp + 0x70;
    fx.mem.set16(FADE, 0x7C);
    let e = fx.mem.u16(FADE) + 0x28;
    fx.mem.set16(FADE + 2, e);
    fx.mem.write_u8(FADE + 5, 6);
    fx.mem.write_u8(FADE + 6, 2);
    fx.mem.write_u32(FADE + 8, 0x4180_0000);
    for (o, v) in [(0x14u32, -8), (0xC, -8), (0x18, 8), (0x10, 8), (0x20, -8), (0x1C, -8), (0x28, 8), (0x24, 8)] {
        fx.mem.set32(FADE + o, v);
    }
    for (o, v) in [(0x2Eu32, 0x80u8), (0x2D, 0x20), (0x2C, 0)] {
        fx.mem.write_u8(FADE + o, v);
    }
    // The streaks (+0x652C).
    let s = D + 0x652C;
    set_counter4(fx, s, 0x28, 0, 0xFF);
    for (o, v) in [(4u32, 0x80u8), (9, 1), (0xA, 0xFF), (8, 0), (0xB, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(s + 0xC, h);
    for (o, v) in [(0x14u32, 0x80u8), (0x15, 0x80), (0x16, 0x80), (0x17, 0x60), (0x10, 0x80), (0x11, 0x80), (0x12, 0x80), (0x13, 0x60), (5, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, b);
    // Five sheets (+0x1D4).
    for k in 0..5i32 {
        let i = first_free(fx, D + 0x1D4, 0x3C, 6);
        if i < 0 {
            continue;
        }
        let r = D + 0x1D4 + 0x3C * i as u32;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 1), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        fx.mem.write_u32(r + 0x10, 0x4040_0000);
        fx.mem.write_u32(r + 0x14, 0x4000_0000);
        fx.mem.write_u32(r + 0x18, if k == 2 { 0x4040_0000 } else { 0x4000_0000 });
        let d = ee::add(-6.0, ee::mul(3.0, ee::from_i32(k)));
        let x = ee::add(fx.mem.f(r + 0x2C), d);
        fx.mem.setf(r + 0x2C, x);
        fx.mem.set32(r + 0x24, -0x2800);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let v = fx.mem.i32(r + 0x24);
            fx.mem.set32(r + 0x24, v.wrapping_neg());
        }
    }
    // Two ribbons (+0x594).
    for k in 0..2u32 {
        let r = D + 0x594 + 0x68 * k;
        set_counter4(fx, r, 0x1C, 4, 0xFF);
        if k != 0 {
            fx.mem.write_u8(r + 4, 5);
            fx.mem.write_u8(r + 0xA, 3);
            fx.mem.write_u8(r + 0xC, 5);
            fx.mem.write_u32(r + 0x14, 0x4000_0000);
            fx.mem.write_u32(r + 0x1C, 0x4220_0000);
            fx.mem.write_u8(r + 0x23, 0);
            fx.mem.write_u8(r + 0x27, 0x60);
            fx.mem.write_u8(r + 8, 0xA0);
            fx.mem.set32(r + 0x58, 0x40);
            fx.mem.set32(r + 0x5C, 0x80);
            fx.mem.set32(r + 0x60, 0x100);
        } else {
            fx.mem.write_u8(r + 4, 7);
            fx.mem.write_u8(r + 0xA, 2);
            fx.mem.write_u8(r + 0xC, 3);
            fx.mem.write_u32(r + 0x14, 0x3FC0_0000);
            fx.mem.write_u32(r + 0x1C, 0x42B4_0000);
            fx.mem.write_u8(r + 0x23, 0x80);
            fx.mem.write_u8(r + 0x27, 0x20);
            fx.mem.write_u8(r + 8, 0x78);
        }
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xB, 2);
        let x = fx.rand_from(OT, 0x01E0_5974);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
    }
    // A flash (+0xA4 pool of 4); s1 = its offset.
    let mut s1 = 2u64;
    let i = first_free(fx, D + 0xA4, 0x4C, 4);
    if i >= 0 {
        s1 = (0x4C * i) as u64;
        let r = D + 0xA4 + 0x4C * i as u32;
        set_counter4(fx, r, 0x22, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 7, 9);
        fx.mem.write_u8(r + 4, 0x81);
        fx.mem.write_u32(r + 0x10, 0x4496_0000);
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x18, 0);
        for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0), (9, 0), (8, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(r + o, 0);
        }
        copy_v4(fx, at, r + 0x1C);
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
    unit_position_saving(fx, t, D + 0x60, u64::from(OT), 0x01E0_5B08);
    s1
}

/// Updates 64..70, 93..97, 111..117 [F 0x1E05B68]: two sprites a
/// update from each of nodes +0x18 / +0x1C (sp + 0x80).
fn rise(fx: &mut Fx, sp: u32) {
    for j in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0x18 + 4 * j);
        node_position_saving(fx, h, n, sp + 0x80, u64::from(OT), Some(u64::from(j)), 0x01E0_5B90);
        for _ in 0..2 {
            let i = first_free(fx, POOL, 0x44, 0x12C);
            if i < 0 {
                continue;
            }
            let b = POOL + 0x44 * i as u32;
            set_counter4(fx, b, 0x1C, 0, 0xFF);
            let x = fx.rand_from(OT, 0x01E0_5BE8) >> 4;
            fx.mem.set8(b + 4, (x & 3) << 4);
            fx.mem.write_u8(b + 8, 0);
            for (o, v) in [(0xEu32, 0x80u8), (0xD, 0xA0), (0xC, 0xC0), (0xF, 0x60)] {
                fx.mem.write_u8(b + o, v);
            }
            let f = spread200(fx, 0x01E0_5C28, 0x1E, 0xBB8);
            fx.mem.setf(b + 0x10, ee::add(150.0, f));
            let x = fx.rand_from(OT, 0x01E0_5C94) >> 4;
            let mut m = x & 1;
            if x < 0 && m != 0 {
                m -= 2;
            }
            let f = ee::div(ee::from_i32(m * 200 - 0xC8), 100.0);
            fx.mem.setf(b + 0x14, ee::add(6.0, f));
            fx.mem.write_u8(b + 0xB, 1);
            fx.mem.write_u8(b + 0xA, 5);
            let x = fx.rand_from(OT, 0x01E0_5D14) >> 4;
            fx.mem.set8(b + 9, x & 3);
            fx.mem.write_u32(b + 0x18, 0);
            fx.mem.write_u32(b + 0x1C, 0);
            for (c, ra, m, off) in [(0u32, 0x01E0_5D34u32, 0x190, 0xC8), (4, 0x01E0_5D84, 0x190, 0xC8), (8, 0x01E0_5DD4, 0x190, 0xC8)] {
                let f = spread(fx, ra, m, off);
                let p = fx.mem.f(sp + 0x80 + c);
                fx.mem.setf(b + 0x20 + c, ee::add(p, f));
            }
            let f = spread(fx, 0x01E0_5E24, 0x1E, 0xF);
            fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3E4C_CCCD), f));
            let f = spread(fx, 0x01E0_5E80, 0x3C, 0x1E);
            fx.mem.setf(b + 0x2C, f);
            let f = spread(fx, 0x01E0_5ECC, 0x1E, 0xF);
            fx.mem.setf(b + 0x34, f);
        }
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x70, u64::from(OT), 0x01E0_5244);
    // The entry's s1 when 0x1031C0 saves it: the parameter or a loop's.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0xAA => return 8,
        0x7C => s1 = strike(fx, sp),
        0x7F | 0x69 | 0x50 | 0x3A => {
            ring(fx, sp, param);
            s1 = 0x24;
        }
        2 => start(fx, param),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x40..0x47).contains(&n) || (0x5D..0x62).contains(&n) || (0x6F..0x76).contains(&n) {
        rise(fx, sp);
        s1 = 2;
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_5F44, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5F54, [Some(u64::from(OT)), Some(s1)]);
    blib::star::flash(fx, 0x01E0_0410, D + 0xA4, OT, 4);
    blib::sheet::sheets(fx, 0x01E0_0FC0, D + 0x1D4, OT, 0x10);
    blib::sprite::bounce(fx, POOL, OT, 0x12C);
    blib::streak::ribbon(fx, 0x01E0_27B0, D + 0x594, OT, 0x1E);
    let (_, arcs_a3) = blib::arc::arcs_a3(fx, 0x01E0_1530, D + 0x6174, OT, 1);
    let (_, streaks_a3) = blib::radiate::streaks_a3(fx, 0x01E0_3DB0, D + 0x652C, OT, 1);
    // a3 = what streaks left, else what arcs left [H].
    let n = fx.mem.u16(D);
    blib::window::frame_fade(fx, 0x01E0_4BD0, FADE, OT, n, streaks_a3.unwrap_or(arcs_a3));
    let n = fx.mem.u16(D);
    if (0x7C..0x99).contains(&n) {
        6
    } else if n == 0x99 {
        2
    } else if n == 0x72 {
        1
    } else {
        0
    }
}
