//! BMODULE b026 (card 212), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E047A0, data area 0x1ED0000) [F] (helper C9, 31
//! Oct 2026): a swath (+0x70, `swath`) between the attacker's nodes 0x35
//! / 0x33 drawn on four stretches of updates; from 0x40 a streamer
//! (+0x1A0) pushed every third update at node 0xC or 0x14 (at random);
//! on 0x18..0x33 a frame quad and, from 0x29, four falling streak sets
//! (+0xB20 + 0x410 k, `rain`) and a particle cloud (+0x1ED0) at node 0x35;
//! a full-window tint (+0x2040), point quads (+0xA50), a quad group
//! (+0x550) and a square cloud (+0x1B60) on 0xB1..0xBA and again from
//! 0xC4 (recoloured and the squares redrawn on 0xBB). The stage dims
//! (0x1F7F10) until 0xC4. Returns 1 on update 0xA7, 6 on 0xB1..0xC3, 3
//! on 0xC4, 8 after 0xFA.
//!
//! Library at its own addresses: `rain::colours2` (0x1E00080), `colours4`
//! (0x1E00100), the tint (0x1E001A0), `pointquad` setup / draw / squares
//! (0x1E00300 / 0x1E00640 / 0x1E01220), `particle_quads` (0x1E01800), the
//! frame quad (0x1E02390), `swath` push / draw (0x1E024B0 / 0x1E02600),
//! `quads_setup` / `quads_draw` (0x1E02E00 / 0x1E02F60), `rain` setup /
//! draw (0x1E03570 / 0x1E037C0), `streamer` push / draw (0x1E03D20 /
//! 0x1E03E00).
use crate::b752::{bw, colour, hw, rand_f};
use crate::blib::{pointquad, rain, streamer, swath};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SETUP: u32 = 0x01E0_0300;
const TEMPLATE: u32 = 0x01E0_5780;
const RAIN_SETUP: u32 = 0x01E0_3570;
const RAIN_DRAW: u32 = 0x01E0_37C0;
const STREAMER: u32 = 0x01E0_3E00;
const LEFT: u32 = D + 0x2020;
const N: u32 = D + 0x2024;
const NODES: u32 = D + 0x202C;
const LIGHT: u32 = D + 0x2044;

/// The entry 0x1E047A0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x90);
    ret
}

fn node(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(NODES + 4 * k);
    fx.node_position(h, s, out);
}

/// The square cloud's 60 points (+0x1B80 + 12 k) by its flag 0x1D.
fn square_points(fx: &mut Fx, ras: [u32; 6]) {
    for k in 0..0x3Cu32 {
        let p = D + 0xC * k;
        if bit(fx.mem.read_u32(D + 0x1B64), 0x1D) {
            let x = rand_f(fx, ras[0], 0x64, 0x32);
            fx.mem.setf(p + 0x1B80, x);
            let z = rand_f(fx, ras[1], 0x32, 0);
            fx.mem.setf(p + 0x1B88, z);
            let y = rand_f(fx, ras[2], 0x64, 0x32);
            fx.mem.setf(p + 0x1B84, y);
        } else {
            let x = rand_f(fx, ras[3], 0x32, 0x19);
            fx.mem.setf(p + 0x1B80, x);
            let y = rand_f(fx, ras[4], 0x19, 0);
            fx.mem.setf(p + 0x1B84, y);
            let z = rand_f(fx, ras[5], 0x32, 0x19);
            fx.mem.setf(p + 0x1B88, z);
        }
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x23D0);
    for x in [0u32, 0x4E0, 0xAB0, 0x1E60] {
        fx.xform_init(D + x);
    }
    for (k, n) in [0x35, 0x36, 0x33, 0x14, 0xC].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set32(NODES + 4 * k as u32, r);
    }
    fx.mem.set32(LEFT, 0xFA);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x2028, param);
    fx.mem.write_u8(LIGHT, 0x80);
    for o in [0x1E50u32, 0x1E54, 0x1E58] {
        fx.mem.write_u32(D + o, 0x3F82_8F5C);
    }
    colour(fx, 0x2040, [0x70, 0xA0, 0xD0, 0x80]);
    // Four streak sets (+0xB20 + 0x410 k).
    for k in 0..4u32 {
        let g = D + 0xB20 + 0x410 * k;
        fx.mem.write_u32(g, 0x7000_0001);
        colour(fx, g + 4 - D, [0xD0, 0xA0, 0x70, 0x80]);
        rain::setup(fx, RAIN_SETUP, g, 0x40, 10.0);
    }
    fx.texture4(sp + 0x80, 0xF);
    // The square cloud (+0x1B60).
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(D + 0x1B60, tex);
    fx.mem.write_u32(D + 0x1B64, 0x2000_0001);
    fx.mem.write_u32(D + 0x1B6C, 0x3F00_0000);
    square_points(fx, [0x01E0_4964, 0x01E0_4990, 0x01E0_49B8, 0x01E0_49EC, 0x01E0_4A18, 0x01E0_4A40]);
    hw(fx, &[(0x1B70, 0xB0), (0x1B72, 0x40), (0x1B74, 0x20), (0x1B76, 0x20)]);
    colour(fx, 0x1B68, [0x70, 0xA0, 0xD0, 0x80]);
    // The swath (+0x70, points +0x2048 / +0x2208) at nodes 0x35 / 0x36.
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(D + 0x70, tex);
    fx.mem.write_u32(D + 0x74, 1);
    hw(fx, &[(0x7C, 8), (0x180, 0x8E), (0x182, 0x54), (0x184, 0x20), (0x186, 1)]);
    colour(fx, 0x78, [0x90, 0x60, 0x20, 0x80]);
    fx.mem.write_u32(D + 0x190, D + 0x2048);
    fx.mem.write_u32(D + 0x194, D + 0x2208);
    node(fx, 0, sp + 0x50);
    node(fx, 1, sp + 0x60);
    for k in 0..8u32 {
        for c in 0..3u32 {
            let a = fx.mem.read_u32(sp + 0x50 + 4 * c);
            fx.mem.write_u32(D + 0x80 + 0x10 * k + 4 * c, a);
            let b = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(D + 0x100 + 0x10 * k + 4 * c, b);
        }
    }
    hw(fx, &[(0x7E, 0)]);
    // The quad group (+0x550).
    fx.mem.write_u32(D + 0x554, 0x8_0001);
    hw(fx, &[(0x560, 0x8E), (0x562, 0), (0x564, 0x22), (0x566, 0x40)]);
    colour(fx, 0x558, [0x70, 0xA0, 0xD0, 0x20]);
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.quads_setup(D + 0x550, tex, f32::from_bits(0x3F4C_CCCD), TEMPLATE);
    // The point quads (+0xA50, points +0x9B0).
    fx.mem.write_u32(D + 0xA88, 0x8_0001);
    hw(fx, &[(0xA50, 0x80), (0xA52, 0), (0xA54, 0x20), (0xA56, 0x20), (0xA58, 0xB0), (0xA5A, 0x20), (0xA5C, 0x100), (0xA5E, 0x100)]);
    fx.mem.write_u32(D + 0xA80, 0x3F80_0000);
    colour(fx, 0xA84, [0x70, 0xA0, 0xD0, 0x60]);
    hw(fx, &[(0xA98, 0), (0xA9A, 0), (0xA9C, 0), (0xA9E, 1), (0xAA0, 0), (0xAA2, 0x3E7)]);
    fx.mem.set32(D + 0x9B8, 1);
    let tex = fx.mem.read_u32(sp + 0x80);
    pointquad::setup(fx, SETUP, D + 0xA50, D + 0x9B0, tex, 10.0);
    // The streamer (+0x1A0, points +0x1E0).
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(D + 0x1A0, tex);
    bw(fx, &[(0x1A6, 0x80), (0x1A5, 0x60), (0x1A4, 0x30), (0x1A7, 0x30)]);
    fx.mem.write_u32(D + 0x1A8, 0x9003);
    fx.mem.write_u32(D + 0x1AC, 0x4040_0000);
    hw(fx, &[(0x1B0, 0x80), (0x1B2, 0xC0), (0x1B4, 0x20), (0x1B6, 0x20), (0x1B8, 0x80), (0x1BA, 0xC0), (0x1BC, 0x80), (0x1BE, 0x100)]);
    fx.mem.write_u32(D + 0x1D0, 0x3E99_999A);
    hw(fx, &[(0x1D4, 0), (0x1D6, 0x18), (0x1D8, 0x30), (0x1DA, 0), (0x1DC, 8)]);
    // Texture 4 / 8: the a1 = 8 of the delay slot's +0x1DC store.
    fx.texture4(sp + 0x80, 8);
    // The particle cloud (+0x1ED0).
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(D + 0x1ED0, tex);
    fx.mem.write_u32(D + 0x1ED4, 0x8_0005);
    for o in [0x1EE0u32, 0x1EE4, 0x1EE8, 0x1EEC] {
        fx.mem.write_u32(D + o, 0);
    }
    hw(fx, &[(0x1EF8, 0), (0x1EFA, 0x3C), (0x1EF4, 0xA), (0x1EF6, 0xA), (0x1EFC, 0x80), (0x1EFE, 0x80)]);
    fx.mem.write_u32(D + 0x1EDC, 0x3F33_3333);
    hw(fx, &[(0x2000, 0x10), (0x200A, 1), (0x200C, 0), (0x2010, 6), (0x2006, -0xA), (0x2008, -6)]);
    bw(fx, &[(0x1EDA, 0x80), (0x1ED9, 0x80), (0x1ED8, 0x80), (0x1EDB, 0x80)]);
    for k in 0..0x10i32 {
        let p = D + 0x10 * k as u32;
        let x = ee::add(fx.mem.f(D + 0x1EE0), rand_f(fx, 0x01E0_4DB8, 0xA, 0));
        fx.mem.setf(p + 0x1F00, x);
        let y = ee::add(fx.mem.f(D + 0x1EE4), rand_f(fx, 0x01E0_4DE8, 6, 0));
        fx.mem.setf(p + 0x1F04, y);
        let z = fx.mem.read_u32(D + 0x1EE8);
        fx.mem.write_u32(p + 0x1F08, z);
        let a = fx.mem.i16(D + 0x200A).wrapping_neg();
        let f = fx.mult(a, k);
        fx.mem.set16(p + 0x1F0E, f);
        fx.mem.set16(p + 0x1F0C, 0);
        let c = fx.mem.i16(D + 0x2010) - 1;
        fx.mem.set16(D + 0x200E, c);
    }
    for o in [0xAB8u32, 0xAB4, 0xAB0] {
        fx.mem.write_u32(D + o, 0x3E99_999A);
    }
    for o in [0x1E68u32, 0x1E64, 0x1E60] {
        fx.mem.write_u32(D + o, 0x3ECC_CCCD);
    }
    0
}

/// `n` in [a, b) or [c, d).
fn within2(n: i32, a: i32, b: i32, c: i32, d: i32) -> bool {
    (a..b).contains(&n) || (c..d).contains(&n)
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    node(fx, 0, sp + 0x50);
    node(fx, 2, sp + 0x60);
    swath::push(fx, D + 0x70, sp + 0x50, sp + 0x60);
    if n(fx) < 0x35 {
        let h = fx.self_handle();
        fx.unit_point(1, h, D + 0xAD0);
        node(fx, 0, D + 0x1E80);
    }
    if within2(n(fx), 0x35, 0xB2, 0xBC, 0xC5) {
        node(fx, 0, D + 0x500);
        node(fx, 0, D + 0xAD0);
    }
    let m = n(fx);
    if m >= 0x40 && m.wrapping_rem(3) == 0 {
        let r = fx.rand_from(D, 0x01E0_4F98);
        let k = if r.wrapping_rem(2) == 0 { 4 } else { 3 };
        node(fx, k, sp + 0x70);
        streamer::push(fx, D + 0x1A0, D + 0x1E0, sp + 0x70);
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x60, s3 the runner's 0.
    let regs = [Some(D), Some(OT), Some(sp + 0x60), Some(0)];
    fx.xform_world_saving(D, 0x01E0_5008, regs);
    for (x, ra) in [(0x4E0u32, 0x01E0_5010u32), (0xAB0, 0x01E0_5018), (0x1E60, 0x01E0_5020)] {
        fx.xform_billboard_saving(D + x, ra, regs);
    }
    if n(fx) < 0xC4 {
        fx.step_down(LIGHT, 0x20, 4);
    } else {
        fx.step_up(LIGHT, 0x80, 4);
    }
    let l = fx.mem.read_u8(LIGHT);
    fx.set_stage_light(i32::from(l));
    if within2(n(fx), 0xB1, 0xBB, 0xC4, i32::MAX) {
        crate::b262::tint(fx, D + 0x2040);
        fx.step_down(D + 0x2043, 0, 0x10);
    }
    let m = n(fx);
    if within2(m, 0x1B, 0x35, 0x6A, 0x97) || within2(m, 0xAE, 0xB7, 0xC3, 0xCF) {
        swath::draw(fx, D + 0x70, D);
    }
    if (0x18..0x34).contains(&n(fx)) {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
        if n(fx) >= 0x29 {
            for k in 0..4u32 {
                rain::draw(fx, RAIN_DRAW, OT, D + 0xB20 + 0x410 * k, D + 0xAB0, -3.0);
            }
            for o in [0x78u32, 0x74, 0x70] {
                fx.mem.write_u32(sp + o, 0);
            }
            fx.particle_quads(D + 0x1E60, D + 0x1ED0, sp + 0x70);
            fx.step_down(D + 0x1EDB, 0, 4);
        }
    }
    if n(fx) == 0x34 {
        for o in [0xAB8u32, 0xAB4, 0xAB0] {
            fx.mem.write_u32(D + o, 0x3E4C_CCCD);
        }
    }
    if (0x40..0xB1).contains(&n(fx)) {
        streamer::draw(fx, STREAMER, OT, D + 0x1A0, D + 0x1E0, D);
        let mut j = 0u32;
        while (j as i32) < fx.mem.i16(D + 0x1DA) {
            fx.step_down(D + 0x1EC + 0x10 * j, 0, 2);
            j += 1;
        }
        if n(fx) >= 0x95 {
            fx.step_down(D + 0x1A7, 0, 8);
        }
    }
    if within2(n(fx), 0xB1, 0xBB, 0xC4, i32::MAX) {
        pointquad::draw(fx, OT, D + 0xA50, D + 0x9B0, D + 0x4E0);
        fx.quads_draw(D + 0x550, D + 0x4E0);
        if ee_le(fx.mem.f(D + 0x4E0), 1.0) {
            let d = f32::from_bits(0x3E4C_CCCD);
            for o in [0x4E0u32, 0x4E4, 0x4E8] {
                let s = ee::add(fx.mem.f(D + o), d);
                fx.mem.setf(D + o, s);
            }
        } else {
            let d = f32::from_bits(0x3D4C_CCCD);
            for o in [0x4E0u32, 0x4E4, 0x4E8] {
                let s = ee::add(fx.mem.f(D + o), d);
                fx.mem.setf(D + o, s);
            }
            fx.step_down(D + 0x55B, 0, 6);
            fx.step_down(D + 0xA87, 0, 0xC);
        }
        for k in 0..0x40u32 {
            for c in 0..3u32 {
                let a = D + 0x5B0 + 0x10 * k + 4 * c;
                let w = fx.mem.read_u32(a).wrapping_add(0x80);
                fx.mem.write_u32(a, w);
            }
        }
    }
    if within2(n(fx), 0xB1, 0xBB, 0xC5, i32::MAX) {
        pointquad::squares(fx, OT, D + 0x1B60, D + 0xAB0, D + 0x1E50);
        fx.step_down(D + 0x1B6B, 0, 0xA);
    }
    if n(fx) == 0xBB {
        for o in [0x4E8u32, 0x4E4, 0x4E0] {
            fx.mem.write_u32(D + o, 0x3F80_0000);
        }
        colour(fx, 0x2040, [0xD0, 0x70, 0x70, 0x80]);
        fx.mem.write_u32(D + 0x1B6C, 0x3F00_0000);
        square_points(fx, [0x01E0_5434, 0x01E0_5460, 0x01E0_5488, 0x01E0_54BC, 0x01E0_54E8, 0x01E0_5510]);
        colour(fx, 0x1B68, [0xD0, 0x70, 0x70, 0x80]);
        colour(fx, 0xA84, [0xD0, 0x70, 0x70, 0x60]);
        colour(fx, 0x558, [0xD0, 0x70, 0x70, 0x20]);
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0xC4 {
        3
    } else if m == 0xA7 {
        1
    } else if (0xB1..0xC4).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
