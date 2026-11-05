//! BMODULE b197 (card 375), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01A10, data area 0x1ED0000) [F] (helper C, 30 Oct
//! 2026): two sprite streams fired from the attacker's node 8 toward the
//! target, each a transform turned by 0x103DB0 to look from the node to the
//! camera's eye; the small one (48 sprites, +0xE0) from update 62, the wide
//! one (18 sprites, +0x7F0) on updates 37..80; the attacker's model op
//! 0x1F83B0 (mode 3, colour 0x80 x 7) every update up to 80. Returns 1 on
//! update 90, 6 on 100..153, 2 on 154, 8 after 170.
//!
//! Library routines: `sprites` (b197 0x1E00120, [`crate::fxlib`]); the
//! executable's 0x103DB0 / 0x103290 / 0x1FACE0 ([`crate::libm`]).
use crate::bus::Bus;
use crate::exe::Fx;

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E01A10.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x70);
    ret
}

/// A sprite group's header at `g` (texture, flags, colour, size 2, cell
/// (0, 0, 64, 64) twice, particles at `particles`).
fn group(fx: &mut Fx, g: u32, tex: u32, flags: u32, particles: u32) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, flags);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0xC, 0x4000_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    for (k, v) in [(0x30u32, 0), (0x32, 0), (0x34, 0x40), (0x36, 0x40), (0x38, 0), (0x3A, 0), (0x3C, 0x40), (0x3E, 0x40)] {
        fx.mem.set16(g + k, v);
    }
    fx.mem.write_u32(g + 0x40, particles);
}

/// Every particle of `g` (count +0x5A) at the origin, alpha 0x30, frame
/// 0, age -k (spawned one per update).
fn particles(fx: &mut Fx, g: u32) {
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = g + 0x60 + 16 * k as u32;
        fx.mem.set16(p + 0xE, -k);
        fx.mem.write_u8(p + 0xC, 0x30);
        fx.mem.write_u8(p + 0xD, 0);
        fx.mem.write_u32(p + 8, 0);
        fx.mem.write_u32(p + 4, 0);
        fx.mem.write_u32(p, 0);
        k += 1;
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x9B0);
    fx.xform_init(D);
    fx.xform_init(D + 0x70);
    fx.xform_init(D + 0x780);
    let h = fx.self_handle();
    let r = fx.request_node(h, 8, 1);
    fx.mem.set32(D + 0x99C, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 1, 0);
    fx.mem.set32(D + 0x9A0, r);
    fx.mem.set32(D + 0x990, 0xAA);
    fx.mem.set32(D + 0x994, 1);
    fx.mem.set32(D + 0x998, param);
    fx.texture5(sp + 0x60, 0);
    // The small stream (+0xE0): 48 sprites, size 15, spread 3.
    let tex = fx.mem.read_u32(sp + 0x60);
    group(fx, D + 0xE0, tex, 0xF800_0003, D + 0x140);
    fx.mem.write_u32(D + 0x124, D + 0x460);
    fx.mem.write_u32(D + 0x128, 0);
    fx.mem.write_u32(D + 0x130, 0x4040_0000);
    fx.mem.write_u32(D + 0x12C, 0x4170_0000);
    fx.mem.set16(D + 0x13A, 0x30);
    fx.mem.set16(D + 0x13C, 1);
    fx.mem.set16(D + 0x13E, 0);
    particles(fx, D + 0xE0);
    // The wide stream (+0x7F0): 18 sprites, size 15, spread 30.
    let tex = fx.mem.read_u32(sp + 0x60);
    group(fx, D + 0x7F0, tex, 0x7800_0003, D + 0x850);
    fx.mem.write_u32(D + 0x834, 0);
    fx.mem.write_u32(D + 0x838, 0);
    fx.mem.write_u32(D + 0x840, 0x41F0_0000);
    fx.mem.write_u32(D + 0x83C, 0x4170_0000);
    fx.mem.set16(D + 0x84A, 0x12);
    fx.mem.set16(D + 0x84C, 1);
    fx.mem.set16(D + 0x84E, 0);
    particles(fx, D + 0x7F0);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let h = fx.self_handle();
    let slot = fx.mem.i32(D + 0x99C);
    fx.node_position(h, slot, sp + 0x40);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x50);
    let h = fx.self_handle();
    let slot = fx.mem.i32(D + 0x99C);
    fx.node_position(h, slot, D + 0x90);
    fx.look_angles(sp + 0x50, sp + 0x40, D + 0x80);
    let eye = fx.camera_eye(0);
    for k in 0..3u32 {
        let v = fx.mem.read_u32(eye.wrapping_add(4 * k));
        fx.mem.write_u32(sp + 0x50 + 4 * k, v);
    }
    let h = fx.self_handle();
    let slot = fx.mem.i32(D + 0x99C);
    fx.node_position(h, slot, D + 0x7A0);
    fx.look_angles(sp + 0x50, sp + 0x40, D + 0x790);
    // s0 = D, s1 = the OT, s2 = sp + 0x50 (left in the 0x20C0B0 frame
    // the sprite descriptors overlay).
    let s = [Some(D), Some(crate::exe::OT), Some(sp + 0x50), None];
    fx.xform_world_saving(D, 0x01E0_1D0C, s);
    fx.xform_world_saving(D + 0x70, 0x01E0_1D14, s);
    fx.xform_world_saving(D + 0x780, 0x01E0_1D1C, s);
    if fx.mem.i32(D + 0x994) < 0x51 {
        let h = fx.self_handle();
        for k in [0x6Cu32, 0x6D, 0x6E, 0x6B, 0x68, 0x69, 0x6A] {
            fx.mem.write_u8(sp + k, 0x80);
        }
        fx.mem.write_u8(sp + 0x6F, 0);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h as u32, 3, 0, sp + 0x68]);
    }
    if fx.mem.i32(D + 0x994) >= 0x3E {
        fx.sprites(D + 0xE0, D + 0x70);
        if fx.mem.i32(D + 0x994) >= 0x9A {
            fx.step_down(D + 0xEB, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x13A) {
            let p = D + 16 * k as u32;
            if fx.mem.i16(p + 0x14E) >= 0 {
                fx.mem.write_u8(p + 0x14C, if k % 2 == 0 { 0x80 } else { 0x40 });
                let y = crate::ee::add(fx.mem.f(p + 0x144), 4.0);
                fx.mem.setf(p + 0x144, y);
                let limit = f32::from_bits(0x4316_0000);
                if crate::exe::ee_lt(y, limit) {
                    fx.mem.set16(p + 0x14E, 0);
                }
                if !crate::exe::ee_lt(fx.mem.f(p + 0x144), limit) {
                    if fx.mem.i16(p + 0x14E) < 9 {
                        fx.mem.write_u32(p + 0x144, 0x4316_0000);
                    } else {
                        fx.mem.write_u32(p + 0x144, 0);
                        fx.mem.write_u32(p + 0x140, 0);
                        fx.mem.write_u32(p + 0x148, 0);
                        fx.mem.set16(p + 0x14E, 0);
                    }
                }
            }
            k += 1;
        }
    }
    let n = fx.mem.i32(D + 0x994);
    if (0x25..0x51).contains(&n) {
        fx.sprites(D + 0x7F0, D + 0x780);
        if fx.mem.i32(D + 0x994) >= 0x32 {
            fx.step_down(D + 0x7FB, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x84A) {
            let p = D + 16 * k as u32;
            let age = fx.mem.i16(p + 0x85E);
            if age >= 0 {
                if age >= 9 {
                    fx.step_down(p + 0x85C, 0, 8);
                } else {
                    fx.mem.write_u8(p + 0x85C, if k % 2 == 0 { 0x80 } else { 0x40 });
                }
                let y = crate::ee::add(fx.mem.f(p + 0x854), 1.0);
                fx.mem.setf(p + 0x854, y);
            }
            k += 1;
        }
    }
    let n = fx.mem.i32(D + 0x994) + 1;
    fx.mem.set32(D + 0x994, n);
    let left = fx.mem.i32(D + 0x990) - 1;
    fx.mem.set32(D + 0x990, left);
    if n == 0x9A {
        2
    } else if n == 0x5A {
        1
    } else if (0x64..0x9A).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
