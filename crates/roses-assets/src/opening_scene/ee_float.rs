//! Single-precision arithmetic as PCSX2 v2.8.2 executes it for SLES-51579.
//!
//! The captured Simon scene was rendered by PCSX2, so bit-exact reproduction
//! needs the emulator's float behaviour rather than IEEE round-to-nearest:
//!
//! * EE COP1 `add/sub/mul/madd/msub`: rounded toward +infinity (GameDB
//!   `eeRoundMode: 2` for SLES-51579), with the recompiler's `FPU_ADD_SUB`
//!   operand alignment for add/sub (the operand with the smaller exponent loses
//!   its low `diff - 1` fraction bits; 25 or more binades apart it becomes ±0).
//! * EE COP1 `div`: rounded to nearest (`FPUDiv.Roundmode = 0` in the ini).
//! * VU0 macro-mode (COP2) ops run inside EE blocks, so they use the EE's
//!   MXCSR: +infinity, without the add/sub alignment. `RSQRT` is a rounded
//!   square root followed by a rounded division.
//! * Denormal results flush to zero and overflow clamps (PCSX2 clamp modes).
//!
//! All of this was established by replaying the game's own camera/light/model
//! routines from a saved PAL memory image inside an interpreter with these
//! rules, which reproduced every captured matrix bit for bit
//! (`work/research/story/claude-opening-disc/tools/simon_oracle.py`).
//! Real R5900/VU0 hardware is not IEEE and is not modelled here.
use crate::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Round {
    Nearest,
    Up,
}

fn invalid(message: &str) -> Error {
    Error::Invalid(format!("PCSX2 float model: {message}"))
}

/// Round the exact value `hi + lo` (|lo| far below one f64 ulp of `hi`, so
/// `lo` only tells the direction of the residual) to f32 under `mode`.
fn round_hi_lo(hi: f64, lo: f64, mode: Round) -> Result<f32> {
    if !hi.is_finite() || !lo.is_finite() {
        return Err(invalid("non-finite intermediate"));
    }
    let nearest = hi as f32;
    let candidate = if f64::from(nearest) == hi {
        match mode {
            Round::Nearest => nearest,
            Round::Up => {
                if lo > 0.0 {
                    next_up(nearest)
                } else {
                    nearest
                }
            }
        }
    } else {
        // `hi` lies strictly between two adjacent f32 values.
        let (lower, upper) = if f64::from(nearest) > hi {
            (next_down(nearest), nearest)
        } else {
            (nearest, next_up(nearest))
        };
        match mode {
            Round::Up => upper,
            Round::Nearest => {
                let below = hi - f64::from(lower);
                let above = f64::from(upper) - hi;
                if below < above {
                    lower
                } else if below > above {
                    upper
                } else if lo > 0.0 {
                    upper
                } else if lo < 0.0 {
                    lower
                } else if lower.to_bits() & 1 == 0 {
                    lower
                } else {
                    upper
                }
            }
        }
    };
    if candidate.is_infinite() {
        return Ok(if candidate > 0.0 { f32::MAX } else { -f32::MAX });
    }
    if candidate != 0.0 && candidate.abs() < f32::MIN_POSITIVE {
        return Ok(if candidate.is_sign_negative() {
            -0.0
        } else {
            0.0
        });
    }
    Ok(candidate)
}

fn next_up(x: f32) -> f32 {
    if x == f32::NEG_INFINITY {
        // A negative overflow rounded up lands on -FLT_MAX (PCSX2 then
        // clamps the result with its sign).
        return -f32::MAX;
    }
    if x == f32::MAX || x.is_infinite() {
        return f32::INFINITY;
    }
    let bits = x.to_bits();
    if x == 0.0 {
        f32::from_bits(1)
    } else if x > 0.0 {
        f32::from_bits(bits + 1)
    } else {
        f32::from_bits(bits - 1)
    }
}

fn next_down(x: f32) -> f32 {
    -next_up(-x)
}

fn check(x: f32) -> Result<f32> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(invalid("non-finite operand"))
    }
}

/// PCSX2 `FPU_ADD_SUB`: drop the low `diff - 1` fraction bits of the operand
/// with the smaller exponent; 25 or more binades apart it keeps only its sign.
fn align(a: f32, b: f32) -> (f32, f32) {
    let (ba, bb) = (a.to_bits(), b.to_bits());
    let (ea, eb) = (((ba >> 23) & 0xff) as i32, ((bb >> 23) & 0xff) as i32);
    let mask_low = |bits: u32, diff: i32| -> u32 {
        if diff >= 25 {
            bits & 0x8000_0000
        } else if diff >= 1 {
            bits & (u32::MAX << (diff - 1))
        } else {
            bits
        }
    };
    let diff = ea - eb;
    if diff > 0 {
        (a, f32::from_bits(mask_low(bb, diff)))
    } else if diff < 0 {
        (f32::from_bits(mask_low(ba, -diff)), b)
    } else {
        (a, b)
    }
}

fn add_impl(a: f32, b: f32, mode: Round, aligned: bool) -> Result<f32> {
    let (a, b) = if aligned {
        align(check(a)?, check(b)?)
    } else {
        (check(a)?, check(b)?)
    };
    let (fa, fb) = (f64::from(a), f64::from(b));
    let sum = fa + fb;
    // Knuth's TwoSum: the rounding error of the f64 addition is exact.
    let bv = sum - fa;
    let err = (fa - (sum - bv)) + (fb - bv);
    round_hi_lo(sum, err, mode)
}

fn mul_impl(a: f32, b: f32, mode: Round) -> Result<f32> {
    // Two 24-bit significands multiply exactly inside f64.
    round_hi_lo(f64::from(check(a)?) * f64::from(check(b)?), 0.0, mode)
}

fn div_impl(a: f32, b: f32, mode: Round) -> Result<f32> {
    let (fa, fb) = (f64::from(check(a)?), f64::from(check(b)?));
    if fb == 0.0 {
        return Err(invalid("division by zero"));
    }
    let q = fa / fb;
    // Exact residual of the correctly rounded quotient.
    let residual = (-q).mul_add(fb, fa);
    round_hi_lo(q, residual / fb, mode)
}

fn sqrt_impl(a: f32, mode: Round) -> Result<f32> {
    let fa = f64::from(check(a)?);
    if fa < 0.0 {
        return Err(invalid("square root of a negative value"));
    }
    if fa == 0.0 {
        return Ok(a);
    }
    let s = fa.sqrt();
    let residual = (-s).mul_add(s, fa);
    round_hi_lo(s, residual / (2.0 * s), mode)
}

/// EE COP1 operations.
pub mod ee {
    use super::{add_impl, div_impl, mul_impl, round_hi_lo, Result, Round};

    pub fn add(a: f32, b: f32) -> Result<f32> {
        add_impl(a, b, Round::Up, true)
    }
    pub fn sub(a: f32, b: f32) -> Result<f32> {
        add_impl(a, -b, Round::Up, true)
    }
    pub fn mul(a: f32, b: f32) -> Result<f32> {
        mul_impl(a, b, Round::Up)
    }
    /// `mula.s`/`madd.s`: the product is rounded, then added to the accumulator.
    pub fn madd(acc: f32, a: f32, b: f32) -> Result<f32> {
        add(acc, mul(a, b)?)
    }
    pub fn msub(acc: f32, a: f32, b: f32) -> Result<f32> {
        sub(acc, mul(a, b)?)
    }
    pub fn div(a: f32, b: f32) -> Result<f32> {
        div_impl(a, b, Round::Nearest)
    }
    /// `cvt.s.w`.
    pub fn from_i32(value: i32) -> Result<f32> {
        round_hi_lo(f64::from(value), 0.0, Round::Up)
    }
    /// `neg.s`: a sign flip, exact.
    pub fn neg(a: f32) -> f32 {
        -a
    }
    /// gcc `__fixsfsi`: truncation toward zero.
    pub fn to_i32(a: f32) -> Result<i32> {
        if !a.is_finite() || a.abs() >= 2_147_483_648.0 {
            return Err(super::invalid("float to int overflow"));
        }
        Ok(a.trunc() as i32)
    }
    /// gcc `__fixunssfsi`: truncation toward zero of a non-negative value.
    pub fn to_u32(a: f32) -> Result<u32> {
        if !a.is_finite() || a < 0.0 || a >= 4_294_967_296.0 {
            return Err(super::invalid("float to unsigned int overflow"));
        }
        Ok(a.trunc() as u32)
    }
}

/// VU0 macro-mode operations (EE rounding mode, no add/sub alignment).
pub mod vu {
    use super::{add_impl, div_impl, mul_impl, sqrt_impl, Result, Round};

    pub fn add(a: f32, b: f32) -> Result<f32> {
        add_impl(a, b, Round::Up, false)
    }
    pub fn sub(a: f32, b: f32) -> Result<f32> {
        add_impl(a, -b, Round::Up, false)
    }
    pub fn mul(a: f32, b: f32) -> Result<f32> {
        mul_impl(a, b, Round::Up)
    }
    pub fn madd(acc: f32, a: f32, b: f32) -> Result<f32> {
        add(acc, mul(a, b)?)
    }
    pub fn div(a: f32, b: f32) -> Result<f32> {
        div_impl(a, b, Round::Up)
    }
    /// `RSQRT Q, a, b` = a / sqrt(|b|), each step rounded.
    pub fn rsqrt(a: f32, b: f32) -> Result<f32> {
        div(a, sqrt_impl(b.abs(), Round::Up)?)
    }
}

/// The executable's software `sqrtf` (newlib `__ieee754_sqrtf`, EE 0x1A5FC0):
/// an integer digit-by-digit root with the final `q += q & 1` sticky rounding.
/// Only normal positive inputs are supported.
pub fn sqrtf(x: f32) -> Result<f32> {
    let ix = x.to_bits();
    if ix & 0x7fff_ffff > 0x7f7f_ffff {
        return Err(invalid("sqrtf of a non-finite value"));
    }
    if x <= 0.0 {
        if ix & 0x7fff_ffff == 0 {
            return Ok(x);
        }
        return Err(invalid("sqrtf of a negative value"));
    }
    let mut m = ((ix >> 23) & 0xff) as i32;
    if m == 0 {
        return Err(invalid("sqrtf of a denormal value"));
    }
    m -= 127;
    let mut ix = (ix & 0x007f_ffff) | 0x0080_0000;
    if m & 1 != 0 {
        ix += ix;
    }
    m >>= 1;
    ix += ix;
    let (mut q, mut s, mut r) = (0u32, 0u32, 0x0100_0000u32);
    while r != 0 {
        let t = s + r;
        if t <= ix {
            s = t + r;
            ix -= t;
            q += r;
        }
        ix += ix;
        r >>= 1;
    }
    if ix != 0 {
        q += q & 1;
    }
    let bits = ((q >> 1) as i32 + 0x3f00_0000 + (m << 23)) as u32;
    Ok(f32::from_bits(bits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_products_and_sums_upward() {
        // 0.94 (0x3f70a3d7) * 1.4285709857940674 (0x3fb6db6a): the exact
        // product is not representable; +infinity rounding picks the upper
        // neighbour, which is what the captured projection needs.
        let product = ee::mul(f32::from_bits(0x3f70_a3d7), f32::from_bits(0x3fb6_db6a)).unwrap();
        let exact = f64::from(f32::from_bits(0x3f70_a3d7)) * f64::from(f32::from_bits(0x3fb6_db6a));
        assert!(f64::from(product) >= exact && f64::from(next_down(product)) < exact);
        assert_eq!(ee::mul(2.0, 3.0).unwrap(), 6.0);
        assert_eq!(vu::add(1.0, f32::from_bits(1)).unwrap(), next_up(1.0));
        assert_eq!(ee::div(1.0, 3.0).unwrap(), 1.0f32 / 3.0);
        assert_eq!(ee::from_i32(-64).unwrap(), -64.0);
        assert_eq!(ee::to_i32(17.51).unwrap(), 17);
    }

    #[test]
    fn add_alignment_drops_far_operands() {
        // 25 or more binades apart the small operand vanishes; within 24 it
        // loses only its low bits.
        assert_eq!(ee::add(1.0, 1e-30).unwrap(), 1.0);
        assert_eq!(ee::sub(1.0, 1e-30).unwrap(), 1.0);
        let small = f32::from_bits(0x3f80_0001) * 2f32.powi(-10);
        let (_, masked) = align(1.0, small);
        assert_eq!(masked, 2f32.powi(-10));
        assert!(vu::add(1.0, 1e-30).unwrap() > 1.0);
        assert_eq!(vu::add(-0.0, 0.0).unwrap().to_bits(), 0);
        assert_eq!(vu::mul(-2.0, 0.0).unwrap().to_bits(), 0x8000_0000);
    }

    #[test]
    fn software_sqrt_matches_exact_roots() {
        for x in [
            1.0f32,
            4.0,
            2.0,
            0.5,
            512.0 * 512.0,
            3.0e-5,
            123456.7,
            262_144.0 + 4096.0,
        ] {
            let root = sqrtf(x).unwrap();
            let exact = f64::from(x).sqrt();
            let error = (f64::from(root) - exact).abs();
            assert!(error <= f64::from(next_up(root) - root), "{x}");
        }
        assert_eq!(sqrtf(0.0).unwrap(), 0.0);
        assert!(sqrtf(-1.0).is_err());
        assert!(vu::rsqrt(1.0, 4.0).unwrap() == 0.5);
    }
}
