//! BMODULE b049 (card 397), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E032E0, data area 0x1ED0000, 0x1700 bytes) [F]
//!: b028's family (card 213) without its quad
//! trail: from update 0x32 rain (+0xFD0, `rain`, 64 streaks falling 3 a
//! step), a cloud of 60 squares (+0x13E0, `pointquad::squares`, half size
//! 0.5) spreading by 1.03, two rings (+0x760 / +0xB60), a point-set strip
//! (+0x700 / look +0x660, `pointquad::draw`) and a quad burst (+0x200)
//! growing by 0.4 to 1 then by 0.005 while everything fades; on 0x2C..0x37
//! the two-history ribbon (+0x70, `ribbon8::ribbon`) between the
//! attacker's nodes 0x1A and 0x19 (both histories kept at the nodes
//! otherwise); the rain / squares / rings follow node 0x1B below 0x33.
//! Returns 1 on update 0x28, 3 on 0x2F, 8 once the 100-update timer
//! +0x16E0 has run out.
//!
//! Library: `rain::colours2` 0x1E00080, `ribbon8::strip_colours`
//! 0x1E00100, `pointquad::setup` / `draw` / `squares` (0x1E00270 /
//! 0x1E005B0 / 0x1E01190), `ring_setup` / `ring_draw` (0x1E01770 /
//! 0x1E01A30), `quads_setup` / `quads_draw` (0x1E01F20 / 0x1E02080),
//! `ribbon8::ribbon` 0x1E02690, `rain::setup` / `draw` (0x1E02B30 /
//! 0x1E02D80).
use crate::b028::{look_init, square_points};
use crate::b436::ring_group;
use crate::b628::halves;
use crate::blib::{pointquad, rain, ribbon8};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad corner template (rodata, 0x1E01F20's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_3C80;
const RAIN: u32 = D + 0xFD0;
const SQUARES: u32 = D + 0x13E0;
const QUADS: u32 = D + 0x200;
const RING_A: u32 = D + 0x760;
const RING_B: u32 = D + 0xB60;
const LOOK: u32 = D + 0x700;
const POINTS: u32 = D + 0x660;
const RIBBON: u32 = D + 0x70;
const TIMER: u32 = D + 0x16E0;
const COUNTER: u32 = D + 0x16E4;
const NODES: u32 = D + 0x16EC;

/// The entry 0x1E032E0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x70);
    ret
}

fn node(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(NODES + 4 * k);
    fx.node_position(h, s, out);
}

/// Both ribbon histories (8 points) at sp + 0x40 / sp + 0x50.
fn hold(fx: &mut Fx, sp: u32) {
    for k in 0..8u32 {
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * c);
            fx.mem.write_u32(RIBBON + 0x10 + 0x10 * k + 4 * c, w);
            let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
            fx.mem.write_u32(RIBBON + 0x90 + 0x10 * k + 4 * c, w);
        }
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1700);
    for x in [0u32, 0x190, 0xF60] {
        fx.xform_init(D + x);
    }
    for (k, node, clear) in [(0u32, 0x1B, 1), (1, 0x1A, 0), (2, 0x19, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(NODES + 4 * k, r);
    }
    fx.mem.set32(TIMER, 0x64);
    fx.mem.set32(COUNTER, 1);
    fx.mem.set32(D + 0x16E8, param);
    for k in [0x16D0u32, 0x16D4, 0x16D8] {
        fx.mem.write_u32(D + k, 0x3F83_D70A);
    }
    // The rain (+0xFD0): 64 streaks, speed 10.
    fx.mem.write_u32(RAIN, 0x4000_0001);
    for (k, x) in [(6u32, 0x80u8), (5, 0xA0), (4, 0xFF), (7, 0x80)] {
        fx.mem.write_u8(RAIN + k, x);
    }
    rain::setup(fx, 0x01E0_2B30, RAIN, 0x40, 10.0);
    fx.texture4(sp + 0x60, 0xF);
    // The squares (+0x13E0): 60 points, half size 0.5.
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.mem.write_u32(SQUARES, tex);
    fx.mem.write_u32(SQUARES + 0xC, 0x3F00_0000);
    square_points(fx, SQUARES);
    fx.mem.write_u32(SQUARES + 4, 1);
    halves(fx, SQUARES, &[(0x10, 0xB0), (0x12, 0x40), (0x14, 0x20), (0x16, 0x20)]);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0xA0), (8, 0xFF), (0xB, 0x80)] {
        fx.mem.write_u8(SQUARES + k, x);
    }
    // The quad burst (+0x200): size 1.6.
    fx.mem.write_u32(QUADS + 4, 0x8_0001);
    halves(fx, QUADS, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    for (k, x) in [(0xAu32, 0xD0u8), (9, 0xA0), (8, 0x70), (0xB, 0x20)] {
        fx.mem.write_u8(QUADS + k, x);
    }
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.quads_setup(QUADS, tex, f32::from_bits(0x3FCC_CCCD), QUAD_TEMPLATE);
    // Two rings (+0x760: radii 12 / 11.2; +0xB60: 12 / 12.8).
    ring_group(fx, RING_A, 0x2008_0000, 0x40, [0x70, 0xA0, 0xD0, 0x20]);
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.ring_setup(RING_A, tex, 12.0, f32::from_bits(0x4133_3333), 0.0, 0.0);
    ring_group(fx, RING_B, 0x2008_0000, 0x40, [0x70, 0xA0, 0xD0, 0x30]);
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.ring_setup(RING_B, tex, 12.0, f32::from_bits(0x414C_CCCD), 0.0, 0.0);
    // The point set's look (+0x700), as b028's.
    look_init(fx, LOOK);
    fx.mem.write_u32(POINTS + 8, 1);
    let tex = fx.mem.read_u32(sp + 0x60);
    pointquad::setup(fx, 0x01E0_0270, LOOK, POINTS, tex, 20.0);
    // The ribbon (+0x70): colour (B, G, R, A) (0xFF, 0xA0, 0x80, 0x80).
    fx.mem.write_u32(RIBBON + 4, 0);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0xA0), (8, 0xFF), (0xB, 0x80)] {
        fx.mem.write_u8(RIBBON + k, x);
    }
    node(fx, 1, sp + 0x40);
    node(fx, 2, sp + 0x50);
    hold(fx, sp);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    node(fx, 1, sp + 0x40);
    node(fx, 2, sp + 0x50);
    let n = |fx: &mut Fx| fx.mem.i32(COUNTER);
    if n(fx) < 0x33 {
        node(fx, 0, D + 0x1B0);
        node(fx, 0, D + 0xF80);
    }
    // s0 = the OT, s1 = sp + 0x50, s2 = D, s3 the caller's.
    let regs = [Some(OT), Some(sp + 0x50), Some(D), None];
    fx.xform_world_saving(D, 0x01E0_3854, regs);
    for (x, ra) in [(0x190u32, 0x01E0_385C), (0xF60, 0x01E0_3864)] {
        fx.xform_billboard_saving(D + x, ra, regs);
    }
    if n(fx) >= 0x32 {
        if ee_lt(fx.mem.f(D + 0xF60), 1.0) {
            let v = ee::add(fx.mem.f(D + 0xF68), 0.5);
            for k in [0xF68u32, 0xF64, 0xF60] {
                fx.mem.setf(D + k, v);
            }
        }
        rain::draw(fx, 0x01E0_2D80, OT, RAIN, D + 0xF60, -3.0);
        fx.step_down(RAIN + 7, 0, 0xA);
        pointquad::squares(fx, OT, SQUARES, D + 0xF60, D + 0x16D0);
        fx.step_down(SQUARES + 0xB, 0, 0xA);
        fx.ring_draw(RING_A, D + 0x190);
        fx.ring_draw(RING_B, D + 0x190);
        pointquad::draw(fx, OT, LOOK, POINTS, D + 0x190);
        fx.quads_draw(QUADS, D + 0x190);
        let grow = ee_le(fx.mem.f(D + 0x190), 1.0);
        let step = f32::from_bits(if grow { 0x3ECC_CCCD } else { 0x3BA3_D70A });
        for k in [0x190u32, 0x194, 0x198] {
            let v = ee::add(fx.mem.f(D + k), step);
            fx.mem.setf(D + k, v);
        }
        if !grow {
            for (k, s) in [(0x20Bu32, 6), (0x76B, 6), (0xB6B, 6), (0x737, 0xC)] {
                fx.step_down(D + k, 0, s);
            }
        }
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0x260 + 16 * k + 4 * c;
                let x = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, x);
            }
        }
    }
    if (0x2C..0x38).contains(&n(fx)) {
        ribbon8::ribbon(fx, RIBBON, D, sp + 0x40, sp + 0x50);
    } else {
        hold(fx, sp);
    }
    let m = n(fx) + 1;
    fx.mem.set32(COUNTER, m);
    let left = fx.mem.i32(TIMER) - 1;
    fx.mem.set32(TIMER, left);
    if m == 0x2F {
        3
    } else if m == 0x28 {
        1
    } else if left > 0 {
        0
    } else {
        8
    }
}
