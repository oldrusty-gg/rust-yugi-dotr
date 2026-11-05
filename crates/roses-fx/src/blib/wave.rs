//! Squares on a wave, hand-ported from b036
//! (card 23) [F]: 8f78f453e062 places camera-plane squares by a table of
//! signed bytes, the offsets growing with the group's age (`draw`).
//!
//! A wave group `g`: +0 the texture, +4 flags (0xA..0xC blend, 0x13 far
//! Z, 0x14..0x17 corner k projected, 0x18 / 0x19 / 0x1A |x| / |y| / |z|,
//! 0x1B / 0x1C / 0x1D x / y / z 0), +8 the colour (B, G, R, A), +0xC the
//! table (i8), +0x10 / +0x12 / +0x14 / +0x16 the cell (u, v, w, h), +0x20
//! the half size, +0x24 the scale, +0x28 the square count, +0x2A the age,
//! +0x2C the table start (u8).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};

/// RGBAQ then UV / XYZ2 x 4 (a textured strip, one colour).
const REGS: u64 = 0x4_2424_2421;

/// One offset of square k: 0 (`zero`), else age x (scale x |t| / 10) x t
/// (|t| with `abs`), t the table byte at (start + k + `axis`) & 0xFF.
fn offset(fx: &mut Fx, g: u32, i: u32, scale: f32, zero: u32, abs: u32) -> f32 {
    let flags = fx.mem.read_u32(g + 4);
    if bit(flags, zero) {
        return 0.0;
    }
    let tab = fx.mem.read_u32(g + 0xC);
    let t = fx.mem.read_u8(tab + (i & 0xFF)) as i8 as i32;
    let t = if bit(flags, abs) { t.abs() } else { t };
    let age = ee::from_i32(fx.mem.i16(g + 0x2A));
    ee::mul(age, ee::mul(scale, ee::from_i32(t)))
}

/// 8f78f453e062 [15 B records; b036 0x1E02AC0, 512 words] [F]: `draw(ot,
/// g, x) -> 0` (descriptor at sp + 0xD8: one RGBAQ (the group's colour,
/// Q 1), the cell's four UVs, ALPHA 0x44 / 0x88 / 0x42 / 0x48 by flags
/// 0xC / 0xB / 0xA): with an age above 0, square k at (x, y, z) from the
/// table bytes k, k + 1, k + 2 from +0x2C (wrapping at 256), scale =
/// +0x24 |byte k| / 10 (byte k from the table's start; 0x195930 abs); corners (x -+ h, y +- h, z) projected through `x` +
/// 0x30 (0x204D80, w at sp + 0xC0..0xCC), the texture (filter 0), keyed
/// far (0x13, key 0, drawn even when not projected) or, with all four
/// projected, by the mean w; corner bits cleared. Then the age + 1. A
/// 0x150 frame.
pub fn draw(fx: &mut Fx, ot: u32, g: u32, x: u32) -> i32 {
    let sp = fx.enter(0x150);
    if fx.mem.i16(g + 0x2A) > 0 {
        let d = sp + 0xD8;
        fx.desc_init(d, 9, 0x24, REGS);
        let flags = fx.mem.read_u32(g + 4);
        let alpha = if bit(flags, 0xC) {
            0x44
        } else if bit(flags, 0xB) {
            0x88
        } else if bit(flags, 0xA) {
            0x42
        } else {
            0x48
        };
        fx.desc_alpha(d, 0x80_0000_0000 | alpha);
        for (k, o) in [0xAu32, 9, 8, 0xB].into_iter().enumerate() {
            let v = fx.mem.read_u8(g + o);
            fx.mem.write_u8(sp + 0xE8 + k as u32, v);
        }
        fx.mem.write_u32(sp + 0xEC, 0x3F80_0000);
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        let h = |fx: &mut Fx, o: u32| fx.mem.i16(g + o);
        let (u0, v0) = (h(fx, 0x10), h(fx, 0x12));
        fx.set_uv(sp + 0xF0, u0);
        fx.set_uv(sp + 0xF2, v0);
        let u = h(fx, 0x10);
        fx.set_uv(sp + 0x100, u);
        let v = h(fx, 0x12) + h(fx, 0x16) - 1;
        fx.set_uv(sp + 0x102, v);
        let u = h(fx, 0x10) + h(fx, 0x14) - 1;
        fx.set_uv(sp + 0x110, u);
        let v = h(fx, 0x12);
        fx.set_uv(sp + 0x112, v);
        let u = h(fx, 0x10) + h(fx, 0x14) - 1;
        fx.set_uv(sp + 0x120, u);
        let v = h(fx, 0x12) + h(fx, 0x16) - 1;
        fx.set_uv(sp + 0x122, v);
        let xyz = [sp + 0xF8, sp + 0x108, sp + 0x118, sp + 0x128];
        let ws = [sp + 0xC0, sp + 0xC4, sp + 0xC8, sp + 0xCC];
        let mut k = 0i32;
        while k < fx.mem.i16(g + 0x28) {
            let i = u32::from(fx.mem.read_u8(g + 0x2C)).wrapping_add(k as u32) & 0xFF;
            let tab = fx.mem.read_u32(g + 0xC);
            let t = fx.mem.read_u8(tab + k as u32) as i8 as i32;
            let scale = ee::mul(fx.mem.f(g + 0x24), ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(t.abs())));
            let ox = offset(fx, g, i, scale, 0x1B, 0x18);
            let oy = offset(fx, g, i + 1, scale, 0x1C, 0x19);
            let oz = offset(fx, g, i + 2, scale, 0x1D, 0x1A);
            fx.mem.setf(sp + 0xB8, oz);
            for (c, (sx, sy)) in [(false, true), (true, true), (false, false), (true, false)].into_iter().enumerate() {
                let hs = fx.mem.f(g + 0x20);
                fx.mem.setf(sp + 0xB0, if sx { ee::add(ox, hs) } else { ee::sub(ox, hs) });
                let hs = fx.mem.f(g + 0x20);
                fx.mem.setf(sp + 0xB4, if sy { ee::add(oy, hs) } else { ee::sub(oy, hs) });
                let m = fx.read_m4(x + 0x30);
                if fx.project_at(sp + 0xB0, &m, xyz[c], ws[c]) {
                    fx.flag(g + 4, 0x14 + c as u32);
                }
            }
            let rh2 = fx.mem.read_u32(g);
            fx.desc_texture(d, rh2, 0);
            let flags = fx.mem.read_u32(g + 4);
            if bit(flags, 0x13) {
                crate::fxlib2::far_z(fx, &[xyz[3], xyz[2], xyz[1], xyz[0]]);
                fx.submit(d, ot, 0);
            } else if (0x14..0x18).all(|b| bit(flags, b)) {
                let s = ee::add(fx.mem.f(ws[0]), fx.mem.f(ws[1]));
                let s = ee::add(fx.mem.f(ws[2]), s);
                let s = ee::add(fx.mem.f(ws[3]), s);
                let key = ee::fixunssfsi(ee::div(s, 4.0)) as i32;
                fx.submit(d, ot, key);
            }
            let f = fx.mem.read_u32(g + 4) & 0xFF0F_FFFF;
            fx.mem.write_u32(g + 4, f);
            k += 1;
        }
    }
    let a = fx.mem.i16(g + 0x2A) + 1;
    fx.mem.set16(g + 0x2A, a);
    fx.leave(0x150);
    0
}
