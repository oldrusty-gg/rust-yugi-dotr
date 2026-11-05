//! BMODULE b040 (card 156), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06ED0, data area a0 = 0x1ED0000, 0x88B0 bytes)
//! [F]: s0 = D, s1 = the OT (0x206E40), s2 the
//! parameter then a record pointer, s3 / s4 counters. b131's family. Init:
//! the handles (+4 / +8), the attacker's rotation (+0x2C), textures (4,
//! 0xF) and (5, 0..3), nodes 0x2E (cleared, +0xC) / 0x30 (+0x10) / 0x1A
//! (+0x14) / 0x10 (+0x18) / 0x4A (+0x1C), the spark-emitter group (+0xA4:
//! update lists 0x1E07FF0 / 0x1E07FE8 at nodes 0x10 / 0x1A, window 0x5F
//! .. 0x65, 180 / 150 / 6), the light ramp (+0xA0: 2 / 0xCC), the random
//! byte table (+0x87AC). Every update the target's position (0x1F71A0)
//! into sp + 0x70. Update 2: the frame fade record (+0x865C,
//! `window::frame_fade_b`: 2..0x18, 6 in, blend 1, grey 0x80). Update
//! 0x18: four bars (+0xFB8, `cross::bars_b`, 0x12 counts, 400 x 0 / 160 x
//! 45 / 400 x 90 / 160 x 135, 4 or 3 wide), a flash (+0x1478,
//! `star::flash_b`, 0x13 counts, 150 wide) and a shake (+0x40, 0xE, kind 2
//! at node 0x30). Update 0x28: the arc (+0x10F8, `arc::arcs_b`, pool
//! +0x1158, 0x1A counts) and the rod (+0x155C, `rod::rods_b`, 0x52 counts,
//! node 0x2E -> 0x30, 50 / 0.1 / 0.1, alpha 0 rising 9 an update to 0xC0).
//! Update 0x72: the fade (+0x1C14, `fade::fade`, 0x10), 400 drops
//! (+0x1C1C, `drop::drops`, 0x26 + rand & 0xF counts, delays k / 40, cell
//! 2 + rand & 3, 80 wide, falling -0.8 / -0.08, from 20 below the target
//! rising 4 (k / 40), random drift) and three frustums (+0x868C,
//! `segtube::frustums`, 0x28 counts, 36 points, texture 3, plane 8, ring B
//! 0 / flags 0x20: radii 2 / 8 -> 8, 3 / 5 -> 13, 3 / 5 -> 8, from 20
//! below the target to 45 above / 10 below / 20 below). Update 0x74: six
//! spikes (+0x1594, `spike::spikes`, 0x14 counts, flags 4, 20 + 5 (k + 1)
//! long, 2.5 - 0.4 k / 0.4 + 0.4 k wide, 20 below the target), the second
//! flash (1400 wide, 20 below) and a shake (0x20, 2.0, 80). While the first
//! flash counts, the bars, the first spike and the flash are put 3 / 4 of
//! the way from node 0x2E to node 0x30 (soft double 0x1A2C30 / 0x1A31D8 /
//! 0x1A3118 / 0x1A38F8). Returns 6 on 0x72..0x8E, 2 on 0x6F, 1 on 0x68, 8
//! on 0xD2 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080 (0538257fe1fa),
//! `sparks` 0x1E003A0, `fan_ring` 0x1E00C70, `emitters` 0x1E00F40,
//! `rods_b` 0x1E01390, `arcs_b` 0x1E02370, `flash_b` 0x1E03490, `bars_b`
//! 0x1E03F00, `spikes` 0x1E04800, `fade` 0x1E052E0, `frame_fade_b`
//! 0x1E057B0, `frustums` 0x1E05C30, `drops` 0x1E06570 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const LIGHT: u32 = D + 0xA0;
const GROUP: u32 = D + 0xA4;
const SPARKS: u32 = D + 0xE8;
const BARS: u32 = D + 0xFB8;
const ARC: u32 = D + 0x10F8;
const FLASH: u32 = D + 0x1478;
const ROD: u32 = D + 0x155C;
const SPIKES: u32 = D + 0x1594;
const FADE: u32 = D + 0x1C14;
const DROPS: u32 = D + 0x1C1C;
const WINDOW: u32 = D + 0x865C;
const TUBES: u32 = D + 0x868C;
const BYTES: u32 = D + 0x87AC;

/// The entry 0x1E06ED0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x88B0).
    fx.bzero(D, 0x88B0);
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
    for (at, node, clear) in [(0xCu32, 0x2E, 1), (0x10, 0x30, 0), (0x14, 0x1A, 0), (0x18, 0x10, 0), (0x1C, 0x4A, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The emitter group (+0xA4) and the light ramp (+0xA0).
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(D + 0xCC, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_7FF0);
    fx.mem.write_u32(D + 0xB0, 0x01E0_7FE8);
    fx.mem.write_u32(D + 0xA4, SPARKS);
    fx.mem.write_u8(D + 0xD4, 0x20);
    fx.mem.write_u32(D + 0xA8, D + 0x7E8);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(D + 0xD7, b);
    fx.mem.set16(D + 0xD8, 0x5F);
    fx.mem.set16(D + 0xDA, 0x65);
    fx.mem.write_u32(D + 0xDC, 0x4334_0000);
    fx.mem.write_u32(D + 0xE0, 0x4316_0000);
    fx.mem.write_u32(D + 0xE4, 0x40C0_0000);
    fx.mem.set16(LIGHT, 2);
    // The `sh` of +0xA2 sits in rand_bytes' delay slot: the constant.
    fx.mem.set16(LIGHT + 2, 0xCC);
    // The entry's s0 = D, s1 = 4 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_707C, [Some(D), Some(4), Some(param as u32)]);
    0
}

/// The camera shake (+0x40) [F]: `count` counts, `kind` (2 at node +0x10
/// / 1 at `pos`), amplitude `amp` on all three, `far` / `rate`.
fn shake(fx: &mut Fx, count: i32, kind: u8, amp: u32, far: u32, rate: u32, pos: Option<u32>) {
    set_counter4(fx, SHAKE, count, 0, 0xFF);
    if pos.is_some() {
        fx.mem.write_u8(SHAKE + 5, 0);
    } else {
        fx.mem.write_u8(SHAKE + 5, 1);
        let node = fx.mem.read_u8(D + 0x10);
        fx.mem.write_u8(SHAKE + 6, node);
    }
    fx.mem.write_u8(SHAKE + 4, kind);
    for o in [0x14u32, 0x18, 0x1C] {
        fx.mem.write_u32(SHAKE + o, amp);
    }
    fx.mem.write_u32(SHAKE + 0x30, 0);
    fx.mem.write_u32(SHAKE + 0x34, far);
    fx.mem.write_u32(SHAKE + 0x50, rate);
    if let Some(p) = pos {
        copy_v4(fx, p, SHAKE + 0x20);
    }
}

/// Update 2 [F 0x1E070F4]: the frame fade record (+0x865C).
fn window(fx: &mut Fx) {
    fx.mem.set16(WINDOW, 2);
    fx.mem.set16(WINDOW + 2, 0x18);
    fx.mem.write_u8(WINDOW + 5, 6);
    fx.mem.write_u8(WINDOW + 6, 1);
    fx.mem.write_u32(WINDOW + 8, 0);
    for o in [0x2Eu32, 0x2D, 0x2C] {
        fx.mem.write_u8(WINDOW + o, 0x80);
    }
}

/// Update 0x18 [F 0x1E07164]: four bars, the first flash and a shake at
/// node +0x10.
fn bars(fx: &mut Fx) {
    for k in 0..4i32 {
        let b = BARS + 0x50 * k as u32;
        set_counter4(fx, b, 0x12, 0, 0xFF);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 4, 0x80);
        match k {
            1 | 3 => {
                fx.mem.write_u32(b + 0x10, 0x4320_0000);
                fx.mem.setf(b + 0x18, ee::mul(45.0, ee::from_i32(k)));
                fx.mem.write_u32(b + 0x14, 0x4040_0000);
            }
            2 => {
                fx.mem.write_u32(b + 0x10, 0x43C8_0000);
                fx.mem.setf(b + 0x18, ee::mul(45.0, ee::from_i32(k)));
                fx.mem.write_u32(b + 0x14, 0x4080_0000);
            }
            _ => {
                fx.mem.write_u32(b + 0x10, 0x43C8_0000);
                fx.mem.write_u32(b + 0x18, 0);
                fx.mem.write_u32(b + 0x14, 0x4080_0000);
            }
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x60), (0xA, 0x80), (9, 0x50), (8, 0x10)] {
            fx.mem.write_u8(b + o, v);
        }
    }
    set_counter4(fx, FLASH, 0x13, 0, 0xFF);
    fx.mem.write_u8(FLASH + 5, 0);
    fx.mem.write_u8(FLASH + 4, 0x81);
    fx.mem.write_u8(FLASH + 7, 9);
    fx.mem.write_u32(FLASH + 0x10, 0x4316_0000);
    fx.mem.write_u32(FLASH + 0x14, 0);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x50), (8, 0x10)] {
        fx.mem.write_u8(FLASH + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(FLASH + o, 0);
    }
    shake(fx, 0xE, 2, 0x3F99_999A, 0x41F0_0000, 0x3F99_999A, None);
}

/// Update 0x28 [F 0x1E07330]: the arc (+0x10F8, pool +0x1158 of 8 + 1)
/// from node +0xC to node +0x10 and the rod between them.
fn arc(fx: &mut Fx) {
    set_counter4(fx, ARC, 0x1A, 0, 0xFF);
    fx.mem.write_u8(ARC + 4, 1);
    fx.mem.write_u32(ARC + 8, D + 0x1158);
    fx.mem.write_u8(ARC + 0x30, 0x32);
    fx.mem.write_u8(ARC + 0x31, 8);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(ARC + 0x32, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(ARC + 0x33, b);
    fx.mem.write_u32(ARC + 0x38, 0x3F80_0000);
    for (o, v) in [(0x47u32, 0xC0u8), (0x44, 0xC0), (0x45, 0xC0), (0x46, 0xC0), (0x4A, 0x80), (0x49, 0x60), (0x48, 0x10)] {
        fx.mem.write_u8(ARC + o, v);
    }
    let mut t0 = 0;
    while t0 < fx.mem.u8(ARC + 0x31) + 1 {
        fx.mem.set16(ARC + 0xC + 2 * t0 as u32, -1);
        fx.mem.set16(ARC + 0x1E + 2 * t0 as u32, -1);
        t0 += 1;
    }
    set_counter4(fx, ROD, 0x52, 0, 0xFF);
    fx.mem.write_u8(ROD + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(ROD + 8, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(ROD + 9, b);
    fx.mem.write_u8(ROD + 4, 2);
    fx.mem.write_u32(ROD + 0x10, 0x3DCC_CCCD);
    fx.mem.write_u32(ROD + 0x14, 0x3DCC_CCCD);
    fx.mem.write_u32(ROD + 0xC, 0x4248_0000);
    for (o, v) in [(0x18u32, 0xC0u8), (0x19, 0xC0), (0x1A, 0xC0), (0x1B, 0), (0x1E, 0xC0), (0x1D, 0x80), (0x1C, 0x60), (0x1F, 0)] {
        fx.mem.write_u8(ROD + o, v);
    }
}

/// Update 0x72 [F 0x1E0745C]: the fade, 400 drops and three frustums
/// about the target's position (sp + 0x70). Returns the entry's (s2, s3).
fn drops(fx: &mut Fx, sp: u32) -> (u64, u64) {
    set_counter4(fx, FADE, 0x10, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    let mut s2 = 0u64;
    let mut s3;
    for s4 in 0..0x190i32 {
        let i = first_free(fx, DROPS, 0x44, 0x190);
        s2 = i as i64 as u64;
        if i < 0 {
            continue;
        }
        let v = fx.rand_from(D, 0x01E0_749C);
        let count = (((v >> 4) & 0xF) + 0x26) & 0xFF;
        let r = DROPS + 0x44 * i as u32;
        s2 = u64::from(D + 0x44 * i as u32);
        // s4 / 40 by the 0x66666667 multiply.
        fx.mult(0x6666_6667, s4);
        s3 = (((0x6666_6667i64 * i64::from(s4)) >> 32) as i32 >> 4) + ((s4 as u32) >> 31) as i32;
        set_counter4(fx, r, count, s3 & 0xFF, 0xFF);
        fx.mem.write_u8(r + 4, 0x42);
        fx.mem.write_u8(r + 8, 1);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xA, 0);
        let v = fx.rand_from(D, 0x01E0_7508);
        fx.mem.set8(r + 9, ((v >> 4) & 3) + 2);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u32(r + 0x10, 0x42A0_0000);
        fx.mem.write_u32(r + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(r + 0x18, 0xBDA3_D70A);
        let rise = ee::mul(4.0, ee::from_i32(s3));
        let y = fx.mem.f(sp + 0x74);
        fx.mem.setf(r + 0x1C, ee::sub(y, 20.0));
        let x = fx.mem.read_u32(sp + 0x70);
        fx.mem.write_u32(r + 0x20, x);
        fx.mem.setf(r + 0x24, ee::add(y, rise));
        let z = fx.mem.read_u32(sp + 0x78);
        fx.mem.write_u32(r + 0x28, z);
        let v = fx.rand_from(D, 0x01E0_759C);
        let (_, rem) = fx.div(v >> 4, 100);
        fx.mem.setf(r + 0x2C, ee::div(ee::from_i32(rem - 50), 100.0));
        let v = fx.rand_from(D, 0x01E0_75D8);
        let (_, rem) = fx.div(v >> 4, 100);
        fx.mem.setf(r + 0x30, ee::add(f32::from_bits(0x3F66_6666), ee::div(ee::from_i32(rem - 50), 100.0)));
        let v = fx.rand_from(D, 0x01E0_7620);
        let (_, rem) = fx.div(v >> 4, 100);
        fx.mem.setf(r + 0x34, ee::div(ee::from_i32(rem - 50), 100.0));
    }
    // Three frustums (the first free of four, no failure test).
    for k in 0..3i32 {
        let i = first_free(fx, TUBES, 0x48, 4);
        let t = D.wrapping_add((0x48 * i) as u32) + 0x868C;
        s2 = u64::from(t - 0x868C);
        set_counter4(fx, t, 0x28, 0, 0xFF);
        for (o, v) in [(7u32, 0x24u8), (8, 3), (9, 2), (0xA, 0x20)] {
            fx.mem.write_u8(t + o, v);
        }
        fx.mem.write_u32(t + 0x10, 0x3F80_0000);
        fx.mem.write_u32(t + 0x14, 0x3C23_D70A);
        fx.mem.write_u32(t + 0x18, 0x3CA3_D70A);
        let y = fx.mem.f(sp + 0x74);
        let below = ee::sub(y, 20.0);
        let (step, ra, rb, grow, top) = match k {
            2 => (0x4040_0000u32, 0x40A0_0000u32, 0x4100_0000u32, 0x3FA6_6666u32, below),
            1 => (0x4040_0000, 0x40A0_0000, 0x4150_0000, 0x3F4C_CCCD, ee::sub(y, 10.0)),
            _ => (0x4000_0000, 0x4100_0000, 0x4100_0000, 0, ee::add(45.0, y)),
        };
        fx.mem.write_u32(t + 0xC, step);
        fx.mem.write_u32(t + 0x1C, ra);
        fx.mem.write_u32(t + 0x20, rb);
        fx.mem.write_u32(t + 0x24, 0);
        fx.mem.write_u32(t + 0x28, grow);
        fx.mem.setf(t + 0x30, below);
        fx.mem.setf(t + 0x3C, top);
        let x = fx.mem.read_u32(sp + 0x70);
        fx.mem.write_u32(t + 0x38, x);
        fx.mem.write_u32(t + 0x2C, x);
        let z = fx.mem.read_u32(sp + 0x78);
        fx.mem.write_u32(t + 0x40, z);
        fx.mem.write_u32(t + 0x34, z);
        for (o, v) in [(0x46u32, 0xA0u8), (0x47, 0x80), (0x44, 0x80), (0x45, 0x80)] {
            fx.mem.write_u8(t + o, v);
        }
    }
    (s2, 3)
}

/// Update 0x74 [F 0x1E07958]: six spikes, the second flash and a shake 20
/// below the target's position (sp + 0x70). Returns the entry's s2.
fn spikes(fx: &mut Fx, sp: u32) -> u64 {
    let mut s2 = 0u64;
    for s3 in 0..6i32 {
        let i = first_free(fx, SPIKES, 0x68, 0x10);
        if i < 0 {
            s2 = i as i64 as u64;
            continue;
        }
        let q = SPIKES + 0x68 * i as u32;
        s2 = u64::from(D + 0x68 * i as u32);
        fx.mem.write_u8(q + 4, 4);
        set_counter4(fx, q, 0x24, 0, 0xFF);
        let c = fx.mem.read_u8(q).wrapping_sub(0x10);
        fx.mem.write_u8(q, c);
        for (o, v) in [(0x27u32, 0x60u8), (0x23, 0x28), (5, 0), (0xA, 1), (0xB, 2), (0xC, 4)] {
            fx.mem.write_u8(q + o, v);
        }
        // The `sb` of +8 sits in rand's delay slot: the constant 0x50.
        fx.mem.write_u8(q + 8, 0x50);
        let v = fx.rand_from(D, 0x01E0_79E8);
        fx.mem.set8(q + 9, v >> 4);
        fx.mem.write_u32(q + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(q + o, 0x80);
        }
        copy_v4(fx, sp + 0x70, q + 0x28);
        let y = ee::sub(fx.mem.f(q + 0x2C), 20.0);
        fx.mem.setf(q + 0x2C, y);
        let len = ee::add(20.0, ee::mul(5.0, ee::from_i32(s3 + 1)));
        fx.mem.setf(q + 0x1C, len);
        let w = ee::mul(f32::from_bits(0x3ECC_CCCD), ee::from_i32(s3));
        fx.mem.setf(q + 0x14, ee::sub(2.5, w));
        fx.mem.setf(q + 0x18, ee::add(f32::from_bits(0x3ECC_CCCD), w));
    }
    let f = FLASH + 0x4C;
    set_counter4(fx, f, 0x2A, 0, 0xFF);
    fx.mem.write_u8(f + 5, 0);
    fx.mem.write_u8(f + 4, 0x81);
    fx.mem.write_u8(f + 7, 9);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x60), (9, 0x10), (8, 0x40)] {
        fx.mem.write_u8(f + o, v);
    }
    fx.mem.write_u32(f + 0x14, 0);
    fx.mem.write_u32(f + 0x10, 0x44AF_0000);
    copy_v4(fx, sp + 0x70, f + 0x1C);
    let y = ee::sub(fx.mem.f(f + 0x20), 20.0);
    fx.mem.setf(f + 0x20, y);
    shake(fx, 0x20, 1, 0x4000_0000, 0x42A0_0000, 0x40C0_0000, Some(sp + 0x70));
    s2
}

/// What the soft-double chain 0x1A2C30 / 0x1A31D8 / 0x1A3118 / 0x1A38F8
/// leaves below the entry's sp `top` [F, as b189 / b555]: 0x1A31D8's saves
/// of the entry's s1 / s2 / s3 at its frame +0x78.., then 0x1A38F8's
/// unpack of the sum into its frame +0 (a normal number's exponent - 0x3FF
/// at +8 and fraction << 8 | 1 << 60 at +0x10; a NaN's raw fraction at
/// +0x10).
fn soft_double_saved(fx: &mut Fx, top: u32, p: f64, s123: [u64; 3]) {
    fx.mem.write_u64(top - 0x38, s123[0]);
    fx.mem.write_u64(top - 0x30, s123[1]);
    fx.mem.write_u64(top - 0x28, s123[2]);
    let bits = p.to_bits();
    let exp = ((bits >> 52) & 0x7FF) as u32;
    let frac = bits & ((1 << 52) - 1);
    if exp == 0x7FF {
        if frac != 0 {
            fx.mem.write_u64(top - 0x30, frac);
        }
    } else if exp != 0 {
        fx.mem.write_u32(top - 0x38, exp.wrapping_sub(0x3FF));
        fx.mem.write_u64(top - 0x30, (frac << 8) | (1 << 60));
    }
}

/// While the first flash counts [F 0x1E07BD4]: sp + 0xA0 = 3 / 4 of the
/// way from node +0xC (sp + 0x80) to node +0x10 (sp + 0x90), each axis
/// (float) ((double) a + 0.75 x (double) (b - a)), copied (with its stale
/// w) to the bars' / the first spike's / the first flash's positions.
fn between(fx: &mut Fx, sp: u32, s3: u64) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, s, sp + 0x80, u64::from(D), Some(u64::from(OT)), 0x01E0_7BE4);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, s, sp + 0x90, u64::from(D), Some(u64::from(OT)), 0x01E0_7BF8);
    let mut s2 = u64::from(sp + 0x90);
    let mut s3 = s3;
    for c in 0..3u32 {
        let (a, b) = (fx.mem.f(sp + 0x80 + 4 * c), fx.mem.f(sp + 0x90 + 4 * c));
        let d = f64::from(ee::sub(b, a));
        let p = 0.75 * d;
        let sum = p + f64::from(a);
        soft_double_saved(fx, sp, sum, [u64::from(OT), s2, s3]);
        fx.mem.setf(sp + 0xA0 + 4 * c, sum as f32);
        s2 = f64::from(a).to_bits();
        s3 = u64::from(sp + 0xA0);
    }
    for to in [BARS + 0x50 * 3 + 0x20, BARS + 0x50 * 2 + 0x20, BARS + 0x50 + 0x20, BARS + 0x20, SPIKES + 0x28, FLASH + 0x1C] {
        copy_v4(fx, sp + 0xA0, to);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x70, u64::from(D), 0x01E0_709C);
    // The entry's s2 / s3 after the block (the parameter, the dispatch's
    // delay slot zeroes s3) [F]: the library prologues save them.
    let p = param as i64 as u64;
    let (s2, s3): (u64, u64) = match fx.mem.u16(D) {
        0xD2 => return 8,
        0x74 => (spikes(fx, sp), 6),
        0x72 => drops(fx, sp),
        0x28 => {
            arc(fx);
            (p, 0)
        }
        0x18 => {
            bars(fx);
            (u64::from(D + 0x140), 4)
        }
        2 => {
            window(fx);
            (p, 0)
        }
        _ => (p, 0),
    };
    let _ = s2;
    if fx.mem.read_u8(FLASH) != 0 {
        between(fx, sp, s3);
    }
    if fx.mem.read_u8(ROD) != 0 {
        let a = fx.mem.u8(ROD + 0x1B);
        if a < 0xC0 {
            fx.mem.set8(ROD + 0x1B, a + 9);
        }
    }
    shake_saving(fx, SHAKE, u64::from(D), 0x01E0_7DB8, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, LIGHT, n, 0x01E0_7DC4, [Some(u64::from(D)), Some(u64::from(OT))]);
    let n = fx.mem.u16(D);
    blib::spawn::emitters(fx, 0x01E0_0F40, GROUP, n);
    blib::sprite::sparks(fx, SPARKS, OT, 0x20);
    // [H] f22 = the runner's (0): the entry never sets it.
    blib::rod::rods_b(fx, 0x01E0_1390, ROD, OT, 1, 0.0);
    let (_, arc_a3) = blib::arc::arcs_b_a3(fx, 0x01E0_2370, ARC, OT, 1);
    let (_, flash_a3) = blib::star::flash_b_a3(fx, 0x01E0_3490, FLASH, OT, 3);
    let (_, bar_a3) = blib::cross::bars_b_a3(fx, 0x01E0_3F00, BARS, OT, 4);
    blib::spike::spikes(fx, 0x01E0_4800, SPIKES, OT, 0x10);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the entry's s0 / s1 (D / the OT), the
    // runner's s5 / s6 (0) and ra [F, as b601]; s2..s4 [H] left.
    fx.saved(sp - 0xF0, &[(0x10, u64::from(D)), (0x20, u64::from(OT)), (0x60, 0), (0x70, 0), (0x80, 0x01E0_7E3C)]);
    let (_, drop_a3) = blib::drop::drops_a3(fx, DROPS, OT, 0x190);
    let (_, tube_a3) = blib::segtube::frustums_a3(fx, 0x01E0_5C30, TUBES, OT, 4);
    let n = fx.mem.u16(D);
    // [H] the spikes' / fade's a3 not tracked (the window closes on 0x18 as
    // the bars start).
    blib::window::frame_fade_b(fx, 0x01E0_57B0, WINDOW, OT, n, tube_a3.or(drop_a3).or(bar_a3).or(flash_a3).unwrap_or(arc_a3));
    let n = fx.mem.u16(D);
    if (0x72..0x8F).contains(&n) {
        6
    } else if n == 0x6F {
        2
    } else if n == 0x68 {
        1
    } else {
        0
    }
}
