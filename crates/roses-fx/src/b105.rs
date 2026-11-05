//! BMODULE b105 (card 96), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E028F0, data area 0x1ED0000) [F]: b423's library with two strip groups. 512 tall blue strips
//! (`blib::spin::strips`, +0xE0 / +0x160) rise round the attacker's node
//! 0x2D (+0x51E0, turned to look at the target's point) and show from
//! update 0x4E; 128 small ones (+0x4160 / +0x41E0) round the attacker's
//! point (+0x5910) fade out from 0x4E; 60 spinning motes (+0x5250) round
//! the node; 32 sprites (+0x5980) burst out and down round the attacker
//! and respawn. Returns 1 on update 0x6E, 6 on 0x78..0x15D, 2 on 0x15E, 8
//! when the 0x17B updates are over.
//!
//! Library (record addresses): `colours4` 0x1E00080, `motes` 0x1E00120,
//! `sprites` 0x1E008A0, `strips` 0x1E02190.
use crate::blib::spin;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const N: u32 = D + 0x5BF4;
const LEFT: u32 = D + 0x5BF0;
const STRIPS: u32 = 0x01E0_2190;
const A: u32 = D + 0xE0;
const A_PARTS: u32 = D + 0x160;
const B: u32 = D + 0x4160;
const B_PARTS: u32 = D + 0x41E0;
const MOTES: u32 = D + 0x5250;
const SPRITES: u32 = D + 0x5980;
const S_PARTS: u32 = D + 0x59E0;
const NODE_X: u32 = D + 0x51E0;
const UNIT_X: u32 = D + 0x5910;

fn angle(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) & 0xFFFF
}

fn tenth(fx: &mut Fx, ra: u32, m: i32) -> f32 {
    let r = fx.rand_from(D, ra);
    let (_, h) = fx.div(r, m);
    ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h))
}

/// The entry 0x1E028F0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    let ret = if param < 0 { step(fx, sp, param) } else { init(fx, sp, param) };
    fx.leave(0xD0);
    ret
}

/// A strip group's header: flags, the lift, growth, count.
fn strips_init(fx: &mut Fx, g: u32, parts: u32, flags: u32, lift: u32, grow: u32, n: i32) {
    for (o, x) in [(6u32, 0x70u8), (5, 0xA0), (4, 0xD0), (7, 0)] {
        fx.mem.write_u8(g + o, x);
    }
    fx.mem.write_u32(g + 8, flags);
    fx.mem.write_u32(g + 0xC, parts);
    fx.mem.write_u32(g + 0x60, lift);
    for (o, x) in [(0x10u32, 0x80i32), (0x12, 0xC0), (0x14, 0x20), (0x16, 0x20), (0x18, 0x80), (0x1A, 0xC0), (0x1C, 0x80), (0x1E, 0x100)] {
        fx.mem.set16(g + o, x);
    }
    fx.mem.write_u32(g + 0x64, grow);
    for (o, x) in [(0x72u32, 0i32), (0x74, 0x18), (0x76, n), (0x78, n), (0x7C, 8), (0x7E, 1)] {
        fx.mem.set16(g + o, x);
    }
    for k in 0..4u32 {
        let t = ee::from_i32((0x1000 * k) as i32);
        let c = fx.cosf(t);
        fx.mem.setf(g + 0x28 + 0x10 * k, c);
        let s = fx.sinf(t);
        fx.mem.setf(g + 0x20 + 0x10 * k, s);
        fx.mem.write_u32(g + 0x24 + 0x10 * k, 0);
    }
}

/// A sprite thrown out (and down) 14 from the attacker's point.
fn sprite_dir(fx: &mut Fx, sp: u32, p: u32) {
    fx.random_dir(sp + 0x90, 14.0);
    let y = fx.mem.f(sp + 0x94);
    if !ee_le(y, 0.0) {
        fx.mem.setf(sp + 0x94, ee::mul(y, -1.0));
    }
    for k in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x90 + 4 * k);
        fx.mem.write_u32(p + 4 * k, w);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x5C10);
    for x in [0u32, 0x70, 0x51E0, 0x5910] {
        fx.xform_init(D + x);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x2D, 1);
    fx.mem.set32(D + 0x5BFC, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x2C, 0);
    fx.mem.set32(D + 0x5C00, r);
    fx.mem.set32(LEFT, 0x17B);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x5BF8, param);
    fx.texture5(sp + 0xC0, 0);
    // The motes: flags 0x40001001, half size 1.0, cell (0, 0, 0x40, 0x80).
    let tex = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(MOTES, tex);
    fx.mem.write_u32(MOTES + 4, 0x4000_1001);
    fx.mem.write_u32(MOTES + 0xC, 0x3F80_0000);
    for (o, x) in [(0x20u32, 0i32), (0x22, 0), (0x24, 0x40), (0x26, 0x80)] {
        fx.mem.set16(MOTES + o, x);
    }
    for o in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(MOTES + o, 0x80);
    }
    for (o, x) in [(0x10u32, 0u32), (0x14, 0x400), (0x18, 0)] {
        fx.mem.write_u32(MOTES + o, x);
    }
    for k in 0..0x3Cu32 {
        let p = MOTES + 0x3F0 + 0xC * k;
        let a = angle(fx, 0x01E0_2A14);
        let y = tenth(fx, 0x01E0_2A30, 0x708);
        fx.mem.setf(p + 4, y);
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(2.0, c));
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(2.0, s));
        for (o, ra) in [(0u32, 0x01E0_2A94u32), (4, 0x01E0_2AB0), (8, 0x01E0_2ACC)] {
            let a = angle(fx, ra);
            fx.mem.set32(MOTES + 0x30 + 0x10 * k + o, a);
        }
    }
    fx.texture4(sp + 0xC0, 0xF);
    let tex = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(A, tex);
    strips_init(fx, A, A_PARTS, 0x9003, 0x4120_0000, 0x3F8C_CCCD, 0x200);
    let mut k = 0;
    while k < fx.mem.i16(A + 0x76) {
        let p = A_PARTS + 0x20 * k as u32;
        fx.mem.set16(p + 0xE, 0);
        fx.mem.write_u8(p + 0xC, 0);
        let r = fx.rand_from(D, 0x01E0_2C00);
        fx.mem.set8(p + 0xD, r % 8);
        let y = tenth(fx, 0x01E0_2C1C, 0x708);
        fx.mem.setf(p + 4, y);
        let a = angle(fx, 0x01E0_2C4C);
        fx.mem.set32(p + 0x14, a);
        let s = tenth(fx, 0x01E0_2C68, 0x3C);
        fx.mem.setf(p + 0x10, s);
        k += 1;
    }
    let tex = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(B, tex);
    strips_init(fx, B, B_PARTS, 0x8003, 0x4080_0000, 0x3F8A_3D71, 0x80);
    let mut k = 0;
    while k < fx.mem.i16(B + 0x76) {
        let p = B_PARTS + 0x20 * k as u32;
        fx.mem.set16(p + 0xE, 0);
        fx.mem.write_u8(p + 0xC, 0x10);
        let r = fx.rand_from(D, 0x01E0_2DAC);
        fx.mem.set8(p + 0xD, r % 8);
        let y = tenth(fx, 0x01E0_2DC8, 0x1F4);
        fx.mem.setf(p + 4, y);
        let a = angle(fx, 0x01E0_2DF8);
        fx.mem.set32(p + 0x14, a);
        let s = tenth(fx, 0x01E0_2E14, 0x3C);
        fx.mem.setf(p + 0x10, s);
        k += 1;
    }
    // The sprites: flags 0x48000003, size 10, 32 particles at +0x59E0.
    let tex = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(SPRITES, tex);
    fx.mem.write_u32(SPRITES + 4, 0x4800_0003);
    for (o, x) in [(0xAu32, 0x70u8), (9, 0xA0), (8, 0xD0), (0xB, 0x40)] {
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
        let r = fx.rand_from(D, 0x01E0_2F64);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, -h);
        fx.mem.write_u8(p + 0xC, 0x40);
        let r = fx.rand_from(D, 0x01E0_2F8C);
        fx.mem.set8(p + 0xD, r % 8);
        k += 1;
    }
    0
}

/// A strip group's particles rising by `up` (odd ones x 1.5) and
/// restarting at `top`; from update `show` restarted ones take the
/// group's alpha.
#[allow(clippy::too_many_arguments)]
fn rise(fx: &mut Fx, g: u32, parts: u32, up: (f32, f32), top: f32, size_m: i32, show: i32, ra: [u32; 2]) {
    let mut k = 0;
    while k < fx.mem.i16(g + 0x76) {
        let p = parts + 0x20 * k as u32;
        let y = ee::add(fx.mem.f(p + 4), if k % 2 == 0 { up.0 } else { up.1 });
        fx.mem.setf(p + 4, y);
        if !ee_lt(fx.mem.f(p + 4), top) {
            fx.mem.set16(p + 0xE, 0);
            let y = tenth(fx, ra[0], 0x1E);
            fx.mem.setf(p + 4, y);
            let s = tenth(fx, ra[1], size_m);
            fx.mem.setf(p + 0x10, s);
        }
        if fx.mem.i32(N) >= show && fx.mem.i16(p + 0xE) == 0 {
            let a = fx.mem.read_u8(g + 7);
            fx.mem.write_u8(p + 0xC, a);
        }
        k += 1;
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x5BE0);
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x5BFC);
    fx.node_position(h, s, sp + 0xA0);
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x5C00);
    fx.node_position(h, s, sp + 0xB0);
    for k in 0..4u32 {
        let w = fx.mem.read_u32(sp + 0xA0 + 4 * k);
        fx.mem.write_u32(NODE_X + 0x20 + 4 * k, w);
    }
    fx.look_angles(D + 0x5BE0, sp + 0xA0, NODE_X + 0x10);
    let h = fx.self_handle();
    fx.unit_point(1, h, UNIT_X + 0x20);
    fx.mem.write_u32(UNIT_X + 0x24, 0x41F0_0000);
    // s0 = D, s1 = the OT, s2 = the parameter, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(param as u32), Some(0)];
    fx.xform_world_saving(D, 0x01E0_3068, regs);
    fx.xform_billboard_saving(D + 0x70, 0x01E0_3070, regs);
    fx.xform_world_saving(NODE_X, 0x01E0_3078, regs);
    fx.xform_world_saving(UNIT_X, 0x01E0_3080, regs);
    spin::strips(fx, STRIPS, OT, A, A_PARTS, NODE_X);
    if n(fx) == 0x4E {
        fx.mem.write_u8(A + 7, 0x10);
    }
    if n(fx) >= 0x15F {
        fx.step_down(A + 7, 0, 1);
    }
    rise(fx, A, A_PARTS, (4.0, 6.0), 180.0, 0xF, 0x4E, [0x01E0_3150, 0x01E0_3180]);
    spin::strips(fx, STRIPS, OT, B, B_PARTS, UNIT_X);
    if n(fx) < 0x4E {
        fx.step_up(B + 7, 0x10, 1);
    } else {
        fx.step_down(B + 7, 0, 1);
    }
    rise(fx, B, B_PARTS, (2.0, 3.0), 50.0, 0x1E, 0x50, [0x01E0_32C0, 0x01E0_32F0]);
    fx.mem.write_u32(sp + 0x94, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x98, 0x3F8C_CCCD);
    fx.mem.write_u32(sp + 0x90, 0x3F8C_CCCD);
    spin::motes(fx, OT, MOTES, NODE_X, sp + 0x90);
    if n(fx) >= 0x15E {
        fx.step_down(MOTES + 0xB, 0, 8);
    }
    for k in 0..0x3Cu32 {
        let p = MOTES + 0x3F0 + 0xC * k;
        let y = ee::add(fx.mem.f(p + 4), if k % 2 == 0 { 4.0 } else { 6.0 });
        fx.mem.setf(p + 4, y);
        if !ee_lt(fx.mem.f(p + 4), 180.0) {
            let a = angle(fx, 0x01E0_3438);
            fx.mem.write_u32(p + 4, 0);
            let c = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(4.0, c));
            let s = fx.tsin(a);
            fx.mem.setf(p, ee::mul(4.0, s));
        }
        for o in [0u32, 4, 8] {
            let a = MOTES + 0x30 + 0x10 * k + o;
            let v = fx.mem.read_u32(a).wrapping_add(0x1000);
            fx.mem.write_u32(a, v);
        }
    }
    fx.sprites(SPRITES, UNIT_X);
    if n(fx) >= 0x6E {
        fx.step_down(SPRITES + 0xB, 0x10, 1);
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
            fx.step_down(p + 0xC, 0, 4);
        }
        if fx.mem.i16(p + 0xE) >= 0x10 {
            sprite_dir(fx, sp, p);
            fx.mem.set16(p + 0xE, 0);
            fx.mem.write_u8(p + 0xC, 0x40);
        }
        k += 1;
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0x15E {
        2
    } else if m == 0x6E {
        1
    } else if (0x78..0x15E).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
