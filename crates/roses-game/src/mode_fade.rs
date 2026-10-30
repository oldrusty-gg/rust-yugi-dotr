//! The mode-change fade of PAL SLES_515.79 between the game's scenes.
//!
//! [F] 0x1A9130 (replace), 0x1A91E0 (push) and 0x1A9290 (pop) start fade
//! channel 0 with 0x1FDE80(0, 1, 1, 0, 60, black) and set the current mode
//! to −2: from the next frame the main loop runs the transition handler
//! instead of the old mode's update, while the channel's full-screen quad (0x1FDFF0 → 0x1FE1A0, GS ALPHA 0x44)
//! is drawn at alpha 128·t/60, t = 0, 1, … 60 (then held). The transition
//! handler 0x1A7C50 waits until 0x1FE3C0(0) reports t = 60, inits the
//! requested mode (0x1A9330), releases channel 0 and starts channel 1 with
//! 0x1FDE80(1, 0, 1, 0, 60, black) paused (0x1FE340(1, 1)): black while the
//! new mode loads; 0x1AAD54 un-pauses it once the loads are done
//! (0x2DAB84 = 0), then it is drawn at 128·(60 − t)/60 and released at 60.
//! The title menu's own requests (Continue, Custom Duel, Trade: 0x1ACEE4,
//! 0x1AD0EC..0x1AD2F4) use the same fade.
//!
//! [emu PCSX2 + PINE, research `story/claude-faces/tools/mode_fade_pine.py`]:
//! duel → slot machine: ch 0 t 0 → 60 one per update, the mode init on the
//! update after t = 60, ch 1 paused at t = 0 (black) for the load (38
//! updates in PCSX2), then t 0 → 60 and idle; the slot machine → map pop
//! fades the same way. Software-GS frames of saved states: the old mode's
//! picture stays on screen under the quad (slot → map at t = 20: 0.69 of
//! its brightness), the new mode fades in under ch 1 (t = 20: darkened);
//! the duel was already black when its request came (its end screens),
//! and no "NOW LOADING" was drawn in that black hold.
//!
//! The port: a switch of the connected game's scene (`GameFlow::next`) is
//! held for 60 updates while the old scene is frozen (virtual time paused:
//! its FixedUpdate systems stop [H: the original keeps drawing the old mode
//! during −2; whether its update runs is not traced]) under the
//! fading quad; the switch then happens on the black frame and the new
//! scene runs under the 60-update fade-in (its loads are synchronous, so
//! the disc-load part of the original's black hold lasts no update here).
//! Scene pairs that are one mode in the original do not fade: the story
//! scene and the campaign map (story events, the voyage), except the map's
//! "Return to Title" (pop) and the title's Continue.
//!
//! NOW LOADING (0x1AA220, `duel_hud::now_loading`) [F 0x1A9330..0x1A94F8,
//! 0x1AABB0..0x1AAD5C; PCSX2 modes/ui3 loading-editor: map → deck editor
//! and back]: the mode init sets 0x2DAB8A bit 4 for every change except to
//! the title (0xA), the boot check (0x24), the slot machine after a duel,
//! negative modes, and pushes with a keep flag (Custom Duel's duel and deck
//! editor, 0x1C7DF8 / 0x1C81AC) and their pops (no re-init). With it the
//! black hold lasts the load (5 updates for the editor, 147 for the map in
//! PCSX2; the port's loads are instant) and then a fixed 32 frames while
//! the overlay fades out from alpha 0x80; only then does the fade-in run.
//! The port reproduces that fixed part: the switch frame (overlay alpha 0),
//! the measured fixed holds (`load_hold`: the editor's loads and the music
//! waits), 32 frames of NOW LOADING with its tile wipe, then the 60-update
//! fade-in; the new scene stays paused meanwhile (0x1AABB0 skips the mode's
//! update while the load state is not 0).
//!
//! [H] The quad is blended in the window's linear space with an alpha that
//! gives the GS's gamma-space darkening (1 − a)·C for a 2.2 transfer.
use super::{GameFlow, GameScene, OpeningState};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use roses_client::duel_hud::{self, HudTables, HudTextures, NowLoading, NowLoadingTables};
use roses_client::gs::Target16;

/// Frames of each fade (0x3C).
pub const FADE_UPDATES: u32 = 60;
const LAYER: usize = 30;
const LOADING_LAYER: usize = 31;
const TICK: f32 = (1.0 / roses_game::UPDATE_HZ) as f32;
const FRAME: usize = 512;

/// The overlay's tables and atlases (the duel import's TEXSYS #6), its
/// frame and the image the presentation shows.
struct LoadingGraphics {
    tables: HudTables,
    strings: NowLoadingTables,
    textures: HudTextures,
    target: Target16,
    image: Handle<Image>,
}

/// The NOW LOADING frame on screen (marks its quad and camera).
#[derive(Component)]
pub struct LoadingOverlay;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    /// Fading out toward the switch to `to`; `k` updates since the request.
    Out { k: u32, to: GameScene },
    /// The black hold after the switch while NOW LOADING fades out:
    /// `load` updates of load state 3 left, then `music` of state 2.
    Loading { overlay: NowLoading, loaded: bool, load: u32, music: u32 },
    /// Fading in; `k` updates since the switch.
    In { k: u32 },
}

#[derive(Resource)]
pub struct ModeFade {
    phase: Phase,
    acc: f32,
    material: Option<Handle<StandardMaterial>>,
    /// The overlay state of the frame to draw (before its step) and whether
    /// the load state is still 3 then.
    shown: Option<NowLoading>,
    shown_loading: bool,
    graphics: Option<LoadingGraphics>,
    /// The graphics could not be built (reported once; the hold still runs).
    graphics_failed: bool,
}

impl Default for ModeFade {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            acc: 0.0,
            material: None,
            shown: None,
            shown_loading: false,
            graphics: None,
            graphics_failed: false,
        }
    }
}

/// Whether the original shows NOW LOADING (0x2DAB8A bit 4 from 0x1A9330)
/// on this scene change.
fn shows_now_loading(from: GameScene, to: GameScene) -> bool {
    use GameScene::*;
    match (from, to) {
        // The title (mode 0xA) [F 0x1A9440].
        (_, Opening) => false,
        // 0xF after the duel (mode 1) [F 0x1A9484].
        (Duel, SlotMachine) => false,
        // Custom Duel pushes its duel and the deck editor with flag 1 (no
        // NOW LOADING), and their pops skip the re-init [F 0x1A93E0, 0x1A940C].
        (CustomDuel, Duel | DeckEditor) | (Duel | DeckEditor, CustomDuel) => false,
        // A port addition with no original counterpart.
        (Online, _) | (_, Online) => false,
        _ => true,
    }
}

/// The original's load hold after the mode init (0x1AABB0), in updates:
/// (load state 3: the disc loads, 0x1FEC40; load state 2: the music,
/// 0x217150) [emu PCSX2 `modes/ui3/tools/loading_cycles.py` and
/// `loading_from_state.py`, captures `loading-cycles/`, `loading-*.json`]:
/// map -> deck editor (5, 33) and back (147, 1) in three cycles each, the
/// same every time; into a duel the music hold was 33 both times measured
/// while its load was 47 (map, session 655) or 34 (session 186); duel -> map
/// (176, 1) once. The fixed ones are reproduced; the duel's and the map's
/// loads after a duel depend on the data read [H: disc position] and stay
/// out (the port's loads are instant). Others were not measured: (0, 0).
fn load_hold(from: GameScene, to: GameScene) -> (u32, u32) {
    use GameScene::*;
    match (from, to) {
        (Map, DeckEditor) => (5, 33),
        (DeckEditor, Map) => (147, 1),
        (Map | Tutorial, Duel) => (0, 33),
        (Duel, Map) => (0, 1),
        _ => (0, 0),
    }
}

/// The fade's camera and quad (kept across the scene switches).
#[derive(Component)]
pub struct Overlay;

impl ModeFade {
    /// The quad's GS alpha as a fraction (0 = clear, 1 = black).
    pub fn level(&self) -> f32 {
        match self.phase {
            Phase::Idle => 0.0,
            Phase::Out { k, .. } => k.min(FADE_UPDATES) as f32 / FADE_UPDATES as f32,
            Phase::Loading { .. } => 1.0,
            Phase::In { k } => FADE_UPDATES.saturating_sub(k) as f32 / FADE_UPDATES as f32,
        }
    }
}

/// Whether the original changes its mode (and fades) between these scenes.
fn fades(world: &World, from: GameScene, to: GameScene) -> bool {
    let flow = world.resource::<GameFlow>();
    match (from, to) {
        // The title's Continue loads the map mode; the story's end and the
        // story events come back to the map within one mode.
        (GameScene::Opening, GameScene::Map) => world
            .get_resource::<OpeningState>()
            .is_some_and(|o| matches!(o.preview.model.stage().id(), "title" | "main-menu")),
        // "Return to Title Screen?" YES pops to the title; story events do
        // not change the mode.
        (GameScene::Map, GameScene::Opening) => flow.story_stage.as_deref() == Some("title"),
        _ => from != to,
    }
}

fn pause(world: &mut World, on: bool) {
    let mut time = world.resource_mut::<Time<Virtual>>();
    if on {
        time.pause();
    } else {
        time.unpause();
    }
}

/// Called by `game_switch` before it takes `GameFlow::next`: true while the
/// switch must wait for the fade-out (the request is held here meanwhile).
pub fn hold(world: &mut World) -> bool {
    let Some(next) = world.get_resource::<GameFlow>().and_then(|f| f.next) else {
        return false;
    };
    if !world.contains_resource::<ModeFade>() {
        world.init_resource::<ModeFade>();
    }
    let scene = world.resource::<GameFlow>().scene;
    let phase = world.resource::<ModeFade>().phase;
    let new_phase = match phase {
        Phase::Out { k, to } if k >= FADE_UPDATES => {
            // The black frame: switch now (the mode init). With NOW LOADING
            // the init frame draws it at alpha min(counter, 0x80) = 0 and the
            // hold follows; else the fade-in runs from the next update.
            if shows_now_loading(scene, to) {
                let mut overlay = NowLoading::start();
                overlay.step();
                let (load, music) = load_hold(scene, to);
                eprintln!("Game: NOW LOADING {scene:?} -> {to:?} (load {load}, music {music}, {} updates after the load)", duel_hud::NOW_LOADING_FADE);
                world.resource_mut::<ModeFade>().phase = Phase::Loading { overlay, loaded: false, load, music };
            } else {
                world.resource_mut::<ModeFade>().phase = Phase::In { k: 0 };
                pause(world, false);
            }
            return false;
        }
        // A repeated request while fading out: keep the latest target.
        Phase::Out { k, .. } => Phase::Out { k, to: next },
        // A request during the black hold: the screen is black already.
        Phase::Loading { .. } => Phase::Out { k: FADE_UPDATES, to: next },
        Phase::Idle | Phase::In { .. } if !fades(world, scene, next) => return false,
        // A new request during a fade-in starts from the current level.
        Phase::In { k } => Phase::Out {
            k: FADE_UPDATES.saturating_sub(k),
            to: next,
        },
        Phase::Idle => Phase::Out { k: 0, to: next },
    };
    if matches!(phase, Phase::Idle | Phase::In { .. }) {
        eprintln!("Game: mode fade {scene:?} -> {next:?} (60 updates out, then 60 in)");
        pause(world, true);
    }
    world.resource_mut::<ModeFade>().phase = new_phase;
    world.resource_mut::<GameFlow>().next = None;
    true
}

/// `UPDATE_HZ` steps of the fade on real time (virtual time is paused while the
/// old scene fades out), and the quad's alpha.
pub fn tick(world: &mut World) {
    if !world.contains_resource::<ModeFade>() || !world.contains_resource::<GameFlow>() {
        return;
    }
    let dt = world.resource::<Time<Real>>().delta_secs();
    let mut release = None;
    let mut resume = false;
    {
        let mut fade = world.resource_mut::<ModeFade>();
        if fade.phase == Phase::Idle {
            fade.acc = 0.0;
        } else {
            fade.acc += dt;
            while fade.acc >= TICK {
                fade.acc -= TICK;
                fade.phase = match fade.phase {
                    // Load state 3 for `load` updates (the overlay's alpha is
                    // min(counter, 0x80)), then 3 -> 2 (alpha 0x80) and the
                    // music wait (`music` updates) while the alpha falls;
                    // each frame draws the overlay, then steps it; once the
                    // music is in and the alpha is 0 the fade-in runs.
                    Phase::Loading { mut overlay, loaded, load, music } => {
                        if load > 0 {
                            fade.shown = Some(overlay);
                            fade.shown_loading = true;
                            overlay.step();
                            Phase::Loading { overlay, loaded, load: load - 1, music }
                        } else {
                            if !loaded {
                                overlay.loaded();
                            }
                            let music = music.saturating_sub(1);
                            fade.shown_loading = false;
                            if overlay.shown(false).is_some() || music > 0 {
                                fade.shown = overlay.shown(false).map(|_| overlay);
                                overlay.step();
                                Phase::Loading { overlay, loaded: true, load, music }
                            } else {
                                fade.shown = None;
                                resume = true;
                                Phase::In { k: 0 }
                            }
                        }
                    }
                    Phase::Out { k, to } => {
                        let k = (k + 1).min(FADE_UPDATES);
                        if k >= FADE_UPDATES {
                            release = Some(to);
                        }
                        Phase::Out { k, to }
                    }
                    Phase::In { k } if k + 1 >= FADE_UPDATES => Phase::Idle,
                    Phase::In { k } => Phase::In { k: k + 1 },
                    Phase::Idle => Phase::Idle,
                };
            }
        }
    }
    if let Some(to) = release {
        // `hold` lets it through on the next switch pass.
        world.resource_mut::<GameFlow>().next = Some(to);
    }
    if resume {
        pause(world, false);
    }
    draw(world);
    draw_loading(world);
}

/// The duel import's executable, messages and atlases for the overlay.
fn loading_graphics(world: &mut World) -> Option<LoadingGraphics> {
    let dir = world.get_resource::<GameFlow>()?.import.duel.clone();
    let built = (|| -> Result<_, String> {
        let data = roses_game::duel_play::load(&dir).map_err(|e| e.to_string())?;
        let tables = HudTables::from_files(&data.executable, &data.messages).map_err(|e| e.to_string())?;
        let strings = NowLoadingTables::from_executable(&data.executable).map_err(|e| e.to_string())?;
        let textures = HudTextures::new(&data.texsys, &data.picmini, [0, 0], &data.messages).map_err(|e| e.to_string())?;
        Ok((tables, strings, textures))
    })();
    match built {
        Ok((tables, strings, textures)) => {
            let image = Image::new(
                Extent3d { width: FRAME as u32, height: FRAME as u32, depth_or_array_layers: 1 },
                TextureDimension::D2,
                vec![0; FRAME * FRAME * 4],
                TextureFormat::Rgba8Unorm,
                RenderAssetUsages::default(),
            );
            let image = world.resource_mut::<Assets<Image>>().add(image);
            Some(LoadingGraphics {
                tables,
                strings,
                textures,
                target: Target16 { width: FRAME, height: FRAME, pixels: vec![0; FRAME * FRAME] },
                image,
            })
        }
        Err(e) => {
            eprintln!("Game: NOW LOADING not drawn (the black hold still runs): {e}");
            None
        }
    }
}

/// The overlay's frame over the black hold: the GS frame cleared to black,
/// 0x1AA220's primitives, shown as the deck editor shows its frames.
fn draw_loading(world: &mut World) {
    let (shown, loading) = match world.resource::<ModeFade>().phase {
        Phase::Loading { .. } => (world.resource::<ModeFade>().shown, world.resource::<ModeFade>().shown_loading),
        _ => (None, false),
    };
    let mut existing = world.query_filtered::<Entity, With<LoadingOverlay>>();
    let existing: Vec<Entity> = existing.iter(world).collect();
    let Some(state) = shown else {
        for e in existing {
            world.despawn(e);
        }
        return;
    };
    if world.resource::<ModeFade>().graphics.is_none() && !world.resource::<ModeFade>().graphics_failed {
        let g = loading_graphics(world);
        let mut fade = world.resource_mut::<ModeFade>();
        fade.graphics_failed = g.is_none();
        fade.graphics = g;
    }
    let Some(mut g) = world.resource_mut::<ModeFade>().graphics.take() else { return };
    g.target.pixels.fill(0);
    let frame = duel_hud::now_loading(&g.tables, &g.strings, &state, loading);
    duel_hud::render_primitives(&frame.prims, &g.tables, &mut g.textures, &mut g.target);
    let rgba = g.target.to_rgba8(0, 0, FRAME, FRAME);
    if let Some(mut image) = world.resource_mut::<Assets<Image>>().get_mut(&g.image) {
        image.data = Some(rgba);
    }
    let handle = g.image.clone();
    world.resource_mut::<ModeFade>().graphics = Some(g);
    if !existing.is_empty() {
        return;
    }
    let quad = world.resource_mut::<Assets<Mesh>>().add(Rectangle::new(2.0, 2.0));
    let material = world
        .resource_mut::<Assets<super::deck_editor_render::EditorMaterial>>()
        .add(super::deck_editor_render::EditorMaterial { frame: handle });
    world.spawn((
        Overlay,
        LoadingOverlay,
        Mesh3d(quad),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        bevy::camera::visibility::NoFrustumCulling,
        RenderLayers::layer(LOADING_LAYER),
    ));
    world.spawn((
        Overlay,
        LoadingOverlay,
        Camera3d::default(),
        Camera { order: 101, clear_color: ClearColorConfig::None, ..default() },
        Transform::IDENTITY,
        Tonemapping::None,
        DebandDither::Disabled,
        Msaa::Off,
        RenderLayers::layer(LOADING_LAYER),
    ));
}

fn draw(world: &mut World) {
    let level = world.resource::<ModeFade>().level();
    // GS (Cs − Cd)·As + Cd with Cs = 0 darkens the gamma-encoded colour
    // by (1 − a); in the linear window target that is (1 − a)^2.2.
    let alpha = 1.0 - (1.0 - level).powf(2.2);
    let material = match world.resource::<ModeFade>().material.clone() {
        Some(m) => m,
        None => {
            let m = world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
                base_color: Color::srgba(0.0, 0.0, 0.0, 0.0),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            });
            world.resource_mut::<ModeFade>().material = Some(m.clone());
            m
        }
    };
    if let Some(mut m) = world.resource_mut::<Assets<StandardMaterial>>().get_mut(&material) {
        if (m.base_color.alpha() - alpha).abs() > f32::EPSILON {
            m.base_color = Color::srgba(0.0, 0.0, 0.0, alpha);
        }
    }
    let mut overlays = world.query_filtered::<Entity, With<Overlay>>();
    let existing: Vec<Entity> = overlays.iter(world).collect();
    if level <= 0.0 {
        for e in existing {
            world.despawn(e);
        }
        return;
    }
    if !existing.is_empty() {
        return;
    }
    let quad = world.resource_mut::<Assets<Mesh>>().add(Rectangle::new(100.0, 100.0));
    world.spawn((
        Overlay,
        Mesh3d(quad),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, 0.0, -1.0),
        bevy::camera::visibility::NoFrustumCulling,
        RenderLayers::layer(LAYER),
    ));
    world.spawn((
        Overlay,
        Camera3d::default(),
        Camera {
            order: 100,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Transform::IDENTITY,
        Tonemapping::None,
        DebandDither::Disabled,
        Msaa::Off,
        RenderLayers::layer(LAYER),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_the_channel_quads() {
        let mut f = ModeFade::default();
        assert_eq!(f.level(), 0.0);
        f.phase = Phase::Out { k: 0, to: GameScene::Duel };
        assert_eq!(f.level(), 0.0);
        f.phase = Phase::Out { k: 30, to: GameScene::Duel };
        assert_eq!(f.level(), 0.5);
        f.phase = Phase::Out { k: 60, to: GameScene::Duel };
        assert_eq!(f.level(), 1.0);
        f.phase = Phase::In { k: 0 };
        assert_eq!(f.level(), 1.0);
        f.phase = Phase::In { k: 45 };
        assert_eq!(f.level(), 0.25);
        f.phase = Phase::Loading { overlay: NowLoading::start(), loaded: false, load: 0, music: 0 };
        assert_eq!(f.level(), 1.0);
    }

    /// A frame of the hold as the port renders it (`ROSES_DUEL_IMPORT`,
    /// `ROSES_LOADING_PNG` = a new PNG path): 20 frames into the fade-out.
    #[test]
    #[ignore = "Requires a private duel import (ROSES_DUEL_IMPORT) and ROSES_LOADING_PNG"]
    fn private_now_loading_frame_png() {
        let dir = std::path::PathBuf::from(std::env::var_os("ROSES_DUEL_IMPORT").unwrap());
        let out = std::path::PathBuf::from(std::env::var_os("ROSES_LOADING_PNG").unwrap());
        let data = roses_game::duel_play::load(&dir).unwrap();
        let tables = HudTables::from_files(&data.executable, &data.messages).unwrap();
        let strings = NowLoadingTables::from_executable(&data.executable).unwrap();
        let mut textures = HudTextures::new(&data.texsys, &data.picmini, [0, 0], &data.messages).unwrap();
        let mut target = Target16 { width: FRAME, height: FRAME, pixels: vec![0; FRAME * FRAME] };
        let mut s = NowLoading::start();
        s.step();
        s.loaded();
        for _ in 0..20 {
            s.step();
        }
        let frame = duel_hud::now_loading(&tables, &strings, &s, false);
        duel_hud::render_primitives(&frame.prims, &tables, &mut textures, &mut target);
        crate::deck_editor_render::write_png(&out, &target.to_rgba8(0, 0, FRAME, FRAME)).unwrap();
    }

    #[test]
    fn now_loading_follows_the_mode_init_rules() {
        use GameScene::*;
        for (from, to) in [(Map, Duel), (Duel, Map), (SlotMachine, Map), (Map, DeckEditor), (DeckEditor, Map), (Opening, Map), (Opening, Trade), (Opening, CustomDuel), (Map, Tutorial)] {
            assert!(shows_now_loading(from, to), "{from:?} -> {to:?}");
        }
        for (from, to) in [(Map, Opening), (Trade, Opening), (Duel, SlotMachine), (CustomDuel, Duel), (Duel, CustomDuel), (CustomDuel, DeckEditor), (DeckEditor, CustomDuel)] {
            assert!(!shows_now_loading(from, to), "{from:?} -> {to:?}");
        }
    }
}
