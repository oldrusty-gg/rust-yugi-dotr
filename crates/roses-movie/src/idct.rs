//! 8x8 inverse DCT (clause 7.5 / Annex A): separable double-precision
//! evaluation of the defining equation, rounded to nearest and saturated to
//! [-256, 255]. This is the IEEE 1180 reference form, not an approximation.
use std::sync::OnceLock;

/// c[u*8 + x] = C(u)/2 * cos((2x+1) u pi / 16)
fn basis() -> &'static [f64; 64] {
    static B: OnceLock<[f64; 64]> = OnceLock::new();
    B.get_or_init(|| {
        let mut c = [0.0; 64];
        for u in 0..8 {
            let cu = if u == 0 { std::f64::consts::FRAC_1_SQRT_2 } else { 1.0 };
            for x in 0..8 {
                c[u * 8 + x] = cu / 2.0
                    * (((2 * x + 1) as f64) * (u as f64) * std::f64::consts::PI / 16.0).cos();
            }
        }
        c
    })
}

/// In-place IDCT of coefficients in raster order (index v*8+u, v vertical).
pub fn idct(block: &mut [i32; 64]) {
    let c = basis();
    // DC only: every sample is F/8.
    if block[1..].iter().all(|&v| v == 0) {
        let v = (f64::from(block[0]) / 8.0 + 0.5).floor().clamp(-256.0, 255.0) as i32;
        block.fill(v);
        return;
    }
    let mut tmp = [0.0f64; 64];
    for v in 0..8 {
        let row = &block[v * 8..v * 8 + 8];
        if row.iter().all(|&x| x == 0) {
            continue;
        }
        for x in 0..8 {
            let mut s = 0.0;
            for u in 0..8 {
                s += c[u * 8 + x] * f64::from(row[u]);
            }
            tmp[v * 8 + x] = s;
        }
    }
    for y in 0..8 {
        for x in 0..8 {
            let mut s = 0.0;
            for v in 0..8 {
                s += c[v * 8 + y] * tmp[v * 8 + x];
            }
            block[y * 8 + x] = (s + 0.5).floor().clamp(-256.0, 255.0) as i32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Forward DCT for the round-trip check.
    fn fdct(pixels: &[i32; 64]) -> [f64; 64] {
        let c = basis();
        let mut out = [0.0; 64];
        for v in 0..8 {
            for u in 0..8 {
                let mut s = 0.0;
                for y in 0..8 {
                    for x in 0..8 {
                        s += c[u * 8 + x] * c[v * 8 + y] * f64::from(pixels[y * 8 + x]);
                    }
                }
                out[v * 8 + u] = s;
            }
        }
        out
    }

    #[test]
    fn round_trips_and_dc() {
        let mut seed = 12345u32;
        for _ in 0..200 {
            let mut px = [0i32; 64];
            for p in &mut px {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                *p = ((seed >> 16) % 512) as i32 - 256;
            }
            let f = fdct(&px);
            let mut block = [0i32; 64];
            for i in 0..64 {
                block[i] = f[i].round() as i32;
            }
            idct(&mut block);
            for i in 0..64 {
                assert!((block[i] - px[i]).abs() <= 1, "{} vs {}", block[i], px[i]);
            }
        }
        let mut dc = [0i32; 64];
        dc[0] = 1016;
        idct(&mut dc);
        assert!(dc.iter().all(|&v| v == 127));
        let mut dc = [0i32; 64];
        dc[0] = -4000;
        idct(&mut dc);
        assert!(dc.iter().all(|&v| v == -256));
    }
}

/// Verification only (feature `verify-int-idct`): an integer row/column
/// IDCT of the "simple" form (14-bit cosine constants, row shift 11, column
/// shift 20) that common software decoders use. With it this crate's output
/// can be compared bit for bit against such a decoder, which isolates every
/// other stage (parsing, dequantisation, prediction) from IDCT rounding.
/// Not used by the game build.
#[cfg(feature = "verify-int-idct")]
pub fn idct_int(block: &mut [i32; 64]) {
    const W1: i32 = 22725;
    const W2: i32 = 21407;
    const W3: i32 = 19266;
    const W4: i32 = 16383;
    const W5: i32 = 12873;
    const W6: i32 = 8867;
    const W7: i32 = 4520;
    let mut b = [0i16; 64];
    for i in 0..64 {
        b[i] = block[i] as i16;
    }
    for r in 0..8 {
        let row = &mut b[r * 8..r * 8 + 8];
        if row[1..].iter().all(|&v| v == 0) {
            let v = (i32::from(row[0]) << 3) as i16;
            row.fill(v);
            continue;
        }
        let x: [i32; 8] = std::array::from_fn(|i| i32::from(row[i]));
        let mut a0 = W4 * x[0] + (1 << 10);
        let (mut a1, mut a2, mut a3) = (a0, a0, a0);
        a0 += W2 * x[2];
        a1 += W6 * x[2];
        a2 -= W6 * x[2];
        a3 -= W2 * x[2];
        let mut b0 = W1 * x[1] + W3 * x[3];
        let mut b1 = W3 * x[1] - W7 * x[3];
        let mut b2 = W5 * x[1] - W1 * x[3];
        let mut b3 = W7 * x[1] - W5 * x[3];
        a0 += W4 * x[4] + W6 * x[6];
        a1 += -W4 * x[4] - W2 * x[6];
        a2 += -W4 * x[4] + W2 * x[6];
        a3 += W4 * x[4] - W6 * x[6];
        b0 += W5 * x[5] + W7 * x[7];
        b1 += -W1 * x[5] - W5 * x[7];
        b2 += W7 * x[5] + W3 * x[7];
        b3 += W3 * x[5] - W1 * x[7];
        row[0] = ((a0 + b0) >> 11) as i16;
        row[7] = ((a0 - b0) >> 11) as i16;
        row[1] = ((a1 + b1) >> 11) as i16;
        row[6] = ((a1 - b1) >> 11) as i16;
        row[2] = ((a2 + b2) >> 11) as i16;
        row[5] = ((a2 - b2) >> 11) as i16;
        row[3] = ((a3 + b3) >> 11) as i16;
        row[4] = ((a3 - b3) >> 11) as i16;
    }
    for c in 0..8 {
        let x: [i32; 8] = std::array::from_fn(|i| i32::from(b[i * 8 + c]));
        let mut a0 = W4 * (x[0] + ((1 << 19) / W4));
        let (mut a1, mut a2, mut a3) = (a0, a0, a0);
        a0 += W2 * x[2];
        a1 += W6 * x[2];
        a2 -= W6 * x[2];
        a3 -= W2 * x[2];
        let mut b0 = W1 * x[1] + W3 * x[3];
        let mut b1 = W3 * x[1] - W7 * x[3];
        let mut b2 = W5 * x[1] - W1 * x[3];
        let mut b3 = W7 * x[1] - W5 * x[3];
        a0 += W4 * x[4] + W6 * x[6];
        a1 += -W4 * x[4] - W2 * x[6];
        a2 += -W4 * x[4] + W2 * x[6];
        a3 += W4 * x[4] - W6 * x[6];
        b0 += W5 * x[5] + W7 * x[7];
        b1 += -W1 * x[5] - W5 * x[7];
        b2 += W7 * x[5] + W3 * x[7];
        b3 += W3 * x[5] - W1 * x[7];
        let out = [a0 + b0, a1 + b1, a2 + b2, a3 + b3, a3 - b3, a2 - b2, a1 - b1, a0 - b0];
        for (i, v) in out.iter().enumerate() {
            block[i * 8 + c] = v >> 20;
        }
    }
}
