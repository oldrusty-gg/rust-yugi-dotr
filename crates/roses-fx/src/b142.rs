//! BMODULE b142 (card 65), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E030D0, data area 0x1ED0000; compiled without
//! delay-slot scheduling) [F]: b028's burst
//! without the quad trail. Object op 3 on the attacker (0x1F83B0) before
//! update 28, on 109..155 and 170..179; a smoke cloud (+0x12F0) at node
//! 0x38 on updates 61..89; at the target from update 97 a pair of
//! cylinders (+0x14B0 / +0x18B0, heights +- 50) whose radius scale +0x1444
//! swells to 1, shrinks back to 0.2 on 155..167 and out to 1.6 from 169;
//! from update 176 at node 0x22 the squares (+0xF90), two rings (+0x5D0 /
//! +0x9D0), the point-set strip (+0x4D0 / look +0x570) and a quad burst
//! (+0x70) growing by 0.4 to 1 then by 0.005 while all fade, with a
//! particle cloud (+0xDD0). Returns 1 on update 165, 2 on 175, 8 after
//! 194 (never 6).
//!
//! Library: `pointquad::setup` / `draw` / `squares` (0x1E00120 /
//! 0x1E00460 / 0x1E01040), `ring_setup` / `ring_draw` (0x1E01620 /
//! 0x1E018E0), `particle_quads` (0x1E01DD0), `quads_setup` / `quads_draw`
//! (0x1E02960 / 0x1E02AC0).
use crate::b028::{cloud16, cloud16_spawn, look_init, square_points};
use crate::b436::{fstep_down, ring_group};
use crate::b628::halves;
use crate::blib::pointquad;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SETUP: u32 = 0x01E0_0120;
/// The quad corner template (rodata, 0x1E02960's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_3F80;

/// The entry 0x1E030D0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x80);
    ret
}

/// 0x103390(p, cap, step) [F]: *p += step while *p <= cap - step (1),
/// else *p = cap (0).
pub(crate) fn fstep_up(fx: &mut Fx, p: u32, cap: f32, step: f32) -> i32 {
    let v = fx.mem.f(p);
    if ee_le(v, ee::sub(cap, step)) {
        fx.mem.setf(p, ee::add(v, step));
        1
    } else {
        fx.mem.setf(p, cap);
        0
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1CE0);
    for x in [0u32, 0xF20, 0x1440, 0x1280] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x1CBCu32, 0x22, 1), (0x1CC0, 0x39, 0), (0x1CC4, 0x38, 0), (0x1CC8, 0x3A, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x1CB0, 0xC3);
    fx.mem.set32(D + 0x1CB4, 1);
    fx.mem.set32(D + 0x1CB8, param);
    for k in [0x1CD0u32, 0x1CD1, 0x1CD2, 0x1CCF, 0x1CCC, 0x1CCD, 0x1CCE] {
        fx.mem.write_u8(D + k, 0x80);
    }
    fx.mem.write_u8(D + 0x1CD3, 0);
    fx.texture4(sp + 0x70, 8);
    // The smoke cloud (+0x12F0): 16 particles, half size 1, 6 frames.
    let g = D + 0x12F0;
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x8_0085);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0), (0x2A, 0x3C), (0x24, 0xA), (0x26, 0xA), (0x2C, 0x80), (0x2E, 0x80)]);
    fx.mem.write_u32(g + 0xC, 0x3F80_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 1), (0x13C, 0), (0x140, 6), (0x136, 0xA), (0x138, 0xA)]);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    cloud16_spawn(fx, g);
    fx.texture4(sp + 0x70, 0xF);
    // The squares (+0xF90): half size 0.75.
    let g = D + 0xF90;
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 0xC, 0x3F40_0000);
    square_points(fx, g);
    fx.mem.write_u32(g + 4, 0x2000_0001);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x40), (0x14, 0x20), (0x16, 0x20)]);
    for (k, x) in [(0xAu32, 0xD0u8), (9, 0xA0), (8, 0x70), (0xB, 0x80)] {
        fx.mem.write_u8(g + k, x);
    }
    // The quad burst (+0x70): size 6.4.
    let g = D + 0x70;
    fx.mem.write_u32(g + 4, 0x8_0001);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    for (k, x) in [(0xAu32, 0xD0u8), (9, 0xA0), (8, 0x70), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, x);
    }
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.quads_setup(g, tex, f32::from_bits(0x40CC_CCCD), QUAD_TEMPLATE);
    // Rings: +0x5D0 (5 / 12, width 10), +0x9D0 (1 / 32), the cylinders
    // +0x14B0 / +0x18B0 (radius 8, heights 50 / -50).
    ring_group(fx, D + 0x5D0, 0x2000_8000, 0x40, [0x70, 0xA0, 0xD0, 0x20]);
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.ring_setup(D + 0x5D0, tex, 5.0, 12.0, 0.0, 10.0);
    ring_group(fx, D + 0x9D0, 0x2000_0000, 0x40, [0x70, 0x70, 0xD0, 0x30]);
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.ring_setup(D + 0x9D0, tex, 1.0, 32.0, 0.0, 0.0);
    for (g, h) in [(0x14B0u32, 50.0f32), (0x18B0, -50.0)] {
        ring_group(fx, D + g, 0, 0x40, [0xA0, 0x40, 0xA0, 0x20]);
        let tex = fx.mem.read_u32(sp + 0x70);
        fx.ring_setup(D + g, tex, 8.0, 8.0, 0.0, h);
    }
    look_init(fx, D + 0x570);
    fx.mem.write_u32(D + 0x4D8, 1);
    let tex = fx.mem.read_u32(sp + 0x70);
    pointquad::setup(fx, SETUP, D + 0x570, D + 0x4D0, tex, 60.0);
    let tex = fx.mem.read_u32(sp + 0x70);
    cloud16(fx, D + 0xDD0, tex, [0xA0, 0x40, 0xA0, 0x60]);
    fx.mem.write_u32(D + 0x1444, 0x3DCC_CCCD);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x1CB4);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    node(fx, 0x1CC4, D + 0x1460);
    node(fx, 0x1CC4, D + 0x12A0);
    if n(fx) >= 0x71 {
        node(fx, 0x1CC0, sp + 0x50);
        node(fx, 0x1CC8, sp + 0x60);
        fx.look_angles(sp + 0x60, sp + 0x50, D + 0x1450);
    }
    if n(fx) < 0xB1 {
        node(fx, 0x1CBC, D + 0x20);
        node(fx, 0x1CBC, D + 0xF40);
    }
    // s0 = D, s1 = the OT, s2 the param (-1, 64 bits), s3 the caller's.
    let regs = [Some(D), Some(OT), None, None];
    fx.xform_billboard_saving(D, 0x01E0_3928, regs);
    fx.saved(sp - 0xF0, &[(0x40, u64::MAX)]);
    fx.xform_billboard_saving(D + 0xF20, 0x01E0_3930, regs);
    fx.saved(sp - 0xF0, &[(0x40, u64::MAX)]);
    fx.xform_world_saving(D + 0x1440, 0x01E0_3938, regs);
    fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
    fx.xform_billboard_saving(D + 0x1280, 0x01E0_3940, regs);
    fx.saved(sp - 0xF0, &[(0x40, u64::MAX)]);
    let m = n(fx);
    if m < 0x1C || (0x6D..0x9C).contains(&m) || (0xAA..0xB4).contains(&m) {
        let h = fx.self_handle();
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h as u32, 3, 0, D + 0x1CCC]);
    }
    let m = n(fx);
    if (0x3D..0x5A).contains(&m) {
        for k in [0x58u32, 0x54, 0x50] {
            fx.mem.write_u32(sp + k, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0x1280, D + 0x12F0, sp + 0x50);
        if n(fx) >= 0x56 {
            fx.step_down(D + 0x12FB, 0, 0x20);
        }
    }
    if n(fx) >= 0x61 {
        fx.ring_draw(D + 0x14B0, D + 0x1440);
    }
    if n(fx) >= 0x68 {
        fx.step_down(D + 0x14BB, 0, 4);
    }
    if n(fx) >= 0x9D {
        fx.ring_draw(D + 0x18B0, D + 0x1440);
    }
    let fifth = f32::from_bits(0x3E4C_CCCD);
    if (0x61..0x72).contains(&n(fx)) {
        fstep_up(fx, D + 0x1444, 1.0, fifth);
    }
    if (0x9B..0xA8).contains(&n(fx)) {
        fx.step_up(D + 0x14BB, 0x20, 8);
        fstep_down(fx, D + 0x1444, fifth, f32::from_bits(0x3D75_C28F));
    }
    if n(fx) == 0xA8 {
        for k in [0x14BAu32, 0x14B9, 0x14B8, 0x14BB] {
            fx.mem.write_u8(D + k, 0x80);
        }
    }
    if n(fx) >= 0xA8 {
        fx.step_down(D + 0x18BB, 0, 4);
    }
    if n(fx) >= 0xA9 {
        if n(fx) >= 0xAA {
            fx.step_down(D + 0x14BB, 0, 0x10);
        }
        fstep_up(fx, D + 0x1444, f32::from_bits(0x3FCC_CCCD), fifth);
    }
    if n(fx) >= 0xB0 {
        if ee_lt(fx.mem.f(D + 0xF20), 1.0) {
            let v = ee::add(fx.mem.f(D + 0xF28), 0.5);
            for k in [0xF28u32, 0xF24, 0xF20] {
                fx.mem.setf(D + k, v);
            }
        }
        for k in [0x58u32, 0x54, 0x50] {
            fx.mem.write_u32(sp + k, 0x3F83_D70A);
        }
        pointquad::squares(fx, OT, D + 0xF90, D + 0xF20, sp + 0x50);
        fx.step_down(D + 0xF9B, 0, 0xA);
        fx.ring_draw(D + 0x5D0, D);
        fx.ring_draw(D + 0x9D0, D);
        pointquad::draw(fx, OT, D + 0x570, D + 0x4D0, D);
        fx.quads_draw(D + 0x70, D);
        let grow = ee_le(fx.mem.f(D), 1.0);
        let step = f32::from_bits(if grow { 0x3ECC_CCCD } else { 0x3BA3_D70A });
        for k in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(D + k), step);
            fx.mem.setf(D + k, v);
        }
        if !grow {
            for (k, s) in [(0x7Bu32, 6), (0x5DB, 6), (0x9DB, 6), (0x5A7, 0xC)] {
                fx.step_down(D + k, 0, s);
            }
            for k in [0x58u32, 0x54, 0x50] {
                fx.mem.write_u32(sp + k, 0x3F82_8F5C);
            }
            fx.particle_quads(D, D + 0xDD0, sp + 0x50);
            fx.step_down(D + 0xDDB, 0, 4);
        }
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0xD0 + 16 * k + 4 * c;
                let x = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, x);
            }
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x1CB4, m);
    let left = fx.mem.i32(D + 0x1CB0) - 1;
    fx.mem.set32(D + 0x1CB0, left);
    if m == 0xB0 {
        2
    } else if m == 0xA6 {
        1
    } else if left > 0 {
        0
    } else {
        8
    }
}
