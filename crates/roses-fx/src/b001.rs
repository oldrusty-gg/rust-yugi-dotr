//! BMODULE b001 (card 35), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E046C0, data area 0x1ED0000) [F] (helper A26, 3
//! Nov 2026): a b129-style program. The launch prints the data size
//! (printf 0x19A410, answered by the runner). The stage dims (0x1F7F10)
//! until update 0x96. On updates 0x28..0xB4 at the attacker: the frame
//! quad, a disc (+0xE0), four flat rings (+0x5D0 + 0x400 k, radii 6 (k +
//! 1) / 6 (k + 1) - 4), a quad burst (+0x100) and the attacker split and
//! zoomed (`zoom::quarters`, +0xF0, 10 layers), from 0x97 the rings
//! swelling to 15 and the attacker's billboard to 10. From 0xB5 at the
//! target: two rings (+0x2710 / +0x2B80, the second 40 below) turning
//! and a sprite cloud (+0x1640) rising in three speeds and respawned
//! below above 35; from 0x154 all shrinking away. From 0x157 a window
//! tint (+0x3140) fading by 8; from 0x166 a particle burst (+0x2FF0)
//! fading by 2. Returns 1 on update 0xAB, 6 on 0xB5..0x153, 2 on 0x154,
//! 8 after 0x17C.
//!
//! Library at its own addresses: `colours4` (0x1E00080), `b262::tint`
//! (0x1E00120), `zoom::quarters` (0x1E00280), the frame quad (0x1E00B10),
//! `particle_quads` (0x1E00C30), `disc` (0x1E017C0), `sprites_spawn` /
//! `sprites` (0x1E01D20 / 0x1E01EB0), `ring_setup` / `ring_draw`
//! (0x1E037A0 / 0x1E03A60), `quads_setup` / `quads_draw` (0x1E03F50 /
//! 0x1E040B0).
use crate::b107::approach_down;
use crate::b262::tint;
use crate::b436::fstep_down;
use crate::blib::zoom;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad corner template (rodata, 0x1E03F50's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_5280;
/// The launch's printf format (record rodata).
const FORMAT: u32 = 0x01E0_5300;
const LEFT: u32 = D + 0x3144;
const N: u32 = D + 0x3148;
const LIGHT: u32 = D + 0x3158;

/// The entry 0x1E046C0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x70);
    ret
}

fn bytes(fx: &mut Fx, at: &[(u32, u8)]) {
    for &(o, v) in at {
        fx.mem.write_u8(D + o, v);
    }
}

fn halves(fx: &mut Fx, at: &[(u32, i32)]) {
    for &(o, v) in at {
        fx.mem.set16(D + o, v);
    }
}

/// A ring group's look at `g` (D-relative): cell (0x8E, 0, 0x20, 0x80),
/// colour (0xE0, 0xB0, 0x70) and alpha `a`.
fn ring_look(fx: &mut Fx, g: u32, flags: u32, a: u8) {
    fx.mem.write_u32(D + g + 4, flags);
    halves(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x80)]);
    bytes(fx, &[(g + 0xA, 0x70), (g + 9, 0xB0), (g + 8, 0xE0), (g + 0xB, a)]);
}

/// rand % m (HI) from the `jal` returning to `ra` (rand's frame: s0 = D).
fn rmod(fx: &mut Fx, ra: u32, m: i32) -> i32 {
    let r = fx.rand_from(D, ra);
    fx.div(r, m).1
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.boundary(0x0019_A410, "printf", [FORMAT, 0x3160, 0, 0]);
    fx.bzero(D, 0x3160);
    for x in [0u32, 0x70, 0x15D0, 0x26A0, 0x2B10, 0x2F80] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x3150u32, 0x12, 1), (0x3154, 1, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(LEFT, 0x17C);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x314C, param);
    fx.mem.set32(LIGHT, 0x80);
    bytes(fx, &[(0x3140, 0x80), (0x3141, 0x80), (0x3142, 0x80), (0x3143, 0xFF)]);
    // The disc (+0xE0): radius 20.
    fx.mem.write_u32(D + 0xE0, 0);
    bytes(fx, &[(0xE6, 0x70), (0xE5, 0xB0), (0xE4, 0xE0), (0xE7, 0)]);
    fx.mem.write_u32(D + 0xE8, 0x41A0_0000);
    halves(fx, &[(0xEC, 0x24), (0xEE, 0)]);
    // The zoom (+0xF0) of the attacker: 10 layers.
    fx.mem.write_u32(D + 0xF0, 0);
    bytes(fx, &[(0xF6, 0x60), (0xF5, 0x60), (0xF4, 0x80), (0xF7, 0)]);
    fx.mem.write_u32(D + 0xF8, 0x3E99_999A);
    halves(fx, &[(0xFC, 0xA)]);
    let h = fx.self_handle();
    halves(fx, &[(0xFE, h)]);
    fx.texture4(sp + 0x60, 0xF);
    // The quad burst (+0x100): size 0.8.
    fx.mem.write_u32(D + 0x104, 0x4_0001);
    halves(fx, &[(0x110, 0x8E), (0x112, 0), (0x114, 0x22), (0x116, 0x40)]);
    bytes(fx, &[(0x10A, 0xD0), (0x109, 0xA0), (0x108, 0x70), (0x10B, 0)]);
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.quads_setup(D + 0x100, tex, f32::from_bits(0x3F4C_CCCD), QUAD_TEMPLATE);
    // Two target rings (+0x2710 at 0, +0x2B80 at -40): radii 18, width 80.
    for (g, h0) in [(0x2710u32, 0.0f32), (0x2B80, -40.0)] {
        ring_look(fx, g, 0, 0x20);
        let tex = fx.mem.read_u32(sp + 0x60);
        fx.ring_setup(D + g, tex, 18.0, 18.0, h0, 80.0);
    }
    // Four flat rings (+0x5D0 + 0x400 k): radii 6 (k + 1) / 6 (k + 1) - 4.
    for k in 0..4u32 {
        let g = 0x5D0 + 0x400 * k;
        let outer = ee::mul(6.0, ee::from_i32(k as i32 + 1));
        ring_look(fx, g, 0x2000_0000, 0);
        let tex = fx.mem.read_u32(sp + 0x60);
        fx.ring_setup(D + g, tex, outer, ee::sub(outer, 4.0), 0.0, 0.0);
    }
    fx.texture5(sp + 0x60, 0);
    // The particle burst (+0x2FF0): 8 particles at +0x3020.
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.mem.write_u32(D + 0x2FF0, tex);
    fx.mem.write_u32(D + 0x2FF4, 0x8_0007);
    for o in [0x3000u32, 0x3004, 0x3008] {
        fx.mem.write_u32(D + o, 0);
    }
    halves(fx, &[(0x3010, 0), (0x3012, 0), (0x3014, 0x20), (0x3016, 0x20), (0x3018, 0), (0x301A, 0), (0x301C, 0x80), (0x301E, 0x40)]);
    fx.mem.write_u32(D + 0x2FFC, 0x3F80_0000);
    halves(fx, &[(0x3120, 0x10), (0x312A, 1), (0x312C, 0), (0x3130, 4), (0x3126, 0xA), (0x3128, 0xA)]);
    bytes(fx, &[(0x2FFA, 0x80), (0x2FF9, 0x80), (0x2FF8, 0x80), (0x2FFB, 0x40)]);
    for j in 0..8i32 {
        let p = D + 0x10 * j as u32;
        let x = rmod(fx, 0x01E0_4A68, 0x14) - 0xA;
        fx.mem.setf(p + 0x3020, ee::from_i32(x));
        let y = rmod(fx, 0x01E0_4A94, 0x14) - 0xA;
        fx.mem.setf(p + 0x3024, ee::from_i32(y));
        let s = fx.mem.i16(D + 0x312A).wrapping_neg();
        let f = fx.mult(s, j);
        fx.mem.set16(p + 0x302E, f);
        fx.mem.set16(p + 0x302C, 0);
        let c = fx.mem.i16(D + 0x3130) - 1;
        fx.mem.set16(D + 0x312E, c);
    }
    // The sprite cloud (+0x1640): 256 sprites at +0x16A0, size 12, radius
    // 15, 35 below.
    let g = D + 0x1640;
    let tex = fx.mem.read_u32(sp + 0x60);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x5000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0xC, 0x4140_0000);
    fx.mem.write_u32(g + 0x20, 0);
    fx.mem.write_u32(g + 0x24, 0x400);
    fx.mem.write_u32(g + 0x28, 0);
    halves(fx, &[(0x1670, 0), (0x1672, 0), (0x1674, 0x20), (0x1676, 0x20), (0x1678, 0), (0x167A, 0), (0x167C, 0x80), (0x167E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x16A0);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4170_0000);
    halves(fx, &[(0x169A, 0x100), (0x169C, 8), (0x169E, 0)]);
    fx.sprites_spawn(g, 12.0);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x169A) {
        let p = D + 0x16A0 + 16 * k as u32;
        let h = rmod(fx, 0x01E0_4BA0, 0xC);
        fx.mem.set16(p + 0xE, h.wrapping_neg());
        fx.mem.write_u8(p + 0xC, 0);
        fx.mem.write_u8(p + 0xD, 0);
        let y = ee::sub(fx.mem.f(p + 4), 35.0);
        fx.mem.setf(p + 4, y);
        k += 1;
    }
    fx.mem.write_u32(D + 0x74, 0x3FC0_0000);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x2FA0);
    for (x, y) in [(0x15D0u32, 0x4248_0000u32), (0x26A0, 0x4170_0000), (0x2B10, 0x425C_0000)] {
        let t = fx.target_handle();
        fx.unit_point(1, t, D + x + 0x20);
        fx.mem.write_u32(D + x + 8, 0x3DCC_CCCD);
        fx.mem.write_u32(D + x, 0x3DCC_CCCD);
        fx.mem.write_u32(D + x + 0x24, y);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    for out in [0x90u32, 0x580] {
        let h = fx.self_handle();
        fx.unit_point(1, h, D + out);
    }
    let r = fx.mem.read_u32(D + 0x26B4).wrapping_add(0x400);
    fx.mem.write_u32(D + 0x26B4, r);
    // s0 = D, s1 = the OT, s2 = the parameter, s3 the runner's 0.
    let regs = [Some(D), Some(OT), Some(param as u32), Some(0)];
    for (x, ra, board) in [(0u32, 0x01E0_4CD4u32, false), (0x70, 0x01E0_4CDC, true), (0x560, 0x01E0_4CE4, true), (0x15D0, 0x01E0_4CEC, false), (0x26A0, 0x01E0_4CF4, false), (0x2B10, 0x01E0_4CFC, false), (0x2F80, 0x01E0_4D04, true)] {
        if board {
            fx.xform_billboard_saving(D + x, ra, regs);
        } else {
            fx.xform_world_saving(D + x, ra, regs);
        }
    }
    if n(fx) < 0x96 {
        fx.step_down(LIGHT, 0x20, 4);
    } else {
        fx.step_up(LIGHT, 0x80, 4);
    }
    let l = fx.mem.read_u8(LIGHT);
    fx.set_stage_light(i32::from(l));
    if n(fx) >= 0x157 {
        tint(fx, D + 0x3140);
        fx.step_down(D + 0x3143, 0, 8);
    }
    if (0x28..0xB5).contains(&n(fx)) {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
        fx.disc(D + 0xE0, D + 0x70);
        fx.step_up(D + 0xE7, 0xFF, 0x10);
        for k in 0..4u32 {
            ring(fx, sp, D + 0x5D0 + 0x400 * k, D + 0x560, 0x01E0_4DD0);
            fx.step_up(D + 0x5DB + 0x400 * k, 0x40, 4);
        }
        fx.quads_draw(D + 0x100, D + 0x70);
        fx.step_up(D + 0x10B, 0x10, 1);
        for k in 0..0x40u32 {
            for c in 0..3u32 {
                let a = D + 0x160 + 0x10 * k + 4 * c;
                let w = fx.mem.read_u32(a).wrapping_add(0x40);
                fx.mem.write_u32(a, w);
            }
        }
        zoom::quarters(fx, D + 0xF0, D + 0x90);
        fx.step_up(D + 0xF7, 0x40, 4);
        if n(fx) >= 0x97 {
            fx.approach(D + 0x560, 15.0, 1.0);
            fx.approach(D + 0x70, 10.0, f32::from_bits(0x3DCC_CCCD));
        }
    }
    if n(fx) >= 0x166 {
        for o in [0x58u32, 0x54, 0x50] {
            fx.mem.write_u32(sp + o, 0);
        }
        fx.particle_quads(D + 0x2F80, D + 0x2FF0, sp + 0x50);
        fx.step_down(D + 0x2FFB, 0, 2);
    }
    if n(fx) >= 0xB5 {
        ring(fx, sp, D + 0x2710, D + 0x26A0, 0x01E0_4F0C);
        ring(fx, sp, D + 0x2B80, D + 0x2B10, 0x01E0_4F1C);
        let small = f32::from_bits(0x3CF5_C28F);
        if n(fx) >= 0x154 {
            approach_down(fx, D + 0x26A0, small, f32::from_bits(0x3D4C_CCCD));
            fx.mem.write_u32(D + 0x26A4, 0x3F80_0000);
            fx.step_down(D + 0x271B, 0, 6);
            let d = f32::from_bits(0x3ECC_CCCD);
            for o in [0x2B10u32, 0x2B18] {
                let v = ee::add(fx.mem.f(D + o), d);
                fx.mem.setf(D + o, v);
            }
            fstep_down(fx, D + 0x2B14, small, f32::from_bits(0x3DA3_D70A));
            fx.step_down(D + 0x2B8B, 0, 8);
            approach_down(fx, D + 0x15D0, small, f32::from_bits(0x3D4C_CCCD));
            fx.mem.write_u32(D + 0x15D4, 0x3F80_0000);
            fx.step_down(D + 0x164B, 0, 6);
        } else {
            fx.step_up(D + 0x2B8B, 0x80, 1);
            fx.step_up(D + 0x271B, 0x60, 1);
            let s = f32::from_bits(0x3CCC_CCCD);
            fx.approach(D + 0x26A0, 1.0, s);
            fx.approach(D + 0x15D0, 1.0, s);
        }
        fx.sprites(D + 0x1640, D + 0x15D0);
        cloud(fx);
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0x154 {
        2
    } else if m == 0xAB {
        1
    } else if (0xB5..0x154).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}

/// `ring_draw` (0x1E03A60) from the `jal` returning to `ra`, with its
/// save of ra (`sq` at +0x90 of its 0x140 frame: the upper half 0) [F:
/// the sprite cloud's TEX0 slot, never written for its null texture,
/// reads it]; the s-register saves below it are not written [H].
fn ring(fx: &mut Fx, sp: u32, g: u32, x: u32, ra: u32) {
    fx.ring_draw(g, x);
    fx.saved(sp - 0x140, &[(0x90, u64::from(ra))]);
}

/// The cloud's sprites (+0x16A0 + 16 k): rising by 2 / 4 / 6 (k % 3),
/// alpha 0x80; above 35 respawned on a ring of radius +0x168C at -35 -
/// rand % 10 (rand's frame: s0 = D).
fn cloud(fx: &mut Fx) {
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0x169A) {
        let p = D + 0x16A0 + 16 * k as u32;
        if fx.mem.i16(p + 0xE) < 0 {
            k += 1;
            continue;
        }
        let up = match fx.div(k, 3).1 {
            0 => 2.0,
            1 => 4.0,
            _ => 6.0,
        };
        let y = ee::add(fx.mem.f(p + 4), up);
        fx.mem.setf(p + 4, y);
        fx.mem.write_u8(p + 0xC, 0x80);
        if !ee_le(fx.mem.f(p + 4), 35.0) {
            let a = fx.rand_from(D, 0x01E0_5120).wrapping_rem(0x1_0000);
            let c = fx.tcos(a);
            let r = fx.mem.f(D + 0x168C);
            fx.mem.setf(p + 8, ee::mul(r, c));
            let s = fx.tsin(a);
            let r = fx.mem.f(D + 0x168C);
            fx.mem.setf(p, ee::mul(r, s));
            let h = rmod(fx, 0x01E0_5164, 0xA);
            fx.mem.setf(p + 4, ee::sub(ee::neg(ee::from_i32(h)), 35.0));
            fx.mem.write_u8(p + 0xC, 0);
        }
        k += 1;
    }
}
