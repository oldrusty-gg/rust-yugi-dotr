//! BMODULE b532 (card 419), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03D50, data area a0 = 0x1ED0000) [F] (helper C10,
//! 31 Oct 2026): b525's family (s4 = D, s0 = the OT, s1 = the parameter
//! or a loop's). On update 12 the streaks (+0x158) from node 1 (their far
//! alpha +0x13 rising by 2 a update to 0xA0 while they count); on 3..80
//! two bouncing sprites a update from each of nodes 0xB / 0x12 (pool of
//! 500 at +0x470); on 80 at the target a coloured fade (+0x8940,
//! `fade::fade_rgb`), four bands (+0x190), a ribbon (+0xA4), a flash
//! (+0x10C) and 300 sprites bursting from it; the stage light pulsed by
//! 0x1031C0 (+0xA0). Returns 1 on update 70, 2 on 77, 6 on 80..108, 8 on
//! 130 (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `ribbon` 0x1E00080, `flash` 0x1E00C10,
//! `streaks` 0x1E017C0, `fade_rgb` 0x1E025E0, `bounce` 0x1E02AF0, `bands`
//! 0x1E03560 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x894C;
const POOL: u32 = D + 0x470;
const FADE: u32 = D + 0x8940;
const STREAKS: u32 = D + 0x158;

/// The entry 0x1E03D50 (a0 = D, a1 = param; s4 = D, s0 = the OT, s1 =
/// the parameter).
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
    fx.bzero(D, 0x8A50);
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
    for (at, node, clear) in [(0xCu32, 0xB, 1), (0x10, 0x12, 0), (0x14, 1, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x7C);
    // The entry's s0 = the OT, s1 = 4 (the texture loop's), s2: the caller's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_3E88, [Some(OT), Some(4), None]);
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

/// Update 12 [F 0x1E03EE0]: the streaks from node +0x14.
fn streaks_start(fx: &mut Fx) {
    let s = STREAKS;
    set_counter4(fx, s, 0x50, 0, 0xFF);
    for (o, v) in [(4u32, 0x80u8), (9, 2), (0xA, 0xFF), (8, 0x14), (0xB, 7)] {
        fx.mem.write_u8(s + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(s + 0xC, h);
    for (o, v) in [(0x16u32, 0x80u8), (0x15, 0x20), (0x14, 0x20), (0x17, 0x10), (0x12, 0xF0), (0x11, 0x40), (0x10, 0x40), (0x13, 0), (5, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(s + 6, b);
}

/// Update 80 [F 0x1E03F6C]: at the target (sp + 0x60) the coloured fade,
/// four bands, a ribbon, a flash and 300 sprites. Returns the entry's s1.
fn strike(fx: &mut Fx, sp: u32) -> u64 {
    let at = sp + 0x60;
    set_counter4(fx, FADE, 0x18, 0, 0xFF);
    for (o, v) in [(0xAu32, 0xFFu8), (9, 0x20), (8, 0), (4, 0x82)] {
        fx.mem.write_u8(FADE + o, v);
    }
    for k in 0..4u32 {
        let r = D + 0x190 + 0x5C * k;
        set_counter4(fx, r, 0x20, (2 * k) as i32 & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 1), (8, 3), (9, 2), (0xA, 6), (0xB, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        for (o, v) in [(0x10u32, 0x3F80_0000u32), (0x14, 0), (0x18, 0x4000_0000), (0x1C, 0x3F80_0000), (0x20, 0x3F00_0000)] {
            fx.mem.write_u32(r + o, v);
        }
        for (o, v) in [(0x28u32, 0x80u8), (0x29, 0x80), (0x2A, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0), (0x2B, 0x40)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        fx.mem.set32(r + 0x3C, 0x4000);
    }
    let r = D + 0xA4;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4060_0000);
    fx.mem.write_u32(r + 0x1C, 0x4270_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 4), (8, 0x8C)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_4114);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x20u32, 0x21, 0x22, 0x24, 0x25, 0x26] {
        fx.mem.write_u8(r + o, 0x80);
    }
    fx.mem.write_u8(r + 0x23, 0x10);
    fx.mem.write_u8(r + 0x27, 0x60);
    copy_v4(fx, at, r + 0x28);
    fx.mem.set32(r + 0x58, 0x40);
    fx.mem.set32(r + 0x5C, 0x80);
    fx.mem.set32(r + 0x60, 0x100);
    let g = D + 0x10C;
    fx.mem.write_u8(g + 5, 0);
    set_counter4(fx, g, 0x20, 0, 0xFF);
    for (o, v) in [(4u32, 0x81u8), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0xC0), (9, 0x40), (8, 0)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 0x14, 0);
    fx.mem.write_u32(g + 0x10, 0x4448_0000);
    copy_v4(fx, at, g + 0x1C);
    for _ in 0..0x12C {
        let i = first_free(fx, POOL, 0x44, 0x1F4);
        if i < 0 {
            continue;
        }
        let delay = (fx.rand_from(OT, 0x01E0_4248) >> 4) & 0xF;
        let b = POOL + 0x44 * i as u32;
        set_counter4(fx, b, 0x20, delay, 0xFF);
        let x = fx.rand_from(OT, 0x01E0_427C) >> 4;
        fx.mem.set8(b + 4, (x & 3) << 4);
        fx.mem.write_u8(b + 8, 1);
        fx.mem.write_u8(b + 0xB, 2);
        fx.mem.write_u8(b + 0xA, 0);
        let x = fx.rand_from(OT, 0x01E0_42AC) >> 4;
        fx.mem.set8(b + 9, (x + 4) & 7);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(b + o, 0x80);
        }
        let f = spread200(fx, 0x01E0_42DC, 0x1E, 0xBB8);
        fx.mem.setf(b + 0x10, ee::add(150.0, f));
        fx.mem.write_u32(b + 0x14, 0x40A0_0000);
        fx.mem.write_u32(b + 0x1C, 0);
        fx.mem.write_u32(b + 0x18, 0);
        for c in [0u32, 4, 8] {
            let v = fx.mem.read_u32(at + c);
            fx.mem.write_u32(b + 0x20 + c, v);
        }
        for (o, ra) in [(0x2Cu32, 0x01E0_4370u32), (0x30, 0x01E0_43BC), (0x34, 0x01E0_4408)] {
            let f = spread(fx, ra, 0x1F4, 0xFA);
            fx.mem.setf(b + o, f);
        }
    }
    0x12C
}

/// Updates 3..80 [F 0x1E04490]: two sprites a update from each of nodes
/// +0xC / +0x10 (sp + 0x70).
fn rise(fx: &mut Fx, sp: u32) {
    for j in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0xC + 4 * j);
        node_position_saving(fx, h, n, sp + 0x70, u64::from(OT), Some(u64::from(j)), 0x01E0_44B4);
        for _ in 0..2 {
            let i = first_free(fx, POOL, 0x44, 0x1F4);
            if i < 0 {
                continue;
            }
            let b = POOL + 0x44 * i as u32;
            set_counter4(fx, b, 0x20, 0, 0xFF);
            let x = fx.rand_from(OT, 0x01E0_4510) >> 4;
            fx.mem.set8(b + 4, (x & 3) << 4);
            fx.mem.write_u8(b + 8, 2);
            for (o, v) in [(0xEu32, 0x80u8), (0xD, 0xA0), (0xC, 0xC0), (0xF, 0x60)] {
                fx.mem.write_u8(b + o, v);
            }
            let f = spread200(fx, 0x01E0_4554, 0x1E, 0xBB8);
            fx.mem.setf(b + 0x10, ee::add(90.0, f));
            let f = spread200(fx, 0x01E0_45C0, 5, 0x1F4);
            fx.mem.setf(b + 0x14, ee::add(10.0, f));
            fx.mem.write_u8(b + 0xB, 1);
            fx.mem.write_u8(b + 0xA, 5);
            let x = fx.rand_from(OT, 0x01E0_4638) >> 4;
            fx.mem.set8(b + 9, x & 3);
            fx.mem.write_u32(b + 0x1C, 0);
            fx.mem.write_u32(b + 0x18, 0);
            for (c, ra) in [(0u32, 0x01E0_4658u32), (4, 0x01E0_46A8), (8, 0x01E0_46F8)] {
                let f = spread(fx, ra, 0x190, 0xC8);
                let p = fx.mem.f(sp + 0x70 + c);
                fx.mem.setf(b + 0x20 + c, ee::add(p, f));
            }
            let f = spread(fx, 0x01E0_4748, 0x1E, 0xF);
            fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3E4C_CCCD), f));
            let f = spread(fx, 0x01E0_47A4, 0x3C, 0x1E);
            fx.mem.setf(b + 0x2C, f);
            let f = spread(fx, 0x01E0_47F0, 0x3C, 0x1E);
            fx.mem.setf(b + 0x34, f);
        }
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(OT), 0x01E0_3EB0);
    // The entry's s1 when 0x1031C0 saves it: the parameter or a loop's.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0x82 => return 8,
        0x50 => s1 = strike(fx, sp),
        0xC => streaks_start(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (3..0x51).contains(&n) {
        rise(fx, sp);
        s1 = 2;
    }
    if fx.mem.u8(STREAKS) > 0 {
        let v = fx.mem.u8(STREAKS + 0x13);
        if v < 0xA0 {
            fx.mem.set8(STREAKS + 0x13, v + 2);
        }
    }
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_488C, [Some(u64::from(OT)), Some(s1)]);
    blib::fade::fade_rgb(fx, FADE, OT);
    // fade_rgb's 0xF0 frame saves the runner's s6 (0) at +0x70 and ra at
    // +0x80: the streaks' first TEXFLUSH reads them as vertex 1's / 2's
    // stale XYZ (ribbon and flash leave those words alone) [run,
    // store-watched].
    fx.saved(sp - 0xF0, &[(0x70, 0), (0x80, 0x01E0_48A0)]);
    blib::streak::ribbon(fx, 0x01E0_0080, D + 0xA4, OT, 1);
    blib::star::flash(fx, 0x01E0_0C10, D + 0x10C, OT, 1);
    blib::radiate::streaks(fx, 0x01E0_17C0, STREAKS, OT, 1);
    blib::band::bands(fx, 0x01E0_3560, D + 0x190, OT, 8);
    blib::sprite::bounce(fx, POOL, OT, 0x1F4);
    let n = fx.mem.u16(D);
    if (0x50..0x6D).contains(&n) {
        6
    } else if n == 0x4D {
        2
    } else if n == 0x46 {
        1
    } else {
        0
    }
}
