//! CPU model of the GS 2D paths the duel HUD uses: PRIM 6 SPRITE with FST
//! texel coordinates (nearest or bilinear), triangles/triangle strips with
//! Gouraud colour and optional FST texture, 8-bit/4-bit CLUT textures, TFX
//! modulate, alpha test, (A − B)·C/128 + D blending, DIMX dithering and a
//! PSMCT16S target. Checked draw by draw against software-GS dumps of an
//! original duel frame (private tests in roses-cli `hud_check`). It models
//! only these paths, not the whole GS.

/// A PSMCT16S frame: `r5 | g5 << 5 | b5 << 10 | a1 << 15` per pixel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target16 {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u16>,
}

impl Target16 {
    /// RGBA8 rows of the rectangle at (`x`, `y`), `width`×`height` (clipped to
    /// the target). Each 5-bit channel becomes `v << 3` with the low bits
    /// zero, the expansion the GS itself applies to 16-bit colour (TEXA
    /// texture expansion, and Cd in the blend unit, which the dump confirms
    /// draw by draw). The display path is not captured in the dumps; this
    /// assumes the output circuit expands the same way (as PCSX2 does when it
    /// reads a 16-bit frame out). Bit replication (`v << 3 | v >> 2`) would
    /// reach full white but is not what the GS does elsewhere. Alpha is 255:
    /// the frame's A bit is not displayed.
    pub fn to_rgba8(&self, x: usize, y: usize, width: usize, height: usize) -> Vec<u8> {
        let (x1, y1) = ((x + width).min(self.width), (y + height).min(self.height));
        let mut out = Vec::with_capacity(width * height * 4);
        for py in y.min(y1)..y1 {
            for &p in &self.pixels[py * self.width + x.min(x1)..py * self.width + x1] {
                out.extend([
                    ((p & 31) << 3) as u8,
                    (((p >> 5) & 31) << 3) as u8,
                    (((p >> 10) & 31) << 3) as u8,
                    255,
                ]);
            }
        }
        out
    }
}

/// Texels in GS colour space (8-bit RGB, alpha usually 0..=128).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texture {
    pub width: usize,
    pub height: usize,
    pub texels: Vec<[u8; 4]>,
}

impl Texture {
    /// A CLUT texture whose 16-bit palette entries expand as the GS does for
    /// C_16 CLUTs: 5-bit channels shifted left by 3, alpha from TEXA (TA0 when
    /// the STP bit is clear, TA1 when set; AEM off).
    pub fn from_c16_clut(
        width: usize,
        height: usize,
        indices: &[u8],
        palette: &[u32],
        ta0: u8,
        ta1: u8,
    ) -> Self {
        let expand = |c: u32| {
            [
                ((c & 31) << 3) as u8,
                (((c >> 5) & 31) << 3) as u8,
                (((c >> 10) & 31) << 3) as u8,
                if c & 0x8000 != 0 { ta1 } else { ta0 },
            ]
        };
        Self {
            width,
            height,
            texels: indices
                .iter()
                .map(|&i| expand(palette[usize::from(i)]))
                .collect(),
        }
    }

    /// An indexed texture (P_8, or P_4 with indices 0..=15) through a C_32
    /// CLUT: entries are R, G, B, A as stored (TEXA does not apply).
    pub fn from_c32_clut(width: usize, height: usize, indices: &[u8], palette: &[[u8; 4]]) -> Self {
        Self {
            width,
            height,
            texels: indices.iter().map(|&i| palette[usize::from(i)]).collect(),
        }
    }

    /// A PSMCT16S/PSMCT16 texture (e.g. a render target used as texture):
    /// channels shifted left by 3, alpha from TEXA by the A bit.
    pub fn from_c16(width: usize, height: usize, pixels: &[u16], ta0: u8, ta1: u8) -> Self {
        Self {
            width,
            height,
            texels: pixels
                .iter()
                .map(|&c| {
                    [
                        ((c & 31) << 3) as u8,
                        (((c >> 5) & 31) << 3) as u8,
                        (((c >> 10) & 31) << 3) as u8,
                        if c & 0x8000 != 0 { ta1 } else { ta0 },
                    ]
                })
                .collect(),
        }
    }

    fn at(&self, x: i64, y: i64) -> [u8; 4] {
        // CLAMP wrap mode: the texture edge repeats.
        let x = x.clamp(0, self.width as i64 - 1) as usize;
        let y = y.clamp(0, self.height as i64 - 1) as usize;
        self.texels[y * self.width + x]
    }

    /// Bilinear sample at 16.16 texel coordinates (texel centres at .5),
    /// the software GS way: subtract half a texel, 4-bit weights, and
    /// `a + ((b − a)·f >> 4)` per channel, first along u then along v.
    fn bilinear(&self, u: i64, v: i64) -> [u8; 4] {
        let (u, v) = (u - 0x8000, v - 0x8000);
        let (uf, vf) = (((u & 0xFFFF) >> 12) as i32, ((v & 0xFFFF) >> 12) as i32);
        let (x, y) = (u >> 16, v >> 16);
        let lerp = |a: [u8; 4], b: [u8; 4], f: i32| {
            std::array::from_fn(|i| {
                let (a, b) = (i32::from(a[i]), i32::from(b[i]));
                (a + (((b - a) * f) >> 4)) as u8
            })
        };
        let top = lerp(self.at(x, y), self.at(x + 1, y), uf);
        let bottom = lerp(self.at(x, y + 1), self.at(x + 1, y + 1), uf);
        lerp(top, bottom, vf)
    }
}

/// ALPHA register: A, B, D select 0 = Cs, 1 = Cd, 2 = 0; C selects
/// 0 = As, 1 = Ad, 2 = FIX.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blend {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub fix: u8,
}

impl Blend {
    /// Decode a GS ALPHA register value.
    pub fn from_register(alpha: u64) -> Self {
        Self {
            a: (alpha & 3) as u8,
            b: ((alpha >> 2) & 3) as u8,
            c: ((alpha >> 4) & 3) as u8,
            d: ((alpha >> 6) & 3) as u8,
            fix: (alpha >> 32) as u8,
        }
    }
}

/// One sprite with its GS state.
#[derive(Clone, Copy, Debug)]
pub struct Sprite<'a> {
    /// Window coordinates after XYOFFSET: x0, y0, x1, y1.
    pub xy: [f32; 4],
    /// FST texel coordinates: u0, v0, u1, v1.
    pub uv: [f32; 4],
    /// Vertex colour of the sprite (flat).
    pub rgba: [u8; 4],
    pub texture: Option<&'a Texture>,
    /// TFX modulate (otherwise decal).
    pub modulate: bool,
    /// TCC: take alpha from the texture.
    pub tcc: bool,
    /// MMAG/MMIN linear (bilinear) instead of nearest.
    pub bilinear: bool,
    pub blend: Option<Blend>,
    /// ATE with ATST GREATER and this AREF.
    pub alpha_test_greater: Option<u8>,
    /// DIMX when DTHE is on (y-major nibbles, signed 3-bit values).
    pub dither: Option<u64>,
}

fn dither(dimx: u64, x: usize, y: usize) -> i32 {
    let v = ((dimx >> (((y % 4) * 4 + x % 4) * 4)) & 7) as i32;
    if v & 4 != 0 {
        v - 8
    } else {
        v
    }
}

/// Sprite texel coordinate of column `p` in 16.16 fixed point, stepped the
/// way the software GS rasteriser does it: a float32 gradient
/// dt = Δt·65536/Δx, the start trunc(t0·65536 + dt·(first − x0)) at the
/// first covered pixel, and pixels handled 8 at a time from the 8-aligned
/// column below `first`, lane j of group g adding trunc(dt·(j − skip)) and
/// g·trunc(8·dt). Checked against the dump: the stretched portraits
/// (nearest), the squeezed text lines and the minified render target
/// (bilinear).
fn sprite_u(t0: f32, t1: f32, x0: f32, x1: f32, first: usize, p: usize) -> i64 {
    let dt = ((t1 - t0) * 65536.0) / (x1 - x0);
    let start = (t0 * 65536.0 + dt * (first as f32 - x0)).trunc() as i64;
    let (left, skip) = (first - first % 8, first % 8);
    let (group, lane) = ((p - left) / 8, (p - left) % 8);
    start
        + (dt * (lane as f32 - skip as f32)).trunc() as i64
        + group as i64 * (dt * 8.0).trunc() as i64
}

/// Sprite texel coordinate of row `p` in 16.16: trunc(t0·65536 + dt·(p − y0))
/// with the float32 gradient of the rows.
fn sprite_v(t0: f32, t1: f32, y0: f32, y1: f32, p: usize) -> i64 {
    let dt = ((t1 - t0) * 65536.0) / (y1 - y0);
    (t0 * 65536.0 + dt * (p as f32 - y0)).trunc() as i64
}

/// Per-pixel state shared by sprites and triangles.
struct Pipeline {
    modulate: bool,
    tcc: bool,
    blend: Option<Blend>,
    alpha_test_greater: Option<u8>,
    dither: Option<u64>,
    /// FBA: every written pixel gets the alpha bit.
    fba: bool,
}

impl Pipeline {
    /// Texture function, alpha test, blend, dither and PSMCT16S write of one
    /// pixel with vertex colour `rgba` and (optional) texel.
    fn pixel(
        &self,
        target: &mut Target16,
        px: usize,
        py: usize,
        rgba: [u8; 4],
        tex: Option<[u8; 4]>,
    ) {
        let (mut c, mut a) = ([rgba[0], rgba[1], rgba[2]], rgba[3]);
        if let Some(texel) = tex {
            if self.modulate {
                for i in 0..3 {
                    c[i] = ((u32::from(texel[i]) * u32::from(c[i])) >> 7).min(255) as u8;
                }
                if self.tcc {
                    a = ((u32::from(texel[3]) * u32::from(a)) >> 7).min(255) as u8;
                }
            } else {
                c = [texel[0], texel[1], texel[2]];
                if self.tcc {
                    a = texel[3];
                }
            }
        }
        if self.alpha_test_greater.is_some_and(|aref| a <= aref) {
            return;
        }
        let index = py * target.width + px;
        let old = target.pixels[index];
        let cd = [
            ((old & 31) << 3) as i32,
            (((old >> 5) & 31) << 3) as i32,
            (((old >> 10) & 31) << 3) as i32,
        ];
        let ad: i32 = if old & 0x8000 != 0 { 128 } else { 0 };
        let mut out = [i32::from(c[0]), i32::from(c[1]), i32::from(c[2])];
        if let Some(b) = self.blend {
            let pick = |sel: u8, i: usize| match sel {
                0 => i32::from(c[i]),
                1 => cd[i],
                _ => 0,
            };
            let factor = match b.c {
                0 => i32::from(a),
                1 => ad,
                _ => i32::from(b.fix),
            };
            for (i, o) in out.iter_mut().enumerate() {
                *o = (((pick(b.a, i) - pick(b.b, i)) * factor) >> 7) + pick(b.d, i);
            }
        }
        let d = self.dither.map_or(0, |dimx| dither(dimx, px, py));
        let q = |v: i32| ((v.clamp(0, 255) + d).clamp(0, 255) >> 3) as u16;
        target.pixels[index] =
            q(out[0]) | q(out[1]) << 5 | q(out[2]) << 10 | u16::from(a >= 128 || self.fba) << 15;
    }
}

/// Rasterize one sprite into `target` (pixels whose centres lie in
/// [x0, x1) × [y0, y1), texels floor-sampled at the pixel centre, or
/// bilinearly filtered).
pub fn draw(target: &mut Target16, s: &Sprite) {
    let [x0, y0, x1, y1] = s.xy;
    let [u0, v0, u1, v1] = s.uv;
    let first_x = x0.ceil().max(0.0) as usize;
    let first_y = y0.ceil().max(0.0) as usize;
    let end_x = (x1.ceil().max(0.0) as usize).min(target.width);
    let end_y = (y1.ceil().max(0.0) as usize).min(target.height);
    let pipe = Pipeline {
        modulate: s.modulate,
        tcc: s.tcc,
        blend: s.blend,
        alpha_test_greater: s.alpha_test_greater,
        dither: s.dither,
        fba: false,
    };
    for py in first_y..end_y {
        for px in first_x..end_x {
            let tex = s.texture.map(|t| {
                let u = sprite_u(u0, u1, x0, x1, first_x, px);
                let v = sprite_v(v0, v1, y0, y1, py);
                if s.bilinear {
                    t.bilinear(u, v)
                } else {
                    t.at(u >> 16, v >> 16)
                }
            });
            pipe.pixel(target, px, py, s.rgba, tex);
        }
    }
}

/// One triangle with its GS state (FST texture coordinates, nearest).
#[derive(Clone, Copy, Debug)]
pub struct Triangle<'a> {
    /// Window coordinates after XYOFFSET per vertex.
    pub xy: [[f32; 2]; 3],
    /// FST texel coordinates per vertex.
    pub uv: [[f32; 2]; 3],
    pub rgba: [[u8; 4]; 3],
    /// IIP: Gouraud colour; otherwise the last vertex's colour (flat).
    pub gouraud: bool,
    pub texture: Option<&'a Texture>,
    pub modulate: bool,
    pub tcc: bool,
    pub blend: Option<Blend>,
    pub alpha_test_greater: Option<u8>,
    pub dither: Option<u64>,
}

/// 1/16-unit fixed point of a coordinate (GS vertex precision).
fn fx(v: f32) -> i64 {
    (f64::from(v) * 16.0).round() as i64
}

/// Floor of n / d for d > 0.
fn floor_div(n: i64, d: i64) -> i64 {
    n.div_euclid(d)
}

/// Rasterize one triangle: rows whose centres lie in [ymin, ymax), and in
/// each row the pixels whose centres lie in [x_left, x_right) of the exact
/// edge intersections (the software GS scan rule; it equals the sprite rule
/// for axis-aligned quads). Texel coordinates are the exact plane through
/// the three vertices at the pixel centre, floored; Gouraud colour that
/// varies along y only follows the float32 edge value (see below), other
/// colour planes are exact and floored (not exercised by the HUD).
pub fn draw_triangle(target: &mut Target16, t: &Triangle) {
    let p: [[i64; 2]; 3] = t.xy.map(|[x, y]| [fx(x), fx(y)]);
    let area =
        (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
    if area == 0 {
        return;
    }
    let pipe = Pipeline {
        modulate: t.modulate,
        tcc: t.tcc,
        blend: t.blend,
        alpha_test_greater: t.alpha_test_greater,
        dither: t.dither,
        fba: false,
    };
    let (ymin, ymax) = (
        p.iter().map(|v| v[1]).min().unwrap(),
        p.iter().map(|v| v[1]).max().unwrap(),
    );
    // Rows py with ymin <= 16·py < ymax.
    let first_row = floor_div(ymin + 15, 16).max(0);
    let end_row = floor_div(ymax + 15, 16).min(target.height as i64);
    // Barycentric weight of vertex i at (x, y) (×area, 1/256 units).
    let weight = |i: usize, x: i64, y: i64| {
        let (a, b) = (p[(i + 1) % 3], p[(i + 2) % 3]);
        (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])
    };
    // Colour that varies along y only (the HUD gradients): the software GS
    // edge value c_top + (c_bottom − c_top)·(1/Δy)·(y − y_top) in float32,
    // truncated. It differs from the exact plane where that lands on an
    // integer from below (checked against the dump).
    let y_only = (0..4).all(|c| {
        let d = |i: usize| i64::from(t.rgba[i][c]) - i64::from(t.rgba[0][c]);
        d(1) * (p[2][1] - p[0][1]) - d(2) * (p[1][1] - p[0][1]) == 0
    });
    let top = (0..3).min_by_key(|&i| p[i][1]).unwrap();
    let bottom = (0..3).max_by_key(|&i| p[i][1]).unwrap();
    let edge_colour = |c: usize, py: i64| {
        let (c0, c1) = (f32::from(t.rgba[top][c]), f32::from(t.rgba[bottom][c]));
        let (y0, y1) = (t.xy[top][1], t.xy[bottom][1]);
        let step = (c1 - c0) * (1.0 / (y1 - y0));
        (c0 + step * (py as f32 - y0)).trunc().clamp(0.0, 255.0) as u8
    };
    for py in first_row..end_row {
        let y = py * 16;
        // Exact intersections x = n / d of the row with the edges spanning it.
        let mut xs: Vec<(i64, i64)> = Vec::new();
        for i in 0..3 {
            let (a, b) = (p[i], p[(i + 1) % 3]);
            let (lo, hi) = if a[1] < b[1] { (a, b) } else { (b, a) };
            if lo[1] <= y && y < hi[1] {
                let d = hi[1] - lo[1];
                xs.push((lo[0] * d + (y - lo[1]) * (hi[0] - lo[0]), d));
            }
        }
        if xs.len() < 2 {
            continue;
        }
        xs.sort_by(|a, b| {
            (i128::from(a.0) * i128::from(b.1)).cmp(&(i128::from(b.0) * i128::from(a.1)))
        });
        let (l, r) = (xs[0], xs[xs.len() - 1]);
        // Pixel centres 16·px in [l, r): px = ceil(l/16) .. ceil(r/16).
        let first = floor_div(l.0 + 16 * l.1 - 1, 16 * l.1).max(0);
        let end = floor_div(r.0 + 16 * r.1 - 1, 16 * r.1).min(target.width as i64);
        for px in first..end {
            let x = px * 16;
            let w = [weight(0, x, y), weight(1, x, y), weight(2, x, y)];
            let plane = |a: [i64; 3]| {
                let n: i64 = (0..3).map(|i| w[i] * a[i]).sum();
                if area > 0 {
                    floor_div(n, area)
                } else {
                    floor_div(-n, -area)
                }
            };
            let rgba: [u8; 4] = if t.gouraud && y_only {
                std::array::from_fn(|c| edge_colour(c, py))
            } else if t.gouraud {
                std::array::from_fn(|c| {
                    plane([0, 1, 2].map(|i| i64::from(t.rgba[i][c]))).clamp(0, 255) as u8
                })
            } else {
                t.rgba[2]
            };
            let tex = t.texture.map(|tex| {
                let u = plane([0, 1, 2].map(|i| fx(t.uv[i][0])));
                let v = plane([0, 1, 2].map(|i| fx(t.uv[i][1])));
                tex.at(floor_div(u, 16), floor_div(v, 16))
            });
            pipe.pixel(target, px as usize, py as usize, rgba, tex);
        }
    }
}

/// One perspective triangle of the VU1 model path: window XY, Z24, STQ
/// texture coordinates (S = s·q, T = t·q with s, t normalised to the
/// texture), Gouraud colour, opaque, Z test GREATER with Z write.
#[derive(Clone, Copy, Debug)]
pub struct Triangle3<'a> {
    pub xy: [[f32; 2]; 3],
    pub z: [f64; 3],
    pub stq: [[f64; 3]; 3],
    pub rgba: [[u8; 4]; 3],
    pub texture: Option<&'a Texture>,
    pub bilinear: bool,
    pub tcc: bool,
    /// FBA: written pixels get the alpha bit.
    pub fba: bool,
    pub dither: Option<u64>,
}

/// Rasterize a perspective triangle into `target` with the Z buffer `zbuf`
/// (one Z24 per pixel): the scan rule of [`draw_triangle`], Z, S, T, Q and
/// colour planes through the three vertices at the pixel centre (colour
/// floored), texel = (S/Q, T/Q) × the texture size, nearest or bilinear;
/// MODULATE; ZTST GREATER. The software GS's exact plane rounding is not
/// modelled [H].
pub fn draw_triangle_z(target: &mut Target16, zbuf: &mut [u32], t: &Triangle3) {
    let p: [[i64; 2]; 3] = t.xy.map(|[x, y]| [fx(x), fx(y)]);
    let area =
        (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
    if area == 0 {
        return;
    }
    let pipe = Pipeline {
        modulate: true,
        tcc: t.tcc,
        blend: None,
        alpha_test_greater: None,
        dither: t.dither,
        fba: t.fba,
    };
    let (ymin, ymax) = (
        p.iter().map(|v| v[1]).min().unwrap(),
        p.iter().map(|v| v[1]).max().unwrap(),
    );
    let first_row = floor_div(ymin + 15, 16).max(0);
    let end_row = floor_div(ymax + 15, 16).min(target.height as i64);
    let weight = |i: usize, x: i64, y: i64| {
        let (a, b) = (p[(i + 1) % 3], p[(i + 2) % 3]);
        (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])
    };
    for py in first_row..end_row {
        let y = py * 16;
        let mut xs: Vec<(i64, i64)> = Vec::new();
        for i in 0..3 {
            let (a, b) = (p[i], p[(i + 1) % 3]);
            let (lo, hi) = if a[1] < b[1] { (a, b) } else { (b, a) };
            if lo[1] <= y && y < hi[1] {
                let d = hi[1] - lo[1];
                xs.push((lo[0] * d + (y - lo[1]) * (hi[0] - lo[0]), d));
            }
        }
        if xs.len() < 2 {
            continue;
        }
        xs.sort_by(|a, b| {
            (i128::from(a.0) * i128::from(b.1)).cmp(&(i128::from(b.0) * i128::from(a.1)))
        });
        let (l, r) = (xs[0], xs[xs.len() - 1]);
        let first = floor_div(l.0 + 16 * l.1 - 1, 16 * l.1).max(0);
        let end = floor_div(r.0 + 16 * r.1 - 1, 16 * r.1).min(target.width as i64);
        for px in first..end {
            let x = px * 16;
            let w = [weight(0, x, y), weight(1, x, y), weight(2, x, y)].map(|v| v as f64 / area as f64);
            let lerp = |a: [f64; 3]| w[0] * a[0] + w[1] * a[1] + w[2] * a[2];
            let z = lerp(t.z).clamp(0.0, f64::from(0xFF_FFFFu32)) as u32;
            let index = py as usize * target.width + px as usize;
            if z <= zbuf[index] {
                continue;
            }
            zbuf[index] = z;
            let rgba: [u8; 4] = std::array::from_fn(|c| lerp(t.rgba.map(|v| f64::from(v[c]))).floor().clamp(0.0, 255.0) as u8);
            let tex = t.texture.map(|tex| {
                let q = lerp(t.stq.map(|v| v[2]));
                let s = lerp(t.stq.map(|v| v[0])) / q;
                let tt = lerp(t.stq.map(|v| v[1])) / q;
                let (u, v) = ((s * tex.width as f64 * 65536.0) as i64, (tt * tex.height as f64 * 65536.0) as i64);
                if t.bilinear {
                    tex.bilinear(u, v)
                } else {
                    tex.at(u >> 16, v >> 16)
                }
            });
            pipe.pixel(target, px as usize, py as usize, rgba, tex);
        }
    }
}

/// A triangle strip: triangles (0,1,2), (1,2,3), … sharing one state.
pub fn draw_strip(
    target: &mut Target16,
    strip: &[([f32; 2], [f32; 2], [u8; 4])],
    state: &Triangle,
) {
    for w in strip.windows(3) {
        draw_triangle(
            target,
            &Triangle {
                xy: [w[0].0, w[1].0, w[2].0],
                uv: [w[0].1, w[1].1, w[2].1],
                rgba: [w[0].2, w[1].2, w[2].2],
                ..*state
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_dither_and_quantize_like_the_gs() {
        let tex = Texture {
            width: 2,
            height: 1,
            texels: vec![[240, 16, 0, 128], [0, 0, 0, 0]],
        };
        let mut t = Target16 {
            width: 4,
            height: 1,
            pixels: vec![31 << 10; 4], // blue
        };
        draw(
            &mut t,
            &Sprite {
                xy: [-0.5, -0.5, 3.5, 0.5],
                uv: [0.0, 0.0, 2.0, 1.0],
                rgba: [128, 128, 128, 64],
                texture: Some(&tex),
                modulate: true,
                tcc: true,
                bilinear: false,
                blend: Some(Blend {
                    a: 0,
                    b: 1,
                    c: 0,
                    d: 1,
                    fix: 128,
                }),
                alpha_test_greater: Some(0),
                dither: None,
            },
        );
        // Texels 0,0,1,1 across four pixels: half-blended red over blue, then
        // the transparent texel fails the alpha test and leaves blue.
        let rgb = |p: u16| [(p & 31) << 3, ((p >> 5) & 31) << 3, ((p >> 10) & 31) << 3];
        assert_eq!(rgb(t.pixels[0]), [120, 8, 120]);
        assert_eq!(t.pixels[0] >> 15, 0);
        assert_eq!(rgb(t.pixels[3]), [0, 0, 248]);
        assert_eq!(dither(0x0171_0716_7101_0607, 0, 0), -1);
    }

    #[test]
    fn gouraud_quad_covers_like_a_sprite_and_floors_the_plane() {
        let mut t = Target16 {
            width: 6,
            height: 6,
            pixels: vec![0; 36],
        };
        let state = Triangle {
            xy: [[0.0; 2]; 3],
            uv: [[0.0; 2]; 3],
            rgba: [[0; 4]; 3],
            gouraud: true,
            texture: None,
            modulate: true,
            tcc: true,
            blend: None,
            alpha_test_greater: None,
            dither: None,
        };
        let (top, bottom) = ([8, 0, 0, 128], [48, 0, 0, 128]);
        draw_strip(
            &mut t,
            &[
                ([1.0, 1.0], [0.0; 2], top),
                ([4.5, 1.0], [0.0; 2], top),
                ([1.0, 5.0], [0.0; 2], bottom),
                ([4.5, 5.0], [0.0; 2], bottom),
            ],
            &state,
        );
        let red = |x: usize, y: usize| (t.pixels[y * 6 + x] & 31) << 3;
        // Columns 1..=4, rows 1..=4; red = floor(8 + 10·(y − 1)) >> 3 << 3.
        for y in 0..6 {
            for x in 0..6 {
                let inside = (1..5).contains(&x) && (1..5).contains(&y);
                let want = if inside {
                    ((8 + 10 * (y as u16 - 1)) >> 3) << 3
                } else {
                    0
                };
                assert_eq!(red(x, y), want, "({x}, {y})");
                assert_eq!(t.pixels[y * 6 + x] >> 15, u16::from(inside));
            }
        }
    }

    #[test]
    fn bilinear_uses_four_bit_weights() {
        let tex = Texture::from_c32_clut(2, 1, &[0, 1], &[[0, 0, 0, 0], [255, 255, 255, 128]]);
        // u = 1.0: halfway between the texel centres 0.5 and 1.5, weight 8.
        let c = tex.bilinear(0x1_0000, 0x8000);
        assert_eq!(c, [127, 127, 127, 64]);
        // Exactly on the first texel centre.
        assert_eq!(tex.bilinear(0x8000, 0x8000), [0, 0, 0, 0]);
    }

    #[test]
    fn sprite_columns_step_in_lanes_of_eight() {
        // 60 texels squeezed into 44 pixels from column 141: column 163 lands
        // exactly on texel 30.5 (a per-pixel truncated step stops short).
        assert_eq!(sprite_u(0.5, 60.5, 141.0, 185.0, 141, 163), 0x1E_8000);
        // 1:1 sprites sample texel centres.
        assert_eq!(sprite_u(0.5, 22.5, 10.0, 32.0, 10, 20), 10 * 65536 + 0x8000);
    }
}
