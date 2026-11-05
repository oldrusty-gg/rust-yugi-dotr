//! The effect library's ribbon trail (helper C, 30 Oct 2026), hand-ported
//! from b263 (0x1E02800 push [ba974e6e98ad], 0x1E02950 draw
//! [4be6b21d7b18: 624 words, 28 B records]) [F].
//!
//! A ribbon group `g`: +0 the texture (RH2), +4 the colour (B, G, R, A;
//! the A also each new point's alpha), +8 flags (bits 0xA..0xC the
//! alpha mode, 0x13 far Z, 0x1F draw every slot), +0xC the width, +0x10
//! the cell (x, y, w, h), +0x22 the fade step, +0x24 an update counter,
//! +0x26 the life, +0x28 the slots, +0x2A the live count. Points (16
//! bytes): x, y, z, +0xC alpha, +0xD (0), +0xE age.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx, OT};
use crate::fxlib::{ALPHA_42, ALPHA_44, ALPHA_88, ALPHA_ADD};

impl Fx {
    /// ba974e6e98ad (b263 0x1E02800) `push(g, pts, p)`: points older than
    /// the life fade by +0x22 (the live count drops to a point that has
    /// faded with its predecessor); while the count is below slots - 1 the
    /// points shift up one and `p` goes in at 0 (alpha +7, age 0).
    pub fn ribbon_push(&mut self, g: u32, pts: u32, p: u32) {
        self.enter(0x60);
        let mut k = self.mem.i16(g + 0x2A);
        while k >= 0 {
            let q = pts.wrapping_add((16 * k) as u32);
            if self.mem.i16(g + 0x26) < self.mem.i16(q + 0xE) {
                let step = self.mem.u8(g + 0x22);
                let r = self.step_down(q + 0xC, 0, step);
                let before = self.mem.read_u8(q.wrapping_sub(4));
                if r == 0 && before == 0 {
                    self.mem.set16(g + 0x2A, k);
                }
            }
            k -= 1;
        }
        let n = self.mem.i16(g + 0x2A);
        if n < self.mem.i16(g + 0x28) - 1 {
            self.mem.set16(g + 0x2A, n + 1);
            let mut i = self.mem.i16(g + 0x2A);
            while i > 0 {
                let q = pts + 16 * i as u32;
                for c in [0u32, 4, 8] {
                    let w = self.mem.read_u32(q - 0x10 + c);
                    self.mem.write_u32(q + c, w);
                }
                let b = self.mem.read_u8(q - 3);
                self.mem.write_u8(q + 0xD, b);
                let h = self.mem.read_u16(q - 2);
                self.mem.write_u16(q + 0xE, h);
                let a = self.mem.read_u8(q - 4);
                self.mem.write_u8(q + 0xC, a);
                i -= 1;
            }
            for c in [0u32, 4, 8] {
                let w = self.mem.read_u32(p + c);
                self.mem.write_u32(pts + c, w);
            }
            self.mem.write_u8(pts + 0xD, 0);
            self.mem.write_u16(pts + 0xE, 0);
            let a = self.mem.read_u8(g + 7);
            self.mem.write_u8(pts + 0xC, a);
        }
        self.leave(0x60);
    }

    /// 4be6b21d7b18 (b263 0x1E02950) `draw(g, pts, x)`: every point ages;
    /// each pair of consecutive points (both on screen through `x`+0x30,
    /// 0x204D80) is drawn as four textured strips (UV from the cell, clamp
    /// to it) widened by the view-scaled width (0x1018E0) up / down / right
    /// / left, alpha the lesser of the group's and the point's (0 at the
    /// first pair), OT key the mean w (or the far Z, bit 0x13).
    pub fn ribbon_draw(&mut self, g: u32, pts: u32, x: u32) {
        let sp = self.enter(0x1B0);
        self.mem.write_u32(sp + 0xE8, OT);
        self.mem.write_u32(sp + 0xE0, x);
        let n = if bit(self.mem.read_u32(g + 8), 0x1F) { self.mem.i16(g + 0x28) } else { self.mem.i16(g + 0x2A) };
        self.mem.set32(sp + 0xB0, n);
        let d = sp + 0x118;
        self.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
        let f = self.mem.read_u32(g + 8);
        let alpha = if bit(f, 0xC) {
            ALPHA_44
        } else if bit(f, 0xB) {
            ALPHA_88
        } else if bit(f, 0xA) {
            ALPHA_42
        } else {
            ALPHA_ADD
        };
        self.desc_alpha(d, alpha);
        self.colours4(d, g + 4);
        let fl = self.mem.read_u16(d + 2) | 0x800;
        self.mem.write_u16(d + 2, fl);
        let (x0, y0, w, h) = (self.mem.i16(g + 0x10), self.mem.i16(g + 0x12), self.mem.i16(g + 0x14), self.mem.i16(g + 0x16));
        for (at, u, v) in [(0x130u32, x0, y0), (0x148, x0 + w, y0), (0x160, x0, y0 + h), (0x178, x0 + w, y0 + h)] {
            self.set_uv(sp + at, u);
            self.set_uv(sp + at + 2, v);
        }
        let mut i = 0;
        while i < n + 1 {
            let q = pts.wrapping_add((16 * i) as u32);
            let a = self.mem.i16(q + 0xE) + 1;
            self.mem.set16(q + 0xE, a);
            i += 1;
        }
        self.mem.set32(sp + 0xD0, 1);
        if self.mem.i32(sp + 0xB0) >= 2 {
            self.mem.write_u32(sp + 0xC0, pts + 0x10);
            loop {
                let cur = self.mem.read_u32(sp + 0xC0);
                let m = self.read_m4(x + 0x30);
                for (src, xyz, wv, bitn) in [(cur.wrapping_sub(0x10), sp + 0xF0, sp + 0x1A8, 0x10_0000u32), (cur, sp + 0xF8, sp + 0x1AC, 0x20_0000)] {
                    for c in 0..3u32 {
                        let v = self.mem.read_u32(src + 4 * c);
                        self.mem.write_u32(sp + 0x100 + 4 * c, v);
                    }
                    if self.project_at(sp + 0x100, &m, xyz, wv) {
                        let f = self.mem.read_u32(g + 8) | bitn;
                        self.mem.write_u32(g + 8, f);
                    }
                }
                let f = self.mem.read_u32(g + 8);
                if bit(f, 0x14) && bit(f, 0x15) {
                    self.ribbon_strips(g, sp, cur);
                }
                let f = self.mem.read_u32(g + 8) & 0xFFCF_FFFF;
                let cur = self.mem.read_u32(sp + 0xC0) + 0x10;
                self.mem.write_u32(sp + 0xC0, cur);
                let k = self.mem.i32(sp + 0xD0) + 1;
                self.mem.set32(sp + 0xD0, k);
                self.mem.write_u32(g + 8, f);
                if k >= self.mem.i32(sp + 0xB0) {
                    break;
                }
            }
        }
        let c = self.mem.i16(g + 0x24) + 1;
        self.mem.set16(g + 0x24, c);
        self.leave(0x1B0);
    }

    /// The four strips of one pair (the draw's inner loop, 0x1E02D08..).
    fn ribbon_strips(&mut self, g: u32, sp: u32, cur: u32) {
        let d = sp + 0x118;
        let size = self.mem.f(g + 0xC);
        let wa = self.mem.f(sp + 0x1A8);
        let f20 = self.view_scale(size, wa);
        let size = self.mem.f(g + 0xC);
        let wb = self.mem.f(sp + 0x1AC);
        let f21 = self.view_scale(size, wb);
        let ga = self.mem.u8(g + 7);
        let pa = self.mem.u8(cur.wrapping_sub(4));
        let a = if ga < pa { ga } else { pa } as u8;
        self.mem.write_u8(sp + 0x15B, a);
        self.mem.write_u8(sp + 0x12B, a);
        let ga = self.mem.u8(g + 7);
        let ca = self.mem.u8(cur + 0xC);
        let a = if ga < ca { ga } else { ca } as u8;
        self.mem.write_u8(sp + 0x173, a);
        self.mem.write_u8(sp + 0x143, a);
        if self.mem.i32(sp + 0xD0) == 1 {
            self.mem.write_u8(sp + 0x15B, 0);
            self.mem.write_u8(sp + 0x12B, 0);
        }
        let widen = |c: u16, off: f32, plus: bool| -> u16 {
            let f = ee::from_i32(i32::from(c));
            let v = if plus { ee::add(f, off) } else { ee::sub(f, off) };
            ee::fixunssfsi(v) as u16
        };
        for j in 0..4 {
            if j < 2 {
                let px = self.mem.read_u16(sp + 0xF0);
                self.mem.write_u16(sp + 0x168, px);
                self.mem.write_u16(sp + 0x138, px);
                let cx = self.mem.read_u16(sp + 0xF8);
                self.mem.write_u16(sp + 0x180, cx);
                self.mem.write_u16(sp + 0x150, cx);
                let py = self.mem.read_u16(sp + 0xF2);
                let cy = self.mem.read_u16(sp + 0xFA);
                self.mem.write_u16(sp + 0x13A, widen(py, f20, j == 0));
                self.mem.write_u16(sp + 0x152, widen(cy, f21, j == 0));
                self.mem.write_u16(sp + 0x16A, py);
                self.mem.write_u16(sp + 0x182, cy);
            } else {
                let py = self.mem.read_u16(sp + 0xF2);
                let cy = self.mem.read_u16(sp + 0xFA);
                self.mem.write_u16(sp + 0x16A, py);
                self.mem.write_u16(sp + 0x13A, py);
                self.mem.write_u16(sp + 0x182, cy);
                self.mem.write_u16(sp + 0x152, cy);
                let px = self.mem.read_u16(sp + 0xF0);
                self.mem.write_u16(sp + 0x138, widen(px, f20, j == 2));
                let cx = self.mem.read_u16(sp + 0xF8);
                self.mem.write_u16(sp + 0x150, widen(cx, f21, j == 2));
                self.mem.write_u16(sp + 0x168, px);
                self.mem.write_u16(sp + 0x180, cx);
            }
            let (x0, y0, w, h) = (self.mem.i16(g + 0x10), self.mem.i16(g + 0x12), self.mem.i16(g + 0x14), self.mem.i16(g + 0x16));
            let clamp = 0xA
                | ((x0 as i64 as u64) << 4)
                | ((((x0 + w - 1) as i64) as u64) << 14)
                | ((y0 as i64 as u64) << 24)
                | ((((y0 + h - 1) as i64) as u64) << 34);
            self.desc_clamp(d, clamp);
            let tex = self.mem.read_u32(g);
            self.desc_texture(d, tex, 1);
            let v0 = self.mem.read_u64(sp + 0x138);
            if !Fx::on_screen(v0) {
                continue;
            }
            let v1 = self.mem.read_u64(sp + 0x150);
            if !Fx::on_screen(v1) {
                continue;
            }
            if bit(self.mem.read_u32(g + 8), 0x13) {
                self.set_xyz_z(sp + 0x180, 0xFF_FFFF);
                let z = self.xyz_z(sp + 0x180);
                self.set_xyz_z(sp + 0x168, z);
                self.set_xyz_z(sp + 0x150, z);
                self.set_xyz_z(sp + 0x138, z);
                self.submit(d, OT, 0);
            } else {
                let z = self.xyz_z(sp + 0xF0);
                self.set_xyz_z(sp + 0x168, z);
                let z = self.xyz_z(sp + 0x168);
                self.set_xyz_z(sp + 0x138, z);
                let z = self.xyz_z(sp + 0xF8);
                self.set_xyz_z(sp + 0x180, z);
                let z = self.xyz_z(sp + 0x180);
                self.set_xyz_z(sp + 0x150, z);
                let wsum = ee::add(self.mem.f(sp + 0x1A8), self.mem.f(sp + 0x1AC));
                let key = ee::fixunssfsi(ee::div(wsum, 2.0));
                self.submit(d, OT, key as i32);
            }
        }
    }
}
