//! BMODULE b741 (card 428), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02E50, data area 0x1ED0000) [F] (helper C3, 31
//! Oct 2026): b740's family ([`crate::b740`]). From update 78 a sprite
//! burst (+0x2E0, 64 sprites thrown out 14) on the target, its flags
//! gaining 0x80000 on update 140; on updates 44..167 eight jittered lines
//! (`zap::zap`, +0x70 + 0x40 k) from the attacker's node 0xB to a ring of
//! radius 20 round the target, and a spark fountain (+0x7B0, 128 sparks)
//! respawned on a ring when above 50. Returns 1 on update 68, 6 on
//! 78..259, 2 on 260, 8 after 275.
//!
//! Library: `sprites` (0x1E00120) [`crate::fxlib`]; `spawn::scatter`
//! (0x1E01A10), `trail::sparks` (0x1E01BD0), `zap::zap` (0x1E02930)
//! [`crate::blib`].
use crate::b740::zaps_init;
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E02E50.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xD0);
    ret
}

fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

fn throw(fx: &mut Fx, sp: u32, p: u32) {
    fx.random_dir(sp + 0x90, 14.0);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x90 + 4 * c);
        fx.mem.write_u32(p + 4 * c, w);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1000);
    for x in [0u32, 0x270, 0x740] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0xFF0u32, 0xC, 1), (0xFF4, 0xB, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0xFE0, 0x114);
    fx.mem.set32(D + 0xFE4, 1);
    fx.mem.set32(D + 0xFE8, param);
    fx.mem.set32(D + 0xFEC, 0);
    fx.texture5(sp + 0xC0, 0);
    // The burst (+0x2E0): 64 sprites at +0x340, size 20.
    let g = D + 0x2E0;
    let tex = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x4800_0003);
    for k in [0xAu32, 9, 8] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 0xB, 0);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x340);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4170_0000);
    fx.mem.write_u32(g + 0x50, 0x4120_0000);
    halves(fx, g, &[(0x5A, 0x40), (0x5C, 8), (0x5E, 0x10)]);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x33A) {
        let p = D + 0x340 + 16 * k as u32;
        throw(fx, sp, p);
        let r = fx.rand();
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, h.wrapping_neg());
        fx.mem.write_u8(p + 0xC, 0x80);
        let r = fx.rand();
        fx.mem.write_u8(p + 0xD, (r % 8) as u8);
        k += 1;
    }
    // The fountain (+0x7B0): 128 sparks at +0x7E0, width 0.5, length 5.
    let g = D + 0x7B0;
    let tex = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x2000_0003);
    for k in [0xAu32, 9, 8] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 0xB, 0xFF);
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x40A0_0000);
    halves(fx, g, &[(0x10, 0x20), (0x12, 0x20), (0x14, 0x20), (0x16, 0x20), (0x18, 0x20), (0x1A, 0x20), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, D + 0x7E0);
    fx.mem.write_u32(g + 0x28, 0x40A0_0000);
    fx.mem.set16(g + 0x2C, 0x80);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 4.0);
    let tex = fx.mem.read_u32(sp + 0xC0);
    zaps_init(fx, tex);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0xFE4);
    let node = |fx: &mut Fx, at: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + at);
        fx.node_position(h, s, out);
    };
    node(fx, 0xFF0, sp + 0x90);
    node(fx, 0xFF4, sp + 0xA0);
    for out in [sp + 0xC0, D + 0x290] {
        let t = fx.target_handle();
        fx.unit_point(1, t, out);
    }
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x750);
    for c in 0..4u32 {
        let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
        fx.mem.write_u32(D + 0x760 + 4 * c, w);
    }
    // s0 = the OT, s1 the param (-1, 64 bits: 0x20C0B0 saves it at sp -
    // 0xA0, under the sprite group's TEX1 slot), s2 = D.
    let regs = [Some(OT), None, Some(D), None];
    for (x, ra) in [(0u32, 0x01E0_31E4), (0x270, 0x01E0_31EC), (0x740, 0x01E0_31F4)] {
        fx.xform_world_saving(D + x, ra, regs);
        fx.saved(sp - 0xD0, &[(0x30, u64::MAX)]);
    }
    if n(fx) == 0x8C {
        let f = fx.mem.read_u32(D + 0x2E4);
        fx.mem.write_u32(D + 0x2E4, f | 0x8_0000);
    }
    if n(fx) >= 0x4E {
        fx.sprites(D + 0x2E0, D + 0x270);
        if n(fx) >= 0x104 {
            fx.step_down(D + 0x2EB, 0, 8);
        } else {
            fx.step_up(D + 0x2EB, 0x80, 2);
        }
        let grow = f32::from_bits(0x3F83_D70A);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x33A) {
            let p = D + 0x340 + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) >= 0 {
                for c in 0..3u32 {
                    let x = ee::mul(fx.mem.f(p + 4 * c), grow);
                    fx.mem.setf(p + 4 * c, x);
                }
                fx.step_down(p + 0xC, 0, 8);
            }
            if fx.mem.i16(p + 0xE) < 0x10 {
                continue;
            }
            throw(fx, sp, p);
            fx.mem.set16(p + 0xE, 0);
            fx.mem.write_u8(p + 0xC, 0x80);
        }
    }
    if (0x2C..0xA8).contains(&n(fx)) {
        let e = sp + 0xB0;
        for k in 0..8u32 {
            for c in 0..4u32 {
                let w = fx.mem.read_u32(sp + 0xC0 + 4 * c);
                fx.mem.write_u32(e + 4 * c, w);
            }
            let a = (0x2000 * k) as i32;
            let c = fx.tcos(a);
            let x = ee::add(fx.mem.f(e), ee::mul(20.0, c));
            fx.mem.setf(e, x);
            let s = fx.tsin(a);
            let y = ee::add(fx.mem.f(e + 4), ee::mul(20.0, s));
            fx.mem.setf(e + 4, y);
            blib::zap::zap(fx, OT, D + 0x70 + 0x40 * k, D, sp + 0xA0, e);
        }
        blib::trail::sparks(fx, OT, D + 0x7B0, D + 0x740);
        if n(fx) >= 0x105 {
            fx.step_down(D + 0x7BB, 0, 0x10);
        }
        // s0: the OT, then each respawn's angle (rand's saved s0).
        let mut s0 = OT;
        let mut k = 0;
        while k < fx.mem.i16(D + 0x7DC) {
            let p = D + 0x7E0 + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) < 0 {
                continue;
            }
            let s = f32::from_bits(0x3FA6_6666);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 4 * c), s);
                fx.mem.setf(p + 4 * c, x);
            }
            if ee_le(fx.mem.f(p + 4), 50.0) {
                continue;
            }
            // Respawned on a flat ring (as `spawn::scatter` with flag 0x1D).
            let f = ee::mul(10.0, fx.mem.f(D + 0x7D8));
            fx.fix_residue(f, 0x01E0_348C);
            let w = ee::fixsfsi(f);
            let a = fx.rand_from(s0, 0x01E0_3494) & 0xFFFF;
            s0 = a as u32;
            let r = fx.rand_from(s0, 0x01E0_34B0);
            let (_, h) = fx.div(r, w);
            fx.mem.write_u32(p + 4, 0x4080_0000);
            let radius = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
            let c = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(radius, c));
            let s = fx.tsin(a);
            fx.mem.setf(p, ee::mul(radius, s));
            let r = fx.rand_from(s0, 0x01E0_3500);
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0xE, h.wrapping_neg());
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0xFE4, m);
    let left = fx.mem.i32(D + 0xFE0) - 1;
    fx.mem.set32(D + 0xFE0, left);
    if m == 0x104 {
        2
    } else if m == 0x44 {
        1
    } else if (0x4E..0x104).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
