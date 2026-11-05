//! BMODULE b427 (card 43), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E049E0, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): b043's family. A rising sprite fountain (+0x2740) at the
//! attacker's animated position (0x1F65D0 kind 1) until update 120; a
//! spark burst (+0x310, `blib::trail::sparks`) from update 100 shrinking
//! back and thrown out again as each spark dies; a disc (+0x26C0) on
//! updates 98..267 (additive from update 240); from update 240 a ribbon
//! trail (+0x70) from node 0x26 toward the target (1 / 16 of the way per
//! update, then 1 / 15 ...) and the frame quad; at the target from update
//! 256 three widening rings (+0x1A50 + 0x400 k), a disc (+0x1420) and a
//! quad burst (+0x14A0) growing to 1; the stage light down to 0x20 until
//! update 345. Returns 1 on update 246, 6 on 256..344, 2 on 345, 8 after
//! 362.
//!
//! Library: `colours4` (0x1E00080), the frame quad (0x1E00120,
//! [`crate::framegs`]), `spawn::scatter` (0x1E00240), `trail::sparks`
//! (0x1E00400) [`crate::blib`], `ring_setup` / `ring_draw` (0x1E01160 /
//! 0x1E01420), `disc` (0x1E01910), `sprites` (0x1E01E70), `quads_setup` /
//! `quads_draw` (0x1E03760 / 0x1E038C0) [`crate::fxlib`], the ribbon
//! (0x1E03ED0 / 0x1E04020, [`crate::ribbon`]).
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad corner template (rodata, 0x1E03760's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_5700;
const TENTH: u32 = 0x3DCC_CCCD;

/// The entry 0x1E049E0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x90);
    ret
}

fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

fn bytes(fx: &mut Fx, at: u32, xs: &[(u32, u8)]) {
    for &(k, x) in xs {
        fx.mem.write_u8(at + k, x);
    }
}

fn disc_record(fx: &mut Fx, at: u32, alpha: u8, radius: u32) {
    fx.mem.write_u32(at, 0x8_0000);
    bytes(fx, at, &[(6, 0xA0), (5, 0xD0), (4, 0x70), (7, alpha)]);
    fx.mem.write_u32(at + 8, radius);
    fx.mem.set16(at + 0xC, 0x24);
    fx.mem.set16(at + 0xE, 0);
}

/// A fountain sprite (`p` = D + 16 k, fields at +0x27A0) thrown out at a
/// random angle, radius (rand % 60 - 30) / 10 [F 0x1E04DEC / 0x1E05208].
fn fountain_throw(fx: &mut Fx, p: u32) {
    let a = fx.rand() & 0xFFFF;
    let r = fx.rand();
    let (_, h) = fx.div(r, 0x3C);
    let radius = ee::mul(f32::from_bits(TENTH), ee::from_i32(h - 0x1E));
    let c = fx.tcos(a);
    fx.mem.setf(p + 0x27A8, ee::mul(radius, c));
    let s = fx.tsin(a);
    fx.mem.setf(p + 0x27A0, ee::mul(radius, s));
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2FD0);
    for x in [0u32, 0x1340, 0x2A0, 0x1900, 0x1970, 0x19E0, 0x2650, 0x13B0, 0x26D0, 0x1430] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2FC0u32, 0x26, 1), (0x2FC4, 0x17, 0), (0x2FC8, 0x21, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2FB4, 0x16A);
    fx.mem.set32(D + 0x2FB8, 1);
    fx.mem.set32(D + 0x2FBC, param);
    fx.mem.write_u8(D + 0x2FCC, 0x80);
    fx.mem.set32(D + 0x2FB0, 0x10);
    disc_record(fx, D + 0x1420, 0xFF, 0x41F0_0000);
    disc_record(fx, D + 0x26C0, 0, 0x4120_0000);
    fx.texture5(sp + 0x80, 0);
    let tex = fx.mem.read_u32(sp + 0x80);
    // The ribbon (+0x70): cell (0, 0, 16, 32), width 20.
    let g = D + 0x70;
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 8, 0x8003);
    bytes(fx, g, &[(6, 0x80), (5, 0x80), (4, 0x80), (7, 0x80)]);
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x10), (0x16, 0x20), (0x18, 0xC8), (0x1A, 0x20), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    halves(fx, g, &[(0x22, 8), (0x24, 0), (0x26, 0x48), (0x28, 0x20), (0x2A, 0), (0x2C, 1)]);
    // The sparks (+0x310): 256 at +0x340, width 0.05, length 5, thrown
    // at 100 with lives up to 16.
    let g = D + 0x310;
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 3);
    bytes(fx, g, &[(0xA, 0x80), (9, 0x80), (8, 0x80), (0xB, 0)]);
    fx.mem.write_u32(g + 0x20, 0x3D4C_CCCD);
    fx.mem.write_u32(g + 0x24, 0x40A0_0000);
    halves(fx, g, &[(0x10, 0), (0x12, 0x20), (0x14, 0x20), (0x16, 0x20), (0x18, 0), (0x1A, 0x20), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, D + 0x340);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0x10, 100.0);
    fx.texture4(sp + 0x80, 0xF);
    // The rings (+0x1A50 + 0x400 k): radii 11.5 / 12.5.
    for k in 0..3u32 {
        let g = D + 0x1A50 + 0x400 * k;
        fx.mem.write_u32(g + 4, 0x400);
        halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
        bytes(fx, g, &[(0xA, 0xA0), (9, 0xD0), (8, 0x70), (0xB, 0x80)]);
        let tex = fx.mem.read_u32(sp + 0x80);
        fx.ring_setup(g, tex, 11.5, 12.5, 0.0, 0.0);
    }
    // The quad burst (+0x14A0), size 3.
    let g = D + 0x14A0;
    fx.mem.write_u32(g + 4, 0x8_0001);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    bytes(fx, g, &[(0xA, 0xA0), (9, 0xD0), (8, 0x70), (0xB, 0x20)]);
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.quads_setup(g, tex, 3.0, QUAD_TEMPLATE);
    fx.texture5(sp + 0x80, 1);
    // The fountain (+0x2740): 128 sprites at +0x27A0, size 1, radius 2.
    let g = D + 0x2740;
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0003);
    bytes(fx, g, &[(0xA, 0x80), (9, 0x80), (8, 0x80), (0xB, 0x80)]);
    fx.mem.write_u32(g + 0xC, 0x3F80_0000);
    fx.mem.write_u32(g + 0x20, 0);
    fx.mem.write_u32(g + 0x24, 0x400);
    fx.mem.write_u32(g + 0x28, 0);
    halves(fx, g, &[(0x30, 0), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x27A0);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4000_0000);
    halves(fx, g, &[(0x5A, 0x80), (0x5C, 8), (0x5E, 0)]);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x279A) {
        let p = D + 16 * k as u32;
        fx.mem.set16(p + 0x27AE, 1);
        fx.mem.write_u8(p + 0x27AC, 0);
        let r = fx.rand();
        fx.mem.set8(p + 0x27AD, r % 8);
        fountain_throw(fx, p);
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x190);
        k += 1;
        fx.mem.setf(p + 0x27A4, ee::mul(f32::from_bits(TENTH), ee::from_i32(h - 0xC8)));
    }
    for (x, s) in [(0x1900u32, 0u32), (0x1970, 0x3F00_0000), (0x19E0, 0x3F80_0000), (0x1430, TENTH)] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, s);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2FB8);
    let node = |fx: &mut Fx, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x2FC0);
        fx.node_position(h, s, out);
    };
    let target = |fx: &mut Fx, out: u32| {
        let t = fx.target_handle();
        fx.unit_position(t, out);
    };
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x26F0);
    node(fx, D + 0x2670);
    node(fx, D + 0x2C0);
    target(fx, D + 0x2FA0);
    target(fx, D + 0x13D0);
    target(fx, D + 0x1450);
    for k in 0..3u32 {
        target(fx, D + 0x1920 + 0x70 * k);
    }
    if n(fx) < 0xF1 {
        node(fx, D + 0x1360);
    }
    if n(fx) >= 0xF0 {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x1360);
        let c = fx.mem.i32(D + 0x2FB0);
        fx.approach_by(D + 0x1360, D + 0x2FA0, c);
        let c = fx.mem.i32(D + 0x2FB0);
        if c != 0 {
            fx.mem.set32(D + 0x2FB0, c - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 = D + 0x150, s3 = 3 (then the loop's).
    let regs = [Some(D), Some(OT), Some(D + 0x150), Some(3)];
    fx.xform_world_saving(D, 0x01E0_5038, regs);
    fx.xform_billboard_saving(D + 0x1340, 0x01E0_5040, regs);
    fx.xform_world_saving(D + 0x2A0, 0x01E0_5048, regs);
    for k in 0..3u32 {
        let regs = [Some(D), Some(OT), Some(D + 0x70 * k), Some(k)];
        fx.xform_world_saving(D + 0x1900 + 0x70 * k, 0x01E0_5058, regs);
    }
    fx.xform_billboard_saving(D + 0x2650, 0x01E0_5074, regs);
    fx.xform_billboard_saving(D + 0x13B0, 0x01E0_507C, regs);
    fx.xform_world_saving(D + 0x26D0, 0x01E0_5084, regs);
    fx.xform_world_saving(D + 0x1430, 0x01E0_508C, regs);
    if n(fx) >= 0xF0 {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) >= 0x159 {
        fx.step_up(D + 0x2FCC, 0x80, 8);
    } else {
        fx.step_down(D + 0x2FCC, 0x20, 8);
    }
    let light = fx.mem.u8(D + 0x2FCC);
    fx.set_stage_light(light);
    if n(fx) < 0x78 {
        fx.sprites(D + 0x2740, D + 0x26D0);
        if n(fx) >= 0x64 {
            fx.step_down(D + 0x274B, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x279A) {
            let p = D + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0x27AE) < 0 {
                continue;
            }
            let y = ee::add(fx.mem.f(p + 0x27A4), 1.0);
            fx.mem.setf(p + 0x27A4, y);
            if fx.mem.u8(p + 0x27AC) != 0 {
                if ee_le(fx.mem.f(p + 0x27A4), 0.0) {
                    fx.step_up(p + 0x27AC, 0x80, 8);
                } else {
                    fx.step_down(p + 0x27AC, 0, 8);
                }
            }
            if ee_lt(fx.mem.f(p + 0x27A4), 20.0) {
                let s = f32::from_bits(0x3F86_6666);
                for c in [8u32, 0] {
                    let x = ee::mul(fx.mem.f(p + 0x27A0 + c), s);
                    fx.mem.setf(p + 0x27A0 + c, x);
                }
            }
            if ee_lt(fx.mem.f(p + 0x27A4), 20.0) {
                continue;
            }
            fountain_throw(fx, p);
            let r = fx.rand();
            let (_, h) = fx.div(r, 0x3C);
            let y = ee::add(-20.0, ee::mul(f32::from_bits(TENTH), ee::from_i32(h - 0x1E)));
            fx.mem.setf(p + 0x27A4, y);
            fx.mem.write_u8(p + 0x27AC, 1);
        }
    }
    let m = n(fx);
    if (0x62..0x10C).contains(&m) {
        if m == 0xF0 {
            let f = fx.mem.read_u32(D + 0x26C0);
            fx.mem.write_u32(D + 0x26C0, f | 0x1000_0000);
        }
        fx.disc(D + 0x26C0, D + 0x2650);
        fx.step_up(D + 0x26C7, 0x80, 8);
    }
    if n(fx) >= 0x100 {
        for k in 0..3u32 {
            let (g, x) = (D + 0x1A50 + 0x400 * k, D + 0x1900 + 0x70 * k);
            fx.ring_draw(g, x);
            fx.step_down(g + 0xB, 0, 8);
            if fx.approach(x, 2.0, f32::from_bits(TENTH)) != 0 {
                continue;
            }
            for c in [8u32, 4, 0] {
                fx.mem.write_u32(x + c, TENTH);
            }
            for c in 0..3u32 {
                let r = fx.rand();
                fx.mem.set32(x + 0x10 + 4 * c, r & 0xFFFF);
            }
            if n(fx) < 0x159 {
                fx.mem.write_u8(g + 0xB, 0x80);
            }
        }
        fx.disc(D + 0x1420, D + 0x13B0);
        if n(fx) >= 0x159 {
            fx.step_down(D + 0x1427, 0, 0x10);
        }
        fx.approach(D + 0x1430, 1.0, f32::from_bits(0x3D4C_CCCD));
        fx.quads_draw(D + 0x14A0, D + 0x1430);
        if n(fx) >= 0x159 {
            fx.step_down(D + 0x14AB, 0, 2);
        }
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0x1500 + 16 * k + 4 * c;
                let x = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, x);
            }
        }
    }
    if n(fx) >= 0x64 {
        blib::trail::sparks(fx, OT, D + 0x310, D + 0x2A0);
        if n(fx) >= 0x15A {
            fx.step_down(D + 0x31B, 0, 8);
        } else {
            fx.step_up(D + 0x31B, 0x80, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x33C) {
            let p = D + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0x34E) < 0 {
                continue;
            }
            let s = f32::from_bits(0x3F4C_CCCD);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 0x340 + 4 * c), s);
                fx.mem.setf(p + 0x340 + 4 * c, x);
            }
            fx.step_up(p + 0x34C, 0x80, 8);
            if fx.mem.i16(p + 0x34E) < 0x11 {
                continue;
            }
            fx.random_dir(sp + 0x70, 100.0);
            for c in [1u32, 2, 0] {
                let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
                fx.mem.write_u32(p + 0x340 + 4 * c, w);
            }
            let r = fx.rand();
            fx.mem.set16(p + 0x34E, (r % 16).wrapping_neg());
            fx.mem.write_u8(p + 0x34C, 0);
        }
    }
    if n(fx) >= 0xF0 {
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2FB8, m);
    let left = fx.mem.i32(D + 0x2FB4) - 1;
    fx.mem.set32(D + 0x2FB4, left);
    if m == 0x159 {
        2
    } else if m == 0xF6 {
        1
    } else if (0x100..0x159).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
