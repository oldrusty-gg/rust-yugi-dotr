//! BMODULE b057 (card 297), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01BD0, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): a column of sparks (+0xE0, `blib::trail::sparks`) rising
//! round the attacker (each respawned at the foot when above 200, colour
//! re-rolled every update), from update 20 a particle-quad burst
//! (+0x11F0) at the target growing 1 % per update, and on updates 42..108
//! the target's added colour (0x1F7790) flickering at random (cleared on
//! update 109). Returns 1 on update 31, 6 on 41..108, 2 on 109, 8 after
//! 125.
//!
//! Library: `colours4` (0x1E00080), `spawn::scatter` (0x1E00120),
//! `trail::sparks` (0x1E002E0) [`crate::blib`], `particle_quads`
//! (0x1E01040) [`crate::fxlib`].
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const TENTH: u32 = 0x3DCC_CCCD;

/// The entry 0x1E01BD0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

/// Spark k (record *(+0xEC) + 16 k) set at a random angle, radius (rand %
/// 60) / 10, height (rand % `high`) / 10 [F 0x1E01D34 / 0x1E0221C]; the
/// three rand calls return to `ras` (their frames' residue, s0 = D).
pub(crate) fn spark_place(fx: &mut Fx, k: u32, high: i32, ras: [u32; 3]) {
    spark_place_r(fx, k, 0x3C, high, ras);
}

/// [`spark_place`] with the radius rand % `wide` / 10 (b545: 200).
pub(crate) fn spark_place_r(fx: &mut Fx, k: u32, wide: i32, high: i32, ras: [u32; 3]) {
    let a = fx.rand_from(D, ras[0]) & 0xFFFF;
    let r = fx.rand_from(D, ras[1]);
    let (_, h) = fx.div(r, wide);
    let radius = ee::mul(f32::from_bits(TENTH), ee::from_i32(h));
    let r = fx.rand_from(D, ras[2]);
    let (_, h) = fx.div(r, high);
    let y = ee::mul(f32::from_bits(TENTH), ee::from_i32(h));
    let e = fx.mem.read_u32(D + 0xEC).wrapping_add(16 * k);
    fx.mem.setf(e + 4, y);
    let c = fx.tcos(a);
    let e = fx.mem.read_u32(D + 0xEC).wrapping_add(16 * k);
    fx.mem.setf(e + 8, ee::mul(radius, c));
    let s = fx.tsin(a);
    let e = fx.mem.read_u32(D + 0xEC).wrapping_add(16 * k);
    fx.mem.setf(e, ee::mul(radius, s));
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1360);
    for x in [0u32, 0x1110, 0x70, 0x1180] {
        fx.xform_init(D + x);
    }
    fx.mem.set32(D + 0x1350, 0x7D);
    fx.mem.set32(D + 0x1354, 1);
    fx.mem.set32(D + 0x1358, param);
    for k in [0x135Fu32, 0x135C, 0x135D, 0x135E] {
        fx.mem.write_u8(D + k, 0x80);
    }
    fx.texture4(sp + 0xB0, 0xF);
    // The sparks (+0xE0): 256 at +0x110, width 0.1, length 3.
    let g = D + 0xE0;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x6000_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0x20, TENTH);
    fx.mem.write_u32(g + 0x24, 0x4040_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x60), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x60), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, D + 0x110);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x10C) {
        spark_place(fx, k as u32, 0x7D0, [0x01E0_1D3C, 0x01E0_1D5C, 0x01E0_1D98]);
        k += 1;
    }
    // The particle quads (+0x11F0): 16 round +0x1200, size 20.
    let g = D + 0x11F0;
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x85);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, D, &[(0x1218, 0xF0), (0x121A, 0), (0x1214, 0x10), (0x1216, 0x10), (0x121C, 0x100), (0x121E, 0x100)]);
    fx.mem.write_u32(D + 0x11FC, 0x41A0_0000);
    halves(fx, D, &[(0x1320, 0x10), (0x132A, 1), (0x132C, 1), (0x1330, 8), (0x1326, 0xA), (0x1328, 0xA)]);
    for k in [0x11FAu32, 0x11F9, 0x11F8] {
        fx.mem.write_u8(D + k, 0x80);
    }
    fx.mem.write_u8(D + 0x11FB, 0x20);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x1320) {
        let p = D + 16 * k as u32;
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x14);
        let x = ee::add(fx.mem.f(D + 0x1200), ee::from_i32(h - 0xA));
        fx.mem.setf(p + 0x1220, x);
        let r = fx.rand();
        let (_, h) = fx.div(r, 0x14);
        let y = ee::add(fx.mem.f(D + 0x1204), ee::from_i32(h - 0xA));
        fx.mem.setf(p + 0x1224, y);
        let z = fx.mem.read_u32(D + 0x1208);
        fx.mem.write_u32(p + 0x1228, z);
        let n = fx.mem.i16(D + 0x132A).wrapping_neg();
        let m = fx.mult(n, k);
        fx.mem.set16(p + 0x122E, m);
        fx.mem.set16(p + 0x122C, 0);
        let n = fx.mem.i16(D + 0x1330) - 1;
        fx.mem.set16(D + 0x132E, n);
        k += 1;
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x1354);
    let entered_at = n(fx);
    if entered_at == 0x14 {
        let t = fx.target_handle();
        fx.unit_position(t, D + 0x1340);
        let h = fx.self_handle();
        fx.unit_point(1, h, D + 0x11A0);
        let h = fx.self_handle();
        fx.unit_point(1, h, D + 0x90);
        let t = fx.target_handle();
        fx.unit_position(t, sp + 0x90);
        let h = fx.self_handle();
        fx.unit_point(1, h, sp + 0xA0);
        fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x80);
    }
    // s0 = D, s1 = the OT, s2 = sp + 0xA0 after the set-up update, else
    // the parameter (-1, sign-extended: not modelled), s3 the caller's.
    let s2 = (entered_at == 0x14).then_some(sp + 0xA0);
    let regs = [Some(D), Some(OT), s2, None];
    fx.xform_world_saving(D, 0x01E0_203C, regs);
    fx.xform_billboard_saving(D + 0x1110, 0x01E0_2048, regs);
    fx.xform_world_saving(D + 0x70, 0x01E0_2054, regs);
    fx.xform_billboard_saving(D + 0x1180, 0x01E0_2060, regs);
    if (0x2A..0x6D).contains(&n(fx)) {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, D + 0x135C, [D, OT], 0x01E0_2094);
        for (at, ra) in [(0x135Eu32, 0x01E0_209C), (0x135D, 0x01E0_20C0), (0x135C, 0x01E0_20E4)] {
            let r = fx.rand_from(D, ra);
            fx.mem.set8(D + at, r % 128);
        }
    }
    if n(fx) == 0x6D {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, 0, [D, OT], 0x01E0_2128);
    }
    let r = fx.rand_from(D, 0x01E0_2130);
    fx.mem.set8(D + 0xEA, r % 128);
    for (at, m, ra) in [(0xE9u32, 0x70, 0x01E0_2154), (0xE8, 0xA0, 0x01E0_2174)] {
        let r = fx.rand_from(D, ra);
        let (_, h) = fx.div(r, m);
        fx.mem.set8(D + at, h);
    }
    blib::trail::sparks(fx, OT, D + 0xE0, D + 0x70);
    if n(fx) >= 0x6E {
        fx.step_down(D + 0xEB, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x10C) {
        let p = D + 16 * k as u32;
        if fx.mem.i16(p + 0x11E) >= 0 {
            let y = ee::add(fx.mem.f(p + 0x114), 4.0);
            fx.mem.setf(p + 0x114, y);
            if !ee_le(y, 200.0) {
                spark_place(fx, k as u32, 0x1E, [0x01E0_2224, 0x01E0_2244, 0x01E0_2280]);
                fx.mem.set16(p + 0x11E, 0);
            }
        }
        k += 1;
    }
    if n(fx) >= 0x14 {
        for c in [8u32, 4, 0] {
            fx.mem.write_u32(sp + 0x80 + c, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0x1180, D + 0x11F0, sp + 0x80);
        fx.step_down(D + 0x11FB, 0, 1);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x1354, m);
    let left = fx.mem.i32(D + 0x1350) - 1;
    fx.mem.set32(D + 0x1350, left);
    if m == 0x6D {
        2
    } else if m == 0x1F {
        1
    } else if (0x29..0x6D).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
