//! BMODULE b196 (card 116), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04A80, data area a0 = 0x1ED0000) [F] (helper A15,
//! 31 Oct 2026): b201's family (halo / spikes_b / drops / fade_c / cones_b,
//! compiled without scheduling) with b044's sprite-ring emitter: on update
//! 38 an emitter (+0x1848, `b044::emitters_with`) of 120 on the attacker's
//! node 0x4D leaving sprite rings (+0x308, 0x50 x 0x44, `b044::rings_with`)
//! on its path, on 68 three cones (+0xA4, 0x48 each, delays 5 k) at node
//! 0x4E and the emitter's rings recoloured, on 80 the emitter paused (+4 =
//! 0x80), on 88 at the target a fade (+0x2E40), six cones, 80 drops
//! (+0x1900, 0x44 each), a spike (+0x254), a halo of 1000 (+0x2BC) and a
//! shake (+0x40), on 118 the emitter moved to node 0x4E for 12 more; the
//! self unit's u16 +0x20A = 1 on 79..119; the stage light by 0x1031C0
//! (+0xA0: down on 2, up on 204). Returns 1 on update 78, 6 on 88..108, 2
//! on 109, 8 on 210 (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` (ac7dccf5ae36) 0x1E00080,
//! `halo` 0x1E01540, `cones_b` 0x1E02290, `spikes_b` 0x1E02C60, `drops`
//! 0x1E03990, `fade_c` 0x1E04510 [`crate::blib`]; record-local `rings`
//! 0x1E00440 (ea834aa07372) and `emitters` 0x1E010B0 (a8e977094cd0) =
//! b044's compiled with other frames / register allocation [`crate::b044`].
use crate::b035::light_ramp;
use crate::b044::{emitters_with, rings_with};
use crate::b076::{copy_v4, rand_bytes};
use crate::b369::unit_flag;
use crate::b463::spike_colours;
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const EMITTER: u32 = D + 0x1848;
const CONES: u32 = D + 0xA4;

/// The entry 0x1E04A80 (a0 = D, a1 = param).
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
    fx.bzero(D, 0x2F50);
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
    for (at, node, clear) in [(0xCu32, 0x4D, 1), (0x10, 0x4E, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xCC);
    // The entry's s0 = the OT, s1 = 2 (the texture loop's), s2 = 0 (the
    // runner's).
    rand_bytes(fx, D + 0x2E48, 0x100, 0x01E0_4B9C, [Some(OT), Some(2), Some(0)]);
    0
}

/// Update 38 [F 0x1E04C18]: the emitter on node +0xC.
fn emitter_start(fx: &mut Fx) {
    let g = EMITTER;
    set_counter4(fx, g, 0x78, 0, 0xFF);
    fx.mem.write_u8(g + 4, 0);
    fx.mem.write_u8(g + 5, 1);
    let node = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(g + 6, node);
    fx.mem.write_u32(g + 8, D + 0x308);
    fx.mem.write_u32(g + 0xC, D + 0x2E48);
    for (o, v) in [(0x10u32, 0x50u8), (0x11, 0x10), (0x12, 4), (0x13, 0), (0x14, 5), (0x15, 2), (0x16, 1), (0x17, 3)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 0x18, 0x3F19_999A);
    fx.mem.write_u32(g + 0x1C, 0x4000_0000);
    fx.mem.write_u32(g + 0x20, 0x41A0_0000);
    fx.mem.write_u32(g + 0x24, 0x41A0_0000);
    for (o, v) in [(0x28u32, 0x80u8), (0x29, 0x80), (0x2A, 0x80), (0x2B, 0x40)] {
        fx.mem.write_u8(g + o, v);
    }
    let h = fx.mem.i32(D + 4);
    let slot = fx.mem.i32(D + 0xC);
    fx.node_position(h, slot, g + 0x3C);
    copy_v4(fx, g + 0x3C, g + 0x4C);
}

/// One cone record's constants shared by both spawns.
fn cone(fx: &mut Fx, c: u32, b8: u8, scale: u32, speed: u32, rise: u32, grow: u32, colours: [u8; 8]) {
    for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, b8), (0xA, 0x20), (9, 7)] {
        fx.mem.write_u8(c + o, v);
    }
    fx.mem.write_u32(c + 0xC, scale);
    fx.mem.write_u32(c + 0x10, speed);
    fx.mem.write_u32(c + 0x14, rise);
    fx.mem.write_u32(c + 0x18, 0);
    fx.mem.write_u32(c + 0x1C, grow);
    for (o, v) in [0x22u32, 0x21, 0x20, 0x23, 0x26, 0x25, 0x24, 0x27].into_iter().zip(colours) {
        fx.mem.write_u8(c + o, v);
    }
}

fn cone_turn(fx: &mut Fx, c: u32) {
    let turn = if fx.mem.i32(D + 0x30) == 0x8000 { 0x4000 } else { -0x4000 };
    fx.mem.set32(c + 0x38, turn);
}

/// Update 68 [F 0x1E04D08]: the emitter's rings recoloured, three cones at
/// node +0x10.
fn cones_start(fx: &mut Fx) {
    fx.mem.write_u8(EMITTER + 0x16, 2);
    for (o, v) in [(0x2Au32, 0xC0u8), (0x29, 0x40), (0x28, 0x20), (0x2B, 0x60)] {
        fx.mem.write_u8(EMITTER + o, v);
    }
    for k in 0..3i32 {
        let c = CONES + 0x48 * k as u32;
        set_counter4(fx, c, 0x14, (5 * k) & 0xFF, 0xFF);
        cone(fx, c, 1, 0x4080_0000, 0x3F4C_CCCD, 0x3FE6_6666, 0x3E4C_CCCD, [0xC0, 0x20, 0x10, 0x80, 0x80, 0x80, 0x80, 0xA0]);
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0x10);
        fx.node_position(h, slot, c + 0x28);
        cone_turn(fx, c);
    }
}

/// ((rand >> 4) % 220 - 110) / 100 [F].
fn spread(fx: &mut Fx, ra: u32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, 0xDC);
    ee::div(ee::from_i32(h - 0x6E), 100.0)
}

/// Update 88 [F 0x1E04FC4]: at the target `at`.
fn strike(fx: &mut Fx, at: u32) {
    set_counter4(fx, D + 0x2E40, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0x2E44, 1);
    for k in 0..6i32 {
        let c = CONES + 0x48 * k as u32;
        set_counter4(fx, c, 0x18, k & 0xFF, 0xFF);
        cone(fx, c, 2, 0x4000_0000, 0x3E99_999A, 0x3FC0_0000, 0x3E23_D70A, [0xA0, 0xA0, 0xA0, 0, 0xC0, 0x20, 0x10, 0xC0]);
        copy_v4(fx, at, c + 0x28);
        cone_turn(fx, c);
    }
    for _ in 0..0x50 {
        let i = first_free(fx, D + 0x1900, 0x44, 0x50);
        if i < 0 {
            continue;
        }
        let r = D + 0x1900 + 0x44 * i as u32;
        let x = fx.rand_from(OT, 0x01E0_52A0);
        set_counter4(fx, r, 0x20, (x >> 4) & 7, 0xFF);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u8(r + 8, 1);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 9, 6);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let x = fx.rand_from(OT, 0x01E0_5398) >> 4;
        let (_, h) = fx.div(x, 0x14);
        let v = ee::add(100.0, ee::div(ee::from_i32(h.wrapping_mul(200) - 2000), 100.0));
        fx.mem.setf(r + 0x10, v);
        fx.mem.write_u32(r + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(r + 0x18, 0xBDA3_D70A);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let v = spread(fx, 0x01E0_54A4);
        fx.mem.setf(r + 0x2C, v);
        let v = spread(fx, 0x01E0_54F8);
        fx.mem.setf(r + 0x30, ee::add(1.0, v));
        let v = spread(fx, 0x01E0_555C);
        fx.mem.setf(r + 0x34, v);
    }
    let r = D + 0x254;
    set_counter4(fx, r, 0x18, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4033_3333);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 0xA, 1);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 3);
    fx.mem.write_u8(r + 8, 0xA0);
    let x = fx.rand_from(OT, 0x01E0_561C);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, D + 0x2E48);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    spike_colours(fx, r);
    fx.mem.write_u8(r + 0x23, 0x20);
    fx.mem.write_u8(r + 0x27, 0x80);
    copy_v4(fx, at, r + 0x28);
    fx.mem.write_u32(r + 0x58, 0x40);
    fx.mem.write_u32(r + 0x5C, 0x80);
    fx.mem.write_u32(r + 0x60, 0x100);
    let g = D + 0x2BC;
    set_counter4(fx, g, 0x1C, 4, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x447A_0000);
    fx.mem.write_u32(g + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x20), (8, 0x80), (0xF, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    copy_v4(fx, at, g + 0x1C);
    let s = D + 0x40;
    set_counter4(fx, s, 0x1C, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, then the drop loop's count.
    let mut s1 = param as i64 as u64;
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, sp + 0x50);
    match fx.mem.u16(D) {
        0xD2 => return 8,
        0x76 => {
            fx.mem.write_u8(EMITTER + 4, 0);
            let node = fx.mem.read_u8(D + 0x10);
            fx.mem.write_u8(EMITTER + 6, node);
            fx.mem.write_u8(EMITTER + 0x11, 0xC);
        }
        0x58 => {
            strike(fx, sp + 0x50);
            s1 = 0x50;
        }
        0x50 => fx.mem.write_u8(EMITTER + 4, 0x80),
        0x44 => cones_start(fx),
        0x26 => emitter_start(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x4F..0x78).contains(&n) {
        unit_flag(fx, 1, u64::from(OT), 0x01E0_57F8);
    }
    blib::shake::camera_shake(fx, D + 0x40);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5814, [Some(u64::from(OT)), Some(s1)]);
    blib::halo::halo(fx, 0x01E0_1540, D + 0x2BC, OT, 1);
    blib::spike::spikes_b(fx, 0x01E0_2C60, D + 0x254, OT, 1);
    blib::cone::cones_b(fx, 0x01E0_2290, CONES, OT, 6);
    emitters_with(fx, EMITTER, 2, 0x01E0_133C);
    rings_with(fx, D + 0x308, OT, 0x50, 0x1D0);
    blib::drop::drops(fx, D + 0x1900, OT, 0x50);
    blib::fade::fade_c(fx, D + 0x2E40, OT);
    let n = fx.mem.u16(D);
    if (0x58..0x6D).contains(&n) {
        6
    } else if n == 0x6D {
        2
    } else if n == 0x4E {
        1
    } else {
        0
    }
}
