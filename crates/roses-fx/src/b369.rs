//! BMODULE b369 (card 486), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04A50, data area a0 = 0x1ED0000, 0xADE0 bytes)
//! [F]: s0 = D, s1 = the OT (0x206E40), s2 the
//! parameter then counters / pointers. Update 2 lowers node 1's height
//! (+0xACC4, y - 7) and starts eight scrolling tubes (+0xAB64,
//! `segtube::tubes`, two groups of four radii 1.8 / 1.5 / 1.0 / 0.7) along
//! eight segments (+0xAAA4) recomputed every update between the attacker's
//! nodes 0xD / 0xE and node 5 / 4: from the node, out along node 4 - node
//! 5 by 0.55 .. 1.0. 100: the fade (+0xA4), the frame fade (+0xAA3C,
//! `window::frame_fade_b`, 100..132), the attacker's streaks (+0xAA6C,
//! `radiate::streaks_b`), the tubes keyed 0, eight bands (+0xA75C,
//! `band::bands_b`, turned +-0x4000), four spikes (+0xA2C), two flashes
//! (+0xAC, `star::flash_b`) and a shake at node 5 (sp + 0x80). While 100
//! <= n < 192 the unit's u16 +0x20A = 1 (0x1F69C0), six sprites (+0x228C,
//! `drop::drops`) a update and a spike; while 2 <= n < 192 six sprites a
//! update about node 1. Returns 6 on updates 100..188, 2 on 189, 1 on 90,
//! 8 on 240 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash_b`
//! 0x1E003A0, `spikes` 0x1E00E10, `drops` 0x1E018F0, `fade` 0x1E02250,
//! `bands_b` 0x1E02720, `tubes` 0x1E02E60, `streaks_b` 0x1E037D0,
//! `frame_fade_b` 0x1E045D0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const FADE: u32 = D + 0xA4;
const FLASH: u32 = D + 0xAC;
const SPIKES: u32 = D + 0xA2C;
const DROPS: u32 = D + 0x228C;
const BANDS: u32 = D + 0xA75C;
const WINDOW: u32 = D + 0xAA3C;
const STREAKS: u32 = D + 0xAA6C;
const PTS: u32 = D + 0xAAA4;
const TUBES: u32 = D + 0xAB64;
const NODE: u32 = D + 0xACC4;
const BYTES: u32 = D + 0xACD4;

/// The entry 0x1E04A50 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0xADE0).
    fx.bzero(D, 0xADE0);
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
    for (at, node, clear) in [(0xCu32, 5, 1), (0x10, 4, 0), (0x14, 0xD, 0), (0x18, 0xE, 0), (0x1C, 1, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xEA);
    // The entry's s0 = D, s1 = 3 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4B84, [Some(D), Some(3), Some(param as u32)]);
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

fn node_at(fx: &mut Fx, at: u32, out: u32, ra: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + at));
    node_position_saving(fx, h, s, out, u64::from(D), Some(u64::from(OT)), ra);
}

/// 0x1F69C0(v) [F]: the self unit's u16 +0x20A = v (0x1F69D0 on the self
/// handle 0x2DF5AC: side 0..2, slot 0..10, its object 0x1E25E0 / 0x1F4200);
/// its frame (0x20: the caller's s0 +0, ra +0x10) left below sp.
pub(crate) fn unit_flag(fx: &mut Fx, v: i32, s0: u64, ra: u32) {
    let f = fx.sp - 0x20;
    fx.saved(f, &[(0, s0), (0x10, u64::from(ra))]);
    let h = fx.self_handle();
    let (side, slot) = (h >> 8, h & 0xFF);
    if !(0..3).contains(&side) || slot >= 0xB {
        return;
    }
    let i = fx.object_index(side, slot);
    if !(0..0x54).contains(&i) {
        return;
    }
    let o = fx.mem.read_u32(crate::world::OBJECT_TABLE + 4 * i as u32);
    if o != 0 {
        fx.mem.set16(o + 0x20A, v);
    }
}

/// Update 2 [F 0x1E04BD8]: node 1's height lowered by 7 and the eight
/// tubes (+0xAB64) on the segments (+0xAAA4).
fn start(fx: &mut Fx) {
    node_at(fx, 0x1C, NODE, 0x01E0_4BEC);
    let y = ee::sub(fx.mem.f(NODE + 4), 7.0);
    fx.mem.setf(NODE + 4, y);
    for s2 in 0..2u32 {
        for s6 in 0..4u32 {
            let s3 = 4 * s2 + s6;
            let r = TUBES + 0x2C * s3;
            set_counter4(fx, r, 0xC0, 0, 0xFF);
            for (o, v) in [(7u32, 0x18u8), (8, 1), (0xB, 0x48), (9, 2), (0xA, 2)] {
                fx.mem.write_u8(r + o, v);
            }
            fx.mem.write_u32(r + 0xC, PTS + 0x18 * s3);
            fx.mem.write_u32(r + 0x18, 0);
            fx.mem.write_u32(r + 0x1C, 0x3E2E_147B);
            fx.mem.write_u32(r + 0x10, 0x4000_0000);
            fx.mem.write_u32(r + 0x14, 0x4000_0000);
            let radius = match s6 {
                0 => 0x3FE6_6666,
                1 => 0x3FC0_0000,
                2 => 0x3F80_0000,
                _ => {
                    fx.mem.write_u32(r + 0x14, 0x4040_0000);
                    0x3F33_3333
                }
            };
            fx.mem.write_u32(r + 0x20, radius);
            for o in [0x28u32, 0x29, 0x2A] {
                fx.mem.write_u8(r + o, 0x80);
            }
            fx.mem.write_u8(r + 0x2B, 0x50);
            fx.mem.write_u32(r + 0x24, 0);
        }
    }
}

/// Update 100 [F 0x1E04DF8]: the fade, the frame fade and streaks, the
/// tubes keyed 0, bands, spikes, flashes and a shake at node 5 (sp + 0x80).
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 0xA, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    let w = WINDOW;
    fx.mem.set16(w, 0x64);
    fx.mem.set16(w + 2, 0x84);
    fx.mem.write_u8(w + 5, 6);
    fx.mem.write_u8(w + 6, 1);
    fx.mem.write_u32(w + 8, 0x4180_0000);
    for (o, v) in [(0x14u32, -8i32), (0xC, -8), (0x18, 8), (0x10, 8), (0x20, -8), (0x1C, -8), (0x28, 8), (0x24, 8)] {
        fx.mem.set32(w + o, v);
    }
    for o in [0x2Eu32, 0x2D, 0x2C] {
        fx.mem.write_u8(w + o, 0x80);
    }
    let s = STREAKS;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    for (o, v) in [(4u32, 0x80u8), (9, 1), (0xA, 0xFF), (8, 0), (0xB, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(s + 0xC, h);
    for (o, v) in [(0x10u32, 0x80u8), (0x11, 0x80), (0x12, 0x80), (0x13, 0x30), (0x14, 0x80), (0x15, 0x80), (0x16, 0x80), (0x17, 0x40), (5, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let v = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(s + 6, v);
    for a3 in 0..8u32 {
        fx.mem.write_u8(TUBES + 0x2C * a3 + 4, 0x80);
    }
    for s4 in 0..2i32 {
        for s3 in 0..4i32 {
            let i = first_free(fx, BANDS, 0x5C, 8);
            if i < 0 {
                continue;
            }
            let b = BANDS + 0x5C * i as u32;
            set_counter4(fx, b, 0x20, s3 & 0xFF, 0xFF);
            for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 1), (8, 2), (9, 2), (0xA, 6), (0xB, 0)] {
                fx.mem.write_u8(b + o, v);
            }
            fx.mem.write_u32(b + 0x10, 0x3F80_0000);
            fx.mem.write_u32(b + 0x14, 0);
            fx.mem.write_u32(b + 0x18, 0x4020_0000);
            fx.mem.write_u32(b + 0x1C, 0x3FA6_6666);
            fx.mem.write_u32(b + 0x20, 0x3E80_0000);
            for (o, v) in [(0x28u32, 0x80u8), (0x29, 0x80), (0x2A, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0), (0x2B, 0x30)] {
                fx.mem.write_u8(b + o, v);
            }
            copy_v4(fx, sp + 0x80, b + 0x2C);
            fx.mem.set32(b + 0x3C, 0x2800);
            fx.mem.set32(b + 0x40, if s4 != 0 { 0x4000 } else { -0x4000 });
        }
    }
    loop {
        let i = first_free(fx, SPIKES, 0x68, 0x3C);
        if i >= 0 {
            let q = SPIKES + 0x68 * i as u32;
            fx.mem.write_u8(q + 4, 5);
            set_counter4(fx, q, 0x20, i & 0xFF, 0xFF);
            let f = fx.mem.read_u8(q + 4) | 2;
            fx.mem.write_u8(q + 4, f);
            fx.mem.write_u32(q + 0x14, 0x4000_0000);
            fx.mem.write_u32(q + 0x1C, 0x4270_0000);
            fx.mem.write_u8(q + 0x23, 0x80);
            fx.mem.write_u8(q + 0x27, 0x80);
            for (o, v) in [(5u32, 0u8), (0xA, 1), (0xB, 2), (0xC, 5), (8, 0x3C)] {
                fx.mem.write_u8(q + o, v);
            }
            let x = rand4(fx, 0x01E0_52A4);
            fx.mem.set8(q + 9, x);
            fx.mem.write_u32(q + 0x10, BYTES);
            fx.mem.write_u32(q + 0x18, 0x3F80_0000);
            for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
                fx.mem.write_u8(q + o, 0x80);
            }
            copy_v4(fx, sp + 0x80, q + 0x28);
        }
        if i + 1 >= 4 {
            break;
        }
    }
    for s3 in 0..2u32 {
        let f = FLASH + 0x4C * s3;
        set_counter4(fx, f, 0x6C, 0, 0xFF);
        fx.mem.write_u8(f + 5, 0);
        fx.mem.write_u8(f + 7, 9);
        if s3 != 0 {
            fx.mem.write_u8(f + 4, 3);
            fx.mem.write_u32(f + 0x10, 0x4348_0000);
        } else {
            fx.mem.write_u8(f + 4, 0x85);
            fx.mem.write_u32(f + 0x10, 0x4416_0000);
        }
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(f + o, 0xC0);
        }
        fx.mem.write_u8(f + 0xA, 0x80);
        fx.mem.write_u8(f + 9, 0x80);
        fx.mem.write_u8(f + 8, 0x10);
        fx.mem.write_u32(f + 0x14, 0);
        fx.mem.write_u32(f + 0x18, 0);
        copy_v4(fx, sp + 0x80, f + 0x1C);
    }
    set_counter4(fx, D + 0x40, 0x6C, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 8);
    fx.mem.write_u32(D + 0x54, 0x4000_0000);
    fx.mem.write_u32(D + 0x58, 0x3FCC_CCCD);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    copy_v4(fx, sp + 0x80, D + 0x60);
}

/// x * -1.0 (the original's negation).
fn neg(fx: &mut Fx, a: u32) {
    let v = ee::mul(fx.mem.f(a), -1.0);
    fx.mem.setf(a, v);
}

/// Updates 100..191 [F 0x1E0549C]: six sprites a update from node 5 and a
/// spike there.
fn burst(fx: &mut Fx, sp: u32) {
    for _ in 0..6 {
        let i = first_free(fx, DROPS, 0x44, 0x1F4);
        if i < 0 {
            continue;
        }
        let delay = rand4(fx, 0x01E0_54C4) & 7;
        let p = DROPS + 0x44 * i as u32;
        set_counter4(fx, p, 0x24, delay, 0xFF);
        for (o, v) in [(4u32, 0x42u8), (8, 1), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(p + o, v);
        }
        let v = rand4(fx, 0x01E0_550C) & 3;
        fx.mem.set8(p + 9, v);
        for (o, v) in [(0xEu32, 0x80u8), (0xD, 0xC0), (0xC, 0xC0), (0xF, 0xC0)] {
            fx.mem.write_u8(p + o, v);
        }
        fx.mem.write_u32(p + 0x10, 0x4248_0000);
        fx.mem.write_u32(p + 0x14, 0xBECC_CCCD);
        fx.mem.write_u32(p + 0x18, 0xBDCC_CCCD);
        let y = fx.mem.read_u32(NODE + 4);
        fx.mem.write_u32(p + 0x1C, y);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * c);
            fx.mem.write_u32(p + 0x20 + 4 * c, w);
        }
        let f = jit(fx, 0x01E0_557C, 0xF0, 0x78);
        fx.mem.setf(p + 0x2C, f);
        let f = ee::add(f32::from_bits(0x3F99_999A), jit(fx, 0x01E0_55B8, 0xBE, 0x5F));
        fx.mem.setf(p + 0x30, f);
        let f = jit(fx, 0x01E0_5600, 0xD2, 0x69);
        fx.mem.setf(p + 0x34, f);
        if ee_lt(f, 0.0) {
            neg(fx, p + 0x34);
        }
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            neg(fx, p + 0x34);
        }
    }
    let i = first_free(fx, SPIKES, 0x68, 0x3C);
    if i < 0 {
        return;
    }
    let q = SPIKES + 0x68 * i as u32;
    set_counter4(fx, q, 0x14, 0, 0xFF);
    fx.mem.write_u8(q + 4, 5);
    fx.mem.write_u32(q + 0x14, 0x4040_0000);
    fx.mem.write_u32(q + 0x1C, 0x41F0_0000);
    for (o, v) in [(8u32, 0xCu8), (5, 0), (0xA, 1), (0xB, 2), (0xC, 6)] {
        fx.mem.write_u8(q + o, v);
    }
    let x = rand4(fx, 0x01E0_5720);
    fx.mem.set8(q + 9, x);
    fx.mem.write_u32(q + 0x10, BYTES);
    fx.mem.write_u32(q + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(q + o, 0x80);
    }
    fx.mem.write_u8(q + 0x23, 0x30);
    fx.mem.write_u8(q + 0x27, 0x80);
    let v = rand4(fx, 0x01E0_576C) & 0x3FF;
    fx.mem.set32(q + 0x60, -v);
    if fx.mem.read_u32(D + 0x30) != 0x8000 {
        fx.mem.set32(q + 0x60, v);
    }
    copy_v4(fx, sp + 0x80, q + 0x28);
}

/// Updates 2..191 [F 0x1E057E8]: six sprites a update about node 1 (sp +
/// 0x90), three on each side.
fn sprites(fx: &mut Fx, sp: u32) {
    node_at(fx, 0x1C, sp + 0x90, 0x01E0_57F8);
    for s4 in 0..2 {
        for _ in 0..3 {
            let i = first_free(fx, DROPS, 0x44, 0x1F4);
            if i < 0 {
                continue;
            }
            let p = DROPS + 0x44 * i as u32;
            set_counter4(fx, p, 0x20, 0, 0xFF);
            let v = (rand4(fx, 0x01E0_5844) & 3) << 4;
            fx.mem.set8(p + 4, v);
            for (o, v) in [(8u32, 0u8), (0xE, 0x80), (0xD, 0xA0), (0xC, 0xC0), (0xF, 0x60)] {
                fx.mem.write_u8(p + o, v);
            }
            fx.mem.write_u32(p + 0x10, 0x4270_0000);
            let r = rand4(fx, 0x01E0_5884) % 4;
            let f = ee::add(9.0, ee::div(ee::from_i32(200 * r - 400), 100.0));
            fx.mem.setf(p + 0x14, f);
            fx.mem.write_u8(p + 0xB, 1);
            fx.mem.write_u8(p + 0xA, 5);
            let v = rand4(fx, 0x01E0_58F0) & 3;
            fx.mem.set8(p + 9, v);
            fx.mem.write_u32(p + 0x18, 0);
            let y = fx.mem.read_u32(NODE + 4);
            fx.mem.write_u32(p + 0x1C, y);
            let x = if s4 != 0 {
                let j = jit(fx, 0x01E0_591C, 0x190, 0xC8);
                ee::add(ee::add(10.0, fx.mem.f(sp + 0x90)), j)
            } else {
                let j = jit(fx, 0x01E0_5970, 0x190, 0xC8);
                ee::add(ee::sub(fx.mem.f(sp + 0x90), 10.0), j)
            };
            fx.mem.setf(p + 0x20, x);
            let j = jit(fx, 0x01E0_59C0, 0x190, 0xC8);
            let f = ee::add(fx.mem.f(NODE + 4), j);
            fx.mem.setf(p + 0x24, f);
            let f = ee::add(f32::from_bits(0x3E4C_CCCD), jit(fx, 0x01E0_5A08, 0x1E, 0xF));
            fx.mem.setf(p + 0x30, f);
            let f = jit(fx, 0x01E0_5A50, 0x3C, 0x1E);
            fx.mem.setf(p + 0x2C, f);
            let (dz, vz, ra) = if fx.mem.read_u32(D + 0x30) == 0x8000 {
                (-12.0, 0x3F99_999A, [0x01E0_5AA0, 0x01E0_5AEC])
            } else {
                (12.0, 0xBF99_999A, [0x01E0_5B3C, 0x01E0_5B88])
            };
            let j = jit(fx, ra[0], 0x190, 0xC8);
            let z = ee::add(ee::add(dz, fx.mem.f(sp + 0x98)), j);
            fx.mem.setf(p + 0x28, z);
            let f = ee::add(f32::from_bits(vz), jit(fx, ra[1], 0x5A, 0x2D));
            fx.mem.setf(p + 0x34, f);
        }
    }
}

/// Every update [F 0x1E05BE8]: the eight segments from nodes 0xD / 0xE
/// (sp + 0xB0 / 0xC0) out along node 4 - node 5 by 0.4 + 0.15 (k + 1).
fn segments(fx: &mut Fx, sp: u32) {
    for (at, out, ra) in [(0xCu32, 0x90u32, 0x01E0_5BF8), (0x10, 0xA0, 0x01E0_5C0C), (0x14, 0xB0, 0x01E0_5C1C), (0x18, 0xC0, 0x01E0_5C2C)] {
        node_at(fx, at, sp + out, ra);
    }
    let d: [f32; 3] = std::array::from_fn(|c| ee::sub(fx.mem.f(sp + 0xA0 + 4 * c as u32), fx.mem.f(sp + 0x90 + 4 * c as u32)));
    for a1 in 0..8u32 {
        let src = sp + 0xB0 + 16 * (a1 >> 2);
        let p = PTS + 0x18 * a1;
        for c in 0..3u32 {
            let w = fx.mem.read_u32(src + 4 * c);
            fx.mem.write_u32(p + 0xC + 4 * c, w);
            fx.mem.write_u32(p + 4 * c, w);
        }
        let f = ee::add(f32::from_bits(0x3ECC_CCCD), ee::mul(f32::from_bits(0x3E19_999A), ee::from_i32((a1 & 3) as i32 + 1)));
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(p + 0xC + 4 * c), ee::mul(d[c as usize], f));
            fx.mem.setf(p + 0xC + 4 * c, v);
        }
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    node_at(fx, 0xC, sp + 0x80, 0x01E0_4BA8);
    // The runner's s5 / s6 (0) unless update 2's tube loop set them.
    let mut s56 = (0u64, 0u64);
    match fx.mem.u16(D) {
        0xF0 => return 8,
        0x64 => strike(fx, sp),
        2 => {
            start(fx);
            s56 = (u64::from(D + 0x2C * 7), 4);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x64..0xC0).contains(&n) {
        unit_flag(fx, 1, u64::from(D), 0x01E0_5480);
    }
    let n = fx.mem.u16(D);
    if (0x64..0xC0).contains(&n) {
        burst(fx, sp);
    }
    let n = fx.mem.u16(D);
    if (2..0xC0).contains(&n) {
        sprites(fx, sp);
    }
    segments(fx, sp);
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_5D74, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5D80, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the entry's s0 / s1 (D / the OT), s5 / s6
    // and ra [F, as b601]; s2..s4 [H] left.
    fx.saved(sp - 0xF0, &[(0x10, u64::from(D)), (0x20, u64::from(OT)), (0x60, s56.0), (0x70, s56.1), (0x80, 0x01E0_5D8C)]);
    blib::band::bands_b(fx, 0x01E0_2720, BANDS, OT, 8);
    blib::star::flash_b(fx, 0x01E0_03A0, FLASH, OT, 5);
    blib::spike::spikes(fx, 0x01E0_0E10, SPIKES, OT, 0x3C);
    let (_, drop_a3) = blib::drop::drops_a3(fx, DROPS, OT, 0x1F4);
    let (_, tube_a3) = blib::segtube::tubes_a3(fx, 0x01E0_2E60, TUBES, OT, 8);
    let (_, a3) = blib::radiate::streaks_with(fx, 0x01E0_37D0, STREAKS, OT, 1, &blib::radiate::STREAKS_B);
    let n = fx.mem.u16(D);
    // [H] bands / flashes / spikes' a3 not tracked (the drops' covers them).
    blib::window::frame_fade_b(fx, 0x01E0_45D0, WINDOW, OT, n, a3.or(tube_a3).or(drop_a3).unwrap_or(0));
    let n = fx.mem.u16(D);
    if (0x64..0xBD).contains(&n) {
        6
    } else if n == 0xBD {
        2
    } else if n == 0x5A {
        1
    } else {
        0
    }
}
