//! BMODULE b753 (card 53), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02090, data area 0x1ED0000) [F] (helper C, 30 Oct
//! 2026): two sprite fountains (+0x1E0: 1024 sprites under the attacker,
//! +0x42B0: 512 under the target from update 161 at 1.5 size), each turned
//! to look up from the unit 100 / 150 high, rising 4 / 5 / 6 and spreading
//! by 1.2 until 150 high; a pulsing disc at the attacker's node 0x44 and a
//! disc billboard; the frame quad (white, alpha 0x60) every update; the
//! stage light down to 0x20 until update 183. Returns 1 on update 171, 6
//! on 181, 2 on 182, 8 after 200.
//!
//! Library: `sprites` (0x1E00240), `disc` (0x1E01B30) [`crate::fxlib`],
//! the frame quad (0x1E00120, [`crate::framegs`]).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E02090.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x90);
    ret
}

fn disc_record(fx: &mut Fx, at: u32, flags: u32, bgr: [u8; 3], radius: u32) {
    fx.mem.write_u32(at, flags);
    fx.mem.write_u8(at + 6, bgr[2]);
    fx.mem.write_u8(at + 5, bgr[1]);
    fx.mem.write_u8(at + 4, bgr[0]);
    fx.mem.write_u8(at + 7, 0);
    fx.mem.write_u32(at + 8, radius);
    fx.mem.set16(at + 0xC, 0x24);
    fx.mem.set16(at + 0xE, 0);
}

/// A fountain's sprite group at `g`: `count` sprites at `parts`.
fn fountain(fx: &mut Fx, g: u32, tex: u32, parts: u32, count: i32) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0003);
    fx.mem.write_u8(g + 0xA, 0x70);
    fx.mem.write_u8(g + 9, 0x70);
    fx.mem.write_u8(g + 8, 0x80);
    fx.mem.write_u8(g + 0xB, 0);
    fx.mem.write_u32(g + 0xC, 0x3F80_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    for (k, x) in [(0x30u32, 0xB0), (0x32, 0x40), (0x34, 0x20), (0x36, 0x20), (0x38, 0xB0), (0x3A, 0x40), (0x3C, 0x100), (0x3E, 0x100)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u32(g + 0x40, parts);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4040_0000);
    fx.mem.write_u32(g + 0x50, 0x3F00_0000);
    fx.mem.set16(g + 0x5A, count);
    fx.mem.set16(g + 0x5C, 2);
    fx.mem.set16(g + 0x5E, 0x64);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = parts + 16 * k as u32;
        let a = fx.rand() & 0xFFFF;
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x5DC);
        fx.mem.setf(p + 4, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h)));
        spread(fx, g, p, a);
        let r = fx.rand();
        let (_, h) = fx.div(r, 20);
        fx.mem.set16(p + 0xE, -h);
        fx.mem.write_u8(p + 0xC, 0);
        let r = fx.rand();
        fx.mem.write_u8(p + 0xD, (r % 2) as u8);
        k += 1;
    }
}

/// z, x = the group's radius +0x4C x (cos, sin) of `a`.
fn spread(fx: &mut Fx, g: u32, p: u32, a: i32) {
    let c = fx.tcos(a);
    let z = ee::mul(fx.mem.f(g + 0x4C), c);
    fx.mem.setf(p + 8, z);
    let s = fx.tsin(a);
    let x = ee::mul(fx.mem.f(g + 0x4C), s);
    fx.mem.setf(p, x);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x6330);
    for x in [0u32, 0x70, 0x170, 0x4240, 0xF0] {
        fx.xform_init(D + x);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x44, 1);
    fx.mem.set32(D + 0x631C, r);
    fx.mem.set32(D + 0x6310, 0xC8);
    fx.mem.set32(D + 0x6314, 1);
    fx.mem.set32(D + 0x6318, param);
    fx.mem.write_u8(D + 0x6320, 0x80);
    disc_record(fx, D + 0xE0, 0, [0x80, 0x70, 0x70], 0x4140_0000);
    disc_record(fx, D + 0x160, 0x8_0000, [0x70, 0x70, 0x80], 0x40A0_0000);
    fx.texture4(sp + 0x80, 0xF);
    let tex = fx.mem.read_u32(sp + 0x80);
    fountain(fx, D + 0x1E0, tex, D + 0x240, 0x400);
    let tex = fx.mem.read_u32(sp + 0x80);
    fountain(fx, D + 0x42B0, tex, D + 0x4310, 0x200);
    0
}

/// The fountain's sprites rise and spread; at 150 high they start again.
fn rise(fx: &mut Fx, g: u32) {
    let top = f32::from_bits(0x4316_0000);
    let parts = fx.mem.read_u32(g + 0x40);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = parts + 16 * k as u32;
        if fx.mem.i16(p + 0xE) >= 0 {
            if ee_lt(fx.mem.f(p + 4), top) {
                let (_, m) = fx.div(k, 3);
                fx.mem.set16(p + 0xE, 0);
                let d = match m {
                    0 => 4.0,
                    1 => 5.0,
                    _ => 6.0,
                };
                let y = ee::add(fx.mem.f(p + 4), d);
                fx.mem.setf(p + 4, y);
                let f = f32::from_bits(0x3F99_999A);
                let z = ee::mul(fx.mem.f(p + 8), f);
                fx.mem.setf(p + 8, z);
                let x = ee::mul(fx.mem.f(p), f);
                fx.mem.setf(p, x);
            }
            fx.step_up(p + 0xC, 0x80, 0x20);
            if !ee_lt(fx.mem.f(p + 4), top) {
                let a = fx.rand() & 0xFFFF;
                let r = fx.rand();
                let (_, h) = fx.div(r, 20);
                fx.mem.set16(p + 0xE, -h);
                spread(fx, g, p, a);
                let r = fx.rand();
                let (_, h) = fx.div(r, 20);
                fx.mem.setf(p + 4, ee::from_i32(h));
                fx.mem.write_u8(p + 0xC, 0);
                let r = fx.rand();
                fx.mem.write_u8(p + 0xD, (r % 2) as u8);
            }
        }
        k += 1;
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x631C);
    fx.node_position(h, s, D + 0x110);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x90);
    fx.mem.write_u32(D + 0x94, 0x4214_0000);
    fx.mem.write_u32(D + 0x80, 0x4000);
    let late = fx.mem.i32(D + 0x6314) >= 0xA1;
    if late {
        fx.mem.write_u32(D + 0x1EC, 0x3FC0_0000);
    }
    fx.mem.write_u32(sp + 0x60, 0);
    fx.mem.write_u32(sp + 0x68, 0);
    // The attacker's fountain looks from 100 (later 150) above the unit
    // (self, later the target) to the unit.
    let late = fx.mem.i32(D + 0x6314) >= 0xA1;
    fx.mem.write_u32(sp + 0x64, if late { 0x4316_0000 } else { 0x42C8_0000 });
    let u = if late { fx.target_handle() } else { fx.self_handle() };
    fx.unit_position(u, sp + 0x70);
    for k in 0..4u32 {
        let w = fx.mem.read_u32(sp + 0x60 + 4 * k);
        fx.mem.write_u32(D + 0x190 + 4 * k, w);
    }
    fx.look_angles(sp + 0x70, sp + 0x60, D + 0x180);
    let late = fx.mem.i32(D + 0x6314) >= 0xA1;
    let u = if late { fx.target_handle() } else { fx.self_handle() };
    fx.unit_position(u, sp + 0x60);
    let z = ee::mul(fx.mem.f(sp + 0x68), 2.0);
    fx.mem.setf(sp + 0x68, z);
    fx.mem.write_u32(sp + 0x64, 0x42C8_0000);
    let u = if late { fx.target_handle() } else { fx.self_handle() };
    fx.unit_position(u, sp + 0x70);
    for k in 0..4u32 {
        let w = fx.mem.read_u32(sp + 0x60 + 4 * k);
        fx.mem.write_u32(D + 0x4260 + 4 * k, w);
    }
    fx.look_angles(sp + 0x70, sp + 0x60, D + 0x4250);
    let regs = [Some(D), Some(OT), Some(sp + 0x68), Some(sp + 0x70)];
    fx.xform_world_saving(D, 0x01E0_2650, regs);
    fx.xform_world_saving(D + 0x70, 0x01E0_2658, regs);
    fx.xform_world_saving(D + 0x170, 0x01E0_2660, regs);
    fx.xform_world_saving(D + 0x4240, 0x01E0_2668, regs);
    fx.xform_billboard_saving(D + 0xF0, 0x01E0_2670, regs);
    fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    if fx.mem.i32(D + 0x6314) >= 0xB7 {
        fx.step_up(D + 0x6320, 0x80, 6);
    } else {
        fx.step_down(D + 0x6320, 0x20, 6);
    }
    let light = fx.mem.u8(D + 0x6320);
    fx.set_stage_light(light);
    fx.disc(D + 0xE0, D + 0x70);
    fx.step_up(D + 0xE7, 0x80, 8);
    let n = fx.mem.i32(D + 0x6314);
    if n >= 0x35 {
        if n == 0xB6 {
            fx.mem.write_u8(D + 0x167, 0xFF);
        }
        fx.disc(D + 0x160, D + 0xF0);
        if fx.mem.i32(D + 0x6314) < 0xB6 {
            fx.step_up(D + 0x167, 0x80, 8);
        } else {
            fx.step_down(D + 0x167, 0, 0x10);
        }
    }
    fx.sprites(D + 0x1E0, D + 0x170);
    if fx.mem.i32(D + 0x6314) >= 0xB6 {
        fx.step_down(D + 0x1EB, 0, 8);
    } else {
        fx.step_up(D + 0x1EB, 0x80, 8);
    }
    rise(fx, D + 0x1E0);
    fx.sprites(D + 0x42B0, D + 0x4240);
    if fx.mem.i32(D + 0x6314) >= 0xB6 {
        fx.step_down(D + 0x42BB, 0, 8);
    } else {
        fx.step_up(D + 0x42BB, 0x80, 8);
    }
    rise(fx, D + 0x42B0);
    let n = fx.mem.i32(D + 0x6314) + 1;
    fx.mem.set32(D + 0x6314, n);
    let left = fx.mem.i32(D + 0x6310) - 1;
    fx.mem.set32(D + 0x6310, left);
    if n == 0xB6 {
        2
    } else if n == 0xAB {
        1
    } else if n == 0xB5 {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
