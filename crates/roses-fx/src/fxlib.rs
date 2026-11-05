//! The effect library linked into the modules: routines compiled into
//! every module that uses them (the same code at a different address in
//! each record), hand-ported once. Named here by their address in c620
//! (CMODULE 620, `c620.bin`) and their normalized-code hash of the
//! inventory (the project notes, private),
//! with how many BMODULE / CMODULE records carry them [F static scan].
//!
//! A *group* is the record a routine works on (in the module's data area):
//! +0 the RH2 texture (or flags, see each), +4 flags whose bits pick
//! blending (0xA 0x42, 0xB 0x88, 0xC 0x44, else 0x48), texturing (0 FST
//! atlas, 0x18 no filter), depth (0x12 the far key, 0x13 the far Z) and
//! bits 0x14..0x17 "vertex k projected" during a draw.
//!
//! Also here: the executable's effect-library helpers they call (0x104540,
//! 0x105260, 0x205210) and descriptor plumbing.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, ee_lt, Fx, M4, OT, V4};

/// ALPHA register values (`0x80 << 32 | A B C D`).
pub const ALPHA_ADD: u64 = 0x80_0000_0048;
pub const ALPHA_42: u64 = 0x80_0000_0042;
pub const ALPHA_88: u64 = 0x80_0000_0088;
pub const ALPHA_44: u64 = 0x80_0000_0044;
/// The far Z / key.
const FAR: u64 = 0xFF_FFFF;
/// Bits 0x14..0x17: the four vertices projected.
const PROJECTED: u32 = 0x00F0_0000;
const CLEAR_PROJECTED: u32 = 0xFF0F_FFFF;
/// Register lists: RGBAQ / XYZF2 x 3 (a triangle), x 4 (a strip);
/// RGBAQ / ST / XYZF2 x 4 (a textured strip, one colour each); RGBAQ then
/// ST / XYZF2 x 4 (one colour).
const REGS_TRI: u64 = 0x41_4141;
pub const REGS_STRIP4: u64 = 0x4141_4141;
const REGS_TEX4: u64 = 0x4214_2142_1421;
const REGS_TEX4_FLAT: u64 = 0x4_2424_2421;
/// Degrees to radians (0x3C8EFA35).
const DEG: f32 = f32::from_bits(0x3C8E_FA35);

impl Fx {
    /// A descriptor header: `n` register values, `flags`, the 4-bit
    /// register list, +4 cleared.
    pub fn desc_init(&mut self, d: u32, n: u16, flags: u16, regs: u64) {
        self.mem.write_u16(d + 2, flags);
        self.mem.write_u16(d, n);
        self.mem.write_u64(d + 8, regs);
        self.mem.write_u32(d + 4, 0);
    }

    /// 0x204D80 with its pointers: the vec at `v` through `m`, the XYZ2
    /// value merged at `xyz`, w stored at `w` (when not 0) on success.
    pub fn project_at(&mut self, v: u32, m: &M4, xyz: u32, w: u32) -> bool {
        let p = [self.mem.read_f32(v), self.mem.read_f32(v + 4), self.mem.read_f32(v + 8)];
        let mut x = self.mem.read_u64(xyz);
        match self.project(p, Some(m), &mut x) {
            Some(wv) => {
                self.mem.write_u64(xyz, x);
                if w != 0 {
                    self.mem.write_f32(w, wv);
                }
                true
            }
            None => false,
        }
    }

    /// Set the z (bits 32..55) of the XYZ2 value at `a`.
    pub fn set_xyz_z(&mut self, a: u32, z: u64) {
        let v = self.mem.read_u64(a);
        self.mem.write_u64(a, (v & 0xFF00_0000_FFFF_FFFF) | (z << 32));
    }
    pub fn xyz_z(&mut self, a: u32) -> u64 {
        (self.mem.read_u64(a) >> 32) & 0xFF_FFFF
    }

    /// The 14-bit UV field (12.4) of the u16 at `a`, its top bits kept.
    pub fn set_uv(&mut self, a: u32, v: i32) {
        let h = self.mem.read_u16(a);
        self.mem.write_u16(a, (h & 0xC000) | (((v << 4) & 0x3FFF) as u16));
    }

    /// Set bit `n` of the group flags at `f`.
    pub(crate) fn flag(&mut self, f: u32, n: u32) {
        let v = self.mem.read_u32(f);
        self.mem.write_u32(f, v | (1 << n));
    }
    pub(crate) fn clear_projected(&mut self, f: u32) {
        let v = self.mem.read_u32(f);
        self.mem.write_u32(f, v & CLEAR_PROJECTED);
    }
    pub(crate) fn all_projected(&mut self, f: u32) -> bool {
        self.mem.read_u32(f) & PROJECTED == PROJECTED
    }

    /// The blend the flags at `f` pick, in the order a routine tests.
    pub(crate) fn alpha_for(&mut self, d: u32, f: u32, order: &[(u32, u64)]) {
        let flags = self.mem.read_u32(f);
        let a = order.iter().find(|(b, _)| bit(flags, *b)).map_or(ALPHA_ADD, |o| o.1);
        self.desc_alpha(d, a);
    }

    /// 0x208400's REGION_CLAMP from an i16 rectangle (x +0, y +2, w +4, h
    /// +6 offsets given): U x..x+w-1, V y..y+h-1.
    pub(crate) fn region_clamp(&mut self, d: u32, x: u32, w: u32, y: u32, h: u32) {
        let (x, w, y, h) = (self.mem.i16(x), self.mem.i16(w), self.mem.i16(y), self.mem.i16(h));
        let v = ((x as i64) << 4) as u64 | 0xA | (((x + w - 1) as i64 as u64) << 14) | ((y as i64 as u64) << 24) | (((y + h - 1) as i64 as u64) << 34);
        self.desc_clamp(d, v);
    }

    /// The ordering-table key of a primitive from the w of its vertices
    /// (0x1A2C70 of their mean, summed in the original's order: ((w0 + w1)
    /// + w2) + w3 for four, (w0 + w1) + w2 for three).
    pub(crate) fn mean_key4(&mut self, w: [u32; 4]) -> i32 {
        let (w0, w1, w2, w3) = (self.mem.f(w[0]), self.mem.f(w[1]), self.mem.f(w[2]), self.mem.f(w[3]));
        let s = ee::add(w3, ee::add(w2, ee::add(w0, w1)));
        ee::fixunssfsi(ee::div(s, 4.0)) as i32
    }
    fn mean_key3(&mut self, w: [u32; 3]) -> i32 {
        let (w0, w1, w2) = (self.mem.f(w[0]), self.mem.f(w[1]), self.mem.f(w[2]));
        let s = ee::add(w2, ee::add(w0, w1));
        ee::fixunssfsi(ee::div(s, 3.0)) as i32
    }

    /// The far-Z draw (bit 0x13): every vertex's z = 0xFFFFFF, copied down
    /// the chain from the last vertex as the original merges it.
    pub(crate) fn far_z(&mut self, xyz: &[u32]) {
        let mut z = FAR;
        for &a in xyz.iter().rev() {
            self.set_xyz_z(a, z);
            z = self.xyz_z(a);
        }
    }

    /// c620 0x1E00080 [f8e6f44d8b9a: 124 B, 45 C records]: the colour at
    /// `src` (B, G, R, A bytes) into the RGBAQ values 0, 3, 6, 9 of a
    /// textured strip, Q = 1.
    pub fn colours4(&mut self, d: u32, src: u32) {
        for off in [0x10u32, 0x28, 0x40, 0x58] {
            let (b2, b1, b0, b3) = (self.mem.read_u8(src + 2), self.mem.read_u8(src + 1), self.mem.read_u8(src), self.mem.read_u8(src + 3));
            self.mem.write_u8(d + off, b2);
            self.mem.write_u8(d + off + 1, b1);
            self.mem.write_u8(d + off + 2, b0);
            self.mem.write_u8(d + off + 3, b3);
            self.mem.write_f32(d + off + 4, 1.0);
        }
    }

    /// 0x205210(in, out, m, desc, mean_w): the descriptor's XYZF2 vertices
    /// (`in`, vec4s, w set to 1) through `m` (VU0) into `out`, then per
    /// vertex q = 1 / w (scaling the Q and ST before it when ST-textured
    /// with IIP), x y z scaled; rejected when z < 1, z > 16777215, x or y
    /// outside [0, 4095]; X / Y = 16 x / 16 y and Z through 0x1A2C70 into
    /// the register values; the mean w to `mean_w` [F; as the interpreter's
    /// native version].
    pub fn project_desc(&mut self, input: u32, out: u32, m: Option<&M4>, desc: u32, mean_w: u32) -> bool {
        let flags = self.mem.read_u16(desc + 2);
        let n = u32::from(self.mem.read_u16(desc));
        let regs = self.mem.read_u64(desc + 8);
        let perspective_st = flags & 0x800 == 0 && flags & 0x20 != 0 && flags & 0x10 != 0;
        let mut count = 0u32;
        for i in 0..n {
            if (regs >> (4 * i)) & 15 == 4 {
                self.mem.write_u32(input + 16 * count + 12, 0x3F80_0000);
                count += 1;
            }
        }
        let m = match m {
            Some(m) => *m,
            None => self.camera_vp(),
        };
        self.push(None);
        for k in 0..count {
            let v: V4 = std::array::from_fn(|j| self.mem.read_f32(((input + 16 * k) & !15) + 4 * j as u32));
            let o = Self::vu_apply(&m, v);
            for j in 0..4 {
                self.mem.write_f32(((out + 16 * k) & !15) + 4 * j as u32, o[j]);
            }
        }
        self.pop();
        let (mut rgbaq, mut st): (Option<u32>, Option<u32>) = (None, None);
        let mut sum_w = 0.0f32;
        let mut v = out;
        for i in 0..n {
            let value = desc + 0x10 + 8 * i;
            match (regs >> (4 * i)) & 15 {
                4 => {
                    let w = self.mem.read_f32(v + 12);
                    let q = ee::div(1.0, w);
                    if perspective_st {
                        if let (Some(c), Some(t)) = (rgbaq, st) {
                            self.mem.write_f32(c + 4, q);
                            let s0 = ee::mul(self.mem.read_f32(t), q);
                            self.mem.write_f32(t, s0);
                            let t0 = ee::mul(self.mem.read_f32(t + 4), q);
                            self.mem.write_f32(t + 4, t0);
                            rgbaq = None;
                            st = None;
                        }
                    }
                    let x = ee::mul(self.mem.read_f32(v), q);
                    self.mem.write_f32(v, x);
                    let y = ee::mul(self.mem.read_f32(v + 4), q);
                    self.mem.write_f32(v + 4, y);
                    let z = ee::mul(self.mem.read_f32(v + 8), q);
                    self.mem.write_f32(v + 8, z);
                    let lim = f32::from_bits(0x457F_F000);
                    if ee_lt(z, 1.0) || !ee_le(z, f32::from_bits(0x4B7F_FFFF)) || ee_lt(x, 0.0) || !ee_le(x, lim) || ee_lt(y, 0.0) || !ee_le(y, lim) {
                        return false;
                    }
                    self.mem.write_u16(value, ee::fixunssfsi(ee::mul(16.0, x)) as u16);
                    self.mem.write_u16(value + 2, ee::fixunssfsi(ee::mul(16.0, y)) as u16);
                    let fz = u64::from(ee::fixunssfsi(z) & 0xFF_FFFF);
                    let old = self.mem.read_u64(value);
                    self.mem.write_u64(value, (old & 0xFF00_0000_FFFF_FFFF) | (fz << 32));
                    sum_w = ee::add(sum_w, w);
                    v += 16;
                }
                2 => st = Some(value),
                1 => rgbaq = Some(value),
                _ => {}
            }
        }
        if mean_w != 0 {
            let mean = ee::div(sum_w, ee::from_i32(count as i32));
            self.mem.write_f32(mean_w, mean);
        }
        true
    }

    /// 0x104540(desc, rect, order): FST UVs of a 4-vertex textured strip
    /// (register values 1, 4, 7, 10) from the i16 rectangle (x, y, w, h),
    /// the corners in one of 8 orders (jump table 0x2B83F0); flag 0x800.
    pub fn uv_rect(&mut self, d: u32, rect: u32, order: u32) {
        let f = self.mem.read_u16(d + 2);
        self.mem.write_u16(d + 2, f | 0x800);
        // (u at x + w, v at y + h) per vertex.
        const ORDERS: [[(bool, bool); 4]; 8] = [
            [(false, false), (true, false), (false, true), (true, true)],
            [(false, false), (false, true), (true, false), (true, true)],
            [(false, true), (false, false), (true, true), (true, false)],
            [(false, true), (true, true), (false, false), (true, false)],
            [(true, false), (false, false), (true, true), (false, true)],
            [(true, false), (true, true), (false, false), (false, true)],
            [(true, true), (false, true), (true, false), (false, false)],
            [(true, true), (true, false), (false, true), (false, false)],
        ];
        let Some(o) = ORDERS.get(order as usize) else { return };
        let (x, y, w, h) = (self.mem.i16(rect), self.mem.i16(rect + 2), self.mem.i16(rect + 4), self.mem.i16(rect + 6));
        for (k, &(cu, cv)) in o.iter().enumerate() {
            let a = d + 0x18 + 0x18 * k as u32;
            self.set_uv(a, if cu { x + w } else { x });
            self.set_uv(a + 2, if cv { y + h } else { y });
        }
    }

    /// 0x105260(desc, g, k): particle `k`'s animation of a particle group
    /// `g` (frame +0x3C, tick +0x3E at g + 16 k): its atlas cell (FST, bit
    /// 0: i16 cell size +0x24 / +0x26, origin +0x28 / +0x2A, sheet +0x2C /
    /// +0x2E; else floats) into the 4 vertices' UVs; then the tick, the
    /// frame (every +0x13C ticks) and past the last frame (+0x13E) a respawn
    /// at the group's origin (+0x10 / +0x14, spread +0x136 / +0x138 with
    /// bit 2) with the frame stepped back by +0x140.
    pub fn uv_frame(&mut self, d: u32, g: u32, k: i32) {
        let flags = self.mem.read_u32(g + 4);
        let p = g.wrapping_add((16 * k) as u32);
        if flags & 1 != 0 {
            if flags & 2 != 0 {
                let (w, sheet_w, frame, ox) = (self.mem.i16(g + 0x24), self.mem.i16(g + 0x2C), self.mem.i16(p + 0x3C), self.mem.i16(g + 0x28));
                let (cols, _) = self.div(sheet_w, w);
                let (_, col) = self.div(frame, cols);
                self.mem.set16(g + 0x20, ox.wrapping_add(w.wrapping_mul(col)));
                let (frame, sheet_w, h, oy, w) = (self.mem.i16(p + 0x3C), self.mem.i16(g + 0x2C), self.mem.i16(g + 0x26), self.mem.i16(g + 0x2A), self.mem.i16(g + 0x24));
                let cols = div1(sheet_w, w).0;
                let row = div1(frame, cols).0;
                self.mem.set16(g + 0x22, oy.wrapping_add(h.wrapping_mul(row)));
            } else {
                let (sheet_h, h, w, frame, ox) = (self.mem.i16(g + 0x2E), self.mem.i16(g + 0x26), self.mem.i16(g + 0x24), self.mem.i16(p + 0x3C), self.mem.i16(g + 0x28));
                let (rows, _) = self.div(sheet_h, h);
                let (c, _) = self.div(frame, rows);
                self.mem.set16(g + 0x20, ox.wrapping_add(w.wrapping_mul(c)));
                let (h, sheet_h, frame, oy) = (self.mem.i16(g + 0x26), self.mem.i16(g + 0x2E), self.mem.i16(p + 0x3C), self.mem.i16(g + 0x2A));
                let rows = div1(sheet_h, h).0;
                let (_, r) = self.div(frame, rows);
                self.mem.set16(g + 0x22, oy.wrapping_add(h.wrapping_mul(r)));
            }
            let f = self.mem.read_u16(d + 2);
            self.mem.write_u16(d + 2, f | 0x800);
            let (u, v, w, h) = (self.mem.i16(g + 0x20), self.mem.i16(g + 0x22), self.mem.i16(g + 0x24), self.mem.i16(g + 0x26));
            self.set_uv(d + 0x18, u);
            self.set_uv(d + 0x1A, v);
            self.set_uv(d + 0x30, u + w - 1);
            self.set_uv(d + 0x32, v);
            self.set_uv(d + 0x48, u);
            self.set_uv(d + 0x4A, v + h - 1);
            self.set_uv(d + 0x60, u + w - 1);
            self.set_uv(d + 0x62, v + h - 1);
        } else {
            if flags & 2 != 0 {
                let f20 = self.mem.f(g + 0x28);
                let frame = self.mem.i16(p + 0x3C);
                let n = ee::fixsfsi(ee::div(1.0, f20)) as i16 as i32;
                let (_, r) = self.div(frame, n);
                self.mem.setf(g + 0x20, ee::mul(f20, ee::from_i32(r)));
                let n2 = ee::fixsfsi(ee::div(1.0, self.mem.f(g + 0x28))) as i16 as i32;
                let frame = self.mem.i16(p + 0x3C);
                let h = self.mem.f(g + 0x2C);
                let (q, _) = self.div(frame, n2);
                self.mem.setf(g + 0x24, ee::mul(h, ee::from_i32(q)));
            } else {
                let frame = self.mem.i16(p + 0x3C);
                let n = ee::fixsfsi(ee::div(1.0, self.mem.f(g + 0x2C))) as i16 as i32;
                let (q, _) = self.div(frame, n);
                let w = self.mem.f(g + 0x28);
                self.mem.setf(g + 0x20, ee::mul(w, ee::from_i32(q)));
                let f20 = self.mem.f(g + 0x2C);
                let frame = self.mem.i16(p + 0x3C);
                let n2 = ee::fixsfsi(ee::div(1.0, f20)) as i16 as i32;
                let (_, r) = self.div(frame, n2);
                self.mem.setf(g + 0x24, ee::mul(f20, ee::from_i32(r)));
            }
            let (s, t, w, h) = (self.mem.f(g + 0x20), self.mem.f(g + 0x24), self.mem.f(g + 0x28), self.mem.f(g + 0x2C));
            self.mem.setf(d + 0x18, s);
            self.mem.setf(d + 0x1C, t);
            self.mem.setf(d + 0x30, ee::add(s, w));
            self.mem.setf(d + 0x34, t);
            self.mem.setf(d + 0x48, s);
            self.mem.setf(d + 0x4C, ee::add(t, h));
            self.mem.setf(d + 0x60, ee::add(s, w));
            self.mem.setf(d + 0x64, ee::add(t, h));
        }
        let tick = self.mem.i16(p + 0x3E).wrapping_add(1);
        self.mem.set16(p + 0x3E, tick);
        if !(self.mem.i16(g + 0x13C) < self.mem.i16(p + 0x3E)) {
            return;
        }
        self.mem.set16(p + 0x3E, 0);
        let frame = self.mem.i16(p + 0x3C).wrapping_add(1);
        self.mem.set16(p + 0x3C, frame);
        if !(self.mem.i16(g + 0x13E) < self.mem.i16(p + 0x3C)) {
            return;
        }
        if self.mem.read_u32(g + 4) & 4 != 0 {
            let ox = self.mem.f(g + 0x10);
            self.mem.setf(p + 0x30, ox);
            let oy = self.mem.f(g + 0x14);
            self.mem.setf(p + 0x34, oy);
            let sx = self.mem.i16(g + 0x136);
            if sx != 0 {
                let r = self.rand();
                let (_, m) = self.div(r, sx.wrapping_mul(2));
                let v = ee::add(self.mem.f(g + 0x10), ee::from_i32(m.wrapping_sub(sx)));
                self.mem.setf(p + 0x30, v);
            }
            let sy = self.mem.i16(g + 0x138);
            if sy != 0 {
                let r = self.rand();
                let (_, m) = self.div(r, sy.wrapping_mul(2));
                let v = ee::add(self.mem.f(g + 0x14), ee::from_i32(m.wrapping_sub(sy)));
                self.mem.setf(p + 0x34, v);
            }
            if self.mem.i16(g + 0x138) < 0 {
                let r = self.rand();
                let d138 = self.mem.i16(g + 0x138);
                let (_, m) = self.div(r, d138);
                let v = ee::add(self.mem.f(g + 0x14), ee::from_i32(m));
                self.mem.setf(p + 0x34, v);
            }
        }
        let back = self.mem.i16(g + 0x140);
        let f = self.mem.i16(p + 0x3C).wrapping_sub(back);
        self.mem.set16(p + 0x3C, f);
    }

    /// c620 0x1E00700 [83b0660ff942: 38 B, 17 C records]: a particle group
    /// `g` (count +0x130, particles at g + 0x30 + 16 k: x, y, z, frame,
    /// tick) moved by `off` (vec3; bit 7: scaled instead), optionally swung
    /// round the origin (+0x10 / +0x14) by the angle +0x132 - +0x134 (bit
    /// 0x1F), each drawn as a textured quad of half size +0xC (aspect from
    /// the cell, bit 0x1E: turned by +0x132 degrees) through `x`+0x30.
    pub fn particle_quads(&mut self, x: u32, g: u32, off: u32) {
        let sp = self.enter(0x160);
        let d = sp + 0xD0;
        self.desc_init(d, 12, 0x34, REGS_TEX4);
        self.alpha_for(d, g + 4, &[(0xC, ALPHA_44), (0xB, ALPHA_88), (0xA, ALPHA_42)]);
        self.colours4(d, g + 8);
        let m = self.read_m4(x + 0x30);
        let xyz = [sp + 0xF0, sp + 0x108, sp + 0x120, sp + 0x138];
        let ws = [sp + 0xB0, sp + 0xB4, sp + 0xB8, sp + 0xBC];
        let v = sp + 0xC0;
        let mut k = 0;
        while k < self.mem.i16(g + 0x130) {
            let p = g.wrapping_add((16 * k) as u32);
            let flags = self.mem.read_u32(g + 4);
            if self.mem.i16(g + 0x134) != self.mem.i16(g + 0x132) {
                if bit(flags, 0x1F) {
                    let (a, b) = (self.mem.i16(g + 0x132), self.mem.i16(g + 0x134));
                    let f20 = ee::sub(self.mem.f(p + 0x30), self.mem.f(g + 0x10));
                    let f21 = ee::sub(self.mem.f(p + 0x34), self.mem.f(g + 0x14));
                    let f22 = ee::from_i32(a.wrapping_sub(b));
                    let f23 = ee::mul(DEG, f22);
                    if bit(self.mem.read_u32(g + 4), 7) {
                        self.mul3(p + 0x30, off);
                        let c = self.cosf(f23);
                        let a1 = ee::mul(f20, c);
                        let s = self.sinf(f23);
                        let b1 = ee::mul(f21, s);
                        let nx = ee::add(self.mem.f(g + 0x10), ee::sub(a1, b1));
                        self.mem.setf(p + 0x30, nx);
                        let s = self.sinf(f23);
                        let a2 = ee::mul(f20, s);
                        let c = self.cosf(f23);
                        let b2 = ee::mul(f21, c);
                        let ny = ee::add(self.mem.f(g + 0x14), ee::add(a2, b2));
                        self.mem.setf(p + 0x34, ny);
                    } else {
                        let c = self.cosf(f23);
                        let a1 = ee::mul(f20, c);
                        let s = self.sinf(f23);
                        let b1 = ee::mul(f21, s);
                        let t = ee::add(self.mem.f(g + 0x10), ee::sub(a1, b1));
                        let nx = ee::add(self.mem.f(off), t);
                        self.mem.setf(p + 0x30, nx);
                        let s = self.sinf(f23);
                        let a2 = ee::mul(f20, s);
                        let c = self.cosf(f23);
                        let b2 = ee::mul(f21, c);
                        let t = ee::add(self.mem.f(g + 0x14), ee::add(a2, b2));
                        let ny = ee::add(self.mem.f(off + 4), t);
                        self.mem.setf(p + 0x34, ny);
                    }
                } else if bit(flags, 7) {
                    self.mul3(p + 0x30, off);
                } else {
                    self.offset_xy(p + 0x30, off);
                }
                if k == self.mem.i16(g + 0x130) - 1 {
                    let a = self.mem.i16(g + 0x132);
                    self.mem.set16(g + 0x134, a);
                }
            } else if bit(flags, 7) {
                self.mul3(p + 0x30, off);
            } else {
                self.offset_xy(p + 0x30, off);
            }
            self.uv_frame(d, g, k);
            let (f20, f21);
            if bit(self.mem.read_u32(g + 4), 0) {
                let (w, h) = (self.mem.i16(g + 0x24), self.mem.i16(g + 0x26));
                if h < w {
                    f21 = ee::div(ee::from_i32(h), ee::from_i32(w));
                    f20 = 1.0;
                } else if w < h {
                    f20 = ee::div(ee::from_i32(w), ee::from_i32(h));
                    f21 = 1.0;
                } else {
                    f20 = 1.0;
                    f21 = 1.0;
                }
            } else {
                let (w, h) = (self.mem.f(g + 0x28), self.mem.f(g + 0x2C));
                if !ee_le(w, h) {
                    f21 = ee::div(h, w);
                    f20 = 1.0;
                } else if ee_lt(w, h) {
                    // As the original: the larger over the smaller here.
                    f20 = ee::div(h, w);
                    f21 = 1.0;
                } else {
                    f20 = 1.0;
                    f21 = 1.0;
                }
            }
            if bit(self.mem.read_u32(g + 4), 0x1E) {
                for (i, a) in [0x87, 0x2D, 0xE1, 0x13B].into_iter().enumerate() {
                    let ang = self.mem.i16(g + 0x132).wrapping_add(a);
                    let (_, r) = self.div(ang, 360);
                    let t = ee::mul(DEG, ee::from_i32(r));
                    let c = self.cosf(t);
                    let vx = ee::add(self.mem.f(p + 0x30), ee::mul(f20, ee::mul(self.mem.f(g + 0xC), c)));
                    self.mem.setf(v, vx);
                    let s = self.sinf(t);
                    let vy = ee::add(self.mem.f(p + 0x34), ee::mul(f21, ee::mul(self.mem.f(g + 0xC), s)));
                    self.mem.setf(v + 4, vy);
                    if i == 0 {
                        let z = self.mem.f(p + 0x38);
                        self.mem.setf(v + 8, z);
                    }
                    if self.project_at(v, &m, xyz[i], ws[i]) {
                        self.flag(g + 4, 0x14 + i as u32);
                    }
                }
            } else {
                for i in 0..4usize {
                    let c = self.mem.f(g + 0xC);
                    let dx = ee::mul(c, f20);
                    let px = self.mem.f(p + 0x30);
                    let vx = if i % 2 == 0 { ee::sub(px, dx) } else { ee::add(px, dx) };
                    self.mem.setf(v, vx);
                    let c = self.mem.f(g + 0xC);
                    let dy = ee::mul(c, f21);
                    let py = self.mem.f(p + 0x34);
                    let vy = if i == 0 || i == 1 { ee::add(py, dy) } else { ee::sub(py, dy) };
                    self.mem.setf(v + 4, vy);
                    if i == 0 {
                        let z = self.mem.f(p + 0x38);
                        self.mem.setf(v + 8, z);
                    }
                    if self.project_at(v, &m, xyz[i], ws[i]) {
                        self.flag(g + 4, 0x14 + i as u32);
                    }
                }
            }
            let rh2 = self.mem.read_u32(g);
            if bit(self.mem.read_u32(g + 4), 0x18) {
                self.desc_texture(d, rh2, 0);
            } else {
                if bit(self.mem.read_u32(g + 4), 0) {
                    self.region_clamp(d, g + 0x20, g + 0x24, g + 0x22, g + 0x26);
                }
                let rh2 = self.mem.read_u32(g);
                self.desc_texture(d, rh2, 1);
            }
            if self.mem.i16(p + 0x3E) >= 0 && self.all_projected(g + 4) {
                if bit(self.mem.read_u32(g + 4), 0x13) {
                    self.far_z(&xyz);
                    self.submit(d, OT, 0);
                } else {
                    let key = self.mean_key4([ws[0], ws[1], ws[2], ws[3]]);
                    self.submit(d, OT, key);
                }
            }
            self.clear_projected(g + 4);
            k += 1;
        }
        if !bit(self.mem.read_u32(g + 4), 7) {
            let x0 = ee::add(self.mem.f(g + 0x10), self.mem.f(off));
            self.mem.setf(g + 0x10, x0);
            let y0 = ee::add(self.mem.f(g + 0x14), self.mem.f(off + 4));
            self.mem.setf(g + 0x14, y0);
        }
        self.leave(0x160);
    }

    /// p.x += off.x, p.y += off.y (the particle step without scaling).
    fn offset_xy(&mut self, p: u32, off: u32) {
        let x = ee::add(self.mem.f(p), self.mem.f(off));
        self.mem.setf(p, x);
        let y = ee::add(self.mem.f(p + 4), self.mem.f(off + 4));
        self.mem.setf(p + 4, y);
    }

    /// c620 0x1E01290 [4bcc6fc4bd32: 65 B, 2 C records]: a disc of +0xC
    /// triangles (i16) of radius +8 at height 0.1 x +0xE (i16) in `x`'s
    /// space, centre colour +4..7 (alpha +7), rim alpha 0; flags at +0.
    pub fn disc(&mut self, g: u32, x: u32) {
        let sp = self.enter(0x140);
        let count = self.mem.i16(g + 0xC);
        let step = ee::div(f32::from_bits(0x40C9_0FDB), ee::from_i32(count));
        let d = sp + 0xE8;
        self.desc_init(d, 6, 0x13, REGS_TRI);
        self.alpha_for(d, g, &[(0xC, ALPHA_44), (0xA, ALPHA_42)]);
        let (c6, c5, c4, c7) = (self.mem.read_u8(g + 6), self.mem.read_u8(g + 5), self.mem.read_u8(g + 4), self.mem.read_u8(g + 7));
        for (off, a) in [(0xF8u32, c7), (0x108, 0), (0x118, 0)] {
            self.mem.write_u8(sp + off, c6);
            self.mem.write_u8(sp + off + 1, c5);
            self.mem.write_u8(sp + off + 2, c4);
            self.mem.write_u8(sp + off + 3, a);
            self.mem.write_f32(sp + off + 4, 1.0);
        }
        self.mem.write_u32(sp + 0xB4, 0);
        self.mem.write_u32(sp + 0xB0, 0);
        let h = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(self.mem.i16(g + 0xE)));
        self.mem.setf(sp + 0xD8, h);
        self.mem.setf(sp + 0xC8, h);
        self.mem.setf(sp + 0xB8, h);
        let m = self.read_m4(x + 0x30);
        let (xyz0, xyz1, xyz2) = (sp + 0x100, sp + 0x110, sp + 0x120);
        let (w0, w1, w2) = (sp + 0x130, sp + 0x134, sp + 0x138);
        let mut k = 0;
        while k < self.mem.i16(g + 0xC) {
            if self.project_at(sp + 0xB0, &m, xyz0, w0) {
                self.flag(g, 0x14);
            }
            let a = ee::mul(step, ee::from_i32(k));
            let c = self.cosf(a);
            let vx = ee::mul(self.mem.f(g + 8), c);
            self.mem.setf(sp + 0xC0, vx);
            let s = self.sinf(a);
            let vy = ee::mul(self.mem.f(g + 8), s);
            self.mem.setf(sp + 0xC4, vy);
            if self.project_at(sp + 0xC0, &m, xyz1, w1) {
                self.flag(g, 0x15);
            }
            if k == self.mem.i16(g + 0xC) - 1 {
                let r = self.mem.f(g + 8);
                self.mem.setf(sp + 0xD0, r);
                self.mem.write_u32(sp + 0xD4, 0);
            } else {
                let a = ee::mul(step, ee::from_i32(k + 1));
                let c = self.cosf(a);
                let vx = ee::mul(self.mem.f(g + 8), c);
                self.mem.setf(sp + 0xD0, vx);
                let s = self.sinf(a);
                let vy = ee::mul(self.mem.f(g + 8), s);
                self.mem.setf(sp + 0xD4, vy);
            }
            if self.project_at(sp + 0xD0, &m, xyz2, w2) {
                self.flag(g, 0x16);
            }
            let flags = self.mem.read_u32(g);
            if bit(flags, 0x14) && bit(flags, 0x15) && bit(flags, 0x16) {
                if bit(flags, 0x13) {
                    self.far_z(&[xyz0, xyz1, xyz2]);
                    self.submit(d, OT, 0);
                    if bit(self.mem.read_u32(g), 0x1C) {
                        self.submit(d, OT, 0);
                    }
                } else if bit(flags, 0x12) {
                    self.submit(d, OT, FAR as i32);
                    if bit(self.mem.read_u32(g), 0x1C) {
                        self.submit(d, OT, FAR as i32);
                    }
                } else {
                    let key = self.mean_key3([w0, w1, w2]);
                    self.submit(d, OT, key);
                    if bit(self.mem.read_u32(g), 0x1C) {
                        let key = self.mean_key3([w0, w1, w2]);
                        self.submit(d, OT, key);
                    }
                }
            }
            let v = self.mem.read_u32(g);
            self.mem.write_u32(g, v & 0xFF8F_FFFF);
            k += 1;
        }
        self.leave(0x140);
    }

    /// One particle's respawn of the sprite routines (c620 0x1E017F0's
    /// body): a random direction at radius +0x4C, height rand % 20 (bit
    /// 0x1D: the direction in the vertical plane).
    fn respawn(&mut self, g: u32, off: u32) {
        let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
        if bit(self.mem.read_u32(g + 4), 0x1D) {
            let a = self.rand() & 0xFFFF;
            let s = self.tsin(a);
            let y = ee::mul(self.mem.f(g + 0x4C), s);
            self.mem.setf(p + 4, y);
            let r = self.rand();
            let (_, h) = self.div(r, 20);
            let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
            self.mem.setf(p + 8, ee::from_i32(h));
            let c = self.tcos(a);
            let x = ee::mul(self.mem.f(g + 0x4C), c);
            let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
            self.mem.setf(p, x);
        } else {
            let a = self.rand() & 0xFFFF;
            let r = self.rand();
            let (_, h) = self.div(r, 20);
            self.mem.setf(p + 4, ee::from_i32(h));
            let c = self.tcos(a);
            let z = ee::mul(self.mem.f(g + 0x4C), c);
            let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
            self.mem.setf(p + 8, z);
            let s = self.tsin(a);
            let x = ee::mul(self.mem.f(g + 0x4C), s);
            let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
            self.mem.setf(p, x);
        }
    }

    /// c620 0x1E017F0 [cbde567e8a8d: 20 B, 2 C records]: the sprite group's
    /// size +0xC = `size` / 2 and every particle (+0x5A) respawned.
    pub fn sprites_spawn(&mut self, g: u32, size: f32) {
        self.enter(0x50);
        let half = ee::div(size, 2.0);
        self.mem.setf(g + 0xC, half);
        let mut k = 0;
        while k < self.mem.i16(g + 0x5A) {
            self.respawn(g, (16 * k) as u32);
            k += 1;
        }
        self.leave(0x50);
    }

    /// c620 0x1E01980 [e853353d6443: 65 B, 5 C records]: the sprite group
    /// `g` (particles at *(g+0x40): x y z, alpha +0xC, frame +0xD, age
    /// +0xE; count +0x5A): screen-aligned sprites (or quads turned by the
    /// per-particle angles *(g+0x44), bit 0x1F) of half size from +0xC and
    /// the cell's aspect, growing by +0x50 per age (bit 0x1B), bobbing by
    /// +0x54 / +0x58 (bit 0x1A), atlas frames (bits 0, 1, 3), the whole
    /// group turned by the angles +0x20 first (bit 0x1C); each particle
    /// ages, steps its frame every +0x5E updates over +0x5C frames and
    /// respawns (bit 2).
    pub fn sprites(&mut self, g: u32, x: u32) {
        let sp = self.enter(0x330);
        let da = sp + 0x228;
        let db = sp + 0x2A0;
        self.desc_init(da, 9, 0x24, REGS_TEX4_FLAT);
        self.mem.write_u32(sp + 0x140, da + 2);
        self.desc_init(db, 12, 0x34, REGS_TEX4);
        self.mem.write_u32(sp + 0x150, db + 2);
        if self.mem.i16(g + 0x5E) == 0 {
            self.mem.set16(g + 0x5E, 1);
        }
        let (w, h) = (self.mem.i16(g + 0x34), self.mem.i16(g + 0x36));
        let (f22, f23);
        if w == h {
            f23 = self.mem.f(g + 0xC);
            f22 = f23;
        } else if h < w {
            f22 = self.mem.f(g + 0xC);
            f23 = ee::mul(f22, ee::div(ee::from_i32(h), ee::from_i32(w)));
        } else {
            f23 = self.mem.f(g + 0xC);
            f22 = ee::mul(f23, ee::div(ee::from_i32(w), ee::from_i32(h)));
        }
        let order = [(0xC, ALPHA_44), (0xB, ALPHA_88), (0xA, ALPHA_42)];
        self.alpha_for(da, g + 4, &order);
        self.alpha_for(db, g + 4, &order);
        let (ca, c9, c8, cb) = (self.mem.read_u8(g + 0xA), self.mem.read_u8(g + 9), self.mem.read_u8(g + 8), self.mem.read_u8(g + 0xB));
        self.mem.write_u8(sp + 0x238, ca);
        self.mem.write_u8(sp + 0x239, c9);
        self.mem.write_u8(sp + 0x23A, c8);
        self.mem.write_u8(sp + 0x23B, cb);
        self.mem.write_f32(sp + 0x23C, 1.0);
        self.colours4(db, g + 8);
        let uv_a = [sp + 0x240, sp + 0x250, sp + 0x260, sp + 0x270];
        let uv_b = [sp + 0x2B8, sp + 0x2D0, sp + 0x2E8, sp + 0x300];
        if bit(self.mem.read_u32(g + 4), 3) {
            self.sprite_uvs(g, da, db, &uv_a, &uv_b, None);
        }
        if bit(self.mem.read_u32(g + 4), 0x1C) {
            let rot = [self.mem.i32(g + 0x20), self.mem.i32(g + 0x24), self.mem.i32(g + 0x28)];
            let r = self.rotation_matrix(rot);
            self.write_m4(sp + 0x170, &r);
            let mut k = 0;
            while k < self.mem.i16(g + 0x5A) {
                let p = self.mem.read_u32(g + 0x40).wrapping_add((16 * k) as u32);
                let v = [self.mem.f(p), self.mem.f(p + 4), self.mem.f(p + 8)];
                self.mem.setf(sp + 0x200, v[0]);
                self.mem.setf(sp + 0x204, v[1]);
                self.mem.setf(sp + 0x208, v[2]);
                let vin = self.read_v4(sp + 0x200);
                let o = self.transform(Some(&r), vin);
                self.write_v4(sp + 0x1F0, o);
                let p = self.mem.read_u32(g + 0x40).wrapping_add((16 * k) as u32);
                self.mem.setf(p, o[0]);
                self.mem.setf(p + 4, o[1]);
                self.mem.setf(p + 8, o[2]);
                k += 1;
            }
        }
        let f20 = ee::neg(f22);
        let f21 = ee::neg(f23);
        let m = self.read_m4(x + 0x30);
        let (s5, s7, fp) = (sp + 0x248, sp + 0x278, sp + 0x268);
        let mut k = 0;
        while k < self.mem.i16(g + 0x5A) {
            let off = (16 * k) as u32;
            let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
            let a = self.mem.read_u8(p + 0xC);
            let cap = self.mem.read_u8(g + 0xB);
            let alpha = if a < cap { a } else { cap };
            for at in [0x2FB, 0x2E3, 0x2CB, 0x2B3, 0x23B] {
                self.mem.write_u8(sp + at, alpha);
            }
            if !bit(self.mem.read_u32(g + 4), 3) {
                self.sprite_uvs(g, da, db, &uv_a, &uv_b, Some(off));
            }
            let rh2 = self.mem.read_u32(g);
            if bit(self.mem.read_u32(g + 4), 0x18) {
                self.desc_texture(da, rh2, 0);
                self.desc_texture(db, rh2, 0);
            } else {
                if bit(self.mem.read_u32(g + 4), 0) {
                    self.region_clamp(da, g + 0x30, g + 0x34, g + 0x32, g + 0x36);
                    self.region_clamp(db, g + 0x30, g + 0x34, g + 0x32, g + 0x36);
                }
                let rh2 = self.mem.read_u32(g);
                self.desc_texture(da, rh2, 1);
                self.desc_texture(db, rh2, 1);
            }
            let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
            let pos = [self.mem.f(p), self.mem.f(p + 4), self.mem.f(p + 8)];
            self.mem.setf(sp + 0x210, pos[0]);
            self.mem.setf(sp + 0x214, pos[1]);
            self.mem.setf(sp + 0x218, pos[2]);
            if bit(self.mem.read_u32(g + 4), 0x1A) {
                let age = self.mem.i16(p + 0xE);
                let freq = self.mem.i16(g + 0x58);
                let s = self.tsin(freq.wrapping_mul(age));
                let z = ee::add(self.mem.f(sp + 0x218), ee::neg(ee::mul(self.mem.f(g + 0x54), s)));
                self.mem.setf(sp + 0x218, z);
            }
            if bit(self.mem.read_u32(g + 4), 0x1F) {
                let ra = self.mem.read_u32(g + 0x44).wrapping_add(off);
                let rot = [self.mem.i32(ra), self.mem.i32(ra + 4), self.mem.i32(ra + 8)];
                let r = self.rotation_matrix(rot);
                self.write_m4(sp + 0x170, &r);
                let corners = if bit(self.mem.read_u32(g + 4), 0x1B) {
                    let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
                    let grow = self.mem.f(g + 0x50);
                    let age = ee::from_i32(self.mem.i16(p + 0xE));
                    let f24 = ee::add(f22, ee::mul(ee::mul(f22, grow), age));
                    let f25 = ee::add(f23, ee::mul(ee::mul(f23, grow), age));
                    let (f27, f26) = (ee::neg(f24), ee::neg(f25));
                    [(f27, f26), (f24, f26), (f27, f25), (f24, f25)]
                } else {
                    [(f20, f21), (f22, f21), (f20, f23), (f22, f23)]
                };
                for (i, (cx, cz)) in corners.into_iter().enumerate() {
                    self.mem.setf(sp + 0x1F0, cx);
                    self.mem.write_u32(sp + 0x1F4, 0);
                    self.mem.setf(sp + 0x1F8, cz);
                    let vin = self.read_v4(sp + 0x1F0);
                    let o = self.transform(Some(&r), vin);
                    let at = sp + 0x1B0 + 16 * i as u32;
                    self.write_v4(at, o);
                    self.add3(at, sp + 0x210);
                }
                if self.project_desc(sp + 0x1B0, 0x7000_0000, Some(&m), db, sp + 0x160) {
                    let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
                    if self.mem.i16(p + 0xE) >= 0 {
                        let key = ee::fixunssfsi(self.mem.f(sp + 0x160)) as i32;
                        self.submit(db, OT, key);
                    }
                }
            } else {
                if self.project_at(sp + 0x210, &m, s5, sp + 0x160) && Self::on_screen(self.mem.read_u64(s5)) {
                    let v = self.mem.read_u32(g + 4);
                    self.mem.write_u32(g + 4, v | PROJECTED);
                }
                let w = self.mem.f(sp + 0x160);
                self.mem.setf(sp + 0x16C, w);
                self.mem.setf(sp + 0x168, w);
                self.mem.setf(sp + 0x164, w);
                let w = self.mem.f(sp + 0x160);
                let mut f24 = ee::mul(20.0, self.view_scale(f22, w));
                let w = self.mem.f(sp + 0x160);
                let mut f25 = ee::mul(20.0, self.view_scale(f23, w));
                if bit(self.mem.read_u32(g + 4), 0x1B) {
                    let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
                    let grow = self.mem.f(g + 0x50);
                    let age = ee::from_i32(self.mem.i16(p + 0xE));
                    f24 = ee::add(f24, ee::mul(ee::mul(f22, grow), age));
                    f25 = ee::add(f25, ee::mul(ee::mul(f23, grow), age));
                }
                let s3 = sp + 0x24A;
                let cx = |fx: &mut Fx| ee::from_i32(i32::from(fx.mem.read_u16(s5)));
                let cy = |fx: &mut Fx| ee::from_i32(i32::from(fx.mem.read_u16(s3)));
                let v = cx(self);
                self.mem.write_u16(sp + 0x258, ee::fixunssfsi(ee::add(v, f24)) as u16);
                let v = cy(self);
                self.mem.write_u16(sp + 0x25A, ee::fixunssfsi(ee::sub(v, f25)) as u16);
                let v = cx(self);
                self.mem.write_u16(fp, ee::fixunssfsi(ee::sub(v, f24)) as u16);
                let v = cy(self);
                self.mem.write_u16(sp + 0x26A, ee::fixunssfsi(ee::add(v, f25)) as u16);
                let v = cx(self);
                self.mem.write_u16(s7, ee::fixunssfsi(ee::add(v, f24)) as u16);
                let v = cy(self);
                self.mem.write_u16(sp + 0x27A, ee::fixunssfsi(ee::add(v, f25)) as u16);
                let v = cx(self);
                self.mem.write_u16(s5, ee::fixunssfsi(ee::sub(v, f24)) as u16);
                let v = cy(self);
                self.mem.write_u16(s3, ee::fixunssfsi(ee::sub(v, f25)) as u16);
                let z = self.xyz_z(s5);
                self.set_xyz_z(s7, z);
                let z = self.xyz_z(s7);
                self.set_xyz_z(fp, z);
                let z = self.xyz_z(fp);
                self.set_xyz_z(sp + 0x258, z);
                if self.all_projected(g + 4) {
                    if bit(self.mem.read_u32(g + 4), 0x13) {
                        self.far_z(&[s5, sp + 0x258, fp, s7]);
                        let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
                        if self.mem.i16(p + 0xE) >= 0 {
                            self.submit(da, OT, 0);
                        }
                    } else {
                        let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
                        if self.mem.i16(p + 0xE) >= 0 {
                            let key = self.mean_key4([sp + 0x160, sp + 0x164, sp + 0x168, sp + 0x16C]);
                            self.submit(da, OT, key);
                        }
                    }
                }
                self.clear_projected(g + 4);
            }
            if !bit(self.mem.read_u32(g + 4), 3) {
                let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
                let age = self.mem.i16(p + 0xE).wrapping_add(1);
                self.mem.set16(p + 0xE, age);
                let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
                let period = self.mem.i16(g + 0x5E);
                let age = self.mem.i16(p + 0xE);
                let (_, r) = self.div(age, period);
                if r == 0 {
                    let f = self.mem.read_u8(p + 0xD).wrapping_add(1);
                    self.mem.write_u8(p + 0xD, f);
                    let p = self.mem.read_u32(g + 0x40).wrapping_add(off);
                    if self.mem.i16(g + 0x5C) == i32::from(self.mem.read_u8(p + 0xD)) {
                        self.mem.write_u8(p + 0xD, 0);
                        if bit(self.mem.read_u32(g + 4), 2) {
                            self.respawn(g, off);
                        }
                    }
                }
            }
            k += 1;
        }
        self.leave(0x330);
    }

    /// The UVs of the two sprite descriptors from the frame (bit 0 FST
    /// cells: origin +0x38 / +0x3A, size +0x34 / +0x36, sheet +0x3C / +0x3E;
    /// else float), `frame`: computed from the particle's frame (else the
    /// static rectangle +0x30..).
    fn sprite_uvs(&mut self, g: u32, da: u32, db: u32, uv_a: &[u32; 4], uv_b: &[u32; 4], particle: Option<u32>) {
        let flags = self.mem.read_u32(g + 4);
        if bit(flags, 0) {
            if let Some(off) = particle {
                let pa = self.mem.read_u32(g + 0x40).wrapping_add(off);
                if bit(flags, 1) {
                    let (w, sheet, f, ox) = (self.mem.i16(g + 0x34), self.mem.i16(g + 0x3C), i32::from(self.mem.read_u8(pa + 0xD)), self.mem.i16(g + 0x38));
                    let (cols, _) = self.div(sheet, w);
                    let (_, col) = self.div(f, cols);
                    self.mem.set16(g + 0x30, ox.wrapping_add(w.wrapping_mul(col)));
                    let (sheet, w, h, oy) = (self.mem.i16(g + 0x3C), self.mem.i16(g + 0x34), self.mem.i16(g + 0x36), self.mem.i16(g + 0x3A));
                    let pa = self.mem.read_u32(g + 0x40).wrapping_add(off);
                    let f = i32::from(self.mem.read_u8(pa + 0xD));
                    let cols = div1(sheet, w).0;
                    let row = div1(f, cols).0;
                    self.mem.set16(g + 0x32, oy.wrapping_add(h.wrapping_mul(row)));
                } else {
                    let (sheet, h, f, w, ox) = (self.mem.i16(g + 0x3E), self.mem.i16(g + 0x36), i32::from(self.mem.read_u8(pa + 0xD)), self.mem.i16(g + 0x34), self.mem.i16(g + 0x38));
                    let (rows, _) = self.div(sheet, h);
                    let (c, _) = self.div(f, rows);
                    self.mem.set16(g + 0x30, ox.wrapping_add(w.wrapping_mul(c)));
                    let (h, sheet, oy) = (self.mem.i16(g + 0x36), self.mem.i16(g + 0x3E), self.mem.i16(g + 0x3A));
                    let pa = self.mem.read_u32(g + 0x40).wrapping_add(off);
                    let f = i32::from(self.mem.read_u8(pa + 0xD));
                    let rows = div1(sheet, h).0;
                    let (_, r) = self.div(f, rows);
                    self.mem.set16(g + 0x32, oy.wrapping_add(h.wrapping_mul(r)));
                }
            }
            for (d, uv) in [(da, uv_a), (db, uv_b)] {
                let f = self.mem.read_u16(d + 2);
                self.mem.write_u16(d + 2, f | 0x800);
                let (u, v, w, h) = (self.mem.i16(g + 0x30), self.mem.i16(g + 0x32), self.mem.i16(g + 0x34), self.mem.i16(g + 0x36));
                for (k, &a) in uv.iter().enumerate() {
                    self.set_uv(a, if k % 2 == 1 { u + w } else { u });
                    self.set_uv(a + 2, if k >= 2 { v + h } else { v });
                }
            }
        } else {
            if let Some(off) = particle {
                let pa = self.mem.read_u32(g + 0x40).wrapping_add(off);
                if bit(flags, 1) {
                    let f24 = self.mem.f(g + 0x38);
                    let f = i32::from(self.mem.read_u8(pa + 0xD));
                    let n = ee::fixsfsi(ee::div(1.0, f24)) as i16 as i32;
                    let (_, r) = self.div(f, n);
                    self.mem.setf(g + 0x30, ee::mul(f24, ee::from_i32(r)));
                    let pa = self.mem.read_u32(g + 0x40).wrapping_add(off);
                    let f = i32::from(self.mem.read_u8(pa + 0xD));
                    let n2 = ee::fixsfsi(ee::div(1.0, self.mem.f(g + 0x38))) as i16 as i32;
                    let h = self.mem.f(g + 0x3C);
                    let (q, _) = self.div(f, n2);
                    self.mem.setf(g + 0x34, ee::mul(h, ee::from_i32(q)));
                } else {
                    let f = i32::from(self.mem.read_u8(pa + 0xD));
                    let n = ee::fixsfsi(ee::div(1.0, self.mem.f(g + 0x3C))) as i16 as i32;
                    let w = self.mem.f(g + 0x38);
                    let (q, _) = self.div(f, n);
                    self.mem.setf(g + 0x30, ee::mul(w, ee::from_i32(q)));
                    let f24 = self.mem.f(g + 0x3C);
                    let pa = self.mem.read_u32(g + 0x40).wrapping_add(off);
                    let f = i32::from(self.mem.read_u8(pa + 0xD));
                    let n2 = ee::fixsfsi(ee::div(1.0, f24)) as i16 as i32;
                    let (_, r) = self.div(f, n2);
                    self.mem.setf(g + 0x34, ee::mul(f24, ee::from_i32(r)));
                }
            }
            for uv in [uv_a, uv_b] {
                let (s, t) = (self.mem.f(g + 0x30), self.mem.f(g + 0x34));
                let sw = ee::add(self.mem.f(g + 0x30), self.mem.f(g + 0x38));
                let th = ee::add(self.mem.f(g + 0x34), self.mem.f(g + 0x3C));
                self.mem.setf(uv[0], s);
                self.mem.setf(uv[0] + 4, t);
                self.mem.setf(uv[1], sw);
                self.mem.setf(uv[1] + 4, t);
                self.mem.setf(uv[2], s);
                self.mem.setf(uv[2] + 4, th);
                self.mem.setf(uv[3], sw);
                self.mem.setf(uv[3] + 4, th);
            }
        }
    }

    /// c620 0x1E03270 [acd3c3277df2: 50 B, 23 C records]: a ring group of
    /// 30 segments (+0x40 + 16 k inner, +0x220 + 16 k outer) of radii
    /// `inner` / `outer` at height `h0` with width `w` (bit 0xF: random
    /// heights; bit 0x1D: in the vertical plane), texture `rh2`.
    pub fn ring_setup(&mut self, g: u32, rh2: u32, inner: f32, outer: f32, h0: f32, w: f32) {
        self.enter(0x70);
        self.mem.write_u32(g, rh2);
        self.mem.setf(g + 0xC, w);
        self.mem.setf(g + 0x20, inner);
        self.mem.setf(g + 0x30, outer);
        let vertical = bit(self.mem.read_u32(g + 4), 0x1D);
        let top = ee::add(h0, w);
        let f20 = ee::mul(2.0, w);
        if vertical {
            self.mem.write_u32(g + 0x24, 0);
            self.mem.setf(g + 0x28, h0);
            self.mem.write_u32(g + 0x34, 0);
            self.mem.setf(g + 0x38, top);
        } else {
            self.mem.setf(g + 0x24, h0);
            self.mem.write_u32(g + 0x28, 0);
            self.mem.setf(g + 0x34, top);
            self.mem.write_u32(g + 0x38, 0);
        }
        for k in 0..30u32 {
            let s = g + 16 * k;
            let a = ee::mul(f32::from_bits(0x3E56_7750), ee::from_i32(k as i32 + 1));
            let c = self.cosf(a);
            self.mem.setf(s + 0x40, ee::mul(inner, c));
            let si = self.sinf(a);
            if vertical {
                self.mem.setf(s + 0x44, ee::mul(inner, si));
                self.mem.setf(s + 0x48, h0);
            } else {
                self.mem.setf(s + 0x48, ee::mul(inner, si));
                self.mem.setf(s + 0x44, h0);
            }
            let c = self.cosf(a);
            self.mem.setf(s + 0x220, ee::mul(outer, c));
            let si = self.sinf(a);
            let (hs, ts) = if vertical { (0x224, 0x228) } else { (0x228, 0x224) };
            self.mem.setf(s + hs, ee::mul(outer, si));
            let t = if bit(self.mem.read_u32(g + 4), 0xF) {
                let r = self.rand();
                let n = ee::fixsfsi(f20);
                let (_, m) = self.div(r, n);
                let n2 = ee::fixsfsi(w);
                ee::add(top, ee::from_i32(m.wrapping_sub(n2) >> 1))
            } else {
                top
            };
            self.mem.setf(s + ts, t);
        }
        self.leave(0x70);
    }

    /// c620 0x1E03530 [32e108615b51: 49 B, 23 C records]: the ring group
    /// drawn as 30 textured quads (UVs from the rectangle +0x10, order 2).
    pub fn ring_draw(&mut self, g: u32, x: u32) {
        let sp = self.enter(0x140);
        let d = sp + 0xB0;
        self.desc_init(d, 12, 0x34, REGS_TEX4);
        self.alpha_for(d, g + 4, &[(0xB, ALPHA_88), (0xA, ALPHA_42)]);
        let m = self.read_m4(x + 0x30);
        let xyz = [sp + 0xD0, sp + 0xE8, sp + 0x100, sp + 0x118];
        let ws = [sp + 0xA0, sp + 0xA4, sp + 0xA8, sp + 0xAC];
        for k in 0..30u32 {
            self.colours4(d, g + 8);
            self.uv_rect(d, g + 0x10, 2);
            let (a, b) = if k == 0 { (g + 0x20, g + 0x30) } else { (g + 16 * (k - 1) + 0x40, g + 16 * (k - 1) + 0x220) };
            if self.project_at(a, &m, xyz[0], ws[0]) {
                self.flag(g + 4, 0x14);
            }
            if self.project_at(b, &m, xyz[1], ws[1]) {
                self.flag(g + 4, 0x15);
            }
            let (a, b) = if k == 29 { (g + 0x20, g + 0x30) } else { (g + 16 * k + 0x40, g + 16 * k + 0x220) };
            if self.project_at(a, &m, xyz[2], ws[2]) {
                self.flag(g + 4, 0x16);
            }
            if self.project_at(b, &m, xyz[3], ws[3]) {
                self.flag(g + 4, 0x17);
            }
            self.region_clamp(d, g + 0x10, g + 0x14, g + 0x12, g + 0x16);
            let rh2 = self.mem.read_u32(g);
            self.desc_texture(d, rh2, 1);
            if self.all_projected(g + 4) {
                if bit(self.mem.read_u32(g + 4), 0x13) {
                    self.far_z(&xyz);
                    self.submit(d, OT, 0);
                } else {
                    let key = self.mean_key4(ws);
                    self.submit(d, OT, key);
                }
            }
            self.clear_projected(g + 4);
        }
        self.leave(0x140);
    }

    /// c620 0x1E03A20 [6b34d77bd8ac: 49 B, 19 C records]: a quad group:
    /// size `size` (+0xC), texture `rh2`, the 4 corners (the record's
    /// constant template x size, at +0x20..), random angles for its 16
    /// (bit 0x1F) or 64 quads (+0x60 + 16 k).
    pub fn quads_setup(&mut self, g: u32, rh2: u32, size: f32, template: u32) {
        let sp = self.enter(0x90);
        for k in 0..4u32 {
            let v = self.read_v4(template + 16 * k);
            self.write_v4(sp + 0x50 + 16 * k, v);
        }
        let n = if bit(self.mem.read_u32(g + 4), 0x1F) { 0x10 } else { 0x40 };
        self.mem.setf(g + 0xC, size);
        self.mem.write_u32(g, rh2);
        for k in 0..4u32 {
            let t = sp + 0x50 + 16 * k;
            for c in 0..3u32 {
                let v = ee::mul(self.mem.f(g + 0xC), self.mem.f(t + 4 * c));
                self.mem.setf(g + 0x20 + 16 * k + 4 * c, v);
            }
        }
        for k in 0..n {
            let s = g + 16 * k;
            for c in 0..3u32 {
                let r = self.rand() & 0xFFFF;
                self.mem.write_u32(s + 0x60 + 4 * c, r as u32);
            }
        }
        self.leave(0x90);
    }

    /// c620 0x1E03B80 [08386c4af2a1: 49 B, 19 C records]: the quad group
    /// drawn: each of its 16 / 64 quads turned by its angles (+0x60 + 16 k).
    pub fn quads_draw(&mut self, g: u32, x: u32) {
        let sp = self.enter(0x1A0);
        let n = if bit(self.mem.read_u32(g + 4), 0x1F) { 0x10 } else { 0x40 };
        self.mem.write_u32(sp + 0xA0, n);
        let d = sp + 0x110;
        self.desc_init(d, 12, 0x34, REGS_TEX4);
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
            let rot = [self.mem.i32(s + 0x60), self.mem.i32(s + 0x64), self.mem.i32(s + 0x68)];
            let r = self.rotation_matrix(rot);
            self.write_m4(sp + 0xC0, &r);
            for i in 0..4usize {
                let c = self.read_v4(g + 0x20 + 16 * i as u32);
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
                    self.submit(d, OT, FAR as i32);
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

/// div1 (pipeline 1): quotient and remainder (by zero 0, 0: never reached
/// by the ported routines' data).
fn div1(a: i32, b: i32) -> (i32, i32) {
    if b == 0 {
        (0, 0)
    } else {
        (a.wrapping_div(b), a.wrapping_rem(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::{Image, Mem};
    use std::sync::Arc;

    fn fx() -> Fx {
        let mut f = Fx::new(Arc::new(Image::default()), &[0u8; 0x80], 0, [0x101, 1], &Default::default());
        f.mem = Mem::new(Arc::new(Image::default()));
        f
    }

    /// 0x104540's 8 orders each use every rectangle corner once (FST,
    /// 12.4 in 14 bits).
    #[test]
    fn uv_rect_orders_are_corner_permutations() {
        for order in 0..8 {
            let mut f = fx();
            let (d, r) = (0x0100_0000, 0x0100_1000);
            for (k, v) in [3i32, 5, 10, 20].into_iter().enumerate() {
                f.mem.set16(r + 2 * k as u32, v);
            }
            f.uv_rect(d, r, order);
            assert_eq!(f.mem.read_u16(d + 2) & 0x800, 0x800);
            let mut corners: Vec<(u16, u16)> = (0..4).map(|k| (f.mem.read_u16(d + 0x18 + 0x18 * k) >> 4, f.mem.read_u16(d + 0x1A + 0x18 * k) >> 4)).collect();
            corners.sort();
            assert_eq!(corners, vec![(3, 5), (3, 25), (13, 5), (13, 25)], "order {order}");
        }
    }

    /// The far-Z chain sets every vertex's z and keeps X / Y / F.
    #[test]
    fn far_z_keeps_xy_and_f() {
        let mut f = fx();
        let a = [0x0100_0000u32, 0x0100_0008];
        f.mem.write_u64(a[0], 0xAB00_0012_3456_7890);
        f.mem.write_u64(a[1], 0xCD00_0000_1111_2222);
        f.far_z(&a);
        assert_eq!(f.mem.read_u64(a[0]), 0xABFF_FFFF_3456_7890);
        assert_eq!(f.mem.read_u64(a[1]), 0xCDFF_FFFF_1111_2222);
    }

    /// The soft-float conversions truncate and saturate as 0x1A2C70 /
    /// 0x1A2B80; newlib's rand steps its LCG.
    #[test]
    fn conversions_and_rand() {
        assert_eq!(ee::fixunssfsi(1.999), 1);
        assert_eq!(ee::fixunssfsi(-3.0), 0);
        assert_eq!(ee::fixunssfsi(5.0e9), u32::MAX);
        assert_eq!(ee::fixsfsi(-2.75), -2);
        assert_eq!(ee::fixsfsi(3.0e9), i32::MAX);
        let mut r = ee::Rand(1);
        let a = r.next();
        assert_eq!(r.0, 0x5851_F42D_4C95_7F2Eu64);
        assert_eq!(a, (0x5851_F42Du32 & 0x7FFF_FFFF) as i32);
    }
}
