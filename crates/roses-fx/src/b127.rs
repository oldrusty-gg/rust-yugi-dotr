//! BMODULE b127 (card 39), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04740, data area 0x1ED0000) [F] (helper C9, 31
//! Oct 2026): b752's program shape with spinning motes. On 0xC..0x1A a
//! frame quad; on 0x1A..0x1E four waves (+0x1AC0 + 0x30 k, `wave::draw`)
//! and a facing quad (+0x1090, `facing::quad`) at the attacker, the other
//! light (0x1F7EF0) 8 on 0x1A and 0x80 on 0x1E; on 0x1B the units' flag
//! 0x1F8470(1) and the update count held for 16 updates (+0x1B88), then
//! 0x1F8470(0); the object op 0x1F83B0 on 0x1F..0x4A; a full-window tint
//! (+0x1CA4) on 0x1B..0x2A; from 0x35 a particle cloud (+0x640) at the
//! attacker's node 0x2D (looking at node 0x2C) and from 0x36 60 spinning
//! motes (+0x10F0, b637's `motes`), point quads (+0x5E0) and a quad group
//! (+0xE0); on 0x36..0x4A two rings (+0x800 / +0xC00) and a cloud
//! (+0x1820). The stage dims until 0x1F, brightens on 0x4A..0x82.
//! Returns 1 on update 0x2C, 6 on 0x36..0x49, 3 on 0x4A, 8 after 0x82.
//!
//! Library at its own addresses: `colours4` (0x1E00080), the tint
//! (0x1E00120), `pointquad` setup / draw (0x1E00280 / 0x1E005C0),
//! `facing::quad` (0x1E011A0), `ring_setup` / `ring_draw` (0x1E01A30 /
//! 0x1E01CF0), `particle_quads` (0x1E021E0), the motes' setup / draw
//! (0x1E02D70 / 0x1E02F30), `wave::draw` (0x1E036B0), the frame quad
//! (0x1E03EB0), `quads_setup` / `quads_draw` (0x1E03FD0 / 0x1E04130).
use crate::blib::spin::motes;
use crate::b752::{bw, cloud8, colour, hw};
use crate::blib::{facing, pointquad, wave};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SETUP: u32 = 0x01E0_0280;
const TEMPLATE: u32 = 0x01E0_5700;
const LEFT: u32 = D + 0x1B80;
const N: u32 = D + 0x1B84;
const HOLD: u32 = D + 0x1B88;
const FLAGS: u32 = D + 0x1B90;
const NODES: u32 = D + 0x1B94;
const LIGHT: u32 = D + 0x1CA8;
/// The light byte 0x1F7EF0 sets.
const LIGHT2: u32 = 0x002D_F5B0;
const MOTES: u32 = D + 0x10F0;

/// The entry 0x1E04740.
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

/// 0x1E02D70(g, size) (55388f0a066a): half size +0xC = size / 2; 60
/// motes (+0x3F0 + 0xC k) at rand % 20 out (flag 0x1D: high, else
/// round) at a random angle, their angles +0x30 + 0x10 k random (frame
/// 0x60; rand's s0 = k).
fn motes_setup(fx: &mut Fx, g: u32, size: f32) {
    fx.enter(0x60);
    fx.mem.setf(g + 0xC, ee::div(size, 2.0));
    for k in 0..0x3Cu32 {
        let p = g + 0x3F0 + 0xC * k;
        let q = g + 0x30 + 0x10 * k;
        if bit(fx.mem.read_u32(g + 4), 0x1D) {
            let a = fx.rand_from(k, 0x01E0_2DC8).wrapping_rem(0x1_0000);
            let r = fx.rand_from(k, 0x01E0_2DE4);
            let (_, h) = fx.div(r, 0x14);
            fx.mem.setf(p + 8, ee::from_i32(h));
            let s = fx.tsin(a);
            let v = ee::mul(fx.mem.f(p + 8), s);
            fx.mem.setf(p + 4, v);
            let c = fx.tcos(a);
            let v = ee::mul(fx.mem.f(p + 8), c);
            fx.mem.setf(p, v);
        } else {
            let a = fx.rand_from(k, 0x01E0_2E38).wrapping_rem(0x1_0000);
            let r = fx.rand_from(k, 0x01E0_2E54);
            let (_, h) = fx.div(r, 0x14);
            fx.mem.setf(p + 4, ee::from_i32(h));
            let c = fx.tcos(a);
            let v = ee::mul(fx.mem.f(p + 4), c);
            fx.mem.setf(p + 8, v);
            let s = fx.tsin(a);
            let v = ee::mul(fx.mem.f(p + 4), s);
            fx.mem.setf(p, v);
        }
        for (o, ra) in [(0u32, 0x01E0_2EA8u32), (4, 0x01E0_2EC4), (8, 0x01E0_2EE0)] {
            let r = fx.rand_from(k, ra).wrapping_rem(0x1_0000);
            fx.mem.set32(q + o, r);
        }
    }
    fx.leave(0x60);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1CB0);
    for x in [0u32, 0x70, 0x17B0, 0x790, 0x1000] {
        fx.xform_init(D + x);
    }
    for (k, n) in [0x2D, 0x2C, 0x1B, 0x49].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set32(NODES + 4 * k as u32, r);
    }
    fx.mem.set32(LEFT, 0x82);
    fx.mem.set32(N, 1);
    fx.mem.set32(HOLD, 0x10);
    fx.mem.set32(D + 0x1B8C, param);
    fx.mem.write_u32(FLAGS, 0);
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_4830);
        fx.mem.write_u8(D + 0x1BA4 + k, (r >> 4) as u8);
    }
    colour(fx, 0x1CA4, [0xA0, 0xD0, 0x70, 0x80]);
    fx.mem.write_u8(LIGHT, 0x80);
    fx.texture4(sp + 0x80, 0xF);
    // Four waves (+0x1AC0 + 0x30 k).
    for k in 0..4u32 {
        let g = 0x1AC0 + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0x80);
        fx.mem.write_u32(D + g, tex);
        fx.mem.write_u32(D + g + 0xC, D + 0x1BA4);
        fx.mem.write_u32(D + g + 4, 0x2000_0000);
        hw(fx, &[(g + 0x10, 0xB0), (g + 0x12, 0x40), (g + 0x14, 0x20), (g + 0x16, 0x20)]);
        colour(fx, g + 8, [0x80, 0x80, 0x80, 0xFF]);
        fx.mem.write_u32(D + g + 0x20, 0x3E4C_CCCD);
        fx.mem.write_u32(D + g + 0x24, 0x3CCC_CCCD);
        hw(fx, &[(g + 0x2A, -2 * k as i32), (g + 0x28, 0x100)]);
        fx.mem.write_u8(D + g + 0x2C, (k + 7) as u8);
    }
    // The quad group (+0xE0).
    fx.mem.write_u32(D + 0xE4, 0x8_0001);
    hw(fx, &[(0xF0, 0xB0), (0xF2, 0x60), (0xF4, 0x20), (0xF6, 0x20)]);
    colour(fx, 0xE8, [0xA0, 0xD0, 0x70, 0x20]);
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.quads_setup(D + 0xE0, tex, f32::from_bits(0x404C_CCCD), TEMPLATE);
    // Two rings (+0x800, +0xC00).
    for (g, a, outer) in [(0x800u32, 0x20u8, 0x4133_3333u32), (0xC00, 0x30, 0x414C_CCCD)] {
        fx.mem.write_u32(D + g + 4, 0x2000_0000);
        hw(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x40)]);
        colour(fx, g + 8, [0xA0, 0xD0, 0x70, a]);
        let tex = fx.mem.read_u32(sp + 0x80);
        fx.ring_setup(D + g, tex, 12.0, f32::from_bits(outer), 0.0, 0.0);
    }
    // The point quads (+0x5E0, points +0x540).
    fx.mem.write_u32(D + 0x618, 0x8_0001);
    hw(fx, &[(0x5E0, 0x80), (0x5E2, 0), (0x5E4, 0x20), (0x5E6, 0x20), (0x5E8, 0xB0), (0x5EA, 0x20), (0x5EC, 0x100), (0x5EE, 0x100)]);
    fx.mem.write_u32(D + 0x610, 0x3F80_0000);
    colour(fx, 0x614, [0x40, 0, 0, 0x60]);
    hw(fx, &[(0x628, 0), (0x62A, 0), (0x62C, 0), (0x62E, 1), (0x630, 0), (0x632, 0x3E7)]);
    fx.mem.set32(D + 0x548, 1);
    let tex = fx.mem.read_u32(sp + 0x80);
    pointquad::setup(fx, SETUP, D + 0x5E0, D + 0x540, tex, 40.0);
    // The late cloud (+0x1820), texture 5 / 2.
    fx.texture5(sp + 0x80, 2);
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(D + 0x1820, tex);
    fx.mem.write_u32(D + 0x1824, 7);
    for o in [0x1830u32, 0x1834, 0x1838] {
        fx.mem.write_u32(D + o, 0);
    }
    hw(fx, &[(0x1840, 0), (0x1842, 0), (0x1844, 0x20), (0x1846, 0x20), (0x1848, 0), (0x184A, 0), (0x184C, 0x80), (0x184E, 0x40)]);
    fx.mem.write_u32(D + 0x182C, 0x40C0_0000);
    hw(fx, &[(0x1950, 0x10), (0x195A, 2), (0x195C, 0), (0x1960, 8), (0x1956, 0xA), (0x1958, 0xA)]);
    bw(fx, &[(0x182A, 0x80), (0x1829, 0x80), (0x1828, 0x80), (0x182B, 0x40)]);
    cloud8(fx, D + 0x1820, [0x01E0_4B8C, 0x01E0_4BB8]);
    // The particle cloud (+0x640), texture 5 / 1.
    fx.texture5(sp + 0x80, 1);
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(D + 0x640, tex);
    fx.mem.write_u32(D + 0x644, 0x8_0487);
    for o in [0x650u32, 0x654, 0x658, 0x65C] {
        fx.mem.write_u32(D + o, 0);
    }
    hw(fx, &[(0x668, 0), (0x66A, 0), (0x664, 0x20), (0x666, 0x20), (0x66C, 0x80), (0x66E, 0x40)]);
    fx.mem.write_u32(D + 0x64C, 0x4000_0000);
    hw(fx, &[(0x770, 0x10), (0x77A, 3), (0x77C, 2), (0x780, 8), (0x776, 0x14), (0x778, 0x14)]);
    bw(fx, &[(0x64A, 0x80), (0x649, 0x80), (0x648, 0x80), (0x64B, 0x60)]);
    for k in 0..0x10i32 {
        let p = D + 0x10 * k as u32;
        for (o, ra) in [(0u32, 0x01E0_4CBCu32), (4, 0x01E0_4CF0)] {
            let r = fx.rand_from(D, ra);
            let (_, h) = fx.div(r, 0x14);
            let v = ee::add(fx.mem.f(D + 0x650 + o), ee::from_i32(h - 0xA));
            fx.mem.setf(p + 0x670 + o, v);
        }
        let z = fx.mem.read_u32(D + 0x658);
        fx.mem.write_u32(p + 0x678, z);
        let a = fx.mem.i16(D + 0x77A).wrapping_neg();
        let f = fx.mult(a, k);
        fx.mem.set16(p + 0x67E, f);
        fx.mem.set16(p + 0x67C, 0);
        let c = fx.mem.i16(D + 0x780) - 1;
        fx.mem.set16(D + 0x77E, c);
    }
    // The facing quad (+0x1090, quad +0x1070).
    fx.mem.write_u32(D + 0x10C8, 3);
    hw(fx, &[(0x1090, 0), (0x1092, 0), (0x1094, 0x22), (0x1096, 0x22), (0x1098, 0), (0x109A, 0), (0x109C, 0x80), (0x109E, 0x40)]);
    fx.mem.write_u32(D + 0x10C0, 0x3F80_0000);
    bw(fx, &[(0x10C6, 0x80), (0x10C5, 0x80), (0x10C4, 0x80), (0x10C7, 0)]);
    hw(fx, &[(0x10D8, 0), (0x10DA, 0), (0x10DC, 0), (0x10DE, 1), (0x10E0, 0), (0x10E2, 0x3E7)]);
    let tex = fx.mem.read_u32(sp + 0x80);
    for o in [0x1088u32, 0x1084, 0x1080] {
        fx.mem.write_u32(D + o, 0);
    }
    fx.mem.write_u32(D + 0x1078, 0x4120_0000);
    fx.mem.write_u32(D + 0x1070, tex);
    bw(fx, &[(0x107E, 0x80), (0x107D, 0x80), (0x107C, 0x80), (0x107F, 0x80)]);
    // The motes (+0x10F0).
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(MOTES, tex);
    motes_setup(fx, MOTES, 3.0);
    fx.mem.write_u32(MOTES + 4, 0x4000_0001);
    hw(fx, &[(0x1110, 0), (0x1112, 0), (0x1114, 0x23), (0x1116, 0x23)]);
    bw(fx, &[(0x10FA, 0x80), (0x10F9, 0x80), (0x10F8, 0x80), (0x10FB, 0x80)]);
    for (o, v) in [(0x10u32, 0u32), (0x14, 0x400), (0x18, 0)] {
        fx.mem.write_u32(MOTES + o, v);
    }
    for x in [0x70u32, 0x790] {
        for o in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + o, 0x3DCC_CCCD);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    let looked = n(fx) == 0x35;
    if looked {
        for out in [D + 0x17D0, D + 0x7B0, D + 0x90, sp + 0x50] {
            node(fx, 0, out);
        }
        node(fx, 1, sp + 0x60);
        fx.look_angles(sp + 0x50, sp + 0x60, D + 0x1010);
        for c in 0..3u32 {
            let v = fx.mem.read_u32(sp + 0x50 + 4 * c);
            fx.mem.write_u32(D + 0x1020 + 4 * c, v);
        }
    }
    if n(fx) < 0x35 {
        let h = fx.self_handle();
        fx.unit_point(1, h, D + 0x1020);
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x60 on update 0x35, else the
    // parameter (-1, 64-bit), s3 the runner's 0.
    let regs = [Some(D), Some(OT), Some(if looked { sp + 0x60 } else { 0 }), Some(0)];
    let world = |fx: &mut Fx, x: u32, ra: u32| {
        fx.xform_world_saving(D + x, ra, regs);
        if !looked {
            fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
        }
    };
    let board = |fx: &mut Fx, x: u32, ra: u32| {
        fx.xform_billboard_saving(D + x, ra, regs);
        if !looked {
            fx.saved(sp - 0xF0, &[(0x40, u64::MAX)]);
        }
    };
    world(fx, 0, 0x01E0_4F5C);
    board(fx, 0x70, 0x01E0_4F64);
    board(fx, 0x17B0, 0x01E0_4F6C);
    board(fx, 0x790, 0x01E0_4F74);
    if n(fx) < 0x35 {
        board(fx, 0x1000, 0x01E0_4F8C);
    } else {
        world(fx, 0x1000, 0x01E0_4FA0);
    }
    let l = fx.mem.read_u8(LIGHT);
    fx.set_stage_light(i32::from(l));
    if n(fx) < 0x1F {
        fx.step_down(LIGHT, 0x20, 2);
    }
    if (0x4A..0x83).contains(&n(fx)) {
        fx.step_up(LIGHT, 0x80, 0x10);
    }
    if (0xC..0x1F).contains(&n(fx)) {
        if n(fx) < 0x1B {
            fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
        }
        if n(fx) == 0x1A {
            fx.mem.write_u8(LIGHT2, 8);
        }
        // a3 as 0x103470 leaves it: the byte before its step.
        let mut a3 = 0u32;
        if n(fx) >= 0x1A {
            for k in 0..4u32 {
                let g = D + 0x1AC0 + 0x30 * k;
                if fx.mem.i16(g + 0x2A) >= 0xA {
                    fx.mem.set16(g + 0x2A, 0);
                    fx.mem.write_u8(g + 0xB, 0x80);
                }
                wave::draw(fx, OT, g, D + 0x1000);
                a3 = u32::from(fx.mem.read_u8(g + 0xB));
                fx.step_down(g + 0xB, 0, 0x10);
            }
        }
        if n(fx) == 0x1B {
            // a1 = 0x1F (bit's), a2 = 0x10 / a3 as the waves' last
            // step_down left them [run].
            if !bit(fx.mem.read_u32(FLAGS), 0x1F) {
                crate::b258::unit_flags(fx, 1, [0x1F, 0x10, a3]);
                // 0x1F8470's frame (s0 = D, ra): the facing quad's
                // missing-texture TEX0 / TEX1 [run].
                fx.saved(sp - 0x20, &[(0, u64::from(D)), (0x10, 0x01E0_50D4)]);
                let f = fx.mem.read_u32(FLAGS) | 0x8000_0000;
                fx.mem.write_u32(FLAGS, f);
            } else {
                let h = fx.mem.i32(HOLD) - 1;
                fx.mem.set32(HOLD, h);
                if fx.mem.i32(HOLD) == 0 {
                    let f = fx.mem.read_u32(FLAGS) & 0x7FFF_FFFF;
                    fx.mem.write_u32(FLAGS, f);
                    crate::b258::unit_flags(fx, 0, [0x1F, 0x10, a3]);
                    fx.saved(sp - 0x20, &[(0, u64::from(D)), (0x10, 0x01E0_5118)]);
                }
            }
        }
        facing::quad(fx, OT, D + 0x1090, D + 0x1070, D + 0x1000);
        fx.step_up(D + 0x10C7, 0x80, 4);
        if n(fx) == 0x1E {
            fx.mem.write_u8(LIGHT2, 0x80);
        }
    }
    if (0x1F..0x4B).contains(&n(fx)) {
        let h = fx.self_handle();
        for o in [0x8Cu32, 0x8D, 0x8E, 0x8B, 0x88, 0x89, 0x8A] {
            fx.mem.write_u8(sp + o, 0x80);
        }
        fx.mem.write_u8(sp + 0x8F, 0);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h as u32, 3, 0, sp + 0x88]);
    }
    if (0x1B..0x2B).contains(&n(fx)) {
        crate::b262::tint(fx, D + 0x1CA4);
        if !bit(fx.mem.read_u32(FLAGS), 0x1F) {
            fx.step_down(D + 0x1CA7, 0, 8);
        }
    }
    if n(fx) >= 0x35 {
        fx.mem.write_u32(sp + 0x78, 0x3F82_8F5C);
        fx.mem.write_u32(sp + 0x70, 0x3F82_8F5C);
        fx.mem.write_u32(sp + 0x74, 0x3F86_6666);
        fx.particle_quads(D + 0x17B0, D + 0x640, sp + 0x70);
        if n(fx) >= 0x46 {
            fx.step_down(D + 0x64B, 0, 4);
        }
    }
    if n(fx) >= 0x36 {
        for o in [0x78u32, 0x74, 0x70] {
            fx.mem.write_u32(sp + o, 0x3F8C_CCCD);
        }
        motes(fx, OT, MOTES, D + 0x1000, sp + 0x70);
        fx.step_down(D + 0x10FB, 0, 4);
        let r = fx.rand_from(D, 0x01E0_52B4);
        fx.mem.write_u8(D + 0x10F8, r.wrapping_rem(0x80) as u8);
        for k in 0..0x3Cu32 {
            for o in [0u32, 4, 8] {
                let a = D + 0x1120 + 0x10 * k + o;
                let w = fx.mem.read_u32(a).wrapping_add(0x1000);
                fx.mem.write_u32(a, w);
            }
        }
        pointquad::draw(fx, OT, D + 0x5E0, D + 0x540, D + 0x70);
        fx.quads_draw(D + 0xE0, D + 0x70);
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
    }
    if (0x36..0x4B).contains(&n(fx)) {
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
        for o in [0x78u32, 0x74, 0x70] {
            fx.mem.write_u32(sp + o, 0);
        }
        fx.particle_quads(D + 0x17B0, D + 0x1820, sp + 0x70);
        if n(fx) >= 0x43 {
            fx.step_down(D + 0x182B, 0, 8);
        }
    }
    if !bit(fx.mem.read_u32(FLAGS), 0x1F) {
        let m = n(fx) + 1;
        fx.mem.set32(N, m);
        let left = fx.mem.i32(LEFT) - 1;
        fx.mem.set32(LEFT, left);
    }
    let m = n(fx);
    if m == 0x4A {
        3
    } else if m == 0x2C {
        1
    } else if (0x36..0x4A).contains(&m) {
        6
    } else if fx.mem.i32(LEFT) > 0 {
        0
    } else {
        8
    }
}
