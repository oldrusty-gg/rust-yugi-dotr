//! BMODULE b416 (card 509), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E00E40, data area 0x1ED0000) [F] (helper C, 30 Oct
//! 2026): two particle-quad clouds at the target (+0xE0 TEXEFF 2, from
//! update 116, fading by 8; +0x230 the attacker's texture 1, growing by
//! 1.01 per update from update 123, fading by 4), seven transforms kept on
//! the attacker's nodes 0x13 / 0x2C..0x2F (turned to look along node
//! pairs) and three sprite groups set up but never drawn. Returns 1 on
//! update 105, 6 on 115..129, 2 on 130, 8 after 170.
//!
//! Library: `particle_quads` (b416 0x1E002B0), `sprites_spawn`
//! (0x1E00120) [`crate::fxlib`]; the executable's 0x103DB0 / 0x103290 /
//! 0x103260 ([`crate::libm`]).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E00E40.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xE0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xE0);
    ret
}

fn halves(fx: &mut Fx, at: u32, v: [(u32, i32); 6]) {
    for (k, x) in v {
        fx.mem.set16(at + k, x);
    }
}

/// A particle-quad group's particles (count +0x130) scattered by
/// rand % 20 - 10 round its origin (+0x10, +0x14; z +0x18), ticks -k
/// x +0x13A; `frames`: a random frame rand % 8 (else 0).
fn scatter(fx: &mut Fx, g: u32, frames: bool) {
    let mut k = 0;
    while k < fx.mem.i16(g + 0x130) {
        let p = g + 16 * k as u32;
        for c in 0..2u32 {
            let r = fx.rand();
            let base = fx.mem.f(g + 0x10 + 4 * c);
            let (_, h) = fx.div(r, 20);
            let v = ee::add(base, ee::from_i32(h - 10));
            fx.mem.setf(p + 0x30 + 4 * c, v);
        }
        let z = fx.mem.read_u32(g + 0x18);
        fx.mem.write_u32(p + 0x38, z);
        let t = fx.mem.i16(g + 0x13A).wrapping_neg().wrapping_mul(k);
        fx.mem.set16(p + 0x3E, t);
        if frames {
            let r = fx.rand();
            fx.mem.set16(p + 0x3C, r % 8);
        } else {
            fx.mem.set16(p + 0x3C, 0);
        }
        let n = fx.mem.i16(g + 0x140) - 1;
        fx.mem.set16(g + 0x13E, n);
        k += 1;
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2560);
    fx.xform_init(D);
    fx.xform_init(D + 0x70);
    for k in 0..3u32 {
        fx.xform_init(D + 0x400 + 0x70 * k);
    }
    for k in 0..2u32 {
        let x = D + 0x1E70 + 0x70 * k;
        fx.xform_init(x);
        for c in [8u32, 4, 0] {
            fx.mem.write_u32(x + c, 0x3DCC_CCCD);
        }
    }
    fx.xform_init(D + 0x380);
    for (at, node, clear) in [(0x253Cu32, 0x13, 1), (0x2540, 0x12, 0), (0x2544, 0x2F, 0), (0x2548, 0x2E, 0), (0x254C, 0x2D, 0), (0x2550, 0x2C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2530, 0xAA);
    fx.mem.set32(D + 0x2534, 1);
    fx.mem.set32(D + 0x2538, param);
    // A disc record (+0x3F0), never drawn.
    fx.mem.write_u32(D + 0x3F0, 0);
    for (k, v) in [(0x3F6u32, 0x80u8), (0x3F5, 0x80), (0x3F4, 0x40), (0x3F7, 0xFF)] {
        fx.mem.write_u8(D + k, v);
    }
    fx.mem.write_u32(D + 0x3F8, 0x4000_0000);
    fx.mem.set16(D + 0x3FC, 0x24);
    fx.mem.set16(D + 0x3FE, 0);
    fx.texture4(sp + 0xD0, 2);
    // Cloud A (+0xE0): 16 quads, size 10, TEXEFF 2.
    let g = D + 0xE0;
    let tex = fx.mem.read_u32(sp + 0xD0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x0008_0007);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, [(0x28, 0), (0x2A, 0), (0x24, 0x20), (0x26, 0x20), (0x2C, 0x80), (0x2E, 0x40)]);
    fx.mem.write_u32(g + 0xC, 0x4120_0000);
    halves(fx, g, [(0x130, 0x10), (0x13A, 1), (0x13C, 0), (0x140, 8), (0x136, 5), (0x138, 0xA)]);
    for (k, v) in [(0xAu32, 0x40u8), (9, 0x80), (8, 0x20), (0xB, 0x80)] {
        fx.mem.write_u8(g + k, v);
    }
    scatter(fx, g, false);
    fx.texture5(sp + 0xD0, 1);
    // Cloud B (+0x230): 16 quads, size 20, the attacker's texture 1.
    let g = D + 0x230;
    let tex = fx.mem.read_u32(sp + 0xD0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x485);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, [(0x28, 0), (0x2A, 0), (0x24, 0x20), (0x26, 0x20), (0x2C, 0x80), (0x2E, 0x40)]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    halves(fx, g, [(0x130, 0x10), (0x13A, 1), (0x13C, 1), (0x140, 8), (0x136, 0xA), (0x138, 0xA)]);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    scatter(fx, g, true);
    fx.texture4(sp + 0xD0, 0xF);
    // Two point sets (+0x1F50 + 0x2F0 k, 60 points each), never drawn.
    for s in 0..2u32 {
        let g = D + 0x2F0 * s + 0x1F50;
        let tex = fx.mem.read_u32(sp + 0xD0);
        fx.mem.write_u32(g, tex);
        fx.mem.write_u32(g + 4, 1);
        for (k, x) in [(0x10u32, 0xB0), (0x12, 0x40), (0x14, 0x20), (0x16, 0x20)] {
            fx.mem.set16(g + k, x);
        }
        fx.mem.write_u32(g + 0xC, 0x3F00_0000);
        for k in 0..0x3Cu32 {
            let p = g + 12 * k;
            if bit(fx.mem.read_u32(g + 4), 0x1D) {
                let r = fx.rand();
                let (_, h) = fx.div(r, 100);
                fx.mem.setf(p + 0x20, ee::from_i32(h - 50));
                let r = fx.rand();
                let (_, h) = fx.div(r, 50);
                fx.mem.setf(p + 0x28, ee::from_i32(h));
                let r = fx.rand();
                let (_, h) = fx.div(r, 100);
                fx.mem.setf(p + 0x24, ee::from_i32(h - 50));
            } else {
                let r = fx.rand();
                let (_, h) = fx.div(r, 50);
                fx.mem.setf(p + 0x20, ee::from_i32(h - 25));
                let r = fx.rand();
                let (_, h) = fx.div(r, 25);
                fx.mem.setf(p + 0x24, ee::from_i32(h));
                let r = fx.rand();
                let (_, h) = fx.div(r, 50);
                fx.mem.setf(p + 0x28, ee::from_i32(h - 25));
            }
        }
        for (k, v) in [(0xAu32, 0x80u8), (9, 0x20), (8, 0x20), (0xB, 0x80)] {
            fx.mem.write_u8(g + k, v);
        }
    }
    fx.texture5(sp + 0xD0, 0);
    // Three sprite groups (+0x550 + 0x60 k, particles +0x670 + 0x800 k),
    // spawned, never drawn.
    for s in 0..3u32 {
        let g = D + 0x60 * s + 0x550;
        let parts = D + 0x800 * s + 0x670;
        let tex = fx.mem.read_u32(sp + 0xD0);
        fx.mem.write_u32(g, tex);
        fx.mem.write_u32(g + 4, 0x5000_0003);
        for k in [0xAu32, 9, 8, 0xB] {
            fx.mem.write_u8(g + k, 0x80);
        }
        fx.mem.write_u32(g + 0xC, 0x4000_0000);
        for k in [0x20u32, 0x24, 0x28] {
            fx.mem.write_u32(g + k, 0);
        }
        for (k, x) in [(0x30u32, 0), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)] {
            fx.mem.set16(g + k, x);
        }
        fx.mem.write_u32(g + 0x40, parts);
        fx.mem.write_u32(g + 0x44, 0);
        fx.mem.write_u32(g + 0x48, 0);
        fx.mem.write_u32(g + 0x4C, 0x4040_0000);
        fx.mem.set16(g + 0x5A, 0x80);
        fx.mem.set16(g + 0x5C, 8);
        fx.mem.set16(g + 0x5E, 0);
        fx.sprites_spawn(g, 2.0);
        let mut k = 0;
        while k < fx.mem.i16(g + 0x5A) {
            let p = parts + 16 * k as u32;
            let r = fx.rand();
            let (_, h) = fx.div(r, 12);
            fx.mem.set16(p + 0xE, -h);
            fx.mem.write_u8(p + 0xC, 0);
            fx.mem.write_u8(p + 0xD, 0);
            let y = ee::sub(fx.mem.f(p + 4), 10.0);
            fx.mem.setf(p + 4, y);
            k += 1;
        }
    }
    0
}

/// Transform `x` (its translation +0x20 = the node, rotation +0x10) kept
/// on node slot `at` looking along the slots `from` -> `to`.
fn aim(fx: &mut Fx, sp: u32, x: u32, at: u32, to: u32, a: u32, b: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, x + 0x20);
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, sp + a);
    let h = fx.self_handle();
    let s = fx.mem.i32(D + to);
    fx.node_position(h, s, sp + b);
    fx.look_angles(sp + a, sp + b, x + 0x10);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x90);
    if fx.mem.i32(D + 0x2534) < 0x7A {
        aim(fx, sp, D + 0x400, 0x2544, 0x2548, 0x80, 0x90);
    }
    if fx.mem.i32(D + 0x2534) < 0x73 {
        aim(fx, sp, D + 0x470, 0x254C, 0x2550, 0xA0, 0xB0);
    }
    if fx.mem.i32(D + 0x2534) < 0x38 {
        aim(fx, sp, D + 0x4E0, 0x253C, 0x2540, 0x60, 0x70);
    }
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x253C);
    fx.node_position(h, s, D + 0x3A0);
    for k in 0..2u32 {
        for c in 0..4u32 {
            let w = fx.mem.read_u32(D + 0x70 * k + 0x420 + 4 * c);
            fx.mem.write_u32(D + 0x70 * k + 0x1E90 + 4 * c, w);
        }
    }
    fx.xform_world(D);
    fx.xform_billboard(D + 0x70);
    for k in 0..2u32 {
        fx.xform_billboard(D + 0x1E70 + 0x70 * k);
    }
    for k in 0..3u32 {
        fx.xform_world(D + 0x400 + 0x70 * k);
    }
    fx.xform_billboard(D + 0x380);
    if fx.mem.i32(D + 0x2534) >= 0x74 {
        for k in [0xC8u32, 0xC4, 0xC0] {
            fx.mem.write_u32(sp + k, 0);
        }
        fx.particle_quads(D + 0x70, D + 0xE0, sp + 0xC0);
        fx.step_down(D + 0xEB, 0, 8);
    }
    if fx.mem.i32(D + 0x2534) == 0x7A {
        fx.mem.write_u8(D + 0xEB, 0x80);
    }
    if fx.mem.i32(D + 0x2534) >= 0x7B {
        for k in [0xC8u32, 0xC4, 0xC0] {
            fx.mem.write_u32(sp + k, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0x70, D + 0x230, sp + 0xC0);
        fx.step_down(D + 0x23B, 0, 4);
    }
    let n = fx.mem.i32(D + 0x2534) + 1;
    fx.mem.set32(D + 0x2534, n);
    let left = fx.mem.i32(D + 0x2530) - 1;
    fx.mem.set32(D + 0x2530, left);
    if n == 0x82 {
        2
    } else if n == 0x69 {
        1
    } else if (0x73..0x82).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
