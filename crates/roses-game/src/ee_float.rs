//! Exact single-precision arithmetic models for the original c400 particle
//! spawn code (mul.s / div.s / add.s on the EE).
//!
//! Verified model (`Pcsx2PositiveRounding`): PCSX2 v2.8.2 with its GameDB
//! entry for SLES-51579 (`eeRoundMode: 2`, round toward +infinity) recompiles
//! `mul.s` as an x86 `mulss` in that rounding mode and `add.s` through its
//! `FPU_ADD_SUB` guard-bit emulation (the operand with the smaller exponent
//! loses its low `diff-1` mantissa bits, operands 25 or more exponents apart
//! are replaced by a signed zero) before an `addss` in the same mode. All 768
//! captured particle scales of the reference save state reproduce bit-exactly
//! under this model and under no plain IEEE rounding mode (see
//! `work/research/story/claude-opening-disc/CLOUDS-START.md`).
//!
//! This is emulator behaviour, not proven R5900 hardware behaviour. The
//! `HostF32` model keeps the host's round-to-nearest arithmetic for comparison.
//! Division is only supported by a power of two (exact in every mode).
//! Unsupported inputs (NaN, infinities, results outside the normal range,
//! exponent gaps the exact adder does not cover) return errors, never guesses.

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FloatModel {
    /// Host IEEE round-to-nearest-even; not capture-exact.
    HostF32,
    /// PCSX2 EE recompiler with GameDB round-toward-positive-infinity.
    #[default]
    Pcsx2PositiveRounding,
}

/// Value = (-1)^neg * mant * 2^exp, mant unnormalized.
#[derive(Clone, Copy, Debug)]
struct Exact {
    neg: bool,
    mant: u128,
    exp: i32,
}

fn unpack(v: f32) -> Result<Exact> {
    if !v.is_finite() {
        return Err("EE float model: non-finite operand".into());
    }
    let bits = v.to_bits();
    let neg = bits >> 31 != 0;
    let field = ((bits >> 23) & 0xff) as i32;
    let frac = u128::from(bits & 0x7f_ffff);
    if field == 0 {
        // PCSX2 runs with denormals-are-zero for the EE FPU.
        return Ok(Exact {
            neg,
            mant: 0,
            exp: 0,
        });
    }
    Ok(Exact {
        neg,
        mant: frac | (1 << 23),
        exp: field - 127 - 23,
    })
}

/// Round toward +infinity and pack as a normal f32 (or zero).
fn pack_positive(neg: bool, mut mant: u128, mut exp: i32) -> Result<f32> {
    if mant == 0 {
        return Ok(0.0);
    }
    let width = 128 - mant.leading_zeros() as i32;
    if width > 24 {
        let shift = (width - 24) as u32;
        let dropped = mant & ((1u128 << shift) - 1);
        mant >>= shift;
        exp += shift as i32;
        if !neg && dropped != 0 {
            mant += 1;
            if mant == 1 << 24 {
                mant >>= 1;
                exp += 1;
            }
        }
    } else if width < 24 {
        let shift = (24 - width) as u32;
        mant <<= shift;
        exp -= shift as i32;
    }
    let field = exp + 23 + 127;
    if !(1..=254).contains(&field) {
        return Err("EE float model: result outside the normal f32 range".into());
    }
    let bits = (u32::from(neg) << 31) | ((field as u32) << 23) | (mant as u32 & 0x7f_ffff);
    Ok(f32::from_bits(bits))
}

fn mul_positive(a: f32, b: f32) -> Result<f32> {
    let (a, b) = (unpack(a)?, unpack(b)?);
    pack_positive(a.neg != b.neg, a.mant * b.mant, a.exp + b.exp)
}

fn clear_low_mantissa_bits(v: f32, bits: u32) -> f32 {
    if bits == 0 {
        return v;
    }
    let mask = if bits >= 23 {
        !0x7f_ffffu32
    } else {
        !((1u32 << bits) - 1)
    };
    f32::from_bits(v.to_bits() & mask)
}

/// PCSX2 `FPU_ADD_SUB` operand masking, on raw exponent fields.
fn pcsx2_mask_operands(a: f32, b: f32) -> (f32, f32) {
    let ea = ((a.to_bits() >> 23) & 0xff) as i32;
    let eb = ((b.to_bits() >> 23) & 0xff) as i32;
    let diff = ea - eb;
    let signed_zero = |v: f32| f32::from_bits(v.to_bits() & 0x8000_0000);
    if diff >= 25 {
        (a, signed_zero(b))
    } else if diff > 0 {
        (a, clear_low_mantissa_bits(b, (diff - 1) as u32))
    } else if diff == 0 {
        (a, b)
    } else if diff > -25 {
        (clear_low_mantissa_bits(a, (-diff - 1) as u32), b)
    } else {
        (signed_zero(a), b)
    }
}

fn add_positive(a: f32, b: f32) -> Result<f32> {
    let (a, b) = pcsx2_mask_operands(a, b);
    let (a, b) = (unpack(a)?, unpack(b)?);
    if a.mant == 0 {
        return pack_positive(b.neg, b.mant, b.exp);
    }
    if b.mant == 0 {
        return pack_positive(a.neg, a.mant, a.exp);
    }
    if (a.exp - b.exp).abs() > 64 {
        return Err("EE float model: operand exponent gap unsupported".into());
    }
    let exp = a.exp.min(b.exp);
    let sa = (a.mant << (a.exp - exp) as u32) as i128;
    let sb = (b.mant << (b.exp - exp) as u32) as i128;
    let sum = if a.neg { -sa } else { sa } + if b.neg { -sb } else { sb };
    pack_positive(sum < 0, sum.unsigned_abs(), exp)
}

impl FloatModel {
    pub fn mul(self, a: f32, b: f32) -> Result<f32> {
        match self {
            Self::HostF32 => Ok(a * b),
            Self::Pcsx2PositiveRounding => mul_positive(a, b),
        }
    }
    pub fn add(self, a: f32, b: f32) -> Result<f32> {
        match self {
            Self::HostF32 => Ok(a + b),
            Self::Pcsx2PositiveRounding => add_positive(a, b),
        }
    }
    /// `div.s` by a power of two is exact in every rounding mode as long as
    /// the result stays normal; other divisors are not modelled.
    pub fn div_pow2(self, a: f32, divisor: f32) -> Result<f32> {
        if !(divisor > 0.0) || divisor.to_bits() & 0x7f_ffff != 0 {
            return Err("EE float model: only power-of-two divisors are modelled".into());
        }
        let quotient = a / divisor;
        if quotient != 0.0 && !quotient.is_normal() {
            return Err("EE float model: quotient leaves the normal range".into());
        }
        Ok(quotient)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: FloatModel = FloatModel::Pcsx2PositiveRounding;

    #[test]
    fn multiplication_rounds_toward_positive_infinity() {
        // 0.6f * 31 = 18.6000007... exact; nearest and chop give 0x4194cccd/cc.
        assert_eq!(M.mul(0.6, 31.0).unwrap().to_bits(), 0x4194_ccce);
        assert_eq!(M.mul(0.6, 255.0).unwrap().to_bits(), 0x4319_0001);
        assert_eq!(M.mul(0.6, 1.0).unwrap().to_bits(), 0.6_f32.to_bits());
        assert_eq!(M.mul(980.0, 0.0).unwrap(), 0.0);
        // Negative results truncate toward zero.
        assert_eq!(M.mul(-0.6, 31.0).unwrap().to_bits(), 0xc194_cccd);
        assert_eq!(
            FloatModel::HostF32.mul(0.6, 31.0).unwrap().to_bits(),
            0x4194_cccd
        );
    }

    #[test]
    fn addition_masks_the_smaller_operand_before_rounding_up() {
        let q = M.div_pow2(0.6, 256.0).unwrap();
        // Exponent gap 7: six low bits of q are cleared, the sum is exact.
        assert_eq!(M.add(0.4, q).unwrap().to_bits(), 0x3ece_0000);
        // Without masking the same sum would round up to 0x3ece0001.
        assert_eq!(
            FloatModel::HostF32.add(0.4, q).unwrap().to_bits(),
            0x3ece_0000
        );
        assert_eq!(
            M.add(0.4, M.div_pow2(M.mul(0.6, 255.0).unwrap(), 256.0).unwrap())
                .unwrap()
                .to_bits(),
            0x3f7f_6668
        );
        assert_eq!(
            M.add(0.4, M.div_pow2(M.mul(0.6, 31.0).unwrap(), 256.0).unwrap())
                .unwrap()
                .to_bits(),
            0x3ef2_0001
        );
        assert_eq!(M.add(0.4, 0.0).unwrap(), 0.4);
        assert_eq!(M.add(0.0, 0.4).unwrap(), 0.4);
        // Signed sums used by the position fields.
        let p = M.div_pow2(M.mul(980.0, 255.0).unwrap(), 256.0).unwrap();
        assert_eq!(M.add(-490.0, p).unwrap(), 486.171875);
        let p = M.div_pow2(M.mul(980.0, 1.0).unwrap(), 256.0).unwrap();
        assert_eq!(M.add(-490.0, p).unwrap(), -486.171875);
        let p = M.div_pow2(M.mul(980.0, 127.0).unwrap(), 256.0).unwrap();
        assert_eq!(M.add(-490.0, p).unwrap(), -3.828125);
        // Operands 25+ exponents apart contribute nothing.
        assert_eq!(M.add(1.0e9, 1.0).unwrap(), 1.0e9);
    }

    #[test]
    fn unsupported_inputs_are_errors_not_guesses() {
        assert!(M.mul(f32::NAN, 1.0).is_err());
        assert!(M.add(f32::INFINITY, 1.0).is_err());
        assert!(M.mul(3.0e38, 10.0).is_err());
        assert!(M.div_pow2(1.0, 3.0).is_err());
        assert!(M.div_pow2(1.0, 0.0).is_err());
        assert!(M.div_pow2(1.0e-38, 256.0).is_err());
        // Denormals read as zero, matching the emulator's DAZ setting.
        assert_eq!(M.add(f32::from_bits(1), 0.0).unwrap(), 0.0);
    }
}
