//! The EE's arithmetic as the effect programs see it: COP1 single precision
//! and VU0 macro floats with the rounding of
//! `roses_assets::opening_scene::ee_float` (PCSX2's), clamped overflow (no
//! infinities or NaNs) and flushed denormals; the executable's soft-float
//! conversions; newlib's rand. Bit for bit the `roses-ee` interpreter's
//! (its `fpu` module), so a port that does the same operations in the same
//! order gets the same bits.
use roses_assets::opening_scene::ee_float::{ee, vu};

fn clamp(x: f64) -> f32 {
    if x.is_nan() {
        return 0.0;
    }
    if x >= f64::from(f32::MAX) {
        return f32::MAX;
    }
    if x <= -f64::from(f32::MAX) {
        return -f32::MAX;
    }
    let f = x as f32;
    if f != 0.0 && f.abs() < f32::MIN_POSITIVE {
        if f.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else {
        f
    }
}

fn fin(x: f32) -> f32 {
    if x.is_nan() {
        if x.is_sign_negative() {
            -f32::MAX
        } else {
            f32::MAX
        }
    } else if x.is_infinite() {
        if x > 0.0 {
            f32::MAX
        } else {
            -f32::MAX
        }
    } else {
        x
    }
}

fn div_zero(a: f32, b: f32) -> f32 {
    if a.is_sign_negative() != b.is_sign_negative() {
        -f32::MAX
    } else {
        f32::MAX
    }
}

/// add.s
pub fn add(a: f32, b: f32) -> f32 {
    let (a, b) = (fin(a), fin(b));
    ee::add(a, b).unwrap_or_else(|_| clamp(f64::from(a) + f64::from(b)))
}
/// sub.s
pub fn sub(a: f32, b: f32) -> f32 {
    let (a, b) = (fin(a), fin(b));
    ee::sub(a, b).unwrap_or_else(|_| clamp(f64::from(a) - f64::from(b)))
}
/// mul.s
pub fn mul(a: f32, b: f32) -> f32 {
    let (a, b) = (fin(a), fin(b));
    ee::mul(a, b).unwrap_or_else(|_| clamp(f64::from(a) * f64::from(b)))
}
/// div.s
pub fn div(a: f32, b: f32) -> f32 {
    let (a, b) = (fin(a), fin(b));
    if b == 0.0 {
        return div_zero(a, b);
    }
    ee::div(a, b).unwrap_or_else(|_| clamp(f64::from(a) / f64::from(b)))
}
/// sqrt.s
pub fn sqrt(b: f32) -> f32 {
    clamp(f64::from(fin(b).abs()).sqrt())
}
/// neg.s (the sign bit flipped).
pub fn neg(a: f32) -> f32 {
    f32::from_bits(a.to_bits() ^ 0x8000_0000)
}
/// abs.s
pub fn abs(a: f32) -> f32 {
    f32::from_bits(a.to_bits() & 0x7FFF_FFFF)
}
/// cvt.s.w
pub fn from_i32(v: i32) -> f32 {
    ee::from_i32(v).unwrap_or(v as f32)
}
/// cvt.w.s (truncating, saturating).
pub fn to_i32(a: f32) -> i32 {
    let a = fin(a);
    if a >= 2_147_483_648.0 {
        i32::MAX
    } else if a <= -2_147_483_648.0 {
        i32::MIN
    } else {
        a.trunc() as i32
    }
}

/// VU0 macro add.
pub fn vadd(a: f32, b: f32) -> f32 {
    let (a, b) = (fin(a), fin(b));
    vu::add(a, b).unwrap_or_else(|_| clamp(f64::from(a) + f64::from(b)))
}
pub fn vsub(a: f32, b: f32) -> f32 {
    let (a, b) = (fin(a), fin(b));
    vu::sub(a, b).unwrap_or_else(|_| clamp(f64::from(a) - f64::from(b)))
}
pub fn vmul(a: f32, b: f32) -> f32 {
    let (a, b) = (fin(a), fin(b));
    vu::mul(a, b).unwrap_or_else(|_| clamp(f64::from(a) * f64::from(b)))
}
pub fn vmadd(acc: f32, a: f32, b: f32) -> f32 {
    vadd(acc, vmul(a, b))
}
pub fn vmsub(acc: f32, a: f32, b: f32) -> f32 {
    vsub(acc, vmul(a, b))
}
pub fn vdiv(a: f32, b: f32) -> f32 {
    let (a, b) = (fin(a), fin(b));
    if b == 0.0 {
        return div_zero(a, b);
    }
    vu::div(a, b).unwrap_or_else(|_| clamp(f64::from(a) / f64::from(b)))
}
pub fn vrsqrt(a: f32, b: f32) -> f32 {
    let (a, b) = (fin(a), fin(b));
    if b == 0.0 {
        return div_zero(a, 1.0);
    }
    vu::rsqrt(a, b).unwrap_or_else(|_| clamp(f64::from(a) / f64::from(b.abs()).sqrt()))
}

/// 0x1A2C70 (float -> unsigned int, the executable's soft-float) [F]: zero
/// exponent field, a sign or an exponent below 0 give 0; 32 and above
/// 0xFFFFFFFF; else the 1.23 mantissa at bit 30 shifted right by 30 - e
/// (left by 1 for e = 31), truncating.
pub fn fixunssfsi(v: f32) -> u32 {
    let bits = v.to_bits();
    let field = (bits >> 23) & 0xFF;
    if field == 0 || bits >> 31 != 0 {
        return 0;
    }
    let e = field as i32 - 127;
    if e < 0 {
        return 0;
    }
    if e >= 32 {
        return u32::MAX;
    }
    let m = ((bits & 0x7F_FFFF) << 7) | 0x4000_0000;
    if e < 31 {
        m >> (30 - e)
    } else {
        m << 1
    }
}

/// 0x1A2B80 (float -> int) [F]: zero exponent field or e < 0 give 0; e >= 31
/// saturates by sign; else the mantissa shifted right by 30 - e, negated
/// for a sign.
pub fn fixsfsi(v: f32) -> i32 {
    let bits = v.to_bits();
    let field = (bits >> 23) & 0xFF;
    let negative = bits >> 31 != 0;
    if field == 0 {
        return 0;
    }
    let e = field as i32 - 127;
    if e < 0 {
        return 0;
    }
    if e >= 31 {
        return if negative { i32::MIN } else { i32::MAX };
    }
    let v = ((((bits & 0x7F_FFFF) << 7) | 0x4000_0000) >> (30 - e)) as i32;
    if negative {
        v.wrapping_neg()
    } else {
        v
    }
}

/// newlib rand 0x19AE88: the u64 state at *(0x28ED18) + 0xA8 [F].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rand(pub u64);

impl Rand {
    /// 0..0x7FFFFFFF.
    pub fn next(&mut self) -> i32 {
        self.0 = self.0.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
        ((self.0 >> 32) & 0x7FFF_FFFF) as i32
    }
}
