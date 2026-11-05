//! BMODULE b736 (card 361), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E023F0, data area 0x1ED0000) [F] (helper C15, 31
//! Oct 2026): b227's program ([`crate::b227`]) with the attacker's
//! afterimage: a burst of 32 sparks (+0x11B0, grey 0x80, thrown 10 in
//! random directions, flying off by 1.03 an update, respawned at age 16)
//! at the target's animated position (+0x1160) from the start; a fountain
//! of 128 sprites (+0xE0, (0x30, 0x30, 0x80)) round the attacker's node
//! 0x25 looking from the target's position, rising 8 / 9 / 10 an update
//! and spreading by 1.12 until 180 high; a full-window grey quad
//! (`frame_quad_rgba`, alpha 0x60) on 0x1C..0x3C; from 0x3D the attacker's
//! afterimage (`afterimage::strips`, +0x1480: grey 0x80 fading out by 8
//! from 0xC9, sway 3 at 0x4000 a strip) with the unit tinted (0x1F75F0,
//! +0x1498) by the complement of the afterimage's alpha; every update a
//! frame flash (`flash::frame_flash`, +0x149C: 0x30, 0x30, 0x80, alpha up
//! 8 to 0x80, down 8 from 0xC8). The burst fades in by 4 to 0x35, out to
//! 0x3D, in to 0xC9, then out by 8; the fountain fades in by 8 to 0xC8
//! then out. Returns 1 on update 0x36, 6 on 0x40..0xC7, 2 on 0xC8, 8 after
//! 0xDC.
//!
//! Library (record addresses): `colours4` 0x1E00080, `frame_quad_rgba`
//! 0x1E00120 (b26f4b13c68d), `flash::frame_flash` 0x1E00240
//! (3b4ae97aa05c), `Fx::sprites` 0x1E00490, `afterimage::strips`
//! 0x1E01D80 (6aca4aa26462); the executable's 0x1F75F0
//! [`crate::b101::unit_tint`] (saving frame [`crate::b015`]).
use crate::b015::unit_tint_saving;
use crate::b227::{colour, fountain_spawn, spark};
use crate::blib::{afterimage, flash};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const N: u32 = D + 0x14A4;
const LEFT: u32 = D + 0x14A0;
const BURST: u32 = D + 0x11B0;
const FOUNTAIN: u32 = D + 0xE0;
const IMAGE: u32 = D + 0x1480;
const UNIT_TINT: u32 = D + 0x1498;
const FLASH: u32 = D + 0x149C;

/// The entry 0x1E023F0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x14C0);
    fx.xform_init(D);
    fx.xform_init(D + 0x70);
    fx.xform_init(D + 0x1140);
    for (at, node, clear) in [(0x14ACu32, 0x49, 1), (0x14B0, 0x25, 0), (0x14B4, 3, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(LEFT, 0xDC);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x14A8, param);
    colour(fx, FLASH, [0x30, 0x30, 0x80, 0]);
    // The afterimage (+0x1480): flags 0x41000, grey 0x80, phase 0, step
    // 0x4000, sway 3, the attacker.
    fx.mem.write_u32(IMAGE, 0x0004_1000);
    colour(fx, IMAGE + 4, [0x80, 0x80, 0x80, 0x80]);
    fx.mem.write_u32(IMAGE + 0x10, 0);
    fx.mem.write_u32(IMAGE + 0xC, 0x4000);
    fx.mem.set16(IMAGE + 8, 0);
    fx.mem.write_u32(IMAGE + 0x14, 0x4040_0000);
    let h = fx.self_handle();
    fx.mem.set16(IMAGE + 0xA, h);
    colour(fx, UNIT_TINT, [0x80, 0x80, 0x80, 0x80]);
    fx.texture5(sp + 0x80, 0);
    // The burst (+0x11B0): 32 sparks, size 20.
    let g = BURST;
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x4808_0003);
    colour(fx, g + 8, [0x80, 0x80, 0x80, 0x80]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    for (k, x) in [(0x30u32, 0), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u32(g + 0x40, D + 0x1210);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4170_0000);
    fx.mem.write_u32(g + 0x50, 0x4120_0000);
    fx.mem.set16(g + 0x5A, 0x20);
    fx.mem.set16(g + 0x5C, 8);
    fx.mem.set16(g + 0x5E, 0x10);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x120A) {
        let p = D + 16 * k as u32;
        spark(fx, sp, p);
        let r = fx.rand();
        let (_, h) = fx.div(r, 12);
        fx.mem.set16(p + 0x121E, -h);
        fx.mem.write_u8(p + 0x121C, 0x80);
        let r = fx.rand();
        fx.mem.write_u8(p + 0x121D, (r % 8) as u8);
        k += 1;
    }
    fx.texture5(sp + 0x80, 1);
    // The fountain (+0xE0): 128 sprites, size 5.
    let g = FOUNTAIN;
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0xF800_0003);
    colour(fx, g + 8, [0x30, 0x30, 0x80, 0]);
    fx.mem.write_u32(g + 0xC, 0x40A0_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    for (k, x) in [(0x30u32, 0), (0x32, 0), (0x34, 0x40), (0x36, 0x40), (0x38, 0), (0x3A, 0), (0x3C, 0x100), (0x3E, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u32(g + 0x40, D + 0x140);
    fx.mem.write_u32(g + 0x44, D + 0x940);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4040_0000);
    fx.mem.write_u32(g + 0x50, 0x3F00_0000);
    fx.mem.set16(g + 0x5A, 0x80);
    fx.mem.set16(g + 0x5C, 4);
    fx.mem.set16(g + 0x5E, 0);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x13A) {
        fountain_spawn(fx, D + 16 * k as u32, false);
        k += 1;
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x60);
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x14B0);
    fx.node_position(h, s, sp + 0x70);
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x14B0);
    fx.node_position(h, s, D + 0x90);
    fx.look_angles(sp + 0x60, sp + 0x70, D + 0x80);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x1160);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x1430);
    // s0 = D, s1 = the OT, s2 = sp + 0x70; s3 the caller's.
    let regs = [Some(D), Some(OT), Some(sp + 0x70), None];
    fx.xform_world_saving(D, 0x01E0_2838, regs);
    fx.xform_world_saving(D + 0x70, 0x01E0_2840, regs);
    fx.xform_world_saving(D + 0x1140, 0x01E0_2848, regs);
    let n = fx.mem.i32(N);
    if (0x1C..0x3D).contains(&n) {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n >= 0x3D {
        let a = fx.mem.read_u8(IMAGE + 7);
        fx.mem.write_u8(UNIT_TINT + 3, 0x80u8.wrapping_sub(a));
        afterimage::strips(fx, IMAGE, D + 0x1430);
        let h = fx.mem.i16(IMAGE + 0xA);
        unit_tint_saving(fx, h, UNIT_TINT, [u64::from(D), u64::from(OT)], 0x01E0_28A4);
        if n >= 0xC9 {
            fx.step_down(IMAGE + 7, 0, 8);
        }
    }
    // The flash's alpha; its first TEXFLUSH reports the step's (floor /
    // cap, step, the byte before) [run b648].
    let before = u32::from(fx.mem.read_u8(FLASH + 3));
    let a123 = if n >= 0xC8 {
        fx.step_down(FLASH + 3, 0, 8);
        [0, 8, before]
    } else {
        fx.step_up(FLASH + 3, 0x80, 8);
        [0x80, 8, before]
    };
    flash::frame_flash(fx, FLASH, a123);
    // The flash's prologue (0xC0: s0 = D, s1 = the OT, ra +0x20): its ra is
    // the burst's missing-texture TEX1 [run].
    fx.saved(sp - 0xC0, &[(0, u64::from(D)), (0x10, u64::from(OT)), (0x20, 0x01E0_2908)]);
    fx.sprites(BURST, D + 0x1140);
    if n < 0x35 {
        fx.step_up(BURST + 0xB, 0x80, 4);
    } else if n < 0x3D {
        fx.step_down(BURST + 0xB, 0, 4);
    } else if n < 0xC9 {
        fx.step_up(BURST + 0xB, 0x80, 4);
    } else {
        fx.step_down(BURST + 0xB, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x120A) {
        let p = D + 16 * k as u32;
        if fx.mem.i16(p + 0x121E) >= 0 {
            let f = f32::from_bits(0x3F83_D70A);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 0x1210 + 4 * c), f);
                fx.mem.setf(p + 0x1210 + 4 * c, x);
            }
            fx.step_down(p + 0x121C, 0, 8);
        }
        if fx.mem.i16(p + 0x121E) >= 0x10 {
            spark(fx, sp, p);
            fx.mem.set16(p + 0x121E, 0);
            fx.mem.write_u8(p + 0x121C, 0x80);
        }
        k += 1;
    }
    fx.sprites(FOUNTAIN, D + 0x70);
    if n >= 0xC8 {
        fx.step_down(FOUNTAIN + 0xB, 0, 8);
    } else {
        fx.step_up(FOUNTAIN + 0xB, 0x80, 8);
    }
    let top = f32::from_bits(0x4334_0000);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x13A) {
        let p = D + 16 * k as u32;
        if fx.mem.i16(p + 0x14E) >= 0 {
            if ee_lt(fx.mem.f(p + 0x144), top) {
                let (_, m) = fx.div(k, 3);
                fx.mem.set16(p + 0x14E, 0);
                let rise = match m {
                    0 => 8.0,
                    1 => 9.0,
                    _ => 10.0,
                };
                let y = ee::add(fx.mem.f(p + 0x144), rise);
                fx.mem.setf(p + 0x144, y);
                let f = f32::from_bits(0x3F8F_5C29);
                let z = ee::mul(fx.mem.f(p + 0x148), f);
                fx.mem.setf(p + 0x148, z);
                let x = ee::mul(fx.mem.f(p + 0x140), f);
                fx.mem.setf(p + 0x140, x);
            }
            if n >= 0x35 {
                fx.step_up(p + 0x14C, 0x80, 0x20);
            }
            if !ee_lt(fx.mem.f(p + 0x144), top) && fx.mem.i16(p + 0x14E) >= 5 {
                fountain_spawn(fx, p, true);
            }
        }
        k += 1;
    }
    let n = n + 1;
    fx.mem.set32(N, n);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if n == 0xC8 {
        2
    } else if n == 0x36 {
        1
    } else if (0x40..0xC8).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
