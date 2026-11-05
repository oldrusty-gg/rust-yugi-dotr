//! BMODULE b452 (card 633), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E045D0, data area 0x1ED0000) [F] (helper C9, 31
//! Oct 2026): four jagged strands (+0x2A0 + 0x30 k, `strand::draw`)
//! between the attacker's nodes 0x1F / 0x27 / 0x1B / 0x23 before update
//! 0x113, discs (+0x520 + 0x10 k) at them from 0x46, flickering together
//! with a disc (+0x640) at node 0x28 until 0xE4; a sprite fountain
//! (+0xBA0) there on 0x28..0xE3; from 0xE4 a frame quad, a second
//! fountain (+0x1C00) and a ribbon (+0x70, `ribbon`) from the node closing
//! on the target; from 0xFE four rings (+0x2E20 + 0x400 k) swelling at
//! the target, a disc (+0xB20) and a quad group (+0x6C0). The stage dims
//! until 0x19B. Returns 1 on update 0xF4, 6 on 0xFE..0x19A, 2 on 0x19B, 8
//! after 0x1AC.
//!
//! Library at its own addresses: `colours4` (0x1E00080), the frame quad
//! (0x1E00120), `sprites_spawn` / `sprites` (0x1E00240 / 0x1E003D0),
//! `ring_setup` / `ring_draw` (0x1E01CC0 / 0x1E01F80), `disc`
//! (0x1E02470), `quads_setup` / `quads_draw` (0x1E029D0 / 0x1E02B30),
//! `ribbon` push / draw (0x1E03140 / 0x1E03290), `strand::draw`
//! (0x1E03C50).
use crate::b142::fstep_up;
use crate::b540::{place, ring_throw, rmod};
use crate::blib::strand;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const STRAND: u32 = 0x01E0_3C50;
const TEMPLATE: u32 = 0x01E0_5700;
const LEFT: u32 = D + 0x3E20;
const N: u32 = D + 0x3E24;
const LIGHT: u32 = D + 0x3E2C;
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

/// Node slot `k` (+0x3E44: 0x28, 0xA, then +0x3E4C: 0x23, 0x1F, 0x1B,
/// 0x27).
fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

/// 0x1E02470 (`disc`)'s prologue saves of the entry's fp (the runner's
/// 0) and ra at its frame +0x90 / +0xA0 = entry sp - 0xB0 / - 0xA0: the
/// sprites' missing-texture TEX0 / TEX1 [run].
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

/// A fountain particle `p` re-thrown on a ring (spread at `spread`) at
/// height 0.1 x -(rand % 40 - 20) (rand's frame: the entry's s0 = D).
fn throw(fx: &mut Fx, p: u32, spread: u32, ras: [u32; 4]) {
    let (a, rad) = ring_throw(fx, D, spread, [ras[0], ras[1], ras[2]]);
    let h = rmod(fx, D, ras[3], 0x28);
    fx.mem.setf(p + 4, ee::mul(f32::from_bits(TENTH), ee::neg(ee::from_i32(h - 0x14))));
    place(fx, p, a, rad);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3E60);
    for x in [0u32, 0x560, 0xB30, 0x5D0, 0x650] {
        fx.xform_init(D + x);
    }
    for k in 0..4u32 {
        fx.xform_init(D + 0x360 + 0x70 * k);
        fx.xform_init(D + 0x2C60 + 0x70 * k);
    }
    for (at, n, clear) in [(0x3E44u32, 0x28, 1), (0x3E48, 0xA, 0), (0x3E50, 0x1F, 0), (0x3E54, 0x1B, 0), (0x3E58, 0x27, 0), (0x3E4C, 0x23, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set32(D + at, r);
    }
    words(fx, &[(0x3E20, 0x1AC), (0x3E24, 1), (0x3E28, param as u32), (0x3E30, 0x1A), (0x3E2C, 0x80)]);
    disc_init(fx, 0x640, 0x40A0_0000);
    disc_init(fx, 0xB20, 0x41C0_0000);
    for k in 0..4u32 {
        disc_init(fx, 0x520 + 0x10 * k, 0x40C0_0000);
    }
    // The ribbon (+0x70, points +0xA0), texture 5 / 0.
    fx.texture5(sp + 0xA0, 0);
    let tex = fx.mem.read_u32(sp + 0xA0);
    words(fx, &[(0x70, tex), (0x78, 0x8_8003)]);
    bytes(fx, &[(0x76, 0x80), (0x75, 0x80), (0x74, 0x80), (0x77, 0x80)]);
    halves(fx, &[(0x80, 0), (0x82, 0), (0x84, 0x10), (0x86, 0x20), (0x88, 0), (0x8A, 0), (0x8C, 0x80), (0x8E, 0x40)]);
    words(fx, &[(0x7C, 0x4348_0000)]);
    halves(fx, &[(0x92, 8), (0x94, 0), (0x96, 0x6C), (0x98, 0x20), (0x9A, 0), (0x9C, 1)]);
    // Fountain A (+0xBA0, particles +0xC00).
    let tex = fx.mem.read_u32(sp + 0xA0);
    sprite_group(fx, 0xBA0, tex, 0x4170_0000);
    fx.sprites_spawn(D + 0xBA0, 1.0);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(D + 0xBFA) {
        let p = D + 0xC00 + 0x10 * j;
        let h = rmod(fx, D, 0x01E0_48E0, 0xC);
        fx.mem.set16(p + 0xE, -h);
        fx.mem.write_u8(p + 0xC, 0);
        fx.mem.write_u8(p + 0xD, 0);
        let y = ee::add(fx.mem.f(p + 4), 10.0);
        fx.mem.setf(p + 4, y);
        j += 1;
    }
    // Fountain B (+0x1C00, particles +0x1C60).
    let tex = fx.mem.read_u32(sp + 0xA0);
    sprite_group(fx, 0x1C00, tex, 0x4000_0000);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(D + 0x1C5A) {
        let p = D + 0x1C60 + 0x10 * j;
        throw(fx, p, D + 0x1C4C, [0x01E0_49D8, 0x01E0_49E0, 0x01E0_49FC, 0x01E0_4A24]);
        let h = rmod(fx, D, 0x01E0_4A84, 0xC);
        fx.mem.set16(p + 0xE, -h);
        fx.mem.write_u8(p + 0xC, 0x80);
        j += 1;
    }
    // The quad group (+0x6C0), texture 4 / 0xF.
    fx.texture4(sp + 0xA0, 0xF);
    words(fx, &[(0x6C4, 0x8_0001)]);
    halves(fx, &[(0x6D0, 0x8E), (0x6D2, 0), (0x6D4, 0x22), (0x6D6, 0x40)]);
    bytes(fx, &[(0x6CA, 0xE0), (0x6C9, 0x50), (0x6C8, 0x40), (0x6CB, 0x20)]);
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.quads_setup(D + 0x6C0, tex, 4.0, TEMPLATE);
    // Four rings (+0x2E20 + 0x400 k).
    for k in 0..4u32 {
        let g = 0x2E20 + 0x400 * k;
        words(fx, &[(g + 4, 0x8_1000)]);
        halves(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x80)]);
        bytes(fx, &[(g + 0xA, 0xE0), (g + 9, 0x50), (g + 8, 0x40), (g + 0xB, 0x80)]);
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(D + g, tex, f32::from_bits(0x41AC_0000), f32::from_bits(0x41B4_0000), 0.0, 0.0);
    }
    // Four strands (+0x2A0 + 0x30 k).
    for k in 0..4u32 {
        let g = 0x2A0 + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0xA0);
        words(fx, &[(g, tex), (g + 4, 0x8_0000)]);
        halves(fx, &[(g + 0x10, 0x48), (g + 0x12, 0xC0), (g + 0x14, 0x38), (g + 0x16, 0x40)]);
        bytes(fx, &[(g + 0xA, 0xE0), (g + 9, 0x50), (g + 8, 0x40), (g + 0xB, 0x80)]);
        halves(fx, &[(g + 0xC, 0x10), (g + 0x28, -4 * k as i32), (g + 0xE, 0xA)]);
        words(fx, &[(g + 0x20, 0), (g + 0x24, 0)]);
    }
    for (x, s) in [(0x650u32, TENTH), (0x2C60, TENTH), (0x2CD0, 0x3ECC_CCCD), (0x2D40, 0x3F33_3333), (0x2DB0, 0x3F80_0000)] {
        words(fx, &[(x + 8, s), (x + 4, s), (x, s)]);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    for (at, out) in [(0x3E50u32, 0x380u32), (0x3E54, 0x3F0), (0x3E58, 0x460), (0x3E4C, 0x4D0)] {
        node(fx, at, D + out);
    }
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x80);
    node(fx, 0x3E44, sp + 0x90);
    fx.look_angles(sp + 0x80, sp + 0x90, D + 0xB40);
    for out in [0x3E34u32, 0x670, 0x2C80, 0x2CF0, 0x2D60, 0x2DD0] {
        let t = fx.target_handle();
        fx.unit_position(t, D + out);
    }
    node(fx, 0x3E44, D + 0xB50);
    node(fx, 0x3E44, D + 0x5F0);
    if n(fx) == 0xE4 {
        node(fx, 0x3E44, D + 0x580);
    }
    if n(fx) >= 0xE4 {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x580);
        let m = fx.mem.i32(D + 0x3E30);
        fx.approach_by(D + 0x580, D + 0x3E34, m);
        let m = fx.mem.i32(D + 0x3E30);
        if m != 0 {
            fx.mem.set32(D + 0x3E30, m - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 / s3 the loops' (D + 0x70 k / k; D + 0x1C0
    // / 4 after the position loop).
    let regs = |s2: u32, s3: u32| [Some(D), Some(OT), Some(s2), Some(s3)];
    let r0 = regs(D + 0x1C0, 4);
    fx.xform_world_saving(D, 0x01E0_4E34, r0);
    fx.xform_billboard_saving(D + 0x560, 0x01E0_4E3C, r0);
    fx.xform_world_saving(D + 0xB30, 0x01E0_4E44, r0);
    fx.xform_billboard_saving(D + 0x5D0, 0x01E0_4E4C, r0);
    fx.xform_billboard_saving(D + 0x650, 0x01E0_4E54, r0);
    for k in 0..4u32 {
        let r = regs(D + 0x70 * k, k);
        fx.xform_billboard_saving(D + 0x360 + 0x70 * k, 0x01E0_4E64, r);
        fx.xform_world_saving(D + 0x2C60 + 0x70 * k, 0x01E0_4E6C, r);
    }
    if n(fx) < 0xE4 {
        // The strands' and discs' colours flicker (rand's frame: s0 = D).
        let r = rmod(fx, D, 0x01E0_4E94, 0x60) + 0x80;
        let g = rmod(fx, D, 0x01E0_4ED0, 0x50);
        let b = fx.rand_from(D, 0x01E0_4F08).wrapping_rem(0x40);
        for at in [0x2A8u32, 0x2D8, 0x308, 0x524, 0x534, 0x544, 0x554, 0x644] {
            bytes(fx, &[(at + 2, r as u8), (at + 1, g as u8), (at, b as u8)]);
        }
    }
    if n(fx) == 0xE4 {
        bytes(fx, &[(0x646, 0xE0), (0x645, 0x50), (0x644, 0x40)]);
    }
    if n(fx) >= 0xE4 {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0x19B {
        fx.step_down(LIGHT, 0x20, 4);
    } else {
        fx.step_up(LIGHT, 0x80, 4);
    }
    let l = fx.mem.read_u8(LIGHT);
    fx.set_stage_light(i32::from(l));
    if n(fx) < 0x113 {
        for (g, a, b) in [(0x2A0u32, 0x380u32, 0x460u32), (0x2D0, 0x4D0, 0x380), (0x300, 0x460, 0x3F0), (0x330, 0x3F0, 0x4D0)] {
            strand::draw(fx, STRAND, OT, D + g, D, D + a, D + b);
        }
    }
    if n(fx) >= 0x46 {
        for k in 0..4u32 {
            fx.disc(D + 0x520 + 0x10 * k, D + 0x360 + 0x70 * k);
            disc_saved(fx, sp, 0x01E0_505C);
            if n(fx) >= 0x113 {
                fx.step_down(D + 0x527 + 0x10 * k, 0, 8);
            } else {
                fx.step_up(D + 0x527 + 0x10 * k, 0x80, 8);
            }
        }
    }
    if n(fx) >= 0xFE {
        for k in 0..4u32 {
            let (g, x) = (D + 0x2E20 + 0x400 * k, D + 0x2C60 + 0x70 * k);
            fx.ring_draw(g, x);
            fx.step_down(g + 0xB, 0, 8);
            if fx.approach(x, 1.0, f32::from_bits(TENTH)) == 0 {
                for o in [8u32, 4, 0] {
                    fx.mem.write_u32(x + o, TENTH);
                }
                for (o, ra) in [(0x10u32, 0x01E0_5128u32), (0x14, 0x01E0_5144), (0x18, 0x01E0_5160)] {
                    let r = fx.rand_from(D, ra).wrapping_rem(0x1_0000);
                    fx.mem.set32(x + o, r);
                }
                if n(fx) < 0x19B {
                    fx.mem.write_u8(g + 0xB, 0x80);
                }
            }
        }
        fx.approach(D + 0x650, 1.0, f32::from_bits(0x3D4C_CCCD));
        fx.disc(D + 0xB20, D + 0x650);
        disc_saved(fx, sp, 0x01E0_51D0);
        fx.quads_draw(D + 0x6C0, D + 0x650);
        if n(fx) >= 0x19B {
            fx.step_down(D + 0xB27, 0, 0x10);
            fx.step_down(D + 0x6CB, 0, 2);
        } else {
            fx.step_up(D + 0xB27, 0xFF, 8);
        }
        for k in 0..0x40u32 {
            for c in 0..3u32 {
                let a = D + 0x720 + 0x10 * k + 4 * c;
                let w = fx.mem.read_u32(a).wrapping_add(0x40);
                fx.mem.write_u32(a, w);
            }
        }
    }
    if n(fx) >= 0x28 {
        if n(fx) < 0x109 {
            fx.disc(D + 0x640, D + 0x5D0);
            disc_saved(fx, sp, 0x01E0_5294);
            if n(fx) < 0x87 {
                fx.step_up(D + 0x647, 0xFF, 4);
            }
            if n(fx) >= 0xE4 {
                fstep_up(fx, D + 0x648, 10.0, f32::from_bits(TENTH));
            }
        }
        if n(fx) < 0xE4 {
            fx.sprites(D + 0xBA0, D + 0xB30);
            if n(fx) >= 0xC8 {
                fx.step_down(D + 0xBAB, 0, 8);
            }
            let mut j = 0u32;
            while (j as i32) < fx.mem.i16(D + 0xBFA) {
                let p = D + 0xC00 + 0x10 * j;
                j += 1;
                if fx.mem.i16(p + 0xE) < 0 {
                    continue;
                }
                let y = ee::sub(fx.mem.f(p + 4), 1.0);
                fx.mem.setf(p + 4, y);
                if n(fx) >= 0x46 {
                    fx.step_up(p + 0xC, 0x80, 8);
                }
                if !ee_le(fx.mem.f(p + 4), 0.0) {
                    let s = f32::from_bits(0x3F4C_CCCD);
                    for o in [8u32, 0] {
                        let v = ee::mul(fx.mem.f(p + o), s);
                        fx.mem.setf(p + o, v);
                    }
                }
                if ee_lt(fx.mem.f(p + 4), 0.0) {
                    let a = fx.rand_from(D, 0x01E0_53DC).wrapping_rem(0x1_0000);
                    let c = fx.tcos(a);
                    let r = fx.mem.f(D + 0xBEC);
                    fx.mem.setf(p + 8, ee::mul(r, c));
                    let s = fx.tsin(a);
                    let r = fx.mem.f(D + 0xBEC);
                    fx.mem.setf(p, ee::mul(r, s));
                    let h = rmod(fx, D, 0x01E0_5420, 0xA);
                    fx.mem.setf(p + 4, ee::add(10.0, ee::from_i32(h)));
                    fx.mem.write_u8(p + 0xC, 0);
                }
            }
        }
    }
    if n(fx) >= 0xE4 {
        fx.sprites(D + 0x1C00, D + 0xB30);
        let g = f32::from_bits(0x3FA6_6666);
        let mut j = 0u32;
        while (j as i32) < fx.mem.i16(D + 0x1C5A) {
            let p = D + 0x1C60 + 0x10 * j;
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
            throw(fx, p, D + 0x1C4C, [0x01E0_551C, 0x01E0_5524, 0x01E0_5540, 0x01E0_5568]);
            let h = rmod(fx, D, 0x01E0_55C0, 0xC);
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
