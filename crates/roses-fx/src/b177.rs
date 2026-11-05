//! BMODULE b177 (card 319), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04CB0, data area a0 = 0x1ED0000) [F] (helper A19,
//! 31 Oct 2026): b468's family (s3 = D, s0 = the OT) with the comets
//! 29cd5954f1de (`comet::comets`) and the aimed rings b25d21b671cb
//! (`iris::rings_b`). On update 61 at the attacker (node 0x2A) a halo of 80
//! (+0xA4), a comet (+0x770: 0x46 counts, a history of 8 in the pool
//! +0x7B4, head 200 / tail 180, cells 3 / 3) and a shake (+0x40); on 80 a
//! set of eight rings (+0xA34, 0x12 counts) at the attacker aimed at its
//! node slot +0x10 (node 0x17, y - 0.6, x / z +-4 / +-5 by the facing); on
//! 68..102 every fourth update a spike at the attacker (+0xF0, 0x68 each,
//! 16 slots: 16 counts, flags 5, 3.5 / 16); on 99 six spikes at the target
//! (delays 3 k, 20 (k + 1) long, 2.5 - 0.4 k / 0.4 + 0.4 k, turned
//! -+0x4000 by the facing), a halo of 800 and a cone (+0xA8C, cones_b,
//! 2.5 / 0.1 / 0.4, turned 0x4000); the stage light by 0x1031C0 (+0xA0:
//! down on 2, up on 0x9E). Returns 1 on update 90, 6 on 100..128, 2 on
//! 129, 8 on 164 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `spikes_b`
//! 0x1E00440, `halo` 0x1E01170, `comets` 0x1E01EC0, `rings_b` 0x1E03020,
//! `cones_b` 0x1E042E0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const HALO: u32 = D + 0xA4;
const SPIKES: u32 = D + 0xF0;
const COMET: u32 = D + 0x770;
const RINGS: u32 = D + 0xA34;
const CONE: u32 = D + 0xA8C;
const BYTES: u32 = D + 0xAD4;

/// The entry 0x1E04CB0 (a0 = D, a1 = param; s3 = D, s1 = param, s0 = the
/// OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x60);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xBE0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x2A, 1);
    fx.mem.set32(D + 0xC, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x17, 0);
    fx.mem.set32(D + 0x10, r);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x9E);
    // The entry's s0 = the OT, s1 = 1 (the texture loop's), s2 = the
    // runner's 0 [H, as b201].
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4DC8, [Some(OT), Some(1), Some(0)]);
    0
}

/// Update 0x3D [F 0x1E04E2C]: a halo, the comet and the shake at the
/// attacker.
fn start(fx: &mut Fx) {
    set_counter4(fx, HALO, 0xC, 0, 0xFF);
    fx.mem.write_u8(HALO + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(HALO + 6, b);
    for (o, v) in [(4u32, 0x80u8), (7, 9), (0xA, 0x80), (9, 0x10), (8, 0x20), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0)] {
        fx.mem.write_u8(HALO + o, v);
    }
    fx.mem.write_u32(HALO + 0x10, 0x42A0_0000);
    fx.mem.write_u32(HALO + 0x14, 0x4270_0000);
    fx.mem.write_u32(HALO + 0x18, 0x4100_0000);
    let c = COMET;
    set_counter4(fx, c, 0x46, 0, 7);
    fx.mem.write_u8(c + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(c + 6, b);
    fx.mem.write_u8(c + 4, 0x86);
    fx.mem.write_u32(c + 8, D + 0x7B4);
    fx.mem.write_u32(c + 0x28, 0x28);
    for (o, v) in [(0x1Eu32, 8u8), (0x1F, 0), (0x21, 2), (0x22, 3), (0x23, 3), (0x34, 0x80), (0x35, 0x80), (0x36, 0x80), (0x37, 0xC0)] {
        fx.mem.write_u8(c + o, v);
    }
    fx.mem.write_u32(c + 0x2C, 0x4348_0000);
    fx.mem.write_u32(c + 0x30, 0x4334_0000);
    let mut k = 0;
    while k < fx.mem.u8(c + 0x1E) + 1 {
        fx.mem.set16(c + 0xC + 2 * k as u32, -1);
        k += 1;
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x46, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, b);
    fx.mem.write_u8(s + 4, 4);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3F99_999A);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
}

/// Update 0x50 [F 0x1E04FD4]: the rings at the attacker, aimed at its
/// node slot +0x10.
fn rings(fx: &mut Fx, param: i32) {
    let r = RINGS;
    set_counter4(fx, r, 0x12, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0x80);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    fx.mem.write_u8(r + 0x10, 8);
    fx.mem.write_u8(r + 0x11, 0x80);
    fx.mem.write_u8(r + 0x12, 0x60);
    fx.mem.write_u32(r + 0x34, 0x3FB3_3333);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, slot, r + 0x48, u64::from(OT), Some(param as i64 as u64), 0x01E0_503C);
    let back = fx.mem.i32(D + 0x30) == 0x8000;
    if !back {
        let y = ee::sub(fx.mem.f(r + 0x4C), f32::from_bits(0x3F19_999A));
        fx.mem.setf(r + 0x4C, y);
    }
    let (x, z) = (fx.mem.f(r + 0x48), fx.mem.f(r + 0x50));
    if back {
        fx.mem.setf(r + 0x48, ee::add(x, 4.0));
        fx.mem.setf(r + 0x50, ee::add(z, 5.0));
    } else {
        fx.mem.setf(r + 0x48, ee::sub(x, 4.0));
        fx.mem.setf(r + 0x50, ee::sub(z, 5.0));
    }
    let rings = [(0u8, 0x41F0_0000u32), (4, 0x4220_0000), (2, 0x4120_0000), (2, 0x41F0_0000), (4, 0x4270_0000), (0, 0x41A0_0000), (5, 0x42A0_0000), (2, 0x41A0_0000)];
    for (k, (kind, size)) in rings.into_iter().enumerate() {
        fx.mem.write_u8(r + 8 + k as u32, kind);
        fx.mem.write_u32(r + 0x14 + 4 * k as u32, size);
    }
}

/// Update 0x63 [F 0x1E05160]: six spikes, a halo and the cone at the
/// target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    let back = |fx: &mut Fx| fx.mem.i32(D + 0x30) == 0x8000;
    for s1 in 0..6i32 {
        let i = first_free(fx, SPIKES, 0x68, 0x10);
        if i < 0 {
            continue;
        }
        let r = SPIKES + 0x68 * i as u32;
        fx.mem.write_u8(r + 4, 4);
        set_counter4(fx, r, 0x14, (s1 * 3) & 0xFF, 0xFF);
        let f = fx.mem.read_u8(r + 4);
        fx.mem.write_u8(r + 4, f | 2);
        for (o, v) in [(0x23u32, 0x80u8), (0x27, 0x80), (5, 0), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0x50)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_52E4);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0x50, r + 0x28);
        fx.mem.setf(r + 0x1C, ee::mul(20.0, ee::from_i32(s1 + 1)));
        let t = ee::mul(f32::from_bits(0x3ECC_CCCD), ee::from_i32(s1));
        fx.mem.setf(r + 0x14, ee::sub(2.5, t));
        let t = ee::mul(f32::from_bits(0x3ECC_CCCD), ee::from_i32(s1));
        fx.mem.setf(r + 0x18, ee::add(f32::from_bits(0x3ECC_CCCD), t));
        let turn = if back(fx) { 0x4000 } else { -0x4000 };
        fx.mem.set32(r + 0x48, turn);
    }
    set_counter4(fx, HALO, 0x20, 2, 0xFF);
    fx.mem.write_u8(HALO + 5, 0);
    fx.mem.write_u8(HALO + 4, 0x81);
    fx.mem.write_u8(HALO + 7, 9);
    fx.mem.write_u32(HALO + 0x10, 0x4448_0000);
    fx.mem.write_u32(HALO + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x20), (8, 0xA0), (0xF, 0xD0)] {
        fx.mem.write_u8(HALO + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(HALO + o, 0);
    }
    copy_v4(fx, sp + 0x50, HALO + 0x1C);
    let c = CONE;
    set_counter4(fx, c, 0x18, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, 0)] {
        fx.mem.write_u8(c + o, v);
    }
    fx.mem.write_u32(c + 0xC, 0x4020_0000);
    fx.mem.write_u32(c + 0x10, 0x3DCC_CCCD);
    fx.mem.write_u32(c + 0x14, 0x3ECC_CCCD);
    fx.mem.write_u32(c + 0x18, 0);
    fx.mem.write_u32(c + 0x1C, 0x3D4C_CCCD);
    for (o, v) in [(0x23u32, 0x80u8), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x26, 0x80), (0x25, 0), (0x24, 0x20), (0x27, 0)] {
        fx.mem.write_u8(c + o, v);
    }
    copy_v4(fx, sp + 0x50, c + 0x28);
    // s1 = 6 after the spikes: never 0.
    fx.mem.set32(c + 0x38, 0x4000);
}

/// Updates 0x44..0x66 (n & 3 = 0) [F 0x1E056F4]: a spike at the attacker.
fn spike(fx: &mut Fx) {
    let i = first_free(fx, SPIKES, 0x68, 0x10);
    if i < 0 {
        return;
    }
    let r = SPIKES + 0x68 * i as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4060_0000);
    fx.mem.write_u32(r + 0x1C, 0x4180_0000);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 6), (8, 0x5A)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_5868);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    fx.mem.write_u8(r + 0x23, 0x10);
    fx.mem.write_u8(r + 0x27, 0x80);
    fx.mem.write_u32(r + 0x58, 0x40);
    fx.mem.write_u32(r + 0x5C, 0x80);
    fx.mem.write_u32(r + 0x60, 0x100);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, then the spike loop's count.
    let mut s1 = param as i64 as u64;
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_4DF0);
    match fx.mem.u16(D) {
        0xA4 => return 8,
        0x63 => {
            strike(fx, sp);
            s1 = 6;
        }
        0x50 => rings(fx, param),
        0x3D => start(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x44..0x67).contains(&n) && n & 3 == 0 {
        spike(fx);
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_5A20, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5A30, [Some(u64::from(OT)), Some(s1)]);
    blib::comet::comets(fx, 0x01E0_1EC0, COMET, OT, 1);
    blib::spike::spikes_b(fx, 0x01E0_0440, SPIKES, OT, 0x10);
    blib::halo::halo(fx, 0x01E0_1170, HALO, OT, 1);
    blib::iris::rings_b(fx, 0x01E0_3020, RINGS, OT, 1);
    blib::cone::cones_b(fx, 0x01E0_42E0, CONE, OT, 1);
    let n = fx.mem.u16(D);
    if (0x64..0x81).contains(&n) {
        6
    } else if n == 0x81 {
        2
    } else if n == 0x5A {
        1
    } else {
        0
    }
}
