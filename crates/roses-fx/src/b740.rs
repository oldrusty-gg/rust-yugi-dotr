//! BMODULE b740 (card 427), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01F30, data area 0x1ED0000) [F] (helper C3, 31
//! Oct 2026). A sprite burst (+0x2E0, 64 sprites thrown out 14 from the
//! attacker's node 0x13, grown to 1.5 and moved onto the target from
//! update 87) and on updates 44..167 eight jittered lines (`zap::zap`,
//! +0x70 + 0x40 k, each growing one segment an update, staggered) from the
//! node to a ring of radius 20 round a point coming down from 100 above
//! the target (1 / 30 of the way per update, then 1 / 29 ...). Returns 1
//! on update 78, 6 on 88..259, 2 on 260, 8 after 275.
//!
//! Library: `sprites` (0x1E00120) [`crate::fxlib`]; `zap::zap`
//! (0x1E01A10) [`crate::blib`].
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E01F30.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

/// A burst sprite (`p`) thrown out: direction 0x1043F0 of length 14.
fn throw(fx: &mut Fx, sp: u32, p: u32) {
    fx.random_dir(sp + 0x70, 14.0);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
        fx.mem.write_u32(p + 4 * c, w);
    }
}

/// Eight line groups (`zap::zap`) at +0x70 + 0x40 k: texture `tex`, half
/// width 5, cell (0x5A, 0, 6, 0x20), white, 28 segments, jitter 4, the
/// k-th starting k updates late [F].
pub(crate) fn zaps_init(fx: &mut Fx, tex: u32) {
    for k in 0..8u32 {
        let q = D + 0x70 + 0x40 * k;
        fx.mem.write_u32(q, tex);
        fx.mem.write_u32(q + 4, 0);
        fx.mem.write_u32(q + 0xC, 0x40A0_0000);
        halves(fx, q, &[(0x10, 0x5A), (0x12, 0), (0x14, 6), (0x16, 0x20)]);
        for o in [0xAu32, 9, 8, 0xB] {
            fx.mem.write_u8(q + o, 0x80);
        }
        halves(fx, q, &[(0x28, 0x1C), (0x2C, -(k as i32)), (0x2A, 0x28)]);
        fx.mem.write_u32(q + 0x20, 0);
        fx.mem.write_u32(q + 0x24, 0);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x770);
    fx.xform_init(D);
    fx.xform_init(D + 0x270);
    for (at, node, clear) in [(0x764u32, 0x13, 1), (0x768, 0x14, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x750, 0x114);
    fx.mem.set32(D + 0x754, 1);
    fx.mem.set32(D + 0x758, param);
    fx.mem.set32(D + 0x75C, 0);
    fx.mem.set32(D + 0x760, 0x1E);
    fx.texture5(sp + 0xA0, 0);
    // The burst (+0x2E0): 64 sprites at +0x340, size 20.
    let g = D + 0x2E0;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x4808_0003);
    for k in [0xAu32, 9, 8] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 0xB, 0x40);
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
    let tex = fx.mem.read_u32(sp + 0xA0);
    zaps_init(fx, tex);
    for k in [0x274u32, 0x278, 0x270] {
        fx.mem.write_u32(D + k, 0x3F00_0000);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x754);
    let node = |fx: &mut Fx, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x764);
        fx.node_position(h, s, out);
    };
    node(fx, sp + 0x70);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x80);
    let t = fx.target_handle();
    fx.unit_point(1, t, sp + 0xA0);
    if n(fx) < 0x57 {
        node(fx, D + 0x290);
    } else {
        fx.mem.write_u32(D + 0x278, 0x3F80_0000);
        fx.mem.write_u32(D + 0x270, 0x3F80_0000);
        fx.mem.write_u32(D + 0x274, 0x3FC0_0000);
        fx.mem.write_u8(D + 0x2EB, 0x80);
        let t = fx.target_handle();
        fx.unit_point(1, t, D + 0x290);
    }
    if n(fx) < 0x67 {
        fx.mem.write_u32(D + 0x740, 0);
        fx.mem.write_u32(D + 0x748, 0);
        fx.mem.write_u32(D + 0x744, 0x42C8_0000);
    }
    if n(fx) >= 0x66 {
        let m = fx.mem.i32(D + 0x760);
        fx.approach_by(D + 0x740, sp + 0x80, m);
        let m = fx.mem.i32(D + 0x760);
        if m != 0 {
            fx.mem.set32(D + 0x760, m - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 the param (-1, 64 bits: 0x20C0B0 saves it
    // at sp - 0x90), s3 the caller's.
    let regs = [Some(D), Some(OT), None, None];
    for (x, ra) in [(0u32, 0x01E0_22A0), (0x270, 0x01E0_22A8)] {
        fx.xform_world_saving(D + x, ra, regs);
        fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
    }
    if (0x2C..0xA8).contains(&n(fx)) {
        let e = sp + 0x90;
        for k in 0..8u32 {
            for c in 0..4u32 {
                let w = fx.mem.read_u32(D + 0x740 + 4 * c);
                fx.mem.write_u32(e + 4 * c, w);
            }
            let a = (0x2000 * k) as i32;
            let c = fx.tcos(a);
            let x = ee::add(fx.mem.f(e), ee::mul(20.0, c));
            fx.mem.setf(e, x);
            let s = fx.tsin(a);
            let y = ee::add(fx.mem.f(e + 4), ee::mul(20.0, s));
            fx.mem.setf(e + 4, y);
            blib::zap::zap(fx, OT, D + 0x70 + 0x40 * k, D, sp + 0x70, e);
        }
    }
    fx.sprites(D + 0x2E0, D + 0x270);
    if n(fx) >= 0x104 {
        fx.step_down(D + 0x2EB, 0, 8);
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
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x754, m);
    let left = fx.mem.i32(D + 0x750) - 1;
    fx.mem.set32(D + 0x750, left);
    if m == 0x104 {
        2
    } else if m == 0x4E {
        1
    } else if (0x58..0x104).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
