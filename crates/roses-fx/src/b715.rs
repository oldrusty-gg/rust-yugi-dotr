//! BMODULE b715 (card 148), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04B60, data area a0 = 0x1ED0000, 0x8410 bytes)
//! [F]: s0 = D, s1 = the OT (0x206E40), s2 the
//! parameter then counters / pointers; compiled without scheduling (nop
//! delay slots). Update 2 reads node 0xB (+0x8300, y - 5), starts two
//! smears (+0x8070, `smear::smears`, nodes 0x45 -> 0x44 and 0x47 -> 0x44,
//! pool +0x8100), the frame fade (+0x8008, `window::frame_fade_b`, updates
//! 2..52) and the attacker's streaks (+0x8038, `radiate::streaks_b`); 51
//! restarts the smears' histories; 127 a second frame fade (128..219) and
//! streaks; 128 and 141 the strike at the target: the fade (+0x6AE4),
//! eight bands (+0x7768, `band::bands_b`, delayed 0..3, turned +-0x4000),
//! two spikes (+0x6B38), a flash (+0x6AEC) and a shake; 155 first 400
//! bouncing sprites (+0xA4) from the node's height to the target, falling
//! through into the strike. The record's object-op table (0x1E05AF0) is
//! walked every update. Returns 6 on updates 128..191, 2 on 192, 1 on 118,
//! 8 on 260 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `bands_b`
//! 0x1E003A0, `flash_b` 0x1E00AE0, `drops` 0x1E01550, `fade` 0x1E01EB0,
//! `spikes` 0x1E02380, `smears` 0x1E02E60, `streaks_b` 0x1E038E0,
//! `frame_fade_b` 0x1E046E0 [`crate::blib`].
use crate::b035::{light_ramp, object_ops};
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const DROPS: u32 = D + 0xA4;
const FADE: u32 = D + 0x6AE4;
const FLASH: u32 = D + 0x6AEC;
const SPIKES: u32 = D + 0x6B38;
const BANDS: u32 = D + 0x7768;
const WINDOW: u32 = D + 0x8008;
const STREAKS: u32 = D + 0x8038;
const SMEARS: u32 = D + 0x8070;
const POOL: u32 = D + 0x8100;
const BYTES: u32 = D + 0x8310;
const OPS: u32 = 0x01E0_5AF0;

/// The entry 0x1E04B60 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x8410).
    fx.bzero(D, 0x8410);
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
    for (at, node, clear) in [(0xCu32, 0xB, 1), (0x10, 0x1A, 0), (0x14, 0x45, 0), (0x18, 0x47, 0), (0x1C, 0x44, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xFE);
    // The entry's s0 = D, s1 = 3 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4C8C, [Some(D), Some(3), Some(param as u32)]);
    0
}

fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 4
}

/// ((rand >> 4) % m - half) / 100.
fn jit(fx: &mut Fx, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

/// A smear's history cleared (-1 x (+0x1F + 1)) and its w -1.0.
fn clear_history(fx: &mut Fx, r: u32) {
    let len = fx.mem.u8(r + 0x1F) as u32;
    for k in 0..=len {
        fx.mem.set16(r + 0xC + 2 * k, -1);
    }
    fx.mem.write_u32(r + 0x30, 0xBF80_0000);
}

/// The frame fade over updates `from..to` (alpha step `a` +5, `b` +6,
/// length `len` +8) and the attacker's streaks (count `count`, far / near
/// alpha `alpha`).
fn window_streaks(fx: &mut Fx, from: i32, to: i32, a: u8, b: u8, len: u32, count: i32, alpha: u8) {
    let w = WINDOW;
    fx.mem.set16(w, from);
    fx.mem.set16(w + 2, to);
    fx.mem.write_u8(w + 5, a);
    fx.mem.write_u8(w + 6, b);
    fx.mem.write_u32(w + 8, len);
    for (o, v) in [(0x14u32, -8i32), (0xC, -8), (0x18, 8), (0x10, 8), (0x20, -8), (0x1C, -8), (0x28, 8), (0x24, 8)] {
        fx.mem.set32(w + o, v);
    }
    for o in [0x2Eu32, 0x2D, 0x2C] {
        fx.mem.write_u8(w + o, 0x80);
    }
    let s = STREAKS;
    set_counter4(fx, s, count, 0, 0xFF);
    for (o, v) in [(4u32, 0x80u8), (9, 1), (0xA, 0xFF), (8, 0), (0xB, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(s + 0xC, h);
    for (o, v) in [(0x14u32, 0x80u8), (0x15, 0x80), (0x16, 0x80), (0x17, alpha), (0x10, 0x80), (0x11, 0x80), (0x12, 0x80), (0x13, alpha), (5, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let v = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(s + 6, v);
}

/// Update 2 [F 0x1E04D0C].
fn start(fx: &mut Fx) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, s, D + 0x8300, u64::from(D), Some(u64::from(OT)), 0x01E0_4D20);
    let y = ee::sub(fx.mem.f(D + 0x8304), 5.0);
    fx.mem.setf(D + 0x8304, y);
    for s2 in 0..2u32 {
        let r = SMEARS + 0x48 * s2;
        set_counter4(fx, r, 0xB1, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let a = if s2 == 0 { 0x14u32 } else { 0x18 };
        let v = fx.mem.read_u8(D + a);
        fx.mem.write_u8(r + 0x21, v);
        let v = fx.mem.read_u8(D + 0x1C);
        fx.mem.write_u8(r + 0x22, v);
        fx.mem.write_u32(r + 8, POOL);
        fx.mem.write_u8(r + 0x1E, 0x20);
        fx.mem.write_u8(r + 0x1F, 8);
        fx.mem.write_u8(r + 0x20, 2);
        fx.mem.write_u32(r + 0x2C, 0x4220_0000);
        fx.mem.write_u32(r + 0x28, 0x3F00_0000);
        for o in [0x34u32, 0x35, 0x36] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x37, 0x60);
        clear_history(fx, r);
    }
    window_streaks(fx, 2, 0x34, 7, 2, 0x3F80_0000, 0x32, 0x60);
}

/// Update 155 [F 0x1E052E4]: 400 bouncing sprites (delayed s % 30) from
/// the node's height to the target.
fn sprites(fx: &mut Fx, sp: u32) {
    for s3 in 0..0x190i32 {
        let i = first_free(fx, DROPS, 0x44, 0x190);
        if i < 0 {
            continue;
        }
        let count = ((rand4(fx, 0x01E0_5308) & 0xF) + 0x20) & 0xFF;
        let q = DROPS + 0x44 * i as u32;
        let (_, delay) = fx.div(s3, 0x1E);
        set_counter4(fx, q, count, delay & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0x42u8), (8, 2), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(q + o, v);
        }
        let v = (rand4(fx, 0x01E0_5360) & 3) + 2;
        fx.mem.set8(q + 9, v);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(q + o, 0x80);
        }
        fx.mem.write_u32(q + 0x10, 0x42A0_0000);
        fx.mem.write_u32(q + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(q + 0x18, 0xBD75_C28F);
        let y = fx.mem.read_u32(D + 0x8304);
        fx.mem.write_u32(q + 0x1C, y);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(q + 0x20 + 4 * c, w);
        }
        let f = jit(fx, 0x01E0_53D0, 0x96, 0x4B);
        fx.mem.setf(q + 0x2C, f);
        let f = ee::add(f32::from_bits(0x3F4C_CCCD), jit(fx, 0x01E0_540C, 0x64, 0x32));
        fx.mem.setf(q + 0x30, f);
        let f = jit(fx, 0x01E0_5454, 0x96, 0x4B);
        fx.mem.setf(q + 0x34, f);
    }
}

/// Updates 128 / 141 / 155 [F 0x1E05498]: the fade, bands, two spikes, a
/// flash and a shake at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 6, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    for s4 in 0..2u32 {
        for s3 in 0..4i32 {
            let i = first_free(fx, BANDS, 0x5C, 0x18);
            if i < 0 {
                continue;
            }
            let b = BANDS + 0x5C * i as u32;
            set_counter4(fx, b, 0x1C, s3 & 0xFF, 0xFF);
            for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 1), (8, 1), (9, 2), (0xA, 6), (0xB, 0)] {
                fx.mem.write_u8(b + o, v);
            }
            fx.mem.write_u32(b + 0x10, 0x3F80_0000);
            fx.mem.write_u32(b + 0x14, 0);
            fx.mem.write_u32(b + 0x18, 0x4000_0000);
            fx.mem.write_u32(b + 0x1C, 0x3F80_0000);
            fx.mem.write_u32(b + 0x20, 0x3E80_0000);
            for (o, v) in [(0x25u32, 0x60u8), (0x26, 0x60), (0x24, 0xC0), (0x29, 0x60), (0x2A, 0x60), (0x28, 0xC0), (0x27, 0x10), (0x2B, 0x30)] {
                fx.mem.write_u8(b + o, v);
            }
            copy_v4(fx, sp + 0x60, b + 0x2C);
            fx.mem.set32(b + 0x3C, 0x3000);
            fx.mem.set32(b + 0x40, if s4 != 0 { 0x4000 } else { -0x4000 });
            if fx.mem.read_u32(D + 0x30) != 0x8000 {
                let v = fx.mem.i32(b + 0x40).wrapping_neg();
                fx.mem.set32(b + 0x40, v);
            }
        }
    }
    for s3 in 0..2u32 {
        let q = SPIKES + 0x68 * s3;
        if s3 != 0 {
            set_counter4(fx, q, 0x1C, 0, 0xFF);
            for (o, v) in [(4u32, 5u8), (0xA, 0), (0xC, 5)] {
                fx.mem.write_u8(q + o, v);
            }
            fx.mem.write_u32(q + 0x14, 0x4000_0000);
            fx.mem.write_u32(q + 0x1C, 0x4220_0000);
            fx.mem.write_u8(q + 0x23, 0);
            fx.mem.write_u8(q + 0x27, 0x60);
            fx.mem.write_u8(q + 8, 0xA0);
            fx.mem.write_u32(q + 0x58, 0x40);
            fx.mem.write_u32(q + 0x5C, 0x80);
            fx.mem.write_u32(q + 0x60, 0x100);
        } else {
            set_counter4(fx, q, 0x20, 0, 0xFF);
            for (o, v) in [(4u32, 7u8), (0xA, 2), (0xC, 3)] {
                fx.mem.write_u8(q + o, v);
            }
            fx.mem.write_u32(q + 0x14, 0x3FC0_0000);
            fx.mem.write_u32(q + 0x1C, 0x42B4_0000);
            fx.mem.write_u8(q + 0x23, 0x80);
            fx.mem.write_u8(q + 0x27, 0x20);
            fx.mem.write_u8(q + 8, 0x78);
        }
        fx.mem.write_u8(q + 5, 0);
        fx.mem.write_u8(q + 0xB, 2);
        let x = rand4(fx, 0x01E0_56E0);
        fx.mem.set8(q + 9, x);
        fx.mem.write_u32(q + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(q + o, 0x80);
        }
        copy_v4(fx, sp + 0x60, q + 0x28);
    }
    let f = FLASH;
    set_counter4(fx, f, 0x20, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xA, 0x40), (9, 0x10), (8, 0x80)] {
        fx.mem.write_u8(f + o, v);
    }
    fx.mem.write_u32(f + 0x14, 0);
    fx.mem.write_u32(f + 0x10, 0x447A_0000);
    copy_v4(fx, sp + 0x60, f + 0x1C);
    set_counter4(fx, D + 0x40, 0x24, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x3F80_0000);
    fx.mem.write_u32(D + 0x58, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x5C, 0x3FB3_3333);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    copy_v4(fx, sp + 0x60, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_4CAC);
    match fx.mem.u16(D) {
        0x104 => return 8,
        0x8D | 0x80 => strike(fx, sp),
        // The sprite loop falls through into the strike [F 0x1E05498].
        0x9B => {
            sprites(fx, sp);
            strike(fx, sp);
        }
        0x7F => window_streaks(fx, 0x80, 0xDB, 6, 1, 0x4180_0000, 0x43, 0x40),
        0x33 => {
            for s3 in 0..2u32 {
                let r = SMEARS + 0x48 * s3;
                set_counter4(fx, r, 0x7E, 0, 0xFF);
                clear_history(fx, r);
            }
        }
        2 => start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_5858, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5864, [Some(u64::from(D)), Some(u64::from(OT))]);
    let n = fx.mem.u16(D);
    object_ops(fx, OPS, n);
    blib::star::flash_b(fx, 0x01E0_0AE0, FLASH, OT, 1);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the runner's s5 / s6 (0) at +0x60 / +0x70
    // and ra at +0x80; s5's upper half clears the u16 at caller sp - 0x86,
    // whose top bits streaks_b's vertex-3 V keeps (case 2 update 7, helper
    // B13) [run, sandbox tracer].
    fx.saved(sp - 0xF0, &[(0x60, 0), (0x70, 0), (0x80, 0x01E0_593C)]);
    blib::spike::spikes(fx, 0x01E0_2380, SPIKES, OT, 0x1E);
    let (_, drop_a3) = blib::drop::drops_a3(fx, DROPS, OT, 0x190);
    blib::band::bands_b(fx, 0x01E0_03A0, BANDS, OT, 0x18);
    let (_, smear_a3) = blib::smear::smears_a3(fx, 0x01E0_2E60, SMEARS, OT, 2);
    let (_, a3) = blib::radiate::streaks_with(fx, 0x01E0_38E0, STREAKS, OT, 1, &blib::radiate::STREAKS_B);
    let n = fx.mem.u16(D);
    // a3 = what the streaks left, without them the smears', then the
    // drops' [run]; [H] bands_b's (between them) not tracked, none: 0.
    blib::window::frame_fade_b(fx, 0x01E0_46E0, WINDOW, OT, n, a3.or(smear_a3).or(drop_a3).unwrap_or(0));
    let n = fx.mem.u16(D);
    if (0x80..0xC0).contains(&n) {
        6
    } else if n == 0xC0 {
        2
    } else if n == 0x76 {
        1
    } else {
        0
    }
}
