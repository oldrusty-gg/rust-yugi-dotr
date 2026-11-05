//! The executable's view-matrix builder, as PCSX2 runs it.
//!
//! `0x20E160` builds a camera slot's view matrix from the eye and the look
//! direction: `0x20CD50` makes the camera-to-world rows (right = (dz, 0,
//! -dx), up = (-dy dx, dx² + dz², -dy dz), forward = d, each divided by its
//! software `sqrtf` length; rows -right, -up, forward, eye), `0x20B370`
//! multiplies by the roll rotation and `0x20B550` inverts the result by
//! cofactors over the determinant.
//!
//! A camera that looks straight down (direction (0, -1, 0), as the battle
//! camera does at pitch 0x4000) has zero-length right and up vectors. PCSX2
//! answers `x / 0` with ±FLT_MAX (sign of x xor sign of 0), the saturated
//! cofactors cancel, and the view matrix becomes the fixed rows
//! (-1, -1, 1), (0, 0, -1), (1, -1, 0) with a translation that depends on the
//! eye. The original then draws the board, the units and the versus cards
//! through that matrix; this port reproduces it bit for bit (checked against
//! the game's own routine run with PCSX2's float rules,
//! the project notes).
use super::ee_float::{ee, sqrtf, vu};
use super::{Matrix, Qword};
use crate::Result;

/// PCSX2 COP1 `div.s`: a zero (or denormal) divisor gives ±FLT_MAX with the
/// sign of `a` xor the sign of `b`; otherwise the rounded quotient.
fn div(a: f32, b: f32) -> Result<f32> {
    if b.to_bits() & 0x7F80_0000 == 0 {
        let sign = (a.to_bits() ^ b.to_bits()) & 0x8000_0000;
        return Ok(f32::from_bits(sign | 0x7F7F_FFFF));
    }
    ee::div(a, b)
}

/// `0x20CD50(out, eye, dir, roll)` with roll 0: the camera-to-world matrix
/// (before the roll multiply and the inverse).
fn camera_rows(eye: [f32; 3], dir: [f32; 3]) -> Result<Matrix> {
    let [dx, dy, dz] = dir;
    // Right (0x2E9E70), up (0x2E9E80) and forward (0x2E9E90), in this order.
    let mut r = [dz, 0.0, ee::neg(dx)];
    let mut u = [
        ee::mul(ee::neg(dy), dx)?,
        ee::madd(ee::mul(dx, dx)?, dz, dz)?,
        ee::mul(ee::neg(dy), dz)?,
    ];
    let mut f = dir;
    let len = |v: &[f32; 3]| -> Result<f32> {
        let acc = ee::add(ee::mul(v[0], v[0])?, ee::mul(v[1], v[1])?)?;
        sqrtf(ee::madd(acc, v[2], v[2])?)
    };
    let s = len(&r)?;
    for x in &mut r {
        *x = div(*x, s)?;
    }
    let s = len(&u)?;
    for x in &mut u {
        *x = div(*x, s)?;
    }
    let s = len(&f)?;
    for x in &mut f {
        *x = div(*x, s)?;
    }
    Ok([
        [ee::neg(r[0]), ee::neg(r[1]), ee::neg(r[2]), 0.0],
        [ee::neg(u[0]), ee::neg(u[1]), ee::neg(u[2]), 0.0],
        [f[0], f[1], f[2], 0.0],
        [eye[0], eye[1], eye[2], 1.0],
    ])
}

/// `0x20BDA0` for the angles (0, 0, 0): cos 0 = 1 and sin 0 = 0 through the
/// same products as the general rotation.
fn roll_zero() -> Result<Matrix> {
    let (c, s) = (1.0f32, 0.0f32);
    let m00 = ee::mul(c, c)?;
    let sxsy = ee::mul(s, s)?;
    let m10 = ee::msub(ee::mul(c, sxsy)?, c, s)?;
    let cxsy = ee::mul(c, s)?;
    let m20 = ee::madd(ee::mul(c, cxsy)?, s, s)?;
    let m01 = ee::mul(c, s)?;
    let m11 = ee::madd(ee::mul(s, sxsy)?, c, c)?;
    let m21 = ee::msub(ee::mul(s, cxsy)?, s, c)?;
    let m02 = ee::neg(s);
    let m12 = ee::mul(s, c)?;
    let m22 = ee::mul(c, c)?;
    Ok([
        [m00, m01, m02, 0.0],
        [m10, m11, m12, 0.0],
        [m20, m21, m22, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// `0x20B370`: `a × b` column by column (VU0 macro mode).
fn mat_mul(a: &Matrix, b: &Matrix) -> Result<Matrix> {
    let col = |v: &Qword| -> Result<Qword> {
        let mut out = [0.0f32; 4];
        for (lane, o) in out.iter_mut().enumerate() {
            let mut acc = vu::mul(a[0][lane], v[0])?;
            acc = vu::madd(acc, a[1][lane], v[1])?;
            acc = vu::madd(acc, a[2][lane], v[2])?;
            *o = vu::madd(acc, a[3][lane], v[3])?;
        }
        Ok(out)
    };
    Ok([col(&b[0])?, col(&b[1])?, col(&b[2])?, col(&b[3])?])
}

/// `0x20B550`: the inverse by cofactors (EE COP1, accumulator order as in the
/// executable); `None` when the determinant is zero (the routine then leaves
/// the output untouched).
pub fn invert(m: &Matrix) -> Result<Option<Matrix>> {
    use ee::{add, madd, msub, mul, sub};
    let g = |r: usize, c: usize| m[r][c];
    let (m00, m01, m02, m03) = (g(0, 0), g(0, 1), g(0, 2), g(0, 3));
    let (m10, m11, m12, m13) = (g(1, 0), g(1, 1), g(1, 2), g(1, 3));
    let (m20, m21, m22, m23) = (g(2, 0), g(2, 1), g(2, 2), g(2, 3));
    let (m30, m31, m32, m33) = (g(3, 0), g(3, 1), g(3, 2), g(3, 3));

    // Determinant (0x20B58C..0x20B704).
    let p12_23 = mul(m12, m23)?;
    let p10_23 = mul(m10, m23)?;
    let p12_20 = mul(m12, m20)?;
    let p11_22 = mul(m11, m22)?;
    let acc = mul(m33, p11_22)?;
    let f11 = madd(acc, m31, p12_23)?;
    let p13_21 = mul(m13, m21)?;
    let f0 = mul(m30, p11_22)?;
    let p13_22 = mul(m13, m22)?;
    let p10_21 = mul(m10, m21)?;
    let f10 = mul(m32, p13_21)?;
    let p12_21 = mul(m12, m21)?;
    let acc = add(f10, f11)?;
    let f10b = mul(m33, p12_21)?;
    let p11_20 = mul(m11, m20)?;
    let f13 = msub(acc, m31, p13_22)?;
    let acc = sub(f13, f10b)?;
    let f12 = mul(m30, p12_23)?;
    let p10_22 = mul(m10, m22)?;
    let f13 = mul(m33, p10_22)?;
    let p11_23 = mul(m11, m23)?;
    let f10c = mul(m30, p12_21)?;
    let p13_20 = mul(m13, m20)?;
    let f13 = add(f13, f12)?;
    let f12 = mul(m32, p13_20)?;
    let f11 = mul(m30, p13_22)?;
    let f12 = add(f12, f13)?;
    let f12 = sub(f12, f11)?;
    let f11 = mul(m31, p13_20)?;
    let f1 = mul(m33, p12_20)?;
    let f12 = sub(f12, f1)?;
    let f1 = mul(m32, p10_23)?;
    let c10 = sub(f12, f1)?; // sp68
    let c00 = msub(acc, m32, p11_23)?; // f7
    let acc = mul(m00, c00)?;
    let f1 = msub(acc, m01, c10)?;
    let acc = mul(m33, p10_21)?;
    let f2 = madd(acc, m30, p11_23)?;
    let acc = add(f11, f2)?;
    let f2 = mul(m32, p10_21)?;
    let f6 = mul(m31, p12_20)?;
    let f0 = add(f2, f0)?;
    let f0 = add(f6, f0)?;
    let f2 = sub(f0, f10c)?;
    let f0 = mul(m33, p11_20)?;
    let f3 = msub(acc, m30, p13_21)?;
    let acc = sub(f3, f0)?;
    let c20 = msub(acc, m31, p10_23)?; // sp6c
    let f0 = mul(m32, p11_20)?;
    let f0 = sub(f2, f0)?;
    let f9 = mul(m31, p10_22)?;
    let c30 = sub(f0, f9)?; // sp70
    let f0 = mul(m02, c20)?;
    let acc = add(f0, f1)?;
    let det = msub(acc, m03, c30)?;
    if det == 0.0 {
        return Ok(None);
    }
    let q = |num: f32| div(num, det);
    let nq = |num: f32| -> Result<f32> { Ok(ee::neg(div(num, det)?)) };

    let mut out = [[0.0f32; 4]; 4];
    out[0][0] = q(c00)?;
    out[1][0] = nq(c10)?;
    out[2][0] = q(c20)?;
    out[3][0] = nq(c30)?;

    // Column 1 (0x20B7D4..).
    let s90 = mul(m01, m22)?;
    let acc = mul(m33, s90)?;
    let s78 = mul(m02, m23)?;
    let f1 = madd(acc, m31, s78)?;
    let s88 = mul(m03, m21)?;
    let f0 = mul(m32, s88)?;
    let acc = add(f0, f1)?;
    let s7c = mul(m03, m22)?;
    let f1 = msub(acc, m31, s7c)?;
    let s9c = mul(m02, m21)?;
    let f0 = mul(m33, s9c)?;
    let acc = sub(f1, f0)?;
    let s80 = mul(m01, m23)?;
    out[0][1] = nq(msub(acc, m32, s80)?)?;

    let sa4 = mul(m00, m22)?;
    let acc = mul(m33, sa4)?;
    let f1 = madd(acc, m30, s78)?;
    let s84 = mul(m03, m20)?;
    let f0 = mul(m32, s84)?;
    let acc = add(f0, f1)?;
    let f1 = msub(acc, m30, s7c)?;
    let s98 = mul(m02, m20)?;
    let f0 = mul(m33, s98)?;
    let acc = sub(f1, f0)?;
    let s8c = mul(m00, m23)?;
    out[1][1] = q(msub(acc, m32, s8c)?)?;

    let f1 = mul(m31, s84)?;
    let s94 = mul(m00, m21)?;
    let acc = mul(m33, s94)?;
    let f0 = madd(acc, m30, s80)?;
    let acc = add(f1, f0)?;
    let f1 = msub(acc, m30, s88)?;
    let sa0 = mul(m01, m20)?;
    let f0 = mul(m33, sa0)?;
    let acc = sub(f1, f0)?;
    out[2][1] = nq(msub(acc, m31, s8c)?)?;

    let acc = mul(m32, s94)?;
    let f1 = madd(acc, m30, s90)?;
    let f0 = mul(m31, s98)?;
    let acc = add(f0, f1)?;
    let f1 = msub(acc, m30, s9c)?;
    let f0 = mul(m32, sa0)?;
    let acc = sub(f1, f0)?;
    out[3][1] = q(msub(acc, m31, sa4)?)?;

    // Column 2 (0x20B9C0..).
    let sc0 = mul(m01, m12)?;
    let acc = mul(m33, sc0)?;
    let sa8 = mul(m02, m13)?;
    let f1 = madd(acc, m31, sa8)?;
    let sb8 = mul(m03, m11)?;
    let f0 = mul(m32, sb8)?;
    let acc = add(f0, f1)?;
    let sac = mul(m03, m12)?;
    let f2 = msub(acc, m31, sac)?;
    let sc8 = mul(m02, m11)?;
    let f0 = mul(m33, sc8)?;
    let acc = sub(f2, f0)?;
    let sb0 = mul(m01, m13)?;
    out[0][2] = q(msub(acc, m32, sb0)?)?;

    let scc = mul(m00, m12)?;
    let acc = mul(m33, scc)?;
    let sb4 = mul(m03, m10)?;
    let sc4 = mul(m02, m10)?;
    let sbc = mul(m00, m13)?;
    let f1 = madd(acc, m30, sa8)?;
    let f0 = mul(m32, sb4)?;
    let acc = add(f0, f1)?;
    let f1 = msub(acc, m30, sac)?;
    let f0 = mul(m33, sc4)?;
    let acc = sub(f1, f0)?;
    out[1][2] = nq(msub(acc, m32, sbc)?)?;

    let f29 = mul(m00, m11)?;
    let acc = mul(m33, f29)?;
    let f28 = mul(m01, m10)?;
    let f1 = mul(m33, f28)?;
    let f2 = madd(acc, m30, sb0)?;
    let f0 = mul(m31, sb4)?;
    let acc = add(f0, f2)?;
    let f0 = msub(acc, m30, sb8)?;
    let acc = sub(f0, f1)?;
    out[2][2] = q(msub(acc, m31, sbc)?)?;

    let acc = mul(m32, f29)?;
    let f1 = mul(m32, f28)?;
    let f2 = madd(acc, m30, sc0)?;
    let f0 = mul(m31, sc4)?;
    let acc = add(f0, f2)?;
    let f0 = msub(acc, m30, sc8)?;
    let acc = sub(f0, f1)?;
    out[3][2] = nq(msub(acc, m31, scc)?)?;

    // Column 3 (0x20BBC4..).
    let acc = mul(m23, sc0)?;
    let f1 = madd(acc, m21, sa8)?;
    let f0 = mul(m22, sb8)?;
    let acc = add(f0, f1)?;
    let f1 = msub(acc, m21, sac)?;
    let f0 = mul(m23, sc8)?;
    let acc = sub(f1, f0)?;
    out[0][3] = nq(msub(acc, m22, sb0)?)?;

    let acc = mul(m23, scc)?;
    let f1 = madd(acc, m20, sa8)?;
    let f0 = mul(m22, sb4)?;
    let acc = add(f0, f1)?;
    let f1 = msub(acc, m20, sac)?;
    let f0 = mul(m23, sc4)?;
    let acc = sub(f1, f0)?;
    out[1][3] = q(msub(acc, m22, sbc)?)?;

    let acc = mul(m23, f29)?;
    let f1 = madd(acc, m20, sb0)?;
    let f0 = mul(m21, sb4)?;
    let acc = add(f0, f1)?;
    let f1 = msub(acc, m20, sb8)?;
    let f0 = mul(m23, f28)?;
    let acc = sub(f1, f0)?;
    out[2][3] = nq(msub(acc, m21, sbc)?)?;

    let acc = mul(m22, f29)?;
    let f1 = mul(m22, f28)?;
    let f2 = madd(acc, m20, sc0)?;
    let f0 = mul(m21, sc4)?;
    let acc = add(f0, f2)?;
    let f0 = msub(acc, m20, sc8)?;
    let acc = sub(f0, f1)?;
    out[3][3] = q(msub(acc, m21, scc)?)?;
    Ok(Some(out))
}

/// `0x20C540(out, m, v)` on the EE FPU path: lane `i` of `v · m` (row
/// vector times the matrix rows), accumulated as the executable does.
pub fn transform(m: &Matrix, v: &Qword) -> Result<Qword> {
    let mut out = [0.0f32; 4];
    for (lane, o) in out.iter_mut().enumerate() {
        let acc = ee::mul(v[1], m[1][lane])?;
        let t = ee::madd(acc, v[0], m[0][lane])?;
        let acc = ee::add(ee::mul(v[2], m[2][lane])?, t)?;
        *o = ee::madd(acc, v[3], m[3][lane])?;
    }
    Ok(out)
}

/// `0x2055F0(out, sx, sy)`: the world point of view-space (sx, sy, d) (d =
/// the projection distance, camera +0x44) through the inverse view.
pub fn view_to_world(view: &Matrix, sx: f32, sy: f32, distance: f32) -> Result<Option<Qword>> {
    let Some(inverse) = invert(view)? else {
        return Ok(None);
    };
    Ok(Some(transform(&inverse, &[sx, sy, distance, 1.0])?))
}

/// The view matrix (row vectors, EE memory order) `0x20E160` stores for a
/// camera at `eye` looking along `dir` with roll 0. `None` when the
/// camera-to-world matrix is singular (the original keeps the old matrix).
pub fn view_from_direction(eye: [f32; 3], dir: [f32; 3]) -> Result<Option<Matrix>> {
    let rows = camera_rows(eye, dir)?;
    let rolled = mat_mul(&rows, &roll_zero()?)?;
    invert(&rolled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(m: &Matrix) -> [[u32; 4]; 4] {
        m.map(|r| r.map(f32::to_bits))
    }

    #[test]
    fn straight_down_view_matches_the_footage_ram() {
        // battle-pine/010-d39: eye (0, 190, 25), direction (0, -1, 0); the
        // matrix PCSX2 left at 0x2ED760.
        let m = view_from_direction([0.0, 190.0, 25.0], [0.0, -1.0, 0.0])
            .unwrap()
            .unwrap();
        let want: [[f32; 4]; 4] = [
            [-1.0, -1.0, 1.0, -0.0],
            [-0.0, 0.0, -1.0, 0.0],
            [1.0, -1.0, 0.0, -0.0],
            [-1.0, 1.0, 1.0, 1.0],
        ];
        assert_eq!(bits(&m), bits(&want));
    }

    #[test]
    fn versus_card_targets_match_the_footage_ram() {
        // battle-pine/010-d39: c604's targets for view x -128 / +128 at
        // depth 800 through the straight-down view.
        let view = view_from_direction([0.0, 190.0, 25.0], [0.0, -1.0, 0.0])
            .unwrap()
            .unwrap();
        let a = view_to_world(&view, -128.0, 0.0, 800.0).unwrap().unwrap();
        let b = view_to_world(&view, 128.0, 0.0, 800.0).unwrap().unwrap();
        assert_eq!(a, [64.0, -735.0, -63.0, 1.0]);
        assert_eq!(b, [-64.0, -863.0, 65.0, 1.0]);
    }

    #[test]
    fn ordinary_directions_give_the_usual_look_at() {
        let m = view_from_direction([0.0, 190.0, 25.0], [0.0, -0.6, -0.8])
            .unwrap()
            .unwrap();
        // The eye maps to the origin.
        for c in 0..3 {
            let v = 0.0 * m[0][c] + 190.0 * m[1][c] + 25.0 * m[2][c] + m[3][c];
            assert!(v.abs() < 1e-3, "{c}: {v}");
        }
    }

    /// The whole oracle table (private research output under
    /// `ROSES_RESEARCH_DIR`, default `work/research`).
    #[test]
    #[ignore = "Requires the private oracle table duel/claude-units/degenerate-view-table.json (ROSES_RESEARCH_DIR)"]
    fn straight_down_views_match_the_oracle_table() {
        let research = std::env::var_os("ROSES_RESEARCH_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../work/research"));
        let path = research.join("duel/claude-units/degenerate-view-table.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("private oracle table {}: {e}", path.display()));
        let table: serde_json::Value = serde_json::from_str(&text).unwrap();
        let mut bad = 0;
        let cases = table["cases"].as_array().unwrap();
        for case in cases {
            let eye: Vec<f32> = case["eye"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| f32::from_bits(v.as_u64().unwrap() as u32))
                .collect();
            let want: Vec<Vec<u32>> = case["view"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    r.as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_u64().unwrap() as u32)
                        .collect()
                })
                .collect();
            let got = view_from_direction([eye[0], eye[1], eye[2]], [0.0, -1.0, 0.0])
                .unwrap()
                .unwrap();
            let got: Vec<Vec<u32>> = bits(&got).iter().map(|r| r.to_vec()).collect();
            if got != want {
                bad += 1;
                if bad < 5 {
                    eprintln!("eye {eye:?}\n got {got:x?}\n want {want:x?}");
                }
            }
        }
        assert_eq!(bad, 0, "{bad} of {} cases differ", cases.len());
    }
}
