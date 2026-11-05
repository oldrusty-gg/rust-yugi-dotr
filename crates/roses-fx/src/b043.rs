//! BMODULE b043 (card 37), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E048C0, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): a glow at the attacker's node 0x14 (disc +0x2340 with a
//! sprite shower +0x23C0 and a quad burst +0x2C90 on it, until update
//! 170); a ribbon trail (+0x70) from that node toward the target from
//! update 38 (1 / 16 of the way per update, then 1 / 15 ...); at the
//! target from update 54 a flashing disc (+0x1420), three widening rings
//! (+0x16D0 + 0x400 k, respawned at random angles when grown to 2) and a
//! spark burst (+0x310, `blib::trail::sparks`) thrown out again as each
//! spark dies; the effects' green / blue channels re-rolled every update.
//! Returns 1 on update 44, 6 on 54..199, 2 on 200, 8 after 216.
//!
//! Library: `colours4` (0x1E00080), `spawn::scatter` (0x1E00120),
//! `trail::sparks` (0x1E002E0) [`crate::blib`], `ring_setup` /
//! `ring_draw` (0x1E01040 / 0x1E01300), `disc` (0x1E017F0), `sprites`
//! (0x1E01D50), `quads_setup` / `quads_draw` (0x1E04150 / 0x1E042B0)
//! [`crate::fxlib`], the ribbon (0x1E03640 / 0x1E03790, [`crate::ribbon`]).
//!
//! b174 (card 318) is the same program with other colours, node and
//! thresholds ([`Variant`], `crate::b174`).
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad corner template (rodata, 0x1E04150's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_5580;

/// b043 / b174's differences.
pub struct Variant {
    /// The node the effect starts from.
    pub node: i32,
    /// Colour bytes (+0, +1, +2 of each group's colour) of the discs, the
    /// ribbon, the sparks, the rings, the sprite shower and the quads.
    pub disc: [u8; 3],
    pub ribbon: [u8; 3],
    pub sparks: [u8; 3],
    pub rings: [u8; 3],
    pub shower: [u8; 3],
    pub quads: [u8; 3],
    /// The colour bytes re-rolled every update: rand % 0x70, rand % 0xA0.
    pub rolled: (u32, u32),
    /// The ribbon starts (pushes from `trail - 1`, draws from `trail`).
    pub trail: i32,
    /// The target's effects start; the update returning 1.
    pub hit: i32,
    pub one: i32,
    /// The step code's offset from b043's (return addresses).
    pub shift: u32,
}

pub const B043: Variant = Variant {
    node: 0x14,
    disc: [0xD0, 0xA0, 0x70],
    ribbon: [0xD0, 0xA0, 0x70],
    sparks: [0xD0, 0xA0, 0x70],
    rings: [0xD0, 0xA0, 0x70],
    shower: [0xD0, 0xA0, 0x70],
    quads: [0xD0, 0xA0, 0x70],
    rolled: (2, 1),
    trail: 0x27,
    hit: 0x36,
    one: 0x2C,
    shift: 0,
};

/// The entry 0x1E048C0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B043)
}

pub fn run(fx: &mut Fx, param: i32, v: &Variant) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param, v) } else { step(fx, sp, v) };
    fx.leave(0xB0);
    ret
}

fn colour(fx: &mut Fx, at: u32, c: [u8; 3]) {
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at, c[0]);
}

fn disc_record(fx: &mut Fx, at: u32, c: [u8; 3], alpha: u8, radius: u32) {
    fx.mem.write_u32(at, 0x8_0000);
    colour(fx, at + 4, c);
    fx.mem.write_u8(at + 7, alpha);
    fx.mem.write_u32(at + 8, radius);
    fx.mem.set16(at + 0xC, 0x24);
    fx.mem.set16(at + 0xE, 0);
}

fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32, v: &Variant) -> i32 {
    fx.bzero(D, 0x3120);
    for x in [0u32, 0x1340, 0x2A0, 0x1580, 0x15F0, 0x1660, 0x22D0, 0x13B0, 0x2350, 0x2C20] {
        fx.xform_init(D + x);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, v.node, 1);
    fx.mem.set32(D + 0x3110, r);
    fx.mem.set32(D + 0x3104, 0xD8);
    fx.mem.set32(D + 0x3108, 1);
    fx.mem.set32(D + 0x310C, param);
    fx.mem.set32(D + 0x3100, 0x10);
    disc_record(fx, D + 0x1420, v.disc, 0xFF, 0x41F0_0000);
    disc_record(fx, D + 0x2340, v.disc, 0, 0x4120_0000);
    fx.texture4(sp + 0xA0, 0xF);
    let tex = fx.mem.read_u32(sp + 0xA0);
    // The ribbon (+0x70): cell (200, 32, 7, 32), width 50.
    let g = D + 0x70;
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 8, 0x8003);
    colour(fx, g + 4, v.ribbon);
    fx.mem.write_u8(g + 7, 0x80);
    halves(fx, g, &[(0x10, 0xC8), (0x12, 0x20), (0x14, 7), (0x16, 0x20), (0x18, 0xC8), (0x1A, 0x20), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, 0x4248_0000);
    halves(fx, g, &[(0x22, 8), (0x24, 0), (0x26, 0x6E), (0x28, 0x20), (0x2A, 0), (0x2C, 1)]);
    // The sparks (+0x310): 256 at +0x340, width 0.5, length 5.
    let g = D + 0x310;
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 3);
    colour(fx, g + 8, v.sparks);
    fx.mem.write_u8(g + 0xB, 0x80);
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x40A0_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x60), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x60), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, D + 0x340);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    // The target disc's shower (+0x1430): 16 points round +0x1440.
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x1430, tex);
    fx.mem.write_u32(D + 0x1434, 0x85);
    for k in [0x1440u32, 0x1444, 0x1448, 0x144C] {
        fx.mem.write_u32(D + k, 0);
    }
    halves(fx, D, &[(0x1458, 0xF0), (0x145A, 0), (0x1454, 0x10), (0x1456, 0x10), (0x145C, 0x100), (0x145E, 0x100)]);
    fx.mem.write_u32(D + 0x143C, 0x41A0_0000);
    halves(fx, D, &[(0x1560, 0x10), (0x156A, 1), (0x156C, 1), (0x1570, 8), (0x1566, 0xA), (0x1568, 0xA)]);
    for k in [0x143Au32, 0x1439, 0x1438] {
        fx.mem.write_u8(D + k, 0x80);
    }
    fx.mem.write_u8(D + 0x143B, 0x20);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x1560) {
        let p = D + 16 * k as u32;
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x14);
        let x = ee::add(fx.mem.f(D + 0x1440), ee::from_i32(h - 0xA));
        fx.mem.setf(p + 0x1460, x);
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x14);
        let y = ee::add(fx.mem.f(D + 0x1444), ee::from_i32(h - 0xA));
        fx.mem.setf(p + 0x1464, y);
        let z = fx.mem.read_u32(D + 0x1448);
        fx.mem.write_u32(p + 0x1468, z);
        let n = fx.mem.i16(D + 0x156A).wrapping_neg();
        let m = fx.mult(n, k);
        fx.mem.set16(p + 0x146E, m);
        fx.mem.set16(p + 0x146C, 0);
        k += 1;
        let n = fx.mem.i16(D + 0x1570) - 1;
        fx.mem.set16(D + 0x156E, n);
    }
    // The rings (+0x16D0 + 0x400 k): radii 11.5 / 12.5.
    for k in 0..3u32 {
        let g = D + 0x16D0 + 0x400 * k;
        fx.mem.write_u32(g + 4, 0);
        halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
        colour(fx, g + 8, v.rings);
        fx.mem.write_u8(g + 0xB, 0x80);
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(g, tex, 11.5, 12.5, 0.0, 0.0);
    }
    // The shower (+0x23C0): 128 sprites at +0x2420, size 1, radius 2.
    let g = D + 0x23C0;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0001);
    colour(fx, g + 8, v.shower);
    fx.mem.write_u8(g + 0xB, 0x80);
    fx.mem.write_u32(g + 0xC, 0x3F80_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0xE4), (0x34, 0xA), (0x36, 0xA), (0x38, 0), (0x3A, 0xE4), (0x3C, 0x100), (0x3E, 0x100)]);
    fx.mem.write_u32(g + 0x40, D + 0x2420);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4000_0000);
    halves(fx, g, &[(0x5A, 0x80), (0x5C, 2), (0x5E, 0)]);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x241A) {
        let p = D + 16 * k as u32;
        shower_spawn(fx, sp, p);
        let r = fx.rand();
        let (_, h) = fx.div(r, 0xC);
        k += 1;
        fx.mem.set16(p + 0x242E, h.wrapping_neg());
        fx.mem.write_u8(p + 0x242C, 0x80);
        fx.mem.write_u8(p + 0x242D, 0);
    }
    // The quad burst (+0x2C90).
    let g = D + 0x2C90;
    fx.mem.write_u32(g + 4, 1);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    colour(fx, g + 8, v.quads);
    fx.mem.write_u8(g + 0xB, 0x20);
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.quads_setup(g, tex, 1.0, QUAD_TEMPLATE);
    for (x, s) in [(0x2C20u32, 0x3DCC_CCCDu32), (0x1580, 0), (0x15F0, 0x3F00_0000), (0x1660, 0x3F80_0000)] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, s);
        }
    }
    0
}

/// A shower sprite (`p` = D + 16 k, fields at +0x2420) thrown in a random
/// direction of length 16 (0x1043F0 into sp + 0x60).
fn shower_spawn(fx: &mut Fx, sp: u32, p: u32) {
    fx.random_dir(sp + 0x60, 16.0);
    for k in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x60 + 4 * k);
        fx.mem.write_u32(p + 0x2420 + 4 * k, w);
    }
}

fn step(fx: &mut Fx, sp: u32, v: &Variant) -> i32 {
    let node = |fx: &mut Fx, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x3110);
        fx.node_position(h, s, out);
    };
    let target = |fx: &mut Fx, out: u32| {
        let t = fx.target_handle();
        fx.unit_position(t, out);
    };
    node(fx, D + 0x2370);
    node(fx, D + 0x22F0);
    node(fx, D + 0x2C40);
    target(fx, D + 0x30F0);
    target(fx, D + 0x13D0);
    target(fx, D + 0x2C0);
    for k in 0..3u32 {
        target(fx, D + 0x15A0 + 0x70 * k);
    }
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x80);
    target(fx, sp + 0x90);
    fx.look_angles(sp + 0x80, sp + 0x90, D + 0x2B0);
    if fx.mem.i32(D + 0x3108) < v.trail {
        node(fx, D + 0x1360);
    }
    if fx.mem.i32(D + 0x3108) >= v.trail - 1 {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x1360);
        let n = fx.mem.i32(D + 0x3100);
        fx.approach_by(D + 0x1360, D + 0x30F0, n);
        let n = fx.mem.i32(D + 0x3100);
        if n != 0 {
            fx.mem.set32(D + 0x3100, n - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x90, s3 = 3 (then the loops').
    let ra = |a: u32| a - v.shift;
    let regs = [Some(D), Some(OT), Some(sp + 0x90), Some(3)];
    fx.xform_world_saving(D, ra(0x01E0_4FDC), regs);
    fx.xform_billboard_saving(D + 0x1340, ra(0x01E0_4FE4), regs);
    fx.xform_world_saving(D + 0x2A0, ra(0x01E0_4FEC), regs);
    for k in 0..3u32 {
        let regs = [Some(D), Some(OT), Some(D + 0x70 * k), Some(k)];
        fx.xform_world_saving(D + 0x1580 + 0x70 * k, ra(0x01E0_4FFC), regs);
    }
    let regs = [Some(D), Some(OT), Some(D + 0x150), Some(3)];
    fx.xform_billboard_saving(D + 0x22D0, ra(0x01E0_5018), regs);
    fx.xform_billboard_saving(D + 0x13B0, ra(0x01E0_5020), regs);
    fx.xform_world_saving(D + 0x2350, ra(0x01E0_5028), regs);
    fx.xform_world_saving(D + 0x2C20, ra(0x01E0_5030), regs);
    let groups = [0x1438u32, 0x2C98, 0x23C8, 0x318, 0x74, 0x2344, 0x1424];
    for (m, at) in [(0x70, v.rolled.0), (0xA0, v.rolled.1)] {
        let r = fx.rand();
        let (_, h) = fx.div(r, m);
        for g in groups {
            fx.mem.set8(D + g + at, h);
        }
    }
    if fx.mem.i32(D + 0x3108) < 0xAA {
        fx.sprites(D + 0x23C0, D + 0x2350);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x241A) {
            let p = D + 16 * k as u32;
            if fx.mem.i16(p + 0x242E) >= 0 {
                let s = f32::from_bits(0x3F66_6666);
                for c in 0..3u32 {
                    let x = ee::mul(fx.mem.f(p + 0x2420 + 4 * c), s);
                    fx.mem.setf(p + 0x2420 + 4 * c, x);
                }
            }
            if fx.mem.i16(p + 0x242E) >= 8 {
                shower_spawn(fx, sp, p);
                fx.mem.set16(p + 0x242E, 0);
                fx.mem.write_u8(p + 0x242C, 0x80);
            }
            k += 1;
        }
        fx.disc(D + 0x2340, D + 0x22D0);
        fx.step_up(D + 0x2347, 0xFF, 0x10);
        fx.quads_draw(D + 0x2C90, D + 0x2C20);
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0x2CF0 + 16 * k + 4 * c;
                let x = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, x);
            }
        }
        if fx.mem.i32(D + 0x3108) >= 0x15 {
            fx.approach(D + 0x2C20, 1.0, f32::from_bits(0x3D4C_CCCD));
        }
    }
    if fx.mem.i32(D + 0x3108) >= v.hit {
        fx.disc(D + 0x1420, D + 0x13B0);
        let n = fx.mem.i32(D + 0x3108);
        if n >= 0xC8 {
            fx.step_down(D + 0x1427, 0, 0x10);
        } else {
            fx.mem.write_u8(D + 0x1427, if n % 2 == 0 { 0xFF } else { 0x80 });
        }
    }
    if fx.mem.i32(D + 0x3108) >= v.hit {
        for k in 0..3u32 {
            let (g, x) = (D + 0x16D0 + 0x400 * k, D + 0x1580 + 0x70 * k);
            fx.ring_draw(g, x);
            fx.step_down(g + 0xB, 0, 8);
            if fx.approach(x, 2.0, f32::from_bits(0x3DCC_CCCD)) != 0 {
                continue;
            }
            for c in [8u32, 4, 0] {
                fx.mem.write_u32(x + c, 0x3DCC_CCCD);
            }
            for c in 0..3u32 {
                let r = fx.rand();
                fx.mem.set32(x + 0x10 + 4 * c, r & 0xFFFF);
            }
            if fx.mem.i32(D + 0x3108) < 0xC8 {
                fx.mem.write_u8(g + 0xB, 0x80);
            }
        }
        blib::trail::sparks(fx, OT, D + 0x310, D + 0x2A0);
        if fx.mem.i32(D + 0x3108) >= 0xC9 {
            fx.step_down(D + 0x31B, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x33C) {
            let p = D + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0x34E) < 0 {
                continue;
            }
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 0x340 + 4 * c), 1.5);
                fx.mem.setf(p + 0x340 + 4 * c, x);
            }
            if fx.mem.i16(p + 0x34E) < 9 {
                continue;
            }
            let r = fx.rand();
            let (_, h) = fx.div(r, 0xA);
            let len = ee::add(1.0, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h)));
            fx.random_dir(sp + 0x70, len);
            for c in [1u32, 2, 0] {
                let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
                fx.mem.write_u32(p + 0x340 + 4 * c, w);
            }
            let r = fx.rand();
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0x34E, h.wrapping_neg());
        }
    }
    if fx.mem.i32(D + 0x3108) >= v.trail {
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
    }
    let n = fx.mem.i32(D + 0x3108) + 1;
    fx.mem.set32(D + 0x3108, n);
    let left = fx.mem.i32(D + 0x3104) - 1;
    fx.mem.set32(D + 0x3104, left);
    if n == 0xC8 {
        2
    } else if n == v.one {
        1
    } else if (v.hit..0xC8).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
