//! The executable's tanf and sinf (newlib-derived, single precision, with
//! their kernels and the argument reduction) and powf's y = 2 case,
//! hand-ported (helper A, 30 Oct 2026; c401 uses tanf through 0x100D50,
//! the second effect library's degree table 0x1084F0 sinf). Every float operation
//! goes through [`crate::ee`] in the original's order, so the bits are the
//! interpreter's. atanf / atan2f / sqrtf are in [`crate::libm`].
//!
//! The polynomial and reduction tables are read from the user's executable
//! at runtime (tan T 0x2BD950, npio2_hw 0x2BD844) [F 0x1A6BD8 / 0x1A5BF8].
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

/// __kernel_tanf's T[0..12] [F 0x1A6CD4].
const TAN_T: u32 = 0x002B_D950;
/// rem_pio2f's npio2_hw table (indexed by n, read at +4 n) [F 0x1A5DCC].
const NPIO2_HW: u32 = 0x002B_D844;

fn bits(x: f32) -> i32 {
    x.to_bits() as i32
}
fn from(b: i32) -> f32 {
    f32::from_bits(b as u32)
}

impl Fx {
    /// 0x1A7508 powf(x, 2.0) -> 0x1A53D8 [F]: with y = 2 (hy 0x40000000)
    /// the function returns x * x after its sign tests (no wrapper, no
    /// error path). Only this case is ported: every caller passes 2.0.
    pub fn powf2(x: f32) -> f32 {
        ee::mul(x, x)
    }

    /// 0x1A7508 powf(x, y) -> 0x1A53D8 __ieee754_powf [F] (helper D20, 3
    /// Nov 2026): newlib's e_powf as built for the EE (no NaN, infinity or
    /// subnormal tests: |y| below 2^-126 gives 1, |x| below it is a zero),
    /// with the build's own x - 1 (not |x| - 1) on the |y| > 2^27 path.
    /// Every float operation goes through [`crate::ee`] in the compiled
    /// order; the tables bp / dp_h / dp_l (0x2BD4A8 / 0x2BD4B0 / 0x2BD4B8)
    /// and huge * huge (0x2BD52C) are the values written here. The stack
    /// temporaries and fabsf's argument are not written.
    pub fn powf(&mut self, x: f32, y: f32) -> f32 {
        let f = f32::from_bits;
        let mask = |v: f32| f32::from_bits(v.to_bits() & 0xFFFF_F000);
        let (hx, hy) = (bits(x), bits(y));
        let (ix, iy) = (hx & 0x7FFF_FFFF, hy & 0x7FFF_FFFF);
        if iy <= 0x7F_FFFF {
            return 1.0;
        }
        let mut yisint = 0;
        if hx < 0 {
            if iy > 0x4B7F_FFFF {
                yisint = 2;
            } else if iy > 0x3F7F_FFFF {
                let sh = (0x96 - (iy >> 23)) as u32 & 31;
                let j = iy >> sh;
                if j.wrapping_shl(sh) == iy {
                    yisint = 2 - (j & 1);
                }
            }
        }
        if iy == 0x3F80_0000 {
            return if hy < 0 { ee::div(1.0, x) } else { x };
        }
        if hy == 0x4000_0000 {
            return ee::mul(x, x);
        }
        if hy == 0x3F00_0000 && hx >= 0 {
            return self.sqrtf(x);
        }
        let ax = from(ix);
        if ix <= 0x7F_FFFF || ix == 0x3F80_0000 {
            let mut z = ax;
            if hy < 0 {
                z = ee::div(1.0, z);
            }
            if hx < 0 {
                if (ix.wrapping_sub(0x3F80_0000) | yisint) == 0 {
                    let d = ee::sub(z, z);
                    z = ee::div(d, d);
                } else if yisint == 1 {
                    z = ee::neg(z);
                }
            }
            return z;
        }
        let n0 = ((hx as u32) >> 31) as i32 - 1;
        if (n0 | yisint) == 0 {
            let d = ee::sub(x, x);
            return ee::div(d, d);
        }
        let huge2 = f(0x7F7F_FFFF);
        let (t1, t2);
        if iy > 0x4D00_0000 {
            if ix <= 0x3F7F_FFF7 {
                return if hy >= 0 { 0.0 } else { huge2 };
            }
            if ix > 0x3F80_0007 {
                return if hy > 0 { huge2 } else { 0.0 };
            }
            let t = ee::sub(x, 1.0);
            let a = ee::mul(t, 0.25);
            let tt = ee::mul(t, t);
            let tl = ee::mul(t, f(0x36EC_A570));
            let u = ee::mul(t, f(0x3FB8_AA00));
            let w = ee::mul(tt, ee::sub(0.5, ee::mul(t, ee::sub(f(0x3EAA_AAAB), a))));
            let v = ee::sub(tl, ee::mul(w, f(0x3FB8_AA3B)));
            t1 = mask(ee::add(u, v));
            t2 = ee::sub(v, ee::sub(t1, u));
        } else {
            let mut n = (ix >> 23) - 0x7F;
            let j = ix & 0x7F_FFFF;
            let mut ixn = j | 0x3F80_0000;
            let mut k = 0usize;
            if j > 0x1C_C471 {
                if j <= 0x5D_B3D6 {
                    k = 1;
                } else {
                    n = (ix >> 23) - 0x7E;
                    ixn = ixn.wrapping_sub(0x80_0000);
                }
            }
            let bp = [1.0f32, 1.5][k];
            let dp_h = [0.0f32, f(0x3F15_C000)][k];
            let dp_l = [0.0f32, f(0x35D1_CFDC)][k];
            let ax = from(ixn);
            let u = ee::sub(ax, bp);
            let v = ee::div(1.0, ee::add(ax, bp));
            let s = ee::mul(u, v);
            let s_h = mask(s);
            let t_h = from(((ixn >> 1) | 0x2000_0000).wrapping_add((k as i32) << 21).wrapping_add(0x4_0000));
            let t_l = ee::sub(ax, ee::sub(t_h, bp));
            let s_l = ee::mul(v, ee::sub(ee::sub(u, ee::mul(s_h, t_h)), ee::mul(s_h, t_l)));
            let s2 = ee::mul(s, s);
            let mut p = ee::mul(s2, f(0x3E53_F142));
            for c in [0x3E6C_3255u32, 0x3E8B_A305, 0x3EAA_AAAB, 0x3EDB_6DB7] {
                p = ee::mul(s2, ee::add(p, f(c)));
            }
            p = ee::add(p, f(0x3F19_999A));
            let r = ee::add(ee::mul(ee::mul(s2, s2), p), ee::mul(s_l, ee::add(s_h, s)));
            let s2 = ee::mul(s_h, s_h);
            let t_h = mask(ee::add(ee::add(s2, 3.0), r));
            let t_l = ee::sub(r, ee::sub(ee::sub(t_h, 3.0), s2));
            let u = ee::mul(s_h, t_h);
            let v = ee::add(ee::mul(s_l, t_h), ee::mul(t_l, s));
            let p_h = mask(ee::add(u, v));
            let p_l = ee::sub(v, ee::sub(p_h, u));
            let z_h = ee::mul(f(0x3F76_3800), p_h);
            let z_l = ee::add(ee::add(ee::mul(f(0x369D_C3A0), p_h), ee::mul(p_l, f(0x3F76_384F))), dp_l);
            let t = ee::from_i32(n);
            t1 = mask(ee::add(ee::add(ee::add(z_h, z_l), dp_h), t));
            t2 = ee::sub(z_l, ee::sub(ee::sub(ee::sub(t1, t), dp_h), z_h));
        }
        let s = if (n0 | (yisint - 1)) == 0 { -1.0 } else { 1.0 };
        let y1 = mask(y);
        let p_l = ee::add(ee::mul(ee::sub(y, y1), t1), ee::mul(y, t2));
        let mut p_h = ee::mul(y1, t1);
        let z = ee::add(p_l, p_h);
        let j = bits(z);
        let i = j & 0x7FFF_FFFF;
        let (huge, tiny) = (f(0x7149_F2CA), f(0x0DA2_4260));
        if j > 0 {
            if i > 0x4301_0000 || (i == 0x4301_0000 && crate::exe::ee_lt(ee::sub(z, p_h), ee::add(p_l, f(0x3338_AA3C)))) {
                return ee::mul(ee::mul(s, huge), huge);
            }
        } else if i > 0x42FC_0000 || (i == 0x42FC_0000 && crate::exe::ee_le(p_l, ee::sub(z, p_h))) {
            return ee::mul(ee::mul(s, tiny), tiny);
        }
        let mut n = 0i32;
        if i > 0x3F00_0000 {
            let nn = j.wrapping_add(0x80_0000 >> (((i >> 23) - 0x7E) as u32 & 31));
            let k = ((nn as u32 >> 23) & 0xFF) as i32 - 0x7F;
            let t = from(nn & !(0x7F_FFFF >> (k as u32 & 31)));
            n = ((nn & 0x7F_FFFF) | 0x80_0000) >> ((23 - k) as u32 & 31);
            if j < 0 {
                n = n.wrapping_neg();
            }
            p_h = ee::sub(p_h, t);
        }
        let t = mask(ee::add(p_l, p_h));
        let u = ee::mul(t, f(0x3F31_7200));
        let v = ee::add(ee::mul(ee::sub(p_l, ee::sub(t, p_h)), f(0x3F31_7218)), ee::mul(t, f(0x35BF_BE8C)));
        let z = ee::add(u, v);
        let w = ee::sub(v, ee::sub(z, u));
        let tt = ee::mul(z, z);
        let mut q = ee::mul(tt, f(0x3331_BB4C));
        for c in [0xB5DD_EA0Eu32, 0x388A_B355, 0xBB36_0B61] {
            q = ee::mul(tt, ee::add(q, f(c)));
        }
        q = ee::mul(tt, ee::add(q, f(0x3E2A_AAAB)));
        let t1 = ee::sub(z, q);
        let r = ee::sub(ee::div(ee::mul(z, t1), ee::sub(t1, 2.0)), ee::add(w, ee::mul(z, w)));
        let z = ee::sub(1.0, ee::sub(r, z));
        let jj = bits(z).wrapping_add(n.wrapping_shl(23));
        let z = if (jj >> 23) <= 0 { Self::scalbnf(z, n) } else { from(jj) };
        ee::mul(s, z)
    }

    /// 0x19BDB8 scalbnf(x, n) [F]: zero as it is, else the exponent field
    /// + n; copysignf (0x19BD78) of huge times huge at 0x100 or above, of
    /// tiny times tiny at 0 or below unless n > 50000.
    fn scalbnf(x: f32, n: i32) -> f32 {
        let hx = bits(x);
        let ix = hx & 0x7FFF_FFFF;
        if ix <= 0x7F_FFFF {
            return x;
        }
        let k = (ix >> 23).wrapping_add(n);
        let big = |v: u32| ee::mul(from(((v & 0x7FFF_FFFF) | (hx as u32 & 0x8000_0000)) as i32), from(v as i32));
        if k >= 0x100 {
            return big(0x7149_F2CA);
        }
        if k > 0 {
            return from((hx & 0x807F_FFFFu32 as i32) | k.wrapping_shl(23));
        }
        if n > 50000 { big(0x7149_F2CA) } else { big(0x0DA2_4260) }
    }

    /// 0x1A7430 tanf(x) [F]: |x| <= pi / 4 (0x3F490FDA) straight to the
    /// kernel, else reduced by 0x1A5BF8 and the kernel with 1 - 2 (n & 1).
    pub fn tanf(&mut self, x: f32) -> f32 {
        let ix = bits(x) & 0x7FFF_FFFF;
        if ix <= 0x3F49_0FDA {
            return self.kernel_tanf(x, 0.0, 1);
        }
        let (n, y0, y1) = Self::rem_pio2f(self, x);
        self.kernel_tanf(y0, y1, 1 - ((n & 1) << 1))
    }

    /// 0x1A6BD8 __kernel_tanf(x, y, iy) [F] (newlib's k_tanf).
    pub fn kernel_tanf(&mut self, x: f32, y: f32, iy: i32) -> f32 {
        let hx = bits(x);
        let ix = hx & 0x7FFF_FFFF;
        let (mut x, mut y) = (x, y);
        if ix <= 0x317F_FFFF && ee::to_i32(x) == 0 {
            if (ix | iy.wrapping_add(1)) == 0 {
                return ee::div(1.0, ee::abs(x));
            }
            return if iy == 1 { x } else { ee::div(-1.0, x) };
        }
        let big = ix > 0x3F2C_A13F;
        if big {
            if hx < 0 {
                x = ee::neg(x);
                y = ee::neg(y);
            }
            let z = ee::sub(f32::from_bits(0x3F49_0FDA), x);
            let w = ee::sub(f32::from_bits(0x3322_2168), y);
            y = 0.0;
            x = ee::add(z, w);
        }
        let z = ee::mul(x, x);
        let t: [f32; 13] = std::array::from_fn(|k| self.mem.read_f32(TAN_T + 4 * k as u32));
        let w = ee::mul(z, z);
        let s = ee::mul(z, x);
        let f8 = ee::mul(w, t[12]);
        let f9 = ee::mul(w, t[11]);
        let f0 = ee::add(t[10], f8);
        let f1 = ee::add(t[9], f9);
        let t0s = ee::mul(t[0], s);
        let f0 = ee::mul(w, f0);
        let f1 = ee::mul(w, f1);
        let f2 = ee::mul(w, ee::add(t[8], f0));
        let f3 = ee::mul(w, ee::add(t[7], f1));
        let f5 = ee::mul(w, ee::add(t[6], f2));
        let f4 = ee::mul(w, ee::add(t[5], f3));
        let f6 = ee::mul(w, ee::add(t[4], f5));
        let f7 = ee::mul(w, ee::add(t[3], f4));
        let f10 = ee::add(t[2], f6);
        let f3 = ee::add(t[1], f7);
        let v = ee::mul(z, f10);
        let f0 = ee::add(f3, v);
        let f0 = ee::mul(s, f0);
        let f0 = ee::add(f0, y);
        let f0 = ee::mul(z, f0);
        let r = ee::add(ee::add(y, f0), t0s);
        let w = ee::add(x, r);
        if big {
            let v = ee::from_i32(iy);
            let f0 = ee::mul(w, w);
            let sign = ee::from_i32(1 - ((hx >> 30) & 2));
            let f1 = ee::add(w, v);
            let f0 = ee::sub(ee::div(f0, f1), r);
            let f0 = ee::sub(x, f0);
            let f0 = ee::add(f0, f0);
            return ee::mul(sign, ee::sub(v, f0));
        }
        if iy == 1 {
            return w;
        }
        let z = from(bits(w) & !0xFFF);
        let f1 = ee::sub(z, x);
        let a = ee::div(-1.0, w);
        let v = ee::sub(r, f1);
        let t = from(bits(a) & !0xFFF);
        let f4 = ee::mul(t, z);
        let f1 = ee::mul(t, v);
        let f12 = ee::add(f4, 1.0);
        let f1 = ee::add(f12, f1);
        ee::add(t, ee::mul(a, f1))
    }

    /// 0x1A5BF8 __ieee754_rem_pio2f(x, y) [F]: (n, y[0], y[1]) for |x| up
    /// to 2^7 pi / 2 (0x43490F80). [H] Larger arguments go to the
    /// Payne-Hanek kernel 0x1A6208 in the original, not ported (no ported
    /// caller reaches it: c401's and the finale's 0x100D50 arguments are
    /// at most 2 pi / 5); here they are reduced in f64 instead.
    pub fn rem_pio2f(&mut self, x: f32) -> (i32, f32, f32) {
        let hx = bits(x);
        let ix = hx & 0x7FFF_FFFF;
        let pio2_1 = f32::from_bits(0x3FC9_0F80);
        let pio2_1t = f32::from_bits(0x3735_4443);
        let pio2_2 = f32::from_bits(0x3735_4400);
        let pio2_2t = f32::from_bits(0x2E85_A308);
        if ix <= 0x3F49_0FD8 {
            return (0, x, 0.0);
        }
        if ix <= 0x4016_CBE3 {
            let near = (ix & !0xF) == 0x3FC9_0FD0;
            if hx > 0 {
                let z = ee::sub(x, pio2_1);
                let (z, t) = if near { (ee::sub(z, pio2_2), pio2_2t) } else { (z, pio2_1t) };
                let y0 = ee::sub(z, t);
                let y1 = ee::sub(ee::sub(z, y0), t);
                return (1, y0, y1);
            }
            let z = ee::add(x, pio2_1);
            let (z, t) = if near { (ee::add(z, pio2_2), pio2_2t) } else { (z, pio2_1t) };
            let y0 = ee::add(z, t);
            let y1 = ee::add(ee::sub(z, y0), t);
            return (-1, y0, y1);
        }
        if ix <= 0x4349_0F80 {
            let t = ee::abs(x);
            let n = ee::to_i32(ee::add(ee::mul(t, f32::from_bits(0x3F22_F984)), 0.5));
            let fnf = ee::from_i32(n);
            let mut r = ee::sub(t, ee::mul(fnf, pio2_1));
            let mut w = ee::mul(fnf, pio2_1t);
            let mut y0 = ee::sub(r, w);
            let quick = n < 32 && (ix & !0xFF) as u32 != self.mem.read_u32(NPIO2_HW.wrapping_add((4 * n) as u32));
            if !quick {
                let j = ix >> 23;
                let i = j - ((bits(y0) >> 23) & 0xFF);
                if i >= 9 {
                    let t = r;
                    w = ee::mul(fnf, pio2_2);
                    r = ee::sub(t, w);
                    let f0 = ee::sub(ee::sub(t, r), w);
                    w = ee::sub(ee::mul(fnf, pio2_2t), f0);
                    y0 = ee::sub(r, w);
                    let i = j - ((bits(y0) >> 23) & 0xFF);
                    if i >= 26 {
                        let t = r;
                        w = ee::mul(fnf, f32::from_bits(0x2E85_A300));
                        r = ee::sub(t, w);
                        let f0 = ee::sub(ee::sub(t, r), w);
                        w = ee::sub(ee::mul(fnf, f32::from_bits(0x248D_3132)), f0);
                        y0 = ee::sub(r, w);
                    }
                }
            }
            let y1 = ee::sub(ee::sub(r, y0), w);
            if hx < 0 {
                return (n.wrapping_neg(), ee::neg(y0), ee::neg(y1));
            }
            return (n, y0, y1);
        }
        // [H] Not the original's code (0x1A6208): an f64 reduction.
        let v = f64::from(x);
        let q = (v / std::f64::consts::FRAC_PI_2).round();
        let rem = v - q * std::f64::consts::FRAC_PI_2;
        let y0 = rem as f32;
        (q as i64 as i32, y0, (rem - f64::from(y0)) as f32)
    }
    /// 0x1A7348 sinf(x) [F] (libm's; [`Fx::sinf`] is the executable's
    /// table sine 0x20AEA0): |x| <= 0x3F490FD8 straight to the sine
    /// kernel, else reduced by 0x1A5BF8 and by n & 3: sin, cos, -sin, -cos.
    pub fn libm_sinf(&mut self, x: f32) -> f32 {
        if bits(x) & 0x7FFF_FFFF <= 0x3F49_0FD8 {
            return Self::kernel_sinf(x, 0.0, 0);
        }
        let (n, y0, y1) = self.rem_pio2f(x);
        match n & 3 {
            0 => Self::kernel_sinf(y0, y1, 1),
            1 => Self::kernel_cosf(y0, y1),
            2 => ee::neg(Self::kernel_sinf(y0, y1, 1)),
            _ => ee::neg(Self::kernel_cosf(y0, y1)),
        }
    }

    /// 0x1A6AB0 __kernel_sinf(x, y, iy) [F] (newlib's k_sinf, its
    /// coefficients as the code loads them).
    pub fn kernel_sinf(x: f32, y: f32, iy: i32) -> f32 {
        let ix = bits(x) & 0x7FFF_FFFF;
        if ix <= 0x31FF_FFFF && ee::to_i32(x) == 0 {
            return x;
        }
        let z = ee::mul(x, x);
        let v = ee::mul(z, x);
        let mut f0 = ee::mul(z, from(0x2F2E_C9D3));
        f0 = ee::add(f0, from(0xB2D7_2F34u32 as i32));
        f0 = ee::add(ee::mul(z, f0), from(0x3638_EF1B));
        f0 = ee::add(ee::mul(z, f0), from(0xB950_0D01u32 as i32));
        let r = ee::add(ee::mul(z, f0), from(0x3C08_8889));
        let s1 = from(0xBE2A_AAABu32 as i32);
        if iy == 0 {
            let f0 = ee::add(ee::mul(z, r), s1);
            return ee::add(x, ee::mul(v, f0));
        }
        let f2 = ee::mul(v, r);
        let f0 = ee::mul(y, 0.5);
        let f1 = ee::mul(v, s1);
        let f0 = ee::sub(ee::mul(z, ee::sub(f0, f2)), y);
        ee::sub(x, ee::sub(f0, f1))
    }

    /// 0x1A6098 __kernel_cosf(x, y) [F] (newlib's k_cosf).
    pub fn kernel_cosf(x: f32, y: f32) -> f32 {
        let ix = bits(x) & 0x7FFF_FFFF;
        if ix <= 0x31FF_FFFF && ee::to_i32(x) == 0 {
            return 1.0;
        }
        let z = ee::mul(x, x);
        let mut f0 = ee::mul(z, from(0xAD47_D74Eu32 as i32));
        f0 = ee::add(f0, from(0x310F_74F6));
        f0 = ee::add(ee::mul(z, f0), from(0xB493_F27Cu32 as i32));
        f0 = ee::add(ee::mul(z, f0), from(0x37D0_0D01));
        f0 = ee::add(ee::mul(z, f0), from(0xBAB6_0B61u32 as i32));
        f0 = ee::add(ee::mul(z, f0), from(0x3D2A_AAAB));
        let f1 = ee::mul(z, f0);
        if ix <= 0x3E99_9999 {
            let f1 = ee::mul(z, f1);
            let f2 = ee::mul(x, y);
            let f0 = ee::mul(z, 0.5);
            let f0 = ee::sub(f0, ee::sub(f1, f2));
            return ee::sub(1.0, f0);
        }
        let qx = if ix > 0x3F48_0000 { 0.28125 } else { from(ix.wrapping_sub(0x0100_0000)) };
        let f2 = ee::mul(z, f1);
        let f3 = ee::mul(x, y);
        let f0 = ee::mul(z, 0.5);
        let a = ee::sub(1.0, qx);
        let f2 = ee::sub(f2, f3);
        let f0 = ee::sub(ee::sub(f0, qx), f2);
        ee::sub(a, f0)
    }
}

/// c.eq.s with the EE's clamped operands.
pub fn ee_eq(a: f32, b: f32) -> bool {
    let fin = |x: f32| if x.is_nan() { if x.is_sign_negative() { -f32::MAX } else { f32::MAX } } else if x.is_infinite() { x.signum() * f32::MAX } else { x };
    fin(a) == fin(b)
}
