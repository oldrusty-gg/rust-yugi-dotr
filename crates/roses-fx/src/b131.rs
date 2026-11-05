//! BMODULE b131 (card 110), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E065F0, data area a0 = 0x1ED0000, 0x2690 bytes)
//! [F]: s0 = D, s1 = the OT (0x206E40), s2 the
//! parameter then a record pointer, s3 / s4 counters. b596's family. Init
//! sets the spark-emitter group (+0xA4, `spawn::emitters` /
//! `sprite::sparks`: update lists 0x1E07240 / 0x1E07260 at nodes 0x3B /
//! 0x35, the late node 0x1D, window 0..0), the light ramp (+0xA0: 2 /
//! 0x1B2) and the random byte table (+0x2584). Update 2: 16 scatter drops
//! (+0x1338, `drop::scatter_drops_b`, 0x30 counts, cell 3 of layout 2,
//! random corner order, 80 wide, spread 40) and 8 more (layout 1, 200
//! wide, spread 1) at node 0x1C, a flash there (+0x18D8, `star::flash_b`,
//! 0x30 counts, 200 wide) and a shake (+0x40, 0x30). Updates 49 and 352:
//! 8 + 8 drops (0x9A counts, 150 wide), the shake again. Update 266: an
//! arc (+0xFB8, `arc::arcs_b`, pool +0x1018, 0x15 counts) from node 0x1C
//! to node 0x1D. Update 280: the bead emitter (+0x24F8,
//! `emit::bead_trails_d`, 3 counts) at node 0x1C feeding the beams
//! (+0x1E18, `beam::beads_at`, 0x14). Update 282 at node 0x1C: five rays
//! (+0x1AFC, `ray::rays_b`, 0x16 - 4 counts, 40 rays each, a random
//! table cursor, turned 0xD000 + 0x1800 k),
//! three cones (+0x1D40, `cone::cones_c`, 0x10 counts, delays 0 / 3 / 6),
//! a flash (0x18 counts, 800 wide) and a shake (0x18, 80). Returns 2 on
//! update 279, 1 on 272, 8 on 440 (no drawing then) and at once for -2 /
//! -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080 (0538257fe1fa),
//! `sparks` 0x1E003A0, `fan_ring` 0x1E00C70, `emitters` 0x1E00F40,
//! `arcs_b` 0x1E01390, `rays_b` 0x1E024B0, `beads_at` 0x1E02F40,
//! `bead_trails_d` 0x1E03BC0, `bars_b` 0x1E040D0 (never spawned),
//! `flash_b` 0x1E049D0, `scatter_drops_b` 0x1E05440, `cones_c` 0x1E05DE0
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const ARC: u32 = D + 0xFB8;
const DROPS: u32 = D + 0x1338;
const FLASH: u32 = D + 0x18D8;
const BARS: u32 = D + 0x19BC;
const RAYS: u32 = D + 0x1AFC;
const CONES: u32 = D + 0x1D40;
const BEADS: u32 = D + 0x1E18;
const EMIT: u32 = D + 0x24F8;
const BYTES: u32 = D + 0x2584;

/// The entry 0x1E065F0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x2690).
    fx.bzero(D, 0x2690);
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
    for (at, node, clear) in [(0xCu32, 0x1C, 1), (0x10, 0x1D, 0), (0x14, 0x35, 0), (0x18, 0x3B, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The emitter group (+0xA4) and the light ramp (+0xA0).
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(D + 0xCC, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_7240);
    fx.mem.write_u32(D + 0xB0, 0x01E0_7260);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x20);
    fx.mem.write_u32(D + 0xA8, D + 0x7E8);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xD7, b);
    fx.mem.write_u16(D + 0xD8, 0);
    fx.mem.write_u32(D + 0xDC, 0x42C8_0000);
    fx.mem.write_u32(D + 0xE4, 0x40A0_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x1B2);
    // The entry's s0 = D, s1 = 3 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_676C, [Some(D), Some(3), Some(param as u32)]);
    0
}

/// `k` scatter drops from record `i` on (0x3C apart) [F 0x1E067F4 /
/// 0x1E069BC]: counted `count`, delayed by their index (`mask`ed), at node
/// +0xC; `big`: cell 3 of layout 2, a random corner order with flags
/// `flags`, 80 wide, alpha 0xC0, spread 40; else layout 1, flag 1, alpha
/// 0x20, `wide`, spread 1.
#[allow(clippy::too_many_arguments)]
fn drops(fx: &mut Fx, i: i32, k: i32, count: i32, mask: i32, big: bool, flags: u8, wide: u32, ra: u32) {
    for s3 in i..i + k {
        let r = DROPS + 0x3C * s3 as u32;
        set_counter4(fx, r, count, s3 & mask, 7);
        fx.mem.write_u8(r + 5, 1);
        let node = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(r + 6, node);
        if big {
            let v = ((fx.rand_from(D, ra) >> 4) & 3) << 4;
            fx.mem.write_u8(r + 4, (v as u8) | flags);
            for (o, v) in [(8u32, 2u8), (9, 3), (0xA, 2), (0xB, 0), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0xC0)] {
                fx.mem.write_u8(r + o, v);
            }
            fx.mem.write_u32(r + 0x10, 0x42A0_0000);
            fx.mem.set32(r + 0x14, 0x28);
        } else {
            for (o, v) in [(4u32, 1u8), (8, 0), (9, 3), (0xA, 1), (0xB, 0), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x20)] {
                fx.mem.write_u8(r + o, v);
            }
            fx.mem.write_u32(r + 0x10, wide);
            fx.mem.set32(r + 0x14, 1);
        }
    }
}

/// The camera shake (+0x40) [F]: `count` counts, `kind` (4 at the unit's
/// node +0xC, 1 at `pos`), amplitudes `amp`, `far` / `rate`.
fn shake(fx: &mut Fx, count: i32, kind: u8, amp: [u32; 3], far: u32, rate: u32, pos: Option<u32>) {
    set_counter4(fx, SHAKE, count, 0, 0xFF);
    if pos.is_some() {
        fx.mem.write_u8(SHAKE + 5, 0);
    } else {
        fx.mem.write_u8(SHAKE + 5, 1);
        let node = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(SHAKE + 6, node);
    }
    fx.mem.write_u8(SHAKE + 4, kind);
    for (k, a) in amp.into_iter().enumerate() {
        fx.mem.write_u32(SHAKE + 0x14 + 4 * k as u32, a);
    }
    fx.mem.write_u32(SHAKE + 0x30, 0);
    fx.mem.write_u32(SHAKE + 0x34, far);
    fx.mem.write_u32(SHAKE + 0x50, rate);
    if let Some(p) = pos {
        copy_v4(fx, p, SHAKE + 0x20);
    }
}

/// The flash (+0x18D8) [F]: `count` counts, flags / colour `c` / sizes
/// `w`, at the unit's node +0xC (`pos` `None`) or `pos`.
fn flash(fx: &mut Fx, count: i32, flags: u8, c: [u8; 3], w: u32, pos: Option<u32>) {
    set_counter4(fx, FLASH, count, 0, 0xFF);
    fx.mem.write_u8(FLASH + 5, u8::from(pos.is_none()));
    fx.mem.write_u8(FLASH + 4, flags);
    fx.mem.write_u8(FLASH + 7, 9);
    for o in [0xFu32, 0xC, 0xD, 0xE] {
        fx.mem.write_u8(FLASH + o, 0xC0);
    }
    fx.mem.write_u8(FLASH + 0xA, c[0]);
    fx.mem.write_u8(FLASH + 9, c[1]);
    fx.mem.write_u8(FLASH + 8, c[2]);
    fx.mem.write_u32(FLASH + 0x14, 0);
    fx.mem.write_u32(FLASH + 0x10, w);
    if let Some(p) = pos {
        copy_v4(fx, p, FLASH + 0x1C);
    }
}

/// Update 2 [F 0x1E067F0].
fn start(fx: &mut Fx) {
    drops(fx, 0, 16, 0x30, 7, true, 0x81, 0, 0x01E0_681C);
    drops(fx, 16, 8, 0x30, 7, false, 0, 0x4348_0000, 0);
    flash(fx, 0x30, 0x85, [0x60, 0x60, 0], 0x4348_0000, None);
    shake(fx, 0x30, 4, [0x4000_0000, 0x3FC0_0000, 0x3F80_0000], 0x4270_0000, 0x4040_0000, None);
}

/// Updates 49 and 352 [F 0x1E069B8].
fn again(fx: &mut Fx) {
    drops(fx, 0, 8, 0x9A, 0xFF, true, 1, 0, 0x01E0_69E4);
    drops(fx, 8, 8, 0x9A, 7, false, 0, 0x4316_0000, 0);
    shake(fx, 0x9A, 4, [0x4000_0000, 0x3FC0_0000, 0x3F80_0000], 0x4270_0000, 0x4040_0000, None);
}

/// Update 266 [F 0x1E06B20]: the arc (+0xFB8, pool +0x1018 of 9 + 1).
fn arc(fx: &mut Fx) {
    set_counter4(fx, ARC, 0x15, 0, 0xFF);
    fx.mem.write_u8(ARC + 4, 1);
    fx.mem.write_u32(ARC + 8, D + 0x1018);
    fx.mem.write_u8(ARC + 0x30, 0x32);
    fx.mem.write_u8(ARC + 0x31, 8);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(ARC + 0x32, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(ARC + 0x33, b);
    fx.mem.write_u32(ARC + 0x38, 0x3F80_0000);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0xC0), (0x4A, 0x80), (0x49, 0x10), (0x48, 0x10)] {
        fx.mem.write_u8(ARC + o, v);
    }
    let mut t0 = 0;
    while t0 < fx.mem.u8(ARC + 0x31) + 1 {
        fx.mem.set16(ARC + 0xC + 2 * t0 as u32, -1);
        fx.mem.set16(ARC + 0x1E + 2 * t0 as u32, -1);
        t0 += 1;
    }
}

/// Update 280 [F 0x1E06BD0]: the bead emitter (+0x24F8) at node +0xC.
fn emitter(fx: &mut Fx) {
    set_counter4(fx, EMIT, 3, 0, 0xFF);
    fx.mem.write_u8(EMIT + 4, 2);
    fx.mem.write_u8(EMIT + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(EMIT + 6, b);
    fx.mem.write_u32(EMIT + 8, BEADS);
    for (o, v) in [(0x10u32, 0x14u8), (0x12, 0x2D), (0x11, 0x20), (0x13, 1), (0x16, 2), (0x14, 3)] {
        fx.mem.write_u8(EMIT + o, v);
    }
    fx.mem.write_u32(EMIT + 0x18, 0x3F80_0000);
    fx.mem.write_u32(EMIT + 0x1C, 0x3E99_999A);
    fx.mem.write_u32(EMIT + 0x20, 0x41F0_0000);
    fx.mem.write_u32(EMIT + 0xC, BYTES);
    for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
        fx.mem.write_u8(EMIT + o, 0x80);
    }
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, s, EMIT + 0x3C, u64::from(D), Some(u64::from(OT)), 0x01E0_6C70);
    copy_v4(fx, EMIT + 0x3C, EMIT + 0x4C);
}

/// Update 282 [F 0x1E06C94]: the strike at node +0xC (sp + 0x70).
fn strike(fx: &mut Fx, sp: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, s, sp + 0x70, u64::from(D), Some(u64::from(OT)), 0x01E0_6CA4);
    // Five rays (+0x1AFC, 0x74 apart), turned 0xD000 + 0x1800 k.
    let mut s4 = 0xD000u32;
    for s2 in 0..5u32 {
        let r = RAYS + 0x74 * s2;
        set_counter4(fx, r, 0x16, 0, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(4);
        fx.mem.write_u8(r, c);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u8(r + 5, 0);
        // The `sb` of +8 sits in rand's delay slot: the constant before it.
        fx.mem.write_u8(r + 8, 0x28);
        let v = fx.rand_from(D, 0x01E0_6CE4);
        fx.mem.set8(r + 9, v >> 4);
        fx.mem.write_u32(r + 0x18, 0x4270_0000);
        fx.mem.write_u32(r + 0xC, BYTES);
        fx.mem.write_u32(r + 0x10, 0x4060_0000);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        for (o, v) in [(0x1Eu32, 0xC0u8), (0x1D, 0), (0x1C, 0), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x70, r + 0x24);
        fx.mem.write_u32(r + 0x54, s4 & 0xFFFF);
        s4 = s4.wrapping_add(0x1800);
    }
    // Three cones (+0x1D40, 0x48 apart), delayed 0 / 3 / 6.
    for s4 in 0..3u32 {
        let r = CONES + 0x48 * s4;
        set_counter4(fx, r, 0x10, (3 * s4) as i32 & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0xC, 0x4040_0000);
        fx.mem.write_u32(r + 0x10, 0x3E4C_CCCD);
        fx.mem.write_u32(r + 0x14, 0x3F00_0000);
        fx.mem.write_u32(r + 0x18, 0);
        fx.mem.write_u32(r + 0x1C, 0x3DCC_CCCD);
        for (o, v) in [(0x23u32, 0x80u8), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x26, 0xC0), (0x25, 0), (0x24, 0), (0x27, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x70, r + 0x28);
    }
    flash(fx, 0x18, 0x81, [0xC0, 0x10, 0x10], 0x4448_0000, Some(sp + 0x70));
    shake(fx, 0x18, 1, [0x4000_0000, 0x4000_0000, 0x4000_0000], 0x42A0_0000, 0x40C0_0000, Some(sp + 0x70));
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_678C);
    // The entry's s2 / s3 / s4 after the block (the parameter -1, the
    // runner's 0 / 0, else the last record pointer / counters) [F]: the
    // library prologues save them.
    let (s2, s3, s4): (u64, u64, u64) = match fx.mem.u16(D) {
        0x1B8 => return 8,
        0x11A => {
            strike(fx, sp);
            (9, u64::from(D + 0xD8), 3)
        }
        0x118 => {
            emitter(fx);
            (u64::MAX, 0, 0)
        }
        0x10A => {
            arc(fx);
            (u64::MAX, 0, 0)
        }
        0x160 | 0x31 => {
            again(fx);
            (u64::from(D + 0x3C0), 0x10, 0)
        }
        2 => {
            start(fx);
            (u64::from(D + 0x5A0), 0x18, 0)
        }
        _ => (u64::MAX, 0, 0),
    };
    shake_saving(fx, SHAKE, u64::from(D), 0x01E0_6F30, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6F3C, [Some(u64::from(D)), Some(u64::from(OT))]);
    let n = fx.mem.u16(D);
    blib::spawn::emitters(fx, 0x01E0_0F40, D + 0xA4, n);
    blib::sprite::sparks(fx, D + 0xE8, OT, 0x20);
    blib::arc::arcs_b(fx, 0x01E0_1390, ARC, OT, 1);
    blib::drop::scatter_drops_b(fx, 0x01E0_5440, DROPS, OT, 0x18);
    blib::star::flash_b(fx, 0x01E0_49D0, FLASH, OT, 3);
    blib::cross::bars_b(fx, 0x01E0_40D0, BARS, OT, 4);
    // bars_b's 0x230 frame saves s0..s7 / fp / ra at +0x20..+0xB0 (its
    // records never count here): bead_trails_d's resampled path reads the
    // s4..s7 slots as the beads' origin w [run, store watch].
    fx.saved(fx.sp - 0x230, &[(0x20, u64::from(D)), (0x30, u64::from(OT)), (0x40, s2), (0x50, s3), (0x60, s4), (0x70, 0), (0x80, 0), (0x90, 0), (0xA0, 0), (0xB0, 0x01E0_6F98)]);
    blib::ray::rays_b(fx, 0x01E0_24B0, RAYS, OT, 5);
    blib::cone::cones_c(fx, 0x01E0_5DE0, CONES, OT, 3);
    blib::emit::bead_trails_d(fx, 0x01E0_3BC0, EMIT, 1);
    blib::beam::beads_at(fx, 0x01E0_2F40, 0x34, 0x3C, BEADS, OT, 0x14);
    match fx.mem.u16(D) {
        0x117 => 2,
        0x110 => 1,
        _ => 0,
    }
}
