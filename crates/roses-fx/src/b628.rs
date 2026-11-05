//! BMODULE b628 (card 263), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04690, data area 0x1ED0000) [F] (helper C4, 31
//! Oct 2026): b185's family with a quad trail: on update 53 two rings of
//! the trail (+0x1E10, `blib::quadtrail`) pushed at the attacker's nodes
//! 0x2D / 0x34 and drawn every update; a glow at node 0x19 (disc +0x380,
//! wobbling quad burst +0x19B0) from update 70, flaring on update 140;
//! from update 140 a ribbon trail (+0x70) toward the target (1 / 16 of the
//! way per update, ...), from 141 a fan of quads (+0x20D0) turned toward
//! the target and a rising sprite fountain (+0x8E0); at the target from
//! update 156 a disc / quad burst growing to 1.5; the frame quad from
//! update 120; the stage light down until update 252. Returns 1 on update
//! 146, 6 on 156..251, 2 on 252, 8 after 270.
//!
//! Library: the frame quad (0x1E00120), `sprites` (0x1E00240), `disc`
//! (0x1E01B30), `quads_setup` / `quads_draw` (0x1E02CA0 / 0x1E02E00)
//! [`crate::fxlib`], `quadtrail::push` / `draw` (0x1E02090 / 0x1E02290,
//! [`crate::blib::quadtrail`]), the ribbon (0x1E03410 / 0x1E03560,
//! [`crate::ribbon`]), the fan (0x1E03F20 / 0x1E04080, [`crate::fan`]).
use crate::blib::quadtrail;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad / fan corner templates (rodata, 0x1E02CA0's / 0x1E03F20's
/// lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_54C0;
const FAN_TEMPLATE: u32 = 0x01E0_5500;
/// `quadtrail::draw` (its template is record-relative to it).
const TRAIL_DRAW: u32 = 0x01E0_2290;

/// The entry 0x1E04690.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn disc_record(fx: &mut Fx, at: u32, radius: u32) {
    fx.mem.write_u32(at, 0x8_0000);
    fx.mem.write_u8(at + 6, 0xA0);
    fx.mem.write_u8(at + 5, 0xD0);
    fx.mem.write_u8(at + 4, 0x70);
    fx.mem.write_u8(at + 7, 0);
    fx.mem.write_u32(at + 8, radius);
    fx.mem.set16(at + 0xC, 0x24);
    fx.mem.set16(at + 0xE, 0);
}

/// A fountain sprite (`p` = D + 16 k, fields at +0x940) set off (as b185's:
/// the radius modulus 10 x +0x92C, an angle, a height -(h - 20) / 10).
pub(crate) fn fountain_spawn(fx: &mut Fx, p: u32, ra: u32, rands: [u32; 4]) {
    let r = fx.mem.f(D + 0x92C);
    let f = ee::mul(10.0, r);
    fx.fix_residue(f, ra);
    let m = ee::fixsfsi(f);
    let a = fx.rand_from(D, rands[0]) & 0xFFFF;
    let x = fx.rand_from(D, rands[1]);
    let (_, h) = fx.div(x, m);
    let tenth = f32::from_bits(0x3DCC_CCCD);
    let radius = ee::mul(tenth, ee::from_i32(h));
    let x = fx.rand_from(D, rands[2]);
    let (_, h) = fx.div(x, 0x28);
    fx.mem.setf(p + 0x944, ee::mul(tenth, ee::neg(ee::from_i32(h - 0x14))));
    let c = fx.tcos(a);
    fx.mem.setf(p + 0x948, ee::mul(radius, c));
    let s = fx.tsin(a);
    fx.mem.setf(p + 0x940, ee::mul(radius, s));
    let x = fx.rand_from(D, rands[3]);
    let (_, h) = fx.div(x, 0xC);
    fx.mem.set16(p + 0x94E, -h);
    fx.mem.write_u8(p + 0x94C, 0x80);
}

fn quad_group(fx: &mut Fx, g: u32, flags: u32, tex: u32, size: f32) {
    fx.mem.write_u32(g + 4, flags);
    for (k, x) in [(0x10u32, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u8(g + 0xA, 0xA0);
    fx.mem.write_u8(g + 9, 0xD0);
    fx.mem.write_u8(g + 8, 0x70);
    fx.mem.write_u8(g + 0xB, 0x20);
    fx.quads_setup(g, tex, size, QUAD_TEMPLATE);
}

pub(crate) fn halves(fx: &mut Fx, g: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(g + k, x);
    }
}

/// The quad trail `g` (b628 / b184 +0x1E10): up to 4 rows of 8 points at
/// `g` + 0x50, ring radius 3.3, cell (128, 192, 32, 32), drift (0, 0.3, 0),
/// half size 3, growth 0.2, flags 0x1_9003, new points' alpha 0x30.
pub(crate) fn trail_group(fx: &mut Fx, g: u32, tex: u32) {
    fx.mem.write_u32(g, tex);
    for k in [6u32, 5, 4] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 7, 0x30);
    fx.mem.write_u32(g + 8, 0x1_9003);
    fx.mem.write_u32(g + 0xC, g + 0x50);
    halves(fx, g, &[(0x10, 0x80), (0x12, 0xC0), (0x14, 0x20), (0x16, 0x20), (0x18, 0x80), (0x1A, 0xC0), (0x1C, 0x80), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0x20, 0);
    fx.mem.write_u32(g + 0x24, 0x3E99_999A);
    fx.mem.write_u32(g + 0x28, 0);
    fx.mem.write_u32(g + 0x30, 0x4040_0000);
    fx.mem.write_u32(g + 0x34, 0x3E4C_CCCD);
    fx.mem.write_u32(g + 0x38, 0x4053_3333);
    halves(fx, g, &[(0x42, 0), (0x44, 0x18), (0x46, 4), (0x48, 0), (0x4A, 8), (0x4C, 8), (0x4E, 0)]);
}

/// Every point's alpha of the trail `g` down by 3 (rows in use +0x48,
/// points a ring +0x4A; the `mult` in the inner test's delay slot kept).
pub(crate) fn trail_fade(fx: &mut Fx, g: u32) {
    let mut r = 0;
    while r < i32::from(fx.mem.i16(g + 0x48)) {
        let mut c = 0;
        loop {
            let cols = i32::from(fx.mem.i16(g + 0x4A));
            let k = fx.mult(r, cols);
            if c >= cols {
                break;
            }
            fx.step_down(g + 0x5C + 16 * (c + k) as u32, 0, 3);
            c += 1;
        }
        r += 1;
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2580);
    for x in [0u32, 0x2A0, 0x870, 0x310, 0x390, 0x1940, 0x2060] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2564u32, 0x19, 1), (0x2568, 0x18, 0), (0x256C, 0x34, 0), (0x2570, 0x2D, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2540, 0x10E);
    fx.mem.set32(D + 0x2544, 1);
    fx.mem.set32(D + 0x2548, param);
    fx.mem.set32(D + 0x2550, 0x10);
    fx.mem.set32(D + 0x254C, 0x80);
    disc_record(fx, D + 0x380, 0x40A0_0000);
    disc_record(fx, D + 0x860, 0x41F0_0000);
    fx.texture5(sp + 0xB0, 0);
    // The ribbon (+0x70): 32 slots, life 72, width 40, cell (0, 0, 16,
    // 32); points at +0xA0.
    let g = D + 0x70;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 8, 0x8003);
    for k in [6u32, 5, 4, 7] {
        fx.mem.write_u8(g + k, 0x80);
    }
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x10), (0x16, 0x20), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, 0x4220_0000);
    halves(fx, g, &[(0x22, 8), (0x24, 0), (0x26, 0x48), (0x28, 0x20), (0x2A, 0), (0x2C, 1)]);
    // The fan (+0x20D0): cell (64, 0, 64, 64), half widths 0.5 / 1,
    // height 8, size 3.
    let g = D + 0x20D0;
    fx.mem.write_u32(g + 4, 1);
    halves(fx, g, &[(0x10, 0x40), (0x12, 0), (0x14, 0x40), (0x16, 0x40)]);
    for k in [0xAu32, 9, 8] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 0xB, 0x10);
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x3F80_0000);
    fx.mem.write_u32(g + 0x28, 0x4100_0000);
    fx.mem.write_u32(g + 0x2C, 0);
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.fan_setup(g, tex, 3.0, FAN_TEMPLATE);
    // The fountain (+0x8E0): 256 sprites, size 1, radius 5.
    let g = D + 0x8E0;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0xC, 0x3F80_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0x20), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0x20), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x940);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x40A0_0000);
    halves(fx, g, &[(0x5A, 0x100), (0x5C, 1), (0x5E, 0)]);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x93A) {
        fountain_spawn(fx, D + 16 * k as u32, 0x01E0_497C, [0x01E0_4984, 0x01E0_49A0, 0x01E0_49C8, 0x01E0_4A20]);
        k += 1;
    }
    fx.texture4(sp + 0xB0, 0xF);
    let tex = fx.mem.read_u32(sp + 0xB0);
    quad_group(fx, D + 0x400, 0x8_0001, tex, 4.0);
    let tex = fx.mem.read_u32(sp + 0xB0);
    quad_group(fx, D + 0x19B0, 1, tex, 0.5);
    for k in 0..64u32 {
        let q = D + 16 * k;
        let r = fx.rand();
        fx.mem.set32(q + 0x1A10, (r & 0x3FFF) - 0x2000);
        let r = fx.rand();
        fx.mem.set32(q + 0x1A14, r & 0xFFFF);
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x6000);
        fx.mem.set32(q + 0x1A18, h - 0x3000);
    }
    let tex = fx.mem.read_u32(sp + 0xB0);
    trail_group(fx, D + 0x1E10, tex);
    for x in [0x390u32, 0x1940] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, 0x3DCC_CCCD);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2544);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    if n(fx) == 0x35 {
        node(fx, 0x2570, sp + 0x80);
        quadtrail::push(fx, D + 0x1E10, sp + 0x80);
        node(fx, 0x256C, sp + 0x80);
        quadtrail::push(fx, D + 0x1E10, sp + 0x80);
    }
    node(fx, 0x2564, sp + 0x90);
    node(fx, 0x2568, sp + 0xA0);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x880);
    for k in 0..4u32 {
        let w = fx.mem.read_u32(D + 0x880 + 4 * k);
        fx.mem.write_u32(D + 0x1950 + 4 * k, w);
    }
    fx.midpoint(sp + 0x90, sp + 0xA0, D + 0x1960);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x2554);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x3B0);
    node(fx, 0x2564, D + 0x890);
    node(fx, 0x2564, D + 0x330);
    if n(fx) < 0x8D {
        node(fx, 0x2564, D + 0x2C0);
    }
    if n(fx) >= 0x8C {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x2C0);
        let m = fx.mem.i32(D + 0x2550);
        fx.approach_by(D + 0x2C0, D + 0x2554, m);
        let m = fx.mem.i32(D + 0x2550);
        if m != 0 {
            fx.mem.set32(D + 0x2550, m - 1);
        }
    }
    if n(fx) == 0x8C {
        let t = fx.target_handle();
        fx.unit_position(t, sp + 0x90);
        node(fx, 0x2564, sp + 0xA0);
        fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x2070);
        for k in 0..4u32 {
            let w = fx.mem.read_u32(sp + 0xA0 + 4 * k);
            fx.mem.write_u32(D + 0x2080 + 4 * k, w);
        }
    }
    // s0 = D, s1 = the OT, s2 = sp + 0xA0.
    let regs = [Some(D), Some(OT), Some(sp + 0xA0), None];
    fx.xform_world_saving(D, 0x01E0_4E78, regs);
    fx.xform_billboard_saving(D + 0x2A0, 0x01E0_4E80, regs);
    fx.xform_world_saving(D + 0x870, 0x01E0_4E88, regs);
    fx.xform_billboard_saving(D + 0x310, 0x01E0_4E90, regs);
    fx.xform_billboard_saving(D + 0x390, 0x01E0_4E98, regs);
    fx.xform_world_saving(D + 0x1940, 0x01E0_4EA0, regs);
    fx.xform_world_saving(D + 0x2060, 0x01E0_4EA8, regs);
    if n(fx) >= 0x78 {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0xFC {
        fx.step_down(D + 0x254C, 0x20, 4);
    } else {
        fx.step_up(D + 0x254C, 0x80, 4);
    }
    let light = fx.mem.u8(D + 0x254C);
    fx.set_stage_light(light);
    let step20 = f32::from_bits(0x3D4C_CCCD);
    if n(fx) >= 0x9C {
        fx.approach(D + 0x390, 1.5, step20);
        fx.disc(D + 0x860, D + 0x390);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_4F40)]);
        fx.quads_draw(D + 0x400, D + 0x390);
        if n(fx) >= 0xFC {
            fx.step_down(D + 0x867, 0, 0x10);
            fx.step_down(D + 0x40B, 0, 2);
        } else {
            fx.step_up(D + 0x867, 0xFF, 8);
        }
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0x460 + 16 * k + 4 * c;
                let x = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, x);
            }
        }
    }
    let m = n(fx);
    if m >= 0x8C {
        if m == 0x8C {
            fx.mem.write_u8(D + 0x387, 0xFF);
        }
        fx.disc(D + 0x380, D + 0x310);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_500C)]);
        fx.step_down(D + 0x387, 0, 2);
    }
    let m = n(fx);
    if (0x46..0x8C).contains(&m) {
        fx.disc(D + 0x380, D + 0x310);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_5048)]);
        fx.quads_draw(D + 0x19B0, D + 0x1940);
        if fx.approach(D + 0x1940, 1.0, step20) == 0 {
            fx.step_down(D + 0x19BB, 0, 1);
            fx.step_down(D + 0x387, 0, 4);
        } else {
            fx.step_up(D + 0x387, 0xFF, 8);
        }
        for k in 0..64u32 {
            let a = D + 0x1A14 + 16 * k;
            let x = fx.mem.i32(a);
            fx.mem.set32(a, if k % 2 == 0 { x.wrapping_add(0x80) } else { x.wrapping_sub(0x80) });
        }
    }
    if n(fx) >= 0x8D {
        fx.sprites(D + 0x8E0, D + 0x870);
        fx.step_down(D + 0x8EB, 0, 8);
        let grow = f32::from_bits(0x3F8C_CCCD);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x93A) {
            let p = D + 16 * k as u32;
            if fx.mem.i16(p + 0x94E) >= 0 {
                let y = ee::add(fx.mem.f(p + 0x944), 2.0);
                fx.mem.setf(p + 0x944, y);
                let x = ee::mul(fx.mem.f(p + 0x940), grow);
                fx.mem.setf(p + 0x940, x);
                let z = ee::mul(fx.mem.f(p + 0x948), grow);
                fx.mem.setf(p + 0x948, z);
                fx.step_down(p + 0x94C, 0, 8);
                if !ee_le(fx.mem.f(p + 0x944), 20.0) {
                    fountain_spawn(fx, p, 0x01E0_51CC, [0x01E0_51D4, 0x01E0_51F0, 0x01E0_5218, 0x01E0_5270]);
                }
            }
            k += 1;
        }
    }
    if n(fx) >= 0x8D {
        fx.fan_draw(D + 0x20D0, D + 0x2060);
        fx.step_down(D + 0x20DB, 0, 1);
        fx.approach(D + 0x2060, 5.0, f32::from_bits(0x3E4C_CCCD));
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
    }
    quadtrail::draw(fx, TRAIL_DRAW, OT, D + 0x1E10, D);
    trail_fade(fx, D + 0x1E10);
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2544, m);
    let left = fx.mem.i32(D + 0x2540) - 1;
    fx.mem.set32(D + 0x2540, left);
    if m == 0xFC {
        2
    } else if m == 0x92 {
        1
    } else if (0x9C..0xFC).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
