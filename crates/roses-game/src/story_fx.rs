//! The Stonehenge story events' light effect c401 (CMODULE 0x191, launched
//! by 11162 / 12132 on the update after their C2 update [emu trace-seto-cam
//! f773, white trace-brest f811]; the same module as Simon's 10040) run as
//! original code by the effect runner with the story camera mirrored, and
//! its light beam rasterised on the CPU into an additive layer. Bevy-free.
//!
//! Source [F `story/claude-flash/FINDINGS.md`]: c401 draws a full-screen
//! additive flash from task frame 32 (the port draws it natively, rows
//! 144..368, `story_effects`), 32 textured rays from (0, 800, 0) (TEXEVE
//! record 0x1A, additive, from frame 16 for 64 frames) and sets scene light
//! slot 7 (not drawn). Here the beam's primitives are taken from the run;
//! the full-screen sprite is skipped (drawn natively).
use roses_ee::bmodule::{AttackEffect, CameraMirror, EffectTextures, GsPrim, TextureKey};

/// CMODULE program of the flash and beam.
pub const C401: u16 = 0x191;
/// The TEXEVE records c401 loads (0x202530(8, 0x18 / 0x19 / 0x1A)).
pub const TEXEVE_RECORDS: [i32; 3] = [0x18, 0x19, 0x1A];

/// TEXEVE.MRG's record `id` as an RH2 byte string.
pub fn texeve_record(texeve: &[u8], id: usize) -> Option<Vec<u8>> {
    let a = roses_assets::inspect_pal_opening_archive(texeve, roses_assets::OpeningArchiveKind::TexEve).ok()?;
    let r = a.records.get(id)?;
    let s = r.file_offset as usize;
    texeve.get(s..s + r.byte_len as usize).map(<[u8]>::to_vec)
}

/// A running c401.
pub struct LightEffect {
    effect: std::sync::Mutex<AttackEffect>,
    pub updates: u32,
    pub prims: Vec<GsPrim>,
}

impl LightEffect {
    /// Launch (init call only; the first update comes with the next call
    /// to [`Self::update`]).
    pub fn launch(executable: &[u8], cmodule: &[u8], texeve: &[u8], camera: CameraMirror) -> Option<Self> {
        let other = TEXEVE_RECORDS
            .iter()
            .filter_map(|&id| texeve_record(texeve, id as usize).map(|b| (8, id, b)))
            .collect::<Vec<_>>();
        let textures = EffectTextures { texeff: Vec::new(), own: Vec::new(), other };
        let effect = AttackEffect::load_cmodule(executable, cmodule, C401, [0, 0], &textures).ok()??;
        let mut fx = Self { effect: std::sync::Mutex::new(effect), updates: 0, prims: Vec::new() };
        fx.effect.get_mut().ok()?.update(&crate::duel_opening_fx::BoardWorld { camera }).ok()?;
        Some(fx)
    }

    /// One update; false once the module has ended.
    pub fn update(&mut self, camera: CameraMirror) -> bool {
        let Ok(effect) = self.effect.get_mut() else { return false };
        if effect.finished {
            self.prims.clear();
            return false;
        }
        match effect.update(&crate::duel_opening_fx::BoardWorld { camera }) {
            Ok(u) => {
                self.updates += 1;
                self.prims = u.prims;
                true
            }
            Err(e) => {
                eprintln!("c401: the light effect stopped: {}", e.0);
                effect.finished = true;
                self.prims.clear();
                false
            }
        }
    }

    /// The beam's primitives (textured ones; the full-screen flash sprite
    /// is drawn natively).
    pub fn beam(&self) -> impl Iterator<Item = &GsPrim> {
        self.prims.iter().filter(|p| p.textured())
    }
}

/// The texture of a primitive's `TextureKey::Own(i)` = TEXEVE record
/// `TEXEVE_RECORDS[i]`.
pub fn texeve_of(key: Option<TextureKey>) -> Option<i32> {
    match key {
        Some(TextureKey::Own(i)) => TEXEVE_RECORDS.get(usize::from(i)).copied(),
        _ => None,
    }
}

/// TEXEVE record `id` as GS texels (RGBA bytes, alpha 0x80 = 1.0).
#[derive(Clone, Debug)]
pub struct Texture {
    pub width: usize,
    pub height: usize,
    pub texels: Vec<[u8; 4]>,
}

impl Texture {
    pub fn from_texeve(texeve: &[u8], id: usize) -> Option<Self> {
        use roses_assets::{OpeningArchiveKind, OpeningPaletteFormat};
        let t = roses_assets::decode_pal_opening_indexed_texture(texeve, OpeningArchiveKind::TexEve, id).ok()?;
        let texel = |w: u32| -> [u8; 4] {
            match t.palette_format {
                OpeningPaletteFormat::Rgba8888Alpha128 => w.to_le_bytes(),
                // RGBA5551 through TEXA (TA0 0, TA1 0x80, AEM 0).
                OpeningPaletteFormat::Rgb5551 => [
                    ((w & 31) << 3) as u8,
                    (((w >> 5) & 31) << 3) as u8,
                    (((w >> 10) & 31) << 3) as u8,
                    if w & 0x8000 != 0 { 0x80 } else { 0 },
                ],
            }
        };
        Some(Self {
            width: t.width as usize,
            height: t.height as usize,
            texels: t.palette_indices.iter().map(|&i| texel(t.palette_words_raw[usize::from(i)])).collect(),
        })
    }

    fn texel(&self, x: i32, y: i32) -> [i32; 4] {
        let x = x.clamp(0, self.width as i32 - 1) as usize;
        let y = y.clamp(0, self.height as i32 - 1) as usize;
        self.texels[y * self.width + x].map(i32::from)
    }

    /// GS sampling at texel coordinates (u, v), CLAMP; bilinear when
    /// `linear` (texel centres at +0.5).
    fn sample(&self, u: f32, v: f32, linear: bool) -> [i32; 4] {
        if !linear {
            return self.texel(u.floor() as i32, v.floor() as i32);
        }
        let (u, v) = (u - 0.5, v - 0.5);
        let (x, y) = (u.floor(), v.floor());
        let (fx, fy) = (((u - x) * 16.0) as i32, ((v - y) * 16.0) as i32);
        let (x, y) = (x as i32, y as i32);
        let [a, b, c, d] = [self.texel(x, y), self.texel(x + 1, y), self.texel(x, y + 1), self.texel(x + 1, y + 1)];
        std::array::from_fn(|i| {
            let top = a[i] * (16 - fx) + b[i] * fx;
            let bottom = c[i] * (16 - fx) + d[i] * fx;
            (top * (16 - fy) + bottom * fy) >> 8
        })
    }
}

/// Rasterise additive textured primitives (GS ALPHA A = Cs, B = 0, C = As,
/// D = Cd: Cd + Cs·As/128, the beam's table entry 1) into an RGBA8 layer of
/// the 512-wide frame's rows `top..top + rows` (window coordinates, the
/// story scene's raw512 frame). The additions commute, so the layer is added
/// to the frame afterwards (saturating) [H: the 16-bit frame's rounding and
/// dithering are not modelled]. Other blends are skipped.
///
/// `depth` (from [`depth_buffer`]): the scene's nearest 1/w per pixel; a
/// beam pixel draws where its 1/w (Q) is at least the scene's (the GS Z
/// test GEQUAL; the stones hide the beam's foot and the lintel crosses it
/// [emu `modes/older/c401/emu`, PCSX2 event 10040]); no depth write.
pub fn rasterise<'a>(prims: impl Iterator<Item = &'a GsPrim>, texture: &Texture, top: usize, rows: usize, depth: Option<&[f32]>) -> Vec<u8> {
    const WIDTH: usize = 512;
    let mut sum = vec![0u16; WIDTH * rows * 3];
    for p in prims {
        if !p.textured() || p.alpha.map(|a| a & 0xFF) != Some(0x48) || p.kind != 4 && p.kind != 3 {
            continue;
        }
        let Some(tex0) = p.tex0 else { continue };
        let (tw, th) = p.texture_size().map_or((1.0, 1.0), |(w, h)| (w as f32, h as f32));
        let modulate = (tex0 >> 35) & 3 == 0;
        let tcc = (tex0 >> 34) & 1 == 1;
        let linear = p.tex1.unwrap_or(0x60) >> 5 & 1 == 1;
        let fst = p.uv_texels();
        let flat = !p.gouraud();
        let vs = &p.vertices;
        let tris: Vec<[usize; 3]> = if p.kind == 4 { (2..vs.len()).map(|i| [i - 2, i - 1, i]).collect() } else { (0..vs.len() / 3).map(|i| [3 * i, 3 * i + 1, 3 * i + 2]).collect() };
        for t in tris {
            let [a, b, c] = t.map(|i| &vs[i]);
            let area = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
            if area.abs() < 1e-6 {
                continue;
            }
            let x0 = a.x.min(b.x).min(c.x).ceil().max(0.0) as i32;
            let x1 = a.x.max(b.x).max(c.x).ceil().min(WIDTH as f32) as i32;
            let y0 = a.y.min(b.y).min(c.y).ceil().max(top as f32) as i32;
            let y1 = a.y.max(b.y).max(c.y).ceil().min((top + rows) as f32) as i32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let (px, py) = (x as f32, y as f32);
                    let w0 = ((b.x - px) * (c.y - py) - (c.x - px) * (b.y - py)) / area;
                    let w1 = ((c.x - px) * (a.y - py) - (a.x - px) * (c.y - py)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let lerp = |f: &dyn Fn(&roses_ee::bmodule::GsVertex) -> f32| w0 * f(a) + w1 * f(b) + w2 * f(c);
                    let k = (y as usize - top) * WIDTH + x as usize;
                    if depth.is_some_and(|d| lerp(&|v| v.q) < d[k]) {
                        continue;
                    }
                    let (u, v) = if fst {
                        (lerp(&|v| v.uv[0]), lerp(&|v| v.uv[1]))
                    } else {
                        let q = lerp(&|v| v.q);
                        if q == 0.0 {
                            continue;
                        }
                        (lerp(&|v| v.uv[0]) / q * tw, lerp(&|v| v.uv[1]) / q * th)
                    };
                    let colour: [i32; 4] = if flat {
                        c.rgba.map(i32::from)
                    } else {
                        std::array::from_fn(|i| lerp(&|v| f32::from(v.rgba[i])) as i32)
                    };
                    let s = texture.sample(u, v, linear);
                    let rgb: [i32; 3] = std::array::from_fn(|i| if modulate { (s[i] * colour[i] >> 7).min(255) } else { s[i] });
                    let alpha = if tcc { if modulate { (s[3] * colour[3]) >> 7 } else { s[3] } } else { colour[3] };
                    let k = k * 3;
                    for i in 0..3 {
                        let add = (rgb[i] * alpha >> 7).clamp(0, 255) as u16;
                        sum[k + i] = (sum[k + i] + add).min(255);
                    }
                }
            }
        }
    }
    sum.chunks(3).flat_map(|c| [c[0] as u8, c[1] as u8, c[2] as u8, 255]).collect()
}

/// The scene's nearest 1/w per pixel of the 512-wide frame's rows
/// `top..top + rows` (0 where nothing is drawn), from triangles in GS clip
/// coordinates (x / w and y / w the GS window position + 1792, w the view
/// depth: the captured draws' `clip_from_source_gs`). Triangles crossing
/// w ≤ 1 (the near plane) are left out [H: they only reach the frame's
/// bottom corners in these scenes].
pub fn depth_buffer(triangles: impl Iterator<Item = [[f32; 4]; 3]>, top: usize, rows: usize) -> Vec<f32> {
    const WIDTH: usize = 512;
    let mut depth = vec![0f32; WIDTH * rows];
    for t in triangles {
        if t.iter().any(|c| c[3] <= 1.0) {
            continue;
        }
        let [a, b, c] = t.map(|c| (c[0] / c[3] - 1792.0, c[1] / c[3] - 1792.0, 1.0 / c[3]));
        let area = (b.0 - a.0) * (c.1 - a.1) - (c.0 - a.0) * (b.1 - a.1);
        if area.abs() < 1e-6 {
            continue;
        }
        let x0 = a.0.min(b.0).min(c.0).ceil().max(0.0) as i32;
        let x1 = a.0.max(b.0).max(c.0).ceil().min(WIDTH as f32) as i32;
        let y0 = a.1.min(b.1).min(c.1).ceil().max(top as f32) as i32;
        let y1 = a.1.max(b.1).max(c.1).ceil().min((top + rows) as f32) as i32;
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32, y as f32);
                let w0 = ((b.0 - px) * (c.1 - py) - (c.0 - px) * (b.1 - py)) / area;
                let w1 = ((c.0 - px) * (a.1 - py) - (a.0 - px) * (c.1 - py)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let q = w0 * a.2 + w1 * b.2 + w2 * c.2;
                let d = &mut depth[(y as usize - top) * WIDTH + x as usize];
                if q > *d {
                    *d = q;
                }
            }
        }
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::*;

    /// c401 run on its own: what it sends per update [run, exploration].
    #[test]
    #[ignore = "requires ROSES_DUEL_IMPORT, ROSES_CMODULE and ROSES_TEXEVE"]
    fn private_c401_sends_a_beam_and_a_flash() {
        let duel = std::path::PathBuf::from(std::env::var_os("ROSES_DUEL_IMPORT").expect("ROSES_DUEL_IMPORT"));
        let cmodule = std::fs::read(std::env::var_os("ROSES_CMODULE").expect("ROSES_CMODULE")).unwrap();
        let texeve = std::fs::read(std::env::var_os("ROSES_TEXEVE").expect("ROSES_TEXEVE")).unwrap();
        let data = crate::duel_play::load(&duel).unwrap();
        // 11162's camera at level 0x1000 (eye (0, 0, 512), target (0, 20.2 + 768, 0)).
        let cam = crate::duel_opening_fx::board_camera([0.0, 0.0, 512.0], [0.0, 20.2 + 768.0, 0.0]);
        let mut fx = LightEffect::launch(&data.executable, &cmodule, &texeve, cam).expect("c401");
        let texture = Texture::from_texeve(&texeve, 0x1A).expect("TEXEVE 0x1A");
        eprintln!("texture {}x{}", texture.width, texture.height);
        let out = std::env::var_os("ROSES_C401_OUT").map(std::path::PathBuf::from);
        let mut n = 0;
        // The level as trace-seto-cam: 0x1000 to f786, then -256 per update.
        let cam_at = |n: i32| {
            let level = (4096 - 256 * (n - 12).max(0)).max(0) as f64;
            crate::duel_opening_fx::board_camera([0.0, 0.0, 512.0], [0.0, f64::from(20.2_f32) + 768.0 * level / 4096.0, 0.0])
        };
        while fx.update(cam_at(n)) {
            if let Some(out) = out.as_ref().filter(|_| [20, 32, 48, 64, 79].contains(&n)) {
                let started = std::time::Instant::now();
                let layer = rasterise(fx.beam(), &texture, 0, 512, None);
                eprintln!("u{n}: rasterised in {:?}", started.elapsed());
                let mut ppm = b"P6 512 512 255
".to_vec();
                ppm.extend(layer.chunks(4).flat_map(|c| [c[0], c[1], c[2]]));
                std::fs::write(out.join(format!("c401-u{n}.ppm")), ppm).unwrap();
            }
            let beam = fx.beam().count();
            let flat = fx.prims.len() - beam;
            if n == 40 {
                let firsts: Vec<(f32, f32, f32)> = fx.beam().step_by(32).map(|p| (p.vertices[0].x, p.vertices[0].y, 1.0 / p.vertices[0].q)).collect();
                eprintln!("u40 ray bases {firsts:?}");
                let lasts: Vec<(f32, f32)> = fx.beam().skip(31).step_by(32).map(|p| (p.vertices[p.vertices.len() - 1].x, p.vertices[p.vertices.len() - 1].y)).collect();
                eprintln!("u40 ray tips {lasts:?}");
            }
            if n % 8 == 0 || (14..=18).contains(&n) || (30..=34).contains(&n) {
                let p = fx.beam().next();
                eprintln!(
                    "u{n}: {} prims, beam {beam}, untextured {flat}; first beam {:?}",
                    fx.prims.len(),
                    p.map(|p| (p.kind, p.flags, p.vertices.len(), p.texture, p.alpha, p.vertices.first().map(|v| (v.x, v.y, v.rgba, v.uv, v.q))))
                );
            }
            n += 1;
            assert!(n < 400);
        }
        eprintln!("c401 ended after {n} updates");
        assert!(n > 80);
    }
}
