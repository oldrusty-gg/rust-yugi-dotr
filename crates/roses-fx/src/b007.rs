//! BMODULE b007 (card 649), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03980, data area 0x1ED0000) [F] (helper D10, 31
//! Oct 2026): b642's family on another layout. Two turned plates (+0x15E0
//! / +0x1690, spinning 0x400 a step until update 64) with a disc each
//! (+0x2660 / +0x26E0) at the attacker's nodes 0x1F / 0x20, faded in on
//! 12..27 and out on 28..43, drawn until update 110; from update 78 two
//! ribbon trails (+0x70 / +0xA0) from those nodes to the target (1 / 24
//! of the way per update, then 1 / 23 ...); a flickering disc (+0x1740)
//! on the target; three widening rings (+0x19F0 + 0x400 k) on the target,
//! restarted when grown to 2; a spark fountain (+0x540) respawned on a
//! ring when above 30 and from update 79 a particle cloud (+0x1750)
//! swelling at the target. Returns 1 on update 88, 6 on 98..202, 2 on 203,
//! 8 after 229.
//!
//! Library: `colours4` 0x1E00080, `spawn::scatter` (its register twin
//! 0x1E00120, 53ac345064f9), `trail::sparks` 0x1E002E0 [`crate::blib`];
//! `ring_setup` / `ring_draw` 0x1E01040 / 0x1E01300, `plate::plates`
//! 0x1E017F0, `disc` 0x1E01D80, `particle_quads` 0x1E022E0
//! [`crate::fxlib`]; the ribbon push 0x1E02E70 and draw (its register twin
//! 0x1E02FC0, bfc5b76dc028: (ot, g, pts, x)) [`crate::ribbon`].
use crate::b642::{disc_record, halves, plate_group};
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const N: u32 = D + 0x2708;
const LEFT: u32 = D + 0x2704;
const FOLLOW: u32 = D + 0x2700;
const PLATES: u32 = 0x01E0_17F0;

/// The entry 0x1E03980.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

/// A ribbon group (cell (0, 0, 0x16, 0x40), width 20, life 0x48, 32
/// slots).
fn ribbon_group(fx: &mut Fx, g: u32, tex: u32) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 8, 0x8003);
    for k in [6u32, 5, 4, 7] {
        fx.mem.write_u8(g + k, 0x80);
    }
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x16), (0x16, 0x40), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x80)]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    halves(fx, g, &[(0x22, 8), (0x24, 0), (0x26, 0x48), (0x28, 0x20), (0x2A, 0), (0x2C, 1)]);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2720);
    for x in [0u32, 0x1570, 0x1620, 0x4D0, 0x18A0, 0x1910, 0x1980, 0x25F0, 0x2670, 0x16D0] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2710u32, 0x1F, 1), (0x2714, 0x20, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(LEFT, 0xE6);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x270C, param);
    fx.mem.set32(FOLLOW, 0x18);
    disc_record(fx, D + 0x1740, [0x30, 0x80, 0xA0, 0xFF], 0x4120_0000);
    disc_record(fx, D + 0x2660, [0x30, 0x80, 0xA0, 0], 0x4000_0000);
    disc_record(fx, D + 0x26E0, [0x30, 0x80, 0xA0, 0], 0x4000_0000);
    fx.texture4(sp + 0xB0, 0xE);
    let tex = fx.mem.read_u32(sp + 0xB0);
    ribbon_group(fx, D + 0x70, tex);
    let tex = fx.mem.read_u32(sp + 0xB0);
    ribbon_group(fx, D + 0xA0, tex);
    fx.texture4(sp + 0xB0, 0xF);
    // The particle cloud (+0x1750): 16 particles, half size 20.
    let g = D + 0x1750;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x85);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0xF0), (0x2A, 0), (0x24, 0x10), (0x26, 0x10), (0x2C, 0x100), (0x2E, 0x100)]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 1), (0x13C, 1), (0x140, 8), (0x136, 0xA), (0x138, 0xA)]);
    for (k, x) in [(0xAu32, 0x60u8), (9, 0x80), (8, 0x60), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, x);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x1880) {
        let p = D + 16 * k as u32;
        for (c, ra) in [(0u32, 0x01E0_3C7C), (1, 0x01E0_3CB0)] {
            let r = fx.rand_from(D, ra);
            let (_, h) = fx.div(r, 0x14);
            let v = ee::add(fx.mem.f(D + 0x1760 + 4 * c), ee::from_i32(h - 10));
            fx.mem.setf(p + 0x1780 + 4 * c, v);
        }
        let z = fx.mem.read_u32(D + 0x1768);
        fx.mem.write_u32(p + 0x1788, z);
        let a = -fx.mem.i16(D + 0x188A);
        let f = fx.mult(a, k);
        fx.mem.set16(p + 0x178E, f);
        fx.mem.set16(p + 0x178C, 0);
        let t = fx.mem.i16(D + 0x1890) - 1;
        fx.mem.set16(D + 0x188E, t);
        k += 1;
    }
    // Three rings (+0x19F0 + 0x400 k).
    for k in 0..3u32 {
        let g = D + 0x19F0 + 0x400 * k;
        fx.mem.write_u32(g + 4, 0);
        halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
        for (o, x) in [(0xAu32, 0xA0u8), (9, 0x80), (8, 0x30), (0xB, 0x80)] {
            fx.mem.write_u8(g + o, x);
        }
        let tex = fx.mem.read_u32(sp + 0xB0);
        fx.ring_setup(g, tex, 11.0, 13.0, -2.0, 0.0);
    }
    fx.texture5(sp + 0xB0, 0);
    // The spark fountain (+0x540): 256 sparks at +0x570, width 0.5,
    // length 3.
    let g = D + 0x540;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x2000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x4040_0000);
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x20), (0x16, 0x20), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, D + 0x570);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 8);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    for g in [0x15E0u32, 0x1690] {
        let tex = fx.mem.read_u32(sp + 0xB0);
        plate_group(fx, D + g, tex, 0);
    }
    // The rings' transforms start at scales 0, 0.5 and 1.
    for (x, s) in [(0x18A0u32, 0u32), (0x1910, 0x3F00_0000), (0x1980, 0x3F80_0000)] {
        for c in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + c, s);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    node(fx, 0x2710, D + 0x2610);
    node(fx, 0x2714, D + 0x2690);
    for out in [D + 0x26F0, D + 0x16F0, D + 0x4F0] {
        let t = fx.target_handle();
        fx.unit_position(t, out);
    }
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x90);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0xA0);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x4E0);
    for k in 0..3u32 {
        let t = fx.target_handle();
        fx.unit_position(t, D + 0x18C0 + 0x70 * k);
        fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x18B0 + 0x70 * k);
    }
    if n(fx) < 0x4F {
        node(fx, 0x2710, D + 0x1590);
        node(fx, 0x2714, D + 0x1640);
    }
    if n(fx) >= 0x4E {
        fx.ribbon_push(D + 0x70, D + 0xD0, D + 0x1590);
        fx.ribbon_push(D + 0xA0, D + 0x2D0, D + 0x1640);
        for p in [0x1590u32, 0x1640] {
            let m = fx.mem.i32(FOLLOW);
            fx.approach_by(D + p, D + 0x26F0, m);
        }
        let m = fx.mem.i32(FOLLOW);
        if m != 0 {
            fx.mem.set32(FOLLOW, m - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 = D + 0x150 (D + 0x70 k in the ring loop),
    // s3 = sp + 0xA0 (k in the ring loop, then 3).
    let regs = [Some(D), Some(OT), Some(D + 0x150), Some(sp + 0xA0)];
    fx.xform_world_saving(D, 0x01E0_40A4, regs);
    fx.xform_billboard_saving(D + 0x1570, 0x01E0_40AC, regs);
    fx.xform_billboard_saving(D + 0x1620, 0x01E0_40B4, regs);
    fx.xform_world_saving(D + 0x4D0, 0x01E0_40BC, regs);
    for k in 0..3u32 {
        fx.xform_world_saving(D + 0x18A0 + 0x70 * k, 0x01E0_40CC, [Some(D), Some(OT), Some(D + 0x70 * k), Some(k)]);
    }
    let regs = [Some(D), Some(OT), Some(D + 0x150), Some(3)];
    fx.xform_billboard_saving(D + 0x25F0, 0x01E0_40E8, regs);
    fx.xform_billboard_saving(D + 0x2670, 0x01E0_40F0, regs);
    fx.xform_billboard_saving(D + 0x16D0, 0x01E0_40F8, regs);
    fx.disc(D + 0x1740, D + 0x16D0);
    // The disc's fp / ra saves (frame 0x140).
    fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_4108)]);
    let m = n(fx);
    if m >= 0xCB {
        fx.step_down(D + 0x1747, 0, 0x10);
    } else {
        fx.mem.write_u8(D + 0x1747, if m % 2 == 0 { 0x80 } else { 0x40 });
    }
    let m = n(fx);
    if m < 0x6E {
        if m == 0x4C {
            for k in [0x26E7u32, 0x2667, 0x169B, 0x15EB] {
                fx.mem.write_u8(D + k, 0x80);
            }
        }
        blib::plate::plates(fx, PLATES, OT, D + 0x15E0, D + 0x25F0);
        blib::plate::plates(fx, PLATES, OT, D + 0x1690, D + 0x2670);
        fx.disc(D + 0x2660, D + 0x25F0);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_41C4)]);
        fx.disc(D + 0x26E0, D + 0x2670);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_41D4)]);
        let m = n(fx);
        if (0xC..0x1C).contains(&m) {
            for k in [0x15EBu32, 0x169B, 0x2667, 0x26E7] {
                fx.step_up(D + k, 0x80, 8);
            }
        } else if (0x1C..0x2C).contains(&m) {
            for k in [0x15EBu32, 0x169B, 0x2667, 0x26E7] {
                fx.step_down(D + k, 0, 8);
            }
        }
    }
    if n(fx) < 0x40 {
        for k in [0x1618u32, 0x16C8] {
            let v = fx.mem.i32(D + k).wrapping_add(0x400);
            fx.mem.set32(D + k, v);
        }
    } else {
        fx.mem.write_u32(D + 0x16C8, 0);
        fx.mem.write_u32(D + 0x1618, 0);
    }
    for k in 0..3u32 {
        let (g, x) = (D + 0x19F0 + 0x400 * k, D + 0x18A0 + 0x70 * k);
        fx.ring_draw(g, x);
        fx.step_down(g + 0xB, 0, 8);
        if fx.approach(x, 2.0, f32::from_bits(0x3DCC_CCCD)) != 0 {
            continue;
        }
        for c in [8u32, 4, 0] {
            fx.mem.write_u32(x + c, 0x3DCC_CCCD);
        }
        if n(fx) < 0xCB {
            fx.mem.write_u8(g + 0xB, 0x80);
        }
    }
    blib::trail::sparks(fx, OT, D + 0x540, D + 0x4D0);
    if n(fx) >= 0xCC {
        fx.step_down(D + 0x54B, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x56C) {
        let p = D + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x57E) < 0 {
            continue;
        }
        let s = f32::from_bits(0x3F8C_CCCD);
        for c in 0..3u32 {
            let x = ee::mul(fx.mem.f(p + 0x570 + 4 * c), s);
            fx.mem.setf(p + 0x570 + 4 * c, x);
        }
        if ee_le(fx.mem.f(p + 0x574), 30.0) {
            continue;
        }
        // Respawned on a flat ring (as `spawn::scatter` with flag 0x1D).
        let f = ee::mul(10.0, fx.mem.f(D + 0x568));
        fx.fix_residue(f, 0x01E0_4400);
        let w = ee::fixsfsi(f);
        let a = fx.rand_from(D, 0x01E0_4408) & 0xFFFF;
        let r = fx.rand_from(D, 0x01E0_4424);
        let (_, h) = fx.div(r, w);
        fx.mem.write_u32(p + 0x574, 0x4080_0000);
        let radius = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
        let c = fx.tcos(a);
        fx.mem.setf(p + 0x578, ee::mul(radius, c));
        let s = fx.tsin(a);
        fx.mem.setf(p + 0x570, ee::mul(radius, s));
        let r = fx.rand_from(D, 0x01E0_4474);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0x57E, h.wrapping_neg());
    }
    if n(fx) >= 0x4F {
        for o in [0x88u32, 0x84, 0x80] {
            fx.mem.write_u32(sp + o, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0x16D0, D + 0x1750, sp + 0x80);
        if n(fx) >= 0xCB {
            fx.step_down(D + 0x175B, 0, 2);
        }
    }
    if n(fx) >= 0x4F {
        fx.ribbon_draw(D + 0x70, D + 0xD0, D);
        fx.ribbon_draw(D + 0xA0, D + 0x2D0, D);
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0xCB {
        2
    } else if m == 0x58 {
        1
    } else if (0x62..0xCB).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
