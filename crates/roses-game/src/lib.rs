//! Isolated, bounded scene-input contract for a renderer research prototype.
//! No game assets are embedded; this crate is not part of the main workspace.

pub mod camera;
pub mod campaign_map;
pub mod campaign_map_hud;
pub mod capture;
pub mod cloud_raster;
pub mod cloud_source;
pub mod clouds;
pub mod custom_duel_screen;
pub mod map_editor_screen;
pub mod deck_editor_screen;
pub mod duel_arena;
pub mod duel_arena_battle;
pub mod duel_bmodule;
pub mod duel_audio;
pub mod duel_activate;
pub mod duel_c021;
pub mod duel_c022;
pub mod duel_c026;
pub mod duel_c027;
pub mod duel_c034;
pub mod duel_c500;
pub mod duel_c501;
pub mod duel_c502;
pub mod duel_c508;
pub mod duel_c509;
pub mod duel_c531;
pub mod duel_c059;
pub mod duel_c544;
pub mod duel_c557;
pub mod duel_c560;
pub mod duel_c600;
pub mod duel_c602;
pub mod duel_c612;
pub mod duel_c555;
pub mod duel_c505;
pub mod duel_c546;
pub mod duel_c507;
pub mod duel_c533;
pub mod duel_c537;
pub mod duel_fxlib;
pub mod duel_equip;
pub mod duel_c621;
pub mod duel_opening;
pub mod duel_opening_fx;
pub mod story_fx;
pub mod duel_hand;
pub mod duel_hud_picture;
pub mod duel_summon;
pub mod duel_board;
pub mod duel_burn;
pub mod duel_end_screens;
pub mod duel_exodia;
pub mod duel_models;
pub mod duel_monster_fx;
pub mod duel_monster_mods;
pub mod duel_overlay;
#[cfg(feature = "online")]
pub mod duel_online;
#[cfg(not(feature = "online"))]
#[path = "duel_online_off.rs"]
pub mod duel_online;
#[cfg(feature = "online")]
pub mod duel_multi;
#[cfg(not(feature = "online"))]
#[path = "duel_multi_off.rs"]
pub mod duel_multi;
pub mod duel_play;
pub mod duel_return;
pub mod duel_versus;
pub mod duel_victims;
pub mod ee_float;
pub mod game;
pub mod interim_save;
pub mod memory_card_store;
#[cfg(feature = "online")]
pub mod online_decks;
#[cfg(feature = "online")]
pub mod online_menu;
#[cfg(feature = "online")]
pub mod online_screen;
#[cfg(feature = "online")]
pub mod online_title;
pub mod opening;
pub mod pads;
pub mod transition;
pub mod card_detail;
pub mod trade_screen;
pub mod tutorial;
/// Video aid: the paced online seat player (a port addition).
pub mod video_bot;
#[cfg(all(test, feature = "online"))]
#[path = "video_tests.rs"]
mod video_tests;

/// Updates per second of the original's main loop (PAL), the rate of every
/// `FixedUpdate` model step. The frame flip 0x209120 waits for a vsync
/// (0x185DA0) and, with the 512-line frame buffer every mode uses (draw
/// height +0x240 of 0x2E93D0 = 0x200 = the PAL entry's), for a second one
/// when the first returned field 1, so an update spans two fields [F].
/// Measured [emu]: in PCSX2 savestates of the map (C2 0, 652-mup ..),
/// the duel (claude-pine 001..039) and the history recording, the EE cycle
/// counter advances 11,796,480 cycles (= 294.912 MHz / 25) and the vsync
/// counter 0x2E8E98 two fields (5,898,240 cycles each) per main-loop
/// iteration 0x2DA950. Before 7 Oct 2026 the port assumed one update per
/// vsync (50 Hz), twice the original's speed.
pub const UPDATE_HZ: f64 = 25.0;

/// Faithful mode: the original 2003 game only, every port addition hidden
/// (docs/ADDITIONS.md): `--faithful` on the command line or
/// `ROSES_FAITHFUL=1` (the launcher's "Online Play and extras" switch off
/// passes `--faithful`). A build without the `online` feature is always
/// faithful. PC presentation (the window, pads, keyboard mapping) stays.
pub fn faithful() -> bool {
    if !cfg!(feature = "online") {
        return true;
    }
    static FAITHFUL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FAITHFUL.get_or_init(|| {
        std::env::args_os().any(|a| a == "--faithful") || std::env::var("ROSES_FAITHFUL").is_ok_and(|v| v == "1")
    })
}

/// The port additions are offered (the opposite of [`faithful`]).
pub fn extras() -> bool {
    !faithful()
}

/// Folder holding the renderer's own WGSL shaders: `assets` beside the
/// executable in a packaged release, else this source tree's `assets`
/// (development builds and the local play copy). A packaged build
/// (`ROSES_RELEASE` set at build time by `scripts/build-release.ps1`) has no
/// source-tree fallback, so no path of the build machine is compiled in.
pub fn asset_folder() -> String {
    const SOURCE_ASSETS: &str = match option_env!("ROSES_RELEASE") {
        Some(_) => "",
        None => concat!(env!("CARGO_MANIFEST_DIR"), "/assets"),
    };
    let packaged = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("assets")));
    match packaged {
        Some(dir) if SOURCE_ASSETS.is_empty() || dir.join("duel_board.wgsl").is_file() => {
            dir.to_string_lossy().into_owned()
        }
        _ => SOURCE_ASSETS.into(),
    }
}

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf},
};

pub type Result<T> = std::result::Result<T, String>;
pub const PROFILE: &str = "roses-private-captured-rh0-scene-1";
const MAX_SCENE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TEXTURE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_VERTICES: usize = 200_000;

/// `original_saved_render_packet` (an emulator capture: state and EE image
/// hashes are required) or `iso_native_replay` (`roses import-opening-scene`
/// derived the scene from the disc; only the archive hash applies).
pub const PROVENANCE_CAPTURED: &str = "original_saved_render_packet";
pub const PROVENANCE_NATIVE: &str = "iso_native_replay";
#[derive(Clone, Debug, Deserialize)]
pub struct Provenance {
    pub kind: String,
    #[serde(default)]
    pub state_sha256: Option<String>,
    #[serde(default)]
    pub ee_sha256: Option<String>,
    pub model_archive_sha256: String,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AddressMode {
    Repeat,
    Clamp,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Sampler {
    pub u: AddressMode,
    pub v: AddressMode,
    pub magnification: String,
    pub minification_code: u8,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Draw {
    pub object_slot: u32,
    pub draw_index: u32,
    /// Only meaningful for captures (null in disc-derived scenes).
    #[serde(default)]
    pub source_geometry_equals_saved_ee: Option<bool>,
    pub positions_raw: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub vertex_rgba_gs: Vec<[u8; 4]>,
    pub indices: Vec<u32>,
    pub clip_from_source_gs_columns: [[f32; 4]; 4],
    pub fog_vif83: [f32; 4],
    pub texture_path: String,
    pub texture_sha256: String,
    pub sampler: Sampler,
    /// Second-chain RH0 draw with GS alpha blending (only in disc-derived
    /// story worlds; absent = opaque).
    #[serde(default)]
    pub alpha_blend: bool,
}
/// Only the consumed fields are deserialized. Other research evidence remains
/// in the source JSON; this is not an authenticity check of original game code.
#[derive(Clone, Debug, Deserialize)]
pub struct Scene {
    pub schema_version: u32,
    pub profile: String,
    pub scene: String,
    pub provenance: Provenance,
    pub matrix_layout: String,
    pub raw_canvas: [u32; 2],
    pub display_aspect: [u32; 2],
    pub gs_projection_center: [u32; 2],
    pub gs_projection_half_extent: [u32; 2],
    /// Column-major GS projection shared by every captured draw.
    pub gs_projection_columns: [[f32; 4]; 4],
    pub meshes: Vec<Draw>,
    /// Animated models (disc-derived scenes from 7 Oct 2026: the sky,
    /// slot 37): their draws' clip matrix per animation frame. Absent in
    /// older imports and captures, whose sky stays at the imported frame.
    #[serde(default)]
    pub animations: Vec<Animation>,
}
/// One animated model of a world (`roses import-opening-scene`):
/// `clip_from_source_gs_columns` of its draws at frames `first_frame` ..=
/// `frame_count − 1` (`0x1EE6E0`'s forward mode, one frame per update).
#[derive(Clone, Debug, Deserialize)]
pub struct Animation {
    pub object_slot: u32,
    pub frame_count: u32,
    pub first_frame: u32,
    pub clip_from_source_gs_columns_by_frame: Vec<[[f32; 4]; 4]>,
}
impl Animation {
    /// `0x1EE6E0` forward mode with speed 1 and step 1 (byte 0x2DAB86 = 1
    /// after boot): a frame below the last advances by one, the last
    /// restarts at 1 [F, roses_assets::opening_scene::advance_animation_frame].
    pub fn next_frame(&self, frame: u32) -> u32 {
        let last = self.frame_count.saturating_sub(1);
        if frame < last { frame + 1 } else { self.first_frame }
    }
    /// The clip matrix of `frame` (clamped into the table).
    pub fn clip(&self, frame: u32) -> Option<&[[f32; 4]; 4]> {
        let i = frame.saturating_sub(self.first_frame) as usize;
        self.clip_from_source_gs_columns_by_frame
            .get(i.min(self.clip_from_source_gs_columns_by_frame.len().saturating_sub(1)))
    }
}
#[derive(Debug)]
pub struct LoadedScene {
    pub scene: Scene,
    /// One hash-checked PNG per draw. Kept in memory to prevent load/hash races.
    pub textures: Vec<Vec<u8>>,
    pub report: SceneReport,
}
#[derive(Clone, Debug, Serialize)]
pub struct SceneReport {
    pub scene_json_sha256: String,
    pub source_state_sha256: String,
    pub source_ee_sha256: String,
    pub draw_count: usize,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub texture_bytes: usize,
    pub provenance: &'static str,
    pub renderer_parity_validated: bool,
}
fn hash_valid(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn bounded_read(path: &Path, max: u64) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Input must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > max {
        return Err(format!("{} exceeds byte limit", path.display()));
    }
    Ok(bytes)
}
pub fn validate_scene(scene: &Scene) -> Result<()> {
    if scene.schema_version != 1
        || scene.profile != PROFILE
        || !matches!(scene.scene.as_str(), "simon" | "stonehenge-scene-0")
        || scene.matrix_layout != "column_major_columns"
        || scene.raw_canvas != [512, 512]
        || scene.display_aspect != [4, 3]
        || scene.gs_projection_center != [2048, 2048]
        || scene.gs_projection_half_extent != [256, 256]
        || !(scene.provenance.kind == PROVENANCE_CAPTURED
            || scene.provenance.kind == PROVENANCE_NATIVE)
    {
        return Err("Unsupported captured-scene profile, matrix layout or projection".into());
    }
    let captured = scene.provenance.kind == PROVENANCE_CAPTURED;
    if !hash_valid(&scene.provenance.model_archive_sha256) {
        return Err("Invalid source fingerprint metadata".into());
    }
    if captured {
        for hash in [&scene.provenance.state_sha256, &scene.provenance.ee_sha256] {
            if !hash.as_deref().is_some_and(hash_valid) {
                return Err("Invalid source fingerprint metadata".into());
            }
        }
    }
    if scene.meshes.is_empty() || scene.meshes.len() > 128 {
        return Err("Scene requires 1..128 draws".into());
    }
    for a in &scene.animations {
        let frames = a.clip_from_source_gs_columns_by_frame.len() as u64;
        if a.first_frame != 1
            || a.frame_count < 2
            || a.frame_count > 4097
            || frames != u64::from(a.frame_count - 1)
            || !scene.meshes.iter().any(|d| d.object_slot == a.object_slot)
            || a.clip_from_source_gs_columns_by_frame.iter().flatten().flatten().any(|v| !v.is_finite())
        {
            return Err("Invalid scene animation".into());
        }
    }
    let mut keys = BTreeSet::new();
    let mut vertices = 0;
    let mut indices = 0;
    for draw in &scene.meshes {
        if !keys.insert((draw.object_slot, draw.draw_index)) {
            return Err("Duplicate draw identity".into());
        }
        let count = draw.positions_raw.len();
        vertices += count;
        indices += draw.indices.len();
        if count == 0
            || vertices > MAX_VERTICES
            || indices > MAX_VERTICES * 6
            || draw.uv.len() != count
            || draw.vertex_rgba_gs.len() != count
            || draw.indices.is_empty()
            || !draw.indices.len().is_multiple_of(3)
            || draw.indices.iter().any(|&i| i as usize >= count)
        {
            return Err("Invalid vertex attributes, triangle indices or geometry budget".into());
        }
        if (captured && draw.source_geometry_equals_saved_ee != Some(true))
            || !hash_valid(&draw.texture_sha256)
        {
            return Err("Draw lacks the required source comparison or texture fingerprint".into());
        }
        if draw
            .positions_raw
            .iter()
            .flatten()
            .chain(draw.uv.iter().flatten())
            .any(|v| !v.is_finite() || v.abs() > 1e9)
            || draw
                .clip_from_source_gs_columns
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 1e12)
            || draw
                .clip_from_source_gs_columns
                .iter()
                .flatten()
                .all(|v| *v == 0.)
        {
            return Err("Invalid or unsupported geometry/matrix values".into());
        }
        if draw.fog_vif83 != [0., 0., 0., 255.]
            || draw.sampler.magnification != "linear"
            || draw.sampler.minification_code != 1
        {
            return Err(
                "Only observed constant per-object fog and linear sampling are supported".into(),
            );
        }
        validate_relative_path(&draw.texture_path)?;
    }
    Ok(())
}
fn validate_relative_path(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 1024
        || value.contains(':')
        || value.contains('\0')
        || Path::new(value)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Texture path must be a relative child path without traversal".into());
    }
    Ok(())
}
pub fn load_scene(scene_path: &Path, data_root: &Path) -> Result<LoadedScene> {
    let bytes = bounded_read(scene_path, MAX_SCENE_BYTES)?;
    let scene: Scene = serde_json::from_slice(&bytes).map_err(|e| format!("Scene JSON: {e}"))?;
    validate_scene(&scene)?;
    let root = data_root.canonicalize().map_err(|e| e.to_string())?;
    let mut textures = Vec::with_capacity(scene.meshes.len());
    let mut texture_bytes = 0usize;
    let mut decoded_texture_bytes = 0u64;
    for draw in &scene.meshes {
        let path: PathBuf = root
            .join(&draw.texture_path)
            .canonicalize()
            .map_err(|e| format!("{}: {e}", draw.texture_path))?;
        if !path.starts_with(&root) {
            return Err("Texture resolves outside the supplied data root".into());
        }
        let texture = bounded_read(&path, MAX_TEXTURE_BYTES)?;
        texture_bytes += texture.len();
        if texture_bytes > 128 * 1024 * 1024 {
            return Err("Texture byte budget exceeded".into());
        }
        if format!("{:x}", Sha256::digest(&texture)) != draw.texture_sha256.to_ascii_lowercase() {
            return Err(format!(
                "Texture fingerprint mismatch: {}",
                draw.texture_path
            ));
        }
        if texture.get(..8) != Some(b"\x89PNG\r\n\x1a\n") {
            return Err("Texture is not PNG".into());
        }
        // Reject image bombs before Bevy allocates decoded pixels.
        if texture.get(12..16) != Some(b"IHDR") || texture.len() < 24 {
            return Err("Missing PNG IHDR".into());
        }
        let width = u32::from_be_bytes(texture[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(texture[20..24].try_into().unwrap());
        if width == 0 || height == 0 || width > 2048 || height > 2048 {
            return Err("Texture dimensions exceed 2048".into());
        }
        decoded_texture_bytes += u64::from(width) * u64::from(height) * 4;
        if decoded_texture_bytes > 256 * 1024 * 1024 {
            return Err("Decoded texture pixel budget exceeds 256 MiB".into());
        }
        textures.push(texture);
    }
    let report = SceneReport {
        scene_json_sha256: format!("{:x}", Sha256::digest(&bytes)),
        source_state_sha256: scene
            .provenance
            .state_sha256
            .clone()
            .unwrap_or_else(|| "none".into()),
        source_ee_sha256: scene
            .provenance
            .ee_sha256
            .clone()
            .unwrap_or_else(|| "none".into()),
        draw_count: scene.meshes.len(),
        vertex_count: scene.meshes.iter().map(|d| d.positions_raw.len()).sum(),
        triangle_count: scene.meshes.iter().map(|d| d.indices.len() / 3).sum(),
        texture_bytes,
        provenance: if scene.provenance.kind == PROVENANCE_NATIVE {
            "iso_native_replay_matrices_from_the_disc"
        } else {
            "captured_original_matrices_not_native_camera_simulation"
        },
        renderer_parity_validated: false,
    };
    Ok(LoadedScene {
        scene,
        textures,
        report,
    })
}
/// Backend adaptation: original GS homogeneous XY, inverse-W reversed depth.
/// This preserves depth ordering with a near plane at W=1; it does not emulate
/// the original fixed-point depth quantization or clipping microcode.
pub fn backend_clip(columns: [[f32; 4]; 4], position: [f32; 3]) -> [f32; 4] {
    let p = [position[0], position[1], position[2], 1.];
    let c: [f32; 4] =
        std::array::from_fn(|row| (0..4).map(|column| columns[column][row] * p[column]).sum());
    [
        (c[0] - 2048. * c[3]) / 256.,
        -(c[1] - 2048. * c[3]) / 256.,
        1.,
        c[3],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Scene {
        Scene {
            schema_version: 1,
            profile: PROFILE.into(),
            scene: "simon".into(),
            matrix_layout: "column_major_columns".into(),
            raw_canvas: [512, 512],
            display_aspect: [4, 3],
            gs_projection_center: [2048, 2048],
            gs_projection_half_extent: [256, 256],
            gs_projection_columns: [
                [1., 0., 0., 0.],
                [0., 1., 0., 0.],
                [0., 0., 1., 0.],
                [0., 0., 0., 1.],
            ],
            provenance: Provenance {
                kind: "original_saved_render_packet".into(),
                state_sha256: Some("a".repeat(64)),
                ee_sha256: Some("b".repeat(64)),
                model_archive_sha256: "c".repeat(64),
            },
            meshes: vec![Draw {
                object_slot: 0,
                draw_index: 0,
                source_geometry_equals_saved_ee: Some(true),
                positions_raw: vec![[0., 0., 0.]; 3],
                uv: vec![[0., 0.]; 3],
                vertex_rgba_gs: vec![[128; 4]; 3],
                indices: vec![0, 1, 2],
                clip_from_source_gs_columns: [
                    [1., 0., 0., 0.],
                    [0., 1., 0., 0.],
                    [0., 0., 1., 0.],
                    [0., 0., 0., 1.],
                ],
                fog_vif83: [0., 0., 0., 255.],
                texture_path: "synthetic.png".into(),
                texture_sha256: "d".repeat(64),
                sampler: Sampler {
                    u: AddressMode::Repeat,
                    v: AddressMode::Clamp,
                    magnification: "linear".into(),
                    minification_code: 1,
                },
                alpha_blend: false,
            }],
            animations: Vec::new(),
        }
    }
    #[test]
    fn animations_step_forward_and_restart_and_are_validated() {
        let mut s = fixture();
        let id = s.meshes[0].clip_from_source_gs_columns;
        s.animations.push(Animation {
            object_slot: 0,
            frame_count: 4,
            first_frame: 1,
            clip_from_source_gs_columns_by_frame: vec![id; 3],
        });
        assert!(validate_scene(&s).is_ok());
        let a = &s.animations[0];
        assert_eq!([1, 2, 3].map(|f| a.next_frame(f)), [2, 3, 1]);
        assert!(a.clip(3).is_some() && a.clip(9).is_some());
        for edit in [
            |s: &mut Scene| s.animations[0].object_slot = 7,
            |s: &mut Scene| s.animations[0].frame_count = 5,
            |s: &mut Scene| s.animations[0].clip_from_source_gs_columns_by_frame[1][0][0] = f32::NAN,
        ] {
            let mut bad = s.clone();
            edit(&mut bad);
            assert!(validate_scene(&bad).is_err());
        }
    }
    #[test]
    fn rejects_malformed_mesh_indices_nan_fog_and_duplicate_identity() {
        let valid = fixture();
        assert!(validate_scene(&valid).is_ok());
        for edit in [
            |s: &mut Scene| s.meshes[0].indices[2] = 3,
            |s: &mut Scene| s.meshes[0].uv.clear(),
            |s: &mut Scene| s.meshes[0].positions_raw[0][0] = f32::NAN,
            |s: &mut Scene| s.meshes[0].fog_vif83[0] = 0.1275,
            |s: &mut Scene| s.meshes.push(s.meshes[0].clone()),
        ] {
            let mut s = valid.clone();
            edit(&mut s);
            assert!(validate_scene(&s).is_err());
        }
    }
    #[test]
    fn native_provenance_needs_no_capture_hashes() {
        let mut s = fixture();
        s.provenance.kind = PROVENANCE_NATIVE.into();
        s.provenance.state_sha256 = None;
        s.provenance.ee_sha256 = None;
        s.meshes[0].source_geometry_equals_saved_ee = None;
        assert!(validate_scene(&s).is_ok());
        s.provenance.kind = PROVENANCE_CAPTURED.into();
        assert!(validate_scene(&s).is_err());
        s.provenance.kind = "something_else".into();
        assert!(validate_scene(&s).is_err());
    }
    #[test]
    fn bounds_and_traversal_are_rejected() {
        for p in [
            "../secret.png",
            "/absolute.png",
            "C:\\test.png",
            "asset.png:stream",
            "",
        ] {
            assert!(validate_relative_path(p).is_err());
        }
        let mut s = fixture();
        s.meshes[0].indices.pop();
        assert!(validate_scene(&s).is_err());
        s = fixture();
        s.meshes[0].clip_from_source_gs_columns = [[0.; 4]; 4];
        assert!(validate_scene(&s).is_err());
    }
    #[test]
    fn column_major_projection_and_reverse_depth_adapter_are_explicit() {
        let m = [
            [256., 0., 0., 0.],
            [0., 256., 0., 0.],
            [0., 0., 1., 0.],
            [4096., 4096., 12345., 2.],
        ];
        assert_eq!(backend_clip(m, [1., -1., 0.]), [1., 1., 1., 2.]);
    }
}
