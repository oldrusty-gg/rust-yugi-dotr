//! 38c063c9ee2c / 5acc58819dd7 (b648 0x1E037A0 / 0x1E03910; 5 B records
//! each): the drifting ribbon, a cousin of phase 2's ribbon pair
//! ([`Fx::ribbon_push`] / [`Fx::ribbon_draw`], `crate::ribbon`) on a wider
//! group [F].
//!
//! A group `g`: +0 the texture (RH2), +4 the colour (B, G, R, A; the A also
//! each new point's alpha), +8 flags (bits 0xA..0xC the alpha mode, 0x13
//! far Z, 0x1E two half-width strips, 0x1F draw every slot), +0xC the
//! width, +0x10 the cell (x, y, w, h), +0x20 a drift (x, y, z) every point
//! takes when it shifts up a slot, +0x30 a factor on the shifted x, +0x34
//! the width's growth an age, +0x3A the fade step, +0x3C an update
//! counter, +0x3E the life, +0x40 the slots, +0x42 the live count. Points
//! (16 bytes): x, y, z, +0xC alpha, +0xD (0), +0xE age.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};
use crate::fxlib::{ALPHA_42, ALPHA_44, ALPHA_88, ALPHA_ADD};

/// 38c063c9ee2c `push(g, pts, p) -> 0`: points older than the life fade
/// by +0x3A (the live count drops to a point that has faded with its
/// predecessor); while the count is below slots - 1 the points shift up
/// one slot, drifting by +0x20 (the x then scaled by +0x30), and `p` goes
/// in at 0 (alpha +7, age 0). A 0x60 frame.
pub fn push(fx: &mut Fx, g: u32, pts: u32, p: u32) -> i32 {
    fx.enter(0x60);
    let mut k = fx.mem.i16(g + 0x42);
    while k >= 0 {
        let q = pts.wrapping_add((16 * k) as u32);
        if fx.mem.i16(g + 0x3E) < fx.mem.i16(q + 0xE) {
            let step = fx.mem.u8(g + 0x3A);
            let r = fx.step_down(q + 0xC, 0, step);
            let before = fx.mem.read_u8(q.wrapping_sub(4));
            if r == 0 && before == 0 {
                fx.mem.set16(g + 0x42, k);
            }
        }
        k -= 1;
    }
    let n = fx.mem.i16(g + 0x42);
    if n < fx.mem.i16(g + 0x40) - 1 {
        fx.mem.set16(g + 0x42, n + 1);
        let mut i = fx.mem.i16(g + 0x42);
        while i > 0 {
            let q = pts + 16 * i as u32;
            let x = ee::add(fx.mem.f(g + 0x20), fx.mem.f(q - 0x10));
            fx.mem.setf(q, x);
            let x = ee::mul(x, fx.mem.f(g + 0x30));
            fx.mem.setf(q, x);
            let y = ee::add(fx.mem.f(g + 0x24), fx.mem.f(q - 0xC));
            fx.mem.setf(q + 4, y);
            let z = ee::add(fx.mem.f(g + 0x28), fx.mem.f(q - 8));
            fx.mem.setf(q + 8, z);
            let b = fx.mem.read_u8(q - 3);
            fx.mem.write_u8(q + 0xD, b);
            let h = fx.mem.read_u16(q - 2);
            fx.mem.write_u16(q + 0xE, h);
            let a = fx.mem.read_u8(q - 4);
            fx.mem.write_u8(q + 0xC, a);
            i -= 1;
        }
        for c in [0u32, 4, 8] {
            let w = fx.mem.read_u32(p + c);
            fx.mem.write_u32(pts + c, w);
        }
        fx.mem.write_u8(pts + 0xD, 0);
        fx.mem.write_u16(pts + 0xE, 0);
        let a = fx.mem.read_u8(g + 7);
        fx.mem.write_u8(pts + 0xC, a);
    }
    fx.leave(0x60);
    0
}

/// 5acc58819dd7 `draw(ot, g, pts, x) -> 0`: each pair of consecutive
/// points (both projected through `x`+0x30, 0x204D80; no screen test) is
/// drawn as one strip widened up and down by the view-scaled width (+0xC
/// plus +0x34 x the point's age, 0x1018E0), or with bit 0x1E as two
/// half-width strips (below, then above), alpha the lesser of the group's
/// and the point's (0 at the first pair), OT key the mean w (or the far Z,
/// bit 0x13); the clamp and texture set once; then every point ages and
/// +0x3C counts the update. A 0x1D0 frame: the descriptor at sp + 0x138
/// (12 regs, prim 0x34).
pub fn draw(fx: &mut Fx, ot: u32, g: u32, pts: u32, x: u32) -> i32 {
    let sp = fx.enter(0x1D0);
    fx.mem.write_u32(sp + 0x100, pts);
    fx.mem.write_u32(sp + 0xF0, x);
    let n = if bit(fx.mem.read_u32(g + 8), 0x1F) { fx.mem.i16(g + 0x40) } else { fx.mem.i16(g + 0x42) };
    fx.mem.set32(sp + 0xC0, n);
    let strips = if bit(fx.mem.read_u32(g + 8), 0x1E) { 2 } else { 1 };
    fx.mem.set32(sp + 0xD0, strips);
    let d = sp + 0x138;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    let f = fx.mem.read_u32(g + 8);
    let alpha = if bit(f, 0xC) {
        ALPHA_44
    } else if bit(f, 0xB) {
        ALPHA_88
    } else if bit(f, 0xA) {
        ALPHA_42
    } else {
        ALPHA_ADD
    };
    fx.desc_alpha(d, alpha);
    fx.colours4(d, g + 4);
    let fl = fx.mem.read_u16(d + 2) | 0x800;
    fx.mem.write_u16(d + 2, fl);
    let (x0, y0, w, h) = (fx.mem.i16(g + 0x10), fx.mem.i16(g + 0x12), fx.mem.i16(g + 0x14), fx.mem.i16(g + 0x16));
    for (at, u, v) in [(0x150u32, x0, y0), (0x168, x0 + w, y0), (0x180, x0, y0 + h), (0x198, x0 + w, y0 + h)] {
        fx.set_uv(sp + at, u);
        fx.set_uv(sp + at + 2, v);
    }
    let clamp = 0xA
        | ((x0 as i64 as u64) << 4)
        | ((((x0 + w - 1) as i64) as u64) << 14)
        | ((y0 as i64 as u64) << 24)
        | ((((y0 + h - 1) as i64) as u64) << 34);
    fx.desc_clamp(d, clamp);
    let tex = fx.mem.read_u32(g);
    fx.desc_texture(d, tex, 1);
    fx.mem.set32(sp + 0xB0, 1);
    if fx.mem.i32(sp + 0xC0) >= 2 {
        fx.mem.write_u32(sp + 0xE0, pts + 0x10);
        loop {
            let cur = fx.mem.read_u32(sp + 0xE0);
            let m = fx.read_m4(x + 0x30);
            for (src, xyz, wv, bitn) in [(cur.wrapping_sub(0x10), sp + 0x110, sp + 0x1C8, 0x10_0000u32), (cur, sp + 0x118, sp + 0x1CC, 0x20_0000)] {
                for c in 0..3u32 {
                    let v = fx.mem.read_u32(src + 4 * c);
                    fx.mem.write_u32(sp + 0x120 + 4 * c, v);
                }
                if fx.project_at(sp + 0x120, &m, xyz, wv) {
                    let f = fx.mem.read_u32(g + 8) | bitn;
                    fx.mem.write_u32(g + 8, f);
                }
            }
            let f = fx.mem.read_u32(g + 8);
            if bit(f, 0x14) && bit(f, 0x15) {
                strip(fx, ot, g, sp, cur);
            }
            let f = fx.mem.read_u32(g + 8) & 0xFFCF_FFFF;
            let cur = fx.mem.read_u32(sp + 0xE0) + 0x10;
            fx.mem.write_u32(sp + 0xE0, cur);
            let k = fx.mem.i32(sp + 0xB0) + 1;
            fx.mem.set32(sp + 0xB0, k);
            fx.mem.write_u32(g + 8, f);
            if k >= fx.mem.i32(sp + 0xC0) {
                break;
            }
        }
    }
    let n = fx.mem.i32(sp + 0xC0);
    let mut i = 0;
    while i < n + 1 {
        let q = pts.wrapping_add((16 * i) as u32);
        let a = fx.mem.i16(q + 0xE) + 1;
        fx.mem.set16(q + 0xE, a);
        i += 1;
        fx.mem.write_u32(sp + 0x100, q + 0x10);
    }
    let c = fx.mem.i16(g + 0x3C) + 1;
    fx.mem.set16(g + 0x3C, c);
    fx.leave(0x1D0);
    0
}

/// The strips of one pair (the draw's inner loop, 0x1E03D20..).
fn strip(fx: &mut Fx, ot: u32, g: u32, sp: u32, cur: u32) {
    let d = sp + 0x138;
    let grow = |fx: &mut Fx, age: i32| {
        let w = ee::mul(fx.mem.f(g + 0x34), ee::from_i32(age));
        ee::add(fx.mem.f(g + 0xC), w)
    };
    let age = fx.mem.i16(cur.wrapping_sub(2));
    let size = grow(fx, age);
    let wa = fx.mem.f(sp + 0x1C8);
    let f20 = fx.view_scale(size, wa);
    let age = fx.mem.i16(cur + 0xE);
    let size = grow(fx, age);
    let wb = fx.mem.f(sp + 0x1CC);
    let f21 = fx.view_scale(size, wb);
    let ga = fx.mem.u8(g + 7);
    let pa = fx.mem.u8(cur.wrapping_sub(4));
    let a = if ga < pa { ga } else { pa } as u8;
    fx.mem.write_u8(sp + 0x17B, a);
    fx.mem.write_u8(sp + 0x14B, a);
    let ga = fx.mem.u8(g + 7);
    let ca = fx.mem.u8(cur + 0xC);
    let a = if ga < ca { ga } else { ca } as u8;
    fx.mem.write_u8(sp + 0x193, a);
    fx.mem.write_u8(sp + 0x163, a);
    if fx.mem.i32(sp + 0xB0) == 1 {
        fx.mem.write_u8(sp + 0x17B, 0);
        fx.mem.write_u8(sp + 0x14B, 0);
    }
    let widen = |c: u16, off: f32, plus: bool| -> u16 {
        let f = ee::from_i32(i32::from(c));
        let v = if plus { ee::add(f, off) } else { ee::sub(f, off) };
        ee::fixunssfsi(v) as u16
    };
    let (v0, v1, v2, v3) = (sp + 0x158, sp + 0x170, sp + 0x188, sp + 0x1A0);
    let strips = fx.mem.i32(sp + 0xD0);
    let mut j = 0;
    while j < strips {
        let px = fx.mem.read_u16(sp + 0x110);
        fx.mem.write_u16(v2, px);
        fx.mem.write_u16(v0, px);
        let cx = fx.mem.read_u16(sp + 0x118);
        fx.mem.write_u16(v3, cx);
        fx.mem.write_u16(v1, cx);
        let py = fx.mem.read_u16(sp + 0x112);
        let cy = fx.mem.read_u16(sp + 0x11A);
        if bit(fx.mem.read_u32(g + 8), 0x1E) {
            // Below (j = 0), then above: a half-width strip each.
            fx.mem.write_u16(v0 + 2, widen(py, f20, j != 0));
            fx.mem.write_u16(v1 + 2, widen(cy, f21, j != 0));
            fx.mem.write_u16(v2 + 2, py);
            fx.mem.write_u16(v3 + 2, cy);
        } else {
            fx.mem.write_u16(v0 + 2, widen(py, f20, false));
            fx.mem.write_u16(v1 + 2, widen(cy, f21, false));
            fx.mem.write_u16(v2 + 2, widen(py, f20, true));
            fx.mem.write_u16(v3 + 2, widen(cy, f21, true));
        }
        if bit(fx.mem.read_u32(g + 8), 0x13) {
            fx.set_xyz_z(v3, 0xFF_FFFF);
            let z = fx.xyz_z(v3);
            fx.set_xyz_z(v2, z);
            fx.set_xyz_z(v1, z);
            fx.set_xyz_z(v0, z);
            fx.submit(d, ot, 0);
        } else {
            let z = fx.xyz_z(sp + 0x110);
            fx.set_xyz_z(v2, z);
            let z = fx.xyz_z(v2);
            fx.set_xyz_z(v0, z);
            let z = fx.xyz_z(sp + 0x118);
            fx.set_xyz_z(v3, z);
            let z = fx.xyz_z(v3);
            fx.set_xyz_z(v1, z);
            let wsum = ee::add(fx.mem.f(sp + 0x1C8), fx.mem.f(sp + 0x1CC));
            let key = ee::fixunssfsi(ee::div(wsum, 2.0));
            fx.submit(d, ot, key as i32);
        }
        j += 1;
    }
}
