//! Trade (title mode 8) as a scene of the connected game (Claude, 6 Oct
//! 2026): the title's memory-card op 4 hands over both slots' files; the
//! model `roses_client::trade` runs once per 25 Hz update on the two pads
//! (`pad_input::Pads`: pad 1 = the duel keyboard layout + C/V for L3/R3 or
//! the first gamepad, pad 2 = T/F/G/H, K/J/L/I … or the second gamepad;
//! port = side, 0x1C3650(side)) and is drawn by `trade_screen` into a
//! 512×512 frame (the deck editor's present shader). YES with every copy
//! moved starts op 5 (both cards written, each with its own SAVE); the
//! mode then pops to the title, as does Cancel's YES or the refusal's
//! press. Notes: `work/research/modes/two-player/NOTES.md`.
//!
//! The card detail pane (DTL, 0x1BE6E0) is `card_detail`'s; its picture is
//! the import's PICTURE.MRG record (duel-end import), else the pane shows
//! the original's NOW LOADING box. Explicit boundary: the mode fades.
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::{NoFrustumCulling, RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use roses_game::trade_screen::{self, TradeData};
use roses_game::{duel_play, memory_card_store as store};
use roses_client::duel_hud::{HudTables, HudTextures, YesNoTables};
use roses_client::gs::Target16;
use roses_client::memory_card::{self as mc, CardDialog, CardOutcome};
use roses_client::trade::{Trade, TradeEvent, TradePad};
use roses_core::card_block::CardBlock;
use roses_core::deck_editor::StaticCards;
use roses_core::memory_card::{SAVE_BYTES, SaveFile};
use std::collections::{BTreeSet, VecDeque};

const FRAME: usize = 512;
/// Mode 8's music (0x1AB440(8, 1)) [F UI-SOUNDS].
const TRACK: u32 = 41;

#[derive(Resource)]
pub struct TradeState {
    trade: Option<Trade>,
    blocks: [CardBlock; 2],
    /// Each slot's SAVE as op 4 loaded it (op 5 writes it back unchanged).
    saves: [[u8; SAVE_BYTES]; 2],
    repeat: [duel_play::PadRepeat; 2],
    /// Op 5 while it runs (the trade update waits for the card module).
    dialog: Option<(CardDialog, store::VirtualCard, store::VirtualCard)>,
    view: Option<View>,
    title: bool,
    reported: BTreeSet<String>,
    /// Test aids: `ROSES_TRADE_REPLAY` / `ROSES_TRADE_REPLAY2` (pad words
    /// per update, the duel's `--replay-pad` syntax), `ROSES_TRADE_CAPTURE
    /// =<new.png>` written when both are used up (`ROSES_TRADE_CAPTURE_AT=N`:
    /// after update N), then the game exits.
    replay: [VecDeque<u16>; 2],
    updates: u64,
}

struct View {
    data: TradeData,
    tables: HudTables,
    yes_no: YesNoTables,
    textures: HudTextures,
    cards: StaticCards,
    /// PICTURE.MRG of the duel-end import (the detail pane's pictures).
    pictures: Option<roses_game::card_detail::Pictures>,
    target: Target16,
    image: Option<Handle<Image>>,
}

/// The title's op 4 returned 1: both blocks restored, mode 8.
pub fn title_trade(world: &mut World, slot1: Vec<u8>, slot2: Vec<u8>) -> Result<(), String> {
    let r1 = store::restore(&slot1)?;
    let r2 = store::restore(&slot2)?;
    let save = |f: &[u8]| SaveFile::parse(f).map(|p| p.file.save).map_err(|e| e.to_string());
    let replay = |var: &str| -> VecDeque<u16> {
        std::env::var(var)
            .ok()
            .and_then(|r| duel_play::parse_replay_pad(&r).map_err(|e| eprintln!("Trade: replay: {e}")).ok())
            .unwrap_or_default()
            .into()
    };
    eprintln!("Game: Trade: both MEMORY CARD slots loaded; {}", roses_game::pads::layout_note());
    world.insert_resource(TradeState {
        trade: None,
        blocks: [r1.block.clone(), r2.block],
        saves: [save(&slot1)?, save(&slot2)?],
        repeat: Default::default(),
        dialog: None,
        view: None,
        title: false,
        reported: BTreeSet::new(),
        replay: [replay("ROSES_TRADE_REPLAY"), replay("ROSES_TRADE_REPLAY2")],
        updates: 0,
    });
    let mut flow = world.resource_mut::<super::GameFlow>();
    flow.player_block = Some(r1.block);
    flow.needs_block = false;
    flow.next = Some(super::GameScene::Trade);
    Ok(())
}

/// 0x2DB828 as Trade left it (`GameFlow::scroll_word`).
/// 0x2DB826 as Trade left it.
pub fn scroll_p1(world: &World) -> Option<roses_client::duel_hud::FlashWave> {
    let t = world.get_resource::<TradeState>()?.trade.as_ref()?;
    Some(roses_client::duel_hud::FlashWave { value: t.scroll[0], rising: t.scroll_dir[0] > 0 })
}

pub fn scroll_word(world: &World) -> Option<roses_client::duel_hud::FlashWave> {
    let t = world.get_resource::<TradeState>()?.trade.as_ref()?;
    Some(roses_client::duel_hud::FlashWave { value: t.scroll[1], rising: t.scroll_dir[1] > 0 })
}

pub fn enter_scene(world: &mut World) -> Result<(), String> {
    let duel_dir = world.resource::<super::GameFlow>().import.duel.clone();
    let end_dir = world.resource::<super::GameFlow>().import.duel_end.clone();
    if let Some(mut sound) = world.get_resource_mut::<super::duel_board_render::DuelSound>() {
        sound.cue(roses_game::duel_audio::AudioCue::Music { track: TRACK, keep: true });
    }
    let view = load_view(&duel_dir, end_dir.as_deref())?;
    let wave = world.resource::<super::GameFlow>().scroll_word;
    let (p1, d63) = (world.resource::<super::GameFlow>().scroll_p1, world.resource::<super::GameFlow>().d63);
    {
        let mut st = world.resource_mut::<TradeState>();
        // Init 0x1C3050; it leaves 0x2DB828 as the previous mode left it
        // (only 0x1C35C0 / 0x1C35C8, the update, write it) [F].
        let mut t = Trade::new(&st.blocks, &view.cards);
        t.scroll[1] = wave.value;
        t.scroll_dir[1] = if wave.rising { 1 } else { -1 };
        t.scroll[0] = p1.value;
        t.scroll_dir[0] = if p1.rising { 1 } else { -1 };
        t.d63 = d63;
        st.trade = Some(t);
        st.view = Some(view);
    }
    if let Some(mut pads) = world.get_resource_mut::<super::pad_input::Pads>() {
        pads.pads.clear_taps();
    }
    world.run_system_cached(setup_scene).map_err(|e| format!("trade setup: {e}"))?;
    Ok(())
}

fn load_view(duel: &std::path::Path, end: Option<&std::path::Path>) -> Result<View, String> {
    let data = duel_play::load_with_end(duel, end)?;
    let mut ids = HudTables::required_messages();
    ids.extend(126..130);
    ids.extend(323..323 + 854);
    ids.extend(roses_game::card_detail::message_ids());
    ids.extend(mc::msg::ALL);
    ids.sort_unstable();
    ids.dedup();
    let mut codes = std::collections::BTreeMap::new();
    for chunk in ids.chunks(256) {
        for m in roses_assets::parse_pal_messages(&data.messages, chunk).map_err(|e| e.to_string())?.messages {
            codes.insert(m.message_id, m.expanded_codes);
        }
    }
    Ok(View {
        data: TradeData::from_executable(&data.executable)?,
        tables: HudTables::from_executable(&data.executable, codes)?,
        yes_no: YesNoTables::from_executable(&data.executable)?,
        textures: HudTextures::new(&data.texsys, &data.picmini, [0, 0], &data.messages)?,
        cards: StaticCards::from_executable(&data.executable, 0).ok_or("the executable is too short for the card records")?,
        pictures: match data.pictures.clone() {
            Some(p) => Some(roses_game::card_detail::Pictures::new(p, &data.executable)?),
            None => {
                eprintln!("Trade: no PICTURE.MRG (duel-end import): the detail pane shows NOW LOADING");
                None
            }
        },
        target: Target16 { width: FRAME, height: FRAME, pixels: vec![0; FRAME * FRAME] },
        image: None,
    })
}

pub fn add_systems(app: &mut App) {
    app.add_systems(FixedUpdate, tick.run_if(resource_exists::<TradeState>.and_then(in_scene)));
}

fn in_scene(flow: Option<Res<super::GameFlow>>) -> bool {
    flow.is_some_and(|f| f.scene == super::GameScene::Trade && f.next.is_none())
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<super::deck_editor_render::EditorMaterial>>,
    mut st: ResMut<TradeState>,
) {
    let st = &mut *st;
    draw(st, [false; 2]);
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

fn draw(st: &mut TradeState, select_held: [bool; 2]) {
    let (Some(view), Some(trade)) = (st.view.as_mut(), st.trade.as_ref()) else { return };
    // Each side's detail card picture (0x215E60 on that side's block).
    let mut textures: [Option<roses_client::gs::Texture>; 2] = [None, None];
    if let Some(p) = view.pictures.as_mut() {
        for side in 0..2 {
            if trade.sides[side].slide != 0 {
                textures[side] = p.texture(trade.sides[side].card(), &st.blocks[side]);
            }
        }
    }
    let pictures = [textures[0].is_some(), textures[1].is_some()];
    let inp = trade_screen::Inputs { trade, blocks: &st.blocks, cards: &view.cards, select_held, pictures, counter: st.updates as u32 };
    let screen = trade_screen::build(&view.data, &view.tables, &inp);
    let mut notes = screen.notes.clone();
    // Drawn in runs: each pane's run binds its side's picture first.
    let mut cuts: Vec<(usize, Option<usize>)> = vec![(0, None)];
    for (r, side) in &screen.panes {
        cuts.push((r.start, Some(*side)));
        cuts.push((r.end, None));
    }
    cuts.push((screen.calls.len(), None));
    for w in cuts.windows(2) {
        let ((start, side), (end, _)) = (w[0], w[1]);
        if start >= end {
            continue;
        }
        view.textures.picture = side.and_then(|s| textures[s].clone());
        let frame = roses_client::duel_hud::menu_frame(&view.tables, &view.yes_no, &screen.calls[start..end]);
        for u in &frame.unsupported {
            notes.push(format!("unsupported {:?}", u.kind));
        }
        for s in roses_client::duel_hud::render_primitives(&frame.prims, &view.tables, &mut view.textures, &mut view.target) {
            notes.push(format!("primitive {} skipped: {}", s.index + start, s.reason));
        }
    }
    if let Some((d, _, _)) = &st.dialog {
        if let Some(bx) = d.overlay() {
            let f = roses_client::duel_hud::overlay_box(&view.tables, &view.yes_no, &store::overlay(bx), d.counter);
            roses_client::duel_hud::render_primitives(&f.prims, &view.tables, &mut view.textures, &mut view.target);
        }
    }
    for n in notes {
        if st.reported.insert(n.clone()) {
            eprintln!("Trade: not drawn: {n}");
        }
    }
}

/// One 25 Hz update (0x1C3470 and the draw).
fn tick(world: &mut World) {
    let Some(mut st) = world.remove_resource::<TradeState>() else { return };
    let words = match world.get_resource_mut::<super::pad_input::Pads>() {
        Some(mut p) => [p.take(0), p.take(1)],
        None => [0, 0],
    };
    let analog = world.get_resource::<super::pad_input::Pads>().map_or([None; 2], |p| [p.analog(0), p.analog(1)]);
    let held = [0, 1].map(|p| st.replay[p].pop_front().unwrap_or(words[p]));
    st.updates += 1;
    let pads = [0, 1].map(|p| {
        let (pressed, repeat) = st.repeat[p].frame_split(held[p]);
        TradePad { pressed, held: held[p], repeat, analog: analog[p] }
    });
    let mut sfx = Vec::new();
    // 0x1ABB20 first: op 5 while it runs.
    let mut card_idle = true;
    if let Some((mut d, mut c1, mut c2)) = st.dialog.take() {
        let cp = mc::CardPad {
            left: pads[0].pressed & 0x8000 != 0,
            right: pads[0].pressed & 0x2000 != 0,
            cross: pads[0].pressed & 0x40 != 0,
            circle: pads[0].pressed & 0x20 != 0,
            square: pads[0].pressed & 0x80 != 0,
            triangle: pads[0].pressed & 0x10 != 0,
        };
        let out = d.update_slots(cp, &mut c1, &mut c2);
        sfx.extend(d.sfx.iter().copied());
        if out == CardOutcome::Running {
            card_idle = false;
            st.dialog = Some((d, c1, c2));
        } else {
            match &d.error {
                Some(e) => eprintln!("Trade: save: {e}"),
                None => eprintln!("Trade: both MEMORY CARDs written"),
            }
        }
    }
    let events = {
        let TradeState { trade, blocks, view, .. } = &mut st;
        match (trade.as_mut(), view.as_ref()) {
            (Some(t), Some(v)) => t.update(pads, blocks, &v.cards, card_idle),
            _ => Vec::new(),
        }
    };
    for e in events {
        match e {
            TradeEvent::Sfx(id) => sfx.push(id),
            TradeEvent::Save => {
                let files: Result<Vec<Vec<u8>>, String> = (0..2)
                    .map(|s| {
                        let block = st.blocks[s].to_bytes().map_err(|e| e.to_string())?;
                        SaveFile::new(block, st.saves[s]).and_then(|f| f.to_bytes()).map_err(|e| e.to_string())
                    })
                    .collect();
                match files {
                    Ok(f) => {
                        let root = world.resource::<super::GameFlow>().import.root.clone();
                        eprintln!("Trade: the trade is made; saving both MEMORY CARDs (op 5)");
                        st.dialog = Some((CardDialog::trade_save(f[0].clone(), f[1].clone()), store::VirtualCard::slot1(&root), store::VirtualCard::slot2(&root)));
                    }
                    Err(e) => eprintln!("Trade: the files cannot be built: {e}"),
                }
            }
            TradeEvent::Title => st.title = true,
        }
    }
    if let Some(mut sound) = world.get_resource_mut::<super::duel_board_render::DuelSound>() {
        for id in sfx {
            sound.cue(roses_game::duel_audio::AudioCue::Se { slot: 0, id: id as u8, skip: false });
        }
    }
    draw(&mut st, [held[0] & 0x100 != 0, held[1] & 0x100 != 0]);
    if let Some(view) = st.view.as_ref() {
        if let Some(handle) = view.image.clone() {
            let rgba = view.target.to_rgba8(0, 0, FRAME, FRAME);
            if let Some(mut image) = world.resource_mut::<Assets<Image>>().get_mut(&handle) {
                image.data = Some(rgba);
            }
        }
    }
    let replaying = std::env::var_os("ROSES_TRADE_REPLAY").is_some() || std::env::var_os("ROSES_TRADE_REPLAY2").is_some();
    let capture_now = match std::env::var("ROSES_TRADE_CAPTURE_AT").ok().and_then(|v| v.parse::<u64>().ok()) {
        Some(n) => st.updates == n,
        None => replaying && st.replay.iter().all(VecDeque::is_empty) && st.dialog.is_none(),
    };
    if capture_now {
        if let (Some(path), Some(view)) = (std::env::var_os("ROSES_TRADE_CAPTURE"), st.view.as_ref()) {
            let path = std::path::PathBuf::from(path);
            match super::deck_editor_render::write_png(&path, &view.target.to_rgba8(0, 0, FRAME, FRAME)) {
                Ok(()) => eprintln!("Trade: frame written to {} (update {})", path.display(), st.updates),
                Err(e) => eprintln!("Trade: capture failed: {e}"),
            }
            world.write_message(AppExit::Success);
        }
    }
    world.insert_resource(st);
}

/// The game's watch while the trade screen is on: back to the title.
pub fn watch(world: &mut World) {
    let done = world.get_resource::<TradeState>().is_some_and(|s| s.title);
    if !done {
        return;
    }
    let st = world.remove_resource::<TradeState>().expect("checked");
    eprintln!("Game: Trade: back to the title");
    let mut flow = world.resource_mut::<super::GameFlow>();
    flow.player_block = Some(st.blocks[0].clone());
    flow.story_stage = Some("title".into());
    flow.next = Some(super::GameScene::Opening);
}
