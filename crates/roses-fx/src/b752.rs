//! BMODULE b752 (card 527), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E041C0, data area 0x1ED0000; compiled without
//! delay-slot scheduling) [F]: b036's program
//! with an afterimage in place of the swaths. Before update 0x31 the
//! attacker's afterimage (+0x17B0, `afterimage::strips` at its point
//! +0x1760, flickering grey) with its tint (0x1F75F0, +0x18E8); from 0x31
//! a frame quad, the object op 0x1F83B0 on 0x28..0x82; from 0x35 a
//! full-window tint (+0x18EC) and a particle cloud (+0x640); from 0x3B a
//! square cloud (+0x1070, `pointquad::squares`), point quads (+0x5E0) and
//! a quad group (+0xE0) growing at the target, two more particle clouds
//! (+0x13E0 / +0x1530) and, before 0x78, four waves (+0x1680 + 0x30 k,
//! `wave::draw`) and two rings (+0x800 / +0xC00) swelling. The stage dims
//! (0x1F7F10) until 0x83. Returns 1 on update 0x2B, 6 on 0x35..0x81, 2 on
//! 0x82, 8 after 0x96.
//!
//! Library at its own addresses: `colours4` (0x1E00080), the tint
//! (0x1E00120), `afterimage::strips` (0x1E00280), `pointquad` setup / draw
//! / squares (0x1E008F0 / 0x1E00C30 / 0x1E01810), `ring_setup` /
//! `ring_draw` (0x1E01DF0 / 0x1E020B0), `particle_quads` (0x1E025A0),
//! `wave::draw` (0x1E03130), the frame quad (0x1E03930), `quads_setup` /
//! `quads_draw` (0x1E03A50 / 0x1E03BB0).
use crate::b101::unit_tint;
use crate::blib::{afterimage, pointquad, wave};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SETUP: u32 = 0x01E0_08F0;
const TEMPLATE: u32 = 0x01E0_5400;
const LEFT: u32 = D + 0x17C8;
const N: u32 = D + 0x17CC;
const LIGHT: u32 = D + 0x18F0;

/// The entry 0x1E041C0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x70);
    ret
}

pub(crate) fn hw(fx: &mut Fx, at: &[(u32, i32)]) {
    for &(o, v) in at {
        fx.mem.set16(D + o, v);
    }
}

pub(crate) fn bw(fx: &mut Fx, at: &[(u32, u8)]) {
    for &(o, v) in at {
        fx.mem.write_u8(D + o, v);
    }
}

/// Colour bytes (+2, +1, +0, +3) at `at`.
pub(crate) fn colour(fx: &mut Fx, at: u32, c: [u8; 4]) {
    bw(fx, &[(at + 2, c[0]), (at + 1, c[1]), (at, c[2]), (at + 3, c[3])]);
}

/// rand % m - sub as a float (rand at `ra`; its frame: s0 = D).
pub(crate) fn rand_f(fx: &mut Fx, ra: u32, m: i32, sub: i32) -> f32 {
    let r = fx.rand_from(D, ra);
    let (_, h) = fx.div(r, m);
    ee::from_i32(h - sub)
}

/// A particle cloud's 8 particles (`g` + 0x30 + 0x10 k; count `g` +
/// 0x130): x / y = rand % 0x14 - 10, frame -(+0x13A) k, tick 0, +0x13E =
/// +0x140 - 1 [b036's].
pub(crate) fn cloud8(fx: &mut Fx, g: u32, ra: [u32; 2]) {
    for k in 0..8i32 {
        let p = g + 0x10 * k as u32;
        let x = rand_f(fx, ra[0], 0x14, 0xA);
        fx.mem.setf(p + 0x30, x);
        let y = rand_f(fx, ra[1], 0x14, 0xA);
        fx.mem.setf(p + 0x34, y);
        let a = fx.mem.i16(g + 0x13A).wrapping_neg();
        let f = fx.mult(a, k);
        fx.mem.set16(p + 0x3E, f);
        fx.mem.set16(p + 0x3C, 0);
        let c = fx.mem.i16(g + 0x140) - 1;
        fx.mem.set16(g + 0x13E, c);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2000);
    for x in [0u32, 0x70, 0x1000, 0x1370, 0x790] {
        fx.xform_init(D + x);
    }
    for (k, n) in [0x6B, 0x60, 0x59, 0x74, 0x70].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set32(D + 0x17D4 + 4 * k as u32, r);
    }
    fx.mem.set32(LEFT, 0x96);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x17D0, param);
    for o in [0x1360u32, 0x1364, 0x1368] {
        fx.mem.write_u32(D + o, 0x3F85_1EB8);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_431C);
        let b = fx.div(r, 0xFF).1 + 1;
        fx.mem.write_u8(D + 0x17E8 + k, b as u8);
    }
    bw(fx, &[(0x18ED, 0x80), (0x18EE, 0x80), (0x18EC, 0xB0), (0x18EF, 0xFF), (0x18F0, 0x80)]);
    // The afterimage (+0x17B0).
    fx.mem.write_u32(D + 0x17B0, 0x4_1000);
    bw(fx, &[(0x17B6, 0x60), (0x17B5, 0x80), (0x17B4, 0x80), (0x17B7, 0x80)]);
    fx.mem.write_u32(D + 0x17C0, 0);
    fx.mem.write_u32(D + 0x17BC, 0x2000);
    hw(fx, &[(0x17B8, 0)]);
    fx.mem.write_u32(D + 0x17C4, 0x40A0_0000);
    let h = fx.self_handle();
    hw(fx, &[(0x17BA, h)]);
    bw(fx, &[(0x18EA, 0x80), (0x18E9, 0x80), (0x18E8, 0x80), (0x18EB, 0)]);
    fx.texture5(sp + 0x60, 0);
    // The particle cloud (+0x640).
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.mem.write_u32(D + 0x640, tex);
    fx.mem.write_u32(D + 0x644, 0x8_0087);
    for o in [0x650u32, 0x654, 0x658, 0x65C] {
        fx.mem.write_u32(D + o, 0);
    }
    hw(fx, &[(0x668, 0), (0x66A, 0), (0x664, 0x20), (0x666, 0x20), (0x66C, 0x80), (0x66E, 0x40)]);
    fx.mem.write_u32(D + 0x64C, 0x4100_0000);
    hw(fx, &[(0x770, 8), (0x77A, 3), (0x77C, 1), (0x780, 8), (0x776, 0xA), (0x778, 0xA)]);
    bw(fx, &[(0x64A, 0x80), (0x649, 0x80), (0x648, 0x80), (0x64B, 0x60)]);
    for k in 0..0x10i32 {
        let p = D + 0x10 * k as u32;
        let x = ee::add(fx.mem.f(D + 0x650), rand_f(fx, 0x01E0_4474, 0xA, 5));
        fx.mem.setf(p + 0x670, x);
        let r = fx.rand_from(D, 0x01E0_44B4);
        let y = ee::add(fx.mem.f(D + 0x654), ee::from_i32(r.wrapping_rem(8) - 2));
        fx.mem.setf(p + 0x674, y);
        let z = fx.mem.read_u32(D + 0x658);
        fx.mem.write_u32(p + 0x678, z);
        let a = fx.mem.i16(D + 0x77A).wrapping_neg();
        let f = fx.mult(a, k);
        fx.mem.set16(p + 0x67E, f);
        fx.mem.set16(p + 0x67C, 0);
        let c = fx.mem.i16(D + 0x780) - 1;
        fx.mem.set16(D + 0x77E, c);
    }
    fx.texture4(sp + 0x60, 0xF);
    // The square cloud (+0x1070, 60 points at +0x1090 + 12 k).
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.mem.write_u32(D + 0x1070, tex);
    fx.mem.write_u32(D + 0x107C, 0x3F40_0000);
    for k in 0..0x3Cu32 {
        let p = D + 0xC * k;
        if bit(fx.mem.read_u32(D + 0x1074), 0x1D) {
            let x = rand_f(fx, 0x01E0_4570, 0x64, 0x32);
            fx.mem.setf(p + 0x1090, x);
            let z = rand_f(fx, 0x01E0_45B0, 0x32, 0);
            fx.mem.setf(p + 0x1098, z);
            let y = rand_f(fx, 0x01E0_45E0, 0x64, 0x32);
            fx.mem.setf(p + 0x1094, y);
        } else {
            let x = rand_f(fx, 0x01E0_461C, 0x32, 0x19);
            fx.mem.setf(p + 0x1090, x);
            let y = rand_f(fx, 0x01E0_465C, 0x19, 0);
            fx.mem.setf(p + 0x1094, y);
            let z = rand_f(fx, 0x01E0_468C, 0x32, 0x19);
            fx.mem.setf(p + 0x1098, z);
        }
    }
    fx.mem.write_u32(D + 0x1074, 1);
    hw(fx, &[(0x1080, 0xB0), (0x1082, 0x40), (0x1084, 0x20), (0x1086, 0x20)]);
    colour(fx, 0x1078, [0x80, 0x70, 0x40, 0x80]);
    // Four waves (+0x1680 + 0x30 k).
    for k in 0..4u32 {
        let g = 0x1680 + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0x60);
        fx.mem.write_u32(D + g, tex);
        fx.mem.write_u32(D + g + 0xC, D + 0x17E8);
        fx.mem.write_u32(D + g + 4, 0);
        hw(fx, &[(g + 0x10, 0xB0), (g + 0x12, 0x40), (g + 0x14, 0x20), (g + 0x16, 0x20)]);
        colour(fx, g + 8, [0x80, 0x40, 0x40, 0x80]);
        fx.mem.write_u32(D + g + 0x20, 0x3F19_999A);
        fx.mem.write_u32(D + g + 0x24, 0x3D4C_CCCD);
        hw(fx, &[(g + 0x2A, -2 * k as i32), (g + 0x28, 0x100)]);
        fx.mem.write_u8(D + g + 0x2C, (k + 7) as u8);
    }
    // The quad group (+0xE0).
    fx.mem.write_u32(D + 0xE4, 1);
    hw(fx, &[(0xF0, 0x8E), (0xF2, 0), (0xF4, 0x22), (0xF6, 0x40)]);
    colour(fx, 0xE8, [0x80, 0x10, 0x10, 0x20]);
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.quads_setup(D + 0xE0, tex, f32::from_bits(0x404C_CCCD), TEMPLATE);
    // Two rings (+0x800, +0xC00).
    for (g, a, outer) in [(0x800u32, 0x20u8, 0x4133_3333u32), (0xC00, 0x30, 0x414C_CCCD)] {
        fx.mem.write_u32(D + g + 4, 0x2000_0000);
        hw(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x40)]);
        colour(fx, g + 8, [0x90, 0x50, 0x20, a]);
        let tex = fx.mem.read_u32(sp + 0x60);
        fx.ring_setup(D + g, tex, 12.0, f32::from_bits(outer), 0.0, 0.0);
    }
    // The point quads (+0x5E0, points +0x540).
    fx.mem.write_u32(D + 0x618, 1);
    hw(fx, &[(0x5E0, 0x80), (0x5E2, 0), (0x5E4, 0x20), (0x5E6, 0x20), (0x5E8, 0xB0), (0x5EA, 0x20), (0x5EC, 0x100), (0x5EE, 0x100)]);
    fx.mem.write_u32(D + 0x610, 0x3F80_0000);
    colour(fx, 0x614, [0x40, 0, 0, 0x60]);
    hw(fx, &[(0x628, 0), (0x62A, 0), (0x62C, 0), (0x62E, 1), (0x630, 0), (0x632, 0x3E7)]);
    fx.mem.set32(D + 0x548, 1);
    let tex = fx.mem.read_u32(sp + 0x60);
    pointquad::setup(fx, SETUP, D + 0x5E0, D + 0x540, tex, 40.0);
    // Two late particle clouds (+0x13E0, +0x1530).
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.mem.write_u32(D + 0x13E0, tex);
    fx.mem.write_u32(D + 0x13E4, 5);
    for o in [0x13F0u32, 0x13F4, 0x13F8] {
        fx.mem.write_u32(D + o, 0);
    }
    hw(fx, &[(0x1400, 0xF0), (0x1402, 0), (0x1404, 0x10), (0x1406, 0x10), (0x1408, 0xF0), (0x140A, 0), (0x140C, 0x100), (0x140E, 0x100)]);
    fx.mem.write_u32(D + 0x13EC, 0x4100_0000);
    hw(fx, &[(0x1510, 0x10), (0x151A, 1), (0x151C, 0), (0x1520, 8), (0x1516, 0xA), (0x1518, 0xA)]);
    colour(fx, 0x13E8, [0x40, 0, 0, 0x40]);
    cloud8(fx, D + 0x13E0, [0x01E0_4A10, 0x01E0_4A48]);
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.mem.write_u32(D + 0x1530, tex);
    fx.mem.write_u32(D + 0x1534, 7);
    for o in [0x1540u32, 0x1544, 0x1548] {
        fx.mem.write_u32(D + o, 0);
    }
    hw(fx, &[(0x1550, 0x80), (0x1552, 0xC0), (0x1554, 0x20), (0x1556, 0x20), (0x1558, 0x80), (0x155A, 0xC0), (0x155C, 0x80), (0x155E, 0x100)]);
    fx.mem.write_u32(D + 0x153C, 0x4120_0000);
    hw(fx, &[(0x1660, 0x10), (0x166A, 1), (0x166C, 0), (0x1670, 8), (0x1666, 0xA), (0x1668, 0xA)]);
    colour(fx, 0x1538, [0x80, 0x20, 0x20, 0x40]);
    cloud8(fx, D + 0x1530, [0x01E0_4B48, 0x01E0_4B80]);
    for x in [0x70u32, 0x1000, 0x790] {
        for o in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + o, 0x3DCC_CCCD);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    for out in [0x1390u32, 0x7B0, 0x90, 0x1020] {
        let t = fx.target_handle();
        fx.unit_position(t, D + out);
    }
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x1760);
    // s0 = D, s1 = the OT, s2 the parameter (-1, 64-bit), s3 the runner's 0.
    let regs = [Some(D), Some(OT), Some(0), Some(0)];
    fx.xform_world_saving(D, 0x01E0_4C9C, regs);
    fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
    for (x, ra) in [(0x70u32, 0x01E0_4CA8u32), (0x1000, 0x01E0_4CB4), (0x1370, 0x01E0_4CC0), (0x790, 0x01E0_4CCC)] {
        fx.xform_billboard_saving(D + x, ra, regs);
        fx.saved(sp - 0xF0, &[(0x40, u64::MAX)]);
    }
    let l = fx.mem.read_u8(LIGHT);
    fx.set_stage_light(i32::from(l));
    if n(fx) >= 0x83 {
        fx.step_up(LIGHT, 0x80, 6);
    } else {
        fx.step_down(LIGHT, 0x20, 6);
    }
    if n(fx) >= 0x31 {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if (0x28..0x83).contains(&n(fx)) {
        let h = fx.self_handle();
        for o in [0x6Cu32, 0x6D, 0x6E, 0x6B, 0x68, 0x69, 0x6A] {
            fx.mem.write_u8(sp + o, 0x80);
        }
        fx.mem.write_u8(sp + 0x6F, 0);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h as u32, 3, 0, sp + 0x68]);
    }
    if n(fx) >= 0x35 {
        crate::b262::tint(fx, D + 0x18EC);
        fx.step_down(D + 0x18EF, 0, 8);
        for o in [0x58u32, 0x54, 0x50] {
            fx.mem.write_u32(sp + o, 0x3F82_8F5C);
        }
        fx.particle_quads(D + 0x1370, D + 0x640, sp + 0x50);
        fx.step_down(D + 0x64B, 0, 4);
    }
    if n(fx) >= 0x3B {
        pointquad::squares(fx, OT, D + 0x1070, D + 0x1000, D + 0x1360);
        fx.step_down(D + 0x107B, 0, 8);
        pointquad::draw(fx, OT, D + 0x5E0, D + 0x540, D + 0x70);
        fx.quads_draw(D + 0xE0, D + 0x70);
        if ee_lt(fx.mem.f(D + 0x1000), 1.0) {
            let s = ee::add(fx.mem.f(D + 0x1008), 0.5);
            for o in [0x1008u32, 0x1004, 0x1000] {
                fx.mem.setf(D + o, s);
            }
        }
        if ee_lt(fx.mem.f(D + 0x70), 1.0) {
            let d = f32::from_bits(0x3ECC_CCCD);
            for o in [0x70u32, 0x74, 0x78] {
                let s = ee::add(fx.mem.f(D + o), d);
                fx.mem.setf(D + o, s);
            }
        } else {
            let d = f32::from_bits(0x3BA3_D70A);
            for o in [0x70u32, 0x74, 0x78] {
                let s = ee::add(fx.mem.f(D + o), d);
                fx.mem.setf(D + o, s);
            }
            fx.step_down(D + 0xEB, 0, 3);
            fx.step_down(D + 0x617, 0, 6);
        }
        for k in 0..0x40u32 {
            for c in 0..3u32 {
                let a = D + 0x140 + 0x10 * k + 4 * c;
                let w = fx.mem.read_u32(a).wrapping_add(0x80);
                fx.mem.write_u32(a, w);
            }
        }
        for o in [0x58u32, 0x54, 0x50] {
            fx.mem.write_u32(sp + o, 0);
        }
        fx.particle_quads(D + 0x1370, D + 0x13E0, sp + 0x50);
        fx.particle_quads(D + 0x1370, D + 0x1530, sp + 0x50);
        if n(fx) >= 0x74 {
            fx.step_down(D + 0x13EB, 0, 4);
            fx.step_down(D + 0x153B, 0, 4);
        }
        if n(fx) < 0x78 {
            for k in 0..4u32 {
                let g = D + 0x1680 + 0x30 * k;
                if fx.mem.i16(g + 0x2A) >= 0xA {
                    fx.mem.set16(g + 0x2A, 1);
                    fx.mem.write_u8(g + 0xB, 0x80);
                }
                wave::draw(fx, OT, g, D + 0x1370);
                fx.step_down(g + 0xB, 0, 0x10);
            }
            fx.ring_draw(D + 0x800, D + 0x790);
            fx.ring_draw(D + 0xC00, D + 0x790);
            fx.step_down(D + 0x80B, 0, 2);
            fx.step_down(D + 0xC0B, 0, 3);
            if fx.approach(D + 0x790, 2.0, f32::from_bits(0x3E4C_CCCD)) == 0 {
                for o in [0x798u32, 0x794, 0x790] {
                    fx.mem.write_u32(D + o, 0x3DCC_CCCD);
                }
                fx.mem.write_u8(D + 0x80B, 0x20);
                fx.mem.write_u8(D + 0xC0B, 0x30);
            }
        }
    }
    if n(fx) < 0x31 {
        let r = fx.rand_from(D, 0x01E0_5160);
        fx.mem.write_u8(D + 0x17B6, (r.wrapping_rem(0x40) + 0x20) as u8);
        let a = fx.mem.read_u8(D + 0x17B7);
        fx.mem.write_u8(D + 0x18EB, 0x80u8.wrapping_sub(a));
        afterimage::strips(fx, D + 0x17B0, D + 0x1760);
        let h = fx.mem.i16(D + 0x17BA);
        unit_tint(fx, h, D + 0x18E8);
        if n(fx) >= 0x1F {
            fx.step_down(D + 0x17B7, 0, 8);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0x82 {
        2
    } else if m == 0x2B {
        1
    } else if (0x35..0x82).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
