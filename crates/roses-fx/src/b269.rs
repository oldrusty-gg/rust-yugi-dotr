//! BMODULE b269 (card 569), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02E90, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): b447's family. At the attacker's node 0x38 a sprite burst
//! (+0x2230, 128 sprites thrown out at 8, shrinking to 0.8 a step and
//! fading in, rethrown after 8 steps; fading out after update 53), a disc
//! (+0x21B0, radius 6 from update 70) and b489's white spark column
//! (+0xE0, rising 4 a step); from update 70 a second spark burst
//! (+0x1110) thrown out again as each spark dies; the stage light down
//! to 0x20; on updates 92..235 the target's added colour (0x1F7790)
//! flickering at random. Returns 1 on update 81, 6 on 91..235, 2 on 236,
//! 8 after 255.
//!
//! Library: `spawn::scatter` (0x1E00120), `trail::sparks` (0x1E002E0)
//! [`crate::blib`], `sprites` (0x1E01040), `disc` (0x1E02930)
//! [`crate::fxlib`].
//!
//! b496 (card 590) is the same program with other nodes, colours and
//! sizes ([`Variant`], `crate::b496`).
use crate::b057::spark_place;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The differences between b269 and b496.
pub struct Variant {
    /// Node requests (slot address, node, clear); the slot the set-up
    /// update reads; the slot every update reads; the stage light byte.
    pub nodes: &'static [(u32, i32, i32)],
    pub setup_slot: u32,
    pub slot: u32,
    pub light: u32,
    /// The burst's sprite size (f32 bits); its frames rolled (rand % 8
    /// returning to the address) or 0.
    pub burst_size: u32,
    pub burst_roll: Option<u32>,
    /// The burst's rand return address; the spark placer's (set-up).
    pub burst_ra: u32,
    pub place_ras: [u32; 3],
    /// Green sparks (b234's group) instead of white.
    pub green: bool,
    /// The blue byte's modulus.
    pub blue: i32,
    /// The step code's offset from b269's (return addresses).
    pub shift: u32,
}

pub const B269: Variant = Variant {
    nodes: &[(0x2AB0, 0x38, 1)],
    setup_slot: 0x2AB0,
    slot: 0x2AB0,
    light: 0x2AB4,
    burst_size: 0x3F80_0000,
    burst_roll: None,
    burst_ra: 0x01E0_303C,
    place_ras: [0x01E0_3128, 0x01E0_3144, 0x01E0_3170],
    green: false,
    blue: 128,
    shift: 0,
};

/// The entry 0x1E02E90.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B269)
}

pub fn run(fx: &mut Fx, param: i32, v: &Variant) -> i32 {
    let sp = fx.enter(0xD0);
    let ret = if param >= 0 { init(fx, sp, param, v) } else { step(fx, sp, v) };
    fx.leave(0xD0);
    ret
}

#[allow(clippy::too_many_arguments)]
fn sparks_group(fx: &mut Fx, v: &Variant, g: u32, tex: u32, flags: u32, alpha: u8, width: u32, length: u32) {
    if v.green {
        crate::b234::sparks_group(fx, g, tex, flags, alpha, width, length);
    } else {
        crate::b447::sparks_group(fx, g, tex, flags, alpha, width, length);
    }
}

fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

/// Burst sprite `p` (D + 16 k, fields at +0x2290) thrown in a random
/// direction of length 8 (0x1043F0 into sp + 0x90).
fn burst_throw(fx: &mut Fx, sp: u32, p: u32) {
    fx.random_dir(sp + 0x90, 8.0);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x90 + 4 * c);
        fx.mem.write_u32(p + 0x2290 + 4 * c, w);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32, v: &Variant) -> i32 {
    fx.bzero(D, 0x2AC0);
    for x in [0u32, 0x2140, 0x70, 0x21C0] {
        fx.xform_init(D + x);
    }
    for &(at, node, clear) in v.nodes {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2AA0, 0x100);
    fx.mem.set32(D + 0x2AA4, 1);
    fx.mem.set32(D + 0x2AA8, param);
    for k in [v.light, 0x2AAF, 0x2AAC, 0x2AAD, 0x2AAE] {
        fx.mem.write_u8(D + k, 0x80);
    }
    // The disc (+0x21B0), radius 3.
    let g = D + 0x21B0;
    fx.mem.write_u32(g, 0x8_0000);
    for (k, x) in [(6u32, 0x80u8), (5, 0x40), (4, 0x40), (7, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 8, 0x4040_0000);
    fx.mem.set16(g + 0xC, 0x24);
    fx.mem.set16(g + 0xE, 0);
    fx.texture5(sp + 0xC0, 0);
    // The burst (+0x2230): 128 sprites at +0x2290, size 1, radius 15.
    let g = D + 0x2230;
    let tex = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0xC, v.burst_size);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x2290);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4170_0000);
    halves(fx, g, &[(0x5A, 0x80), (0x5C, 8), (0x5E, 0)]);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x228A) {
        let p = D + 16 * k as u32;
        burst_throw(fx, sp, p);
        let r = fx.rand_from(D, v.burst_ra);
        let (_, h) = fx.div(r, 0xC);
        k += 1;
        fx.mem.set16(p + 0x229E, h.wrapping_neg());
        fx.mem.write_u8(p + 0x229C, 0);
        let roll = match v.burst_roll {
            Some(ra) => fx.rand_from(D, ra) % 8,
            None => 0,
        };
        fx.mem.set8(p + 0x229D, roll);
    }
    let tex = fx.mem.read_u32(sp + 0xC0);
    sparks_group(fx, v, D + 0xE0, tex, 0x6000_0003, 0x80, 0x3ECC_CCCD, 0x41B0_0000);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x10C) {
        spark_place(fx, k as u32, 0x7D0, v.place_ras);
        let e = fx.mem.read_u32(D + 0xEC).wrapping_add(16 * k as u32);
        fx.mem.write_u8(e + 0xC, 0);
        k += 1;
    }
    let tex = fx.mem.read_u32(sp + 0xC0);
    sparks_group(fx, v, D + 0x1110, tex, 3, 0, 0x3E4C_CCCD, 0x4120_0000);
    0
}

fn step(fx: &mut Fx, sp: u32, v: &Variant) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2AA4);
    let node = |fx: &mut Fx, slot: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + slot);
        fx.node_position(h, s, out);
    };
    let ra = |a: u32| a + v.shift;
    let first = n(fx) == 0x46;
    if first {
        let t = fx.target_handle();
        fx.unit_position(t, D + 0x2A90);
        node(fx, v.setup_slot, D + 0x90);
        let t = fx.target_handle();
        fx.unit_position(t, sp + 0xA0);
        node(fx, v.setup_slot, sp + 0xB0);
        fx.look_angles(sp + 0xA0, sp + 0xB0, D + 0x80);
    }
    node(fx, v.slot, D + 0x21E0);
    node(fx, v.slot, D + 0x2160);
    // s0 = D, s1 = the OT, s2 = sp + 0xB0 after the set-up update, else
    // the parameter (-1, sign-extended: not modelled), s3 the caller's.
    let regs = [Some(D), Some(OT), first.then_some(sp + 0xB0), None];
    fx.xform_world_saving(D, ra(0x01E0_3348), regs);
    fx.xform_billboard_saving(D + 0x2140, ra(0x01E0_3350), regs);
    fx.xform_world_saving(D + 0x70, ra(0x01E0_3358), regs);
    fx.xform_world_saving(D + 0x21C0, ra(0x01E0_3360), regs);
    if n(fx) >= 0xEC {
        fx.step_up(D + v.light, 0x80, 4);
    } else {
        fx.step_down(D + v.light, 0x20, 4);
    }
    let light = fx.mem.u8(D + v.light);
    fx.set_stage_light(light);
    if (0x5C..0xEC).contains(&n(fx)) {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, D + 0x2AAC, [D, OT], ra(0x01E0_33D4));
        for (at, a, m) in [(0x2AAEu32, ra(0x01E0_33DC), 128), (0x2AAD, ra(0x01E0_33F8), 128), (0x2AAC, ra(0x01E0_3414), v.blue)] {
            let r = fx.rand_from(D, a);
            fx.mem.set8(D + at, r % m);
        }
    }
    if n(fx) == 0xEC {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, 0, [D, OT], ra(0x01E0_3450));
    }
    if n(fx) == 0x46 {
        fx.mem.write_u32(D + 0x21B8, 0x40C0_0000);
    }
    fx.disc(D + 0x21B0, D + 0x2140);
    // The disc's fp / ra saves, under the sprites' TEX0 / TEX1 slots.
    fx.saved(sp - 0x140, &[(0x90, 0), (0xA0, u64::from(ra(0x01E0_3478)))]);
    if n(fx) >= 0xEC {
        fx.step_down(D + 0x21B7, 0, 0x10);
    } else {
        fx.step_up(D + 0x21B7, 0xFF, 0x10);
    }
    fx.sprites(D + 0x2230, D + 0x21C0);
    if n(fx) >= 0x35 {
        fx.step_down(D + 0x223B, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x228A) {
        let p = D + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x229E) >= 0 {
            fx.step_up(p + 0x229C, 0x80, 0x10);
            let s = f32::from_bits(0x3F4C_CCCD);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 0x2290 + 4 * c), s);
                fx.mem.setf(p + 0x2290 + 4 * c, x);
            }
        }
        if fx.mem.i16(p + 0x229E) < 8 {
            continue;
        }
        burst_throw(fx, sp, p);
        fx.mem.write_u8(p + 0x229C, 0);
        fx.mem.set16(p + 0x229E, 0);
    }
    crate::blib::trail::sparks(fx, OT, D + 0xE0, D + 0x70);
    if n(fx) >= 0xED {
        fx.step_down(D + 0xEB, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x10C) {
        let p = D + 16 * k as u32;
        if fx.mem.i16(p + 0x11E) >= 0 {
            let y = ee::add(fx.mem.f(p + 0x114), 4.0);
            fx.mem.setf(p + 0x114, y);
            fx.step_up(p + 0x11C, 0x80, 0x10);
            if !ee_le(fx.mem.f(p + 0x114), 200.0) {
                spark_place(fx, k as u32, 0x1E, [ra(0x01E0_3630), ra(0x01E0_364C), ra(0x01E0_3678)]);
                fx.mem.write_u8(p + 0x11C, 0);
                fx.mem.set16(p + 0x11E, 0);
            }
        }
        k += 1;
    }
    if n(fx) == 0x46 {
        fx.mem.write_u8(D + 0x111B, 0x80);
    }
    crate::blib::trail::sparks(fx, OT, D + 0x1110, D + 0x21C0);
    if n(fx) >= 0xED {
        fx.step_down(D + 0x111B, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x113C) {
        let p = D + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x114E) < 0 {
            continue;
        }
        if n(fx) >= 0x46 {
            fx.step_up(p + 0x114C, 0x80, 0x10);
        }
        let s = f32::from_bits(0x3FA6_6666);
        for c in 0..3u32 {
            let x = ee::mul(fx.mem.f(p + 0x1140 + 4 * c), s);
            fx.mem.setf(p + 0x1140 + 4 * c, x);
        }
        if fx.mem.i16(p + 0x114E) < 0x11 {
            continue;
        }
        fx.random_dir(sp + 0x80, 1.0);
        for c in [1u32, 2, 0] {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * c);
            fx.mem.write_u32(p + 0x1140 + 4 * c, w);
        }
        let r = fx.rand_from(D, ra(0x01E0_37F8));
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0x114E, h.wrapping_neg());
        fx.mem.write_u8(p + 0x114C, 0);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2AA4, m);
    let left = fx.mem.i32(D + 0x2AA0) - 1;
    fx.mem.set32(D + 0x2AA0, left);
    if m == 0xEC {
        2
    } else if m == 0x51 {
        1
    } else if (0x5B..0xEC).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
