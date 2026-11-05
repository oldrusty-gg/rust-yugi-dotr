//! BMODULE b435 (card 463), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05B90, data area 0x1ED0000) [F] (helper C7, 31
//! Oct 2026): the attacker's afterimage in wavy strips (+0x2860,
//! `blib::afterimage`, the unit tinted by 0x1F75F0) fading in on updates
//! 0x28..0x92 and out from 0x94; the stage dimmed to 0x20 until 0xEC; on
//! updates 0x32..0x59 a sprite fountain (+0x1B90) at the attacker's node
//! 0xB; from 0x72 a ring swelling at the target (+0x16A0), sparks (+0x370)
//! and a particle cloud (+0x14E0); from 0x7B a frame quad, a ribbon
//! (+0x70) from node 0xB flying to the target (0x104490), a ring
//! (+0x23F0) and two flashing discs (+0x1450, +0x14D0), with plates
//! (+0x1410) and a disc (+0x1B10) until 0xB5. Returns 1 on update 0x89, 6
//! on 0x93..0xEB, 2 on 0xEC, 8 after 0x104.
//!
//! Library at its own addresses: `colours4` (0x1E00080), the frame quad
//! (0x1E00120, b26f4b13c68d), `strips` (0x1E00240), `scatter` (0x1E008B0),
//! `trail::sparks` (0x1E00A70), `ring_setup` / `ring_draw` (0x1E017D0 /
//! 0x1E01A90), `plates` (0x1E01F80), `disc` (0x1E02510), `sprites_spawn`
//! / `sprites` (0x1E02A70 / 0x1E02C00), `particle_quads` (0x1E044F0),
//! ribbon push / draw (0x1E05080 / 0x1E051D0).
use crate::b101::unit_tint;
use crate::b186::{bytes, halves};
use crate::blib::{afterimage, plate, spawn, trail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PLATES: u32 = 0x01E0_1F80;

/// The entry 0x1E05B90.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

/// x % 2 (signed, as compiled).
fn odd(n: i32) -> bool {
    n.wrapping_rem(2) != 0
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x28B0);
    for x in [0u32, 0x13A0, 0x300, 0x1630, 0x1AA0, 0x1460, 0x1B20, 0x27F0] {
        fx.xform_init(D + x);
    }
    for (at, n, clear) in [(0x28A0u32, 0x10, 1), (0x28A4, 0xB, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2890, 0x104);
    fx.mem.set32(D + 0x2894, 1);
    fx.mem.set32(D + 0x2898, param);
    fx.mem.set32(D + 0x288C, 0x24);
    fx.mem.set32(D + 0x289C, 0x80);
    // The afterimage (+0x2860) of the attacker.
    fx.mem.write_u32(D + 0x2860, 0x4_1000);
    bytes(fx, &[(0x2866, 0x60), (0x2865, 0x60), (0x2864, 0x80), (0x2867, 0)]);
    fx.mem.write_u32(D + 0x2870, 0);
    fx.mem.write_u32(D + 0x286C, 0x2000);
    halves(fx, &[(0x2868, 0)]);
    fx.mem.write_u32(D + 0x2874, 0x40A0_0000);
    let h = fx.self_handle();
    halves(fx, &[(0x286A, h)]);
    bytes(fx, &[(0x288A, 0x80), (0x2889, 0x80), (0x2888, 0x80), (0x288B, 0)]);
    // Discs (+0x14D0, +0x1450, +0x1B10).
    for (g, c, a, f) in [(0x14D0u32, 0x8_0000u32, [0x60u8, 0x60, 0x80, 0xFF], 0x41A0_0000u32), (0x1450, 0, [0x60, 0x60, 0x80, 0xFF], 0x40A0_0000), (0x1B10, 0x8_0000, [0x80; 4], 0x4000_0000)] {
        fx.mem.write_u32(D + g, c);
        bytes(fx, &[(g + 6, a[0]), (g + 5, a[1]), (g + 4, a[2]), (g + 7, a[3])]);
        fx.mem.write_u32(D + g + 8, f);
        halves(fx, &[(g + 0xC, 0x24), (g + 0xE, 0)]);
    }
    fx.texture5(sp + 0xB0, 0);
    // The ribbon (+0x70, points +0xA0).
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(D + 0x70, tex);
    fx.mem.write_u32(D + 0x78, 0x8003);
    bytes(fx, &[(0x76, 0x80), (0x75, 0x80), (0x74, 0x80), (0x77, 0x80)]);
    halves(fx, &[(0x80, 0), (0x82, 0), (0x84, 0x10), (0x86, 0x20), (0x88, 0), (0x8A, 0), (0x8C, 0x80), (0x8E, 0x40)]);
    fx.mem.write_u32(D + 0x7C, 0x4220_0000);
    halves(fx, &[(0x92, 8), (0x94, 0), (0x96, 0x6E), (0x98, 0x26), (0x9A, 0), (0x9C, 1)]);
    // The sparks (+0x370).
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(D + 0x370, tex);
    fx.mem.write_u32(D + 0x374, 0x2000_0003);
    bytes(fx, &[(0x37A, 0x80), (0x379, 0x80), (0x378, 0x80), (0x37B, 0x80)]);
    fx.mem.write_u32(D + 0x390, 0x3F00_0000);
    fx.mem.write_u32(D + 0x394, 0x4040_0000);
    halves(fx, &[(0x380, 0), (0x382, 0x20), (0x384, 0x20), (0x386, 0x20), (0x388, 0), (0x38A, 0x20), (0x38C, 0x80), (0x38E, 0x40)]);
    fx.mem.write_u32(D + 0x37C, D + 0x3A0);
    fx.mem.write_u32(D + 0x398, 0x4120_0000);
    halves(fx, &[(0x39C, 0x100)]);
    bytes(fx, &[(0x39E, 1), (0x39F, 0)]);
    spawn::scatter(fx, D + 0x370, 0xC, 3.0);
    // The plates (+0x1410).
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(D + 0x1410, tex);
    fx.mem.write_u32(D + 0x1414, 0x8_0000);
    bytes(fx, &[(0x141A, 0x80), (0x1419, 0x80), (0x1418, 0x80), (0x141B, 0x80)]);
    for (o, v) in [(0x141Cu32, 0x3F00_0000u32), (0x1420, 0x4080_0000), (0x1424, 0x3F00_0000), (0x1428, 0x4040_0000)] {
        fx.mem.write_u32(D + o, v);
    }
    halves(fx, &[(0x1430, 0x1E), (0x1432, 0), (0x1434, 0x20), (0x1436, 0x20)]);
    for o in [0x1440u32, 0x1444, 0x1448] {
        fx.mem.write_u32(D + o, 0);
    }
    fx.texture4(sp + 0xB0, 0xF);
    // The particle cloud (+0x14E0, 16 particles at +0x1510).
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(D + 0x14E0, tex);
    fx.mem.write_u32(D + 0x14E4, 0x85);
    for o in [0x14F0u32, 0x14F4, 0x14F8, 0x14FC] {
        fx.mem.write_u32(D + o, 0);
    }
    halves(fx, &[(0x1508, 0xF0), (0x150A, 0), (0x1504, 0x10), (0x1506, 0x10), (0x150C, 0x100), (0x150E, 0x100)]);
    fx.mem.write_u32(D + 0x14EC, 0x41A0_0000);
    halves(fx, &[(0x1610, 0x10), (0x161A, 1), (0x161C, 1), (0x1620, 8), (0x1616, 0xA), (0x1618, 0xA)]);
    bytes(fx, &[(0x14EA, 0x40), (0x14E9, 0x50), (0x14E8, 0x80), (0x14EB, 0x20)]);
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0x1610) {
        let p = D + 0x10 * k as u32;
        for (o, ra) in [(0u32, 0x01E0_5F60u32), (4, 0x01E0_5F94)] {
            let r = fx.rand_from(D, ra);
            let (_, h) = fx.div(r, 0x14);
            let v = ee::add(fx.mem.f(D + 0x14F0 + o), ee::from_i32(h - 0xA));
            fx.mem.setf(p + 0x1510 + o, v);
        }
        let z = fx.mem.read_u32(D + 0x14F8);
        fx.mem.write_u32(p + 0x1518, z);
        let a = fx.mem.i16(D + 0x161A).wrapping_neg();
        let f = fx.mult(a, k);
        fx.mem.set16(p + 0x151E, f);
        fx.mem.set16(p + 0x151C, 0);
        let c = fx.mem.i16(D + 0x1620) - 1;
        fx.mem.set16(D + 0x161E, c);
        k += 1;
    }
    // Two rings (+0x16A0, +0x23F0).
    fx.mem.write_u32(D + 0x16A4, 0);
    halves(fx, &[(0x16B0, 0x8E), (0x16B2, 0), (0x16B4, 0x20), (0x16B6, 0x80)]);
    bytes(fx, &[(0x16AA, 0x60), (0x16A9, 0x60), (0x16A8, 0x80), (0x16AB, 0x80)]);
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.ring_setup(D + 0x16A0, tex, f32::from_bits(0x4133_3333), f32::from_bits(0x414C_CCCD), 0.0, 0.0);
    fx.mem.write_u32(D + 0x23F4, 0x8000);
    halves(fx, &[(0x2400, 0x8E), (0x2402, 0), (0x2404, 0x20), (0x2406, 0x40)]);
    bytes(fx, &[(0x23FA, 0x40), (0x23F9, 0x40), (0x23F8, 0x80), (0x23FB, 0x80)]);
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.ring_setup(D + 0x23F0, tex, 1.0, 4.0, 0.0, 8.0);
    // The sprite fountain (+0x1B90, sprites +0x1BF0).
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(D + 0x1B90, tex);
    fx.mem.write_u32(D + 0x1B94, 0x7000_0001);
    bytes(fx, &[(0x1B9A, 0x60), (0x1B99, 0x60), (0x1B98, 0x80), (0x1B9B, 0)]);
    fx.mem.write_u32(D + 0x1B9C, 0x3F80_0000);
    for o in [0x1BB0u32, 0x1BB4, 0x1BB8] {
        fx.mem.write_u32(D + o, 0);
    }
    halves(fx, &[(0x1BC0, 0), (0x1BC2, 0xE4), (0x1BC4, 0xA), (0x1BC6, 0xA), (0x1BC8, 0), (0x1BCA, 0xE4), (0x1BCC, 0x80), (0x1BCE, 0x40)]);
    fx.mem.write_u32(D + 0x1BD0, D + 0x1BF0);
    fx.mem.write_u32(D + 0x1BD4, 0);
    fx.mem.write_u32(D + 0x1BD8, 0);
    fx.mem.write_u32(D + 0x1BDC, 0x4140_0000);
    halves(fx, &[(0x1BEA, 0x80), (0x1BEC, 2), (0x1BEE, 0)]);
    fx.sprites_spawn(D + 0x1B90, 1.0);
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0x1BEA) {
        let p = D + 0x10 * k as u32;
        let r = fx.rand_from(D, 0x01E0_617C);
        let (_, h) = fx.div(r, 0xC);
        k += 1;
        fx.mem.set16(p + 0x1BFE, h.wrapping_neg());
        fx.mem.write_u8(p + 0x1BFC, 0x80);
        fx.mem.write_u8(p + 0x1BFD, 0);
        let y = ee::add(fx.mem.f(p + 0x1BF4), 10.0);
        fx.mem.setf(p + 0x1BF4, y);
    }
    0
}

/// The fountain's sprites (+0x1BF0 + 0x10 k: x, y, z, .., life +0xE)
/// fall by 1 and draw in by 0.8 while above 0; below 0 thrown again: a
/// random direction (rand & 0xFFFF) at speed +0x1BDC, y 10 + rand % 10.
fn fountain_step(fx: &mut Fx) {
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0x1BEA) {
        let p = D + 0x10 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x1BFE) < 0 {
            continue;
        }
        let y = ee::sub(fx.mem.f(p + 0x1BF4), 1.0);
        fx.mem.setf(p + 0x1BF4, y);
        if !ee_le(y, 0.0) {
            let g = f32::from_bits(0x3F4C_CCCD);
            let z = ee::mul(fx.mem.f(p + 0x1BF8), g);
            fx.mem.setf(p + 0x1BF8, z);
            let x = ee::mul(fx.mem.f(p + 0x1BF0), g);
            fx.mem.setf(p + 0x1BF0, x);
        }
        if ee_lt(fx.mem.f(p + 0x1BF4), 0.0) {
            let a = fx.rand_from(D, 0x01E0_658C) & 0xFFFF;
            let c = fx.tcos(a);
            let v = ee::mul(fx.mem.f(D + 0x1BDC), c);
            fx.mem.setf(p + 0x1BF8, v);
            let s = fx.tsin(a);
            let v = ee::mul(fx.mem.f(D + 0x1BDC), s);
            fx.mem.setf(p + 0x1BF0, v);
            let r = fx.rand_from(D, 0x01E0_65D0);
            let (_, h) = fx.div(r, 0xA);
            fx.mem.setf(p + 0x1BF4, ee::add(10.0, ee::from_i32(h)));
        }
    }
}

/// The sparks (+0x3A0 + 0x10 k, life +0xE) spread by 1.1; above y 30
/// thrown again: y 4, a random direction (rand & 0xFFFF) at 0.1 (rand %
/// (10 +0x398)), life -(rand % 0xC).
fn sparks_step(fx: &mut Fx) {
    let grow = f32::from_bits(0x3F8C_CCCD);
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0x39C) {
        let p = D + 0x10 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x3AE) < 0 {
            continue;
        }
        for o in [0x3A0u32, 0x3A4, 0x3A8] {
            let v = ee::mul(fx.mem.f(p + o), grow);
            fx.mem.setf(p + o, v);
        }
        if ee_le(fx.mem.f(p + 0x3A4), 30.0) {
            continue;
        }
        let v = ee::mul(10.0, fx.mem.f(D + 0x398));
        fx.fix_residue(v, 0x01E0_6870);
        let s5 = ee::fixsfsi(v);
        let a = fx.rand_from(D, 0x01E0_6878) & 0xFFFF;
        let r = fx.rand_from(D, 0x01E0_6894);
        let (_, h) = fx.div(r, s5);
        fx.mem.write_u32(p + 0x3A4, 0x4080_0000);
        let f20 = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
        let c = fx.tcos(a);
        fx.mem.setf(p + 0x3A8, ee::mul(f20, c));
        let s = fx.tsin(a);
        fx.mem.setf(p + 0x3A0, ee::mul(f20, s));
        let r = fx.rand_from(D, 0x01E0_68E4);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0x3AE, h.wrapping_neg());
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2894);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x2810);
    node(fx, 0x28A0, sp + 0x90);
    node(fx, 0x28A4, sp + 0xA0);
    node(fx, 0x28A4, D + 0x1B40);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x1B30);
    node(fx, 0x28A4, D + 0x1AC0);
    for out in [D + 0x2878, D + 0x1480, D + 0x320, D + 0x1650] {
        let t = fx.target_handle();
        fx.unit_position(t, out);
    }
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x90);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0xA0);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x310);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x1640);
    if n(fx) < 0x7C {
        node(fx, 0x28A4, D + 0x13C0);
    }
    if n(fx) >= 0x7B {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x13C0);
        let k = fx.mem.i32(D + 0x288C);
        fx.approach_by(D + 0x13C0, D + 0x2878, k);
        let k = fx.mem.i32(D + 0x288C);
        if k != 0 {
            fx.mem.set32(D + 0x288C, k - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 = sp + 0xA0, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(sp + 0xA0), Some(0)];
    fx.xform_world_saving(D, 0x01E0_636C, regs);
    fx.xform_billboard_saving(D + 0x13A0, 0x01E0_6374, regs);
    fx.xform_world_saving(D + 0x300, 0x01E0_637C, regs);
    fx.xform_world_saving(D + 0x1630, 0x01E0_6384, regs);
    fx.xform_billboard_saving(D + 0x1AA0, 0x01E0_638C, regs);
    fx.xform_billboard_saving(D + 0x1460, 0x01E0_6394, regs);
    fx.xform_world_saving(D + 0x1B20, 0x01E0_639C, regs);
    fx.xform_world_saving(D + 0x27F0, 0x01E0_63A4, regs);
    if n(fx) >= 0x7B {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0xEC {
        fx.step_down(D + 0x289C, 0x20, 4);
    } else {
        fx.step_up(D + 0x289C, 0x80, 4);
    }
    let l = fx.mem.read_u8(D + 0x289C);
    fx.set_stage_light(i32::from(l));
    let m = n(fx);
    if m < 0xEC {
        if (0x28..0x93).contains(&m) {
            fx.step_up(D + 0x2867, 0x7F, 8);
        }
        let a = fx.mem.read_u8(D + 0x2867);
        fx.mem.write_u8(D + 0x288B, 0x80u8.wrapping_sub(a));
        afterimage::strips(fx, D + 0x2860, D + 0x2810);
        let h = fx.mem.i16(D + 0x286A);
        unit_tint(fx, h, D + 0x2888);
        if n(fx) >= 0x94 {
            fx.step_down(D + 0x2867, 0, 8);
        }
    }
    if n(fx) >= 0x7B {
        fx.ring_draw(D + 0x23F0, D + 0x1B20);
        if n(fx) >= 0x64 {
            fx.step_down(D + 0x23FB, 0, 6);
        }
    }
    if (0x32..0x5A).contains(&n(fx)) {
        fx.sprites(D + 0x1B90, D + 0x1B20);
        fx.step_up(D + 0x1B9B, 0x80, 8);
        fountain_step(fx);
    }
    if n(fx) >= 0x82 {
        fx.disc(D + 0x1450, D + 0x13A0);
    }
    let m = n(fx);
    if m >= 0xEC {
        fx.step_down(D + 0x1457, 0, 0x10);
    } else {
        fx.mem.write_u32(D + 0x1458, if odd(m) { 0x4040_0000 } else { 0x40A0_0000 });
    }
    if n(fx) >= 0x7B {
        fx.disc(D + 0x14D0, D + 0x1AA0);
        let m = n(fx);
        if m >= 0xEC {
            fx.step_down(D + 0x14D7, 0, 0x10);
        } else {
            fx.mem.write_u8(D + 0x14D7, if odd(m) { 0x80 } else { 0xFF });
        }
    }
    if (0x7B..0xB6).contains(&n(fx)) {
        plate::plates(fx, PLATES, OT, D + 0x1410, D + 0x1AA0);
        fx.disc(D + 0x1B10, D + 0x1AA0);
    }
    if n(fx) >= 0x72 {
        fx.ring_draw(D + 0x16A0, D + 0x1630);
        fx.step_down(D + 0x16AB, 0, 8);
        if fx.approach(D + 0x1630, 2.0, f32::from_bits(0x3E4C_CCCD)) == 0 && n(fx) < 0xEC {
            for o in [0x1638u32, 0x1634, 0x1630] {
                fx.mem.write_u32(D + o, 0x3DCC_CCCD);
            }
            fx.mem.write_u8(D + 0x16AB, 0x80);
        }
        trail::sparks(fx, OT, D + 0x370, D + 0x300);
        if n(fx) >= 0xED {
            fx.step_down(D + 0x37B, 0, 8);
        }
        sparks_step(fx);
        for o in [0x88u32, 0x84, 0x80] {
            fx.mem.write_u32(sp + o, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0x1460, D + 0x14E0, sp + 0x80);
        if n(fx) >= 0xEC {
            fx.step_down(D + 0x14EB, 0, 2);
        }
    }
    if n(fx) >= 0x7C {
        for (o, ra) in [(0x76u32, 0x01E0_6978u32), (0x75, 0x01E0_6998), (0x74, 0x01E0_69B8)] {
            let r = fx.rand_from(D, ra);
            fx.mem.write_u8(D + o, (r.wrapping_rem(0x40) + 0x40) as u8);
        }
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
        if n(fx) >= 0xED {
            crate::b436::fstep_down(fx, D + 0x7C, 0.0, 4.0);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2894, m);
    let left = fx.mem.i32(D + 0x2890) - 1;
    fx.mem.set32(D + 0x2890, left);
    if m == 0xEC {
        2
    } else if m == 0x89 {
        1
    } else if (0x93..0xEC).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
