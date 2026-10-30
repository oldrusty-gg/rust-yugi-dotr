//! Optional Bevy research renderer. All original data comes from supplied files.
// The duel-end report json! in duel_board_render.rs nests deeper than 128.
#![recursion_limit = "256"]
use bevy::{
    app::ScheduleRunnerPlugin,
    asset::{AssetId, RenderAssetUsages, UntypedAssetId},
    camera::{
        RenderTarget, Viewport,
        visibility::{NoFrustumCulling, RenderLayers},
    },
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::{
        CompressedImageFormats, ImageAddressMode, ImageFilterMode, ImageFormat, ImageSampler,
        ImageSamplerDescriptor, ImageType,
    },
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    mesh::{Indices, MeshVertexBufferLayoutRef},
    pbr::{MaterialPipeline, MaterialPipelineKey, PreparedMaterial},
    prelude::*,
    reflect::TypePath,
    render::{
        Render, RenderApp, RenderSystems,
        erased_render_asset::ErasedRenderAssets,
        mesh::RenderMesh,
        render_asset::RenderAssets,
        render_resource::{
            AsBindGroup, CachedPipelineState, CompareFunction, Extent3d, PipelineCache,
            PipelineDescriptor, PrimitiveTopology, RenderPipelineDescriptor,
            SpecializedMeshPipelineError, TextureDimension, TextureFormat, TextureUsages,
        },
        storage::{GpuShaderBuffer, ShaderBuffer},
        texture::GpuImage,
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    shader::{ShaderCacheError, ShaderRef},
    window::{ExitCondition, PrimaryWindow},
    winit::WinitPlugin,
};
use roses_game::camera::{self, Pose};
use roses_game::opening::{OpeningPreview, World, parse_replay, stage_world};
use roses_game::transition::{HistoryExit, SimonEntry};
use roses_game::{AddressMode, LoadedScene, load_scene};
use roses_client::Button;
use roses_core::duel::script::{Lesson, LessonEnd};
mod campaign_map_render;
mod cloud_render;
mod custom_duel_render;
mod map_editor_render;
#[cfg(feature = "online")]
mod online_render;
#[cfg(not(feature = "online"))]
#[path = "online_render_off.rs"]
mod online_render;
mod deck_editor_render;
mod duel_board_render;
mod duel_model_render;
mod duel_units_render;
mod ending_render;
mod mode_fade;
mod movie_play;
mod pad_input;
mod record;
mod slot_machine_render;
mod trade_render;
/// Video aid: key frames of a long headless capture (`ROSES_VIDEO_FRAMES`).
mod video_frames;
use cloud_render::{CloudAnimation, CloudMaterial};
use roses_game::cloud_source::load_cloud_source;
use std::{
    fs::OpenOptions,
    io::{Cursor, Write},
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

const CAPTURE_PIPELINE: &str = "roses-captured-scene";
const COMPOSITE_PIPELINE: &str = "roses-opening-composite";
const CAPTURE_READY_FRAMES: u32 = 8;

struct CaptureWatchdog {
    finish: mpsc::Sender<()>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl CaptureWatchdog {
    /// `extra_s`: a map-update gate's own wait (N updates at `UPDATE_HZ`).
    fn start(extra_s: u64) -> Self {
        let (finish, receiver) = mpsc::channel();
        let limit = 110 + extra_s;
        let worker = std::thread::spawn(move || {
            if matches!(
                receiver.recv_timeout(Duration::from_secs(limit)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ) {
                eprintln!("GPU capture watchdog timed out ({limit} seconds)");
                std::process::exit(1);
            }
        });
        Self {
            finish,
            worker: Some(worker),
        }
    }
}
impl Drop for CaptureWatchdog {
    fn drop(&mut self) {
        let _ = self.finish.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Default)]
struct RenderProgress {
    meshes: Vec<AssetId<Mesh>>,
    textures: Vec<AssetId<Image>>,
    materials: Vec<UntypedAssetId>,
    buffers: Vec<AssetId<ShaderBuffer>>,
    expect_composite: bool,
    expect_clouds: bool,
    ready_frames: u32,
    scene_ready: bool,
    saved: bool,
    error: Option<String>,
}
#[derive(Resource, Clone, Default)]
struct CaptureBridge(Arc<Mutex<RenderProgress>>);
#[derive(Resource)]
struct CaptureState {
    path: PathBuf,
    target: Option<Handle<Image>>,
    started: Instant,
    requested: bool,
    completed: bool,
    /// Main-world frames since cloud uploads stopped changing.
    settled_frames: u32,
}

fn inspect_render_progress(
    bridge: Res<CaptureBridge>,
    cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    textures: Res<RenderAssets<GpuImage>>,
    materials: Res<ErasedRenderAssets<PreparedMaterial>>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
) {
    let mut state = bridge.0.lock().expect("Capture progress mutex poisoned");
    let mut own_pipeline = false;
    let mut composite_pipeline = false;
    let mut cloud_pipeline = false;
    let mut own_ready = true;
    let mut all_ready = true;
    for pipeline in cache.pipelines() {
        let label = match &pipeline.descriptor {
            PipelineDescriptor::RenderPipelineDescriptor(d) => d.label.as_deref(),
            _ => None,
        };
        let is_own = matches!(
            label,
            Some(CAPTURE_PIPELINE | COMPOSITE_PIPELINE | cloud_render::PIPELINE)
        ) || label.is_some_and(duel_board_render::is_board_pipeline);
        cloud_pipeline |= label == Some(cloud_render::PIPELINE);
        composite_pipeline |= label == Some(COMPOSITE_PIPELINE);
        own_pipeline |= matches!(label, Some(CAPTURE_PIPELINE | duel_board_render::PIPELINE));
        if is_own && !matches!(pipeline.state, CachedPipelineState::Ok(_)) {
            own_ready = false;
        }
        match &pipeline.state {
            CachedPipelineState::Err(
                ShaderCacheError::ShaderNotLoaded(_)
                | ShaderCacheError::ShaderImportNotYetAvailable,
            ) => all_ready = false,
            CachedPipelineState::Err(error) => {
                state.error = Some(format!("GPU pipeline error: {error}"));
                return;
            }
            CachedPipelineState::Ok(_) => (),
            _ => all_ready = false,
        }
    }
    state.scene_ready = own_pipeline
        && (!state.expect_composite || composite_pipeline)
        && (!state.expect_clouds || cloud_pipeline)
        && own_ready
        && !state.meshes.is_empty()
        && state.meshes.iter().all(|id| meshes.get(*id).is_some())
        && state.textures.iter().all(|id| textures.get(*id).is_some())
        && state.buffers.iter().all(|id| buffers.get(*id).is_some())
        && state
            .materials
            .iter()
            .all(|id| materials.get(*id).is_some());
    all_ready &= state.scene_ready;
    state.ready_frames = if all_ready {
        state.ready_frames.saturating_add(1)
    } else {
        0
    };
}

fn drive_capture(
    mut commands: Commands,
    mut state: ResMut<CaptureState>,
    bridge: Res<CaptureBridge>,
    clouds: Option<Res<CloudAnimation>>,
    flow: Option<Res<GameFlow>>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.completed {
        return;
    }
    let scene_ready = flow.as_ref().map_or(Ok(true), |flow| {
        capture_scene_ready(
            flow.capture_scene,
            flow.scene,
            flow.next,
            flow.stopped.as_deref(),
        )
        .map(|ready| ready && capture_gate_open(flow))
    });
    // Live-tick captures wait until the last per-frame upload has had time to
    // reach the render world; static captures are settled immediately.
    if scene_ready == Ok(true) && clouds.as_ref().is_none_or(|c| !c.animate || c.settled()) {
        state.settled_frames = state.settled_frames.saturating_add(1);
    } else {
        state.settled_frames = 0;
    }
    let progress = bridge.0.lock().expect("Capture progress mutex poisoned");
    // A map-update gate waits N updates (`UPDATE_HZ`) on top of the usual
    // bound (the ending movie alone is 120 s). The base bound is 100 s: at
    // the original's 25 Hz the connected duel replay alone takes ~50 s.
    // An N-player duel capture (`ROSES_MULTI_CAPTURE_ROUND`) plays rounds
    // of four players first.
    let multi = if std::env::var_os("ROSES_MULTI_CAPTURE_ROUND").is_some() && roses_game::extras() { 900 } else { 0 };
    let limit = 100 + multi + video_frames::extra_seconds() + flow
        .as_ref()
        .and_then(|f| f.capture_map_gate)
        .map_or(0, |(_, n)| u64::from(n) / roses_game::UPDATE_HZ as u64 + 1);
    let error = scene_ready
        .err()
        .or_else(|| progress.error.clone())
        .or_else(|| {
            (state.started.elapsed() > Duration::from_secs(limit))
                .then(|| format!("GPU capture timed out before validated readback ({limit} seconds)"))
        });
    if let Some(error) = error {
        eprintln!("{error}");
        state.completed = true;
        exit.write(AppExit::error());
    } else if !state.requested
        && progress.ready_frames >= CAPTURE_READY_FRAMES
        && state.settled_frames >= CAPTURE_READY_FRAMES
    {
        if let Some(target) = state.target.clone() {
            commands
                .spawn(Screenshot::image(target))
                .observe(receive_capture);
            state.requested = true;
        }
    }
}

/// The target state within the target scene: the map's event gate, or a
/// story stage played from the map (`--capture-game-scene story`).
fn capture_gate_open(flow: &GameFlow) -> bool {
    match flow.capture_scene {
        Some(GameScene::Map) => flow.capture_map_gate.is_none() || flow.capture_map_reached,
        // `--capture-map-event 11184 --capture-map-updates N`: N updates
        // into the red ending (or its end, back at the title).
        Some(GameScene::Opening) if flow.capture_map_gate.is_some() => {
            flow.capture_map_reached
                || flow.capture_map_gate.is_some_and(|(_, n)| flow.capture_map_updates >= n)
        }
        Some(GameScene::Opening) => flow.map_save.is_some(),
        Some(GameScene::MapEditor) => flow.map_editor_settled,
        // `--capture-game-scene online`: once the ROSES_ONLINE_KEYS script
        // is used up (its `shot:` steps write the frames on the way; the
        // usual 100 s bound applies, so a headless script stays in the
        // menus and ends with `exit`).
        Some(GameScene::Online) => flow.online_script_done,
        _ => true,
    }
}

/// A connected capture may never approve an earlier scene or a stopped game.
/// The pending transition matters even without an explicit target: its old
/// framebuffer is about to be replaced and must not win the readback race.
fn capture_scene_ready(
    target: Option<GameScene>,
    current: GameScene,
    pending: Option<GameScene>,
    stopped: Option<&str>,
) -> Result<bool, String> {
    if let Some(reason) = stopped {
        return Err(format!(
            "Connected capture stopped before acceptance: {reason}"
        ));
    }
    Ok(pending.is_none() && target.is_none_or(|target| target == current))
}

fn reset_capture_scene(world: &mut bevy::ecs::world::World) {
    if let Some(bridge) = world.get_resource::<CaptureBridge>() {
        *bridge.0.lock().expect("Capture progress mutex poisoned") = RenderProgress::default();
    }
    if let Some(mut state) = world.get_resource_mut::<CaptureState>() {
        state.target = None;
        state.requested = false;
        state.settled_frames = 0;
    }
}

fn receive_capture(
    event: On<ScreenshotCaptured>,
    cloud_materials: Res<Assets<CloudMaterial>>,
    mut state: ResMut<CaptureState>,
    bridge: Res<CaptureBridge>,
    clouds: Option<Res<CloudAnimation>>,
    flow: Option<Res<GameFlow>>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.completed {
        return;
    }
    let result = (|| -> Result<(), String> {
        if let Some(flow) = &flow {
            if !capture_scene_ready(
                flow.capture_scene,
                flow.scene,
                flow.next,
                flow.stopped.as_deref(),
            )? {
                return Err("Connected scene changed before GPU readback completed".into());
            }
        }
        let mut progress = bridge.0.lock().map_err(|e| e.to_string())?;
        if let Some(error) = &progress.error {
            return Err(error.clone());
        }
        // The screenshot adds its own pipeline after the initial ready-frame
        // threshold. Its completion need not wait eight more frames, but every
        // source asset and material pipeline must still be ready.
        if !progress.scene_ready || !state.requested {
            return Err("Render assets or pipelines became unready during capture".into());
        }
        let image = event
            .image
            .clone()
            .try_into_dynamic()
            .map_err(|e| e.to_string())?;
        let pixels = image.to_rgba8();
        let stats = roses_game::capture::validate_capture_pixels(
            pixels.width(),
            pixels.height(),
            pixels.as_raw(),
        )?;
        let mut encoded = Cursor::new(Vec::new());
        image
            .write_to(
                &mut encoded,
                ImageFormat::Png
                    .as_image_crate_format()
                    .ok_or("PNG encoder unavailable")?,
            )
            .map_err(|e| format!("PNG encode failed: {e}"))?;
        // Only create the destination after the actual GPU readback passes.
        // create_new also closes the preflight check's filesystem race.
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&state.path)
            .map_err(|e| format!("Cannot create {}: {e}", state.path.display()))?;
        output
            .write_all(encoded.get_ref())
            .and_then(|_| output.sync_all())
            .map_err(|e| format!("Cannot finish {}: {e}", state.path.display()))?;
        progress.saved = true;
        println!(
            "GPU capture saved to {}\n{}",
            state.path.display(),
            serde_json::to_string_pretty(&stats).map_err(|e| e.to_string())?
        );
        if let Some(clouds) = clouds.filter(|c| c.enabled) {
            let fade_alpha = clouds
                .material
                .as_ref()
                .and_then(|h| cloud_materials.get(h))
                .map_or(128, |m| m.settings.fade_alpha.x.clamp(0, 128) as u8);
            if fade_alpha < 128 {
                // The CPU mirror has no Stonehenge world beneath the fade.
                println!(
                    "Cloud GPU/CPU band check skipped: native world visible (fade alpha {fade_alpha})"
                );
            } else {
                let report =
                    cloud_render::compare_band_with_cpu(&clouds, fade_alpha, pixels.as_raw())?;
                println!(
                    "Cloud GPU/CPU band check (same native model, not original parity):\n{}",
                    serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
                );
            }
        }
        Ok(())
    })();
    state.completed = true;
    match result {
        Ok(()) => {
            exit.write(AppExit::Success);
        }
        Err(error) => {
            eprintln!("GPU capture failed: {error}");
            exit.write(AppExit::error());
        }
    }
}

/// Captured draw matrix at the settled Simon pose, recomposed per native pose.
#[derive(Component)]
struct WorldDraw {
    captured: camera::Mat4,
    /// Part of the story scene-0 world (else the opening's Stonehenge).
    story: bool,
    /// Index into the world's `Scene::animations` when the draw's model
    /// animates (the sky): `captured` is then replaced by the frame's clip.
    animation: Option<usize>,
    /// Source positions and triangle indices: the beam's depth test
    /// (`story_beam`).
    occluder: std::sync::Arc<(Vec<[f32; 3]>, Vec<u32>)>,
}
/// The Stonehenge story events' world (game part `story-scene`), if any.
#[derive(Resource)]
struct StoryScene(LoadedScene);
/// Native Stonehenge camera state (see camera.rs for the traced sources).
#[derive(Resource)]
struct WorldRig {
    rig: camera::Rig,
    /// 0x1ED53F6: phase at checkpoint load is not reproduced (it only depends
    /// on how long the history pages were read); it spins 0x300 per update.
    history_angle: i16,
    /// 0x1ED5439 camera pan, -100..=100; the captured pose is 100.
    simon_pan: i8,
    /// Level of the running handoff (0x1ED0064).
    level: f64,
    last: Option<Pose>,
    stage: String,
    /// 0x1ED53F6 during event 11184 (mode 1 spin, −0x300 per update).
    story_angle: i16,
    /// Which world the draws were last shown for (visibility switch).
    showing_story: Option<bool>,
    /// Each world's animated models (the sky) and their frame counters
    /// (object +0x19C), [opening, story]. The opening's starts at 1.0 when
    /// the Simon scene (10040) begins, the story's when scene 0 is shown
    /// [H: the objects' creation update is taken as the stage's first].
    animations: [Vec<roses_game::Animation>; 2],
    animation_frames: [Vec<u32>; 2],
    animation_world: Option<World>,
}
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct CapturedMaterial {
    #[uniform(0)]
    clip_from_source_gs: Mat4,
    #[texture(1)]
    #[sampler(2)]
    texture: Handle<Image>,
    /// x = 1 for a blended (second DMA chain) draw: GS ALPHA 0x44,
    /// (Cs − Cd)·As + Cd with As = texel alpha × vertex alpha / 128.
    #[uniform(3)]
    params: Vec4,
}
impl Material for CapturedMaterial {
    fn alpha_mode(&self) -> AlphaMode {
        if self.params.x > 0.5 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        }
    }
    fn vertex_shader() -> ShaderRef {
        "captured.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "captured.wgsl".into()
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
    ) -> std::result::Result<(), SpecializedMeshPipelineError> {
        descriptor.label = Some(CAPTURE_PIPELINE.into());
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
        ])?];
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = &mut descriptor.depth_stencil {
            depth.depth_compare = Some(CompareFunction::GreaterEqual);
            depth.depth_write_enabled = Some(true);
        }
        Ok(())
    }
}
#[derive(Resource)]
struct InputScene(LoadedScene);

#[derive(Resource)]
struct OpeningState {
    preview: OpeningPreview,
    overlay: Option<Handle<Image>>,
    blocked: Option<String>,
    simon_target: Option<Handle<Image>>,
    /// The story scene-0 draws exist (game part `story-scene`).
    story_world: bool,
    /// Black canvas for overlay-only stages (title, main menu).
    black_target: Option<Handle<Image>>,
    composite: Option<Handle<CompositeMaterial>>,
    /// Source-timed scene handoff in progress; input waits until it ends.
    transition: Option<Transition>,
}
enum Transition {
    /// Event 10020 after the last page: cloud exit and navy overlay.
    HistoryExit(HistoryExit),
    /// Event 10040 opening: black overlay fading off the Simon scene.
    SimonEntry(SimonEntry),
}
impl OpeningState {
    fn world(&self) -> World {
        match self.transition {
            Some(Transition::HistoryExit(_)) => World::Clouds,
            Some(Transition::SimonEntry(_)) => World::Simon,
            None => stage_world(self.preview.model.stage()),
        }
    }
    fn caption(&self) -> String {
        match (&self.blocked, &self.transition) {
            (Some(message), _) => {
                format!("Roses Bevy preview — unsupported continuation: {message}")
            }
            (None, Some(Transition::HistoryExit(_))) => {
                "Roses Bevy preview · history exit · cloud exit and navy fade; history camera fly-in not implemented (black) · music, no SFX".into()
            }
            (None, Some(Transition::SimonEntry(_))) => {
                "Roses Bevy preview · Simon entry · black fade, camera settle, white flash and light beam (c401, with the CMODULE import) · music".into()
            }
            (None, None) => self.preview.caption(),
        }
    }
}
#[derive(Component)]
struct PresentationCamera;

/// Which part of the connected game (`--game`) is on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GameScene {
    Opening,
    Map,
    Duel,
    /// The reward reels after a won duel (mode 0xF).
    SlotMachine,
    /// A practice lesson (practice choice "Yes": the scripted tutorial on
    /// the duel board, FLOW.md Y1-Y6).
    Tutorial,
    /// The deck editor (mode 6) pushed by the map's SYSTEM icon 0.
    DeckEditor,
    /// Custom Duel's set-up screen (title mode 9, `custom_duel_render`).
    CustomDuel,
    /// The map editor (mode 0xD) pushed by Custom Duel's Edit Map
    /// (`map_editor_render`).
    MapEditor,
    /// Trade (title mode 8, `trade_render`).
    Trade,
    /// Online Play (a port addition on the title, `online_render`).
    Online,
}

/// The scripted tutorial's progress between its lesson scenes.
struct TutorialFlow {
    /// The story stage of event 10060 the series returns to.
    resume: String,
    /// The lesson to start next.
    lesson: Lesson,
    /// The previous lesson's end (its series counter), after the first.
    from: Option<LessonEnd>,
    /// `--tutorial-auto`: the operator pages the messages.
    auto: bool,
    /// `--tutorial-ticks`: updates run before the first frame of the first
    /// lesson (captures of a given poll).
    ticks: u32,
    /// Started from the map's tutorial menu (SYSTEM icon 3): the lesson
    /// plays alone and its end returns to the map with the card block as
    /// it was (0x23B710 / 0x21D14C).
    to_map: bool,
}
/// The connected game: one `roses import-game` folder, the scene on screen
/// and the progress the story made (starter deck, side).
#[derive(Resource)]
struct GameFlow {
    import: roses_game::game::GameImport,
    /// The last campaign duel's leaders (the slot machine's portraits).
    duel_leaders: Option<[u16; 2]>,
    scene: GameScene,
    next: Option<GameScene>,
    /// Optional connected GPU checkpoint; only map/duel are accepted by CLI.
    capture_scene: Option<GameScene>,
    campaign: roses_client::CampaignState,
    /// Deck shuffle seed for the next duel (wall clock at start: the
    /// original's frame-timed global RNG is not reproduced).
    seed: u64,
    /// The shared 64-bit LCG state (0x19AE88) after the duel: the result
    /// screens, the reel shuffle, the slips and the card grants draw from it
    /// and the main loop's two calls per frame are added while those screens
    /// run. It starts from `seed` (the deck shuffle's own draws are not
    /// carried over) and the original's frame-exact cadence before, between
    /// and after those screens is not reproduced, so the prizes are the
    /// model's, not a prediction of the game's.
    rng: u64,
    /// The player's persistent card block: made when the story chooses the
    /// starter deck (`game::new_player_block`, an assumed new-game layout),
    /// the single source of the player's deck (its current deck record
    /// builds every campaign duel's player side), edited by the deck
    /// editor, changed by the result screens and the reels.
    player_block: Option<roses_core::card_block::CardBlock>,
    /// The two blocks of the duel being played (player, computer) as the
    /// result screens left them, kept for the slot machine. The computer's
    /// block is made per duel from its static deck record and dropped
    /// afterwards (whether the original keeps BLK(1) between duels is not
    /// traced).
    blocks: Option<[roses_core::card_block::CardBlock; 2]>,
    /// The campaign save the map held when the deck editor was pushed; the
    /// map resumes from it on return.
    map_save: Option<roses_client::campaign_map::CampaignSave>,
    /// `--capture-map-event E --capture-map-updates N`: a map capture waits
    /// until the map spent N updates at event E (counted here).
    capture_map_gate: Option<(i16, u32)>,
    capture_map_updates: u32,
    capture_map_reached: bool,
    /// Scripted pad words for the deck editor (`--editor-replay`), one per
    /// 25 Hz update, consumed before the keyboard (test aid).
    editor_replay: std::collections::VecDeque<u16>,
    /// `--exit-after-editor`: quit once the map is re-entered from the deck
    /// editor (test aid).
    exit_after_editor: bool,
    /// `--exit-at-duel`: quit once a campaign duel scene has been entered
    /// (test aid: the duel's player side is logged by then).
    exit_at_duel: bool,
    /// Scripted pad words for the result screens and the reels
    /// (`--end-replay`), one per update, consumed before the keyboard.
    end_replay: std::collections::VecDeque<u16>,
    /// `--exit-at-map`: quit once the map is entered after a duel (test aid).
    exit_at_map: bool,
    /// Scripted pads for the campaign map (`--map-replay`), one per 25 Hz
    /// update, delivered to the map scene as synthetic key presses (the
    /// scene reads the keyboard only): a test aid for scripted runs.
    map_replay: std::collections::VecDeque<roses_client::campaign_map::Pad>,
    /// The pad the last synthetic key events left held.
    map_replay_held: roses_client::campaign_map::Pad,
    /// `--game-computer-player`: the ported controller plays the player's
    /// side too (test aid for scripted runs through a won duel).
    computer_player: bool,
    /// The reel data a won duel handed over, until the reel scene takes it,
    /// with the winner (D50: the side whose block receives the prizes).
    reels: Option<(u8, roses_core::slot_machine::ReelData)>,
    /// Set once a boundary stops the game; shown in the title.
    stopped: Option<String>,
    /// The map's duel request being played, and the last duel's result.
    request: Option<roses_client::campaign_map::DuelRequest>,
    won: Option<bool>,
    /// The scripted tutorial in progress (practice "Yes").
    tutorial: Option<TutorialFlow>,
    /// The story model while the tutorial runs (its committed name and
    /// campaign state come back with it).
    parked_preview: Option<OpeningPreview>,
    /// `--exit-at-story`: quit once the story resumes after the tutorial
    /// (test aid).
    exit_at_story: bool,
    /// The campaign map fired a story event: the story scene shows this
    /// stage (`map-story-<event>`) and the map resumes from `map_save` at
    /// the event's map continuation (`StoryEnd::MapEvent`).
    story_stage: Option<String>,
    /// The story event whose pages just handed back to the map
    /// (`MapEntry::AfterStory`: 11162/12132 open their letterbox over the
    /// map first).
    story_exit: Option<i16>,
    /// `--story-auto`: Cross every few updates on map story stages (test aid).
    story_auto: bool,
    story_auto_ticks: u32,
    /// The story just chose the starter deck: make the block on the next
    /// exclusive-world pass (`game_block_watch`).
    needs_block: bool,
    /// 0x2DB828 (with its direction, 0x2DB904 bits 2-3) as the last mode that
    /// steps it left it: boot sets 0 rising (0x1002D0 -> 0x1AE2A0 -> 0x1AE640),
    /// the duel-type mode inits reset it the same way, the duel, the deck
    /// editor, Trade and Custom Duel step it and the map never writes it, so the
    /// editor starts from the previous stepper's value [F; PCSX2 modes/ui3
    /// blink-map-editor.json]. Custom Duel's menu does not step it [F: no
    /// writer in 0x1C6BC0..0x1CD750; interpreter `modes/custom-duel/tools/
    /// blink_cd.py`: 84 menu frames leave both words as they were]; the
    /// writer 0x1CD9E8 once listed for it is the map editor's update.
    scroll_word: roses_client::duel_hud::FlashWave,
    /// 0x2DB826 (direction 0x2DB904 bits 0-1), the second scroll word,
    /// carried the same way (boot / duel inits reset it to 0 rising; the
    /// duel, deck editor, Trade and map editor step it ±0x200).
    scroll_p1: roses_client::duel_hud::FlashWave,
    /// d63 (0x2DB903): non-zero holds 0x2DB826 rising in every stepper; the
    /// map editor sets 0x3C on a move and never clears it, its init and the
    /// duel-type inits zero it [F 0x1CD88C, 0x1CDC94, 0x1AEBD8].
    d63: u8,
    /// `--capture-game-scene map-editor`: the map editor used up its
    /// `ROSES_MAP_EDITOR_REPLAY` (test aid).
    map_editor_settled: bool,
    /// Online Play's key script is used up (`--capture-game-scene online`).
    online_script_done: bool,
}

/// Make the player's card block right after the story's deck choice.
fn game_block_watch(world: &mut bevy::ecs::world::World) {
    let Some(flow) = world.get_resource::<GameFlow>() else {
        return;
    };
    if !flow.needs_block || flow.stopped.is_some() {
        return;
    }
    world.resource_mut::<GameFlow>().needs_block = false;
    if let Err(message) = ensure_player_block(world) {
        eprintln!("Game boundary: {message}");
        set_title(world, &format!("Roses - stopped: {message}"));
        world.resource_mut::<GameFlow>().stopped = Some(message);
    }
}
/// Opening systems run in the standalone preview and while the game shows it.
fn opening_active(flow: Option<Res<GameFlow>>) -> bool {
    flow.is_none_or(|f| f.scene == GameScene::Opening)
}

fn set_title(world: &mut bevy::ecs::world::World, title: &str) {
    let mut windows = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
    for mut window in windows.iter_mut(world) {
        window.title = title.to_owned();
    }
}

/// Switch the game's scene at a frame boundary: despawn what the old scene
/// drew (cameras and meshes), drop its state, then build the new one.
fn game_switch(world: &mut bevy::ecs::world::World) {
    // The original's mode change fades out over 60 updates first.
    if mode_fade::hold(world) {
        return;
    }
    let Some(next) = world
        .get_resource_mut::<GameFlow>()
        .and_then(|mut f| f.next.take())
    else {
        return;
    };
    // Discard readiness for the old scene's assets before registering the
    // new scene. Otherwise a despawned opening can stall a map capture.
    reset_capture_scene(world);
    let doomed: Vec<Entity> = world
        .query_filtered::<Entity, (Or<(With<Camera>, With<Mesh3d>)>, Without<mode_fade::Overlay>)>()
        .iter(world)
        .collect();
    for entity in doomed {
        world.despawn(entity);
    }
    let previous = world.resource::<GameFlow>().scene;
    match previous {
        GameScene::Opening => {
            if let Some(opening) = world.remove_resource::<OpeningState>() {
                world.resource_mut::<GameFlow>().parked_preview = Some(opening.preview);
            }
            world.remove_resource::<CloudAnimation>();
            world.remove_resource::<WorldRig>();
        }
        GameScene::Map => campaign_map_render::leave_map_scene(world),
        GameScene::Tutorial => {
            if let Some(wave) = duel_board_render::scroll_word(world) {
                world.resource_mut::<GameFlow>().scroll_word = wave;
            }
            if let Some((p1, d63)) = duel_board_render::scroll_p1(world) {
                let mut flow = world.resource_mut::<GameFlow>();
                flow.scroll_p1 = p1;
                flow.d63 = d63;
            }
            duel_board_render::leave_play_scene(world)
        }
        GameScene::Duel => {
            if let Some(wave) = duel_board_render::scroll_word(world) {
                world.resource_mut::<GameFlow>().scroll_word = wave;
            }
            if let Some((p1, d63)) = duel_board_render::scroll_p1(world) {
                let mut flow = world.resource_mut::<GameFlow>();
                flow.scroll_p1 = p1;
                flow.d63 = d63;
            }
            if let Some((blocks, rng, replay)) = duel_board_render::take_end_state(world) {
                let mut flow = world.resource_mut::<GameFlow>();
                let side = player_side(&flow);
                flow.player_block = Some(blocks[side].clone());
                flow.blocks = Some(blocks);
                flow.rng = rng;
                flow.end_replay = replay;
            }
            duel_board_render::leave_play_scene(world);
        }
        GameScene::SlotMachine => {
            if let Some(result) = slot_machine_render::leave_slot_scene(world) {
                let mut flow = world.resource_mut::<GameFlow>();
                let side = player_side(&flow);
                flow.player_block = Some(result.blocks[side].clone());
                flow.blocks = Some(result.blocks);
                flow.rng = result.rng;
                flow.end_replay = result.replay;
                eprintln!(
                    "Game: {} prize(s) granted into the player's card block; recent list {:?}",
                    result.prizes.len(),
                    flow.player_block.as_ref().map(|b| b.recent)
                );
            }
        }
        GameScene::Trade => {
            if let Some(wave) = trade_render::scroll_word(world) {
                world.resource_mut::<GameFlow>().scroll_word = wave;
            }
            if let Some(p1) = trade_render::scroll_p1(world) {
                world.resource_mut::<GameFlow>().scroll_p1 = p1;
            }
        }
        GameScene::CustomDuel | GameScene::Online => {}
        GameScene::MapEditor => {
            if let Some((save, wave, p1, d63)) = map_editor_render::leave_scene(world) {
                let mut flow = world.resource_mut::<GameFlow>();
                flow.scroll_word = wave;
                flow.scroll_p1 = p1;
                flow.d63 = d63;
                if let (Some((map, cells)), Some(block)) = (save, flow.player_block.as_mut()) {
                    // 0x1CDA64: the editor writes the pad side's block (RAM).
                    block.set_own_map(map, &cells);
                }
            }
        }
        GameScene::DeckEditor if online_render::building(world) => {
            if let Some(result) = deck_editor_render::leave_editor_scene(world) {
                online_render::builder_closed(world, result);
            }
        }
        GameScene::DeckEditor => {
            if let Some(result) = deck_editor_render::leave_editor_scene(world) {
                let mut flow = world.resource_mut::<GameFlow>();
                flow.player_block = Some(result.block);
                flow.rng = result.rng;
                flow.editor_replay = result.replay;
                flow.scroll_word = result.blink;
                flow.scroll_p1 = result.blink_p1;
                if let Some(save) = flow.map_save.as_mut() {
                    save.duel_count = result.duel_counter;
                }
                eprintln!(
                    "Game: the deck editor closed after {} updates; current deck {} cards, leader {}, DC {}",
                    result.frames,
                    result.deck.count(),
                    result.deck.leader,
                    result.deck.cost
                );
            }
        }
    }
    world.resource_mut::<GameFlow>().scene = next;
    let result = match next {
        GameScene::Map => enter_map(world, previous),
        GameScene::Duel => enter_first_duel(world),
        GameScene::SlotMachine => enter_slot_machine(world),
        GameScene::Tutorial => enter_tutorial(world),
        GameScene::Opening => resume_opening(world),
        GameScene::DeckEditor => enter_deck_editor(world),
        GameScene::CustomDuel => custom_duel_render::enter_scene(world),
        GameScene::MapEditor => match custom_duel_render::edit_list(world) {
            Some((top, count)) => map_editor_render::enter_scene(world, top, count),
            None => Err("the map editor is entered from Custom Duel only".into()),
        },
        GameScene::Trade => trade_render::enter_scene(world),
        GameScene::Online => online_render::enter_scene(world),
    };
    if result.is_ok() {
        eprintln!("Game: {previous:?} -> {next:?}");
        if next == GameScene::Duel && world.resource::<GameFlow>().exit_at_duel {
            eprintln!("Game: --exit-at-duel: the duel scene was entered; exiting");
            world.write_message(AppExit::Success);
        }
        if next == GameScene::Map && previous == GameScene::DeckEditor {
            let flow = world.resource::<GameFlow>();
            if flow.exit_after_editor {
                eprintln!(
                    "Game: --exit-after-editor: the map resumed after the deck editor; exiting"
                );
                world.write_message(AppExit::Success);
            }
        }
        if next == GameScene::Opening && previous == GameScene::Tutorial {
            let flow = world.resource::<GameFlow>();
            if flow.exit_at_story {
                eprintln!("Game: --exit-at-story: the story resumed after the tutorial; exiting");
                world.write_message(AppExit::Success);
            }
        }
        if next == GameScene::Map
            && previous != GameScene::Opening
            && previous != GameScene::DeckEditor
        {
            let flow = world.resource::<GameFlow>();
            if flow.exit_at_map {
                eprintln!("Game: --exit-at-map: the map was entered after the duel; exiting");
                world.write_message(AppExit::Success);
            }
        }
    }
    if let Err(message) = result {
        eprintln!("Game boundary: {message}");
        set_title(world, &format!("Roses - stopped: {message}"));
        world.resource_mut::<GameFlow>().stopped = Some(message);
    }
}

/// The player's card block exists from the moment the story chooses the
/// starter deck (the original's new-game block is written by the deck
/// choice; its layout is assumed, see `game::new_player_block`). Also run
/// when a replay skipped straight to the map.
fn ensure_player_block(world: &mut bevy::ecs::world::World) -> Result<(), String> {
    use roses_game::{duel_play, game};
    let flow = world.resource::<GameFlow>();
    if flow.player_block.is_some() {
        return Ok(());
    }
    let Some(chosen) = flow.campaign.starter_deck.clone() else {
        return Ok(());
    };
    let executable = duel_play::load_executable(&flow.import.duel)?;
    let decks = roses_assets::parse_pal_decks(&executable)
        .map_err(|e| e.to_string())?
        .decks;
    let thresholds =
        roses_assets::parse_pal_rank_thresholds(&executable).map_err(|e| e.to_string())?;
    let mut rng = flow.rng;
    let block = game::new_player_block(&chosen, &decks, &executable, &thresholds, &mut rng)?;
    let record = roses_core::deck_editor::deck_record(&block, 0);
    eprintln!(
        "Game: the player's card block is made from starter deck {:#x} (leader {}, {} cards, DC {}; assumed new-game layout)",
        chosen.record,
        record.leader,
        record.count(),
        record.cost
    );
    let mut flow = world.resource_mut::<GameFlow>();
    flow.player_block = Some(block);
    flow.rng = rng;
    Ok(())
}

/// The SYSTEM icon rectangles of the map's HUD from the import's executable.
fn system_icons(world: &bevy::ecs::world::World) -> Vec<[i16; 4]> {
    use roses_game::duel_play;
    let flow = world.resource::<GameFlow>();
    match duel_play::load_executable(&flow.import.duel)
        .and_then(|exe| roses_client::campaign_map::system_icon_rects(&exe))
    {
        Ok(rects) => rects,
        Err(e) => {
            eprintln!("Game: SYSTEM icons unavailable ({e})");
            Vec::new()
        }
    }
}

/// The side the player's pad plays in the map's duel (p2 = SAVE[0]).
fn player_side(flow: &GameFlow) -> usize {
    flow.request.as_ref().map_or(0, |r| usize::from(r.params[1] == 1))
}

/// The story's first map event: 100 (the voyage to Brest, or the White
/// Rose side's Windsor reveal after 12000) starts the campaign map.
fn first_map_save(
    event: u16,
    side: Option<roses_client::CampaignSide>,
) -> Result<roses_client::campaign_map::CampaignSave, String> {
    use roses_client::campaign_map::{CampaignSave, EVENT_FIRST_VOYAGE};
    if i16::try_from(event) != Ok(EVENT_FIRST_VOYAGE) {
        return Err(format!(
            "the story continues at map event {event} without a campaign map state (only {EVENT_FIRST_VOYAGE} starts one)"
        ));
    }
    Ok(match side {
        Some(roses_client::CampaignSide::Yorkists) => CampaignSave::white_start(),
        _ => CampaignSave::before_landing(),
    })
}

/// The campaign map: event 0 after the story, the duel result after a
/// duel, or the save the map left when the deck editor was pushed.
fn enter_map(world: &mut bevy::ecs::world::World, previous: GameScene) -> Result<(), String> {
    use campaign_map_render::MapEntry;
    ensure_player_block(world)?;
    let icons = system_icons(world);
    let deck_costs = map_deck_costs(world);
    let hud_source = map_hud_source(world);
    let mut flow = world.resource_mut::<GameFlow>();
    let manifest = flow.import.campaign_map.join("manifest.json");
    let story_exit = flow.story_exit.take();
    let entry = match (previous, flow.won, flow.map_save.take()) {
        (GameScene::Opening, _, Some(save)) if story_exit.is_some() => {
            MapEntry::AfterStory(save, story_exit.unwrap_or(0))
        }
        (GameScene::DeckEditor | GameScene::Opening | GameScene::Tutorial, _, Some(save)) => {
            MapEntry::Resume(save)
        }
        (GameScene::Opening, _, _) | (_, None, _) => MapEntry::EventZero,
        // The save the map left at the duel request keeps every node's
        // state, the counters and C9 (audit RED-ROUTE-AUDIT.md 1).
        (_, Some(won), Some(save)) => MapEntry::AfterDuelFrom(save, won),
        (_, Some(won), None) => MapEntry::AfterDuel(won),
    };
    // The map's track 4 (0x1AB440(4, ..) on mode entry; after a duel it
    // restarts) [F UI-SOUNDS.md 3].
    let keep = !matches!(entry, MapEntry::AfterDuel(_) | MapEntry::AfterDuelFrom(..));
    // The story events the map plays (11124, 11149) never left the map mode
    // in the original: the story's track runs on until their own requests.
    let story_window = matches!(&entry, MapEntry::Resume(save) | MapEntry::AfterStory(save, _) if roses_client::campaign_map::MapModel::is_window_event(save.event));
    if let Some(mut sound) = world.get_resource_mut::<duel_board_render::DuelSound>().filter(|_| !story_window) {
        sound.cue(roses_game::duel_audio::AudioCue::Music { track: 4, keep });
    }
    let bits = world
        .resource::<GameFlow>()
        .player_block
        .as_ref()
        .map_or(roses_client::game_options::DEFAULT_BITS, roses_client::game_options::bits);
    campaign_map_render::enter_map_scene(world, &manifest, entry, icons, deck_costs, hud_source)?;
    // The OPTION panel starts from the player's block (0x1E5270).
    campaign_map_render::map_set_option_bits(world, bits);
    Ok(())
}

/// 0x217200: Sound Stereo / Mono into the mixer.
fn apply_sound_option(world: &mut bevy::ecs::world::World, bits: u8) {
    let mono = !roses_client::game_options::stereo(bits);
    if let Some(mixer) = world.get_resource::<duel_board_render::DuelSound>().and_then(|s| s.mixer()) {
        if let Ok(mut m) = mixer.lock() {
            m.set_mono(mono);
        }
    }
}

/// SYSTEM -> SAVE on the map: the memory-card file (the player's block and
/// SAVE, `memory_card_store`) the save dialog writes to MEMORY CARD slot 1.
fn map_card_file(
    world: &mut bevy::ecs::world::World,
) -> Result<(Vec<u8>, roses_game::memory_card_store::VirtualCard), String> {
    use roses_game::memory_card_store as store;
    let map = campaign_map_render::map_save(world).ok_or("the map has no save")?;
    let flow = world.resource::<GameFlow>();
    let block = flow.player_block.as_ref().ok_or("no card block yet")?;
    let name = flow
        .parked_preview
        .as_ref()
        .and_then(|p| p.model.player_name().cloned());
    let codes = flow
        .campaign
        .starter_deck
        .as_ref()
        .map(|c| c.name_codes.clone())
        .unwrap_or_default();
    let mut card = store::VirtualCard::slot1(&flow.import.root);
    let file = store::build_file(block, &map, &codes, card.current_save().as_ref())?;
    card.sidecar = Some(store::PortSidecar::new(flow.campaign.clone(), name));
    Ok((file, card))
}

/// The title's Continue: the memory-card load dialog (op 0) over the menu.
#[derive(Resource)]
struct TitleLoad {
    dialog: roses_client::memory_card::CardDialog,
    card: roses_game::memory_card_store::VirtualCard,
    /// MEMORY CARD slot 2 for the title's Custom Duel (op 3).
    slot2: Option<roses_game::memory_card_store::VirtualCard>,
    boxes: Option<roses_game::memory_card_store::BoxRenderer>,
    /// Presses gathered by `update_opening` since the last update.
    pad: roses_client::memory_card::CardPad,
    drawn: Option<(roses_client::memory_card::CardBox, u32)>,
}

impl TitleLoad {
    fn start(flow: &GameFlow) -> Self {
        use roses_game::memory_card_store as store;
        let card = store::VirtualCard::slot1(&flow.import.root);
        match store::migrate_interim(&flow.import.root, &card) {
            Ok(true) => eprintln!("Game: the interim save was converted to the memory card ({})", card.data_path().display()),
            Ok(false) => {}
            Err(e) => eprintln!("Game: the interim save could not be converted: {e}"),
        }
        let boxes = store::BoxRenderer::from_duel_import(&flow.import.duel)
            .map_err(|e| eprintln!("Game: the memory-card boxes cannot be drawn ({e})"))
            .ok();
        Self {
            dialog: roses_client::memory_card::CardDialog::load(),
            card,
            slot2: None,
            boxes,
            pad: Default::default(),
            drawn: None,
        }
    }
}

impl TitleLoad {
    /// The title's Custom Duel: op 3 (slot 1 loaded, slot 2 checked).
    fn custom_duel(flow: &GameFlow) -> Self {
        let mut load = Self::start(flow);
        load.dialog = roses_client::memory_card::CardDialog::custom_duel();
        load.slot2 = Some(roses_game::memory_card_store::VirtualCard::slot2(&flow.import.root));
        load
    }

    /// The title's Trade: op 4 (both slots loaded).
    fn trade(flow: &GameFlow) -> Self {
        let mut load = Self::start(flow);
        load.dialog = roses_client::memory_card::CardDialog::trade_load();
        load.slot2 = Some(roses_game::memory_card_store::VirtualCard::slot2(&flow.import.root));
        load
    }
}

/// One update of the title's load dialog (0x1ABB20 at the top of the
/// title update 0x1AD050): its box over the menu, SE, and on a load the
/// map (story init with param[11] = 1 resumes at the saved location).
fn title_load_tick(world: &mut bevy::ecs::world::World) {
    use roses_game::memory_card_store as store;
    use roses_client::memory_card::CardOutcome;
    let Some(mut load) = world.remove_resource::<TitleLoad>() else {
        return;
    };
    let mut pad = std::mem::take(&mut load.pad);
    // `--story-auto` (test aid) also acknowledges the load boxes with
    // Cross every eighth update, so scripted runs can Continue.
    if world.get_resource::<GameFlow>().is_some_and(|f| f.story_auto) && load.dialog.counter % 8 == 7 {
        pad.cross = true;
    }
    let outcome = match load.slot2.as_mut() {
        Some(slot2) => load.dialog.update_slots(pad, &mut load.card, slot2),
        None => load.dialog.update(pad, &mut load.card),
    };
    if let Some(mut sound) = world.get_resource_mut::<duel_board_render::DuelSound>() {
        for id in &load.dialog.sfx {
            sound.cue(roses_game::duel_audio::AudioCue::Se { slot: 0, id: *id as u8, skip: false });
        }
    }
    let shown = load.dialog.overlay().map(|b| (b, load.dialog.counter / 5));
    if shown != load.drawn {
        load.drawn = shown;
        let layer = match (shown, load.boxes.as_mut()) {
            (Some((b, _)), Some(r)) => Some(r.layer(b, load.dialog.counter)),
            _ => None,
        };
        world.resource_scope(|world, opening: Mut<OpeningState>| {
            let Ok(mut base) = opening.preview.overlay() else {
                return;
            };
            if let Some(layer) = layer {
                store::blend_layer(&mut base.rgba, base.width as usize, base.height as usize, &layer);
            }
            if let Some(handle) = opening.overlay.clone() {
                if let Some(mut image) = world.resource_mut::<Assets<Image>>().get_mut(&handle) {
                    *image = ui_image(base);
                }
            }
        });
    }
    match outcome {
        CardOutcome::Running => world.insert_resource(load),
        CardOutcome::Loaded(bytes) => match store::restore(&bytes) {
            Ok(restored) => {
                let (campaign, name) = store::campaign_after_load(&load.card, &restored.record);
                let map = restored.record.map.clone();
                eprintln!(
                    "Game: Continue loaded {} (event {}, location {}, {} wins, {} losses)",
                    load.card.data_path().display(),
                    map.event,
                    map.location,
                    map.wins,
                    map.losses
                );
                // Story init 0x21C1A4 with param[11] = 1 (`ending::continue_target`).
                use roses_client::ending::{continue_target, ContinueTarget, SIDE_CHOICE_STAGE, STAGE_AFTER_ENDING};
                let target = continue_target(&restored.record);
                if target == ContinueTarget::FreeDuel {
                    // W+18C: the map's set-up 0x21C254 (`MapModel::resume`).
                    eprintln!("Game: Continue: both routes are cleared (SAVE[1], SAVE[2]): the free-duel map");
                }
                {
                    let model = &mut world.resource_mut::<OpeningState>().into_inner().preview.model;
                    model.restore_progress(name, campaign.clone());
                    if let ContinueTarget::OtherRoute { side_cursor, .. } = &target {
                        model.preset_selection(SIDE_CHOICE_STAGE, usize::from(*side_cursor));
                        // One route is cleared (SAVE[1] or SAVE[2]): the
                        // cursor is locked to the other side (FLOW C5).
                        model.lock_selection(SIDE_CHOICE_STAGE);
                    }
                }
                let mut restored = restored;
                // Files the port wrote before the options were modelled have
                // +0xFF9C bits 2..4 clear where 0x213DB0 sets them: a port
                // file (its sidecar present) with all three off gets them.
                if roses_client::game_options::bits(&restored.block) == 0 && load.card.read_sidecar().is_some() {
                    roses_client::game_options::set_bits(&mut restored.block, roses_client::game_options::DEFAULT_BITS);
                }
                // 0x1AD210: the title applies block 0's sound option.
                apply_sound_option(world, roses_client::game_options::bits(&restored.block));
                let mut flow = world.resource_mut::<GameFlow>();
                flow.campaign = campaign;
                flow.player_block = Some(restored.block);
                flow.needs_block = false;
                flow.won = None;
                match target {
                    ContinueTarget::OtherRoute { save, side_cursor, .. } => {
                        // SAVE+0x3C = 0 (saved by the ending): 0x21C734 starts
                        // the other route's story at 10090 with the side
                        // choice's cursor on the other side.
                        eprintln!(
                            "Game: Continue after the ending: event {} ({STAGE_AFTER_ENDING}), node states cleared, side choice cursor {side_cursor}",
                            save.event
                        );
                        flow.map_save = Some(save);
                        flow.story_stage = Some(STAGE_AFTER_ENDING.into());
                        flow.next = Some(GameScene::Opening);
                    }
                    _ => {
                        flow.map_save = Some(map);
                        flow.next = Some(GameScene::Map);
                    }
                }
            }
            Err(e) => eprintln!("Game: Continue: the loaded file is unusable: {e}"),
        },
        CardOutcome::CustomDuel { slot1, result, slot2 } => {
            // 0x1AD1xx: param[9] = (result >> 8) - 1, push mode 9.
            if let Err(message) = custom_duel_render::title_custom_duel(world, slot1, result, slot2) {
                eprintln!("Game boundary: {message}");
            }
        }
        CardOutcome::Trade { slot1, slot2 } => {
            // Op 4 returned 1: mode 8 (track 41).
            if let Err(message) = trade_render::title_trade(world, slot1, slot2) {
                eprintln!("Game boundary: {message}");
            }
        }
        CardOutcome::LoadFailed | CardOutcome::Closed | CardOutcome::Saved | CardOutcome::NotSaved => {
            if let Some(e) = &load.dialog.error {
                eprintln!("Game: Continue: {e}");
            }
        }
    }
}

/// The map HUD's inputs from the duel import and the player's block.
fn map_hud_source(world: &bevy::ecs::world::World) -> Option<campaign_map_render::MapHudSource> {
    use roses_game::{campaign_map_hud, duel_play};
    let flow = world.resource::<GameFlow>();
    let block = flow.player_block.as_ref()?;
    match duel_play::load_with_end(&flow.import.duel, flow.import.duel_end.as_deref()) {
        Ok(data) => Some(campaign_map_render::MapHudSource {
            texanm: None,
            models_manifest: Some(flow.import.duel_models_manifest.clone()),
            opponents: roses_assets::parse_pal_decks(&data.executable)
                .map(|t| {
                    t.decks
                        .iter()
                        .enumerate()
                        .skip(roses_game::game::OPPONENT_DECK_RECORD_BASE)
                        .filter_map(|(record, deck)| {
                            let opponent = u8::try_from(
                                record - roses_game::game::OPPONENT_DECK_RECORD_BASE,
                            )
                            .ok()?;
                            Some((
                                opponent,
                                campaign_map_hud::OpponentHud {
                                    leader: deck.leader_card,
                                    leader_rank: u16::from(deck.leader_rank_raw),
                                },
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            executable: data.executable,
            messages: data.messages,
            texsys: data.texsys,
            picmini: data.picmini,
            deck: campaign_map_hud::deck_hud(block),
            player_name: flow
                .campaign
                .starter_deck
                .as_ref()
                .map(|c| c.name_codes.clone())
                .unwrap_or_default(),
        }),
        Err(e) => {
            eprintln!("Game: the map HUD's duel import is unavailable: {e}");
            None
        }
    }
}

/// The DC check's deck costs (player's current deck, every opponent's
/// static record); `None` (check off, reported) when the import lacks them.
fn map_deck_costs(
    world: &bevy::ecs::world::World,
) -> Option<(u16, std::collections::BTreeMap<u8, u16>)> {
    use roses_game::{duel_play, game};
    let flow = world.resource::<GameFlow>();
    let block = flow.player_block.as_ref()?;
    let result = duel_play::load_executable(&flow.import.duel).and_then(|executable| {
        let decks = roses_assets::parse_pal_decks(&executable)
            .map_err(|e| e.to_string())?
            .decks;
        game::map_deck_costs(block, &decks, &executable)
    });
    match result {
        Ok(costs) => Some(costs),
        Err(e) => {
            eprintln!("Game: the map's deck costs are unavailable: {e}");
            None
        }
    }
}

/// The deck editor (mode 6) on the player's card block, with the map's
/// duel counter (SAVE+0xCA) and the shared LCG.
fn enter_deck_editor(world: &mut bevy::ecs::world::World) -> Result<(), String> {
    use roses_game::duel_play;
    // Online Play's deck builder: a custom deck on the full card pool.
    if let Some(inputs) = online_render::builder_inputs(world) {
        if let Some(mut sound) = world.get_resource_mut::<duel_board_render::DuelSound>() {
            sound.cue(roses_game::duel_audio::AudioCue::Music { track: 5, keep: true });
        }
        return deck_editor_render::enter_editor_scene(world, inputs?);
    }
    ensure_player_block(world)?;
    // The deck editor's track 5 on mode entry [F UI-SOUNDS.md 4].
    if let Some(mut sound) = world.get_resource_mut::<duel_board_render::DuelSound>() {
        sound.cue(roses_game::duel_audio::AudioCue::Music { track: 5, keep: true });
    }
    let flow = world.resource::<GameFlow>();
    let block = flow
        .player_block
        .clone()
        .ok_or("the deck editor needs the player's card block (no starter deck chosen)")?;
    let data = duel_play::load_with_end(&flow.import.duel, flow.import.duel_end.as_deref())?;
    let inputs = deck_editor_render::EditorInputs {
        side: 0,
        block,
        duel_counter: flow.map_save.as_ref().map_or(0, |s| s.duel_count),
        rng: flow.rng,
        executable: data.executable,
        messages: data.messages,
        texsys: data.texsys,
        picmini: data.picmini,
        names: data.names,
        replay: flow.editor_replay.clone(),
        blink: flow.scroll_word,
        blink_p1: flow.scroll_p1,
        d63: flow.d63,
        builder: false,
    };
    deck_editor_render::enter_editor_scene(world, inputs)
}

/// A practice lesson on the original board (`tutorial::start_lesson`):
/// the lesson named by the flow, continued from the previous lesson's end.
/// The original shows "NOW LOADING" over the mode change [O 416]; here
/// the scene switches at once.
fn enter_tutorial(world: &mut bevy::ecs::world::World) -> Result<(), String> {
    use roses_game::{duel_play, tutorial};
    let flow = world.resource::<GameFlow>();
    let t = flow.tutorial.as_ref().ok_or("no tutorial in progress")?;
    let data = duel_play::load_with_end(&flow.import.duel, flow.import.duel_end.as_deref())?;
    let (lesson, from, auto, ticks, seed) = (t.lesson, t.from, t.auto, t.ticks, flow.seed);
    let single = t.to_map;
    let board = flow.import.duel_board_manifest.clone();
    let mut play = tutorial::start_lesson(data, lesson, from, seed, auto, single)?;
    eprintln!(
        "Game: practice lesson {} (controller {}, deck records {:#x}/{:#x}, series counter {}) starts on board 0; the original's NOW LOADING screen is not shown and the lesson's duel music is not played (not traced){}",
        lesson.series_index() + 1,
        lesson.controller(),
        lesson.deck_pair().0,
        lesson.deck_pair().1,
        play.tutorial().map_or(0, |t| t.series_counter),
        if auto {
            "; the operator pages the messages (--tutorial-auto)"
        } else {
            ""
        }
    );
    if ticks > 0 && from.is_none() {
        for _ in 0..ticks {
            if play.halted.is_some() || play.tutorial().is_some_and(|t| t.ended.is_some()) {
                break;
            }
            play.update(0);
        }
        eprintln!(
            "Game: --tutorial-ticks: {ticks} updates run before the first frame: {}",
            play.title()
        );
    }
    let scene = duel_board_render::prepare_tutorial_scene(
        &board,
        play,
        Some(flow.import.duel_models_manifest.as_path()),
    )?;
    let scene = match &flow.import.duel_arena {
        Some(arena) => scene.with_arena(arena)?,
        None => scene,
    };
    let scene = match &flow.import.duel_audio {
        Some(audio) => scene.with_audio(audio)?,
        None => scene,
    };
    scene.enter(world)
}

/// Back to the story after the tutorial (FLOW.md Y6: the scene is reloaded,
/// event 10060, track 25 restarted): the parked story model continues at
/// the resume stage and the opening's scene is set up again.
fn resume_opening(world: &mut bevy::ecs::world::World) -> Result<(), String> {
    let mut flow = world.resource_mut::<GameFlow>();
    let mut preview = flow
        .parked_preview
        .take()
        .ok_or("the story model was not kept while the tutorial ran")?;
    let story_stage = flow.story_stage.take();
    match (flow.tutorial.take().map(|t| t.resume), story_stage) {
        (Some(resume), _) => {
            preview.resume_at(&resume)?;
            eprintln!(
                "Game: the story resumes at {resume} (event {}, track {} restarted by the stage's music; the original's camera on re-entry is not traced, the captured Simon pose is shown)",
                roses_core::duel::script::STORY_RESUME_EVENT,
                roses_core::duel::script::STORY_RESUME_TRACK
            );
        }
        // After the red ending (the title) or Continue after it (10090).
        (None, Some(stage)) if stage == "title" || stage == roses_client::ending::STAGE_AFTER_ENDING => {
            preview.resume_at(&stage)?;
            eprintln!("Game: the story continues at {stage}");
        }
        (None, Some(stage)) if !roses_game::opening::is_map_story(&stage) => {
            preview.resume_at(&stage)?;
            eprintln!(
                "Game: the campaign map fired story stage {stage}; Cross pages, the map resumes afterwards"
            );
        }
        (None, Some(stage)) => {
            preview.resume_at(&stage)?;
            eprintln!(
                "Game: the campaign map fired a story event; its pages play as {stage} (paintings, figures, panels, faces and letterbox moving per 25 Hz update (roses_client::story_motion; ROSES_STORY_SETTLED=1 draws the settled pages), the Stonehenge world behind 11160-11184 when the import has it; the map plays 11120/11124/11149's windows itself); Cross pages, the map resumes afterwards"
            );
        }
        (None, None) => return Err("no tutorial or map story event to return from".into()),
    }
    world.insert_resource(OpeningState {
        preview,
        overlay: None,
        blocked: None,
        simon_target: None,
        story_world: false,
        black_target: None,
        composite: None,
        transition: None,
    });
    world
        .run_system_cached(setup)
        .map_err(|e| format!("opening setup: {e}"))?;
    let title = world.resource::<OpeningState>().caption();
    set_title(world, &title);
    Ok(())
}

fn enter_first_duel(world: &mut bevy::ecs::world::World) -> Result<(), String> {
    use roses_game::{duel_play, game};
    // Online Play's match (the lobby paired this player).
    if let Some(entered) = online_render::duel_scene(world) {
        return entered;
    }
    ensure_player_block(world)?;
    let flow = world.resource::<GameFlow>();
    let mut data = duel_play::load_with_end(&flow.import.duel, flow.import.duel_end.as_deref())?;
    // Custom Duel's Start (mode 9 → 1): its own sides, board and rules.
    let custom = match custom_duel_render::duel_setup(world, &data) {
        Some(r) => Some(r?),
        None => None,
    };
    let flow = world.resource::<GameFlow>();
    let request = match &custom {
        Some((duel, _)) => roses_client::campaign_map::DuelRequest {
            node: 0,
            opponent: duel.opponent,
            params: [duel.board_map as i32, 0, 1100, 1000 + i32::from(duel.opponent)],
        },
        None => flow.request.clone().ok_or("the map asked for no duel")?,
    };
    let decks = roses_assets::parse_pal_decks(&data.executable)
        .map_err(|e| e.to_string())?
        .decks;
    let maps = roses_assets::parse_pal_rules(&data.executable)
        .map_err(|e| e.to_string())?
        .maps;
    // The player's side is the card block's current deck (the deck record
    // the duel setup 0x214120 reads), never the story's starter list.
    let player_block = flow
        .player_block
        .clone()
        .ok_or("the duel needs the player's card block (no starter deck chosen)")?;
    let starter_record = flow
        .campaign
        .starter_deck
        .as_ref()
        .map_or(0, |c| usize::from(c.record));
    let player = game::PlayerDeck::from_block(&player_block, starter_record)?;
    let custom_block = match custom {
        Some((duel, block)) => {
            data.first_duel = duel;
            Some(block)
        }
        None => {
            data.first_duel = game::duel_for_request(
                &request,
                &data.first_duel,
                &player,
                &decks,
                &maps,
                &data.executable,
            )?;
            None
        }
    };
    let leaders = [data.first_duel.sides[0].leader, data.first_duel.sides[1].leader];
    world.resource_mut::<GameFlow>().duel_leaders = Some(leaders);
    let flow = world.resource::<GameFlow>();
    eprintln!(
        "Game: the map starts the duel at {} with the card block's deck {} (starter record {:#x}, leader {} at rank {}, {} cards, record order starts {:?}), shuffle seed {}",
        request.node,
        roses_core::deck_editor::current_deck(&player_block),
        starter_record,
        player.leader,
        player.leader_rank,
        player.cards.len(),
        &player.cards[..player.cards.len().min(6)],
        flow.seed
    );
    eprintln!(
        "Game: the player's deck record in order: {:?}",
        player.cards
    );
    let board = flow.import.duel_board_manifest.clone();
    let seed = flow.seed;
    let map = request.params[0] as u8;
    let end = match &flow.import.duel_end {
        Some(dir) => {
            use roses_game::duel_end_screens as screens;
            let files = screens::load_duel_end_import(dir)?;
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
            let mut rng = flow.rng;
            let sides = &data.first_duel.sides;
            // The computer's block is made per duel from its static deck
            // record (assumed layout, see new_game_block; the owner key is
            // 0x40000000 | its deck record as the capture shows).
            // Blocks by side: the player's on its side (1 on the White Rose route).
            let them = 1 - usize::from(data.first_duel.player_side);
            let computer = match custom_block {
                Some(block) => block,
                None => screens::new_game_block(
                    screens::computer_owner_key(sides[them].deck_record),
                    &sides[them].cards,
                    sides[them].leader,
                    rank_of(sides[them].deck_record),
                    &thresholds,
                    &mut rng,
                ),
            };
            let blocks = if them == 1 {
                [player_block.clone(), computer]
            } else {
                [computer, player_block.clone()]
            };
            Some(duel_board_render::EndInputs {
                blocks,
                rng,
                map,
                duel_end: files,
                replay: flow.end_replay.clone(),
            })
        }
        None => {
            eprintln!(
                "Game boundary ahead: the game import has no duel-end part (result screens and rewards need `roses import-game` run again); the duel will stop at its outcome"
            );
            None
        }
    };
    let computer_player = flow.computer_player;
    let capture = world.contains_resource::<CaptureState>();
    let scene = duel_board_render::prepare_play_scene(
        &board,
        data,
        seed,
        computer_player,
        end,
        Some(flow.import.duel_models_manifest.as_path()),
        // A headless capture freezes at the first gameplay frame: no
        // opening there.
        !capture,
    )?;
    let scene = match &flow.import.duel_arena {
        Some(arena) => scene.with_arena(arena)?,
        None => scene,
    };
    let scene = match &flow.import.duel_audio {
        Some(audio) => scene.with_audio(audio)?,
        None => scene,
    };
    scene.enter(world)
}

/// The reward reels after a won duel: the reel data from the result
/// sequence, the blocks and the LCG from the game, the popup drawer's files
/// from the duel and duel-end imports.
fn enter_slot_machine(world: &mut bevy::ecs::world::World) -> Result<(), String> {
    use roses_game::{duel_end_screens as screens, duel_play};
    // Mode 0xF entry: 0x1AB440(15, 1) → track 21 [F UI-SOUNDS.md 5].
    if let Some(mut sound) = world.get_resource_mut::<duel_board_render::DuelSound>() {
        sound.cue(roses_game::duel_audio::AudioCue::Music { track: 21, keep: true });
    }
    let flow = world.resource::<GameFlow>();
    let (winner, reels) = flow
        .reels
        .clone()
        .ok_or("no reel data for the slot machine")?;
    let blocks = flow
        .blocks
        .clone()
        .ok_or("no card blocks for the slot machine")?;
    let dir = flow
        .import
        .duel_end
        .clone()
        .ok_or("the game import has no duel-end part")?;
    let data = duel_play::load_with_end(&flow.import.duel, flow.import.duel_end.as_deref())?;
    let inputs = slot_machine_render::SlotInputs {
        reels,
        winner,
        blocks,
        rng: flow.rng,
        executable: data.executable,
        messages: data.messages,
        texsys: data.texsys,
        picmini: data.picmini,
        // The campaign duel's leaders, not the imported first duel's.
        leaders: flow.duel_leaders.unwrap_or([
            data.first_duel.sides[0].leader,
            data.first_duel.sides[1].leader,
        ]),
        duel_end: screens::load_duel_end_import(&dir)?,
        replay: flow.end_replay.clone(),
    };
    slot_machine_render::enter_slot_scene(world, inputs)
}

/// Hand-offs inside the game: the map's duel request starts the duel; a
/// finished duel returns to the map with its result (the result screens and
/// rewards in between are not built yet).
fn game_duel_watch(world: &mut bevy::ecs::world::World) {
    let Some(flow) = world.get_resource::<GameFlow>() else {
        return;
    };
    if flow.stopped.is_some() || flow.next.is_some() {
        return;
    }
    if flow.scene == GameScene::CustomDuel {
        custom_duel_render::watch(world);
        return;
    }
    if flow.scene == GameScene::Trade {
        trade_render::watch(world);
        return;
    }
    if flow.scene == GameScene::Online {
        online_render::watch(world);
        return;
    }
    // Custom Duel's deck editor, duel and reels return to mode 9 (init −1).
    let custom = custom_duel_render::active(world);
    // Online Play's duel returns to its screens.
    let online = online_render::active(world);
    let flow = world.resource::<GameFlow>();
    if flow.scene == GameScene::Map {
        // The OPTION panel wrote its bits: 0x214340 on the player's block
        // (the 2P block is RAM-only and not in the story's file), the sound
        // row into the mixer (0x217200).
        if let Some(bits) = campaign_map_render::map_take_options(world) {
            if let Some(block) = world.resource_mut::<GameFlow>().player_block.as_mut() {
                roses_client::game_options::set_bits(block, bits);
            }
            apply_sound_option(world, bits);
        }
        if campaign_map_render::map_title_request(world) {
            // YES on MSG 146: 0x1A9290 pops to the title [H: whether the
            // title's init replays the opening movie here (param[10]) is
            // not traced; the menu is shown as after the ending].
            eprintln!("Game: Return to Title Screen? YES: back to the title");
            let mut flow = world.resource_mut::<GameFlow>();
            flow.map_save = None;
            flow.won = None;
            flow.request = None;
            flow.story_stage = Some("title".into());
            flow.next = Some(GameScene::Opening);
            return;
        }
        if let Some(request) = campaign_map_render::map_duel_request(world) {
            let save = campaign_map_render::map_save(world);
            let mut flow = world.resource_mut::<GameFlow>();
            flow.map_save = save;
            flow.request = Some(request);
            flow.next = Some(GameScene::Duel);
        } else if campaign_map_render::map_save_request(world) {
            // The memory-card save dialog (op 1) runs inside the map update.
            match map_card_file(world) {
                Ok((file, card)) => campaign_map_render::map_start_save(world, file, card),
                Err(message) => eprintln!("Game: SYSTEM -> SAVE: {message}"),
            }
        } else if let Some(row) = campaign_map_render::map_tutorial_request(world) {
            // SYSTEM icon 3 → HELP: 0x23B710 backs up the card block and
            // pushes the lesson's duel (rows 0..3 → controls 32..35, other
            // → 36); the map resumes from its save afterwards.
            let save = campaign_map_render::map_save(world);
            let mut flow = world.resource_mut::<GameFlow>();
            flow.map_save = save;
            let (auto, ticks) = flow.tutorial.as_ref().map_or((false, 0), |t| (t.auto, t.ticks));
            flow.tutorial = Some(TutorialFlow {
                resume: String::new(),
                lesson: Lesson::from_menu_row(row),
                from: None,
                auto,
                ticks,
                to_map: true,
            });
            flow.next = Some(GameScene::Tutorial);
        } else if campaign_map_render::map_editor_request(world) {
            let save = campaign_map_render::map_save(world);
            let mut flow = world.resource_mut::<GameFlow>();
            flow.map_save = save;
            flow.next = Some(GameScene::DeckEditor);
        } else if let Some(event) = campaign_map_render::map_story_request(world) {
            // A Stonehenge-scene story event (11160, 11180, 12130, 12150):
            // its pages play in the story scene; the map resumes at the
            // event's continuation. (11122 and 11140..11148 play over the
            // running map: `campaign_map_render::MapStoryHost`.)
            let save = campaign_map_render::map_save(world);
            let mut flow = world.resource_mut::<GameFlow>();
            flow.map_save = save;
            let stage = roses_game::opening::map_story_stage(event);
            // A story import without the event's stage (the White Rose
            // events before their import) stops here, on the map.
            let known = flow
                .parked_preview
                .as_ref()
                .is_none_or(|p| p.model.stage_ids().contains(&stage.as_str()));
            if !known {
                let message = format!(
                    "map event {event} plays story stage {stage}, which this story import does not have (the White Rose story events are not imported yet)"
                );
                eprintln!("Game boundary: {message}");
                flow.stopped = Some(message.clone());
                set_title(world, &format!("Roses - stopped: {message}"));
                return;
            }
            flow.story_stage = Some(stage);
            flow.next = Some(GameScene::Opening);
        }
        return;
    }
    if flow.scene == GameScene::DeckEditor {
        if deck_editor_render::editor_scene_finished(world) {
            let mut flow = world.resource_mut::<GameFlow>();
            flow.won = None;
            flow.next = Some(if online {
                GameScene::Online
            } else if custom {
                GameScene::CustomDuel
            } else {
                GameScene::Map
            });
        }
        return;
    }
    if flow.scene == GameScene::Tutorial {
        if let Some(end) = duel_board_render::tutorial_end(world) {
            let mut flow = world.resource_mut::<GameFlow>();
            match end {
                LessonEnd::NextLesson { lesson, .. } => {
                    if let Some(t) = flow.tutorial.as_mut() {
                        t.lesson = lesson;
                        t.from = Some(end);
                    }
                    flow.next = Some(GameScene::Tutorial);
                }
                LessonEnd::SeriesEnd { quit } if flow.tutorial.as_ref().is_some_and(|t| t.to_map) => {
                    eprintln!(
                        "Game: the map's lesson {}; back to the map (card block unchanged)",
                        if quit { "was quit with Start" } else { "finished" }
                    );
                    flow.tutorial = None;
                    flow.next = Some(GameScene::Map);
                }
                LessonEnd::SeriesEnd { quit } => {
                    eprintln!(
                        "Game: the tutorial series {}; back to the story",
                        if quit {
                            "was quit with Start"
                        } else {
                            "finished its third lesson"
                        }
                    );
                    flow.next = Some(GameScene::Opening);
                }
            }
            return;
        }
        if let (_, Some(error)) = duel_board_render::play_scene_outcome(world) {
            let message = format!("the lesson stopped: {error}");
            eprintln!("Game boundary: {message}");
            world.resource_mut::<GameFlow>().stopped = Some(message);
        }
        return;
    }
    if flow.scene == GameScene::SlotMachine {
        if slot_machine_render::slot_scene_finished(world) {
            let mut flow = world.resource_mut::<GameFlow>();
            flow.won = Some(true);
            flow.next = Some(if custom { GameScene::CustomDuel } else { GameScene::Map });
        }
        return;
    }
    if flow.scene != GameScene::Duel {
        return;
    }
    // Test aid (a port addition, MODES): `ROSES_MULTI_CAPTURE_ROUND=N` with
    // `--capture-game-scene online` captures the N-player duel itself once
    // it reaches round N, at rest.
    let multi_round = std::env::var("ROSES_MULTI_CAPTURE_ROUND").ok().and_then(|v| v.parse::<u8>().ok());
    if let (Some(n), Some((round, quiet, rested))) = (multi_round, duel_board_render::multi_progress(world)) {
        // Held at a quiet moment, captured once the camera and HUD rest.
        let waiting = online && round >= n && flow.capture_scene == Some(GameScene::Online);
        if waiting && quiet && !rested {
            duel_board_render::freeze_for_capture(world);
            return;
        }
        if waiting && rested {
            eprintln!("Game: N-player duel at round {round}: capturing it");
            duel_board_render::freeze_for_capture(world);
            world.resource_mut::<GameFlow>().capture_scene = Some(GameScene::Duel);
            // The duel has replaced meshes and images since its scene was
            // registered: the capture waits only for those still in use.
            if let Some(bridge) = world.get_resource::<CaptureBridge>().cloned() {
                let mut p = bridge.0.lock().expect("Capture progress mutex poisoned");
                let meshes = world.resource::<Assets<Mesh>>();
                p.meshes.retain(|id| meshes.get(*id).is_some());
                let images = world.resource::<Assets<Image>>();
                p.textures.retain(|id| images.get(*id).is_some());
                p.materials.clear();
                p.buffers.clear();
            }
            return;
        }
    }
    // The result sequence (over the board) ends with the reels or the map.
    if let Some(outcome) = duel_board_render::end_outcome(world) {
        use roses_core::duel_end::EndOutcome;
        // D50: the reels only follow a won duel, so the duel's winner is the
        // human side whose block receives the prizes.
        let winner = duel_board_render::play_scene_outcome(world)
            .0
            .flatten()
            .unwrap_or(0);
        let mut flow = world.resource_mut::<GameFlow>();
        match outcome {
            _ if online => flow.next = Some(GameScene::Online),
            EndOutcome::SlotMachine(reels) => {
                flow.reels = Some((winner, reels));
                flow.next = Some(GameScene::SlotMachine);
            }
            EndOutcome::ReturnToMap { human_won } if custom => {
                // 0x1A9290 pops to mode 9 (map >= 22: no campaign save).
                eprintln!("Game: Custom Duel: the duel is over ({}); back to the menu", if human_won { "won" } else { "lost" });
                flow.next = Some(GameScene::CustomDuel);
            }
            EndOutcome::ReturnToMap { human_won } => {
                flow.won = Some(human_won);
                flow.next = Some(GameScene::Map);
            }
            EndOutcome::Return if custom => flow.next = Some(GameScene::CustomDuel),
            EndOutcome::Return => {
                // 0x1A9290 at 0x5D with two pads or none: the original
                // writes no save; the campaign map's loss continuation is
                // the only return modelled here.
                eprintln!(
                    "Game boundary: a duel without exactly one pad side pops without a save; continuing on the map as a loss"
                );
                flow.won = Some(false);
                flow.next = Some(GameScene::Map);
            }
        }
        return;
    }
    let (outcome, halted) = duel_board_render::play_scene_outcome(world);
    let message = match (outcome, halted) {
        // A spectator pressed Start: back to the WATCH list.
        (_, Some(error)) if online && error == roses_game::duel_online::STOPPED_WATCHING => {
            Some("You stopped watching.".to_string())
        }
        // An N-player match's placing list was closed (a port addition).
        (_, Some(error)) if online && error == roses_game::duel_multi::MULTI_DONE => {
            Some(duel_board_render::multi_end_line(world).unwrap_or_else(|| "The match is over.".to_string()))
        }
        (_, Some(error)) => Some(format!("the duel stopped: {error}")),
        // Without the duel-end import the duel has no result screens.
        (Some(winner), _) if flow.import.duel_end.is_none() => Some(format!(
            "duel over ({}); the result screens and rewards need the duel-end import (re-run roses import-game)",
            match winner {
                Some(0) => "you win",
                Some(_) => "you lose",
                None => "draw",
            }
        )),
        _ => None,
    };
    if let Some(message) = message {
        if online {
            // The lobby goes on; the menu says why the duel ended here.
            eprintln!("Game: Online Play: {message}; back to the lobby");
            online_render::duel_stopped(world, &message);
            world.resource_mut::<GameFlow>().next = Some(GameScene::Online);
            return;
        }
        eprintln!("Game boundary: {message}");
        world.resource_mut::<GameFlow>().stopped = Some(message);
    }
}

/// The key events that take the synthetic map pad from `held` to `next`
/// (the map scene reads arrows and X as held keys, Cross once per press).
fn map_pad_key_events(
    held: roses_client::campaign_map::Pad,
    next: roses_client::campaign_map::Pad,
) -> Vec<(KeyCode, Key, ButtonState)> {
    let keys = [
        (held.up, next.up, KeyCode::ArrowUp, Key::ArrowUp),
        (held.down, next.down, KeyCode::ArrowDown, Key::ArrowDown),
        (held.left, next.left, KeyCode::ArrowLeft, Key::ArrowLeft),
        (held.right, next.right, KeyCode::ArrowRight, Key::ArrowRight),
        (
            held.cross,
            next.cross,
            KeyCode::KeyX,
            Key::Character("x".into()),
        ),
        (held.menu.select, next.menu.select, KeyCode::Tab, Key::Tab),
        (held.menu.circle, next.menu.circle, KeyCode::KeyZ, Key::Character("z".into())),
        (held.menu.square, next.menu.square, KeyCode::KeyA, Key::Character("a".into())),
        (held.menu.l1, next.menu.l1, KeyCode::KeyQ, Key::Character("q".into())),
        (held.menu.r1, next.menu.r1, KeyCode::KeyE, Key::Character("e".into())),
    ];
    keys.into_iter()
        .filter(|(was, is, _, _)| was != is)
        .map(|(_, is, code, key)| {
            (
                code,
                key,
                if is {
                    ButtonState::Pressed
                } else {
                    ButtonState::Released
                },
            )
        })
        .collect()
}

/// `--story-auto`: on a map story stage, Cross (Enter) every eighth update
/// as synthetic key events (test aid for scripted runs through the story
/// events the map fires).
fn game_story_auto(
    mut flow: ResMut<GameFlow>,
    opening: Option<Res<OpeningState>>,
    ending: Option<Res<ending_render::EndingRun>>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut keys: MessageWriter<KeyboardInput>,
) {
    if !flow.story_auto || flow.scene != GameScene::Opening || flow.next.is_some() || ending.is_some() {
        return;
    }
    let Some(opening) = opening else {
        return;
    };
    let stage = opening.preview.model.stage().id();
    // Map story events, and Brest (11020) once the map has played event 100.
    let from_map = roses_game::opening::is_map_story(stage)
        || (stage == roses_game::opening::BREST_STAGE && flow.map_save.is_some());
    if !from_map {
        return;
    }
    flow.story_auto_ticks += 1;
    if flow.story_auto_ticks % 8 != 0 {
        return;
    }
    // A headless capture has no window; the opening reads the messages
    // without checking their window.
    let window = windows.single().unwrap_or(Entity::PLACEHOLDER);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        keys.write(KeyboardInput {
            key_code: KeyCode::Enter,
            logical_key: Key::Enter,
            state,
            text: None,
            repeat: false,
            window,
        });
    }
}

/// `--map-replay`: one scripted pad per 25 Hz update while the campaign
/// map is on screen, delivered as synthetic key events that the map scene's
/// own keyboard reader picks up on the next frame (so a pad lands about one
/// frame late and several updates of one frame share it). When the script
/// runs out every held key is released.
fn game_map_replay(
    mut flow: ResMut<GameFlow>,
    capture: Option<Res<CaptureState>>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut keys: MessageWriter<KeyboardInput>,
    recording: Option<Res<record::RecordTarget>>,
) {
    // A headless capture (or a recording) has no PrimaryWindow. The map's
    // own tick consumes the same replay directly, one original pad word per
    // FixedUpdate.
    if flow.scene != GameScene::Map || capture.is_some() || recording.is_some() {
        return;
    }
    let next = match flow.map_replay.pop_front() {
        Some(pad) => pad,
        None if flow.map_replay_held != roses_client::campaign_map::Pad::NONE => {
            roses_client::campaign_map::Pad::NONE
        }
        None => return,
    };
    let Ok(window) = windows.single() else {
        return;
    };
    for (key_code, logical_key, state) in map_pad_key_events(flow.map_replay_held, next) {
        keys.write(KeyboardInput {
            key_code,
            logical_key,
            state,
            text: None,
            repeat: false,
            window,
        });
    }
    flow.map_replay_held = next;
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct CompositeMaterial {
    #[texture(0)]
    #[sampler(1)]
    world: Handle<Image>,
    #[texture(2)]
    #[sampler(3)]
    overlay: Handle<Image>,
    /// Scene overlay quad: RGB bytes/255 and GS alpha/128 (zero when absent).
    #[uniform(4)]
    cover: Vec4,
    /// Additive flash, RGB bytes·As/128/255 (zero when absent).
    #[uniform(5)]
    flash: Vec4,
    /// c401's light beam, an additive layer over the 3D band (rows
    /// 144..368, RGBA8 bytes; black when absent), `story_beam`.
    #[texture(6)]
    beam: Handle<Image>,
}
impl Material for CompositeMaterial {
    fn vertex_shader() -> ShaderRef {
        "opening_composite.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "opening_composite.wgsl".into()
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
        descriptor.label = Some(COMPOSITE_PIPELINE.into());
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
        ])?];
        descriptor.primitive.cull_mode = None;
        // Camera3d's render pass owns a depth attachment, so keep its format in
        // the pipeline even though this fullscreen pass neither tests nor writes.
        if let Some(depth) = &mut descriptor.depth_stencil {
            depth.depth_compare = Some(CompareFunction::Always);
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

fn ui_image(image: roses_client::Image) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: image.width,
            height: image.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        image.rgba,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        ..default()
    });
    image
}

fn opening_button(event: &KeyboardInput, primary: Option<Button>) -> Option<Button> {
    if event.state != ButtonState::Pressed || event.repeat {
        return None;
    }
    // Preserve physical controls. Only an otherwise-unmapped physical key may
    // fall back to a logical named arrow (observed Windows input: Numpad6/right).
    match event.key_code {
        KeyCode::Enter => primary,
        KeyCode::KeyX => Some(Button::Cross),
        KeyCode::KeyA => Some(Button::Square),
        KeyCode::KeyZ => Some(Button::Circle),
        KeyCode::KeyS => Some(Button::Start),
        KeyCode::KeyQ => Some(Button::L1),
        KeyCode::KeyE => Some(Button::R1),
        KeyCode::ArrowUp => Some(Button::Up),
        KeyCode::ArrowDown => Some(Button::Down),
        KeyCode::ArrowLeft => Some(Button::Left),
        KeyCode::ArrowRight => Some(Button::Right),
        _ => match event.logical_key {
            Key::ArrowUp => Some(Button::Up),
            Key::ArrowDown => Some(Button::Down),
            Key::ArrowLeft => Some(Button::Left),
            Key::ArrowRight => Some(Button::Right),
            _ => None,
        },
    }
}

fn update_opening(
    mut events: MessageReader<KeyboardInput>,
    mut opening: ResMut<OpeningState>,
    mut images: ResMut<Assets<Image>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
    clouds: Option<ResMut<CloudAnimation>>,
    mut composites: ResMut<Assets<CompositeMaterial>>,
    mut cameras: Query<&mut Camera>,
    mut flow: Option<ResMut<GameFlow>>,
    mut sound: Option<ResMut<duel_board_render::DuelSound>>,
    mut title_load: Option<ResMut<TitleLoad>>,
    mut ending: Option<ResMut<ending_render::EndingRun>>,
    mut commands: Commands,
) {
    // The red ending (11184's tail) has the pad from caption 3028's Cross
    // to the title: its save dialog and the movie stand-in.
    if let Some(run) = ending.as_deref_mut() {
        for event in events.read() {
            if event.state != bevy::input::ButtonState::Pressed || event.repeat {
                continue;
            }
            match event.key_code {
                KeyCode::KeyX | KeyCode::Enter | KeyCode::KeyS => run.pad.cross = true,
                KeyCode::KeyZ => run.pad.circle = true,
                KeyCode::KeyA => run.pad.square = true,
                KeyCode::KeyD => run.pad.triangle = true,
                KeyCode::ArrowLeft => run.pad.left = true,
                KeyCode::ArrowRight => run.pad.right = true,
                _ => {}
            }
        }
        return;
    }
    // The title's memory-card load dialog has the pad (0x1AD050 returns
    // while 0x1ABB20 runs): X/Enter Cross, Z Circle, A Square, D Triangle.
    if let Some(load) = title_load.as_deref_mut() {
        for event in events.read() {
            if event.state != bevy::input::ButtonState::Pressed || event.repeat {
                continue;
            }
            match event.key_code {
                KeyCode::KeyX | KeyCode::Enter => load.pad.cross = true,
                KeyCode::KeyZ => load.pad.circle = true,
                KeyCode::KeyA => load.pad.square = true,
                KeyCode::KeyD => load.pad.triangle = true,
                KeyCode::ArrowLeft => load.pad.left = true,
                KeyCode::ArrowRight => load.pad.right = true,
                _ => {}
            }
        }
        return;
    }
    // The presses' SYSTEM0 sounds (UI-SOUNDS.md 2).
    let sounds = opening.preview.take_sounds();
    if let Some(sound) = sound.as_deref_mut() {
        for id in sounds {
            sound.cue(roses_game::duel_audio::AudioCue::Se { slot: 0, id, skip: false });
        }
    }
    if opening.blocked.is_some() || opening.transition.is_some() {
        // The original ignores the pad while a scene handoff runs.
        events.clear();
        return;
    }
    // The title's Continue (live or replayed): the memory-card load
    // dialog (0x1AB8F0(0)) from the next update.
        if opening.preview.take_continue() {
            let Some(flow) = flow.as_mut() else {
                opening.blocked = Some("Continue needs the connected game (--game)".into());
                return;
            };
            commands.insert_resource(TitleLoad::start(flow));
            events.clear();
            return;
        }
    // The title's Custom Duel: memory-card op 3, then mode 9.
        if opening.preview.take_custom_duel() {
            let Some(flow) = flow.as_mut() else {
                opening.blocked = Some("Custom Duel needs the connected game (--game)".into());
                return;
            };
            commands.insert_resource(TitleLoad::custom_duel(flow));
            events.clear();
            return;
        }
    // The title's Online Play (a port addition): the Online Play screens.
        if opening.preview.take_online() && roses_game::extras() {
            let Some(flow) = flow.as_mut() else {
                opening.blocked = Some("Online Play needs the connected game (--game)".into());
                return;
            };
            flow.next = Some(GameScene::Online);
            events.clear();
            return;
        }
    // The title's Trade: memory-card op 4, then mode 8.
        if opening.preview.take_trade() {
            let Some(flow) = flow.as_mut() else {
                opening.blocked = Some("Trade needs the connected game (--game)".into());
                return;
            };
            commands.insert_resource(TitleLoad::trade(flow));
            events.clear();
            return;
        }
    let mut changed = false;
    let from_history = opening.preview.model.stage().id() == "historical-story";
    // Crosses the story motion released (page turns, stage ends) first,
    // then the pad.
    let primary = opening.preview.model.stage().primary_button();
    let presses: Vec<(Button, bool)> = opening
        .preview
        .take_forwarded()
        .into_iter()
        .map(|b| (b, true))
        .chain(events.read().filter_map(|e| opening_button(e, primary)).map(|b| (b, false)))
        .collect();
    for (button, forwarded) in presses {
        let result = if forwarded {
            opening.preview.input_forwarded(button)
        } else {
            opening.preview.input(button)
        };
        match result {
            Ok(repaint) => {
                changed |= repaint;
                if let Some(flow) = flow.as_mut() {
                    let chosen = opening.preview.model.campaign().starter_deck.clone();
                    if flow.player_block.is_none() && chosen.is_some() {
                        flow.campaign = opening.preview.model.campaign().clone();
                        flow.needs_block = true;
                    }
                }
                if let Some(event) = opening.preview.take_map_event() {
                    match flow.as_mut() {
                        Some(flow) if roses_client::ending::Route::of_event(event).is_some() => {
                            // The rest of map event 11184 / 12156 is the ending.
                            let route = roses_client::ending::Route::of_event(event).unwrap_or(roses_client::ending::Route::Red);
                            match ending_render::EndingRun::start(flow, route) {
                                Ok(run) => {
                                    eprintln!("Game: the last caption closed; the {route:?} ending runs (letterbox, save op 2, movie {}, title)", route.movie().disc_path);
                                    commands.insert_resource(run);
                                }
                                Err(message) => {
                                    eprintln!("Game boundary: {message}");
                                    opening.blocked = Some(message);
                                }
                            }
                            events.clear();
                            return;
                        }
                        Some(flow) => {
                            flow.campaign = opening.preview.model.campaign().clone();
                            flow.story_exit = roses_game::opening::map_story_event(opening.preview.model.stage().id());
                            let chosen_side = flow.campaign.side;
                            match flow.map_save.as_mut() {
                                Some(save) => {
                                    save.event = i16::try_from(event).unwrap_or(0);
                                    // The side choice writes SAVE[0] and C9
                                    // (0x21ED34..0x21ED60: Yorkists 1/2,
                                    // Lancastrians 0/0).
                                    if let Some(side) = chosen_side {
                                        save.side = side.story_byte();
                                        save.c9 = 2 * save.side;
                                    }
                                }
                                None => match first_map_save(event, chosen_side) {
                                    Ok(save) => flow.map_save = Some(save),
                                    Err(message) => {
                                        eprintln!("Game boundary: {message}");
                                        opening.blocked = Some(message);
                                        changed = true;
                                        break;
                                    }
                                },
                            }
                            eprintln!(
                                "Game: the story pages ended; the campaign map continues at event {event}"
                            );
                            flow.won = None;
                            flow.next = Some(GameScene::Map);
                            events.clear();
                            return;
                        }
                        None => {
                            let message = format!(
                                "the campaign map continues at event {event} in the connected game (--game)"
                            );
                            eprintln!("Opening preview boundary: {message}");
                            opening.blocked = Some(message);
                            changed = true;
                            break;
                        }
                    }
                }
                if let Some(resume) = opening.preview.take_tutorial() {
                    match flow.as_mut() {
                        Some(flow) => {
                            // Practice "Yes" (FLOW.md Y1): the tutorial series
                            // starts with lesson 1 (counter 1).
                            let (auto, ticks) = flow
                                .tutorial
                                .as_ref()
                                .map_or((false, 0), |t| (t.auto, t.ticks));
                            let lesson =
                                flow.tutorial.as_ref().map_or(Lesson::Basics, |t| t.lesson);
                            flow.tutorial = Some(TutorialFlow {
                                resume,
                                lesson,
                                from: None,
                                auto,
                                ticks,
                                to_map: false,
                            });
                            flow.next = Some(GameScene::Tutorial);
                            events.clear();
                            return;
                        }
                        None => {
                            let message = format!(
                                "the scripted tutorial (practice Yes) is played by the connected game (--game); the story would resume at {resume}"
                            );
                            eprintln!("Opening preview boundary: {message}");
                            opening.blocked = Some(message);
                            changed = true;
                            break;
                        }
                    }
                }
            }
            Err(_)
                if flow.is_some()
                    && opening.preview.model.stage().id()
                        == roses_game::game::LAST_STORY_STAGE =>
            {
                // The story reached the campaign map: the game continues.
                if let Some(flow) = flow.as_mut() {
                    flow.campaign = opening.preview.model.campaign().clone();
                    flow.next = Some(GameScene::Map);
                }
                events.clear();
                return;
            }
            Err(message) => {
                eprintln!("Opening preview boundary: {message}");
                opening.blocked = Some(message);
                changed = true;
                break;
            }
        }
    }
    if !changed {
        return;
    }
    if from_history && opening.preview.model.stage().id() == "simon-arrival" && clouds.is_some() {
        opening.transition = Some(Transition::HistoryExit(HistoryExit::new()));
    }
    if let Err(error) = present(
        &mut opening,
        clouds.map(|c| c.into_inner()),
        &mut composites,
        &mut images,
        &mut cameras,
        &mut windows,
    ) {
        eprintln!("Overlay failed: {error}");
        exit.write(AppExit::error());
    }
}

/// Map story stages move (`roses_client::story_motion`) unless
/// ROSES_STORY_SETTLED is set (the imported settled pages then).
fn story_motion_enabled() -> bool {
    std::env::var_os("ROSES_STORY_SETTLED").is_none()
}

/// One 25 Hz update of a moving story stage (letterbox, slides, fades,
/// panels, pan, text reveal); the overlay is redrawn when the frame changed.
fn story_motion_tick(
    opening: Option<ResMut<OpeningState>>,
    ending: Option<Res<ending_render::EndingRun>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut opening) = opening else { return };
    if opening.blocked.is_some() || opening.transition.is_some() || ending.is_some() {
        return;
    }
    // Test aid: ROSES_STORY_MOTION_STOP=<n> holds each moving stage at its
    // n-th update (deterministic mid-motion captures).
    let stop = std::env::var("ROSES_STORY_MOTION_STOP").ok().and_then(|v| v.parse::<u32>().ok());
    if stop.is_some_and(|n| opening.preview.active_motion().is_some_and(|m| m.update >= n)) {
        return;
    }
    if !opening.preview.tick() {
        return;
    }
    match opening.preview.overlay() {
        Ok(rgba) => {
            if let Some(mut image) = opening.overlay.as_ref().and_then(|h| images.get_mut(h)) {
                *image = ui_image(rgba);
            }
        }
        Err(error) => eprintln!("Story motion overlay: {error}"),
    }
}

/// The C-module effects of the Stonehenge story events, on the story's
/// update clock (after `update_world_pose`, before the cloud step):
/// - c401 (11162 / 12132, launched on the event's C2 update [emu
///   trace-seto-cam f773, white trace-brest f811]): its additive white-blue
///   flash, As = trunc(128·(48−k)/48) for k = 0..=48 from task frame 32
///   (0x1E001B0, the Simon entry's flash); its light beam is not drawn.
/// - c402 (11184 / 12156, launched once W+64 ≥ 2001 [F 0x2217D0 /
///   0x222CF4]): the converging clouds (`clouds::CloudKind::Converging`)
///   over the story world, until the event ends.
#[allow(clippy::too_many_arguments)]
fn story_effects(
    opening: Option<Res<OpeningState>>,
    rig: Option<Res<WorldRig>>,
    clouds: Option<ResMut<CloudAnimation>>,
    mut cloud_materials: ResMut<Assets<CloudMaterial>>,
    mut composites: ResMut<Assets<CompositeMaterial>>,
    mut cameras: Query<&mut Camera>,
) {
    let Some(opening) = opening else { return };
    if opening.transition.is_some() {
        return;
    }
    let stage = opening.preview.model.stage().id();
    let story = opening.world() == World::StoryScene0;
    // c401's flash: task frame n = 0 on motion update 2.
    let flash = match opening.preview.active_motion() {
        Some(m) if story && matches!(stage, "map-story-11162" | "map-story-12132") => {
            match m.update.checked_sub(C401_FLASH_UPDATE) {
                Some(k) if k <= 48 => (128 * (48 - k) / 48) as u8,
                _ => 0,
            }
        }
        _ => 0,
    };
    let mut composite = opening.composite.as_ref().and_then(|h| composites.get_mut(h));
    if let Some(material) = composite.as_mut() {
        let [r, g, b] = roses_game::transition::FLASH_RGB
            .map(|c| (u32::from(c) * u32::from(flash) / 128) as f32 / 255.);
        let value = Vec4::new(r, g, b, 0.);
        if material.flash != value {
            material.flash = value;
        }
    }
    let Some(mut clouds) = clouds else { return };
    let ending = story && matches!(stage, "map-story-11184" | "map-story-12156");
    let launch = ending && clouds.converging.is_none() && rig.as_ref().is_some_and(|r| r.level >= C402_LEVEL);
    let stop = !ending && clouds.converging.is_some();
    if !(launch || stop) {
        return;
    }
    if launch {
        if let Err(error) = clouds.start_converging() {
            eprintln!("c402 clouds: {error}");
            return;
        }
    } else {
        clouds.stop_converging();
    }
    let active = launch || opening.world() == World::Clouds;
    clouds.enabled = active;
    if let Some(camera) = clouds.camera.and_then(|id| cameras.get_mut(id).ok()) {
        camera.into_inner().is_active = active;
    }
    // The story world shows under the clouds (no navy cover); the history
    // sets its own cover alpha when it runs again.
    if let Some(material) = clouds.material.as_ref().and_then(|h| cloud_materials.get_mut(h)) {
        material.into_inner().settings.fade_alpha.x = if launch { 0 } else { 128 };
    }
    let target = if launch { clouds.target.clone() } else { opening.simon_target.clone() };
    if let (Some(target), Some(material)) = (target, composite.as_mut()) {
        material.world = target;
    }
}
/// The 3D band c401's beam layer covers (raw512 rows 144..368).
const BEAM_WIDTH: usize = 512;
const BEAM_TOP: usize = 144;
const BEAM_ROWS: usize = 224;

fn beam_image(rgba: Vec<u8>) -> Image {
    Image::new(
        Extent3d {
            width: BEAM_WIDTH as u32,
            height: BEAM_ROWS as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    )
}

/// c401 (`story_fx`): the original module's inputs from the connected
/// game's import and the running task.
#[derive(Resource)]
struct LightBeam {
    executable: Vec<u8>,
    cmodule: Vec<u8>,
    texeve: Vec<u8>,
    texture: roses_game::story_fx::Texture,
    effect: Option<roses_game::story_fx::LightEffect>,
    /// The stage whose task ran (one launch per stage visit).
    ran: Option<String>,
    /// The layer holds a beam (cleared once when it ends).
    shown: bool,
}

/// c401's inputs: CMODULE.MRG and the executable from the import's duel
/// arena part (import-game made after 7 Oct 2026), TEXEVE.MRG from its
/// cloud part. `None` (no beam, said once) without them.
fn light_beam(import: &roses_game::game::GameImport, clouds: Option<&std::path::Path>) -> Option<LightBeam> {
    let loaded = import
        .duel_arena
        .as_deref()
        .map(roses_game::duel_arena::load_cmodule)
        .transpose()
        .unwrap_or_else(|e| {
            eprintln!("c401 light beam: {e}");
            None
        })
        .flatten();
    let Some((executable, cmodule)) = loaded else {
        println!("Story light beam (c401): this import has no CMODULE.MRG (re-import the ISO to add it); the beam is not drawn");
        return None;
    };
    let texeve = clouds
        .map(|c| c.join("TEXEVE.MRG"))
        .filter(|p| std::fs::metadata(p).is_ok_and(|m| m.len() <= 8 * 1024 * 1024))
        .and_then(|p| std::fs::read(p).ok())?;
    let texture = roses_game::story_fx::Texture::from_texeve(&texeve, 0x1A)?;
    Some(LightBeam { executable, cmodule, texeve, texture, effect: None, ran: None, shown: false })
}

/// c401's task frame n for this update: event 10040's subphase 0 spawns it
/// with the entry (0x21E20C), n = the entry's update + 1 (the flash, n =
/// 32, with the camera settled at level 0, `transition::FLASH_START_UPDATE`
/// [emu `modes/older/c401/emu`]);
/// 11162 / 12132 launch it on motion update 2 (the flash, n = 32, at
/// `C401_FLASH_UPDATE`).
fn c401_frame(opening: &OpeningState) -> Option<(String, u32)> {
    match &opening.transition {
        Some(Transition::SimonEntry(entry)) => Some(("simon-entry".into(), entry.updates())),
        Some(_) => None,
        None => {
            let stage = opening.preview.model.stage().id();
            if opening.world() != World::StoryScene0 || !matches!(stage, "map-story-11162" | "map-story-12132") {
                return None;
            }
            let m = opening.preview.active_motion()?;
            Some((stage.to_owned(), m.update.checked_sub(C401_FLASH_UPDATE - 32)?))
        }
    }
}

/// Run c401 as original code once per update with the scene's camera and
/// draw its beam into the composite's additive layer (the flash sprite is
/// drawn natively: `story_effects`, `advance_transition`).
fn story_beam(
    opening: Option<Res<OpeningState>>,
    rig: Option<Res<WorldRig>>,
    beam: Option<ResMut<LightBeam>>,
    composites: Res<Assets<CompositeMaterial>>,
    draws: Query<(&WorldDraw, &MeshMaterial3d<CapturedMaterial>)>,
    materials: Res<Assets<CapturedMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(opening), Some(mut beam)) = (opening, beam) else { return };
    let beam = &mut *beam;
    let frame = c401_frame(&opening);
    if frame.is_none() {
        beam.effect = None;
        beam.ran = None;
    }
    let camera = rig.as_ref().and_then(|r| r.last).map(|pose| {
        let (eye, target) = pose.eye_target();
        roses_game::duel_opening_fx::board_camera(eye, target)
    });
    if let (Some((stage, n)), Some(camera)) = (frame, camera) {
        if beam.ran.as_deref() != Some(stage.as_str()) {
            beam.ran = Some(stage);
            beam.effect = roses_game::story_fx::LightEffect::launch(&beam.executable, &beam.cmodule, &beam.texeve, camera);
            if beam.effect.is_none() {
                eprintln!("c401: the light effect did not load");
            }
        }
        if let Some(fx) = beam.effect.as_mut() {
            // Catch up to this update's task frame (10040's task starts
            // before the entry; nothing is drawn before n = 16).
            while fx.updates <= n {
                if !fx.update(camera) {
                    break;
                }
            }
        }
    }
    let drawing = beam.effect.as_ref().is_some_and(|fx| fx.beam().next().is_some());
    if !drawing && !beam.shown {
        return;
    }
    let layer = match beam.effect.as_ref().filter(|_| drawing) {
        Some(fx) => {
            // The shown world's opaque draws at their current clip.
            let story_world = opening.world() == World::StoryScene0;
            let triangles = draws
                .iter()
                .filter(|(d, _)| d.story == story_world)
                .filter_map(|(d, m)| materials.get(&m.0).filter(|m| m.params.x < 0.5).map(|m| (d, m.clip_from_source_gs)))
                .flat_map(|(d, clip)| {
                    let (positions, indices) = &*d.occluder;
                    let clip_of = move |i: u32| clip.mul_vec4(Vec3::from_array(positions[i as usize]).extend(1.0)).to_array();
                    indices.chunks_exact(3).map(move |t| [clip_of(t[0]), clip_of(t[1]), clip_of(t[2])])
                });
            let depth = roses_game::story_fx::depth_buffer(triangles, BEAM_TOP, BEAM_ROWS);
            roses_game::story_fx::rasterise(fx.beam(), &beam.texture, BEAM_TOP, BEAM_ROWS, Some(&depth))
        }
        None => vec![0; BEAM_WIDTH * BEAM_ROWS * 4],
    };
    beam.shown = drawing;
    let handle = opening.composite.as_ref().and_then(|h| composites.get(h)).map(|m| m.beam.clone());
    if let Some(mut image) = handle.and_then(|h| images.get_mut(&h)) {
        *image = beam_image(layer);
    }
}

/// c401's flash starts at task frame 32 (0x1E001B0). The task starts on
/// the update after the C2 update (motion update 2) [emu trace-seto-cam
/// f773/f774, white trace-brest f811]; n = 0 is taken as that update (the
/// executor 0x1F52E0 runs in its render pass) [H: ±1 update].
const C401_FLASH_UPDATE: u32 = 34;
/// 0x2217D0 / 0x222CF4: c402 starts once W+64 ≥ 2001.
const C402_LEVEL: f64 = 2001.0;

/// Apply the current stage or transition to world, overlay and window title.
fn present(
    opening: &mut OpeningState,
    clouds: Option<&mut CloudAnimation>,
    composites: &mut Assets<CompositeMaterial>,
    images: &mut Assets<Image>,
    cameras: &mut Query<&mut Camera>,
    windows: &mut Query<&mut Window, With<PrimaryWindow>>,
) -> Result<(), String> {
    let world = opening.world();
    let mut cloud_target = None;
    let mut converging = false;
    if let Some(clouds) = clouds {
        // The ending's c402 draws over the story world (`story_effects`).
        converging = world == World::StoryScene0 && clouds.converging.is_some();
        let active = world == World::Clouds || converging;
        clouds.enabled = active;
        if let Some(camera) = clouds.camera.and_then(|id| cameras.get_mut(id).ok()) {
            camera.into_inner().is_active = active;
        }
        cloud_target = clouds.target.clone();
    }
    let target = match world {
        World::Clouds => cloud_target,
        World::StoryScene0 if converging && opening.story_world => cloud_target,
        World::Simon => opening.simon_target.clone(),
        World::StoryScene0 if opening.story_world => opening.simon_target.clone(),
        World::Black | World::StoryScene0 => opening.black_target.clone(),
    };
    if let (Some(target), Some(mut material)) = (
        target,
        opening
            .composite
            .as_ref()
            .and_then(|h| composites.get_mut(h)),
    ) {
        material.world = target;
    }
    // Text/portraits are hidden during a handoff: the history text has faded
    // (display phase 12) and Simon's dialogue starts only after the entry.
    let overlay = if opening.transition.is_some() {
        roses_client::Image {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
        }
    } else {
        opening.preview.overlay()?
    };
    if let Some(mut image) = opening.overlay.as_ref().and_then(|h| images.get_mut(h)) {
        *image = ui_image(overlay);
    }
    let title = opening.caption();
    for mut window in windows.iter_mut() {
        window.title = title.clone();
    }
    Ok(())
}

/// One original main-loop update of the scene handoff, on the cloud clock.
fn advance_transition(
    opening: Option<ResMut<OpeningState>>,
    clouds: Option<ResMut<CloudAnimation>>,
    mut cloud_materials: ResMut<Assets<CloudMaterial>>,
    mut composites: ResMut<Assets<CompositeMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<&mut Camera>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut rig: Option<ResMut<WorldRig>>,
    mut sound: Option<ResMut<duel_board_render::DuelSound>>,
    mut exit: MessageWriter<AppExit>,
) {
    use roses_game::duel_audio::AudioCue;
    let (Some(mut opening), Some(mut clouds)) = (opening, clouds) else {
        return;
    };
    if !clouds.animate || clouds.settled() {
        return;
    }
    let (frame, next) = match opening.transition.as_mut() {
        None => return,
        Some(Transition::HistoryExit(exit)) => {
            let frame = exit.step();
            clouds.exit_requested |= frame.cloud_exit_requested;
            if let Some(mut material) = clouds
                .material
                .as_ref()
                .and_then(|h| cloud_materials.get_mut(h))
            {
                material.settings.fade_alpha.x = i32::from(frame.overlay_alpha);
            }
            if frame.simon_music_cue {
                // 0x217110(25, 0): 24 fades out, then 25 [F UI-SOUNDS.md 2.1].
                if let (Some(sound), Some(music)) = (sound.as_mut(), opening.preview.story_music("simon-music")) {
                    add_story_track(sound, &opening.preview, &music);
                    sound.cue(AudioCue::Music { track: 25, keep: false });
                }
            }
            (
                frame,
                frame
                    .done
                    .then(|| Some(Transition::SimonEntry(SimonEntry::new()))),
            )
        }
        Some(Transition::SimonEntry(entry)) => {
            let frame = entry.step();
            if let (true, Some(sound)) = (frame.thunder, sound.as_mut()) {
                for id in [0x22, 0x23] {
                    sound.cue(AudioCue::Se { slot: 0, id, skip: false });
                }
            }
            (frame, frame.done.then_some(None))
        }
    };
    let entering_simon = matches!(next, Some(Some(_)));
    let cover = if matches!(opening.transition, Some(Transition::SimonEntry(_))) && !frame.done {
        let [r, g, b] = frame.overlay_color.rgb().map(|v| f32::from(v) / 255.);
        Vec4::new(r, g, b, f32::from(frame.overlay_alpha) / 128.)
    } else if entering_simon {
        // Event 10040 begins fully covered by its black overlay.
        Vec4::new(0., 0., 0., 1.)
    } else {
        Vec4::ZERO
    };
    if let Some(mut material) = opening
        .composite
        .as_ref()
        .and_then(|h| composites.get_mut(h))
    {
        material.cover = cover;
        let [r, g, b] = roses_game::transition::FLASH_RGB
            .map(|c| (u32::from(c) * u32::from(frame.flash_alpha) / 128) as f32 / 255.);
        material.flash = Vec4::new(r, g, b, 0.);
    }
    if let Some(rig) = rig.as_mut() {
        if entering_simon {
            // Event 10040 init: level full, pan byte cleared (0x21E294).
            rig.level = camera::LEVEL_FULL;
            rig.simon_pan = 0;
        } else {
            rig.level = f64::from(frame.camera_level);
        }
    }
    let Some(next) = next else {
        return;
    };
    opening.transition = next;
    if let Err(error) = present(
        &mut opening,
        Some(clouds.into_inner()),
        &mut composites,
        &mut images,
        &mut cameras,
        &mut windows,
    ) {
        eprintln!("Overlay failed: {error}");
        exit.write(AppExit::error());
    }
}

/// The opening's music on the game's one mixer (`DuelSound`): each stage's
/// request as the original makes it (`opening::stage_music`), the history
/// exit's track 25 from `advance_transition`. The story import's pinned
/// tracks 24 / 25 join the mixer's tracks on first use.
fn sync_music(
    opening: Option<Res<OpeningState>>,
    sound: Option<ResMut<duel_board_render::DuelSound>>,
    mut shown: Local<Option<String>>,
) {
    let (Some(opening), Some(mut sound)) = (opening, sound) else {
        return;
    };
    if opening.is_added() {
        *shown = None;
    }
    if opening.transition.is_some() {
        return;
    }
    let stage = opening.preview.model.stage();
    if shown.as_deref() == Some(stage.id()) {
        return;
    }
    *shown = Some(stage.id().to_owned());
    if let Some(music) = stage.music() {
        add_story_track(&mut sound, &opening.preview, music);
    }
    for cue in roses_game::opening::stage_music(stage.id()) {
        sound.cue(cue);
    }
}

/// Hand a story WAV (24 kHz stereo, loop start to its end) to the mixer as
/// its track number.
fn add_story_track(
    sound: &mut duel_board_render::DuelSound,
    preview: &OpeningPreview,
    music: &roses_client::Music,
) {
    let Some(track) = roses_game::opening::story_track(&music.asset) else {
        eprintln!("Audio: {} has no known track number; not played", music.asset);
        return;
    };
    if sound.has_track(track) {
        return;
    }
    let Some(wave) = preview.loaded.assets.get(&music.asset).and_then(|a| a.pcm16.clone()) else {
        eprintln!("Audio: {} has no decoded PCM; not played", music.asset);
        return;
    };
    if wave.channels() != 2 || wave.sample_rate() != 24_000 {
        eprintln!("Audio: {} is not 24 kHz stereo; not played", music.asset);
        return;
    }
    let samples: std::sync::Arc<[i16]> =
        wave.data().chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]])).collect();
    let frames = wave.frame_count() as usize;
    let looped = match (music.looped, music.loop_start_frame) {
        (true, Some(start)) => Some((start as usize, frames)),
        (true, None) => Some((0, frames)),
        (false, _) => None,
    };
    sound.insert_track(std::sync::Arc::new(roses_client::sound_mix::MusicPcm { track, samples, looped }));
}

/// Intro movie: the disc's PSS through the built-in decoder (movie_play),
/// or a legacy converted MP4 through the shared client's MFPlay player.
#[derive(Default)]
struct MoviePlayback {
    #[cfg(windows)]
    player: Option<roses_client::movie::MoviePlayer>,
    native: Option<roses_client::movie_native::NativeMovie>,
    failed: bool,
    reported: bool,
}
fn sync_movie(
    opening: Option<ResMut<OpeningState>>,
    mut movie: NonSendMut<MoviePlayback>,
    clouds: Option<ResMut<CloudAnimation>>,
    mut composites: ResMut<Assets<CompositeMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<&mut Camera>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
    sound: Option<Res<duel_board_render::DuelSound>>,
) {
    let Some(mut opening) = opening else {
        return;
    };
    let asset = match opening.preview.model.stage() {
        roses_client::Stage::Movie { asset, .. } => Some(asset.clone()),
        _ => None,
    };
    let native_path = asset
        .as_ref()
        .and_then(|a| opening.preview.loaded.assets.get(a))
        .map(|a| a.path.clone())
        .filter(|p| movie_play::is_native(p));
    if native_path.is_none() {
        movie.native = None; // stage left (ended or skipped): stops its sound
    }
    if let Some(path) = native_path {
        if movie.failed {
            return;
        }
        let overlay = opening.overlay.as_ref().and_then(|h| images.get_mut(h));
        let mixer = sound.as_ref().and_then(|s| s.mixer());
        match movie_play::update(&mut movie.native, &path, mixer, overlay.map(|i| i.into_inner())) {
            Ok(false) => {}
            Ok(true) => match opening.preview.movie_ended() {
                Ok(_) => {
                    if let Err(error) = present(
                        &mut opening,
                        clouds.map(|c| c.into_inner()),
                        &mut composites,
                        &mut images,
                        &mut cameras,
                        &mut windows,
                    ) {
                        eprintln!("Overlay failed: {error}");
                        exit.write(AppExit::error());
                    }
                }
                Err(error) => opening.blocked = Some(error),
            },
            Err(error) => {
                eprintln!("Movie: {error}");
                movie.native = None;
                movie.failed = true;
                opening.blocked = Some(format!("intro movie unavailable: {error}"));
            }
        }
        return;
    }
    #[cfg(windows)]
    {
        let Some(asset) = asset else {
            movie.player = None;
            return;
        };
        if movie.failed {
            return;
        }
        if movie.player.is_none() {
            let Some(path) = opening
                .preview
                .loaded
                .assets
                .get(&asset)
                .map(|a| a.path.clone())
            else {
                eprintln!("Movie asset {asset} is missing");
                exit.write(AppExit::error());
                return;
            };
            // The video recorder draws nothing on screen: an old import's
            // MP4 intro (a player window) is not recorded.
            if duel_board_render::NO_SOUND_OUTPUT.load(std::sync::atomic::Ordering::SeqCst) {
                movie.failed = true;
                opening.blocked = Some("this import's intro is an old converted MP4, which the recorder cannot draw off-screen; re-import the ISO for the disc's own movie".into());
                return;
            }
            match roses_client::movie::MoviePlayer::start(&path, (4, 3)) {
                Ok(player) => movie.player = Some(player),
                Err(error) => {
                    // Keep the stage explicit rather than silently skipping it.
                    eprintln!("Movie: {error}");
                    movie.failed = true;
                    opening.blocked = Some(format!("intro movie unavailable: {error}"));
                    return;
                }
            }
        }
        let Some(player) = movie.player.as_mut() else {
            return;
        };
        player.fit();
        match player.poll() {
            Ok(roses_client::movie::MovieStatus::Ended) => {
                movie.player = None;
                match opening.preview.movie_ended() {
                    Ok(_) => {
                        if let Err(error) = present(
                            &mut opening,
                            clouds.map(|c| c.into_inner()),
                            &mut composites,
                            &mut images,
                            &mut cameras,
                            &mut windows,
                        ) {
                            eprintln!("Overlay failed: {error}");
                            exit.write(AppExit::error());
                        }
                    }
                    Err(error) => opening.blocked = Some(error),
                }
            }
            Ok(roses_client::movie::MovieStatus::Playing) if !movie.reported => {
                movie.reported = true;
                eprintln!("Movie: playback started");
            }
            Ok(_) => (),
            Err(error) => {
                eprintln!("Movie: {error}");
                movie.player = None;
                movie.failed = true;
                opening.blocked = Some(error);
            }
        }
    }
    #[cfg(not(windows))]
    if asset.is_some() && !movie.failed {
        movie.failed = true;
        opening.blocked = Some("intro movie playback requires Windows Media Foundation".into());
    }
}

/// Recompose every captured draw for the native camera pose of this update.
fn update_world_pose(
    opening: Option<Res<OpeningState>>,
    clouds: Option<Res<CloudAnimation>>,
    rig: Option<ResMut<WorldRig>>,
    mut draws: Query<(&WorldDraw, &MeshMaterial3d<CapturedMaterial>, &mut Visibility)>,
    mut materials: ResMut<Assets<CapturedMaterial>>,
) {
    let (Some(opening), Some(mut rig)) = (opening, rig) else {
        return;
    };
    // Only one of the two worlds is drawn into the world target.
    let story_world = opening.world() == World::StoryScene0;
    if rig.showing_story != Some(story_world) {
        rig.showing_story = Some(story_world);
        for (draw, _, mut visibility) in &mut draws {
            *visibility = if draw.story == story_world {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
    // Frozen cloud captures keep the captured opening pose; the story world
    // always follows its event's camera.
    if !story_world && clouds.as_ref().is_some_and(|c| !c.animate || c.settled()) {
        return;
    }
    let pose = match opening.world() {
        World::Black => return,
        World::StoryScene0 => {
            // Story camera 0x23A02C by W+53B5 [F, emu trace-*-cam.json]:
            // 11160/11180 mode 3 and 11162 mode 2 = the Simon formula with
            // the page pan (±10 per update, from 0 at the event's start);
            // 11162 enters at level 0x1000 and lowers it 256 per update;
            // 11184 mode 1: level +16 per update to 0x1000, target y 20.2
            // kept from mode 3, the world spinning −0x300 per update.
            let stage = opening.preview.model.stage().id();
            if rig.stage != stage {
                rig.stage = stage.to_owned();
                rig.simon_pan = 0;
                rig.story_angle = 0;
                rig.level = if matches!(stage, "map-story-11162" | "map-story-12132") {
                    camera::LEVEL_FULL
                } else {
                    0.0
                };
                // 12156 enters with the rise 12154 started behind its
                // paintings: W+64 1040, W+53F6 -0x4000 [emu
                // claude-story-white trace-manawyddan f1318].
                if stage == "map-story-12156" {
                    rig.level = 1040.0;
                    rig.story_angle = -0x4000;
                }
            }
            if matches!(stage, "map-story-11184" | "map-story-12156") {
                rig.level = (rig.level + 16.0).min(camera::LEVEL_FULL);
                rig.story_angle = rig.story_angle.wrapping_sub(0x300);
                Pose::Overhead {
                    level: rig.level,
                    angle: rig.story_angle,
                    target_y: f64::from(20.2_f32),
                }
            } else if let Some(cam) = opening.preview.active_motion().map(|m| m.camera()) {
                // The moving stage's own W+64, W+5439 and shake (11160's
                // and 12130's Seto, the 11162/12132/12150 effect waits).
                rig.level = f64::from(cam.level);
                rig.simon_pan = cam.pan.clamp(-100, 100) as i8;
                Pose::Story {
                    level: rig.level,
                    pan: cam.pan,
                    shake: cam.shake,
                }
            } else {
                rig.level = (rig.level - 256.0).max(0.0);
                let target = opening.preview.pan_target();
                rig.simon_pan = if rig.simon_pan < target {
                    (rig.simon_pan + 10).min(target)
                } else {
                    (rig.simon_pan - 10).max(target)
                };
                Pose::Simon {
                    level: rig.level,
                    pan: rig.simon_pan,
                }
            }
        }
        World::Clouds => {
            // 0x23A17C: the spin runs every mode-1 update.
            rig.history_angle = rig.history_angle.wrapping_sub(0x300);
            if opening.transition.is_none() {
                rig.level = camera::LEVEL_FULL;
            }
            Pose::History {
                level: rig.level,
                angle: rig.history_angle,
            }
        }
        World::Simon => {
            let stage = opening.preview.model.stage().id();
            if rig.stage != stage {
                // Event 10041 init clears the pan byte (0x21E5C0).
                if stage == "simon-post-name" {
                    rig.simon_pan = 0;
                }
                rig.stage = stage.to_owned();
            }
            if opening.transition.is_none() {
                rig.level = 0.0;
                // The dialogue layout moves the pan byte ±10 per update toward
                // the page's target (0x2321A0): +100 Simon (layout 2), −100
                // top speaker panel (layout 3), 0 otherwise. Its exact start
                // relative to page 1 is unverified.
                let target = opening.preview.pan_target();
                rig.simon_pan = if rig.simon_pan < target {
                    (rig.simon_pan + 10).min(target)
                } else {
                    (rig.simon_pan - 10).max(target)
                };
            }
            Pose::Simon {
                level: rig.level,
                pan: rig.simon_pan,
            }
        }
    };
    // The animated models (the sky's slow turn, 0x1EE6E0 once per update
    // after the draw): restart where the world's objects are created.
    let world = opening.world();
    let w = usize::from(story_world);
    if rig.animation_world != Some(world) {
        let restart = matches!(world, World::Simon | World::StoryScene0);
        rig.animation_world = Some(world);
        if restart {
            for f in &mut rig.animation_frames[w] {
                *f = 1;
            }
        }
    }
    let animating = matches!(world, World::Simon | World::StoryScene0) && !rig.animation_frames[w].is_empty();
    let frames = rig.animation_frames[w].clone();
    if animating {
        let next: Vec<u32> = rig.animations[w]
            .iter()
            .zip(&frames)
            .map(|(a, &f)| a.next_frame(f))
            .collect();
        rig.animation_frames[w] = next;
    }
    let pose_changed = rig.last != Some(pose);
    if !pose_changed && !animating {
        return;
    }
    rig.last = Some(pose);
    for (draw, handle, _) in &draws {
        if draw.story != story_world || !(pose_changed || draw.animation.is_some()) {
            continue;
        }
        let captured = match draw.animation {
            Some(i) if animating => rig.animations[w]
                .get(i)
                .zip(frames.get(i))
                .and_then(|(a, &f)| a.clip(f))
                .map_or(draw.captured, |m| m.map(|column| column.map(f64::from))),
            _ => draw.captured,
        };
        let Some(clip) = rig.rig.clip(pose, &captured) else {
            continue;
        };
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.clip_from_source_gs =
                Mat4::from_cols_array_2d(&clip.map(|column| column.map(|v| v as f32)));
        }
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<CapturedMaterial>>,
    mut composites: ResMut<Assets<CompositeMaterial>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut cloud_materials: ResMut<Assets<CloudMaterial>>,
    clouds: Option<ResMut<CloudAnimation>>,
    input: Res<InputScene>,
    mut exit: MessageWriter<AppExit>,
    capture: Option<ResMut<CaptureState>>,
    bridge: Option<Res<CaptureBridge>>,
    opening: Option<ResMut<OpeningState>>,
    story: Option<Res<StoryScene>>,
) {
    let cloud_target = if let Some(mut clouds) = clouds {
        let (mesh, material, atlas, target) = match cloud_render::setup_clouds(
            &mut commands,
            &mut images,
            &mut meshes,
            &mut buffers,
            &mut cloud_materials,
            &mut clouds,
        ) {
            Ok(created) => created,
            Err(error) => {
                eprintln!("Cloud setup failed: {error}");
                exit.write(AppExit::error());
                return;
            }
        };
        if let Some(bridge) = &bridge {
            let mut progress = bridge.0.lock().expect("Capture progress mutex poisoned");
            progress.meshes.push(mesh.id());
            progress.materials.push(material.id().untyped());
            progress.textures.extend([atlas.id(), target.id()]);
            progress.buffers.extend([
                clouds.vertices.as_ref().unwrap().id(),
                clouds.bins.as_ref().unwrap().id(),
            ]);
            progress.expect_clouds = clouds.enabled;
        }
        Some((target, material))
    } else {
        None
    };
    let (cloud_target, cloud_material) = match cloud_target {
        Some((target, material)) => (Some(target), Some(material)),
        None => (None, None),
    };
    let projection = input
        .0
        .scene
        .gs_projection_columns
        .map(|column| column.map(f64::from));
    match camera::Rig::new(
        projection,
        Pose::Simon {
            level: 0.0,
            pan: 100,
        },
    ) {
        Some(rig) => commands.insert_resource(WorldRig {
            rig,
            history_angle: 0,
            // The story init 0x21BF50 clears the work area (0x21CEC0:
            // 0x1959C8(W, 0x54B0)), so a story entered afresh (Continue
            // after an ending at 10090, the tutorial's return) starts with
            // the pan byte W+5439 = 0 and moves it ±10 per update to the
            // page's target [F; emu claude-ending run_continue: W+5439 0
            // after 0x21BF50(1)]; the new game's 10040 clears it as well.
            simon_pan: 0,
            level: camera::LEVEL_FULL,
            last: None,
            stage: String::new(),
            story_angle: 0,
            showing_story: None,
            animations: [
                input.0.scene.animations.clone(),
                story.as_ref().map_or_else(Vec::new, |s| s.0.scene.animations.clone()),
            ],
            animation_frames: [
                vec![1; input.0.scene.animations.len()],
                vec![1; story.as_ref().map_or(0, |s| s.0.scene.animations.len())],
            ],
            animation_world: None,
        }),
        None => {
            eprintln!("Captured projection/view is not invertible");
            exit.write(AppExit::error());
            return;
        }
    }
    let worlds = std::iter::once((&input.0, false)).chain(story.as_ref().map(|s| (&s.0, true)));
    for (scene, story_draw) in worlds {
    for (draw, png) in scene.scene.meshes.iter().zip(&scene.textures) {
        let address = |mode| match mode {
            AddressMode::Clamp => ImageAddressMode::ClampToEdge,
            AddressMode::Repeat => ImageAddressMode::Repeat,
        };
        let sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: address(draw.sampler.u),
            address_mode_v: address(draw.sampler.v),
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Nearest,
            lod_min_clamp: 0.,
            lod_max_clamp: 0.,
            ..default()
        });
        let texture = match Image::from_buffer(
            png,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            false,
            sampler,
            RenderAssetUsages::default(),
        ) {
            Ok(image) => images.add(image),
            Err(error) => {
                eprintln!("Texture decode failed: {error}");
                exit.write(AppExit::error());
                return;
            }
        };
        let colors: Vec<[f32; 4]> = draw
            .vertex_rgba_gs
            .iter()
            .map(|c| c.map(f32::from))
            .collect();
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, draw.positions_raw.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, draw.uv.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(draw.indices.clone()));
        let material = CapturedMaterial {
            clip_from_source_gs: Mat4::from_cols_array_2d(&draw.clip_from_source_gs_columns),
            texture,
            params: Vec4::new(if draw.alpha_blend { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0),
        };
        let mesh = meshes.add(mesh);
        if let Some(bridge) = &bridge {
            let mut progress = bridge.0.lock().expect("Capture progress mutex poisoned");
            progress.meshes.push(mesh.id());
            progress.textures.push(material.texture.id());
        }
        let material = materials.add(material);
        if let Some(bridge) = &bridge {
            bridge
                .0
                .lock()
                .expect("Capture progress mutex poisoned")
                .materials
                .push(material.id().untyped());
        }
        commands.spawn((
            Name::new(format!(
                "Captured object {} draw {}",
                draw.object_slot, draw.draw_index
            )),
            WorldDraw {
                captured: draw
                    .clip_from_source_gs_columns
                    .map(|column| column.map(f64::from)),
                story: story_draw,
                animation: scene
                    .scene
                    .animations
                    .iter()
                    .position(|a| a.object_slot == draw.object_slot),
                occluder: std::sync::Arc::new((draw.positions_raw.clone(), draw.indices.clone())),
            },
            if story_draw {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            NoFrustumCulling,
            RenderLayers::layer(0),
        ));
    }
    }
    // The custom vertex shader supplies captured clip coordinates. This camera
    // provides Bevy's render view only; it does not invent a game camera pose.
    let world_camera = commands
        .spawn((
            Camera3d::default(),
            Transform::IDENTITY,
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
            RenderLayers::layer(0),
        ))
        .id();
    let presentation = if let Some(mut opening) = opening {
        let rgba = match opening.preview.overlay() {
            Ok(rgba) => rgba,
            Err(error) => {
                eprintln!("Overlay failed: {error}");
                exit.write(AppExit::error());
                return;
            }
        };
        let rgba = if opening.transition.is_some() {
            roses_client::Image {
                width: 1,
                height: 1,
                rgba: vec![0; 4],
            }
        } else {
            rgba
        };
        let overlay = images.add(ui_image(rgba));
        opening.overlay = Some(overlay.clone());
        let mut target = Image::new_target_texture(512, 512, TextureFormat::Rgba8UnormSrgb, None);
        target.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            ..default()
        });
        let world = images.add(target);
        opening.simon_target = Some(world.clone());
        opening.story_world = story.is_some();
        // The history world renders under the navy fade and clouds.
        if let Some(material) = cloud_material
            .as_ref()
            .and_then(|h| cloud_materials.get_mut(h))
        {
            material.into_inner().world = world.clone();
        }
        commands.entity(world_camera).insert((
            Camera {
                order: -1,
                ..default()
            },
            RenderTarget::Image(world.clone().into()),
        ));
        let black = images.add(Image::new_fill(
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        ));
        opening.black_target = Some(black.clone());
        let quad = meshes.add(Rectangle::new(2., 2.));
        let beam = images.add(beam_image(vec![0; BEAM_WIDTH * BEAM_ROWS * 4]));
        let composite = composites.add(CompositeMaterial {
            world: match opening.world() {
                World::Clouds => match cloud_target {
                    Some(target) => target,
                    None => {
                        eprintln!("Historical stage requires the cloud renderer");
                        exit.write(AppExit::error());
                        return;
                    }
                },
                World::Simon => world.clone(),
                World::StoryScene0 if story.is_some() => world.clone(),
                World::Black | World::StoryScene0 => black.clone(),
            },
            overlay: overlay.clone(),
            cover: Vec4::ZERO,
            flash: Vec4::ZERO,
            beam: beam.clone(),
        });
        opening.composite = Some(composite.clone());
        if let Some(bridge) = &bridge {
            let mut progress = bridge.0.lock().expect("Capture progress mutex poisoned");
            progress.meshes.push(quad.id());
            progress
                .textures
                .extend([world.id(), overlay.id(), black.id(), beam.id()]);
            progress.materials.push(composite.id().untyped());
            progress.expect_composite = true;
        }
        commands.spawn((
            Mesh3d(quad),
            MeshMaterial3d(composite),
            Transform::IDENTITY,
            NoFrustumCulling,
            RenderLayers::layer(1),
        ));
        commands
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
                PresentationCamera,
            ))
            .id()
    } else {
        commands.entity(world_camera).insert(PresentationCamera);
        world_camera
    };
    if let Some(mut capture) = capture {
        let mut target = Image::new_target_texture(512, 512, TextureFormat::Rgba8UnormSrgb, None);
        target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let target = images.add(target);
        capture.target = Some(target.clone());
        commands
            .entity(presentation)
            .insert(RenderTarget::Image(target.into()));
    }
}
fn fit_view(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<&mut Camera, With<PresentationCamera>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let w = window.physical_width();
    let h = window.physical_height();
    if w == 0 || h == 0 {
        return;
    }
    let (vw, vh) = if u64::from(w) * 3 <= u64::from(h) * 4 {
        (w, w * 3 / 4)
    } else {
        (h * 4 / 3, h)
    };
    for mut camera in &mut cameras {
        camera.viewport = Some(Viewport {
            physical_position: UVec2::new((w - vw) / 2, (h - vh) / 2),
            physical_size: UVec2::new(vw.max(1), vh.max(1)),
            ..default()
        });
    }
}
fn exit_on_escape(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
fn trace_input(mut events: MessageReader<KeyboardInput>) {
    for event in events.read() {
        if event.state == ButtonState::Pressed && !event.repeat {
            eprintln!(
                "ROSES_INPUT key_code={:?} logical_key={:?}",
                event.key_code, event.logical_key
            );
        }
    }
}
/// `--game <game.json> --online <addr> [--seat N] [deck flags] [--capture
/// ...]`: the duel-board online client with every duel part of the user's
/// game import (board, duel rules data, models, result-screen pictures,
/// arenas, audio); the other flags pass through.
fn online_from_game(args: impl Iterator<Item = std::ffi::OsString>) -> std::result::Result<(), String> {
    let mut game = None;
    let mut rest = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--game" {
            game = Some(PathBuf::from(args.next().ok_or("--game requires game.json")?));
        } else {
            rest.push(arg);
        }
    }
    let game = game.ok_or("--online needs --game <game.json> (or --duel-board <manifest> --duel-play <duel import>)")?;
    let import = roses_game::game::GameImport::open(&game)?;
    let mut out: Vec<std::ffi::OsString> = vec![
        "--duel-board".into(),
        import.duel_board_manifest.clone().into(),
        "--duel-play".into(),
        import.duel.clone().into(),
        "--duel-models".into(),
        import.duel_models_manifest.clone().into(),
    ];
    if let Some(end) = &import.duel_end {
        out.extend(["--duel-end".into(), end.clone().into()]);
    }
    if let Some(arena) = &import.duel_arena {
        out.extend(["--duel-arena".into(), arena.clone().into()]);
    }
    if let Some(audio) = &import.duel_audio {
        out.extend(["--duel-audio".into(), audio.clone().into()]);
    }
    out.extend(rest);
    duel_board_render::run(out.into_iter())
}

fn run() -> std::result::Result<(), String> {
    if roses_game::faithful() {
        eprintln!(
            "Game: faithful mode{}: the original game only, no Online Play or other port additions (docs/ADDITIONS.md)",
            if cfg!(feature = "online") { "" } else { " (a build without the online feature)" }
        );
    }
    // `--record <out.mp4>` (the video recorder, a tool): the connected game
    // (`--game`) or the opening preview, headless.
    let (record_args, rest) = record::RecordArgs::take(std::env::args_os().skip(1).collect())?;
    if record_args.active()
        && std::env::args_os().any(|arg| {
            ["--duel-board", "--online", "--multi-preview", "--campaign-map", "--duel-model"].iter().any(|m| arg == *m)
        })
    {
        return Err("--record works with --game (the whole game) or --scene; not with the --duel-board, --online, --campaign-map or --duel-model previews".into());
    }
    // The duel board mode has its own input (no captured scene); see duel_board_render.rs.
    if std::env::args_os().any(|arg| arg == "--duel-board") {
        return duel_board_render::run(std::env::args_os().skip(1));
    }
    // An online duel from the connected game's import: the duel board with
    // the import's duel parts (see duel_board_render.rs, --online).
    if std::env::args_os().any(|arg| arg == "--online" || arg == "--multi-preview") {
        return online_from_game(std::env::args_os().skip(1));
    }
    if std::env::args_os().any(|arg| arg == "--campaign-map") {
        return campaign_map_render::run(std::env::args_os().skip(1));
    }
    if std::env::args_os().any(|arg| arg == "--duel-model") {
        return duel_model_render::run(std::env::args_os().skip(1));
    }
    let mut scene = None;
    let mut root = None;
    let mut validate_only = false;
    let mut capture = None;
    let mut capture_game_scene = None;
    let mut capture_map_event: Option<i16> = None;
    let mut capture_map_updates: Option<u32> = None;
    let mut opening_manifest = None;
    let mut start_stage = None;
    let mut replay = None;
    let mut cloud_source = None;
    let mut cloud_ticks = 0u32;
    let mut freeze_clouds = false;
    let mut capture_live_ticks = None;
    let mut game = None;
    let mut research_root = None;
    let mut end_replay = std::collections::VecDeque::new();
    let mut map_replay = std::collections::VecDeque::new();
    let mut exit_at_map = false;
    let mut exit_after_editor = false;
    let mut exit_at_duel = false;
    let mut editor_replay = std::collections::VecDeque::new();
    let mut game_seed = None;
    let mut computer_player = false;
    let mut tutorial_lesson: Option<Lesson> = None;
    let mut tutorial_auto = false;
    let mut tutorial_ticks = 0u32;
    let mut exit_at_story = false;
    let mut story_auto = false;
    let mut map_save: Option<roses_client::campaign_map::CampaignSave> = None;
    let mut args = rest.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--game") if game.is_none() => {
                game = Some(PathBuf::from(
                    args.next().ok_or("--game requires game.json")?,
                ));
            }
            Some("--capture-game-scene") if capture_game_scene.is_none() => {
                let value = args
                    .next()
                    .ok_or("--capture-game-scene requires map, story or duel")?;
                capture_game_scene = Some(match value.to_str() {
                    Some("map") => GameScene::Map,
                    // A story stage played from the campaign map (11020).
                    Some("story") => GameScene::Opening,
                    Some("duel") => GameScene::Duel,
                    Some("map-editor") => GameScene::MapEditor,
                    Some("online") if roses_game::extras() => GameScene::Online,
                    _ => return Err("--capture-game-scene requires map, story or duel".into()),
                });
            }
            Some("--capture-map-event") if capture_map_event.is_none() => {
                let value = args
                    .next()
                    .and_then(|v| v.to_str().and_then(|v| v.parse::<i16>().ok()))
                    .ok_or("--capture-map-event requires a map event number")?;
                capture_map_event = Some(value);
            }
            Some("--capture-map-updates") if capture_map_updates.is_none() => {
                let value = args
                    .next()
                    .and_then(|v| v.to_str().and_then(|v| v.parse::<u32>().ok()))
                    .filter(|v| (1..=20_000).contains(v))
                    .ok_or("--capture-map-updates requires 1..=20000")?;
                capture_map_updates = Some(value);
            }
            Some("--end-replay") if end_replay.is_empty() => {
                let text = args
                    .next()
                    .ok_or("--end-replay requires comma-separated presses")?
                    .into_string()
                    .map_err(|_| "--end-replay must be UTF-8")?;
                end_replay = roses_game::duel_play::parse_replay_pad(&text)?.into();
            }
            Some("--exit-at-map") => exit_at_map = true,
            Some("--exit-after-editor") => exit_after_editor = true,
            Some("--exit-at-duel") => exit_at_duel = true,
            Some("--editor-replay") if editor_replay.is_empty() => {
                let text = args
                    .next()
                    .ok_or("--editor-replay requires comma-separated presses")?
                    .into_string()
                    .map_err(|_| "--editor-replay must be UTF-8")?;
                editor_replay =
                    roses_game::duel_play::parse_replay_pad(&text)?.into();
            }
            Some("--exit-at-story") => exit_at_story = true,
            Some("--story-auto") => story_auto = true,
            // Test aid: the map starts from this campaign save (JSON of
            // `CampaignSave`, e.g. a return event at a later node) instead of
            // event 0 when the replay reaches the map.
            Some("--map-save") if map_save.is_none() => {
                let path = PathBuf::from(args.next().ok_or("--map-save requires a JSON file")?);
                let bytes = std::fs::read(&path).map_err(|e| format!("--map-save: {e}"))?;
                if bytes.len() > 64 * 1024 {
                    return Err("--map-save: file too large".into());
                }
                let save: roses_client::campaign_map::CampaignSave =
                    serde_json::from_slice(&bytes).map_err(|e| format!("--map-save: {e}"))?;
                if save.node_states.len() != roses_client::campaign_map::NODE_COUNT
                    || save.wins_by_opponent.len() < 32
                    || save.losses_by_opponent.len() < 32
                {
                    return Err("--map-save: node states or counters have the wrong length".into());
                }
                map_save = Some(save);
            }
            Some("--tutorial-auto") => tutorial_auto = true,
            Some("--tutorial-lesson") if tutorial_lesson.is_none() => {
                let n: usize = args
                    .next()
                    .ok_or("--tutorial-lesson requires 1..5")?
                    .to_string_lossy()
                    .parse()
                    .map_err(|_| "--tutorial-lesson must be 1..5")?;
                tutorial_lesson = Some(
                    *Lesson::ALL
                        .get(n.wrapping_sub(1))
                        .ok_or("--tutorial-lesson must be 1..5")?,
                );
            }
            Some("--tutorial-ticks") if tutorial_ticks == 0 => {
                tutorial_ticks = args
                    .next()
                    .ok_or("--tutorial-ticks requires a count")?
                    .to_string_lossy()
                    .parse()
                    .map_err(|_| "--tutorial-ticks must be a number")?;
            }
            Some("--game-computer-player") => computer_player = true,
            Some("--map-replay") if map_replay.is_empty() => {
                let text = args
                    .next()
                    .ok_or("--map-replay requires comma-separated pads")?
                    .into_string()
                    .map_err(|_| "--map-replay must be UTF-8")?;
                map_replay = roses_game::campaign_map::parse_replay(&text)?.into();
            }
            Some("--game-seed") if game_seed.is_none() => {
                game_seed = Some(
                    args.next()
                        .and_then(|v| v.to_str()?.parse::<u64>().ok())
                        .ok_or("--game-seed requires a number")?,
                );
            }
            Some("--research-root") if research_root.is_none() => {
                research_root = Some(PathBuf::from(
                    args.next().ok_or("--research-root requires a folder")?,
                ));
            }
            Some("--cloud-source") if cloud_source.is_none() => {
                cloud_source = Some(PathBuf::from(
                    args.next().ok_or("--cloud-source requires a path")?,
                ));
            }
            Some("--cloud-ticks") => {
                cloud_ticks = args
                    .next()
                    .ok_or("--cloud-ticks requires a count")?
                    .to_str()
                    .ok_or("Invalid cloud tick count")?
                    .parse()
                    .map_err(|_| "Invalid cloud tick count")?;
                if cloud_ticks > 10000 {
                    return Err("Cloud ticks exceed 10000".into());
                }
            }
            Some("--freeze-clouds") => freeze_clouds = true,
            Some("--capture-live-ticks") if capture_live_ticks.is_none() => {
                let ticks: u32 = args
                    .next()
                    .ok_or("--capture-live-ticks requires a count")?
                    .to_str()
                    .ok_or("Invalid live tick count")?
                    .parse()
                    .map_err(|_| "Invalid live tick count")?;
                if !(1..=1000).contains(&ticks) {
                    return Err("Live capture ticks must be 1..=1000".into());
                }
                capture_live_ticks = Some(ticks);
            }
            Some("--scene") => {
                scene = Some(PathBuf::from(args.next().ok_or("--scene requires a path")?))
            }
            Some("--data-root") => {
                root = Some(PathBuf::from(
                    args.next().ok_or("--data-root requires a path")?,
                ))
            }
            Some("--validate-only") => validate_only = true,
            Some("--opening-manifest") => {
                if opening_manifest.is_some() {
                    return Err("Duplicate --opening-manifest".into());
                }
                opening_manifest = Some(PathBuf::from(
                    args.next().ok_or("--opening-manifest requires a path")?,
                ));
            }
            Some("--start-stage") => {
                if start_stage.is_some() {
                    return Err("Duplicate --start-stage".into());
                }
                start_stage = Some(
                    args.next()
                        .ok_or("--start-stage requires an ID")?
                        .into_string()
                        .map_err(|_| "Stage must be UTF-8")?,
                );
            }
            Some("--replay") => {
                if replay.is_some() {
                    return Err("Duplicate --replay".into());
                }
                replay = Some(parse_replay(
                    &args
                        .next()
                        .ok_or("--replay requires comma-separated buttons")?
                        .into_string()
                        .map_err(|_| "Replay must be UTF-8")?,
                )?);
            }
            Some("--capture") => {
                if capture.is_some() {
                    return Err("--capture may only appear once".into());
                }
                capture = Some(PathBuf::from(
                    args.next().ok_or("--capture requires a new PNG path")?,
                ));
            }
            Some("--help") => {
                println!(
                    "roses-bevy-scene --duel-board <duel-board manifest.json> [--help]  (original duel board; see --duel-board --help)
roses-bevy-scene --game <game.json> [--research-root <workspace>] [--start-stage ...] [--replay ...] [--map-replay none*120,up*5,cross,...] [--end-replay none*70,cross,... ] [--exit-at-map] [--game-seed N] [--game-computer-player] [--tutorial-lesson 1|2|3] [--tutorial-auto] [--tutorial-ticks N] [--exit-at-story] [--story-auto] [--map-save <CampaignSave.json>] [--capture <new.png> --capture-game-scene map|story|duel [--capture-map-event E [--capture-map-updates N]]]  (the connected game; story 11000 continues at map event 100 (the voyage to Brest) and 11020 at 101 (the voyage to Milford Haven, reveal 102, free map); --capture-game-scene story captures the story stage the map plays (11020); --capture-map-event/--capture-map-updates hold a map capture until the map has spent N updates at event E; --map-save starts the map from that save once the replay reaches it (test aid); --story-auto pages the map's story events (Brest, Bosworth, Stonehenge) with Cross every eighth update; --map-replay scripts the campaign map's pad as synthetic key presses (one frame late), --end-replay scripts the result screens' and reels' pad, --exit-at-map quits once the map returns after the duel; the practice choice Yes plays the scripted tutorial on the duel board: --tutorial-lesson starts the series at that lesson, --tutorial-auto pages its messages (a fresh Cross on every fifth accepting poll), --tutorial-ticks runs N updates of the first lesson before its first frame, --exit-at-story quits once the story resumes at event 10060)
roses-bevy-scene --scene <captured-scene.json> --data-root <workspace> [--validate-only | --capture <new.png>] [--opening-manifest <manifest.json> [--start-stage intro|title|main-menu|historical-story|simon-arrival|name-entry|practice-choice|deck-choice|side-choice|<story or stand-in stage id>] [--replay cross,...] [--cloud-source <checkpoint.json> [--cloud-ticks 0..10000] [--freeze-clouds | --capture-live-ticks 1..1000]]]\nCapture renders 512x512 without a window. The intro movie plays in the window through the built-in PSS decoder (a legacy MP4 import uses Media Foundation; Start skips); title and main menu are overlay-only. --capture-live-ticks animates through the live upload path before capturing and prints a GPU/CPU cloud band check. Cloud source defaults the start to historical-story and resumes a private research checkpoint. --cloud-ticks advances from its first resumed frame; captures freeze that frame. Live clouds advance once per 25 Hz update (the original main loop spans two PAL fields; see UPDATE_HZ). The Simon camera is static; this preview has no audio. Enter advances; arrows navigate; X/A insert; Z deletes; Q/E change name page; S commits; Escape closes.
Video recorder (a tool, docs/ADDITIONS.md): add --record <new.mp4> [--record-size 1920x1080] [--record-fps 60] [--record-seconds S] [--record-keys 120:down,2.5s:x*10] [--record-realtime] to --game or --scene. Headless; frames and the game's own sound go to an H.264/AAC MP4 (Media Foundation); no sound device is opened. ROSES_RECORD_WAV=<file> also writes the sound as WAV."
                );
                return Ok(());
            }
            // Faithful mode: the original only (lib.rs `faithful`, read from
            // the arguments; the launcher's "Online Play and extras" off).
            Some("--faithful") => {}
            _ => return Err(format!("Unknown argument: {}", arg.to_string_lossy())),
        }
    }
    // The connected game: every input comes from one `roses import-game`
    // folder, except the two private captures still used for the opening.
    if capture_game_scene.is_some() && (game.is_none() || capture.is_none()) {
        return Err("--capture-game-scene requires --game and --capture".into());
    }
    let capture_map_gate = match (capture_map_event, capture_map_updates) {
        (None, None) => None,
        (Some(event), updates) if capture_game_scene == Some(GameScene::Map) => {
            Some((event, updates.unwrap_or(1)))
        }
        (Some(event), updates)
            if capture_game_scene == Some(GameScene::Opening)
                && u16::try_from(event)
                    .ok()
                    .and_then(roses_client::ending::Route::of_event)
                    .is_some() =>
        {
            Some((event, updates.unwrap_or(1)))
        }
        _ => {
            return Err(
                "--capture-map-event (and --capture-map-updates) require --capture-game-scene map"
                    .into(),
            );
        }
    };
    if capture_game_scene.is_some()
        && (exit_at_map || exit_at_duel || exit_after_editor || exit_at_story)
    {
        return Err("--capture-game-scene cannot be combined with early-exit flags".into());
    }
    let mut game_import = None;
    let mut cloud_root: Option<PathBuf> = None;
    if let Some(path) = &game {
        if opening_manifest.is_some() || scene.is_some() || cloud_source.is_some() || root.is_some()
        {
            return Err(
                "--game chooses its own inputs; use --research-root for the private captures"
                    .into(),
            );
        }
        let import = roses_game::game::GameImport::open(path)?;
        let research = research_root
            .as_deref()
            .map(roses_game::game::ResearchInputs::from_root)
            .unwrap_or_default();
        opening_manifest = Some(import.story_manifest.clone());
        start_stage = start_stage.or(Some("intro".into()));
        // Disc-derived Simon scene and cloud start when the import has them
        // (roses import-game steps 9-10); else the private research captures.
        let folder = |p: &PathBuf| p.parent().map(std::path::Path::to_path_buf);
        match (&import.opening_scene, &import.clouds) {
            (Some(scene_json), Some(clouds_json)) => {
                scene = Some(scene_json.clone());
                root = folder(scene_json);
                cloud_source = Some(clouds_json.clone());
                cloud_root = folder(clouds_json);
            }
            _ => {
                scene = research.scene;
                cloud_source = research.cloud_source;
                root = research.data_root;
            }
        }
        if scene.is_none() {
            return Err("This game import has no Simon scene (made before import-game step 9); re-import the ISO, or start with --research-root <workspace> that has the private capture work/research/story/mesh-scene-simon.json".into());
        }
        game_import = Some(import);
    } else if research_root.is_some()
        || !end_replay.is_empty()
        || !map_replay.is_empty()
        || exit_at_map
        || game_seed.is_some()
        || computer_player
    {
        return Err(
            "--research-root, --end-replay, --map-replay, --exit-at-map, --game-seed and --game-computer-player are used only with --game"
                .into(),
        );
    }
    if opening_manifest.is_none() && (start_stage.is_some() || replay.is_some()) {
        return Err("--start-stage and --replay require --opening-manifest".into());
    }
    if cloud_source.is_some() && opening_manifest.is_none() {
        return Err("Cloud presentation requires --opening-manifest".into());
    }
    if cloud_source.is_none()
        && (cloud_ticks != 0 || freeze_clouds || start_stage.as_deref() == Some("historical-story"))
    {
        return Err("Historical presentation and cloud controls require --cloud-source".into());
    }
    if cloud_source.is_some() && start_stage.is_none() {
        start_stage = Some("historical-story".into());
    }
    if capture_live_ticks.is_some()
        && (capture.is_none() || cloud_source.is_none() || freeze_clouds)
    {
        return Err("--capture-live-ticks requires --capture and unfrozen --cloud-source".into());
    }
    if capture.is_some() && start_stage.as_deref() == Some("intro") {
        return Err(
            "The intro movie plays in a window only; headless capture cannot show it".into(),
        );
    }
    if validate_only && capture.is_some() {
        return Err("--validate-only and --capture cannot be combined".into());
    }
    let record_settings = if record_args.active() {
        if capture.is_some() || validate_only {
            return Err("--record cannot be combined with --capture or --validate-only".into());
        }
        Some(record_args.settings()?)
    } else {
        None
    };
    let recording = record_settings.is_some();
    if let Some(path) = &capture {
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
    let root = root.ok_or("--data-root is required")?;
    let loaded = load_scene(&scene.ok_or("--scene is required")?, &root)?;
    // The Stonehenge story events' world (import part `story-scene`).
    let story_scene = match game_import.as_ref().and_then(|g| g.story_scene.clone()) {
        Some(path) => {
            let folder = path.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
            Some(StoryScene(load_scene(&path, &folder)?))
        }
        None => None,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&loaded.report).map_err(|e| e.to_string())?
    );
    let mut replayed_to_map = None;
    let mut replayed_map_save = None;
    let mut replayed_to_tutorial = None;
    let opening = if let Some(path) = opening_manifest {
        let mut preview =
            OpeningPreview::open(&path, start_stage.as_deref(), cloud_source.is_some())?;
        // The story panels' talking faces read the executable's face tables.
        if let Some(import) = game_import.as_ref() {
            match roses_game::duel_play::load_executable(&import.duel)
                .and_then(|exe| roses_client::face_anim::FaceTables::from_executable(&exe))
            {
                Ok(tables) => preview.set_face_tables(tables),
                Err(e) => eprintln!("Story faces stay at frame 0: {e}"),
            }
        }
        // A replay whose final press leaves the last history page starts the
        // source-timed handoff (later presses would be ignored by the original).
        let mut history_exit = false;
        if let Some(replay) = replay {
            for (index, button) in replay.into_iter().enumerate() {
                let from_history = preview.model.stage().id() == "historical-story";
                match preview.input(button) {
                    // In the game, a replay may run the story to the map.
                    Err(_)
                        if game_import.is_some()
                            && preview.model.stage().id()
                                == roses_game::game::LAST_STORY_STAGE =>
                    {
                        replayed_to_map = Some(preview.model.campaign().clone());
                        break;
                    }
                    result => {
                        result.map_err(|e| {
                            format!("Replay stopped at input {} ({button:?}): {e}", index + 1)
                        })?;
                        // The story's end continues at map event 100 (the
                        // voyage), played by the connected game.
                        if let Some(event) = preview.take_map_event() {
                            if game_import.is_none() {
                                return Err(format!(
                                    "Replay stopped at input {} ({button:?}): the campaign map continues at event {event} in the connected game (--game)",
                                    index + 1
                                ));
                            }
                            replayed_map_save = Some(first_map_save(event, preview.model.campaign().side).map_err(|e| {
                                format!("Replay stopped at input {} ({button:?}): {e}", index + 1)
                            })?);
                            replayed_to_map = Some(preview.model.campaign().clone());
                            break;
                        }
                        if let Some(resume) = preview.take_tutorial() {
                            if game_import.is_none() {
                                return Err(format!(
                                    "Replay stopped at input {} ({button:?}): the scripted tutorial (practice Yes) is played by the connected game (--game); the story would resume at {resume}",
                                    index + 1
                                ));
                            }
                            replayed_to_tutorial = Some(resume);
                            break;
                        }
                    }
                }
                history_exit = from_history
                    && cloud_source.is_some()
                    && preview.model.stage().id() == "simon-arrival";
            }
        }
        // Validate actual overlay rendering even in the CPU-only inspection path.
        preview.loaded.render_overlay(&preview.model)?;
        preview.enable_motion(story_motion_enabled());
        println!(
            "{}\nOpening manifest SHA256: {}",
            preview.caption(),
            preview.loaded.manifest_sha256
        );
        Some(OpeningState {
            preview,
            overlay: None,
            blocked: None,
            simon_target: None,
            story_world: false,
            black_target: None,
            composite: None,
            transition: history_exit.then(|| Transition::HistoryExit(HistoryExit::new())),
        })
    } else {
        None
    };
    let clouds = if let Some(path) = cloud_source {
        let source = load_cloud_source(&path, cloud_root.as_ref().unwrap_or(&root))?;
        println!(
            "Cloud input SHA256: {}\nCloud source state SHA256: {}",
            source.input_sha256, source.source_state_sha256
        );
        match &source.boot_schedule {
            Some(schedule) => println!(
                "Cloud start derived from the disc: newlib seed 1, schedule {schedule:?} (the recorded dwell time; the original's depends on the player)"
            ),
            None => println!(
                "Cloud animation resumes a captured state; native boot seed/timing are not verified."
            ),
        }
        let live = capture_live_ticks.is_some() || (capture.is_none() && !freeze_clouds);
        let mut animation = CloudAnimation::new(source, cloud_ticks, live)?;
        // Exercise the same per-frame upload path as the interactive preview.
        animation.stop_at = capture_live_ticks.map(|n| animation.tick + n);
        animation.enabled = opening.as_ref().is_some_and(|o| o.world() == World::Clouds);
        println!(
            "Cloud state counter: {}; rendered quads: {}; relative steps: {}",
            animation.source.state.counter(),
            animation.quads.len(),
            animation.tick
        );
        Some(animation)
    } else {
        None
    };
    if validate_only {
        return Ok(());
    }
    // Device initialization happens synchronously inside add_plugins on some
    // platforms. Start the independent deadline before constructing plugins.
    // An N-player duel capture (`ROSES_MULTI_CAPTURE_ROUND`, a port
    // addition's test aid) plays rounds of four players first.
    let multi_wait_s = if std::env::var_os("ROSES_MULTI_CAPTURE_ROUND").is_some() && roses_game::extras() { 900 } else { 0 };
    let gate_wait_s =
        multi_wait_s + video_frames::extra_seconds() + capture_map_updates.map_or(0, |n| u64::from(n) / roses_game::UPDATE_HZ as u64 + 1);
    let _watchdog = capture.as_ref().map(|_| CaptureWatchdog::start(gate_wait_s));
    let mut app = App::new();
    let mut plugins = DefaultPlugins.set(AssetPlugin {
        file_path: roses_game::asset_folder(),
        ..default()
    });
    if capture.is_some() || recording {
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
                title: opening
                    .as_ref()
                    .map(|o| o.preview.caption())
                    .unwrap_or_else(|| {
                        "Roses Bevy research — captured matrices, no renderer parity claim".into()
                    }),
                resolution: (960, 720).into(),
                ..default()
            }),
            ..default()
        });
    }
    if let Some(story_scene) = story_scene {
        app.insert_resource(story_scene);
    }
    app.insert_resource(ClearColor(Color::BLACK))
        .insert_resource(InputScene(loaded))
        .add_plugins(plugins)
        .add_plugins(MaterialPlugin::<CapturedMaterial>::default())
        .add_plugins(MaterialPlugin::<CompositeMaterial>::default())
        .add_plugins(MaterialPlugin::<CloudMaterial>::default())
        .add_systems(Startup, setup);
    if let Some(clouds) = clouds {
        app.insert_resource(clouds)
            .insert_resource(Time::<Fixed>::from_hz(roses_game::UPDATE_HZ))
            .add_systems(
                FixedUpdate,
                (
                    advance_transition,
                    update_world_pose,
                    story_effects,
                    story_beam,
                    cloud_render::animate_clouds,
                )
                    .chain()
                    .run_if(opening_active),
            );
    }
    let has_opening = opening.is_some();
    if let Some(opening) = opening {
        app.insert_resource(opening)
            .insert_resource(Time::<Fixed>::from_hz(roses_game::UPDATE_HZ))
            .add_systems(FixedUpdate, story_motion_tick.run_if(opening_active));
    }
    let mut capture_result = None;
    let mut record_outcome: Option<record::RecordOutcome> = None;
    if let Some(path) = capture {
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
            .add_systems(Update, drive_capture);
        video_frames::add_systems(&mut app);
        // A connected capture may play story stages fired by the map (11020,
        // paged by --story-auto), so the opening keeps reading its input.
        if has_opening && game_import.is_some() {
            app.add_systems(Update, update_opening.run_if(opening_active));
        }
        app.get_sub_app_mut(RenderApp)
            .ok_or("Bevy render sub-app unavailable")?
            .insert_resource(bridge)
            .add_systems(Render, inspect_render_progress.after(RenderSystems::Render));
    } else {
        if let Some(settings) = record_settings {
            // The video recorder (a tool): headless, the game clock stepped
            // by its frames; the window's systems (fit, Escape) are left out.
            let pace = if settings.realtime { 1.0 / f64::from(settings.format.fps) } else { 0.0 };
            app.add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(pace)));
            record_outcome = Some(record::add(&mut app, settings));
            // Key frames of a recorded duel too (ROSES_VIDEO_FRAMES).
            video_frames::add_systems(&mut app);
        } else {
            app.add_systems(Update, (fit_view, exit_on_escape));
        }
        // The gamepads (XInput, DualSense / DualShock 4) press pad 1's keys
        // in every windowed scene, the title and story included.
        pad_input::add_systems(&mut app);
        if has_opening {
            app.insert_non_send(MoviePlayback::default())
                .add_systems(
                    Update,
                    (sync_music, sync_movie.after(update_opening)).run_if(opening_active),
                );
        }
        if has_opening {
            app.add_systems(Update, update_opening.run_if(opening_active));
            if std::env::var("ROSES_TRACE_INPUT").is_ok_and(|v| v == "1") {
                app.add_systems(Update, trace_input);
            }
        }
    }
    if let Some(beam) = game_import.as_ref().and_then(|import| light_beam(import, cloud_root.as_deref())) {
        app.insert_resource(beam);
    }
    if let Some(import) = game_import {
        let seed = game_seed.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos() as u64)
        });
        duel_board_render::add_play_materials(&mut app);
        duel_board_render::add_play_systems(&mut app);
        campaign_map_render::add_map_materials(&mut app);
        campaign_map_render::add_map_systems(&mut app);
        slot_machine_render::add_slot_materials(&mut app);
        slot_machine_render::add_slot_systems(&mut app);
        deck_editor_render::add_editor_materials(&mut app);
        deck_editor_render::add_editor_systems(&mut app);
        pad_input::add_systems(&mut app);
        custom_duel_render::add_systems(&mut app);
        map_editor_render::add_systems(&mut app);
        trade_render::add_systems(&mut app);
        online_render::add_systems(&mut app);
        let tutorial = replayed_to_tutorial.map(|resume| TutorialFlow {
            resume,
            lesson: tutorial_lesson.unwrap_or(Lesson::Basics),
            from: None,
            auto: tutorial_auto,
            ticks: tutorial_ticks,
            to_map: false,
        });
        let next = if tutorial.is_some() {
            Some(GameScene::Tutorial)
        } else {
            replayed_to_map.is_some().then_some(GameScene::Map)
        };
        // Without a replay into the practice "Yes", --tutorial-lesson and
        // the other tutorial options apply once the choice is made.
        let tutorial = tutorial.or_else(|| {
            (tutorial_lesson.is_some() || tutorial_auto || tutorial_ticks > 0).then(|| {
                TutorialFlow {
                    resume: roses_game::opening::TUTORIAL_RESUME_STAGE.into(),
                    lesson: tutorial_lesson.unwrap_or(Lesson::Basics),
                    from: None,
                    auto: tutorial_auto,
                    ticks: tutorial_ticks,
                    to_map: false,
                }
            })
        });
        // One sound output for the whole game (SYSTEM0 stays loaded from the
        // title on) [F UI-SOUNDS.md 1, 7.1].
        if let Some(manifest) = &import.duel_audio {
            match duel_board_render::DuelSound::open(manifest) {
                Ok(mut sound) => {
                    sound.app_wide = true;
                    app.insert_resource(sound);
                }
                Err(e) => eprintln!("Game: sound not opened ({e}); silent"),
            }
        }
        app.insert_resource(GameFlow {
            import,
            duel_leaders: None,
            scene: GameScene::Opening,
            next,
            capture_scene: capture_game_scene,
            campaign: replayed_to_map.unwrap_or_default(),
            seed,
            rng: seed,
            player_block: None,
            blocks: None,
            map_save: map_save.or(replayed_map_save),
            capture_map_gate,
            capture_map_updates: 0,
            capture_map_reached: false,
            editor_replay,
            exit_after_editor,
            exit_at_duel,
            end_replay,
            exit_at_map,
            map_replay,
            map_replay_held: roses_client::campaign_map::Pad::NONE,
            computer_player,
            reels: None,
            stopped: None,
            request: None,
            won: None,
            tutorial,
            parked_preview: None,
            exit_at_story,
            story_stage: None,
            story_exit: None,
            story_auto,
            story_auto_ticks: 0,
            needs_block: false,
            scroll_word: Default::default(),
            scroll_p1: Default::default(),
            d63: 0,
            map_editor_settled: false,
            online_script_done: false,
        })
        .insert_resource(Time::<Fixed>::from_hz(roses_game::UPDATE_HZ))
        .add_systems(Update, (game_block_watch, game_switch, game_duel_watch, mode_fade::tick))
        .add_systems(
            FixedUpdate,
            (game_map_replay, game_story_auto).run_if(resource_exists::<GameFlow>),
        )
        .add_systems(FixedUpdate, title_load_tick.run_if(resource_exists::<TitleLoad>))
        .add_systems(FixedUpdate, ending_render::ending_tick.run_if(resource_exists::<ending_render::EndingRun>));
    }
    let outcome = app.run();
    if let Some(record) = record_outcome {
        match record.0.lock().map_err(|e| e.to_string())?.take() {
            Some(Ok(_)) => {}
            Some(Err(e)) => return Err(format!("Recording failed: {e}")),
            None => return Err("The game closed before anything was recorded".into()),
        }
    }
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
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod keyboard_tests {
    use super::*;
    #[test]
    fn connected_capture_rejects_wrong_pending_and_stopped_scenes() {
        assert_eq!(
            capture_scene_ready(Some(GameScene::Duel), GameScene::Map, None, None),
            Ok(false)
        );
        assert_eq!(
            capture_scene_ready(Some(GameScene::Map), GameScene::Map, None, None),
            Ok(true)
        );
        assert_eq!(
            capture_scene_ready(None, GameScene::Opening, Some(GameScene::Map), None),
            Ok(false)
        );
        assert!(
            capture_scene_ready(
                Some(GameScene::Map),
                GameScene::Map,
                None,
                Some("unsupported effect")
            )
            .is_err()
        );
        assert!(
            capture_scene_ready(None, GameScene::Opening, None, Some("missing asset")).is_err()
        );
    }

    #[test]
    fn scene_switch_clears_stale_capture_without_extending_its_deadline() {
        let mut world = bevy::ecs::world::World::new();
        let started = Instant::now();
        world.insert_resource(CaptureState {
            path: PathBuf::from("unused.png"),
            target: Some(Handle::default()),
            started,
            requested: true,
            completed: false,
            settled_frames: 17,
        });
        let bridge = CaptureBridge::default();
        {
            let mut progress = bridge.0.lock().unwrap();
            progress.ready_frames = 40;
            progress.scene_ready = true;
            progress.expect_clouds = true;
            progress.error = Some("old scene".into());
        }
        world.insert_resource(bridge.clone());
        reset_capture_scene(&mut world);
        let state = world.resource::<CaptureState>();
        assert!(state.target.is_none() && !state.requested && !state.completed);
        assert_eq!(state.settled_frames, 0);
        assert_eq!(state.started, started);
        let progress = bridge.0.lock().unwrap();
        assert_eq!(progress.ready_frames, 0);
        assert!(!progress.scene_ready && !progress.expect_clouds && progress.error.is_none());
    }

    #[test]
    fn map_replay_pads_become_press_and_release_events() {
        use roses_client::campaign_map::Pad;
        let up = Pad {
            up: true,
            ..Pad::NONE
        };
        let up_cross = Pad {
            up: true,
            cross: true,
            ..Pad::NONE
        };
        assert!(map_pad_key_events(Pad::NONE, Pad::NONE).is_empty());
        let press = map_pad_key_events(Pad::NONE, up);
        assert_eq!(press.len(), 1);
        assert_eq!(
            (press[0].0, press[0].2),
            (KeyCode::ArrowUp, ButtonState::Pressed)
        );
        // A held direction is not re-sent; Cross is pressed on its own.
        let more = map_pad_key_events(up, up_cross);
        assert_eq!(more.len(), 1);
        assert_eq!(
            (more[0].0, more[0].2),
            (KeyCode::KeyX, ButtonState::Pressed)
        );
        let release = map_pad_key_events(up_cross, Pad::NONE);
        assert_eq!(release.len(), 2);
        assert!(release.iter().all(|e| e.2 == ButtonState::Released));
    }
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
    fn observed_windows_logical_arrow_falls_back_without_double_action() {
        let injected = event(KeyCode::Numpad6, Key::ArrowRight);
        assert_eq!(opening_button(&injected, None), Some(Button::Right));
        let physical = event(KeyCode::ArrowRight, Key::ArrowRight);
        let actions: Vec<_> = [&physical]
            .into_iter()
            .filter_map(|e| opening_button(e, None))
            .collect();
        assert_eq!(actions, [Button::Right]);
        assert_eq!(
            opening_button(&event(KeyCode::Numpad6, Key::Character("6".into())), None),
            None
        );
        assert_eq!(
            opening_button(&event(KeyCode::KeyA, Key::ArrowRight), None),
            Some(Button::Square)
        );
    }

    #[test]
    fn repeat_and_release_are_ignored_and_enter_uses_stage_primary() {
        let mut input = event(KeyCode::Numpad6, Key::ArrowRight);
        input.repeat = true;
        assert_eq!(opening_button(&input, None), None);
        input.repeat = false;
        input.state = ButtonState::Released;
        assert_eq!(opening_button(&input, None), None);
        assert_eq!(
            opening_button(&event(KeyCode::Enter, Key::Enter), Some(Button::Cross)),
            Some(Button::Cross)
        );
        assert_eq!(
            opening_button(&event(KeyCode::Enter, Key::Enter), None),
            None
        );
    }
}
