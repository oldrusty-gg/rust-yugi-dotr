//! Spokes of the effect library: gouraud lines
//! radiating from a turned, moving centre, their directions read from a
//! byte table.
use super::exe::{camera_vp_into, countdown, libm_cosf, mul_mem, node_source, rot_trans_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx};

const PI: u32 = 0x4049_0FDB;

/// 0ffb907b5b35 [2 B records; b133 0x1E014E0, b176 0x1E03A20, 328 words]
/// [F]: `lines(at, recs, ot, n) -> 1` steps and draws `n` records (0x60
/// bytes; a 0x190 frame, the descriptor at sp + 0x150: RGBAQ / XYZ2 x 2,
/// prim 0x11, ALPHA 0x48 (0x207E20), flags | 0x400). Per record: countdown
/// 0x101AF0; position +0x20 += +0x30 (floats), turn +0x40 += +0x50 (i32
/// angles), then from a node with +5 (0x101BA0); camera (0x20E2B0 into sp
/// + 0xF0) x the turn at the position (0x20C0B0 into sp + 0xB0). The table
/// +0xC read from cursor +9: its first byte + 20 the angle step; colours
/// +0x1E.. (inner) / +0x1A.. (outer), alpha +0x1B (x count / 8 over the
/// last 8), Q 1.0. +8 lines k: the angle steps (wrapped above 360), the
/// direction (r cos, h, r sin) with r = +0x14 x b0 / 255, h = +0x10 x b1 /
/// 255 (libm 0x1A7158 / 0x1A7348); inner point that x elapsed (+2 - +0),
/// outer one that + direction x (1 + (b2 & +0xA)); cursor += 3. Both must
/// project (0x204D80, w at sp + 0x18C); then submitted twice keyed by the
/// outer w (0x1A2C70 at +0x464 / +0x484).
pub fn lines(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x190);
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x60;
        }
        one(fx, at, sp, g, ot);
        i += 1;
    }
    fx.leave(0x190);
    1
}

/// u8 at `tbl + (k & 0xFF)`.
fn tb(fx: &mut Fx, g: u32, k: u32) -> i32 {
    let tbl = fx.mem.read_u32(g + 0xC);
    fx.mem.u8(tbl.wrapping_add(k & 0xFF))
}

fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    for o in [0x20u32, 0x24, 0x28] {
        let v = ee::add(fx.mem.f(g + o), fx.mem.f(g + o + 0x10));
        fx.mem.setf(g + o, v);
    }
    for o in [0x40u32, 0x44, 0x48] {
        let v = fx.mem.i32(g + o).wrapping_add(fx.mem.i32(g + o + 0x10));
        fx.mem.set32(g + o, v);
    }
    node_source(fx, g, g + 0x20);
    camera_vp_into(fx, sp + 0xF0);
    rot_trans_into(fx, sp + 0xB0, g + 0x40, g + 0x20);
    mul_mem(fx, sp + 0xF0, sp + 0xB0);
    let mut cur = fx.mem.u8(g + 9) as u32;
    let step = ee::add(20.0, ee::from_i32(tb(fx, g, cur)));
    cur = (cur + 1) & 0xFF;
    let mut a = 0.0f32;
    let e = fx.mem.u8(g + 2) - fx.mem.u8(g);
    let d = sp + 0x150;
    fx.desc_init(d, 4, 0x11, 0x4141);
    for (k, src) in [0x1Eu32, 0x1D, 0x1C].into_iter().enumerate() {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + 0x160 + k as u32, v);
    }
    for (k, src) in [0x1Au32, 0x19, 0x18].into_iter().enumerate() {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + 0x170 + k as u32, v);
    }
    fx.mem.write_u32(sp + 0x174, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x164, 0x3F80_0000);
    let c = fx.mem.u8(g);
    let alpha = if c < 8 {
        let v = fx.mem.u8(g + 0x1B) >> 3;
        fx.mult(v, c)
    } else {
        fx.mem.u8(g + 0x1B)
    };
    fx.mem.set8(sp + 0x173, alpha);
    fx.mem.set8(sp + 0x163, alpha);
    fx.desc_alpha(d, 0x48);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x400);
    let mut k = 0;
    while k < fx.mem.u8(g + 8) {
        a = ee::add(a, step);
        if !ee_le(a, 360.0) {
            a = ee::sub(a, 360.0);
        }
        let r = ee::div(ee::mul(fx.mem.f(g + 0x14), ee::from_i32(tb(fx, g, cur))), 255.0);
        let rad = ee::div(ee::mul(f32::from_bits(PI), a), 180.0);
        let cs = libm_cosf(fx, rad);
        let x = ee::mul(r, cs);
        let sn = fx.libm_sinf(rad);
        let z = ee::mul(r, sn);
        let y = ee::div(ee::mul(fx.mem.f(g + 0x10), ee::from_i32(tb(fx, g, cur + 1))), 255.0);
        let fe = ee::from_i32(e);
        let (ax, ay, az) = (ee::mul(x, fe), ee::mul(y, fe), ee::mul(z, fe));
        fx.mem.setf(sp + 0x130, ax);
        fx.mem.setf(sp + 0x134, ay);
        fx.mem.setf(sp + 0x138, az);
        let b2 = tb(fx, g, cur + 2) & fx.mem.u8(g + 0xA);
        let len = ee::add(1.0, ee::from_i32(b2));
        cur = (cur + 3) & 0xFF;
        fx.mem.setf(sp + 0x140, ee::add(ax, ee::mul(x, len)));
        fx.mem.setf(sp + 0x144, ee::add(ay, ee::mul(y, len)));
        fx.mem.setf(sp + 0x148, ee::add(az, ee::mul(z, len)));
        let m = fx.read_m4(sp + 0xF0);
        if fx.project_at(sp + 0x130, &m, sp + 0x168, sp + 0x18C) && fx.project_at(sp + 0x140, &m, sp + 0x178, sp + 0x18C) {
            for off in [0x46Cu32, 0x48C] {
                let w = fx.mem.f(sp + 0x18C);
                fx.fix_residue(w, at + off);
                let key = ee::fixunssfsi(w) as i32;
                fx.submit(d, ot, key);
            }
        }
        k += 1;
    }
}
