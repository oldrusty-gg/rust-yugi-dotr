//! A streamer of camera-facing animated quads along a point history
//!, hand-ported from b025 (card 211) [F]: the
//! one-row sibling of `quadtrail` with its points in a separate array
//! (`push` 63dbc47681c1, `draw` 2056c53d763e).
//!
//! A group `g`: +0 the texture, +4 the colour (B, G, R, A; the A also each
//! new point's alpha), +8 flags (0xA..0xC the blend, 0xF scale by age,
//! 0x10 drift, 0x13 far Z, 0x14..0x17 corners projected, 0x1F draw every
//! slot), +0xC the half size, +0x14 / +0x16 the cell (w, h), +0x18 / +0x1A
//! the sheet's origin, +0x1C / +0x1E the sheet's size, +0x20 the drift,
//! +0x30 the age scale, +0x36 the life, +0x38 the slots, +0x3A the points
//! in use, +0x3C the frames, +0x3E the ages a frame. Points (16 bytes):
//! xyz, +0xC alpha, +0xD frame, +0xE age.
use super::quadtrail::camera_turn;
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};
use crate::fxlib::{ALPHA_42, ALPHA_44, ALPHA_88};

/// RGBAQ then UV / XYZF2 x 4.
const REGS_TEX4_FLAT: u64 = 0x4_2424_2421;

/// 63dbc47681c1 [9 B records; b025 0x1E01F20, 56 words] [F]: `push(g,
/// pts, p) -> 0`: the count +0x3A grows by one below the slots +0x38; it
/// drops to the newest point older than the life +0x36; the points shift
/// up one and `p` goes in at 0 (alpha +7, frame and age 0). No frame.
pub fn push(fx: &mut Fx, g: u32, pts: u32, p: u32) -> i32 {
    let n = fx.mem.i16(g + 0x3A);
    if fx.mem.i16(g + 0x38) - 1 >= n {
        fx.mem.set16(g + 0x3A, n + 1);
    }
    let mut k = fx.mem.i16(g + 0x3A);
    while k >= 0 {
        let age = fx.mem.i16(pts + 16 * k as u32 + 0xE);
        if fx.mem.i16(g + 0x36) < age {
            fx.mem.set16(g + 0x3A, k);
        }
        k -= 1;
    }
    let mut k = fx.mem.i16(g + 0x3A);
    while k > 0 {
        let (to, from) = (pts + 16 * k as u32, pts + 16 * (k - 1) as u32);
        for c in [0u32, 4, 8] {
            let v = fx.mem.read_u32(from + c);
            fx.mem.write_u32(to + c, v);
        }
        let b = fx.mem.read_u8(from + 0xD);
        fx.mem.write_u8(to + 0xD, b);
        let a = fx.mem.read_u16(from + 0xE);
        fx.mem.write_u16(to + 0xE, a);
        let b = fx.mem.read_u8(from + 0xC);
        fx.mem.write_u8(to + 0xC, b);
        k -= 1;
    }
    for c in [0u32, 4, 8] {
        let v = fx.mem.read_u32(p + c);
        fx.mem.write_u32(pts + c, v);
    }
    fx.mem.write_u8(pts + 0xD, 0);
    fx.mem.set16(pts + 0xE, 0);
    let a = fx.mem.read_u8(g + 7);
    fx.mem.write_u8(pts + 0xC, a);
    0
}

/// 2056c53d763e [9 B records; b025 0x1E02000, 616 words] [F]: `draw(at,
/// ot, g, pts, x) -> 0`: `quadtrail::draw` over the points in use (+0x3A;
/// flag 0x1F: every slot +0x38) of `pts`: the record's corner template
/// (record-relative `lui` / `addiu` +0x28 / +0x30) with x / y -+ the half
/// size +0xC, turned to face the camera (0x2055D0, 0x20C540), scaled by
/// +0x30 x age (flag 0xF), at the point (drift +0x20 with flag 0x10), the
/// fourth w on the second's slot and the key's fourth w the stale sp +
/// 0x15C; the frame steps every +0x3E ages and wraps after +0x3C, the
/// point then jumping x by rand % 4 - 2 (rand saving s0 = the cell's u).
pub fn draw(fx: &mut Fx, at: u32, ot: u32, g: u32, pts: u32, x: u32) -> i32 {
    let template = rel(fx, at, 0x28, 0x30);
    let sp = fx.enter(0x270);
    for k in 0..4u32 {
        let v = fx.read_v4(template + 16 * k);
        fx.write_v4(sp + 0x160 + 16 * k, v);
    }
    for (k, (sx, sy)) in [(true, false), (false, false), (true, true), (false, true)].into_iter().enumerate() {
        let a = sp + 0x160 + 16 * k as u32;
        let h = fx.mem.f(g + 0xC);
        fx.mem.setf(a, if sx { ee::neg(h) } else { h });
        let h = fx.mem.f(g + 0xC);
        fx.mem.setf(a + 4, if sy { ee::neg(h) } else { h });
    }
    let count = if bit(fx.mem.read_u32(g + 8), 0x1F) { fx.mem.i16(g + 0x38) } else { fx.mem.i16(g + 0x3A) };
    fx.mem.set32(sp + 0xB0, count);
    camera_turn(fx, sp + 0x110);
    let cam = fx.read_m4(sp + 0x110);
    for k in 0..4u32 {
        let v = fx.read_v4(sp + 0x160 + 16 * k);
        let o = fx.transform(Some(&cam), v);
        fx.write_v4(sp + 0x1A0 + 16 * k, o);
    }
    let d = sp + 0x1F8;
    fx.desc_init(d, 9, 0x24, REGS_TEX4_FLAT);
    fx.mem.write_u32(sp + 0x100, d + 2);
    fx.alpha_for(d, g + 8, &[(0xC, ALPHA_44), (0xB, ALPHA_88), (0xA, ALPHA_42)]);
    for (o, s) in [(0u32, 6u32), (1, 5), (2, 4)] {
        let c = fx.mem.read_u8(g + s);
        fx.mem.write_u8(sp + 0x208 + o, c);
    }
    fx.mem.write_u32(sp + 0x20C, 0x3F80_0000);
    let xyz = [sp + 0x218, sp + 0x228, sp + 0x238, sp + 0x248];
    let wslot = [sp + 0x150, sp + 0x154, sp + 0x158, sp + 0x154];
    let mut i = 0;
    while i < fx.mem.i32(sp + 0xB0) {
        let e = pts + (16 * i) as u32;
        let frame = i32::from(fx.mem.read_u8(e + 0xD));
        let (cw, ch) = (i32::from(fx.mem.i16(g + 0x14)), i32::from(fx.mem.i16(g + 0x16)));
        let (u0, v0) = (i32::from(fx.mem.i16(g + 0x18)), i32::from(fx.mem.i16(g + 0x1A)));
        let (u, v) = if bit(fx.mem.read_u32(g + 8), 1) {
            let sheet = i32::from(fx.mem.i16(g + 0x1C));
            let (cols, _) = fx.div(sheet, cw);
            let (_, col) = fx.div(frame, cols);
            let a = fx.mult(cw, col);
            let (row, _) = fx.div(frame, cols);
            let b = fx.mult(ch, row);
            (u0 + a, v0 + b)
        } else {
            let sheet = i32::from(fx.mem.i16(g + 0x1E));
            let (rws, _) = fx.div(sheet, ch);
            let (col, _) = fx.div(frame, rws);
            let a = fx.mult(cw, col);
            let (_, row) = fx.div(frame, rws);
            let b = fx.mult(ch, row);
            (u0 + a, v0 + b)
        };
        let (u, v) = (i32::from(u as i16), i32::from(v as i16));
        // The frame steps every +0x3E ages; on its wrap the point jumps.
        let age = i32::from(fx.mem.i16(e + 0xE));
        let ticks = i32::from(fx.mem.i16(g + 0x3E));
        let (_, rem) = fx.div(age, ticks);
        if rem == 0 {
            let f = fx.mem.read_u8(e + 0xD).wrapping_add(1);
            fx.mem.write_u8(e + 0xD, f);
            if i32::from(fx.mem.i16(g + 0x3C)) - 1 < i32::from(fx.mem.read_u8(e + 0xD)) {
                fx.mem.write_u8(e + 0xD, 0);
                let r = fx.rand_from(u as u32, at + 0x38C);
                let j = r.wrapping_rem(4) - 2;
                let p = ee::add(fx.mem.f(e), ee::from_i32(j));
                fx.mem.setf(e, p);
            }
        }
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        let (cw, ch) = (i32::from(fx.mem.i16(g + 0x14)), i32::from(fx.mem.i16(g + 0x16)));
        for (k, (du, dv)) in [(false, false), (true, false), (false, true), (true, true)].into_iter().enumerate() {
            let a = sp + 0x210 + 0x10 * k as u32;
            fx.set_uv(a, if du { u + cw - 1 } else { u });
            fx.set_uv(a + 2, if dv { v + ch - 1 } else { v });
        }
        let a = fx.mem.read_u8(e + 0xC);
        fx.mem.write_u8(sp + 0x20B, a);
        let scale = if bit(fx.mem.read_u32(g + 8), 0xF) { ee::mul(fx.mem.f(g + 0x30), ee::from_i32(i32::from(fx.mem.i16(e + 0xE)))) } else { 1.0 };
        if bit(fx.mem.read_u32(g + 8), 0x10) {
            fx.add3(e, g + 0x20);
        }
        let age = fx.mem.i16(e + 0xE).wrapping_add(1);
        fx.mem.set16(e + 0xE, i32::from(age));
        let m = fx.read_m4(x + 0x30);
        for k in 0..4u32 {
            let c = sp + 0x1A0 + 16 * k;
            for j in 0..3u32 {
                let v = ee::add(ee::mul(fx.mem.f(c + 4 * j), scale), fx.mem.f(e + 4 * j));
                fx.mem.setf(sp + 0x1E0 + 4 * j, v);
            }
            if fx.project_at(sp + 0x1E0, &m, xyz[k as usize], wslot[k as usize]) {
                fx.flag(g + 8, 0x14 + k);
            }
        }
        let clamp = ((u as i64) << 4) as u64 | 0xA | (((u + cw - 1) as i64 as u64) << 14) | ((v as i64 as u64) << 24) | (((v + ch - 1) as i64 as u64) << 34);
        fx.desc_clamp(d, clamp);
        let rh2 = fx.mem.read_u32(g);
        fx.desc_texture(d, rh2, 1);
        if fx.all_projected(g + 8) {
            if bit(fx.mem.read_u32(g + 8), 0x13) {
                fx.far_z(&xyz);
                fx.submit(d, ot, 0);
            } else {
                let key = fx.mean_key4([sp + 0x150, sp + 0x154, sp + 0x158, sp + 0x15C]);
                fx.submit(d, ot, key);
            }
        }
        fx.clear_projected(g + 8);
        i += 1;
    }
    fx.leave(0x270);
    0
}
