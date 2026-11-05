//! The reward slot machine (mode 0xF "Slot Machine") as a scene of the
//! connected game: `roses_core::slot_machine::SlotMachine` stepped once per
//! 25 Hz update with the keyboard pad (A = □ stops reel 0, D = △ reel 1,
//! Z = ○ reel 2; any face button closes a prize popup), its prizes granted
//! into the winner's card block, and every frame drawn by the CPU GS model
//! (`duel_end_screens::render_slot_screen`) into a 512×512 image that a
//! full-screen quad presents at 4:3.
//!
//! Not reproduced: the mode-switch fade before the first reel frame and the
//! original's global frame counter (the press icon's cel and the RNG cadence
//! start at this scene's own frame 0) and the mode fade back to the map.
//! Sound: track 21 on entry, the lever 0x20 and the spin loop 0x1E, reel
//! stops 0x21, the loop's stop and the popups 7 / 0x19 (`slot_sounds`).
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
use roses_game::duel_end_screens::{
    DuelEndFiles, EndAssets, FRAME, SlotTextures, current_popup, render_slot_screen,
};
use roses_game::duel_audio::AudioCue;
use roses_game::duel_play::{self, PadRepeat};
use roses_client::gs::Target16;
use roses_core::card_block::{AlternateArt, CardBlock};
use roses_core::duel::next_rand31;
use roses_core::slot_machine::{
    Prize, PrizeGrant, PrizeKind, ReelData, RewardCardTable, SlotEvent, SlotMachine, special_pool,
};
use std::collections::{BTreeMap, VecDeque};

const PIPELINE: &str = "roses-slot-machine-present";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SlotMaterial {
    #[texture(0)]
    #[sampler(1)]
    frame: Handle<Image>,
}

impl Material for SlotMaterial {
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

/// What the game hands the scene when the result states end with a reel
/// list (`EndOutcome::SlotMachine`).
pub struct SlotInputs {
    pub reels: ReelData,
    /// D50: the winner, whose block receives the prizes.
    pub winner: u8,
    pub blocks: [CardBlock; 2],
    /// The shared LCG state.
    pub rng: u64,
    /// The duel import's verbatim files for the popup drawer.
    pub executable: Vec<u8>,
    pub messages: Vec<u8>,
    pub texsys: Vec<u8>,
    pub picmini: Vec<u8>,
    pub leaders: [u16; 2],
    pub duel_end: DuelEndFiles,
    /// Scripted pad words, one per update (`--end-replay`), applied before
    /// the keyboard; the leftover is returned with the result.
    pub replay: VecDeque<u16>,
}

/// What the scene returns when it has finished.
pub struct SlotResult {
    pub blocks: [CardBlock; 2],
    pub rng: u64,
    pub prizes: Vec<Prize>,
    pub replay: VecDeque<u16>,
}

#[derive(Resource)]
pub struct SlotRuntime {
    machine: SlotMachine,
    assets: EndAssets,
    textures: SlotTextures,
    blocks: [CardBlock; 2],
    winner: u8,
    rng: u64,
    prizes: Vec<Prize>,
    last_prize: Option<(PrizeKind, u16)>,
    frame: u64,
    repeat: PadRepeat,
    held: BTreeMap<KeyCode, u16>,
    tapped: u16,
    replay: VecDeque<u16>,
    target: Target16,
    image: Option<Handle<Image>>,
    title: String,
    reported: std::collections::BTreeSet<String>,
    finished: bool,
}

struct BlockGrant<'a> {
    block: &'a mut CardBlock,
    art: &'a [AlternateArt],
    prizes: &'a mut Vec<Prize>,
}

impl PrizeGrant for BlockGrant<'_> {
    fn grant(&mut self, prize: Prize, rng: &mut u64) {
        let granted = self.block.grant(prize.card, None, rng);
        if prize.kind == PrizeKind::GetSpecialCard {
            self.block.cycle_art(prize.card, self.art);
        }
        eprintln!(
            "Slot machine: {:?} card {} (order {}){}",
            prize.kind,
            prize.card,
            prize.order,
            if granted {
                ""
            } else {
                " — not granted (nine copies already)"
            }
        );
        self.prizes.push(prize);
    }
}

/// Build the machine and its assets, insert the scene's resources and spawn
/// the present quad.
pub fn enter_slot_scene(world: &mut World, inputs: SlotInputs) -> Result<(), String> {
    let SlotInputs {
        reels,
        winner,
        blocks,
        mut rng,
        executable,
        messages,
        texsys,
        picmini,
        leaders,
        duel_end,
        replay,
    } = inputs;
    let assets = EndAssets::new(
        &executable,
        &messages,
        &texsys,
        &picmini,
        leaders,
        duel_end.pictures,
    )?;
    let textures = SlotTextures::new(&duel_end.texetc)?;
    let flags = roses_assets::parse_pal_reward_flags(&executable).map_err(|e| e.to_string())?;
    let reward = RewardCardTable::from_record_flag_bytes(flags.flag_bytes.iter().copied());
    let pool = special_pool(&reward);
    let (machine, events) = SlotMachine::new(&reels, pool, &mut rng).map_err(|e| e.to_string())?;
    for e in &events {
        report_event(e);
    }
    eprintln!(
        "Slot machine: {} cards on the reels {:?}, lines enabled {:?}, random slip {}",
        reels.strip.len(),
        reels.strip,
        reels.enabled_lines,
        reels.random_slip
    );
    let title = caption(&machine, None);
    world.insert_resource(SlotRuntime {
        machine,
        assets,
        textures,
        blocks,
        winner,
        rng,
        prizes: Vec::new(),
        last_prize: None,
        frame: 0,
        repeat: PadRepeat::default(),
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
        reported: Default::default(),
        finished: false,
    });
    world
        .run_system_cached(setup_slot)
        .map_err(|e| format!("slot machine setup: {e}"))
}

/// Remove the scene's resources; the blocks, RNG and prizes go back to the game.
pub fn leave_slot_scene(world: &mut World) -> Option<SlotResult> {
    world.remove_resource::<SlotRuntime>().map(|r| SlotResult {
        blocks: r.blocks,
        rng: r.rng,
        prizes: r.prizes,
        replay: r.replay,
    })
}

/// The machine has popped (0x1A9290): the map follows.
pub fn slot_scene_finished(world: &World) -> bool {
    world
        .get_resource::<SlotRuntime>()
        .is_some_and(|r| r.finished)
}

pub fn add_slot_materials(app: &mut App) {
    app.add_plugins(MaterialPlugin::<SlotMaterial>::default());
}

pub fn add_slot_systems(app: &mut App) {
    let active = resource_exists::<SlotRuntime>;
    app.add_systems(
        RunFixedMainLoop,
        slot_keys
            .run_if(active)
            .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
    )
    .add_systems(FixedUpdate, slot_tick.run_if(active));
}

fn setup_slot(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SlotMaterial>>,
    mut runtime: ResMut<SlotRuntime>,
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
        MeshMaterial3d(materials.add(SlotMaterial { frame: image })),
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

/// Keyboard → held pad buttons (the duel's key map), read every frame.
fn slot_keys(
    mut events: MessageReader<KeyboardInput>,
    mut focus_lost: MessageReader<KeyboardFocusLost>,
    mut runtime: ResMut<SlotRuntime>,
) {
    for event in events.read() {
        match event.state {
            ButtonState::Pressed if !event.repeat => {
                let bits = duel_play::key_pad(&format!("{:?}", event.key_code));
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

fn report_event(event: &SlotEvent) {
    match event {
        SlotEvent::SpinStarted => eprintln!("Slot machine: reels spinning"),
        SlotEvent::ReelStopping {
            reel,
            counter,
            slip,
        } => {
            eprintln!(
                "Slot machine: reel {reel} stopping in {counter} frames (slip {slip})"
            )
        }
        SlotEvent::ReelStopped {
            reel,
            rows,
            live_lines,
        } => {
            eprintln!("Slot machine: reel {reel} stopped on {rows:?}, live lines {live_lines:#x}")
        }
        SlotEvent::AllStopped { paying_lines } => {
            eprintln!(
                "Slot machine: all reels stopped, paying lines {paying_lines:#x}"
            )
        }
        SlotEvent::LineFlash { line } => eprintln!("Slot machine: line {line} flashes"),
        SlotEvent::PrizeScreen => eprintln!("Slot machine: prize screen"),
        SlotEvent::Prize(_) => {}
        SlotEvent::PopupClosed => eprintln!("Slot machine: popup closed"),
        SlotEvent::Finished => eprintln!("Slot machine: finished, back to the map"),
    }
}

fn caption(machine: &SlotMachine, status: Option<&str>) -> String {
    let reels: Vec<String> = machine
        .reels
        .iter()
        .map(|r| format!("{:?}", r.state).to_lowercase())
        .collect();
    let mut parts = vec![
        "Roses - Graveyards Slot Machine".to_string(),
        format!("screen {:?}", machine.screen),
        format!("reels {}", reels.join("/")),
        "A=square stops reel 1, D=triangle reel 2, Z=circle reel 3; X closes a popup".to_string(),
    ];
    if let Some(s) = status {
        parts.push(s.to_string());
    }
    parts.join(" · ")
}

fn render_frame(runtime: &mut SlotRuntime) {
    let popup = current_popup(&runtime.machine, runtime.last_prize);
    let icon = ((runtime.frame / 5) % 3) as u8;
    let winner = usize::from(runtime.winner.min(1));
    // The HUD's DIMX matrix: the original popup frame matches it best
    // (87% of pixels exact against 69% without dithering).
    let dimx = runtime.assets.tables.dimx;
    let result = render_slot_screen(
        &runtime.machine,
        popup,
        &mut runtime.assets,
        &runtime.textures,
        &runtime.blocks[winner],
        icon,
        Some(dimx),
        &mut runtime.target,
    );
    match result {
        Ok(notes) => {
            for n in notes {
                if runtime.reported.insert(n.clone()) {
                    eprintln!("Slot machine: unsupported, not drawn: {n}");
                }
            }
        }
        Err(e) => {
            if runtime.reported.insert(e.clone()) {
                eprintln!("Slot machine: frame not drawn: {e}");
            }
        }
    }
}

/// One 25 Hz update: the pad (scripted words first), the mode update and
/// draw pass, the prizes into the winner's block, the main loop's two LCG
/// calls, then the frame.
/// The SYSTEM0 requests of an event (the project notes 5)
/// [F sites]: the lever 0x20 (0x2853A0, the only call of the spin start)
/// then the spin loop 0x1E (0x2815CC); a stop button 0x21; all stopped
/// 0x217050(0, 0x1E); GET CARD 7, GET SPECIAL CARD 0x19.
pub fn slot_sounds(event: &SlotEvent) -> Vec<AudioCue> {
    let se = |id| AudioCue::Se { slot: 0, id, skip: false };
    match event {
        SlotEvent::SpinStarted => vec![se(0x20), se(0x1E)],
        SlotEvent::ReelStopping { .. } => vec![se(0x21)],
        SlotEvent::AllStopped { .. } => vec![AudioCue::StopSe { slot: 0, id: 0x1E }],
        SlotEvent::Prize(p) => vec![se(match p.kind {
            PrizeKind::GetCard => 7,
            PrizeKind::GetSpecialCard => 0x19,
        })],
        _ => Vec::new(),
    }
}

fn slot_tick(
    mut runtime: ResMut<SlotRuntime>,
    mut images: ResMut<Assets<Image>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut sound: Option<ResMut<super::duel_board_render::DuelSound>>,
) {
    let runtime = &mut *runtime;
    if runtime.finished {
        return;
    }
    let presses = match runtime.replay.pop_front() {
        Some(word) => word,
        None => {
            let held = runtime
                .held
                .values()
                .fold(runtime.tapped, |acc, &b| acc | b);
            runtime.repeat.frame(held)
        }
    };
    runtime.tapped = 0;
    let pressed = (presses & 0xF0) as u8;
    let winner = usize::from(runtime.winner.min(1));
    let SlotRuntime {
        machine,
        blocks,
        rng,
        prizes,
        assets,
        ..
    } = runtime;
    let mut grant = BlockGrant {
        block: &mut blocks[winner],
        art: &assets.art,
        prizes,
    };
    let events = machine.step(pressed, rng, &mut grant);
    next_rand31(rng);
    next_rand31(rng);
    runtime.frame += 1;
    for e in &events {
        report_event(e);
        if let Some(sound) = sound.as_mut() {
            for cue in slot_sounds(e) {
                sound.cue(cue);
            }
        }
        if let SlotEvent::Prize(p) = e {
            runtime.last_prize = Some((p.kind, p.card));
        }
        if matches!(e, SlotEvent::Finished) {
            runtime.finished = true;
        }
    }
    render_frame(runtime);
    if let Some(image) = runtime.image.clone() {
        if let Some(mut image) = images.get_mut(&image) {
            image.data = Some(runtime.target.to_rgba8(0, 0, FRAME, FRAME));
        }
    }
    let status = runtime
        .last_prize
        .map(|(k, c)| format!("last prize {k:?} {c}"));
    let title = caption(&runtime.machine, status.as_deref());
    if title != runtime.title {
        for mut window in &mut windows {
            window.title = title.clone();
        }
        runtime.title = title;
    }
}

#[cfg(test)]
mod sound_tests {
    use super::*;

    #[test]
    fn slot_events_make_the_original_requests() {
        let se = |id| AudioCue::Se { slot: 0, id, skip: false };
        assert_eq!(slot_sounds(&SlotEvent::SpinStarted), [se(0x20), se(0x1E)]);
        assert_eq!(slot_sounds(&SlotEvent::ReelStopping { reel: 1, counter: 4, slip: 0 }), [se(0x21)]);
        assert_eq!(slot_sounds(&SlotEvent::AllStopped { paying_lines: 0 }), [AudioCue::StopSe { slot: 0, id: 0x1E }]);
        let prize = |kind| SlotEvent::Prize(Prize { kind, card: 1, order: 0 });
        assert_eq!(slot_sounds(&prize(PrizeKind::GetCard)), [se(7)]);
        assert_eq!(slot_sounds(&prize(PrizeKind::GetSpecialCard)), [se(0x19)]);
        assert!(slot_sounds(&SlotEvent::PrizeScreen).is_empty());
    }
}
