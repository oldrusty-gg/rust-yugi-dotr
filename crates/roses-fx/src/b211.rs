//! BMODULE b211 (card 243), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03C50, data area 0x1ED0000) [F] (helper C, 30 Oct
//! 2026): b138's layout and set-up ([`crate::b138`], `Variant`) with its
//! own timeline: the shower at the attacker's node 0x11 on updates 34..69
//! (fading from 54), the attacker's disc / wobbling quad burst on updates
//! 34..137 (the disc turned and fading from 70), the ribbon from update 70
//! (1 / 24 of the way per update); at the target from update 94 three
//! rings widening to 1 and a disc / quad burst growing to 2 until update
//! 172 (then full size); the frame quad from update 70; the stage light
//! down until update 195. Returns 1 on update 84, 6 on 94..194, 2 on 195,
//! 8 after 212.
use crate::b138::{fountain_spawn, init, step_head, Variant};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx};

const D: u32 = crate::bmodule::DATA_AREA;

pub const B211: Variant = Variant {
    template: 0x01E0_4C00,
    nodes: [0x11, 0x12, 0x39, 0xB],
    length: 0xD4,
    approach: 0x18,
    bgr: [0xB0, 0x70, 0xD0],
    disc2_flags: 0,
    life: 0x48,
    shower_alpha: 0,
    quad_flags: 1,
    quad_size: 2.0,
    ring_flags: 0,
    ring_radii: (0x4178_0000, 0x4184_0000),
};

/// The entry 0x1E03C50.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param, &B211) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    step_head(fx, sp, [0x01E0_4428, 0x01E0_4430, 0x01E0_4438, 0x01E0_4440, 0x01E0_4448, 0x01E0_4450, 0x01E0_4460]);
    let step20 = f32::from_bits(0x3D4C_CCCD);
    if fx.mem.i32(D + 0x3BC4) >= 0x46 {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if fx.mem.i32(D + 0x3BC4) < 0xC3 {
        fx.step_down(D + 0x3BCC, 0x20, 4);
    } else {
        fx.step_up(D + 0x3BCC, 0x80, 4);
    }
    let light = fx.mem.u8(D + 0x3BCC);
    fx.set_stage_light(light);
    if fx.mem.i32(D + 0x3BC4) >= 0x5E {
        for s in 0..3u32 {
            let (x, g) = (D + 0x2E70 + 0x70 * s, D + 0x2FC0 + 0x400 * s);
            fx.ring_draw(g, x);
            fx.step_down(g + 0xB, 0, 8);
            if fx.approach(x, 1.0, f32::from_bits(0x3DCC_CCCD)) == 0 {
                for k in [8u32, 4, 0] {
                    fx.mem.write_u32(x + k, 0x3DCC_CCCD);
                }
                for k in [0x10u32, 0x14, 0x18] {
                    let r = fx.rand();
                    fx.mem.set32(x + k, r & 0xFFFF);
                }
                if fx.mem.i32(D + 0x3BC4) < 0xC3 {
                    fx.mem.write_u8(g + 0xB, 0x80);
                }
            }
        }
        let n = fx.mem.i32(D + 0x3BC4);
        if (0x5E..0xAD).contains(&n) {
            fx.approach(D + 0x390, 2.0, step20);
        }
        if fx.mem.i32(D + 0x3BC4) == 0xAD {
            for k in [8u32, 4, 0] {
                fx.mem.write_u32(D + 0x390 + k, 0x3F80_0000);
            }
        }
        fx.disc(D + 0x860, D + 0x390);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_462C)]);
        fx.quads_draw(D + 0x400, D + 0x390);
        if fx.mem.i32(D + 0x3BC4) >= 0xC3 {
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
    let n = fx.mem.i32(D + 0x3BC4);
    if (0x22..0x8A).contains(&n) {
        if n == 0x46 {
            let f = fx.mem.read_u32(D + 0x380) | 0x1000_0000;
            fx.mem.write_u32(D + 0x380, f);
        }
        fx.disc(D + 0x380, D + 0x310);
        fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, 0x01E0_4708)]);
        if fx.mem.i32(D + 0x3BC4) >= 0x46 {
            fx.approach(D + 0x310, 2.0, step20);
            fx.step_down(D + 0x387, 0x80, 8);
        } else {
            fx.step_up(D + 0x387, 0xFF, 8);
        }
        fx.quads_draw(D + 0x2A10, D + 0x29A0);
        fx.approach(D + 0x29A0, 1.0, step20);
        for k in 0..64u32 {
            let a = D + 0x2A74 + 16 * k;
            let x = fx.mem.i32(a);
            fx.mem.set32(a, if k % 2 == 0 { x.wrapping_add(0x40) } else { x.wrapping_sub(0x40) });
        }
    }
    let n = fx.mem.i32(D + 0x3BC4);
    if n < 0x46 {
        if n == 0x22 {
            fx.mem.write_u8(D + 0x8EB, 0x80);
        }
        fx.sprites(D + 0x8E0, D + 0x870);
        if fx.mem.i32(D + 0x3BC4) >= 0x36 {
            fx.step_down(D + 0x8EB, 0, 8);
        }
        let mut k = 0;
        while k < fx.mem.i16(D + 0x93A) {
            let p = D + 16 * k as u32;
            if fx.mem.i16(p + 0x94E) >= 0 {
                let y = ee::sub(fx.mem.f(p + 0x944), 1.0);
                fx.mem.setf(p + 0x944, y);
                fx.step_up(p + 0x94C, 0x80, 8);
                if !ee_le(fx.mem.f(p + 0x944), 0.0) {
                    let f = f32::from_bits(0x3F4C_CCCD);
                    let z = ee::mul(fx.mem.f(p + 0x948), f);
                    fx.mem.setf(p + 0x948, z);
                    let x = ee::mul(fx.mem.f(p + 0x940), f);
                    fx.mem.setf(p + 0x940, x);
                }
                if ee_lt(fx.mem.f(p + 0x944), 0.0) {
                    let a = fx.rand() & 0xFFFF;
                    let c = fx.tcos(a);
                    let z = ee::mul(fx.mem.f(D + 0x92C), c);
                    fx.mem.setf(p + 0x948, z);
                    let s = fx.tsin(a);
                    let x = ee::mul(fx.mem.f(D + 0x92C), s);
                    fx.mem.setf(p + 0x940, x);
                    let r = fx.rand();
                    let (_, h) = fx.div(r, 0xA);
                    fx.mem.setf(p + 0x944, ee::add(10.0, ee::from_i32(h)));
                    fx.mem.write_u8(p + 0x94C, 0);
                }
            }
            k += 1;
        }
    }
    if fx.mem.i32(D + 0x3BC4) >= 0x41 {
        fx.sprites(D + 0x1940, D + 0x870);
        let grow = f32::from_bits(0x3F99_999A);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x199A) {
            let p = D + 16 * k as u32;
            if fx.mem.i16(p + 0x19AE) >= 0 {
                let y = ee::add(fx.mem.f(p + 0x19A4), 2.0);
                fx.mem.setf(p + 0x19A4, y);
                let x = ee::mul(fx.mem.f(p + 0x19A0), grow);
                fx.mem.setf(p + 0x19A0, x);
                let z = ee::mul(fx.mem.f(p + 0x19A8), grow);
                fx.mem.setf(p + 0x19A8, z);
                fx.step_down(p + 0x19AC, 0, 8);
                if !ee_le(fx.mem.f(p + 0x19A4), 20.0) {
                    fountain_spawn(fx, p, 0x01E0_4A14);
                }
            }
            k += 1;
        }
    }
    if fx.mem.i32(D + 0x3BC4) >= 0x47 {
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
    }
    let n = fx.mem.i32(D + 0x3BC4) + 1;
    fx.mem.set32(D + 0x3BC4, n);
    let left = fx.mem.i32(D + 0x3BC0) - 1;
    fx.mem.set32(D + 0x3BC0, left);
    if n == 0xC3 {
        2
    } else if n == 0x54 {
        1
    } else if (0x5E..0xC3).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
