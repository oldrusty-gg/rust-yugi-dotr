//! BMODULE b634 (card 201), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03100, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): b043's family with three ribbons. From update 69 three quad
//! bursts (+0x3180 + 0x460 k, two updates apart) at the attacker's nodes
//! 0x73 / 0x6F / 0x71 grow to 2; the frame quad (colour +0x3ED0, alpha
//! 0x60 fading) on updates 83..117; from update 122 a disc (+0x3020) at
//! node 0x4D and three ribbons (+0x70 + 0x30 k, points at *(+0x700 + 4 k))
//! from that node toward three points beside the target (1 / 16 of the
//! way per update ...); at the target from update 138 a disc (+0x1970),
//! five widening rings (+0x1BB0 + 0x400 k) and a spark burst (+0x780,
//! `blib::trail::sparks`). Returns 1 on update 128, 6 on 138..153, 2 on
//! 154, 8 after 169.
//!
//! Library: `colours4` (0x1E00080), the frame quad (0x1E00120,
//! [`crate::framegs`]), `spawn::scatter` (0x1E00250), `trail::sparks`
//! (0x1E00410) [`crate::blib`], `ring_setup` / `ring_draw` (0x1E01170 /
//! 0x1E01430), `disc` (0x1E01920), `quads_setup` / `quads_draw` (0x1E02990
//! / 0x1E02AF0) [`crate::fxlib`], the ribbon (0x1E01E80 / 0x1E01FD0,
//! [`crate::ribbon`]).
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad corner template (rodata, 0x1E02990's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_3C80;
const TENTH: u32 = 0x3DCC_CCCD;
const FIFTH: u32 = 0x3E4C_CCCD;

/// The entry 0x1E03100.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

fn bytes(fx: &mut Fx, at: u32, xs: &[(u32, u8)]) {
    for &(k, x) in xs {
        fx.mem.write_u8(at + k, x);
    }
}

fn disc_record(fx: &mut Fx, at: u32, flags: u32, radius: u32) {
    fx.mem.write_u32(at, flags);
    bytes(fx, at, &[(6, 0x70), (5, 0x70), (4, 0xC0), (7, 0xFF)]);
    fx.mem.write_u32(at + 8, radius);
    fx.mem.set16(at + 0xC, 0x24);
    fx.mem.set16(at + 0xE, 0);
}

/// Three random 16-bit angles at x + 0x10.
fn spin(fx: &mut Fx, x: u32) {
    for c in 0..3u32 {
        let r = fx.rand();
        fx.mem.set32(x + 0x10 + 4 * c, r & 0xFFFF);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3F00);
    fx.xform_init(D);
    for k in 0..3u32 {
        fx.xform_init(D + 0x17B0 + 0x70 * k);
        fx.xform_init(D + 0x3030 + 0x70 * k);
    }
    fx.xform_init(D + 0x2FB0);
    fx.xform_init(D + 0x710);
    for k in 0..5u32 {
        fx.xform_init(D + 0x1980 + 0x70 * k);
    }
    fx.xform_init(D + 0x1900);
    for (at, node, clear) in [(0x3EE0u32, 0x4D, 1), (0x3EE4, 0x73, 0), (0x3EE8, 0x6F, 0), (0x3EEC, 0x71, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x3ED4, 0xAA);
    fx.mem.set32(D + 0x3ED8, 1);
    fx.mem.set32(D + 0x3EDC, param);
    for k in 0..3u32 {
        fx.mem.set32(D + 0x3EF0 + 4 * k, 0x10);
    }
    bytes(fx, D, &[(0x3ED2, 0x80), (0x3ED1, 0x80), (0x3ED0, 0x80), (0x3ED3, 0x60)]);
    disc_record(fx, D + 0x1970, 0x8_0000, 0x41F0_0000);
    disc_record(fx, D + 0x3020, 0x1008_0000, 0x4040_0000);
    fx.texture5(sp + 0xB0, 0);
    // The ribbons (+0x70 + 0x30 k): cell (0, 0, 16, 32), width 30.
    for k in 0..3u32 {
        let g = D + 0x70 + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0xB0);
        fx.mem.write_u32(g, tex);
        fx.mem.write_u32(g + 8, 0x8003);
        bytes(fx, g, &[(6, 0x80), (5, 0x80), (4, 0x80), (7, 0x80)]);
        halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x10), (0x16, 0x20), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)]);
        fx.mem.write_u32(g + 0xC, 0x41F0_0000);
        halves(fx, g, &[(0x22, 0x20), (0x24, 0), (0x26, 1), (0x28, 0x10), (0x2A, 0), (0x2C, 1)]);
    }
    for k in 0..3u32 {
        fx.mem.write_u32(D + 0x700 + 4 * k, D + 0x100 + 0x200 * k);
    }
    fx.texture4(sp + 0xB0, 0xF);
    let tex = fx.mem.read_u32(sp + 0xB0);
    // The sparks (+0x780): 256 at +0x7B0, width 0.2, length 5.
    let g = D + 0x780;
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 3);
    bytes(fx, g, &[(0xA, 0x70), (9, 0x70), (8, 0xC0), (0xB, 0x80)]);
    fx.mem.write_u32(g + 0x20, FIFTH);
    fx.mem.write_u32(g + 0x24, 0x40A0_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x60), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x60), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, D + 0x7B0);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    // The rings (+0x1BB0 + 0x400 k): radii 11.7 / 12.3.
    for k in 0..5u32 {
        let g = D + 0x1BB0 + 0x400 * k;
        fx.mem.write_u32(g + 4, 0);
        halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
        bytes(fx, g, &[(0xA, 0x70), (9, 0x70), (8, 0xC0), (0xB, 0x80)]);
        let tex = fx.mem.read_u32(sp + 0xB0);
        fx.ring_setup(g, tex, f32::from_bits(0x413B_3333), f32::from_bits(0x4144_CCCD), 0.0, 0.0);
    }
    // The quad bursts (+0x3180 + 0x460 k), size 0.05.
    for k in 0..3u32 {
        let g = D + 0x3180 + 0x460 * k;
        fx.mem.write_u32(g + 4, 0x8_0001);
        halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
        bytes(fx, g, &[(0xA, 0x70), (9, 0x70), (8, 0xC0), (0xB, 0x20)]);
        let tex = fx.mem.read_u32(sp + 0xB0);
        fx.quads_setup(g, tex, f32::from_bits(0x3D4C_CCCD), QUAD_TEMPLATE);
    }
    for (x, s) in [(0x1980u32, 0u32), (0x19F0, FIFTH), (0x1A60, 0x3ECC_CCCD), (0x1AD0, 0x3F19_999A), (0x1B40, 0x3F4C_CCCD)] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, s);
        }
    }
    for k in 0..5u32 {
        spin(fx, D + 0x1980 + 0x70 * k);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x3ED8);
    let node = |fx: &mut Fx, slot: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + slot);
        fx.node_position(h, s, out);
    };
    let target = |fx: &mut Fx, out: u32| {
        let t = fx.target_handle();
        fx.unit_position(t, out);
    };
    if n(fx) < 0x7B {
        node(fx, 0x3EE0, D + 0x2FD0);
    }
    for k in 0..3u32 {
        node(fx, 0x3EE4 + 4 * k, D + 0x3050 + 0x70 * k);
    }
    for k in 0..3u32 {
        let p = D + 0x3EA0 + 0x10 * k;
        target(fx, p);
        let y = ee::sub(fx.mem.f(p + 4), ee::mul(10.0, ee::from_i32(k as i32 - 1)));
        fx.mem.setf(p + 4, y);
    }
    target(fx, D + 0x1920);
    target(fx, D + 0x730);
    for k in 0..5u32 {
        target(fx, D + 0x19A0 + 0x70 * k);
    }
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x90);
    target(fx, sp + 0xA0);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x720);
    for k in 0..3u32 {
        let p = D + 0x17D0 + 0x70 * k;
        if n(fx) < 0x7B {
            node(fx, 0x3EE0, p);
        }
        if n(fx) >= 0x7A {
            let pts = fx.mem.read_u32(D + 0x700 + 4 * k);
            fx.ribbon_push(D + 0x70 + 0x30 * k, pts, p);
            let c = fx.mem.i32(D + 0x3EF0 + 4 * k);
            fx.approach_by(p, D + 0x3EA0 + 0x10 * k, c);
            let c = fx.mem.i32(D + 0x3EF0 + 4 * k);
            if c != 0 {
                fx.mem.set32(D + 0x3EF0 + 4 * k, c - 1);
            }
        }
    }
    // s0 = the OT, s1 = D, s2 / s3 the loops'.
    fx.xform_world_saving(D, 0x01E0_37D4, [Some(OT), Some(D), Some(D + 0x150), Some(D + 0xC)]);
    for k in 0..3u32 {
        let regs = [Some(OT), Some(D), Some(D + 0x70 * k), Some(k)];
        fx.xform_billboard_saving(D + 0x17B0 + 0x70 * k, 0x01E0_37E4, regs);
        fx.xform_world_saving(D + 0x3030 + 0x70 * k, 0x01E0_37EC, regs);
    }
    let regs = [Some(OT), Some(D), Some(D + 0x150), Some(3)];
    fx.xform_billboard_saving(D + 0x2FB0, 0x01E0_3804, regs);
    fx.xform_world_saving(D + 0x710, 0x01E0_380C, regs);
    for k in 0..5u32 {
        let regs = [Some(OT), Some(D), Some(D + 0x70 * k), Some(k)];
        fx.xform_world_saving(D + 0x1980 + 0x70 * k, 0x01E0_381C, regs);
    }
    fx.xform_billboard_saving(D + 0x1900, 0x01E0_383C, [Some(OT), Some(D), Some(D + 0x230), Some(5)]);
    if (0x53..0x76).contains(&n(fx)) {
        fx.frame_quad(D + 0x3ED0);
        if n(fx) >= 0x64 {
            fx.step_down(D + 0x3ED3, 0, 6);
        }
    }
    if n(fx) >= 0x45 {
        for k in 0..3u32 {
            if n(fx) - (0x45 + 2 * k as i32) < 0 {
                continue;
            }
            let (g, x) = (D + 0x3180 + 0x460 * k, D + 0x3030 + 0x70 * k);
            fx.quads_draw(g, x);
            fx.step_down(g + 0xB, 0, 4);
            fx.approach(x, 2.0, f32::from_bits(FIFTH));
            for q in 0..64u32 {
                for c in 0..3u32 {
                    let a = g + 0x60 + 16 * q + 4 * c;
                    let v = fx.mem.i32(a).wrapping_add(0x80);
                    fx.mem.set32(a, v);
                }
            }
        }
    }
    if n(fx) >= 0x7A {
        fx.disc(D + 0x3020, D + 0x2FB0);
        fx.step_down(D + 0x3027, 0, 0x20);
    }
    if n(fx) >= 0x8A {
        fx.disc(D + 0x1970, D + 0x1900);
        fx.step_down(D + 0x1977, 0, 0x10);
        for k in 0..5u32 {
            let (g, x) = (D + 0x1BB0 + 0x400 * k, D + 0x1980 + 0x70 * k);
            fx.ring_draw(g, x);
            fx.step_down(g + 0xB, 0, 8);
            if fx.approach(x, 2.0, f32::from_bits(TENTH)) != 0 {
                continue;
            }
            for c in [8u32, 4, 0] {
                fx.mem.write_u32(x + c, TENTH);
            }
            spin(fx, x);
        }
        blib::trail::sparks(fx, OT, D + 0x780, D + 0x710);
        fx.step_down(D + 0x78B, 0, 8);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x7AC) {
            let p = D + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0x7BE) < 0 {
                continue;
            }
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 0x7B0 + 4 * c), 1.5);
                fx.mem.setf(p + 0x7B0 + 4 * c, x);
            }
            if fx.mem.i16(p + 0x7BE) < 9 {
                continue;
            }
            let r = fx.rand();
            let (_, h) = fx.div(r, 0xA);
            let len = ee::add(1.0, ee::mul(f32::from_bits(TENTH), ee::from_i32(h)));
            fx.random_dir(sp + 0x80, len);
            for c in [1u32, 2, 0] {
                let w = fx.mem.read_u32(sp + 0x80 + 4 * c);
                fx.mem.write_u32(p + 0x7B0 + 4 * c, w);
            }
            let r = fx.rand();
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0x7BE, h.wrapping_neg());
        }
    }
    if n(fx) >= 0x7B {
        for k in 0..3u32 {
            let pts = fx.mem.read_u32(D + 0x700 + 4 * k);
            fx.ribbon_draw(D + 0x70 + 0x30 * k, pts, D);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x3ED8, m);
    let left = fx.mem.i32(D + 0x3ED4) - 1;
    fx.mem.set32(D + 0x3ED4, left);
    if m == 0x9A {
        2
    } else if m == 0x80 {
        1
    } else if (0x8A..0x9A).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
