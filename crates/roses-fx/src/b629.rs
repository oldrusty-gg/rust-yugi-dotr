//! BMODULE b629 (card 451), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04EE0, data area a0 = 0x1ED0000, 0xA960 bytes)
//! [F]: s3 = D, s7 = the OT (0x206E40), s0 the
//! parameter then offsets / pointers. Update 2 node 0x41 read (+0xA84C, 5
//! lower); 30 a flash (+0x9DDC, 0x4C apart) and a ribbon (+0xAC, 0x68
//! apart) on node 0x34 and a shake (+0x40); on updates 112..163 six
//! bouncing sprites (+0x190C, 0x44 apart) a update from nodes 0x41 / 0x4B;
//! on even updates 80..149 the attacker's tint (object op 0x1F83B0); 149 an
//! arch (+0x9E74, 0x54 apart, copied on every update while it counts) from
//! node 0xC to the target, 112 sprites along it and the frame fade
//! (+0xA504, `window::frame_fade`, updates 149..189); 157 at the target the
//! fade (+0xA4), 250 sprites, three ribbons, a flash and a shake. The
//! streaks (+0xA534) and bands (+0xA56C) are stepped but never set. The
//! stage light pulsed by 0x1031C0 (+0xA0). Returns 6 on updates 157..185,
//! 2 on 186, 1 on 147, 8 on 280 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `fade`
//! 0x1E00410, `flash` 0x1E00950, `ribbon` 0x1E01500, `bounce` 0x1E02090,
//! `bands` 0x1E02B00, `arches` 0x1E032F0, `streaks` 0x1E03BE0,
//! `frame_fade` 0x1E04A00 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0xA85C;
const NODE_Y: u32 = D + 0xA850;
const SPRITES: u32 = D + 0x190C;
const ARCHES: u32 = D + 0x9E74;
const FLASH: u32 = D + 0x9DDC;
const RIBBONS: u32 = D + 0xAC;
const FADE: u32 = D + 0xA504;

/// The entry 0x1E04EE0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0xA960).
    fx.bzero(D, 0xA960);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x34, 0), (0x14, 0x41, 0), (0x18, 0x4B, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x112);
    // The entry's s0 = the parameter, s1 / s2 the runner's 0.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5040, [Some(param as u32), Some(0), Some(0)]);
    0
}

/// rand >> 4 from return address `ra`, the entry's s0 (64 bits) saved.
fn rand4(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand() >> 4
}

/// ((rand >> 4) % m - `half`) / 100.
fn jit(fx: &mut Fx, s0: u64, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand4(fx, s0, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

/// ((rand >> 4) % m x 200 - `off`) / 100.
fn spread(fx: &mut Fx, s0: u64, ra: u32, m: i32, off: i32) -> f32 {
    let v = rand4(fx, s0, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(200 * r - off), 100.0)
}

fn shake_at(fx: &mut Fx, count: i32, hold: bool, ky: u32, z: u32) {
    set_counter4(fx, D + 0x40, count, 0, 0xFF);
    if hold {
        fx.mem.write_u8(D + 0x45, 1);
        let b = fx.mem.read_u8(D + 0x10);
        fx.mem.write_u8(D + 0x46, b);
        fx.mem.write_u8(D + 0x44, 2);
    } else {
        fx.mem.write_u8(D + 0x45, 0);
        fx.mem.write_u8(D + 0x44, 1);
    }
    fx.mem.write_u32(D + 0x54, 0x4000_0000);
    fx.mem.write_u32(D + 0x58, 0x3FCC_CCCD);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, ky);
    fx.mem.write_u32(D + 0x90, z);
}

/// The ribbon tail common to updates 30 / 157 [F 0x1E05DB0].
fn ribbon_tail(fx: &mut Fx, r: u32, s0: u64, ra: u32) {
    fx.mem.write_u8(r + 0xB, 2);
    let x = rand4(fx, s0, ra);
    fx.mem.set8(r + 9, x);
    fx.mem.write_u32(r + 0x10, BYTES);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
}

/// Update 30 [F 0x1E050F8].
fn first(fx: &mut Fx, s0: u64) {
    set_counter4(fx, FLASH, 0x12, 0, 0xFF);
    fx.mem.write_u8(FLASH + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(FLASH + 6, b);
    for (o, v) in [(4u32, 0x80u8), (7, 9), (0xA, 0x80), (9, 0), (8, 0), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0)] {
        fx.mem.write_u8(FLASH + o, v);
    }
    fx.mem.write_u32(FLASH + 0x10, 0x4248_0000);
    fx.mem.write_u32(FLASH + 0x14, 0x41C8_0000);
    fx.mem.write_u32(FLASH + 0x18, 0x4120_0000);
    let r = RIBBONS;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(r + 6, b);
    for (o, v) in [(4u32, 5u8), (0xA, 0), (0xC, 5)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(r + 0x1C, 0x41A0_0000);
    fx.mem.write_u8(r + 0x23, 0);
    fx.mem.write_u8(r + 0x27, 0x60);
    fx.mem.write_u8(r + 8, 0x28);
    fx.mem.write_u32(r + 0x58, 0);
    fx.mem.write_u32(r + 0x5C, 0);
    fx.mem.write_u32(r + 0x60, 0x300);
    ribbon_tail(fx, r, s0, 0x01E0_525C);
    shake_at(fx, 0x12, true, 0x42A0_0000, 0x4080_0000);
}

/// Update 149 [F 0x1E052F4]: the arch from node 0xC, sprites along it and
/// the frame fade. Returns the entry's (s0, s1) after it.
fn arch(fx: &mut Fx, sp: u32, s0: u64) -> (u64, u64) {
    let i = first_free(fx, ARCHES, 0x54, 0x14);
    if i >= 0 {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
        node_position_saving(fx, h, s, sp + 0xA0, s0, Some(0), 0x01E0_532C);
        // (s0 = 0x54 i here, then sp + 0xB0 below.)
        let r = ARCHES + 0x54 * i as u32;
        set_counter4(fx, r, 8, 0, 0xFF);
        for (o, v) in [(4u32, 0xC0u8), (5, 0), (7, 9), (0xE, 0xC0), (0xD, 0x10), (0xC, 0x10), (0xF, 0x20), (0xB, 0xC0), (8, 0xC0), (9, 0xC0), (0xA, 0xC0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x41C0_0000);
        fx.mem.write_u32(r + 0x14, 0x4000_0000);
        for o in [0x18u32, 0x1C, 0x20] {
            fx.mem.write_u32(r + o, 0);
        }
        let t = if fx.mem.read_u32(D + 0x30) == 0x8000 { -0x4000 } else { 0x4000 };
        fx.mem.set32(r + 0x44, t);
        fx.mem.set32(r + 0x4C, 0x4000);
        copy_v4(fx, sp + 0xA0, r + 0x24);
        for c in 0..3u32 {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x90 + 4 * c), fx.mem.f(sp + 0xA0 + 4 * c)), 8.0);
            fx.mem.setf(r + 0x34 + 4 * c, v);
        }
    }
    // The record (even the -1st when none was free) [F].
    let r = ARCHES.wrapping_add((0x54 * i) as u32);
    copy_v4(fx, r + 0x34, sp + 0xB0);
    copy_v4(fx, sp + 0xA0, sp + 0xC0);
    let y = fx.mem.read_u32(NODE_Y);
    fx.mem.write_u32(sp + 0xC4, y);
    let s0 = u64::from(sp + 0xB0);
    for s4 in 0..0x10i32 {
        for _ in 0..7 {
            let i = first_free(fx, SPRITES, 0x44, 0x1F4);
            if i < 0 {
                continue;
            }
            let count = ((rand4(fx, s0, 0x01E0_55D4) & 7) + 0x1E) & 0xFF;
            let r = SPRITES + 0x44 * i as u32;
            set_counter4(fx, r, count, (s4 >> 1) & 0xFF, 0xFF);
            let v = rand4(fx, s0, 0x01E0_5614);
            fx.mem.set8(r + 4, (v & 3) << 4);
            for (o, v) in [(8u32, 1u8), (0xE, 0x80), (0xD, 0xA0), (0xC, 0xC0), (0xF, 0x60)] {
                fx.mem.write_u8(r + o, v);
            }
            let f = ee::add(150.0, spread(fx, s0, 0x01E0_5658, 0x1E, 0xBB8));
            fx.mem.setf(r + 0x10, f);
            let f = ee::add(12.0, spread(fx, s0, 0x01E0_56C4, 6, 0x258));
            fx.mem.setf(r + 0x14, f);
            fx.mem.write_u8(r + 0xB, 1);
            fx.mem.write_u8(r + 0xA, 5);
            let v = rand4(fx, s0, 0x01E0_5740);
            fx.mem.set8(r + 9, v & 3);
            fx.mem.write_u32(r + 0x18, 0xBD23_D70A);
            let y = fx.mem.read_u32(NODE_Y);
            fx.mem.write_u32(r + 0x1C, y);
            let f3 = ee::from_i32(s4);
            let x = ee::add(fx.mem.f(sp + 0xC0), ee::mul(f3, ee::div(fx.mem.f(sp + 0xB0), 2.0)));
            fx.mem.setf(r + 0x20, x);
            let y = fx.mem.read_u32(sp + 0xC4);
            fx.mem.write_u32(r + 0x24, y);
            let z = ee::add(fx.mem.f(sp + 0xC8), ee::mul(f3, ee::div(fx.mem.f(sp + 0xB8), 2.0)));
            fx.mem.setf(r + 0x28, z);
            let f = ee::add(1.0, jit(fx, s0, 0x01E0_57C0, 0x96, 0x4B));
            fx.mem.setf(r + 0x30, f);
            let f = jit(fx, s0, 0x01E0_5818, 0x28, 0x14);
            fx.mem.setf(r + 0x2C, f);
            let f = jit(fx, s0, 0x01E0_5864, 0x28, 0x14);
            fx.mem.setf(r + 0x34, f);
        }
    }
    fx.mem.set16(FADE, 0x95);
    fx.mem.set16(FADE + 2, 0xBD);
    fx.mem.write_u8(FADE + 5, 6);
    fx.mem.write_u8(FADE + 6, 1);
    fx.mem.write_u32(FADE + 8, 0x4180_0000);
    for (o, v) in [(0x14u32, -8i32), (0xC, -8), (0x18, 8), (0x10, 8), (0x20, -8), (0x1C, -8), (0x28, 8), (0x24, 8)] {
        fx.mem.set32(FADE + o, v);
    }
    for o in [0x2Eu32, 0x2D, 0x2C] {
        fx.mem.write_u8(FADE + o, 0x80);
    }
    (s0, u64::from(sp + 0xC0))
}

/// Update 157 [F 0x1E059B4]: the strike at the target (sp + 0x90).
/// Returns the entry's s0 after it.
fn strike(fx: &mut Fx, sp: u32, mut s0: u64) -> u64 {
    set_counter4(fx, D + 0xA4, 4, 0, 0xFF);
    fx.mem.write_u8(D + 0xA8, 1);
    for s4 in 0..0xFAi32 {
        let i = first_free(fx, SPRITES, 0x44, 0x1F4);
        if i < 0 {
            continue;
        }
        let count = ((rand4(fx, s0, 0x01E0_5A08) & 0x1F) + 0x20) & 0xFF;
        s0 = u64::from(0x44 * i as u32);
        let r = SPRITES + 0x44 * i as u32;
        // s4 / 25 by the multiply-high 0x51EB851F.
        let p = i64::from(0x51EB_851Fi32) * i64::from(s4);
        let hi = (p >> 32) as i32;
        fx.set_hilo(p as i32, hi);
        let delay = (hi >> 3) + ((s4 as u32) >> 31) as i32;
        set_counter4(fx, r, count, delay & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0x42u8), (8, 3), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        let v = (rand4(fx, s0, 0x01E0_5A84) & 3) + 2;
        fx.mem.set8(r + 9, v);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u32(r + 0x10, 0x4316_0000);
        fx.mem.write_u32(r + 0x14, 0xBF80_0000);
        fx.mem.write_u32(r + 0x18, 0xBD75_C28F);
        let y = fx.mem.read_u32(NODE_Y);
        fx.mem.write_u32(r + 0x1C, y);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x90 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let f = jit(fx, s0, 0x01E0_5AF8, 0xD2, 0x69);
        fx.mem.setf(r + 0x2C, f);
        let f = ee::add(f32::from_bits(0x3F99_999A), jit(fx, s0, 0x01E0_5B44, 0xB4, 0x5A));
        fx.mem.setf(r + 0x30, f);
        let f = jit(fx, s0, 0x01E0_5BA0, 0xD2, 0x69);
        fx.mem.setf(r + 0x34, f);
    }
    for s4 in 0..3u32 {
        s0 = u64::from(0x68 * s4);
        let r = RIBBONS + 0x68 * s4;
        match s4 {
            0 => {
                set_counter4(fx, r, 0x1C, 0, 0xFF);
                for (o, v) in [(4u32, 5u8), (0xA, 0), (0xC, 5)] {
                    fx.mem.write_u8(r + o, v);
                }
                fx.mem.write_u32(r + 0x14, 0x4000_0000);
                fx.mem.write_u32(r + 0x1C, 0x4220_0000);
                fx.mem.write_u8(r + 0x23, 0);
                fx.mem.write_u8(r + 0x27, 0x60);
                fx.mem.write_u8(r + 8, 0xA0);
                fx.mem.write_u32(r + 0x58, 0x40);
                fx.mem.write_u32(r + 0x5C, 0x80);
                fx.mem.write_u32(r + 0x60, 0x100);
            }
            1 => {
                set_counter4(fx, r, 0x1C, 0, 0xFF);
                for (o, v) in [(4u32, 7u8), (0xA, 3), (0xC, 3)] {
                    fx.mem.write_u8(r + o, v);
                }
                fx.mem.write_u32(r + 0x14, 0x3FC0_0000);
                fx.mem.write_u32(r + 0x1C, 0x42B4_0000);
                fx.mem.write_u8(r + 0x23, 0x80);
                fx.mem.write_u8(r + 0x27, 0x20);
                fx.mem.write_u8(r + 8, 0x78);
                for o in [0x60u32, 0x5C, 0x58] {
                    fx.mem.write_u32(r + o, 0);
                }
            }
            _ => {
                set_counter4(fx, r, 0x1E, 0, 0xFF);
                for (o, v) in [(4u32, 5u8), (0xA, 0), (0xC, 5)] {
                    fx.mem.write_u8(r + o, v);
                }
                fx.mem.write_u32(r + 0x14, 0x4090_0000);
                fx.mem.write_u32(r + 0x1C, 0x42B4_0000);
                fx.mem.write_u8(r + 0x23, 0);
                fx.mem.write_u8(r + 0x27, 0x60);
                fx.mem.write_u8(r + 8, 0x50);
                fx.mem.write_u32(r + 0x58, 0x200);
                fx.mem.write_u32(r + 0x5C, 0);
                fx.mem.write_u32(r + 0x60, 0);
            }
        }
        fx.mem.write_u8(r + 5, 0);
        ribbon_tail(fx, r, s0, 0x01E0_5DDC);
        copy_v4(fx, sp + 0x90, r + 0x28);
    }
    let i = first_free(fx, FLASH, 0x4C, 4);
    if i >= 0 {
        s0 = u64::from(0x4C * i as u32);
        let a = FLASH + 0x4C * i as u32;
        set_counter4(fx, a, 0x22, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (7, 9), (4, 0x81)] {
            fx.mem.write_u8(a + o, v);
        }
        fx.mem.write_u32(a + 0x10, 0x4448_0000);
        fx.mem.write_u32(a + 0x14, 0);
        fx.mem.write_u32(a + 0x18, 0);
        for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0), (9, 0), (8, 0x80)] {
            fx.mem.write_u8(a + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(a + o, 0);
        }
        copy_v4(fx, sp + 0x90, a + 0x1C);
    }
    shake_at(fx, 0x20, false, 0x4220_0000, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, s0, 0x01E0_6004);
    s0
}

/// Updates 112..163 [F 0x1E06018]: three sprites from each of nodes 0x41
/// / 0x4B. Returns the entry's s0 after.
fn sparks(fx: &mut Fx, sp: u32, mut s0: u64, s1: u64) -> u64 {
    for s4 in 0..2u32 {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14 + 4 * s4));
        node_position_saving(fx, h, s, sp + 0xA0, s0, Some(s1), 0x01E0_605C);
        for _ in 0..2 {
            let i = first_free(fx, SPRITES, 0x44, 0x1F4);
            if i < 0 {
                continue;
            }
            s0 = u64::from(0x44 * i as u32);
            let r = SPRITES + 0x44 * i as u32;
            set_counter4(fx, r, 0x20, 0, 0xFF);
            let v = rand4(fx, s0, 0x01E0_60B8);
            fx.mem.set8(r + 4, (v & 3) << 4);
            for (o, v) in [(8u32, 1u8), (0xE, 0x80), (0xD, 0xA0), (0xC, 0xC0), (0xF, 0x60)] {
                fx.mem.write_u8(r + o, v);
            }
            let f = ee::add(150.0, spread(fx, s0, 0x01E0_60FC, 0x1E, 0xBB8));
            fx.mem.setf(r + 0x10, f);
            let v = rand4(fx, s0, 0x01E0_6168);
            let mut m = v & 3;
            if v < 0 && m != 0 {
                m -= 4;
            }
            let f = ee::add(8.0, ee::div(ee::from_i32(200 * m - 400), 100.0));
            fx.mem.setf(r + 0x14, f);
            fx.mem.write_u8(r + 0xB, 1);
            fx.mem.write_u8(r + 0xA, 5);
            let v = rand4(fx, s0, 0x01E0_61E8);
            fx.mem.set8(r + 9, v & 3);
            fx.mem.write_u32(r + 0x18, 0);
            fx.mem.write_u32(r + 0x1C, 0);
            for (c, ra) in [(0u32, 0x01E0_6208u32), (1, 0x01E0_6258), (2, 0x01E0_62A8)] {
                let f = ee::add(fx.mem.f(sp + 0xA0 + 4 * c), jit(fx, s0, ra, 0x1F4, 0xFA));
                fx.mem.setf(r + 0x20 + 4 * c, f);
            }
            let f = ee::add(f32::from_bits(0x3E4C_CCCD), jit(fx, s0, 0x01E0_62F8, 0x1E, 0xF));
            fx.mem.setf(r + 0x30, f);
            let f = jit(fx, s0, 0x01E0_6354, 0x3C, 0x1E);
            fx.mem.setf(r + 0x2C, f);
            let f = ee::add(f32::from_bits(0x3E4C_CCCD), jit(fx, s0, 0x01E0_63A0, 0x5A, 0x2D));
            fx.mem.setf(r + 0x34, f);
            if fx.mem.read_u32(D + 0x30) != 0x8000 {
                let v = ee::mul(fx.mem.f(r + 0x34), -1.0);
                fx.mem.setf(r + 0x34, v);
            }
        }
    }
    s0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut s0 = i64::from(param) as u64;
    let mut s1 = 0u64;
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x90, s0, 0x01E0_5068);
    match fx.mem.u16(D) {
        0x118 => return 8,
        0x9D => s0 = strike(fx, sp, s0),
        0x95 => (s0, s1) = arch(fx, sp, s0),
        0x1E => first(fx, s0),
        2 => {
            let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14));
            node_position_saving(fx, h, s, D + 0xA84C, s0, Some(0), 0x01E0_50C8);
            let y = ee::sub(fx.mem.f(NODE_Y), 5.0);
            fx.mem.setf(NODE_Y, y);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x70..0xA4).contains(&n) {
        s0 = sparks(fx, sp, s0, s1);
    }
    let n = fx.mem.u16(D);
    if (0x50..0x96).contains(&n) && n & 1 == 0 {
        for o in [0xCAu32, 0xC9, 0xC8, 0xCE, 0xCD, 0xCC] {
            fx.mem.write_u8(sp + o, 0x80);
        }
        fx.mem.write_u8(sp + 0xCB, 0x60);
        fx.mem.write_u8(sp + 0xCF, 0x10);
        let h = fx.mem.read_u32(D + 4);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h, 6, 0, sp + 0xC8]);
    }
    if fx.mem.u8(ARCHES) > 0 {
        let i = first_free(fx, ARCHES, 0x54, 0x14);
        if i >= 0 {
            let r = ARCHES + 0x54 * i as u32;
            for k in 0..0x15u32 {
                let w = fx.mem.read_u32(ARCHES + 4 * k);
                fx.mem.write_u32(r + 4 * k, w);
            }
            set_counter4(fx, r, 6, 0, 0xFF);
            fx.mem.write_u8(r + 4, 0);
            fx.mem.write_u32(r + 0x20, 0);
            for o in [0x3Cu32, 0x38, 0x34] {
                fx.mem.setf(r + o, 0.0);
            }
            fx.mem.write_u32(r + 0x18, 0xC000_0000);
        }
    }
    shake_saving(fx, D + 0x40, s0, 0x01E0_65CC, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_65DC, [Some(s0), Some(s1)]);
    blib::fade::fade(fx, D + 0xA4, OT);
    let (_, flash_a3) = blib::star::flash_a3(fx, 0x01E0_0950, FLASH, OT, 2);
    let (_, a3) = blib::radiate::streaks_a3(fx, 0x01E0_3BE0, D + 0xA534, OT, 1);
    // a3 = what streaks / flash left, else the entry's last 0x101AD0's
    // 0xFF (the arch copy's, updates 149..156) [run]; [H] no counter set
    // this update: not modelled.
    let n = fx.mem.u16(D);
    blib::window::frame_fade(fx, 0x01E0_4A00, FADE, OT, n, a3.or(flash_a3).unwrap_or(0xFF));
    blib::streak::ribbon(fx, 0x01E0_1500, RIBBONS, OT, 0x3C);
    blib::sprite::bounce(fx, SPRITES, OT, 0x1F4);
    blib::band::bands(fx, 0x01E0_2B00, D + 0xA56C, OT, 8);
    blib::arch::arches(fx, 0x01E0_32F0, ARCHES, OT, 0x14);
    let n = fx.mem.u16(D);
    if (0x9D..0xBA).contains(&n) {
        6
    } else if n == 0xBA {
        2
    } else if n == 0x93 {
        1
    } else {
        0
    }
}
