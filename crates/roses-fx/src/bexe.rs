//! More of the executable's helpers the BMODULE records call, hand-ported
//! (helper C, 30 Oct 2026): random directions, unit points.
use crate::bus::Bus;
use crate::exe::Fx;

/// The base vector 0x1043F0 turns (lq 0x28B4A0, read from the user's
/// executable).
const RANDOM_DIR_BASE: u32 = 0x0028_B4A0;
/// The object table 0x2DE2C0 (0x54 entries).
const OBJECTS: u32 = crate::world::OBJECT_TABLE;

impl Fx {
    /// 0x1043F0(out, length) [F]: the vector 0x28B4A0 with y = `length`
    /// turned by the rotation (rand & 0xFFFF, rand & 0xFFFF, 0) (0x20BDA0,
    /// then 0x20C540 into `out`); locals at their stack addresses (v +0x60,
    /// angles +0x70, matrix +0x20).
    pub fn random_dir(&mut self, out: u32, length: f32) {
        let sp = self.enter(0x80);
        for k in 0..4u32 {
            let w = self.mem.read_u32(RANDOM_DIR_BASE + 4 * k);
            self.mem.write_u32(sp + 0x60 + 4 * k, w);
        }
        self.mem.write_f32(sp + 0x64, length);
        let a = (self.rand() & 0xFFFF) as u32;
        self.mem.write_u32(sp + 0x70, a);
        let b = (self.rand() & 0xFFFF) as u32;
        self.mem.write_u32(sp + 0x74, b);
        self.mem.write_u32(sp + 0x78, 0);
        let rot = [a as i32, b as i32, 0];
        let m = self.rotation_matrix(rot);
        self.write_m4(sp + 0x20, &m);
        let v = self.read_v4(sp + 0x60);
        let m = self.read_m4(sp + 0x20);
        let o = self.transform(Some(&m), v);
        self.write_v4(out, o);
        self.leave(0x80);
    }

    /// 0x1036E0(a, b, out) [F]: out = a + (b - a) / 2 (xyz; out's w kept).
    pub fn midpoint(&mut self, a: u32, b: u32, out: u32) {
        let d: [f32; 3] = std::array::from_fn(|k| crate::ee::div(crate::ee::sub(self.mem.read_f32(b + 4 * k as u32), self.mem.read_f32(a + 4 * k as u32)), 2.0));
        for k in 0..3u32 {
            let v = self.mem.read_f32(a + 4 * k);
            self.mem.write_f32(out + 4 * k, v);
        }
        for k in 0..3u32 {
            let v = crate::ee::add(self.mem.read_f32(out + 4 * k), d[k as usize]);
            self.mem.write_f32(out + 4 * k, v);
        }
    }

    /// 0x104490(a, b, n) [F]: `a` moves 1 / n of the way to `b` (xyz);
    /// n = 0: `a` = `b`.
    pub fn approach_by(&mut self, a: u32, b: u32, n: i32) {
        if n == 0 {
            for k in 0..3u32 {
                let v = self.mem.read_u32(b + 4 * k);
                self.mem.write_u32(a + 4 * k, v);
            }
            return;
        }
        let f = crate::ee::from_i32(n);
        let d: [f32; 3] = std::array::from_fn(|k| crate::ee::div(crate::ee::sub(self.mem.read_f32(b + 4 * k as u32), self.mem.read_f32(a + 4 * k as u32)), f));
        for k in 0..3u32 {
            let v = crate::ee::add(self.mem.read_f32(a + 4 * k), d[k as usize]);
            self.mem.write_f32(a + 4 * k, v);
        }
    }

    /// 0x1F65D0(kind, handle, out) [F]: `out` = (0, 0, 0, 1), then for a
    /// valid handle (side 0..2, slot 0..10) with a unit object: kind 1 the
    /// unit's animated position obj+0x80 (0x1F3260: when obj+0x1F2 >= 0,
    /// obj+0x1FE > 0 and obj+0x20C bit 5), kind 2 object 8's position
    /// (0x1F31E0(8, out)). Kind 0 (the card's cell, 0x1E4DD0) is not ported
    /// (no ported record asks for it): it answers 0.
    pub fn unit_point(&mut self, kind: i32, handle: i32, out: u32) -> i32 {
        self.mem.write_u32(out + 8, 0);
        self.mem.write_u32(out + 4, 0);
        self.mem.write_u32(out, 0);
        self.mem.write_f32(out + 0xC, 1.0);
        let (side, slot) = (handle >> 8, handle & 0xFF);
        if !(0..3).contains(&side) || slot >= 0xB {
            return 0;
        }
        let i = self.object_index(side, slot);
        if i < 0 {
            return 0;
        }
        match kind {
            1 => self.object_point(i, 0x80, true, out),
            2 => self.object_point(8, 0, false, out),
            0 => {
                debug_assert!(false, "0x1F65D0 kind 0 (0x1E4DD0) is not ported");
                0
            }
            _ => 0,
        }
    }

    /// 0x1F3260 (`checked`, the vector at +0x80) / 0x1F31E0 (the vector at
    /// +0) of object `i`.
    fn object_point(&mut self, i: i32, at: u32, checked: bool, out: u32) -> i32 {
        if !(0..0x54).contains(&i) {
            return 0;
        }
        let o = self.mem.read_u32(OBJECTS + 4 * i as u32);
        if o == 0 {
            return 0;
        }
        if checked && (self.mem.i8(o + 0x1F2) < 0 || self.mem.read_u8(o + 0x1FE) == 0 || self.mem.read_u8(o + 0x20C) & 0x20 == 0) {
            return 0;
        }
        if out != 0 {
            for k in 0..4u32 {
                let w = self.mem.read_u32(o + at + 4 * k);
                self.mem.write_u32(out + 4 * k, w);
            }
        }
        1
    }
}
