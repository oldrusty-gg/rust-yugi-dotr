//! The duel HUD's terrain picture (`claude-screen/hud/TERRAIN-PICTURE.md`):
//! 0x1B4370 renders the board terrain model of the cursor cell's code T
//! (`terrain-{T}`, the board tiles' own meshes and texture) into render
//! target 0x2000 every HUD frame, and the HUD's RT sprite then shows it in
//! the terrain box [F/emu]:
//! - direction d = normalize(0, f.y, |f.xz|) of the main camera's forward f
//!   (yaw removed, pitch kept), eye e = -100 d - (0, 25, 0), look-at
//!   (e, e + d), the board's GS projection centred on 256 of the 512 target;
//! - the model at its object position (0, -35, 0), Y angle + 0x8000, the
//!   angle advancing 0x200 per HUD frame (u32 0x2DE290, start 0 [H]);
//! - vertex colour by the board rule with g = 1.0, MODULATE, bilinear,
//!   CLAMP, Z test GREATER, FBA 1 into a PSMCT16S target cleared to 0, DTHE
//!   with the HUD's DIMX.
//!
//! The software rasteriser follows the GS scan rule, not its exact plane
//! rounding [H]; only forest was checked against a dump (by the helper).
use crate::Result;
use crate::camera::{self, Mat4};
use crate::duel_board::{self, BoardModel, LoadedBoard};
use roses_client::gs::{self, Target16, Texture, Triangle3};
use std::collections::BTreeMap;

pub const SIZE: usize = 512;
/// The angle step per HUD frame [F/emu: three multi-frame dumps].
pub const ANGLE_STEP: u32 = 0x200;

/// The terrain models and their textures, decoded once.
pub struct TerrainPicture {
    models: BTreeMap<u8, BoardModel>,
    textures: BTreeMap<String, Texture>,
    /// 0x2DE290.
    pub angle: u32,
}

/// The preview camera's view for a main-camera forward vector.
pub fn preview_view(forward: [f64; 3]) -> Option<Mat4> {
    let horizontal = (forward[0] * forward[0] + forward[2] * forward[2]).sqrt();
    let len = (forward[1] * forward[1] + horizontal * horizontal).sqrt();
    if len == 0.0 {
        return None;
    }
    let d = [0.0, forward[1] / len, horizontal / len];
    let eye = [-100.0 * d[0], -100.0 * d[1] - 25.0, -100.0 * d[2]];
    camera::look_at(eye, [eye[0] + d[0], eye[1] + d[1], eye[2] + d[2]])
}

impl TerrainPicture {
    pub fn new(board: &LoadedBoard) -> Result<Self> {
        let mut models = BTreeMap::new();
        let mut textures = BTreeMap::new();
        for t in 0..=u8::MAX {
            let Some(model) = board.models.get(&format!("terrain-{t}")) else {
                continue;
            };
            for d in model.draws.iter().filter(|d| d.textured) {
                if textures.contains_key(&d.texture) {
                    continue;
                }
                let bytes = board.textures.get(&d.texture).ok_or_else(|| format!("{}: not imported", d.texture))?;
                let image = roses_client::decode_png(bytes)?;
                // The PNGs hold the expanded CLUT colours; alpha 255 is the
                // GS's 0x80.
                let texels = image
                    .rgba
                    .chunks_exact(4)
                    .map(|p| [p[0], p[1], p[2], ((u32::from(p[3]) * 128 + 127) / 255) as u8])
                    .collect();
                textures.insert(d.texture.clone(), Texture { width: image.width as usize, height: image.height as usize, texels });
            }
            models.insert(t, model.clone());
        }
        if models.is_empty() {
            return Err("no terrain models in the board import".into());
        }
        Ok(Self { models, textures, angle: 0 })
    }

    /// RT 0x2000 for terrain `t` seen along `forward` at the current angle
    /// (C_16S, TA0 0 / TA1 0x80 as the RT sprite reads it).
    pub fn render(&self, t: u8, forward: [f64; 3], dimx: u64) -> Option<Texture> {
        let model = self.models.get(&t)?;
        let view = preview_view(forward)?;
        let world = duel_board::object_world(model, [0.0, -35.0, 0.0], (self.angle as u16).wrapping_add(0x8000));
        let m = camera::mul(&view, &world);
        let mut target = Target16 { width: SIZE, height: SIZE, pixels: vec![0; SIZE * SIZE] };
        let mut zbuf = vec![0u32; SIZE * SIZE];
        let mut draws: Vec<&duel_board::DrawMesh> = model.draws.iter().collect();
        draws.sort_by_key(|d| d.draw_index);
        for d in draws {
            let colour = duel_board::vertex_color(&d.material, 1.0).map(|v| v as u8);
            let texture = d.textured.then(|| self.textures.get(&d.texture)).flatten();
            let projected: Vec<Option<([f32; 2], f64, f64)>> = d
                .positions_raw
                .iter()
                .map(|p| {
                    let v = roses_client::monster_pose::apply(&m, [f64::from(p[0]), f64::from(p[1]), f64::from(p[2]), 1.0]);
                    (v[2] > 0.0).then(|| {
                        let x = 256.0 + 800.0 * v[0] / v[2];
                        let y = 256.0 + 1_074.285_522_460_937_5 * v[1] / v[2];
                        ([x as f32, y as f32], 16_779_264.0 / v[2] - 2048.25, 1.0 / v[2])
                    })
                })
                .collect();
            for tri in d.indices.chunks_exact(3) {
                let Some(vs) = tri.iter().map(|&i| projected.get(i as usize).copied().flatten()).collect::<Option<Vec<_>>>() else {
                    // [H] the VU1 clipper's handling of a vertex behind the eye.
                    continue;
                };
                let uv = |k: usize| d.uv[tri[k] as usize];
                gs::draw_triangle_z(
                    &mut target,
                    &mut zbuf,
                    &Triangle3 {
                        xy: [vs[0].0, vs[1].0, vs[2].0],
                        z: [vs[0].1, vs[1].1, vs[2].1],
                        stq: std::array::from_fn(|k| {
                            let q = vs[k].2;
                            [f64::from(uv(k)[0]) * q, f64::from(uv(k)[1]) * q, q]
                        }),
                        rgba: [colour; 3],
                        texture,
                        bilinear: d.linear,
                        tcc: d.texture_alpha,
                        fba: true,
                        dither: Some(dimx),
                    },
                );
            }
        }
        Some(Texture::from_c16(SIZE, SIZE, &target.pixels, 0, 0x80))
    }

    /// One HUD frame later.
    pub fn advance(&mut self) {
        self.angle = self.angle.wrapping_add(ANGLE_STEP);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_camera_removes_the_yaw_and_keeps_the_pitch() {
        let s = std::f64::consts::FRAC_1_SQRT_2;
        // Side 0 and side 1 board cameras give the same preview.
        let a = preview_view([0.0, -s, -s]).unwrap();
        let b = preview_view([0.0, -s, s]).unwrap();
        assert_eq!(a, b);
        // Eye (0, 45.711, -70.711) at 45 degrees [F].
        let eye = [0.0, 100.0 * s - 25.0, -100.0 * s];
        let origin = roses_client::monster_pose::apply(&a, [eye[0], eye[1], eye[2], 1.0]);
        assert!(origin[..3].iter().all(|v| v.abs() < 1e-9), "{origin:?}");
    }
}

#[cfg(test)]
mod private_tests {
    use super::*;

    /// Writes the forest picture at the round-2 dump's angle (0x1200, pitch
    /// 45 degrees) to `ROSES_PICTURE_OUT` for comparison with the decoded
    /// RT of dump draws 6931/6932 (`claude-screen/hud/terrain-rt-2000.png`).
    #[test]
    #[ignore = "research print; needs ROSES_BOARD_MANIFEST"]
    fn print_forest_picture() {
        let board = duel_board::load_board(std::path::Path::new(&std::env::var("ROSES_BOARD_MANIFEST").unwrap())).unwrap();
        let mut p = TerrainPicture::new(&board).unwrap();
        p.angle = 0x1200;
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let t = p.render(0, [0.0, -s, -s], 0).unwrap();
        let out = std::env::var("ROSES_PICTURE_OUT").unwrap();
        let mut rgba = Vec::new();
        for texel in &t.texels {
            rgba.extend([texel[0], texel[1], texel[2], if texel[3] != 0 { 255 } else { 0 }]);
        }
        // Raw 512x512 RGBA8.
        std::fs::write(out, rgba).unwrap();
    }
}
