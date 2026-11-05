//! BMODULE b223 (card 178), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04170, data area 0x1ED0000) [F] (helper C9, 31
//! Oct 2026): b752's program shape with four planks in place of the
//! afterimage. Every update four textured planks (+0x70 / +0xC0 / +0x110
//! / +0x160, `plank::draw`, local 0x1E03160) between pairs of the
//! attacker's nodes (0x56 / 0x54, 0x6C / 0x68, 0x5D / 0x59, 0x64 / 0x63),
//! their half widths random each update; a frame quad before update 0x6E;
//! from 0xD2 a particle cloud (+0x780); on 0x10E..0x14E four waves
//! (+0x17C0 + 0x30 k, `wave::draw`), two rings (+0x940 / +0xD40) swelling
//! and two more particle clouds (+0x1520 / +0x1670); from 0x14F a square
//! cloud (+0x11B0, `pointquad::squares`), point quads (+0x720) and a quad
//! group (+0x220) growing. Returns 1 on update 0x104, 6 on 0x10E..0x14D,
//! 2 on 0x14E, 8 after 0x172.
//!
//! Library at its own addresses: `colours4` (0x1E00080), `pointquad`
//! setup / draw / squares (0x1E00120 / 0x1E00460 / 0x1E01040),
//! `ring_setup` / `ring_draw` (0x1E01620 / 0x1E018E0), `particle_quads`
//! (0x1E01DD0), `wave::draw` (0x1E02960), `plank::draw` (0x1E03160), the
//! frame quad (0x1E038E0), `quads_setup` / `quads_draw` (0x1E03A00 /
//! 0x1E03B60).
use crate::b752::{bw, cloud8, colour, hw, rand_f};
use crate::blib::{plank, pointquad, wave};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SETUP: u32 = 0x01E0_0120;
const TEMPLATE: u32 = 0x01E0_5280;
const LEFT: u32 = D + 0x1880;
const N: u32 = D + 0x1884;
const NODES: u32 = D + 0x188C;
const PLANKS: [u32; 4] = [0x70, 0xC0, 0x110, 0x160];

/// The entry 0x1E04170.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x70);
    ret
}

fn node(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(NODES + 4 * k);
    fx.node_position(h, s, out);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x19B0);
    for x in [0u32, 0x1B0, 0x1140, 0x14B0, 0x8D0] {
        fx.xform_init(D + x);
    }
    for (k, n) in [0x56, 0x54, 0x6C, 0x68, 0x5D, 0x59, 0x64, 0x63].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set32(NODES + 4 * k as u32, r);
    }
    fx.mem.set32(LEFT, 0x172);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x1888, param);
    for o in [0x14A0u32, 0x14A4, 0x14A8] {
        fx.mem.write_u32(D + o, 0x3F83_D70A);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_42C8);
        fx.mem.write_u8(D + 0x18AC + k, (r >> 4) as u8);
    }
    fx.texture4(sp + 0x60, 0xF);
    // The square cloud (+0x11B0, 60 points at +0x11D0 + 12 k).
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.mem.write_u32(D + 0x11B0, tex);
    fx.mem.write_u32(D + 0x11BC, 0x3F19_999A);
    for k in 0..0x3Cu32 {
        let p = D + 0xC * k;
        if bit(fx.mem.read_u32(D + 0x11B4), 0x1D) {
            let x = rand_f(fx, 0x01E0_4328, 0x64, 0x32);
            fx.mem.setf(p + 0x11D0, x);
            let z = rand_f(fx, 0x01E0_4354, 0x32, 0);
            fx.mem.setf(p + 0x11D8, z);
            let y = rand_f(fx, 0x01E0_437C, 0x64, 0x32);
            fx.mem.setf(p + 0x11D4, y);
        } else {
            let x = rand_f(fx, 0x01E0_43B0, 0x32, 0x19);
            fx.mem.setf(p + 0x11D0, x);
            let y = rand_f(fx, 0x01E0_43DC, 0x19, 0);
            fx.mem.setf(p + 0x11D4, y);
            let z = rand_f(fx, 0x01E0_4404, 0x32, 0x19);
            fx.mem.setf(p + 0x11D8, z);
        }
    }
    fx.mem.write_u32(D + 0x11B4, 1);
    hw(fx, &[(0x11C0, 0xB0), (0x11C2, 0x40), (0x11C4, 0x20), (0x11C6, 0x20)]);
    colour(fx, 0x11B8, [0x40, 0, 0, 0x80]);
    // Four waves (+0x17C0 + 0x30 k).
    for k in 0..4u32 {
        let g = 0x17C0 + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0x60);
        fx.mem.write_u32(D + g, tex);
        fx.mem.write_u32(D + g + 0xC, D + 0x18AC);
        fx.mem.write_u32(D + g + 4, 0x2200_0000);
        hw(fx, &[(g + 0x10, 0xB0), (g + 0x12, 0x40), (g + 0x14, 0x20), (g + 0x16, 0x20)]);
        colour(fx, g + 8, [0x90, 0x70, 0x40, 0x80]);
        fx.mem.write_u32(D + g + 0x20, 0x3E99_999A);
        fx.mem.write_u32(D + g + 0x24, 0x3D4C_CCCD);
        hw(fx, &[(g + 0x2A, -2 * k as i32), (g + 0x28, 0x100)]);
        fx.mem.write_u8(D + g + 0x2C, (k + 7) as u8);
    }
    // The quad group (+0x220).
    fx.mem.write_u32(D + 0x224, 0x8_0001);
    hw(fx, &[(0x230, 0x8E), (0x232, 0), (0x234, 0x22), (0x236, 0x40)]);
    colour(fx, 0x228, [0x40, 0, 0, 0x20]);
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.quads_setup(D + 0x220, tex, f32::from_bits(0x3FCC_CCCD), TEMPLATE);
    // Two rings (+0x940, +0xD40).
    for (g, a, outer) in [(0x940u32, 0x20u8, 0x4133_3333u32), (0xD40, 0x30, 0x414C_CCCD)] {
        fx.mem.write_u32(D + g + 4, 0x2000_0000);
        hw(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x40)]);
        colour(fx, g + 8, [0x90, 0x50, 0x20, a]);
        let tex = fx.mem.read_u32(sp + 0x60);
        fx.ring_setup(D + g, tex, 12.0, f32::from_bits(outer), 0.0, 0.0);
    }
    // The point quads (+0x720, points +0x680).
    fx.mem.write_u32(D + 0x758, 0x8_0001);
    hw(fx, &[(0x720, 0x80), (0x722, 0), (0x724, 0x20), (0x726, 0x20), (0x728, 0xB0), (0x72A, 0x20), (0x72C, 0x100), (0x72E, 0x100)]);
    fx.mem.write_u32(D + 0x750, 0x3F80_0000);
    colour(fx, 0x754, [0x40, 0, 0, 0x60]);
    hw(fx, &[(0x768, 0), (0x76A, 0), (0x76C, 0), (0x76E, 1), (0x770, 0), (0x772, 0x3E7)]);
    fx.mem.set32(D + 0x688, 1);
    let tex = fx.mem.read_u32(sp + 0x60);
    pointquad::setup(fx, SETUP, D + 0x720, D + 0x680, tex, 20.0);
    // The particle cloud (+0x780).
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.mem.write_u32(D + 0x780, tex);
    fx.mem.write_u32(D + 0x784, 0x8_0481);
    for o in [0x790u32, 0x794, 0x798, 0x79C] {
        fx.mem.write_u32(D + o, 0);
    }
    hw(fx, &[(0x7A8, 0xF0), (0x7AA, 0), (0x7A4, 0x10), (0x7A6, 0x10), (0x7AC, 0x100), (0x7AE, 0x100)]);
    fx.mem.write_u32(D + 0x78C, 0x4100_0000);
    hw(fx, &[(0x8B0, 8), (0x8BA, 3), (0x8BC, 1), (0x8C0, 8), (0x8B6, 0xA), (0x8B8, 0xA)]);
    bw(fx, &[(0x78A, 0x80), (0x789, 0x80), (0x788, 0x80), (0x78B, 0x60)]);
    for k in 0..0x10i32 {
        let p = D + 0x10 * k as u32;
        let x = ee::add(fx.mem.f(D + 0x790), rand_f(fx, 0x01E0_4768, 0xA, 5));
        fx.mem.setf(p + 0x7B0, x);
        let r = fx.rand_from(D, 0x01E0_479C);
        let y = ee::add(fx.mem.f(D + 0x794), ee::from_i32(r.wrapping_rem(8) - 2));
        fx.mem.setf(p + 0x7B4, y);
        let z = fx.mem.read_u32(D + 0x798);
        fx.mem.write_u32(p + 0x7B8, z);
        let a = fx.mem.i16(D + 0x8BA).wrapping_neg();
        let f = fx.mult(a, k);
        fx.mem.set16(p + 0x7BE, f);
        fx.mem.set16(p + 0x7BC, 0);
        let c = fx.mem.i16(D + 0x8C0) - 1;
        fx.mem.set16(D + 0x8BE, c);
    }
    // Two late particle clouds (+0x1520, +0x1670).
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.mem.write_u32(D + 0x1520, tex);
    fx.mem.write_u32(D + 0x1524, 5);
    for o in [0x1530u32, 0x1534, 0x1538] {
        fx.mem.write_u32(D + o, 0);
    }
    hw(fx, &[(0x1540, 0xF0), (0x1542, 0), (0x1544, 0x10), (0x1546, 0x10), (0x1548, 0xF0), (0x154A, 0), (0x154C, 0x100), (0x154E, 0x100)]);
    fx.mem.write_u32(D + 0x152C, 0x40C0_0000);
    hw(fx, &[(0x1650, 0x10), (0x165A, 1), (0x165C, 0), (0x1660, 8), (0x1656, 0xA), (0x1658, 0xA)]);
    colour(fx, 0x1528, [0x40, 0, 0, 0x40]);
    cloud8(fx, D + 0x1520, [0x01E0_489C, 0x01E0_48C8]);
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.mem.write_u32(D + 0x1670, tex);
    fx.mem.write_u32(D + 0x1674, 7);
    for o in [0x1680u32, 0x1684, 0x1688] {
        fx.mem.write_u32(D + o, 0);
    }
    hw(fx, &[(0x1690, 0x80), (0x1692, 0xC0), (0x1694, 0x20), (0x1696, 0x20), (0x1698, 0x80), (0x169A, 0xC0), (0x169C, 0x80), (0x169E, 0x100)]);
    fx.mem.write_u32(D + 0x167C, 0x4100_0000);
    hw(fx, &[(0x17A0, 0x10), (0x17AA, 1), (0x17AC, 0), (0x17B0, 8), (0x17A6, 0xA), (0x17A8, 0xA)]);
    colour(fx, 0x1678, [0x80, 0x20, 0x20, 0x40]);
    cloud8(fx, D + 0x1670, [0x01E0_49C0, 0x01E0_49EC]);
    // The planks (+0x70 + 0x50 k): integer UVs, half width 5.
    for (g, c) in PLANKS.into_iter().zip([[0x80u8, 0x60, 0x40, 0xFF], [0x80, 0x40, 0x40, 0xFF], [0x80, 0x80, 0x80, 0xFF], [0x40, 0x40, 0x80, 0xFF]]) {
        let tex = fx.mem.read_u32(sp + 0x60);
        fx.mem.write_u32(D + g, tex);
        fx.mem.write_u32(D + g + 4, 1);
        hw(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x40)]);
        colour(fx, g + 8, c);
        fx.mem.write_u32(D + g + 0xC, 0x40A0_0000);
    }
    for x in [0x1B0u32, 0x1140, 0x8D0] {
        for o in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + o, 0x3DCC_CCCD);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    if n(fx) < 0x10E {
        node(fx, 0, D + 0x14D0);
        node(fx, 0, D + 0x8F0);
    }
    if n(fx) < 0x14E {
        node(fx, 6, D + 0x1D0);
        node(fx, 6, D + 0x1160);
    }
    for (k, g) in PLANKS.into_iter().enumerate() {
        node(fx, 2 * k as u32, D + g + 0x20);
        node(fx, 2 * k as u32 + 1, D + g + 0x30);
    }
    // s0 = D, s1 = the OT, s2 the parameter (-1, 64-bit), s3 the runner's 0.
    let regs = [Some(D), Some(OT), Some(0), Some(0)];
    fx.xform_world_saving(D, 0x01E0_4CA0, regs);
    fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
    for (x, ra) in [(0x1B0u32, 0x01E0_4CA8u32), (0x1140, 0x01E0_4CB0), (0x14B0, 0x01E0_4CB8), (0x8D0, 0x01E0_4CC0)] {
        fx.xform_billboard_saving(D + x, ra, regs);
        fx.saved(sp - 0xF0, &[(0x40, u64::MAX)]);
    }
    // The planks' half widths: 3..5, 1..3, 1..2, 3..5.
    for (g, ra, m, add) in [(0x70u32, 0x01E0_4CC8u32, 3, 3), (0xC0, 0x01E0_4CF4, 3, 1), (0x110, 0x01E0_4D20, 2, 1), (0x160, 0x01E0_4D4C, 3, 3)] {
        let r = fx.rand_from(D, ra);
        let w = r.wrapping_rem(m) + add;
        fx.mem.setf(D + g + 0xC, ee::from_i32(w));
    }
    for g in PLANKS {
        plank::draw(fx, OT, D + g, D);
    }
    if n(fx) < 0x6E {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) >= 0xD2 {
        for o in [0x58u32, 0x54, 0x50] {
            fx.mem.write_u32(sp + o, 0x3F82_8F5C);
        }
        fx.particle_quads(D + 0x14B0, D + 0x780, sp + 0x50);
        fx.step_down(D + 0x78B, 0, 4);
    }
    if (0x10E..0x14F).contains(&n(fx)) {
        for k in 0..4u32 {
            let g = D + 0x17C0 + 0x30 * k;
            if fx.mem.i16(g + 0x2A) >= 0xA {
                fx.mem.set16(g + 0x2A, 0);
                fx.mem.write_u8(g + 0xB, 0x80);
            }
            wave::draw(fx, OT, g, D + 0x14B0);
            fx.step_down(g + 0xB, 0, 0x10);
        }
        fx.ring_draw(D + 0x940, D + 0x8D0);
        fx.ring_draw(D + 0xD40, D + 0x8D0);
        fx.step_down(D + 0x94B, 0, 2);
        fx.step_down(D + 0xD4B, 0, 3);
        if fx.approach(D + 0x8D0, 2.0, f32::from_bits(0x3E4C_CCCD)) == 0 {
            for o in [0x8D8u32, 0x8D4, 0x8D0] {
                fx.mem.write_u32(D + o, 0x3DCC_CCCD);
            }
            fx.mem.write_u8(D + 0x94B, 0x20);
            fx.mem.write_u8(D + 0xD4B, 0x30);
        }
        for o in [0x58u32, 0x54, 0x50] {
            fx.mem.write_u32(sp + o, 0);
        }
        fx.particle_quads(D + 0x14B0, D + 0x1520, sp + 0x50);
        fx.particle_quads(D + 0x14B0, D + 0x1670, sp + 0x50);
        if n(fx) >= 0x14B {
            fx.step_down(D + 0x152B, 0, 0x10);
            fx.step_down(D + 0x167B, 0, 0x10);
        }
    }
    if n(fx) >= 0x14F {
        pointquad::squares(fx, OT, D + 0x11B0, D + 0x1140, D + 0x14A0);
        fx.step_down(D + 0x11BB, 0, 0xA);
        pointquad::draw(fx, OT, D + 0x720, D + 0x680, D + 0x1B0);
        fx.quads_draw(D + 0x220, D + 0x1B0);
        if ee_lt(fx.mem.f(D + 0x1140), 1.0) {
            let s = ee::add(fx.mem.f(D + 0x1148), 0.5);
            for o in [0x1148u32, 0x1144, 0x1140] {
                fx.mem.setf(D + o, s);
            }
        }
        if ee_lt(fx.mem.f(D + 0x1B0), 1.0) {
            let d = f32::from_bits(0x3ECC_CCCD);
            for o in [0x1B0u32, 0x1B4, 0x1B8] {
                let s = ee::add(fx.mem.f(D + o), d);
                fx.mem.setf(D + o, s);
            }
        } else {
            let d = f32::from_bits(0x3BA3_D70A);
            for o in [0x1B0u32, 0x1B4, 0x1B8] {
                let s = ee::add(fx.mem.f(D + o), d);
                fx.mem.setf(D + o, s);
            }
            fx.step_down(D + 0x22B, 0, 6);
            fx.step_down(D + 0x757, 0, 0xC);
        }
        for k in 0..0x40u32 {
            for c in 0..3u32 {
                let a = D + 0x280 + 0x10 * k + 4 * c;
                let w = fx.mem.read_u32(a).wrapping_add(0x80);
                fx.mem.write_u32(a, w);
            }
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0x14E {
        2
    } else if m == 0x104 {
        1
    } else if (0x10E..0x14E).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
