//! BMODULE b463 (card 279), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03A80, data area a0 = 0x1ED0000) [F] (helper A13,
//! 31 Oct 2026): the halo / spikes_b / drops / bars_c / fade_c family
//! (compiled without scheduling). On update 66 four crossbars (+0xF0, 0x50
//! each: one wide, three turned 45 degrees apart) at the attacker's node
//! 0x44, a small halo (+0xA4) there and a camera shake (+0x40); on update
//! 94 at the target a fade (+0x5370), six spikes (+0x5F0, 0x68 each) rising
//! by twos, 200 drops (+0x1E50, 0x44 each) falling round it, a wide halo and
//! a long shake; on odd updates 95..113 a spike a update at the target; the
//! stage light pulsed by 0x1031C0 (+0xA0: down on update 50, up on 154);
//! object ops 0x1F83B0 from the record's table 0x1E04DE0. Returns 1 on
//! update 84, 6 on 94..122, 2 on 123, 8 on 160 (and at once for a parameter
//! of -2 / -3).
//!
//! Library (record addresses): `camera_shake` (ac7dccf5ae36) 0x1E00080,
//! `spikes_b` 0x1E00440, `drops` 0x1E01170, `halo` 0x1E01CF0, `bars_c`
//! 0x1E02A40, `fade_c` 0x1E03510 [`crate::blib`].
use crate::b035::{light_ramp, object_ops};
use crate::b076::{copy_v4, rand_bytes};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The object-op table (rodata, 0x14-byte entries up to a 0 start).
const OPS: u32 = 0x01E0_4DE0;

/// The entry 0x1E03A80 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x60);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x5480);
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
    for (at, node, clear) in [(0xCu32, 0x44, 1), (0x10, 0x33, 0), (0x14, 0x36, 0), (0x18, 0x62, 0), (0x1C, 0x64, 0), (0x20, 2, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 0x32);
    fx.mem.set16(D + 0xA2, 0x9A);
    // The entry's s0 = D, s1 = the OT, s2 = 2 (the texture loop's).
    rand_bytes(fx, D + 0x5378, 0x100, 0x01E0_3BFC, [Some(D), Some(OT), Some(2)]);
    0
}

/// Update 66 [F 0x1E03C54]: crossbars, a halo and a shake at node +0xC
/// (b061 the same).
pub(crate) fn bars_start(fx: &mut Fx) {
    for k in 0..4u32 {
        let b = D + 0xF0 + 0x50 * k;
        set_counter4(fx, b, 0x12, 0, 0xFF);
        fx.mem.write_u8(b + 5, 1);
        let node = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(b + 6, node);
        fx.mem.write_u8(b + 4, 0x80);
        if k == 0 {
            fx.mem.write_u32(b + 0x10, 0x447A_0000);
            fx.mem.write_u32(b + 0x14, 0x4140_0000);
        } else {
            fx.mem.write_u32(b + 0x10, 0x43FA_0000);
            let a = ee::mul(45.0, ee::from_i32(k as i32));
            fx.mem.setf(b + 0x18, a);
            fx.mem.write_u32(b + 0x14, 0x40C0_0000);
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0x80), (9, 0x80), (8, 0)] {
            fx.mem.write_u8(b + o, v);
        }
        fx.mem.write_u32(b + 0x1C, 0x4100_0000);
    }
    let g = D + 0xA4;
    set_counter4(fx, g, 0x12, 0, 0xFF);
    fx.mem.write_u8(g + 5, 1);
    let node = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(g + 6, node);
    fx.mem.write_u8(g + 4, 0x80);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x41A0_0000);
    fx.mem.write_u32(g + 0x14, 0x41C8_0000);
    for (o, v) in [(0xCu32, 0xF0u8), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x20), (9, 0x10), (8, 0x80), (0xF, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    fx.mem.write_u32(g + 0x18, 0x4080_0000);
    let s = D + 0x40;
    set_counter4(fx, s, 0x12, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let node = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, node);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
}

/// The spike record's shared colours: RGB 0x80 at +0x24.. / +0x20...
pub(crate) fn spike_colours(fx: &mut Fx, r: u32) {
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
}

/// ((rand >> 4) % 210 - 105) / 100 [F].
pub(crate) fn spread(fx: &mut Fx, s0: u32, ra: u32) -> f32 {
    let x = fx.rand_from(s0, ra) >> 4;
    let (_, h) = fx.div(x, 0xD2);
    ee::div(ee::from_i32(h - 0x69), 100.0)
}

/// What b061's strike sets differently.
pub(crate) struct Strike {
    /// The spikes' +0xA.
    pub spike_a: u8,
    /// The drops' +8.
    pub drop_8: u8,
    /// The shake's count.
    pub shake: i32,
    /// The entry's s0 (rand's frame keeps it): b463 D, b061 the OT.
    pub s0: u32,
}

/// Update 94 [F 0x1E03F2C]: at the target `at` (b061: update 90, the same
/// code addresses).
pub(crate) fn strike(fx: &mut Fx, at: u32, o: &Strike) {
    set_counter4(fx, D + 0x5370, 0xA, 0, 0xFF);
    fx.mem.write_u8(D + 0x5374, 1);
    for k in 0..6i32 {
        let i = first_free(fx, D + 0x5F0, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let r = D + 0x5F0 + 0x68 * i as u32;
        fx.mem.write_u8(r + 4, 4);
        set_counter4(fx, r, 0x1C, (3 * k) & 0xFF, 0xFF);
        let b = fx.mem.read_u8(r + 4) | 2;
        fx.mem.write_u8(r + 4, b);
        fx.mem.write_u8(r + 0x27, 0x40);
        fx.mem.write_u8(r + 0x23, 0x80);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xA, o.spike_a);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 4);
        fx.mem.write_u8(r + 8, 0x32);
        let x = fx.rand_from(o.s0, 0x01E0_40D8);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, D + 0x5378);
        spike_colours(fx, r);
        copy_v4(fx, at, r + 0x28);
        let kf = ee::from_i32(k);
        let v = ee::add(30.0, ee::mul(20.0, kf));
        fx.mem.setf(r + 0x1C, v);
        let v = ee::sub(3.0, ee::mul(0.5, kf));
        fx.mem.setf(r + 0x14, v);
        let c = f32::from_bits(0x3ECC_CCCD);
        let v = ee::add(c, ee::mul(c, kf));
        fx.mem.setf(r + 0x18, v);
        let turn = if fx.mem.i32(D + 0x30) == 0x8000 { 0x2000 } else { -0x2000 };
        fx.mem.set32(r + 0x48, turn);
    }
    for _ in 0..0xC8 {
        let i = first_free(fx, D + 0x1E50, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let r = D + 0x1E50 + 0x44 * i as u32;
        let x = fx.rand_from(o.s0, 0x01E0_4378);
        set_counter4(fx, r, 0x1C, (x >> 4) & 0xF, 0xFF);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u8(r + 8, o.drop_8);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 9, 3);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let x = fx.rand_from(o.s0, 0x01E0_4470) >> 4;
        let (_, h) = fx.div(x, 0x14);
        let v = ee::add(100.0, ee::div(ee::from_i32(h.wrapping_mul(200) - 2000), 100.0));
        fx.mem.setf(r + 0x10, v);
        fx.mem.write_u32(r + 0x14, 0xBF00_0000);
        fx.mem.write_u32(r + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let v = spread(fx, o.s0, 0x01E0_4578);
        fx.mem.setf(r + 0x2C, v);
        let v = spread(fx, o.s0, 0x01E0_45CC);
        fx.mem.setf(r + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), v));
        let v = spread(fx, o.s0, 0x01E0_4634);
        fx.mem.setf(r + 0x34, v);
    }
    let g = D + 0xA4;
    set_counter4(fx, g, 0x24, 0, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u32(g + 0x10, 0x4448_0000);
    fx.mem.write_u32(g + 0x18, 0);
    fx.mem.write_u32(g + 0x14, 0);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x10), (8, 0x80)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    copy_v4(fx, at, g + 0x1C);
    let s = D + 0x40;
    set_counter4(fx, s, o.shake, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x4000_0000);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}

/// Odd updates 95..113 [F 0x1E047E4]: one spike at the target.
fn spike(fx: &mut Fx, sp: u32) {
    let i = first_free(fx, D + 0x5F0, 0x68, 0x3C);
    if i < 0 {
        return;
    }
    let r = D + 0x5F0 + 0x68 * i as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4080_0000);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    fx.mem.write_u8(r + 8, 0x1E);
    fx.mem.write_u8(r + 5, 0);
    copy_v4(fx, sp + 0x50, r + 0x28);
    fx.mem.write_u8(r + 0xA, 0);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 5);
    let x = fx.rand_from(D, 0x01E0_4970);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0x5378);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    spike_colours(fx, r);
    fx.mem.write_u8(r + 0x23, 0x10);
    fx.mem.write_u8(r + 0x27, 0x80);
    for (o, ra) in [(0x58u32, 0x01E0_4AC4u32), (0x5C, 0x01E0_4AF0), (0x60, 0x01E0_4B1C)] {
        let x = fx.rand_from(D, ra);
        fx.mem.set32(r + o, (x >> 4) & 0x7F);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, sp + 0x50);
    match fx.mem.u16(D) {
        0xA0 => return 8,
        0x5E => strike(fx, sp + 0x50, &Strike { spike_a: 1, drop_8: 1, shake: 0x28, s0: D }),
        0x42 => bars_start(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x5E..0x72).contains(&n) && n & 1 != 0 {
        spike(fx, sp);
    }
    blib::shake::camera_shake(fx, D + 0x40);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4B5C, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade_c(fx, D + 0x5370, OT);
    let n = fx.mem.u16(D);
    object_ops(fx, OPS, n);
    blib::spike::spikes_b(fx, 0x01E0_0440, D + 0x5F0, OT, 0x3C);
    blib::halo::halo(fx, 0x01E0_1CF0, D + 0xA4, OT, 1);
    blib::cross::bars_c(fx, 0x01E0_2A40, D + 0xF0, OT, 0x10);
    blib::drop::drops(fx, D + 0x1E50, OT, 0xC8);
    let n = fx.mem.u16(D);
    if (0x5E..0x7B).contains(&n) {
        6
    } else if n == 0x7B {
        2
    } else if n == 0x54 {
        1
    } else {
        0
    }
}
