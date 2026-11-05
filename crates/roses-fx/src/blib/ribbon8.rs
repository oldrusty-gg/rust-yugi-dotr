//! The two-history ribbon of cards 236 / 395 / 397, lifted from E3's b154 (card 236) record-local port into the
//! library [F]: 59e4f30e51ec draws it (`ribbon`), 04990d089783 colours a
//! strip (`strip_colours`).
//!
//! A ribbon `g`: +4 flags (bits 0x14..0x17 the corners projected, cleared
//! after each strip), +8 the colour (B, G, R, A), +0x10 + 16 k / +0x90 +
//! 16 k two eight-point histories (newest first).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx, OT};

/// 04990d089783 [3 B records; b049 / card 397 0x1E00100, 52 words; a leaf
/// before the first prologue] [F]: `strip_colours(d, c, k)`: a ribbon
/// strip's colours: (c+2, c+1, c+0) at all four vertices, alpha c+3 - k
/// (c+3 >> 3) (0 for k = 0), c+3 - (k + 1) (c+3 >> 3) (0 for k = 6), 0, 0;
/// Q 1.
pub fn strip_colours(fx: &mut Fx, d: u32, c: u32, k: i32) {
    let a = i32::from(fx.mem.read_u8(c + 3));
    let step = a >> 3;
    for v in 0..4u32 {
        let at = d + 0x10 + 0x10 * v;
        for (o, from) in [(0u32, 2u32), (1, 1), (2, 0)] {
            let b = fx.mem.read_u8(c + from);
            fx.mem.write_u8(at + o, b);
        }
        // The `mult` leaves LO / HI for a later division by zero [run b154].
        let al = match v {
            0 if k != 0 => a - fx.mult(k, step),
            1 if k != 6 => a - (step + fx.mult(k, step)),
            _ => 0,
        };
        fx.mem.write_u8(at + 3, al as u8);
        fx.mem.write_u32(at + 4, 0x3F80_0000);
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 59e4f30e51ec [3 B records; b049 / card 397 0x1E02690, 296 words] [F]:
/// `ribbon(g, x, a, b)` (frame 0x140): both histories shift up one, point 0
/// = `a` / `b` (xyz); in a packet layer, for each of the 7 segments two
/// untextured gouraud strips (mid k, mid k + 1, A k, A k + 1) and (mid k,
/// mid k + 1, B k, B k + 1), mid the histories' midpoint (0x1036E0),
/// coloured by [`strip_colours`], the corners through `x` + 0x30
/// (0x204D80, w at sp + 0xD0..0xDC, corner bits 0x14..0x17 of +4), drawn
/// with no ordering table keyed by the mean w when all four bits are set;
/// the corner bits cleared. Returns 0.
pub fn ribbon(fx: &mut Fx, g: u32, x: u32, a: u32, b: u32) -> u32 {
    let sp = fx.enter(0x140);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xAC, layer);
    let mut k = 6i32;
    while k >= 0 {
        let v = g + 0x60 - 0x10 * (6 - k) as u32;
        for o in [0x10u32, 0x90] {
            copy4(fx, v + o + 0x10, v + o);
        }
        if k == 0 {
            for c in 0..3 {
                let w = fx.mem.read_u32(a + 4 * c);
                fx.mem.write_u32(v + 0x10 + 4 * c, w);
                let w = fx.mem.read_u32(b + 4 * c);
                fx.mem.write_u32(v + 0x90 + 4 * c, w);
            }
        }
        k -= 1;
    }
    let d = sp + 0xE8;
    fx.desc_init(d, 8, 0x14, 0x4141_4141);
    fx.desc_alpha(d, 0x80_0000_0048);
    for k in 0..7u32 {
        let (p0, p1) = (g + 0x10 * k, g + 0x10 * (k + 1));
        fx.midpoint(p0 + 0x10, p0 + 0x90, sp + 0xB0);
        fx.midpoint(p1 + 0x10, p1 + 0x90, sp + 0xC0);
        for side in [0x10u32, 0x90] {
            strip_colours(fx, d, g + 8, k as i32);
            let corners = [(sp + 0xB0, sp + 0x100, sp + 0xD0), (sp + 0xC0, sp + 0x110, sp + 0xD4), (p0 + side, sp + 0x120, sp + 0xD8), (p1 + side, sp + 0x130, sp + 0xDC)];
            for (bitn, (v, xyz, w)) in (0x14u32..).zip(corners) {
                let m = fx.read_m4(x + 0x30);
                if fx.project_at(v, &m, xyz, w) {
                    fx.flag(g + 4, bitn);
                }
            }
            let flags = fx.mem.read_u32(g + 4);
            if (0x14..0x18).all(|n| bit(flags, n)) {
                let s = ee::add(fx.mem.f(sp + 0xD0), fx.mem.f(sp + 0xD4));
                let s = ee::add(fx.mem.f(sp + 0xD8), s);
                let s = ee::add(fx.mem.f(sp + 0xDC), s);
                let key = ee::fixunssfsi(ee::div(s, 4.0)) as i32;
                fx.submit(d, 0, key);
            }
            let f = fx.mem.read_u32(g + 4) & 0xFF0F_FFFF;
            fx.mem.write_u32(g + 4, f);
        }
    }
    let layer = fx.mem.read_u32(sp + 0xAC);
    fx.layer_end(layer, OT, 0);
    fx.leave(0x140);
    0
}
