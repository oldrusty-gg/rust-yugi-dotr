//! BMODULE b061 (card 272), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03A80, data area a0 = 0x1ED0000) [F] (helper A13,
//! 31 Oct 2026): b463's program (the same library at the same addresses,
//! the same code addresses up to 0x1E047B4) with a 0x90 frame: on updates
//! 2..89 four drops a update rising from the attacker's nodes 0x33 / 0x36
//! / 0x62 / 0x64, b463's crossbars / halo / shake on 66 at node 0x18, its
//! strike on 90 (spikes +0xA 2, drops +8 2, a shorter shake), a spike on
//! odd updates 91..109 (longer, wider); no object ops. Returns 1 on update
//! 80, 6 on 90..118, 2 on 119, 8 on 160 (and at once for a parameter of
//! -2 / -3).
//!
//! Library (record addresses): `camera_shake` (ac7dccf5ae36) 0x1E00080,
//! `spikes_b` 0x1E00440, `drops` 0x1E01170, `halo` 0x1E01CF0, `bars_c`
//! 0x1E02A40, `fade_c` 0x1E03510 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes};
use crate::b463::{bars_start, spike_colours, strike, Strike};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E03A80 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x90);
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
    for k in 0..4 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x18, 1), (0x10, 0x33, 0), (0x14, 0x36, 0), (0x18, 0x62, 0), (0x1C, 0x64, 0), (0x20, 2, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x9A);
    // The entry's s0 = the OT, s1 = 4 (the texture loop's).
    rand_bytes(fx, D + 0x5378, 0x100, 0x01E0_3BFC, [Some(OT), Some(4), None]);
    0
}

/// (-((rand >> 4) % 3 x 200 - 300) / 100) [F 0x1E049D0].
fn lift(fx: &mut Fx, ra: u32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, 3);
    ee::neg(ee::div(ee::from_i32(h.wrapping_mul(200) - 300), 100.0))
}

/// Updates 2..89 [F 0x1E047D4]: a drop from each of the nodes +0x10..
fn node_drops(fx: &mut Fx, sp: u32) {
    for k in 0..4u32 {
        let i = first_free(fx, D + 0x1E50, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let r = D + 0x1E50 + 0x44 * i as u32;
        fx.fix_residue(20.0, 0x01E0_4814);
        let life = ee::fixsfsi(20.0);
        set_counter4(fx, r, life & 0xFF, 0, 0xFF);
        let x = fx.rand_from(OT, 0x01E0_4844);
        fx.mem.set8(r + 4, (((x >> 4) & 3) << 4) | 0x41);
        fx.mem.write_u8(r + 8, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 9, 3);
        fx.mem.write_u8(r + 0xA, 0);
        for o in [0xCu32, 0xD, 0xE, 0xF] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u32(r + 0x10, 0x4320_0000);
        fx.mem.write_u32(r + 0x14, 0);
        fx.mem.write_u32(r + 0x1C, 0);
        fx.mem.write_u32(r + 0x18, 0);
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0x10 + 4 * k);
        fx.node_position(h, slot, sp + 0x70);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        for (c, ra) in [(0u32, 0x01E0_49D8u32), (4, 0x01E0_4A38), (8, 0x01E0_4A98)] {
            let v = lift(fx, ra);
            fx.mem.setf(sp + 0x80 + c, v);
        }
        for c in 0..3 {
            let v = ee::div(fx.mem.f(sp + 0x80 + 4 * c), 20.0);
            fx.mem.setf(r + 0x2C + 4 * c, v);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(r + o, 0);
        }
    }
}

/// Odd updates 91..109 [F 0x1E04BC8]: one spike at the target.
fn spike(fx: &mut Fx, sp: u32) {
    let i = first_free(fx, D + 0x5F0, 0x68, 0x3C);
    if i < 0 {
        return;
    }
    let r = D + 0x5F0 + 0x68 * i as u32;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x40E0_0000);
    fx.mem.write_u32(r + 0x1C, 0x4270_0000);
    fx.mem.write_u8(r + 8, 0x1E);
    fx.mem.write_u8(r + 5, 0);
    copy_v4(fx, sp + 0x60, r + 0x28);
    fx.mem.write_u8(r + 0xA, 1);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 3);
    let x = fx.rand_from(OT, 0x01E0_4D58);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0x5378);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    spike_colours(fx, r);
    fx.mem.write_u8(r + 0x23, 0x20);
    fx.mem.write_u8(r + 0x27, 0x60);
    for (o, ra) in [(0x58u32, 0x01E0_4EACu32), (0x5C, 0x01E0_4ED8), (0x60, 0x01E0_4F04)] {
        let x = fx.rand_from(OT, ra);
        fx.mem.set32(r + o, (x >> 4) & 0x7F);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, then the last loop's count.
    let mut s1 = param as i64 as u64;
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, sp + 0x60);
    match fx.mem.u16(D) {
        0xA0 => return 8,
        0x5A => {
            strike(fx, sp + 0x60, &Strike { spike_a: 2, drop_8: 2, shake: 0x20, s0: OT });
            s1 = 0xC8;
        }
        0x42 => {
            bars_start(fx);
            s1 = 4;
        }
        _ => {}
    }
    if (2..0x5A).contains(&fx.mem.u16(D)) {
        node_drops(fx, sp);
        s1 = 4;
    }
    let n = fx.mem.u16(D);
    if (0x5A..0x6E).contains(&n) && n & 1 != 0 {
        spike(fx, sp);
    }
    blib::shake::camera_shake(fx, D + 0x40);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4F44, [Some(u64::from(OT)), Some(s1)]);
    blib::fade::fade_c(fx, D + 0x5370, OT);
    blib::spike::spikes_b(fx, 0x01E0_0440, D + 0x5F0, OT, 0x3C);
    blib::halo::halo(fx, 0x01E0_1CF0, D + 0xA4, OT, 1);
    blib::cross::bars_c(fx, 0x01E0_2A40, D + 0xF0, OT, 0x10);
    blib::drop::drops(fx, D + 0x1E50, OT, 0xC8);
    let n = fx.mem.u16(D);
    if (0x5A..0x77).contains(&n) {
        6
    } else if n == 0x77 {
        2
    } else if n == 0x50 {
        1
    } else {
        0
    }
}
