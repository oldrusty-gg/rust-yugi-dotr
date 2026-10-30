//! CPU reference for the GPU cloud pass in `assets/clouds.wgsl`.
//!
//! This mirrors the same model: ordered quads, affine FST UV, bilinear sample
//! truncation, byte-space modulation, GS blend, fixed cloud DIMX and 5-bit
//! quantization after every particle. It is a regression oracle for the native
//! renderer, not independent evidence of original PS2 output.
use crate::clouds::CloudQuad;

pub const WIDTH: usize = 512;
pub const BAND_TOP: usize = 144;
pub const BAND_BOTTOM: usize = 368;
pub const ATLAS_WIDTH: usize = 128;
pub const ATLAS_HEIGHT: usize = 64;
/// Source-backed per-scene cloud DIMX (see research handoff); not rotated.
pub const CLOUD_DIMX: u64 = 0x0231_1023_3120_2310;
/// DIMX in effect for the opaque navy fade drawn before the clouds: the scene
/// chain writes table entry 1 early in each frame and entry 3 only after the fade.
pub const FADE_DIMX: u64 = 0x0171_0716_7101_0607;
/// Fade RGB from its DIRECT packet (alpha 128, so the blend yields Cs exactly).
pub const FADE_RGB: [u8; 3] = [0, 0, 32];

/// The dithered PSMCT16S navy fade covering the band before any cloud quad.
pub fn fade_band() -> Vec<u8> {
    fade_band_alpha(128)
}

/// The fade quad at GS vertex alpha 0..=128 over a black world (the native
/// preview has no history-scene world yet): `(Cs - 0) * As >> 7`, dithered.
pub fn fade_band_alpha(alpha: u8) -> Vec<u8> {
    let alpha = i32::from(alpha.min(128));
    let rows = BAND_BOTTOM - BAND_TOP;
    let mut out = Vec::with_capacity(WIDTH * rows * 3);
    for y in BAND_TOP..BAND_BOTTOM {
        for x in 0..WIDTH {
            let d = dither(FADE_DIMX, x, y);
            out.extend(FADE_RGB.map(|c| {
                let blended = (i32::from(c) * alpha) >> 7;
                ((blended + d).clamp(0, 255) / 8 * 8) as u8
            }));
        }
    }
    out
}

pub fn dither(dimx: u64, x: usize, y: usize) -> i32 {
    let value = ((dimx >> (((y % 4) * 4 + x % 4) * 4)) & 7) as i32;
    if value & 4 != 0 { value - 8 } else { value }
}

fn edge(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}
fn top_left(a: [f32; 2], b: [f32; 2]) -> bool {
    b[1] < a[1] || (b[1] == a[1] && b[0] > a[0])
}
type Vertex = [f32; 4];
fn triangle(p: [f32; 2], a: Vertex, b: Vertex, c: Vertex) -> Option<[f32; 2]> {
    let xy = |v: Vertex| [v[0], v[1]];
    let (mut b, mut c) = (b, c);
    let mut area = edge(xy(a), xy(b), xy(c));
    if area < 0. {
        std::mem::swap(&mut b, &mut c);
        area = -area;
    }
    if area == 0. {
        return None;
    }
    let w = [
        edge(xy(b), xy(c), p),
        edge(xy(c), xy(a), p),
        edge(xy(a), xy(b), p),
    ];
    if w.iter().any(|&v| v < 0.)
        || (w[0] == 0. && !top_left(xy(b), xy(c)))
        || (w[1] == 0. && !top_left(xy(c), xy(a)))
        || (w[2] == 0. && !top_left(xy(a), xy(b)))
    {
        return None;
    }
    let q = w.map(|v| v / area);
    Some(std::array::from_fn(|i| {
        a[2 + i] * q[0] + b[2 + i] * q[1] + c[2 + i] * q[2]
    }))
}

fn texel(atlas: &[u8], x: i32, y: i32) -> [f32; 4] {
    let x = x.rem_euclid(ATLAS_WIDTH as i32) as usize;
    let y = y.rem_euclid(ATLAS_HEIGHT as i32) as usize;
    let at = (y * ATLAS_WIDTH + x) * 4;
    std::array::from_fn(|i| f32::from(atlas[at + i]))
}
fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}
fn sampled_bytes(atlas: &[u8], uv: [f32; 2]) -> [f32; 4] {
    let at = [uv[0] - 0.5, uv[1] - 0.5];
    let (lx, ly) = (at[0].floor() as i32, at[1].floor() as i32);
    let (qx, qy) = (at[0] - at[0].floor(), at[1] - at[1].floor());
    let a = mix(texel(atlas, lx, ly), texel(atlas, lx + 1, ly), qx);
    let b = mix(texel(atlas, lx, ly + 1), texel(atlas, lx + 1, ly + 1), qx);
    mix(a, b, qy).map(|v| (v + 0.05).trunc())
}

/// Raster the visible band (rows 144..368) as RGB bytes, `WIDTH` wide.
/// `atlas` is 128x64 RGBA with raw GS alpha (0..=128).
pub fn raster_band(quads: &[CloudQuad], atlas: &[u8], dimx: u64) -> Vec<u8> {
    raster_band_over(quads, atlas, dimx, &fade_band())
}

/// As [`raster_band`], over a supplied band background (RGB bytes). Research
/// comparisons use this to isolate the cloud pass from the preceding fade.
pub fn raster_band_over(
    quads: &[CloudQuad],
    atlas: &[u8],
    dimx: u64,
    background: &[u8],
) -> Vec<u8> {
    assert_eq!(atlas.len(), ATLAS_WIDTH * ATLAS_HEIGHT * 4);
    let rows = BAND_BOTTOM - BAND_TOP;
    assert_eq!(background.len(), WIDTH * rows * 3);
    let mut dst: Vec<[f32; 3]> = background
        .chunks_exact(3)
        .map(|p| [f32::from(p[0]), f32::from(p[1]), f32::from(p[2])])
        .collect();
    for quad in quads {
        let v: [Vertex; 4] = std::array::from_fn(|i| {
            [
                quad.xy[i][0] as f32,
                quad.xy[i][1] as f32,
                f32::from(quad.uv[i][0]) / 16.,
                f32::from(quad.uv[i][1]) / 16.,
            ]
        });
        let color = quad.rgba.map(f32::from);
        let minx = quad.xy.iter().map(|p| p[0]).min().unwrap().max(0);
        let maxx = quad
            .xy
            .iter()
            .map(|p| p[0])
            .max()
            .unwrap()
            .min(WIDTH as i32 - 1);
        let miny = quad
            .xy
            .iter()
            .map(|p| p[1])
            .min()
            .unwrap()
            .max(BAND_TOP as i32);
        let maxy = quad
            .xy
            .iter()
            .map(|p| p[1])
            .max()
            .unwrap()
            .min(BAND_BOTTOM as i32 - 1);
        for y in miny..=maxy {
            for x in minx..=maxx {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let Some(uv) =
                    triangle(p, v[0], v[1], v[2]).or_else(|| triangle(p, v[2], v[1], v[3]))
                else {
                    continue;
                };
                let tex = sampled_bytes(atlas, uv);
                let modulated: [f32; 4] =
                    std::array::from_fn(|i| (tex[i] * color[i] / 128.).trunc().min(255.));
                if modulated[3] <= 0. {
                    continue;
                }
                let d = dither(dimx, x as usize, y as usize) as f32;
                let pixel = &mut dst[(y as usize - BAND_TOP) * WIDTH + x as usize];
                for c in 0..3 {
                    let blend =
                        ((modulated[c] - pixel[c]) * (modulated[3] / 128.) + pixel[c]).floor();
                    pixel[c] = ((blend + d).clamp(0., 255.) / 8.).floor() * 8.;
                }
            }
        }
    }
    dst.into_iter().flat_map(|p| p.map(|v| v as u8)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(xy: [[i32; 2]; 4], rgba: [u8; 4]) -> CloudQuad {
        CloudQuad {
            group: 0,
            particle: 0,
            age: 3,
            xy,
            uv: [[0, 0], [512, 0], [0, 512], [512, 512]],
            rgba,
        }
    }

    #[test]
    fn fixed_cloud_dimx_is_non_negative_y_major() {
        assert_eq!(
            (0..4).map(|x| dither(CLOUD_DIMX, x, 0)).collect::<Vec<_>>(),
            [0, 1, 3, 2]
        );
        assert_eq!(dither(CLOUD_DIMX, 0, 1), 0);
        assert_eq!(dither(CLOUD_DIMX, 1, 1), 2);
        assert!((0..16).all(|i| (0..=3).contains(&dither(CLOUD_DIMX, i % 4, i / 4))));
    }

    #[test]
    fn navy_fade_uses_dimx_entry_one_and_5_bit_output() {
        // Negative entries (-1/-2) drop 32 to 24; y-major rows from band top.
        let band = fade_band();
        let blue = |x: usize, y: usize| band[((y - BAND_TOP) * WIDTH + x) * 3 + 2];
        let rows: Vec<Vec<u8>> = (0..4)
            .map(|y| (0..4).map(|x| blue(x, BAND_TOP + y)).collect())
            .collect();
        assert_eq!(
            rows,
            [
                [24, 32, 24, 32],
                [32, 32, 32, 24],
                [24, 32, 24, 32],
                [32, 24, 32, 32]
            ]
        );
        assert!(band.chunks(3).all(|p| p[0] == 0 && p[1] == 0));
    }

    #[test]
    fn transparent_texels_do_not_touch_the_background_and_order_matters() {
        let mut atlas = vec![0_u8; ATLAS_WIDTH * ATLAS_HEIGHT * 4];
        for y in 0..32 {
            for x in 0..32 {
                atlas[(y * ATLAS_WIDTH + x) * 4..][..4].copy_from_slice(&[200, 200, 200, 128]);
            }
        }
        let bounds = [[0, 144], [16, 144], [0, 160], [16, 160]];
        let clear = raster_band(&[], &atlas, 0);
        assert_eq!(clear, fade_band());
        let a = quad(bounds, [255, 0, 0, 128]);
        let b = quad(bounds, [0, 255, 0, 64]);
        let ab = raster_band(&[a.clone(), b.clone()], &atlas, 0);
        let ba = raster_band(&[b, a], &atlas, 0);
        assert_ne!(ab[..3], ba[..3]);
        // Opaque clamped red (255 -> 248), then half green, quantized each time.
        assert_eq!(ab[..3], [120, 120, 0]);
        assert_eq!(
            ab[16 * 3..16 * 3 + 3],
            clear[16 * 3..16 * 3 + 3],
            "outside the quad"
        );
    }
}
