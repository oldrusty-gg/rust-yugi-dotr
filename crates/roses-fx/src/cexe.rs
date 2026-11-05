//! Executable helpers the CMODULE ports c401 and c061..c063 call (helper
//! A, 30 Oct 2026), hand-ported: the effect-library rotations 0x100D50 /
//! 0x100E00 / 0x100E40 / 0x101020, the VU helpers 0x1883D8 / 0x188420, the
//! identity 0x20B320, 0x20C300, the alpha presets 0x208450 and the camera
//! light slots 0x1FAB50. Addresses are the PAL executable's [F]; float
//! operations go through [`crate::ee`] in the original's order.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, M4, CAMERA_TABLE};

/// 0x208450's ALPHA presets (4 u64 at 0x2D0FA0, read from the executable).
const ALPHA_PRESETS: u32 = 0x002D_0FA0;
/// The zero rotation vector 0x28B490 (lq'd by 0x100E00 / 0x100E40).
const ZERO_ROT: u32 = 0x0028_B490;
/// 2 pi as the code loads it (0x40C90FDB).
const TWO_PI: f32 = f32::from_bits(0x40C9_0FDB);

impl Fx {
    /// 0x208450(i): ALPHA preset `i` (u64 0x2D0FA0[i]) for 0 <= i < 4, else 0.
    pub fn alpha_preset(&mut self, i: i32) -> u64 {
        if (0..4).contains(&i) {
            self.mem.read_u64(ALPHA_PRESETS + 8 * i as u32)
        } else {
            0
        }
    }

    /// 0x1883D8(out, a, b): out = a - b (VU0, xyzw).
    pub fn vu_sub4(&mut self, out: u32, a: u32, b: u32) {
        let (x, y) = (self.read_v4(a & !15), self.read_v4(b & !15));
        let r = std::array::from_fn(|k| ee::vsub(x[k], y[k]));
        self.write_v4(out & !15, r);
    }

    /// 0x188420(out, m, v): out = m with row 3's xyz + v's xyz (VU0; row
    /// 3's w kept).
    pub fn translate_rows(&mut self, out: u32, m: u32, v: u32) {
        let mut a = self.read_m4(m);
        let t = self.read_v4(v & !15);
        for k in 0..3 {
            a[3][k] = ee::vadd(a[3][k], t[k]);
        }
        self.write_m4(out, &a);
    }

    /// 0x20C290(m, v) [F] for m != 0: `m` = `m` x diag(v.x,
    /// v.y, v.z, 1) - an identity in its 0x70 frame at +0x30 with the
    /// diagonal set, then 0x20B370 (b175 / b507).
    pub fn scale_rows(&mut self, m: u32, v: u32) {
        let f = self.enter(0x70);
        self.identity(f + 0x30);
        let (x, y, z) = (self.mem.read_u32(v), self.mem.read_u32(v + 4), self.mem.read_u32(v + 8));
        self.mem.write_u32(f + 0x30, x);
        self.mem.write_u32(f + 0x44, y);
        self.mem.write_u32(f + 0x58, z);
        let s = self.read_m4(f + 0x30);
        self.mul_into(Some(m), &s);
        self.leave(0x70);
    }

    /// 0x20B320(m): `m` = identity (the current matrix when 0).
    pub fn identity(&mut self, m: u32) {
        let id: M4 = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        if m == 0 {
            self.cur = id;
        } else {
            self.write_m4(m, &id);
        }
    }

    /// 0x20E2B0(out): the current camera record's view-projection copied
    /// to `out` (16 words).
    pub fn camera_vp_to(&mut self, out: u32) {
        let vp = self.camera_vp();
        for r in 0..4 {
            for k in 0..4 {
                self.mem.write_f32(out + 16 * r as u32 + 4 * k as u32, vp[r][k]);
            }
        }
    }

    /// 0x20D660(out) [F]: the current camera record's nine
    /// quads from +0xC0 (0x2ED7E0 + 0x1A0 x index 0x2ED6C0: the input
    /// 0x20DAD0 sets a camera from) copied to `out` (lq / sq, 16-aligned);
    /// returns 1.
    pub fn camera_save(&mut self, out: u32) -> i32 {
        let i = self.mem.read_u32(0x002E_D6C0);
        let src = 0x002E_D7E0u32.wrapping_add(0x1A0u32.wrapping_mul(i));
        for k in 0..9u32 {
            let (s, d) = ((src & !15) + 0x10 * k, (out & !15) + 0x10 * k);
            for w in 0..4u32 {
                let x = self.mem.read_u32(s + 4 * w);
                self.mem.write_u32(d + 4 * w, x);
            }
        }
        1
    }

    /// 0x20C300(m, rot): `m` = R(rot) x `m` (0x20BDA0 then 0x20B370).
    pub fn rotate_by(&mut self, m: u32, rot: [i32; 3]) {
        let r = self.rotation_matrix(rot);
        self.mul_into(Some(m), &r);
    }

    /// 0x100E00(m, angle, axis): `m` rotated by `angle` & 0xFFFF about
    /// `axis` (0x20C300). The vector is kept at the frame's sp + 0x10 as
    /// the original keeps it.
    pub fn rotate_axis(&mut self, m: u32, angle: i32, axis: i32) {
        let sp = self.enter(0x20);
        for k in 0..4 {
            let v = self.mem.read_u32(ZERO_ROT + 4 * k);
            self.mem.write_u32(sp + 0x10 + 4 * k, v);
        }
        self.mem.set32(sp + 0x10 + 4 * axis as u32, angle & 0xFFFF);
        let rot = std::array::from_fn(|k| self.mem.i32(sp + 0x10 + 4 * k as u32));
        self.rotate_by(m, rot);
        self.leave(0x20);
    }

    /// 0x100E40(m, v): `m` = identity, turned to point along `v`: about z
    /// by atan2f(v.y, v.x), then about y by atan2f(sqrt(x x + y y), v.z),
    /// each angle 0x1A2B80(65536 (a / 2 pi)) & 0xFFFF.
    pub fn point_along(&mut self, m: u32, v: u32) {
        let sp = self.enter(0x60);
        self.identity(m);
        let (x, y) = (self.mem.f(v), self.mem.f(v + 4));
        let a = self.atan2f(y, x);
        let ang = ee::fixsfsi(ee::mul(65536.0, ee::div(a, TWO_PI))) & 0xFFFF;
        for k in 0..4 {
            let w = self.mem.read_u32(ZERO_ROT + 4 * k);
            self.mem.write_u32(sp + 0x40 + 4 * k, w);
        }
        self.mem.set32(sp + 0x48, ang);
        let rot = std::array::from_fn(|k| self.mem.i32(sp + 0x40 + 4 * k as u32));
        self.rotate_by(m, rot);
        let x2 = Self::powf2(self.mem.f(v));
        let y2 = Self::powf2(self.mem.f(v + 4));
        let h = self.sqrtf(ee::add(x2, y2));
        let z = self.mem.f(v + 8);
        let a = self.atan2f(h, z);
        let ang = ee::fixsfsi(ee::mul(65536.0, ee::div(a, TWO_PI))) & 0xFFFF;
        for k in 0..4 {
            let w = self.mem.read_u32(ZERO_ROT + 4 * k);
            self.mem.write_u32(sp + 0x50 + 4 * k, w);
        }
        self.mem.set32(sp + 0x54, ang);
        let rot = std::array::from_fn(|k| self.mem.i32(sp + 0x50 + 4 * k as u32));
        self.rotate_by(m, rot);
        self.leave(0x60);
    }

    /// 0x101020(a, b): a / b when a < b, else 1.0.
    pub fn ratio_below(a: f32, b: f32) -> f32 {
        if crate::exe::ee_lt(a, b) {
            ee::div(a, b)
        } else {
            1.0
        }
    }

    /// 0x100D50(k, n, p): 1 - (tanf(2 pi (((n - k) << 16) / 5 n) / 65536)
    /// / tanf(2 pi / 5 (0x3FA0D8DB)))^p for p = 2 (powf's only ported
    /// case; every ported caller passes 2).
    pub fn tan_falloff(&mut self, k: i32, n: i32) -> f32 {
        let num = n.wrapping_sub(k) << 16;
        let den = n.wrapping_mul(5);
        let (q, _) = self.div(num, den);
        let f = ee::div(ee::from_i32(q), 65536.0);
        let a = self.tanf(ee::mul(TWO_PI, f));
        let b = self.tanf(f32::from_bits(0x3FA0_D8DB));
        ee::sub(1.0, Self::powf2(ee::div(a, b)))
    }

    /// 0x1FAB50(cam, slot, src): camera object `cam` (only 0)'s light slot
    /// `slot` (< 10): src 0 clears bit `slot` of +0x8AA, 1 sets it, else
    /// the 3 quads at `src` copied to +0x600 + 0x30 slot (+0x1C = 1.0) and
    /// the bit set; 1 when done, 0 when refused.
    pub fn camera_light(&mut self, cam: i32, slot: i32, src: u32) -> i32 {
        if cam != 0 {
            return 0;
        }
        let o = self.mem.read_u32(CAMERA_TABLE);
        if o == 0 || !(0..10).contains(&slot) {
            return 0;
        }
        let bits = self.mem.read_u16(o + 0x8AA);
        if src == 0 {
            self.mem.write_u16(o + 0x8AA, bits & !(1u16 << slot));
            return 1;
        }
        if src != 1 {
            let t = o + 0x30 * slot as u32;
            for k in 0..12 {
                let w = self.mem.read_u32((src & !15) + 4 * k);
                self.mem.write_u32(t + 0x600 + 4 * k, w);
            }
            self.mem.write_u32(t + 0x61C, 0x3F80_0000);
        }
        let bits = self.mem.read_u16(o + 0x8AA);
        self.mem.write_u16(o + 0x8AA, bits | (1u16 << slot));
        1
    }
}

/// The board's unit records (0x1DE3A0: 0x2DB990 + 0x3F4 side + 0x5C slot).
const UNIT_RECORDS: u32 = 0x002D_B990;

impl Fx {
    /// The object record at index `i` of the table 0x2DE2C0 (0 when
    /// none or out of 0..0x54).
    pub fn object_at(&mut self, i: i32) -> u32 {
        if !(0..0x54).contains(&i) {
            return 0;
        }
        self.mem.read_u32(crate::world::OBJECT_TABLE + 4 * i as u32)
    }

    /// 0x1F3FE0(index, key, slot): object `index`'s node request `slot`
    /// (< 8) = `key` (0x7FFF for a negative key); refused (0) when the
    /// node is not below the object's count (+0x1FE) or the slot holds a
    /// request of another module type.
    pub fn object_request_node(&mut self, index: i32, key: i32, slot: i32) -> i32 {
        let o = self.object_at(index);
        if o == 0 || !(0..8).contains(&slot) {
            return 0;
        }
        let at = o + 0x1B0 + 2 * slot as u32;
        if key >= 0 {
            let kind = (key & 0x7F00) >> 8;
            if (key & 0xFF) >= self.mem.u8(o + 0x1FE) {
                return 0;
            }
            let cur = self.mem.u16(at);
            if (cur & 0x7FFF) < 0x7FFF && (cur & 0x7F00) >> 8 != kind {
                return 0;
            }
        }
        self.mem.set16(at, if key < 0 { 0x7FFF } else { key });
        1
    }

    /// 0x1F40D0(index, slot, out): object `index`'s node `slot` position
    /// (vec4 +0xA0 + 0x10 slot) into `out` when the object is drawn (+0x1F2
    /// >= 0, +0x20C bit 5) and the request is filled (key bit 0x8000);
    /// `out` untouched otherwise (0).
    pub fn object_node(&mut self, index: i32, slot: i32, out: u32) -> i32 {
        let o = self.object_at(index);
        if o == 0 || self.mem.i8(o + 0x1F2) < 0 || self.mem.read_u8(o + 0x20C) & 0x20 == 0 || !(0..8).contains(&slot) {
            return 0;
        }
        if self.mem.u16(o + 0x1B0 + 2 * slot as u32) & 0x8000 == 0 {
            return 0;
        }
        for k in 0..4 {
            let w = self.mem.read_u32(o + 0xA0 + 0x10 * slot as u32 + 4 * k);
            self.mem.write_u32(out + 4 * k, w);
        }
        1
    }

    /// 0x1F31E0(index, out): object `index`'s position (+0, vec4) into
    /// `out` (when not 0); 1 when the object exists.
    pub fn object_position(&mut self, index: i32, out: u32) -> i32 {
        let o = self.object_at(index);
        if o == 0 {
            return 0;
        }
        if out != 0 {
            for k in 0..4 {
                let w = self.mem.read_u32(o + 4 * k);
                self.mem.write_u32(out + 4 * k, w);
            }
        }
        1
    }

    /// 0x1F34C0(index, v): object `index`'s position (+0) = `v` (vec4).
    pub fn object_set_position(&mut self, index: i32, v: u32) -> i32 {
        let o = self.object_at(index);
        if o == 0 {
            return 0;
        }
        if v != 0 {
            for k in 0..4 {
                let w = self.mem.read_u32(v + 4 * k);
                self.mem.write_u32(o + 4 * k, w);
            }
        }
        1
    }

    /// 0x1F2770(index, motion, blend) (0x1F2760: blend 6, ChangeMotion):
    /// with the motion loaded (+0x178 + 4 m): the previous state kept
    /// (+0x198 -> +0x1A4, +0x1E0 -> +0x1E1), +0x1E0 = m, the time +0x19C =
    /// 1.0 (frames - 1 when +0x1F7's top bits are 11, reverse), +0x1A0 =
    /// 0, +0x1AC = 1.0, the blend +0x1FB, +0x1FD = 0; 1, or 0 when refused.
    pub fn change_motion(&mut self, index: i32, motion: i32, blend: i32) -> i32 {
        let o = self.object_at(index);
        if o == 0 || !(0..8).contains(&motion) {
            return 0;
        }
        let m = o + 4 * motion as u32;
        if self.mem.read_u32(m + 0x178) == 0 {
            return 0;
        }
        let last = ee::sub(frames_f32(self.mem.read_u32(m + 0x1C0)), 1.0);
        for k in 0..3 {
            let v = self.mem.read_u32(o + 0x198 + 4 * k);
            self.mem.write_u32(o + 0x1A4 + 4 * k, v);
        }
        let b = self.mem.read_u8(o + 0x1E0);
        self.mem.write_u8(o + 0x1E1, b);
        self.mem.set8(o + 0x1E0, motion);
        let dir = ((i64::from(self.mem.read_u8(o + 0x1F7) as i8) << 56) >> 62) as i32;
        if dir == -1 {
            self.mem.setf(o + 0x19C, last);
        } else {
            self.mem.write_u32(o + 0x19C, 0x3F80_0000);
        }
        self.mem.set32(o + 0x1A0, 0);
        self.mem.setf(o + 0x1AC, ee::sub(1.0, 0.0));
        self.mem.set8(o + 0x1FB, blend);
        self.mem.write_u8(o + 0x1FD, 0);
        1
    }

    /// 0x1F2610(index, 0) (0x1F2600): object `index`'s motion (+0x1E0, i8),
    /// -1 when none.
    pub fn object_motion_of(&mut self, index: i32) -> i32 {
        let o = self.object_at(index);
        if o == 0 {
            return -1;
        }
        self.mem.i8(o + 0x1E0)
    }

    /// 0x1F2D70(index, 0) (0x1F2D60): object `index`'s motion time
    /// (+0x19C), -1.0 when it has no loaded motion.
    pub fn object_motion_time(&mut self, index: i32) -> f32 {
        let o = self.object_at(index);
        if o == 0 {
            return -1.0;
        }
        let m = self.mem.i8(o + 0x1E0);
        if m < 0 || self.mem.read_u32(o + 0x178 + 4 * m as u32) == 0 {
            return -1.0;
        }
        self.mem.f(o + 0x19C)
    }

    /// 0x1F2E60(index, 0, t) (0x1F2E50): object `index`'s motion time =
    /// `t` clamped to 1 .. frames - 1; 1 when set.
    pub fn object_set_motion_time(&mut self, index: i32, t: f32) -> i32 {
        let o = self.object_at(index);
        if o == 0 {
            return 0;
        }
        let m = self.mem.i8(o + 0x1E0);
        if m < 0 {
            return 0;
        }
        let at = o + 4 * m as u32;
        if self.mem.read_u32(at + 0x178) == 0 {
            return 0;
        }
        let f = frames_f32(self.mem.read_u32(at + 0x1C0));
        let mut t = t;
        if crate::exe::ee_lt(t, 1.0) {
            t = 1.0;
        }
        let last = ee::sub(f, 1.0);
        if !crate::exe::ee_le(t, last) {
            t = last;
        }
        self.mem.setf(o + 0x19C, t);
        1
    }

    /// A unit handle (side << 8 | slot) checked as 0x1F7830.. check it
    /// (side 0..2, slot 0..10), as its object index (0x1E25E0), or None.
    fn handle_index(&mut self, h: i32) -> Option<i32> {
        let (side, slot) = (h >> 8, h & 0xFF);
        if !(0..3).contains(&side) || slot >= 11 {
            return None;
        }
        let i = self.object_index(side, slot);
        (i >= 0).then_some(i)
    }

    /// 0x262FA0(handle): keep a unit playing motion 4 (its idle loop
    /// [H]): another motion is changed to 4 (0x1F78B0 -> 0x1F2760); in
    /// motion 4 a time of 16 or more is set back to 11, one between 11 and
    /// 16 by 2 on one rand draw in three.
    pub fn keep_motion4(&mut self, h: i32) {
        let cur = match self.handle_index(h) {
            Some(i) => self.object_motion_of(i),
            None => -1,
        };
        if cur != 4 {
            if let Some(i) = self.handle_index(h) {
                self.change_motion(i, 4, 6);
            }
            return;
        }
        let t = match self.handle_index(h) {
            Some(i) => self.object_motion_time(i),
            None => 0.0,
        };
        if !crate::exe::ee_lt(t, 16.0) {
            if let Some(i) = self.handle_index(h) {
                self.object_set_motion_time(i, 11.0);
            }
            return;
        }
        if crate::exe::ee_le(t, 11.0) {
            return;
        }
        let r = self.rand();
        let (_, m) = self.div(r, 3);
        if m == 0 {
            if let Some(i) = self.handle_index(h) {
                self.object_set_motion_time(i, ee::sub(t, 2.0));
            }
        }
    }

    /// 0x1DE3A0(side, slot): the board unit record (side >= 0: 0x2DB990 +
    /// 0x3F4 side + 0x5C slot; side < 0: `slot` is an object index whose
    /// kind +0x1F3 4 / 5 names the record), 0 when out of range.
    pub fn unit_record(&mut self, side: i32, slot: i32) -> u32 {
        let (mut a, mut s) = (side, slot);
        if side < 0 {
            let o = self.object_at(slot);
            let kind = if o == 0 { -1 } else { self.mem.i8(o + 0x1F3) };
            match kind {
                4 => {
                    let v = slot - 0x14;
                    let (_, m) = self.div(v, 5);
                    a = i32::from(v >= 5);
                    s = m + 1;
                }
                5 => {
                    a = slot - 0x1E;
                    s = 0;
                }
                _ => return 0,
            }
        }
        if !(0..3).contains(&a) || !(0..11).contains(&s) {
            return 0;
        }
        UNIT_RECORDS.wrapping_add((0x3F4 * a + 0x5C * s) as u32)
    }

    /// 0x1F6B80(handle, on) -> 0x1E4AD0(side, slot, on): the board unit's
    /// bit 7 of +0x57 and bit 0 of +0x58 = `on` != 0 (a unit whose +0x45
    /// is 11 or more is refused); 1 when set.
    pub fn unit_flags(&mut self, h: i32, on: i32) -> i32 {
        let (side, slot) = (h >> 8, h & 0xFF);
        if !(0..3).contains(&side) || slot >= 11 {
            return 0;
        }
        let r = self.unit_record(side, slot);
        if r == 0 || self.mem.u8(r + 0x45) >= 11 {
            return 0;
        }
        let v = u8::from(on != 0);
        let b = self.mem.read_u8(r + 0x57);
        self.mem.write_u8(r + 0x57, (b & 0x7F) | (v << 7));
        let b = self.mem.read_u8(r + 0x58);
        self.mem.write_u8(r + 0x58, (b & !1) | v);
        1
    }
}

/// A frame count (u32) as the motion code converts it (cvt.s.w, halved
/// and doubled for the top bit).
fn frames_f32(frames: u32) -> f32 {
    if (frames as i32) < 0 {
        let h = ee::from_i32(((frames >> 1) | (frames & 1)) as i32);
        ee::add(h, h)
    } else {
        ee::from_i32(frames as i32)
    }
}

/// The board-card accessors 0x1F6D00..0x1F7050 (through the unit record
/// 0x1DE3A0 and its card object pointer +4) and the unit-object scale
/// 0x1F74C0 / 0x1F7550, as c062 calls them [F].
impl Fx {
    /// 0x1E07C0(side, slot): the unit record when its +0x45 is below 11.
    fn card_record(&mut self, h: i32) -> u32 {
        let r = self.unit_record(h >> 8, h & 0xFF);
        if r == 0 || self.mem.u8(r + 0x45) >= 11 {
            0
        } else {
            r
        }
    }

    /// The card object of a handle checked as 0x1F6D80.. check it (side
    /// 0..2, slot 0..10), 0 when none.
    fn card_object(&mut self, h: i32) -> u32 {
        let (side, slot) = (h >> 8, h & 0xFF);
        if !(0..3).contains(&side) || slot >= 11 {
            return 0;
        }
        let r = self.card_record(h);
        if r == 0 {
            0
        } else {
            self.mem.read_u32(r + 4)
        }
    }

    fn copy16(&mut self, to: u32, from: u32) {
        for k in 0..4 {
            let w = self.mem.read_u32(from + 4 * k);
            self.mem.write_u32(to + 4 * k, w);
        }
    }

    /// 0x1F6D00(h): 1 when the unit has a card object.
    pub fn card_exists(&mut self, h: i32) -> i32 {
        let r = self.card_record(h);
        i32::from(r != 0 && self.mem.read_u32(r + 4) != 0)
    }

    /// 0x1F6DE0(h, out): the card's position (vec4 +0), (0, 0, 0, 1) first.
    pub fn card_position(&mut self, h: i32, out: u32) -> i32 {
        self.write_v4(out, [0.0, 0.0, 0.0, 1.0]);
        let o = self.card_object(h);
        if o == 0 {
            return 0;
        }
        self.copy16(out, o);
        1
    }

    /// 0x1F6D80(h, v): the card's position = `v`.
    pub fn card_set_position(&mut self, h: i32, v: u32) -> i32 {
        let o = self.card_object(h);
        if o == 0 {
            return 0;
        }
        if v != 0 {
            self.copy16(o, v);
        }
        1
    }

    /// 0x1F6EB0(h, out): the card's rotation (+0x10), zeros first.
    pub fn card_rotation(&mut self, h: i32, out: u32) -> i32 {
        for k in 0..4 {
            self.mem.write_u32(out + 4 * k, 0);
        }
        let o = self.card_object(h);
        if o == 0 {
            return 0;
        }
        self.copy16(out, o + 0x10);
        1
    }

    /// 0x1F6E50(h, v): the card's rotation = `v`.
    pub fn card_set_rotation(&mut self, h: i32, v: u32) -> i32 {
        let o = self.card_object(h);
        if o == 0 {
            return 0;
        }
        if v != 0 {
            self.copy16(o + 0x10, v);
        }
        1
    }

    /// 0x1F6F20(h, v): the card's scale (+0x30) = `v`.
    pub fn card_set_scale(&mut self, h: i32, v: u32) -> i32 {
        let o = self.card_object(h);
        if o == 0 {
            return 0;
        }
        if v != 0 {
            self.copy16(o + 0x30, v);
        }
        1
    }

    /// 0x1F7050(h, out): the card's colour bytes (+0x40), 0x80 each first.
    pub fn card_colour(&mut self, h: i32, out: u32) -> i32 {
        for k in 0..4 {
            self.mem.write_u8(out + k, 0x80);
        }
        let o = self.card_object(h);
        if o == 0 {
            return 0;
        }
        for k in 0..4 {
            let b = self.mem.read_u8(o + 0x40 + k);
            self.mem.write_u8(out + k, b);
        }
        1
    }

    /// 0x1F6FF0(h, c): the card's colour bytes = `c`.
    pub fn card_set_colour(&mut self, h: i32, c: u32) -> i32 {
        let o = self.card_object(h);
        if o == 0 {
            return 0;
        }
        if c != 0 {
            for k in 0..4 {
                let b = self.mem.read_u8(c + k);
                self.mem.write_u8(o + 0x40 + k, b);
            }
        }
        1
    }

    /// 0x1F70D0(h): the unit's model object (unit record +0x46) is drawn
    /// (0x1F2580: +0x1F2 >= 0 and bit 1 of +0x20C).
    pub fn unit_drawn(&mut self, h: i32) -> i32 {
        let r = self.card_record(h);
        if r == 0 {
            return 0;
        }
        let i = self.mem.u8(r + 0x46);
        let o = self.object_at(i);
        if o == 0 || self.mem.i8(o + 0x1F2) < 0 {
            return 0;
        }
        i32::from(self.mem.read_u8(o + 0x20C) & 2 != 0)
    }

    /// 0x1F7550(h, out): the unit object's scale (+0x30), (0, 0, 0, 1) first.
    pub fn unit_scale(&mut self, h: i32, out: u32) -> i32 {
        self.write_v4(out, [0.0, 0.0, 0.0, 1.0]);
        let Some(i) = self.handle_index(h) else { return 0 };
        let o = self.object_at(i);
        if o == 0 {
            return 0;
        }
        if out != 0 {
            self.copy16(out, o + 0x30);
        }
        1
    }

    /// 0x1F74C0(h, v): the unit object's scale = `v` (1.0 each when 0).
    pub fn unit_set_scale(&mut self, h: i32, v: u32) -> i32 {
        let Some(i) = self.handle_index(h) else { return 0 };
        let o = self.object_at(i);
        if o == 0 {
            return 0;
        }
        if v != 0 {
            self.copy16(o + 0x30, v);
        } else {
            for k in 0..4 {
                self.mem.write_u32(o + 0x30 + 4 * k, 0x3F80_0000);
            }
        }
        1
    }
}
