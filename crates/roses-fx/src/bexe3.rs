//! Executable helpers the "a0 = D" single-function records b150 / b475 /
//! b483 (cards 168 / 411 / 589) call,
//! hand-ported: the VU0 helpers 0x1883C0 / 0x1881A0 / 0x188408, the
//! clamp 0x101050, the random table vector 0x101080, the unit side
//! 0x100D00, the ramp 0x100FF0 and the camera turn with roll 0x2055E0
//! (= 0x1FC3E0(0, out)). Addresses are the PAL executable's [F]; float
//! operations go through [`crate::ee`] in the original's order.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, CAMERA_TABLE};

/// 0x101080's direction table (4 x 32 vec3 at 0x28AE90, read from the
/// user's executable).
const DIR_TABLE: u32 = 0x0028_AE90;
/// The zero rotation vector 0x28B490 (lq'd by 0x101080).
const ZERO_ROT: u32 = 0x0028_B490;
/// 0x1FC3E0's base rotation (lq 0x2922E0).
const ROLL_BASE: u32 = 0x0029_22E0;
const PI: f32 = f32::from_bits(0x4049_0FDB);

/// The compiler's signed remainder by a power of two `m` (`andi`, then
/// `- m` for a negative dividend with a non-zero remainder).
fn rem_pow2(x: i32, m: i32) -> i32 {
    let r = x & (m - 1);
    if x < 0 && r != 0 {
        r - m
    } else {
        r
    }
}

impl Fx {
    /// 0x1883C0(out, a, b): out = a + b (VU0 vadd, xyzw; quadword
    /// addresses).
    pub fn vu_add4(&mut self, out: u32, a: u32, b: u32) {
        let (x, y) = (self.read_v4(a & !15), self.read_v4(b & !15));
        let r = std::array::from_fn(|k| ee::vadd(x[k], y[k]));
        self.write_v4(out & !15, r);
    }

    /// 0x1881A0(out, m, v): out = v through the matrix rows m (VU0 mulax /
    /// maddax / maddax / maddw, xyzw).
    pub fn vu_apply_to(&mut self, out: u32, m: u32, v: u32) {
        let m = self.read_m4(m & !15);
        let v = self.read_v4(v & !15);
        let r = Self::vu_apply(&m, v);
        self.write_v4(out & !15, r);
    }

    /// 0x188408(out, v, f): out = v x f (VU0 vmulx, xyzw).
    pub fn vu_scale4(&mut self, out: u32, v: u32, f: f32) {
        let x = self.read_v4(v & !15);
        let r = std::array::from_fn(|k| ee::vmul(x[k], f));
        self.write_v4(out & !15, r);
    }

    /// 0x101050(v, lo, hi): `v` clamped to [lo, hi] (lo tested first).
    pub fn clamp_i(v: i32, lo: i32, hi: i32) -> i32 {
        if v < lo {
            lo
        } else if hi < v {
            hi
        } else {
            v
        }
    }

    /// 0x101080(out, length) [F]: a random vector of the table 0x28AE90:
    /// per axis the entry (rand >> 23) % 4 x 384 + % 32 x 12 + % 3 x 4
    /// (signed remainders) times 2 ((rand >> 23) % 2) - 1; w = 1.0; the
    /// whole scaled by `length` (0x188408), then turned by three random
    /// angles ((rand >> 23) % 256 << 8, about x, y, z in turn: 0x20B320,
    /// 0x20C300 with the vectors at sp + 0x80 / 0x90 / 0xA0) through
    /// 0x1881A0. A 0xB0 frame (the matrix at sp + 0x40); rand's frames
    /// saved s0 = `out`.
    pub fn random_table_vec(&mut self, out: u32, length: f32) {
        let sp = self.enter(0xB0);
        let ras = [
            [0x0010_10A0u32, 0x0010_10D4, 0x0010_1100, 0x0010_1124],
            [0x0010_116C, 0x0010_11A0, 0x0010_11CC, 0x0010_11F0],
            [0x0010_1238, 0x0010_126C, 0x0010_1298, 0x0010_12BC],
        ];
        for (c, ra) in ras.iter().enumerate() {
            let r = self.rand_from(out, ra[0]) >> 23;
            let i = rem_pow2(r, 4);
            let r = self.rand_from(out, ra[1]) >> 23;
            let j = rem_pow2(r, 32);
            let r = self.rand_from(out, ra[2]) >> 23;
            let (_, k) = self.div(r, 3);
            let e = DIR_TABLE.wrapping_add((i * 384 + j * 12 + k * 4) as u32);
            let r = self.rand_from(out, ra[3]) >> 23;
            let s = rem_pow2(r, 2);
            let f = ee::sub(ee::mul(2.0, ee::from_i32(s)), 1.0);
            let v = ee::mul(f, self.mem.f(e));
            self.mem.setf(out + 4 * c as u32, v);
        }
        self.mem.write_u32(out + 0xC, 0x3F80_0000);
        self.vu_scale4(out, out, length);
        self.identity(sp + 0x40);
        for (k, ra) in [0x0010_1324u32, 0x0010_1370, 0x0010_13BC].iter().enumerate() {
            let r = self.rand_from(out, *ra) >> 23;
            let a = (((rem_pow2(r, 256) << 16) >> 8) & 0xFFFF) as u32;
            let at = sp + 0x80 + 0x10 * k as u32;
            for w in 0..4 {
                let v = self.mem.read_u32(ZERO_ROT + 4 * w);
                self.mem.write_u32(at + 4 * w, v);
            }
            self.mem.write_u32(at + 4 * k as u32, a);
            let rot = std::array::from_fn(|w| self.mem.i32(at + 4 * w as u32));
            self.rotate_by(sp + 0x40, rot);
        }
        self.vu_apply_to(out, sp + 0x40, out);
        self.leave(0xB0);
    }

    /// 0x100D00(handle) [F]: 1 when the unit's position (0x1F71A0 into
    /// sp + 0x20 of a 0x30 frame) has z >= 0, else -1.
    pub fn unit_side(&mut self, handle: i32) -> i32 {
        let sp = self.enter(0x30);
        self.unit_position(handle, sp + 0x20);
        let f = sp - 0x20;
        self.saved(f, &[(0, 1), (0x10, 0x0010_0D18)]);
        let z = self.mem.f(sp + 0x28);
        let r = if ee_lt(z, 0.0) { -1 } else { 1 };
        self.leave(0x30);
        r
    }

    /// 0x100FF0(x, lo, hi): 1.0 when x <= hi, else (x - lo) / (hi - lo).
    pub fn ramp_above(x: f32, lo: f32, hi: f32) -> f32 {
        if ee_le(x, hi) {
            1.0
        } else {
            ee::div(ee::sub(x, lo), ee::sub(hi, lo))
        }
    }

    /// 0x2055E0(out) = 0x1FC3E0(0, out) [F]: with a camera object
    /// (0x2DF5C0 [0]) `out` = the rotation (-(32768 obj+8 / pi), 0x4000 -
    /// 32768 obj+4 / pi, 0) (0x1A2B80; 0x20BDA0), then turned by the
    /// rotation of 0x2922E0's vector with z = -obj+0x40 (sp + 0x30 /
    /// matrix sp + 0x50, 0x20B370); 1. Else 0 and nothing written. A 0x90
    /// frame.
    pub fn camera_turn_roll(&mut self, out: u32) -> i32 {
        let sp = self.enter(0x90);
        for k in 0..4 {
            let w = self.mem.read_u32(ROLL_BASE + 4 * k);
            self.mem.write_u32(sp + 0x30 + 4 * k, w);
        }
        let obj = self.mem.read_u32(CAMERA_TABLE);
        let roll = if obj == 0 { 0 } else { self.mem.read_u32(obj + 0x40) };
        self.mem.write_u32(sp + 0x38, roll.wrapping_neg());
        if out == 0 || obj == 0 {
            self.leave(0x90);
            return 0;
        }
        let a = |fx: &mut Fx, off: u32| ee::fixsfsi(ee::div(ee::mul(32768.0, fx.mem.f(obj + off)), PI));
        let rx = a(self, 8).wrapping_neg();
        self.mem.set32(sp + 0x40, rx);
        let ry = 0x4000i32.wrapping_sub(a(self, 4));
        self.mem.set32(sp + 0x44, ry);
        self.mem.write_u32(sp + 0x4C, 0);
        self.mem.write_u32(sp + 0x48, 0);
        let m = self.rotation_matrix([rx, ry, 0]);
        self.write_m4(out, &m);
        let rot = std::array::from_fn(|k| self.mem.i32(sp + 0x30 + 4 * k as u32));
        let r = self.rotation_matrix(rot);
        self.write_m4(sp + 0x50, &r);
        self.mul_into(Some(out), &r);
        self.leave(0x90);
        1
    }

    /// 0x100D50(k, n, p) [F]: [`Fx::tan_falloff`] with powf(r, p);
    /// powf(r, 1.0) is r (0x1A53D8 returns x for |y| = 1, y >= 0),
    /// powf(r, 2.0) r x r, any other power through [`Fx::powf`] (helper
    /// D20).
    pub fn tan_falloff_p(&mut self, k: i32, n: i32, p: i32) -> f32 {
        if p == 2 {
            return self.tan_falloff(k, n);
        }
        if p != 1 {
            let num = n.wrapping_sub(k) << 16;
            let den = n.wrapping_mul(5);
            let (q, _) = self.div(num, den);
            let f = ee::div(ee::from_i32(q), 65536.0);
            let a = self.tanf(ee::mul(f32::from_bits(0x40C9_0FDB), f));
            let b = self.tanf(f32::from_bits(0x3FA0_D8DB));
            let r = self.powf(ee::div(a, b), ee::from_i32(p));
            return ee::sub(1.0, r);
        }
        let num = n.wrapping_sub(k) << 16;
        let den = n.wrapping_mul(5);
        let (q, _) = self.div(num, den);
        let f = ee::div(ee::from_i32(q), 65536.0);
        let a = self.tanf(ee::mul(f32::from_bits(0x40C9_0FDB), f));
        let b = self.tanf(f32::from_bits(0x3FA0_D8DB));
        ee::sub(1.0, ee::div(a, b))
    }
}

/// Helper C13 (31 Oct 2026): the helpers b483 (card 589) adds - the
/// screen-position UVs 0x100C60, the triangle / quad projections 0x100B00
/// / 0x100BA0 (vertex copies through 0x205210), the unit's tint with a
/// mode 0x1F7600 and its position setter 0x1F7110 [F].
impl Fx {
    /// 0x100C60(uv, xyz, du, dv): the UV halfwords at `uv` (top two bits
    /// kept) = 16 x the texel under the XYZ2 at `xyz`: x / 16 - 0x700 + du
    /// and y / 16 - 0x700 + dv, each clamped to 1..0x1FF (<= 0 -> 1).
    pub fn uv_from_xy(&mut self, uv: u32, xyz: u32, du: i32, dv: i32) {
        let y = i32::from(self.mem.read_u16(xyz + 2)) >> 4;
        let x = i32::from(self.mem.read_u16(xyz)) >> 4;
        let clamp = |v: i32| if v <= 0 { 1 } else if v < 0x200 { v } else { 0x1FF };
        let u = clamp((x - 0x700).wrapping_add(du));
        let v = clamp((y - 0x700).wrapping_add(dv));
        let h = self.mem.read_u16(uv);
        self.mem.write_u16(uv, (h & 0xC000) | ((u << 4) & 0x3FFF) as u16);
        let h = self.mem.read_u16(uv + 2);
        self.mem.write_u16(uv + 2, (h & 0xC000) | ((v << 4) & 0x3FFF) as u16);
    }

    /// 0x100B00(v0, v1, v2, m, desc, w) / 0x100BA0(v0, v1, v2, v3, m,
    /// desc, w): the vertices copied into a frame (0x40 / 0x50: `ra` at +0,
    /// the copies from +0x10), projected there by 0x205210 into 0x2D3600
    /// (the matrix at `m`); 1 when drawn.
    pub fn project_n(&mut self, vs: &[u32], m: u32, desc: u32, w: u32, ra: u32) -> i32 {
        let bytes = 0x10 * vs.len() as u32 + 0x10;
        let sp = self.enter(bytes);
        self.saved(sp, &[(0, u64::from(ra))]);
        for (k, &v) in vs.iter().enumerate() {
            for c in 0..4u32 {
                let x = self.mem.read_u32(v + 4 * c);
                self.mem.write_u32(sp + 0x10 + 0x10 * k as u32 + 4 * c, x);
            }
        }
        let mm = self.read_m4(m);
        let r = self.project_desc(sp + 0x10, 0x002D_3600, Some(&mm), desc, w);
        self.leave(bytes);
        i32::from(r)
    }

    /// 0x1F7600(handle, mode, c) [F]: for a unit object (side 0..2, slot
    /// 0..10, 0x1E25E0; 0x1F38F0) obj+0x1F8's low two bits = `mode` when
    /// 0..3, its tint obj+0x12C..0x12F = the 4 bytes at `c` (0: 0x80
    /// each); 1, else 0. Its 0x30 frame (the caller's s0 / s1 = `s`, `ra`)
    /// is left below the caller's stack pointer.
    pub fn unit_tint_mode(&mut self, handle: i32, mode: i32, c: u32, s: [u64; 2], ra: u32) -> i32 {
        let f = self.sp.wrapping_sub(0x30);
        self.saved(f, &[(0, s[0]), (0x10, s[1]), (0x20, u64::from(ra))]);
        let (side, slot) = (handle >> 8, handle & 0xFF);
        if !(0..3).contains(&side) || slot >= 0xB {
            return 0;
        }
        let i = self.object_index(side, slot);
        if !(0..0x54).contains(&i) {
            return 0;
        }
        let o = self.mem.read_u32(crate::world::OBJECT_TABLE + 4 * i as u32);
        if o == 0 {
            return 0;
        }
        if (0..4).contains(&mode) {
            let b = self.mem.read_u8(o + 0x1F8);
            self.mem.write_u8(o + 0x1F8, (b & !3) | (mode & 3) as u8);
        }
        for k in 0..4u32 {
            let v = if c != 0 { self.mem.read_u8(c + k) } else { 0x80 };
            self.mem.write_u8(o + 0x12C + k, v);
        }
        1
    }

    /// 0x1F7110(handle, v) [F]: a unit object's position (+0) = the vec4
    /// at `v` (0x1F34C0); 1, else 0. Its 0x20 frame (the caller's s0 = `s0`,
    /// `ra`) is left below the caller's stack pointer.
    pub fn unit_set_position(&mut self, handle: i32, v: u32, s0: u64, ra: u32) -> i32 {
        let f = self.sp.wrapping_sub(0x20);
        self.saved(f, &[(0, s0), (0x10, u64::from(ra))]);
        let (side, slot) = (handle >> 8, handle & 0xFF);
        if !(0..3).contains(&side) || slot >= 0xB {
            return 0;
        }
        let i = self.object_index(side, slot);
        if i < 0 {
            return 0;
        }
        self.object_set_position(i, v)
    }
}

/// Helper C14 (31 Oct 2026): the unit flag b083 (card 299) sets.
impl Fx {
    /// 0x1F7940(handle, v) [F]: for a unit object (side 0..2, slot 0..10,
    /// 0x1E25E0; 0x1F2C10 on the object index 0..0x53 of table 0x2EE2C0)
    /// the byte obj+0x1F7's bits 6..7 = `v` & 3; 1 when written, else 0.
    /// Its 0x20 frame (s0 = `v`, `ra`) is left below the caller's stack
    /// pointer.
    pub fn unit_show(&mut self, handle: i32, v: i32, ra: u32) -> i32 {
        let f = self.sp.wrapping_sub(0x20);
        self.saved(f, &[(0, i64::from(v) as u64), (0x10, u64::from(ra))]);
        let (side, slot) = (handle >> 8, handle & 0xFF);
        if !(0..3).contains(&side) || slot >= 0xB {
            return 0;
        }
        let i = self.object_index(side, slot);
        if !(0..0x54).contains(&i) {
            return 0;
        }
        let o = self.mem.read_u32(crate::world::OBJECT_TABLE + 4 * i as u32);
        if o == 0 {
            return 0;
        }
        let b = self.mem.read_u8(o + 0x1F7);
        self.mem.write_u8(o + 0x1F7, (b & 0x3F) | (((v & 3) as u8) << 6));
        1
    }
}
