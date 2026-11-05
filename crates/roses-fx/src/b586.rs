//! BMODULE b586 (card 259), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E047A0, data area a0 = 0x1ED0000) [F] (helper B4, 31
//! Oct 2026): b076's "a0 = D" family (s0 = D, s1 = the OT). On update 2
//! two long flashes on the attacker's nodes 0x1A / 0x23 (+0x1904), a bolt
//! (+0x1DC4, `bolt::bolts`, between nodes 0x1B / 0x24) and a long camera
//! shake (+0x40); on even updates 2..225 two ribbons a update from those
//! nodes (+0xA4, pool of 60); on updates 5..127 five sprites a update
//! falling onto the nodes (+0x358C, pool of 80); on 88 at the target a
//! flash, a fade (+0x3584) and eight bead beams (+0x2EA4, staggered); on
//! 90 a wide flash and four segment spawners (+0x2CC4,
//! `emit::segment_spawns`, feeding the bolt pool); the stage light pulsed
//! by 0x1031C0 (+0xA0: down on update 2, up on 244). Returns 1 on update
//! 80, 6 on 90..236, 2 on 237, 8 on 250 (and at once for a parameter of
//! -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `ribbon` 0x1E00FC0, `bolts` 0x1E01B50, `segment_spawns`
//! 0x1E026A0, `beads` 0x1E02A20, `fade` 0x1E037F0, `bounce` 0x1E03D30
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x4ACC;
const FLASHES: u32 = D + 0x1904;
const BOLTS: u32 = D + 0x1DC4;
const SPRITES: u32 = D + 0x358C;

/// The entry 0x1E047A0 (a0 = D, a1 = param; s0 = D, s1 = the OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x4BD0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..2 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x1A, 1), (0x10, 0x23, 0), (0x14, 0x1B, 0), (0x18, 0x24, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xF4);
    // The entry's s0 = D, s1 = the OT, s2 = 2 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_48F4, [Some(D), Some(OT), Some(2)]);
    0
}

/// (rand >> 4) % 4 as the compiler's signed remainder.
fn rem4(fx: &mut Fx, ra: u32) -> i32 {
    let x = fx.rand_from(D, ra) >> 4;
    let mut r = x & 3;
    if x < 0 && r != 0 {
        r -= 4;
    }
    r
}

/// A flash of the pool at +0x1904 (`count`, blend `b4`, colour `c`, size
/// `w` x `h`, at `at` when given) [F 0x1E04B1C / 0x1E04CFC].
fn flash_start(fx: &mut Fx, count: i32, b4: u8, c: u8, size: [u32; 2], at: Option<u32>) -> Option<u32> {
    let i = first_free(fx, FLASHES, 0x4C, 0x10);
    if i < 0 {
        return None;
    }
    let r = FLASHES + 0x4C * i as u32;
    set_counter4(fx, r, count, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, b4);
    fx.mem.write_u8(r + 7, 9);
    for (o, v) in [(0xAu32, 0x40u8), (9, 0x20), (8, 0x80), (0xF, c), (0xC, c), (0xD, c), (0xE, c)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, size[0]);
    fx.mem.write_u32(r + 0x14, size[1]);
    if let Some(at) = at {
        copy_v4(fx, at, r + 0x1C);
    }
    Some(r)
}

/// Update 2 [F 0x1E04958]: two flashes on nodes +0xC / +0x10, a bolt
/// between nodes +0x14 / +0x18 and the shake on node +0x1C.
fn charge(fx: &mut Fx) {
    for k in 0..2u32 {
        let r = FLASHES + 0x4C * k;
        set_counter4(fx, r, 0xA0, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        for (o, v) in [(4u32, 0x85u8), (7, 9), (0xA, 0x40), (9, 0x20), (8, 0x80), (0xF, 0xB0), (0xC, 0xB0), (0xD, 0xB0), (0xE, 0xB0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x4348_0000);
        fx.mem.write_u32(r + 0x14, 0);
    }
    let i = first_free(fx, BOLTS, 0x40, 0x3C);
    if i < 0 {
        return;
    }
    let b = BOLTS + ((i as u32) << 6);
    set_counter4(fx, b, 0x46, 8, 0xFF);
    fx.mem.write_u8(b + 5, 1);
    fx.mem.write_u8(b + 4, 0);
    let v = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(b + 9, v);
    let v = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(b + 0xA, v);
    fx.mem.write_u8(b + 8, 7);
    fx.mem.write_u32(b + 0x10, 0x3F33_3333);
    fx.mem.write_u32(b + 0x14, 0x3E99_999A);
    for (o, v) in [(0xBu32, 1u8), (0xD, 4), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x30)] {
        fx.mem.write_u8(b + o, v);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0xFA, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let v = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(s + 6, v);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x3F99_999A);
    fx.mem.write_u32(s + 0x18, 0x3F99_999A);
    fx.mem.write_u32(s + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x4000_0000);
}

/// Update 88 [F 0x1E04B1C]: a flash, the fade (+0x3584) and eight bead
/// beams (+0x2EA4 + 0x58 k, delays k) at the target (sp + 0x80).
fn beads_start(fx: &mut Fx, sp: u32) {
    let at = sp + 0x80;
    if let Some(r) = flash_start(fx, 0x10, 0x81, 0xF0, [0x4348_0000, 0x4316_0000], None) {
        fx.mem.write_u32(r + 0x18, 0x4220_0000);
        copy_v4(fx, at, r + 0x1C);
    }
    set_counter4(fx, D + 0x3584, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0x3588, 1);
    for k in 0..8i32 {
        let r = D + 0x2EA4 + 0x58 * k as u32;
        set_counter4(fx, r, 0x20, k & 0xFF, 0xFF);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x1E);
        fx.mem.set8(r + 9, k);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xC, 2);
        let x = fx.rand_from(D, 0x01E0_4C78) >> 4;
        fx.mem.set8(r + 0xB, x & 3);
        fx.mem.write_u32(r + 0x1C, 0x4270_0000);
        fx.mem.write_u32(r + 0x20, 0xBF00_0000);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x3FC0_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
    }
}

/// Update 90 [F 0x1E04CFC]: a wide flash and four segment spawners
/// (+0x2CC4 + 0x3C k, delays k) at the target (sp + 0x80).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x80;
    flash_start(fx, 0x96, 0x83, 0xC0, [0x447A_0000, 0], Some(at));
    for k in 0..4i32 {
        let r = D + 0x2CC4 + 0x3C * k as u32;
        set_counter4(fx, r, 0x8C, k & 0xFF, 3);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u32(r + 8, BOLTS);
        for (o, v) in [(0xCu32, 0x28u8), (0xD, 0xC), (0xE, 1), (0xF, 0), (0x10, 6), (0x11, 0x10)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.set32(r + 0x14, 0x1388);
        fx.mem.write_u32(r + 0x18, 0x3F4C_CCCD);
        fx.mem.write_u32(r + 0x1C, 0x3ECC_CCCD);
        for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x30)] {
            fx.mem.write_u8(r + o, v);
        }
        for c in 0..3u32 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(r + 0x24 + 4 * c, w);
        }
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(r + o, 0);
        }
    }
}

/// Even updates 2..225 [F 0x1E04ED4]: a ribbon from each of nodes +0xC /
/// +0x10 (thin before update 90).
fn ribbons(fx: &mut Fx) {
    for k in 0..2u32 {
        let i = first_free(fx, D + 0xA4, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let r = D + 0xA4 + 0x68 * i as u32;
        set_counter4(fx, r, 0x10, 0, 0xFF);
        fx.mem.write_u8(r + 4, 5);
        if fx.mem.u16(D) < 0x5A {
            fx.mem.write_u32(r + 0x14, 0x3F19_999A);
            fx.mem.write_u32(r + 0x1C, 0x4170_0000);
            fx.mem.write_u8(r + 8, 8);
        } else {
            fx.mem.write_u32(r + 0x14, 0x4080_0000);
            fx.mem.write_u32(r + 0x1C, 0x4220_0000);
            fx.mem.write_u8(r + 8, 0x1E);
        }
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 3);
        let x = fx.rand_from(D, 0x01E0_4FD0);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x23, 0x10);
        fx.mem.write_u8(r + 0x27, 0x40);
        for (o, ra) in [(0x58u32, 0x01E0_5020u32), (0x5C, 0x01E0_5038), (0x60, 0x01E0_5050)] {
            let x = fx.rand_from(D, ra) >> 4;
            fx.mem.set32(r + o, x & 0x7F);
        }
    }
}

/// Updates 5..127 [F 0x1E05094]: five sprites a update, each from a
/// random offset (+-4) of node +0xC / +0x10 in turn, falling back onto it
/// over its count (16 + rand % 6 x 2 - 6, 0x1A2B80).
fn gather(fx: &mut Fx, sp: u32) {
    for k in 0..5u32 {
        let i = first_free(fx, SPRITES, 0x44, 0x50);
        if i < 0 {
            continue;
        }
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0xC + 4 * (k & 1));
        node_position_saving(fx, h, n, sp + 0x90, u64::from(D), Some(u64::from(OT)), 0x01E0_50E0);
        let x = fx.rand_from(D, 0x01E0_50E8) >> 4;
        let (_, m) = fx.div(x, 6);
        let life = ee::add(16.0, ee::div(ee::from_i32(m * 200 - 0x258), 100.0));
        let c = ee::fixsfsi(life);
        fx.fix_residue(life, 0x01E0_5150);
        let b = SPRITES + 0x44 * i as u32;
        set_counter4(fx, b, c & 0xFF, 0, 0xFF);
        for (o, v) in [(4u32, 0x41u8), (8, 0), (0xB, 2), (9, 3), (0xA, 0), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(b + o, v);
        }
        fx.mem.write_u32(b + 0x10, 0x41A0_0000);
        fx.mem.write_u32(b + 0x14, 0xBF00_0000);
        fx.mem.write_u32(b + 0x1C, 0);
        fx.mem.write_u32(b + 0x18, 0);
        for (c, ra) in [0x01E0_51D0u32, 0x01E0_5234, 0x01E0_5298].into_iter().enumerate() {
            let m = rem4(fx, ra);
            let j = ee::div(ee::from_i32(m * 200 - 0x190), 100.0);
            fx.mem.setf(sp + 0xA0 + 4 * c as u32, j);
        }
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0x90 + 4 * c), fx.mem.f(sp + 0xA0 + 4 * c));
            fx.mem.setf(b + 0x20 + 4 * c, v);
        }
        for c in 0..3u32 {
            let v = ee::neg(ee::div(fx.mem.f(sp + 0xA0 + 4 * c), life));
            fx.mem.setf(b + 0x2C + 4 * c, v);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(b + o, 0);
        }
    }
}

/// `segment_spawns` called from the entry from return address `ra`,
/// leaving its frame's (0x80) saved s4's upper half (0: the runner's) +0x58
/// and `ra` +0x60: the missing-texture TEX0 / TEX1 of the sprites / beads
/// drawn after it read these words [run b586, b559; F 0x1E026A0].
pub(crate) fn segment_spawns_saving(fx: &mut Fx, at: u32, g: u32, n: i32, ra: u32) -> i32 {
    let r = blib::emit::segment_spawns(fx, at, g, n);
    let f = fx.sp - 0x80;
    fx.mem.write_u64(f + 0x58, 0);
    fx.saved(f, &[(0x60, u64::from(ra))]);
    r
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x80, u64::from(D), 0x01E0_491C);
    match fx.mem.u16(D) {
        0xFA => return 8,
        0x5A => strike(fx, sp),
        0x58 => beads_start(fx, sp),
        2 => charge(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0xE2).contains(&n) && n & 1 == 0 {
        ribbons(fx);
    }
    let n = fx.mem.u16(D);
    if (5..0x80).contains(&n) {
        gather(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_5384, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5394, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0x3584, OT);
    blib::star::flash(fx, 0x01E0_0410, FLASHES, OT, 0x10);
    blib::streak::ribbon(fx, 0x01E0_0FC0, D + 0xA4, OT, 0x3C);
    segment_spawns_saving(fx, 0x01E0_26A0, D + 0x2CC4, 8, 0x01E0_53E0);
    blib::bolt::bolts(fx, 0x01E0_1B50, BOLTS, OT, 0x28);
    blib::beam::beads(fx, 0x01E0_2A20, D + 0x2EA4, OT, 0x14);
    blib::sprite::bounce(fx, SPRITES, OT, 0x50);
    let n = fx.mem.u16(D);
    if (0x5A..0xED).contains(&n) {
        6
    } else if n == 0xED {
        2
    } else if n == 0x50 {
        1
    } else {
        0
    }
}
