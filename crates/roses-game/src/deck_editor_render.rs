//! The deck editor (mode 6 "BUILD DECK") as a scene of the connected game:
//! `roses_core::deck_editor::DeckEditor` stepped once per 25 Hz update with
//! the keyboard pad (the duel key map plus C = L3 reincarnation and V = R3
//! password), editing the player's card block in place, and every frame
//! drawn by the CPU GS model from `deck_editor_screen::build_screen` into a
//! 512×512 image that a full-screen quad presents at 4:3 (the slot
//! machine's present shader).
//!
//! The detail pane (DTL, 0x1BE6E0) is `card_detail`'s (shared with Trade):
//! the lists move with the slide and the pane follows; its picture is the
//! import's PICTURE.MRG record (duel-end import), else NOW LOADING.
//!
//! Not reproduced (reported through the window title and stderr): the
//! reincarnation/password screens and
//! the four message boxes (the model runs them), the animation slots, the 3D card viewer (mode 7: logged, the
//! editor continues at once), sounds (ids logged). The analog sticks scroll the lists (port 0's
//! gamepad: right / left stick Y, as the model reads them).
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::{NoFrustumCulling, RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    input::{
        ButtonState,
        keyboard::{KeyboardFocusLost, KeyboardInput},
    },
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    reflect::TypePath,
    render::render_resource::{
        AsBindGroup, CompareFunction, Extent3d, RenderPipelineDescriptor,
        SpecializedMeshPipelineError, TextureDimension, TextureFormat,
    },
    shader::ShaderRef,
    window::PrimaryWindow,
};
use roses_game::deck_editor_screen::{self, ScreenInputs};
use roses_game::duel_end_screens::FRAME;
use roses_game::duel_play;
use roses_client::duel_hud::{HudTables, HudTextures, render_primitives};
use roses_client::gs::Target16;
use roses_core::card_block::{CardBlock, EMPTY_CARD};
use roses_core::deck_editor::{
    self, DeckEditor, DeckRecord, EditorEnv, EditorEvent, PadInput, StaticCards, Sticks, pad, state,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const PIPELINE: &str = "roses-deck-editor-present";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct EditorMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub(crate) frame: Handle<Image>,
}

impl Material for EditorMaterial {
    fn vertex_shader() -> ShaderRef {
        "slot_machine.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "slot_machine.wgsl".into()
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
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = &mut descriptor.depth_stencil {
            depth.depth_compare = Some(CompareFunction::Always);
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

/// What the game hands the scene.
pub struct EditorInputs {
    /// The side whose block is edited (0 from the map).
    pub side: u8,
    pub block: CardBlock,
    /// SAVE+0xCA, the campaign duel counter (reincarnation gate).
    pub duel_counter: u8,
    /// The shared LCG state.
    pub rng: u64,
    /// The duel import's verbatim files.
    pub executable: Vec<u8>,
    pub messages: Vec<u8>,
    pub texsys: Vec<u8>,
    pub picmini: Vec<u8>,
    /// Card names (for the log).
    pub names: Vec<String>,
    /// Scripted pad words, one per update (`--editor-replay`), applied
    /// before the keyboard; the leftover is returned with the result.
    pub replay: VecDeque<u16>,
    /// 0x2DB828 as the previous mode left it: the editor's init does not set
    /// it [F 0x1B9100; PCSX2 modes/ui3 blink-map-editor.json: the map leaves
    /// the word alone and the editor steps on from it].
    pub blink: roses_client::duel_hud::FlashWave,
    /// 0x2DB826 and d63 as the previous mode left them (the editor steps the
    /// word, 0x1B9558..0x1B9600, and never writes d63).
    pub blink_p1: roses_client::duel_hud::FlashWave,
    pub d63: u8,
    /// Online Play's deck builder (a port addition) on a full-pool block:
    /// ○ leaves with the deck as it is (an unfinished deck too, where the
    /// original refuses with message 0xD), and Start in the deck list does
    /// not switch to the block's other deck records (one deck is built at
    /// a time).
    pub builder: bool,
}

/// What the scene returns when the editor pops.
pub struct EditorResult {
    pub block: CardBlock,
    pub rng: u64,
    pub duel_counter: u8,
    pub replay: VecDeque<u16>,
    pub frames: u64,
    /// The current deck as the editor left it.
    pub deck: DeckRecord,
    /// 0x2DB828 as the editor left it (the next mode that steps it goes on
    /// from here).
    pub blink: roses_client::duel_hud::FlashWave,
    /// 0x2DB826 as the editor left it.
    pub blink_p1: roses_client::duel_hud::FlashWave,
}

#[derive(Resource)]
pub struct EditorRuntime {
    editor: DeckEditor,
    block: CardBlock,
    duel_counter: u8,
    rng: u64,
    cards: StaticCards,
    tables: HudTables,
    textures: HudTextures,
    /// The detail pane's strings and tables, the YES/NO tables menu calls
    /// need, and PICTURE.MRG when the duel-end import has it.
    pane: roses_game::card_detail::DetailData,
    yes_no: roses_client::duel_hud::YesNoTables,
    pictures: Option<roses_game::card_detail::Pictures>,
    picmini: Vec<u8>,
    names: Vec<String>,
    leader_shown: u16,
    held: BTreeMap<KeyCode, u16>,
    tapped: u16,
    replay: VecDeque<u16>,
    target: Target16,
    image: Option<Handle<Image>>,
    title: String,
    reported: BTreeSet<String>,
    finished: bool,
    /// 0x2DB828: the scroll word the editor's update steps (0x1B9604, ±0x800
    /// per update, as the duel's); on entry the previous mode's value
    /// ([`EditorInputs::blink`]).
    blink: roses_client::duel_hud::FlashWave,
    /// 0x2DB826 (±0x200, held rising while d63 != 0) and d63.
    blink_p1: roses_client::duel_hud::FlashWave,
    d63: u8,
    builder: bool,
}

/// Keyboard: the duel key map plus C = L3 and V = R3 (this scene's choice).
fn key_bits(key_code: KeyCode) -> u16 {
    match key_code {
        KeyCode::KeyC => pad::L3,
        KeyCode::KeyV => pad::R3,
        other => duel_play::key_pad(&format!("{other:?}")),
    }
}

/// The HUD tables with every message the screen composes: the HUD's own
/// set (all 854 card names included).
fn tables(executable: &[u8], messages: &[u8]) -> Result<HudTables, String> {
    let mut codes = BTreeMap::new();
    let mut ids = HudTables::required_messages();
    ids.extend(roses_game::card_detail::message_ids());
    // The message boxes (deck_editor_screen::message_box).
    ids.extend([125, 151, 152]);
    ids.sort_unstable();
    ids.dedup();
    for chunk in ids.chunks(256) {
        let parsed =
            roses_assets::parse_pal_messages(messages, chunk).map_err(|e| e.to_string())?;
        for m in parsed.messages {
            codes.insert(m.message_id, m.expanded_codes);
        }
    }
    HudTables::from_executable(executable, codes)
}

/// Build the editor on the block, decode the atlases and spawn the quad.
pub fn enter_editor_scene(world: &mut World, inputs: EditorInputs) -> Result<(), String> {
    let EditorInputs {
        side,
        mut block,
        mut duel_counter,
        mut rng,
        executable,
        messages,
        texsys,
        picmini,
        names,
        replay,
        blink,
        blink_p1,
        d63,
        builder,
    } = inputs;
    let cards = StaticCards::from_executable(&executable, 0)
        .ok_or("the executable is too short for the card records")?;
    let tables = tables(&executable, &messages)?;
    let editor = {
        let mut env = EditorEnv {
            block: &mut block,
            duel_counter: &mut duel_counter,
            cards: &cards,
            rng: &mut rng,
            graphics_busy: false,
        };
        DeckEditor::new(side, &mut env)
    };
    let deck = deck_editor::deck_record(&block, deck_editor::current_deck(&block));
    let leader = if deck.leader == EMPTY_CARD {
        0
    } else {
        deck.leader
    };
    let textures = HudTextures::new(&texsys, &picmini, [leader, leader], &messages)?;
    let pane = roses_game::card_detail::DetailData::from_executable(&executable)?;
    let yes_no = roses_client::duel_hud::YesNoTables::from_executable(&executable).map_err(|e| e.to_string())?;
    // PICTURE.MRG from the duel-end import (the pane's pictures).
    let end_dir = world.get_resource::<super::GameFlow>().and_then(|f| f.import.duel_end.clone());
    let pictures = match end_dir {
        Some(dir) => {
            let files = roses_game::duel_end_screens::load_duel_end_import(&dir)?;
            Some(roses_game::card_detail::Pictures::new(files.pictures, &executable)?)
        }
        None => {
            eprintln!("Deck editor: no PICTURE.MRG (duel-end import): the detail pane shows NOW LOADING");
            None
        }
    };
    eprintln!(
        "Deck editor: side {side}, deck {} ({} cards, leader {} {:?}, DC {}), {} owned kinds in the trunk list, duel counter {duel_counter}; keys: arrows, X=cross add, A=square remove, S=start leader/deck switch, D=triangle detail, Z=circle back/exit, Q/E=L1/R1 page, 1/W=L2/R2 sort, Tab=select, C=L3 reincarnation, V=R3 password",
        deck_editor::current_deck(&block),
        deck.count(),
        deck.leader,
        names
            .get(usize::from(deck.leader))
            .map(String::as_str)
            .unwrap_or("?"),
        deck.cost,
        editor.trunk_list().len()
    );
    eprintln!(
        "Deck editor: not reproduced: the prompt/message screens (model only), animations, the 3D viewer"
    );
    let title = if builder { builder_caption(&block) } else { caption(&editor, &block) };
    world.insert_resource(EditorRuntime {
        editor,
        block,
        duel_counter,
        rng,
        cards,
        tables,
        textures,
        pane,
        yes_no,
        pictures,
        picmini,
        names,
        leader_shown: leader,
        held: BTreeMap::new(),
        tapped: 0,
        replay,
        target: Target16 {
            width: FRAME,
            height: FRAME,
            pixels: vec![0; FRAME * FRAME],
        },
        image: None,
        title,
        reported: BTreeSet::new(),
        finished: false,
        blink,
        blink_p1,
        d63,
        builder,
    });
    world
        .run_system_cached(setup_editor)
        .map_err(|e| format!("deck editor setup: {e}"))
}

/// Remove the scene's resources; the block, counter and RNG go back.
pub fn leave_editor_scene(world: &mut World) -> Option<EditorResult> {
    world.remove_resource::<EditorRuntime>().map(|r| {
        let deck = deck_editor::deck_record(&r.block, deck_editor::current_deck(&r.block));
        EditorResult {
            block: r.block,
            rng: r.rng,
            duel_counter: r.duel_counter,
            replay: r.replay,
            frames: r.editor.frame(),
            deck,
            blink: r.blink,
            blink_p1: r.blink_p1,
        }
    })
}

/// ○ with a valid deck popped the mode (0x1A9290): the map follows.
pub fn editor_scene_finished(world: &World) -> bool {
    world
        .get_resource::<EditorRuntime>()
        .is_some_and(|r| r.finished)
}

pub fn add_editor_materials(app: &mut App) {
    app.add_plugins(MaterialPlugin::<EditorMaterial>::default());
}

pub fn add_editor_systems(app: &mut App) {
    let active = resource_exists::<EditorRuntime>;
    app.add_systems(
        RunFixedMainLoop,
        editor_keys
            .run_if(active)
            .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
    )
    .add_systems(FixedUpdate, editor_tick.run_if(active));
}

fn setup_editor(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<EditorMaterial>>,
    mut runtime: ResMut<EditorRuntime>,
) {
    let runtime = &mut *runtime;
    render_frame(runtime);
    let mut image = Image::new(
        Extent3d {
            width: FRAME as u32,
            height: FRAME as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        runtime.target.to_rgba8(0, 0, FRAME, FRAME),
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Nearest,
        lod_min_clamp: 0.,
        lod_max_clamp: 0.,
        ..default()
    });
    let image = images.add(image);
    runtime.image = Some(image.clone());
    let quad = meshes.add(Rectangle::new(2., 2.));
    commands.spawn((
        Mesh3d(quad),
        MeshMaterial3d(materials.add(EditorMaterial { frame: image })),
        Transform::IDENTITY,
        NoFrustumCulling,
        RenderLayers::layer(1),
    ));
    commands.spawn((
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
        super::PresentationCamera,
    ));
}

fn editor_keys(
    mut events: MessageReader<KeyboardInput>,
    mut focus_lost: MessageReader<KeyboardFocusLost>,
    mut runtime: ResMut<EditorRuntime>,
) {
    for event in events.read() {
        match event.state {
            ButtonState::Pressed if !event.repeat => {
                let bits = key_bits(event.key_code);
                if bits != 0 {
                    runtime.held.insert(event.key_code, bits);
                    runtime.tapped |= bits;
                }
            }
            ButtonState::Pressed => {}
            ButtonState::Released => {
                runtime.held.remove(&event.key_code);
            }
        }
    }
    if focus_lost.read().count() > 0 {
        runtime.held.clear();
    }
}

pub(crate) fn write_png(path: &std::path::Path, rgba: &[u8]) -> Result<(), String> {
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    let image = Image::new(
        Extent3d {
            width: FRAME as u32,
            height: FRAME as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba.to_vec(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    );
    let dynamic = image.try_into_dynamic().map_err(|e| e.to_string())?;
    let mut encoded = std::io::Cursor::new(Vec::new());
    dynamic
        .write_to(
            &mut encoded,
            bevy::image::ImageFormat::Png
                .as_image_crate_format()
                .ok_or("PNG encoder unavailable")?,
        )
        .map_err(|e| format!("PNG encode: {e}"))?;
    std::fs::write(path, encoded.get_ref()).map_err(|e| format!("{}: {e}", path.display()))
}

fn state_name(s: u8) -> &'static str {
    match s {
        state::CATALOGUE => "catalogue",
        state::TRUNK_DETAIL => "trunk-card detail",
        state::TRUNK_LIST_DETAIL => "trunk list + detail",
        state::TRUNK_LIST => "trunk list",
        state::DECK_LIST => "deck list",
        state::DECK_LIST_DETAIL => "deck list + detail",
        state::DECK_DETAIL => "deck-card detail",
        state::REINCARNATION_PROMPT => "reincarnation prompt",
        state::REINCARNATION_RESULTS => "reincarnation results",
        state::PASSWORD => "password",
        state::MSG_PASSWORD_OK => "message: password accepted",
        state::MSG_PASSWORD_WRONG => "message: wrong password",
        state::MSG_PASSWORD_USED => "message: password already used",
        state::MSG_DECK_INVALID => "message: deck not valid",
        _ => "?",
    }
}

fn caption(editor: &DeckEditor, block: &CardBlock) -> String {
    let d = deck_editor::current_deck(block);
    let deck = deck_editor::deck_record(block, d);
    let s = editor.snapshot();
    format!(
        "Roses - Build Deck · {} · deck {} {}/40 leader {} DC {} · trunk row {}/{} · slide {} · X add, A remove, S leader, Z exit · lists only (other screens not drawn)",
        state_name(s.state),
        d,
        deck.count(),
        deck.leader,
        deck.cost,
        u32::from(s.trunk_window) + u32::from(s.trunk_row) + 1,
        s.trunk_count,
        s.slide
    )
}

/// The deck builder's window caption.
fn builder_caption(block: &CardBlock) -> String {
    let deck = deck_editor::deck_record(block, deck_editor::current_deck(block));
    format!(
        "Roses - Online Play - Deck Builder · {}/40 cards · leader {} · X add, A remove, S leader, D detail, Q/E page, 1/W sort, Z done",
        deck.count(),
        if deck.leader == EMPTY_CARD { "none".to_string() } else { deck.leader.to_string() }
    )
}

fn render_frame(runtime: &mut EditorRuntime) {
    // 0x1BBF50: the brown gradient over the whole frame, first [F].
    runtime.target.pixels.fill(0);
    let background = deck_editor_screen::background_calls(runtime.pane.editor_background);
    let frame = roses_client::duel_hud::menu_frame(&runtime.tables, &runtime.yes_no, &background);
    let mut skipped = render_primitives(&frame.prims, &runtime.tables, &mut runtime.textures, &mut runtime.target);
    let snapshot = runtime.editor.snapshot();
    let (prims, notes) = deck_editor_screen::build_screen(&ScreenInputs {
        snapshot: &snapshot,
        block: &runtime.block,
        cards: &runtime.cards,
        tables: &runtime.tables,
    });
    for n in notes {
        if runtime.reported.insert(n.0.clone()) {
            eprintln!("Deck editor: not drawn: {}", n.0);
        }
    }
    // The list screen moves with the slide (its draws take it as x).
    let mut prims = prims;
    if snapshot.slide != 0 {
        for p in &mut prims {
            for v in &mut p.verts {
                v.x += f32::from(snapshot.slide);
            }
        }
    }
    // 0x1BBB90's order: the catalogue pane (slide >= 0x101), the detail
    // panes, then the lists [F 0x1BBC34..0x1BBDFC].
    if snapshot.slide >= 0x101 {
        let calls = deck_editor_screen::catalogue_calls(
            &deck_editor_screen::Catalogue::of(&snapshot),
            &runtime.block,
            &runtime.cards,
            &runtime.pane,
        );
        let frame = roses_client::duel_hud::menu_frame(&runtime.tables, &runtime.yes_no, &calls);
        skipped.extend(render_primitives(&frame.prims, &runtime.tables, &mut runtime.textures, &mut runtime.target));
    }
    skipped.extend(render_primitives(
        &prims,
        &runtime.tables,
        &mut runtime.textures,
        &mut runtime.target,
    ));
    // 0x1BC620's blinking REINCARNATION (the L3 gate) [F 0x1BCC48].
    let reincarnation = deck_editor_screen::ReincarnationInputs {
        block: &runtime.block,
        duel_counter: runtime.duel_counter,
        graphics_busy: false,
        blink: runtime.blink.value,
    };
    if let Some(call) = deck_editor_screen::reincarnation_call(&snapshot, &reincarnation, &runtime.pane) {
        let frame = roses_client::duel_hud::menu_frame(&runtime.tables, &runtime.yes_no, &[call]);
        skipped.extend(render_primitives(&frame.prims, &runtime.tables, &mut runtime.textures, &mut runtime.target));
    }
    if let Some(choice) = deck_editor_screen::detail_pane(&snapshot, &runtime.block) {
        use roses_game::card_detail;
        let texture = runtime.pictures.as_mut().and_then(|p| p.texture(choice.card, &runtime.block));
        let pane = card_detail::Pane {
            card: choice.card,
            x: i32::from(snapshot.slide),
            y: 0x20,
            side: choice.side,
            page: if choice.side == 0 { snapshot.trunk_page } else { snapshot.deck_page },
            raised_tab: choice.raised_tab,
            tinted: false,
            editor_state: choice.editor.then_some(snapshot.state),
            block: &runtime.block,
            cards: &runtime.cards,
            picture: texture.is_some(),
            // 0x2DB828 >> 8, stepped by the editor's update [F 0x1B9604].
            blink: (runtime.blink.value >> 8) as u8,
            rank: None,
        };
        let (calls, notes) = card_detail::build(&runtime.pane, &runtime.tables, &pane);
        for n in notes {
            if runtime.reported.insert(n.clone()) {
                eprintln!("Deck editor: not drawn: {n}");
            }
        }
        runtime.textures.picture = texture;
        let frame = roses_client::duel_hud::menu_frame(&runtime.tables, &runtime.yes_no, &calls);
        skipped.extend(render_primitives(&frame.prims, &runtime.tables, &mut runtime.textures, &mut runtime.target));
    }
    // 0x1BBB90's state switch draws last [F 0x1BBE08].
    let calls = deck_editor_screen::message_box_calls(snapshot.state, &runtime.tables, &runtime.pane, runtime.editor.frame() as u32);
    if !calls.is_empty() {
        let frame = roses_client::duel_hud::menu_frame(&runtime.tables, &runtime.yes_no, &calls);
        skipped.extend(render_primitives(&frame.prims, &runtime.tables, &mut runtime.textures, &mut runtime.target));
    }
    for s in skipped {
        let text = format!("primitive {} skipped: {}", s.index, s.reason);
        if runtime.reported.insert(text.clone()) {
            eprintln!("Deck editor: {text}");
        }
    }
}

fn report(runtime: &EditorRuntime, event: &EditorEvent) {
    let name = |card: u16| {
        runtime
            .names
            .get(usize::from(card))
            .map(String::as_str)
            .unwrap_or("?")
    };
    match event {
        EditorEvent::Sound(_) => {}
        EditorEvent::Viewer(card) => eprintln!(
            "Deck editor: BOUNDARY — the 3D card viewer (mode 7) of card {card} {:?} is not implemented; the editor continues",
            name(*card)
        ),
        EditorEvent::Exit => eprintln!("Deck editor: ○ with a valid deck pops the mode"),
        EditorEvent::ReleaseGraphics | EditorEvent::ReloadLeader => {}
        EditorEvent::Animation {
            card,
            row,
            kind,
            leader_removed,
            removed,
        } => eprintln!(
            "Deck editor: animation slot (card {card} {:?}, row {row}, add {kind}, leader removed {leader_removed}, removed {removed}) not drawn",
            name(*card)
        ),
        EditorEvent::Granted(card) => {
            eprintln!(
                "Deck editor: card {card} {:?} granted to the trunk",
                name(*card)
            )
        }
        EditorEvent::DuelCounterReset => eprintln!("Deck editor: SAVE+0xCA reset to 0"),
    }
}

/// One 25 Hz update: the pad word (scripted first), the model step on the
/// block, the frame.
fn editor_tick(
    mut runtime: ResMut<EditorRuntime>,
    mut images: ResMut<Assets<Image>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut sound: Option<ResMut<super::duel_board_render::DuelSound>>,
    pads: Option<Res<super::pad_input::Pads>>,
) {
    let runtime = &mut *runtime;
    if runtime.finished {
        return;
    }
    // Port 0's sticks when its gamepad is analog (0x203500(0) != 0; right
    // and left Y, 0x203580(0, 1) / (0, 3)); none for a scripted word.
    let mut sticks = pads.and_then(|p| p.sticks(0)).map(|s| Sticks { right_y: s[1], left_y: s[3] });
    let word = match runtime.replay.pop_front() {
        Some(word) => {
            sticks = None;
            word
        }
        None => runtime
            .held
            .values()
            .fold(runtime.tapped, |acc, &b| acc | b),
    };
    runtime.tapped = 0;
    // The builder builds one deck: Start in the deck list (the original's
    // deck switch) is not passed on.
    let word = if runtime.builder && matches!(runtime.editor.state(), state::DECK_LIST | state::DECK_LIST_DETAIL) {
        word & !pad::START
    } else {
        word
    };
    // 0x1B9554..0x1B9600: d63, 0x2DB826, then 0x2DB828.
    runtime.blink_p1.step_p1(runtime.d63);
    runtime.blink.step();
    let events = {
        let EditorRuntime {
            editor,
            block,
            duel_counter,
            rng,
            cards,
            ..
        } = runtime;
        let mut env = EditorEnv {
            block,
            duel_counter,
            cards,
            rng,
            graphics_busy: false,
        };
        editor.step(PadInput { word, sticks }, &mut env)
    };
    for e in &events {
        report(runtime, e);
        if let (EditorEvent::Sound(id), Some(sound)) = (e, sound.as_deref_mut()) {
            sound.cue(roses_game::duel_audio::AudioCue::Se { slot: 0, id: *id as u8, skip: false });
        }
        if matches!(e, EditorEvent::Exit) {
            runtime.finished = true;
        }
    }
    // The builder keeps an unfinished deck: ○ leaves where the original
    // shows "deck not valid" (the deck select says what it lacks).
    if runtime.builder && runtime.editor.state() == state::MSG_DECK_INVALID {
        eprintln!("Deck builder: left with an unfinished deck");
        runtime.finished = true;
    }
    // The portrait follows the current deck's leader (0x200320(side+5)).
    let deck = deck_editor::deck_record(&runtime.block, deck_editor::current_deck(&runtime.block));
    let leader = if deck.leader == EMPTY_CARD {
        0
    } else {
        deck.leader
    };
    if leader != runtime.leader_shown {
        if let Err(e) = runtime
            .textures
            .set_leaders(&runtime.picmini, [leader, leader])
        {
            eprintln!("Deck editor: portrait of card {leader} not decoded: {e}");
        }
        runtime.leader_shown = leader;
    }
    render_frame(runtime);
    // `ROSES_EDITOR_CAPTURE=<new.png>`: the CPU frame after the second
    // update, or after update `ROSES_EDITOR_CAPTURE_AT` (test aids for
    // comparisons with original screenshots).
    let at = std::env::var("ROSES_EDITOR_CAPTURE_AT").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(2);
    if runtime.editor.frame() == at {
        if let Some(path) = std::env::var_os("ROSES_EDITOR_CAPTURE") {
            let path = std::path::PathBuf::from(path);
            match write_png(&path, &runtime.target.to_rgba8(0, 0, FRAME, FRAME)) {
                Ok(()) => eprintln!("Deck editor: frame written to {}", path.display()),
                Err(e) => eprintln!("Deck editor: capture failed: {e}"),
            }
        }
    }
    if let Some(image) = runtime.image.clone() {
        if let Some(mut image) = images.get_mut(&image) {
            image.data = Some(runtime.target.to_rgba8(0, 0, FRAME, FRAME));
        }
    }
    let title = if runtime.builder { builder_caption(&runtime.block) } else { caption(&runtime.editor, &runtime.block) };
    if title != runtime.title {
        for mut window in &mut windows {
            window.title = title.clone();
        }
        runtime.title = title;
    }
}
