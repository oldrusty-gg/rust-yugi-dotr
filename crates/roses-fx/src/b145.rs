//! BMODULE b145 (card 113), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03640, data area 0x1ED0000) [F] (helper C3, 31
//! Oct 2026): b851's twin ([`crate::b851`], one program with
//! `b851::Variant`): the attacker's nodes 0x15 / 0x16 / 0x14, the counters
//! at +0x4220, and other sprite groups: a rising fountain (+0x1FC0, 128
//! sprites, frames at random), two drifting clouds (+0x2890 of size 1,
//! the flickering colour, 2 frames; +0x3160 of size 5, 16 frames, faded
//! from update 240) and a burst (+0x39C0, 128 sprites thrown out again
//! from the sparks' origin as each dies, respawned dark until update 240).
//! Returns 1 on update 113, 6 on 123..359, 2 on 360, 8 after 375.
//!
//! Library: as b851 (the same addresses).
use crate::b851::{cloud_init, cloud_step, halves, ring_place, run, B145};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E03640.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B145)
}

/// A sprite group's header (`g`: texture, flags, colour 0x80 (alpha
/// `a`), size, +0x20.. 0 / `mid` / 0, the cells, sprites at +0x60, +0x4C
/// 2, count 0x80, +0x5C / +0x5E).
fn group(fx: &mut Fx, g: u32, tex: u32, flags: u32, a: u8, size: u32, mid: u32, cells: &[(u32, i32)], tail: [i32; 2]) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, flags);
    for k in [0xAu32, 9, 8] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u8(g + 0xB, a);
    fx.mem.write_u32(g + 0xC, size);
    fx.mem.write_u32(g + 0x20, 0);
    fx.mem.write_u32(g + 0x24, mid);
    fx.mem.write_u32(g + 0x28, 0);
    halves(fx, g, cells);
    fx.mem.write_u32(g + 0x40, g + 0x60);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4000_0000);
    halves(fx, g, &[(0x5A, 0x80), (0x5C, tail[0]), (0x5E, tail[1])]);
}

const CELLS_SMALL: [(u32, i32); 8] = [(0x30, 0), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x80)];

/// b145's sprite groups set up (sp + 0xA0 holding texture (4, 0xF)).
pub(crate) fn groups(fx: &mut Fx, sp: u32) {
    let g = D + 0x2890;
    let tex = fx.mem.read_u32(sp + 0xA0);
    group(fx, g, tex, 0x7000_0001, 0x80, 0x3F80_0000, 0x400, &[(0x30, 0), (0x32, 0xE4), (0x34, 0xA), (0x36, 0xA), (0x38, 0), (0x3A, 0xE4), (0x3C, 0x100), (0x3E, 0x100)], [2, 0]);
    cloud_init(fx, g, 2);
    fx.texture5(sp + 0xA0, 0);
    let g = D + 0x1FC0;
    let tex = fx.mem.read_u32(sp + 0xA0);
    group(fx, g, tex, 0x7000_0003, 0, 0x4000_0000, 0, &[(0x30, 0), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)], [0x10, 0x64]);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = g + 0x60 + 16 * k as u32;
        fx.mem.set16(p + 0xE, 0);
        fx.mem.write_u8(p + 0xC, 0);
        let r = fx.rand();
        fx.mem.write_u8(p + 0xD, (r % 16) as u8);
        ring_place(fx, p, 0x28);
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x1F4);
        fx.mem.setf(p + 4, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::neg(ee::from_i32(h))));
        k += 1;
    }
    let g = D + 0x3160;
    let tex = fx.mem.read_u32(sp + 0xA0);
    group(fx, g, tex, 0x7000_0003, 0x80, 0x40A0_0000, 0x400, &CELLS_SMALL, [0x10, 0x64]);
    cloud_init(fx, g, 16);
    fx.texture5(sp + 0xA0, 1);
    let g = D + 0x39C0;
    let tex = fx.mem.read_u32(sp + 0xA0);
    group(fx, g, tex, 0x7000_0003, 0x80, 0x40A0_0000, 0x400, &CELLS_SMALL, [0x10, 0x64]);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = g + 0x60 + 16 * k as u32;
        burst_spawn(fx, sp, p);
        fx.mem.write_u8(p + 0xC, 0);
        k += 1;
    }
}

/// A burst sprite thrown out: direction 0x1043F0 (length 1), countdown
/// -(rand % 12).
fn burst_spawn(fx: &mut Fx, sp: u32, p: u32) {
    fx.random_dir(sp + 0x70, 1.0);
    for c in [1u32, 2, 0] {
        let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
        fx.mem.write_u32(p + 4 * c, w);
    }
    let r = fx.rand();
    let (_, h) = fx.div(r, 0xC);
    fx.mem.set16(p + 0xE, h.wrapping_neg());
}

/// From update 80: the clouds and the burst drawn and stepped (`n`: the
/// update counter).
pub(crate) fn clouds_step(fx: &mut Fx, sp: u32, n: u32) {
    let m = |fx: &mut Fx| fx.mem.i32(n);
    fx.sprites(D + 0x2890, D + 0x2820);
    if m(fx) >= 0x168 {
        fx.step_down(D + 0x289B, 0, 8);
    }
    cloud_step(fx, D + 0x2890, 2);
    fx.sprites(D + 0x3160, D + 0x30F0);
    if m(fx) >= 0xF0 {
        fx.step_down(D + 0x316B, 0, 8);
    }
    cloud_step(fx, D + 0x3160, 16);
    let g = D + 0x39C0;
    fx.sprites(g, D + 0x70);
    if m(fx) >= 0x168 {
        fx.step_down(g + 0xB, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = g + 0x60 + 16 * k as u32;
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
        burst_spawn(fx, sp, p);
        let a = if m(fx) < 0xF0 { 0 } else { 0x80 };
        fx.mem.write_u8(p + 0xC, a);
    }
}
