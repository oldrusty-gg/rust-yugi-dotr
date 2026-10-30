//! Custom Duel (title mode 9) as a scene of the connected game: the title's
//! memory-card op 3 hands over slot 1's file and the slot-2 result; the
//! menu model `roses_client::custom_duel` runs once per 25 Hz update on the
//! keyboard pad (the duel key map: arrows, X Cross, A Square, Z Circle,
//! S Start, Q/E L1/R1) and is drawn by `custom_duel_screen` into a 512×512
//! frame (the deck editor's present shader). Build Deck opens the deck
//! editor, Save the op-1 dialog over the menu, Start the duel; the duel (and
//! its reels) and the deck editor come back here with init −1, as the
//! original's mode switch does. Notes: `work/research/modes/
//! custom-duel/NOTES.md`.
//!
//! Two players (slot 2 holding a save: selector 0; `work/research/
//! modes/two-player/NOTES.md`): side 1 is slot 2's card block, played on
//! pad 2 (`pad_input::Pads`: the second keyboard layout or gamepad), its
//! name plate MSG 5 = slot 2's player name; the duel seats two humans
//! (controls 100/101). The 2P menu has no Save row: slot 2's block is
//! never written by Custom Duel, as in the original.
//!
//! Edit Map (only with the both-routes mark) opens the map editor
//! (map_editor_render, mode 0xD) on the chosen own map; it comes back here
//! without init −1. Explicit boundary: the mode fades.
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::{NoFrustumCulling, RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    input::{ButtonState, keyboard::{KeyboardFocusLost, KeyboardInput}},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use roses_game::custom_duel_screen::{self as screen, Leader, ScreenData, ScreenInputs, SideView};
use roses_game::{duel_play, game, memory_card_store as store};
use roses_client::custom_duel::{self as cd, CustomDuel, DuelStart, MenuEvent, MenuPad, SideOneBlock};
use roses_client::duel_hud::{HudTables, HudTextures, YesNoTables};
use roses_client::gs::Target16;
use roses_client::memory_card::{self as mc, CardDialog, CardOutcome};
use roses_core::card_block::{CardBlock, EMPTY_CARD};
use roses_core::deck_editor::{self, StaticCards};
use std::collections::BTreeMap;

const FRAME: usize = 512;

/// The Custom Duel flow: lives from the title's op 3 until the menu
/// returns to the title (kept across the deck editor and the duel).
#[derive(Resource)]
pub struct CustomDuelState {
    pub menu: CustomDuel,
    /// Side 1's block as init prepares it (selector 1: the mirror).
    side_one: SideOneBlock,
    computer_block: CardBlock,
    /// SAVE as loaded from slot 1 (op 1 writes it back unchanged).
    save: [u8; roses_core::memory_card::SAVE_BYTES],
    /// The Start being played; `returning` = the next entry is init −1.
    pub start: Option<DuelStart>,
    returning: bool,
    /// The op-1 dialog Save started (runs inside the row's update).
    dialog: Option<(CardDialog, store::VirtualCard)>,
    held: BTreeMap<KeyCode, u16>,
    tapped: u16,
    repeat: duel_play::PadRepeat,
    /// Pad 2's auto-repeat (two players).
    repeat2: duel_play::PadRepeat,
    view: Option<View>,
    /// What the menu asked of the game this update.
    request: Option<Request>,
    reported: std::collections::BTreeSet<String>,
    /// Test aids: `ROSES_CUSTOM_DUEL_REPLAY` (pad words per update, the
    /// duel's `--replay-pad` syntax, consumed before the keyboard, across
    /// re-entries), `ROSES_CUSTOM_DUEL_CAPTURE=<new.png>` written when the
    /// replay is used up (`ROSES_CUSTOM_DUEL_CAPTURE_AT=N`: after update N
    /// instead) and the game exits.
    replay: std::collections::VecDeque<u16>,
    /// `ROSES_CUSTOM_DUEL_REPLAY2`: pad 2's words, the same syntax.
    replay2: std::collections::VecDeque<u16>,
    updates: u64,
    /// SAVE+0x24: the player's name words (message 4's slots).
    name: Vec<u16>,
    /// Slot 2's player name (message 5, two players).
    name2: Vec<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    BuildDeck,
    Duel,
    Title,
    /// Edit Map: mode 0xD on own map (0xA9C08C + 2) % 0xA9C08D.
    MapEditor,
}

struct View {
    data: ScreenData,
    tables: HudTables,
    yes_no: YesNoTables,
    textures: HudTextures,
    picmini: Vec<u8>,
    cards: StaticCards,
    default_maps: Vec<Vec<u8>>,
    decks: Vec<roses_assets::DeckRecord>,
    target: Target16,
    image: Option<Handle<Image>>,
}

/// The title's op 3 returned 1: restore the player's block, seat side 1
/// and open mode 9 (track 3 on entry [F UI-SOUNDS]).
pub fn title_custom_duel(world: &mut World, slot1: Vec<u8>, result: u16, slot2: Option<Vec<u8>>) -> Result<(), String> {
    let selector = cd::selector_from_result(result).ok_or(format!("op 3 result {result:#x} has no selector"))?;
    let restored = store::restore(&slot1)?;
    let save = roses_core::memory_card::SaveFile::parse(&slot1).map_err(|e| e.to_string())?.file.save;
    let side_one = cd::side_one_block(selector).ok_or("no selector")?;
    let block = restored.block;
    let mut name2 = Vec::new();
    let (computer_block, record_note) = match side_one {
        SideOneBlock::SecondCard => {
            // Selector 0: side 1 = slot 2's block, control 101 (pad 2).
            let file = slot2.ok_or("op 3 reported a save in slot 2 but returned no file")?;
            let second = store::restore(&file)?;
            name2 = second.record.name_codes.clone();
            (second.block, "nobody: a second player on pad 2 with MEMORY CARD slot 2's block (control 101)".to_string())
        }
        SideOneBlock::PlayerCopy => (cd::mirror_block(&block), "a copy of the player's block (control 22)".to_string()),
        SideOneBlock::StaticDeck(record) => (CardBlock::empty(0), format!("deck record {record:#x} (control {})", cd::Opponent::of(selector).map_or(0, |o| o.control()))),
    };
    let blocks = [block.clone(), computer_block.clone()];
    let menu = CustomDuel::new(selector, &blocks);
    eprintln!("Game: Custom Duel: op 3 result {result:#x}, selector {selector}: the computer plays {record_note}");
    world.insert_resource(CustomDuelState {
        menu,
        side_one,
        computer_block,
        save,
        start: None,
        returning: false,
        dialog: None,
        held: BTreeMap::new(),
        tapped: 0,
        repeat: Default::default(),
        repeat2: Default::default(),
        view: None,
        request: None,
        reported: Default::default(),
        replay: std::env::var("ROSES_CUSTOM_DUEL_REPLAY")
            .ok()
            .and_then(|r| duel_play::parse_replay_pad(&r).map_err(|e| eprintln!("Custom Duel: replay: {e}")).ok())
            .unwrap_or_default()
            .into(),
        replay2: Default::default(),
        updates: 0,
        name: restored.record.name_codes.clone(),
        name2,
    });
    let mut flow = world.resource_mut::<super::GameFlow>();
    flow.player_block = Some(block);
    flow.needs_block = false;
    flow.next = Some(super::GameScene::CustomDuel);
    Ok(())
}

/// Edit Map's own-map list (0xA9C08C top, 0xA9C08D count) the map editor
/// saves into.
pub fn edit_list(world: &World) -> Option<(u8, u8)> {
    world.get_resource::<CustomDuelState>().map(|s| (s.menu.own_top, s.menu.own_count))
}

/// Whether the Custom Duel flow is active (the deck editor, the duel and the
/// reels return to it).
pub fn active(world: &World) -> bool {
    world.contains_resource::<CustomDuelState>()
}

/// Enter (or re-enter with init −1) the menu scene.
pub fn enter_scene(world: &mut World) -> Result<(), String> {
    let (duel_dir, player_block, second_after_duel) = {
        let flow = world.resource::<super::GameFlow>();
        (
            flow.import.duel.clone(),
            flow.player_block.clone().ok_or("Custom Duel needs the player's card block")?,
            flow.blocks.as_ref().map(|b| b[1].clone()),
        )
    };
    if let Some(mut sound) = world.get_resource_mut::<super::duel_board_render::DuelSound>() {
        sound.cue(roses_game::duel_audio::AudioCue::Music { track: 3, keep: true });
    }
    let view = {
        let st = world.resource::<CustomDuelState>();
        if st.view.is_none() { Some(load_view(&duel_dir, &st.name, &st.name2)?) } else { None }
    };
    let mut st = world.resource_mut::<CustomDuelState>();
    if let Some(v) = view {
        st.view = Some(v);
    }
    if std::mem::take(&mut st.returning) {
        // Init −1: the selector branch again (the mirror re-copies the
        // block), the duel words from the committed LPs.
        if st.side_one == SideOneBlock::PlayerCopy {
            st.computer_block = cd::mirror_block(&player_block);
        }
        if st.side_one == SideOneBlock::SecondCard {
            // The duel's end wrote its experience into both blocks (RAM only).
            if let Some(block) = second_after_duel {
                st.computer_block = block;
            }
        }
        st.menu.resume();
        st.start = None;
        eprintln!("Game: Custom Duel resumes (state {:#x})", st.menu.players[0].state);
    }
    world.run_system_cached(setup_scene).map_err(|e| format!("custom duel setup: {e}"))?;
    Ok(())
}

fn load_view(duel: &std::path::Path, name: &[u16], name2: &[u16]) -> Result<View, String> {
    let data = duel_play::load(duel)?;
    let mut codes = BTreeMap::new();
    let mut ids = HudTables::required_messages();
    ids.extend([4u16, 5]);
    ids.extend(127..=146);
    ids.extend(mc::msg::ALL);
    ids.sort_unstable();
    ids.dedup();
    for chunk in ids.chunks(256) {
        for m in roses_assets::parse_pal_messages(&data.messages, chunk).map_err(|e| e.to_string())?.messages {
            codes.insert(m.message_id, m.expanded_codes);
        }
    }
    // Message 4 is twelve player-name slots; the compose stops at the
    // name's end bit (as the map HUD does).
    let end = name.iter().position(|c| c & 0x8000 != 0).map_or(name.len(), |i| i + 1);
    let name: Vec<u16> = name[..end].iter().map(|c| c & 0x7FFF).collect();
    if !name.is_empty() {
        codes.insert(4, name);
    }
    let end = name2.iter().position(|c| c & 0x8000 != 0).map_or(name2.len(), |i| i + 1);
    let name2: Vec<u16> = name2[..end].iter().map(|c| c & 0x7FFF).collect();
    if !name2.is_empty() {
        codes.insert(5, name2);
    }
    let tables = HudTables::from_executable(&data.executable, codes)?;
    let maps = roses_assets::parse_pal_rules(&data.executable).map_err(|e| e.to_string())?.maps;
    let default_maps = (0..24).filter_map(|i| maps.get(0x16 + i).map(|m| m.cells.clone())).collect();
    Ok(View {
        data: ScreenData::from_executable(&data.executable)?,
        yes_no: YesNoTables::from_executable(&data.executable)?,
        textures: HudTextures::new(&data.texsys, &data.picmini, [0, 0], &data.messages)?,
        cards: StaticCards::from_executable(&data.executable, 0).ok_or("the executable is too short for the card records")?,
        picmini: data.picmini,
        tables,
        default_maps,
        decks: roses_assets::parse_pal_decks(&data.executable).map_err(|e| e.to_string())?.decks,
        target: Target16 { width: FRAME, height: FRAME, pixels: vec![0; FRAME * FRAME] },
        image: None,
    })
}

pub fn add_systems(app: &mut App) {
    let on = resource_exists::<CustomDuelState>.and_then(in_scene);
    app.add_systems(
        RunFixedMainLoop,
        keys.run_if(on.clone()).in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
    )
    .add_systems(FixedUpdate, tick.run_if(on));
}

fn in_scene(flow: Option<Res<super::GameFlow>>) -> bool {
    flow.is_some_and(|f| f.scene == super::GameScene::CustomDuel && f.next.is_none())
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<super::deck_editor_render::EditorMaterial>>,
    mut st: ResMut<CustomDuelState>,
    flow: Res<super::GameFlow>,
) {
    let st = &mut *st;
    draw(st, flow.player_block.as_ref());
    let Some(view) = st.view.as_mut() else { return };
    let mut image = Image::new(
        Extent3d { width: FRAME as u32, height: FRAME as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        view.target.to_rgba8(0, 0, FRAME, FRAME),
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
    view.image = Some(image.clone());
    let quad = meshes.add(Rectangle::new(2., 2.));
    commands.spawn((
        Mesh3d(quad),
        MeshMaterial3d(materials.add(super::deck_editor_render::EditorMaterial { frame: image })),
        Transform::IDENTITY,
        NoFrustumCulling,
        RenderLayers::layer(1),
    ));
    commands.spawn((
        Camera3d::default(),
        Camera { order: 1, ..default() },
        Transform::IDENTITY,
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        RenderLayers::layer(1),
        super::PresentationCamera,
    ));
}

fn keys(mut events: MessageReader<KeyboardInput>, mut focus: MessageReader<KeyboardFocusLost>, mut st: ResMut<CustomDuelState>) {
    for e in events.read() {
        match e.state {
            ButtonState::Pressed if !e.repeat => {
                let bits = duel_play::key_pad(&format!("{:?}", e.key_code));
                if bits != 0 {
                    st.held.insert(e.key_code, bits);
                    st.tapped |= bits;
                }
            }
            ButtonState::Pressed => {}
            ButtonState::Released => {
                st.held.remove(&e.key_code);
            }
        }
    }
    if focus.read().count() > 0 {
        st.held.clear();
    }
}

/// The side views for the panels from the blocks.
fn side_view(cards: &StaticCards, block: &CardBlock, name: u16) -> SideView {
    let deck = deck_editor::current_deck(block);
    let record = deck_editor::deck_record(block, deck);
    let leader = (record.leader != EMPTY_CARD).then(|| Leader {
        card: record.leader,
        kind: cards.record(record.leader).map_or(0, |r| r[0] & 0x1F),
        rank: block.stored_rank(record.leader).map_or(0, |r| r.min(12) as u8),
    });
    let deck_leaders = [0u8, 1, 2].map(|d| {
        let r = deck_editor::deck_record(block, d);
        (r.is_valid()).then_some(r.leader)
    });
    SideView { name, leader, deck, cost: record.cost, deck_leaders }
}

fn draw(st: &mut CustomDuelState, player: Option<&CardBlock>) {
    let Some(view) = st.view.as_mut() else { return };
    let Some(player) = player else { return };
    let blocks = [player, &st.computer_block];
    // 0x1C8C00: side 1's plate is MSG 5 with two players, else 0x8D +
    // selector [F].
    let names = [4u16, if st.menu.selector == 0 { 5 } else { 141 + u16::from(st.menu.selector) }];
    let mut sides = [0usize, 1].map(|s| side_view(&view.cards, blocks[s], names[s]));
    if let SideOneBlock::StaticDeck(record) = st.side_one {
        // The static record (0x214120(1, record)) is side 1's deck, its
        // leader at the record's rank.
        if let Some(v) = static_leader(view, record, &view.decks) {
            sides[1] = SideView { name: names[1], ..v };
        }
    }
    let inp = ScreenInputs {
        menu: &st.menu,
        sides,
        has_maps: [0usize, 1].map(|s| roses_client::ending::both_routes_marked(blocks[s])),
        default_maps: &view.default_maps,
        own_maps: [0usize, 1].map(|s| (0..roses_core::card_block::OWN_MAP_COUNT).filter_map(|i| blocks[s].own_map(i)).collect()),
    };
    let built = screen::build(&view.data, &view.tables, &inp);
    let mut notes = built.notes.clone();
    notes.extend(screen::render(&built, &view.tables, &view.yes_no, &mut view.textures, &view.picmini, &mut view.target));
    if let Some((d, _)) = &st.dialog {
        if let Some(bx) = d.overlay() {
            let frame = roses_client::duel_hud::overlay_box(&view.tables, &view.yes_no, &store::overlay(bx), d.counter);
            roses_client::duel_hud::render_primitives(&frame.prims, &view.tables, &mut view.textures, &mut view.target);
        }
    }
    for n in notes {
        if st.reported.insert(n.clone()) {
            eprintln!("Custom Duel: not drawn: {n}");
        }
    }
}

/// Side 1's panel for a static deck record (selectors 2..4).
fn static_leader(view: &View, record: u8, exe_decks: &[roses_assets::DeckRecord]) -> Option<SideView> {
    let r = exe_decks.get(usize::from(record))?;
    Some(SideView {
        name: 0,
        leader: Some(Leader {
            card: r.leader_card,
            kind: view.cards.record(r.leader_card).map_or(0, |c| c[0] & 0x1F),
            rank: r.leader_rank_raw.min(12),
        }),
        deck: 0,
        // 0x2153D0: the sum of the cards' byte 3.
        cost: r.cards.iter().filter_map(|&c| view.cards.record(c).map(|rec| u16::from(rec[3]))).sum(),
        deck_leaders: [Some(r.leader_card), None, None],
    })
}

/// One 25 Hz update: the pad, the save dialog the Save row waits on, the
/// menu, its events, the frame.
fn tick(world: &mut World) {
    let Some(mut st) = world.remove_resource::<CustomDuelState>() else { return };
    let (extra1, held2) = match world.get_resource_mut::<super::pad_input::Pads>() {
        Some(mut p) => (p.pad1_extra(), p.take(1)),
        None => (0, 0),
    };
    let held = match st.replay.pop_front() {
        Some(w) => w,
        None => st.held.values().fold(st.tapped, |a, &b| a | b) | extra1,
    };
    // Pad 2 (0x1C6FC0(1) runs only with two players).
    if st.updates == 0 {
        if let Ok(r) = std::env::var("ROSES_CUSTOM_DUEL_REPLAY2") {
            st.replay2 = duel_play::parse_replay_pad(&r).unwrap_or_default().into();
        }
    }
    let held2 = st.replay2.pop_front().unwrap_or(held2);
    let fresh2 = held2 & !st.repeat2.held();
    let repeat2 = st.repeat2.frame(held2) | fresh2;
    let pad2 = MenuPad { pressed: fresh2, repeat: repeat2 };
    st.updates += 1;
    let fresh = held & !st.repeat.held();
    let repeat = st.repeat.frame(held) | fresh;
    st.tapped = 0;
    let pad = MenuPad { pressed: fresh, repeat };
    let mut sfx = Vec::new();
    // Op 1 runs inside row 6's update (0x1ABB20); its box is drawn over
    // the menu.
    let mut card_idle = true;
    if let Some((mut d, mut card)) = st.dialog.take() {
        let cp = mc::CardPad {
            left: fresh & cd::pad::LEFT != 0,
            right: fresh & cd::pad::RIGHT != 0,
            cross: fresh & cd::pad::CROSS != 0,
            circle: fresh & cd::pad::CIRCLE != 0,
            square: fresh & cd::pad::SQUARE != 0,
            triangle: fresh & cd::pad::TRIANGLE != 0,
        };
        let out = d.update(cp, &mut card);
        sfx.extend(d.sfx.iter().copied());
        if out == CardOutcome::Running {
            card_idle = false;
            st.dialog = Some((d, card));
        } else if let Some(e) = &d.error {
            eprintln!("Custom Duel: Save: {e}");
        } else {
            eprintln!("Custom Duel: Save closed");
        }
    }
    let player = world.resource::<super::GameFlow>().player_block.clone();
    let Some(mut player) = player else {
        world.insert_resource(st);
        return;
    };
    let events = if card_idle || st.dialog.is_some() {
        let mut blocks = [player.clone(), st.computer_block.clone()];
        let ev = st.menu.update(
            if st.dialog.is_some() { [MenuPad::default(); 2] } else { [pad, pad2] },
            &mut blocks,
            card_idle,
        );
        let [p, c] = blocks;
        player = p;
        st.computer_block = c;
        ev
    } else {
        Vec::new()
    };
    for e in events {
        match e {
            MenuEvent::Sfx(id) => sfx.push(id),
            MenuEvent::BuildDeck => {
                st.returning = true;
                st.request = Some(Request::BuildDeck);
            }
            MenuEvent::Save => {
                let file = roses_core::memory_card::SaveFile::new(
                    player.to_bytes().unwrap_or_default(),
                    st.save,
                )
                .and_then(|f| f.to_bytes());
                match file {
                    Ok(file) => {
                        let root = world.resource::<super::GameFlow>().import.root.clone();
                        st.dialog = Some((CardDialog::save(file), store::VirtualCard::slot1(&root)));
                    }
                    Err(e) => eprintln!("Custom Duel: Save: the file cannot be built: {e}"),
                }
            }
            MenuEvent::EditMap { side, map } => {
                // 0x1C7988: the pad side's block's map goes to the board
                // and mode 0xD is pushed (no init −1 when it pops).
                eprintln!("Custom Duel: Edit Map on side {side}'s own map {map}");
                st.request = Some(Request::MapEditor);
            }
            MenuEvent::Title => st.request = Some(Request::Title),
            MenuEvent::Start(s) => {
                eprintln!(
                    "Custom Duel: Start: map {}, identities {:?}, LP {:?}, SP {:?}, leader abilities {:#x}, card display {:#x}",
                    s.map,
                    s.identities,
                    s.sides.map(|x| x.lp),
                    s.sides.map(|x| x.sp),
                    s.leader_abilities,
                    s.card_display
                );
                st.start = Some(s);
                st.returning = true;
                st.request = Some(Request::Duel);
            }
        }
    }
    if let Some(mut sound) = world.get_resource_mut::<super::duel_board_render::DuelSound>() {
        for id in sfx {
            sound.cue(roses_game::duel_audio::AudioCue::Se { slot: 0, id: id as u8, skip: false });
        }
    }
    world.resource_mut::<super::GameFlow>().player_block = Some(player.clone());
    draw(&mut st, Some(&player));
    if let Some(view) = st.view.as_ref() {
        if let Some(handle) = view.image.clone() {
            let rgba = view.target.to_rgba8(0, 0, FRAME, FRAME);
            if let Some(mut image) = world.resource_mut::<Assets<Image>>().get_mut(&handle) {
                image.data = Some(rgba);
            }
        }
    }
    let capture_now = match std::env::var("ROSES_CUSTOM_DUEL_CAPTURE_AT").ok().and_then(|v| v.parse::<u64>().ok()) {
        Some(n) => st.updates == n,
        None => st.replay.is_empty() && st.replay2.is_empty() && st.request.is_none() && std::env::var_os("ROSES_CUSTOM_DUEL_REPLAY").is_some(),
    };
    if capture_now {
        if let (Some(path), Some(view)) = (std::env::var_os("ROSES_CUSTOM_DUEL_CAPTURE"), st.view.as_ref()) {
            let path = std::path::PathBuf::from(path);
            match super::deck_editor_render::write_png(&path, &view.target.to_rgba8(0, 0, FRAME, FRAME)) {
                Ok(()) => eprintln!("Custom Duel: frame written to {} (update {})", path.display(), st.updates),
                Err(e) => eprintln!("Custom Duel: capture failed: {e}"),
            }
            world.write_message(AppExit::Success);
        }
    }
    world.insert_resource(st);
}

/// Called by the game's watch while the menu is on screen: the scene the
/// menu asked for.
pub fn watch(world: &mut World) {
    let Some(request) = world.get_resource_mut::<CustomDuelState>().and_then(|mut s| s.request.take()) else { return };
    let mut flow = world.resource_mut::<super::GameFlow>();
    match request {
        Request::BuildDeck => flow.next = Some(super::GameScene::DeckEditor),
        Request::MapEditor => flow.next = Some(super::GameScene::MapEditor),
        Request::Duel => {
            flow.won = None;
            flow.request = None;
            flow.next = Some(super::GameScene::Duel);
        }
        Request::Title => {
            eprintln!("Game: Custom Duel: YES on MSG 146: back to the title");
            flow.story_stage = Some("title".into());
            flow.next = Some(super::GameScene::Opening);
            world.remove_resource::<CustomDuelState>();
        }
    }
}

/// The duel a Start opens, with the computer's block for the end screens
/// (`None` outside Custom Duel).
pub fn duel_setup(world: &World, data: &duel_play::DuelData) -> Option<Result<(duel_play::FirstDuel, CardBlock), String>> {
    let st = world.get_resource::<CustomDuelState>()?;
    let start = st.start?;
    let flow = world.resource::<super::GameFlow>();
    Some((|| {
        let block = flow.player_block.clone().ok_or("no card block")?;
        let starter = flow.campaign.starter_deck.as_ref().map_or(0, |c| usize::from(c.record));
        let player = game::PlayerDeck::from_block(&block, starter)?;
        let decks = roses_assets::parse_pal_decks(&data.executable).map_err(|e| e.to_string())?.decks;
        let maps = roses_assets::parse_pal_rules(&data.executable).map_err(|e| e.to_string())?.maps;
        let opponent = match st.side_one {
            SideOneBlock::PlayerCopy => game::CustomOpponent::Mirror(st.computer_block.clone()),
            SideOneBlock::StaticDeck(r) => game::CustomOpponent::Record(usize::from(r)),
            SideOneBlock::SecondCard => game::CustomOpponent::Second(st.computer_block.clone()),
        };
        let duel = game::custom_duel(&start, &data.first_duel, &player, &block, &opponent, &decks, &maps, &data.executable)?;
        let computer = match &opponent {
            game::CustomOpponent::Mirror(b) | game::CustomOpponent::Second(b) => b.clone(),
            game::CustomOpponent::Record(_) => {
                let thresholds = roses_assets::parse_pal_rank_thresholds(&data.executable).map_err(|e| e.to_string())?;
                let mut rng = flow.rng;
                let rank = decks.get(duel.sides[1].deck_record).map_or(0, |d| u16::from(d.leader_rank_raw));
                roses_game::duel_end_screens::new_game_block(
                    roses_game::duel_end_screens::computer_owner_key(duel.sides[1].deck_record),
                    &duel.sides[1].cards,
                    duel.sides[1].leader,
                    rank,
                    &thresholds,
                    &mut rng,
                )
            }
        };
        Ok((duel, computer))
    })())
}
