//! Flat annuli of the effect library: a ring
//! between two radii in the plane y = 0, a quarter of quads computed once
//! and mirrored into the four quadrants.
use super::exe::{camera_vp_into, countdown, libm_cosf, mul_mem, node_source, rot_trans_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

const PI: u32 = 0x4049_0FDB;

/// e2af898f9b56 [b379 (card 2) 0x1E04710, 372 words] [F]: `flat(at, recs,
/// ot, n) -> 1` steps and draws `n` records (0x3C bytes; a 0x380 frame
/// with the record pointer at sp + 0xB0, the counters i / j at sp + 0xC0 /
/// + 0xE0, n at sp + 0xFC, the segments at sp + 0xD0). Per record:
/// countdown 0x101AF0; the radii +0x10 / +0x14 grow by +0x18 / +0x1C;
/// segments = +7 (at most 9), the step 90 / segments degrees; the position
/// +0x20 from a node with +5 (0x101BA0, copied through sp + 0x310); the
/// matrix: the camera (0x20E2B0, sp + 0x140) x the turn +0x2C at the
/// position (0x20C0B0 into sp + 0x100, 0x20B370). The quarter's points
/// (+0x10 cos, -, +0x10 sin) at sp + 0x180 + 16 i and (+0x14 cos, -, +0x14
/// sin) 0xA0 above them for i = 0..=segments (libm sinf / cosf of pi x
/// step i / 180). A gouraud strip descriptor (8 / 0x14, regs 0x41414141)
/// at sp + 0x328: the inner vertices the colour +0xE / +0xD / +0xC / +0xF,
/// the outer +0xA / +9 / +8 / +0xB, Q 1; unless +4 0x40 the alphas x
/// count / 8 over the last 8 (0x207E20 ALPHA 0x48). Then per segment and
/// quadrant q the quad (inner i, inner i + 1, outer i, outer i + 1) at
/// sp + 0x2C0.. with x negated for q = 1 / 2 and z for q = 2 / 3 (each
/// quadrant only rewrites what it changes), all four corners projected
/// (0x204D80 into sp + 0x340.. with the w's at sp + 0x300..) and the quad
/// submitted with the key 0x1A2C70 of the corners' mean w.
pub fn flat(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x380);
    fx.mem.set32(sp + 0xFC, n);
    fx.mem.write_u32(sp + 0xB0, recs);
    fx.mem.set32(sp + 0xC0, 0);
    let mut i = 0;
    while i < n {
        if i != 0 {
            let g = fx.mem.read_u32(sp + 0xB0) + 0x3C;
            fx.mem.write_u32(sp + 0xB0, g);
        }
        let g = fx.mem.read_u32(sp + 0xB0);
        one(fx, at, sp, g, ot);
        i += 1;
        fx.mem.set32(sp + 0xC0, i);
    }
    fx.leave(0x380);
    1
}

fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    for o in [0x10u32, 0x14] {
        let v = ee::add(fx.mem.f(g + o), fx.mem.f(g + o + 8));
        fx.mem.setf(g + o, v);
    }
    let mut segs = fx.mem.u8(g + 7);
    fx.mem.set32(sp + 0xD0, segs);
    if segs >= 10 {
        segs = 9;
        fx.mem.set32(sp + 0xD0, segs);
    }
    let step = ee::div(90.0, ee::from_i32(segs));
    for o in [0u32, 4, 8] {
        let w = fx.mem.read_u32(g + 0x20 + o);
        fx.mem.write_u32(sp + 0x310 + o, w);
    }
    node_source(fx, g, sp + 0x310);
    for o in [0u32, 4, 8] {
        let w = fx.mem.read_u32(sp + 0x310 + o);
        fx.mem.write_u32(g + 0x20 + o, w);
    }
    camera_vp_into(fx, sp + 0x140);
    rot_trans_into(fx, sp + 0x100, g + 0x2C, sp + 0x310);
    mul_mem(fx, sp + 0x140, sp + 0x100);
    let pi = f32::from_bits(PI);
    for k in 0..=segs {
        let ang = ee::div(ee::mul(pi, ee::mul(step, ee::from_i32(k))), 180.0);
        let s = fx.libm_sinf(ang);
        let c = libm_cosf(fx, ang);
        let p = sp + 0x180 + 16 * k as u32;
        let r = fx.mem.f(g + 0x10);
        fx.mem.setf(p, ee::mul(r, c));
        let r = fx.mem.f(g + 0x10);
        fx.mem.setf(p + 8, ee::mul(r, s));
        let r = fx.mem.f(g + 0x14);
        fx.mem.setf(p + 0xA0, ee::mul(r, c));
        let r = fx.mem.f(g + 0x14);
        fx.mem.setf(p + 0xA8, ee::mul(r, s));
    }
    let d = sp + 0x328;
    fx.desc_init(d, 8, 0x14, 0x4141_4141);
    for (o, v) in [(0x348u32, 0xEu32), (0x338, 0xE), (0x368, 0xA), (0x358, 0xA), (0x349, 0xD), (0x339, 0xD), (0x369, 9), (0x359, 9), (0x34A, 0xC), (0x33A, 0xC), (0x36A, 8), (0x35A, 8)] {
        let b = fx.mem.read_u8(g + v);
        fx.mem.write_u8(sp + o, b);
    }
    for o in [0x36Cu32, 0x35C, 0x34C, 0x33C] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    for o in [0x377u32, 0x367, 0x357, 0x347] {
        fx.mem.write_u8(sp + o, 0);
    }
    let b = fx.mem.read_u8(g + 0xF);
    fx.mem.write_u8(sp + 0x34B, b);
    fx.mem.write_u8(sp + 0x33B, b);
    let b = fx.mem.read_u8(g + 0xB);
    fx.mem.write_u8(sp + 0x36B, b);
    fx.mem.write_u8(sp + 0x35B, b);
    if fx.mem.u8(g + 4) & 0x40 == 0 {
        let c = fx.mem.u8(g);
        if c < 8 {
            let a = fx.mem.u8(g + 0xF) >> 3;
            let v = fx.mult(a, c);
            fx.mem.set8(sp + 0x34B, v);
            fx.mem.set8(sp + 0x33B, v);
            // mult1: LO1, not LO.
            let v = (fx.mem.u8(g + 0xB) >> 3).wrapping_mul(c);
            fx.mem.set8(sp + 0x36B, v);
            fx.mem.set8(sp + 0x35B, v);
        }
    }
    fx.desc_alpha(d, 0x48);
    for o in [0x2F4u32, 0x2E4, 0x2D4, 0x2C4] {
        fx.mem.write_u32(sp + o, 0);
    }
    fx.mem.set32(sp + 0xE0, 0);
    if segs <= 0 {
        return;
    }
    let m = fx.read_m4(sp + 0x140);
    let mut j = 0;
    while j < segs {
        let s1 = sp + 0x180 + 16 * j as u32;
        for q in 0..4 {
            let xs = [(0x2C0u32, 0u32), (0x2D0, 0x10), (0x2E0, 0xA0), (0x2F0, 0xB0)];
            let zs = [(0x2C8u32, 8u32), (0x2D8, 0x18), (0x2E8, 0xA8), (0x2F8, 0xB8)];
            let (set, neg): (&[(u32, u32)], bool) = match q {
                0 => (&xs, false),
                1 => (&xs, true),
                2 => (&zs, true),
                _ => (&xs, false),
            };
            for &(o, from) in set {
                let v = fx.mem.read_u32(s1 + from);
                fx.mem.write_u32(sp + o, if neg { v ^ 0x8000_0000 } else { v });
            }
            if q == 0 {
                for &(o, from) in &zs {
                    let v = fx.mem.read_u32(s1 + from);
                    fx.mem.write_u32(sp + o, v);
                }
            }
            let ok = fx.project_at(sp + 0x2C0, &m, sp + 0x340, sp + 0x300)
                && fx.project_at(sp + 0x2D0, &m, sp + 0x350, sp + 0x304)
                && fx.project_at(sp + 0x2E0, &m, sp + 0x360, sp + 0x308)
                && fx.project_at(sp + 0x2F0, &m, sp + 0x370, sp + 0x30C);
            if ok {
                let w = ee::add(fx.mem.f(sp + 0x300), fx.mem.f(sp + 0x304));
                let w = ee::add(fx.mem.f(sp + 0x308), w);
                let w = ee::add(fx.mem.f(sp + 0x30C), w);
                let w = ee::div(w, 4.0);
                fx.fix_residue(w, at + 0x520 + 8);
                fx.submit(d, ot, ee::fixunssfsi(w) as i32);
            }
        }
        j += 1;
        fx.mem.set32(sp + 0xE0, j);
    }
}
