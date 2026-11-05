//! BMODULE b186 (card 224), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E048C0, data area 0x1ED0000) [F] (helper C6, 31
//! Oct 2026): the stage dims until update 0x100; the quad trail (+0x70)
//! pushed at the attacker's nodes 0x4F (updates 0x3B / 0x45 / 0x4F) and
//! 0xD (0x3C / 0x46 / 0x50), drawn until 0x64 with its cells fading; the
//! object op 0x1F83B0 on 0x32..0x100; from update 0x85 a fountain of 128
//! sprites (+0x13B0) at the target; from 0x7D a disc (+0x1330) at node
//! 0x2E, sparks (+0x1C80) there and the target's afterimage in wavy strips
//! (+0x3A70, `blib::afterimage`) with the unit tinted (0x1F75F0); from
//! 0x81 three rings swelling at the target (+0x2E00 + 0x400 k). Returns 1
//! on update 0x73, 6 on 0x7D..0xFF, 2 on 0x100, 8 after 0x110.
//!
//! Library at its own addresses: `colours4` (0x1E00080), `strips`
//! (0x1E00120), `scatter` (0x1E00790), `trail::sparks` (0x1E00950), `disc`
//! (0x1E016B0), `ring_setup` / `ring_draw` (0x1E01C10 / 0x1E01ED0),
//! `sprites` (0x1E023C0), `quadtrail::push` / `draw` (0x1E03CB0 /
//! 0x1E03EB0).
use crate::b101::unit_tint;
use crate::blib::{afterimage, quadtrail, spawn, trail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const TRAIL_DRAW: u32 = 0x01E0_3EB0;

/// The entry 0x1E048C0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

pub(crate) fn bytes(fx: &mut Fx, at: &[(u32, u8)]) {
    for &(o, v) in at {
        fx.mem.write_u8(D + o, v);
    }
}

pub(crate) fn halves(fx: &mut Fx, at: &[(u32, i32)]) {
    for &(o, v) in at {
        fx.mem.set16(D + o, v);
    }
}

fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

/// A sprite of a fountain (x at `p` + `base`, z at + 8) thrown again: a
/// random direction (rand & 0xFFFF), speed 0.1 (rand % 0x3C - 0x1E)
/// sideways (rand draws with s0 = D at the return addresses `ra`).
pub(crate) fn sprite_throw(fx: &mut Fx, p: u32, base: u32, ra: [u32; 2]) {
    let tenth = f32::from_bits(0x3DCC_CCCD);
    let a = fx.rand_from(D, ra[0]) & 0xFFFF;
    let r = fx.rand_from(D, ra[1]);
    let (_, h) = fx.div(r, 0x3C);
    let s = ee::mul(tenth, ee::from_i32(h - 0x1E));
    let c = fx.tcos(a);
    fx.mem.setf(p + base + 8, ee::mul(s, c));
    let n = fx.tsin(a);
    fx.mem.setf(p + base, ee::mul(s, n));
}

/// The fountain's per-update sprite step (sprite k at `base` + 0x10 k: x
/// +0, y +4, z +8, alpha +0xC, frame +0xD, life +0xE; `count` sprites):
/// y + 1; a fading-in sprite (alpha set) steps up to 0x80 by 8 while y <=
/// 10, else down by 8; below y 20 it spreads by 1.05, at 20 it is thrown
/// again (y 0.1 (rand % 0x3C - 0x1E), alpha flag 1, a random frame).
pub(crate) fn fountain_step(fx: &mut Fx, base: u32, count: u32, ra: [u32; 4]) {
    let mut k = 0i32;
    while k < fx.mem.i16(D + count) {
        let p = D + 0x10 * k as u32 + base;
        if fx.mem.i16(p + 0xE) >= 0 {
            let y = ee::add(fx.mem.f(p + 4), 1.0);
            fx.mem.setf(p + 4, y);
            if fx.mem.read_u8(p + 0xC) != 0 {
                if ee_le(fx.mem.f(p + 4), 10.0) {
                    fx.step_up(p + 0xC, 0x80, 8);
                } else {
                    fx.step_down(p + 0xC, 0, 8);
                }
            }
            if ee_lt(fx.mem.f(p + 4), 20.0) {
                let g = f32::from_bits(0x3F86_6666);
                let z = ee::mul(fx.mem.f(p + 8), g);
                fx.mem.setf(p + 8, z);
                let x = ee::mul(fx.mem.f(p), g);
                fx.mem.setf(p, x);
            }
            if !ee_lt(fx.mem.f(p + 4), 20.0) {
                sprite_throw(fx, p, 0, [ra[0], ra[1]]);
                let r = fx.rand_from(D, ra[2]);
                let (_, h) = fx.div(r, 0x3C);
                fx.mem.setf(p + 4, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h - 0x1E)));
                fx.mem.write_u8(p + 0xC, 1);
                let r = fx.rand_from(D, ra[3]);
                fx.mem.write_u8(p + 0xD, (r & 7) as u8);
            }
        }
        k += 1;
    }
}

/// The fountain's sprites thrown at the start (`count` at `base`): life 1,
/// alpha 0, a random frame, y 0.1 (rand % 0xC8).
pub(crate) fn fountain_init(fx: &mut Fx, base: u32, count: u32, ra: [u32; 4]) {
    let mut k = 0i32;
    while k < fx.mem.i16(D + count) {
        let p = D + 0x10 * k as u32 + base;
        fx.mem.set16(p + 0xE, 1);
        fx.mem.write_u8(p + 0xC, 0);
        let r = fx.rand_from(D, ra[0]);
        fx.mem.write_u8(p + 0xD, (r & 7) as u8);
        sprite_throw(fx, p, 0, [ra[1], ra[2]]);
        let r = fx.rand_from(D, ra[3]);
        let (_, h) = fx.div(r, 0xC8);
        fx.mem.setf(p + 4, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h)));
        k += 1;
    }
}

/// The sparks' per-update step (spark k at `base` + 0x10 k, `count`):
/// position x 1.3; from life `old` a random direction of length 3
/// (0x1043F0 into sp + 0x70), life -(rand % 0xC), alpha 0x80.
pub(crate) fn sparks_step(fx: &mut Fx, sp: u32, base: u32, count: u32, old: i32, ra: u32) {
    let grow = f32::from_bits(0x3FA6_6666);
    let mut k = 0i32;
    while k < fx.mem.i16(D + count) {
        let p = D + 0x10 * k as u32 + base;
        if fx.mem.i16(p + 0xE) >= 0 {
            for o in [0u32, 4, 8] {
                let v = ee::mul(fx.mem.f(p + o), grow);
                fx.mem.setf(p + o, v);
            }
            if fx.mem.i16(p + 0xE) >= old {
                fx.random_dir(sp + 0x70, 3.0);
                for (o, s) in [(4u32, 0x74u32), (8, 0x78), (0, 0x70)] {
                    let v = fx.mem.read_u32(sp + s);
                    fx.mem.write_u32(p + o, v);
                }
                let r = fx.rand_from(D, ra);
                let (_, h) = fx.div(r, 0xC);
                fx.mem.set16(p + 0xE, h.wrapping_neg());
                fx.mem.write_u8(p + 0xC, 0x80);
            }
        }
        k += 1;
    }
}

/// Three rings (`g` + 0x400 k, transforms `x` + 0x70 k) drawn, fading by
/// 8, swelling to 2 by 0.1 and restarting turned at random (rand & 0xFFFF
/// per axis at `ra`) at 0.1, bright again before update `bright`.
pub(crate) fn rings_step(fx: &mut Fx, g: u32, x: u32, n: i32, bright: i32, ra: [u32; 3]) {
    let tenth = f32::from_bits(0x3DCC_CCCD);
    for k in 0..3u32 {
        let (g, x) = (D + g + 0x400 * k, D + x + 0x70 * k);
        fx.ring_draw(g, x);
        fx.step_down(g + 0xB, 0, 8);
        if fx.approach(x, 2.0, tenth) == 0 {
            for o in [8u32, 4, 0] {
                fx.mem.setf(x + o, tenth);
            }
            for (o, ra) in [0x10u32, 0x14, 0x18].into_iter().zip(ra) {
                let r = fx.rand_from(D, ra);
                fx.mem.set32(x + o, r & 0xFFFF);
            }
            if n < bright {
                fx.mem.write_u8(g + 0xB, 0x80);
            }
        }
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3E40);
    for x in [0u32, 0x12C0, 0x1C10, 0x2CB0, 0x2D20, 0x2D90, 0x3A00, 0x1340] {
        fx.xform_init(D + x);
    }
    for (at, n, clear) in [(0x3AA4u32, 0xD, 1), (0x3AA8, 0x4F, 0), (0x3AAC, 0x2E, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x3A98, 0x110);
    fx.mem.set32(D + 0x3A9C, 1);
    fx.mem.set32(D + 0x3AA0, param);
    let rad = crate::b107::target_radius(fx);
    fx.mem.setf(D + 0x3AB8, rad);
    fx.mem.set32(D + 0x3AB4, 0x80);
    // The disc (+0x1330).
    fx.mem.write_u32(D + 0x1330, 0x8_0000);
    bytes(fx, &[(0x1336, 0xC0), (0x1335, 0xA0), (0x1334, 0x60), (0x1337, 0xFF)]);
    fx.mem.write_u32(D + 0x1338, 0x4170_0000);
    halves(fx, &[(0x133C, 0x24), (0x133E, 0)]);
    // The afterimage (+0x3A70).
    fx.mem.write_u32(D + 0x3A70, 0x4_1000);
    bytes(fx, &[(0x3A76, 0xC0), (0x3A75, 0xA0), (0x3A74, 0x60), (0x3A77, 0x80)]);
    fx.mem.write_u32(D + 0x3A80, 0);
    fx.mem.write_u32(D + 0x3A7C, 0x2000);
    halves(fx, &[(0x3A78, 0)]);
    fx.mem.write_u32(D + 0x3A84, 0x4120_0000);
    let t = fx.target_handle();
    halves(fx, &[(0x3A7A, t)]);
    bytes(fx, &[(0x3AB2, 0x80), (0x3AB1, 0x80), (0x3AB0, 0x80), (0x3AB3, 0x80)]);
    fx.texture5(sp + 0xA0, 0);
    // The sparks (+0x1C80).
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x1C80, tex);
    fx.mem.write_u32(D + 0x1C84, 3);
    bytes(fx, &[(0x1C8A, 0xC0), (0x1C89, 0xA0), (0x1C88, 0x60), (0x1C8B, 0)]);
    fx.mem.write_u32(D + 0x1CA0, 0x3ECC_CCCD);
    fx.mem.write_u32(D + 0x1CA4, 0x4040_0000);
    halves(fx, &[(0x1C90, 0x20), (0x1C92, 0x20), (0x1C94, 0x20), (0x1C96, 0x20), (0x1C98, 0x20), (0x1C9A, 0x20), (0x1C9C, 0x80), (0x1C9E, 0x40)]);
    fx.mem.write_u32(D + 0x1C8C, D + 0x1CB0);
    fx.mem.write_u32(D + 0x1CA8, 0x4120_0000);
    halves(fx, &[(0x1CAC, 0x100)]);
    bytes(fx, &[(0x1CAE, 1), (0x1CAF, 0)]);
    spawn::scatter(fx, D + 0x1C80, 0xC, 3.0);
    // The sprite fountain (+0x13B0).
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x13B0, tex);
    fx.mem.write_u32(D + 0x13B4, 0x7000_0003);
    bytes(fx, &[(0x13BA, 0xC0), (0x13B9, 0xA0), (0x13B8, 0x60), (0x13BB, 0x80)]);
    fx.mem.write_u32(D + 0x13BC, 0x4000_0000);
    fx.mem.write_u32(D + 0x13D0, 0);
    fx.mem.write_u32(D + 0x13D4, 0x400);
    fx.mem.write_u32(D + 0x13D8, 0);
    halves(fx, &[(0x13E0, 0), (0x13E2, 0), (0x13E4, 0x20), (0x13E6, 0x20), (0x13E8, 0), (0x13EA, 0), (0x13EC, 0x80), (0x13EE, 0x40)]);
    fx.mem.write_u32(D + 0x13F0, D + 0x1410);
    fx.mem.write_u32(D + 0x13F4, 0);
    fx.mem.write_u32(D + 0x13F8, 0);
    fx.mem.write_u32(D + 0x13FC, 0x4000_0000);
    halves(fx, &[(0x140A, 0x80), (0x140C, 8), (0x140E, 0x64)]);
    fountain_init(fx, 0x1410, 0x140A, [0x01E0_4BAC, 0x01E0_4BC8, 0x01E0_4BE4, 0x01E0_4C34]);
    fx.texture4(sp + 0xA0, 0xF);
    // The quad trail (+0x70).
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x70, tex);
    bytes(fx, &[(0x76, 0x80), (0x75, 0x80), (0x74, 0x80), (0x77, 0x30)]);
    fx.mem.write_u32(D + 0x78, 0x1_9003);
    fx.mem.write_u32(D + 0x7C, D + 0xC0);
    halves(fx, &[(0x80, 0x80), (0x82, 0xC0), (0x84, 0x20), (0x86, 0x20), (0x88, 0x80), (0x8A, 0xC0), (0x8C, 0x80), (0x8E, 0x100)]);
    fx.mem.write_u32(D + 0x90, 0);
    fx.mem.write_u32(D + 0x94, 0x3E99_999A);
    fx.mem.write_u32(D + 0x98, 0);
    fx.mem.write_u32(D + 0xA0, 0x4040_0000);
    fx.mem.write_u32(D + 0xA4, 0x3E4C_CCCD);
    fx.mem.write_u32(D + 0xA8, 0x4053_3333);
    halves(fx, &[(0xB2, 0), (0xB4, 0x18), (0xB6, 0x24), (0xB8, 0), (0xBA, 8), (0xBC, 8), (0xBE, 0)]);
    // Three rings (+0x2E00 + 0x400 k).
    for k in 0..3u32 {
        let g = 0x2E00 + 0x400 * k;
        fx.mem.write_u32(D + g + 4, 0);
        halves(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x80)]);
        bytes(fx, &[(g + 0xA, 0xC0), (g + 9, 0xA0), (g + 8, 0x60), (g + 0xB, 0x80)]);
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(D + g, tex, f32::from_bits(0x413C_CCCD), f32::from_bits(0x4143_3333), 0.0, 0.0);
    }
    for (x, v) in [(0x2CB0u32, 0u32), (0x2D20, 0x3F00_0000), (0x2D90, 0x3F80_0000), (0x1340, 0x4000_0000)] {
        for o in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + o, v);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x3A9C);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x1360);
    node(fx, 0x3AAC, D + 0x1C30);
    if n(fx) < 0x7D {
        node(fx, 0x3AAC, D + 0x12E0);
    }
    for k in 0..3u32 {
        let t = fx.target_handle();
        fx.unit_position(t, D + 0x2CD0 + 0x70 * k);
        let h = fx.self_handle();
        fx.unit_point(1, h, sp + 0x80);
        let t = fx.target_handle();
        fx.unit_point(1, t, sp + 0x90);
        fx.look_angles(sp + 0x80, sp + 0x90, D + 0x2CC0 + 0x70 * k);
    }
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x3A20);
    if matches!(n(fx), 0x3B | 0x45 | 0x4F) {
        node(fx, 0x3AA8, sp + 0x70);
        quadtrail::push(fx, D + 0x70, sp + 0x70);
    }
    if matches!(n(fx), 0x3C | 0x46 | 0x50) {
        node(fx, 0x3AA4, sp + 0x70);
        quadtrail::push(fx, D + 0x70, sp + 0x70);
    }
    // s0 = D, s1 = the OT, s2 = D + 0x70 k (0x150 after the loops), s3 =
    // sp + 0x90 then the transform loop's counter.
    let regs = |s2: u32, s3: u32| [Some(D), Some(OT), Some(s2), Some(s3)];
    fx.xform_world_saving(D, 0x01E0_4F84, regs(D + 0x150, sp + 0x90));
    fx.xform_billboard_saving(D + 0x12C0, 0x01E0_4F8C, regs(D + 0x150, sp + 0x90));
    fx.xform_world_saving(D + 0x1C10, 0x01E0_4F94, regs(D + 0x150, sp + 0x90));
    for k in 0..3u32 {
        fx.xform_world_saving(D + 0x2CB0 + 0x70 * k, 0x01E0_4FA4, regs(D + 0x70 * k, k));
    }
    fx.xform_world_saving(D + 0x3A00, 0x01E0_4FC0, regs(D + 0x150, 3));
    fx.xform_world_saving(D + 0x1340, 0x01E0_4FC8, regs(D + 0x150, 3));
    if n(fx) < 0x100 {
        fx.step_down(D + 0x3AB4, 0x20, 2);
    } else {
        fx.step_up(D + 0x3AB4, 0x80, 4);
    }
    let l = fx.mem.read_u8(D + 0x3AB4);
    fx.set_stage_light(i32::from(l));
    let m = n(fx);
    if (0x32..0x101).contains(&m) {
        let h = fx.self_handle();
        for o in [0xACu32, 0xAD, 0xAE, 0xAB, 0xA8, 0xA9, 0xAA] {
            fx.mem.write_u8(sp + o, 0x80);
        }
        fx.mem.write_u8(sp + 0xAF, 0);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h as u32, 3, 0, sp + 0xA8]);
    }
    if n(fx) >= 0x85 {
        fx.sprites(D + 0x13B0, D + 0x1340);
        if n(fx) >= 0x100 {
            fx.step_down(D + 0x13BB, 0, 8);
        }
        fountain_step(fx, 0x1410, 0x140A, [0x01E0_51A8, 0x01E0_51C4, 0x01E0_5214, 0x01E0_5254]);
    }
    if n(fx) >= 0x7D {
        fx.disc(D + 0x1330, D + 0x12C0);
        fx.step_down(D + 0x1337, 0, 0x10);
    }
    if n(fx) == 0x7D {
        fx.mem.write_u8(D + 0x1C8B, 0x80);
    }
    trail::sparks(fx, OT, D + 0x1C80, D + 0x1C10);
    fx.step_down(D + 0x1C8B, 0, 4);
    sparks_step(fx, sp, 0x1CB0, 0x1CAC, 9, 0x01E0_5378);
    if n(fx) >= 0x81 {
        let m = n(fx);
        rings_step(fx, 0x2E00, 0x2CB0, m, 0x100, [0x01E0_5430, 0x01E0_544C, 0x01E0_5468]);
    }
    if n(fx) < 0x64 {
        quadtrail::draw(fx, TRAIL_DRAW, OT, D + 0x70, D);
        let mut i = 0;
        while i < fx.mem.i16(D + 0xB8) {
            let mut j = 0;
            loop {
                let w = fx.mem.i16(D + 0xBA);
                let lo = fx.mult(i, w);
                if j >= w {
                    break;
                }
                fx.step_down(D + 0xCC + 0x10 * (j + lo) as u32, 0, 3);
                j += 1;
            }
            i += 1;
        }
    }
    if n(fx) >= 0x7D {
        let a = fx.mem.read_u8(D + 0x3A77);
        fx.mem.write_u8(D + 0x3AB3, 0x80u8.wrapping_sub(a));
        afterimage::strips(fx, D + 0x3A70, D + 0x3A20);
        let h = fx.mem.i16(D + 0x3A7A);
        unit_tint(fx, h, D + 0x3AB0);
        if n(fx) >= 0x101 {
            fx.step_down(D + 0x3A77, 0, 8);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x3A9C, m);
    let left = fx.mem.i32(D + 0x3A98) - 1;
    fx.mem.set32(D + 0x3A98, left);
    if m == 0x100 {
        2
    } else if m == 0x73 {
        1
    } else if (0x7D..0x100).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
