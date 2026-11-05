//! The effect library's "tube", hand-ported from
//! b600 (card 388: 0x1E01160 setup [2e6e76fc3163: 64 words], 0x1E01260
//! draw [7e9d99ed8199: 456 words]; 5 B records) [F]: rings of n points
//! round the y axis (n <= 36), one ring per record of a list, drawn as a
//! band of textured gouraud quads between consecutive rings, the texture
//! scrolling by one row per draw.
//!
//! Layout of the group `g`: +0 texture (RH2), +4 colour (B, G, R, A), +7
//! alpha cap, +8 flags (0xA..0xC blend, 0xF radius growing per ring, 0x14
//! ..0x17 projected), +0xC radius, +0x10 / +0x12 / +0x14 / +0x16 the cell
//! (x, y, w, h), +0x20 + 8 k (cos, sin) of point k, +0x140 n (i16), +0x142
//! the scroll counter (i16), +0x144 the ring spacing (y per ring), +0x148
//! the records (16 bytes, alpha at +0xC), +0x14C their count (i16),
//! +0x14E the rings drawn (i16, at most count - 1), +0x150 the radius
//! growth per ring.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};
use crate::fxlib::{ALPHA_42, ALPHA_44, ALPHA_88, ALPHA_ADD};

/// The UV field (already 12.4) of the u16 at `a`, its top bits kept.
fn put_uv(fx: &mut Fx, a: u32, v: i32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | ((v & 0x3FFF) as u16));
}

impl Fx {
    /// 2e6e76fc3163 `setup(g, rh2) -> 0` [F]: the texture; n capped at 36;
    /// point k's (cos, sin) of the angle k (0x10000 - 0x10000 / n) (table
    /// trig 0x20ABF0 / 0x20ACC0); every record's alpha = the cap +7.
    pub fn tube_setup(&mut self, g: u32, rh2: u32) -> i32 {
        self.enter(0x60);
        self.mem.write_u32(g, rh2);
        if self.mem.i16(g + 0x140) >= 0x25 {
            self.mem.set16(g + 0x140, 0x24);
        }
        let n = self.mem.i16(g + 0x140);
        let (q, _) = self.div(0x1_0000, n);
        let step = 0x1_0000i32.wrapping_sub(q);
        let mut a = 0i32;
        let mut k = 0;
        while k < self.mem.i16(g + 0x140) {
            let c = self.tcos(a);
            self.mem.setf(g + 0x20 + 8 * k as u32, c);
            let s = self.tsin(a);
            self.mem.setf(g + 0x24 + 8 * k as u32, s);
            a = a.wrapping_add(step);
            k += 1;
        }
        let mut k = 0;
        while k < self.mem.i16(g + 0x14C) {
            let r = self.mem.read_u32(g + 0x148).wrapping_add(16 * k as u32);
            let cap = self.mem.read_u8(g + 7);
            self.mem.write_u8(r + 0xC, cap);
            k += 1;
        }
        self.leave(0x60);
        0
    }

    /// 7e9d99ed8199 `draw(ot, g, x) -> 0` [F]: a 12-value descriptor at sp
    /// + 0x160 (blend by flags 0xC / 0xB / 0xA: 0x44 / 0x88 / 0x42, else
    /// 0x48; the colour; REGION_REPEAT-style clamp from the cell; the
    /// texture, bilinear), the rings drawn clamped to count - 1; for ring
    /// r and point k the quad (k, r), (k, r + 1), (k + 1, r), (k + 1, r +
    /// 1) at radius +0xC (+ r +0x150 with flag 0xF), height r +0x144,
    /// projected through x+0x30 (0x204D80); U from k w / n, V from h -
    /// scroll % h + r (h / count); alphas min(record r / r + 1, cap), 0 on
    /// the first ring's inner and the last ring's outer edge; the key the
    /// mean w (0x1A2C70). Then the scroll counter + 1.
    pub fn tube_draw(&mut self, ot: u32, g: u32, x: u32) -> i32 {
        let sp = self.enter(0x1F0);
        self.mem.write_u32(sp + 0x108, ot);
        let d = sp + 0x160;
        self.desc_init(d, 12, 0x34, 0x4214_2142_1421);
        let flags = self.mem.read_u32(g + 8);
        let alpha = if bit(flags, 0xC) {
            ALPHA_44
        } else if bit(flags, 0xB) {
            ALPHA_88
        } else if bit(flags, 0xA) {
            ALPHA_42
        } else {
            ALPHA_ADD
        };
        self.desc_alpha(d, alpha);
        self.colours4(d, g + 4);
        let f = self.mem.read_u16(d + 2);
        self.mem.write_u16(d + 2, f | 0x800);
        let (cx, cw, cy, ch) = (self.mem.i16(g + 0x10), self.mem.i16(g + 0x14), self.mem.i16(g + 0x12), self.mem.i16(g + 0x16));
        let clamp = (((cx as i64) << 4) as u64 | 0xE) | (((cx + cw - 1) as i64 as u64) << 14) | (((cy + ch - 1) as i64 as u64) << 24) | ((cy as i64 as u64) << 34);
        self.desc_clamp(d, clamp);
        let rh2 = self.mem.read_u32(g);
        self.desc_texture(d, rh2, 1);
        let last = self.mem.i16(g + 0x14C) - 1;
        if last < self.mem.i16(g + 0x14E) {
            self.mem.set16(g + 0x14E, last);
        }
        self.mem.write_u32(sp + 0xD0, sp + 0x17A);
        self.mem.write_u32(sp + 0xE0, sp + 0x192);
        let m = self.read_m4(x + 0x30);
        let mut row = 0;
        while row < self.mem.i16(g + 0x14E) {
            let (h, frame, count) = (self.mem.i16(g + 0x16), self.mem.i16(g + 0x142), self.mem.i16(g + 0x14C));
            let (_, rem) = self.div(frame, h);
            let base = h - rem;
            let (q, _) = self.div(h, count);
            let v0 = (base.wrapping_add(self.mult(row, q)) as i16 as i32) << 4;
            let v1 = (base.wrapping_add(self.mult(row + 1, q)) as i16 as i32) << 4;
            self.mem.set32(sp + 0xB0, v0);
            self.mem.set32(sp + 0xC0, v1);
            let rec = 16 * row as u32;
            let mut k = 0;
            while k < self.mem.i16(g + 0x140) {
                let n = self.mem.i16(g + 0x140);
                let w = self.mem.i16(g + 0x14);
                let (cell, _) = self.div(w, n);
                let u0 = ((self.mult(k, cell) as i16 as i32) << 4) & 0x3FFF;
                let u1 = ((self.mult(k + 1, cell) as i16 as i32) << 4) & 0x3FFF;
                let (v0, v1) = (v0 & 0x3FFF, v1 & 0x3FFF);
                for (a, v) in [(0x178u32, u0), (0x17A, v0), (0x190, u0), (0x192, v1), (0x1A8, u1), (0x1AA, v0), (0x1C0, u1), (0x1C2, v1)] {
                    put_uv(self, sp + a, v);
                }
                let recs = self.mem.read_u32(g + 0x148).wrapping_add(rec);
                let cap = self.mem.u8(g + 7);
                let a = self.mem.u8(recs + 0xC).min(cap);
                self.mem.set8(sp + 0x1A3, a);
                self.mem.set8(sp + 0x173, a);
                let a = self.mem.u8(recs + 0x1C).min(cap);
                self.mem.set8(sp + 0x1BB, a);
                self.mem.set8(sp + 0x18B, a);
                if row == 0 {
                    self.mem.write_u8(sp + 0x1A3, 0);
                    self.mem.write_u8(sp + 0x173, 0);
                }
                if row == self.mem.i16(g + 0x14E) - 1 {
                    self.mem.write_u8(sp + 0x1BB, 0);
                    self.mem.write_u8(sp + 0x18B, 0);
                }
                let next = if k + 1 < self.mem.i16(g + 0x140) { k + 1 } else { 0 };
                let (r0, r1) = if bit(self.mem.read_u32(g + 8), 0xF) {
                    let (grow, base) = (self.mem.f(g + 0x150), self.mem.f(g + 0xC));
                    (ee::add(base, ee::mul(grow, ee::from_i32(row))), ee::add(base, ee::mul(grow, ee::from_i32(row + 1))))
                } else {
                    let r = self.mem.f(g + 0xC);
                    (r, r)
                };
                self.mem.write_u32(sp + 0xF0, g + 0x20 + 8 * next as u32);
                let quad = [(k, r0, row, 0x110u32, 0x180u32, 0x150u32), (k, r1, row + 1, 0x120, 0x198, 0x154), (next, r0, row, 0x130, 0x1B0, 0x158), (next, r1, row + 1, 0x140, 0x1C8, 0x15C)];
                for (i, &(p, r, ring, v, xyz, wv)) in quad.iter().enumerate() {
                    let c = self.mem.f(g + 0x20 + 8 * p as u32);
                    self.mem.setf(sp + v + 8, ee::mul(c, r));
                    let s = self.mem.f(g + 0x24 + 8 * p as u32);
                    self.mem.setf(sp + v, ee::mul(s, r));
                    let y = ee::mul(self.mem.f(g + 0x144), ee::from_i32(ring));
                    self.mem.setf(sp + v + 4, y);
                    if self.project_at(sp + v, &m, sp + xyz, sp + wv) {
                        self.flag(g + 8, 0x14 + i as u32);
                    }
                }
                if self.all_projected(g + 8) {
                    let key = self.mean_key4([sp + 0x150, sp + 0x154, sp + 0x158, sp + 0x15C]);
                    let ot = self.mem.read_u32(sp + 0x108);
                    self.submit(d, ot, key);
                }
                self.clear_projected(g + 8);
                k += 1;
            }
            row += 1;
        }
        let c = self.mem.i16(g + 0x142) + 1;
        self.mem.set16(g + 0x142, c);
        self.leave(0x1F0);
        0
    }
}
