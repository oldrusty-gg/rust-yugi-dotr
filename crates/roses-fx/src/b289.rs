//! BMODULE b289 (card 133), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03330, data area 0x1ED0000) [F] (helper C9, 31
//! Oct 2026): a sprite fountain (+0x3460) rising from 20 above the target
//! throughout (lit on 0x4F..0xAC); a facing quad (+0x170) and a disc
//! (+0x1D0) high over the target, coming down from 300 to 68 from update
//! 0x7B (recoloured, wider) and to 20 from 0xAD; a frame quad before
//! 0x4F; from 0xAD the target's unit shrunk away (0x1F74C0 scale 0 until
//! 0x12C), two rings of blades (+0x250 / +0x12F0, `blade`) and a cloud of
//! re-thrown sprites (+0x44C0) at it, from 0xAE a second fountain
//! (+0x2400); from 0x11C a full-window tint (+0x4940). The stage dims
//! until 0x7C, lightens to 0x20, then to 0x80 from 0x12C. Returns 1 on
//! update 0xA3, 6 on 0xAD..0x12B, 2 on 0x12C, 8 after 0x14A.
//!
//! Library at its own addresses: `colours4` (0x1E00080), the tint
//! (0x1E00120), the frame quad (0x1E00280), `disc` (0x1E003A0),
//! `facing::quad` (0x1E00900), `sprites` (0x1E01190), `blade` setup /
//! draw (0x1E02A80 / 0x1E02C20).
use crate::b142::fstep_up;
use crate::b436::fstep_down;
use crate::b540::{place, ring_throw, rmod};
use crate::blib::{blade, facing};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BLADE_SETUP: u32 = 0x01E0_2A80;
const LEFT: u32 = D + 0x4930;
const N: u32 = D + 0x4934;
const LIGHT: u32 = D + 0x493C;
const NODES: u32 = D + 0x4944;
const TENTH: u32 = 0x3DCC_CCCD;

/// The entry 0x1E03330.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
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

fn node(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(NODES + 4 * k);
    fx.node_position(h, s, out);
}

/// 0x1034B0(p, floor, step): the i16 *p = floor (0) when below floor +
/// step, else *p -= step (1).
fn hstep_down(fx: &mut Fx, p: u32, floor: i32, step: i32) -> i32 {
    let v = fx.mem.i16(p);
    if v < floor + step {
        fx.mem.set16(p, floor);
        0
    } else {
        fx.mem.set16(p, v - step);
        1
    }
}

/// A fountain particle `p` re-thrown on a ring (spread at `spread`) at
/// height 0.1 x -(rand % 40 - 20) (rand's frame: the entry's s0 = D).
fn throw(fx: &mut Fx, p: u32, spread: u32, ras: [u32; 4]) {
    let (a, rad) = ring_throw(fx, D, spread, [ras[0], ras[1], ras[2]]);
    let h = rmod(fx, D, ras[3], 0x28);
    fx.mem.setf(p + 4, ee::mul(f32::from_bits(TENTH), ee::neg(ee::from_i32(h - 0x14))));
    place(fx, p, a, rad);
}

/// A sprite group's common fields: texture `tex`, flags, colour (+8..+0xB
/// = 0x80, alpha `a`), +0xC, the cell +0x30.., particles at +0x60.
fn sprite_group(fx: &mut Fx, g: u32, tex: u32, flags: u32, a: u8, half: u32, cell: [i32; 8]) {
    words(fx, &[(g, tex), (g + 4, flags)]);
    bytes(fx, &[(g + 0xA, 0x80), (g + 9, 0x80), (g + 8, 0x80), (g + 0xB, a)]);
    words(fx, &[(g + 0xC, half), (g + 0x20, 0), (g + 0x24, 0), (g + 0x28, 0)]);
    let at: Vec<(u32, i32)> = (0..8u32).map(|k| (g + 0x30 + 2 * k, cell[k as usize])).collect();
    halves(fx, &at);
    words(fx, &[(g + 0x40, D + g + 0x60)]);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4950);
    for x in [0u32, 0x70, 0xE0, 0x1E0, 0x2390] {
        fx.xform_init(D + x);
    }
    for (k, n) in [0x31, 0x2F].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set32(NODES + 4 * k as u32, r);
    }
    fx.mem.set32(LEFT, 0x14A);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x4938, param);
    fx.mem.set32(LIGHT, 0x80);
    bytes(fx, &[(0x4942, 0xD0), (0x4941, 0xA0), (0x4940, 0x70), (0x4943, 0)]);
    // The disc (+0x1D0): radius 21.
    words(fx, &[(0x1D0, 0x8_0000)]);
    bytes(fx, &[(0x1D6, 0x70), (0x1D5, 0xA0), (0x1D4, 0xD0), (0x1D7, 0x80)]);
    words(fx, &[(0x1D8, 0x41A8_0000)]);
    halves(fx, &[(0x1DC, 0x24), (0x1DE, 0)]);
    // The facing quad (+0x170, quad +0x150), texture 5 / 0.
    fx.texture5(sp + 0xB0, 0);
    words(fx, &[(0x1A8, 0x8_1003)]);
    halves(fx, &[(0x170, 0), (0x172, 0), (0x174, 0x80), (0x176, 0x80), (0x178, 0), (0x17A, 0), (0x17C, 0x80), (0x17E, 0x80)]);
    words(fx, &[(0x1A0, 0x3E99_999A)]);
    bytes(fx, &[(0x1A6, 0x80), (0x1A5, 0x80), (0x1A4, 0x80), (0x1A7, 0x80)]);
    halves(fx, &[(0x1B8, 0), (0x1BA, 0), (0x1BC, 0), (0x1BE, 1), (0x1C0, 0), (0x1C2, 0x3E7)]);
    let tex = fx.mem.read_u32(sp + 0xB0);
    words(fx, &[(0x168, 0), (0x164, 0), (0x160, 0), (0x158, 0x4240_0000), (0x150, tex)]);
    bytes(fx, &[(0x15E, 0x80), (0x15D, 0x80), (0x15C, 0x80), (0x15F, 0x80)]);
    // Two rings of blades (+0x250 / +0x12F0), texture 5 / 1.
    fx.texture5(sp + 0xB0, 1);
    for (g, lift) in [(0x250u32, 0u32), (0x12F0, 0x4120_0000)] {
        words(fx, &[(g + 4, 1)]);
        halves(fx, &[(g + 0x10, 0), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x40)]);
        bytes(fx, &[(g + 0xA, 0xB0), (g + 9, 0x70), (g + 8, 0x40), (g + 0xB, 0x80)]);
        words(fx, &[(g + 0x80, D + g + 0xA0), (g + 0x84, 0x41A0_0000), (g + 0x88, 0x41F0_0000), (g + 0x8C, lift), (g + 0x90, 0), (g + 0x94, 0x1_0000), (g + 0xC, 0x4248_0000)]);
        halves(fx, &[(g + 0x98, 0), (g + 0x9A, 0x100), (g + 0x9C, 0x3E7), (g + 0x9E, 3)]);
        let tex = fx.mem.read_u32(sp + 0xB0);
        blade::setup(fx, BLADE_SETUP, D + g, tex);
    }
    // Fountain C (+0x3460), texture 5 / 2.
    fx.texture5(sp + 0xB0, 2);
    let tex = fx.mem.read_u32(sp + 0xB0);
    sprite_group(fx, 0x3460, tex, 0x7000_0003, 0x80, 0x4000_0000, [0, 0, 0x20, 0x20, 0, 0, 0x80, 0x40]);
    words(fx, &[(0x34A4, 0), (0x34A8, 0), (0x34AC, 0x41A0_0000)]);
    halves(fx, &[(0x34BA, 0x100), (0x34BC, 8), (0x34BE, 0x32)]);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(D + 0x34BA) {
        let p = D + 0x34C0 + 0x10 * j;
        throw(fx, p, D + 0x34AC, [0x01E0_36CC, 0x01E0_36D4, 0x01E0_36F0, 0x01E0_3718]);
        let h = rmod(fx, D, 0x01E0_3770, 0xC);
        fx.mem.set16(p + 0xE, -h);
        let r = fx.rand_from(D, 0x01E0_3790).wrapping_rem(8);
        fx.mem.write_u8(p + 0xD, r as u8);
        fx.mem.write_u8(p + 0xC, 0);
        j += 1;
    }
    // Fountain D (+0x2400), texture 4 / 7.
    fx.texture4(sp + 0xB0, 7);
    let tex = fx.mem.read_u32(sp + 0xB0);
    sprite_group(fx, 0x2400, tex, 0x7000_0403, 0x80, 0x4000_0000, [0x40, 0, 0x20, 0x20, 0x40, 0, 0x40, 0x40]);
    words(fx, &[(0x2444, 0), (0x2448, 0), (0x244C, 0x41A0_0000)]);
    halves(fx, &[(0x245A, 0x100), (0x245C, 4), (0x245E, 0x32)]);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(D + 0x245A) {
        let p = D + 0x2460 + 0x10 * j;
        throw(fx, p, D + 0x244C, [0x01E0_3888, 0x01E0_3890, 0x01E0_38AC, 0x01E0_38D4]);
        let h = rmod(fx, D, 0x01E0_392C, 0xC);
        fx.mem.set16(p + 0xE, -h);
        let r = fx.rand_from(D, 0x01E0_394C).wrapping_rem(4);
        fx.mem.write_u8(p + 0xD, r as u8);
        let a = fx.mem.read_u8(D + 0x240B);
        fx.mem.write_u8(p + 0xC, a);
        j += 1;
    }
    // The burst (+0x44C0, particles +0x4520, angles +0x4720), texture 4 / 1.
    fx.texture4(sp + 0xB0, 1);
    let tex = fx.mem.read_u32(sp + 0xB0);
    sprite_group(fx, 0x44C0, tex, 0x4800_0003, 0x40, 0x41A0_0000, [0, 0, 0x18, 0x20, 0, 0, 0x80, 0x40]);
    words(fx, &[(0x4504, D + 0x4720), (0x4508, 0), (0x450C, 0x4170_0000), (0x4510, 0x4120_0000)]);
    halves(fx, &[(0x451A, 0x20), (0x451C, 8), (0x451E, 0x10)]);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(D + 0x451A) {
        let p = D + 0x4520 + 0x10 * j;
        fx.random_dir(sp + 0x80, 14.0);
        for o in [0u32, 4, 8] {
            let w = fx.mem.read_u32(sp + 0x80 + o);
            fx.mem.write_u32(p + o, w);
        }
        let h = rmod(fx, D, 0x01E0_3A6C, 0xC);
        fx.mem.set16(p + 0xE, -h);
        fx.mem.write_u8(p + 0xC, 0x40);
        let r = fx.rand_from(D, 0x01E0_3A94).wrapping_rem(8);
        fx.mem.write_u8(p + 0xD, r as u8);
        j += 1;
    }
    0
}

/// A fountain's particles (`base` + 0x10 k, count `cnt`): rising by `up`,
/// spreading by 1.15; above 60 re-thrown (`life0` the new life: `None`
/// rand % 12) with alpha `alpha`.
fn fountain(fx: &mut Fx, base: u32, cnt: u32, spread: u32, up: f32, ras: [u32; 5], life0: bool, alpha: u8) {
    let g = f32::from_bits(0x3F93_3333);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(cnt) {
        let p = base + 0x10 * j;
        j += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        let y = ee::add(fx.mem.f(p + 4), up);
        fx.mem.setf(p + 4, y);
        for o in [0u32, 8] {
            let v = ee::mul(fx.mem.f(p + o), g);
            fx.mem.setf(p + o, v);
        }
        if ee_le(fx.mem.f(p + 4), 60.0) {
            continue;
        }
        throw(fx, p, spread, [ras[0], ras[1], ras[2], ras[3]]);
        if life0 {
            fx.mem.set16(p + 0xE, 0);
        } else {
            let h = rmod(fx, D, ras[4], 0xC);
            fx.mem.set16(p + 0xE, -h);
        }
        fx.mem.write_u8(p + 0xC, alpha);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    let looked = n(fx) == 0x4F;
    if looked {
        node(fx, 0, sp + 0x90);
        node(fx, 1, sp + 0xA0);
        node(fx, 0, D + 0x90);
        fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x80);
    }
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x23B0);
    words(fx, &[(0x23B4, 0x41A0_0000)]);
    let (x, z) = (fx.mem.read_u32(D + 0x23B0), fx.mem.read_u32(D + 0x23B8));
    words(fx, &[(0x100, x), (0x108, z)]);
    if n(fx) == 0x7B {
        words(fx, &[(0x1A0, 0x3F80_0000)]);
        bytes(fx, &[(0x1D6, 0xD0), (0x1D5, 0xA0), (0x1D4, 0x70)]);
        words(fx, &[(0x1D8, 0x428C_0000), (0x104, 0x4390_0000)]);
    }
    if n(fx) == 0xAD {
        let t = fx.target_handle();
        fx.unit_point(1, t, D + 0x200);
        words(fx, &[(0x204, 0x41A0_0000), (0x104, 0x4288_0000)]);
        for o in [0x1D0u32, 0x1A8] {
            let f = fx.mem.read_u32(D + o) & 0xFFF7_FFFF;
            fx.mem.write_u32(D + o, f);
        }
    }
    let m = n(fx);
    if m >= 0xAD {
        fstep_down(fx, D + 0x104, 20.0, f32::from_bits(0x3EC0_0000));
    } else if m >= 0x7B {
        fstep_down(fx, D + 0x104, 68.0, 4.0);
    } else {
        words(fx, &[(0x104, 0x4396_0000)]);
    }
    // s0 = D, s1 = the OT, s2 = sp + 0xA0 on update 0x4F, else the
    // parameter (-1, 64-bit), s3 the runner's 0.
    let regs = [Some(D), Some(OT), Some(if looked { sp + 0xA0 } else { 0 }), Some(0)];
    for (x, ra, board) in [(0u32, 0x01E0_3C80u32, false), (0x70, 0x01E0_3C88, false), (0x1E0, 0x01E0_3C90, false), (0xE0, 0x01E0_3C98, true), (0x2390, 0x01E0_3CA0, false)] {
        if board {
            fx.xform_billboard_saving(D + x, ra, regs);
            if !looked {
                fx.saved(sp - 0xF0, &[(0x40, u64::MAX)]);
            }
        } else {
            fx.xform_world_saving(D + x, ra, regs);
            if !looked {
                fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
            }
        }
    }
    let s = if (0xAC..0x12C).contains(&n(fx)) { 0 } else { 0x3F80_0000 };
    words(fx, &[(0x4928, s), (0x4924, s), (0x4920, s)]);
    let t = fx.target_handle();
    fx.unit_set_scale(t, D + 0x4920);
    // 0x1F74C0's frame (s0 = D, ra): the frame quad's TEX1 bits merge
    // over its ra slot [run].
    fx.saved(sp - 0x20, &[(0, u64::from(D)), (0x10, 0x01E0_3CF0)]);
    let m = n(fx);
    if m < 0x7C {
        fx.step_down(LIGHT, 0x10, 2);
    } else if m < 0x12C {
        fx.step_up(LIGHT, 0x20, 2);
    } else {
        fx.step_up(LIGHT, 0x80, 8);
    }
    let l = fx.mem.read_u8(LIGHT);
    fx.set_stage_light(i32::from(l));
    if n(fx) >= 0x11C {
        crate::b262::tint(fx, D + 0x4940);
        if n(fx) < 0x12C {
            fx.step_up(D + 0x4943, 0xFF, 0x10);
        } else {
            fx.step_down(D + 0x4943, 0, 0x10);
        }
    }
    if n(fx) < 0x4F {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if (0xAD..0x12C).contains(&n(fx)) {
        blade::draw(fx, OT, D + 0x250, D + 0x1E0);
        blade::draw(fx, OT, D + 0x12F0, D + 0x1E0);
        let mut j = 0u32;
        while (j as i32) < fx.mem.i16(D + 0x2EA) {
            let p = fx.mem.read_u32(D + 0x1370) + 0x10 * j;
            if fx.mem.i16(p + 0xE) >= 0xB {
                fx.step_down(p + 0xC, 0, 8);
            }
            j += 1;
        }
    }
    if n(fx) < 0x12C {
        fx.disc(D + 0x1D0, D + 0xE0);
    }
    if n(fx) >= 0xAD {
        fstep_up(fx, D + 0x1D8, 150.0, 1.0);
    }
    facing::quad(fx, OT, D + 0x170, D + 0x150, D + 0xE0);
    if (0xAE..0x12C).contains(&n(fx)) {
        hstep_down(fx, D + 0x176, 0, 1);
        fx.sprites(D + 0x2400, D + 0x2390);
        let a = if n(fx) < 0x12C { 0x80 } else { 0 };
        fountain(fx, D + 0x2460, D + 0x245A, D + 0x244C, 4.0, [0x01E0_3FA4, 0x01E0_3FAC, 0x01E0_3FC8, 0x01E0_3FF0, 0x01E0_4048], false, a);
    }
    fx.sprites(D + 0x3460, D + 0x70);
    if n(fx) >= 0x5E {
        fx.step_down(D + 0x346B, 0, 8);
    }
    let a = if (0x4F..0xAD).contains(&n(fx)) { 0x80 } else { 0 };
    fountain(fx, D + 0x34C0, D + 0x34BA, D + 0x34AC, 6.0, [0x01E0_4154, 0x01E0_415C, 0x01E0_4178, 0x01E0_41A0, 0], true, a);
    if n(fx) >= 0xAD {
        fx.sprites(D + 0x44C0, D + 0x2390);
        if n(fx) >= 0x12C {
            fx.step_down(D + 0x44CB, 0, 2);
        } else {
            fx.step_up(D + 0x44CB, 0x40, 1);
        }
        let g = f32::from_bits(0x3F83_D70A);
        let mut j = 0u32;
        while (j as i32) < fx.mem.i16(D + 0x451A) {
            let p = D + 0x4520 + 0x10 * j;
            j += 1;
            if fx.mem.i16(p + 0xE) >= 0 {
                for o in [0u32, 4, 8] {
                    let v = ee::mul(fx.mem.f(p + o), g);
                    fx.mem.setf(p + o, v);
                }
                fx.step_down(p + 0xC, 0, 4);
            }
            if fx.mem.i16(p + 0xE) < 0x10 {
                continue;
            }
            fx.random_dir(sp + 0x80, 14.0);
            for o in [0u32, 4, 8] {
                let w = fx.mem.read_u32(sp + 0x80 + o);
                fx.mem.write_u32(p + o, w);
            }
            fx.mem.set16(p + 0xE, 0);
            fx.mem.write_u8(p + 0xC, 0x40);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0x12C {
        2
    } else if m == 0xA3 {
        1
    } else if (0xAD..0x12C).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
