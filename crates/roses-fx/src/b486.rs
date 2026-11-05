//! BMODULE b486 (card 253), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02E70, data area 0x1ED0000) [F] (helper B7, 31
//! Oct 2026): the phase-1 library family (c620's transforms and groups).
//! Every update a frame quad of (0x80, 0x80, 0x80, 0x60) and the stage
//! light (+0x7CCC: down by 4 to 0x20 until update 105, back up by 4 to
//! 0x80 from 310); two sprite fans on the attacker's node 8 (+0x150, 512
//! sprites flying out and growing back in, +0x4220, 256 rising puffs; both
//! drawn by the record's 0x1E03840 `fans`); on updates 21..49 a
//! particle-quad cloud (+0x7B60, TEXEFF 15) at node 8, fading from 34; on
//! 129..309 a ring (+0x72F0) at the target growing to 2.0 with random
//! colours and the target's added colour flickering (0x1F7790, cleared on
//! 310). Returns 1 on update 118, 6 on 128..309, 2 on 310, 8 after 329.
//!
//! Library (record addresses): `frame_quad_rgba` 0x1E00120 (b26f4b13c68d,
//! [`crate::framegs`]), `sprites` 0x1E00240, `particle_quads` 0x1E01B30,
//! `ring_setup` 0x1E026C0, `ring_draw` 0x1E02980 [`crate::fxlib`]; the
//! executable's 0x1F6A60 [`crate::b107::target_radius`].
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

const D: u32 = crate::bmodule::DATA_AREA;
const N: u32 = D + 0x7CB4;

/// The entry 0x1E02E70.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn halves(fx: &mut Fx, at: u32, v: &[(u32, i32)]) {
    for &(k, x) in v {
        fx.mem.set16(at + k, x);
    }
}

/// A fan sprite's speed (+0 / +8 of the particle at `p`) from the 16-bit
/// angle `a`: `r` cos (0x20ABF0) into +8, `r` sin (0x20ACC0) into +0.
fn fan_dir(fx: &mut Fx, p: u32, a: i32, r: f32) {
    let c = fx.table_trig(a, true);
    fx.mem.setf(p + 8, ee::mul(r, c));
    let s = fx.table_trig(a, false);
    fx.mem.setf(p, ee::mul(r, s));
}

/// rand() & 0xFFFF (the compiler's % 0x10000 of a non-negative int),
/// rand's frame left (the caller's s0 = `s0`, return address `ra`).
fn rand16(fx: &mut Fx, s0: u32, ra: u32) -> i32 {
    fx.rand_from(s0, ra) & 0xFFFF
}

fn rem(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let r = fx.rand_from(s0, ra);
    fx.div(r, m).1
}

/// A puff's respawn [F 0x1E03354 / 0x1E03AF0]: speed 0.1 (rand % (10
/// +0x426C)) along a random angle, rise 0.1 (20 - rand % 40), age -(rand %
/// 12), alpha 0x80; rand called with s0 = `s0` from the return addresses
/// `ras`.
fn puff(fx: &mut Fx, p: u32, s0: u32, ras: [u32; 4]) {
    let s4 = ee::fixsfsi(ee::mul(10.0, fx.mem.f(D + 0x426C)));
    let a = rand16(fx, s0, ras[0]);
    let h = rem(fx, s0, ras[1], s4);
    let r = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
    let h = rem(fx, s0, ras[2], 0x28) - 0x14;
    let up = ee::neg(ee::from_i32(h));
    fx.mem.setf(p + 0x4284, ee::mul(f32::from_bits(0x3DCC_CCCD), up));
    fan_dir(fx, p + 0x4280, a, r);
    let h = rem(fx, s0, ras[3], 0xC);
    fx.mem.set16(p + 0x428E, h.wrapping_neg());
    fx.mem.write_u8(p + 0x428C, 0x80);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x7CD0);
    for x in [0u32, 0x70, 0xE0, 0x41B0, 0x7AF0, 0x7280] {
        fx.xform_init(D + x);
    }
    for c in [8u32, 4, 0] {
        fx.mem.write_u32(D + 0x7280 + c, 0x3DCC_CCCD);
    }
    for (at, node, clear) in [(0x7CBCu32, 8, 1), (0x7CC0, 0xA, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x7CB0, 0x149);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x7CB8, param);
    let r = crate::b107::target_radius(fx);
    fx.mem.setf(D + 0x7CC8, r);
    for k in [0x7CCCu32, 0x7CC7, 0x7CC4, 0x7CC5, 0x7CC6] {
        fx.mem.write_u8(D + k, 0x80);
    }
    fx.texture4(sp + 0xA0, 0xF);
    // The ring (+0x72F0), TEXEFF 15.
    let g = D + 0x72F0;
    fx.mem.write_u32(g + 4, 0x2000_0000);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x80)]);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.ring_setup(g, tex, f32::from_bits(0x4133_3333), f32::from_bits(0x414C_CCCD), 0.0, 0.0);
    // The cloud (+0x7B60): 16 quads of size 5, TEXEFF 15.
    let g = D + 0x7B60;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x0008_0083);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0x80), (0x2A, 0xC0), (0x24, 0x20), (0x26, 0x20), (0x2C, 0x80), (0x2E, 0x40)]);
    fx.mem.write_u32(g + 0xC, 0x40A0_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 2), (0x13C, 1), (0x140, 8), (0x136, 0), (0x138, 0)]);
    for (k, v) in [(0xAu32, 0x80u8), (9, 0x60), (8, 0x70), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, v);
    }
    for k in 0..0x10i32 {
        let p = g + 16 * k as u32;
        for (c, ra) in [(0u32, 0x01E0_3068u32), (1, 0x01E0_309C)] {
            let r = fx.rand_from(D, ra);
            let base = fx.mem.f(g + 0x10 + 4 * c);
            let h = fx.div(r, 6).1;
            fx.mem.setf(p + 0x30 + 4 * c, ee::add(base, ee::from_i32(h - 3)));
        }
        let z = fx.mem.read_u32(g + 0x18);
        fx.mem.write_u32(p + 0x38, z);
        let a = fx.mem.i16(g + 0x13A).wrapping_neg();
        let t = fx.mult(a, k);
        fx.mem.set16(p + 0x3E, t);
        fx.mem.set16(p + 0x3C, 0);
        let n = fx.mem.i16(g + 0x140) - 1;
        fx.mem.set16(g + 0x13E, n);
    }
    fx.texture5(sp + 0xA0, 0);
    // The out fan (+0x150): 512 sprites of the attacker's texture 0.
    let g = D + 0x150;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0xD000_0003);
    for (k, v) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, 0)] {
        fx.mem.write_u8(g + k, v);
    }
    fx.mem.write_u32(g + 0xC, 0x4040_0000);
    fx.mem.write_u32(g + 0x20, 0);
    fx.mem.write_u32(g + 0x24, 0x800);
    fx.mem.write_u32(g + 0x28, 0);
    halves(fx, g, &[(0x30, 0), (0x32, 0), (0x34, 0x80), (0x36, 0x40), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x1B0);
    fx.mem.write_u32(g + 0x44, D + 0x21B0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4000_0000);
    fx.mem.write_u32(g + 0x54, 0x41C8_0000);
    halves(fx, g, &[(0x58, 0x400), (0x5A, 0x200), (0x5C, 1), (0x5E, 0)]);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x1AA) {
        let p = D + 16 * k as u32;
        let a = rand16(fx, D, 0x01E0_31C8);
        let h = rem(fx, D, 0x01E0_31E4, 0x7D0);
        fx.mem.setf(p + 0x1B4, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h)));
        let r = fx.mem.f(D + 0x19C);
        fan_dir(fx, p + 0x1B0, a, r);
        let h = rem(fx, D, 0x01E0_323C, 0x14);
        fx.mem.set16(p + 0x1BE, h.wrapping_neg());
        fx.mem.write_u8(p + 0x1BC, 0);
        fx.mem.write_u8(p + 0x1BD, 0);
        let v = rand16(fx, D, 0x01E0_3264);
        fx.mem.set32(p + 0x21B0, v);
        let v = rand16(fx, D, 0x01E0_3280);
        fx.mem.set32(p + 0x21B4, v);
        k += 1;
    }
    // The puffs (+0x4220): 256 sprites of the attacker's texture 0.
    let g = D + 0x4220;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0xF000_0003);
    for (k, v) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, 0)] {
        fx.mem.write_u8(g + k, v);
    }
    fx.mem.write_u32(g + 0xC, 0x4040_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0), (0x34, 0x80), (0x36, 0x40), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x4280);
    fx.mem.write_u32(g + 0x44, D + 0x5280);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4080_0000);
    fx.mem.write_u32(g + 0x54, 0);
    halves(fx, g, &[(0x58, 0), (0x5A, 0x100), (0x5C, 1), (0x5E, 0)]);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x427A) {
        let p = D + 16 * k as u32;
        puff(fx, p, D, [0x01E0_3370, 0x01E0_338C, 0x01E0_33B4, 0x01E0_340C]);
        let v = rand16(fx, D, 0x01E0_3434);
        fx.mem.set32(p + 0x5280, v);
        let v = rand16(fx, D, 0x01E0_3450);
        fx.mem.set32(p + 0x5284, v);
        k += 1;
    }
    0
}

/// The record's 0x1E03840(D, ot) [F] (842f625273fa, b486 only): the two
/// fans drawn (`sprites`) and stepped. Out fan (+0x150): each live
/// sprite's distance +0x1B4 += 6, its speed x 0.8 while the distance is
/// negative, else x 1.12; spins +0x21B0 / +0x21B4 += 0x100; alpha up by
/// 0x20 from update 95; past 200 a respawn (random angle, age -(rand %
/// 20), distance 0.1 (rand % 200), alpha 0). Puffs (+0x4220): rise +0x4284
/// += 2, speed x 1.2, alpha down by 8; past 20 a respawn. The groups'
/// alphas set to 0x80 from update 95, the out fan's fading by 16 from 311.
fn fans(fx: &mut Fx) {
    fx.enter(0x70);
    if fx.mem.i32(N) >= 0x5F {
        fx.mem.write_u8(D + 0x15B, 0x80);
    }
    fx.sprites(D + 0x150, D + 0xE0);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x1AA) {
        let p = D + 16 * k as u32;
        let s0 = k as u32;
        k += 1;
        if fx.mem.i16(p + 0x1BE) < 0 {
            continue;
        }
        let d = ee::add(fx.mem.f(p + 0x1B4), 6.0);
        fx.mem.setf(p + 0x1B4, d);
        let s = if crate::exe::ee_lt(d, 0.0) { f32::from_bits(0x3F4C_CCCD) } else { f32::from_bits(0x3F8F_5C29) };
        for o in [0x1B8u32, 0x1B0] {
            let v = ee::mul(fx.mem.f(p + o), s);
            fx.mem.setf(p + o, v);
        }
        for o in [0x21B0u32, 0x21B4] {
            let v = fx.mem.i32(p + o).wrapping_add(0x100);
            fx.mem.set32(p + o, v);
        }
        if fx.mem.i32(N) >= 0x5F {
            fx.step_up(p + 0x1BC, 0x80, 0x20);
        }
        if !crate::exe::ee_lt(fx.mem.f(p + 0x1B4), 200.0) {
            let a = rand16(fx, s0, 0x01E0_3980);
            let h = rem(fx, s0, 0x01E0_399C, 0x14);
            fx.mem.set16(p + 0x1BE, h.wrapping_neg());
            let r = fx.mem.f(D + 0x19C);
            fan_dir(fx, p + 0x1B0, a, r);
            let h = rem(fx, s0, 0x01E0_39E0, 0xC8);
            fx.mem.setf(p + 0x1B4, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h)));
            fx.mem.write_u8(p + 0x1BC, 0);
        }
    }
    if fx.mem.i32(N) >= 0x137 {
        fx.step_down(D + 0x15B, 0, 0x10);
    }
    fx.sprites(D + 0x4220, D + 0x41B0);
    if fx.mem.i32(N) >= 0x5F {
        fx.mem.write_u8(D + 0x422B, 0x80);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x427A) {
        let p = D + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x428E) < 0 {
            continue;
        }
        let v = ee::add(fx.mem.f(p + 0x4284), 2.0);
        fx.mem.setf(p + 0x4284, v);
        let s = f32::from_bits(0x3F99_999A);
        for o in [0x4280u32, 0x4288] {
            let v = ee::mul(fx.mem.f(p + o), s);
            fx.mem.setf(p + o, v);
        }
        fx.step_down(p + 0x428C, 0, 8);
        if !crate::exe::ee_le(fx.mem.f(p + 0x4284), 20.0) {
            puff(fx, p, p, [0x01E0_3B0C, 0x01E0_3B28, 0x01E0_3B50, 0x01E0_3BA8]);
        }
    }
    fx.leave(0x70);
}

/// rand() % 128 (the compiler's signed remainder by masks).
fn rand7(fx: &mut Fx, ra: u32) -> u8 {
    fx.rand_from(D, ra).wrapping_rem(0x80) as u8
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x72A0);
    for out in [D + 0x7B10, D + 0x100] {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x7CBC);
        fx.node_position(h, s, out);
    }
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x70);
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x7CBC);
    fx.node_position(h, s, sp + 0x80);
    fx.look_angles(sp + 0x70, sp + 0x80, D + 0xF0);
    for out in [D + 0x41D0, sp + 0x70] {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x7CC0);
        fx.node_position(h, s, out);
    }
    fx.look_angles(sp + 0x70, sp + 0x80, D + 0x41C0);
    // s0 = D, s1 = the OT, s2 = sp + 0x80, s3 the runner's.
    let regs = [Some(D), Some(crate::exe::OT), Some(sp + 0x80), None];
    fx.xform_world_saving(D, 0x01E0_3554, regs);
    fx.xform_billboard_saving(D + 0x70, 0x01E0_355C, regs);
    fx.xform_billboard_saving(D + 0x7AF0, 0x01E0_3564, regs);
    for (x, ra) in [(0xE0u32, 0x01E0_356Cu32), (0x41B0, 0x01E0_3574), (0x7280, 0x01E0_357C)] {
        fx.xform_world_saving(D + x, ra, regs);
    }
    fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    // 0x207EF0's frame below the frame quad's (sq ra 0x1E0020C), read as
    // the sprites' missing-texture TEX1.
    fx.saved(sp - 0x80, &[(0, 0x01E0_020C)]);
    let n = fx.mem.i32(N);
    if n < 0x69 {
        fx.step_down(D + 0x7CCC, 0x20, 4);
    } else if n >= 0x136 {
        fx.step_up(D + 0x7CCC, 0x80, 4);
    }
    let l = fx.mem.u8(D + 0x7CCC);
    fx.set_stage_light(l);
    let n = fx.mem.i32(N);
    if (0x15..0x32).contains(&n) {
        for k in [0x98u32, 0x94, 0x90] {
            fx.mem.write_u32(sp + k, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0x7AF0, D + 0x7B60, sp + 0x90);
        if fx.mem.i32(N) >= 0x22 {
            fx.step_down(D + 0x7B6B, 0, 2);
        }
    }
    let n = fx.mem.i32(N);
    if (0x81..0x136).contains(&n) {
        let t = fx.target_handle();
        fx.unit_add_colour(t, D + 0x7CC4);
        for (k, ra) in [(0x7CC6u32, 0x01E0_3670u32), (0x7CC5, 0x01E0_368C), (0x7CC4, 0x01E0_36A8)] {
            let v = rand7(fx, ra);
            fx.mem.write_u8(D + k, v);
        }
        fx.ring_draw(D + 0x72F0, D + 0x7280);
        fx.step_down(D + 0x72FB, 0, 8);
        for (k, ra) in [(0x72FAu32, 0x01E0_36E8u32), (0x72F9, 0x01E0_3704), (0x72F8, 0x01E0_3720)] {
            let v = rand7(fx, ra);
            fx.mem.write_u8(D + k, v);
        }
        if fx.approach(D + 0x7280, 2.0, f32::from_bits(0x3E4C_CCCD)) == 0 {
            for c in [8u32, 4, 0] {
                fx.mem.write_u32(D + 0x7280 + c, 0x3DCC_CCCD);
            }
            fx.mem.write_u8(D + 0x72FB, 0x80);
        }
    }
    fans(fx);
    let n = fx.mem.i32(N) + 1;
    fx.mem.set32(N, n);
    let left = fx.mem.i32(D + 0x7CB0) - 1;
    fx.mem.set32(D + 0x7CB0, left);
    if n == 0x136 {
        let t = fx.target_handle();
        fx.unit_add_colour(t, 0);
        2
    } else if n == 0x76 {
        1
    } else if (0x80..0x136).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
