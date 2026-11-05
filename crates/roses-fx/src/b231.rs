//! BMODULE b231 (card 323), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E045D0, data area 0x1ED0000) [F] (helper C10, 31
//! Oct 2026): b452's library at the same addresses with another program.
//! Three strands (+0x2A0 + 0x30 k, flags 0, `strand::draw`) between the
//! attacker's nodes 0x4A / 0x41 / 0x38 before update 0xD3 with discs
//! (+0x480 + 0x10 k) at them, flickering with a disc (+0x590) at node 8
//! until 0xE4; a sprite fountain (+0xAF0) there before 0xE4, a quad group
//! (+0x2C20, random angles, turning) at node 0xC from 0x28; a frame quad on
//! 0x28..0x86 and from 0xE4; from 0xE4 a second fountain (+0x1B50) and a
//! ribbon (+0x70) from node 8 closing on the target; from 0xFE three rings
//! (+0x31D0 + 0x400 k) swelling at the target, a disc (+0xA70) and a quad
//! group (+0x610). The stage dims until 0x19B. Returns 1 on update 0xF4,
//! 6 on 0xFE..0x19A, 2 on 0x19B, 8 after 0x1AC.
//!
//! Library at b452's addresses (`colours4` 0x1E00080, the frame quad
//! 0x1E00120, `sprites_spawn` / `sprites` 0x1E00240 / 0x1E003D0,
//! `ring_setup` / `ring_draw` 0x1E01CC0 / 0x1E01F80, `disc` 0x1E02470,
//! `quads_setup` / `quads_draw` 0x1E029D0 / 0x1E02B30, `ribbon` push /
//! draw 0x1E03140 / 0x1E03290, `strand::draw` 0x1E03C50); the quad
//! template at 0x1E05800.
use crate::b142::fstep_up;
use crate::b540::{place, ring_throw, rmod};
use crate::blib::strand;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const STRAND: u32 = 0x01E0_3C50;
const TEMPLATE: u32 = 0x01E0_5800;
const LEFT: u32 = D + 0x3DD0;
const N: u32 = D + 0x3DD4;
const LIGHT: u32 = D + 0x3DDC;
const TENTH: u32 = 0x3DCC_CCCD;

/// The entry 0x1E045D0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn bytes(fx: &mut Fx, at: &[(u32, u8)]) {
    for &(o, v) in at {
        fx.mem.write_u8(D + o, v);
    }
}

fn halves(fx: &mut Fx, at: &[(u32, i32)]) {
    for &(o, v) in at {
        fx.mem.set16(D + o, v);
    }
}

fn words(fx: &mut Fx, at: &[(u32, u32)]) {
    for &(o, v) in at {
        fx.mem.write_u32(D + o, v);
    }
}

/// Node slot at `at` (+0x3DF8: 8, +0x3DFC: 0xC, +0x3E00: 0x4A, +0x3E04:
/// 0x41, +0x3E08: 0x38).
fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

/// `disc` (0x1E02470)'s prologue saves of the entry's fp (the runner's 0)
/// and ra at entry sp - 0xB0 / - 0xA0 (as b452) [run].
fn disc_saved(fx: &mut Fx, sp: u32, ra: u32) {
    fx.saved(sp - 0xB0, &[(0, 0), (0x10, u64::from(ra))]);
}

fn disc_init(fx: &mut Fx, g: u32, r: u32) {
    words(fx, &[(g, 0x8_0000)]);
    bytes(fx, &[(g + 6, 0xE0), (g + 5, 0x50), (g + 4, 0x40), (g + 7, 0)]);
    words(fx, &[(g + 8, r)]);
    halves(fx, &[(g + 0xC, 0x24), (g + 0xE, 0)]);
}

fn sprite_group(fx: &mut Fx, g: u32, tex: u32, spread: u32) {
    words(fx, &[(g, tex), (g + 4, 0x7000_0003)]);
    bytes(fx, &[(g + 0xA, 0x80), (g + 9, 0x80), (g + 8, 0x80), (g + 0xB, 0x80)]);
    words(fx, &[(g + 0xC, 0x3F80_0000), (g + 0x20, 0), (g + 0x24, 0), (g + 0x28, 0)]);
    halves(fx, &[(g + 0x30, 0x20), (g + 0x32, 0x20), (g + 0x34, 0x20), (g + 0x36, 0x20), (g + 0x38, 0x20), (g + 0x3A, 0x20), (g + 0x3C, 0x80), (g + 0x3E, 0x40)]);
    words(fx, &[(g + 0x40, D + g + 0x60), (g + 0x44, 0), (g + 0x48, 0), (g + 0x4C, spread)]);
    halves(fx, &[(g + 0x5A, 0x100), (g + 0x5C, 1), (g + 0x5E, 0)]);
}

/// A fountain B particle `p` re-thrown on a ring (spread +0x1B9C) at
/// height 0.1 x -(rand % 40 - 20) (rand's frame: the entry's s0 = D).
fn throw(fx: &mut Fx, p: u32, ras: [u32; 4]) {
    let (a, rad) = ring_throw(fx, D, D + 0x1B9C, [ras[0], ras[1], ras[2]]);
    let h = rmod(fx, D, ras[3], 0x28);
    fx.mem.setf(p + 4, ee::mul(f32::from_bits(TENTH), ee::neg(ee::from_i32(h - 0x14))));
    place(fx, p, a, rad);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3E10);
    for x in [0u32, 0x4B0, 0xA80, 0x520, 0x5A0, 0x2BB0] {
        fx.xform_init(D + x);
    }
    for k in 0..3u32 {
        fx.xform_init(D + 0x330 + 0x70 * k);
        fx.xform_init(D + 0x3080 + 0x70 * k);
    }
    for (at, n, clear) in [(0x3DF8u32, 8, 1), (0x3DFC, 0xC, 0), (0x3E00, 0x4A, 0), (0x3E04, 0x41, 0), (0x3E08, 0x38, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set32(D + at, r);
    }
    words(fx, &[(0x3DD0, 0x1AC), (0x3DD4, 1), (0x3DD8, param as u32), (0x3DE0, 0x1A), (0x3DDC, 0x80)]);
    disc_init(fx, 0x590, 0x40A0_0000);
    disc_init(fx, 0xA70, 0x41C0_0000);
    for k in 0..3u32 {
        disc_init(fx, 0x480 + 0x10 * k, 0x4040_0000);
    }
    // The ribbon (+0x70, points +0xA0), texture 5 / 0.
    fx.texture5(sp + 0xA0, 0);
    let tex = fx.mem.read_u32(sp + 0xA0);
    words(fx, &[(0x70, tex), (0x78, 0x8003)]);
    bytes(fx, &[(0x76, 0x80), (0x75, 0x80), (0x74, 0x80), (0x77, 0x80)]);
    halves(fx, &[(0x80, 0), (0x82, 0), (0x84, 0x10), (0x86, 0x20), (0x88, 0), (0x8A, 0), (0x8C, 0x80), (0x8E, 0x40)]);
    words(fx, &[(0x7C, 0x42C8_0000)]);
    halves(fx, &[(0x92, 8), (0x94, 0), (0x96, 0x48), (0x98, 0x20), (0x9A, 0), (0x9C, 1)]);
    // Fountain A (+0xAF0, particles +0xB50).
    let tex = fx.mem.read_u32(sp + 0xA0);
    sprite_group(fx, 0xAF0, tex, 0x4170_0000);
    fx.sprites_spawn(D + 0xAF0, 1.0);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(D + 0xB4A) {
        let p = D + 0xB50 + 0x10 * j;
        let h = rmod(fx, D, 0x01E0_48CC, 0xC);
        j += 1;
        fx.mem.set16(p + 0xE, -h);
        fx.mem.write_u8(p + 0xC, 0x30);
        fx.mem.write_u8(p + 0xD, 0);
        let y = ee::add(fx.mem.f(p + 4), 10.0);
        fx.mem.setf(p + 4, y);
    }
    // Fountain B (+0x1B50, particles +0x1BB0).
    let tex = fx.mem.read_u32(sp + 0xA0);
    sprite_group(fx, 0x1B50, tex, 0x4000_0000);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(D + 0x1BAA) {
        let p = D + 0x1BB0 + 0x10 * j;
        throw(fx, p, [0x01E0_49C8, 0x01E0_49D0, 0x01E0_49EC, 0x01E0_4A14]);
        let h = rmod(fx, D, 0x01E0_4A6C, 0xC);
        j += 1;
        fx.mem.set16(p + 0xE, -h);
        fx.mem.write_u8(p + 0xC, 0x80);
    }
    // Quad groups +0x610 (size 4) and +0x2C20 (0.5), texture 4 / 0xF.
    fx.texture4(sp + 0xA0, 0xF);
    for (g, flags, size) in [(0x610u32, 0x8_0001u32, 4.0f32), (0x2C20, 1, 0.5)] {
        words(fx, &[(g + 4, flags)]);
        halves(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x22), (g + 0x16, 0x40)]);
        bytes(fx, &[(g + 0xA, 0xE0), (g + 9, 0x50), (g + 8, 0x40), (g + 0xB, 0x20)]);
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.quads_setup(D + g, tex, size, TEMPLATE);
    }
    for k in 0..0x40u32 {
        let p = D + 0x2C80 + 0x10 * k;
        let r = rmod(fx, D, 0x01E0_4B70, 0x3000);
        fx.mem.set32(p, r - 0x1800);
        let a = fx.rand_from(D, 0x01E0_4B90).wrapping_rem(0x1_0000);
        fx.mem.set32(p + 4, a);
        fx.mem.write_u32(p + 8, 0);
    }
    // Three rings (+0x31D0 + 0x400 k).
    for k in 0..3u32 {
        let g = 0x31D0 + 0x400 * k;
        words(fx, &[(g + 4, 0x8_1000)]);
        halves(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x80)]);
        bytes(fx, &[(g + 0xA, 0xE0), (g + 9, 0x50), (g + 8, 0x40), (g + 0xB, 0x80)]);
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(D + g, tex, f32::from_bits(0x41AC_0000), f32::from_bits(0x41B4_0000), 0.0, 0.0);
    }
    // Three strands (+0x2A0 + 0x30 k), flags 0.
    for k in 0..3u32 {
        let g = 0x2A0 + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0xA0);
        words(fx, &[(g, tex), (g + 4, 0)]);
        halves(fx, &[(g + 0x10, 0x48), (g + 0x12, 0xC0), (g + 0x14, 0x38), (g + 0x16, 0x40)]);
        bytes(fx, &[(g + 0xA, 0xE0), (g + 9, 0x50), (g + 8, 0x40), (g + 0xB, 0x80)]);
        halves(fx, &[(g + 0xC, 0x10), (g + 0x28, -4 * k as i32), (g + 0xE, 0xA)]);
        words(fx, &[(g + 0x20, 0), (g + 0x24, 0)]);
    }
    for (x, s) in [(0x5A0u32, TENTH), (0x2BB0, TENTH), (0x3080, 0), (0x30F0, 0x3F00_0000), (0x3160, 0x3F80_0000)] {
        words(fx, &[(x + 8, s), (x + 4, s), (x, s)]);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    for (at, out) in [(0x3E00u32, D + 0x350), (0x3E04, D + 0x3C0), (0x3E08, D + 0x430), (0x3DF8, sp + 0x80), (0x3DFC, sp + 0x90), (0x3DFC, D + 0x2BD0)] {
        node(fx, at, out);
    }
    fx.look_angles(sp + 0x80, sp + 0x90, D + 0xA90);
    for o in [0u32, 4, 8, 0xC] {
        let v = fx.mem.read_u32(D + 0xA90 + o);
        fx.mem.write_u32(D + 0x2BC0 + o, v);
    }
    for out in [0x3DE4u32, 0x5C0, 0x30A0, 0x3110, 0x3180] {
        let t = fx.target_handle();
        fx.unit_position(t, D + out);
    }
    node(fx, 0x3DF8, D + 0xAA0);
    node(fx, 0x3DF8, D + 0x540);
    if n(fx) == 0xE4 {
        node(fx, 0x3DF8, D + 0x4D0);
    }
    if n(fx) >= 0xE4 {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x4D0);
        let m = fx.mem.i32(D + 0x3DE0);
        fx.approach_by(D + 0x4D0, D + 0x3DE4, m);
        let m = fx.mem.i32(D + 0x3DE0);
        if m != 0 {
            fx.mem.set32(D + 0x3DE0, m - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 / s3 the loops' (D + 0x70 k / k; D + 0x150
    // / 3 after the position loop).
    let regs = |s2: u32, s3: u32| [Some(D), Some(OT), Some(s2), Some(s3)];
    let r0 = regs(D + 0x150, 3);
    fx.xform_world_saving(D, 0x01E0_4EE4, r0);
    fx.xform_billboard_saving(D + 0x4B0, 0x01E0_4EEC, r0);
    fx.xform_world_saving(D + 0xA80, 0x01E0_4EF4, r0);
    fx.xform_billboard_saving(D + 0x520, 0x01E0_4EFC, r0);
    fx.xform_billboard_saving(D + 0x5A0, 0x01E0_4F04, r0);
    fx.xform_world_saving(D + 0x2BB0, 0x01E0_4F0C, r0);
    for k in 0..3u32 {
        let r = regs(D + 0x70 * k, k);
        fx.xform_billboard_saving(D + 0x330 + 0x70 * k, 0x01E0_4F1C, r);
        fx.xform_world_saving(D + 0x3080 + 0x70 * k, 0x01E0_4F24, r);
    }
    if n(fx) < 0xE4 {
        // The strands' and discs' colours flicker (rand's frame: s0 = D).
        let r = rmod(fx, D, 0x01E0_4F4C, 0x60) + 0x80;
        let g = rmod(fx, D, 0x01E0_4F84, 0x50);
        let b = fx.rand_from(D, 0x01E0_4FB8).wrapping_rem(0x40);
        for at in [0x2A8u32, 0x2D8, 0x308, 0x484, 0x494, 0x4A4, 0x594] {
            bytes(fx, &[(at + 2, r as u8), (at + 1, g as u8), (at, b as u8)]);
        }
    }
    if n(fx) == 0xE4 {
        bytes(fx, &[(0x596, 0xE0), (0x595, 0x50), (0x594, 0x40)]);
    }
    let m = n(fx);
    if (0x28..0x87).contains(&m) || m >= 0xE4 {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0x19B {
        fx.step_down(LIGHT, 0x20, 4);
    } else {
        fx.step_up(LIGHT, 0x80, 4);
    }
    let l = fx.mem.read_u8(LIGHT);
    fx.set_stage_light(i32::from(l));
    if n(fx) < 0xD3 {
        for (g, a, b) in [(0x2A0u32, 0x350u32, 0x3C0u32), (0x2D0, 0x3C0, 0x430), (0x300, 0x430, 0x350)] {
            strand::draw(fx, STRAND, OT, D + g, D, D + a, D + b);
        }
    }
    for k in 0..3u32 {
        fx.disc(D + 0x480 + 0x10 * k, D + 0x330 + 0x70 * k);
        disc_saved(fx, sp, 0x01E0_50F8);
        if n(fx) >= 0xD3 {
            fx.step_down(D + 0x487 + 0x10 * k, 0, 8);
        } else {
            fx.step_up(D + 0x487 + 0x10 * k, 0x80, 8);
        }
    }
    if n(fx) >= 0xFE {
        for k in 0..3u32 {
            let (g, x) = (D + 0x31D0 + 0x400 * k, D + 0x3080 + 0x70 * k);
            fx.ring_draw(g, x);
            fx.step_down(g + 0xB, 0, 8);
            if fx.approach(x, 1.0, f32::from_bits(TENTH)) == 0 {
                for o in [8u32, 4, 0] {
                    fx.mem.write_u32(x + o, TENTH);
                }
                for (o, ra) in [(0x10u32, 0x01E0_51BCu32), (0x14, 0x01E0_51D8), (0x18, 0x01E0_51F4)] {
                    let r = fx.rand_from(D, ra).wrapping_rem(0x1_0000);
                    fx.mem.set32(x + o, r);
                }
                if n(fx) < 0x19B {
                    fx.mem.write_u8(g + 0xB, 0x80);
                }
            }
        }
        fx.approach(D + 0x5A0, 1.0, f32::from_bits(0x3D4C_CCCD));
        fx.disc(D + 0xA70, D + 0x5A0);
        disc_saved(fx, sp, 0x01E0_5268);
        fx.quads_draw(D + 0x610, D + 0x5A0);
        if n(fx) >= 0x19B {
            fx.step_down(D + 0xA77, 0, 0x10);
            fx.step_down(D + 0x61B, 0, 2);
        } else {
            fx.step_up(D + 0xA77, 0xFF, 8);
        }
        for k in 0..0x40u32 {
            for c in 0..3u32 {
                let a = D + 0x670 + 0x10 * k + 4 * c;
                let w = fx.mem.read_u32(a).wrapping_add(0x40);
                fx.mem.write_u32(a, w);
            }
        }
    }
    if n(fx) >= 0x28 {
        if n(fx) < 0x109 {
            fx.disc(D + 0x590, D + 0x520);
            disc_saved(fx, sp, 0x01E0_532C);
            if n(fx) < 0x87 {
                fx.step_up(D + 0x597, 0xFF, 4);
            }
            if n(fx) >= 0xE4 {
                fstep_up(fx, D + 0x598, 10.0, f32::from_bits(TENTH));
            }
        }
        fx.quads_draw(D + 0x2C20, D + 0x2BB0);
        if n(fx) >= 0xE4 {
            fx.approach(D + 0x2BB0, 1.0, f32::from_bits(0x3D4C_CCCD));
        }
        for k in 0..0x40u32 {
            let a = D + 0x2C84 + 0x10 * k;
            let d: u32 = if k & 1 == 0 { 0x40 } else { 0xFFFF_FFC0 };
            let w = fx.mem.read_u32(a).wrapping_add(d);
            fx.mem.write_u32(a, w);
        }
        if n(fx) < 0xE4 {
            fx.sprites(D + 0xAF0, D + 0xA80);
            if n(fx) >= 0xC8 {
                fx.step_down(D + 0xAFB, 0, 8);
            }
            let mut j = 0u32;
            while (j as i32) < fx.mem.i16(D + 0xB4A) {
                let p = D + 0xB50 + 0x10 * j;
                j += 1;
                if fx.mem.i16(p + 0xE) < 0 {
                    continue;
                }
                let y = ee::sub(fx.mem.f(p + 4), 1.0);
                fx.mem.setf(p + 4, y);
                fx.step_up(p + 0xC, 0x80, 8);
                if !ee_le(fx.mem.f(p + 4), 0.0) {
                    let s = f32::from_bits(0x3F4C_CCCD);
                    for o in [8u32, 0] {
                        let v = ee::mul(fx.mem.f(p + o), s);
                        fx.mem.setf(p + o, v);
                    }
                }
                if ee_lt(fx.mem.f(p + 4), 0.0) {
                    let a = fx.rand_from(D, 0x01E0_54EC).wrapping_rem(0x1_0000);
                    let c = fx.tcos(a);
                    let r = fx.mem.f(D + 0xB3C);
                    fx.mem.setf(p + 8, ee::mul(r, c));
                    let s = fx.tsin(a);
                    let r = fx.mem.f(D + 0xB3C);
                    fx.mem.setf(p, ee::mul(r, s));
                    let h = rmod(fx, D, 0x01E0_5530, 0xA);
                    fx.mem.setf(p + 4, ee::add(10.0, ee::from_i32(h)));
                    fx.mem.write_u8(p + 0xC, 0);
                }
            }
        }
    }
    if n(fx) >= 0xE4 {
        fx.sprites(D + 0x1B50, D + 0xA80);
        let g = f32::from_bits(0x3F99_999A);
        let mut j = 0u32;
        while (j as i32) < fx.mem.i16(D + 0x1BAA) {
            let p = D + 0x1BB0 + 0x10 * j;
            j += 1;
            if fx.mem.i16(p + 0xE) < 0 {
                continue;
            }
            let y = ee::add(fx.mem.f(p + 4), 2.0);
            fx.mem.setf(p + 4, y);
            for o in [0u32, 8] {
                let v = ee::mul(fx.mem.f(p + o), g);
                fx.mem.setf(p + o, v);
            }
            fx.step_down(p + 0xC, 0, 8);
            if ee_le(fx.mem.f(p + 4), 20.0) {
                continue;
            }
            throw(fx, p, [0x01E0_562C, 0x01E0_5634, 0x01E0_5650, 0x01E0_5678]);
            let h = rmod(fx, D, 0x01E0_56D0, 0xC);
            fx.mem.set16(p + 0xE, -h);
            fx.mem.write_u8(p + 0xC, 0x80);
        }
    }
    if n(fx) >= 0xE5 {
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0x19B {
        2
    } else if m == 0xF4 {
        1
    } else if (0xFE..0x19B).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
