//! BMODULE b201 (card 242), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03980, data area a0 = 0x1ED0000) [F] (helper A15,
//! 31 Oct 2026): b463's family (halo / spikes_b / drops / fade_c, compiled
//! without scheduling) with cones_b (818ce78a7471) instead of the
//! crossbars. On update 33 at the attacker's node 2 two halos (+0x25C: a
//! wide one of 200 for 42 after a delay of 8, a small one of 20 for 16), a
//! spike (+0x340, 0x68 each) and a shake (+0x40); on even updates 40..58 a
//! spike a update at node 2; on update 75 at the target a fade (+0xA4),
//! six cones (+0xAC, 0x48 each, delays k, turned by the attacker's facing
//! +0x30), a halo of 900, a spike (its slot taken without the free test),
//! 200 drops (+0x1040, 0x44 each) and a shake; the stage light by 0x1031C0
//! (+0xA0: down on update 2, up on 134). Returns 1 on update 65, 6 on
//! 75..95, 2 on 96, 8 on 140 (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` (ac7dccf5ae36) 0x1E00080,
//! `cones_b` 0x1E00440, `halo` 0x1E00E10, `spikes_b` 0x1E01B60, `drops`
//! 0x1E02890, `fade_c` 0x1E03410 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes};
use crate::b463::spike_colours;
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E03980 (a0 = D, a1 = param).
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
    fx.bzero(D, 0x4660);
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
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 2, 1);
    fx.mem.set32(D + 0xC, r);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x86);
    // The entry's s0 = the OT, s1 = 2 (the texture loop's), s2 = 0 (the
    // runner's).
    rand_bytes(fx, D + 0x4560, 0x100, 0x01E0_3A84, [Some(OT), Some(2), Some(0)]);
    0
}

/// The shake record +0x40 (count, +5, +4).
fn shake(fx: &mut Fx, count: i32, b5: u8, b4: u8) {
    let s = D + 0x40;
    set_counter4(fx, s, count, 0, 0xFF);
    fx.mem.write_u8(s + 5, b5);
    if b5 != 0 {
        let node = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(s + 6, node);
    }
    fx.mem.write_u8(s + 4, b4);
    fx.mem.write_u32(s + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
}

/// Update 33 [F 0x1E03ADC]: two halos, a spike and a shake at node +0xC.
fn start(fx: &mut Fx) {
    let g = D + 0x25C;
    set_counter4(fx, g, 0x2A, 8, 0xFF);
    fx.mem.write_u8(g + 5, 1);
    let node = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(g + 6, node);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x4348_0000);
    fx.mem.write_u32(g + 0x14, 0x41A0_0000);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x60), (8, 0x20), (0xF, 0xF0)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    let g = D + 0x2A8;
    set_counter4(fx, g, 0x10, 0, 0xFF);
    fx.mem.write_u8(g + 5, 1);
    let node = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(g + 6, node);
    fx.mem.write_u8(g + 4, 0x80);
    fx.mem.write_u8(g + 7, 9);
    for (o, v) in [(0xAu32, 0x80u8), (9, 0x40), (8, 0), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 0x10, 0x41A0_0000);
    fx.mem.write_u32(g + 0x14, 0x41A0_0000);
    fx.mem.write_u32(g + 0x18, 0x4120_0000);
    // One spike (the loop body with s1 = 0).
    let r = D + 0x340;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let node = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, node);
    fx.mem.write_u8(r + 4, 0x85);
    fx.mem.write_u8(r + 0xA, 0);
    fx.mem.write_u8(r + 0xC, 5);
    fx.mem.write_u32(r + 0x14, 0x4000_0000);
    fx.mem.write_u32(r + 0x1C, 0x4170_0000);
    fx.mem.write_u8(r + 0x23, 0);
    fx.mem.write_u8(r + 0x27, 0x60);
    fx.mem.write_u8(r + 8, 0x28);
    fx.mem.write_u32(r + 0x58, 0);
    fx.mem.write_u32(r + 0x5C, 0);
    fx.mem.write_u32(r + 0x60, 0x300);
    fx.mem.write_u8(r + 0xB, 2);
    let x = fx.rand_from(OT, 0x01E0_3DC4);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0x4560);
    spike_colours(fx, r);
    shake(fx, 0x40, 1, 4);
}

/// ((rand >> 4) % 150 - 75) / 100 [F].
fn spread(fx: &mut Fx, ra: u32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, 0x96);
    ee::div(ee::from_i32(h - 0x4B), 100.0)
}

/// Update 75 [F 0x1E03F1C]: at the target `at`.
fn strike(fx: &mut Fx, at: u32) {
    set_counter4(fx, D + 0xA4, 0xC, 0, 0xFF);
    fx.mem.write_u8(D + 0xA8, 1);
    for k in 0..6i32 {
        let c = D + 0xAC + 0x48 * k as u32;
        set_counter4(fx, c, 0x18, k & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, 2), (0xA, 0x20), (9, 7)] {
            fx.mem.write_u8(c + o, v);
        }
        fx.mem.write_u32(c + 0xC, 0x4000_0000);
        fx.mem.write_u32(c + 0x10, 0x3E99_999A);
        fx.mem.write_u32(c + 0x14, 0x3FC0_0000);
        fx.mem.write_u32(c + 0x18, 0);
        fx.mem.write_u32(c + 0x1C, 0x3DCC_CCCD);
        for (o, v) in [(0x22u32, 0xA0u8), (0x21, 0xA0), (0x20, 0xA0), (0x23, 0), (0x26, 0xC0), (0x25, 0x20), (0x24, 0x10), (0x27, 0x80)] {
            fx.mem.write_u8(c + o, v);
        }
        copy_v4(fx, at, c + 0x28);
        let turn = if fx.mem.i32(D + 0x30) == 0x8000 { 0x4000 } else { -0x4000 };
        fx.mem.set32(c + 0x38, turn);
    }
    let g = D + 0x2F4;
    set_counter4(fx, g, 0x1C, 8, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x4461_0000);
    fx.mem.write_u32(g + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x20), (8, 0x80), (0xF, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    copy_v4(fx, at, g + 0x1C);
    // The spike slot is taken without the free test (-1 lands below the
    // pool).
    let i = first_free(fx, D + 0x340, 0x68, 0x20);
    let r = D.wrapping_add(0x340).wrapping_add((0x68 * i) as u32);
    set_counter4(fx, r, 0x18, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x3FA6_6666);
    fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 0xA, 0);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 3);
    fx.mem.write_u8(r + 8, 0x3C);
    let x = fx.rand_from(OT, 0x01E0_43A8);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0x4560);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    spike_colours(fx, r);
    fx.mem.write_u8(r + 0x23, 0x20);
    fx.mem.write_u8(r + 0x27, 0x80);
    copy_v4(fx, at, r + 0x28);
    fx.mem.write_u32(r + 0x58, 0x40);
    fx.mem.write_u32(r + 0x5C, 0x80);
    fx.mem.write_u32(r + 0x60, 0x100);
    for _ in 0..0xC8 {
        let i = first_free(fx, D + 0x1040, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let r = D + 0x1040 + 0x44 * i as u32;
        let x = fx.rand_from(OT, 0x01E0_45C4);
        set_counter4(fx, r, 0x24, (x >> 4) & 7, 0xFF);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u8(r + 8, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 9, 5);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let x = fx.rand_from(OT, 0x01E0_46B8) >> 4;
        let (_, h) = fx.div(x, 0x14);
        let v = ee::add(50.0, ee::div(ee::from_i32(h.wrapping_mul(200) - 2000), 100.0));
        fx.mem.setf(r + 0x10, v);
        fx.mem.write_u32(r + 0x14, 0xBF19_999A);
        fx.mem.write_u32(r + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let v = spread(fx, 0x01E0_47C4);
        fx.mem.setf(r + 0x2C, v);
        let v = spread(fx, 0x01E0_4818);
        fx.mem.setf(r + 0x30, ee::add(1.0, v));
        let v = spread(fx, 0x01E0_487C);
        fx.mem.setf(r + 0x34, v);
    }
    shake(fx, 0x20, 0, 1);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x60);
}

/// Even updates 40..58 [F 0x1E04988]: one spike at node +0xC.
fn spike(fx: &mut Fx) {
    let i = first_free(fx, D + 0x340, 0x68, 0x20);
    if i < 0 {
        return;
    }
    let r = D + 0x340 + 0x68 * i as u32;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(r + 0x1C, 0x41A0_0000);
    fx.mem.write_u8(r + 8, 0x10);
    fx.mem.write_u8(r + 5, 1);
    let node = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, node);
    fx.mem.write_u8(r + 0xA, 0);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 3);
    let x = fx.rand_from(OT, 0x01E0_4B00);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0x4560);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    spike_colours(fx, r);
    fx.mem.write_u8(r + 0x23, 0x10);
    fx.mem.write_u8(r + 0x27, 0x40);
    for (o, ra) in [(0x58u32, 0x01E0_4C54u32), (0x5C, 0x01E0_4C80), (0x60, 0x01E0_4CAC)] {
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
    fx.unit_position(t, sp + 0x50);
    match fx.mem.u16(D) {
        0x8C => return 8,
        0x4B => {
            strike(fx, sp + 0x50);
            s1 = 0xC8;
        }
        0x21 => {
            start(fx);
            s1 = 0;
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x28..0x3B).contains(&n) && n & 1 == 0 {
        spike(fx);
    }
    blib::shake::camera_shake(fx, D + 0x40);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4CEC, [Some(u64::from(OT)), Some(s1)]);
    blib::fade::fade_c(fx, D + 0xA4, OT);
    blib::cone::cones_b(fx, 0x01E0_0440, D + 0xAC, OT, 6);
    blib::halo::halo(fx, 0x01E0_0E10, D + 0x25C, OT, 3);
    blib::spike::spikes_b(fx, 0x01E0_1B60, D + 0x340, OT, 0x20);
    blib::drop::drops(fx, D + 0x1040, OT, 0xC8);
    let n = fx.mem.u16(D);
    if (0x4B..0x60).contains(&n) {
        6
    } else if n == 0x60 {
        2
    } else if n == 0x41 {
        1
    } else {
        0
    }
}
