//! BMODULE b097 (card 107), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06430, data area a0 = 0x1ED0000) [F] (helper A18,
//! 31 Oct 2026): b201's family (s3 = D, s0 = the OT) with four recompiled
//! library functions: `rays_c` (fbc4ffd89c21), `arc_rays_c`
//! (db6d31a2770e), `bead_trails_e` (8fbdb570917b) and `drops_c`
//! (efb41f6a08dd). On 8..180 two drops an update (+0xA0, 0x44 each, 64
//! slots: 0x20 counts, cell (rand + 2) & 3, 150 wide growing 8, alpha
//! 0x50) at the attacker's node +0xC, 4 lower, scattered +-0.5 and thrown
//! +-0.3; on 70 an arc (+0x11A0, nodes +0x10 -> +0x14, 0x28 wide, 8
//! points); on 125 / 133 / 141 a bead-trail emitter (+0x35E8: 7 counts,
//! beads into +0x1A68, 0x14 / 0x1C counts) at the attacker's node slot 0;
//! on 127 / 135 / 143 at the target a halo of 800 (+0x1480, 8 / 0x1C
//! counts), a cone (+0x3674, 0x48 each, 3 slots: 0x14 counts, 3 / 0.4 /
//! 2.5, 0.18, turned +-0x2000 by the facing, +0x40 = the turn on 127,
//! its negation on 135) and a shake (+0x40, 80); on 128 / 136 / 144 the
//! arc's ray bursts (+0x14CC into the ray pool +0x14F8, 11 samples, 8 /
//! 16 counts). Returns 1 on update 118, 6 on 128..140, 2 on 141, 8 on 195
//! (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` (ac7dccf5ae36) 0x1E00080,
//! `arcs` 0x1E00440, `halo` 0x1E01880, `rays_c` 0x1E025D0, `arc_rays_c`
//! 0x1E03290, `beads_at 40 44` 0x1E03A70, `bead_trails_e` 0x1E04980,
//! `cones_b` 0x1E05060, `drops_c` 0x1E05A30 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const DROPS: u32 = D + 0xA0;
const ARC: u32 = D + 0x11A0;
const HALO: u32 = D + 0x1480;
const BURST: u32 = D + 0x14CC;
const RAYS: u32 = D + 0x14F8;
const BEADS: u32 = D + 0x1A68;
const EMITTER: u32 = D + 0x35E8;
const CONES: u32 = D + 0x3674;
const BYTES: u32 = D + 0x374C;

/// The entry 0x1E06430 (a0 = D, a1 = param; s3 = D, s1 = param, s0 = the
/// OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3850);
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
    for (at, node, clear) in [(0xCu32, 0x40, 1), (0x10, 0x1C, 0), (0x14, 0x1B, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The entry's s0 = 2 (the texture loop's), s1 = the parameter, s2 =
    // the runner's 0 [H, as b201].
    rand_bytes(fx, BYTES, 0x100, 0x01E0_6554, [Some(2), Some(param as u32), Some(0)]);
    0
}

/// ((rand >> 4) % m + add) / 100 [F].
fn spread(fx: &mut Fx, ra: u32, m: i32, add: i32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h + add), 100.0)
}

/// Update 0x46 [F 0x1E0660C]: the arc (+0x11A0), points +0x1200.
fn arc_start(fx: &mut Fx) {
    let g = ARC;
    set_counter4(fx, g, 0xA0, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x1200);
    fx.mem.write_u8(g + 0x30, 0x28);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F99_999A);
    fx.mem.write_u8(g + 0x34, 0);
    fx.mem.set16(g + 0x36, 0);
    for (o, v) in [(0x47u32, 0x80u8), (0x44, 0x80), (0x45, 0x80), (0x46, 0x80), (0x4A, 0xC0), (0x49, 0x10), (0x48, 0x10)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        fx.mem.set16(g + 0xC + 2 * a, -1);
        a += 1;
    }
}

/// Updates 0x7D / 0x85 / 0x8D [F 0x1E066DC]: the bead-trail emitter at the
/// attacker's node slot +0x1C (never requested: 0).
fn emitter_start(fx: &mut Fx, n: i32, param: i32) {
    let g = EMITTER;
    set_counter4(fx, g, 7, 0, 0xFF);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(g + 6, b);
    for (o, v) in [(4u32, 2u8), (5, 1), (0x14, 3), (0x16, 2)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 8, BEADS);
    fx.mem.write_u8(g + 0x10, 0x50);
    fx.mem.write_u8(g + 0x12, 0x14);
    fx.mem.write_u8(g + 0x11, if n == 0x8D { 0x1C } else { 0x14 });
    fx.mem.write_u8(g + 0x13, 0);
    fx.mem.write_u32(g + 0x18, 0x3F19_999A);
    fx.mem.write_u32(g + 0x1C, 0x3F19_999A);
    fx.mem.write_u32(g + 0x20, 0x4270_0000);
    fx.mem.write_u32(g + 0xC, BYTES);
    for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
        fx.mem.write_u8(g + o, 0x80);
    }
    let h = fx.mem.i32(D + 4);
    let slot = fx.mem.i32(D + 0x1C);
    node_position_saving(fx, h, slot, g + 0x3C, u64::from(OT), Some(param as i64 as u64), 0x01E0_67B0);
    copy_v4(fx, g + 0x3C, g + 0x4C);
}

/// Updates 0x7F / 0x87 / 0x8F [F 0x1E067D8]: the halo, a cone and the
/// shake at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32, n: i32) {
    let r = HALO;
    for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9), (0xF, 0xD0), (0xC, 0xD0), (0xD, 0xD0), (0xE, 0xD0), (0xA, 0x60), (9, 0x10), (8, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x14, 0);
    fx.mem.write_u32(r + 0x10, 0x4448_0000);
    copy_v4(fx, sp + 0x50, r + 0x1C);
    set_counter4(fx, r, if n == 0x8F { 0x1C } else { 8 }, 0, 0xFF);
    let i = first_free(fx, CONES, 0x48, 3);
    if i < 0 {
        return;
    }
    let r = CONES + 0x48 * i as u32;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, 1), (0xA, 0x20), (9, 7)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0xC, 0x4040_0000);
    fx.mem.write_u32(r + 0x10, 0x3ECC_CCCD);
    fx.mem.write_u32(r + 0x14, 0x4020_0000);
    fx.mem.write_u32(r + 0x18, 0);
    fx.mem.write_u32(r + 0x1C, 0x3E38_51EC);
    for (o, v) in [(0x22u32, 0xC0u8), (0x23, 0), (0x20, 0), (0x21, 0), (0x26, 0xC0), (0x27, 0x80), (0x24, 0x80), (0x25, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, sp + 0x50, r + 0x28);
    let a = if fx.mem.read_u32(D + 0x30) != 0x8000 { -0x2000 } else { 0x2000 };
    fx.mem.set32(r + 0x38, a);
    match n {
        0x87 => {
            let v = fx.mem.i32(r + 0x38).wrapping_neg();
            fx.mem.set32(r + 0x40, v);
        }
        0x7F => {
            let v = fx.mem.i32(r + 0x38);
            fx.mem.set32(r + 0x40, v);
        }
        _ => {}
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x10, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x50, s + 0x20);
}

/// Updates 0x80 / 0x88 / 0x90 [F 0x1E06C04]: the arc's ray bursts.
fn burst_start(fx: &mut Fx, n: i32) {
    let g = BURST;
    set_counter4(fx, g, 1, 0, 0xFF);
    fx.mem.write_u8(g + 4, 2);
    fx.mem.write_u32(g + 8, ARC);
    fx.mem.write_u32(g + 0xC, RAYS);
    fx.mem.write_u8(g + 0x14, 0x14);
    fx.mem.write_u8(g + 0x16, 0x1E);
    if n == 0x90 {
        fx.mem.write_u8(g + 0x15, 0x10);
        fx.mem.write_u32(g + 0x18, 0x4080_0000);
        fx.mem.write_u32(g + 0x1C, 0x3F4C_CCCD);
    } else {
        fx.mem.write_u8(g + 0x15, 8);
        fx.mem.write_u32(g + 0x18, 0x4100_0000);
        fx.mem.write_u32(g + 0x1C, 0x3FC0_0000);
    }
    fx.mem.write_u8(g + 0x17, 0xB);
    fx.mem.write_u32(g + 0x20, 0x41F0_0000);
    fx.mem.write_u32(g + 0x10, BYTES);
    for (o, v) in [(0x26u32, 0xC0u8), (0x24, 0), (0x25, 0), (0x2B, 0x60), (0x28, 0x60), (0x29, 0x60), (0x2A, 0x60)] {
        fx.mem.write_u8(g + o, v);
    }
}

/// Updates 8..0xB4 [F 0x1E06D0C]: two drops at the node +0xC.
fn drops(fx: &mut Fx, sp: u32) -> i32 {
    let mut s2 = 0;
    for s1 in 0..2 {
        let i = first_free(fx, DROPS, 0x44, 0x40);
        s2 = i;
        if i < 0 {
            continue;
        }
        let r = DROPS + 0x44 * i as u32;
        set_counter4(fx, r, 0x20, 0, 0xFF);
        let x = fx.rand_from(OT, 0x01E0_6D5C);
        fx.mem.set8(r + 4, (((x >> 4) & 3) << 4) | 4);
        fx.mem.write_u8(r + 9, 1);
        fx.mem.write_u8(r + 8, 1);
        let x = fx.rand_from(OT, 0x01E0_6DB8);
        fx.mem.set8(r + 0xA, ((x >> 4) + 2) & 3);
        for (o, v) in [(0xBu32, 0u8), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x50)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x4316_0000);
        fx.mem.write_u32(r + 0x14, 0x4100_0000);
        fx.mem.write_u32(r + 0x18, 0xBCA3_D70A);
        fx.mem.write_u32(r + 0x1C, 0);
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0xC);
        node_position_saving(fx, h, slot, sp + 0x60, u64::from(OT), Some(s1 as u64), 0x01E0_6EB8);
        let v = spread(fx, 0x01E0_6EC0, 0x64, -0x32);
        let w = ee::add(fx.mem.f(sp + 0x60), v);
        fx.mem.setf(r + 0x20, w);
        let w = ee::sub(fx.mem.f(sp + 0x64), 4.0);
        fx.mem.setf(r + 0x24, w);
        let v = spread(fx, 0x01E0_6F44, 0x64, -0x32);
        let w = ee::add(fx.mem.f(sp + 0x68), v);
        fx.mem.setf(r + 0x28, w);
        let v = spread(fx, 0x01E0_6FA0, 0x3C, -0x1E);
        fx.mem.setf(r + 0x2C, v);
        fx.mem.write_u32(r + 0x30, 0x3ECC_CCCD);
        let v = spread(fx, 0x01E0_7010, 0x3C, -0x1E);
        fx.mem.setf(r + 0x34, v);
    }
    s2
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_657C);
    let n = fx.mem.u16(D);
    match n {
        0xC3 => return 8,
        0x90 | 0x88 | 0x80 => burst_start(fx, n),
        0x8F | 0x87 | 0x7F => strike(fx, sp, n),
        0x8D | 0x85 | 0x7D => emitter_start(fx, n, param),
        0x46 => arc_start(fx),
        _ => {}
    }
    // The entry's s1 / s2 at the library calls: the parameter / the
    // runner's 0 [H], or the drop loop's 2 / its last free slot.
    let (mut s1, mut s2) = (param as i64 as u64, 0u64);
    let n = fx.mem.u16(D);
    if (8..0xB5).contains(&n) {
        s2 = drops(fx, sp) as i64 as u64;
        s1 = 2;
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_7084, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::arc::arcs(fx, 0x01E0_0440, ARC, OT, 1);
    blib::halo::halo(fx, 0x01E0_1880, HALO, OT, 1);
    blib::ray::arc_rays_c(fx, 0x01E0_3290, BURST, 1);
    blib::ray::rays_c(fx, 0x01E0_25D0, RAYS, OT, 0xC);
    // rays_c's frame (0x250: s0 .. s7 / fp +0x10 .. +0x90, ra +0xA0; s4 ..
    // fp the runner's 0 [H]): its saved s7 / fp / ra are the stale w of
    // the emitter's path samples 0..2 (copied into the beads' +0x34) [run].
    let f = sp - 0x250;
    fx.saved(f, &[(0x10, u64::from(OT)), (0x20, s1), (0x30, s2), (0x40, u64::from(D)), (0x50, 0), (0x60, 0), (0x70, 0), (0x80, 0), (0x90, 0), (0xA0, 0x01E0_70D4)]);
    blib::emit::bead_trails_e(fx, 0x01E0_4980, EMITTER, 1);
    blib::beam::beads_at(fx, 0x01E0_3A70, 0x40, 0x44, BEADS, OT, 0x50);
    blib::cone::cones_b(fx, 0x01E0_5060, CONES, OT, 3);
    blib::drop::drops_c(fx, DROPS, OT, 0x40);
    let n = fx.mem.u16(D);
    if (0x80..0x8D).contains(&n) {
        6
    } else if n == 0x8D {
        2
    } else if n == 0x76 {
        1
    } else {
        0
    }
}
