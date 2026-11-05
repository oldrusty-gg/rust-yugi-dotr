//! BMODULE b851 (card 680), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03640, data area 0x1ED0000) [F] (helper C3, 31
//! Oct 2026). A rising sprite fountain (+0x1FC0, 128 sprites thrown out
//! again above 50) at the attacker's node 0x85, its colour (with the
//! disc's, the sparks' and the rings') flickering at random every update;
//! from update 80 two drifting sprite clouds (+0x2890 / +0x3160, 128
//! sprites each, sizes 3 / 5) on the attacker and the target; from update
//! 123 a flickering disc (+0x11F0) on the target, three rings (+0x1350 +
//! 0x400 k) swelling and respawned turned, and a spark burst (+0xE0)
//! thrown out again as each dies; the stage light down to 0x20 until
//! update 359. Returns 1 on update 113, 6 on 123..359, 2 on 360, 8 after
//! 375.
//!
//! Library: `spawn::scatter` (0x1E00120), `trail::sparks` (0x1E002E0)
//! [`crate::blib`]; `ring_setup` / `ring_draw` (0x1E01040 / 0x1E01300),
//! `disc` (0x1E017F0), `sprites` (0x1E01D50) [`crate::fxlib`].
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const TENTH: u32 = 0x3DCC_CCCD;

/// What b851 and its twin b145 (card 113, [`crate::b145`]) differ in [F].
pub(crate) struct Variant {
    /// b145's sprite groups (else b851's).
    b145: bool,
    /// The data area's size (bzero).
    size: u32,
    /// The counters: +0 updates left, +4 update, +8 param, +0xC..+0x14
    /// the node slots, +0x18 the stage light.
    ctl: u32,
    /// The three nodes asked for (the first cleared).
    nodes: [i32; 3],
    /// How far the step's calls sit after b851's (return addresses).
    shift: u32,
    /// The disc call's return address.
    disc_ra: u32,
}

const B851: Variant = Variant { b145: false, size: 0x39E0, ctl: 0x39C0, nodes: [0x1D, 0x85, 0x84], shift: 0, disc_ra: 0x01E0_4578 };
pub(crate) const B145: Variant = Variant { b145: true, size: 0x4240, ctl: 0x4220, nodes: [0x15, 0x16, 0x14], shift: 0x110, disc_ra: 0x01E0_47B0 };

/// The entry 0x1E03640.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B851)
}

pub(crate) fn run(fx: &mut Fx, param: i32, v: &Variant) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param, v) } else { step(fx, sp, v) };
    fx.leave(0xB0);
    ret
}

pub(crate) fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

pub(crate) fn tenth_of(h: i32) -> f32 {
    ee::mul(f32::from_bits(TENTH), ee::from_i32(h))
}

/// A sprite at `p` placed on a flat ring: angle rand, radius (rand % `m`
/// - `m` / 2) / 10 (x +0 sin, z +8 cos).
pub(crate) fn ring_place(fx: &mut Fx, p: u32, m: i32) {
    let a = fx.rand() & 0xFFFF;
    let r = fx.rand();
    let (_, h) = fx.div(r, m);
    let radius = tenth_of(h - m / 2);
    let c = fx.tcos(a);
    fx.mem.setf(p + 8, ee::mul(radius, c));
    let s = fx.tsin(a);
    fx.mem.setf(p, ee::mul(radius, s));
}

/// A drifting sprite cloud (`g`: count +0x5A, sprites +0x60 + 16 k) set
/// off: each alive (+0xE = 1), clear, frame rand % `frames`, on a ring of
/// radius 3, height (rand % 400 - 200) / 10.
pub(crate) fn cloud_init(fx: &mut Fx, g: u32, frames: i32) {
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = g + 0x60 + 16 * k as u32;
        fx.mem.set16(p + 0xE, 1);
        fx.mem.write_u8(p + 0xC, 0);
        let r = fx.rand();
        fx.mem.write_u8(p + 0xD, (r % frames) as u8);
        ring_place(fx, p, 0x3C);
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x190);
        fx.mem.setf(p + 4, tenth_of(h - 0xC8));
        k += 1;
    }
}

fn cloud_group(fx: &mut Fx, g: u32, tex: u32, size: u32) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0xC, size);
    fx.mem.write_u32(g + 0x20, 0);
    fx.mem.write_u32(g + 0x24, 0x400);
    fx.mem.write_u32(g + 0x28, 0);
    halves(fx, g, &[(0x30, 0), (0x32, 0x30), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0x30), (0x3C, 0x100), (0x3E, 0x100)]);
    fx.mem.write_u32(g + 0x40, g + 0x60);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4000_0000);
    halves(fx, g, &[(0x5A, 0x80), (0x5C, 8), (0x5E, 0x64)]);
    cloud_init(fx, g, 8);
}

/// A cloud's sprites stepped: rising 1, fading in while above 0 (out
/// below), spreading 1.05 below 20, respawned at -20 + (rand % 60 - 30)
/// / 10 above it (frame rand % `frames`).
pub(crate) fn cloud_step(fx: &mut Fx, g: u32, frames: i32) {
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = g + 0x60 + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        let y = ee::add(fx.mem.f(p + 4), 1.0);
        fx.mem.setf(p + 4, y);
        if fx.mem.read_u8(p + 0xC) != 0 {
            if ee_le(fx.mem.f(p + 4), 0.0) {
                fx.step_up(p + 0xC, 0x80, 8);
            } else {
                fx.step_down(p + 0xC, 0, 8);
            }
        }
        let grow = f32::from_bits(0x3F86_6666);
        if ee_lt(fx.mem.f(p + 4), 20.0) {
            for c in [8u32, 0] {
                let v = ee::mul(fx.mem.f(p + c), grow);
                fx.mem.setf(p + c, v);
            }
        }
        if ee_lt(fx.mem.f(p + 4), 20.0) {
            continue;
        }
        ring_place(fx, p, 0x3C);
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x3C);
        fx.mem.setf(p + 4, ee::add(-20.0, tenth_of(h - 0x1E)));
        fx.mem.write_u8(p + 0xC, 1);
        let r = fx.rand();
        fx.mem.write_u8(p + 0xD, (r % frames) as u8);
    }
}

/// Three rings (`g` + 0x400 k, through `x` + 0x70 k) drawn, faded, and
/// when grown to 2 restarted at 0.1 turned at random (bright again before
/// update `until`) [F].
pub(crate) fn rings_step(fx: &mut Fx, g: u32, x: u32, n: u32, until: i32) {
    for k in 0..3u32 {
        let (g, x) = (g + 0x400 * k, x + 0x70 * k);
        fx.ring_draw(g, x);
        fx.step_down(g + 0xB, 0, 8);
        if fx.approach(x, 2.0, f32::from_bits(TENTH)) != 0 {
            continue;
        }
        for o in [8u32, 4, 0] {
            fx.mem.write_u32(x + o, TENTH);
        }
        for o in [0x10u32, 0x14, 0x18] {
            let r = fx.rand();
            fx.mem.set32(x + o, r & 0xFFFF);
        }
        if fx.mem.i32(n) < until {
            fx.mem.write_u8(g + 0xB, 0x80);
        }
    }
}

/// b851's sprite groups set up (sp + 0xA0 holding texture (4, 0xF)).
fn groups(fx: &mut Fx, sp: u32) {
    // The fountain (+0x1FC0): 128 sprites at +0x2020, size 2.
    let g = D + 0x1FC0;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0001);
    for k in [0xAu32, 9, 8] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 0xB, 0);
    fx.mem.write_u32(g + 0xC, 0x4000_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0xE4), (0x34, 0xA), (0x36, 0xA), (0x38, 0), (0x3A, 0xE4), (0x3C, 0x100), (0x3E, 0x100)]);
    fx.mem.write_u32(g + 0x40, D + 0x2020);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4000_0000);
    halves(fx, g, &[(0x5A, 0x80), (0x5C, 2), (0x5E, 0)]);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x201A) {
        let p = D + 0x2020 + 16 * k as u32;
        fx.mem.set16(p + 0xE, 0);
        fx.mem.write_u8(p + 0xC, 0);
        fx.mem.write_u8(p + 0xD, 0);
        ring_place(fx, p, 0x28);
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x1F4);
        fx.mem.setf(p + 4, ee::mul(f32::from_bits(TENTH), ee::neg(ee::from_i32(h))));
        k += 1;
    }
    fx.texture5(sp + 0xA0, 0);
    let tex = fx.mem.read_u32(sp + 0xA0);
    cloud_group(fx, D + 0x2890, tex, 0x4020_0000);
    let tex = fx.mem.read_u32(sp + 0xA0);
    cloud_group(fx, D + 0x3160, tex, 0x4060_0000);
}

fn init(fx: &mut Fx, sp: u32, param: i32, v: &Variant) -> i32 {
    fx.bzero(D, v.size);
    for x in [0u32, 0x1110, 0x70, 0x1200, 0x1270, 0x12E0, 0x1180, 0x1F50, 0x2820, 0x30F0] {
        fx.xform_init(D + x);
    }
    let c = D + v.ctl;
    for (k, clear) in [(0u32, 1), (1, 0), (2, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, v.nodes[k as usize], clear);
        fx.mem.set32(c + 0xC + 4 * k, r);
    }
    fx.mem.set32(c, 0x178);
    fx.mem.set32(c + 4, 1);
    fx.mem.set32(c + 8, param);
    fx.mem.write_u8(c + 0x18, 0x80);
    // The disc (+0x11F0), radius 30.
    let g = D + 0x11F0;
    fx.mem.write_u32(g, 0x8_0000);
    for (k, x) in [(6u32, 0x80u8), (5, 0x80), (4, 0x80), (7, 0xFF)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 8, 0x41F0_0000);
    fx.mem.set16(g + 0xC, 0x24);
    fx.mem.set16(g + 0xE, 0);
    fx.texture4(sp + 0xA0, 0xF);
    // The spark burst (+0xE0): 256 sparks at +0x110, width 0.2, length 10.
    let g = D + 0xE0;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 3);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0x20, 0x3E4C_CCCD);
    fx.mem.write_u32(g + 0x24, 0x4120_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x60), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x60), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, D + 0x110);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    for k in 0..3u32 {
        let g = D + 0x1350 + 0x400 * k;
        fx.mem.write_u32(g + 4, 0);
        halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
        for (o, x) in [(0xAu32, 0xD0u8), (9, 0x70), (8, 0xA0), (0xB, 0x80)] {
            fx.mem.write_u8(g + o, x);
        }
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(g, tex, 11.5, 12.5, 0.0, 0.0);
    }
    if v.b145 {
        crate::b145::groups(fx, sp);
    } else {
        groups(fx, sp);
    }
    for (x, s) in [(0x1200u32, 0u32), (0x1270, 0x3F00_0000), (0x12E0, 0x3F80_0000), (0x2820, 0x4040_0000), (0x30F0, 0x40A0_0000)] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, s);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32, v: &Variant) -> i32 {
    let c = D + v.ctl;
    let n = |fx: &mut Fx| fx.mem.i32(c + 4);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(c + at);
        fx.node_position(h, s, out);
    };
    node(fx, 0x10, sp + 0x80);
    node(fx, 0x14, sp + 0x90);
    node(fx, 0x10, D + 0x1F70);
    fx.look_angles(sp + 0x80, sp + 0x90, D + 0x1F60);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x2840);
    fx.mem.write_u32(D + 0x3100, 0x8000);
    for out in [0x3110u32, 0x11A0, 0x90, 0x1220, 0x1290, 0x1300] {
        let t = fx.target_handle();
        fx.unit_point(1, t, D + out);
    }
    // s0 = D, s1 = the OT, s2 = D + 0x150, s3 = 3 (after the unit loop),
    // then the loop's.
    let regs = [Some(D), Some(OT), Some(D + 0x150), Some(3)];
    let ra = |a: u32| a + v.shift;
    fx.xform_world_saving(D, ra(0x01E0_3E54), regs);
    fx.xform_billboard_saving(D + 0x1110, ra(0x01E0_3E5C), regs);
    fx.xform_world_saving(D + 0x70, ra(0x01E0_3E64), regs);
    for k in 0..3u32 {
        fx.xform_world_saving(D + 0x1200 + 0x70 * k, ra(0x01E0_3E74), [Some(D), Some(OT), Some(D + 0x70 * k), Some(k)]);
    }
    fx.xform_billboard_saving(D + 0x1180, ra(0x01E0_3E90), regs);
    fx.xform_world_saving(D + 0x1F50, ra(0x01E0_3E98), regs);
    fx.xform_world_saving(D + 0x2820, ra(0x01E0_3EA0), regs);
    fx.xform_world_saving(D + 0x30F0, ra(0x01E0_3EA8), regs);
    if n(fx) >= 0x168 {
        fx.step_up(c + 0x18, 0x80, 4);
    } else {
        fx.step_down(c + 0x18, 0x20, 4);
    }
    let light = fx.mem.u8(c + 0x18);
    fx.set_stage_light(light);
    // The flicker: the fountain's (b145: the first cloud's), the sparks',
    // the rings' and the disc's colour.
    let first = if v.b145 { 0x2898u32 } else { 0x1FC8 };
    for (b, a) in [(2u32, 0x01E0_3EF4), (1, 0x01E0_3F24), (0, 0x01E0_3F54)] {
        let r = fx.rand_from(D, ra(a));
        let x = (r % 128) as u8;
        for g in [first, 0xE8, 0x1B58, 0x1758, 0x1358, 0x11F4] {
            fx.mem.write_u8(D + g + b, x);
        }
    }
    fx.sprites(D + 0x1FC0, D + 0x1F50);
    if n(fx) >= 0x32 {
        fx.step_up(D + 0x1FCB, 0x80, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x201A) {
        let p = D + 0x2020 + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        let y = ee::add(fx.mem.f(p + 4), 3.0);
        fx.mem.setf(p + 4, y);
        fx.step_down(p + 0xC, 0, 8);
        if ee_lt(fx.mem.f(p + 4), 50.0) {
            let s = f32::from_bits(0x3F99_999A);
            for c in [8u32, 0] {
                let v = ee::mul(fx.mem.f(p + c), s);
                fx.mem.setf(p + c, v);
            }
        }
        if ee_lt(fx.mem.f(p + 4), 50.0) {
            continue;
        }
        ring_place(fx, p, 0x28);
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x1E);
        fx.mem.setf(p + 4, ee::mul(f32::from_bits(TENTH), ee::neg(ee::from_i32(h))));
        fx.mem.write_u8(p + 0xC, 0x80);
        if v.b145 {
            let r = fx.rand();
            fx.mem.write_u8(p + 0xD, (r % 16) as u8);
        }
    }
    if n(fx) >= 0x50 && v.b145 {
        crate::b145::clouds_step(fx, sp, c + 4);
    } else if n(fx) >= 0x50 {
        fx.sprites(D + 0x2890, D + 0x2820);
        if n(fx) >= 0x168 {
            fx.step_down(D + 0x289B, 0, 8);
        }
        cloud_step(fx, D + 0x2890, 8);
        fx.sprites(D + 0x3160, D + 0x30F0);
        if n(fx) >= 0x168 {
            fx.step_down(D + 0x316B, 0, 8);
        }
        cloud_step(fx, D + 0x3160, 8);
    }
    if n(fx) >= 0x7B {
        fx.disc(D + 0x11F0, D + 0x1180);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, u64::from(v.disc_ra))]);
        let m = n(fx);
        if m >= 0x168 {
            fx.step_down(D + 0x11F7, 0, 0x10);
        } else {
            fx.mem.write_u8(D + 0x11F7, if m % 2 == 0 { 0xFF } else { 0x80 });
        }
    }
    if n(fx) >= 0x7B {
        rings_step(fx, D + 0x1350, D + 0x1200, c + 4, 0x168);
        blib::trail::sparks(fx, OT, D + 0xE0, D + 0x70);
        if n(fx) >= 0x169 {
            fx.step_down(D + 0xEB, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x10C) {
            let p = D + 0x110 + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) < 0 {
                continue;
            }
            let s = f32::from_bits(0x3FA6_6666);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 4 * c), s);
                fx.mem.setf(p + 4 * c, x);
            }
            if fx.mem.i16(p + 0xE) < 0x11 {
                continue;
            }
            fx.random_dir(sp + 0x70, 1.0);
            for c in [1u32, 2, 0] {
                let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
                fx.mem.write_u32(p + 4 * c, w);
            }
            let r = fx.rand();
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0xE, h.wrapping_neg());
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(c + 4, m);
    let left = fx.mem.i32(c) - 1;
    fx.mem.set32(c, left);
    if m == 0x168 {
        2
    } else if m == 0x71 {
        1
    } else if (0x7B..0x168).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
