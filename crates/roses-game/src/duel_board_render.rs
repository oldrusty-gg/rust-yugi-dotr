//! `--duel-board <manifest>`: the original duel board (terrain tiles, arena,
//! leader windows) from a private `roses import-duel-board` import, drawn
//! live with the native orbit camera, plus the cell and unit layers (fills,
//! outlines, cursor tint pass, face-down cards, leader emblems) driven by a
//! plain `duel_overlay::OverlayInput`. Scene rules live in the GPU-free
//! `duel_board` and `duel_overlay` modules; this file only drives Bevy.
//!
//! The board renders into a 512x512 Rgba8Unorm canvas so that fixed-function
//! blending adds GS byte values; a present pass shows it at 4:3 or captures it.
//!
//! `--duel-play <import dir>` plays the first campaign duel on this board:
//! a `duel_play::DuelPlay` session runs once per 25 Hz update before the
//! board update, fed by the keyboard (the original auto-repeat of the d-pad,
//! other buttons once per press) or by
//! `--replay-pad`; the board camera follows the active side's cursor and the
//! cell and unit layers are rebuilt from the session every update. The
//! original HUD (`duel_play::DuelHud`, `roses_client::duel_hud`) is computed
//! from the session every update and, when its primitives change, rendered
//! into a 512x512 premultiplied RGBA layer that the present pass composites
//! over the same canvas coordinates.
//!
//! With `--duel-models` (and in the game) leaders and face-up monsters are
//! drawn with their MONSTER.MRG models (`duel_units_render`), idle only.
//!
//! Explicitly not implemented here: 3D unit models without a models import,
//! unit motions other than idle, face-up card images without PICTURE.MRG
//! (`--duel-end`), the held unit's second card image (0x1CEC14), the
//! HUD's terrain tile picture (and the whole HUD outside duel play), arena
//! #3/#4, the backdrop spin, live terrain changes, side-1 camera control and
//! (outside duel play) held d-pad repeat.
use super::{
    CaptureBridge, CaptureState, CaptureWatchdog, PresentationCamera, drive_capture,
    exit_on_escape, fit_view, inspect_render_progress,
};
use crate::cloud_render::CloudMaterial;
use crate::campaign_map_render::MapMaterial;
use crate::duel_model_render::ModelMaterial;
use crate::duel_units_render::{self, UnitModels};
use bevy::{
    app::ScheduleRunnerPlugin,
    asset::RenderAssetUsages,
    camera::{
        RenderTarget,
        visibility::{NoFrustumCulling, RenderLayers},
    },
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::{
        CompressedImageFormats, ImageAddressMode, ImageFilterMode, ImageSampler,
        ImageSamplerDescriptor, ImageType,
    },
    input::{
        ButtonState,
        keyboard::{Key, KeyboardFocusLost, KeyboardInput},
    },
    mesh::{Indices, MeshVertexBufferLayoutRef},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    reflect::TypePath,
    render::{
        Render, RenderApp, RenderSystems,
        render_resource::{
            AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, CompareFunction,
            Extent3d, PrimitiveTopology, RenderPipelineDescriptor, SpecializedMeshPipelineError,
            TextureDimension, TextureFormat, TextureUsages,
        },
    },
    shader::ShaderRef,
    window::{ExitCondition, PrimaryWindow},
    winit::WinitPlugin,
};
use roses_game::camera;
use roses_game::duel_board::{
    self, BoardCamera, BoardLayout, CELLS, DEFAULT_MAP, DEFAULT_WINDOW_KINDS, LoadedBoard,
};
use roses_game::duel_end_screens::{
    BannerKind, DuelEndFiles, EndAssets, banner_primitives, composite_over, darkening_primitive,
    render_banner_layer,
};
use roses_game::duel_overlay::{
    self, CARD_BACK_ORIGIN, CARD_IMAGE, OverlayInput, OverlayState, UnitInput,
};
use roses_game::duel_play::{self, DuelHud, DuelPlay};
use roses_game::tutorial::{TutorialScreen, View as TutorialView};
use roses_core::card_block::CardBlock;
use roses_core::duel::ai::pad;
use roses_core::duel::next_rand31;
use roses_core::duel::script::{CameraOp, LessonEnd};
use roses_core::duel_end::{DuelEnd, DuelEndSetup, EndEvent, EndOutcome, EndTimings, banner};
use roses_core::slot_machine::RewardCardTable;
use std::collections::VecDeque;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    path::PathBuf,
    time::{Duration, Instant},
};

pub const PIPELINE: &str = "roses-duel-board";
const OVERLAY_PIPELINE: &str = "roses-duel-board-overlay";
const PRESENT_PIPELINE: &str = "roses-duel-board-present";
const NOT_IMPLEMENTED: &str = "3D unit models, face-up card images and HUD not implemented";
/// Longest `--ticks` run (headless updates before the first frame): 2,000 s
/// at 25 Hz, room for a whole computer-vs-computer duel (about 8,600–10,900
/// updates for seeds 5, 1 and 2).
const MAX_TICKS: u32 = 100_000;

/// Pipelines the headless capture must see ready.
pub fn is_board_pipeline(label: &str) -> bool {
    matches!(
        label,
        PIPELINE | OVERLAY_PIPELINE | PRESENT_PIPELINE | crate::duel_model_render::PIPELINE
    )
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct BoardMaterial {
    #[uniform(0)]
    ndc_from_source: Mat4,
    #[uniform(1)]
    color_gs: Vec4,
    #[uniform(2)]
    params: Vec4,
    #[texture(3)]
    #[sampler(4)]
    texture: Handle<Image>,
    #[uniform(5)]
    dither_rows: Mat4,
    #[uniform(6)]
    tint: Vec4,
}
impl Material for BoardMaterial {
    fn vertex_shader() -> ShaderRef {
        "duel_board.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "duel_board.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        // The sorted phase keeps the original draw order (cursor cell, cells
        // by terrain code, arena, windows): with ZTST GREATER the first of two
        // equal-depth fragments wins, so order decides shared tile edges.
        // Blending is switched off in `specialize`.
        AlphaMode::Blend
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.label = Some(PIPELINE.into());
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
        ])?];
        // The VU emits every strip triangle of the board models (no culling).
        descriptor.primitive.cull_mode = None;
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                // ABE 0 for every board draw.
                target.blend = None;
            }
        }
        if let Some(depth) = &mut descriptor.depth_stencil {
            // TEST ZTE 1, ZTST GREATER, Z writes on for every board draw.
            depth.depth_compare = Some(CompareFunction::Greater);
            depth.depth_write_enabled = Some(true);
        }
        Ok(())
    }
}

/// Overlay draw kinds (see duel_overlay.wgsl).
const KIND_FILL: u8 = 0;
const KIND_LINE: u8 = 1;
const KIND_QUAD: u8 = 2;
/// Textured quad written as Cs * As (the damage digits' ALPHA (Cs - 0) * As).
const KIND_DARKEN: u8 = 3;
/// Textured quad added as Cs * As + Cd (the return's flash layers).
const KIND_ADD_TEXTURED: u8 = 4;
/// Untextured quad subtracted as Cd - Cs * As (the arena crossfade's white
/// sprite, (0 - Cs) * As + Cd).
const KIND_SUBTRACT: u8 = 5;

/// The duel's music and sound effects (`duel_audio`) and the output mixer.
#[derive(Resource)]
pub struct DuelSound {
    /// Opened once for the whole game (kept across scenes) rather than
    /// with a duel scene.
    pub app_wide: bool,
    audio: roses_game::duel_audio::DuelAudio,
    mixer: Option<std::sync::Arc<std::sync::Mutex<roses_client::sound_mix::Mixer>>>,
    #[cfg(windows)]
    _out: Option<roses_client::sound_out::SoundOut>,
}

/// The video recorder (`record.rs`, a tool) takes the mixer's sound itself:
/// every `DuelSound` then opens a mixer without an output device (silent
/// speakers whatever ROSES_MUTE says).
pub static NO_SOUND_OUTPUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl DuelSound {
    /// Load the duel audio import and open the output.
    pub fn open(manifest: &std::path::Path) -> Result<Self, String> {
        let audio = roses_game::duel_audio::DuelAudio::load(manifest)?;
        if NO_SOUND_OUTPUT.load(std::sync::atomic::Ordering::SeqCst) {
            let mixer = std::sync::Arc::new(std::sync::Mutex::new(roses_client::sound_mix::Mixer::new()));
            return Ok(Self {
                app_wide: false,
                audio,
                mixer: Some(mixer),
                #[cfg(windows)]
                _out: None,
            });
        }
        #[cfg(windows)]
        {
            match roses_client::sound_out::SoundOut::start() {
                Ok(out) => Ok(Self { app_wide: false, audio, mixer: Some(out.mixer()), _out: Some(out) }),
                Err(e) => {
                    eprintln!("Duel audio: no output ({e}); silent");
                    Ok(Self { app_wide: false, audio, mixer: None, _out: None })
                }
            }
        }
        #[cfg(not(windows))]
        {
            Ok(Self { app_wide: false, audio, mixer: None })
        }
    }

    /// A music track decoded outside the duel-audio import.
    pub fn insert_track(&mut self, pcm: std::sync::Arc<roses_client::sound_mix::MusicPcm>) {
        self.audio.insert_track(pcm);
    }

    /// The open output's mixer (`None` when silent, e.g. ROSES_MUTE=1); the
    /// built-in movie player streams its sound track through it.
    pub fn mixer(&self) -> Option<std::sync::Arc<std::sync::Mutex<roses_client::sound_mix::Mixer>>> {
        self.mixer.clone()
    }

    pub fn has_track(&self, track: u32) -> bool {
        self.audio.has_track(track)
    }

    /// One sound request. A track that is not imported (or not in PCMLIST,
    /// e.g. track 1) fades the playing one out [F UI-SOUNDS.md 7.2].
    pub fn cue(&mut self, cue: roses_game::duel_audio::AudioCue) {
        use roses_game::duel_audio::AudioCue;
        match cue {
            AudioCue::Music { track, keep } => match (
                self.audio.track(track).or_else(|| {
                    // An import made before the post-battle variants of
                    // every duel track were imported has only 1008..3008:
                    // play the same variant of track 8 rather than silence
                    // (reported above as not in the import).
                    (track > 1000 && track % 1000 != 8).then(|| self.audio.track(track / 1000 * 1000 + 8)).flatten()
                }),
                &self.mixer,
            ) {
                (Some(pcm), Some(m)) => {
                    if let Ok(mut m) = m.lock() {
                        m.play_music(pcm, keep);
                    }
                }
                (None, Some(m)) => {
                    if let Ok(mut m) = m.lock() {
                        m.fade_out_music(0x100);
                    }
                }
                _ => {}
            },
            AudioCue::FadeOut { speed } => {
                if let Some(Ok(mut m)) = self.mixer.as_ref().map(|m| m.lock()) {
                    m.fade_out_music(speed);
                }
            }
            AudioCue::Se { slot, id, skip } => {
                if let (Some(se), Some(m)) = (self.audio.se(slot, id), &self.mixer) {
                    if let Ok(mut m) = m.lock() {
                        m.play_se(slot, id, &se, skip);
                    }
                }
            }
            AudioCue::StopSe { slot, id } => {
                if let Some(Ok(mut m)) = self.mixer.as_ref().map(|m| m.lock()) {
                    m.stop_se(slot, Some(id));
                }
            }
            AudioCue::Group { slot, card } => {
                if let Some(Ok(mut m)) = self.mixer.as_ref().map(|m| m.lock()) {
                    m.stop_se(slot, None);
                }
                self.audio.load_group(slot, card);
            }
        }
    }
}

/// Each update: the duel's sound requests to the mixer.
fn duel_sound(runtime: Option<ResMut<PlayRuntime>>, sound: Option<ResMut<DuelSound>>, mut reported: Local<bool>) {
    let Some(mut runtime) = runtime else {
        return;
    };
    let cues = std::mem::take(&mut runtime.play.audio);
    let Some(mut sound) = sound else {
        if !cues.is_empty() && !*reported {
            *reported = true;
            eprintln!("Duel audio: unsupported, not played: music and sound effects (the game import has no duel-audio part; import the game again)");
        }
        return;
    };
    for cue in cues {
        sound.cue(cue);
    }
}

/// The battle arena's stage draws on the versus canvas (`duel_arena`).
#[derive(Resource, Default)]
struct ArenaRender {
    loaded: Option<roses_game::duel_arena::LoadedArena>,
    /// Terrain the stage entities were spawned for.
    built: Option<u8>,
    draws: Vec<(Entity, camera::Mat4, Handle<MapMaterial>)>,
    textures: std::collections::HashMap<(String, bool, bool, bool), Handle<Image>>,
    reported: BTreeSet<String>,
}
/// An arena stage draw entity.
#[derive(Component)]
struct ArenaDraw;
/// The board's near-camera unit fade composite (rebuilt every update).
#[derive(Component)]
struct UnitFadeDraw;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(OverlayKey)]
pub struct OverlayMaterial {
    #[uniform(0)]
    ndc_from_source: Mat4,
    #[uniform(1)]
    color_gs: Vec4,
    #[uniform(2)]
    params: Vec4,
    #[uniform(3)]
    dither_rows: Mat4,
    #[texture(4)]
    #[sampler(5)]
    texture: Handle<Image>,
    kind: u8,
    z_write: bool,
    /// Drawn into the cover canvas (alpha accumulates as coverage).
    cover: bool,
}
#[repr(C)]
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct OverlayKey {
    kind: u8,
    z_write: bool,
    cover: bool,
}
impl From<&OverlayMaterial> for OverlayKey {
    fn from(material: &OverlayMaterial) -> Self {
        Self {
            kind: material.kind,
            z_write: material.z_write,
            cover: material.cover,
        }
    }
}
impl Material for OverlayMaterial {
    fn vertex_shader() -> ShaderRef {
        "duel_overlay.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "duel_overlay.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        // Transparent phase: drawn after the board, ordered by entity depth.
        AlphaMode::Blend
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.label = Some(OVERLAY_PIPELINE.into());
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
        ])?];
        descriptor.primitive.cull_mode = None;
        let keep = BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        };
        let color = if key.bind_group_data.kind == KIND_SUBTRACT {
            BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::ReverseSubtract,
            }
        } else if key.bind_group_data.kind == KIND_ADD_TEXTURED {
            BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            }
        } else if key.bind_group_data.kind == KIND_DARKEN {
            BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::Zero,
                operation: BlendOperation::Add,
            }
        } else if key.bind_group_data.kind == KIND_QUAD {
            // ALPHA 0x44: (Cs - Cd) * As + Cd.
            BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            }
        } else {
            // ALPHA 0x48: Cs * As + Cd (the shader applies As).
            BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            }
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                let alpha = if key.bind_group_data.cover {
                    BlendComponent {
                        src_factor: BlendFactor::One,
                        dst_factor: BlendFactor::OneMinusSrcAlpha,
                        operation: BlendOperation::Add,
                    }
                } else {
                    keep
                };
                target.blend = Some(BlendState { color, alpha });
            }
        }
        if let Some(depth) = &mut descriptor.depth_stencil {
            // ZTST GEQUAL; cards and emblems write Z.
            depth.depth_compare = Some(CompareFunction::GreaterEqual);
            depth.depth_write_enabled = Some(key.bind_group_data.z_write);
        }
        Ok(())
    }
}

/// Battle effects with per-vertex colour (`duel_c621`), additive (or
/// subtractive, ALPHA 0x42: Cd - Cs * As), no depth.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(FxKey)]
pub struct FxMaterial {
    #[uniform(0)]
    ndc_from_source: Mat4,
    #[uniform(1)]
    params: Vec4,
    #[uniform(2)]
    dither_rows: Mat4,
    #[texture(3)]
    #[sampler(4)]
    texture: Handle<Image>,
    /// GS state: TFX (0 modulate, 1 decal, 2 / 3 highlight), TCC, the
    /// blend's alpha source (0 As, 1 FIX, 2 unblended: 128) and FIX; (0, 1,
    /// 0, 0) for the hand-ported effects.
    #[uniform(5)]
    gs: Vec4,
    /// The attack effects' GS texture sampling (`gs_tex`): CLAMP WMS / WMT,
    /// TEX1 MMAG / MMIN linear; the region (MINU, MAXU, MINV, MAXV); 2^TW,
    /// 2^TH, LOD K, L; the image's size in GS texels.
    #[uniform(6)]
    wrap: Vec4,
    #[uniform(7)]
    region: Vec4,
    #[uniform(8)]
    lod: Vec4,
    #[uniform(9)]
    size: Vec4,
    /// 0 additive (0x48), 1 subtractive (0x42), 2 Cs * As (0x88), 3 normal
    /// (0x44), 4 (Cs - Cd) As (0x84).
    blend: u8,
    /// ZTE ZTST GEQUAL against the units and stage, ZMSK 1 (the attack
    /// effects: the vertex Z as depth); else drawn over everything.
    z_test: bool,
    /// The GS sampler (texel fetches with CLAMP / TEX1, S T Q per pixel;
    /// the mesh carries UV_1 = (Q, LOD Q)); else the image's own sampler.
    gs_tex: bool,
}
#[repr(C)]
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct FxKey {
    /// 0 additive (0x48), 1 subtractive (0x42), 2 Cs * As (0x88), 3 normal
    /// (0x44), 4 (Cs - Cd) As (0x84).
    blend: u8,
    z_test: bool,
    gs_tex: bool,
}
impl From<&FxMaterial> for FxKey {
    fn from(material: &FxMaterial) -> Self {
        Self { blend: material.blend, z_test: material.z_test, gs_tex: material.gs_tex }
    }
}
impl Material for FxMaterial {
    fn vertex_shader() -> ShaderRef {
        "duel_fx.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "duel_fx.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.gs_tex {
            descriptor.vertex.buffers = vec![layout.0.get_layout(&[
                Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
                Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
                Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
                Mesh::ATTRIBUTE_UV_1.at_shader_location(3),
            ])?];
            descriptor.vertex.shader_defs.push("GS_TEX".into());
            if let Some(fragment) = descriptor.fragment.as_mut() {
                fragment.shader_defs.push("GS_TEX".into());
            }
        } else {
            descriptor.vertex.buffers = vec![layout.0.get_layout(&[
                Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
                Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
                Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
            ])?];
        }
        descriptor.primitive.cull_mode = None;
        let add = match key.bind_group_data.blend {
            // 0x44: (Cs - Cd) As + Cd, the shader writes Cs with As.
            3 => BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            // 0x84: (Cs - Cd) As + 0, negative clamped to 0 (COLCLAMP).
            4 => BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::SrcAlpha,
                operation: BlendOperation::Subtract,
            },
            b => BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: if b == 2 { BlendFactor::Zero } else { BlendFactor::One },
                operation: if b == 1 { BlendOperation::ReverseSubtract } else { BlendOperation::Add },
            },
        };
        let keep = BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState { color: add, alpha: keep });
            }
        }
        if let Some(depth) = &mut descriptor.depth_stencil {
            // GS Z24 = 16779264 / w - 2048.25 and the units' / stage's depth
            // is 1 / w (reverse Z): GEQUAL is GreaterEqual.
            depth.depth_compare = Some(if key.bind_group_data.z_test { CompareFunction::GreaterEqual } else { CompareFunction::Always });
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct PresentMaterial {
    #[texture(0)]
    #[sampler(1)]
    canvas: Handle<Image>,
    #[uniform(2)]
    params: Vec4,
    /// The HUD layer (premultiplied RGBA bytes over the same 512x512 GS
    /// frame), composited before the 5-bit pass; transparent without duel
    /// play.
    #[texture(3)]
    #[sampler(4)]
    hud: Handle<Image>,
    /// The cursor unit's canvas (`duel_units_render`): composited over the
    /// scene where its alpha bit is set, under the HUD; cleared each frame.
    #[texture(5)]
    #[sampler(6)]
    units: Handle<Image>,
    /// The cursor tint (RGB bytes, w = 1 when on): the tint pass's
    /// full-screen sprite also adds it to the cursor unit still in 0x2000
    /// before the alpha-bit composite [H, matching the pink cursor leader of
    /// the first-duel-start screenshot].
    #[uniform(7)]
    unit_tint: Vec4,
    /// The board's DIMX rows (the tinted unit pixels are written again).
    #[uniform(8)]
    dither_rows: Mat4,
    /// The card-versus canvas ([`VersusLayer`]), shown alone when
    /// params.w = 1.
    #[texture(9)]
    #[sampler(10)]
    versus: Handle<Image>,
    /// The previous update's output ([`BlurLayer`]) and the blur alpha
    /// (x, 128 = 1.0; y = 1 for the history pass's raw output).
    #[texture(11)]
    #[sampler(12)]
    history: Handle<Image>,
    #[uniform(13)]
    blur: Vec4,
    /// The effects drawn after the HUD (C600: Z 0xFFFFFF, OT key 0, over
    /// the HUD too [F/emu C600-FUSION-EFFECT.md 0]): additive bytes over a
    /// black canvas, added after the HUD composite.
    #[texture(14)]
    #[sampler(15)]
    top: Handle<Image>,
    /// Cards drawn after the HUD with the normal blend (c601's copy over
    /// the HUD panels [emu 257]): premultiplied, composited before `top`.
    #[texture(16)]
    #[sampler(17)]
    cover: Handle<Image>,
}
impl Material for PresentMaterial {
    fn vertex_shader() -> ShaderRef {
        "duel_board_present.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "duel_board_present.wgsl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.label = Some(PRESENT_PIPELINE.into());
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
        ])?];
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = &mut descriptor.depth_stencil {
            depth.depth_compare = Some(CompareFunction::Always);
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

/// One board draw: its model-to-world matrix, recomposed each camera update.
#[derive(Component)]
struct BoardDraw {
    world: camera::Mat4,
    cell: Option<(u8, u8)>,
    /// The draw's material (vertex colour lanes) and its board object for
    /// the arena objects (0..2) and the windows (5, 6): the Exodia event
    /// dims objects 1, 5 and 6 (0x26E6B0).
    material: [f32; 12],
    object: Option<u8>,
}
/// A world-space overlay draw (fill, card, emblem): geometry is in world units.
#[derive(Component)]
struct OverlayWorld;
/// A pre-projected antialiased line draw, rebuilt when the camera moves.
#[derive(Component)]
struct OverlayLines {
    segments: Vec<[[f64; 3]; 2]>,
    mesh: Handle<Mesh>,
}
/// Unit-layer entities (rebuilt when the unit list changes).
#[derive(Component)]
struct UnitLayer;

/// Render layer of the card-versus canvas.
const VERSUS_LAYER: usize = 3;
/// Render layer of the motion-blur history pass.
const BLUR_LAYER: usize = 4;
/// Effects over the HUD (the top canvas, [`PresentMaterial::top`]).
const TOP_LAYER: usize = 6;
/// Cards over the HUD (the cover canvas, [`PresentMaterial::cover`]).
const COVER_LAYER: usize = 7;
/// The attack effects' draws after a frame copy (the versus camera renders
/// them, the copy camera does not).
const FX_POST_LAYER: usize = 12;

/// The camera rendering the versus layer into the frame-copy image.
#[derive(Component)]
struct FrameCopyCamera;

/// Research captures (`ROSES_CAPTURE_LIVE=N`): the last N updates of
/// `--ticks` run live, one per rendered 25 Hz step with the motion-blur
/// history, so frame-buffer textures have a previous frame; the capture
/// waits until they ran.
#[derive(Resource)]
struct CaptureLive {
    remaining: u32,
}

fn capture_live_updates() -> u32 {
    std::env::var("ROSES_CAPTURE_LIVE").ok().and_then(|v| v.parse().ok()).unwrap_or(0)
}

fn capture_live_running(live: Option<Res<CaptureLive>>) -> bool {
    live.is_some_and(|l| l.remaining > 0)
}

/// After a live capture update: one fewer to go; the capture stays
/// unsettled until the last one has been drawn.
fn capture_live_step(mut live: ResMut<CaptureLive>, capture: Option<ResMut<CaptureState>>) {
    live.remaining = live.remaining.saturating_sub(1);
    if let Some(mut c) = capture {
        c.settled_frames = 0;
    }
}

/// The unit camera's motion blur: two history images written alternately,
/// once per update, by a second present pass; the displayed present reads
/// the one written by the previous update.
#[derive(Resource, Default)]
struct BlurLayer {
    images: Vec<Handle<Image>>,
    /// Index of the image the next update's output goes to.
    write: usize,
    camera: Option<Entity>,
    display: Option<Handle<PresentMaterial>>,
    history: Option<Handle<PresentMaterial>>,
    /// Set by each update; the next rendered frame writes the history.
    pending: bool,
    alpha: u8,
    /// The battle scene's board light (128 = full).
    light: u8,
}

/// The battle scene's card-versus screen (`duel_versus`): its canvas, the
/// present material that shows it, and the face textures made so far.
#[derive(Resource, Default)]
struct VersusLayer {
    canvas: Option<Handle<Image>>,
    present: Option<Handle<PresentMaterial>>,
    faces: std::collections::HashMap<(u16, u8, i32, i32), Option<Handle<Image>>>,
    showing: bool,
    /// TEXEFF record 15 (the damage digits), once made; `Some(None)` when
    /// the import has no TEXEFF.MRG (reported).
    digits: Option<Option<Handle<Image>>>,
    /// TEXEFF record 1 (the burn's flames and sparks).
    flames: Option<Option<Handle<Image>>>,
    /// A 1x1 white texture (the arena crossfade sprite).
    white: Option<Handle<Image>>,
    /// The arena's fade canvas (a fading unit rendered alone): role 0 (and
    /// the board's fading unit), and role 1's own.
    fade: Option<Handle<Image>>,
    fade1: Option<Handle<Image>>,
    /// The frame-copy image (the versus layer up to an effect's frame copy)
    /// and whether this update's effect samples it.
    copy: Option<Handle<Image>>,
    copy_wanted: bool,
    /// The offscreen unit targets of the attack effects (0x1F0FD0 draws a
    /// unit into TBP 0x2000 for afterimages), by role.
    unit_rt: [Option<Handle<Image>>; 2],
    /// Other TEXEFF records of the effects (12: C600's wings).
    sheets: std::collections::HashMap<u8, Option<Handle<Image>>>,
    /// The attack effects' textures by (attacker card, key), with their
    /// sizes (alpha = GS alpha * 255 / 128).
    bfx: std::collections::HashMap<(u16, roses_fx::types::TextureKey), Option<(Handle<Image>, (u32, u32))>>,
    /// Attack-effect draws not reproduced, reported once per kind.
    bfx_reported: std::collections::BTreeSet<String>,
}
/// A versus card quad (rebuilt every update while the screen shows).
#[derive(Component)]
struct VersusDraw;

#[derive(Resource)]
struct BoardState {
    board_camera: BoardCamera,
    pending: Vec<(i8, i8)>,
    /// Eye and target the camera-dependent draws were last built for.
    applied_target: Option<[f64; 7]>,
    /// Duel play owns the window title.
    duel_play: bool,
}

/// The HUD layer image and the present material sampling it. `initial`
/// holds the layer rendered before the app starts (all zero without duel
/// play) until `setup_board` uploads it.
#[derive(Resource)]
struct HudLayer {
    initial: Vec<u8>,
    image: Option<Handle<Image>>,
    present: Option<Handle<PresentMaterial>>,
    /// `DuelHud::version` of the uploaded layer.
    version: u64,
    /// Captures start transparent and upload the layer once after startup
    /// through the live path, so every duel-play capture also checks it.
    pending: Option<(Vec<u8>, u64)>,
}

impl HudLayer {
    /// Replace the layer image's bytes (the asset change re-uploads it)
    /// and touch the present material so its bind group sees the new
    /// texture.
    fn upload(
        &mut self,
        data: &[u8],
        version: u64,
        images: &mut Assets<Image>,
        presents: &mut Assets<PresentMaterial>,
    ) {
        let Some(image) = self.image.clone() else {
            return;
        };
        if let Some(mut image) = images.get_mut(&image) {
            image.data = Some(data.to_vec());
            self.version = version;
        }
        if let Some(present) = self.present.clone() {
            presents.get_mut(&present);
        }
    }
}

/// `--duel-play`: the session driving the board, and the keyboard pad.
#[derive(Resource)]
pub(crate) struct PlayRuntime {
    play: DuelPlay,
    hud: DuelHud,
    /// Held keys and the pad bits each was mapped to when pressed.
    held: BTreeMap<KeyCode, u16>,
    /// Buttons pressed since the last update, so a tap shorter than one
    /// 25 Hz update still counts as held for one update.
    tapped: u16,
    /// Set once `play_keys` has run: its first run drops the key events
    /// buffered before the scene existed (the press that confirmed the
    /// previous scene's choice is still down, not fresh: a carried Start
    /// would quit a practice lesson on its first poll).
    keys_primed: bool,
    /// `ROSES_PAD_TRACE`: the last update's words.
    trace_pads: [u16; 2],
    title: String,
    /// `ROSES_TRACE_DUEL=1`: print every title change on stderr.
    trace: bool,
    /// `ROSES_ONLINE_PADBOT=1` (test aid): an online duel's seat is played
    /// by the pad-level computer player in a real window (it presses the
    /// result screens' face button too), for unattended two-window runs.
    /// (`ROSES_BOT_PACE` / `ROSES_BOT_AI`: the video aid's paced player,
    /// `video_bot`; otherwise the plain pad bot.)
    bot: Option<roses_game::video_bot::VideoBot>,
    /// `ROSES_MULTI_CAPTURE_ROUND` reached: the duel holds still for the
    /// capture (a port addition's test aid).
    capture_frozen: bool,
    /// Updates the duel has rested (nothing presented, the camera and the
    /// HUD still), for that capture.
    rest_updates: u32,
}

impl PlayRuntime {
    /// Video aid (`video_frames`): the paced bot's duel clock (its 25 Hz
    /// updates) and the update of its last big moment.
    pub(crate) fn video_clock(&self) -> Option<(u64, Option<u64>)> {
        let b = self.bot.as_ref()?;
        Some((b.updates, b.last_big.as_ref().map(|x| x.0)))
    }
}

/// `ROSES_ONLINE_PADBOT=1`: the test aid of [`PlayRuntime::bot`].
fn online_test_bot(play: &DuelPlay) -> Option<roses_game::video_bot::VideoBot> {
    (play.online.is_some() && std::env::var("ROSES_ONLINE_PADBOT").is_ok_and(|v| v == "1")).then(|| {
        eprintln!("Online: ROSES_ONLINE_PADBOT=1: the pad-level computer player plays this seat");
        roses_game::video_bot::VideoBot::from_env()
    })
}

/// Video aid: the paced bot also plays in a headless capture of the
/// connected game (`ROSES_BOT_PACE` or `ROSES_BOT_AI` set).
fn video_bot_in_capture() -> bool {
    roses_game::video_bot::BotConfig::wanted()
}

/// What a duel scene needs so the result screens can run when the duel
/// ends: the two persistent card blocks, the shared LCG state, the map index
/// (D37) and the duel-end import's files; `replay` holds scripted pad words
/// (one per update) applied before the keyboard.
pub struct EndInputs {
    pub blocks: [CardBlock; 2],
    pub rng: u64,
    pub map: u8,
    pub duel_end: DuelEndFiles,
    pub replay: VecDeque<u16>,
}

/// The result-state waits the model does not trace (DUEL-END §6): sound
/// idle 0 and camera 0 as in the interpreter runs; the HUD panel slides out
/// by 8 per update from 0 to 0x80 (16 updates) once the sequence hides it.
pub const END_TIMINGS: EndTimings = EndTimings {
    sound_idle_frames: 0,
    hud_slide_frames: 16,
    camera_wait_frames: 0,
};

/// The running result sequence (states 0x54..0x62) over the board.
struct EndActive {
    end: DuelEnd,
    assets: EndAssets,
    blocks: [CardBlock; 2],
    rng: u64,
    replay: VecDeque<u16>,
    human: u8,
    /// D50: the winner (2 = draw), the 1P/2P WIN banners' side.
    winner: u8,
    /// (banner, slide, darken, rank card, icon cel) of the last rendered layer.
    last_key: Option<(u8, i16, u8, Option<u16>, u8)>,
    /// The banner layer rendered for `last_key` (premultiplied RGBA).
    layer: Vec<u8>,
    /// `DuelHud::version` the composite was last built from.
    hud_version: u64,
    outcome: Option<EndOutcome>,
    reported: BTreeSet<String>,
}

/// The duel's end: waits for the outcome, then steps the result sequence
/// every update, drawing the banners over the (hidden) HUD layer.
#[derive(Resource)]
pub struct EndRuntime {
    inputs: Option<EndInputs>,
    active: Option<EndActive>,
}

impl EndRuntime {
    pub fn new(inputs: EndInputs) -> Self {
        Self {
            inputs: Some(inputs),
            active: None,
        }
    }
}

/// A practice lesson's presentation over the HUD: the tutorial layer
/// renderer, what it was last composited with, the script's camera request
/// in force and the notes already printed.
#[derive(Resource)]
pub struct TutorialRuntime {
    screen: TutorialScreen,
    /// `DuelHud::version` the uploaded composite was built from.
    hud_version: Option<u64>,
    reported: BTreeSet<String>,
    /// The message page the camera trace last reported.
    traced_page: Option<u16>,
}

/// The lesson's end once the script reported it (next lesson, or the
/// series' end back to the story).
pub fn tutorial_end(world: &World) -> Option<LessonEnd> {
    world
        .get_resource::<PlayRuntime>()
        .and_then(|r| r.play.tutorial())
        .and_then(|t| t.ended)
}

/// The result sequence's outcome once it has finished.
pub fn end_outcome(world: &World) -> Option<EndOutcome> {
    world
        .get_resource::<EndRuntime>()
        .and_then(|r| r.active.as_ref())
        .and_then(|a| a.outcome.clone())
}

/// Take the card blocks, the LCG state and the leftover scripted presses out
/// of the duel scene (when leaving it).
pub fn take_end_state(world: &mut World) -> Option<([CardBlock; 2], u64, VecDeque<u16>)> {
    let runtime = world.remove_resource::<EndRuntime>()?;
    match (runtime.active, runtime.inputs) {
        (Some(a), _) => Some((a.blocks, a.rng, a.replay)),
        (None, Some(i)) => Some((i.blocks, i.rng, i.replay)),
        (None, None) => None,
    }
}

#[derive(Resource)]
struct BoardInput {
    board: LoadedBoard,
    layout: BoardLayout,
    brightness: f32,
    gs16: bool,
    dither_board: Mat4,
    dither_overlay: Mat4,
}

/// Cell/unit layer state. A game loop may replace `input` every update.
#[derive(Resource)]
pub struct OverlayRuntime {
    pub input: OverlayInput,
    state: OverlayState,
    enabled: bool,
    fills: Vec<(Entity, Handle<OverlayMaterial>)>,
    outlines: Vec<(Entity, Handle<OverlayMaterial>)>,
    built_units: Option<Vec<UnitInput>>,
    card_back: Option<Handle<Image>>,
    emblems: Option<Handle<Image>>,
    blank: Option<Handle<Image>>,
    reported: BTreeSet<String>,
    /// Report each kind of unsupported unit part once (duel play changes the
    /// unit list every few updates).
    report_kinds: bool,
    /// Unit models are drawn (`duel_units_render`): the overlay's model
    /// report is not logged.
    models_drawn: bool,
    /// Face-up card images by (card, values), composed by the HUD's
    /// `board_face` (duel play with PICTURE.MRG).
    face_images: BTreeMap<(u16, duel_overlay::BoardFace), Handle<Image>>,
    /// The duel's cursor tint pass (0x1AEF30 with a cursor colour); the map
    /// editor's board has none [F 0x1AF4D8].
    cursor_tint: bool,
}

impl OverlayRuntime {
    fn tint(&self, cursor: (u8, u8)) -> Vec4 {
        if !self.enabled || !self.cursor_tint {
            return Vec4::ZERO;
        }
        let t = self.state.cursor_tint(cursor, self.input.brightness);
        Vec4::new(f32::from(t[0]), f32::from(t[1]), f32::from(t[2]), 1.0)
    }
}

struct Options {
    manifest: PathBuf,
    cursor: (u8, u8),
    map: usize,
    terrain: Option<[u8; CELLS]>,
    window_kinds: [u8; 2],
    backdrop_angle: u16,
    brightness: u8,
    gs16: bool,
    capture: Option<PathBuf>,
    validate_only: bool,
    /// Cursor steps applied one per update from the start state.
    replay: Vec<(i8, i8)>,
    /// Updates to run before showing or capturing (default: replay + 8).
    ticks: Option<u32>,
    units: Option<PathBuf>,
    overlays: bool,
    /// `--duel-play`: a private `roses import-duel` folder.
    play: Option<PathBuf>,
    /// `--opponent N [--node M]`: a campaign duel against opponent N built
    /// as the map request does (`game::duel_for_request`), the player's
    /// deck from the duel import.
    opponent: Option<(u8, u8)>,
    seed: u64,
    computer_player: bool,
    /// Pad presses applied one per update from the start (duel play).
    replay_pad: Vec<u16>,
    /// `--duel-end <dir>`: run the result screens when the duel ends.
    duel_end: Option<PathBuf>,
    /// `--online <addr> --seat N`: play seat N of an online match on that
    /// loopback server (`roses-server`), deck from `--deck-save`,
    /// `--deck-file` or the import's first duel (`--import-side`).
    online: Option<OnlineOptions>,
    /// `--end-replay`: pad words applied one per update from the first
    /// result frame (before the keyboard).
    end_replay: Vec<u16>,
    /// `--duel-models <manifest>`: unit models (`roses import-duel-models`).
    duel_models: Option<PathBuf>,
    /// `roses import-duel-arena` manifest (the battle arena).
    duel_arena: Option<PathBuf>,
    /// `roses import-duel-audio` manifest: the duel's music and sounds.
    duel_audio: Option<PathBuf>,
    /// `--multi-preview ffa|2v2` (a port addition, MODES; a test aid): an
    /// N-player duel of four computer players on this computer.
    multi_preview: Option<String>,
    /// `--multi-until over|ko|ko-show|round:N`: headless, run the preview until
    /// the placing list shows, a player is knocked out, or round N.
    multi_until: Option<String>,
}

/// The offline N-player preview's duel (a port addition; a test aid): the
/// import's starter deck and three computer opponents' decks round the
/// table, every seat a computer player.
#[cfg(feature = "online")]
fn multi_preview_play(dir: &std::path::Path, options: &Options) -> Result<DuelPlay, String> {
    use roses_game::duel_multi;
    let data = duel_play::load(dir)?;
    let mode = if options.multi_preview.as_deref() == Some("2v2") {
        roses_core::duel::MultiMode::Teams
    } else {
        roses_core::duel::MultiMode::FreeForAll
    };
    let (names, decks) = duel_multi::preview_seats(&data, 4);
    let duel = duel_multi::preview_duel(data.first_duel.terrain.clone(), &decks, mode, options.seed, &data.catalogue)?;
    eprintln!("Multi preview: {mode:?}, seats {names:?}, leaders {:?}", decks.iter().map(|d| d.0).collect::<Vec<_>>());
    // `ROSES_MULTI_PREVIEW_PACE`: updates between the computers' commands.
    let pace = std::env::var("ROSES_MULTI_PREVIEW_PACE").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
    DuelPlay::multi_preview(data, duel, None, names, pace)
}

#[cfg(not(feature = "online"))]
fn multi_preview_play(_: &std::path::Path, _: &Options) -> Result<DuelPlay, String> {
    Err("--multi-preview: the N-player modes are not in this build (the original game only)".into())
}

/// `--multi-until`: the preview reached the moment asked for.
fn multi_condition(play: &DuelPlay, until: &str) -> bool {
    #[cfg(feature = "online")]
    {
        let duel = &play.session.duel;
        // At rest: nothing presented, and the HUD back after a turn change.
        let idle = !play.animating();
        if let Some(n) = until.strip_prefix("round:").and_then(|n| n.parse::<u8>().ok()) {
            return idle && (duel.round >= n || duel.outcome.is_some());
        }
        match until {
            "ko" => idle && duel.multi().is_some_and(|m| m.out.iter().any(Option::is_some)),
            // A knockout's presentation in progress (LEAVE: the pieces and
            // puffs at the overview; STAY: the note over the dimmed cards).
            "ko-show" => play.multi.as_ref().is_some_and(|m| {
                m.knockout.as_ref().is_some_and(|k| k.u() >= 6)
                    || (m.knockout.is_none() && m.knockouts_waiting.is_empty() && m.out_note.is_some_and(|n| n.1 < roses_game::duel_multi::OUT_NOTE_UPDATES - 20) && idle)
            }),
            _ => play.multi.as_ref().is_some_and(|m| m.results >= 40),
        }
    }
    #[cfg(not(feature = "online"))]
    {
        let _ = (play, until);
        true
    }
}

fn number<T: std::str::FromStr>(value: Option<OsString>, what: &str) -> Result<T, String> {
    let text = value
        .ok_or(format!("{what} requires a value"))?
        .into_string()
        .map_err(|_| format!("{what} must be UTF-8"))?;
    let parsed = if let Some(hex) = text.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
            .ok()
            .and_then(|v| v.to_string().parse().ok())
    } else {
        text.parse().ok()
    };
    parsed.ok_or(format!("Invalid {what}: {text}"))
}
fn pair(value: Option<OsString>, what: &str, max: u8) -> Result<(u8, u8), String> {
    let text = value
        .ok_or(format!("{what} requires a value"))?
        .into_string()
        .map_err(|_| format!("{what} must be UTF-8"))?;
    let parts: Vec<u8> = text
        .split(',')
        .map(|p| {
            p.trim()
                .parse()
                .map_err(|_| format!("Invalid {what}: {text}"))
        })
        .collect::<Result<_, _>>()?;
    match parts[..] {
        [a, b] if a <= max && b <= max => Ok((a, b)),
        _ => Err(format!("{what} must be two numbers 0..={max}")),
    }
}

/// The online seat's connection and deck choice.
#[derive(Clone, Debug, Default)]
struct OnlineOptions {
    addr: String,
    seat: Option<u8>,
    resume: Option<String>,
    deck_save: Option<PathBuf>,
    deck_number: Option<u8>,
    deck_file: Option<PathBuf>,
    import_side: Option<usize>,
    /// `--online-bot`: this seat is played through its pad by the pad-level
    /// computer player (`duel_online::PadBot`), streaming its presses as a
    /// person's would (a sparring partner; headless captures).
    bot: bool,
    /// Video aid: `--bot-pace off|human`, `--bot-seed N`, `--bot-ai
    /// pad|yugi|seto|<control byte>`, `--bot-tier N`, `--bot-log <file>`
    /// (over the `ROSES_BOT_*` environment; `video_bot`). A human pace
    /// also runs the headless updates at 25 Hz (real time).
    bot_config: Option<roses_game::video_bot::BotConfig>,
    /// `--online-until remote-hand|remote-pick|remote-flight|result|end`:
    /// headless, keep running updates after the scripted ones until the
    /// other seat's hand row is open / its picked cards are held / its
    /// summon card flies, the 2-player result banner waits for a press, or
    /// the match is over (then capture or report).
    until: Option<String>,
}

/// The headless online stop condition (`--online-until`), true once it has
/// held for a few updates (the HUD and camera settled).
fn online_condition(play: &DuelPlay, end: Option<&EndRuntime>, until: &str, settled: &mut u32) -> bool {
    use roses_core::duel::ui::Mode;
    let Some(link) = play.online.as_ref() else {
        return true;
    };
    let remote = play.session.duel.active != link.seat && play.session.duel.outcome.is_none();
    let mode = &play.session.input.mode;
    let hit = match until {
        "remote-hand" => remote && matches!(mode, Mode::Hand(_)) && play.hand_anim.is_none(),
        "remote-pick" => remote && matches!(mode, Mode::Confirm(_)) && play.hand_anim.is_none(),
        "remote-flight" => remote && play.summon_flight.as_ref().is_some_and(|f| f.step >= 12 && f.fusion_playing().is_none()),
        "result" => end.and_then(|e| e.active.as_ref()).is_some_and(|a| a.end.view().state == 0x5C),
        _ => link.ended() && link.state.revision == link.shown_revision && !play.animating(),
    };
    *settled = if hit { *settled + 1 } else { 0 };
    let over = link.ended() && link.state.revision == link.shown_revision && !play.animating();
    match until {
        "remote-flight" | "end" => hit,
        // The result banner settles for a few updates.
        "result" => *settled >= 8,
        // The other seat's menus last as long as it reads them (a bot
        // presses every update): its own presentation, then this one.
        _ if over => {
            eprintln!("Online: the match ended before --online-until {until}");
            true
        }
        _ => *settled >= 1,
    }
}

/// Online play is not in a build without the `online` feature.
#[cfg(not(feature = "online"))]
fn online_play(_: duel_play::DuelData, _: &OnlineOptions) -> Result<DuelPlay, String> {
    Err("--online: Online Play is not in this build (the original game only)".into())
}

/// Join the online match (blocking until both seats are in, up to 10
/// minutes) and build the online duel from the user's duel import.
#[cfg(feature = "online")]
fn online_play(data: duel_play::DuelData, o: &OnlineOptions) -> Result<DuelPlay, String> {
    use roses_game::duel_online::{ClientConn, Connector, OnlineLink};
    use roses_net::transport::FramedTcp;
    use roses_net::{ClientMessage, Join, ServerMessage};
    let addr: std::net::SocketAddr = o.addr.parse().map_err(|_| format!("--online {} is not an address", o.addr))?;
    if !addr.ip().is_loopback() {
        return Err("--online accepts loopback addresses only (127.0.0.1 / ::1)".into());
    }
    let deck = match (&o.deck_file, &o.deck_save) {
        (Some(file), _) => roses_net::deck::from_json_file(file)?,
        (None, Some(save)) => roses_net::deck::from_save_file(save, o.deck_number)?,
        (None, None) => {
            // The import's first-duel deck of the side this seat plays
            // (the starter deck for seat 0, Weevil's for seat 1).
            let side = &data.first_duel.sides[o.import_side.unwrap_or(usize::from(o.seat.unwrap_or(0) & 1))];
            roses_net::deck::from_import_side(&roses_net::data::ImportSide { leader: side.leader, cards: side.cards.clone() })
        }
    };
    roses_net::check_deck(&deck, &data.catalogue).map_err(|p| format!("this deck would be refused: {p}"))?;
    // The leader ability table and Destiny Draw rows of this import join
    // the rules fingerprint the server checks.
    let tables = roses_net::RulesTables::from_executable(&data.executable)?;
    eprintln!(
        "Online: deck with leader {} (rank {}), {} cards; connecting to {addr}",
        deck.leader,
        deck.leader_rank,
        deck.cards.len()
    );
    let join = match &o.resume {
        Some(token) => Join::Resume { token: token.clone() },
        None => Join::Seat { seat: o.seat },
    };
    let connector: Connector = Box::new(move || {
        FramedTcp::<ClientMessage, ServerMessage>::connect(addr).map(|c| Box::new(c) as ClientConn)
    });
    let link = OnlineLink::connect(
        connector,
        &data.catalogue,
        &tables,
        join,
        Some(deck),
        std::time::Duration::from_secs(600),
        |line| eprintln!("Online: {line}"),
    )?;
    eprintln!(
        "Online: the match began; this window plays seat {} ({}P; resume token {})",
        link.seat,
        link.seat + 1,
        link.state.token.clone().unwrap_or_default()
    );
    let o = link.options();
    let say = |b: [bool; 2], on: &str, off: &str| format!("{} / {}", if b[0] { on } else { off }, if b[1] { on } else { off });
    eprintln!(
        "Online: the server's switches (1P / 2P): Leader Ability {}, Card Display {}{}",
        say(o.leader_ability, "USE", "NOT USE"),
        say(o.card_display_open, "OPEN", "HIDE"),
        if o.card_display_open[usize::from(link.seat & 1)] { "" } else { " (this seat's hand and face-down cards stay hidden from the other seat)" }
    );
    DuelPlay::online(data, link)
}

fn parse(args: impl Iterator<Item = OsString>) -> Result<Options, String> {
    let mut options = Options {
        manifest: PathBuf::new(),
        cursor: (3, 6),
        map: DEFAULT_MAP,
        terrain: None,
        window_kinds: DEFAULT_WINDOW_KINDS,
        backdrop_angle: 0,
        brightness: 128,
        gs16: true,
        capture: None,
        validate_only: false,
        replay: Vec::new(),
        ticks: None,
        units: None,
        overlays: true,
        play: None,
        opponent: None,
        seed: duel_play::DEFAULT_SEED,
        computer_player: false,
        replay_pad: Vec::new(),
        duel_end: None,
        online: None,
        duel_models: None,
        duel_arena: None,
        duel_audio: None,
        end_replay: Vec::new(),
        multi_preview: None,
        multi_until: None,
    };
    let mut manifest = None;
    // Options owned by the duel session in duel play, and duel-play options.
    let mut board_only = Vec::new();
    let mut play_only = Vec::new();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--duel-board") if manifest.is_none() => {
                manifest = Some(PathBuf::from(
                    args.next().ok_or("--duel-board requires a manifest path")?,
                ));
            }
            // Faithful mode (lib.rs `faithful`) is read from the arguments.
            Some("--faithful") => {}
            Some("--duel-play") if options.play.is_none() => {
                options.play = Some(PathBuf::from(
                    args.next()
                        .ok_or("--duel-play requires a duel import directory")?,
                ));
            }
            Some("--online") => {
                let addr = args.next().ok_or("--online requires a loopback address")?;
                let addr = addr.into_string().map_err(|_| "--online must be UTF-8")?;
                options.online.get_or_insert_with(OnlineOptions::default).addr = addr;
                play_only.push("--online");
            }
            Some(flag @ ("--multi-preview" | "--multi-until")) => {
                let flag = flag.to_string();
                let value = args
                    .next()
                    .ok_or(format!("{flag} requires a value"))?
                    .into_string()
                    .map_err(|_| format!("{flag} must be UTF-8"))?;
                if flag == "--multi-preview" {
                    if !matches!(value.as_str(), "ffa" | "2v2") {
                        return Err("--multi-preview is ffa or 2v2".into());
                    }
                    options.multi_preview = Some(value);
                } else {
                    options.multi_until = Some(value);
                }
                play_only.push("--multi-preview");
            }
            Some("--online-bot") => {
                options.online.get_or_insert_with(OnlineOptions::default).bot = true;
                play_only.push("--online");
            }
            Some(flag @ ("--bot-pace" | "--bot-seed" | "--bot-ai" | "--bot-tier" | "--bot-log")) => {
                use roses_game::video_bot as vb;
                let flag = flag.to_string();
                let value = args
                    .next()
                    .ok_or(format!("{flag} requires a value"))?
                    .into_string()
                    .map_err(|_| format!("{flag} must be UTF-8"))?;
                let o = options.online.get_or_insert_with(OnlineOptions::default);
                let c = o.bot_config.get_or_insert_with(vb::BotConfig::from_env);
                match flag.as_str() {
                    "--bot-pace" => c.pace = vb::Pace::parse(&value)?,
                    "--bot-seed" => c.seed = value.parse().map_err(|_| "--bot-seed is a number")?,
                    "--bot-ai" => c.controller = vb::controller_of(&value)?,
                    "--bot-tier" => c.tier = value.parse().map_err(|_| "--bot-tier is a number")?,
                    _ => c.log = Some(value.into()),
                }
                play_only.push("--online");
            }
            Some(flag @ ("--seat" | "--resume" | "--deck-save" | "--deck" | "--deck-file" | "--import-side" | "--online-until")) => {
                let flag = flag.to_string();
                let value = args
                    .next()
                    .ok_or(format!("{flag} requires a value"))?
                    .into_string()
                    .map_err(|_| format!("{flag} must be UTF-8"))?;
                let o = options.online.get_or_insert_with(OnlineOptions::default);
                match flag.as_str() {
                    "--seat" => o.seat = Some(value.parse().ok().filter(|s: &u8| *s < 2).ok_or("--seat is 0 or 1")?),
                    "--resume" => o.resume = Some(value),
                    "--deck-save" => o.deck_save = Some(value.into()),
                    "--deck" => o.deck_number = Some(value.parse().ok().filter(|d: &u8| *d < 3).ok_or("--deck is 0, 1 or 2")?),
                    "--deck-file" => o.deck_file = Some(value.into()),
                    "--online-until" => {
                        if !matches!(value.as_str(), "remote-hand" | "remote-pick" | "remote-flight" | "result" | "end") {
                            return Err("--online-until is remote-hand, remote-pick, remote-flight, result or end".into());
                        }
                        o.until = Some(value)
                    }
                    _ => o.import_side = Some(value.parse().ok().filter(|s: &usize| *s < 2).ok_or("--import-side is 0 or 1")?),
                }
                play_only.push("--online");
            }
            Some("--seed") => {
                options.seed = number(args.next(), "--seed")?;
                play_only.push("--seed");
            }
            Some("--opponent") => {
                let o: u64 = number(args.next(), "--opponent")?;
                let node = options.opponent.map_or(0, |(_, n)| n);
                options.opponent = Some((o as u8, node));
                play_only.push("--opponent");
            }
            Some("--node") => {
                let n: u64 = number(args.next(), "--node")?;
                let o = options.opponent.map_or(2, |(o, _)| o);
                options.opponent = Some((o, n as u8));
                play_only.push("--node");
            }
            Some("--player") => {
                options.computer_player = match args.next().as_ref().and_then(|v| v.to_str()) {
                    Some("human") => false,
                    Some("computer") => true,
                    _ => return Err("--player must be human or computer".into()),
                };
                play_only.push("--player");
            }
            Some("--replay-pad") => {
                let text = args
                    .next()
                    .ok_or("--replay-pad requires comma-separated pad buttons")?
                    .into_string()
                    .map_err(|_| "--replay-pad must be UTF-8")?;
                options.replay_pad = duel_play::parse_replay_pad(&text)?;
                play_only.push("--replay-pad");
            }
            Some("--cursor") => {
                options.cursor = pair(args.next(), "--cursor", 6)?;
                board_only.push("--cursor");
            }
            Some("--map") => options.map = number(args.next(), "--map")?,
            Some("--terrain") => {
                let text = args
                    .next()
                    .ok_or("--terrain requires 49 digits")?
                    .into_string()
                    .map_err(|_| "--terrain must be UTF-8")?;
                options.terrain = Some(duel_board::parse_terrain(&text)?);
            }
            Some("--window-kinds") => {
                let (a, b) = pair(args.next(), "--window-kinds", 20)?;
                options.window_kinds = [a, b];
            }
            Some("--backdrop-angle") => {
                options.backdrop_angle = number(args.next(), "--backdrop-angle")?
            }
            Some("--brightness") => {
                options.brightness = number(args.next(), "--brightness")?;
                if options.brightness > 128 {
                    return Err("--brightness is the u8 0x1F7F30 value, 0..=128".into());
                }
            }
            Some("--rgb888") => options.gs16 = false,
            Some("--no-overlays") => {
                options.overlays = false;
                board_only.push("--no-overlays");
            }
            Some("--duel-units") if options.units.is_none() => {
                options.units = Some(PathBuf::from(
                    args.next().ok_or("--duel-units requires a JSON path")?,
                ));
                board_only.push("--duel-units");
            }
            Some("--replay") => {
                board_only.push("--replay");
                let text = args
                    .next()
                    .ok_or("--replay requires comma-separated arrows")?
                    .into_string()
                    .map_err(|_| "--replay must be UTF-8")?;
                options.replay = text
                    .split(',')
                    .map(|step| match step.trim() {
                        "up" => Ok((0, -1)),
                        "down" => Ok((0, 1)),
                        "left" => Ok((-1, 0)),
                        "right" => Ok((1, 0)),
                        other => Err(format!("Unknown replay step {other:?}")),
                    })
                    .collect::<Result<_, _>>()?;
                if options.replay.len() > 256 {
                    return Err("--replay accepts at most 256 steps".into());
                }
            }
            Some("--duel-end") if options.duel_end.is_none() => {
                options.duel_end = Some(PathBuf::from(
                    args.next()
                        .ok_or("--duel-end requires an import directory")?,
                ));
                play_only.push("--duel-end");
            }
            Some("--duel-audio") if options.duel_audio.is_none() => {
                options.duel_audio = Some(PathBuf::from(
                    args.next()
                        .ok_or("--duel-audio requires an audio manifest")?,
                ));
                play_only.push("--duel-audio");
            }
            Some("--duel-arena") if options.duel_arena.is_none() => {
                options.duel_arena = Some(PathBuf::from(
                    args.next()
                        .ok_or("--duel-arena requires an arena manifest")?,
                ));
                play_only.push("--duel-arena");
            }
            Some("--duel-models") if options.duel_models.is_none() => {
                options.duel_models = Some(PathBuf::from(
                    args.next()
                        .ok_or("--duel-models requires a models manifest")?,
                ));
                play_only.push("--duel-models");
            }
            Some("--end-replay") => {
                play_only.push("--end-replay");
                let text = args
                    .next()
                    .ok_or("--end-replay requires comma-separated presses")?
                    .into_string()
                    .map_err(|_| "--end-replay must be UTF-8")?;
                options.end_replay = duel_play::parse_replay_pad(&text)?;
            }
            Some("--ticks") => {
                let ticks: u32 = number(args.next(), "--ticks")?;
                if ticks > MAX_TICKS {
                    return Err(format!("--ticks accepts at most {MAX_TICKS} updates"));
                }
                options.ticks = Some(ticks);
            }
            Some("--validate-only") => options.validate_only = true,
            Some("--capture") if options.capture.is_none() => {
                options.capture = Some(PathBuf::from(
                    args.next().ok_or("--capture requires a new PNG path")?,
                ));
            }
            Some("--help") => {
                println!(
                    "roses-bevy-scene --duel-board <duel-board manifest.json> [--duel-units <overlay-input.json> | --no-overlays] [--cursor c,r] [--map 0..45 | --terrain <49 digits>] [--window-kinds k0,k1] [--backdrop-angle u16] [--brightness 0..128] [--rgb888] [--replay right,up,... [--ticks n]] [--validate-only | --capture <new.png>]\n--duel-units is a duel_overlay::OverlayInput (state, active_side, marks, move_allowance, path, brightness, units, pulse, settled); without it the board shows the browsing cross and cursor. --replay applies one cursor step per 25 Hz update from the start state, then runs --ticks updates in total (default: steps + 8, at most {MAX_TICKS}) before the first frame. Arrow keys move the side-0 cursor one cell with the original linear 8-update camera track at 25 Hz; Escape closes. {NOT_IMPLEMENTED}.\n\nroses-bevy-scene --duel-board <manifest.json> --duel-play <duel import dir> [--seed u64 (default {})] [--player human|computer] [--replay-pad square,up,cross*2,none,up+left,... [--ticks n]] [--duel-end <duel-end import dir> [--end-replay none*80,cross,...]] [--duel-models <duel-models manifest.json>] [--map n | --terrain <49 digits>] [--validate-only | --capture <new.png>]\nPlays the first campaign duel against Weevil; with --duel-end the duel's end runs the result screens over the board (--end-replay scripts their pad from the first result frame); with --duel-models leaders and face-up monsters are drawn with their MONSTER.MRG models (the cursor unit over the board, under the HUD); the session owns the cursor and units (no --cursor, --replay, --duel-units or --no-overlays). --replay-pad presses the listed buttons (none = no press, a+b together, x*n = n updates; at most {} entries) one per 25 Hz update from the start, then runs --ticks updates in total (default: entries + 8, at most {MAX_TICKS}). Keys: arrows = d-pad, X Cross, A Square, Z Circle, D Triangle, S Start, Tab Select, Q L1, E R1, 1 L2, W R2 (held arrows repeat from the 13th update after the press; other buttons act once per press); Escape closes. The window title shows round, side to act, LP, stars, input state and the last refused or unsupported press.

roses-bevy-scene --game <game.json> --online <127.0.0.1:port> [--seat 0|1 | --resume <token>] [--deck-save <memory-card slot folder or data file> [--deck 0|1|2] | --deck-file <deck.json> | --import-side 0|1] [--replay-pad ... --capture <new.png>] [--online-bot] [--online-until remote-hand|remote-pick|remote-flight|result|end]
Plays one seat of an online duel against a loopback `roses-server` (the duel parts of the game import; or --duel-board <manifest> --duel-play <duel import> --online ...): joins with the seat's own deck (refused before the match when it breaks the deck rules), waits for the other seat, then the pad drives the input states on the seat's copy of the duel and every intention goes to the server; the board and HUD show the server's revisions, and the other seat's turn shows its streamed presses (cursor, menus, hand row, summon flight) as the original's 2-player duel does. The server decides Leader Ability / Card Display (HIDE keeps a seat's hand and face-down cards from the other seat; a seat always reads its own). The duel ends with the 1P WIN / 2P WIN / DRAW result; the window closes after it. A scripted --replay-pad press waits until the seat may act. Headless: --online-bot plays this seat through its pad (a streaming computer player), --online-until runs updates until that moment (then --capture or --validate-only).",
                    duel_play::DEFAULT_SEED,
                    duel_play::MAX_REPLAY_PAD
                );
                std::process::exit(0);
            }
            _ => {
                return Err(format!(
                    "Unknown duel-board argument: {}",
                    arg.to_string_lossy()
                ));
            }
        }
    }
    options.manifest = manifest.ok_or("--duel-board requires a manifest path")?;
    if options.play.is_some() {
        if let Some(flag) = board_only.first() {
            return Err(format!(
                "{flag} cannot be combined with --duel-play (the duel session owns the cursor and units)"
            ));
        }
    } else if let Some(flag) = play_only.first() {
        return Err(format!("{flag} requires --duel-play"));
    }
    if options.multi_preview.is_some() || options.multi_until.is_some() {
        if roses_game::faithful() {
            return Err("--multi-preview: the N-player modes are a port addition, not available in faithful mode".into());
        }
        if options.online.is_some() || options.opponent.is_some() || options.multi_preview.is_none() {
            return Err("--multi-preview cannot be combined with --online or --opponent (and --multi-until needs it)".into());
        }
    }
    if let Some(o) = &options.online {
        if roses_game::faithful() {
            return Err("--online: Online Play is a port addition, not available in faithful mode (--faithful, or a build without the online feature)".into());
        }
        if o.addr.is_empty() {
            return Err("--seat, --resume and the deck flags need --online <addr>".into());
        }
        if options.opponent.is_some() || options.computer_player {
            return Err("--online cannot be combined with --opponent or --player computer".into());
        }
    }
    if options
        .ticks
        .is_some_and(|t| (t as usize) < options.replay.len().max(options.replay_pad.len()))
    {
        return Err("--ticks must cover every --replay / --replay-pad step".into());
    }
    if options.validate_only && options.capture.is_some() {
        return Err("--validate-only and --capture cannot be combined".into());
    }
    if !options.overlays && options.units.is_some() {
        return Err("--duel-units and --no-overlays cannot be combined".into());
    }
    if let Some(path) = &options.capture {
        if path
            .extension()
            .and_then(|v| v.to_str())
            .is_none_or(|v| !v.eq_ignore_ascii_case("png"))
        {
            return Err("Capture output must have a .png extension".into());
        }
        if path.exists() {
            return Err(format!("Capture output already exists: {}", path.display()));
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        if !parent.is_dir() {
            return Err(format!(
                "Capture output directory does not exist: {}",
                parent.display()
            ));
        }
    }
    Ok(options)
}

fn load_units(path: &std::path::Path) -> Result<OverlayInput, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(file, 1024 * 1024 + 1), &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("--duel-units file exceeds 1 MiB".into());
    }
    let input: OverlayInput =
        serde_json::from_slice(&bytes).map_err(|e| format!("--duel-units: {e}"))?;
    input.validate()?;
    Ok(input)
}

fn title(state: &BoardState) -> String {
    let (c, r) = state.board_camera.cursor;
    format!(
        "Roses Bevy duel board · cursor ({c},{r}) · original terrain, arena, windows, cell and unit layers · {NOT_IMPLEMENTED}"
    )
}

fn board_button(event: &KeyboardInput) -> Option<(i8, i8)> {
    if event.state != ButtonState::Pressed || event.repeat {
        return None;
    }
    let physical = match event.key_code {
        KeyCode::ArrowUp => Some((0, -1)),
        KeyCode::ArrowDown => Some((0, 1)),
        KeyCode::ArrowLeft => Some((-1, 0)),
        KeyCode::ArrowRight => Some((1, 0)),
        _ => None,
    };
    physical.or(match event.logical_key {
        Key::ArrowUp => Some((0, -1)),
        Key::ArrowDown => Some((0, 1)),
        Key::ArrowLeft => Some((-1, 0)),
        Key::ArrowRight => Some((1, 0)),
        _ => None,
    })
}

fn board_input(mut events: MessageReader<KeyboardInput>, mut state: ResMut<BoardState>) {
    for event in events.read() {
        if let Some(step) = board_button(event) {
            state.pending.push(step);
        }
    }
}

/// Duel-play pad bits of a key event: the physical key (`duel_play::KEYBOARD`);
/// only an otherwise-unmapped physical key falls back to a logical named
/// arrow (the observed Windows Numpad/arrow case, as in the Simon preview).
fn play_pad_bits(event: &KeyboardInput) -> u16 {
    let physical = duel_play::key_pad(&format!("{:?}", event.key_code));
    if physical != 0 {
        return physical;
    }
    match event.logical_key {
        Key::ArrowUp => pad::UP,
        Key::ArrowDown => pad::DOWN,
        Key::ArrowLeft => pad::LEFT,
        Key::ArrowRight => pad::RIGHT,
        _ => 0,
    }
}

/// Keyboard → held pad buttons for duel play (runs every frame, before the
/// fixed updates). Releases are matched by physical key; focus loss
/// releases everything.
fn play_keys(
    mut events: MessageReader<KeyboardInput>,
    mut focus_lost: MessageReader<KeyboardFocusLost>,
    mut play: ResMut<PlayRuntime>,
) {
    if !play.keys_primed {
        // Presses from before the scene existed are held, not fresh.
        play.keys_primed = true;
        events.read().count();
        focus_lost.read().count();
        return;
    }
    for event in events.read() {
        match event.state {
            ButtonState::Pressed if !event.repeat => {
                let bits = play_pad_bits(event);
                // N-player (a port addition): T chooses the next target.
                #[cfg(feature = "online")]
                if event.key_code == KeyCode::KeyT {
                    if let Some(m) = play.play.multi.as_mut() {
                        m.target_key = true;
                    }
                }
                if bits != 0 {
                    play.held.insert(event.key_code, bits);
                    play.tapped |= bits;
                }
            }
            ButtonState::Pressed => {}
            ButtonState::Released => {
                play.held.remove(&event.key_code);
            }
        }
    }
    if focus_lost.read().count() > 0 {
        play.held.clear();
    }
}

/// One duel update at 25 Hz, before `board_tick`: held buttons become new
/// presses (plus the original d-pad/L1/R1 auto-repeat) for one session
/// frame; then the board camera steps toward the active side's cursor and
/// the cell/unit layer input is rebuilt from the session. After a session error nothing ticks and the last state
/// stays on screen, with the error in the title.
fn play_tick(
    mut play: ResMut<PlayRuntime>,
    pads: Option<ResMut<crate::pad_input::Pads>>,
    mut state: ResMut<BoardState>,
    mut overlay: ResMut<OverlayRuntime>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    tutorial: Option<Res<TutorialRuntime>>,
    capture: Option<Res<CaptureState>>,
    mut models: Option<ResMut<UnitModels>>,
    live: Option<Res<CaptureLive>>,
) {
    let runtime = &mut *play;
    let held = runtime
        .held
        .values()
        .fold(runtime.tapped, |acc, &bits| acc | bits);
    runtime.tapped = 0;
    // Pad 1's gamepad / L3-R3 keys and pad 2 (a duel with two humans reads
    // the active side's port, 0x1CFF80).
    let (extra1, held2, sticks) = match pads {
        Some(mut p) => (p.pad1_extra(), p.take(1), [p.sticks(0), p.sticks(1)]),
        None => (0, 0, [None; 2]),
    };
    // The analog sticks of both ports (the duel camera, 0x1D3C7C).
    runtime.play.sticks = sticks;
    let held = held | extra1;
    // A headless capture of the game shows the state the pre-run updates
    // reached (--tutorial-ticks); the session does not advance while the
    // GPU readies.
    // Test aid (a port addition, MODES): an N-player duel captured with
    // `ROSES_MULTI_CAPTURE_ROUND` plays on until that round.
    #[cfg(feature = "online")]
    let multi_live = runtime.play.multi.is_some() && !runtime.capture_frozen && std::env::var_os("ROSES_MULTI_CAPTURE_ROUND").is_some();
    #[cfg(not(feature = "online"))]
    let multi_live = false;
    if let Some(bot) = runtime.bot.as_mut().filter(|_| capture.is_none() || multi_live || video_bot_in_capture()) {
        // Test aid: the pad-level computer player presses for this seat
        // (as the headless `--online-bot` loop does), and a face button
        // every 25 updates once the result screens wait for one.
        let play = &mut runtime.play;
        if let Some(link) = play.online.as_ref() {
            bot.observe(link.shown_revision, &play.session.duel, &play.data.names);
        }
        let key = if play.shown_outcome().is_some() {
            if play.frames % 25 == 0 { roses_core::duel::ai::pad::CROSS } else { 0 }
        } else {
            match play.online.as_ref() {
                Some(link) => {
                    let can = !play.animating() && link.may_act();
                    match link.state.view.clone() {
                        Some(view) => {
                            let revision = link.state.revision;
                            let k = bot.key(&play.session.duel, &play.session.input, &view, revision, can, &play.data.catalogue, &play.data.tables);
                            // N-player (MODES): the bot's Target key.
                            #[cfg(feature = "online")]
                            if bot.take_target() {
                                if let Some(m) = play.multi.as_mut() {
                                    m.target_key = true;
                                }
                            }
                            k
                        }
                        None => 0,
                    }
                }
                None => 0,
            }
        };
        play.press(key);
        play.last_presses = key;
    } else if capture.is_none() || multi_live || live.is_some_and(|l| l.remaining > 0) {
        runtime.play.update_pads([held, held2]);
        // ROSES_PAD_TRACE=1: the words this duel update read and the
        // presses (new + d-pad repeat) they gave.
        if roses_game::pads::trace_enabled() && (held | held2 | runtime.play.last_presses != 0 || runtime.trace_pads != [held, held2]) {
            eprintln!(
                "ROSES_PAD_TRACE duel update {}: held {held:#06x} pad2 {held2:#06x} -> presses {:#06x}",
                runtime.play.frames, runtime.play.last_presses
            );
        }
        runtime.trace_pads = [held, held2];
    }
    let target = runtime.play.cursor();
    let radius = std::cell::RefCell::new(models.as_deref_mut());
    // A lesson's camera requests of this update (close-up, overview, zoom,
    // save/restore, CURSOR) before the camera follows the cursor.
    let lesson = runtime.play.tutorial().map(|t| t.lesson.controller());
    if let (Some(controller), true) = (lesson, tutorial.is_some()) {
        let ops = runtime.play.last_script.as_ref().map(|s| s.camera.clone()).unwrap_or_default();
        for note in duel_play::tutorial_camera(
            &mut state.board_camera,
            &mut runtime.play,
            controller,
            &ops,
            &|card| radius.borrow_mut().as_mut().and_then(|m| m.radius(card)),
        ) {
            eprintln!("Tutorial: {note}");
        }
    }
    duel_play::drive_camera(
        &mut state.board_camera,
        &mut runtime.play,
        target,
        &|card| radius.borrow_mut().as_mut().and_then(|m| m.radius(card)),
    );
    let input = runtime.play.overlay_input();
    if overlay.input != input {
        overlay.input = input;
    }
    overlay.state.glow_hold = runtime.play.glow_hold;
    let rests = !runtime.play.animating()
        && runtime.hud.previous.d3e == 0
        && !state.board_camera.moving()
        && state.board_camera.polar_track.is_none();
    runtime.rest_updates = if rests { runtime.rest_updates + 1 } else { 0 };
    let caption = runtime.play.title();
    if caption != runtime.title {
        for mut window in &mut windows {
            window.title = caption.clone();
        }
        if runtime.trace {
            eprintln!("ROSES_DUEL update {}: {caption}", runtime.play.frames);
        }
        runtime.title = caption;
    }
}

/// The duel HUD for this update (after the session frame): the HUD state
/// from the session and the routine's last write-backs, then the layer,
/// re-rendered and re-uploaded only when the primitive list changed (a few
/// ms per render; most updates change nothing).
fn play_hud(
    mut play: ResMut<PlayRuntime>,
    mut layer: ResMut<HudLayer>,
    mut images: ResMut<Assets<Image>>,
    mut presents: ResMut<Assets<PresentMaterial>>,
    end: Option<Res<EndRuntime>>,
    state: Option<Res<BoardState>>,
) {
    let runtime = &mut *play;
    // The terrain picture's preview follows the main camera's pitch.
    if let Some(state) = state {
        let c = &state.board_camera;
        let (eye, target) = (c.eye(), c.target);
        runtime.hud.picture_forward = std::array::from_fn(|i| target[i] - eye[i]);
    }
    // FLAG906 (0x2DB906 bit 7) clear = the HUD slides out and stays hidden
    // (saved states 005-t5, 004-t4, 034-c2 show no HUD with it clear).
    if let Some(t) = runtime.play.tutorial() {
        runtime.hud.hidden = !t.hud_flag;
    }
    runtime.hud.update(&runtime.play);
    runtime.hud.render();
    // While the result screens draw, `end_tick` uploads the composite.
    let end_draws = end.is_some_and(|e| e.active.is_some());
    if runtime.hud.version != layer.version && !end_draws {
        let hud = &runtime.hud;
        layer.upload(&hud.layer, hud.version, &mut images, &mut presents);
    }
}

/// The duel's end (after `play_hud`): when the duel has an outcome the
/// result sequence starts (the HUD hides, the banner slides up from below,
/// the loss darkens the scene, a press slides it out, rank banners follow),
/// stepping once per update with the pad's new-press word; the darkening and
/// banner layer is composited over the HUD layer and uploaded.
fn end_tick(
    mut play: ResMut<PlayRuntime>,
    end: Option<ResMut<EndRuntime>>,
    mut layer: ResMut<HudLayer>,
    mut images: ResMut<Assets<Image>>,
    mut presents: ResMut<Assets<PresentMaterial>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Some(mut end) = end else {
        return;
    };
    let runtime = &mut *play;
    let title = runtime.title.clone();
    let Some((composite, caption)) =
        end_advance(&mut runtime.play, &mut runtime.hud, &title, &mut end)
    else {
        return;
    };
    if let Some(composite) = composite {
        layer.upload(&composite, runtime.hud.version, &mut images, &mut presents);
    }
    for mut window in &mut windows {
        if window.title != caption {
            window.title = caption.clone();
        }
    }
}

/// A practice lesson's presentation (after `play_hud`): the script's
/// one-shots of this update are played (sounds) or logged (unit model
/// effects, the Exodia event, the Destiny Draw, the lesson's end; the
/// camera requests are applied in `play_tick`), the fade bytes set the scene brightness of
/// the present pass, and the tutorial layer (window, caption, pointer) is
/// re-rendered when its view changed and composited over the HUD layer.
fn tutorial_tick(
    mut play: ResMut<PlayRuntime>,
    tutorial: Option<ResMut<TutorialRuntime>>,
    mut layer: ResMut<HudLayer>,
    mut images: ResMut<Assets<Image>>,
    mut presents: ResMut<Assets<PresentMaterial>>,
    mut sound: Option<ResMut<DuelSound>>,
    board: Option<Res<BoardState>>,
) {
    let Some(mut tut) = tutorial else {
        return;
    };
    let runtime = &mut *play;
    let Some(t) = runtime.play.tutorial() else {
        return;
    };
    // Test aid (ROSES_TUTORIAL_CAMERA_TRACE=1): the board camera as each
    // message page opens, to compare with the original's saved states
    // (the project notes: camera 0x2DF5C0 +0x00 polar, +0x30 target).
    let page = t.message.as_ref().map(|m| m.page);
    if page != tut.traced_page {
        tut.traced_page = page;
        if let (Some(page), Some(board)) = (page, board.as_ref()) {
            if std::env::var_os("ROSES_TUTORIAL_CAMERA_TRACE").is_some() {
                let c = &board.board_camera;
                eprintln!(
                    "Tutorial camera: lesson {} step {} page {page}: cursor ({},{}) target ({:.1}, {:.1}, {:.1}) polar ({:.1}, {:.4}, {:.4})",
                    t.lesson.controller(),
                    t.step,
                    c.cursor.0,
                    c.cursor.1,
                    c.target[0],
                    c.target[1],
                    c.target[2],
                    c.polar[0],
                    c.polar[1],
                    c.polar[2]
                );
            }
        }
    }
    let fade = f32::from(t.fade[0]) / f32::from(roses_core::duel::script::FADE_VISIBLE);
    let view = TutorialView::of(t);
    let (lesson, step) = (t.lesson.controller(), t.step);
    if let Some(script) = runtime.play.last_script.take() {
        let note = |tut: &mut TutorialRuntime, text: String| {
            if tut.reported.insert(text.clone()) {
                eprintln!("Tutorial: {text}");
            }
        };
        // Page 3 / last page 4 (0x280C80), pointer blink 0x13 (0x280690),
        // SYSTEM0 [F UI-SOUNDS.md 6].
        for &id in &script.sounds {
            match sound.as_mut() {
                Some(sound) => sound.cue(roses_game::duel_audio::AudioCue::Se { slot: 0, id, skip: false }),
                None => note(&mut tut, format!("SFX {id:#x} (no sound output)")),
            }
        }
        for op in &script.camera {
            match op {
                // Applied to the board camera in `play_tick`
                // (`duel_play::tutorial_camera`).
                CameraOp::CloseUp { .. }
                | CameraOp::Restore
                | CameraOp::Save
                | CameraOp::Preset { .. }
                | CameraOp::Zoom
                | CameraOp::Cursor { .. } => {}
                CameraOp::UnitFx { side, slot, value }
                | CameraOp::UnitFx2 { side, slot, value } => note(
                    &mut tut,
                    format!(
                        "unit model effect (0x1E2B80/0x1E2D70) side {side} slot {slot} value {value} is not reproduced [H meaning]"
                    ),
                ),
            }
        }
        if script.exodia_event {
            eprintln!(
                "Tutorial: lesson {lesson} step {step}: Exodia assembled at the scripted side's turn start: the original shows the Exodia event (not drawn; no win)"
            );
        }
        if let Some(d) = &script.destiny {
            eprintln!(
                "Tutorial: lesson {lesson} step {step}: Destiny Draw ({d:?}); the \"Destiny Draw!\" box and its animation are not drawn"
            );
        }
        if let Some(end) = script.ended {
            eprintln!("Tutorial: lesson {lesson} ended at step {step}: {end:?}");
        }
    }
    if let Some(mut present) = layer.present.clone().and_then(|h| presents.get_mut(&h)) {
        if (present.params.y - fade).abs() > 1e-6 {
            present.params.y = fade;
        }
    }
    let changed = tut.screen.render(&view);
    for n in tut.screen.take_notes() {
        if tut.reported.insert(n.clone()) {
            eprintln!("Tutorial: unsupported, not drawn: {n}");
        }
    }
    let hud_version = runtime.hud.version;
    if changed || tut.hud_version != Some(hud_version) {
        let composite = composite_over(&runtime.hud.layer, &tut.screen.layer);
        layer.upload(&composite, hud_version, &mut images, &mut presents);
        tut.hud_version = Some(hud_version);
    }
}

/// One update of the result sequence (Bevy-free): starts it at the duel's
/// outcome, steps it with the scripted or keyboard press, re-renders the
/// banner layer when what it shows changed. Returns the composite to upload
/// (when it changed) and the window caption; `None` before the duel ends or
/// after the sequence finished.
fn end_advance(
    play: &mut DuelPlay,
    hud: &mut DuelHud,
    title: &str,
    end: &mut EndRuntime,
) -> Option<(Option<Vec<u8>>, String)> {
    if end.active.is_none() {
        let outcome = play.shown_outcome()?;
        if play.halted.is_some() {
            return None;
        }
        let inputs = end.inputs.take()?;
        match start_end(play, hud, inputs) {
            Ok(active) => {
                eprintln!(
                    "Duel end: {:?} ({:?}); result screens start (waits: sound 0, HUD slide 16, camera 0 updates [H])",
                    outcome.winner, outcome.reason
                );
                end.active = Some(active);
            }
            Err(e) => {
                eprintln!("Duel end: result screens cannot start: {e}");
                play.halted = Some(format!("result screens: {e}"));
                return None;
            }
        }
    }
    let active = end.active.as_mut()?;
    if active.outcome.is_some() {
        return None;
    }
    // 0x5C waits on the human pad(s): both ports with two humans
    // (0x1D89E0..0x1D8A60) [F].
    let both = if play.two_humans() { play.last_presses | play.last_presses2 } else { play.last_presses };
    let pad_new = active.replay.pop_front().unwrap_or(both);
    let events = active.end.step(pad_new, &mut active.blocks);
    // The main loop's two LCG calls per frame (DUEL-END §5.5).
    next_rand31(&mut active.rng);
    next_rand31(&mut active.rng);
    for event in events {
        match &event {
            EndEvent::Music { track, arg } => play.audio.push(roses_game::duel_audio::AudioCue::Music {
                track: u32::from(roses_game::duel_end_screens::seat_music(*track, active.winner, play.hud_own_side())),
                keep: *arg == 1,
            }),
            EndEvent::Sfx { id } => play.audio.push(roses_game::duel_audio::AudioCue::Se {
                slot: 0,
                id: *id,
                skip: false,
            }),
            EndEvent::CameraToLeader { side } => eprintln!(
                "Duel end: unsupported, not shown: camera move to the leader of side {side} (0x1DA360) and the orbit (0x1FC8B0)"
            ),
            EndEvent::ExperienceApplied => {
                eprintln!("Duel end: experience applied to both card blocks")
            }
            EndEvent::SaveDuelEnd { won } => eprintln!(
                "Duel end: save counters (0x227F00, won {won}) are applied by the campaign map"
            ),
            EndEvent::RankList { side, changes } => {
                eprintln!(
                    "Duel end: rank list of side {side}: {} entries",
                    changes.len()
                );
                for c in changes {
                    eprintln!("  card {}: rank {} -> {}", c.card, c.old, c.new);
                }
            }
            EndEvent::Finished(o) => {
                eprintln!("Duel end: finished: {o:?}");
                active.outcome = Some(o.clone());
            }
        }
    }
    let view = active.end.view();
    let icon = ((active.end.frame() / 5) % 3) as u8;
    let key = (
        view.banner,
        view.slide,
        view.darken,
        view.rank_entry.map(|e| e.card),
        icon,
    );
    let mut changed = false;
    if active.last_key != Some(key) {
        active.last_key = Some(key);
        let human = active.human;
        let kind = match view.banner {
            banner::YOU_WIN | banner::YOU_LOSE => Some(BannerKind::Result {
                code: view.banner,
                side: human,
            }),
            // Two pads: 1P WIN / 2P WIN / DRAW with D54 = D50, the winner
            // (2 for a draw: no portrait) [F DUEL-END.md 0].
            banner::P1_WIN | banner::P2_WIN | banner::DRAW => {
                let (code, side) = roses_game::duel_end_screens::seat_result(view.banner, active.winner, play.hud_own_side(), play.spectating());
                Some(BannerKind::Result { code, side })
            }
            banner::PROMOTION | banner::DEMOTION => view.rank_entry.map(|e| BannerKind::Rank {
                demotion: e.demotion,
                side: e.side,
                card: e.card,
            }),
            banner::OFF => None,
            other => {
                let note = format!("banner code {other:#x} (two pads / draw) not drawn");
                if active.reported.insert(note.clone()) {
                    eprintln!("Duel end: unsupported, not drawn: {note}");
                }
                None
            }
        };
        let mut prims = Vec::new();
        if let Some(d) = darkening_primitive(view.darken) {
            prims.push(d);
        }
        let mut picture = None;
        if let Some(kind) = kind {
            let block = &active.blocks[usize::from(human.min(1))];
            match banner_primitives(&mut active.assets, kind, view.slide, icon, block) {
                Ok((p, pic)) => {
                    prims.extend(p);
                    picture = pic;
                }
                Err(e) => {
                    if active.reported.insert(e.clone()) {
                        eprintln!("Duel end: banner not drawn: {e}");
                    }
                }
            }
        }
        let (layer_rgba, notes) = render_banner_layer(&mut active.assets, prims, picture);
        for n in notes {
            if active.reported.insert(n.clone()) {
                eprintln!("Duel end: {n}");
            }
        }
        active.layer = layer_rgba;
        changed = true;
    }
    let mut composite = None;
    if changed || active.hud_version != hud.version {
        active.hud_version = hud.version;
        composite = Some(composite_over(&hud.layer, &active.layer));
    }
    let caption = format!(
        "{} · result state {:#x}{}",
        title,
        view.state,
        match view.state {
            0x5C => " · press X (any face button) to continue",
            0x61 => " · press X for the next rank entry",
            _ => "",
        }
    );
    Some((composite, caption))
}

/// Online: the 2-player result flow ends with 0x5D's pop (back to the
/// Custom Duel menu in the original); an online window has no menu to go
/// back to, so it closes once the result screens have finished.
fn online_end_exit(play: Res<PlayRuntime>, end: Option<Res<EndRuntime>>, mut exit: MessageWriter<AppExit>) {
    if play.play.online.is_none() {
        return;
    }
    if end.is_some_and(|e| e.active.as_ref().is_some_and(|a| a.outcome.is_some())) {
        eprintln!("Online: the result screens finished; closing this window");
        exit.write(AppExit::Success);
    }
}

/// Build the result sequence from the ended duel and the end inputs.
fn start_end(play: &DuelPlay, hud: &mut DuelHud, inputs: EndInputs) -> Result<EndActive, String> {
    let data = &play.data;
    let flags =
        roses_assets::parse_pal_reward_flags(&data.executable).map_err(|e| e.to_string())?;
    let reward = RewardCardTable::from_record_flag_bytes(flags.flag_bytes.iter().copied());
    let thresholds =
        roses_assets::parse_pal_rank_thresholds(&data.executable).map_err(|e| e.to_string())?;
    // S+0x344 bit 7: the side's Destiny Draw was announced (0x27133C); the
    // winner's clears the reels' random slip (0x1DF9C0).
    let duel = &play.session.duel;
    let special_draw_announced = [
        duel.sides[0].destiny & 0x80 != 0,
        duel.sides[1].destiny & 0x80 != 0,
    ];
    let setup = DuelEndSetup::from_duel(
        duel,
        &data.catalogue,
        inputs.map,
        true,
        special_draw_announced,
        reward,
        END_TIMINGS,
    )
    .map_err(|e| e.to_string())?;
    let human = setup.human.iter().position(|h| *h).unwrap_or(0) as u8;
    let winner = setup.winner_code();
    let leaders = [
        data.first_duel.sides[0].leader,
        data.first_duel.sides[1].leader,
    ];
    let assets = EndAssets::new(
        &data.executable,
        &data.messages,
        &data.texsys,
        &data.picmini,
        leaders,
        inputs.duel_end.pictures,
    )?;
    hud.hidden = true;
    Ok(EndActive {
        end: DuelEnd::new(setup, thresholds),
        assets,
        blocks: inputs.blocks,
        rng: inputs.rng,
        replay: inputs.replay,
        human,
        winner,
        last_key: None,
        layer: vec![0; duel_play::HUD_SIZE * duel_play::HUD_SIZE * 4],
        hud_version: 0,
        outcome: None,
        reported: BTreeSet::new(),
    })
}

/// Captures: the one upload of the pre-rendered HUD layer (see
/// `HudLayer::pending`).
fn capture_hud_upload(
    mut layer: ResMut<HudLayer>,
    mut images: ResMut<Assets<Image>>,
    mut presents: ResMut<Assets<PresentMaterial>>,
) {
    if let Some((data, version)) = layer.pending.take() {
        layer.upload(&data, version, &mut images, &mut presents);
    }
}

/// One original update at 25 Hz: pad steps start tracks, then tracks advance,
/// then the overlay pulses and easing run against the new cursor.
fn board_tick(
    mut state: ResMut<BoardState>,
    mut overlay: ResMut<OverlayRuntime>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let steps: Vec<_> = state.pending.drain(..).collect();
    let mut moved = false;
    for (dc, dr) in steps {
        moved |= state.board_camera.step(dc, dr);
    }
    state.board_camera.tick();
    let cursor = state.board_camera.cursor;
    let overlay = &mut *overlay;
    overlay.state.update(&overlay.input, cursor);
    if moved && !state.duel_play {
        let caption = title(&state);
        for mut window in &mut windows {
            window.title = caption.clone();
        }
    }
}

fn to_mat4(m: &camera::Mat4) -> Mat4 {
    Mat4::from_cols_array_2d(&m.map(|column| column.map(|v| v as f32)))
}
/// DIMX rows as matrix columns (the shader indexes [y & 3][x & 3]).
fn dither_mat4(m: &[[i8; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array_2d(&m.map(|row| row.map(f32::from)))
}
fn rgba(c: [u8; 4]) -> Vec4 {
    Vec4::new(
        f32::from(c[0]),
        f32::from(c[1]),
        f32::from(c[2]),
        f32::from(c[3]),
    )
}

/// Antialiased band mesh data for line segments seen from `view`.
fn band_mesh(view: &camera::Mat4, segments: &[[[f64; 3]; 2]]) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut uv: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for segment in segments {
        let (Some(a), Some(b)) = (
            duel_overlay::project(view, segment[0]),
            duel_overlay::project(view, segment[1]),
        ) else {
            continue;
        };
        let base = positions.len() as u32;
        for (position, distance) in duel_overlay::aa_line_band(a, b) {
            positions.push(position);
            uv.push([distance, 0.0]);
        }
        indices.extend([base, base + 1, base + 2, base + 2, base + 1, base + 3]);
    }
    if indices.is_empty() {
        // Every segment behind the near plane: keep a valid, empty-area mesh.
        positions = vec![[0.0; 3]; 3];
        uv = vec![[2.0, 0.0]; 3];
        indices = vec![0, 1, 2];
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(indices))
}
fn quad_mesh(corners: &[[f64; 3]; 4], uv: &[[f32; 2]; 4]) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        corners.map(|c| c.map(|v| v as f32)).to_vec(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv.to_vec())
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
}
/// Sorted-phase order (Bevy draws translucent items by increasing view z;
/// the shaders ignore the transform): board draws first in their original
/// order, then cards, emblems, emblem outlines, cell outlines and fills, as in
/// dump draws 6865-6928 and 6990-6996.
fn order(key: f32) -> Transform {
    Transform::from_xyz(0.0, 0.0, -1000.0 + key)
}
fn board_order(index: &mut u32) -> Transform {
    *index += 1;
    Transform::from_xyz(0.0, 0.0, -100_000.0 + *index as f32)
}

/// The board's draws (cells by terrain, arena, windows) for `input.layout`
/// in the original draw order (0x1AEF30), as `BoardDraw` entities.
fn spawn_board_draws(
    commands: &mut Commands,
    input: &BoardInput,
    view: &camera::Mat4,
    cursor: (u8, u8),
    tint: Vec4,
    (images, meshes, materials): (&mut Assets<Image>, &mut Assets<Mesh>, &mut Assets<BoardMaterial>),
    mut progress: Option<&mut super::RenderProgress>,
) -> Result<(), String> {
    let instances = duel_board::instances(&input.board, &input.layout, cursor)?;
    let mut textures: BTreeMap<(String, bool, bool, bool), Handle<Image>> = BTreeMap::new();
    let mut mesh_cache: BTreeMap<(String, usize), Handle<Mesh>> = BTreeMap::new();
    let mut draw_index = 0u32;
    for instance in &instances {
        let model = &input.board.models[&instance.model];
        for (index, draw) in model.draws.iter().enumerate() {
            let key = (
                draw.texture.clone(),
                draw.clamp_u,
                draw.clamp_v,
                draw.linear,
            );
            let texture = match textures.get(&key) {
                Some(handle) => handle.clone(),
                None => {
                    let address = |clamp| {
                        if clamp {
                            ImageAddressMode::ClampToEdge
                        } else {
                            ImageAddressMode::Repeat
                        }
                    };
                    let filter = if draw.linear {
                        ImageFilterMode::Linear
                    } else {
                        ImageFilterMode::Nearest
                    };
                    let image = Image::from_buffer(
                        &input.board.textures[&draw.texture],
                        ImageType::Extension("png"),
                        CompressedImageFormats::NONE,
                        false,
                        ImageSampler::Descriptor(ImageSamplerDescriptor {
                            address_mode_u: address(draw.clamp_u),
                            address_mode_v: address(draw.clamp_v),
                            mag_filter: filter,
                            min_filter: filter,
                            mipmap_filter: ImageFilterMode::Nearest,
                            lod_min_clamp: 0.,
                            lod_max_clamp: 0.,
                            ..default()
                        }),
                        RenderAssetUsages::default(),
                    )
                    .map_err(|e| format!("Texture decode failed: {e}"))?;
                    let handle = images.add(image);
                    if let Some(p) = progress.as_deref_mut() {
                        p.textures.push(handle.id());
                    }
                    textures.insert(key, handle.clone());
                    handle
                }
            };
            let mesh = mesh_cache
                .entry((model.id.clone(), index))
                .or_insert_with(|| {
                    let handle = meshes.add(
                        Mesh::new(
                            PrimitiveTopology::TriangleList,
                            RenderAssetUsages::default(),
                        )
                        .with_inserted_attribute(
                            Mesh::ATTRIBUTE_POSITION,
                            draw.positions_raw.clone(),
                        )
                        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, draw.uv.clone())
                        .with_inserted_indices(Indices::U32(draw.indices.clone())),
                    );
                    if let Some(p) = progress.as_deref_mut() {
                        p.meshes.push(handle.id());
                    }
                    handle
                })
                .clone();
            let color = duel_board::vertex_color(&draw.material, input.brightness);
            let material = materials.add(BoardMaterial {
                ndc_from_source: to_mat4(&duel_board::ndc_from_source(&view, &instance.world)),
                color_gs: Vec4::from_array(color),
                params: Vec4::new(
                    f32::from(u8::from(draw.textured)),
                    draw.alpha_greater_than.map_or(-1.0, f32::from),
                    f32::from(u8::from(input.gs16)),
                    f32::from(u8::from(draw.texture_alpha)),
                ),
                texture,
                dither_rows: input.dither_board,
                tint: if instance.cell == Some(cursor) {
                    tint
                } else {
                    Vec4::ZERO
                },
            });
            if let Some(p) = progress.as_deref_mut() {
                p.materials.push(material.id().untyped());
            }
            commands.spawn((
                Name::new(format!("{} {} draw {index}", instance.label, model.id)),
                BoardDraw {
                    world: instance.world,
                    cell: instance.cell,
                    material: draw.material,
                    object: board_object(&instance.label),
                },
                Mesh3d(mesh),
                MeshMaterial3d(material),
                board_order(&mut draw_index),
                NoFrustumCulling,
                RenderLayers::layer(0),
            ));
        }
    }
    Ok(())
}

/// A terrain change shown on the board (field magic, effects): the cells'
/// draws are rebuilt for the shown terrain [the original's tile swap is
/// per cell at c502's u 68; H: an instant rebuild].
fn sync_terrain(
    mut commands: Commands,
    runtime: Option<Res<PlayRuntime>>,
    mut input: ResMut<BoardInput>,
    mut state: ResMut<BoardState>,
    draws: Query<Entity, With<BoardDraw>>,
    mut placed: Query<(&mut BoardDraw, Entity)>,
    (mut images, mut meshes, mut materials): (ResMut<Assets<Image>>, ResMut<Assets<Mesh>>, ResMut<Assets<BoardMaterial>>),
) {
    let Some(runtime) = runtime else {
        return;
    };
    let shown = runtime.play.shown_terrain();
    let heights = runtime.play.shown_heights();
    if shown.len() != CELLS {
        return;
    }
    if shown == input.layout.terrain[..] {
        if heights != input.layout.heights {
            // Only the heights moved: re-place the cells' draws.
            input.layout.heights = heights;
            if let Ok(instances) = duel_board::instances(&input.board, &input.layout, state.board_camera.cursor) {
                for (mut draw, _) in &mut placed {
                    if let Some(i) = instances.iter().find(|i| i.cell.is_some() && i.cell == draw.cell) {
                        draw.world = i.world;
                    }
                }
                state.applied_target = None;
            }
        }
        return;
    }
    let Some(view) = state.board_camera.view() else {
        return;
    };
    input.layout.heights = heights;
    input.layout.terrain.copy_from_slice(&shown);
    for e in &draws {
        commands.entity(e).despawn();
    }
    if let Err(e) = spawn_board_draws(
        &mut commands,
        &input,
        &view,
        state.board_camera.cursor,
        Vec4::ZERO,
        (&mut images, &mut meshes, &mut materials),
        None,
    ) {
        eprintln!("Duel board: terrain rebuild failed: {e}");
    }
}

/// The board object of an instance label ("arena n", "window slot n at
/// ..."), cells have none.
fn board_object(label: &str) -> Option<u8> {
    let rest = label.strip_prefix("arena ").or_else(|| label.strip_prefix("window slot "))?;
    rest.split_whitespace().next()?.parse().ok()
}

#[allow(clippy::too_many_arguments)]
fn apply_camera(
    mut state: ResMut<BoardState>,
    draws: Query<(&BoardDraw, &MeshMaterial3d<BoardMaterial>)>,
    world_overlays: Query<&MeshMaterial3d<OverlayMaterial>, With<OverlayWorld>>,
    lines: Query<&OverlayLines>,
    mut materials: ResMut<Assets<BoardMaterial>>,
    mut overlay_materials: ResMut<Assets<OverlayMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    (runtime, input, mut dimmed): (Option<Res<PlayRuntime>>, Option<Res<BoardInput>>, Local<Option<f32>>),
) {
    // The Exodia event's 0x26E6B0: objects 1, 5 and 6's light +0x124 from
    // 0.8 down by 0.08 per update and back (colour min(max, k g ambient)
    // with k in place of 0.8 [F 0x26E6B0; H: which lane +0x124 is]).
    use roses_game::duel_exodia::{DIMMED_OBJECTS, DIM_RESTORED};
    let k = runtime.as_deref().and_then(|r| r.play.exodia.as_ref()).map_or(DIM_RESTORED, |e| e.event.backdrop);
    if let (Some(input), true) = (input.as_deref(), *dimmed != Some(k)) {
        for (draw, handle) in &draws {
            if draw.object.is_some_and(|o| DIMMED_OBJECTS.contains(&o)) {
                if let Some(mut material) = materials.get_mut(&handle.0) {
                    let mut color = duel_board::vertex_color(&draw.material, input.brightness * k / DIM_RESTORED);
                    color[3] = material.color_gs.w;
                    material.color_gs = Vec4::from_array(color);
                }
            }
        }
        *dimmed = Some(k);
    }
    let [x, y, z] = state.board_camera.target;
    let [ex, ey, ez] = state.board_camera.eye();
    let target = [x, y, z, ex, ey, ez, state.board_camera.zoom];
    if state.applied_target == Some(target) {
        return;
    }
    let Some(view) = state.board_camera.view() else {
        return;
    };
    for (draw, handle) in &draws {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.ndc_from_source = to_mat4(&duel_board::ndc_from_source(&view, &draw.world));
        }
    }
    let identity = to_mat4(&duel_board::ndc_from_source(&view, &camera::identity()));
    for handle in &world_overlays {
        if let Some(mut material) = overlay_materials.get_mut(&handle.0) {
            material.ndc_from_source = identity;
        }
    }
    for line in &lines {
        if let Some(mut mesh) = meshes.get_mut(&line.mesh) {
            *mesh = band_mesh(&view, &line.segments);
        }
    }
    state.applied_target = Some(target);
}

/// Push the current overlay colours, visibility and cursor tint to materials.
fn apply_overlay(
    overlay: Res<OverlayRuntime>,
    state: Res<BoardState>,
    draws: Query<(&BoardDraw, &MeshMaterial3d<BoardMaterial>)>,
    mut board_materials: ResMut<Assets<BoardMaterial>>,
    mut overlay_materials: ResMut<Assets<OverlayMaterial>>,
    mut visibility: Query<&mut Visibility>,
    hud_layer: Res<HudLayer>,
    mut presents: ResMut<Assets<PresentMaterial>>,
) {
    if !overlay.enabled {
        return;
    }
    let brightness = overlay.input.brightness;
    let mut set = |entity: Entity, handle: &Handle<OverlayMaterial>, color: [u8; 4]| {
        let value = rgba(color);
        if overlay_materials
            .get(handle)
            .is_some_and(|m| m.color_gs != value)
        {
            if let Some(mut material) = overlay_materials.get_mut(handle) {
                material.color_gs = value;
            }
        }
        if let Ok(mut v) = visibility.get_mut(entity) {
            let want = if color[3] > 0 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *v != want {
                *v = want;
            }
        }
    };
    for (index, (entity, handle)) in overlay.fills.iter().enumerate() {
        set(*entity, handle, overlay.state.fill_drawn(index, brightness));
    }
    for (index, (entity, handle)) in overlay.outlines.iter().enumerate() {
        set(*entity, handle, overlay.state.cells[index].outline);
    }
    let cursor = state.board_camera.cursor;
    let tint = overlay.tint(cursor);
    let _ = brightness;
    if let Some(handle) = &hud_layer.present {
        if presents.get(handle).is_some_and(|m| m.unit_tint != tint) {
            if let Some(mut present) = presents.get_mut(handle) {
                present.unit_tint = tint;
            }
        }
    }
    for (draw, handle) in &draws {
        let want = if draw.cell == Some(cursor) {
            tint
        } else {
            Vec4::ZERO
        };
        if board_materials
            .get(&handle.0)
            .is_some_and(|m| m.tint != want)
        {
            if let Some(mut material) = board_materials.get_mut(&handle.0) {
                material.tint = want;
            }
        }
    }
}

/// (Re)build the card and emblem layers when the unit list changes.
#[allow(clippy::too_many_arguments)]
fn sync_units(
    mut commands: Commands,
    mut overlay: ResMut<OverlayRuntime>,
    input: Res<BoardInput>,
    state: Res<BoardState>,
    old: Query<Entity, With<UnitLayer>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<OverlayMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut runtime: Option<ResMut<PlayRuntime>>,
) {
    if !overlay.enabled || overlay.built_units.as_ref() == Some(&overlay.input.units) {
        return;
    }
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let Some(view) = state.board_camera.view() else {
        return;
    };
    let overlay = &mut *overlay;
    spawn_units(
        &mut commands,
        overlay,
        &input,
        &view,
        &mut meshes,
        &mut materials,
        &mut images,
        runtime.as_deref_mut().map(|r| &mut r.hud),
        None,
    );
}

/// The card-versus screen: while the battle scene's cards show, their two
/// quads are rebuilt every update on the versus canvas and the present
/// pass shows that canvas alone (the original clears to black and draws no
/// board or HUD from the fly-in on [emu dumps-battle-frames 118-208]).
#[allow(clippy::too_many_arguments)]
fn sync_versus(
    mut commands: Commands,
    mut layer: ResMut<VersusLayer>,
    state: Res<BoardState>,
    input: Res<BoardInput>,
    old: Query<Entity, With<VersusDraw>>,
    mut runtime: Option<ResMut<PlayRuntime>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<OverlayMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut presents: ResMut<Assets<PresentMaterial>>,
    (mut arena, mut map_materials, mut arena_visibility, mut fx_materials, blur): (
        ResMut<ArenaRender>,
        ResMut<Assets<MapMaterial>>,
        Query<&mut Visibility, With<ArenaDraw>>,
        ResMut<Assets<FxMaterial>>,
        Option<Res<BlurLayer>>,
    ),
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let Some(runtime) = runtime.as_deref_mut() else {
        return;
    };
    if let (Some(m), Some(view)) = (runtime.play.monster_fx.clone(), state.board_camera.view()) {
        build_monster_fx(
            &mut commands,
            &mut layer,
            &m,
            &view,
            (state.board_camera.eye(), state.board_camera.target),
            &input,
            &mut runtime.hud,
            &mut meshes,
            &mut materials,
            &mut images,
            None,
            (runtime.play.data.texeff.as_deref(), &mut fx_materials),
        );
    }
    if let (Some(a), Some(view)) = (runtime.play.activation.clone(), state.board_camera.view()) {
        build_activation(
            &mut commands,
            &mut layer,
            &a,
            &view,
            (state.board_camera.polar, state.board_camera.eye(), state.board_camera.target),
            &input,
            &mut runtime.hud,
            &mut meshes,
            &mut materials,
            &mut images,
            None,
            (runtime.play.data.texeff.as_deref(), &mut fx_materials),
        );
    }
    build_extra_fx(
        &mut commands,
        &mut layer,
        &runtime.play,
        (state.board_camera.eye(), state.board_camera.target),
        &input,
        &mut meshes,
        &mut images,
        (runtime.play.data.texeff.as_deref(), &mut fx_materials),
    );
    if let Some(view) = state.board_camera.view() {
        build_knockout_fx(
            &mut commands,
            &mut layer,
            &runtime.play,
            &view,
            (state.board_camera.eye(), state.board_camera.target),
            &input,
            &mut runtime.hud,
            (&mut *meshes, &mut *materials, &mut *images),
            None,
            (runtime.play.data.texeff.as_deref(), &mut fx_materials),
        );
    }
    if let (Some(flight), Some(view)) = (runtime.play.summon_flight.clone(), state.board_camera.view()) {
        build_summon_flight(
            &mut commands,
            &mut layer,
            &flight,
            (&view, state.board_camera.polar),
            &input,
            &mut runtime.hud,
            &mut meshes,
            &mut materials,
            &mut images,
            None,
            (runtime.play.data.texeff.as_deref(), &mut fx_materials),
        );
    }
    let versus = runtime.play.intro.as_ref().and_then(|i| i.versus.clone());
    // c605 draws over the board: the versus canvas is shown alone before it.
    let showing = versus.as_ref().is_some_and(|v| v.end.is_none());
    if showing != layer.showing {
        layer.showing = showing;
        if let Some(present) = layer.present.clone() {
            if let Some(mut present) = presents.get_mut(&present) {
                present.params.w = f32::from(u8::from(showing));
            }
        }
    }
    let Some(versus) = versus else {
        build_arena(
            &mut commands,
            &mut arena,
            None,
            &mut meshes,
            &mut images,
            &mut map_materials,
            Some(&mut arena_visibility),
            None,
        );
        return;
    };
    let Some(view) = state.board_camera.view() else {
        return;
    };
    build_versus(
        &mut commands,
        &mut layer,
        &versus,
        &view,
        &input,
        &mut runtime.hud,
        runtime.play.data.texeff.as_deref(),
        &mut meshes,
        &mut materials,
        &mut images,
        None,
    );
    build_arena(
        &mut commands,
        &mut arena,
        versus.arena.as_ref(),
        &mut meshes,
        &mut images,
        &mut map_materials,
        Some(&mut arena_visibility),
        None,
    );
    if let Some(battle) = versus.arena.as_ref().and_then(|a| a.battle.as_ref()) {
        build_unit_fade(&mut commands, &layer, battle, versus.arena.as_ref().and_then(|a| a.effect.as_ref()), &input, &mut meshes, &mut materials);
        build_unit_colour_pass(&mut commands, &mut layer, battle, versus.arena.as_ref().and_then(|a| a.effect.as_ref()), &input, &mut meshes, &mut fx_materials, &mut images);
        build_destroy_effects(
            &mut commands,
            &mut layer,
            battle,
            runtime.play.data.texeff.as_deref(),
            &input,
            &mut meshes,
            &mut fx_materials,
            &mut images,
        );
    }
    if let Some(draw) = versus.arena.as_ref().and_then(|a| a.effect.as_ref()) {
        // The previous frame's output (the motion-blur history written by
        // the last rendered frame) stands in for the GS frame buffer the
        // effect samples [H: the other of the two display buffers].
        let history = blur.as_deref().filter(|b| b.images.len() == 2 && b.camera.is_some()).map(|b| b.images[1 - b.write].clone());
        build_attack_effect(&mut commands, &mut layer, draw, history, runtime.play.data.texeff.as_deref(), &input, &mut meshes, &mut fx_materials, &mut images);
    }
    if let Some(shot) = &versus.arena {
        build_arena_black(&mut commands, shot, &input, &mut meshes, &mut materials, &mut images, &mut layer);
    }
    if let Some(ret) = &versus.ret {
        build_return_effects(
            &mut commands,
            &mut layer,
            ret,
            runtime.play.data.texeff.as_deref(),
            &input,
            &mut meshes,
            &mut materials,
            &mut images,
        );
    }
    if let Some(damage) = versus.ret.as_ref().and_then(|r| r.number) {
        let texeff = runtime.play.data.texeff.as_deref();
        build_damage_number(
            &mut commands,
            &mut layer,
            &damage,
            texeff,
            &input,
            &mut meshes,
            &mut materials,
            &mut images,
        );
    }
}

/// c621 (the destroy effect) on the versus canvas over the arena.
#[allow(clippy::too_many_arguments)]
fn build_destroy_effects(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    battle: &roses_game::duel_arena_battle::ArenaBattle,
    texeff: Option<&[u8]>,
    input: &BoardInput,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<FxMaterial>,
    images: &mut Assets<Image>,
) {
    let camera = battle.camera.view();
    for (n, effect) in battle.destroy_effects().enumerate() {
        let tris = effect.draw(&camera);
        spawn_fx(commands, layer, &tris, texeff, input, meshes, materials, images, VERSUS_LAYER, 500.0 + n as f32 * 4.0, "c621");
    }
}

/// The attack effect's GS primitives (BMODULE run live, `duel_bmodule`) on
/// the versus canvas over the arena units: one mesh per draw state in the
/// order of first use (`duel_bmodule::fx_batches`), TEXEFF or the attacker's
/// own effect textures by the shim's TEX0, the GS blend from ALPHA.
/// Frame-buffer textures sample the motion-blur history (the previous frame;
/// none in a single-frame capture: reported), unit targets the role's
/// offscreen unit canvas, frame copies (0x207FC0) the history too; Z test
/// GEQUAL against the units and stage. Textures are sampled as the GS does
/// (`gs_tex`): CLAMP modes, TEX1 nearest / linear by LOD, S T Q per pixel.
/// Points: none in the 683 programs (sweep 3).
#[allow(clippy::too_many_arguments)]
fn build_attack_effect(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    draw: &roses_game::duel_bmodule::EffectDraw,
    history: Option<Handle<Image>>,
    texeff: Option<&[u8]>,
    input: &BoardInput,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<FxMaterial>,
    images: &mut Assets<Image>,
) {
    build_effect_prims(commands, layer, draw, history, texeff, input, meshes, materials, images, (VERSUS_LAYER, true));
}

/// [`build_attack_effect`] onto `canvas` (the versus canvas, or the board's
/// 0 for the duel opening's c620), with the Z test against the units and
/// stage when `z` (the board canvas has none for the effects).
#[allow(clippy::too_many_arguments)]
fn build_effect_prims(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    draw: &roses_game::duel_bmodule::EffectDraw,
    history: Option<Handle<Image>>,
    texeff: Option<&[u8]>,
    input: &BoardInput,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<FxMaterial>,
    images: &mut Assets<Image>,
    (canvas, z): (usize, bool),
) {
    use roses_game::duel_bmodule;
    use roses_fx::types::TextureKey;
    // Textures first (sizes are needed for the UVs).
    for p in draw.prims.iter() {
        let Some(key) = p.texture.filter(|k| matches!(k, TextureKey::TexEff(_) | TextureKey::Own(_))) else { continue };
        layer.bfx.entry((draw.card, key)).or_insert_with(|| {
            let image = match key {
                TextureKey::TexEff(k) => texeff.and_then(|t| {
                    roses_assets::decode_pal_opening_indexed_texture(t, roses_assets::OpeningArchiveKind::TexEff, usize::from(k))
                        .ok()
                        .and_then(|t| duel_bmodule::indexed_rgba(&t))
                }),
                TextureKey::Own(k) => draw.own.get(usize::from(k)).cloned().flatten(),
                TextureKey::Frame | TextureKey::UnitRt(_) => None,
            }?;
            let size = (image.width, image.height);
            let mut texture = Image::new(
                Extent3d { width: image.width, height: image.height, depth_or_array_layers: 1 },
                TextureDimension::D2,
                image.pixels,
                TextureFormat::Rgba8Unorm,
                RenderAssetUsages::default(),
            );
            texture.sampler = canvas_sampler(ImageFilterMode::Linear);
            Some((images.add(texture), size))
        });
    }
    let sizes: std::collections::HashMap<TextureKey, (f32, f32)> = layer
        .bfx
        .iter()
        .filter(|((card, _), _)| *card == draw.card)
        .filter_map(|((_, k), v)| v.as_ref().map(|(_, s)| (*k, (s.0 as f32, s.1 as f32))))
        .collect();
    let (batches, report) = duel_bmodule::fx_batches(&draw.prims, &|state, _| {
        if matches!(state.texture, Some(TextureKey::Frame | TextureKey::UnitRt(_))) {
            return Some((512.0, 512.0));
        }
        state.texture.and_then(|k| sizes.get(&k).copied())
    });
    if std::env::var_os("ROSES_BMODULE_LOG").is_some() {
        eprintln!(
            "bmodule draw: {} prims, {} batches: {:?}",
            draw.prims.len(),
            batches.len(),
            batches.iter().map(|b| (b.state, b.positions.len(), b.positions.first().copied(), b.colours.first().copied())).collect::<Vec<_>>()
        );
    }
    let mut notes = Vec::new();
    if !report.blends.is_empty() {
        notes.push(format!("ALPHA {:x?} drawn additively", report.blends.keys().collect::<Vec<_>>()));
    }
    if !report.textures.is_empty() {
        notes.push(format!("textures at TBP {:x?}", report.textures.keys().collect::<Vec<_>>()));
    }
    if report.points > 0 {
        notes.push("points".into());
    }
    let o = 1792.0f32;
    // ROSES_BMODULE_NO_Z=1 (research): drawn over everything as before.
    let z_test = z && std::env::var_os("ROSES_BMODULE_NO_Z").is_none();
    let mut first_copy: Option<usize> = None;
    for (i, b) in batches.iter().enumerate() {
        let Some(state) = b.state else { continue };
        // From the first sampler of a frame copy on, the draws are not part
        // of the copy (FX_POST_LAYER) and sample the copy image.
        if state.copy && first_copy.is_none() {
            first_copy = Some(i);
        }
        let texture = match state.texture {
            Some(TextureKey::Frame) if state.copy => match layer.copy.clone() {
                Some(h) => {
                    layer.copy_wanted = true;
                    h
                }
                None => {
                    notes.push("frame copies (no copy image)".into());
                    continue;
                }
            },
            Some(TextureKey::Frame) => match &history {
                Some(h) => h.clone(),
                None => {
                    notes.push("frame-buffer textures (no previous frame in this run)".into());
                    continue;
                }
            },
            Some(TextureKey::UnitRt(role)) => match layer.unit_rt[usize::from(role & 1)].clone() {
                Some(h) => h,
                None => {
                    notes.push("unit targets".into());
                    continue;
                }
            },
            None => fx_white(layer, images),
            Some(k) => match layer.bfx.get(&(draw.card, k)).cloned().flatten() {
                Some((h, _)) => h,
                None => {
                    notes.push(format!("texture {k:?}"));
                    continue;
                }
            },
        };
        let positions: Vec<[f32; 3]> = b.positions.iter().zip(&b.depths).map(|(p, d)| [p[0] + o, p[1] + o, *d]).collect();
        let colours: Vec<[f32; 4]> = b.colours.iter().map(|c| c.map(f32::from)).collect();
        let mesh = meshes.add(
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, b.uvs.clone())
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, b.qs.clone()),
        );
        // The image's size in GS texels (its UVs are normalised to it).
        let image_size = match state.texture {
            Some(TextureKey::Frame | TextureKey::UnitRt(_)) => (512.0, 512.0),
            Some(k) => sizes.get(&k).copied().unwrap_or((1.0, 1.0)),
            None => (1.0, 1.0),
        };
        let material = materials.add(FxMaterial {
            ndc_from_source: to_mat4(&duel_board::ndc_from_gs_window()),
            params: Vec4::new(
                f32::from(u8::from(state.texture.is_some())),
                f32::from(u8::from(input.gs16)),
                f32::from(u8::from(matches!(state.blend, 3 | 4))),
                f32::from(u8::from(z_test)),
            ),
            dither_rows: input.dither_overlay,
            texture,
            gs: Vec4::from_array(state.gs),
            wrap: Vec4::from_array(state.wrap),
            region: Vec4::from_array(state.region),
            lod: Vec4::from_array(state.lod),
            size: Vec4::new(image_size.0, image_size.1, f32::from(u8::from(state.frame_word)), 0.0),
            blend: state.blend,
            z_test,
            gs_tex: true,
        });
        commands.spawn((
            Name::new(format!("attack effect {i}")),
            VersusDraw,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            order(450.0 + i as f32 * 0.01),
            NoFrustumCulling,
            // Into a unit's offscreen target: drawn by that target's camera
            // after the unit (the FX pass), the target's alpha kept.
            RenderLayers::layer(match state.target {
                Some(role) if state.target_pre => duel_units_render::UNIT_RT_PRE_LAYERS[usize::from(role & 1)],
                Some(role) => duel_units_render::UNIT_RT_LAYERS[usize::from(role & 1)],
                None if canvas == VERSUS_LAYER && (first_copy.is_some() || state.post_copy) => FX_POST_LAYER,
                None => canvas,
            }),
        ));
    }
    for n in notes {
        let line = format!("Duel arena: unsupported, not drawn exactly: card {}'s attack effect {n}", draw.card);
        if layer.bfx_reported.insert(line.clone()) {
            eprintln!("{line}");
        }
    }
}

/// Effect triangles (`FxTri`, additive, per-vertex colour) as one mesh per
/// texture: untextured, TEXEFF 15, 12, 8 and 11.
#[allow(clippy::too_many_arguments)]
fn spawn_fx(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    tris: &[roses_game::duel_c621::FxTri],
    texeff: Option<&[u8]>,
    input: &BoardInput,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<FxMaterial>,
    images: &mut Assets<Image>,
    render_layer: usize,
    key: f32,
    name: &str,
) {
    spawn_fx_blend(commands, layer, tris, texeff, input, meshes, materials, images, (render_layer, key, name, 0));
}

/// [`spawn_fx`] with the blend: 0 ALPHA 0x48 (add), 1 0x42 (Cd - Cs * As),
/// 2 0x88 (Cs * As, replacing).
#[allow(clippy::too_many_arguments)]
fn spawn_fx_blend(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    tris: &[roses_game::duel_c621::FxTri],
    texeff: Option<&[u8]>,
    input: &BoardInput,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<FxMaterial>,
    images: &mut Assets<Image>,
    (render_layer, key, name, blend): (usize, f32, &str, u8),
) {
    let o = 1792.0f32;
    for (i, (textured, sheet)) in [(false, 0u8), (true, 15), (true, 12), (true, 8), (true, 11), (true, 9), (true, 3), (true, 14), (true, 0), (true, 1), (true, 7), (true, 4)].into_iter().enumerate() {
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut colours = Vec::new();
        for t in tris.iter().filter(|t| t.textured == textured && (!textured || t.sheet == sheet)) {
            for k in 0..3 {
                positions.push([t.screen[k][0] + o, t.screen[k][1] + o, 0.0]);
                uvs.push(t.uv[k]);
                colours.push(t.colour[k].map(f32::from));
            }
        }
        if positions.is_empty() {
            continue;
        }
        let texture = if !textured {
            fx_white(layer, images)
        } else if sheet == 15 {
            match texeff_digits(layer, texeff, images) {
                Some(t) => t,
                None => continue,
            }
        } else {
            match fx_sheet(layer, texeff, images, sheet) {
                Some(t) => t,
                None => continue,
            }
        };
        let mesh = meshes.add(
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours),
        );
        let material = materials.add(FxMaterial {
            ndc_from_source: to_mat4(&duel_board::ndc_from_gs_window()),
            params: Vec4::new(f32::from(u8::from(textured)), f32::from(u8::from(input.gs16)), f32::from(u8::from(matches!(blend, 3 | 4))), 0.0),
            dither_rows: input.dither_overlay,
            texture,
            gs: Vec4::new(0.0, 1.0, 0.0, 0.0),
            wrap: Vec4::ZERO,
            region: Vec4::ZERO,
            lod: Vec4::ZERO,
            size: Vec4::ZERO,
            blend,
            z_test: false,
            gs_tex: false,
        });
        commands.spawn((
            Name::new(format!("{name} fx {i}")),
            VersusDraw,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            order(key + i as f32 * 0.5),
            NoFrustumCulling,
            RenderLayers::layer(render_layer),
        ));
    }
}

/// A TEXEFF record other than 15 for the effects (alpha as the digits').
fn fx_sheet(layer: &mut VersusLayer, texeff: Option<&[u8]>, images: &mut Assets<Image>, record: u8) -> Option<Handle<Image>> {
    if let Some(h) = layer.sheets.get(&record) {
        return h.clone();
    }
    let handle = versus_texeff(texeff, usize::from(record)).map(|image| {
        let mut pixels = image.pixels;
        for p in pixels.chunks_exact_mut(4) {
            p[3] = if p[3] >= 128 { 255 } else { 0 };
        }
        let mut texture = Image::new(
            Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        );
        texture.sampler = canvas_sampler(ImageFilterMode::Nearest);
        images.add(texture)
    });
    layer.sheets.insert(record, handle.clone());
    handle
}

/// The fading arena unit's composite: a full-buffer sprite of the fade
/// canvas, colour (128, 128, 128, alpha), ALPHA 0x44 [F C621-DESTROY.md 7].
fn build_unit_fade(
    commands: &mut Commands,
    layer: &VersusLayer,
    battle: &roses_game::duel_arena_battle::ArenaBattle,
    effect: Option<&roses_game::duel_bmodule::EffectDraw>,
    input: &BoardInput,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<OverlayMaterial>,
) {
    for (i, unit) in battle.units.iter().enumerate() {
        let Some(canvas) = (if i == 0 { layer.fade.clone() } else { layer.fade1.clone() }) else {
            continue;
        };
        let pose = duel_units_render::ArenaPose {
            card: 0,
            position: [0.0; 3],
            yaw: 0,
            clock: None,
            blend: None,
            light: None,
            alpha: unit.alpha,
            rt: false,
            rt_ndc: None,
            writes: effect.map(|e| e.unit_writes[i]),
            stage_light: 128,
        };
        let alpha = pose.shown_alpha();
        // Composites with another ALPHA (obj+0x1F8 bits 0-1) are FX draws
        // (`build_unit_colour_pass`).
        if alpha == 0 || !pose.composited() || pose.composite_mode() != 0 {
            continue;
        }
        let rgb = pose.tint().unwrap_or([128; 3]).map(f32::from);
        let (a, b) = (1792.0f32, 1792.0 + 512.0);
        let mesh = meshes.add(
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[a, a, 0.0], [a, b, 0.0], [b, a, 0.0], [b, b, 0.0]])
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]])
                .with_inserted_indices(Indices::U32(vec![0, 1, 2, 1, 3, 2])),
        );
        let material = materials.add(overlay_material(
            KIND_QUAD,
            false,
            to_mat4(&duel_board::ndc_from_gs_window()),
            Vec4::new(rgb[0], rgb[1], rgb[2], f32::from(alpha)),
            input,
            canvas.clone(),
        ));
        commands.spawn((
            Name::new("arena unit fade"),
            VersusDraw,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            order(-400.0),
            NoFrustumCulling,
            RenderLayers::layer(VERSUS_LAYER),
        ));
    }
}

/// 0x208450's table (0x2D0FA0: ALPHA 0x44, 0x48, 0x42, 0x84, FIX 0x80) as
/// [`FxMaterial`] blends.
fn table_blend(index: u8) -> u8 {
    match index & 3 {
        0 => 3,
        1 => 0,
        2 => 1,
        _ => 4,
    }
}

/// The attack effects' unit object writes beyond the multiplier [F 0x1EFD08
/// .. 0x1F0860; emu the project notes emu-pass]: the full-window colour pass
/// over the unit in its offscreen target (an untextured sprite after the
/// model, RGB blended by 0x208450(obj+0x1F8 bits 2-3), the target's alpha
/// kept: FBMSK 0xFF000000), and the composite of that target with an ALPHA
/// other than 0x44 (obj+0x1F8 bits 0-1: full-window sprite of the target,
/// colour (+0x12E, +0x12D, +0x12C, alpha), TCC 1, alpha-tested).
#[allow(clippy::too_many_arguments)]
fn build_unit_colour_pass(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    battle: &roses_game::duel_arena_battle::ArenaBattle,
    effect: Option<&roses_game::duel_bmodule::EffectDraw>,
    input: &BoardInput,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<FxMaterial>,
    images: &mut Assets<Image>,
) {
    use roses_game::duel_c621::FxTri;
    let Some(effect) = effect else { return };
    for (i, unit) in battle.units.iter().enumerate() {
        let Some(canvas) = (if i == 0 { layer.fade.clone() } else { layer.fade1.clone() }) else {
            continue;
        };
        let pose = duel_units_render::ArenaPose {
            card: 0,
            position: [0.0; 3],
            yaw: 0,
            clock: None,
            blend: None,
            light: None,
            alpha: unit.alpha,
            rt: false,
            rt_ndc: None,
            writes: Some(effect.unit_writes[i]),
            stage_light: 128,
        };
        let alpha = pose.shown_alpha();
        if alpha == 0 {
            continue;
        }
        if let Some((rgba, mode)) = pose.colour_pass() {
            let tri = |screen: [[f32; 2]; 3]| FxTri { screen, uv: [[0.0; 2]; 3], colour: [rgba; 3], textured: false, sheet: 0 };
            let tris = [tri([[0.0, 0.0], [512.0, 0.0], [0.0, 512.0]]), tri([[512.0, 0.0], [512.0, 512.0], [0.0, 512.0]])];
            spawn_fx_blend(commands, layer, &tris, None, input, meshes, materials, images, (duel_units_render::FADE_LAYERS[i], 440.0, "unit colour pass", table_blend(mode)));
        }
        let mode = pose.composite_mode();
        if mode == 0 {
            continue;
        }
        let rgb = pose.tint().unwrap_or([128; 3]).map(f32::from);
        let (a, b) = (1792.0f32, 1792.0 + 512.0);
        let colour = [rgb[0], rgb[1], rgb[2], f32::from(alpha)];
        let mesh = meshes.add(
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[a, a, 0.0], [a, b, 0.0], [b, a, 0.0], [b, b, 0.0]])
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]])
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![colour; 4])
                .with_inserted_indices(Indices::U32(vec![0, 1, 2, 1, 3, 2])),
        );
        let blend = table_blend(mode);
        let material = materials.add(FxMaterial {
            ndc_from_source: to_mat4(&duel_board::ndc_from_gs_window()),
            params: Vec4::new(1.0, f32::from(u8::from(input.gs16)), f32::from(u8::from(matches!(blend, 3 | 4))), 0.0),
            dither_rows: input.dither_overlay,
            texture: canvas.clone(),
            gs: Vec4::new(0.0, 1.0, 0.0, 128.0),
            wrap: Vec4::ZERO,
            region: Vec4::ZERO,
            lod: Vec4::ZERO,
            size: Vec4::ZERO,
            blend,
            z_test: false,
            gs_tex: false,
        });
        commands.spawn((
            Name::new("arena unit composite"),
            VersusDraw,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            order(-400.0),
            NoFrustumCulling,
            RenderLayers::layer(VERSUS_LAYER),
        ));
    }
}

/// The 1x1 white texture of the versus layer.
fn fx_white(layer: &mut VersusLayer, images: &mut Assets<Image>) -> Handle<Image> {
    layer
        .white
        .get_or_insert_with(|| {
            images.add(Image::new_fill(
                Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[255, 255, 255, 255],
                TextureFormat::Rgba8Unorm,
                RenderAssetUsages::default(),
            ))
        })
        .clone()
}

/// The arena stage for `shot`: spawned once per terrain on the versus
/// canvas, its draws' clip matrices set from the StageCam camera each
/// update; hidden when no arena shows.
#[allow(clippy::too_many_arguments)]
fn build_arena(
    commands: &mut Commands,
    arena: &mut ArenaRender,
    shot: Option<&roses_game::duel_versus::ArenaShot>,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<MapMaterial>,
    mut visibility: Option<&mut Query<&mut Visibility, With<ArenaDraw>>>,
    mut progress: Option<&mut super::RenderProgress>,
) {
    use roses_game::duel_arena;
    let Some(shot) = shot else {
        for (entity, _, _) in &arena.draws {
            if let Some(Ok(mut v)) = visibility.as_deref_mut().map(|q| q.get_mut(*entity)) {
                *v = Visibility::Hidden;
            }
        }
        return;
    };
    let Some(loaded) = arena.loaded.as_ref() else {
        if arena.reported.insert("arena".into()) {
            eprintln!("Duel board: unsupported, not drawn: the battle arena (no arena import: --duel-arena / import-duel-arena)");
        }
        return;
    };
    let terrain = u32::from(shot.terrain);
    let Some(cmot) = loaded.cameras.get(&terrain) else {
        if arena.reported.insert(format!("camera {terrain}")) {
            eprintln!("Duel board: unsupported, not drawn: the battle arena of terrain {terrain} (no StageCam path)");
        }
        return;
    };
    let cam = shot.camera(cmot);
    if arena.built != Some(shot.terrain) {
        for (entity, _, _) in arena.draws.drain(..) {
            commands.entity(entity).despawn();
        }
        let unsupported = loaded.unsupported.get(&terrain).copied().unwrap_or(0);
        if unsupported > 0 && arena.reported.insert(format!("records {terrain}")) {
            eprintln!("Duel board: unsupported, not drawn: {unsupported} stage record(s) of terrain {terrain} (RH0 grammar not covered)");
        }
        let models: Vec<&roses_game::campaign_map::MapModel> = loaded
            .shared
            .get(&terrain)
            .into_iter()
            .flatten()
            .chain(loaded.stages.get(&terrain).into_iter().flatten())
            .collect();
        let mut index = 0u32;
        for model in models {
            let world = duel_arena::stage_world(model);
            let ndc = duel_arena::ndc(&cam, &world).unwrap_or(duel_board::GS_PROJECTION);
            for draw in &model.draws {
                let texture = match &draw.texture {
                    Some(path) => {
                        let key = (path.clone(), draw.clamp_u, draw.clamp_v, draw.linear);
                        if let Some(h) = arena.textures.get(&key) {
                            h.clone()
                        } else {
                            let Some(bytes) = loaded.textures.get(path) else {
                                continue;
                            };
                            let Ok(image) = crate::campaign_map_render::load_png(bytes, draw.clamp_u, draw.clamp_v, draw.linear) else {
                                continue;
                            };
                            let h = images.add(image);
                            if let Some(p) = progress.as_deref_mut() {
                                p.textures.push(h.id());
                            }
                            arena.textures.insert(key, h.clone());
                            h
                        }
                    }
                    None => {
                        let key = (String::new(), false, false, false);
                        arena
                            .textures
                            .entry(key)
                            .or_insert_with(|| {
                                images.add(Image::new_fill(
                                    Extent3d {
                                        width: 1,
                                        height: 1,
                                        depth_or_array_layers: 1,
                                    },
                                    TextureDimension::D2,
                                    &[255, 255, 255, 255],
                                    TextureFormat::Rgba8Unorm,
                                    RenderAssetUsages::default(),
                                ))
                            })
                            .clone()
                    }
                };
                if std::env::var_os("ROSES_ARENA_LOG").is_some() {
                    let m = to_mat4(&ndc);
                    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                    for p in &draw.positions_raw {
                        let c = m * Vec4::new(p[0], p[1], p[2], 1.0);
                        for (k, v) in [c.x / c.w * 256.0 + 2047.5 - 1792.0, 2047.5 - c.y / c.w * 256.0 - 1792.0, c.w].into_iter().enumerate() {
                            lo[k] = lo[k].min(v);
                            hi[k] = hi[k].max(v);
                        }
                    }
                    eprintln!("arena stage slot {} draw {} verts {} x {:.0}..{:.0} y {:.0}..{:.0} w {:.1}..{:.1}", model.slot, draw.draw_index, draw.positions_raw.len(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
                }
                let mesh = meshes.add(crate::campaign_map_render::draw_mesh(draw));
                let mut stage_material = MapMaterial::for_draw(draw, texture, to_mat4(&ndc));
                stage_material.set_z_masked(!matches!(draw.blend, roses_game::campaign_map::Blend::Opaque));
                // Height fog on the terrain's stage objects: 0x1F4480 sets
                // the fog bit on objects 8.. when the terrain is not 4; terrain
                // 4 turns it on for object 9, terrain 2 off for object 9 [F
                // ARENA-ENVIRONMENT.md 1]; the shared object 7 keeps it off [H].
                let slot = model.slot;
                let fogged = slot >= 8 && (terrain != 4 || slot == 9) && !(terrain == 2 && slot == 9);
                if let (true, Some(light)) = (fogged, loaded.lights.get(&terrain)) {
                    let (fog, colour) = light.fog_uniforms();
                    let y = [world[0][1] as f32, world[1][1] as f32, world[2][1] as f32, world[3][1] as f32];
                    stage_material.set_fog(fog, colour, y);
                }
                let material = materials.add(stage_material);
                if let Some(p) = progress.as_deref_mut() {
                    p.meshes.push(mesh.id());
                    p.materials.push(material.id().untyped());
                }
                index += 1;
                let entity = commands
                    .spawn((
                        Name::new(format!("arena slot {} draw", model.slot)),
                        ArenaDraw,
                        Mesh3d(mesh),
                        MeshMaterial3d(material.clone()),
                        Transform::from_xyz(0.0, 0.0, -100_000.0 + index as f32),
                        NoFrustumCulling,
                        RenderLayers::layer(VERSUS_LAYER),
                    ))
                    .id();
                arena.draws.push((entity, world, material));
            }
        }
        arena.built = Some(shot.terrain);
    }
    for (entity, world, material) in &arena.draws {
        if let Some(Ok(mut v)) = visibility.as_deref_mut().map(|q| q.get_mut(*entity)) {
            if *v != Visibility::Inherited {
                *v = Visibility::Inherited;
            }
        }
        // An attack effect's stage light (0x2DF5B2).
        let g = shot.effect.as_ref().map_or(128, |e| e.stage_light);
        if let (Some(ndc), Some(mut m)) = (duel_arena::ndc(&cam, world), materials.get_mut(material)) {
            m.set_ndc(to_mat4(&ndc));
            if m.stage_light() != g {
                m.set_stage_light(g);
            }
        }
    }
}

/// Each update: the blur alpha of the session and a history write for the
/// next rendered frame (while a battle scene plays).
fn blur_tick(runtime: Option<Res<PlayRuntime>>, mut blur: ResMut<BlurLayer>) {
    let Some(runtime) = runtime else {
        return;
    };
    blur.alpha = runtime.play.motion_blur();
    blur.light = runtime.play.board_light();
    blur.pending = blur.camera.is_some() && runtime.play.intro.is_some();
    if !blur.pending {
        blur.alpha = 0;
    }
}

/// Before each rendered frame: when an update ran, the history pass writes
/// this frame's output into the other image and the display reads the
/// previous update's; otherwise the history pass stays off.
fn blur_frame(
    mut blur: ResMut<BlurLayer>,
    mut presents: ResMut<Assets<PresentMaterial>>,
    mut cameras: Query<(&mut Camera, &mut RenderTarget)>,
) {
    let Some(camera) = blur.camera else {
        return;
    };
    let pending = std::mem::take(&mut blur.pending);
    if let Ok((mut cam, mut target)) = cameras.get_mut(camera) {
        if !pending {
            if cam.is_active {
                cam.is_active = false;
            }
            // Outside the battle scene only the board light changes (the
            // hand row's dimming).
            let dim = 1.0 - f32::from(blur.light) / 128.0;
            if let Some(display) = blur.display.clone() {
                if presents.get(&display).is_some_and(|m| (m.blur.z - dim).abs() > 1e-6 || m.blur.x != 0.0) {
                    if let Some(mut m) = presents.get_mut(&display) {
                        m.blur = Vec4::new(0.0, 0.0, dim, 0.0);
                    }
                }
            }
            return;
        }
        let write = blur.write;
        let read = 1 - write;
        cam.is_active = true;
        *target = RenderTarget::Image(blur.images[write].clone().into());
        let alpha = f32::from(blur.alpha);
        let (Some(display), Some(history)) = (blur.display.clone(), blur.history.clone()) else {
            return;
        };
        let Some(mut shown) = presents.get(&display).cloned() else {
            return;
        };
        shown.history = blur.images[read].clone();
        shown.blur = Vec4::new(alpha, 0.0, 1.0 - f32::from(blur.light) / 128.0, 0.0);
        let mut copy = shown.clone();
        copy.blur.y = 1.0;
        if let Some(mut m) = presents.get_mut(&display) {
            *m = shown;
        }
        if let Some(mut m) = presents.get_mut(&history) {
            *m = copy;
        }
        blur.write = read;
    }
}

/// The arena crossfade's subtractive white sprite (d3a; 128 = black).
#[allow(clippy::too_many_arguments)]
fn build_arena_black(
    commands: &mut Commands,
    shot: &roses_game::duel_versus::ArenaShot,
    input: &BoardInput,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<OverlayMaterial>,
    images: &mut Assets<Image>,
    layer: &mut VersusLayer,
) {
    let a = shot.black();
    if a == 0 {
        return;
    }
    let white = layer
        .white
        .get_or_insert_with(|| {
            images.add(Image::new_fill(
                Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[255, 255, 255, 255],
                TextureFormat::Rgba8Unorm,
                RenderAssetUsages::default(),
            ))
        })
        .clone();
    let o = 1792.0;
    let corners = [
        [o, o, 0.0],
        [o, o + 512.0, 0.0],
        [o + 512.0, o + 512.0, 0.0],
        [o + 512.0, o, 0.0],
    ];
    let mesh = meshes.add(quad_mesh(&corners, &[[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]));
    let material = materials.add(overlay_material(
        KIND_SUBTRACT,
        false,
        to_mat4(&duel_board::ndc_from_gs_window()),
        Vec4::new(255.0, 255.0, 255.0, f32::from(a)),
        input,
        white,
    ));
    commands.spawn((
        Name::new("arena crossfade"),
        VersusDraw,
        Mesh3d(mesh),
        MeshMaterial3d(material),
        order(-0.5),
        NoFrustumCulling,
        RenderLayers::layer(VERSUS_LAYER),
    ));
}

/// TEXEFF record 15 as a nearest-sampled texture (alpha bit -> 255), made
/// once; `None` (reported) without TEXEFF.MRG.
fn texeff_digits(
    layer: &mut VersusLayer,
    texeff: Option<&[u8]>,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    if layer.digits.is_none() {
        let decoded = texeff.and_then(|bytes| {
            let t = roses_assets::decode_pal_opening_indexed_texture(
                bytes,
                roses_assets::OpeningArchiveKind::TexEff,
                15,
            )
            .ok()?;
            roses_client::card_face::indexed_gs_rgba(&t).ok()
        });
        if decoded.is_none() {
            eprintln!(
                "Duel board: unsupported, not drawn: the battle return effects and damage number (the import has no TEXEFF.MRG; import the game again)"
            );
        }
        layer.digits = Some(decoded.map(|image| {
            let mut pixels = image.pixels;
            for p in pixels.chunks_exact_mut(4) {
                p[3] = if p[3] >= 128 { 255 } else { 0 };
            }
            let mut texture = Image::new(
                Extent3d {
                    width: image.width,
                    height: image.height,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                pixels,
                TextureFormat::Rgba8Unorm,
                RenderAssetUsages::default(),
            );
            texture.sampler = canvas_sampler(ImageFilterMode::Nearest);
            images.add(texture)
        }));
    }
    layer.digits.clone().flatten()
}

/// The return's flash layers (`duel_return`): one mesh per layer, added
/// (Cs * As + Cd) onto the versus canvas, in the original's draw order.
#[allow(clippy::too_many_arguments)]
fn build_return_effects(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    ret: &roses_game::duel_return::Return,
    texeff: Option<&[u8]>,
    input: &BoardInput,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<OverlayMaterial>,
    images: &mut Assets<Image>,
) {
    if ret.done {
        return;
    }
    let Some(texture) = texeff_digits(layer, texeff, images) else {
        return;
    };
    let o = 1792.0f32;
    for (index, effect) in ret.pass.layers().into_iter().enumerate() {
        if effect.colour[3] == 0 || effect.quads.is_empty() {
            continue;
        }
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut indices = Vec::new();
        for q in &effect.quads {
            let base = positions.len() as u32;
            for i in 0..4 {
                positions.push([q.screen[i][0] + o, q.screen[i][1] + o, 0.0]);
                uvs.push([q.uv[i][0] / 256.0, q.uv[i][1] / 256.0]);
            }
            // GS strip order: (0, 1, 2), (1, 3, 2).
            indices.extend([base, base + 1, base + 2, base + 1, base + 3, base + 2]);
        }
        let mesh = meshes.add(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
            .with_inserted_indices(Indices::U32(indices)),
        );
        let c = effect.colour.map(f32::from);
        let material = materials.add(overlay_material(
            KIND_ADD_TEXTURED,
            false,
            to_mat4(&duel_board::ndc_from_gs_window()),
            Vec4::new(c[0], c[1], c[2], c[3]),
            input,
            texture.clone(),
        ));
        commands.spawn((
            Name::new(format!("return {}", effect.name)),
            VersusDraw,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            order(4.0 + index as f32 * 0.1),
            NoFrustumCulling,
            RenderLayers::layer(VERSUS_LAYER),
        ));
    }
}

/// The return's damage number (`duel_versus::DamageNumber`): one quad per
/// glyph from TEXEFF record 15 on the versus canvas.
#[allow(clippy::too_many_arguments)]
fn build_damage_number(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    damage: &roses_game::duel_versus::DamageNumber,
    texeff: Option<&[u8]>,
    input: &BoardInput,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<OverlayMaterial>,
    images: &mut Assets<Image>,
) {
    use roses_game::duel_versus::{DAMAGE_COLOUR, GLYPH_TEXELS};
    let Some(alpha) = damage.alpha() else {
        return;
    };
    let Some(texture) = texeff_digits(layer, texeff, images) else {
        return;
    };
    let colour = Vec4::new(
        f32::from(DAMAGE_COLOUR[0]),
        f32::from(DAMAGE_COLOUR[1]),
        f32::from(DAMAGE_COLOUR[2]),
        f32::from(alpha),
    );
    let o = 1792.0;
    for (index, ([x0, y0, x1, y1], v)) in damage.quads().into_iter().enumerate() {
        let [x0, y0, x1, y1] = [x0, y0, x1, y1].map(f64::from);
        let corners = [
            [x0 + o, y0 + o, 0.0],
            [x0 + o, y1 + o, 0.0],
            [x1 + o, y1 + o, 0.0],
            [x1 + o, y0 + o, 0.0],
        ];
        let (u1, v0, v1) = (
            GLYPH_TEXELS[0] / 256.0,
            v / 256.0,
            (v + GLYPH_TEXELS[1]) / 256.0,
        );
        let uv = [[0.0, v0], [0.0, v1], [u1, v1], [u1, v0]];
        let mesh = meshes.add(quad_mesh(&corners, &uv));
        let material = materials.add(overlay_material(
            KIND_DARKEN,
            false,
            to_mat4(&duel_board::ndc_from_gs_window()),
            colour,
            input,
            texture.clone(),
        ));
        commands.spawn((
            Name::new(format!("damage glyph {index}")),
            VersusDraw,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            order(10.0 + index as f32),
            NoFrustumCulling,
            RenderLayers::layer(VERSUS_LAYER),
        ));
    }
}

/// Spawn the versus card quads: flying cards as 3D quads through the
/// board camera's view (the back when facing away), resting cards as
/// screen sprites.
#[allow(clippy::too_many_arguments)]
fn build_versus(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    versus: &roses_game::duel_versus::Versus,
    view: &camera::Mat4,
    input: &BoardInput,
    hud: &mut DuelHud,
    texeff: Option<&[u8]>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<OverlayMaterial>,
    images: &mut Assets<Image>,
    mut progress: Option<&mut super::RenderProgress>,
) {
    use roses_game::duel_versus::{CARD_CORNERS, CARD_UV, Versus};
    // Set again by this update's attack effect when it samples a frame copy.
    layer.copy_wanted = false;
    // c605 draws onto the board canvas (the board shows around the cards);
    // before it the versus canvas is shown alone.
    let render_layer = if versus.end.is_some() {
        0
    } else {
        VERSUS_LAYER
    };
    let drawn: Vec<(
        usize,
        &roses_game::duel_versus::VersusCard,
        bool,
    )> = match &versus.end {
        Some(end) => end
            .survivors
            .iter()
            .filter(|s| !s.landed)
            .map(|s| (s.index, &s.card, !s.flying))
            .collect(),
        None => {
            let resting = versus.resting();
            versus
                .cards
                .iter()
                .take(versus.shown())
                .enumerate()
                .map(|(i, c)| (i, c, resting))
                .collect()
        }
    };
    for (index, card, resting) in drawn {
        let front = resting || card.front_facing(view);
        let key = if front {
            (card.card, card.level, card.attack, card.defence)
        } else {
            (roses_client::card_face::BACK_CARD, 0, 0, 0)
        };
        versus_face_texture(layer, hud, key, images, progress.as_deref_mut());
        let Some(texture) = layer.faces[&key].clone() else {
            continue;
        };
        let uv = CARD_UV.map(|[u, v]| [u / 256.0, v / 368.0]);
        let (corners, ndc) = if resting {
            // 0x1DB530: a screen sprite; positions in GS window coordinates.
            let [x0, y0, x1, y1] = Versus::rest_rect(index).map(f64::from);
            let o = 1792.0;
            (
                [
                    [x0 + o, y0 + o, 0.0],
                    [x0 + o, y1 + o, 0.0],
                    [x1 + o, y1 + o, 0.0],
                    [x1 + o, y0 + o, 0.0],
                ],
                to_mat4(&duel_board::ndc_from_gs_window()),
            )
        } else {
            (
                CARD_CORNERS.map(|[x, y]| [x, y, 0.0]),
                to_mat4(&duel_board::ndc_from_source(view, &card.world())),
            )
        };
        let mesh = meshes.add(quad_mesh(&corners, &uv));
        // Over the arena the cards fade with d3b.
        let alpha = versus.arena.as_ref().map_or(128, |a| a.card_alpha());
        if alpha == 0 {
            continue;
        }
        let mut card_material = overlay_material(
            KIND_QUAD,
            true,
            ndc,
            Vec4::new(128.0, 128.0, 128.0, f32::from(alpha)),
            input,
            texture,
        );
        if render_layer == 0 {
            // Over the board: c605's copies draw at Z 0xFFFFFF [H for the
            // 3D copy; F for its burn quad].
            card_material.params.z = 1.0;
        }
        let material = materials.add(card_material);
        if let Some(p) = progress.as_deref_mut() {
            p.meshes.push(mesh.id());
            p.materials.push(material.id().untyped());
        }
        commands.spawn((
            Name::new(format!("versus card {index}")),
            VersusDraw,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            order(index as f32 + if render_layer == 0 { 900.0 } else { 0.0 }),
            NoFrustumCulling,
            RenderLayers::layer(render_layer),
        ));
    }
    if let Some(end) = &versus.end {
        build_burns(
            commands, layer, end, texeff, hud, versus, meshes, materials, images, input,
        );
    }
}

/// The summon flight's card (`duel_summon`) on the board canvas, its face
/// or back by the 0x1DB180 facing test, in front of the board [H depth].
#[allow(clippy::too_many_arguments)]
fn build_summon_flight(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    flight: &roses_game::duel_summon::SummonFlight,
    (view, polar): (&camera::Mat4, [f64; 3]),
    input: &BoardInput,
    hud: &mut DuelHud,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<OverlayMaterial>,
    images: &mut Assets<Image>,
    mut progress: Option<&mut super::RenderProgress>,
    fx: (Option<&[u8]>, &mut Assets<FxMaterial>),
) {
    use roses_game::duel_versus::{CARD_CORNERS, CARD_UV};
    if let Some(show) = flight.fusion_playing() {
        build_fusion_show(commands, layer, (show, &flight.fusion_cards()), (view, polar), input, hud, meshes, materials, images, progress, fx);
        return;
    }
    if !flight.flying() {
        return;
    }
    let card = &flight.card;
    let key = if card.front_facing(view) {
        flight.face()
    } else {
        (roses_client::card_face::BACK_CARD, 0, 0, 0)
    };
    versus_face_texture(layer, hud, key, images, progress.as_deref_mut());
    let Some(texture) = layer.faces.get(&key).cloned().flatten() else {
        return;
    };
    let uv = CARD_UV.map(|[u, v]| [u / 256.0, v / 368.0]);
    let corners = CARD_CORNERS.map(|[x, y]| [x, y, 0.0]);
    let mesh = meshes.add(quad_mesh(&corners, &uv));
    let mut m = overlay_material(
        KIND_QUAD,
        true,
        to_mat4(&duel_board::ndc_from_source(view, &card.world())),
        Vec4::new(128.0, 128.0, 128.0, 128.0),
        input,
        texture,
    );
    m.params.z = 1.0;
    let material = materials.add(m);
    if let Some(p) = progress.as_deref_mut() {
        p.meshes.push(mesh.id());
        p.materials.push(material.id().untyped());
    }
    commands.spawn((
        Name::new("summon flight card"),
        VersusDraw,
        Mesh3d(mesh),
        MeshMaterial3d(material),
        order(905.0),
        NoFrustumCulling,
        RenderLayers::layer(0),
    ));
}

/// c603's 368 pieces of a card image (`duel_victims::C603`), opaque
/// (Cs * As) for u 1-16 then additive, on the board canvas under the HUD.
#[allow(clippy::too_many_arguments)]
fn spawn_c603(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    c603: &roses_game::duel_victims::C603,
    key: (u16, u8, i32, i32),
    view: &camera::Mat4,
    input: &BoardInput,
    hud: &mut DuelHud,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<OverlayMaterial>,
    images: &mut Assets<Image>,
    progress: Option<&mut super::RenderProgress>,
) {
    versus_face_texture(layer, hud, key, images, progress);
    let Some(texture) = layer.faces.get(&key).cloned().flatten() else {
        return;
    };
    let (alpha, add) = c603.alpha();
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for (corners, uv) in c603.pieces() {
        let base = positions.len() as u32;
        for k in [0usize, 1, 3, 2] {
            positions.push(corners[k].map(|v| v as f32));
            uvs.push([uv[k][0] / 256.0, uv[k][1] / 368.0]);
        }
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let mesh = meshes.add(
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
            .with_inserted_indices(Indices::U32(indices)),
    );
    let m = overlay_material(
        if add { KIND_ADD_TEXTURED } else { KIND_DARKEN },
        false,
        to_mat4(&duel_board::ndc_from_source(view, &camera::identity())),
        Vec4::new(128.0, 128.0, 128.0, f32::from(alpha)),
        input,
        texture,
    );
    let material = materials.add(m);
    commands.spawn((
        Name::new("c603 pieces"),
        VersusDraw,
        Mesh3d(mesh),
        MeshMaterial3d(material),
        order(930.0),
        NoFrustumCulling,
        RenderLayers::layer(0),
    ));
}

/// A monster effect's modules (`duel_monster_fx`) on the board canvas and
/// the Bug victim's c603.
#[allow(clippy::too_many_arguments)]
fn build_monster_fx(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    show: &roses_game::duel_monster_fx::MonsterShow,
    view: &camera::Mat4,
    (eye, target): ([f64; 3], [f64; 3]),
    input: &BoardInput,
    hud: &mut DuelHud,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<OverlayMaterial>,
    images: &mut Assets<Image>,
    mut progress: Option<&mut super::RenderProgress>,
    (texeff, fx_materials): (Option<&[u8]>, &mut Assets<FxMaterial>),
) {
    // The Duster's cross cells (0x261DB0 tint; its look H): alpha blended
    // under the modules.
    let tint = show.tint_tris(eye, target);
    if !tint.is_empty() {
        spawn_fx_blend(commands, layer, &tint, texeff, input, meshes, fx_materials, images, (0, 939.0, "cell tint", 3));
    }
    let tris = show.tris(eye, target);
    if !tris.is_empty() {
        spawn_fx(commands, layer, &tris, texeff, input, meshes, fx_materials, images, 0, 940.0, "monster fx");
    }
    // c533 (Lawbook): screen anchored, Z max, key 0: over the HUD.
    let top = show.top_tris();
    if !top.is_empty() {
        spawn_fx(commands, layer, &top, texeff, input, meshes, fx_materials, images, TOP_LAYER, 955.0, "c533");
    }
    // c509: explosion B subtractive (0x42), cores and number Cs * As (0x88).
    let (sub, opaque) = show.blended_tris();
    if !sub.is_empty() {
        spawn_fx_blend(commands, layer, &sub, texeff, input, meshes, fx_materials, images, (0, 941.0, "c509 smoke", 1));
    }
    if !opaque.is_empty() {
        spawn_fx_blend(commands, layer, &opaque, texeff, input, meshes, fx_materials, images, (0, 942.0, "c509 cores", 2));
    }
    let flash = show.flash_tris(eye, target);
    if !flash.is_empty() {
        spawn_fx(commands, layer, &flash, texeff, input, meshes, fx_materials, images, TOP_LAYER, 960.0, "c500 flash");
    }
    if let (Some(c603), Some(t)) = (&show.c603, show.targets.first()) {
        let face = match (t.face_up, t.face) {
            (true, Some(f)) => (t.card, f.level, f.attack, f.defence),
            _ => (roses_client::card_face::BACK_CARD, 0, 0, 0),
        };
        spawn_c603(commands, layer, c603, face, view, input, hud, meshes, materials, images, progress.as_deref_mut());
    }
    // The ritual's card objects (c059): the sacrifices flying and the
    // black result, as board card quads with their colour.
    for (i, (card, pose, colour)) in show.ritual_cards().into_iter().enumerate() {
        use roses_game::duel_versus::{CARD_CORNERS, CARD_UV, VersusCard};
        if colour[3] == 0 {
            continue;
        }
        let v = VersusCard { card, level: 0, attack: 0, defence: 0, pose, target: pose, remaining: 0 };
        let key = if v.front_facing(view) {
            let face = show.targets.iter().find(|t| t.card == card).and_then(|t| t.face);
            face.map_or((card, 0, 0, 0), |f| (card, f.level, f.attack, f.defence))
        } else {
            (roses_client::card_face::BACK_CARD, 0, 0, 0)
        };
        versus_face_texture(layer, hud, key, images, progress.as_deref_mut());
        let Some(texture) = layer.faces.get(&key).cloned().flatten() else {
            continue;
        };
        let uv = CARD_UV.map(|[u, v]| [u / 256.0, v / 368.0]);
        let corners = CARD_CORNERS.map(|[x, y]| [x, y, 0.0]);
        let mesh = meshes.add(quad_mesh(&corners, &uv));
        let mut m = overlay_material(
            KIND_QUAD,
            true,
            to_mat4(&duel_board::ndc_from_source(view, &v.world())),
            Vec4::new(f32::from(colour[0]), f32::from(colour[1]), f32::from(colour[2]), f32::from(colour[3])),
            input,
            texture,
        );
        m.params.z = 1.0;
        let material = materials.add(m);
        commands.spawn((
            Name::new(format!("ritual card {i}")),
            VersusDraw,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            order(906.0 + i as f32 * 0.1),
            NoFrustumCulling,
            RenderLayers::layer(0),
        ));
    }
    // c612's smoke (ALPHA 0x44) over the scene.
    let smoke = show.alpha_blend_tris();
    if !smoke.is_empty() {
        spawn_fx_blend(commands, layer, &smoke, texeff, input, meshes, fx_materials, images, (0, 944.0, "c612 smoke", 3));
    }
    // The Force Field's trap shatters (c603 param 1), one per trap.
    for (c603, t) in show.c603s.iter().zip(&show.targets) {
        let face = match (t.face_up, t.face) {
            (true, Some(f)) => (t.card, f.level, f.attack, f.defence),
            (true, None) => (t.card, 0, 0, 0),
            _ => (roses_client::card_face::BACK_CARD, 0, 0, 0),
        };
        spawn_c603(commands, layer, c603, face, view, input, hud, meshes, materials, images, progress.as_deref_mut());
    }
}

/// N-player (a port addition, MODES): a knockout's presentation with
/// "cards leave" (`duel_multi::KnockoutShow`): c603's pieces of every unit
/// of the player and c545's puffs, drawn as a destroyed trap victim's.
#[allow(clippy::too_many_arguments)]
fn build_knockout_fx(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    play: &DuelPlay,
    view: &camera::Mat4,
    (eye, target): ([f64; 3], [f64; 3]),
    input: &BoardInput,
    hud: &mut DuelHud,
    (meshes, materials, images): (&mut Assets<Mesh>, &mut Assets<OverlayMaterial>, &mut Assets<Image>),
    mut progress: Option<&mut super::RenderProgress>,
    (texeff, fx_materials): (Option<&[u8]>, &mut Assets<FxMaterial>),
) {
    #[cfg(feature = "online")]
    {
        let Some(show) = play.multi.as_ref().and_then(|m| m.knockout.as_ref()) else {
            return;
        };
        let tris = show.smoke_tris(eye, target);
        if !tris.is_empty() {
            spawn_fx(commands, layer, &tris, texeff, input, meshes, fx_materials, images, 0, 942.0, "knockout c545");
        }
        for (c603, face) in show.pieces() {
            spawn_c603(commands, layer, c603, face, view, input, hud, meshes, materials, images, progress.as_deref_mut());
        }
    }
    #[cfg(not(feature = "online"))]
    {
        let _ = (commands, layer, play, view, eye, target, input, hud, meshes, materials, images, progress.as_deref_mut(), texeff, fx_materials);
    }
}

/// Dark Elf's c513 in the battle opening (board canvas) and Riryoku's c537
/// (screen anchored, Z max, key 0: over the HUD; its number first, Cs * As
/// at u 16..21 then alpha blended, then the library).
#[allow(clippy::too_many_arguments)]
fn build_extra_fx(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    play: &DuelPlay,
    (eye, target): ([f64; 3], [f64; 3]),
    input: &BoardInput,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    (texeff, fx_materials): (Option<&[u8]>, &mut Assets<FxMaterial>),
) {
    if let Some(intro) = &play.intro {
        let tris = intro.effect_tris(eye, target);
        if !tris.is_empty() {
            spawn_fx(commands, layer, &tris, texeff, input, meshes, fx_materials, images, 0, 940.0, "c513 (Dark Elf)");
        }
    }
    // The duel opening's c620 (run as original code), additive on the
    // board canvas.
    for (side, fx) in play.opening_fx.iter().enumerate() {
        let Some(fx) = fx else { continue };
        if fx.prims.is_empty() {
            continue;
        }
        let draw = roses_game::duel_bmodule::EffectDraw {
            card: 0xFF00 | side as u16,
            prims: fx.prims.clone(),
            stage_light: 0x80,
            ..Default::default()
        };
        build_effect_prims(commands, layer, &draw, None, texeff, input, meshes, fx_materials, images, (0, false));
    }
    // The Exodia event's c620s and finale (run as original code).
    if let Some(run) = &play.exodia {
        for (i, prims) in run.prims().into_iter().enumerate() {
            // c062's pair, a full-screen subtraction (ALPHA 0x42, colour 240)
            // under a sprite of the previous frame (ALPHA 0x44): a feedback
            // trail that needs the frame history this canvas does not keep;
            // without the history the subtraction alone blacks the screen,
            // so the whole list is left out (reported at the launch).
            let prims = if prims.iter().any(|p| p.texture == Some(roses_fx::types::TextureKey::Frame)) {
                std::sync::Arc::new(Vec::new())
            } else {
                prims
            };
            if prims.is_empty() {
                continue;
            }
            let finale = i >= run.c620.len();
            let draw = roses_game::duel_bmodule::EffectDraw {
                card: if finale { roses_game::duel_exodia::LORD_CARD } else { 0xFE00 | i as u16 },
                prims,
                own: run.own.clone(),
                stage_light: 0x80,
                ..Default::default()
            };
            build_effect_prims(commands, layer, &draw, None, texeff, input, meshes, fx_materials, images, (0, false));
        }
    }
    if let Some((fx, _)) = play.equip_number.as_deref() {
        let (opaque, blend) = fx.number_tris();
        if !opaque.is_empty() {
            spawn_fx_blend(commands, layer, opaque, texeff, input, meshes, fx_materials, images, (TOP_LAYER, 950.0, "c537 number", 2));
        }
        if !blend.is_empty() {
            spawn_fx_blend(commands, layer, blend, texeff, input, meshes, fx_materials, images, (TOP_LAYER, 950.0, "c537 number", 3));
        }
        if !fx.tris.is_empty() {
            spawn_fx(commands, layer, &fx.tris, texeff, input, meshes, fx_materials, images, TOP_LAYER, 951.0, "c537");
        }
    }
}

/// The eye of a view matrix (column-major, rigid): -R^T t.
fn view_eye(view: &camera::Mat4) -> [f64; 3] {
    let t = [view[3][0], view[3][1], view[3][2]];
    std::array::from_fn(|i| -(0..3).map(|r| view[i][r] * t[r]).sum::<f64>())
}

/// c601's big card (`duel_activate`): the activated card's copy flying from
/// its cell to view (0, 0) at depth 800, held, zoomed and faded, over the
/// HUD panels (257 shows it covering the LP box [emu]); its burst on the
/// additive canvas over that; c602 on the board canvas.
#[allow(clippy::too_many_arguments)]
fn build_activation(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    activation: &roses_game::duel_activate::Activation,
    view: &camera::Mat4,
    (polar, eye, target): ([f64; 3], [f64; 3], [f64; 3]),
    input: &BoardInput,
    hud: &mut DuelHud,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<OverlayMaterial>,
    images: &mut Assets<Image>,
    mut progress: Option<&mut super::RenderProgress>,
    (texeff, fx_materials): (Option<&[u8]>, &mut Assets<FxMaterial>),
) {
    use roses_game::duel_versus::{CARD_CORNERS, CARD_UV, VersusCard};
    if let Some(burst) = &activation.burst {
        // c602 on the board canvas over the board, under the HUD (depth
        // keyed with the scene in the original; no depth test here [H]).
        let tris = burst.draw(eye, target);
        spawn_fx(commands, layer, &tris, texeff, input, meshes, fx_materials, images, 0, 940.0, "c602");
    }
    let (add, sub) = activation.decrease_tris(eye, target);
    if !sub.is_empty() {
        spawn_fx_blend(commands, layer, &sub, texeff, input, meshes, fx_materials, images, (0, 943.0, "c557 smoke A", 1));
    }
    if !add.is_empty() {
        spawn_fx(commands, layer, &add, texeff, input, meshes, fx_materials, images, 0, 944.0, "c557");
    }
    // c502's frames and walls on the board canvas, its flash over the HUD
    // (Z max, key 0).
    let (add, sub, flash) = activation.field_tris(eye, target);
    if !sub.is_empty() {
        spawn_fx_blend(commands, layer, &sub, texeff, input, meshes, fx_materials, images, (0, 947.0, "c502 subtract", 1));
    }
    if !add.is_empty() {
        spawn_fx(commands, layer, &add, texeff, input, meshes, fx_materials, images, 0, 948.0, "c502");
    }
    if !flash.is_empty() {
        spawn_fx(commands, layer, &flash, texeff, input, meshes, fx_materials, images, TOP_LAYER, 960.0, "c502 flash");
    }
    // c508 over the board, under the HUD [H as the equip pass].
    spawn_fx(commands, layer, activation.recovery_tris(), texeff, input, meshes, fx_materials, images, 0, 946.0, "c508");
    let tris = activation.shield_tris(eye, target);
    if !tris.is_empty() {
        spawn_fx(commands, layer, &tris, texeff, input, meshes, fx_materials, images, 0, 945.0, "c034");
    }
    let tris = activation.smoke_tris(eye, target);
    if !tris.is_empty() {
        spawn_fx(commands, layer, &tris, texeff, input, meshes, fx_materials, images, 0, 942.0, "c545");
    }
    if let (Some((c603, _)), Some(v)) = (&activation.shatter, activation.victim) {
        let face = match (v.face_up, v.face) {
            (true, Some(f)) => (v.card, f.level, f.attack, f.defence),
            _ => (roses_client::card_face::BACK_CARD, 0, 0, 0),
        };
        spawn_c603(commands, layer, c603, face, view, input, hud, meshes, materials, images, progress.as_deref_mut());
    }
    let tris = activation.spellbind_tris(eye, target);
    if !tris.is_empty() {
        spawn_fx(commands, layer, &tris, texeff, input, meshes, fx_materials, images, 0, 941.0, "c544");
    }
    // A trap plan's modules (c555, c546, c513): add, subtract, Cs * As, alpha
    // blend on the board canvas; c555's arrival flashes over the HUD.
    let [add, sub, opaque, blend, flash] = activation.plan_tris(eye, target);
    for (tris, key, name, mode) in [(&sub, 948.5, "plan subtract", 1u8), (&opaque, 949.0, "plan opaque", 2), (&add, 949.5, "plan", 0), (&blend, 949.8, "plan blend", 3)] {
        if !tris.is_empty() {
            spawn_fx_blend(commands, layer, tris, texeff, input, meshes, fx_materials, images, (0, key, name, mode));
        }
    }
    if !flash.is_empty() {
        spawn_fx(commands, layer, &flash, texeff, input, meshes, fx_materials, images, TOP_LAYER, 960.0, "c555 flash");
    }
    if let Some((c603, v)) = &activation.plan_c603 {
        let face = match (v.face_up, v.face) {
            (true, Some(f)) => (v.card, f.level, f.attack, f.defence),
            _ => (roses_client::card_face::BACK_CARD, 0, 0, 0),
        };
        spawn_c603(commands, layer, c603, face, view, input, hud, meshes, materials, images, progress.as_deref_mut());
    }
    if let Some((k, scale, alpha)) = activation.big_card() {
        let m: roses_assets::opening_scene::Matrix = view.map(|r| r.map(|v| v as f32));
        if let Ok(Some(p)) = roses_assets::opening_scene::ee_view::view_to_world(&m, 0.0, 0.0, 800.0) {
            let centre = roses_game::duel_versus::CardPose {
                position: [p[0], p[1], p[2]],
                spin: [0, 0x10000, 0x10000],
                tilt: roses_game::duel_versus::camera_angles(polar),
                scale: [1.0, 1.0],
            };
            let pose = roses_game::duel_activate::big_card_pose(
                activation.unit.0,
                activation.cell,
                &centre,
                k,
                scale,
            );
            let card = VersusCard {
                card: activation.card,
                level: 0,
                attack: 0,
                defence: 0,
                pose,
                target: pose,
                remaining: 0,
            };
            let key = if card.front_facing(view) {
                (activation.card, 0, 0, 0)
            } else {
                (roses_client::card_face::BACK_CARD, 0, 0, 0)
            };
            versus_face_texture(layer, hud, key, images, progress.as_deref_mut());
            if let Some(texture) = layer.faces.get(&key).cloned().flatten() {
                let uv = CARD_UV.map(|[u, v]| [u / 256.0, v / 368.0]);
                let mesh = meshes.add(quad_mesh(&CARD_CORNERS.map(|[x, y]| [x, y, 0.0]), &uv));
                let mut m = overlay_material(
                    KIND_QUAD,
                    false,
                    to_mat4(&duel_board::ndc_from_source(view, &card.world())),
                    Vec4::new(128.0, 128.0, 128.0, f32::from(alpha)),
                    input,
                    texture,
                );
                m.params.z = 1.0;
                m.cover = true;
                let material = materials.add(m);
                if let Some(p) = progress.as_deref_mut() {
                    p.meshes.push(mesh.id());
                    p.materials.push(material.id().untyped());
                }
                commands.spawn((
                    Name::new("c601 card"),
                    VersusDraw,
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    order(905.0),
                    NoFrustumCulling,
                    RenderLayers::layer(COVER_LAYER),
                ));
            }
        }
    }
    spawn_fx(commands, layer, activation.effects(), texeff, input, meshes, fx_materials, images, TOP_LAYER, 950.0, "c601");
}

/// The fusion modules' cards (C600 / C538, `duel_summon::FusionShow`) and
/// C600's white flash, on the board canvas in front of the board. C600's
/// effect layers (starburst, rings, feathers) are not drawn (reported).
#[allow(clippy::too_many_arguments)]
fn build_fusion_show(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    (show, cards): (&roses_game::duel_summon::FusionShow, &[roses_game::duel_summon::ShownCard]),
    (view, polar): (&camera::Mat4, [f64; 3]),
    input: &BoardInput,
    hud: &mut DuelHud,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<OverlayMaterial>,
    images: &mut Assets<Image>,
    mut progress: Option<&mut super::RenderProgress>,
    (texeff, fx_materials): (Option<&[u8]>, &mut Assets<FxMaterial>),
) {
    use roses_game::duel_versus::{CARD_CORNERS, CARD_UV, VersusCard};
    // The camera holds the summon cell's polar: the active side's board
    // setting (side 0 (pi/2, pi/4); side 1 from the other end).
    for (n, shown) in cards.iter().enumerate() {
        let Some(pose) = roses_game::duel_summon::shown_pose(view, polar, shown) else {
            continue;
        };
        let card = VersusCard {
            card: shown.face.0,
            level: shown.face.1,
            attack: shown.face.2,
            defence: shown.face.3,
            pose,
            target: pose,
            remaining: 0,
        };
        let key = if card.front_facing(view) {
            shown.face
        } else {
            (roses_client::card_face::BACK_CARD, 0, 0, 0)
        };
        versus_face_texture(layer, hud, key, images, progress.as_deref_mut());
        let Some(texture) = layer.faces.get(&key).cloned().flatten() else {
            continue;
        };
        let uv = CARD_UV.map(|[u, v]| [u / 256.0, v / 368.0]);
        let mesh = meshes.add(quad_mesh(&CARD_CORNERS.map(|[x, y]| [x, y, 0.0]), &uv));
        let mut m = overlay_material(
            KIND_QUAD,
            false,
            to_mat4(&duel_board::ndc_from_source(view, &card.world())),
            Vec4::new(128.0, 128.0, 128.0, f32::from(shown.alpha)),
            input,
            texture,
        );
        m.params.z = 1.0;
        let material = materials.add(m);
        commands.spawn((
            Name::new("fusion card"),
            VersusDraw,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            // First in the list on top.
            order(905.0 - n as f32 * 0.01),
            NoFrustumCulling,
            RenderLayers::layer(0),
        ));
    }
    // C600's effect layers and flash (additive, over the cards).
    spawn_fx(commands, layer, show.effects(), texeff, input, meshes, fx_materials, images, TOP_LAYER, 950.0, "c600");
}

/// The large face (or back) texture of a versus card, made once.
fn versus_face_texture(
    layer: &mut VersusLayer,
    hud: &mut DuelHud,
    key: (u16, u8, i32, i32),
    images: &mut Assets<Image>,
    mut progress: Option<&mut super::RenderProgress>,
) -> Option<Handle<Image>> {
    if !layer.faces.contains_key(&key) {
        let handle = hud.versus_face(key.0, key.1, key.2, key.3).map(|image| {
            let mut texture = Image::new(
                Extent3d {
                    width: image.width,
                    height: image.height,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                image.pixels.clone(),
                TextureFormat::Rgba8Unorm,
                RenderAssetUsages::default(),
            );
            texture.sampler = canvas_sampler(ImageFilterMode::Linear);
            let handle = images.add(texture);
            if let Some(p) = progress.as_deref_mut() {
                p.textures.push(handle.id());
            }
            handle
        });
        if handle.is_none() {
            eprintln!(
                "Duel board: unsupported, not drawn: versus card {} (PICTURE.MRG not attached or the composer failed)",
                key.0
            );
        }
        layer.faces.insert(key, handle);
    }
    layer.faces[&key].clone()
}

/// c605's burning cards on the board canvas: the card quad (Cs * As, the
/// face texture) and its flames and sparks (TEXEFF record 1, added).
#[allow(clippy::too_many_arguments)]
fn build_burns(
    commands: &mut Commands,
    layer: &mut VersusLayer,
    end: &roses_game::duel_burn::Return605,
    texeff: Option<&[u8]>,
    hud: &mut DuelHud,
    versus: &roses_game::duel_versus::Versus,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<OverlayMaterial>,
    images: &mut Assets<Image>,
    input: &BoardInput,
) {
    let o = 1792.0f32;
    let spawn_layer = |commands: &mut Commands,
                       meshes: &mut Assets<Mesh>,
                       materials: &mut Assets<OverlayMaterial>,
                       name: String,
                       quads: &[roses_game::duel_return::EffectQuad],
                       uv_size: [f32; 2],
                       kind: u8,
                       colour: [u8; 4],
                       texture: Handle<Image>,
                       key: f32| {
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut indices = Vec::new();
        for q in quads {
            let base = positions.len() as u32;
            for i in 0..4 {
                positions.push([q.screen[i][0] + o, q.screen[i][1] + o, 0.0]);
                uvs.push([q.uv[i][0] / uv_size[0], q.uv[i][1] / uv_size[1]]);
            }
            indices.extend([base, base + 1, base + 2, base + 1, base + 3, base + 2]);
        }
        if indices.is_empty() {
            return;
        }
        let mesh = meshes.add(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
            .with_inserted_indices(Indices::U32(indices)),
        );
        let c = colour.map(f32::from);
        let material = materials.add(overlay_material(
            kind,
            false,
            to_mat4(&duel_board::ndc_from_gs_window()),
            Vec4::new(c[0], c[1], c[2], c[3]),
            input,
            texture,
        ));
        commands.spawn((
            Name::new(name),
            VersusDraw,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            order(key),
            NoFrustumCulling,
            RenderLayers::layer(0),
        ));
    };
    for burn in &end.burns {
        let card = &versus.cards[burn.card];
        let key = (card.card, card.level, card.attack, card.defence);
        let face = versus_face_texture(layer, hud, key, images, None);
        if let (Some(q), Some(face)) = (burn.card_quad(), face) {
            spawn_layer(
                commands,
                meshes,
                materials,
                format!("burning card {}", burn.card),
                &[q],
                [256.0, 368.0],
                KIND_DARKEN,
                [128, 128, 128, 128],
                face,
                905.0,
            );
        }
    }
    if layer.flames.is_none() {
        let decoded = versus_texeff(texeff, 1);
        layer.flames = Some(decoded.map(|image| {
            let mut pixels = image.pixels;
            for p in pixels.chunks_exact_mut(4) {
                p[3] = if p[3] >= 128 { 255 } else { 0 };
            }
            let mut texture = Image::new(
                Extent3d {
                    width: image.width,
                    height: image.height,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                pixels,
                TextureFormat::Rgba8Unorm,
                RenderAssetUsages::default(),
            );
            texture.sampler = canvas_sampler(ImageFilterMode::Nearest);
            images.add(texture)
        }));
    }
    let Some(Some(flames)) = layer.flames.clone() else {
        return;
    };
    for (card, effects) in &end.effects {
        for (i, e) in effects.iter().enumerate() {
            if e.colour[3] == 0 {
                continue;
            }
            spawn_layer(
                commands,
                meshes,
                materials,
                format!("burn {} {card}", e.name),
                &e.quads,
                [128.0, 64.0],
                KIND_ADD_TEXTURED,
                e.colour,
                flames.clone(),
                910.0 + i as f32 * 0.01,
            );
        }
    }
}

/// A TEXEFF record decoded from the duel import (`None` without TEXEFF.MRG).
fn versus_texeff(texeff: Option<&[u8]>, record: usize) -> Option<roses_assets::RgbaImage> {
    let bytes = texeff?;
    let t = roses_assets::decode_pal_opening_indexed_texture(
        bytes,
        roses_assets::OpeningArchiveKind::TexEff,
        record,
    )
    .ok()?;
    roses_client::card_face::indexed_gs_rgba(&t).ok()
}

/// The arena's units (attacker, defender) and the arena camera's
/// world-to-clip matrix while the arena shows: during the attack phase
/// their poses, motions and fades come from the battle, before it they idle.
fn arena_units(
    runtime: Option<&PlayRuntime>,
    arena: Option<&ArenaRender>,
) -> (Option<[duel_units_render::ArenaPose; 2]>, camera::Mat4) {
    let versus = runtime.and_then(|r| r.play.intro.as_ref()).and_then(|i| i.versus.as_ref());
    let shot = versus.and_then(|v| v.arena.as_ref());
    let light = shot.and_then(|shot| arena?.loaded.as_ref()?.lights.get(&u32::from(shot.terrain)).copied());
    let ndc = shot.and_then(|shot| {
        let cmot = arena?.loaded.as_ref()?.cameras.get(&u32::from(shot.terrain))?;
        roses_game::duel_arena::ndc(&shot.camera(cmot), &camera::identity())
    });
    match (ndc, versus, shot) {
        (Some(ndc), Some(v), Some(shot)) => {
            let effect = shot.effect.as_ref();
            let poses = std::array::from_fn(|i| {
                let (position, yaw) = duel_units_render::ARENA_POSES[i];
                match &shot.battle {
                    Some(b) => duel_units_render::ArenaPose {
                        card: b.units[i].data.card,
                        position: b.units[i].object.position,
                        yaw: b.units[i].object.rotation[1],
                        clock: Some(b.units[i].clock),
                        blend: b.units[i].blend.map(|m| (m.previous.motion, m.previous.time, m.weight())),
                        light,
                        alpha: b.units[i].alpha,
                        rt: effect.is_some_and(|e| e.unit_rt[i]),
                        // A pre-model callback's camera for the render into
                        // the target: its view-projection (GS window, row
                        // vectors: the port's column-major layout as is).
                        rt_ndc: effect.and_then(|e| e.rt_vp[i]).map(|vp| {
                            let vp: camera::Mat4 = vp.map(|r| r.map(f64::from));
                            camera::mul(&duel_board::ndc_from_gs_window(), &vp)
                        }),
                        writes: effect.map(|e| e.unit_writes[i]),
                        stage_light: effect.map_or(128, |e| e.stage_light),
                    },
                    None => duel_units_render::ArenaPose {
                        card: v.cards[i].card,
                        position,
                        yaw,
                        clock: None,
                        blend: None,
                        light,
                        alpha: 128,
                        rt: false,
                        rt_ndc: None,
                        writes: None,
                        stage_light: 128,
                    },
                }
            });
            (Some(poses), ndc)
        }
        _ => (None, camera::identity()),
    }
}

/// Unit models follow the session's units, the cursor and the camera
/// every update (`duel_units_render`).
#[allow(clippy::too_many_arguments)]
fn sync_unit_models(
    mut commands: Commands,
    models: Option<ResMut<UnitModels>>,
    overlay: Res<OverlayRuntime>,
    state: Res<BoardState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<ModelMaterial>>,
    (runtime, arena): (Option<Res<PlayRuntime>>, Option<Res<ArenaRender>>),
    (versus_layer, input, mut overlay_materials, old_fades): (
        Option<Res<VersusLayer>>,
        Res<BoardInput>,
        ResMut<Assets<OverlayMaterial>>,
        Query<Entity, With<UnitFadeDraw>>,
    ),
) {
    for entity in &old_fades {
        commands.entity(entity).despawn();
    }
    let Some(mut models) = models else {
        return;
    };
    models.fade_update(state.board_camera.eye(), &overlay.input.units);
    if let (Some(a), Some(canvas)) = (models.fade_composite_alpha(), versus_layer.and_then(|l| l.fade.clone())) {
        // A fading board unit: the fade canvas over the board canvas,
        // colour (128, 128, 128, a), ALPHA 0x44 [F C621-DESTROY.md 7].
        let (p, q) = (1792.0f32, 1792.0 + 512.0);
        let mesh = meshes.add(
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[p, p, 0.0], [p, q, 0.0], [q, p, 0.0], [q, q, 0.0]])
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]])
                .with_inserted_indices(Indices::U32(vec![0, 1, 2, 1, 3, 2])),
        );
        let material = overlay_materials.add(overlay_material(
            KIND_QUAD,
            false,
            to_mat4(&duel_board::ndc_from_gs_window()),
            Vec4::new(128.0, 128.0, 128.0, f32::from(a)),
            &input,
            canvas,
        ));
        commands.spawn((
            Name::new("board unit fade"),
            UnitFadeDraw,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_xyz(0.0, 0.0, -40_000.0),
            NoFrustumCulling,
            RenderLayers::layer(0),
        ));
    }
    let (cards, ndc) = arena_units(runtime.as_deref(), arena.as_deref());
    models.sync_arena(
        &mut commands,
        cards,
        &ndc,
        VERSUS_LAYER,
        &mut meshes,
        &mut images,
        &mut materials,
        None,
    );
    let Some(view) = state.board_camera.view() else {
        return;
    };
    models.sync(
        &mut commands,
        &overlay.input.units,
        state.board_camera.cursor,
        &view,
        true,
        &mut meshes,
        &mut images,
        &mut materials,
        None,
    );
    let free = exodia_lord_pose(runtime.as_deref());
    models.sync_free(&mut commands, free, &view, &mut meshes, &mut images, &mut materials, None);
}

/// The Exodia event's Summoned Lord Exodia (object 0x20) while loaded.
fn exodia_lord_pose(runtime: Option<&PlayRuntime>) -> Option<crate::duel_units_render::FreePose> {
    let run = runtime?.play.exodia.as_ref()?;
    let lord = run.lord.as_ref()?;
    let s = &lord.scene;
    Some(crate::duel_units_render::FreePose {
        card: roses_game::duel_exodia::LORD_CARD,
        position: s.object.position,
        yaw: s.object.rotation[1],
        scale: lord.scale,
        clock: s.clock,
        motion: s.motion,
        blend: s.blend,
    })
}

fn overlay_material(
    kind: u8,
    z_write: bool,
    ndc: Mat4,
    color: Vec4,
    input: &BoardInput,
    texture: Handle<Image>,
) -> OverlayMaterial {
    OverlayMaterial {
        ndc_from_source: ndc,
        color_gs: color,
        params: Vec4::new(f32::from(kind), f32::from(u8::from(input.gs16)), 0.0, 0.0),
        dither_rows: input.dither_overlay,
        texture,
        kind,
        z_write,
        cover: false,
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn spawn_units(
    commands: &mut Commands,
    overlay: &mut OverlayRuntime,
    input: &BoardInput,
    view: &camera::Mat4,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<OverlayMaterial>,
    images: &mut Assets<Image>,
    mut hud: Option<&mut DuelHud>,
    mut progress: Option<&mut super::RenderProgress>,
) {
    let layers = duel_overlay::unit_layers(&overlay.input);
    for message in &layers.unsupported {
        if overlay.models_drawn && message.starts_with("3D unit models") {
            continue;
        }
        // In duel play only the first message of each kind (the text before
        // its first card number) is logged.
        let key = if overlay.report_kinds {
            message.split(" card ").next().unwrap_or(message)
        } else {
            message
        };
        if overlay.reported.insert(key.to_string()) {
            eprintln!("Duel board: unsupported, not drawn: {message}");
        }
    }
    let ndc = to_mat4(&duel_board::ndc_from_source(view, &camera::identity()));
    let (Some(card_back), Some(emblems), Some(blank)) = (
        overlay.card_back.clone(),
        overlay.emblems.clone(),
        overlay.blank.clone(),
    ) else {
        return;
    };
    // The eye (for 0x1DB180's facing test of a flipping card).
    let eye = view_eye(view);
    let mut flipped = Vec::new();
    for quad in &layers.cards {
        if let Some((normal, back)) = quad.flip {
            let c: [f64; 3] = std::array::from_fn(|k| quad.corners.iter().map(|v| v[k]).sum::<f64>() / 4.0);
            let front = (0..3).map(|k| normal[k] * (eye[k] - c[k])).sum::<f64>() > 0.0;
            if !front {
                let mut q = quad.clone();
                q.corners = back;
                q.face = None;
                flipped.push(q);
            }
        }
    }
    let mut quads = Vec::new();
    for quad in &flipped {
        quads.push((quad, card_back.clone(), 0.0));
    }
    for quad in &layers.cards {
        if let Some((normal, _)) = quad.flip {
            let c: [f64; 3] = std::array::from_fn(|k| quad.corners.iter().map(|v| v[k]).sum::<f64>() / 4.0);
            if (0..3).map(|k| normal[k] * (eye[k] - c[k])).sum::<f64>() <= 0.0 {
                continue;
            }
        }
        let Some((card, face)) = quad.face else {
            quads.push((quad, card_back.clone(), 0.0));
            continue;
        };
        if let Some(handle) = overlay.face_images.get(&(card, face)) {
            quads.push((quad, handle.clone(), 0.0));
            continue;
        }
        let image = hud.as_deref_mut().and_then(|h| h.board_face(card, face));
        let Some(image) = image else {
            let message = "face-up card image (PICTURE.MRG not attached or the composer failed)";
            if overlay.reported.insert(message.to_string()) {
                eprintln!("Duel board: unsupported, not drawn: {message}");
            }
            continue;
        };
        // The card quad samples the face like the card back (bilinear,
        // clamped; PNG-style alpha 255 = GS 128).
        let mut texture = Image::new(
            Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            image.pixels.clone(),
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        );
        texture.sampler = images
            .get(&card_back)
            .map(|i| i.sampler.clone())
            .unwrap_or_default();
        let handle = images.add(texture);
        if let Some(p) = progress.as_deref_mut() {
            p.textures.push(handle.id());
        }
        overlay.face_images.insert((card, face), handle.clone());
        quads.push((quad, handle, 0.0));
    }
    quads.extend(layers.emblems.iter().map(|q| (q, emblems.clone(), 1.0)));
    for (quad, texture, key) in quads {
        let mesh = meshes.add(quad_mesh(&quad.corners, &quad.uv));
        let rgb = f32::from(quad.rgb);
        let material = materials.add(overlay_material(
            KIND_QUAD,
            true,
            ndc,
            Vec4::new(rgb, rgb, rgb, f32::from(quad.alpha)),
            input,
            texture,
        ));
        if let Some(p) = progress.as_deref_mut() {
            p.meshes.push(mesh.id());
            p.materials.push(material.id().untyped());
        }
        commands.spawn((
            Name::new(format!("unit side {} slot {}", quad.side, quad.slot)),
            UnitLayer,
            OverlayWorld,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            order(key),
            NoFrustumCulling,
            RenderLayers::layer(0),
        ));
        if !quad.outline.is_empty() {
            let mesh = meshes.add(band_mesh(view, &quad.outline));
            let [r, g, b] = quad.outline_rgb.map(f32::from);
            // 6993 has ZMSK 0, but its AA1 pixels do not block the later
            // fills in the dump (the fills add over them): no Z write here.
            let material = materials.add(overlay_material(
                KIND_LINE,
                false,
                ndc,
                Vec4::new(r, g, b, 128.0),
                input,
                blank.clone(),
            ));
            if let Some(p) = progress.as_deref_mut() {
                p.meshes.push(mesh.id());
                p.materials.push(material.id().untyped());
            }
            commands.spawn((
                Name::new(format!("emblem outline side {}", quad.side)),
                UnitLayer,
                OverlayLines {
                    segments: quad.outline.clone(),
                    mesh: mesh.clone(),
                },
                Mesh3d(mesh),
                MeshMaterial3d(material),
                order(2.0),
                NoFrustumCulling,
                RenderLayers::layer(0),
            ));
        }
    }
    overlay.built_units = Some(overlay.input.units.clone());
}

fn load_png(bytes: &[u8], clamp: bool) -> Result<Image, String> {
    let address = if clamp {
        ImageAddressMode::ClampToEdge
    } else {
        ImageAddressMode::Repeat
    };
    Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        false,
        ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: address,
            address_mode_v: address,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Nearest,
            lod_min_clamp: 0.,
            lod_max_clamp: 0.,
            ..default()
        }),
        RenderAssetUsages::default(),
    )
    .map_err(|e| format!("Texture decode failed: {e}"))
}

/// The card image pass (dump draw 6991): TEXSYS #2 texels 256..512 x 0..368
/// copied one-to-one into a 16-bit target with the overlay DIMX. The dump
/// shows the result equals the texels (the rounding matrix never changes a
/// 5-bit value) with the alpha bit kept; sampled later with a region clamp.
fn card_back_image(frames: &Image) -> Result<Image, String> {
    let data = frames
        .data
        .as_ref()
        .ok_or("Card frame texture has no data")?;
    let width = frames.width() as usize;
    let [x0, y0] = CARD_BACK_ORIGIN.map(|v| v as usize);
    let [w, h] = CARD_IMAGE.map(|v| v as usize);
    if frames.texture_descriptor.format != TextureFormat::Rgba8Unorm
        || width < x0 + w
        || (frames.height() as usize) < y0 + h
    {
        return Err("Card frame texture does not hold the card back".into());
    }
    let mut pixels = Vec::with_capacity(w * h * 4);
    for y in y0..y0 + h {
        pixels.extend_from_slice(&data[(y * width + x0) * 4..(y * width + x0 + w) * 4]);
    }
    let mut image = Image::new(
        Extent3d {
            width: w as u32,
            height: h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    image.sampler = frames.sampler.clone();
    Ok(image)
}

#[allow(clippy::too_many_arguments)]
fn setup_board(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<BoardMaterial>>,
    mut overlay_materials: ResMut<Assets<OverlayMaterial>>,
    mut presents: ResMut<Assets<PresentMaterial>>,
    input: Res<BoardInput>,
    state: Res<BoardState>,
    mut overlay: ResMut<OverlayRuntime>,
    mut hud_layer: ResMut<HudLayer>,
    mut model_materials: ResMut<Assets<ModelMaterial>>,
    unit_models: Option<ResMut<UnitModels>>,
    mut play_runtime: Option<ResMut<PlayRuntime>>,
    capture: Option<ResMut<CaptureState>>,
    bridge: Option<Res<CaptureBridge>>,
    (mut exit, mut versus_layer, mut blur, mut arena, mut map_materials, mut fx_materials): (
        MessageWriter<AppExit>,
        ResMut<VersusLayer>,
        ResMut<BlurLayer>,
        ResMut<ArenaRender>,
        ResMut<Assets<MapMaterial>>,
        ResMut<Assets<FxMaterial>>,
    ),
) {
    let result = (|| -> Result<(), String> {
        let view = state
            .board_camera
            .view()
            .ok_or("Board camera view is degenerate")?;
        let cursor = state.board_camera.cursor;
        let mut guard = bridge
            .as_ref()
            .map(|b| b.0.lock().expect("Capture progress mutex poisoned"));
        let mut progress = guard.as_deref_mut();
        let tint = overlay.tint(cursor);
        spawn_board_draws(
            &mut commands,
            &input,
            &view,
            cursor,
            tint,
            (&mut images, &mut meshes, &mut materials),
            progress.as_deref_mut(),
        )?;
        if overlay.enabled {
            let overlay = &mut *overlay;
            let paths = &input.board.manifest.overlay_textures;
            let frames = load_png(&input.board.textures[&paths.card_frames.path], true)?;
            let card_back = images.add(card_back_image(&frames)?);
            let emblems = images.add(load_png(&input.board.textures[&paths.emblems.path], false)?);
            let blank = images.add(Image::new_fill(
                Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[0, 0, 0, 0],
                TextureFormat::Rgba8Unorm,
                RenderAssetUsages::default(),
            ));
            if let Some(p) = progress.as_deref_mut() {
                p.textures
                    .extend([card_back.id(), emblems.id(), blank.id()]);
            }
            overlay.card_back = Some(card_back);
            overlay.emblems = Some(emblems);
            overlay.blank = Some(blank.clone());
            let ndc = to_mat4(&duel_board::ndc_from_source(&view, &camera::identity()));
            let brightness = overlay.input.brightness;
            for index in 0..CELLS {
                let cell = ((index % 7) as u8, (index / 7) as u8);
                let fill = overlay.state.fill_drawn(index, brightness);
                let outline = overlay.state.cells[index].outline;
                let visible = |c: [u8; 4]| {
                    if c[3] > 0 {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    }
                };
                let mesh = meshes.add(quad_mesh(&duel_overlay::fill_quad(cell), &[[0.0; 2]; 4]));
                let material = overlay_materials.add(overlay_material(
                    KIND_FILL,
                    false,
                    ndc,
                    rgba(fill),
                    &input,
                    blank.clone(),
                ));
                if let Some(p) = progress.as_deref_mut() {
                    p.meshes.push(mesh.id());
                    p.materials.push(material.id().untyped());
                }
                let entity = commands
                    .spawn((
                        Name::new(format!("fill ({},{})", cell.0, cell.1)),
                        OverlayWorld,
                        Mesh3d(mesh),
                        MeshMaterial3d(material.clone()),
                        order(4.0),
                        visible(fill),
                        NoFrustumCulling,
                        RenderLayers::layer(0),
                    ))
                    .id();
                overlay.fills.push((entity, material));
                let segments = duel_overlay::outline_segments(cell);
                let mesh = meshes.add(band_mesh(&view, &segments));
                let material = overlay_materials.add(overlay_material(
                    KIND_LINE,
                    false,
                    ndc,
                    rgba(outline),
                    &input,
                    blank.clone(),
                ));
                if let Some(p) = progress.as_deref_mut() {
                    p.meshes.push(mesh.id());
                    p.materials.push(material.id().untyped());
                }
                let entity = commands
                    .spawn((
                        Name::new(format!("outline ({},{})", cell.0, cell.1)),
                        OverlayLines {
                            segments,
                            mesh: mesh.clone(),
                        },
                        Mesh3d(mesh),
                        MeshMaterial3d(material.clone()),
                        order(3.0),
                        visible(outline),
                        NoFrustumCulling,
                        RenderLayers::layer(0),
                    ))
                    .id();
                overlay.outlines.push((entity, material));
            }
            spawn_units(
                &mut commands,
                overlay,
                &input,
                &view,
                &mut meshes,
                &mut overlay_materials,
                &mut images,
                play_runtime.as_deref_mut().map(|r| &mut r.hud),
                progress.as_deref_mut(),
            );
        }
        // Board canvas: GS 512x512 raw canvas in byte space (Rgba8Unorm), so
        // fixed-function blending adds bytes. Present pass: capture one-to-one
        // (with the final 5-bit pass) or stretch to 4:3 in the window.
        let mut canvas = Image::new_target_texture(512, 512, TextureFormat::Rgba8Unorm, None);
        let filter = if capture.is_some() {
            ImageFilterMode::Nearest
        } else {
            ImageFilterMode::Linear
        };
        canvas.sampler = canvas_sampler(filter);
        let canvas = images.add(canvas);
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: -1,
                ..default()
            },
            RenderTarget::Image(canvas.clone().into()),
            Transform::IDENTITY,
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
            RenderLayers::layer(0),
        ));
        // The card-versus canvas: cleared to black, its own depth buffer.
        let mut versus_canvas =
            Image::new_target_texture(512, 512, TextureFormat::Rgba8Unorm, None);
        versus_canvas.sampler = canvas_sampler(filter);
        let versus_canvas = images.add(versus_canvas);
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: 0,
                clear_color: ClearColorConfig::Custom(Color::BLACK),
                ..default()
            },
            RenderTarget::Image(versus_canvas.clone().into()),
            Transform::IDENTITY,
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
            RenderLayers::from_layers(&[VERSUS_LAYER, FX_POST_LAYER]),
        ));
        // The attack effects' frame copies (0x207FC0 with a destination: a GS
        // copy of the frame being drawn): the versus layer without the effect
        // draws from the copy's first sampler on, rendered first into its own
        // image, which those draws sample [F 0x207FC0; H: the copy's place in
        // the draw order]. Active only in updates with a copy.
        let mut copy_canvas = Image::new_target_texture(512, 512, TextureFormat::Rgba8Unorm, None);
        copy_canvas.sampler = canvas_sampler(filter);
        let copy_canvas = images.add(copy_canvas);
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: -1,
                is_active: false,
                clear_color: ClearColorConfig::Custom(Color::BLACK),
                ..default()
            },
            RenderTarget::Image(copy_canvas.clone().into()),
            Transform::IDENTITY,
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
            RenderLayers::layer(VERSUS_LAYER),
            FrameCopyCamera,
        ));
        versus_layer.copy = Some(copy_canvas);
        // The arena's fade canvas: a unit whose alpha is below 128 is drawn
        // alone into it (cleared to alpha 0, own depth) and composited over
        // the versus canvas at alpha / 128 [F C621-DESTROY.md 7; the
        // off-screen target is H].
        let mut fade_canvas = Image::new_target_texture(512, 512, TextureFormat::Rgba8Unorm, None);
        fade_canvas.sampler = canvas_sampler(filter);
        let fade_canvas = images.add(fade_canvas);
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: -2,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(fade_canvas.clone().into()),
            Transform::IDENTITY,
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
            RenderLayers::layer(duel_units_render::FADE_LAYER),
        ));
        versus_layer.fade = Some(fade_canvas);
        // Role 1's own fade canvas (per unit, as the original's target).
        let mut fade1 = Image::new_target_texture(512, 512, TextureFormat::Rgba8Unorm, None);
        fade1.sampler = canvas_sampler(filter);
        let fade1 = images.add(fade1);
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: -5,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(fade1.clone().into()),
            Transform::IDENTITY,
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
            RenderLayers::layer(duel_units_render::FADE_LAYERS[1]),
        ));
        versus_layer.fade1 = Some(fade1);
        // The attack effects' unit targets (TBP 0x2000 512 x 512, cleared
        // to alpha 0): a unit an effect draws there (0x1F0FD0) renders into
        // its role's target as well [emu card 21 / 373: the composite
        // sprites sample it in the same frame].
        for role in 0..2 {
            let mut rt = Image::new_target_texture(512, 512, TextureFormat::Rgba8Unorm, None);
            rt.sampler = canvas_sampler(filter);
            let rt = images.add(rt);
            // First the clear and the pre-model callback's draws, then the
            // unit and the rest over them (0x1EF8F0's order).
            commands.spawn((
                Camera3d::default(),
                Camera {
                    order: -7 - role as isize,
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    ..default()
                },
                RenderTarget::Image(rt.clone().into()),
                Transform::IDENTITY,
                Tonemapping::None,
                DebandDither::Disabled,
                Msaa::Off,
                RenderLayers::layer(duel_units_render::UNIT_RT_PRE_LAYERS[role]),
            ));
            commands.spawn((
                Camera3d::default(),
                Camera {
                    order: -3 - role as isize,
                    clear_color: ClearColorConfig::None,
                    ..default()
                },
                RenderTarget::Image(rt.clone().into()),
                Transform::IDENTITY,
                Tonemapping::None,
                DebandDither::Disabled,
                Msaa::Off,
                RenderLayers::layer(duel_units_render::UNIT_RT_LAYERS[role]),
            ));
            versus_layer.unit_rt[role] = Some(rt);
        }
        // The effects canvas over the HUD: cleared to black, no depth test.
        let mut top_canvas = Image::new_target_texture(512, 512, TextureFormat::Rgba8Unorm, None);
        top_canvas.sampler = canvas_sampler(filter);
        let top_canvas = images.add(top_canvas);
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: 0,
                clear_color: ClearColorConfig::Custom(Color::BLACK),
                ..default()
            },
            RenderTarget::Image(top_canvas.clone().into()),
            Transform::IDENTITY,
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
            RenderLayers::layer(TOP_LAYER),
        ));
        let mut cover_canvas = Image::new_target_texture(512, 512, TextureFormat::Rgba8Unorm, None);
        cover_canvas.sampler = canvas_sampler(filter);
        let cover_canvas = images.add(cover_canvas);
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: 0,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(cover_canvas.clone().into()),
            Transform::IDENTITY,
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
            RenderLayers::layer(COVER_LAYER),
        ));
        // The cursor unit's canvas (the original's 0x2000 render): cleared to
        // alpha 0, with its own depth buffer.
        let mut units_canvas = Image::new_target_texture(512, 512, TextureFormat::Rgba8Unorm, None);
        units_canvas.sampler = canvas_sampler(filter);
        let units_canvas = images.add(units_canvas);
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: 0,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(units_canvas.clone().into()),
            Transform::IDENTITY,
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
            RenderLayers::layer(duel_units_render::UNIT_LAYER),
        ));
        if let Some(mut models) = unit_models {
            let mut created = duel_units_render::Created::default();
            models.sync(
                &mut commands,
                &overlay.input.units,
                cursor,
                &view,
                false,
                &mut meshes,
                &mut images,
                &mut model_materials,
                Some(&mut created),
            );
            let (cards, ndc) = arena_units(play_runtime.as_deref(), Some(&arena));
            models.sync_arena(
                &mut commands,
                cards,
                &ndc,
                VERSUS_LAYER,
                &mut meshes,
                &mut images,
                &mut model_materials,
                Some(&mut created),
            );
            let free = exodia_lord_pose(play_runtime.as_deref());
            models.sync_free(&mut commands, free, &view, &mut meshes, &mut images, &mut model_materials, Some(&mut created));
            if let Some(p) = progress.as_deref_mut() {
                p.textures.extend(created.textures);
                p.meshes.extend(created.meshes);
                p.materials.extend(created.materials);
            }
        }
        let quantize = capture.is_some() && input.gs16;
        // The HUD layer: bytes like the canvas (Rgba8Unorm), sampled with
        // the canvas's filter at the same coordinates.
        let mut hud = Image::new(
            Extent3d {
                width: duel_play::HUD_SIZE as u32,
                height: duel_play::HUD_SIZE as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            std::mem::take(&mut hud_layer.initial),
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        );
        hud.sampler = canvas_sampler(filter);
        let hud = images.add(hud);
        let present = presents.add(PresentMaterial {
            canvas: canvas.clone(),
            params: Vec4::new(
                f32::from(u8::from(quantize)),
                1.0,
                f32::from(u8::from(input.gs16)),
                0.0,
            ),
            hud: hud.clone(),
            units: units_canvas.clone(),
            unit_tint: tint,
            dither_rows: input.dither_board,
            versus: versus_canvas.clone(),
            history: versus_canvas.clone(),
            top: top_canvas.clone(),
            cover: cover_canvas.clone(),
            blur: Vec4::new(
                0.0,
                0.0,
                play_runtime
                    .as_deref()
                    .map_or(0.0, |r| 1.0 - f32::from(r.play.board_light()) / 128.0),
                0.0,
            ),
        });
        versus_layer.canvas = Some(versus_canvas.clone());
        versus_layer.present = Some(present.clone());
        let versus = play_runtime
            .as_deref()
            .and_then(|r| r.play.intro.as_ref())
            .and_then(|i| i.versus.clone());
        if let Some(runtime) = play_runtime.as_deref_mut() {
            if let Some(m) = runtime.play.monster_fx.clone() {
                build_monster_fx(
                    &mut commands,
                    &mut versus_layer,
                    &m,
                    &view,
                    (state.board_camera.eye(), state.board_camera.target),
                    &input,
                    &mut runtime.hud,
                    &mut meshes,
                    &mut overlay_materials,
                    &mut images,
                    progress.as_deref_mut(),
                    (runtime.play.data.texeff.as_deref(), &mut fx_materials),
                );
            }
            if let Some(a) = runtime.play.activation.clone() {
                build_activation(
                    &mut commands,
                    &mut versus_layer,
                    &a,
                    &view,
                    (state.board_camera.polar, state.board_camera.eye(), state.board_camera.target),
                    &input,
                    &mut runtime.hud,
                    &mut meshes,
                    &mut overlay_materials,
                    &mut images,
                    progress.as_deref_mut(),
                    (runtime.play.data.texeff.as_deref(), &mut fx_materials),
                );
            }
            build_extra_fx(
                &mut commands,
                &mut versus_layer,
                &runtime.play,
                (state.board_camera.eye(), state.board_camera.target),
                &input,
                &mut meshes,
                &mut images,
                (runtime.play.data.texeff.as_deref(), &mut fx_materials),
            );
            build_knockout_fx(
                &mut commands,
                &mut versus_layer,
                &runtime.play,
                &view,
                (state.board_camera.eye(), state.board_camera.target),
                &input,
                &mut runtime.hud,
                (&mut *meshes, &mut *overlay_materials, &mut *images),
                progress.as_deref_mut(),
                (runtime.play.data.texeff.as_deref(), &mut fx_materials),
            );
            if let Some(flight) = runtime.play.summon_flight.clone() {
                build_summon_flight(
                    &mut commands,
                    &mut versus_layer,
                    &flight,
                    (&view, state.board_camera.polar),
                    &input,
                    &mut runtime.hud,
                    &mut meshes,
                    &mut overlay_materials,
                    &mut images,
                    progress.as_deref_mut(),
                    (runtime.play.data.texeff.as_deref(), &mut fx_materials),
                );
            }
        }
        if let (Some(versus), Some(runtime)) = (versus, play_runtime.as_deref_mut()) {
            versus_layer.showing = versus.end.is_none();
            if let Some(mut present) = presents.get_mut(&present) {
                present.params.w = f32::from(u8::from(versus.end.is_none()));
            }
            build_versus(
                &mut commands,
                &mut versus_layer,
                &versus,
                &view,
                &input,
                &mut runtime.hud,
                runtime.play.data.texeff.as_deref(),
                &mut meshes,
                &mut overlay_materials,
                &mut images,
                progress.as_deref_mut(),
            );
            build_arena(
                &mut commands,
                &mut arena,
                versus.arena.as_ref(),
                &mut meshes,
                &mut images,
                &mut map_materials,
                None,
                progress.as_deref_mut(),
            );
            if std::env::var_os("ROSES_BMODULE_LOG").is_some() {
                eprintln!("bmodule setup: arena {} battle {:?} effect {:?}", versus.arena.is_some(), versus.arena.as_ref().and_then(|a| a.battle.as_ref()).map(|b| (b.camera, b.units.iter().map(|u| (u.data.card, u.object.position, u.clock.motion, u.clock.time)).collect::<Vec<_>>())), versus.arena.as_ref().map(|a| a.effect.as_ref().map(|e| e.prims.len())));
            }
            if let Some(battle) = versus.arena.as_ref().and_then(|a| a.battle.as_ref()) {
                build_unit_fade(&mut commands, &versus_layer, battle, versus.arena.as_ref().and_then(|a| a.effect.as_ref()), &input, &mut meshes, &mut overlay_materials);
                build_unit_colour_pass(&mut commands, &mut versus_layer, battle, versus.arena.as_ref().and_then(|a| a.effect.as_ref()), &input, &mut meshes, &mut fx_materials, &mut images);
                build_destroy_effects(
                    &mut commands,
                    &mut versus_layer,
                    battle,
                    runtime.play.data.texeff.as_deref(),
                    &input,
                    &mut meshes,
                    &mut fx_materials,
                    &mut images,
                );
            }
            if let Some(draw) = versus.arena.as_ref().and_then(|a| a.effect.as_ref()) {
                build_attack_effect(
                    &mut commands,
                    &mut versus_layer,
                    draw,
                    None,
                    runtime.play.data.texeff.as_deref(),
                    &input,
                    &mut meshes,
                    &mut fx_materials,
                    &mut images,
                );
            }
            if let Some(shot) = &versus.arena {
                build_arena_black(
                    &mut commands,
                    shot,
                    &input,
                    &mut meshes,
                    &mut overlay_materials,
                    &mut images,
                    &mut versus_layer,
                );
            }
            if let Some(ret) = &versus.ret {
                build_return_effects(
                    &mut commands,
                    &mut versus_layer,
                    ret,
                    runtime.play.data.texeff.as_deref(),
                    &input,
                    &mut meshes,
                    &mut overlay_materials,
                    &mut images,
                );
            }
            if let Some(damage) = versus.ret.as_ref().and_then(|r| r.number) {
                build_damage_number(
                    &mut commands,
                    &mut versus_layer,
                    &damage,
                    runtime.play.data.texeff.as_deref(),
                    &input,
                    &mut meshes,
                    &mut overlay_materials,
                    &mut images,
                );
            }
        }
        hud_layer.image = Some(hud.clone());
        hud_layer.present = Some(present.clone());
        let quad = meshes.add(Rectangle::new(2., 2.));
        if let Some(p) = progress.as_deref_mut() {
            p.textures
                .extend([canvas.id(), hud.id(), units_canvas.id(), top_canvas.id(), cover_canvas.id()]);
            p.meshes.push(quad.id());
            p.materials.push(present.id().untyped());
        }
        drop(guard);
        commands.spawn((
            Mesh3d(quad),
            MeshMaterial3d(present.clone()),
            Transform::IDENTITY,
            NoFrustumCulling,
            RenderLayers::layer(1),
        ));
        // Motion-blur history (interactive play only: a capture renders one
        // frame and has no previous one).
        if (capture.is_none() || capture_live_updates() > 0) && play_runtime.is_some() {
            let mut make = || {
                let mut image =
                    Image::new_target_texture(512, 512, TextureFormat::Rgba8Unorm, None);
                image.sampler = canvas_sampler(filter);
                images.add(image)
            };
            let pair = vec![make(), make()];
            let history = presents.get(&present).cloned().map(|mut m| {
                m.history = pair[0].clone();
                m.blur = Vec4::new(0.0, 1.0, 0.0, 0.0);
                presents.add(m)
            });
            if let (Some(history), Some(mut display)) = (history, presents.get_mut(&present)) {
                display.history = pair[0].clone();
                commands.spawn((
                    Mesh3d(meshes.add(Rectangle::new(2., 2.))),
                    MeshMaterial3d(history.clone()),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(BLUR_LAYER),
                ));
                let camera = commands
                    .spawn((
                        Camera3d::default(),
                        Camera {
                            order: 2,
                            is_active: false,
                            ..default()
                        },
                        RenderTarget::Image(pair[1].clone().into()),
                        Transform::IDENTITY,
                        Msaa::Off,
                        Tonemapping::None,
                        DebandDither::Disabled,
                        RenderLayers::layer(BLUR_LAYER),
                    ))
                    .id();
                blur.images = pair;
                blur.write = 1;
                blur.camera = Some(camera);
                blur.display = Some(present.clone());
                blur.history = Some(history);
            }
        }
        let present_camera = commands
            .spawn((
                Camera3d::default(),
                Camera {
                    order: 1,
                    ..default()
                },
                Transform::IDENTITY,
                Msaa::Off,
                Tonemapping::None,
                DebandDither::Disabled,
                RenderLayers::layer(1),
            ))
            .id();
        if let Some(mut capture) = capture {
            let mut target =
                Image::new_target_texture(512, 512, TextureFormat::Rgba8UnormSrgb, None);
            target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
            let target = images.add(target);
            capture.target = Some(target.clone());
            commands
                .entity(present_camera)
                .insert(RenderTarget::Image(target.into()));
        } else {
            commands.entity(present_camera).insert(PresentationCamera);
        }
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("Duel board setup failed: {error}");
        exit.write(AppExit::error());
    }
}

fn canvas_sampler(filter: ImageFilterMode) -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        mag_filter: filter,
        min_filter: filter,
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        ..default()
    })
}

/// A duel scene for the connected game (`--game`): everything `run` builds
/// for a live `--duel-play` window, as values the game inserts when the map
/// starts a duel and removes when it ends.
pub struct PlayScene {
    input: BoardInput,
    state: BoardState,
    overlay: OverlayRuntime,
    hud_layer: HudLayer,
    runtime: PlayRuntime,
    end: Option<EndRuntime>,
    tutorial: Option<TutorialRuntime>,
    models: Option<UnitModels>,
    arena: ArenaRender,
    sound: Option<DuelSound>,
}

/// Prepare the first duel on the original board: `data.first_duel` already
/// carries the decks the game chose (see `game::duel_for_request`). With
/// `end`, the duel's end runs the result screens (without it the duel stops
/// at its outcome). `opening` plays the duel's opening first
/// ([`DuelPlay::begin_opening`]).
pub fn prepare_play_scene(
    board_manifest: &std::path::Path,
    data: duel_play::DuelData,
    seed: u64,
    computer_player: bool,
    end: Option<EndInputs>,
    models: Option<&std::path::Path>,
    opening: bool,
) -> Result<PlayScene, String> {
    let mut play = DuelPlay::new(data, seed, computer_player)?;
    if opening {
        play.begin_opening();
    }
    scene_for(board_manifest, play, end, None, models)
}

/// Prepare an online duel of the game's Online Play (`online_render`): the
/// duel the lobby arranged, already joined (`DuelPlay::online`), with the
/// 2-player result screens when `end` is given.
#[cfg(feature = "online")]
pub fn prepare_online_scene(
    board_manifest: &std::path::Path,
    play: DuelPlay,
    end: Option<EndInputs>,
    models: Option<&std::path::Path>,
) -> Result<PlayScene, String> {
    scene_for(board_manifest, play, end, None, models)
}

/// Prepare a practice lesson (`tutorial::start_lesson`) on the original
/// board with the tutorial layer over the HUD; the lesson's end is read
/// with [`tutorial_end`].
pub fn prepare_tutorial_scene(
    board_manifest: &std::path::Path,
    play: DuelPlay,
    models: Option<&std::path::Path>,
) -> Result<PlayScene, String> {
    let leaders = [
        play.data.first_duel.sides[0].leader,
        play.data.first_duel.sides[1].leader,
    ];
    let screen = TutorialScreen::new(&play.data, leaders)?;
    let tutorial = TutorialRuntime {
        screen,
        hud_version: None,
        reported: BTreeSet::new(),
        traced_page: None,
    };
    scene_for(board_manifest, play, None, Some(tutorial), models)
}

fn scene_for(
    board_manifest: &std::path::Path,
    play: DuelPlay,
    end: Option<EndInputs>,
    tutorial: Option<TutorialRuntime>,
    models: Option<&std::path::Path>,
) -> Result<PlayScene, String> {
    let board = duel_board::load_board(board_manifest)?;
    let terrain: [u8; CELLS] = play.session.duel.terrain[..]
        .try_into()
        .map_err(|_| "Duel terrain must have 49 cells")?;
    let layout = BoardLayout {
        terrain,
        heights: [0; CELLS],
        window_kinds: DEFAULT_WINDOW_KINDS,
        backdrop_angle: 0,
    };
    let overlay_input = play.overlay_input();
    let board_camera = play.start_camera();
    let overlay_state = OverlayState::new(&overlay_input, board_camera.cursor);
    let mut hud = DuelHud::new(&play)?;
    if let Err(e) = hud.attach_picture(&board) {
        eprintln!("Duel HUD: unsupported, not drawn: terrain picture ({e})");
    }
    hud.update(&play);
    hud.render();
    let dither = &board.manifest.gs_dither_matrices;
    // Empty: the first update writes the caption to the window (the scene
    // before left its own there).
    let title = String::new();
    Ok(PlayScene {
        input: BoardInput {
            dither_board: dither_mat4(&dither.board),
            dither_overlay: dither_mat4(&dither.overlay),
            board,
            layout,
            brightness: 1.0,
            gs16: true,
        },
        state: BoardState {
            board_camera,
            pending: Vec::new(),
            applied_target: None,
            duel_play: true,
        },
        hud_layer: HudLayer {
            initial: hud.layer.clone(),
            image: None,
            present: None,
            version: hud.version,
            pending: None,
        },
        overlay: OverlayRuntime {
            input: overlay_input,
            state: overlay_state,
            enabled: true,
            fills: Vec::new(),
            outlines: Vec::new(),
            built_units: None,
            card_back: None,
            emblems: None,
            blank: None,
            reported: BTreeSet::new(),
            report_kinds: true,
            models_drawn: models.is_some(),
            face_images: BTreeMap::new(),
            cursor_tint: true,
        },
        runtime: PlayRuntime {
            bot: online_test_bot(&play),
            capture_frozen: false,
            rest_updates: 0,
            play,
            hud,
            held: BTreeMap::new(),
            tapped: 0,
            keys_primed: false,
            trace_pads: [0; 2],
            title,
            trace: std::env::var("ROSES_TRACE_DUEL").is_ok_and(|v| v == "1"),
        },
        end: end.map(EndRuntime::new),
        tutorial,
        models: models.map(|m| UnitModels::new(m.to_path_buf(), true)),
        arena: ArenaRender::default(),
        sound: None,
    })
}

impl PlayScene {
    /// Play the duel's music and sound effects from the duel-audio import.
    pub fn with_audio(mut self, manifest: &std::path::Path) -> Result<Self, String> {
        self.sound = Some(DuelSound::open(manifest)?);
        Ok(self)
    }

    /// Load the battle arenas (`duel-arena/manifest.json`) so battles show
    /// the arena's crossfade and orbit; without them the battle scene goes
    /// from the versus cards to the return.
    pub fn with_arena(mut self, manifest: &std::path::Path) -> Result<Self, String> {
        let loaded = roses_game::duel_arena::load_arena(manifest)?;
        if let Some(models) = &self.models {
            self.runtime.play.arena_provider =
                roses_game::duel_arena::arena_provider(&loaded, models.manifest().to_path_buf());
        }
        self.arena.loaded = Some(loaded);
        self.runtime.play.arena_available = true;
        Ok(self)
    }

    /// Insert the scene's resources, then spawn it (the same `setup_board`
    /// the standalone window runs at startup).
    pub fn enter(self, world: &mut World) -> Result<(), String> {
        world.insert_resource(self.input);
        world.insert_resource(self.state);
        world.insert_resource(self.overlay);
        world.insert_resource(self.hud_layer);
        world.insert_resource(VersusLayer::default());
        world.insert_resource(self.arena);
        // The game's app-wide sound stays; a standalone duel brings its own.
        if let Some(sound) = self.sound {
            if !world.contains_resource::<DuelSound>() {
                world.insert_resource(sound);
            }
        }
        world.insert_resource(BlurLayer::default());
        world.insert_resource(self.runtime);
        if let Some(end) = self.end {
            world.insert_resource(end);
        }
        if let Some(tutorial) = self.tutorial {
            world.insert_resource(tutorial);
        }
        if let Some(models) = self.models {
            world.insert_resource(models);
        }
        world
            .run_system_cached(setup_board)
            .map_err(|e| format!("duel board setup: {e}"))
    }
}

/// The map editor's board (mode 0xD, `map_editor_render`): the same board
/// draws with the editor's fixed camera, the cell fills the editor's draw
/// computes every update (0x1E42F0 / 0x1E44B0), no units, no cursor tint
/// pass [F 0x1AF4D8], and the editor's 2D layer as the HUD layer.
#[derive(Resource)]
pub struct EditorBoard {
    /// The board cells' terrain (respawned when it changes).
    pub terrain: [u8; CELLS],
    /// Each cell's current fill (R, G, B, A; alpha 128 = 1.0).
    pub fills: [[u8; 4]; CELLS],
    /// The 2D layer (premultiplied RGBA, 512 × 512) and its version.
    pub hud: Option<(Vec<u8>, u64)>,
}

/// Systems of the map editor's board run after this set (the editor's own
/// update writes [`EditorBoard`] in it).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EditorTick;

pub struct EditorScene {
    input: BoardInput,
    state: BoardState,
    overlay: OverlayRuntime,
    hud_layer: HudLayer,
    board: EditorBoard,
}

/// Prepare the map editor's board: `terrain` as Custom Duel loaded it, the
/// camera's target and polar (distance, yaw, pitch) as the editor's init
/// sets them.
pub fn prepare_editor_scene(
    board_manifest: &std::path::Path,
    terrain: [u8; CELLS],
    target: [f64; 3],
    polar: [f64; 3],
) -> Result<EditorScene, String> {
    let board = duel_board::load_board(board_manifest)?;
    let layout = BoardLayout { terrain, heights: [0; CELLS], window_kinds: DEFAULT_WINDOW_KINDS, backdrop_angle: 0 };
    let board_camera = BoardCamera { cursor: (3, 3), target, track: None, polar, polar_track: None, zoom: 1.0 };
    let overlay_input = OverlayInput {
        state: roses_client::map_editor::EDITOR_STATE,
        active_side: 0,
        marks: Vec::new(),
        move_allowance: 0,
        path: Vec::new(),
        brightness: 128,
        units: Vec::new(),
        pulse: None,
        settled: false,
        hidden_cells: false,
    };
    let overlay_state = OverlayState::new(&overlay_input, board_camera.cursor);
    let dither = &board.manifest.gs_dither_matrices;
    Ok(EditorScene {
        input: BoardInput {
            dither_board: dither_mat4(&dither.board),
            dither_overlay: dither_mat4(&dither.overlay),
            board,
            layout,
            brightness: 1.0,
            gs16: true,
        },
        state: BoardState { board_camera, pending: Vec::new(), applied_target: None, duel_play: true },
        overlay: OverlayRuntime {
            input: overlay_input,
            state: overlay_state,
            enabled: true,
            fills: Vec::new(),
            outlines: Vec::new(),
            built_units: Some(Vec::new()),
            card_back: None,
            emblems: None,
            blank: None,
            reported: BTreeSet::new(),
            report_kinds: false,
            models_drawn: false,
            face_images: BTreeMap::new(),
            cursor_tint: false,
        },
        hud_layer: HudLayer {
            initial: vec![0; duel_play::HUD_SIZE * duel_play::HUD_SIZE * 4],
            image: None,
            present: None,
            version: 0,
            pending: None,
        },
        board: EditorBoard { terrain, fills: [[0; 4]; CELLS], hud: None },
    })
}

impl EditorScene {
    pub fn enter(self, world: &mut World) -> Result<(), String> {
        world.insert_resource(self.input);
        world.insert_resource(self.state);
        world.insert_resource(self.overlay);
        world.insert_resource(self.hud_layer);
        world.insert_resource(VersusLayer::default());
        world.insert_resource(ArenaRender::default());
        world.insert_resource(BlurLayer::default());
        world.insert_resource(self.board);
        world.run_system_cached(setup_board).map_err(|e| format!("map editor board setup: {e}"))
    }
}

pub fn leave_editor_scene(world: &mut World) {
    world.remove_resource::<EditorBoard>();
    leave_play_scene(world);
}

/// The map editor's board systems (after [`EditorTick`]).
pub fn add_editor_systems(app: &mut App) {
    app.add_systems(
        FixedUpdate,
        (editor_board_sync, apply_camera, apply_overlay).chain().after(EditorTick).run_if(resource_exists::<EditorBoard>),
    );
}

/// Push the editor's terrain, fills and 2D layer to the board.
#[allow(clippy::too_many_arguments)]
fn editor_board_sync(
    mut commands: Commands,
    board: Res<EditorBoard>,
    mut input: ResMut<BoardInput>,
    state: Res<BoardState>,
    mut overlay: ResMut<OverlayRuntime>,
    mut layer: ResMut<HudLayer>,
    draws: Query<(Entity, &Mesh3d, &MeshMaterial3d<BoardMaterial>), With<BoardDraw>>,
    bridge: Option<Res<CaptureBridge>>,
    (mut images, mut meshes, mut materials, mut presents): (
        ResMut<Assets<Image>>,
        ResMut<Assets<Mesh>>,
        ResMut<Assets<BoardMaterial>>,
        ResMut<Assets<PresentMaterial>>,
    ),
) {
    for (cell, fill) in overlay.state.cells.iter_mut().zip(board.fills.iter()) {
        cell.fill = *fill;
        cell.outline = [0; 4];
    }
    if board.terrain != input.layout.terrain {
        if let Some(view) = state.board_camera.view() {
            input.layout.terrain = board.terrain;
            let mut guard = bridge.as_ref().map(|b| b.0.lock().expect("Capture progress mutex poisoned"));
            let mut progress = guard.as_deref_mut();
            for (e, mesh, material) in &draws {
                // A capture waits for the new draws' assets, not the old.
                if let Some(p) = progress.as_deref_mut() {
                    p.meshes.retain(|id| *id != mesh.0.id());
                    p.materials.retain(|id| *id != material.0.id().untyped());
                    if let Some(m) = materials.get(&material.0) {
                        let texture = m.texture.id();
                        p.textures.retain(|id| *id != texture);
                    }
                }
                commands.entity(e).despawn();
            }
            if let Err(e) = spawn_board_draws(
                &mut commands,
                &input,
                &view,
                state.board_camera.cursor,
                Vec4::ZERO,
                (&mut images, &mut meshes, &mut materials),
                progress,
            ) {
                eprintln!("Map editor: terrain rebuild failed: {e}");
            }
        }
    }
    if let Some((rgba, version)) = &board.hud {
        if *version != layer.version {
            layer.upload(rgba, *version, &mut images, &mut presents);
        }
    }
}

/// Remove a duel scene's resources (its entities are despawned by the game).
/// 0x2DB828 as the duel left it (the HUD's flash wave), for the next mode
/// that steps the word (the deck editor, Trade).
pub fn scroll_word(world: &World) -> Option<roses_client::duel_hud::FlashWave> {
    world.get_resource::<PlayRuntime>().map(|r| r.hud.flash)
}

/// 0x2DB826 and d63 as the duel left them (the overlay's P1, the cursor
/// glow).
pub fn scroll_p1(world: &World) -> Option<(roses_client::duel_hud::FlashWave, u8)> {
    let d63 = world.get_resource::<PlayRuntime>()?.play.d63;
    let p1 = world.get_resource::<OverlayRuntime>()?.state.p1;
    Some((roses_client::duel_hud::FlashWave { value: p1.value, rising: p1.rising }, d63))
}

pub fn leave_play_scene(world: &mut World) {
    world.remove_resource::<BoardInput>();
    world.remove_resource::<BoardState>();
    world.remove_resource::<OverlayRuntime>();
    world.remove_resource::<HudLayer>();
    world.remove_resource::<VersusLayer>();
    world.remove_resource::<ArenaRender>();
    if world.get_resource::<DuelSound>().is_some_and(|s| !s.app_wide) {
        world.remove_resource::<DuelSound>();
    }
    world.remove_resource::<BlurLayer>();
    world.remove_resource::<PlayRuntime>();
    world.remove_resource::<EndRuntime>();
    world.remove_resource::<TutorialRuntime>();
    world.remove_resource::<UnitModels>();
}

/// The running duel's result once it has ended: `Some(Some(side))` for a
/// winner, `Some(None)` for a draw; also the session error that halted it.
/// An N-player duel's end for the lobby's note: this seat's placing and
/// the winner (a port addition).
pub fn multi_end_line(world: &World) -> Option<String> {
    #[cfg(feature = "online")]
    {
        use roses_game::duel_multi;
        let r = world.get_resource::<PlayRuntime>()?;
        let m = r.play.multi.as_ref()?;
        let duel = &r.play.session.duel;
        let mine = m.seat.and_then(|s| duel_multi::placings(duel).into_iter().find(|p| p.1 == s));
        let winner = duel.outcome.and_then(|o| o.winner).map(|w| m.name(w));
        return Some(match (mine, winner) {
            (Some((1, _)), Some(_)) => "You won the match!".into(),
            (Some((p, _)), Some(w)) => format!("You placed {}; {w} won.", duel_multi::place_word(p).to_lowercase()),
            (_, None) => "The match was a draw.".into(),
            (None, Some(w)) => format!("{w} won the match."),
        });
    }
    #[cfg(not(feature = "online"))]
    {
        let _ = world;
        None
    }
}

/// Hold the N-player duel still for `ROSES_MULTI_CAPTURE_ROUND`'s capture.
pub fn freeze_for_capture(world: &mut World) {
    if let Some(mut r) = world.get_resource_mut::<PlayRuntime>() {
        r.capture_frozen = true;
    }
}

/// An N-player duel (a port addition), for `ROSES_MULTI_CAPTURE_ROUND`:
/// its round, whether nothing is presented now (the duel may be held
/// there), and whether it has rested (the camera and the HUD still too).
pub fn multi_progress(world: &World) -> Option<(u8, bool, bool)> {
    let r = world.get_resource::<PlayRuntime>()?;
    r.play.multi.as_ref()?;
    let duel = &r.play.session.duel;
    let quiet = !r.play.animating() && duel.outcome.is_none();
    Some((duel.round, quiet, quiet && r.rest_updates >= 12))
}

pub fn play_scene_outcome(world: &World) -> (Option<Option<u8>>, Option<String>) {
    world
        .get_resource::<PlayRuntime>()
        .map_or((None, None), |r| {
            (
                r.play.shown_outcome().map(|o| o.winner),
                r.play.halted.clone(),
            )
        })
}

/// The duel materials, registered once for the whole app.
pub fn add_play_materials(app: &mut App) {
    app.add_plugins(MaterialPlugin::<BoardMaterial>::default())
        .add_plugins(MaterialPlugin::<OverlayMaterial>::default())
        .add_plugins(MaterialPlugin::<FxMaterial>::default())
        .add_plugins(MaterialPlugin::<PresentMaterial>::default())
        .add_plugins(MaterialPlugin::<ModelMaterial>::default());
}

/// The live duel systems, running only while a duel scene exists (its
/// `PlayRuntime` resource), in the same order as the standalone window.
pub fn add_play_systems(app: &mut App) {
    let active = resource_exists::<PlayRuntime>;
    app.add_systems(
        RunFixedMainLoop,
        play_keys
            .run_if(active)
            .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
    )
    .add_systems(
        FixedUpdate,
        (
            play_tick,
            board_tick,
            sync_terrain,
            sync_units,
            apply_camera,
            sync_unit_models,
            sync_versus,
            apply_overlay,
            play_hud,
            end_tick,
            tutorial_tick,
            blur_tick,
            duel_sound,
        )
            .chain()
            .run_if(active),
    )
    .add_systems(Update, blur_frame.run_if(resource_exists::<BlurLayer>))
    .add_systems(Update, sync_frame_copy);
}

/// The frame-copy camera renders only in updates whose attack effect samples
/// a copy.
fn sync_frame_copy(layer: Option<Res<VersusLayer>>, mut cameras: Query<&mut Camera, With<FrameCopyCamera>>) {
    let wanted = layer.is_some_and(|l| l.copy_wanted);
    for mut camera in &mut cameras {
        if camera.is_active != wanted {
            camera.is_active = wanted;
        }
    }
}

pub fn run(args: impl Iterator<Item = OsString>) -> Result<(), String> {
    let options = parse(args)?;
    let board = duel_board::load_board(&options.manifest)?;
    let mut terrain: [u8; CELLS] = match options.terrain {
        Some(terrain) => terrain,
        None => board
            .manifest
            .terrain_maps
            .maps
            .get(options.map)
            .ok_or(format!(
                "--map {} is outside the imported maps",
                options.map
            ))?
            .clone()
            .try_into()
            .map_err(|_| "Terrain map must have 49 cells")?,
    };
    let digits = |cells: &[u8]| {
        cells
            .iter()
            .map(|c| char::from(b'0' + c))
            .collect::<String>()
    };
    let mut play = match (&options.play, &options.online) {
        (Some(dir), Some(online)) => {
            let data = duel_play::load_with_end(dir, options.duel_end.as_deref())?;
            let play = online_play(data, online)?;
            // The board shows the online duel's own board.
            terrain = play.session.duel.terrain[..]
                .try_into()
                .map_err(|_| "Duel terrain must have 49 cells")?;
            Some(play)
        }
        (Some(dir), None) if options.multi_preview.is_some() => {
            let play = multi_preview_play(dir, &options)?;
            terrain = play.session.duel.terrain[..]
                .try_into()
                .map_err(|_| "Duel terrain must have 49 cells")?;
            Some(play)
        }
        (Some(dir), None) => {
            let mut data = duel_play::load_with_end(dir, options.duel_end.as_deref())?;
            if let Some((opponent, node)) = options.opponent {
                use roses_game::game;
                let decks = roses_assets::parse_pal_decks(&data.executable).map_err(|e| e.to_string())?.decks;
                let maps = roses_assets::parse_pal_rules(&data.executable).map_err(|e| e.to_string())?.maps;
                let side = &data.first_duel.sides[0];
                let player = game::PlayerDeck {
                    deck_record: side.deck_record,
                    leader: side.leader,
                    cards: side.cards.clone(),
                    leader_rank: decks.get(side.deck_record).map_or(1, |d| i32::from(d.leader_rank_raw)),
                };
                let request = roses_client::campaign_map::DuelRequest { node, opponent, params: [i32::from(opponent), 0, 1100, 1000 + i32::from(opponent)] };
                data.first_duel = game::duel_for_request(&request, &data.first_duel, &player, &decks, &maps, &data.executable)?;
                // The board shows the duel's own starting terrain.
                terrain = data.first_duel.terrain[..].try_into().map_err(|_| "Duel terrain must have 49 cells")?;
            }
            let mut play = DuelPlay::new(data, options.seed, options.computer_player)?;
            // Test aid: ROSES_DUEL_OPENING=1 plays the duel's opening as the
            // game's campaign duels do (the pre-run updates step it).
            if std::env::var_os("ROSES_DUEL_OPENING").is_some() {
                play.begin_opening();
            }
            // The tiles are instanced once from the duel's starting terrain,
            // which must be the board's own starting terrain.
            let duel_terrain = &play.session.duel.terrain;
            if duel_terrain[..] != terrain[..] {
                return Err(format!(
                    "The duel import's starting terrain (first-duel.json, board map {}) differs from the duel board's starting terrain ({}):
  duel  {}
  board {}",
                    play.data.first_duel.board_map,
                    if options.terrain.is_some() {
                        "--terrain".to_string()
                    } else {
                        format!("map {}", options.map)
                    },
                    digits(duel_terrain),
                    digits(&terrain)
                ));
            }
            terrain = duel_terrain[..]
                .try_into()
                .map_err(|_| "Duel terrain must have 49 cells")?;
            Some(play)
        }
        (None, _) => None,
    };
    let mut layout = BoardLayout {
        terrain,
        heights: [0; CELLS],
        window_kinds: options.window_kinds,
        backdrop_angle: options.backdrop_angle,
    };
    let mut overlay_input = match (&play, &options.units) {
        (Some(play), _) => play.overlay_input(),
        (None, Some(path)) => load_units(path)?,
        (None, None) => serde_json::from_str::<OverlayInput>("{}").map_err(|e| e.to_string())?,
    };
    let mut board_camera = play
        .as_ref()
        .map_or_else(|| BoardCamera::snapped(options.cursor), |p| p.start_camera());
    let mut overlay_state = OverlayState::new(&overlay_input, board_camera.cursor);
    let mut hud = play.as_ref().map(DuelHud::new).transpose()?;
    if let Some(hud) = hud.as_mut() {
        if let Err(e) = hud.attach_picture(&board) {
            eprintln!("Duel HUD: unsupported, not drawn: terrain picture ({e})");
        }
    }
    let mut arena_render = ArenaRender::default();
    if let Some(manifest) = &options.duel_arena {
        let loaded = roses_game::duel_arena::load_arena(manifest)?;
        if let Some(play) = play.as_mut() {
            play.arena_available = true;
            if let Some(models) = &options.duel_models {
                play.arena_provider = roses_game::duel_arena::arena_provider(&loaded, models.clone());
            }
        }
        arena_render.loaded = Some(loaded);
    }
    let mut unit_models = match (&options.duel_models, &play) {
        (Some(manifest), Some(_)) => Some(UnitModels::new(manifest.clone(), options.gs16)),
        _ => None,
    };
    // Online too: two pad controllers give the 2-player result (1P WIN /
    // 2P WIN / DRAW, any face button continues, no ranks or reels).
    let mut end = match (&options.duel_end, &play) {
        // An N-player duel ends with its placing list, not the 2-player
        // result screens.
        (Some(_), Some(play)) if play.multi.is_some() => None,
        (Some(dir), Some(play)) => {
            use roses_game::duel_end_screens as screens;
            let files = screens::load_duel_end_import(dir)?;
            let data = &play.data;
            let thresholds = roses_assets::parse_pal_rank_thresholds(&data.executable)
                .map_err(|e| e.to_string())?;
            let ranks =
                roses_assets::parse_pal_decks(&data.executable).map_err(|e| e.to_string())?;
            let rank_of = |record: usize| {
                ranks
                    .decks
                    .get(record)
                    .map_or(0, |d| u16::from(d.leader_rank_raw))
            };
            let sides = &data.first_duel.sides;
            let mut rng = options.seed;
            let key = next_rand31(&mut rng) | 1;
            let blocks = [
                screens::new_game_block(
                    key,
                    &sides[0].cards,
                    sides[0].leader,
                    rank_of(sides[0].deck_record),
                    &thresholds,
                    &mut rng,
                ),
                screens::new_game_block(
                    screens::computer_owner_key(sides[1].deck_record),
                    &sides[1].cards,
                    sides[1].leader,
                    rank_of(sides[1].deck_record),
                    &thresholds,
                    &mut rng,
                ),
            ];
            Some(EndRuntime::new(EndInputs {
                blocks,
                rng,
                map: data.first_duel.board_map as u8,
                duel_end: files,
                replay: options.end_replay.iter().copied().collect(),
            }))
        }
        _ => None,
    };
    let mut end_composite: Option<Vec<u8>> = None;
    let steps = options.replay.len().max(options.replay_pad.len());
    let ticks = options
        .ticks
        .unwrap_or(steps as u32 + duel_board::CURSOR_TRACK_FRAMES);
    // A live capture runs its last updates in the app.
    let live_updates = if options.capture.is_some() { capture_live_updates().min(ticks) } else { 0 };
    let ticks = ticks - live_updates;
    // Online, headless: after the scripted updates, keep going (the pad bot
    // playing this seat when asked) until `--online-until` holds.
    let online_drive = options.online.clone().filter(|o| o.bot || o.until.is_some());
    let bot_config = online_drive
        .as_ref()
        .and_then(|o| o.bot_config.clone())
        .unwrap_or_else(roses_game::video_bot::BotConfig::from_env);
    let real_time = bot_config.pace == roses_game::video_bot::Pace::Human;
    let mut pad_bot = roses_game::video_bot::VideoBot::new(bot_config);
    let mut next_update = Instant::now();
    let drive_start = Instant::now();
    let mut settled = 0u32;
    // The same update order as the live FixedUpdate chain (play_tick, then
    // board_tick).
    let mut tick = 0usize;
    loop {
        let scripted = tick < ticks as usize;
        if !scripted {
            if let (Some(until), Some(p)) = (options.multi_until.as_deref(), play.as_ref()) {
                // The HUD back in place after a turn change too.
                let hud_rests = hud.as_ref().is_none_or(|h| h.previous.d3e == 0);
                if (multi_condition(p, until) && hud_rests) || drive_start.elapsed() > Duration::from_secs(900) {
                    eprintln!("Multi preview: --multi-until {until} at update {} (round {}; camera target {:?} polar {:?})", p.frames, p.session.duel.round, board_camera.target, board_camera.polar);
                    break;
                }
            }
            let (Some(o), Some(p)) = (online_drive.as_ref(), play.as_ref()) else {
                if options.multi_until.is_some() && play.is_some() {
                    // Keep going until the preview's moment (above).
                    if let Some(play) = play.as_mut() {
                        play.press(0);
                        let cursor = play.cursor();
                        let radius = std::cell::RefCell::new(unit_models.as_mut());
                        duel_play::drive_camera(&mut board_camera, play, cursor, &|card| {
                            radius.borrow_mut().as_mut().and_then(|m| m.radius(card))
                        });
                        overlay_input = play.overlay_input();
                        if let Some(models) = unit_models.as_mut() {
                            models.count_update(&overlay_input.units);
                        }
                        if let Some(hud) = hud.as_mut() {
                            hud.update(play);
                        }
                    }
                    board_camera.tick();
                    overlay_state.update(&overlay_input, board_camera.cursor);
                    tick += 1;
                    continue;
                }
                break;
            };
            if online_condition(p, end.as_ref(), o.until.as_deref().unwrap_or("end"), &mut settled) {
                eprintln!("Online: --online-until {} reached after {} updates", o.until.as_deref().unwrap_or("end"), p.frames);
                break;
            }
            if drive_start.elapsed() > Duration::from_secs(if real_time { 5400 } else { 900 }) {
                eprintln!("Online: --online-until not reached in time; stopping here");
                break;
            }
        }
        if let Some(play) = play.as_mut() {
            let mut key = options.replay_pad.get(tick).copied().unwrap_or(0);
            if key != 0 {
                // A scripted press online waits (up to 10 s) until the
                // seat may act: its earlier intentions decided, the other
                // seat's turn over, the presentation idle.
                play.online_wait_until_ready(std::time::Duration::from_secs(10));
            }
            if !scripted && online_drive.as_ref().is_some_and(|o| o.bot) {
                if let Some(link) = play.online.as_ref() {
                    let can = !play.animating() && link.may_act();
                    pad_bot.observe(link.shown_revision, &play.session.duel, &play.data.names);
                    if let Some(view) = link.state.view.clone() {
                        let revision = link.state.revision;
                        key = pad_bot.key(&play.session.duel, &play.session.input, &view, revision, can, &play.data.catalogue, &play.data.tables);
                        #[cfg(feature = "online")]
                        if pad_bot.take_target() {
                            if let Some(m) = play.multi.as_mut() {
                                m.target_key = true;
                            }
                        }
                    }
                }
                if real_time {
                    // A human pace plays in real time (25 updates a second),
                    // as a window does: the other seat sees the same waits.
                    next_update += Duration::from_millis(40);
                    let now = Instant::now();
                    if next_update > now {
                        std::thread::sleep(next_update - now);
                    } else if now - next_update > Duration::from_secs(1) {
                        next_update = now;
                    }
                }
            }
            if play.opening.is_some() {
                play.update_pads([key, 0]);
            } else {
                play.press(key);
            }
            let cursor = play.cursor();
            let radius = std::cell::RefCell::new(unit_models.as_mut());
            duel_play::drive_camera(&mut board_camera, play, cursor, &|card| {
                radius.borrow_mut().as_mut().and_then(|m| m.radius(card))
            });
            overlay_input = play.overlay_input();
            if let Some(models) = unit_models.as_mut() {
                models.count_update(&overlay_input.units);
            }
            if let Some(hud) = hud.as_mut() {
                hud.update(play);
                if let Some(end) = end.as_mut() {
                    // The result sequence runs after the HUD, as in the live chain.
                    hud.render();
                    if let Some((composite, _)) = end_advance(play, hud, "", end) {
                        if composite.is_some() {
                            end_composite = composite;
                        }
                    }
                }
            }
            if !scripted && !play.animating() && play.online.as_ref().is_some_and(|l| !l.may_act()) {
                // Waiting for the server or the other seat.
                std::thread::sleep(Duration::from_millis(2));
            }
        } else if let Some(&(dc, dr)) = options.replay.get(tick) {
            board_camera.step(dc, dr);
        }
        board_camera.tick();
        overlay_state.update(&overlay_input, board_camera.cursor);
        tick += 1;
    }
    if let Some(hud) = hud.as_mut() {
        hud.render();
    }
    if let Some(play) = &play {
        // The board shows the terrain as the duel presents it now.
        if let Ok(t) = <[u8; CELLS]>::try_from(play.shown_terrain()) {
            layout.terrain = t;
        }
        layout.heights = play.shown_heights();
    }
    let instances = duel_board::instances(&board, &layout, board_camera.cursor)?;
    let draws: usize = instances
        .iter()
        .map(|i| board.models[&i.model].draws.len())
        .sum();
    let layers = duel_overlay::unit_layers(&overlay_input);
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "duel_board_manifest_sha256": board.manifest_sha256,
            "model_archive_sha256": board.manifest.source.model_archive.sha256,
            "cursor": [board_camera.cursor.0, board_camera.cursor.1],
            "camera_target": board_camera.target,
            "camera_eye": board_camera.eye(),
            "updates_run": ticks,
            "terrain": digits(&terrain),
            "window_kinds": options.window_kinds,
            "backdrop_angle": options.backdrop_angle,
            "brightness_u8": options.brightness,
            "frame_buffer": if options.gs16 { "C_16S with DTHE (5-bit output)" } else { "RGB888" },
            "instances": instances.len(),
            "draws": draws,
            "overlays": options.overlays,
            "unit_models": unit_models.as_ref().map(|_| duel_units_render::model_units(&overlay_input.units).iter().map(|w| serde_json::json!({"object": w.key.object, "card": w.key.card, "cell": [w.cell.0, w.cell.1], "cursor": w.cell == board_camera.cursor})).collect::<Vec<_>>()),
            "overlay_state": format!("{:#04x}", overlay_input.state),
            "pulse_p1": format!("{:#06x}", overlay_state.p1.value),
            "cursor_fill": overlay_state.cells[duel_overlay::cell_index(board_camera.cursor)].fill,
            "cards": layers.cards.len(),
            "emblems": layers.emblems.len(),
            "unsupported": layers.unsupported,
            "not_implemented": [
                if unit_models.is_some() { "unit motions other than idle, unit movement, summon grow-in, near-camera fade" } else { "3D unit models" },
                if options.duel_end.is_some() && play.is_some() { "the held unit's second card image (0x1CEC14)" } else { "face-up card images" },
                if play.is_some() { "HUD terrain tile picture (backing only)" } else { "HUD (duel play only)" },
                "arena #3/#4", "backdrop spin (static angle)",
                if play.is_some() { "live terrain changes (tiles not re-instanced)" } else { "held d-pad repeat" },
                "side-1 camera"
            ],
            "renderer_parity_validated": false,
            "duel_play": play.as_ref().map(|p| {
                let duel = &p.session.duel;
                serde_json::json!({
                    "seed": p.seed,
                    "computer_sides": p.session.seats.iter().enumerate()
                        .filter(|(_, s)| s.is_some()).map(|(i, _)| i).collect::<Vec<_>>(),
                    "updates_run": p.frames,
                    "battle_intro": p.intro.as_ref().map(|i| serde_json::json!({"frame": i.frame, "turns": i.turns})),
                    "round": duel.round,
                    "active_side": duel.active,
                    "lp": [duel.sides[0].lp, duel.sides[1].lp],
                    "stars": [duel.sides[0].stars, duel.sides[1].stars],
                    "input_state": format!("{:#04x}", p.session.input.mode.state()),
                    "session_cursor": [p.cursor().column, p.cursor().row],
                    "units": duel.units.len(),
                    "outcome": duel.outcome.map(|o| format!("{o:?}")),
                    "halted": p.halted,
                    "last_status": p.status,
                    "terrain_change": p.terrain_note(),
                    "title": p.title(),
                    "result_screens": end.as_ref().map(|e| match &e.active {
                        Some(a) => {
                            let v = a.end.view();
                            serde_json::json!({"state": format!("{:#04x}", v.state), "banner": v.banner, "slide": v.slide, "darken": v.darken, "frame": a.end.frame(), "outcome": a.outcome.as_ref().map(|o| format!("{o:?}"))})
                        }
                        None => serde_json::json!({"state": "waiting for the outcome"}),
                    }),
                })
            }),
            "hud": hud.as_ref().map(|h| {
                let frame = h.frame.as_ref();
                serde_json::json!({
                    "leader_ranks": h.ranks(),
                    "primitives": frame.map_or(0, |f| f.prims.len()),
                    "slide_d3e": h.previous.d3e,
                    "turn_latched_d3c": h.previous.d3c,
                    "layer_renders": h.renders,
                    "render_ms": h.render_time.as_secs_f64() * 1000.0 / h.renders.max(1) as f64,
                    "opaque_pixels": h.layer.chunks(4).filter(|p| p[3] == 255).count(),
                    "unsupported": h.unsupported(),
                })
            }),
        }))
        .map_err(|e| e.to_string())?
    );
    if options.validate_only {
        return Ok(());
    }
    let _watchdog = options.capture.as_ref().map(|_| CaptureWatchdog::start(0));
    let dither = &board.manifest.gs_dither_matrices;
    let scene_input = BoardInput {
        dither_board: dither_mat4(&dither.board),
        dither_overlay: dither_mat4(&dither.overlay),
        board,
        layout,
        brightness: f32::from(options.brightness) / 128.0,
        gs16: options.gs16,
    };
    let state = BoardState {
        board_camera,
        pending: Vec::new(),
        applied_target: None,
        duel_play: play.is_some(),
    };
    let blank = vec![0; duel_play::HUD_SIZE * duel_play::HUD_SIZE * 4];
    // The result screens' composite (banner over the HUD) replaces the HUD
    // layer while the sequence draws.
    let shown_layer = |h: &DuelHud| end_composite.clone().unwrap_or_else(|| h.layer.clone());
    let hud_layer = match (&hud, options.capture.is_some()) {
        (Some(h), true) => HudLayer {
            initial: blank,
            image: None,
            present: None,
            version: 0,
            pending: Some((shown_layer(h), h.version)),
        },
        (h, _) => HudLayer {
            initial: h.as_ref().map_or(blank, shown_layer),
            image: None,
            present: None,
            version: h.as_ref().map_or(0, |h| h.version),
            pending: None,
        },
    };
    let overlay = OverlayRuntime {
        input: overlay_input,
        state: overlay_state,
        enabled: options.overlays,
        fills: Vec::new(),
        outlines: Vec::new(),
        built_units: None,
        card_back: None,
        emblems: None,
        blank: None,
        reported: BTreeSet::new(),
        report_kinds: play.is_some(),
        models_drawn: options.duel_models.is_some() && play.is_some(),
        face_images: BTreeMap::new(),
        cursor_tint: true,
    };
    let caption = play.as_ref().map_or_else(|| title(&state), DuelPlay::title);
    let caption_for_play = caption.clone();
    let mut app = App::new();
    let mut plugins = DefaultPlugins.set(AssetPlugin {
        file_path: roses_game::asset_folder(),
        ..default()
    });
    if options.capture.is_some() {
        plugins = plugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .disable::<WinitPlugin>();
    } else {
        plugins = plugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: caption,
                resolution: (960, 720).into(),
                ..default()
            }),
            ..default()
        });
    }
    let playing = play.is_some();
    if let (Some(play), Some(hud)) = (play, hud) {
        app.insert_resource(PlayRuntime {
            bot: online_test_bot(&play),
            capture_frozen: false,
            rest_updates: 0,
            play,
            hud,
            held: BTreeMap::new(),
            tapped: 0,
            keys_primed: false,
            trace_pads: [0; 2],
            title: caption_for_play.clone(),
            trace: std::env::var("ROSES_TRACE_DUEL").is_ok_and(|v| v == "1"),
        });
        if let Some(end) = end.take() {
            app.insert_resource(end);
        }
    }
    if let Some(models) = unit_models.take() {
        app.insert_resource(models);
    }
    app.insert_resource(ClearColor(Color::BLACK))
        .insert_resource(scene_input)
        .insert_resource(state)
        .insert_resource(overlay)
        .insert_resource(hud_layer)
        .insert_resource(VersusLayer::default())
        .insert_resource(BlurLayer::default())
        .insert_resource(arena_render)
        .add_plugins(plugins)
        .add_plugins(MaterialPlugin::<MapMaterial>::default())
        .add_plugins(MaterialPlugin::<BoardMaterial>::default())
        .add_plugins(MaterialPlugin::<OverlayMaterial>::default())
        .add_plugins(MaterialPlugin::<FxMaterial>::default())
        .add_plugins(MaterialPlugin::<PresentMaterial>::default())
        .add_plugins(MaterialPlugin::<ModelMaterial>::default())
        .add_plugins(MaterialPlugin::<CloudMaterial>::default())
        .add_systems(Startup, setup_board);
    if let Some(manifest) = &options.duel_audio {
        app.insert_resource(DuelSound::open(manifest)?);
    }
    let mut capture_result = None;
    if let Some(path) = options.capture {
        let bridge = CaptureBridge::default();
        capture_result = Some(bridge.clone());
        app.insert_resource(bridge.clone())
            .insert_resource(CaptureState {
                path,
                target: None,
                started: Instant::now(),
                requested: false,
                completed: false,
                settled_frames: 0,
            })
            .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / 60.0,
            )))
            .add_systems(Update, (capture_hud_upload, drive_capture).chain())
            .add_systems(Update, sync_frame_copy);
        if live_updates > 0 && playing {
            app.insert_resource(CaptureLive { remaining: live_updates })
                .insert_resource(Time::<Fixed>::from_hz(roses_game::UPDATE_HZ))
                .add_systems(
                    FixedUpdate,
                    (
                        play_tick,
                        board_tick,
                        sync_units,
                        apply_camera,
                        sync_unit_models,
                        sync_versus,
                        apply_overlay,
                        play_hud,
                        end_tick,
                        blur_tick,
                        capture_live_step,
                    )
                        .chain()
                        .run_if(capture_live_running),
                )
                .add_systems(Update, (blur_frame, sync_frame_copy));
        }
        app.get_sub_app_mut(RenderApp)
            .ok_or("Bevy render sub-app unavailable")?
            .insert_resource(bridge)
            .add_systems(Render, inspect_render_progress.after(RenderSystems::Render));
    } else if playing {
        // The session owns the cursor: no plain board_input stepping. Keys
        // are read every frame before the fixed updates; each 25 Hz update
        // runs the duel frame, then the board update. The gamepads
        // (XInput, DualSense / DualShock 4) reach play_tick through
        // pad_input (an online duel or --duel-board has no connected game).
        crate::pad_input::add_systems(&mut app);
        app.insert_resource(Time::<Fixed>::from_hz(roses_game::UPDATE_HZ))
            .add_systems(
                RunFixedMainLoop,
                play_keys.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            )
            .add_systems(
                FixedUpdate,
                (
                    play_tick,
                    board_tick,
                    sync_units,
                    apply_camera,
                    sync_unit_models,
                    sync_versus,
                    apply_overlay,
                    play_hud,
                    end_tick,
                    blur_tick,
                    duel_sound,
                )
                    .chain(),
            )
            .add_systems(Update, (blur_frame, sync_frame_copy))
            .add_systems(Update, (fit_view, exit_on_escape, online_end_exit));
    } else {
        // Original updates run once per two PAL fields (25 Hz, `UPDATE_HZ`).
        app.insert_resource(Time::<Fixed>::from_hz(roses_game::UPDATE_HZ))
            .add_systems(
                FixedUpdate,
                (board_tick, sync_units, apply_camera, apply_overlay).chain(),
            )
            .add_systems(Update, (board_input, fit_view, exit_on_escape));
    }
    let outcome = app.run();
    if !outcome.is_success() {
        return Err(format!("Bevy renderer exited with {outcome:?}"));
    }
    if let Some(bridge) = capture_result {
        if !bridge.0.lock().map_err(|e| e.to_string())?.saved {
            return Err("Renderer exited before a validated capture was saved".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(physical: KeyCode, logical: Key) -> KeyboardInput {
        KeyboardInput {
            key_code: physical,
            logical_key: logical,
            state: ButtonState::Pressed,
            repeat: false,
            text: None,
            window: Entity::PLACEHOLDER,
        }
    }

    #[test]
    fn arrows_move_the_side_zero_cursor_and_repeats_are_ignored() {
        assert_eq!(
            board_button(&event(KeyCode::ArrowUp, Key::ArrowUp)),
            Some((0, -1))
        );
        assert_eq!(
            board_button(&event(KeyCode::ArrowRight, Key::ArrowRight)),
            Some((1, 0))
        );
        assert_eq!(
            board_button(&event(KeyCode::Numpad4, Key::ArrowLeft)),
            Some((-1, 0))
        );
        let mut held = event(KeyCode::ArrowDown, Key::ArrowDown);
        held.repeat = true;
        assert_eq!(board_button(&held), None);
        assert_eq!(
            board_button(&event(KeyCode::KeyA, Key::Character("a".into()))),
            None
        );
    }

    #[test]
    fn options_default_to_the_first_duel_and_validate_values() {
        let args = |s: &str| {
            s.split(' ')
                .map(OsString::from)
                .collect::<Vec<_>>()
                .into_iter()
        };
        let o = parse(args("--duel-board m.json")).unwrap();
        assert_eq!(
            (
                o.cursor,
                o.map,
                o.window_kinds,
                o.brightness,
                o.gs16,
                o.overlays
            ),
            ((3, 6), 2, [8, 9], 128, true, true)
        );
        let o = parse(args(
            "--duel-board m.json --cursor 1,2 --backdrop-angle 0xB958 --rgb888",
        ))
        .unwrap();
        assert_eq!(
            (o.cursor, o.backdrop_angle, o.gs16),
            ((1, 2), 0xB958, false)
        );
        let o = parse(args("--duel-board m.json --replay right,up --ticks 5")).unwrap();
        assert_eq!((o.replay, o.ticks), (vec![(1, 0), (0, -1)], Some(5)));
        let o = parse(args("--duel-board m.json --duel-units u.json")).unwrap();
        assert_eq!(o.units, Some(PathBuf::from("u.json")));
        assert!(
            parse(args(
                "--duel-board m.json --duel-units u.json --no-overlays"
            ))
            .is_err()
        );
        assert!(parse(args("--duel-board m.json --replay right,up --ticks 1")).is_err());
        assert_eq!(
            parse(args("--duel-board m.json --ticks 100000"))
                .unwrap()
                .ticks,
            Some(MAX_TICKS)
        );
        assert!(parse(args("--duel-board m.json --ticks 100001")).is_err());
        assert!(parse(args("--duel-board m.json --replay jump")).is_err());
        assert!(parse(args("--duel-board m.json --cursor 7,0")).is_err());
        assert!(parse(args("--duel-board m.json --brightness 200")).is_err());
        assert!(parse(args("--cursor 1,1")).is_err());
    }

    #[test]
    fn duel_play_keys_map_physical_keys_to_pad_bits() {
        let bits = |code: KeyCode, logical: Key| play_pad_bits(&event(code, logical));
        let c = |s: &str| Key::Character(s.into());
        assert_eq!(bits(KeyCode::ArrowUp, Key::ArrowUp), pad::UP);
        assert_eq!(bits(KeyCode::ArrowRight, Key::ArrowRight), pad::RIGHT);
        assert_eq!(bits(KeyCode::KeyX, c("x")), pad::CROSS);
        assert_eq!(bits(KeyCode::KeyA, c("a")), pad::SQUARE);
        assert_eq!(bits(KeyCode::KeyZ, c("z")), pad::CIRCLE);
        assert_eq!(bits(KeyCode::KeyD, c("d")), pad::TRIANGLE);
        assert_eq!(bits(KeyCode::KeyS, c("s")), pad::START);
        assert_eq!(bits(KeyCode::Tab, Key::Tab), pad::SELECT);
        assert_eq!(bits(KeyCode::KeyQ, c("q")), pad::L1);
        assert_eq!(bits(KeyCode::KeyE, c("e")), pad::R1);
        assert_eq!(bits(KeyCode::Digit1, c("1")), pad::L2);
        assert_eq!(bits(KeyCode::KeyW, c("w")), pad::R2);
        // Physical keys win; only unmapped ones fall back to logical arrows.
        assert_eq!(bits(KeyCode::Numpad6, Key::ArrowRight), pad::RIGHT);
        assert_eq!(bits(KeyCode::KeyA, Key::ArrowRight), pad::SQUARE);
        assert_eq!(bits(KeyCode::Numpad6, c("6")), 0);
        assert_eq!(bits(KeyCode::Enter, Key::Enter), 0);
        // Every documented key name is a real KeyCode.
        for code in [
            KeyCode::ArrowUp,
            KeyCode::ArrowDown,
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
            KeyCode::KeyX,
            KeyCode::KeyA,
            KeyCode::KeyZ,
            KeyCode::KeyD,
            KeyCode::KeyS,
            KeyCode::Tab,
            KeyCode::KeyQ,
            KeyCode::KeyE,
            KeyCode::Digit1,
            KeyCode::KeyW,
        ] {
            assert_ne!(duel_play::key_pad(&format!("{code:?}")), 0, "{code:?}");
        }
    }

    #[test]
    fn duel_play_options_exclude_board_only_input() {
        let args = |s: &str| {
            s.split(' ')
                .map(OsString::from)
                .collect::<Vec<_>>()
                .into_iter()
        };
        let o = parse(args("--duel-board m.json --duel-play d")).unwrap();
        assert_eq!(
            (o.play, o.seed, o.computer_player, o.replay_pad.len()),
            (Some(PathBuf::from("d")), duel_play::DEFAULT_SEED, false, 0)
        );
        let o = parse(args(
            "--duel-board m.json --duel-play d --seed 0x10 --player computer --replay-pad square,up*2 --ticks 3",
        ))
        .unwrap();
        assert_eq!(
            (o.seed, o.computer_player, o.replay_pad),
            (16, true, vec![pad::SQUARE, pad::UP, pad::UP])
        );
        for bad in [
            "--duel-board m.json --duel-play d --cursor 1,1",
            "--duel-board m.json --duel-play d --replay up",
            "--duel-board m.json --duel-play d --duel-units u.json",
            "--duel-board m.json --duel-play d --no-overlays",
            "--duel-board m.json --duel-play d --player weevil",
            "--duel-board m.json --duel-play d --replay-pad up,up --ticks 1",
            "--duel-board m.json --seed 3",
            "--duel-board m.json --replay-pad cross",
            "--duel-play d",
        ] {
            assert!(parse(args(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn dither_rows_become_columns_indexed_by_y_then_x() {
        let m = [[0, 1, 2, 3], [-1, -2, -3, -4], [0; 4], [3, 2, 1, 0]];
        let mat = dither_mat4(&m);
        // WGSL m[y][x] = column y, row x.
        assert_eq!(mat.col(1).x, -1.0);
        assert_eq!(mat.col(0).w, 3.0);
        assert_eq!(mat.col(3).x, 3.0);
    }
}
