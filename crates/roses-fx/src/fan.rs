//! The effect library's "fan" quad group (helper C, 30 Oct 2026),
//! hand-ported from b185 (0x1E03310 setup [c3c279b5733a: 88 words], 0x1E03470
//! draw [b63b30c68827: 388 words]; 6 B records) [F]: the quad group of
//! `fxlib::quads_*` with a trapezoid built from the group's own fields and
//! its quads spread evenly round the y axis.
//!
//! Layout: +0 texture, +4 flags (as the quad group), +8 colour, +0xC size,
//! +0x10 the cell, +0x20 / +0x24 the top / bottom half widths, +0x28 the
//! height, +0x2C the base; corners +0x30 + 16 k; quad angles +0x70 + 16 k.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx, OT};
use crate::fxlib::{ALPHA_42, ALPHA_88};

impl Fx {
    /// c3c279b5733a `setup(g, rh2, size)` [F]: the corner template (the
    /// record's rodata at `template`) with corner 0 = (-top, height + base),
    /// corner 1's y = height + base, corner 2 = (-bottom, base), corner 3's
    /// y = base; corners x size; the quads' y angles 0x10000 / n x k (n = 16
    /// with flag bit 0x1F, else 64).
    pub fn fan_setup(&mut self, g: u32, rh2: u32, size: f32, template: u32) {
        let sp = self.enter(0x80);
        for k in 0..4u32 {
            let v = self.read_v4(template + 16 * k);
            self.write_v4(sp + 0x40 + 16 * k, v);
        }
        let top = ee::neg(self.mem.f(g + 0x20));
        self.mem.setf(sp + 0x40, top);
        let hb = ee::add(self.mem.f(g + 0x28), self.mem.f(g + 0x2C));
        self.mem.setf(sp + 0x44, hb);
        let hb = ee::add(self.mem.f(g + 0x28), self.mem.f(g + 0x2C));
        self.mem.setf(sp + 0x54, hb);
        let bottom = ee::neg(self.mem.f(g + 0x24));
        self.mem.setf(sp + 0x60, bottom);
        let base = self.mem.read_u32(g + 0x2C);
        self.mem.write_u32(sp + 0x64, base);
        self.mem.write_u32(sp + 0x74, base);
        let n = if bit(self.mem.read_u32(g + 4), 0x1F) { 0x10 } else { 0x40 };
        self.mem.setf(g + 0xC, size);
        self.mem.write_u32(g, rh2);
        for k in 0..4u32 {
            for c in 0..3u32 {
                let v = ee::mul(self.mem.f(g + 0xC), self.mem.f(sp + 0x40 + 16 * k + 4 * c));
                self.mem.setf(g + 0x30 + 16 * k + 4 * c, v);
            }
        }
        let (q, _) = self.div(0x1_0000, n);
        for k in 0..n {
            self.mem.set32(g + 0x74 + 16 * k as u32, q.wrapping_mul(k));
        }
        self.leave(0x80);
    }

    /// b63b30c68827 `draw(g, x)` [F]: `fxlib::quads_draw` with the corners
    /// at +0x30 and the angles at +0x70.
    pub fn fan_draw(&mut self, g: u32, x: u32) {
        let sp = self.enter(0x1A0);
        let n = if bit(self.mem.read_u32(g + 4), 0x1F) { 0x10 } else { 0x40 };
        self.mem.write_u32(sp + 0xA0, n);
        let d = sp + 0x110;
        self.desc_init(d, 12, 0x34, 0x4214_2142_1421);
        self.alpha_for(d, g + 4, &[(0xB, ALPHA_88), (0xA, ALPHA_42)]);
        self.colours4(d, g + 8);
        let uv = [sp + 0x128, sp + 0x140, sp + 0x158, sp + 0x170];
        if bit(self.mem.read_u32(g + 4), 0) {
            let f = self.mem.read_u16(d + 2);
            self.mem.write_u16(d + 2, f | 0x800);
            let (u, v, w, h) = (self.mem.i16(g + 0x10), self.mem.i16(g + 0x12), self.mem.i16(g + 0x14), self.mem.i16(g + 0x16));
            for (k, &a) in uv.iter().enumerate() {
                self.set_uv(a, if k % 2 == 1 { u + w - 1 } else { u });
                self.set_uv(a + 2, if k >= 2 { v + h - 1 } else { v });
            }
        } else {
            let (s, t) = (self.mem.f(g + 0x10), self.mem.f(g + 0x14));
            let sw = ee::add(self.mem.f(g + 0x10), self.mem.f(g + 0x18));
            let th = ee::add(self.mem.f(g + 0x14), self.mem.f(g + 0x1C));
            for (k, &a) in uv.iter().enumerate() {
                self.mem.setf(a, if k % 2 == 1 { sw } else { s });
                self.mem.setf(a + 4, if k >= 2 { th } else { t });
            }
        }
        let m = self.read_m4(x + 0x30);
        let xyz = [sp + 0x130, sp + 0x148, sp + 0x160, sp + 0x178];
        let ws = [sp + 0x100, sp + 0x104, sp + 0x108, sp + 0x10C];
        let mut k = 0;
        while k < self.mem.i32(sp + 0xA0) {
            let s = g + 16 * k as u32;
            let rot = [self.mem.i32(s + 0x70), self.mem.i32(s + 0x74), self.mem.i32(s + 0x78)];
            let r = self.rotation_matrix(rot);
            self.write_m4(sp + 0xC0, &r);
            for i in 0..4usize {
                let c = self.read_v4(g + 0x30 + 16 * i as u32);
                let o = self.transform(Some(&r), c);
                self.write_v4(sp + 0xB0, o);
                if self.project_at(sp + 0xB0, &m, xyz[i], ws[i]) {
                    self.flag(g + 4, 0x14 + i as u32);
                }
            }
            let rh2 = self.mem.read_u32(g);
            self.desc_texture(d, rh2, 0);
            if self.all_projected(g + 4) {
                if bit(self.mem.read_u32(g + 4), 0x13) {
                    self.far_z(&xyz);
                    self.submit(d, OT, 0);
                } else if bit(self.mem.read_u32(g + 4), 0x12) {
                    self.submit(d, OT, 0xFF_FFFF);
                } else {
                    let key = self.mean_key4(ws);
                    self.submit(d, OT, key);
                }
            }
            self.clear_projected(g + 4);
            k += 1;
        }
        self.leave(0x1A0);
    }
}
