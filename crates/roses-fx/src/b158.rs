//! BMODULE b158 (card 237), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E041A0, data area a0 = 0x1ED0000) [F] (helper A18,
//! 31 Oct 2026): b201's family (s3 = D, s0 = the OT) with the drops
//! efb41f6a08dd (`drop::drops_c`). On update 2 the point +0x20E4 at the
//! attacker's node +0x10, 1 lower; on 32 eight drops (+0xA0, 0x44 each, 100
//! slots: 0x18 counts, cell rand & 3 + 2 of the flag-8 run, 60 wide growing
//! 8, falling 0.01) at the attacker's node +0xC scattered +-0.5 in x / z
//! (y = +0x20E8) and thrown +-0.3; on 42..49 an object op 0x1F83B0 (mode 4,
//! colours (0x80, 0x80, 0x80, 0x70)); on 50 eight bead beams (+0x1B30, 0x58
//! each, 0x24 - 4 counts after rand & 7, kind 5, 100 long, row k, 18 wide)
//! at that node; on 50..80 six drops an update (16 counts, cell rand & 7,
//! 120 wide growing 25) up the node in 2 steps, thrown 0.4 k; on 80 at the
//! target a halo of 800 (+0x1EC8), two spikes (+0x1F14: 0x1C counts 1.4 /
//! 60, and 0x20 counts flag 2 1.0 / 40) and a shake (+0x40, 80 at the
//! target); on 80 / 84 / 88 a cone (+0x1DF0, 0x48 each, 3 slots: 0x12
//! counts, 3 / 0.4 / 2, 0.16, turned +-0x4000 by the facing). Returns 1 on
//! update 70, 6 on 80..106, 2 on 107, 8 on 140 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` (ac7dccf5ae36) 0x1E00080,
//! `drops_c` 0x1E00440, `beads_at 40 44` 0x1E00E40, `cones_b` 0x1E01D50,
//! `halo` 0x1E02720, `spikes_b` 0x1E03470 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const DROPS: u32 = D + 0xA0;
const BEADS: u32 = D + 0x1B30;
const CONES: u32 = D + 0x1DF0;
const HALO: u32 = D + 0x1EC8;
const SPIKES: u32 = D + 0x1F14;
const POINT: u32 = D + 0x20E4;

/// The entry 0x1E041A0 (a0 = D, a1 = param; s3 = D, s1 = param, s0 = the
/// OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x2100);
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
    for (at, node, clear) in [(0xCu32, 0x18, 1), (0x10, 6, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The entry's s0 = 2 (the texture loop's), s1 = the parameter, s2 =
    // the runner's 0 [H, as b201].
    rand_bytes(fx, D + 0x1FE4, 0x100, 0x01E0_42AC, [Some(2), Some(param as u32), Some(0)]);
    0
}

/// ((rand >> 4) % m + add) / 100 [F].
fn spread(fx: &mut Fx, ra: u32, m: i32, add: i32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h + add), 100.0)
}

/// The attacker's node +0xC into sp + 0x60 (the entry's s1 = `s1`).
fn node_c(fx: &mut Fx, sp: u32, s1: i32, ra: u32) {
    let h = fx.mem.i32(D + 4);
    let slot = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, slot, sp + 0x60, u64::from(OT), Some(s1 as i64 as u64), ra);
}

/// A drop's head (+4 the corner order | 4, +8 1, cell +0xA by `cell`).
fn drop_head(fx: &mut Fx, r: u32, count: i32, ras: [u32; 2], cell: impl Fn(i32) -> i32) {
    set_counter4(fx, r, count, 0, 0xFF);
    let x = fx.rand_from(OT, ras[0]);
    fx.mem.set8(r + 4, (((x >> 4) & 3) << 4) | 4);
    fx.mem.write_u8(r + 9, 0);
    fx.mem.write_u8(r + 8, 1);
    let x = fx.rand_from(OT, ras[1]);
    fx.mem.set8(r + 0xA, cell(x));
    for (o, v) in [(0xBu32, 0u8), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
}

/// Update 0x20 [F 0x1E043C8]: eight drops at the node +0xC.
fn drops_start(fx: &mut Fx, sp: u32) {
    for s1 in 0..8 {
        let i = first_free(fx, DROPS, 0x44, 0x64);
        if i < 0 {
            continue;
        }
        let r = DROPS + 0x44 * i as u32;
        drop_head(fx, r, 0x18, [0x01E0_4424, 0x01E0_447C], |x| ((x >> 4) & 3) + 2);
        fx.mem.write_u32(r + 0x10, 0x4270_0000);
        fx.mem.write_u32(r + 0x14, 0x4100_0000);
        fx.mem.write_u32(r + 0x18, 0xBC23_D70A);
        fx.mem.write_u32(r + 0x1C, 0);
        node_c(fx, sp, s1, 0x01E0_4578);
        let v = spread(fx, 0x01E0_4580, 0x64, -0x32);
        let w = ee::add(fx.mem.f(sp + 0x60), v);
        fx.mem.setf(r + 0x20, w);
        let y = fx.mem.read_u32(D + 0x20E8);
        fx.mem.write_u32(r + 0x24, y);
        let v = spread(fx, 0x01E0_45F4, 0x64, -0x32);
        let w = ee::add(fx.mem.f(sp + 0x68), v);
        fx.mem.setf(r + 0x28, w);
        let v = spread(fx, 0x01E0_4650, 0x3C, -0x1E);
        fx.mem.setf(r + 0x2C, v);
        fx.mem.write_u32(r + 0x30, 0x3E4C_CCCD);
        let v = spread(fx, 0x01E0_46C0, 0x3C, -0x1E);
        fx.mem.setf(r + 0x34, v);
    }
}

/// Updates 0x32..0x50 [F 0x1E05158]: six drops up the node +0xC.
fn drops_rise(fx: &mut Fx, sp: u32) {
    for s1 in 0..6 {
        let i = first_free(fx, DROPS, 0x44, 0x64);
        if i < 0 {
            continue;
        }
        let r = DROPS + 0x44 * i as u32;
        drop_head(fx, r, 0x10, [0x01E0_51B4, 0x01E0_520C], |x| (x >> 4) & 7);
        fx.mem.write_u32(r + 0x10, 0x42F0_0000);
        fx.mem.write_u32(r + 0x14, 0x41C8_0000);
        fx.mem.write_u32(r + 0x18, 0xBCA3_D70A);
        fx.mem.write_u32(r + 0x1C, 0);
        node_c(fx, sp, s1, 0x01E0_5304);
        let v = spread(fx, 0x01E0_530C, 0x50, -0x28);
        let w = ee::add(fx.mem.f(sp + 0x60), v);
        fx.mem.setf(r + 0x20, w);
        let y = fx.mem.read_u32(D + 0x20E8);
        fx.mem.write_u32(r + 0x24, y);
        let z = ee::add(fx.mem.f(sp + 0x68), ee::mul(2.0, ee::from_i32(s1)));
        fx.mem.setf(r + 0x28, z);
        let v = spread(fx, 0x01E0_53B4, 0x3C, -0x1E);
        fx.mem.setf(r + 0x2C, v);
        fx.mem.write_u32(r + 0x30, 0x3ECC_CCCD);
        fx.mem.setf(r + 0x34, ee::mul(f32::from_bits(0x3ECC_CCCD), ee::from_i32(s1)));
    }
}

/// Update 0x32 [F 0x1E0477C]: eight bead beams at the node +0xC.
fn beads_start(fx: &mut Fx) {
    for s1 in 0..8u32 {
        let r = BEADS + 0x58 * s1;
        let x = fx.rand_from(OT, 0x01E0_4790);
        set_counter4(fx, r, 0x24, (x >> 4) & 7, 0xFF);
        let c = fx.mem.read_u8(r);
        fx.mem.write_u8(r, c.wrapping_sub(4));
        for (o, v) in [(4u32, 5u8), (5, 0), (8, 0x64), (9, s1 as u8), (0xA, 1), (0xC, 2)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_48A8);
        fx.mem.set8(r + 0xB, (x & 3) + 4);
        fx.mem.write_u32(r + 0x1C, 0x4190_0000);
        fx.mem.write_u32(r + 0x20, 0);
        fx.mem.write_u32(r + 0x10, D + 0x1FE4);
        fx.mem.write_u32(r + 0x14, 0x4026_6666);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0xC);
        node_position_saving(fx, h, slot, r + 0x28, u64::from(OT), Some(u64::from(s1)), 0x01E0_49EC);
    }
}

/// Update 0x50 [F 0x1E04A04]: the halo, two spikes and the shake at the
/// target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    let r = HALO;
    set_counter4(fx, r, 0x20, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x10), (8, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0);
    fx.mem.write_u32(r + 0x10, 0x4448_0000);
    copy_v4(fx, sp + 0x50, r + 0x1C);
    for s1 in 0..2u32 {
        let r = SPIKES + 0x68 * s1;
        fx.mem.write_u8(r + 4, 5);
        if s1 & 1 != 0 {
            set_counter4(fx, r, 0x20, 0, 0xFF);
            let f = fx.mem.read_u8(r + 4);
            fx.mem.write_u8(r + 4, f | 2);
            fx.mem.write_u32(r + 0x14, 0x3F80_0000);
            fx.mem.write_u32(r + 0x1C, 0x4220_0000);
            fx.mem.write_u8(r + 0x23, 0x80);
            fx.mem.write_u8(r + 0x27, 0x80);
        } else {
            set_counter4(fx, r, 0x1C, 0, 0xFF);
            fx.mem.write_u32(r + 0x14, 0x3FB3_3333);
            fx.mem.write_u32(r + 0x1C, 0x4270_0000);
            fx.mem.write_u8(r + 0x27, 0xC0);
            fx.mem.write_u8(r + 0x23, 0);
        }
        for (o, v) in [(5u32, 0u8), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0x50)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_4CF4);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, D + 0x1FE4);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0x50, r + 0x28);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x18, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x50, s + 0x20);
}

/// Updates 0x50 / 0x54 / 0x58 [F 0x1E04EC4]: a cone at the target.
fn cone(fx: &mut Fx, sp: u32) {
    let i = first_free(fx, CONES, 0x48, 3);
    if i < 0 {
        return;
    }
    let r = CONES + 0x48 * i as u32;
    set_counter4(fx, r, 0x12, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, 1), (0xA, 0x20), (9, 7)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0xC, 0x4040_0000);
    fx.mem.write_u32(r + 0x10, 0x3ECC_CCCD);
    fx.mem.write_u32(r + 0x14, 0x4000_0000);
    fx.mem.write_u32(r + 0x1C, 0x3E23_D70A);
    for (o, v) in [(0x22u32, 0x80u8), (0x23, 0), (0x20, 0), (0x21, 0), (0x26, 0xC0), (0x27, 0x80), (0x24, 0x80), (0x25, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, sp + 0x50, r + 0x28);
    let a = if fx.mem.read_u32(D + 0x30) != 0x8000 { -0x4000 } else { 0x4000 };
    fx.mem.set32(r + 0x38, a);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_42D4);
    match fx.mem.u16(D) {
        0x8C => return 8,
        0x58 | 0x54 => cone(fx, sp),
        0x50 => {
            strike(fx, sp);
            cone(fx, sp);
        }
        0x32 => beads_start(fx),
        0x2A..=0x31 => {
            for o in [0x7Cu32, 0x7D, 0x7E, 0x78, 0x79, 0x7A] {
                fx.mem.write_u8(sp + o, 0x80);
            }
            fx.mem.write_u8(sp + 0x7F, 0x70);
            fx.mem.write_u8(sp + 0x7B, 0x70);
            let h = fx.mem.read_u32(D + 4);
            fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h, 4, 1, sp + 0x78]);
        }
        0x20 => drops_start(fx, sp),
        2 => {
            let h = fx.mem.i32(D + 4);
            let slot = fx.mem.i32(D + 0x10);
            node_position_saving(fx, h, slot, POINT, u64::from(OT), Some(param as i64 as u64), 0x01E0_43A8);
            let y = ee::sub(fx.mem.f(POINT + 4), 1.0);
            fx.mem.setf(POINT + 4, y);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x32..0x51).contains(&n) {
        drops_rise(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_5474, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::drop::drops_c(fx, DROPS, OT, 0x64);
    blib::beam::beads_at(fx, 0x01E0_0E40, 0x40, 0x44, BEADS, OT, 8);
    blib::cone::cones_b(fx, 0x01E0_1D50, CONES, OT, 3);
    blib::halo::halo(fx, 0x01E0_2720, HALO, OT, 1);
    blib::spike::spikes_b(fx, 0x01E0_3470, SPIKES, OT, 2);
    let n = fx.mem.u16(D);
    if (0x50..0x6B).contains(&n) {
        6
    } else if n == 0x6B {
        2
    } else if n == 0x46 {
        1
    } else {
        0
    }
}
