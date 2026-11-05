//! Flat wedge fans of the effect library: a
//! pie slice of one-degree-step triangles about a projected centre, swept
//! from a start angle to an end angle that move towards each other.
use super::exe::{camera_vp_into, countdown, libm_cosf, mul_mem, node_source, rot_trans_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx};

const PI: u32 = 0x4049_0FDB;

/// be74bb4196f6 [b351 (card 187) 0x1E029A0, 288 words] [F]: `fans(at,
/// recs, ot, n) -> 1` steps and draws `n` records (0x5C bytes; a 0x1B0
/// frame with n at sp + 0xBC, a 6-register fan descriptor at sp + 0x158
/// (prim 0x13, RGBAQ / XYZ2 x 3, ALPHA 0x48 by 0x207E20), the local point
/// at sp + 0x140 and the three w's at sp + 0x1A0). Per record: countdown
/// 0x101AF0; the angles +0xC / +0x10 both move by +0x18 (down when +0x10
/// <= +0xC, else up); the position +0x2C += the velocity +0x3C; the
/// radius +0x1C += +0x20 while the count is above +9; the position from a
/// node with +5 (0x101BA0); the matrix: the camera (0x20E2B0) x the turn
/// +0x4C at the position (0x20C0B0, 0x20B370). Vertex 0 (the centre) takes
/// the colour +0x24.. with the alpha +0x27 (x count / 8 over the last 8),
/// vertices 1 / 2 the colour +0x28.. / +0x2B; Q 1. The centre must project
/// (0x204D80 into vertex 0, else nothing is drawn). Then from the lower
/// angle a to the higher b in steps of +8 x +0x14 degrees: the points at
/// (+0x1C cos, 0, +0x1C sin) of a and of a + +0x14 (libm cosf / sinf of
/// pi x / 180) projected into vertices 1 / 2 (either failing skips the
/// step) and the fan submitted with the key 0x1A2C70 of vertex 2's w.
pub fn fans(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1B0);
    fx.mem.set32(sp + 0xBC, n);
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x5C;
        }
        one(fx, at, sp, g, ot);
        i += 1;
    }
    fx.leave(0x1B0);
    1
}

fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let down = ee_le(fx.mem.f(g + 0x10), fx.mem.f(g + 0xC));
    for o in [0xCu32, 0x10] {
        let step = fx.mem.f(g + 0x18);
        let a = fx.mem.f(g + o);
        let v = if down { ee::sub(a, step) } else { ee::add(a, step) };
        fx.mem.setf(g + o, v);
    }
    for o in [0x2Cu32, 0x30, 0x34] {
        let v = ee::add(fx.mem.f(g + o), fx.mem.f(g + o + 0x10));
        fx.mem.setf(g + o, v);
    }
    if fx.mem.u8(g + 9) < fx.mem.u8(g) {
        let v = ee::add(fx.mem.f(g + 0x1C), fx.mem.f(g + 0x20));
        fx.mem.setf(g + 0x1C, v);
    }
    node_source(fx, g, g + 0x2C);
    camera_vp_into(fx, sp + 0x100);
    rot_trans_into(fx, sp + 0xC0, g + 0x4C, g + 0x2C);
    mul_mem(fx, sp + 0x100, sp + 0xC0);
    let fan = sp + 0x158;
    fx.desc_init(fan, 6, 0x13, 0x41_4141_4141);
    for (o, v) in [(0x168u32, 0x26u32), (0x188, 0x2A), (0x178, 0x2A), (0x169, 0x25), (0x189, 0x29), (0x179, 0x29), (0x16A, 0x24), (0x18A, 0x28), (0x17A, 0x28)] {
        let b = fx.mem.read_u8(g + v);
        fx.mem.write_u8(sp + o, b);
    }
    for o in [0x18Cu32, 0x17C, 0x16C] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let b = fx.mem.read_u8(g + 0x2B);
    fx.mem.write_u8(sp + 0x18B, b);
    fx.mem.write_u8(sp + 0x17B, b);
    let c = fx.mem.u8(g);
    if c < 8 {
        let a = fx.mem.u8(g + 0x27) >> 3;
        let a = fx.mult(c, a);
        fx.mem.set8(sp + 0x16B, a);
    } else {
        let a = fx.mem.read_u8(g + 0x27);
        fx.mem.write_u8(sp + 0x16B, a);
    }
    fx.desc_alpha(fan, 0x48);
    fx.mem.set32(sp + 0x148, 0);
    fx.mem.set32(sp + 0x144, 0);
    fx.mem.set32(sp + 0x140, 0);
    let m = fx.read_m4(sp + 0x100);
    if !fx.project_at(sp + 0x140, &m, sp + 0x170, sp + 0x1A0) {
        return;
    }
    let (mut a, b) = if down { (fx.mem.f(g + 0x10), fx.mem.f(g + 0xC)) } else { (fx.mem.f(g + 0xC), fx.mem.f(g + 0x10)) };
    let step = ee::from_i32(i32::from(fx.mem.read_u8(g + 8)));
    let f22 = ee::mul(fx.mem.f(g + 0x14), step);
    if !ee_lt(a, b) {
        return;
    }
    let pi = f32::from_bits(PI);
    loop {
        let ang = ee::div(ee::mul(pi, a), 180.0);
        let r = fx.mem.f(g + 0x1C);
        let c = libm_cosf(fx, ang);
        fx.mem.setf(sp + 0x140, ee::mul(r, c));
        let r = fx.mem.f(g + 0x1C);
        let s = fx.libm_sinf(ang);
        fx.mem.setf(sp + 0x148, ee::mul(r, s));
        if fx.project_at(sp + 0x140, &m, sp + 0x180, sp + 0x1A4) {
            let a2 = ee::add(a, fx.mem.f(g + 0x14));
            let ang = ee::div(ee::mul(pi, a2), 180.0);
            let r = fx.mem.f(g + 0x1C);
            let c = libm_cosf(fx, ang);
            fx.mem.setf(sp + 0x140, ee::mul(r, c));
            let a2 = ee::add(a, fx.mem.f(g + 0x14));
            let ang = ee::div(ee::mul(pi, a2), 180.0);
            let r = fx.mem.f(g + 0x1C);
            let s = fx.libm_sinf(ang);
            fx.mem.setf(sp + 0x148, ee::mul(r, s));
            if fx.project_at(sp + 0x140, &m, sp + 0x190, sp + 0x1A8) {
                let w = fx.mem.f(sp + 0x1A8);
                fx.fix_residue(w, at + 0x3EC + 8);
                fx.submit(fan, ot, ee::fixunssfsi(w) as i32);
            }
        }
        a = ee::add(a, f22);
        if !ee_lt(a, b) {
            break;
        }
    }
}
