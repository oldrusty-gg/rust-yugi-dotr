//! BMODULE b143 (card 97), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02500, data area 0x1ED0000) [F] (helper D10, 31
//! Oct 2026): 512 sprites (`blib::flatsprite::sprites`, +0x150) falling
//! round the attacker's node 0x23 (+0xE0, turned to look at the target)
//! from a ring of the target's radius, drawn in by 0.8 above the ground
//! and out by 1.12 below it, respawned below -200; their bob amplitude
//! (+0x1A4) swinging between -30 and 30 by 1.5; 60 spinning motes
//! (+0x3280) at 1.5 x the first 60 sprites' places; a puff of particles
//! (+0x39B0) on 21..49; the frame quad from update 111; at 238 the
//! target's added colour cleared. Returns 1 on update 78, 6 on 88..237, 2
//! on 238, 8 when the 250 updates are over.
//!
//! Library (record addresses): `colours4` 0x1E00080, the frame quad
//! 0x1E00120, `motes` 0x1E00240, `flatsprite::sprites` 0x1E009C0,
//! `particle_quads` 0x1E01970; the step 0x1E02B50 (local).
use crate::b107::target_radius;
use crate::b142::fstep_up;
use crate::b436::fstep_down;
use crate::blib::{flatsprite, spin};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const N: u32 = D + 0x3B04;
const LEFT: u32 = D + 0x3B00;
const SPRITES: u32 = D + 0x150;
const S_PARTS: u32 = D + 0x1B0;
const MOTES: u32 = D + 0x3280;
const CLOUD: u32 = D + 0x39B0;
const BITS: u32 = D + 0x3B0C;
const TENTH: u32 = 0x3DCC_CCCD;

/// The entry 0x1E02500.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if param < 0 { step(fx, sp) } else { init(fx, sp, param) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3B20);
    for x in [0u32, 0x70, 0xE0, 0x3940] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x3B10u32, 0x23, 1), (0x3B14, 0x1A, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(LEFT, 0xFA);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x3B08, param);
    let r = target_radius(fx);
    fx.mem.setf(D + 0x3B18, r);
    fx.mem.write_u8(D + 0x3B1C, 0x80);
    fx.mem.write_u32(BITS, 0);
    fx.texture4(sp + 0x80, 0xF);
    // The puff (+0x39B0): 16 particles, half size 5.
    let g = CLOUD;
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x8_0083);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    for (o, x) in [(0x28u32, 0x80i32), (0x2A, 0xC0), (0x24, 0x20), (0x26, 0x20), (0x2C, 0x80), (0x2E, 0x40)] {
        fx.mem.set16(g + o, x);
    }
    fx.mem.write_u32(g + 0xC, 0x40A0_0000);
    for (o, x) in [(0x130u32, 0x10i32), (0x13A, 2), (0x13C, 1), (0x140, 8), (0x136, 0), (0x138, 0)] {
        fx.mem.set16(g + o, x);
    }
    for (o, x) in [(0xAu32, 0x70u8), (9, 0xA0), (8, 0xA0), (0xB, 0x20)] {
        fx.mem.write_u8(g + o, x);
    }
    for k in 0..0x10i32 {
        let p = D + 16 * k as u32;
        for (c, ra) in [(0u32, 0x01E0_2664u32), (1, 0x01E0_2698)] {
            let r = fx.rand_from(k as u32, ra);
            let (_, h) = fx.div(r, 6);
            let v = ee::add(fx.mem.f(g + 0x10 + 4 * c), ee::from_i32(h - 3));
            fx.mem.setf(p + 0x39E0 + 4 * c, v);
        }
        let z = fx.mem.read_u32(g + 0x18);
        fx.mem.write_u32(p + 0x39E8, z);
        let a = -fx.mem.i16(g + 0x13A);
        let f = fx.mult(a, k);
        fx.mem.set16(p + 0x39EE, f);
        fx.mem.set16(p + 0x39EC, 0);
        let t = fx.mem.i16(g + 0x140) - 1;
        fx.mem.set16(g + 0x13E, t);
    }
    // The sprites (+0x150): flags 0x58000003, alpha cap 0x40, half size
    // 2 growing 10 an age, bobbing 50 at 0x400 an age, 512 at +0x1B0.
    fx.texture5(sp + 0x80, 0);
    let g = SPRITES;
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x5800_0003);
    for (o, x) in [(0xAu32, 0x70u8), (9, 0xA0), (8, 0xA0), (0xB, 0x40)] {
        fx.mem.write_u8(g + o, x);
    }
    fx.mem.write_u32(g + 0xC, 0x41F0_0000);
    for (o, x) in [(0x20u32, 0u32), (0x24, 0x1000), (0x28, 0)] {
        fx.mem.write_u32(g + o, x);
    }
    for (o, x) in [(0x30u32, 0i32), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)] {
        fx.mem.set16(g + o, x);
    }
    fx.mem.write_u32(g + 0x40, S_PARTS);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4000_0000);
    fx.mem.write_u32(g + 0x50, 0x4120_0000);
    fx.mem.write_u32(g + 0x54, 0x4248_0000);
    for (o, x) in [(0x58u32, 0x400i32), (0x5A, 0x200), (0x5C, 8), (0x5E, 0x28)] {
        fx.mem.set16(g + o, x);
    }
    // rand's saved s0: the puff loop's count (16), then each angle.
    let mut s0 = 0x10u32;
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = S_PARTS + 16 * k as u32;
        let a = fx.rand_from(s0, 0x01E0_27D8) & 0xFFFF;
        s0 = a as u32;
        let r = fx.rand_from(s0, 0x01E0_27F4);
        let (_, h) = fx.div(r, 0x7D0);
        fx.mem.setf(p + 4, ee::mul(f32::from_bits(TENTH), ee::neg(ee::from_i32(h))));
        let c = fx.tcos(a);
        let v = ee::mul(fx.mem.f(g + 0x4C), c);
        fx.mem.setf(p + 8, v);
        let s = fx.tsin(a);
        let v = ee::mul(fx.mem.f(g + 0x4C), s);
        fx.mem.setf(p, v);
        fx.mem.set16(p + 0xE, 0);
        fx.mem.write_u8(p + 0xC, 0);
        let r = fx.rand_from(s0, 0x01E0_2858);
        fx.mem.set8(p + 0xD, r % 8);
        k += 1;
    }
    // The motes (+0x3280): flags 0x40001001, half size 1, cell (0, 0,
    // 0x40, 0x80), white, alpha 0, turned by (0, 0x400, 0).
    fx.texture5(sp + 0x80, 1);
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(MOTES, tex);
    fx.mem.write_u32(MOTES + 4, 0x4000_1001);
    fx.mem.write_u32(MOTES + 0xC, 0x3F80_0000);
    for (o, x) in [(0x20u32, 0i32), (0x22, 0), (0x24, 0x40), (0x26, 0x80)] {
        fx.mem.set16(MOTES + o, x);
    }
    for (o, x) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, 0)] {
        fx.mem.write_u8(MOTES + o, x);
    }
    for (o, x) in [(0x10u32, 0u32), (0x14, 0x400), (0x18, 0)] {
        fx.mem.write_u32(MOTES + o, x);
    }
    for k in 0..0x3Cu32 {
        let s0 = D + 0xC * k;
        fx.rand_from(s0, 0x01E0_28F8);
        let q = MOTES + 0x3F0 + 0xC * k;
        let p = S_PARTS + 16 * k;
        for (o, src) in [(4u32, 4u32), (8, 8), (0, 0)] {
            let w = fx.mem.read_u32(p + src);
            fx.mem.write_u32(q + o, w);
        }
        for (o, ra) in [(0u32, 0x01E0_2914u32), (4, 0x01E0_2930), (8, 0x01E0_294C)] {
            let a = fx.rand_from(s0, ra) & 0xFFFF;
            fx.mem.set32(MOTES + 0x30 + 0x10 * k + o, a);
        }
    }
    0
}

/// 0x1E02B50 (D, ot): the bob swing, the sprites drawn and moved, the
/// motes following them.
fn sprites_step(fx: &mut Fx) {
    let sp = fx.enter(0x70);
    if bit(fx.mem.read_u32(BITS), 0x1F) {
        if fstep_down(fx, D + 0x1A4, -30.0, 1.5) == 0 {
            let w = fx.mem.read_u32(BITS) & 0x7FFF_FFFF;
            fx.mem.write_u32(BITS, w);
        }
        fstep_down(fx, D + 0x2274, -30.0, 1.5);
    } else {
        if fstep_up(fx, D + 0x1A4, 30.0, 1.5) == 0 {
            let w = fx.mem.read_u32(BITS) | 0x8000_0000;
            fx.mem.write_u32(BITS, w);
        }
        fstep_up(fx, D + 0x2274, 30.0, 1.5);
    }
    flatsprite::sprites(fx, OT, SPRITES, D + 0xE0);
    let mut k = 0;
    while k < fx.mem.i16(SPRITES + 0x5A) {
        let p = S_PARTS + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        let y = ee::sub(fx.mem.f(p + 4), 6.0);
        fx.mem.setf(p + 4, y);
        let f = if !ee_le(y, 0.0) { f32::from_bits(0x3F4C_CCCD) } else { f32::from_bits(0x3F8F_5C29) };
        let z = ee::mul(fx.mem.f(p + 8), f);
        fx.mem.setf(p + 8, z);
        let x = ee::mul(fx.mem.f(p), f);
        fx.mem.setf(p, x);
        if !ee_lt(fx.mem.f(p + 4), -200.0) {
            continue;
        }
        let a = fx.rand_from(D, 0x01E0_2CF0) & 0xFFFF;
        fx.mem.set16(p + 0xE, 0);
        let c = fx.tcos(a);
        let v = ee::mul(fx.mem.f(SPRITES + 0x4C), c);
        fx.mem.setf(p + 8, v);
        let s = fx.tsin(a);
        let v = ee::mul(fx.mem.f(SPRITES + 0x4C), s);
        fx.mem.setf(p, v);
        let r = fx.rand_from(D, 0x01E0_2D34);
        let (_, h) = fx.div(r, 0xC8);
        fx.mem.setf(p + 4, ee::mul(f32::from_bits(TENTH), ee::neg(ee::from_i32(h))));
        let a = if fx.mem.i32(N) >= 0x2B { 0x40 } else { 0 };
        fx.mem.write_u8(p + 0xC, a);
    }
    if fx.mem.i32(N) >= 0xEF {
        fx.step_down(SPRITES + 0xB, 0, 0x10);
    }
    for o in [0x64u32, 0x68, 0x60] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    spin::motes(fx, OT, MOTES, D + 0xE0, sp + 0x60);
    if fx.mem.i32(N) == 0x2B {
        fx.mem.write_u8(MOTES + 0xB, 0x80);
    }
    if fx.mem.i32(N) >= 0xEE {
        fx.step_down(MOTES + 0xB, 0, 8);
    }
    for k in 0..0x3Cu32 {
        let p = S_PARTS + 16 * k;
        let q = MOTES + 0x3F0 + 0xC * k;
        let v = ee::mul(1.5, fx.mem.f(p + 4));
        fx.mem.setf(q + 4, v);
        let e = fx.mem.read_u32(SPRITES + 0x40).wrapping_add(16 * k);
        let (fq, age) = (fx.mem.i16(SPRITES + 0x58), fx.mem.i16(e + 0xE));
        let fa = fx.mult(fq, age);
        let s = fx.tsin(fa);
        let bob = ee::mul(fx.mem.f(SPRITES + 0x54), s);
        let v = ee::sub(fx.mem.f(p + 8), bob);
        fx.mem.setf(q + 8, v);
        let v = ee::mul(1.5, fx.mem.f(p));
        fx.mem.setf(q, v);
        for o in [0u32, 4, 8] {
            let a = MOTES + 0x30 + 0x10 * k + o;
            let v = fx.mem.read_u32(a).wrapping_add(0x1000);
            fx.mem.write_u32(a, v);
        }
    }
    fx.leave(0x70);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    let node = |fx: &mut Fx, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x3B10);
        fx.node_position(h, s, out);
    };
    node(fx, D + 0x3960);
    node(fx, D + 0x100);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x50);
    node(fx, sp + 0x60);
    fx.look_angles(sp + 0x60, sp + 0x50, D + 0xF0);
    // s0 = the OT, s1 = sp + 0x60, s2 = D, s3 the runner's (0).
    let regs = [Some(OT), Some(sp + 0x60), Some(D), Some(0)];
    fx.xform_world_saving(D, 0x01E0_29F8, regs);
    fx.xform_billboard_saving(D + 0x70, 0x01E0_2A00, regs);
    fx.xform_billboard_saving(D + 0x3940, 0x01E0_2A08, regs);
    fx.xform_world_saving(D + 0xE0, 0x01E0_2A10, regs);
    if n(fx) >= 0x6F {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
        // 0x207EF0's frame below the frame quad's (sq ra 0x1E0020C), read
        // as the sprites' missing-texture TEX1 (as b486).
        fx.saved(sp - 0x80, &[(0, 0x01E0_020C)]);
    }
    let m = n(fx);
    if (0x15..0x32).contains(&m) {
        for o in [0x78u32, 0x74, 0x70] {
            fx.mem.write_u32(sp + o, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0x3940, CLOUD, sp + 0x70);
        if n(fx) >= 0x22 {
            fx.step_down(CLOUD + 0xB, 0, 2);
        }
    }
    sprites_step(fx);
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0xEE {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, 0, [OT, sp + 0x60], 0x01E0_2AD4);
        2
    } else if m == 0x4E {
        1
    } else if (0x58..0xEE).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
