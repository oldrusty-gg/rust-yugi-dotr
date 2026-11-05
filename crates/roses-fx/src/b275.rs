//! BMODULE b275 (card 380), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03310, data area 0x1ED0000) [F] (helper D10, 31
//! Oct 2026): b105's library with a spark column. 512 strips
//! (`blib::spin::strips`, +0xE0 / +0x160) rise round the attacker's node
//! 0xD (+0x4160, turned to look at the target's point) and show from
//! update 0x81; from 0x96 32 sprites (+0x4240) burst out and down round
//! the target (+0x41D0); from 0x118 a column of 256 sparks
//! (`blib::trail::sparks`, +0x4510) rises round the attacker's point
//! (+0x44A0, turned to look at the target) and a pulsing tint (b262's,
//! colour +0x5550); a frame quad every update, the stage light down to
//! 0x20 (back up from 0x1C3). Returns 1 on update 0x8B, 6 on 0x95..0x1C1,
//! 2 on 0x1C2, 8 when the 0x1DF updates are over.
//!
//! Library (record addresses): `colours4` 0x1E00080, the frame quad
//! 0x1E00120, b262's tint 0x1E00240, `sprites` 0x1E003A0,
//! `spawn::scatter` 0x1E01C90, `trail::sparks` 0x1E01E50, `strips`
//! 0x1E02BB0.
use crate::blib;
use crate::blib::spin;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const N: u32 = D + 0x5558;
const LEFT: u32 = D + 0x5554;
const STRIPS: u32 = 0x01E0_2BB0;
const A: u32 = D + 0xE0;
const A_PARTS: u32 = D + 0x160;
const NODE_X: u32 = D + 0x4160;
const TARGET_X: u32 = D + 0x41D0;
const SPRITES: u32 = D + 0x4240;
const S_PARTS: u32 = D + 0x42A0;
const UNIT_X: u32 = D + 0x44A0;
const SPARKS: u32 = D + 0x4510;
const TINT: u32 = D + 0x5550;
const LIGHT: u32 = D + 0x556C;

fn angle(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) & 0xFFFF
}

fn tenth(fx: &mut Fx, ra: u32, m: i32) -> f32 {
    let r = fx.rand_from(D, ra);
    let (_, h) = fx.div(r, m);
    ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h))
}

/// The entry 0x1E03310.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param < 0 { step(fx, sp) } else { init(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

/// A sprite thrown out (and down) 14 from its group's point.
fn sprite_dir(fx: &mut Fx, sp: u32, p: u32) {
    fx.random_dir(sp + 0x70, 14.0);
    let y = fx.mem.f(sp + 0x74);
    if !ee_le(y, 0.0) {
        fx.mem.setf(sp + 0x74, ee::mul(y, -1.0));
    }
    for k in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x70 + 4 * k);
        fx.mem.write_u32(p + 4 * k, w);
    }
}

/// Spark `k` at a random angle, radius 0.1 x (rand % 200) and height 0.1
/// x (rand % `high`) (the sparks' array at +0x451C).
fn spark_place(fx: &mut Fx, k: u32, high: i32, ras: [u32; 3]) {
    let a = angle(fx, ras[0]);
    let radius = tenth(fx, ras[1], 200);
    let y = tenth(fx, ras[2], high);
    let e = fx.mem.read_u32(SPARKS + 0xC).wrapping_add(16 * k);
    fx.mem.setf(e + 4, y);
    let c = fx.tcos(a);
    let e = fx.mem.read_u32(SPARKS + 0xC).wrapping_add(16 * k);
    fx.mem.setf(e + 8, ee::mul(radius, c));
    let s = fx.tsin(a);
    let e = fx.mem.read_u32(SPARKS + 0xC).wrapping_add(16 * k);
    fx.mem.setf(e, ee::mul(radius, s));
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x5570);
    for x in [0u32, 0x70, 0x4160, 0x41D0, 0x44A0] {
        fx.xform_init(D + x);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0xD, 1);
    fx.mem.set32(D + 0x5564, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0xB, 0);
    fx.mem.set32(D + 0x5568, r);
    fx.mem.set32(LEFT, 0x1DF);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x5560, param);
    fx.mem.write_u32(D + 0x555C, 0);
    fx.mem.write_u32(LIGHT, 0x80);
    for (o, x) in [(2u32, 0xE0u8), (1, 0xB0), (0, 0x70)] {
        fx.mem.write_u8(TINT + o, x);
    }
    fx.texture4(sp + 0xA0, 0xF);
    fx.mem.write_u8(TINT + 3, 0);
    // The strips (+0xE0): flags 0x8003, colour (0xD0, 0xA0, 0x70), lifted
    // 10, growing x 1.1, 512 particles, 8 frames each update.
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(A, tex);
    for (o, x) in [(6u32, 0x70u8), (5, 0xA0), (4, 0xD0), (7, 0)] {
        fx.mem.write_u8(A + o, x);
    }
    fx.mem.write_u32(A + 8, 0x8003);
    fx.mem.write_u32(A + 0xC, A_PARTS);
    fx.mem.write_u32(A + 0x60, 0x4120_0000);
    for (o, x) in [(0x10u32, 0x80i32), (0x12, 0xC0), (0x14, 0x20), (0x16, 0x20), (0x18, 0x80), (0x1A, 0xC0), (0x1C, 0x80), (0x1E, 0x100)] {
        fx.mem.set16(A + o, x);
    }
    fx.mem.write_u32(A + 0x64, 0x3F8C_CCCD);
    for (o, x) in [(0x72u32, 0i32), (0x74, 0x18), (0x76, 0x200), (0x78, 0x200), (0x7C, 8), (0x7E, 1)] {
        fx.mem.set16(A + o, x);
    }
    for k in 0..4u32 {
        let t = ee::from_i32((0x1000 * k) as i32);
        let c = fx.cosf(t);
        fx.mem.setf(A + 0x28 + 0x10 * k, c);
        let s = fx.sinf(t);
        fx.mem.setf(A + 0x20 + 0x10 * k, s);
        fx.mem.write_u32(A + 0x24 + 0x10 * k, 0);
    }
    let mut k = 0;
    while k < fx.mem.i16(A + 0x76) {
        let p = A_PARTS + 0x20 * k as u32;
        fx.mem.set16(p + 0xE, 0);
        fx.mem.write_u8(p + 0xC, 0);
        let r = fx.rand_from(D, 0x01E0_34F0);
        fx.mem.set8(p + 0xD, r % 8);
        let y = tenth(fx, 0x01E0_350C, 0x708);
        fx.mem.setf(p + 4, y);
        let a = angle(fx, 0x01E0_353C);
        fx.mem.set32(p + 0x14, a);
        let s = tenth(fx, 0x01E0_3558, 0x3C);
        fx.mem.setf(p + 0x10, s);
        k += 1;
    }
    // The sprites (+0x4240): flags 0x48000003, alpha 0x20, size 10, 32
    // particles at +0x42A0.
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(SPRITES, tex);
    fx.mem.write_u32(SPRITES + 4, 0x4800_0003);
    for (o, x) in [(0xAu32, 0x70u8), (9, 0xA0), (8, 0xD0), (0xB, 0x20)] {
        fx.mem.write_u8(SPRITES + o, x);
    }
    fx.mem.write_u32(SPRITES + 0xC, 0x4120_0000);
    for o in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(SPRITES + o, 0);
    }
    for (o, x) in [(0x30u32, 0x80i32), (0x32, 0xC0), (0x34, 0x20), (0x36, 0x20), (0x38, 0x80), (0x3A, 0xC0), (0x3C, 0x80), (0x3E, 0x40)] {
        fx.mem.set16(SPRITES + o, x);
    }
    fx.mem.write_u32(SPRITES + 0x40, S_PARTS);
    fx.mem.write_u32(SPRITES + 0x44, 0);
    fx.mem.write_u32(SPRITES + 0x48, 0);
    fx.mem.write_u32(SPRITES + 0x4C, 0x4170_0000);
    fx.mem.write_u32(SPRITES + 0x50, 0x4120_0000);
    fx.mem.set16(SPRITES + 0x5A, 0x20);
    fx.mem.set16(SPRITES + 0x5C, 8);
    fx.mem.set16(SPRITES + 0x5E, 0x10);
    let mut k = 0;
    while k < fx.mem.i16(SPRITES + 0x5A) {
        let p = S_PARTS + 0x10 * k as u32;
        sprite_dir(fx, sp, p);
        let r = fx.rand_from(D, 0x01E0_36AC);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, -h);
        fx.mem.write_u8(p + 0xC, 0x20);
        let r = fx.rand_from(D, 0x01E0_36D4);
        fx.mem.set8(p + 0xD, r % 8);
        k += 1;
    }
    // The sparks (+0x4510): flags 0x60000003, white, width 2.4, length
    // 22, 256 at +0x4540, scattered (life 12, speed 3).
    fx.texture5(sp + 0xA0, 0);
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(SPARKS, tex);
    fx.mem.write_u32(SPARKS + 4, 0x6000_0003);
    for o in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(SPARKS + o, 0x80);
    }
    fx.mem.write_u32(SPARKS + 0x20, 0x4019_999A);
    fx.mem.write_u32(SPARKS + 0x24, 0x41B0_0000);
    for (o, x) in [(0x10u32, 0i32), (0x12, 0), (0x14, 0x20), (0x16, 0x20), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)] {
        fx.mem.set16(SPARKS + o, x);
    }
    fx.mem.write_u32(SPARKS + 0xC, SPARKS + 0x30);
    fx.mem.write_u32(SPARKS + 0x28, 0x41F0_0000);
    fx.mem.set16(SPARKS + 0x2C, 0x100);
    fx.mem.write_u8(SPARKS + 0x2E, 8);
    fx.mem.write_u8(SPARKS + 0x2F, 0);
    blib::spawn::scatter(fx, SPARKS, 0xC, 3.0);
    let mut k = 0;
    while k < fx.mem.i16(SPARKS + 0x2C) {
        spark_place(fx, k as u32, 0x7D0, [0x01E0_37C4, 0x01E0_37E0, 0x01E0_380C]);
        k += 1;
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x5540);
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x5564);
    fx.node_position(h, s, sp + 0x80);
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x5568);
    fx.node_position(h, s, sp + 0x90);
    for k in 0..4u32 {
        let w = fx.mem.read_u32(sp + 0x80 + 4 * k);
        fx.mem.write_u32(NODE_X + 0x20 + 4 * k, w);
    }
    fx.look_angles(D + 0x5540, sp + 0x80, NODE_X + 0x10);
    let t = fx.target_handle();
    fx.unit_position(t, TARGET_X + 0x20);
    let h = fx.self_handle();
    fx.unit_point(1, h, UNIT_X + 0x20);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x80);
    let h = fx.self_handle();
    fx.unit_point(1, h, sp + 0x90);
    fx.look_angles(sp + 0x80, sp + 0x90, UNIT_X + 0x10);
    // s0 = D, s1 = the OT, s2 = sp + 0x90, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(sp + 0x90), Some(0)];
    fx.xform_world_saving(D, 0x01E0_397C, regs);
    fx.xform_billboard_saving(D + 0x70, 0x01E0_3984, regs);
    fx.xform_world_saving(NODE_X, 0x01E0_398C, regs);
    fx.xform_world_saving(TARGET_X, 0x01E0_3994, regs);
    fx.xform_world_saving(UNIT_X, 0x01E0_399C, regs);
    fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    if n(fx) < 0x1C3 {
        fx.step_down(LIGHT, 0x20, 4);
    } else {
        fx.step_up(LIGHT, 0x80, 4);
    }
    let light = fx.mem.u8(LIGHT);
    fx.set_stage_light(light as i32);
    if n(fx) >= 0x118 {
        if n(fx) == 0x1C2 {
            fx.mem.write_u8(TINT + 3, 0x80);
        }
        crate::b262::tint(fx, TINT);
        if n(fx) >= 0x1C2 {
            fx.step_down(TINT + 3, 0, 8);
        } else if bit(fx.mem.read_u32(D + 0x555C), 0x1F) {
            if fx.step_down(TINT + 3, 0x20, 4) == 0 {
                let w = fx.mem.read_u32(D + 0x555C) & 0x7FFF_FFFF;
                fx.mem.write_u32(D + 0x555C, w);
            }
        } else if fx.step_up(TINT + 3, 0x40, 4) == 0 {
            let w = fx.mem.read_u32(D + 0x555C) | 0x8000_0000;
            fx.mem.write_u32(D + 0x555C, w);
        }
    }
    spin::strips(fx, STRIPS, OT, A, A_PARTS, NODE_X);
    let m = n(fx);
    if m < 0x81 || m >= 0x1C2 {
        fx.step_down(A + 7, 0, 1);
    } else {
        fx.step_up(A + 7, 0x10, 1);
    }
    let mut k = 0;
    while k < fx.mem.i16(A + 0x76) {
        let p = A_PARTS + 0x20 * k as u32;
        let y = ee::add(fx.mem.f(p + 4), if k % 2 == 0 { 4.0 } else { 6.0 });
        fx.mem.setf(p + 4, y);
        if !ee_lt(fx.mem.f(p + 4), 180.0) {
            fx.mem.set16(p + 0xE, 0);
            let y = tenth(fx, 0x01E0_3B88, 0x1E);
            fx.mem.setf(p + 4, y);
            let s = tenth(fx, 0x01E0_3BB8, 0xF);
            fx.mem.setf(p + 0x10, s);
        }
        if n(fx) >= 0x81 && fx.mem.i16(p + 0xE) == 0 {
            let a = fx.mem.read_u8(A + 7);
            fx.mem.write_u8(p + 0xC, a);
        }
        k += 1;
    }
    if n(fx) >= 0x96 {
        fx.sprites(SPRITES, TARGET_X);
        if n(fx) >= 0x1C2 {
            fx.step_down(SPRITES + 0xB, 0, 2);
        }
        let mut k = 0;
        while k < fx.mem.i16(SPRITES + 0x5A) {
            let p = S_PARTS + 0x10 * k as u32;
            if fx.mem.i16(p + 0xE) >= 0 {
                let f = f32::from_bits(0x3F86_6666);
                let x = ee::mul(fx.mem.f(p), f);
                fx.mem.setf(p, x);
                let y = ee::mul(fx.mem.f(p + 4), f32::from_bits(0x3F88_F5C3));
                fx.mem.setf(p + 4, y);
                let z = ee::mul(fx.mem.f(p + 8), f);
                fx.mem.setf(p + 8, z);
                fx.step_down(p + 0xC, 0, 2);
            }
            if fx.mem.i16(p + 0xE) >= 0x10 {
                sprite_dir(fx, sp, p);
                fx.mem.set16(p + 0xE, 0);
                fx.mem.write_u8(p + 0xC, 0x20);
            }
            k += 1;
        }
    }
    if n(fx) >= 0x118 {
        blib::trail::sparks(fx, OT, SPARKS, UNIT_X);
        if n(fx) >= 0x1C3 {
            fx.step_down(SPARKS + 0xB, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(SPARKS + 0x2C) {
            let p = D + 0x10 * k as u32;
            if fx.mem.i16(p + 0x454E) >= 0 {
                let y = ee::add(fx.mem.f(p + 0x4544), 12.0);
                fx.mem.setf(p + 0x4544, y);
                if !ee_le(y, 200.0) {
                    spark_place(fx, k as u32, 0x3C, [0x01E0_3DF0, 0x01E0_3E0C, 0x01E0_3E38]);
                    fx.mem.set16(p + 0x454E, 0);
                }
            }
            k += 1;
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0x1C2 {
        2
    } else if m == 0x8B {
        1
    } else if (0x95..0x1C2).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
