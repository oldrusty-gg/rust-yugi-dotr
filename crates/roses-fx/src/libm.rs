//! The executable's float library (newlib-derived, single precision) and
//! the effect library's angle helper built on it, hand-ported (helper C, 30
//! Oct 2026). Every float operation goes through [`crate::ee`] in the
//! original's order, so the bits are the interpreter's.
//!
//! The polynomial tables are read from the user's executable at runtime
//! (atanhi 0x2BD988, atanlo 0x2BD998, aT 0x2BD9A8) [F 0x1A6EB8].
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx};

const ATANHI: u32 = 0x002B_D988;
const ATANLO: u32 = 0x002B_D998;
const AT: u32 = 0x002B_D9A8;
/// 2 pi as the code loads it (0x40C90FDB).
const TWO_PI: u32 = 0x40C9_0FDB;

impl Fx {
    /// 0x1A6EB8 atanf(x) [F]: newlib's s_atanf without its NaN check.
    pub fn atanf(&mut self, x: f32) -> f32 {
        let ix = x.to_bits() as i32;
        let hx = ix & 0x7FFF_FFFF;
        if hx > 0x507F_FFFF {
            let (hi, lo) = (self.mem.read_f32(ATANHI + 12), self.mem.read_f32(ATANLO + 12));
            return if ix > 0 { ee::add(hi, lo) } else { ee::sub(ee::neg(hi), lo) };
        }
        let mut x = x;
        let id: i32;
        let z;
        if hx <= 0x3EDF_FFFF {
            if hx <= 0x30FF_FFFF {
                // huge + x > one: return x.
                let t = ee::add(x, f32::from_bits(0x7149_F2CA));
                if ee_lt(1.0, t) {
                    return x;
                }
            }
            id = -1;
            z = ee::mul(x, x);
        } else {
            x = ee::abs(x);
            if hx <= 0x3F97_FFFF {
                if hx <= 0x3F2F_FFFF {
                    let a = ee::add(x, x);
                    let b = ee::add(x, 2.0);
                    let a = ee::sub(a, 1.0);
                    x = ee::div(a, b);
                    id = 0;
                } else {
                    let b = ee::add(x, 1.0);
                    let a = ee::sub(x, 1.0);
                    x = ee::div(a, b);
                    id = 1;
                }
            } else if hx <= 0x401B_FFFF {
                let b = ee::mul(x, 1.5);
                let a = ee::sub(x, 1.5);
                let b = ee::add(b, 1.0);
                x = ee::div(a, b);
                id = 2;
            } else {
                x = ee::div(-1.0, x);
                id = 3;
            }
            z = ee::mul(x, x);
        }
        let t: [f32; 11] = std::array::from_fn(|k| self.mem.read_f32(AT + 4 * k as u32));
        let w = ee::mul(z, z);
        let f7 = ee::mul(w, t[10]);
        let f5 = ee::mul(w, t[9]);
        let f4 = ee::mul(w, ee::add(t[8], f7));
        let f1 = ee::mul(w, ee::add(t[7], f5));
        let f6 = ee::mul(w, ee::add(t[6], f4));
        let f2 = ee::mul(w, ee::add(t[5], f1));
        let f8 = ee::mul(w, ee::add(t[4], f6));
        let f3 = ee::mul(w, ee::add(t[3], f2));
        let f9 = ee::mul(w, ee::add(t[2], f8));
        let s2 = ee::mul(w, ee::add(t[1], f3));
        let s1 = ee::mul(z, ee::add(t[0], f9));
        if id < 0 {
            let f0 = ee::mul(x, ee::add(s1, s2));
            return ee::sub(x, f0);
        }
        let lo = self.mem.read_f32(ATANLO + 4 * id as u32);
        let hi = self.mem.read_f32(ATANHI + 4 * id as u32);
        let f0 = ee::mul(x, ee::add(s1, s2));
        let f0 = ee::sub(ee::sub(f0, lo), x);
        let r = ee::sub(hi, f0);
        if ix < 0 {
            ee::neg(r)
        } else {
            r
        }
    }

    /// 0x1A74D8 atan2f(y, x) -> 0x1A5020 [F]: the executable's own
    /// variant (no NaN cases; pi 0x40490FDA, pi / 2 0x3FC90FDB for a zero
    /// x, 0x3FC90FDC for a huge ratio, pi_lo 0x34222168).
    pub fn atan2f(&mut self, y: f32, x: f32) -> f32 {
        let (iy, ix) = (y.to_bits() as i32, x.to_bits() as i32);
        let (ay, ax) = (iy & 0x7FFF_FFFF, ix & 0x7FFF_FFFF);
        if x.to_bits() == 0x3F80_0000 {
            return self.atanf(y);
        }
        let m = (((iy as u32) >> 31) | (((ix >> 30) & 2) as u32)) as i32;
        let pi = f32::from_bits(0x4049_0FDA);
        if ay <= 0x7F_FFFF {
            match m {
                2 => return pi,
                3 => return ee::neg(pi),
                _ => return y,
            }
        }
        if ax <= 0x7F_FFFF {
            return if iy < 0 { f32::from_bits(0xBFC9_0FDB) } else { f32::from_bits(0x3FC9_0FDB) };
        }
        let k = ay.wrapping_sub(ax) >> 23;
        let z = if k > 0x3C {
            f32::from_bits(0x3FC9_0FDC)
        } else if ix < 0 && k < -0x3C {
            0.0
        } else {
            let q = ee::div(y, x);
            self.atanf(ee::abs(q))
        };
        let pi_lo = f32::from_bits(0x3422_2168);
        match m {
            0 => z,
            1 => ee::neg(z),
            2 => ee::sub(pi, ee::sub(z, pi_lo)),
            _ => ee::sub(ee::sub(z, pi_lo), pi),
        }
    }

    /// 0x1A7520 sqrtf -> 0x1A5FC0 [F]: the bit-by-bit integer square root
    /// (zero and denormals returned as they are; a negative (x - x) / (x - x)).
    pub fn sqrtf(&mut self, x: f32) -> f32 {
        let ix = x.to_bits() as i32;
        if (ix as u32 & 0x7FFF_FFFF) <= 0x7F_FFFF {
            return x;
        }
        if ix < 0 {
            let d = ee::sub(x, x);
            return ee::div(d, d);
        }
        let mut m = (ix >> 23).wrapping_sub(0x7F);
        let mut v = ((ix & 0x7F_FFFF) | 0x80_0000) << (m & 1);
        m >>= 1;
        v = v.wrapping_shl(1);
        let (mut s, mut q, mut r) = (0i32, 0i32, 0x100_0000i32);
        while r != 0 {
            let t = s.wrapping_add(r);
            if v >= t {
                v = v.wrapping_sub(t);
                s = t.wrapping_add(r);
                q = q.wrapping_add(r);
            }
            r = ((r as u32) >> 1) as i32;
            v = v.wrapping_shl(1);
        }
        if v != 0 {
            q = q.wrapping_add(q & 1);
        }
        let bits = 0x3F00_0000i32.wrapping_add(q >> 1).wrapping_add(m.wrapping_shl(23));
        f32::from_bits(bits as u32)
    }

    /// An angle in radians as the effect library turns it into 16 bits:
    /// 0x1A2B80(65536 (a / 2 pi)) & 0xFFFF.
    fn angle16(&mut self, a: f32, ra: u32) -> u32 {
        let f = ee::div(a, f32::from_bits(TWO_PI));
        let v = ee::mul(65536.0, f);
        self.fix_residue(v, ra);
        (ee::fixsfsi(v) & 0xFFFF) as u32
    }

    /// 0x103DB0(a, b, out) [F]: the angles that look from `b` to `a`
    /// (d = a - b): out = (atan2f(sqrt(dx dx + dz dz), dy), atan2f(dx, dz),
    /// 0, 0) as 16-bit angles (the pitch +0, the yaw +4).
    pub fn look_angles(&mut self, a: u32, b: u32, out: u32) {
        let sp = self.enter(0x30);
        let dx = ee::sub(self.mem.read_f32(a), self.mem.read_f32(b));
        let dz = ee::sub(self.mem.read_f32(a + 8), self.mem.read_f32(b + 8));
        let dy = ee::sub(self.mem.read_f32(a + 4), self.mem.read_f32(b + 4));
        let yaw = self.atan2f(dx, dz);
        let v = self.angle16(yaw, 0x0010_3E24);
        self.mem.write_u32(out + 4, v);
        let h = self.sqrtf(ee::add(ee::mul(dx, dx), ee::mul(dz, dz)));
        let pitch = self.atan2f(h, dy);
        let v = self.angle16(pitch, 0x0010_3E7C);
        self.mem.write_u32(out, v);
        self.mem.write_u32(out + 0xC, 0);
        self.mem.write_u32(out + 8, 0);
        let _ = sp;
        self.leave(0x30);
    }

    /// 0x103290(out, id) [F]: *out = 0x202530(5, id); returns 0.
    pub fn texture5(&mut self, out: u32, id: i32) -> i32 {
        self.enter(0x20);
        let t = self.texture(5, id);
        self.mem.write_u32(out, t);
        self.leave(0x20);
        0
    }

    /// 0x103260(out, id) [F]: *out = 0x202530(4, id) (TEXEFF); returns 0.
    pub fn texture4(&mut self, out: u32, id: i32) -> i32 {
        self.enter(0x20);
        let t = self.texture(4, id);
        self.mem.write_u32(out, t);
        self.leave(0x20);
        0
    }

    /// 0x1FACE0(index) [F]: the camera object `index`'s +0x20 (its eye),
    /// 0 for a missing object or an index other than 0.
    pub fn camera_eye(&mut self, index: i32) -> u32 {
        if index != 0 {
            return 0;
        }
        let cam = self.mem.read_u32(crate::exe::CAMERA_TABLE);
        if cam == 0 {
            0
        } else {
            cam.wrapping_add(0x20)
        }
    }
}
