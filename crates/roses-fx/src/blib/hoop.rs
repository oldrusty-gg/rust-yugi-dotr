//! Spinning hoops, hand-ported from b613 (card
//! 425) [F]: 039c490f1d7b lays out a half circle of points and gives each
//! hoop a random turn (`setup`), 54355bee1fed draws every hoop as a chain
//! of flat-coloured lines growing point by point (`draw`).
//!
//! A hoop group `g`: +4 flags (0x1A: hold when grown, every segment), +8
//! the colour (B, G, R, A), +0xC the radius, +0x20 the hoops (16 bytes:
//! turn x / y / z i32, alpha +0xC, age i16 +0xE), +0x24 the points (16
//! bytes), +0x28 the point count, +0x2A the hoop count, +0x30 the start
//! delay range, +0x32 the growth an update.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};

/// RGBAQ / XYZ2 / XYZ2 (a flat line).
const REGS_LINE: u64 = 0x441;

/// 039c490f1d7b [2 B records; b613 0x1E02E50, 100 words] [F]: `setup(g)
/// -> 0`: point k of +0x28 at the 16-bit angle k (0x8000 / count) on the
/// radius (x = r sin, y = r cos; z kept); each hoop turned by three
/// random 16-bit angles, its age -(rand % +0x30), its alpha +0xB. A 0x60
/// frame.
pub fn setup(fx: &mut Fx, at: u32, g: u32) -> i32 {
    fx.enter(0x60);
    let n = fx.mem.i16(g + 0x28);
    let (step, _) = fx.div(0x8000, n);
    let mut k = 0i32;
    let mut a = 0i32;
    while k < fx.mem.i16(g + 0x28) {
        let p = fx.mem.read_u32(g + 0x24) + 16 * k as u32;
        let c = fx.tcos(a);
        let v = ee::mul(fx.mem.f(g + 0xC), c);
        fx.mem.setf(p + 4, v);
        let s = fx.tsin(a);
        let v = ee::mul(fx.mem.f(g + 0xC), s);
        let p = fx.mem.read_u32(g + 0x24) + 16 * k as u32;
        fx.mem.setf(p, v);
        a = a.wrapping_add(step);
        k += 1;
    }
    let mut c = 0i32;
    while c < fx.mem.i16(g + 0x2A) {
        let off = 16 * c as u32;
        for (o, ra) in [(0u32, 0xB4u32), (4, 0xD8), (8, 0xFC)] {
            let r = fx.rand_from(off, at + ra) & 0xFFFF;
            let p = fx.mem.read_u32(g + 0x20) + off;
            fx.mem.set32(p + o, r);
        }
        let r = fx.rand_from(off, at + 0x120);
        let m = fx.mem.i16(g + 0x30);
        let (_, h) = fx.div(r, m);
        let p = fx.mem.read_u32(g + 0x20) + off;
        fx.mem.set16(p + 0xE, h.wrapping_neg());
        let al = fx.mem.read_u8(g + 0xB);
        let p = fx.mem.read_u32(g + 0x20) + off;
        fx.mem.write_u8(p + 0xC, al);
        c += 1;
    }
    fx.leave(0x60);
    0
}

/// 54355bee1fed [2 B records; b613 0x1E02FE0, 264 words] [F]: `draw(ot,
/// g, x) -> 0` (descriptor at sp + 0x290: a flat line, blend 0x48, the
/// group's colour, flag 0x400): every hoop past its delay (a negative age
/// counts up) turned by its angles (0x20BDA0 into sp + 0xC0) through `x`
/// + 0x30 (0x20B460); its points projected (0x204D80: XYZ at sp + 0x110 +
/// 8 k, w at sp + 0x210 + 4 k); segment k (k <= age, every one with flag
/// 0x1A) drawn when both ends are projected, keyed by their mean w; the
/// age + the growth, at the count held (0x1A) or restarted at 0 with new
/// random angles. The projected-point bits carry over a waiting hoop. A
/// 0x2C0 frame.
pub fn draw(fx: &mut Fx, at: u32, ot: u32, g: u32, x: u32) -> i32 {
    let sp = fx.enter(0x2C0);
    fx.mem.write_u32(sp + 0xB8, ot);
    fx.mem.write_u32(sp + 0xB0, x);
    let d = sp + 0x290;
    fx.desc_init(d, 3, 1, REGS_LINE);
    fx.desc_alpha(d, 0x80_0000_0048);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x400);
    for (k, o) in [0xAu32, 9, 8, 0xB].into_iter().enumerate() {
        let v = fx.mem.read_u8(g + o);
        fx.mem.write_u8(sp + 0x2A0 + k as u32, v);
    }
    fx.mem.write_u32(sp + 0x2A4, 0x3F80_0000);
    let (xyz, ws) = (sp + 0x110, sp + 0x210);
    let mut mask = 0u32;
    let mut c = 0i32;
    while c < fx.mem.i16(g + 0x2A) {
        fx.mem.set32(sp + 0xA0, c);
        let rec = |fx: &mut Fx| fx.mem.read_u32(g + 0x20) + 16 * c as u32;
        let p = rec(fx);
        if fx.mem.i16(p + 0xE) < 0 {
            let a = fx.mem.i16(p + 0xE) + 1;
            fx.mem.set16(p + 0xE, a);
            c += 1;
            continue;
        }
        for k in 0..3u32 {
            let p = rec(fx);
            let w = fx.mem.read_u32(p + 4 * k);
            fx.mem.write_u32(sp + 0x100 + 4 * k, w);
        }
        let rot = [fx.mem.i32(sp + 0x100), fx.mem.i32(sp + 0x104), fx.mem.i32(sp + 0x108)];
        let m = fx.rotation_matrix(rot);
        fx.write_m4(sp + 0xC0, &m);
        super::blade::mul_after(fx, sp + 0xC0, x + 0x30);
        let mut k = 0i32;
        while k < fx.mem.i16(g + 0x28) {
            let v = fx.mem.read_u32(g + 0x24) + 16 * k as u32;
            let m = fx.read_m4(sp + 0xC0);
            if fx.project_at(v, &m, xyz + 8 * k as u32, ws + 4 * k as u32) {
                mask |= 1 << (k & 31);
            }
            k += 1;
        }
        let mut k = 0i32;
        while k < fx.mem.i16(g + 0x28) - 1 {
            let every = bit(fx.mem.read_u32(g + 4), 0x1A);
            let p = rec(fx);
            if (every || fx.mem.i16(p + 0xE) >= k) && bit(mask, k as u32 & 31) && bit(mask, (k as u32 + 1) & 31) {
                let (a, b) = (xyz + 8 * k as u32, xyz + 8 * (k + 1) as u32);
                let (v0, v1) = (sp + 0x2A8, sp + 0x2B0);
                let (ax, ay, bx, by) = (fx.mem.read_u16(a), fx.mem.read_u16(a + 2), fx.mem.read_u16(b), fx.mem.read_u16(b + 2));
                fx.mem.write_u16(v0, ax);
                fx.mem.write_u16(v1, bx);
                fx.mem.write_u16(v0 + 2, ay);
                fx.mem.write_u16(v1 + 2, by);
                let z = fx.xyz_z(a);
                fx.set_xyz_z(v0, z);
                let z = fx.xyz_z(b);
                fx.set_xyz_z(v1, z);
                let (w0, w1) = (ws + 4 * k as u32, ws + 4 * (k + 1) as u32);
                let key = ee::fixunssfsi(ee::div(ee::add(fx.mem.f(w0), fx.mem.f(w1)), 2.0)) as i32;
                fx.submit(d, ot, key);
            }
            k += 1;
        }
        let p = rec(fx);
        let a = fx.mem.i16(p + 0xE) + fx.mem.i16(g + 0x32);
        fx.mem.set16(p + 0xE, a);
        let p = rec(fx);
        if fx.mem.i16(p + 0xE) >= fx.mem.i16(g + 0x28) {
            if bit(fx.mem.read_u32(g + 4), 0x1A) {
                let n = fx.mem.i16(g + 0x28);
                let p = rec(fx);
                fx.mem.set16(p + 0xE, n);
            } else {
                let p = rec(fx);
                fx.mem.set16(p + 0xE, 0);
                for (o, ra) in [(0u32, 0x360u32), (4, 0x384), (8, 0x3A8)] {
                    let r = fx.rand_from(0, at + ra) & 0xFFFF;
                    let p = rec(fx);
                    fx.mem.set32(p + o, r);
                }
            }
        }
        mask = 0;
        c += 1;
    }
    fx.mem.set32(sp + 0xA0, c);
    fx.leave(0x2C0);
    0
}
