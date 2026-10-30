//! The map editor (mode 0xD, Custom Duel's Edit Map) as a scene of the
//! connected game: the model `roses_client::map_editor` runs once per 25 Hz
//! update on the keyboard pad (the duel key map: arrows, X Cross, A Square,
//! Z Circle, S Start, Q/E L1/R1), the board is the duel board's renderer
//! with the editor's straight-down camera and cell fills
//! (`duel_board_render::EditorBoard`), the 2D layer
//! `map_editor_screen::render_layer`. Cross / Square / Start save the board
//! into the chosen own map of the player's card block (RAM, as the
//! original: the memory card is written by Custom Duel's Save row), Circle
//! leaves without saving; both return to Custom Duel's Edit Map row without
//! init −1 (the mode switch calls no init for 0xD's pop) and keep the
//! music. Notes: `work/research/modes/map-editor/NOTES.md`.
use bevy::{
    input::{ButtonState, keyboard::{KeyboardFocusLost, KeyboardInput}},
    prelude::*,
};
use roses_game::duel_hud_picture::TerrainPicture;
use roses_game::{duel_board, duel_play, map_editor_screen};
use roses_client::duel_hud::{HudTables, HudTextures, YesNoTables};
use roses_client::map_editor::{self as me, EditorEvent, EditorPad, MapEditor, Pulse};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Resource)]
pub struct MapEditorState {
    pub editor: MapEditor,
    tables: me::EditorTables,
    hud: HudTables,
    yes_no: YesNoTables,
    textures: HudTextures,
    picture: Option<TerrainPicture>,
    forward: [f64; 3],
    held: BTreeMap<KeyCode, u16>,
    tapped: u16,
    repeat: duel_play::PadRepeat,
    /// `ROSES_MAP_EDITOR_REPLAY`: pad words per update (the duel's
    /// `--replay-pad` syntax), consumed before the keyboard.
    replay: VecDeque<u16>,
    pub updates: u64,
    version: u64,
    /// The pop: `Some(save)` once Cross / Square / Start (`Some`) or Circle
    /// (`None`) ended the editor.
    pub done: Option<Option<(usize, [u8; 49])>>,
    reported: BTreeSet<String>,
}

/// Enter mode 0xD on own map `map` of the player's block (Custom Duel's
/// Edit Map: the block's map copied into the board, 0x1C79D8).
pub fn enter_scene(world: &mut World, own_top: u8, own_count: u8) -> Result<(), String> {
    let (duel_dir, board_manifest, block, wave) = {
        let flow = world.resource::<super::GameFlow>();
        (
            flow.import.duel.clone(),
            flow.import.duel_board_manifest.clone(),
            flow.player_block.clone().ok_or("the map editor needs the player's card block")?,
            (flow.scroll_word, flow.scroll_p1),
        )
    };
    let (wave, p1) = wave;
    let map = usize::from((own_top + 2) % own_count.max(1));
    let cells = block.own_map(map).ok_or(format!("own map {map} is missing"))?;
    let data = duel_play::load(&duel_dir)?;
    let mut codes = BTreeMap::new();
    for chunk in HudTables::required_messages().chunks(256) {
        for m in roses_assets::parse_pal_messages(&data.messages, chunk).map_err(|e| e.to_string())?.messages {
            codes.insert(m.message_id, m.expanded_codes);
        }
    }
    let hud = HudTables::from_executable(&data.executable, codes).map_err(|e| e.to_string())?;
    let yes_no = YesNoTables::from_executable(&data.executable).map_err(|e| e.to_string())?;
    let textures = HudTextures::new(&data.texsys, &data.picmini, [0, 0], &data.messages).map_err(|e| e.to_string())?;
    let tables = me::EditorTables::from_executable(&data.executable)?;
    // 0x2DB826 and 0x2DB828 as the last steppers left them (`GameFlow`).
    let editor = MapEditor::new(cells, own_top, own_count, 0, Pulse::new(p1.value, p1.rising), Pulse::new(wave.value, wave.rising));
    let target = me::CAMERA_TARGET.map(f64::from);
    let polar = me::CAMERA_POLAR.map(f64::from);
    let scene = super::duel_board_render::prepare_editor_scene(&board_manifest, cells, target, polar)?;
    let board = duel_board::load_board(&board_manifest)?;
    let mut reported = BTreeSet::new();
    let picture = match TerrainPicture::new(&board) {
        Ok(p) => Some(p),
        Err(e) => {
            reported.insert(format!("terrain picture: {e}"));
            eprintln!("Map editor: not drawn: terrain picture ({e})");
            None
        }
    };
    let camera = duel_board::BoardCamera { cursor: (3, 3), target, track: None, polar, polar_track: None, zoom: 1.0 };
    let eye = camera.eye();
    let forward = std::array::from_fn(|i| target[i] - eye[i]);
    eprintln!("Game: the map editor opens own map {map} (Edit Map list top {own_top}, count {own_count})");
    world.insert_resource(MapEditorState {
        editor,
        tables,
        hud,
        yes_no,
        textures,
        picture,
        forward,
        held: BTreeMap::new(),
        tapped: 0,
        repeat: Default::default(),
        replay: std::env::var("ROSES_MAP_EDITOR_REPLAY")
            .ok()
            .and_then(|r| duel_play::parse_replay_pad(&r).map_err(|e| eprintln!("Map editor: replay: {e}")).ok())
            .unwrap_or_default()
            .into(),
        updates: 0,
        version: 0,
        done: None,
        reported,
    });
    scene.enter(world)?;
    // The first frame before any update (init's state).
    draw(world);
    Ok(())
}

/// Leave the scene: the board's resources go, the save (if any) and the
/// flash word are handed back.
pub fn leave_scene(world: &mut World) -> Option<(Option<(usize, [u8; 49])>, roses_client::duel_hud::FlashWave, roses_client::duel_hud::FlashWave, u8)> {
    super::duel_board_render::leave_editor_scene(world);
    let st = world.remove_resource::<MapEditorState>()?;
    let (p1, p2) = (st.editor.p1, st.editor.p2);
    Some((
        st.done.flatten(),
        roses_client::duel_hud::FlashWave { value: p2.value, rising: p2.rising() },
        roses_client::duel_hud::FlashWave { value: p1.value, rising: p1.rising() },
        st.editor.d63,
    ))
}

pub fn add_systems(app: &mut App) {
    let on = resource_exists::<MapEditorState>.and_then(in_scene);
    app.add_systems(
        RunFixedMainLoop,
        keys.run_if(on.clone()).in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
    )
    .add_systems(FixedUpdate, tick.run_if(on).in_set(super::duel_board_render::EditorTick));
    super::duel_board_render::add_editor_systems(app);
}

fn in_scene(flow: Option<Res<super::GameFlow>>) -> bool {
    flow.is_some_and(|f| f.scene == super::GameScene::MapEditor && f.next.is_none())
}

fn keys(mut events: MessageReader<KeyboardInput>, mut focus: MessageReader<KeyboardFocusLost>, mut st: ResMut<MapEditorState>) {
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

/// One 25 Hz update 0x1CD8C0, then draw 0x1CDE40.
fn tick(world: &mut World) {
    let extra = world.get_resource_mut::<super::pad_input::Pads>().map_or(0, |mut p| p.pad1_extra());
    let Some(mut st) = world.get_resource_mut::<MapEditorState>() else { return };
    if st.done.is_some() {
        return;
    }
    let held = match st.replay.pop_front() {
        Some(w) => w,
        None => st.held.values().fold(st.tapped, |a, &b| a | b) | extra,
    };
    st.tapped = 0;
    let fresh = held & !st.repeat.held();
    let repeat = st.repeat.frame(held) | fresh;
    st.updates += 1;
    let events = st.editor.update(EditorPad { pressed: fresh, repeat, held });
    let mut sfx = Vec::new();
    for e in events {
        match e {
            EditorEvent::Sfx(id) => sfx.push(id),
            EditorEvent::Save { map, cells } => {
                eprintln!("Map editor: saved own map {map}: {cells:?}");
                st.done = Some(Some((map, cells)));
            }
            EditorEvent::Cancel => {
                eprintln!("Map editor: left without saving");
                st.done = Some(None);
            }
        }
    }
    let done = st.done.is_some();
    if let Some(mut sound) = world.get_resource_mut::<super::duel_board_render::DuelSound>() {
        for id in sfx {
            sound.cue(roses_game::duel_audio::AudioCue::Se { slot: 0, id: id as u8, skip: false });
        }
    }
    draw(world);
    let settled = world.get_resource::<MapEditorState>().is_some_and(|s| s.replay.is_empty());
    let mut flow = world.resource_mut::<super::GameFlow>();
    if settled && !flow.map_editor_settled && std::env::var_os("ROSES_MAP_EDITOR_REPLAY").is_some() {
        eprintln!("Map editor: replay used up");
    }
    flow.map_editor_settled = settled && !done;
    if done {
        flow.next = Some(super::GameScene::CustomDuel);
    }
}

/// Draw 0x1CDE40: the overlay step, the 2D layer, both handed to the board.
fn draw(world: &mut World) {
    let Some(mut st) = world.remove_resource::<MapEditorState>() else { return };
    let cross = st.tables.cross;
    st.editor.draw_overlay(cross);
    let s = &mut st;
    let (layer, notes) = map_editor_screen::render_layer(
        &s.editor,
        &s.tables,
        &s.hud,
        &s.yes_no,
        &mut s.textures,
        s.picture.as_mut(),
        s.forward,
    );
    for n in notes {
        if s.reported.insert(n.clone()) {
            eprintln!("Map editor: not drawn: {n}");
        }
    }
    s.version += 1;
    if let Some(mut board) = world.get_resource_mut::<super::duel_board_render::EditorBoard>() {
        board.terrain = s.editor.cells;
        board.fills = std::array::from_fn(|i| s.editor.overlay[i].fill_rgba());
        board.hud = Some((layer, s.version));
    }
    world.insert_resource(st);
}
