//! BMODULE b221 (card 322), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03D10, data area 0x1ED0000) [F] (helper C5, 31
//! Oct 2026). Every update the colours flicker (three random bytes into
//! nine groups). A quad burst (+0x28B0) grows at node 0x33 from update 36;
//! from 107 a ribbon (+0x70) trails a point moving from node 0x33 to the
//! target (16 steps) while a glowing line (+0x2A0, `blib::glow`) runs
//! from node 0x33 to the target until 170, with plates (+0x13F0) at node
//! 0x33 until 170 and a disc there (+0x2830); from 123 at the target a
//! blinking disc (+0x14A0), three rings (+0x1750..) growing to 2 and
//! re-spun, a spark burst (+0x350) thrown out again, and a ring (+0x23C0)
//! from 108. Returns 1 on update 113, 6 on 123..223, 2 on 224, 8 after
//! 260.
//!
//! b395 (card 381) is the same program with other nodes (0xF / 0x10)
//! and timings ([`Variant`]).
//!
//! Library: `colours4` (0x1E00080), `spawn::scatter` (0x1E00120),
//! `trail::sparks` (0x1E002E0), `ring_setup` / `ring_draw` (0x1E01040 /
//! 0x1E01300), `plate::plates` (0x1E017F0), `disc` (0x1E01D80),
//! `quads_setup` / `quads_draw` (0x1E022E0 / 0x1E02440), `ribbon_push` /
//! `ribbon_draw` (0x1E02A50 / 0x1E02BA0), `glow::line` (0x1E03560).
use crate::b628::halves;
use crate::blib::{self, glow, plate};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PLATES: u32 = 0x01E0_17F0;
/// The quad corner template (rodata, 0x1E022E0's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_4C00;
const TENTH: u32 = 0x3DCC_CCCD;

/// What b395 (card 381, the same program and library at the same
/// addresses) changes [F diff of the two records].
pub struct Variant {
    /// The attacker's nodes (the line's start, the aim).
    nodes: (i32, i32),
    /// Updates (+0x2D24) and the point's steps to the target (+0x2D20).
    left: i32,
    steps: i32,
    /// The blinking disc's flags (+0x14A0), the ribbon's life (+0x96),
    /// the plates' size (+0x13FC / +0x1404).
    disc_flags: u32,
    ribbon_life: i32,
    plate_size: u32,
    /// The ribbon pushes from this update; the point follows the node
    /// before `node_until`; the line runs until `line_end`; the target's
    /// group ends at `end`.
    push_from: i32,
    node_until: i32,
    line_end: i32,
    end: i32,
    /// b221 clears the disc's flag 0x13 at the line's end and fades the
    /// quad burst on even updates; b395 fades it every update and again
    /// once grown.
    b221: bool,
}

pub const B221: Variant = Variant { nodes: (0x33, 0x30), left: 0x104, steps: 0x10, disc_flags: 0, ribbon_life: 0x64, plate_size: 0x3E99_999A, push_from: 0x6B, node_until: 0x6C, line_end: 0xAA, end: 0xE0, b221: true };
pub const B395: Variant = Variant { nodes: (0xF, 0x10), left: 0x160, steps: 0x14, disc_flags: 0x8_0000, ribbon_life: 0x8A, plate_size: 0x3F00_0000, push_from: 0x61, node_until: 0x62, line_end: 0xEC, end: 0x14A, b221: false };

/// The entry 0x1E03D10.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B221)
}

/// b395's entry 0x1E03D10.
pub fn program_b395(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B395)
}

fn run(fx: &mut Fx, param: i32, v: &Variant) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if param >= 0 { init(fx, sp, param, v) } else { step(fx, sp, v) };
    fx.leave(0xA0);
    ret
}

/// The bytes the flicker writes each update (B, G, R of nine groups).
const FLICKER: [[u32; 9]; 3] = [
    [0x14BA, 0x13FA, 0x23CA, 0x28BA, 0x35A, 0x2AA, 0x76, 0x2836, 0x14A6],
    [0x14B9, 0x13F9, 0x23C9, 0x28B9, 0x359, 0x2A9, 0x75, 0x2835, 0x14A5],
    [0x14B8, 0x13F8, 0x23C8, 0x28B8, 0x358, 0x2A8, 0x74, 0x2834, 0x14A4],
];

fn colour(fx: &mut Fx, g: u32, bgra: [u8; 4]) {
    for (k, x) in [(0xAu32, bgra[2]), (9, bgra[1]), (8, bgra[0]), (0xB, bgra[3])] {
        fx.mem.write_u8(g + k, x);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32, v: &Variant) -> i32 {
    fx.bzero(D, 0x2D40);
    for x in [0u32, 0x1380, 0x2E0, 0x1600, 0x1670, 0x16E0, 0x2350, 0x27C0, 0x1430, 0x2840] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2D30u32, v.nodes.0, 1), (0x2D38, v.nodes.1, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2D24, v.left);
    fx.mem.set32(D + 0x2D28, 1);
    fx.mem.set32(D + 0x2D2C, param);
    fx.mem.set32(D + 0x2D20, v.steps);
    // The discs (+0x14A0: size 30, +0x2830: 4).
    for (g, f, a, size) in [(0x14A0u32, v.disc_flags, 0xFFu8, 0x41F0_0000u32), (0x2830, 0x8_0000, 0, 0x4080_0000)] {
        let g = D + g;
        fx.mem.write_u32(g, f);
        for (k, x) in [(6u32, 0xA0u8), (5, 0xD0), (4, 0x70), (7, a)] {
            fx.mem.write_u8(g + k, x);
        }
        fx.mem.write_u32(g + 8, size);
        halves(fx, g, &[(0xC, 0x24), (0xE, 0)]);
    }
    fx.texture4(sp + 0x90, 0xF);
    let tex = |fx: &mut Fx| fx.mem.read_u32(sp + 0x90);
    // The ribbon (+0x70, points +0xA0).
    let g = D + 0x70;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 8, 0x8003);
    for (k, x) in [(6u32, 0xA0u8), (5, 0xD0), (4, 0x70), (7, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    halves(fx, g, &[(0x10, 0xC8), (0x12, 0x20), (0x14, 7), (0x16, 0x20), (0x18, 0xC8), (0x1A, 0x20), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    halves(fx, g, &[(0x22, 8), (0x24, 0), (0x26, v.ribbon_life), (0x28, 0x20), (0x2A, 0), (0x2C, 1)]);
    // The glowing line (+0x2A0): 28 pieces, width 20.
    let g = D + 0x2A0;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x400_0000);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    halves(fx, g, &[(0x10, 0xC8), (0x12, 0x20), (0x14, 7), (0x16, 0x20)]);
    colour(fx, g, [0x70, 0xD0, 0xA0, 0x80]);
    halves(fx, g, &[(0x28, 0x1C), (0x2C, -3), (0x2A, 0x28)]);
    fx.mem.write_u32(g + 0x20, 0);
    fx.mem.write_u32(g + 0x24, 0);
    // The spark burst (+0x350): 256 sparks at +0x380, width 0.5, length 3.
    let g = D + 0x350;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 3);
    colour(fx, g, [0x70, 0xD0, 0xA0, 0x80]);
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x4040_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x60), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x60), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, D + 0x380);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    // The plates (+0x13F0).
    let g = D + 0x13F0;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x8_0000);
    colour(fx, g, [0x70, 0xD0, 0xA0, 0]);
    for (k, x) in [(0xCu32, v.plate_size), (0x10, 0x4100_0000), (0x14, v.plate_size), (0x18, 0x4080_0000)] {
        fx.mem.write_u32(g + k, x);
    }
    halves(fx, g, &[(0x20, 0xB0), (0x22, 0x40), (0x24, 0x20), (0x26, 0x20)]);
    for k in [0x30u32, 0x34, 0x38] {
        fx.mem.write_u32(g + k, 0);
    }
    // A particle cloud (+0x14B0, 16 particles; never drawn).
    let g = D + 0x14B0;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x85);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0xF0), (0x2A, 0), (0x24, 0x10), (0x26, 0x10), (0x2C, 0x100), (0x2E, 0x100)]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 1), (0x13C, 1), (0x140, 8), (0x136, 0xA), (0x138, 0xA)]);
    colour(fx, g, [0x80, 0x80, 0x80, 0x20]);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x130) {
        let p = g + 0x30 + 16 * k as u32;
        for (c, ra) in [(0u32, 0x01E0_40D4u32), (4, 0x01E0_4108)] {
            let r = fx.rand_from(D, ra);
            let (_, h) = fx.div(r, 0x14);
            let v = ee::add(fx.mem.f(g + 0x10 + c), ee::from_i32(h - 0xA));
            fx.mem.setf(p + c, v);
        }
        let z = fx.mem.read_u32(g + 0x18);
        fx.mem.write_u32(p + 8, z);
        let a = -fx.mem.i16(g + 0x13A);
        let f = fx.mult(a, k);
        fx.mem.set16(p + 0xE, f);
        fx.mem.set16(p + 0xC, 0);
        let t = fx.mem.i16(g + 0x140) - 1;
        fx.mem.set16(g + 0x13E, t);
        k += 1;
    }
    // Rings: three at +0x1750 (radii 11.5 / 12.5), one at +0x23C0 (2.7 / 3).
    for (g, inner, outer) in [(0x1750u32, 0x4138_0000u32, 0x4148_0000u32), (0x1B50, 0x4138_0000, 0x4148_0000), (0x1F50, 0x4138_0000, 0x4148_0000), (0x23C0, 0x402C_CCCD, 0x4040_0000)] {
        let g = D + g;
        fx.mem.write_u32(g + 4, 0);
        halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
        colour(fx, g, [0x70, 0xD0, 0xA0, 0x80]);
        let t = tex(fx);
        fx.ring_setup(g, t, f32::from_bits(inner), f32::from_bits(outer), 0.0, 0.0);
    }
    // The quad burst (+0x28B0): size 0.5, 64 quads turned at random.
    let g = D + 0x28B0;
    fx.mem.write_u32(g + 4, 1);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    colour(fx, g, [0x70, 0xD0, 0xA0, 0x20]);
    let t = tex(fx);
    fx.quads_setup(g, t, 0.5, QUAD_TEMPLATE);
    for k in 0..64u32 {
        let p = D + 0x2910 + 16 * k;
        let r = fx.rand_from(D, 0x01E0_42BC);
        fx.mem.set32(p, r % 0x4000 - 0x2000);
        let r = fx.rand_from(D, 0x01E0_42DC);
        fx.mem.set32(p + 4, r & 0xFFFF);
        fx.mem.write_u32(p + 8, 0);
    }
    for (x, v) in [(0x2840u32, TENTH), (0x2350, 0), (0x1600, 0), (0x1670, 0x3F00_0000), (0x16E0, 0x3F80_0000)] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, v);
        }
    }
    0
}

fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

fn step(fx: &mut Fx, sp: u32, v: &Variant) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2D28);
    node(fx, 0x2D30, sp + 0x70);
    node(fx, 0x2D38, sp + 0x80);
    node(fx, 0x2D30, D + 0x2860);
    fx.look_angles(sp + 0x70, sp + 0x80, D + 0x2850);
    for (from, to) in [(0x2860u32, 0x2370u32), (0x2850, 0x2360)] {
        for k in 0..4u32 {
            let w = fx.mem.read_u32(D + from + 4 * k);
            fx.mem.write_u32(D + to + 4 * k, w);
        }
    }
    node(fx, 0x2D30, D + 0x27E0);
    for out in [0x2D10u32, 0x1450, 0x300, 0x1620, 0x1690, 0x1700] {
        let t = fx.target_handle();
        fx.unit_position(t, D + out);
    }
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x70);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x80);
    fx.look_angles(sp + 0x70, sp + 0x80, D + 0x2F0);
    if n(fx) < v.node_until {
        node(fx, 0x2D30, D + 0x13A0);
    }
    if n(fx) >= v.push_from {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x13A0);
        let left = fx.mem.i32(D + 0x2D20);
        fx.approach_by(D + 0x13A0, D + 0x2D10, left);
        let left = fx.mem.i32(D + 0x2D20);
        if left != 0 {
            fx.mem.set32(D + 0x2D20, left - 1);
        }
    }
    fx.xform_world(D);
    fx.xform_billboard(D + 0x1380);
    for x in [0x2E0u32, 0x1600, 0x1670, 0x16E0, 0x2350] {
        fx.xform_world(D + x);
    }
    fx.xform_billboard(D + 0x27C0);
    fx.xform_billboard(D + 0x1430);
    fx.xform_world(D + 0x2840);
    for (row, (m, ra)) in FLICKER.iter().zip([(0xA0, 0x01E0_4580u32), (0xD0, 0x01E0_45BC), (0x70, 0x01E0_45F8)]) {
        let r = fx.rand_from(D, ra);
        let (_, h) = fx.div(r, m);
        for &a in row {
            fx.mem.write_u8(D + a, h as u8);
        }
    }
    if n(fx) >= 0x7B {
        fx.disc(D + 0x14A0, D + 0x1430);
        let m = n(fx);
        if m >= v.end {
            fx.step_down(D + 0x14A7, 0, 0x10);
        } else {
            fx.mem.write_u8(D + 0x14A7, if m % 2 == 0 { 0xFF } else { 0x80 });
        }
    }
    if (v.push_from..v.line_end).contains(&n(fx)) {
        plate::plates(fx, PLATES, OT, D + 0x13F0, D + 0x27C0);
        fx.step_up(D + 0x13FB, 0x80, 0x10);
    }
    if v.b221 && n(fx) == v.line_end {
        let f = fx.mem.read_u32(D + 0x2830) & 0xFFF7_FFFF;
        fx.mem.write_u32(D + 0x2830, f);
    }
    fx.disc(D + 0x2830, D + 0x27C0);
    fx.quads_draw(D + 0x28B0, D + 0x2840);
    let m = n(fx);
    if (0x24..0x5A).contains(&m) {
        if fx.approach(D + 0x2840, 1.0, f32::from_bits(0x3D4C_CCCD)) != 0 {
            fx.step_up(D + 0x2837, 0x80, 0x10);
        } else if !v.b221 {
            fx.step_down(D + 0x28BB, 0, 1);
            fx.step_down(D + 0x2837, 0, 0x10);
        }
    } else if m >= v.push_from {
        if m == v.push_from {
            fx.mem.write_u8(D + 0x28BB, 0x20);
        }
        if !v.b221 || n(fx) % 2 == 0 {
            fx.step_down(D + 0x28BB, 0, 1);
        }
        fx.step_up(D + 0x2837, 0x80, 0x10);
    }
    if n(fx) >= 0x7B {
        for k in 0..3u32 {
            let (g, x) = (D + 0x1750 + 0x400 * k, D + 0x1600 + 0x70 * k);
            fx.ring_draw(g, x);
            fx.step_down(g + 0xB, 0, 8);
            if fx.approach(x, 2.0, f32::from_bits(TENTH)) != 0 {
                continue;
            }
            for c in [8u32, 4, 0] {
                fx.mem.write_u32(x + c, TENTH);
            }
            for (c, ra) in [(0x10u32, 0x01E0_485Cu32), (0x14, 0x01E0_4878), (0x18, 0x01E0_4894)] {
                let r = fx.rand_from(D, ra);
                fx.mem.set32(x + c, r & 0xFFFF);
            }
            if n(fx) < v.end {
                fx.mem.write_u8(g + 0xB, 0x80);
            }
        }
        blib::trail::sparks(fx, OT, D + 0x350, D + 0x2E0);
        if n(fx) >= v.end + 1 {
            fx.step_down(D + 0x35B, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x37C) {
            let p = D + 0x380 + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) < 0 {
                continue;
            }
            let s = f32::from_bits(0x3FB3_3333);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 4 * c), s);
                fx.mem.setf(p + 4 * c, x);
            }
            if fx.mem.i16(p + 0xE) < 9 {
                continue;
            }
            fx.random_dir(sp + 0x60, 1.0);
            for (o, s) in [(4u32, 0x64u32), (8, 0x68), (0, 0x60)] {
                let w = fx.mem.read_u32(sp + s);
                fx.mem.write_u32(p + o, w);
            }
            let r = fx.rand_from(D, 0x01E0_4994);
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0xE, h.wrapping_neg());
        }
    }
    if n(fx) >= v.node_until {
        fx.ring_draw(D + 0x23C0, D + 0x2350);
        if fx.approach(D + 0x2350, 1.0, f32::from_bits(0x3E4C_CCCD)) == 0 {
            for c in [8u32, 4, 0] {
                fx.mem.write_u32(D + 0x2350 + c, 0);
            }
        }
        if n(fx) == v.line_end {
            fx.mem.write_u8(D + 0x77, 0x80);
            let mut k = 0;
            while k < fx.mem.i16(D + 0x98) + 1 {
                fx.mem.write_u8(D + 0xAC + 16 * k as u32, 0x80);
                k += 1;
            }
        }
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
        if n(fx) < v.line_end {
            glow::line(fx, OT, D + 0x2A0, D, D + 0x27E0, D + 0x2D10);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2D28, m);
    let left = fx.mem.i32(D + 0x2D24) - 1;
    fx.mem.set32(D + 0x2D24, left);
    if m == v.end {
        2
    } else if m == 0x71 {
        1
    } else if (0x7B..v.end).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
